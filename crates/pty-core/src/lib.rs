//! pty-core — PTY 统一抽象（零平台实现）。
//!
//! 三个职责：
//! 1. [`Pty`] / [`Backend`] trait：后端实现方接口
//! 2. 后端注册与运行时选择：[`register_backend`] / [`available_backends`] / [`spawn`]
//! 3. C ABI（`include/pty_core.h`）：非 Rust 调用方入口（[`ffi`]）
//!
//! 依赖方向：`pty-unix` / `pty-conpty` / `pty-win10-early` 依赖本 crate；
//! 本 crate 不依赖任何后端（避免循环依赖），后端通过注册表接入。

mod ffi;

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

pub use ffi::{
    pty_child_pid, pty_close, pty_last_error, pty_read, pty_resize, pty_signal, pty_spawn,
    pty_write, PtyHandle,
};

/// 后端种类（与注册表条目对应）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BackendKind {
    UnixPty,
    Conpty,
    Win10Early,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::UnixPty => write!(f, "pty-unix"),
            BackendKind::Conpty => write!(f, "pty-conpty"),
            BackendKind::Win10Early => write!(f, "pty-win10-early"),
        }
    }
}

/// 启动选项。
#[derive(Clone, Debug)]
pub struct SpawnOptions {
    /// 程序名（相对名走 PATH）。
    pub program: OsString,
    /// 参数（不含 argv[0]）。
    pub args: Vec<OsString>,
    /// 工作目录。
    pub cwd: Option<PathBuf>,
    /// 追加/覆盖的环境变量（其余继承父进程）。
    pub env: Vec<(OsString, OsString)>,
    /// 伪终端列数。
    pub cols: u16,
    /// 伪终端行数。
    pub rows: u16,
}

impl SpawnOptions {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            cols: 80,
            rows: 24,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    pub fn env(mut self, key: impl Into<OsString>, val: impl Into<OsString>) -> Self {
        self.env.push((key.into(), val.into()));
        self
    }

    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }
}

/// 子进程信号（后端映射到平台原语）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signal {
    Interrupt,
    Term,
    Quit,
    Hangup,
    Kill,
}

/// 已启动的伪终端会话。
///
/// 约定：
/// - `read` 返回 `Ok(0)` 表示 PTY 已到 EOF（子进程退出且缓冲耗尽）
/// - `close` 幂等；Drop 实现也应尽力回收子进程
pub trait Pty: Send {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()>;
    fn send_signal(&mut self, sig: Signal) -> io::Result<()>;
    fn close(&mut self) -> io::Result<()>;
    /// 非阻塞收割；`Ok(Some(code))` 表示已退出。
    fn try_wait(&mut self) -> io::Result<Option<i32>>;
    fn child_id(&self) -> u32;
    fn kind(&self) -> BackendKind;
}

/// PTY 后端。实现方在模块 `init()` 中调用 [`register_backend`]。
pub trait Backend: Send + Sync {
    fn kind(&self) -> BackendKind;
    /// 选择优先级：数值小者优先（如 conpty 20、win10-early 30、unix 10）。
    fn priority(&self) -> u8;
    /// 快速可用性探测（动态加载的 DLL/导出缺失等应返回 false）。
    fn is_available(&self) -> bool;
    fn spawn(&self, opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError>;
}

/// 启动错误。
#[derive(Debug)]
pub enum SpawnError {
    /// 未注册任何后端。
    NoBackend,
    /// 后端存在但报告不可用（继续尝试下一后端）。
    BackendUnavailable(BackendKind),
    /// 平台 IO 错误。
    Io(io::Error),
    /// 后端启动失败（程序不存在、注入失败等；终止回退）。
    Spawn(String),
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnError::NoBackend => write!(f, "no PTY backend registered"),
            SpawnError::BackendUnavailable(k) => write!(f, "backend {k} unavailable"),
            SpawnError::Io(e) => write!(f, "PTY IO error: {e}"),
            SpawnError::Spawn(s) => write!(f, "PTY spawn failed: {s}"),
        }
    }
}

impl std::error::Error for SpawnError {}

impl From<io::Error> for SpawnError {
    fn from(e: io::Error) -> Self {
        SpawnError::Io(e)
    }
}

static REGISTRY: OnceLock<Mutex<Vec<Arc<dyn Backend>>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Arc<dyn Backend>>> {
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// 注册后端（按 kind 去重替换；幂等）。
pub fn register_backend(backend: Arc<dyn Backend>) {
    let mut reg = registry().lock().expect("registry poisoned");
    if let Some(slot) = reg.iter_mut().find(|b| b.kind() == backend.kind()) {
        *slot = backend;
    } else {
        reg.push(backend);
    }
}

/// 清空注册表（测试用）。
pub fn clear_backends() {
    registry().lock().expect("registry poisoned").clear();
}

/// 已注册后端（按优先级排序，不筛可用性）。
pub fn registered_backends() -> Vec<BackendKind> {
    let reg = registry().lock().expect("registry poisoned");
    let mut v: Vec<(u8, BackendKind)> = reg.iter().map(|b| (b.priority(), b.kind())).collect();
    v.sort_by_key(|&(p, _)| p);
    v.into_iter().map(|(_, k)| k).collect()
}

/// 可用后端（可用且按优先级排序）。
pub fn available_backends() -> Vec<BackendKind> {
    let reg = registry().lock().expect("registry poisoned");
    let mut v: Vec<(u8, BackendKind)> = reg
        .iter()
        .filter(|b| b.is_available())
        .map(|b| (b.priority(), b.kind()))
        .collect();
    v.sort_by_key(|&(p, _)| p);
    v.into_iter().map(|(_, k)| k).collect()
}

/// 按注册优先级选择后端启动。
///
/// `BackendUnavailable` → 尝试下一后端；其余错误直接返回。
pub fn spawn(opts: &SpawnOptions) -> Result<Box<dyn Pty>, SpawnError> {
    let candidates: Vec<Arc<dyn Backend>> = {
        let reg = registry().lock().expect("registry poisoned");
        let mut v: Vec<(u8, Arc<dyn Backend>)> =
            reg.iter().filter(|b| b.is_available()).map(|b| (b.priority(), b.clone())).collect();
        v.sort_by_key(|&(p, _)| p);
        v.into_iter().map(|(_, b)| b).collect()
    };
    if candidates.is_empty() {
        return Err(SpawnError::NoBackend);
    }
    let mut last_unavailable: Option<SpawnError> = None;
    for b in candidates {
        match b.spawn(opts) {
            Ok(pty) => return Ok(pty),
            Err(SpawnError::BackendUnavailable(kind)) => {
                last_unavailable = Some(SpawnError::BackendUnavailable(kind));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_unavailable.unwrap_or(SpawnError::NoBackend))
}
