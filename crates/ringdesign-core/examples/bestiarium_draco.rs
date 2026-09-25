//! Draco, the wyvern displayed: factory 002 Kite as a Delft sand master, one wyvern from face to palm.
//! cargo run -p ringdesign-core --release --example bestiarium_draco -- [OUT_DIR] [--draft] [--verify] [--graph EVALUATED.ring.json]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, Mesh, ProfileStyle, RingDesign,
    castability::{CastProcess, SandProcess, Verdict},
    csg,
    field::{Blend, Window, smootherstep},
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    manufacturing as mf,
    render::{self, Part},
    setting::Stamp,
    skin::{self, Atlas, ClampReport, Hide, HidePoint, Sample, draft_clamp},
    stl,
};
use std::{
    f64::consts::{PI, TAU},
    path::{Path, PathBuf},
};

const AW: usize = 2048;
const AH: usize = 768;
/// A finger's joint, swelling its crest halfway out, mm.
const KNUCKLE: f64 = 0.07;
/// Steepest a ramp climbs per mm walked out from the parting line, mm.
const RATE: f64 = 0.17;
/// Width of every step's fall, mm.
const BEVEL: f64 = 0.55;

/// A point on the plan of the face: `u` along the ridge (+ toward the neck), `z` across the finger.
type P2 = [f64; 2];

fn lerp2(a: P2, b: P2, t: f64) -> P2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    smootherstep(e0, e1, x)
}

/// A quadratic Bezier from `a` to `b` bowed `bow` mm to the left, as a polyline.
fn bone(a: P2, b: P2, bow: f64, n: usize) -> Vec<P2> {
    let m = lerp2(a, b, 0.5);
    let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
    let l = ex.hypot(ez).max(1e-9);
    let c = [m[0] - ez / l * bow, m[1] + ex / l * bow];
    (0..=n).map(|i| { let t = i as f64 / n as f64; lerp2(lerp2(a, c, t), lerp2(c, b, t), t) }).collect()
}

/// Distance from `p` to a polyline, the share along it, and the nearest point on it.
fn poly(p: P2, pts: &[P2]) -> (f64, f64, P2) {
    let n = pts.len() - 1;
    let mut best = (f64::MAX, 0.0, pts[0]);
    for i in 0..n {
        let (a, b) = (pts[i], pts[i + 1]);
        let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ez) / (ex * ex + ez * ez).max(1e-12)).clamp(0.0, 1.0);
        let q = [a[0] + ex * t, a[1] + ez * t];
        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
        if d < best.0 {
            best = (d, (i as f64 + t) / n as f64, q);
        }
    }
    best
}

/// Where a polyline stands at `u`, mm of z.
fn z_on(pts: &[P2], u: f64) -> Option<f64> {
    pts.windows(2).find(|w| (w[0][0] - u) * (w[1][0] - u) <= 0.0).map(|w| {
        let t = (u - w[0][0]) / (w[1][0] - w[0][0]);
        w[0][1] + (w[1][1] - w[0][1]) * if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 }
    })
}

/// The table's plan: how far across the finger its roof runs at each point along the ridge.
struct Table(Vec<(f64, f64)>);

impl Table {
    fn of(a: &Atlas, hide: &Hide) -> Self {
        let mut rows: Vec<(f64, f64)> = roof_edges(a, hide).iter().enumerate().filter(|(_, e)| e[1] > 0.0).map(|(x, e)| (-hide.along[x], e[1].min(-e[0]))).collect();
        rows.sort_by(|p, q| p.0.total_cmp(&q.0));
        Self(rows)
    }

    /// Half the roof's width at `u`, mm; zero off the table.
    fn hi(&self, u: f64) -> f64 {
        let r = &self.0;
        if r.len() < 2 || u < r[0].0 || u > r[r.len() - 1].0 {
            return 0.0;
        }
        let j = r.partition_point(|p| p.0 < u).clamp(1, r.len() - 1);
        let (a, b) = (r[j - 1], r[j]);
        a.1 + (b.1 - a.1) * ((u - a.0) / (b.0 - a.0).max(1e-9)).clamp(0.0, 1.0)
    }

    /// Where a ray from `from` along `deg` first comes within `inset` of the rim or reaches `floor(u)`.
    fn march(&self, from: P2, deg: f64, inset: f64, floor: impl Fn(f64) -> f64) -> P2 {
        let (s, c) = deg.to_radians().sin_cos();
        let mut p = from;
        for k in 1..4000 {
            let q = [from[0] + c * k as f64 * 0.005, from[1] + s * k as f64 * 0.005];
            if q[1] > self.hi(q[0]) - inset || q[1] < floor(q[0]) {
                break;
            }
            p = q;
        }
        p
    }
}

/// One bone of the wing: a polyline from its root, its crest over the membrane on its near side, and its half-width, root to tip.
struct Bone {
    pts: Vec<P2>,
    lip: (f64, f64),
    half: (f64, f64),
    /// The membrane a column walking out from the spine meets before the bone; the bone is its lip.
    near: usize,
}

impl Bone {
    /// The bone's bearing from its root at `rho` mm out, radians, held past either end.
    fn bearing(&self, rho: f64) -> f64 {
        let o = self.pts[0];
        let at = |q: P2| (q[1] - o[1]).atan2(q[0] - o[0]);
        let r = |q: P2| (q[0] - o[0]).hypot(q[1] - o[1]);
        let last = self.pts.len() - 1;
        for (i, w) in self.pts[1..].windows(2).enumerate() {
            let (a, b) = (r(w[0]), r(w[1]));
            if rho <= b || i + 2 == last {
                let t = ((rho - a) / (b - a).max(1e-9)).clamp(0.0, 1.0);
                let (pa, pb) = (at(w[0]), at(w[1]));
                return pa + ((pb - pa + PI).rem_euclid(TAU) - PI) * t;
            }
        }
        at(self.pts[last])
    }

    fn tip(&self) -> P2 {
        self.pts[self.pts.len() - 1]
    }
}

/// The wyvern's wing on the face: an arm from the shoulder to a wrist near the forward rim, a thumb to the rim, and seven fingers fanning from the wrist across the half-table.
struct Wing {
    w: P2,
    s: P2,
    /// Thumb, the seven fingers leading first, then the arm; each rooted at the wrist.
    bones: Vec<Bone>,
    /// How many fingers end on the aft rim, leading first.
    rim: usize,
    /// Membrane levels: the leading strip, the six panels between fingers leading first, the panel behind the arm, the membrane in front of it.
    levels: [f64; 9],
}

