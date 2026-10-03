#!/usr/bin/env python3
"""阶段 0 工具：官方安装器头文件 vs 快照归档 交叉校验。

对比范围限定在 um/shared/ucrt/km（SDK 用户态/内核头文件；winrt 等与本项目无关）：
  1. 规格点名的关键头文件存在性 + 逐文件 SHA-256
  2. .h 文件集合差异
  3. 逐文件 SHA-256 一致/不一致计数
  4. API 声明集合差异（复用 gen-compat-matrix 的扫描器）
  5. config/api-whitelist.json 的 static_allowed 是否全部可声明于官方头文件

输出 markdown（--out 指定 UTF-8 文件），由调用方汇入 docs/header-verification.md。

用法:
  python tools/compare-sdk-headers.py --official D:/temp/winsdk-10240/Include/10.0.10240.0 \\
      --snapshot sdk-headers/10.0.10240 --label "10.0.10240 (1507)" --out D:/temp/cmp-10240.md
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

_GCM_PATH = Path(__file__).resolve().parent / "gen-compat-matrix.py"
_spec = importlib.util.spec_from_file_location("gen_compat_matrix", _GCM_PATH)
gcm = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gcm)  # noqa: E402

# 对比范围：用户态 SDK 头文件。km（WDK 提供）与 winrt/与项目无关的组件不参与对比。
TOP_DIRS = {"um", "shared"}

KEY_HEADERS = [
    ("um", "windows.h"), ("um", "winver.h"), ("shared", "sdkddkver.h"),
    ("um", "wincon.h"), ("um", "consoleapi.h"), ("um", "consoleapi2.h"),
    ("um", "consoleapi3.h"), ("um", "processthreadsapi.h"),
    ("um", "handleapi.h"), ("um", "fileapi.h"), ("um", "winbase.h"),
]


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def headers(root: Path) -> dict[str, tuple[str, Path]]:
    """返回 lower(rel) -> (原样 rel, path)。Windows SDK 头文件在 NTFS 上大小写不敏感
    （如 um/OAIdl.h vs um/oaidl.h），集合比对按小写规范化，避免误报。"""
    out: dict[str, tuple[str, Path]] = {}
    for p in root.rglob("*.h"):
        rel = p.relative_to(root)
        if rel.parts[0] in TOP_DIRS:
            out[rel.as_posix().lower()] = (rel.as_posix(), p)
    return out


def funcset(root: Path) -> set[str]:
    return set(gcm.scan_version(root, top_dirs=TOP_DIRS)["functions"].keys())


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass
    ap = argparse.ArgumentParser()
    ap.add_argument("--official", required=True)
    ap.add_argument("--snapshot", required=True)
    ap.add_argument("--label", required=True)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    off, snap = Path(args.official), Path(args.snapshot)
    if not off.is_dir():
        print(f"官方头文件目录不存在: {off}", file=sys.stderr)
        return 1
    if not snap.is_dir():
        print(f"快照目录不存在: {snap}", file=sys.stderr)
        return 1

    buf: list[str] = []
    p = buf.append
    p(f"### {args.label}：官方安装器 vs 快照归档\n")
    p(f"- 官方：`{off}`")
    p(f"- 快照：`{snap}`\n")

    # 1. 关键头文件
    p("关键头文件存在性（官方 | 快照）与内容比对：\n")
    p("| 头文件 | 官方 | 快照 | SHA-256 |")
    p("|---|---|---|---|")
    all_key_ok = True
    for sub, name in KEY_HEADERS:
        o, s = off / sub / name, snap / sub / name
        oi, si = o.is_file(), s.is_file()
        if oi and si:
            same = sha256(o) == sha256(s)
            verdict = "一致" if same else "**内容不同**"
            if not same:
                all_key_ok = False
        else:
            verdict = "—"
            if oi != si:
                all_key_ok = False
        note = "（该版本不存在，符合预期）" if not oi and not si else ""
        p(f"| `{sub}/{name}` | {'✓' if oi else '✗'} | {'✓' if si else '✗'} "
          f"| {verdict}{note} |")
    p("")

    # 2. 文件集合与哈希（键为小写规范化路径）
    oh, sh = headers(off), headers(snap)
    only_off = sorted(oh[k][0] for k in set(oh) - set(sh))
    only_snap = sorted(sh[k][0] for k in set(sh) - set(oh))
    common = sorted(set(oh) & set(sh))
    same = diff = 0
    diff_files = []
    for k in common:
        if sha256(oh[k][1]) == sha256(sh[k][1]):
            same += 1
        else:
            diff += 1
            diff_files.append(sh[k][0])

    p(f"文件集合（um/shared，大小写不敏感）：官方 {len(oh)} / 快照 {len(sh)} / "
      f"共有 {len(common)}；共有文件 SHA-256：一致 {same}，不同 {diff}。\n")
    if only_off:
        p(f"- 仅官方有（{len(only_off)}）："
          + ", ".join(f"`{x}`" for x in only_off[:8]) + (" …" if len(only_off) > 8 else ""))
    if only_snap:
        p(f"- 仅快照有（{len(only_snap)}）："
          + ", ".join(f"`{x}`" for x in only_snap[:8]) + (" …" if len(only_snap) > 8 else ""))
    if diff_files:
        p(f"- 内容不同（{len(diff_files)}）："
          + ", ".join(f"`{x}`" for x in diff_files[:12])
          + (" …" if len(diff_files) > 12 else ""))
    p("")

    # 3. API 集合
    fo, fs = funcset(off), funcset(snap)
    add, miss = sorted(fs - fo), sorted(fo - fs)
    p(f"API 声明集合：官方 {len(fo)} / 快照 {len(fs)}；"
      f"快照多 {len(add)}、官方多 {len(miss)}。\n")
    if add:
        p(f"- 快照中多出（官方缺失）：" + ", ".join(f"`{x}`" for x in add[:20])
          + (" …" if len(add) > 20 else ""))
    if miss:
        p(f"- 官方中多出（快照缺失）：" + ", ".join(f"`{x}`" for x in miss[:20])
          + (" …" if len(miss) > 20 else ""))
    p("")

    # 4. 白名单验证
    rc = 0
    wl_path = Path(__file__).resolve().parent.parent / "config" / "api-whitelist.json"
    if wl_path.is_file():
        wl = json.loads(wl_path.read_text(encoding="utf-8"))
        bad = [n for n in wl["static_allowed"] if n not in fo]
        if bad:
            p("**白名单验证失败**：官方头文件缺少以下 static_allowed 符号："
              + ", ".join(f"`{x}`" for x in bad))
            rc = 1
        else:
            p(f"白名单验证：{len(wl['static_allowed'])} 个 static_allowed 符号在官方头文件中"
              "全部可声明 ✓")
        conpty = sorted(n for n in wl["dynamic_only"] if n in fo)
        p("ConPTY 符号在该官方头文件中："
          + (f"{', '.join(f'`{x}`' for x in conpty)}（存在）" if conpty
             else "不存在（1809 之前预期如此）"))
        p("")

    p(f"结论：{'关键头文件全部一致' if all_key_ok else '**存在关键头文件差异（见上表）**'}；"
      f"共有文件差异 {diff} 个，API 面差异 {len(add) + len(miss)} 项"
      "（快照为裁剪闭包、官方为完整 SDK，结构性差异属预期；"
      "项目相关 API 见白名单验证）。")

    text = "\n".join(buf) + "\n"
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        with open(args.out, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        print(f"[compare] wrote {args.out}")
    else:
        print(text, end="")
    return rc


if __name__ == "__main__":
    sys.exit(main())
