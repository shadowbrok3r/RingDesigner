//! Draco, the wyvern displayed: factory 002 Kite as a Delft sand master, one wyvern from face to palm.
//! cargo run -p ringdesign-core --release --example bestiarium_draco -- [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    castability::{CastProcess, SandProcess, Verdict},
    csg,
    field::{Blend, Window},
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    manufacturing as mf,
    render::{self, Part},
    setting::Stamp,
    skin::{self, Atlas, ClampReport, Hide, HidePoint, Joints, Sample, draft_clamp},
};
use std::{
    f64::consts::PI,
    path::{Path, PathBuf},
};

const AW: usize = 2048;
const AH: usize = 768;

/// A point on the plan of the face: `u` along the ridge (+ toward the neck), `z` across the finger.
type P2 = [f64; 2];

fn lerp2(a: P2, b: P2, t: f64) -> P2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    ringdesign_core::field::smoothstep(e0, e1, x)
}

/// Distance from `p` to segment `a`-`b`, which side (positive to the left walking a to b), and the share along it.
fn seg(p: P2, a: P2, b: P2) -> (f64, f64, f64) {
    let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
    let l2 = (ex * ex + ez * ez).max(1e-12);
    let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ez) / l2).clamp(0.0, 1.0);
    let (dx, dz) = (p[0] - a[0] - ex * t, p[1] - a[1] - ez * t);
    (dx.hypot(dz), ex * (p[1] - a[1]) - ez * (p[0] - a[0]), t)
}

/// A quadratic Bezier from `a` to `b` bowed `bow` mm to the left, as a polyline.
fn bone(a: P2, b: P2, bow: f64, n: usize) -> Vec<P2> {
    let m = lerp2(a, b, 0.5);
    let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
    let l = ex.hypot(ez).max(1e-9);
    let c = [m[0] - ez / l * bow, m[1] + ex / l * bow];
    (0..=n).map(|i| { let t = i as f64 / n as f64; lerp2(lerp2(a, c, t), lerp2(c, b, t), t) }).collect()
}

/// Nearest distance to a polyline, its side there, and the share along it.
fn poly(p: P2, pts: &[P2]) -> (f64, f64, f64) {
    let mut best = (f64::MAX, 0.0, 0.0);
    let n = pts.len() - 1;
    for i in 0..n {
        let (d, side, t) = seg(p, pts[i], pts[i + 1]);
        if d < best.0 {
            best = (d, side, (i as f64 + t) / n as f64);
        }
    }
    best
}

/// How a bone meets the membrane round it.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// A rounded ridge standing on one membrane level: an arm bone.
    Ridge,
    /// The rounded lip of a step down to the panel outside it: a finger.
    Lip,
}

/// One bone of the wing, drawn so that its left side is the side nearer the ridge.
struct Bone {
    pts: Vec<P2>,
    kind: Kind,
    /// Membrane level on its ridge side, mm.
    inner: f64,
    /// Membrane level on its far side, mm; zero where it is the wing's own edge.
    outer: f64,
    /// Crest over the inner level at the root and at the tip, mm.
    rise: (f64, f64),
    /// Half the crest's width at the root and at the tip, mm.
    half: (f64, f64),
    /// How squarely a column crosses it: |cos| of its run against the ridge.
    cross: f64,
}

impl Bone {
    fn new(pts: Vec<P2>, kind: Kind, inner: f64, outer: f64, rise: (f64, f64), half: (f64, f64)) -> Self {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let cross = ((b[0] - a[0]) / (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9)).abs();
        Self { pts, kind, inner, outer, rise, half, cross }
    }

    /// Height of the bone's own section at `p` over membrane sagged by `sag`, mm, how far off its line `p` lies, and how far up its section, 0..1.
    fn profile(&self, p: P2, sag: f64) -> Option<(f64, f64, f64)> {
        let (d, side, t) = poly(p, &self.pts);
        let crest = self.inner + self.rise.0 + (self.rise.1 - self.rise.0) * t;
        let half = self.half.0 + (self.half.1 - self.half.0) * t;
        // Arm bones include their knuckles' swell.
        let rise = crest - self.inner + sag + if self.kind == Kind::Ridge { 0.12 } else { 0.0 };
        // Ridge-side flank climbs 0.13 mm per mm of column; the far flank drops steeply.
        let ramp = (rise * self.cross / 0.13).max(half * 1.2);
        let (level, run) = if side > 0.0 { (self.inner - sag, ramp) } else { ((self.outer - sag).max(0.0), half * 0.8 + 0.05) };
        if d > half + run {
            return None;
        }
        let f = if d <= half { 1.0 - 0.18 * (d / half).powi(2) } else { 0.82 * (1.0 - smooth(0.0, run, d - half)) };
        Some((level + (crest - level) * f, d, f))
    }

