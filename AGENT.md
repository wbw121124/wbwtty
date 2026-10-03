# AGENT.md — 项目状态与执行记录

- 最近更新：2026-10-03

## 当前 Git 状态

- 当前分支：main（阶段 1 经 `feature/vt-parser`、`feature/pty-core` 分支开发后合并）
- 最近提交：阶段 1 `feat: implement vt-parser terminal state machine` → `feat: add pty-core PTY abstraction, registry and C ABI` → `feat: add pty-unix POSIX PTY backend` → 合并提交（精确哈希见 `git log`）
- 已打标签：`v0.1.0-stage0`、`v0.2.0-stage1`（阶段 1 合并提交上，见 plan.md 进度日志）
- 待合并分支：（无）

## 当前阶段

**阶段 1 完成（v0.2.0-stage1）**，进入阶段 2：term-input + term-render-gtk（前置 MSYS2 → D:\msys）。

## 模块划分（计划）

| 模块 | 职责 | 状态 |
|---|---|---|
| vt-parser | VT 序列解析 + 终端状态（网格/颜色/光标/滚动缓冲/damage） | ✅ 阶段 1（58 单元 + 13 集成测试） |
| pty-core | PTY 统一抽象（trait + C ABI），无平台实现 | ✅ 阶段 1（注册/选择/回退 + FFI，9 测试） |
| pty-unix | openpty/forkpty 后端（Linux/macOS） | ✅ 阶段 1（POSIX 实现；Windows 空壳；unix 测试由 CI 执行） |
| pty-conpty | ConPTY 后端（Win10 1809+，全部动态加载） | 未开始（阶段 3） |
| pty-win10-early | 1809 前桥接：控制台 API 直驱 / Cygwin PTY / WinPTY 回退 | 未开始（阶段 4） |
| term-render-gtk | GTK3+Cairo/OpenGL 渲染 | 未开始（阶段 2） |
| term-render-qt | Qt 渲染（可选，backlog） | backlog |
| term-input | 键鼠事件 → VT 序列编码（纯函数） | 未开始（阶段 2） |
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

## 下一步（阶段 2 待办）

1. MSYS2 安装到 `D:\msys`（ucrt64 + GTK3 开发包）
2. `term-input`：键/鼠/滚轮/paste → VT 序列（修饰键、application keypad、SGR mouse、bracketed paste）
3. `term-render-gtk`：GTK3+Cairo，damage 驱动重绘、字形缓存、真彩色、光标、resize 联动
4. 各模块 README + 测试 → 标签 `v0.3.0-stage2`

## 已知问题 / 阻塞

- 本机无 MSVC/Windows SDK → per-SDK 编译由 CI（windows-latest + ilammy/msvc-dev-cmd）承担；
  本机 `scripts/ci-local.ps1` 在 cl 不在 PATH 时自动跳过该步（已决策）
- 本机无 gh CLI 且未认证 → 创建 GitHub 远端仓库需要一次用户认证（device flow 或 GH_TOKEN）
- MSYS2（D:\msys）尚未安装 → 阶段 2 前完成
- 本机 Rust host 为 x86_64-pc-windows-gnu（无 MSVC 工具链），GTK 链接方案阶段 2 实测
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
- 下载与解压一律 `D:\temp`（不写 C:、不写 F:）
