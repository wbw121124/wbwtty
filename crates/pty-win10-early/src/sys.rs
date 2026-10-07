//! Win32 层：类型/常量/结构/extern（全部静态链接）。
//!
//! 契约：`config/api-whitelist.json` + `tools/check_api_whitelist.py` 强制；
//! 本文件 `extern "system"` 只允许出现 `static_allowed` 中的函数（ConPTY 三函数
//! 属 `dynamic_only`，本后端完全不使用）。结构字段与 sdk-headers/ 官方头文件
//! 一一对应（repr(C)，无隐式断言依赖）。

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;

// ---- 基础类型（SDK 头文件一一对应，全 pointer-sized） -----------------
pub type BOOL = i32;
pub type DWORD = u32;
pub type UINT = u32;
pub type LONG = i32;
pub type WORD = u16;
pub type SHORT = i16;
pub type WCHAR = u16;
pub type HANDLE = *mut c_void;
pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;

// ---- 常量 ---------------------------------------------------------------
/// `ShowWindow`（隐藏控制台窗口；winuser.h）
pub const SW_HIDE: i32 = 0;
/// `ShowWindow`：SW_FORCEMINIMIZE——同步最小化、不激活其他窗口、
/// 宿主线程卡死也能生效（winuser.h；守卫线程压制 WT 弹窗用）
pub const SW_FORCEMINIMIZE: i32 = 11;

/// `WNDENUMPROC`：`EnumWindows` 回调（winuser.h：BOOL CALLBACK (HWND, LPARAM)）
pub type WndEnumProc = Option<unsafe extern "system" fn(HANDLE, isize) -> BOOL>;

/// `WINEVENTPROC`：`SetWinEventHook` 回调（winuser.h：VOID CALLBACK，
/// idObject 为 `OBJID_WINDOW` 时 hwnd 才是窗口级事件）
pub type WinEventProc =
    Option<unsafe extern "system" fn(HANDLE, DWORD, HANDLE, LONG, LONG, DWORD, DWORD)>;

// SetWinEventHook/WinEvent 常量（winuser.h；非函数，不受白名单 linter 约束）
/// `SetWinEventHook`：跨进程事件，不映射 DLL（回调在本线程消息泵中派发）
pub const WINEVENT_OUTOFCONTEXT: DWORD = 0x0000;
/// `EVENT_OBJECT_SHOW`：窗口被显示（含 hide→show 重显示）
pub const EVENT_OBJECT_SHOW: DWORD = 0x8002;
/// `EVENT_SYSTEM_MINIMIZEEND`：窗口从最小化恢复（WT 改标题会强拉起来）
pub const EVENT_SYSTEM_MINIMIZEEND: DWORD = 0x0017;
/// `OBJID_WINDOW`（oleacc.h）：窗口级对象
pub const OBJID_WINDOW: LONG = 0;
/// `PeekMessageW.wRemoveMsg`：取走消息
pub const PM_REMOVE: UINT = 0x0001;

