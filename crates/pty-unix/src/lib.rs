//! pty-unix — POSIX 伪终端后端（Linux/macOS），Windows 下为可编译空壳。
//!
//! 平台实现：
//! - `posix_openpt` / `grantpt` / `unlockpt` 分配 PTY 对
//! - `fork` 后子进程 `setsid` + `TIOCSCTTY` 取得控制终端，`dup2` 到 0/1/2
//! - `execvp` 启动程序（PATH 搜索），`chdir`/`setenv` 应用 SpawnOptions
//! - 主端 `TIOCSWINSZ` resize；`kill(-pid, sig)` 向子进程组发信号
//! - 读到 `EIO`（全部从端关闭）视作 EOF
//!
//! Windows 端真正后端由阶段 2 的 `pty-conpty` / `pty-win10-early` 提供；
//! 本 crate 的 `register()` 在非 unix 平台是空操作。

#[cfg(unix)]
mod imp;

#[cfg(unix)]
pub use imp::{is_available, register};

#[cfg(not(unix))]
/// Windows/非 POSIX：无可用后端。
pub fn is_available() -> bool {
    false
}

#[cfg(not(unix))]
/// Windows/非 POSIX：不注册任何后端（占位，保持调用方代码可编译）。
pub fn register() {}
