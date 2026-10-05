# term-render-gtk

GTK3 + Cairo 终端渲染器：damage 驱动重绘、Pango 字形缓存、真彩色、光标、回滚缓冲、
resize 联动。最近更新：2026-10-05。

## 职责

| 能力 | API / 模块 |
| --- | --- |
| 单元格分段 | `batch::{batch_row, batch_full_row, attrs_bits}`（同色同属性合并；行尾纯空白裁掉） |
| 颜色 | `color::{resolve, indexed_rgb, to_f64}`（xterm 16/256 色 + 真彩色直通） |
| 字形缓存 | `cache::GlyphCache<pango::Layout>`，键 = (fg, bg, attrs, text)，带命中统计 |
| 单元格度量 | `metrics::CellMetrics`（Pango 度量 → px；网格↔像素、hit_test） |
| 回滚视口 | `viewport::build_viewport(&Terminal, offset)` → 一屏 `ScreenView` |
| 绘制 | `renderer::Renderer::draw(cr, view, force)`：背景 → 文本 → 下划线/删除线 → 光标 |
| 控件 | `widget::TerminalView`：`DrawingArea` 封装、damage→queue_draw、resize 回调、滚轮回滚 |
| pangocairo FFI | `pangocairo::show_layout`（gtk-rs 0.18 未绑定 `pango_cairo_*`，自声明一个符号） |

## 绘制流程

1. `TerminalView::new` 用控件的 Pango 上下文 `measure` 出单元格尺寸并设 `size_request`
2. `connect_draw` 取快照（回滚中用 `build_viewport`），`force` 或 `damage` 行才重画，
   画完 `Terminal::clear_damage`
3. 行循环：先整行刷默认底 + 非默认 `bg` 单元格（宽字符连同续格）铺底；
   再按 `batch_row` 的 span 建 Pango Layout（缓存命中即复用）绘制文本、下划线、删除线；
   最后画光标（块/竖条，块光标内文本反色）
4. `INVERSE` 交换 fg/bg、`FAINT` ×0.6、`HIDDEN` 跳过、粗体/斜体走 Pango 权重与字形样式
5. `connect_size_allocate`：按度量算网格 → `set_on_resize` 回调（调用方负责
   `Terminal::resize`）→ 全屏重绘

## 回滚缓冲

- 滚轮（或 `scroll_by(delta)`）改变 `scroll_offset`，`0` = 跟随实屏
- 偏移 >0：视口 = 历史行 + 实屏行拼成的 `ScreenView`，**整屏重绘、隐藏光标**
- `offset` 夹紧到 `[0, scrollback_len]`；备用屏（is_alt）无回滚，始终跟随实屏
- `scroll_to_bottom()` 回到底部，`is_scrolled()`/`scroll_offset()` 查询

## 构建要求

- Linux/macOS 之外的本机：`pkg-config` 必须能找到 `pangocairo`
  （Windows/MSYS2：`<MSYS2>\ucrt64\bin` 前置 PATH，已装 `mingw-w64-ucrt-x86_64-gtk3`；
  `scripts/env.ps1` 按候选路径自动前置）
- 运行需要 GTK3 动态库在同一 PATH 上
- 本 crate 的 `build.rs` 用 pkg-config 探测 `pangocairo` 并链接 `libpangocairo-1.0`

## 运行

```text
cargo run -p term-render-gtk --example gtk_smoke     # 链接/运行时冒烟
cargo run -p term-render-gtk --example demo_window   # 动画演示窗口（关窗退出）
```

## 测试

`cargo test -p term-render-gtk`：31 单元（batch/color/cache/metrics/viewport/renderer）
+ 8 集成（`tests/pipeline.rs`：feed→span、光标、damage、resize、宽字符、网格度量）。
依赖 GTK 运行库但**不开窗口**，CI Ubuntu 上可跑。
