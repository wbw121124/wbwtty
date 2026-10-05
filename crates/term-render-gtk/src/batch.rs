//! 单元格 → 样式分段（span）：文本绘制与字形缓存的分组依据（纯逻辑）。

use vt_parser::{Attrs, Color, ScreenView};

/// 同一样式（fg/bg/attrs）的连续单元格段。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub row: u16,
    /// 起始列（含）。
    pub col: u16,
    /// 显示文本（含组合符；宽字符为单个字符）。
    pub text: String,
    /// 占用列数（宽字符占 2）。
    pub width: u16,
    pub fg: vt_parser::Color,
    pub bg: vt_parser::Color,
    pub attrs: Attrs,
}

/// 属性位值（缓存键用；与 [`Attrs`] 常量位序一致）。
pub fn attrs_bits(a: Attrs) -> u8 {
    const FLAGS: [Attrs; 8] = [
        Attrs::BOLD,
        Attrs::FAINT,
        Attrs::ITALIC,
        Attrs::UNDERLINE,
        Attrs::BLINK,
        Attrs::INVERSE,
        Attrs::HIDDEN,
        Attrs::STRIKE,
    ];
    let mut bits = 0u8;
    for (i, f) in FLAGS.iter().enumerate() {
        if a.contains(*f) {
            bits |= 1 << i;
        }
    }
    bits
}

/// 把一行的 `[x0, x1)` 列按 (fg, bg, attrs) 分组为 span 列表。
///
/// - 宽字符续格并入前一 span（`width`+1，文本不追加）
/// - 首列即续格（不可能出现在 x0=0 的完整行，但防御性跳过）
/// - 空文本段不会产生（空格也是字符）
pub fn batch_row(view: &ScreenView, row: u16, x0: u16, x1: u16) -> Vec<Span> {
    let Some(cells) = view.row(row) else {
        return Vec::new();
    };
    let x1 = x1.min(view.cols);
    let mut out: Vec<Span> = Vec::new();
    let mut cur: Option<Span> = None;

    for x in x0..x1 {
        let c = &cells[x as usize];
        if c.is_continuation() {
            if let Some(s) = cur.as_mut() {
                s.width += 1;
            }
            continue;
        }
        let mut text = String::with_capacity(1 + c.combining.len());
        text.push(c.ch);
        for &d in &c.combining {
            text.push(d);
        }
        match &mut cur {
            Some(s)
                if s.fg == c.fg
                    && s.bg == c.bg
                    && s.attrs == c.attrs
                    && c.combining.is_empty() =>
            {
                s.text.push_str(&text);
                s.width += 1;
            }
            _ => {
                if let Some(s) = cur.take() {
                    out.push(s);
                }
                cur = Some(Span {
                    row,
                    col: x,
                    text,
                    width: 1,
                    fg: c.fg,
                    bg: c.bg,
                    attrs: c.attrs,
                });
            }
        }
    }
    if let Some(s) = cur.take() {
        out.push(s);
    }
    // 行尾纯空白不用画（背景已整行铺底）：仅当末段「无属性+默认色」时裁掉尾部空格，
    // 带样式的空白段（下划线/反色/着色底）必须保留，否则会出现断线/漏底。
    if let Some(last) = out.last_mut() {
        let plain = last.attrs == Attrs::empty()
            && last.fg == Color::Default
            && last.bg == Color::Default;
        if plain {
            let keep = last.text.trim_end_matches(' ').len();
            let drop = last.text.len() - keep;
            if drop > 0 {
                last.text.truncate(keep);
                last.width = last.width.saturating_sub(drop as u16);
                if last.text.is_empty() {
                    out.pop();
                }
            }
        }
    }
    out
}

/// 整行分段（`0..cols`）。
pub fn batch_full_row(view: &ScreenView, row: u16) -> Vec<Span> {
    batch_row(view, row, 0, view.cols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vt_parser::{Cell, Color, Terminal};

    fn view(term: &Terminal) -> ScreenView {
        term.get_screen()
    }

    #[test]
    fn plain_run_is_single_span() {
        let mut t = Terminal::new(10, 2);
        t.feed(b"hello");
        let v = view(&t);
        let spans = batch_full_row(&v, 0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
        assert_eq!(spans[0].col, 0);
        assert_eq!(spans[0].width, 5);
        assert_eq!(spans[0].fg, Color::Default);
    }

    #[test]
    fn style_change_splits_spans() {
        let mut t = Terminal::new(20, 2);
        t.feed(b"ab\x1b[1;31mcd\x1b[0m");
        let v = view(&t);
        let spans = batch_full_row(&v, 0);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "ab");
        assert!(!spans[0].attrs.contains(vt_parser::Attrs::BOLD));
        assert_eq!(spans[1].text, "cd");
        assert_eq!(spans[1].col, 2);
        assert!(spans[1].attrs.contains(vt_parser::Attrs::BOLD));
        assert_eq!(spans[1].fg, Color::Indexed(1));
    }

    #[test]
    fn wide_char_continuation_extends_width() {
        let mut t = Terminal::new(10, 2);
        t.feed("中x".as_bytes());
        let v = view(&t);
        let spans = batch_full_row(&v, 0);
        assert_eq!(spans.len(), 1, "中(2列)+x 同色应同段: {spans:?}");
        assert_eq!(spans[0].text, "中x");
        assert_eq!(spans[0].width, 3); // 2 + 1
    }

    #[test]
    fn combining_breaks_span_then_continues() {
        let mut t = Terminal::new(10, 2);
        t.feed("xa\u{301}b".as_bytes()); // x + (a + 组合锐音符) + b
        let v = view(&t);
        let spans = batch_full_row(&v, 0);
        // 带组合符的单元格不能并入已有段（必须另起一段），其后的普通字符可继续追加
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert_eq!(spans[0].text, "x");
        assert_eq!(spans[1].text, "a\u{301}b");
        assert_eq!(spans[1].col, 1);
    }

    #[test]
    fn column_range() {
        let mut t = Terminal::new(10, 1);
        t.feed(b"abcdefghij");
        let v = view(&t);
        let spans = batch_row(&v, 0, 3, 6);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "def");
        assert_eq!(spans[0].col, 3);
    }

    #[test]
    fn out_of_range_safe() {
        let t = Terminal::new(4, 1);
        let v = view(&t);
        assert!(batch_full_row(&v, 5).is_empty());
        assert!(batch_row(&v, 0, 0, 99).is_empty() || batch_row(&v, 0, 0, 99)[0].width <= 4);
    }

    #[test]
    fn attrs_bits_values() {
        assert_eq!(attrs_bits(Attrs::empty()), 0);
        assert_eq!(attrs_bits(Attrs::BOLD), 1);
        assert_eq!(attrs_bits(Attrs::INVERSE), 1 << 5);
        let mut both = Attrs::empty();
        both.insert(Attrs::BOLD);
        both.insert(Attrs::STRIKE);
        assert_eq!(attrs_bits(both), 1 | (1 << 7));
    }

    #[test]
    fn cell_helpers() {
        let c = Cell::blank();
        assert!(!c.is_continuation());
        let w = Cell { ch: Cell::CONTINUATION, ..Cell::blank() };
        assert!(w.is_continuation());
    }
}