    /// The bone's bearing from its root at `rho` mm out, radians in 0..2pi, held past either end.
    fn bearing(&self, rho: f64) -> f64 {
        let o = self.pts[0];
        let at = |q: P2| (q[1] - o[1]).atan2(q[0] - o[0]).rem_euclid(2.0 * PI);
        let r = |q: P2| (q[0] - o[0]).hypot(q[1] - o[1]);
        for w in self.pts[1..].windows(2) {
            let (a, b) = (r(w[0]), r(w[1]));
            if rho <= b || w[1] == *self.pts.last().unwrap() {
                let t = ((rho - a) / (b - a).max(1e-9)).clamp(0.0, 1.0);
                let (pa, pb) = (at(w[0]), at(w[1]));
                return pa + (pb - pa) * t;
            }
        }
        at(self.pts[1])
    }

    /// Whether `p` lies on the ridge side of a bone fanning from its root: past its bearing at the same radius.
    fn inside(&self, p: P2) -> bool {
        let o = self.pts[0];
        let psi = (p[1] - o[1]).atan2(p[0] - o[0]).rem_euclid(2.0 * PI);
        psi > self.bearing((p[0] - o[0]).hypot(p[1] - o[1]))
    }
}

/// The wyvern displayed on the face: one wing's plan, mirrored across the ridge, and the plan of its back.
struct Wyvern {
    /// Where the membrane in front of the arm leaves the neck, and the thumb's knuckle at the wing's forward corner.
    n: P2,
    k: P2,
    /// The wrist, where the fingers fan from.
    w: P2,
    /// Finger tips on the trailing edge, leading first.
    tips: [P2; 5],
    /// Where the innermost membrane's trailing edge meets the flank.
    hip: P2,
    /// Humerus, forearm, thumb, then the five fingers leading first.
    bones: Vec<Bone>,
    /// Membrane between fingers 1-2, 2-3, 3-4, 4-5, and the membrane round the arm.
    levels: [f64; 5],
    /// Elbow and wrist knuckles: centre, rise over the arm's crest, and radius, mm.
    knuckles: [(P2, f64, f64); 2],
}

impl Wyvern {
    fn new() -> Self {
        let (n, s, e, w, k) = ([6.5, 0.95], [4.15, 1.8], [3.55, 3.35], [5.05, 4.9], [6.1, 3.55]);
        let tips = [[1.15, 8.55], [-1.75, 7.95], [-4.05, 6.3], [-5.75, 4.3], [-6.75, 2.35]];
        let hip = [-7.1, 1.0];
        let levels = [0.40, 0.48, 0.56, 0.64, 0.72];
        let lm = levels[4];
        let mut bones = vec![
            Bone::new(bone(s, e, -0.25, 24), Kind::Ridge, lm, lm, (0.08, 0.26), (0.18, 0.25)),
            Bone::new(bone(w, e, -0.25, 24), Kind::Ridge, lm, lm, (0.24, 0.24), (0.23, 0.24)),
            Bone::new(bone(k, w, -0.12, 16), Kind::Ridge, lm, 0.0, (0.14, 0.24), (0.10, 0.21)),
        ];
        let bows = [0.5, 0.7, 0.7, 0.55, 0.35];
        for i in 0..5 {
            let outer = if i == 0 { 0.0 } else { levels[i - 1] };
            bones.push(Bone::new(bone(w, tips[i], bows[i], 56), Kind::Lip, levels[i], outer, (0.18, 0.10), (0.13, 0.06)));
        }
        Self { n, k, w, tips, hip, bones, levels, knuckles: [(e, 0.10, 0.8), (w, 0.12, 0.8)] }
    }

