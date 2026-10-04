//! Cataphracta — Gekko, the tokay: a tokay gecko spread along the crown, cast in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_gekko
//! target/release/examples/cataphracta_gekko [OUT_DIR] [--draft] [--verify] [--block-out]
//!
//! The animal is one sculpted part lying along the crown with its spine on the crest: a broad flat wedge of a head
//! with two great lidded eyes and slit pupils, a plump spotted body, four limbs bent at elbow and knee whose feet are
//! planted flat on the crown's shoulders with five splayed toes each ending in a broad pad, and a ringed tail running
//! down the crest toward the palm. The sculpt is a distance field meshed by `sculpt`, joined to a keyed Flat band whose
//! crown flanks carry granules and whose side faces carry rows of tubercles. Lost wax by Logan's rule of 2026-10-03:
//! 0.8 mm minimum section, no pull rule; the two-part undercut is reported as a number.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    cad::{Attach, Component, Document, Feature, Operation, Placement, measure, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Layer, LayerEntry},
    manufacturing as mf, mesh,
    render,
    sculpt::{self, ellipsoid, round_cone, smin},
    reptile::svg::{self as rsvg, Params},
    setting::{RowPath, Stamp, StampRow, StampTop, stamp_row},
    stl,
    svg::SvgAlpha,
};
use serde_json::{Value, json};
use std::{
    f64::consts::PI,
    path::{Path, PathBuf},
    time::Instant,
};

type P3 = [f64; 3];

const NAME: &str = "Gekko \u{2014} the tokay";
const SLUG: &str = "gekko";
const BORE_MM: f64 = 18.6;
/// The face, mm: across the band (along the finger) and round the ring.
const FACE_W_MM: f64 = 15.0;
const FACE_L_MM: f64 = 20.0;

/// Nothing of the sculpt comes nearer the finger axis than the bore plus this, mm.
const BORE_CLEAR_MM: f64 = 0.6;
/// How deep the sculpt reaches into the band under its own outline, mm: past it the band is metal already.
const SINK_MM: f64 = 0.45;
const STEP_MM: f64 = 0.05;
/// The investment's fill floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const FACES: usize = 210_000;

fn params(draft: bool) -> BuildParams {
    let (t, p) = if draft { (768, 320) } else { (1536, 448) };
    BuildParams { theta_steps: t, profile_steps: p, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..BuildParams::default() }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Factory 017 Tonneau, 20 mm round the ring by 15 across (the master's 16 x 12 at 125%), cast in lost wax: its long
/// axis runs the way the gecko lies, and its width takes the splayed feet.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    let source = PRESETS.iter().find(|p| p.id == "017").expect("017").load().expect("017 loads");
    ImportedBase::attach(&mut d, source).expect("attach");
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = FACE_W_MM;
    d.shank.head.length_mm = FACE_L_MM;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).expect("bore");
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.crisp_relief = std::env::var_os("GEKKO_CRISP").is_some();
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

// --- Geometry helpers ------------------------------------------------------------------------------------------------

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
fn norm(a: P3) -> f64 {
    dot(a, a).sqrt()
}
/// Deterministic jitter in 0..1.
fn hash(k: usize, salt: u64) -> f64 {
    let mut x = (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// The bare ring's top seen from above: the height of its outer surface over a plan grid in world `x` and `z`, read
/// by casting down onto the built bare mesh. Over the signet's face it is the face; past the face's ends it is the
/// shoulder falling away to the shank.
struct Ground {
    x0: f64,
    z0: f64,
    step: f64,
    nx: usize,
    nz: usize,
    y: Vec<f64>,
    /// How far each grid point stands inside the flat face from its edge, mm; 0 off the face.
    inset: Vec<f64>,
}

impl Ground {
    fn of(d: &RingDesign) -> Result<Self> {
        use ringdesign_core::interaction::bvh::Bvh;
        let built = mesh::try_build(d, &AlphaLibrary::default(), BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() })?;
        let bvh = Bvh::build(&built.mesh);
        let (x0, z0, step) = (-16.0, -9.5, 0.05);
        let (nx, nz) = ((32.0 / step) as usize + 1, (19.0 / step) as usize + 1);
        let mut y = vec![-50.0; nx * nz];
        for i in 0..nx {
            for j in 0..nz {
                let (x, z) = (x0 + i as f64 * step, z0 + j as f64 * step);
                if let Some((_, t)) = bvh.ray(&built.mesh, [x, 40.0, z], [0.0, -1.0, 0.0]) {
                    y[i * nz + j] = 40.0 - t;
                }
            }
        }
        // The flat face, and each of its points' distance in from its edge (a two-pass chamfer).
        let face = y[(nx / 2) * nz + nz / 2];
        let mut inset: Vec<f64> = y.iter().map(|h| if (h - face).abs() < 0.005 { f64::MAX } else { 0.0 }).collect();
        let (a, b) = (step, step * std::f64::consts::SQRT_2);
        for i in 0..nx {
            for j in 0..nz {
                let mut v = inset[i * nz + j];
                if i > 0 { v = v.min(inset[(i - 1) * nz + j] + a); }
                if j > 0 { v = v.min(inset[i * nz + j - 1] + a); }
                if i > 0 && j > 0 { v = v.min(inset[(i - 1) * nz + j - 1] + b); }
                if i > 0 && j + 1 < nz { v = v.min(inset[(i - 1) * nz + j + 1] + b); }
                inset[i * nz + j] = v;
            }
        }
        for i in (0..nx).rev() {
            for j in (0..nz).rev() {
                let mut v = inset[i * nz + j];
                if i + 1 < nx { v = v.min(inset[(i + 1) * nz + j] + a); }
                if j + 1 < nz { v = v.min(inset[i * nz + j + 1] + a); }
                if i + 1 < nx && j + 1 < nz { v = v.min(inset[(i + 1) * nz + j + 1] + b); }
                if i + 1 < nx && j > 0 { v = v.min(inset[(i + 1) * nz + j - 1] + b); }
                inset[i * nz + j] = v;
            }
        }
        Ok(Self { x0, z0, step, nx, nz, y, inset })
    }
    /// How far plan `x`, `z` stands inside the flat face from its edge, mm, bilinear; 0 off it.
    fn inset(&self, x: f64, z: f64) -> f64 {
        let fx = ((x - self.x0) / self.step).clamp(0.0, (self.nx - 1) as f64 - 1e-9);
        let fz = ((z - self.z0) / self.step).clamp(0.0, (self.nz - 1) as f64 - 1e-9);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (tx, tz) = (fx - i as f64, fz - j as f64);
        let g = |a: usize, b: usize| self.inset[a * self.nz + b];
        (g(i, j) * (1.0 - tx) + g(i + 1, j) * tx) * (1.0 - tz) + (g(i, j + 1) * (1.0 - tx) + g(i + 1, j + 1) * tx) * tz
    }
    /// The ground's height at plan `x`, `z`, bilinear.
    fn s(&self, x: f64, z: f64) -> f64 {
        let fx = ((x - self.x0) / self.step).clamp(0.0, (self.nx - 1) as f64 - 1e-9);
        let fz = ((z - self.z0) / self.step).clamp(0.0, (self.nz - 1) as f64 - 1e-9);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (tx, tz) = (fx - i as f64, fz - j as f64);
        let g = |a: usize, b: usize| self.y[a * self.nz + b];
        (g(i, j) * (1.0 - tx) + g(i + 1, j) * tx) * (1.0 - tz) + (g(i, j + 1) * (1.0 - tx) + g(i + 1, j + 1) * tx) * tz
    }
    /// How far a point stands over the ground, straight up, mm: negative under it.
    fn band_sd(&self, p: P3) -> f64 {
        p[1] - self.s(p[0], p[2])
    }
}

/// The body frame over the face: `x` mm round the ring's tangent at the top (toward the head positive), `h` mm over
/// the ground straight up, `w` mm along the finger.
#[derive(Clone, Copy)]
struct Frame<'a> {
    crest: &'a Ground,
}

impl Frame<'_> {
    fn local(&self, p: P3) -> P3 {
        [p[0], p[1] - self.crest.s(p[0], p[2]), p[2]]
    }
    fn world(&self, q: P3) -> P3 {
        [q[0], self.crest.s(q[0], q[2]) + q[1], q[2]]
    }
}

