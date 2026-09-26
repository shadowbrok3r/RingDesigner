//! Fenrir: the wolf and the moon, on native factory 010 trillion stock in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_fenrir
//! target/release/examples/bestiarium_fenrir [OUT_DIR] [--draft] [--verify] [--sculpt]
use anyhow::{Result, ensure};
use rayon::prelude::*;
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, Mesh, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, Document, Feature, Operation, Placement, Stage, builders},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Blend, Decal, DecalLayer, Layer, LayerEntry, Window},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf,
    mesh::{self, Vec3},
    render::{self, Part},
    skin::{self, Atlas, Hide, Sample},
    stl,
    svg::SvgAlpha,
    tiling::TilingLayer,
};
use serde_json::{Value, json};
use std::{f64::consts::PI, path::Path, time::Instant};

type P3 = [f64; 3];

/// Moon centre across the face, mm toward the ears from the head's mid-plane.
const MOON_U: f64 = -4.8;
/// Moonstone diameter, mm.
const MOON_MM: f64 = 10.0;

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
fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn lerp3(a: P3, b: P3, t: f64) -> P3 {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}
fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Polynomial smooth minimum of radius `k`.
fn smin(a: f64, b: f64, k: f64) -> f64 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}
fn smax(a: f64, b: f64, k: f64) -> f64 {
    -smin(-a, -b, k)
}
/// Approximate distance to an axis-aligned ellipsoid of semi-axes `r` at the origin.
fn ellipsoid(p: P3, r: P3) -> f64 {
    let k0 = len([p[0] / r[0], p[1] / r[1], p[2] / r[2]]);
    let k1 = len([p[0] / (r[0] * r[0]), p[1] / (r[1] * r[1]), p[2] / (r[2] * r[2])]);
    if k1 < 1e-12 { -r[0].min(r[1]).min(r[2]) } else { k0 * (k0 - 1.0) / k1 }
}
/// Exact distance to the rounded cone joining sphere `a` of radius `ra` to sphere `b` of radius `rb`.
fn round_cone(p: P3, a: P3, b: P3, ra: f64, rb: f64) -> f64 {
    let ba = sub(b, a);
    let l2 = dot(ba, ba).max(1e-12);
    let rr = ra - rb;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = sub(p, a);
    let y = dot(pa, ba);
    let z = y - l2;
    let x = sub(mul(pa, l2), mul(ba, y));
    let x2 = dot(x, x);
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - rb;
    }
    if y.signum() * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - ra;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - ra
}
/// `p` turned by `deg` in the plane of axes `i` and `j`.
fn turn(p: P3, i: usize, j: usize, deg: f64) -> P3 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut q = p;
    q[i] = c * p[i] - s * p[j];
    q[j] = s * p[i] + c * p[j];
    q
}

/// The stock's outer surface seen from over the face: height over the table per face point.
struct Relief {
    x0: f64,
    u0: f64,
    step: f64,
    n: usize,
    h: Vec<f64>,
}

impl Relief {
    const FLOOR: f64 = -12.0;
    fn of(a: &Atlas, table: f64) -> Self {
        let (x0, u0, step, n) = (-13.0, -13.0, 0.1, 261);
        let mut h = vec![Self::FLOOR; n * n];
        for s in &a.samples {
            if s.p[1] < 4.0 {
                continue;
            }
            let (i, j) = (((s.p[0] - x0) / step).round(), ((-s.p[2] - u0) / step).round());
            if i < 0.0 || j < 0.0 || i >= n as f64 || j >= n as f64 {
                continue;
            }
            let k = j as usize * n + i as usize;
            h[k] = h[k].max(s.p[1] - table);
        }
        // Holes between splatted samples take their neighbours' height.
        for _ in 0..3 {
            let prev = h.clone();
            for j in 1..n - 1 {
                for i in 1..n - 1 {
                    let k = j * n + i;
                    if prev[k] > Self::FLOOR {
                        continue;
                    }
                    let near = [prev[k - 1], prev[k + 1], prev[k - n], prev[k + n]];
                    let found: Vec<f64> = near.into_iter().filter(|v| *v > Self::FLOOR).collect();
                    if found.len() >= 2 {
                        h[k] = found.iter().sum::<f64>() / found.len() as f64;
                    }
                }
            }
        }
        Self { x0, u0, step, n, h }
    }
    /// Surface height over the table at face point (`x`, `u`), bilinear.
    fn at(&self, x: f64, u: f64) -> f64 {
        let fx = ((x - self.x0) / self.step).clamp(0.0, (self.n - 2) as f64);
        let fy = ((u - self.u0) / self.step).clamp(0.0, (self.n - 2) as f64);
        let (i, j) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let g = |a: usize, b: usize| self.h[b * self.n + a];
        lerp(lerp(g(i, j), g(i + 1, j), tx), lerp(g(i, j + 1), g(i + 1, j + 1), tx), ty)
    }
}

/// Signed distance to the bare stock's outer surface on a coarse grid, read trilinear.
struct Stock {
    lo: P3,
    step: f64,
    n: [usize; 3],
    g: Vec<f32>,
}

impl Stock {
    const REACH: f64 = 1.2;
    fn of(a: &Atlas, relief: &Relief, table: f64, lo: P3, hi: P3, step: f64) -> Self {
        use std::collections::HashMap;
        let cell = 0.4;
        let key = |p: P3| -> [i32; 3] { std::array::from_fn(|k| (p[k] / cell).floor() as i32) };
        let mut map: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        for (i, s) in a.samples.iter().enumerate() {
            if s.p[1] < 4.0 || dot(s.n, s.n) < 0.5 {
                continue;
            }
            map.entry(key(s.p)).or_default().push(i as u32);
        }
        let n: [usize; 3] = std::array::from_fn(|k| ((hi[k] - lo[k]) / step).ceil() as usize + 1);
        let reach = (Self::REACH / cell).ceil() as i32;
        let g: Vec<f32> = (0..n[0] * n[1] * n[2])
            .into_par_iter()
            .map(|m| {
                let (i, j, k) = (m % n[0], (m / n[0]) % n[1], m / (n[0] * n[1]));
                let p = [lo[0] + i as f64 * step, lo[1] + j as f64 * step, lo[2] + k as f64 * step];
                let c = key(p);
                let mut best = (f64::MAX, 0u32);
                for dx in -reach..=reach {
                    for dy in -reach..=reach {
                        for dz in -reach..=reach {
                            if let Some(list) = map.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                                for &s in list {
                                    let d2 = dot(sub(p, a.samples[s as usize].p), sub(p, a.samples[s as usize].p));
                                    if d2 < best.0 {
                                        best = (d2, s);
                                    }
                                }
                            }
                        }
                    }
                }
                if best.0.sqrt() <= Self::REACH {
                    let s = &a.samples[best.1 as usize];
                    (best.0.sqrt() * dot(sub(p, s.p), s.n).signum()) as f32
                } else {
                    let (x, u, h) = (p[0], -p[2], p[1] - table);
                    if h < relief.at(x, u) { -Self::REACH as f32 } else { Self::REACH as f32 }
                }
            })
            .collect();
        Self { lo, step, n, g }
    }
    fn at(&self, p: P3) -> f64 {
        let f: [f64; 3] = std::array::from_fn(|k| ((p[k] - self.lo[k]) / self.step).clamp(0.0, (self.n[k] - 2) as f64));
        let i: [usize; 3] = std::array::from_fn(|k| f[k].floor() as usize);
        let t: [f64; 3] = std::array::from_fn(|k| f[k] - i[k] as f64);
        let g = |a: usize, b: usize, c: usize| self.g[((i[2] + c) * self.n[1] + i[1] + b) * self.n[0] + i[0] + a] as f64;
        let x00 = lerp(g(0, 0, 0), g(1, 0, 0), t[0]);
        let x10 = lerp(g(0, 1, 0), g(1, 1, 0), t[0]);
        let x01 = lerp(g(0, 0, 1), g(1, 0, 1), t[0]);
        let x11 = lerp(g(0, 1, 1), g(1, 1, 1), t[0]);
        lerp(lerp(x00, x10, t[1]), lerp(x01, x11, t[1]), t[2])
    }
}

/// The head keeps this far outside the finger's cylinder, mm.
const BORE_CLEAR_MM: f64 = 0.35;
/// How deep the sculpt reaches under the stock's surface, mm.
const BURY_MM: f64 = 0.6;
/// No sculpt within this radius of the moon's axis over the table, so the stone drops past the gums.
const KEEP_OUT_MM: f64 = 5.03;
/// The gums round the moon stand this high over the table, over the fangs' base rail, mm.
const GUM_H: f64 = 1.35;
/// Half the angle each open mouth corner spans at the moon's side, degrees.
const CORNER_DEG: f64 = 16.0;
/// The fangs' bearing round the moon, degrees from +x.
const FANG_DEG: f64 = 73.4;
/// Where the facial fur flows toward, out past each ear: face `x` and `u`.
const FUR_SINK: (f64, f64) = (15.0, 10.0);

/// A point `rho` from the moon's axis, `deg` from +x toward +u, at height `h`.
fn rim(rho: f64, deg: f64, h: f64) -> P3 {
    let (s, c) = deg.to_radians().sin_cos();
    [rho * c, MOON_U + rho * s, h]
}

fn bell(x: f64, c: f64, w: f64) -> f64 {
    (-((x - c) / w).powi(2)).exp()
}

/// Distance in the section plane to an isosceles trapezoid `r1` half-wide at `y = -he`, `r2` at `y = he`.
fn trapezoid(p: [f64; 2], r1: f64, r2: f64, he: f64) -> f64 {
    let (x, y) = (p[0].abs(), p[1]);
    let k1 = [r2, he];
    let k2 = [r2 - r1, 2.0 * he];
    let ca = [x - x.min(if y < 0.0 { r1 } else { r2 }), y.abs() - he];
    let t = (((k1[0] - x) * k2[0] + (k1[1] - y) * k2[1]) / (k2[0] * k2[0] + k2[1] * k2[1])).clamp(0.0, 1.0);
    let cb = [x - k1[0] + k2[0] * t, y - k1[1] + k2[1] * t];
    let s = if cb[0] < 0.0 && ca[1] < 0.0 { -1.0 } else { 1.0 };
    s * (ca[0] * ca[0] + ca[1] * ca[1]).min(cb[0] * cb[0] + cb[1] * cb[1]).sqrt()
}

/// A tooth: a stout body from `root` to `knee`, then a short point to `tip`, with radii at the three.
struct Tooth {
    name: String,
    root: P3,
    knee: P3,
    tip: P3,
    r: [f64; 3],
}

impl Tooth {
    fn new(name: String, root: P3, tip: P3, r_root: f64) -> Self {
        let l = len(sub(tip, root));
        let knee = lerp3(tip, root, (0.52 / l.max(1e-9)).min(0.6));
        Self { name, root, knee, tip, r: [r_root, 0.45, 0.06] }
    }
    fn sdf(&self, q: P3) -> f64 {
        smin(round_cone(q, self.root, self.knee, self.r[0], self.r[1]), round_cone(q, self.knee, self.tip, self.r[1], self.r[2]), 0.1)
    }
}

/// Where the upper lip's inner edge stands from the moon's axis, and how high, by moon bearing in degrees: three
/// scallops over the incisors, lifted and drawn back over each fang, pulled well back over the premolars.
fn upper_lip(deg: f64) -> (f64, f64) {
    let a = (90.0 - (90.0 - deg).abs()).clamp(CORNER_DEG, 90.0);
    let scallops = if a > 76.0 { (3.0 * PI * (a - 76.0) / 28.0).sin().powi(2) } else { 0.0 };
    let rho = 5.7 + 0.95 * bell(a, 38.0, 17.0) - 0.2 * scallops + 0.3 * bell(a, FANG_DEG, 5.0);
    let h = lerp(1.8, 2.4, smooth(CORNER_DEG, 88.0, a)) + 0.4 * bell(a, FANG_DEG, 5.5);
    (rho, h)
}

/// Where the lower lip's inner edge stands from the moon's axis, and how high: near the moon at the corners, falling
/// away toward the chin.
fn lower_lip(deg: f64) -> (f64, f64) {
    let a = deg.clamp(-90.0, -CORNER_DEG);
    let rho = 6.0 + 0.35 * bell(a, -32.0, 9.0) + 0.75 * smooth(-45.0, -90.0, a);
    let h = lerp(1.6, 1.25, smooth(-CORNER_DEG, -90.0, a)) + 0.3 * bell(a, -FANG_DEG, 5.5);
    (rho, h)
}

/// How far from the moon's axis the mandible's outline runs at bearing `deg`: close under the corners, narrowing to
/// a pointed chin.
fn jaw_out(deg: f64) -> f64 {
    let a = deg.clamp(-90.0, -CORNER_DEG);
    7.2 - 0.25 * smooth(-40.0, -CORNER_DEG, a) + 1.0 * smooth(-50.0, -90.0, a)
}

/// The facial fur, 0..1: flames flowing up and out toward a point past each ear, in two overlapping tiers, long at the
/// jowls and short by the eyes.
fn cheek_fur(s: P3) -> f64 {
    let (dx, du) = (FUR_SINK.0 - s[0], FUR_SINK.1 - s[1]);
    let d = dx.hypot(du).max(0.5);
    let k = 13.0;
    let (along, across) = (k * (16.0 / d).ln(), k * du.atan2(dx));
    let jowl = flames(along, across, 1.55, 4.2, 0.16, 0.35, 5, &sculpted_lock(1.55)).h * (1.0 - smooth(1.0, 3.5, s[1]));
    let cheek = flames(along + 0.9, across + 0.4, 1.3, 3.0, 0.16, 0.35, 11, &sculpted_lock(1.3)).h * smooth(0.0, 2.5, s[1]);
    smax(jowl, cheek, 0.3)
}

/// Fur over the crown, flowing back from the brows between the ears, 0..1; `s` carries its signed `x`.
fn crown_fur(s: P3) -> f64 {
    flames(s[1] - 6.3, s[0], 1.5, 3.3, 0.16, 0.35, 31, &sculpted_lock(1.5)).h
}

/// Fur on the throat, flowing down the apex wall, 0..1; `s` carries its signed `x`.
fn throat_fur(s: P3) -> f64 {
    flames(-s[2] - 0.4, s[0], 1.4, 3.0, 0.16, 0.35, 53, &sculpted_lock(1.4)).h
}

/// A raised fold of skin along a bowed curve from `a` to `b` (face `x`, `u`), tapering at both ends.
fn fold(q: P3, a: (f64, f64), b: (f64, f64), bow: f64, amp: f64) -> f64 {
    let (dx, du) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(du).max(1e-9);
    let mid = (0.5 * (a.0 + b.0) - bow * du / l, 0.5 * (a.1 + b.1) + bow * dx.abs() / l);
    let mut best = (f64::MAX, 0.0);
    for i in 0..=20 {
        let t = i as f64 / 20.0;
        let x = (1.0 - t) * (1.0 - t) * a.0 + 2.0 * t * (1.0 - t) * mid.0 + t * t * b.0;
        let u = (1.0 - t) * (1.0 - t) * a.1 + 2.0 * t * (1.0 - t) * mid.1 + t * t * b.1;
        let d = (q[0] - x).hypot(q[1] - u);
        if d < best.0 {
            best = (d, t);
        }
    }
    let taper = smooth(0.0, 0.25, best.1) * (1.0 - smooth(0.55, 1.0, best.1));
    amp * (-(best.0 / 0.24).powi(2)).exp() * taper
}

/// The wolf's head as a signed distance field, in face coordinates: `x` across, `u` toward the ears, `h` out of the table.
struct Wolf {
    table: f64,
    bore: f64,
    relief: Relief,
    stock: Stock,
    teeth: Vec<Tooth>,
    /// The masses' surface height where each eye sits.
    eye_h: f64,
}

