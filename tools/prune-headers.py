#!/usr/bin/env python3
"""阶段 0 工具：include 闭包裁剪。

从种子头文件出发，递归解析 #include，把每个 SDK 版本真正需要的头文件
从 D:\\temp\\win-headers 裁剪到仓库 sdk-headers/<version>/，
保持 um/shared/ucrt/km 相对路径，供 CI 的 cl 编译矩阵离线使用。

种子 = 规格点名的关键头文件（consoleapi*.h、wincon.h、processthreadsapi.h、
handleapi.h、fileapi.h、winbase.h、winver.h、sdkddkver.h、windows.h）。

用法:
  python tools/prune-headers.py --src D:/temp/win-headers --dest sdk-headers
"""
from __future__ import annotations

import argparse
import re
import shutil
import sys
from pathlib import Path

VERSIONS = [
    "10.0.10240", "10.0.10586", "10.0.14393", "10.0.15063",
    "10.0.16299", "10.0.17134", "10.0.17763",
]

# 搜索顺序与 CI 中 cl /I 顺序保持一致
SEARCH_DIRS = ["um", "shared", "ucrt", "km"]

SEEDS = [
    "windows.h",
    "winver.h",
    "sdkddkver.h",
    "wincon.h",
    "consoleapi.h",
    "consoleapi2.h",
    "consoleapi3.h",
    "processthreadsapi.h",
    "handleapi.h",
    "fileapi.h",
    "winbase.h",
]

INCLUDE_RE = re.compile(r'^\s*#\s*include\s+([<"])([^>"]+)[>"]', re.MULTILINE)


def find_header(vdir: Path, name: str, cur_dir: str | None) -> Path | None:
    """按 MSVC include 语义解析：引号包含先查当前目录，再按 um/shared/ucrt/km。"""
    name = name.replace("\\", "/")
    candidates = []
    if cur_dir is not None:
        candidates.append(vdir / cur_dir / name)
    # 裸文件名（如 "foo.h" 形式跨目录引用）也允许相对种子根
    candidates.append(vdir / name)
    for d in SEARCH_DIRS:
        candidates.append(vdir / d / name)
    # 引号包含时也允许相对引用文件所在目录
    for c in candidates:
        if c.is_file():
            return c
    return None


def resolve_root_rel(vdir: Path, path: Path) -> str:
    return path.relative_to(vdir).as_posix()


def closure(vdir: Path) -> tuple[list[str], dict[str, list[str]], dict[str, list[str]]]:
    """返回 (sorted include 列表, missing=引用->未找到的 include, stats)"""
    found: dict[str, Path] = {}
    missing: dict[str, list[str]] = {}
    queue: list[tuple[Path, str]] = []

    for seed in SEEDS:
        p = find_header(vdir, seed, None)
        if p is None:
            missing.setdefault("<seed>", []).append(seed)
            continue
        rel = resolve_root_rel(vdir, p)
        if rel not in found:
            found[rel] = p
            queue.append((p, rel))

    while queue:
        cur, cur_rel = queue.pop()
        try:
            text = cur.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            missing.setdefault(cur_rel, []).append(f"<unreadable: {e}>")
            continue
        cur_dir = str(Path(cur_rel).parent).replace("\\", "/")
        if cur_dir == ".":
            cur_dir = ""
        for m in INCLUDE_RE.finditer(text):
            target = m.group(2)
            p = find_header(vdir, target, cur_dir)
            if p is None:
                missing.setdefault(cur_rel, []).append(target)
                continue
            rel = resolve_root_rel(vdir, p)
            if rel not in found:
                found[rel] = p
                queue.append((p, rel))

    return sorted(found), missing, {}


def prune_version(src: Path, dest_root: Path, version: str) -> int:
    vdir = src / version
    if not vdir.is_dir():
        print(f"[prune] ERROR missing source tree: {vdir}", file=sys.stderr)
        return 1
    out = dest_root / version
    if out.exists():
        shutil.rmtree(out)
    files, missing, _ = closure(vdir)
    for rel in files:
        s = vdir / rel
        d = out / rel
        d.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(s, d)
    # 记录未解析的 include（条件编译保护，正常编译不会走到）
    if missing:
        notes = out / "_prune_report.txt"
        with notes.open("w", encoding="utf-8") as f:
            f.write(f"# unresolved #include while pruning {version}\n")
            f.write("# (guarded by preprocessor conditionals; compile probe must not reach them)\n")
            for k, vs in sorted(missing.items()):
                for v in sorted(set(vs)):
                    f.write(f"{k}: {v}\n")
    print(f"[prune] {version}: {len(files)} headers -> {out} "
          f"({len({t for v in missing.values() for t in v})} unresolved refs)")
    return 0


def verify_seeds(dest_root: Path) -> int:
    """确认规格点名的关键头文件全部入库（缺失的 consoleapi2/3.h 仅限 <1803）。"""
    rc = 0
    for v in VERSIONS:
        for seed in SEEDS:
            p = dest_root / v / "um" / seed
            if p.is_file():
                continue
            p2 = dest_root / v / "shared" / seed
            if p2.is_file():
                continue
            # consoleapi2.h / consoleapi3.h 在 1803 之前不存在 —— 预期内
            if seed in ("consoleapi2.h", "consoleapi3.h") and v in (
                "10.0.10240", "10.0.10586", "10.0.14393", "10.0.15063", "10.0.16299"
            ):
                print(f"[prune] note: {v} has no {seed} (expected: introduced in 1803)")
                continue
            print(f"[prune] ERROR {v}: seed {seed} not found after prune", file=sys.stderr)
            rc = 1
    return rc


def main() -> int:
    # 默认 <仓库盘符>:\wbwtty-temp\win-headers（按脚本位置探测，路径按盘符自适应）
    _default_src = Path(__file__).resolve().parents[1]
    _default_src = Path(_default_src.anchor) / 'wbwtty-temp' / 'win-headers'
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--src", default=str(_default_src))
    ap.add_argument("--dest", default="sdk-headers")
    ap.add_argument("--versions", nargs="*", default=VERSIONS)
    args = ap.parse_args()

    src = Path(args.src)
    dest = Path(args.dest)
    if not src.is_dir():
        print(f"[prune] source not found: {src} (run tools/fetch-win-headers.ps1 first)",
              file=sys.stderr)
        return 1
    dest.mkdir(parents=True, exist_ok=True)

    rc = 0
    for v in args.versions:
        rc |= prune_version(src, dest, v)
    rc |= verify_seeds(dest)
    return rc


if __name__ == "__main__":
    sys.exit(main())
