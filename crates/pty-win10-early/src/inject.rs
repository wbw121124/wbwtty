//! 第二刀宿主端：conhook.dll 远程注入 + 命名管道服务端 + 读线程。
//!
//! 设计依据 `docs/win10-early-bridge.md` §3.6（注入序列）/§6（降级语义）：
//! - **注入**：子进程 `CREATE_SUSPENDED` 创建 → `VirtualAllocEx` 摆 DLL 路径 →
//!   `CreateRemoteThread(LoadLibraryW)` → 校验线程退出码非 0 → `ResumeThread`；
//!   任何一步失败 → 杀挂起子进程、回退纯轮询路径重建（cut 1 行为不变）。
//! - **管道**：宿主先建 `\\.\pipe\wbwtty-early-<hostPid>-<nonce>`（消息模式），
//!   环境变量 `WBWTTY_EARLY_PIPE` 交给 conhook 客户端；恢复后 5s 连不上 →
//!   保留子进程、回退轮询（读线程已起但未连通的，客户端后到时直接断管，
//!   conhook 自行卸载钩子，杜绝双路输出）。
//! - **读线程**：`PeekNamedPipe` 5ms 轮询（句柄同步，避免 overlapped 复杂化）
//!   → `frame::Parser` 拆帧 → [`Sink`] 分发；EXIT 帧 / 子进程死亡 = 正常收尾，
//!   子进程仍活而断管 = 异常（`Sink::broken` → `read()` 报 `BrokenPipe`）。

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::frame::{self, Frame, Parser};
use crate::sys;

/// conhook 读取管道名的环境变量（宿主写入子进程环境块）。
pub const PIPE_ENV: &str = "WBWTTY_EARLY_PIPE";

/// 读线程把帧分发给 imp 侧（`Shared` 队列）的回调面。
pub trait Sink: Send + Sync + 'static {
    /// `VT_DATA`：合成 VT 字节入队
    fn vt(&self, bytes: Vec<u8>);
    /// `EXIT` 帧或子进程死亡：正常收尾（eof）
    fn exit(&self);
    /// 子进程仍活但管道断开/协议错误：`read()` 报 `BrokenPipe`
    fn broken(&self);
}

/// 命名管道服务端（宿主侧；`Arc` 共享给读线程 + resize 发送方）。
///
/// 句柄关闭责任：[`PipeServer::close_now`] 幂等关闭；`Drop` 兜底。
/// 超时路径由宿主主动关管解除读线程的 `ConnectNamedPipe` 阻塞。
pub struct PipeServer {
    h: usize,
    name: String,
    write_lock: Mutex<()>,
    closed: AtomicBool,
}

impl PipeServer {
    /// 创建服务端实例（须在 `CreateProcessW` **之前**调用，保证 conhook
    /// 加载即连）。返回 (服务端, 管道全名)。
    pub fn create(host_pid: u32) -> io::Result<(Arc<Self>, String)> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() ^ ((d.subsec_nanos() as u64) << 32))
            .unwrap_or(0);
        let name = format!(r"\\.\pipe\wbwtty-early-{host_pid}-{stamp:016x}");
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let h = unsafe {
            sys::CreateNamedPipeW(
                wide.as_ptr(),
                sys::PIPE_ACCESS_DUPLEX,
                sys::PIPE_TYPE_MESSAGE | sys::PIPE_READMODE_MESSAGE | sys::PIPE_WAIT,
                1,
                65536,
                65536,
                0,
                ptr::null(),
            )
        };
        if h.is_null() || h as isize == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok((
            Arc::new(Self {
                h: h as usize,
                name: name.clone(),
                write_lock: Mutex::new(()),
                closed: AtomicBool::new(false),
            }),
            name,
        ))
    }

    /// 管道全名（写进环境块的值；诊断/日志备用）。
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 环境变量值（`OsString`，供 `build_environment` 使用）。
    pub fn env_value(&self) -> OsString {
        OsString::from(&self.name)
    }

    /// 发一帧（读线程发 PONG / 主线程发 RESIZE 共用；写锁防交错）。
    pub fn send(&self, typ: u8, payload: &[u8]) -> io::Result<()> {
        let wire = frame::encode(typ, payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut written: sys::DWORD = 0;
        let ok = unsafe {
            sys::WriteFile(
                self.h as sys::HANDLE,
                wire.as_ptr() as sys::LPCVOID,
                wire.len() as sys::DWORD,
                &mut written,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        if written as usize != wire.len() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "short pipe write"));
        }
        Ok(())
    }

    /// 发 `RESIZE` 帧（resize_console 成功之后调用，conhook 据此改虚拟网格）。
    pub fn send_resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        self.send(frame::T_RESIZE, &frame::resize_payload(cols, rows))
    }

    /// 幂等关管（解除对端 connect/read；线程侧后续调用无害）。
    pub fn close_now(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            unsafe { sys::CloseHandle(self.h as sys::HANDLE) };
        }
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        self.close_now();
    }
}

