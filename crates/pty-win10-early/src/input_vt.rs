//! 前端 VT 字节 → 控制台 `INPUT_RECORD` 解码（纯逻辑，无 Win32 调用）。
//!
//! 覆盖面见 `docs/win10-early-bridge.md` §3.3：可打印字符（UTF-8→UTF-16）、
//! CR/LF/BS/TAB、ETX(Ctrl+C)、CSI 方向/功能键、SS3 F1–F4、常见 `~` 键、
//! SGR 鼠标（1006）、bracketed paste 标记透传。未知序列静默丢弃（debug 日志级别）。

use crate::sys::{
    Coord, InputEvent, InputRecord, KeyEventRecord, MouseEventRecord, MOUSE_EVENT, MOUSE_MOVED,
    MOUSE_WHEEL, KEY_EVENT, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_F1, VK_HOME,
    VK_INSERT, VK_LEFT, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP, ENHANCED_KEY, FROM_LEFT_1ST_BUTTON_PRESSED,
    FROM_LEFT_2ND_BUTTON_PRESSED, RIGHTMOST_BUTTON_PRESSED, LEFT_CTRL_PRESSED, LEFT_ALT_PRESSED,
    SHIFT_PRESSED, DWORD,
};

/// 增量解码器：`feed` 返回完整键/鼠标事件；跨调用保留不完整尾部。
#[derive(Default)]
pub struct VtDecoder {
    pending: Vec<u8>,
}

impl VtDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加字节并产出可注入的输入事件（可能为空——序列不完整或被丢弃）。
    pub fn feed(&mut self, input: &[u8]) -> Vec<InputRecord> {
        self.pending.extend_from_slice(input);
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            match parse_one(&self.pending[i..]) {
                Parse::Incomplete => break,
                Parse::Skip(n) => i += n,
                Parse::Items(recs, n) => {
                    out.extend(recs);
                    i += n;
                }
            }
        }
        self.pending.drain(..i);
        out
    }
}

enum Parse {
    Incomplete,
    Skip(usize),
    Items(Vec<InputRecord>, usize),
}

fn key_record(vk: WORD_, ch: u16, ctrl: DWORD, down: bool) -> InputRecord {
    InputRecord {
        EventType: KEY_EVENT,
        Event: InputEvent {
            KeyEvent: KeyEventRecord {
                bKeyDown: if down { 1 } else { 0 },
                wRepeatCount: 1,
                wVirtualKey: vk,
                wVirtualScanCode: 0,
                UnicodeChar: ch,
                dwControlKeyState: ctrl,
            },
        },
    }
}

type WORD_ = u16;

/// 一次按键 = down + up 两条记录。
fn key_pair(vk: WORD_, ch: u16, ctrl: DWORD) -> Vec<InputRecord> {
    vec![key_record(vk, ch, ctrl, true), key_record(vk, ch, ctrl, false)]
}

/// 普通字符的 VK：A-Z/0-9 直取大写，其余 0；大写顺带 SHIFT。
fn char_keys(c: char) -> (WORD_, u16, DWORD) {
    let mut ctrl: DWORD = 0;
    let vk = if c.is_ascii_alphanumeric() {
        let up = c.to_ascii_uppercase();
        if up != c {
            ctrl |= SHIFT_PRESSED;
        }
        up as WORD_
    } else {
        0
    };
    let mut buf = [0u16; 2];
    let n = c.encode_utf16(&mut buf).len();
    let ch = if n == 1 { buf[0] } else { 0 }; // 基本多文种平面外走代理对（下方按对处理）
    (vk, ch, ctrl)
}

fn csi_final(bytes: &[u8]) -> Option<(usize, usize)> {
    // 返回 (params_end, final_idx)；final ∈ 0x40..=0x7E
    for (i, &b) in bytes.iter().enumerate().skip(1) {
        if (0x40..=0x7E).contains(&b) {
            return Some((i, i));
        }
        if !(b'0'..=b'?').contains(&b) && !(0x20..=0x2F).contains(&b) {
            return None; // 非法中间字节 → 丢弃
        }
    }
    None
}

