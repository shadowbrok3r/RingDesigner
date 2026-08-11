//! The band brush: pressure to millimetres of metal, bounded by the surface.
//!
//! Everywhere else pressure means opacity; here it means *depth*, and the
//! ceiling is not a preference — it comes from the geometry. The band's local
//! draft angle says how much relief a spot can carry and still pull out of
//! sand: a squared side face is measured clean to **1.6 mm**, the crest of a
//! half-round undercuts at 0.30 mm and is honest only to about **0.05 mm**.
//! The same hard press gives a deep cut on the flank and almost nothing on
//! the crown, and the stroke says so while it is drawn.
//!
//! This module is shared by the desktop's unrolled editor and the Android
//! app's pen canvas — one behavior, and one file convention
//! ([`ensure_band_layer`]), so a band painted on either device opens on the
//! other as the same ordinary layers.

use crate::drawn::DrawnAlpha;
use crate::field::{FieldContext, Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG};
use crate::tiling::TilingLayer;
use crate::RingDesign;

/// Deepest relief allowed anywhere, mm. The measured side-face figure.
pub const MAX_RELIEF_MM: f64 = 1.6;
/// Shallowest mark worth making, mm. Below `MIN_EDGE_MM` (0.2) a feather
/// edge will not fill, so a lighter touch than this is not a fainter mark —
/// it is no mark at all.
pub const MIN_RELIEF_MM: f64 = 0.2;
/// Draft at or above this is a side face and takes the full depth.
const FREE_DRAFT_DEG: f64 = SIDE_FACE_MIN_DRAFT_DEG;
/// Below this the surface is effectively crown and takes almost nothing.
const CREST_DRAFT_DEG: f64 = 20.0;
/// What the crest can hold, mm.
const CREST_RELIEF_MM: f64 = 0.05;

/// Name of the band-wide painted drawing and its layer, on every device.
pub const BAND_ALPHA: &str = "band";
/// Band raster: 2048 px across a ~67 mm circumference is 0.033 mm per pixel —
/// well under what the mesh resolves. 512 would be 0.13 mm, the mesh's own
/// step, and pen detail would quantize away.
pub const BAND_W: u32 = 2048;
pub const BAND_H: u32 = 320;

/// What the surface at a given `v` will take.
///
/// Interpolates between the crest figure and the side-face figure over the
/// draft angles between them, so there is no cliff in the middle of the band
/// for the pen to fall off.
pub fn ceiling_mm(ctx: &FieldContext, v_mm: f64) -> f64 {
    let Some(draft) = ctx.surface.draft_deg(v_mm, ctx.band_v_len_mm) else {
        return CREST_RELIEF_MM;
    };
    if draft >= FREE_DRAFT_DEG {
        MAX_RELIEF_MM
    } else if draft <= CREST_DRAFT_DEG {
        CREST_RELIEF_MM
    } else {
        let t = (draft - CREST_DRAFT_DEG) / (FREE_DRAFT_DEG - CREST_DRAFT_DEG);
        // Smoothstep, so the transition has no slope step to read as a ridge.
        let t = t * t * (3.0 - 2.0 * t);
        CREST_RELIEF_MM + (MAX_RELIEF_MM - CREST_RELIEF_MM) * t
    }
}

/// The mark a press of `pressure` wants to make at `depth_scale`, before the
/// surface has its say.
///
/// Floored at [`MIN_RELIEF_MM`] rather than at a fraction of the maximum: the
/// usual `0.35 + 0.65 * p` curve bottoms out at a *proportion*, which against
/// a 0.35 mm layer is 0.12 mm — under the minimum edge the metal can hold.
pub fn wanted_mm(pressure: f32, depth_scale: f64) -> f64 {
    let p = pressure.clamp(0.0, 1.0) as f64;
    let top = (MAX_RELIEF_MM * depth_scale.clamp(0.05, 1.0)).max(MIN_RELIEF_MM);
    MIN_RELIEF_MM + (top - MIN_RELIEF_MM) * p
}

/// A brush sample resolved against the surface under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bite {
    /// What will actually be cut, mm.
    pub depth_mm: f64,
    /// What the pressure asked for, mm.
    pub wanted_mm: f64,
    /// What the surface allows here, mm.
    pub ceiling_mm: f64,
}

impl Bite {
    /// The press asked for more than the surface can hold. Worth showing: it
    /// is the moment the geometry pushes back, and the answer is usually to
    /// move to a flank or widen the side faces, not to press more gently.
    pub fn clamped(&self) -> bool {
        self.wanted_mm > self.ceiling_mm + 1e-9
    }

    /// Depth as a fraction of the global maximum — what the alpha stores.
    pub fn alpha_value(&self) -> f32 {
        (self.depth_mm / MAX_RELIEF_MM).clamp(0.0, 1.0) as f32
    }
}

