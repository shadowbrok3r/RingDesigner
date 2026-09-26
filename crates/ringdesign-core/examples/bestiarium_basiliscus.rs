//! Basiliscus on native escutcheon stock: a crowned serpent's head struck on the shield with a tsavorite
//! set down its brow, and rooster hackles pouring over the shoulders into serpent scale and belly scutes.
use anyhow::{Result, anyhow, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, Mesh, ProfileStyle, RingDesign,
    castability::{self, CastProcess, Verdict},
    csg,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle, Window, smoothstep},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    manufacturing as mf,
    mesh::{self, BuildResult},
    outline,
    render::{self, Part},
    reptile,
    setting::{self, SolidKind, Stamp, StampTop},
    skin::{self, Atlas, Hide, Sample},
};
use serde_json::{Value, json};
use std::{f64::consts::PI, path::Path, time::Instant};

const AW: usize = 2048;
const HACKLE_HEIGHT: f64 = 0.75;
const FLANK_HEIGHT: f64 = 0.40;
const BELLY_HEIGHT: f64 = 0.30;
/// Painted serpent's full height, mm.
const SERPENT_HEIGHT: f64 = 2.0;
/// The pounced field's grain, mm.
const POUNCE_HEIGHT: f64 = 0.07;
/// The bordure's beads, mm.
const BEAD_HEIGHT: f64 = 0.26;
/// The boss the tsavorite is set flush in, mm over the field.
const STONE_BOSS: f64 = 0.9;
/// Relief sampling of the export build.
const EXPORT_THETA: usize = 1536;
/// Mask confining the hide's detail to where it paints: clear of the lower walls by the bore edges.
const REACH: &str = "Relief reach";

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

/// Closed centripetal Catmull-Rom spline through `ctrl`, `per` samples a span.
fn spline(ctrl: &[P2], per: usize) -> Vec<P2> {
    let n = ctrl.len();
    let gap = |a: P2, b: P2| (a[0] - b[0]).hypot(a[1] - b[1]).sqrt().max(1e-6);
    let mut out = Vec::with_capacity(n * per);
    for i in 0..n {
        let p = [ctrl[(i + n - 1) % n], ctrl[i], ctrl[(i + 1) % n], ctrl[(i + 2) % n]];
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
    out
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

fn ellipse(c: P2, rx: f64, rz: f64, rot_deg: f64) -> Vec<P2> {
    let (s, k) = rot_deg.to_radians().sin_cos();
    let pts: Vec<P2> = (0..720)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / 720.0;
            let (x, z) = (rx * a.cos(), rz * a.sin());
            [c[0] + x * k - z * s, c[1] + x * s + z * k]
        })
        .collect();
    resample(&pts, 0.09)
}

/// A lens from `a` to `b`, `half` wide at its middle and `tip` wide at its ends.
fn lens(a: P2, b: P2, half: f64, tip: f64) -> Vec<P2> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = d[0].hypot(d[1]);
    let (u, w) = ([d[0] / l, d[1] / l], [-d[1] / l, d[0] / l]);
    let width = |t: f64| tip + (half - tip) * (1.0 - (2.0 * t - 1.0).powi(2)).max(0.0).powf(0.8);
    let n = 200;
    let mut pts = Vec::with_capacity(2 * n + 2);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let h = width(t);
        pts.push([a[0] + u[0] * l * t + w[0] * h, a[1] + u[1] * l * t + w[1] * h]);
    }
    for k in 0..=n {
        let t = 1.0 - k as f64 / n as f64;
        let h = width(t);
        pts.push([a[0] + u[0] * l * t - w[0] * h, a[1] + u[1] * l * t - w[1] * h]);
    }
    ccw(resample(&pts, 0.09))
}

/// A fang from `root` to `tip`, `half` wide at the root and bowed `bend` to its left, sharp at the tip.
fn fang(root: P2, tip: P2, half: f64, bend: f64) -> Vec<P2> {
    let d = [tip[0] - root[0], tip[1] - root[1]];
    let l = d[0].hypot(d[1]);
    let (u, w) = ([d[0] / l, d[1] / l], [-d[1] / l, d[0] / l]);
    let at = |t: f64| {
        let b = bend * 4.0 * t * (1.0 - t);
        [root[0] + u[0] * l * t + w[0] * b, root[1] + u[1] * l * t + w[1] * b]
    };
    let width = |t: f64| 0.04 + (half - 0.04) * (1.0 - t).powf(0.85);
    let n = 160;
    let mut pts = Vec::with_capacity(3 * n);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let c = at(t);
        pts.push([c[0] + w[0] * width(t), c[1] + w[1] * width(t)]);
    }
    for k in (0..=n).rev() {
        let t = k as f64 / n as f64;
        let c = at(t);
        pts.push([c[0] - w[0] * width(t), c[1] - w[1] * width(t)]);
    }
    for k in 1..n {
        let a = PI * k as f64 / n as f64;
        let (c, s) = (a.cos(), a.sin());
        pts.push([root[0] - w[0] * half * c - u[0] * half * s, root[1] - w[1] * half * c - u[1] * half * s]);
    }
    ccw(resample(&pts, 0.09))
}