impl Wing {
    fn new(table: &Table, body_half: impl Fn(f64) -> f64) -> Self {
        let (w, s) = ([5.5, 4.35], [3.4, 1.30]);
        let levels = [0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.75, 0.85, 0.85];
        let mut bones = vec![Bone { pts: bone(w, table.march(w, 50.0, 0.28, |_| -1.0), 0.0, 8), lip: (0.26, 0.16), half: (0.18, 0.11), near: 8 }];
        // Leading finger along the forward rim to the kite's point, the arm's own continuation.
        let top = [0.0, table.hi(0.0) - 0.34];
        bones.push(Bone { pts: bone(w, top, -0.75, 64), lip: (0.30, 0.18), half: (0.22, 0.12), near: 1 });
        // Four more to the aft rim, then two landing on the flank, each bowed toward the leading edge.
        let rim_deg = [152.0, 164.0, 176.0, 187.5];
        let body_deg = [205.0, 221.0];
        let bows = [-0.55, -0.65, -0.6, -0.45, -0.3, -0.2];
        for (k, deg) in rim_deg.iter().chain(&body_deg).enumerate() {
            let tip = table.march(w, *deg, 0.3, |u| body_half(u) - 0.25);
            bones.push(Bone { pts: bone(w, tip, bows[k], 64), lip: (0.20, 0.15), half: (0.18, 0.11), near: k + 2 });
        }
        bones.push(Bone { pts: bone(w, s, 0.3, 40), lip: (0.50, 0.30), half: (0.36, 0.28), near: 8 });
        Self { w, s, bones, rim: 1 + rim_deg.len(), levels }
    }

    /// Which membrane a point lies in, counted round the wrist from the thumb: 0 the leading strip, 8 in front of the arm.
    fn region(&self, p: P2) -> usize {
        let rho = (p[0] - self.w[0]).hypot(p[1] - self.w[1]);
        let psi = (p[1] - self.w[1]).atan2(p[0] - self.w[0]);
        let thumb = self.bones[0].bearing(rho);
        let rel = |a: f64| (a - thumb).rem_euclid(TAU);
        let at = rel(psi);
        self.bones[1..].iter().filter(|b| rel(b.bearing(rho)) <= at).count()
    }

    /// How far across the finger the membrane runs at `u`, mm: the rim, bitten toward the spine into a scallop between each pair of tips on the aft rim, never below the finger that ends the scallop.
    fn edge(&self, table: &Table, u: f64) -> f64 {
        let hi = table.hi(u);
        for k in 1..self.rim {
            let (a, b) = (self.bones[k].tip(), self.bones[k + 1].tip());
            if u <= a[0] && u >= b[0] {
                let t = (a[0] - u) / (a[0] - b[0]).max(1e-9);
                let chord = a[1] + (b[1] - a[1]) * t;
                let bite = 0.95 * (a[0] - b[0]).hypot(a[1] - b[1]).min(2.6) / 2.6 * (PI * t).sin().powf(0.7);
                let floor = z_on(&self.bones[k + 1].pts, u).map_or(0.0, |z| z + 0.14 + BEVEL);
                return (chord - bite).max(floor).min(hi);
            }
        }
        hi
    }

    /// Relief of the wings at a face point, mm: stepped membrane from the spine out, its bones, its scalloped trailing edge, faded at the rim.
    fn height(&self, table: &Table, u: f64, z: f64) -> f64 {
        // Under the back, the membrane's value at the back's edge.
        let p = [u, z.abs().max(body_half(u) - 0.2)];
        let hi = table.hi(u);
        // Fade widths measured square to the rim and the trailing edge.
        let steep = |f: &dyn Fn(f64) -> f64| (1.0 + ((f(u + 0.05) - f(u - 0.05)) / 0.1).powi(2)).sqrt().min(4.0);
        let rim = 1.0 - smooth(hi - 0.12 - 0.43 * steep(&|x| table.hi(x)), hi - 0.12, p[1]);
        if hi <= 0.0 || rim <= 0.0 {
            return 0.0;
        }
        let edge = self.edge(table, u);
        let hem = 1.0 - smooth(edge - BEVEL * steep(&|x| self.edge(table, x)), edge, p[1]);
        let region = self.region(p);
        let base = self.levels[region];
        let mut h = base;
        for b in &self.bones {
            let (d, t, _) = poly(p, &b.pts);
            let half = b.half.0 + (b.half.1 - b.half.0) * t;
            let rise = b.lip.0 + (b.lip.1 - b.lip.0) * t + KNUCKLE * (-((t - 0.45) / 0.05).powi(2)).exp();
            let crest = self.levels[b.near] + rise;
            let v = if region == b.near {
                // Near side: a concave ramp to the crest line at the draft rule's rate.
                let run = 1.6 * (b.lip.0.max(b.lip.1) + KNUCKLE) / RATE;
                let from = self.levels[b.near];
                from + (crest - from) * (1.0 - (d / run).min(1.0)).powf(1.6)
            } else {
                // Far side: a rounded fall to the membrane below.
                base + (crest - base) * (1.0 - smooth(0.0, half + BEVEL, d))
            };
            h = h.max(v);
        }
        h * hem * rim
    }

    /// The graver's veins, 0..1: one down each finger panel and two up the membrane in front of the arm.
    fn veins(&self, table: &Table, u: f64, z: f64) -> f64 {
        let p = [u, z.abs()];
        let hi = table.hi(u);
        if hi <= 0.0 || p[1] > hi - 0.5 {
            return 0.0;
        }
        let region = self.region(p);
        if p[1] > self.edge(table, u) - 0.55 || self.bones.iter().any(|b| poly(p, &b.pts).0 < 0.42) {
            return 0.0;
        }
        let groove = |d: f64, w: f64| 1.0 - smooth(0.15 * w, w, d);
        let mut cut: f64 = 0.0;
        if (1..=7).contains(&region) {
            let (a, b) = (self.bones[region].tip(), self.bones[region + 1].tip());
            let mid = lerp2(a, b, 0.5);
            let main = bone(lerp2(self.w, mid, 0.2), lerp2(self.w, mid, 0.9), -0.3, 32);
            let (d, t, _) = poly(p, &main);
            cut = cut.max(groove(d, 0.34 * (1.0 - 0.3 * t)));
        } else if region == 8 {
            for f in [0.35, 0.75] {
                let from = lerp2(self.s, [self.s[0] + 2.4, 1.2], f);
                let (d, t, _) = poly(p, &bone(from, lerp2(from, self.w, 0.6), -0.2, 16));
                cut = cut.max(groove(d, 0.32 * (1.0 - 0.3 * t)));
            }
        }
        cut
    }
}

