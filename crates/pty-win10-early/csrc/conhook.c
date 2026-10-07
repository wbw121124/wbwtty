/* conhook.dll — 第二刀子进程钩子：控制台 API 镜像到虚拟网格 + 命名管道客户端
 *
 * 设计依据 docs/win10-early-bridge.md §3.1/§3.6/§6 与 ipc-signal-protocol.md。
 * 线程模型：
 *   DllMain（远程 LoadLibrary 线程，子进程主模块仍挂起）：
 *     初始化 CS → 解析原始函数指针（GetProcAddress）→ 开 CONOUT$ →
 *     从真实缓冲 resync 网格 → IAT 修补主模块 → 起 worker（窗口期零丢字）
 *   worker：连管道 → 发 HELLO → 立即全量帧 → 循环收 RESIZE/PING →
 *     断管/失败 → 卸 IAT 补丁（DLL 常驻，不做 FreeLibrary 避免卸载竞态）
 *   钩子回调（子进程任意线程）：先调真实 API → 镜像进网格 → diff 合成 VT
 *     → 单条 WriteFile 消息发出（cs 串行化全部管道写）。
 * 网格是前端唯一数据源；真实缓冲与它的差异由“真实 API 先行 + 同规则镜像”
 * 收敛，ReadConsoleW 回合后 resync 全量校准（回显等旁路写入）。
 * 命名约定：本文件所有本地函数/宏为 snake_case（tools/check_api_whitelist.py R3）。
 * 最近更新：2026-10-07。
 */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

/* 不含 ucrt 头（stdio/stdlib/string）：SDK 矩阵把旧版 UCRT 头与 runner
 * 新版 UCRT corecrt_* 混编会直接语法爆炸（ci.yml /I 旧快照 ucrt 优先）。
 * stdarg/stdint 由编译器（VC 工具链）提供、stddef 旧快照内是纯 typedef，
 * 均安全；下列 CRT 函数改为手写等价声明。
 * vsnprintf/snprintf 声明同时保证 mingw gcc（-static-libgcc 无 ucrt）
 * 和 MSVC /MD（cl 隐式 int 声明与 ucrt 导出符号一致）可链接。 */
void *malloc(size_t size);
void *calloc(size_t nmemb, size_t size);
void free(void *ptr);
void *memcpy(void *dst, const void *src, size_t n);
void *memset(void *dst, int c, size_t n);
int memcmp(const void *a, const void *b, size_t n);
size_t strlen(const char *s);
int vsnprintf(char *buf, size_t n, const char *fmt, va_list ap);
int snprintf(char *buf, size_t n, const char *fmt, ...);

#include "early_proto.h"

#define GRID_MAX_CELLS 1048576
#define IAT_MAX_SLOTS 32

/* ---- 原始 API 函数指针类型（与 SDK 原型一致） ------------------------- */
typedef BOOL(WINAPI *t_write_console_w)(HANDLE, CONST VOID *, DWORD, LPDWORD, LPVOID);
typedef BOOL(WINAPI *t_fill_char_w)(HANDLE, WCHAR, DWORD, COORD, LPDWORD);
typedef BOOL(WINAPI *t_fill_attr_w)(HANDLE, WORD, DWORD, COORD, LPDWORD);
typedef BOOL(WINAPI *t_set_cursor)(HANDLE, COORD);
typedef BOOL(WINAPI *t_set_attr)(HANDLE, WORD);
typedef BOOL(WINAPI *t_scroll_w)(HANDLE, CONST SMALL_RECT *, CONST SMALL_RECT *, COORD,
                                 CONST CHAR_INFO *);
typedef BOOL(WINAPI *t_read_console_w)(HANDLE, LPVOID, DWORD, LPDWORD, LPDWORD);
typedef BOOL(WINAPI *t_get_csbi)(HANDLE, PCONSOLE_SCREEN_BUFFER_INFO);
typedef BOOL(WINAPI *t_read_console_out_w)(HANDLE, PCHAR_INFO, COORD, COORD, PSMALL_RECT);
typedef FARPROC(WINAPI *t_get_proc_address)(HMODULE, LPCSTR);

typedef struct cell_s {
    WCHAR ch;
    WORD attr;
} cell_t;

typedef struct iat_slot_s {
    void **slot;
    void *orig;
} iat_slot_t;

typedef struct hook_entry_s {
    const char *name;
    FARPROC hook;
} hook_entry_t;