/// A convex quad's corners in counter-clockwise order round their centre.
fn around(mut corners: Vec<P2>) -> Vec<P2> {
    let c = centroid(&corners);
    corners.sort_by(|p, q| {
        (p[1] - c[1])
            .atan2(p[0] - c[0])
            .total_cmp(&(q[1] - c[1]).atan2(q[0] - c[0]))
    });
    corners
}

fn plate(corners: Vec<P2>, radius: f64) -> Vec<P2> {
    ccw(outline::rounded_polygon(&around(corners), radius))
}

fn centroid(poly: &[P2]) -> P2 {
    let n = poly.len() as f64;
    [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n]
}

/// One stamp as drawn in world millimetres on the face: `x` round the ring, `z` along the finger.
struct Shape {
    name: String,
    centre: P2,
    poly: Vec<P2>,
    lo: P2,
    hi: P2,
    height: f64,
    sink: f64,
    draft: f64,
    tier: u8,
    top: StampTop,
    cut: bool,
}

impl Shape {
    #[allow(clippy::too_many_arguments)]
    fn new(name: impl Into<String>, centre: P2, poly: Vec<P2>, height: f64, sink: f64, draft: f64, tier: u8, top: StampTop, cut: bool) -> Self {
        let lo = [
            poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min),
            poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min),
        ];
        let hi = [
            poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max),
            poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max),
        ];
        Self { name: name.into(), centre, poly, lo, hi, height, sink, draft, tier, top, cut }
    }

    /// Distance to the outline, positive inside, `-cap` anywhere further than `cap` outside its box.
    fn sdf(&self, p: P2, cap: f64) -> f64 {
        if p[0] < self.lo[0] - cap || p[0] > self.hi[0] + cap || p[1] < self.lo[1] - cap || p[1] > self.hi[1] + cap {
            return -cap;
        }
        let n = self.poly.len();
        let mut best = f64::MAX;
        let mut inside = false;
        for i in 0..n {
            let (a, b) = (self.poly[i], self.poly[(i + 1) % n]);
            let e = [b[0] - a[0], b[1] - a[1]];
            let w = [p[0] - a[0], p[1] - a[1]];
            let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-18)).clamp(0.0, 1.0);
            best = best.min((w[0] - e[0] * t).hypot(w[1] - e[1] * t));
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * e[0] {
                inside = !inside;
            }
        }
        if inside { best } else { -best.min(cap) }
    }
}

/// The tsavorite's centre on the face; its long axis runs round the ring.
const STONE: P2 = [-0.3, 1.6];
/// The serpent's spine from under its head, round the stone, to the tail at the shield's point.
const SPINE: [P2; 11] = [
    [3.0, -2.6],
    [4.3, -1.3],
    [5.1, 1.0],
    [4.6, 3.6],
    [2.2, 5.1],
    [-1.8, 5.1],
    [-4.1, 5.7],
    [-2.8, 6.9],
    [-0.6, 7.25],
    [0.55, 7.8],
    [0.1, 8.45],
];
/// Half-width of the body at the neck and at the tail, mm.
const GIRTH: (f64, f64) = (1.3, 0.3);
/// The body's height over its spine, mm.
const TUBE: f64 = 1.1;
/// The head in profile under the chief, facing round the ring toward -x, jaws open.
const HEAD: [P2; 25] = [
    [3.6, -3.4],
    [3.2, -4.6],
    [2.2, -5.3],
    [0.8, -5.5],
    [-0.7, -5.35],
    [-2.0, -5.0],
    [-3.1, -4.6],
    [-3.9, -4.3],
    [-4.35, -3.95],
    [-4.2, -3.6],
    [-3.1, -3.45],
    [-1.9, -3.25],
    [-0.7, -2.95],
    [0.35, -2.6],
    [-0.8, -2.4],
    [-2.0, -2.15],
    [-3.1, -1.85],
    [-3.85, -1.62],
    [-3.65, -1.35],
    [-2.3, -1.3],
    [-0.8, -1.4],
    [0.7, -1.5],
    [2.0, -1.55],
    [3.0, -2.0],
    [3.55, -2.7],
];
/// The crown band on the head: its ends round the ring and its chief and lower edges.
const CROWN_BAND: [f64; 4] = [-1.9, 1.9, -5.95, -5.3];
/// The crown's points: centre round the ring, tip, half-width at the base.
const CROWN_POINTS: [(f64, f64, f64); 3] = [(-1.3, -6.7, 0.5), (0.0, -7.05, 0.58), (1.3, -6.7, 0.5)];
const EYE_AT: P2 = [-1.4, -4.2];

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
    lo: P2,
    hi: P2,
}

