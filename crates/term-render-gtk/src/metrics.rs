//! 单元格像素尺寸与网格换算。

/// 单元格像素尺寸（含内边距）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellMetrics {
    /// 单列宽（px，≥1）。
    pub width: u32,
    /// 单行高（px，≥1）。
    pub height: u32,
}

impl Default for CellMetrics {
    fn default() -> Self {
        Self { width: 8, height: 16 }
    }
}

impl CellMetrics {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width: width.max(1), height: height.max(1) }
    }

    /// 由 Pango 字体度量 + 内边距计算。
    ///
    /// `char_width`/`line_height` 为 Pango 单位（`PANGO_SCALE=1024` 倍数），由调用方从
    /// `FontMetrics` 取得后传入；此函数保持纯逻辑可测。
    pub fn from_pango_units(char_width: i32, line_height: i32, pad_x: u32, pad_y: u32) -> Self {
        const SCALE: i32 = 1024; // pango::SCALE
        let px = |v: i32| -> i32 {
            if v <= 0 {
                0
            } else {
                (v + SCALE / 2) / SCALE
            }
        };
        let w = px(char_width).max(1) as u32 + pad_x * 2;
        let h = px(line_height).max(1) as u32 + pad_y * 2;
        Self::new(w, h)
    }

    /// 列 → 像素 x。
    pub fn x(&self, col: u16) -> f64 {
        col as f64 * self.width as f64
    }

    /// 行 → 像素 y。
    pub fn y(&self, row: u16) -> f64 {
        row as f64 * self.height as f64
    }

    /// 像素宽 → 列数（floor，最小 1）。
    pub fn cols_for_width(&self, px: i32) -> u16 {
        let c = px.max(0) as u32 / self.width;
        c.clamp(1, u16::MAX as u32) as u16
    }

    /// 像素高 → 行数（floor，最小 1）。
    pub fn rows_for_height(&self, px: i32) -> u16 {
        let r = px.max(0) as u32 / self.height;
        r.clamp(1, u16::MAX as u32) as u16
    }

    /// 像素 → 网格坐标（floor，夹紧到给定网格内）。
    pub fn hit_test(&self, px: f64, py: f64, cols: u16, rows: u16) -> (u16, u16) {
        let col = (px.max(0.0) / self.width as f64).floor() as i64;
        let row = (py.max(0.0) / self.height as f64).floor() as i64;
        let col = col.clamp(0, cols.saturating_sub(1) as i64) as u16;
        let row = row.clamp(0, rows.saturating_sub(1) as i64) as u16;
        (col, row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cell() {
        let m = CellMetrics::default();
        assert_eq!((m.width, m.height), (8, 16));
    }

    #[test]
    fn clamps_minimum() {
        let m = CellMetrics::new(0, 0);
        assert_eq!((m.width, m.height), (1, 1));
    }

    #[test]
    fn from_pango_units_maths() {
        // PANGO_SCALE = 1024；char 5120 单位 = 5px，行高 12288 = 12px，pad(1,2) → 7x16
        let m = CellMetrics::from_pango_units(5120, 12288, 1, 2);
        assert_eq!((m.width, m.height), (7, 16));
        // 非法输入兜底
        let m = CellMetrics::from_pango_units(0, 0, 0, 0);
        assert!(m.width >= 1 && m.height >= 1);
    }

    #[test]
    fn grid_math() {
        let m = CellMetrics::new(10, 20);
        assert_eq!(m.x(3), 30.0);
        assert_eq!(m.y(2), 40.0);
        assert_eq!(m.cols_for_width(99), 9);
        assert_eq!(m.cols_for_width(0), 1, "最小 1 列");
        assert_eq!(m.rows_for_height(41), 2);
        assert_eq!(m.hit_test(25.0, 39.0, 10, 5), (2, 1));
        assert_eq!(m.hit_test(-5.0, -5.0, 10, 5), (0, 0));
        assert_eq!(m.hit_test(999.0, 999.0, 10, 5), (9, 4));
    }
}
