//! 滚动视口：把「回滚缓冲 + 实屏」拼成一屏 [`ScreenView`]（纯逻辑，无 GTK）。
//!
//! 坐标系：`0..scrollback_len` 为回滚历史行，其后 `0..rows` 为实屏行。
//! `offset` 表示视口底端离屏幕底部上移多少行（`0` = 跟随实屏）。

use vt_parser::{Cell, CursorState, ScreenView, Terminal};

/// 构造滚动视口快照。
///
/// - `offset == 0`：直接返回实屏快照（零拷贝）
/// - 备用屏无回滚缓冲，任何 `offset` 都返回实屏
/// - `offset` 超过历史长度时夹到最老一行
/// - 有偏移时光标隐藏、damage 全量（滚动期间必然整屏重绘）
pub fn build_viewport(terminal: &Terminal, offset: usize) -> ScreenView {
    let live = terminal.get_screen();
    let sc_len = terminal.scrollback_len();
    if offset == 0 || live.is_alt || sc_len == 0 {
        return live;
    }
    let offset = offset.min(sc_len);
    let total = sc_len + live.rows as usize;
    let bottom = total - offset; // 视口底端（不含）
    let top = bottom - live.rows as usize; // bottom >= rows 恒成立
    let cols = live.cols as usize;
    let mut cells: Vec<Cell> = Vec::with_capacity(live.cells.len());

    for line in top..bottom {
        if line < sc_len {
            match terminal.scrollback_line(line) {
                Some(row) => cells.extend_from_slice(row),
                None => cells.extend((0..cols).map(|_| Cell::blank())),
            }
        } else {
            let r = line - sc_len;
            if r < live.rows as usize {
                let start = r * cols;
                cells.extend_from_slice(&live.cells[start..start + cols]);
            } else {
                cells.extend((0..cols).map(|_| Cell::blank()));
            }
        }
    }

    // 兜底：行数必须严格等于 cols*rows，否则 ScreenView::row 会越界
    cells.resize(cols * live.rows as usize, Cell::blank());

    ScreenView {
        cols: live.cols,
        rows: live.rows,
        cells,
        cursor: CursorState { visible: false, ..live.cursor },
        damage: (0..live.rows).collect(),
        is_alt: live.is_alt,
        scrollback_len: live.scrollback_len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fed_terminal() -> Terminal {
        let mut t = Terminal::new(10, 3);
        // 5 行内容 -> 行1 被挤进回滚缓冲
        t.feed(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        t
    }

    #[test]
    fn offset_zero_is_live_screen() {
        let t = fed_terminal();
        let v = build_viewport(&t, 0);
        let live = t.get_screen();
        assert_eq!(v.cells, live.cells);
        assert_eq!(v.cursor.visible, live.cursor.visible);
    }

    #[test]
    fn scrollback_got_a_line() {
        let t = fed_terminal();
        assert!(t.scrollback_len() >= 1, "应已有回滚行");
    }

    #[test]
    fn offset_one_puts_oldest_line_on_top() {
        let t = fed_terminal();
        let sc = t.scrollback_len();
        let v = build_viewport(&t, 1);
        assert_eq!((v.cols, v.rows), (10, 3));
        assert_eq!(v.cells.len(), 30);
        // 视口底端上移 1 行 -> 顶行 = 历史倒数第二行（sc-1 处为最后一条历史）
        let expected_top = t.scrollback_line(sc - 1).unwrap();
        assert_eq!(&v.cells[0..10], expected_top, "顶行应是最新一条回滚行");
    }

    #[test]
    fn offset_beyond_history_clamps() {
        let t = fed_terminal();
        let max = t.scrollback_len();
        let a = build_viewport(&t, max);
        let b = build_viewport(&t, max + 10_000);
        assert_eq!(a.cells, b.cells, "越界 offset 应夹到最老一行");
        // 最老一行是 "one"
        assert_eq!(v_line_text(&a, 0), "one");
    }

    #[test]
    fn cursor_hidden_while_scrolled() {
        let mut t = fed_terminal();
        t.feed(b"Z");
        assert!(t.get_screen().cursor.visible);
        let v = build_viewport(&t, 1);
        assert!(!v.cursor.visible, "回滚视图不画光标");
        assert_eq!(v.damage.len(), v.rows as usize, "滚动期全量 damage");
    }

    #[test]
    fn alt_screen_never_scrolls() {
        let mut t = Terminal::new(10, 3);
        t.feed(b"\x1b[?1049h"); // 切备用屏
        t.feed(b"alt1\r\nalt2");
        let v = build_viewport(&t, 5);
        assert!(v.is_alt);
        assert_eq!(v.cells, t.get_screen().cells);
    }

    fn v_line_text(v: &ScreenView, y: u16) -> String {
        v.line_text(y)
    }
}
