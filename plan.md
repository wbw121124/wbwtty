# plan.md — 跨平台终端框架实施计划

- 最近更新：2026-10-06（阶段 3 收尾 **round 3**：run #10 后定案 interrupt 根因——runner
  启动链 `CREATE_NEW_PROCESS_GROUP` 隐式“忽略 Ctrl+C”属性**可继承** → 测试 spawn 前清继承
  ignore 即修；本地 0x200 标志精确复现）
- 上次更新：2026-10-06（round 2：run #9 复盘——conpty env/cwd 读竞态 + interrupt 重发 ETX、
  macos `/tmp` 符号链接、msys2 `PKG_CONFIG_ALLOW_CROSS`）
- 当前阶段：**阶段 2 完成**（`v0.3.0-stage2`）→ **阶段 3 pty-conpty 已合 main 并打标签
  `v0.4.0-stage3`**；round 1+2 合入 main `2d79f4c` 推送后 run #10 **10 job 绿 / 2 job 红**
  （仅 `interrupt_stops_long_running_child` 在 windows-latest 与 msys2 双挂 → round 3 根因
  修复待合入推送）→ CI 全绿后关闭阶段 3、进入阶段 4
- 版本规划：v0.1.0-stage0 → v0.2.0-stage1 → v0.3.0-stage2 → v0.4.0-stage3 → v0.5.0-stage4 → 阶段 5 持续

## 1. 目标与范围

跨平台终端框架（Windows 10 全版本含 1809 之前、Windows 11、Linux、macOS），完整 PTY 语义，
GPU 加速渲染，模块化可独立复用，全程 Git + Conventional Commits + 分支/标签规范 + CI。

已确认的执行决策（2026-10-03 与需求方确认）：

| 决策点 | 结论 |
|---|---|
| SDK 头文件获取 | ralish/win-headers 快照为 7 版本权威来源 + 官方安装器（10240、17763）静默安装交叉校验 |
| 下载/解压位置 | **按仓库所在盘符自动检测**（`scripts/env.ps1`/`ci-local.ps1`）→ `<盘符>:\wbwtty-temp`（当前仓库在 F:，即 `F:\wbwtty-temp`）；网络代理 127.0.0.1:7890 可用 |
| per-SDK 编译检查 | 真实编译矩阵在 GitHub Actions windows-latest（MSVC cl）；本机用 Python 白名单 linter |
| CI 平台 | GitHub Actions（.github/workflows/ci.yml）+ 创建远端仓库并推送 |
| GTK 渲染 | 本机已有 MSYS2（`E:\吴邦玮\项目\mymsys2`，ucrt64），2026-10-05 补装 gtk3+pkgconf；CI Ubuntu 为权威验证 |
| 执行范围 | 阶段 0→5 严格顺序推进，每阶段提交+标签后进入下一阶段 |
| Git 身份 | wbw121124 <wbw121124@163.com>（仅仓库 local 配置） |
| 本机持久化 | **项目盘符优先的候选探测**（`scripts/env.ps1`，`ci-local.ps1` 点号引入同一份逻辑）：临时/下载 → `<盘符>:\wbwtty-temp`；`RUSTUP_HOME` `<盘符>:\rustup`→`D:\rustup`、`CARGO_HOME` `<盘符>:\cargo`→`D:\cargo`、MSYS2 `<盘符>:\msys`→`E:\吴邦玮\项目\mymsys2`→`D:\msys`（候选不存在自动跳过）；**`.ps1` 必须 UTF-8 带 BOM**（PS 5.1 无 BOM 按 GBK 解码会吞换行）；git 按需 safe.directory；换机历史见 §6（2026-10-05 wbw/MoMo → 2026-10-06 Win10 LTSC 17763 机器、仓库 `F:\wbwtty`） |

## 2. 仓库结构