impl Body {
    fn new() -> Self {
        let spine = open_spline(&SPINE, 120);
        let mut at = vec![0.0];
        for w in spine.windows(2) {
            at.push(at.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
        }
        let lo = [spine.iter().map(|p| p[0]).fold(f64::MAX, f64::min) - GIRTH.0, spine.iter().map(|p| p[1]).fold(f64::MAX, f64::min) - GIRTH.0];
        let hi = [spine.iter().map(|p| p[0]).fold(f64::MIN, f64::max) + GIRTH.0, spine.iter().map(|p| p[1]).fold(f64::MIN, f64::max) + GIRTH.0];
        Self { spine, at, lo, hi }
    }

    fn length(&self) -> f64 {
        *self.at.last().unwrap()
    }

    fn girth(&self, s: f64) -> f64 {
        GIRTH.0 + (GIRTH.1 - GIRTH.0) * (s / self.length()).clamp(0.0, 1.0).powf(0.9)
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

    /// Painted body height, mm: a rounded tube of keeled dorsal scales with belly scutes down one side.
    fn height(&self, p: P2) -> f64 {
        let Some((d, s, lat)) = self.nearest(p) else { return 0.0 };
        let w = self.girth(s);
        if d >= w {
            return 0.0;
        }
        let tube = (1.0 - (d / w).powi(2)).sqrt();
        let rise = 0.55 + 0.45 * smooth(0.0, 3.0, s);
        let across = lat / w;
        let belly = smooth(-0.22, -0.42, across);
        let scales = reptile::snake(s / 0.85, lat / 0.62);
        let scutes = reptile::ventral(s / 0.6, ((across + 0.71) / 0.29).clamp(-1.0, 1.0));
        let skin = scales * (1.0 - belly) + scutes * belly;
        TUBE * tube * rise * (0.78 + 0.22 * skin) + 0.3 * smooth(0.0, 0.12, w - d)
    }
}

/// Distance from `p` to the segment `a`–`b`, and how far along it the nearest point lies.
fn seg_dist(p: P2, a: P2, b: P2) -> (f64, f64) {
    let e = [b[0] - a[0], b[1] - a[1]];
    let w = [p[0] - a[0], p[1] - a[1]];
    let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-18)).clamp(0.0, 1.0);
    ((w[0] - e[0] * t).hypot(w[1] - e[1] * t), t)
}

