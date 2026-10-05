//! 渲染演示窗口：内置 VT 序列动画（颜色/方块移动/局部 damage）。
//!
//! 运行：`cargo run -p term-render-gtk --example demo_window`
//! （PATH 需含 MSYS2 ucrt64 的 bin，`scripts/env.ps1` 已自动处理；关闭窗口退出）

use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use term_render_gtk::{RenderConfig, TerminalView};
use vt_parser::Terminal;

fn frame_bytes(i: u64) -> Vec<u8> {
    let mut s = String::new();
    if i == 0 {
        s.push_str("\x1b[?25l\x1b[2J\x1b[H");
    }
    // 行1：帧计数（256 色轮转）
    let idx = 16 + (i as u8 % 216);
    s.push_str(&format!(
        "\x1b[1;1H\x1b[1mframe \x1b[0m\x1b[38;5;{idx}m{i:06}\x1b[0m   \x1b[7m reverse \x1b[0m"
    ));
    // 行2：真彩色渐变文本
    s.push_str(&format!(
        "\x1b[2;1H\x1b[38;2;{};{};128mtruecolor gradient demo\x1b[0m          ",
        (i * 3) % 256,
        (i * 7) % 256
    ));
    // 行3：下划线/删除线/粗斜体
    s.push_str("\x1b[3;1H\x1b[4munderline\x1b[0m \x1b[9mstrike\x1b[0m \x1b[3;1mitalic\x1b[0m   ");
    // 方块动画：第 5..12 行
    let row = 5 + ((i / 30) % 8);
    let col = 1 + (i % 40);
    // 清掉上一列方块（局部 damage）
    if i > 0 {
        let prev_row = 5 + (((i - 1) / 30) % 8);
        let prev_col = 1 + ((i - 1) % 40);
        s.push_str(&format!("\x1b[{prev_row};{prev_col}H\x1b[0m  "));
    }
    s.push_str(&format!("\x1b[{row};{col}H\x1b[44;97m  \x1b[0m"));
    // 底行状态（宽字符 + 组合符）
    s.push_str("\x1b[24;1H\x1b[36m宽字符 中文abc é 完成\x1b[0m          ");
    s.into_bytes()
}

fn main() {
    gtk::init().expect("gtk::init failed");

    let terminal = Rc::new(RefCell::new(Terminal::new(80, 24)));
    let view = TerminalView::new(terminal.clone(), RenderConfig::default());

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("term-render-gtk demo");
    let m = view.metrics();
    window.set_default_size(80 * m.width as i32, 24 * m.height as i32);
    window.add(view.widget());
    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        gtk::glib::Propagation::Proceed
    });

    {
        let terminal = terminal.clone();
        view.set_on_resize(move |cols, rows| {
            terminal.borrow_mut().resize(cols, rows);
        });
    }

    let frame = Rc::new(Cell::new(0u64));
    {
        let terminal = terminal.clone();
        let frame = frame.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let i = frame.get();
            terminal.borrow_mut().feed(&frame_bytes(i));
            view.queue_redraw();
            frame.set(i + 1);
            glib::ControlFlow::Continue
        });
    }

    window.show_all();
    gtk::main();
}
