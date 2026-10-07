//! 第一刀直驱实现：共享隐藏控制台 + 轮询 → VT 合成（阶段 4）。
//!
//! 设计依据：`docs/win10-early-bridge.md` §3.1（模型）/§3.2（输出）/§3.3（输入）/
//! §3.4（resize）/§3.5（信号）/§3.6（生命周期）。要点：
//! - **全局互斥**：Windows 一进程一控制台 → spawn 到 close 持有 `CONSOLE_LOCK`
//! - spawn：`FreeConsole` → `AllocConsole` → 隐藏窗口 → `CONIN$/CONOUT$` 句柄
//!   → 忽略 Ctrl+C → 初始 resize → `CreateProcessW`（**不带** `CREATE_NEW_PROCESS_GROUP`）
//! - 轮询线程：16ms `WaitForSingleObject` + 全屏 `ReadConsoleOutputW` → diff →
//!   VT 入队（`vt_synth`）；子进程退出 → 最终帧 → `eof` → `read` 转 `Ok(0)`
//! - close：杀子进程 → join 轮询 → 关句柄 → 恢复 handler → `FreeConsole` → 放锁

use std::collections::VecDeque;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pty_core::{
    register_backend, Backend, BackendKind, Pty, Signal, SpawnError, SpawnOptions,
};

use crate::cmdline::{build_command_line, build_environment};
use crate::input_vt::VtDecoder;
use crate::sys;
use crate::vt_synth::{self, Screen};

const PRIORITY: u8 = 30;
const POLL: Duration = Duration::from_millis(16);
/// 单帧缓冲上限（防御异常大缓冲的分配）
const MAX_CELLS: usize = 1_048_576;

/// 全进程一个控制台（Windows 语义）→ EarlyPty 全局互斥：持有至 close/Drop。
/// 用 AtomicBool 而非 `Mutex` 守卫——`MutexGuard` 非 `Send`，会破坏 `Pty: Send`。
/// 阻塞语义与互斥锁一致（10ms 自旋等待；死锁由测试 watchdog 兜底）。
static CONSOLE_HELD: AtomicBool = AtomicBool::new(false);

fn acquire_console() {
    while CONSOLE_HELD.swap(true, Ordering::Acquire) {
        thread::sleep(Duration::from_millis(10));
    }
}

fn release_console_lock() {
    CONSOLE_HELD.store(false, Ordering::Release);
}

pub fn is_available() -> bool {
    true
}

pub fn register() {
    register_backend(Arc::new(Win10EarlyBackend));
}

struct Win10EarlyBackend;

impl Backend for Win10EarlyBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Win10Early
    }
    fn priority(&self) -> u8 {
        PRIORITY
    }
    fn is_available(&self) -> bool {
        is_available()
    }
    fn spawn(&self, opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
        EarlyPty::spawn(opts)
    }
}

// ---- 屏幕读取 / 尺寸调整（轮询线程与主线程共用） -------------------------

/// 读取整个屏幕缓冲 + 元信息；失败返回 `None`（轮询线程跳过本帧）。
///
/// # Safety
/// `h_out` 必须是有效的控制台输出句柄（`CONOUT$`）。
unsafe fn read_screen(h_out: sys::HANDLE) -> Option<Screen> {
    let mut info: sys::ConsoleScreenBufferInfo = std::mem::zeroed();
    if sys::GetConsoleScreenBufferInfo(h_out, &mut info) == 0 {
        return None;
    }
    let w = info.dwSize.X as usize;
    let h = info.dwSize.Y as usize;
    if w == 0 || h == 0 || w * h > MAX_CELLS {
        return None;
    }
    let mut cells = vec![
        sys::CharInfo { UnicodeChar: b' ' as u16, Attributes: 0 };
        w * h
    ];
    let mut region = sys::SmallRect {
        Left: 0,
        Top: 0,
        Right: (w - 1) as i16,
        Bottom: (h - 1) as i16,
    };
    let buf_size = sys::Coord { X: w as i16, Y: h as i16 };
    let origin = sys::Coord { X: 0, Y: 0 };
    if sys::ReadConsoleOutputW(h_out, cells.as_mut_ptr(), buf_size, origin, &mut region) == 0 {
        return None;
    }
    Some(Screen {
        w,
        h,
        cells,
        cursor: (info.dwCursorPosition.X as u16, info.dwCursorPosition.Y as u16),
    })
}