// --- The tokay -------------------------------------------------------------------------------------------------------

/// What a shape belongs to, for the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
enum Kind {
    Body,
    Head,
    Limb,
    Toe,
    Pad,
    Tail,
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    /// An ellipsoid at a frame point, semi-axes along x, h, w; `snout` narrows its w toward +x.
    Egg { c: P3, r: P3, snout: f64 },
    /// A rounded cone between two frame points.
    Limb { a: P3, b: P3, ra: f64, rb: f64 },
}

#[derive(Clone, Copy, Debug)]
struct Prim {
    kind: Kind,
    shape: Shape,
    /// Blend radius into what came before, mm.
    blend: f64,
}

impl Prim {
    fn eval(&self, q: P3) -> f64 {
        match self.shape {
            Shape::Egg { c, r, snout } => {
                let d = sub(q, c);
                let narrow = if snout > 0.0 { 1.0 - snout * (d[0] / r[0]).clamp(0.0, 1.0).powf(1.1) } else { 1.0 };
                ellipsoid(d, [r[0], r[1], r[2] * narrow])
            }
            Shape::Limb { a, b, ra, rb } => round_cone(q, a, b, ra, rb),
        }
    }
    /// A sphere in the frame that holds the shape.
    fn bound(&self) -> (P3, f64) {
        match self.shape {
            Shape::Egg { c, r, .. } => (c, r[0].max(r[1]).max(r[2])),
            Shape::Limb { a, b, ra, rb } => (mul(add(a, b), 0.5), 0.5 * norm(sub(b, a)) + ra.max(rb)),
        }
    }
}

/// A spot on the hide: a frame plan point and its radius.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Spot {
    x: f64,
    w: f64,
    r: f64,
}

/// The tokay's measurements, frame mm.
const HEAD_X: f64 = 6.2;
const EYE_X: f64 = 5.6;
const EYE_W: f64 = 2.85;
const EYE_R: f64 = 0.8;
const VENT_X: f64 = -6.2;
/// Front and hind limbs: where each leaves the body.
const LEGS: [(f64, f64); 2] = [(2.2, 1.0), (-4.0, -1.0)];
/// The skin's granules: lattice pitch and height; the spots' height.
const GRANULE_PITCH_MM: f64 = 0.5;
const GRANULE_MM: f64 = 0.0;
const SPOT_MM: f64 = 0.2;

struct Gekko<'a> {
    frame: Frame<'a>,
    prims: Vec<Prim>,
    spots: Vec<Spot>,
    /// The eyes' centre height over the crest.
    eye_h: f64,
    limbs: Vec<LimbPlan>,
    bore_r: f64,
}

/// The tail's centre line and radius at a share `t` of its length from the vent: off the face's end and down the
/// shoulder, swinging to one side and curling its tip back.
fn tail_at(g: &Ground, t: f64) -> (P3, f64) {
    let r = 1.45 * (1.0 - t).powf(1.0) + 0.4 * t;
    let pts: [(f64, f64); 7] = [(VENT_X, 0.0), (-7.4, -0.8), (-8.3, -0.6), (-8.6, 0.4), (-8.2, 1.4), (-7.9, 2.4), (-8.2, 3.2)];
    let n = pts.len() - 1;
    let f = (t.clamp(0.0, 1.0) * n as f64).min(n as f64 - 1e-9);
    let i = f.floor() as usize;
    let u = f - i as f64;
    let q = |k: isize| pts[(k.max(0) as usize).min(n)];
    let (p0, p1, p2, p3) = (q(i as isize - 1), q(i as isize), q(i as isize + 1), q(i as isize + 2));
    let cr = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * u + (2.0 * a - 5.0 * b + 4.0 * c - d) * u * u + (-a + 3.0 * b - 3.0 * c + d) * u * u * u);
    let _ = g;
    ([cr(p0.0, p1.0, p2.0, p3.0), -0.35 * r, cr(p0.1, p1.1, p2.1, p3.1)], r)
}

fn body_prims(crest: &Ground) -> Vec<Prim> {
    let egg = |kind, c: P3, r: P3, snout: f64, blend: f64| Prim { kind, shape: Shape::Egg { c, r, snout }, blend };
    let limb = |kind, a: P3, b: P3, ra: f64, rb: f64, blend: f64| Prim { kind, shape: Shape::Limb { a, b, ra, rb }, blend };
    let mut out = vec![
        // Trunk: plump and smooth, nearly 5 mm across.
        egg(Kind::Body, [-1.6, -0.7, 0.0], [4.4, 2.5, 2.8], 0.0, 0.0),
        egg(Kind::Body, [1.7, -0.7, 0.0], [2.2, 2.35, 2.5], 0.0, 0.6),
        // Neck: narrower than the head, so the jaw's hinge stands out from it.
        egg(Kind::Head, [3.5, -0.7, 0.0], [1.3, 2.2, 1.9], 0.0, 0.35),
        // Head: a broad flat triangle to a blunt round snout.
        egg(Kind::Head, [HEAD_X, -0.6, 0.0], [3.1, 1.95, 3.0], 0.4, 0.3),
        // The jaw's hinges, the head's broadest point, behind the eyes.
        egg(Kind::Head, [4.7, -0.7, 2.3], [1.3, 1.5, 1.35], 0.0, 0.3),
        egg(Kind::Head, [4.7, -0.7, -2.3], [1.3, 1.5, 1.35], 0.0, 0.3),
        // The hips.
        egg(Kind::Body, [VENT_X + 1.3, -0.7, 0.0], [1.9, 2.2, 2.4], 0.0, 0.5),
        // The shoulder girdle's and the pelvis's swells over the limb roots.
        egg(Kind::Body, [2.0, -0.5, 1.75], [1.25, 1.65, 1.05], 0.0, 0.45),
        egg(Kind::Body, [2.0, -0.5, -1.75], [1.25, 1.65, 1.05], 0.0, 0.45),
        egg(Kind::Body, [-4.2, -0.5, 1.75], [1.3, 1.7, 1.1], 0.0, 0.45),
        egg(Kind::Body, [-4.2, -0.5, -1.75], [1.3, 1.7, 1.1], 0.0, 0.45),
    ];
    // The tail: a chain of rounded cones from the vent down the crest.
    let n = 30;
    for k in 0..n {
        let (a, ra) = tail_at(crest, k as f64 / n as f64);
        let (b, rb) = tail_at(crest, (k + 1) as f64 / n as f64);
        out.push(limb(Kind::Tail, a, b, ra, rb, if k == 0 { 0.5 } else { 0.03 }));
    }
    // Four limbs, each bent at the elbow or knee, ending in a foot planted flat on the crown's shoulder with five toes
    // splayed from it, each tipped with a broad round pad.
    for l in limb_plans(crest) {
        let [sh, el, wr, ft] = l.joints;
        let thick = if l.hind { 1.15 } else { 1.0 };
        out.push(limb(Kind::Limb, sh, el, 0.72 * thick, 0.5, 0.35));
        out.push(egg(Kind::Limb, el, [0.52, 0.48, 0.52], 0.0, 0.12));
        out.push(limb(Kind::Limb, el, wr, 0.48, 0.42, 0.1));
        out.push(limb(Kind::Limb, wr, ft, 0.42, 0.44, 0.08));
        out.push(egg(Kind::Limb, ft, [0.62, 0.36, 0.62], 0.0, 0.12));
        for (root, tip) in &l.toes {
            out.push(limb(Kind::Toe, *root, *tip, 0.47, 0.43, 0.08));
            out.push(egg(Kind::Pad, *tip, [0.48, 0.42, 0.48], 0.0, 0.06));
        }
    }
    out
}