    /// Half the back's width on the face at `u`: broad at the shoulders and hips, waisted between, narrowing to neck and tail.
    fn body_half(&self, u: f64) -> f64 {
        let k: [(f64, f64); 11] = [(8.0, 0.56), (6.6, 0.64), (5.6, 1.20), (4.4, 1.72), (3.0, 1.62), (1.0, 1.12), (-1.6, 1.00), (-4.4, 1.46), (-5.8, 1.20), (-7.0, 0.64), (-8.0, 0.54)];
        if u >= k[0].0 {
            return k[0].1;
        }
        if u <= k[10].0 {
            return k[10].1;
        }
        let i = k.iter().position(|q| q.0 <= u).unwrap().max(1);
        let (a, b) = (k[i - 1], k[i]);
        let t = (u - a.0) / (b.0 - a.0);
        a.1 + (b.1 - a.1) * t * t * (3.0 - 2.0 * t)
    }

    /// Where a polyline stands at `u`, mm of z.
    fn z_on(pts: &[P2], u: f64) -> Option<f64> {
        pts.windows(2).find(|w| (w[0][0] - u) * (w[1][0] - u) <= 0.0).map(|w| {
            let t = (u - w[0][0]) / (w[1][0] - w[0][0]);
            w[0][1] + (w[1][1] - w[0][1]) * if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 }
        })
    }

    /// The wing's outer boundary at `u`: one z per column.
    fn outline(&self, u: f64) -> Option<f64> {
        if u > self.n[0] || u < self.hip[0] {
            return None;
        }
        if u >= self.k[0] {
            // Front edge of the membrane before the arm, neck to knuckle.
            let t = (self.n[0] - u) / (self.n[0] - self.k[0]);
            return Some(self.n[1] + (self.k[1] - self.n[1]) * t.powf(0.8));
        }
        if u >= self.w[0] {
            return Some(Self::z_on(&self.bones[2].pts, u)? + 0.12);
        }
        if u >= self.tips[0][0] {
            return Some(Self::z_on(&self.bones[3].pts, u)? + 0.10);
        }
        let mut pts = self.tips.to_vec();
        pts.push(self.hip);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if u <= a[0] && u >= b[0] {
                let t = (a[0] - u) / (a[0] - b[0]).max(1e-9);
                let chord = a[1] + (b[1] - a[1]) * t;
                // Scallop between two tips, deepest past the middle, the tip standing proud.
                let bite = if b == self.hip { 0.25 } else { 0.95 * (a[0] - b[0]).min(2.6) / 2.6 };
                return Some(chord - bite * (PI * t).sin().powf(0.7) * (1.0 - 0.2 * t) + 0.10 * (1.0 - smooth(0.0, 0.1, t)));
            }
        }
        None
    }

    /// The membrane's level at a point inside the wing: counted by the fingers whose ridge side it lies on.
    fn panel(&self, p: P2) -> f64 {
        let n = self.bones[3..].iter().filter(|b| b.inside(p)).count();
        if n == 0 { 0.0 } else { self.levels[n - 1] }
    }

    /// Relief of the wings at a face point, mm: the membrane from the ridge out, its bones, its scalloped edge.
    fn wings(&self, u: f64, z: f64) -> f64 {
        let z = z.abs();
        let Some(edge) = self.outline(u) else { return 0.0 };
        if z > edge + 0.12 {
            return 0.0;
        }
        let p = [u, z];
        let base = self.panel(p);
        let near = self.bones.iter().map(|b| poly(p, &b.pts).0).fold((edge - z).max(0.0).min((z - self.body_half(u)).max(0.0)), f64::min);
        // Panel sag away from bones and edges; bones rise from the sagged membrane.
        let sag = 0.06 * smooth(0.10, 1.2, near);
        let mut h = base - sag;
        for (i, b) in self.bones.iter().enumerate() {
            if let Some((v, _, f)) = b.profile(p, sag) {
                // Knuckle swell on the arm bones, scaled by the bone's own section.
                let swell: f64 = if i < 3 {
                    self.knuckles.iter().map(|(c, rise, r)| rise * (1.0 - smooth(0.0, *r, (u - c[0]).hypot(z - c[1])))).fold(0.0, f64::max)
                } else {
                    0.0
                };
                h = h.max(v + swell * f);
            }
        }
        h * (1.0 - smooth(edge - 0.03, edge + 0.10, z))
    }

    /// The graver's veins in the membrane: two running down each panel between its fingers, a few crossing them.
    fn veins(&self, u: f64, z: f64) -> f64 {
        let z = z.abs();
        let Some(edge) = self.outline(u) else { return 0.0 };
        if z > edge - 0.25 || z < self.body_half(u) + 0.25 {
            return 0.0;
        }
        let p = [u, z];
        if self.bones.iter().any(|b| poly(p, &b.pts).0 < 0.22) {
            return 0.0;
        }
        let mut cut: f64 = 0.0;
        let tips: Vec<P2> = self.tips.to_vec();
        for i in 0..4 {
            for (k, f) in [0.36, 0.68].into_iter().enumerate() {
                let tip = lerp2(tips[i], tips[i + 1], f);
                let from = lerp2(self.w, tip, 0.12 + 0.08 * k as f64);
                let vein = bone(from, lerp2(self.w, tip, 0.93), 0.35 + 0.1 * k as f64, 24);
                let (d, _, t) = poly(p, &vein);
                let w = 0.08 * (1.0 - 0.4 * t);
                cut = cut.max(1.0 - smooth(w * 0.5, w, d));
            }
        }
        // Veins running aft from the arm toward the hip's edge.
        for f in [0.3, 0.62] {
            let from = lerp2(self.bones[0].pts[24], self.w, f);
            let to = lerp2(self.tips[4], self.hip, 0.2 + 0.5 * f);
            let (d, _, t) = poly(p, &bone(from, to, -0.4, 32));
            let w = 0.08 * (1.0 - 0.35 * t);
            cut = cut.max(1.0 - smooth(w * 0.5, w, d));
        }
        cut
    }
}

