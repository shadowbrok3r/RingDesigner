//! Draco, the wyvern displayed: factory 002 Kite as a Delft sand master, one wyvern from face to palm.
//! cargo run -p ringdesign-core --release --example bestiarium_draco -- [OUT_DIR] [--draft] [--verify] [--probe] [--graph EVALUATED.ring.json [--lift-ms N] [--eval-ms N]]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, Mesh, ProfileStyle, RingDesign,
    castability::{CastProcess, SandProcess, Verdict},
    csg,
    field::{Blend, Window, smootherstep},
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    manufacturing as mf,
    render::{self, Part},
    setting::{Stamp, StampTop},
    skin::{self, Atlas, ClampReport, Hide, HidePoint, Sample, draft_clamp},
    stl,
};
use std::{
    f64::consts::{PI, TAU},
    path::{Path, PathBuf},
    time::Instant,
};

const AW: usize = 2048;
const AH: usize = 768;
/// Steepest a lip climbs per mm walked out from the parting line on the table, mm.
const RATE: f64 = 0.2;
/// Width of every membrane's and bone's fall, mm.
const BEVEL: f64 = 0.8;

/// A point on the plan of the face: `u` along the ridge (+ toward the neck), `z` across the finger.
type P2 = [f64; 2];

fn lerp2(a: P2, b: P2, t: f64) -> P2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn mix(r: (f64, f64), t: f64) -> f64 {
    r.0 + (r.1 - r.0) * t.clamp(0.0, 1.0)
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

/// Distance from `p` to a polyline and the share along it of the nearest point.
fn poly(p: P2, pts: &[P2]) -> (f64, f64) {
    let n = pts.len() - 1;
    let mut best = (f64::MAX, 0.0);
    for i in 0..n {
        let (a, b) = (pts[i], pts[i + 1]);
        let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ez) / (ex * ex + ez * ez).max(1e-12)).clamp(0.0, 1.0);
        let d = (p[0] - a[0] - ex * t).hypot(p[1] - a[1] - ez * t);
        if d < best.0 {
            best = (d, (i as f64 + t) / n as f64);
        }
    }
    best
}

/// Where a polyline stands at `u`: its z, the share along it, and the u-share of its local direction.
fn at_u(pts: &[P2], u: f64) -> Option<(f64, f64, f64)> {
    let n = pts.len() - 1;
    pts.windows(2).enumerate().find(|(_, w)| (w[0][0] - u) * (w[1][0] - u) <= 0.0).map(|(i, w)| {
        let du = w[1][0] - w[0][0];
        let t = if du.abs() > 1e-12 { ((u - w[0][0]) / du).clamp(0.0, 1.0) } else { 0.0 };
        let len = du.hypot(w[1][1] - w[0][1]).max(1e-12);
        (w[0][1] + (w[1][1] - w[0][1]) * t, (i as f64 + t) / n as f64, du.abs() / len)
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

    /// Where on the aft half of the table the roof is `z` wide.
    fn aft(&self, z: f64) -> f64 {
        let (mut lo, mut hi) = (self.0[0].0, 0.0);
        for _ in 0..50 {
            let m = 0.5 * (lo + hi);
            if self.hi(m) < z { lo = m } else { hi = m }
        }
        0.5 * (lo + hi)
    }
}

/// One run of a terrace's outer edge, single-valued round the ring, carrying a lip `lip` mm proud over a crest `half` mm deep, both from its first point to its last.
struct Piece {
    pts: Vec<P2>,
    lip: (f64, f64),
    half: (f64, f64),
    /// A joint swelling the lip at 0.45 of the way along, mm.
    knuckle: f64,
}

impl Piece {
    fn covers(&self, u: f64) -> bool {
        let (a, b) = (self.pts[0][0], self.pts[self.pts.len() - 1][0]);
        u >= a.min(b) && u <= a.max(b)
    }
}

/// A membrane panel `level` mm proud, sagging `sag` toward its outer edge, which is traced piece by piece.
struct Terrace {
    level: f64,
    sag: f64,
    pieces: Vec<Piece>,
}

/// Half the run round the ring over which two joined pieces' lips blend, mm.
const JOIN: f64 = 0.4;

impl Terrace {
    /// The edge at column `u`: its z, and the lip, crest depth and squareness there, blended with the joining piece near a join.
    fn edge(&self, u: f64) -> Option<(f64, f64, f64, f64)> {
        let k = self.pieces.iter().position(|pc| pc.covers(u))?;
        let pc = &self.pieces[k];
        let (zb, t, du) = at_u(&pc.pts, u)?;
        let (mut lip, mut half, mut square) = (mix(pc.lip, t) + pc.knuckle * (-((t - 0.45) / 0.06).powi(2)).exp(), mix(pc.half, t), du.max(0.25));
        let end = |q: &Piece, first: bool| {
            let (a, b) = if first { (q.pts[0], q.pts[1]) } else { (q.pts[q.pts.len() - 1], q.pts[q.pts.len() - 2]) };
            let s = if first { 0.0 } else { 1.0 };
            (a[0], mix(q.lip, s), mix(q.half, s), ((b[0] - a[0]).abs() / (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12)).max(0.25))
        };
        let joins = [(k > 0).then(|| (end(pc, true), end(&self.pieces[k - 1], false))), (k + 1 < self.pieces.len()).then(|| (end(pc, false), end(&self.pieces[k + 1], true)))];
        for (own, other) in joins.into_iter().flatten() {
            let w = 0.5 * (1.0 - (u - own.0).abs() / JOIN).max(0.0);
            if w > 0.0 {
                lip += (other.1 - own.1) * w;
                half += (other.2 - own.2) * w;
                square += (other.3 - own.3) * w;
            }
        }
        Some((zb, lip, half, square))
    }

    /// Height at a plan point, mm: never rising walking out from the spine except up a lip at the draft rule's rate.
    fn height(&self, p: P2) -> f64 {
        let Some((zb, lip, half, square)) = self.edge(p[0]) else { return 0.0 };
        if p[1] > zb {
            return (self.level + lip) * (1.0 - smooth(0.0, BEVEL, (p[1] - zb) * square));
        }
        let d = (zb - p[1]) * square;
        let run = 1.6 * lip / (RATE / square);
        let ramp = if d <= half { 1.0 } else { (1.0 - ((d - half) / run.max(1e-9)).min(1.0)).powf(1.6) };
        self.level + self.sag * smooth(0.0, 2.5, zb - p[1]) + lip * ramp
    }
}

/// A trailing edge from `a` to `b` sagging toward the spine by `depth` mm of z at its deepest, the deepest point `peak` of the way along, as a polyline single-valued round the ring.
fn scallop(a: P2, b: P2, depth: f64, peak: f64, n: usize) -> Vec<P2> {
    let gamma = 0.5f64.ln() / peak.clamp(0.1, 0.9).ln();
    let k: f64 = 0.9;
    let base = (1.0 - k).sqrt();
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let s = t.powf(gamma);
            let phi = (((1.0 - k * (2.0 * s - 1.0).powi(2)).max(0.0)).sqrt() - base) / (1.0 - base);
            let c = lerp2(a, b, t);
            [c[0], c[1] - depth * phi]
        })
        .collect()
}

/// Deepest a scallop's bite reaches square to its chord, mm.
fn bite(pts: &[P2]) -> f64 {
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
    let l = ex.hypot(ez).max(1e-9);
    pts.iter().map(|p| ((p[0] - a[0]) * ez - (p[1] - a[1]) * ex).abs() / l).fold(0.0, f64::max)
}

/// Smooth minimum of `a` and `b` over a blend `k` wide, never above either.
fn smin(a: f64, b: f64, k: f64) -> f64 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// A scallop from `a` to `b` biting `target` mm square to its chord where the panel has room, and running `room` mm clear of `floor` toward the tips where it has not.
fn fit_scallop(a: P2, b: P2, target: f64, peak: f64, room: f64, floor: impl Fn(f64) -> f64) -> Vec<P2> {
    let slope = (b[1] - a[1]) / (b[0] - a[0]);
    let depth = target * (1.0 + slope * slope).sqrt();
    let chord = scallop(a, b, 0.0, peak, 64);
    scallop(a, b, depth, peak, 64)
        .iter()
        .zip(&chord)
        .map(|(p, c)| {
            let avail = (c[1] - floor(c[0]) - room).max(0.0);
            [c[0], c[1] - smin(c[1] - p[1], avail, 0.3).max(0.0)]
        })
        .collect()
}

/// The wyvern's wing on the face: an arm from the shoulder to the wrist, a thumb claw forward off it, three fingers to the kite's point and the aft rim, and four stepped membranes whose trailing edges scallop between the tips.
struct Wing {
    /// Behind the arm and inside the last finger, down to the spine.
    inner: Terrace,
    /// In front of the arm, to the thumb and the neck.
    fore: Terrace,
    /// Between the last two fingers.
    middle: Terrace,
    /// Between the first two fingers, the lowest.
    outer: Terrace,
    wrist: P2,
    shoulder: P2,
    tips: [P2; 3],
    /// How deep each scallop bites square to its chord, aft of the first, second and third finger, and in front of the arm.
    bites: [f64; 4],
    /// The veins' paths, one down each panel.
    veins: Vec<Vec<P2>>,
    /// Every bone, for keeping the veins off them.
    bones: Vec<Vec<P2>>,
}

