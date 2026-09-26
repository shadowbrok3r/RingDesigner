//! A wire swept along a drawn path — scrolls, vines, and wavy rails.
//!
//! The path is a Catmull-Rom spline through control points in *cell space*:
//! `x` runs 0..1 across one instance's arc and `v` is millimetres across the
//! band. `repeats_around` instances tile the ring, and because the count is an
//! integer and `x` wraps at the cell edge, the result closes on itself the
//! same way tiling does. The height at a point is the wire's cross-section
//! applied to the distance from the path — a distance field, so it composes
//! with every blend and never needs a raster.
//!
//! A profiled wire (per-point widths, heights or beads) is the union of its sections swept along the path.

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::field::{FeatureFootprint, FieldContext, Uv, smoothstep};

/// Control points beyond this are ignored; the spline is evaluated per mesh
/// sample and a hostile file must not turn that into an unbounded loop.
pub const MAX_CURVE_POINTS: usize = 64;

/// Beads one instance may carry: a hostile pitch must not turn a layer into an
/// unbounded walk or an unbounded list.
pub const MAX_CURVE_BEADS: usize = 4096;

/// Straight chords each spline segment is measured through.
const SEG_STEPS: usize = 12;

/// Longest chord a profiled wire is flattened to, mm.
const PROFILED_CHORD_MM: f64 = 0.15;

/// Chords a profiled wire's sampler tests as one box.
const BLOCK_CHORDS: usize = 16;

/// Largest per-point multiplier read; hostile values clamp here.
const MAX_MULTIPLIER: f64 = 64.0;

/// A cupped bead's dimple, fraction of its diameter.
pub const CUP_SPAN: f64 = 0.5;

/// Wire cross-sections. A subset of the border rails: rope needs a phase along
/// the rail, which a free path does not carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireProfile {
    Round,
    Flat,
    Knife,
    /// A semicircle, vertical at its own edge: a limb lying on the band, for lost wax.
    Tube,
}

impl WireProfile {
    pub const ALL: &'static [WireProfile] =
        &[WireProfile::Round, WireProfile::Flat, WireProfile::Knife, WireProfile::Tube];

    pub fn label(self) -> &'static str {
        match self {
            WireProfile::Round => "Round wire",
            WireProfile::Flat => "Flat strap",
            WireProfile::Knife => "Knife edge",
            WireProfile::Tube => "Tube (lost wax)",
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
            WireProfile::Tube => (1.0 - x * x).max(0.0).sqrt(),
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
    /// Per control point, times `width_mm`; empty is uniform, a short list repeats its last value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widths: Vec<f64>,
    /// Per control point, times `height_mm`; empty is uniform, a short list repeats its last value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heights: Vec<f64>,
    /// A row of beads laid along the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beads: Option<CurveBeads>,
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
            widths: Vec::new(),
            heights: Vec::new(),
            beads: None,
        }
    }
}

/// A row of beads standing on a wire, each in the wire's own profile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveBeads {
    /// Spacing along the path, mm.
    pub pitch_mm: f64,
    pub diameter_mm: f64,
    pub height_mm: f64,
    /// Where across the wire the row runs, -1..1 of the local half-width, positive to the path's left.
    pub offset: f64,
    /// Pitch, diameter and height follow the local width multiplier.
    pub graded: bool,
    /// Where along one pitch the first bead sits, 0..1.
    pub phase: f64,
    /// Share of the path's length the row runs over, from its first point.
    #[serde(default = "full_span")]
    pub span: [f64; 2],
    /// Alternate beads step this far either side of `offset`, fraction of the half-width.
    #[serde(default)]
    pub stagger: f64,
    /// Depth of a dimple at each bead's centre, fraction of its height: 0 is a dome, toward 1 a cup.
    #[serde(default)]
    pub cup: f64,
}

fn full_span() -> [f64; 2] {
    [0.0, 1.0]
}

impl Default for CurveBeads {
    fn default() -> Self {
        Self { pitch_mm: 0.75, diameter_mm: 0.55, height_mm: 0.25, offset: 0.0, graded: true, phase: 0.0, span: full_span(), stagger: 0.0, cup: 0.0 }
    }
}

