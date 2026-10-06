# AGENT.md — 项目状态与执行记录

- 最近更新：2026-10-06（阶段 3 收尾：**CI run #7/#8 三项失败诊断与修复**，分支
  `fix/ci-stage3-green`；另修 env.ps1 项目盘符候选探测 + BOM，本机 17/17×3、ci-local 全绿）
- 上次更新：2026-10-06（**Linux GTK 渲染器本机实测通过**：`cargo test -p term-render-gtk` 39/39、
  `gtk_smoke` OK、`demo_window` 窗口截图核验，见"Linux GTK 实测"）

## 当前 Git 状态

- 当前分支：`fix/ci-stage3-green`（自 main `9d0920a` 拉出；**计划 4 个提交**：`fix:`×2 +
  `ci:` + `docs:`，`--no-ff` 合回 main 后推送）
- main = tag **`v0.4.0-stage3`**（`9d0920a` = Merge `feature/pty-conpty`，`c2829ab` feat +
  `cd02620` docs），均已推送 origin
- 已打标签：`v0.1.0-stage0`、`v0.2.0-stage1`、`v0.3.0-stage2`、`v0.4.0-stage3`
- 工作树未提交（本分支改动）：`.github/workflows/ci.yml`（timeout + msys2 PATH）、
  `crates/pty-conpty/{src/imp.rs,tests/spawn_conpty.rs}`（probe 累积读 + ANSI 剥离匹配）、
  `crates/pty-unix/tests/spawn_unix.rs`（roundtrip 死锁 + watchdog）、
  `scripts/{env.ps1,ci-local.ps1}`（项目盘符候选探测 + BOM）、`tools/prune-headers.py`（文档）、
  `plan.md`/`AGENT.md`（含 2026-10-06 Linux GTK 实测记录）
- 其余分支均未删除（用户要求保留）：`feature/pty-core`、`feature/term-input`、
  `feature/term-render-gtk`、`feature/vt-parser`、`feature/pty-conpty`
- 待办：ci-local 全绿 → 四个提交 → 合 main → 推送 → gh 盯 CI 三项转绿 → 关闭阶段 3

## 当前阶段

**阶段 2 完成**（`v0.3.0-stage2`）。**阶段 3 pty-conpty 已交付**（main `9d0920a` + 标签
`v0.4.0-stage3` 已推送；17/17 测试全绿）；**CI run #7/#8 三项失败已诊断并修复**（本分支），
待 push 后 CI 全绿即关闭阶段 3、进入阶段 4。

## 模块划分（计划）

| 模块 | 职责 | 状态 |
|---|---|---|
| vt-parser | VT 序列解析 + 终端状态（网格/颜色/光标/滚动缓冲/damage） | ✅ 阶段 1（58 单元 + 13 集成测试） |
| pty-core | PTY 统一抽象（trait + C ABI），无平台实现 | ✅ 阶段 1（注册/选择/回退 + FFI，9 测试） |
| pty-unix | openpty/forkpty 后端（Linux/macOS） | ✅ 阶段 1（POSIX 实现；Windows 空壳；unix 测试由 CI 执行） |
| pty-conpty | ConPTY 后端（Win10 1809+，全部动态加载） | ✅ 阶段 3（合 main `9d0920a` + 标签 `v0.4.0-stage3`；17/17 全绿；CI 修复已提交待全绿） |
| pty-win10-early | 1809 前桥接：控制台 API 直驱 / Cygwin PTY / WinPTY 回退 | 未开始（阶段 4） |
| term-render-gtk | GTK3+Cairo/OpenGL 渲染 | ✅ 阶段 2（31 单元 + 8 集成测试；Windows(MSYS2) 与 Linux(Ubuntu 20.04) 双平台实测通过） |
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

## 进行中（阶段 3：pty-conpty）— 交付完成，剩 CI 全绿

### 已完成

1. 白名单：`tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` 新增 group
   `proc-thread-attribute`（`InitializeProcThreadAttributeList`/
   `UpdateProcThreadAttribute`/`DeleteProcThreadAttributeList`）→ 65 static +
   3 dynamic-only，7 版全绿；`ALLOW_LOCAL` 放行 Rust 惯用 `Some`/`Ok`/`Err`
   （提交 `e111b4f`）