impl Wing {
    fn new(table: &Table) -> Self {
        let (shoulder, wrist) = ([3.5, 1.0], [2.0, 5.75]);
        let t1 = [0.0, table.hi(0.0) - 0.85];
        let u2 = table.aft(7.3);
        let t2 = [u2 + 0.15, table.hi(u2) - 0.32];
        let u3 = table.aft(4.4);
        let t3 = [u3 + 0.22, table.hi(u3) - 0.32];
        let junction = [-7.8, body_half(-7.8) + 0.1];
        let arm = bone(shoulder, wrist, 0.0, 40);
        let f1 = bone(wrist, t1, -0.4, 48);
        let f2 = bone(wrist, t2, -0.35, 48);
        let f3 = bone(wrist, t3, -0.25, 48);
        let on = |pts: &[P2], u: f64| at_u(pts, u).map_or(-9.0, |z| z.0);
        let s1 = fit_scallop(t1, t2, 1.4, 0.38, 0.42, |u| on(&f2, u));
        let s2 = fit_scallop(t2, t3, 1.2, 0.34, 0.40, |u| on(&f3, u));
        let s3 = fit_scallop(t3, junction, 1.2, 0.45, 0.35, body_half);
        // Thumb claw: a teardrop pointing forward, round end at the wrist.
        let r0 = 0.24;
        let c0 = [wrist[0] + r0, wrist[1] + 0.2];
        let claw_tip = [c0[0] + 0.62, c0[1] + 0.12];
        let mut claw: Vec<P2> = (0..=10).map(|i| { let a = PI - PI * 0.5 * i as f64 / 10.0; [c0[0] + r0 * a.cos(), c0[1] + r0 * a.sin()] }).collect();
        claw.extend((1..=12).map(|i| { let s = i as f64 / 12.0; [c0[0] + (claw_tip[0] - c0[0]) * s, c0[1] + r0 * (1.0 - s).powf(0.8) + (claw_tip[1] - c0[1]) * s] }));
        let neck = [7.4, body_half(7.4) + 0.1];
        let free = fit_scallop(claw_tip, neck, 0.8, 0.5, 0.45, |u| if u < shoulder[0] { on(&arm, u) } else { body_half(u) });
        let bites = [bite(&s1), bite(&s2), bite(&s3), bite(&free)];
        let deepest = |pts: &[P2]| { let (a, b) = (pts[0], pts[pts.len() - 1]); *pts.iter().max_by(|p, q| { let d = |r: &P2| ((r[0] - a[0]) * (b[1] - a[1]) - (r[1] - a[1]) * (b[0] - a[0])).abs(); d(p).total_cmp(&d(q)) }).unwrap() };
        let (d1, d2, d3) = (deepest(&s1), deepest(&s2), deepest(&s3));
        let mut veins: Vec<Vec<P2>> = Vec::new();
        // One main vein down each finger panel toward its scallop, forking toward both tips.
        for (d, a, b, bow) in [(d1, t1, t2, -0.15), (d2, t2, t3, -0.1)] {
            let main = bone(lerp2(wrist, d, 0.2), lerp2(wrist, d, 0.8), bow, 24);
            let fork = main[12];
            veins.push(main);
            veins.push(bone(fork, lerp2(d, a, 0.45), 0.12, 16));
            veins.push(bone(fork, lerp2(d, b, 0.45), -0.12, 16));
        }
        // The inner panel: one vein toward its scallop, one toward the hips, each forking once.
        let root = lerp2(wrist, shoulder, 0.35);
        let to_edge = bone(lerp2(root, d3, 0.12), lerp2(root, d3, 0.82), 0.3, 24);
        let hips = [-4.3, body_half(-4.3) + 0.8];
        let to_hips = bone(lerp2(root, hips, 0.15), lerp2(root, hips, 0.85), 0.4, 24);
        veins.push(bone(to_edge[13], lerp2(d3, t3, 0.5), -0.15, 16));
        veins.push(bone(to_hips[12], [-1.0, body_half(-1.0) + 0.75], 0.15, 16));
        veins.push(to_edge);
        veins.push(to_hips);
        // The fore membrane: one vein from the thumb toward the neck.
        let fore = bone(lerp2(claw_tip, neck, 0.12), lerp2(claw_tip, neck, 0.78), 0.35, 24);
        veins.push(bone(fore[12], [4.3, body_half(4.3) + 0.8], 0.1, 16));
        veins.push(fore);
        let bones = vec![arm.clone(), f1.clone(), f2.clone(), f3.clone(), claw.clone()];
        Self {
            inner: Terrace { level: 0.85, sag: 0.03, pieces: vec![
                Piece { pts: arm, lip: (0.30, 0.34), half: (0.22, 0.30), knuckle: 0.0 },
                Piece { pts: f3, lip: (0.30, 0.08), half: (0.18, 0.07), knuckle: 0.07 },
                Piece { pts: s3, lip: (0.05, 0.05), half: (0.07, 0.07), knuckle: 0.0 },
            ] },
            fore: Terrace { level: 0.60, sag: 0.03, pieces: vec![
                Piece { pts: claw, lip: (0.30, 0.26), half: (0.40, 0.06), knuckle: 0.0 },
                Piece { pts: free, lip: (0.06, 0.05), half: (0.07, 0.07), knuckle: 0.0 },
            ] },
            middle: Terrace { level: 0.56, sag: 0.03, pieces: vec![
                Piece { pts: f2, lip: (0.30, 0.08), half: (0.18, 0.07), knuckle: 0.07 },
                Piece { pts: s2, lip: (0.05, 0.05), half: (0.07, 0.07), knuckle: 0.0 },
            ] },
            outer: Terrace { level: 0.28, sag: 0.03, pieces: vec![
                Piece { pts: f1, lip: (0.30, 0.08), half: (0.18, 0.07), knuckle: 0.07 },
                Piece { pts: s1, lip: (0.05, 0.05), half: (0.07, 0.07), knuckle: 0.0 },
            ] },
            wrist,
            shoulder,
            tips: [t1, t2, t3],
            bites,
            veins,
            bones,
        }
    }

    /// Relief of the wings at a face point, mm: the four terraces joined by their highest, clear of the rim by 0.85 mm at the kite's point and 0.3 elsewhere.
    fn height(&self, table: &Table, u: f64, z: f64) -> f64 {
        let p = [u, z.abs().max(body_half(u) - 0.2)];
        let hi = table.hi(u);
        if hi <= 0.0 {
            return 0.0;
        }
        let clear = 0.3 + 0.55 * (1.0 - smooth(1.2, 2.6, u.abs()));
        let steep = (1.0 + ((table.hi(u + 0.05) - table.hi(u - 0.05)) / 0.1).powi(2)).sqrt().min(4.0);
        let rim = 1.0 - smooth(hi - clear - 0.45 * steep, hi - clear, p[1]);
        if rim <= 0.0 {
            return 0.0;
        }
        let h = [&self.inner, &self.fore, &self.middle, &self.outer].iter().map(|t| t.height(p)).fold(0.0, f64::max);
        h * rim
    }

    /// The graver's veins, 0..1: one down each panel, clear of every bone.
    fn veins(&self, u: f64, z: f64) -> f64 {
        let p = [u, z.abs()];
        if self.bones.iter().any(|b| poly(p, b).0 < 0.45) {
            return 0.0;
        }
        self.veins.iter().map(|v| { let (d, t) = poly(p, v); let w = 0.17 * (1.0 - 0.45 * t); (1.0 - smooth(0.3 * w, w, d)) * smooth(0.0, 0.1, t) * (1.0 - smooth(0.8, 1.0, t)) }).fold(0.0, f64::max)
    }
}

/// Pitch of the matting punch's dots, mm.
const MAT_PITCH: f64 = 0.46;
/// Radius of one matting dot, mm.
const MAT_DOT: f64 = 0.19;

/// The matting punch's ground, 0..1: jittered rows of round dots over the plan.
fn matting(u: f64, z: f64) -> f64 {
    let row = MAT_PITCH * 0.866;
    let j0 = (z / row).round() as i64;
    let mut best: f64 = 0.0;
    for j in j0 - 1..=j0 + 1 {
        let shift = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let i0 = (u / MAT_PITCH - shift).round() as i64;
        for i in i0 - 1..=i0 + 1 {
            let c = [(i as f64 + shift + 0.3 * (skin::hash(i, j) - 0.5)) * MAT_PITCH, (j as f64 + 0.3 * (skin::hash(j, i + 7919) - 0.5)) * row];
            let r = (u - c[0]).hypot(z - c[1]) / MAT_DOT;
            if r < 1.0 {
                best = best.max(1.0 - r * r);
            }
        }
    }
    best
}

