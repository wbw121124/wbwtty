//! ConPTY 实现（仅 `cfg(windows)` 编译）。
//!
//! 启动流程：管道对 → `CreatePseudoConsole` → `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`
//! 属性 → `CreateProcessW`（`EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT`）。
//! 伪控制台一侧的两个管道端在创建后立刻关闭（ConPTY 持有各自副本），
//! 否则输出读端永远等不到 EOF。

use std::io;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use std::time::Duration;

use pty_core::{
    register_backend, Backend, BackendKind, Pty, Signal, SpawnError, SpawnOptions,
};

use crate::cmdline::{build_command_line, build_environment};
use crate::sys;

/// 后端优先级（见 pty-core README：conpty 20、win10-early 30、unix 10）。
const PRIORITY: u8 = 20;
/// 管道空时的轮询间隔（`PeekNamedPipe` → 有数据才 `ReadFile`）。
const POLL: Duration = Duration::from_millis(10);

const IN_READ: usize = 0;
const IN_WRITE: usize = 1;
const OUT_READ: usize = 2;
const OUT_WRITE: usize = 3;

/// ConPTY 导出可解析（Win10 1809+）。
pub fn is_available() -> bool {
    sys::conpty_api().is_some()
}

/// 注册本后端（幂等；同 kind 替换）。
pub fn register() {
    register_backend(Arc::new(ConptyBackend));
}

struct ConptyBackend;

impl Backend for ConptyBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Conpty
    }
    fn priority(&self) -> u8 {
        PRIORITY
    }
    fn is_available(&self) -> bool {
        is_available()
    }
    fn spawn(&self, opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
        spawn_conpty(opts)
    }
}

/// `PROC_THREAD_ATTRIBUTE_LIST` 的 RAII 封装。
///
/// 用 `std::alloc`（对齐 16）分配，避免为 `HeapAlloc`/`GetProcessHeap` 再扩白名单。
struct AttributeList {
    raw: sys::LPVOID,
    layout: std::alloc::Layout,
}

impl AttributeList {
    unsafe fn new(count: sys::DWORD) -> io::Result<Self> {
        let mut size: usize = 0;
        // 第一次调用只为查询大小：返回 FALSE + ERROR_INSUFFICIENT_BUFFER 是预期路径
        let _ = sys::InitializeProcThreadAttributeList(std::ptr::null_mut(), count, 0, &mut size);
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let layout = std::alloc::Layout::from_size_align(size, 16)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let ptr = std::alloc::alloc(layout);
        if ptr.is_null() {
            return Err(io::Error::new(io::ErrorKind::OutOfMemory, "attribute list"));
        }
        if sys::InitializeProcThreadAttributeList(ptr as sys::LPVOID, count, 0, &mut size) == 0 {
            let e = io::Error::last_os_error();
            std::alloc::dealloc(ptr, layout);
            return Err(e);
        }
        Ok(Self { raw: ptr as sys::LPVOID, layout })
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        unsafe {
            sys::DeleteProcThreadAttributeList(self.raw);
            std::alloc::dealloc(self.raw as *mut u8, self.layout);
        }
    }
}

/// 失败路径：关闭伪控制台（若已创建）与父进程仍持有的管道端。
unsafe fn release(api: &'static sys::ConptyApi, hpc: sys::Hpc, h: &mut [sys::HANDLE]) {
    if !hpc.is_null() {
        api.close(hpc);
    }
    for slot in h.iter_mut() {
        if !slot.is_null() {
            sys::CloseHandle(*slot);
            *slot = std::ptr::null_mut();
        }
    }
}

fn spawn_conpty(opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
    spawn_conpty_typed(opts).map(|pty| Box::new(pty) as Box<dyn Pty>)
}

