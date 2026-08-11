//! Continuous tiling of an alpha around the band.
//!
//! Tiles live on a lattice in unrolled `(u, v)` space. `repeats_around` is an
//! integer, so the lattice divides the circumference exactly and the pattern
//! closes on itself with no seam at 0°.

use serde::{Deserialize, Serialize};

use crate::alpha::AlphaLibrary;
use crate::field::{FieldContext, Uv};

/// One tile's footprint in unrolled mm space, for the layout preview.
#[derive(Clone, Copy, Debug)]
pub struct TileCell {
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
    pub rot_deg: f64,
    pub mirror_u: bool,
    pub mirror_v: bool,
    pub col: u32,
    pub row: u32,
}

impl TileCell {
    pub fn width(&self) -> f64 {
        self.u1 - self.u0
    }
    pub fn height(&self) -> f64 {
        self.v1 - self.v0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TilingLayer {
    /// Name of the alpha in the [`AlphaLibrary`].
    pub alpha: String,
    /// Tiles around the circumference. Integer, so the pattern is seamless.
    pub repeats_around: u32,
    /// Tile rows across the band.
    pub rows: u32,
    /// Centre of the tiled band across the cross-section, mm.
    pub v_center_mm: f64,
    /// Total `v` extent covered by the tiling, mm.
    pub v_span_mm: f64,
    /// Rotation of the alpha inside its cell, degrees.
    pub rotation_deg: f64,
    /// Lattice shift, in fractions of a cell.
    pub offset_u: f64,
    pub offset_v: f64,
    pub height_mm: f64,
    /// Flat gap left between neighbouring tiles, mm.
    pub gap_mm: f64,
    /// Brick-style offset applied per row, 0..1 of a cell.
    pub stagger: f64,
    pub mirror_alternate_u: bool,
    pub mirror_alternate_v: bool,
    /// Gamma on the alpha response. >1 deepens, <1 flattens.
    pub contrast: f64,
    /// Added to the alpha before shaping, -1..1.
    pub bias: f64,
    pub invert: bool,
    /// Fade the tiling out over this distance at the `v` edges of the band, mm.
    pub feather_mm: f64,
    /// Sample the alpha wrapped instead of clamped, so a seamless source keeps
    /// flowing across cell boundaries.
    pub continuous: bool,
    /// Repeat the band mirrored about the middle of the cross-section, so one
    /// layer covers both side faces.
    #[serde(default)]
    pub mirror_v: bool,
}

impl TilingLayer {
    /// A sensible starting tiling for a given band.
    pub fn default_for(alpha: impl Into<String>, ctx: &FieldContext) -> Self {
        Self {
            alpha: alpha.into(),
            repeats_around: 24,
            rows: 1,
            v_center_mm: ctx.crest_v_mm,
            v_span_mm: (ctx.band_v_len_mm * 0.6).max(0.5),
            rotation_deg: 0.0,
            offset_u: 0.0,
            offset_v: 0.0,
            height_mm: 0.35,
            gap_mm: 0.0,
            stagger: 0.0,
            mirror_alternate_u: false,
            mirror_alternate_v: false,
            contrast: 1.0,
            bias: 0.0,
            invert: false,
            feather_mm: 0.4,
            continuous: true,
            mirror_v: false,
        }
    }

    /// Tile count that makes each cell as wide around the ring as it is tall
    /// across the band, so the alpha is not stretched.
    pub fn repeats_for_square_cells(&self, ctx: &FieldContext) -> u32 {
        let rows = self.rows.max(1) as f64;
        let cell_h = self.v_span_mm / rows;
        if cell_h <= 1e-6 || !ctx.circumference_mm.is_finite() {
            return self.repeats_around.max(1);
        }
        ((ctx.circumference_mm / cell_h).round() as i64).clamp(1, 4096) as u32
    }

    /// Sit the tiling on the band's side faces — the surfaces square to the
    /// mould pull — with unstretched cells.
    ///
    /// Mirrors onto both faces only when they are the same size. A one-sided
    /// flange leaves a wide face on one edge and bare dome on the other, and
    /// mirroring onto that dome is where relief undercuts worst, so an uneven
    /// profile takes the wider face alone.
    ///
    /// Returns false and leaves the layer alone when the profile has no side
    /// face at all, which is the case for a plain dome.
    pub fn fit_to_side_faces(&mut self, ctx: &FieldContext, min_draft_deg: f64) -> bool {
        let Some(faces) = ctx.side_faces(min_draft_deg) else {
            return false;
        };
        let even = faces.is_even();
        // Mirroring reflects about the middle of the band, so an even pair takes
        // the overlap of the low face and the high one folded onto it. Both
        // copies then land on real face rather than on the dome beside it.
        let (v0, v1) = match (even, faces.low, faces.high) {
            (true, Some(low), Some(high)) => {
                let span = ctx.band_v_len_mm;
                (low.0.max(span - high.1), low.1.min(span - high.0))
            }
            _ => match faces.wider() {
                Some(f) => f,
                None => return false,
            },
        };
        let span = v1 - v0;
        if span <= 1e-6 {
            return false;
        }
        self.rows = 1;
        self.v_span_mm = span;
        self.v_center_mm = 0.5 * (v0 + v1);
        self.mirror_v = even;
        self.feather_mm = (span * 0.12).clamp(0.05, 0.4);
        self.repeats_around = self.repeats_for_square_cells(ctx);
        true
    }

    /// Cell size in unrolled mm: `(u extent, v extent)`.
    pub fn cell_size(&self, ctx: &FieldContext) -> (f64, f64) {
        let cols = self.repeats_around.max(1) as f64;
        let rows = self.rows.max(1) as f64;
        (ctx.circumference_mm / cols, self.v_span_mm / rows)
    }

    /// `v` bounds of the tiled band: `(low, high)`.
    pub fn v_bounds(&self) -> (f64, f64) {
        let half = self.v_span_mm * 0.5;
        (self.v_center_mm - half, self.v_center_mm + half)
    }

    /// Displacement at a surface point, in mm.
    pub fn height(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        let h = self.height_at(uv, ctx, lib);
        if !self.mirror_v {
            return h;
        }
        let across = Uv { u: uv.u, v: ctx.band_v_len_mm - uv.v };
        h.max(self.height_at(across, ctx, lib))
    }

    fn height_at(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        let Some(alpha) = lib.get(&self.alpha) else {
            return 0.0;
        };
        let (lo, hi) = self.v_bounds();
        if !uv.u.is_finite() || !lo.is_finite() || !hi.is_finite() || !(uv.v >= lo && uv.v <= hi) {
            return 0.0;
        }
        let circ = ctx.circumference_mm;
        let (cw, ch) = self.cell_size(ctx);
        // Comparisons against NaN are all false, so test for finiteness first.
        if !circ.is_finite() || !cw.is_finite() || !ch.is_finite() {
            return 0.0;
        }
        if circ <= 1e-9 || cw <= 1e-9 || ch <= 1e-9 {
            return 0.0;
        }

        // The row's stagger shifts the column lattice.
        let last_row = self.rows.max(1) as f64 - 1.0;
        let fv = (uv.v - lo) / ch - fin(self.offset_v);
        if !(0.0..=last_row + 1.0).contains(&fv) {
            return 0.0;
        }
        let row = fv.floor().min(last_row);
        let cols = self.repeats_around.max(1) as f64;
        let fu = uv.u.rem_euclid(circ) / cw - fin(self.offset_u) - fin(self.stagger) * row;
        let col = fu.floor().rem_euclid(cols);

        let mut lu = fu - fu.floor();
        let mut lv = fv - row;

        let gu = fin(self.gap_mm).max(0.0) * 0.5 / cw;
        let gv = fin(self.gap_mm).max(0.0) * 0.5 / ch;
        let (ku, kv) = (1.0 - 2.0 * gu, 1.0 - 2.0 * gv);
        if ku <= 1e-9 || kv <= 1e-9 {
            return 0.0;
        }
        if lu < gu || lu > 1.0 - gu || lv < gv || lv > 1.0 - gv {
            return 0.0;
        }
        lu = (lu - gu) / ku;
        lv = (lv - gv) / kv;

        let flip_u = self.mirror_alternate_u && (col as i64).rem_euclid(2) == 1;
        let flip_v = self.mirror_alternate_v && (row as i64).rem_euclid(2) == 1;
        let x = if flip_u { 1.0 - lu } else { lu } - 0.5;
        let y = if flip_v { 1.0 - lv } else { lv } - 0.5;
        let (sin, cos) = (-fin(self.rotation_deg).to_radians()).sin_cos();
        let sx = x * cos - y * sin + 0.5;
        let sy = x * sin + y * cos + 0.5;

        let raw = if self.continuous { alpha.sample_wrapped(sx, sy) } else { alpha.sample(sx, sy) };
        let mut h = alpha.shaped(raw, self.contrast, self.bias, self.invert) * self.height_mm;

        let feather = fin(self.feather_mm);
        if feather > 1e-9 {
            let d = (uv.v - lo).min(hi - uv.v);
            h *= (d / feather).clamp(0.0, 1.0);
        }
        if h.is_finite() { h } else { 0.0 }
    }

    /// Every cell's footprint, for the unrolled layout editor. Cells are laid
    /// out left to right then bottom to top, clipped in `v` to the tiled band
    /// so a footprint never covers metal [`TilingLayer::height`] does not lay
    /// down. At most [`MAX_CELLS`] are returned.
    pub fn cells(&self, ctx: &FieldContext) -> Vec<TileCell> {
        // Iteration is bounded independently of `cell_size`, which keeps using
        // the unclamped counts so footprints stay aligned with `height`.
        let limit = MAX_CELLS as u32;
        let cols = self.repeats_around.max(1).min(limit);
        let rows = self.rows.max(1).min(limit);
        let circ = ctx.circumference_mm;
        let (cw, ch) = self.cell_size(ctx);
        let (lo, hi) = self.v_bounds();
        let mut out = Vec::with_capacity((cols as usize).saturating_mul(rows as usize).min(1024));
        for row in 0..rows {
            // Clipped so a drawn footprint never covers metal `height` skips.
            let v0 = clip_v(lo + (row as f64 + fin(self.offset_v)) * ch, lo, hi);
            let v1 = clip_v(v0 + ch, lo, hi);
            if v1 - v0 <= 1e-9 {
                continue;
            }
            let shift = fin(self.offset_u) + fin(self.stagger) * row as f64;
            for col in 0..cols {
                if out.len() >= MAX_CELLS {
                    return out;
                }
                let u = (col as f64 + shift) * cw;
                let u0 = if circ > 1e-9 { u.rem_euclid(circ) } else { u };
                out.push(TileCell {
                    u0,
                    u1: u0 + cw,
                    v0,
                    v1,
                    rot_deg: self.rotation_deg,
                    mirror_u: self.mirror_alternate_u && col.rem_euclid(2) == 1,
                    mirror_v: self.mirror_alternate_v && row.rem_euclid(2) == 1,
                    col,
                    row,
                });
            }
        }
        if self.mirror_v {
            let span = ctx.band_v_len_mm;
            let room = MAX_CELLS.saturating_sub(out.len()).min(out.len());
            for i in 0..room {
                let c = out[i];
                out.push(TileCell { v0: span - c.v1, v1: span - c.v0, mirror_v: !c.mirror_v, ..c });
            }
        }
        out
    }
}

/// Upper bound on the cells [`TilingLayer::cells`] will build.
pub const MAX_CELLS: usize = 1 << 16;

/// Non-finite parameters fall back to 0.
#[inline]
fn fin(x: f64) -> f64 {
    if x.is_finite() { x } else { 0.0 }
}

/// `v` clipped into the band, collapsed to `lo` when the band is inverted.
#[inline]
fn clip_v(v: f64, lo: f64, hi: f64) -> f64 {
    if lo <= hi { v.max(lo).min(hi) } else { lo }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha::{Alpha, Procedural};

    fn ctx() -> FieldContext {
        FieldContext {
            circumference_mm: 60.0,
            band_v_len_mm: 8.0,
            crest_v_mm: 4.0,
            crest_radius_mm: 9.549_296_585_513_72,
            surface: Default::default(),
            side_faces_cache: Default::default(),
        }
    }

    /// Library holding one alpha that samples to exactly `x`, so a height reads
    /// back the cell-local `u`.
    fn ramp_lib() -> AlphaLibrary {
        let mut lib = AlphaLibrary::default();
        lib.insert(Alpha::new("ramp", 2, 2, vec![0.0, 1.0, 0.0, 1.0]));
        lib
    }

    fn ramp_layer() -> TilingLayer {
        TilingLayer {
            alpha: "ramp".into(),
            repeats_around: 12,
            rows: 2,
            v_center_mm: 4.0,
            v_span_mm: 4.0,
            rotation_deg: 0.0,
            offset_u: 0.0,
            offset_v: 0.0,
            height_mm: 1.0,
            gap_mm: 0.0,
            stagger: 0.0,
            mirror_alternate_u: false,
            mirror_alternate_v: false,
            contrast: 1.0,
            bias: 0.0,
            invert: false,
            feather_mm: 0.0,
            continuous: false,
            mirror_v: false,
        }
    }

    fn patterned_layer() -> TilingLayer {
        TilingLayer {
            repeats_around: 7,
            rows: 3,
            stagger: 0.5,
            offset_u: 0.31,
            offset_v: 0.0,
            mirror_alternate_u: true,
            mirror_alternate_v: true,
            rotation_deg: 23.0,
            gap_mm: 0.2,
            feather_mm: 0.3,
            ..TilingLayer::default_for("Rope", &ctx())
        }
    }

    fn lib() -> AlphaLibrary {
        let mut lib = AlphaLibrary::default();
        lib.insert(Procedural::Rope.generate(64));
        lib
    }

    #[test]
    fn cell_size_divides_the_circumference_and_the_band() {
        let c = ctx();
        let t = TilingLayer { repeats_around: 7, rows: 3, v_span_mm: 4.0, ..ramp_layer() };
        let (cw, ch) = t.cell_size(&c);
        assert!((cw * 7.0 - c.circumference_mm).abs() < 1e-12, "{cw}");
        assert!((ch * 3.0 - t.v_span_mm).abs() < 1e-12, "{ch}");

        // Degenerate counts fall back to one cell instead of dividing by zero.
        let t = TilingLayer { repeats_around: 0, rows: 0, ..t };
        let (cw, ch) = t.cell_size(&c);
        assert_eq!((cw, ch), (c.circumference_mm, t.v_span_mm));
    }

    #[test]
    fn the_pattern_closes_across_the_seam() {
        let c = ctx();
        let l = lib();
        let t = patterned_layer();
        for k in 0..9 {
            let v = 2.5 + k as f64 * 0.3;
            let a = t.height(Uv { u: 0.0, v }, &c, &l);
            let b = t.height(Uv { u: c.circumference_mm, v }, &c, &l);
            assert!((a - b).abs() < 1e-12, "seam at v={v}: {a} vs {b}");
        }
    }

    #[test]
    fn near_seam_samples_match_the_interior_one_cell_in() {
        let c = ctx();
        let l = lib();
        // Mirroring off, so every cell in a row carries the same phase.
        let t = TilingLayer { mirror_alternate_u: false, ..patterned_layer() };
        let (cw, _) = t.cell_size(&c);
        for d in [0.01, 0.2, 0.9, 1.7] {
            let v = 4.0;
            let before = t.height(Uv { u: c.circumference_mm - d, v }, &c, &l);
            let inside = t.height(Uv { u: c.circumference_mm - d + cw, v }, &c, &l);
            assert!((before - inside).abs() < 1e-9, "d={d}: {before} vs {inside}");
            let after = t.height(Uv { u: d, v }, &c, &l);
            let inside = t.height(Uv { u: d + cw, v }, &c, &l);
            assert!((after - inside).abs() < 1e-9, "d={d}: {after} vs {inside}");
        }
    }

    #[test]
    fn cells_tile_the_circumference_without_gaps_or_overlap() {
        let c = ctx();
        let t = TilingLayer { repeats_around: 12, rows: 3, ..ramp_layer() };
        let cells = t.cells(&c);
        assert_eq!(cells.len(), 12 * 3);
        let (cw, ch) = t.cell_size(&c);

        for row in 0..3u32 {
            let mut us: Vec<f64> = cells.iter().filter(|k| k.row == row).map(|k| k.u0).collect();
            assert_eq!(us.len(), 12);
            us.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert!(us[0].abs() < 1e-12, "row {row} does not start at 0: {}", us[0]);
            for w in us.windows(2) {
                assert!((w[1] - w[0] - cw).abs() < 1e-9, "gap or overlap: {w:?}");
            }
            assert!((us[11] + cw - c.circumference_mm).abs() < 1e-9);
        }
        for k in &cells {
            assert!((k.width() - cw).abs() < 1e-12);
            assert!((k.height() - ch).abs() < 1e-12);
            assert!(k.col < 12 && k.row < 3);
        }
        // Row 0 sits at the low edge of the band, the last row at the high edge.
        let (lo, hi) = t.v_bounds();
        assert!((cells[0].v0 - lo).abs() < 1e-12);
        assert!((cells[cells.len() - 1].v1 - hi).abs() < 1e-12);
    }

    #[test]
    fn every_cell_centre_carries_the_middle_of_the_alpha() {
        let c = ctx();
        let l = ramp_lib();
        let t = TilingLayer {
            repeats_around: 12,
            rows: 3,
            offset_u: 0.37,
            stagger: 0.5,
            mirror_alternate_u: true,
            ..ramp_layer()
        };
        for cell in t.cells(&c) {
            let uv = Uv { u: (cell.u0 + cell.u1) * 0.5, v: (cell.v0 + cell.v1) * 0.5 };
            let h = t.height(uv, &c, &l);
            assert!(
                (h - 0.5 * t.height_mm).abs() < 1e-9,
                "cell {},{} centre reads {h}",
                cell.col,
                cell.row
            );
        }
    }

    #[test]
    fn mirroring_flips_odd_columns() {
        let c = ctx();
        let l = ramp_lib();
        let t = TilingLayer { mirror_alternate_u: true, ..ramp_layer() };
        let (cw, _) = t.cell_size(&c);
        let v = 4.0;
        let even = t.height(Uv { u: 0.25 * cw, v }, &c, &l);
        let odd = t.height(Uv { u: cw + 0.25 * cw, v }, &c, &l);
        assert!((even - 0.25).abs() < 1e-9, "{even}");
        assert!((odd - 0.75).abs() < 1e-9, "{odd}");

        // Without the flip both columns read the same phase.
        let t = TilingLayer { mirror_alternate_u: false, ..t };
        let odd = t.height(Uv { u: cw + 0.25 * cw, v }, &c, &l);
        assert!((odd - 0.25).abs() < 1e-9, "{odd}");
    }

    #[test]
    fn stagger_shifts_a_row_by_a_fraction_of_a_cell() {
        let c = ctx();
        let l = ramp_lib();
        let t = TilingLayer { rows: 2, stagger: 0.5, ..ramp_layer() };
        let (cw, ch) = t.cell_size(&c);
        let (lo, _) = t.v_bounds();
        let row0 = t.height(Uv { u: 0.25 * cw, v: lo + 0.5 * ch }, &c, &l);
        let row1 = t.height(Uv { u: 0.75 * cw, v: lo + 1.5 * ch }, &c, &l);
        assert!((row0 - 0.25).abs() < 1e-9, "{row0}");
        assert!((row1 - 0.25).abs() < 1e-9, "{row1}");
    }

    #[test]
    fn gap_leaves_flat_metal_between_tiles() {
        let c = ctx();
        let l = ramp_lib();
        // 5.0 x 2.0 mm cells, inset by 0.5 mm on every side.
        let t = TilingLayer { gap_mm: 1.0, ..ramp_layer() };
        let (cw, ch) = t.cell_size(&c);
        let (lo, _) = t.v_bounds();
        let v = lo + 0.5 * ch;
        assert_eq!(t.height(Uv { u: 0.05 * cw, v }, &c, &l), 0.0);
        assert_eq!(t.height(Uv { u: 0.95 * cw, v }, &c, &l), 0.0);
        assert_eq!(t.height(Uv { u: 0.5 * cw, v: lo + 0.05 * ch }, &c, &l), 0.0);
        // What is left of the cell is rescaled over 0..1.
        let mid = t.height(Uv { u: 0.5 * cw, v }, &c, &l);
        assert!((mid - 0.5).abs() < 1e-6, "{mid}");
        let inset = t.height(Uv { u: 0.1 * cw, v }, &c, &l);
        assert!(inset.abs() < 1e-6, "{inset}");

        // A gap wider than the cell leaves nothing to tile.
        let t = TilingLayer { gap_mm: ch * 2.0, ..t };
        assert_eq!(t.height(Uv { u: 0.5 * cw, v }, &c, &l), 0.0);
    }

    #[test]
    fn out_of_band_v_contributes_nothing() {
        let c = ctx();
        let l = lib();
        let t = patterned_layer();
        let (lo, hi) = t.v_bounds();
        for v in [lo - 0.01, lo - 3.0, hi + 0.01, hi + 3.0, f64::NAN] {
            assert_eq!(t.height(Uv { u: 7.3, v }, &c, &l), 0.0, "v={v}");
        }
        assert_eq!(t.height(Uv { u: f64::NAN, v: 4.0 }, &c, &l), 0.0);
    }

    #[test]
    fn a_missing_alpha_contributes_nothing() {
        let c = ctx();
        let l = lib();
        let t = TilingLayer { alpha: "not in the library".into(), ..patterned_layer() };
        assert_eq!(t.height(Uv { u: 3.0, v: 4.0 }, &c, &l), 0.0);
        assert_eq!(t.height(Uv { u: 0.0, v: t.v_center_mm }, &c, &l), 0.0);
    }

    #[test]
    fn feather_fades_the_band_edges_to_zero() {
        let c = ctx();
        let l = ramp_lib();
        let t = TilingLayer { feather_mm: 0.5, ..ramp_layer() };
        let (cw, _) = t.cell_size(&c);
        let (lo, hi) = t.v_bounds();
        let u = 0.9 * cw;
        assert_eq!(t.height(Uv { u, v: lo }, &c, &l), 0.0);
        assert_eq!(t.height(Uv { u, v: hi }, &c, &l), 0.0);
        let quarter = t.height(Uv { u, v: lo + 0.125 }, &c, &l);
        let half = t.height(Uv { u, v: lo + 0.25 }, &c, &l);
        let full = t.height(Uv { u, v: (lo + hi) * 0.5 }, &c, &l);
        assert!((half - 0.5 * full).abs() < 1e-9, "{half} vs {full}");
        assert!((quarter - 0.25 * full).abs() < 1e-9, "{quarter} vs {full}");
        assert!((full - 0.9).abs() < 1e-6, "{full}");
    }

    #[test]
    fn a_full_turn_of_rotation_matches_none() {
        let c = ctx();
        let l = lib();
        let a = TilingLayer { rotation_deg: 0.0, ..patterned_layer() };
        let b = TilingLayer { rotation_deg: 360.0, ..patterned_layer() };
        let mut nonzero = 0;
        for i in 0..40 {
            let uv = Uv { u: i as f64 * 1.37, v: 2.4 + (i % 7) as f64 * 0.4 };
            let (ha, hb) = (a.height(uv, &c, &l), b.height(uv, &c, &l));
            assert!((ha - hb).abs() < 1e-9, "at {uv:?}: {ha} vs {hb}");
            if ha.abs() > 1e-9 {
                nonzero += 1;
            }
        }
        assert!(nonzero > 10, "only {nonzero} samples carried relief");
    }

    #[test]
    fn height_stays_finite_and_within_the_layer_height() {
        let c = ctx();
        let l = lib();
        let t = TilingLayer { contrast: f64::NAN, bias: f64::NAN, ..patterned_layer() };
        for i in 0..50 {
            let uv = Uv { u: i as f64 * 2.9 - 30.0, v: i as f64 * 0.2 };
            let h = t.height(uv, &c, &l);
            assert!(h.is_finite(), "non-finite height at {uv:?}");
        }
        let t = patterned_layer();
        for i in 0..200 {
            let uv = Uv { u: i as f64 * 0.71, v: 2.2 + (i % 11) as f64 * 0.16 };
            let h = t.height(uv, &c, &l);
            assert!((0.0..=t.height_mm).contains(&h), "{h} outside 0..{}", t.height_mm);
        }
    }

    #[test]
    fn a_degenerate_context_contributes_nothing() {
        let l = lib();
        let t = patterned_layer();
        let c = FieldContext { circumference_mm: 0.0, ..ctx() };
        assert_eq!(t.height(Uv { u: 0.0, v: 4.0 }, &c, &l), 0.0);
        assert_eq!(t.cells(&c).len(), (t.repeats_around * t.rows) as usize);

        let t = TilingLayer { v_span_mm: 0.0, ..t };
        assert_eq!(t.height(Uv { u: 0.0, v: t.v_center_mm }, &ctx(), &l), 0.0);
    }

    /// Library holding one alpha that is 1 everywhere, so any point the field
    /// tiles reads back non-zero.
    fn solid_lib() -> AlphaLibrary {
        let mut lib = AlphaLibrary::default();
        lib.insert(Alpha::new("solid", 4, 4, vec![1.0; 16]));
        lib
    }

    fn solid_layer() -> TilingLayer {
        TilingLayer {
            alpha: "solid".into(),
            repeats_around: 7,
            rows: 3,
            v_span_mm: 4.5,
            rotation_deg: 37.0,
            offset_u: 0.31,
            stagger: 0.37,
            mirror_alternate_u: true,
            mirror_alternate_v: true,
            ..ramp_layer()
        }
    }

    #[test]
    fn the_pattern_closes_across_the_seam_under_every_lattice() {
        let c = ctx();
        let l = lib();
        for ou in [0.0, 0.31, 0.87] {
            for stagger in [0.0, 0.37, 0.5] {
                for rows in [1u32, 2, 5] {
                    for rot in [0.0, 23.0, -47.0] {
                        for gap in [0.0, 0.2] {
                            for ov in [0.0, 0.4] {
                                let t = TilingLayer {
                                    offset_u: ou,
                                    stagger,
                                    rows,
                                    rotation_deg: rot,
                                    gap_mm: gap,
                                    offset_v: ov,
                                    mirror_alternate_u: true,
                                    ..patterned_layer()
                                };
                                for k in 0..25 {
                                    let v = 2.0 + k as f64 * 0.16;
                                    let a = t.height(Uv { u: 0.0, v }, &c, &l);
                                    let b = t.height(Uv { u: c.circumference_mm, v }, &c, &l);
                                    assert!(
                                        (a - b).abs() < 1e-12,
                                        "seam at v={v}, ou={ou} stagger={stagger} rows={rows} \
                                         rot={rot} gap={gap} ov={ov}: {a} vs {b}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_cell_footprint_carries_metal() {
        let c = ctx();
        let l = solid_lib();
        for ov in [0.0, 0.4, -0.4, 0.9] {
            for ou in [0.0, 0.31] {
                for stagger in [0.0, 0.37] {
                    let t = TilingLayer { offset_v: ov, offset_u: ou, stagger, ..solid_layer() };
                    let (lo, hi) = t.v_bounds();
                    for cell in t.cells(&c) {
                        assert!(cell.v0 >= lo - 1e-12 && cell.v1 <= hi + 1e-12,
                            "cell {},{} spans {}..{} outside the band {lo}..{hi}",
                            cell.col, cell.row, cell.v0, cell.v1);
                        if cell.height() <= 1e-9 {
                            continue;
                        }
                        for a in [0.07, 0.5, 0.93] {
                            for b in [0.07, 0.5, 0.93] {
                                let uv = Uv {
                                    u: cell.u0 + a * cell.width(),
                                    v: cell.v0 + b * cell.height(),
                                };
                                assert!(
                                    t.height(uv, &c, &l) > 0.0,
                                    "ov={ov} ou={ou} stagger={stagger}: cell {},{} is bare at \
                                     ({a},{b}) -> {uv:?}",
                                    cell.col,
                                    cell.row
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_cell_reads_the_phase_the_preview_advertises() {
        let c = ctx();
        let mut across = AlphaLibrary::default();
        across.insert(Alpha::new("vramp", 2, 2, vec![0.0, 0.0, 1.0, 1.0]));
        let along = ramp_lib();
        for ou in [0.0, 0.31, 0.75] {
            for stagger in [0.0, 0.37, 0.5] {
                let t = TilingLayer {
                    offset_u: ou,
                    stagger,
                    rotation_deg: 0.0,
                    mirror_alternate_u: true,
                    mirror_alternate_v: true,
                    ..solid_layer()
                };
                let t_u = TilingLayer { alpha: "ramp".into(), ..t.clone() };
                let t_v = TilingLayer { alpha: "vramp".into(), ..t };
                for cell in t_u.cells(&c) {
                    for a in [0.13, 0.61, 0.88] {
                        let v = (cell.v0 + cell.v1) * 0.5;
                        let u = cell.u0 + a * cell.width();
                        let h = t_u.height(Uv { u, v }, &c, &along);
                        let want = if cell.mirror_u { 1.0 - a } else { a };
                        assert!((h - want).abs() < 1e-6,
                            "u phase in cell {},{} at {a}: {h} vs {want}", cell.col, cell.row);

                        let mid_u = (cell.u0 + cell.u1) * 0.5;
                        let v = cell.v0 + a * cell.height();
                        let h = t_v.height(Uv { u: mid_u, v }, &c, &across);
                        let want = if cell.mirror_v { 1.0 - a } else { a };
                        assert!((h - want).abs() < 1e-6,
                            "v phase in cell {},{} at {a}: {h} vs {want}", cell.col, cell.row);
                    }
                }
            }
        }
    }

    #[test]
    fn a_gap_empties_the_rim_of_every_cell() {
        let c = ctx();
        let l = solid_lib();
        let t = TilingLayer { gap_mm: 0.6, ..solid_layer() };
        let (cw, ch) = t.cell_size(&c);
        let (gu, gv) = (0.3 / cw, 0.3 / ch);
        for cell in t.cells(&c) {
            for (a, b) in [(gu * 0.5, 0.5), (1.0 - gu * 0.5, 0.5), (0.5, gv * 0.5)] {
                let uv = Uv { u: cell.u0 + a * cw, v: cell.v0 + b * ch };
                assert_eq!(t.height(uv, &c, &l), 0.0, "cell {},{} rim", cell.col, cell.row);
            }
            let uv = Uv { u: cell.u0 + 0.5 * cw, v: cell.v0 + 0.5 * ch };
            assert!(t.height(uv, &c, &l) > 0.0, "cell {},{} centre", cell.col, cell.row);
        }
    }

    #[test]
    fn a_non_finite_context_contributes_nothing() {
        let l = solid_lib();
        let t = solid_layer();
        for circ in [f64::NAN, f64::INFINITY, -60.0] {
            let c = FieldContext { circumference_mm: circ, ..ctx() };
            for u in [0.0, 3.0, -7.0, 1e18] {
                assert_eq!(t.height(Uv { u, v: 4.0 }, &c, &l), 0.0, "circ={circ} u={u}");
            }
        }
        let c = FieldContext { band_v_len_mm: f64::NAN, ..ctx() };
        let t = TilingLayer { v_span_mm: f64::INFINITY, ..solid_layer() };
        assert_eq!(t.height(Uv { u: 3.0, v: 4.0 }, &c, &l), 0.0);
    }

    #[test]
    fn an_inverted_band_lays_no_metal_and_no_cells() {
        let c = ctx();
        let l = solid_lib();
        let t = TilingLayer { v_span_mm: -4.0, ..solid_layer() };
        for v in [0.0, 2.0, 4.0, 6.0, 8.0] {
            assert_eq!(t.height(Uv { u: 3.0, v }, &c, &l), 0.0, "v={v}");
        }
        for cell in t.cells(&c) {
            assert_eq!(cell.height(), 0.0, "cell {},{} spans {}..{}", cell.col, cell.row, cell.v0,
                cell.v1);
        }
    }

    /// A design whose cross-section carries real geometry, unlike `ctx()`.
    fn design(style: crate::ProfileStyle) -> crate::RingDesign {
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(style);
        d
    }

    #[test]
    fn mirror_v_is_symmetric_about_the_middle_of_the_band() {
        let c = ctx();
        let l = ramp_lib();
        let t = TilingLayer { mirror_v: true, v_center_mm: 1.0, v_span_mm: 2.0, ..ramp_layer() };
        for v in [0.2, 0.7, 1.4, 1.9] {
            let lo = t.height(Uv { u: 7.0, v }, &c, &l);
            let hi = t.height(Uv { u: 7.0, v: c.band_v_len_mm - v }, &c, &l);
            assert!((lo - hi).abs() < 1e-12, "v={v}: {lo} vs {hi}");
        }
        // Without it the far side is bare.
        let t = TilingLayer { mirror_v: false, ..t };
        assert_eq!(t.height(Uv { u: 7.0, v: c.band_v_len_mm - 0.7 }, &c, &l), 0.0);
    }

    #[test]
    fn square_cells_are_as_wide_as_they_are_tall() {
        let c = ctx();
        let mut t = TilingLayer { v_span_mm: 1.5, rows: 1, ..ramp_layer() };
        t.repeats_around = t.repeats_for_square_cells(&c);
        let (w, h) = t.cell_size(&c);
        assert!((w / h - 1.0).abs() < 0.02, "cells are {w:.3} x {h:.3}");
    }

    /// Every point a fitted layer covers must actually be a side face.
    fn assert_on_face(t: &TilingLayer, c: &FieldContext) {
        let (v0, v1) = t.v_bounds();
        for k in 0..=20 {
            let v = v0 + (v1 - v0) * k as f64 / 20.0;
            for w in [v, if t.mirror_v { c.band_v_len_mm - v } else { v }] {
                let deg = c.surface.draft_deg(w, c.band_v_len_mm).unwrap_or(0.0);
                assert!(
                    deg >= crate::field::SIDE_FACE_MIN_DRAFT_DEG,
                    "v {w:.2} drafts only {deg:.1} deg"
                );
            }
        }
    }

    #[test]
    fn a_dome_has_no_side_face_to_fit_to() {
        let c = design(crate::ProfileStyle::HalfRound).field_context();
        let min = crate::field::SIDE_FACE_MIN_DRAFT_DEG;
        assert!(c.side_faces(min).is_none(), "a dome rolls straight past square to the pull");
        let mut t = TilingLayer::default_for("Rope", &c);
        let before = t.clone();
        assert!(!t.fit_to_side_faces(&c, min));
        assert_eq!(t.v_span_mm, before.v_span_mm, "a failed fit must not edit the layer");
        assert_eq!(t.repeats_around, before.repeats_around);
    }

    #[test]
    fn squaring_the_sides_gives_a_flat_band_two_faces_to_fit_to() {
        let mut d = design(crate::ProfileStyle::Flat);
        d.profile.flatten_sides();
        let c = d.field_context();
        let f = c
            .side_faces(crate::field::SIDE_FACE_MIN_DRAFT_DEG)
            .expect("squared sides should expose a face");
        assert!(f.low_width() > 0.5, "low face is only {:.2} mm", f.low_width());
        assert!(f.is_even(), "a symmetric profile should give even faces");

        let mut t = TilingLayer::default_for("Rope", &c);
        assert!(t.fit_to_side_faces(&c, crate::field::SIDE_FACE_MIN_DRAFT_DEG));
        assert!(t.mirror_v, "even faces should be mirrored onto both sides");
        assert_on_face(&t, &c);
    }

    #[test]
    fn a_one_sided_flange_is_not_mirrored_onto_the_bare_dome() {
        let mut d = design(crate::ProfileStyle::HalfRound);
        d.profile.flange = crate::profile::Flange {
            enabled: true,
            v_pos: 0.0,
            extent_mm: 1.2,
            thickness_mm: 0.9,
            edge_round_mm: 0.15,
        };
        let c = d.field_context();
        let mut t = TilingLayer::default_for("Rope", &c);
        assert!(t.fit_to_side_faces(&c, crate::field::SIDE_FACE_MIN_DRAFT_DEG));
        assert!(!t.mirror_v, "mirroring a one-sided flange puts relief on the dome flank");
        assert_on_face(&t, &c);
    }

    #[test]
    fn a_hostile_cell_count_stays_bounded() {
        let c = ctx();
        let t = TilingLayer { repeats_around: u32::MAX, rows: u32::MAX, ..ramp_layer() };
        assert!(t.cells(&c).len() <= MAX_CELLS);
        let t = TilingLayer { repeats_around: 1, rows: u32::MAX, ..ramp_layer() };
        assert!(t.cells(&c).len() <= MAX_CELLS);
    }
}
