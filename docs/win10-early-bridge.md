# Windows 10 早期桥接设计（pty-win10-early）

- 最近更新：2026-10-06（阶段 4 开工：设计定稿，第一刀范围划定）
- 关联：`docs/architecture.md` §3/§4、`docs/sdk-compat-matrix.md` §2/§5、
  `docs/api-whitelist.md`、`docs/ipc-signal-protocol.md`、`plan.md` §阶段 4

## 1. 目标与约束

- 目标：Windows 10 1809 之前（1507–1709）无 ConPTY 时提供与 pty-conpty/pty-unix
  一致的 `Pty` 语义（spawn/read/write/resize/send_signal/close/try_wait）。
- 编译基线 **10.0.10240（1507）**；全部 Win32 引用必须在 `config/api-whitelist.json`
  白名单内（`python tools/check_api_whitelist.py` 强制，本机 + CI）。
- 动态加载边界：ConPTY 三函数 `dynamic_only`；`pty-win10-early` 反向**禁止**静态
  引用 ConPTY 符号（linter R2）。新增可用 API 走
  `tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` → 7 版头文件全有才入选。
- 运行时选择（pty-core 注册表，priority 升序）：unix → ConPTY（动态探测成功）→
  early 兜底。early 的 `Backend::available()` 在 Windows 恒真（注册为最低优先级，
  仅当 ConPTY 探测失败 `BackendUnavailable` 时才会被选中）。
- **Win11/1809+ 也可强制走 early 路径**（本机与 CI windows-latest 直接实测，
  不依赖旧系统）；测试通过显式指定 early 后端覆盖默认选择。

## 2. 三路径总览（决策见 docs/sdk-compat-matrix.md §5）

| 路径 | 地位 | 一句话 | 状态 |
|---|---|---|---|
| A. 控制台 API 直驱 | **主路径** | 隐藏控制台 + 屏幕缓冲 → 合成 VT + 输入注入 | 本设计，第一/二刀实现 |
| B. Cygwin PTY 适配 | 兜底 A 失败 | 运行时探测 `cygwin1.dll` 导出（实施时核实，不猜测） | backlog（阶段 4 后段） |
| C. WinPTY 回退 | 最终兜底 | 动态探测 winpty-agent，仅简单场景，缺库明确报错 | backlog |
| D. 内核 condrv 修改 | 拒绝 | 非用户态可及，签名/标签风险 | 永不采用 |

选择逻辑：`EarlyPty::spawn` 内部按 A → B → C 顺序探测；全部失败返回
`SpawnError::BackendUnavailable`（上层注册表回退/明确报错）。注入被杀软拦截
属 A 的已知风险 → 转 B/C 而非硬失败（B/C 就位前先明确报错 + 日志）。

## 3. 路径 A：控制台 API 直驱 —— 详细设计

### 3.1 进程与控制台模型

**第一刀（本会话，轮询共享控制台，零注入）**：

```
term-app / 测试进程（GUI 或 console 均可）
  ├─ FreeConsole()            # 若已挂在父控制台（终端启动的测试/调试）
  ├─ AllocConsole() + 隐藏窗口 # 建立本进程私有控制台
  │    GetConsoleWindow() + ShowWindow(SW_HIDE)   [ShowWindow 入白名单]
  ├─ CreateFileW("CONIN$/CONOUT$") → hIn/hOut     # 不依赖 GetStdHandle
  └─ CreateProcessW(child)   # child 继承本控制台（默认继承，无特殊 flags）
        child = cmd / powershell（原生控制台程序，零适配）
```

- 宿主与 child **共享同一控制台**：宿主用 `ReadConsoleOutputW` 轮询屏幕、
  `WriteConsoleInputW` 注入输入、`SetConsoleScreenBufferSize/WindowInfo` 改尺寸、
  `GenerateConsoleCtrlEvent` 发信号——全部 API 均 7 版可用（min 1507）。