/// Whether any wire in `stack`, groups included, carries per-point widths, heights, beads or the tube section.
pub fn profiled_in(stack: &crate::LayerStack) -> bool {
    stack.layers.iter().any(|e| match &e.layer {
        crate::Layer::Curve(c) => !c.is_plain() || c.profile == WireProfile::Tube,
        crate::Layer::Group(g) => profiled_in(&g.stack),
        _ => false,
    })
}

/// Whether a serialized wire carries per-point widths, heights, beads or the tube section.
pub fn profiled_json(curve: &serde_json::Value) -> bool {
    let listed = |key: &str| curve.get(key).and_then(serde_json::Value::as_array).is_some_and(|a| !a.is_empty());
    listed("widths") || listed("heights") || curve.get("beads").is_some_and(|b| !b.is_null()) || curve.get("profile").and_then(serde_json::Value::as_str) == Some("Tube")
}

impl CurveLayer {
    /// No per-point profile and no beads: the construction every older build reads.
    pub fn is_plain(&self) -> bool {
        self.widths.is_empty() && self.heights.is_empty() && self.beads.is_none()
    }

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

    /// Land the wire on the band's wider side face, at `fill` of that face's
    /// width, and report whether there was one.
    ///
    /// This is the placement the doctrine requires and it was being spelled
    /// out at every call site: a wire crossing the crown undercuts on its
    /// crest-side flank wherever the dome's draft is shallower than the wire's
    /// own slope, while the same wire on a side face measures 0.000%. A
    /// profile with no side face returns `false` and is left alone — keep the
    /// wire shallow there, or square the band's sides first.
    pub fn land_on_side_face(&mut self, ctx: &FieldContext, fill: f64) -> bool {
        let Some((lo, hi)) = ctx.side_faces_std().and_then(|sf| sf.wider()) else { return false };
        self.retarget_v(0.5 * (lo + hi), (hi - lo) * 0.5 * fill.clamp(0.0, 1.0));
        true
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

    /// Instance arc in mm, when the ring has one.
    fn cell_mm(&self, ctx: &FieldContext) -> Option<f64> {
        let circ = ctx.circumference_mm;
        (circ > 1e-9).then(|| circ / self.repeats_around.clamp(1, 400) as f64)
    }

    /// Beads one instance places, and the smallest one's finest feature in mm (infinite with none).
    pub fn bead_census(&self, ctx: &FieldContext) -> (usize, f64) {
        match self.cell_mm(ctx) {
            Some(cell) if self.beads.is_some() && self.points.len().min(MAX_CURVE_POINTS) >= 2 => {
                let p = profiled(self, cell);
                (p.beads.len(), p.bead_extent().map_or(f64::INFINITY, |(d, _)| d))
            }
            _ => (0, f64::INFINITY),
        }
    }

    /// The thinnest and the widest point of a profiled wire, mm.
    pub fn width_range_mm(&self) -> (f64, f64) {
        let n = self.points.len().min(MAX_CURVE_POINTS).max(1);
        let (lo, hi) = (0..n).map(|k| multiplier(&self.widths, k)).fold((f64::MAX, f64::MIN), |(a, b), m| (a.min(m), b.max(m)));
        (self.width_mm * lo, self.width_mm * hi)
    }

    pub fn feature_footprints(&self, ctx: &FieldContext) -> Vec<FeatureFootprint> {
        if !self.is_plain() {
            return self.profiled_footprints(ctx);
        }
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

    /// The thinnest point of the wire, then the smallest bead, each mirrored when the wire is.
    fn profiled_footprints(&self, ctx: &FieldContext) -> Vec<FeatureFootprint> {
        let (lo, hi) = self.v_extent();
        let (thin, wide) = self.width_range_mm();
        let half = 0.5 * wide;
        let mut out = vec![FeatureFootprint::round(thin.max(0.1), None, (lo - half, hi + half))];
        if let Some(cell) = self.cell_mm(ctx).filter(|_| self.beads.is_some()) {
            if let Some((d, v)) = profiled(self, cell).bead_extent() {
                out.push(FeatureFootprint::round(d.max(0.1), None, v));
            }
        }
        if self.mirror_v {
            let band = ctx.band_v_len_mm;
            let mirrored: Vec<FeatureFootprint> = out
                .iter()
                .map(|f| FeatureFootprint { v_mm: (band - f.v_mm.1, band - f.v_mm.0), ..*f })
                .collect();
            out.extend(mirrored);
        }
        out
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

        if !self.is_plain() {
            let p = profiled(self, cell_mm);
            let h = p.height(px, uv.v, cell_mm, self.profile);
            return if self.mirror_v { h.max(p.height(px, ctx.band_v_len_mm - uv.v, cell_mm, self.profile)) } else { h };
        }
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

/// A per-point multiplier, clamped; a non-finite value reads as zero.
fn multiplier(list: &[f64], k: usize) -> f64 {
    match list.len() {
        0 => 1.0,
        len => {
            let x = list[k.min(len - 1)];
            if x.is_finite() { x.clamp(0.0, MAX_MULTIPLIER) } else { 0.0 }
        }
    }
}

/// Catmull-Rom through four knot values, held between the segment's own two.
fn knot_cr([a, b, c, d]: [f64; 4], t: f64) -> f64 {
    let t2 = t * t;
    let x = 0.5 * ((2.0 * b) + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t2 * t);
    x.clamp(b.min(c), b.max(c))
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// A profiled wire flattened for sampling.
struct Profiled {
    /// Chord ends in mm across one instance; a closed path runs on past the cell edge.
    pts: Vec<[f64; 2]>,
    /// Half-width and height, taper included, at each chord end.
    half: Vec<f64>,
    tall: Vec<f64>,
    /// Beads as centre, radius and height.
    beads: Vec<([f64; 2], f64, f64)>,
    /// Dimple depth of every bead, fraction of its height.
    cup: f64,
    blocks: Vec<Block>,
}

/// A run of chords and the beads standing on them, with the box a sample must fall in to feel either.
struct Block {
    chords: std::ops::Range<usize>,
    beads: std::ops::Range<usize>,
    lo: [f64; 2],
    hi: [f64; 2],
}

impl Profiled {
    fn new(c: &CurveLayer, cell_mm: f64) -> Self {
        let cup = c.beads.map_or(0.0, |b| if b.cup.is_finite() { b.cup.clamp(0.0, 1.0) } else { 0.0 });
        let mut out = Self { pts: Vec::new(), half: Vec::new(), tall: Vec::new(), beads: Vec::new(), cup, blocks: Vec::new() };
        let n = c.points.len().min(MAX_CURVE_POINTS);
        if n < 2 || !(cell_mm > 1e-9) {
            return out;
        }
        let pt = |i: isize| -> [f64; 2] {
            let p = if c.closed {
                let k = i.rem_euclid(n as isize) as usize;
                let cycles = ((i - k as isize) / n as isize) as f64;
                [c.points[k][0] + cycles, c.points[k][1]]
            } else {
                c.points[i.clamp(0, n as isize - 1) as usize]
            };
            [p[0].clamp(-1.5, 2.5) * cell_mm, p[1]]
        };
        let knot = |i: isize| -> usize {
            if c.closed { i.rem_euclid(n as isize) as usize } else { i.clamp(0, n as isize - 1) as usize }
        };
        let segs = if c.closed { n } else { n - 1 };
        let (mut t_at, mut wm, mut hm) = (vec![0.0], vec![multiplier(&c.widths, 0)], vec![multiplier(&c.heights, 0)]);
        out.pts.push(pt(0));
        for s in 0..segs as isize {
            let (p0, p1, p2, p3) = (pt(s - 1), pt(s), pt(s + 1), pt(s + 2));
            let reach = (p2[0] - p1[0]).hypot(p2[1] - p1[1]);
            let steps = if reach.is_finite() { ((reach / PROFILED_CHORD_MM).ceil() as usize).clamp(SEG_STEPS, 96) } else { SEG_STEPS };
            let w = [-1, 0, 1, 2].map(|k| multiplier(&c.widths, knot(s + k)));
            let h = [-1, 0, 1, 2].map(|k| multiplier(&c.heights, knot(s + k)));
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                out.pts.push(catmull_rom(p0, p1, p2, p3, t));
                t_at.push((s as f64 + t) / segs as f64);
                wm.push(knot_cr(w, t));
                hm.push(knot_cr(h, t));
            }
        }
        let taper = if c.closed || !(c.taper > 1e-6) { 0.0 } else { c.taper.min(0.5) };
        for (i, t) in t_at.iter().enumerate() {
            let fade = if taper > 0.0 { smoothstep(0.0, taper, t.min(1.0 - t)) } else { 1.0 };
            out.half.push(0.5 * c.width_mm.max(0.0) * wm[i]);
            out.tall.push(c.height_mm * hm[i] * fade);
        }
        let on_chord = out.lay_beads(c, &wm);
        out.block(&on_chord);
        out
    }

    /// Walks the path by arc length and stands a bead at every pitch; returns the chord each bead stands on.
    fn lay_beads(&mut self, c: &CurveLayer, wm: &[f64]) -> Vec<usize> {
        let mut on_chord = Vec::new();
        let Some(b) = c.beads else { return on_chord };
        let finite = [b.pitch_mm, b.diameter_mm, b.height_mm, b.offset, b.stagger].iter().all(|x| x.is_finite());
        if !finite || b.pitch_mm <= 1e-6 || b.diameter_mm <= 1e-6 || b.height_mm <= 0.0 || self.pts.len() < 2 {
            return on_chord;
        }
        let mut acc = vec![0.0; self.pts.len()];
        for i in 1..self.pts.len() {
            let l = (self.pts[i][0] - self.pts[i - 1][0]).hypot(self.pts[i][1] - self.pts[i - 1][1]);
            acc[i] = acc[i - 1] + if l.is_finite() { l } else { 0.0 };
        }
        let total = acc[acc.len() - 1];
        if !(total > 1e-9) {
            return on_chord;
        }
        let frac = |x: f64, or: f64| if x.is_finite() { x.clamp(0.0, 1.0) } else { or };
        let (from, to) = (frac(b.span[0], 0.0) * total, frac(b.span[1], 1.0) * total);
        let at = |s: f64| -> (usize, f64) {
            let i = acc.partition_point(|&a| a <= s).saturating_sub(1).min(acc.len() - 2);
            let run = acc[i + 1] - acc[i];
            (i, if run > 1e-12 { ((s - acc[i]) / run).clamp(0.0, 1.0) } else { 0.0 })
        };
        let grade = |i: usize, f: f64| if b.graded { lerp(wm[i], wm[i + 1], f) } else { 1.0 };
        let pitch = |g: f64| (b.pitch_mm * g).max(0.02 * b.pitch_mm).max(1e-3);
        let whole_loop = c.closed && from <= 0.0 && to >= total;
        let (i, f) = at(from);
        let mut s = from + frac(b.phase, 0.0) * pitch(grade(i, f));
        for k in 0..MAX_CURVE_BEADS {
            if if whole_loop { s >= to - 1e-9 } else { s > to + 1e-9 } {
                break;
            }
            let (i, f) = at(s);
            let g = grade(i, f);
            let (a, e) = (self.pts[i], self.pts[i + 1]);
            let len = (e[0] - a[0]).hypot(e[1] - a[1]);
            let left = if len > 1e-12 { [-(e[1] - a[1]) / len, (e[0] - a[0]) / len] } else { [0.0, 1.0] };
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let across = (b.offset + side * b.stagger) * lerp(self.half[i], self.half[i + 1], f);
            let centre = [lerp(a[0], e[0], f) + left[0] * across, lerp(a[1], e[1], f) + left[1] * across];
            let (r, h) = (0.5 * b.diameter_mm * g, b.height_mm * g);
            if r > 1e-6 && h > 0.0 && centre[0].is_finite() && centre[1].is_finite() {
                self.beads.push((centre, r, h));
                on_chord.push(i);
                if self.beads.len() >= MAX_CURVE_BEADS {
                    break;
                }
            }
            s += pitch(g);
        }
        on_chord
    }

    /// Groups the chords into runs, each boxed with its half-widths and its beads.
    fn block(&mut self, on_chord: &[usize]) {
        let chords = self.pts.len().saturating_sub(1);
        let (mut start, mut next_bead) = (0, 0);
        while start < chords {
            let end = (start + BLOCK_CHORDS).min(chords);
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            let mut grow = |p: [f64; 2], r: f64| {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k] - r);
                    hi[k] = hi[k].max(p[k] + r);
                }
            };
            for i in start..=end {
                grow(self.pts[i], self.half[i].max(0.0));
            }
            let first = next_bead;
            while next_bead < on_chord.len() && on_chord[next_bead] < end {
                let (c, r, _) = self.beads[next_bead];
                grow(c, r);
                next_bead += 1;
            }
            self.blocks.push(Block { chords: start..end, beads: first..next_bead, lo, hi });
            start = end;
        }
    }

    /// Height at `(px, pv)` mm in the owning instance, the neighbouring instances included.
    fn height(&self, px: f64, pv: f64, cell_mm: f64, profile: WireProfile) -> f64 {
        let (mut wire, mut bead) = (0.0f64, 0.0f64);
        for shift in [-1.0, 0.0, 1.0] {
            let q = [px + shift * cell_mm, pv];
            for b in &self.blocks {
                if !(q[0] >= b.lo[0] && q[0] <= b.hi[0] && q[1] >= b.lo[1] && q[1] <= b.hi[1]) {
                    continue;
                }
                for i in b.chords.clone() {
                    let (d, f) = seg_distance(q, self.pts[i], self.pts[i + 1]);
                    let r = lerp(self.half[i], self.half[i + 1], f);
                    if r > 1e-9 && d < r {
                        wire = wire.max(lerp(self.tall[i], self.tall[i + 1], f) * profile.shape(d / r));
                    }
                }
                for &(c, r, h) in &self.beads[b.beads.clone()] {
                    let d = ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2)).sqrt();
                    if d < r {
                        let x = d / r;
                        let dimple = if x < CUP_SPAN { self.cup * profile.shape(x / CUP_SPAN) } else { 0.0 };
                        bead = bead.max(h * (profile.shape(x) - dimple));
                    }
                }
            }
        }
        wire + bead
    }

    /// The smallest bead's finest feature (its diameter, or its dimple when cupped) and the `v` span the row covers, mm.
    fn bead_extent(&self) -> Option<(f64, (f64, f64))> {
        let mut d = f64::INFINITY;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let scale = if self.cup > 0.0 { CUP_SPAN } else { 1.0 };
        for &(c, r, _) in &self.beads {
            d = d.min(2.0 * r * scale);
            lo = lo.min(c[1] - r);
            hi = hi.max(c[1] + r);
        }
        d.is_finite().then_some((d, (lo, hi)))
    }
}

