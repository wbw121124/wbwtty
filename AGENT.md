# AGENT.md — 项目状态与执行记录

- 最近更新：2026-10-05

## 当前 Git 状态

- 当前分支：`feature/term-render-gtk`（阶段 2 收尾；main 停在 `feature/term-input` 合并提交）
- 最近提交：阶段 2 系列（`feat:` 渲染器 / `test:` 管线测试 / `docs:` 模块 README /
  `ci:` GTK 平台矩阵 / `docs:` 阶段收尾，精确哈希见 `git log`）
- 已打标签：`v0.1.0-stage0`、`v0.2.0-stage1`、`v0.3.0-stage2`（合回 main 的合并提交）
- 待合并分支：`feature/term-render-gtk`

## 当前阶段

**阶段 2 完成**：MSYS2 → `D:\msys` ✅、term-input ✅（17 测试）、
term-render-gtk ✅（39 测试）→ 标签 `v0.3.0-stage2`。**下一步：阶段 3 pty-conpty**。

## 模块划分（计划）

| 模块 | 职责 | 状态 |
|---|---|---|
| vt-parser | VT 序列解析 + 终端状态（网格/颜色/光标/滚动缓冲/damage） | ✅ 阶段 1（58 单元 + 13 集成测试） |
| pty-core | PTY 统一抽象（trait + C ABI），无平台实现 | ✅ 阶段 1（注册/选择/回退 + FFI，9 测试） |
| pty-unix | openpty/forkpty 后端（Linux/macOS） | ✅ 阶段 1（POSIX 实现；Windows 空壳；unix 测试由 CI 执行） |
| pty-conpty | ConPTY 后端（Win10 1809+，全部动态加载） | 未开始（阶段 3） |
| pty-win10-early | 1809 前桥接：控制台 API 直驱 / Cygwin PTY / WinPTY 回退 | 未开始（阶段 4） |
| term-render-gtk | GTK3+Cairo/OpenGL 渲染 | ✅ 阶段 2（31 单元 + 8 集成测试；本机链接/窗口跑通） |
| term-render-qt | Qt 渲染（可选，backlog） | backlog |
| term-input | 键鼠事件 → VT 序列编码（纯函数） | ✅ 阶段 2（11 单元 + 6 集成测试） |
| term-app | 组装示例主程序 | 未开始（阶段 2 起） |

## 已完成任务（阶段 0）

1. 需求确认（SDK 获取方式、CI 平台、GTK 方案、执行范围、Git 身份）
2. `git init`（main 分支）+ 仓库 local 身份 + `.gitignore` + `plan.md`/`AGENT.md`
3. 头文件获取：`tools/fetch-win-headers.ps1` 稀疏克隆 7 版官方头文件归档 → `D:\temp\win-headers`
4. 闭包裁剪入库：`tools/prune-headers.py` → `sdk-headers/`（每版约 177 个头文件；1507–1709 无
   consoleapi2/3.h 属预期）
5. 官方安装器交叉校验（`docs/header-verification.md`）：
   - 10240 静默安装成功：关键头文件 SHA-256 全一致、共有 147 文件 0 差异、快照 API 为官方严格子集
   - 17763 引导器非提权失败（0x3e9）→ 官方 ISO + `msiexec /a` 管理解压（取舍已记录）：
     153 共有文件 129 一致 / 24 servicing 级别差异，快照为官方超集（含 GetTempPath2）
   - 62 个白名单符号在两版官方头文件中全部可声明；ConPTY 符号 10240 无 / 17763 有
6. 兼容矩阵生成：`tools/gen-compat-matrix.py` → `docs/sdk-compat-matrix.md`（控制台 API 表、
   kernel API 表、标记表、守卫证据、ConPTY 结论、后端最低 SDK）
7. API 白名单：`config/api-whitelist.json`（62 static + 3 dynamic-only）+ 人读版
   `docs/api-whitelist.md` + linter `tools/check_api_whitelist.py`（R1–R4 规则）
8. CI：`.github/workflows/ci.yml`（docs-check / api-whitelist / build-test 三平台 /
   pipe-mvp / sdk-compile-matrix 7 版 cl 编译）+ `scripts/ci-local.ps1`
9. 顶层 `README.md`（模块组合方式）、`docs/architecture.md`（分层/数据流/后端选择）

## 已完成任务（阶段 1）

1. Cargo workspace：成员 `crates/*`、根包 `wbwtty` + `[[example]] pipe_mvp`、`Cargo.lock` 入库
2. `vt-parser`：增量 UTF-8、ESC/CSI/OSC/DCS（`;` 参数 + `:` 子参数）、SGR 真彩色（两形式）、
   光标/擦除/滚动区/alt-screen/宽字符/组合符/行级 damage/回滚、DA/DSR 应答、模式跟踪
   （1000/1002/1003/1006、2004、1004、DECCKM、DECAWM、DECOM…）；58 单元 + 13 集成测试
