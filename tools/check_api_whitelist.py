#!/usr/bin/env python3
"""API 白名单 linter（阶段 0 强制检查，本机 + CI 均运行）。

规则（对应规格"前置准备 4"）：
  R1  crates/pty-win10-early、crates/pty-conpty 的 extern "system"/"C" 块中，
      每个函数声明必须在 config/api-whitelist.json 的 static_allowed 中。
  R2  dynamic_only 中的符号（ConPTY 三函数）严禁出现在 extern 块或直接调用中，
      只允许以 GetProcAddress 字符串/动态解析形式出现。
  R3  目标 crate 中 PascalCase 调用形态 `Foo(` 的符号必须在 static_allowed 中
      （本地辅助函数请用 snake_case；确有例外加进 allow_local）。
  R4  tools/sdk-probe/probe.c 只能引用 static_allowed，且必须全覆盖（由生成器保证，
      此处做一致性校验）。

退出码：0 = 通过；1 = 存在违规。

用法:
  python tools/check_api_whitelist.py            # 检查
  python tools/check_api_whitelist.py --emit-md  # 重新生成 docs/api-whitelist.md
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

TARGET_CRATES = ["pty-win10-early", "pty-conpty"]
SCAN_SUFFIXES = {".rs", ".c", ".h", ".cpp", ".hpp"}
# PascalCase 调用但非 Win32 API 的本地例外（需要时追加，保持最小）
ALLOW_LOCAL: set[str] = {"Some", "Ok", "Err"}

EXTERN_RE = re.compile(
    r'extern\s*"(?:system|C)"\s*\{', re.S)
FN_RE = re.compile(r'\bfn\s+([A-Za-z_]\w*)\s*\(')
PASCAL_CALL_RE = re.compile(r'\b([A-Z][A-Za-z0-9]+)\s*\(')
STRING_RE = re.compile(r'"([^"\\]|\\.)*"')
LINE_RE = re.compile(r'^[^\n:]*?:\d+:\s*(.*)$', re.S)


class Finding:
    def __init__(self, path: Path, line: int, rule: str, symbol: str, msg: str):
        self.path, self.line, self.rule, self.symbol, self.msg = path, line, rule, symbol, msg

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: [{self.rule}] {self.symbol} — {self.msg}"


def load_whitelist(path: Path) -> dict:
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def iter_scan_files(repo: Path) -> list[Path]:
    out: list[Path] = []
    crates = repo / "crates"
    for crate in TARGET_CRATES:
        d = crates / crate
        if not d.is_dir():
            continue
        for p in sorted(d.rglob("*")):
            if p.suffix.lower() in SCAN_SUFFIXES and p.is_file():
                out.append(p)
    return out


def check_file(path: Path, wl: dict, findings: list[Finding]) -> None:
    text = path.read_text(encoding="utf-8", errors="replace")
    static: dict = wl["static_allowed"]
    dynamic: dict = wl["dynamic_only"]

    # 逐行建索引：行号工具
    def lineno(pos: int) -> int:
        return text.count("\n", 0, pos) + 1

    # --- R1/R2: extern 块 ---
    for m in EXTERN_RE.finditer(text):
        # 找到匹配的结束大括号（简单深度计数）
        i = m.end()
        depth = 1
        while i < len(text) and depth:
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
            i += 1
        block = text[m.start():i]
        for fm in FN_RE.finditer(block):
            pos = m.start() + fm.start()
            name = fm.group(1)
            if name in dynamic:
                findings.append(Finding(path, lineno(pos), "R2", name,
                                        "dynamic_only 符号禁止静态声明，必须 LoadLibrary+GetProcAddress"))
            elif name not in static:
                findings.append(Finding(path, lineno(pos), "R1", name,
                                        "不在 static_allowed 白名单中（若确需使用：先加入 "
                                        "tools/gen-compat-matrix.py 的 CURATED_KERNEL 并重新生成）"))

    # 去掉字符串字面量与注释后再做调用形态扫描
    stripped = strip_noise(text)

    # --- R2: dynamic_only 直接调用 ---
    for name in dynamic:
        for m in re.finditer(r'(?<!["\w])' + re.escape(name) + r'\s*\(', stripped):
            findings.append(Finding(path, lineno(m.start()), "R2", name,
                                    "dynamic_only 符号出现直接调用形态"))

    # --- R3: PascalCase 调用 ---
    for m in PASCAL_CALL_RE.finditer(stripped):
        name = m.group(1)
        if name in dynamic:
            continue  # R2 已覆盖
        if name in static or name in ALLOW_LOCAL:
            continue
        # 排除常见非 API 的 PascalCase：测试宏/类型构造等由 ALLOW_LOCAL 兜底
        findings.append(Finding(path, lineno(m.start()), "R3", name,
                                "PascalCase 调用不在 static_allowed 白名单中"))


def strip_noise(text: str) -> str:
    """移除字符串字面量与行/块注释，保留换行以便行号计算。"""
    def blank(m: re.Match) -> str:
        return re.sub(r"[^\n]", " ", m.group(0))
    text = re.sub(r'"(?:[^"\\\n]|\\.)*"', blank, text)
    text = re.sub(r"'(?:[^'\\\n]|\\.)*'", blank, text)
    text = re.sub(r"/\*.*?\*/", blank, text, flags=re.S)
    text = re.sub(r"//[^\n]*", blank, text)
    text = re.sub(r"#[^\n]*", blank, text)  # Rust 属性行
    return text


def check_probe(repo: Path, wl: dict, findings: list[Finding]) -> None:
    probe = repo / "tools" / "sdk-probe" / "probe.c"
    if not probe.is_file():
        findings.append(Finding(probe, 0, "R4", "probe.c",
                                "探针缺失（运行 python tools/gen-compat-matrix.py 生成）"))
        return
    text = probe.read_text(encoding="utf-8", errors="replace")
    refs = set(re.findall(r"\(const void\s*\*\)\s*&([A-Za-z_]\w*)", text))
    expected = set(wl["static_allowed"].keys())
    for missing in sorted(expected - refs):
        findings.append(Finding(probe, 0, "R4", missing,
                                "白名单符号未被探针引用（探针需重新生成）"))
    for extra in sorted(refs - expected):
        findings.append(Finding(probe, 0, "R4", extra,
                                "探针引用了非白名单符号"))


def emit_md(wl: dict, out: Path) -> None:
    static_by_group: dict[str, list[str]] = {}
    for name, info in sorted(wl["static_allowed"].items()):
        static_by_group.setdefault(info["group"], []).append(name)

    lines = [
        "# pty-win10-early / pty-conpty API 白名单",
        "",
        "- 最近更新：2026-10-03（阶段 0）",
        "- 机读版本：`config/api-whitelist.json`（本文件由 "
        "`python tools/check_api_whitelist.py --emit-md` 从 JSON 生成）",
        "- 证据：`docs/sdk-compat-matrix.md` + `config/sdk-matrix.json`",
        "",
        "## 规则",
        "",
        "1. **static_allowed**：以下 API 在全部 7 个 Windows 10 SDK（10240…17763）头文件中",
        "   均有声明，允许 `pty-win10-early` 与 `pty-conpty` 静态链接引用。",
        "2. **dynamic_only**：只能 `LoadLibraryW + GetProcAddress` 运行时解析；出现静态声明或",
        "   直接调用即 CI 失败。",
        "3. 其他任何未列入 static_allowed 的 Win32 函数符号一律禁止。需要新增时：",
        "   编辑 `tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` → 重新运行生成器 →",
        "   确认 7 版矩阵全绿 → 提交（生成器会拒绝任何一版缺失的 API）。",
        "4. 强制执行：`python tools/check_api_whitelist.py`（本机与 CI 均运行）。",
        "",
        "## static_allowed（静态可链接）",
        "",
    ]
    for group, names in sorted(static_by_group.items()):
        lines.append(f"### {group}")
        lines.append("")
        for n in names:
            info = wl["static_allowed"][n]
            lines.append(f"- `{n}` — `{info['declared_in']}`（min SDK {info['min_sdk']}）")
        lines.append("")
    lines += ["## dynamic_only（必须动态加载）", ""]
    for name, info in sorted(wl["dynamic_only"].items()):
        lines.append(f"- `{name}` — min SDK {info['min_sdk']}；{info['reason']}")
    lines += [
        "",
        "## 拒绝清单示例（违规会被 CI 拦下）",
        "",
        "```text",
        "crates/pty-win10-early/src/host.rs:120: [R1] SetCurrentDirectoryW — 不在 static_allowed 白名单中",
        "crates/pty-conpty/src/sys.rs:44: [R2] CreatePseudoConsole — dynamic_only 符号禁止静态声明",
        "```",
        "",
    ]
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines))
    print(f"[whitelist] wrote {out}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo", default=str(Path(__file__).resolve().parent.parent))
    ap.add_argument("--whitelist", default=None)
    ap.add_argument("--emit-md", action="store_true")
    ap.add_argument("--md", default=None)
    args = ap.parse_args()

    repo = Path(args.repo)
    wl_path = Path(args.whitelist) if args.whitelist else repo / "config" / "api-whitelist.json"
    if not wl_path.is_file():
        print(f"[whitelist] ERROR missing {wl_path}（先运行 tools/gen-compat-matrix.py）",
              file=sys.stderr)
        return 1
    wl = load_whitelist(wl_path)

    if args.emit_md:
        emit_md(wl, Path(args.md) if args.md else repo / "docs" / "api-whitelist.md")
        return 0

    findings: list[Finding] = []
    files = iter_scan_files(repo)
    for p in files:
        check_file(p, wl, findings)
    check_probe(repo, wl, findings)

    if findings:
        print(f"[whitelist] {len(findings)} violation(s):")
        for f in findings:
            print(f"  {f}")
        return 1
    print(f"[whitelist] OK — {len(files)} file(s) checked, "
          f"{len(wl['static_allowed'])} static, {len(wl['dynamic_only'])} dynamic-only")
    return 0


if __name__ == "__main__":
    sys.exit(main())