/// Half the back's width on the face at `u`: broad at the shoulders and hips, waisted between, narrowing to neck and tail.
fn body_half(u: f64) -> f64 {
    let k: [(f64, f64); 13] = [(8.0, 0.56), (6.8, 0.66), (5.6, 1.05), (4.4, 1.55), (3.3, 1.70), (2.0, 1.40), (0.5, 1.08), (-1.5, 1.02), (-3.2, 1.30), (-4.3, 1.45), (-5.6, 1.20), (-7.0, 0.66), (-8.0, 0.54)];
    if u >= k[0].0 {
        return k[0].1;
    }
    if u <= k[12].0 {
        return k[12].1;
    }
    let i = k.iter().position(|q| q.0 <= u).unwrap().max(1);
    let (a, b) = (k[i - 1], k[i]);
    a.1 + (b.1 - a.1) * smooth(0.0, 1.0, (u - a.0) / (b.0 - a.0))
}

/// Joints from `start` to `end` whose pitch runs linearly from `p0` to `p1`, with a joint on every anchor between.
fn graded(start: f64, end: f64, p0: f64, p1: f64, anchors: &[f64]) -> Vec<f64> {
    let pitch = |l: f64| p0 + (p1 - p0) * ((l - start) / (end - start)).clamp(0.0, 1.0);
    let count = |a: f64, b: f64| (0..200).map(|i| (b - a) / 200.0 / pitch(a + (b - a) * (i as f64 + 0.5) / 200.0)).sum::<f64>();
    let mut stops = vec![start];
    stops.extend(anchors.iter().copied().filter(|a| *a > start + 1.0 && *a < end - 1.0));
    stops.push(end);
    stops.sort_by(f64::total_cmp);
    let mut out = vec![start];
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = count(a, b).round().max(1.0) as usize;
        let total = count(a, b);
        for i in 1..=n {
            let goal = total * i as f64 / n as f64;
            let (mut lo, mut hi) = (a, b);
            for _ in 0..40 {
                let m = 0.5 * (lo + hi);
                if count(a, m) < goal { lo = m } else { hi = m }
            }
            out.push(if i == n { b } else { 0.5 * (lo + hi) });
        }
    }
    out
}

/// The wyvern's body along the ring in hide millimetres: its back on the face, its neck and tail down the shoulders, both to the palm, flowing round the ring toward the tail.
struct Body {
    /// Joints of the back as |along| from the head's centre, the central plate spanning the first either side.
    joints: Vec<f64>,
    /// Arc from the head's centre to the palm along the parting line.
    reach: f64,
    /// The dorsal spines struck on the backbone, whose fins the backbone carries.
    spines: Vec<Spine>,
}

/// Width of each joint's groove either side, mm.
const GROOVE: f64 = 0.26;
/// How far round the ring an outer scute's joint slips per mm out from the back, mm.
const SLANT: f64 = 0.9;
/// Length of the flat plate under the spade, from the palm, mm.
const PALM_PLATE: f64 = 3.6;

impl Body {
    fn new(reach: f64, anchors: &[f64]) -> Self {
        let mut joints = vec![1.7, 5.1];
        joints.extend(graded(8.5, reach - PALM_PLATE, 3.6, 2.2, anchors));
        Self { joints, reach, spines: Vec::new() }
    }

    /// The plate holding signed `s`: where it starts and ends in `s`, flowing toward +s.
    fn plate(&self, s: f64) -> (f64, f64) {
        let l = s.abs();
        let g = &self.joints;
        if l < g[0] {
            return (-g[0], g[0]);
        }
        if l >= *g.last().unwrap() {
            let e = *g.last().unwrap();
            return if s > 0.0 { (e, 2.0 * self.reach - e) } else { (-(2.0 * self.reach - e), -e) };
        }
        let j = g.partition_point(|x| *x <= l);
        let (a, b) = (g[j - 1], g[j]);
        if s > 0.0 { (a, b) } else { (-b, -a) }
    }

    /// How deep the joints' grooves cut at signed `s`, 0..1.
    fn groove(&self, s: f64) -> f64 {
        let last = *self.joints.last().unwrap();
        let from = if s.abs() >= last {
            s.abs() - last
        } else {
            let (a, b) = self.plate(s);
            (s - a).min(b - s).max(0.0)
        };
        1.0 - smooth(0.03, GROOVE, from)
    }

    /// How far along its plate a scute stands at signed `s`, 0 at the plate's start rising to 1, falling over the free edge's bevel; flat under the spade.
    fn saw(&self, s: f64) -> f64 {
        if s.abs() >= *self.joints.last().unwrap() {
            return 0.0;
        }
        let (a, b) = self.plate(s);
        let t = ((s - a) / (b - a)).clamp(0.0, 1.0);
        t * (1.0 - smooth(b - a - BEVEL, b - a, s - a))
    }

    /// Half the body's width across the surface at `along`, mm: the face's own plan, thickening over the head's ends into neck and tail and tapering to the palm.
    fn half(&self, along: f64, rim: f64) -> f64 {
        let l = along.abs();
        let face = body_half(-along);
        let taper = 1.62 + 1.2 * (1.0 - smooth(11.0, 21.0, l));
        face + (taper.min(rim - 0.3) - face) * smooth(7.0, 12.5, l)
    }

    /// How far the outer scutes reach beside the back, mm: to the rim off the face, nowhere on it.
    fn flank(&self, along: f64, rim: f64) -> f64 {
        let half = self.half(along, rim);
        half + (rim - 0.02 - half).max(0.0) * smooth(7.2, 9.6, along.abs())
    }

    /// Height of the backbone, mm: tall on the face, lower down the shank, dipping over the head's end walls.
    fn crown(&self, along: f64) -> f64 {
        let l = along.abs();
        let face = 1.45 - 0.40 * smooth(4.8, 7.4, l);
        let over = 1.0 - 0.5 * (-((l - 8.5) / 1.1).powi(2)).exp();
        (face + (0.95 - face) * smooth(6.5, 12.5, l) - 0.17 * smooth(20.0, self.reach, l)) * over
    }