/// 远程注入 conhook.dll：写路径 → `LoadLibraryW` 远程线程 → 校验退出码。
/// 只能在子进程**挂起态**调用（主模块尚未执行，IAT 干净）。
pub fn inject_dll(h_process: usize, dll_path: &Path) -> io::Result<()> {
    let mut wide: Vec<u16> = dll_path.as_os_str().encode_wide().collect();
    wide.push(0);
    let bytes = wide.len() * 2;
    unsafe {
        let h = h_process as sys::HANDLE;
        let remote = sys::VirtualAllocEx(
            h,
            ptr::null_mut(),
            bytes,
            sys::MEM_COMMIT_RESERVE,
            sys::PAGE_READWRITE,
        );
        if remote.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut written: usize = 0;
        let ok = sys::WriteProcessMemory(
            h,
            remote,
            wide.as_ptr() as sys::LPCVOID,
            bytes,
            &mut written,
        );
        if ok == 0 || written != bytes {
            let e = io::Error::last_os_error();
            sys::VirtualFreeEx(h, remote, 0, sys::MEM_RELEASE);
            return Err(e);
        }
        let k32: Vec<u16> = "kernel32.dll".encode_utf16().chain(std::iter::once(0)).collect();
        let k32 = sys::GetModuleHandleW(k32.as_ptr());
        let load = if k32.is_null() {
            ptr::null_mut()
        } else {
            sys::GetProcAddress(k32, b"LoadLibraryW\0".as_ptr())
        };
        let th = if load.is_null() {
            ptr::null_mut()
        } else {
            sys::CreateRemoteThread(h, ptr::null_mut(), 0, load, remote, 0, ptr::null_mut())
        };
        if th.is_null() {
            let e = io::Error::last_os_error();
            sys::VirtualFreeEx(h, remote, 0, sys::MEM_RELEASE);
            return Err(e);
        }
        // 注入线程挂起态（主模块尚未执行）中 LoadLibraryW 通常 <100ms；
        // 加 15s 硬上限定防远程线程死锁/杀软拦截导致 spawn 永久阻塞。
        let wait = sys::WaitForSingleObject(th, 15_000);
        let mut code: sys::DWORD = 0;
        if wait != sys::WAIT_OBJECT_0 {
            // 超时：线程可能仍在跑；强制关句柄并释放 path 内存（child 内部
            // 已 Load 完毕则无泄漏，否则 ~4KB 泄漏可接受——远胜 spawn 永久 hang）
            sys::CloseHandle(th);
            sys::VirtualFreeEx(h, remote, 0, sys::MEM_RELEASE);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "LoadLibraryW remote thread timeout (15s)",
            ));
        }
        sys::GetExitCodeThread(th, &mut code);
        sys::CloseHandle(th);
        sys::VirtualFreeEx(h, remote, 0, sys::MEM_RELEASE);
        if code == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "LoadLibraryW returned NULL in child",
            ));
        }
        Ok(())
    }
}

/// build.rs 注入的 conhook.dll 路径；未构建/文件缺失 → 注入不可用（回退轮询）。
/// mingw 目标下即使 DLL 存在也跳过注入（钩子在某些 mingw 环境下不稳定，
/// 子进程会在第一次 WriteConsoleW 钩子调用后崩溃）。
pub fn hook_dll_path() -> Option<&'static str> {
    // mingw 目标：跳过注入，使用第一刀轮询路径
    if cfg!(not(target_env = "msvc")) {
        return None;
    }
    let p = option_env!("WBWTTY_EARLY_HOOK_DLL")?;
    if p.is_empty() || !Path::new(p).exists() {
        None
    } else {
        Some(p)
    }
}

/// 起读线程：线程内 `ConnectNamedPipe` 阻塞等待，`timeout` 内连上则返回
/// 线程句柄（进入读循环）；超时 → 宿主关管、客户端后到即断、返回 `Err`。
pub fn start_reader(
    server: Arc<PipeServer>,
    sink: Arc<dyn Sink>,
    h_process: usize,
    timeout: Duration,
) -> io::Result<JoinHandle<()>> {
    let (tx, rx) = mpsc::channel::<bool>();
    let t_server = server.clone();
    let th = thread::Builder::new()
        .name("conhook-reader".into())
        .spawn(move || {
            let r = unsafe { sys::ConnectNamedPipe(t_server.h as sys::HANDLE, ptr::null_mut()) };
            let connected = r != 0
                || io::Error::last_os_error().raw_os_error()
                    == Some(sys::ERROR_PIPE_CONNECTED as i32);
            if !connected || t_server.closed.load(Ordering::Acquire) {
                eprintln!("[early-diag] reader connect failed (server closed={})", t_server.closed.load(Ordering::Acquire));
                let _ = tx.send(false);
                return;
            }
            eprintln!("[early-diag] reader connected → starting frame loop");
            let _ = tx.send(true);
            read_loop(&t_server, sink, h_process);
        })?;
    match rx.recv_timeout(timeout) {
        Ok(true) => Ok(th),
        Ok(false) => Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "ConnectNamedPipe failed",
        )),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // 关管解除线程阻塞；客户端后到 → 断管 → conhook 卸钩退出
            server.close_now();
            Err(io::Error::new(io::ErrorKind::TimedOut, "conhook connect timeout"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            server.close_now();
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "reader thread died",
            ))
        }
    }
}

