# plan.md — 跨平台终端框架实施计划

- 最近更新：2026-10-03
- 当前阶段：**阶段 0 完成（v0.1.0-stage0）→ 阶段 1 进行中**
- 版本规划：v0.1.0-stage0 → v0.2.0-stage1 → v0.3.0-stage2 → v0.4.0-stage3 → v0.5.0-stage4 → 阶段 5 持续

## 1. 目标与范围

跨平台终端框架（Windows 10 全版本含 1809 之前、Windows 11、Linux、macOS），完整 PTY 语义，
GPU 加速渲染，模块化可独立复用，全程 Git + Conventional Commits + 分支/标签规范 + CI。

已确认的执行决策（2026-10-03 与需求方确认）：

| 决策点 | 结论 |
|---|---|
| SDK 头文件获取 | ralish/win-headers 快照为 7 版本权威来源 + 官方安装器（10240、17763）静默安装交叉校验 |
| 下载/解压位置 | **一律 D:\temp**（不在 C:、不在 F:）；F: U盘仅放仓库本体；网络代理 127.0.0.1:7890 可用 |
| per-SDK 编译检查 | 真实编译矩阵在 GitHub Actions windows-latest（MSVC cl）；本机用 Python 白名单 linter |
| CI 平台 | GitHub Actions（.github/workflows/ci.yml）+ 创建远端仓库并推送 |
| GTK 渲染 | MSYS2 安装到 D:\msys（ucrt64 环境），本机可构建；CI Ubuntu 为权威验证 |
| 执行范围 | 阶段 0→5 严格顺序推进，每阶段提交+标签后进入下一阶段 |
| Git 身份 | wbw121124 <wbw121124@163.com>（仅仓库 local 配置） |

## 2. 仓库结构

```
F:\wbwtty\
├── .gitignore  AGENT.md  plan.md  README.md  Cargo.toml (workspace)
├── .github/workflows/ci.yml
├── tools/                      # 阶段0脚本
│   ├── fetch-win-headers.ps1   # 稀疏克隆 ralish/win-headers → D:\temp
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

### 阶段 1：vt-parser + pty-core + pty-unix + 管道 MVP（1-2 周）— 当前
- [ ] vt-parser：UTF-8 增量解码、ESC/CSI/OSC/DCS、SGR 真彩色、光标/擦除/滚动区/alt-screen、
      鼠标模式 1000/1002/1003/1006、bracketed paste、宽字符、滚动缓冲、行级 damage；feed()/get_screen()
- [ ] pty-core：trait Pty{spawn/read/write/resize/send_signal/close} + Signal + 后端选择 + C ABI 头
- [ ] pty-unix：forkpty + TIOCSWINSZ + 作业控制（CI ubuntu/macos 执行测试）
- [ ] 管道 MVP：examples/pipe_mvp.rs 匿名管道启动 python → 喂 vt-parser → 断言屏幕
- [ ] 各模块 README + 测试
- [ ] 标签 **v0.2.0-stage1**
依赖：阶段 0。验收：模块可独立构建测试；MVP 本机运行通过。

### 阶段 2：term-input + term-render-gtk（2-4 周）
- [ ] MSYS2 → D:\msys（ucrt64 + GTK3 开发包）
- [ ] term-input：键/鼠/滚轮/paste → VT 序列；修饰键、application keypad、SGR mouse、bracketed paste
- [ ] term-render-gtk：GTK3+Cairo，damage 驱动重绘、字形缓存、真彩色、光标、滚动缓冲、resize 联动
- [ ] 标签 **v0.3.0-stage2**（term-render-qt 列入 backlog）

### 阶段 3：pty-conpty（2-3 周）
- [ ] ConPTY 三函数 + STARTUPINFOEXW/PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE 全部运行时动态加载
- [ ] 加载失败返回 BackendUnavailable → 上层回退 pty-win10-early
- [ ] 本机 build 17763 集成测试（spawn cmd/resize/Ctrl+C）
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
4. Rust(gnu) ↔ MSYS2 ucrt64 链接 → 实测为准，失败则 GTK 本机构建降级 CI-only 并记入 README。
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