fn parse_mouse_sgr(params: &[u8], final_byte: u8, consumed: usize) -> Option<(InputRecord, usize)> {
    // CSI < b ; x ; y M/m
    let s = std::str::from_utf8(params).ok()?;
    let s = s.strip_prefix('<')?;
    let mut it = s.split(';');
    let b: u32 = it.next()?.parse().ok()?;
    let x: i32 = it.next()?.parse().ok()?;
    let y: i32 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }

    let motion = b & 32 != 0;
    let wheel = b & 64 != 0;
    let press = final_byte == b'M';
    let mut button: DWORD = 0;
    let mut flags: DWORD = 0;
    if wheel {
        flags = MOUSE_WHEEL;
        let up = b & 1 == 0;
        let delta: u32 = if up { 120 } else { 0u32.wrapping_sub(120) };
        button = delta << 16;
    } else if motion {
        flags = MOUSE_MOVED;
        button = match b & 3 {
            0 => FROM_LEFT_1ST_BUTTON_PRESSED,
            1 => FROM_LEFT_2ND_BUTTON_PRESSED,
            2 => RIGHTMOST_BUTTON_PRESSED,
            _ => 0,
        };
    } else if press {
        button = match b & 3 {
            0 => FROM_LEFT_1ST_BUTTON_PRESSED,
            1 => FROM_LEFT_2ND_BUTTON_PRESSED,
            2 => RIGHTMOST_BUTTON_PRESSED,
            _ => 0,
        };
    } // 释放（m 且非 motion/wheel）→ button 0

    let mut ctrl: DWORD = 0;
    if b & 4 != 0 {
        ctrl |= SHIFT_PRESSED;
    }
    if b & 8 != 0 {
        ctrl |= LEFT_ALT_PRESSED;
    }
    if b & 16 != 0 {
        ctrl |= LEFT_CTRL_PRESSED;
    }

    let rec = InputRecord {
        EventType: MOUSE_EVENT,
        Event: InputEvent {
            MouseEvent: MouseEventRecord {
                dwMousePosition: Coord { X: (x - 1) as i16, Y: (y - 1) as i16 },
                dwButtonState: button,
                dwControlKeyState: ctrl,
                dwEventFlags: flags,
            },
        },
    };
    Some((rec, consumed))
}

fn parse_one(b: &[u8]) -> Parse {
    let first = b[0];
    match first {
        // ---- 控制字符 --------------------------------------------------
        b'\r' | b'\n' => Parse::Items(key_pair(VK_RETURN, b'\r' as u16, 0), 1),
        b'\x08' => Parse::Items(key_pair(VK_BACK, 8, 0), 1),
        b'\t' => Parse::Items(key_pair(VK_TAB, 9, 0), 1),
        // ETX → Ctrl+C 键事件（'C' + CTRL，conhost 转发为 CTRL_C_EVENT）
        b'\x03' => Parse::Items(key_pair(0x43, 0x03, LEFT_CTRL_PRESSED), 1),
        b'\x1b' => parse_escape(b),
        // ---- UTF-8 ------------------------------------------------------
        0x00..=0x7F => {
            if first < 0x20 || first == 0x7F {
                return Parse::Skip(1); // 其余 C0 控制符 / DEL 丢弃
            }
            let (c, n) = match first {
                0x20..=0x7E => (first as char, 1),
                _ => return Parse::Skip(1),
            };
            let (vk, ch, ctrl) = char_keys(c);
            Parse::Items(key_pair(vk, ch, ctrl), n)
        }
        0xC2..=0xDF => utf8_seq(b, 2),
        0xE0..=0xEF => utf8_seq(b, 3),
        0xF0..=0xF4 => utf8_seq(b, 4),
        _ => Parse::Skip(1),
    }
}

/// 解析完整 UTF-8 多字节序列（含代理对 → 单条 KEY_EVENT unicodeChar 无法装下
/// 4 字节码点时拆两条 surrogate KEY_EVENT，conhost 按序重组）。
fn utf8_seq(b: &[u8], n: usize) -> Parse {
    if b.len() < n {
        return Parse::Incomplete;
    }
    match std::str::from_utf8(&b[..n]) {
        Ok(s) => {
            let mut items = Vec::new();
            for c in s.chars() {
                let mut buf = [0u16; 2];
                let units = c.encode_utf16(&mut buf);
                if units.len() == 1 {
                    let (vk, ch, ctrl) = char_keys(c);
                    items.extend(key_pair(vk, ch, ctrl));
                } else {
                    // 代理对：两条 KEY_EVENT（低 16 位 / 高 16 位各 down+up）
                    for &u in &buf[..2] {
                        items.extend(key_pair(0, u, 0));
                    }
                }
            }
            Parse::Items(items, n)
        }
        Err(_) => Parse::Skip(1),
    }
}

