//! Basiliscus on native escutcheon stock: a crowned serpent in profile under the chief, its jaws open on a
//! forked tongue, coiled round a flush tsavorite on a pitted field inside a beaded bordure, with feathered
//! mantling pouring over the shoulders into serpent scale and shingled belly scutes across the palm.
use anyhow::{Result, anyhow, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, Mesh, ProfileStyle, RingDesign,
    castability::{self, CastProcess, Verdict},
    cad::{self, Attach, Component, Document, Feature, Operation, Placement, Stage},
    csg,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle, Window, smoothstep},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    manufacturing as mf,
    mesh::{self, BuildResult},
    render::{self, Part},
    reptile, sculpt::{self, ellipsoid, round_cone, smax, smin},
    setting::{self, SolidKind},
    skin::{self, Atlas, Hide, Sample},
};
use serde_json::{Value, json};
use std::{f64::consts::PI, path::Path, time::Instant};

const AW: usize = 2048;
const HACKLE_HEIGHT: f64 = 0.75;
const BELLY_HEIGHT: f64 = 0.34;
/// Scale of the painted serpent layer, mm.
const SERPENT_HEIGHT: f64 = 2.0;
/// Depth of the punched pits, mm.
const PIT_DEPTH: f64 = 0.09;
/// The bordure's beads, mm.
const BEAD_HEIGHT: f64 = 0.26;
/// The boss the tsavorite is set flush in, mm over the field.
const STONE_BOSS: f64 = 0.9;
/// Relief sampling of the export build.
const EXPORT_THETA: usize = 1536;
/// Mask confining the hide's detail to where it paints: clear of the lower walls by the bore edges.
const REACH: &str = "Relief reach";
/// The investment land every proud feature meets or is named against, mm.
const LAND_FLOOR: f64 = 0.8;

type P2 = [f64; 2];

fn draft_params() -> BuildParams {
    BuildParams {
        theta_steps: 768,
        profile_steps: 320,
        refine: None,
        ..Default::default()
    }
}

fn export_params() -> BuildParams {
    BuildParams {
        theta_steps: EXPORT_THETA,
        profile_steps: 448,
        refine: None,
        ..Default::default()
    }
}

fn coarse_params() -> BuildParams {
    BuildParams {
        theta_steps: 384,
        profile_steps: 192,
        refine: None,
        ..Default::default()
    }
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (6.0 * t - 15.0))
}

fn window(centre: f64, span: f64) -> Window {
    let mut w = Window::around(centre, span);
    w.fade_deg = 6.0;
    w
}

fn base() -> Result<RingDesign> {
    let mut d = RingDesign::default();
    let source = PRESETS.iter().find(|p| p.id == "020").unwrap().load()?;
    ImportedBase::attach(&mut d, source)?;
    d.name = "Basiliscus — king of serpents".into();
    d.imported_base.as_mut().unwrap().sand_envelope = false;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 16.5;
    d.shank.head.length_mm = 16.0;
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    // Crisp relief stays off: on this unmirrored stock it outlines the wall scales on the right-hand wall.
    d.crisp_relief = false;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart {
        profile: d.profile,
        bore_radius_mm: d.inner_radius_mm(),
    });
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = 0.8;
    d.draft.min_detail_mm = 0.15;
    d.draft.min_draft_deg = 0.0;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    Ok(d)
}

fn setup(d: &RingDesign) -> mf::Setup {
    let mut setup = mf::Setup::from_design(d);
    setup.recipe.name = "Basiliscus / Silver 925 investment".into();
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925")
        .unwrap()
        .shrink_pct;
    setup.sample_pitch_mm = 0.1;
    setup.bench_notes = "Investment pattern with the flush marquise seat, the crowned head and every scale plate cast in place. Chase the barbs and keel lines at the bench, finish the bearing to the measured tsavorite, burnish the lip, and polish the crown, the brows and the reserved margins.".into();
    setup
}

/// A closed polyline at equal steps no longer than `step`.
fn resample(poly: &[P2], step: f64) -> Vec<P2> {
    let n = poly.len();
    let mut cum = vec![0.0];
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        cum.push(cum[i] + (a[0] - b[0]).hypot(a[1] - b[1]));
    }
    let total = cum[n];
    let m = (total / step).ceil().max(3.0) as usize;
    let mut j = 0;
    (0..m)
        .map(|k| {
            let s = total * k as f64 / m as f64;
            while j + 1 < n && cum[j + 1] < s {
                j += 1;
            }
            let (a, b) = (poly[j], poly[(j + 1) % n]);
            let t = (s - cum[j]) / (cum[j + 1] - cum[j]).max(1e-12);
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        })
        .collect()
}

fn area(poly: &[P2]) -> f64 {
    let n = poly.len();
    0.5 * (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
}

/// Counter-clockwise in the (x, z) plane.
fn ccw(mut poly: Vec<P2>) -> Vec<P2> {
    if area(&poly) < 0.0 {
        poly.reverse();
    }
    poly
}

/// A closed outline on the face in world millimetres, `x` round the ring and `z` along the finger, with its box.
struct Shape {
    poly: Vec<P2>,
    lo: P2,
    hi: P2,
}

impl Shape {
    fn new(poly: Vec<P2>) -> Self {
        let lo = [poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min), poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min)];
        let hi = [poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max), poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max)];
        Self { poly, lo, hi }
    }

    /// Distance to the outline, positive inside, `-cap` anywhere further than `cap` outside its box.
    fn sdf(&self, p: P2, cap: f64) -> f64 {
        if p[0] < self.lo[0] - cap || p[0] > self.hi[0] + cap || p[1] < self.lo[1] - cap || p[1] > self.hi[1] + cap {
            return -cap;
        }
        let d = poly_dist(&self.poly, p);
        if inside(&self.poly, p) { d } else { -d.min(cap) }
    }
}

/// Distance from `p` to a closed polyline.
fn poly_dist(poly: &[P2], p: P2) -> f64 {
    let n = poly.len();
    (0..n).map(|i| seg_dist(p, poly[i], poly[(i + 1) % n]).0).fold(f64::MAX, f64::min)
}

/// Whether `p` lies inside a closed polyline, by crossings.
fn inside(poly: &[P2], p: P2) -> bool {
    let n = poly.len();
    let mut odd = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

/// Whether ring angle `theta` lies within `half` degrees of `centre`.
fn within(theta: f64, centre: f64, half: f64) -> bool {
    (theta - centre + 180.0).rem_euclid(360.0) - 180.0 <= half && (theta - centre + 180.0).rem_euclid(360.0) - 180.0 >= -half
}

fn centroid(poly: &[P2]) -> P2 {
    let n = poly.len() as f64;
    [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n]
}

/// The tsavorite's centre on the face; its long axis runs round the ring.
const STONE: P2 = [-0.2, 2.95];
/// The seat's plan: semi-axes round the ring and across, and the superellipse's power. Rounder than the marquise's own
/// 1.5, so the wall round its girdle holds 0.85 mm at the shoulders as well as at the ends and sides.
const SEAT_PLAN: (f64, f64, f64) = (4.88, 2.88, 1.8);
/// How far the coil laps onto the seat's rim, mm.
const LAP: f64 = 0.52;
/// The serpent's spine from the throat, down the right of the seat and round it, lapping its rim, to the tail.
const SPINE: [P2; 17] = [
    [2.0, -1.3],
    [3.35, -1.2],
    [4.25, -0.75],
    [4.72, 0.25],
    [4.9, 1.7],
    [4.97, 2.86],
    [4.67, 3.75],
    [3.88, 4.6],
    [2.7, 5.28],
    [1.3, 5.71],
    [-0.08, 5.85],
    [-1.36, 5.68],
    [-2.74, 5.24],
    [-3.89, 4.56],
    [-4.67, 3.76],
    [-4.96, 3.01],
    [-4.77, 2.32],
];
/// Half-width of the body at the neck and at the tail's tip, mm.
const GIRTH: (f64, f64) = (1.25, 0.4);
/// The body's crown over its plinth, and the relief of its scales, mm.
const DOME: f64 = 1.4;
const SCALE_RELIEF: f64 = 0.24;

/// Open centripetal Catmull-Rom spline through `ctrl`, `per` samples a span.
fn open_spline(ctrl: &[P2], per: usize) -> Vec<P2> {
    let n = ctrl.len();
    let ext = |i: isize| -> P2 {
        if i < 0 {
            [2.0 * ctrl[0][0] - ctrl[1][0], 2.0 * ctrl[0][1] - ctrl[1][1]]
        } else if i as usize >= n {
            [2.0 * ctrl[n - 1][0] - ctrl[n - 2][0], 2.0 * ctrl[n - 1][1] - ctrl[n - 2][1]]
        } else {
            ctrl[i as usize]
        }
    };
    let gap = |a: P2, b: P2| (a[0] - b[0]).hypot(a[1] - b[1]).sqrt().max(1e-6);
    let mut out = Vec::with_capacity(n * per);
    for i in 0..n - 1 {
        let p = [ext(i as isize - 1), ext(i as isize), ext(i as isize + 1), ext(i as isize + 2)];
        let t1 = gap(p[0], p[1]);
        let t2 = t1 + gap(p[1], p[2]);
        let t3 = t2 + gap(p[2], p[3]);
        for k in 0..per {
            let t = t1 + (t2 - t1) * k as f64 / per as f64;
            let mix = |a: P2, b: P2, ta: f64, tb: f64| {
                let u = (t - ta) / (tb - ta);
                [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]
            };
            let a1 = mix(p[0], p[1], 0.0, t1);
            let a2 = mix(p[1], p[2], t1, t2);
            let a3 = mix(p[2], p[3], t2, t3);
            out.push(mix(mix(a1, a2, 0.0, t2), mix(a2, a3, t1, t3), t1, t2));
        }
    }
    out.push(ctrl[n - 1]);
    out
}

/// The serpent's body as a spine with arc length, for painting.
struct Body {
    spine: Vec<P2>,
    at: Vec<f64>,
    /// How many scales down the spine each vertex lies: the integral of one over the scale's length, which shrinks with
    /// the girth, so the rows stay continuous as the body tapers.
    phase: Vec<f64>,
    lo: P2,
    hi: P2,
}

/// A dorsal scale's width across the body, as a share of the girth, and its bounds, mm; and its length over its width.
const SCALE_CELL: (f64, f64, f64, f64) = (0.36, 0.25, 0.55, 1.37);

impl Body {
    fn new() -> Self {
        let spine = open_spline(&SPINE, 120);
        let mut at = vec![0.0];
        for w in spine.windows(2) {
            at.push(at.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
        }
        let lo = [spine.iter().map(|p| p[0]).fold(f64::MAX, f64::min) - GIRTH.0, spine.iter().map(|p| p[1]).fold(f64::MAX, f64::min) - GIRTH.0];
        let hi = [spine.iter().map(|p| p[0]).fold(f64::MIN, f64::max) + GIRTH.0, spine.iter().map(|p| p[1]).fold(f64::MIN, f64::max) + GIRTH.0];
        let mut me = Self { spine, at, phase: Vec::new(), lo, hi };
        me.phase = vec![0.0];
        for w in me.at.windows(2) {
            let step = (w[1] - w[0]) / (SCALE_CELL.3 * me.scale_width(0.5 * (w[0] + w[1])));
            me.phase.push(me.phase.last().unwrap() + step);
        }
        me
    }

    /// A dorsal scale's width across the body at `s` along the spine, mm: a fixed share of the girth, so every station
    /// carries the same rows, bounded so the neck's are not coarse and the tail's stay above the detail floor.
    fn scale_width(&self, s: f64) -> f64 {
        (SCALE_CELL.0 * self.girth(s)).clamp(SCALE_CELL.1, SCALE_CELL.2)
    }

    /// Scales counted down the spine to `s`.
    fn phase_at(&self, s: f64) -> f64 {
        let i = self.at.partition_point(|&a| a < s).clamp(1, self.at.len() - 1);
        let (a, b) = (self.at[i - 1], self.at[i]);
        let t = ((s - a) / (b - a).max(1e-12)).clamp(0.0, 1.0);
        self.phase[i - 1] + (self.phase[i] - self.phase[i - 1]) * t
    }

    fn length(&self) -> f64 {
        *self.at.last().unwrap()
    }

    fn girth(&self, s: f64) -> f64 {
        GIRTH.1 + (GIRTH.0 - GIRTH.1) * (1.0 - (s / self.length()).clamp(0.0, 1.0)).powi(2)
    }

    /// Distance to the spine, arc length at the nearest spine point, and the signed offset across it.
    fn nearest(&self, p: P2) -> Option<(f64, f64, f64)> {
        if p[0] < self.lo[0] || p[0] > self.hi[0] || p[1] < self.lo[1] || p[1] > self.hi[1] {
            return None;
        }
        let mut best = (f64::MAX, 0.0, 0.0);
        for i in 0..self.spine.len() - 1 {
            let (a, b) = (self.spine[i], self.spine[i + 1]);
            let (d, t) = seg_dist(p, a, b);
            if d < best.0 {
                let e = [b[0] - a[0], b[1] - a[1]];
                let l = e[0].hypot(e[1]).max(1e-12);
                let side = ((p[0] - a[0]) * e[1] - (p[1] - a[1]) * e[0]) / l;
                best = (d, self.at[i] + t * l, side);
            }
        }
        Some(best)
    }

    /// As [`Body::nearest`], over the spine's first `run` mm only.
    fn nearest_until(&self, p: P2, run: f64) -> Option<(f64, f64, f64)> {
        let mut best = (f64::MAX, 0.0, 0.0);
        let end = self.at.partition_point(|&a| a < run).min(self.spine.len() - 1);
        for i in 0..end {
            let (a, b) = (self.spine[i], self.spine[i + 1]);
            let (d, t) = seg_dist(p, a, b);
            if d < best.0 {
                let e = [b[0] - a[0], b[1] - a[1]];
                let l = e[0].hypot(e[1]).max(1e-12);
                let side = ((p[0] - a[0]) * e[1] - (p[1] - a[1]) * e[0]) / l;
                best = (d, self.at[i] + t * l, side);
            }
        }
        (best.0 < f64::MAX).then_some(best)
    }

    /// The body's crest height and its steepest flank over the outer quarter of the girth, degrees, both without the scales.
    fn profile(&self) -> (f64, f64) {
        let (mut crest, mut steep): (f64, f64) = (0.0, 0.0);
        for i in (1..self.spine.len() - 1).step_by(4) {
            let (a, b) = (self.spine[i - 1], self.spine[i + 1]);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
            let n = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
            let w = self.girth(self.at[i]);
            let at = |d: f64| self.height([self.spine[i][0] + n[0] * d, self.spine[i][1] + n[1] * d], false);
            crest = crest.max(at(0.0));
            for k in 0..20 {
                let d = w * (0.75 + 0.25 * k as f64 / 20.0);
                let h = 0.0005;
                let own = |d: f64| self.nearest([self.spine[i][0] + n[0] * d, self.spine[i][1] + n[1] * d]).is_some_and(|(_, s, _)| (s - self.at[i]).abs() < 0.5);
                if own(d - h) && own(d + h) {
                    steep = steep.max(((at(d - h) - at(d + h)) / (2.0 * h)).abs().atan().to_degrees());
                }
            }
        }
        (crest, steep)
    }

    /// Distance outside the body's edge, mm; negative inside it.
    fn clear(&self, p: P2) -> f64 {
        self.nearest(p).map_or(f64::MAX, |(d, s, _)| d - self.girth(s))
    }

    /// Painted body height, mm: a raised-cosine crown over a low plinth that gives the edge its line, and with `skin` the
    /// keeled dorsal scales, sized to the girth, at full relief over the inner seven tenths of the half-girth and faded out
    /// by nine tenths. The belly lies on the field; its scutes are on the palm.
    fn height(&self, p: P2, skin: bool) -> f64 {
        let Some((d, s, lat)) = self.nearest(p) else { return 0.0 };
        let w = self.girth(s);
        if d >= w {
            return 0.0;
        }
        let plinth = 0.08 * (w / 0.8).min(1.0) * smoothstep(0.0, (0.6 * w).min(0.3), w - d);
        let x = d / w;
        let dome = DOME.min(1.1 * w) * 0.5 * (1.0 + (PI * x).cos());
        if !skin {
            return plinth + 0.9 * dome;
        }
        let scales = reptile::snake(self.phase_at(s), lat / self.scale_width(s));
        plinth + 0.9 * dome + SCALE_RELIEF * scales * (1.0 - smooth(0.7, 0.9, x))
    }
}

/// Distance from `p` to the segment `a`–`b`, and how far along it the nearest point lies.
fn seg_dist(p: P2, a: P2, b: P2) -> (f64, f64) {
    let e = [b[0] - a[0], b[1] - a[1]];
    let w = [p[0] - a[0], p[1] - a[1]];
    let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-18)).clamp(0.0, 1.0);
    ((w[0] - e[0] * t).hypot(w[1] - e[1] * t), t)
}