/// Half the back's width on the face at `u`: broad at the shoulders and hips, waisted between, running on at the width it has where it leaves the table.
fn body_half(u: f64) -> f64 {
    let k: [(f64, f64); 13] = [(9.0, 1.40), (7.4, 1.38), (6.0, 1.40), (4.6, 1.52), (3.3, 1.66), (2.0, 1.38), (0.5, 1.08), (-1.5, 1.04), (-3.2, 1.30), (-4.3, 1.45), (-5.6, 1.32), (-7.0, 1.30), (-9.0, 1.32)];
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
        let total = count(a, b);
        let n = total.round().max(1.0) as usize;
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

/// Width of each joint's groove either side, mm.
const GROOVE: f64 = 0.26;
/// How far round the ring an outer scute's joint slips per mm out from the back, mm.
const SLANT: f64 = 0.9;
/// Half the head's resting plate either side of the palm, mm.
const HEAD_HALF: f64 = 4.6;
/// Height of the ridge under the head, mm.
const HEAD_GABLE: f64 = 0.15;
/// Height of the keel along the backbone, mm.
const KEEL: f64 = 0.12;
/// Half the keel's width, the same from palm to palm, mm.
const KEEL_HALF: f64 = 0.62;
/// Radius each plate's free edge is rounded over, mm.
const EDGE_ROUND: f64 = 0.3;

/// The wyvern's body along the ring in hide millimetres: its back on the face, its neck and tail down the shoulders, both to the palm, flowing round the ring toward the tail.
struct Body {
    /// Plate joints measured from the signet's centre, with the head resting beyond the last pair.
    joints: Vec<f64>,
    /// Arc from the head's centre to the palm along the parting line.
    reach: f64,
    /// The dorsal spines on the backbone, whose fins the backbone carries.
    spines: Vec<Spine>,
}

impl Body {
    fn new(reach: f64, anchors: &[f64]) -> Self {
        let mut joints = vec![1.8, 5.2];
        joints.extend(graded(8.5, reach - HEAD_HALF, 3.6, 2.2, anchors));
        Self { joints, reach, spines: Vec::new() }
    }

    fn last(&self) -> f64 {
        *self.joints.last().unwrap()
    }

    /// The plate holding signed `s`: where it starts and ends in `s`, flowing toward +s.
    fn plate(&self, s: f64) -> (f64, f64) {
        let l = s.abs();
        let g = &self.joints;
        if l < g[0] {
            return (-g[0], g[0]);
        }
        if l >= self.last() {
            let e = self.last();
            return if s > 0.0 { (e, 2.0 * self.reach - e) } else { (-(2.0 * self.reach - e), -e) };
        }
        let j = g.partition_point(|x| *x <= l);
        let (a, b) = (g[j - 1], g[j]);
        if s > 0.0 { (a, b) } else { (-b, -a) }
    }

    /// How deep the joints' grooves cut at signed `s`, 0..1.
    fn groove(&self, s: f64) -> f64 {
        let (a, b) = self.plate(s);
        1.0 - smooth(0.03, GROOVE, (s - a).min(b - s).max(0.0))
    }

    /// Rise along a scute to its rounded free edge, zero under the head.
    fn saw(&self, s: f64) -> f64 {
        if s.abs() >= self.last() {
            return 0.0;
        }
        let (a, b) = self.plate(s);
        let t = ((s - a) / (b - a)).clamp(0.0, 1.0);
        let e = (b - s).max(0.0) / EDGE_ROUND;
        t * if e < 1.0 { (1.0 - (1.0 - e).powi(2)).sqrt() } else { 1.0 }
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
        half + (rim - 0.02 - half).max(0.0) * smooth(8.0, 9.4, along.abs())
    }

    /// Backbone height, tall on the face and lower down the shank.
    fn crown(&self, along: f64) -> f64 {
        let l = along.abs();
        let face = 1.45 - 0.40 * smooth(4.8, 7.4, l);
        let over = 1.0 - 0.5 * (-((l - 8.5) / 1.1).powi(2)).exp();
        (face + (0.95 - face) * smooth(6.5, 12.5, l) - 0.17 * smooth(20.0, self.reach - 3.0, l)) * over
    }

    /// Keeled backbone, rounded scutes and a continuous seat beneath the head.
    fn relief(&self, p: &HidePoint) -> f64 {
        let (s, w) = (p.along, p.across.abs());
        let half = self.half(s, p.rim);
        let flank = self.flank(s, p.rim);
        if w > flank + 0.2 {
            return 0.0;
        }
        let crown = self.crown(s);
        let terminal = s.abs() >= self.last();
        if terminal {
            return (crown + HEAD_GABLE * (1.0 - w / 2.2).max(0.0)) * (1.0 - smooth(p.rim - 0.25, p.rim + 0.05, w));
        }
        let groove = self.groove(s) * smooth(0.2, 0.5, w);
        let keel = KEEL * (1.0 - smooth(0.0, KEEL_HALF, w));
        let (backbone, inner, k0) = (crown - 0.16 * groove + keel, crown - 0.30 + 0.10 * self.saw(s) - 0.12 * groove, KEEL_HALF + 0.1);
        let slid = s + SLANT * (w - half).max(0.0);
        let outer = crown - 0.54 + 0.072 * self.saw(slid);
        let top = backbone + (inner - backbone) * smooth(k0 - 0.14, k0 + 0.14, w);
        let fin = self.spines.iter().map(|sp| sp.fin_at(s, p.across)).fold(0.0, f64::max);
        if flank > half + 0.2 {
            let top = top + (outer - top) * smooth(half - 0.2, half + 0.2, w);
            top * (1.0 - smooth(flank - 0.45, flank + 0.05, w)) + fin
        } else {
            top * (1.0 - smooth(half - 0.02, half + 0.16, w)) + fin
        }
    }
}

/// Size of a head wall's scales `d` mm down from its rim: 1.6 at the rim falling to 1.1 by 4.5 mm down.
fn head_scale(d: f64, _l: f64, _reach: f64) -> f64 {
    1.6 - 0.5 * smooth(0.0, 4.5, d)
}

/// Size of a shank side face's granules `l` mm round the ring from the head's centre: 1.2 off the head falling to 0.95 at the palm.
fn shank_granule(_d: f64, l: f64, reach: f64) -> f64 {
    1.2 - 0.25 * smooth(12.0, reach - 2.0, l)
}

/// The skin the head's scales stand on, mm: 0.6 of the layer's height, so the wall reads as one stroke.
const SCALE_BED: f64 = 0.24;
/// Tallest a head scale stands on its bed, mm.
const SCALE_HEIGHT: f64 = 0.16;
/// Height of every granule, mm.
const GRANULE: f64 = 0.14;
/// The skin the granules stand on, mm.
const GRANULE_BED: f64 = 0.24;
/// Sand left between two granules, mm.
const GRANULE_GAP: f64 = 0.4;
/// Depth step of the rows' table, mm.
const SCALE_STEP: f64 = 0.02;
/// Most wall the rows are laid over, mm.
const SCALE_DEPTH: f64 = 8.0;
/// Most rows down any wall.
const SCALE_ROWS: usize = 24;

/// Staggered rows of round cells over every wall facing the pull, hanging from the rim, each row an integer count round the ring.
struct Lattice {
    /// Per column, the row coordinate at every step down the wall.
    rows: Vec<Vec<f64>>,
    /// Per row, each column's position round the ring in cells.
    cols: Vec<Vec<f64>>,
    /// Per row, each column's cell size and the depth of the row's centre.
    size: Vec<Vec<f64>>,
    depth: Vec<Vec<f64>>,
}

impl Lattice {
    /// Cells sized by `size(d, l, reach)`, rows `pitch` of a cell apart.
    fn new(a: &Atlas, hide: &Hide, size: fn(f64, f64, f64) -> f64, pitch: f64) -> Self {
        let reach = hide.reach();
        let n = (SCALE_DEPTH / SCALE_STEP) as usize + 1;
        let rows: Vec<Vec<f64>> = (0..AW)
            .map(|x| {
                let l = hide.along[x].abs();
                let mut y = 0.0;
                let mut out = vec![0.0];
                for k in 1..n {
                    y += SCALE_STEP / (pitch * size((k as f64 - 0.5) * SCALE_STEP, l, reach));
                    out.push(y);
                }
                out
            })
            .collect();
        // Radius of the high side's wall `d` mm below its rim, per column.
        let walls: Vec<Vec<(f64, f64)>> = (0..AW)
            .map(|x| (hide.crest[x]..AH).map(|y| { let s = a.at(x, y); (hide.across[s.i].abs() - hide.rim[x][1], s.p[0].hypot(s.p[1])) }).filter(|q| q.0 >= 0.0).collect())
            .collect();
        let radius = |x: usize, d: f64| -> f64 {
            let w = &walls[x];
            if w.is_empty() {
                return a.bore;
            }
            let j = w.partition_point(|q| q.0 < d);
            if j == 0 { w[0].1 } else if j >= w.len() { w[w.len() - 1].1 } else {
                let (p, q) = (w[j - 1], w[j]);
                p.1 + (q.1 - p.1) * ((d - p.0) / (q.0 - p.0).max(1e-9))
            }
        };
        let (mut cols, mut sizes, mut depth) = (Vec::new(), Vec::new(), Vec::new());
        for i in 0..SCALE_ROWS {
            let goal = i as f64 + 0.5;
            let ds: Vec<f64> = (0..AW).map(|x| { let r = &rows[x]; r.partition_point(|y| *y < goal).min(n - 1) as f64 * SCALE_STEP }).collect();
            let ss: Vec<f64> = (0..AW).map(|x| size(ds[x], hide.along[x].abs(), reach)).collect();
            let widths: Vec<f64> = (0..AW).map(|x| radius(x, ds[x]) * TAU / AW as f64 / ss[x]).collect();
            let total: f64 = widths.iter().sum();
            let count = total.round().max(3.0);
            let mut acc = 0.0;
            let xs: Vec<f64> = widths.iter().map(|w| { let c = acc + 0.5 * w; acc += w; c * count / total }).collect();
            cols.push(xs);
            sizes.push(ss);
            depth.push(ds);
        }
        Self { rows, cols, size: sizes, depth }
    }

    /// The cells whose rows lie near `d` mm down the wall in column `x`: each cell's offset from the point round the ring and down the wall and its size, mm, and its row and index in the row.
    fn near(&self, x: usize, d: f64) -> Vec<(f64, f64, f64, usize, i64)> {
        let r = &self.rows[x];
        let k = ((d / SCALE_STEP) as usize).min(r.len() - 2);
        let f = (d / SCALE_STEP - k as f64).clamp(0.0, 1.0);
        let y = r[k] + (r[k + 1] - r[k]) * f;
        ((y.floor() as i64 - 1).max(0)..=(y.floor() as i64 + 1).min(SCALE_ROWS as i64 - 1))
            .map(|i| {
                let i = i as usize;
                let (s, xx) = (self.size[i][x], self.cols[i][x]);
                let stagger = if i % 2 == 0 { 0.0 } else { 0.5 };
                let k = (xx - stagger).round();
                ((xx - (k + stagger)) * s, d - self.depth[i][x], s, i, k as i64)
            })
            .collect()
    }

    /// A head wall's scales `d` mm down in column `x`, mm: each scale a disc rising from its root to its free edge toward the bore, the one above lapping over it.
    fn scales(&self, x: usize, d: f64) -> f64 {
        if d <= 0.0 {
            return 0.0;
        }
        let mut best: f64 = 0.0;
        for (du, dv, s, ..) in self.near(x, d) {
            let rad = 0.5 * s;
            let r = du.hypot(dv);
            if r >= rad {
                continue;
            }
            let tau = ((dv + rad) / (2.0 * rad)).clamp(0.0, 1.0);
            let soft = s * (0.3 - 0.15 * smooth(-0.5, 0.5, dv / rad));
            let crown = 1.0 - 0.3 * (du / rad).powi(2);
            best = best.max((0.15 * s).min(SCALE_HEIGHT) * smooth(0.0, soft, rad - r) * crown * (0.42 + 0.58 * tau.powf(0.9)));
        }
        best * smooth(0.7, 1.3, d)
    }

    /// A side face's granules `d` mm down in column `x`, mm: separate rounded domes with sand between, each whole or absent as `present` says.
    fn granules(&self, x: usize, d: f64, present: &[Vec<bool>]) -> f64 {
        if d <= 0.0 {
            return 0.0;
        }
        let mut best: f64 = 0.0;
        for (du, dv, s, i, k) in self.near(x, d) {
            let row = &present[i];
            if !row.is_empty() && !row[k.rem_euclid(row.len() as i64) as usize] {
                continue;
            }
            let rad = 0.5 * (s - GRANULE_GAP);
            let q = du.hypot(dv) / rad;
            if q < 1.0 {
                best = best.max(GRANULE * (1.0 - q * q).powf(1.2));
            }
        }
        best
    }

    /// Which cells lie whole on a wall facing the pull: per row, per cell, the fade full at its centre and its footprint clear of the rim, the bore's edge break and the head's scales.
    fn whole(&self, a: &Atlas, hide: &Hide, fade: &[f64]) -> Vec<Vec<bool>> {
        (0..SCALE_ROWS)
            .map(|i| {
                let cols = &self.cols[i];
                let count = (cols[AW - 1] + 0.5 * (cols[AW - 1] - cols[AW - 2])).round().max(1.0) as usize;
                let stagger = if i % 2 == 0 { 0.0 } else { 0.5 };
                (0..count)
                    .map(|k| {
                        let x = cols.partition_point(|c| *c < k as f64 + stagger).min(AW - 1);
                        let (d, s) = (self.depth[i][x], self.size[i][x]);
                        let rad = 0.5 * (s - GRANULE_GAP);
                        let c = hide.crest[x];
                        let Some(y) = (c..AH).find(|y| hide.across[*y * AW + x].abs() >= hide.rim[x][1] + d) else { return false };
                        let sample = a.at(x, y);
                        d - rad > 0.45 && sample.p[0].hypot(sample.p[1]) - rad > a.bore + 0.22 && fade[sample.i] > 0.9 && hide.along[x].abs() > 12.0
                    })
                    .collect()
            })
            .collect()
    }
}

/// How much of each sample a wall's relief may use: full where the wall faces the pull, fading where it turns to face round the ring, never rising again walking out from the parting line, and blurred round the ring.
fn wall_fade(hide: &Hide, lean: &[f64]) -> Vec<f64> {
    let (w, h) = (AW, AH);
    let mut mono = vec![0.0; w * h];
    for x in 0..w {
        let c = hide.crest[x];
        for dir in [1i64, -1] {
            let mut run: f64 = 1.0;
            let mut y = c as i64;
            while y >= 0 && y < h as i64 {
                let i = y as usize * w + x;
                let side = if hide.across[i] < 0.0 { 0 } else { 1 };
                if hide.across[i].abs() > hide.rim[x][side] + 0.5 {
                    run = run.min(smooth(0.55, 0.75, lean[i]));
                }
                mono[i] = run;
                y += dir;
            }
        }
    }
    let k = 6usize;
    (0..w * h).map(|i| { let (x, y) = (i % w, i / w); (0..=2 * k).map(|o| mono[y * w + (x + w + o - k) % w]).sum::<f64>() / (2 * k + 1) as f64 }).collect()
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
        an arm from each shoulder to a wrist knuckle with a thumb claw pointing forward, three fingers to the kite's point and the \
        aft rim, and four membranes stepping down from the spine, their trailing edges scalloped between the finger tips; its keeled \
        back on the ridge with a line of raked dorsal thorns from palm to palm; its neck and tail down the shoulders in graded scutes \
        with pointed outer rows; a horned wyvern head at the palm meeting the tail at its muzzle; round scales over the walls facing the pull. \
        Every panel steps down away from the parting line and every straight joint runs across the band, so the pattern pulls as \
        drawn. Z=0 parting, opposed Z withdrawal. At the bench: carve the head's horn separations, eye sockets, jaw lines and nostrils; engrave \
        the membrane veins. Polish the bones, thorns, cranial ridge and scute tops; leave the membranes and scales satin."
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

/// How far each sample may rise per mm walked out from the parting line without the draft rule cutting it.
fn allowed_rise(s: &Sample) -> f64 {
    let lean = (s.n[2] * s.p[2].signum()).clamp(0.0, 0.9995);
    lean / (1.0 - lean * lean).sqrt()
}

/// |n_z| of the bare surface blurred over a square of `r` samples, for fading relief where a wall turns from facing the pull.
fn lean_blur(a: &Atlas, r: usize) -> Vec<f64> {
    let (w, h) = (AW, AH);
    let raw: Vec<f64> = a.samples.iter().map(|s| s.n[2].abs()).collect();
    let mut rows = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            rows[y * w + x] = (0..=2 * r).map(|o| raw[y * w + (x + w + o - r) % w]).sum::<f64>() / (2 * r + 1) as f64;
        }
    }
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (lo..=hi).map(|yy| rows[yy * w + x]).sum::<f64>() / (hi - lo + 1) as f64;
        }
    }
    out
}