/// 窗口/缓冲两段式调整（设计 §3.4：先收窗口/先扩缓冲，任何时刻窗口 ⊆ 缓冲）。
///
/// # Safety
/// `h_out` 须为有效控制台输出句柄。
unsafe fn resize_console(h_out: sys::HANDLE, cols: u16, rows: u16) -> io::Result<()> {
    let mut info: sys::ConsoleScreenBufferInfo = std::mem::zeroed();
    if sys::GetConsoleScreenBufferInfo(h_out, &mut info) == 0 {
        return Err(io::Error::last_os_error());
    }
    let (cur_w, cur_h) = (info.dwSize.X, info.dwSize.Y);
    let target_w = cols.max(1) as i16;
    let target_h = rows.max(1) as i16;

    let set_window = |w: i16, h: i16| -> io::Result<()> {
        let rect = sys::SmallRect { Left: 0, Top: 0, Right: w - 1, Bottom: h - 1 };
        if sys::SetConsoleWindowInfo(h_out, 1, &rect) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    };

    // 1) 窗口收到 min(target, current)（收缩方向先把窗口降下来）
    set_window(target_w.min(cur_w), target_h.min(cur_h))?;
    // 2) 缓冲 → target（相等则跳过）
    if (target_w, target_h) != (cur_w, cur_h) {
        let size = sys::Coord { X: target_w, Y: target_h };
        if sys::SetConsoleScreenBufferSize(h_out, size) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // 3) 窗口扩到 target（收缩方向此步幂等重发）
    set_window(target_w, target_h)
}

/// 打开 `CONIN$`/`CONOUT$`（不依赖 `GetStdHandle`，白名单无此函数）。
/// **可继承**——`STARTF_USESTDHANDLES` 把这两个句柄交给子进程当标准句柄。
unsafe fn open_console(name: &str) -> io::Result<usize> {
    let w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let sa = sys::SecurityAttributes {
        nLength: std::mem::size_of::<sys::SecurityAttributes>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let h = sys::CreateFileW(
        w.as_ptr(),
        sys::GENERIC_READ | sys::GENERIC_WRITE,
        sys::FILE_SHARE_READ | sys::FILE_SHARE_WRITE,
        &sa,
        sys::OPEN_EXISTING,
        0,
        ptr::null_mut(),
    );
    if h.is_null() || h as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(h as usize)
}

/// `STARTUPINFOEXW.lpAttributeList` RAII：把子进程句柄继承范围钉死为
/// `handles`（否则 `bInheritHandles=1` 会把宿主/runner 的全部可继承句柄
/// 复制进子进程——长命子进程会拖住 runner 日志管道的 EOF）。
struct HandleAttrList {
    /// 属性列表 backing store：必须活到 `CreateProcessW` 之后（Drop 释放）；
    /// 逻辑上只经 `ptr` 使用，故允许“未读取”
    #[allow(dead_code)]
    buf: Vec<usize>,
    /// `UpdateProcThreadAttribute(lpValue=...)` 指向的句柄数组。MSDN 仅对
    /// 部分属性（如 IDEAL_PROCESSOR）注明“值须存活到列表销毁”，HANDLE_LIST
    /// 未承诺拷贝——数组必须活过 `CreateProcessW`，否则属性列表读到已释放
    /// 堆（Windows 表现为 CreateProcessW ERROR_INVALID_PARAMETER=87）。
    #[allow(dead_code)] // 仅保活：读取方是 CreateProcessW（经属性列表）
    arr: Vec<sys::HANDLE>,
    ptr: sys::LPVOID,
}

impl HandleAttrList {
    fn new(handles: &[sys::HANDLE]) -> io::Result<Self> {
        let mut size: usize = 0;
        // 首次调用传 NULL 探测大小（返回 FALSE + ERROR_INSUFFICIENT_BUFFER 是预期）
        unsafe {
            sys::InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
        }
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let words = size.div_ceil(std::mem::size_of::<usize>());
        let mut buf = vec![0usize; words];
        let p = buf.as_mut_ptr() as sys::LPVOID;
        if unsafe { sys::InitializeProcThreadAttributeList(p, 1, 0, &mut size) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let cb = handles.len() * std::mem::size_of::<sys::HANDLE>();
        let mut arr = handles.to_vec();
        if unsafe {
            sys::UpdateProcThreadAttribute(
                p,
                0,
                sys::PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                arr.as_mut_ptr() as sys::LPVOID,
                cb,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } == 0
        {
            let e = io::Error::last_os_error();
            unsafe { sys::DeleteProcThreadAttributeList(p) };
            return Err(e);
        }
        // 注：`arr` 随 Self 活到 `CreateProcessW` 之后（见结构体字段注释——
        // HANDLE_LIST 值的拷贝语义未见文档承诺，不可提前释放）
        Ok(Self { buf, arr, ptr: p })
    }
}

impl Drop for HandleAttrList {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { sys::DeleteProcThreadAttributeList(self.ptr) };
        }
    }
}

/// 宿主原控制台身份：spawn 前记录（可能附着也可能无），close/失败时恢复。
/// - 有同伴进程 → `AttachConsole(同伴 pid)` 精确挂回原控制台；
/// - 曾有控制台但自己独占（FreeConsole 已销毁）→ 尽力 `AttachConsole(父)`；
/// - 原本无控制台 → 恢复=什么都不做（保持无控制台）。
struct OriginConsole {
    had_console: bool,
    peer: Option<u32>,
}

impl OriginConsole {
    fn capture() -> Self {
        let mut pids = [0u32; 64];
        // 无控制台时返回 0（失败）→ had_console=false
        let n = unsafe { sys::GetConsoleProcessList(pids.as_mut_ptr(), 64) };
        if n == 0 {
            return Self { had_console: false, peer: None };
        }
        let me = std::process::id();
        let peer = pids[..n.min(64) as usize]
            .iter()
            .copied()
            .find(|&p| p != me && p != 0);
        Self { had_console: true, peer }
    }

    /// 恢复：须在 `FreeConsole` 之后调用（AttachConsole 要求当前无控制台）。
    fn restore(&self) {
        if !self.had_console {
            return;
        }
        let target = self.peer.unwrap_or(sys::ATTACH_PARENT_PROCESS);
        unsafe {
            sys::AttachConsole(target);
        }
    }
}

/// 终止子进程（幂等：已退出视为成功）。
fn terminate_child(h_process: usize) -> io::Result<()> {
    unsafe {
        let h = h_process as sys::HANDLE;
        let mut code: sys::DWORD = 0;
        if sys::GetExitCodeProcess(h, &mut code) != 0 && code != sys::STILL_ACTIVE {
            return Ok(());
        }
        if sys::TerminateProcess(h, 1) != 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if sys::GetExitCodeProcess(h, &mut code) != 0 && code != sys::STILL_ACTIVE {
            return Ok(());
        }
        Err(err)
    }
}

// ---- 输出队列（轮询线程 → read()） ---------------------------------------

struct QueueState {
    bytes: VecDeque<u8>,
    eof: bool,
}

struct Shared {
    q: Mutex<QueueState>,
    cv: Condvar,
}

impl Shared {
    fn new() -> Self {
        Self {
            q: Mutex::new(QueueState { bytes: VecDeque::new(), eof: false }),
            cv: Condvar::new(),
        }
    }
    fn push(&self, vt: Vec<u8>) {
        let mut q = self.q.lock().unwrap_or_else(|e| e.into_inner());
        q.bytes.extend(vt);
        drop(q);
        self.cv.notify_all();
    }
    fn finish(&self) {
        let mut q = self.q.lock().unwrap_or_else(|e| e.into_inner());
        q.eof = true;
        drop(q);
        self.cv.notify_all();
    }
}

/// 轮询线程：子进程存活期间 16ms 全屏 diff；退出后补最终帧 + `eof`。
fn poll_loop(h_out: usize, h_process: usize, shared: Arc<Shared>) {
    let mut prev: Option<Screen> = None;
    loop {
        let timeout = POLL.as_millis() as sys::DWORD;
        unsafe {
            sys::WaitForSingleObject(h_process as sys::HANDLE, timeout);
            let mut code: sys::DWORD = 0;
            sys::GetExitCodeProcess(h_process as sys::HANDLE, &mut code);
            let alive = code == sys::STILL_ACTIVE;

            if let Some(screen) = read_screen(h_out as sys::HANDLE) {
                let vt = vt_synth::synth(prev.as_ref(), &screen);
                if !vt.is_empty() {
                    shared.push(vt);
                }
                prev = Some(screen);
            }
            if !alive {
                break;
            }
        }
    }
    shared.finish();
}

// ---- 后端主体 -------------------------------------------------------------

struct EarlyPty {
    h_in: usize,
    h_out: usize,
    h_process: usize,
    pid: u32,
    shared: Arc<Shared>,
    poll: Option<JoinHandle<()>>,
    decoder: VtDecoder,
    /// spawn 期间 `SetConsoleCtrlHandler(NULL, TRUE)` 是否生效（close 恢复）
    handler_ignore: bool,
    /// 是否持有全局控制台互斥（close/Drop 释放）
    console_acquired: bool,
    /// 宿主原控制台身份（close 时 FreeConsole 后挂回）
    origin: OriginConsole,
    /// 已收割的退出码（close 后句柄已关，避免句柄复用误判）
    exit: Option<i32>,
    closed: bool,
}

impl EarlyPty {
    fn spawn(opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
        acquire_console();

        // 0) 先启动宿主窗口守卫：AllocConsole 在 Win11 默认终端=WT 时弹出的
        //    CASCADIA 窗口只能事后最小化（ShowWindow 拿不到它），守卫须先于
        //    AllocConsole 取快照
        crate::host_win::minimize_console_host_window();

        // 1) 记录宿主原控制台身份（close/失败恢复用）→ 换成干净自有控制台
        let origin = OriginConsole::capture();
        unsafe {
            sys::FreeConsole();
            if sys::AllocConsole() == 0 {
                let e = io::Error::last_os_error();
                origin.restore(); // 原控制台还没被销毁——赶紧挂回去
                release_console_lock();
                return Err(SpawnError::Spawn(format!("AllocConsole failed: {e}")));
            }
        }
        // 经典 conhost 窗口隐藏（无委派场景）；隐藏失效则最小化兜底
        crate::host_win::hide_console_window();

        // 2) CONIN$/CONOUT$ 句柄
        let h_in = match unsafe { open_console("CONIN$") } {
            Ok(v) => v,
            Err(e) => {
                unsafe { sys::FreeConsole() };
                origin.restore();
                release_console_lock();
                return Err(SpawnError::Spawn(format!("open CONIN$: {e}")));
            }
        };
        let h_out = match unsafe { open_console("CONOUT$") } {
            Ok(v) => v,
            Err(e) => {
                unsafe {
                    sys::CloseHandle(h_in as sys::HANDLE);
                    sys::FreeConsole();
                }
                origin.restore();
                release_console_lock();
                return Err(SpawnError::Spawn(format!("open CONOUT$: {e}")));
            }
        };

        // 3) “忽略 Ctrl+C”属性会被子进程继承（MSDN SetConsoleCtrlHandler）——
        //    spawn 前必须清掉，否则 Interrupt 广播对子进程是 no-op
        //    （阶段 3 同根因；宿主侧置 TRUE 移到 CreateProcessW 成功之后）
        if unsafe { sys::SetConsoleCtrlHandler(None, 0) } == 0 {
            release_console(h_in, h_out, false, &origin);
            return Err(SpawnError::Spawn(format!(
                "SetConsoleCtrlHandler(clear): {}",
                io::Error::last_os_error()
            )));
        }

        // 4) 初始尺寸
        if let Err(e) = unsafe { resize_console(h_out as sys::HANDLE, opts.cols, opts.rows) } {
            release_console(h_in, h_out, false, &origin);
            return Err(SpawnError::Spawn(format!("initial resize: {e}")));
        }

        // 5) 子进程（默认进程组——阶段 3 实证不能用 CREATE_NEW_PROCESS_GROUP）；
        //    标准句柄 = 我们的 CONIN$/CONOUT$（可继承），否则 cmd 无输出可言；
        //    属性列表把继承范围钉死在这两个句柄（防 runner 管道泄漏给子进程）
        let mut cmdline = build_command_line(opts);
        let env = build_environment(&opts.env);
        let env_empty = env.is_empty();
        let cwd_wide: Option<Vec<u16>> = opts.cwd.as_ref().map(|p| {
            p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
        });
        let inherit = [h_in as sys::HANDLE, h_out as sys::HANDLE];
        let attr_list = match HandleAttrList::new(&inherit) {
            Ok(v) => v,
            Err(e) => {
                release_console(h_in, h_out, false, &origin);
                return Err(SpawnError::Spawn(format!("attr list: {e}")));
            }
        };
        let mut si: sys::StartUpInfoExW = unsafe { std::mem::zeroed() };
        si.StartupInfo.cb = std::mem::size_of::<sys::StartUpInfoExW>() as u32;
        si.StartupInfo.dwFlags = sys::STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = h_in as sys::HANDLE;
        si.StartupInfo.hStdOutput = h_out as sys::HANDLE;
        si.StartupInfo.hStdError = h_out as sys::HANDLE;
        si.lpAttributeList = attr_list.ptr;
        let mut pi: sys::ProcessInformation = unsafe { std::mem::zeroed() };

        let flags = sys::EXTENDED_STARTUPINFO_PRESENT
            | if env_empty { 0 } else { sys::CREATE_UNICODE_ENVIRONMENT };
        let env_ptr = if env_empty {
            ptr::null()
        } else {
            env.as_ptr() as sys::LPCVOID
        };
        let created = unsafe {
            sys::CreateProcessW(
                ptr::null(),
                cmdline.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1, // 继承可继承句柄（范围由 HANDLE_LIST 属性限定为 CONIN$/CONOUT$）
                flags,
                env_ptr,
                cwd_wide.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
                &mut si.StartupInfo,
                &mut pi,
            )
        };
        // 须在 drop(attr_list)（DeleteProcThreadAttributeList 可能改写线程最后
        // 错误码）之前取错误，否则报出的可能是 Delete 的错误码而非真因
        let create_err = (created == 0).then(|| io::Error::last_os_error());
        drop(attr_list); // CreateProcessW 已消费，立即释放属性列表
        if let Some(e) = create_err {
            release_console(h_in, h_out, false, &origin);
            return Err(SpawnError::Spawn(format!(
                "CreateProcessW: {e} [cmdline={} env_null={env_empty} cwd={} \
                 flags=0x{flags:x} si_cb={} h_in={h_in:#x} h_out={h_out:#x}]",
                String::from_utf16_lossy(&cmdline).trim_end_matches('\0'),
                cwd_wide.is_some(),
                si.StartupInfo.cb,
            )));
        }
        unsafe { sys::CloseHandle(pi.hThread) };

        // 5) 子进程已生成（继承到 ignore=FALSE）→ 现在才给宿主置忽略，
        //    否则 Interrupt 广播会杀死宿主自身；失败即 fail-fast
        if unsafe { sys::SetConsoleCtrlHandler(None, 1) } == 0 {
            let e = io::Error::last_os_error();
            unsafe {
                let _ = terminate_child(pi.hProcess as usize);
                sys::CloseHandle(pi.hProcess);
            }
            release_console(h_in, h_out, false, &origin);
            return Err(SpawnError::Spawn(format!(
                "SetConsoleCtrlHandler(set): {e}"
            )));
        }
        let handler_ignore = true;

        // 6) 轮询线程
        let shared = Arc::new(Shared::new());
        let poll = {
            let s = shared.clone();
            let (ho, hp) = (h_out, pi.hProcess as usize);
            thread::spawn(move || poll_loop(ho, hp, s))
        };

        Ok(Box::new(EarlyPty {
            h_in,
            h_out,
            h_process: pi.hProcess as usize,
            pid: pi.dwProcessId,
            shared,
            poll: Some(poll),
            decoder: VtDecoder::new(),
            handler_ignore,
            console_acquired: true,
            origin,
            exit: None,
            closed: false,
        }))
    }
}

/// spawn 失败路径清理（关句柄 + 恢复 handler + FreeConsole + 恢复原控制台 + 放锁）。
fn release_console(h_in: usize, h_out: usize, handler_ignore: bool, origin: &OriginConsole) {
    unsafe {
        sys::CloseHandle(h_in as sys::HANDLE);
        sys::CloseHandle(h_out as sys::HANDLE);
        if handler_ignore {
            sys::SetConsoleCtrlHandler(None, 0);
        }
        sys::FreeConsole();
    }
    origin.restore();
    release_console_lock();
}

/// Ctrl+C 键事件（down + up），仅布局单测使用（注入路径已被广播取代）。
#[cfg(test)]
fn ctrl_c_records() -> Vec<sys::InputRecord> {
    let mk = |down: bool| sys::InputRecord {
        EventType: sys::KEY_EVENT,
        Event: sys::InputEvent {
            KeyEvent: sys::KeyEventRecord {
                bKeyDown: if down { 1 } else { 0 },
                wRepeatCount: 1,
                wVirtualKey: 0x43, // 'C'
                wVirtualScanCode: 0,
                UnicodeChar: 0x03,
                dwControlKeyState: sys::LEFT_CTRL_PRESSED,
            },
        },
    };
    vec![mk(true), mk(false)]
}

impl Pty for EarlyPty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut q = self.shared.q.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !q.bytes.is_empty() {
                let n = buf.len().min(q.bytes.len());
                for (i, b) in q.bytes.iter().take(n).enumerate() {
                    buf[i] = *b;
                }
                q.bytes.drain(..n);
                return Ok(n);
            }
            if q.eof {
                return Ok(0);
            }
            q = self.shared.cv.wait(q).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed"));
        }
        let records = self.decoder.feed(buf);
        if records.is_empty() {
            return Ok(buf.len()); // 不完整尾部已缓冲 / 未知序列已丢弃
        }
        let mut written: sys::DWORD = 0;
        let ok = unsafe {
            sys::WriteConsoleInputW(
                self.h_in as sys::HANDLE,
                records.as_ptr(),
                records.len() as sys::DWORD,
                &mut written,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(buf.len())
    }

    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "PTY closed"));
        }
        unsafe { resize_console(self.h_out as sys::HANDLE, cols, rows) }
    }

    fn send_signal(&mut self, sig: Signal) -> io::Result<()> {
        if self.closed {
            // close 已释放控制台/句柄——再广播会打到下一个持有者（跨实例污染）
            return Err(io::Error::new(io::ErrorKind::NotConnected, "PTY closed"));
        }
        match sig {
            // 设计 §3.5 修订：WriteConsoleInputW 注入的 Ctrl+C 键事件不经过
            // conhost 急切处理（无人读输入缓冲时不会生成 CTRL_C_EVENT），
            // 故主路径用 GenerateConsoleCtrlEvent(0) 广播——宿主在子进程
            // 生成后、广播前置位忽略，避免自杀。
            Signal::Interrupt | Signal::Quit => {
                if unsafe { sys::GenerateConsoleCtrlEvent(sys::CTRL_C_EVENT, 0) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            }
            Signal::Term | Signal::Hangup | Signal::Kill => {
                terminate_child(self.h_process)
            }
        }
    }

    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;

        // 1) 杀子进程（同时唤醒轮询线程走最终帧）
        let _ = terminate_child(self.h_process);
        // 2) join 轮询（其内部补最终帧 + eof）
        if let Some(h) = self.poll.take() {
            let _ = h.join();
        }
        // 3) 防御：线程异常退出时保证 eof，避免 read 挂死
        self.shared.finish();
        // 4) 收割退出码（关句柄前）+ 关句柄
        unsafe {
            if self.exit.is_none() {
                let mut code: sys::DWORD = 0;
                if sys::GetExitCodeProcess(self.h_process as sys::HANDLE, &mut code) != 0 {
                    self.exit = Some(code as i32);
                }
            }
            sys::CloseHandle(self.h_in as sys::HANDLE);
            sys::CloseHandle(self.h_out as sys::HANDLE);
            sys::CloseHandle(self.h_process as sys::HANDLE);
        }
        // 5) 恢复 Ctrl+C handler + 摘除控制台 + 挂回宿主原控制台
        if self.handler_ignore {
            unsafe { sys::SetConsoleCtrlHandler(None, 0) };
            self.handler_ignore = false;
        }
        unsafe { sys::FreeConsole() };
        self.origin.restore();
        // 6) 释放全局互斥
        if self.console_acquired {
            release_console_lock();
            self.console_acquired = false;
        }
        Ok(())
    }

    fn try_wait(&mut self) -> io::Result<Option<i32>> {
        if let Some(code) = self.exit {
            return Ok(Some(code));
        }
        if self.closed {
            return Ok(None); // 句柄已关且未收割过 → 不拿复用句柄赌状态
        }
        unsafe {
            let h = self.h_process as sys::HANDLE;
            if sys::WaitForSingleObject(h, 0) == sys::WAIT_OBJECT_0 {
                let mut code: sys::DWORD = 0;
                sys::GetExitCodeProcess(h, &mut code);
                self.exit = Some(code as i32);
                Ok(self.exit)
            } else {
                Ok(None)
            }
        }
    }

    fn child_id(&self) -> u32 {
        self.pid
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Win10Early
    }
}

