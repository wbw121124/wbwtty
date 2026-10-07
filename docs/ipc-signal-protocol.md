# IPC 与信号协议（early 桥接命名管道）

- 最近更新：2026-10-06（阶段 4 开工随 win10-early-bridge.md 一并定稿）
- 关联：`docs/architecture.md` §6、`docs/win10-early-bridge.md` §3.1（第二刀）、
  `docs/api-whitelist.md`（file-pipe / console-host 组）

## 1. 适用范围

本协议定义 **conhook.dll（child 内）↔ 宿主（EarlyPty 宿主进程）** 之间的
命名管道帧格式与消息语义，用于路径 A 第二刀（注入版）的事件驱动输出与
带外控制。

- **第一刀（轮询共享控制台）不建立管道**——协议先行定稿，第二刀直接落地，
  避免接口返工。
- 管道字节流方向：child 侧 conhook = client（主动连出），宿主 = server
  （`CreateNamedPipeW` + `ConnectNamedPipe` 阻塞接受，file-pipe 组白名单）。
- 前端（term-app/vt-parser）**不直接接触本协议**：宿主拆帧后把 `VT_DATA`
  payload 拼入 `Pty::read` 字节流，对外表现为普通 PTY 输出。

## 2. 端点命名

```
\\.\pipe\wbwtty-early-<hostPid>-<nonce>
```

- `hostPid`：宿主进程 PID；`nonce`：spawn 时生成的随机 u64（16 hex），
  防同宿主多实例串线（第二刀支持并发实例后隔离用）。
- 传递方式：注入前经环境变量 `WBWTTY_EARLY_PIPE=<name>` 写入 child 环境块
  （`build_environment` 同款 UTF-16 双 NUL 构造），conhook 在 `DllMain` 后
  首次使用时读取。
- 超时：宿主 `ConnectNamedPipe` 等待 conhook 就绪，上限 5s；超时 → 视为注入
  失败，走清理/回退（杀 child、报 `BackendUnavailable`）。

## 3. 帧格式

```
+--------+--------+------------------+
| len u32| type u8| payload (len-1 B)|
+--------+--------+------------------+
```

- `len`：**大端 u32**，值 = `1 + payload_len`（含 type 字节，便于单次
  `type+payload` 读取）；上限 `1 MiB`（超限即断管，视为协议错误）。
- `type`：u8，见 §4。
- `payload`：按 type 定义，均为小端整数 / UTF-8 字节。
- 无 payload 的消息 `len = 1`。
- 单帧原子性：管道消息写入（`WriteFile` ≤ 一帧）保证不拆帧。

## 4. 消息类型

| type | 名称 | payload | 方向 | 语义 |
|---|---|---|---|---|
| 0x01 | `VT_DATA` | 合成 VT 字节（UTF-8） | conhook → 宿主 | 屏幕 diff 合成的输出流，宿主并入 `Pty::read` |
| 0x02 | `RESIZE` | `cols u16` + `rows u16`（小端） | 宿主 → conhook | 宿主已改控制台尺寸，conhook 重读缓冲维度 |
| 0x03 | `SIG_INT` | 空 | 宿主 → conhook | 要求 child 收 Ctrl+C（conhook 在 child 内注入键事件或宿主直接走控制台 API——由实施定，见 §5） |
| 0x04 | `SIG_TERM` | 空 | 宿主 → conhook | 要求终止 child |
| 0x05 | `PING` | 空 | 宿主 → conhook | 保活探测 |
| 0x06 | `PONG` | 空 | conhook → 宿主 | 应答（宿主 3s 无应答 → 断管处理） |
| 0x07 | `EXIT` | `code i32`（小端） | conhook → 宿主 | child 退出，**此后 conhook 必须发完残留帧再关管**；宿主据此产出 EOF |
| 0x08 | `ERR` | UTF-8 错误消息 | 双向 | 非致命错误上报（hook 恢复失败等），宿主记日志 |
| 0x00 | `HELLO` | `proto_ver u16`（=1）+ `flags u16` | conhook → 宿主 | 连接后首帧；版本不符宿主断管（向后兼容靠版本协商） |

- 保留 0x09–0x7F 给本协议扩展；0x80+ 留给未来 vendor 扩展。
- 未知 type：接收方丢弃该帧并记 `ERR`（不断管，保证向前兼容）。

## 5. 信号映射细则

| 语义 | 第一刀（无管道） | 第二刀（有管道） |
|---|---|---|
| `Signal::Interrupt` | 宿主 `GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)` 广播（子进程生成前清继承忽略标志、生成后宿主置忽略） | 宿主仍直接操作（控制台 API 在宿主侧可用时优先，conhook 仅在"宿主已脱离控制台"部署形态下用 `SIG_INT` 让 conhook 在 child 内注入） |
| `Signal::Term`/`Kill` | `TerminateProcess(child,1)` | 同左（宿主持有进程句柄；`SIG_TERM` 帧仅用于 conhook 自持句柄的部署形态，首版不用） |
| `Signal::Hangup` | 同 `Term`，close 时幂等 | 同左 |
| resize | 直接 `SetConsoleWindowInfo` + `SetConsoleScreenBufferSize` | 同左 + 补发 `RESIZE` 帧让 conhook 重读 |
| `Signal::Winch` | 无操作（resize 即触发） | 同左 |

- 设计原则：**控制台 API 宿主能做的就不开帧**——帧协议只承担"输出上行 +
  通知"职责，控制下行保持调用式（少一个可失败的 IPC 面）。
- 与 `architecture.md` §6 的差异说明：原文写"SIGINT 按
  `GenerateConsoleCtrlEvent(CTRL_C_EVENT, groupId)` 映射"；阶段 3 实证定点组路径
  （`CREATE_NEW_PROCESS_GROUP` + 非零 groupId）被继承性忽略、键事件注入路径
  conhost 不做急切转换，故首版用**组 0 广播**，配对顺序约束：
  `SetConsoleCtrlHandler(NULL, FALSE)` 在子进程生成前、`(NULL, TRUE)` 在生成后
  （`CreateProcessW` 之后），否则子进程继承忽略标志、广播成 no-op。

## 6. 关闭与错误语义

- 正常关闭顺序：conhook 发 `EXIT(code)` → flush → `CloseHandle`；宿主读到
  `EXIT` → 拆完缓冲 → `Pty::read` 转 `Ok(0)`（EOF）→ 等 `try_wait` 收割。
- 异常断管（无 `EXIT`）：宿主 `WaitForSingleObject(child)` 判活；已退出 →
  按 EOF 处理，未退出 → `read` 返回 `Err(BrokenPipe)`（上层可重连或杀 child）。
- 宿主侧先死：conhook 下次 `WriteFile` 失败 → conhook 撤销 hook、
  恢复原函数指针、`FreeLibraryAndExitThread`（child 继续正常运行，
  输出回到原始控制台——降级而非崩溃）。
- 所有句柄路径归一到 `release()` 式清理（同 pty-conpty 惯例）。

## 7. 版本与演进

- `HELLO.proto_ver` 起始 = 1；不兼容变更递增并使旧宿主拒连。
- 新消息只加 type 不改旧 payload；`len` 上限与帧头布局永不变更。
- 协议由宿主侧（Rust）与 conhook（C, `csrc/conhook.c`）双向实现，
  单测：Rust 拆帧器 fuzz（截断/超限/未知 type）+ C 侧 smoke（CI per-SDK
  编译通过即视为可链接，运行验证在第二刀集成测试）。
