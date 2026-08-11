//! Text rendered to an alpha — names, dates, monograms.
//!
//! The text travels in the design as a string and a font choice, like
//! [`crate::drawn`] travels as strokes: the raster is derived on load at
//! whatever resolution is wanted, so the inscription survives moving machines
//! and re-renders clean at export. Both bundled fonts are SIL OFL
//! (`assets/fonts/OFL.txt`).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::alpha::Alpha;

/// Longest text worth rasterizing; the library caps entry sizes anyway.
pub const MAX_TEXT_CHARS: usize = 64;

/// Raster height of the text row, px. Width follows the layout.
const RASTER_EM_PX: f32 = 160.0;

/// Padding around the rendered text, as a share of the em.
const PAD_FRAC: f32 = 0.14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFont {
    /// EB Garamond — an engraver's serif.
    Serif,
    /// Great Vibes — a flowing script.
    Script,
}

impl TextFont {
    pub const ALL: &'static [TextFont] = &[TextFont::Serif, TextFont::Script];

    pub fn label(self) -> &'static str {
        match self {
            TextFont::Serif => "Serif (EB Garamond)",
            TextFont::Script => "Script (Great Vibes)",
        }
    }

    fn font(self) -> &'static fontdue::Font {
        static SERIF: OnceLock<fontdue::Font> = OnceLock::new();
        static SCRIPT: OnceLock<fontdue::Font> = OnceLock::new();
        let (cell, bytes): (&OnceLock<fontdue::Font>, &[u8]) = match self {
            TextFont::Serif => {
                (&SERIF, include_bytes!("../../../assets/fonts/EBGaramond.ttf").as_slice())
            }
            TextFont::Script => (
                &SCRIPT,
                include_bytes!("../../../assets/fonts/GreatVibes-Regular.ttf").as_slice(),
            ),
        };
        cell.get_or_init(|| {
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
                .expect("bundled font parses")
        })
    }
}

/// One inscription carried by the design and rasterized into the library.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextAlpha {
    /// Library name the raster lands under.
    pub name: String,
    pub text: String,
    pub font: TextFont,
    /// Extra letter spacing as a share of the em.
    pub tracking: f64,
}

impl Default for TextAlpha {
    fn default() -> Self {
        Self {
            name: "Text".into(),
            text: "Amor vincit".into(),
            font: TextFont::Script,
            tracking: 0.0,
        }
    }
}

impl TextAlpha {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Render the text to a coverage alpha. Returns an empty alpha for empty
    /// or unrenderable text rather than failing.
    pub fn rasterize(&self) -> Alpha {
        let text: String = self.text.chars().take(MAX_TEXT_CHARS).collect();
        if text.trim().is_empty() {
            return Alpha::new(self.name.clone(), 0, 0, Vec::new());
        }
        let font = self.font.font();
        let px = RASTER_EM_PX;
        let tracking = (self.tracking.clamp(-0.2, 1.0) as f32) * px;

        // Lay glyphs on a shared baseline by hand: advance plus tracking,
        // bitmaps hung from the baseline by their own metrics.
        struct Placed {
            x: f32,
            metrics: fontdue::Metrics,
            coverage: Vec<u8>,
        }
        let mut placed: Vec<Placed> = Vec::new();
        let mut pen = 0.0f32;
        let mut prev: Option<char> = None;
        for ch in text.chars() {
            if let Some(p) = prev
                && let Some(kern) = font.horizontal_kern(p, ch, px)
            {
                pen += kern;
            }
            let (metrics, coverage) = font.rasterize(ch, px);
            placed.push(Placed { x: pen, metrics, coverage });
            pen += metrics.advance_width + tracking;
            prev = Some(ch);
        }

        // Tight vertical bounds from the glyphs actually present.
        let mut top = f32::MIN;
        let mut bottom = f32::MAX;
        for g in &placed {
            top = top.max(g.metrics.ymin as f32 + g.metrics.height as f32);
            bottom = bottom.min(g.metrics.ymin as f32);
        }
        if placed.is_empty() || top <= bottom {
            return Alpha::new(self.name.clone(), 0, 0, Vec::new());
        }

        let pad = (px * PAD_FRAC).round() as usize;
        let w = (pen - tracking).ceil().max(1.0) as usize + 2 * pad;
        let h = (top - bottom).ceil().max(1.0) as usize + 2 * pad;
        let mut data = vec![0.0f32; w * h];
        for g in &placed {
            let gx = g.x.round() as isize + pad as isize;
            // Row 0 of the buffer is the text's top; a glyph hangs from the
            // baseline by ymin + height.
            let gy = (top - (g.metrics.ymin as f32 + g.metrics.height as f32)).round() as isize
                + pad as isize;
            for row in 0..g.metrics.height {
                for col in 0..g.metrics.width {
                    let x = gx + col as isize;
                    let y = gy + row as isize;
                    if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                        continue;
                    }
                    let v = g.coverage[row * g.metrics.width + col] as f32 / 255.0;
                    let dst = &mut data[y as usize * w + x as usize];
                    *dst = dst.max(v);
                }
            }
        }
        Alpha::new(self.name.clone(), w, h, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rasterizes_with_ink_and_travels_by_content() {
        let t = TextAlpha { name: "test".into(), text: "Ava".into(), ..Default::default() };
        let a = t.rasterize();
        assert!(a.width > a.height, "a short word is wider than tall");
        let ink: f32 = a.data.iter().sum();
        assert!(ink > 100.0, "the raster carries ink: {ink}");
        let peak = a.data.iter().cloned().fold(0.0f32, f32::max);
        assert!(peak > 0.9, "solid strokes reach full coverage: {peak}");

        // Same content, same raster — the derived alpha is deterministic.
        let b = t.rasterize();
        assert_eq!(a.data, b.data);
    }

    #[test]
    fn both_fonts_render_and_empty_text_stays_empty() {
        for font in TextFont::ALL {
            let t = TextAlpha {
                name: "x".into(),
                text: "1888".into(),
                font: *font,
                tracking: 0.1,
            };
            let a = t.rasterize();
            assert!(!a.is_empty(), "{font:?} rendered nothing");
        }
        let empty = TextAlpha { text: "   ".into(), ..Default::default() };
        assert!(empty.rasterize().is_empty());
        let hostile = TextAlpha { text: "\u{0}\u{7}".repeat(200), ..Default::default() };
        let _ = hostile.rasterize();
    }
}