fn spawn_conpty_typed(opts: &SpawnOptions) -> Result<ConptyPty, SpawnError> {
    let api = match sys::conpty_api() {
        Some(a) => a,
        None => return Err(SpawnError::BackendUnavailable(BackendKind::Conpty)),
    };
    if opts.program.is_empty() {
        return Err(SpawnError::Spawn("empty program".into()));
    }

    let mut h: [sys::HANDLE; 4] = [std::ptr::null_mut(); 4];
    let mut hpc: sys::Hpc = std::ptr::null_mut();

    unsafe {
        // 1) 两根管道：in（我们写、ConPTY 读）、out（ConPTY 写、我们读）
        if sys::CreatePipe(&mut h[IN_READ], &mut h[IN_WRITE], std::ptr::null(), 0) == 0 {
            let e = io::Error::last_os_error();
            release(api, hpc, &mut h);
            return Err(e.into());
        }
        if sys::CreatePipe(&mut h[OUT_READ], &mut h[OUT_WRITE], std::ptr::null(), 0) == 0 {
            let e = io::Error::last_os_error();
            release(api, hpc, &mut h);
            return Err(e.into());
        }

        // 2) 伪控制台（列/行夹紧到 COORD 的有符号范围）
        let size = sys::Coord {
            X: opts.cols.clamp(1, i16::MAX as u16) as i16,
            Y: opts.rows.clamp(1, i16::MAX as u16) as i16,
        };
        let hr = api.create(size, h[IN_READ], h[OUT_WRITE], 0, &mut hpc);
        sys::CloseHandle(h[IN_READ]);
        h[IN_READ] = std::ptr::null_mut();
        sys::CloseHandle(h[OUT_WRITE]);
        h[OUT_WRITE] = std::ptr::null_mut();
        if hr < 0 {
            release(api, hpc, &mut h);
            return Err(SpawnError::Spawn(format!(
                "CreatePseudoConsole failed: HRESULT 0x{hr:08X}"
            )));
        }

        // 3) 属性列表：把 HPCON 交给子进程
        let attrs = match AttributeList::new(1) {
            Ok(a) => a,
            Err(e) => {
                release(api, hpc, &mut h);
                return Err(e.into());
            }
        };
        // lpValue 直接是 HPCON 的值本身（MSDN 示例即如此），不是它的地址
        if sys::UpdateProcThreadAttribute(
            attrs.raw,
            0,
            sys::PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            hpc as sys::LPVOID,
            mem::size_of::<sys::Hpc>(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
        {
            let e = io::Error::last_os_error();
            drop(attrs);
            release(api, hpc, &mut h);
            return Err(e.into());
        }

        // 4) 创建子进程（句柄不继承：伪控制台经属性列表传递）
        let mut cmdline = build_command_line(opts);
        let env_block = build_environment(&opts.env);
        let mut startup: sys::StartUpInfoExW = mem::zeroed();
        startup.StartupInfo.cb = mem::size_of::<sys::StartUpInfoExW>() as sys::DWORD;
        // 标准句柄显式置 NULL 并置 STARTF_USESTDHANDLES（microsoft/terminal#4380
        // issuecomment-580865346 官方建议；zhiburt/conpty 同款实现）。
        // 否则子进程（cmd.exe 等）会继承父进程的 std 句柄——测试 harness、
        // CI、调试器下父 stdout 是管道——cmd 直接把输出写进父管道，ConPTY
        // 输出管道 0 字节，read 永久阻塞导致测试挂死。置 NULL 后子进程回落
        // 到伪控制台句柄，输出正常进 ConPTY 管道。
        startup.StartupInfo.dwFlags |= sys::STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = std::ptr::null_mut();
        startup.StartupInfo.hStdOutput = std::ptr::null_mut();
        startup.StartupInfo.hStdError = std::ptr::null_mut();
        startup.lpAttributeList = attrs.raw;
        let mut pi: sys::ProcessInformation = mem::zeroed();
        let cwd_wide = opts.cwd.as_ref().map(|p| {
            p.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        });
        let env_ptr = if env_block.is_empty() {
            std::ptr::null()
        } else {
            env_block.as_ptr() as sys::LPCVOID
        };
        let cwd_ptr = cwd_wide.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());

        let ok = sys::CreateProcessW(
            std::ptr::null(),
            cmdline.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            sys::EXTENDED_STARTUPINFO_PRESENT | sys::CREATE_UNICODE_ENVIRONMENT,
            env_ptr,
            cwd_ptr,
            &mut startup.StartupInfo,
            &mut pi,
        );
        drop(attrs); // 属性列表在 CreateProcess 返回后即可释放
        if ok == 0 {
            let e = io::Error::last_os_error();
            release(api, hpc, &mut h);
            return Err(e.into());
        }
        sys::CloseHandle(pi.hThread);

        // 5) 成功：in_write / out_read 归本会话所有
        Ok(ConptyPty {
            api,
            hpc: hpc as isize,
            input: h[IN_WRITE] as isize,
            output: h[OUT_READ] as isize,
            process: pi.hProcess as isize,
            pid: pi.dwProcessId,
            closed: false,
            exit: None,
        })
    }
}

struct ConptyPty {
    api: &'static sys::ConptyApi,
    /// HPCON（句柄按整数保存，满足 `Pty: Send`）
    hpc: isize,
    /// ConPTY 输入（我们写）
    input: isize,
    /// ConPTY 输出（我们读）
    output: isize,
    process: isize,
    pid: sys::DWORD,
    closed: bool,
    exit: Option<i32>,
}

impl ConptyPty {
    fn hpc_handle(&self) -> sys::Hpc {
        self.hpc as sys::Hpc
    }
    fn input_handle(&self) -> sys::HANDLE {
        self.input as sys::HANDLE
    }
    fn output_handle(&self) -> sys::HANDLE {
        self.output as sys::HANDLE
    }
    fn process_handle(&self) -> sys::HANDLE {
        self.process as sys::HANDLE
    }

    fn exit_code(&mut self) -> io::Result<Option<i32>> {
        if let Some(c) = self.exit {
            return Ok(Some(c));
        }
        let mut code: sys::DWORD = 0;
        if unsafe { sys::GetExitCodeProcess(self.process_handle(), &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if code == sys::STILL_ACTIVE {
            Ok(None)
        } else {
            self.exit = Some(code as i32);
            Ok(self.exit)
        }
    }
}

/// 管道对端已关闭（读端/写端任一侧）→ 读返回 EOF、写返回 BrokenPipe。
fn pipe_closed(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(c)
            if c == sys::ERROR_BROKEN_PIPE
                || c == sys::ERROR_NO_DATA
                || c == sys::ERROR_PIPE_NOT_CONNECTED
    )
}

impl Pty for ConptyPty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.closed || buf.is_empty() {
            return Ok(0);
        }
        unsafe {
            // 先轮询：管道空时不阻塞在 ReadFile 上，ConPTY 关闭后立刻看到 EOF
            loop {
                let mut avail: sys::DWORD = 0;
                let r = sys::PeekNamedPipe(
                    self.output_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut avail,
                    std::ptr::null_mut(),
                );
                if r == 0 {
                    let e = io::Error::last_os_error();
                    if pipe_closed(&e) {
                        return Ok(0);
                    }
                    return Err(e);
                }
                if avail > 0 {
                    break;
                }
                std::thread::sleep(POLL);
            }

            let mut read: sys::DWORD = 0;
            let n = buf.len().min(0xFFFF_FFFF) as sys::DWORD;
            if sys::ReadFile(
                self.output_handle(),
                buf.as_mut_ptr() as sys::LPVOID,
                n,
                &mut read,
                std::ptr::null_mut(),
            ) == 0
            {
                let e = io::Error::last_os_error();
                if pipe_closed(&e) {
                    return Ok(0);
                }
                return Err(e);
            }
            Ok(read as usize)
        }
    }

    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed"));
        }
        if data.is_empty() {
            return Ok(0);
        }
        unsafe {
            let mut total = 0usize;
            while total < data.len() {
                let chunk = &data[total..];
                let mut written: sys::DWORD = 0;
                let n = chunk.len().min(0xFFFF_FFFF) as sys::DWORD;
                if sys::WriteFile(
                    self.input_handle(),
                    chunk.as_ptr() as sys::LPCVOID,
                    n,
                    &mut written,
                    std::ptr::null_mut(),
                ) == 0
                {
                    if total > 0 {
                        break;
                    }
                    let e = io::Error::last_os_error();
                    if pipe_closed(&e) {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "ConPTY input closed",
                        ));
                    }
                    return Err(e);
                }
                if written == 0 {
                    break;
                }
                total += written as usize;
            }
            Ok(total)
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "PTY closed"));
        }
        let size = sys::Coord {
            X: cols.clamp(1, i16::MAX as u16) as i16,
            Y: rows.clamp(1, i16::MAX as u16) as i16,
        };
        let hr = unsafe { self.api.resize(self.hpc_handle(), size) };
        if hr < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("ResizePseudoConsole failed: HRESULT 0x{hr:08X}"),
            ))
        } else {
            Ok(())
        }
    }

    fn send_signal(&mut self, sig: Signal) -> io::Result<()> {
        if self.closed || self.exit.is_some() {
            return Ok(());
        }
        match sig {
            // ConPTY 输入侧把 ETX 解析成 Ctrl+C 键事件，由 conhost 转给客户端
            Signal::Interrupt => {
                self.write(&[0x03])?;
                Ok(())
            }
            // Win32 没有对应信号原语：TerminateProcess 近似 SIGKILL
            Signal::Term | Signal::Quit | Signal::Hangup | Signal::Kill => unsafe {
                if sys::TerminateProcess(self.process_handle(), 1) != 0 {
                    return Ok(());
                }
                let e = io::Error::last_os_error();
                if e.raw_os_error() == Some(sys::ERROR_ACCESS_DENIED) {
                    Ok(()) // 进程已退出
                } else {
                    Err(e)
                }
            },
        }
    }

    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        unsafe {
            // 1) 关伪控制台：conhost 退出，子进程收到控制台关闭事件
            self.api.close(self.hpc_handle());
            // 2) 关管道端：此后读端 EOF、写端 BrokenPipe
            sys::CloseHandle(self.input_handle());
            sys::CloseHandle(self.output_handle());
            // 3) 收割：短暂等待，仍在运行则终止后再等一次
            if self.exit.is_none() {
                if sys::WaitForSingleObject(self.process_handle(), 250) == sys::WAIT_OBJECT_0 {
                    let _ = self.exit_code();
                } else {
                    let _ = sys::TerminateProcess(self.process_handle(), 1);
                    if sys::WaitForSingleObject(self.process_handle(), 1_000)
                        == sys::WAIT_OBJECT_0
                    {
                        let _ = self.exit_code();
                    }
                }
            }
            // 4) 关进程句柄
            sys::CloseHandle(self.process_handle());
            self.process = 0;
        }
        Ok(())
    }

    fn try_wait(&mut self) -> io::Result<Option<i32>> {
        if let Some(c) = self.exit {
            return Ok(Some(c));
        }
        if self.closed {
            return Ok(None); // 句柄已关，退出码未捕获
        }
        self.exit_code()
    }

    fn child_id(&self) -> u32 {
        self.pid
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Conpty
    }
}