fn parse_escape(b: &[u8]) -> Parse {
    if b.len() == 1 {
        // 单独 ESC：可能是 Alt 修饰前缀，等下一个字节
        return Parse::Incomplete;
    }
    match b[1] {
        b'[' => parse_csi(b),
        b'O' => {
            if b.len() < 3 {
                return Parse::Incomplete;
            }
            let vk = match b[2] {
                b'P' => VK_F1,
                b'Q' => VK_F1 + 1,
                b'R' => VK_F1 + 2,
                b'S' => VK_F1 + 3,
                _ => return Parse::Skip(3),
            };
            Parse::Items(key_pair(vk, 0, ENHANCED_KEY), 3)
        }
        // ESC <printable> = Alt+key：首版丢 ESC 仅注入字符
        0x20..=0x7E => parse_one(&b[1..]),
        _ => Parse::Skip(2),
    }
}

fn parse_csi(b: &[u8]) -> Parse {
    let Some((params_end, final_idx)) = csi_final(&b[1..]) else {
        // b[1..] 可能不完整；区分"不完整"与"非法"
        if b.len() > 64 {
            return Parse::Skip(1); // 过长保护：丢 ESC 从头重解析
        }
        for &x in &b[1..] {
            if !(0x30..=0x3F).contains(&x) && !(0x20..=0x2F).contains(&x) && !(0x40..=0x7E).contains(&x) {
                return Parse::Skip(1);
            }
        }
        return Parse::Incomplete;
    };
    let params = &b[2..1 + params_end]; // b = ESC [ params final
    let final_byte = b[1 + final_idx];
    let consumed = 1 + final_idx + 1;

    if params.first() == Some(&b'<') {
        return match parse_mouse_sgr(params, final_byte, consumed) {
            Some((rec, n)) => Parse::Items(vec![rec], n),
            None => Parse::Skip(consumed),
        };
    }

    let ctrl: DWORD = ENHANCED_KEY;
    match final_byte {
        b'A' => Parse::Items(key_pair(VK_UP, 0, ctrl), consumed),
        b'B' => Parse::Items(key_pair(VK_DOWN, 0, ctrl), consumed),
        b'C' => Parse::Items(key_pair(VK_RIGHT, 0, ctrl), consumed),
        b'D' => Parse::Items(key_pair(VK_LEFT, 0, ctrl), consumed),
        b'H' => Parse::Items(key_pair(VK_HOME, 0, ctrl), consumed),
        b'F' => Parse::Items(key_pair(VK_END, 0, ctrl), consumed),
        b'Z' => Parse::Items(key_pair(VK_TAB, 0, SHIFT_PRESSED | ENHANCED_KEY), consumed),
        b'~' => {
            let nums = String::from_utf8_lossy(params);
            let n: u32 = nums.split(';').next().unwrap_or("").parse().unwrap_or(0);
            let vk = match n {
                1 | 7 => Some(VK_HOME),
                2 => Some(VK_INSERT),
                3 => Some(VK_DELETE),
                4 | 8 => Some(VK_END),
                11..=15 => Some(VK_F1 + (n - 11) as u16),
                17..=21 => Some(VK_F1 + (n - 11) as u16),
                23 | 24 => Some(VK_F1 + (n - 11) as u16),
                // bracketed paste 标记（200~ / 201~）与未知参数：透传/丢弃
                200 | 201 => None,
                _ => None,
            };
            match vk {
                Some(vk) => Parse::Items(key_pair(vk, 0, ctrl), consumed),
                None => Parse::Skip(consumed),
            }
        }
        // SGR 颜色等控制序列（应用发给终端的）：丢弃
        b'm' | b'h' | b'l' | b'r' | b't' | b'J' | b'K' => Parse::Skip(consumed),
        _ => Parse::Skip(consumed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(parts: &[&[u8]]) -> Vec<InputRecord> {
        let mut d = VtDecoder::new();
        let mut out = Vec::new();
        for p in parts {
            out.extend(d.feed(p));
        }
        out
    }

    fn key_at(rec: &InputRecord) -> (bool, u16, u16, DWORD) {
        assert_eq!(rec.EventType, KEY_EVENT);
        let k = unsafe { rec.Event.KeyEvent };
        (k.bKeyDown != 0, k.wVirtualKey, k.UnicodeChar, k.dwControlKeyState)
    }

    #[test]
    fn printable_ascii_makes_down_up_pair() {
        let recs = feed_all(&[b"a"]);
        assert_eq!(recs.len(), 2);
        let (down, vk, ch, ctrl) = key_at(&recs[0]);
        assert!(down && !key_at(&recs[1]).0);
        assert_eq!((vk, ch), (0x41, 'a' as u16));
        assert_eq!(ctrl, SHIFT_PRESSED);
    }

    #[test]
    fn enter_backspace_tab_and_etx() {
        for (bytes, vk, ch, ctrl) in [
            (&b"\r"[..], VK_RETURN, b'\r' as u16, 0),
            (&b"\n"[..], VK_RETURN, b'\r' as u16, 0),
            (&b"\x08"[..], VK_BACK, 8, 0),
            (&b"\t"[..], VK_TAB, 9, 0),
            (&b"\x03"[..], 0x43, 3, LEFT_CTRL_PRESSED),
        ] {
            let recs = feed_all(&[bytes]);
            assert_eq!(recs.len(), 2, "{bytes:?}");
            let (_, v, c, ct) = key_at(&recs[0]);
            assert_eq!((v, c, ct), (vk, ch, ctrl), "{bytes:?}");
        }
    }

    #[test]
    fn arrow_split_across_feeds() {
        let recs = feed_all(&[b"\x1b", b"[", b"A"]);
        assert_eq!(recs.len(), 2);
        let (down, vk, _, ctrl) = key_at(&recs[0]);
        assert!(down);
        assert_eq!(vk, VK_UP);
        assert_eq!(ctrl, ENHANCED_KEY);
    }

    #[test]
    fn ss3_function_keys() {
        for (b, vk) in [(b'P', VK_F1), (b'Q', VK_F1 + 1), (b'R', VK_F1 + 2), (b'S', VK_F1 + 3)] {
            let seq = [b'\x1b', b'O', b];
            let recs = feed_all(&[&seq]);
            assert_eq!(key_at(&recs[0]).1, vk);
        }
    }

    #[test]
    fn tilde_sequences() {
        for (params, vk) in [("2", VK_INSERT), ("3", VK_DELETE), ("15", VK_F1 + 4), ("21", VK_F1 + 10)] {
            let seq = format!("\x1b[{params}~");
            let recs = feed_all(&[seq.as_bytes()]);
            assert_eq!(recs.len(), 2, "{params}");
            assert_eq!(key_at(&recs[0]).1, vk, "{params}");
        }
        // bracketed paste 标记被丢弃
        let recs = feed_all(&[b"\x1b[200~x\x1b[201~"]);
        assert_eq!(recs.len(), 2); // 只剩 'x'
        assert_eq!(key_at(&recs[0]).2, 'x' as u16);
    }

    #[test]
    fn utf8_multibyte_char() {
        let recs = feed_all(&["中".as_bytes()]);
        assert_eq!(recs.len(), 2);
        let (_, _, ch, _) = key_at(&recs[0]);
        assert_eq!(ch, 0x4E2D);
    }

    #[test]
    fn utf8_split_across_feeds() {
        let bytes = "中".as_bytes();
        let recs = feed_all(&[&bytes[..1], &bytes[1..]]);
        assert_eq!(recs.len(), 2);
        assert_eq!(key_at(&recs[0]).2, 0x4E2D);
    }

    #[test]
    fn sgr_mouse_click_and_motion() {
        let press = feed_all(&[b"\x1b[<0;5;3M"]);
        assert_eq!(press.len(), 1);
        assert_eq!(press[0].EventType, MOUSE_EVENT);
        let m = unsafe { press[0].Event.MouseEvent };
        assert_eq!((m.dwMousePosition.X, m.dwMousePosition.Y), (4, 2));
        assert_eq!(m.dwButtonState, FROM_LEFT_1ST_BUTTON_PRESSED);
        assert_eq!(m.dwEventFlags, 0);

        let release = feed_all(&[b"\x1b[<0;5;3m"]);
        let m = unsafe { release[0].Event.MouseEvent };
        assert_eq!(m.dwButtonState, 0);

        let motion = feed_all(&[b"\x1b[<32;6;4M"]);
        let m = unsafe { motion[0].Event.MouseEvent };
        assert_eq!(m.dwEventFlags, MOUSE_MOVED);
        assert_eq!(m.dwMousePosition.X, 5);

        let wheel = feed_all(&[b"\x1b[<64;1;1M"]);
        let m = unsafe { wheel[0].Event.MouseEvent };
        assert_eq!(m.dwEventFlags, MOUSE_WHEEL);
        assert_eq!(m.dwButtonState, 120 << 16);
    }

    #[test]
    fn unknown_sequences_are_dropped() {
        // SGR 颜色序列（发给终端的）不产生输入事件
        let recs = feed_all(&[b"\x1b[0;31m"]);
        assert!(recs.is_empty());
        // 孤立 ESC 后跟非转义字节：按字符处理
        let recs = feed_all(&[b"\x1bx"]);
        assert!(!recs.is_empty());
    }

    #[test]
    fn c0_controls_other_than_known_are_dropped() {
        let recs = feed_all(&[b"\x07\x00"]);
        assert!(recs.is_empty());
    }
}