impl Wolf {
    fn face(&self, p: P3) -> P3 {
        [p[0], -p[2], p[1] - self.table]
    }
    fn new(table: f64, bore: f64, relief: Relief, stock: Stock) -> Self {
        let mut w = Self { table, bore, relief, stock, teeth: Vec::new(), eye_h: 0.0 };
        w.eye_h = w.surface_h(Self::EYE.0, Self::EYE.1);
        w.teeth = Self::dentition();
        w
    }
    /// Where each eye sits, face `x` and `u`.
    const EYE: (f64, f64) = (2.45, 5.5);
    /// The head's top surface over face point (`x`, `u`): the stock's, or the head's masses where they stand higher.
    fn surface_h(&self, x: f64, u: f64) -> f64 {
        let floor = self.relief.at(x, u);
        let mut h = 7.0;
        while h > floor && self.masses([x, u, h]) > 0.0 {
            h -= 0.1;
        }
        if h <= floor {
            return floor;
        }
        let (mut lo, mut hi) = (h, h + 0.1);
        for _ in 0..24 {
            let m = 0.5 * (lo + hi);
            if self.masses([x, u, m]) > 0.0 {
                hi = m;
            } else {
                lo = m;
            }
        }
        lo.max(floor)
    }
    /// Five teeth a jaw a side counting the fang: an incisor, then two premolars and a molar behind the fang, each a
    /// stout body with a short point, standing on the gums clear of the stone.
    fn dentition() -> Vec<Tooth> {
        let mut teeth = Vec::new();
        for (side, sx) in [("right", 1.0), ("left", -1.0)] {
            let m = |p: P3| [p[0] * sx, p[1], p[2]];
            // Each tooth rises out of the gum at a slant toward the moon, its root sunk in the gum under the lip.
            for (name, a, reach, rise, r) in [("upper incisor", 84.5, 0.75, 0.45, 0.62), ("upper premolar", 61.0, 1.15, 0.55, 0.66), ("upper second premolar", 48.0, 1.45, 0.75, 0.68), ("upper molar", 35.0, 1.0, 0.5, 0.7)] {
                let (rho, _) = upper_lip(a);
                let root = rim(rho - 0.25, a, GUM_H - 0.35);
                let tip = rim((rho - 0.25 - reach).max(KEEP_OUT_MM + 0.15), a, GUM_H + rise);
                teeth.push(Tooth::new(format!("{name}, {side}"), m(root), m(tip), r));
            }
            for (name, a, reach, rise, r) in [("lower incisor", -84.5, 0.8, 0.5, 0.62), ("lower premolar", -61.0, 1.05, 0.6, 0.66), ("lower second premolar", -48.0, 0.9, 0.7, 0.66), ("lower molar", -35.0, 1.25, 0.5, 0.7)] {
                let (rho, _) = lower_lip(a);
                let root = rim(rho - 0.25, a, GUM_H - 0.35);
                let tip = rim((rho - 0.25 - reach).max(KEEP_OUT_MM + 0.15), a, GUM_H + rise);
                teeth.push(Tooth::new(format!("{name}, {side}"), m(root), m(tip), r));
            }
        }
        teeth
    }
    fn cranium(s: P3) -> f64 {
        let root = ellipsoid(sub(s, [4.5, 7.7, 0.5]), [1.4, 1.4, 1.4]);
        smin(ellipsoid(sub(s, [0.0, 7.15, -1.0]), [5.2, 2.7, 3.9]), root, 0.6)
    }
    fn cheek(s: P3) -> f64 {
        ellipsoid(sub(s, [5.0, 2.3, -1.5]), [2.8, 3.4, 2.5])
    }
    fn jowl(s: P3) -> f64 {
        ellipsoid(sub(s, [6.1, -1.9, -1.3]), [2.0, 2.6, 2.1])
    }
    /// The unit normal of an ellipsoid's level set through `p`.
    fn ellipsoid_normal(p: P3, c: P3, r: P3) -> P3 {
        let g: P3 = std::array::from_fn(|k| (p[k] - c[k]) / (r[k] * r[k]));
        mul(g, 1.0 / len(g).max(1e-12))
    }
    fn brow(s: P3) -> f64 {
        round_cone(s, [1.05, 5.75, 2.75], [3.55, 7.35, 2.05], 0.55, 0.32)
    }
    /// The brow's crest height over face `x`.
    fn brow_crest(x: f64) -> f64 {
        let t = ((x - 1.05) / 2.5).clamp(0.0, 1.0);
        lerp(2.75, 2.05, t) + lerp(0.55, 0.32, t)
    }
    /// The muzzle: a trapezoid section with a flat bridge and crisp side planes, from the stop to the nose.
    fn muzzle(s: P3) -> f64 {
        let (st, nz) = ([0.0, 5.55, 0.95], [0.0, 1.7, 2.35]);
        let axis = sub(nz, st);
        let l = len(axis);
        let a = mul(axis, 1.0 / l);
        let b = [0.0, a[2], -a[1]];
        let ps = sub(s, st);
        let t = dot(ps, a) / l;
        let tc = t.clamp(0.0, 1.0);
        let sec = [s[0], dot(ps, b)];
        let body = trapezoid(sec, lerp(1.74, 1.29, tc), lerp(0.99, 0.74, tc), lerp(1.39, 1.09, tc)) - 0.22;
        smax(body, (t - 1.0) * l, 0.3).max(-t * l - 1.2)
    }
    /// The nose pad: wider than it is deep, its top running on from the bridge, its front turned down toward the moon.
    fn nose_frame(s: P3) -> P3 {
        turn(sub(s, [0.0, 1.25, 3.5]), 1, 2, 22.0)
    }
    fn nose(s: P3) -> f64 {
        let p = Self::nose_frame(s);
        let pad = [1.3, 0.72, 0.55];
        let q: P3 = std::array::from_fn(|k| (p[k].abs() - pad[k] + 0.4).max(0.0));
        let inner = (p[0].abs() - pad[0] + 0.4).max(p[1].abs() - pad[1] + 0.4).max(p[2].abs() - pad[2] + 0.4).min(0.0);
        smax(len(q) + inner - 0.4, ellipsoid(sub(p, [0.0, 0.0, -0.1]), [1.45, 0.95, 0.85]), 0.25)
    }
    /// The upper jaw at the query's bearing: a rolled lip over the gums, full under the nose and thinning to the
    /// corners, falling back into the cheeks.
    fn upper_jaw(s: P3) -> f64 {
        let ang = (s[1] - MOON_U).atan2(s[0]).to_degrees();
        let a = ang.clamp(CORNER_DEG, 90.0);
        let (rho, h) = upper_lip(a);
        let r = lerp(0.46, 0.62, smooth(25.0, 80.0, a));
        let back = lerp(1.5, 2.4, smooth(20.0, 60.0, a));
        round_cone(s, rim(rho + r, a, h), rim(rho + back, a, -1.0), r, lerp(0.95, 1.25, smooth(20.0, 60.0, a)))
    }
    /// The mandible at the query's bearing: a rolled lip inside a V that narrows to the chin, rounded underneath.
    fn mandible(s: P3) -> f64 {
        let ang = (s[1] - MOON_U).atan2(s[0]).to_degrees();
        let a = ang.clamp(-90.0, -CORNER_DEG);
        let (rho, h) = lower_lip(a);
        let out = smax(jaw_out(a), rho + 1.1, 0.4);
        let r = lerp(0.46, 0.55, smooth(-25.0, -70.0, a));
        let jaw = round_cone(s, rim(rho + r, a, h), rim(out + 0.1, a, -1.3), r, 0.9);
        let chin = ellipsoid(sub(s, [0.0, MOON_U - 7.55, -0.2]), [1.2, 0.85, 0.95]);
        smin(jaw, chin, 0.8)
    }
    /// The gums between the lips and the stone, up to `GUM_H`, left open at the mouth's corners.
    fn gums(s: P3) -> f64 {
        let rho = s[0].hypot(s[1] - MOON_U);
        let ang = (s[1] - MOON_U).atan2(s[0]).to_degrees();
        let lip = if ang >= 0.0 { upper_lip(ang).0 } else { lower_lip(ang).0 };
        let end = (CORNER_DEG + 3.0 - ang.abs()).to_radians() * rho.max(1.0);
        smax((rho - lip - 0.35).max(s[2] - GUM_H).max(-(s[2] + 1.2)), end, 0.4)
    }
    /// The throat under the chin, hanging over the apex wall, its lowest point a millimetre and more over the finger.
    fn throat(s: P3) -> f64 {
        ellipsoid(sub(s, [0.0, MOON_U - 6.1, -1.25]), [2.3, 1.5, 1.55])
    }
    /// A pricked ear: a thick triangle standing up from the crown's corner, curling back at its point, cupped 0.35 mm
    /// deep on the face it turns to the viewer, with a rolled rim.
    fn ear(s: P3) -> f64 {
        let (base, tip) = ([3.7, 7.9, 1.3], [5.1, 11.0, 2.2]);
        let axis = sub(tip, base);
        let l = len(axis);
        let a = mul(axis, 1.0 / l);
        let face = [-0.35, -0.3, 1.0];
        let n0 = sub(face, mul(a, dot(face, a)));
        let n = mul(n0, 1.0 / len(n0));
        let w = cross(a, n);
        let p = sub(s, base);
        let (x, y, z) = (dot(p, w), dot(p, a), dot(p, n));
        let t = (y / l).clamp(0.0, 1.0);
        let half = 1.5 * (1.0 - t).powf(0.85);
        let thick = 1.0 - 0.42 * t;
        let z = z + 0.25 * t * t;
        let r = 0.38;
        let outline = smax(x.abs() - half, -y, 0.8).max(y - l) * 0.9;
        let q = [outline + r, z.abs() - thick + r];
        let plate = len([q[0].max(0.0), q[1].max(0.0), 0.0]) + q[0].max(q[1]).min(0.0) - r;
        // The cup: a parabolic hollow inside the rim, deepest along the ear's middle and shallowing toward both ends.
        let w = (half - 0.6).max(0.01);
        let depth = 0.3 * smooth(0.02, 0.18, t) * (1.0 - smooth(0.3, 0.5, t));
        let cup = (thick - depth * (1.0 - (x / w).powi(2)).max(0.0) - z).max(x.abs() - w).max(0.1 - y).max(y - 0.55 * l);
        smax(plate, -cup, 0.2)
    }
    fn chin_tuft(s: P3) -> f64 {
        let a = round_cone(s, [0.0, MOON_U - 7.7, -0.35], [0.0, MOON_U - 8.5, -1.1], 0.6, 0.42);
        smin(a, round_cone(s, [0.0, MOON_U - 8.5, -1.1], [0.0, MOON_U - 9.0, -1.9], 0.42, 0.25), 0.2)
    }
    /// The head's masses and smooth features, without fur or carved detail.
    fn masses(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let mut d = smin(smin(Self::cranium(s), Self::cheek(s), 1.1), Self::jowl(s), 0.9);
        d = smin(d, Self::upper_jaw(s), 0.7);
        d = smin(d, Self::brow(s), 0.5);
        d = smin(d, Self::muzzle(s), 0.6);
        d = smin(d, Self::nose(s), 0.4);
        let jaws = smin(Self::mandible(s), Self::gums(s), 0.3);
        d = smin(d, smin(jaws, Self::throat(s), 0.9), 0.5);
        smin(d, Self::ear(s), 0.5)
    }
    /// How much fur the skin at face point `f` (signed `x`) takes: none within a millimetre of the stock it stands on,
    /// none on the lips or round the eyes, full on the cheeks, crown and throat.
    fn fur_room(&self, f: P3) -> f64 {
        let over = self.stock.at([f[0], f[2] + self.table, -f[1]]);
        smooth(0.35, 1.35, over)
    }
    /// A mass `d` at `s` with fur `amp` mm high laid over its surface along `n`, fading out `fade` by where it lands.
    fn furred(&self, s: P3, x: f64, d: f64, n: P3, amp: f64, fur: &dyn Fn(P3, P3) -> f64) -> f64 {
        if d.abs() > 1.6 {
            return d;
        }
        let f = sub(s, mul(n, d));
        let signed = [x.signum() * f[0], f[1], f[2]];
        d - amp * fur(f, signed) * self.fur_room(signed) * (1.0 - smooth(0.9, 1.6, d.abs()))
    }
    /// The masses with their fur: the cheeks and jowls, the crown and the throat, each furred on its own convex surface
    /// so no lock is ever raised along converging normals.
    fn body(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let x = if q[0] == 0.0 { 1.0 } else { q[0] };
        let off_face = |f: P3| {
            let rho = f[0].hypot(f[1] - MOON_U);
            let ang = (f[1] - MOON_U).atan2(f[0]).to_degrees();
            let lip = lerp(lower_lip(ang).0, upper_lip(ang).0, smooth(-8.0, 8.0, ang));
            smooth(lip + 0.9, lip + 2.2, rho) * smooth(1.5, 3.2, f[0]) * smooth(1.2, 2.6, (f[0] - Self::EYE.0).hypot(f[1] - Self::EYE.1))
        };
        let cheek_fur_at = |f: P3, _: P3| cheek_fur(f) * off_face(f) * (1.0 - smooth(6.0, 7.8, f[1]));
        let (cc, cr) = ([5.0, 2.3, -1.5], [2.8, 3.4, 2.5]);
        let cheek = self.furred(s, x, Self::cheek(s), Self::ellipsoid_normal(s, cc, cr), 0.42, &cheek_fur_at);
        let (jc, jr) = ([6.1, -1.9, -1.3], [2.0, 2.6, 2.1]);
        let jowl = self.furred(s, x, Self::jowl(s), Self::ellipsoid_normal(s, jc, jr), 0.42, &cheek_fur_at);
        let (kc, kr) = ([0.0, 7.15, -1.0], [5.2, 2.7, 3.9]);
        let crown_at = |f: P3, signed: P3| {
            crown_fur(signed) * smooth(6.2, 7.6, f[1]) * (1.0 - smooth(3.0, 4.6, f[0])) * smooth(-0.4, 1.0, f[2]) * smooth(0.1, 1.0, Self::ear(f))
        };
        let crown = self.furred(s, x, ellipsoid(sub(s, kc), kr), Self::ellipsoid_normal(s, kc, kr), 0.26, &crown_at);
        let root = ellipsoid(sub(s, [4.5, 7.7, 0.5]), [1.4, 1.4, 1.4]);
        let throat_n = Self::ellipsoid_normal(s, [0.0, MOON_U - 6.1, -1.25], [2.3, 1.5, 1.55]);
        let throat_at = |f: P3, signed: P3| throat_fur(signed) * smooth(-0.2, -1.2, f[2]) * (1.0 - smooth(MOON_U - 5.5, MOON_U - 3.5, f[1]));
        let throat = self.furred(s, x, Self::throat(s), throat_n, 0.38, &throat_at);
        let mut d = smin(smin(smin(crown, root, 0.6), cheek, 1.1), jowl, 0.9);
        d = smin(d, Self::upper_jaw(s), 0.7);
        d = smin(d, Self::brow(s), 0.5);
        d = smin(d, Self::muzzle(s), 0.6);
        d = smin(d, Self::nose(s), 0.4);
        let jaws = smin(Self::mandible(s), Self::gums(s), 0.3);
        d = smin(d, smin(jaws, throat, 0.9), 0.5);
        smin(d, Self::ear(s), 0.5)
    }
    fn head(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let core = self.body(q);
        let near = 1.0 - smooth(0.6, 1.2, core.abs());
        let mut d = core;
        // The stop furrow between the brows, 3 mm up the forehead.
        d += 0.3 * near * (-(q[0] / 0.3).powi(2)).exp() * smooth(5.3, 5.8, q[1]) * (1.0 - smooth(8.4, 9.0, q[1]));
        // Three raised folds across the bridge, unequal, bowed toward the eyes, dying out short of the midline on
        // alternating sides; two more from each nose corner back along the lifted lip.
        let folds = fold(q, (2.5, 2.45), (0.3, 3.2), 0.22, 0.15)
            + fold(q, (-2.05, 3.65), (-0.3, 4.05), 0.2, 0.14)
            + fold(q, (1.45, 4.72), (0.3, 4.78), 0.12, 0.12)
            + fold(s, (1.35, 1.05), (2.7, 1.85), -0.1, 0.12)
            + fold(s, (1.25, 0.62), (2.5, 0.72), 0.08, 0.1);
        d -= folds * near;
        // Nostrils on the nose's front, and the groove under it.
        d = smax(d, -ellipsoid(sub(Self::nose_frame(s), [0.55, -0.8, -0.12]), [0.26, 0.2, 0.17]), 0.1);
        d += 0.1 * near * (-(s[0] / 0.14).powi(2)).exp() * smooth(0.95, 0.6, s[1]) * smooth(0.25, 0.5, s[1]);
        // Eyes sunk under the brows: a slanted pocket whose upper rim is the brow, a ball low in it.
        let top = (self.eye_h - 0.05).min(Self::brow_crest(Self::EYE.0) - 0.45);
        let eye = turn(sub(s, [Self::EYE.0, Self::EYE.1, self.eye_h + 0.15]), 0, 1, -25.0);
        d = smax(d, -ellipsoid(eye, [0.98, 0.52, 0.6]), 0.3);
        let ball = ellipsoid(sub(eye, [0.0, -0.04, top - 0.26 - self.eye_h - 0.15]), [0.7, 0.34, 0.26]);
        d = smin(d, ball, 0.22);
        let teeth = self.teeth.iter().fold(f64::MAX, |m, t| m.min(t.sdf(q)));
        d = smin(d, teeth, 0.3);
        smin(d, Self::chin_tuft(s), 0.3)
    }
    /// The feature a face point lies on, and that feature's tip.
    fn label(&self, q: P3) -> (String, P3) {
        let s = [q[0].abs(), q[1], q[2]];
        let side = if q[0] >= 0.0 { "right" } else { "left" };
        let mut best = ("head mass".to_string(), q, 0.1);
        let mut take = |name: String, d: f64, tip: P3| {
            if d.abs() < best.2 {
                best = (name, tip, d.abs());
            }
        };
        take(format!("ear, {side}"), Self::ear(s), [5.1 * q[0].signum(), 11.0, 2.2]);
        for t in &self.teeth {
            // A tooth owns its fillet into the gum.
            let d = t.sdf(q);
            take(t.name.clone(), if d < 0.3 { 0.0 } else { d }, t.tip);
        }
        take("chin tuft".into(), Self::chin_tuft(s), [0.0, MOON_U - 9.0, -1.9]);
        take(format!("brow, {side}"), Self::brow(s), q);
        take("mandible".into(), Self::mandible(s), q);
        take("nose".into(), Self::nose(s), q);
        (best.0, best.1)
    }
    fn sdf(&self, p: P3) -> f64 {
        let q = self.face(p);
        let d = self.head(q);
        let rho = q[0].hypot(q[1] - MOON_U);
        let keep = KEEP_OUT_MM - rho;
        let d = smax(d, keep, 0.45);
        let g = self.stock.at(p);
        // A skirt just under the stock's surface near the head, so the head meets the stock tangentially.
        let d = smin(d, (g + 0.08).max(d - 0.9), 0.45);
        let d = smax(d, -(g + BURY_MM), 0.3);
        smax(d, self.bore + BORE_CLEAR_MM - p[0].hypot(p[1]), 0.2)
    }
}