/// One limb: shoulder (or hip), elbow (or knee), wrist (or ankle) and the foot's centre, in the frame, and its toes.
struct LimbPlan {
    hind: bool,
    joints: [P3; 4],
    toes: Vec<(P3, P3)>,
}

/// The four limbs in the tokay's sprawl seen from above: the upper arm out and back, the forearm out and forward,
/// the hand's toes fanned forward; the thigh out and forward, the shin out and back, the foot's toes fanned back.
fn limb_plans(crest: &Ground) -> Vec<LimbPlan> {
    let on = |x: f64, w: f64, lift: f64| [x, lift, w];
    let mut out = Vec::new();
    for s in [1.0, -1.0] {
        for (x0, hind) in [(LEGS[0].0, false), (LEGS[1].0, true)] {
            let dir = if hind { -1.0 } else { 1.0 };
            let sh = [x0, -0.2, 1.8 * s];
            let el = on(x0 - 1.4 * dir, 3.9 * s, 0.95);
            let wr = on(x0 - 0.25 * dir, 4.75 * s, 0.55);
            let ft = on(x0 + 0.4 * dir, 5.1 * s, 0.4);
            let mut toes = Vec::new();
            for k in 0..5 {
                let j = k as f64 - 2.0;
                // Fanned about a line out and toward the head (forelimb) or the tail (hind limb).
                let a = (46.0 * j + 28.0 * dir).to_radians();
                // Each toe ends with its pad at least 0.75 mm inside the flat face, never over the factory's wall.
                let mut len: f64 = [1.45, 1.65, 1.75, 1.65, 1.45][k];
                let tip = |len: f64| (ft[0] + a.sin() * len, ft[2].abs() + a.cos() * len);
                while len > 0.9 && crest.inset(tip(len).0, tip(len).1 * s) < 0.75 {
                    len -= 0.02;
                }
                let (x, w) = tip(len);
                let root = on(ft[0] + a.sin() * 0.3, (ft[2].abs() + a.cos() * 0.3) * s, 0.3);
                toes.push((root, on(x, w * s, 0.32)));
            }
            out.push(LimbPlan { hind, joints: [sh, el, wr, ft], toes });
        }
    }
    out
}

/// The tokay's spots: rows down both flanks and on the head's and tail's sides, each a smooth raised disc.
fn spots() -> Vec<Spot> {
    // Two loose rows of flat-topped spots down the back and a scatter on the flanks, of mixed sizes, never in ranks.
    let mut out: Vec<Spot> = Vec::new();
    let mut tries = 0usize;
    while out.len() < 15 && tries < 5000 {
        tries += 1;
        let x = VENT_X + 0.5 + (3.2 - VENT_X - 0.5) * hash(tries, 41);
        let w = 2.2 * (2.0 * hash(tries, 42) - 1.0);
        let r = 0.4 + 0.38 * hash(tries, 43);
        if w.abs() < 0.35 || out.iter().any(|o| (o.x - x).hypot(o.w - w) < o.r + r + 0.55) {
            continue;
        }
        out.push(Spot { x, w, r });
    }
    out
}