/// How sharply the bare surface curves inward at each sample, 1/mm: the larger of the two grid directions' concave curvature, read over two samples either side.
fn concavity(a: &Atlas) -> Vec<f64> {
    let (w, h) = (AW, AH);
    let bend = |p: &Sample, q: &Sample| {
        let dp = [q.p[0] - p.p[0], q.p[1] - p.p[1], q.p[2] - p.p[2]];
        let dn = [q.n[0] - p.n[0], q.n[1] - p.n[1], q.n[2] - p.n[2]];
        let l2 = dp.iter().map(|v| v * v).sum::<f64>().max(1e-12);
        (-(dp[0] * dn[0] + dp[1] * dn[1] + dp[2] * dn[2]) / l2).max(0.0)
    };
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if y < 3 || y + 3 >= h {
                return 0.0;
            }
            let across = bend(a.at(x, y - 2), a.at(x, y + 2));
            let around = bend(a.at((x + w - 2) % w, y), a.at((x + 2) % w, y));
            across.max(around)
        })
        .collect()
}

/// Most relief each sample may carry, mm: half the concave radius wherever a wall's stock creases under half a millimetre and `floor` at each spot where a build folded, rising from each 0.6 mm per mm round the ring, 3 per mm toward the parting line and at the draft rule's own rate away from it.
fn caps(a: &Atlas, hide: &Hide, kappa: &[f64], spots: &[(f64, f64, f64)]) -> Vec<f64> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (w, h) = (AW, AH);
    let mut cap = vec![f64::MAX; w * h];
    for (i, s) in a.samples.iter().enumerate() {
        let p = hide.at(s);
        if kappa[i] > 2.0 && p.across.abs() > p.rim - 0.2 {
            cap[i] = cap[i].min(0.5 / kappa[i]);
        }
    }
    for (theta, z, floor) in spots {
        let x0 = (theta / 360.0 * w as f64).round() as i64;
        for o in -3..=3 {
            let x = (x0 + o).rem_euclid(w as i64) as usize;
            for y in 0..h {
                let s = a.at(x, y);
                if (s.p[2].abs() - z.abs()).abs() <= 0.35 {
                    cap[y * w + x] = cap[y * w + x].min(*floor);
                }
            }
        }
    }
    let key = |v: f64| Reverse(v.to_bits());
    let mut heap: BinaryHeap<(Reverse<u64>, usize)> = cap.iter().enumerate().filter(|(_, c)| c.is_finite() && **c < f64::MAX).map(|(i, c)| (key(*c), i)).collect();
    let dist = |i: usize, j: usize| (0..3).map(|k| (a.samples[i].p[k] - a.samples[j].p[k]).powi(2)).sum::<f64>().sqrt();
    while let Some((Reverse(bits), i)) = heap.pop() {
        let c = f64::from_bits(bits);
        if c > cap[i] {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let mut next = vec![y * w + (x + 1) % w, y * w + (x + w - 1) % w];
        if y > 0 {
            next.push(i - w);
        }
        if y + 1 < h {
            next.push(i + w);
        }
        for j in next {
            let rate = if j / w == y {
                0.6
            } else if hide.across[j].abs() > hide.across[i].abs() {
                allowed_rise(&a.samples[j]).min(3.0)
            } else {
                3.0
            };
            let v = c + rate * dist(i, j);
            if v < cap[j] {
                cap[j] = v;
                heap.push((key(v), j));
            }
        }
    }
    cap
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

fn portable(lib: &mut AlphaLibrary, a: Alpha) {
    lib.insert(Alpha::from_png16(a.name.clone(), &a.to_png16().unwrap()).unwrap());
}

fn window(centre: f64, span: f64, fade: f64) -> Window {
    let mut w = Window::around(centre, span);
    w.fade_deg = fade;
    w
}

/// Lower every stroke of ink in `alpha` thinner than `r` pixels across to just under the ink line, which only ever takes metal away.
fn open_ink(alpha: &mut Alpha, r: f32) {
    let sd = alpha.signed_distance_px();
    let core = Alpha::new("core", alpha.width, alpha.height, sd.data.iter().map(|v| if *v >= r { 1.0 } else { 0.0 }).collect());
    let reach = core.signed_distance_px();
    for (v, (d, c)) in alpha.data.iter_mut().zip(sd.data.iter().zip(&reach.data)) {
        if *d > 0.0 && *c < -r {
            *v = v.min(0.49);
        }
    }
}

/// What the painting took from each layer, by name.
type Bites = Vec<(String, ClampReport)>;

/// Clamp a painted layer to the sand's rule, record what the rule took, and show it on the design.
fn paint(d: &mut RingDesign, lib: &mut AlphaLibrary, a: &Atlas, bites: &mut Bites, mut alpha: Alpha, height: f64, win: Window, blend: Blend) -> Result<()> {
    let before = alpha.data.clone();
    let cut = draft_clamp(a, &mut alpha, height)?;
    let worst = before.iter().zip(&alpha.data).enumerate().max_by(|p, q| (p.1.0 - p.1.1).total_cmp(&(q.1.0 - q.1.1))).map(|(i, _)| &a.samples[i]);
    let at = worst.map(|s| format!(" at x {} y {} theta {:.1}, z {:.2}, r {:.2}, n {:.2},{:.2},{:.2}, rise {:.2}", s.i % AW, s.i / AW, s.theta, s.p[2], s.p[0].hypot(s.p[1]), s.n[0], s.n[1], s.n[2], allowed_rise(s))).unwrap_or_default();
    println!("  {}: the draft rule cut {} texels, at most {:.3} mm{at}", alpha.name, cut.texels_cut, cut.worst_mm);
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

/// A dorsal thorn seen from above: round at its root and drawn to a point toward the tail, the point blunted square to the ring.
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

/// One dorsal thorn: where it stands along the parting line, its plan, the stamp's height, the fin raked under it, and whether a stamp is struck on the fin.
struct Spine {
    name: String,
    at: f64,
    half_len: f64,
    half_w: f64,
    height: f64,
    fin: f64,
    struck: bool,
}

impl Spine {
    /// The fin at a hide point, mm: a sharp gable inside the thorn's plan, climbing from the keel at its root to a peak near its point and dropping round to it.
    fn fin_at(&self, along: f64, across: f64) -> f64 {
        let x = along - self.at;
        let hl = self.half_len;
        if x.abs() >= hl {
            return 0.0;
        }
        let half = 0.85 * thorn_half(x, hl, self.half_w);
        let t = (x + hl) / (2.0 * hl);
        let profile = if t < PEAK { (t / PEAK).powf(1.6) } else { (1.0 - ((t - PEAK) / (1.0 - PEAK)).powi(2)).max(0.0).sqrt() };
        self.fin * profile * (1.0 - (across.abs() - 0.03).max(0.0) / (half - 0.03).max(1e-6)).max(0.0)
    }
}

/// Where along a thorn its fin peaks, as a share of its length from the root.
const PEAK: f64 = 0.78;

/// Graded thorns on backbone plates clear of the stock's folds.
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
        let half_len = (0.42 * len).min(0.5 * (len - 0.65));
        if half_len < 0.5 {
            continue;
        }
        let struck = !folds.iter().any(|f| (f - centre).abs() < half_len + 1.0);
        let face = l < 7.0;
        let grade = smooth(9.0, body.reach - 4.0, l);
        let (height, fin, half_w) = if face { (0.16, 0.60 + 0.06 * smooth(0.0, 5.0, l), 0.28) } else { (0.14 - 0.04 * grade, 0.70 - 0.42 * grade, 0.27 - 0.04 * grade) };
        let side = if a < 0.0 && b > 0.0 { "centre".to_string() } else if centre > 0.0 { format!("tail {:.0}", l) } else { format!("neck {:.0}", l) };
        out.push(Spine { name: format!("Dorsal spine, {side}"), at: centre, half_len, half_w, height, fin, struck });
    }
    out
}

