//! Fenrir: the wolf and the moon, on native factory 010 trillion stock in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_fenrir
//! target/release/examples/bestiarium_fenrir [OUT_DIR] [--draft] [--verify] [--sculpt] [--map]
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
    sculpt::{self, Heights, Stock, clean_decimate, decimate, gradient, relax, settle, tetra_mesh},
    skin::{self, Atlas, Hide, Sample},
    stl,
    svg::SvgAlpha,
    tiling::TilingLayer,
};
use serde_json::{Value, json};
use std::{f64::consts::PI, path::Path, time::Instant};

type P3 = [f64; 3];
/// A triangle mesh as positions and faces.
type Nets = csg::Solid;

fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

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
/// Approximate distance to an axis-aligned ellipse of semi-axes `r` at the origin.
fn ellipse(p: [f64; 2], r: [f64; 2]) -> f64 {
    let k0 = (p[0] / r[0]).hypot(p[1] / r[1]);
    let k1 = (p[0] / (r[0] * r[0])).hypot(p[1] / (r[1] * r[1]));
    if k1 < 1e-12 { -r[0].min(r[1]) } else { k0 * (k0 - 1.0) / k1 }
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

/// The head keeps this far outside the finger's cylinder, mm.
const BORE_CLEAR_MM: f64 = 0.35;
/// How deep the sculpt reaches under the stock's surface, mm.
const BURY_MM: f64 = 0.6;
/// No sculpt within this radius of the moon's axis over the table, so the stone drops past the gums.
const KEEP_OUT_MM: f64 = 5.03;
/// The gums round the moon stand this high over the table, over the fangs' base rail, mm.
const GUM_H: f64 = 1.35;
/// Half the angle each open mouth corner spans at the moon's side, degrees.
const CORNER_DEG: f64 = 19.0;
/// The fangs' bearing round the moon, degrees from +x.
const FANG_DEG: f64 = 73.4;
/// The stop, where the muzzle leaves the brow, and the nose leather's centre, face `u`: a wolf's long muzzle.
const STOP_U: f64 = 7.3;
const NOSE_U: f64 = 0.95;
/// Each ear's base and tip, face `x`, `u`, `h` (right side).
const EAR_BASE: P3 = [3.75, 8.6, 1.35];
const EAR_TIP: P3 = [5.3, 12.1, 2.45];

/// A point `rho` from the moon's axis, `deg` from +x toward +u, at height `h`.
fn rim(rho: f64, deg: f64, h: f64) -> P3 {
    let (s, c) = deg.to_radians().sin_cos();
    [rho * c, MOON_U + rho * s, h]
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

/// What a tooth is: its height over the gum, half its length along the jaw, half its thickness across, its tip's
/// radius, its outward lean in degrees, and a second, lower cusp ahead of the main one as (offset along the jaw, share
/// of the height), for the carnassials.
#[derive(Clone, Copy)]
struct Kind {
    h: f64,
    len: f64,
    thick: f64,
    tip: f64,
    lean: f64,
    cusp: Option<(f64, f64)>,
}

/// A tooth: a laterally flattened cone standing on the gum at a bearing round the moon, leaning out toward the lip.
struct Tooth {
    name: String,
    base: P3,
    tan: P3,
    out: P3,
    up: P3,
    kind: Kind,
    tip: P3,
}

impl Tooth {
    /// A tooth of `kind` rooted at moon bearing `deg`, `rho` from the moon's axis, its base `floor` over the table.
    fn new(name: String, deg: f64, rho: f64, floor: f64, kind: Kind) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        let (radial, tan) = ([c, s, 0.0], [-s, c, 0.0]);
        let (ls, lc) = kind.lean.to_radians().sin_cos();
        let up = add(mul([0.0, 0.0, 1.0], lc), mul(radial, ls));
        let out = add(mul([0.0, 0.0, 1.0], -ls), mul(radial, lc));
        let base = rim(rho, deg, floor);
        let tip = add(base, mul(up, kind.h + 0.35));
        Self { name, base, tan, out, up, kind, tip }
    }
    fn sdf(&self, q: P3) -> f64 {
        let p = sub(q, self.base);
        let k = self.kind;
        let (pt, pr, pz) = (dot(p, self.tan), dot(p, self.out), dot(p, self.up));
        // Squeezed along the jaw, so a round cone becomes a blade as long as `len` and as thick as `thick`.
        let squeeze = (k.len / k.thick).max(1.0);
        let top = k.h + 0.35;
        // A stout body holding its thickness to within 0.55 mm of the tip, then a short point.
        let blade = |off: f64, share: f64| {
            let tz = top * share;
            let p = [(pt - off) / squeeze, pr, pz];
            let knee = (tz - 0.55).max(0.3);
            smin(round_cone(p, [0.0, 0.0, 0.0], [0.0, 0.0, knee], k.thick, 0.92 * k.thick), round_cone(p, [0.0, 0.0, knee], [0.0, 0.0, tz - k.tip], 0.92 * k.thick, k.tip), 0.08)
        };
        let main = blade(0.0, 1.0);
        match k.cusp {
            Some((off, share)) => smin(main, blade(off, share), 0.12),
            None => main,
        }
    }
}

/// A smooth curve through `(key, values)` rows sorted by key, read at `x`: cubic Hermite with Catmull-Rom tangents.
fn spline<const N: usize>(rows: &[(f64, [f64; N])], x: f64) -> [f64; N] {
    let n = rows.len();
    let x = x.clamp(rows[0].0, rows[n - 1].0);
    let i = (0..n - 1).find(|&i| x <= rows[i + 1].0).unwrap_or(n - 2);
    let (x0, x1) = (rows[i].0, rows[i + 1].0);
    let h = (x1 - x0).max(1e-9);
    let t = (x - x0) / h;
    let slope = |j: usize, k: usize| -> f64 {
        let (a, b) = (j.saturating_sub(1), (j + 1).min(n - 1));
        (rows[b].1[k] - rows[a].1[k]) / (rows[b].0 - rows[a].0).max(1e-9)
    };
    let (h00, h10, h01, h11) = (2.0 * t * t * t - 3.0 * t * t + 1.0, t * t * t - 2.0 * t * t + t, -2.0 * t * t * t + 3.0 * t * t, t * t * t - t * t);
    std::array::from_fn(|k| h00 * rows[i].1[k] + h10 * h * slope(i, k) + h01 * rows[i + 1].1[k] + h11 * h * slope(i + 1, k))
}

/// The upper lip by moon bearing in degrees: its inner edge's distance from the moon's axis, its crest's height, its
/// width across and its thickness. Narrow under the nose, hitched up and back over the fang, drawn far back and fleshy
/// over the premolars where the snarl pulls it, and curling down into the mouth's corner.
fn upper_lip(deg: f64) -> [f64; 4] {
    let a = (90.0 - (90.0 - deg).abs()).clamp(CORNER_DEG, 90.0);
    spline(
        &[
            (CORNER_DEG, [5.95, 1.35, 1.0, 0.85]),
            (25.0, [5.95, 1.6, 1.1, 0.9]),
            (34.0, [5.95, 1.85, 1.15, 0.9]),
            (47.0, [5.95, 2.05, 1.15, 0.95]),
            (61.0, [5.95, 2.3, 1.1, 0.95]),
            (FANG_DEG, [6.1, 2.75, 1.05, 0.95]),
            (82.0, [5.95, 2.35, 0.9, 0.9]),
            (90.0, [5.95, 2.25, 0.85, 0.85]),
        ],
        a,
    )
}

/// The lower lip by moon bearing, as [`upper_lip`]: close under the corners, drawn down off the premolars, hitched up
/// under the lower fang and rounding under the chin.
fn lower_lip(deg: f64) -> [f64; 4] {
    let a = (90.0 - (90.0 + deg).abs()).clamp(CORNER_DEG, 90.0);
    let mut l = lower_lip_line(a);
    // The lip's edge dips between the teeth behind the fang and swells over each, so it reads as flesh on a jaw.
    let wave = (2.0 * PI * (a - 27.0) / 11.5).cos() * smooth(66.0, 58.0, a) * smooth(CORNER_DEG, 26.0, a);
    l[0] += 0.12 * wave;
    l[1] += 0.06 * wave;
    l
}

/// The lower lip's smooth line by folded bearing, before its scallops.
fn lower_lip_line(a: f64) -> [f64; 4] {
    spline(
        &[
            (CORNER_DEG, [6.4, 1.35, 0.9, 0.85]),
            (32.0, [6.8, 1.4, 0.95, 0.9]),
            (45.0, [6.85, 1.35, 0.95, 0.9]),
            (60.0, [6.6, 1.45, 0.9, 0.85]),
            (FANG_DEG, [6.3, 1.7, 0.85, 0.85]),
            (82.0, [6.05, 1.45, 0.8, 0.8]),
            (90.0, [6.0, 1.35, 0.8, 0.8]),
        ],
        a,
    )
}

/// The gum's top over the table: the upper jaw's shelf, and the lower's a little lower so its teeth stand clear.
fn gum_h(deg: f64) -> f64 {
    if deg >= 0.0 { GUM_H } else { GUM_H - 0.2 }
}

/// How far from the moon's axis the mandible's outline runs at bearing `deg`: a little past the lip, fuller toward the
/// chin, which stands no further out than 8 mm.
fn jaw_out(deg: f64) -> f64 {
    let l = lower_lip(deg);
    l[0] + l[2] + 0.05
}

/// Short fur over the mandible, 0..1: locks lying along the jaw, sweeping back from the chin to each corner.
fn jaw_fur(s: P3) -> f64 {
    let (dx, du) = (s[0], s[1] - MOON_U);
    let rho = dx.hypot(du);
    let a = du.atan2(dx);
    flames((a + 0.5 * PI) * 7.6, rho - 7.0, 0.95, 3.6, 0.2, 0.3, 41, &sculpted_lock(0.95)).h
}

/// The facial fur, 0..1: three tiers of pointed locks 3 to 4 mm long sweeping back from the muzzle toward the ears,
/// the lowest flaring out over the jowl and the highest lying back beside the eyes; each lock has a rounded crest and
/// one groove down its middle.
fn cheek_fur(s: P3) -> f64 {
    let tier = |dir: (f64, f64), pitch: f64, length: f64, seed: i64, shift: f64| {
        let l = dir.0.hypot(dir.1);
        let (cx, cu) = (dir.0 / l, dir.1 / l);
        let (along, across) = (s[0] * cx + s[1] * cu, -s[0] * cu + s[1] * cx);
        flames(along + shift, across, pitch, length, 0.12, 0.3, seed, &grooved_lock(pitch)).h
    };
    let low = tier((0.85, 0.55), 1.85, 4.2, 5, 0.0) * (1.0 - smooth(0.6, 2.2, s[1]));
    let mid = tier((0.6, 0.8), 1.75, 4.0, 11, 0.7) * smooth(0.6, 2.2, s[1]) * (1.0 - smooth(3.4, 4.8, s[1]));
    let high = tier((0.35, 0.94), 1.6, 3.6, 23, 1.3) * smooth(3.4, 4.8, s[1]);
    smax(smax(low, mid, 0.3), high, 0.3)
}

/// A lock carved for the face: a rounded crest with a single groove down its middle, full height until near its
/// point, which the flame's narrowing draws to a tip.
fn grooved_lock(pitch: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |t, q, half| {
        let across = (q * PI * 0.5).cos().max(0.0).powi(2) * (1.0 - 0.15 * (-(q / 0.4).powi(2)).exp());
        let rise = (0.35 + 0.65 * smooth(0.0, 0.5, t)) * smooth(0.0, 0.2, t) * (1.0 - smooth(0.72, 1.0, t));
        // Lowered where it narrows, so the point is drawn by its outline and never stands up as a blade.
        across * rise * (half / (0.45 * pitch)).min(1.0).powi(2)
    }
}

/// Fur over the crown, flowing back from the brows between the ears, 0..1; `s` carries its signed `x`.
fn crown_fur(s: P3) -> f64 {
    flames(s[1] - STOP_U, s[0], 1.5, 3.3, 0.16, 0.35, 31, &sculpted_lock(1.5)).h
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
    let at = |t: f64| {
        let x = (1.0 - t) * (1.0 - t) * a.0 + 2.0 * t * (1.0 - t) * mid.0 + t * t * b.0;
        let u = (1.0 - t) * (1.0 - t) * a.1 + 2.0 * t * (1.0 - t) * mid.1 + t * t * b.1;
        (x, u)
    };
    // The nearest point on the curve, projected onto each of its chords so the share along it moves continuously.
    let n = 24;
    let mut best = (f64::MAX, 0.0);
    for i in 0..n {
        let (t0, t1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
        let (p0, p1) = (at(t0), at(t1));
        let (sx, su) = (p1.0 - p0.0, p1.1 - p0.1);
        let k = (((q[0] - p0.0) * sx + (q[1] - p0.1) * su) / (sx * sx + su * su).max(1e-12)).clamp(0.0, 1.0);
        let d = (q[0] - p0.0 - k * sx).hypot(q[1] - p0.1 - k * su);
        if d < best.0 {
            best = (d, t0 + k * (t1 - t0));
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
    /// A canine sheath over each fang's claw: its axis stations in face coordinates with their radii.
    sheaths: Vec<Vec<(P3, f64)>>,
    /// The moonstone as the planes of its convex hull in face coordinates, and a sphere round it.
    stone: (Vec<(P3, f64)>, P3, f64),
}

impl Wolf {
    fn face(&self, p: P3) -> P3 {
        [p[0], -p[2], p[1] - self.table]
    }
    /// Whether world point `p` lies over the cheeks, brow and muzzle, clear of the mouth and ears.
    fn in_face_zone(&self, p: P3) -> bool {
        let q = self.face(p);
        q[2] > 0.1 && q[0].hypot(q[1] - MOON_U) > 6.7 && q[1] < STOP_U + 1.1 && q[0].abs() < 9.4
    }
    fn new(table: f64, bore: f64, relief: Relief, stock: Stock, sheaths: Vec<Vec<(P3, f64)>>, stone: (Vec<(P3, f64)>, P3, f64)) -> Self {
        let mut w = Self { table, bore, relief, stock, teeth: Vec::new(), eye_h: 0.0, sheaths, stone };
        w.eye_h = w.surface_h(Self::EYE.0, Self::EYE.1);
        w.teeth = Self::dentition();
        w
    }
    /// Where each eye sits, face `x` and `u`.
    const EYE: (f64, f64) = (2.4, STOP_U - 0.25);
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
    /// The teeth each side of each jaw besides the fang: two small chisel incisors between the fangs, two blade-edged
    /// premolars and the long two-cusped carnassial behind it, and a low molar in the corner, graded from 0.55 mm at
    /// the front to 1.1 mm at the carnassial, each leaning out toward the drawn-back lip so its crown shows from above.
    fn dentition() -> Vec<Tooth> {
        let incisor = Kind { h: 0.55, len: 0.52, thick: 0.46, tip: 0.13, lean: 4.0, cusp: None };
        let incisor2 = Kind { h: 0.62, len: 0.6, thick: 0.47, tip: 0.11, lean: 5.0, cusp: None };
        let premolar = Kind { h: 0.85, len: 0.82, thick: 0.5, tip: 0.07, lean: 8.0, cusp: None };
        let premolar2 = Kind { h: 0.95, len: 0.9, thick: 0.51, tip: 0.07, lean: 9.0, cusp: None };
        let carnassial = Kind { h: 1.1, len: 1.05, thick: 0.52, tip: 0.07, lean: 10.0, cusp: Some((0.66, 0.62)) };
        let molar = Kind { h: 0.7, len: 0.95, thick: 0.54, tip: 0.16, lean: 8.0, cusp: None };
        let mut teeth = Vec::new();
        for (side, sx) in [("right", 1.0), ("left", -1.0)] {
            // A bearing on the right side mirrors across the midline to the left.
            let mirror = |deg: f64| if sx > 0.0 { deg } else { 180.0 - deg };
            for (jaw, sign, lip) in [("upper", 1.0, upper_lip as fn(f64) -> [f64; 4]), ("lower", -1.0, lower_lip as fn(f64) -> [f64; 4])] {
                for (name, a, kind) in [("incisor", 86.0, incisor), ("second incisor", 80.3, incisor2), ("premolar", 61.5, premolar), ("second premolar", 50.5, premolar2), ("carnassial", 38.5, carnassial), ("molar", 27.0, molar)] {
                    // Behind the canine the upper flew hangs over the cheek teeth, so only the lower row shows them.
                    if sign > 0.0 && a < FANG_DEG {
                        continue;
                    }
                    let deg = sign * a;
                    let edge = lip(deg)[0];
                    // Rooted on the stone's side of the gum, standing nearly upright, so a clear gap of 0.35 mm and more
                    // opens between each tooth and the lip behind it where the gum is wide enough.
                    let near = KEEP_OUT_MM + 0.14 + kind.thick;
                    let rho = near + 0.25 * (edge - 0.4 - kind.thick - near).max(0.0);
                    // The carnassial's second cusp stands ahead of it, toward the front of the mouth.
                    let mut kind = kind;
                    // Along the lower row the crowns alternate high and low by a quarter millimetre and stretch into
                    // blades, so from above each shows its own point rather than one more bead in a string.
                    if sign < 0.0 {
                        let (dh, dl) = match name {
                            "incisor" => (0.0, 0.1),
                            "second incisor" => (0.3, 0.12),
                            "premolar" => (-0.05, 0.3),
                            "second premolar" => (0.25, 0.25),
                            "carnassial" => (-0.1, 0.1),
                            _ => (0.25, 0.0),
                        };
                        kind.h += dh;
                        kind.len += dl;
                        kind.tip = kind.tip.min(0.09);
                    }
                    if let Some((off, share)) = kind.cusp {
                        kind.cusp = Some((off * sign * sx, share));
                    }
                    teeth.push(Tooth::new(format!("{jaw} {name}, {side}"), mirror(deg), rho, gum_h(deg) - 0.35, kind));
                }
            }
        }
        teeth
    }
    /// The skull, its underside rounded off well clear of the finger where it hangs past the stock's back edge.
    fn cranium(s: P3) -> f64 {
        let root = ellipsoid(sub(s, [4.5, EAR_BASE[1] - 0.2, 0.5]), [1.4, 1.4, 1.4]);
        let skull = smax(ellipsoid(sub(s, [0.0, STOP_U + 0.6, -1.0]), [5.0, 2.9, 3.9]), -(s[2] + 3.0), 0.8);
        smin(skull, root, 0.6)
    }
    /// The cheek, widest beside the eyes and narrowing toward the mouth's corner, so the face tapers like a wedge.
    fn cheek(s: P3) -> f64 {
        ellipsoid(turn(sub(s, [5.4, 4.3, -1.45]), 0, 1, -24.0), [3.0, 3.6, 2.45])
    }
    fn jowl(s: P3) -> f64 {
        ellipsoid(sub(s, [5.75, -0.8, -1.55]), [1.35, 2.1, 1.8])
    }
    fn brow(s: P3) -> f64 {
        round_cone(s, [1.05, STOP_U, 2.75], [3.5, STOP_U + 1.35, 2.05], 0.55, 0.32)
    }
    /// The brow's crest height over face `x`.
    fn brow_crest(x: f64) -> f64 {
        let t = ((x - 1.05) / 2.5).clamp(0.0, 1.0);
        lerp(2.75, 2.05, t) + lerp(0.55, 0.32, t)
    }
    /// The muzzle: a trapezoid section with a flat bridge and softened side planes, from the stop to the nose,
    /// narrowing toward the nose so the snout reads long.
    fn muzzle(s: P3) -> f64 {
        let (st, nz) = ([0.0, STOP_U, 1.0], [0.0, NOSE_U + 0.35, 2.5]);
        let axis = sub(nz, st);
        let l = len(axis);
        let a = mul(axis, 1.0 / l);
        let b = [0.0, a[2], -a[1]];
        let ps = sub(s, st);
        let t = dot(ps, a) / l;
        let tc = t.clamp(0.0, 1.0);
        let sec = [s[0], dot(ps, b)];
        // A wedge tapering from the stop to the nose, its sides sloping straight down to the lips and its top a plane that
        // runs straight to the leather.
        let body = trapezoid(sec, lerp(2.6, 1.55, tc), lerp(1.0, 0.45, tc), lerp(1.4, 1.15, tc)) - 0.3;
        smax(body, (t - 1.0) * l, 0.3).max(-t * l - 1.2)
    }
    /// The nose's frame: its top running on from the bridge, its front turned down toward the moon.
    fn nose_frame(s: P3) -> P3 {
        turn(sub(s, [0.0, NOSE_U, 3.55]), 1, 2, 22.0)
    }
    /// The nose: a wedge of leather seen from over the face, 1.96 mm across its back and 1.1 mm across its front,
    /// rounded 0.3 mm all round, standing clear of the bridge; under it the philtrum carries it down into the lip.
    fn nose(s: P3) -> f64 {
        let p = Self::nose_frame(s);
        let plan = trapezoid([p[0], p[1]], 0.85, 1.4, 0.62);
        let w = [plan + 0.42, p[2].abs() - 0.45 + 0.42];
        // Rounded 0.42 mm all round and domed, so the pad reads as leather rather than a block.
        let wedge = len([w[0].max(0.0), w[1].max(0.0), 0.0]) + w[0].max(w[1]).min(0.0) - 0.42;
        let pad = smin(wedge, ellipsoid(sub(p, [0.0, 0.05, 0.08]), [1.3, 0.72, 0.6]), 0.3);
        // The philtrum, and a web under the pad's back that closes the crease between nose and lip.
        let philtrum = smin(ellipsoid(sub(s, [0.0, NOSE_U - 0.28, 2.7]), [0.75, 0.42, 0.6]), ellipsoid(sub(s, [0.0, NOSE_U + 0.1, 2.75]), [1.3, 0.6, 0.45]), 0.3);
        smin(pad, philtrum, 0.5)
    }
    /// A lip at the query's bearing: a rolled flew of the width and thickness `lip` gives, its section an ellipse
    /// whose crest stands at the lip's height, falling back behind it into the jaw's body toward `back` at `low`.
    fn lip(s: P3, a: f64, lip: [f64; 4], back: f64, low: f64, r_back: f64, blend: f64) -> f64 {
        let [rho, h, w, t] = lip;
        // The query sits on its own bearing, so the section is taken at its own distance from the moon's axis.
        let r = s[0].hypot(s[1] - MOON_U);
        let roll = ellipse([r - (rho + 0.5 * w), s[2] - (h - 0.5 * t)], [0.5 * w, 0.5 * t]);
        let body = round_cone(s, rim(rho + 0.55 * w, a, h - 0.6 * t), rim(back, a, low), 0.4 * t, r_back);
        smin(roll, body, blend)
    }
    /// The upper jaw at the query's bearing: the flew over the gums, falling back into the cheeks.
    fn upper_jaw(s: P3) -> f64 {
        let ang = (s[1] - MOON_U).atan2(s[0]).to_degrees();
        let a = ang.clamp(CORNER_DEG, 90.0);
        let mut l = upper_lip(a);
        // The flew is broken at the fang: its crest drops 0.65 mm where the canine comes out, so the lip reads as two,
        // the short one under the nose and the long one hauled back over the premolars.
        l[1] -= 0.65 * sculpt::bell(a, FANG_DEG, 4.5);
        let lip = Self::lip(s, a, l, l[0] + l[2] + 1.1, -1.0, lerp(0.95, 1.2, smooth(20.0, 60.0, a)), lerp(1.0, 0.45, smooth(28.0, 50.0, a)));
        // The flew ends in a rounded cap at the mouth's corner rather than running on round it, and the two flews
        // pinch together under the nose in a cleft, so the lip reads as a pair meeting at the philtrum.
        let rho = s[0].hypot(s[1] - MOON_U);
        let cleft = 0.28 * (-(s[0] / 0.32).powi(2)).exp() * smooth(l[1] - l[3], l[1] - 0.2 * l[3], s[2]);
        smax(lip + cleft, (CORNER_DEG - 4.0 - ang).to_radians() * rho, 0.6)
    }
    /// The mandible at the query's bearing: the lower lip inside the jaw's outline, rounded underneath to the chin.
    fn mandible(s: P3) -> f64 {
        let ang = (s[1] - MOON_U).atan2(s[0]).to_degrees();
        let a = ang.clamp(-90.0, -CORNER_DEG);
        let l = lower_lip(a);
        let rho = s[0].hypot(s[1] - MOON_U);
        let jaw = smax(Self::lip(s, a, l, jaw_out(a) - 0.75, -1.35, 0.9, 0.45), (ang + CORNER_DEG - 4.0).to_radians() * rho, 0.6);
        let chin = ellipsoid(sub(s, [0.0, MOON_U - 5.95, -0.3]), [1.2, 0.8, 0.9]);
        smin(jaw, chin, 0.8)
    }
    /// The gums between the lips and the stone, carried under the lips down to the table so no air is shut in
    /// between them, and left open at the mouth's corners.
    fn gums(s: P3) -> f64 {
        let rho = s[0].hypot(s[1] - MOON_U);
        let ang = (s[1] - MOON_U).atan2(s[0]).to_degrees();
        let l = if ang >= 0.0 { upper_lip(ang) } else { lower_lip(ang) };
        let end = (CORNER_DEG + 3.0 - ang.abs()).to_radians() * rho.max(1.0);
        // Below the lip's roll the gum widens into the jaw's body, so no slot opens between them.
        let widen = if ang >= 0.0 { (l[2] + 0.4) * smooth(l[1] - l[3] + 0.1, l[1] - l[3] - 0.5, s[2]) } else { (jaw_out(ang) - l[0] - 0.5).max(0.0) * smooth(0.8, -1.0, s[2]) };
        smax(smax((rho - l[0] - 0.3 - widen).max(s[2] - gum_h(ang)), -(s[2] + 2.5), 0.4), end, 0.4)
    }
    /// The throat under the chin, hanging over the apex wall, its lowest point a millimetre and more over the finger.
    fn throat(s: P3) -> f64 {
        ellipsoid(sub(s, [0.0, MOON_U - 4.85, -1.95]), [3.7, 1.6, 1.7])
    }
    /// A pricked ear: a leaf standing up from the crown's corner, its section a stadium rounded 0.5 mm at the edges and
    /// bowed so the back is convex and the front cupped, twisting outward toward its point, which curls back; a deeper
    /// cup inside a soft rim, and a tuft of fur at the inner base.
    fn ear(s: P3) -> f64 {
        let (base, tip) = (EAR_BASE, EAR_TIP);
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
        // The outward twist, growing to 13 degrees at the point.
        let (ts, tc) = (13.0 * t).to_radians().sin_cos();
        let (x, z) = (tc * x - ts * z, ts * x + tc * z);
        let half = (1.75 * (1.0 - t).powf(0.85)).max(0.02);
        let thick = 0.7 * (1.0 - 0.3 * t) * (1.0 - 0.75 * t.powi(8));
        // Bowed across: the edges swept forward of the middle, and the point curling back.
        let bow = 0.16 * (x / half.max(0.3)).clamp(-1.3, 1.3).powi(2) - 0.25 * t * t;
        let z = z - bow;
        let r = thick.min(0.5);
        let q = [x.abs() - (half - r).max(0.0), z.abs() - (thick - r).max(0.0)];
        let section = len([q[0].max(0.0), q[1].max(0.0), 0.0]) + q[0].max(q[1]).min(0.0) - r;
        let leaf = smax(smax(section, -y, 0.6), y - l, 0.25);
        // The cup: an ellipsoidal hollow inside the rim on the front, deepest a third of the way up.
        let wcup = (half - 0.7).max(0.05);
        let cup = ellipsoid(sub([x, y, z], [0.0, 0.36 * l, thick + 0.08]), [wcup, 0.3 * l, 0.48 - 0.26 * smooth(0.4, 0.66, t)]);
        let cupped = smax(leaf, -cup, 0.22);
        // A tuft of three short locks at the inner base, lying up the cup's lower edge.
        // ... rising as three strands up the cup, so its floor shades as a curve.
        let tuft = [(-0.55, 0.05, 1.25, 0.24), (-0.15, 0.0, 1.7, 0.26), (0.3, 0.05, 1.35, 0.22)]
            .iter()
            .map(|&(x0, y0, reach, r0)| round_cone([x, y, z], [x0, y0, thick - 0.1], [x0 * 0.55, y0 + reach, thick - 0.32], r0, 0.12))
            .fold(f64::MAX, f64::min);
        smin(cupped, tuft, 0.12)
    }
    /// A short tuft lying down the chin's front, pointing away from the moon.
    fn chin_tuft(s: P3) -> f64 {
        let a = round_cone(s, [0.0, MOON_U - 6.4, 0.2], [0.0, MOON_U - 6.7, -0.45], 0.42, 0.34);
        smin(a, round_cone(s, [0.0, MOON_U - 6.7, -0.45], [0.0, MOON_U - 6.75, -1.15], 0.34, 0.26), 0.25)
    }
    /// The head's masses and smooth features, without fur or carved detail.
    fn masses(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let mut d = smin(smin(Self::cranium(s), Self::cheek(s), 1.5), Self::jowl(s), 0.9);
        d = smin(d, Self::upper_jaw(s), 0.7);
        d = smin(d, Self::brow(s), 0.5);
        d = smin(d, Self::muzzle(s), 1.0);
        // The muzzle's underside carried down solid to the table between the jaws, so no pocket closes under it.
        d = smin(d, ellipsoid(sub(s, [0.0, 3.6, 0.2]), [1.7, 2.5, 0.95]), 0.6);
        d = smin(d, Self::nose(s), 0.4);
        // The jaw's hinge: a masseter at each corner, falling from the cheek under the open corner to the mandible's
        // back end, so the lower jaw hangs from the skull instead of floating.
        let hinge = ellipsoid(sub(s, [7.05, MOON_U - 0.4, -1.35]), [0.85, 2.8, 1.3]);
        let jaws = smin(smin(Self::mandible(s), Self::gums(s), 0.3), hinge, 0.9);
        d = smin(d, smin(jaws, Self::throat(s), 0.8), 0.5);
        d = smin(d, Self::ear(s), 0.5);
        // Along the flanks the masses swell into a wide fillet down onto the stock, run out a little under its
        // surface, so the head slopes into the ruff instead of standing as a wall, and the fur rides on it.
        let flank = smooth(6.2, 8.2, s[0]) * smooth(-3.5, -1.5, s[1]) * smooth(-1.8, -0.8, s[2]);
        if flank > 0.01 {
            let g = self.stock.at([q[0], q[2] + self.table, -q[1]]);
            d = smin(d, g + 0.35, 1.4 * flank);
        }
        d
    }
    /// How much fur the skin at face point `f` (signed `x`) takes: none within a millimetre of the stock it stands on,
    /// none on the lips or round the eyes, full on the cheeks, crown and throat.
    fn fur_room(&self, f: P3) -> f64 {
        let over = self.stock.at([f[0], f[2] + self.table, -f[1]]);
        // Along the flanks the fur runs a little further down the fillet toward the stock, where the ruff takes it up.
        let flank = smooth(6.2, 8.2, f[0].abs()) * smooth(-3.5, -1.5, f[1]) * smooth(-1.8, -0.8, f[2]);
        smooth(lerp(0.1, -0.1, flank), lerp(0.6, 0.15, flank), over)
    }
    /// The fur's relief at face point `q`, mm: cheeks and jowls flowing up and out toward the ears, the crown flowing back
    /// between the ears, the throat flowing down the apex wall; none on the lips, round the eyes, on the muzzle or ears,
    /// and none within half a millimetre of the stock the head stands on. It is read where `q` drops onto the skin, so
    /// it stands straight out of it; it dies away in hollows tighter than a millimetre and on walls the smooth carrier
    /// does not share, where locks raised along converging normals would fold.
    fn pelt(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let core = self.masses(q);
        if core.abs() > 1.6 {
            return 0.0;
        }
        let _ = s;
        let grad = |field: &dyn Fn(P3) -> f64, p: P3, e: f64| -> (P3, f64) {
            let c = field(p);
            let mut g = [0.0; 3];
            let mut lap = 0.0;
            for k in 0..3 {
                let (mut a, mut b) = (p, p);
                a[k] += e;
                b[k] -= e;
                let (fa, fb) = (field(a), field(b));
                g[k] = fa - fb;
                lap += (fa + fb - 2.0 * c) / (e * e);
            }
            (mul(g, 1.0 / len(g).max(1e-12)), lap)
        };
        let masses = |p: P3| self.masses(p);
        let (n, _) = grad(&masses, q, 0.03);
        let f = sub(q, mul(n, core));
        let fs = [f[0].abs(), f[1], f[2]];
        let rho = fs[0].hypot(fs[1] - MOON_U);
        let ang = (fs[1] - MOON_U).atan2(fs[0]).to_degrees();
        let (lo, up) = (lower_lip(ang), upper_lip(ang));
        let lip = lerp(lo[0] + lo[2], up[0] + up[2], smooth(-8.0, 8.0, ang));
        // Over the premolars the cheek's locks run down to the flew's outer edge, so it ends in fur rather than a
        // bare rolled rim; under the nose the lip stays clean. Past the mouth's corners there is no lip to keep off.
        let behind = smooth(FANG_DEG - 6.0, FANG_DEG - 16.0, ang.abs());
        let off_lips = smooth(lip + lerp(0.1, 0.0, behind), lip + lerp(1.3, 0.8, behind), rho).max(smooth(CORNER_DEG, CORNER_DEG - 7.0, ang.abs()));
        let off_face = off_lips * smooth(1.8, 3.4, fs[0]) * smooth(1.3, 2.7, (fs[0] - Self::EYE.0).hypot(fs[1] - Self::EYE.1));
        let cheeks = 0.38 * cheek_fur(fs) * off_face * (1.0 - smooth(STOP_U - 1.9, STOP_U + 0.3, fs[1])) * smooth(MOON_U - 1.5, MOON_U + 1.0, fs[1]);
        let jaw = 0.12 * jaw_fur(fs) * off_lips * (1.0 - smooth(MOON_U - 0.5, MOON_U + 1.5, fs[1])) * smooth(-1.4, -0.6, fs[2]);
        let crown = 0.26 * crown_fur(f) * smooth(STOP_U - 0.1, STOP_U + 1.3, fs[1]) * (1.0 - smooth(3.0, 4.6, fs[0])) * smooth(-0.4, 1.0, fs[2]) * smooth(0.1, 1.0, Self::ear(fs));
        // The throat's locks run up over the jaw fur's lower edge by a millimetre, so no line parts them.
        let throat = 0.26 * throat_fur(f) * smooth(0.4, -0.5, fs[2]) * (1.0 - smooth(MOON_U - 5.0, MOON_U - 3.5, fs[1]));
        // No fur on undersides turned down toward the stock, where locks would overhang and leave slits.
        // Along the jowls' outer foot, where the skin turns tightly round past the stock's edge, the locks lie lower.
        let foot = 1.0 - 0.65 * (1.0 - smooth(-0.3, 0.7, f[2])) * smooth(7.6, 8.4, fs[0]);
        let fur = cheeks.max(jaw).max(crown).max(throat) * self.fur_room(f) * smooth(-0.55, -0.1, n[2]) * foot;
        if fur < 1e-5 {
            return 0.0;
        }
        // Where the skin under the query turns away from the query's own normal, the query sits over a crease and
        // its drop lands on the far wall; locks raised there would float off the near one, so they fade.
        let (nf, lap) = grad(&masses, f, 0.35);
        fur * smooth(-4.0, -0.3, lap) * smooth(0.9, 0.98, dot(n, nf)) * (1.0 - smooth(0.9, 1.6, core.abs()))
    }
    fn head(&self, q: P3) -> f64 {
        let s = [q[0].abs(), q[1], q[2]];
        let core = self.masses(q);
        let near = 1.0 - smooth(0.6, 1.2, core.abs());
        let core = core - self.pelt(q);
        let mut d = core;
        // The stop furrow between the brows, 3 mm up the forehead.
        d += 0.3 * near * (-(q[0] / 0.3).powi(2)).exp() * smooth(STOP_U - 0.35, STOP_U + 0.15, q[1]) * (1.0 - smooth(STOP_U + 2.4, STOP_U + 3.0, q[1]));
        // Three raised folds across the bridge, unequal, bowed toward the eyes, dying out short of the midline on
        // alternating sides; two more from each nose corner back along the lifted lip.
        let folds = fold(q, (2.4, 2.0), (0.3, 2.95), 0.22, 0.15)
            + fold(q, (-2.0, 3.5), (-0.3, 4.04), 0.2, 0.14)
            + fold(q, (1.4, 4.85), (0.3, 4.97), 0.12, 0.12)
            + fold(s, (1.2, 0.85), (2.4, 1.87), -0.1, 0.12)
            // The snarl: three wrinkles fanning up from over the fang toward the eye, where the lip is hauled up.
            + fold(s, (2.25, 1.49), (2.85, 3.4), 0.15, 0.17)
            + fold(s, (3.0, 1.11), (3.85, 2.76), 0.15, 0.15)
            + fold(s, (3.75, 0.6), (4.8, 1.87), 0.12, 0.13);
        d -= folds * near;
        // Nostrils on the nose's front, and the groove under it.
        // The mouth's corners: a notch cut back into each side between the jaws, narrowing and shallowing as it runs
        // back past the lips, so the upper and lower jaws read as two hinged at the cheek.
        {
            let rho = s[0].hypot(s[1] - MOON_U);
            let bearing = (s[1] - MOON_U).atan2(s[0]);
            let t = ((rho - 6.0) / 2.8).clamp(0.0, 1.0);
            // The notch's line bends up toward the ears as it runs back, as a mouth's line rises to its corner, and
            // its walls bow, so no edge of it runs straight.
            let half = (lerp(17.0, 4.5, t.powf(0.8)) + 1.5 * (PI * t).sin()).to_radians();
            let centre = (6.0 * t * t).to_radians();
            let notch = (((bearing - centre).abs() - half) * rho).max(lerp(0.35, 1.3, t) - s[2]).max(rho - 8.8).max(5.2 - rho);
            d = smax(d, -notch, 0.6);
        }
        // Comma nostrils: a pit at each lower corner of the pad, curling up and out along its side.
        let nf = Self::nose_frame(s);
        let nostril = smin(ellipsoid(sub(nf, [0.34, -0.66, -0.12]), [0.17, 0.09, 0.12]), ellipsoid(turn(sub(nf, [0.48, -0.6, -0.14]), 0, 1, 35.0), [0.18, 0.07, 0.09]), 0.06);
        d = smax(d, -nostril, 0.08);
        // The leather stands off the bridge behind a crease, so the nose reads as its own pad.
        d += 0.16 * near * sculpt::bell(q[1], NOSE_U + 0.78, 0.16) * (1.0 - smooth(1.1, 1.6, s[0])) * smooth(2.9, 3.3, q[2]);
        // The groove down the middle of the pad and on under it.
        d += 0.07 * near * (-(s[0] / 0.2).powi(2)).exp() * smooth(NOSE_U + 0.5, NOSE_U + 0.1, s[1]) * smooth(NOSE_U - 0.35, NOSE_U - 0.1, s[1]);
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
        take(format!("ear, {side}"), Self::ear(s), [EAR_TIP[0] * q[0].signum(), EAR_TIP[1], EAR_TIP[2]]);
        for t in &self.teeth {
            // A tooth owns its fillet into the gum.
            let d = t.sdf(q);
            take(t.name.clone(), if d < 0.3 { 0.0 } else { d }, t.tip);
        }
        let tuft = Self::chin_tuft(s);
        take("chin tuft".into(), if tuft < 0.3 { 0.0 } else { tuft }, [0.0, MOON_U - 6.75, -1.15]);
        for (k, sh) in self.sheaths.iter().enumerate() {
            let d = Self::sheath(sh, q);
            take(format!("fang sheath {}", k + 1), if d < 0.25 { 0.0 } else { d }, sh.last().map_or(q, |p| p.0));
        }
        take(format!("brow, {side}"), Self::brow(s), q);
        take("mandible".into(), Self::mandible(s), q);
        take("nose".into(), Self::nose(s), q);
        (best.0, best.1)
    }
    /// A canine sheath: round sections along its stations, blended so its taper runs smooth to a small round point.
    fn sheath(sh: &[(P3, f64)], q: P3) -> f64 {
        sh.windows(2).fold(f64::MAX, |m, w| m.min(round_cone(q, w[0].0, w[1].0, w[0].1, w[1].1)))
    }
    /// Signed distance to the moonstone's hull, a lower bound outside it.
    fn stone_d(&self, q: P3) -> f64 {
        self.stone.0.iter().fold(f64::MIN, |m, (n, c)| m.max(dot(*n, q) - c))
    }
    fn sdf(&self, p: P3) -> f64 {
        let q = self.face(p);
        let d = self.head(q);
        let rho = q[0].hypot(q[1] - MOON_U);
        // The keep-out stands over the table only: below it, under the chin, the stock is the floor, and a cylinder run
        // on down would slit the throat where it meets the apex wall.
        let keep = (KEEP_OUT_MM - rho).min(q[2] + 0.8);
        let mut d = smax(d, keep, 0.45);
        // The fangs' sheaths reach in over the stone past the keep-out, and are cut to the stone's own dome.
        if len(sub(q, self.stone.1)) < self.stone.2 + 2.5 {
            let near: f64 = self.sheaths.iter().map(|sh| Self::sheath(sh, q)).fold(f64::MAX, f64::min);
            if near < 1.0 {
                d = smin(d, near, 0.3).max(0.01 - self.stone_d(q));
            }
        }
        let g = self.stock.at(p);
        // A skirt just under the stock's surface near the head, so the head meets the stock tangentially.
        let d = smin(d, (g + 0.12).max(d - 0.9), 0.45);
        let d = smax(d, -(g + BURY_MM), 0.3);
        smax(d, self.bore + BORE_CLEAR_MM - p[0].hypot(p[1]), 0.2)
    }
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
        let others: Vec<f64> = m.f.iter().filter(|t| t.contains(&k.0) && t.contains(&k.1)).map(|t| {
            let ps = t.map(|x| m.v[x as usize]);
            (0..3).map(|j| len(sub(ps[j], ps[(j + 1) % 3]))).fold(f64::MAX, f64::min)
        }).collect();
        if shown < 12 {
            println!("    artifact fold at ({:.2}, {:.2}, {:.2}): edge {e:.3} mm, shortest edges of its faces {:?}", q[0], q[1], q[2], others.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>());
            shown += 1;
        }
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
    for c in [[4.45, 5.5, 0.2], [4.2, 5.5, 0.2], [5.6, 5.5, -0.05], [5.5, 5.5, -0.05]] {
        let s = c;
        let teeth = wolf.teeth.iter().map(|t| (t.name.clone(), t.sdf(s))).fold(("".to_string(), f64::MAX), |a, b| if b.1 < a.1 { b } else { a });
        let parts = [("cranium", Wolf::cranium(s)), ("cheek", Wolf::cheek(s)), ("jowl", Wolf::jowl(s)), ("upper jaw", Wolf::upper_jaw(s)), ("brow", Wolf::brow(s)), ("muzzle", Wolf::muzzle(s)), ("nose", Wolf::nose(s)), ("mandible", Wolf::mandible(s)), ("gums", Wolf::gums(s)), ("throat", Wolf::throat(s)), ("ear", Wolf::ear(s)), ("tuft", Wolf::chin_tuft(s))];
        let world = [s[0], s[2] + wolf.table, -s[1]];
        println!("    at {c:?}: masses {:.3}, pelt {:.3}, head {:.3}, sdf {:.3}, stock {:.3}, nearest tooth {} {:.3}; {}", wolf.masses(s), wolf.pelt(s), wolf.head(s), wolf.sdf(world), wolf.stock.at(world), teeth.0, teeth.1, parts.iter().map(|(n, v)| format!("{n} {v:.2}")).collect::<Vec<_>>().join(", "));
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
const RUFF_MM: f64 = 0.6;
/// How far the ruff's rows beside the head lean back toward the ears, degrees.
const RUFF_LEAN_DEG: f64 = 42.0;
/// Out along the shoulders each lock still sweeps this far off the band's line, so the rows never stand as a comb.
const RUFF_SWEEP_DEG: f64 = 20.0;
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
    // The painted ruff and the fetter are steep height-field relief: read through one cell so their walls lie straight
    // across the sweep grid instead of stepping row by row. The verdict still reads the true surface.
    d.crisp_relief = true;
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
/// The moonstone stands this much further off the table than its claws' seat asks, so every railless fang's foot finds
/// the table under it, mm.
const MOON_LIFT_MM: f64 = 0.4;
/// The moonstone leans back this far to stand level over the table's slope toward the apex, degrees.
const MOON_TILT_DEG: f64 = 4.0;
/// The fangs' wire, which also sizes their base rail, mm.
const FANG_WIRE_MM: f64 = 1.8;

/// The band anchor, the moonstone and its four fangs.
fn stone_and_fangs(d: &mut RingDesign) -> Result<()> {
    let gem = moonstone();
    let doc = d.cad.get_or_insert_with(Document::default);
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let placement = Placement::Ring { theta_deg: 90.0, across_mm: -MOON_U, height_mm: builders::stand_off_mm(builders::CLAW, gem) + MOON_LIFT_MM, spin_deg: 90.0, tilt_deg: MOON_TILT_DEG, cant_deg: 0.0, level: false };
    doc.append(builders::stone_feature(2, gem, placement))?;
    let mut fangs = builders::feature_on(3, "Fangs", builders::CLAW, 2, json!({"prongs": 4, "wire_mm": FANG_WIRE_MM, "rails": "None", "style": "Fang", "grouping": "Jaws", "tip": "Point", "rise": FANG_RISE}));
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
    stone_and_fangs(&mut d)?;
    let wolf = wolf_of(&d, &a)?;
    let (lo, hi) = sculpt_box(wolf.table);
    let field = |p: P3| wolf.sdf(p);
    let t = Instant::now();
    let mut raw = tetra_mesh(lo, hi, step, &field);
    println!("  marched {} triangles in {:.1} s", raw.f.len(), t.elapsed().as_secs_f64());
    relax(&mut raw, &field, 3);
    let nets = settle(clean_decimate(&raw, SCULPT_FACES), &field, &|p| wolf.in_face_zone(p));
    println!("  relaxed and decimated to {} in {:.1} s", nets.f.len(), t.elapsed().as_secs_f64());
    println!("  face-zone folds of 60 degrees: raw {}, decimated {}", zone_folds(&wolf, &raw), zone_folds(&wolf, &nets));
    fold_causes(&wolf, &nets, &field);
    let sculpt = sculpt::to_mesh(&nets, &field);
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
    println!("  fangs reach inside the moon's disc in plan: {:?} mm", fang_overlap(&wolf, &built));
    let _ = sculpt;
    let mut close = vec![Part::metal(&built.mesh, render::GOLD)];
    close.extend(gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)));
    render::write_png_parts(out.join("sculpt-close-face.png"), &close, 0.0, PI * 0.5, 1100)?;
    render::write_png_parts(out.join("sculpt-close-hero.png"), &close, 0.55, 0.95, 1100)?;
    Ok(())
}

/// A fast look at the head's field: marched coarse and relaxed, no decimation, over a draft build of the stock with its
/// stone and fangs; face, hero and side views.
fn quick_preview(out: &Path, step: f64) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut d = base()?;
    let a = Atlas::of(&d, AW, atlas_rows(&d))?;
    stone_and_fangs(&mut d)?;
    let wolf = wolf_of(&d, &a)?;
    let (lo, hi) = sculpt_box(wolf.table);
    let field = |p: P3| wolf.sdf(p);
    let t = Instant::now();
    let mut raw = tetra_mesh(lo, hi, step, &field);
    relax(&mut raw, &field, 2);
    println!("  marched {} triangles in {:.1} s", raw.f.len(), t.elapsed().as_secs_f64());
    let head = sculpt::to_mesh(&raw, &field);
    let lib = AlphaLibrary::builtin();
    let built = mesh::try_build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..Default::default() })?;
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let mut parts = vec![Part::metal(&built.mesh, render::GOLD), Part::metal(&head, render::GOLD)];
    parts.extend(gems.iter().map(|(m, tint)| Part::tinted_stone(m, *tint)));
    render::write_png_parts(out.join("quick-face.png"), &parts, 0.0, PI * 0.5, 900)?;
    render::write_png_parts(out.join("quick-hero.png"), &parts, 0.55, 0.95, 900)?;
    render::write_png_parts(out.join("quick-side.png"), &parts, PI * 0.5, 0.35, 900)?;
    Ok(())
}

/// The saved export rebuilt from its design file, which `--verify` shows builds bit for bit the same mesh, and its
/// crease census rewritten into its report.
fn recensus(out: &Path) -> Result<()> {
    let saved = library::load_design(out.join("design.ring.json"))?;
    let lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
    let params = BuildParams { theta_steps: EXPORT_THETA, profile_steps: 448, refine: None, ..Default::default() };
    let built = mesh::try_build(&saved, &lib, params)?;
    let mut d = base()?;
    let a = Atlas::of(&d, AW, atlas_rows(&d))?;
    stone_and_fangs(&mut d)?;
    let wolf = wolf_of(&d, &a)?;
    let cuts: Vec<(f64, f64, f64)> = [70.0, 90.0, 110.0]
        .into_iter()
        .map(|theta| section_cut(&built.mesh, theta, &out.join(format!("section-{theta:.0}.png")), 40.0).map(|(o, f)| (theta, o, f)))
        .collect::<Result<_>>()?;
    for (theta, o, f) in &cuts {
        println!("  section at theta {theta}: {o:.2} mm over the hollow, floor under it {f:.2} mm");
    }
    let (census, marks) = census(&wolf, &built, 4);
    for p in marks.iter().take(12) {
        let q = wolf.face(*p);
        println!("  face-zone crease at ({:.2}, {:.2}, {:.2})", q[0], q[1], q[2]);
    }
    println!("  census {census}");
    let path = out.join("report.json");
    let mut report: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    report["census"]["edges_60_deg_by_zone"] = census;
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

/// The hollow alone cut from the stock at the draft build, seen from the palm and cut at theta 70, 90 and 110.
fn hollow_preview(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut d = base()?;
    let a = Atlas::of(&d, AW, atlas_rows(&d))?;
    stone_and_fangs(&mut d)?;
    let wolf = wolf_of(&d, &a)?;
    let (hollow, stats) = hollow_feature(&wolf)?;
    println!("  {stats}");
    d.cad.as_mut().unwrap().append(hollow)?;
    let lib = AlphaLibrary::builtin();
    let built = mesh::try_build(&d, &lib, BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..Default::default() })?;
    render::write_png_parts(out.join("hollow-palm.png"), &[Part::metal(&built.mesh, render::GOLD)], PI, 1.05, 1000)?;
    for theta in [70.0, 90.0, 110.0] {
        let over = section_at(&built.mesh, theta, &out.join(format!("hollow-section-{theta:.0}.png")), 40.0)?;
        println!("  section {theta}: {over:.2} mm over");
    }
    Ok(())
}

/// The head's grid box in world millimetres over a table at `table`.
fn sculpt_box(table: f64) -> (P3, P3) {
    ([-13.5, table - 6.5, -12.5], [13.5, table + 6.8, 15.0])
}

/// How much fuller than its claw a fang's sheath is, mm.
const SHEATH_MM: f64 = 0.12;

/// The claws and the stone of a design that carries its moonstone and fangs, from a quick build of the parts: each
/// claw's axis as stations along it with their radii, fattened into a sheath, and the stone as its hull's planes, all
/// in face coordinates.
fn claws_and_stone(d: &RingDesign, table: f64) -> Result<(Vec<Vec<(P3, f64)>>, (Vec<(P3, f64)>, P3, f64))> {
    let face = |p: P3| [p[0], -p[2], p[1] - table];
    let lib = AlphaLibrary::builtin();
    let params = BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..Default::default() };
    let built = mesh::try_build(d, &lib, params)?;
    let mut sheaths = Vec::new();
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
            let mut vs: Vec<P3> = s.f.iter().zip(&made.named.patch).filter(|(_, p)| **p as usize == pi).flat_map(|(t, _)| t.map(|x| face(s.v[x as usize]))).collect();
            vs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            vs.dedup();
            let tip = vs.iter().copied().fold([0.0, 0.0, f64::MIN], |b, p| if p[2] > b[2] { p } else { b });
            let foot = vs.iter().copied().fold(tip, |b, p| if len(sub(p, tip)) > len(sub(b, tip)) { p } else { b });
            let l = len(sub(tip, foot)).max(1e-6);
            let dir = mul(sub(tip, foot), 1.0 / l);
            let bins = 18;
            let mut acc = vec![([0.0; 3], 0usize); bins];
            for p in &vs {
                let k = ((dot(sub(*p, foot), dir) / l * bins as f64) as usize).min(bins - 1);
                acc[k].0 = add(acc[k].0, *p);
                acc[k].1 += 1;
            }
            let centres: Vec<(usize, P3)> = acc.iter().enumerate().filter(|(_, a)| a.1 > 0).map(|(k, a)| (k, mul(a.0, 1.0 / a.1 as f64))).collect();
            let mut stations: Vec<(P3, f64)> = Vec::new();
            for (k, c) in &centres {
                let ring: Vec<f64> = vs.iter().filter(|p| ((dot(sub(**p, foot), dir) / l * bins as f64) as usize).min(bins - 1) == *k).map(|p| len(sub(*p, *c))).collect();
                let r = ring.iter().sum::<f64>() / ring.len() as f64;
                stations.push((*c, r));
            }
            // Smoothed along the axis, fattened, and run out to a small round point just past the claw's own.
            let n = stations.len();
            let smoothed: Vec<(P3, f64)> = (0..n)
                .map(|i| {
                    let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
                    let c = mul(add(add(stations[a].0, stations[i].0), stations[b].0), 1.0 / 3.0);
                    let r = (stations[a].1 + stations[i].1 + stations[b].1) / 3.0;
                    (if i == 0 || i + 1 == n { stations[i].0 } else { c }, r + SHEATH_MM)
                })
                .collect();
            let mut sh: Vec<(P3, f64)> = smoothed.into_iter().filter(|(_, r)| *r > SHEATH_MM + 0.05).collect();
            let last = sh.last().map_or(tip, |p| p.0);
            let run = sub(tip, last);
            sh.push((add(tip, mul(run, 0.12 / len(run).max(1e-6))), 0.13));
            sheaths.push(sh);
        }
    }
    let gems = ringdesign_core::gems::built_meshes(d, &lib, &built);
    ensure!(gems.len() == 1, "Expected one stone, found {}", gems.len());
    let m = &gems[0].0;
    let v: Vec<P3> = m.vertices.iter().map(|p| face([p.0 as f64, p.1 as f64, p.2 as f64])).collect();
    let centre = mul(v.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / v.len() as f64);
    let radius = v.iter().map(|p| len(sub(*p, centre))).fold(0.0, f64::max);
    let mut planes: Vec<(P3, f64)> = Vec::new();
    for t in m.faces.iter() {
        let [a, b, c] = t.map(|i| v[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let l = len(n);
        if l < 1e-9 {
            continue;
        }
        let mut n = mul(n, 1.0 / l);
        if dot(n, sub(a, centre)) < 0.0 {
            n = mul(n, -1.0);
        }
        let off = dot(n, a);
        if planes.iter().all(|(m, o)| dot(*m, n) < 0.99999 || (o - off).abs() > 1e-4) {
            planes.push((n, off));
        }
    }
    println!("  sheaths over {} fangs; stone hull of {} planes, {radius:.2} mm round", sheaths.len(), planes.len());
    Ok((sheaths, (planes, centre, radius)))
}

/// The wolf's field over the design's own stock, with its stone and fangs in place.
fn wolf_of(d: &RingDesign, a: &Atlas) -> Result<Wolf> {
    let table = a.top;
    let relief = Relief::of(a, table);
    let coarse = Atlas::of(d, 1024, 384)?;
    let (lo, hi) = sculpt_box(table);
    let stock = Stock::of(&coarse.samples, |s| s.p[1] >= 4.0, lo, hi, 0.25, 1.2, |p| p[1] - table < relief.at(p[0], -p[2]));
    let (sheaths, stone) = claws_and_stone(d, table)?;
    Ok(Wolf::new(table, d.inner_radius_mm(), relief, stock, sheaths, stone))
}

/// Metal kept over the hollow under the head, mm.
const HOLLOW_WALL_MM: f64 = 1.0;
/// The hollow's mouth into the finger hole, inside the pocket's own: half its width across the face, and its ends
/// along the ring, face mm.
const HOLLOW_OPENING: (f64, f64, f64) = (3.1, -7.0, 5.0);

/// The hollow scooped under the head from the finger hole: over each point of the head's plan, up to where a ball of
/// [`HOLLOW_WALL_MM`] still clears the first air above the bore, within a rounded footprint; as a cut part.
fn hollow_feature(wolf: &Wolf) -> Result<(Feature, Value)> {
    let t = Instant::now();
    let (table, bore) = (wolf.table, wolf.bore);
    let union = |q: P3| {
        let p = [q[0], q[2] + table, -q[1]];
        wolf.stock.at(p).min(wolf.sdf(p))
    };
    let floor = |x: f64| (bore * bore - x * x).max(0.0).sqrt() - table;
    let air = Heights::first_air([-9.0, -12.0], [9.0, 9.0], 0.1, 8.0, |x, _| floor(x), union);
    // The roof stays the wall's thickness from the surface in every direction, plus a fifth for what the blur lifts.
    let eroded = air.ball_eroded(HOLLOW_WALL_MM + 0.2);
    // Blurred smooth, then held under the eroded roof by a soft minimum so no valley the blur fills thins the wall.
    let blurred = eroded.blurred(0.6);
    let roof_at = |x: f64, u: f64| smin(blurred.at(x, u), eroded.at(x, u) + 0.15, 0.3);
    if std::env::var("FENRIR_HOLLOW").is_ok() {
        for k in 0..=40 {
            let u = -10.0 + k as f64 * 0.5;
            println!("    x 0, u {u:5.1}: first air {:6.2}, roof {:6.2}, floor {:6.2}", air.at(0.0, u), roof_at(0.0, u), floor(0.0));
        }
    }
    let footprint = |q: P3| {
        let (x, u) = (q[0].abs() - 7.2, (q[1] + 0.5).abs() - 8.6);
        let r = 2.2;
        len([x.max(-r) + r, u.max(-r) + r, 0.0]).max(0.0) + x.max(u).min(-r) + r - r
    };
    // The mouth it opens into the finger hole: an oval as a superellipse, curved all round with no corner tighter than
    // 2 mm; the pocket widens past it only from 1.05 mm over the bore up, so the floor it runs on over the finger is
    // 1 mm and more.
    let (ox, ou0, ou1) = HOLLOW_OPENING;
    let opening = |q: P3, r: f64| {
        let (x, u) = (q[0].abs() / ox, (q[1] - 0.5 * (ou0 + ou1)).abs() / (0.5 * (ou1 - ou0)));
        let f = (x.powf(2.6) + u.powf(2.6)).powf(1.0 / 2.6);
        (f - 1.0) * ox - 4.0 * smooth(bore + 1.05, bore + 1.6, r)
    };
    // A round of 0.5 mm where the pocket's walls meet the bore, entered at 50 degrees rather than tangent so the bore
    // and the round never run together: the opening's edge is broken, not sharp.
    let flare = |r: f64| {
        let k = (r - bore + 0.12).clamp(0.0, 0.5);
        0.5 - (0.25 - (0.5 - k) * (0.5 - k)).max(0.0).sqrt()
    };
    let field = |p: P3| -> f64 {
        let q = wolf.face(p);
        let r = p[0].hypot(p[1]);
        let e = flare(r);
        // The roof rises no more than 0.3 mm over the table's plane into the head, and everywhere stays the wall's
        // thickness under the first air.
        let top = smin(roof_at(q[0], q[1]), 0.3, 0.6) + e;
        smax(smax((q[2] - top).max(bore - 0.3 - r), footprint(q) - e, 0.5), opening(q, r) - e, 0.6)
    };
    let (lo, hi) = ([-9.0, bore - 0.6, -11.0], [9.0, table + 1.0, 13.0]);
    let raw = tetra_mesh(lo, hi, 0.13, &field);
    {
        // Where the pocket meets the bore, in face x and u.
        let rim: Vec<P3> = raw.v.iter().filter(|p| (p[0].hypot(p[1]) - bore).abs() < 0.06).map(|p| wolf.face(*p)).collect();
        let ext = |k: usize| rim.iter().map(|p| p[k]).fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
        println!("  hollow opening on the bore: x {:?}, u {:?}", ext(0), ext(1));
    }
    // Only the pockets that open into the finger hole are kept: a sealed one would cast solid.
    let (mut raw, dropped) = sculpt::open_shells(&raw, |p| p[0].hypot(p[1]) < bore);
    relax(&mut raw, &field, 3);
    let mut nets = decimate(&raw, 22_000, 1e-2, 3.0, 20.0, 30.0);
    if csg::self_crossings(&nets) > 0 {
        nets = clean_decimate(&raw, 14_000);
    }
    // The same sliver collapse, edge flips and fold smoothing as the head, so no burr stands off the pocket's walls.
    let nets = settle(nets, &field, &|_| true);
    let (bad, volume) = sculpt::closure(&nets);
    ensure!(bad == 0 && volume > 0.0, "The hollow does not close: {bad} open edges");
    let crossings = csg::self_crossings(&nets);
    ensure!(crossings == 0, "The hollow crosses itself {crossings} times");
    let mesh = sculpt::packed(&nets)?;
    println!("  hollow: {} triangles, {:.0} mm3 in {:.1} s, {} KB packed; {dropped} sealed pockets left out", nets.f.len(), volume, t.elapsed().as_secs_f64(), mesh.data.len() / 1024);
    let stats = json!({"wall_mm": HOLLOW_WALL_MM, "triangles": nets.f.len(), "volume_mm3": volume, "packed_bytes": mesh.data.len()});
    let recipe = cad::stored::Recipe {
        kernel: "fenrir".into(),
        op: "hollow".into(),
        params: json!({"field": "bestiarium_fenrir.rs hollow_feature", "wall_mm": HOLLOW_WALL_MM}),
        digest: String::new(),
    };
    let component = Component { attach: Attach::Cut, stage: Stage::Cast, placement: Placement::Free, ..Component::default() };
    Ok((Feature { id: 5, name: "Hollow under the head".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh }, component }, stats))
}