/// A triangle mesh as positions and faces.
struct Nets {
    v: Vec<P3>,
    f: Vec<[u32; 3]>,
}

/// A closed 2-manifold from a sampled distance field by marching tetrahedra on the Kuhn split of each cube,
/// the field sampled only in blocks a coarse pass finds within reach of the surface.
fn tetra_mesh(lo: P3, hi: P3, step: f64, field: &(dyn Fn(P3) -> f64 + Sync)) -> Nets {
    use std::collections::HashMap;
    let n: [usize; 3] = std::array::from_fn(|k| ((hi[k] - lo[k]) / step).ceil() as usize + 1);
    let at = |i: usize, j: usize, k: usize| -> P3 { [lo[0] + i as f64 * step, lo[1] + j as f64 * step, lo[2] + k as f64 * step] };
    const B: usize = 8;
    let nb: [usize; 3] = std::array::from_fn(|k| (n[k] - 1).div_ceil(B));
    let reach = 2.5 * step * B as f64 * 3f64.sqrt();
    let live: Vec<bool> = (0..nb[0] * nb[1] * nb[2])
        .into_par_iter()
        .map(|m| {
            let (bi, bj, bk) = (m % nb[0], (m / nb[0]) % nb[1], m / (nb[0] * nb[1]));
            let c = at(bi * B + B / 2, bj * B + B / 2, bk * B + B / 2);
            field(c).abs() < reach
        })
        .collect();
    let block = |i: usize, j: usize, k: usize| live[((k / B).min(nb[2] - 1) * nb[1] + (j / B).min(nb[1] - 1)) * nb[0] + (i / B).min(nb[0] - 1)];
    let values: Vec<f32> = (0..n[2])
        .into_par_iter()
        .flat_map_iter(|k| {
            let mut slab = Vec::with_capacity(n[0] * n[1]);
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let border = i == 0 || j == 0 || k == 0 || i == n[0] - 1 || j == n[1] - 1 || k == n[2] - 1;
                    let near = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 1, 0), (1, 0, 1), (0, 1, 1), (1, 1, 1)]
                        .iter()
                        .any(|&(a, b, c)| block(i.saturating_sub(a), j.saturating_sub(b), k.saturating_sub(c)));
                    let v = if border { 1.0 } else if near { field(at(i, j, k)) as f32 } else { 1.0 };
                    slab.push(if v.abs() < 1e-5 { 1e-5 } else { v });
                }
            }
            slab
        })
        .collect();
    let idx = |i: usize, j: usize, k: usize| (k * n[1] + j) * n[0] + i;
    const TETS: [[usize; 4]; 6] = [[0, 1, 3, 7], [0, 1, 5, 7], [0, 2, 3, 7], [0, 2, 6, 7], [0, 4, 5, 7], [0, 4, 6, 7]];
    let mut index: HashMap<(usize, usize), u32> = HashMap::new();
    let mut v: Vec<P3> = Vec::new();
    let mut f: Vec<[u32; 3]> = Vec::new();
    for k in 0..n[2] - 1 {
        for j in 0..n[1] - 1 {
            for i in 0..n[0] - 1 {
                let g: [usize; 8] = std::array::from_fn(|c| idx(i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1)));
                let c: [f32; 8] = std::array::from_fn(|m| values[g[m]]);
                let inside = c.iter().filter(|x| **x < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let pos = |m: usize| at(i + (m & 1), j + ((m >> 1) & 1), k + ((m >> 2) & 1));
                for tet in TETS {
                    let ins: Vec<usize> = tet.iter().copied().filter(|m| c[*m] < 0.0).collect();
                    let outs: Vec<usize> = tet.iter().copied().filter(|m| c[*m] >= 0.0).collect();
                    if ins.is_empty() || outs.is_empty() {
                        continue;
                    }
                    let mut vert = |a: usize, b: usize| -> u32 {
                        let key = (g[a].min(g[b]), g[a].max(g[b]));
                        *index.entry(key).or_insert_with(|| {
                            let t = ((c[a] / (c[a] - c[b])) as f64).clamp(0.02, 0.98);
                            v.push(lerp3(pos(a), pos(b), t));
                            (v.len() - 1) as u32
                        })
                    };
                    let mut tris: Vec<[u32; 3]> = Vec::new();
                    match (ins.len(), outs.len()) {
                        (1, 3) => tris.push([vert(ins[0], outs[0]), vert(ins[0], outs[1]), vert(ins[0], outs[2])]),
                        (3, 1) => tris.push([vert(ins[0], outs[0]), vert(ins[1], outs[0]), vert(ins[2], outs[0])]),
                        _ => {
                            let (a, b, c2, d) = (vert(ins[0], outs[0]), vert(ins[0], outs[1]), vert(ins[1], outs[1]), vert(ins[1], outs[0]));
                            tris.push([a, b, c2]);
                            tris.push([a, c2, d]);
                        }
                    }
                    let inner = ins.iter().fold([0.0; 3], |s, m| add(s, pos(*m)));
                    let outer = outs.iter().fold([0.0; 3], |s, m| add(s, pos(*m)));
                    let dir = sub(mul(outer, 1.0 / outs.len() as f64), mul(inner, 1.0 / ins.len() as f64));
                    for t in tris {
                        let [p, q, r] = t.map(|x| v[x as usize]);
                        let nrm = cross(sub(q, p), sub(r, p));
                        f.push(if dot(nrm, dir) >= 0.0 { t } else { [t[0], t[2], t[1]] });
                    }
                }
            }
        }
    }
    Nets { v, f }
}

fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The field's gradient at `p` by central differences.
fn gradient(field: &(dyn Fn(P3) -> f64 + Sync), p: P3) -> P3 {
    let e = 1e-3;
    std::array::from_fn(|k| {
        let (mut a, mut b) = (p, p);
        a[k] += e;
        b[k] -= e;
        (field(a) - field(b)) / (2.0 * e)
    })
}

/// Relax each vertex toward its neighbours' centroid within its tangent plane, then step it back onto the surface.
/// Vertices where the surface turns sharply stay put, no vertex moves more than a third of its shortest edge, and a
/// triangle that turns over puts its corners back.
fn relax(mesh: &mut Nets, field: &(dyn Fn(P3) -> f64 + Sync), rounds: usize) {
    let mut ring: Vec<Vec<u32>> = vec![Vec::new(); mesh.v.len()];
    for t in &mesh.f {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            ring[a as usize].push(b);
            ring[b as usize].push(a);
        }
    }
    for r in &mut ring {
        r.sort_unstable();
        r.dedup();
    }
    let unit = |g: P3| mul(g, 1.0 / len(g).max(1e-12));
    for _ in 0..rounds {
        let normals: Vec<P3> = mesh.v.par_iter().map(|p| unit(gradient(field, *p))).collect();
        let before = mesh.v.clone();
        let next: Vec<P3> = (0..mesh.v.len())
            .into_par_iter()
            .map(|i| {
                let p = before[i];
                let n = normals[i];
                if ring[i].iter().any(|j| dot(normals[*j as usize], n) < 0.87) {
                    return p;
                }
                let shortest = ring[i].iter().map(|j| len(sub(before[*j as usize], p))).sum::<f64>() / ring[i].len().max(1) as f64;
                let c = ring[i].iter().fold([0.0; 3], |s, j| add(s, before[*j as usize]));
                let c = mul(c, 1.0 / ring[i].len().max(1) as f64);
                let dv = sub(c, p);
                let dv = mul(sub(dv, mul(n, dot(dv, n))), 0.5);
                let cap = shortest / 3.0;
                let dv = if len(dv) > cap { mul(dv, cap / len(dv)) } else { dv };
                let mut q = add(p, dv);
                for _ in 0..2 {
                    let f = field(q);
                    let g = gradient(field, q);
                    let step = mul(g, f / dot(g, g).max(1e-9));
                    if len(step) > cap {
                        return p;
                    }
                    q = sub(q, step);
                }
                q
            })
            .collect();
        mesh.v = next;
        let mut undo = vec![false; mesh.v.len()];
        for t in &mesh.f {
            let [a, b, c] = t.map(|x| mesh.v[x as usize]);
            let nf = cross(sub(b, a), sub(c, a));
            let nv = t.iter().fold([0.0; 3], |s, x| add(s, normals[*x as usize]));
            if dot(nf, nv) <= 0.3 * len(nf) * len(nv) {
                for &x in t {
                    undo[x as usize] = true;
                }
            }
        }
        for (i, u) in undo.iter().enumerate() {
            if *u {
                mesh.v[i] = before[i];
            }
        }
    }
}

/// Quadric edge-collapse decimation of a closed manifold to `target` faces, never past `max_cost` or a collapse
/// that breaks the link condition, turns a face more than `max_turn_deg` or leaves one sharper than `min_deg`.
fn decimate(mesh: &Nets, target: usize, max_cost: f64, min_deg: f64, max_turn_deg: f64, max_fold_deg: f64) -> Nets {
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashSet};
    #[derive(PartialEq)]
    struct Item(f64, u32, u32, u32, u32);
    impl Eq for Item {}
    impl PartialOrd for Item {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Item {
        fn cmp(&self, o: &Self) -> Ordering {
            o.0.total_cmp(&self.0)
        }
    }
    let mut pos = mesh.v.clone();
    let mut faces = mesh.f.clone();
    let mut alive = vec![true; faces.len()];
    let mut vfaces: Vec<Vec<u32>> = vec![Vec::new(); pos.len()];
    for (i, t) in faces.iter().enumerate() {
        for &x in t {
            vfaces[x as usize].push(i as u32);
        }
    }
    let plane = |t: &[u32; 3], pos: &[P3]| -> Option<[f64; 4]> {
        let [a, b, c] = t.map(|x| pos[x as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let l = len(n);
        (l > 1e-14).then(|| {
            let n = mul(n, 1.0 / l);
            [n[0], n[1], n[2], -dot(n, a)]
        })
    };
    let mut quad = vec![[0.0f64; 10]; pos.len()];
    let add_q = |q: &mut [f64; 10], p: [f64; 4]| {
        let [a, b, c, d] = p;
        let terms = [a * a, a * b, a * c, a * d, b * b, b * c, b * d, c * c, c * d, d * d];
        for k in 0..10 {
            q[k] += terms[k];
        }
    };
    for t in &faces {
        if let Some(p) = plane(t, &pos) {
            for &x in t {
                add_q(&mut quad[x as usize], p);
            }
        }
    }
    let err = |q: &[f64; 10], p: P3| {
        let [x, y, z] = p;
        q[0] * x * x + 2.0 * q[1] * x * y + 2.0 * q[2] * x * z + 2.0 * q[3] * x + q[4] * y * y + 2.0 * q[5] * y * z + 2.0 * q[6] * y + q[7] * z * z + 2.0 * q[8] * z + q[9]
    };
    let best = |q: &[f64; 10], a: P3, b: P3| -> (P3, f64) {
        let m = [[q[0], q[1], q[2]], [q[1], q[4], q[5]], [q[2], q[5], q[7]]];
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        let mut cands = vec![a, b, mul(add(a, b), 0.5)];
        if det.abs() > 1e-9 {
            let r = [-q[3], -q[6], -q[8]];
            let solve = |col: usize| {
                let mut mm = m;
                for row in 0..3 {
                    mm[row][col] = r[row];
                }
                (mm[0][0] * (mm[1][1] * mm[2][2] - mm[1][2] * mm[2][1]) - mm[0][1] * (mm[1][0] * mm[2][2] - mm[1][2] * mm[2][0]) + mm[0][2] * (mm[1][0] * mm[2][1] - mm[1][1] * mm[2][0])) / det
            };
            let p = [solve(0), solve(1), solve(2)];
            let mid = mul(add(a, b), 0.5);
            if len(sub(p, mid)) < len(sub(a, b)) {
                cands.push(p);
            }
        }
        cands.into_iter().map(|p| (p, err(q, p))).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap()
    };
    let mut version = vec![0u32; pos.len()];
    let mut heap = BinaryHeap::new();
    let push_edges = |v0: u32, heap: &mut BinaryHeap<Item>, faces: &[[u32; 3]], alive: &[bool], vfaces: &[Vec<u32>], pos: &[P3], quad: &[[f64; 10]], version: &[u32]| {
        let mut seen = HashSet::new();
        for &fi in &vfaces[v0 as usize] {
            if !alive[fi as usize] {
                continue;
            }
            for &w in &faces[fi as usize] {
                if w != v0 && seen.insert(w) {
                    let mut q = quad[v0 as usize];
                    for k in 0..10 {
                        q[k] += quad[w as usize][k];
                    }
                    let (_, cost) = best(&q, pos[v0 as usize], pos[w as usize]);
                    heap.push(Item(cost, v0, w, version[v0 as usize], version[w as usize]));
                }
            }
        }
    };
    for v0 in 0..pos.len() as u32 {
        push_edges(v0, &mut heap, &faces, &alive, &vfaces, &pos, &quad, &version);
    }
    let mut count = faces.len();
    let min_cos = min_deg.to_radians().cos();
    let turn_cos = max_turn_deg.to_radians().cos();
    while count > target {
        let Some(Item(cost, a, b, va, vb)) = heap.pop() else { break };
        if cost > max_cost {
            break;
        }
        if version[a as usize] != va || version[b as usize] != vb {
            continue;
        }
        let fa: Vec<u32> = vfaces[a as usize].iter().copied().filter(|f| alive[*f as usize]).collect();
        let fb: Vec<u32> = vfaces[b as usize].iter().copied().filter(|f| alive[*f as usize]).collect();
        let shared: Vec<u32> = fa.iter().copied().filter(|f| faces[*f as usize].contains(&b)).collect();
        if shared.len() != 2 {
            continue;
        }
        let ring = |fs: &[u32], me: u32| -> HashSet<u32> { fs.iter().flat_map(|f| faces[*f as usize]).filter(|x| *x != me).collect() };
        let (na, nb) = (ring(&fa, a), ring(&fb, b));
        let common: Vec<u32> = na.intersection(&nb).copied().collect();
        let opposite: HashSet<u32> = shared.iter().flat_map(|f| faces[*f as usize]).filter(|x| *x != a && *x != b).collect();
        if common.len() != 2 || !common.iter().all(|x| opposite.contains(x)) {
            continue;
        }
        let mut q = quad[a as usize];
        for k in 0..10 {
            q[k] += quad[b as usize][k];
        }
        let (p, _) = best(&q, pos[a as usize], pos[b as usize]);
        let mut ok = true;
        for &fi in fa.iter().chain(&fb) {
            if shared.contains(&fi) {
                continue;
            }
            let t = faces[fi as usize];
            let old = t.map(|x| pos[x as usize]);
            let new = t.map(|x| if x == a || x == b { p } else { pos[x as usize] });
            let (n0, n1) = (cross(sub(old[1], old[0]), sub(old[2], old[0])), cross(sub(new[1], new[0]), sub(new[2], new[0])));
            if len(n1) < 1e-10 || dot(n0, n1) <= turn_cos * len(n0) * len(n1) {
                ok = false;
                break;
            }
            for e in 0..3 {
                let (u, w) = (sub(new[(e + 1) % 3], new[e]), sub(new[(e + 2) % 3], new[e]));
                if dot(u, w) > min_cos * len(u) * len(w) {
                    ok = false;
                }
            }
            if !ok {
                break;
            }
        }
        if !ok {
            continue;
        }
        // No edge round the collapse may fold past `max_fold_deg` unless it already did.
        let fold = {
            let near: HashSet<u32> = na.union(&nb).copied().chain([a, b]).flat_map(|x| vfaces[x as usize].iter().copied()).filter(|f| alive[*f as usize]).collect();
            let worst = |after: bool| -> f64 {
                let mut edges: std::collections::HashMap<(u32, u32), Vec<P3>> = std::collections::HashMap::new();
                for &fi in &near {
                    if after && shared.contains(&fi) {
                        continue;
                    }
                    let t = faces[fi as usize].map(|x| if after && x == b { a } else { x });
                    let ps = t.map(|x| if after && x == a { p } else { pos[x as usize] });
                    let n = cross(sub(ps[1], ps[0]), sub(ps[2], ps[0]));
                    let n = mul(n, 1.0 / len(n).max(1e-30));
                    let touches = |x: u32| x == a || (!after && x == b);
                    for e in 0..3 {
                        let (u, w) = (t[e], t[(e + 1) % 3]);
                        if touches(u) || touches(w) || touches(t[(e + 2) % 3]) {
                            edges.entry((u.min(w), u.max(w))).or_default().push(n);
                        }
                    }
                }
                edges.values().filter(|ns| ns.len() == 2).map(|ns| dot(ns[0], ns[1]).clamp(-1.0, 1.0).acos().to_degrees()).fold(0.0, f64::max)
            };
            let after = worst(true);
            after > max_fold_deg && after > worst(false) + 1.0
        };
        if fold {
            continue;
        }
        pos[a as usize] = p;
        quad[a as usize] = q;
        for &fi in &shared {
            alive[fi as usize] = false;
            count -= 1;
        }
        for &fi in &fb {
            if alive[fi as usize] {
                for x in faces[fi as usize].iter_mut() {
                    if *x == b {
                        *x = a;
                    }
                }
                vfaces[a as usize].push(fi);
            }
        }
        vfaces[b as usize].clear();
        vfaces[a as usize].retain(|f| alive[*f as usize]);
        vfaces[a as usize].sort_unstable();
        vfaces[a as usize].dedup();
        version[a as usize] += 1;
        version[b as usize] += 1;
        push_edges(a, &mut heap, &faces, &alive, &vfaces, &pos, &quad, &version);
    }
    let mut remap = vec![u32::MAX; pos.len()];
    let mut v = Vec::new();
    let mut f = Vec::new();
    for (i, t) in faces.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        f.push(t.map(|x| {
            if remap[x as usize] == u32::MAX {
                remap[x as usize] = v.len() as u32;
                v.push(pos[x as usize]);
            }
            remap[x as usize]
        }));
    }
    Nets { v, f }
}

/// [`decimate`] to `target` faces, backing off toward the raw mesh until the result does not cross itself.
fn clean_decimate(raw: &Nets, target: usize) -> Nets {
    for (k, cap) in [2e-3, 1e-3, 5e-4, 2e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 20_000 * k, cap, 2.0 + k as f64, 18.0, 35.0);
        let crossings = csg::self_crossings(&csg::Solid { v: nets.v.clone(), f: nets.f.clone() });
        if crossings == 0 {
            return nets;
        }
        println!("  decimation {k}: {crossings} crossings, backing off");
    }
    decimate(raw, raw.f.len(), 0.0, 0.0, 0.0, 180.0)
}

