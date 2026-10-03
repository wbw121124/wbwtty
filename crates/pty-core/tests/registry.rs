//! 后端注册表、优先级选择与 C ABI 测试（含伪后端，全平台可跑）。

use std::ffi::CString;
use std::io;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use pty_core::{
    available_backends, clear_backends, pty_child_pid, pty_close, pty_last_error, pty_read,
    pty_resize, pty_signal, pty_spawn, pty_write, register_backend, registered_backends, spawn,
    Backend, BackendKind, Pty, PtyHandle, SpawnError, SpawnOptions, Signal,
};

static REG_LOCK: Mutex<()> = Mutex::new(());

/// 全程持锁：注册表是进程级全局，串行化各测试。
fn setup() -> MutexGuard<'static, ()> {
    let g = REG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_backends();
    g
}

struct EchoPty {
    buf: Vec<u8>,
    closed: bool,
    kind: BackendKind,
}

impl Pty for EchoPty {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.closed || self.buf.is_empty() {
            return Ok(0); // 模拟 EOF
        }
        let n = out.len().min(self.buf.len());
        out[..n].copy_from_slice(&self.buf[..n]);
        self.buf.drain(..n);
        Ok(n)
    }
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::new(ErrorKind::BrokenPipe, "closed"));
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }
    fn resize(&mut self, _c: u16, _r: u16) -> io::Result<()> {
        Ok(())
    }
    fn send_signal(&mut self, _s: Signal) -> io::Result<()> {
        Ok(())
    }
    fn close(&mut self) -> io::Result<()> {
        self.closed = true;
        Ok(())
    }
    fn try_wait(&mut self) -> io::Result<Option<i32>> {
        Ok(Some(0))
    }
    fn child_id(&self) -> u32 {
        4242
    }
    fn kind(&self) -> BackendKind {
        self.kind
    }
}

struct FakeBackend {
    kind: BackendKind,
    priority: u8,
    available: bool,
    calls: Arc<AtomicUsize>,
    fail: Option<fn(&SpawnOptions) -> SpawnError>,
}

impl Backend for FakeBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn priority(&self) -> u8 {
        self.priority
    }
    fn is_available(&self) -> bool {
        self.available
    }
    fn spawn(&self, opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(f) = self.fail {
            return Err(f(opts));
        }
        Ok(Box::new(EchoPty { buf: Vec::new(), closed: false, kind: self.kind }))
    }
}

#[test]
fn registry_priority_order_and_available_filter() {
    let _g = setup();
    let low = Arc::new(AtomicUsize::new(0));
    let high = Arc::new(AtomicUsize::new(0));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 20,
        available: true,
        calls: low.clone(),
        fail: None,
    }));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::UnixPty,
        priority: 10,
        available: false,
        calls: high.clone(),
        fail: None,
    }));

    // 已注册两个（含不可用），按 priority 排序
    assert_eq!(registered_backends(), vec![BackendKind::UnixPty, BackendKind::Conpty]);
    // 可用列表过滤掉不可用项
    assert_eq!(available_backends(), vec![BackendKind::Conpty]);

    // spawn 选中唯一可用后端（优先级较低数值大的 Conpty 也可用）
    let pty = spawn(&SpawnOptions::new("whatever")).expect("spawn");
    assert_eq!(pty.kind(), BackendKind::Conpty);
    assert_eq!(low.load(Ordering::SeqCst), 1);
    assert_eq!(high.load(Ordering::SeqCst), 0);
}

#[test]
fn spawn_prefers_smaller_priority() {
    let _g = setup();
    let c = Arc::new(AtomicUsize::new(0));
    let u = Arc::new(AtomicUsize::new(0));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 20,
        available: true,
        calls: c.clone(),
        fail: None,
    }));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::UnixPty,
        priority: 10,
        available: true,
        calls: u.clone(),
        fail: None,
    }));
    let pty = spawn(&SpawnOptions::new("x")).expect("spawn");
    assert_eq!(pty.kind(), BackendKind::UnixPty);
    assert_eq!(u.load(Ordering::SeqCst), 1);
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

