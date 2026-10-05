//! pty-conpty — Windows ConPTY 后端（Win10 1809+），其余平台为可编译空壳。
//!
//! 要点：
//! - **动态加载**：`CreatePseudoConsole` / `ResizePseudoConsole` / `ClosePseudoConsole`
//!   属 `dynamic_only` 白名单，运行时 `GetProcAddress` 解析；解析失败（1809 之前）
//!   返回 `SpawnError::BackendUnavailable`，由 `pty-core` 注册表回退到下一后端
//! - **启动**：输入/输出各一根管道 + `EXTENDED_STARTUPINFO_PRESENT` +
//!   `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` 属性把伪控制台交给子进程
//! - **IO**：`PeekNamedPipe` 轮询后才 `ReadFile`（空管道不阻塞、对端关闭即 EOF）
//! - **信号**：`Interrupt` 写 ETX（conhost 解析为 Ctrl+C）；其余映射 `TerminateProcess`
//! - **优先级 20**（`pty-unix` 10、`pty-win10-early` 30），见 pty-core README
//!
//! 使用方调用 [`register`] 把本后端挂进 `pty-core` 注册表。

#[cfg(windows)]
mod cmdline;
#[cfg(windows)]
mod imp;
#[cfg(windows)]
mod sys;

#[cfg(windows)]
pub use imp::{is_available, register};

#[cfg(not(windows))]
/// 非 Windows：ConPTY 不可用。
pub fn is_available() -> bool {
    false
}

#[cfg(not(windows))]
/// 非 Windows：不注册任何后端（占位，保持调用方代码可编译）。
pub fn register() {}