/// Resolve a press at `v_mm` across the band.
pub fn bite(ctx: &FieldContext, v_mm: f64, pressure: f32, depth_scale: f64) -> Bite {
    let ceiling = ceiling_mm(ctx, v_mm);
    let wanted = wanted_mm(pressure, depth_scale);
    Bite { depth_mm: wanted.min(ceiling), wanted_mm: wanted, ceiling_mm: ceiling }
}

/// The band drawing and the layer that shows it, created on first use.
/// Returns the drawing's index in `design.drawn`.
///
/// The two are halves of one thing: strokes travel inside the design so a
/// shared file is self-contained, and the layer is an ordinary
/// [`TilingLayer`] — one seam-wrapped cell covering the whole band at
/// [`MAX_RELIEF_MM`], so every existing blend, window and mask control
/// applies, and both apps produce byte-compatible structures.
pub fn ensure_band_layer(design: &mut RingDesign) -> usize {
    let index = match design.drawn.iter().position(|d| d.name == BAND_ALPHA) {
        Some(i) => i,
        None => {
            let mut d = DrawnAlpha::new(BAND_ALPHA, BAND_W, BAND_H);
            d.wrap_x = true;
            design.drawn.push(d);
            design.drawn.len() - 1
        }
    };

    let exists = design
        .layers
        .layers
        .iter()
        .any(|e| matches!(&e.layer, Layer::Tiling(t) if t.alpha == BAND_ALPHA));
    if !exists {
        let ctx = design.field_context();
        let mut t = TilingLayer::default_for(BAND_ALPHA.to_string(), &ctx);
        t.repeats_around = 1;
        t.rows = 1;
        t.continuous = true;
        // The alpha stores depth as a fraction of the 1.6 mm maximum, so the
        // layer's height is that maximum and the composite gives back the
        // millimetres the pen asked for.
        t.height_mm = MAX_RELIEF_MM;
        t.v_center_mm = ctx.band_v_len_mm * 0.5;
        t.v_span_mm = ctx.band_v_len_mm;
        design
            .layers
            .layers
            .push(LayerEntry::new(BAND_ALPHA.to_string(), Layer::Tiling(t)));
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfileStyle, RingDesign};

    fn ctx_for(style: ProfileStyle) -> FieldContext {
        let mut d = RingDesign::default();
        d.profile.apply_style(style);
        d.field_context()
    }

    #[test]
    fn a_half_round_crest_takes_almost_nothing() {
        let ctx = ctx_for(ProfileStyle::HalfRound);
        let at_crest = ceiling_mm(&ctx, ctx.crest_v_mm);
        assert!(
            at_crest <= 0.1,
            "the crown of a half-round undercuts at 0.30 mm; got a {at_crest} mm ceiling"
        );
    }

    #[test]
    fn a_flat_bands_side_face_takes_the_full_depth() {
        let ctx = ctx_for(ProfileStyle::Flat);
        let v = ctx.band_v_len_mm * 0.06;
        assert_eq!(ceiling_mm(&ctx, v), MAX_RELIEF_MM);
    }

    #[test]
    fn the_ceiling_never_leaves_the_measured_range() {
        let ctx = ctx_for(ProfileStyle::DShape);
        for i in 0..=100 {
            let v = ctx.band_v_len_mm * i as f64 / 100.0;
            let c = ceiling_mm(&ctx, v);
            assert!((0.05..=MAX_RELIEF_MM).contains(&c), "v={v} gave {c}");
        }
    }

    #[test]
    fn pressure_maps_honestly() {
        for p in [0.0, 0.01, 0.2, 0.5, 1.0] {
            assert!(wanted_mm(p, 1.0) >= MIN_RELIEF_MM, "p={p} fell under the minimum edge");
        }
        assert!(wanted_mm(0.2, 1.0) < wanted_mm(0.8, 1.0));
        assert!((wanted_mm(1.0, 1.0) - MAX_RELIEF_MM).abs() < 1e-9);
        let ctx = ctx_for(ProfileStyle::HalfRound);
        let b = bite(&ctx, ctx.crest_v_mm, 1.0, 1.0);
        assert!(b.clamped());
        assert_eq!(b.depth_mm, b.ceiling_mm);
    }

    #[test]
    fn the_band_layer_convention_is_stable_and_idempotent() {
        let mut d = RingDesign::default();
        let i = ensure_band_layer(&mut d);
        assert_eq!(i, 0);
        assert_eq!(d.drawn[i].name, BAND_ALPHA);
        assert!(d.drawn[i].wrap_x && !d.drawn[i].wrap_y);
        assert_eq!(ensure_band_layer(&mut d), i, "second call must not duplicate");
        assert_eq!(d.drawn.len(), 1);
        let bands: Vec<&LayerEntry> = d
            .layers
            .layers
            .iter()
            .filter(|e| matches!(&e.layer, Layer::Tiling(t) if t.alpha == BAND_ALPHA))
            .collect();
        assert_eq!(bands.len(), 1);
        let Layer::Tiling(t) = &bands[0].layer else { unreachable!() };
        assert_eq!(t.repeats_around, 1);
        assert_eq!(t.height_mm, MAX_RELIEF_MM);
    }
}
