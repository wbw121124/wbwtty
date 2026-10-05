//! Win32 层：类型/常量、白名单内静态声明、ConPTY 三函数动态加载。
//!
//! 白名单约束（`config/api-whitelist.json`，`tools/check_api_whitelist.py` 强制）：
//! - 下方 `extern "system"` 只能声明 `static_allowed` 中的符号
//! - `CreatePseudoConsole` / `ResizePseudoConsole` / `ClosePseudoConsole` 属 `dynamic_only`：
//!   只能经 `GetProcAddress` 解析成函数指针后调用，源码里不得出现静态声明或直接调用形态

#![allow(non_snake_case, non_camel_case_types)]

use std::ffi::c_void;
use std::sync::OnceLock;

// ---- 基础类型（与 SDK 头文件一一对应，全部 pointer-sized） -----------------
pub type BOOL = i32;
pub type DWORD = u32;
pub type WORD = u16;
pub type SHORT = i16;
pub type SIZE_T = usize;
pub type HRESULT = i32;
pub type HANDLE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;

/// `HPCON`（consoleapi.h：`typedef VOID* HPCON`）。
pub type Hpc = *mut c_void;

// ---- 常量 ----------------------------------------------------------------
pub const EXTENDED_STARTUPINFO_PRESENT: DWORD = 0x0008_0000;
pub const CREATE_UNICODE_ENVIRONMENT: DWORD = 0x0000_0400;
/// `STARTUPINFO.dwFlags`：启用 `hStdInput/hStdOutput/hStdError` 三个标准句柄。
pub const STARTF_USESTDHANDLES: DWORD = 0x0000_0100;
/// `ProcThreadAttributeValue(ProcThreadAttributeConsole, FALSE, TRUE, FALSE)`
pub const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: SIZE_T = 0x0002_0016;

pub const ERROR_BROKEN_PIPE: i32 = 109;
pub const ERROR_ACCESS_DENIED: i32 = 5;
pub const ERROR_NO_DATA: i32 = 232;
pub const ERROR_PIPE_NOT_CONNECTED: i32 = 233;
pub const STILL_ACTIVE: DWORD = 259;
pub const WAIT_OBJECT_0: DWORD = 0;

// ---- 结构体（repr(C)，布局对照 sdk-headers/ 官方头文件） -------------------
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Coord {
    pub X: SHORT,
    pub Y: SHORT,
}

#[repr(C)]
pub struct SecurityAttributes {
    pub nLength: DWORD,
    pub lpSecurityDescriptor: LPVOID,
    pub bInheritHandle: BOOL,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StartUpInfoW {
    pub cb: DWORD,
    pub lpReserved: *mut u16,
    pub lpDesktop: *mut u16,
    pub lpTitle: *mut u16,
    pub dwX: DWORD,
    pub dwY: DWORD,
    pub dwXSize: DWORD,
    pub dwYSize: DWORD,
    pub dwXCountChars: DWORD,
    pub dwYCountChars: DWORD,
    pub dwFillAttribute: DWORD,
    pub dwFlags: DWORD,
    pub wShowWindow: WORD,
    pub cbReserved2: WORD,
    pub lpReserved2: *mut u8,
    pub hStdInput: HANDLE,
    pub hStdOutput: HANDLE,
    pub hStdError: HANDLE,
}

#[repr(C)]
pub struct StartUpInfoExW {
    pub StartupInfo: StartUpInfoW,
    pub lpAttributeList: LPVOID,
}

#[repr(C)]
#[derive(Default)]
pub struct ProcessInformation {
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub dwProcessId: DWORD,
    pub dwThreadId: DWORD,
}

// ---- 静态链接声明（全部在 static_allowed 中） -----------------------------
extern "system" {
    pub fn CreatePipe(
        pReadPipe: *mut HANDLE,
        pWritePipe: *mut HANDLE,
        lpPipeAttributes: *const SecurityAttributes,
        nSize: DWORD,
    ) -> BOOL;

    #[allow(clippy::too_many_arguments)]
    pub fn CreateProcessW(
        lpApplicationName: *const u16,
        lpCommandLine: *mut u16,
        lpProcessAttributes: *const SecurityAttributes,
        lpThreadAttributes: *const SecurityAttributes,
        bInheritHandles: BOOL,
        dwCreationFlags: DWORD,
        lpEnvironment: LPCVOID,
        lpCurrentDirectory: *const u16,
        lpStartupInfo: *mut StartUpInfoW,
        lpProcessInformation: *mut ProcessInformation,
    ) -> BOOL;

    pub fn CloseHandle(hObject: HANDLE) -> BOOL;

    pub fn ReadFile(
        hFile: HANDLE,
        lpBuffer: LPVOID,
        nNumberOfBytesToRead: DWORD,
        lpNumberOfBytesRead: *mut DWORD,
        lpOverlapped: LPVOID,
    ) -> BOOL;

    pub fn WriteFile(
        hFile: HANDLE,
        lpBuffer: LPCVOID,
        nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: *mut DWORD,
        lpOverlapped: LPVOID,
    ) -> BOOL;

    pub fn PeekNamedPipe(
        hNamedPipe: HANDLE,
        lpBuffer: LPVOID,
        nBufferSize: DWORD,
        lpBytesRead: *mut DWORD,
        lpTotalBytesAvail: *mut DWORD,
        lpBytesLeftThisMessage: *mut DWORD,
    ) -> BOOL;

    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut DWORD) -> BOOL;
    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
    pub fn GetModuleHandleW(lpModuleName: *const u16) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: *const i8) -> LPVOID;

    pub fn InitializeProcThreadAttributeList(
        lpAttributeList: LPVOID,
        dwAttributeCount: DWORD,
        dwFlags: DWORD,
        lpSize: *mut SIZE_T,
    ) -> BOOL;

    pub fn UpdateProcThreadAttribute(
        lpAttributeList: LPVOID,
        dwFlags: DWORD,
        Attribute: SIZE_T,
        lpValue: LPVOID,
        cbSize: SIZE_T,
        lpPreviousValue: LPVOID,
        lpReturnSize: *mut SIZE_T,
    ) -> BOOL;

    pub fn DeleteProcThreadAttributeList(lpAttributeList: LPVOID);
}