/// The head as a stored part, joined to the stock.
fn head_feature(wolf: &Wolf) -> Result<(Feature, Value)> {
    let t = Instant::now();
    let (lo, hi) = sculpt_box(wolf.table);
    let field = |p: P3| wolf.sdf(p);
    let raw = tetra_mesh(lo, hi, SCULPT_STEP, &field);
    // Only the shell that stands on the stock is the head: a fleck of fur the field leaves floating off it would cast
    // as a loose crumb, so it is left out and counted.
    let (mut raw, islands) = sculpt::open_shells(&raw, |p| p[1] < wolf.table - 0.5);
    relax(&mut raw, &field, 3);
    let nets = settle(clean_decimate(&raw, SCULPT_FACES), &field, &|p| wolf.in_face_zone(p));
    let (bad, volume) = sculpt::closure(&nets);
    ensure!(bad == 0 && volume > 0.0, "The head does not close: {bad} open edges");
    let crossings = csg::self_crossings(&nets);
    ensure!(crossings == 0, "The head crosses itself {crossings} times");
    let mesh = sculpt::packed(&nets)?;
    // A digest of the packed mesh, so two runs can show the sculpt is the same bit for bit.
    let digest = format!("{:016x}", mesh.data.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)));
    let stats = json!({"packed_fnv1a": digest, "loose_islands_left_out": islands, "marching_step_mm": SCULPT_STEP, "raw_triangles": raw.f.len(), "triangles": nets.f.len(), "vertices": nets.v.len(), "volume_mm3": volume, "packed_bytes": mesh.data.len(), "self_crossings": crossings, "seconds": t.elapsed().as_secs_f64()});
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
    /// How far the winning lock stands over the next one here, 0..1: small where two locks meet.
    lead: f64,
}