/// What a profiled wire was flattened from, compared bit for bit.
struct ProfiledKey {
    closed: bool,
    scalars: [u64; 4],
    points: Vec<[u64; 2]>,
    widths: Vec<u64>,
    heights: Vec<u64>,
    beads: Option<[u64; 10]>,
}

impl ProfiledKey {
    fn scalars(c: &CurveLayer, cell_mm: f64) -> [u64; 4] {
        [c.width_mm.to_bits(), c.height_mm.to_bits(), c.taper.to_bits(), cell_mm.to_bits()]
    }

    fn beads(b: &CurveBeads) -> [u64; 10] {
        let f = [b.pitch_mm, b.diameter_mm, b.height_mm, b.offset, b.phase, b.span[0], b.span[1], b.stagger, b.cup];
        let mut out = [0; 10];
        for (o, x) in out.iter_mut().zip(f) {
            *o = x.to_bits();
        }
        out[9] = u64::from(b.graded);
        out
    }

    /// The first `n` values, which are all a wire of `n` points reads.
    fn read(list: &[f64], n: usize) -> &[f64] {
        &list[..list.len().min(n)]
    }

    fn of(c: &CurveLayer, cell_mm: f64) -> Self {
        let n = c.points.len().min(MAX_CURVE_POINTS);
        Self {
            closed: c.closed,
            scalars: Self::scalars(c, cell_mm),
            points: c.points[..n].iter().map(|p| [p[0].to_bits(), p[1].to_bits()]).collect(),
            widths: Self::read(&c.widths, n).iter().map(|x| x.to_bits()).collect(),
            heights: Self::read(&c.heights, n).iter().map(|x| x.to_bits()).collect(),
            beads: c.beads.as_ref().map(Self::beads),
        }
    }

