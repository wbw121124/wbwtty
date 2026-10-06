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

/// 去掉 CSI/OSC 等转义序列：resize 重绘会产生 `ESC[137X`/`ESC[53H`，
/// 直接在原始字节上匹配会把重绘误当成 `mode con` 的纯文本输出。
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match it.peek().copied() {
            Some('[') => {
                it.next();
                for c2 in it.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c2) {
                        break;
                    }
                }
            }
            Some(']') => {
                it.next();
                while let Some(c2) = it.next() {
                    if c2 == '\u{7}' {
                        break;
                    }
                    if c2 == '\u{1b}' && it.peek() == Some(&'\\') {
                        it.next();
                        break;
                    }
                }
            }
            Some(_) => {
                it.next();
            }
            None => {}
        }
    }
    out
}

/// 读到去转义后的文本命中 `needle`、EOF 或错误（阻塞式读，靠看门狗兜底）。
fn read_until(pty: &mut dyn Pty, needle: &str, deadline: Duration) -> String {
    read_until_all(pty, &[needle], deadline)
}

/// 直到**全部** needle 都在纯文本里出现（或 EOF/超时）。
fn read_until_all(pty: &mut dyn Pty, needles: &[&str], deadline: Duration) -> String {
    let start = Instant::now();
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    while start.elapsed() < deadline {
        let text = strip_ansi(&String::from_utf8_lossy(&out));
        if needles.iter().all(|n| text.contains(n)) {
            break;
        }
        match pty.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
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
    // resize 重绘的 `ESC[137X` 会先于 mode con 文本到达 → 等两个数字都在纯文本里
    let out = read_until_all(pty.as_mut(), &["137", "53"], Duration::from_secs(20));
    let plain = strip_ansi(&out);
    assert!(plain.contains("137"), "mode con output: {plain:?}");
    assert!(plain.contains("53"), "mode con output: {plain:?}");

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