static struct {
    HMODULE self;
    CRITICAL_SECTION cs;
    HANDLE pipe;      /* cs 保护；NULL = 未连接 */
    HANDLE hout;      /* CONOUT$，resync 用 */
    LONG pipe_dead;   /* 写失败置位，钩子停止发帧 */
    cell_t *cur;      /* cs 保护：当前网格 */
    int gw, gh;
    cell_t *prev;     /* cs 保护：上一帧快照 */
    int pw, ph;
    int has_prev;
    int cx, cy;
    WORD attr;
    t_write_console_w p_write;
    t_fill_char_w p_fill_char_w;
    t_fill_attr_w p_fill_attr_w;
    t_set_cursor p_set_cursor;
    t_set_attr p_set_attr;
    t_scroll_w p_scroll_w;
    t_read_console_w p_read_console_w;
    t_get_csbi p_get_csbi;
    t_read_console_out_w p_read_console_out_w;
    t_get_proc_address p_get_proc_address;
    iat_slot_t iats[IAT_MAX_SLOTS];
    int niats;
} g;

static DWORD WINAPI worker(LPVOID unused);
static void patch_iat(void);
static void unpatch_iat(void);
static void resync_full(void);
static void emit_diff(void);

/* 钩子表：IAT 修补与 GetProcAddress 包装共用（名字须与导出名逐字一致） */
static BOOL WINAPI hook_write_console_w(HANDLE h, CONST VOID *buf, DWORD n,
                                        LPDWORD written, LPVOID reserved);
static BOOL WINAPI hook_fill_char_w(HANDLE h, WCHAR ch, DWORD n, COORD start,
                                    LPDWORD written);
static BOOL WINAPI hook_fill_attr_w(HANDLE h, WORD attr, DWORD n, COORD start,
                                    LPDWORD written);
static BOOL WINAPI hook_set_cursor(HANDLE h, COORD pos);
static BOOL WINAPI hook_set_attr(HANDLE h, WORD attr);
static BOOL WINAPI hook_scroll_w(HANDLE h, CONST SMALL_RECT *scroll,
                                 CONST SMALL_RECT *clip, COORD dest,
                                 CONST CHAR_INFO *fill);
static BOOL WINAPI hook_read_console_w(HANDLE h, LPVOID buf, DWORD n, LPDWORD read,
                                       LPDWORD nread);
static FARPROC WINAPI hook_get_proc_address(HMODULE mod, LPCSTR name);

static const hook_entry_t k_hooks[] = {
    {"WriteConsoleW", (FARPROC)hook_write_console_w},
    {"FillConsoleOutputCharacterW", (FARPROC)hook_fill_char_w},
    {"FillConsoleOutputAttribute", (FARPROC)hook_fill_attr_w},
    {"SetConsoleCursorPosition", (FARPROC)hook_set_cursor},
    {"SetConsoleTextAttribute", (FARPROC)hook_set_attr},
    {"ScrollConsoleScreenBufferW", (FARPROC)hook_scroll_w},
    {"ReadConsoleW", (FARPROC)hook_read_console_w},
    {"GetProcAddress", (FARPROC)hook_get_proc_address},
    {NULL, NULL},
};

/* ---- 小工具 ------------------------------------------------------------ */

static int str_ieq(const char *a, const char *b) {
    while (*a && *b) {
        int ca = (unsigned char)*a;
        int cb = (unsigned char)*b;
        if (ca >= 'A' && ca <= 'Z') {
            ca += 32;
        }
        if (cb >= 'A' && cb <= 'Z') {
            cb += 32;
        }
        if (ca != cb) {
            return 0;
        }
        a++;
        b++;
    }
    return *a == *b;
}

/* 字符串缓冲（emit_diff 组帧；容量按网格上限估算，失败即丢帧不打断子进程） */
typedef struct sbuf_s {
    char *p;
    size_t len;
    size_t cap;
    int fail;
} sbuf_t;

static void sb_init(sbuf_t *b, size_t cap) {
    b->p = (char *)malloc(cap);
    b->len = 0;
    b->cap = cap;
    b->fail = (b->p == NULL);
}

static void sb_putn(sbuf_t *b, const char *s, size_t n) {
    if (b->fail) {
        return;
    }
    if (b->len + n + 1 > b->cap) {
        b->fail = 1;
        return;
    }
    memcpy(b->p + b->len, s, n);
    b->len += n;
}

static void sb_puts(sbuf_t *b, const char *s) {
    sb_putn(b, s, strlen(s));
}

static void sb_fmt(sbuf_t *b, const char *fmt, ...) {
    va_list ap;
    int n;
    if (b->fail) {
        return;
    }
    va_start(ap, fmt);
    n = vsnprintf(b->p + b->len, b->cap - b->len, fmt, ap);
    va_end(ap);
    if (n < 0 || (size_t)n >= b->cap - b->len) {
        b->fail = 1;
        return;
    }
    b->len += (size_t)n;
}

/* ---- VT 合成（Rust vt_synth.rs 的 C 移植，输出规则逐条对齐） ----------- */

static void ansi_color_str(int bits, int bg, char *out, size_t cap) {
    int idx = ((bits & 4) ? 1 : 0) + ((bits & 2) ? 2 : 0) + ((bits & 1) ? 4 : 0);
    int base = (bits & 8) ? (bg ? 100 : 90) : (bg ? 40 : 30);
    snprintf(out, cap, "%d", base + idx);
}

