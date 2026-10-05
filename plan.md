# plan.md — 跨平台终端框架实施计划

- 最近更新：2026-10-05（阶段 3 测试全绿，文档/提交推进中；**已恢复到新机器 wbw/MoMo**）
- 当前阶段：**阶段 2 完成**（标签 `v0.3.0-stage2`）→ **阶段 3 pty-conpty**：阻塞已修复
  （`STARTF_USESTDHANDLES` + `hStd*=NULL`，17/17 测试全绿，见 §6），待 README/docs/提交/标签/CI
- 版本规划：v0.1.0-stage0 → v0.2.0-stage1 → v0.3.0-stage2 → v0.4.0-stage3 → v0.5.0-stage4 → 阶段 5 持续

## 1. 目标与范围

跨平台终端框架（Windows 10 全版本含 1809 之前、Windows 11、Linux、macOS），完整 PTY 语义，
GPU 加速渲染，模块化可独立复用，全程 Git + Conventional Commits + 分支/标签规范 + CI。

已确认的执行决策（2026-10-03 与需求方确认）：

| 决策点 | 结论 |
|---|---|
| SDK 头文件获取 | ralish/win-headers 快照为 7 版本权威来源 + 官方安装器（10240、17763）静默安装交叉校验 |
| 下载/解压位置 | **按仓库所在盘符自动检测**（`scripts/env.ps1`/`ci-local.ps1`）→ `<盘符>:\wbwtty-temp`（当前仓库在 G:，即 `G:\wbwtty-temp`）；网络代理 127.0.0.1:7890 可用 |
| per-SDK 编译检查 | 真实编译矩阵在 GitHub Actions windows-latest（MSVC cl）；本机用 Python 白名单 linter |
| CI 平台 | GitHub Actions（.github/workflows/ci.yml）+ 创建远端仓库并推送 |
| GTK 渲染 | 本机已有 MSYS2（`E:\吴邦玮\项目\mymsys2`，ucrt64），2026-10-05 补装 gtk3+pkgconf；CI Ubuntu 为权威验证 |
| 执行范围 | 阶段 0→5 严格顺序推进，每阶段提交+标签后进入下一阶段 |
| Git 身份 | wbw121124 <wbw121124@163.com>（仅仓库 local 配置） |
| 本机持久化 | **换机 wbw/MoMo（Win11 22621，2026-10-05）**：仓库 `G:\wbwtty`；Rust 用默认 `C:\Users\wbw\.rustup/.cargo`；**路径一律按工作目录盘符检测**（临时/下载 → `<盘符>:\wbwtty-temp`，MSYS2 候选探测 `E:\吴邦玮\项目\mymsys2` → `D:\msys`）；git 用 safe.directory 免 dubious ownership |

## 2. 仓库结构

```
G:\wbwtty\
├── .gitignore  AGENT.md  plan.md  README.md  Cargo.toml (workspace)
├── .github/workflows/ci.yml
├── tools/                      # 阶段0脚本
│   ├── fetch-win-headers.ps1   # 稀疏克隆 ralish/win-headers → <盘符>:\wbwtty-temp\win-headers
│   ├── verify-sdk-installers.ps1 # 官方安装器静默安装 + 头文件 diff 校验
│   ├── prune-headers.py        # include 闭包裁剪
│   ├── gen-compat-matrix.py    # 扫描头文件生成兼容矩阵
│   └── check_api_whitelist.py  # API 白名单 linter（本机+CI）
├── sdk-headers/<10.0.xxxxx>/   # 裁剪后的头文件闭包（入库）
├── config/api-whitelist.json   # 机读白名单
├── docs/
│   ├── sdk-compat-matrix.md  api-whitelist.md  header-verification.md
│   ├── architecture.md  win10-early-bridge.md  ipc-signal-protocol.md
│   ├── porting-guide.md  api/
├── crates/
│   ├── vt-parser/  pty-core/  pty-unix/  pty-conpty/
│   ├── pty-win10-early/ (含 csrc/，per-SDK 编译矩阵目标)
│   ├── term-input/  term-render-gtk/  term-render-qt/(可选,backlog)
├── apps/term-app/
├── examples/ (pipe_mvp 等)
└── scripts/ci-local.ps1
```