/// The tsavorite's plan radius at `p`, 1 on its girdle.
fn stone_q(p: P2) -> f64 {
    (((p[0] - STONE[0]).abs() / 4.0).powf(1.5) + ((p[1] - STONE[1]).abs() / 2.0).powf(1.5)).powf(1.0 / 1.5)
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

/// The arms on the shield: a crowned serpent erect and undulating, devouring the tsavorite, on a diapered field.
struct Arms {
    table: Shape,
    body: Body,
    shapes: Vec<Shape>,
    head: Shape,
    mouth: Shape,
    beads: Vec<P2>,
}

/// The bordure's beads: inset from the table's edge, pitch and radius, mm.
const BEADS: (f64, f64, f64) = (0.62, 0.66, 0.22);

/// Pitch of the pounced field's grain, mm.
const GRAIN: f64 = 0.62;

impl Arms {
    fn new(table: Vec<P2>) -> Result<Self> {
        let flat = StampTop::Flat;
        let dome = |h: f64| StampTop::Dome { crown_mm: h };
        let mut s = Vec::new();
        let head = ccw(resample(&spline(&HEAD, 40), 0.09));
        let head = Shape::new("Basilisk's head", centroid(&head), head, 0.0, 0.0, 0.0, 0, flat, false);
        let [x0, x1, chief, low] = CROWN_BAND;
        let band = plate(vec![[x0, low], [x1, low], [x1 - 0.15, chief], [x0 + 0.15, chief]], 0.12);
        s.push(Shape::new("Crown", [(x0 + x1) * 0.5, (chief + low) * 0.5], band, 0.7, 0.3, 2.0, 0, flat, false));
        for (k, &(x, tip, half)) in CROWN_POINTS.iter().enumerate() {
            let base = chief + 0.15;
            let corners = [[x - half, base], [x, tip], [x + half, base]];
            let apex = [x, base + 0.4 * (tip - base)];
            s.push(Shape::new(format!("Crown point, {}", k + 1), apex, ccw(outline::rounded_polygon(&corners, 0.08)), 0.65, 0.3, 2.0, 0, StampTop::Cone { apex_mm: 0.85, at: [0.0, 0.0], tip_mm: 0.0 }, false));
            let c = [x, tip + 0.18];
            s.push(Shape::new(format!("Crown pearl, {}", k + 1), c, ellipse(c, 0.34, 0.34, 0.0), 0.75, 0.3, 2.0, 0, dome(0.32), false));
        }
        let fangs: [(P2, P2, f64); 2] = [([-3.2, -3.62], [-3.15, -2.9], 0.2), ([-2.9, -1.72], [-2.9, -2.35], 0.18)];
        for (k, (root, tip, half)) in fangs.into_iter().enumerate() {
            let axis = (tip[1] - root[1]).atan2(-(tip[0] - root[0])).to_degrees();
            s.push(Shape::new(format!("Fang, {}", k + 1), [(root[0] + tip[0]) * 0.5, (root[1] + tip[1]) * 0.5], fang(root, tip, half, 0.03), 0.45, 0.3, 2.0, 0, StampTop::Taper { axis_deg: axis, tip_mm: 0.15 }, false));
        }
        let fork = outline::fork(2.4, 44.0, 0.36, 0.26);
        let tongue: Vec<P2> = fork.iter().map(|p| [-5.0 - p[0], -2.95 - p[1]]).collect();
        s.push(Shape::new("Forked tongue", [-5.0, -2.95], ccw(resample(&tongue, 0.09)), 0.45, 0.3, 0.0, 0, flat, false));
        s.push(Shape::new("Eye", EYE_AT, ellipse(EYE_AT, 0.72, 0.52, -8.0), 0.15, 0.3, 2.0, 0, dome(0.38), false));
        s.push(Shape::new("Pupil", [EYE_AT[0] + 1.2, EYE_AT[1]], lens([EYE_AT[0], EYE_AT[1] - 0.44], [EYE_AT[0], EYE_AT[1] + 0.44], 0.13, 0.03), 0.4, 0.25, 0.0, 1, flat, true));
        s.push(Shape::new("Brow", [-1.4, -4.8], lens([-2.45, -4.72], [-0.35, -4.87], 0.2, 0.05), 0.18, 0.3, 2.0, 0, dome(0.22), false));
        let c = [-3.75, -4.05];
        s.push(Shape::new("Nostril", c, ellipse(c, 0.2, 0.12, -20.0), 0.3, 0.22, 0.0, 0, flat, true));
        for (k, x) in [-0.65, 0.65].into_iter().enumerate() {
            let c = [x, -5.62];
            s.push(Shape::new(format!("Crown jewel, {}", k + 1), c, ellipse(c, 0.24, 0.15, 0.0), 0.08, 0.3, 0.0, 1, dome(0.14), false));
        }
        let scales: [(&str, P2, f64, f64, f64); 5] = [
            ("Upper lip scale, 1", [-3.3, -3.83], 0.44, 0.21, -8.0),
            ("Upper lip scale, 2", [-2.4, -3.62], 0.44, 0.21, -10.0),
            ("Upper lip scale, 3", [-0.35, -3.27], 0.44, 0.21, -16.0),
            ("Lower lip scale, 1", [-2.2, -1.8], 0.46, 0.19, 14.0),
            ("Lower lip scale, 2", [-1.1, -1.95], 0.46, 0.19, 10.0),
        ];
        for (name, c, rx, rz, rot) in scales {
            s.push(Shape::new(name, c, ellipse(c, rx, rz, rot), 0.12, 0.3, 4.0, 0, dome(0.06), false));
        }
        for sh in &s {
            let local: Vec<P2> = sh.poly.iter().map(|p| [p[0] - sh.centre[0], p[1] - sh.centre[1]]).collect();
            outline::check(&ccw(local)).map_err(|e| anyhow!("{}: {e}", sh.name))?;
        }
        let mouth = Shape::new("Mouth", [0.0, 0.0], ccw(resample(&[[0.35, -2.6], [-4.3, -3.75], [-3.95, -1.55]], 0.09)), 0.0, 0.0, 0.0, 0, flat, false);
        let mut arms = Self { table: Shape::new("Table", [0.0, 0.0], table, 0.0, 0.0, 0.0, 0, flat, false), body: Body::new(), shapes: s, head, mouth, beads: Vec::new() };
        arms.beads = arms.bordure();
        Ok(arms)
    }

    fn named(&self, name: &str) -> &Shape {
        self.shapes.iter().find(|s| s.name == name).unwrap()
    }

    /// Tier-0 stamps that raise metal, the ground the painted relief keeps clear of.
    fn ground(&self) -> impl Iterator<Item = &Shape> {
        self.shapes.iter().filter(|s| s.tier == 0 && !s.cut)
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

    /// Distance outside every piece of struck ground, mm; negative inside one.
    fn clear_of_ground(&self, p: P2) -> f64 {
        self.ground().map(|s| -s.sdf(p, 2.0)).fold(-self.head.sdf(p, 2.0), f64::min)
    }

    /// The serpent painted over the table, mm: its body, the rounded skull under the head and the crown band's swell.
    fn serpent(&self, p: P2) -> f64 {
        if self.table.sdf(p, 1.0) <= 0.0 {
            return 0.0;
        }
        let dh = self.head.sdf(p, 2.0);
        let skull = if dh > -0.1 { (0.65 + 0.55 * smooth(0.0, 1.3, dh).powf(0.6)) * smooth(-0.1, 0.15, dh) } else { 0.0 };
        let band = self.named("Crown").sdf(p, 1.0);
        let swell = if band > 0.0 {
            let [_, _, chief, low] = CROWN_BAND;
            let u = ((p[1] - chief) / (low - chief)).clamp(0.0, 1.0);
            (0.35 + 0.25 * (PI * u).sin().powf(0.6)) * smooth(0.12, 0.4, band)
        } else {
            0.0
        };
        let body = self.body.height(p) * (1.0 - smooth(0.0, 0.5, dh));
        skull.max(swell).max(body)
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
            let clear = beads.last().is_none_or(|b| (b[0] - w[1][0]).hypot(b[1] - w[1][1]) > 0.95 * pitch);
            if run >= pitch && clear {
                run = 0.0;
                let q = w[1];
                let body = self.body.nearest(q).map_or(f64::MAX, |(d, s, _)| d - self.body.girth(s));
                if self.clear_of_ground(q) > radius + 0.3 && body > radius + 0.3 && -self.mouth.sdf(q, 1.0) > radius + 0.3 {
                    beads.push(q);
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

    /// The field matted with a jittered grain of punched pits, 0..1: a low plateau kept off the arms.
    fn pounce(&self, p: P2) -> f64 {
        let dt = self.table.sdf(p, 1.0);
        if dt <= 0.0 {
            return 0.0;
        }
        let clear = self.clear_of_ground(p).min(self.body.nearest(p).map_or(f64::MAX, |(d, s, _)| d - self.body.girth(s))).min(-self.mouth.sdf(p, 1.0));
        let plateau = smooth(0.35, 0.6, clear) * smooth(1.3, 1.5, stone_q(p)) * smooth(1.05, 1.3, dt);
        if plateau <= 0.0 {
            return 0.0;
        }
        let (i0, j0) = ((p[0] / GRAIN).floor() as i64, (p[1] / GRAIN).floor() as i64);
        let mut pit: f64 = 0.0;
        for i in i0 - 1..=i0 + 1 {
            for j in j0 - 1..=j0 + 1 {
                let c = [
                    (i as f64 + 0.5 + 0.12 * (skin::hash(i, j) - 0.5)) * GRAIN,
                    (j as f64 + 0.5 + 0.12 * (skin::hash(j + 7919, i) - 0.5)) * GRAIN,
                ];
                pit = pit.max(1.0 - smooth(0.1, 0.15, (p[0] - c[0]).hypot(p[1] - c[1])));
            }
        }
        plateau * (1.0 - pit)
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

/// Every face stamp placed on the ring, its outline read in the frame the build stands it in.
fn strike(d: &mut RingDesign, a: &Atlas, face: &Arms) -> Result<Vec<Value>> {
    let first = d.stamps.len();
    for sh in &face.shapes {
        let (theta_deg, v_mm) = chart(a, sh.centre)?;
        d.stamps.push(Stamp {
            name: sh.name.clone(),
            theta_deg,
            v_mm,
            rot_deg: 0.0,
            outline: outline::circle(0.6),
            height_mm: sh.height,
            sink_mm: sh.sink,
            draft_deg: sh.draft,
            cut: sh.cut,
            bench: false,
            along_pull: false,
            tier: sh.tier,
            top: sh.top,
        });
    }
    let ctx = d.field_context();
    let frames: Vec<_> = (first..d.stamps.len()).map(|k| d.stamps[k].frame(d, &ctx)).collect();
    let mut placed = Vec::new();
    for (k, (sh, f)) in face.shapes.iter().zip(&frames).enumerate() {
        let local: Vec<P2> = sh
            .poly
            .iter()
            .map(|p| {
                let w = [p[0] - f.origin[0], a.top - f.origin[1], p[1] - f.origin[2]];
                [w[0] * f.x[0] + w[1] * f.x[1] + w[2] * f.x[2], w[0] * f.y[0] + w[1] * f.y[1] + w[2] * f.y[2]]
            })
            .collect();
        ensure!((f.x[0] + 1.0).abs() < 1e-6 && (f.y[2] - 1.0).abs() < 1e-6, "{} stands off the table's frame", sh.name);
        let local = if area(&local) < 0.0 { local.into_iter().rev().collect() } else { local };
        outline::check(&local).map_err(|e| anyhow!("{}: {e}", sh.name))?;
        d.stamps[first + k].outline = local;
        let off = (f.origin[0] - sh.centre[0]).hypot(f.origin[2] - sh.centre[1]);
        placed.push(json!({"name": sh.name, "tier": sh.tier, "cut": sh.cut, "points": sh.poly.len(), "origin_error_mm": off}));
    }
    let ctx = d.field_context();
    for (k, f) in frames.iter().enumerate() {
        let g = d.stamps[first + k].frame(d, &ctx);
        let moved = (0..3).map(|i| (g.origin[i] - f.origin[i]).abs() + (g.x[i] - f.x[i]).abs()).sum::<f64>();
        ensure!(moved < 1e-9, "{} moved with its outline", d.stamps[first + k].name);
    }
    Ok(placed)
}

fn round_scales(row: f64, col: f64) -> f64 {
    let mut best: f64 = 0.0;
    let i0 = row.floor() as i64;
    for i in [i0 - 1, i0] {
        let stagger = if i.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let centre = (col - stagger).round() + stagger;
        let t = (row - i as f64) / 1.55;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let dc = (col - centre).abs();
        let half = 0.56 * (1.0 - t.powf(2.4)).max(0.0).powf(0.6);
        if dc >= half {
            continue;
        }
        best = best.max(
            smoothstep(0.0, 0.22, half - dc) * smoothstep(0.0, 0.16, 1.0 - t) * (0.35 + 0.65 * t),
        );
    }
    best
}

/// Lanceolate vanes overlap away from the head and become short keeled scales.
fn hackle_scale(along: f64, across: f64, morph: f64) -> (f64, f64) {
    let pitch = (1.5 + 0.8 * smooth(0.0, 8.0, along)) * (1.0 - morph) + 1.2 * morph;
    let aspect = 3.2 * (1.0 - morph) + 1.2 * morph;
    let length = pitch * aspect;
    let step = length * 0.70;
    let fan = 1.0 + 0.065 * along * (1.0 - morph);
    let cross = across / fan;
    let row = (cross / pitch).round() as i64;
    let mut height: f64 = 0.0;
    let mut barbs: f64 = 0.0;
    for j in row - 1..=row + 1 {
        let y = cross - j as f64 * pitch;
        let offset = j.rem_euclid(2) as f64 * 0.5 * step;
        let i0 = ((along - offset) / step).floor() as i64;
        for i in i0 - 1..=i0 {
            let t = (along - offset - i as f64 * step) / length;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let half = 0.125 + (0.53 * pitch - 0.125) * (PI * t).sin().max(0.0).powf(0.68);
            let edge = 1.0 - smooth((half - 0.23).max(0.0), half, y.abs());
            let tip = 1.0 - smooth(0.90, 1.0, t);
            let root = smooth(0.0, 0.13, t);
            let vane = 0.44 + 0.33 * smooth(0.0, 0.86, t) - 0.12 * (y / half.max(0.1)).powi(2);
            let rachis = 0.11 * (1.0 - smooth(0.04, 0.19, y.abs())) * smooth(0.05, 0.25, t);
            let h = (vane + rachis) * edge * tip * root;
            if h > height {
                height = h;
                let line = ((t * length - 0.58 * y.abs()) / 0.80).rem_euclid(1.0);
                barbs = (1.0 - smooth(0.04, 0.17, (line - 0.5).abs()))
                    * smooth(0.10, 0.25, y.abs())
                    * edge
                    * tip
                    * root;
            }
        }
    }
    (height, barbs)
}

fn skin_masks(a: &Atlas, hide: &Hide, s: &Sample) -> (f64, f64, f64) {
    let h = hide.at(s);
    let over_bore = smooth(a.bore + 1.15, a.bore + 1.65, s.p[0].hypot(s.p[1]));
    let edge = h.rim - h.across.abs();
    let crown = smooth(-0.25, 0.55, edge) * (1.0 - smooth(0.52, 0.80, s.n[2].abs()));
    let crease = smooth(0.40, 1.00, edge.abs());
    let neck = smooth(38.0, 46.0, s.theta)
        * (1.0 - smooth(64.0, 72.0, s.theta))
        * (1.0 - smooth(0.30, 0.75, (s.p[2] + 3.05).abs()));
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

fn author(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Value)> {
    let mut d = base()?;
    d.build = params;
    let ctx = d.field_context();
    let ah = (AW as f64 * ctx.band_v_len_mm / ctx.circumference_mm).round() as usize;
    let a = Atlas::of(&d, AW, ah)?;
    let hide = Hide::of(&a);
    let arms = Arms::new(table_outline(&a))?;
    let l70 = hide.along[(AW as f64 * 160.0 / 360.0).round() as usize].abs();
    let l110 = hide.along[(AW as f64 * 200.0 / 360.0).round() as usize].abs();
    let on_table = |s: &Sample| s.n[1] > 0.95 && s.p[1] > a.top - 0.05;
    let near_table = |s: &Sample| s.n[1] > 0.3 && s.p[1] > a.top - 1.2;
    let off_table = a.paint("off", |s| if near_table(s) { smooth(0.05, 0.45, -arms.table.sdf([s.p[0], s.p[2]], 1.0)) } else { 1.0 });
    let width: Vec<f64> = (0..=400).map(|k| arms.table_width_at(-10.0 + 0.05 * k as f64)).collect();
    let width_at = |z: f64| width[((z + 10.0) / 0.05).round().clamp(0.0, 400.0) as usize];
    let mut lib = AlphaLibrary::builtin();
    let reach = a.paint(REACH, |s| smooth(a.bore + 1.0, a.bore + 1.1, s.p[0].hypot(s.p[1])));
    lib.insert(Alpha::from_png16(REACH, &reach.to_png16()?)?);
    let feather = |s: &Sample| {
        let h = hide.at(s);
        let (_, _, along) = skin_masks(&a, &hide, s);
        let m = smooth(l70, l110, along);
        let near = 1.0 - smooth(9.0, 14.0, along);
        let root = (along - width_at(s.p[2]) * near - 7.0 * (1.0 - near) - 0.2).max(0.0);
        hackle_scale(root, h.across, m)
    };
    let alpha = a.paint("Basiliscus", |s| if on_table(s) { arms.serpent([s.p[0], s.p[2]]) / SERPENT_HEIGHT } else { 0.0 });
    portable(&mut d, &mut lib, alpha, SERPENT_HEIGHT, window(90.0, 70.0), false, None)?;
    let alpha = a.paint("Pounced field", |s| if on_table(s) { arms.pounce([s.p[0], s.p[2]]) } else { 0.0 });
    let mut face = Window::around(90.0, 64.0);
    face.fade_deg = 1.0;
    portable(&mut d, &mut lib, alpha, POUNCE_HEIGHT, face, false, None)?;
    let alpha = a.paint("Beaded bordure", |s| if on_table(s) { arms.bordure_height([s.p[0], s.p[2]]) } else { 0.0 });
    portable(&mut d, &mut lib, alpha, BEAD_HEIGHT, face, false, None)?;
    let alpha = a.paint("Hackles into scales", |s| {
        let (bore, crown, _) = skin_masks(&a, &hide, s);
        feather(s).0 * crown * bore * off_table.data[s.i] as f64
    });
    portable(&mut d, &mut lib, alpha, HACKLE_HEIGHT, window(90.0, 240.0), false, Some(REACH))?;
    let alpha = a.paint("Serpent flanks", |s| {
        let h = hide.at(s);
        let (bore, crown, along) = skin_masks(&a, &hide, s);
        let m = smooth(l70, l110 + 5.0, along);
        let rounds = round_scales(along / 2.1, h.across / 2.8);
        let shields = reptile::shields(along / 2.5, h.across / 2.8);
        (rounds * (1.0 - m) + shields * m) * (1.0 - crown) * bore * off_table.data[s.i] as f64
    });
    portable(&mut d, &mut lib, alpha, FLANK_HEIGHT, window(180.0, 360.0), false, Some(REACH))?;
    let far = hide.reach();
    let alpha = a.paint("Belly scutes", |s| {
        let h = hide.at(s);
        let (bore, crown, _) = skin_masks(&a, &hide, s);
        reptile::ventral((far - h.along.abs()) / 2.8, h.across / 2.0) * crown * bore
    });
    portable(&mut d, &mut lib, alpha, BELLY_HEIGHT, window(270.0, 110.0), false, Some(REACH))?;
    let alpha = a.paint("Graver's barbs and keels", |s| {
        let (bore, crown, along) = skin_masks(&a, &hide, s);
        feather(s).1 * crown * bore * off_table.data[s.i] as f64 * (1.0 - smooth(l110, l110 + 5.0, along))
    });
    portable(&mut d, &mut lib, alpha, 0.065, window(90.0, 240.0), true, Some(REACH))?;
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
        blend_mm: 0.45,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        rot_deg: 0.0,
        ..Default::default()
    };
    seat.fit_stone(gem);
    seat.height_mm = STONE_BOSS;
    let mut e = LayerEntry::new("Tsavorite, flush", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    let placed = strike(&mut d, &a, &arms)?;
    let composition = json!({
        "stone_mm": STONE, "stone_boss_mm": STONE_BOSS, "spine_mm": arms.body.length(), "girth_mm": [GIRTH.0, GIRTH.1], "tube_mm": TUBE,
        "serpent_height_mm": SERPENT_HEIGHT, "pounce_height_mm": POUNCE_HEIGHT, "bead_height_mm": BEAD_HEIGHT, "beads": arms.beads.len(),
        "table_points": arms.table.poly.len(), "table_z_mm": [arms.table.lo[1], arms.table.hi[1]], "table_x_mm": [arms.table.lo[0], arms.table.hi[0]],
        "morph_start_along_mm": l70, "morph_end_along_mm": l110, "stock": "020 native unmirrored", "atlas": [AW, ah],
        "stamps": placed,
    });
    Ok((d, lib, composition))
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
    let head = crop(&built.mesh, [0.0, 13.3, 0.6], 11.0);
    let mut close = vec![Part::metal(&head, render::GOLD)];
    close.extend(stone_parts());
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
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
    let (d, lib, composition) = author(params)?;
    let authored_s = start.elapsed().as_secs_f64();
    let built = mesh::try_build(&d, &lib, params)?;
    println!("built {} faces in {:.1} s (authored in {authored_s:.1} s); {:?}", built.mesh.faces.len(), start.elapsed().as_secs_f64(), built.solids.notes);
    let geometry = mesh_gates(&d, &built);
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
        ("zero DFM findings", dfm.is_empty()),
        ("one stone in report and preview, no warnings or crowding", stone_count == 1 && gems.len() == 1 && warnings.is_empty() && tight == 0),
        ("cold reload identical", !verify || cold["identical_vertices_faces_normals"] == true),
    ];
    let report = json!({
        "name": d.name,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "faces": built.mesh.faces.len(), "build_ms": built.report.build_ms,
                  "export_note": "The hide's layers carry the Relief reach mask, which keeps imported_base::subdivide from detailing the lower walls by the bore edges; without it 1536 x 448 overruns its 2 million triangle budget"},
        "composition": composition,
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
    for (name, pass) in &gates {
        println!("{}: {name}", if *pass { "pass" } else { "FAIL" });
    }
    let failed: Vec<_> = gates.iter().filter(|g| !g.1).map(|g| g.0).collect();
    ensure!(failed.is_empty(), "Failed gates: {failed:?}");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
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
