//! 字形布局缓存：按 (fg, bg, attrs, text) 键控 Pango Layout，带代龄失效与命中统计。

use std::collections::HashMap;

use crate::batch::attrs_bits;
use vt_parser::{Attrs, Color};

use crate::color::{resolve, Rgb, DEFAULT_BG, DEFAULT_FG};

/// 缓存键：样式 + 文本。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct GlyphKey {
    pub fg: Rgb,
    pub bg: Rgb,
    pub attrs: u8,
    pub text: String,
}

impl GlyphKey {
    /// 由单元格属性构造（`INVERSE` 交换前后景由调用方先解析，或在此传入已交换值）。
    pub fn new(fg: Rgb, bg: Rgb, attrs: Attrs, text: impl Into<String>) -> Self {
        Self { fg, bg, attrs: attrs_bits(attrs), text: text.into() }
    }

    /// 由 vt-parser 颜色构造（`Default` 落到默认前景/背景）。
    pub fn from_colors(fg: Color, bg: Color, attrs: Attrs, text: impl Into<String>) -> Self {
        Self::new(resolve(fg, DEFAULT_FG), resolve(bg, DEFAULT_BG), attrs, text)
    }
}

/// 命中/未命中统计。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub len: usize,
}

/// 通用值缓存（值类型抽象为 `T`，便于无 GTK 单测；渲染时 `T = pango::Layout`）。
pub struct GlyphCache<T> {
    map: HashMap<GlyphKey, T>,
    hits: u64,
    misses: u64,
}

impl<T> GlyphCache<T> {
    pub fn new() -> Self {
        Self { map: HashMap::new(), hits: 0, misses: 0 }
    }

    /// 取或建。返回值的生命周期借用缓存自身（渲染时随用随画）。
    pub fn get_or_insert_with(&mut self, key: GlyphKey, f: impl FnOnce() -> T) -> &mut T {
        if !self.map.contains_key(&key) {
            self.misses += 1;
            let v = f();
            self.map.insert(key.clone(), v);
        } else {
            self.hits += 1;
        }
        self.map.get_mut(&key).expect("just inserted")
    }

    /// 全清（字体/主题/DPI 变化时）。
    pub fn clear(&mut self) {
        self.map.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats { hits: self.hits, misses: self.misses, len: self.map.len() }
    }
}

impl<T> Default for GlyphCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_hit() {
        let mut c = GlyphCache::new();
        let k = GlyphKey::new((255, 0, 0), (0, 0, 0), Attrs::empty(), "hi");
        c.get_or_insert_with(k.clone(), || 42u32);
        let v = *c.get_or_insert_with(k.clone(), || panic!("should hit"));
        assert_eq!(v, 42);
        let s = c.stats();
        assert_eq!((s.hits, s.misses, s.len), (1, 1, 1));
    }

    #[test]
    fn miss_creates_value() {
        let mut c = GlyphCache::new();
        let v = *c.get_or_insert_with(
            GlyphKey::new((1, 2, 3), (0, 0, 0), Attrs::BOLD, "x"),
            || 7u32,
        );
        assert_eq!(v, 7);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn clear_resets() {
        let mut c: GlyphCache<u32> = GlyphCache::new();
        c.get_or_insert_with(GlyphKey::new((0, 0, 0), (0, 0, 0), Attrs::empty(), "a"), || 1);
        c.clear();
        assert_eq!(c.len(), 0);
        assert_eq!(c.stats(), CacheStats::default());
    }

    #[test]
    fn key_distinguishes_style_and_text() {
        let a = GlyphKey::new((0, 0, 0), (255, 255, 255), Attrs::empty(), "x");
        let b = GlyphKey::new((0, 0, 0), (255, 255, 255), Attrs::BOLD, "x");
        let c = GlyphKey::new((1, 0, 0), (255, 255, 255), Attrs::empty(), "x");
        let d = GlyphKey::new((0, 0, 0), (255, 255, 255), Attrs::empty(), "y");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn key_from_colors_resolves_defaults() {
        let k = GlyphKey::from_colors(Color::Default, Color::Default, Attrs::empty(), "z");
        assert_eq!(k.fg, crate::color::DEFAULT_FG);
        assert_eq!(k.bg, crate::color::DEFAULT_BG);
        let k2 = GlyphKey::from_colors(Color::Indexed(9), Color::Rgb(1, 2, 3), Attrs::BOLD, "z");
        assert_eq!(k2.fg, (255, 0, 0));
        assert_eq!(k2.bg, (1, 2, 3));
        assert_eq!(k2.attrs, 1);
    }
}
