# 架构说明（docs/architecture.md）

- 最近更新：2026-10-03（阶段 0 初版，随阶段 1–5 实现持续补充）

## 1. 分层

```
┌────────────────────────────────────────────────────────────┐
│ term-app（示例组装，可忽略）                                  │
├──────────────────────┬─────────────────────────────────────┤
│ term-render-gtk/(-qt)│ term-input（事件 → VT，纯函数）        │   前端层
├──────────────────────┴─────────────────────────────────────┤
│ vt-parser（feed(bytes) → 屏幕状态 + damage；纯逻辑）          │   状态层
├────────────────────────────────────────────────────────────┤
│ pty-core（trait Pty + Signal + 后端选择 + C ABI）            │   抽象层
├──────────────┬───────────────────┬─────────────────────────┤
│ pty-unix     │ pty-conpty        │ pty-win10-early          │   后端层
│ (Linux/macOS)│ (Win10 1809+)     │ (Win10 <1809 三条路径)    │
└──────────────┴───────────────────┴─────────────────────────┘
```

规则：

- 前端只与 `pty-core` 交互，不感知后端（运行时选择，无前端条件编译）。
- `vt-parser`、`term-input`、`pty-core` 零平台依赖，可在任何目标上测试。
- 禁止循环依赖；禁止模块依赖 `term-app`。

## 2. 数据流

```
子进程 ⇄ PTY 后端 ⇄[读: 后端字节流]⇄ vt-parser.feed()
                                        │
                                   屏幕状态+damage
                                        ▼
                                   term-render-gtk → 窗口
用户键鼠 → term-input → VT 字节 → pty.write()
窗口 resize → pty.resize(cols, rows) → 后端（SIGWINCH / ResizePseudoConsole /
                                          SetConsoleScreenBufferSize）
Ctrl+C → term-input 编码 0x03（或后端信号）→ pty.send_signal / 注入
```

## 3. 后端选择（pty-core）

```
platform == unix?                     → pty-unix
windows && ConPTY 可用(动态探测)?      → pty-conpty
windows && Win10 < 1809 或 ConPTY 失败 → pty-win10-early（内部三路径，见
                                        docs/win10-early-bridge.md）
```

探测方式：对 `CreatePseudoConsole` 等做 `GetProcAddress`；失败返回
`BackendUnavailable`，由 `available_backends()` 依次回退。

## 4. Windows 10 早期桥接（概要，详见 docs/win10-early-bridge.md）

1. **控制台 API 直驱**（原生 cmd/powershell）：隐藏控制台宿主 + DLL 注入拦截
   `*Console*` 导出 → 虚拟屏幕缓冲 → 脏矩形 diff → VT 帧 → 命名管道 → 前端；
   输入反向：前端 VT → 宿主解析 → `WriteConsoleInputW`；信号经命名管道控制帧 +
   `GenerateConsoleCtrlEvent`。所有 API 受 `config/api-whitelist.json` 约束。
2. **Cygwin PTY 适配**（Cygwin/MSYS2 程序）：运行时探测 `cygwin1.dll` PTY 导出。
3. **WinPTY 回退**（仅简单场景）：动态探测 winpty，缺库明确报错。

## 5. 渲染

GTK3 + Cairo：damage 驱动重绘、字形缓存、真彩色（24-bit SGR）、光标、滚动缓冲。
输入延迟目标 <10ms（批量读取 + 帧内合并 diff）。

## 6. IPC 信号映射（概要，详见 docs/ipc-signal-protocol.md）

命名管道帧协议：`[len][type][payload]`；type 包含 `VT_DATA`、`SIG_INT/SIG_TERM/
SIG_WINCH/...`、`RESIZE(cols,rows)`、`PING/PONG`。SIGINT 优先
`GenerateConsoleCtrlEvent(CTRL_C_EVENT, groupId)`，回退控制帧。