impl<'a> Gekko<'a> {
    fn new(frame: Frame<'a>, bore_r: f64) -> Self {
        let prims = body_prims(frame.crest);
        let mut gek = Self { prims, spots: spots(), eye_h: 0.0, limbs: limb_plans(frame.crest), frame, bore_r };
        // The eyes sit on the head's top either side, a quarter of each ball sunk into it.
        let (mut lo, mut hi) = (-3.0, 4.0);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if gek.body_at([EYE_X, m, EYE_W]) > 0.0 { hi = m } else { lo = m }
        }
        gek.eye_h = lo + 0.65 - EYE_R;
        gek
    }

    /// The soft body, eyes left out: every shape blended in order.
    fn body_at(&self, q: P3) -> f64 {
        let mut f = f64::MAX;
        for s in &self.prims {
            let (c, r) = s.bound();
            let far = norm(sub(q, c)) - r;
            if f != f64::MAX && far > f + s.blend + 0.05 {
                continue;
            }
            let v = s.eval(q);
            f = if f == f64::MAX { v } else { smin(f, v, s.blend) };
        }
        // Brow ridges over the eyes, the skull's planes between them.
        for side in [1.0, -1.0] {
            let brow = ellipsoid(sub(q, [EYE_X + 0.1, 0.8, (EYE_W - 0.75) * side]), [0.95, 0.32, 0.45]);
            f = smin(f, brow, 0.25);
        }
        // Two nostrils at the snout's tip, only into the skin.
        if f > -0.3 && f < 0.3 {
            for side in [1.0, -1.0] {
                let n = norm(sub(q, [HEAD_X + 2.7, 0.75, 0.5 * side])) - 0.17;
                f = f.max(-n);
            }
        }
        // The mouth: a line cut along each side of the head from the snout round to the jaw's angle, only into the skin.
        if q[0] > 5.6 && q[0] < HEAD_X + 3.6 && q[2].abs() > 0.6 && f > -0.3 {
            let t = ((q[0] - 5.6) / (HEAD_X + 3.6 - 5.6)).clamp(0.0, 1.0);
            let line = -0.6 + 0.25 * t;
            let g = (q[1] - line).abs() - 0.08 * (1.0 - 0.6 * t);
            f = f.max(-(g.max(-f - 0.22)));
        }
        f
    }

    /// The eyes: two great balls standing out of the head's sides, each with an upright slit pupil cut into its outer
    /// face.
    fn eyes_at(&self, q: P3) -> f64 {
        let mut f = f64::MAX;
        for s in [1.0, -1.0] {
            let c = [EYE_X, self.eye_h, EYE_W * s];
            let d = sub(q, c);
            let ball = norm(d) - EYE_R;
            // The eye looks up and out. Its pupil is an upright groove 0.25 mm wide and 0.12 deep down the middle of
            // its face; a smooth lid rim 0.2 mm round rings its base.
            let look = [0.0, 0.55, 0.835 * s];
            let up = [0.0, 0.835, -0.55 * s];
            let (fa, fv) = (dot(d, look), dot(d, up));
            let (gw, gd) = groove_dims(); let groove = (d[0].abs() - gw).max(EYE_R - gd - norm(d)).max(0.3 * EYE_R - fa).max(fv.abs() - 0.6 * EYE_R);
            let eye = ball.max(-groove);
            let ring_c = sub(d, mul(look, -0.05 * EYE_R));
            let along = dot(ring_c, look);
            let across = norm(sub(ring_c, mul(look, along)));
            let lid = (across - 0.95 * EYE_R).hypot(along) - 0.1;
            f = f.min(smin(eye, lid, 0.06));
        }
        f
    }

    /// The hide's relief at a point, mm: smooth raised spots, fine granules between them; and apart, the spots on the
    /// spine, which straddle the parting line and fall away from it either side, so they stand whatever the slope.
    fn hide(&self, q: P3, p: P3) -> (f64, f64) {
        let (mut spot, spine) = (0.0f64, 0.0f64);
        for s in &self.spots {
            let (dx, dw) = (q[0] - s.x, q[2] - s.w);
            let ang = dw.atan2(dx);
            let wobble = 1.0 + 0.08 * (2.0 * ang + 7.0 * s.x).sin() + 0.04 * (3.0 * ang + 3.0 * s.w).sin();
            let d = dx.hypot(dw) / wobble;
            if d < s.r + 0.25 {
                let v = 1.0 - smoothstep(s.r - 0.16, s.r + 0.14, d);
                spot = spot.max(v);
            }
        }
        (SPOT_MM * spot + GRANULE_MM * granules(p) * (1.0 - spot.max(spine)), SPOT_MM * spine)
    }

    /// The eyes alone, in world mm.
    fn eyes_world(&self, p: P3) -> f64 {
        self.eyes_at(self.frame.local(p))
    }

    /// The sculpt: the body with its hide (granules and smooth spots, the tail's rings), the eyes set into the head,
    /// held clear of the bore and kept to a skin over the band.
    fn field(&self, p: P3) -> f64 {
        let q = self.frame.local(p);
        let mut f = self.body_at(q);
        let eyes = self.eyes_at(q);
        if f.abs() < 0.35 {
            let toe_near = self.prims.iter().filter(|s| matches!(s.kind, Kind::Toe | Kind::Pad)).map(|s| s.eval(q)).fold(f64::MAX, f64::min);
            let fade = smoothstep(0.05, 0.3, toe_near) * smoothstep(0.05, 0.3, eyes);
            let (flank, spine) = self.hide(q, p);
            f -= (flank + spine) * fade;
        }
        let _ = eyes;
        // The face's ground: a pebbled plate over the face, inset from its rim, the gecko grown out of it.
        if q[1] < 0.5 {
            f = smin(f, self.ground_plate(q, f), 0.12);
        }
        f.max(-self.frame.crest.band_sd(p) - SINK_MM).max(self.bore_r + BORE_CLEAR_MM - p[0].hypot(p[1]))
    }

    /// The pebbled plate on the face: granules `CROWN_GRANULE_MM` high over the face, stopping `RIM_MM` short of its rim.
    fn ground_plate(&self, q: P3, figure: f64) -> f64 {
        // The plate covers the whole flat face to within 0.05 mm of its edge, so the face shows nowhere: a polished
        // border a millimetre wide, the pebbling inside it, and at the very edge the plate's top runs down under the
        // face's own plane, meeting the factory's wall at its hard corner.
        let inset = self.frame.crest.inset(q[0], q[2]);
        let rim = 0.05 - inset;
        if rim > 0.3 {
            return rim;
        }
        // A quiet halo: the granules sink to half their height within a millimetre of the figure.
        let halo = 0.5 + 0.5 * smoothstep(0.15, 1.0, figure);
        let field = smoothstep(0.9, 1.5, inset);
        let top = 0.03 + CROWN_GRANULE_MM * crown_granules(q[0], q[2]) * halo * field - 0.11 * (1.0 - smoothstep(0.08, 0.3, inset));
        (q[1] - top).max(-q[1] - SINK_MM).max(rim)
    }

    /// The sculpt's box in world mm.
    fn bounds(&self, pad: f64) -> (P3, P3) {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for s in &self.prims {
            let (c, r) = s.bound();
            for dx in [-r, 0.0, r] {
                for dh in [-r, r] {
                    let w = self.frame.world([c[0] + dx, c[1] + dh, c[2]]);
                    for k in 0..2 {
                        lo[k] = lo[k].min(w[k] - r - pad);
                        hi[k] = hi[k].max(w[k] + r + pad);
                    }
                }
            }
            lo[2] = lo[2].min(c[2] - r - pad);
            hi[2] = hi[2].max(c[2] + r + pad);
        }
        // The ground plate covers the face: the box spans it, and stands no higher than the figure does.
        let face = self.frame.crest.s(0.0, 0.0);
        lo[0] = lo[0].min(-10.5);
        hi[0] = hi[0].max(10.5);
        lo[2] = lo[2].min(-7.6);
        hi[2] = hi[2].max(7.6);
        lo[1] = face - SINK_MM - 0.3;
        hi[1] = face + 3.2;
        (lo, hi)
    }
}

/// The hide's granules, 0..1: cells of a jittered 3D lattice, each a low dome with a groove where two cells meet.
fn granules(p: P3) -> f64 {
    let g = GRANULE_PITCH_MM;
    let c: [i64; 3] = std::array::from_fn(|k| (p[k] / g).floor() as i64);
    let (mut f1, mut f2) = (f64::MAX, f64::MAX);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cell = [c[0] + dx, c[1] + dy, c[2] + dz];
                let key = (cell[0].wrapping_mul(73_856_093) ^ cell[1].wrapping_mul(19_349_663) ^ cell[2].wrapping_mul(83_492_791)) as usize;
                let o: P3 = std::array::from_fn(|k| (cell[k] as f64 + 0.15 + 0.7 * hash(key, 50 + k as u64)) * g);
                let d = norm(sub(p, o));
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    let t = ((f2 - f1) / 0.16).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) * (1.0 - 0.5 * (f1 / (0.6 * g)).min(1.0).powi(2))
}

// --- The pull fill ---------------------------------------------------------------------------------------------------




// --- Meshing ---------------------------------------------------------------------------------------------------------

#[derive(Default, serde::Serialize)]
struct Composition {
    lamellae: usize,
    face_y_mm: f64,
    prims: usize,
    spots: usize,
    raw_faces: usize,
    faces: usize,
    volume_mm3: f64,
    box_mm: [P3; 2],
    sculpt_s: f64,
    decimation: (usize, f64),
    notes: Vec<String>,
}




