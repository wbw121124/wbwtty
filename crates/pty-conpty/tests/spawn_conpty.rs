#![cfg(windows)]
//! ConPTY 集成测试（本机 Win10 1809+ / CI windows-latest / CI msys2-ucrt64）。
//!
//! 每个测试都带看门狗：`cargo test` 没有默认超时，卡住会中止进程把“挂死”变成失败。

use std::time::{Duration, Instant};

use pty_core::{BackendKind, Pty, Signal, SpawnOptions};

fn watchdog(seconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(seconds));
        eprintln!("watchdog: 超过 {seconds}s，中止测试以暴露挂死");
        std::process::abort();
    });
}

fn spawn(opts: &SpawnOptions) -> Box<dyn Pty> {
    pty_conpty::register();
    pty_core::spawn(opts).expect("ConPTY spawn failed")
}

/// 读到命中 `needle`、EOF 或错误（阻塞式读，靠看门狗兜底）。
fn read_until(pty: &mut dyn Pty, needle: &str, deadline: Duration) -> String {
    let start = Instant::now();
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    while start.elapsed() < deadline {
        match pty.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&out).contains(needle) {
                    break;
                }
            }
            Err(e) => panic!("read failed: {e}"),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn wait_exit(pty: &mut dyn Pty, deadline: Duration) -> Option<i32> {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if let Ok(Some(code)) = pty.try_wait() {
            return Some(code);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

#[test]
fn spawn_cmd_echo_produces_output_and_exit_code() {
    watchdog(60);
    let opts = SpawnOptions::new("cmd.exe").args(["/c", "echo hello-conpty"]);
    let mut pty = spawn(&opts);
    assert_eq!(pty.kind(), BackendKind::Conpty);

    let out = read_until(pty.as_mut(), "hello-conpty", Duration::from_secs(20));
    assert!(out.contains("hello-conpty"), "output: {out:?}");

    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert_eq!(code, Some(0), "output: {out:?}");
}

#[test]
fn resize_applies_and_interactive_shell_echoes() {
    watchdog(60);
    let mut pty = spawn(&SpawnOptions::new("cmd.exe"));
    pty.resize(137, 53).expect("resize");

    pty.write(b"mode con\r").expect("write mode con");
    let out = read_until(pty.as_mut(), "137", Duration::from_secs(20));
    assert!(out.contains("137"), "mode con output: {out:?}");
    assert!(out.contains("53"), "mode con output: {out:?}");

    pty.write(b"exit\r").expect("write exit");
    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert_eq!(code, Some(0), "shell output before exit: {out:?}");
}

#[test]
fn env_and_cwd_are_applied() {
    watchdog(60);
    let cwd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let opts = SpawnOptions::new("cmd.exe")
        .args(["/c", "echo %WT_STAGE3_VAR%&cd"])
        .cwd(cwd)
        .env("WT_STAGE3_VAR", "stage3-ok");

    let mut pty = spawn(&opts);
    let out = read_until(pty.as_mut(), "stage3-ok", Duration::from_secs(20));
    assert!(out.contains("stage3-ok"), "env echo: {out:?}");
    assert!(
        out.to_lowercase().contains("pty-conpty"),
        "cwd echo: {out:?}"
    );

    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert_eq!(code, Some(0), "output: {out:?}");
}

#[test]
fn interrupt_stops_long_running_child() {
    watchdog(60);
    // ping 需 300 秒才自然结束：若 ETX → Ctrl+C 生效应在数秒内退出
    let opts = SpawnOptions::new("cmd.exe").args(["/c", "ping -n 300 127.0.0.1 >nul"]);
    let mut pty = spawn(&opts);

    std::thread::sleep(Duration::from_millis(800));
    pty.send_signal(Signal::Interrupt).expect("send interrupt");

    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert!(code.is_some(), "child should exit after Ctrl+C");
}

#[test]
fn backend_registers_and_reports_available() {
    pty_conpty::register();
    assert!(pty_conpty::is_available());
    assert!(pty_core::available_backends().contains(&BackendKind::Conpty));
}

#[test]
fn closed_pty_is_idempotent() {
    watchdog(60);
    let mut pty = spawn(&SpawnOptions::new("cmd.exe").args(["/c", "echo bye"]));
    let out = read_until(pty.as_mut(), "bye", Duration::from_secs(20));
    assert!(out.contains("bye"), "output: {out:?}");

    pty.close().expect("close");
    pty.close().expect("close is idempotent");
    assert_eq!(pty.read(&mut [0u8; 16]).expect("read after close"), 0);
    assert!(pty
        .write(b"x")
        .expect_err("write after close")
        .kind()
        == std::io::ErrorKind::BrokenPipe);
}
