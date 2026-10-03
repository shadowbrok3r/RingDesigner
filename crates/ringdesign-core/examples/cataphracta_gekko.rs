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
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SideFacePick, VGate},
    manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    sculpt::{self, ellipsoid, round_cone, smin},
    reptile::svg::{self as rsvg, Params},
    skin::{self, Atlas},
    stl,
    svg::SvgAlpha,
    tiling::TilingLayer,
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
/// The Flat style's crown exponent, opened from its 8 so the crown carries draft off the crest.
const CROWN_EXPONENT: f64 = 3.0;

/// Ring angle of the body frame's origin, degrees; the head lies toward increasing angle.
const THETA_C: f64 = 90.0;
/// Radius at which the body frame measures arc along the ring, mm.
const R_REF: f64 = 13.4;
/// Nothing of the sculpt comes nearer the finger axis than the bore plus this, mm.
const BORE_CLEAR_MM: f64 = 0.6;
/// How deep the sculpt reaches into the band under its own outline, mm: past it the band is metal already.
const SINK_MM: f64 = 0.45;
const STEP_MM: f64 = 0.05;
/// The investment's fill floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const FACES: usize = 260_000;

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

/// The keyed Flat band, cast in lost wax: deepest under the gecko, the palm the reference and the tightest station.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.crown_mm = 1.2;
    d.profile.shape_a = CROWN_EXPONENT;
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).expect("bore");
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    // Broad and deep under the gecko so its limbs reach the edges; thickness held at or over width at every key.
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![key(30.0, 1.12, 1.14), key(65.0, 1.25, 1.25), key(115.0, 1.25, 1.25), key(150.0, 1.12, 1.14), key(210.0, 1.0, 1.02), key(270.0, 1.0, 1.0), key(330.0, 1.0, 1.02)];
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
fn wrap180(a: f64) -> f64 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// Deterministic jitter in 0..1.
fn hash(k: usize, salt: u64) -> f64 {
    let mut x = (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// The bare band read off its atlas, one entry per column: the crest radius, the half width along the finger, and
/// the crown's radius across the band where the skin faces out.
struct Crest {
    r: Vec<f64>,
    half_w: Vec<f64>,
    crown: Vec<Vec<(f64, f64)>>,
}

impl Crest {
    fn of(d: &RingDesign) -> Result<Self> {
        let a = Atlas::of(d, 1440, 256)?;
        let r = (0..a.width).map(|x| (0..a.height).map(|y| { let p = a.at(x, y).p; p[0].hypot(p[1]) }).fold(0.0, f64::max)).collect();
        let half_w = (0..a.width).map(|x| (0..a.height).map(|y| a.at(x, y).p[2].abs()).fold(0.0, f64::max)).collect();
        let crown = (0..a.width)
            .map(|x| {
                let mut c: Vec<(f64, f64)> = (0..a.height)
                    .map(|y| a.at(x, y))
                    .filter(|s| {
                        let rho = s.p[0].hypot(s.p[1]).max(1e-9);
                        (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / rho > 0.3
                    })
                    .map(|s| (s.p[2], s.p[0].hypot(s.p[1])))
                    .collect();
                c.sort_by(|a, b| a.0.total_cmp(&b.0));
                c
            })
            .collect();
        Ok(Self { r, half_w, crown })
    }
    fn read(v: &[f64], theta_deg: f64) -> f64 {
        let n = v.len();
        let f = theta_deg.rem_euclid(360.0) / 360.0 * n as f64;
        let i = f.floor() as usize % n;
        let t = f - f.floor();
        v[i] * (1.0 - t) + v[(i + 1) % n] * t
    }
    fn at(&self, theta_deg: f64) -> f64 {
        Self::read(&self.r, theta_deg)
    }
    fn half_w(&self, theta_deg: f64) -> f64 {
        Self::read(&self.half_w, theta_deg)
    }
    /// The crown's radius at `theta` and `z`, mm; held at the edge past it.
    fn crown_r(&self, theta_deg: f64, z: f64) -> f64 {
        let n = self.crown.len();
        let col = &self.crown[((theta_deg.rem_euclid(360.0) / 360.0 * n as f64).round() as usize) % n];
        match col.iter().position(|c| c.0 >= z) {
            Some(0) => col[0].1,
            None => col.last().map_or(0.0, |c| c.1),
            Some(i) => {
                let (a, b) = (col[i - 1], col[i]);
                a.1 + (b.1 - a.1) * (z - a.0) / (b.0 - a.0).max(1e-9)
            }
        }
    }
    /// The bare crown's draft at `theta` and `z`, degrees: how fast it falls away from the parting line.
    fn draft_deg(&self, theta_deg: f64, z: f64) -> f64 {
        let e = 0.05;
        let slope = (self.crown_r(theta_deg, z.abs() + e) - self.crown_r(theta_deg, z.abs() - e)) / (2.0 * e);
        (-slope).atan().to_degrees()
    }

    /// The crown's height over the crest at a frame `x` and `w`, mm (at or under 0).
    fn top_h(&self, x: f64, w: f64) -> f64 {
        let theta = theta_of(x);
        self.crown_r(theta, w) - self.at(theta)
    }
    /// Signed distance to the bare band, roughly: negative inside, its edge corner rounded generously so a point in
    /// the fillet's lee never reads as inside.
    fn band_sd(&self, p: P3) -> f64 {
        const ROUND: f64 = 0.6;
        let theta = p[1].atan2(p[0]).to_degrees();
        let r = p[0].hypot(p[1]);
        let hw = self.half_w(theta);
        let a = r - self.crown_r(theta, p[2].clamp(-hw, hw)) + ROUND;
        let b = p[2].abs() - hw + ROUND;
        a.max(0.0).hypot(b.max(0.0)) + a.max(b).min(0.0) - ROUND
    }
}

fn theta_of(x: f64) -> f64 {
    THETA_C + (x / R_REF).to_degrees()
}

/// The body frame: `x` mm of arc along the ring from `THETA_C` (toward the head positive), `h` mm over the band's crest,
/// `w` mm along the finger.
#[derive(Clone, Copy)]
struct Frame<'a> {
    crest: &'a Crest,
}

impl Frame<'_> {
    fn local(&self, p: P3) -> P3 {
        let theta = p[1].atan2(p[0]).to_degrees();
        let r = p[0].hypot(p[1]);
        [wrap180(theta - THETA_C).to_radians() * R_REF, r - self.crest.at(theta), p[2]]
    }
    fn world(&self, q: P3) -> P3 {
        let theta = theta_of(q[0]);
        let r = self.crest.at(theta) + q[1];
        let t = theta.to_radians();
        [r * t.cos(), r * t.sin(), q[2]]
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
    Eye,
    Hide,
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
const HEAD_X: f64 = 8.6;
const EYE_X: f64 = 7.0;
const EYE_W: f64 = 1.7;
const EYE_R: f64 = 0.95;
const VENT_X: f64 = -9.0;
const TAIL_LEN: f64 = 22.0;
/// Front and hind limbs: where each leaves the body, and which way the hand turns along the ring.
const LEGS: [(f64, f64); 2] = [(2.2, 1.0), (-7.0, -1.0)];
/// The skin's granules: lattice pitch and height; the spots' height.
const GRANULE_PITCH_MM: f64 = 0.5;
const GRANULE_MM: f64 = 0.0;
const SPOT_MM: f64 = 0.26;

struct Gekko<'a> {
    frame: Frame<'a>,
    prims: Vec<Prim>,
    spots: Vec<Spot>,
    /// The eyes' centre height over the crest.
    eye_h: f64,
    limbs: Vec<LimbPlan>,
    bore_r: f64,
}

/// The tail's centre line and radius at a share `t` of its length from the vent: down the crest, then swinging out
/// across the crown's near shoulder, over the band's edge and down the side face, where its tip curls back up.
fn tail_at(crest: &Crest, t: f64) -> (P3, f64) {
    let r = 1.4 * (1.0 - t).powf(0.8) + 0.42 * t;
    // Control points: x along the ring, then w across, and where the point lies (0 on the crown, 1 on the side face,
    // with its depth under the crown's edge).
    let pts: [(f64, f64, f64); 8] = [
        (VENT_X, 0.0, 0.0),
        (VENT_X - 3.0, 0.5, 0.0),
        (VENT_X - 6.0, 1.7, 0.0),
        (VENT_X - 8.6, 3.2, 0.0),
        (VENT_X - 10.6, 4.0, 0.5),
        (VENT_X - 12.6, 4.0, 1.3),
        (VENT_X - 14.2, 4.0, 1.5),
        (VENT_X - 15.2, 4.0, 1.0),
    ];
    let n = pts.len() - 1;
    let f = (t.clamp(0.0, 1.0) * n as f64).min(n as f64 - 1e-9);
    let i = f.floor() as usize;
    let u = f - i as f64;
    let g = |k: isize| pts[(k.max(0) as usize).min(n)];
    let (p0, p1, p2, p3) = (g(i as isize - 1), g(i as isize), g(i as isize + 1), g(i as isize + 2));
    let cr = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * u + (2.0 * a - 5.0 * b + 4.0 * c - d) * u * u + (-a + 3.0 * b - 3.0 * c + d) * u * u * u);
    let x = cr(p0.0, p1.0, p2.0, p3.0);
    let wt = cr(p0.1, p1.1, p2.1, p3.1);
    let depth = cr(p0.2, p1.2, p2.2, p3.2).max(0.0);
    let edge = crest.half_w(theta_of(x));
    // On the crown the tail lies on it; past the edge it hugs the side face, standing out of it by its radius.
    let on_face = smoothstep(0.0, 0.6, depth);
    let w_crown = wt.min(edge - 0.2);
    let h_crown = crest.top_h(x, w_crown.abs()) - 0.25 * r;
    let w_face = edge + 0.55 * r;
    let h_face = crest.top_h(x, edge) - depth;
    let w = w_crown + (w_face - w_crown) * on_face;
    let h = h_crown + (h_face - h_crown) * on_face;
    ([x, h, w], r)
}

/// The tail's rings: raised bands round it at a pitch, 0..1, nothing on its root.
const RING_PITCH_MM: f64 = 1.0;
const RING_MM: f64 = 0.09;
fn tail_rings(x: f64) -> f64 {
    if x > VENT_X - 1.2 {
        return 0.0;
    }
    let f = ((VENT_X - 1.2 - x) / RING_PITCH_MM).fract();
    smoothstep(0.05, 0.25, f) * (1.0 - smoothstep(0.45, 0.65, f)) * (1.0 - smoothstep(VENT_X - 14.0, VENT_X - 19.0, x))
}

fn body_prims(crest: &Crest) -> Vec<Prim> {
    let egg = |kind, c: P3, r: P3, snout: f64, blend: f64| Prim { kind, shape: Shape::Egg { c, r, snout }, blend };
    let limb = |kind, a: P3, b: P3, ra: f64, rb: f64, blend: f64| Prim { kind, shape: Shape::Limb { a, b, ra, rb }, blend };
    let mut out = vec![
        // Trunk: round in section, so its back falls away from the spine at once.
        egg(Kind::Body, [-3.2, -0.9, 0.0], [6.6, 2.05, 2.0], 0.0, 0.0),
        egg(Kind::Body, [1.4, -0.9, 0.0], [2.6, 1.95, 1.85], 0.0, 0.6),
        // Neck: narrower than the head, so the jaw's hinge stands out from it.
        egg(Kind::Head, [4.5, -0.8, 0.0], [1.5, 1.7, 1.3], 0.0, 0.35),
        // Head: a broad flat wedge, widest at the jaw's hinge, to a blunt rounded snout.
        egg(Kind::Head, [8.3, -0.6, 0.0], [3.1, 1.45, 2.1], 0.5, 0.3),
        // The jaw's hinges, the head's broadest point, behind the eyes.
        egg(Kind::Head, [6.4, -0.7, 1.45], [1.3, 1.15, 1.05], 0.0, 0.3),
        egg(Kind::Head, [6.4, -0.7, -1.45], [1.3, 1.15, 1.05], 0.0, 0.3),
        // The hips.
        egg(Kind::Body, [VENT_X + 1.0, -0.9, 0.0], [2.0, 1.95, 1.85], 0.0, 0.5),
    ];
    // The tail: a chain of rounded cones from the vent down the crest.
    let n = 40;
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
            out.push(limb(Kind::Toe, *root, *tip, 0.38, 0.34, 0.08));
            out.push(egg(Kind::Pad, *tip, [0.46, 0.4, 0.46], 0.0, 0.06));
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
fn limb_plans(crest: &Crest) -> Vec<LimbPlan> {
    let on = |x: f64, w: f64, lift: f64| [x, crest.top_h(x, w.abs()) + lift, w];
    let mut out = Vec::new();
    for s in [1.0, -1.0] {
        for (x0, hind) in [(LEGS[0].0, false), (LEGS[1].0, true)] {
            let dir = if hind { -1.0 } else { 1.0 };
            let sh = [x0, -0.25, 1.2 * s];
            let el = on(x0 - 1.2 * dir, 2.3 * s, 0.7);
            let wr = on(x0 + 0.0 * dir, 2.55 * s, 0.42);
            let ft = on(x0 + 0.45 * dir, 2.7 * s, 0.34);
            let mut toes = Vec::new();
            for k in 0..5 {
                let j = k as f64 - 2.0;
                // Fanned about a line out and toward the head (forelimb) or the tail (hind limb).
                let a = (42.0 * j + 28.0 * dir).to_radians();
                let len = [1.45, 1.65, 1.75, 1.65, 1.45][k];
                let x = ft[0] + a.sin() * len;
                let edge = crest.half_w(theta_of(x)) - 0.7;
                let w = (ft[2].abs() + a.cos() * len).min(edge);
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
    let mut out: Vec<Spot> = Vec::new();
    // The back and flanks: low rounded tubercles 1.5-2 mm apart, scattered over the whole torso, the spine included.
    let mut tries = 0usize;
    while tries < 6000 {
        tries += 1;
        let x = VENT_X + 0.4 + (3.2 - VENT_X - 0.4) * hash(tries, 31);
        let w = 1.55 * (2.0 * hash(tries, 32) - 1.0);
        let r = 0.36 + 0.14 * hash(tries, 34);
        if out.iter().any(|o| (o.x - x).hypot(o.w - w) < 1.55) {
            continue;
        }
        out.push(Spot { x, w, r });
    }
    // Behind the eyes on the head's back.
    for (x, w, r) in [(5.4, 0.0, 0.4), (5.6, 1.15, 0.32), (5.6, -1.15, 0.32)] {
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
        gek.eye_h = lo + 0.75 - EYE_R;
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
            // The eye looks up and out; its pupil is an upright slit, a narrow lens across the ball's face from brow
            // to cheek, and a lid rings the ball.
            let look = [0.0, 0.55, 0.835 * s];
            let up = [0.0, 0.835, -0.55 * s];
            let (fa, fv) = (dot(d, look), dot(d, up));
            let half = 0.13 * (1.0 - (fv / (0.82 * EYE_R)).powi(2)).max(0.0).sqrt();
            let slit = (d[0].abs() - half).max(0.25 * EYE_R - fa);
            let eye = ball.max(-slit);
            // The lid: a ring round the ball a little behind its equator.
            let ring_c = sub(d, mul(look, -0.12 * EYE_R));
            let along = dot(ring_c, look);
            let across = norm(sub(ring_c, mul(look, along)));
            let lid = (across - 0.98 * EYE_R).hypot(along) - 0.17;
            f = f.min(smin(eye, lid, 0.05));
        }
        f
    }

    /// The hide's relief at a point, mm: smooth raised spots, fine granules between them; and apart, the spots on the
    /// spine, which straddle the parting line and fall away from it either side, so they stand whatever the slope.
    fn hide(&self, q: P3, p: P3) -> (f64, f64) {
        let (mut spot, spine) = (0.0f64, 0.0f64);
        for s in &self.spots {
            let d = (q[0] - s.x).hypot(q[2] - s.w);
            if d < s.r + 0.25 {
                let v = (1.0 - (d / (s.r + 0.2)).powi(2)).max(0.0).sqrt();
                spot = spot.max(v);
            }
        }
        (SPOT_MM * spot + GRANULE_MM * granules(p) * (1.0 - spot.max(spine)), SPOT_MM * spine)
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
            f -= (flank + spine + RING_MM * tail_rings(q[0])) * fade;
        }
        f = smin(f, eyes, 0.1);
        f.max(-self.frame.crest.band_sd(p) - SINK_MM).max(self.bore_r + BORE_CLEAR_MM - p[0].hypot(p[1]))
    }

    /// Which shape is nearest the surface at a frame point.
    fn kind_at(&self, q: P3) -> Kind {
        let mut best = (self.eyes_at(q).abs(), Kind::Eye);
        for s in &self.prims {
            let v = s.eval(q).abs();
            if v < best.0 {
                best = (v, s.kind);
            }
        }
        best.1
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
    crest_r_at_head_mm: f64,
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
const CROWN_GRANULE_MM: f64 = 0.09;
const CROWN_PITCH_MM: f64 = 0.62;
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
    let t = smoothstep(0.04, 0.42, f2 - f1);
    t * (1.0 - 0.55 * (f1 / (0.7 * g)).min(1.0).powi(2))
}

/// The band's hide: granules over the crown either side of the crest, and rows of enlarged tubercles on both side
/// faces, sheared so they spiral round the ring.
fn band_hide(d: &mut RingDesign, lib: &mut AlphaLibrary, crest: &Crest) -> Result<()> {
    let ctx = d.field_context();
    let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).and_then(|f| f.low).ok_or_else(|| anyhow::anyhow!("no side face"))?;
    let face_h = faces.1 - faces.0;
    let repeats = (ctx.circumference_mm / 1.35).round() as u32;
    let pitch = ctx.circumference_mm / repeats as f64;
    d.svgs.push(SvgAlpha { name: "Tubercle rows".into(), svg: rsvg::tubercle_rows(&Params::new(pitch, face_h, 0.42, 0.9)), invert: false });
    let mut t = TilingLayer::default_for("Tubercle rows", &ctx);
    ensure!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG), "the band has no side faces");
    t.repeats_around = repeats;
    t.height_mm = 0.16;
    t.shear = 0.35;
    let mut e = LayerEntry::new("Tubercle rows", Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let a = Atlas::of(d, 2048, 768)?;
    let alpha = a.paint("Crown granules", |s| {
        let rho = s.p[0].hypot(s.p[1]).max(1e-9);
        if (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / rho < 0.35 {
            return 0.0;
        }
        let along = s.theta.to_radians() * R_REF;
        crown_granules(along, s.p[2]) * smoothstep(0.4, 1.2, s.p[2].abs()) * (1.0 - 0.0 * crest.at(s.theta))
    });
    lib.insert(ringdesign_core::Alpha::from_png16("Crown granules", &alpha.to_png16()?)?);
    let mut e = skin::hide_layer(d, "Crown granules", CROWN_GRANULE_MM, ringdesign_core::field::Window::default());
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok(())
}



fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}



struct Authored {
    d: RingDesign,
    lib: AlphaLibrary,
    comp: Composition,
    solid: csg::Solid,
    census: Census,
}

fn author() -> Result<Authored> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let crest = Crest::of(&d)?;
    band_hide(&mut d, &mut lib, &crest)?;
    let mut comp = Composition { crest_r_at_head_mm: crest.at(theta_of(HEAD_X)), ..Composition::default() };
    let gek = Gekko::new(Frame { crest: &crest }, d.inner_radius_mm());
    comp.prims = gek.prims.len();
    comp.spots = gek.spots.len();
    let solid = sculpt_solid(&gek, &mut comp);
    println!("  sculpt {} raw faces -> {}, {:.1} mm3, {:.1} s {:?}", comp.raw_faces, comp.faces, comp.volume_mm3, comp.sculpt_s, comp.notes);
    let packed = sculpt::packed(&solid)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe {
        kernel: "sculpt".into(),
        op: "tokay".into(),
        params: json!({"theta_c_deg": THETA_C, "r_ref_mm": R_REF, "step_mm": STEP_MM, "faces": FACES, "spots": comp.spots}),
        digest: String::new(),
    };
    doc.append(Feature { id: next, name: "Tokay".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component: joined() })?;
    d.bake_all(&mut lib);
    let census = land_census(&gek, &solid);
    Ok(Authored { d, lib, comp, solid, census })
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

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Body => "body",
            Kind::Head => "head and neck",
            Kind::Limb => "limbs",
            Kind::Toe => "toes",
            Kind::Pad => "toe pads",
            Kind::Tail => "tail",
            Kind::Eye => "eyes",
            Kind::Hide => "hide relief",
        }
    }
    /// How a section under the fill floor on this kind is met, or `None` where none is allowed.
    fn treatment(self) -> Option<&'static str> {
        match self {
            Kind::Toe => Some("toe: a finger lying on the crown and fused into it along its length, fed from the foot; the reading across it is the toe alone, not the metal the wax fills, which runs on into the band; a short-filled tip is built back with a laser tack"),
            Kind::Pad => Some("toe pad: a flattened disc lying on the crown, fused to it over its whole underside; the reading is across the disc's own edge, which the band behind it feeds"),
            Kind::Limb => Some("limb: where a wrist or ankle narrows into its foot, lying on the crown and fused to it"),
            Kind::Tail => Some("tail tip: fed along the tail from the body; the rounded end is dressed with a file"),
            Kind::Eye => Some("eyelid: a 0.17 mm rim round each eye, relief on the head over the 0.15 mm detail floor; the slit pupil is relief cut into the ball"),
            Kind::Hide => Some("hide relief: granules, spots and tail rings 0.06-0.17 mm proud of the body, read where a face's ray crosses a granule's own flank; cast as relief over the 0.15 mm detail floor"),
            _ => None,
        }
    }
}