/// The table's outline in world `(x, z)`, row by row along the finger.
fn table_outline(a: &Atlas) -> Vec<P2> {
    let mut rows = vec![(f64::MAX, f64::MIN); 400];
    for s in &a.samples {
        if s.n[1] > 0.97 && s.p[1] > a.top - 0.05 {
            let k = ((s.p[2] + 10.0) / 0.05).floor().clamp(0.0, 399.0) as usize;
            rows[k] = (rows[k].0.min(s.p[0]), rows[k].1.max(s.p[0]));
        }
    }
    let rows: Vec<(f64, f64, f64)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.0 < r.1)
        .map(|(k, r)| (-10.0 + 0.05 * (k as f64 + 0.5), r.0, r.1))
        .collect();
    let mut poly: Vec<P2> = rows.iter().map(|r| [r.1, r.0]).collect();
    poly.extend(rows.iter().rev().map(|r| [r.2, r.0]));
    ccw(resample(&poly, 0.09))
}

/// An open polyline with its arc length, for measuring along a lip or a plate's edge.
struct Line {
    pts: Vec<P2>,
    at: Vec<f64>,
    lo: P2,
    hi: P2,
}

impl Line {
    fn new(pts: Vec<P2>) -> Self {
        let mut at = vec![0.0];
        for w in pts.windows(2) {
            at.push(at.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
        }
        let lo = [pts.iter().map(|p| p[0]).fold(f64::MAX, f64::min), pts.iter().map(|p| p[1]).fold(f64::MAX, f64::min)];
        let hi = [pts.iter().map(|p| p[0]).fold(f64::MIN, f64::max), pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max)];
        Self { pts, at, lo, hi }
    }

    fn length(&self) -> f64 {
        *self.at.last().unwrap()
    }

    /// Distance to the line and the arc length at its nearest point, or `None` further than `cap` from its box.
    fn nearest(&self, p: P2, cap: f64) -> Option<(f64, f64)> {
        if p[0] < self.lo[0] - cap || p[0] > self.hi[0] + cap || p[1] < self.lo[1] - cap || p[1] > self.hi[1] + cap {
            return None;
        }
        let mut best = (f64::MAX, 0.0);
        for i in 0..self.pts.len() - 1 {
            let (d, t) = seg_dist(p, self.pts[i], self.pts[i + 1]);
            if d < best.0 {
                best = (d, self.at[i] + t * (self.at[i + 1] - self.at[i]));
            }
        }
        Some(best)
    }
}

/// Punched pits: radius and the even ground's pitch, mm (the third is the ground's pitch again).
const PIT: (f64, f64, f64) = (0.21, 0.62, 0.62);
/// The polished halo kept round every arm, mm past a pit's rim.
const PIT_HALO: f64 = 0.22;
/// The bordure's beads: inset from the table's edge, pitch and radius, mm.
const BEADS: (f64, f64, f64) = (0.5, 0.62, 0.22);
/// The seat's skirt beyond its rim, mm.
const SKIRT: f64 = 0.45;

/// The head's footprint on the table: how far outside the sculpted head (and its tongue and neck) each face point lies,
/// mm, negative under it, on a grid over the head's box; and the iso-line the field's first row of pits follows.
struct Footprint {
    lo: P2,
    step: f64,
    n: [usize; 2],
    d: Vec<f32>,
}

impl Footprint {
    /// From the sculpt's mesh: every face standing over the table marks the cells it covers, and each cell's distance to
    /// the nearest marked cell (or, under the head, to the nearest unmarked one) is measured exactly within 2.5 mm.
    fn of(head: &csg::Solid, table: f64) -> Self {
        let step = 0.04;
        let lo = [-9.0, -8.0];
        let n = [((18.0) / step) as usize, ((12.5) / step) as usize];
        let mut on = vec![false; n[0] * n[1]];
        let cell = |x: f64, z: f64| -> Option<usize> {
            let (i, j) = (((x - lo[0]) / step).floor(), ((z - lo[1]) / step).floor());
            (i >= 0.0 && j >= 0.0 && (i as usize) < n[0] && (j as usize) < n[1]).then(|| j as usize * n[0] + i as usize)
        };
        for t in &head.f {
            let [p, q, r] = t.map(|i| head.v[i as usize]);
            if p[1].min(q[1]).min(r[1]) < table + 0.02 {
                continue;
            }
            let (x0, x1) = (p[0].min(q[0]).min(r[0]), p[0].max(q[0]).max(r[0]));
            let (z0, z1) = (p[2].min(q[2]).min(r[2]), p[2].max(q[2]).max(r[2]));
            let mut z = (z0 / step).floor() * step + 0.5 * step;
            while z <= z1 + step {
                let mut x = (x0 / step).floor() * step + 0.5 * step;
                while x <= x1 + step {
                    let (a, b, c) = ([p[0], p[2]], [q[0], q[2]], [r[0], r[2]]);
                    let e = |u: P2, v: P2| (v[0] - u[0]) * (z - u[1]) - (v[1] - u[1]) * (x - u[0]);
                    let (s0, s1, s2) = (e(a, b), e(b, c), e(c, a));
                    let near = [a, b, c].iter().any(|v| (v[0] - x).hypot(v[1] - z) < 0.75 * step);
                    if near || (s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) || (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0) {
                        if let Some(k) = cell(x, z) {
                            on[k] = true;
                        }
                    }
                    x += step;
                }
                z += step;
            }
        }
        // Close pinholes the triangles' rasterisation left.
        for _ in 0..2 {
            let prev = on.clone();
            for j in 1..n[1] - 1 {
                for i in 1..n[0] - 1 {
                    let k = j * n[0] + i;
                    if !prev[k] && [k - 1, k + 1, k - n[0], k + n[0]].iter().filter(|&&m| prev[m]).count() >= 3 {
                        on[k] = true;
                    }
                }
            }
        }
        let reach = (2.5 / step).ceil() as i64;
        let edge: Vec<usize> = (0..on.len())
            .filter(|&k| {
                let (i, j) = (k % n[0], k / n[0]);
                on[k] && (i == 0 || j == 0 || i + 1 == n[0] || j + 1 == n[1] || !on[k - 1] || !on[k + 1] || !on[k - n[0]] || !on[k + n[0]])
            })
            .collect();
        let mut bins: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        for &k in &edge {
            bins.entry(((k % n[0]) as i64 / 16, (k / n[0]) as i64 / 16)).or_default().push(k);
        }
        let d: Vec<f32> = (0..on.len())
            .map(|k| {
                let (i, j) = ((k % n[0]) as i64, (k / n[0]) as i64);
                let mut best = f64::MAX;
                for bi in (i - reach) / 16 - 1..=(i + reach) / 16 + 1 {
                    for bj in (j - reach) / 16 - 1..=(j + reach) / 16 + 1 {
                        for &m in bins.get(&(bi, bj)).map_or(&[][..], |v| v.as_slice()) {
                            let (mi, mj) = ((m % n[0]) as i64, (m / n[0]) as i64);
                            best = best.min((((mi - i) * (mi - i) + (mj - j) * (mj - j)) as f64).sqrt());
                        }
                    }
                }
                let dist = (best * step).min(2.5);
                (if on[k] { -dist } else { dist + 0.5 * step }) as f32
            })
            .collect();
        Self { lo, step, n, d }
    }

    /// Distance outside the head's footprint at face point `p`, mm; negative under it.
    fn at(&self, p: P2) -> f64 {
        let f = [((p[0] - self.lo[0]) / self.step - 0.5).clamp(0.0, (self.n[0] - 2) as f64), ((p[1] - self.lo[1]) / self.step - 0.5).clamp(0.0, (self.n[1] - 2) as f64)];
        let (i, j) = (f[0].floor() as usize, f[1].floor() as usize);
        let (tx, tz) = (f[0] - i as f64, f[1] - j as f64);
        let g = |a: usize, b: usize| self.d[(j + b) * self.n[0] + i + a] as f64;
        let v = (g(0, 0) * (1.0 - tx) + g(1, 0) * tx) * (1.0 - tz) + (g(0, 1) * (1.0 - tx) + g(1, 1) * tx) * tz;
        let outside = p[0] < self.lo[0] || p[1] < self.lo[1] || p[0] > self.lo[0] + self.step * self.n[0] as f64 || p[1] > self.lo[1] + self.step * self.n[1] as f64;
        if outside { v.max(2.5) } else { v }
    }

    /// Points `by` outside the footprint, about `pitch` apart, in order round the head's centre.
    fn row(&self, by: f64, pitch: f64) -> Vec<P2> {
        let mut pts: Vec<P2> = Vec::new();
        for j in 0..self.n[1] {
            for i in 0..self.n[0] {
                let p = [self.lo[0] + (i as f64 + 0.5) * self.step, self.lo[1] + (j as f64 + 0.5) * self.step];
                if (self.at(p) - by).abs() < 0.5 * self.step {
                    pts.push(p);
                }
            }
        }
        let c = centroid(&pts);
        pts.sort_by(|a, b| (a[1] - c[1]).atan2(a[0] - c[0]).total_cmp(&(b[1] - c[1]).atan2(b[0] - c[0])));
        let mut out: Vec<P2> = Vec::new();
        for p in pts {
            if out.iter().all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) >= pitch) {
                out.push(p);
            }
        }
        out
    }
}