2. `crates/pty-conpty`（已入 workspace成员）：
   - `src/sys.rs`：repr(C) 类型/常量/白名单 `extern "system"` 块、`ConptyApi` 从
     kernel32 **与** kernelbase 动态解析（`GetModuleHandleW` 必须收 UTF-16 宽串，
     已修）；单测断言结构体尺寸 Coord=4 / StartUpInfoW=104 / StartUpInfoExW=112 /
     ProcessInformation=24 / Hpc=8（x64）
   - `src/cmdline.rs`：`quote`/`build_command_line`/`build_environment` + 7 单测
   - `src/imp.rs`：`AttributeList`（`std::alloc` 对齐 16 的 RAII）、`spawn_conpty`
     （拆出 `spawn_conpty_typed -> ConptyPty` 供测试取原始句柄）、`ConptyPty`
     （PeekNamedPipe 10ms 轮询读、ETX(0x03) 作 Ctrl+C、句柄存 isize 保证 Send、
     `release()` 统一失败清理）
   - `tests/spawn_conpty.rs`：6 个集成测试，每个带 `watchdog(60)` → 超时 abort
3. 测试：`cargo test -p pty-conpty` **17/17 全绿**（11 单元 + 6 集成，0.88s）

### 阻塞诊断与修复（2026-10-05 新机器定案）

- 现象：4 个集成测试挂死 → 60s watchdog abort；**`hello-conpty` 出现在 harness
  自己的 stdout**（子进程把输出写进了父进程的 std 管道，ConPTY 管道 0 字节）
- 根因：Windows 总是把父进程 std 句柄(0/1/2)传给子进程——`bInheritHandles=FALSE` 与
  显式去 `HANDLE_FLAG_INHERIT` 都拦不住（旧机 exp_prop/exp_inhflag 实验）；测试
  harness/CI/调试器下父 stdout 是管道 → `cmd /c echo` 不走 ConPTY 管道
- **修法（已落地）**：`imp.rs` spawn 处
  `startup.StartupInfo.dwFlags |= sys::STARTF_USESTDHANDLES` +
  `hStdInput/hStdOutput/hStdError = NULL`（`sys.rs` 补常量 `0x0000_0100`）。
  来源：microsoft/terminal#4380 issuecomment-580865346 官方建议；zhiburt/conpty 0.7.0
  同款实现（其注释原话 "avoid issues when debugging or using cargo-nextest"）
- **对照实验调和了旧机矛盾**（旧机"STARTF 后 echo 输出彻底丢失"的 Python 复现不可靠，
  以新实验为准）：
  1. `G:\wbwtty-temp\zctest`（conpty@0.7.0）→ 读到 `ZTEST-OK` **PASS**；
  2. 原失败测试复现 → 输出泄漏到父 stdout、看门狗 abort；
  3. 修法落地后 → 17/17 全绿
- 决策：`probe_output_pipe_receives_child_output` **保留**（原计划删除）：3 秒快败
  回归护栏（PeekNamedPipe 不阻塞），比 60s 挂死更早暴露修法回退
- CI run #6（head `5675262`）：docs-check / api-whitelist / windows build-test /
  7×SDK 矩阵**全绿**；`build-test (msys2-ucrt64-latest)` 在 "Environment sanity
  (GTK3 + rustc)" 步骤**失败**；ubuntu/macos "Test workspace" 挂 2h13m 后被取消
  （run 整体 cancelled）→ **已在 run #7/#8 日志中定案并修复（见下节）**

### CI run #7/#8 失败诊断与修复（2026-10-06，分支 `fix/ci-stage3-green`）

run #7/#8（main `9d0920a` / tag `v0.4.0-stage3` 推送触发）三项失败，gh 拉 job 日志定案：

1. **windows-latest probe 竞态**：`imp.rs::probe_output_pipe_receives_child_output`
   只读 16 字节 ConPTY 握手（`ESC[?9001h ESC[?1004h`）就断言 `probe-marker` →
   改为循环 PeekNamedPipe + 累积读直到含 marker 或 3s 截止（另有 `E0502` 借用错一并修，
   先 `let take` 再切片）
2. **msys2-ucrt64 `rustc: command not found`**：MSYS2 shell 不继承 GITHUB_PATH 注入的
   rustup 目录 → 该 job 三个 run step 内 `export PATH="$(cygpath -u
   "${CARGO_HOME:-$USERPROFILE/.cargo}")/bin:$PATH"`
3. **ubuntu/macos 挂满 6h 被取消**：`pty-unix::spawn_cwd_env_and_write_roundtrip`
   **自死锁**——先 `read_until("|got:")` 才写 `hi\n`，阻塞在 `libc::read` 使 10s deadline
   断言永不触发（同 job 其余 4 个 unix 测试 1s 内通过）→ 改为先读 banner
   （`OUT:/tmp:bar`）→ 写 `hi\n` → 读 `got:hi`，4 个 spawn 测试加 `watchdog(30)`