- 已知代价与限制：
  1. **同进程一次只能有一个 EarlyPty**（一个进程只有一个控制台）→
     并发由全局互斥串行化；多实例隔离留给第二刀 helper 进程模型。
   2. 宿主启动过 AllocConsole 后控制台相关的 Ctrl+C 会广播到宿主 →
      **子进程生成前** `SetConsoleCtrlHandler(NULL, FALSE)` 清继承忽略标志
      （否则子进程继承忽略、Interrupt 广播成 no-op——实证），**生成成功后**
      `(NULL, TRUE)` 忽略宿主、close 时恢复
      （**库级副作用，记录为已知限制**；与 pty-conpty"不改父进程 handler"
      的立场不同，原因是共享控制台的广播语义——设计取舍，见 §6）。
  3. 宿主自身的控制台写入（若有）会污染屏幕 → 宿主为 GUI/管道输出，
     不向 CONOUT$ 写（测试 harness 走捕获管道，安全）。
- 隐藏控制台的窗口隐藏必须 `ShowWindow(SW_HIDE)`（`GetConsoleWindow` 早已入
  白名单，`ShowWindow` 为本阶段新增，见 §5）。

**第二刀（注入 conhook.dll，plan 主方案，事件驱动）**：

```
宿主 CreateProcessW(cmd, CREATE_SUSPENDED, 独立隐藏控制台)
  ├─ 注入：VirtualAllocEx(路径) → WriteProcessMemory → CreateRemoteThread(LoadLibraryW)
  │        → ResumeThread                      [thread-injection 组全在白名单]
  └─ conhook.dll（csrc/，CI 逐 SDK cl 编译）
       ├─ 自实现 detour：IAT + GetProcAddress 结果改写，劫持
       │  WriteConsoleW / WriteConsoleOutputCharacterW / WriteConsoleOutputAttribute /
       │  ReadConsoleOutputW / FlushConsoleInputBuffer 等 *Console* 写入面
       ├─ 虚拟屏幕缓冲（字符 + 属性，双宽字符按 2 cell）
       ├─ 脏矩形 diff → VT 合成 → 命名管道（docs/ipc-signal-protocol.md）
       └─ 输入/resize/信号仍由宿主直接走控制台 API（宿主 AttachConsole(child) 或
          child 控制台由宿主持有句柄——实施时按句柄所有权定案，不猜测）
```

- 事件驱动替代轮询：无轮询延迟、可捕获滚动中间态（轮询只保证"当前屏一致"）。
- 注入失败（杀软/受保护进程）→ 落回路径 B/C（就位前明确报错）。
- 第二刀不改变 `Pty` 接口与 VT 合成/输入解码模块——只替换"输出源"
  （轮询读屏 → conhook 管道帧），第一刀代码直接复用。

### 3.2 输出路径：屏幕 → diff → VT

```
轮询线程（第一刀，~16ms 周期；第二刀由管道帧驱动）
  ReadConsoleOutputW(整个可见缓冲)      # 或区域读，先全量后优化
  → 与上一帧 diff（cell: char + attributes）
  → 变更行/矩形 → VT 合成器（src/vt_synth.rs）
  → Pty::read 的字节流（阻塞等待新帧；child 退出 → 最终 flush → Ok(0)=EOF）
```

VT 合成规则（legacy console 属性 → SGR；1507 无 LVB 网格属性时宽字符按
UCS-2 双 cell 写两次/占位，1607+ 读 `COMMON_LVB_GRID_WORLDWIDE`）：

| console 属性 | VT |
|---|---|
| FOREGROUND_*（R/G/B/I） | SGR 30–37/90–97（I=加亮） |
| BACKGROUND_*（R/G/B/I） | SGR 40–47/100–107 |
| COMMON_LVB_UNDERSCORE | SGR 4 |
| COMMON_LVB_REVERSE_VIDEO | SGR 7 |
| 光标位置 | CUP/HVP + 可见性 DECSCUSR |

- 第一版策略：**变更行整行重绘**（行首 CUP + 该行新内容），矩形级优化留阶段 5。
- 与 vt-parser 的关系：合成的 VT 是"给前端 vt-parser 吃"的字节流，测试用
  vt-parser 断言屏幕状态（与 pipe_mvp 同一验证模式）。

### 3.3 输入路径：前端 VT → INPUT_RECORD