/// How far the field's own normal turns across each face-zone fold of `m`: a mesh artifact turns little, a real crease a lot.
fn fold_causes(wolf: &Wolf, m: &Nets, field: &(dyn Fn(P3) -> f64 + Sync)) {
    let mut edges: std::collections::HashMap<(u32, u32), Vec<(P3, P3)>> = std::collections::HashMap::new();
    for t in &m.f {
        let [a, b, c] = t.map(|x| m.v[x as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let n = mul(n, 1.0 / len(n).max(1e-30));
        let centre = mul(add(add(a, b), c), 1.0 / 3.0);
        for e in 0..3 {
            let (u, w) = (t[e], t[(e + 1) % 3]);
            edges.entry((u.min(w), u.max(w))).or_default().push((n, centre));
        }
    }
    let mut hist = [0usize; 7];
    let mut shown = 0;
    let mut real: std::collections::BTreeMap<[i64; 3], usize> = std::collections::BTreeMap::new();
    for (k, fs) in &edges {
        if fs.len() != 2 || dot(fs[0].0, fs[1].0) >= 0.5 {
            continue;
        }
        let mid = mul(add(m.v[k.0 as usize], m.v[k.1 as usize]), 0.5);
        let q = wolf.face(mid);
        if !(q[2] > 0.3 && q[0].hypot(q[1] - MOON_U) > 7.0 && q[1] < 7.0 && q[0].abs() < 9.0) {
            continue;
        }
        let g0 = gradient(field, fs[0].1);
        let g1 = gradient(field, fs[1].1);
        let turn = (dot(g0, g1) / (len(g0) * len(g1)).max(1e-30)).clamp(-1.0, 1.0).acos().to_degrees();
        hist[((turn / 10.0) as usize).min(6)] += 1;
        if turn >= 30.0 {
            *real.entry(q.map(|c| (c * 2.0).round() as i64)).or_default() += 1;
        }
        let e = len(sub(m.v[k.0 as usize], m.v[k.1 as usize]));
        if shown < 0 {
            println!("    fold at face ({:.2}, {:.2}, {:.2}): field turns {turn:.0} deg across it, edge {e:.3} mm, face spread {:.3} mm", q[0], q[1], q[2], len(sub(fs[0].1, fs[1].1)));
            shown += 1;
        }
    }
    println!("  folds by the field's own turn, per 10 degrees: {hist:?}");
    if let Ok(spec) = std::env::var("FENRIR_SLICE") {
        // An inside/outside map in the plane through a face point, across `x` and up `h`, at a fixed `u`: '#' the head,
        // 'o' the stock, '+' both.
        let v: Vec<f64> = spec.split(',').map(|x| x.parse().unwrap()).collect();
        let (cx, cu, ch) = (v[0], v[1], v[2]);
        let step = v.get(3).copied().unwrap_or(0.05);
        let along_u = v.get(4).is_some_and(|f| *f > 0.0);
        for row in (0..40).rev() {
            let h = ch - 20.0 * step + row as f64 * step;
            let line: String = (0..80).map(|col| {
                let x = cx - 40.0 * step + col as f64 * step;
                let world = if along_u { [cx, h + wolf.table, -(cu - 40.0 * step + col as f64 * step)] } else { [x, h + wolf.table, -cu] };
                match (wolf.sdf(world) < 0.0, wolf.stock.at(world) < 0.0) {
                    (true, true) => '+',
                    (true, false) => '#',
                    (false, true) => 'o',
                    _ => '.',
                }
            }).collect();
            println!("    h {h:5.2} {line}");
        }
    }
    for c in [[1.15, -10.8, 1.65], [1.05, -10.8, 1.65], [0.95, -10.8, 1.65]] {
        let s = c;
        let teeth = wolf.teeth.iter().map(|t| (t.name.clone(), t.sdf(s))).fold(("".to_string(), f64::MAX), |a, b| if b.1 < a.1 { b } else { a });
        let parts = [("cranium", Wolf::cranium(s)), ("cheek", Wolf::cheek(s)), ("jowl", Wolf::jowl(s)), ("upper jaw", Wolf::upper_jaw(s)), ("brow", Wolf::brow(s)), ("muzzle", Wolf::muzzle(s)), ("nose", Wolf::nose(s)), ("mandible", Wolf::mandible(s)), ("gums", Wolf::gums(s)), ("throat", Wolf::throat(s)), ("ear", Wolf::ear(s)), ("tuft", Wolf::chin_tuft(s))];
        println!("    at {c:?}: masses {:.3}, body {:.3}, head {:.3}, nearest tooth {} {:.3}; {}", wolf.masses(s), wolf.body(s), wolf.head(s), teeth.0, teeth.1, parts.iter().map(|(n, v)| format!("{n} {v:.2}")).collect::<Vec<_>>().join(", "));
    }
    println!("  field creases by half-mm cell (face x, u, h): {:?}", real.iter().map(|(c, n)| (c.map(|x| x as f64 / 2.0), *n)).collect::<Vec<_>>());
}

/// Edges of `m` turning 60 degrees or more over the cheeks, brow and muzzle.
fn zone_folds(wolf: &Wolf, m: &Nets) -> usize {
    let mut edges: std::collections::HashMap<(u32, u32), Vec<P3>> = std::collections::HashMap::new();
    for t in &m.f {
        let [a, b, c] = t.map(|x| m.v[x as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let n = mul(n, 1.0 / len(n).max(1e-30));
        for e in 0..3 {
            let (u, w) = (t[e], t[(e + 1) % 3]);
            edges.entry((u.min(w), u.max(w))).or_default().push(n);
        }
    }
    edges
        .iter()
        .filter(|(k, ns)| {
            let q = wolf.face(mul(add(m.v[k.0 as usize], m.v[k.1 as usize]), 0.5));
            ns.len() == 2 && q[2] > 0.3 && q[0].hypot(q[1] - MOON_U) > 7.0 && q[1] < 7.0 && q[0].abs() < 9.0 && dot(ns[0], ns[1]) < 0.5
        })
        .count()
}

/// Two more rounds of relaxing on a decimated mesh, kept only if it still does not cross itself.
fn settle(nets: Nets, field: &(dyn Fn(P3) -> f64 + Sync)) -> Nets {
    let mut relaxed = Nets { v: nets.v.clone(), f: nets.f.clone() };
    relax(&mut relaxed, field, 2);
    if csg::self_crossings(&csg::Solid { v: relaxed.v.clone(), f: relaxed.f.clone() }) == 0 { relaxed } else { nets }
}

/// Undirected edges used other than twice, and whether the mesh encloses positive volume.
fn closure(v: &[P3], f: &[[u32; 3]]) -> (usize, f64) {
    use std::collections::HashMap;
    let mut uses: HashMap<(u32, u32), i32> = HashMap::new();
    for t in f {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { 1000 };
        }
    }
    let bad = uses.values().filter(|u| **u != 1001).count();
    let vol = f
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| v[i as usize]);
            dot(a, [b[1] * c[2] - b[2] * c[1], b[2] * c[0] - b[0] * c[2], b[0] * c[1] - b[1] * c[0]]) / 6.0
        })
        .sum();
    (bad, vol)
}

