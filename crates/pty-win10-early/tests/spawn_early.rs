#![cfg(windows)]
//! 第一刀直驱（共享隐藏控制台 + 轮询）集成测试。
//! 覆盖：本机 Win11 + CI windows-latest；强制走 pty-win10-early（注册表只挂它）。
//!
//! 每个测试自带 watchdog（`cargo test` 无默认超时，conpty/unix 同款惯例）；
//! 在 **spawn 返回后**启动——排队等全局互斥不占预算（占锁者自带 watchdog
//! 兜底，其线程独立于测试线程）；EarlyPty 全局互斥串行 spawn..close（设计 §3.1）。

use std::sync::Once;
use std::time::{Duration, Instant};

use pty_core::{BackendKind, Pty, Signal, SpawnOptions};

fn watchdog(seconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(seconds));
        eprintln!("watchdog: 超过 {seconds}s，中止（防止挂死误报为超时失败）");
        std::process::abort();
    });
}

/// 只注册 early 后端（`Once` 防并行测试互相 clear 注册表）。
fn spawn(opts: &SpawnOptions) -> Box<dyn Pty> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        pty_core::clear_backends();
        pty_win10_early::register();
    });
    pty_core::spawn(opts).expect("early spawn failed")
}

/// 去掉 CSI/OSC 转义序列后按行——resize 回合会插 `ESC[137X`/`ESC[53H`。
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

/// 读到**全部** needle 出现在去转义文本（大小写不敏感）为止；EOF/超时提前收口。
fn read_until_all(pty: &mut dyn Pty, needles: &[&str], deadline: Duration) -> String {
    let start = Instant::now();
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    while start.elapsed() < deadline {
        let text = strip_ansi(&String::from_utf8_lossy(&out)).to_lowercase();
        if needles.iter().all(|n| text.contains(&n.to_lowercase())) {
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
fn echo_roundtrip_and_exit_code() {
    let opts = SpawnOptions::new("cmd.exe").args(["/c", "echo early-roundtrip-ok"]);
    let mut pty = spawn(&opts);
    watchdog(60);
    assert_eq!(pty.kind(), BackendKind::Win10Early);

    let out = read_until_all(pty.as_mut(), &["early-roundtrip-ok"], Duration::from_secs(20));
    assert!(out.contains("early-roundtrip-ok"), "output: {out:?}");

    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert_eq!(code, Some(0), "output: {out:?}");
}

#[test]
fn resize_applies_and_interactive_shell_echoes() {
    let mut pty = spawn(&SpawnOptions::new("cmd.exe"));
    watchdog(60);
    pty.resize(137, 53).expect("resize");

    pty.write(b"mode con\r").expect("write mode con");
    let out = read_until_all(pty.as_mut(), &["137", "53"], Duration::from_secs(20));
    let plain = strip_ansi(&out);
    assert!(plain.contains("137"), "mode con output: {plain:?}");
    assert!(plain.contains("53"), "mode con output: {plain:?}");

    pty.write(b"exit\r").expect("write exit");
    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert_eq!(code, Some(0), "shell output before exit: {plain:?}");
}

#[test]
fn env_and_cwd_are_applied() {
    let cwd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let opts = SpawnOptions::new("cmd.exe")
        .args(["/c", "echo %WT_STAGE4_VAR%&cd"])
        .cwd(cwd)
        .env("WT_STAGE4_VAR", "stage4-ok");

    let mut pty = spawn(&opts);
    watchdog(60);
    let out = read_until_all(
        pty.as_mut(),
        &["stage4-ok", "pty-win10-early"],
        Duration::from_secs(20),
    );
    assert!(out.contains("stage4-ok"), "env echo: {out:?}");
    assert!(
        out.to_lowercase().contains("pty-win10-early"),
        "cwd echo: {out:?}"
    );
    let _ = pty.close();
}

#[test]
fn interrupt_terminates_running_child() {
    let opts = SpawnOptions::new("cmd.exe").args(["/c", "ping -n 60 127.0.0.1 >nul"]);
    let mut pty = spawn(&opts);
    watchdog(60);
    // 让 cmd 进入命令执行阶段（输入队列会缓存注入，睡眠只为时序稳妥）
    std::thread::sleep(Duration::from_millis(500));
    pty.send_signal(Signal::Interrupt).expect("interrupt");
    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert!(code.is_some(), "child should exit after Ctrl+C");
    pty.close().expect("close");
}

#[test]
fn kill_then_close_is_idempotent() {
    let mut pty = spawn(&SpawnOptions::new("cmd.exe"));
    watchdog(60);
    pty.send_signal(Signal::Kill).expect("kill");
    let code = wait_exit(pty.as_mut(), Duration::from_secs(20));
    assert!(code.is_some(), "killed child must be reaped");

    pty.close().expect("close 1");
    pty.close().expect("close 2 (idempotent)");
    drop(pty); // Drop → close（已关闭，幂等）
}