/// The head's sand-monotone hull, from swept horns through broad cheeks to a long blunt muzzle.
fn head_outline() -> Vec<[f64; 2]> {
    let upper: Vec<P2> = vec![[-4.0, 0.45], [-3.85, 1.4], [-3.4, 1.65], [-2.6, 1.8], [-1.75, 1.9], [-1.1, 1.55], [-0.35, 1.22], [0.5, 0.86], [1.75, 0.83], [2.55, 1.0], [3.2, 0.82], [3.6, 0.42], [3.65, 0.0]];
    let mut corners = upper.clone();
    corners.extend(upper.iter().rev().skip(1).map(|p| [p[0], -p[1]]));
    dense(&corners)
}

/// An almond socket with blunted corners, at its signed cheek position.
fn eye_outline(side: f64) -> Vec<P2> {
    dense(&[[-1.35, side * 1.05], [-0.98, side * 1.37], [-0.52, side * 1.30], [-0.15, side * 1.05], [-0.52, side * 0.85], [-1.0, side * 0.83]])
}

/// The horn, nostril and mouth cuts follow the already sculpted head.
fn head(d: &mut RingDesign, at: (f64, f64)) {
    d.stamps.push(Stamp { top: StampTop::Ridge { rise_mm: 0.82, from: [-1.6, 0.0], to: [2.55, 0.0], end_mm: 0.35 }, sink_mm: 0.45, ..stamp("Wyvern head, cranial ridge and muzzle".into(), at, head_outline(), 0.20) });
    let mut cut = |name: &str, outline: Vec<P2>, depth: f64| {
        d.stamps.push(Stamp { tier: 1, sink_mm: depth, height_mm: 0.20, draft_deg: 0.0, cut: true, bench: true, ..stamp(name.into(), at, outline, 0.20) });
    };
    for (side, word) in [(1.0, "near"), (-1.0, "far")] {
        cut(&format!("Wyvern {word} eye socket"), eye_outline(side), 0.35);
        let horn: Vec<P2> = [[-4.15, 0.46], [-3.3, 0.55], [-2.1, 0.97], [-2.3, 1.30], [-3.45, 0.96], [-4.15, 0.93]].into_iter().map(|p| [p[0], side * p[1]]).collect();
        cut(&format!("Wyvern {word} horn separation"), dense(&horn), 0.50);
        let mouth: Vec<P2> = [[0.1, 0.73], [1.4, 0.52], [2.6, 0.56], [3.65, 0.34], [3.65, 0.64], [2.65, 0.85], [1.4, 0.82], [0.1, 1.03]].into_iter().map(|p| [p[0], side * p[1]]).collect();
        cut(&format!("Wyvern {word} jaw line"), dense(&mouth), 0.18);
        let nostril: Vec<P2> = (0..24).map(|i| { let t = TAU * i as f64 / 24.0; [2.67 + 0.29 * t.cos(), side * 0.43 + 0.22 * t.sin()] }).collect();
        cut(&format!("Wyvern {word} nostril"), dense(&nostril), 0.27);
    }
}

fn stamp(name: String, at: (f64, f64), outline: Vec<[f64; 2]>, height: f64) -> Stamp {
    Stamp { name, theta_deg: at.0, v_mm: at.1, rot_deg: 0.0, outline, height_mm: height, sink_mm: 0.3, draft_deg: 4.0, cut: false, bench: false, along_pull: false, tier: 0, top: StampTop::Flat }
}

