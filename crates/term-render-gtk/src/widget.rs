//! `gtk::DrawingArea` 封装：damage 驱动重绘、网格尺寸回调。

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::DrawingArea;
use vt_parser::Terminal;

use crate::metrics::CellMetrics;
use crate::renderer::{RenderConfig, Renderer};
use crate::viewport::build_viewport;

/// 视图共享状态。
struct ViewState {
    terminal: Rc<RefCell<Terminal>>,
    renderer: Renderer,
    on_resize: Option<Box<dyn FnMut(u16, u16)>>,
    /// 下一帧全屏重绘（首帧/resize/显式请求）。
    pending_force: bool,
    /// 回滚视口偏移：0=跟随实屏，>0=向上翻看的历史行数。
    scroll_offset: usize,
}

/// 终端视图：持有 `DrawingArea`，从共享 `Terminal` 读快照绘制。
///
/// 约定：
/// - 调用方在 feed 后调用 [`TerminalView::queue_redraw`]
/// - resize 通过 [`TerminalView::set_on_resize`] 回调交给调用方改 `Terminal`，
///   回调后视图自动请求全屏重绘
/// - draw 完成后自动 `Terminal::clear_damage`
/// - 滚轮（或 [`TerminalView::scroll_by`])上下翻看回滚缓冲；偏移 >0 时整屏重绘且不画光标
pub struct TerminalView {
    da: DrawingArea,
    state: Rc<RefCell<ViewState>>,
}

impl TerminalView {
    /// 构造（要求已 `gtk::init()`）。
    pub fn new(terminal: Rc<RefCell<Terminal>>, config: RenderConfig) -> Self {
        let da = DrawingArea::new();
        let mut renderer = Renderer::new(config);
        // 用 Widget 的 Pango 上下文度量（无需 realized）
        let ctx = da.create_pango_context();
        renderer.measure(&ctx);

        let state = Rc::new(RefCell::new(ViewState {
            terminal: terminal.clone(),
            renderer,
            on_resize: None,
            pending_force: true, // 首帧全绘
            scroll_offset: 0,
        }));

        {
            let st = state.clone();
            da.connect_draw(move |_w, cr| {
                let mut s = st.borrow_mut();
                let force = s.pending_force || s.scroll_offset > 0;
                s.pending_force = false;
                let view = build_viewport(&s.terminal.borrow(), s.scroll_offset);
                s.renderer.draw(cr, &view, force);
                s.terminal.borrow_mut().clear_damage();
                gtk::glib::Propagation::Proceed
            });
        }
        {
            let st = state.clone();
            da.connect_scroll_event(move |w, ev| {
                let mut s = st.borrow_mut();
                let lines: i32 = match ev.direction() {
                    gtk::gdk::ScrollDirection::Up => 3,
                    gtk::gdk::ScrollDirection::Down => -3,
                    gtk::gdk::ScrollDirection::Smooth => {
                        let dy = ev.delta().1;
                        if dy.abs() < f64::EPSILON {
                            return gtk::glib::Propagation::Proceed;
                        }
                        (-dy * 6.0).round() as i32
                    }
                    _ => return gtk::glib::Propagation::Proceed,
                };
                if s.scroll_by(lines) {
                    s.pending_force = true;
                    w.queue_draw();
                }
                gtk::glib::Propagation::Proceed
            });
        }
        {
            let st = state.clone();
            da.connect_size_allocate(move |_w, alloc| {
                let mut s = st.borrow_mut();
                let m = s.renderer.metrics();
                let cols = m.cols_for_width(alloc.width());
                let rows = m.rows_for_height(alloc.height());
                let view = s.terminal.borrow().get_screen();
                if cols != view.cols || rows != view.rows {
                    if let Some(cb) = s.on_resize.as_mut() {
                        cb(cols, rows);
                    }
                    s.pending_force = true;
                    _w.queue_draw();
                }
            });
        }

        let view = terminal.borrow().get_screen();
        let m = renderer_metrics(&state);
        da.set_size_request(
            (view.cols as i32 * m.width as i32).max(1),
            (view.rows as i32 * m.height as i32).max(1),
        );

        Self { da, state }
    }

    /// 底层控件（放入容器用）。
    pub fn widget(&self) -> &DrawingArea {
        &self.da
    }

    /// 有 damage 后请求重绘（只画脏行）。
    pub fn queue_redraw(&self) {
        self.da.queue_draw();
    }

    /// 请求下一帧全屏重绘。
    pub fn queue_full_redraw(&self) {
        self.state.borrow_mut().pending_force = true;
        self.da.queue_draw();
    }

    /// 网格尺寸变化回调（调用方负责 `Terminal::resize`）。
    pub fn set_on_resize(&self, f: impl FnMut(u16, u16) + 'static) {
        self.state.borrow_mut().on_resize = Some(Box::new(f));
    }

    /// 上下翻看回滚缓冲。`delta > 0` 向上（看更早的输出），`< 0` 向下。
    ///
    /// 返回值 `true` 表示偏移确实变化了（调用方通常无需处理，控件内部已请求重绘）。
    pub fn scroll_by(&self, delta: i32) -> bool {
        let changed = self.state.borrow_mut().scroll_by(delta);
        if changed {
            self.da.queue_draw();
        }
        changed
    }

    /// 回到实屏底部（0 = 跟随最新输出）。
    pub fn scroll_to_bottom(&self) -> bool {
        self.scroll_by(i32::MIN)
    }

    /// 当前回滚偏移（0 表示跟随实屏）。
    pub fn scroll_offset(&self) -> usize {
        self.state.borrow().scroll_offset
    }

    /// 是否处于回滚查看状态。
    pub fn is_scrolled(&self) -> bool {
        self.state.borrow().scroll_offset > 0
    }

    pub fn metrics(&self) -> CellMetrics {
        self.state.borrow().renderer.metrics()
    }

    pub fn cache_stats(&self) -> crate::cache::CacheStats {
        self.state.borrow().renderer.cache_stats()
    }

    /// 像素 → 网格坐标（鼠标事件换算用）。
    pub fn hit_test(&self, px: f64, py: f64) -> (u16, u16) {
        let s = self.state.borrow();
        let v = s.terminal.borrow().get_screen();
        s.renderer.metrics().hit_test(px, py, v.cols, v.rows)
    }
}

fn renderer_metrics(state: &Rc<RefCell<ViewState>>) -> CellMetrics {
    state.borrow().renderer.metrics()
}

impl ViewState {
    /// 调整回滚偏移，夹紧到 `[0, scrollback_len]`；返回是否发生变化。
    fn scroll_by(&mut self, delta: i32) -> bool {
        let sc_len = self.terminal.borrow().scrollback_len() as i64;
        let cur = self.scroll_offset as i64;
        // i32::MIN 作为「回到底部」的哨兵，避免 abs 溢出
        let next = if delta == i32::MIN {
            0
        } else {
            (cur + i64::from(delta)).clamp(0, sc_len)
        };
        if next == cur {
            return false;
        }
        self.scroll_offset = next as usize;
        true
    }
}

#[cfg(test)]
mod tests {
    // widget 引 GTK 运行时，无显示环境不单测；纯逻辑见 batch/color/cache/metrics 测试。
}