/// 读循环：Peek 5ms 轮询 → 消息读 → 拆帧分发。退出时按状态回调 sink。
fn read_loop(server: &PipeServer, sink: Arc<dyn Sink>, h_process: usize) {
    let h = server.h as sys::HANDLE;
    let mut parser = Parser::new();
    let mut clean = false;
    let mut chunk = vec![0u8; 64 * 1024];
    let mut ticks: u32 = 0;
    let mut vt_count: u32 = 0;
    loop {
        // 每 ~100 次迭代（≤~600ms）探一次子进程：TerminateProcess 跳过
        // DllMain → 不会发 EXIT 帧，对端句柄也可能挂着 → 仅靠断管检测收
        // 不了尾（close 依赖本检查在有限时间内 join 读线程）
        if ticks % 100 == 0
            && unsafe { sys::WaitForSingleObject(h_process as sys::HANDLE, 0) }
                == sys::WAIT_OBJECT_0
        {
            eprintln!("[early-diag] read_loop: child exited → sink.exit()");
            break; // 循环后 alive=false → sink.exit() 正常收尾
        }
        ticks = ticks.wrapping_add(1);
        let mut avail: sys::DWORD = 0;
        let ok = unsafe {
            sys::PeekNamedPipe(h, ptr::null_mut(), 0, ptr::null_mut(), &mut avail, ptr::null_mut())
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            eprintln!(
                "[early-diag] read_loop: Peek failed err={:?} avail_was=? child_alive={}",
                err,
                unsafe { sys::WaitForSingleObject(h_process as sys::HANDLE, 0) } != sys::WAIT_OBJECT_0
            );
            break; // 对端关闭（正常断管）
        }
        if avail == 0 {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let want = (avail as usize).min(chunk.len());
        let mut n: sys::DWORD = 0;
        let ok = unsafe {
            sys::ReadFile(
                h,
                chunk.as_mut_ptr() as sys::LPVOID,
                want as sys::DWORD,
                &mut n,
                ptr::null_mut(),
            )
        };
        if n > 0 {
            match parser.feed(&chunk[..n as usize]) {
                Err(e) => {
                    eprintln!("[early-diag] read_loop: parser error: {e:?} → break");
                    break; // 协议错误（坏 len / 坏已知帧）= 断管语义
                }
                Ok(frames) => {
                    for f in frames {
                        match f {
                            Frame::VtData(b) => {
                                vt_count += 1;
                                eprintln!(
                                    "[early-diag] read_loop: VT_DATA frame #{vt_count} len={}",
                                    b.len()
                                );
                                sink.vt(b);
                            }
                            Frame::Hello { ver, flags } => {
                                eprintln!(
                                    "[early-diag] read_loop: HELLO ver={ver} flags={flags}"
                                );
                            }
                            Frame::Exit { .. } => {
                                eprintln!("[early-diag] read_loop: received EXIT frame → sink.exit()");
                                clean = true;
                                sink.exit();
                            }
                            Frame::Ping => {
                                let _ = server.send(frame::T_PONG, &[]);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if ok == 0
            && io::Error::last_os_error().raw_os_error() != Some(sys::ERROR_MORE_DATA as i32)
        {
            eprintln!("[early-diag] read_loop: ReadFile failed (avail >0 but read err)");
            break;
        }
    }
    if !clean {
        let alive = unsafe { sys::WaitForSingleObject(h_process as sys::HANDLE, 0) }
            != sys::WAIT_OBJECT_0;
        eprintln!(
            "[early-diag] read_loop: ended (clean={clean} vt_frames={vt_count} child_alive={alive})"
        );
        if alive {
            eprintln!("[early-diag] read_loop: → sink.broken()");
            sink.broken();
        } else {
            eprintln!("[early-diag] read_loop: → sink.exit() (post-loop child-dead)");
            sink.exit(); // 子进程被杀（TerminateProcess 跳过 DllMain）→ 正常 eof
        }
    }
}
