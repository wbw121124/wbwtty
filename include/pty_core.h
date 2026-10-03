/*
 * pty_core.h — PTY 后端 C ABI（与 crates/pty-core/src/ffi.rs 对应）
 *
 * 约定：
 *   - 返回 0 成功、-1 失败；pty_read 例外：>0 读取字节数、0 EOF、-1 失败。
 *   - pty_last_error() 返回线程局部的最近一次失败描述（NUL 结尾）。
 *   - pty_spawn 分配的句柄必须用 pty_close 释放；pty_close 后句柄不可再用。
 *   - argv 必须 NULL 结尾（argv[0] 为程序名），元素在调用期间保持有效。
 *   - cwd 可为 NULL（继承父进程目录）；环境继承父进程（当前 ABI 不设环境表）。
 */
#ifndef PTY_CORE_H
#define PTY_CORE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct pty_handle pty_handle;

/* 与 Rust 侧 Signal 枚举序号一致 */
typedef enum pty_signal {
    PTY_SIG_INT = 0,
    PTY_SIG_TERM = 1,
    PTY_SIG_QUIT = 2,
    PTY_SIG_HUP = 3,
    PTY_SIG_KILL = 4
} pty_signal;

/* 启动：成功返回 0，失败返回 -1（详见 pty_last_error）。 */
int pty_spawn(const char* const argv[], const char* cwd,
              uint16_t cols, uint16_t rows, pty_handle** out);

/* 读：>0 字节数、0 EOF、-1 失败。 */
int64_t pty_read(pty_handle* h, uint8_t* buf, size_t len);

/* 写：0 成功（*written = 实际写入），-1 失败。 */
int pty_write(pty_handle* h, const uint8_t* buf, size_t len, size_t* written);

/* 调整窗口尺寸：0 成功、-1 失败。 */
int pty_resize(pty_handle* h, uint16_t cols, uint16_t rows);

/* 信号：0 成功、-1 失败。 */
int pty_signal(pty_handle* h, pty_signal sig);

/* 关闭并释放句柄：0 成功、-1 失败。 */
int pty_close(pty_handle* h);

/* pty_spawn 启动的子进程 PID（0 = 未知）。 */
uint32_t pty_child_pid(void);

/* 最近一次失败描述（线程局部，NUL 结尾；无错误时为空串）。 */
const char* pty_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* PTY_CORE_H */
