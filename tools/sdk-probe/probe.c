/* 自动生成：tools/gen-compat-matrix.py — 勿手工编辑
 * 用途：CI 用每个早期 Windows 10 SDK 的头文件编译本探针，
 * 确认 pty-win10-early 静态白名单中的每个 API 在该 SDK 中都可声明+链接。
 * 编译示例：
 *   cl /nologo /c probe.c /I<SDK>\\um /I<SDK>\\shared /I<SDK>\\ucrt /Fo probe.obj
 */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* 逐一引用白名单符号，取地址以强制声明与导入解析 */
static const void *volatile probe_refs[] = {
    (const void *)&AllocConsole,
    (const void *)&AttachConsole,
    (const void *)&CloseHandle,
    (const void *)&ConnectNamedPipe,
    (const void *)&CreateEventW,
    (const void *)&CreateFileW,
    (const void *)&CreateNamedPipeW,
    (const void *)&CreatePipe,
    (const void *)&CreateProcessAsUserW,
    (const void *)&CreateProcessW,
    (const void *)&CreateProcessWithLogonW,
    (const void *)&CreateRemoteThread,
    (const void *)&CreateThread,
    (const void *)&DisconnectNamedPipe,
    (const void *)&DuplicateHandle,
    (const void *)&FlushFileBuffers,
    (const void *)&FormatMessageW,
    (const void *)&FreeConsole,
    (const void *)&FreeLibrary,
    (const void *)&GenerateConsoleCtrlEvent,
    (const void *)&GetConsoleProcessList,
    (const void *)&GetConsoleWindow,
    (const void *)&GetExitCodeProcess,
    (const void *)&GetExitCodeThread,
    (const void *)&GetFileSizeEx,
    (const void *)&GetFileType,
    (const void *)&GetLastError,
    (const void *)&GetModuleHandleExW,
    (const void *)&GetModuleHandleW,
    (const void *)&GetProcAddress,
    (const void *)&GetProcessId,
    (const void *)&GetThreadContext,
    (const void *)&GetThreadId,
    (const void *)&GetTickCount,
    (const void *)&GetTickCount64,
    (const void *)&LoadLibraryExW,
    (const void *)&LoadLibraryW,
    (const void *)&MultiByteToWideChar,
    (const void *)&OpenProcess,
    (const void *)&PeekNamedPipe,
    (const void *)&QueryPerformanceCounter,
    (const void *)&QueryPerformanceFrequency,
    (const void *)&ReadFile,
    (const void *)&ReadProcessMemory,
    (const void *)&ResetEvent,
    (const void *)&ResumeThread,
    (const void *)&SetConsoleCtrlHandler,
    (const void *)&SetEvent,
    (const void *)&SetLastError,
    (const void *)&SetNamedPipeHandleState,
    (const void *)&SetThreadContext,
    (const void *)&Sleep,
    (const void *)&SuspendThread,
    (const void *)&TerminateProcess,
    (const void *)&VirtualAllocEx,
    (const void *)&VirtualFreeEx,
    (const void *)&WaitForMultipleObjects,
    (const void *)&WaitForSingleObject,
    (const void *)&WideCharToMultiByte,
    (const void *)&WriteFile,
    (const void *)&WriteProcessMemory,
    (const void *)&lstrlenW,
};

int probe_main(void)
{
    return (int)(sizeof(probe_refs) / sizeof(probe_refs[0]));
}