`Pty::write(bytes)` 收到的是 term-input 产出的 VT 字节 → `src/input_vt.rs`
子集解码 → `WriteConsoleInputW(hIn, KEY_EVENT/MOUSE_EVENT records)`：

| 输入 | 注入 |
|---|---|
| 可打印字符（UTF-8→UTF-16） | KEY_EVENT down+up，UnicodeChar，repeat=1 |
| `\n`/`\r`、BS、TAB | VK_RETURN/VK_BACK/VK_TAB |
| CSI 方向/功能键（A/B/C/D/H/F、OP–OS） | VK_UP/…/VK_F1–F4 + LEFT/RIGHT_SHIFT_PRESSED 按需 |
| ETX(0x03) | 'C' + LEFT_CTRL_PRESSED（等效 Ctrl+C，走 conhost 处理输入） |
| SGR 鼠标（1000/1002/1003/1006） | MOUSE_EVENT_RECORD（MOUSE_MOVED + 按键位 + 坐标） |
| bracketed paste（2004） | 逐字符序列（console 无粘贴语义） |

- 1507 无 `ENABLE_VIRTUAL_TERMINAL_INPUT`（1607 起）→ **必须注入键事件**，
  不能依赖 console 直读 VT（matrix §5 结论）。
- 不支持的序列：静默丢弃（记 debug 日志），不报错——与 ConPTY 写入面一致。

### 3.4 resize

`Pty::resize(cols, rows)` → `GetConsoleScreenBufferInfo` → 先
`SetConsoleWindowInfo(bAbsolute=TRUE, 新 srWindow=左上原点+新尺寸)` → 再
`SetConsoleScreenBufferSize(新尺寸)`（顺序：窗口先收、缓冲后随，避免
窗口 > 缓冲的非法态；扩缓冲先于扩窗口的对偶顺序同样遵守）。
child 侧感知：console 程序轮询 `GetConsoleScreenBufferInfo` / 收到
`WINDOW_BUFFER_SIZE_EVENT`（若开启了 ENABLE_WINDOW_INPUT 输入队列——第一刀不开，
child 以轮询感知为准，cmd/powershell 均如此工作）。

### 3.5 信号映射

| `pty_core::Signal` | 早期路径实现 | 说明 |
|---|---|---|
| `Interrupt` | `GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)` 广播（同控制台全进程） | 子进程生成**前**先 `SetConsoleCtrlHandler(NULL, FALSE)` 清继承标志，生成**后**才给宿主置忽略——顺序反了子进程会继承忽略、广播成 no-op（实证） |
| `Quit` | 同上（Ctrl+\ 未实现→ 首版按 Interrupt 处理并记限制） | 首版降级 |
| `Term`/`Kill` | `TerminateProcess(child, 1)` | Windows 控制台无 SIGTERM 对应；硬终止 |
| `Hangup` | 同 `Term`（close 时亦然） | 幂等：已退出则 Ok |
| `Winch` | 无操作（resize 即触发） | 与 ConPTY 后端一致 |

- 备选（记录，不采用）：① child 用 `CREATE_NEW_PROCESS_GROUP` +
  `GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, groupId)` 定向——阶段 3 已证明该标志
  使组内 Ctrl+C **被继承性忽略**，且 CTRL_BREAK 对 cmd 的终止语义等同 Kill；
  ② `WriteConsoleInputW` 键事件注入——conhost 不对注入路径做急切 CTRL_C 转换，
  无人读输入缓冲时永不生成信号（实证）。

### 3.6 生命周期与失败清理

- spawn 失败任何一步 → 逆序清理（已开控制台 FreeConsole、已挂 handler 恢复、
  已创建进程 TerminateProcess+等待），与 pty-conpty `release()` 同模式。
- EOF：轮询发现 `GetExitCodeProcess != STILL_ACTIVE` → 最终帧 flush →
  后续 `read` 返回 `Ok(0)`；`try_wait` 收割返回 `Ok(Some(code))`。
- `close`：幂等（重复 Ok）；发 `Hangup`（TerminateProcess）→ 等待 →
  FreeConsole + 恢复 Ctrl+C handler → Drop 同样走 close（防僵尸）。

### 3.7 测试策略