#[test]
fn spawn_skips_backend_unavailable_and_propagates_real_errors() {
    let _g = setup();
    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 10,
        available: true,
        calls: a.clone(),
        fail: Some(|_| SpawnError::BackendUnavailable(BackendKind::Conpty)),
    }));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::UnixPty,
        priority: 20,
        available: true,
        calls: b.clone(),
        fail: None,
    }));
    let pty = spawn(&SpawnOptions::new("x")).expect("fall through to second backend");
    assert_eq!(pty.kind(), BackendKind::UnixPty);
    assert_eq!(a.load(Ordering::SeqCst), 1);
    assert_eq!(b.load(Ordering::SeqCst), 1);

    // 真实错误（非 BackendUnavailable）不再回退
    clear_backends();
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 10,
        available: true,
        calls: Arc::new(AtomicUsize::new(0)),
        fail: Some(|_| SpawnError::Spawn("program not found".into())),
    }));
    match spawn(&SpawnOptions::new("x")) {
        Err(SpawnError::Spawn(msg)) => assert!(msg.contains("not found")),
        Err(other) => panic!("expected Spawn error, got {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn spawn_without_backends_reports_no_backend() {
    let _g = setup();
    match spawn(&SpawnOptions::new("x")) {
        Err(SpawnError::NoBackend) => {}
        Err(other) => panic!("expected NoBackend, got {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn register_backend_replaces_same_kind() {
    let _g = setup();
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 20,
        available: true,
        calls: first.clone(),
        fail: None,
    }));
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 5,
        available: true,
        calls: second.clone(),
        fail: None,
    }));
    assert_eq!(registered_backends(), vec![BackendKind::Conpty]);
    let _ = spawn(&SpawnOptions::new("x")).expect("spawn");
    assert_eq!(first.load(Ordering::SeqCst), 0, "old entry must be replaced");
    assert_eq!(second.load(Ordering::SeqCst), 1);
}

#[test]
fn spawn_options_defaults() {
    let o = SpawnOptions::new("sh").args(["-c", "true"]);
    assert_eq!(o.program, "sh");
    assert_eq!(o.args.len(), 2);
    assert_eq!((o.cols, o.rows), (80, 24));
    assert!(o.cwd.is_none());
    assert!(o.env.is_empty());
    let o = o.size(120, 40).cwd("D:/tmp").env("K", "V");
    assert_eq!((o.cols, o.rows), (120, 40));
    assert!(o.cwd.is_some());
    assert_eq!(o.env.len(), 1);
}

#[test]
fn spawn_error_display() {
    assert!(SpawnError::NoBackend.to_string().contains("no PTY backend"));
    assert!(SpawnError::BackendUnavailable(BackendKind::Conpty)
        .to_string()
        .contains("pty-conpty"));
    assert!(SpawnError::Spawn("boom".into()).to_string().contains("boom"));
    let io = SpawnError::from(io::Error::new(ErrorKind::Other, "io!"));
    assert!(io.to_string().contains("io!"));
}

// ---- C ABI -----------------------------------------------------------------

fn c_argv(args: &[&CString]) -> Vec<*const std::ffi::c_char> {
    let mut v: Vec<*const std::ffi::c_char> = args.iter().map(|c| c.as_ptr()).collect();
    v.push(std::ptr::null());
    v
}

#[test]
fn ffi_roundtrip_with_fake_backend() {
    let _g = setup();
    register_backend(Arc::new(FakeBackend {
        kind: BackendKind::Conpty,
        priority: 10,
        available: true,
        calls: Arc::new(AtomicUsize::new(0)),
        fail: None,
    }));

    let prog = CString::new("prog").unwrap();
    let a1 = CString::new("--flag").unwrap();
    let argv = c_argv(&[&prog, &a1]);

    unsafe {
        let mut h: *mut PtyHandle = std::ptr::null_mut();
        let rc = pty_spawn(argv.as_ptr(), std::ptr::null(), 100, 30, &mut h);
        let err = std::ffi::CStr::from_ptr(pty_last_error());
        assert_eq!(rc, 0, "spawn failed: {err:?}");
        assert!(!h.is_null());
        assert_eq!(pty_child_pid(), 4242);

        // 写 -> 读 echo
        let msg = b"hello";
        let mut written = 0usize;
        assert_eq!(pty_write(h, msg.as_ptr(), msg.len(), &mut written), 0);
        assert_eq!(written, 5);
        let mut buf = [0u8; 16];
        assert_eq!(pty_read(h, buf.as_mut_ptr(), buf.len()), 5);
        assert_eq!(&buf[..5], b"hello");

        assert_eq!(pty_resize(h, 80, 24), 0);
        assert_eq!(pty_signal(h, 1), 0);
        assert_eq!(pty_signal(h, 99), -1, "unknown signal must fail");

        assert_eq!(pty_close(h), 0);
        assert_eq!(pty_close(std::ptr::null_mut()), -1);
    }
}

#[test]
fn ffi_spawn_rejects_bad_args() {
    let _g = setup();
    unsafe {
        let mut h: *mut PtyHandle = std::ptr::null_mut();
        assert_eq!(pty_spawn(std::ptr::null(), std::ptr::null(), 1, 1, &mut h), -1);
        let empty: [*const std::ffi::c_char; 1] = [std::ptr::null()];
        assert_eq!(pty_spawn(empty.as_ptr(), std::ptr::null(), 1, 1, &mut h), -1);
        let err = std::ffi::CStr::from_ptr(pty_last_error());
        assert!(!err.to_bytes().is_empty(), "last_error must be set");
        // 未注册后端
        let prog = CString::new("p").unwrap();
        let argv = c_argv(&[&prog]);
        assert_eq!(pty_spawn(argv.as_ptr(), std::ptr::null(), 1, 1, &mut h), -1);
    }
}
