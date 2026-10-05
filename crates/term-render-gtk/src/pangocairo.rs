//! 极简 pangocairo 绑定。
//!
//! gtk-rs 0.18 没有绑定 `pango_cairo_*`（`pango` crate 无 `cairo` 子模块），
//! 而把 Pango 布局画到 Cairo 只需要一个符号；由 `build.rs` 经 pkg-config 链接
//! `libpangocairo-1.0`。

use gtk::glib::translate::ToGlibPtr;
use gtk::{cairo, pango};

extern "C" {
    fn pango_cairo_show_layout(cr: *mut cairo::ffi::cairo_t, layout: *mut pango::ffi::PangoLayout);
}

/// 把 Pango 布局绘制到 Cairo 上下文的 `(x, y)`（布局左上角）。
///
/// 走 Pango 的完整塑形（宽字符、组合符、复杂文种），不用 Cairo 玩具字体 API。
pub fn show_layout(cr: &cairo::Context, layout: &pango::Layout, x: f64, y: f64) {
    cr.move_to(x, y);
    unsafe {
        pango_cairo_show_layout(cr.to_raw_none(), layout.to_glib_none().0);
    }
}