/// The arms on the shield: the sculpted crowned head, and the serpent's painted body coiled round the tsavorite on a
/// pounced field inside a beaded bordure.
struct Arms {
    table: Shape,
    body: Body,
    head: Footprint,
    boss: Shape,
    beads: Vec<P2>,
    pits: Vec<P2>,
    grid: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl Arms {
    fn new(table: Vec<P2>, head: Footprint) -> Result<Self> {
        let (ra, rb, pw) = SEAT_PLAN;
        let rim: Vec<P2> = (0..720)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / 720.0;
                let (c, s) = (t.cos(), t.sin());
                [STONE[0] + ra * c.signum() * c.abs().powf(2.0 / pw), STONE[1] + rb * s.signum() * s.abs().powf(2.0 / pw)]
            })
            .collect();
        let rim = ccw(resample(&rim, 0.05));
        let boss = Shape::new(rim);
        let table = Shape::new(table);
        let mut arms = Self { table, body: Body::new(), head, boss, beads: Vec::new(), pits: Vec::new(), grid: Default::default() };
        arms.beads = arms.bordure();
        arms.pounce();
        Ok(arms)
    }

    /// The table's half-width round the ring at `z`, mm.
    fn table_width_at(&self, z: f64) -> f64 {
        let poly = &self.table.poly;
        let n = poly.len();
        let mut best: f64 = 0.0;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if (a[1] > z) != (b[1] > z) {
                best = best.max((a[0] + (z - a[1]) / (b[1] - a[1]) * (b[0] - a[0])).abs());
            }
        }
        best
    }

    /// Distance outside every arm on the field, mm: the sculpted head, the body and the seat's skirt.
    fn clear(&self, p: P2) -> f64 {
        self.head.at(p).min(self.body.clear(p)).min(-self.boss.sdf(p, 2.0) - SKIRT)
    }

    /// The serpent's body painted over the table, mm. Over the seat's rim it lies on the seat's own surface, `pad`
    /// there, so the coil laps over the setting.
    fn serpent(&self, p: P2, pad: f64) -> f64 {
        if self.table.sdf(p, 1.0) <= 0.0 {
            return 0.0;
        }
        let lie = self.body.height(p, true);
        if lie > 0.0 { lie + pad * smooth(0.0, 0.03, lie) } else { 0.0 }
    }

    /// Bead centres a fixed inset inside the table's edge at an even pitch, less any that would touch the arms.
    fn bordure(&self) -> Vec<P2> {
        let (inset, pitch, radius) = BEADS;
        let poly = &self.table.poly;
        let n = poly.len();
        let mut line = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b) = (poly[(i + n - 1) % n], poly[(i + 1) % n]);
            let t = [b[0] - a[0], b[1] - a[1]];
            let l = t[0].hypot(t[1]).max(1e-12);
            let q = [poly[i][0] - t[1] / l * inset, poly[i][1] + t[0] / l * inset];
            if (self.table.sdf(q, 1.0) - inset).abs() < 0.03 {
                line.push(q);
            }
        }
        let mut beads: Vec<P2> = Vec::new();
        let mut run = 0.0;
        for w in line.windows(2) {
            let step = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
            if step > 0.3 {
                continue;
            }
            run += step;
            let spaced = beads.last().is_none_or(|b| (b[0] - w[1][0]).hypot(b[1] - w[1][1]) > 0.95 * pitch);
            if run >= pitch && spaced {
                run = 0.0;
                if self.clear(w[1]) > radius + 0.12 {
                    beads.push(w[1]);
                }
            }
        }
        if let (Some(a), Some(b)) = (beads.first().copied(), beads.last().copied()) {
            if beads.len() > 1 && (a[0] - b[0]).hypot(a[1] - b[1]) < 0.95 * pitch {
                beads.pop();
            }
        }
        beads
    }

    /// The beaded bordure, 0..1: a rounded bead at every bordure station.
    fn bordure_height(&self, p: P2) -> f64 {
        let r = BEADS.2;
        self.beads
            .iter()
            .map(|b| {
                let d = (p[0] - b[0]).hypot(p[1] - b[1]);
                if d < r { (1.0 - (d / r).powi(2)).max(0.0).sqrt() } else { 0.0 }
            })
            .fold(0.0, f64::max)
    }

    /// Whether a pit centred at `q` stays on the field: inside the bordure, clear of the arms by `gap`.
    fn pit_fits(&self, q: P2, gap: f64) -> bool {
        let (r, _, _) = PIT;
        self.table.sdf(q, 3.0) > BEADS.0 + BEADS.2 + 0.12 + r && self.clear(q) >= r + gap
    }

    /// Lays the punched ground: a row of pits following every arm's outline, then a jittered grain filling the field.
    fn pounce(&mut self) {
        // One even ground over the whole open field on a jittered staggered lattice, with a polished halo round every
        // arm and no rows tracing their outlines.
        let (_, pitch, _) = PIT;
        let mut pits: Vec<P2> = Vec::new();
        let [lo, hi] = [self.table.lo, self.table.hi];
        let row = pitch * 0.866;
        let (i0, i1) = ((lo[0] / pitch).floor() as i64 - 1, (hi[0] / pitch).ceil() as i64 + 1);
        let (j0, j1) = ((lo[1] / row).floor() as i64 - 1, (hi[1] / row).ceil() as i64 + 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let stagger = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
                let q = [
                    (i as f64 + stagger + 0.06 * (skin::hash(i, j) - 0.5)) * pitch,
                    j as f64 * row + 0.06 * pitch * (skin::hash(j + 7919, i) - 0.5),
                ];
                if self.pit_fits(q, PIT_HALO) {
                    pits.push(q);
                }
            }
        }
        let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        for (k, q) in pits.iter().enumerate() {
            grid.entry(((q[0] / 0.5).floor() as i64, (q[1] / 0.5).floor() as i64)).or_default().push(k);
        }
        self.pits = pits;
        self.grid = grid;
    }

    /// Distance from `p` to the nearest pit's rim, mm.
    fn to_pit(&self, p: P2) -> f64 {
        let (ci, cj) = ((p[0] / 0.5).floor() as i64, (p[1] / 0.5).floor() as i64);
        let mut best = f64::MAX;
        for ring in 1..8 {
            for i in ci - ring..=ci + ring {
                for j in cj - ring..=cj + ring {
                    for &k in self.grid.get(&(i, j)).map_or(&[][..], |v| v.as_slice()) {
                        let q = self.pits[k];
                        best = best.min((p[0] - q[0]).hypot(p[1] - q[1]) - PIT.0);
                    }
                }
            }
            if best < 0.5 * ring as f64 - 0.5 {
                break;
            }
        }
        best
    }

    /// How close the ground's pits come: the widest bare stretch of field, and the furthest any arm's edge lies from a pit, mm.
    fn pounce_reach(&self) -> (f64, f64, f64) {
        let (lo, hi) = (self.table.lo, self.table.hi);
        let open = |q: P2| self.pit_fits(q, 0.09);
        let mut bare: f64 = 0.0;
        let mut z = lo[1];
        while z < hi[1] {
            let mut x = lo[0];
            while x < hi[0] {
                if open([x, z]) {
                    bare = bare.max(self.to_pit([x, z]));
                }
                x += 0.05;
            }
            z += 0.05;
        }
        let mut edge: f64 = 0.0;
        let (mut near, mut all) = (0usize, 0usize);
        let mut probe = |q: P2, into: P2| {
            if open(into) {
                let d = self.to_pit(q) + 0.12;
                all += 1;
                near += usize::from(d <= 0.4);
                edge = edge.max(d);
            }
        };
        // Round the head the outward direction is the footprint's own gradient.
        for p in self.head.row(0.0, 0.1) {
            let g = [self.head.at([p[0] + 0.02, p[1]]) - self.head.at([p[0] - 0.02, p[1]]), self.head.at([p[0], p[1] + 0.02]) - self.head.at([p[0], p[1] - 0.02])];
            let l = g[0].hypot(g[1]).max(1e-12);
            let n = [g[0] / l, g[1] / l];
            probe([p[0] + n[0] * 0.12, p[1] + n[1] * 0.12], [p[0] + n[0] * (0.12 + PIT.0), p[1] + n[1] * (0.12 + PIT.0)]);
        }
        let poly = resample(&self.boss.poly, 0.1);
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[(i + n - 1) % n], poly[(i + 1) % n]);
            let t = [b[0] - a[0], b[1] - a[1]];
            let l = t[0].hypot(t[1]).max(1e-12);
            let by = SKIRT + 0.12;
            let q = [poly[i][0] + t[1] / l * by, poly[i][1] - t[0] / l * by];
            probe(q, [q[0] + t[1] / l * PIT.0, q[1] - t[0] / l * PIT.0]);
        }
        (bare, edge, near as f64 / all.max(1) as f64)
    }

    /// The pounced ground sunk into the table, 0..1 of the pits' depth.
    fn pounce_depth(&self, p: P2) -> f64 {
        let (r, _, _) = PIT;
        let (ci, cj) = ((p[0] / 0.5).floor() as i64, (p[1] / 0.5).floor() as i64);
        let mut pit: f64 = 0.0;
        for i in ci - 1..=ci + 1 {
            for j in cj - 1..=cj + 1 {
                for &k in self.grid.get(&(i, j)).map_or(&[][..], |v| v.as_slice()) {
                    let q = self.pits[k];
                    let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                    pit = pit.max((1.0 - (d / r).powi(2)).max(0.0).powf(0.6));
                }
            }
        }
        pit
    }
}

/// The chart point of the face point at world `(x, z)`.
fn chart(a: &Atlas, p: P2) -> Result<(f64, f64)> {
    let theta = a.top.atan2(p[0]).to_degrees();
    let fx = theta / 360.0 * a.width as f64;
    let col = |x: usize| -> Option<f64> {
        (1..a.height - 2).find_map(|y| {
            let (s, t) = (a.at(x, y), a.at(x, y + 1));
            let on = s.n[1] > 0.95 && t.n[1] > 0.95 && s.p[1] > a.top - 0.05 && t.p[1] > a.top - 0.05;
            (on && (s.p[2] - p[1]) * (t.p[2] - p[1]) <= 0.0 && s.p[2] != t.p[2])
                .then(|| s.v + (t.v - s.v) * (p[1] - s.p[2]) / (t.p[2] - s.p[2]))
        })
    };
    let x0 = fx.floor() as usize % a.width;
    let tx = fx - fx.floor();
    let v = match (col(x0), col((x0 + 1) % a.width)) {
        (Some(v0), Some(v1)) => v0 + (v1 - v0) * tx,
        (Some(v), None) | (None, Some(v)) => v,
        _ => anyhow::bail!("({:.2}, {:.2}) is off the table", p[0], p[1]),
    };
    let q = a.point(theta, v);
    ensure!(
        (q[0] - p[0]).hypot(q[2] - p[1]) < 0.03,
        "Chart point for ({:.2}, {:.2}) lands at ({:.3}, {:.3})",
        p[0],
        p[1],
        q[0],
        q[2]
    );
    Ok((theta, v))
}

/// The outer surface's half-width the mantling's rows are laid in: every column's own rim maps onto it, mm.
const PLUME_WIDTH: f64 = 3.0;

/// How far a hackle's shaft bows away from the pale by its tip, as a share of its length: a 10 degree sickle.
const SICKLE: f64 = 0.1;

/// The mantling's feathers: lanceolate hackles that shorten into keeled scales between two distances along the ring.
/// A feather's place along its row is read from the integral of one over the row's step, so rows shorten smoothly
/// instead of crowding into ribs where the step changes fastest.
struct Hackles {
    l70: f64,
    l110: f64,
    phase: Vec<Vec<f64>>,
}

/// Grid of the hackles' phase tables: offset of the table's edge along the ring, then distance past it, mm.
const HACKLE_GRID: (f64, usize, f64, usize) = (0.25, 64, 0.02, 3000);

impl Hackles {
    fn new(l70: f64, l110: f64) -> Self {
        let (off_step, offs, r_step, rs) = HACKLE_GRID;
        let mut me = Self { l70, l110, phase: Vec::new() };
        me.phase = (0..=offs)
            .map(|k| {
                let off = k as f64 * off_step;
                let mut acc = 0.0;
                let mut table = Vec::with_capacity(rs + 1);
                for i in 0..=rs {
                    table.push(acc);
                    let r = (i as f64 + 0.5) * r_step;
                    acc += r_step / me.shape(r, smooth(me.l70, me.l110, r + off)).2;
                }
                table
            })
            .collect();
        me
    }

    /// Row pitch across, feather length and the step between feathers along a row, mm: long hackles, better than four
    /// to one, shortening into scales through the morph.
    fn shape(&self, root: f64, morph: f64) -> (f64, f64, f64) {
        let pitch = (1.35 + 0.35 * smooth(0.0, 8.0, root)) * (1.0 - morph) + 1.2 * morph;
        let length = pitch * (4.6 * (1.0 - morph) + 1.2 * morph);
        (pitch, length, 0.7 * length)
    }

    fn phase_at(&self, root: f64, off: f64) -> f64 {
        let (off_step, offs, r_step, rs) = HACKLE_GRID;
        let fo = (off / off_step).clamp(0.0, offs as f64 - 1e-9);
        let fr = (root / r_step).clamp(0.0, rs as f64 - 1e-9);
        let (ko, kr) = (fo.floor() as usize, fr.floor() as usize);
        let (to, tr) = (fo - ko as f64, fr - kr as f64);
        let at = |k: usize| self.phase[k][kr] + (self.phase[k][kr + 1] - self.phase[k][kr]) * tr;
        at(ko) + (at(ko + 1) - at(ko)) * to
    }

