//! Vepres — Prunus, *Straif, the blackthorn*, rethought for lost wax: a dark, knobbly blackthorn twig thrown over the
//! ring's head with long spurs pointing every way, white blossom along it, and a black sloe hanging from it on a short
//! stalk with its calyx. Three block-out options share this file (`--option`), all lost wax at a 0.8 mm section.
//! cargo build --release -p ringdesign-core --example vepres_prunus
//! target/release/examples/vepres_prunus [OUT_DIR] [--draft] [--verify] [--option wax-twig|wax-calyx|wax-twig-calyx]
use anyhow::{Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Attach, Component, Document, Feature, Operation, Placement, Stage, SurfaceKind, builders, stored},
    castability::{self, CastProcess},
    csg, dfm,
    gem::{Gem, GemCut, GemForm},
    library, mesh, render, skin, stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

type P3 = [f64; 3];

/// How far a spur bends along its run, mm per mm squared.
const SPUR_BEND: f64 = 0.03;
/// A spur's point: its radius where the round cap closes it, the investment's detail floor across.
const SPUR_POINT_R: f64 = 0.1;
/// The lost-wax floors: Logan's 0.8 mm section, investment's 0.15 mm detail.
const MIN_SECTION_MM: f64 = 0.8;

/// The house's finish: the twig's oxidised wood, its deep fissures, the calyx's half-dark, and the sloe's bloom.
const WOOD_TINT: [f32; 3] = [0.11, 0.075, 0.036];
const WOOD_DEEP_TINT: [f32; 3] = [0.035, 0.025, 0.013];
const CALYX_TINT: [f32; 3] = [0.11, 0.075, 0.035];
const ANTIQUE_MID: [f32; 3] = [0.42, 0.31, 0.14];
const ANTIQUE_DARK: [f32; 3] = [0.10, 0.07, 0.035];
const SLOE_TINT: [f32; 3] = [0.05, 0.056, 0.08];
const ANTIQUE_PASSES: usize = 40;
const ANTIQUE_SHALLOW_MM: f64 = 0.015;
const ANTIQUE_DEEP_MM: f64 = 0.04;

/// How the sloe is held: four low claws with a small calyx at its stalk, or five sepals and a cup with no collet.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
enum Hold {
    Claws,
    Calyx,
}