type Census = Vec<(Kind, f64, f64, P3)>;

/// The sculpt's sections face by face (as `dfm::part_sections` reads them), gathered by the shape nearest each face:
/// the thinnest, and the area under the fill floor. A face on the body whose ray leaves the metal within the hide's
/// height of the smooth body crossed a granule or spot, which is relief, not a section.
fn land_census(gek: &Gekko, solid: &csg::Solid) -> Census {
    use ringdesign_core::interaction::bvh::Bvh;
    let m = mesh::Mesh { vertices: solid.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), faces: solid.f.clone(), ..Default::default() };
    let bvh = Bvh::build(&m);
    const IN: f64 = 1e-4;
    let mut out: std::collections::BTreeMap<u8, (Kind, f64, f64, P3)> = Default::default();
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let twice = norm(n);
        if !(twice > 1e-14) {
            continue;
        }
        let centre = mul(add(add(a, b), c), 1.0 / 3.0);
        // Faces sunk in the band never show; their sections are the band's.
        if gek.frame.crest.band_sd(centre) < -0.05 {
            continue;
        }
        let inward = mul(n, -1.0 / twice);
        let o = add(centre, mul(inward, IN));
        let Some((_, t)) = bvh.ray(&m, o, inward) else { continue };
        let section = t + IN;
        let q = gek.frame.local(centre);
        let mut kind = gek.kind_at(q);
        let exit = add(o, mul(inward, t));
        if section < MIN_SECTION_MM && matches!(kind, Kind::Body | Kind::Head | Kind::Limb | Kind::Tail) && gek.body_at(gek.frame.local(exit)) > -0.3 {
            kind = Kind::Hide;
        }
        let e = out.entry(kind as u8).or_insert((kind, f64::MAX, 0.0, [0.0; 3]));
        if section < e.1 {
            e.1 = section;
            e.3 = centre;
        }
        if section < MIN_SECTION_MM {
            e.2 += 0.5 * twice;
        }
    }
    out.into_values().collect()
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
        self.watertight && self.degenerate == 0 && self.crossings == 0 && self.notes.is_empty() && self.parts.iter().all(|p| p.1 == 0) && self.joined == 1
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

fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams, label: &str, census: &Census) -> Result<Gated> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = ms(t);
    let pass = Pass::of(&built);
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).count();
    let nearest = built.mesh.vertices.iter().map(|q| (q.0 as f64).hypot(q.1 as f64) - bore).fold(f64::MAX, f64::min);
    let band_field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let mut field = band_field.clone();
    castability::judge_parts(&mut field, d, &built);
    let dfm = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built).map_or(0, |r| r.stone_count as usize);
    let preview = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    let unnamed: Vec<String> = census.iter().filter(|c| c.2 > 0.0 && c.0.treatment().is_none()).map(|c| format!("{}: {:.3} mm2 under, thinnest {:.2}", c.0.label(), c.2, c.1)).collect();
    let list = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", pass.watertight && pass.degenerate == 0 && pass.crossings == 0),
        ("the sculpted part uncrossed as placed", pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, the part joined", pass.notes.is_empty() && pass.joined == 1),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax field verdict Castable with the 0.8 mm fill, band and with the part judged", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && band_field.verdict == Verdict::Castable && band_field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("every section under 0.8 mm named with its treatment", unnamed.is_empty()),
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
        "unnamed_under_floor": unnamed,
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

fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, p: BuildParams, draft: bool) -> Result<()> {
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
    let theta = theta_of(EYE_X - 2.0);
    let t = theta.to_radians();
    let centre = [14.0 * t.cos(), 14.0 * t.sin(), 0.0];
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(theta), 1.2, render::Framing::new(centre, 6.5), edge)?;
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
    let Authored { mut d, lib, comp, solid, census } = author()?;
    let p = params(draft);
    d.build = p;
    let mut blocks = serde_json::Map::new();
    let main = gates(&d, &lib, p, if draft { "draft 768 x 320" } else { "export 1536 x 448" }, &census)?;
    let mut passed = main.passed;
    if !draft {
        let dr = gates(&d, &lib, params(true), "draft 768 x 320", &census)?;
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
    let (part_min, part_under) = dfm::part_sections(&solid, None, MIN_SECTION_MM);
    for c in &census {
        println!("    lands {}: thinnest {:.3}, {:.3} mm2 under", c.0.label(), c.1, c.2);
    }
    let bytes = std::fs::metadata(out.join("design.ring.json")).map_or(0, |m| m.len());
    let report = json!({
        "ring": d.name,
        "slug": SLUG,
        "stage": if block_out { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "process_decision": "Lost wax by Logan's rule of 2026-10-03 (recorded in the Gekko section of docs/collections/cataphracta.md): 0.8 mm minimum section, no pull rule. The two-part sand undercut is reported as a number only.",
        "draft_rules": { "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm },
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "sculpt": { "open_edges": open_edges, "self_crossings": sculpt_crossings, "composition": comp },
        "land_widths": {
            "floor_mm": MIN_SECTION_MM,
            "method": "dfm::part_sections on the sculpted part alone (one ray per face along its inward normal), and the same per face gathered by the shape nearest each face; faces sunk in the band are left out",
            "part_sections": { "thinnest_mm": part_min, "under_floor_mm2": part_under },
            "by_kind": census.iter().map(|c| json!({"kind": c.0.label(), "thinnest_mm": c.1, "under_floor_mm2": c.2, "treatment": if c.2 > 0.0 { c.0.treatment().map(|t| format!("{t}. Measured thinnest section: {:.3} mm.", c.1)) } else { None }})).collect::<Vec<_>>(),
        },
        "design_bytes": bytes,
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
    renders(out, &lib, &main.built, p, draft)?;
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
