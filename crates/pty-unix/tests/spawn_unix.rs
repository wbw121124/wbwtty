//! POSIX PTY 集成测试（`cfg(unix)`；Windows/CI windows 不编译）。
#![cfg(unix)]

use std::time::{Duration, Instant};

use pty_core::{spawn, SpawnOptions, Signal};

fn read_until(pty: &mut dyn pty_core::Pty, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut acc = String::new();
    let mut buf = [0u8; 4096];
    while !acc.contains(needle) {
        assert!(Instant::now() < deadline, "timeout waiting for {needle:?}; got {acc:?}");
        match pty.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => acc.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => panic!("read error: {e} (so far {acc:?})"),
        }
    }
    acc
}

/// 阻塞 read 触发不了 read_until 的 deadline 断言 → 超时兜底中止（与 pty-conpty 测试同款）。
fn watchdog(seconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(seconds));
        eprintln!("watchdog: 超过 {seconds}s，中止（测试可能挂死）");
        std::process::abort();
    });
}

#[test]
fn is_available_on_posix() {
    assert!(pty_unix::is_available());
}

#[test]
fn spawn_echo_banner_and_eof() {
    watchdog(30);
    pty_unix::register();
    let mut pty = spawn(&SpawnOptions::new("/bin/sh").args(["-c", "printf 'BANNER:%s' ok"]))
        .expect("spawn sh");
    let out = read_until(pty.as_mut(), "BANNER:ok");
    assert!(out.contains("BANNER:ok"), "got {out:?}");
    // 子进程退出 -> EOF
    let mut buf = [0u8; 16];
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match pty.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        assert!(Instant::now() < deadline, "EOF not reached");
    }
    // 收割
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(code) = pty.try_wait().expect("try_wait") {
            assert_eq!(code, 0, "child should exit 0");
            break;
        }
        assert!(Instant::now() < deadline, "child not reaped");
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = pty.close();
}

#[test]
fn spawn_cwd_env_and_write_roundtrip() {
    watchdog(30);
    pty_unix::register();
    // cwd=/tmp、env FOO=bar、读一行后回显
    let mut pty = spawn(
        &SpawnOptions::new("/bin/sh")
            .args(["-c", r#"printf 'OUT:%s:%s' "$PWD" "$FOO"; read x; printf '|got:%s' "$x""#])
            .cwd("/tmp")
            .env("FOO", "bar"),
    )
    .expect("spawn");
    // 子进程先打印 OUT: 再阻塞等 stdin：必须先读到 OUT: 才能写。
    // 旧顺序（先等 |got: 再写）会让阻塞 read 永远等不到 needle → CI 曾挂死 6h。
    let out = read_until(pty.as_mut(), "OUT:/tmp:bar");
    assert!(out.contains("OUT:/tmp:bar"), "got {out:?}");

    pty.write(b"hi\n").expect("write");
    let out2 = read_until(pty.as_mut(), "got:hi");
    assert!(out2.contains("got:hi"), "got {out2:?}");
    let _ = pty.close();
}

#[test]
fn resize_and_signal_term() {
    watchdog(30);
    pty_unix::register();
    let mut pty = spawn(&SpawnOptions::new("/bin/sh").args(["-c", "sleep 30"]).size(60, 20))
        .expect("spawn");
    pty.resize(100, 30).expect("resize");
    pty.send_signal(Signal::Term).expect("signal");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(code) = pty.try_wait().expect("try_wait") {
            // sh 被 TERM 杀死 -> 128+15；或已转发给 sleep 返回其码
            assert!(code != 0, "child should not exit cleanly, got {code}");
            break;
        }
        assert!(Instant::now() < deadline, "child not reaped after signal");
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = pty.close();
}

#[test]
fn missing_program_exits_127() {
    watchdog(30);
    pty_unix::register();
    let mut pty = spawn(&SpawnOptions::new("definitely-not-a-real-program-xyz"))
        .expect("spawn mechanics succeed; exec failure surfaces as child exit");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(code) = pty.try_wait().expect("try_wait") {
            assert_eq!(code, 127, "execvp failure => 127");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = pty.close();
}
