# Windows 10 SDK 兼容性矩阵（docs/sdk-compat-matrix.md）

- 最近更新：2026-10-03（阶段 0）
- 数据来源：`sdk-headers/<version>/` —— 各版本 Windows 10 SDK 官方头文件的 include 闭包
  （快照归档自 ralish/win-headers，并以官方安装器抽查校验，见 `docs/header-verification.md`）
- 生成方式：`python tools/gen-compat-matrix.py`（标记区间内的表格自动生成，幂等；
  区间外内容为人工撰写并随实现更新）
- 原始证据：`config/sdk-matrix.json`（函数声明的文件:行号与预处理器守卫）

## 覆盖的 SDK 版本

| SDK 版本 | 对应 Windows 10 | 头文件闭包 |
|---|---|---|
| 10.0.10240 | 1507（初始版本） | ✓ 入库 |
| 10.0.10586 | 1511 | ✓ 入库 |
| 10.0.14393 | 1607 | ✓ 入库 |
| 10.0.15063 | 1703 | ✓ 入库 |
| 10.0.16299 | 1709 | ✓ 入库 |
| 10.0.17134 | 1803 | ✓ 入库 |
| 10.0.17763 | 1809（对比基线） | ✓ 入库 |

## 1. ConPTY API：1809 之前完全不可用

`CreatePseudoConsole` / `ResizePseudoConsole` / `ClosePseudoConsole`（以及 `HPCON` 类型、
`PSEUDOCONSOLE_INHERIT_CURSOR`、`PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`）**仅在 10.0.17763
头文件中声明**。1507–1803 的任何 SDK 中不存在这些符号：既不能静态声明，也不能假设导入表中存在。

<!-- BEGIN GENERATED: conpty -->
| ConPTY API | 1507–1803 | 1809 (17763) |
|---|---|---|
| `CreatePseudoConsole` | **✗ 不声明，不可链接** | ✓ `um/consoleapi.h`:303 |
| `ResizePseudoConsole` | **✗ 不声明，不可链接** | ✓ `um/consoleapi.h`:315 |
| `ClosePseudoConsole` | **✗ 不声明，不可链接** | ✓ `um/consoleapi.h`:324 |
<!-- END GENERATED: conpty -->

结论（对应规格"前置准备 3/4"）：

1. `pty-conpty` 的三个 ConPTY 函数必须 `LoadLibraryW("kernel32.dll") + GetProcAddress` 动态解析，
   解析失败即返回 `BackendUnavailable`，由上层切换到 `pty-win10-early`。**禁止静态链接。**
2. `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` 的数值（0x00020016）在 `shared/winnt.h` 中仅 17763+ 提供，
   `pty-conpty` 内以本地常量定义并注释出处，不依赖该头文件常量。
3. `pty-win10-early` 编译基线 = 10.0.10240（1507）。

## 2. 控制台 API 矩阵（规格点名的拦截函数）

规格要求至少覆盖：`ReadConsoleOutputW`、`WriteConsoleInputW`、`GetConsoleScreenBufferInfo`、
`SetConsoleScreenBufferSize`、`SetConsoleWindowInfo`、`SetConsoleCursorPosition`、
`SetConsoleTextAttribute`、`ScrollConsoleScreenBuffer`、`ReadConsoleInputW` —— 下表连同全部
`consoleapi*.h`/`wincon.h` 控制台函数一并列出，每项标注在各早期 SDK 中的存在性：