/// The finished design, its library, what the draft rule took from each painted layer, and the scallops' bites.
fn design(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Bites, [f64; 4])> {
    let mut d = base()?;
    let mut lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let table = Table::of(&a, &hide);
    let edges = roof_edges(&a, &hide);
    let wing = Wing::new(&table);
    let scales = Lattice::new(&a, &hide, head_scale, 0.62);
    let granules = Lattice::new(&a, &hide, shank_granule, 0.866);
    let lean = lean_blur(&a, 6);
    let fade = wall_fade(&hide, &lean);
    let whole = granules.whole(&a, &hide, &fade);
    let kappa = concavity(&a);
    let mut body = Body::new(hide.reach(), &[15.75, 22.95]);
    let mut bites = Bites::new();
    let r2 = |p: P2| [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0];
    println!("  hide: {:.2} mm to the palm; joints {:?}", hide.reach(), body.joints.iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<_>>());
    println!("  wing: shoulder {:?} wrist {:?} tips {:?}; scallops bite {:?} mm square to their chords", r2(wing.shoulder), r2(wing.wrist), wing.tips.map(r2), wing.bites.map(|b| (b * 100.0).round() / 100.0));
    let u_of = |s: &Sample| -hide.along[s.i % AW];
    let on_roof = |s: &Sample| edges[s.i % AW][1] > 0.0;
    body.spines = spine_plan(&body, &hide.folds(&a, 12.0));
    const WINGS: f64 = 1.2;
    const BACK: f64 = 2.4;
    const NECK: f64 = 1.7;
    const WALL: f64 = SCALE_BED + SCALE_HEIGHT;
    let painted = |caps: &[f64]| -> Vec<(Alpha, f64, Window, Blend)> {
        let capped = |s: &Sample, h: f64| h.min(caps[s.i]).max(0.0);
        let mut neck = Window::except(90.0, 56.0);
        neck.fade_deg = 4.0;
        vec![
            (a.paint("Wings displayed", |s| if on_roof(s) { capped(s, wing.height(&table, u_of(s), s.p[2])) / WINGS } else { 0.0 }), WINGS, window(90.0, 58.0, 3.0), Blend::Max),
            (a.paint("The wyvern's back", |s| { let p = hide.at(s); if p.along.abs() > 10.5 { 0.0 } else { capped(s, body.relief(&p)) / BACK } }), BACK, window(90.0, 64.0, 4.0), Blend::Max),
            (a.paint("Neck and tail", |s| { let p = hide.at(s); if p.along.abs() < 6.5 { 0.0 } else { capped(s, body.relief(&p)) / NECK } }), NECK, neck, Blend::Max),
            (
                { let mut al = a.paint("Scaled walls", |s| {
                    let p = hide.at(s);
                    let rho = s.p[0].hypot(s.p[1]);
                    let d = p.across.abs() - p.rim;
                    let keep = smooth(a.bore + 0.15, a.bore + 0.45, rho) * (1.0 - smooth(10.5, 12.5, p.along.abs()));
                    let h = if d > 0.0 { SCALE_BED * keep * smooth(0.0, 0.7, d) + scales.scales(s.i % AW, d) * smooth(0.9, 1.0, keep * fade[s.i]) } else { 0.0 };
                    capped(s, h) / WALL
                }); if std::env::var("DRACO_OPEN").is_ok() { open_ink(&mut al, 22.0); } al },
                WALL,
                Window::default(),
                Blend::Max,
            ),
            (
                { let mut al = a.paint("Granular flanks", |s| {
                    let p = hide.at(s);
                    let rho = s.p[0].hypot(s.p[1]);
                    let d = p.across.abs() - p.rim;
                    let keep = smooth(a.bore + 0.08, a.bore + 0.22, rho) * smooth(10.5, 12.5, p.along.abs());
                    let h = if d > 0.0 { GRANULE_BED * keep * smooth(0.0, 0.45, d) + granules.granules(s.i % AW, d, &whole) * smooth(0.9, 1.0, keep) } else { 0.0 };
                    capped(s, h) / (GRANULE_BED + GRANULE)
                }); if std::env::var("DRACO_OPEN").is_ok() { open_ink(&mut al, 22.0); } al },
                GRANULE_BED + GRANULE,
                Window::default(),
                Blend::Max,
            ),
        ]
    };
    let mut spots: Vec<(f64, f64, f64)> = Vec::new();
    let mut cap = caps(&a, &hide, &kappa, &spots);
    let mut layers = painted(&cap);
    for _ in 0..6 {
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
        cap = caps(&a, &hide, &kappa, &spots);
        layers = painted(&cap);
    }
    if std::env::var("DRACO_DFM").is_ok() {
        for (alpha, ..) in &layers {
            println!("    {}: ink/gap px {:?}", alpha.name, alpha.min_feature_px());
        }
        let only = |f: &(dyn Fn(&Sample) -> f64 + Sync)| a.paint("probe", |s| f(s)).min_feature_px();
        println!("    fade alone {:?}", only(&|s| fade[s.i]));
        println!("    bed alone {:?}", only(&|s| { let p = hide.at(s); if p.across.abs() > p.rim { smooth(0.0, 0.8, p.across.abs() - p.rim) } else { 0.0 } }));
        println!("    bed and fade {:?}", only(&|s| { let p = hide.at(s); if p.across.abs() > p.rim { smooth(0.0, 0.8, p.across.abs() - p.rim) * fade[s.i] } else { 0.0 } }));
        let variant = |no_caps: bool, no_head: bool, no_pattern: bool, no_bore: bool, no_fade: bool| {
            only(&|s| {
                let p = hide.at(s);
                let rho = s.p[0].hypot(s.p[1]);
                let d = p.across.abs() - p.rim;
                let keep = (if no_fade { 1.0 } else { fade[s.i] }) * (if no_bore { 1.0 } else { smooth(a.bore + 0.15, a.bore + 0.45, rho) }) * (if no_head { 1.0 } else { 1.0 - smooth(10.5, 12.5, p.along.abs()) });
                let h = if d > 0.0 { SCALE_BED * keep * smooth(0.0, 1.0, d) + if no_pattern { 0.0 } else { scales.scales(s.i % AW, d) * smooth(0.75, 1.0, keep) } } else { 0.0 };
                (if no_caps { h } else { h.min(cap[s.i]) }) / WALL
            })
        };
        println!("    all {:?}", variant(false, false, false, false, false));
        println!("    no caps {:?}", variant(true, false, false, false, false));
        println!("    no head mask {:?}", variant(false, true, false, false, false));
        println!("    no pattern {:?}", variant(false, false, true, false, false));
        println!("    no bore {:?}", variant(false, false, false, true, false));
        println!("    no fade {:?}", variant(false, false, false, false, true));
        let bed = |s: &Sample| { let p = hide.at(s); if p.across.abs() > p.rim { smooth(0.0, 0.8, p.across.abs() - p.rim) } else { 0.0 } };
        println!("    bed and bore {:?}", only(&|s| bed(s) * smooth(a.bore + 0.15, a.bore + 0.45, s.p[0].hypot(s.p[1]))));
        println!("    bed and caps {:?}", only(&|s| (bed(s) * 0.45).min(cap[s.i]) / 0.45));
        println!("    bed, bore and caps {:?}", only(&|s| (bed(s) * 0.45 * smooth(a.bore + 0.15, a.bore + 0.45, s.p[0].hypot(s.p[1]))).min(cap[s.i]) / 0.45));
        let bad: Vec<(f64, f64, f64)> = a.samples.iter().filter(|s| cap[s.i] < 0.2 && bed(s) > 0.5).take(2_000_000).map(|s| (s.theta, s.p[2], s.p[0].hypot(s.p[1]))).collect();
        println!("    capped wall samples {}; e.g. {:?}", bad.len(), bad.iter().step_by((bad.len() / 12).max(1)).collect::<Vec<_>>());
    }
    if std::env::var("DRACO_DEBUG").is_ok() {
        let x: usize = std::env::var("DRACO_DEBUG").unwrap().parse().unwrap_or(225);
        let walls = &layers[3].0;
        for y in 0..AH {
            let s = a.at(x, y);
            if (y as i64 - std::env::var("DRACO_Y").ok().and_then(|v| v.parse::<i64>().ok()).unwrap_or(-100)).abs() < 14 {
                let p = hide.at(s);
                println!("    y {y} z {:.3} r {:.3} across {:.3} rim {:.3} h {:.4} cap {:.4} rise {:.3} kappa {:.2}", s.p[2], s.p[0].hypot(s.p[1]), p.across, p.rim, walls.data[s.i] as f64 * WALL, cap[s.i].min(9.0), allowed_rise(s), kappa[s.i]);
            }
        }
    }
    for (alpha, height, win, blend) in layers {
        paint(&mut d, &mut lib, &a, &mut bites, alpha, height, win, blend)?;
    }

    // Membrane veins cut at the bench.
    const GRAVER: f64 = 0.06;
    let graver = a.paint("Graver's veins", |s| if on_roof(s) { wing.veins(u_of(s), s.p[2]) } else { 0.0 });
    portable(&mut lib, graver);
    let mut cut = skin::hide_layer(&d, "Graver's veins", GRAVER, window(90.0, 58.0, 3.0));
    cut.blend = Blend::Subtract;
    cut.bench_only = true;
    d.layers.layers.push(cut);

    // The table's bare ground round the wings matted with a punch at the bench.
    const MAT: f64 = 0.07;
    let ground = a.paint("Matted ground", |s| {
        if !on_roof(s) {
            return 0.0;
        }
        let (u, z) = (u_of(s), s.p[2].abs());
        let hi = table.hi(u);
        let free = (1.0 - smooth(0.0, 0.06, wing.height(&table, u, z))) * smooth(body_half(u) + 0.3, body_half(u) + 0.5, z) * (1.0 - smooth(hi - 0.55, hi - 0.35, z)) * (1.0 - smooth(7.9, 8.4, u.abs()));
        free * matting(u, s.p[2])
    });
    portable(&mut lib, ground);
    let mut mat = skin::hide_layer(&d, "Matted ground", MAT, window(90.0, 58.0, 3.0));
    mat.blend = Blend::Subtract;
    mat.bench_only = true;
    d.layers.layers.push(mat);

    for s in body.spines.iter().filter(|s| s.struck) {
        d.stamps.push(stamp(s.name.clone(), hide.crest_at(&a, s.at), spine_outline(s.half_len, s.half_w), s.height));
    }
    // The head rests across the palm with the tail meeting its muzzle.
    let palm = hide.crest_at(&a, body.reach - 0.02);
    head(&mut d, palm);
    // Thorns standing where the stock's facets fold are left as fins alone.
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
        println!("  thorns over folds left as fins: {dropped:?} ({found:?})");
        if dropped.is_empty() {
            break;
        }
        let mut k = keep.into_iter();
        d.stamps.retain(|_| k.next().unwrap_or(true));
    }
    println!("  stamps: {} ({} thorns on {} plates)", d.stamps.len(), d.stamps.iter().filter(|s| s.name.starts_with("Dorsal")).count(), body.spines.len());
    Ok((d, lib, bites, wing.bites))
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

