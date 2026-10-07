#![cfg(windows)]
//! 探针：独立进程验证 GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) 是否送达
//! 同控制台子进程（与 spawn_early 隔离：不同测试二进制 = 不同进程/控制台）。

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use pty_win10_early::{hide_console_window, minimize_console_host_window};

extern "system" {
    fn FreeConsole() -> i32;
    fn AllocConsole() -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn GenerateConsoleCtrlEvent(ctrl_event: u32, process_group_id: u32) -> i32;
}

#[test]
fn probe_generate_ctrl_c_terminates_child() {
    // AllocConsole 前启动宿主窗口守卫（WT 委派弹窗 → 最小化）
    minimize_console_host_window();
    unsafe {
        FreeConsole();
        assert_ne!(AllocConsole(), 0, "AllocConsole");
    }
    // 经典 conhost 窗口隐藏/最小化（无委派场景）
    hide_console_window();
    unsafe {
        // 子进程继承“忽略 Ctrl+C”属性 → 必须 spawn 前清掉
        assert_ne!(SetConsoleCtrlHandler(None, 0), 0, "clear inherited ignore");
    }

    let mut child = Command::new("cmd")
        .args(["/c", "ping -n 60 127.0.0.1 >nul"])
        .creation_flags(0) // 默认进程组
        .spawn()
        .expect("spawn cmd");
    let pid = child.id();
    // 子进程已生成 → 现在才忽略宿主侧 Ctrl+C（否则广播自杀）
    unsafe {
        assert_ne!(SetConsoleCtrlHandler(None, 1), 0, "ignore ctrl+c");
    }
    std::thread::sleep(Duration::from_millis(500));

    let gen = unsafe { GenerateConsoleCtrlEvent(0, 0) };
    eprintln!("probe: GenerateConsoleCtrlEvent = {gen} (pid={pid})");

    let start = Instant::now();
    let exited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(10) {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("try_wait: {e}"),
        }
    };
    assert!(exited.is_some(), "child must exit after Ctrl+C broadcast");
}
