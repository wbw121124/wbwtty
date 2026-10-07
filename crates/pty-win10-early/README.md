# pty-win10-early

Windows 10 1809 之前（1507–1709，无 ConPTY）的早期 PTY 后端；非 Windows 平台
为可编译空壳。最近更新：2026-10-06。

## 实现要点

| 项 | 说明 |
| --- | --- |
| 地位 | 阶段 4 对象；设计定稿见 `docs/win10-early-bridge.md`、`docs/ipc-signal-protocol.md` |
| 选择 | 注册 priority **30**（conpty 20、unix 10），仅当 ConPTY 动态探测失败时被注册表兜底 |
| 路径 A（主，第一刀） | 本进程 `FreeConsole`+`AllocConsole` 隐藏控制台（`GetConsoleWindow`+`ShowWindow(SW_HIDE)`）→ child 继承共享控制台；轮询线程 `ReadConsoleOutputW` 全屏 diff → VT 合成（`src/vt_synth.rs`）→ `Pty::read`；输入 VT 解码（`src/input_vt.rs`）→ `WriteConsoleInputW` 注入键/鼠事件 |
| 路径 A（第二刀） | `CreateProcessW(CREATE_SUSPENDED)` + 注入 conhook.dll（csrc/ 垫片，IAT/GetProcAddress detour + 虚拟屏 + 命名管道，`docs/ipc-signal-protocol.md`），事件驱动替代轮询 |
| 路径 B/C | 运行时探测 `cygwin1.dll` PTY 导出 / winpty（backlog，A 失败才尝试） |
| resize | `SetConsoleWindowInfo`（窗口先收）→ `SetConsoleScreenBufferSize`（缓冲后随） |
| 信号 | `Interrupt` → 键事件注入 Ctrl+C（不用 `CREATE_NEW_PROCESS_GROUP`，阶段 3 已证其隐式忽略 Ctrl+C）；`Term`/`Kill`/`Hangup` → `TerminateProcess`；`Quit` 首版按 Interrupt 降级 |
| 白名单 | 全部 Win32 引用在 `config/api-whitelist.json`（阶段 4 新增 console-screen / console-input / console-resize / window-control 组，7 版全绿 min 1507）；`tools/check_api_whitelist.py` 强制 |

已知限制（详见设计文档 §6）：一个进程一个控制台 → 第一刀 EarlyPty 全局互斥串行；
spawn 期间宿主临时忽略 Ctrl+C（close 恢复）；轮询只保证当前屏一致（第二刀消除）。

## 状态

骨架：注册 + `spawn` 返回 `BackendUnavailable`（第一刀直驱实现进行中）。

## 测试

`cargo test -p pty-win10-early`（Windows；CI windows-latest 本机同跑）。
集成测试带 30s watchdog、early 后端显式强制、控制台类测试串行互斥。