/// Sand slots the release scan found in the parting plane, narrowest first: ring angle, width.
fn crest_slots(r: &mf::release::ReleaseReport) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = r.sand_findings.iter().filter(|f| f.point[2].abs() < 1e-6).map(|f| (f.point[1].atan2(f.point[0]).to_degrees().rem_euclid(360.0), f.width_mm)).collect();
    out.sort_by(|p, q| p.1.total_cmp(&q.1));
    out
}

/// The parting line's outline: the mesh's outermost radius at z = 0 in each 0.01-degree bin.
fn crest_outline(mesh: &Mesh) -> Vec<f64> {
    let n = 36000;
    let mut r = vec![0.0f64; n];
    for f in &mesh.faces {
        let t: Vec<[f64; 3]> = f.iter().map(|&i| { let p = mesh.vertices[i as usize]; [p.0 as f64, p.1 as f64, p.2 as f64] }).collect();
        let cut: Vec<[f64; 2]> = (0..3)
            .filter_map(|a| {
                let (p, q) = (t[a], t[(a + 1) % 3]);
                ((p[2] <= 0.0 && q[2] > 0.0) || (q[2] <= 0.0 && p[2] > 0.0)).then(|| { let s = p[2] / (p[2] - q[2]); [p[0] + (q[0] - p[0]) * s, p[1] + (q[1] - p[1]) * s] })
            })
            .collect();
        if cut.len() == 2 {
            let steps = (((cut[1][0] - cut[0][0]).hypot(cut[1][1] - cut[0][1])) / 0.0015).ceil() as usize + 1;
            for k in 0..=steps {
                let s = k as f64 / steps as f64;
                let (x, y) = (cut[0][0] + (cut[1][0] - cut[0][0]) * s, cut[0][1] + (cut[1][1] - cut[0][1]) * s);
                if x.hypot(y) < 10.5 {
                    continue;
                }
                let b = ((y.atan2(x).to_degrees().rem_euclid(360.0)) * 100.0) as usize % n;
                r[b] = r[b].max(x.hypot(y));
            }
        }
    }
    for b in 0..n {
        if r[b] == 0.0 {
            r[b] = r[(b + n - 1) % n];
        }
    }
    r
}

/// Every notch in the parting line's outline at least 0.05 mm deep: its angle, depth and width round the ring 0.1 mm above its floor, narrowest first.
fn crest_notches(mesh: &Mesh) -> Vec<(f64, f64, f64)> {
    let r = crest_outline(mesh);
    let n = r.len();
    let at = |i: i64| r[i.rem_euclid(n as i64) as usize];
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    for i in 0..n as i64 {
        if !(at(i) <= at(i - 1) && at(i) < at(i + 1)) {
            continue;
        }
        let floor = at(i);
        let level = floor + 0.1;
        let (mut a, mut b) = (i, i);
        while at(a - 1) < level && i - a < 3000 {
            a -= 1;
        }
        while at(b + 1) < level && b - i < 3000 {
            b += 1;
        }
        let (mut pa, mut pb) = (a, b);
        while at(pa - 1) >= at(pa) && a - pa < 3000 {
            pa -= 1;
        }
        while at(pb + 1) >= at(pb) && pb - b < 3000 {
            pb += 1;
        }
        let depth = at(pa).min(at(pb)) - floor;
        if depth < 0.05 {
            continue;
        }
        let width = (b - a) as f64 * 0.01_f64.to_radians() * level;
        let theta = i as f64 * 0.01;
        if !out.iter().any(|o| (o.0 - theta).abs() < 0.3) {
            out.push((theta, depth, width));
        }
    }
    out.sort_by(|p, q| p.2.total_cmp(&q.2));
    out
}