```
F:\wbwtty\
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
- [x] Linux 本机（Ubuntu 20.04 / X11）**实测通过**（2026-10-06）：39/39 测试 + `gtk_smoke` +
      `demo_window` 窗口截图核验，详见 §6 与 AGENT.md"Linux GTK 实测"
- [x] 标签 **v0.3.0-stage2**（term-render-qt 列入 backlog）

### 阶段 3：pty-conpty（2-3 周）— 代码/文档/标签已交付，CI 修复已提交（待 CI 全绿后关闭）
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
- [x] 提交 `feat: implement ConPTY backend with dynamic loading`（`c2829ab`）→
      `docs: update stage-3 progress…`（`cd02620`），合 main（`9d0920a`）并打标签
      **v0.4.0-stage3**（均推送 origin）
- [x] CI run #7/#8 三项失败修复（round 1，2026-10-06，分支 `fix/ci-stage3-green`，见 §6）：
      probe 测试竞态 → 累积读；msys2 `rustc not found` → step 内 export PATH；
      unix roundtrip 自死锁 → 读写顺序 + 看门狗；全 job `timeout-minutes`
- [x] run #9 复盘 round 2 修复（同分支续）：conpty `env_and_cwd` 读竞态 + `interrupt` 重发
      ETX、macos `/tmp`→`/private/tmp` 符号链接、msys2 `PKG_CONFIG_ALLOW_CROSS='1'`
- [x] run #10 复盘 round 3 修复（同分支续）：interrupt 根因 = `CREATE_NEW_PROCESS_GROUP`
      祖先隐式 `SetConsoleCtrlHandler(NULL,TRUE)` 且**可继承** → 测试 spawn 前清继承 ignore
      （本地 0x200 标志精确复现；CTRL_BREAK 兜底弯路已弃，见 §6）
- [ ] CI 全绿确认（push 后监控）→ 关闭阶段 3、进入阶段 4

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
   **Linux 侧亦已本机实测**（2026-10-06，Ubuntu 20.04：39/39 测试 + 冒烟 + 窗口截图核验）。
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
- 2026-10-06（**Linux GTK 实测**，仓库在 U 盘 `/media/noi/wbw_121124 的 USB 闪存盘/wbwtty`，
  主机 K404-B111 / Ubuntu 20.04 / X11 `DISPLAY=:0`）：
  - 环境补装：rustup 初始**无任何 toolchain** → `rustup default stable`（rustc/cargo 1.99.0）；
    GTK3 只有运行库缺 dev → `sudo apt-get install -y libgtk-3-dev`（gtk+-3.0 **3.24.20**，
    连带 epoxy/wayland/xkbcommon dev）；`pkg-config --modversion gtk+-3.0 pangocairo`
    → `3.24.20` / `1.44.7`
  - `cargo test -p term-render-gtk` → **39/39 全绿**（31 单元 batch/color/cache/metrics/
    viewport/renderer + 8 集成 `tests/pipeline.rs`，0 失败）
  - `cargo run -p term-render-gtk --example gtk_smoke` → `gtk::init OK - GTK3 linked and loadable`（退出码 0）
  - `cargo run -p term-render-gtk --example demo_window`（后台）→ `xwininfo` 见窗口
    `"term-render-gtk demo"` 800x600；`gnome-screenshot` 截图核验：帧计数、真彩色渐变、
    下划线/删除线/斜体、移动方块（局部 damage）、底行宽字符 `宽字符 中文abc é 完成` 均正确
  - 副作用清理：`Cargo.lock` 跨机带来的 `version = 4→3` 头部差异已 `git checkout` 恢复，
    **工作树干净**
  - 结论：term-render-gtk 在 **Windows(MSYS2 ucrt64) / Ubuntu 20.04 双平台实测通过**
- 2026-10-06（**阶段 3 收尾：CI run #7/#8 三项失败诊断与修复**，分支 `fix/ci-stage3-green`；
  本机为另一台机器：Win10 企业版 LTSC 17763、仓库 `F:\wbwtty`、**无 WSL** → unix 测试只靠 CI）：
  - 现状：main `9d0920a`（tag `v0.4.0-stage3`）已推送；run #7/#8 红，gh 拉 job 日志定案三项根因
  - 根因 1（windows-latest）：`imp.rs` probe 测试只读 16 字节 ConPTY 握手
    （`ESC[?9001h ESC[?1004h`）就断言 `probe-marker` → **竞态** → 改为循环 PeekNamedPipe+
    累积读直到含 marker 或 3s 截止
  - 根因 2（msys2-ucrt64）：MSYS2 shell 不继承 GITHUB_PATH 注入的 rustup 目录 →
    `rustc: command not found` → 该 job 三个 run step 内 `export PATH="$(cygpath -u
    "${CARGO_HOME:-$USERPROFILE/.cargo}")/bin:$PATH"`
  - 根因 3（ubuntu/macos 挂满 6h 被取消）：`pty-unix` 集成测试 `spawn_cwd_env_and_write_roundtrip`
    **自死锁**——先 `read_until("|got:")` 才写 `hi\n`，阻塞在 `libc::read` 使 10s deadline 断言
    永不触发（其余 4 个 unix 测试 1 秒内通过，日志证据）→ 改为先读 banner（`OUT:/tmp:bar`）
    再写、再读 `got:hi`，并给 4 个 spawn 测试加 `watchdog(30)`（conpty 同款惯例）
  - 附带：全 job 补 `timeout-minutes`（docs 10 / whitelist 10 / build-test 30 / msys2 30 /
    pipe-mvp 20 / matrix 15），杜绝再挂 6h；本地复跑又抓到两个测试健壮性问题并一并修：
    probe 测试 `E0502` 借用错（先 `let take` 再切片）、`resize_applies_and_interactive_shell_echoes`
    被 resize 重绘 `ESC[137X` 提前命中 → `strip_ansi` + `read_until_all(["137","53"])` 按纯文本匹配
  - 环境：`scripts/env.ps1` 改为**项目盘符优先的候选探测**（`RUSTUP_HOME` `<盘符>:\rustup`→
    `D:\rustup`、`CARGO_HOME` `<盘符>:\cargo`→`D:\cargo`、MSYS2 `<盘符>:\msys`→旧机路径），
    `ci-local.ps1` 改为点号引入 env.ps1 去重；**修 `.ps1` 必须 UTF-8 带 BOM**（env.ps1 编辑时
    丢 BOM → PS 5.1 按 GBK 解码吞换行 → 行合并语法错）
  - 本机验证：`cargo test -p pty-conpty` **17/17 连跑 3 次全绿**、`scripts/ci-local.ps1` **PASSED**、
    白名单 linter 65 static + 3 dynamic-only、`pipe_mvp` PASS；unix 测试无法本机执行 → 由 CI 判定
- 2026-10-06（**CI round 1 推送后 run #9 复盘 + round 2 修复**，同分支续）：
  - round 1 已合入 main（`653ba8c`，`--no-ff`，4 提交 `fix:`×2 + `ci:` + `docs:`）并推送
  - run #9 结果：**9 job 绿**——docs-check、api-whitelist、7×SDK 矩阵、
    `build-test (ubuntu-latest)`（unix roundtrip 修复生效，5/5 过）；**3 job 红**（均为首次
    真正跑到的新代码路径）：
    1. **windows（pty-conpty 集成 4/6）**：`env_and_cwd_are_applied` 又是读竞态——只读到
       `stage3-ok` 就返回、`cd` 的 cwd 行未到（断言 `pty-conpty` 失败）→
       `read_until_all(["stage3-ok","pty-conpty"])` 且**大小写不敏感**；
       `interrupt_stops_long_running_child` 单发 ETX 在 Server 2022 runner 上不生效
       （同 job `mode con\r` 交互通过 → 输入管道本身是通的）→ 每 500ms **重发 ETX** 直到
       退出或 25s 截止（本地重发版 17/17，interrupt ~1.3s）
    2. **macos（spawn_unix 4/5）**：`/tmp` 是 `/private/tmp` 的符号链接 → `getcwd` 返回
       `/private/tmp`，字面 needle `OUT:/tmp:bar` 永不命中，输出读完后阻塞到 30s 看门狗；
       → needle 改行尾 `:bar`（保证整行读全），再分别断言 `OUT:` / `:bar` / `/tmp`
    3. **msys2（Build workspace）**：rustup 主机是 msvc、`--target` 是 gnu → pkg-config crate
       判定交叉编译，`glib-sys` build script 报 "pkg-config has not been configured to
       support cross-compilation"（该 job round 1 才第一次走到 build）→ job 级
       `env: PKG_CONFIG_ALLOW_CROSS: '1'`
  - 本机验证：`cargo test -p pty-conpty` **17/17 ×3**（重发版 interrupt 正常 ~1.3s）、
    ci-local 全绿后推送 round 2
- 2026-10-06（**CI run #10 复盘 + round 3：interrupt 根因定案**，同分支续）：
  - round 2 已合入 main（`2d79f4c`，`--no-ff`）并推送 → run #10：**10 job 绿**
    （macos `/private/tmp` 修复生效、msys2 build 过、windows 集成 5/6、ubuntu/docs/白名单/
    7×矩阵）；**2 job 红**：`interrupt_stops_long_running_child` 在 windows-latest 与 msys2
    均 25.83s 挂在 `child should exit after Ctrl+C`（每 500ms 重发 ETX 25s 仍无效）
  - 诊断：同 job `mode con\r` 交互通过 → 输入管道通；本地 `CREATE_NO_WINDOW`（无控制台）
    模拟通过 → 排除“测试进程无控制台”；**本地 python `creationflags=0x200`
    （CREATE_NEW_PROCESS_GROUP）启动 cargo test → 与 CI 一字不差复现**（26s 同断言失败，
    flag=0 则 1.3s 过）
  - 根因：带该标志创建的进程隐式 `SetConsoleCtrlHandler(NULL,TRUE)`（忽略 Ctrl+C），且该
    属性**可继承** → 祖先链（CI runner 启动链）→ 测试进程 → cmd → ping 全链继承 →
    conhost 把 ETX 转成 Ctrl+C 事件投递成功，但各进程按继承属性跳过处理器 → 无人退出
  - 修法：`interrupt_stops_long_running_child` 在 spawn 前 `SetConsoleCtrlHandler(NULL,FALSE)`
    清掉继承的 ignore（子进程从干净状态创建）→ 复现条件下 1.30s 过、正常环境 6/6 过
  - 弯路（已弃，仅记结论）：CTRL_BREAK 兜底（AttachConsole + GenerateConsoleCtrlEvent）——
    group id 0 广播对伪控制台不投递；`NULL,TRUE` 挡不住 Ctrl+Break 会自杀（0xC000013A）；
    定向投递成功也杀不掉 cmd → 复杂且无效，全部移除
  - 产品级提示（记入已知限制）：pty-conpty spawn **不改**父进程的 Ctrl+C 处理器（库副作用
    不可接受）；处于此类被保护启动链的应用需自行 `SetConsoleCtrlHandler(NULL,FALSE)` 才能让
    ConPTY 子进程收到 Ctrl+C
  - 本机验证：`cargo test -p pty-conpty` 6/6（含单元 17/17）×2、flag=0x200 复现条件 ×2、
    新增代码 fmt 干净（仓库其余 fmt 漂移为既有、CI 不检查）、ci-local **PASSED**