static void sgr_to_str(WORD attrs, char *out, size_t cap) {
    char fg[8];
    char bg[8];
    char extra[8];
    extra[0] = '\0';
    ansi_color_str((int)(attrs & 0x000F), 0, fg, sizeof(fg));
    ansi_color_str((int)((attrs >> 4) & 0x000F), 1, bg, sizeof(bg));
    if (attrs & 0x2000) { /* COMMON_LVB_UNDERSCORE */
        snprintf(extra, sizeof(extra), ";4");
    }
    if (attrs & 0x1000) { /* COMMON_LVB_REVERSE_VIDEO */
        size_t el = strlen(extra);
        snprintf(extra + el, sizeof(extra) - el, ";7");
    }
    snprintf(out, cap, "0;%s;%s%s", fg, bg, extra);
}

/* 从 next 单元流取下一个显示码点（代理对合并），返回码点 + 消耗 cell 数 */
static unsigned next_cp(const cell_t *row, int x, int w, int *consumed) {
    unsigned u = row[x].ch;
    if (u >= 0xD800u && u < 0xDC00u) {
        if (x + 1 < w) {
            unsigned lo = row[x + 1].ch;
            if (lo >= 0xDC00u && lo < 0xE000u) {
                *consumed = 2;
                return 0x10000u + (((u - 0xD800u) << 10) | (lo - 0xDC00u));
            }
        }
        *consumed = 1;
        return 0xFFFDu;
    }
    if (u >= 0xDC00u && u < 0xE000u) {
        *consumed = 1;
        return 0xFFFDu;
    }
    if (u >= 0xFFFEu) {
        *consumed = 1;
        return 0xFFFDu;
    }
    *consumed = 1;
    return u;
}

static void put_utf8(sbuf_t *b, unsigned cp) {
    char t[4];
    if (cp < 0x80u) {
        t[0] = (char)cp;
        sb_putn(b, t, 1);
    } else if (cp < 0x800u) {
        t[0] = (char)(0xC0u | (cp >> 6));
        t[1] = (char)(0x80u | (cp & 0x3Fu));
        sb_putn(b, t, 2);
    } else if (cp < 0x10000u) {
        t[0] = (char)(0xE0u | (cp >> 12));
        t[1] = (char)(0x80u | ((cp >> 6) & 0x3Fu));
        t[2] = (char)(0x80u | (cp & 0x3Fu));
        sb_putn(b, t, 3);
    } else {
        t[0] = (char)(0xF0u | (cp >> 18));
        t[1] = (char)(0x80u | ((cp >> 12) & 0x3Fu));
        t[2] = (char)(0x80u | ((cp >> 6) & 0x3Fu));
        t[3] = (char)(0x80u | (cp & 0x3Fu));
        sb_putn(b, t, 4);
    }
}

/* ---- 管道写（cs 持有态调用；单条消息 = 头+载荷一次 WriteFile） --------- */

static int pipe_send_locked(BYTE type, const void *payload, DWORD len) {
    size_t total;
    uint8_t *wire;
    DWORD n = 0;
    BOOL ok;
    if (g.pipe_dead || g.pipe == NULL) {
        return 0;
    }
    total = (size_t)WBWTTY_EARLY_HDR_LEN + len;
    wire = (uint8_t *)malloc(total);
    if (wire == NULL) {
        return 0;
    }
    if (!wbw_early_write_header(wire, type, len)) {
        free(wire);
        return 0;
    }
    if (len > 0) {
        memcpy(wire + WBWTTY_EARLY_HDR_LEN, payload, len);
    }
    ok = WriteFile(g.pipe, wire, (DWORD)total, &n, NULL);
    free(wire);
    if (!ok || (size_t)n != total) {
        g.pipe_dead = 1;
        return 0;
    }
    return 1;
}

/* ---- 网格操作（cs 持有态） --------------------------------------------- */

static void grid_scroll_up(void) {
    if (g.gh <= 1) {
        memset(g.cur, 0, sizeof(cell_t) * (size_t)g.gw);
    } else {
        memmove(g.cur, g.cur + g.gw, sizeof(cell_t) * (size_t)g.gw * (size_t)(g.gh - 1));
    }
    {
        cell_t *last = g.cur + (size_t)g.gw * (size_t)(g.gh - 1);
        int i;
        for (i = 0; i < g.gw; i++) {
            last[i].ch = L' ';
            last[i].attr = g.attr;
        }
    }
}

/* WriteConsoleW 镜像：\r \n \t \b + 行尾换行 + 底行滚动；其余控制字符
 * （含 ESC）不落格——与 conhost 的可见结果一致，且保证网格永不含 ESC，
 * 合成帧不会意外驱动前端终端状态机。 */
