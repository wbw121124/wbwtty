//! 颜色解析：vt-parser `Color` → RGB（xterm 256 色调色板）。

use vt_parser::Color;

/// 8-bit RGB。
pub type Rgb = (u8, u8, u8);

/// 默认前景（亮灰）。
pub const DEFAULT_FG: Rgb = (0xd0, 0xd0, 0xd0);
/// 默认背景（近黑）。
pub const DEFAULT_BG: Rgb = (0x10, 0x10, 0x14);

/// xterm 基本 16 色。
const BASE16: [Rgb; 16] = [
    (0, 0, 0),
    (128, 0, 0),
    (0, 128, 0),
    (128, 128, 0),
    (0, 0, 128),
    (128, 0, 128),
    (0, 128, 128),
    (192, 192, 192),
    (128, 128, 128),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (0, 0, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// 256 色索引 → RGB。
pub fn indexed_rgb(i: u8) -> Rgb {
    match i {
        0..=15 => BASE16[i as usize],
        16..=231 => {
            let n = i - 16;
            let step = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (step(n / 36), step((n / 6) % 6), step(n % 6))
        }
        // 24 级灰 8..238
        _ => {
            let g = 8 + 10 * (i - 232);
            (g, g, g)
        }
    }
}

/// 解析为 RGB；`Default` 用提供的默认色。
pub fn resolve(c: Color, default: Rgb) -> Rgb {
    match c {
        Color::Default => default,
        Color::Indexed(i) => indexed_rgb(i),
        Color::Rgb(r, g, b) => (r, g, b),
    }
}

/// 转 cairo 浮点分量。
pub fn to_f64(c: Rgb) -> (f64, f64, f64) {
    (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base16_mapping() {
        assert_eq!(indexed_rgb(0), (0, 0, 0));
        assert_eq!(indexed_rgb(9), (255, 0, 0));
        assert_eq!(indexed_rgb(15), (255, 255, 255));
    }

    #[test]
    fn color_cube_6x6x6() {
        // 索引 = 16 + r*36 + g*6 + b，分量 = 0 或 55+40*n
        assert_eq!(indexed_rgb(16), (0, 0, 0)); // cube 0,0,0
        assert_eq!(indexed_rgb(231), (255, 255, 255)); // cube 5,5,5
        assert_eq!(indexed_rgb(17), (0, 0, 95)); // cube 0,0,1
        assert_eq!(indexed_rgb(21), (0, 0, 255)); // cube 0,0,5
        assert_eq!(indexed_rgb(196), (255, 0, 0)); // cube 5,0,0
        assert_eq!(indexed_rgb(46), (0, 255, 0)); // cube 0,5,0
        assert_eq!(indexed_rgb(28), (0, 135, 0)); // cube 0,2,0 -> 55+40*2
    }

    #[test]
    fn grayscale_ramp() {
        assert_eq!(indexed_rgb(232), (8, 8, 8));
        assert_eq!(indexed_rgb(255), (238, 238, 238));
    }

    #[test]
    fn resolve_colors() {
        assert_eq!(resolve(Color::Default, (1, 2, 3)), (1, 2, 3));
        assert_eq!(resolve(Color::Indexed(9), (1, 2, 3)), (255, 0, 0));
        assert_eq!(resolve(Color::Rgb(10, 20, 30), (1, 2, 3)), (10, 20, 30));
        assert_eq!(to_f64((255, 128, 0)), (1.0, 128.0 / 255.0, 0.0));
    }
}
