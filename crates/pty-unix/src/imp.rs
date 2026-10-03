//! POSIX 实现（仅 `cfg(unix)` 编译）。

use std::ffi::{CStr, CString, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::sync::Arc;

use pty_core::{
    register_backend, Backend, BackendKind, Pty, SpawnError, SpawnOptions, Signal,
};

/// 后端优先级（见 pty-core README）。
const PRIORITY: u8 = 10;

/// 探测 devpts 是否可用。
pub fn is_available() -> bool {
    std::path::Path::new("/dev/ptmx").exists() || std::path::Path::new("/dev/pts").exists()
}

/// 注册本后端（幂等；同 kind 替换）。
pub fn register() {
    register_backend(Arc::new(UnixPtyBackend));
}

struct UnixPtyBackend;

impl Backend for UnixPtyBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::UnixPty
    }
    fn priority(&self) -> u8 {
        PRIORITY
    }
    fn is_available(&self) -> bool {
        is_available()
    }
    fn spawn(&self, opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
        spawn_unix(opts)
    }
}

fn to_cstr(s: &OsStr, what: &str) -> Result<CString, SpawnError> {
    CString::new(s.as_bytes())
        .map_err(|_| SpawnError::Spawn(format!("{what} contains an interior NUL")))
}

fn exit_code(status: libc::c_int) -> i32 {
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        -1
    }
}

/// 获取从端设备名。
unsafe fn pts_name(fd: RawFd) -> io::Result<CString> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let mut buf = [0 as libc::c_char; 128];
        let rc = libc::ptsname_r(fd, buf.as_mut_ptr(), buf.len());
        if rc != 0 {
            return Err(io::Error::from_raw_os_error(rc));
        }
        Ok(CStr::from_ptr(buf.as_ptr()).to_owned())
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let p = libc::ptsname(fd);
        if p.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(CStr::from_ptr(p).to_owned())
    }
}

