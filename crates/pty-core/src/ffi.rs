//! C ABI — 对应仓库根 `include/pty_core.h`。
//!
//! 约定：函数返回 `0` 成功、`-1` 失败（`pty_read` 例外：`>0` 字节数、`0` EOF、`-1` 失败）；
//! 错误详情由 [`pty_last_error`]（线程局部）返回。句柄由 `pty_spawn` 分配、`pty_close` 释放。

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::slice;

use crate::{Pty, SpawnOptions, Signal};

thread_local! {
    static LAST_ERR: RefCell<Option<CString>> = const { RefCell::new(None) };
    static CHILD_PID: RefCell<u32> = const { RefCell::new(0) };
}

fn set_err(msg: impl Into<String>) {
    let s = msg.into();
    let c = CString::new(s.replace('\0', "?")).unwrap_or_default();
    LAST_ERR.with(|e| *e.borrow_mut() = Some(c));
}

fn clear_err() {
    LAST_ERR.with(|e| *e.borrow_mut() = None);
}

/// 返回最近一次失败的描述（线程局部；成功调用后内容未定义）。
#[no_mangle]
pub extern "C" fn pty_last_error() -> *const c_char {
    LAST_ERR.with(|e| match e.borrow().as_ref() {
        Some(c) => c.as_ptr(),
        None => b"\0".as_ptr() as *const c_char,
    })
}

/// 不透明句柄（C 侧 `typedef struct pty_handle pty_handle;`）。
#[repr(C)]
pub struct PtyHandle {
    pty: Box<dyn Pty>,
}

const MAX_ARGV: usize = 256;

/// 启动 PTY 会话。`argv` 为 NULL 结尾的参数数组（`argv[0]` = 程序名），
/// `cwd` 可为 NULL（继承），`cols/rows` 为初始窗口尺寸。
#[no_mangle]
pub unsafe extern "C" fn pty_spawn(
    argv: *const *const c_char,
    cwd: *const c_char,
    cols: u16,
    rows: u16,
    out: *mut *mut PtyHandle,
) -> c_int {
    if argv.is_null() || out.is_null() {
        set_err("pty_spawn: null argv/out");
        return -1;
    }
    clear_err();

    let mut words: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < MAX_ARGV {
        let p = *argv.add(i);
        if p.is_null() {
            break;
        }
        match CStr::from_ptr(p).to_str() {
            Ok(s) => words.push(s.to_string()),
            Err(_) => {
                set_err("pty_spawn: argv not valid UTF-8");
                return -1;
            }
        }
        i += 1;
    }
    if words.is_empty() {
        set_err("pty_spawn: empty argv");
        return -1;
    }
    if i == MAX_ARGV && !(*argv.add(MAX_ARGV)).is_null() {
        set_err("pty_spawn: argv too long");
        return -1;
    }

    let mut opts = SpawnOptions::new(words[0].clone()).args(words[1..].iter().cloned()).size(cols, rows);
    if !cwd.is_null() {
        let s = match CStr::from_ptr(cwd).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_err("pty_spawn: cwd not valid UTF-8");
                return -1;
            }
        };
        opts = opts.cwd(s);
    }

    match crate::spawn(&opts) {
        Ok(pty) => {
            CHILD_PID.with(|p| *p.borrow_mut() = pty.child_id());
            let h = Box::new(PtyHandle { pty });
            *out = Box::into_raw(h);
            0
        }
        Err(e) => {
            set_err(format!("pty_spawn: {e}"));
            -1
        }
    }
}

/// 读取最多 `len` 字节。返回 `>0` 读取数、`0` EOF、`-1` 失败。
#[no_mangle]
pub unsafe extern "C" fn pty_read(h: *mut PtyHandle, buf: *mut u8, len: usize) -> i64 {
    if h.is_null() || buf.is_null() {
        set_err("pty_read: null handle/buf");
        return -1;
    }
    match (*h).pty.read(slice::from_raw_parts_mut(buf, len)) {
        Ok(n) => n as i64,
        Err(e) => {
            set_err(format!("pty_read: {e}"));
            -1
        }
    }
}

/// 写入 `len` 字节；实际写入数写入 `written`（可为 NULL）。
#[no_mangle]
pub unsafe extern "C" fn pty_write(
    h: *mut PtyHandle,
    buf: *const u8,
    len: usize,
    written: *mut usize,
) -> c_int {
    if h.is_null() || (buf.is_null() && len > 0) {
        set_err("pty_write: null handle/buf");
        return -1;
    }
    let data = slice::from_raw_parts(buf, len);
    match (*h).pty.write(data) {
        Ok(n) => {
            if !written.is_null() {
                *written = n;
            }
            0
        }
        Err(e) => {
            set_err(format!("pty_write: {e}"));
            -1
        }
    }
}

/// 调整窗口尺寸。
#[no_mangle]
pub unsafe extern "C" fn pty_resize(h: *mut PtyHandle, cols: u16, rows: u16) -> c_int {
    if h.is_null() {
        set_err("pty_resize: null handle");
        return -1;
    }
    match (*h).pty.resize(cols, rows) {
        Ok(()) => 0,
        Err(e) => {
            set_err(format!("pty_resize: {e}"));
            -1
        }
    }
}

/// 向子进程组发送信号（见 `pty_signal_t`）。
#[no_mangle]
pub unsafe extern "C" fn pty_signal(h: *mut PtyHandle, sig: c_int) -> c_int {
    if h.is_null() {
        set_err("pty_signal: null handle");
        return -1;
    }
    let s = match sig {
        0 => Signal::Interrupt,
        1 => Signal::Term,
        2 => Signal::Quit,
        3 => Signal::Hangup,
        4 => Signal::Kill,
        _ => {
            set_err("pty_signal: unknown signal id");
            return -1;
        }
    };
    match (*h).pty.send_signal(s) {
        Ok(()) => 0,
        Err(e) => {
            set_err(format!("pty_signal: {e}"));
            -1
        }
    }
}

/// 关闭会话并释放句柄（幂等要求由实现方 Drop 保证；此函数后句柄失效）。
#[no_mangle]
pub unsafe extern "C" fn pty_close(h: *mut PtyHandle) -> c_int {
    if h.is_null() {
        set_err("pty_close: null handle");
        return -1;
    }
    let mut boxed = Box::from_raw(h);
    let r = boxed.pty.close();
    drop(boxed);
    match r {
        Ok(()) => 0,
        Err(e) => {
            set_err(format!("pty_close: {e}"));
            -1
        }
    }
}

/// 返回 `pty_spawn` 启动的子进程 PID（0 = 未知/未启动）。
#[no_mangle]
pub extern "C" fn pty_child_pid() -> u32 {
    CHILD_PID.with(|p| *p.borrow())
}
