//! A wire swept along a drawn path — scrolls, vines, and wavy rails.
//!
//! The path is a Catmull-Rom spline through control points in *cell space*:
//! `x` runs 0..1 across one instance's arc and `v` is millimetres across the
//! band. `repeats_around` instances tile the ring, and because the count is an
//! integer and `x` wraps at the cell edge, the result closes on itself the
//! same way tiling does. The height at a point is the wire's cross-section
//! applied to the distance from the path — a distance field, so it composes
//! with every blend and never needs a raster.

use serde::{Deserialize, Serialize};

use crate::field::{FeatureFootprint, FieldContext, Uv, smoothstep};

/// Control points beyond this are ignored; the spline is evaluated per mesh
/// sample and a hostile file must not turn that into an unbounded loop.
pub const MAX_CURVE_POINTS: usize = 64;

/// Straight chords each spline segment is measured through.
const SEG_STEPS: usize = 12;

/// Wire cross-sections. A subset of the border rails: rope needs a phase along
/// the rail, which a free path does not carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireProfile {
    Round,
    Flat,
    Knife,
}

impl WireProfile {
    pub const ALL: &'static [WireProfile] =
        &[WireProfile::Round, WireProfile::Flat, WireProfile::Knife];

    pub fn label(self) -> &'static str {
        match self {
            WireProfile::Round => "Round wire",
            WireProfile::Flat => "Flat strap",
            WireProfile::Knife => "Knife edge",
        }
    }

    /// Height fraction at normalized distance `x` (0 at the spine, 1 at the
    /// edge of the wire).
    ///
    /// Round is a cosine dome, not a circle: a circular section has a vertical
    /// wall at its own edge, which leans past vertical wherever the crown
    /// curves — measured at 4.1% undercut area on the vine preset. The cosine
    /// caps the edge slope at about 57 degrees of wall for a wire as tall as
    /// it is wide, and reads as round wire at ring scale.
    fn shape(self, x: f64) -> f64 {
        match self {
            WireProfile::Round => 0.5 + 0.5 * (std::f64::consts::PI * x.clamp(0.0, 1.0)).cos(),
            WireProfile::Flat => 1.0 - smoothstep(0.7, 1.0, x),
            WireProfile::Knife => 1.0 - x,
        }
    }
}

/// A drawn path swept with a wire profile, instanced around the ring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveLayer {
    /// Control points as `(x, v_mm)`: `x` 0..1 across one instance's arc.
    pub points: Vec<[f64; 2]>,
    /// Instances around the ring. Integer, so the pattern is seamless.
    pub repeats_around: u32,
    /// Join the last point back to the first, for a rail with no ends.
    pub closed: bool,
    pub width_mm: f64,
    pub height_mm: f64,
    pub profile: WireProfile,
    /// Fraction of each end the wire tapers over, 0..0.5. Open paths only.
    pub taper: f64,
    /// Also place a copy mirrored about the middle of the band.
    pub mirror_v: bool,
}

impl Default for CurveLayer {
    fn default() -> Self {
        Self {
            points: vec![[0.1, 2.0], [0.35, 3.2], [0.65, 0.8], [0.9, 2.0]],
            repeats_around: 8,
            closed: false,
            width_mm: 0.7,
            height_mm: 0.35,
            profile: WireProfile::Round,
            taper: 0.15,
            mirror_v: false,
        }
    }
}

impl CurveLayer {
    /// A classic S-scroll through the middle of the band.
    pub fn preset_scroll(ctx: &FieldContext) -> Self {
        let m = ctx.crest_v_mm;
        let a = (ctx.band_v_len_mm * 0.22).min(2.2);
        Self {
            points: vec![
                [0.12, m - a * 0.2],
                [0.24, m + a],
                [0.44, m + a * 0.4],
                [0.56, m - a * 0.4],
                [0.76, m - a],
                [0.88, m + a * 0.2],
            ],
            repeats_around: 8,
            ..Self::default()
        }
    }

    /// A running vine: one sine arch per instance, ends meeting at the cell
    /// edge so the repeats read as one continuous stem.
    pub fn preset_vine(ctx: &FieldContext) -> Self {
        let m = ctx.crest_v_mm;
        let a = (ctx.band_v_len_mm * 0.18).min(1.8);
        Self {
            points: vec![[0.0, m], [0.25, m + a], [0.5, m], [0.75, m - a], [1.0, m]],
            repeats_around: 12,
            closed: false,
            taper: 0.0,
            ..Self::default()
        }
    }