<!-- BEGIN GENERATED: console-api-matrix -->
| API | 1507<br>10240 | 1511<br>10586 | 1607<br>14393 | 1703<br>15063 | 1709<br>16299 | 1803<br>17134 | 1809<br>17763 | 最低 SDK |
|---|---|---|---|---|---|---|---|---|
| `AddConsoleAliasA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `AddConsoleAliasW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `AllocConsole` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `AttachConsole` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ClosePseudoConsole` | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | 1809 |
| `CreateConsoleScreenBuffer` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreatePseudoConsole` | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | 1809 |
| `ExpungeConsoleCommandHistoryA` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `ExpungeConsoleCommandHistoryW` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `FillConsoleOutputAttribute` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FillConsoleOutputCharacterA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FillConsoleOutputCharacterW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FlushConsoleInputBuffer` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FreeConsole` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GenerateConsoleCtrlEvent` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasExesA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasExesLengthA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasExesLengthW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasExesW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasesA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasesLengthA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasesLengthW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleAliasesW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleCP` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleCommandHistoryA` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `GetConsoleCommandHistoryLengthA` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `GetConsoleCommandHistoryLengthW` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `GetConsoleCommandHistoryW` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `GetConsoleCursorInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleDisplayMode` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleFontSize` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleHistoryInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleMode` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleOriginalTitleA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleOriginalTitleW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleOutputCP` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleProcessList` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleScreenBufferInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleScreenBufferInfoEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleSelectionInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleTitleA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleTitleW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleWindow` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetCurrentConsoleFont` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetCurrentConsoleFontEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetLargestConsoleWindowSize` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetNumberOfConsoleInputEvents` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetNumberOfConsoleMouseButtons` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `PeekConsoleInputA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `PeekConsoleInputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleInputA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleInputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleOutputA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleOutputAttribute` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleOutputCharacterA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleOutputCharacterW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleOutputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ResizePseudoConsole` | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | 1809 |
| `ScrollConsoleScreenBufferA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ScrollConsoleScreenBufferW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleActiveScreenBuffer` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleCP` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleCtrlHandler` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleCursorInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleCursorPosition` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleDisplayMode` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleHistoryInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleMode` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleNumberOfCommandsA` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `SetConsoleNumberOfCommandsW` | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ | ✓ | 1803 |
| `SetConsoleOutputCP` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleScreenBufferInfoEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleScreenBufferSize` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleTextAttribute` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleTitleA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleTitleW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleWindowInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetCurrentConsoleFontEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleInputA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleInputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleOutputA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleOutputAttribute` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleOutputCharacterA` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleOutputCharacterW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleOutputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |

声明来源：`um/consoleapi.h`、`um/consoleapi2.h`、`um/consoleapi3.h`、`um/wincon.h`（函数按来源文件归属，跨版本取并集）。
<!-- END GENERATED: console-api-matrix -->

结构性结论：`consoleapi2.h`、`consoleapi3.h` 两个文件**自 1803（10.0.17134）才存在**；
1507–1709 的控制台函数全部位于 `wincon.h`/`consoleapi.h`。裁剪入库时已按版本如实保留该差异
（见 `tools/prune-headers.py` 输出提示）。

## 3. 宿主/桥接所需 kernel32 API 矩阵

`pty-win10-early` 服务进程与注入 DLL、`pty-conpty` 进程创建所需的非控制台 API：
（进程创建、注入（CreateRemoteThread/VirtualAllocEx/WriteProcessMemory）、
句柄与命名管道、加载器、等待与计时、字符串与错误处理）