    /// Relief of the body at a hide point, mm: a keeled backbone, one scute series stepping down beside it, and slanted outer scutes to the rim.
    fn relief(&self, p: &HidePoint) -> f64 {
        let (s, w) = (p.along, p.across.abs());
        let half = self.half(s, p.rim);
        let flank = self.flank(s, p.rim);
        if w > flank + 0.2 {
            return 0.0;
        }
        let crown = self.crown(s);
        let k0 = 0.45 * half;
        let palm = smooth(self.reach - PALM_PLATE - 1.0, self.reach - PALM_PLATE, s.abs());
        let keel = (0.12 + 0.03 * palm) * (1.0 - smooth(0.0, k0, w));
        let backbone = crown - 0.16 * self.groove(s) + keel;
        let inner = crown - 0.30 + 0.12 * self.saw(s) - 0.12 * self.groove(s);
        let slid = s + SLANT * (w - half).max(0.0);
        let outer = crown - 0.54 + 0.12 * self.saw(slid);
        let top = backbone + (inner - backbone) * smooth(k0 - 0.14, k0 + 0.14, w);
        let (top, edge) = if flank > half + 0.2 { (top + (outer - top) * smooth(half - 0.2, half + 0.2, w), flank) } else { (top, half) };
        let fin = self.spines.iter().map(|sp| sp.fin_at(s, p.across)).fold(0.0, f64::max);
        top * (1.0 - smooth(edge - 0.02, edge + 0.16, w)) + fin
    }
}

/// Small round scales on the walls facing the pull, overlapping down the wall on a raised bed, the scales faded to the bed where a wall turns to face round the ring.
fn wall_scales(p: &HidePoint, s: &Sample, bore: f64, facing: f64) -> f64 {
    let wall = smooth(0.12, 0.9, p.across.abs() - p.rim) * smooth(1.0, 1.5, p.wall);
    let pattern = smooth(0.66, 0.78, facing);
    let rho = s.p[0].hypot(s.p[1]);
    if wall <= 0.0 || rho < bore + 0.45 {
        return 0.0;
    }
    let (row, col) = ((p.across.abs() - p.rim) / 0.66, s.p[0].atan2(s.p[1]) * rho / 0.80);
    let i0 = row.floor() as i64;
    let mut best: f64 = 0.0;
    for i in [i0 - 1, i0] {
        let stagger = if i.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let centre = (col - stagger).round() + stagger;
        let t = (row - i as f64) / 1.45;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let dc = (col - centre).abs();
        let half_w = 0.54 * (1.0 - t.powf(2.4)).max(0.0).powf(0.6);
        if dc >= half_w {
            continue;
        }
        best = best.max(smooth(0.0, 0.22, half_w - dc) * smooth(0.0, 0.16, 1.0 - t) * smooth(0.0, 0.14, t) * (0.35 + 0.65 * t));
    }
    wall * (0.62 + 0.38 * best * pattern) * smooth(bore + 0.45, bore + 0.9, rho)
}

/// Factory 002 as its sand master at a 13 x 19 mm face on an 18.6 mm bore, poured in Delft clay.
fn base() -> Result<RingDesign> {
    let mut d = RingDesign::default();
    let source = PRESETS.iter().find(|p| p.id == "002").unwrap().load()?;
    ImportedBase::attach(&mut d, sand_master(source)?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = true;
    d.name = "Draco — the wyvern displayed".into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 19.0;
    d.shank.head.length_mm = 13.0;
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..Default::default() };
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    setup.recipe.name = format!("{} / Delft clay", d.name);
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find(&setup.recipe.alloy).unwrap().shrink_pct;
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.flask.width_mm = 80.0;
    setup.flask.length_mm = 80.0;
    setup.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.3, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -33.0, 0.0], diameter_mm: 6.0 },
    ];
    setup.bench_notes = "Imported-stock master, one wyvern flowing round the ring: its wings displayed across the kite's pitched table, \
        an arm from each shoulder to a wrist knuckle by the forward rim and seven fingers fanning from it, the membrane stepping \
        down finger by finger from the spine to the scalloped trailing edge; its keeled back on the ridge with three low raked spines \
        struck between the wings; its neck and tail down the shoulders in graded scutes, the outer row pointed, a raked spine \
        struck on each backbone plate; an arrowhead spade across the palm; round scales on the head's walls. Every panel steps \
        down away from the parting line and every straight joint runs across the band, so the pattern pulls as drawn. Z=0 \
        parting, opposed Z withdrawal. At the bench: cut the spade's two barb notches and engrave the membrane's veins. Polish \
        the bones, the spines and the scutes' tops; leave the membranes and wall scales satin."
        .into();
    d.draft.process = setup.recipe.process;
    d.draft.sand = setup.recipe.sand;
    d.draft.min_detail_mm = setup.recipe.min_detail_mm;
    d.draft.min_section_mm = setup.recipe.min_section_mm;
    d.draft.min_draft_deg = setup.recipe.min_draft_deg;
    d.manufacturing = Some(setup);
    Ok(d)
}

/// Where each atlas column's roof ends on either side of the ridge, mm of z; zero where the column has no roof.
fn roof_edges(a: &Atlas, hide: &Hide) -> Vec<[f64; 2]> {
    (0..AW)
        .map(|x| {
            let c = hide.crest[x];
            if a.at(x, c).p[1] < a.top - 2.4 {
                return [0.0, 0.0];
            }
            let up = (c..AH - 1).find(|y| a.at(x, *y).n[2].abs() > 0.4).map_or(0.0, |y| a.at(x, y).p[2]);
            let down = (1..=c).rev().find(|y| a.at(x, *y).n[2].abs() > 0.4).map_or(0.0, |y| a.at(x, y).p[2]);
            [down, up]
        })
        .collect()
}

/// How squarely each column's wall faces the pull: the median of its wall samples' axial normal, blurred a degree and a half round the ring.
fn facing(a: &Atlas, hide: &Hide) -> Vec<f64> {
    let raw: Vec<f64> = (0..AW)
        .map(|x| {
            let mut v: Vec<f64> = (1..AH - 1)
                .map(|y| a.at(x, y))
                .filter(|s| { let p = hide.at(s); p.across.abs() - p.rim > 0.12 && s.p[0].hypot(s.p[1]) > a.bore + 0.45 })
                .map(|s| s.n[2].abs())
                .collect();
            v.sort_by(f64::total_cmp);
            if v.is_empty() { 1.0 } else { v[v.len() / 2] }
        })
        .collect();
    let k = AW / 240;
    (0..AW).map(|x| (0..=2 * k).map(|o| raw[(x + AW + o - k) % AW]).sum::<f64>() / (2 * k + 1) as f64).collect()
}

