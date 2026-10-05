//! 渲染管线集成测试（无 GTK：feed → ScreenView → batch 分组 → 光标语义）。

use term_render_gtk::batch::{batch_full_row, attrs_bits};
use term_render_gtk::cache::GlyphKey;
use term_render_gtk::color::{resolve, DEFAULT_BG, DEFAULT_FG};
use term_render_gtk::metrics::CellMetrics;
use vt_parser::{Attrs, Color, Terminal};

#[test]
fn feed_to_spans_pipeline() {
    let mut t = Terminal::new(30, 3);
    t.feed(b"\x1b[2J\x1b[H\x1b[1;32mOK\x1b[0m ready\x1b[4;31m!\x1b[0m");
    let v = t.get_screen();
    let spans = batch_full_row(&v, 0);
    assert_eq!(spans.len(), 3, "{spans:?}");
    assert_eq!(spans[0].text, "OK");
    assert!(spans[0].attrs.contains(Attrs::BOLD));
    assert_eq!(spans[0].fg, Color::Indexed(2));
    assert_eq!(spans[1].text, " ready");
    assert_eq!(spans[1].fg, Color::Default);
    assert!(spans[2].attrs.contains(Attrs::UNDERLINE));
    assert_eq!(spans[2].fg, Color::Indexed(1));
    // damage：有输出的行
    assert!(v.damage.contains(&0));
}

#[test]
fn cursor_feed_semantics() {
    let mut t = Terminal::new(10, 5);
    t.feed(b"abc\x1b[3;4H");
    let v = t.get_screen();
    assert_eq!((v.cursor.x, v.cursor.y), (3, 2));
    assert!(v.cursor.visible);
    t.feed(b"\x1b[?25l");
    assert!(!t.get_screen().cursor.visible);
    t.feed(b"\x1b[?25h");
    assert!(t.get_screen().cursor.visible);
}

#[test]
fn clear_damage_then_partial() {
    let mut t = Terminal::new(10, 5);
    t.feed(b"line1\r\nline2\r\nline3");
    let v = t.get_screen();
    assert!(!v.damage.is_empty());
    t.clear_damage();
    assert!(t.get_screen().damage.is_empty());
    t.feed(b"Z"); // 光标在 row2
    let v2 = t.get_screen();
    assert_eq!(v2.damage, vec![2]);
}

#[test]
fn resize_marks_damage_and_keeps_content() {
    let mut t = Terminal::new(20, 4);
    t.feed(b"hello world");
    t.clear_damage();
    t.resize(30, 8);
    let v = t.get_screen();
    assert_eq!((v.cols, v.rows), (30, 8));
    assert_eq!(v.line_text(0), "hello world");
    assert!(!v.damage.is_empty(), "resize 应标记全量 damage");
}

#[test]
fn glyph_key_and_bits_align_with_batch() {
    let mut t = Terminal::new(10, 2);
    t.feed(b"\x1b[1;4;7mXY\x1b[0m");
    let v = t.get_screen();
    let spans = batch_full_row(&v, 0);
    assert_eq!(spans.len(), 1);
    let a = spans[0].attrs;
    let bits = attrs_bits(a);
    assert_eq!(bits, 1 | (1 << 3) | (1 << 5)); // bold|underline|inverse
    let key = GlyphKey::from_colors(a_fg(a), Color::Default, a, "XY");
    assert_eq!(key.attrs, bits);
    assert_eq!(key.fg, DEFAULT_FG); // Default → 默认前景
    let _ = DEFAULT_BG;
}

fn a_fg(a: Attrs) -> Color {
    let _ = a;
    Color::Default
}

#[test]
fn metrics_drive_grid_layout() {
    let m = CellMetrics::new(8, 16);
    let mut t = Terminal::new(m.cols_for_width(640), m.rows_for_height(240));
    assert_eq!((t.get_screen().cols, t.get_screen().rows), (80, 15));
    t.feed(b"edge");
    let v = t.get_screen();
    assert_eq!(m.x(v.cols - 1) + m.width as f64, 640.0);
}

#[test]
fn wide_char_grid_alignment() {
    let mut t = Terminal::new(10, 2);
    t.feed("汉字ab".as_bytes());
    let v = t.get_screen();
    let spans = batch_full_row(&v, 0);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].width, 6); // 2+2+1+1
    // 续格占位
    assert!(v.cell(1, 0).unwrap().is_continuation());
}

#[test]
fn resolve_defaults_used_by_key() {
    assert_eq!(resolve(Color::Default, DEFAULT_FG), DEFAULT_FG);
    assert_eq!(resolve(Color::Default, DEFAULT_BG), DEFAULT_BG);
}