    /// A closed wavy rail all the way round — the border a fixed-`v` rail
    /// cannot make.
    pub fn preset_wave_rail(ctx: &FieldContext) -> Self {
        let m = ctx.crest_v_mm;
        let a = (ctx.band_v_len_mm * 0.15).min(1.5);
        Self {
            points: vec![[0.0, m], [0.25, m + a], [0.5, m], [0.75, m - a]],
            repeats_around: 6,
            closed: true,
            taper: 0.0,
            ..Self::default()
        }
    }

    /// Move the drawn points to a new `v` centre and amplitude, keeping the
    /// shape. This is how a preset lands on a side face: a wire crossing the
    /// crown undercuts on its crest-side flank wherever the dome's draft is
    /// shallower than the wire's own slope — measured 1.1% of the surface at
    /// -31 degrees for a rail waving just 0.2 mm at 0.15 mm high — while the
    /// same wire on a side face measures 0.000% at 0.5 mm high.
    pub fn retarget_v(&mut self, center_mm: f64, amplitude_mm: f64) {
        let (lo, hi) = self.v_extent();
        let old_c = 0.5 * (lo + hi);
        let old_a = (hi - lo) * 0.5;
        for p in &mut self.points {
            let t = if old_a > 1e-9 { (p[1] - old_c) / old_a } else { 0.0 };
            p[1] = center_mm + t * amplitude_mm;
        }
    }

    /// The spline flattened to a polyline in `(x, v_mm)` cell space, for
    /// editors and overlays.
    pub fn sample_path(&self, per_seg: usize) -> Vec<[f64; 2]> {
        let n = self.points.len().min(MAX_CURVE_POINTS);
        if n < 2 {
            return self.points.iter().take(n).copied().collect();
        }
        let per_seg = per_seg.clamp(2, 64);
        let pt = |i: isize| -> [f64; 2] {
            if self.closed {
                let k = i.rem_euclid(n as isize) as usize;
                let cycles = ((i - k as isize) / n as isize) as f64;
                [self.points[k][0] + cycles, self.points[k][1]]
            } else {
                self.points[i.clamp(0, n as isize - 1) as usize]
            }
        };
        let segs = if self.closed { n } else { n - 1 };
        let mut out = Vec::with_capacity(segs * per_seg + 1);
        out.push(pt(0));
        for s in 0..segs as isize {
            let (p0, p1, p2, p3) = (pt(s - 1), pt(s), pt(s + 1), pt(s + 2));
            for k in 1..=per_seg {
                out.push(catmull_rom(p0, p1, p2, p3, k as f64 / per_seg as f64));
            }
        }
        out
    }

    /// `v` extent of the drawn points, without the wire's own width.
    pub fn v_extent(&self) -> (f64, f64) {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for p in self.points.iter().take(MAX_CURVE_POINTS) {
            lo = lo.min(p[1]);
            hi = hi.max(p[1]);
        }
        if lo > hi { (0.0, 0.0) } else { (lo, hi) }
    }

    pub fn feature_footprints(&self, ctx: &FieldContext) -> Vec<FeatureFootprint> {
        let (lo, hi) = self.v_extent();
        let half = self.width_mm * 0.5;
        // A wire's width is measured across its own path, so it is the same
        // number whichever way the path runs.
        let f = |v: (f64, f64)| FeatureFootprint::round(self.width_mm.max(0.1), None, v);
        let v = (lo - half, hi + half);
        if self.mirror_v {
            let m = (ctx.band_v_len_mm - v.1, ctx.band_v_len_mm - v.0);
            vec![f(v), f(m)]
        } else {
            vec![f(v)]
        }
    }

    /// Displacement at a surface point, mm.
    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let n = self.points.len().min(MAX_CURVE_POINTS);
        if n < 2 || self.width_mm <= 1e-6 || !uv.u.is_finite() || !uv.v.is_finite() {
            return 0.0;
        }
        let circ = ctx.circumference_mm;
        if !(circ > 1e-9) {
            return 0.0;
        }
        let repeats = self.repeats_around.clamp(1, 400) as f64;
        let cell_mm = circ / repeats;

        // Position inside the owning instance, in mm.
        let x_frac = (uv.u / circ).rem_euclid(1.0) * repeats;
        let local = x_frac - x_frac.floor();
        let px = local * cell_mm;

