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
/// No sculpt within this radius of the moon's axis, so the stone drops into its claws.
const KEEP_OUT_MM: f64 = 5.06;

/// The wolf's head as a signed distance field, in face coordinates: `x` across, `u` toward the ears, `h` out of the table.
struct Wolf {
    table: f64,
    bore: f64,
    relief: Relief,
    stock: Stock,
}

impl Wolf {
    fn face(&self, p: P3) -> P3 {
        [p[0], -p[2], p[1] - self.table]
    }
    /// A point at radius `rho` from the moon's axis, `deg` from +x toward +u, at height `h`.
    fn rim(rho: f64, deg: f64, h: f64) -> P3 {
        let (s, c) = deg.to_radians().sin_cos();
        [rho * c, MOON_U + rho * s, h]
    }
    /// The surface point under face point (`x`, `u`), raised `lift`.
    fn on(&self, x: f64, u: f64, lift: f64) -> P3 {
        [x, u, self.relief.at(x, u) + lift]
    }
    /// A flattened cone from `base` toward `tip`, its broad face turned to `face`, hollowed on that side, grooved along its length.
    fn blade(s: P3, base: P3, tip: P3, face: P3, r: (f64, f64), flat: f64, hollow: f64, strands: f64) -> f64 {
        let axis = sub(tip, base);
        let l = len(axis);
        let a = mul(axis, 1.0 / l);
        let n0 = sub(face, mul(a, dot(face, a)));
        let n = mul(n0, 1.0 / len(n0));
        let w = [a[1] * n[2] - a[2] * n[1], a[2] * n[0] - a[0] * n[2], a[0] * n[1] - a[1] * n[0]];
        let p = sub(s, base);
        let local = [dot(p, w), dot(p, a), dot(p, n) * flat];
        let mut body = round_cone(local, [0.0; 3], [0.0, l, 0.0], r.0, r.1);
        if strands > 0.0 && body < 0.3 {
            let half = lerp(r.0, r.1, (local[1] / l).clamp(0.0, 1.0)).max(0.05);
            let across = local[0] / half;
            body += strands * (0.5 - 0.5 * (PI * 2.2 * across + 0.7 * local[1]).cos()) * smooth(1.0, 0.6, across.abs());
        }
        if hollow <= 0.0 {
            return body;
        }
        let dip = round_cone([local[0] * 1.2, local[1], local[2] - hollow * flat], [0.0, 0.2 * l, 0.0], [0.0, 0.9 * l, 0.0], 0.7 * r.0, 0.05);
        smax(body, -dip, 0.15)
    }
    /// Surface normal of the stock under face point (`x`, `u`), from the relief's slope.
    fn up(&self, x: f64, u: f64) -> P3 {
        let e = 0.3;
        let gx = self.relief.at(x + e, u) - self.relief.at(x - e, u);
        let gu = self.relief.at(x, u + e) - self.relief.at(x, u - e);
        let n = [-gx, -gu, 2.0 * e];
        mul(n, 1.0 / len(n))
    }
    /// A pointed flame of fur swept along a curve on the surface from `root`, `deg` from +x toward +u, `l` long:
    /// `bend` bows its middle and `curl` its tip sideways, `w` its root half-width; three strands converge at the tip.
    fn lock(&self, s: P3, root: (f64, f64), deg: f64, l: f64, bend: f64, w: f64) -> f64 {
        self.lock_from(s, root, None, deg, l, bend, w)
    }
    /// [`Self::lock`] starting at height `root_h` and draping onto the surface by 60% of its length.
    fn lock_from(&self, s: P3, root: (f64, f64), root_h: Option<f64>, deg: f64, l: f64, bend: f64, w: f64) -> f64 {
        let (sn, cs) = deg.to_radians().sin_cos();
        let centre = (root.0 + 0.5 * l * cs, root.1 + 0.5 * l * sn);
        if (s[0] - centre.0).hypot(s[1] - centre.1) > 0.5 * l + w + 1.0 {
            return 1.0 + (s[0] - centre.0).hypot(s[1] - centre.1) - 0.5 * l - w;
        }
        let n = if root_h.is_some() {
            let (sn, cs) = deg.to_radians().sin_cos();
            self.up(root.0 + 0.6 * l * cs, root.1 + 0.6 * l * sn)
        } else {
            self.up(root.0, root.1)
        };
        let flat = 2.3;
        let squash = |p: P3, o: P3| {
            let d = sub(p, o);
            let k = dot(d, n);
            add(o, add(sub(d, mul(n, k)), mul(n, k * flat)))
        };
        let steps = 9;
        let pts: Vec<(P3, f64)> = (0..=steps)
            .map(|i| {
                let t = i as f64 / steps as f64;
                let b = bend * (PI * t).sin() - 0.6 * bend * t * t;
                let (x, u) = (root.0 + t * l * cs - b * sn, root.1 + t * l * sn + b * cs);
                let lift = lerp(0.2 + 0.3 * w, 0.05, t.powf(0.7));
                let r = w * (1.0 - t).powf(0.75) + 0.05;
                let mut p = self.on(x, u, lift);
                if let Some(h) = root_h {
                    p[2] = p[2].max(lerp(h, p[2], smooth(0.0, 0.6, t)));
                }
                (p, r)
            })
            .collect();
        let o = pts[0].0;
        let q = squash(s, o);
        let mut best = (f64::MAX, 0usize);
        for i in 0..steps {
            let (a, ra) = pts[i];
            let (b, rb) = pts[i + 1];
            let d = round_cone(q, squash(a, o), squash(b, o), ra, rb);
            if d < best.0 {
                best = (d, i);
            }
        }
        let (mut d, i) = best;
        if d < 0.25 {
            let (a, ra) = pts[i];
            let (b, rb) = pts[i + 1];
            let dir = sub(b, a);
            let side = cross(n, dir);
            let side = mul(side, 1.0 / len(side).max(1e-9));
            let t = (dot(sub(s, a), dir) / dot(dir, dir)).clamp(0.0, 1.0);
            let across = dot(sub(s, a), side) / lerp(ra, rb, t).max(0.05);
            d += 0.07 * (0.5 - 0.5 * (3.0 * PI * across).cos()) * smooth(1.1, 0.7, across.abs());
        }
        d
    }
    fn head(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let skull = ellipsoid(sub(s, [0.0, 7.0, -1.0]), [5.4, 3.3, 3.5]);
        let cheek = ellipsoid(sub(s, [5.3, 3.0, -1.1]), [2.8, 3.5, 2.6]);
        let mut d = smin(skull, cheek, 1.3);
        if d < 0.3 {
            let phi = (s[1] - 5.9).atan2(s[0]);
            let r = s[0].hypot(s[1] - 5.9);
            d += 0.08 * smooth(2.0, 2.8, r) * (0.5 - 0.5 * (phi * 18.0).cos());
        }
        let brow = ellipsoid(turn(sub(s, [2.65, 6.45, 2.65]), 0, 1, -24.0), [2.05, 0.66, 0.75]);
        d = smin(d, brow, 0.8);
        let muzzle = round_cone(s, [0.0, 6.0, 1.9], [0.0, 1.8, 3.7], 1.85, 1.4);
        let pads = round_cone(s, [1.15, 1.7, 3.2], [2.3, 5.3, 1.6], 1.15, 1.5);
        d = smin(d, smin(muzzle, pads, 0.9), 1.1);
        // The fold between muzzle and cheek, from the eye's inner corner to the corner of the lip.
        let fold = round_cone(s, [2.15, 5.1, 2.55], [3.7, 1.2, 2.0], 0.22, 0.16);
        d = smax(d, -fold, 0.3);
        let nose = ellipsoid(sub(s, [0.0, 1.35, 4.2]), [1.7, 1.0, 1.0]);
        d = smin(d, nose, 0.5);
        // Upper and lower jaws as two masses over the mouth's region, the chin under them.
        let upper = ellipsoid(sub(s, [0.0, MOON_U + 3.2, -0.2]), [7.0, 4.6, 3.2]);
        d = smin(d, upper, 1.2);
        // A slender mandible along the moon's lower arc, the chin at its point.
        let mut mandible = f64::MAX;
        let arc = |a: f64| Self::rim(6.05, a, lerp(0.35, 0.15, smooth(-38.0, -90.0, a)));
        for k in 0..24 {
            let (a0, a1) = (lerp(-30.0, -90.0, k as f64 / 24.0), lerp(-30.0, -90.0, (k + 1) as f64 / 24.0));
            let (r0, r1) = (lerp(0.95, 0.8, (-30.0 - a0) / 60.0), lerp(0.95, 0.8, (-30.0 - a1) / 60.0));
            mandible = mandible.min(round_cone(s, arc(a0), arc(a1), r0, r1));
        }
        let chin = ellipsoid(sub(s, [0.0, MOON_U - 6.55, -0.15]), [1.9, 1.05, 1.1]);
        d = smin(d, smin(mandible, chin, 0.6), 0.9);
        for u in [2.5, 3.1, 3.7, 4.3, 4.9] {
            let h = 3.75 + (6.0 - u) / 4.2 * 1.35 - 0.05;
            let groove = round_cone(s, [0.0, u, h + 0.05], [1.5, u + 0.35, h - 0.65], 0.2, 0.14);
            d = smax(d, -groove, 0.15);
        }
        let nostril = ellipsoid(sub(s, [0.62, 0.95, 4.75]), [0.4, 0.28, 0.4]);
        d = smax(d, -nostril, 0.12);
        let eye = turn(sub(s, [3.0, 5.85, 2.45]), 0, 1, -26.0);
        d = smax(d, -ellipsoid(sub(eye, [0.0, 0.0, 0.5]), [1.6, 0.72, 0.42]), 0.2);
        let ball = ellipsoid(sub(eye, [0.0, -0.05, 0.28]), [1.35, 0.55, 0.3]);
        let ball = smax(ball, -ellipsoid(sub(eye, [0.22, -0.02, 0.6]), [0.26, 0.26, 0.2]), 0.06);
        d = smin(d, ball, 0.1);

        // A gum channel carved round the moon, its floor over the fangs' collar, and snarl creases at the corners.
        let rho = s[0].hypot(s[1] - MOON_U);
        let mouth = (rho - 5.9).max(1.0 - s[2]);
        d = smax(d, -mouth, 0.3);
        let crease = round_cone([s[0], s[1], 0.0], [5.7, MOON_U - 0.5, 0.0], [7.7, MOON_U - 2.7, 0.0], 0.55, 0.15).max(0.9 - s[2]);
        d = smax(d, -crease, 0.25);
        // Teeth rooted in the jaws' walls: canines longest, crossing at the moon's side, incisors toward the fangs.
        let tooth = |root: (f64, f64, f64), tip: (f64, f64, f64), fat: f64| round_cone(s, Self::rim(root.0, root.1, root.2), Self::rim(tip.0, tip.1, tip.2), fat, 0.1);
        let mut teeth = f64::MAX;
        teeth = teeth.min(tooth((6.5, 33.0, 1.9), (5.3, 6.0, 0.9), 0.6));
        teeth = teeth.min(tooth((6.5, -35.0, 0.9), (5.4, -10.0, 1.9), 0.55));
        for (a, fat) in [(48.0, 0.42), (60.0, 0.38)] {
            teeth = teeth.min(tooth((6.35, a, 2.0), (5.3, a - 3.0, 1.1), fat));
        }
        for (a, fat) in [(-49.0, 0.4), (-61.0, 0.36)] {
            teeth = teeth.min(tooth((6.35, a, 0.7), (5.3, a + 3.0, 1.45), fat));
        }
        teeth = teeth.min(tooth((6.5, 17.0, 1.5), (5.55, 15.0, 1.0), 0.45));
        d = smin(d, teeth, 0.12);
        let ear = Self::blade(s, [4.3, 7.9, 1.6], [5.9, 9.8, 4.2], [0.0, -0.75, 1.0], (1.95, 0.25), 2.4, 0.55, 0.0);
        d = smin(d, ear, 0.9);
        // Cheek tufts framing the face, and a beard hanging from the chin.
        let mut fur = f64::MAX;
        for (x, u, deg, l, bend, w) in [
            (6.0, 6.3, 52.0, 2.6, 0.35, 1.0),
            (6.6, 5.1, 36.0, 2.8, 0.4, 1.05),
            (7.0, 3.7, 18.0, 3.0, 0.45, 1.1),
            (7.2, 2.2, 2.0, 3.1, 0.45, 1.1),
            (7.2, 0.6, -14.0, 3.1, 0.45, 1.1),
            (7.1, -1.0, -30.0, 3.0, 0.45, 1.05),
            (6.8, -2.6, -46.0, 2.8, 0.4, 1.0),
            (6.3, -4.1, -62.0, 2.6, 0.35, 0.95),
        ] {
            fur = smin(fur, self.lock(s, (x, u), deg, l, bend, w), 0.45);
        }
        d = smin(d, fur, 0.35);
        d
    }
    fn sdf(&self, p: P3) -> f64 {
        let q = self.face(p);
        let d = self.head(q);
        let rho = q[0].hypot(q[1] - MOON_U);
        let keep = (KEEP_OUT_MM - rho).min(q[2] - 0.1);
        let d = smax(d, keep, 0.12);
        let d = smax(d, -(self.stock.at(p) + BURY_MM), 0.3);
        smax(d, self.bore + BORE_CLEAR_MM - p[0].hypot(p[1]), 0.2)
    }
}