<!-- BEGIN GENERATED: kernel-api-matrix -->
| API | 1507<br>10240 | 1511<br>10586 | 1607<br>14393 | 1703<br>15063 | 1709<br>16299 | 1803<br>17134 | 1809<br>17763 | 最低 SDK |
|---|---|---|---|---|---|---|---|---|
| `AllocConsole` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `AttachConsole` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CloseHandle` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ConnectNamedPipe` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateEventW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateFileW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateNamedPipeW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreatePipe` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateProcessAsUserW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateProcessW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateProcessWithLogonW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateRemoteThread` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `CreateThread` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `DeleteProcThreadAttributeList` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `DisconnectNamedPipe` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `DuplicateHandle` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `EnumWindows` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FlushFileBuffers` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FormatMessageW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FreeConsole` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `FreeLibrary` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GenerateConsoleCtrlEvent` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetClassNameW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleProcessList` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleScreenBufferInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetConsoleWindow` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetExitCodeProcess` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetExitCodeThread` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetFileSizeEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetFileType` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetLastError` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetModuleHandleExW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetModuleHandleW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetProcAddress` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetProcessId` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetThreadContext` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetThreadId` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetTickCount` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetTickCount64` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `GetWindowTextW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `InitializeProcThreadAttributeList` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `IsIconic` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `IsWindowVisible` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `LoadLibraryExW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `LoadLibraryW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `MultiByteToWideChar` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `OpenProcess` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `PeekMessageW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `PeekNamedPipe` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `QueryPerformanceCounter` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `QueryPerformanceFrequency` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadConsoleOutputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadFile` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ReadProcessMemory` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ResetEvent` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ResumeThread` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleCtrlHandler` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleScreenBufferSize` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetConsoleWindowInfo` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetEvent` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetLastError` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetNamedPipeHandleState` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetThreadContext` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SetWinEventHook` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `ShowWindow` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `Sleep` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `SuspendThread` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `TerminateProcess` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `UnhookWinEvent` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `UpdateProcThreadAttribute` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `VirtualAllocEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `VirtualFreeEx` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WaitForMultipleObjects` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WaitForSingleObject` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WideCharToMultiByte` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteConsoleInputW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteFile` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `WriteProcessMemory` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |
| `lstrlenW` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1507 |

覆盖 pty-win10-early 宿主（进程创建、注入、管道、加载器）与 pty-conpty 所需的 kernel32 表面；`CURATED_KERNEL` 定义见 `tools/gen-compat-matrix.py`。
<!-- END GENERATED: kernel-api-matrix -->

## 4. 关键宏与类型标记（行为差异的编译期证据）