fn sculpt_solid(gek: &Gekko, comp: &mut Composition) -> csg::Solid {
    let t = Instant::now();
    let field = |p: P3| gek.field(p);
    let (lo, hi) = gek.bounds(0.4);
    comp.box_mm = [lo, hi];
    let mut raw = sculpt::tetra_mesh(lo, hi, STEP_MM, &field);
    comp.raw_faces = raw.f.len();
    let unrelaxed = raw.clone();
    sculpt::relax(&mut raw, &field, 3);
    for reach in [0.3, 0.6, 1.2, 1e9] {
        let sites = sculpt::crossing_sites(&raw);
        if sites.is_empty() {
            break;
        }
        comp.notes.push(format!("relax undone within {reach} mm of {} crossings", sites.len()));
        for (v, orig) in raw.v.iter_mut().zip(&unrelaxed.v) {
            if sites.iter().any(|s| dot(sub(*v, *s), sub(*v, *s)) < reach * reach) {
                *v = *orig;
            }
        }
    }
    let mut nets = None;
    'caps: for target in [FACES, FACES + FACES / 10, FACES + FACES / 4] {
        for cap in [2e-3, 1e-3, 5e-4, 2e-4] {
            let d = sculpt::decimate(&raw, target, cap, 3.0, 18.0, 35.0);
            if csg::self_crossings(&d) == 0 {
                comp.decimation = (target, cap);
                nets = Some(d);
                break 'caps;
            }
        }
    }
    let nets = nets.unwrap_or_else(|| sculpt::clean_decimate(&raw, FACES));
    let s = sculpt::settle(nets, &field, &|_| false);
    comp.faces = s.f.len();
    comp.volume_mm3 = sculpt::closure(&s).1;
    comp.sculpt_s = t.elapsed().as_secs_f64();
    s
}