/// The wyvern's body along the ring in hide millimetres: its back on the face, its neck and tail down the shoulders, both to the palm.
struct Body<'a> {
    wy: &'a Wyvern,
    /// Joints of the backbone, the scales beside it, and the flank scales, as |along| from the head's centre.
    series: [Vec<f64>; 3],
    /// Arc from the head's centre to the palm along the parting line.
    reach: f64,
    /// The dorsal spines struck on the backbone, whose fins the backbone carries.
    spines: Vec<Spine>,
}

impl<'a> Body<'a> {
    fn new(wy: &'a Wyvern, reach: f64) -> Self {
        // Three long face plates, then plates graded to one unbroken plate under the spade.
        let mut spine = vec![1.6, 4.8];
        spine.extend(Joints::eccentric(4.8, reach - 3.2, 2.6, 1.85).0.into_iter().skip(1));
        // Series joints: each backbone plate split in two.
        let split = |g: &[f64], k: usize| -> Vec<f64> {
            let mut out = vec![g[0]];
            for w in g.windows(2) {
                out.extend((1..=k).map(|i| w[0] + (w[1] - w[0]) * i as f64 / k as f64));
            }
            out
        };
        let mut s1 = vec![1.6 / 3.0, 1.6, 1.6 + 3.2 / 3.0, 1.6 + 6.4 / 3.0];
        s1.extend(split(&spine[1..], 2));
        let s2 = s1.clone();
        Self { wy, series: [spine, s1, s2], reach, spines: Vec::new() }
    }

    /// Half the body's width across the surface at `along`, mm: the face's own plan, thickening over the head's ends into neck and tail and tapering to the palm.
    fn half(&self, along: f64, rim: f64) -> f64 {
        let l = along.abs();
        let face = self.wy.body_half(-along) * 1.035;
        let taper = 1.62 + 1.2 * (1.0 - smooth(11.0, 21.0, l));
        face + (taper.min(rim - 0.3) - face) * smooth(7.0, 12.5, l)
    }

    /// How far the flank's own plates reach beside the body, mm: to the rim off the face, nowhere on it.
    fn flank(&self, along: f64, rim: f64) -> f64 {
        let half = self.half(along, rim);
        half + (rim - 0.02 - half).max(0.0) * smooth(7.2, 9.6, along.abs())
    }