fn to_mesh(nets: &Nets, field: &(dyn Fn(P3) -> f64 + Sync)) -> Mesh {
    let normals: Vec<Vec3> = nets
        .v
        .par_iter()
        .map(|p| {
            let g = gradient(field, *p);
            let l = len(g).max(1e-12);
            Vec3((g[0] / l) as f32, (g[1] / l) as f32, (g[2] / l) as f32)
        })
        .collect();
    Mesh {
        vertices: nets.v.iter().map(|p| Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        normals,
        faces: nets.f.clone(),
        ..Default::default()
    }
}

fn moonstone() -> Gem {
    Gem { preview_tint: Some([0.66, 0.72, 0.86]), ..Gem::cabochon(GemCut::Round, MOON_MM) }
}

/// Atlas the hide is painted on.
const AW: usize = 1536;
/// Atlas rows that make its texels square in the chart, so DFM's disc reads a stroke at its true width.
fn atlas_rows(d: &RingDesign) -> usize {
    let ctx = d.field_context();
    (AW as f64 * ctx.band_v_len_mm / ctx.circumference_mm).round() as usize
}

/// Marching grid of the head, mm, and the faces it is decimated to.
const SCULPT_STEP: f64 = 0.09;
const SCULPT_FACES: usize = 120_000;
/// Seam bead where the head meets the stock, mm.
const HEAD_BLEND_MM: f64 = 0.0;
/// The ruff's relief and the palm's fetter, mm.
const RUFF_MM: f64 = 0.7;
const GLEIPNIR_MM: f64 = 0.6;
const BINDING_MM: f64 = 0.5;
const HAIR_LINES_MM: f64 = 0.08;

fn base() -> Result<RingDesign> {
    let mut d = RingDesign::default();
    let source = PRESETS.iter().find(|p| p.id == "010").unwrap().load()?;
    ImportedBase::attach(&mut d, source)?;
    d.name = "Fenrir — the wolf and the moon".into();
    d.imported_base.as_mut().unwrap().sand_envelope = false;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 17.0;
    d.shank.head.length_mm = 16.0;
    d.size = ringdesign_core::resize::size_from_bore(19.0).unwrap();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart {
        profile: d.profile.clone(),
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
    setup.recipe.name = "Fenrir / investment / Gold 18k".into();
    setup.recipe.alloy = "Gold 18k".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").unwrap().shrink_pct;
    setup.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    setup.sample_pitch_mm = 0.1;
    setup.bench_notes = "Investment cast the head, jaws and the four fangs in place. Clean investment from the maw, ears and ruff. Seat the 10 mm moonstone on the fangs' collar and close the four fangs over its girdle. Cut the hair lines with a graver and polish the teeth, nose and moon's rim bright.".into();
    setup
}

fn joined(blend: f64) -> Component {
    Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::Free, blend_mm: blend, ..Component::default() }
}

/// How far up the moonstone's dome the fangs' points rest, of its height.
const FANG_RISE: f64 = 0.6;
/// The moonstone leans back this far to stand level over the table's slope toward the apex, degrees.
const MOON_TILT_DEG: f64 = 4.0;
/// The fangs' wire, which also sizes their base rail, mm.
const FANG_WIRE_MM: f64 = 1.8;

/// The band anchor, the moonstone and its four fangs.
fn stone_and_fangs(d: &mut RingDesign) -> Result<()> {
    let gem = moonstone();
    let doc = d.cad.get_or_insert_with(Document::default);
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let placement = Placement::Ring { theta_deg: 90.0, across_mm: -MOON_U, height_mm: builders::stand_off_mm(builders::CLAW, gem), spin_deg: 90.0, tilt_deg: MOON_TILT_DEG, cant_deg: 0.0 };
    doc.append(builders::stone_feature(2, gem, placement))?;
    let mut fangs = builders::feature_on(3, "Fangs", builders::CLAW, 2, json!({"prongs": 4, "wire_mm": FANG_WIRE_MM, "rails": "Base", "style": "Fang", "grouping": "Jaws", "tip": "Point", "rise": FANG_RISE}));
    fangs.component.attach = Attach::Join;
    fangs.component.stage = Stage::Cast;
    fangs.component.blend_mm = 0.0;
    doc.append(fangs)?;
    Ok(())
}

fn sculpt_preview(out: &Path, step: f64) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut d = base()?;
    let a = Atlas::of(&d, AW, atlas_rows(&d))?;
    let wolf = wolf_of(&d, &a)?;
    let (lo, hi) = sculpt_box(wolf.table);
    let field = |p: P3| wolf.sdf(p);
    let t = Instant::now();
    let mut raw = tetra_mesh(lo, hi, step, &field);
    println!("  marched {} triangles in {:.1} s", raw.f.len(), t.elapsed().as_secs_f64());
    relax(&mut raw, &field, 3);
    let nets = settle(clean_decimate(&raw, SCULPT_FACES), &field);
    println!("  relaxed and decimated to {} in {:.1} s", nets.f.len(), t.elapsed().as_secs_f64());
    println!("  face-zone folds of 60 degrees: raw {}, decimated {}", zone_folds(&wolf, &raw), zone_folds(&wolf, &nets));
    fold_causes(&wolf, &raw, &field);
    let sculpt = to_mesh(&nets, &field);
    stone_and_fangs(&mut d)?;
    let (head, _) = head_feature(&wolf)?;
    d.cad.as_mut().unwrap().append(head)?;
    let lib = AlphaLibrary::builtin();
    let params = BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..Default::default() };
    let built = mesh::try_build(&d, &lib, params)?;
    println!("  built {} triangles; notes {:?} {:?}", built.mesh.faces.len(), built.solids.notes, built.parts.notes);
    let lands = measure_lands(out, &wolf, &built)?;
    println!("  unnamed sub-floor {:?}; head mass min {:.3} mm; longest 60 deg seam run {:.2} mm", lands.unnamed, lands.head_min, lands.seam_run);
    if let Ok(spec) = std::env::var("FENRIR_LOOK") {
        // Close views framed on face points: "x,u,h,yaw,pitch,half;..." renders look-N.png, the head gold and the rest green.
        for (k, one) in spec.split(';').enumerate() {
            let v: Vec<f64> = one.split(',').map(|x| x.parse().unwrap()).collect();
            let c = [v[0], v[2] + wolf.table, -v[1]];
            let owner = |fi: usize| built.mesh.faces[fi].iter().map(|i| built.mesh.origin.get(*i as usize).and_then(|o| built.parts.feature_of(*o))).fold(None, |a, b| a.or(b));
            let head = clip_where(&built.mesh, c, 3.0 * v[5], |fi| owner(fi) == Some(4));
            let other = clip_where(&built.mesh, c, 3.0 * v[5], |fi| owner(fi) != Some(4));
            let frame = markers(&[add(c, [-v[5]; 3]), add(c, [v[5]; 3])], 0.002);
            let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
            let mut parts = vec![Part::metal(&frame, render::GOLD), Part::metal(&head, render::GOLD), Part::metal(&other, [0.55, 0.75, 0.6])];
            parts.extend(gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)));
            render::write_png_parts(out.join(format!("look-{k}.png")), &parts, v[3], v[4], 900)?;
        }
    }
    println!("  census {}", lands.census);
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    for (m, _) in &gems {
        let v: Vec<P3> = m.vertices.iter().map(|p| wolf.face([p.0 as f64, p.1 as f64, p.2 as f64])).collect();
        let lo = v.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        let hi = v.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
        let far = |f: &dyn Fn(&P3) -> f64| v.iter().map(|p| f(p)).fold(f64::MIN, f64::max);
        println!("  stone: h {lo:.3}..{hi:.3}; reach +x {:.3} -x {:.3} +u {:.3} -u {:.3}", far(&|p| p[0]), far(&|p| -p[0]), far(&|p| p[1]), far(&|p| -p[1]));
        for (label, pick) in [("+u", 1usize), ("-u", 1), ("+x", 0), ("-x", 0)] {
            let sign = if label.starts_with('+') { 1.0 } else { -1.0 };
            let p = v.iter().copied().fold([0.0, 0.0, f64::MIN], |b, p| if sign * (p[pick] - if pick == 1 { MOON_U } else { 0.0 }) > sign * (b[pick] - if pick == 1 { MOON_U } else { 0.0 }) || b[2] == f64::MIN { p } else { b });
            println!("    girdle toward {label}: ({:.2}, {:.2}, {:.2})", p[0], p[1], p[2]);
        }
    }
    for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
        let Some(made) = &c.made else { continue };
        if !made.key.contains("claw") {
            continue;
        }
        let s = made.solid();
        for (pi, name) in made.named.names.iter().enumerate() {
            let hs: Vec<P3> = s.f.iter().zip(&made.named.patch).filter(|(_, p)| **p as usize == pi).flat_map(|(t, _)| t.map(|x| wolf.face(s.v[x as usize]))).collect();
            let top = hs.iter().copied().fold([0.0, 0.0, f64::MIN], |b, p| if p[2] > b[2] { p } else { b });
            println!("    {name}: top at ({:.2}, {:.2}, {:.2}), lowest h {:.2}", top[0], top[1], top[2], hs.iter().map(|p| p[2]).fold(f64::MAX, f64::min));
        }
    }
    for tip in fang_tips(&built) {
        let q = wolf.face(tip);
        let rho = q[0].hypot(q[1] - MOON_U);
        println!("  fang tip at face ({:.2}, {:.2}, {:.2}): {:.2} mm inside the girdle", q[0], q[1], q[2], MOON_MM * 0.5 - rho);
    }
    let _ = sculpt;
    let mut close = vec![Part::metal(&built.mesh, render::GOLD)];
    close.extend(gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)));
    render::write_png_parts(out.join("sculpt-close-face.png"), &close, 0.0, PI * 0.5, 1100)?;
    render::write_png_parts(out.join("sculpt-close-hero.png"), &close, 0.55, 0.95, 1100)?;
    Ok(())
}

/// The head's grid box in world millimetres over a table at `table`.
fn sculpt_box(table: f64) -> (P3, P3) {
    ([-13.5, table - 6.5, -12.5], [13.5, table + 6.8, 15.0])
}

/// The wolf's field over the design's own stock.
fn wolf_of(d: &RingDesign, a: &Atlas) -> Result<Wolf> {
    let table = a.top;
    let relief = Relief::of(a, table);
    let coarse = Atlas::of(d, 1024, 384)?;
    let (lo, hi) = sculpt_box(table);
    let stock = Stock::of(&coarse, &relief, table, lo, hi, 0.25);
    Ok(Wolf::new(table, d.inner_radius_mm(), relief, stock))
}

/// The head as a stored part, joined to the stock.
fn head_feature(wolf: &Wolf) -> Result<(Feature, Value)> {
    let t = Instant::now();
    let (lo, hi) = sculpt_box(wolf.table);
    let field = |p: P3| wolf.sdf(p);
    let mut raw = tetra_mesh(lo, hi, SCULPT_STEP, &field);
    relax(&mut raw, &field, 3);
    let nets = settle(clean_decimate(&raw, SCULPT_FACES), &field);
    let (bad, volume) = closure(&nets.v, &nets.f);
    ensure!(bad == 0 && volume > 0.0, "The head does not close: {bad} open edges");
    let crossings = csg::self_crossings(&csg::Solid { v: nets.v.clone(), f: nets.f.clone() });
    ensure!(crossings == 0, "The head crosses itself {crossings} times");
    let mesh = cad::stored::Packed::encode(&nets.v, &nets.f, &vec![0; nets.f.len()], &[cad::SurfaceKind::Freeform])?;
    let stats = json!({"marching_step_mm": SCULPT_STEP, "raw_triangles": raw.f.len(), "triangles": nets.f.len(), "vertices": nets.v.len(), "volume_mm3": volume, "packed_bytes": mesh.data.len(), "self_crossings": crossings, "seconds": t.elapsed().as_secs_f64()});
    println!("  head: {} triangles from {} in {:.1} s, {} KB packed", nets.f.len(), raw.f.len(), t.elapsed().as_secs_f64(), mesh.data.len() / 1024);
    let recipe = cad::stored::Recipe {
        kernel: "fenrir".into(),
        op: "sculpt".into(),
        params: json!({"field": "bestiarium_fenrir.rs Wolf::sdf", "moon_u_mm": MOON_U, "keep_out_mm": KEEP_OUT_MM, "bury_mm": BURY_MM, "step_mm": SCULPT_STEP, "faces": SCULPT_FACES}),
        digest: String::new(),
    };
    Ok((Feature { id: 4, name: "Fenrir's head".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh }, component: joined(HEAD_BLEND_MM) }, stats))
}

/// Where a point falls in a field of flame locks.
#[derive(Clone, Copy, Default)]
struct Flame {
    /// Height 0..1.
    h: f64,
    /// Across the winning lock, -1..1.
    across: f64,
    /// The winning lock's half-width here, in chart units.
    half: f64,
}

/// Flame locks on a chart where `along` runs with the fur and `across` over it: rows `pitch` apart, locks about `length`
/// long and shingled half a lock apart, each bowed in an S of `bow` of its length, widest a third of the way along and
/// drawn out to a point at both ends; `shape` gives a lock's height from its share along, its place across and its
/// half-width. Lengths vary by a quarter and headings by ten degrees; neighbours join by a soft maximum of `soft`.
#[allow(clippy::too_many_arguments)]
fn flames(along: f64, across: f64, pitch: f64, length: f64, bow: f64, soft: f64, seed: i64, shape: &dyn Fn(f64, f64, f64) -> f64) -> Flame {
    let step = length * 0.45;
    let row = (across / pitch).round() as i64;
    let mut out = Flame::default();
    let mut acc = 0.0f64;
    for j in row - 1..=row + 1 {
        let shift = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 * step };
        let i0 = ((along - shift) / step).floor() as i64;
        for i in i0 - 3..=i0 {
            let len_i = length * (0.75 + 0.5 * skin::hash(i * 7 + seed, j * 13 - seed));
            let tilt = ((skin::hash(j * 5 + seed, i * 11 + 3 * seed) - 0.5) * 20.0).to_radians();
            let (ds, dc) = (along - (i as f64 * step + shift), across - j as f64 * pitch);
            let (sn, cs) = tilt.sin_cos();
            let (sl, cl) = (ds * cs + dc * sn, -ds * sn + dc * cs);
            let t = sl / len_i;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let centre = bow * len_i * (2.0 * PI * t).sin() * (PI * t).sin();
            let half = 0.6 * pitch * (PI * t.powf(0.6)).sin().max(0.0).powf(0.8);
            let y = cl - centre;
            if y.abs() >= half {
                continue;
            }
            let q = y / half;
            let h = shape(t, q, half);
            if h > out.h {
                out = Flame { h, across: q, half };
            }
            acc = if soft > 0.0 { smax(acc, h, soft) } else { acc.max(h) };
        }
    }
    if soft > 0.0 {
        out.h = acc;
    }
    out
}

/// A painted lock: rounded across, rising toward its tip so each point lies over the root of the lock beyond it.
fn painted_lock(t: f64, q: f64, _half: f64) -> f64 {
    (q * PI * 0.5).cos().max(0.0).powf(1.1) * (0.3 + 0.7 * smooth(0.0, 0.8, t)) * (1.0 - smooth(0.9, 1.0, t))
}

/// A sculpted lock: its section a smooth hump, faded in at the root and out over the last quarter, and lowered where it
/// narrows so no point stands up as a blade.
fn sculpted_lock(pitch: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |t, q, half| {
        let across = (q * PI * 0.5).cos().max(0.0).powi(2);
        let rise = (0.3 + 0.7 * smooth(0.0, 0.7, t)) * smooth(0.0, 0.15, t) * (1.0 - smooth(0.7, 1.0, t));
        across * rise * (half / (0.45 * pitch)).min(1.0).powi(2)
    }
}

/// The painted pelt at hide point (`along` from the head, `across`): height 0..1, and the hair line 0..1 running along
/// its lock, 0.22 mm apart where the lock is wide and fading out where the lines would crowd under 0.15 mm.
fn fur(along: f64, across: f64) -> (f64, f64) {
    let f = flames(along, across, 1.3, 4.6, 0.27, 0.0, 17, &painted_lock);
    let lines = 0.5 - 0.5 * (2.0 * PI * f.across * f.half / 0.22).cos();
    let hair = lines * smooth(0.3, 0.6, f.h) * smooth(0.45, 0.65, f.half);
    (0.1 + 0.9 * f.h, hair)
}

/// The Gleipnir cord tile: parallel strands laid in an S across the band, each rounded across by stacked strokes and
/// dipping where it turns under at the band's edges, drawn in metal millimetres and squeezed by the chart's own squash.
fn gleipnir_svg(cell_w: f64, cell_h: f64, squash: f64) -> String {
    let (w, h) = (cell_w, cell_h * squash);
    let strand = 1.1;
    let per_tile = 1;
    let lay = 1.1 * h;
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cell_w:.4} {cell_h:.4}" width="{cell_w:.4}" height="{cell_h:.4}"><rect width="{cell_w:.4}" height="{cell_h:.4}" fill="#fff"/><defs>"##
    );
    let tiers = [(1.0, 0x80, 0x58), (0.76, 0x66, 0x3a), (0.52, 0x4c, 0x1e), (0.28, 0x34, 0x00)];
    for (m, (_, end, mid)) in tiers.iter().enumerate() {
        s += &format!(
            r##"<linearGradient id="t{m}" gradientUnits="userSpaceOnUse" x1="0" y1="{:.4}" x2="0" y2="{:.4}"><stop offset="0" stop-color="#{e:02x}{e:02x}{e:02x}"/><stop offset="0.5" stop-color="#{c:02x}{c:02x}{c:02x}"/><stop offset="1" stop-color="#{e:02x}{e:02x}{e:02x}"/></linearGradient>"##,
            0.06 * h,
            0.94 * h,
            e = end,
            c = mid
        );
    }
    s += &format!(r##"</defs><g transform="scale(1 {:.6})" fill="none" stroke-linecap="round">"##, 1.0 / squash.max(0.05));
    let pitch = w / per_tile as f64;
    for k in -4i64..=(per_tile as i64 + 2) {
        let u0 = k as f64 * pitch;
        let (a, b) = ((u0, 0.07 * h), (u0 + lay, 0.93 * h));
        let path = format!(
            "M{:.4} {:.4} C{:.4} {:.4} {:.4} {:.4} {:.4} {:.4}",
            a.0,
            a.1,
            a.0 + 0.55 * lay,
            a.1 - 0.05 * h,
            b.0 - 0.55 * lay,
            b.1 + 0.05 * h,
            b.0,
            b.1
        );
        for (m, (share, _, _)) in tiers.iter().enumerate() {
            s += &format!(r##"<path d="{path}" stroke="url(#t{m})" stroke-width="{:.4}"/>"##, strand * share);
        }
    }
    s += "</g></svg>";
    s
}

/// The fetter's binding where it meets the fur: turns of cord wrapped across the band.
fn binding_svg(w: f64, h: f64, squash: f64) -> String {
    let cord = 0.62;
    let gap = 0.3;
    let turns = ((w + gap) / (cord + gap)).floor() as usize;
    let used = turns as f64 * (cord + gap) - gap;
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.4} {h:.4}" width="{w:.4}" height="{h:.4}"><rect width="{w:.4}" height="{h:.4}" fill="#fff"/><defs><linearGradient id="c" gradientUnits="objectBoundingBox" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#555"/><stop offset="0.5" stop-color="#000"/><stop offset="1" stop-color="#555"/></linearGradient></defs>"##
    );
    let slant = 0.35 * cord;
    for k in 0..turns {
        let x = 0.5 * (w - used) + k as f64 * (cord + gap);
        let (top, bottom) = (0.06 * h, 0.94 * h);
        s += &format!(
            r##"<path d="M{:.4} {top:.4} L{:.4} {top:.4} L{:.4} {bottom:.4} L{:.4} {bottom:.4} Z" fill="url(#c)"/>"##,
            x + slant,
            x + cord + slant,
            x + cord - slant,
            x - slant
        );
    }
    let _ = squash;
    s += "</svg>";
    s
}

/// Per atlas sample, the most relief the stock's surface takes along its normals before the offset folds: half the
/// radius of its tightest concave bend, from the atlas's own normals.
fn fold_room(a: &Atlas) -> Vec<f64> {
    let (w, h) = (a.width, a.height);
    (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if y == 0 || y + 1 >= h {
                return f64::MAX;
            }
            let s = a.at(x, y);
            let mut room = f64::MAX;
            for (p, q) in [(a.at(x + w - 1, y), a.at(x + 1, y)), (a.at(x, y - 1), a.at(x, y + 1))] {
                if dot(p.n, p.n) < 0.5 || dot(q.n, q.n) < 0.5 {
                    continue;
                }
                let dp = sub(q.p, p.p);
                let dn = sub(q.n, p.n);
                let k = dot(dn, dp) / dot(dp, dp).max(1e-12);
                if k < 0.0 {
                    room = room.min(0.5 / -k);
                }
            }
            let _ = s;
            room
        })
        .collect()
}