/// The crown's granules, 0..1, at `along` mm round the ring and `z` mm across: an irregular pebbling of low domed
/// cells, each parted from its neighbours by a groove (Worley's second-minus-first distance), as a tokay's hide is.
const CROWN_GRANULE_MM: f64 = 0.13;
const CROWN_PITCH_MM: f64 = 0.8;
fn crown_granules(along: f64, z: f64) -> f64 {
    let g = CROWN_PITCH_MM;
    let (ci, cj) = ((along / g).floor() as i64, (z / g).floor() as i64);
    let (mut f1, mut f2) = (f64::MAX, f64::MAX);
    for di in -2..=2 {
        for dj in -2..=2 {
            let (i, j) = (ci + di, cj + dj);
            let key = (i.wrapping_mul(73_856_093) ^ j.wrapping_mul(19_349_663)) as usize;
            let cx = (i as f64 + 0.1 + 0.8 * hash(key, 61)) * g;
            let cz = (j as f64 + 0.1 + 0.8 * hash(key, 62)) * g;
            let d = (along - cx).hypot(z - cz);
            if d < f1 {
                f2 = f1;
                f1 = d;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    let t = smoothstep(0.08, 0.36, f2 - f1);
    t * (0.55 + 0.45 * (1.0 - (f1 / (0.8 * g)).min(1.0).powi(2)))
}





/// Crossings once the mesh is stored, its vertices rounded to the stored grid as a stored part keeps them.
fn stored_crossings(s: &csg::Solid) -> usize {
    let q = ringdesign_core::cad::stored::QUANTUM_MM;
    let r = csg::Solid { v: s.v.iter().map(|p| p.map(|c| (c / q).round() * q)).collect(), f: s.f.clone() };
    csg::self_crossings(&r)
}

/// The shank's hide, struck as stamps (relief the field would also hold, but a field layer on this stock dips the bore
/// by 0.011 mm; see the report): the palm's lamellae, crescent plates graded 0.9 to 1.45 mm with the split chevron at
/// 270 degrees; and rows of domed tubercles down both side faces of the shank.
const LAMELLA_MM: f64 = 0.36;
const LAMELLA_SPAN_DEG: f64 = 68.0;
fn shank_stamps(d: &mut RingDesign) -> Result<usize> {
    let bare = mesh::try_build(d, &AlphaLibrary::default(), params(true))?;
    let near = |theta: f64, tol: f64| bare.mesh.vertices.iter().filter(move |v| { let t = (v.1 as f64).atan2(v.0 as f64).to_degrees().rem_euclid(360.0); (t - theta).abs() < tol });
    let outer_r = near(270.0, 1.0).map(|v| (v.0 as f64).hypot(v.1 as f64)).fold(0.0, f64::max);
    let half_w_at = |theta: f64| {
        let rmax = near(theta, 1.0).map(|v| (v.0 as f64).hypot(v.1 as f64)).fold(0.0, f64::max);
        near(theta, 1.0).filter(|v| (v.0 as f64).hypot(v.1 as f64) > rmax - 0.7).map(|v| (v.2 as f64).abs()).fold(0.0, f64::max)
    };
    let ctx = d.field_context();
    let stamp = |name: String, theta: f64, outline: Vec<[f64; 2]>, height: f64, top: StampTop| Stamp {
        name,
        theta_deg: theta,
        v_mm: ctx.crest_v_mm,
        rot_deg: 0.0,
        outline,
        height_mm: height,
        sink_mm: 0.3,
        draft_deg: 0.0,
        cut: false,
        bench: false,
        along_pull: false,
        tier: 0,
        top,
        fine_cap: true,
    };
    let mut out = Vec::new();
    // The split lamella: a chevron pointing round the ring, its arms back along both halves of the band, each arm
    // 0.7 mm thick under 0.38 mm of relief, so it stands no taller than it is thick.
    let hw0 = half_w_at(270.0) - 0.45;
    out.push(stamp("Lamella, split".into(), 270.0, ringdesign_core::outline::rounded_polygon(&[[-0.6, -hw0], [0.1, -hw0], [0.75, 0.0], [0.1, hw0], [-0.6, hw0], [0.05, 0.0]], 0.1), 0.3, StampTop::Pillow { crown_mm: 0.08 }));
    for side in [1.0f64, -1.0] {
        let mut a = 0.9f64;
        let mut k = 0;
        while a < LAMELLA_SPAN_DEG {
            let pitch = 0.9 + 0.55 * smoothstep(0.0, LAMELLA_SPAN_DEG, a);
            let step = (pitch / outer_r).to_degrees();
            let theta = 270.0 + side * (a + 0.5 * step);
            let hw = half_w_at(theta) - 0.45;
            if hw > 1.0 {
                // A crescent: both edges bowed the same way, round the ring away from the palm's centre.
                let (half_l, sag) = (0.5 * (pitch - 0.3), 0.3);
                let n = 16;
                let mut o = Vec::new();
                for i in 0..=n {
                    let z = -hw + 2.0 * hw * i as f64 / n as f64;
                    o.push([side * (half_l + sag * (1.0 - (z / hw).powi(2))), z]);
                }
                for i in (0..=n).rev() {
                    let z = -hw + 2.0 * hw * i as f64 / n as f64;
                    o.push([side * (-half_l + sag * (1.0 - (z / hw).powi(2))), z]);
                }
                if side < 0.0 {
                    o.reverse();
                }
                let o = ringdesign_core::outline::rounded_polygon(&o, 0.08);
                k += 1;
                let h = LAMELLA_MM * (1.0 - 0.4 * smoothstep(LAMELLA_SPAN_DEG - 20.0, LAMELLA_SPAN_DEG, a));
                out.push(stamp(format!("Lamella, {} {k}", if side > 0.0 { "right" } else { "left" }), theta, o, h, StampTop::Taper { axis_deg: if side > 0.0 { 180.0 } else { 0.0 }, tip_mm: 0.1 }));
            }
            a += step;
        }
    }
    let n = out.len();
    d.stamps.extend(out);
    // The shoulders: staggered rows of domed tubercles over the shank's outer crown, from the head's walls down to
    // where the lamellae begin, larger toward the head.
    let hw = half_w_at(180.0) - 0.5;
    for (from, to) in [(142.0, 200.0), (340.0, 398.0)] {
        for (k, frac) in [-0.72, -0.25, 0.25, 0.72].into_iter().enumerate() {
            let size = 1.15 - 0.15 * (k % 2) as f64;
            let shift = if k % 2 == 1 { 0.5 } else { 0.0 };
            let r_sh = outer_r;
            let pitch = 1.45;
            let count = ((to - from) as f64).to_radians() * r_sh / pitch;
            let step = (to - from) / count;
            let row = StampRow {
                stamp: stamp("Tubercle".into(), 0.0, ringdesign_core::outline::circle(size), 0.3, StampTop::Dome { crown_mm: 0.16 }),
                path: RowPath::ChartV { v_mm: ctx.crest_v_mm + frac * hw },
                from_deg: from + shift * step,
                to_deg: to - (1.0 - shift) * step,
                count: count.floor() as u32,
                taper: 0.15,
                fold_clear_mm: 0.0,
                mirror_shoulders: false,
            };
            let mut struck = stamp_row(d, &row);
            for (j, st) in struck.iter_mut().enumerate() {
                st.name = format!("Tubercle, {from:.0} {k} {j}");
                // Toward the head the tubercles are larger.
                let _ = &st;
            }
            d.stamps.extend(struck);
        }
    }
    Ok(n)
}

/// The eyes, meshed apart at a finer step so the lids and pupils keep clean edges, and joined to the head.
fn eye_solid(gek: &Gekko) -> csg::Solid {
    let field = |p: P3| gek.eyes_world(p);
    let c = gek.frame.world([EYE_X, gek.eye_h, 0.0]);
    let pad = EYE_R + 0.4;
    let lo = [c[0] - pad, c[1] - pad, -EYE_W - pad];
    let hi = [c[0] + pad, c[1] + pad, EYE_W + pad];
    let mut raw = sculpt::tetra_mesh(lo, hi, 0.02, &field);
    sculpt::relax(&mut raw, &field, 3);
    for target in [32_000, 24_000, 40_000, 20_000, 48_000] {
        let s = sculpt::settle(sculpt::clean_decimate(&raw, target), &field, &|_| false);
        let x = stored_crossings(&s);
        if std::env::var_os("GEKKO_EYES").is_some() {
            println!("  eyes at {target}: {} faces, {x} crossings as stored", s.f.len());
        }
        if x == 0 {
            return s;
        }
    }
    raw
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}



struct Authored {
    d: RingDesign,
    lib: AlphaLibrary,
    comp: Composition,
    solid: csg::Solid,
}

fn author() -> Result<Authored> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let crest = Ground::of(&d)?;
    if std::env::var_os("GEKKO_GROUND").is_some() {
        for z in [-8.0, -7.0, -6.0, -5.0, -4.0, -3.0, 0.0] { println!("  z {z}: {}", (-12..=12).map(|x| format!("{:.1}", crest.s(x as f64, z))).collect::<Vec<_>>().join(" ")); }
        std::process::exit(0);
    }
    let mut comp = Composition { face_y_mm: crest.s(0.0, 0.0), ..Composition::default() };
    comp.lamellae = shank_stamps(&mut d)?;
    println!("  {} stamps ({} lamellae)", d.stamps.len(), comp.lamellae);
    let gek = Gekko::new(Frame { crest: &crest }, d.inner_radius_mm());
    if std::env::var_os("GEKKO_PROBE").is_some() {
        for l in &gek.limbs {
            let ft = l.joints[3];
            let w = gek.frame.world(ft);
            println!("  foot frame {:?} world {:?}: body {:.3} field {:.3} ground {:.3}", ft, w, gek.body_at(ft), gek.field(w), crest.s(ft[0], ft[2]));
        }
        let (lo, hi) = gek.bounds(0.4);
        println!("  bounds {:?} {:?}", lo, hi);
        std::process::exit(0);
    }
    comp.prims = gek.prims.len();
    comp.spots = gek.spots.len();
    if let Ok(h) = std::env::var("GEKKO_TESTLAYER") {
        let ctx = d.field_context();
        d.svgs.push(SvgAlpha { name: "Test".into(), svg: rsvg::tubercle_rows(&Params::new(1.3, 3.0, 0.42, 0.9)), invert: false });
        let mut t = ringdesign_core::tiling::TilingLayer::default_for("Test", &ctx);
        t.height_mm = h.parse().unwrap_or(0.0);
        let mut e = LayerEntry::new("Test", Layer::Tiling(t));
        e.window = ringdesign_core::field::Window::around(270.0, 60.0);
        d.layers.layers.push(e);
    }
    if let Ok(at) = std::env::var("GEKKO_AT") {
        for t in at.split(';') {
            let w: Vec<f64> = t.split(',').map(|x| x.trim().parse().unwrap()).collect();
            let q = gek.frame.local([w[0], w[1], w[2]]);
            let mut near: Vec<(f64, String)> = gek.prims.iter().map(|s| (s.eval(q), format!("{:?}", s.kind))).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            println!("  at {w:?} local {q:.3?} body {:.3} field {:.3} inset {:.3} nearest {:?}", gek.body_at(q), gek.field(w.clone().try_into().unwrap()), crest.inset(q[0], q[2]), &near[..4]);
        }
        std::process::exit(0);
    }
    if std::env::var_os("GEKKO_EYES").is_some() {
        let eyes = eye_solid(&gek);
        println!("  eyes {} faces", eyes.f.len());
        std::process::exit(0);
    }
    if std::env::var_os("GEKKO_NOSCULPT").is_some() {
        d.bake_all(&mut lib);
        return Ok(Authored { d, lib, comp, solid: csg::Solid { v: Vec::new(), f: Vec::new() } });
    }
    let solid = sculpt_solid(&gek, &mut comp);
    println!("  sculpt {} raw faces -> {}, {:.1} mm3, {:.1} s {:?}", comp.raw_faces, comp.faces, comp.volume_mm3, comp.sculpt_s, comp.notes);
    let packed = sculpt::packed(&solid)?;
    let eyes = eye_solid(&gek);
    println!("  eyes {} faces", eyes.f.len());
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe {
        kernel: "sculpt".into(),
        op: "tokay".into(),
        params: json!({"base": "017", "step_mm": STEP_MM, "faces": FACES, "spots": comp.spots}),
        digest: String::new(),
    };
    doc.append(Feature { id: next, name: "Tokay".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component: joined() })?;
    let recipe = stored::Recipe { kernel: "sculpt".into(), op: "tokay eyes".into(), params: json!({"eye_r_mm": EYE_R, "step_mm": 0.02}), digest: String::new() };
    doc.append(Feature { id: next + 1, name: "Eyes".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: sculpt::packed(&eyes)? }, component: joined() })?;
    d.bake_all(&mut lib);
    Ok(Authored { d, lib, comp, solid })
}

// --- Gates -----------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
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

/// The geometry gates at one build size.
struct Pass {
    triangles: usize,
    watertight: bool,
    degenerate: usize,
    crossings: usize,
    notes: Vec<String>,
    parts: Vec<(String, usize)>,
    joined: usize,
}

impl Pass {
    fn of(built: &mesh::BuildResult) -> Self {
        let (watertight, degenerate, crossings) = geometry(&built.mesh);
        let mut notes = built.solids.notes.clone();
        notes.extend(built.parts.notes.iter().cloned());
        Self { triangles: built.mesh.faces.len(), watertight, degenerate, crossings, notes, parts: part_crossings(built), joined: built.parts.joined }
    }
    fn ok(&self) -> bool {
        self.watertight && self.degenerate == 0 && self.crossings == 0 && self.notes.is_empty() && self.parts.iter().all(|p| p.1 == 0) && self.joined == 2
    }
    fn json(&self) -> Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "notes": self.notes, "made_part_crossings": self.parts, "parts_joined": self.joined})
    }
    fn line(&self) -> String {
        format!("{} tris, watertight {}, degenerate {}, crossings {}, notes {:?}, parts {:?}, joined {}", self.triangles, self.watertight, self.degenerate, self.crossings, self.notes, self.parts, self.joined)
    }
}

