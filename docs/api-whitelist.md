# pty-win10-early / pty-conpty API 白名单

- 最近更新：2026-10-03（阶段 0）
- 机读版本：`config/api-whitelist.json`（本文件由 `python tools/check_api_whitelist.py --emit-md` 从 JSON 生成）
- 证据：`docs/sdk-compat-matrix.md` + `config/sdk-matrix.json`

## 规则

1. **static_allowed**：以下 API 在全部 7 个 Windows 10 SDK（10240…17763）头文件中
   均有声明，允许 `pty-win10-early` 与 `pty-conpty` 静态链接引用。
2. **dynamic_only**：只能 `LoadLibraryW + GetProcAddress` 运行时解析；出现静态声明或
   直接调用即 CI 失败。
3. 其他任何未列入 static_allowed 的 Win32 函数符号一律禁止。需要新增时：
   编辑 `tools/gen-compat-matrix.py` 的 `CURATED_KERNEL` → 重新运行生成器 →
   确认 7 版矩阵全绿 → 提交（生成器会拒绝任何一版缺失的 API）。
4. 强制执行：`python tools/check_api_whitelist.py`（本机与 CI 均运行）。

## static_allowed（静态可链接）

### conhook

- `DeleteCriticalSection` — `um/synchapi.h`（min SDK 10.0.10240）
- `DisableThreadLibraryCalls` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `EnterCriticalSection` — `um/synchapi.h`（min SDK 10.0.10240）
- `FreeLibraryAndExitThread` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `GetEnvironmentVariableW` — `um/processenv.h`（min SDK 10.0.10240）
- `InitializeCriticalSection` — `um/synchapi.h`（min SDK 10.0.10240）
- `LeaveCriticalSection` — `um/synchapi.h`（min SDK 10.0.10240）
- `VirtualProtect` — `um/memoryapi.h`（min SDK 10.0.10240）

### console-host

- `AllocConsole` — `um/consoleapi.h`（min SDK 10.0.10240）
- `AttachConsole` — `um/consoleapi.h`（min SDK 10.0.10240）
- `FreeConsole` — `um/consoleapi.h`（min SDK 10.0.10240）
- `GenerateConsoleCtrlEvent` — `um/consoleapi2.h`（min SDK 10.0.10240）
- `GetConsoleProcessList` — `um/consoleapi3.h`（min SDK 10.0.10240）
- `GetConsoleWindow` — `um/consoleapi3.h`（min SDK 10.0.10240）
- `SetConsoleCtrlHandler` — `um/consoleapi.h`（min SDK 10.0.10240）

### console-input

- `WriteConsoleInputW` — `um/consoleapi2.h`（min SDK 10.0.10240）

### console-resize

- `SetConsoleScreenBufferSize` — `um/consoleapi2.h`（min SDK 10.0.10240）
- `SetConsoleWindowInfo` — `um/consoleapi2.h`（min SDK 10.0.10240）

### console-screen

- `GetConsoleScreenBufferInfo` — `um/consoleapi2.h`（min SDK 10.0.10240）
- `ReadConsoleOutputW` — `um/consoleapi2.h`（min SDK 10.0.10240）

### error

- `FormatMessageW` — `um/winbase.h`（min SDK 10.0.10240）
- `GetLastError` — `um/errhandlingapi.h`（min SDK 10.0.10240）
- `SetLastError` — `um/errhandlingapi.h`（min SDK 10.0.10240）

### file-pipe

- `CloseHandle` — `um/handleapi.h`（min SDK 10.0.10240）
- `ConnectNamedPipe` — `um/namedpipeapi.h`（min SDK 10.0.10240）
- `CreateFileW` — `um/fileapi.h`（min SDK 10.0.10240）
- `CreateNamedPipeW` — `um/namedpipeapi.h`（min SDK 10.0.10240）
- `CreatePipe` — `um/namedpipeapi.h`（min SDK 10.0.10240）
- `DisconnectNamedPipe` — `um/namedpipeapi.h`（min SDK 10.0.10240）
- `DuplicateHandle` — `um/handleapi.h`（min SDK 10.0.10240）
- `FlushFileBuffers` — `um/fileapi.h`（min SDK 10.0.10240）
- `GetFileSizeEx` — `um/fileapi.h`（min SDK 10.0.10240）
- `GetFileType` — `um/fileapi.h`（min SDK 10.0.10240）
- `PeekNamedPipe` — `um/namedpipeapi.h`（min SDK 10.0.10240）
- `ReadFile` — `um/fileapi.h`（min SDK 10.0.10240）
- `SetNamedPipeHandleState` — `um/namedpipeapi.h`（min SDK 10.0.10240）
- `WriteFile` — `um/fileapi.h`（min SDK 10.0.10240）