/// Where the twig runs: diagonally over the table from shoulder to shoulder, or up each shoulder to the face.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
enum Layout {
    Diagonal,
    Shoulders,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Opt {
    slug: &'static str,
    base: &'static str,
    layout: Layout,
    hold: Hold,
    /// The sloe's centre on the table (x round the ring, z along the finger), its size, and its girdle's rise over the table.
    sloe_xz: [f64; 2],
    sloe_mm: f64,
    sloe_rise_mm: f64,
}

const OPTIONS: [Opt; 3] = [
    Opt { slug: "wax-twig", base: "012", layout: Layout::Diagonal, hold: Hold::Claws, sloe_xz: [-0.4, -2.35], sloe_mm: 5.6, sloe_rise_mm: 0.6 },
    Opt { slug: "wax-calyx", base: "013", layout: Layout::Shoulders, hold: Hold::Calyx, sloe_xz: [-0.5, -0.3], sloe_mm: 6.2, sloe_rise_mm: 0.5 },
    Opt { slug: "wax-twig-calyx", base: "013", layout: Layout::Diagonal, hold: Hold::Calyx, sloe_xz: [-0.3, -2.2], sloe_mm: 5.6, sloe_rise_mm: 0.5 },
];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: P3) -> P3 {
    mul(a, 1.0 / len(a).max(1e-12))
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn smooth01(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// Smooth value noise on a unit lattice, 0 to 1.
fn noise(x: f64, y: f64, seed: i64) -> f64 {
    let (i, j) = (x.floor() as i64, y.floor() as i64);
    let (u, v) = (smooth01(x - x.floor()), smooth01(y - y.floor()));
    let h = |a: i64, b: i64| skin::hash(a * 7919 + seed, b * 104_729 + seed * 31);
    lerp(lerp(h(i, j), h(i + 1, j), u), lerp(h(i, j + 1), h(i + 1, j + 1), u), v)
}

/// The bare stock's outer surface, read by rays from the finger's axis.
struct Ground {
    band: std::sync::Arc<mesh::Mesh>,
    top: f64,
}
impl Ground {
    fn at(&self, theta_deg: f64, across: f64) -> Option<(P3, P3)> {
        cad::surface_hit(&self.band, theta_deg, across)
    }
    /// The point on the ground under a world point, read along the ray from the axis through it.
    fn under(&self, p: P3) -> Option<(P3, P3)> {
        self.at(p[1].atan2(p[0]).to_degrees(), p[2])
    }
}

/// A closed tube round `path` (at least two points), radius `r(i, around_angle)` at each station; both ends capped
/// flat, or the far end with a round cap of the last radius when `round_tip`.
fn tube(path: &[P3], around: usize, r: impl Fn(usize, f64) -> f64, round_tip: bool) -> csg::Solid {
    let n = path.len();
    let tangent = |i: usize| unit(sub(path[(i + 1).min(n - 1)], path[i.saturating_sub(1)]));
    // Rotation-minimising frames by double reflection.
    let t0 = tangent(0);
    let seed = if t0[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let mut u = unit(cross(cross(t0, seed), t0));
    let mut frames = vec![(t0, u)];
    for i in 1..n {
        let (tp, up) = frames[i - 1];
        let v1 = sub(path[i], path[i - 1]);
        let c1 = dot(v1, v1).max(1e-18);
        let ul = sub(up, mul(v1, 2.0 / c1 * dot(v1, up)));
        let tl = sub(tp, mul(v1, 2.0 / c1 * dot(v1, tp)));
        let ti = tangent(i);
        let v2 = sub(ti, tl);
        let c2 = dot(v2, v2);
        u = if c2 < 1e-18 { ul } else { sub(ul, mul(v2, 2.0 / c2 * dot(v2, ul))) };
        u = unit(sub(u, mul(ti, dot(u, ti))));
        frames.push((ti, u));
    }
    let mut s = csg::Solid::default();
    for (i, &(t, u)) in frames.iter().enumerate() {
        let b = cross(t, u);
        for k in 0..around {
            let a = 2.0 * PI * k as f64 / around as f64;
            let rr = r(i, a);
            s.v.push(add(path[i], add(mul(u, rr * a.cos()), mul(b, rr * a.sin()))));
        }
    }
    let ring = |i: usize, k: usize| (i * around + k % around) as u32;
    for i in 0..n - 1 {
        for k in 0..around {
            s.f.push([ring(i, k), ring(i + 1, k), ring(i + 1, k + 1)]);
            s.f.push([ring(i, k), ring(i + 1, k + 1), ring(i, k + 1)]);
        }
    }
    // The near end: a flat fan.
    s.v.push(path[0]);
    let c0 = (s.v.len() - 1) as u32;
    for k in 0..around {
        s.f.push([c0, ring(0, k), ring(0, k + 1)]);
    }
    let (t, u) = frames[n - 1];
    let b = cross(t, u);
    let mut last: Vec<u32> = (0..around).map(|k| ring(n - 1, k)).collect();
    if round_tip {
        let rr: Vec<f64> = (0..around).map(|k| r(n - 1, 2.0 * PI * k as f64 / around as f64)).collect();
        let steps = 5;
        for j in 1..steps {
            let q = (j as f64 / steps as f64) * PI * 0.5;
            let mut next = Vec::with_capacity(around);
            for k in 0..around {
                let a = 2.0 * PI * k as f64 / around as f64;
                let rad = rr[k] * q.cos();
                s.v.push(add(path[n - 1], add(mul(t, rr[k] * q.sin()), add(mul(u, rad * a.cos()), mul(b, rad * a.sin())))));
                next.push((s.v.len() - 1) as u32);
            }
            for k in 0..around {
                let (a0, a1, b0, b1) = (last[k], last[(k + 1) % around], next[k], next[(k + 1) % around]);
                s.f.push([a0, b0, b1]);
                s.f.push([a0, b1, a1]);
            }
            last = next;
        }
        let rmean = rr.iter().sum::<f64>() / around as f64;
        s.v.push(add(path[n - 1], mul(t, rmean)));
    } else {
        s.v.push(path[n - 1]);
    }
    let c1 = (s.v.len() - 1) as u32;
    for k in 0..around {
        s.f.push([c1, last[(k + 1) % around], last[k]]);
    }
    orient_outward(&mut s);
    s
}

/// Running means over `half` stations either side, the window narrowing to keep both ends put.
fn relax(p: &[P3], half: usize, passes: usize) -> Vec<P3> {
    let mut q = p.to_vec();
    for _ in 0..passes {
        q = (0..q.len())
            .map(|i| {
                let h = half.min(i).min(q.len() - 1 - i);
                let s = q[i - h..=i + h].iter().fold([0.0; 3], |m, v| add(m, *v));
                mul(s, 1.0 / (2 * h + 1) as f64)
            })
            .collect();
    }
    q
}

/// `p` resampled every `step` mm of arc.
fn resample(p: &[P3], step: f64) -> Vec<P3> {
    let mut acc = vec![0.0];
    for w in p.windows(2) {
        acc.push(acc.last().unwrap() + len(sub(w[1], w[0])));
    }
    let total = *acc.last().unwrap();
    let n = (total / step).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(n + 1);
    let mut j = 0;
    for i in 0..=n {
        let s = total * i as f64 / n as f64;
        while j + 1 < acc.len() - 1 && acc[j + 1] < s {
            j += 1;
        }
        let t = ((s - acc[j]) / (acc[j + 1] - acc[j]).max(1e-12)).clamp(0.0, 1.0);
        out.push(add(p[j], mul(sub(p[j + 1], p[j]), t)));
    }
    out
}

/// One twig: its centreline every 0.1 mm, its radius along it before the bark, the ground's normal under each
/// station, and the knots (nodes) where spurs and blossom grow.
struct Twig {
    name: String,
    path: Vec<P3>,
    radius: Vec<f64>,
    up: Vec<P3>,
    nodes: Vec<usize>,
}

/// Blackthorn bark at `s` mm along and `arc` mm round: short, wavering fissures that break and restart, small
/// plates between them, and scattered lenticel dashes across. Returns mm out from the round section.
fn bark(s: f64, arc: f64, seed: i64) -> f64 {
    const PITCH: f64 = 0.42;
    let col = (arc / PITCH).floor();
    let mut out: f64 = 0.0;
    for c in [col - 1.0, col, col + 1.0] {
        let ci = c as i64;
        let wave = 0.10 * (s / 1.3 + 1.7 * c).sin() + 0.06 * (s / 0.55 + 2.9 * c).sin();
        let centre = (c + 0.5 + 0.3 * (skin::hash(ci, seed) - 0.5)) * PITCH + wave;
        // Each fissure runs a varying length, then breaks.
        let run = 1.2 + 1.6 * skin::hash(ci, seed + 5);
        let seg = ((s + 3.1 * skin::hash(ci, seed + 9)) / run).floor() as i64;
        if skin::hash(ci * 131 + seg, seed + 13) < 0.3 {
            continue;
        }
        let along = ((s + 3.1 * skin::hash(ci, seed + 9)) / run).fract();
        let fade = smooth01(along / 0.2) * smooth01((1.0 - along) / 0.2);
        let d = (arc - centre).abs();
        out = out.min(-0.075 * fade * (1.0 - smooth01(d / 0.07)));
    }
    // Plates swell a little between fissures; lenticels stand as short dashes round the twig.
    let plate = 0.025 * (noise(s / 0.9, arc / 0.5, seed + 21) - 0.5);
    let (ls, la) = (s / 0.7, arc / 0.55);
    let lid = (ls.floor() as i64, la.floor() as i64);
    let lent = if skin::hash(lid.0 * 17 + lid.1, seed + 33) < 0.28 {
        let (fs, fa) = (ls.fract() - 0.5, la.fract() - 0.5);
        0.03 * (1.0 - smooth01(fs.abs() * 0.7 / 0.04)) * (1.0 - smooth01(fa.abs() * 0.55 / 0.16))
    } else {
        0.0
    };
    out + plate + lent
}

/// A twig through the ground points `(theta, across)` given in order, standing `proud` of its radius over the ground,
/// its radius running `r0` at its middle to `r1` at its ends, diving into the band over the last `dive` mm at each end
/// marked to dive. Nodes every so often, swelling the wood.
fn twig(g: &Ground, name: &str, stations: &[(f64, f64)], r0: f64, r1: f64, proud: f64, dive: [bool; 2], node_every: f64, seed: i64) -> Result<Twig> {
    let mut raw = Vec::new();
    let mut ups = Vec::new();
    for &(th, z) in stations {
        let Some((p, n)) = g.at(th, z) else { bail!("{name}: no ground at {th:.1} deg, {z:.2} mm") };
        raw.push((p, n));
    }
    // Centre over the ground, before the dives.
    let centre: Vec<P3> = raw.iter().map(|(p, n)| add(*p, mul(*n, r0 * proud))).collect();
    let centre = resample(&relax(&resample(&centre, 0.1), 15, 3), 0.1);
    let n = centre.len();
    let total = 0.1 * (n - 1) as f64;
    let dive_mm = 2.4;
    let mut path = Vec::with_capacity(n);
    let mut radius = Vec::with_capacity(n);
    for (i, c) in centre.iter().enumerate() {
        let s = 0.1 * i as f64;
        let mid = 1.0 - ((s - 0.5 * total).abs() / (0.5 * total)).powi(2);
        let r = lerp(r1, r0, mid.clamp(0.0, 1.0));
        let (_, nn) = g.under(*c).unwrap_or(([0.0; 3], unit(*c)));
        let mut sink = 0.0;
        if dive[0] {
            sink += (1.0 - smooth01(s / dive_mm)) * (proud + 1.3) * r;
        }
        if dive[1] {
            sink += (1.0 - smooth01((total - s) / dive_mm)) * (proud + 1.3) * r;
        }
        path.push(sub(*c, mul(nn, sink)));
        radius.push(r);
        ups.push(nn);
    }
    // Relaxed, and trimmed where the narrowing window leaves a kink at either buried end.
    let trim = 8;
    let path = relax(&path, 6, 2)[trim..n - trim].to_vec();
    let mut radius = radius[trim..n - trim].to_vec();
    let ups: Vec<P3> = ups[trim..n - trim].to_vec();
    let total = total - 0.2 * trim as f64;
    // Nodes at irregular spacing, never within 1.5 mm of an end.
    let mut nodes = Vec::new();
    let mut s = 1.8 + 0.6 * skin::hash(seed, 3);
    let mut k = 0;
    while s < total - 1.5 {
        nodes.push((s / 0.1).round() as usize);
        k += 1;
        s += node_every * (0.7 + 0.6 * skin::hash(seed + k, 17));
    }
    for &j in &nodes {
        for (i, r) in radius.iter_mut().enumerate() {
            let d = (i as f64 - j as f64) * 0.1;
            *r *= 1.0 + 0.16 * (-(d / 0.45).powi(2)).exp();
        }
    }
    // A slow waver in the wood's girth.
    for (i, r) in radius.iter_mut().enumerate() {
        *r *= 1.0 + 0.06 * (noise(i as f64 * 0.1 / 1.7, 0.5, seed + 1) - 0.5);
    }
    // The tightest bend against the wood's girth there: a tube crosses itself where it bends tighter than it is thick.
    let worst = (2..path.len() - 2)
        .map(|i| {
            let (a, b, c) = (path[i - 2], path[i], path[i + 2]);
            let (x, y, z) = (len(sub(b, a)), len(sub(c, b)), len(sub(c, a)));
            let area = 0.5 * len(cross(sub(b, a), sub(c, a)));
            (x * y * z / (4.0 * area).max(1e-12) / (radius[i] + 0.1), i)
        })
        .fold((f64::MAX, 0), |m, v| if v.0 < m.0 { v } else { m });
    println!("  {name}: {:.1} mm, tightest bend {:.2} of its girth at {:.1} mm", 0.1 * (path.len() - 1) as f64, worst.0, 0.1 * worst.1 as f64);
    Ok(Twig { name: name.into(), path, radius, up: ups, nodes })
}

fn twig_solid(t: &Twig, seed: i64) -> csg::Solid {
    tube(&t.path, 40, |i, a| {
        let r = t.radius[i];
        // The bark fades out over the dived ends.
        r + bark(i as f64 * 0.1, a * r, seed)
    }, false)
}

/// The frame at station `i` of a twig: tangent, up (the ground's normal square to it), and side.
fn frame_at(t: &Twig, i: usize) -> (P3, P3, P3) {
    let n = t.path.len();
    let tg = unit(sub(t.path[(i + 2).min(n - 1)], t.path[i.saturating_sub(2)]));
    let up = unit(sub(t.up[i], mul(tg, dot(t.up[i], tg))));
    (tg, up, cross(tg, up))
}

/// A blackthorn spur: from inside the twig's wood at `foot`, out along `dir` bending a little toward `bend`, `length`
/// mm past the twig's skin `skin` mm from the foot; thick and flared where it leaves the wood, tapering to a blunt point
/// that keeps the 0.8 mm section.
fn spur_solid(foot: P3, dir: P3, bend: P3, skin_mm: f64, length: f64, base_r: f64, seed: i64) -> csg::Solid {
    let total = skin_mm + length;
    let steps = (total / 0.08).ceil() as usize;
    let path: Vec<P3> = (0..=steps)
        .map(|k| {
            let t = total * k as f64 / steps as f64;
            let past = (t - skin_mm).max(0.0);
            add(foot, add(mul(dir, t), mul(bend, SPUR_BEND * past * past)))
        })
        .collect();
    tube(&path, 20, |i, a| {
        let t = total * i as f64 / steps as f64;
        let past = (t - skin_mm).max(0.0);
        let u = (past / length).clamp(0.0, 1.0);
        // A straight cone from the base to a sharp point a detail's width across, slightly convex near the root.
        let core = lerp(base_r, SPUR_POINT_R, u.powf(0.9));
        let flare = 0.42 * (1.0 - smooth01(past / 1.0)).powi(2);
        let wood = if u < 0.5 { 0.6 * bark(t, a * core, seed) * (1.0 - u * 2.0) } else { 0.0 };
        core + flare + wood
    }, true)
}

/// An open blossom, sculpted as a closed solid in its own frame (z out of the ground): five rounded petals each domed
/// along its midline and cupped a little, parted by notches, round a raised heart with a ring of stamen beads. Its foot
/// stands `sink` under z = 0 so every petal keeps the 0.8 mm section over its margin.
fn blossom_solid(diameter: f64, sink: f64, turn: f64) -> csg::Solid {
    const AROUND: usize = 200;
    const RINGS: usize = 30;
    let r = 0.5 * diameter;
    let outline = |phi: f64| {
        let c = (2.5 * (phi - turn)).cos();
        let lo = 0.03_f64.sqrt();
        let k = ((c * c + 0.03).sqrt() - lo) / (1.03_f64.sqrt() - lo);
        (r * (0.40 + 0.60 * k.powf(0.7)), k)
    };
    let margin = (MIN_SECTION_MM - sink + 0.02).max(0.12);
    let top = |u: f64, phi: f64| {
        let (_, petal) = outline(phi);
        let dome = 0.36 * (1.0 - u * u).max(0.0).sqrt() * (0.5 + 0.5 * petal);
        // Each petal cups: its rim lifts a little over its middle.
        let cup = 0.17 * smooth01((u - 0.55) / 0.4) * petal;
        let heart = 0.36 * (1.0 - smooth01(u / 0.22));
        let stamens = 0.17 * (1.0 - smooth01((u - 0.33).abs() / 0.07)) * (0.5 + 0.5 * (10.0 * (phi - turn)).cos()).powi(6);
        margin + dome + cup + heart + stamens
    };
    let mut s = csg::Solid::default();
    s.v.push([0.0, 0.0, top(0.0, 0.0)]);
    for j in 1..=RINGS {
        let u = j as f64 / RINGS as f64;
        for i in 0..AROUND {
            let phi = 2.0 * PI * i as f64 / AROUND as f64;
            let (rr, _) = outline(phi);
            s.v.push([u * rr * phi.cos(), u * rr * phi.sin(), top(u, phi)]);
        }
    }
    let foot = s.v.len() as u32;
    for i in 0..AROUND {
        let phi = 2.0 * PI * i as f64 / AROUND as f64;
        let (rr, _) = outline(phi);
        // A short rounded margin: in a little, then down to the foot.
        s.v.push([0.97 * rr * phi.cos(), 0.97 * rr * phi.sin(), -sink]);
    }
    s.v.push([0.0, 0.0, -sink]);
    let bottom = foot + AROUND as u32;
    let ring = |j: usize, i: usize| (1 + (j - 1) * AROUND + i % AROUND) as u32;
    for i in 0..AROUND {
        s.f.push([0, ring(1, i), ring(1, i + 1)]);
        for j in 1..RINGS {
            s.f.push([ring(j, i), ring(j + 1, i), ring(j + 1, i + 1)]);
            s.f.push([ring(j, i), ring(j + 1, i + 1), ring(j, i + 1)]);
        }
        let (a, b) = (ring(RINGS, i), ring(RINGS, i + 1));
        let (c, d) = (foot + i as u32, foot + ((i + 1) % AROUND) as u32);
        s.f.push([a, c, d]);
        s.f.push([a, d, b]);
        s.f.push([bottom, d, c]);
    }
    s
}

/// A solid given in a local frame (x, y in the ground, z out) moved to stand at `origin`.
fn placed(mut s: csg::Solid, origin: P3, x: P3, z: P3) -> csg::Solid {
    let z = unit(z);
    let x = unit(sub(x, mul(z, dot(x, z))));
    let y = cross(z, x);
    for v in &mut s.v {
        *v = add(origin, add(mul(x, v[0]), add(mul(y, v[1]), mul(z, v[2]))));
    }
    s
}

/// The sloe's frame: girdle centre, normal, and a direction in its plane toward the stalk.
#[derive(Clone, Copy, serde::Serialize)]
struct SloeFrame {
    girdle: P3,
    normal: P3,
    to_stalk: P3,
    radius: f64,
    crown: f64,
}

/// The cabochon's dome radius at height `z` over its girdle.
fn dome_r(f: &SloeFrame, z: f64) -> f64 {
    if z <= 0.0 { f.radius } else { f.radius * (1.0 - (z / f.crown).powi(2)).max(0.0).sqrt() }
}

/// One sepal of the calyx, hugging the fruit: from under the girdle up its dome to `reach` of the crown, `half_deg`
/// wide at its base, rounding to a blunt leaf tip, its back `MIN_SECTION_MM` off the stone with a midrib.
fn sepal_solid(f: &SloeFrame, centre_deg: f64, half_deg: f64, reach: f64) -> csg::Solid {
    let (nv, nw) = (40usize, 14usize);
    let z0 = -0.75;
    let z1 = reach * f.crown;
    let ex = f.to_stalk;
    let ey = cross(f.normal, ex);
    let at = |v: f64, w: f64, out: f64| -> P3 {
        let z = lerp(z0, z1, v);
        let tip = (1.0 - v.powf(1.2)).max(0.0).powf(0.8);
        let half = (half_deg.to_radians() * (0.35 + 0.65 * tip) * (1.0 - 0.15 * v)).max(0.07 / (f.radius + 0.5));
        let phi = centre_deg.to_radians() + w * half;
        let rib = 0.12 * (1.0 - w * w) * (1.0 - v);
        let r = dome_r(f, z.max(0.0)) + 0.03 + out * (lerp(MIN_SECTION_MM + 0.04, 0.42, v.powf(1.5)) + rib);
        // At the top the sepal's back curls in a little onto the fruit.
        add(f.girdle, add(mul(f.normal, z + out * 0.15 * v * v), add(mul(ex, r * phi.cos()), mul(ey, r * phi.sin()))))
    };
    let mut s = csg::Solid::default();
    for layer in 0..2 {
        for i in 0..=nv {
            for j in 0..=nw {
                let v = i as f64 / nv as f64 * 0.985;
                let w = -1.0 + 2.0 * j as f64 / nw as f64;
                s.v.push(at(v, w, layer as f64));
            }
        }
    }
    let id = |l: usize, i: usize, j: usize| (l * (nv + 1) * (nw + 1) + i * (nw + 1) + j) as u32;
    for i in 0..nv {
        for j in 0..nw {
            // Outer face out, inner face toward the stone.
            s.f.push([id(1, i, j), id(1, i + 1, j), id(1, i + 1, j + 1)]);
            s.f.push([id(1, i, j), id(1, i + 1, j + 1), id(1, i, j + 1)]);
            s.f.push([id(0, i, j), id(0, i + 1, j + 1), id(0, i + 1, j)]);
            s.f.push([id(0, i, j), id(0, i, j + 1), id(0, i + 1, j + 1)]);
        }
    }
    // Side walls along w = -1 and w = +1, the base and the tip.
    for i in 0..nv {
        let (a, b, c, d) = (id(0, i, 0), id(0, i + 1, 0), id(1, i, 0), id(1, i + 1, 0));
        s.f.push([a, b, d]);
        s.f.push([a, d, c]);
        let (a, b, c, d) = (id(0, i, nw), id(0, i + 1, nw), id(1, i, nw), id(1, i + 1, nw));
        s.f.push([a, d, b]);
        s.f.push([a, c, d]);
    }
    for j in 0..nw {
        let (a, b, c, d) = (id(0, 0, j), id(0, 0, j + 1), id(1, 0, j), id(1, 0, j + 1));
        s.f.push([a, d, b]);
        s.f.push([a, c, d]);
        let (a, b, c, d) = (id(0, nv, j), id(0, nv, j + 1), id(1, nv, j), id(1, nv, j + 1));
        s.f.push([a, b, d]);
        s.f.push([a, d, c]);
    }
    orient_outward(&mut s);
    s
}

/// A sepal spread out from under the fruit over the ground: a pointed, domed leaf `reach` mm past the girdle's
/// radius at `centre_deg`, its foot under the girdle, falling to the table and lifting its tip a little.
fn flared_sepal(f: &SloeFrame, centre_deg: f64, rise: f64, reach: f64) -> csg::Solid {
    let ex = f.to_stalk;
    let ey = cross(f.normal, ex);
    let a = centre_deg.to_radians();
    let out = add(mul(ex, a.cos()), mul(ey, a.sin()));
    let across = cross(f.normal, out);
    let (nv, nw) = (36usize, 16usize);
    let r0 = f.radius * 0.55;
    let length = f.radius - r0 + reach;
    let mut s = csg::Solid::default();
    let at = |v: f64, w: f64, top: bool| -> P3 {
        let along = r0 + length * v;
        let width = 0.95 * (1.0 - v.powf(1.6)).max(0.0).powf(0.6) * (0.6 + 0.4 * (PI * v).sin()) + 0.42;
        let y = w * width * 0.5;
        // From under the girdle down to the table, then a slight lift to the tip.
        let drop = -0.25 - rise * smooth01((along - f.radius * 0.8) / 0.9) + 0.25 * smooth01((v - 0.75) / 0.25);
        let thick = MIN_SECTION_MM * (1.0 - 0.15 * w * w) + 0.12 * (1.0 - w.abs()) * (1.0 - v);
        let z = if top { drop } else { drop - thick };
        add(f.girdle, add(mul(f.normal, z), add(mul(out, along), mul(across, y))))
    };
    for layer in 0..2 {
        for i in 0..=nv {
            for j in 0..=nw {
                s.v.push(at(i as f64 / nv as f64 * 0.98, -1.0 + 2.0 * j as f64 / nw as f64, layer == 0));
            }
        }
    }
    let id = |l: usize, i: usize, j: usize| (l * (nv + 1) * (nw + 1) + i * (nw + 1) + j) as u32;
    for i in 0..nv {
        for j in 0..nw {
            s.f.push([id(0, i, j), id(0, i + 1, j), id(0, i + 1, j + 1)]);
            s.f.push([id(0, i, j), id(0, i + 1, j + 1), id(0, i, j + 1)]);
            s.f.push([id(1, i, j), id(1, i + 1, j + 1), id(1, i + 1, j)]);
            s.f.push([id(1, i, j), id(1, i, j + 1), id(1, i + 1, j + 1)]);
        }
    }
    for i in 0..nv {
        let (a, b, c, d) = (id(0, i, 0), id(0, i + 1, 0), id(1, i, 0), id(1, i + 1, 0));
        s.f.push([a, d, b]);
        s.f.push([a, c, d]);
        let (a, b, c, d) = (id(0, i, nw), id(0, i + 1, nw), id(1, i, nw), id(1, i + 1, nw));
        s.f.push([a, b, d]);
        s.f.push([a, d, c]);
    }
    for j in 0..nw {
        let (a, b, c, d) = (id(0, 0, j), id(0, 0, j + 1), id(1, 0, j), id(1, 0, j + 1));
        s.f.push([a, b, d]);
        s.f.push([a, d, c]);
        let (a, b, c, d) = (id(0, nv, j), id(0, nv, j + 1), id(1, nv, j), id(1, nv, j + 1));
        s.f.push([a, d, b]);
        s.f.push([a, c, d]);
    }
    orient_outward(&mut s);
    s
}

/// The cup under the fruit's girdle: a short turned collar from the ground up to just under the girdle.
fn cup_solid(f: &SloeFrame, ground_drop: f64, top_r: f64, foot_r: f64) -> csg::Solid {
    let around = 72;
    let steps = 8;
    let ex = f.to_stalk;
    let ey = cross(f.normal, ex);
    let mut path = Vec::new();
    for k in 0..=steps {
        path.push(sub(f.girdle, mul(f.normal, 0.04 + (ground_drop + 0.4) * (1.0 - k as f64 / steps as f64))));
    }
    let _ = (ex, ey);
    tube(&path, around, |i, _| lerp(foot_r, top_r, smooth01(i as f64 / steps as f64)), false)
}

/// Turn a closed solid's faces outward if they face in (by signed volume).
fn orient_outward(s: &mut csg::Solid) {
    let vol: f64 = s.f.iter().map(|f| dot(s.v[f[0] as usize], cross(s.v[f[1] as usize], s.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
}

fn stored_op(s: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    let mesh = stored::Packed::encode(&s.v, &s.f, &vec![0; s.f.len()], &[SurfaceKind::Freeform])?;
    Ok(Operation::Stored { recipe: stored::Recipe { kernel: "vepres_prunus".into(), op: op.into(), params, digest: String::new() }, sources: Vec::new(), mesh })
}

fn cast_part(blend: f64) -> Component {
    Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::Free, blend_mm: blend, ..Component::default() }
}

/// What the renders tint: wood round a centreline point, a blossom's disc, the calyx round the fruit.
#[derive(Clone, Copy)]
enum Mark {
    Wood(P3, f64),
    Blossom(P3, P3, f64),
    Calyx(P3, P3, f64),
}

#[derive(Default, serde::Serialize)]
struct Authored {
    #[serde(skip)]
    marks: Vec<Mark>,
    option: String,
    base: String,
    twigs: Vec<serde_json::Value>,
    spurs: Vec<serde_json::Value>,
    blossoms: Vec<serde_json::Value>,
    sloe: serde_json::Value,
}

/// The twig's ground stations for a layout.
fn twig_stations(o: &Opt, g: &Ground) -> Vec<(String, Vec<(f64, f64)>, [bool; 2])> {
    let x_of = |th: f64| g.top / th.to_radians().tan();
    match o.layout {
        Layout::Diagonal => {
            // A line over the table clear of the sloe by its radius, the twig's and a hair's gap, running from the
            // left shoulder over the head to the right, easing back to the band's middle down each shoulder.
            let slope = 0.55_f64;
            let clear = 0.5 * o.sloe_mm + 1.15 + 1.3;
            let c = o.sloe_xz[1] - slope * o.sloe_xz[0] + clear * (1.0 + slope * slope).sqrt();
            let mut st = Vec::new();
            let mut th = 152.0;
            while th >= 28.0 {
                let x = x_of(th).clamp(-6.5, 6.5);
                // Bowed away round the fruit, so the twig curves about it.
                let bow = 0.6 * (-((x - o.sloe_xz[0]) / 3.0).powi(2)).exp();
                let line = (slope * x + c + bow).clamp(-3.7, 3.7);
                let off = smooth01(((th - 90.0).abs() - 24.0) / 26.0);
                st.push((th, lerp(line, 0.25 * (th - 90.0).signum() * -1.0, off)));
                th -= 0.5;
            }
            vec![("Twig over the head".into(), st, [true, true])]
        }
        Layout::Shoulders => {
            // Each shoulder's twig climbs from the shank to the face; the right one runs on over the rim to the
            // sloe's stalk, the left one dives into the face's rim beside the fruit.
            let mut right = Vec::new();
            let mut th: f64 = 18.0;
            while th <= 75.0 {
                let wav = 0.45 * ((th - 18.0) / 9.0).sin();
                let x = x_of(th);
                let onto = smooth01((th - 62.0) / 10.0);
                right.push((th, lerp(wav, -2.3 + 0.0 * x, onto)));
                th += 0.5;
            }
            let mut left = Vec::new();
            let mut th: f64 = 162.0;
            while th >= 111.0 {
                let wav = -0.4 * ((162.0 - th) / 8.0).sin();
                let onto = smooth01((116.0 - th) / 6.0);
                left.push((th, lerp(wav, 1.6, onto)));
                th -= 0.5;
            }
            vec![("Twig, right shoulder".into(), right, [true, false]), ("Twig, left shoulder".into(), left, [true, true])]
        }
    }
}

fn author(o: &Opt) -> Result<(RingDesign, AlphaLibrary, Authored)> {
    let mut d = probe::stock(o.base, false, None)?;
    d.name = "Prunus".into();
    probe::cast_in(&mut d, &probe::wax_setup(0.1));
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    {
        let setup = d.manufacturing.as_mut().unwrap();
        setup.recipe.alloy = "Gold 18k".into();
        setup.recipe.min_section_mm = MIN_SECTION_MM;
        setup.bench_notes = "Lost wax, 0.8 mm section. Twig, spurs, blossom, stalk and calyx cast in one with the band. At the bench: set the onyx sloe and close its calyx or collet over it.".into();
    }
    let lib = AlphaLibrary::builtin();
    let bare = mesh::try_build(&d, &lib, draft_params())?;
    let (_, hi) = bare.mesh.bounds().unwrap();
    let g = Ground { band: bare.band.clone().unwrap(), top: hi.1 as f64 };
    let mut info = Authored { option: o.slug.into(), base: o.base.into(), ..Default::default() };

    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Band".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let mut id = 2;

    // The sloe: a round onyx cabochon, its girdle a little over the table, off the face's centre.
    let theta = g.top.atan2(o.sloe_xz[0]).to_degrees();
    let (gp, _) = g.at(theta, o.sloe_xz[1]).ok_or_else(|| anyhow::anyhow!("no table under the sloe"))?;
    let mut sloe = Gem::cabochon(GemCut::Round, o.sloe_mm);
    sloe.preview_tint = Some(SLOE_TINT);
    let stand = o.sloe_rise_mm;
    let stone_id = id;
    doc.append(builders::stone_feature(id, sloe, Placement::Ring { theta_deg: theta, across_mm: o.sloe_xz[1], height_mm: stand, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0, level: false }))?;
    id += 1;
    // Read the girdle where the build seats the stone, from the band and the stone alone.
    let (girdle, normal) = {
        let mut probe_d = d.clone();
        probe_d.cad = Some(doc.clone());
        let b = mesh::try_build(&probe_d, &lib, draft_params())?;
        let (_, f) = ringdesign_core::stones::all_stone_frames_built(&probe_d, &b).into_iter().next().ok_or_else(|| anyhow::anyhow!("the sloe did not seat"))?;
        let _ = gp;
        (f.girdle, unit(f.normal))
    };
    let mut twigs: Vec<Twig> = Vec::new();
    for (k, (name, st, dive)) in twig_stations(o, &g).into_iter().enumerate() {
        let t = twig(&g, &name, &st, 1.15, 0.72, 0.5, dive, 2.3, 40 + k as i64)?;
        twigs.push(t);
    }
    // The stalk leaves the twig at the station nearest the fruit and runs to the fruit's shoulder.
    let (ti, si) = twigs
        .iter()
        .enumerate()
        .flat_map(|(ti, t)| t.path.iter().enumerate().map(move |(si, p)| (ti, si, len(sub(*p, girdle)))))
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(a, b, _)| (a, b))
        .unwrap();
    let stalk_from = twigs[ti].path[si];
    let to_stalk = {
        let v = sub(stalk_from, girdle);
        unit(sub(v, mul(normal, dot(v, normal))))
    };
    let frame = SloeFrame { girdle, normal, to_stalk, radius: 0.5 * o.sloe_mm, crown: sloe.crown_mm() };
    info.sloe = json!({"diameter_mm": o.sloe_mm, "centre_xz": o.sloe_xz, "girdle": girdle, "rise_mm": o.sloe_rise_mm, "crown_mm": frame.crown, "hold": o.hold});

    match o.hold {
        Hold::Claws => {
            doc.append(builders::feature_on(id, "Sloe claws", builders::CLAW, stone_id, json!({"prongs": 4, "wire_mm": 0.8})))?;
            id += 1;
            // A small calyx where the stalk meets the fruit: three short sepals over the collet's stalk side.
            for (k, off) in [-34.0, 0.0, 34.0].into_iter().enumerate() {
                let s = sepal_solid(&frame, off, 16.0, 0.55);
                doc.append(Feature { id, name: format!("Calyx sepal {}", k + 1), enabled: true, operation: stored_op(&s, "sepal", json!({"at_deg": off}))?, component: cast_part(0.0) })?;
                id += 1;
            }
        }
        Hold::Calyx => {
            let cup = cup_solid(&frame, o.sloe_rise_mm, frame.radius - 0.35, frame.radius * 0.55);
            doc.append(Feature { id, name: "Calyx cup".into(), enabled: true, operation: stored_op(&cup, "calyx_cup", json!({}))?, component: cast_part(0.0) })?;
            id += 1;
            for k in 0..5 {
                let at = 36.0 + 72.0 * k as f64 + 9.0 * (skin::hash(k, 5) - 0.5);
                let s = sepal_solid(&frame, at, 21.0, 0.36 + 0.06 * skin::hash(k, 9));
                doc.append(Feature { id, name: format!("Calyx sepal {}", k + 1), enabled: true, operation: stored_op(&s, "sepal", json!({"at_deg": at}))?, component: cast_part(0.0) })?;
                id += 1;
            }
        }
    }
    // The stalk: from the twig, arching up and over to the fruit's shoulder under its calyx.
    {
        let end = match o.hold {
            Hold::Claws => add(girdle, add(mul(to_stalk, frame.radius + 0.25), mul(normal, -0.2))),
            Hold::Calyx => add(girdle, add(mul(to_stalk, frame.radius * 0.55), mul(normal, -0.55 - o.sloe_rise_mm * 0.5))),
        };
        let mid = add(mul(add(stalk_from, end), 0.5), mul(normal, if o.hold == Hold::Claws { 0.55 } else { -0.3 }));
        let mut pts = Vec::new();
        for k in 0..=40 {
            let t = k as f64 / 40.0;
            let a = add(mul(stalk_from, (1.0 - t) * (1.0 - t)), add(mul(mid, 2.0 * t * (1.0 - t)), mul(end, t * t)));
            pts.push(a);
        }
        let path = resample(&pts, 0.08);
        let n = path.len();
        let s = tube(&path, 24, |i, _| lerp(0.62, 0.46, i as f64 / (n - 1) as f64), true);
        info.marks.extend(path.iter().map(|p| Mark::Wood(*p, 0.75)));
        doc.append(Feature { id, name: "Sloe stalk".into(), enabled: true, operation: stored_op(&s, "stalk", json!({}))?, component: cast_part(0.0) })?;
        id += 1;
    }
    // The twigs, their spurs and their blossom.
    let mut blossom_at: Vec<(P3, f64)> = Vec::new();
    let keep_clear = |p: P3, r: f64| len(sub(p, girdle)) > frame.radius + r + 0.35;
    for (ti, t) in twigs.iter().enumerate() {
        let seed = 40 + ti as i64;
        doc.append(Feature { id, name: t.name.clone(), enabled: true, operation: stored_op(&twig_solid(t, seed), "twig", json!({"stations": t.path.len()}))?, component: cast_part(0.0) })?;
        id += 1;
        info.marks.extend(t.path.iter().zip(&t.radius).map(|(p, r)| Mark::Wood(*p, *r + 0.12)));
        info.twigs.push(json!({"name": t.name, "length_mm": 0.1 * (t.path.len() - 1) as f64, "nodes": t.nodes.len(), "radius_mm": [t.radius.iter().copied().fold(f64::MAX, f64::min), t.radius.iter().copied().fold(0.0, f64::max)]}));
        // Spurs at the nodes, turned every way round the twig and leaning toward one end or the other; every third
        // node carries a blossom instead.
        let total = 0.1 * (t.path.len() - 1) as f64;
        let mut spur_no = 0;
        for (k, &node) in t.nodes.iter().enumerate() {
            let (tg, up, side) = frame_at(t, node);
            let c = t.path[node];
            let r = t.radius[node];
            let on_face = c[1] > g.top - 0.2;
            if k % 2 == 0 {
                // Spurs leave the wood at an acute angle, pointing toward the nearer tip of the twig, turned out to
                // one side then the other, long and short in turn.
                let long = spur_no % 2 == 0;
                spur_no += 1;
                let flank = if (k / 2) % 2 == 0 { 1.0 } else { -1.0 };
                let mut phi: f64 = flank * if on_face { 58.0 + 22.0 * skin::hash(k as i64, seed + 8) } else { 30.0 + 15.0 * skin::hash(k as i64, seed + 8) };
                let lean = 48.0 + 12.0 * skin::hash(k as i64, seed + 2);
                let toward = if 0.1 * node as f64 > 0.5 * total { 1.0 } else { -1.0 };
                let length = match (on_face, long) {
                    (true, true) => 3.9 + 0.8 * skin::hash(k as i64, seed + 4),
                    (true, false) => 2.0 + 0.5 * skin::hash(k as i64, seed + 4),
                    (false, true) => 3.0 + 0.6 * skin::hash(k as i64, seed + 4),
                    (false, false) => 1.8 + 0.4 * skin::hash(k as i64, seed + 4),
                };
                let mut placed_ok = None;
                for _ in 0..4 {
                    let out = add(mul(up, phi.to_radians().cos()), mul(side, phi.to_radians().sin()));
                    let dir = unit(add(mul(out, lean.to_radians().cos()), mul(tg, toward * lean.to_radians().sin())));
                    let tip = add(c, mul(dir, r + length));
                    let clear = keep_clear(tip, 0.3) && keep_clear(add(c, mul(dir, r + 0.5 * length)), 0.4);
                    let above = g.under(tip).is_none_or(|(gp, gn)| dot(sub(tip, gp), gn) > 0.3);
                    if clear && above {
                        placed_ok = Some((dir, tip));
                        break;
                    }
                    phi *= 0.6;
                }
                let Some((dir, tip)) = placed_ok else { continue };
                let bend = mul(tg, toward);
                let s = spur_solid(c, dir, bend, r, length, 0.6, seed * 100 + k as i64);
                doc.append(Feature { id, name: format!("Spur {} on the {}", k + 1, t.name.to_lowercase()), enabled: true, operation: stored_op(&s, "spur", json!({"length_mm": length, "turn_deg": phi, "lean_deg": lean}))?, component: cast_part(0.0) })?;
                id += 1;
                let steps = 30;
                for j in 0..=steps {
                    let q = (r + length) * j as f64 / steps as f64;
                    let past = (q - r).max(0.0);
                    info.marks.push(Mark::Wood(add(c, add(mul(dir, q), mul(bend, SPUR_BEND * past * past))), 1.05));
                }
                info.spurs.push(json!({"twig": t.name, "node": k, "length_mm": length, "turn_deg": phi, "lean_deg": lean, "tip": tip, "point_r_mm": SPUR_POINT_R}));
            } else {
                // A pair of blossoms on the wood at the node, one each side, the second a little smaller and further on.
                let dia = if on_face { 3.5 } else { 3.0 };
                let flank = if (k / 2) % 2 == 0 { -1.0 } else { 1.0 };
                for (j, (scale, shift, f)) in [(1.0, 0.0, flank), (0.82, 0.62, -flank)].into_iter().enumerate() {
                    let dia = dia * scale;
                    let node2 = ((node as f64 + shift * dia / 0.1).round() as usize).min(t.path.len() - 1);
                    let (tg2, up2, side2) = frame_at(t, node2);
                    let nrm = unit(add(up2, mul(side2, f * 0.45)));
                    let p = add(t.path[node2], mul(nrm, t.radius[node2] * 0.7));
                    if !keep_clear(p, 0.5 * dia) || blossom_at.iter().any(|(q, rr)| len(sub(*q, p)) < rr + 0.5 * dia - 0.1) {
                        continue;
                    }
                    let s = placed(blossom_solid(dia, 0.55, 0.3 * k as f64 + j as f64), p, tg2, nrm);
                    blossom_at.push((p, 0.5 * dia));
                    info.marks.push(Mark::Blossom(p, nrm, 0.5 * dia));
                    doc.append(Feature { id, name: format!("Blossom {}{} on the {}", k + 1, ["a", "b"][j], t.name.to_lowercase()), enabled: true, operation: stored_op(&s, "blossom", json!({"diameter_mm": dia}))?, component: cast_part(0.0) })?;
                    id += 1;
                    info.blossoms.push(json!({"twig": t.name, "node": k, "diameter_mm": dia, "at": p}));
                }
            }
        }
    }
    info.marks.push(Mark::Calyx(girdle, normal, frame.radius + 1.05));
    d.cad = Some(doc);
    Ok((d, lib, info))
}

/// Every made part's self-crossings.
fn crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| {
            let n = match &c.made {
                Some(m) => csg::self_crossings(m.solid()),
                None => csg::self_crossings(&csg::Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }),
            };
            (c.name.clone(), n)
        })
        .collect()
}

fn mesh_crossings(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() })
}