3. `pty-core`：`Pty`/`Backend` trait、`SpawnOptions`/`Signal`/`SpawnError`、注册表
   （同 kind 替换、priority 升序选择、`BackendUnavailable` 回退、真实错误终止回退）、
   C ABI `include/pty_core.h`（spawn/read/write/resize/signal/close/child_pid/last_error，
   线程局部错误）；9 测试（含伪后端 FFI 往返）
4. `pty-unix`：`posix_openpt`/`grantpt`/`unlockpt` + fork/`setsid`/`TIOCSCTTY`/`dup2` +
   `execvp`、`TIOCSWINSZ` resize、`kill(-pid)` 信号、EIO→EOF、`waitpid(WNOHANG)` 收割、
   幂等 close（SIGHUP）、Drop 防僵尸；Windows 下空壳可编译；unix 集成测试 5 项（CI 执行）
5. `examples/pipe_mvp.rs`：python → 匿名管道 → vt-parser → 断言（颜色/光标/宽字符/真彩色/damage）
6. ci-local 扩展：步骤 4b linux 目标交叉检查（`cargo check -p pty-unix --target
   x86_64-unknown-linux-gnu --all-targets`）+ 自动运行 pipe_mvp；`x86_64-apple-darwin` 亦 check 通过
7. 模块 README（vt-parser/pty-core/pty-unix）+ 本机 ci-local 全绿（80 测试）

## 已完成任务（阶段 2）

1. ~~MSYS2 安装到 `D:\msys`~~ ✅（ucrt64；见下方"MSYS2 环境"）
2. ~~`term-input`：键/鼠/滚轮/paste → VT 序列~~ ✅（11 单元 + 6 集成测试）
3. `term-render-gtk`：GTK3+Cairo 渲染器（`crates/term-render-gtk`）
   - 分层：`batch`（同色同属性分段 + 行尾纯空白裁剪）、`color`（xterm 16/256/真彩色）、
     `metrics`（Pango 度量 → 网格换算/hit_test）、`cache`（Pango Layout 键控缓存）、
     `viewport`（回滚缓冲 + 实屏 → 一屏快照）、`pangocairo`（自声明 FFI）、
     `renderer`（背景/文本/下划线/删除线/块光标+竖条光标）、`widget`（damage 重绘、
     resize 回调、滚轮回滚、`scroll_by`/`scroll_to_bottom`）
   - **gtk-rs 0.18 适配**：`Propagation`（`Inhibit` 已移除）、`Context::metrics`、
     `FontMetrics::approximate_char_width/ascent/descent`、`set_font_description(Some(..))`、
     `FontDescription::size()`、`Allocation::width()/height()`、`ControlFlow::Continue`
   - **pangocairo**：gtk-rs 0.18 未绑定 `pango_cairo_*` → `src/pangocairo.rs` 自声明
     `pango_cairo_show_layout` + `build.rs` pkg-config 探测 `pangocairo`（走 Pango 塑形，
     不用 Cairo 玩具字体 API）
   - 示例：`gtk_smoke`（链接/运行时冒烟，**本机通过**）、`demo_window`（动画窗口，本机跑通）
   - 31 单元 + 8 集成测试（`tests/pipeline.rs`）
4. 本机 `cargo build/test --workspace` 全绿（136 测试）+ `scripts/ci-local.ps1` **PASSED**
5. CI：`build-test` 非 Linux 平台 `--exclude term-render-gtk`（GTK 权威验证在 Ubuntu）、
   Linux 补装 `pkg-config`；docs-check 已覆盖新增 README
6. 模块 README（`crates/term-render-gtk/README.md`）、`scripts/env.ps1`（本机会话环境）

## 下一步（阶段 3：pty-conpty）

1. ConPTY 三函数（`CreatePseudoConsole`/`ResizePseudoConsole`/`ClosePseudoConsole`）+ 
   `STARTUPINFOEXW`/`PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` **全部运行时动态加载**
2. 加载失败返回 `BackendUnavailable` → 上层回退 `pty-win10-early`（阶段 4 才实现，先留占位）
3. 本机 17763 集成测试（spawn cmd / resize / Ctrl+C）
4. 提交 `feat: implement ConPTY backend with dynamic loading`，标签 `v0.4.0-stage3`

## 本机持久化环境（C: 重启被清空，2026-10-05）

- **C: 重启会还原**（依赖缓存、新增 rustup target、装的软件都会没）→ 一切持久的东西放 D:
- Rust：`RUSTUP_HOME=D:\rustup`、`CARGO_HOME=D:\cargo`（由 `C:\Users\Administrator\.rustup`
  `.cargo` 整体复制而来，1.57GB + 0.17GB）；已装 target：windows-gnu / linux-gnu / darwin