### loader

- `FreeLibrary` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `GetModuleHandleExW` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `GetModuleHandleW` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `GetProcAddress` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `LoadLibraryExW` — `um/libloaderapi.h`（min SDK 10.0.10240）
- `LoadLibraryW` — `um/libloaderapi.h`（min SDK 10.0.10240）

### proc-thread-attribute

- `DeleteProcThreadAttributeList` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `InitializeProcThreadAttributeList` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `UpdateProcThreadAttribute` — `um/processthreadsapi.h`（min SDK 10.0.10240）

### process

- `CreateProcessAsUserW` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `CreateProcessW` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `CreateProcessWithLogonW` — `um/winbase.h`（min SDK 10.0.10240）
- `CreateThread` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `GetExitCodeProcess` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `GetExitCodeThread` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `GetProcessId` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `GetThreadId` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `OpenProcess` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `ResumeThread` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `SuspendThread` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `TerminateProcess` — `um/processthreadsapi.h`（min SDK 10.0.10240）

### string

- `MultiByteToWideChar` — `um/stringapiset.h`（min SDK 10.0.10240）
- `WideCharToMultiByte` — `um/stringapiset.h`（min SDK 10.0.10240）
- `lstrlenW` — `um/winbase.h`（min SDK 10.0.10240）

### sync-wait

- `CreateEventW` — `um/synchapi.h`（min SDK 10.0.10240）
- `GetTickCount` — `um/sysinfoapi.h`（min SDK 10.0.10240）
- `GetTickCount64` — `um/sysinfoapi.h`（min SDK 10.0.10240）
- `QueryPerformanceCounter` — `um/profileapi.h`（min SDK 10.0.10240）
- `QueryPerformanceFrequency` — `um/profileapi.h`（min SDK 10.0.10240）
- `ResetEvent` — `um/synchapi.h`（min SDK 10.0.10240）
- `SetEvent` — `um/synchapi.h`（min SDK 10.0.10240）
- `Sleep` — `um/synchapi.h`（min SDK 10.0.10240）
- `WaitForMultipleObjects` — `um/synchapi.h`（min SDK 10.0.10240）
- `WaitForSingleObject` — `um/synchapi.h`（min SDK 10.0.10240）

### thread-injection

- `CreateRemoteThread` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `GetThreadContext` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `ReadProcessMemory` — `um/memoryapi.h`（min SDK 10.0.10240）
- `SetThreadContext` — `um/processthreadsapi.h`（min SDK 10.0.10240）
- `VirtualAllocEx` — `um/memoryapi.h`（min SDK 10.0.10240）
- `VirtualFreeEx` — `um/memoryapi.h`（min SDK 10.0.10240）
- `WriteProcessMemory` — `um/memoryapi.h`（min SDK 10.0.10240）

### window-control

- `EnumWindows` — `um/winuser.h`（min SDK 10.0.10240）
- `GetClassNameW` — `um/winuser.h`（min SDK 10.0.10240）
- `GetWindowTextW` — `um/winuser.h`（min SDK 10.0.10240）
- `IsIconic` — `um/winuser.h`（min SDK 10.0.10240）
- `IsWindowVisible` — `um/winuser.h`（min SDK 10.0.10240）
- `PeekMessageW` — `um/winuser.h`（min SDK 10.0.10240）
- `SetWinEventHook` — `um/winuser.h`（min SDK 10.0.10240）
- `ShowWindow` — `um/winuser.h`（min SDK 10.0.10240）
- `UnhookWinEvent` — `um/winuser.h`（min SDK 10.0.10240）

## dynamic_only（必须动态加载）

- `ClosePseudoConsole` — min SDK 10.0.17763；ConPTY 仅 Windows 10 1809+ 提供；1809 之前运行时加载失败须回退 pty-win10-early
- `CreatePseudoConsole` — min SDK 10.0.17763；ConPTY 仅 Windows 10 1809+ 提供；1809 之前运行时加载失败须回退 pty-win10-early
- `ResizePseudoConsole` — min SDK 10.0.17763；ConPTY 仅 Windows 10 1809+ 提供；1809 之前运行时加载失败须回退 pty-win10-early

## 拒绝清单示例（违规会被 CI 拦下）

```text
crates/pty-win10-early/src/host.rs:120: [R1] SetCurrentDirectoryW — 不在 static_allowed 白名单中
crates/pty-conpty/src/sys.rs:44: [R2] CreatePseudoConsole — dynamic_only 符号禁止静态声明
```