/// Hero: three-quarters from the forward end, Caiman's angle.
const HERO: (f64, f64) = (0.48, 1.0);

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn write(out: &Path, draft: bool, verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let steps = if draft { (768, 320) } else { (1536, 448) };
    let params = BuildParams { theta_steps: steps.0, profile_steps: steps.1, refine: None, ..Default::default() };
    let (mut d, lib, bites, scallops) = design(params)?;
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
    let drag = 100.0 * (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9);
    println!("  field: {} (undercut {:.4} mm², drag {drag:.1}%, wall {:.2} mm) {:?}", field.verdict.label(), field.undercut_area_mm2, field.thinnest_wall_mm, field.notes);
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
    let (slots, slots_fine) = (crest_slots(r), crest_slots(&release_fine));
    println!("  crest slots 0.100: {slots:?}\n  crest slots 0.075: {slots_fine:?}");
    let notches = crest_notches(&built.mesh);
    println!("  crest notches, narrowest first (theta, depth, width 0.1 mm over the floor): {:?}", notches.iter().take(8).map(|n| [(n.0 * 10.0).round() / 10.0, (n.1 * 1000.0).round() / 1000.0, (n.2 * 1000.0).round() / 1000.0]).collect::<Vec<_>>());
    let narrowest = notches.iter().map(|n| n.2).fold(f64::MAX, f64::min);
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
        ("no crest notch under 0.30 mm", narrowest >= 0.30),
    ];
    for (g, ok) in &gates {
        println!("  gate {}: {g}", if *ok { "pass" } else { "FAIL" });
    }
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let mut reload = serde_json::Value::Null;
    let mut open = serde_json::Value::Null;
    if verify {
        let t = Instant::now();
        let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
        let read = ms(t);
        let t = Instant::now();
        let cold = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let bake = ms(t);
        let t = Instant::now();
        let rebuilt = ringdesign_core::mesh::try_build(&saved, &cold, params)?;
        let build = ms(t);
        let t = Instant::now();
        let verdict = ringdesign_core::castability::attributed_field_report(&saved, &cold, &saved.draft, 256, 128).verdict;
        let judge = ms(t);
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces;
        println!("  cold reload with an empty library: {} (read {read:.0} ms, bake {bake:.0}, build {build:.0}, verdict {judge:.0}: {})", if same { "identical vertices and faces" } else { "CHANGED" }, verdict.label());
        reload = serde_json::json!({ "identical": same, "vertices": rebuilt.mesh.vertices.len(), "faces": rebuilt.mesh.faces.len() });
        open = serde_json::json!({ "read_ms": read, "bake_ms": bake, "build_ms": build, "verdict_ms": judge, "total_ms": read + bake + build + judge });
        ensure!(same, "Saved design changed geometry");
    }
    let clamp = bites.iter().map(|(n, c)| serde_json::json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>();
    let report = serde_json::json!({
        "design": d.name,
        "build": { "theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "ms": built.report.build_ms },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": built.report.quality.degenerate_faces, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "notes": built.solids.notes },
        "wing": { "scallop_bites_mm": { "first_to_second": scallops[0], "second_to_third": scallops[1], "third_to_spine": scallops[2], "fore_edge": scallops[3] } },
        "field": field,
        "drag_pct": drag,
        "dfm_findings": dfm.iter().map(|f| serde_json::json!({ "label": f.label, "message": f.message })).collect::<Vec<_>>(),
        "stones": { "report": stones, "preview_faces": preview },
        "clamp": clamp,
        "release_0100": release_json(r),
        "release_0075": release_json(&release_fine),
        "crest_slots": { "release_scan_0100": slots, "release_scan_0075": slots_fine, "notches_theta_depth_width": notches, "narrowest_notch_mm": narrowest },
        "gates": gates.iter().map(|(g, ok)| serde_json::json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "reload": reload,
        "manufacturing": mf::package::report(&d, &setup, &inspection, false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    std::fs::write(out.join("release-fine.json"), serde_json::to_vec_pretty(&release_fine)?)?;
    if verify {
        let bytes = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
        std::fs::write(out.join("verification.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "cold_design_reload": reload,
            "clamp_worst_mm": worst_bite,
            "clamp": clamp,
            "drag_pct": drag,
            "open_ms": open,
            "design_bytes": bytes(&out.join("design.ring.json")),
        }))?)?;
    }
    stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    stl::write_stl(out.join("casting-pattern.stl"), &inspection.prepared.mesh, &format!("{} / shrink compensated sand pattern", d.name))?;
    for (name, yaw, pitch) in [("hero", HERO.0, HERO.1), ("face", 0.0, PI * 0.5), ("palm", PI, PI * 0.5), ("side", PI * 0.5, 0.62), ("cheek", 0.0, 0.22), ("shoulder", 1.25, 1.05), ("reverse", PI, 0.8)] {
        render::write_png_parts(out.join(format!("{name}.png")), &metal, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("face-300.png"), &metal, 0.0, PI * 0.5, 300)?;
    crop_png(&out.join("detail.png"), &metal, 0.3, 1.15, if draft { 2000 } else { 2800 }, 0.5)?;
    crop_png(&out.join("head.png"), &metal, PI, 1.3, if draft { 2400 } else { 3200 }, 0.30)?;
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

/// The template gate's mesh half: a design evaluated from its lifted graph rebuilds the source's mesh exactly, with an empty library; the lift's and evaluation's times join the verification record.
fn graph_check(out: &Path, evaluated: &Path, lift_ms: Option<f64>, eval_ms: Option<f64>) -> Result<()> {
    let source = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
    let t = Instant::now();
    let lifted = ringdesign_core::library::load_design(evaluated).with_context(|| format!("reading {}", evaluated.display()))?;
    let read = ms(t);
    let graph = lifted.graph.clone().context("the evaluated design carries no graph")?;
    let nodes = graph["nodes"].as_array().map_or(0, |n| n.len());
    let patches: Vec<String> = graph["nodes"].as_array().into_iter().flatten().filter(|n| n["kind"] == "design.set").map(|n| n["inputs"]["pointer"].to_string()).collect();
    let same_source = serde_json::to_value(&RingDesign { graph: None, ..lifted.clone() })? == serde_json::to_value(&source)?;
    let cold = |d: &RingDesign| mf::source_library(d, &AlphaLibrary::default()).into_owned();
    let before = ringdesign_core::mesh::try_build(&source, &cold(&source), source.build)?;
    let t = Instant::now();
    let lib = cold(&lifted);
    let bake = ms(t);
    let t = Instant::now();
    let after = ringdesign_core::mesh::try_build(&lifted, &lib, lifted.build)?;
    let build = ms(t);
    let same_mesh = before.mesh.vertices == after.mesh.vertices && before.mesh.faces == after.mesh.faces && before.mesh.normals == after.mesh.normals;
    let bytes = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
    let graph_bytes = serde_json::to_vec(&graph)?.len();
    println!("graph: {nodes} nodes, {} design.set patches {patches:?}; source identical {same_source}; mesh identical {same_mesh} ({} triangles); {graph_bytes} graph bytes", patches.len(), after.mesh.faces.len());
    let path = out.join("verification.json");
    let mut v: serde_json::Value = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| serde_json::json!({}));
    v["cold_graph_reload"] = serde_json::json!(true);
    v["source_identical"] = serde_json::json!(same_source);
    v["vertices_faces_normals_identical"] = serde_json::json!(same_mesh);
    v["triangles"] = serde_json::json!(after.mesh.faces.len());
    v["nodes"] = serde_json::json!(nodes);
    v["design_set_patches"] = serde_json::json!(patches);
    v["graph_bytes"] = serde_json::json!(graph_bytes);
    v["graph_over_3mb_flag"] = serde_json::json!(graph_bytes > 3_000_000);
    v["editable_graph_bytes"] = serde_json::json!(bytes(evaluated));
    v["template_open_ms"] = serde_json::json!({ "lift_cli": lift_ms, "evaluate_cli": eval_ms, "read_evaluated": read, "bake": bake, "build": build });
    std::fs::write(&path, serde_json::to_vec_pretty(&v)?)?;
    ensure!(same_source && same_mesh, "the lifted graph does not rebuild the source");
    ensure!(patches.len() <= 4, "the lift needs {} design.set patches, at most 4 allowed: {patches:?}", patches.len());
    Ok(())
}

/// Builds at draft and renders the views that judge the read, without the gates.
fn look(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let params = BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..Default::default() };
    let (d, lib, bites, _) = design(params)?;
    let built = ringdesign_core::mesh::try_build(&d, &lib, params)?;
    println!("{} triangles, worst bite {:.3}", built.mesh.faces.len(), bites.iter().map(|b| b.1.worst_mm).fold(0.0, f64::max));
    let metal = [Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch, edge) in [("face", 0.0, PI * 0.5, 1000), ("face-300", 0.0, PI * 0.5, 300), ("hero", HERO.0, HERO.1, 1000), ("side", PI * 0.5, 0.62, 800), ("cheek", 0.0, 0.22, 800), ("shoulder", 1.25, 1.05, 800), ("reverse", PI, 0.8, 800)] {
        render::write_png_parts(out.join(format!("{name}.png")), &metal, yaw, pitch, edge)?;
    }
    Ok(())
}

/// Prints the table's plan, the hide's measures, the wing's skeleton and the stock's sharpest concave creases.
fn probe() -> Result<()> {
    let d = base()?;
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let table = Table::of(&a, &hide);
    println!("reach {:.2}, bore {:.2}, top {:.2}, folds {:?}", hide.reach(), a.bore, a.top, hide.folds(&a, 12.0));
    for i in -18..=18 {
        let u = i as f64 * 0.5;
        println!("  u {u:5.1}: hi {:.2} body {:.2}", table.hi(u), body_half(u));
    }
    let wing = Wing::new(&table);
    println!("  wing tips {:?}, bites {:?}", wing.tips, wing.bites);
    // Plan of the wings, hill-shaded, 30 px per mm, +u to the right.
    let (px, half_u, half_z) = (30.0, 9.5, 10.0);
    let (w, h) = ((2.0 * half_u * px) as usize, (2.0 * half_z * px) as usize);
    let height = |i: usize, j: usize| {
        let (u, z) = (i as f64 / px - half_u, half_z - j as f64 / px);
        if z.abs() < body_half(u) - 0.2 {
            return 1.4;
        }
        let h = wing.height(&table, u, z);
        let hi = table.hi(u);
        let free = (1.0 - smooth(0.0, 0.06, h)) * smooth(body_half(u) + 0.3, body_half(u) + 0.5, z.abs()) * (1.0 - smooth(hi - 0.55, hi - 0.35, z.abs()));
        h - 0.05 * wing.veins(u, z) - 0.05 * free * matting(u, z)
    };
    let hs: Vec<f64> = (0..w * h).map(|k| height(k % w, k / w)).collect();
    let mut img = vec![0u8; w * h];
    for j in 1..h - 1 {
        for i in 1..w - 1 {
            let (dx, dy) = ((hs[j * w + i + 1] - hs[j * w + i - 1]) * px * 0.5, (hs[(j - 1) * w + i] - hs[(j + 1) * w + i]) * px * 0.5);
            let shade = (0.55 + 0.9 * (-0.6 * dx + 0.6 * dy) / (1.0 + dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            img[j * w + i] = ((0.35 * shade + 0.65 * shade * (0.4 + 0.6 * hs[j * w + i] / 1.4)) * 255.0) as u8;
        }
    }
    if let Some(path) = std::env::args().skip_while(|a| a != "--probe").nth(1).filter(|a| !a.starts_with("--")) {
        image::save_buffer(&path, &img, w as u32, h as u32, image::ColorType::L8)?;
        println!("  plan written to {path}");
    }
    let kappa = concavity(&a);
    let mut worst: Vec<(f64, usize)> = kappa.iter().enumerate().map(|(i, k)| (*k, i)).filter(|(k, _)| *k > 0.5).collect();
    worst.sort_by(|p, q| q.0.total_cmp(&p.0));
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for (k, i) in worst {
        let s = &a.samples[i];
        if seen.iter().any(|(t, z)| (t - s.theta).abs() < 2.0 && (z - s.p[2]).abs() < 1.0) {
            continue;
        }
        seen.push((s.theta, s.p[2]));
        println!("  concave 1/{:.2} mm at theta {:.1}, z {:.2}, r {:.2}", 1.0 / k, s.theta, s.p[2], s.p[0].hypot(s.p[1]));
        if seen.len() > 24 {
            break;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().position(|a| a == f);
    let value = |f: &str| flag(f).and_then(|i| args.get(i + 1)).cloned();
    let takes = ["--graph", "--lift-ms", "--eval-ms"];
    let out = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && !takes.iter().any(|t| flag(t).is_some_and(|j| j + 1 == *i)))
        .map(|(_, a)| PathBuf::from(a))
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/draco"));
    if let Some(g) = value("--graph") {
        let num = |f: &str| value(f).and_then(|v| v.parse::<f64>().ok());
        return graph_check(&out, Path::new(&g), num("--lift-ms"), num("--eval-ms"));
    }
    if flag("--probe").is_some() {
        return probe();
    }
    if flag("--look").is_some() {
        return look(&out);
    }
    write(&out, flag("--draft").is_some(), flag("--verify").is_some())
}