- 单元：VT 合成（属性→SGR 矩阵）、输入解码（VT→INPUT_RECORD 表驱动）、
  resize 顺序、信号表、白名单符号引用（linter 兜底）。
- 集成（本机 Win11 + CI windows-latest 均跑，强制 early 后端）：
  1. `spawn_echo_roundtrip`：spawn cmd → 写 `echo` + 回车 → 轮询读 →
     vt-parser 断言屏幕含输出；
  2. `resize_reflected_in_screen`：resize → 读 GetConsoleScreenBufferInfo 断言；
  3. `interrupt_terminates_child`：Interrupt → try_wait 返回；
  4. `close_is_idempotent` + Drop 清理。
- 每个集成测试带 `watchdog(30)`（conpty/unix 同款惯例）；EarlyPty 互斥
  （§3.1 限制 1）→ 测试内共享静态 Mutex 串行。
- ci-local 增加 `cargo test -p pty-win10-early`；unix 交叉检查照旧。

## 4. 路径 B（Cygwin PTY）与路径 C（WinPTY）—— 探测设计

- B：`LoadLibraryW("cygwin1.dll")` → `GetProcAddress` 探测 `openpty/forkpty/
  winsize` 导出（**具体导出名实施时以真实 DLL 为准核实，不猜测**）；成功则
  经 Cygwin PTY 语义转接。仅当 A 失败（注入被拦）才尝试。
- C：`LoadLibraryW("winpty.dll")` + `winpty-agent.exe` 存在性探测；
  `SpawnError` 缺库消息明确（"early path A/B/C 全部不可用: …"）。
- 两者均为动态加载（loader 组白名单），缺库不报崩溃。

## 5. 白名单扩展（本阶段新增）

`tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` 新增组（7 版全有才入选，
生成器把关）：

| 组 | API | 用途 |
|---|---|---|
| `console-screen` | `ReadConsoleOutputW`、`GetConsoleScreenBufferInfo` | 第一刀读屏/取尺寸 |
| `console-input` | `WriteConsoleInputW` | 输入注入 |
| `console-resize` | `SetConsoleScreenBufferSize`、`SetConsoleWindowInfo` | resize |
| `window-control` | `ShowWindow` | 隐藏控制台窗口（GetConsoleWindow 已在库） |

（`GetConsoleScreenBufferInfoEx`/`SetConsoleCursorPosition`/`SetConsoleTextAttribute`
等按需后补，未用不入白名单——白名单是允许面不是清单义务。）
已核对：matrix §2 的 console-api 表显示上述 API 均 min 1507（7 版全绿）；
`ShowWindow` 在 `um/winuser.h`（7 版 SDK 必有）。

## 6. 已知限制与风险（随 README/AGENT 摘要发布）

1. **一个进程一个控制台** → 第一刀 EarlyPty 串行；并发实例待第二刀/helper 模型。
2. **Ctrl+C 广播副作用**：spawn 期间宿主临时忽略 Ctrl+C（close 恢复）——
   库在 early 模式下的已知副作用；与 conpty 后端"零副作用"立场的差异已记录。
3. 轮询语义：只保证"当前屏一致"，快速滚动的中间态可能丢失（第二刀注入消除）。
4. 注入对杀软/受保护进程敏感（第二刀）→ 落回 B/C。
5. 1507 无 LVB 网格位/VT 模式 → 宽字符与 UTF-8 代码页行为按 matrix §5 记录
   取舍（统一走 `*W` API，不依赖 65001 代码页）。

## 7. 与 plan.md 阶段 4 清单的映射

| plan 清单项 | 本设计对应 |
|---|---|
| a) 控制台 API 直驱（注入 conhook…） | §3；第一刀=§3 轮询版，第二刀=§3.1 注入版（plan 主方案） |
| b) Cygwin PTY 适配 | §4（backlog，A 就位后） |
| c) WinPTY 回退 | §4（backlog） |
| csrc/ C 垫片供 7 SDK cl 编译 | 第二刀 `csrc/conhook.c`；CI `sdk-compile-matrix` 已就位 |
| 子任务逐个提交，标签 v0.5.0-stage4 | 按 Step 提交；标签待阶段 4 全部完成 |
