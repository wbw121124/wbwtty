/* early_proto.h 实现（纯 ucrt，供 7 版 SDK cl /c 编译验证）。最近更新：2026-10-06。 */
/* 与 conhook.c 保持一致：不 include <stdio.h>（旧 SDK ucrt 头与新 SDK corecrt 混用会报错），
 * 但必须本地声明 vsnprintf/snprintf，否则 cl 隐式当成 int vsnprintf(...) 导致 LNK2001。
 * 声明放在 #include 之后，确保 stddef/stdarg 已定义 size_t/va_list。 */
#include "early_proto.h"
int vsnprintf(char *buf, size_t n, const char *fmt, va_list ap);
int snprintf(char *buf, size_t n, const char *fmt, ...);

size_t wbw_early_frame_size(size_t payload_len) {
    if (payload_len + 1u > WBWTTY_EARLY_MAX_FRAME) {
        return 0;
    }
    return (size_t)WBWTTY_EARLY_HDR_LEN + payload_len;
}

int wbw_early_write_header(uint8_t *dst, uint8_t type, uint32_t payload_len) {
    uint32_t len;
    if (dst == NULL) {
        return 0;
    }
    if ((uint64_t)payload_len + 1u > (uint64_t)WBWTTY_EARLY_MAX_FRAME) {
        return 0;
    }
    len = payload_len + 1u; /* 含 type 字节 */
    dst[0] = (uint8_t)((len >> 24) & 0xFFu);
    dst[1] = (uint8_t)((len >> 16) & 0xFFu);
    dst[2] = (uint8_t)((len >> 8) & 0xFFu);
    dst[3] = (uint8_t)(len & 0xFFu);
    dst[4] = type;
    return 1;
}

int wbw_early_parse_len(const uint8_t *p, uint32_t *out_len) {
    uint32_t len;
    if (p == NULL || out_len == NULL) {
        return 0;
    }
    len = ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
          ((uint32_t)p[2] << 8) | (uint32_t)p[3];
    if (len == 0 || len > WBWTTY_EARLY_MAX_FRAME) {
        return 0;
    }
    *out_len = len;
    return 1;
}