/// Flame locks on a chart where `along` runs with the fur and `across` over it: rows `pitch` apart, locks about `length`
/// long and shingled half a lock apart with their roots jittered, each bowed in an S of `bow` of its length, widest a third of the way along and
/// drawn out to a point at both ends; `shape` gives a lock's height from its share along, its place across and its
/// half-width. Lengths vary by a quarter and headings by up to ten degrees; neighbours join by a soft maximum of `soft`.
#[allow(clippy::too_many_arguments)]
fn flames(along: f64, across: f64, pitch: f64, length: f64, bow: f64, soft: f64, seed: i64, shape: &dyn Fn(f64, f64, f64) -> f64) -> Flame {
    let step = length * 0.45;
    let row = (across / pitch).round() as i64;
    let mut out = Flame::default();
    let mut acc = 0.0f64;
    let mut second = 0.0f64;
    // A lock bowed and tilted off its row reaches up to two rows over, so three rows each side are searched: a
    // narrower search cuts locks off along the row lines, which read as terraces.
    let reach = ((bow * length * 1.25 + 0.2 * length * 1.25 + 0.6 * pitch) / pitch).ceil() as i64;
    for j in row - reach..=row + reach {
        let shift = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 * step };
        let i0 = ((along - shift) / step).floor() as i64;
        for i in i0 - 4..=i0 + 1 {
            let len_i = length * (0.75 + 0.5 * skin::hash(i * 7 + seed, j * 13 - seed));
            // Headings in three classes a lattice apart, so no two neighbours run within five degrees of each other.
            let axial = i - (j - j.rem_euclid(2)) / 2;
            let class = (axial - j).rem_euclid(3) as f64 - 1.0;
            let tilt = (class * 9.0 + (skin::hash(j * 5 + seed, i * 11 + 3 * seed) - 0.5) * 2.0).to_radians();
            // Each lock's root is jittered along and across its row, so neither the points nor the slots line up.
            let slide = (skin::hash(i * 3 + 2 * seed, j * 17 + seed) - 0.5) * 0.5 * step;
            let sway = (skin::hash(j * 19 - seed, i * 5 + seed) - 0.5) * 0.3 * pitch;
            let (ds, dc) = (along - (i as f64 * step + shift + slide), across - (j as f64 * pitch + sway));
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
                second = out.h;
                out = Flame { h, across: q, half, lead: 0.0 };
            } else {
                second = second.max(h);
            }
            acc = if soft > 0.0 { smax(acc, h, soft) } else { acc.max(h) };
        }
    }
    out.lead = out.h - second;
    if soft > 0.0 {
        out.h = acc;
    }
    out
}