/// A triangle mesh as positions and faces.
struct Nets {
    v: Vec<P3>,
    f: Vec<[u32; 3]>,
}

/// A closed 2-manifold from a sampled distance field by marching tetrahedra on the Kuhn split of each cube.
fn tetra_mesh(lo: P3, hi: P3, step: f64, field: &(dyn Fn(P3) -> f64 + Sync)) -> Nets {
    use std::collections::HashMap;
    let n: [usize; 3] = std::array::from_fn(|k| ((hi[k] - lo[k]) / step).ceil() as usize + 1);
    let at = |i: usize, j: usize, k: usize| -> P3 { [lo[0] + i as f64 * step, lo[1] + j as f64 * step, lo[2] + k as f64 * step] };
    let values: Vec<f32> = (0..n[2])
        .into_par_iter()
        .flat_map_iter(|k| {
            let mut slab = Vec::with_capacity(n[0] * n[1]);
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let border = i == 0 || j == 0 || k == 0 || i == n[0] - 1 || j == n[1] - 1 || k == n[2] - 1;
                    let v = if border { 1.0 } else { field(at(i, j, k)) as f32 };
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

/// Quadric edge-collapse decimation of a closed manifold to `target` faces, never past `max_cost` or a collapse
/// that breaks the link condition, flips a face or leaves one sharper than `min_deg`.
fn decimate(mesh: &Nets, target: usize, max_cost: f64, min_deg: f64) -> Nets {
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
            if len(n1) < 1e-10 || dot(n0, n1) <= 0.3 * len(n0) * len(n1) {
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
    for (k, cap) in [3e-3, 1.5e-3, 7e-4, 3e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 15_000 * k, cap, 1.5 + k as f64);
        let crossings = csg::self_crossings(&csg::Solid { v: nets.v.clone(), f: nets.f.clone() });
        if crossings == 0 {
            return nets;
        }
        println!("  decimation {k}: {crossings} crossings, backing off");
    }
    decimate(raw, raw.f.len(), 0.0, 0.0)
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
    let vol = f.iter().map(|t| {
        let [a, b, c] = t.map(|i| v[i as usize]);
        dot(a, [b[1] * c[2] - b[2] * c[1], b[2] * c[0] - b[0] * c[2], b[0] * c[1] - b[1] * c[0]]) / 6.0
    }).sum();
    (bad, vol)
}

fn to_mesh(nets: &Nets, field: &(dyn Fn(P3) -> f64 + Sync)) -> Mesh {
    let e = 1e-3;
    let normals: Vec<Vec3> = nets
        .v
        .par_iter()
        .map(|p| {
            let g: P3 = std::array::from_fn(|k| {
                let mut a = *p;
                let mut b = *p;
                a[k] += e;
                b[k] -= e;
                field(a) - field(b)
            });
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
const SCULPT_STEP: f64 = 0.11;
const SCULPT_FACES: usize = 90_000;
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

/// The band anchor, the moonstone and its four fangs.
fn stone_and_fangs(d: &mut RingDesign) -> Result<()> {
    let gem = moonstone();
    let doc = d.cad.get_or_insert_with(Document::default);
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let placement = Placement::Ring { theta_deg: 90.0, across_mm: -MOON_U, height_mm: builders::stand_off_mm(builders::CLAW, gem), spin_deg: 90.0, tilt_deg: 0.0, cant_deg: 0.0 };
    doc.append(builders::stone_feature(2, gem, placement))?;
    let mut fangs = builders::feature_on(3, "Fangs", builders::CLAW, 2, json!({"prongs": 4, "wire_mm": 2.0, "style": "Fang", "grouping": "Jaws", "tip": "Point"}));
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
    let raw = tetra_mesh(lo, hi, step, &field);
    let nets = clean_decimate(&raw, SCULPT_FACES);
    let sculpt = to_mesh(&nets, &field);
    stone_and_fangs(&mut d)?;
    let lib = AlphaLibrary::builtin();
    let params = BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..Default::default() };
    let built = mesh::try_build(&d, &lib, params)?;
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let mut parts = vec![Part::metal(&built.mesh, render::GOLD), Part::metal(&sculpt, render::GOLD)];
    parts.extend(gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)));
    for (name, yaw, pitch) in [("face", 0.0, PI * 0.5), ("hero", 0.55, 1.0), ("low", 0.3, 0.45), ("side", PI * 0.5, 0.25)] {
        render::write_png_parts(out.join(format!("sculpt-{name}.png")), &parts, yaw, pitch, 800)?;
    }
    Ok(())
}

/// The head's grid box in world millimetres over a table at `table`.
fn sculpt_box(table: f64) -> (P3, P3) {
    ([-11.0, table - 5.5, -11.5], [11.0, table + 6.5, 13.5])
}

/// The wolf's field over the design's own stock.
fn wolf_of(d: &RingDesign, a: &Atlas) -> Result<Wolf> {
    let table = a.top;
    let relief = Relief::of(a, table);
    let coarse = Atlas::of(d, 1024, 384)?;
    let (lo, hi) = sculpt_box(table);
    let stock = Stock::of(&coarse, &relief, table, lo, hi, 0.25);
    Ok(Wolf { table, bore: d.inner_radius_mm(), relief, stock })
}

/// The head as a stored part, joined to the stock.
fn head_feature(wolf: &Wolf) -> Result<(Feature, Value)> {
    let t = Instant::now();
    let (lo, hi) = sculpt_box(wolf.table);
    let field = |p: P3| wolf.sdf(p);
    let raw = tetra_mesh(lo, hi, SCULPT_STEP, &field);
    let nets = clean_decimate(&raw, SCULPT_FACES);
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

/// The pelt at hide point (`along` from the head, `across`): height 0..1 and the crest groove 0..1.
/// A low pelt everywhere, and on it long flames pointed at both ends, bowed in an S, three strands each,
/// shingled half a lock apart and flowing away from the head.
fn fur(along: f64, across: f64) -> (f64, f64) {
    let pitch = 1.3;
    let length = 5.2;
    let step = length * 0.45;
    let row = (across / pitch).round() as i64;
    let (mut best, mut crest) = (0.0f64, 0.0f64);
    for j in row - 1..=row + 1 {
        let shift = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 * step };
        let i0 = ((along - shift) / step).floor() as i64;
        for i in i0 - 3..=i0 {
            let t = (along - shift - i as f64 * step) / length;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let wave = if (i + j).rem_euclid(2) == 0 { 1.0 } else { -1.0 };
            let centre = j as f64 * pitch + 0.38 * wave * (1.4 * PI * t + 0.3).sin() + 0.2 * across.signum() * t;
            let half = 0.62 * pitch * (PI * t.powf(0.7)).sin().max(0.0).powf(0.8);
            let y = across - centre;
            if y.abs() >= half {
                continue;
            }
            let q = y / half;
            let body = (q * PI * 0.5).cos().max(0.0).powf(1.2);
            let grooves = 0.5 - 0.5 * (3.0 * PI * q).cos();
            let h = body * (PI * t.powf(0.75)).sin().max(0.0).powf(0.7) * (0.8 + 0.2 * t) * (1.0 - 0.16 * grooves * smooth(0.08, 0.3, t));
            if h > best {
                best = h;
                crest = (1.0 - smooth(0.0, 0.2, q.abs())) * smooth(0.1, 0.3, t) * (1.0 - smooth(0.75, 0.95, t));
            }
        }
    }
    (0.28 + 0.72 * best, crest)
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
    for v in &mut alpha.data {
        *v = (*v * 255.0).round() / 255.0;
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

fn author(params: BuildParams) -> Result<(RingDesign, AlphaLibrary, Value)> {
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
            smooth(a.bore + 1.0, a.bore + 1.5, r) * smooth(-0.05, 0.7, wolf.sdf(s.p)) * off_folds(s.p, caps)
        };
        let ruff = a.paint("Ruff", |s| {
            let h = hide.at(s);
            let (lock, crest) = fur(h.along.abs(), h.across);
            ((lock - 0.22 * crest * lock) * clear(s)).min(room[s.i] / RUFF_MM)
        });
        portable(d, &mut lib, ruff, RUFF_MM, window(90.0, 262.0), false)?;
        let hair = a.paint("Graver's hair lines", |s| {
            let h = hide.at(s);
            let (lock, _) = fur(h.along.abs(), h.across);
            let line = (h.across * 7.5 + 0.9 * (h.along.abs() * 0.8).sin()).fract();
            (1.0 - smooth(0.0, 0.18, (line - 0.5).abs())) * smooth(0.35, 0.6, lock) * clear(s)
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
    Ok((d, lib, composition))
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
    let (d, lib, composition) = author(params)?;
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
    let (d, lib, _) = author(params)?;
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
    if args.iter().any(|a| a == "--sculpt") {
        return sculpt_preview(Path::new(out), SCULPT_STEP);
    }
    write(Path::new(out), args.iter().any(|a| a == "--draft"), args.iter().any(|a| a == "--verify"))
}
