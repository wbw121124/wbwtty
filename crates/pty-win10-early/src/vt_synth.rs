//! 屏幕缓冲 → VT 合成（纯逻辑，无 Win32 调用，便于单测）。
//!
//! 规则见 `docs/win10-early-bridge.md` §3.2：
//! - 变更**行整行重绘**（`CUP(row,1)` + 全行内容）；尺寸变化视为全屏变更
//! - 每帧先用 `ESC[0;…m` 全量 SGR（状态确定，不依赖上一帧残留）
//! - 光标定位在帧尾 `CUP(cy,cx)`（1-based）
//! - 宽字符：UTF-16 代理对合成一个 char（双 cell 由行级重绘覆盖）；
//!   孤立代理项输出 U+FFFD 占位

use crate::sys::CharInfo;

/// 某一时刻的完整屏幕快照。
#[derive(Clone, PartialEq, Eq)]
pub struct Screen {
    pub w: usize,
    pub h: usize,
    /// 行优先，`len == w * h`
    pub cells: Vec<CharInfo>,
    /// 光标（缓冲坐标，0-based）
    pub cursor: (u16, u16),
}

impl Screen {
    /// 以空白填充构造（单测基线；生产路径由 read_screen 直接构造）。
    #[cfg(test)]
    pub fn new(w: usize, h: usize, cursor: (u16, u16)) -> Self {
        Self {
            w,
            h,
            cells: vec![CharInfo { UnicodeChar: b' ' as u16, Attributes: 0 }; w * h],
            cursor,
        }
    }
}

/// 单元属性 → SGR 序列（`ESC[0;…m`，含 fg/bg/下划线/反显）。
pub fn sgr_for(attrs: u16) -> String {
    let fg = (attrs & 0x000F) as u8;
    let bg = ((attrs >> 4) & 0x000F) as u8;
    let mut codes = String::from("0");
    codes.push(';');
    codes.push_str(&ansi_color(fg, false));
    codes.push(';');
    codes.push_str(&ansi_color(bg, true));
    if attrs & crate::sys::COMMON_LVB_UNDERSCORE != 0 {
        codes.push_str(";4");
    }
    if attrs & crate::sys::COMMON_LVB_REVERSE_VIDEO != 0 {
        codes.push_str(";7");
    }
    codes
}

/// console BGR+I 位 → ANSI 色号（30/40 基数；intensity → +60 → 90/100 基数）。
fn ansi_color(bits: u8, background: bool) -> String {
    // console: bit0=B, bit1=G, bit2=R, bit3=I → ANSI 顺序 R,G,B
    let idx = (if bits & 0x04 != 0 { 1 } else { 0 })
        + (if bits & 0x02 != 0 { 2 } else { 0 })
        + (if bits & 0x01 != 0 { 4 } else { 0 });
    let base = if bits & 0x08 != 0 {
        if background { 100 } else { 90 }
    } else if background {
        40
    } else {
        30
    };
    (base + idx).to_string()
}