    /// The distance past the table's edge where the row's phase reaches `phase`, mm: zero for a feather rooted under the table.
    fn root_of(&self, phase: f64, off: f64) -> f64 {
        if phase <= 0.0 {
            return 0.0;
        }
        let (mut lo, mut hi) = (0.0, 60.0);
        for _ in 0..28 {
            let mid = 0.5 * (lo + hi);
            if self.phase_at(mid, off) < phase { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    }

    /// Height and barbs, 0..1, at `root` mm past the table's edge (which lies `off` along the ring from the head's centre)
    /// and `across` the section. Each hackle has a drawn-out tip, a shaft bowed like a sickle, a vane notched once on each
    /// side and frayed along its barbs, and a free end standing over the row it overlaps. `keep` is asked once per
    /// feather, with its root and its row's reach across, so a feather is drawn whole or not at all.
    fn at(&self, root: f64, off: f64, across: f64, keep: &dyn Fn(f64, f64, f64) -> bool) -> (f64, f64) {
        let morph = smooth(self.l70, self.l110, root + off);
        let (pitch, length, _) = self.shape(root, morph);
        let phase = self.phase_at(root, off);
        let cross = across;
        let row = (cross / pitch).round() as i64;
        let mut height: f64 = 0.0;
        let mut barbs: f64 = 0.0;
        for j in row - 2..=row + 2 {
            let stagger = j.rem_euclid(2) as f64 * 0.5;
            let i0 = (phase - stagger).floor() as i64;
            for i in i0 - 1..=i0 {
                let t = (phase - stagger - i as f64) * 0.7;
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let r0 = self.root_of(i as f64 + stagger, off);
                let m0 = smooth(self.l70, self.l110, r0 + off);
                let (p0, l0, _) = self.shape(r0, m0);
                let reach = (j as f64).abs() * p0 + 0.55 * p0;
                if !keep(r0, l0, reach * (j as f64 + 0.001).signum()) {
                    continue;
                }
                let bow = SICKLE * length * t * t * (1.0 - morph) * (j as f64).signum();
                let y = cross - j as f64 * pitch - bow;
                let spread = (PI * t.powf(0.62)).sin().max(0.0).powf(0.8);
                let barb = t * length - 1.43 * y.abs();
                let fray = 1.0 - 0.06 * (1.0 - morph) * (1.0 - (2.0 * (barb / 0.45).rem_euclid(1.0) - 1.0).abs()) * smooth(0.2, 0.6, t);
                let half = (0.09 + (0.5 * pitch - 0.09) * spread) * fray;
                let edge = 1.0 - smooth((half - 0.36).max(0.0), half, y.abs());
                let tip = 1.0 - smooth(0.92, 1.0, t);
                let root_in = smooth(0.0, 0.30 - 0.17 * morph, t);
                let lift = 0.2 * smooth(0.55, 0.9, t) * (1.0 - morph);
                let vane = 0.44 + 0.33 * smooth(0.0, 0.8, t) - 0.12 * (y / half.max(0.1)).powi(2) + lift;
                let rachis = 0.035 * (1.0 - smooth(0.03, 0.12, y.abs())) * smooth(0.05, 0.25, t);
                let notch_at = if y > 0.0 { 0.52 } else { 0.7 };
                let notch = 1.0 - (1.0 - morph) * (1.0 - smooth(0.02, 0.08, (barb - notch_at * length).abs())) * smooth(0.3, 0.55, y.abs() / half.max(0.1));
                let h = (vane + rachis) * edge * tip * root_in * notch;
                if h > height {
                    height = h;
                    let line = (barb / 0.45).rem_euclid(1.0);
                    barbs = (1.0 - smooth(0.05, 0.2, (line - 0.5).abs())) * smooth(0.08, 0.2, y.abs()) * edge * tip * root_in;
                }
            }
        }
        (height, barbs)
    }
}

/// A ventral scute, 0..1, `u` counting scutes from the palm toward the head and `across` over the band's half-width:
/// domed across, rising from its root tucked under the last scute to a free edge toward the palm that rolls over onto
/// the next scute's root, with no step anywhere; the joints bow toward the head across the band.
fn scute(u: f64, across: f64) -> f64 {
    let c = across.clamp(-1.0, 1.0);
    let u = u + 0.26 * c * c;
    let t = u - u.floor();
    let lip = smooth(0.0, 0.16, t).sqrt();
    let rise = smooth(0.0, 0.84, 1.0 - t);
    let dome = 1.0 - 0.4 * c * c;
    (0.5 + 0.5 * rise * lip) * dome
}

/// Degrees from the palm's centre.
fn from_palm(theta: f64) -> f64 {
    ((theta - 270.0 + 180.0).rem_euclid(360.0) - 180.0).abs()
}

/// How much of the palm's skin is ventral scutes: 1 over the palm, handing over to keeled dorsal scales between 28 and
/// 46 degrees from it, about 3.5 mm of the band.
fn ventral_share(theta: f64) -> f64 {
    1.0 - smooth(28.0, 46.0, from_palm(theta))
}

/// How much of the band is the mantling's own morph rather than the dorsal scales that run on to the palm: 0 within
/// 60 degrees of the palm, 1 past 76.
fn mantling_share(theta: f64) -> f64 {
    smooth(60.0, 76.0, from_palm(theta))
}

/// Keeled dorsal scales, 0..1, in true mm at `DORSAL_PITCH` across and 1.2 times that along.
fn dorsal(along: f64, across: f64) -> f64 {
    reptile::snake(along / (1.2 * DORSAL_PITCH), across / DORSAL_PITCH)
}

/// The dorsal scales' pitch across the band, mm, and their relief as a share of the scutes' height.
const DORSAL_PITCH: f64 = 0.84;
const DORSAL_SHARE: f64 = 0.6;

fn skin_masks(a: &Atlas, hide: &Hide, s: &Sample) -> (f64, f64, f64) {
    let h = hide.at(s);
    let over_bore = smooth(a.bore + 1.15, a.bore + 1.65, s.p[0].hypot(s.p[1]));
    let edge = h.rim - h.across.abs();
    let crown = smooth(-0.25, 0.55, edge) * (1.0 - smooth(0.52, 0.80, s.n[2].abs()));
    let crease = smooth(0.40, 1.00, edge.abs());
    let neck = smooth(38.0, 46.0, s.theta)
        * (1.0 - smooth(64.0, 72.0, s.theta))
        * (1.0 - smooth(0.30, 1.4, (s.p[2] + 3.05).abs()));
    (over_bore * crease * (1.0 - neck), crown, h.along.abs())
}

fn portable(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    alpha: Alpha,
    height: f64,
    win: Window,
    bench: bool,
    reach: Option<&str>,
) -> Result<()> {
    let name = alpha.name.clone();
    lib.insert(Alpha::from_png16(&name, &alpha.to_png16()?)?);
    let mut layer = skin::hide_layer(d, &name, height, win);
    layer.mask = reach.map(Into::into);
    layer.bench_only = bench;
    if bench {
        layer.blend = Blend::Subtract;
    }
    d.layers.layers.push(layer);
    Ok(())
}

/// Domed round scales on a staggered lattice `pitch` apart in true mm, each `radius` round, 0..1, every centre nudged
/// off the lattice so no row runs ruled.
fn round_scales(x: f64, y: f64, pitch: f64, radius: f64) -> f64 {
    let row_h = pitch * 0.866;
    let j = (y / row_h).round() as i64;
    let mut h: f64 = 0.0;
    for jj in j - 1..=j + 1 {
        let off = if jj.rem_euclid(2) == 0 { 0.0 } else { 0.5 * pitch };
        let i = ((x - off) / pitch).round() as i64;
        for ii in i - 1..=i + 1 {
            let cx = ii as f64 * pitch + off + 0.12 * pitch * (skin::hash(ii, jj) - 0.5);
            let cy = jj as f64 * row_h + 0.12 * pitch * (skin::hash(jj + 101, ii) - 0.5);
            let r = radius * (0.92 + 0.16 * skin::hash(ii * 7 + 3, jj * 5 - 1));
            let d = (x - cx).hypot(y - cy) / r;
            h = h.max((1.0 - d * d).max(0.0));
        }
    }
    h
}

/// The walls' scales: their pitch and radius in true mm, and their relief.
const WALL_SCALES: (f64, f64, f64) = (0.92, 0.5, 0.11);

/// The polished walls under the table's chief and point, and the one bead line run 0.5 mm under the table's edge there.
struct RimBeads {
    edge: Line,
    stations: Vec<f64>,
}

/// The rim beads: how far under the table's edge they run, their pitch and radius, mm.
const RIM_BEADS: (f64, f64, f64) = (0.5, 0.62, 0.22);

impl RimBeads {
    fn new(table: &[P2]) -> Self {
        let mut ring = table.to_vec();
        ring.push(table[0]);
        let edge = Line::new(ring);
        let (_, pitch, _) = RIM_BEADS;
        let n = (edge.length() / pitch).floor() as usize;
        let step = edge.length() / n as f64;
        let stations = (0..n)
            .map(|k| k as f64 * step)
            .filter(|&s| {
                let i = edge.at.iter().position(|&a| a >= s).unwrap_or(1).clamp(1, edge.pts.len() - 1);
                let (a, b) = (edge.pts[i - 1], edge.pts[i]);
                let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
                ((b[0] - a[0]) / l).abs() > 0.72
            })
            .collect();
        Self { edge, stations }
    }

    /// Bead height at a wall sample, 0..1: `rim` its depth under the table's edge along the wall, `s` its place round it.
    fn at(&self, s: f64, rim: f64) -> f64 {
        let (depth, _, radius) = RIM_BEADS;
        let k = self.stations.partition_point(|&t| t < s);
        [k.saturating_sub(1), k.min(self.stations.len().saturating_sub(1))]
            .into_iter()
            .filter_map(|j| self.stations.get(j))
            .map(|&t| {
                let r = (s - t).hypot(rim - depth) / radius;
                if r < 1.0 { (1.0 - r * r).sqrt() } else { 0.0 }
            })
            .fold(0.0, f64::max)
    }
}

// --- The sculpted head -----------------------------------------------------------------------------------------------

type P3 = [f64; 3];

fn add3(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul3(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len3(a: P3) -> f64 {
    dot3(a, a).sqrt()
}

/// Imbricated scales for a sculpted surface, 0..1: staggered rows of smooth domes, each fuller toward its free edge
/// (+`along`) and keeled down its middle, meeting its neighbours in soft valleys so no wall is steep enough to step.
fn soft_scale(along: f64, across: f64) -> f64 {
    let row = across.round() as i64;
    let mut h: f64 = 0.0;
    for j in row - 1..=row + 1 {
        let stagger = 0.5 * j.rem_euclid(2) as f64;
        let col = (along - stagger).round();
        for i in [-1.0, 0.0, 1.0] {
            let x = along - col - stagger - i;
            let y = across - j as f64;
            let q = (x / 0.68).powi(2) + (y / 0.58).powi(2);
            let dome = (1.0 - q).max(0.0).powf(1.3);
            let lean = 0.62 + 0.38 * smooth(-0.6, 0.45, x);
            let keel = 0.12 * (-(y / 0.13).powi(2)).exp() * (1.0 - smooth(0.2, 0.6, x.abs()));
            h = h.max(dome * (lean + keel));
        }
    }
    h.min(1.0)
}

/// Distance in a plane to an axis-aligned box of half-sizes `b`, its corners rounded by `r`.
fn round_box2(p: P2, b: P2, r: f64) -> f64 {
    let q = [p[0].abs() - b[0] + r, p[1].abs() - b[1] + r];
    q[0].max(0.0).hypot(q[1].max(0.0)) + q[0].max(q[1]).min(0.0) - r
}

/// The head's frame on the face: where its axis stands (face `x`, `z` and height over the table, mm), the snout's
/// heading from -x toward the stone, how far the crown rolls toward the viewer and how far the snout pitches to the table
/// (degrees).
const HEAD_FRAME: (P3, f64, f64, f64) = ([-0.6, -1.95, 0.5], 6.0, 28.0, 6.0);
/// The lower jaw rolls less than the skull, so it drops across the table toward the stone rather than into it, degrees.
const JAW_ROLL: f64 = 4.0;
/// The gape: the mouth's corner along the head, the upper lip's height under the axis, and how far the lower jaw drops
/// (degrees).
const GAPE: (f64, f64, f64) = (-1.15, -0.32, 25.0);
/// Where the eye sits along the head and up it.
const EYE_AT: P2 = [1.35, 0.4];
/// The crown: its band's centre along the head, how high up the skull its foot sits, and the band's height and how far
/// it stands proud of the skull, mm.
const CROWN: (f64, f64, f64, f64) = (-0.75, 0.62, 0.9, 0.55);
/// The crown's fleurons: bearing round the band (degrees from the snout toward the viewer), height to the pearl's
/// centre and half-width at the band, mm.
const FLEURONS: [(f64, f64, f64); 5] = [(-4.0, 1.0, 0.66), (44.0, 1.18, 0.74), (90.0, 1.38, 0.8), (136.0, 1.18, 0.74), (184.0, 1.0, 0.66)];
/// A fleuron's thickness off the band, how far it leans out, the pearl's radius and the leaf's half-width where it
/// meets the pearl, mm and degrees.
const FLEURON_BODY: (f64, f64, f64, f64) = (0.78, 6.0, 0.37, 0.3);
/// Fangs, one each side of the upper jaw: root along the head, radius at the root and at the rounded tip, mm.
const FANG: (f64, f64, f64) = (3.25, 0.25, 0.165);
/// The tongue's stem and tines, radius at the root, the fork and the tines' rounded ends, mm.
const TONGUE_R: (f64, f64, f64) = (0.25, 0.21, 0.17);
/// How far the head's skirt sits under the table, how deep the part is buried, and the fillet where it meets the table.
const SKIRT_UNDER: (f64, f64, f64) = (0.12, 0.6, 0.45);
/// The neck: how far down the coil's spine the sculpt carries the painted body's own surface, about where it passes
/// under the paint, and where it has sunk to 0.85 of the paint's height, mm.
const NECK_RUN: (f64, f64, f64) = (5.0, 2.9, 4.0);
/// The grid the neck reads the painted coil from: its corner (face `x`, `z`), its size and its step, mm.
const NECK_GRID: (P2, [usize; 2], f64) = ([0.5, -4.0], [351, 326], 0.02);
/// Marching step and face budget of the sculpt.
const SCULPT_STEP: f64 = 0.045;
/// The head's size over the units its frame is drawn in.
const HEAD_SCALE: f64 = 1.28;
const SCULPT_FACES: usize = 92_000;

/// The basilisk's head as a distance field over world millimetres, standing on the table with its neck running down into
/// the painted coil.
struct Basilisk {
    table: f64,
    o: P3,
    a: P3,
    u: P3,
    w: P3,
    /// The lower jaw's frame, sharing the skull's heading and origin, and its hinge in that frame.
    uj: P3,
    wj: P3,
    hinge: P2,
    /// Where the base head's surface stands out along `w` at the eye and at the nostril.
    eye_w: f64,
    nose_w: f64,
    /// The skull's half-length and half-width where the crown's band sits.
    band: (f64, f64),
    tongue: Vec<(P3, f64)>,
    tines: [Vec<(P3, f64)>; 2],
    /// The painted coil's surface over the neck's zone, sampled every `NECK_GRID.2` mm from `NECK_GRID.0`: its height,
    /// how far outside its edge, and how far down its spine.
    neck: Vec<[f32; 3]>,
    /// The table's outline, for keeping the skirt on it.
    outline: Shape,
}

impl Basilisk {
    fn new(table_y: f64, outline: &Shape) -> Self {
        let (o, psi, rho, chi) = HEAD_FRAME;
        let (sp, cp) = psi.to_radians().sin_cos();
        let (sr, cr) = rho.to_radians().sin_cos();
        let (sc, cc) = chi.to_radians().sin_cos();
        let a0 = [-cp, 0.0, sp];
        let u0 = [-sp, 0.0, -cp];
        let w0 = [0.0, 1.0, 0.0];
        let u = add3(mul3(u0, cr), mul3(w0, sr));
        let w1 = add3(mul3(u0, -sr), mul3(w0, cr));
        let a = add3(mul3(a0, cc), mul3(w1, -sc));
        let w = add3(mul3(a0, sc), mul3(w1, cc));
        let (sj, cj) = JAW_ROLL.to_radians().sin_cos();
        let uj = add3(mul3(u0, cj), mul3(w0, sj));
        let wj1 = add3(mul3(u0, -sj), mul3(w0, cj));
        let wj = add3(mul3(a0, sc), mul3(wj1, cc));
        let uj = sub3(uj, mul3(a, dot3(uj, a)));
        let uj = mul3(uj, 1.0 / len3(uj));
        let o = [o[0], table_y + o[2], o[1]];
        let mut me = Self { table: table_y, o, a, u, w, uj, wj, hinge: [0.0, 0.0], eye_w: 0.0, nose_w: 0.0, band: (0.0, 0.0), tongue: Vec::new(), tines: [Vec::new(), Vec::new()], neck: Vec::new(), outline: Shape::new(outline.poly.clone()) };
        let (corner, lip, _) = GAPE;
        let h = me.jq([corner - 0.3, lip - 0.05, 0.0]);
        me.hinge = [h[0], h[1]];
        let surface = |me: &Self, at: P2| {
            let (mut lo, mut hi) = (0.0, 2.5);
            for _ in 0..50 {
                let mid = 0.5 * (lo + hi);
                if me.skull([at[0], at[1], mid], me.jq([at[0], at[1], mid])) < 0.0 { lo = mid } else { hi = mid }
            }
            0.5 * (lo + hi)
        };
        me.eye_w = surface(&me, EYE_AT);
        me.nose_w = surface(&me, [4.0, 0.15]);
        me.band = me.section(CROWN.1 + 0.5 * CROWN.2);
        let body = Body::new();
        let ([x0, z0], [nx, nz], step) = NECK_GRID;
        me.neck = (0..nx * nz)
            .map(|k| {
                let p = [x0 + (k % nx) as f64 * step, z0 + (k / nx) as f64 * step];
                match body.nearest_until(p, NECK_RUN.0 + 0.5) {
                    Some((d, s, _)) => [body.height(p, true) as f32, (d - body.girth(s)) as f32, s as f32],
                    None => [0.0, 9.0, 99.0],
                }
            })
            .collect();
        // The tongue leaves the gape over the lower jaw and runs out past the snout onto the table, lying on whatever
        // is under it, and forks there with its tines on the field.
        let (r0, r1, r2) = TONGUE_R;
        let lie = |me: &Self, p: P3, r: f64| -> P3 {
            let mut h = 4.0;
            while h > 0.0 && me.lower(me.local_scaled([p[0], table_y + h, p[2]]), me.jaw_scaled([p[0], table_y + h, p[2]])) > 0.0 {
                h -= 0.01;
            }
            [p[0], table_y + h.max(0.0) + 0.85 * r, p[2]]
        };
        let rise = r0 / HEAD_SCALE;
        let mut path: Vec<P3> = [0.7, 2.4, 4.0].iter().map(|&ja| me.jaw_world(ja, 0.04 + 0.8 * rise, 0.0)).collect();
        // Past the chin it dips onto the field and turns down toward the stone, where it forks.
        let (p1, p2) = (path[1], path[2]);
        let run = {
            let d = sub3(p2, p1);
            let l = d[0].hypot(d[2]).max(1e-9);
            [d[0] / l, 0.0, d[2] / l]
        };
        let turn_by = |v: P3, deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            [v[0] * c - v[2] * s, 0.0, v[0] * s + v[2] * c]
        };
        let bend = turn_by(run, -28.0);
        let q3 = add3(p2, mul3(run, 0.7));
        let q4 = add3(q3, mul3(bend, 0.55));
        path.push(lie(&me, q3, 0.5 * (r0 + r1)));
        path.push(lie(&me, q4, r1));
        let radii = [r0, r0, r0, 0.5 * (r0 + r1), r1];
        me.tongue = path.iter().zip(radii).map(|(p, r)| (lie(&me, *p, r), r)).collect();
        let (out, fork) = (me.tongue[3].0, me.tongue[4].0);
        let _ = (corner, lip);
        let dir = {
            let d = sub3(fork, out);
            let l = d[0].hypot(d[2]).max(1e-9);
            [d[0] / l, 0.0, d[2] / l]
        };
        for (k, side) in [1.0f64, -1.0].into_iter().enumerate() {
            let (s, c) = (side * 26.0f64).to_radians().sin_cos();
            let d = [dir[0] * c - dir[2] * s, 0.0, dir[0] * s + dir[2] * c];
            let mid = lie(&me, add3(fork, mul3(d, 0.5)), 0.5 * (r1 + r2));
            let end = lie(&me, add3(fork, mul3(d, 1.0)), r2);
            me.tines[k] = vec![(fork, r1), (mid, 0.5 * (r1 + r2)), (end, r2)];
        }
        me
    }

    /// World point of a point in the head's frame: along it toward the snout, up its crown, out toward the viewer.
    fn world(&self, l: P3) -> P3 {
        add3(self.o, add3(mul3(self.a, l[0]), add3(mul3(self.u, l[1]), mul3(self.w, l[2]))))
    }

    fn local(&self, p: P3) -> P3 {
        let d = sub3(p, self.o);
        [dot3(d, self.a), dot3(d, self.u), dot3(d, self.w)]
    }

    /// A world point in the head's own units, before its scale.
    fn local_scaled(&self, p: P3) -> P3 {
        mul3(self.local(p), 1.0 / HEAD_SCALE)
    }

    /// A world point in the lower jaw's frame, in the head's own units.
    fn jaw_scaled(&self, p: P3) -> P3 {
        let d = sub3(p, self.o);
        mul3([dot3(d, self.a), dot3(d, self.uj), dot3(d, self.wj)], 1.0 / HEAD_SCALE)
    }

    /// The world point of a point in the lower jaw's own frame: along it from the hinge, up from its lip line, across.
    fn jaw_world(&self, ja: f64, ju: f64, w: f64) -> P3 {
        let (sg, cg) = GAPE.2.to_radians().sin_cos();
        let h = [ja * cg + ju * sg, -ja * sg + ju * cg];
        let j = [self.hinge[0] + h[0], self.hinge[1] + h[1], w];
        add3(self.o, mul3(add3(mul3(self.a, j[0]), add3(mul3(self.uj, j[1]), mul3(self.wj, j[2]))), HEAD_SCALE))
    }

    /// The jaw frame's point for a point of the skull's frame.
    fn jq(&self, q: P3) -> P3 {
        self.jaw_scaled(self.world(mul3(q, HEAD_SCALE)))
    }

    /// The lower jaw, the mouth's lining and the throat alone, in the head's frame: what the tongue lies on.
    fn lower(&self, q: P3, j: P3) -> f64 {
        let (a, u) = (q[0], q[1]);
        let w = j[2];
        let (corner, lip, _) = GAPE;
        let (ja, ju) = self.jaw_frame(j);
        let taper = 1.0 - 0.32 * (ja / 4.6).clamp(0.0, 1.0);
        let jaw = smax(ellipsoid([ja - 2.05, ju + 0.34, w / taper], [2.6, 0.56, 1.1]) * taper, ju - 0.04, 0.2);
        let _ = (a, u, w, corner, lip);
        jaw
    }

    /// The bare skull and jaws without eyes, plates or crown, in the head's frame.
    fn skull(&self, q: P3, j: P3) -> f64 {
        let (a, u, w) = (q[0], q[1], q[2]);
        let wj = j[2];
        let cran = ellipsoid([a - 0.1, u - 0.3, w], [2.9, 1.0, 1.45]);
        let jowl = ellipsoid([a + 1.75, u + 0.02, w], [1.7, 1.12, 1.72]);
        // The snout: a long wedge, its top flattened, narrowing to the rostral.
        let snout = smax(ellipsoid([a - 2.35, u - 0.18, w], [2.2, 0.66, 0.9 - 0.08 * (a - 2.35).max(0.0)]), u - 0.62, 0.2);
        let mut up = smin(smin(cran, jowl, 0.8), snout, 0.9);
        // The canthus: a crisp ridge each side from the nostril back to the brow, where the flat top turns down the side.
        let canthus = round_cone([a, u, w.abs()], [4.2, 0.52, 0.42], [1.75, 0.82, 0.92], 0.07, 0.12);
        up = smin(up, canthus, 0.12);
        let (corner, lip, gape) = GAPE;
        let region = (u - lip).max(corner - a);
        up = smax(up, -region, 0.18);
        let (ja, ju) = self.jaw_frame(j);
        let taper = 1.0 - 0.32 * (ja / 4.6).clamp(0.0, 1.0);
        let jaw = ellipsoid([ja - 2.05, ju + 0.34, wj / taper], [2.6, 0.56, 1.1]) * taper;
        let jaw = smax(jaw, ju - 0.04, 0.2);
        let throat = ellipsoid([a + 1.5, u + 0.7, w], [1.35, 0.78, 1.3]);
        let _ = gape;
        smin(smin(up, jaw, 0.35), throat, 0.6)
    }

    /// The lower jaw's frame: along it from the hinge, and up from its lip line.
    fn jaw_frame(&self, j: P3) -> (f64, f64) {
        let (_, _, gape) = GAPE;
        let h = [j[0] - self.hinge[0], j[1] - self.hinge[1]];
        let (sg, cg) = gape.to_radians().sin_cos();
        (h[0] * cg - h[1] * sg, h[0] * sg + h[1] * cg)
    }

    /// Plates over the snout and between the eyes: unequal domed shields parted by V-grooves; mm the surface moves out.
    fn plates(&self, q: P3) -> f64 {
        let (a, u, w) = (q[0], q[1], q[2].abs());
        let mask = smooth(0.55, 0.85, u) * smooth(0.75, 1.05, a) * (1.0 - smooth(4.45, 4.7, a));
        if mask <= 0.0 {
            return 0.0;
        }
        // Rostral, internasals, prefrontals, the long frontal and a supraocular over each eye: eight shields of
        // unequal size, each with its own reach.
        const SITES: [(f64, f64, f64); 5] = [(4.4, 0.0, 0.3), (3.9, 0.27, 0.34), (3.15, 0.44, 0.48), (2.05, 0.0, 0.72), (1.35, 0.92, 0.6)];
        let mut best = (f64::MAX, f64::MAX);
        for &(sa, sw, r) in &SITES {
            for m in [1.0, -1.0] {
                if sw == 0.0 && m < 0.0 {
                    continue;
                }
                let d = (a - sa).hypot(w - m * sw) / r;
                if d < best.0 {
                    best = (d, best.0);
                } else if d < best.1 {
                    best.1 = d;
                }
            }
        }
        let edge = (best.1 - best.0) * 0.25;
        let groove = 0.09 * (1.0 - edge / 0.08).max(0.0);
        let dome = 0.1 * (1.0 - best.0.min(1.2).powi(2) / 1.44);
        mask * (dome - groove)
    }

    /// Lip scales along both jaws, and small keeled scales over the temples and jowls; mm the surface moves out.
    fn scales(&self, q: P3, j: P3) -> f64 {
        let (a, u, w) = (q[0], q[1], q[2].abs());
        let (corner, lip, _) = GAPE;
        let side = smooth(0.25, 0.55, w);
        let mut out = 0.0;
        // Supralabials: a row of low scalloped plates over the upper lip, parted by soft hollows.
        let t = (u - lip) / 0.5;
        if (0.0..1.3).contains(&t) && a > corner + 0.1 && a < 4.2 {
            let k = (a - corner - 0.1) / 0.62;
            let f = k - k.floor();
            let dome = (PI * f).sin().powf(0.8) * (PI * t.min(1.0)).sin().max(0.0);
            out += side * 0.045 * dome * (1.0 - smooth(1.0, 1.3, t));
        }
        // Infralabials: the same along the lower jaw.
        let (ja, ju) = self.jaw_frame(j);
        let side_j = smooth(0.25, 0.55, j[2].abs());
        let t = -ju / 0.45;
        if (0.0..1.3).contains(&t) && ja > 0.5 && ja < 4.4 {
            let k = (ja - 0.5) / 0.6;
            let f = k - k.floor();
            let dome = (PI * f).sin().powf(0.8) * (PI * t.min(1.0)).sin().max(0.0);
            out += side_j * 0.04 * dome * (1.0 - smooth(1.0, 1.3, t));
        }
        // Temporal and jowl scales behind the eye, clear of the lips and the crown.
        let behind = 1.0 - smooth(0.55, 0.95, a);
        let clear_lip = smooth(0.55, 0.8, u - lip);
        let under_crown = 1.0 - smooth(CROWN.1 - 0.25, CROWN.1, u) * (1.0 - smooth(0.9, 1.6, (a - CROWN.0).abs()));
        let m = behind * clear_lip * under_crown * side;
        if m > 0.0 {
            let lat = u.atan2(w) * 1.25 / 0.42;
            out += m * smooth(0.45, 0.8, w) * 0.06 * soft_scale(-a / 0.58, lat);
        }
        out
    }

    /// The crown: a flared band round the skull with five broad points, each under a pearl, and a jewel between the
    /// front three.
    fn crown(&self, q: P3) -> f64 {
        let (ca, foot, tall, proud) = CROWN;
        let (da, du, w) = (q[0] - ca, q[1] - foot, q[2]);
        if du < -1.2 || du > 4.0 || da.abs() > 4.5 || w.abs() > 3.5 {
            return 1.0;
        }
        // The band's ellipse follows the skull's own section at its foot, flaring out by a fifth of its height.
        let (ea, ew) = self.band;
        let flare = 0.18 * (du / tall).clamp(-0.5, 2.5);
        let (ra, rw) = (ea + flare, ew + flare);
        let e = (da / ra).hypot(w / rw);
        let g = (da / (ra * ra)).hypot(w / (rw * rw)) / e.max(1e-9);
        let radial = (e - 1.0) / g.max(1e-9);
        let band = round_box2([radial - 0.5 * proud + 0.15, du - 0.5 * tall], [0.5 * proud + 0.15, 0.5 * tall], 0.16);
        let mut d = band;
        let (thick, lean, pearl, petal) = FLEURON_BODY;
        let top = tall - 0.1;
        for &(deg, high, half) in &FLEURONS {
            let (s, c) = deg.to_radians().sin_cos();
            let (ra_t, rw_t) = (ea + 0.18 * top / tall, ew + 0.18 * top / tall);
            let p = [ra_t * c, rw_t * s];
            let n = {
                let v = [c / ra_t, s / rw_t];
                let l = v[0].hypot(v[1]);
                [v[0] / l, v[1] / l]
            };
            let tdir = [-n[1], n[0]];
            let rel = [da - p[0], w - p[1]];
            let t = rel[0] * tdir[0] + rel[1] * tdir[1];
            let v = du - top;
            let (sl, cl) = lean.to_radians().sin_cos();
            let nn = rel[0] * n[0] + rel[1] * n[1] - 0.5 * proud;
            let (along, off) = (v * cl + nn * sl, nn * cl - v * sl);
            if along < -0.8 || along > high + 0.8 || t.abs() > half + 0.8 {
                continue;
            }
            // The point in the fleuron's own plane: a foot on the band and a broad leaf narrowing to the pearl, its
            // flanks bowed in, so neighbouring points meet at the band in one serrated rim.
            let foot2 = round_box2([t, along + 0.05], [half, 0.32], 0.14);
            let k = (along / high).clamp(0.0, 1.0);
            let leaf_w = 0.62 * half * (1.0 - k).powf(1.1) + petal * k;
            let leaf = (t.abs() - leaf_w).max(-along - 0.1).max(along - high);
            // Two rounded side lobes lean out from the leaf: a trefoil, the pearl seated on the leaf's own tip.
            let lobe = ((t.abs() - 0.72 * half).hypot(along - 0.42 * high) - 0.32).max(-along);
            let shape = smin(smin(foot2, leaf, 0.12), lobe, 0.16);
            let half_thick = 0.5 * thick * (0.82 + 0.3 * (-shape / 0.3).clamp(0.0, 1.0));
            let r = 0.2;
            let dz = off.abs() - half_thick + r;
            let s2 = shape + r;
            let plate = s2.max(0.0).hypot(dz.max(0.0)) + s2.max(dz).min(0.0) - r;
            let ball = (t.hypot(along - high)).hypot(off) - pearl;
            d = smin(d, smin(plate, ball, 0.12), 0.1);
        }
        // Two cabochon jewels on the band between the front fleurons.
        for deg in [66.0f64, 114.0] {
            let (s, c) = deg.to_radians().sin_cos();
            let mid = 0.5 * tall;
            let (ra_m, rw_m) = (ea + 0.18 * mid / tall, ew + 0.18 * mid / tall);
            let n = {
                let v = [c / ra_m, s / rw_m];
                let l = v[0].hypot(v[1]);
                [v[0] / l, v[1] / l]
            };
            let centre = [ra_m * c + n[0] * (proud + 0.02), rw_m * s + n[1] * (proud + 0.02)];
            let rel = [da - centre[0], w - centre[1]];
            let t = rel[0] * -n[1] + rel[1] * n[0];
            let o = rel[0] * n[0] + rel[1] * n[1];
            d = smin(d, ellipsoid([t, du - mid, o], [0.4, 0.33, 0.2]), 0.06);
        }
        d
    }

    /// The head-frame point of crown point `k` at `t` across its plane, `along` up it and `off` through it.
    fn fleuron_point(&self, k: usize, t: f64, along: f64, off: f64) -> P3 {
        let (ca, foot, tall, proud) = CROWN;
        let (ea, ew) = self.band;
        let (_, lean, _, _) = FLEURON_BODY;
        let top = tall - 0.1;
        let (deg, _, _) = FLEURONS[k];
        let (s, c) = deg.to_radians().sin_cos();
        let (ra_t, rw_t) = (ea + 0.18 * top / tall, ew + 0.18 * top / tall);
        let p = [ra_t * c, rw_t * s];
        let n = {
            let v = [c / ra_t, s / rw_t];
            let l = v[0].hypot(v[1]);
            [v[0] / l, v[1] / l]
        };
        let tdir = [-n[1], n[0]];
        let (sl, cl) = lean.to_radians().sin_cos();
        let nn = along * sl + off * cl + 0.5 * proud;
        let v = along * cl - off * sl;
        [ca + p[0] + t * tdir[0] + nn * n[0], foot + v + top, p[1] + t * tdir[1] + nn * n[1]]
    }

    /// The skull's half-length and half-width at height `u` up its crown, mm: where the crown's band sits on it.
    fn section(&self, u: f64) -> (f64, f64) {
        let (mut ea, mut ew) = (0.0, 0.0);
        for (k, out) in [(0usize, &mut ea), (2, &mut ew)] {
            let (mut lo, mut hi) = (0.0, 4.0);
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                let mut q = [CROWN.0, u, 0.0];
                q[k] += if k == 0 { -mid } else { mid };
                if self.skull(q, self.jq(q)) < 0.0 { lo = mid } else { hi = mid }
            }
            *out = 0.5 * (lo + hi);
        }
        (ea, ew)
    }

    /// The head alone in its frame: skull and jaws with their plates and scales, the eyes under their brows, the
    /// nostrils, the fangs and the crown.
    fn head(&self, q: P3, j: P3) -> f64 {
        let (a, u) = (q[0], q[1]);
        let s = [a, u, q[2].abs()];
        let mut d = self.skull(q, j);
        let near = 1.0 - smooth(0.25, 0.6, d.abs());
        if near > 0.0 {
            d -= near * (self.plates(q) + self.scales(q, j));
        }
        // The brow ridge over each eye, the eye sunk in its socket under it, and a slit pupil.
        let (ea, eu) = (EYE_AT[0], EYE_AT[1]);
        let ew = self.eye_w;
        // The brow is a shelf standing out over the eye by 0.2 mm, so the eye sits in its shadow.
        let brow = ellipsoid([s[0] - ea + 0.05, s[1] - eu - 0.45, s[2] - ew + 0.12], [0.85, 0.24, 0.36]);
        d = smin(d, brow, 0.16);
        d = smax(d, -ellipsoid([s[0] - ea, s[1] - eu, s[2] - ew - 0.1], [0.48, 0.42, 0.42]), 0.08);
        d = smin(d, ellipsoid([s[0] - ea, s[1] - eu, s[2] - ew + 0.3], [0.38, 0.36, 0.4]), 0.04);
        d = smax(d, -ellipsoid([s[0] - ea, s[1] - eu, s[2] - ew - 0.12], [0.1, 0.28, 0.2]), 0.03);
        // Nostrils.
        d = smax(d, -ellipsoid([s[0] - 4.0, s[1] - 0.15, s[2] - self.nose_w - 0.05], [0.2, 0.13, 0.17]), 0.05);
        // A fang hanging from each side of the upper jaw, curving back into the gape.
        let (fa, r0, r1) = FANG;
        let (_, lip, _) = GAPE;
        let root = [fa, lip + 0.12, 0.52];
        let bend = [fa - 0.08, lip - 0.55, 0.55];
        let tip = [fa - 0.38, lip - 1.02, 0.5];
        let fang = round_cone(s, root, bend, r0, 0.5 * (r0 + r1)).min(round_cone(s, bend, tip, 0.5 * (r0 + r1), r1));
        d = smin(d, fang, 0.08);
        smin(d, self.crown(q), 0.12)
    }

    fn chain(c: &[(P3, f64)], p: P3) -> f64 {
        c.windows(2).fold(f64::MAX, |m, w| m.min(round_cone(p, w[0].0, w[1].0, w[0].1, w[1].1)))
    }

    /// The neck: the painted coil's own surface, scales and all, raised by half again behind the head and easing down
    /// to sink just under the paint by `NECK_RUN.2`, so the two skins share every scale and meet along one clean line
    /// across the body instead of grazing.
    fn neck_d(&self, p: P3) -> f64 {
        let ([x0, z0], [nx, nz], step) = NECK_GRID;
        let (fx, fz, h) = ((p[0] - x0) / step, (p[2] - z0) / step, p[1] - self.table);
        if fx < 0.0 || fz < 0.0 || fx >= (nx - 1) as f64 || fz >= (nz - 1) as f64 || h > 3.5 {
            return 1.0;
        }
        let (i, j) = (fx as usize, fz as usize);
        let (tx, tz) = (fx - i as f64, fz - j as f64);
        let at = |c: usize| {
            let g = |a: usize, b: usize| self.neck[(j + b) * nx + i + a][c] as f64;
            (g(0, 0) * (1.0 - tx) + g(1, 0) * tx) * (1.0 - tz) + (g(0, 1) * (1.0 - tx) + g(1, 1) * tx) * tz
        };
        let (painted, outside, s) = (at(0), at(1), at(2));
        let (run, level, under) = NECK_RUN;
        let k = 1.5 - 0.65 * smooth(0.5, under, s);
        let _ = level;
        (h - painted * k).max(outside + 0.15).max(-(h + 0.6)).max(s - run)
    }

    /// The whole part: head, tongue and neck, met to the table by a fillet over a skirt just under it, and buried
    /// `SKIRT_UNDER.1` deep.
    fn sdf(&self, p: P3) -> f64 {
        let q = self.local_scaled(p);
        let mut d = HEAD_SCALE * if len3(q) < 7.5 { self.head(q, self.jaw_scaled(p)) } else { len3(q) - 6.0 };
        d = smin(d, Self::chain(&self.tongue, p).min(Self::chain(&self.tines[0], p)).min(Self::chain(&self.tines[1], p)), 0.12);
        d = smin(d, self.neck_d(p), 0.5);
        let h = p[1] - self.table;
        let (under, bury, fillet) = SKIRT_UNDER;
        let d = smin(d, (h + under).max(d - 0.9), fillet);
        let d = smax(d, -(h + bury), 0.25);
        smax(d, 0.35 - self.outline.sdf([p[0], p[2]], 2.0), 0.2)
    }

    /// The head's box in world millimetres.
    fn bounds(&self) -> (P3, P3) {
        let t = self.table;
        ([-8.0, t - 0.8, -7.2], [6.6, t + 5.6, 3.6])
    }
}

/// A quick look at the sculpt alone on the bare stock: `--head [STEP]`.
fn preview_head(out: &Path, step: f64) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let d = base()?;
    let ctx = d.field_context();
    let ah = (AW as f64 * ctx.band_v_len_mm / ctx.circumference_mm).round() as usize;
    let a = Atlas::of(&d, AW, ah)?;
    let table = Shape::new(table_outline(&a));
    let b = Basilisk::new(a.top, &table);
    println!("table at {:.3} mm; eye surface {:.3}, nose {:.3}; crown section {:?}", a.top, b.eye_w, b.nose_w, b.section(CROWN.1 + 0.5 * CROWN.2));
    let t = Instant::now();
    let field = |p: P3| b.sdf(p);
    let (lo, hi) = b.bounds();
    let mut raw = sculpt::tetra_mesh(lo, hi, step, &field);
    sculpt::relax(&mut raw, &field, 2);
    println!("sculpt: {} triangles in {:.1} s", raw.f.len(), t.elapsed().as_secs_f64());
    let head = sculpt::to_mesh(&raw, &field);
    let bare = mesh::try_build(&d, &AlphaLibrary::builtin(), draft_params())?;
    let parts = [Part::metal(&bare.mesh, render::GOLD), Part::metal(&head, render::GOLD)];
    for (name, yaw, pitch) in [("hero", 0.55, 0.95), ("face", 0.0, PI * 0.5), ("side", 0.0, 0.0)] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, 1000)?;
    }
    render::write_png_parts(out.join("hero-300.png"), &parts, 0.55, 0.95, 300)?;
    let sheet: Vec<Vec<u8>> = [(0.55, 0.95), (0.0, PI * 0.5), (0.35, 1.05)].iter().map(|&(y, p)| render::render_parts_ss(&parts, y, p, 300, 300, 2)).collect();
    let mut contact = Vec::with_capacity(900 * 300 * 3);
    for row in 0..300 {
        for img in &sheet {
            contact.extend_from_slice(&img[row * 900..(row + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &contact, 900, 300, image::ColorType::Rgb8)?;
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    let near = crop(&bare.mesh, [0.0, 13.3, 0.6], 11.0);
    let close = [Part::metal(&near, render::GOLD), Part::metal(&head, render::GOLD)];
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, 1000)?;
    let only = crop(&head, [-1.2, a.top + 1.0, -2.6], 4.5);
    let zoom = [Part::metal(&only, render::GOLD)];
    render::write_png_parts(out.join("head-close.png"), &zoom, 0.35, 1.05, 1000)?;
    render::write_png_parts(out.join("head-front.png"), &zoom, -1.2, 0.5, 1000)?;
    render::write_png_parts(out.join("head-top.png"), &zoom, 0.0, PI * 0.5, 1000)?;
    Ok(())
}

/// The narrowest chord through `p` of the closed mesh `m` over forty directions, mm: the land a feature has there.
fn chord_land(m: &csg::Solid, p: P3) -> f64 {
    let hit = |o: P3, d: P3| -> f64 {
        let mut best = f64::MAX;
        for t in &m.f {
            let [a, b, c] = t.map(|i| m.v[i as usize]);
            let (e1, e2) = (sub3(b, a), sub3(c, a));
            let h = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
            let det = dot3(e1, h);
            if det.abs() < 1e-14 {
                continue;
            }
            let sv = sub3(o, a);
            let u = dot3(sv, h) / det;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = [sv[1] * e1[2] - sv[2] * e1[1], sv[2] * e1[0] - sv[0] * e1[2], sv[0] * e1[1] - sv[1] * e1[0]];
            let v = dot3(d, q) / det;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let dist = dot3(e2, q) / det;
            if dist > 1e-9 {
                best = best.min(dist);
            }
        }
        best
    };
    let mut least = f64::MAX;
    // Forty directions spread over a hemisphere on a golden spiral; each chord runs both ways.
    for k in 0..40 {
        let z = (k as f64 + 0.5) / 40.0;
        let (r, th) = ((1.0 - z * z).sqrt(), k as f64 * 2.399_963);
        let d = [r * th.cos(), z, r * th.sin()];
        least = least.min(hit(p, d) + hit(p, mul3(d, -1.0)));
    }
    least
}

/// The sculpt's thin members measured on the decimated mesh, each as its narrowest chord at its thinnest station.
fn head_lands(b: &Basilisk, m: &csg::Solid) -> Vec<(String, f64, Option<&'static str>)> {
    let k = HEAD_SCALE;
    let at = |l: P3| b.world(mul3(l, k));
    let (_, _, pearl, _) = FLEURON_BODY;
    let mut out = Vec::new();
    let crown = |f: &dyn Fn(usize) -> P3| (0..FLEURONS.len()).map(|i| chord_land(m, at(f(i)))).fold(f64::MAX, f64::min);
    out.push(("Crown points, the plate at mid-height (sculpted, measured)".to_string(), crown(&|i| b.fleuron_point(i, 0.0, 0.45 * FLEURONS[i].1, 0.0)), None));
    out.push(("Crown points, the leaf's waist under its pearl (sculpted, measured)".to_string(), crown(&|i| b.fleuron_point(i, 0.0, FLEURONS[i].1 - pearl - 0.06, 0.0)), Some("Point's waist under its pearl: investment detail, cast in place and cleaned up with a graver")));
    out.push(("Crown points, each side lobe (sculpted, measured)".to_string(), crown(&|i| b.fleuron_point(i, 0.72 * FLEURONS[i].2, 0.42 * FLEURONS[i].1, 0.0)), Some("Trefoil's side lobe: investment detail, cast in place")));
    out.push(("Crown pearls (sculpted, measured)".to_string(), crown(&|i| b.fleuron_point(i, 0.0, FLEURONS[i].1, 0.0)), None));
    let (fa, _, _) = FANG;
    let (_, lip, _) = GAPE;
    let tip = [fa - 0.38, lip - 1.02, 0.5];
    let bend = [fa - 0.08, lip - 0.55, 0.55];
    let near_tip = add3(tip, mul3(sub3(bend, tip), 0.18));
    out.push(("Fang near its rounded tip (sculpted, measured)".to_string(), chord_land(m, at(near_tip)), Some("Fang hanging from the upper jaw to a rounded point: investment detail, cast in place")));
    out.push(("Forked tongue, stem (sculpted, measured)".to_string(), chord_land(m, b.tongue[2].0), Some("Tongue's round stem: investment detail cast in place, lying on the jaw and the field")));
    let tine = b.tines.iter().map(|t| {
        let (e, p) = (t[2].0, t[1].0);
        chord_land(m, add3(e, mul3(sub3(p, e), 0.25)))
    }).fold(f64::MAX, f64::min);
    out.push(("Forked tongue, tines near their ends (sculpted, measured)".to_string(), tine, Some("Tongue's tine to a rounded end, lying on the field: investment detail cast in place")));
    out
}

/// The sculpted head as a stored part joined to the stock, its solid for the footprint, and its numbers.
fn head_part(b: &Basilisk) -> Result<(Feature, csg::Solid, Value, Vec<(String, f64, Option<&'static str>)>)> {
    let t = Instant::now();
    let field = |p: P3| b.sdf(p);
    let (lo, hi) = b.bounds();
    // The sculpt is deterministic, so a run keeps it under target/ keyed by the source of its field and reuses it.
    let src = include_str!("bestiarium_basiliscus.rs");
    let field_src = &src[src.find("// --- The sculpted head").unwrap_or(0)..src.find("/// A quick look at the sculpt").unwrap_or(src.len())];
    let key = field_src.bytes().fold(0xcbf29ce484222325u64, |h, c| (h ^ c as u64).wrapping_mul(0x100000001b3)) ^ b.table.to_bits();
    let cache = std::path::PathBuf::from(format!("target/basiliscus-head-{key:016x}.json"));
    let (raw_faces, nets) = match std::fs::read(&cache).ok().and_then(|b| serde_json::from_slice::<(usize, Vec<P3>, Vec<[u32; 3]>)>(&b).ok()) {
        Some((n, v, f)) => (n, csg::Solid { v, f }),
        None => {
            let mut raw = sculpt::tetra_mesh(lo, hi, SCULPT_STEP, &field);
            sculpt::relax(&mut raw, &field, 3);
            let nets = sculpt::settle(sculpt::clean_decimate(&raw, SCULPT_FACES), &field, &|_| true);
            let _ = std::fs::write(&cache, serde_json::to_vec(&(raw.f.len(), &nets.v, &nets.f))?);
            (raw.f.len(), nets)
        }
    };
    let (open, volume) = sculpt::closure(&nets);
    let crossings = csg::self_crossings(&nets);
    let mesh = sculpt::packed(&nets)?;
    let top = nets.v.iter().map(|p| p[1]).fold(f64::MIN, f64::max) - b.table;
    let stats = json!({"marching_step_mm": SCULPT_STEP, "raw_triangles": raw_faces, "triangles": nets.f.len(), "vertices": nets.v.len(),
        "open_edges": open, "volume_mm3": volume, "self_crossings": crossings, "packed_bytes": mesh.data.len(), "height_over_table_mm": top,
        "seconds": t.elapsed().as_secs_f64()});
    println!("  head: {} triangles from {} in {:.1} s, {} KB packed, {top:.2} mm over the table", nets.f.len(), raw_faces, t.elapsed().as_secs_f64(), mesh.data.len() / 1024);
    let recipe = cad::stored::Recipe {
        kernel: "basiliscus".into(),
        op: "sculpt".into(),
        params: json!({"field": "bestiarium_basiliscus.rs Basilisk::sdf", "frame": [HEAD_FRAME.0, HEAD_FRAME.1, HEAD_FRAME.2, HEAD_FRAME.3], "scale": HEAD_SCALE, "gape": [GAPE.0, GAPE.1, GAPE.2], "step_mm": SCULPT_STEP, "faces": SCULPT_FACES}),
        digest: String::new(),
    };
    let component = Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::Free, blend_mm: 0.0, ..Component::default() };
    let lands = head_lands(b, &nets);
    Ok((Feature { id: 2, name: "Basilisk's crowned head".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh }, component }, nets, stats, lands))
}

fn author(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Value, Vec<Value>)> {
    let mut d = base()?;
    d.build = params;
    let ctx = d.field_context();
    let ah = (AW as f64 * ctx.band_v_len_mm / ctx.circumference_mm).round() as usize;
    let a = Atlas::of(&d, AW, ah)?;
    let hide = Hide::of(&a);
    let outline = table_outline(&a);
    let basilisk = Basilisk::new(a.top, &Shape::new(outline.clone()));
    let (head, head_solid, head_stats, head_lands) = head_part(&basilisk)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    doc.append(head)?;
    let arms = Arms::new(outline, Footprint::of(&head_solid, a.top))?;
    let gem = Gem {
        l_mm: 8.0,
        preview_tint: Some([0.025, 0.30, 0.085]),
        ..Gem::calibrated(GemCut::Marquise, 4.0)
    };
    let (theta_deg, v_mm) = chart(&a, STONE)?;
    let mut seat = SeatPadLayer {
        theta_deg,
        v_mm,
        style: SeatStyle::Boss,
        crown: 0.15,
        blend_mm: SKIRT,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        rot_deg: 0.0,
        ..Default::default()
    };
    seat.fit_stone(gem);
    let (ra, rb, pw) = SEAT_PLAN;
    seat.diameter_mm = 2.0 * rb;
    seat.elong = ra / rb;
    seat.plan_pow = pw;
    seat.height_mm = STONE_BOSS;
    let l70 = hide.along[(AW as f64 * 160.0 / 360.0).round() as usize].abs();
    let l110 = hide.along[(AW as f64 * 200.0 / 360.0).round() as usize].abs();
    let on_table = |s: &Sample| s.n[1] > 0.95 && s.p[1] > a.top - 0.05;
    let off_table = a.paint("off", |s| {
        let high = smooth(a.top - 1.6, a.top - 0.9, s.p[1]);
        1.0 - high * (1.0 - smooth(0.05, 0.45, -arms.table.sdf([s.p[0], s.p[2]], 1.0)))
    });
    let width: Vec<f64> = (0..=400).map(|k| arms.table_width_at(-10.0 + 0.05 * k as f64)).collect();
    let width_at = |z: f64| width[((z + 10.0) / 0.05).round().clamp(0.0, 400.0) as usize];
    let mut lib = AlphaLibrary::builtin();
    let reach = a.paint(REACH, |s| smooth(a.bore + 1.0, a.bore + 1.1, s.p[0].hypot(s.p[1])));
    lib.insert(Alpha::from_png16(REACH, &reach.to_png16()?)?);
    // Where the surface still faces outward it carries the mantling; past the rim, where it faces the pull, the walls
    // are polished. Every gate is a Hide distance in true mm, asked once per feather at its root, so nothing is drawn in part.
    let hackles = Hackles::new(l70, l110);
    let feather = |s: &Sample| {
        let h = hide.at(s);
        let (_, _, along) = skin_masks(&a, &hide, s);
        let near = 1.0 - smooth(9.0, 14.0, along);
        let off = width_at(s.p[2]) * near + 7.0 * (1.0 - near) + 0.2;
        // Rows are laid across the outer surface as a share of its own width, so they narrow with it down the shoulder;
        // a row is kept whole when its feathers stay clear of the rim.
        let keep = |_: f64, _: f64, reach: f64| reach.abs() < PLUME_WIDTH - 0.3;
        hackles.at((along - off).max(0.0), along.min(off), h.across * PLUME_WIDTH / h.rim.max(0.5), &keep)
    };
    let alpha = a.paint("Basiliscus", |s| {
        if !on_table(s) {
            return 0.0;
        }
        // Where the coil laps the boss it rides the boss's wall as a 0.35 mm fillet, so no crease opens between them.
        let at = |dt: f64, dv: f64| seat.height(ringdesign_core::Uv { u: ctx.u_of_theta(s.theta + dt), v: s.v + dv }, &ctx);
        let r = s.p[0].hypot(s.p[1]).max(1.0);
        let reach = 0.35;
        let dt = (reach / r).to_degrees();
        let pad = (0..8).map(|k| {
            let (sn, cs) = (k as f64 * PI / 4.0).sin_cos();
            at(dt * cs, reach * sn)
        }).fold(at(0.0, 0.0), f64::max);
        arms.serpent([s.p[0], s.p[2]], pad) / SERPENT_HEIGHT
    });
    let painted_peak = alpha.data.iter().copied().fold(0.0f32, f32::max) as f64 * SERPENT_HEIGHT;
    portable(&mut d, &mut lib, alpha, SERPENT_HEIGHT, window(90.0, 70.0), false, None)?;
    let mut face = Window::around(90.0, 64.0);
    face.fade_deg = 1.0;
    let alpha = a.paint("Beaded bordure", |s| if on_table(s) { arms.bordure_height([s.p[0], s.p[2]]) } else { 0.0 });
    portable(&mut d, &mut lib, alpha, BEAD_HEIGHT, face, false, None)?;
    let rims = RimBeads::new(&arms.table.poly);
    let alpha = a.paint("Rim beads", |s| {
        if !within(s.theta, 90.0, 40.0) || on_table(s) || s.p[1] < a.top - 1.5 {
            return 0.0;
        }
        let h = hide.at(s);
        if h.rim - h.across.abs() > -0.2 {
            return 0.0;
        }
        let Some((flat, round)) = rims.edge.nearest([s.p[0], s.p[2]], 1.0) else { return 0.0 };
        rims.at(round, flat.hypot(a.top - s.p[1]))
    });
    portable(&mut d, &mut lib, alpha, RIM_BEADS.2, face, false, Some(REACH))?;
    // The walls under the table carry the serpent's skin: domed round scales laid along the table's edge and down the
    // wall in true mm, starting under the rim beads with a 1 mm ramp and fading to polish over the last 1.5 mm above the
    // bore edge, and handing over to the mantling where the wall runs out into the shoulder.
    let (pitch, radius, wall_relief) = WALL_SCALES;
    let alpha = a.paint("Wall scales", |s| {
        if !within(s.theta, 90.0, 52.0) || on_table(s) || s.p[1] > a.top - 0.3 {
            return 0.0;
        }
        let Some((flat, round)) = rims.edge.nearest([s.p[0], s.p[2]], 2.5) else { return 0.0 };
        let depth = a.top - s.p[1];
        let r = s.p[0].hypot(s.p[1]);
        let w = smooth(0.8, 1.8, depth.hypot(flat)) * smooth(a.bore + 0.2, a.bore + 1.5, r) * (1.0 - smooth(0.8, 1.6, flat));
        if w <= 0.0 {
            return 0.0;
        }
        w * round_scales(round, depth, pitch, radius)
    });
    portable(&mut d, &mut lib, alpha, wall_relief, window(90.0, 100.0), false, Some(REACH))?;
    let alpha = a.paint("Hackles into scales", |s| {
        if !within(s.theta, 90.0, 136.0) {
            return 0.0;
        }
        let (bore, _, _) = skin_masks(&a, &hide, s);
        feather(s).0 * bore * off_table.data[s.i] as f64 * mantling_share(s.theta)
    });
    portable(&mut d, &mut lib, alpha, HACKLE_HEIGHT, window(90.0, 260.0), false, Some(REACH))?;
    let far = hide.reach();
    let alpha = a.paint("Belly scutes", |s| {
        if !within(s.theta, 270.0, 82.0) {
            return 0.0;
        }
        let h = hide.at(s);
        let (bore, _, _) = skin_masks(&a, &hide, s);
        let keep = smooth(0.0, 0.8, h.rim - h.across.abs()) * bore;
        let v = ventral_share(s.theta);
        let belly = scute((far - h.along.abs()) / 2.2, h.across / h.rim.max(0.5)) * v;
        let back = DORSAL_SHARE * dorsal(h.along.abs(), h.across) * (1.0 - v) * (1.0 - mantling_share(s.theta));
        belly.max(back) * keep
    });
    portable(&mut d, &mut lib, alpha, BELLY_HEIGHT, window(270.0, 164.0), false, Some(REACH))?;
    let alpha = a.paint("Graver's barbs and keels", |s| {
        if !within(s.theta, 90.0, 136.0) {
            return 0.0;
        }
        let (bore, _, along) = skin_masks(&a, &hide, s);
        feather(s).1 * bore * off_table.data[s.i] as f64 * (1.0 - smooth(0.35, 0.9, smooth(l70, l110, along))) * mantling_share(s.theta)
    });
    portable(&mut d, &mut lib, alpha, 0.065, window(90.0, 260.0), true, Some(REACH))?;
    let lands = land_rows(&arms, &seat, gem, &head_lands);
    let mut e = LayerEntry::new("Tsavorite, flush", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    let alpha = a.paint("Pounced field", |s| if on_table(s) { arms.pounce_depth([s.p[0], s.p[2]]) } else { 0.0 });
    portable(&mut d, &mut lib, alpha, PIT_DEPTH, face, false, None)?;
    d.layers.layers.last_mut().unwrap().blend = Blend::Subtract;
    let (bare, edge, share) = arms.pounce_reach();
    let (crest, steep) = arms.body.profile();
    let composition = json!({
        "stone_mm": STONE, "seat_plan_mm": [SEAT_PLAN.0, SEAT_PLAN.1, SEAT_PLAN.2], "stone_boss_mm": STONE_BOSS, "coil_lap_on_seat_mm": LAP,
        "spine_mm": arms.body.length(), "girth_mm": [GIRTH.0, GIRTH.1], "body_crown_mm": DOME, "body_scale_relief_mm": SCALE_RELIEF,
        "body_scale_width": {"share_of_girth": SCALE_CELL.0, "bounds_mm": [SCALE_CELL.1, SCALE_CELL.2], "length_over_width": SCALE_CELL.3},
        "body_crest_mm": crest, "body_outer_quarter_steepest_deg": steep,
        "head": head_stats,
        "head_frame": {"axis_face_x_z_height_mm": HEAD_FRAME.0, "heading_deg": HEAD_FRAME.1, "crown_roll_deg": HEAD_FRAME.2, "snout_pitch_deg": HEAD_FRAME.3, "jaw_roll_deg": JAW_ROLL, "scale": HEAD_SCALE,
                       "gape": {"corner": GAPE.0, "lip": GAPE.1, "drop_deg": GAPE.2}},
        "crown_mm": {"band_half_axes": [basilisk.band.0 * HEAD_SCALE, basilisk.band.1 * HEAD_SCALE], "band_height": CROWN.2 * HEAD_SCALE, "band_proud": CROWN.3 * HEAD_SCALE,
                     "fleurons": FLEURONS.iter().map(|f| json!({"bearing_deg": f.0, "height": f.1 * HEAD_SCALE, "width_at_band": 2.0 * f.2 * HEAD_SCALE})).collect::<Vec<_>>(),
                     "pearl_diameter": 2.0 * FLEURON_BODY.2 * HEAD_SCALE},
        "serpent_layer_scale_mm": SERPENT_HEIGHT, "serpent_painted_peak_mm": painted_peak,
        "pit_depth_mm": PIT_DEPTH, "pits": arms.pits.len(), "pit_reach_mm": {"widest_bare_field": bare, "furthest_arm_edge_from_a_pit": edge, "share_of_arm_edge_within_0_4": share},
        "bead_height_mm": BEAD_HEIGHT, "beads": arms.beads.len(), "rim_beads": rims.stations.len(),
        "table_points": arms.table.poly.len(), "table_z_mm": [arms.table.lo[1], arms.table.hi[1]], "table_x_mm": [arms.table.lo[0], arms.table.hi[0]],
        "morph_start_along_mm": l70, "morph_end_along_mm": l110, "stock": "020 native unmirrored", "atlas": [AW, ah],
    });
    Ok((d, lib, composition, lands))
}

/// The lost-wax land census: every proud feature's narrowest land in plan, and what names it when it is under the floor.
fn land_rows(arms: &Arms, seat: &SeatPadLayer, gem: Gem, head: &[(String, f64, Option<&'static str>)]) -> Vec<Value> {
    let row = |name: &str, kind: &str, land: f64, note: Option<&str>| {
        json!({"name": name, "kind": kind, "min_land_mm": (land * 1000.0).round() / 1000.0,
               "status": if land >= LAND_FLOOR - 1e-3 { "at or above 0.8".to_string() } else { note.map_or("UNNAMED".to_string(), str::to_string) },
               "exception": land < LAND_FLOOR - 1e-3 && note.is_some()})
    };
    let mut rows = Vec::new();
    let (ra, rb) = seat.semi_axes_mm();
    let girdle = |t: f64, grow: f64| {
        let (c, s) = (t.cos(), t.sin());
        [(gem.l_mm * 0.5 + grow) * c.signum() * c.abs().powf(2.0 / 1.5), (gem.w_mm * 0.5 + grow) * s.signum() * s.abs().powf(2.0 / 1.5)]
    };
    let rim: Vec<P2> = (0..1440)
        .map(|i| {
            let t = 2.0 * PI * i as f64 / 1440.0;
            let (c, s) = (t.cos(), t.sin());
            [ra * c.signum() * c.abs().powf(2.0 / seat.plan_pow), rb * s.signum() * s.abs().powf(2.0 / seat.plan_pow)]
        })
        .collect();
    let wall = |grow: f64| (0..1440).map(|i| poly_dist(&rim, girdle(2.0 * PI * i as f64 / 1440.0, grow))).fold(f64::MAX, f64::min);
    let bevel = (0.07 * gem.w_mm).clamp(0.08, 0.22);
    rows.push(row("Boss wall round the girdle", "seat", wall(0.03), None));
    rows.push(row(
        "Boss rim at the bright-cut bevel's top edge",
        "seat",
        wall(0.03 + bevel),
        Some("Bright-cut bevel over the girdle: cast as a chamfer, recut by the setter and burnished over the tsavorite"),
    ));
    let body = &arms.body;
    let n = (body.length() / 0.02).ceil() as usize;
    let thinnest = (0..=n).map(|k| 2.0 * body.girth(body.length() * k as f64 / n as f64)).fold(f64::MAX, f64::min);
    rows.push(row("Serpent's body to the tail's tip (painted)", "painted", thinnest, None));
    // The sculpted head's thin members, from the sizes its field is drawn with.
    let k = HEAD_SCALE;
    rows.push(row("Crown band, its height on the skull (sculpted)", "sculpt", CROWN.2 * k, None));
    for (name, land, note) in head {
        rows.push(row(name, "sculpt", *land, *note));
    }
    rows.push(row("Beaded bordure beads (painted)", "painted", 2.0 * BEADS.2, Some("Bordure bead: investment detail, cast in place and burnished")));
    rows.push(row("Rim beads under the table's edge (painted)", "painted", 2.0 * RIM_BEADS.2, Some("Rim bead under the table's edge: investment detail, cast in place and burnished")));
    rows
}

/// The land census passes when every row is at or above the floor, or is named and still at or above the investment detail floor.
fn lands_pass(rows: &[Value], detail: f64) -> bool {
    rows.iter().all(|r| {
        let land = r["min_land_mm"].as_f64().unwrap_or(0.0);
        land >= LAND_FLOOR - 1e-3 || (r["exception"] == true && land >= detail)
    })
}

fn solid(mesh: &Mesh) -> csg::Solid {
    csg::Solid {
        v: mesh
            .vertices
            .iter()
            .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
            .collect(),
        f: mesh.faces.clone(),
    }
}

/// The faces of `m` within `radius` of `centre`, as a mesh of their own.
fn crop(m: &Mesh, centre: [f64; 3], radius: f64) -> Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
}

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut image = Vec::with_capacity(edge * edge * 6);
    for y in 0..edge {
        image.extend_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        image.extend_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &image, (2 * edge) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Studio-gold renders with the stone set: named views, a close-up of the head, bare stock against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &BuildResult, gems: &[(Mesh, [f32; 3])], params: BuildParams, edge: usize) -> Result<()> {
    let stone_parts = || gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)).collect::<Vec<_>>();
    let mut parts = vec![Part::metal(&built.mesh, render::GOLD)];
    parts.extend(stone_parts());
    for (name, yaw, pitch) in [
        ("hero", 0.55, 0.95),
        ("face", 0.0, PI * 0.5),
        ("palm", PI, 1.05),
        ("side", 0.0, 0.0),
        ("shoulder", -0.9, 0.62),
        ("reverse", 1.6, 0.8),
    ] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, 0.55, 0.95, 300)?;
    // Close-ups frame the whole ring on a point, so every edge keeps its crease normals.
    render::write_png_framed(out.join("stones.png"), &parts, 0.35, 1.05, render::Framing::new([-0.3, 14.0, 0.4], 9.5), edge)?;
    render::write_png_framed(out.join("head.png"), &parts, 0.35, 1.05, render::Framing::new([-1.2, 14.8, -2.4], 4.8), edge)?;
    let bare = mesh::try_build(&base()?, lib, params)?;
    let left = render::render_parts_ss(&[Part::metal(&bare.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 2);
    let right = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 2);
    side_by_side(&out.join("bare-vs-finished.png"), &left, &right, edge)?;
    Ok(())
}

/// Least distance from the finger axis to any finished vertex less the bore radius, how many fall 0.01 mm inside, and where the least is.
fn bore_clearance(d: &RingDesign, m: &Mesh) -> (f64, usize, [f64; 3]) {
    let bore = d.inner_radius_mm();
    let mut worst = (f64::MAX, 0, [0.0; 3]);
    for p in &m.vertices {
        let r = (p.0 as f64).hypot(p.1 as f64) - bore;
        if r < -0.01 {
            worst.1 += 1;
        }
        if r < worst.0 {
            worst.0 = r;
            worst.2 = [p.0 as f64, p.1 as f64, p.2 as f64];
        }
    }
    worst
}

/// Each stamp's solid made as the build makes it, tier by tier, checked closed and uncrossed.
fn stamp_checks(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Vec<Value>> {
    let ctx = d.field_context();
    let mut tiers: Vec<u8> = d.stamps.iter().map(|s| s.tier).collect();
    tiers.sort_unstable();
    tiers.dedup();
    let mut out = Vec::new();
    for tier in tiers {
        let mut under = d.clone();
        under.stamps.retain(|s| s.tier < tier);
        for e in &mut under.layers.layers {
            if let Layer::SeatPad(s) = &mut e.layer {
                s.solid = SolidKind::None;
            }
        }
        let built = mesh::try_build(&under, lib, params)?;
        let band = solid(&built.mesh);
        for s in d.stamps.iter().filter(|s| s.tier == tier) {
            let part = s.solid(&s.frame(d, &ctx), &band).map_err(|e| anyhow!("{}: {e}", s.name))?;
            let check = part.check(true);
            out.push(json!({"name": s.name, "tier": s.tier, "self_crossings": check.self_crossings, "zero_area_faces": check.zero_area_faces, "open_edges": check.open_edges, "repeated_edges": check.repeated_edges}));
        }
    }
    Ok(out)
}

/// The seat's made bur and relief, checked closed and uncrossed.
fn seat_checks(d: &RingDesign) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for (stone, frame) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let bare = [frame.girdle[0] - frame.normal[0] * stand, frame.girdle[1] - frame.normal[1] * stand];
        let r = bare[0].hypot(bare[1]);
        let outward = (frame.normal[0] * bare[0] + frame.normal[1] * bare[1]) / r;
        let through = (stone.seat.through && outward > 0.75).then(|| stand + (r - d.inner_radius_mm()).max(0.0) / outward + 0.6);
        let fit = setting::Fit { surface_z: stone.seat.height_mm - stand, through_mm: through, prongs: 0 };
        let parts = setting::parts(stone.gem, stone.seat.solid, fit).map_err(|e| anyhow!("{e}"))?;
        for part in parts.add.iter().chain(&parts.cut) {
            let check = part.check(true);
            out.push(json!({"name": stone.label, "self_crossings": check.self_crossings, "zero_area_faces": check.zero_area_faces, "open_edges": check.open_edges, "repeated_edges": check.repeated_edges}));
        }
    }
    Ok(out)
}