依赖方向严格单向：`vt-parser`、`term-input`（纯逻辑零平台依赖）← 渲染器与 apps；
`pty-core`（纯 trait/C ABI）← `pty-unix`/`pty-conpty`/`pty-win10-early`。
任何模块不得依赖 apps 或其他无关模块；禁止循环依赖。

## 3. 阶段任务、估算、依赖、验收

### 阶段 0：SDK 兼容性调研（1 周）— 完成
前置条件：无（未完成前不得进入编码阶段）
- [x] git init + .gitignore + AGENT.md/plan.md 初始提交
- [x] 下载 7 版头文件（D:\temp）+ include 闭包裁剪入库
- [x] 官方安装器（10240/17763）静默安装交叉校验 → docs/header-verification.md
      （17763 引导器非提权 0x3e9 失败 → 官方 ISO + msiexec /a 替代，取舍已记录）
- [x] 生成 docs/sdk-compat-matrix.md（逐 API × SDK 版本；ConPTY API 仅 17763+；行为差异；各后端最低 SDK）
- [x] config/api-whitelist.json + docs/api-whitelist.md（早期 SDK 全部存在的 API 才允许静态链接）
- [x] tools/check_api_whitelist.py 本机 linter
- [x] CI：构建+测试矩阵、per-SDK 编译矩阵（cl × 7 版本）、文档存在性检查
- [x] 提交 `docs: add SDK compatibility matrix and API whitelist`
- [x] 标签 **v0.1.0-stage0**
验收：7 版头文件齐备且裁剪入库；矩阵覆盖规格点名的全部拦截函数；
白名单机读+人读齐套；CI 配置齐全；标签与提交符合规范。

### 阶段 1：vt-parser + pty-core + pty-unix + 管道 MVP（1-2 周）— 完成
- [x] vt-parser：UTF-8 增量解码、ESC/CSI/OSC/DCS、SGR 真彩色、光标/擦除/滚动区/alt-screen、
      鼠标模式 1000/1002/1003/1006、bracketed paste、宽字符、滚动缓冲、行级 damage；feed()/get_screen()
- [x] pty-core：trait Pty{spawn/read/write/resize/send_signal/close} + Signal + 后端选择 + C ABI 头
- [x] pty-unix：forkpty + TIOCSWINSZ + 作业控制（CI ubuntu/macos 执行测试）
- [x] 管道 MVP：examples/pipe_mvp.rs 匿名管道启动 python → 喂 vt-parser → 断言屏幕
- [x] 各模块 README + 测试
- [x] 标签 **v0.2.0-stage1**（合回 main 的合并提交上打，见进度日志）
依赖：阶段 0。验收：模块可独立构建测试；MVP 本机运行通过。

### 阶段 2：term-input + term-render-gtk（2-4 周）— 完成
- [x] MSYS2 → D:\msys（ucrt64 + GTK3 开发包；包命名已重构为 mingw-w64-ucrt-x86_64-*）
- [x] term-input：键/鼠/滚轮/paste → VT 序列；修饰键、application keypad、SGR mouse、bracketed paste
- [x] term-render-gtk：GTK3+Cairo，damage 驱动重绘、字形缓存、真彩色、光标、滚动缓冲、resize 联动
      （Rust(gnu) ↔ MSYS2 ucrt64 链接**实测通过**；pangocairo 自声明 FFI；CI 仅 Linux 全量构建）
- [x] 标签 **v0.3.0-stage2**（term-render-qt 列入 backlog）

### 阶段 3：pty-conpty（2-3 周）— 收尾中（测试全绿，待提交/标签/CI）
- [x] 白名单扩展：`proc-thread-attribute` 三函数（Initialize/Update/Delete）入 CURATED_KERNEL
      → 65 static + 3 dynamic-only，7 版全绿（提交 `e111b4f`）
