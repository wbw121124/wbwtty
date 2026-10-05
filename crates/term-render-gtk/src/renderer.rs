//! Cairo 渲染：背景 → 文本 span（字形缓存）→ 下划线/删除线 → 光标。

use gtk::{cairo, pango};

use crate::batch::batch_row;
use crate::cache::{GlyphCache, GlyphKey, CacheStats};
use crate::color::{resolve, to_f64, Rgb, DEFAULT_BG, DEFAULT_FG};
use crate::metrics::CellMetrics;
use crate::pangocairo;
use vt_parser::{Attrs, ScreenView};

/// 渲染配置。
#[derive(Clone, Debug)]
pub struct RenderConfig {
    /// 字体（含字号）。
    pub font: pango::FontDescription,
    /// `Color::Default` 前景。
    pub fg_default: Rgb,
    /// `Color::Default` 背景。
    pub bg_default: Rgb,
    /// 块光标填充色 / 块光标下文本色。
    pub cursor_bg: Rgb,
    pub cursor_fg: Rgb,
    /// 单元格水平/垂直内边距（px）。
    pub pad_x: u32,
    pub pad_y: u32,
    /// 是否绘制光标。
    pub show_cursor: bool,
    /// `true`=竖条光标，`false`=块光标。
    pub bar_cursor: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            font: pango::FontDescription::from_string("Monospace 12"),
            fg_default: DEFAULT_FG,
            bg_default: DEFAULT_BG,
            cursor_bg: (0x70, 0x78, 0x88),
            cursor_fg: (0x10, 0x10, 0x14),
            pad_x: 1,
            pad_y: 1,
            show_cursor: true,
            bar_cursor: false,
        }
    }
}