/// How much relief the stock takes at `p` away from the facet folds `caps`, 0..1.
fn off_folds(p: P3, caps: &[P3]) -> f64 {
    caps.iter().fold(1.0, |m, c| m * smooth(0.5, 1.3, len(sub(p, *c))))
}

/// Where the painted stack alone crosses the stock's own mesh, at the draft and the export resolution.
fn fold_sites(d: &RingDesign, lib: &AlphaLibrary) -> Result<Vec<P3>> {
    let mut part = d.clone();
    part.cad = None;
    let mut out = Vec::new();
    for theta_steps in [768, EXPORT_THETA] {
        let params = BuildParams { theta_steps, profile_steps: if theta_steps == 768 { 320 } else { 448 }, refine: None, ..Default::default() };
        out.extend(crossing_sites(&mesh::try_build(&part, lib, params)?.mesh));
    }
    Ok(out)
}

/// A painted alpha stored as portable 16-bit PNG and shown as one tile over the chart.
fn portable(d: &mut RingDesign, lib: &mut AlphaLibrary, mut alpha: Alpha, height: f64, win: Window, bench: bool) -> Result<()> {
    let name = alpha.name.clone();
    let levels = if bench { 8.0 } else { 255.0 };
    for v in &mut alpha.data {
        *v = (*v * levels).round() / levels;
    }
    lib.insert(Alpha::from_png16(&name, &alpha.to_png16()?)?);
    let mut layer = skin::hide_layer(d, &name, height, win);
    layer.bench_only = bench;
    if bench {
        layer.blend = Blend::Subtract;
    }
    d.layers.layers.push(layer);
    Ok(())
}

fn window(centre: f64, span: f64) -> Window {
    let mut w = Window::around(centre, span);
    w.fade_deg = 6.0;
    w
}

