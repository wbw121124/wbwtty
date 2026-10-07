//! 宿主控制台窗口管理（Win11 默认终端 = Windows Terminal 场景）。
//!
//! 实证（diag-time / verify-min）：本进程 `FreeConsole` → `AllocConsole` 后，
//! 弹出的顶层窗口是 Windows Terminal 的 `CASCADIA_HOSTING_WINDOW_CLASS`
//! （标题先为 `Terminal`，后改成本进程命令行），`GetConsoleWindow()` 返回的
//! 就是它（或 conhost 经典窗口），`ShowWindow(SW_HIDE)` 会被 WT 的重显示
//! 对抗。纯轮询有 ~40-100ms 的可见闪烁；`ShowWindowAsync` 走消息队列实测
//! 延迟 ~250ms，短命窗口根本来不及生效。
//!
//! 策略（进程级去重，TTL 4s，两条线程）：
//! - **事件线程**：`SetWinEventHook(WINEVENT_OUTOFCONTEXT)` 监听
//!   `EVENT_OBJECT_SHOW` / `EVENT_SYSTEM_MINIMIZEEND`，回调里立即
//!   `ShowWindow(SW_FORCEMINIMIZE)`（同步、不抢焦点）——show/restore 的
//!   瞬间就压住，闪烁窗口基本不可见；`PeekMessageW` 泵消息派发回调；
//! - **轮询线程**：40ms（1s 后 150ms）兜底——本进程经典控制台窗口
//!   可见即 `SW_HIDE`（conhost 可能被控制台活动重新显示），CASCADIA
//!   窗口未最小化则最小化（回调竞态/漏事件的 backstop）。
//!
//! 候选判定：CASCADIA = 快照之后新出现 或 标题含本进程可执行文件名；
//! 经典 `ConsoleWindowClass` 仅凭标题规则（不误伤用户自己的控制台窗口）。
//!
//! API：[`minimize_console_host_window`] 在 `AllocConsole` **之前**调用；
//! [`hide_console_window`] 在 `AllocConsole` **之后**立即调用一次。

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::sys;

const CASCADIA: &str = "CASCADIA_HOSTING_WINDOW_CLASS";
const CLASSIC_CONSOLE: &str = "ConsoleWindowClass";
const GUARD_TTL: Duration = Duration::from_secs(4);
const FAST_POLL: Duration = Duration::from_millis(40);
const SLOW_POLL: Duration = Duration::from_millis(150);
const FAST_WINDOW: Duration = Duration::from_millis(1000);
const HOOK_PUMP: Duration = Duration::from_millis(4);

/// 守卫去重：TTL 内重复请求直接复用已在跑的线程。
static GUARD: Mutex<Option<Instant>> = Mutex::new(None);
/// 事件回调读取的守卫状态（hook 线程在 `minimize_console_host_window`
/// 写入后才启动，之后只读）。
static HOOK_STATE: Mutex<Option<(HashSet<isize>, Option<String>)>> = Mutex::new(None);

/// 启动宿主窗口守卫（**在 `AllocConsole` 之前调用**）。
///
/// 进程级去重：4 秒内重复调用不重复起线程；超过 TTL 后再次调用以新的
/// 窗口快照重启。线程随进程退出而终止（不 join）。
pub fn minimize_console_host_window() {
    let now = Instant::now();
    {
        let mut g = GUARD.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = *g {
            if now.duration_since(t) < GUARD_TTL {
                return;
            }
        }
        *g = Some(now);
    }
    // 标题规则：WT 窗口标题 = 本进程命令行 → 含可执行文件名
    let exe = std::env::current_exe()
        .ok()
        .map(|p| {
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().to_lowercase())
        })
        .filter(|s| !s.is_empty());
    let snapshot = host_windows();
    *HOOK_STATE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((snapshot.clone(), exe.clone()));
    std::thread::spawn(move || event_hook_loop());
    std::thread::spawn(move || poll_loop(exe, snapshot));
}

/// 隐藏本进程当前控制台的顶层窗口；隐藏仍可见时最小化兜底。
/// **在 `AllocConsole` 之后调用**（后续被重新显示由事件/轮询压住）。
pub fn hide_console_window() {
    unsafe {
        let win = sys::GetConsoleWindow();
        if win.is_null() {
            return;
        }
        sys::ShowWindow(win, sys::SW_HIDE);
        if sys::IsWindowVisible(win) != 0 {
            sys::ShowWindow(win, sys::SW_FORCEMINIMIZE);
        }
    }
}

/// 判定 hwnd 是否是“本进程的宿主弹窗”，是则（尚未最小化时）压住。
/// CASCADIA 走 delta/标题规则；经典窗口只走标题规则。
fn suppress(
    hwnd: sys::HANDLE,
    class: &str,
    title: &str,
    snapshot: &HashSet<isize>,
    exe: Option<&str>,
) {
    let titled = exe.is_some_and(|e| !e.is_empty() && title.to_lowercase().contains(e));
    let mine = if class == CASCADIA {
        titled || !snapshot.contains(&(hwnd as isize))
    } else {
        titled // 经典窗口仅标题规则，绝不误伤用户自己的控制台
    };
    if !mine {
        return;
    }
    unsafe {
        if sys::IsIconic(hwnd) == 0 {
            sys::ShowWindow(hwnd, sys::SW_FORCEMINIMIZE);
        }
    }
}