/// 渲染器（持有 Pango 上下文与字形缓存）。
pub struct Renderer {
    config: RenderConfig,
    cache: GlyphCache<pango::Layout>,
    metrics: Option<CellMetrics>,
    pctx: Option<pango::Context>,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self { config, cache: GlyphCache::new(), metrics: None, pctx: None }
    }

    pub fn with_defaults() -> Self {
        Self::new(RenderConfig::default())
    }

    pub fn config(&self) -> &RenderConfig {
        &self.config
    }

    /// 修改配置；字体变化清字形缓存并作废度量（下次 `draw` 自动重新 `measure`）。
    pub fn config_mut(&mut self) -> &mut RenderConfig {
        self.cache.clear();
        self.metrics = None;
        &mut self.config
    }

    /// 用 Pango 上下文度量单元格尺寸（构造后、draw 前由 Widget 调用）。
    pub fn measure(&mut self, ctx: &pango::Context) {
        let fm = ctx.metrics(Some(&self.config.font), None);
        let char_w = fm.approximate_char_width();
        let line_h = fm.ascent() + fm.descent();
        self.metrics = Some(CellMetrics::from_pango_units(
            char_w,
            line_h,
            self.config.pad_x,
            self.config.pad_y,
        ));
        self.pctx = Some(ctx.clone());
    }

    pub fn metrics(&self) -> CellMetrics {
        self.metrics.unwrap_or_default()
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// 画屏幕。
    ///
    /// - `force=true`：全屏重绘（首帧/resize 后）
    /// - `force=false`：仅 `view.damage` 行 + 光标行
    ///
    /// 从未 [`Renderer::measure`] 过时为 no-op（安全）。
    pub fn draw(&mut self, cr: &cairo::Context, view: &ScreenView, force: bool) {
        // 配置改动作废度量后，用保留的 Pango 上下文重新度量
        if self.metrics.is_none() {
            if let Some(ctx) = self.pctx.clone() {
                self.measure(&ctx);
            }
        }
        let (Some(pctx), Some(_)) = (self.pctx.clone(), self.metrics) else {
            return;
        };
        let m = self.metrics();
        let cell_w = m.width as f64;
        let cell_h = m.height as f64;

        // ---- 重画行集合 ----
        let rows: Vec<u16> = if force {
            (0..view.rows).collect()
        } else {
            let mut r: Vec<u16> =
                view.damage.iter().copied().filter(|&y| y < view.rows).collect();
            if self.config.show_cursor && view.cursor.visible {
                r.push(view.cursor.y.min(view.rows.saturating_sub(1)));
            }
            r.sort_unstable();
            r.dedup();
            r
        };
        if rows.is_empty() {
            return;
        }

        let (fg_d, bg_d) = (self.config.fg_default, self.config.bg_default);
        let (bg_r, bg_g, bg_b) = to_f64(bg_d);

        // ---- 背景：整行刷默认底，非默认 bg 单元格填充 ----
        for &row in &rows {
            let y = m.y(row);
            cr.set_source_rgb(bg_r, bg_g, bg_b);
            cr.rectangle(0.0, y, view.cols as f64 * cell_w, cell_h);
            cr.fill().ok();
            for x in 0..view.cols {
                let Some(cell) = view.cell(x, row) else { continue };
                if cell.is_continuation() {
                    continue;
                }
                let bg = resolve(cell.bg, bg_d);
                if bg == bg_d {
                    continue;
                }
                // 宽字符：连同右侧续格一起铺底
                let wide = view
                    .cell(x + 1, row)
                    .map(|n| n.is_continuation())
                    .unwrap_or(false);
                let w = if wide { cell_w * 2.0 } else { cell_w };
                let (r, g, b) = to_f64(bg);
                cr.set_source_rgb(r, g, b);
                cr.rectangle(m.x(x), y, w, cell_h);
                cr.fill().ok();
            }
        }

        // ---- 文本 span ----
        let pad_y = self.config.pad_y as f64;
        for &row in &rows {
            let y = m.y(row);
            for sp in batch_row(view, row, 0, view.cols) {
                let mut fg = resolve(sp.fg, fg_d);
                let mut bg = resolve(sp.bg, bg_d);
                if sp.attrs.contains(Attrs::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if sp.attrs.contains(Attrs::HIDDEN) {
                    continue;
                }
                if sp.attrs.contains(Attrs::FAINT) {
                    fg = dim(fg);
                }
                let bold = sp.attrs.contains(Attrs::BOLD);
                let italic = sp.attrs.contains(Attrs::ITALIC);
                let underline = sp.attrs.contains(Attrs::UNDERLINE);
                let strike = sp.attrs.contains(Attrs::STRIKE);

                let x = m.x(sp.col);
                let key = GlyphKey::new(fg, bg, sp.attrs, sp.text.clone());
                let font = self.config.font.clone();
                let text = sp.text.clone();
                let layout = self.cache.get_or_insert_with(key, || {
                    let l = pango::Layout::new(&pctx);
                    let mut fd = font;
                    if bold {
                        fd.set_weight(pango::Weight::Bold);
                    }
                    if italic {
                        fd.set_style(pango::Style::Italic);
                    }
                    l.set_font_description(Some(&fd));
                    l.set_text(&text);
                    l
                });
                let (fr, fgb, fb) = to_f64(fg);
                cr.set_source_rgb(fr, fgb, fb);
                pangocairo::show_layout(cr, layout, x, y + pad_y);

                let span_px = sp.width as f64 * cell_w;
                if underline {
                    cr.rectangle(x, y + cell_h - 2.0, span_px, 1.0);
                    cr.fill().ok();
                }
                if strike {
                    cr.rectangle(x, y + (cell_h / 2.0).floor(), span_px, 1.0);
                    cr.fill().ok();
                }
            }
        }

        // ---- 光标 ----
        if self.config.show_cursor && view.cursor.visible && view.rows > 0 {
            let cx = view.cursor.x.min(view.cols.saturating_sub(1));
            let cy = view.cursor.y.min(view.rows.saturating_sub(1));
            // 光标行可能不在 damage 中（force=false 时已并入 rows）
            if force || rows.contains(&cy) {
                self.draw_cursor(cr, view, m, cx, cy, fg_d, bg_d);
            }
        }
    }

    fn draw_cursor(
        &mut self,
        cr: &cairo::Context,
        view: &ScreenView,
        m: CellMetrics,
        cx: u16,
        cy: u16,
        fg_d: Rgb,
        bg_d: Rgb,
    ) {
        let cell_w = m.width as f64;
        let cell_h = m.height as f64;
        let x = m.x(cx);
        let y = m.y(cy);
        let (cur_r, cur_g, cur_b) = to_f64(self.config.cursor_bg);

        if self.config.bar_cursor {
            cr.set_source_rgb(cur_r, cur_g, cur_b);
            cr.rectangle(x, y, 2.0, cell_h);
            cr.fill().ok();
            return;
        }

        // 块光标：填充后用反色画该格文本
        cr.set_source_rgb(cur_r, cur_g, cur_b);
        cr.rectangle(x, y, cell_w, cell_h);
        cr.fill().ok();

        let Some(cell) = view.cell(cx, cy) else { return };
        let ch = if cell.is_continuation() {
            view.cell(cx.wrapping_sub(1), cy).map(|c| c.ch).filter(|&c| c != ' ')
        } else if cell.ch == ' ' {
            None
        } else {
            Some(cell.ch)
        };
        let Some(ch) = ch else { return };
        let mut text = String::new();
        text.push(ch);
        for &d in &cell.combining {
            text.push(d);
        }

        // INVERSE 已把光标格的 fg/bg 语义翻转：块光标下显示 cursor_fg
        let fg = self.config.cursor_fg;
        let bg = self.config.cursor_bg;
        let key = GlyphKey::new(fg, bg, cell.attrs, text);
        let font = self.config.font.clone();
        let text2 = key.text.clone();
        let pctx = self
            .pctx
            .clone()
            .expect("draw_cursor only runs after measure");
        let bold = cell.attrs.contains(Attrs::BOLD);
        let italic = cell.attrs.contains(Attrs::ITALIC);
        let layout = self.cache.get_or_insert_with(key, || {
            let l = pango::Layout::new(&pctx);
            let mut fd = font;
            if bold {
                fd.set_weight(pango::Weight::Bold);
            }
            if italic {
                fd.set_style(pango::Style::Italic);
            }
            l.set_font_description(Some(&fd));
            l.set_text(&text2);
            l
        });
        let (r, g, b) = to_f64(fg);
        cr.set_source_rgb(r, g, b);
        pangocairo::show_layout(cr, layout, x, y + self.config.pad_y as f64);

        if cell.attrs.contains(Attrs::UNDERLINE) {
            cr.rectangle(x, y + cell_h - 2.0, cell_w, 1.0);
            cr.fill().ok();
        }
        // fg_d/bg_d 参数保留（后续反色块光标文本微调用）
        let _ = (fg_d, bg_d);
    }
}

/// FAINT：亮度 ×0.6。
fn dim(c: Rgb) -> Rgb {
    (((c.0 as f32) * 0.6) as u8, ((c.1 as f32) * 0.6) as u8, ((c.2 as f32) * 0.6) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let c = RenderConfig::default();
        assert!(c.pad_x < 8 && c.pad_y < 8);
        assert!(c.show_cursor);
    }

    #[test]
    fn dim_scales_down() {
        assert_eq!(dim((100, 200, 255)), (60, 120, 153));
        assert_eq!(dim((0, 0, 0)), (0, 0, 0));
    }

    #[test]
    fn renderer_starts_without_measure() {
        // 未 measure 时 draw 安全退出（无 GTK 环境可测 new/metrics）
        let r = Renderer::with_defaults();
        assert_eq!(r.metrics(), CellMetrics::default());
        assert_eq!(r.cache_stats().len, 0);
    }

    #[test]
    fn config_default_font_parses() {
        let fd = pango::FontDescription::from_string("Monospace 14");
        assert_eq!(fd.size(), 14 * pango::SCALE);
    }
}
