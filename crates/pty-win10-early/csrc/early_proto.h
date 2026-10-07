/* early_proto.h — 早期桥命名管道帧协议（docs/ipc-signal-protocol.md §3/§4 的 C 侧）
 *
 * conhook.dll（第二刀）与宿主 Rust 侧共享的常量/布局；CI 以 7 版 SDK
 * 逐个 cl /c 编译验证头文件兼容性（纯 ucrt，无 Win32 依赖）。
 * 布局：[len u32 BE][type u8][payload len-1 B]，len 含 type 字节。
 * 最近更新：2026-10-06。
 */
#pragma once

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define WBWTTY_EARLY_PROTO_VER      1u
#define WBWTTY_EARLY_MAX_FRAME      (1u * 1024u * 1024u) /* 1 MiB 上限 */
#define WBWTTY_EARLY_HDR_LEN        5u                   /* len(4) + type(1) */

/* 消息类型（ipc-signal-protocol.md §4；0x09+ 保留，0x80+ vendor） */
#define WBWTTY_EARLY_T_HELLO        0x00u /* 首帧：ver u16 LE + flags u16 LE */
#define WBWTTY_EARLY_T_VT_DATA      0x01u
#define WBWTTY_EARLY_T_RESIZE       0x02u /* cols u16 LE + rows u16 LE */
#define WBWTTY_EARLY_T_SIG_INT      0x03u
#define WBWTTY_EARLY_T_SIG_TERM     0x04u
#define WBWTTY_EARLY_T_PING         0x05u
#define WBWTTY_EARLY_T_PONG         0x06u
#define WBWTTY_EARLY_T_EXIT         0x07u /* code i32 LE */
#define WBWTTY_EARLY_T_ERR          0x08u /* UTF-8 消息 */

/* 帧总长（含头）；payload 上限超限返回 0（调用方视为协议错误）。 */
size_t wbw_early_frame_size(size_t payload_len);

/* 大端写入 len 头到 dst（至少 WBWTTY_EARLY_HDR_LEN 字节）。
 * 校验：1 + payload_len <= WBWTTY_EARLY_MAX_FRAME；失败返回 0，成功返回 1。 */
int wbw_early_write_header(uint8_t *dst, uint8_t type, uint32_t payload_len);

/* 解析已读到的 4 字节大端 len；非法（0 或 > MAX）返回 0。 */
int wbw_early_parse_len(const uint8_t *p, uint32_t *out_len);

#ifdef __cplusplus
}
#endif