impl Drop for ConptyPty {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pty_core::SpawnOptions;

    /// 诊断：子进程输出是否真的落在 ConPTY 输出管道上。
    ///
    /// 管道里通常**先**出现 ConPTY 握手序列（`ESC[?9001h`/`ESC[?1004h`，16 字节），
    /// `cmd` 的回显随后才到；单次 read 只拿到握手就断言 needle 会误报（CI windows 复现），
    /// 所以必须在截止时间内**累积**读到 needle。
    #[test]
    fn probe_output_pipe_receives_child_output() {
        const NEEDLE: &str = "probe-marker";
        let mut pty = match spawn_conpty_typed(
            &SpawnOptions::new("cmd.exe").args(["/c", "echo probe-marker"]),
        ) {
            Ok(b) => b,
            Err(e) => panic!("spawn failed: {e}"),
        };
        let out_handle = pty.output_handle();

        let start = std::time::Instant::now();
        let mut acc: Vec<u8> = Vec::new();
        let mut buf = [0u8; 1024];
        while start.elapsed() < Duration::from_secs(3)
            && !String::from_utf8_lossy(&acc).contains(NEEDLE)
        {
            let avail = unsafe {
                let mut avail: sys::DWORD = 0;
                let r = sys::PeekNamedPipe(
                    out_handle,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut avail,
                    std::ptr::null_mut(),
                );
                if r == 0 {
                    break; // 管道已关闭
                }
                avail as usize
            };
            if avail == 0 {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            // 数据已在管道里，read 不会阻塞；只取已确认可用的字节
            let take = avail.min(buf.len());
            let n = pty.read(&mut buf[..take]).expect("read");
            if n == 0 {
                break; // EOF
            }
            acc.extend_from_slice(&buf[..n]);
        }
        let text = String::from_utf8_lossy(&acc).into_owned();
        eprintln!("probe: read {} bytes: {text:?}", acc.len());
        assert!(
            text.contains(NEEDLE),
            "输出未走 ConPTY 管道（bytes={}, 3s 内未见 {NEEDLE:?}）",
            acc.len()
        );
    }
}