- [x] `crates/pty-conpty` 骨架 + 动态加载：`src/sys.rs`（repr(C) 类型/白名单 extern/
      `ConptyApi` 从 kernel32/kernelbase GetProcAddress，宽串 GetModuleHandleW 已修）、
      `src/cmdline.rs`（引号/环境块 + 7 单测）、`src/imp.rs`（AttributeList RAII、
      spawn_conpty、ConptyPty）、`src/lib.rs`（双平台桩）、`tests/spawn_conpty.rs`
- [x] 单元测试全绿（cmdline 7 + sys 布局/导出 4）；`spawn_conpty_typed` 拆分供测试取句柄
- [x] **阻塞已修复**：4 个集成测试挂死 → 根因“子进程继承父进程 std 管道句柄” →
      修法 `StartupInfo.dwFlags |= STARTF_USESTDHANDLES` + `hStd*=NULL`
      （microsoft/terminal#4380 官方建议；见 §6 2026-10-05 条目）
- [x] 加载失败返回 `SpawnError::BackendUnavailable`（上层注册表回退，pty-win10-early 留占位）
- [x] 本机集成测试 6/6 + 单元 11/11 = **17/17 全绿**（0.88s，看门狗未触发）
- [x] `crates/pty-conpty/README.md`（docs-check 必需）
- [ ] 提交 `feat: implement ConPTY backend with dynamic loading`，标签 **v0.4.0-stage3**

### 阶段 4：pty-win10-early（4-8 周）
- [ ] a) 控制台 API 直驱：隐藏控制台 host + CREATE_SUSPENDED 注入 conhook.dll +
      IAT/GetProcAddress 劫持（自实现 detour）+ 虚拟屏幕缓冲 + 脏矩形 diff→VT→命名管道 +
      输入 VT→WriteConsoleInputW + GenerateConsoleCtrlEvent/带外控制帧信号映射
- [ ] b) Cygwin PTY 适配：运行时探测 cygwin1.dll 导出（实施时核实，不猜测）
- [ ] c) WinPTY 回退：动态探测，仅简单场景，缺库明确报错
- [ ] csrc/ C 垫片供 CI 对 7 个 SDK 逐个 cl 编译；Rust FFI ⊆ 白名单双向校验
- [ ] 子任务逐个 feat:/test:/docs: 提交，标签 **v0.5.0-stage4**

### 阶段 5：完整特性与性能（持续）
鼠标上报、真彩色、宽字符、bracketed paste 端到端；SIGWINCH/Ctrl+C/滚动缓冲；
批量读取、零拷贝、延迟基准（输入延迟 <10ms）；持续文档与提交，保持 main 稳定。

## 4. Git 里程碑与标签计划

| 标签 | 阶段 | 关键提交 |
|---|---|---|
| v0.1.0-stage0 | 阶段 0 完成 | `docs: add SDK compatibility matrix and API whitelist` |
| v0.2.0-stage1 | 阶段 1 完成 | `feat: implement vt-parser` 等 + `test:`/`docs:` |
| v0.3.0-stage2 | 阶段 2 完成 | `feat: implement term-render-gtk` 等 |
| v0.4.0-stage3 | 阶段 3 完成 | `feat: implement ConPTY backend with dynamic loading` |
| v0.5.0-stage4 | 阶段 4 完成 | `feat: implement pty-win10-early ...` 系列 |

分支计划：main 恒定可构建；开发走 `feature/<模块>`、修复走 `fix/<简述>`，
完成后合回 main（CI 通过），必要时 squash；AGENT.md 记录当前分支与最近提交。

CI 门禁：push 与 pull_request 触发；构建全模块、跑单元测试、7 版 SDK 编译矩阵、
文档存在性检查；CI 失败禁止合并 main（仓库启用 branch protection）。

## 5. 风险与取舍