pub const GENERIC_READ: DWORD = 0x8000_0000;
pub const GENERIC_WRITE: DWORD = 0x4000_0000;
pub const FILE_SHARE_READ: DWORD = 0x0000_0001;
pub const FILE_SHARE_WRITE: DWORD = 0x0000_0002;
pub const OPEN_EXISTING: DWORD = 3;
/// `CreateProcessW`：子进程创建后挂起（第二刀注入窗口，winbase.h）
pub const CREATE_SUSPENDED: DWORD = 0x0000_0004;
/// `VirtualAllocEx`：MEM_COMMIT | MEM_RESERVE
pub const MEM_COMMIT_RESERVE: DWORD = 0x0000_1000 | 0x0000_2000;
/// `VirtualFreeEx`：MEM_RELEASE（dwSize 必须 0）
pub const MEM_RELEASE: DWORD = 0x0000_8000;
/// `VirtualAllocEx.flProtect`
pub const PAGE_READWRITE: DWORD = 0x04;
/// `CreateNamedPipeW.dwOpenMode`：双向
pub const PIPE_ACCESS_DUPLEX: DWORD = 0x0000_0003;
/// `CreateNamedPipeW.dwPipeMode`：消息型写 / 消息型读 / 阻塞
pub const PIPE_TYPE_MESSAGE: DWORD = 0x0000_0004;
pub const PIPE_READMODE_MESSAGE: DWORD = 0x0000_0002;
pub const PIPE_WAIT: DWORD = 0x0000_0000;
/// `ConnectNamedPipe`：连接已建立（客户端先连上时返回 TRUE+ERROR_PIPE_CONNECTED 视作成功）
pub const ERROR_PIPE_CONNECTED: DWORD = 535;
/// 消息模式下缓冲不足（部分字节已读入，剩余下轮续读）
pub const ERROR_MORE_DATA: DWORD = 234;
/// `WaitForSingleObject`：无限等待；C 侧 conhook.worker 仍使用。
#[allow(dead_code)]
pub const INFINITE: DWORD = 0xFFFF_FFFF;
/// `CreateProcessW`：lpEnvironment 为 UTF-16 块
pub const CREATE_UNICODE_ENVIRONMENT: DWORD = 0x0000_0400;
/// `STARTUPINFO.dwFlags`：用 `hStdInput/hStdOutput/hStdError` 作子进程标准句柄
/// （必须配合**可继承**的 CONIN$/CONOUT$ 句柄 + `bInheritHandles=TRUE`，
/// 否则子进程标准句柄为 NULL——cmd 等不输出任何东西）
pub const STARTF_USESTDHANDLES: DWORD = 0x0000_0100;
/// `CreateProcessW.dwCreationFlags`：`STARTUPINFOEXW.lpAttributeList` 生效
/// （同时 `StartupInfo.cb` 须为 `size_of::<StartUpInfoExW>()`）
pub const EXTENDED_STARTUPINFO_PRESENT: DWORD = 0x0008_0000;
/// `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` = ProcThreadAttributeValue(2, F, T, F)
/// （winbase.h）：把子进程句柄继承范围钉死为指定列表
pub const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
/// `GenerateConsoleCtrlEvent`：CTRL_C 信号
pub const CTRL_C_EVENT: DWORD = 0;
/// `AttachConsole`：挂到父进程的控制台（原控制台已销毁时的兜底）
pub const ATTACH_PARENT_PROCESS: DWORD = 0xFFFF_FFFF;

pub const STILL_ACTIVE: DWORD = 259;
pub const WAIT_OBJECT_0: DWORD = 0;

// 屏幕单元属性（wincon.h / consoleapi*.h；注意 LVB 位在 0x0100+ 段，
// 与低 8 位颜色位不重叠）
pub const COMMON_LVB_UNDERSCORE: WORD = 0x2000;
/// COMMON_LVB_REVERSE_VIDEO：反显
pub const COMMON_LVB_REVERSE_VIDEO: WORD = 0x1000;

// INPUT_RECORD.EventType
pub const KEY_EVENT: WORD = 0x0001;
pub const MOUSE_EVENT: WORD = 0x0002;

// KEY_EVENT_RECORD.wVirtualKey（只列用到的）
pub const VK_BACK: WORD = 0x08;
pub const VK_TAB: WORD = 0x09;
pub const VK_RETURN: WORD = 0x0D;
pub const VK_HOME: WORD = 0x24;
pub const VK_END: WORD = 0x23;
pub const VK_LEFT: WORD = 0x25;
pub const VK_UP: WORD = 0x26;
pub const VK_RIGHT: WORD = 0x27;
pub const VK_DOWN: WORD = 0x28;
pub const VK_INSERT: WORD = 0x2D;
pub const VK_DELETE: WORD = 0x2E;
pub const VK_F1: WORD = 0x70; // F1..F12 连续

// KEY_EVENT_RECORD.dwControlKeyState
pub const LEFT_ALT_PRESSED: DWORD = 0x0002;
pub const LEFT_CTRL_PRESSED: DWORD = 0x0008;
pub const SHIFT_PRESSED: DWORD = 0x0010;
pub const ENHANCED_KEY: DWORD = 0x0100;

// MOUSE_EVENT_RECORD
pub const FROM_LEFT_1ST_BUTTON_PRESSED: DWORD = 0x0001;
pub const RIGHTMOST_BUTTON_PRESSED: DWORD = 0x0004;
pub const FROM_LEFT_2ND_BUTTON_PRESSED: DWORD = 0x0008;
pub const MOUSE_MOVED: DWORD = 0x0001;
pub const MOUSE_WHEEL: DWORD = 0x0004;

