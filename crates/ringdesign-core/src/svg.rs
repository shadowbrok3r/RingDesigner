//! SVG import: vector art carried in the design, rasterized on load.
//!
//! The SVG text is the source of truth and travels in the `.ring.json` the
//! way strokes and inscriptions do, so the motif survives moving machines and
//! re-rasterizes at full quality instead of shipping one frozen raster.
//! Height is ink coverage — a black path on a blank ground raises metal where
//! the ink is — and `invert` flips documents drawn the other way round.
//! Text elements inside the SVG are not rendered (no font database is
//! bundled); inscriptions are what [`crate::text`] is for.

use serde::{Deserialize, Serialize};

use crate::alpha::Alpha;

/// Longest raster edge, px. Matches the alpha editor's working resolution;
/// the vector source means nothing is lost until far past casting detail.
const RASTER_EDGE_PX: u32 = 1024;

/// Guard against absurd documents before Pixmap allocation.
const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;

/// One imported SVG carried by the design and rasterized into the library.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SvgAlpha {
    /// Library name the raster lands under.
    pub name: String,
    /// The SVG document itself.
    pub svg: String,
    /// Raise the ground instead of the ink.
    #[serde(default)]
    pub invert: bool,
}

impl SvgAlpha {
    pub fn is_empty(&self) -> bool {
        self.svg.trim().is_empty()
    }

    /// Render to a coverage alpha. Returns an empty alpha rather than failing
    /// on an unparsable document, so a bad import cannot poison a load.
    pub fn rasterize(&self) -> Alpha {
        let empty = || Alpha::new(self.name.clone(), 0, 0, Vec::new());
        if self.is_empty() || self.svg.len() > MAX_SVG_BYTES {
            return empty();
        }
        let opt = resvg::usvg::Options::default();
        let Ok(tree) = resvg::usvg::Tree::from_str(&self.svg, &opt) else {
            return empty();
        };
        let size = tree.size();
        if !(size.width() > 0.0 && size.height() > 0.0) {
            return empty();
        }
        let scale = f64::from(RASTER_EDGE_PX) / f64::from(size.width().max(size.height()));
        let w = ((f64::from(size.width()) * scale).round() as u32).max(1);
        let h = ((f64::from(size.height()) * scale).round() as u32).max(1);
        let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(w, h) else {
            return empty();
        };
        let transform = resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // Ink coverage: opacity weighted by darkness, so black paths read
        // full height whether the ground is transparent or painted white.
        let px = pixmap.data();
        let mut data = Vec::with_capacity((w * h) as usize);
        for p in px.chunks_exact(4) {
            let a = f32::from(p[3]) / 255.0;
            // Premultiplied RGB: un-multiply before reading brightness.
            let luma = if a > 0.0 {
                (0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]))
                    / (255.0 * a)
            } else {
                0.0
            };
            let ink = a * (1.0 - luma.clamp(0.0, 1.0));
            data.push(if self.invert { a - ink } else { ink });
        }
        Alpha::new(self.name.clone(), w as usize, h as usize, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 90">
        <path d="M50 82 C 20 60, 4 40, 4 25 C 4 10, 16 4, 27 4 C 37 4, 46 10, 50 19
                 C 54 10, 63 4, 73 4 C 84 4, 96 10, 96 25 C 96 40, 80 60, 50 82 Z"/>
    </svg>"##;

    #[test]
    fn a_path_rasterizes_as_ink_and_invert_flips_it() {
        let s = SvgAlpha { name: "heart".into(), svg: HEART.into(), invert: false };
        let a = s.rasterize();
        assert_eq!(a.width, 1024, "longest edge lands on the working resolution");
        assert!(a.height > 800 && a.height < 1024);
        let at = |fx: f64, fy: f64| {
            a.data[(a.height as f64 * fy) as usize * a.width + (a.width as f64 * fx) as usize]
        };
        // Centre of the heart is ink; the top notch and corners are ground.
        assert!(at(0.5, 0.4) > 0.9, "centre {}", at(0.5, 0.4));
        assert!(at(0.02, 0.95) < 0.05, "corner {}", at(0.02, 0.95));
        assert!(at(0.5, 0.02) < 0.05, "notch {}", at(0.5, 0.02));

        let inv = SvgAlpha { invert: true, ..s.clone() };
        let b = inv.rasterize();
        let bat = |fx: f64, fy: f64| {
            b.data[(b.height as f64 * fy) as usize * b.width + (b.width as f64 * fx) as usize]
        };
        assert!(bat(0.5, 0.4) < 0.1);

        // Garbage does not fail, it vanishes.
        let bad = SvgAlpha { name: "x".into(), svg: "<not svg".into(), invert: false };
        assert!(bad.rasterize().is_empty());
    }
}