/// 从 next 单元流中取出下一个显示字符（处理代理对），返回 (char, 消耗 cell 数)。
fn next_char(cells: &[CharInfo], x: usize) -> (char, usize) {
    let u = cells[x].UnicodeChar;
    if (0xD800..0xDC00).contains(&u) {
        if let Some(low) = cells.get(x + 1).map(|c| c.UnicodeChar) {
            if (0xDC00..0xE000).contains(&low) {
                let cp = 0x1_0000 + (((u as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                if let Some(ch) = char::from_u32(cp) {
                    return (ch, 2);
                }
            }
        }
        return ('\u{FFFD}', 1);
    }
    if (0xDC00..0xE000).contains(&u) {
        return ('\u{FFFD}', 1);
    }
    (char::from_u32(u as u32).unwrap_or('\u{FFFD}'), 1)
}

/// 将 `next` 相对 `prev` 的差异合成为 VT 字节；`prev` 尺寸不同或为 `None` 时全屏重绘。
pub fn synth(prev: Option<&Screen>, next: &Screen) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let full = match prev {
        Some(p) => p.w != next.w || p.h != next.h,
        None => true,
    };

    let mut cur_sgr: Option<u16> = None;
    for y in 0..next.h {
        let row = &next.cells[y * next.w..(y + 1) * next.w];
        let changed = full
            || prev.is_some_and(|p| p.cells[y * p.w..(y + 1) * p.w] != *row);
        if !changed {
            continue;
        }
        out.extend_from_slice(format!("\x1b[{};1H", y + 1).as_bytes());
        let mut x = 0;
        while x < next.w {
            let attrs = row[x].Attributes;
            if cur_sgr != Some(attrs) {
                out.extend_from_slice(format!("\x1b[{}m", sgr_for(attrs)).as_bytes());
                cur_sgr = Some(attrs);
            }
            let (ch, n) = next_char(row, x);
            let mut buf = [0u8; 4];
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            x += n;
        }
    }
    // 帧尾光标定位
    out.extend_from_slice(
        format!("\x1b[{};{}H", next.cursor.1 + 1, next.cursor.0 + 1).as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(ch: char, attrs: u16) -> CharInfo {
        let mut buf = [0u16; 2];
        let n = ch.encode_utf16(&mut buf).len();
        assert_eq!(n, 1, "test helper only handles BMP chars");
        CharInfo { UnicodeChar: buf[0], Attributes: attrs }
    }

    fn put(screen: &mut Screen, x: usize, y: usize, ch: char, attrs: u16) {
        screen.cells[y * screen.w + x] = cell(ch, attrs);
    }

    /// 抽掉 SGR 与 CUP 后剩余的可读文本（断言用）。
    fn visible(s: &str) -> String {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\u{1b}' {
                out.push(c);
                continue;
            }
            if it.next() == Some('[') {
                for c2 in it.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c2) {
                        break;
                    }
                }
            }
        }
        out
    }

    #[test]
    fn sgr_maps_console_attrs() {
        // 0x07 灰字黑底 → 37;40；绿字红底（fg=0x02, bg=0x40）→ 32;41
        assert_eq!(sgr_for(0x07), "0;37;40");
        assert_eq!(sgr_for(0x40 | 0x02), "0;32;41");
        // 加亮：0x0F fg=亮白→97，bg=0→40；0x80 bg=亮黑→100，fg=0→30；0xF0 bg=亮白→107
        assert_eq!(sgr_for(0x0F), "0;97;40");
        assert_eq!(sgr_for(0x80), "0;30;100");
        assert_eq!(sgr_for(0xF0), "0;30;107");
        // 下划线（0x2000）+ 反显（0x1000）——与颜色位不重叠
        let s = sgr_for(0x07 | crate::sys::COMMON_LVB_UNDERSCORE);
        assert!(s.ends_with(";4"), "{s}");
        let s = sgr_for(0x07 | crate::sys::COMMON_LVB_REVERSE_VIDEO);
        assert!(s.ends_with(";7"), "{s}");
    }

    #[test]
    fn first_frame_is_full_redraw_with_cursor() {
        let mut s = Screen::new(4, 2, (1, 0));
        put(&mut s, 0, 0, 'H', 0x07);
        put(&mut s, 1, 0, 'i', 0x07);
        let vt = String::from_utf8(synth(None, &s)).unwrap();
        assert!(vt.contains("Hi"), "{vt}");
        // 两行都被绘制，帧尾定位到 (1,0) → ESC[1;2H
        assert!(vt.contains("\x1b[1;1H"), "{vt}");
        assert!(vt.contains("\x1b[2;1H"), "{vt}");
        assert!(vt.ends_with("\x1b[1;2H"), "{vt}");
        assert!(vt.contains("\x1b[0;37;40m"), "{vt}");
    }

    #[test]
    fn unchanged_screen_emits_only_cursor() {
        let mut s = Screen::new(4, 2, (0, 0));
        put(&mut s, 0, 0, 'a', 0x07);
        let vt1 = synth(None, &s);
        let vt2 = synth(Some(&s), &s);
        let t = String::from_utf8(vt2).unwrap();
        assert!(t.ends_with("\x1b[1;1H"), "{t}");
        assert!(!t.contains('a'), "无变更不应重绘字符: {t}");
        assert!(vt1.len() > t.len());
    }

    #[test]
    fn only_changed_row_is_redrawn() {
        let mut a = Screen::new(4, 2, (0, 0));
        put(&mut a, 0, 0, 'x', 0x07);
        put(&mut a, 0, 1, 'y', 0x07);
        let mut b = a.clone();
        put(&mut b, 1, 1, 'z', 0x07); // 第 2 行变化
        b.cursor = (2, 1); // 避免帧尾 CUP 恰好等于 "\x1b[1;1H" 污染断言
        let t = String::from_utf8(synth(Some(&a), &b)).unwrap();
        assert!(t.contains("\x1b[2;1H"), "{t}");
        assert!(!t.contains("\x1b[1;1H"), "第 1 行不应重绘: {t}");
        assert!(t.contains("yz"), "{t}");
    }

    #[test]
    fn dim_change_triggers_full_redraw() {
        let a = Screen::new(4, 2, (0, 0));
        let mut b = Screen::new(6, 2, (0, 0));
        put(&mut b, 5, 0, 'W', 0x07);
        let t = String::from_utf8(synth(Some(&a), &b)).unwrap();
        assert!(t.contains("W"), "尺寸变化必须全屏重绘: {t}");
    }

    #[test]
    fn wide_char_pair_emits_single_codepoint() {
        let mut s = Screen::new(4, 1, (0, 0));
        // U+1F600 😀 的 UTF-16 代理对（中/文等是 BMP 单元，不触发此路径）
        let cp = 0x1F600u32 - 0x10000;
        let lead = (0xD800 + (cp >> 10)) as u16;
        let trail = (0xDC00 + (cp & 0x3FF)) as u16;
        s.cells[0] = CharInfo { UnicodeChar: lead, Attributes: 0x07 };
        s.cells[1] = CharInfo { UnicodeChar: trail, Attributes: 0x07 };
        put(&mut s, 2, 0, 'a', 0x07);
        let t = String::from_utf8(synth(None, &s)).unwrap();
        assert!(visible(&t).contains('\u{1F600}'), "{t}");
        assert!(visible(&t).contains('a'), "{t}");
    }

    #[test]
    fn attr_change_on_row_emits_sgr() {
        let mut a = Screen::new(4, 1, (0, 0));
        put(&mut a, 0, 0, 'x', 0x07);
        let mut b = a.clone();
        put(&mut b, 0, 0, 'x', 0x1F); // 亮白字 + 蓝底（fg=0x0F→97, bg=0x01→44）
        let t = String::from_utf8(synth(Some(&a), &b)).unwrap();
        assert!(t.contains("\x1b[0;97;44m"), "{t}");
    }
}
