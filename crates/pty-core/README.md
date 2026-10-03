# pty-core

PTY 统一抽象层（零平台实现），最近更新：2026-10-03。

## 职责

| 能力 | API |
| --- | --- |
| 后端实现接口 | `trait Backend`（`kind` / `priority` / `is_available` / `spawn`） |
| 会话操作接口 | `trait Pty`：`read` / `write` / `resize` / `send_signal` / `close` / `try_wait` / `child_id` / `kind` |
| 注册与选择 | `register_backend`、`registered_backends`、`available_backends`、`spawn`、`clear_backends` |
| C ABI | `include/pty_core.h`（与 `src/ffi.rs` 对应） |

## 后端选择规则

1. 按 `priority` 数值升序（小者优先）尝试**可用**后端；
2. 后端返回 `SpawnError::BackendUnavailable` → 回退到下一后端；
3. 其余错误（`Io` / `Spawn`）直接终止回退并上抛；
4. 无可用后端 → `SpawnError::NoBackend`。

约定优先级：`pty-unix` = 10、`pty-conpty` = 20、`pty-win10-early` = 30。
同 kind 重复注册为**替换**（幂等）。

## 信号映射（`Signal` → 平台）

`Interrupt` / `Term` / `Quit` / `Hangup` / `Kill`；Unix 侧映射 SIGINT/SIGTERM/SIGQUIT/SIGHUP/SIGKILL，
Windows 侧映射 `GenerateConsoleCtrlEvent` / `TerminateProcess`（后端文档注明差异）。

## C ABI 约定

- 返回 `0` 成功、`-1` 失败；`pty_read` 例外：`>0` 字节数、`0` EOF、`-1` 失败；
- `pty_spawn` 分配句柄、`pty_close` 释放；错误详情见 `pty_last_error()`（线程局部）；
- `argv` 必须 NULL 结尾（`argv[0]` = 程序名），环境继承父进程。

## 测试

`cargo test -p pty-core`：注册/优先级/回退/替换语义 + C ABI 往返（伪后端，全平台可跑）。