/// Every gate at one build size, as JSON, with whether they all passed.
struct Gated {
    json: Value,
    passed: bool,
    built: mesh::BuildResult,
    pattern: mesh::BuildResult,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams, label: &str) -> Result<Gated> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = ms(t);
    let pass = Pass::of(&built);
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).count();
    let nearest = built.mesh.vertices.iter().map(|q| (q.0 as f64).hypot(q.1 as f64) - bore).fold(f64::MAX, f64::min);
    for q in built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).take(4) {
        println!("  inside the bore: theta {:.2}, z {:.3}, r {:.4}", (q.1 as f64).atan2(q.0 as f64).to_degrees(), q.2, (q.0 as f64).hypot(q.1 as f64));
    }
    let band_field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let mut field = band_field.clone();
    castability::judge_parts(&mut field, d, &built);
    let dfm = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built).map_or(0, |r| r.stone_count as usize);
    let preview = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    let t = Instant::now();
    let walls = measure::thickness(&built.mesh, MIN_SECTION_MM);
    let census_ms = ms(t);
    // The lead's interim gate (2026-10-03): a wall zone whose thinnest reading is a real section, 0.05 mm up to the
    // floor, fails; one thinner than 0.05 mm is listed as a suspected census artifact and does not reshape the ring.
    const ARTIFACT_MM: f64 = 0.05;
    let real_walls: Vec<&measure::ThinZone> = walls.walls.iter().filter(|z| z.thinnest_mm >= ARTIFACT_MM).collect();
    let artifacts: Vec<Value> = walls.walls.iter().filter(|z| z.thinnest_mm < ARTIFACT_MM).map(|z| json!({"point": z.point, "area_mm2": z.area_mm2, "thinnest_mm": z.thinnest_mm, "span_mm": z.span_mm})).collect();
    let walls_ok = walls.assessed && walls.unresolved == 0 && real_walls.is_empty() && walls.walls.len() < measure::MAX_ZONES;
    let list = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", pass.watertight && pass.degenerate == 0 && pass.crossings == 0),
        ("the sculpted parts uncrossed as placed", pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, every stamp resolved, the tokay and its eyes joined", pass.notes.is_empty() && pass.joined == 2 && built.solids.stamped == d.stamps.len()),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax field verdict Castable with the 0.8 mm fill, band and with the part judged", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && band_field.verdict == Verdict::Castable && band_field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("lost-wax wall census of the finished ring: assessed, 0 unresolved, no wall zone with a real section of 0.05-0.8 mm (thinner zones listed as suspected census artifacts)", walls_ok),
        ("zero DFM findings", dfm.is_empty()),
        ("stones reported equal the preview", stones == preview),
        ("within 2 million triangles", pass.triangles <= 2_000_000),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
    ];
    let passed = list.iter().all(|g| g.1);
    println!("[{label}] {} ({build_ms:.0} ms)", pass.line());
    for (g, ok) in &list {
        println!("  {} {g}", if *ok { "pass" } else { "FAIL" });
    }
    println!("  field {} / with part {} (thinnest wall {:.2} mm); two-part undercut {:.3}% band, {:.3}% with the part", band_field.verdict.label(), field.verdict.label(), band_field.thinnest_wall_mm, band_field.undercut_fraction() * 100.0, field.undercut_fraction() * 100.0);
    println!("  walls: assessed {}, {} samples, {} unresolved, {} wall ({:.3} mm2), {} edge ({:.3} mm2), thinnest {:?} ({census_ms:.0} ms)", walls.assessed, walls.rays, walls.unresolved, walls.below_limit, walls.wall_area_mm2, walls.edge_below_limit, walls.edge_area_mm2, walls.sampled_min_mm);
    for z in walls.walls.iter().take(8) {
        println!("    wall {:.3} mm2 thinnest {:.3} at [{:.2}, {:.2}, {:.2}] span {:.2}", z.area_mm2, z.thinnest_mm, z.point[0], z.point[1], z.point[2], z.span_mm);
    }
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    for n in &field.notes {
        println!("  field: {n}");
    }
    let json = json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": pass.triangles, "ms": build_ms },
        "geometry": pass.json(),
        "finger_hole": { "bore_radius_mm": bore, "vertices_inside": inside, "nearest_margin_mm": nearest },
        "field": { "verdict": band_field.verdict.label(), "with_part_verdict": field.verdict.label(), "process": format!("{:?}", field.process), "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": field.notes },
        "two_part_undercut": { "band_percent": band_field.undercut_fraction() * 100.0, "with_part_percent": field.undercut_fraction() * 100.0, "part_undercut_mm2": field.parts.iter().map(|p| p.undercut_area_mm2).sum::<f64>(), "note": "reported only: lost wax judges fill and detail, never the pull" },
        "dfm_findings": dfm.iter().map(|f| json!({ "label": f.label, "message": f.message })).collect::<Vec<_>>(),
        "stones": { "reported": stones, "previewed": preview },
        "casting_pattern": { "watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len() },
        "wall_census": { "call": "cad::measure::thickness(&built.mesh, 0.8) on the finished ring's own mesh, default pitch and edge reach", "gate": "the lead's interim rule of 2026-10-03: every wall zone whose thinnest reading is 0.05-0.8 mm fails; thinner zones are suspected census artifacts, listed, not reshaped", "passed": walls_ok, "strict_clean": walls.clean(), "real_wall_zones": real_walls.len(), "suspected_census_artifacts": artifacts, "ms": census_ms, "census": walls },
        "gates": list.iter().map(|(g, ok)| json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "gates_passed": passed,
    });
    Ok(Gated { json, passed, built, pattern })
}

