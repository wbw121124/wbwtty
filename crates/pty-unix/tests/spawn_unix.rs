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

#[test]
fn is_available_on_posix() {
    assert!(pty_unix::is_available());
}

#[test]
fn spawn_echo_banner_and_eof() {
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
    pty_unix::register();
    // cwd=/tmp、env FOO=bar、读一行后回显
    let mut pty = spawn(
        &SpawnOptions::new("/bin/sh")
            .args(["-c", r#"printf 'OUT:%s:%s' "$PWD" "$FOO"; read x; printf '|got:%s' "$x""#])
            .cwd("/tmp")
            .env("FOO", "bar"),
    )
    .expect("spawn");
    let out = read_until(pty.as_mut(), "|got:");
    assert!(out.contains("OUT:/tmp:bar"), "got {out:?}");

    pty.write(b"hi\n").expect("write");
    let out2 = read_until(pty.as_mut(), "got:hi");
    assert!(out2.contains("got:hi"), "got {out2:?}");
    let _ = pty.close();
}

#[test]
fn resize_and_signal_term() {
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
