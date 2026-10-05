# pty-conpty

ConPTY 伪终端后端（Windows 10 1809+，全部动态加载）；非 Windows 平台为可编译空壳。
最近更新：2026-10-05。

## 实现要点

| 环节 | 做法 |
| --- | --- |
| 加载 | `ConptyApi` 运行时 `GetModuleHandleW`（UTF-16 宽串）+ `GetProcAddress` 从 kernel32 **与** kernelbase 解析 `CreatePseudoConsole`/`ResizePseudoConsole`/`ClosePseudoConsole`；失败 → `SpawnError::BackendUnavailable`（上层回退其他后端） |
| 管道 | 两根匿名管道：in（我们写、ConPTY 读）、out（ConPTY 写、我们读）；`CreatePseudoConsole` 后立即关 PTY 端句柄 |
| 子进程挂接 | `STARTUPINFOEXW` + `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE = 0x00020016` 属性列表（`AttributeList` RAII，std::alloc 16 对齐） |
| **标准句柄** | `StartupInfo.dwFlags |= STARTF_USESTDHANDLES` 且 `hStdInput/hStdOutput/hStdError = NULL`（microsoft/terminal#4380 issuecomment-580865346 官方建议，zhiburt/conpty 同款）。**不这么做的后果**：子进程（cmd 等）继承父进程 std 句柄——测试 harness/CI/调试器下父 stdout 是管道——`cmd /c echo` 直接把输出写进父管道，ConPTY 输出管道 0 字节 → read 永久阻塞挂死 |
| 命令行 | `cmdline::build_command_line`：Windows 反斜杠引号规则 + 环境块（排序、双 NUL、按 `SpawnOptions::env` 覆盖同名键） |
| 读写 | 输出：`PeekNamedPipe` 10ms 轮询读（无数据不阻塞，避免 ConPTY 未及时写入时卡死）；输入直写 in 管道 |
| Ctrl+C | `Signal::Interrupt` → 写 `ETX (0x03)` 到 in 管道（ConPTY 翻译为 CTRL_C_EVENT），其余信号映射对应控制码 |
| 尺寸/关闭 | `ResizePseudoConsole`；`close()` 幂等（关管道 + `ClosePseudoConsole`），关闭后 read → `Ok(0)`、write → `BrokenPipe`；`Drop` 兜底 |
| Send | 句柄以 `isize` 保存，`ConptyPty: Send` |

注册：`pty_conpty::register()` → priority **20**（见 `pty-core` README 的选择规则）。
`is_available()` = ConPTY 三函数加载成功（Win10 1809+）。

非 Windows 平台：`is_available()` → false、`register()` 空操作（占位，供统一调用方编译）。

## 测试

`cargo test -p pty-conpty`（Windows）：11 单元（引号/环境块 7、结构体布局与导出解析 4，
含 `probe_output_pipe_receives_child_output` 3 秒快速回归护栏——断言子进程输出确实落在
ConPTY 管道上，防止标准句柄修法被回退）+ 6 集成（`tests/spawn_conpty.rs`，每个带 60s
看门狗）：echo 输出与退出码、resize+交互回显、cwd+env、Ctrl+C 中断、注册可用性、
关闭幂等。