// ---- 结构（repr(C)，与官方头文件布局核对） ----------------------------
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Coord {
    pub X: SHORT,
    pub Y: SHORT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SmallRect {
    pub Left: SHORT,
    pub Top: SHORT,
    pub Right: SHORT,
    pub Bottom: SHORT,
}

/// `CONSOLE_SCREEN_BUFFER_INFO`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConsoleScreenBufferInfo {
    pub dwSize: Coord,
    pub dwCursorPosition: Coord,
    pub wAttributes: WORD,
    pub srWindow: SmallRect,
    pub dwMaximumWindowSize: Coord,
}

/// `CHAR_INFO`（只用 UnicodeChar 分支；布局 4 字节）
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CharInfo {
    pub UnicodeChar: WCHAR,
    pub Attributes: WORD,
}

/// `KEY_EVENT_RECORD`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyEventRecord {
    pub bKeyDown: BOOL,
    pub wRepeatCount: WORD,
    pub wVirtualKey: WORD,
    pub wVirtualScanCode: WORD,
    pub UnicodeChar: WCHAR,
    pub dwControlKeyState: DWORD,
}

/// `MOUSE_EVENT_RECORD`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MouseEventRecord {
    pub dwMousePosition: Coord,
    pub dwButtonState: DWORD,
    pub dwControlKeyState: DWORD,
    pub dwEventFlags: DWORD,
}

/// `INPUT_RECORD.Event` 联合（对齐取最大成员 4）
#[repr(C)]
#[derive(Clone, Copy)]
pub union InputEvent {
    pub KeyEvent: KeyEventRecord,
    pub MouseEvent: MouseEventRecord,
}

/// `INPUT_RECORD`（WORD EventType + 2 字节填充 + 联合 = 24 字节）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputRecord {
    pub EventType: WORD,
    pub Event: InputEvent,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SecurityAttributes {
    pub nLength: DWORD,
    pub lpSecurityDescriptor: LPVOID,
    pub bInheritHandle: BOOL,
}

/// `STARTUPINFOW`（`CreateProcessW` 用，无扩展属性）
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

/// `POINT`（windef.h）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Point {
    pub x: LONG,
    pub y: LONG,
}

/// `MSG`（winuser.h：HWND + UINT + WPARAM + LPARAM + DWORD + POINT；
/// x64 对齐 8 → 48 字节，wParam@16 / time@32 / pt@36）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Msg {
    pub hwnd: HANDLE,
    pub message: UINT,
    pub wParam: usize,
    pub lParam: isize,
    pub time: DWORD,
    pub pt: Point,
}

#[repr(C)]
#[derive(Default)]
pub struct ProcessInformation {
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub dwProcessId: DWORD,
    pub dwThreadId: DWORD,
}

/// `STARTUPINFOEXW`（winbase.h：`STARTUPINFOW` 打头 + 属性列表指针）
#[repr(C)]
pub struct StartUpInfoExW {
    pub StartupInfo: StartUpInfoW,
    pub lpAttributeList: LPVOID,
}