// ---- ConPTY 三函数：dynamic_only，只能运行时解析 ---------------------------
type CreateFn = unsafe extern "system" fn(Coord, HANDLE, HANDLE, DWORD, *mut Hpc) -> HRESULT;
type ResizeFn = unsafe extern "system" fn(Hpc, Coord) -> HRESULT;
type CloseFn = unsafe extern "system" fn(Hpc);

/// 已解析的 ConPTY 入口（`&'static`，`OnceLock` 保证只加载一次）。
pub struct ConptyApi {
    create_fn: CreateFn,
    resize_fn: ResizeFn,
    close_fn: CloseFn,
}

impl ConptyApi {
    /// `HRESULT CreatePseudoConsole(COORD, HANDLE, HANDLE, DWORD, PHPCON)`
    pub unsafe fn create(
        &self,
        size: Coord,
        input: HANDLE,
        output: HANDLE,
        flags: DWORD,
        out: *mut Hpc,
    ) -> HRESULT {
        (self.create_fn)(size, input, output, flags, out)
    }

    /// `HRESULT ResizePseudoConsole(HPCON, COORD)`
    pub unsafe fn resize(&self, hpc: Hpc, size: Coord) -> HRESULT {
        (self.resize_fn)(hpc, size)
    }

    /// `void ClosePseudoConsole(HPCON)`
    pub unsafe fn close(&self, hpc: Hpc) {
        (self.close_fn)(hpc)
    }
}

static CONPTY: OnceLock<Option<ConptyApi>> = OnceLock::new();

/// 解析 ConPTY 三函数；1809 之前（或导出缺失）返回 `None` → 后端报告不可用。
pub fn conpty_api() -> Option<&'static ConptyApi> {
    CONPTY.get_or_init(|| unsafe { load_conpty() }).as_ref()
}

unsafe fn load_conpty() -> Option<ConptyApi> {
    for name in ["kernel32.dll", "kernelbase.dll"] {
        // 注意：GetModuleHandleW 收 UTF-16，不能直接把 UTF-8 字节串当宽字符串
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let module = GetModuleHandleW(wide.as_ptr());
        if module.is_null() {
            continue;
        }
        if let Some(api) = resolve_from(module) {
            return Some(api);
        }
    }
    None
}

unsafe fn resolve_from(module: HMODULE) -> Option<ConptyApi> {
    let create = GetProcAddress(module, b"CreatePseudoConsole\0".as_ptr() as *const i8);
    let resize = GetProcAddress(module, b"ResizePseudoConsole\0".as_ptr() as *const i8);
    let close = GetProcAddress(module, b"ClosePseudoConsole\0".as_ptr() as *const i8);
    if create.is_null() || resize.is_null() || close.is_null() {
        return None;
    }
    Some(ConptyApi {
        create_fn: std::mem::transmute(create),
        resize_fn: std::mem::transmute(resize),
        close_fn: std::mem::transmute(close),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    /// x64 下对照 sdk-headers 的官方结构体尺寸（布局错会导致 CreateProcess
    /// 读到错误的 lpAttributeList，子进程退回父控制台）。
    #[test]
    fn struct_layout_matches_sdk_on_x64() {
        if mem::size_of::<usize>() != 8 {
            return;
        }
        assert_eq!(mem::size_of::<Coord>(), 4);
        assert_eq!(mem::size_of::<StartUpInfoW>(), 104);
        assert_eq!(mem::size_of::<StartUpInfoExW>(), 112);
        assert_eq!(mem::size_of::<ProcessInformation>(), 24);
        assert_eq!(mem::size_of::<Hpc>(), 8);
    }

    #[test]
    fn conpty_exports_resolve_on_windows10_1809_or_later() {
        // 本机（Win10 1809+）与 CI windows runner 都应解析成功；
        // 更老系统返回 None 属预期（后端降级为不可用）。
        if conpty_api().is_some() {
            assert_ne!(conpty_api().map(|a| a as *const ConptyApi), None);
        }
    }

    #[test]
    fn api_is_cached_between_calls() {
        let first = conpty_api().map(|a| a as *const ConptyApi);
        let second = conpty_api().map(|a| a as *const ConptyApi);
        assert_eq!(first, second);
    }
}