static void apply_write(const WCHAR *s, DWORD n) {
    DWORD i;
    if (g.cur == NULL) {
        return;
    }
    for (i = 0; i < n; i++) {
        WCHAR c = s[i];
        if (c == L'\r') {
            g.cx = 0;
        } else if (c == L'\n') {
            if (g.cy + 1 >= g.gh) {
                grid_scroll_up();
            } else {
                g.cy++;
            }
        } else if (c == L'\t') {
            int nx = ((g.cx / 8) + 1) * 8;
            if (nx >= g.gw) {
                nx = 0;
                if (g.cy + 1 >= g.gh) {
                    grid_scroll_up();
                } else {
                    g.cy++;
                }
            }
            g.cx = nx;
        } else if (c == L'\b') {
            if (g.cx > 0) {
                g.cx--;
            } else if (g.cy > 0) {
                g.cy--;
                g.cx = g.gw - 1;
            }
        } else if (c >= L' ') {
            g.cur[(size_t)g.cy * g.gw + g.cx].ch = c;
            g.cur[(size_t)g.cy * g.gw + g.cx].attr = g.attr;
            g.cx++;
            if (g.cx >= g.gw) {
                g.cx = 0;
                if (g.cy + 1 >= g.gh) {
                    grid_scroll_up();
                } else {
                    g.cy++;
                }
            }
        }
    }
}

static void apply_fill(size_t start, DWORD count, int set_char, WCHAR ch, int set_attr,
                       WORD attr) {
    size_t total = (size_t)g.gw * (size_t)g.gh;
    DWORD k;
    for (k = 0; k < count; k++) {
        size_t idx = start + k;
        if (idx >= total) {
            break;
        }
        if (set_char) {
            g.cur[idx].ch = ch;
        }
        if (set_attr) {
            g.cur[idx].attr = attr;
        }
    }
}

static void apply_scroll(const SMALL_RECT *scroll, const SMALL_RECT *clip, COORD dest,
                         const CHAR_INFO *fill) {
    SMALL_RECT reg;
    int dx, dy, rw, rh;
    int i, j;
    cell_t *tmp;
    WCHAR fch;
    WORD fa;
    if (g.cur == NULL || scroll == NULL) {
        return;
    }
    reg = *scroll;
    if (clip != NULL) {
        if (clip->Left > reg.Left) {
            reg.Left = clip->Left;
        }
        if (clip->Top > reg.Top) {
            reg.Top = clip->Top;
        }
        if (clip->Right < reg.Right) {
            reg.Right = clip->Right;
        }
        if (clip->Bottom < reg.Bottom) {
            reg.Bottom = clip->Bottom;
        }
    }
    if (reg.Left < 0) {
        reg.Left = 0;
    }
    if (reg.Top < 0) {
        reg.Top = 0;
    }
    if (reg.Right > (SHORT)(g.gw - 1)) {
        reg.Right = (SHORT)(g.gw - 1);
    }
    if (reg.Bottom > (SHORT)(g.gh - 1)) {
        reg.Bottom = (SHORT)(g.gh - 1);
    }
    if (reg.Left > reg.Right || reg.Top > reg.Bottom) {
        return;
    }
    rw = reg.Right - reg.Left + 1;
    rh = reg.Bottom - reg.Top + 1;
    dx = dest.X - reg.Left;
    dy = dest.Y - reg.Top;
    tmp = (cell_t *)malloc(sizeof(cell_t) * (size_t)rw * (size_t)rh);
    if (tmp == NULL) {
        return;
    }
    for (j = 0; j < rh; j++) {
        const cell_t *src = g.cur + (size_t)(reg.Top + j) * g.gw + reg.Left;
        memcpy(tmp + (size_t)j * rw, src, sizeof(cell_t) * (size_t)rw);
    }
    fch = (fill != NULL) ? fill->Char.UnicodeChar : L' ';
    fa = (fill != NULL) ? fill->Attributes : g.attr;
    for (j = 0; j < rh; j++) {
        cell_t *dst = g.cur + (size_t)(reg.Top + j) * g.gw + reg.Left;
        for (i = 0; i < rw; i++) {
            dst[i].ch = fch;
            dst[i].attr = fa;
        }
    }
    for (j = 0; j < rh; j++) {
        int sy = reg.Top + dy + j;
        if (sy < reg.Top || sy > reg.Bottom) {
            continue;
        }
        for (i = 0; i < rw; i++) {
            int sx = reg.Left + dx + i;
            if (sx < reg.Left || sx > reg.Right) {
                continue;
            }
            g.cur[(size_t)sy * g.gw + sx] = tmp[(size_t)j * rw + i];
        }
    }
    free(tmp);
}