        let h = self.instance_height(px, uv.v, cell_mm);
        if self.mirror_v {
            h.max(self.instance_height(px, ctx.band_v_len_mm - uv.v, cell_mm))
        } else {
            h
        }
    }

    /// Height from one instance's path, with the two neighbouring instances
    /// checked too so a stroke reaching past its cell edge stays continuous.
    fn instance_height(&self, px: f64, pv: f64, cell_mm: f64) -> f64 {
        let mut best_d = f64::MAX;
        let mut best_t = 0.0;
        for shift in [-1.0, 0.0, 1.0] {
            let (d, t) = self.path_distance(px + shift * cell_mm, pv, cell_mm);
            if d < best_d {
                best_d = d;
                best_t = t;
            }
        }
        let half = (self.width_mm * 0.5).max(1e-6);
        let x = best_d / half;
        if x >= 1.0 {
            return 0.0;
        }
        let mut h = self.height_mm * self.profile.shape(x.clamp(0.0, 1.0));
        if !self.closed && self.taper > 1e-6 {
            let end = best_t.min(1.0 - best_t);
            h *= smoothstep(0.0, self.taper.min(0.5), end);
        }
        h.max(0.0)
    }

    /// Distance from `(px, pv)` mm to the spline, and the parameter 0..1 of
    /// the nearest point along it.
    fn path_distance(&self, px: f64, pv: f64, cell_mm: f64) -> (f64, f64) {
        let n = self.points.len().min(MAX_CURVE_POINTS);
        // Closed paths continue into the neighbouring instance rather than
        // jumping back across the cell: index i cycles through the points
        // while x unwraps by a whole cell per cycle.
        let pt = |i: isize| -> [f64; 2] {
            let p = if self.closed {
                let k = i.rem_euclid(n as isize) as usize;
                let cycles = ((i - k as isize) / n as isize) as f64;
                [self.points[k][0] + cycles, self.points[k][1]]
            } else {
                self.points[i.clamp(0, n as isize - 1) as usize]
            };
            [p[0].clamp(-1.5, 2.5) * cell_mm, p[1]]
        };
        let segs = if self.closed { n } else { n - 1 };

        let mut best = f64::MAX;
        let mut best_t = 0.0;
        // Every segment starts at its own p1; seeding anywhere else adds a
        // phantom chord cutting across the loop.
        let mut prev = pt(0);
        // Catmull-Rom per segment, walked as chords with point-to-segment
        // distance so a coarse walk cannot cut a corner by a whole chord.
        for s in 0..segs as isize {
            let (p0, p1, p2, p3) = (pt(s - 1), pt(s), pt(s + 1), pt(s + 2));
            for k in 1..=SEG_STEPS {
                let t = k as f64 / SEG_STEPS as f64;
                let cur = catmull_rom(p0, p1, p2, p3, t);
                let (d, ft) = seg_distance([px, pv], prev, cur);
                if d < best {
                    best = d;
                    let tt = (s as f64 + t - 1.0 / SEG_STEPS as f64 + ft / SEG_STEPS as f64)
                        / segs as f64;
                    best_t = tt.clamp(0.0, 1.0);
                }
                prev = cur;
            }
        }
        (best, best_t)
    }
}

fn catmull_rom(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], p3: [f64; 2], t: f64) -> [f64; 2] {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * ((2.0 * b) + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (3.0 * b - a - 3.0 * c + d) * t3)
    };
    [f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]
}