1. 1809 前无 ConPTY → 规格指定的 DLL 注入直驱；注入对受保护进程/杀软敏感，由 Cygwin/WinPTY 路径兜底。
2. 1507 无 `ENABLE_VIRTUAL_TERMINAL_PROCESSING`（1511 起）→ 前端只吃 VT，转换全在 host；
   1507 上依赖 VT 直写的子程序只能经 hook 捕获，不承诺其自带 VT 着色。
3. 本机无 MSVC/SDK → 编译矩阵权威在 CI，本机 linter 等价检查 API 引用。
4. Rust(gnu) ↔ MSYS2 ucrt64 链接 → **已实测通过**（阶段 2：`gtk_smoke`/`demo_window` 本机可运行；
   CI Ubuntu 为权威验证，Windows/macOS CI 无 GTK 开发包时排除该 crate）。
5. 老 SDK 官方安装器 fwlink 可能失效 → 记录于 header-verification.md，以快照为准并说明取舍。

## 6. 进度日志

- 2026-10-03：计划建立；git 仓库初始化（main）；阶段 0 开始。
- 2026-10-03（阶段 0 收尾）：
  - 头文件：稀疏克隆 7 版官方归档 → `sdk-headers/`（闭包裁剪，1230 文件）
  - 官方抽查：10240 静默安装成功（关键头 SHA-256 全一致、147 共有文件 0 差异）；
    17763 引导器 0x3e9 → ISO+MSI 管理解压（153 共有文件 129 一致、24 servicing 差异、
    快照为官方超集）；62 白名单符号两版官方头全部可声明
  - 产出：`docs/sdk-compat-matrix.md`（GENERATED）、`config/api-whitelist.json`、
    `docs/api-whitelist.md`、`docs/header-verification.md`、`docs/architecture.md`、
    `README.md`、`tools/*`（fetch/prune/gen/compare/verify/linter/probe）、
    `scripts/ci-local.ps1`、`.github/workflows/ci.yml`
  - 提交：`chore: add SDK header fetch, pruning and matrix tooling` →
    `docs: add SDK compatibility matrix and API whitelist` →
    `chore: add GitHub Actions CI with per-SDK compile matrix and doc checks` →
    `docs: add README and architecture overview`
  - 标签 **v0.1.0-stage0** 打在阶段 0 收尾提交；进入阶段 1。
- 2026-10-03（阶段 1 完成）：
  - workspace：根 `Cargo.toml`（成员 `.`/vt-parser/pty-core/pty-unix，根包 `wbwtty` +
    `pipe_mvp` example）；`Cargo.lock` 入库（libc 0.2.190）
  - `vt-parser`：零依赖实现（decode/width/parser/screen/terminal），58 单元 + 13 集成测试；
    分支 `feature/vt-parser`（`feat:`/`test:`/`docs:` 三提交）合并回 main
  - `pty-core`：trait + 注册表（priority 升序、`BackendUnavailable` 回退、同 kind 替换）+
    C ABI `include/pty_core.h`；9 测试（伪后端 FFI 往返）
  - `pty-unix`：POSIX 实现（posix_openpt/fork/setsid/TIOCSCTTY/execvp/TIOCSWINSZ/kill(-pid)/
    EIO→EOF/waitpid）；unix 集成测试 5 项；`x86_64-unknown-linux-gnu`（含 tests）与
    `x86_64-apple-darwin` cargo check 通过；Windows 下空壳
  - `examples/pipe_mvp.rs` PASS（python → 管道 → 断言颜色/光标/宽字符/真彩色/damage）
  - ci-local 新增步骤 4b（linux 目标交叉检查）与 pipe_mvp 运行；本机 ci-local **PASSED**
  - 修复提交：`fix: track vt-parser package manifest and workspace lockfile`（清单漏提交）
  - 合并 `feature/pty-core` 后在合并提交打标签 **v0.2.0-stage1**；进入阶段 2。
