//! term-render-gtk — GTK3 + Cairo 终端渲染器（damage 驱动重绘）。
//!
//! 分层：
//! - [`batch`] / [`color`]：纯逻辑（单元格分组、颜色转换），无 GTK 运行时依赖，可独立测试
//! - [`metrics`]：字体度量与单元格尺寸/坐标换算
//! - [`cache`]：Pango 布局缓存（按 文本+样式+前景色 键控，带命中统计）
//! - [`viewport`]：回滚缓冲 + 实屏 → 一屏快照（纯逻辑，可独立测试）
//! - [`pangocairo`]：gtk-rs 0.18 未绑定的 `pango_cairo_show_layout` 最小 FFI
//! - [`renderer`]：把 vt-parser `ScreenView` 画到 Cairo（背景/文本/光标）
//! - [`widget`]：`gtk::DrawingArea` 封装（damage → queue_draw、resize → 网格回调、滚轮回滚）
//!
//! 使用前提：调用方先 `gtk::init()`，终端状态由 vt-parser `Terminal` 持有，
//! 本 crate 只负责绘制与坐标换算。

pub mod batch;
pub mod cache;
pub mod color;
pub mod metrics;
pub mod pangocairo;
pub mod renderer;
pub mod viewport;
pub mod widget;

pub use cache::GlyphCache;
pub use metrics::CellMetrics;
pub use renderer::{RenderConfig, Renderer};
pub use viewport::build_viewport;
pub use widget::TerminalView;

/// 重新导出 GTK 栈（调用方无需直接依赖 gtk crate 版本）。
pub use gtk;