fn spawn_unix(opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
    let prog = to_cstr(&opts.program, "program")?;
    if prog.as_bytes().is_empty() {
        return Err(SpawnError::Spawn("empty program".into()));
    }
    let mut cargs = vec![prog];
    for a in &opts.args {
        cargs.push(to_cstr(a, "argument")?);
    }
    let cwd = match &opts.cwd {
        Some(p) => Some(to_cstr(p.as_os_str(), "cwd")?),
        None => None,
    };
    let mut envs = Vec::with_capacity(opts.env.len());
    for (k, v) in &opts.env {
        envs.push((to_cstr(k, "env key")?, to_cstr(v, "env value")?));
    }

    unsafe {
        // --- 分配 PTY 对 ---------------------------------------------------
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        if master < 0 {
            return Err(io::Error::last_os_error().into());
        }
        let fail_master = |e: io::Error| {
            libc::close(master);
            SpawnError::Io(e)
        };
        if libc::grantpt(master) != 0 || libc::unlockpt(master) != 0 {
            return Err(fail_master(io::Error::last_os_error()));
        }
        let slave_name = match pts_name(master) {
            Ok(n) => n,
            Err(e) => return Err(fail_master(e)),
        };
        let slave = libc::open(slave_name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY);
        if slave < 0 {
            return Err(fail_master(io::Error::last_os_error()));
        }
        let ws = libc::winsize {
            ws_row: opts.rows,
            ws_col: opts.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if libc::ioctl(master, libc::TIOCSWINSZ, &ws as *const libc::winsize) != 0 {
            let e = io::Error::last_os_error();
            libc::close(slave);
            return Err(fail_master(e));
        }

        // --- fork ------------------------------------------------------------
        let pid = libc::fork();
        if pid < 0 {
            let e = io::Error::last_os_error();
            libc::close(slave);
            return Err(fail_master(e));
        }
        if pid == 0 {
            // 子进程
            libc::close(master);
            if libc::setsid() < 0 {
                libc::_exit(127);
            }
            // 将从端设为控制终端（会话首进程，Linux/macOS 通用）
            if libc::ioctl(slave, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                libc::_exit(127);
            }
            for fd in 0..3 {
                if libc::dup2(slave, fd) < 0 {
                    libc::_exit(127);
                }
            }
            if slave > 2 {
                libc::close(slave);
            }
            if let Some(c) = &cwd {
                if libc::chdir(c.as_ptr()) != 0 {
                    libc::_exit(127);
                }
            }
            for (k, v) in &envs {
                if libc::setenv(k.as_ptr(), v.as_ptr(), 1) != 0 {
                    libc::_exit(127);
                }
            }
            let mut argv: Vec<*const libc::c_char> = cargs.iter().map(|c| c.as_ptr()).collect();
            argv.push(std::ptr::null());
            libc::execvp(argv[0], argv.as_ptr());
            libc::_exit(127); // exec 失败
        }

        // --- 父进程 ----------------------------------------------------------
        libc::close(slave);
        Ok(Box::new(UnixPty {
            master,
            child: pid,
            closed: false,
            exit: None,
        }))
    }
}

struct UnixPty {
    master: RawFd,
    child: libc::pid_t,
    closed: bool,
    exit: Option<i32>,
}

impl UnixPty {
    /// 向子进程组发信号（会话/进程组首进程 pgid == pid；回退单进程）。
    fn kill_child(&self, sig: libc::c_int) {
        unsafe {
            if libc::kill(-self.child, sig) != 0 {
                let _ = libc::kill(self.child, sig);
            }
        }
    }
}

impl Pty for UnixPty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.closed {
            return Ok(0);
        }
        let n = unsafe { libc::read(self.master, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let e = io::Error::last_os_error();
        // 所有从端关闭后主端读返回 EIO —— 视作 EOF
        if e.raw_os_error() == Some(libc::EIO) {
            Ok(0)
        } else {
            Err(e)
        }
    }

    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "PTY closed"));
        }
        let n = unsafe { libc::write(self.master, data.as_ptr() as *const libc::c_void, data.len()) };
        if n >= 0 {
            Ok(n as usize)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "PTY closed"));
        }
        let ws = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
        let r = unsafe { libc::ioctl(self.master, libc::TIOCSWINSZ, &ws as *const libc::winsize) };
        if r == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn send_signal(&mut self, sig: Signal) -> io::Result<()> {
        if self.exit.is_some() {
            return Ok(()); // 已收割：无目标
        }
        let s = match sig {
            Signal::Interrupt => libc::SIGINT,
            Signal::Term => libc::SIGTERM,
            Signal::Quit => libc::SIGQUIT,
            Signal::Hangup => libc::SIGHUP,
            Signal::Kill => libc::SIGKILL,
        };
        self.kill_child(s);
        Ok(())
    }

    fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        // 关闭主端 + 会话终端关闭惯例：向子进程组发 SIGHUP
        self.kill_child(libc::SIGHUP);
        let r = unsafe { libc::close(self.master) };
        if r == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<i32>> {
        if let Some(c) = self.exit {
            return Ok(Some(c));
        }
        let mut status: libc::c_int = 0;
        let r = unsafe { libc::waitpid(self.child, &mut status, libc::WNOHANG) };
        if r == 0 {
            Ok(None)
        } else if r == self.child {
            self.exit = Some(exit_code(status));
            Ok(self.exit)
        } else {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ECHILD) {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }

    fn child_id(&self) -> u32 {
        self.child as u32
    }

    fn kind(&self) -> BackendKind {
        BackendKind::UnixPty
    }
}

impl Drop for UnixPty {
    fn drop(&mut self) {
        let _ = self.close();
        // 未收割的子进程：尽力收割避免僵尸
        if self.exit.is_none() {
            let mut status: libc::c_int = 0;
            unsafe {
                let _ = libc::waitpid(self.child, &mut status, libc::WNOHANG);
            }
        }
    }
}