fn author(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Value, Wolf)> {
    let mut d = base()?;
    d.build = params;
    let a = Atlas::of(&d, AW, atlas_rows(&d))?;
    let hide = Hide::of(&a);
    let wolf = wolf_of(&d, &a)?;
    stone_and_fangs(&mut d)?;
    let (head, head_stats) = head_feature(&wolf)?;
    d.cad.as_mut().unwrap().append(head)?;
    let room = fold_room(&a);
    let paint = |d: &mut RingDesign, caps: &[P3]| -> Result<AlphaLibrary> {
        let mut lib = AlphaLibrary::builtin();
        d.layers.layers.retain(|e| e.name != "Ruff" && e.name != "Graver's hair lines");
        let clear = |s: &Sample| {
            let r = s.p[0].hypot(s.p[1]);
            let q = wolf.face(s.p);
            let mouth = smooth(7.0, 8.2, q[0].hypot(q[1] - MOON_U));
            smooth(a.bore + 1.0, a.bore + 1.5, r) * smooth(0.15, 0.8, wolf.sdf(s.p)) * off_folds(s.p, caps) * mouth
        };
        let ruff = a.paint("Ruff", |s| {
            let h = hide.at(s);
            let (lock, _) = fur(h.along.abs(), h.across);
            (lock * clear(s)).min(room[s.i] / RUFF_MM)
        });
        portable(d, &mut lib, ruff, RUFF_MM, window(90.0, 262.0), false)?;
        let hair = a.paint("Graver's hair lines", |s| {
            let h = hide.at(s);
            let (_, line) = fur(h.along.abs(), h.across);
            line * clear(s)
        });
        portable(d, &mut lib, hair, HAIR_LINES_MM, window(90.0, 262.0), true)?;
        Ok(lib)
    };
    let mut caps: Vec<P3> = Vec::new();
    let mut lib = paint(&mut d, &caps)?;
    for round in 0..4 {
        let sites = fold_sites(&d, &lib)?;
        if sites.is_empty() {
            break;
        }
        for p in sites {
            if caps.iter().all(|c| len(sub(*c, p)) > 0.6) {
                caps.push(p);
            }
        }
        println!("  fold round {round}: {} caps", caps.len());
        lib = paint(&mut d, &caps)?;
    }
    // Gleipnir at the palm: its outer face in the chart, and how the chart squeezes it there.
    let x = (270.0 / 360.0 * AW as f64).round() as usize % AW;
    let face: Vec<&Sample> = (0..a.height).map(|y| a.at(x, y)).filter(|s| {
        let r = s.p[0].hypot(s.p[1]).max(1e-9);
        (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / r > 0.8
    }).collect();
    ensure!(face.len() > 4, "The palm has no outer face");
    let (v0, v1) = (face.first().unwrap().v, face.last().unwrap().v);
    let metal: f64 = face.windows(2).map(|w| len(sub(w[1].p, w[0].p))).sum();
    let squash = metal / (v1 - v0).max(1e-6);
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Gleipnir", &ctx);
    t.v_center_mm = 0.5 * (v0 + v1);
    t.v_span_mm = (v1 - v0) * 0.96;
    t.repeats_around = (ctx.circumference_mm / 2.7).round() as u32;
    t.height_mm = GLEIPNIR_MM;
    t.feather_mm = 0.25;
    t.continuous = true;
    let (cw, ch) = t.cell_size(&ctx);
    d.svgs.push(SvgAlpha { name: "Gleipnir".into(), svg: gleipnir_svg(cw, ch, squash), invert: false });
    let mut e = LayerEntry::new("Gleipnir", Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window = window(270.0, 68.0);
    d.layers.layers.push(e);
    let (bw, bh) = (3.2, (v1 - v0) * 0.98);
    d.svgs.push(SvgAlpha { name: "Gleipnir binding".into(), svg: binding_svg(bw, bh, squash), invert: false });
    let mut knots = DecalLayer { alpha: "Gleipnir binding".into(), decals: Vec::new(), feather_mm: 0.2, invert: false };
    for theta in [236.0, 304.0] {
        knots.decals.push(Decal { theta_deg: theta, v_mm: 0.5 * (v0 + v1), size_mm: bw, rotation_deg: 0.0, height_mm: BINDING_MM, flip: false });
    }
    let mut e = LayerEntry::new("Gleipnir's bindings", Layer::Decals(knots));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    d.bake_all(&mut lib);
    let composition = json!({
        "table_y_mm": a.top,
        "moon_across_mm": -MOON_U,
        "head": head_stats,
        "palm_face_v_mm": [v0, v1],
        "palm_metal_per_chart_mm": squash,
        "gleipnir_cell_mm": [cw, ch],
        "ruff_window_deg": [90.0 - 131.0, 90.0 + 131.0],
        "fold_caps": caps.iter().map(|p| json!({"theta_deg": p[1].atan2(p[0]).to_degrees(), "z_mm": p[2], "r_mm": p[0].hypot(p[1])})).collect::<Vec<_>>(),
    });
    Ok((d, lib, composition, wolf))
}

fn solid(m: &Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// Every made part's self-crossings, as placed.
fn part_crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
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

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// A loose-triangle stone welded and given area-weighted vertex normals, so a cabochon shades as a dome.
fn welded(m: &Mesh) -> Mesh {
    let mut out = Mesh::default();
    let mut index = std::collections::HashMap::new();
    for f in &m.faces {
        let g = f.map(|i| {
            let p = m.vertices[i as usize];
            let key = [p.0, p.1, p.2].map(|c| (c * 1e4).round() as i64);
            *index.entry(key).or_insert_with(|| {
                out.vertices.push(p);
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    let mut n = vec![[0.0f64; 3]; out.vertices.len()];
    for f in &out.faces {
        let [a, b, c] = f.map(|i| { let p = out.vertices[i as usize]; [p.0 as f64, p.1 as f64, p.2 as f64] });
        let fn_ = cross(sub(b, a), sub(c, a));
        for &i in f {
            n[i as usize] = add(n[i as usize], fn_);
        }
    }
    out.normals = n.iter().map(|v| { let l = len(*v).max(1e-12); Vec3((v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32) }).collect();
    out
}

/// Studio-gold renders with the moon set.
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, params: BuildParams, edge: usize) -> Result<()> {
    let gems: Vec<(Mesh, [f32; 3])> = ringdesign_core::gems::built_meshes(d, lib, built).into_iter().map(|(m, t)| (welded(&m), t)).collect();
    let mut parts = vec![Part::metal(&built.mesh, render::GOLD)];
    parts.extend(gems.iter().map(|(m, tint)| {
        let mut p = Part::tinted_stone(m, *tint);
        p.smooth = true;
        p
    }));
    for (name, yaw, pitch) in [
        ("hero", 0.55, 0.95),
        ("face", 0.0, PI * 0.5),
        ("palm", PI, 1.05),
        ("side", 0.0, 0.0),
        ("shoulder", -0.9, 0.62),
        ("reverse", 1.6, 0.8),
        ("stones", 0.2, 1.25),
    ] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, 0.55, 0.95, 300)?;
    let mut bare = base()?;
    bare.name = d.name.clone();
    let b = mesh::try_build(&bare, lib, params)?;
    let left = render::render_parts_ss(&[Part::metal(&b.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &left, &right, edge)
}

fn write(out: &Path, draft: bool, verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let (theta_steps, profile_steps) = if draft { (768, 320) } else { (EXPORT_THETA, 448) };
    let params = BuildParams { theta_steps, profile_steps, refine: None, ..Default::default() };
    let start = Instant::now();
    let (d, lib, composition, wolf) = author(params)?;
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = start.elapsed().as_secs_f64();
    println!("  built {} triangles in {build_s:.1} s; notes {:?} {:?}", built.mesh.faces.len(), built.solids.notes, built.parts.notes);
    let cross = csg::self_crossings(&solid(&built.mesh));
    let made = part_crossings(&built);
    let bore = d.inner_radius_mm();
    let inner = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    for p in built.mesh.vertices.iter().filter(|p| ((p.0 as f64).hypot(p.1 as f64)) < bore - 0.01).take(6) {
        println!("    inside the bore: theta {:.1}, z {:.2}, r {:.3}", (p.1 as f64).atan2(p.0 as f64).to_degrees(), p.2, (p.0 as f64).hypot(p.1 as f64));
    }
    if cross > 0 {
        for p in crossing_sites(&built.mesh).iter().take(6) {
            println!("    crossing at theta {:.1}, z {:.2}, r {:.3}", p[1].atan2(p[0]).to_degrees(), p[2], p[0].hypot(p[1]));
        }
    }
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stone_report = ringdesign_core::stones::report_built(&d, 0.0, &built);
    let stone_count = stone_report.as_ref().map_or(0, |r| r.stone_count as usize);
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let previewed = gems.len();
    let warnings: Vec<String> = stone_report.iter().flat_map(|r| r.seats.iter().flat_map(|s| s.warnings.iter())).cloned().collect();
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let inspection = mf::inspect(&d, &lib, &setup(&d), params)?;
    let pattern = &inspection.prepared.mesh;
    let pattern_cross = csg::self_crossings(&solid(pattern));
    let pattern_validation = pattern.validate();
    let pattern_quality = pattern.quality();
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let design_bytes = text.len();
    let format = serde_json::from_str::<Value>(&text)?.get("format_version").and_then(Value::as_u64).unwrap_or(0);
    let mut cold = Value::Null;
    if verify {
        let t = Instant::now();
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        cold = json!({"identical_vertices_faces_normals": same, "ms": t.elapsed().as_secs_f64() * 1000.0});
    }
    let t_lands = Instant::now();
    let lands = measure_lands(out, &wolf, &built)?;
    println!("  lands and census in {:.1} s; unnamed sub-floor {:?}; head mass min {:.3} mm; longest 60 deg seam run {:.2} mm", t_lands.elapsed().as_secs_f64(), lands.unnamed, lands.head_min, lands.seam_run);
    let gates = [
        ("finished mesh watertight with zero degenerates", built.report.validation.watertight && built.report.quality.degenerate_faces == 0),
        ("finished mesh has zero self crossings", cross == 0),
        ("every made part has zero self crossings", made.iter().all(|(_, n)| *n == 0)),
        ("all solids and parts resolved", built.solids.notes.is_empty() && built.parts.notes.is_empty() && built.solids.stamped == d.stamps.len()),
        ("nothing enters the finger hole", inner >= bore - 0.01),
        ("lost wax verdict and 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && field.thinnest_wall_mm >= 0.8),
        ("zero DFM findings", findings.is_empty()),
        ("one stone in report and preview, no warnings", stone_count == 1 && previewed == 1 && warnings.is_empty()),
        ("investment pattern watertight with zero degenerates and crossings", pattern_validation.watertight && pattern_quality.degenerate_faces == 0 && pattern_cross == 0),
        ("cold reload identical", !verify || cold["identical_vertices_faces_normals"] == true),
        ("lost-wax land widths >= 0.8 mm or named", lands.unnamed.is_empty()),
    ];
    let report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "size": d.size.display(),
        "bore_mm": bore * 2.0,
        "build": {"theta_steps": theta_steps, "profile_steps": profile_steps, "triangles": built.mesh.faces.len(), "seconds": build_s},
        "composition": composition,
        "geometry": {"validation": built.report.validation, "quality": built.report.quality, "self_crossings": cross, "volume_mm3": built.report.volume_mm3, "innermost_radius_mm": inner, "bore_radius_mm": bore},
        "made_parts": made.iter().map(|(n, c)| json!({"name": n, "self_crossings": c})).collect::<Vec<_>>(),
        "solids": {"resolved": built.solids.resolved, "stamped": built.solids.stamped, "notes": built.solids.notes, "parts_notes": built.parts.notes, "joined": built.parts.joined},
        "field": {"verdict": field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "notes": field.notes},
        "dfm": findings.iter().map(|f| json!({"label": f.label, "message": f.message})).collect::<Vec<_>>(),
        "stones": {"reported": stone_count, "previewed": previewed, "warnings": warnings, "carats": stone_report.as_ref().map_or(0.0, |r| r.total_carats)},
        "grams_18k": grams,
        "pattern": {"validation": pattern_validation, "quality": pattern_quality, "self_crossings": pattern_cross, "release": {"obstructions": inspection.release.obstructions.len()}},
        "design": {"bytes": design_bytes, "format_version": format, "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), "stamps": d.stamps.len(), "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "cold_reload": cold,
        "land_widths": {"floor_mm": 0.8, "head": lands.head, "fangs_and_rail": lands.fangs, "head_min_section_mm": lands.head_min, "exceptions": lands.exceptions, "unnamed": lands.unnamed},
        "census": {"edges_60_deg_by_zone": lands.census, "seam_crease_run_mm": lands.seam_run},
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), pattern, "Fenrir / investment pattern")?;
        for (m, _) in &gems {
            stl::write_stl(out.join("reference-moonstone.stl"), m, "Moonstone cabochon 10 mm")?;
        }
        let tint = moonstone().preview_tint.unwrap();
        std::fs::write(
            out.join("stones.json"),
            serde_json::to_vec_pretty(&json!({"stones": [{"mesh": "reference-moonstone.stl", "name": "Moonstone", "tint": tint, "ior": 1.53, "dispersion": 0.012, "roughness": 0.12, "transmission": 0.45}]}))?,
        )?;
    }
    let art = out.join("artwork");
    let _ = std::fs::remove_dir_all(&art);
    std::fs::create_dir_all(&art)?;
    for svg in &d.svgs {
        std::fs::write(art.join(format!("{}.svg", svg.name.to_lowercase().replace([' ', '\''], "-"))), &svg.svg)?;
    }
    for name in ["Ruff", "Graver's hair lines"] {
        if let Some(alpha) = lib.get(name) {
            std::fs::write(art.join(format!("{}.png", name.to_lowercase().replace([' ', '\''], "-"))), alpha.to_png16()?)?;
        }
    }
    renders(out, &d, &lib, &built, params, if draft { 1000 } else { 1600 })?;
    println!(
        "  crossings {cross}; made parts {:?}; innermost {inner:.3} of bore {bore:.3}; field {} wall {:.2} mm; DFM {}; stones {stone_count}/{previewed}; {grams:.1} g 18k; design {} KB at format {format}",
        made.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
        field.verdict.label(),
        field.thinnest_wall_mm,
        findings.len(),
        design_bytes / 1024
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for (g, p) in &gates {
        println!("  {}: {g}", if *p { "pass" } else { "FAIL" });
    }
    let failed: Vec<_> = gates.iter().filter(|g| !g.1).map(|g| g.0).collect();
    ensure!(failed.is_empty(), "Failed gates: {failed:?}");
    Ok(())
}

/// Faces of a closed mesh hashed by cell, for rays.
struct Rays<'a> {
    v: &'a [P3],
    f: &'a [[u32; 3]],
    cell: f64,
    map: std::collections::HashMap<[i32; 3], Vec<u32>>,
}

impl<'a> Rays<'a> {
    fn new(v: &'a [P3], f: &'a [[u32; 3]], cell: f64, keep: impl Fn(P3) -> bool) -> Self {
        let mut map: std::collections::HashMap<[i32; 3], Vec<u32>> = std::collections::HashMap::new();
        for (i, t) in f.iter().enumerate() {
            let p = t.map(|k| v[k as usize]);
            if !p.iter().any(|q| keep(*q)) {
                continue;
            }
            let lo: [i32; 3] = std::array::from_fn(|k| (p.iter().map(|q| q[k]).fold(f64::MAX, f64::min) / cell).floor() as i32);
            let hi: [i32; 3] = std::array::from_fn(|k| (p.iter().map(|q| q[k]).fold(f64::MIN, f64::max) / cell).floor() as i32);
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        map.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        Self { v, f, cell, map }
    }
    /// Distance along unit `d` from `o` to the nearest face beyond `min` and within `max`, faces holding `skip` aside.
    fn hit(&self, o: P3, d: P3, min: f64, max: f64, skip: u32) -> Option<f64> {
        self.hit_face(o, d, min, max, skip).map(|h| h.0)
    }
    /// [`Rays::hit`] with the face it lands on.
    fn hit_face(&self, o: P3, d: P3, min: f64, max: f64, skip: u32) -> Option<(f64, u32)> {
        let mut seen = std::collections::HashSet::new();
        let mut best: Option<(f64, u32)> = None;
        let steps = (max / (self.cell / 3.0)).ceil() as usize;
        for k in 0..=steps {
            let s = k as f64 * self.cell / 3.0;
            if best.is_some_and(|b| b.0 < s - self.cell) {
                break;
            }
            let p = add(o, mul(d, s));
            let key: [i32; 3] = std::array::from_fn(|i| (p[i] / self.cell).floor() as i32);
            if !seen.insert(key) {
                continue;
            }
            let Some(list) = self.map.get(&key) else { continue };
            for &fi in list {
                if self.f[fi as usize].contains(&skip) {
                    continue;
                }
                let [a, b, c] = self.f[fi as usize].map(|x| self.v[x as usize]);
                let (e1, e2) = (sub(b, a), sub(c, a));
                let pv = cross(d, e2);
                let det = dot(e1, pv);
                if det.abs() < 1e-14 {
                    continue;
                }
                let tv = sub(o, a);
                let u = dot(tv, pv) / det;
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let qv = cross(tv, e1);
                let w = dot(d, qv) / det;
                if w < 0.0 || u + w > 1.0 {
                    continue;
                }
                let t = dot(e2, qv) / det;
                if t > min && t < max && best.is_none_or(|b| t < b.0) {
                    best = Some((t, fi));
                }
            }
        }
        best
    }
}

/// Section at a surface point by the median of seven rays into the metal: along `-n` and six at 25 degrees round it.
fn section(rays: &Rays, p: P3, n: P3, skip: u32) -> f64 {
    let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let e1 = cross(n, a);
    let e1 = mul(e1, 1.0 / len(e1));
    let e2 = cross(n, e1);
    let o = add(p, mul(n, -1e-4));
    let (s, c) = 25f64.to_radians().sin_cos();
    let mut hits: Vec<f64> = std::iter::once(mul(n, -1.0))
        .chain((0..6).map(|k| {
            let (ks, kc) = (k as f64 * PI / 3.0).sin_cos();
            add(mul(n, -c), add(mul(e1, s * kc), mul(e2, s * ks)))
        }))
        .map(|d| rays.hit(o, d, 0.01, 4.0, skip).unwrap_or(4.0))
        .collect();
    hits.sort_by(f64::total_cmp);
    hits[3]
}

/// Area-weighted vertex normals of a closed mesh.
fn vertex_normals(v: &[P3], f: &[[u32; 3]]) -> Vec<P3> {
    let mut n = vec![[0.0; 3]; v.len()];
    for t in f {
        let [a, b, c] = t.map(|x| v[x as usize]);
        let fnrm = cross(sub(b, a), sub(c, a));
        for &x in t {
            n[x as usize] = add(n[x as usize], fnrm);
        }
    }
    n.into_iter().map(|x| mul(x, 1.0 / len(x).max(1e-12))).collect()
}

/// One feature's measured section: its thinnest, and how far from its tip the metal stays under the floor.
#[derive(serde::Serialize, Clone)]
struct Land {
    feature: String,
    samples: usize,
    min_section_mm: f64,
    sub_floor_samples: usize,
    /// The furthest a sample under 0.8 mm stands from the feature's tip, mm.
    sub_floor_from_tip_mm: f64,
}

/// The land-width verdict and the crease census of a finished build.
struct Lands {
    head: Vec<Land>,
    fangs: Vec<Land>,
    exceptions: Vec<Value>,
    unnamed: Vec<String>,
    head_min: f64,
    census: Value,
    seam_run: f64,
}

/// What each feature may leave under the 0.8 mm floor from its tip, and what the bench does with it.
fn land_rule(feature: &str) -> Option<(f64, &'static str)> {
    if feature.starts_with("ear") {
        Some((1.0, "ear point: the last 1.0 mm to the tip tapers from a thick root; cast in place and polished"))
    } else if feature.contains("incisor") || feature.contains("premolar") || feature.contains("molar") {
        Some((0.6, "tooth point: the last 0.6 mm of each tooth; cast from a root of 1.0 mm or more and polished"))
    } else if feature.contains("tuft") {
        Some((0.9, "tuft point: the last 0.9 mm of the chin tuft; chased at the bench"))
    } else if feature.contains("Claw") {
        Some((1.6, "fang point: the last 1.6 mm of each fang lies on the moonstone's dome and is notched by it; cast in place, closed onto the dome and polished at the bench"))
    } else if feature.contains("rail") {
        Some((f64::MAX, "base rail: the claw head's rail, notched by the stone where it passes under the girdle; buried in the gums except in the mouth's corners, burnished to the stone at the bench"))
    } else {
        None
    }
}

/// Land widths of the head and fangs by rays into the finished metal, named against [`land_rule`], and the crease census.
fn measure_lands(out: &Path, wolf: &Wolf, built: &mesh::BuildResult) -> Result<Lands> {
    let head_id: ringdesign_core::sketch::Id = 4;
    let (head, thin_at) = head_lands(wolf, built, head_id);
    let fangs = fang_lands(built);
    let mut unnamed: Vec<String> = Vec::new();
    let mut exceptions: Vec<Value> = Vec::new();
    for l in head.iter().chain(&fangs) {
        if l.min_section_mm >= 0.8 {
            continue;
        }
        match land_rule(&l.feature) {
            Some((limit, bench)) if l.sub_floor_from_tip_mm <= limit => exceptions.push(json!({"feature": l.feature, "min_section_mm": l.min_section_mm, "sub_floor_from_tip_mm": l.sub_floor_from_tip_mm, "treatment": bench})),
            _ => unnamed.push(format!("{} {:.2} mm, {:.2} mm from its tip", l.feature, l.min_section_mm, l.sub_floor_from_tip_mm)),
        }
    }
    let head_min = head.iter().filter(|l| land_rule(&l.feature).is_none()).map(|l| l.min_section_mm).fold(f64::MAX, f64::min);
    let (census, crease_at) = census(wolf, &built.mesh);
    let (seam_run, seam_at) = seam_creases(built, head_id);
    if std::env::var("FENRIR_DEBUG").is_ok() {
        debug_views(out, wolf, built, &thin_at, &crease_at, &seam_at)?;
    }
    Ok(Lands { head, fangs, exceptions, unnamed, head_min, census, seam_run })
}

/// The finished mesh's faces within `r` of `c`, as their own mesh.
fn clip_mesh(m: &Mesh, c: P3, r: f64) -> Mesh {
    clip_where(m, c, r, |_| true)
}

/// [`clip_mesh`] keeping only the faces `keep` accepts by index.
fn clip_where(m: &Mesh, c: P3, r: f64, keep: impl Fn(usize) -> bool) -> Mesh {
    let mut out = Mesh::default();
    let mut map = std::collections::HashMap::new();
    for (fi, f) in m.faces.iter().enumerate() {
        let ps = f.map(|k| m.vertices[k as usize]);
        if !keep(fi) || ps.iter().any(|p| len(sub([p.0 as f64, p.1 as f64, p.2 as f64], c)) > r) {
            continue;
        }
        let t = f.map(|k| {
            *map.entry(k).or_insert_with(|| {
                out.vertices.push(m.vertices[k as usize]);
                out.normals.push(m.normals.get(k as usize).copied().unwrap_or(Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(t);
    }
    out
}

/// Renders marking thin samples red, face creases blue and seam creases green, with close views of the thin clusters.
fn debug_views(out: &Path, wolf: &Wolf, built: &mesh::BuildResult, thin_at: &[(P3, P3, String)], crease_at: &[P3], seam_at: &[P3]) -> Result<()> {
    let thin_points: Vec<P3> = thin_at.iter().map(|t| t.0).collect();
    let (thin, creases, seams) = (markers(&thin_points, 0.05), markers(crease_at, 0.04), markers(seam_at, 0.04));
    let mut cells: std::collections::HashMap<[i64; 3], Vec<usize>> = std::collections::HashMap::new();
    for (k, t) in thin_at.iter().enumerate() {
        cells.entry(t.0.map(|c| (c / 1.5).floor() as i64)).or_default().push(k);
    }
    let mut clusters: Vec<Vec<usize>> = cells.into_values().collect();
    clusters.sort_by_key(|c| std::cmp::Reverse(c.len()));
    let mut tiles = Vec::new();
    for (ci, c) in clusters.iter().take(12).enumerate() {
        let centre = mul(c.iter().fold([0.0; 3], |s, k| add(s, thin_at[*k].0)), 1.0 / c.len() as f64);
        let n = c.iter().fold([0.0; 3], |s, k| add(s, thin_at[*k].1));
        let n = mul(n, 1.0 / len(n).max(1e-9));
        let (yaw, pitch) = (n[0].atan2(n[1]), n[2].clamp(-1.0, 1.0).acos());
        let owner = |fi: usize| built.mesh.faces[fi].iter().map(|k| built.mesh.origin.get(*k as usize).and_then(|o| built.parts.feature_of(*o))).fold(None, |a, b| a.or(b));
        let head = clip_where(&built.mesh, centre, 1.6, |fi| owner(fi) == Some(4));
        let other = clip_where(&built.mesh, centre, 1.6, |fi| owner(fi) != Some(4));
        let frame = markers(&[add(centre, [-1.3, -1.3, -1.3]), add(centre, [1.3, 1.3, 1.3])], 0.005);
        let marks = clip_mesh(&thin, centre, 1.6);
        let zoom = vec![Part::metal(&frame, render::GOLD), Part::metal(&head, render::GOLD), Part::metal(&other, [0.55, 0.75, 0.6]), Part::tinted_stone(&marks, [0.9, 0.1, 0.1])];
        let q = wolf.face(centre);
        println!("  thin cluster {ci}: {} samples at face ({:.2}, {:.2}, {:.2}), {}", c.len(), q[0], q[1], q[2], thin_at[c[0]].2);
        let path = out.join(format!("debug-thin-{ci}.png"));
        render::write_png_parts(&path, &zoom, yaw, pitch, 400)?;
        tiles.push(path);
    }
    let parts = vec![Part::metal(&built.mesh, render::GOLD), Part::tinted_stone(&thin, [0.9, 0.1, 0.1]), Part::tinted_stone(&creases, [0.1, 0.3, 0.95]), Part::tinted_stone(&seams, [0.1, 0.85, 0.2])];
    for (name, yaw, pitch) in [("face", 0.0, PI * 0.5), ("hero", 0.55, 0.95), ("side", 0.0, 0.0), ("back", PI, 0.35), ("right", -PI * 0.5, 0.45), ("left", PI * 0.5, 0.45)] {
        render::write_png_parts(out.join(format!("debug-{name}.png")), &parts, yaw, pitch, 1400)?;
    }
    Ok(())
}

/// Sections of the finished ring's head by rays into the metal, gathered by the feature each vertex lies on; seam and
/// crease vertices, whose normals the boolean leaves ragged, are left out.
fn head_lands(wolf: &Wolf, built: &mesh::BuildResult, head_id: ringdesign_core::sketch::Id) -> (Vec<Land>, Vec<(P3, P3, String)>) {
    let m = &built.mesh;
    let v: Vec<P3> = m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect();
    let mine: Vec<usize> = (0..v.len()).filter(|&i| m.origin.get(i).is_some_and(|o| built.parts.feature_of(*o) == Some(head_id))).collect();
    if mine.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let (lo, hi) = mine.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), &i| (std::array::from_fn(|k| lo[k].min(v[i][k] - 4.5)), std::array::from_fn(|k| hi[k].max(v[i][k] + 4.5))));
    let rays = Rays::new(&v, &m.faces, 0.2, |p| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]));
    let normals = vertex_normals(&v, &m.faces);
    let mut incident: Vec<Vec<u32>> = vec![Vec::new(); v.len()];
    for (fi, t) in m.faces.iter().enumerate() {
        for &x in t {
            incident[x as usize].push(fi as u32);
        }
    }
    let smooth_here = |i: usize| {
        incident[i].iter().all(|fi| {
            let [a, b, c] = m.faces[*fi as usize].map(|x| v[x as usize]);
            let n = cross(sub(b, a), sub(c, a));
            dot(n, normals[i]) > 0.5 * len(n)
        })
    };
    let owner = |fi: u32| m.faces[fi as usize].iter().map(|k| m.origin.get(*k as usize).and_then(|o| built.parts.feature_of(*o))).fold(None, |a, b| a.or(b));
    // A head sample whose metal runs straight on into another part is that part's section, measured with it.
    let own_metal = |i: usize| match rays.hit_face(add(v[i], mul(normals[i], -1e-4)), mul(normals[i], -1.0), 0.01, 1.0, i as u32) {
        Some((_, fi)) => owner(fi) == Some(head_id),
        None => true,
    };
    let rows: Vec<(String, f64, f64, P3, usize)> = mine
        .par_iter()
        .filter(|&&i| smooth_here(i) && own_metal(i))
        .map(|&i| {
            let t = section(&rays, v[i], normals[i], i as u32);
            let (name, tip) = wolf.label(wolf.face(v[i]));
            (name, t, len(sub(wolf.face(v[i]), tip)), wolf.face(v[i]), i)
        })
        .collect();
    let thin_at: Vec<(P3, P3, String)> = rows.iter().filter(|r| r.1 < 0.8).map(|r| (v[r.4], normals[r.4], r.0.clone())).collect();
    if std::env::var("FENRIR_OWNERS").is_ok() {
        let mut tally: std::collections::BTreeMap<(String, String), usize> = std::collections::BTreeMap::new();
        for r in rows.iter().filter(|r| r.1 < 0.8) {
            let (p, n) = (v[r.4], normals[r.4]);
            let hit = rays.hit_face(add(p, mul(n, -1e-4)), mul(n, -1.0), 0.01, 4.0, r.4 as u32);
            let what = match hit {
                Some((t, fi)) => format!("{:?} at {:.1}", owner(fi), (t * 10.0).round() / 10.0),
                None => "nothing".into(),
            };
            *tally.entry((r.0.clone(), what)).or_default() += 1;
        }
        for ((who, what), n) in tally {
            println!("    thin {who}: straight ray lands on {what} x{n}");
        }
    }
    if std::env::var("FENRIR_RAYS").is_ok() {
        for r in rows.iter().filter(|r| r.1 < 0.8 && r.0.starts_with("ear") && r.2 > 1.2).take(6) {
            let (p, n) = (v[r.4], normals[r.4]);
            let a = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
            let e1 = cross(n, a);
            let e1 = mul(e1, 1.0 / len(e1));
            let e2 = cross(n, e1);
            let o = add(p, mul(n, -1e-4));
            let (s, c) = 25f64.to_radians().sin_cos();
            let dists: Vec<String> = std::iter::once(mul(n, -1.0))
                .chain((0..6).map(|k| {
                    let (ks, kc) = (k as f64 * PI / 3.0).sin_cos();
                    add(mul(n, -c), add(mul(e1, s * kc), mul(e2, s * ks)))
                }))
                .map(|dir| {
                    let hit = rays.hit(o, dir, 0.01, 4.0, r.4 as u32);
                    let field_exit = (1..200).map(|k| k as f64 * 0.02).find(|t| wolf.sdf(add(o, mul(dir, *t))) > 0.0);
                    format!("{:?}/{:?}", hit.map(|h| (h * 100.0).round() / 100.0), field_exit.map(|h| (h * 100.0).round() / 100.0))
                })
                .collect();
            let incident: Vec<usize> = m.faces.iter().enumerate().filter(|(_, t)| t.contains(&(r.4 as u32))).map(|(i, _)| i).collect();
            println!("    {} at face {:?}, {:.2} from tip, sdf {:.3}, {} faces round it: rays mesh/field {}", r.0, wolf.face(p).map(|c| (c * 100.0).round() / 100.0), r.2, wolf.sdf(p), incident.len(), dists.join(" "));
        }
    }
    for who in ["head mass", "ear, right"] {
        let mut thin: Vec<[i64; 3]> = rows.iter().filter(|r| r.0 == who && r.1 < 0.8).map(|r| r.3.map(|c| (c * 2.0).round() as i64)).collect();
        thin.sort_unstable();
        thin.dedup();
        if !thin.is_empty() {
            println!("  thin {who} near (face mm): {:?}", thin.iter().step_by((thin.len() / 16).max(1)).take(16).map(|c| c.map(|x| x as f64 / 2.0)).collect::<Vec<_>>());
        }
    }
    let mut out: Vec<Land> = Vec::new();
    for (name, t, from_tip, _, _) in rows {
        let k = match out.iter().position(|l| l.feature == name) {
            Some(k) => k,
            None => {
                out.push(Land { feature: name.clone(), samples: 0, min_section_mm: f64::MAX, sub_floor_samples: 0, sub_floor_from_tip_mm: 0.0 });
                out.len() - 1
            }
        };
        let l = &mut out[k];
        l.samples += 1;
        l.min_section_mm = l.min_section_mm.min(t);
        if t < 0.8 {
            l.sub_floor_samples += 1;
            l.sub_floor_from_tip_mm = l.sub_floor_from_tip_mm.max(from_tip);
        }
    }
    out.sort_by(|a, b| a.feature.cmp(&b.feature));
    (out, thin_at)
}

/// Each claw's point in world millimetres: its vertex furthest up the ring's radius.
fn fang_tips(built: &mesh::BuildResult) -> Vec<P3> {
    let mut out = Vec::new();
    for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
        let Some(made) = &c.made else { continue };
        if !made.key.contains("claw") {
            continue;
        }
        let s = made.solid();
        for (pi, name) in made.named.names.iter().enumerate() {
            if !name.starts_with("Claw") {
                continue;
            }
            let tip = s.f.iter().zip(&made.named.patch).filter(|(_, p)| **p as usize == pi).flat_map(|(t, _)| t.map(|x| s.v[x as usize])).fold([0.0, f64::MIN, 0.0], |b, p| if p[1] > b[1] { p } else { b });
            out.push(tip);
        }
    }
    out
}

/// Sections of each claw and rail of the fangs' made head, by patch, the tip taken as the claw's highest point.
fn fang_lands(built: &mesh::BuildResult) -> Vec<Land> {
    let mut out = Vec::new();
    for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
        let Some(made) = &c.made else { continue };
        if !made.key.contains("claw") {
            continue;
        }
        let s = made.solid();
        let normals = vertex_normals(&s.v, &s.f);
        let rays = Rays::new(&s.v, &s.f, 0.2, |_| true);
        for (pi, name) in made.named.names.iter().enumerate() {
            let mut verts: Vec<usize> = s.f.iter().zip(&made.named.patch).filter(|(_, p)| **p as usize == pi).flat_map(|(t, _)| t.map(|x| x as usize)).collect();
            verts.sort_unstable();
            verts.dedup();
            if verts.is_empty() {
                continue;
            }
            let tip = verts.iter().map(|&i| s.v[i]).fold([0.0, f64::MIN, 0.0], |b, p| if p[1] > b[1] { p } else { b });
            let mut land = Land { feature: format!("{}: {name}", c.name), samples: 0, min_section_mm: f64::MAX, sub_floor_samples: 0, sub_floor_from_tip_mm: 0.0 };
            for &i in &verts {
                let t = section(&rays, s.v[i], normals[i], i as u32);
                land.samples += 1;
                land.min_section_mm = land.min_section_mm.min(t);
                if t < 0.8 {
                    land.sub_floor_samples += 1;
                    land.sub_floor_from_tip_mm = land.sub_floor_from_tip_mm.max(len(sub(s.v[i], tip)));
                }
            }
            out.push(land);
        }
    }
    out
}

/// Dihedral census of the finished mesh by zone: edges at or over 60 and 90 degrees, their length, and 1 mm cells.
fn census(wolf: &Wolf, m: &Mesh) -> (Value, Vec<P3>) {
    use std::collections::{HashMap, HashSet};
    let v: Vec<P3> = m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect();
    let fnrm: Vec<P3> = m
        .faces
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|x| v[x as usize]);
            let n = cross(sub(b, a), sub(c, a));
            mul(n, 1.0 / len(n).max(1e-30))
        })
        .collect();
    let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (i, t) in m.faces.iter().enumerate() {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push(i as u32);
        }
    }
    let ear_tips = [[5.1, 11.0, 2.2], [-5.1, 11.0, 2.2]];
    let mut face_marks: Vec<P3> = Vec::new();
    let mut zones: HashMap<&str, (usize, f64, usize, HashSet<[i64; 3]>)> = HashMap::new();
    for ((a, b), fs) in &edges {
        if fs.len() != 2 {
            continue;
        }
        let ang = dot(fnrm[fs[0] as usize], fnrm[fs[1] as usize]).clamp(-1.0, 1.0).acos().to_degrees();
        if ang < 60.0 {
            continue;
        }
        let mid = mul(add(v[*a as usize], v[*b as usize]), 0.5);
        let q = wolf.face(mid);
        let rho = q[0].hypot(q[1] - MOON_U);
        let zone = if q[2] > 0.3 && rho > 7.0 && q[1] < 7.0 && q[0].abs() < 9.0 {
            "cheeks, brow and muzzle"
        } else if q[2] > 0.3 && q[1] >= 7.0 {
            if ear_tips.iter().any(|t| len(sub(q, *t)) < 1.0) { "ear tips" } else { "ears and crown" }
        } else if q[2] > 0.0 && rho <= 7.0 {
            "mouth, teeth, fangs and rail"
        } else {
            "elsewhere"
        };
        if zone == "cheeks, brow and muzzle" || zone == "ears and crown" {
            face_marks.push(mid);
        }
        let z = zones.entry(zone).or_insert((0, 0.0, 0, HashSet::new()));
        z.0 += 1;
        z.1 += len(sub(v[*a as usize], v[*b as usize]));
        if ang >= 90.0 {
            z.2 += 1;
        }
        z.3.insert(std::array::from_fn(|k| mid[k].floor() as i64));
    }
    if let Some(z) = zones.get("cheeks, brow and muzzle") {
        let mut cells: Vec<[i64; 3]> = z.3.iter().map(|c| wolf.face([c[0] as f64 + 0.5, c[1] as f64 + 0.5, c[2] as f64 + 0.5]).map(|x| x.round() as i64)).collect();
        cells.sort_unstable();
        println!("  60 deg face-zone cells (face mm): {:?}", cells.iter().step_by((cells.len() / 24).max(1)).collect::<Vec<_>>());
    }
    let mut out = serde_json::Map::new();
    for (zone, (n, l, n90, cells)) in zones {
        out.insert(zone.into(), json!({"edges_60": n, "length_mm": l, "edges_90": n90, "cells_1mm": cells.len()}));
    }
    (Value::Object(out), face_marks)
}

/// The longest run of seam edges turning 60 degrees or more where the head meets the stock, mm.
fn seam_creases(built: &mesh::BuildResult, head_id: ringdesign_core::sketch::Id) -> (f64, Vec<P3>) {
    use std::collections::HashMap;
    let m = &built.mesh;
    let v: Vec<P3> = m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect();
    // 0 the band, 1 the head, 2 any other part.
    let class = |i: u32| match m.origin.get(i as usize).map(|o| built.parts.feature_of(*o)) {
        Some(Some(id)) if id == head_id => 1,
        Some(Some(_)) => 2,
        _ => 0,
    };
    let fnrm: Vec<P3> = m
        .faces
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|x| v[x as usize]);
            let n = cross(sub(b, a), sub(c, a));
            mul(n, 1.0 / len(n).max(1e-30))
        })
        .collect();
    let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (i, t) in m.faces.iter().enumerate() {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push(i as u32);
        }
    }
    let mut graph: HashMap<u32, Vec<(u32, f64)>> = HashMap::new();
    for ((a, b), fs) in &edges {
        if fs.len() != 2 {
            continue;
        }
        let mixed = |f: u32| {
            let cs = m.faces[f as usize].map(class);
            cs.contains(&1) && cs.contains(&0) && !cs.contains(&2)
        };
        if !(mixed(fs[0]) || mixed(fs[1])) {
            continue;
        }
        let ang = dot(fnrm[fs[0] as usize], fnrm[fs[1] as usize]).clamp(-1.0, 1.0).acos().to_degrees();
        if ang >= 60.0 {
            let l = len(sub(v[*a as usize], v[*b as usize]));
            graph.entry(*a).or_default().push((*b, l));
            graph.entry(*b).or_default().push((*a, l));
        }
    }
    let marks: Vec<P3> = graph.keys().map(|k| v[*k as usize]).collect();
    let mut seen = std::collections::HashSet::new();
    let mut longest: f64 = 0.0;
    for &start in graph.keys() {
        if !seen.insert(start) {
            continue;
        }
        let (mut stack, mut total) = (vec![start], 0.0);
        while let Some(x) = stack.pop() {
            for &(y, l) in &graph[&x] {
                total += 0.5 * l;
                if seen.insert(y) {
                    stack.push(y);
                }
            }
        }
        longest = longest.max(total);
    }
    (longest, marks)
}

/// Small octahedra at `points`, `r` across, for marking places on a render.
fn markers(points: &[P3], r: f64) -> Mesh {
    let dirs: [P3; 6] = [[r, 0.0, 0.0], [-r, 0.0, 0.0], [0.0, r, 0.0], [0.0, -r, 0.0], [0.0, 0.0, r], [0.0, 0.0, -r]];
    let tris = [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];
    let mut m = Mesh::default();
    for p in points {
        for t in tris {
            let base = m.vertices.len() as u32;
            let c = t.map(|k| add(*p, dirs[k]));
            let n = cross(sub(c[1], c[0]), sub(c[2], c[0]));
            let n = mul(n, 1.0 / len(n).max(1e-12));
            for q in c {
                m.vertices.push(Vec3(q[0] as f32, q[1] as f32, q[2] as f32));
                m.normals.push(Vec3(n[0] as f32, n[1] as f32, n[2] as f32));
            }
            m.faces.push([base, base + 1, base + 2]);
        }
    }
    m
}

/// Points where one face of `mesh` pierces another.
fn crossing_sites(mesh: &Mesh) -> Vec<P3> {
    use std::collections::HashMap;
    let s = solid(mesh);
    let side = |a: P3, b: P3, c: P3, d: P3| {
        let xyz = |p: P3| robust::Coord3D { x: p[0], y: p[1], z: p[2] };
        robust::orient3d(xyz(a), xyz(b), xyz(c), xyz(d))
    };
    let pierce = |p: P3, q: P3, tri: [P3; 3]| {
        let [a, b, c] = tri;
        let (sp, sq) = (side(a, b, c, p), side(a, b, c, q));
        if sp == 0.0 || sq == 0.0 || (sp > 0.0) == (sq > 0.0) {
            return None;
        }
        let signs = [side(p, q, a, b), side(p, q, b, c), side(p, q, c, a)];
        if signs.iter().all(|v| *v > 0.0) || signs.iter().all(|v| *v < 0.0) {
            let t = sp / (sp - sq);
            Some(std::array::from_fn(|k| p[k] + t * (q[k] - p[k])))
        } else {
            None
        }
    };
    let boxes: Vec<([i32; 3], [i32; 3])> = s
        .f
        .iter()
        .map(|f| {
            let lo: P3 = std::array::from_fn(|k| f.iter().map(|i| s.v[*i as usize][k]).fold(f64::MAX, f64::min));
            let hi: P3 = std::array::from_fn(|k| f.iter().map(|i| s.v[*i as usize][k]).fold(f64::MIN, f64::max));
            (lo.map(|v| (v / 0.3).floor() as i32), hi.map(|v| (v / 0.3).floor() as i32))
        })
        .collect();
    let mut cells: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    cells.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let mut stamp = vec![usize::MAX; s.f.len()];
    let mut points = Vec::new();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        let a = s.f[i].map(|k| s.v[k as usize]);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for &j in &cells[&[x, y, z]] {
                        if j <= i || stamp[j] == i || s.f[i].iter().any(|v| s.f[j].contains(v)) {
                            continue;
                        }
                        stamp[j] = i;
                        let b = s.f[j].map(|k| s.v[k as usize]);
                        if let Some(p) = (0..3).find_map(|k| pierce(a[k], a[(k + 1) % 3], b)).or_else(|| (0..3).find_map(|k| pierce(b[k], b[(k + 1) % 3], a))) {
                            points.push(p);
                        }
                    }
                }
            }
        }
    }
    points
}