<!-- BEGIN GENERATED: markers -->
| 标记 | 1507<br>10240 | 1511<br>10586 | 1607<br>14393 | 1703<br>15063 | 1709<br>16299 | 1803<br>17134 | 1809<br>17763 |
|---|---|---|---|---|---|---|---|
| `ENABLE_VIRTUAL_TERMINAL_PROCESSING` | **✗** | ✓ `um/wincon.h:327` | ✓ `um/wincon.h:328` | ✓ `um/wincon.h:328` | ✓ `um/wincon.h:328` | ✓ `um/consoleapi.h:97` | ✓ `um/consoleapi.h:98` | 1511 |
| `ENABLE_VIRTUAL_TERMINAL_INPUT` | **✗** | **✗** | ✓ `um/wincon.h:320` | ✓ `um/wincon.h:320` | ✓ `um/wincon.h:320` | ✓ `um/consoleapi.h:89` | ✓ `um/consoleapi.h:90` | 1607 |
| `ENABLE_PROCESSED_OUTPUT` | ✓ `um/wincon.h:325` | ✓ `um/wincon.h:325` | ✓ `um/wincon.h:326` | ✓ `um/wincon.h:326` | ✓ `um/wincon.h:326` | ✓ `um/consoleapi.h:95` | ✓ `um/consoleapi.h:96` | 1507 |
| `ENABLE_LVB_GRID_WORLDWIDE` | **✗** | **✗** | ✓ `um/wincon.h:330` | ✓ `um/wincon.h:330` | ✓ `um/wincon.h:330` | ✓ `um/consoleapi.h:99` | ✓ `um/consoleapi.h:100` | 1607 |
| `STARTUPINFOEXW` | ✓ `um/winbase.h:2901` | ✓ `um/winbase.h:2947` | ✓ `um/winbase.h:2958` | ✓ `um/winbase.h:2970` | ✓ `um/winbase.h:3021` | ✓ `um/winbase.h:3037` | ✓ `um/winbase.h:3075` | 1507 |
| `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ `um/winbase.h:3467` | 1809 |
| `PSEUDOCONSOLE_INHERIT_CURSOR` | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ `um/consoleapi.h:299` | 1809 |
| `HPCON` | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | ✓ `um/consoleapi.h:309` | 1809 |
| `GetConsoleHistoryInfo` | ✓ `um/wincon.h:613` | ✓ `um/wincon.h:614` | ✓ `um/wincon.h:617` | ✓ `um/wincon.h:617` | ✓ `um/wincon.h:617` | ✓ `um/consoleapi3.h:137` | ✓ `um/consoleapi3.h:137` | 1507 |
| `GetConsoleProcessList` | ✓ `um/wincon.h:918` | ✓ `um/wincon.h:919` | ✓ `um/wincon.h:922` | ✓ `um/wincon.h:928` | ✓ `um/wincon.h:928` | ✓ `um/consoleapi3.h:415` | ✓ `um/consoleapi3.h:415` | 1507 |
| `GetTickCount64` | ✓ `um/sysinfoapi.h:214` | ✓ `um/sysinfoapi.h:214` | ✓ `um/sysinfoapi.h:206` | ✓ `um/sysinfoapi.h:207` | ✓ `um/sysinfoapi.h:207` | ✓ `um/sysinfoapi.h:155` | ✓ `um/sysinfoapi.h:155` | 1507 |
<!-- END GENERATED: markers -->

## 5. 存在但行为在 1809 前后有差异的 API/机制

> 本节为人工撰写；"头文件证据"可在 `config/sdk-matrix.json` 中按 函数/标记 → file:line 追溯。

| 项 | 1809 之前 | 1809 及之后 | 对框架的影响 |
|---|---|---|---|
| `ENABLE_VIRTUAL_TERMINAL_PROCESSING`（SetConsoleMode 位 0x0004） | **1507 无此常量与行为**；1511–1803 头文件在 `wincon.h` 声明，仅当控制台宿主支持 VT 渲染管道时有效 | 常量迁至 `consoleapi.h`，控制台宿主支持 VT 分层渲染，ConPTY 由其生成 VT 输出 | 1507 上没有任何 VT 输出通道：`pty-win10-early` 必须走"控制台 API 捕获→服务端合成 VT"路径，见第 6 节取舍 |
| `ENABLE_LVB_GRID_WORLDWIDE`（宽字符/LVB 网格属性） | 1507/1511 不存在；1607+ 提供 | 提供 | 早期桥接的虚拟屏幕缓冲不依赖该位，用自有属性位表达真彩色/粗斜体等 |
| ConPTY 三函数 + `HPCON` | 不存在（见第 1 动态加载并回退 | 存在且为官方推荐路径 | 运行时后端选择：1809+ ConPTY，否则 `pty-win10-early` |
| `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` | 不存在；`STARTUPINFOEXW` + `UpdateProcThreadAttribute` 本身 1507 即存在（见标记表） | 可用 | `pty-conpty` 用本地常量 0x20016，仅在加载 ConPTY 成功后使用 |
| `GetConsoleHistoryInfo` / `SetConsoleHistoryInfo` | 1507 即存在 | 存在 | 历史缓冲查询可用；滚动缓冲主要由前端 vt-parser 维护 |
| `GenerateConsoleCtrlEvent` | 全版本可用（Win2000 起） | 同 | 信号映射首选：Ctrl+C/Ctrl+Break 直接投递进程组；跨会话/无法附组时回退命名管道带外控制帧 |
| `ReadConsoleOutputW` 系列读取 vs `WriteConsoleInputW` 注入 | 全版本可用 | 同（ConPTY 下前端不再需要注入，输入直接写 PTY 写端） | 仅 `pty-win10-early` 使用；接口语义统一在 pty-core 层抹平 |
| 控制台代码页（`SetConsoleCP/OutputCP`） | 全版本可用；1809 前 UTF-8(65001) 输出行为在旧系统有历史缺陷 | 改进 | 桥接层统一按 UTF-16 (`*W`) API 交互，规避 65001 问题 |

### 1507 的内核/宿主限制与替代方案（取舍说明，对应规格"如果遇到限制给出替代方案"）

- **限制**：1507 无 ConPTY、无 VT 输出处理、控制台宿主不提供任何"字节流 PTY"。
- **方案 A（规格指定，采用）**：用户态桥接 —— DLL 注入拦截 `*Console*` 导出，服务进程维护
  虚拟屏幕缓冲并合成 VT。取舍：能完整覆盖原生 cmd/powershell；代价是需要注入（对受保护进程、
  部分杀软场景失败），此时回退路径 B/C。
- **方案 B**：Cygwin PTY 适配 —— 覆盖 Cygwin/MSYS2 程序，代价是仅对链接 cygwin1.dll 的程序有效。
- **方案 C**：WinPTY 回退 —— 仅简单场景（无完整 PTY 语义：受限的信号/resize/鼠标），
  规格明确禁止把它作为唯一方案。
- **不采用**：内核驱动/`condrv` 修改（超出用户态框架范围，分发与签名成本不可接受）。

## 6. 各平台后端需要的最低 SDK 版本

| 后端/模块 | 最低编译 SDK | 运行时要求 | 说明 |
|---|---|---|---|
| vt-parser | 无（纯 Rust，无平台依赖） | — | 任何平台 |
| pty-core | 无（trait + 可选 C ABI 头） | — | 任何平台 |
| pty-unix | 无（Unix 头由系统提供） | Linux/macOS | CI ubuntu/macos 验证 |
| pty-conpty | 10.0.17763（如需声明 ConPTY 类型） | Windows 10 1809+ / Windows 11 | 三个函数动态加载；实际以运行时 `GetProcAddress` 结果为准，1803 及以下返回 `BackendUnavailable` |
| pty-win10-early | **10.0.10240（1507）** | Windows 10 全版本 | 静态白名单 7 版全绿；其余动态加载；CI 在 7 个 SDK 上逐个 cl 编译 |
| term-input | 无 | — | 任何平台 |
| term-render-gtk | 无（GTK3 由系统/MSYS2 提供） | GTK3 运行时 | CI ubuntu + 本机 MSYS2 ucrt64 |
| term-app | 随所选后端 | — | 运行时自动选择后端 |

## 7. 白名单规则（对应规格"前置准备 4"）

1. **静态白名单** = 在全部 7 个 SDK（10240..17763）头文件中均声明的 API 集合
   （`config/api-whitelist.json` 的 `static_allowed`，人读版 `docs/api-whitelist.md`）。
2. 白名单之外的任何 Win32 符号（含 `dynamic_only` 的 ConPTY 三函数）必须
   `LoadLibrary + GetProcAddress` 动态加载 + 版本/存在性检测，**绝不允许静态链接**。
3. 强制执行：`python tools/check_api_whitelist.py` 扫描 `crates/pty-win10-early`、
   `crates/pty-conpty` 源码；CI 每次 push/PR 运行，失败禁止合并。
4. CI 编译矩阵：`tools/sdk-probe/probe.c`（由白名单自动生成，逐符号取地址）
   在 windows-latest 上用 7 个 SDK 的头文件分别 `cl /c` 编译。

<!-- BEGIN GENERATED: guards -->
- `AddConsoleAliasA` @ 1507 `um/wincon.h:928` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasA` @ 1511 `um/wincon.h:929` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasA` @ 1607 `um/wincon.h:932` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasA` @ 1703 `um/wincon.h:944` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `AddConsoleAliasA` @ 1709 `um/wincon.h:944` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `AddConsoleAliasA` @ 1803 `um/consoleapi3.h:188` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasA` @ 1809 `um/consoleapi3.h:188` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasW` @ 1507 `um/wincon.h:935` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasW` @ 1511 `um/wincon.h:936` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasW` @ 1607 `um/wincon.h:939` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasW` @ 1703 `um/wincon.h:951` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `AddConsoleAliasW` @ 1709 `um/wincon.h:951` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `AddConsoleAliasW` @ 1803 `um/consoleapi3.h:197` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `AddConsoleAliasW` @ 1809 `um/consoleapi3.h:197` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `AllocConsole` @ 1507 `um/consoleapi.h:48` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `AllocConsole` @ 1511 `um/consoleapi.h:48` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `AllocConsole` @ 1607 `um/consoleapi.h:48` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `AllocConsole` @ 1703 `um/consoleapi.h:49` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `AllocConsole` @ 1709 `um/consoleapi.h:49` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `AllocConsole` @ 1803 `um/consoleapi.h:32` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `AllocConsole` @ 1809 `um/consoleapi.h:33` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `AttachConsole` @ 1507 `um/wincon.h:749` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `AttachConsole` @ 1511 `um/wincon.h:750` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `AttachConsole` @ 1607 `um/wincon.h:753` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `AttachConsole` @ 1703 `um/wincon.h:753` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `AttachConsole` @ 1709 `um/wincon.h:753` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `AttachConsole` @ 1803 `um/consoleapi.h:50` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `AttachConsole` @ 1809 `um/consoleapi.h:51` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `ClosePseudoConsole` @ 1809 `um/consoleapi.h:324` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (NTDDI_VERSION >= NTDDI_WIN10_RS5)`
- `CreateConsoleScreenBuffer` @ 1507 `um/wincon.h:854` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `CreateConsoleScreenBuffer` @ 1511 `um/wincon.h:855` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `CreateConsoleScreenBuffer` @ 1607 `um/wincon.h:858` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `CreateConsoleScreenBuffer` @ 1703 `um/wincon.h:858` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `CreateConsoleScreenBuffer` @ 1709 `um/wincon.h:858` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `CreateConsoleScreenBuffer` @ 1803 `um/consoleapi2.h:104` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `CreateConsoleScreenBuffer` @ 1809 `um/consoleapi2.h:104` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `CreatePseudoConsole` @ 1809 `um/consoleapi.h:303` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM) / (NTDDI_VERSION >= NTDDI_WIN10_RS5)`
- `ExpungeConsoleCommandHistoryA` @ 1803 `um/consoleapi3.h:326` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `ExpungeConsoleCommandHistoryA` @ 1809 `um/consoleapi3.h:326` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `ExpungeConsoleCommandHistoryW` @ 1803 `um/consoleapi3.h:333` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `ExpungeConsoleCommandHistoryW` @ 1809 `um/consoleapi3.h:333` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1507 `um/wincon.h:526` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1511 `um/wincon.h:527` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1607 `um/wincon.h:530` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1703 `um/wincon.h:530` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1709 `um/wincon.h:530` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1803 `um/consoleapi2.h:83` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputAttribute` @ 1809 `um/consoleapi2.h:83` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1507 `um/wincon.h:500` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1511 `um/wincon.h:501` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1607 `um/wincon.h:504` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1703 `um/wincon.h:504` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1709 `um/wincon.h:504` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1803 `um/consoleapi2.h:55` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterA` @ 1809 `um/consoleapi2.h:55` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1507 `um/wincon.h:510` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1511 `um/wincon.h:511` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1607 `um/wincon.h:514` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1703 `um/wincon.h:514` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1709 `um/wincon.h:514` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1803 `um/consoleapi2.h:66` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FillConsoleOutputCharacterW` @ 1809 `um/consoleapi2.h:66` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1507 `um/wincon.h:661` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1511 `um/wincon.h:662` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1607 `um/wincon.h:665` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1703 `um/wincon.h:665` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1709 `um/wincon.h:665` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1803 `um/consoleapi2.h:124` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FlushConsoleInputBuffer` @ 1809 `um/consoleapi2.h:124` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1507 `um/wincon.h:742` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1511 `um/wincon.h:743` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1607 `um/wincon.h:746` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1703 `um/wincon.h:746` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1709 `um/wincon.h:746` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1803 `um/consoleapi.h:40` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `FreeConsole` @ 1809 `um/consoleapi.h:41` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1507 `um/wincon.h:735` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1511 `um/wincon.h:736` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1607 `um/wincon.h:739` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1703 `um/wincon.h:739` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1709 `um/wincon.h:739` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1803 `um/consoleapi2.h:95` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GenerateConsoleCtrlEvent` @ 1809 `um/consoleapi2.h:95` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleAliasA` @ 1507 `um/wincon.h:948` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasA` @ 1511 `um/wincon.h:949` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasA` @ 1607 `um/wincon.h:952` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasA` @ 1703 `um/wincon.h:964` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasA` @ 1709 `um/wincon.h:964` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasA` @ 1803 `um/consoleapi3.h:212` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasA` @ 1809 `um/consoleapi3.h:212` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesA` @ 1507 `um/wincon.h:1022` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesA` @ 1511 `um/wincon.h:1023` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesA` @ 1607 `um/wincon.h:1026` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesA` @ 1703 `um/wincon.h:1038` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesA` @ 1709 `um/wincon.h:1038` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesA` @ 1803 `um/consoleapi3.h:302` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesA` @ 1809 `um/consoleapi3.h:302` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthA` @ 1507 `um/wincon.h:986` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthA` @ 1511 `um/wincon.h:987` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthA` @ 1607 `um/wincon.h:990` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthA` @ 1703 `um/wincon.h:1002` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesLengthA` @ 1709 `um/wincon.h:1002` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesLengthA` @ 1803 `um/consoleapi3.h:258` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthA` @ 1809 `um/consoleapi3.h:258` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthW` @ 1507 `um/wincon.h:991` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthW` @ 1511 `um/wincon.h:992` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthW` @ 1607 `um/wincon.h:995` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthW` @ 1703 `um/wincon.h:1007` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesLengthW` @ 1709 `um/wincon.h:1007` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesLengthW` @ 1803 `um/consoleapi3.h:265` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesLengthW` @ 1809 `um/consoleapi3.h:265` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesW` @ 1507 `um/wincon.h:1028` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesW` @ 1511 `um/wincon.h:1029` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesW` @ 1607 `um/wincon.h:1032` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesW` @ 1703 `um/wincon.h:1044` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesW` @ 1709 `um/wincon.h:1044` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasExesW` @ 1803 `um/consoleapi3.h:310` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasExesW` @ 1809 `um/consoleapi3.h:310` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasW` @ 1507 `um/wincon.h:956` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasW` @ 1511 `um/wincon.h:957` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasW` @ 1607 `um/wincon.h:960` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasW` @ 1703 `um/wincon.h:972` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasW` @ 1709 `um/wincon.h:972` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasW` @ 1803 `um/consoleapi3.h:222` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasW` @ 1809 `um/consoleapi3.h:222` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesA` @ 1507 `um/wincon.h:1002` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesA` @ 1511 `um/wincon.h:1003` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesA` @ 1607 `um/wincon.h:1006` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesA` @ 1703 `um/wincon.h:1018` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesA` @ 1709 `um/wincon.h:1018` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesA` @ 1803 `um/consoleapi3.h:278` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesA` @ 1809 `um/consoleapi3.h:278` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthA` @ 1507 `um/wincon.h:970` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthA` @ 1511 `um/wincon.h:971` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthA` @ 1607 `um/wincon.h:974` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthA` @ 1703 `um/wincon.h:986` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesLengthA` @ 1709 `um/wincon.h:986` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesLengthA` @ 1803 `um/consoleapi3.h:238` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthA` @ 1809 `um/consoleapi3.h:238` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthW` @ 1507 `um/wincon.h:975` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthW` @ 1511 `um/wincon.h:976` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthW` @ 1607 `um/wincon.h:979` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthW` @ 1703 `um/wincon.h:991` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesLengthW` @ 1709 `um/wincon.h:991` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesLengthW` @ 1803 `um/consoleapi3.h:245` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesLengthW` @ 1809 `um/consoleapi3.h:245` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesW` @ 1507 `um/wincon.h:1009` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesW` @ 1511 `um/wincon.h:1010` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesW` @ 1607 `um/wincon.h:1013` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesW` @ 1703 `um/wincon.h:1025` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesW` @ 1709 `um/wincon.h:1025` guard: `_WINCON_ / (_WIN32_WINNT >= 0x0501) / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP)`
- `GetConsoleAliasesW` @ 1803 `um/consoleapi3.h:287` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleAliasesW` @ 1809 `um/consoleapi3.h:287` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0501)`
- `GetConsoleCP` @ 1507 `um/consoleapi.h:56` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCP` @ 1511 `um/consoleapi.h:56` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCP` @ 1607 `um/consoleapi.h:56` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCP` @ 1703 `um/consoleapi.h:57` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCP` @ 1709 `um/consoleapi.h:57` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCP` @ 1803 `um/consoleapi.h:62` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCP` @ 1809 `um/consoleapi.h:63` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryA` @ 1803 `um/consoleapi3.h:388` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryA` @ 1809 `um/consoleapi3.h:388` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryLengthA` @ 1803 `um/consoleapi3.h:368` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryLengthA` @ 1809 `um/consoleapi3.h:368` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryLengthW` @ 1803 `um/consoleapi3.h:375` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryLengthW` @ 1809 `um/consoleapi3.h:375` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryW` @ 1803 `um/consoleapi3.h:397` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCommandHistoryW` @ 1809 `um/consoleapi3.h:397` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1507 `um/wincon.h:569` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1511 `um/wincon.h:570` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1607 `um/wincon.h:573` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1703 `um/wincon.h:573` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1709 `um/wincon.h:573` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1803 `um/consoleapi2.h:153` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleCursorInfo` @ 1809 `um/consoleapi2.h:153` guard: `_APISETCONSOLEL2_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleDisplayMode` @ 1507 `um/wincon.h:890` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleDisplayMode` @ 1511 `um/wincon.h:891` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleDisplayMode` @ 1607 `um/wincon.h:894` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleDisplayMode` @ 1703 `um/wincon.h:894` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleDisplayMode` @ 1709 `um/wincon.h:894` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleDisplayMode` @ 1803 `um/consoleapi3.h:155` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleDisplayMode` @ 1809 `um/consoleapi3.h:155` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1507 `um/wincon.h:624` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1511 `um/wincon.h:625` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1607 `um/wincon.h:628` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1703 `um/wincon.h:628` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1709 `um/wincon.h:628` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1803 `um/consoleapi3.h:47` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleFontSize` @ 1809 `um/consoleapi3.h:47` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1507 `um/wincon.h:612` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1511 `um/wincon.h:613` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1607 `um/wincon.h:616` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1703 `um/wincon.h:616` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1709 `um/wincon.h:616` guard: `_WINCON_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1803 `um/consoleapi3.h:136` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleHistoryInfo` @ 1809 `um/consoleapi3.h:136` guard: `_APISETCONSOLEL3_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_APP | WINAPI_PARTITION_SYSTEM) / (_WIN32_WINNT >= 0x0500)`
- `GetConsoleMode` @ 1507 `um/consoleapi.h:64` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleMode` @ 1511 `um/consoleapi.h:64` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleMode` @ 1607 `um/consoleapi.h:64` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
- `GetConsoleMode` @ 1703 `um/consoleapi.h:65` guard: `_APISETCONSOLE_ / WINAPI_FAMILY_PARTITION(WINAPI_PARTITION_DESKTOP | WINAPI_PARTITION_SYSTEM)`
<!-- END GENERATED: guards -->