    /// Height of the backbone's plates, mm: tall on the face, lower down the shank.
    fn crown(&self, along: f64) -> f64 {
        let l = along.abs();
        let face = 1.26 - 0.26 * smooth(4.6, 7.4, l);
        // Neck and tail dip where they cross the head's end walls.
        let over = 1.0 - 0.42 * (-((l - 8.6) / 1.1).powi(2)).exp();
        (face + (0.88 - face) * smooth(6.5, 12.5, l)) * over
    }

    /// How far `l` stands inside its plate of series `k`: 1 on the plate, falling into each joint.
    fn loaf(&self, k: usize, l: f64) -> f64 {
        if l > self.reach - 3.2 {
            return 1.0;
        }
        let g = &self.series[k];
        let j = g.partition_point(|x| *x <= l);
        let (a, b) = (if j == 0 { -g[0] } else { g[j - 1] }, if j < g.len() { g[j] } else { self.reach });
        let from = (l - a).min(b - l).max(0.0);
        let dip = [0.18, 0.08, 0.22][k];
        1.0 - dip * (1.0 - smooth(0.03, [0.26, 0.30, 0.24][k], from))
    }

    /// Relief of the body at a hide point, mm: backbone, two series of scales stepping down beside it, and the flank's plates to the rim.
    fn relief(&self, p: &HidePoint) -> f64 {
        let (l, w) = (p.along.abs(), p.across.abs());
        let half = self.half(p.along, p.rim);
        let flank = self.flank(p.along, p.rim);
        if w > flank + 0.14 {
            return 0.0;
        }
        let crown = self.crown(p.along);
        let (k0, k1) = (0.38 * half, 0.70 * half);
        let shank = smooth(6.5, 12.5, l);
        let drop = [0.0, 0.17 - 0.03 * shank, 0.42 - 0.10 * shank, 0.47 + 0.01 * shank];
        let lvl = |k: usize| (crown - drop[k]) * self.loaf(k.min(2), l);
        let keel = 0.12 * (1.0 - smooth(0.0, k0, w));
        let backbone = lvl(0) + keel * self.loaf(0, l);
        // Series plates slope down from their inner edge.
        let s1 = lvl(1) - 0.04 * ((w - k0) / (k1 - k0).max(0.1)).clamp(0.0, 1.0);
        let s2 = lvl(2) - 0.06 * ((w - k1) / (half - k1).max(0.1)).clamp(0.0, 1.0);
        let top = backbone + (s1 - backbone) * smooth(k0 - 0.05, k0 + 0.05, w);
        let top = top + (s2 - top) * smooth(k1 - 0.05, k1 + 0.05, w);
        let (top, edge) = if flank > half + 0.2 { (top + (lvl(3) - top) * smooth(half - 0.06, half + 0.06, w), flank) } else { (top, half) };
        let fin = self.spines.iter().filter(|s| s.fin > 0.0).map(|s| s.fin_at(p.along, p.across)).fold(0.0, f64::max);
        top * (1.0 - smooth(edge - 0.02, edge + 0.14, w)) + fin
    }
}