/// Every made CAD part as the build placed it, checked closed and uncrossed: the sculpted head.
fn cad_part_checks(built: &BuildResult) -> Vec<Value> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter_map(|c| c.made.as_ref().map(|m| (c.name.clone(), m.solid().check(true))))
        .map(|(name, check)| json!({"name": name, "self_crossings": check.self_crossings, "zero_area_faces": check.zero_area_faces, "open_edges": check.open_edges, "repeated_edges": check.repeated_edges}))
        .collect()
}

fn clean_part(g: &Value) -> bool {
    g["self_crossings"] == 0 && g["zero_area_faces"] == 0 && g["open_edges"] == 0 && g["repeated_edges"] == 0
}

/// Mesh gates of one build: closed, clean, uncrossed, every solid and stamp resolved, nothing in the finger hole.
fn mesh_gates(d: &RingDesign, built: &BuildResult) -> Value {
    let cross = csg::self_crossings(&solid(&built.mesh));
    let (bore_min, bore_inside, bore_at) = bore_clearance(d, &built.mesh);
    json!({
        "triangles": built.mesh.faces.len(),
        "watertight": built.report.validation.watertight,
        "degenerate_faces": built.report.quality.degenerate_faces,
        "self_crossings": cross,
        "solids_resolved": built.solids.resolved,
        "stamps_struck": built.solids.stamped,
        "stamps": d.stamps.len(),
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "bore_clearance_min_mm": bore_min,
        "bore_vertices_inside": bore_inside,
        "bore_clearance_at": bore_at,
        "pass": built.report.validation.watertight
            && built.report.quality.degenerate_faces == 0
            && cross == 0
            && built.solids.notes.is_empty()
            && built.parts.notes.is_empty()
            && built.solids.stamped == d.stamps.len()
            && built.solids.resolved == 1
            && bore_inside == 0,
    })
}