- 2026-10-03（阶段 2 开始）：
  - MSYS2 完成：安装器 85047704 B（SHA-256 `7CCD43DE…6B5B7E`，GitHub nightly-x86_64 资产）→
    静默装到 `D:\msys`；**包命名重构**：`mingw-w64-ucrt64-*` → `mingw-w64-ucrt-x86_64-*`；
    已装 toolchain（gcc 16.2.0）+ gtk3（3.24.52，85 包/1.26GB）+ pkgconf，
    `pkg-config --modversion gtk+-3.0` = 3.24.52
  - `feature/term-input` 分支：`feat: add term-input VT sequence encoder` →
    `test: add term-input integration tests` → `docs: add term-input module README` →
    `chore: sync Cargo.lock…`（11 单元 + 6 集成测试全绿）
  - 下一步：term-render-gtk（Rust(gnu) ↔ ucrt64 链接实测）。
- 2026-10-05（阶段 2 完成）：
  - 本机环境：C: 重启被清空 → `RUSTUP_HOME=D:\rustup`、`CARGO_HOME=D:\cargo`（工具链 + 依赖缓存
    + linux/darwin 交叉 target 全部落 D:）；仓库内操作只在 `F:\wbwtty`
  - `feature/term-render-gtk` 分支新增 `crates/term-render-gtk`：
    - 分层 `batch`/`color`/`metrics`/`cache`/`viewport`/`pangocairo`/`renderer`/`widget`
    - gtk-rs 0.18 API 适配（`Propagation`、`Context::metrics`、`Allocation::width()`、
      `set_font_description(Some(..))`、`FontDescription::size()`）
    - **pangocairo FFI**：gtk-rs 0.18 未绑定 `pango_cairo_*` → 自声明一个符号 +
      `build.rs` pkg-config 探测 `pangocairo`（本机 MSYS2 / CI Linux 同一路径）
    - 行尾纯空白段裁剪（背景已铺底，省字形布局；带样式空白保留）
    - 回滚缓冲：`viewport::build_viewport` + 滚轮/`scroll_by`（偏移 >0 整屏重绘、隐藏光标）
    - 示例 `gtk_smoke`（链接冒烟）、`demo_window`（动画演示窗口，本机跑通）
  - 本机 `cargo build/test --workspace` 全绿（**136 测试**：vt-parser 71、pty-core 9、
    term-input 17、term-render-gtk 39、pty-unix 空壳）；`ci-local.ps1` **PASSED**；
    `pipe_mvp` PASS；linux 目标交叉检查 OK
  - CI：`build-test` 在非 Linux 平台 `--exclude term-render-gtk`（GTK 权威验证在 Ubuntu），
    Linux 补装 `pkg-config`
  - 标签 **v0.3.0-stage2** 打在合回 main 的合并提交上；进入阶段 3（pty-conpty）。