/// Metal vertices inside each stone's crown.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize, f64)> {
    const TOLERANCE_MM: f64 = 0.05;
    let m = &built.mesh;
    ringdesign_core::stones::all_stone_frames_built(d, built)
        .into_iter()
        .map(|(st, f)| {
            let (a, b, h) = (st.gem.l_mm * 0.5, st.gem.w_mm * 0.5, st.gem.crown_mm());
            let faceted = st.gem.form == GemForm::Faceted;
            let (mut n, mut worst) = (0, 0.0_f64);
            for p in &m.vertices {
                let q = sub([p.0 as f64, p.1 as f64, p.2 as f64], f.girdle);
                let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                if !(z > TOLERANCE_MM && z < h - TOLERANCE_MM) {
                    continue;
                }
                let t = z / h;
                let k = if faceted { 1.0 - 0.45 * t } else { (1.0 - t * t).sqrt() };
                let r = ((x / a).powi(2) + (y / b).powi(2)).sqrt();
                let depth = (k - r) * a.min(b);
                if depth > TOLERANCE_MM {
                    n += 1;
                    worst = worst.max(depth);
                }
            }
            (st.label, n, worst)
        })
        .collect()
}

/// The metal's section, sampled: from vertices of the finished mesh, a ray straight in along the inward normal to the
/// first surface it meets. Returns the least found, where, how many samples fall under the floor, and how many ran.
fn section_samples(m: &mesh::Mesh, floor: f64, stride: usize) -> (f64, P3, usize, usize) {
    const CELL: f64 = 0.3;
    const REACH: f64 = 1.2;
    let tri: Vec<[P3; 3]> = m.faces.iter().map(|f| f.map(|i| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] })).collect();
    let key = |p: P3| -> (i32, i32, i32) { ((p[0] / CELL).floor() as i32, (p[1] / CELL).floor() as i32, (p[2] / CELL).floor() as i32) };
    let mut grid: std::collections::HashMap<(i32, i32, i32), Vec<u32>> = std::collections::HashMap::new();
    for (k, t) in tri.iter().enumerate() {
        let lo = key([t[0][0].min(t[1][0]).min(t[2][0]), t[0][1].min(t[1][1]).min(t[2][1]), t[0][2].min(t[1][2]).min(t[2][2])]);
        let hi = key([t[0][0].max(t[1][0]).max(t[2][0]), t[0][1].max(t[1][1]).max(t[2][1]), t[0][2].max(t[1][2]).max(t[2][2])]);
        for i in lo.0..=hi.0 {
            for j in lo.1..=hi.1 {
                for l in lo.2..=hi.2 {
                    grid.entry((i, j, l)).or_default().push(k as u32);
                }
            }
        }
    }
    let mut incident: Vec<Vec<u32>> = vec![Vec::new(); m.vertices.len()];
    for (k, f) in m.faces.iter().enumerate() {
        for &i in f {
            incident[i as usize].push(k as u32);
        }
    }
    let (mut least, mut at, mut under, mut runs) = (f64::MAX, [0.0; 3], 0, 0);
    for (vi, v) in m.vertices.iter().enumerate().step_by(stride.max(1)) {
        let Some(nn) = m.normals.get(vi) else { continue };
        let o = [v.0 as f64, v.1 as f64, v.2 as f64];
        let dir = unit([-(nn.0 as f64), -(nn.1 as f64), -(nn.2 as f64)]);
        runs += 1;
        let mut cells = std::collections::HashSet::new();
        let mut t = 0.0;
        while t <= REACH {
            let c = key(add(o, mul(dir, t)));
            for di in -1..=1 {
                for dj in -1..=1 {
                    for dl in -1..=1 {
                        cells.insert((c.0 + di, c.1 + dj, c.2 + dl));
                    }
                }
            }
            t += 0.25;
        }
        let mut hit = f64::MAX;
        for c in cells {
            let Some(list) = grid.get(&c) else { continue };
            for &k in list {
                if incident[vi].contains(&k) {
                    continue;
                }
                let [a, b, cc] = tri[k as usize];
                let (e1, e2) = (sub(b, a), sub(cc, a));
                let pv = cross(dir, e2);
                let det = dot(e1, pv);
                if det.abs() < 1e-12 {
                    continue;
                }
                let tv = sub(o, a);
                let u = dot(tv, pv) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let qv = cross(tv, e1);
                let w = dot(dir, qv) / det;
                if w < 0.0 || u + w > 1.0 {
                    continue;
                }
                let d = dot(e2, qv) / det;
                if d > 0.01 && d < hit {
                    hit = d;
                }
            }
        }
        if hit < REACH {
            if hit < floor {
                under += 1;
            }
            if hit < least {
                least = hit;
                at = o;
            }
        }
    }
    (least, at, under, runs)
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams, built: &mesh::BuildResult) -> Result<serde_json::Value> {
    let v = &built.report.validation;
    let q = built.report.quality;
    let made = crossings(built);
    let ring_crossings = mesh_crossings(&built.mesh);
    let bore = d.inner_radius_mm();
    let min_r = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, built);
    let statuses: Vec<(String, String)> = built.parts.evaluated.iter().flat_map(|e| e.features.iter()).map(|f| (f.name.clone(), format!("{:?}", f.status))).collect();
    let features_ok = built.parts.evaluated.iter().flat_map(|e| e.features.iter()).all(|f| f.status.is_ok());
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let previewed = ringdesign_core::gems::built_meshes(d, lib, built).len();
    let in_stones = metal_in_stones(d, built);
    let crowding: Vec<String> = stones.as_ref().map_or(Vec::new(), |s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} mm", p.a, p.b, p.gap_mm)).collect());
    let tight = stones.as_ref().map_or(0, |s| s.tight_pairs);
    let stride = (built.mesh.vertices.len() / 400_000).max(1);
    let (least, least_at, under, runs) = section_samples(&built.mesh, MIN_SECTION_MM, stride);
    let pass = v.watertight
        && q.degenerate_faces == 0
        && ring_crossings == 0
        && made.iter().all(|(_, n)| *n == 0)
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && features_ok
        && min_r >= bore - 0.01
        && field.verdict == castability::Verdict::Castable
        && field.thinnest_wall_mm >= MIN_SECTION_MM
        && under == 0
        && findings.is_empty()
        && stones.as_ref().map_or(0, |s| s.stone_count) as usize == previewed
        && in_stones.iter().all(|(_, n, _)| *n == 0)
        && tight == 0
        && built.mesh.faces.len() <= 2_000_000;
    Ok(json!({
        "build": [params.theta_steps, params.profile_steps],
        "triangles": built.mesh.faces.len(),
        "watertight": v.watertight,
        "boundary_edges": v.boundary_edges,
        "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces,
        "ring_self_crossings": ring_crossings,
        "made_part_crossings_worst": made.iter().map(|(_, n)| *n).max().unwrap_or(0),
        "made_parts": made.len(),
        "made_parts_crossing": made.iter().filter(|(_, n)| *n > 0).collect::<Vec<_>>(),
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "features_all_ok": features_ok,
        "features_not_ok": statuses.iter().filter(|(_, s)| s != "Ok").collect::<Vec<_>>(),
        "bore_radius_mm": bore,
        "closest_vertex_to_axis_mm": min_r,
        "bore_clear": min_r >= bore - 0.01,
        "process": d.draft.process.label(),
        "min_section_mm": d.draft.min_section_mm,
        "field_verdict": field.verdict.label(),
        "field_undercut_percent": field.undercut_fraction() * 100.0,
        "field_notes": field.notes,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "section_sampled": {"method": "inward rays from every vertex (stride shown) of the finished mesh to the first surface", "stride": stride, "samples": runs, "least_mm": least, "at": least_at, "under_floor": under},
        "dfm_findings": findings,
        "stones_reported": stones.as_ref().map_or(0, |s| s.stone_count),
        "stones_previewed": previewed,
        "metal_inside_stones": in_stones,
        "crowding": crowding,
        "tight_pairs": tight,
        "pass": pass,
    }))
}