impl Drop for EarlyPty {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

// EarlyPty 全字段（usize/Arc/JoinHandle/VtDecoder/bool/MutexGuard）自动 Send。

#[cfg(test)]
mod tests {
    use pty_core::{clear_backends, registered_backends, BackendKind as K};

    use super::*;

    #[test]
    fn registers_with_priority_30() {
        clear_backends();
        register();
        assert_eq!(registered_backends(), vec![K::Win10Early]);
        assert_eq!(Win10EarlyBackend.priority(), 30);
        clear_backends();
    }

    #[test]
    fn spawn_error_when_console_unavailable_is_spawn_variant() {
        // 结构性检查：spawn 返回类型为 SpawnError（控制台可用时不触发）
        let opts = SpawnOptions::new("cmd");
        match Win10EarlyBackend.spawn(&opts) {
            Err(e) => {
                assert!(matches!(
                    e,
                    SpawnError::Spawn(_) | SpawnError::BackendUnavailable(_)
                ));
            }
            Ok(mut p) => {
                assert_eq!(p.kind(), K::Win10Early);
                p.close().expect("close");
            }
        }
    }

    #[test]
    fn ctrl_c_records_layout() {
        let recs = ctrl_c_records();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].EventType, sys::KEY_EVENT);
        let k = unsafe { recs[0].Event.KeyEvent };
        assert_eq!((k.bKeyDown, k.wVirtualKey, k.UnicodeChar), (1, 0x43, 0x03));
        assert_eq!(k.dwControlKeyState, sys::LEFT_CTRL_PRESSED);
    }
}