/// Small round scales on the walls facing the pull, overlapping down the wall on a raised bed.
fn wall_scales(p: &HidePoint, s: &Sample, bore: f64) -> f64 {
    // Wall weight from distance past the rim and the wall's own height.
    let wall = smooth(0.12, 0.62, p.across.abs() - p.rim) * smooth(1.0, 1.5, p.wall);
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
    wall * (0.62 + 0.38 * best) * smooth(bore + 0.45, bore + 0.9, rho)
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
    setup.bench_notes = "Imported-stock master, one wyvern from face to palm: its wings displayed across the kite's pitched table, \
        stepped finger by finger down from the spine to the points at the rims; its plated back on the ridge with three tall spines \
        struck between the wings; its neck and tail down the shoulders in graded plates, a raked spine struck on each; a spade tail \
        across the palm; round scales on the head's walls. Every panel steps down away from the parting line and every joint runs \
        across the band, so the pattern pulls as drawn. Z=0 parting, opposed Z withdrawal. At the bench: cut the spade's two barbs \
        and engrave the membrane's veins. Polish the bones, the spines and the plates' tops; leave the membranes and scales satin."
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

/// Where a build's surface crosses itself: the ring angle and z of each half-degree, quarter-millimetre cell holding a crossing.
fn crossing_spots(mesh: &ringdesign_core::Mesh) -> Vec<(f64, f64)> {
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

/// Most relief each sample may carry near spots where the stock's own facets fold, mm: each spot's floor across a band
/// round its |z|, rising 1.0 mm per mm round the ring, and `slope` per mm across outside the band.
fn fold_caps(a: &Atlas, hide: &Hide, spots: &[(f64, f64, f64)], slope: f64) -> Vec<f64> {
    let seeds: Vec<(f64, f64, f64)> = spots
        .iter()
        .map(|(theta, z, floor)| (hide.along[((theta / 360.0 * AW as f64).round() as usize) % AW], z.abs(), *floor))
        .collect();
    a.samples
        .iter()
        .map(|s| {
            let (along, z) = (hide.along[s.i % AW], s.p[2].abs());
            seeds.iter().map(|(l, zs, floor)| floor + 1.0 * (along - l).abs() + slope * ((z - zs).abs() - 0.3).max(0.0)).fold(f64::MAX, f64::min)
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

/// An outline through the corners, a point every 0.1 mm.
fn dense(corners: &[P2]) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for (c, n) in corners.iter().zip(corners.iter().cycle().skip(1)) {
        let steps = ((c[0] - n[0]).hypot(c[1] - n[1]) / 0.1).ceil().max(1.0) as usize;
        out.extend((0..steps).map(|q| lerp2(*c, *n, q as f64 / steps as f64)));
    }
    out
}

/// A dorsal spine seen from above: a thorn along the ring, round at its root toward the head and drawn to a point toward the tail, the point blunted square to the ring.
fn spine_outline(half_len: f64, half_w: f64) -> Vec<[f64; 2]> {
    let tip = 0.08;
    let mut corners: Vec<P2> = Vec::new();
    // Round root, then both flanks tapering to the blunted point.
    for i in 0..=10 {
        let a = PI * 0.5 + PI * i as f64 / 10.0;
        corners.push([-half_len + half_w + half_w * a.cos(), half_w * a.sin()]);
    }
    for i in 1..8 {
        let t = i as f64 / 8.0;
        corners.push([-half_len + half_w + (2.0 * half_len - half_w) * t, -(tip + (half_w - tip) * (1.0 - t).powf(0.6))]);
    }
    corners.push([half_len, -tip]);
    corners.push([half_len, tip]);
    for i in (1..8).rev() {
        let t = i as f64 / 8.0;
        corners.push([-half_len + half_w + (2.0 * half_len - half_w) * t, tip + (half_w - tip) * (1.0 - t).powf(0.6)]);
    }
    let mut out = dense(&corners);
    let area: f64 = (0..out.len()).map(|i| { let (p, q) = (out[i], out[(i + 1) % out.len()]); p[0] * q[1] - q[0] * p[1] }).sum();
    if area < 0.0 {
        out.reverse();
    }
    out
}

/// Half the thorn's width at `x` along it, mm: its round root, then the flanks tapering to the blunted point.
fn thorn_half(x: f64, half_len: f64, half_w: f64) -> f64 {
    let root = -half_len + half_w;
    if x < root {
        (half_w * half_w - (x - root).powi(2)).max(0.0).sqrt()
    } else {
        let t = ((x - root) / (2.0 * half_len - half_w)).clamp(0.0, 1.0);
        0.08 + (half_w - 0.08) * (1.0 - t).powf(0.6)
    }
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
    /// The fin under the spine at a hide point, mm: a sharp gable inside the thorn's plan, rising gently from root to point.
    fn fin_at(&self, along: f64, across: f64) -> f64 {
        let x = along - self.at;
        let hl = self.half_len;
        if x.abs() >= hl {
            return 0.0;
        }
        let half = 0.85 * thorn_half(x, hl, self.half_w);
        let rake = ((x + hl) / (2.0 * hl)) * (1.0 - smooth(0.3 * hl, 0.8 * hl, x));
        // Gable with a flat of 0.04 mm either side of the parting line.
        self.fin * rake * (1.0 - (across.abs() - 0.04).max(0.0) / (half - 0.04).max(1e-6)).max(0.0)
    }
}

/// The spines: three tall on the table between the wings, then one on every plate of neck and tail, graded to the palm; clear of the folds.
fn spine_plan(body: &Body, folds: &[f64]) -> Vec<Spine> {
    let g = &body.series[0];
    let mut plates: Vec<(f64, f64)> = vec![(0.0, 3.2)];
    plates.extend(g.windows(2).map(|w| (0.5 * (w[0] + w[1]), w[1] - w[0])));
    let mut out = Vec::new();
    for sign in [1.0, -1.0] {
        for (j, (centre, len)) in plates.iter().enumerate() {
            if j == 0 && sign < 0.0 {
                continue;
            }
            let at = sign * centre;
            let fade = smooth(4.0, body.reach - 4.0, *centre);
            let half_len = (0.33 * len).min(0.5 * len - if *centre < 5.0 { 0.62 } else { 0.5 });
            if half_len < 0.36 || *centre > body.reach - 3.9 || folds.iter().any(|f| (f - at).abs() < half_len + 1.0) {
                continue;
            }
            let face = *centre < 5.0;
            let (height, half_w) = if face { (0.95, 0.34) } else { (0.62 - 0.26 * fade, 0.32 - 0.02 * fade) };
            // Fins under spines long enough to hold one; the shortest stay plain blades.
            let fin = if face { 0.34 } else if half_len >= 0.5 { 0.24 - 0.10 * fade } else { 0.0 };
            let height = if face { height } else if fin > 0.0 { height - 0.5 * fin } else { height };
            let side = if j == 0 { "centre".to_string() } else if sign > 0.0 { format!("tail {j}") } else { format!("neck {j}") };
            out.push(Spine { name: format!("Dorsal spine, {side}"), at, half_len, half_w, height, fin });
        }
    }
    out
}

/// The spade at the tail's end, pointing along +x: a lanceolate blade whose base carries two barbs, cast as its hull.
fn spade_outline() -> Vec<[f64; 2]> {
    let (len, w) = (5.4, 1.42);
    let mut pts: Vec<[f64; 2]> = Vec::new();
    // Shaft, lower barb and flank to the point, back by the upper flank and barb.
    let lower: Vec<P2> = (0..=36)
        .map(|i| {
            let t = i as f64 / 36.0;
            let x = -0.40 * len + 0.90 * len * t;
            let bulge = (PI * (0.18 + 0.82 * t)).sin().powf(0.85);
            [x, -(0.16 + (w - 0.16) * bulge * (1.0 - smooth(0.82, 1.0, t)) + 0.09 * smooth(0.9, 1.0, t))]
        })
        .collect();
    pts.push([-0.5 * len, -0.42]);
    pts.push([-0.5 * len, -0.95]);
    pts.extend(lower.iter().copied());
    pts.push([0.5 * len, 0.0]);
    pts.extend(lower.iter().rev().map(|p| [p[0], -p[1]]));
    pts.push([-0.5 * len, 0.95]);
    pts.push([-0.5 * len, 0.42]);
    let mut out = Vec::new();
    for (c, n) in pts.iter().zip(pts.iter().cycle().skip(1)) {
        let steps = ((c[0] - n[0]).hypot(c[1] - n[1]) / 0.1).ceil().max(1.0) as usize;
        out.extend((0..steps).map(|q| lerp2(*c, *n, q as f64 / steps as f64)));
    }
    out
}

/// A barb's notch, cut at the bench from the spade's hull: the wedge between one barb and the shaft.
fn barb_cutter(side: f64) -> Vec<[f64; 2]> {
    let mut out: Vec<P2> = [[-2.95, 0.40], [-1.75, 0.56], [-2.95, 0.80]].iter().map(|p| [p[0], p[1] * side]).collect();
    if side < 0.0 {
        out.reverse();
    }
    dense(&out)
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
    let edges = roof_edges(&a, &hide);
    let wy = Wyvern::new();
    let mut body = Body::new(&wy, hide.reach());
    let mut bites = Bites::new();
    println!("  hide: {:.2} mm to the palm; spine joints {:?}", hide.reach(), body.series[0].iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<_>>());
    let u_of = |s: &Sample| -hide.along[s.i % AW];
    let on_roof = |s: &Sample| {
        let [lo, hi] = edges[s.i % AW];
        hi > 0.0 && s.p[2] < hi - 0.3 && s.p[2] > lo + 0.3
    };

    body.spines = spine_plan(&body, &hide.folds(&a, 12.0));

    // Cast layers, each capped where the stock's facets fold.
    let painted = |caps: &[f64]| -> Vec<(Alpha, f64, Window, Blend)> {
        let capped = |s: &Sample, h: f64| h.min(caps[s.i]);
        const WINGS: f64 = 1.16;
        const BACK: f64 = 1.74;
        const NECK: f64 = 1.26;
        const WALL: f64 = 0.28;
        let mut neck = Window::except(90.0, 56.0);
        neck.fade_deg = 4.0;
        vec![
            (a.paint("Wings displayed", |s| if on_roof(s) { capped(s, wy.wings(u_of(s), s.p[2])) / WINGS } else { 0.0 }), WINGS, window(90.0, 58.0, 3.0), Blend::Max),
            (a.paint("The wyvern's back", |s| { let p = hide.at(s); if p.along.abs() > 10.5 { 0.0 } else { capped(s, body.relief(&p)) / BACK } }), BACK, window(90.0, 64.0, 4.0), Blend::Max),
            (a.paint("Neck and tail", |s| { let p = hide.at(s); if p.along.abs() < 6.5 { 0.0 } else { capped(s, body.relief(&p)) / NECK } }), NECK, neck, Blend::Max),
            (a.paint("Wall scales", |s| capped(s, wall_scales(&hide.at(s), s, a.bore) * WALL) / WALL), WALL, window(90.0, 170.0, 8.0), Blend::Max),
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
        caps = fold_caps(&a, &hide, &spots, 0.35);
        layers = painted(&caps);
    }
    for (alpha, height, win, blend) in layers {
        paint(&mut d, &mut lib, &a, &mut bites, alpha, height, win, blend)?;
    }

    // Membrane veins cut at the bench.
    const GRAVER: f64 = 0.08;
    let graver = a.paint("Graver's veins", |s| if on_roof(s) { wy.veins(u_of(s), s.p[2]) } else { 0.0 });
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
    d.stamps.push(stamp("Spade tail, blank".into(), palm, spade_outline(), 0.56));
    for (side, name) in [(1.0, "high"), (-1.0, "low")] {
        d.stamps.push(Stamp { sink_mm: -0.02, draft_deg: 0.0, cut: true, bench: true, ..stamp(format!("Spade tail, {name} barb cut at the bench"), palm, barb_cutter(side), 0.8) });
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

fn write(out: &Path, draft: bool, verify: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let steps = if draft { (768, 320) } else { (1536, 448) };
    let params = BuildParams { theta_steps: steps.0, profile_steps: steps.1, refine: None, ..Default::default() };
    let (mut d, lib, bites) = design(params)?;
    d.build = params;
    let built = ringdesign_core::mesh::try_build(&d, &lib, params)?;
    let v = &built.report.validation;
    println!("{}: {} triangles, watertight {}, {} degenerate, {:.0} ms; stamps {} {:?}", d.name, built.mesh.faces.len(), v.watertight, built.report.quality.degenerate_faces, built.report.build_ms, built.solids.stamped, built.solids.notes);
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
    let metal = [Part::metal(&built.mesh, render::GOLD)];
    let edge = if draft { 1100 } else { 1600 };
    for (name, yaw, pitch) in [("hero", 0.48, 1.0), ("face", 0.0, PI * 0.5), ("palm", PI, PI * 0.5), ("side", PI * 0.5, 0.62), ("cheek", 0.0, 0.22), ("shoulder", 1.25, 1.05), ("reverse", PI, 0.8)] {
        render::write_png_parts(out.join(format!("{name}.png")), &metal, yaw, pitch, edge)?;
    }
    crop_png(&out.join("detail.png"), &metal, 0.3, 1.15, if draft { 2000 } else { 2800 }, 0.5)?;
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    bare.stamps.clear();
    let b = ringdesign_core::mesh::try_build(&bare, &lib, params)?;
    pair_png(&out.join("bare-finished.png"), &[Part::metal(&b.mesh, render::GOLD)], &metal, 0.48, 1.0, if draft { 800 } else { 1200 })?;
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/draco"));
    write(&out, args.iter().any(|a| a == "--draft"), args.iter().any(|a| a == "--verify"))
}