4. **全 job 补 `timeout-minutes`**（docs 10 / whitelist 10 / build-test 30 / msys2 30 /
   pipe-mvp 20 / matrix 15）杜绝再挂 6h
5. 本地复跑又抓到 `resize_applies_and_interactive_shell_echoes` 竞态（resize 重绘
   `ESC[137X` 先于 `mode con` 纯文本命中 `read_until("137")`）→ `strip_ansi` +
   `read_until_all(["137","53"])` 按去转义后的纯文本匹配

本机验证：`cargo test -p pty-conpty` **17/17 连跑 3 次全绿**、`scripts/ci-local.ps1`
**PASSED**（docs/生成物幂等/白名单 65+3/workspace 构建测试/pipe_mvp）、白名单 linter OK；
unix 测试本机不可执行 → push 后由 CI 判定（修复含读写顺序，无行为猜测风险）。

## 本机环境（当前机器，2026-10-06；非 MoMo）

- OS：**Windows 10 企业版 LTSC（build 10.0.17763 / 1809）**→ ConPTY 本机可用；
  **本机无 WSL**（`wsl.exe` 不存在）→ unix 集成测试只由 CI 执行
- 仓库：`F:\wbwtty`；临时/下载：`F:\wbwtty-temp`
- Rust：`RUSTUP_HOME=D:\rustup`、`CARGO_HOME=D:\cargo`（cargo/rustc **1.99.0**、
  rustup 1.29.0，toolchain `stable-x86_64-pc-windows-gnu`，含 linux-gnu target 供交叉检查）
- MSYS2：`D:\msys`（ucrt64，阶段 2 起的 gtk3 安装；workspace 构建已验证）
- **路径/环境变量一律「项目盘符优先、旧机固定路径兜底」的候选探测**（`scripts/env.ps1`，
  `ci-local.ps1` 点号引入同一份逻辑，候选不存在自动跳过）；**`.ps1` 必须 UTF-8 带 BOM**
  （PS 5.1 无 BOM 按 GBK 解码会吞换行——env.ps1 曾因此语法错）
- 每次开新终端：`. .\scripts\env.ps1`（输出应见 `rustup=D:\rustup cargo=D:\cargo
  msys=D:\msys\ucrt64\bin`）
- 代理：127.0.0.1:7890（crates.io/GitHub 直连可用）
- gh CLI：已认证 wbw121124（scopes 含 repo/workflow；仓库 public，未认证 REST 也可读结论，
  job logs 需认证）
- 换机历史：2026-10-05 wbw/MoMo（Win11 22621、`G:\wbwtty`）→ 2026-10-06 回到本机
  （`F:\wbwtty`；旧记录保留在 §"Linux GTK 实测"与 plan.md §6）
- 范围约定：只在 `F:\wbwtty` 内改文件；下载/解压 `F:\wbwtty-temp`；不扫全盘

## MSYS2 环境（本机 `D:\msys`）

- 本机（Win10 LTSC，2026-10-06）：MSYS2 在 **`D:\msys`**（ucrt64；阶段 2 安装并补装
  gtk3/pkgconf）→ `scripts/env.ps1` 候选探测 `<盘符>:\msys` 不存在 → 命中 `D:\msys` 前置
  PATH；本机 workspace 构建（含 term-render-gtk）已验证可用（ci-local 全绿）
- 历史（2026-10-05 wbw/MoMo 机）：MSYS2 预装 `E:\吴邦玮\项目\mymsys2`（ucrt64 gcc 16.2.0），
  补装 `mingw-w64-ucrt-x86_64-gtk3 pkgconf` → gtk3 **3.24.52**、pangocairo 1.58.2
  （tuna 镜像对代理 403 → 去代理直连成功）；该路径仍保留在 env.ps1 候选列表中

## Linux GTK 实测（Ubuntu 20.04，2026-10-06）

在 Windows（MSYS2）之外的**第二平台**实测 GTK 渲染器，仓库位于 U 盘
`/media/noi/wbw_121124 的 USB 闪存盘/wbwtty`，主机 K404-B111（X11 `DISPLAY=:0`）：

- 环境补装（初始状态：无任何 rustup toolchain、有 GTK3 运行库但缺 `libgtk-3-dev`）：
  1. `rustup default stable` → rustc/cargo **1.99.0**
  2. `sudo apt-get install -y libgtk-3-dev` → gtk+-3.0 **3.24.20**（连带 epoxy/wayland/xkbcommon dev）
  3. 校验：`pkg-config --modversion gtk+-3.0 pangocairo` → `3.24.20` / `1.44.7`