/// Distance from `p` to segment `a..b`, and the fraction along it.
fn seg_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 > 1e-18 {
        (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    (((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt(), t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> FieldContext {
        FieldContext {
            circumference_mm: 60.0,
            band_v_len_mm: 8.0,
            crest_v_mm: 4.0,
            crest_radius_mm: 9.5,
            surface: Default::default(),
            bore_radius_mm: 8.5,
            side_faces_cache: Default::default(),
        }
    }

    #[test]
    fn the_wire_peaks_on_the_path_and_vanishes_off_it() {
        let c = ctx();
        let l = CurveLayer {
            points: vec![[0.0, 4.0], [1.0, 4.0]],
            repeats_around: 1,
            taper: 0.0,
            ..CurveLayer::default()
        };
        let mid = l.height(Uv { u: 30.0, v: 4.0 }, &c);
        assert!((mid - l.height_mm).abs() < 1e-6, "on the spine: {mid}");
        assert_eq!(l.height(Uv { u: 30.0, v: 6.5 }, &c), 0.0, "clear of the wire");
    }

    #[test]
    fn instances_close_seamlessly_at_the_joint() {
        let c = ctx();
        let l = CurveLayer::preset_vine(&c);
        for dv in [-0.4, 0.0, 0.3] {
            let v = 4.0 + dv;
            let a = l.height(Uv { u: 0.0, v }, &c);
            let b = l.height(Uv { u: 60.0 - 1e-9, v }, &c);
            assert!((a - b).abs() < 1e-6, "joint mismatch at v {v}: {a} vs {b}");
        }
    }

    #[test]
    fn a_closed_rail_has_no_ends_and_an_open_stroke_tapers() {
        let c = ctx();
        let rail = CurveLayer::preset_wave_rail(&c);
        // Sample along the rail's own spine: height stays full everywhere.
        for i in 0..48 {
            let u = i as f64 / 48.0 * 60.0;
            let mut peak = 0.0f64;
            for j in 0..40 {
                let v = 2.0 + j as f64 * 0.1;
                peak = peak.max(rail.height(Uv { u, v }, &c));
            }
            assert!(
                (peak - rail.height_mm).abs() < 0.02,
                "closed rail dipped to {peak} at u {u}"
            );
        }

        let scroll = CurveLayer { taper: 0.25, ..CurveLayer::preset_scroll(&c) };
        let cell = 60.0 / scroll.repeats_around as f64;
        let end_x = scroll.points[0][0] * cell;
        let end_v = scroll.points[0][1];
        let end = scroll.height(Uv { u: end_x, v: end_v }, &c);
        assert!(
            end < scroll.height_mm * 0.7,
            "the stroke's end should taper: {end} vs {}",
            scroll.height_mm
        );
    }

    /// A wire crossing the crown undercuts on its crest-side flank wherever
    /// the dome's draft is shallower than the wire's own slope — that is the
    /// casting constraint, not a bug (measured 1.1% at -31 degrees for a rail
    /// waving 0.2 mm at 0.15 mm high). On a side face the same wire is
    /// castable by construction. This pins both, and that the side-face vine
    /// actually contributes rather than being gated to nothing.
    #[test]
    fn a_curve_layer_builds_watertight_and_releases_on_a_side_face() {
        let lib = crate::AlphaLibrary::builtin();
        let params =
            crate::BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() };

        let mut d = crate::RingDesign::default();
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 3.0;
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.flatten_sides();
        let fc = d.field_context();
        let (lo, hi) = fc.side_faces_std().expect("squared sides").wider().unwrap();

        let mut vine = CurveLayer::preset_vine(&fc);
        vine.height_mm = 0.5;
        vine.taper = 0.0;
        vine.retarget_v(0.5 * (lo + hi), (hi - lo) * 0.3);
        let mut entry = crate::LayerEntry::new("side vine", crate::Layer::Curve(vine));
        entry.window.v_gate =
            crate::field::VGate::SideFaces(crate::field::SideFacePick::Wider);
        d.layers.layers.push(entry);

        let out = crate::mesh::build(&d, &lib, params);
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        assert!(
            out.report.max_relief_mm > 0.4,
            "the gated vine must still land on the face: relief {:.3}",
            out.report.max_relief_mm
        );
        let cast = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert!(
            cast.undercut_fraction() < 0.001,
            "side-face vine at 0.5 mm: {:.4}% undercut",
            cast.undercut_fraction() * 100.0
        );

        // The crown regression fence: the rail's undercut area is real and
        // must keep being reported, not silently shrink or grow.
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let fc = d.field_context();
        let mut rail = CurveLayer::preset_wave_rail(&fc);
        rail.height_mm = 0.15;
        d.layers.layers.push(crate::LayerEntry::new("rail", crate::Layer::Curve(rail)));
        let out = crate::mesh::build(&d, &lib, params);
        assert!(out.report.validation.watertight);
        let cast = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        let pct = cast.undercut_fraction() * 100.0;
        assert!(
            (0.5..6.0).contains(&pct),
            "crown rail undercut should stay honestly reported: {pct:.3}%"
        );
    }

    #[test]
    fn hostile_inputs_do_not_panic() {
        let c = ctx();
        let cases = [
            CurveLayer { points: vec![], ..Default::default() },
            CurveLayer { points: vec![[f64::NAN, f64::NAN]; 3], ..Default::default() },
            CurveLayer { points: vec![[0.0, 0.0]; 10_000], ..Default::default() },
            CurveLayer { repeats_around: 0, ..Default::default() },
            CurveLayer { width_mm: -1.0, ..Default::default() },
        ];
        for l in cases {
            let h = l.height(Uv { u: f64::INFINITY, v: f64::NAN }, &c);
            assert!(h == 0.0 || h.is_finite());
            let h = l.height(Uv { u: 10.0, v: 4.0 }, &c);
            assert!(h == 0.0 || h.is_finite());
        }
    }
}