// ---- 静态链接声明（全部在 static_allowed 中） ---------------------------
extern "system" {
    pub fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *const SecurityAttributes,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;

    pub fn CloseHandle(hObject: HANDLE) -> BOOL;

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

    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
    pub fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut DWORD) -> BOOL;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;

    pub fn FreeConsole() -> BOOL;
    pub fn AllocConsole() -> BOOL;
    pub fn GetConsoleWindow() -> HANDLE;
    pub fn SetConsoleCtrlHandler(
        HandlerRoutine: Option<unsafe extern "system" fn(DWORD) -> BOOL>,
        Add: BOOL,
    ) -> BOOL;
    /// `dwProcessGroupId = 0` → 广播给调用方所在控制台的全部进程
    /// （宿主靠 spawn 时 `SetConsoleCtrlHandler(NULL, TRUE)` 忽略）
    pub fn GenerateConsoleCtrlEvent(dwCtrlEvent: DWORD, dwProcessGroupId: DWORD) -> BOOL;

    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: HANDLE,
        lpConsoleScreenBufferInfo: *mut ConsoleScreenBufferInfo,
    ) -> BOOL;

    pub fn ReadConsoleOutputW(
        hConsoleOutput: HANDLE,
        lpBuffer: *mut CharInfo,
        dwBufferSize: Coord,
        dwBufferCoord: Coord,
        lpReadRegion: *mut SmallRect,
    ) -> BOOL;

    pub fn WriteConsoleInputW(
        hConsoleInput: HANDLE,
        lpBuffer: *const InputRecord,
        nLength: DWORD,
        lpNumberOfEventsWritten: *mut DWORD,
    ) -> BOOL;

    pub fn SetConsoleScreenBufferSize(
        hConsoleOutput: HANDLE,
        dwSize: Coord,
    ) -> BOOL;

    pub fn SetConsoleWindowInfo(
        hConsoleOutput: HANDLE,
        bAbsolute: BOOL,
        lpConsoleWindow: *const SmallRect,
    ) -> BOOL;

    pub fn AttachConsole(dwProcessId: DWORD) -> BOOL;
    pub fn GetConsoleProcessList(
        lpdwProcessList: *mut DWORD,
        dwProcessCount: DWORD,
    ) -> DWORD;

    pub fn InitializeProcThreadAttributeList(
        lpAttributeList: LPVOID,
        dwAttributeCount: DWORD,
        dwFlags: DWORD,
        lpSize: *mut usize,
    ) -> BOOL;
    pub fn UpdateProcThreadAttribute(
        lpAttributeList: LPVOID,
        dwFlags: DWORD,
        attribute: usize,
        lpValue: LPVOID,
        cbSize: usize,
        lpPreviousValue: LPVOID,
        lpdwReturnedSize: *mut usize,
    ) -> BOOL;
    pub fn DeleteProcThreadAttributeList(lpAttributeList: LPVOID);

    // ---- 加载器（loader 组；注入/反注入两侧共用） --------------------------
    pub fn GetModuleHandleW(lpModuleName: *const u16) -> HANDLE;
    pub fn GetProcAddress(hModule: HANDLE, lpProcName: *const u8) -> LPVOID;

    // ---- 子进程线程退出码（process 组；注入后校验 LoadLibraryW 结果） ------
    pub fn GetExitCodeThread(hThread: HANDLE, lpExitCode: *mut DWORD) -> BOOL;

    // ---- 管道水位探测（file-pipe 组；读线程非阻塞轮询） --------------------
    pub fn PeekNamedPipe(
        hNamedPipe: HANDLE,
        lpBuffer: LPVOID,
        nBufferSize: DWORD,
        lpBytesRead: *mut DWORD,
        lpTotalBytesAvail: *mut DWORD,
        lpBytesLeftThisMessage: *mut DWORD,
    ) -> BOOL;

    // ---- 第二刀注入（thread-injection / process 组，白名单静态链接） --------
    pub fn VirtualAllocEx(
        hProcess: HANDLE,
        lpAddress: LPVOID,
        dwSize: usize,
        flAllocationType: DWORD,
        flProtect: DWORD,
    ) -> LPVOID;
    pub fn VirtualFreeEx(
        hProcess: HANDLE,
        lpAddress: LPVOID,
        dwSize: usize,
        dwFreeType: DWORD,
    ) -> BOOL;
    pub fn WriteProcessMemory(
        hProcess: HANDLE,
        lpBaseAddress: LPVOID,
        lpBuffer: LPCVOID,
        nSize: usize,
        lpNumberOfBytesWritten: *mut usize,
    ) -> BOOL;
    pub fn CreateRemoteThread(
        hProcess: HANDLE,
        lpThreadAttributes: LPVOID,
        dwStackSize: usize,
        lpStartAddress: LPVOID,
        lpParameter: LPVOID,
        dwCreationFlags: DWORD,
        lpThreadId: *mut DWORD,
    ) -> HANDLE;
    pub fn ResumeThread(hThread: HANDLE) -> DWORD;

    // ---- conhook 命名管道（file-pipe 组） ---------------------------------
    pub fn CreateNamedPipeW(
        lpName: *const u16,
        dwOpenMode: DWORD,
        dwPipeMode: DWORD,
        nMaxInstances: DWORD,
        nOutBufferSize: DWORD,
        nInBufferSize: DWORD,
        nDefaultTimeOut: DWORD,
        lpSecurityAttributes: *const SecurityAttributes,
    ) -> HANDLE;
    pub fn ConnectNamedPipe(hNamedPipe: HANDLE, lpOverlapped: LPVOID) -> BOOL;
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
}