fn write(out: &Path, draft: bool, verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let params = if draft { draft_params() } else { export_params() };
    let start = Instant::now();
    let (d, lib, composition, lands) = author(params)?;
    let authored_s = start.elapsed().as_secs_f64();
    let built = mesh::try_build(&d, &lib, params)?;
    println!("built {} faces in {:.1} s (authored in {authored_s:.1} s); {:?}", built.mesh.faces.len(), start.elapsed().as_secs_f64(), built.solids.notes);
    let geometry = mesh_gates(&d, &built);
    for p in sculpt::crossing_sites(&solid(&built.mesh)).iter().take(12) {
        println!("  crossing at x {:.2} y {:.2} z {:.2} (theta {:.1})", p[0], p[1], p[2], p[1].atan2(p[0]).to_degrees());
    }
    let coarse = mesh_gates(&d, &mesh::try_build(&d, &lib, coarse_params())?);
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let dfm = ringdesign_core::dfm::findings_in(&d, &lib);
    let stone_report = ringdesign_core::stones::report_built(&d, 0.0, &built);
    let stone_count = stone_report.as_ref().map_or(0, |r| r.stone_count as usize);
    let warnings: Vec<_> = stone_report.iter().flat_map(|r| r.seats.iter().flat_map(|s| s.warnings.iter())).cloned().collect();
    let tight = stone_report.as_ref().map_or(0, |r| r.tight_pairs);
    let prepared = mf::prepare(&d, &lib, &setup(&d), params)?;
    let pattern_cross = csg::self_crossings(&solid(&prepared.mesh));
    let pattern_validation = prepared.mesh.validate();
    let pattern_quality = prepared.mesh.quality();
    let mut made = stamp_checks(&d, &lib, params)?;
    made.extend(seat_checks(&d)?);
    let cad_parts = cad_part_checks(&built);
    ensure!(!cad_parts.is_empty(), "The sculpted head was not made");
    made.extend(cad_parts);
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
    let mut cold = Value::Null;
    if verify {
        let t = Instant::now();
        let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
        let read_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let library = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let bake_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let rebuilt = mesh::try_build(&saved, &library, params)?;
        let build_ms = t.elapsed().as_secs_f64() * 1000.0;
        let same = rebuilt.mesh.vertices == built.mesh.vertices
            && rebuilt.mesh.faces == built.mesh.faces
            && rebuilt.mesh.normals == built.mesh.normals;
        cold = json!({"identical_vertices_faces_normals": same, "read_ms": read_ms, "bake_ms": bake_ms, "build_ms": build_ms});
        ensure!(same, "Cold design geometry changed");
    }
    let gates = [
        ("finished mesh watertight, clean, uncrossed, every solid and stamp resolved, bore clear", geometry["pass"] == true),
        ("the same at 384 x 192", coarse["pass"] == true),
        ("every made stamp and seat part closed without crossings or degenerates", made.iter().all(clean_part)),
        ("investment pattern watertight with zero degenerates and crossings", pattern_validation.watertight && pattern_quality.degenerate_faces == 0 && pattern_cross == 0),
        ("lost wax verdict Castable with 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && field.thinnest_wall_mm >= 0.8),
        ("lost-wax land widths at or above 0.8 mm or named", lands_pass(&lands, d.draft.min_detail_mm)),
        ("zero DFM findings", dfm.is_empty()),
        ("one stone in report and preview, no warnings or crowding", stone_count == 1 && gems.len() == 1 && warnings.is_empty() && tight == 0),
        ("cold reload identical", !verify || cold["identical_vertices_faces_normals"] == true),
    ];
    let report = json!({
        "name": d.name,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "faces": built.mesh.faces.len(), "build_ms": built.report.build_ms,
                  "export_note": "The hide's layers carry the Relief reach mask, which keeps imported_base::subdivide from detailing the lower walls by the bore edges; without it 1536 x 448 overruns its 2 million triangle budget"},
        "composition": composition,
        "relief": {"serpent_painted_peak_mm": composition["serpent_painted_peak_mm"], "built_max_relief_mm": built.report.max_relief_mm,
                   "note": "The painted serpent's peak and the finished mesh's highest relief over the bare stock; the layer's 2.0 mm is only its alpha scale"},
        "land_widths": lands,
        "geometry": geometry,
        "coarse_384x192": coarse,
        "mesh": {"validation": built.report.validation, "quality": built.report.quality, "volume_mm3": built.report.volume_mm3},
        "pattern": {"triangles": prepared.mesh.faces.len(), "validation": pattern_validation, "quality": pattern_quality, "self_crossings": pattern_cross, "notes": prepared.notes, "bench_layers": prepared.bench_layers, "scale": prepared.scale},
        "made_parts": made,
        "field": field,
        "dfm": dfm.iter().map(|f| json!({"label": f.label, "message": f.message})).collect::<Vec<_>>(),
        "stones": {"reported": stone_count, "previewed": gems.len(), "warnings": warnings, "tight_pairs": tight},
        "process": {"name": "LostWax", "release_gates": "Not applicable: native upright stock in investment", "draft_clamp": "Not applied to lost wax", "minimum_section_mm": 0.8},
        "gates": gates.iter().map(|(name, pass)| json!({"gate": name, "pass": pass})).collect::<Vec<_>>(),
        "cold_reload": cold,
        "design_bytes": design_bytes,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    if verify {
        std::fs::write(
            out.join("verification.json"),
            serde_json::to_vec_pretty(&json!({"cold_design_reload": cold, "design_bytes": design_bytes, "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps}}))?,
        )?;
    }
    ringdesign_core::stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    ringdesign_core::stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Basiliscus / investment pattern")?;
    for (m, _) in &gems {
        ringdesign_core::stl::write_stl(out.join("reference-tsavorite.stl"), m, "Tsavorite marquise 8 x 4 mm")?;
    }
    let tint = gems.first().map_or([0.025, 0.30, 0.085], |g| g.1);
    std::fs::write(
        out.join("stones.json"),
        serde_json::to_vec_pretty(&json!({"stones": [{"mesh": "reference-tsavorite.stl", "name": "Tsavorite", "tint": tint, "ior": 1.74, "dispersion": 0.028, "roughness": 0.06, "transmission": 0.7}]}))?,
    )?;
    std::fs::create_dir_all(out.join("artwork"))?;
    for name in d.layers.referenced_alphas() {
        if let Some(alpha) = lib.get(name) {
            std::fs::write(out.join("artwork").join(format!("{}.png", name.replace([' ', '\''], "-"))), alpha.to_png16()?)?;
        }
    }
    renders(out, &lib, &built, &gems, params, if draft { 1100 } else { 1600 })?;
    println!(
        "crossings {}, coarse {}, pattern crossings {pattern_cross}, wall {:.3} mm, DFM {}, stones {stone_count}/{}, bore {:.4} mm, {} stamps",
        geometry["self_crossings"],
        coarse["self_crossings"],
        field.thinnest_wall_mm,
        dfm.len(),
        gems.len(),
        geometry["bore_clearance_min_mm"].as_f64().unwrap_or(0.0),
        d.stamps.len()
    );
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    for n in built.solids.notes.iter().chain(&built.parts.notes) {
        println!("  note: {n}");
    }
    for w in &warnings {
        println!("  stone: {w}");
    }
    for r in &lands {
        println!("  land {:.3} mm {}: {}", r["min_land_mm"].as_f64().unwrap_or(0.0), r["name"].as_str().unwrap_or(""), r["status"].as_str().unwrap_or(""));
    }
    println!("  relief: painted peak {:.3} mm, built {:.3} mm", composition["serpent_painted_peak_mm"].as_f64().unwrap_or(0.0), built.report.max_relief_mm);
    for (name, pass) in &gates {
        println!("{}: {name}", if *pass { "pass" } else { "FAIL" });
    }
    let failed: Vec<_> = gates.iter().filter(|g| !g.1).map(|g| g.0).collect();
    ensure!(failed.is_empty(), "Failed gates: {failed:?}");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(k) = args.iter().position(|a| a == "--head") {
        let step = args.get(k + 1).and_then(|s| s.parse().ok()).unwrap_or(0.07);
        return preview_head(Path::new("target/head-preview"), step);
    }
    let out = args
        .iter()
        .find(|s| !s.starts_with("--"))
        .map_or("showcase/bestiarium/basiliscus", String::as_str);
    write(
        Path::new(out),
        args.iter().any(|a| a == "--draft"),
        args.iter().any(|a| a == "--verify"),
    )
}