/* 从真实缓冲全量重建网格（init / RESIZE / ReadConsoleW 回合后校准）。 */
static void resync_full(void) {
    CONSOLE_SCREEN_BUFFER_INFO info;
    int w, h, i;
    cell_t *nc;
    CHAR_INFO *tmp;
    SMALL_RECT r;
    COORD sz;
    COORD org;
    if (g.p_get_csbi == NULL || g.p_read_console_out_w == NULL || g.hout == NULL) {
        return;
    }
    if (!g.p_get_csbi(g.hout, &info)) {
        return;
    }
    w = info.dwSize.X;
    h = info.dwSize.Y;
    if (w <= 0 || h <= 0 || (size_t)w * (size_t)h > GRID_MAX_CELLS) {
        return;
    }
    nc = (cell_t *)calloc((size_t)w * (size_t)h, sizeof(cell_t));
    if (nc == NULL) {
        return;
    }
    tmp = (CHAR_INFO *)malloc(sizeof(CHAR_INFO) * (size_t)w * (size_t)h);
    if (tmp == NULL) {
        free(nc);
        return;
    }
    r.Left = 0;
    r.Top = 0;
    r.Right = (SHORT)(w - 1);
    r.Bottom = (SHORT)(h - 1);
    sz.X = (SHORT)w;
    sz.Y = (SHORT)h;
    org.X = 0;
    org.Y = 0;
    if (g.p_read_console_out_w(g.hout, tmp, sz, org, &r)) {
        for (i = 0; i < w * h; i++) {
            nc[i].ch = tmp[i].Char.UnicodeChar;
            nc[i].attr = tmp[i].Attributes;
        }
    }
    free(tmp);
    free(g.cur);
    g.cur = nc;
    g.gw = w;
    g.gh = h;
    g.cx = info.dwCursorPosition.X;
    g.cy = info.dwCursorPosition.Y;
    g.attr = info.wAttributes;
    if (g.cx < 0) {
        g.cx = 0;
    }
    if (g.cy < 0) {
        g.cy = 0;
    }
    if (g.cx >= g.gw) {
        g.cx = g.gw - 1;
    }
    if (g.cy >= g.gh) {
        g.cy = g.gh - 1;
    }
    g.has_prev = 0; /* 尺寸/来源刷新 → 下一帧全量重绘 */
}

/* diff → VT_DATA（整行重绘 + 帧内 SGR 跟踪 + 帧尾 CUP；cs 持有态） */
static void emit_diff(void) {
    sbuf_t b;
    int full;
    int cur_sgr = -1;
    int y;
    if (g.pipe_dead || g.pipe == NULL || g.cur == NULL || g.gw <= 0 || g.gh <= 0) {
        return;
    }
    full = (!g.has_prev || g.pw != g.gw || g.ph != g.gh);
    sb_init(&b, (size_t)g.gw * (size_t)g.gh * 32u + (size_t)g.gh * 32u + 64u);
    if (b.fail) {
        return;
    }
    for (y = 0; y < g.gh; y++) {
        const cell_t *row = g.cur + (size_t)y * g.gw;
        int changed = full;
        if (!changed && g.has_prev) {
            const cell_t *prow = g.prev + (size_t)y * g.pw;
            changed = (memcmp(row, prow, sizeof(cell_t) * (size_t)g.gw) != 0);
        }
        if (!changed) {
            continue;
        }
        sb_fmt(&b, "\x1b[%d;1H", y + 1);
        {
            int x = 0;
            while (x < g.gw) {
                WORD a = row[x].attr;
                if (cur_sgr != (int)a) {
                    char s[48];
                    sgr_to_str(a, s, sizeof(s));
                    sb_fmt(&b, "\x1b[%sm", s);
                    cur_sgr = (int)a;
                }
                {
                    int consumed = 1;
                    unsigned cp = next_cp(row, x, g.gw, &consumed);
                    put_utf8(&b, cp);
                    x += consumed;
                }
            }
        }
    }
    sb_fmt(&b, "\x1b[%d;%dH", g.cy + 1, g.cx + 1);
    if (!b.fail && b.len > 0) {
        pipe_send_locked(WBWTTY_EARLY_T_VT_DATA, b.p, (DWORD)b.len);
    }
    free(b.p);
    /* 快照推进（即便本帧没发出去：管道已死，worker 即将卸钩） */
    if (g.prev == NULL || g.pw != g.gw || g.ph != g.gh) {
        cell_t *np = (cell_t *)malloc(sizeof(cell_t) * (size_t)g.gw * (size_t)g.gh);
        if (np == NULL) {
            g.has_prev = 0;
            return;
        }
        free(g.prev);
        g.prev = np;
        g.pw = g.gw;
        g.ph = g.gh;
    }
    memcpy(g.prev, g.cur, sizeof(cell_t) * (size_t)g.gw * (size_t)g.gh);
    g.has_prev = 1;
}

/* ---- 钩子实现（真实 API 先行 → 镜像 → 发帧） --------------------------- */