/// Where a build's surface crosses itself: the ring angle and z of each half-degree, quarter-millimetre cell holding a crossing.
fn crossing_spots(mesh: &Mesh) -> Vec<(f64, f64)> {
    let v: Vec<[f64; 3]> = mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect();
    let theta = |f: &[u32; 3]| { let p = v[f[0] as usize]; p[1].atan2(p[0]).to_degrees().rem_euclid(360.0) };
    let mut bins: Vec<Vec<[u32; 3]>> = vec![Vec::new(); 720];
    for f in &mesh.faces {
        bins[((theta(f) * 2.0) as usize).min(719)].push(*f);
    }
    let mut out = Vec::new();
    for b in 0..720 {
        let set: Vec<[u32; 3]> = [bins[(b + 719) % 720].as_slice(), bins[b].as_slice(), bins[(b + 1) % 720].as_slice()].concat();
        if csg::self_crossings(&csg::Solid { v: v.clone(), f: set.clone() }) == 0 {
            continue;
        }
        for zb in -48..48 {
            let z = zb as f64 * 0.25;
            let cell: Vec<[u32; 3]> = set.iter().filter(|f| (v[f[0] as usize][2] - z - 0.125).abs() < 0.3).copied().collect();
            if !cell.is_empty() && csg::self_crossings(&csg::Solid { v: v.clone(), f: cell }) > 0 {
                out.push((b as f64 * 0.5 + 0.25, z + 0.125));
            }
        }
    }
    out
}

/// Most relief each sample may carry near spots where the stock's own facets fold, mm: each spot's floor across a band round its |z|, rising 1.6 mm per mm round the ring, 3 mm per mm toward the parting line and at the draft rule's rate past the spot.
fn fold_caps(a: &Atlas, hide: &Hide, spots: &[(f64, f64, f64)]) -> Vec<f64> {
    let seeds: Vec<(f64, f64, f64)> = spots
        .iter()
        .map(|(theta, z, floor)| (hide.along[((theta / 360.0 * AW as f64).round() as usize) % AW], z.abs(), *floor))
        .collect();
    a.samples
        .iter()
        .map(|s| {
            let (along, z) = (hide.along[s.i % AW], s.p[2].abs());
            seeds
                .iter()
                .map(|(l, zs, floor)| floor + 1.6 * (along - l).abs() + (zs - 0.3 - z).max(0.0) * 3.0 + (z - zs - 0.3).max(0.0) * RATE)
                .fold(f64::MAX, f64::min)
        })
        .collect()
}

fn portable(lib: &mut AlphaLibrary, a: Alpha) {
    lib.insert(Alpha::from_png16(a.name.clone(), &a.to_png16().unwrap()).unwrap());
}

fn window(centre: f64, span: f64, fade: f64) -> Window {
    let mut w = Window::around(centre, span);
    w.fade_deg = fade;
    w
}

/// What the painting took from each layer, by name.
type Bites = Vec<(String, ClampReport)>;

/// Clamp a painted layer to the sand's rule, record what the rule took, and show it on the design.
fn paint(d: &mut RingDesign, lib: &mut AlphaLibrary, a: &Atlas, bites: &mut Bites, mut alpha: Alpha, height: f64, win: Window, blend: Blend) -> Result<()> {
    let cut = draft_clamp(a, &mut alpha, height)?;
    println!("  {}: the draft rule cut {} texels, at most {:.3} mm", alpha.name, cut.texels_cut, cut.worst_mm);
    bites.push((alpha.name.clone(), cut));
    let name = alpha.name.clone();
    portable(lib, alpha);
    let mut e = skin::hide_layer(d, &name, height, win);
    e.blend = blend;
    d.layers.layers.push(e);
    Ok(())
}

/// An outline through the corners, a point every 0.1 mm, counter-clockwise.
fn dense(corners: &[P2]) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for (c, n) in corners.iter().zip(corners.iter().cycle().skip(1)) {
        let steps = ((c[0] - n[0]).hypot(c[1] - n[1]) / 0.1).ceil().max(1.0) as usize;
        out.extend((0..steps).map(|q| lerp2(*c, *n, q as f64 / steps as f64)));
    }
    let area: f64 = (0..out.len()).map(|i| { let (p, q) = (out[i], out[(i + 1) % out.len()]); p[0] * q[1] - q[0] * p[1] }).sum();
    if area < 0.0 {
        out.reverse();
    }
    out
}

/// Blunted tip's half-width of a thorn, mm.
const THORN_TIP: f64 = 0.10;

/// Half the thorn's width at `x` along it, mm: its round root, then the flanks tapering to the blunted point.
fn thorn_half(x: f64, half_len: f64, half_w: f64) -> f64 {
    let root = -half_len + half_w;
    if x < root {
        (half_w * half_w - (x - root).powi(2)).max(0.0).sqrt()
    } else {
        let t = ((x - root) / (2.0 * half_len - half_w)).clamp(0.0, 1.0);
        THORN_TIP + (half_w - THORN_TIP) * (1.0 - t).sqrt()
    }
}

/// A dorsal spine seen from above: a thorn along the ring, round at its root and drawn to a point toward the tail, the point blunted square to the ring.
fn spine_outline(half_len: f64, half_w: f64) -> Vec<[f64; 2]> {
    let mut corners: Vec<P2> = (0..=10).map(|i| { let a = PI * 0.5 + PI * i as f64 / 10.0; [-half_len + half_w + half_w * a.cos(), half_w * a.sin()] }).collect();
    let n = 10;
    for i in 1..n {
        let x = -half_len + half_w + (2.0 * half_len - half_w) * i as f64 / n as f64;
        corners.push([x, -thorn_half(x, half_len, half_w)]);
    }
    corners.push([half_len, -THORN_TIP]);
    corners.push([half_len, THORN_TIP]);
    for i in (1..n).rev() {
        let x = -half_len + half_w + (2.0 * half_len - half_w) * i as f64 / n as f64;
        corners.push([x, thorn_half(x, half_len, half_w)]);
    }
    dense(&corners)
}

/// One dorsal spine: where it stands along the parting line, its thorn, the stamp's height, and the fin raked under it.
struct Spine {
    name: String,
    at: f64,
    half_len: f64,
    half_w: f64,
    height: f64,
    fin: f64,
}

impl Spine {
    /// The fin under the spine at a hide point, mm: a sharp gable inside the thorn's plan, tallest at the root and raked down to the point.
    fn fin_at(&self, along: f64, across: f64) -> f64 {
        let x = along - self.at;
        let hl = self.half_len;
        if x.abs() >= hl {
            return 0.0;
        }
        let half = 0.85 * thorn_half(x, hl, self.half_w);
        let widest = self.half_w / (2.0 * hl);
        let t = (((x + hl) / (2.0 * hl) - widest) / (1.0 - widest)).clamp(0.0, 1.0);
        let rake = 1.0 - 0.91 * t.powf(0.85);
        // Gable with a flat of 0.04 mm either side of the parting line.
        self.fin * rake * (1.0 - (across.abs() - 0.04).max(0.0) / (half - 0.04).max(1e-6)).max(0.0)
    }
}