- 每次开新终端：`. .\scripts\env.ps1`（或依赖已写入的用户环境变量；被 C: 还原冲掉就用脚本）
- `scripts/ci-local.ps1` 内置同样的 D: 环境兜底（CI runner 无 D:\rustup 时自动跳过）
- 代理：crates.io 直连可用；需要时 `scripts/env.ps1` 里取消 `127.0.0.1:7890` 注释
- 范围约定：只在 `F:\wbwtty` 内改文件，下载/解压 `D:\temp`，软件装 `D:\`，不扫 F:\ G:\ 全盘

## MSYS2 环境（阶段 2 前置，2026-10-03 完成）

- 安装：`D:\msys`（非管理员静默 `in --root D:\msys`）；安装器
  `msys2-x86_64-latest.exe`（GitHub nightly-x86_64 资产，85047704 B，
  SHA-256 `7CCD43DE6EBA7ADB6686E50B5D053C848EF14D53BC66FB84C6C70EF2DF6B5B7E`）→ `D:\temp\msys2-installer.exe`
- **包命名已重构**：`mingw-w64-ucrt64-*` → `mingw-w64-ucrt-x86_64-*`（旧名报"未找到目标"）
- 已装：`mingw-w64-ucrt-x86_64-toolchain`（gcc 16.2.0）+ `-gtk3`（3.24.52，85 包/1.26GB）+ `-pkgconf`
- 验证：`D:\msys\ucrt64\bin\gcc.exe --version`、`pkg-config --modversion gtk+-3.0` = 3.24.52
- 用法：构建时 PATH 前置 `D:\msys\ucrt64\bin`（gcc/pkg-config/GTK DLL），代理 env 供 pacman

## 已知问题 / 阻塞

- 本机无 MSVC/Windows SDK → per-SDK 编译由 CI（windows-latest + ilammy/msvc-dev-cmd）承担；
  本机 `scripts/ci-local.ps1` 在 cl 不在 PATH 时自动跳过该步（已决策）
- 本机无 gh CLI 且未认证 → 创建 GitHub 远端仓库需要一次用户认证（device flow 或 GH_TOKEN）
- ~~MSYS2（D:\msys）尚未安装~~ ✅ 已完成（2026-10-03，见"MSYS2 环境"）
- ~~Rust(gnu) ↔ MSYS2 ucrt64 GTK 链接未验证~~ ✅ 已实测通过（`gtk_smoke`、`demo_window` 本机跑通）
- CI 的 windows/macos runner 无 GTK 开发包 → `build-test` 用 `--exclude term-render-gtk`，
  GTK 渲染的权威验证在 Ubuntu + 本机 MSYS2
- C: 重启被清空 → 工具链/缓存已迁 `D:\rustup`、`D:\cargo`，新终端跑 `. .\scripts\env.ps1`
  （用户环境变量若被还原冲掉，以脚本为准）
- `pty-unix` 的 unix 集成测试本机（Windows）不可执行 → 由 CI ubuntu/macos 运行；
  本机以 linux 目标 `cargo check --all-targets` 把编译关（ci-local 步骤 4b）

## SDK 兼容性结论（摘要，详见 docs/sdk-compat-matrix.md）

- `CreatePseudoConsole`/`ResizePseudoConsole`/`ClosePseudoConsole`（+`HPCON`、
  `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`、`PSEUDOCONSOLE_INHERIT_CURSOR`）**仅 10.0.17763 存在**，
  必须动态加载，1809 前加载失败回退 pty-win10-early
- `consoleapi2.h`/`consoleapi3.h` 自 1803（17134）才存在
- `ENABLE_VIRTUAL_TERMINAL_PROCESSING` 1511 起有、**1507 无**；`ENABLE_VIRTUAL_TERMINAL_INPUT`
  1607 起；`ENABLE_LVB_GRID_WORLDWIDE` 1607 起
- `STARTUPINFOEXW`、`GetConsoleHistoryInfo`、`GenerateConsoleCtrlEvent`、全部规格点名拦截函数
  在 7 版中全绿（1507 即可用）
- pty-win10-early 编译基线 = 10.0.10240；pty-conpty 运行时要求 1809+

## API 白名单摘要（详见 docs/api-whitelist.md）

- static_allowed：62 个（process / thread-injection / loader / file-pipe / sync-wait /
  console-host / string / error 分组），7 版 SDK 全部声明，允许静态链接
- dynamic_only：ConPTY 三函数，必须 `LoadLibrary+GetProcAddress`
- 其余任何 Win32 函数符号禁止在 pty-win10-early/pty-conpty 静态引用；
  新增需改 `tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` 重新生成（生成器拒绝任何一版缺失）
- 强制检查：`python tools/check_api_whitelist.py`（本机 + CI）

## 约定

- 提交：Conventional Commits，单一逻辑变更；提交前相关模块构建+测试通过
- 分支：main 稳定；`feature/<模块>`、`fix/<简述>`，CI 通过后合回
- 标签：`v0.x.0-stageN`，仅打在 main；文档随代码同步更新
- 下载与解压一律 `D:\temp`（不写 C:、不写 F:）；软件装 `D:\`；只在 `F:\wbwtty` 内改文件
