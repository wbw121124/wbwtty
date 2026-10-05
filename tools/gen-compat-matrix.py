#!/usr/bin/env python3
"""阶段 0 工具：SDK 兼容性矩阵生成器。

扫描 sdk-headers/<ver>/ 中的头文件，提取：
  1. 控制台 API 函数声明（consoleapi*.h / wincon.h）→ docs 中的控制台 API 表
  2. 宿主/桥接所需 kernel32 API（CURATED_KERNEL 白名单候选）→ API 表
  3. 关键宏/类型标记存在性（VT 处理、PSEUDOCONSOLE 等）→ 标记表
  4. 每个函数声明处的预处理器守卫（#if _WIN32_WINNT ... 等）

产出：
  - docs/sdk-compat-matrix.md 中 `<!-- BEGIN GENERATED ... -->` 之间的自动表格
    （标记之外的手写章节保持不动，重复执行幂等）
  - config/sdk-matrix.json      原始证据（供 linter / CI / 追溯）
  - config/api-whitelist.json   pty-win10-early 静态链接白名单（全部 7 版均存在才入选）
  - tools/sdk-probe/probe.c     CI 逐 SDK cl 编译矩阵使用的探针（引用白名单每个符号）

用法:
  python tools/gen-compat-matrix.py
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

VERSIONS = [
    "10.0.10240", "10.0.10586", "10.0.14393", "10.0.15063",
    "10.0.16299", "10.0.17134", "10.0.17763",
]
VERSION_LABEL = {
    "10.0.10240": "1507", "10.0.10586": "1511", "10.0.14393": "1607",
    "10.0.15063": "1703", "10.0.16299": "1709", "10.0.17134": "1803",
    "10.0.17763": "1809",
}

CONSOLE_FILES = {"consoleapi.h", "consoleapi2.h", "consoleapi3.h", "wincon.h"}

# pty-win10-early / pty-conpty 宿主与桥接需要静态调用的 kernel32 表面。
# 分组仅用于文档可读性；生成白名单时要求全部 7 版头文件均声明。
CURATED_KERNEL: dict[str, list[str]] = {
    "process": [
        "CreateProcessW", "CreateProcessAsUserW", "CreateProcessWithLogonW",
        "TerminateProcess", "GetExitCodeProcess", "OpenProcess", "GetProcessId",
        "ResumeThread", "SuspendThread", "GetThreadId", "CreateThread",
        "GetExitCodeThread",
    ],
    "thread-injection": [
        "CreateRemoteThread", "VirtualAllocEx", "VirtualFreeEx",
        "WriteProcessMemory", "ReadProcessMemory", "GetThreadContext",
        "SetThreadContext",
    ],
    "loader": [
        "LoadLibraryW", "LoadLibraryExW", "FreeLibrary", "GetProcAddress",
        "GetModuleHandleW", "GetModuleHandleExW",
    ],
    "file-pipe": [
        "CreateFileW", "ReadFile", "WriteFile", "CloseHandle", "DuplicateHandle",
        "CreatePipe", "CreateNamedPipeW", "ConnectNamedPipe", "DisconnectNamedPipe",
        "SetNamedPipeHandleState", "PeekNamedPipe", "FlushFileBuffers",
        "GetFileType", "GetFileSizeEx",
    ],
    "sync-wait": [
        "WaitForSingleObject", "WaitForMultipleObjects", "Sleep",
        "GetTickCount", "GetTickCount64", "QueryPerformanceCounter",
        "QueryPerformanceFrequency", "CreateEventW", "SetEvent", "ResetEvent",
    ],
    "console-host": [
        "GenerateConsoleCtrlEvent", "AttachConsole", "FreeConsole",
        "AllocConsole", "SetConsoleCtrlHandler", "GetConsoleWindow",
        "GetConsoleProcessList",
    ],
    "string": [
        "MultiByteToWideChar", "WideCharToMultiByte", "lstrlenW",
    ],
    "error": [
        "GetLastError", "SetLastError", "FormatMessageW",
    ],
}

# ConPTY：仅 17763+，必须动态加载，禁止静态链接
CONPTY_FUNCS = ["CreatePseudoConsole", "ResizePseudoConsole", "ClosePseudoConsole"]

MARKERS = [
    "ENABLE_VIRTUAL_TERMINAL_PROCESSING",
    "ENABLE_VIRTUAL_TERMINAL_INPUT",
    "ENABLE_PROCESSED_OUTPUT",
    "ENABLE_LVB_GRID_WORLDWIDE",
    "STARTUPINFOEXW",
    "PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE",
    "PSEUDOCONSOLE_INHERIT_CURSOR",
    "HPCON",
    "GetConsoleHistoryInfo",
    "GetConsoleProcessList",
    "GetTickCount64",
]

FUNC_RE = re.compile(r"\b(?:WINAPI|APIENTRY|NTAPI)\s+([A-Za-z_]\w*)\s*\(")
IF_RE = re.compile(r"^\s*#\s*(if|ifdef|ifndef|elif|else|endif)\b(.*)$")
INCLUDE_ANY = re.compile(r"^\s*#\s*include\b")


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", lambda m: "\n" * m.group(0).count("\n"), text, flags=re.S)
    text = re.sub(r"//[^\n]*", "", text)
    return text


def line_guards(text: str) -> list[tuple[str, ...]]:
    """为每一行（1-based 索引使用 lines[i-1]）计算激活的预处理器条件栈。"""
    guards: list[tuple[str, ...]] = []
    stack: list[str] = []
    for line in text.splitlines():
        guards.append(tuple(stack))
        m = IF_RE.match(line)
        if not m:
            continue
        kind, rest = m.group(1), m.group(2).strip()
        if kind in ("if", "ifdef", "ifndef"):
            stack.append(rest[:120])
        elif kind == "elif":
            if stack:
                stack[-1] = stack[-1] + " | elif " + rest[:80]
        elif kind == "else":
            if stack:
                stack[-1] = stack[-1] + " | else"
        elif kind == "endif":
            if stack:
                stack.pop()
    return guards


def scan_version(vdir: Path, top_dirs: set[str] | None = None) -> dict:
    """扫描 vdir 下头文件；top_dirs 非空时只扫描这些顶层目录（um/shared/ucrt/km）。"""
    funcs: dict[str, dict] = {}
    markers: dict[str, str | None] = {}
    marker_tokens = {m: None for m in MARKERS}
    headers = sorted(vdir.rglob("*.h"))
    if top_dirs is not None:
        headers = [h for h in headers
                   if h.relative_to(vdir).parts[0] in top_dirs]
    for h in headers:
        try:
            raw = h.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        rel = h.relative_to(vdir).as_posix()
        for tok in marker_tokens:
            if marker_tokens[tok] is None and tok in raw:
                # 记录首次出现的文件与行号
                idx = raw.find(tok)
                line_no = raw.count("\n", 0, idx) + 1
                marker_tokens[tok] = f"{rel}:{line_no}"
        clean = strip_comments(raw)
        guards = line_guards(clean)
        lines = clean.splitlines()
        for m in FUNC_RE.finditer(clean):
            name = m.group(1)
            line_no = clean.count("\n", 0, m.start()) + 1
            if name in funcs:
                continue
            g = guards[line_no - 1] if line_no - 1 < len(guards) else ()
            decl_line = lines[line_no - 1].strip() if line_no - 1 < len(lines) else ""
            funcs[name] = {
                "file": rel,
                "line": line_no,
                "guards": list(g),
                "preview": decl_line[:100],
            }
    return {"functions": funcs, "markers": marker_tokens,
            "header_count": len(headers)}


def table(matrix: dict, names: list[str], title_note: str = "") -> list[str]:
    out = []
    header = "| API | " + " | ".join(
        f"{VERSION_LABEL[v]}<br>{v.split('.')[-1]}" for v in VERSIONS) + " | 最低 SDK |"
    out.append(header)
    out.append("|---" * (len(VERSIONS) + 2) + "|")
    for name in names:
        cells = []
        first = None
        for v in VERSIONS:
            present = name in matrix[v]["functions"]
            if present and first is None:
                first = v
            cells.append("✓" if present else "**✗**")
        min_sdk = VERSION_LABEL[first] if first else "—"
        out.append(f"| `{name}` | " + " | ".join(cells) + f" | {min_sdk} |")
    if title_note:
        out.append("")
        out.append(title_note)
    return out


def marker_table(matrix: dict) -> list[str]:
    out = ["| 标记 | " + " | ".join(
        f"{VERSION_LABEL[v]}<br>{v.split('.')[-1]}" for v in VERSIONS) + " |",
           "|---" * (len(VERSIONS) + 1) + "|"]
    for tok in MARKERS:
        cells = []
        first = None
        for v in VERSIONS:
            loc = matrix[v]["markers"].get(tok)
            if loc and first is None:
                first = v
            cells.append(f"✓ `{loc}`" if loc else "**✗**")
        out.append(f"| `{tok}` | " + " | ".join(cells) +
                   (f" | {VERSION_LABEL[first]} |" if first else " | — |"))
    return out


def replace_region(doc: str, region: str, lines: list[str]) -> str:
    begin = f"<!-- BEGIN GENERATED: {region} -->"
    end = f"<!-- END GENERATED: {region} -->"
    block = begin + "\n" + "\n".join(lines) + "\n" + end
    if begin in doc and end in doc:
        pre, rest = doc.split(begin, 1)
        _, post = rest.split(end, 1)
        return pre + block + post
    return doc.rstrip() + "\n\n" + block + "\n"


def build_whitelist(matrix: dict) -> dict:
    static_allowed: dict[str, dict] = {}
    dynamic_only: dict[str, dict] = {}
    missing_everywhere: list[str] = []

    def all_present(name: str) -> str | None:
        for v in VERSIONS:
            if name not in matrix[v]["functions"]:
                return v
        return None

    for group, names in CURATED_KERNEL.items():
        for name in names:
            bad = all_present(name)
            if bad is None:
                info = matrix[VERSIONS[-1]]["functions"][name]
                static_allowed[name] = {
                    "group": group,
                    "min_sdk": "10.0.10240",
                    "declared_in": info["file"],
                }
            else:
                missing_everywhere.append(f"{name} (missing in {bad})")

    for name in CONPTY_FUNCS:
        info = matrix["10.0.17763"]["functions"].get(name)
        if info is None:
            missing_everywhere.append(f"{name} (missing in 10.0.17763)")
            continue
        dynamic_only[name] = {
            "min_sdk": "10.0.17763",
            "reason": "ConPTY 仅 Windows 10 1809+ 提供；1809 之前运行时加载失败须回退 pty-win10-early",
            "declared_in": info["file"],
        }

    return {
        "_comment": (
            "pty-win10-early 静态链接白名单。static_allowed 中的符号在全部 7 个 "
            "Windows 10 SDK 头文件中均有声明，允许在 pty-win10-early 静态引用；"
            "dynamic_only 中的符号必须 LoadLibrary+GetProcAddress 动态加载，"
            "严禁出现在 extern/import 中。其余任何 Win32 符号引用都会被 "
            "tools/check_api_whitelist.py 拒绝。"
        ),
        "static_allowed": static_allowed,
        "dynamic_only": dynamic_only,
        "missing_everywhere": missing_everywhere,
    }


def emit_probe(wl: dict, out: Path) -> None:
    names = sorted(wl["static_allowed"].keys())
    lines = [
        "/* 自动生成：tools/gen-compat-matrix.py — 勿手工编辑",
        " * 用途：CI 用每个早期 Windows 10 SDK 的头文件编译本探针，",
        " * 确认 pty-win10-early 静态白名单中的每个 API 在该 SDK 中都可声明+链接。",
        " * 编译示例：",
        " *   cl /nologo /c probe.c /I<SDK>\\\\um /I<SDK>\\\\shared /I<SDK>\\\\ucrt /Fo probe.obj",
        " */",
        "#define WIN32_LEAN_AND_MEAN",
        "#include <windows.h>",
        "",
        "/* 逐一引用白名单符号，取地址以强制声明与导入解析 */",
        "static const void *volatile probe_refs[] = {",
    ]
    for n in names:
        lines.append(f"    (const void *)&{n},")
    lines += [
        "};",
        "",
        "int probe_main(void)",
        "{",
        "    return (int)(sizeof(probe_refs) / sizeof(probe_refs[0]));",
        "}",
        "",
    ]
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8", newline="\r\n") as f:
        f.write("\n".join(lines))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--headers", default="sdk-headers")
    ap.add_argument("--doc", default="docs/sdk-compat-matrix.md")
    ap.add_argument("--matrix-json", default="config/sdk-matrix.json")
    ap.add_argument("--whitelist", default="config/api-whitelist.json")
    ap.add_argument("--probe", default="tools/sdk-probe/probe.c")
    args = ap.parse_args()

    root = Path(args.headers)
    matrix: dict[str, dict] = {}
    for v in VERSIONS:
        vdir = root / v
        if not vdir.is_dir():
            print(f"[matrix] missing headers for {v}: {vdir}", file=sys.stderr)
            return 1
        matrix[v] = scan_version(vdir)
        print(f"[matrix] {v}: {len(matrix[v]['functions'])} functions scanned "
              f"({matrix[v]['header_count']} headers)")

    # ---- 控制台 API 表（声明文件属于 console 集的全部函数，按名字排序）----
    console_names = sorted({
        n for v in VERSIONS
        for n, info in matrix[v]["functions"].items()
        if Path(info["file"]).name in CONSOLE_FILES
    })

    # ---- kernel 表：仅列 CURATED 中出现过的 ----
    kernel_names = sorted({
        n for names in CURATED_KERNEL.values() for n in names
        if any(n in matrix[v]["functions"] for v in VERSIONS)
    })

    curated_all = {n for names in CURATED_KERNEL.values() for n in names}
    conpty_present = [n for n in CONPTY_FUNCS
                      if n in matrix["10.0.17763"]["functions"]]

    # ---- 白名单 / 探针 / JSON ----
    wl = build_whitelist(matrix)
    whitelist_path = Path(args.whitelist)
    whitelist_path.parent.mkdir(parents=True, exist_ok=True)
    # 固定 LF：write_text 在 Windows 会按 os.linesep 写 CRLF，Linux CI 再生成即 diff
    with whitelist_path.open("w", encoding="utf-8", newline="\n") as f:
        json.dump(wl, f, indent=2, ensure_ascii=False)
        f.write("\n")
    if wl["missing_everywhere"]:
        print("[matrix] ERROR curated API missing from some SDK:", file=sys.stderr)
        for m in wl["missing_everywhere"]:
            print(f"  - {m}", file=sys.stderr)
        return 1
    emit_probe(wl, Path(args.probe))
    print(f"[matrix] whitelist: {len(wl['static_allowed'])} static, "
          f"{len(wl['dynamic_only'])} dynamic-only -> {args.whitelist}")

    matrix_path = Path(args.matrix_json)
    matrix_path.parent.mkdir(parents=True, exist_ok=True)
    with matrix_path.open("w", encoding="utf-8", newline="\n") as f:
        json.dump(matrix, f, indent=1, ensure_ascii=False)
        f.write("\n")

    # ---- 文档 ----
    doc_path = Path(args.doc)
    doc_path.parent.mkdir(parents=True, exist_ok=True)
    doc = doc_path.read_text(encoding="utf-8") if doc_path.exists() else (
        "# Windows 10 SDK 兼容性矩阵\n\n"
        "本文件由 `tools/gen-compat-matrix.py` 从 `sdk-headers/`（各版本官方头文件"
        "include 闭包）扫描生成；标记外的手工章节随实现持续补充。\n\n"
        "复现：`python tools/gen-compat-matrix.py`\n"
    )

    doc = replace_region(doc, "console-api-matrix", table(
        matrix, console_names,
        title_note="声明来源：`um/consoleapi.h`、`um/consoleapi2.h`、`um/consoleapi3.h`、"
                   "`um/wincon.h`（函数按来源文件归属，跨版本取并集）。"))
    doc = replace_region(doc, "kernel-api-matrix", table(
        matrix, kernel_names,
        title_note="覆盖 pty-win10-early 宿主（进程创建、注入、管道、加载器）与 pty-conpty "
                   "所需的 kernel32 表面；`CURATED_KERNEL` 定义见 "
                   "`tools/gen-compat-matrix.py`。"))
    doc = replace_region(doc, "markers", marker_table(matrix))

    # 守卫证据（受限数量）：只列带非空守卫的控制台函数
    guarded = []
    for v in VERSIONS:
        for n in console_names:
            info = matrix[v]["functions"].get(n)
            if info and info["guards"]:
                guarded.append(f"- `{n}` @ {VERSION_LABEL[v]} `{info['file']}:{info['line']}` "
                               f"guard: `{' / '.join(info['guards'])}`")
    doc = replace_region(doc, "guards", sorted(set(guarded))[:200])

    if conpty_present:
        doc = replace_region(doc, "conpty", [
            "| ConPTY API | 1507–1803 | 1809 (17763) |",
            "|---|---|---|",
        ] + [f"| `{n}` | **✗ 不声明，不可链接** | ✓ `{matrix['10.0.17763']['functions'][n]['file']}`"
             f":{matrix['10.0.17763']['functions'][n]['line']} |" for n in CONPTY_FUNCS])

    with doc_path.open("w", encoding="utf-8", newline="\n") as f:
        f.write(doc)
    print(f"[matrix] wrote {doc_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