/// The spines: three low on the table between the wings, then one on every backbone plate of neck and tail, graded to the palm; clear of the folds.
fn spine_plan(body: &Body, folds: &[f64]) -> Vec<Spine> {
    let g = &body.joints;
    let mut plates: Vec<(f64, f64)> = vec![(-g[0], g[0])];
    for w in g.windows(2) {
        plates.push((w[0], w[1]));
        plates.push((-w[1], -w[0]));
    }
    plates.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut out = Vec::new();
    for (a, b) in plates {
        let (centre, len) = (0.5 * (a + b), b - a);
        let l = centre.abs();
        let half_len = (0.42 * len).min(0.5 * len - 0.3);
        if half_len < 0.5 || l > body.reach - PALM_PLATE - 0.3 || folds.iter().any(|f| (f - centre).abs() < half_len + 1.0) {
            continue;
        }
        let face = l < 5.2;
        let grade = smooth(9.0, body.reach - 4.0, l);
        let (height, fin, half_w) = if face { (0.14, 0.40, 0.25) } else { (0.12 - 0.02 * grade, 0.70 - 0.42 * grade, 0.26 - 0.03 * grade) };
        let side = if a < 0.0 && b > 0.0 { "centre".to_string() } else if centre > 0.0 { format!("tail {:.0}", l) } else { format!("neck {:.0}", l) };
        out.push(Spine { name: format!("Dorsal spine, {side}"), at: centre, half_len, half_w, height, fin });
    }
    out
}

/// The spade at the tail's end, pointing along +x: an arrowhead 5 mm long and 3 across its shoulders, whose barbs sweep back 1.2 mm past them, cast as its hull.
fn spade_outline() -> Vec<[f64; 2]> {
    // Convex flank from the point to the shoulder, the barb's tip, then the shaft's end.
    let mut upper: Vec<P2> = (0..=14).map(|i| { let t = i as f64 / 14.0; [2.5 - 2.4 * t, 1.5 * (1.0 - (1.0 - t).powf(1.35))] }).collect();
    upper.push([-1.1, 1.36]);
    upper.push([-2.5, 0.36]);
    let mut corners = upper.clone();
    corners.extend(upper.iter().rev().take(upper.len() - 1).map(|p| [p[0], -p[1]]));
    dense(&corners)
}

/// A barb's notch, cut at the bench from the spade's hull: the wedge between one barb and the shaft, run past the hull.
fn barb_cutter(side: f64) -> Vec<[f64; 2]> {
    let pts: Vec<P2> = [[-1.12, 1.42], [-1.35, 1.62], [-2.7, 1.0], [-2.7, 0.38], [-0.45, 0.40]].iter().map(|p| [p[0], p[1] * side]).collect();
    dense(&pts)
}

fn stamp(name: String, at: (f64, f64), outline: Vec<[f64; 2]>, height: f64) -> Stamp {
    Stamp { name, theta_deg: at.0, v_mm: at.1, rot_deg: 0.0, outline, height_mm: height, sink_mm: 0.3, draft_deg: 4.0, cut: false, bench: false, along_pull: false }
}

/// The finished design, its library, and what the draft rule took from each painted layer.
fn design(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Bites)> {
    let mut d = base()?;
    let mut lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let table = Table::of(&a, &hide);
    let edges = roof_edges(&a, &hide);
    let facing = facing(&a, &hide);
    let wing = Wing::new(&table, body_half);
    let mut body = Body::new(hide.reach(), &[15.75, 22.95]);
    let mut bites = Bites::new();
    println!("  hide: {:.2} mm to the palm; joints {:?}", hide.reach(), body.joints.iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<_>>());
    println!("  wing tips {:?}", wing.bones.iter().map(|b| { let t = b.tip(); [(t[0] * 100.0).round() / 100.0, (t[1] * 100.0).round() / 100.0] }).collect::<Vec<_>>());
    let u_of = |s: &Sample| -hide.along[s.i % AW];
    let on_roof = |s: &Sample| edges[s.i % AW][1] > 0.0;
    body.spines = spine_plan(&body, &hide.folds(&a, 12.0));
    // Cast layers, each capped where the stock's facets fold.
    const WINGS: f64 = 1.5;
    const BACK: f64 = 2.0;
    const NECK: f64 = 1.7;
    const WALL: f64 = 0.28;
    let painted = |caps: &[f64]| -> Vec<(Alpha, f64, Window, Blend)> {
        let capped = |s: &Sample, h: f64| h.min(caps[s.i]);
        let mut neck = Window::except(90.0, 56.0);
        neck.fade_deg = 4.0;
        vec![
            (a.paint("Wings displayed", |s| if on_roof(s) { capped(s, wing.height(&table, u_of(s), s.p[2])) / WINGS } else { 0.0 }), WINGS, window(90.0, 58.0, 3.0), Blend::Max),
            (a.paint("The wyvern's back", |s| { let p = hide.at(s); if p.along.abs() > 10.5 { 0.0 } else { capped(s, body.relief(&p)) / BACK } }), BACK, window(90.0, 64.0, 4.0), Blend::Max),
            (a.paint("Neck and tail", |s| { let p = hide.at(s); if p.along.abs() < 6.5 { 0.0 } else { capped(s, body.relief(&p)) / NECK } }), NECK, neck, Blend::Max),
            (a.paint("Wall scales", |s| capped(s, wall_scales(&hide.at(s), s, a.bore, facing[s.i % AW]) * WALL) / WALL), WALL, window(90.0, 170.0, 8.0), Blend::Max),
        ]
    };
    let mut caps = vec![f64::MAX; AW * AH];
    let mut spots: Vec<(f64, f64, f64)> = Vec::new();
    let mut layers = painted(&caps);
    for _ in 0..5 {
        let (mut probe, mut plib, mut scratch) = (d.clone(), lib.clone(), Bites::new());
        for (alpha, height, win, blend) in &layers {
            paint(&mut probe, &mut plib, &a, &mut scratch, alpha.clone(), *height, *win, *blend)?;
        }
        let built = ringdesign_core::mesh::try_build(&probe, &plib, params)?;
        let found = crossing_spots(&built.mesh);
        if found.is_empty() {
            break;
        }
        println!("  the stock's facets fold the relief at {found:?}: capped there");
        for (theta, z) in found {
            match spots.iter_mut().find(|s| (s.0 - theta).abs() < 0.3 && (s.1 - z).abs() < 0.2) {
                Some(s) => s.2 *= 0.5,
                None => spots.push((theta, z, 0.26)),
            }
        }
        caps = fold_caps(&a, &hide, &spots);
        layers = painted(&caps);
    }
    for (alpha, height, win, blend) in layers {
        paint(&mut d, &mut lib, &a, &mut bites, alpha, height, win, blend)?;
    }

    // Membrane veins cut at the bench.
    const GRAVER: f64 = 0.05;
    let graver = a.paint("Graver's veins", |s| if on_roof(s) { wing.veins(&table, u_of(s), s.p[2]) } else { 0.0 });
    portable(&mut lib, graver);
    let mut cut = skin::hide_layer(&d, "Graver's veins", GRAVER, window(90.0, 58.0, 3.0));
    cut.blend = Blend::Subtract;
    cut.bench_only = true;
    d.layers.layers.push(cut);

    for s in &body.spines {
        d.stamps.push(stamp(s.name.clone(), hide.crest_at(&a, s.at), spine_outline(s.half_len, s.half_w), s.height));
    }
    // Spade tail at the palm pointing round the ring; barb notches cut at the bench.
    let palm = hide.crest_at(&a, body.reach - 0.02);
    d.stamps.push(stamp("Spade tail, blank".into(), palm, spade_outline(), 0.8));
    for (side, name) in [(1.0, "high"), (-1.0, "low")] {
        d.stamps.push(Stamp { sink_mm: -0.02, draft_deg: 0.0, cut: true, bench: true, ..stamp(format!("Spade tail, {name} barb cut at the bench"), palm, barb_cutter(side), 1.2) });
    }
    // Spines standing where the stock's facets fold are left off.
    let ctx = d.field_context();
    for _ in 0..2 {
        let built = ringdesign_core::mesh::try_build(&d, &lib, params)?;
        let found = crossing_spots(&built.mesh);
        if found.is_empty() {
            break;
        }
        let keep: Vec<bool> = d
            .stamps
            .iter()
            .map(|s| {
                let o = s.frame(&d, &ctx).origin;
                let theta = o[1].atan2(o[0]).to_degrees().rem_euclid(360.0);
                !(s.name.starts_with("Dorsal spine") && found.iter().any(|(t, _)| ((t - theta + 180.0).rem_euclid(360.0) - 180.0).abs() < 4.0))
            })
            .collect();
        let dropped: Vec<String> = d.stamps.iter().zip(&keep).filter(|(_, k)| !**k).map(|(s, _)| s.name.clone()).collect();
        println!("  spines over folds left off: {dropped:?} ({found:?})");
        if dropped.is_empty() {
            break;
        }
        let mut k = keep.into_iter();
        d.stamps.retain(|_| k.next().unwrap_or(true));
    }
    println!("  stamps: {}", d.stamps.len());
    Ok((d, lib, bites))
}

