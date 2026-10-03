//! 管道 MVP：匿名管道启动 python 输出 VT 序列 -> vt-parser -> 断言屏幕状态。
//!
//! 运行：`cargo run --example pipe_mvp`（CI job `pipe-mvp` 与 `scripts/ci-local.ps1` 同此）。
//! 不依赖 PTY：本阶段验证“字节流 -> 解析 -> 屏幕”链路；PTY 后端在阶段 1 的 pty-* 提供。

use std::io::Read;
use std::process::{Command, Stdio};

use vt_parser::{Attrs, Color, Terminal, TerminalOptions};

/// python 脚本：通过二进制 stdout 精确写出 VT 序列（避免平台换行翻译）。
const SCRIPT: &str = r#"
import sys
w = sys.stdout.buffer.write
w(b"\x1b[2J\x1b[H")                       # 清屏 + 回原点
w(b"\x1b[1;32mREADY\x1b[0m\r\n")          # 绿色 READY（行1）
w(b"line two\r\n")                        # 普通文本（行2）
w(b"\x1b[3;5H\x1b[1;31mX")                # 行3 列5 起：加粗红 X
w(b"\x1b[0m\r\n\r\n")
w(b"\x1b[5;2H\xe4\xb8\xad\xe6\x96\x87")    # 行5 列2：中文（宽字符）
w(b"\r\n")
w(b"\x1b[6;2H\x1b[38;2;10;20;30mRGB")     # 行6 列2：真彩色 RGB
w(b"\x1b[0m")
sys.stdout.buffer.flush()
"#;

fn fail(msg: &str, term: &Terminal) -> ! {
    eprintln!("[pipe-mvp] FAIL: {}", msg);
    let v = term.get_screen();
    eprintln!(
        "[pipe-mvp] screen {}x{}, cursor ({}, {})",
        v.cols, v.rows, v.cursor.x, v.cursor.y
    );
    for y in 0..v.rows {
        eprintln!("[pipe-mvp] row{:2}: {:?}", y, v.line_text(y));
    }
    std::process::exit(1);
}

fn find_python() -> Option<String> {
    for cand in ["python", "python3", "py"] {
        let st = Command::new(cand)
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if matches!(st, Ok(s) if s.success()) {
            return Some(cand.to_string());
        }
    }
    None
}

fn main() {
    let python = match find_python() {
        Some(p) => {
            println!("[pipe-mvp] python: {}", p);
            p
        }
        None => {
            eprintln!("[pipe-mvp] FAIL: no python interpreter in PATH");
            std::process::exit(1);
        }
    };

    // 匿名管道：stdout/stderr 管道（std::process 自动创建匿名管道对）
    let mut child = match Command::new(&python)
        .arg("-c")
        .arg(SCRIPT)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[pipe-mvp] FAIL: spawn: {}", e);
            std::process::exit(1);
        }
    };

    let mut out = Vec::new();
    if let Some(mut so) = child.stdout.take() {
        if let Err(e) = so.read_to_end(&mut out) {
            eprintln!("[pipe-mvp] FAIL: read: {}", e);
            std::process::exit(1);
        }
    }
    let mut err_buf = String::new();
    if let Some(mut se) = child.stderr.take() {
        let _ = se.read_to_string(&mut err_buf);
    }
    let status = child.wait().expect("wait child");
    if !status.success() {
        eprintln!("[pipe-mvp] FAIL: child exit {:?}, stderr: {}", status.code(), err_buf.trim());
        std::process::exit(1);
    }
    println!("[pipe-mvp] captured {} bytes from child", out.len());

    let mut term = Terminal::with_options(80, 24, TerminalOptions::default());
    term.feed(&out);

    let v = term.get_screen();

    // 1) 行1：绿色加粗 READY
    if v.line_text(0) != "READY" {
        fail("line 1 should be READY", &term);
    }
    let ready = v.cell(0, 0).unwrap();
    if ready.fg != Color::Indexed(2) || !ready.attrs.contains(Attrs::BOLD) {
        fail("READY should be bold green (SGR 1;32)", &term);
    }

    // 2) 行2：普通文本
    if v.line_text(1) != "line two" {
        fail("line 2 should be 'line two'", &term);
    }

    // 3) 行3 列5：加粗红 X
    let x = v.cell(4, 2).unwrap();
    if x.ch != 'X' || x.fg != Color::Indexed(1) || !x.attrs.contains(Attrs::BOLD) {
        fail("cell (4,2) should be bold red X (CUP 3;5)", &term);
    }

    // 4) 行5 列2：宽字符中文（占两格）
    let c1 = v.cell(1, 4).unwrap();
    let c2 = v.cell(2, 4).unwrap();
    if c1.ch != '\u{4e2d}' || !c2.is_continuation() {
        fail("cell (1,4) should be wide CJK with continuation", &term);
    }

    // 5) 行6 列2：24-bit 真彩色
    let rgb = v.cell(1, 5).unwrap();
    if rgb.ch != 'R' || rgb.fg != Color::Rgb(10, 20, 30) {
        fail("cell (1,5) should be R with truecolor fg", &term);
    }

    // 6) damage 应覆盖改动行，且 clear 后为空
    let damaged = v.damage.clone();
    if !damaged.contains(&0) || !damaged.contains(&5) {
        fail("damage should cover rows 0 and 5", &term);
    }
    term.clear_damage();
    if !term.get_screen().damage.is_empty() {
        fail("damage should be empty after clear", &term);
    }

    println!("[pipe-mvp] PASS: VT pipeline produced the expected screen state");
}