/// Crossings as each layer joins the bare stock, without the parts.
fn probe(theta_steps: usize) -> Result<()> {
    let params = BuildParams { theta_steps, profile_steps: 320, refine: None, ..Default::default() };
    let (d, lib, _, _) = author(params)?;
    let mut part = d.clone();
    part.cad = None;
    part.layers.layers.clear();
    for step in 0..=d.layers.layers.len() {
        if step > 0 {
            part.layers.layers.push(d.layers.layers[step - 1].clone());
        }
        let built = mesh::try_build(&part, &lib, params)?;
        let sites = crossing_sites(&built.mesh);
        let name = if step == 0 { "bare stock".to_string() } else { d.layers.layers[step - 1].name.clone() };
        println!("  {name}: {} crossings {:?}", sites.len(), sites.iter().take(3).map(|p| (p[1].atan2(p[0]).to_degrees().round(), (p[2] * 100.0).round() / 100.0, (p[0].hypot(p[1]) * 100.0).round() / 100.0)).collect::<Vec<_>>());
    }
    Ok(())
}

/// The export build's steps round the ring.
const EXPORT_THETA: usize = 1536;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let out = args.iter().find(|s| !s.starts_with("--")).map_or("showcase/bestiarium/fenrir", String::as_str);
    if args.iter().any(|a| a == "--probe") {
        return probe(if args.iter().any(|a| a == "--draft") { 768 } else { EXPORT_THETA });
    }
    if args.iter().any(|a| a == "--map") {
        let d = base()?;
        let a = Atlas::of(&d, AW, atlas_rows(&d))?;
        let relief = Relief::of(&a, a.top);
        println!("table {:.3} bore {:.3}; stock height over the table, x across -12..12, u from 12 down to -14", a.top, d.inner_radius_mm());
        for j in (-14..=12).rev() {
            let row: Vec<String> = (-12..=12).map(|i| { let h = relief.at(i as f64, j as f64); if h < -9.0 { "   .".into() } else { format!("{h:4.1}") } }).collect();
            println!("u {j:3}: {}", row.join(""));
        }
        let gem = moonstone();
        println!("girdle over the table {:.3}, crown {:.3}, pavilion {:.3}", builders::stand_off_mm(builders::CLAW, gem), gem.crown_mm(), gem.pavilion_mm());
        return Ok(());
    }
    if args.iter().any(|a| a == "--sculpt") {
        return sculpt_preview(Path::new(out), SCULPT_STEP);
    }
    write(Path::new(out), args.iter().any(|a| a == "--draft"), args.iter().any(|a| a == "--verify"))
}