// --- Renders ---------------------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger axis, pitch from it.
const HERO: (f64, f64) = (-0.3, 1.12);
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", HERO.0, HERO.1),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, PI * 0.5),
    ("side", 0.0, 0.0),
    ("shoulder", 1.2, 1.0),
    ("reverse", PI - 0.5, 0.6),
];

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, p: BuildParams, draft: bool, face_y: f64) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    let edge = if draft { 1100 } else { 1600 };
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("hero-300.png"), &parts, HERO.0, HERO.1, 300)?;
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    let mut sheet = vec![0u8; 900 * 600 * 3];
    for (k, (_, yaw, pitch)) in VIEWS.iter().enumerate() {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3);
        paste(&mut sheet, 900, &img, 300, (k % 3) * 300, (k / 3) * 300);
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 900, 600, image::ColorType::Rgb8)?;
    // No stone: the close-up is the head and forelimbs, framed on the whole ring, never a cropped mesh.
    let centre = [EYE_X - 0.5, face_y + 0.6, 0.0];
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(80.0), 1.2, render::Framing::new(centre, 6.5), edge)?;
    let bare = mesh::try_build(&band(), lib, p)?;
    let e = if draft { 700 } else { 1000 };
    let left = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], HERO.0, HERO.1, e, e, 3);
    let right = render::render_parts_ss(&parts, HERO.0, HERO.1, e, e, 3);
    let mut pair = vec![0u8; e * 2 * e * 3];
    paste(&mut pair, e * 2, &left, e, 0, 0);
    paste(&mut pair, e * 2, &right, e, e, 0);
    image::save_buffer(out.join("bare-vs-finished.png"), &pair, (e * 2) as u32, e as u32, image::ColorType::Rgb8)?;
    if std::env::var_os("GEKKO_VIEWS").is_some() {
        let mut sheet = vec![0u8; 1200 * 900 * 3];
        for (k, (yaw, pitch)) in [(-0.9, 0.7), (-0.45, 0.95), (0.0, 0.95), (0.45, 0.95), (0.9, 0.7), (1.4, 0.8), (-1.4, 0.8), (0.3, 1.2), (-0.3, 1.2), (0.0, 0.3), (PI, 0.9), (PI / 2.0, 0.3)].iter().enumerate() {
            let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 2);
            paste(&mut sheet, 1200, &img, 300, (k % 4) * 300, (k / 4) * 300);
        }
        image::save_buffer(out.join("views-probe.png"), &sheet, 1200, 900, image::ColorType::Rgb8)?;
    }
    Ok(())
}

// --- Writing ---------------------------------------------------------------------------------------------------------

fn write(out: &Path, draft: bool, verify: bool, block_out: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let started = Instant::now();
    let Authored { mut d, lib, comp, solid } = author()?;
    let comp_face_y = comp.face_y_mm;
    let p = params(draft);
    d.build = p;
    let mut blocks = serde_json::Map::new();
    let main = gates(&d, &lib, p, if draft { "draft 768 x 320" } else { "export 1536 x 448" })?;
    let mut passed = main.passed;
    if !draft {
        let dr = gates(&d, &lib, params(true), "draft 768 x 320")?;
        passed &= dr.passed;
        blocks.insert("draft".into(), dr.json);
        blocks.insert("export".into(), main.json.clone());
    } else {
        blocks.insert("draft".into(), main.json.clone());
    }
    if std::env::var_os("GEKKO_COARSE").is_some() || !draft {
        let co = Pass::of(&mesh::try_build(&d, &lib, coarse_params())?);
        println!("  384x192: {}", co.line());
        passed &= co.ok();
        blocks.insert("coarse_384x192".into(), co.json());
    }
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let mut reload = Value::Null;
    if verify {
        let t = Instant::now();
        let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
        let cold = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold, p)?;
        let same = rebuilt.mesh.vertices == main.built.mesh.vertices && rebuilt.mesh.faces == main.built.mesh.faces && rebuilt.mesh.normals == main.built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical vertices, faces and normals" } else { "CHANGED" });
        reload = json!({ "identical": same, "vertices": rebuilt.mesh.vertices.len(), "faces": rebuilt.mesh.faces.len(), "ms": ms(t) });
        passed &= same;
    }
    let (open_edges, _) = sculpt::closure(&solid);
    let sculpt_crossings = csg::self_crossings(&solid);
    let bytes = std::fs::metadata(out.join("design.ring.json")).map_or(0, |m| m.len());
    let report = json!({
        "ring": d.name,
        "slug": SLUG,
        "stage": if block_out { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "gates_that_apply": "Lost wax (Logan's rule of 2026-10-03): geometry, made parts uncrossed, notes and stamps, bore, the lost-wax field verdict at the 0.8 mm fill, the wall census of the finished ring (cad::measure::thickness at 0.8 mm, judged by the lead's interim rule: real walls of 0.05-0.8 mm fail, thinner zones listed as suspected artifacts), DFM, stones, 2 M triangles, casting pattern, 384 x 192, cold reload, template gate. The sand gates do not apply and are recorded as such below.",
        "ray_release": { "applies": false, "why": "lost wax has no two-part pull; the two-part undercut is reported as a number in each build block (two_part_undercut)" },
        "draft_clamp": { "applies": false, "clamped_layers": 0, "why": "no field layer and no clamp: the face's ground and the figure are one sculpted part, the shank's hide is stamps" },
        "build_note": "The factory 017 stock is built from its stored surface: its triangle count does not change with the build's theta and profile steps, so the draft, export and 384 x 192 builds report the same count; each is built at its own steps and gated separately.",
        "process_decision": "Lost wax by Logan's rule of 2026-10-03 (recorded in the Gekko section of docs/collections/cataphracta.md): 0.8 mm minimum section, no pull rule. The two-part sand undercut is reported as a number only.",
        "draft_rules": { "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm },
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "sculpt": { "open_edges": open_edges, "self_crossings": sculpt_crossings, "composition": comp },
        "design_bytes": bytes,
        "template_class": { "class": "painted", "budget_bytes": 3_000_000, "why": "declared in the collection manifest: the default for an imported base, stock (1 MB), budgets the factory surface alone; this ring also carries the tokay and the pebbled face as one stored sculpt plus its eyes, about 10 bytes a face, which weighs what a painted atlas does. The template gate itself runs after the last round (template-verification.json)." },
        "draft": blocks.get("draft"),
        "export": blocks.get("export"),
        "coarse_384x192": blocks.get("coarse_384x192"),
        "cold_reload": reload,
        "gates_passed": passed,
        "author_s": started.elapsed().as_secs_f64(),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    stl::write_stl(out.join("finished-metal.stl"), &main.built.mesh, &d.name)?;
    stl::write_stl(out.join("casting-pattern.stl"), &main.pattern.mesh, &format!("{} / lost-wax pattern", d.name))?;
    std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": [], "note": "Gekko carries no stone." }))?)?;
    renders(out, &lib, &main.built, p, draft, comp_face_y)?;
    println!("gates {}", if passed { "all pass" } else { "FAILED" });
    ensure!(passed || std::env::var_os("GEKKO_LOOSE").is_some(), "gates failed");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("showcase/cataphracta").join(SLUG));
    write(&out, flag("--draft"), flag("--verify"), flag("--block-out"))
}

/// The slit pupil's half-width and depth, mm.
fn groove_dims() -> (f64, f64) {
    let get = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    (get("GEKKO_GW", 0.145), get("GEKKO_GD", 0.13))
}