- 测试结果（全部通过）：
  | 项 | 命令 | 结果 |
  |---|---|---|
  | 单元+集成 | `cargo test -p term-render-gtk` | **39/39**（31 单元 + 8 集成 `pipeline.rs`，0 失败） |
  | 链接/运行时冒烟 | `cargo run -p term-render-gtk --example gtk_smoke` | `gtk::init OK - GTK3 linked and loadable`，退出码 0 |
  | 窗口视觉核验 | `cargo run -p term-render-gtk --example demo_window` + `xwininfo` + `gnome-screenshot` | 窗口 `"term-render-gtk demo"` 800x600 存在；截图可见帧计数、真彩色渐变、下划线/删除线/斜体、移动方块（局部 damage）、底行宽字符 `宽字符 中文abc é 完成` |
- 副作用：`Cargo.lock` 跨机带来的 `version = 4→3` 头部差异已 `git checkout` 恢复，
  **工作树干净**（`cargo metadata` 仍可解析 v4）
- 结论：GTK 渲染器在 **Windows(MSYS2)/Ubuntu 两平台均实测通过**，与 CI `build-test`
  的 Linux 权威验证一致

## 已知问题 / 阻塞

- 本机无 MSVC/Windows SDK → per-SDK 编译由 CI（windows-latest + ilammy/msvc-dev-cmd）承担；
  本机 `scripts/ci-local.ps1` 在 cl 不在 PATH 时自动跳过该步（已决策）
- ~~本机无 gh CLI 且未认证~~ ✅ 已认证（wbw121124，2026-10-05）
- ~~MSYS2 gtk3 未安装~~ ✅ 已补装 `E:\吴邦玮\项目\mymsys2`（2026-10-05，见"MSYS2 环境"）
- ~~Rust(gnu) ↔ MSYS2 ucrt64 GTK 链接未验证~~ ✅ 已实测通过（`gtk_smoke`、`demo_window` 本机跑通）
- CI 的 windows/macos runner 无 GTK 开发包 → `build-test` 用 `--exclude term-render-gtk`，
  GTK 渲染的权威验证在 Ubuntu + 本机 MSYS2
- ~~**ConPTY 阶段 3 阻塞**~~ ✅ 已修复（`STARTF_USESTDHANDLES` + `hStd*=NULL`，17/17 全绿，
  见"进行中（阶段 3）"）
- ~~**CI run #6/#7/#8 失败（probe 竞态 / msys2 rustc PATH / unix roundtrip 挂死 6h）**~~
  ✅ 已诊断并修复（含全 job `timeout-minutes`），**待 push 后 CI 全绿确认**，
  见"进行中（阶段 3）→ CI run #7/#8 失败诊断与修复"
- `pty-unix` 的 unix 集成测试本机（Windows）不可执行 → 由 CI ubuntu/macos 运行
  （**本机无 WSL**，用户原约定的"WSL 仅用于 test"在本机不适用）；本机以 linux 目标
  `cargo check --all-targets` 把编译关（ci-local 步骤 4b）

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

- static_allowed：65 个（process / thread-injection / loader / file-pipe / sync-wait /
  console-host / string / error / **proc-thread-attribute** 分组），7 版 SDK 全部声明，允许静态链接
- dynamic_only：ConPTY 三函数，必须 `LoadLibrary+GetProcAddress`
- 其余任何 Win32 函数符号禁止在 pty-win10-early/pty-conpty 静态引用；
  新增需改 `tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` 重新生成（生成器拒绝任何一版缺失）
- 强制检查：`python tools/check_api_whitelist.py`（本机 + CI）

## 约定

- 提交：Conventional Commits，单一逻辑变更；提交前相关模块构建+测试通过
- 分支：main 稳定；`feature/<模块>`、`fix/<简述>`，CI 通过后合回
- 标签：`v0.x.0-stageN`，仅打在 main；文档随代码同步更新
- 路径：**项目盘符优先的候选探测**（`scripts/env.ps1`，`ci-local.ps1` 点号引入同一份逻辑，
  候选缺失自动跳过）；下载/解压 `<项目盘符>:\wbwtty-temp`；只在当前仓库 `F:\wbwtty` 内改文件；
  **`.ps1` 必须 UTF-8 带 BOM**；旧机历史记录（docs/header-verification.md 等）保留原文不改写