/// `WINEVENTPROC`：show / restore 瞬间立即压制（回调在事件线程内）。
unsafe extern "system" fn winevent_cb(
    _hook: sys::HANDLE,
    _event: sys::DWORD,
    hwnd: sys::HANDLE,
    id_object: sys::LONG,
    _id_child: sys::LONG,
    _thread: sys::DWORD,
    _time: sys::DWORD,
) {
    if id_object != sys::OBJID_WINDOW || hwnd.is_null() {
        return;
    }
    let mut cls = [0u16; 128];
    let n = sys::GetClassNameW(hwnd, cls.as_mut_ptr(), 128);
    if n == 0 {
        return;
    }
    let class = String::from_utf16_lossy(&cls[..n as usize]);
    if class != CASCADIA && class != CLASSIC_CONSOLE {
        return;
    }
    let st = HOOK_STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((snapshot, exe)) = st.as_ref() {
        let mut title = [0u16; 512];
        let m = sys::GetWindowTextW(hwnd, title.as_mut_ptr(), 512);
        let title = if m > 0 {
            String::from_utf16_lossy(&title[..m as usize])
        } else {
            String::new()
        };
        suppress(hwnd, &class, &title, snapshot, exe.as_deref());
    }
}

/// 事件线程：挂两个 out-of-context hook，`PeekMessageW` 泵消息；TTL 到点
/// 卸载 hook 后退出。
fn event_hook_loop() {
    unsafe {
        let hooks = [
            sys::SetWinEventHook(
                sys::EVENT_OBJECT_SHOW,
                sys::EVENT_OBJECT_SHOW,
                std::ptr::null_mut(),
                Some(winevent_cb),
                0,
                0,
                sys::WINEVENT_OUTOFCONTEXT,
            ),
            sys::SetWinEventHook(
                sys::EVENT_SYSTEM_MINIMIZEEND,
                sys::EVENT_SYSTEM_MINIMIZEEND,
                std::ptr::null_mut(),
                Some(winevent_cb),
                0,
                0,
                sys::WINEVENT_OUTOFCONTEXT,
            ),
        ];
        let start = Instant::now();
        let mut msg: sys::Msg = std::mem::zeroed();
        while start.elapsed() < GUARD_TTL {
            while sys::PeekMessageW(
                &mut msg,
                std::ptr::null_mut(),
                0,
                0,
                sys::PM_REMOVE,
            ) != 0
            {
                // 回调在 PeekMessage 内部派发；本线程无窗口，普通消息丢弃
            }
            std::thread::sleep(HOOK_PUMP);
        }
        for h in hooks {
            if !h.is_null() {
                sys::UnhookWinEvent(h);
            }
        }
    }
}

/// `EnumWindows` 回调：收集全部顶层窗口的 (hwnd, 类名, 标题)——含隐藏
/// 窗口（须在 show 之前就压住）。
unsafe extern "system" fn enum_cb(hwnd: sys::HANDLE, lparam: isize) -> sys::BOOL {
    let out = &mut *(lparam as *mut Vec<(isize, String, String)>);
    let mut cls = [0u16; 128];
    let n = sys::GetClassNameW(hwnd, cls.as_mut_ptr(), 128);
    if n == 0 {
        return 1;
    }
    let class = String::from_utf16_lossy(&cls[..n as usize]);
    if class == CASCADIA || class == CLASSIC_CONSOLE {
        let mut title = [0u16; 512];
        let m = sys::GetWindowTextW(hwnd, title.as_mut_ptr(), 512);
        let title = if m > 0 {
            String::from_utf16_lossy(&title[..m as usize])
        } else {
            String::new()
        };
        out.push((hwnd as isize, class, title));
    }
    1 // 继续枚举
}

fn host_windows() -> HashSet<isize> {
    let mut v: Vec<(isize, String, String)> = Vec::new();
    unsafe { sys::EnumWindows(Some(enum_cb), &mut v as *mut _ as isize) };
    v.into_iter()
        .filter(|(_, c, _)| c == CASCADIA)
        .map(|(h, _, _)| h)
        .collect()
}

/// 每轮：自己的经典窗口重新压一遍 + 所有候选窗口兑底最小化。
fn poll_pass(exe: Option<&str>, snapshot: &HashSet<isize>) {
    unsafe {
        let own = sys::GetConsoleWindow();
        if !own.is_null() && sys::IsWindowVisible(own) != 0 {
            sys::ShowWindow(own, sys::SW_HIDE);
            if sys::IsWindowVisible(own) != 0 {
                sys::ShowWindow(own, sys::SW_FORCEMINIMIZE);
            }
        }
    }
    let mut v: Vec<(isize, String, String)> = Vec::new();
    unsafe { sys::EnumWindows(Some(enum_cb), &mut v as *mut _ as isize) };
    for (hwnd, class, title) in v {
        suppress(hwnd as sys::HANDLE, &class, &title, snapshot, exe);
    }
}

fn poll_loop(exe: Option<String>, snapshot: HashSet<isize>) {
    let start = Instant::now();
    while start.elapsed() < GUARD_TTL {
        poll_pass(exe.as_deref(), &snapshot);
        let step = if start.elapsed() < FAST_WINDOW {
            FAST_POLL
        } else {
            SLOW_POLL
        };
        std::thread::sleep(step);
    }
}