- 2026-10-05（阶段 3 进行中，会话在此暂停以便关机）：
  - CI 修复（已提交）：`0fb69ef` 生成 JSON 改 LF（Linux docs-check 幂等）、
    `5675262` 新增 `build-test (msys2-ucrt64-latest)` job；分支 `feature/pty-conpty`
    已推远端（head `5675262`）→ **待查 CI run #6 结果**
  - `e111b4f chore: whitelist proc-thread-attribute APIs needed by ConPTY`（**未推送**）
  - 代码：`crates/pty-conpty/` 全套源码 + 6 个集成测试（工作树未提交：`Cargo.toml`/
    `Cargo.lock` 已改、crate 未跟踪）
  - 测试现状：单元全绿；集成 `backend_registers_and_reports_available`、
    `interrupt_stops_long_running_child`（ETX→Ctrl+C ✅）通过；另外 4 个挂死
  - **根因诊断（跨语言复现，非本 crate FFI 问题）**：
    1. 结构体尺寸 104/112/24/4/8 与 SDK 一致；`PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE
       =0x00020016` 与 `winbase.h` 一致（`ProcThreadAttributePseudoConsole = 22`）
    2. 伪控制台真的建起来了：`conhost.exe --headless --width 80 --height 25` 出现；
       子进程 `title` 不改我们的标题、`mode con` 报 **25 行 × 80 列**（我们本机是
       3000×120）→ **子进程确实挂在伪控制台上**
    3. 但 Windows **总是把父进程 std 句柄 0/1/2 传给子进程**：`bInheritHandles=FALSE`
       且句柄显式去掉 HANDLE_FLAG_INHERIT 仍被继承（`D:\temp\exp_prop.py`、
       `exp_inhflag.py` 验证）→ `cmd /c echo X` 写到**父进程 stdout**，ConPTY 输出
       管道 0 字节 → `read_until` 永远等不到 needle
  - 实验脚本（关机后仍在 D:\temp）：`conpty_probe.py`、`exp_title.py`（title 判定）、
    `exp_matrix.py`、`exp_prop.py`/`exp_inhflag.py`（std 继承语义）、
    `exp_std.py`/`exp_final.py`（STARTF_USESTDHANDLES 变体）、`exp_width.py`
  - 候选修法：`STARTF_USESTDHANDLES` + `hStd*=0`（zhiburt/conpty 做法，引
    microsoft/terminal#4380 评论 580865346）；**但本机 Python 复现该方案时
    `echo` 输出彻底丢失**，尚未调和 → 对照实验 `D:\temp\zctest`（conpty 0.7.0，
    main.rs 已写好**尚未运行**）是下一步第一件事
  - 待办：跑 zctest 对照 → 定修法 → 6/6 集成测试 → README/docs 更新 → ci-local →
    分批提交合 main + 标签 `v0.4.0-stage3` → 推送（含 `e111b4f`）+ 查 CI run #6
- 2026-10-05（**换机恢复** wbw/MoMo / Win11 22621，仓库迁至 `G:\wbwtty`）：
  - 环境适配：`git safe.directory`（原属主 SID 不同报 dubious ownership）；
    `scripts/env.ps1`/`ci-local.ps1` 重写为**按仓库盘符检测**（临时 → `G:\wbwtty-temp`、
    Rust 默认 `C:\Users\wbw`、MSYS2 候选 `E:\吴邦玮\项目\mymsys2`）；gh CLI 已认证
    （wbw121124，token scopes 含 repo+workflow）；MSYS2 补装 gtk3 3.24.52 + pkgconf
    （pacman 直连失败走 127.0.0.1:7890；tuna 镜像对代理 403 → 用直连；post-transaction
    hook 有一次非 ASCII 路径报错但不影响包落地）
  - **对照实验（关键，调和了旧机“输出彻底丢失”的错误结论）**：
    1. `G:\wbwtty-temp\zctest`（conpty@0.7.0，带官方修法）→ `ZTEST-OK` **PASS**；
    2. 复跑原失败测试 → `hello-conpty` 出现在 harness 自己的 stdout、ConPTY 管道
       0 字节 → 60s 看门狗 abort（本机复现“子进程继承父 stdout 管道”）
  - **修法落地**：`imp.rs` spawn 处 `StartupInfo.dwFlags |= STARTF_USESTDHANDLES` +
    `hStdInput/hStdOutput/hStdError = NULL`（`sys.rs` 补常量 0x0000_0100；
    microsoft/terminal#4380 issuecomment-580865346 官方建议，zhiburt/conpty 同款）→
    `cargo test -p pty-conpty` **17/17 全绿**（11 单元 + 6 集成，0.88s）
  - 决策：`probe_output_pipe_receives_child_output` **保留**（原计划删除）——它是
    3 秒快败的回归护栏（PeekNamedPipe 不阻塞），比 60s 看门狗挂死更早暴露修法回退
  - CI run #6（head `5675262`）结论：docs-check / api-whitelist / windows build-test /
    7×SDK 矩阵全绿；`build-test (msys2-ucrt64-latest)` 在 "Environment sanity" 步骤失败；
    ubuntu/macos "Test workspace" 挂 2h13m 后被取消（run 整体 cancelled，疑手动）→ 待 gh 拉日志定位