/// A render of the parts cropped to the middle `keep` of the frame.
fn crop_png(path: &Path, parts: &[Part], yaw: f64, pitch: f64, edge: usize, keep: f64) -> Result<()> {
    let big = render::render_parts_ss(parts, yaw, pitch, edge, edge, 2);
    let w = (edge as f64 * keep) as usize;
    let o = (edge - w) / 2;
    let mut out = Vec::with_capacity(w * w * 3);
    for y in o..o + w {
        out.extend_from_slice(&big[(y * edge + o) * 3..(y * edge + o + w) * 3]);
    }
    image::save_buffer(path, &out, w as u32, w as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Two renders side by side in one frame.
fn pair_png(path: &Path, left: &[Part], right: &[Part], yaw: f64, pitch: f64, edge: usize) -> Result<()> {
    let (l, r) = (render::render_parts_ss(left, yaw, pitch, edge, edge, 3), render::render_parts_ss(right, yaw, pitch, edge, edge, 3));
    let mut out = Vec::with_capacity(edge * edge * 6);
    for y in 0..edge {
        out.extend_from_slice(&l[y * edge * 3..(y + 1) * edge * 3]);
        out.extend_from_slice(&r[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn release_json(r: &mf::release::ReleaseReport) -> serde_json::Value {
    serde_json::json!({
        "status": format!("{:?}", r.status),
        "obstructions": r.obstructions.len(),
        "unresolved_rays": r.unresolved_rays,
        "occupied_rays": r.occupied_rays,
        "worst_draft_deg": r.worst_draft_deg,
        "low_draft_area_mm2": r.low_draft_area_mm2,
        "deepest_mm": r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max),
        "at": r.obstructions.iter().take(12).map(|o| o.world).collect::<Vec<_>>(),
    })
}

/// Hero from above the head's aft end, looking forward along the spine.
const HERO: (f64, f64) = (-0.45, 1.2);

fn write(out: &Path, draft: bool, verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let steps = if draft { (768, 320) } else { (1536, 448) };
    let params = BuildParams { theta_steps: steps.0, profile_steps: steps.1, refine: None, ..Default::default() };
    let (mut d, lib, bites) = design(params)?;
    d.build = params;
    let built = ringdesign_core::mesh::try_build(&d, &lib, params)?;
    let v = &built.report.validation;
    println!("{}: {} triangles, watertight {}, {} degenerate, {:.0} ms; stamps {} {:?}", d.name, built.mesh.faces.len(), v.watertight, built.report.quality.degenerate_faces, built.report.build_ms, built.solids.stamped, built.solids.notes);
    let metal = [Part::metal(&built.mesh, render::GOLD)];
    let edge = if draft { 1100 } else { 1600 };
    let solid = csg::Solid { v: built.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: built.mesh.faces.clone() };
    let crossings = csg::self_crossings(&solid);
    println!("  self crossings {crossings}");
    if crossings > 0 {
        println!("    at {:?}", crossing_spots(&built.mesh));
    }
    let field = ringdesign_core::castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    println!("  field: {} (undercut {:.4} mm², drag {:.1}%, wall {:.2} mm) {:?}", field.verdict.label(), field.undercut_area_mm2, 100.0 * (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9), field.thinnest_wall_mm, field.notes);
    let dfm = ringdesign_core::dfm::findings_in(&d, &lib);
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    let stones = ringdesign_core::stones::report(&d, 0.0).map_or(0, |r| r.stone_count as usize);
    let preview = ringdesign_core::gems::preview_mesh(&d, &lib).map_or(0, |m| m.faces.len());
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(&d, &lib, &setup, params)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let r = &inspection.release;
    println!("  release 0.100: {:?}, {} obstructions, {} unresolved; 0.075: {:?}, {} obstructions, {} unresolved", r.status, r.obstructions.len(), r.unresolved_rays, release_fine.status, release_fine.obstructions.len(), release_fine.unresolved_rays);
    for o in r.obstructions.iter().chain(&release_fine.obstructions) {
        let th = o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0);
        println!("    obstruction {:.3} mm deep, {:.3} mm², at theta {th:.1}, z {:.2}, r {:.2}", o.depth_mm, o.projected_area_mm2, o.world[2], o.world[0].hypot(o.world[1]));
    }
    let worst_bite = bites.iter().map(|b| b.1.worst_mm).fold(0.0, f64::max);
    let gates = [
        ("watertight, no degenerate faces", v.watertight && built.report.quality.degenerate_faces == 0),
        ("no self crossings", crossings == 0),
        ("every stamp resolved", built.solids.notes.is_empty() && built.solids.stamped == d.stamps.len()),
        ("field Castable under its own process", field.process == CastProcess::SandTwoPart && field.verdict == Verdict::Castable),
        ("release clean at 0.100", r.obstructions.is_empty() && r.unresolved_rays == 0),
        ("release clean at 0.075", release_fine.obstructions.is_empty() && release_fine.unresolved_rays == 0),
        ("clamp bite at most 0.05 mm", worst_bite <= 0.05),
        ("no DFM findings", dfm.is_empty()),
        ("stones report matches preview", stones == 0 && preview == 0),
    ];
    for (g, ok) in &gates {
        println!("  gate {}: {g}", if *ok { "pass" } else { "FAIL" });
    }
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let mut reload = serde_json::Value::Null;
    if verify {
        let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
        let cold = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = ringdesign_core::mesh::try_build(&saved, &cold, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces;
        println!("  cold reload with an empty library: {}", if same { "identical vertices and faces" } else { "CHANGED" });
        reload = serde_json::json!({ "identical": same, "vertices": rebuilt.mesh.vertices.len(), "faces": rebuilt.mesh.faces.len() });
        ensure!(same, "Saved design changed geometry");
    }
    let report = serde_json::json!({
        "design": d.name,
        "build": { "theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "ms": built.report.build_ms },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": built.report.quality.degenerate_faces, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "notes": built.solids.notes },
        "field": field,
        "dfm_findings": dfm.iter().map(|f| serde_json::json!({ "label": f.label, "message": f.message })).collect::<Vec<_>>(),
        "stones": { "report": stones, "preview_faces": preview },
        "clamp": bites.iter().map(|(n, c)| serde_json::json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>(),
        "release_0100": release_json(r),
        "release_0075": release_json(&release_fine),
        "gates": gates.iter().map(|(g, ok)| serde_json::json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "reload": reload,
        "manufacturing": mf::package::report(&d, &setup, &inspection, false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    std::fs::write(out.join("release-fine.json"), serde_json::to_vec_pretty(&release_fine)?)?;
    stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    stl::write_stl(out.join("casting-pattern.stl"), &inspection.prepared.mesh, &format!("{} / shrink compensated sand pattern", d.name))?;
    for (name, yaw, pitch) in [("hero", HERO.0, HERO.1), ("face", 0.0, PI * 0.5), ("palm", PI, PI * 0.5), ("side", PI * 0.5, 0.62), ("cheek", 0.0, 0.22), ("shoulder", 1.25, 1.05), ("reverse", PI, 0.8)] {
        render::write_png_parts(out.join(format!("{name}.png")), &metal, yaw, pitch, edge)?;
    }
    crop_png(&out.join("detail.png"), &metal, 0.3, 1.15, if draft { 2000 } else { 2800 }, 0.5)?;
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    bare.stamps.clear();
    let b = ringdesign_core::mesh::try_build(&bare, &lib, params)?;
    pair_png(&out.join("bare-finished.png"), &[Part::metal(&b.mesh, render::GOLD)], &metal, HERO.0, HERO.1, if draft { 800 } else { 1200 })?;
    let art = out.join("artwork");
    std::fs::create_dir_all(&art)?;
    for name in d.layers.referenced_alphas() {
        if let Some(a) = lib.get(name) {
            std::fs::write(art.join(format!("{}.png", name.replace([' ', '/', '\''], "-"))), a.to_png16()?)?;
        }
    }
    let failed: Vec<&str> = gates.iter().filter(|g| !g.1).map(|g| g.0).collect();
    ensure!(failed.is_empty(), "gates failed: {failed:?}");
    Ok(())
}

/// The template gate's mesh half: a design evaluated from its lifted graph rebuilds the source's mesh exactly, with an empty library.
fn graph_check(out: &Path, evaluated: &Path) -> Result<()> {
    let source = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
    let lifted = ringdesign_core::library::load_design(evaluated).with_context(|| format!("reading {}", evaluated.display()))?;
    let graph = lifted.graph.clone().context("the evaluated design carries no graph")?;
    let nodes = graph["nodes"].as_array().map_or(0, |n| n.len());
    let patches: Vec<String> = graph["nodes"].as_array().into_iter().flatten().filter(|n| n["kind"] == "design.set").map(|n| n["inputs"]["pointer"].to_string()).collect();
    let same_source = serde_json::to_value(&RingDesign { graph: None, ..lifted.clone() })? == serde_json::to_value(&source)?;
    let cold = |d: &RingDesign| mf::source_library(d, &AlphaLibrary::default()).into_owned();
    let before = ringdesign_core::mesh::try_build(&source, &cold(&source), source.build)?;
    let after = ringdesign_core::mesh::try_build(&lifted, &cold(&lifted), lifted.build)?;
    let same_mesh = before.mesh.vertices == after.mesh.vertices && before.mesh.faces == after.mesh.faces && before.mesh.normals == after.mesh.normals;
    let bytes = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
    let graph_bytes = serde_json::to_vec(&graph)?.len();
    println!("graph: {nodes} nodes, {} design.set patches {patches:?}; source identical {same_source}; mesh identical {same_mesh} ({} triangles); {graph_bytes} graph bytes", patches.len(), after.mesh.faces.len());
    std::fs::write(out.join("verification.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "cold_design_reload": true,
        "cold_graph_reload": true,
        "source_identical": same_source,
        "vertices_faces_normals_identical": same_mesh,
        "triangles": after.mesh.faces.len(),
        "nodes": nodes,
        "design_set_patches": patches,
        "graph_bytes": graph_bytes,
        "design_bytes": bytes(&out.join("design.ring.json")),
        "editable_graph_bytes": bytes(evaluated),
    }))?)?;
    ensure!(same_source && same_mesh, "the lifted graph does not rebuild the source");
    ensure!(patches.len() <= 4, "the lift needs {} design.set patches, at most 4 allowed: {patches:?}", patches.len());
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().position(|a| a == f);
    let g = flag("--graph");
    let graph = g.and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let out = args.iter().enumerate().find(|(i, a)| !a.starts_with("--") && g.is_none_or(|g| *i != g + 1)).map(|(_, a)| PathBuf::from(a)).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/draco"));
    if let Some(g) = graph {
        return graph_check(&out, &g);
    }
    write(&out, flag("--draft").is_some(), flag("--verify").is_some())
}