fn side_by_side(path: &Path, imgs: &[Vec<u8>], edge: usize) -> Result<()> {
    let n = imgs.len();
    let mut out = vec![0u8; edge * n * edge * 3];
    for y in 0..edge {
        for (k, img) in imgs.iter().enumerate() {
            let row = (y * n * edge + k * edge) * 3;
            out[row..row + edge * 3].copy_from_slice(&img[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, (edge * n) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, PI * 0.5),
    ("side", 0.0, 0.0),
    ("shoulder", 1.25, 1.05),
    ("reverse", PI, 0.8),
];

/// What finish each vertex takes: 0 gold (antiqued by depth), 1 the twig's wood, 2 polished blossom, 3 the calyx.
fn finish_classes(m: &mesh::Mesh, marks: &[Mark]) -> Vec<u8> {
    const CELL: f64 = 0.6;
    let key = |p: P3| ((p[0] / CELL).floor() as i32, (p[1] / CELL).floor() as i32, (p[2] / CELL).floor() as i32);
    let mut grid: std::collections::HashMap<(i32, i32, i32), Vec<(P3, f64)>> = std::collections::HashMap::new();
    for mk in marks {
        if let Mark::Wood(c, r) = mk {
            let k = key(*c);
            let reach = (r / CELL).ceil() as i32 + 1;
            for i in -reach..=reach {
                for j in -reach..=reach {
                    for l in -reach..=reach {
                        grid.entry((k.0 + i, k.1 + j, k.2 + l)).or_default().push((*c, *r));
                    }
                }
            }
        }
    }
    m.vertices
        .iter()
        .map(|v| {
            let p = [v.0 as f64, v.1 as f64, v.2 as f64];
            for mk in marks {
                if let Mark::Blossom(c, n, r) = mk {
                    let q = sub(p, *c);
                    let h = dot(q, *n);
                    if h > -0.12 && len(sub(q, mul(*n, h))) < r + 0.06 {
                        return 2;
                    }
                }
            }
            if grid.get(&key(p)).is_some_and(|l| l.iter().any(|(c, r)| len(sub(p, *c)) < *r)) {
                return 1;
            }
            for mk in marks {
                if let Mark::Calyx(c, n, r) = mk {
                    let q = sub(p, *c);
                    let h = dot(q, *n);
                    if h > -0.75 && len(sub(q, mul(*n, h))) < *r {
                        return 3;
                    }
                }
            }
            0
        })
        .collect()
}

/// The metal split for the studio: gold darkened in its recesses, the wood oxidised and darker in its fissures, the
/// blossom polished, the calyx half dark. Depth is how far under a relaxed copy of the surface a vertex lies.
fn antiqued(m: &mesh::Mesh, class: &[u8]) -> Vec<(mesh::Mesh, [f32; 3])> {
    let n = m.vertices.len();
    let mut nb: Vec<Vec<u32>> = vec![Vec::new(); n];
    for f in &m.faces {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            nb[a as usize].push(b);
            nb[b as usize].push(a);
        }
    }
    nb.iter_mut().for_each(|l| {
        l.sort_unstable();
        l.dedup();
    });
    let start: Vec<P3> = m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();
    let mut p = start.clone();
    for _ in 0..ANTIQUE_PASSES {
        p = (0..n)
            .map(|i| {
                let l = &nb[i];
                if l.is_empty() {
                    return p[i];
                }
                let mean: P3 = std::array::from_fn(|c| l.iter().map(|&j| p[j as usize][c]).sum::<f64>() / l.len() as f64);
                std::array::from_fn(|c| 0.5 * p[i][c] + 0.5 * mean[c])
            })
            .collect();
    }
    let depth: Vec<f64> = (0..n).map(|i| { let v = m.normals.get(i).map_or([0.0; 3], |v| [v.0 as f64, v.1 as f64, v.2 as f64]); dot(sub(p[i], start[i]), v) }).collect();
    let tints: [[f32; 3]; 7] = [render::GOLD, ANTIQUE_MID, ANTIQUE_DARK, WOOD_TINT, WOOD_DEEP_TINT, render::GOLD, CALYX_TINT];
    let mut out: Vec<(mesh::Mesh, [f32; 3])> = tints.iter().map(|t| (mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..Default::default() }, *t)).collect();
    for f in &m.faces {
        let h = f.iter().map(|&i| depth[i as usize]).sum::<f64>() / 3.0;
        let cs = f.map(|i| class.get(i as usize).copied().unwrap_or(0));
        // A face takes a finish only when all its corners do, so a coarse face of the band beside a part stays gold.
        let c = if cs[0] == cs[1] && cs[1] == cs[2] { cs[0] } else { cs.iter().copied().filter(|&c| c != 0).min().map_or(0, |c| if cs.contains(&0) { 0 } else { c }) };
        let bin = match c {
            0 => if h > ANTIQUE_DEEP_MM { 2 } else if h > ANTIQUE_SHALLOW_MM { 1 } else { 0 },
            1 => if h > 0.02 { 4 } else { 3 },
            2 => if h > 0.05 { 1 } else { 5 },
            _ => 6,
        };
        out[bin].0.faces.push(*f);
    }
    out.retain(|(m, _)| !m.faces.is_empty());
    out
}

fn welded(m: &mesh::Mesh) -> mesh::Mesh {
    let mut out = mesh::Mesh::default();
    let mut index = std::collections::HashMap::new();
    for f in &m.faces {
        let g = f.map(|i| {
            let v = m.vertices[i as usize];
            let key = [v.0, v.1, v.2].map(|c| (c * 1e4).round() as i64);
            *index.entry(key).or_insert_with(|| {
                out.vertices.push(v);
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    let mut normals = vec![[0.0_f64; 3]; out.vertices.len()];
    for f in &out.faces {
        let [a, b, c] = f.map(|i| { let v = out.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] });
        let nrm = cross(sub(b, a), sub(c, a));
        for &i in f {
            normals[i as usize] = add(normals[i as usize], nrm);
        }
    }
    out.normals = normals.into_iter().map(|v| { let u = unit(v); mesh::Vec3(u[0] as f32, u[1] as f32, u[2] as f32) }).collect();
    out
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, marks: &[Mark], edge: usize) -> Result<()> {
    let stones: Vec<(mesh::Mesh, [f32; 3])> = ringdesign_core::gems::built_meshes(d, lib, built).into_iter().map(|(m, t)| (welded(&m), t)).collect();
    let finish = antiqued(&built.mesh, &finish_classes(&built.mesh, marks));
    let stone = |m, t: [f32; 3]| {
        let mut p = render::Part::tinted_stone(m, t);
        // The sloe is a fruit under its bloom: smooth and dull, never a polished gem.
        p.smooth = true;
        p.roughness = 1.0;
        p.gem = false;
        p
    };
    let mut parts: Vec<render::Part> = Vec::new();
    parts.extend(finish.iter().map(|(m, t)| render::Part::metal(m, *t)));
    parts.extend(stones.iter().map(|(m, t)| stone(m, *t)));
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let small: Vec<Vec<u8>> = [VIEWS[0], VIEWS[1], VIEWS[3], VIEWS[2]].iter().map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 4)).collect();
    image::save_buffer(out.join("hero-300.png"), &small[0], 300, 300, image::ColorType::Rgb8)?;
    image::save_buffer(out.join("face-300.png"), &small[1], 300, 300, image::ColorType::Rgb8)?;
    side_by_side(&out.join("contact-300.png"), &small, 300)?;
    // The head close, framed on whole parts.
    let (_, hi) = built.mesh.bounds().unwrap();
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(90.0) + 0.35, 0.75, render::Framing::new([0.0, hi.1 as f64 - 1.5, 0.0], 7.5), edge)?;
    let mut bare = d.clone();
    bare.cad = None;
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let left = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.48, 1.0, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.48, 1.0, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &[left, right], edge)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let pick = args.iter().position(|a| a == "--option").and_then(|i| args.get(i + 1)).cloned();
    let chosen = pick.as_deref().unwrap_or("wax-twig");
    let Some(o) = OPTIONS.iter().find(|o| o.slug == chosen) else { bail!("no option {chosen}") };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/prunus");
    let positional: Vec<&String> = args.iter().enumerate().filter(|(i, a)| !a.starts_with("--") && !(*i > 0 && args[*i - 1] == "--option")).map(|(_, a)| a).collect();
    let out = positional.first().map(PathBuf::from).unwrap_or_else(|| if pick.is_some() { root.join(o.slug) } else { root.clone() });
    std::fs::create_dir_all(&out)?;
    let (mut d, lib, info) = author(o)?;
    if args.iter().any(|a| a == "--bare") {
        d.cad = None;
    }
    let params = if draft { draft_params() } else { export_params() };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    println!("Prunus {}: {} triangles in {build_s:.1} s", o.slug, built.mesh.faces.len());
    for (st, f) in ringdesign_core::stones::all_stone_frames_built(&d, &built) {
        println!("  stone {}: girdle {:?} normal {:?}", st.label, f.girdle, f.normal);
    }
    let g = gates(&d, &lib, params, &built)?;
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = ringdesign_core::manufacturing::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let mut pattern_gate = serde_json::Value::Null;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        let pq = pattern.report.quality;
        let pc = mesh_crossings(&pattern.mesh);
        pattern_gate = json!({"watertight": pattern.report.validation.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "triangles": pattern.mesh.faces.len(), "pass": pattern.report.validation.watertight && pq.degenerate_faces == 0 && pc == 0});
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        let stones = ringdesign_core::gems::built_meshes(&d, &lib, &built);
        let mut materials = Vec::new();
        for (m, tint) in stones.iter() {
            stl::write_stl(out.join("reference-onyx.stl"), m, "Prunus reference stone")?;
            materials.push(json!({"mesh": "reference-onyx.stl", "name": "Onyx", "tint": tint}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": materials}))?)?;
    }
    renders(&out, &d, &lib, &built, &info.marks, if draft { 1000 } else { 1600 })?;
    let mut report = json!({
        "ring": "Prunus",
        "slug": "prunus",
        "option": o,
        "process": d.draft.process.label(),
        "min_section_mm": d.draft.min_section_mm,
        "min_detail_mm": d.draft.min_detail_mm,
        "min_draft_deg": d.draft.min_draft_deg,
        "base": format!("Factory {} native, lost wax", o.base),
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build_s": build_s,
        "design_bytes": text.len(),
        "design_format": format,
        "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len()),
        "authored": serde_json::to_value(&info)?,
        "cold_reload_identical": cold,
        "casting_pattern": pattern_gate,
    });
    let key = if draft { "draft" } else { "export" };
    report[key] = g.clone();
    if let Ok(old) = std::fs::read_to_string(out.join("report.json")) {
        if let Ok(old) = serde_json::from_str::<serde_json::Value>(&old) {
            for other in ["draft", "export", "cold_reload_identical", "casting_pattern", "template_gate"] {
                if report.get(other).is_none_or(|v| v.is_null()) {
                    if let Some(v) = old.get(other) {
                        report[other] = v.clone();
                    }
                }
            }
        }
    }
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("  {key}: pass {}", g["pass"]);
    if !args.iter().any(|a| a == "--blockout") {
        ensure!(g["pass"].as_bool() == Some(true), "Prunus failed a gate at {key}; see report.json");
    }
    ensure!(cold != Some(false), "Prunus changed on a cold reload with an empty library");
    Ok(())
}
