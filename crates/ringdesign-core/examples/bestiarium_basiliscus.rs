//! Basiliscus on native escutcheon stock: a crowned serpent in profile under the chief, its jaws open on a
//! forked tongue, coiled round a flush tsavorite on a pitted field inside a beaded bordure, with feathered
//! mantling pouring over the shoulders into serpent scale and shingled belly scutes across the palm.
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
/// Scale of the painted serpent layer, mm.
const SERPENT_HEIGHT: f64 = 2.0;
/// Depth of the punched pits, mm.
const PIT_DEPTH: f64 = 0.09;
/// The bordure's beads, mm.
const BEAD_HEIGHT: f64 = 0.26;
/// The boss the tsavorite is set flush in, mm over the field.
const STONE_BOSS: f64 = 0.9;
/// Metal left round the tsavorite's plan in its boss, mm.
const STONE_STOCK: f64 = 2.1;
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
    /// Narrowest land in plan, mm, and the bench treatment that names it when under the floor.
    land: f64,
    note: Option<&'static str>,
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
        Self { name: name.into(), centre, poly, lo, hi, height, sink, draft, tier, top, cut, land: 0.0, note: None }
    }

    fn land(mut self, land: f64, note: Option<&'static str>) -> Self {
        self.land = land;
        self.note = note;
        self
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

/// A strip `half` wide either side of an open polyline, its ends rounded.
fn stroke(line: &[P2], half: f64) -> Vec<P2> {
    let n = line.len();
    let dir = |i: usize| {
        let (a, b) = (line[i.saturating_sub(1)], line[(i + 1).min(n - 1)]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
        [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
    };
    let cap = |c: P2, t: P2, pts: &mut Vec<P2>| {
        for j in 1..24 {
            let a = -PI * 0.5 + PI * j as f64 / 24.0;
            let (s, co) = a.sin_cos();
            pts.push([c[0] + (t[0] * co - t[1] * s) * half, c[1] + (t[1] * co + t[0] * s) * half]);
        }
    };
    let mut pts = Vec::with_capacity(2 * n + 48);
    for i in 0..n {
        let t = dir(i);
        pts.push([line[i][0] + t[1] * half, line[i][1] - t[0] * half]);
    }
    cap(line[n - 1], dir(n - 1), &mut pts);
    for i in (0..n).rev() {
        let t = dir(i);
        pts.push([line[i][0] - t[1] * half, line[i][1] + t[0] * half]);
    }
    let t = dir(0);
    cap(line[0], [-t[0], -t[1]], &mut pts);
    ccw(resample(&pts, 0.04))
}

/// A forked tongue along `line`: a stem `stem` wide that forks over its last `fork` into two tines `tine` wide and
/// `spread` degrees apart. Each piece is its own outline with the ends of its ridge: stem, then the two tines.
fn tongue(line: &[P2], fork: f64, stem: f64, tine: f64, spread: f64) -> Vec<(Vec<P2>, P2, P2, Vec<(P2, P2)>)> {
    let path = open_spline(line, 60);
    let mut at = vec![0.0];
    for w in path.windows(2) {
        at.push(at.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let k = at.iter().position(|&s| s >= at.last().unwrap() - fork).unwrap_or(path.len() - 1).max(2);
    let f = path[k];
    let t = {
        let a = path[k - 2];
        let l = (f[0] - a[0]).hypot(f[1] - a[1]).max(1e-12);
        [(f[0] - a[0]) / l, (f[1] - a[1]) / l]
    };
    let turn = |v: P2, deg: f64| {
        let (s, c) = deg.to_radians().sin_cos();
        [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
    };
    let mut stem_line = path[..=k].to_vec();
    stem_line.push([f[0] + t[0] * 0.12, f[1] + t[1] * 0.12]);
    let bones: Vec<(P2, P2)> = path[..=k].windows(2).map(|w| (w[0], w[1])).collect();
    let crest = [f[0] - t[0] * 0.06, f[1] - t[1] * 0.06];
    let mut out = vec![(stroke(&stem_line, stem * 0.5), path[0], crest, bones)];
    for side in [1.0, -1.0] {
        let d = turn(t, side * spread * 0.5);
        let from = [f[0] - d[0] * 0.06, f[1] - d[1] * 0.06];
        let ridge = [f[0] + d[0] * 0.04, f[1] + d[1] * 0.04];
        let end = [f[0] + d[0] * (fork - tine * 0.5), f[1] + d[1] * (fork - tine * 0.5)];
        out.push((stroke(&[from, end], tine * 0.5), ridge, end, vec![(ridge, end)]));
    }
    out
}

/// Box-blurs a `w` x `h` grid in place, wrapping round the ring and clamping across it.
fn blur(data: &mut [f32], w: usize, h: usize, r: usize, passes: usize) {
    let mut line = vec![0.0f32; w.max(h)];
    let span = (2 * r + 1) as f32;
    for _ in 0..passes {
        for y in 0..h {
            let row = &mut data[y * w..(y + 1) * w];
            let mut acc: f32 = (0..=2 * r).map(|k| row[(k + w - r) % w]).sum();
            for x in 0..w {
                line[x] = acc / span;
                acc += row[(x + r + 1) % w] - row[(x + w - r) % w];
            }
            row.copy_from_slice(&line[..w]);
        }
        for x in 0..w {
            let at = |d: &[f32], y: i64| d[(y.clamp(0, h as i64 - 1) as usize) * w + x];
            let mut acc: f32 = (-(r as i64)..=r as i64).map(|y| at(data, y)).sum();
            for y in 0..h {
                line[y] = acc / span;
                acc += at(data, y as i64 + r as i64 + 1) - at(data, y as i64 - r as i64);
            }
            for y in 0..h {
                data[y * w + x] = line[y];
            }
        }
    }
}

/// Whether ring angle `theta` lies within `half` degrees of `centre`.
fn within(theta: f64, centre: f64, half: f64) -> bool {
    (theta - centre + 180.0).rem_euclid(360.0) - 180.0 <= half && (theta - centre + 180.0).rem_euclid(360.0) - 180.0 >= -half
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

/// A fang from `root` to `tip`, `half` wide at the root, bowed `bend` to its left and rounded to `point` at the tip.
fn fang(root: P2, tip: P2, half: f64, bend: f64, point: f64) -> Vec<P2> {
    let d = [tip[0] - root[0], tip[1] - root[1]];
    let l = d[0].hypot(d[1]);
    let (u, w) = ([d[0] / l, d[1] / l], [-d[1] / l, d[0] / l]);
    let at = |t: f64| {
        let b = bend * 4.0 * t * (1.0 - t);
        [root[0] + u[0] * l * t + w[0] * b, root[1] + u[1] * l * t + w[1] * b]
    };
    let dir = |t: f64| {
        let (a, b) = (at((t - 0.01).max(0.0)), at((t + 0.01).min(1.0)));
        let m = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
        [(b[0] - a[0]) / m, (b[1] - a[1]) / m]
    };
    let width = |t: f64| point + (half - point) * (1.0 - t).powf(0.85);
    let n = 160;
    let mut pts = Vec::with_capacity(4 * n);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let (c, e) = (at(t), dir(t));
        pts.push([c[0] - e[1] * width(t), c[1] + e[0] * width(t)]);
    }
    let (c, e) = (at(1.0), dir(1.0));
    for k in 1..n {
        let a = PI * 0.5 - PI * k as f64 / n as f64;
        let (s, co) = a.sin_cos();
        pts.push([c[0] + (e[0] * co - e[1] * s) * point, c[1] + (e[1] * co + e[0] * s) * point]);
    }
    for k in (0..=n).rev() {
        let t = k as f64 / n as f64;
        let (c, e) = (at(t), dir(t));
        pts.push([c[0] + e[1] * width(t), c[1] - e[0] * width(t)]);
    }
    let e = dir(0.0);
    for k in 1..n {
        let a = -PI * 0.5 - PI * k as f64 / n as f64;
        let (s, co) = a.sin_cos();
        pts.push([root[0] + (e[0] * co - e[1] * s) * half, root[1] + (e[1] * co + e[0] * s) * half]);
    }
    ccw(resample(&pts, 0.03))
}

/// The tsavorite's centre on the face; its long axis runs round the ring.
const STONE: P2 = [0.2, 2.95];
/// Where the head's own frame stands on the face.
const HEAD_AT: P2 = [-0.5, -2.33];
/// The head in profile under its crown, in its own frame: facing round the ring toward -x, jaws open.
const HEAD: [P2; 27] = [
    [3.5, 0.55],
    [3.85, -0.35],
    [3.5, -1.35],
    [2.5, -1.95],
    [1.0, -2.12],
    [-0.4, -2.08],
    [-1.6, -1.98],
    [-2.6, -1.68],
    [-3.4, -1.2],
    [-3.95, -0.7],
    [-4.1, -0.3],
    [-3.85, 0.1],
    [-2.7, 0.32],
    [-1.5, 0.58],
    [-0.6, 0.8],
    [-0.05, 0.95],
    [-0.9, 1.22],
    [-2.0, 1.52],
    [-3.0, 1.78],
    [-3.55, 1.92],
    [-3.82, 2.15],
    [-3.6, 2.58],
    [-2.35, 2.68],
    [-0.9, 2.45],
    [0.6, 2.05],
    [1.9, 1.7],
    [3.0, 1.2],
];
/// Control points of `HEAD` at the upper lip's front, the gape's corner and the lower jaw's tip.
const LIPS: [usize; 3] = [11, 15, 19];
/// Samples a span of the head's spline.
const HEAD_PER: usize = 40;
/// The eye in the head's frame: centre, half-length, half-height, and how far the brow comes down over it.
const EYE: (P2, f64, f64, f64) = ([-1.5, -0.78], 0.7, 0.62, 0.25);
const NOSTRIL: P2 = [-3.55, -0.45];
/// The fangs in the head's frame: root inside the jaw, tip, and bow toward the snout.
const FANGS: [(P2, P2, f64); 2] = [([-3.5, -0.02], [-3.2, 0.95], 0.1), ([-1.42, 1.78], [-1.1, 0.8], -0.1)];
/// The tongue's stroke in the head's frame, from inside the gape out past the snout.
const TONGUE: [P2; 4] = [[-2.5, 1.32], [-3.6, 1.4], [-4.5, 1.55], [-5.05, 1.95]];
/// Plate boundaries across the head's side, in its frame: nasal, preocular, postoculars and temporals.
const PLATES: [&[P2]; 8] = [
    &[[-3.3, -0.95], [-3.1, -0.5], [-3.12, -0.1]],
    &[[-2.35, -1.2], [-2.52, -0.75], [-2.32, -0.28]],
    &[[-0.76, -1.02], [-0.2, -1.2], [0.35, -1.25]],
    &[[-0.78, -0.62], [-0.2, -0.6], [0.4, -0.42]],
    &[[-0.82, -0.3], [-0.3, -0.05], [0.3, 0.12]],
    &[[0.35, -1.25], [0.78, -0.55], [0.62, 0.22]],
    &[[1.7, -1.25], [2.18, -0.4], [1.95, 0.5]],
    &[[-0.05, 1.0], [0.7, 1.22], [1.5, 1.35]],
];
/// The jaws' rims over the gape's floor, the eye's socket, the brow over it and the plates' grooves, mm.
const LIP: f64 = 0.65;
const SOCKET: f64 = 0.35;
const BROW: f64 = 0.45;
const GROOVE: f64 = 0.1;
/// Labial plates along each jaw: count and pitch from the snout, mm.
const LABIALS: (usize, f64) = (5, 0.9);
/// The crown: how far its band comes down over the head's crest, the band's height in plan, its lift over the skull.
const CROWN: (f64, f64, f64) = (0.65, 0.8, 0.4);
/// The crown's points off the band's centre: offset, rise over the band, half-width at the base.
const POINTS: [(f64, f64, f64); 3] = [(-1.6, 1.2, 0.55), (0.0, 1.5, 0.62), (1.6, 1.2, 0.55)];
/// The points' centre off the band's, toward the head's back where the chief's cusp leaves them room, mm.
const CROWN_SHIFT: f64 = 0.2;
const PEARL: f64 = 0.41;
/// The serpent's spine from the throat, down past the stone, under it and up its far side to the tail.
const SPINE: [P2; 10] = [
    [2.4, -1.3],
    [4.8, -0.4],
    [5.6, 1.8],
    [5.15, 4.5],
    [2.95, 5.95],
    [0.3, 6.6],
    [-2.35, 6.2],
    [-4.3, 5.0],
    [-5.4, 3.3],
    [-5.55, 2.0],
];
/// Half-width of the body at the neck and at the tail's tip, mm.
const GIRTH: (f64, f64) = (1.3, 0.4);
/// The body's crown over its plinth, mm.
const DOME: f64 = 1.4;

/// A point of the head's frame on the face.
fn head_at(p: P2) -> P2 {
    [p[0] + HEAD_AT[0], p[1] + HEAD_AT[1]]
}

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

    /// The body's crest height and its steepest flank over the outer quarter of the girth, degrees, both unmodulated by skin.
    fn profile(&self) -> (f64, f64) {
        let (mut crest, mut steep): (f64, f64) = (0.0, 0.0);
        for i in (1..self.spine.len() - 1).step_by(4) {
            let (a, b) = (self.spine[i - 1], self.spine[i + 1]);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
            let n = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
            let w = self.girth(self.at[i]);
            let at = |d: f64| self.height([self.spine[i][0] + n[0] * d, self.spine[i][1] + n[1] * d], 0.0);
            crest = crest.max(at(0.0));
            for k in 0..20 {
                let d = w * (0.75 + 0.25 * k as f64 / 20.0);
                let h = 0.0005;
                steep = steep.max(((at(d - h) - at(d + h)) / (2.0 * h)).abs().atan().to_degrees());
            }
        }
        (crest, steep)
    }

    /// Distance outside the body's edge, mm; negative inside it.
    fn clear(&self, p: P2) -> f64 {
        self.nearest(p).map_or(f64::MAX, |(d, s, _)| d - self.girth(s))
    }

    /// Painted body height, mm: a raised-cosine crown of keeled dorsal scales over a low plinth that gives the edge its line,
    /// belly scutes down its inner side, the skin fading out over the outer quarter so the flanks stay smooth.
    fn height(&self, p: P2, calm: f64) -> f64 {
        let Some((d, s, lat)) = self.nearest(p) else { return 0.0 };
        let w = self.girth(s);
        if d >= w {
            return 0.0;
        }
        let plinth = 0.1 * (w / 0.8).min(1.0) * smoothstep(0.0, (0.6 * w).min(0.3), w - d);
        let x = d / w;
        let crown = DOME.min(1.12 * w);
        let dome = crown * 0.5 * (1.0 + (PI * x).cos());
        let across = lat / w;
        let belly = smooth(-0.22, -0.42, across);
        let scales = reptile::snake(s / 0.85, lat / 0.62);
        let scutes = reptile::ventral(s / 0.6, ((across + 0.71) / 0.29).clamp(-1.0, 1.0));
        let skin = (scales * (1.0 - belly) + (0.5 + 0.5 * scutes) * belly) * (1.0 - smooth(0.55, 0.75, d / w)) * calm;
        plinth + dome * (0.88 + 0.12 * skin)
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

/// Punched pits: radius, the pitch of the rows that outline each arm, the grain of the fill between them, mm.
const PIT: (f64, f64, f64) = (0.21, 0.5, 0.78);
/// The bordure's beads: inset from the table's edge, pitch and radius, mm.
const BEADS: (f64, f64, f64) = (0.62, 0.66, 0.22);
/// The seat's skirt beyond its rim, mm.
const SKIRT: f64 = 0.45;

/// The arms on the shield: a crowned serpent coiled round the tsavorite, jaws open on a forked tongue, on a pounced field.
struct Arms {
    table: Shape,
    body: Body,
    shapes: Vec<Shape>,
    head: Shape,
    full: Shape,
    mouth: Shape,
    boss: Shape,
    eye: Shape,
    /// World z of the brow's edge over the eye.
    clip: f64,
    /// Height of the eye socket's flat floor, and whether the skull is read with its socket and brow.
    floor: f64,
    socketed: bool,
    lips: [Line; 2],
    gape: (P2, P2),
    plates: Vec<Line>,
    /// The crown band: its ends round the ring, its top edge toward the chief and its lower edge on the skull.
    band: [f64; 4],
    beads: Vec<P2>,
    pits: Vec<P2>,
    grid: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl Arms {
    fn new(table: Vec<P2>) -> Result<Self> {
        let flat = StampTop::Flat;
        let dome = |h: f64| StampTop::Dome { crown_mm: h };
        let outline = spline(&HEAD.map(head_at), HEAD_PER);
        let [up, corner, low] = LIPS.map(|i| i * HEAD_PER);
        let area_of = |name: &str, pts: Vec<P2>| {
            let pts = ccw(resample(&pts, 0.05));
            Shape::new(name, centroid(&pts), pts, 0.0, 0.0, 0.0, 0, StampTop::Flat, false)
        };
        let head = area_of("Basilisk's head", outline.clone());
        let mut closed = outline[..=up].to_vec();
        closed.extend_from_slice(&outline[low..]);
        let full = area_of("Head, mouth closed", closed);
        let mouth = area_of("Gape", outline[up..=low].to_vec());
        let upper = Line::new(outline[up..=corner].to_vec());
        let lower = Line::new(outline[corner..=low].iter().rev().copied().collect());
        let gape = (outline[low], outline[up]);
        let (ra, rb) = (4.0 + STONE_STOCK * 0.5, 2.0 + STONE_STOCK * 0.5);
        let rim: Vec<P2> = (0..720)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / 720.0;
                let (c, s) = (t.cos(), t.sin());
                [STONE[0] + ra * c.signum() * c.abs().powf(2.0 / 1.5), STONE[1] + rb * s.signum() * s.abs().powf(2.0 / 1.5)]
            })
            .collect();
        let boss = area_of("Boss", rim);
        let (e, ex, ez, over) = EYE;
        let c = head_at(e);
        let clip = c[1] - ez + over;
        let mut lid: Vec<P2> = ellipse(c, ex, ez, 0.0).into_iter().map(|p| [p[0], p[1].max(clip)]).collect();
        lid.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9);
        let eye_mid = [c[0], 0.5 * (clip + c[1] + ez)];
        let eye = Shape::new("Eye", eye_mid, ccw(resample(&lid, 0.05)), 0.33, 0.3, 0.0, 0, dome(0.12), false);
        let socket = Shape::new("Eye socket", eye_mid, eye.poly.clone(), 0.0, 0.0, 0.0, 0, flat, false);
        let plates: Vec<Line> = PLATES.iter().map(|g| Line::new(open_spline(&g.iter().map(|p| head_at(*p)).collect::<Vec<_>>(), 12))).collect();

        let crest = head.poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        let z_low = crest + CROWN.0;
        let z_top = z_low - CROWN.1;
        let n = head.poly.len();
        let cuts: Vec<f64> = (0..n)
            .filter_map(|i| {
                let (a, b) = (head.poly[i], head.poly[(i + 1) % n]);
                ((a[1] - z_low) * (b[1] - z_low) <= 0.0 && a[1] != b[1] && a[1].min(b[1]) < HEAD_AT[1])
                    .then(|| a[0] + (z_low - a[1]) / (b[1] - a[1]) * (b[0] - a[0]))
            })
            .collect();
        let (x0, x1) = (cuts.iter().copied().fold(f64::MAX, f64::min), cuts.iter().copied().fold(f64::MIN, f64::max));
        ensure!(x1 - x0 > 4.0, "Crown band only {:.2} mm wide", x1 - x0);
        let xc = 0.5 * (x0 + x1) + CROWN_SHIFT;
        let mut s = Vec::new();
        let band = plate(vec![[x0, z_low], [x1, z_low], [x1, z_top], [x0, z_top]], 0.12);
        s.push(Shape::new("Crown", [0.5 * (x0 + x1), 0.5 * (z_top + z_low)], band, CROWN.2, 0.3, 2.0, 0, StampTop::Gable { rise_mm: 0.08, axis_deg: 0.0 }, false));
        for (k, &(dx, rise, half)) in POINTS.iter().enumerate() {
            let x = xc + dx;
            let (base, tip) = (z_top + 0.1, z_top - rise);
            let at = [x, base + 0.4 * (tip - base)];
            let poly = ccw(resample(&outline::rounded_polygon(&[[x - half, base], [x, tip], [x + half, base]], 0.1), 0.03));
            let end = tip + 0.1 / (half / (base - tip)).atan().sin();
            let land = land_of(&poly, &[([x, base - 0.4], [x, end])]);
            s.push(
                Shape::new(format!("Crown point, {}", k + 1), at, poly, 1.0, 0.3, 2.0, 0, StampTop::Cone { apex_mm: 0.4, at, tip_mm: 0.0 }, false)
                    .land(land, Some("Pointed fleuron under its pearl: investment detail, cast in place and cleaned up with a graver")),
            );
            s.push(Shape::new(format!("Crown pearl, {}", k + 1), [x, tip], ellipse([x, tip], PEARL, PEARL, 0.0), 1.05, 0.3, 2.0, 0, dome(0.35), false));
        }
        for (k, dx) in [-0.875, 0.875].into_iter().enumerate() {
            let (x, z) = (xc + dx, z_low - 0.3);
            let poly = plate(vec![[x - 0.4, z], [x, z - 0.25], [x + 0.4, z], [x, z + 0.25]], 0.06);
            s.push(
                Shape::new(format!("Crown jewel, {}", k + 1), [x + 1.2, z], poly, 0.06, 0.3, 0.0, 1, StampTop::Cone { apex_mm: 0.2, at: [x, z], tip_mm: 0.0 }, false)
                    .land(0.0, Some("Faceted lozenge set on the band: investment detail, polished at the bench")),
            );
        }
        s.push(eye);
        let pupil_mid = eye_mid[1];
        s.push(Shape::new("Pupil", [c[0] + 1.2, pupil_mid], lens([c[0], pupil_mid - 0.38], [c[0], pupil_mid + 0.38], 0.2, 0.04), 0.4, 0.1, 0.0, 1, StampTop::Gable { rise_mm: 0.45, axis_deg: 90.0 }, true));
        let nose = head_at(NOSTRIL);
        s.push(Shape::new("Nostril", nose, ellipse(nose, 0.2, 0.12, -20.0), 0.3, 0.22, 0.0, 0, flat, true));
        for (k, (root, tip, bend)) in FANGS.into_iter().enumerate() {
            let (root, tip) = (head_at(root), head_at(tip));
            let axis = (tip[1] - root[1]).atan2(-(tip[0] - root[0])).to_degrees();
            let poly = fang(root, tip, 0.2, bend, 0.09);
            let spine: Vec<P2> = (0..=40)
                .map(|k| {
                    let t = k as f64 / 40.0;
                    let (d, l) = ([tip[0] - root[0], tip[1] - root[1]], (tip[0] - root[0]).hypot(tip[1] - root[1]));
                    let b = bend * 4.0 * t * (1.0 - t);
                    [root[0] + d[0] * t - d[1] / l * b, root[1] + d[1] * t + d[0] / l * b]
                })
                .collect();
            let land = land_of(&poly, &spine.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>());
            s.push(
                Shape::new(format!("Fang, {}", k + 1), [0.5 * (root[0] + tip[0]), 0.5 * (root[1] + tip[1])], poly, 0.15, 0.3, 0.0, 0, StampTop::Taper { axis_deg: axis, tip_mm: 0.35 }, false)
                    .land(land, Some("Fang curving to a rounded point: investment detail, cast in place")),
            );
        }
        let line: Vec<P2> = TONGUE.iter().map(|p| head_at(*p)).collect();
        for (k, (poly, from, to, bones)) in tongue(&line, 0.9, 0.4, 0.3, 38.0).into_iter().enumerate() {
            let name = ["Forked tongue", "Forked tongue, upper tine", "Forked tongue, lower tine"][k];
            let mid = [0.5 * (from[0] + to[0]), 0.5 * (from[1] + to[1])];
            let centre = if k == 0 { line[1] } else { mid };
            let (rise, end) = if k == 0 { (0.2, 0.18) } else { (0.18, 0.05) };
            let land = land_of(&poly, &bones);
            let note = if k == 0 { "Tongue's stem, 0.4 mm and ridged: investment detail cast in place" } else { "Tongue's tine, 0.3 mm and ridged to a rounded end: investment detail cast in place" };
            s.push(Shape::new(name, centre, poly, 0.12, 0.3, 0.0, 0, StampTop::Ridge { rise_mm: rise, from, to, end_mm: end }, false).land(land, Some(note)));
        }
        for sh in &mut s {
            if sh.land == 0.0 && !sh.cut {
                sh.land = land_of(&sh.poly, &[]);
            }
            let local: Vec<P2> = sh.poly.iter().map(|p| [p[0] - sh.centre[0], p[1] - sh.centre[1]]).collect();
            outline::check(&ccw(local)).map_err(|e| anyhow!("{}: {e}", sh.name))?;
        }
        let table = Shape::new("Table", [0.0, 0.0], table, 0.0, 0.0, 0.0, 0, flat, false);
        let mut arms = Self {
            table,
            body: Body::new(),
            shapes: s,
            head,
            full,
            mouth,
            boss,
            eye: socket,
            clip,
            floor: 0.0,
            socketed: false,
            lips: [upper, lower],
            gape,
            plates,
            band: [x0, x1, z_top, z_low],
            beads: Vec::new(),
            pits: Vec::new(),
            grid: Default::default(),
        };
        arms.floor = arms.skull(eye_mid) - SOCKET;
        arms.socketed = true;
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

    /// Distance outside every arm on the field, mm: the head with its gape, the body, the raised stamps, and the seat's skirt.
    fn clear(&self, p: P2) -> f64 {
        let mut d = (-self.full.sdf(p, 2.0)).min(self.body.clear(p)).min(-self.boss.sdf(p, 2.0) - SKIRT);
        for s in self.shapes.iter().filter(|s| s.tier == 0 && !s.cut) {
            d = d.min(-s.sdf(p, 2.0));
        }
        d
    }

    /// The head's painted relief before the crown's bed is laid under the band, mm.
    fn skull(&self, p: P2) -> f64 {
        let df = self.full.sdf(p, 1.5);
        if df <= 0.0 {
            return 0.0;
        }
        let dm = -self.mouth.sdf(p, 1.5);
        if dm <= 0.0 {
            return 0.0;
        }
        let q = [p[0] - HEAD_AT[0], p[1] - HEAD_AT[1]];
        let general = 0.35 * smooth(0.0, 0.3, df) + 0.95 * (1.0 - (1.0 - (df / 1.3).min(1.0)).powi(2));
        let snout = 1.0 - 0.16 * smooth(-2.4, -3.9, q[0]);
        let cheek = {
            let (u, v) = ((q[0] - 1.5) / 1.5, (q[1] + 0.3) / 0.9);
            0.12 * (1.0 - (u * u + v * v)).max(0.0).powi(2) * smooth(0.2, 0.6, df)
        };
        let lip = LIP * smooth(0.0, 0.22, dm) + 0.6 * smooth(0.64, 1.3, dm);
        let mut h = (general * snout + cheek).min(lip) + self.labials(p, dm);
        let plates = self.plates.iter().filter_map(|g| g.nearest(p, 0.2)).map(|n| n.0).fold(f64::MAX, f64::min);
        let z_low = self.band[3];
        h -= GROOVE * (1.0 - smooth(0.03, 0.09, plates)) * smooth(0.3, 0.5, df) * smooth(0.7, 0.8, dm) * smooth(z_low + 0.12, z_low + 0.3, p[1]);
        if !self.socketed {
            return h;
        }
        let e = self.eye.sdf(p, 1.0);
        let socket = h + (self.floor - h) * smooth(-0.22, -0.08, e);
        let u = self.clip - 0.08 - p[1];
        if !(-0.1..0.7).contains(&u) {
            return socket.max(0.0);
        }
        let x = p[0] - self.eye.centre[0];
        let ex = EYE.1;
        let span = smooth(-ex - 0.25, -ex + 0.1, x) * (1.0 - smooth(ex - 0.15, ex + 0.2, x));
        let brow = self.floor + (h + BROW * span * (1.0 - smooth(0.2, 0.45, u)) - self.floor) * smooth(0.0, 0.11, u);
        socket.max(brow).max(0.0)
    }

    /// Labial plates along each jaw's rim, mm: a domed plate every pitch from the snout, grooved against the head's plates above.
    fn labials(&self, p: P2, dm: f64) -> f64 {
        if dm > 0.9 {
            return 0.0;
        }
        let near: Vec<(f64, f64, f64)> = self.lips.iter().filter_map(|l| l.nearest(p, 1.0).map(|(d, s)| (d, s, l.length()))).collect();
        let Some(&(_, s, _)) = near.iter().min_by(|a, b| a.0.total_cmp(&b.0)) else { return 0.0 };
        let (count, pitch) = LABIALS;
        let k = (s / pitch).floor();
        if k < 0.0 || k as usize >= count {
            return 0.0;
        }
        let t = s / pitch - k;
        let along = (1.0 - (2.0 * t - 1.0).powi(2)).max(0.0).powf(0.35);
        let across = (1.0 - ((dm - 0.4) / 0.26).powi(2)).max(0.0).powf(0.5);
        let fade = 1.0 - smooth(count as f64 - 0.4, count as f64, s / pitch);
        0.11 * along * across - 0.07 * (1.0 - smooth(0.02, 0.07, (dm - 0.68).abs())) * fade
    }

    /// The head's painted relief with the crown's bed under the band: level with the skull at the band's lower edge,
    /// eased down to the field over the band's last quarter millimetre toward the chief.
    fn head_relief(&self, p: P2) -> f64 {
        let [x0, x1, z_top, z_low] = self.band;
        if p[0] >= x0 && p[0] <= x1 && p[1] < z_low {
            if p[1] < z_top {
                return 0.0;
            }
            return self.skull([p[0], z_low]) * smooth(z_top, z_top + 0.25, p[1]);
        }
        self.skull(p)
    }

    /// The serpent painted over the table, mm: the head with its crown's bed, and the body coiled round the seat.
    fn serpent(&self, p: P2) -> f64 {
        if self.table.sdf(p, 1.0) <= 0.0 {
            return 0.0;
        }
        let df = self.full.sdf(p, 1.5);
        let calm = smooth(-0.2, 0.5, -self.boss.sdf(p, 2.0) - SKIRT);
        let body = self.body.height(p, calm) * (1.0 - smooth(0.0, 0.5, df));
        self.head_relief(p).max(body)
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
                if self.clear(w[1]) > radius + 0.3 {
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
        let (r, pitch, grain) = PIT;
        let gap = 0.12;
        let mut rows: Vec<Vec<P2>> = Vec::new();
        let offset = |poly: &[P2], by: f64| -> Vec<P2> {
            let n = poly.len();
            (0..n)
                .map(|i| {
                    let (a, b) = (poly[(i + n - 1) % n], poly[(i + 1) % n]);
                    let t = [b[0] - a[0], b[1] - a[1]];
                    let l = t[0].hypot(t[1]).max(1e-12);
                    [poly[i][0] + t[1] / l * by, poly[i][1] - t[0] / l * by]
                })
                .collect()
        };
        rows.push(offset(&self.full.poly, r + gap));
        rows.push(offset(&self.boss.poly, SKIRT + r + gap));
        for sh in self.shapes.iter().filter(|s| s.tier == 0 && !s.cut) {
            rows.push(offset(&resample(&sh.poly, 0.05), r + gap));
        }
        let spine = &self.body.spine;
        for side in [1.0, -1.0] {
            let mut row = Vec::new();
            for i in 0..spine.len() {
                let (a, b) = (spine[i.saturating_sub(1)], spine[(i + 1).min(spine.len() - 1)]);
                let t = [b[0] - a[0], b[1] - a[1]];
                let l = t[0].hypot(t[1]).max(1e-12);
                let by = self.body.girth(self.body.at[i]) + r + gap;
                row.push([spine[i][0] - t[1] / l * by * side, spine[i][1] + t[0] / l * by * side]);
            }
            rows.push(row);
        }
        let tail = *spine.last().unwrap();
        let by = GIRTH.1 + r + gap;
        rows.push((0..48).map(|k| {
            let a = 2.0 * PI * k as f64 / 48.0;
            [tail[0] + by * a.cos(), tail[1] + by * a.sin()]
        }).collect());
        rows.push(offset(&self.table.poly, -(BEADS.0 + BEADS.2 + 0.13 + r)));
        let mut pits: Vec<P2> = Vec::new();
        let spaced = |pits: &[P2], q: P2, d: f64| pits.iter().all(|p| (p[0] - q[0]).hypot(p[1] - q[1]) >= d);
        for row in &rows {
            for &q in row {
                if self.pit_fits(q, gap - 0.03) && spaced(&pits, q, pitch) {
                    pits.push(q);
                }
            }
        }
        let [lo, hi] = [self.table.lo, self.table.hi];
        let (i0, i1) = ((lo[0] / grain).floor() as i64, (hi[0] / grain).ceil() as i64);
        let (j0, j1) = ((lo[1] / grain).floor() as i64, (hi[1] / grain).ceil() as i64);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let stagger = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
                let q = [
                    (i as f64 + stagger + 0.2 * (skin::hash(i, j) - 0.5)) * grain,
                    (j as f64 * 0.866 + 0.2 * (skin::hash(j + 7919, i) - 0.5)) * grain,
                ];
                if self.pit_fits(q, gap + 0.12) && spaced(&pits, q, pitch) {
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
        let open = |q: P2| self.pit_fits(q, 0.09) && self.mouth.sdf(q, 0.5) <= 0.0;
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
        for sh in [&self.full, &self.boss].into_iter().chain(self.shapes.iter().filter(|s| s.tier == 0 && !s.cut)) {
            let poly = resample(&sh.poly, 0.1);
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[(i + n - 1) % n], poly[(i + 1) % n]);
                let t = [b[0] - a[0], b[1] - a[1]];
                let l = t[0].hypot(t[1]).max(1e-12);
                let by = if std::ptr::eq(sh, &self.boss) { SKIRT + 0.12 } else { 0.12 };
                let q = [poly[i][0] + t[1] / l * by, poly[i][1] - t[0] / l * by];
                let into = [q[0] + t[1] / l * PIT.0, q[1] - t[0] / l * PIT.0];
                if open(into) {
                    let d = self.to_pit(q) + 0.12;
                    all += 1;
                    near += usize::from(d <= 0.4);
                    edge = edge.max(d);
                }
            }
        }
        (bare, edge, near as f64 / all.max(1) as f64)
    }

    /// The pounced ground sunk into the table and the gape's floor, 0..1 of the pits' depth.
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
        if self.mouth.sdf(p, 0.5) > 0.0 {
            let lips = self.lips.iter().filter_map(|l| l.nearest(p, 0.5)).map(|n| n.0).fold(f64::MAX, f64::min);
            let open = seg_dist(p, self.gape.0, self.gape.1).0;
            pit = pit.max(smooth(0.0, 0.1, lips) * smooth(0.0, 0.35, open));
        }
        pit
    }
}

/// Narrowest land of a closed outline in plan, mm: twice its least inscribed radius along `bones` (sampled every 0.01 mm),
/// or with no bones twice its largest inscribed radius, which is the narrowest caliper of a convex outline.
fn land_of(poly: &[P2], bones: &[(P2, P2)]) -> f64 {
    if bones.is_empty() {
        let lo = [poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min), poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min)];
        let hi = [poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max), poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max)];
        let step = (hi[0] - lo[0]).max(hi[1] - lo[1]) / 60.0;
        let mut seeds: Vec<(f64, P2)> = Vec::new();
        for i in 0..=60 {
            for j in 0..=60 {
                let q = [lo[0] + step * i as f64, lo[1] + step * j as f64];
                if inside(poly, q) {
                    seeds.push((poly_dist(poly, q), q));
                }
            }
        }
        seeds.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut best = (0.0, [0.0, 0.0]);
        for &(d0, q0) in seeds.iter().take(8) {
            let mut here = (d0, q0);
            let mut s = step;
            while s > 1e-4 {
                let mut moved = false;
                for k in 0..8 {
                    let a = PI * 0.25 * k as f64;
                    let q = [here.1[0] + s * a.cos(), here.1[1] + s * a.sin()];
                    if inside(poly, q) {
                        let d = poly_dist(poly, q);
                        if d > here.0 {
                            here = (d, q);
                            moved = true;
                        }
                    }
                }
                if !moved {
                    s *= 0.6;
                }
            }
            if here.0 > best.0 {
                best = here;
            }
        }
        return 2.0 * best.0;
    }
    let mut least = f64::MAX;
    for &(a, b) in bones {
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        let n = (l / 0.01).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let q = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if inside(poly, q) {
                least = least.min(2.0 * poly_dist(poly, q));
            }
        }
    }
    least
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

/// Every face stamp placed on the ring, its outline and its top read in the frame the build stands it in.
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
            top: StampTop::Flat,
        });
    }
    let ctx = d.field_context();
    let frames: Vec<_> = (first..d.stamps.len()).map(|k| d.stamps[k].frame(d, &ctx)).collect();
    let mut placed = Vec::new();
    for (k, (sh, f)) in face.shapes.iter().zip(&frames).enumerate() {
        ensure!((f.x[0] + 1.0).abs() < 1e-6 && (f.y[2] - 1.0).abs() < 1e-6, "{} stands off the table's frame", sh.name);
        let local = |p: P2| -> P2 {
            let w = [p[0] - f.origin[0], a.top - f.origin[1], p[1] - f.origin[2]];
            [w[0] * f.x[0] + w[1] * f.x[1] + w[2] * f.x[2], w[0] * f.y[0] + w[1] * f.y[1] + w[2] * f.y[2]]
        };
        let outline: Vec<P2> = sh.poly.iter().map(|p| local(*p)).collect();
        let outline = if area(&outline) < 0.0 { outline.into_iter().rev().collect() } else { outline };
        outline::check(&outline).map_err(|e| anyhow!("{}: {e}", sh.name))?;
        d.stamps[first + k].outline = outline;
        d.stamps[first + k].top = match sh.top {
            StampTop::Ridge { rise_mm, from, to, end_mm } => StampTop::Ridge { rise_mm, from: local(from), to: local(to), end_mm },
            StampTop::Cone { apex_mm, at, tip_mm } => StampTop::Cone { apex_mm, at: local(at), tip_mm },
            top => top,
        };
        let off = (f.origin[0] - sh.centre[0]).hypot(f.origin[2] - sh.centre[1]);
        placed.push(json!({"name": sh.name, "tier": sh.tier, "cut": sh.cut, "points": sh.poly.len(), "origin_error_mm": off, "land_mm": sh.land}));
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

/// How far a hackle's shaft bows away from the pale by its tip, as a share of its length: a 20 degree sickle.
const SICKLE: f64 = 0.182;

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

    /// Row pitch across, feather length and the step between feathers along a row, mm.
    fn shape(&self, root: f64, morph: f64) -> (f64, f64, f64) {
        let pitch = (1.5 + 0.8 * smooth(0.0, 8.0, root)) * (1.0 - morph) + 1.2 * morph;
        let length = pitch * (3.2 * (1.0 - morph) + 1.2 * morph);
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

    /// Height and barbs, 0..1, at `root` mm past the table's edge (which lies `off` along the ring from the head's centre)
    /// and `across` the section: each shaft bows away from the pale like a sickle, each free end stands over the row it overlaps.
    fn at(&self, root: f64, off: f64, across: f64) -> (f64, f64) {
        let morph = smooth(self.l70, self.l110, root + off);
        let (pitch, length, _) = self.shape(root, morph);
        let phase = self.phase_at(root, off);
        let fan = 1.0 + 0.065 * root * (1.0 - morph);
        let cross = across / fan;
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
                let bow = SICKLE * length * t * t * (1.0 - morph) * (j as f64).signum();
                let y = cross - j as f64 * pitch - bow;
                let half = 0.125 + (0.53 * pitch - 0.125) * (PI * t).sin().max(0.0).powf(0.68);
                let edge = 1.0 - smooth((half - 0.23).max(0.0), half, y.abs());
                let tip = 1.0 - smooth(0.90, 1.0, t);
                let root_in = smooth(0.0, 0.30 - 0.17 * morph, t);
                let lift = 0.13 * smooth(0.55, 0.9, t) * (1.0 - morph);
                let vane = 0.44 + 0.33 * smooth(0.0, 0.86, t) - 0.12 * (y / half.max(0.1)).powi(2) + lift;
                let rachis = 0.11 * (1.0 - smooth(0.04, 0.19, y.abs())) * smooth(0.05, 0.25, t);
                let h = (vane + rachis) * edge * tip * root_in;
                if h > height {
                    height = h;
                    let line = ((t * length - 0.58 * y.abs()) / 0.80).rem_euclid(1.0);
                    barbs = (1.0 - smooth(0.04, 0.17, (line - 0.5).abs())) * smooth(0.10, 0.25, y.abs()) * edge * tip * root_in;
                }
            }
        }
        (height, barbs)
    }
}