    fn matches(&self, c: &CurveLayer, cell_mm: f64) -> bool {
        let n = c.points.len().min(MAX_CURVE_POINTS);
        let same = |bits: &[u64], list: &[f64]| bits.len() == list.len() && bits.iter().zip(list).all(|(b, x)| *b == x.to_bits());
        self.closed == c.closed
            && self.scalars == Self::scalars(c, cell_mm)
            && self.points.len() == n
            && self.points.iter().zip(&c.points[..n]).all(|(b, p)| b[0] == p[0].to_bits() && b[1] == p[1].to_bits())
            && same(&self.widths, Self::read(&c.widths, n))
            && same(&self.heights, Self::read(&c.heights, n))
            && self.beads == c.beads.as_ref().map(Self::beads)
    }
}

/// Flattened wires kept per thread, newest first.
const PROFILED_KEPT: usize = 16;

thread_local! {
    static PROFILED: RefCell<Vec<(ProfiledKey, Rc<Profiled>)>> = const { RefCell::new(Vec::new()) };
}

/// The flattened form of `c` at this instance arc, built once per thread and content.
fn profiled(c: &CurveLayer, cell_mm: f64) -> Rc<Profiled> {
    PROFILED.with(|kept| {
        let mut kept = kept.borrow_mut();
        if let Some(i) = kept.iter().position(|(k, _)| k.matches(c, cell_mm)) {
            let hit = kept.remove(i);
            let shape = hit.1.clone();
            kept.insert(0, hit);
            return shape;
        }
        let shape = Rc::new(Profiled::new(c, cell_mm));
        kept.insert(0, (ProfiledKey::of(c, cell_mm), shape.clone()));
        kept.truncate(PROFILED_KEPT);
        shape
    })
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
            stretch: None,
            crest_scale: None,
            ..Default::default()
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
        let mut vine = CurveLayer::preset_vine(&fc);
        vine.height_mm = 0.5;
        vine.taper = 0.0;
        assert!(vine.land_on_side_face(&fc, 0.6), "a squared band has a side face");
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

    const PLAIN_VINE: f64 = 2558.1455685122073;
    const PLAIN_SCROLL: f64 = 2980.847592197458;
    const PLAIN_RAIL: f64 = 1841.9730508062225;

    /// Samples of the nearest-point wire, captured from the construction every older build draws.
    fn plain_samples(l: &CurveLayer, c: &FieldContext) -> f64 {
        let mut sum = 0.0;
        for i in 0..240 {
            for j in 0..33 {
                let uv = Uv { u: i as f64 * 0.25, v: 2.0 + j as f64 * 0.125 };
                sum += l.height(uv, c) * ((i * 31 + j * 7) % 13 + 1) as f64;
            }
        }
        sum
    }

    #[test]
    fn a_plain_wire_is_saved_and_sampled_as_before() {
        let c = ctx();
        let vine = CurveLayer::preset_vine(&c);
        let scroll = CurveLayer { mirror_v: true, taper: 0.25, ..CurveLayer::preset_scroll(&c) };
        let rail = CurveLayer::preset_wave_rail(&c);
        for l in [&vine, &scroll, &rail] {
            assert!(l.is_plain());
            let keys: std::collections::BTreeSet<String> = serde_json::to_value(l).unwrap().as_object().unwrap().keys().cloned().collect();
            let old = ["closed", "height_mm", "mirror_v", "points", "profile", "repeats_around", "taper", "width_mm"];
            assert_eq!(keys, old.iter().map(|k| k.to_string()).collect(), "a plain wire writes only the keys it always wrote");
            let d = crate::RingDesign { layers: crate::LayerStack { layers: vec![crate::LayerEntry::new("Wire", crate::Layer::Curve(l.clone()))] }, ..Default::default() };
            assert_eq!(crate::library::format_version_for(&d), crate::library::PLAIN_FORMAT_VERSION);
        }
        let got = [plain_samples(&vine, &c), plain_samples(&scroll, &c), plain_samples(&rail, &c)];
        let want = [PLAIN_VINE, PLAIN_SCROLL, PLAIN_RAIL];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-9, "a plain wire's samples moved: {got:?}");
        }
    }

    #[test]
    fn a_graded_wire_reports_its_thinnest_point_and_tapers_along_its_path() {
        let c = ctx();
        let arm = CurveLayer {
            points: vec![[0.1, 4.0], [0.4, 4.0], [0.7, 4.0], [0.9, 4.0]],
            repeats_around: 1,
            taper: 0.0,
            width_mm: 2.6,
            height_mm: 1.2,
            widths: vec![1.0, 0.6, 0.3, 0.17],
            heights: vec![1.0, 0.25],
            ..CurveLayer::default()
        };
        let f = arm.feature_footprints(&c);
        assert_eq!(f.len(), 1);
        assert!((f[0].min_feature_mm() - 2.6 * 0.17).abs() < 1e-12, "the footprint is the thinnest point: {:?}", f[0]);
        assert!((f[0].v_mm.0 - 2.7).abs() < 1e-12 && (f[0].v_mm.1 - 5.3).abs() < 1e-12, "{:?}", f[0].v_mm);
        let at = |x: f64, v: f64| arm.height(Uv { u: x * 60.0, v }, &c);
        assert!((at(0.1, 4.0) - 1.2).abs() < 1e-9, "full height at the root");
        assert!((at(0.7, 4.0) - 0.3).abs() < 1e-9, "a short list repeats its last value");
        let reach = |x: f64| (0..800).map(|k| k as f64 * 0.0025).take_while(|dv| at(x, 4.0 + dv) > 0.0).last().unwrap();
        assert!((reach(0.1) - 1.3).abs() < 0.01, "root half-width {}", reach(0.1));
        assert!((reach(0.7) - 0.39).abs() < 0.01, "third knot's half-width {}", reach(0.7));
        assert!((reach(0.9) - 0.221).abs() < 0.01, "tip half-width {}", reach(0.9));
        let mut last = f64::MAX;
        for k in 0..=80 {
            let h = at(0.1 + 0.8 * k as f64 / 80.0, 4.0);
            assert!(h <= last + 1e-9, "the spine only falls toward the tip");
            last = h;
        }
    }

    fn beaded() -> CurveLayer {
        CurveLayer {
            points: vec![[0.05, 4.0], [0.95, 4.0]],
            repeats_around: 1,
            taper: 0.0,
            width_mm: 2.0,
            height_mm: 0.8,
            widths: vec![1.0, 0.25],
            beads: Some(CurveBeads { pitch_mm: 1.0, diameter_mm: 0.8, height_mm: 0.3, offset: 0.5, graded: true, phase: 0.0, span: [0.0, 1.0], stagger: 0.0, cup: 0.0 }),
            ..CurveLayer::default()
        }
    }

    #[test]
    fn a_bead_row_stands_on_the_wire_grades_with_it_and_reports_its_smallest_bead() {
        let c = ctx();
        let arm = beaded();
        let mut even = arm.clone();
        even.beads.as_mut().unwrap().graded = false;
        assert_eq!(even.bead_census(&c), (55, 0.8), "a bead every millimetre of 54 mm, both ends included");
        let (count, smallest) = arm.bead_census(&c);
        assert!(count > 55, "the graded pitch closes up toward the tip: {count}");
        assert!((smallest - 0.8 * 0.25).abs() < 0.005, "the tip's bead: {smallest}");
        let finest = arm.feature_footprints(&c).iter().map(|f| f.min_feature_mm()).fold(f64::MAX, f64::min);
        assert!((finest - smallest).abs() < 1e-12, "the footprint names the smallest bead");
        let first = arm.height(Uv { u: 3.0, v: 4.5 }, &c);
        assert!((first - (0.4 + 0.3)).abs() < 1e-9, "the first bead adds its height to the wire's flank: {first}");
        let mut staggered = even.clone();
        let b = staggered.beads.as_mut().unwrap();
        (b.offset, b.stagger, b.span) = (0.0, 0.4, [0.0, 0.5]);
        assert_eq!(staggered.bead_census(&c).0, 28);
        let (up, down) = (staggered.height(Uv { u: 4.0, v: 4.4 }, &c), staggered.height(Uv { u: 4.0, v: 3.6 }, &c));
        assert!(down > up + 0.2, "the second bead steps to the path's right: {up} {down}");
        let mut mirrored = arm.clone();
        mirrored.mirror_v = true;
        assert_eq!(mirrored.feature_footprints(&c).len(), 4);
        let mut cupped = arm.clone();
        cupped.beads.as_mut().unwrap().cup = 0.7;
        let centre = cupped.height(Uv { u: 3.0, v: 4.5 }, &c);
        assert!((centre - (0.4 + 0.3 * 0.3)).abs() < 1e-9, "a cup dimples the bead's centre: {centre}");
        assert!(cupped.height(Uv { u: 3.12, v: 4.5 }, &c) > centre + 0.05, "and leaves a rim round it");
        assert!((cupped.bead_census(&c).1 - smallest * CUP_SPAN).abs() < 1e-12, "a cupped row's finest feature is its smallest dimple");
    }

    #[test]
    fn a_tube_stands_vertical_at_its_edge_and_is_fenced() {
        let c = ctx();
        let tube = CurveLayer { points: vec![[0.0, 4.0], [1.0, 4.0]], repeats_around: 1, taper: 0.0, width_mm: 2.0, height_mm: 1.0, profile: WireProfile::Tube, ..CurveLayer::default() };
        assert!(tube.is_plain(), "a tube alone keeps the plain construction");
        let at = |dv: f64| tube.height(Uv { u: 30.0, v: 4.0 + dv }, &c);
        assert!((at(0.0) - 1.0).abs() < 1e-9 && (at(0.6) - 0.8).abs() < 1e-9, "a semicircle: {} {}", at(0.0), at(0.6));
        assert!(at(0.999) > 0.04 && at(1.001) == 0.0, "vertical at the edge: {}", at(0.999));
        let d = crate::RingDesign { layers: crate::LayerStack { layers: vec![crate::LayerEntry::new("Arm", crate::Layer::Curve(tube.clone()))] }, ..Default::default() };
        assert_eq!(crate::library::format_version_for(&d), crate::library::FORMAT_VERSION);
        assert!(profiled_json(&serde_json::to_value(&tube).unwrap()));
    }

    #[test]
    fn a_bead_row_caps_its_count() {
        let c = ctx();
        let mut arm = beaded();
        arm.beads.as_mut().unwrap().pitch_mm = 1e-5;
        assert_eq!(arm.bead_census(&c).0, MAX_CURVE_BEADS);
        let h = arm.height(Uv { u: 30.0, v: 4.0 }, &c);
        assert!(h.is_finite() && h > 0.0);
    }

    #[test]
    fn hostile_profiles_do_not_panic() {
        let c = ctx();
        let bad = CurveBeads { pitch_mm: f64::NAN, ..CurveBeads::default() };
        let cases = [
            CurveLayer { widths: vec![f64::NAN, f64::INFINITY, -3.0, 1e30], ..beaded() },
            CurveLayer { heights: vec![f64::NAN; 70], widths: vec![], ..beaded() },
            CurveLayer { beads: Some(bad), ..beaded() },
            CurveLayer { beads: Some(CurveBeads { span: [0.9, 0.1], phase: f64::NAN, ..CurveBeads::default() }), ..beaded() },
            CurveLayer { beads: Some(CurveBeads { diameter_mm: -1.0, ..CurveBeads::default() }), ..beaded() },
            CurveLayer { points: vec![[f64::NAN, 1.0], [0.5, f64::INFINITY], [0.9, 4.0]], ..beaded() },
            CurveLayer { points: vec![[0.0, 0.0]; 10_000], closed: true, ..beaded() },
        ];
        for l in cases {
            for uv in [Uv { u: 10.0, v: 4.0 }, Uv { u: 3.0, v: 4.5 }, Uv { u: f64::INFINITY, v: f64::NAN }] {
                let h = l.height(uv, &c);
                assert!(h == 0.0 || h.is_finite());
            }
            let _ = l.feature_footprints(&c);
            assert!(l.bead_census(&c).0 <= MAX_CURVE_BEADS);
        }
    }
}