static BOOL WINAPI hook_write_console_w(HANDLE h, CONST VOID *buf, DWORD n, LPDWORD written,
                                        LPVOID reserved) {
    BOOL ok = g.p_write(h, buf, n, written, reserved);
    if (ok) {
        DWORD cnt = (written != NULL) ? *written : n;
        EnterCriticalSection(&g.cs);
        if (g.cur != NULL) {
            apply_write((CONST WCHAR *)buf, cnt);
            emit_diff();
        }
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static BOOL WINAPI hook_fill_char_w(HANDLE h, WCHAR ch, DWORD n, COORD start,
                                    LPDWORD written) {
    BOOL ok = g.p_fill_char_w(h, ch, n, start, written);
    if (ok && g.cur != NULL && start.Y >= 0 && start.Y < g.gh) {
        DWORD cnt = (written != NULL) ? *written : n;
        size_t idx = (size_t)start.Y * g.gw + (size_t)start.X;
        EnterCriticalSection(&g.cs);
        apply_fill(idx, cnt, 1, ch, 0, 0);
        emit_diff();
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static BOOL WINAPI hook_fill_attr_w(HANDLE h, WORD attr, DWORD n, COORD start,
                                    LPDWORD written) {
    BOOL ok = g.p_fill_attr_w(h, attr, n, start, written);
    if (ok && g.cur != NULL && start.Y >= 0 && start.Y < g.gh) {
        DWORD cnt = (written != NULL) ? *written : n;
        size_t idx = (size_t)start.Y * g.gw + (size_t)start.X;
        EnterCriticalSection(&g.cs);
        apply_fill(idx, cnt, 0, 0, 1, attr);
        emit_diff();
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static BOOL WINAPI hook_set_cursor(HANDLE h, COORD pos) {
    BOOL ok = g.p_set_cursor(h, pos);
    if (ok && g.cur != NULL) {
        EnterCriticalSection(&g.cs);
        g.cx = pos.X;
        g.cy = pos.Y;
        if (g.cx < 0) {
            g.cx = 0;
        }
        if (g.cy < 0) {
            g.cy = 0;
        }
        if (g.cx >= g.gw) {
            g.cx = g.gw - 1;
        }
        if (g.cy >= g.gh) {
            g.cy = g.gh - 1;
        }
        emit_diff();
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static BOOL WINAPI hook_set_attr(HANDLE h, WORD attr) {
    BOOL ok = g.p_set_attr(h, attr);
    if (ok) {
        EnterCriticalSection(&g.cs);
        g.attr = attr; /* 只影响后续写入，无可见变更 → 不发帧 */
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static BOOL WINAPI hook_scroll_w(HANDLE h, CONST SMALL_RECT *scroll, CONST SMALL_RECT *clip,
                                 COORD dest, CONST CHAR_INFO *fill) {
    BOOL ok = g.p_scroll_w(h, scroll, clip, dest, fill);
    if (ok && g.cur != NULL) {
        EnterCriticalSection(&g.cs);
        apply_scroll(scroll, clip, dest, fill);
        emit_diff();
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static BOOL WINAPI hook_read_console_w(HANDLE h, LPVOID buf, DWORD n, LPDWORD read,
                                       LPDWORD nread) {
    BOOL ok = g.p_read_console_w(h, buf, n, read, nread);
    if (ok) {
        /* 回显由 conhost 写入缓冲（旁路钩子）→ 回合后全量校准 */
        EnterCriticalSection(&g.cs);
        resync_full();
        emit_diff();
        LeaveCriticalSection(&g.cs);
    }
    return ok;
}

static FARPROC WINAPI hook_get_proc_address(HMODULE mod, LPCSTR name) {
    FARPROC real = g.p_get_proc_address(mod, name);
    if (real != NULL && name != NULL) {
        int k;
        for (k = 0; k_hooks[k].name != NULL; k++) {
            if (str_ieq(name, k_hooks[k].name)) {
                return k_hooks[k].hook;
            }
        }
    }
    return real;
}

/* ---- IAT 修补 / 卸载 ---------------------------------------------------- */

static void patch_iat(void) {
    HMODULE main_mod = GetModuleHandleW(NULL);
    IMAGE_DOS_HEADER *dos;
    IMAGE_NT_HEADERS *nt;
    IMAGE_DATA_DIRECTORY *dir;
    IMAGE_IMPORT_DESCRIPTOR *imp;
    if (main_mod == NULL) {
        return;
    }
    dos = (IMAGE_DOS_HEADER *)main_mod;
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) {
        return;
    }
    nt = (IMAGE_NT_HEADERS *)((uint8_t *)main_mod + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) {
        return;
    }
    dir = &nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    if (dir->VirtualAddress == 0) {
        return;
    }
    imp = (IMAGE_IMPORT_DESCRIPTOR *)((uint8_t *)main_mod + dir->VirtualAddress);
    for (; imp->Name != 0; imp++) {
        const char *dllname = (const char *)((uint8_t *)main_mod + imp->Name);
        IMAGE_THUNK_DATA *names;
        IMAGE_THUNK_DATA *ft;
        int i;
        if (!str_ieq(dllname, "KERNEL32.dll")) {
            continue;
        }
        names = (imp->OriginalFirstThunk != 0)
                    ? (IMAGE_THUNK_DATA *)((uint8_t *)main_mod + imp->OriginalFirstThunk)
                    : (IMAGE_THUNK_DATA *)((uint8_t *)main_mod + imp->FirstThunk);
        ft = (IMAGE_THUNK_DATA *)((uint8_t *)main_mod + imp->FirstThunk);
        for (i = 0; names[i].u1.AddressOfData != 0; i++) {
            IMAGE_IMPORT_BY_NAME *ibn;
            const char *fname;
            int k;
            if (names[i].u1.Ordinal & 0x80000000u) { /* IMAGE_ORDINAL_FLAG32：旧 SDK/MinGW 无此宏，取裸值 */
                continue;
            }
            ibn = (IMAGE_IMPORT_BY_NAME *)((uint8_t *)main_mod + names[i].u1.AddressOfData);
            fname = (const char *)ibn->Name;
            for (k = 0; k_hooks[k].name != NULL; k++) {
                void **slot;
                DWORD prot = 0;
                if (!str_ieq(fname, k_hooks[k].name)) {
                    continue;
                }
                if (g.niats >= IAT_MAX_SLOTS) {
                    break;
                }
                slot = (void **)&ft[i].u1.Function;
                if (!VirtualProtect(slot, sizeof(void *), PAGE_READWRITE, &prot)) {
                    break;
                }
                g.iats[g.niats].slot = slot;
                g.iats[g.niats].orig = *slot;
                *(FARPROC *)slot = k_hooks[k].hook;
                VirtualProtect(slot, sizeof(void *), prot, &prot);
                g.niats++;
                break;
            }
        }
    }
}

static void unpatch_iat(void) {
    int i;
    for (i = g.niats - 1; i >= 0; i--) {
        void **slot = g.iats[i].slot;
        DWORD prot = 0;
        if (VirtualProtect(slot, sizeof(void *), PAGE_READWRITE, &prot)) {
            *slot = g.iats[i].orig;
            VirtualProtect(slot, sizeof(void *), prot, &prot);
        }
    }
    g.niats = 0;
}

static void resolve_originals(void) {
    HMODULE k32 = GetModuleHandleW(L"kernel32.dll");
    if (k32 == NULL) {
        return;
    }
    g.p_write = (t_write_console_w)GetProcAddress(k32, "WriteConsoleW");
    g.p_fill_char_w = (t_fill_char_w)GetProcAddress(k32, "FillConsoleOutputCharacterW");
    g.p_fill_attr_w = (t_fill_attr_w)GetProcAddress(k32, "FillConsoleOutputAttribute");
    g.p_set_cursor = (t_set_cursor)GetProcAddress(k32, "SetConsoleCursorPosition");
    g.p_set_attr = (t_set_attr)GetProcAddress(k32, "SetConsoleTextAttribute");
    g.p_scroll_w = (t_scroll_w)GetProcAddress(k32, "ScrollConsoleScreenBufferW");
    g.p_read_console_w = (t_read_console_w)GetProcAddress(k32, "ReadConsoleW");
    g.p_get_csbi = (t_get_csbi)GetProcAddress(k32, "GetConsoleScreenBufferInfo");
    g.p_read_console_out_w =
        (t_read_console_out_w)GetProcAddress(k32, "ReadConsoleOutputW");
    g.p_get_proc_address = (t_get_proc_address)GetProcAddress(k32, "GetProcAddress");
}

static void cleanup_hooks(void) {
    unpatch_iat();
    EnterCriticalSection(&g.cs);
    g.pipe_dead = 1;
    if (g.pipe != NULL) {
        CloseHandle(g.pipe);
        g.pipe = NULL;
    }
    LeaveCriticalSection(&g.cs);
}

/* ---- worker：连管道 → 收宿主帧 → 断则卸钩常驻退出 ---------------------- */

static DWORD WINAPI worker(LPVOID unused) {
    WCHAR name[320];
    HANDLE p = INVALID_HANDLE_VALUE;
    int attempt;
    DWORD mode;
    uint8_t hello[WBWTTY_EARLY_HDR_LEN + 4];
    uint16_t ver16 = (uint16_t)WBWTTY_EARLY_PROTO_VER;
    uint16_t flags16 = 0;
    (void)unused;
    if (GetEnvironmentVariableW(L"WBWTTY_EARLY_PIPE", name, 320) == 0) {
        cleanup_hooks();
        return 0;
    }
    for (attempt = 0; attempt < 50; attempt++) {
        p = CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING, 0, NULL);
        if (p != INVALID_HANDLE_VALUE) {
            break;
        }
        Sleep(100);
    }
    if (p == INVALID_HANDLE_VALUE) {
        cleanup_hooks();
        return 0;
    }
    mode = PIPE_READMODE_MESSAGE;
    SetNamedPipeHandleState(p, &mode, NULL, NULL);
    EnterCriticalSection(&g.cs);
    g.pipe = p;
    g.pipe_dead = 0;
    LeaveCriticalSection(&g.cs);

    /* 发 HELLO；pipe_send_locked 内部持 cs，不可再外层套 cs（否则死锁） */
    {
        uint8_t hello_payload[4];
        uint16_t ver16 = (uint16_t)WBWTTY_EARLY_PROTO_VER;
        uint16_t flags16 = 0;
        hello_payload[0] = (uint8_t)(ver16 & 0xFFu);
        hello_payload[1] = (uint8_t)(ver16 >> 8);
        hello_payload[2] = (uint8_t)(flags16 & 0xFFu);
        hello_payload[3] = (uint8_t)(flags16 >> 8);
        pipe_send_locked(WBWTTY_EARLY_T_HELLO, hello_payload, sizeof(hello_payload));
    }
    /* 连接即补一帧全量（钩子窗口期内的写入在这里送达前端） */
    emit_diff();

    for (;;) {
        DWORD avail = 0;
        uint8_t buf[4096];
        DWORD n = 0;
        BOOL ok;
        uint32_t flen = 0;
        BYTE ftype;
        if (!PeekNamedPipe(p, NULL, 0, NULL, &avail, NULL)) {
            break;
        }
        if (avail == 0) {
            Sleep(10);
            continue;
        }
        ok = ReadFile(p, buf, sizeof(buf), &n, NULL);
        if (!ok && GetLastError() != ERROR_MORE_DATA) {
            break;
        }
        while (!ok && GetLastError() == ERROR_MORE_DATA) {
            /* 超大帧（宿主不发）→ 吞掉剩余字节保持消息边界 */
            DWORD junk_n = 0;
            DWORD junk_avail = 0;
            uint8_t junk[256];
            if (!ReadFile(p, junk, sizeof(junk), &junk_n, NULL)) {
                break;
            }
            ok = 1;
            (void)junk_avail;
        }
        if (n < WBWTTY_EARLY_HDR_LEN) {
            continue;
        }
        if (!wbw_early_parse_len(buf, &flen)) {
            break; /* 协议错误 = 断管语义 */
        }
        if ((size_t)flen + 4u > n) {
            continue; /* 半帧（消息模式不应出现）→ 丢弃 */
        }
        ftype = buf[4];
        if (ftype == WBWTTY_EARLY_T_RESIZE && flen >= 5) {
            /* 宿主已 resize 真实缓冲（frame 先于/紧随 resize_console 到达）→ 校准 */
            EnterCriticalSection(&g.cs);
            resync_full();
            emit_diff();
            LeaveCriticalSection(&g.cs);
        } else if (ftype == WBWTTY_EARLY_T_PING) {
            EnterCriticalSection(&g.cs);
            pipe_send_locked(WBWTTY_EARLY_T_PONG, NULL, 0);
            LeaveCriticalSection(&g.cs);
        } else if (ftype == WBWTTY_EARLY_T_EXIT || ftype == WBWTTY_EARLY_T_ERR) {
            break;
        }
        if (g.pipe_dead) {
            break;
        }
    }
    cleanup_hooks();
    return 0;
}

/* ---- 入口 --------------------------------------------------------------- */

BOOL WINAPI DllMain(HINSTANCE inst, DWORD reason, LPVOID reserved) {
    (void)reserved;
    if (reason == DLL_PROCESS_ATTACH) {
        g.self = (HMODULE)inst;
        g.attr = 0x07;
        InitializeCriticalSection(&g.cs);
        DisableThreadLibraryCalls(inst);
        resolve_originals();
        g.hout = CreateFileW(L"CONOUT$", GENERIC_READ | GENERIC_WRITE,
                             FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL);
        if (g.hout == INVALID_HANDLE_VALUE) {
            g.hout = NULL;
        }
        /* 子进程主模块仍挂起 → 现在修补零竞态；worker 起后再连管道 */
        EnterCriticalSection(&g.cs);
        resync_full();
        LeaveCriticalSection(&g.cs);
        patch_iat();
        {
            HANDLE th = CreateThread(NULL, 0, worker, NULL, 0, NULL);
            if (th != NULL) {
                CloseHandle(th);
            } else {
                cleanup_hooks(); /* 起不了 worker → 立即卸钩回退轮询 */
            }
        }
        return TRUE;
    }
    if (reason == DLL_PROCESS_DETACH) {
        /* 进程退出路径（我们从不 FreeLibrary；worker 卸钩后 DLL 常驻） */
        if (g.hout != NULL) {
            CloseHandle(g.hout);
            g.hout = NULL;
        }
        DeleteCriticalSection(&g.cs);
        return TRUE;
    }
    return TRUE;
}