/// A painted lock: a cosine-squared hump across, so its edges meet the skin and its neighbours tangentially and no
/// outline stands as a step; swelling from its root and drawn out to its point over the last half.
fn painted_lock(t: f64, q: f64, _half: f64) -> f64 {
    (q * PI * 0.5).cos().max(0.0).powi(2) * smooth(0.0, 0.3, t) * (1.0 - smooth(0.35, 1.0, t))
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
    // Overlapping locks join by a soft maximum a third of the relief wide, so no shingle leaves a terrace on the one
    // under it; the hair lines keep to the crown of the lock that leads and die where it meets another.
    let f = flames(along, across, 1.6, 7.5, 0.2, 0.3, 17, &painted_lock);
    let lines = 0.5 - 0.5 * (2.0 * PI * f.across * f.half / 0.22).cos();
    let hair = lines * smooth(0.45, 0.75, f.h) * smooth(0.45, 0.65, f.half) * smooth(0.15, 0.35, f.lead) * (1.0 - smooth(0.55, 0.8, f.across.abs()));
    (0.1 + 0.9 * smin(f.h, 1.0, 0.15), hair)
}

/// A gradient across a strand, ink rising as a cosine from nothing at both edges to full at the middle.
fn strand_gradient(id: &str, from: (f64, f64), to: (f64, f64)) -> String {
    let stops: String = (0..=16)
        .map(|k| {
            let o = k as f64 / 16.0;
            let ink = (PI * (o - 0.5)).cos().max(0.0);
            let g = (255.0 * (1.0 - ink)).round() as u8;
            format!(r##"<stop offset="{o:.4}" stop-color="#{g:02x}{g:02x}{g:02x}"/>"##)
        })
        .collect();
    format!(r##"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{:.4}" y1="{:.4}" x2="{:.4}" y2="{:.4}">{stops}</linearGradient>"##, from.0, from.1, to.0, to.1)
}

/// A mask that lets ink through in full along the cord's middle and fades it toward both edges, so every strand turns
/// under the cord's round.
fn cord_mask(id: &str, w: f64, h: f64) -> String {
    let stops: String = (0..=16)
        .map(|k| {
            let o = k as f64 / 16.0;
            let keep = (PI * (o - 0.5)).cos().max(0.0).powf(0.45);
            let g = (255.0 * keep).round() as u8;
            format!(r##"<stop offset="{o:.4}" stop-color="#{g:02x}{g:02x}{g:02x}"/>"##)
        })
        .collect();
    format!(
        r##"<linearGradient id="{id}g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="{h:.4}">{stops}</linearGradient><mask id="{id}" maskUnits="userSpaceOnUse" x="{:.4}" y="0" width="{:.4}" height="{h:.4}"><rect x="{:.4}" y="0" width="{:.4}" height="{h:.4}" fill="url(#{id}g)"/></mask>"##,
        -w,
        3.0 * w,
        -w,
        3.0 * w
    )
}

/// The Gleipnir cord tile: rounded strands laid diagonally across the band like a rope's, each a cosine hump with its
/// walls under 60 degrees, turning under at the band's edges; drawn in metal millimetres and squeezed by the chart's squash.
fn gleipnir_svg(cell_w: f64, cell_h: f64, squash: f64) -> String {
    let (w, h) = (cell_w, cell_h * squash);
    let lay = 40f64.to_radians();
    // Strands one tile apart along the ring; their width across themselves fills the pitch less a groove.
    let run = h / lay.tan();
    let pitch = w * lay.sin();
    let half = 0.5 * pitch * 0.94;
    let along = half / lay.sin();
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cell_w:.4} {cell_h:.4}" width="{cell_w:.4}" height="{cell_h:.4}"><rect width="{cell_w:.4}" height="{cell_h:.4}" fill="#fff"/><defs>{}"##,
        cord_mask("m", w, h)
    );
    let bands: Vec<f64> = (-4i64..=((run / w).ceil() as i64 + 2)).map(|k| -(k as f64) * w).collect();
    for (i, u0) in bands.iter().enumerate() {
        // Across the band: from its lower-left edge to its upper-right, perpendicular to the strand.
        let mid = (u0 + 0.5 * run, 0.5 * h);
        let n = (lay.sin(), -lay.cos());
        s += &strand_gradient(&format!("s{i}"), (mid.0 - half * n.0, mid.1 - half * n.1), (mid.0 + half * n.0, mid.1 + half * n.1));
    }
    s += &format!(r##"</defs><g transform="scale(1 {:.6})"><g mask="url(#m)">"##, 1.0 / squash.max(0.05));
    for (i, u0) in bands.iter().enumerate() {
        let (a0, a1) = (u0 - along, u0 + along);
        s += &format!(
            r##"<path d="M{:.4} 0 L{:.4} 0 L{:.4} {h:.4} L{:.4} {h:.4} Z" fill="url(#s{i})"/>"##,
            a0,
            a1,
            a1 + run,
            a0 + run
        );
    }
    s += "</g></g></svg>";
    s
}

/// The fetter's binding where it meets the fur: five rounded turns of cord wrapped round it at 75 degrees, the last
/// one's end tucked back under the turns.
fn binding_svg(w: f64, h: f64, squash: f64) -> String {
    let hm = h * squash;
    let lean = 15f64.to_radians();
    let slant = hm * lean.tan();
    let turns = 5;
    let (wrap, gap) = (0.55, 0.12);
    let used = turns as f64 * (wrap + gap) - gap;
    let x0 = 0.5 * (w - used - slant);
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.4} {h:.4}" width="{w:.4}" height="{h:.4}"><rect width="{w:.4}" height="{h:.4}" fill="#fff"/><defs>{}"##,
        cord_mask("m", w, hm)
    );
    let n = (lean.cos(), -lean.sin());
    for k in 0..turns {
        let mid = (x0 + k as f64 * (wrap + gap) + 0.5 * wrap + 0.5 * slant, 0.5 * hm);
        let r = 0.5 * wrap * lean.cos();
        s += &strand_gradient(&format!("t{k}"), (mid.0 - r * n.0, mid.1 - r * n.1), (mid.0 + r * n.0, mid.1 + r * n.1));
    }
    // The tucked end: a short tapering tail running back from the last turn's foot and under the one before it.
    let last = x0 + (turns - 1) as f64 * (wrap + gap);
    let tail_mid = (last + 0.1 + 0.5 * slant, 0.8 * hm);
    s += &strand_gradient("tail", (tail_mid.0, tail_mid.1 - 0.25), (tail_mid.0, tail_mid.1 + 0.25));
    s += &format!(r##"</defs><g transform="scale(1 {:.6})"><g mask="url(#m)">"##, 1.0 / squash.max(0.05));
    for k in 0..turns {
        let a = x0 + k as f64 * (wrap + gap);
        s += &format!(
            r##"<path d="M{:.4} 0 L{:.4} 0 L{:.4} {hm:.4} L{:.4} {hm:.4} Z" fill="url(#t{k})"/>"##,
            a + slant,
            a + slant + wrap,
            a + wrap,
            a
        );
    }
    s += &format!(
        r##"<path d="M{:.4} {:.4} C{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} L{:.4} {:.4} C{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} Z" fill="url(#tail)"/>"##,
        last + wrap,
        0.62 * hm,
        last + wrap - 0.4,
        0.64 * hm,
        last - 0.6,
        0.7 * hm,
        last - 1.1,
        0.8 * hm,
        last - 1.1,
        0.8 * hm + 0.02,
        last - 0.6,
        0.9 * hm,
        last - 0.4,
        0.96 * hm,
        last + wrap,
        0.98 * hm
    );
    s += "</g></g></svg>";
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

/// A Gaussian blur of `sigma` texels, wrapping round the ring and clamped across the band.
fn blur_alpha(a: &mut Alpha, sigma: f64) {
    let (w, h) = (a.width, a.height);
    let k = (3.0 * sigma).ceil() as i64;
    let wt: Vec<f64> = (-k..=k).map(|d| (-0.5 * (d as f64 / sigma).powi(2)).exp()).collect();
    let sum: f64 = wt.iter().sum();
    let src = a.data.clone();
    let along: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|c| {
            let (x, y) = ((c % w) as i64, c / w);
            (-k..=k).zip(&wt).map(|(d, t)| t * src[y * w + (x + d).rem_euclid(w as i64) as usize] as f64).sum::<f64>() as f32 / sum as f32
        })
        .collect();
    a.data = (0..w * h)
        .into_par_iter()
        .map(|c| {
            let (x, y) = (c % w, (c / w) as i64);
            (-k..=k).zip(&wt).map(|(d, t)| t * along[(y + d).clamp(0, h as i64 - 1) as usize * w + x] as f64).sum::<f64>() as f32 / sum as f32
        })
        .collect();
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
    stone_and_fangs(&mut d)?;
    let wolf = wolf_of(&d, &a)?;
    let (head, head_stats) = head_feature(&wolf)?;
    d.cad.as_mut().unwrap().append(head)?;
    let (hollow, hollow_stats) = hollow_feature(&wolf)?;
    d.cad.as_mut().unwrap().append(hollow)?;
    let room = fold_room(&a);
    let paint = |d: &mut RingDesign, caps: &[P3]| -> Result<AlphaLibrary> {
        let mut lib = AlphaLibrary::builtin();
        d.layers.layers.retain(|e| e.name != "Ruff" && e.name != "Graver's hair lines");
        // No paint round the moon on the table, where the fangs find their footing; the walls below may carry it.
        let clear = |s: &Sample| {
            let r = s.p[0].hypot(s.p[1]);
            let q = wolf.face(s.p);
            let mouth = 1.0 - (1.0 - smooth(7.0, 8.2, q[0].hypot(q[1] - MOON_U))) * smooth(-2.2, -1.8, q[2]);
            smooth(a.bore + 1.0, a.bore + 1.5, r) * smooth(-0.05, 0.3, wolf.sdf(s.p)) * off_folds(s.p, caps) * mouth
        };
        // Round the ring the ruff flows along it from the head; on the apex wall under the throat it flows down it.
        let pelt = |s: &Sample| -> (f64, f64) {
            let h = hide.at(s);
            let q = wolf.face(s.p);
            let apex = smooth(-7.8, -8.8, q[1]) * smooth(-0.6, -1.6, q[2]) * (1.0 - smooth(5.0, 6.5, q[0].abs()));
            // Beside the head the ruff's first rows lean back toward the ears as the head's own cheek fur does, and
            // straighten out along the ring by 7 mm on; the chart's `across` grows toward +z, away from the ears.
            let lean = lerp(RUFF_SWEEP_DEG, RUFF_LEAN_DEG, 1.0 - smooth(11.0, 18.0, h.along.abs())).to_radians();
            let (sn, cs) = lean.sin_cos();
            let (al, ac) = (h.along.abs() * cs - h.across * sn, h.along.abs() * sn + h.across * cs);
            let (lock, line) = fur(al, ac);
            let (down, down_line) = fur(-q[2] - 0.6, q[0]);
            ((lock * (1.0 - apex)).max(down * apex), (line * (1.0 - apex)).max(down_line * apex))
        };
        // The fold room caps the relief by a soft minimum, so where it bites the lock rounds over instead of flattening.
        let mut ruff = a.paint("Ruff", |s| smin(pelt(s).0 * clear(s), room[s.i] / RUFF_MM, 0.15).max(0.0));
        // Each texel reads the lock that wins at its own point, so where two locks meet the crease between them aliases
        // into a saw-tooth one texel deep; a blur of a texel and a half takes it out and leaves the locks' shapes.
        blur_alpha(&mut ruff, 1.5);
        portable(d, &mut lib, ruff, RUFF_MM, window(90.0, 262.0), false)?;
        let hair = a.paint("Graver's hair lines", |s| pelt(s).1 * clear(s));
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
    let mut knots = DecalLayer { alpha: "Gleipnir binding".into(), decals: Vec::new(), feather_mm: 0.35, invert: false };
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
        "hollow": hollow_stats,
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
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, params: BuildParams, edge: usize, table: f64) -> Result<()> {
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
    // Close-ups framed on the whole ring, so every crease keeps its own normals: the head face-on, and the ruff on
    // the right shoulder and at the palm's binding.
    let rim = |theta: f64| -> [f64; 3] {
        let t = theta.to_radians();
        let r = built.mesh.vertices.iter().filter(|p| ((p.1 as f64).atan2(p.0 as f64) - t).abs() < 0.01 && (p.2 as f64).abs() < 0.6).map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(0.0, f64::max);
        [r * t.cos(), r * t.sin(), 0.0]
    };
    render::write_png_framed(out.join("close-head.png"), &parts, 0.0, PI * 0.5, render::Framing::new([0.0, table + 2.0, -5.0], 7.5), edge)?;
    render::write_png_framed(out.join("close-ruff.png"), &parts, render::yaw_facing(40.0), PI * 0.5, render::Framing::new(rim(40.0), 4.0), edge)?;
    render::write_png_framed(out.join("close-binding.png"), &parts, render::yaw_facing(236.0), PI * 0.5, render::Framing::new(rim(236.0), 3.0), edge)?;
    // The 300 px read sheet: hero, face, shoulder, palm and reverse side by side, as a jeweller sees them small.
    let views = [(0.55, 0.95), (0.0, PI * 0.5), (-0.9, 0.62), (PI, 1.05), (1.6, 0.8)];
    let tiles: Vec<Vec<u8>> = views.iter().map(|&(yaw, pitch)| render::render_parts_ss(&parts, yaw, pitch, 300, 300, 3)).collect();
    let mut sheet = vec![0u8; 300 * views.len() * 300 * 3];
    for (k, tile) in tiles.iter().enumerate() {
        for y in 0..300 {
            let row = (y * 300 * views.len() + k * 300) * 3;
            sheet[row..row + 900].copy_from_slice(&tile[y * 900..(y + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, (300 * views.len()) as u32, 300, image::ColorType::Rgb8)?;
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
    let cuts: Vec<(f64, f64, f64)> = [70.0, 90.0, 110.0]
        .into_iter()
        .map(|theta| section_cut(&built.mesh, theta, &out.join(format!("section-{theta:.0}.png")), 40.0).map(|(o, f)| (theta, o, f)))
        .collect::<Result<_>>()?;
    let over_hollow = cuts[1].1;
    let least_floor = cuts.iter().map(|c| c.2).fold(f64::MAX, f64::min);
    let least_over = cuts.iter().map(|c| c.1).fold(f64::MAX, f64::min);
    for (theta, o, f) in &cuts {
        println!("  section at theta {theta}: {o:.2} mm over the hollow, floor under it {}", if *f < 50.0 { format!("{f:.2} mm") } else { "none".into() });
    }
    let voids = internal_voids(&built.mesh);
    for (vol, c) in voids.iter().take(8) {
        let q = wolf.face(*c);
        println!("    internal void {:.4} mm3 at face ({:.2}, {:.2}, {:.2})", -vol, q[0], q[1], q[2]);
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
        ("hollow under the head: at most 32 g in 18k, 1.0 mm or more over it at theta 90", grams <= 32.0 && over_hollow >= 1.0 && over_hollow < 50.0),
        ("hollow sections at theta 70, 90 and 110: 1.0 mm or more over it and under it", least_over >= 1.0 && least_floor >= 1.0),
        ("no closed internal voids in the finished metal", voids.is_empty()),
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
        "hollow": {"wall_mm": HOLLOW_WALL_MM, "section_90_min_metal_over_mm": over_hollow, "section_png": "section-90.png", "more_sections": ["section-70.png", "section-110.png"], "sections": cuts.iter().map(|(t, o, f)| json!({"theta_deg": t, "min_metal_over_mm": o, "min_floor_under_mm": if *f < 50.0 { json!(f) } else { Value::Null }})).collect::<Vec<_>>()},
        "internal_voids": voids.len(),
        "internal_void_sites": voids.iter().map(|(v, c)| json!({"volume_mm3": -v, "face_mm": wolf.face(*c)})).collect::<Vec<_>>(),
        "jaws": jaw_measures(&wolf),
        "fangs": {"rise_of_dome": FANG_RISE, "rails": "None", "wire_mm": FANG_WIRE_MM, "reach_inside_moon_disc_mm": fang_overlap(&wolf, &built)},
        "pattern": {"validation": pattern_validation, "quality": pattern_quality, "self_crossings": pattern_cross, "release": {"obstructions": inspection.release.obstructions.len()}},
        "design": {"bytes": design_bytes, "format_version": format, "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), "stamps": d.stamps.len(), "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "cold_reload": cold,
        "land_widths": {"floor_mm": 0.8, "head": lands.head, "fangs_and_rail": lands.fangs, "head_min_section_mm": lands.head_min, "exceptions": lands.exceptions, "unnamed": lands.unnamed},
        "census": {"edges_60_deg_by_zone": lands.census, "seam_crease_run_mm": lands.seam_run},
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
    });
    let mut report = report;
    // The draft build's own report rides along in the export's, so one file carries every gate at both resolutions.
    if let Some(path) = std::env::args().find_map(|a| a.strip_prefix("--draft-report=").map(String::from)) {
        let draft: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        report["draft"] = json!({"build": draft["build"], "gates": draft["gates"], "geometry": draft["geometry"], "made_parts": draft["made_parts"], "field": draft["field"], "dfm": draft["dfm"], "stones": draft["stones"], "grams_18k": draft["grams_18k"], "hollow": draft["hollow"], "internal_voids": draft["internal_voids"], "land_widths": draft["land_widths"], "pattern": draft["pattern"], "jaws": draft["jaws"]});
    }
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
    renders(out, &d, &lib, &built, params, if draft { 1000 } else { 1600 }, wolf.table)?;
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
    } else if feature.contains("incisor") || feature.contains("premolar") || feature.contains("molar") || feature.contains("carnassial") {
        Some((0.6, "tooth point: the last 0.6 mm of each tooth; cast from a root of 1.0 mm or more and polished"))
    } else if feature == "nose" {
        Some((0.0, "nostril rims: the comma nostrils leave their rims at 0.65 mm and more; cast in place and chased open at the bench"))
    } else if feature.contains("tuft") {
        Some((0.9, "tuft point: the last 0.9 mm of the chin tuft; chased at the bench"))
    } else if feature.contains("sheath") {
        Some((2.4, "fang point: the last 2.4 mm of each canine sheath runs out over the moonstone's dome round its claw; cast in place, closed onto the dome with the claw and polished at the bench"))
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
    let (census, crease_at) = census(wolf, built, head_id);
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
    // FENRIR_DEBUG=<feature> clusters only that feature's thin samples.
    let only = std::env::var("FENRIR_DEBUG").ok().filter(|v| v != "1");
    for (k, t) in thin_at.iter().enumerate() {
        if only.as_ref().is_some_and(|w| !t.2.contains(w.as_str())) {
            continue;
        }
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
    if let Ok(who) = std::env::var("FENRIR_RAYS") {
        for r in rows.iter().filter(|r| r.1 < 0.8 && r.0.contains(who.as_str()) && r.2 > 0.8).take(8) {
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

/// The mouth measured off the head's field: each corner's gap between the jaws, as arc at the lips' radius, and how far
/// each lip's inner edge strays from a constant radius over its steadiest 90 degrees.
fn jaw_measures(wolf: &Wolf) -> Value {
    // The head's top over face point (x, u), searching down from above.
    let top = |x: f64, u: f64| {
        let mut h = 4.0;
        while h > -0.5 && wolf.head([x, u, h]) > 0.0 {
            h -= 0.02;
        }
        h
    };
    // The furthest the head's own field reaches past the moon's axis on the midline, fur and tuft included, and the
    // nose leather's front.
    let chin = {
        let mut u = MOON_U - 9.0;
        while u < MOON_U - 5.0 && !(0..=120).any(|k| wolf.head([0.0, u, -3.0 + 0.05 * k as f64]) < 0.0) {
            u += 0.01;
        }
        MOON_U - u
    };
    let nose_front = {
        let mut u = MOON_U + 5.0;
        while u < STOP_U && !(0..=120).any(|k| Wolf::nose([0.0, u, -1.0 + 0.05 * k as f64]) < 0.0) {
            u += 0.01;
        }
        u
    };
    let rho = 6.3;
    let lip_at = |deg: f64| {
        let p = rim(rho, deg, 0.0);
        top(p[0], p[1]) > 0.8
    };
    let gap = |from: f64, to: f64| -> f64 {
        let n = 400;
        let open = (0..=n).filter(|k| !lip_at(from + (to - from) * *k as f64 / n as f64)).count();
        open as f64 / n as f64 * (to - from).abs().to_radians() * rho
    };
    let spread = |f: &dyn Fn(f64) -> f64, from: f64, to: f64| -> f64 {
        let mut best = f64::MAX;
        let mut a = from;
        while a + 90.0 <= to + 1e-9 {
            let rs: Vec<f64> = (0..=90).map(|k| f(a + k as f64)).collect();
            best = best.min(rs.iter().cloned().fold(f64::MIN, f64::max) - rs.iter().cloned().fold(f64::MAX, f64::min));
            a += 1.0;
        }
        best
    };
    // Each lip's width across, least and most, from corner to midline.
    let widths = |lip: &dyn Fn(f64) -> [f64; 4], sign: f64| -> (f64, f64) {
        (0..=140).map(|k| lip(sign * (CORNER_DEG + (90.0 - CORNER_DEG) * k as f64 / 140.0))[2]).fold((f64::MAX, f64::MIN), |(lo, hi), w| (lo.min(w), hi.max(w)))
    };
    // Both lips run from one corner to the other; the field is mirrored across the midline at 90 degrees.
    let fold_over = |deg: f64| if deg > 90.0 { 180.0 - deg } else { deg };
    let upper = |deg: f64| upper_lip(fold_over(deg))[0];
    let lower = |deg: f64| lower_lip(-fold_over(deg))[0];
    json!({
        "corner_gap_right_mm": gap(-40.0, 40.0),
        "corner_gap_left_mm": gap(140.0, 220.0),
        "lip_radius_mm": rho,
        "upper_lip_least_spread_over_90_deg_mm": spread(&upper, CORNER_DEG, 180.0 - CORNER_DEG),
        "lower_lip_least_spread_over_90_deg_mm": spread(&lower, CORNER_DEG, 180.0 - CORNER_DEG),
        "upper_lip_width_min_mm": widths(&upper_lip, 1.0).0,
        "upper_lip_width_max_mm": widths(&upper_lip, 1.0).1,
        "lower_lip_width_min_mm": widths(&lower_lip, -1.0).0,
        "lower_lip_width_max_mm": widths(&lower_lip, -1.0).1,
        "chin_reach_from_moon_axis_mm": jaw_out(-90.0),
        "chin_reach_measured_mm": chin,
        "nose_front_u_mm": nose_front,
        "stop_u_mm": STOP_U,
        "muzzle_stop_to_nose_mm": STOP_U - nose_front,
        "lower_jaw_to_muzzle_ratio": chin / (STOP_U - nose_front),
    })
}

/// How far each claw reaches inside the moon's girdle circle seen from over the face, mm.
fn fang_overlap(wolf: &Wolf, built: &mesh::BuildResult) -> Vec<f64> {
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
            let inner = s.f.iter().zip(&made.named.patch).filter(|(_, p)| **p as usize == pi).flat_map(|(t, _)| t.map(|x| wolf.face(s.v[x as usize]))).map(|q| q[0].hypot(q[1] - MOON_U)).fold(f64::MAX, f64::min);
            out.push(((MOON_MM * 0.5 - inner) * 100.0).round() / 100.0);
        }
    }
    out
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

/// Dihedral census of the finished mesh by zone: edges at or over 60 and 90 degrees, their length, and 1 mm cells. The
/// head's own surface is split by feature; everything else is the band with its painted layers, or the fangs.
fn census(wolf: &Wolf, built: &mesh::BuildResult, head_id: ringdesign_core::sketch::Id) -> (Value, Vec<P3>) {
    use std::collections::{HashMap, HashSet};
    let m = &built.mesh;
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
    let owner = |x: u32| m.origin.get(x as usize).and_then(|o| built.parts.feature_of(*o));
    let ear_tips = [EAR_TIP, [-EAR_TIP[0], EAR_TIP[1], EAR_TIP[2]]];
    let mut face_marks: Vec<P3> = Vec::new();
    let mut zones: HashMap<&str, (usize, f64, usize, HashSet<[i64; 3]>)> = HashMap::new();
    // Every zone is listed, a clean one with zeros.
    for zone in ["band and painted layers", "fang claws", "hollow under the head", "other made parts", "ear tips", "ears", "teeth", "mouth, gums and lips", "nose pad and nostrils", "muzzle", "brow", "eyes", "crown", "lower jaw and chin", "cheeks", "head flanks and throat"] {
        zones.insert(zone, (0, 0.0, 0, HashSet::new()));
    }
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
        let s = [q[0].abs(), q[1], q[2]];
        let rho = q[0].hypot(q[1] - MOON_U);
        let bearing = (q[1] - MOON_U).atan2(q[0]).to_degrees();
        let (oa, ob) = (owner(*a), owner(*b));
        let on_head = oa == Some(head_id) || ob == Some(head_id);
        let zone = if !on_head {
            match oa.or(ob) {
                Some(3) => "fang claws",
                Some(5) => "hollow under the head",
                Some(_) => "other made parts",
                None => "band and painted layers",
            }
        } else if ear_tips.iter().any(|t| len(sub(q, *t)) < 1.0) {
            "ear tips"
        } else if Wolf::ear(s) < 0.3 {
            "ears"
        } else if wolf.teeth.iter().any(|t| t.sdf(q) < 0.3) {
            "teeth"
        } else if q[2] > 0.0 && rho <= 6.9 {
            "mouth, gums and lips"
        } else if Wolf::nose(s) < 0.25 {
            "nose pad and nostrils"
        } else if q[2] > 0.3 && s[0] < 1.9 && q[1] > NOSE_U - 0.6 && q[1] < STOP_U {
            "muzzle"
        } else if q[2] > 0.3 && Wolf::brow(s) < 0.4 {
            "brow"
        } else if q[2] > 0.3 && (s[0] - Wolf::EYE.0).hypot(q[1] - Wolf::EYE.1) < 1.3 {
            "eyes"
        } else if q[2] > 0.3 && q[1] >= STOP_U + 0.7 {
            "crown"
        } else if q[2] > 0.3 && bearing < -10.0 && bearing > -170.0 {
            "lower jaw and chin"
        } else if q[2] > 0.3 && s[0] < 9.4 {
            "cheeks"
        } else {
            "head flanks and throat"
        };
        if matches!(zone, "cheeks" | "brow" | "muzzle" | "nose pad and nostrils" | "crown") {
            face_marks.push(mid);
            if face_marks.len() <= 12 && std::env::var("FENRIR_DEBUG").is_ok() {
                println!("    {zone} crease at face ({:.2}, {:.2}, {:.2}), {ang:.0} deg", q[0], q[1], q[2]);
            }
        }
        let z = zones.entry(zone).or_insert((0, 0.0, 0, HashSet::new()));
        z.0 += 1;
        z.1 += len(sub(v[*a as usize], v[*b as usize]));
        if ang >= 90.0 {
            z.2 += 1;
        }
        z.3.insert(std::array::from_fn(|k| mid[k].floor() as i64));
    }
    if let Some(z) = zones.get("cheeks") {
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
    let mut runs: Vec<(f64, P3, P3)> = Vec::new();
    for &start in graph.keys() {
        if !seen.insert(start) {
            continue;
        }
        let (mut stack, mut total) = (vec![start], 0.0);
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        while let Some(x) = stack.pop() {
            let p = v[x as usize];
            lo = std::array::from_fn(|k| lo[k].min(p[k]));
            hi = std::array::from_fn(|k| hi[k].max(p[k]));
            for &(y, l) in &graph[&x] {
                total += 0.5 * l;
                if seen.insert(y) {
                    stack.push(y);
                }
            }
        }
        longest = longest.max(total);
        runs.push((total, lo, hi));
    }
    runs.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (l, lo, hi) in runs.iter().take(6) {
        if *l > 0.5 {
            println!("    seam crease run {l:.2} mm between {:?} and {:?}", lo.map(|c| (c * 100.0).round() / 100.0), hi.map(|c| (c * 100.0).round() / 100.0));
        }
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

/// The closed shells of a mesh that enclose air inside the metal: each connected shell whose signed volume is
/// negative, as its volume and centre.
fn internal_voids(m: &Mesh) -> Vec<(f64, P3)> {
    let n = m.vertices.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    fn find(p: &mut [u32], x: u32) -> u32 {
        let mut r = x;
        while p[r as usize] != r {
            r = p[r as usize];
        }
        let mut y = x;
        while p[y as usize] != r {
            let next = p[y as usize];
            p[y as usize] = r;
            y = next;
        }
        r
    }
    for t in &m.faces {
        for e in 1..3 {
            let (a, b) = (find(&mut parent, t[0]), find(&mut parent, t[e]));
            if a != b {
                parent[a as usize] = b;
            }
        }
    }
    let mut shells: std::collections::HashMap<u32, (f64, P3, f64)> = std::collections::HashMap::new();
    for t in &m.faces {
        let [a, b, c] = t.map(|k| m.vertices[k as usize]).map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
        let vol = dot(a, cross(b, c)) / 6.0;
        let area = len(cross(sub(b, a), sub(c, a)));
        let s = shells.entry(find(&mut parent, t[0])).or_insert((0.0, [0.0; 3], 0.0));
        s.0 += vol;
        s.1 = add(s.1, mul(add(add(a, b), c), area / 3.0));
        s.2 += area;
    }
    let mut out: Vec<(f64, P3)> = shells.into_values().filter(|s| s.0 < 0.0).map(|s| (s.0, mul(s.1, 1.0 / s.2.max(1e-12)))).collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The finished ring cut by the half-plane through the ring's axis at `theta_deg` (90 is the head's centre, x = 0):
/// metal gold, the hollow and the finger dark, written as a PNG at `px_per_mm`; and the thinnest metal left over the
/// hollow along the cut, mm.
fn section_at(m: &Mesh, theta_deg: f64, path: &Path, px_per_mm: f64) -> Result<f64> {
    Ok(section_cut(m, theta_deg, path, px_per_mm)?.0)
}

/// [`section_at`] with the thinnest floor left between the pocket and the finger where the pocket runs on past its
/// mouth, mm (`f64::MAX` where it has none), both a millimetre in from the ends of each run of pocket columns.
fn section_cut(m: &Mesh, theta_deg: f64, path: &Path, px_per_mm: f64) -> Result<(f64, f64)> {
    let (z0, z1, y0, y1) = (-13.0, 13.0, 8.0, 20.0);
    let (w, h) = (((z1 - z0) * px_per_mm) as usize, ((y1 - y0) * px_per_mm) as usize);
    let (s, c) = theta_deg.to_radians().sin_cos();
    // Off the plane, and out along it from the axis.
    let (off, out) = (|v: P3| v[0] * s - v[1] * c, |v: P3| v[0] * c + v[1] * s);
    // Each face crossing the plane leaves a segment in (z, radial), on the half-plane's own side.
    let mut segs: Vec<[f64; 4]> = Vec::new();
    for t in &m.faces {
        let p = t.map(|k| m.vertices[k as usize]).map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
        let mut cut: Vec<[f64; 2]> = Vec::new();
        for e in 0..3 {
            let (a, b) = (p[e], p[(e + 1) % 3]);
            let (da, db) = (off(a), off(b));
            if (da < 0.0) != (db < 0.0) {
                let k = da / (da - db);
                let (ra, rb) = (out(a), out(b));
                if ra + k * (rb - ra) > 0.0 {
                    cut.push([a[2] + k * (b[2] - a[2]), ra + k * (rb - ra)]);
                }
            }
        }
        if cut.len() == 2 {
            segs.push([cut[0][0], cut[0][1], cut[1][0], cut[1][1]]);
        }
    }
    let mut img = vec![18u8; w * h * 3];
    // Per column over the pocket: its z and the metal over it; and where a floor runs under it, the floor.
    let mut over: Vec<(f64, f64)> = Vec::new();
    let mut floor: Vec<(f64, f64)> = Vec::new();
    for col in 0..w {
        let z = z0 + (col as f64 + 0.5) / px_per_mm;
        let mut ys: Vec<f64> = segs
            .iter()
            .filter(|s| (s[0] < z) != (s[2] < z))
            .map(|s| s[1] + (z - s[0]) / (s[2] - s[0]) * (s[3] - s[1]))
            .collect();
        ys.sort_by(f64::total_cmp);
        if col == w / 2 && std::env::var("FENRIR_HOLLOW").is_ok() {
            println!("    section crossings at z {z:.3}: {:?}", ys.iter().map(|y| (y * 1000.0).round() / 1000.0).collect::<Vec<_>>());
        }
        for pair in ys.chunks(2) {
            if pair.len() < 2 {
                continue;
            }
            for row in 0..h {
                let y = y1 - (row as f64 + 0.5) / px_per_mm;
                if y >= pair[0] && y <= pair[1] {
                    let k = (row * w + col) * 3;
                    img[k..k + 3].copy_from_slice(&[214, 178, 92]);
                }
            }
        }
        // Where the first metal starts clear of the finger's cylinder, the hollow opens under it; where the first
        // metal starts on the finger's cylinder and air follows it under the table's plane, the pocket runs over a
        // floor.
        // Films of metal or air thinner than 0.05 mm are merged away, and the roof is the metal over the pocket's
        // highest air closed under the table's plane (at this cut's own distance from the axis), so neither a burr
        // on the pocket's wall nor open air over the band is read as the roof.
        let table_r = 14.0 / (theta_deg - 90.0).to_radians().cos();
        let mut merged: Vec<[f64; 2]> = Vec::new();
        for p in ys.chunks(2).filter(|p| p.len() == 2) {
            match merged.last_mut() {
                Some(last) if p[0] - last[1] < 0.05 => last[1] = p[1],
                _ => merged.push([p[0], p[1]]),
            }
        }
        merged.retain(|p| p[1] - p[0] >= 0.05);
        let pairs: Vec<&[f64]> = merged.iter().map(|p| &p[..]).collect();
        if z.abs() < 8.0 && !pairs.is_empty() {
            let first = pairs[0];
            let roof = (0..pairs.len()).rev().find(|&k| k > 0 && pairs[k][0] < table_r + 0.35 && pairs[k][1] > table_r - 0.5);
            if first[0] > 9.5 + 0.4 {
                let top = roof.map_or(first, |k| pairs[k]);
                over.push((z, top[1] - top[0]));
            } else if let Some(k) = roof {
                floor.push((z, first[1] - first[0]));
                over.push((z, pairs[k][1] - pairs[k][0]));
            }
            if std::env::var("FENRIR_HOLLOW").is_ok() && over.last().is_some_and(|o| o.0 == z && o.1 < 0.5) {
                println!("    thin column at z {z:.3}: {:?}", ys.iter().map(|y| (y * 1000.0).round() / 1000.0).collect::<Vec<_>>());
            }
        }
    }
    image::save_buffer(path, &img, w as u32, h as u32, image::ColorType::Rgb8)?;
    // The thinnest over each run of pocket columns, a millimetre in from either end, clear of the rounded walls.
    let trimmed = |cols: &[(f64, f64)]| {
        let mut least = f64::MAX;
        let mut start = 0;
        for k in 0..=cols.len() {
            if k == cols.len() || (k > start && cols[k].0 - cols[k - 1].0 > 1.5 / px_per_mm) {
                if k > start {
                    let (z0, z1) = (cols[start].0 + 1.0, cols[k - 1].0 - 1.0);
                    least = cols[start..k].iter().filter(|(z, _)| *z >= z0 && *z <= z1).fold(least, |m, (_, t)| m.min(*t));
                }
                start = k;
            }
        }
        least
    };
    over.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok((trimmed(&over), trimmed(&floor)))
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
        let hide = Hide::of(&a);
        for x in [a.width / 8, a.width / 8 + 60, 3 * a.width / 8] {
            for y in [a.height / 4, a.height / 2, 3 * a.height / 4] {
                let s = a.at(x, y);
                let h = hide.at(s);
                println!("  sample at world ({:.2}, {:.2}, {:.2}): along {:.2}, across {:.2}", s.p[0], s.p[1], s.p[2], h.along, h.across);
            }
        }
        return Ok(());
    }
    if let Some(spec) = args.iter().find(|a| a.starts_with("--slice=")) {
        // An inside/outside map of the finished head's field: "--slice=x,u,h,step,along_u" ('#' head, 'o' stock).
        let v: Vec<f64> = spec[8..].split(',').map(|x| x.parse().unwrap()).collect();
        let mut d = base()?;
        let a = Atlas::of(&d, AW, atlas_rows(&d))?;
        stone_and_fangs(&mut d)?;
        let wolf = wolf_of(&d, &a)?;
        let (cx, cu, ch, step) = (v[0], v[1], v[2], v[3]);
        if std::env::var("FENRIR_PROBE").is_ok() {
            for dh in [-0.1, -0.05, 0.0, 0.05, 0.1] {
                let q = [cx, cu, ch + dh];
                let s = [q[0].abs(), q[1], q[2]];
                let world = [q[0], q[2] + wolf.table, -q[1]];
                println!("at {q:?}: masses {:.3} pelt {:.3} head {:.3} sdf {:.3} stock {:.3}; cranium {:.2} cheek {:.2} jowl {:.2} upper jaw {:.2} mandible {:.2} gums {:.2} throat {:.2}", wolf.masses(q), wolf.pelt(q), wolf.head(q), wolf.sdf(world), wolf.stock.at(world), Wolf::cranium(s), Wolf::cheek(s), Wolf::jowl(s), Wolf::upper_jaw(s), Wolf::mandible(s), Wolf::gums(s), Wolf::throat(s));
            }
            return Ok(());
        }
        let along_u = v.get(4).is_some_and(|f| *f > 0.0);
        for row in (0..40).rev() {
            let h = ch - 20.0 * step + row as f64 * step;
            let line: String = (0..80)
                .map(|col| {
                    let t = -40.0 * step + col as f64 * step;
                    let world = if along_u { [cx, h + wolf.table, -(cu + t)] } else { [cx + t, h + wolf.table, -cu] };
                    match (wolf.sdf(world) < 0.0, wolf.stock.at(world) < 0.0) {
                        (true, true) => '+',
                        (true, false) => '#',
                        (false, true) => 'o',
                        _ => '.',
                    }
                })
                .collect();
            println!("h {h:5.2} {line}");
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--recensus") {
        return recensus(Path::new(out));
    }
    if args.iter().any(|a| a == "--hollow") {
        return hollow_preview(Path::new(out));
    }
    if args.iter().any(|a| a == "--jaws") {
        let mut d = base()?;
        let a = Atlas::of(&d, AW, atlas_rows(&d))?;
        stone_and_fangs(&mut d)?;
        let wolf = wolf_of(&d, &a)?;
        println!("{}", serde_json::to_string_pretty(&jaw_measures(&wolf))?);
        return Ok(());
    }
    if args.iter().any(|a| a == "--quick") {
        return quick_preview(Path::new(out), 0.13);
    }
    if args.iter().any(|a| a == "--sculpt") {
        return sculpt_preview(Path::new(out), SCULPT_STEP);
    }
    write(Path::new(out), args.iter().any(|a| a == "--draft"), args.iter().any(|a| a == "--verify"))
}
