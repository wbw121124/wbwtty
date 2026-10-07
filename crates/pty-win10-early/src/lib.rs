//! pty-win10-early — Windows 10 1809 之前（1507–1709）无 ConPTY 时的直驱后端。
//!
//! 要点：
//! - **注册 priority 30**（pty-unix 10、pty-conpty 20）——仅当 ConPTY 动态探测
//!   失败返回 `BackendUnavailable` 时才会被注册表选中兜底
//! - **路径 A 第一刀**：本进程隐藏控制台 + `ReadConsoleOutputW` 轮询 → VT 合成；
//!   第二刀为 conhook.dll 注入（设计见 `docs/win10-early-bridge.md`）
//! - **白名单约束**：全部 Win32 引用在 `config/api-whitelist.json`
//!   （`python tools/check_api_whitelist.py` 强制）
//! - 模块：`imp`（后端主体）/`sys`（Win32 层）/`vt_synth`（屏→VT）/
//!   `input_vt`（VT→输入事件）/`cmdline`（命令行/环境块，复制自 pty-conpty）
//!
//! 使用方调用 [`register`] 把本后端挂进 `pty-core` 注册表。

#[cfg(windows)]
mod cmdline;
#[cfg(windows)]
mod host_win;
#[cfg(windows)]
mod imp;
#[cfg(windows)]
mod input_vt;
#[cfg(windows)]
mod sys;
#[cfg(windows)]
mod vt_synth;

#[cfg(windows)]
pub use host_win::{hide_console_window, minimize_console_host_window};
#[cfg(windows)]
pub use imp::{is_available, register};

#[cfg(not(windows))]
/// 非 Windows：早期桥不可用。
pub fn is_available() -> bool {
    false
}

#[cfg(not(windows))]
/// 非 Windows：不注册任何后端（占位，保持调用方代码可编译）。
pub fn register() {}