// user32：窗口控制 + WinEvent。必须显式 `#[link]`——MSVC 下 rust std 只链
// kernel32 等，不链 user32（windows-gnu 隐式有导入库，本机曾掩盖该问题，
// CI 的 link.exe 会报 LNK2019 unresolved external）。
#[link(name = "user32")]
extern "system" {
    pub fn ShowWindow(hWnd: HANDLE, nCmdShow: i32) -> BOOL;
    pub fn IsWindowVisible(hWnd: HANDLE) -> BOOL;
    pub fn IsIconic(hWnd: HANDLE) -> BOOL;
    pub fn EnumWindows(lpEnumFunc: WndEnumProc, lParam: isize) -> BOOL;
    pub fn GetClassNameW(hWnd: HANDLE, lpClassName: *mut u16, nMaxCount: i32) -> i32;
    pub fn GetWindowTextW(hWnd: HANDLE, lpString: *mut u16, nMaxCount: i32) -> i32;

    pub fn SetWinEventHook(
        eventMin: DWORD,
        eventMax: DWORD,
        hmodWinEventProc: LPVOID,
        pfnWinEventProc: WinEventProc,
        idProcess: DWORD,
        idThread: DWORD,
        dwFlags: DWORD,
    ) -> HANDLE;
    pub fn UnhookWinEvent(hWinEventHook: HANDLE) -> BOOL;
    pub fn PeekMessageW(
        lpMsg: *mut Msg,
        hWnd: HANDLE,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
        wRemoveMsg: UINT,
    ) -> BOOL;
}

// ---- 结构布局回归测试（防止 repr(C) 手误） -----------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_sizes_match_sdk() {
        assert_eq!(std::mem::size_of::<Coord>(), 4);
        assert_eq!(std::mem::size_of::<SmallRect>(), 8);
        // 全 SHORT/WORD 成员（无 DWORD）→ align 2 → 22 字节
        assert_eq!(std::mem::size_of::<ConsoleScreenBufferInfo>(), 22);
        assert_eq!(std::mem::size_of::<CharInfo>(), 4);
        // KEY_EVENT: i32 + 3*WORD + WCHAR + DWORD = 16（12 处已 4 对齐）
        assert_eq!(std::mem::size_of::<KeyEventRecord>(), 16);
        assert_eq!(std::mem::size_of::<MouseEventRecord>(), 16);
        // INPUT_RECORD: WORD + pad2 + union16 = 20
        assert_eq!(std::mem::size_of::<InputRecord>(), 20);
        assert_eq!(std::mem::offset_of!(InputRecord, Event), 4);
        // STARTUPINFO：含 4 个指针成员 → align 8 → 104
        assert_eq!(std::mem::size_of::<StartUpInfoW>(), 104);
        // PROCESS_INFORMATION：2*HANDLE + 2*DWORD → x64 = 24（8+8+4+4）
        assert_eq!(std::mem::size_of::<ProcessInformation>(), 24);
        // STARTUPINFOEXW：STARTUPINFO(104) + 属性列表指针(8) = 112
        assert_eq!(std::mem::size_of::<StartUpInfoExW>(), 112);
        // MSG: hwnd(8) + message(4)+pad(4) + wParam(8) + lParam(8) + time(4)
        //      + POINT(8) = 44 → 对齐 8 → 48
        assert_eq!(std::mem::size_of::<Msg>(), 48);
        assert_eq!(std::mem::offset_of!(Msg, wParam), 16);
        assert_eq!(std::mem::offset_of!(Msg, lParam), 24);
        assert_eq!(std::mem::offset_of!(Msg, time), 32);
        assert_eq!(std::mem::offset_of!(Msg, pt), 36);
    }
}