/// A ventral scute, 0..1, `u` counting scutes from the palm toward the head and `across` over the band's half-width:
/// domed across, its free edge toward the palm standing over the next, its ends rounded off.
fn scute(u: f64, across: f64) -> f64 {
    let t = u - u.floor();
    let shingle = 0.73 + 0.27 * (1.0 - smooth(0.05, 0.95, t));
    let edge = 0.6 + 0.4 * smooth(0.0, 0.06, t);
    let dome = 1.0 - 0.27 * across.clamp(-1.0, 1.0).powi(2);
    shingle * edge * dome
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

fn author(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Value, Vec<Value>)> {
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
    let mut facing: Vec<f32> = a.samples.iter().map(|s| if s.n[2].abs() < 0.45 { 1.0 } else { 0.0 }).collect();
    blur(&mut facing, a.width, a.height, 10, 3);
    let plume = |s: &Sample| {
        let h = hide.at(s);
        smooth(-0.25, 0.55, h.rim - h.across.abs()) * facing[s.i] as f64
    };
    let hackles = Hackles::new(l70, l110);
    let feather = |s: &Sample| {
        let h = hide.at(s);
        let (_, _, along) = skin_masks(&a, &hide, s);
        let near = 1.0 - smooth(9.0, 14.0, along);
        let off = width_at(s.p[2]) * near + 7.0 * (1.0 - near) + 0.2;
        hackles.at((along - off).max(0.0), along.min(off), h.across)
    };
    let alpha = a.paint("Basiliscus", |s| if on_table(s) { arms.serpent([s.p[0], s.p[2]]) / SERPENT_HEIGHT } else { 0.0 });
    let painted_peak = alpha.data.iter().copied().fold(0.0f32, f32::max) as f64 * SERPENT_HEIGHT;
    portable(&mut d, &mut lib, alpha, SERPENT_HEIGHT, window(90.0, 70.0), false, None)?;
    let mut face = Window::around(90.0, 64.0);
    face.fade_deg = 1.0;
    let alpha = a.paint("Beaded bordure", |s| if on_table(s) { arms.bordure_height([s.p[0], s.p[2]]) } else { 0.0 });
    portable(&mut d, &mut lib, alpha, BEAD_HEIGHT, face, false, None)?;
    let alpha = a.paint("Hackles into scales", |s| {
        if !within(s.theta, 90.0, 126.0) {
            return 0.0;
        }
        let (bore, _, _) = skin_masks(&a, &hide, s);
        feather(s).0 * plume(s) * bore * off_table.data[s.i] as f64
    });
    portable(&mut d, &mut lib, alpha, HACKLE_HEIGHT, window(90.0, 240.0), false, Some(REACH))?;
    let alpha = a.paint("Serpent flanks", |s| {
        let h = hide.at(s);
        let (bore, _, along) = skin_masks(&a, &hide, s);
        let m = smooth(l70, l110 + 5.0, along);
        let rounds = round_scales(h.along / 2.1, h.across / 2.8);
        let shields = reptile::shields(h.along / 2.5, h.across / 2.8);
        (rounds * (1.0 - m) + shields * m) * (1.0 - plume(s)) * bore * off_table.data[s.i] as f64
    });
    portable(&mut d, &mut lib, alpha, FLANK_HEIGHT, window(180.0, 360.0), false, Some(REACH))?;
    let far = hide.reach();
    let alpha = a.paint("Belly scutes", |s| {
        if !within(s.theta, 270.0, 61.0) {
            return 0.0;
        }
        let h = hide.at(s);
        let (bore, _, _) = skin_masks(&a, &hide, s);
        scute((far - h.along.abs()) / 2.2, h.across / h.rim.max(0.5)) * smooth(0.0, 0.8, h.rim - h.across.abs()) * bore
    });
    portable(&mut d, &mut lib, alpha, BELLY_HEIGHT, window(270.0, 110.0), false, Some(REACH))?;
    let alpha = a.paint("Graver's barbs and keels", |s| {
        if !within(s.theta, 90.0, 126.0) {
            return 0.0;
        }
        let (bore, _, along) = skin_masks(&a, &hide, s);
        feather(s).1 * plume(s) * bore * off_table.data[s.i] as f64 * (1.0 - smooth(l70 - 2.0, l70, along))
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
        blend_mm: SKIRT,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        rot_deg: 0.0,
        ..Default::default()
    };
    seat.fit_stone(gem);
    seat.diameter_mm = gem.w_mm + STONE_STOCK;
    seat.elong = (gem.l_mm + STONE_STOCK) / (gem.w_mm + STONE_STOCK);
    seat.height_mm = STONE_BOSS;
    let lands = land_rows(&arms, &seat, gem);
    let mut e = LayerEntry::new("Tsavorite, flush", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    let alpha = a.paint("Pounced field", |s| if on_table(s) { arms.pounce_depth([s.p[0], s.p[2]]) } else { 0.0 });
    portable(&mut d, &mut lib, alpha, PIT_DEPTH, face, false, None)?;
    d.layers.layers.last_mut().unwrap().blend = Blend::Subtract;
    let placed = strike(&mut d, &a, &arms)?;
    let [x0, x1, z_top, z_low] = arms.band;
    let (bare, edge, share) = arms.pounce_reach();
    let (crest, steep) = arms.body.profile();
    let pearls: Vec<f64> = arms.shapes.iter().filter(|s| s.name.starts_with("Crown pearl")).map(|s| arms.table.sdf(s.centre, 3.0) - PEARL).collect();
    let composition = json!({
        "stone_mm": STONE, "stone_stock_mm": STONE_STOCK, "stone_boss_mm": STONE_BOSS, "spine_mm": arms.body.length(), "girth_mm": [GIRTH.0, GIRTH.1], "body_crown_mm": DOME, "body_crest_mm": crest, "body_outer_quarter_steepest_deg": steep,
        "crown_pearls_inside_chief_mm": pearls, "crown_points_mm": POINTS, "crown_band_overlap_on_crest_mm": CROWN.0,
        "serpent_layer_scale_mm": SERPENT_HEIGHT, "serpent_painted_peak_mm": painted_peak,
        "pit_depth_mm": PIT_DEPTH, "pits": arms.pits.len(), "pit_reach_mm": {"widest_bare_field": bare, "furthest_arm_edge_from_a_pit": edge, "share_of_arm_edge_within_0_4": share}, "bead_height_mm": BEAD_HEIGHT, "beads": arms.beads.len(),
        "crown_band_mm": {"x": [x0, x1], "z": [z_top, z_low], "width": x1 - x0, "height": z_low - z_top},
        "table_points": arms.table.poly.len(), "table_z_mm": [arms.table.lo[1], arms.table.hi[1]], "table_x_mm": [arms.table.lo[0], arms.table.hi[0]],
        "morph_start_along_mm": l70, "morph_end_along_mm": l110, "stock": "020 native unmirrored", "atlas": [AW, ah],
        "stamps": placed,
    });
    Ok((d, lib, composition, lands))
}

/// The lost-wax land census: every proud feature's narrowest land in plan, and what names it when it is under the floor.
fn land_rows(arms: &Arms, seat: &SeatPadLayer, gem: Gem) -> Vec<Value> {
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
    for sh in arms.shapes.iter().filter(|s| !s.cut) {
        rows.push(row(&sh.name, "stamp", sh.land, sh.note));
    }
    let body = &arms.body;
    let n = (body.length() / 0.02).ceil() as usize;
    let thinnest = (0..=n).map(|k| 2.0 * body.girth(body.length() * k as f64 / n as f64)).fold(f64::MAX, f64::min);
    rows.push(row("Serpent's body to the tail's tip (painted)", "painted", thinnest, None));
    let chord = |line: &Line, sign: f64| {
        let n = (line.length() / 0.02).ceil() as usize;
        let poly = &arms.head.poly;
        (0..=n)
            .filter_map(|k| {
                let s = line.length() * k as f64 / n as f64;
                (s > 0.3 && s < line.length() - 0.3).then(|| {
                    let i = line.at.iter().position(|&a| a >= s).unwrap_or(1).clamp(1, line.pts.len() - 1);
                    let (a, b) = (line.pts[i - 1], line.pts[i]);
                    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
                    let dir = [-(b[1] - a[1]) / l * sign, (b[0] - a[0]) / l * sign];
                    let from = [a[0] + dir[0] * 0.005, a[1] + dir[1] * 0.005];
                    let far = (0..poly.len())
                        .filter_map(|j| {
                            let (p, q) = (poly[j], poly[(j + 1) % poly.len()]);
                            let e = [q[0] - p[0], q[1] - p[1]];
                            let den = dir[0] * e[1] - dir[1] * e[0];
                            if den.abs() < 1e-12 {
                                return None;
                            }
                            let w = [p[0] - from[0], p[1] - from[1]];
                            let t = (w[0] * e[1] - w[1] * e[0]) / den;
                            let u = (w[0] * dir[1] - w[1] * dir[0]) / den;
                            (t > 0.0 && (0.0..=1.0).contains(&u)).then_some(t)
                        })
                        .fold(f64::MAX, f64::min);
                    far + 0.005
                })
            })
            .fold(f64::MAX, f64::min)
    };
    let (up, low) = (chord(&arms.lips[0], -1.0), chord(&arms.lips[1], 1.0));
    rows.push(row("Upper jaw, lip to crown (painted)", "painted", up, None));
    rows.push(row("Lower jaw, lip to chin (painted)", "painted", low, None));
    rows.push(row("Beaded bordure beads (painted)", "painted", 2.0 * BEADS.2, Some("Bordure bead: investment detail, cast in place and burnished")));
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
    let (d, lib, composition, lands) = author(params)?;
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
