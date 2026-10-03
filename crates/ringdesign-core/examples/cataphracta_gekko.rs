//! Cataphracta — Gekko, the tokay: a tokay gecko spread along the crown, poured in Delft sand.
//! cargo build --release -p ringdesign-core --example cataphracta_gekko
//! target/release/examples/cataphracta_gekko [OUT_DIR] [--draft] [--verify] [--block-out]
//!
//! The animal is one sculpted part lying with its spine on the parting line: a broad flat head with two great eyes,
//! a slender spotted body, four limbs reaching to the band's edges where the toes splay and end in broad pads, and a
//! banded tail running down the crest toward the palm. The sculpt is a distance field made pullable before it is
//! meshed: every point the mould would have to drag sand past is filled, along the pull, with an 8° lean (the "pull
//! fill"). What is left draws from both halves of a two-part sand mould by construction, and the ray release and the
//! part verdict prove it on the built ring.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, Operation, Placement, Stage, stored},
    castability::{self, CastProcess, SandProcess, Verdict},
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
/// The lean the pull fill gives every wall it makes, degrees; over the Delft clay's 3.0.
const FILL_LEAN_DEG: f64 = 8.0;
/// The fill grid's step and the meshing step, mm.
const FILL_STEP_MM: f64 = 0.05;
const STEP_MM: f64 = 0.05;
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

/// The thickness-keyed Flat band in Delft sand: deepest under the gecko, the palm the reference and the tightest station.
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
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    setup.recipe.name = format!("{} / Delft clay", d.name);
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").unwrap().shrink_pct;
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.flask.width_mm = 80.0;
    setup.flask.length_mm = 80.0;
    let palm = d.inner_radius_mm() + d.profile.thickness_mm - 0.2;
    setup.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -palm, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -33.0, 0.0], diameter_mm: 6.0 },
    ];
    setup.bench_notes = "Procedural Flat band, thickness keyed deepest under the gecko. A tokay lies along the crown with its spine \
        on the parting line, its toes splayed to the band's edges and its tail down the crest to the palm; every wall the pull would \
        drag is filled at 8 deg before meshing. Z=0 parting, opposed Z withdrawal. At the bench: polish the eyes and the spots, \
        leave the granular skin satin."
        .into();
    d.draft.process = setup.recipe.process;
    d.draft.sand = setup.recipe.sand;
    d.draft.min_detail_mm = setup.recipe.min_detail_mm;
    d.draft.min_section_mm = setup.recipe.min_section_mm;
    d.draft.min_draft_deg = setup.recipe.min_draft_deg;
    d.manufacturing = Some(setup);
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
const EYE_X: f64 = 7.2;
const EYE_W: f64 = 2.05;
const EYE_R: f64 = 1.08;
const VENT_X: f64 = -9.0;
const TAIL_LEN: f64 = 22.0;
/// Front and hind limbs: where each leaves the body, and which way the hand turns along the ring.
const LEGS: [(f64, f64); 2] = [(2.2, 1.0), (-7.0, -1.0)];
/// The skin's granules: lattice pitch and height; the spots' height.
const GRANULE_PITCH_MM: f64 = 0.5;
const GRANULE_MM: f64 = 0.055;
const SPOT_MM: f64 = 0.14;
/// The hide is let in only where the surface under it already falls away from the parting line by the fill's lean and
/// this margin, and in full this much past that, degrees: so a granule never asks the pull for more.
const HIDE_MARGIN_DEG: f64 = 2.0;
const HIDE_RAMP_DEG: f64 = 28.0;

struct Gekko<'a> {
    frame: Frame<'a>,
    prims: Vec<Prim>,
    spots: Vec<Spot>,
    /// The eyes' centre height over the crest.
    eye_h: f64,
    limbs: Vec<LimbPlan>,
    bore_r: f64,
    block_out: bool,
    /// The fill grids, once made.
    grid: Option<Grid>,
}

/// The tail's centre line and radius at a share `t` of its length from the vent.
fn tail_at(t: f64) -> (P3, f64) {
    let x = VENT_X - TAIL_LEN * t;
    let r = 1.45 * (1.0 - t).powf(0.85) + 0.55 * t;
    ([x, -0.4 * r, 0.0], r)
}

/// The tail's rings: raised bands round it at a pitch, 0..1, nothing on its root.
const RING_PITCH_MM: f64 = 1.0;
const RING_MM: f64 = 0.13;
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
        egg(Kind::Body, [-3.2, -0.7, 0.0], [6.6, 2.1, 1.85], 0.0, 0.0),
        egg(Kind::Body, [1.6, -0.7, 0.0], [2.7, 2.0, 1.75], 0.0, 0.6),
        // Neck.
        egg(Kind::Head, [4.7, -0.7, 0.0], [1.8, 1.85, 1.7], 0.0, 0.6),
        // Head: broad and flat, a rounded triangle to the snout.
        egg(Kind::Head, [HEAD_X, -0.6, 0.0], [3.6, 1.5, 2.8], 0.62, 0.5),
        // The jaw's angles either side behind the eyes.
        egg(Kind::Head, [6.7, -0.75, 1.75], [1.5, 1.25, 1.2], 0.0, 0.45),
        egg(Kind::Head, [6.7, -0.75, -1.75], [1.5, 1.25, 1.2], 0.0, 0.45),
        // The hips.
        egg(Kind::Body, [VENT_X + 1.0, -0.7, 0.0], [2.0, 1.9, 1.8], 0.0, 0.5),
    ];
    // The tail: a chain of rounded cones from the vent down the crest.
    let n = 16;
    for k in 0..n {
        let (a, ra) = tail_at(k as f64 / n as f64);
        let (b, rb) = tail_at((k + 1) as f64 / n as f64);
        out.push(limb(Kind::Tail, a, b, ra, rb, if k == 0 { 0.5 } else { 0.05 }));
    }
    // Four limbs, each bent at the elbow or knee, ending in a foot planted flat on the crown's shoulder with five toes
    // splayed from it, each tipped with a broad round pad.
    for l in limb_plans(crest) {
        let [sh, el, wr, ft] = l.joints;
        let thick = if l.hind { 1.15 } else { 1.0 };
        out.push(limb(Kind::Limb, sh, el, 0.7 * thick, 0.46, 0.35));
        out.push(egg(Kind::Limb, el, [0.48, 0.44, 0.48], 0.0, 0.12));
        out.push(limb(Kind::Limb, el, wr, 0.44, 0.34, 0.1));
        out.push(limb(Kind::Limb, wr, ft, 0.34, 0.36, 0.08));
        out.push(egg(Kind::Limb, ft, [0.55, 0.26, 0.55], 0.0, 0.12));
        for (root, tip) in &l.toes {
            out.push(limb(Kind::Toe, *root, *tip, 0.22, 0.19, 0.08));
            out.push(egg(Kind::Pad, *tip, [0.52, 0.24, 0.52], 0.0, 0.06));
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
            let el = on(x0 - 0.85 * dir, 2.2 * s, 0.42);
            let wr = on(x0 - 0.05 * dir, 2.75 * s, 0.32);
            let ft = on(x0 + 0.45 * dir, 2.95 * s, 0.26);
            let mut toes = Vec::new();
            for k in 0..5 {
                let j = k as f64 - 2.0;
                // Fanned about a line out and toward the head (forelimb) or the tail (hind limb).
                let a = (32.0 * j + 38.0 * dir).to_radians();
                let len = [0.95, 1.1, 1.15, 1.1, 0.95][k];
                let x = ft[0] + a.sin() * len;
                let edge = crest.half_w(theta_of(x)) - 0.62;
                let w = (ft[2].abs() + a.cos() * len).min(edge);
                let root = on(ft[0] + a.sin() * 0.3, (ft[2].abs() + a.cos() * 0.3) * s, 0.24);
                toes.push((root, on(x, w * s, 0.2)));
            }
            out.push(LimbPlan { hind, joints: [sh, el, wr, ft], toes });
        }
    }
    out
}

/// The tokay's spots: rows down both flanks and on the head's and tail's sides, each a smooth raised disc.
fn spots() -> Vec<Spot> {
    let mut out = Vec::new();
    // Flanks: scattered, of mixed sizes, thinning toward the belly line, never in ranks.
    let mut k = 0usize;
    let mut tries = 0usize;
    while out.len() < 44 && tries < 4000 {
        tries += 1;
        let x = VENT_X + 0.3 + (3.6 - VENT_X - 0.3) * hash(tries, 31);
        let w = (0.65 + 1.15 * hash(tries, 32)) * if hash(tries, 33) < 0.5 { -1.0 } else { 1.0 };
        let r = 0.3 + 0.24 * hash(tries, 34);
        if out.iter().any(|o: &Spot| (o.x - x).hypot(o.w - w) < o.r + r + 0.35) {
            continue;
        }
        out.push(Spot { x, w, r });
        k += 1;
    }
    let _ = k;
    // Down the spine, from the nape to the tail's root.
    for (x, w, r) in [(5.9, 1.25, 0.4), (5.9, -1.25, 0.4), (9.9, 1.0, 0.36), (9.9, -1.0, 0.36)] {
        out.push(Spot { x, w, r });
    }
    for j in 0..12 {
        let t = (j as f64 + 0.5) / 13.0;
        let (c, r) = tail_at(t);
        for s in [1.0, -1.0] {
            out.push(Spot { x: c[0] - 0.3 * (j % 2) as f64, w: 0.66 * r * s, r: (0.32 * r).clamp(0.22, 0.42) });
        }
    }
    out
}

impl<'a> Gekko<'a> {
    fn new(frame: Frame<'a>, bore_r: f64, block_out: bool) -> Self {
        let prims = body_prims(frame.crest);
        let mut gek = Self { prims, spots: spots(), eye_h: 0.0, limbs: limb_plans(frame.crest), frame, bore_r, block_out, grid: None };
        // The eyes sit on the head's top either side, a quarter of each ball sunk into it.
        let (mut lo, mut hi) = (-3.0, 4.0);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if gek.body_at([EYE_X, m, EYE_W]) > 0.0 { hi = m } else { lo = m }
        }
        gek.eye_h = lo - 0.45 * EYE_R;
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
        let (mut spot, mut spine) = (0.0f64, 0.0f64);
        for s in &self.spots {
            let d = (q[0] - s.x).hypot(q[2] - s.w);
            if d < s.r + 0.25 {
                let v = 1.0 - smoothstep(s.r - 0.22, s.r + 0.22, d);
                if s.w == 0.0 {
                    spine = spine.max(v);
                } else {
                    spot = spot.max(v);
                }
            }
        }
        (SPOT_MM * spot + GRANULE_MM * granules(p) * (1.0 - spot.max(spine)), SPOT_MM * spine)
    }

    /// The sculpt as poured, before the fill: the body, held clear of the bore. The eyes are set at the bench.
    fn raw(&self, p: P3) -> f64 {
        let q = self.frame.local(p);
        self.body_at(q).max(self.bore_r + BORE_CLEAR_MM - p[0].hypot(p[1]))
    }

    /// The eyes alone, in world mm, sunk into the head.
    fn eyes_world(&self, p: P3) -> f64 {
        self.eyes_at(self.frame.local(p))
    }

    /// The modelled sculpt with the first fill, then the hide let in where the slope allows.
    fn skinned(&self, p: P3) -> f64 {
        let Some(g) = &self.grid else { return self.raw(p) };
        self.skinned_on(self.raw(p).min(g.at(&g.fill1, p)), p)
    }

    /// The modelled sculpt with its hide and no fill at all: what the finished ring shows where the bench cuts away
    /// what the pull asked for.
    fn modelled(&self, p: P3) -> f64 {
        self.skinned_on(self.raw(p), p)
    }

    fn skinned_on(&self, base: f64, p: P3) -> f64 {
        let Some(g) = &self.grid else { return base };
        if base.abs() > 0.35 {
            return base;
        }
        let k = g.at(&g.budget, p).clamp(0.0, 1.0);
        let q = self.frame.local(p);
        // The tail's rings vary only along the ring, so they stand anywhere round it, the spine included.
        let rings = RING_MM * tail_rings(q[0]);
        if k <= 0.0 && rings <= 0.0 && q[2].abs() > 0.8 {
            return base;
        }
        let toe_near = self.prims.iter().filter(|s| matches!(s.kind, Kind::Toe | Kind::Pad)).map(|s| s.eval(q)).fold(f64::MAX, f64::min);
        let fade = smoothstep(0.05, 0.3, toe_near) * smoothstep(0.05, 0.3, self.eyes_at(q));
        let (flank, spine) = self.hide(q, p);
        base - (flank * k + spine + rings) * fade
    }

    /// The sculpt as poured: the skinned field and its second fill, kept to a skin over the band.
    fn field(&self, p: P3) -> f64 {
        let mut f = self.skinned(p);
        if let Some(g) = &self.grid {
            f = f.min(g.at(&g.fill2, p));
        }
        f.max(-self.frame.crest.band_sd(p) - SINK_MM).max(self.bore_r + BORE_CLEAR_MM - p[0].hypot(p[1]))
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

/// A cylindrical grid whose columns run along the pull, holding the fills and the hide's budget. A fill at a node is
/// the least, over the sculpt above it toward its own mould half's open side, of the field there less the lean the
/// walls are given for each step down: what the sand needs filled under the sculpt. Sampled trilinearly; outside the
/// grid it is far.
struct Grid {
    theta0: f64,
    dtheta: f64,
    r0: f64,
    z0: f64,
    step: f64,
    n: [usize; 3],
    fill1: Vec<f32>,
    budget: Vec<f32>,
    fill2: Vec<f32>,
    /// Nodes each fill reached outside what it filled under.
    filled: [usize; 2],
}

impl Grid {
    fn new(lo_x: f64, hi_x: f64, r_lo: f64, r_hi: f64, z_hi: f64) -> Self {
        let step = FILL_STEP_MM;
        let nt = ((hi_x - lo_x) / step).ceil() as usize + 1;
        let nr = ((r_hi - r_lo) / step).ceil() as usize + 1;
        let nz = (2.0 * z_hi / step).ceil() as usize + 1;
        Self { theta0: theta_of(lo_x).to_radians(), dtheta: step / R_REF, r0: r_lo, z0: -z_hi, step, n: [nt, nr, nz], fill1: Vec::new(), budget: Vec::new(), fill2: Vec::new(), filled: [0, 0] }
    }

    fn node(&self, it: usize, ir: usize, iz: usize) -> P3 {
        let th = self.theta0 + it as f64 * self.dtheta;
        let r = self.r0 + ir as f64 * self.step;
        [r * th.cos(), r * th.sin(), self.z0 + iz as f64 * self.step]
    }

    /// `f` at every node, column by column across the threads.
    fn sample(&self, f: &(dyn Fn(P3) -> f64 + Sync)) -> Vec<f32> {
        let [nt, nr, nz] = self.n;
        let cols = nt * nr;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let chunk = cols.div_ceil(threads);
        let mut out = vec![0f32; cols * nz];
        std::thread::scope(|sc| {
            for (ci, part) in out.chunks_mut(chunk * nz).enumerate() {
                sc.spawn(move || {
                    for (k, column) in part.chunks_mut(nz).enumerate() {
                        let c = ci * chunk + k;
                        let (it, ir) = (c / nr, c % nr);
                        for (iz, v) in column.iter_mut().enumerate() {
                            *v = f(self.node(it, ir, iz)) as f32;
                        }
                    }
                });
            }
        });
        out
    }

    /// The pull fill of a sampled field, and how many nodes it puts metal at that the field had as air.
    fn fill_of(&self, raw: &[f32]) -> (Vec<f32>, usize) {
        let nz = self.n[2];
        let lean = (self.step * FILL_LEAN_DEG.to_radians().tan()) as f32;
        let zero = ((0.0 - self.z0) / self.step).round() as usize;
        let mut g = vec![f32::INFINITY; raw.len()];
        let mut added = 0;
        for (col, out) in raw.chunks(nz).zip(g.chunks_mut(nz)) {
            // Above the parting plane the cope lifts toward +z: fill from the top down; below, from the bottom up.
            let mut run = f32::INFINITY;
            for iz in (zero..nz).rev() {
                out[iz] = run;
                run = run.min(col[iz]) - lean;
            }
            let mut run = f32::INFINITY;
            for iz in 0..zero {
                out[iz] = run;
                run = run.min(col[iz]) - lean;
            }
            out[zero] = out[zero].min(run);
            added += col.iter().zip(out.iter()).filter(|(c, g)| **c > 0.0 && **g < 0.0).count();
        }
        (g, added)
    }

    /// The hide's budget at every node: 0 where the filled surface stands near upright to the pull, 1 where it falls
    /// away from the parting line steeply enough to carry the hide's own slopes.
    fn budget_of(&self, raw: &[f32], fill: &[f32]) -> Vec<f32> {
        let [nt, nr, nz] = self.n;
        let base = |it: usize, ir: usize, iz: usize| {
            let i = (it * nr + ir) * nz + iz;
            raw[i].min(fill[i]) as f64
        };
        let mut out = vec![0f32; raw.len()];
        for it in 1..nt - 1 {
            for ir in 1..nr - 1 {
                let r = self.r0 + ir as f64 * self.step;
                for iz in 1..nz - 1 {
                    let v = base(it, ir, iz);
                    if v.abs() > 0.45 {
                        continue;
                    }
                    let z = self.z0 + iz as f64 * self.step;
                    if z.abs() < 0.08 {
                        continue;
                    }
                    let gt = (base(it + 1, ir, iz) - base(it - 1, ir, iz)) / (2.0 * r * self.dtheta);
                    let gr = (base(it, ir + 1, iz) - base(it, ir - 1, iz)) / (2.0 * self.step);
                    let gz = (base(it, ir, iz + 1) - base(it, ir, iz - 1)) / (2.0 * self.step);
                    let l = (gt * gt + gr * gr + gz * gz).sqrt();
                    if l < 1e-9 {
                        continue;
                    }
                    let draft = (gz / l * z.signum()).clamp(-1.0, 1.0).asin().to_degrees();
                    let from = FILL_LEAN_DEG + HIDE_MARGIN_DEG;
                    out[(it * nr + ir) * nz + iz] = smoothstep(from, from + HIDE_RAMP_DEG, draft) as f32;
                }
            }
        }
        out
    }

    fn at(&self, a: &[f32], p: P3) -> f64 {
        if a.is_empty() {
            return 1e3;
        }
        let th = p[1].atan2(p[0]);
        let dth = (th - self.theta0).rem_euclid(2.0 * PI);
        let f = [dth / self.dtheta, (p[0].hypot(p[1]) - self.r0) / self.step, (p[2] - self.z0) / self.step];
        let mut i = [0usize; 3];
        let mut t = [0.0; 3];
        for k in 0..3 {
            if !(f[k] >= 0.0 && f[k] <= (self.n[k] - 1) as f64) {
                return 1e3;
            }
            let fl = f[k].floor().min((self.n[k] - 2) as f64);
            i[k] = fl as usize;
            t[k] = f[k] - fl;
        }
        let idx = |x: usize, y: usize, z: usize| ((x * self.n[1]) + y) * self.n[2] + z;
        let mut v = 0.0;
        for (da, wa) in [(0, 1.0 - t[0]), (1, t[0])] {
            for (db, wb) in [(0, 1.0 - t[1]), (1, t[1])] {
                for (dc, wc) in [(0, 1.0 - t[2]), (1, t[2])] {
                    let g = a[idx(i[0] + da, i[1] + db, i[2] + dc)];
                    v += wa * wb * wc * if g.is_finite() { (g as f64).min(1e3) } else { 1e3 };
                }
            }
        }
        v
    }
}

/// Make the sculpt pullable: fill the modelled body, let the hide in where the filled slope can carry it, and fill once
/// more for what the hide itself asks.
fn make_fills(gek: &mut Gekko, grid: Grid) -> Grid {
    let mut grid = grid;
    let raw = grid.sample(&|p| gek.raw(p));
    let (fill1, a1) = grid.fill_of(&raw);
    grid.budget = grid.budget_of(&raw, &fill1);
    grid.fill1 = fill1;
    drop(raw);
    gek.grid = Some(grid);
    let skinned = gek.grid.as_ref().unwrap().sample(&|p| gek.skinned(p));
    let mut grid = gek.grid.take().unwrap();
    let (fill2, a2) = grid.fill_of(&skinned);
    grid.fill2 = fill2;
    grid.filled = [a1, a2];
    grid
}

// --- Meshing ---------------------------------------------------------------------------------------------------------

#[derive(Default, serde::Serialize)]
struct Composition {
    crest_r_at_head_mm: f64,
    prims: usize,
    spots: usize,
    fill_nodes: [usize; 3],
    fill_added_nodes: [usize; 2],
    fill_s: f64,
    raw_faces: usize,
    faces: usize,
    parting_splits: usize,
    /// Edges flipped so a face leaning back across the parting line leans forward.
    draft_flips: usize,
    eye_faces: usize,
    web_cut_faces: Vec<usize>,
    volume_mm3: f64,
    box_mm: [P3; 2],
    sculpt_s: f64,
    decimation: (usize, f64),
    notes: Vec<String>,
}

/// Split every face that crosses the parting plane where it crosses, and lift each new vertex radially onto the
/// field: the spine's ridge then runs on z = 0 itself, and no face leans across the plane into the other mould half.
fn split_at_parting(s: &mut csg::Solid, field: &(dyn Fn(P3) -> f64 + Sync)) -> usize {
    use std::collections::HashMap;
    let mut on: HashMap<(u32, u32), u32> = HashMap::new();
    let mut faces = Vec::with_capacity(s.f.len() + 1024);
    let side = |p: P3| if p[2] > 1e-9 { 1 } else if p[2] < -1e-9 { -1 } else { 0 };
    let mut made = 0;
    let old = std::mem::take(&mut s.f);
    for f in old {
        let sd = f.map(|i| side(s.v[i as usize]));
        let crosses = (0..3).any(|k| sd[k] * sd[(k + 1) % 3] < 0);
        if !crosses {
            faces.push(f);
            continue;
        }
        // Rotate so the lone vertex on its own side is first.
        let lone = (0..3).find(|&k| sd[k] != 0 && sd[(k + 1) % 3] != sd[k] && sd[(k + 2) % 3] != sd[k]).unwrap_or(0);
        let [a, b, c] = [f[lone], f[(lone + 1) % 3], f[(lone + 2) % 3]];
        let mut cut = |i: u32, j: u32, v: &mut Vec<P3>| -> u32 {
            let key = (i.min(j), i.max(j));
            if let Some(&m) = on.get(&key) {
                return m;
            }
            let (p, q) = (v[i as usize], v[j as usize]);
            let t = p[2] / (p[2] - q[2]);
            let m = [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, 0.0];
            v.push(m);
            let id = (v.len() - 1) as u32;
            on.insert(key, id);
            made += 1;
            id
        };
        let sb = sd[(lone + 1) % 3];
        let sc = sd[(lone + 2) % 3];
        if sb == 0 {
            // b on the plane: one cut on a-c.
            let m = cut(a, c, &mut s.v);
            faces.push([a, b, m]);
            faces.push([m, b, c]);
        } else if sc == 0 {
            let m = cut(a, b, &mut s.v);
            faces.push([a, m, c]);
            faces.push([m, b, c]);
        } else {
            let mab = cut(a, b, &mut s.v);
            let mac = cut(a, c, &mut s.v);
            faces.push([a, mab, mac]);
            faces.push([mab, b, c]);
            faces.push([mab, c, mac]);
        }
    }
    s.f = faces;
    // Lift each new vertex along the radius onto the surface.
    if std::env::var_os("GEKKO_NOLIFT").is_some() {
        return made;
    }
    for (_, &id) in on.iter() {
        let p = s.v[id as usize];
        let rr = p[0].hypot(p[1]);
        let at = |dr: f64| {
            let k = (rr + dr) / rr;
            field([p[0] * k, p[1] * k, 0.0])
        };
        let (mut lo, mut hi) = (-0.06, 0.06);
        if at(lo) <= 0.0 && at(hi) > 0.0 {
            for _ in 0..30 {
                let m = 0.5 * (lo + hi);
                if at(m) > 0.0 { hi = m } else { lo = m }
            }
            let k = (rr + 0.5 * (lo + hi)) / rr;
            s.v[id as usize] = [p[0] * k, p[1] * k, 0.0];
        }
    }
    made
}

/// The draft of a face against the mould half its centroid stands in, degrees; `None` on a sliver or on the plane.
fn face_draft(s: &csg::Solid, f: [u32; 3]) -> Option<f64> {
    let [a, b, c] = f.map(|i| s.v[i as usize]);
    let (e1, e2) = (sub(b, a), sub(c, a));
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let l = norm(n);
    let m = mul(add(add(a, b), c), 1.0 / 3.0);
    if l < 1e-12 || m[2].abs() < 0.012 {
        return None;
    }
    Some((n[2] / l * m[2].signum()).clamp(-1.0, 1.0).asin().to_degrees())
}

/// Flip the shared edge of two faces wherever that turns a face leaning back across the parting line forward, the
/// quad barely folding: the facets that cut a curved surface's corners then lean the way the surface itself does.
/// Moves no vertex.
fn flip_for_draft(s: &mut csg::Solid, min_deg: f64) -> usize {
    use std::collections::HashMap;
    let unit_n = |s: &csg::Solid, f: [u32; 3]| {
        let [a, b, c] = f.map(|i| s.v[i as usize]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = norm(n);
        if l < 1e-14 { None } else { Some((mul(n, 1.0 / l), 0.5 * l)) }
    };
    let mut flips = 0;
    for _ in 0..6 {
        let mut edges: HashMap<(u32, u32), usize> = HashMap::with_capacity(s.f.len() * 3);
        for (fi, f) in s.f.iter().enumerate() {
            for k in 0..3 {
                edges.insert((f[k], f[(k + 1) % 3]), fi);
            }
        }
        let mut done = vec![false; s.f.len()];
        let mut round = 0;
        for fi in 0..s.f.len() {
            if done[fi] {
                continue;
            }
            let Some(d1) = face_draft(s, s.f[fi]) else { continue };
            if d1 >= min_deg {
                continue;
            }
            let mut best: Option<(f64, usize, [u32; 3], [u32; 3])> = None;
            for k in 0..3 {
                let f1 = s.f[fi];
                let (a, b, c) = (f1[k], f1[(k + 1) % 3], f1[(k + 2) % 3]);
                let Some(&gi) = edges.get(&(b, a)) else { continue };
                if done[gi] || gi == fi {
                    continue;
                }
                let f2 = s.f[gi];
                let Some(&d) = f2.iter().find(|&&v| v != a && v != b) else { continue };
                if edges.contains_key(&(c, d)) || edges.contains_key(&(d, c)) || c == d {
                    continue;
                }
                let (n1, n2) = (f1, [c, a, d]);
                let _ = n1;
                let (g1, g2) = ([c, a, d], [d, b, c]);
                let (Some((m1, ar1)), Some((m2, ar2)), Some((o1, _)), Some((o2, _))) = (unit_n(s, g1), unit_n(s, g2), unit_n(s, f1), unit_n(s, f2)) else { continue };
                let _ = n2;
                // The flip must not fold the quad: each new face near the old ones, and not a sliver.
                if dot(m1, m2) < 0.94 || dot(m1, o1) < 0.94 || dot(m2, o2) < 0.94 || dot(m1, o2) < 0.94 || ar1 < 1e-7 || ar2 < 1e-7 {
                    continue;
                }
                let old = d1.min(face_draft(s, f2).unwrap_or(90.0));
                let new = face_draft(s, g1).unwrap_or(90.0).min(face_draft(s, g2).unwrap_or(90.0));
                if new > old + 0.25 && best.as_ref().is_none_or(|bst| new > bst.0) {
                    best = Some((new, gi, g1, g2));
                }
            }
            if let Some((_, gi, g1, g2)) = best {
                for f in [s.f[fi], s.f[gi]] {
                    for k in 0..3 {
                        edges.remove(&(f[k], f[(k + 1) % 3]));
                    }
                }
                s.f[fi] = g1;
                s.f[gi] = g2;
                for (f, id) in [(g1, fi), (g2, gi)] {
                    for k in 0..3 {
                        edges.insert((f[k], f[(k + 1) % 3]), id);
                    }
                }
                done[fi] = true;
                done[gi] = true;
                round += 1;
            }
        }
        flips += round;
        if round == 0 {
            break;
        }
    }
    flips
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
    let mut s = sculpt::settle(nets, &field, &|_| false);
    let before = s.clone();
    comp.parting_splits = split_at_parting(&mut s, &field);
    let (xc, open) = (csg::self_crossings(&s), sculpt::closure(&s).0);
    if xc != 0 || open != 0 {
        comp.notes.push(format!("the parting split crossed itself ({xc} crossings, {open} open edges); kept the settled mesh"));
        s = before;
        comp.parting_splits = 0;
    }
    let before = s.clone();
    comp.draft_flips = flip_for_draft(&mut s, 1.0);
    let (xc, open) = (csg::self_crossings(&s), sculpt::closure(&s).0);
    if xc != 0 || open != 0 {
        comp.notes.push(format!("the draft flips crossed the mesh ({xc} crossings, {open} open edges); kept it unflipped"));
        s = before;
        comp.draft_flips = 0;
    }
    if std::env::var_os("GEKKO_UNDERCUT").is_some() {
        let mut bad: Vec<(f64, f64, P3)> = Vec::new();
        for f in &s.f {
            let [a, b, c] = f.map(|i| s.v[i as usize]);
            let n = {
                let (e1, e2) = (sub(b, a), sub(c, a));
                [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
            };
            let area = 0.5 * norm(n);
            if area < 1e-12 {
                continue;
            }
            let m = mul(add(add(a, b), c), 1.0 / 3.0);
            if m[2].abs() < 0.015 || gek.frame.crest.band_sd(m) < -0.02 {
                continue;
            }
            let draft = (n[2] / (2.0 * area) * m[2].signum()).asin().to_degrees();
            if draft < -1.0 {
                bad.push((area, draft, gek.frame.local(m)));
            }
        }
        let total: f64 = bad.iter().map(|b| b.0).sum();
        bad.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("  undercut faces {} over {:.4} mm2 (above the band)", bad.len(), total);
        for (a, d, q) in bad.iter().take(25) {
            println!("    {:.5} mm2 at {:.1} deg: x {:.2} h {:.2} w {:.2}", a, d, q[0], q[1], q[2]);
        }
    }
    comp.faces = s.f.len();
    comp.volume_mm3 = sculpt::closure(&s).1;
    comp.sculpt_s = t.elapsed().as_secs_f64();
    s
}

/// The crown's granules, 0..1, at `along` mm round the ring and `z` mm across where the bare crown falls at
/// `draft_deg`: a lattice of low scales, each rising gently from the parting line's side, no faster than the crown
/// falls, and dropping steeply on its outer side, so the sand draws every one.
const CROWN_GRANULE_MM: f64 = 0.1;
const CROWN_PITCH_MM: f64 = 0.85;
const CROWN_ROW_MM: f64 = 1.05;
fn crown_granules(along: f64, z: f64, draft_deg: f64) -> f64 {
    if draft_deg < 12.0 {
        return 0.0;
    }
    let r = 0.3;
    // The inner ramp's length, so its rise stays under the crown's fall less the sand's draft.
    let room = (draft_deg.to_radians().tan() - 6f64.to_radians().tan()).max(0.05);
    let stretch = (CROWN_GRANULE_MM * 2.2 / (0.55 * r * room)).clamp(1.0, 2.4);
    let (gx, gz) = (CROWN_PITCH_MM, CROWN_ROW_MM);
    let side = z.signum();
    let za = z.abs();
    let (ci, cj) = ((along / gx).floor() as i64, (za / gz).floor() as i64);
    let mut v = 0.0f64;
    for di in -1..=1 {
        for dj in -1..=1 {
            let (i, j) = (ci + di, cj + dj);
            let key = (i.wrapping_mul(73_856_093) ^ j.wrapping_mul(19_349_663) ^ (side as i64).wrapping_mul(83_492_791)) as usize;
            let stagger = if j.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
            let cx = (i as f64 + 0.5 + stagger + 0.14 * (hash(key, 61) - 0.5)) * gx;
            let cz = (j as f64 + 0.62) * gz;
            let a = za - cz;
            let b = along - cx;
            let a = if a < 0.0 { a / stretch } else { a };
            let d = a.hypot(b) / r;
            v = v.max(1.0 - smoothstep(0.45, 1.0, d));
        }
    }
    v
}

/// The band's hide: granules over the crown's flanks, let in as the crown falls away steeply enough to carry them,
/// and rows of enlarged tubercles on both side faces.
fn band_hide(d: &mut RingDesign, lib: &mut AlphaLibrary, crest: &Crest) -> Result<Vec<(String, skin::ClampReport)>> {
    let ctx = d.field_context();
    let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).and_then(|f| f.low).ok_or_else(|| anyhow::anyhow!("no side face"))?;
    let face_h = faces.1 - faces.0;
    let repeats = (ctx.circumference_mm / 1.35).round() as u32;
    let pitch = ctx.circumference_mm / repeats as f64;
    d.svgs.push(SvgAlpha { name: "Tubercle rows".into(), svg: rsvg::tubercle_rows(&Params::new(pitch, face_h, 0.42, 0.9)), invert: false });
    let mut t = TilingLayer::default_for("Tubercle rows", &ctx);
    ensure!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG), "the band has no side faces");
    t.repeats_around = repeats;
    t.height_mm = 0.32;
    t.shear = 0.35;
    let mut e = LayerEntry::new("Tubercle rows", Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);

    let a = Atlas::of(d, 2048, 768)?;
    let mut alpha = a.paint("Crown granules", |s| {
        let rho = s.p[0].hypot(s.p[1]).max(1e-9);
        if (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / rho < 0.35 {
            return 0.0;
        }
        let along = s.theta.to_radians() * R_REF;
        crown_granules(along, s.p[2], crest.draft_deg(s.theta, s.p[2]))
    });
    let bite = skin::draft_clamp(&a, &mut alpha, CROWN_GRANULE_MM)?;
    println!("  Crown granules: the draft rule cut {} texels, at most {:.3} mm", bite.texels_cut, bite.worst_mm);
    lib.insert(ringdesign_core::Alpha::from_png16("Crown granules", &alpha.to_png16()?)?);
    let mut e = skin::hide_layer(d, "Crown granules", CROWN_GRANULE_MM, ringdesign_core::field::Window::default());
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok(vec![("Crown granules".into(), bite)])
}

/// The crown's own relief at a point, mm, as painted: what a bench cut's floor stands over.
fn crown_relief(crest: &Crest, p: P3) -> f64 {
    let theta = p[1].atan2(p[0]).to_degrees();
    CROWN_GRANULE_MM * crown_granules(theta.rem_euclid(360.0).to_radians() * R_REF, p[2], crest.draft_deg(theta, p[2]))
}

/// What the bench cuts from the cast round each limb: everything the pull fill put between the elbow's crook, the
/// toes and the pads, down to a 0.03 mm film on the band, so the finished limb stands as modelled.
fn web_cutter(gek: &Gekko, l: &LimbPlan) -> csg::Solid {
    let [_, el, wr, ft] = l.joints;
    let zone = |q: P3| {
        let mut z = (q[0] - ft[0]).hypot(q[2] - ft[2]) - 2.1;
        for (a, b, r) in [(el, wr, 1.1), (wr, ft, 1.1)] {
            z = z.min(round_cone([q[0], 0.0, q[2]], [a[0], 0.0, a[2]], [b[0], 0.0, b[2]], r, r));
        }
        z = z.min((q[0] - el[0]).hypot(q[2] - el[2]) - 1.05);
        z.max(1.45 - q[2].abs())
    };
    let field = |p: P3| {
        let q = gek.frame.local(p);
        let z = zone(q);
        if z > 0.3 {
            return z;
        }
        z.max(0.025 - gek.modelled(p)).max(0.03 + crown_relief(gek.frame.crest, p) - gek.frame.crest.band_sd(p)).max(q[1] - 1.2)
    };
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for c in [el, wr, ft] {
        for dx in [-2.4, 2.4] {
            for dh in [-1.8, 1.4] {
                let w = gek.frame.world([c[0] + dx, c[1] + dh, c[2]]);
                for k in 0..2 {
                    lo[k] = lo[k].min(w[k]);
                    hi[k] = hi[k].max(w[k]);
                }
            }
        }
        lo[2] = lo[2].min(c[2] - 2.4);
        hi[2] = hi[2].max(c[2] + 2.4);
    }
    let mut raw = sculpt::tetra_mesh(lo, hi, 0.035, &field);
    let unrelaxed = raw.clone();
    sculpt::relax(&mut raw, &field, 2);
    if csg::self_crossings(&raw) != 0 {
        raw = unrelaxed;
    }
    for target in [30_000, 45_000, 70_000] {
        let nets = sculpt::clean_decimate(&raw, target);
        let s = sculpt::settle(nets, &field, &|_| false);
        if csg::self_crossings(&s) == 0 && sculpt::closure(&s).0 == 0 {
            return s;
        }
    }
    raw
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

/// The eyes: two balls with their slit pupils, meshed apart and set into the head at the bench.
fn eye_solid(gek: &Gekko) -> csg::Solid {
    let field = |p: P3| gek.eyes_world(p);
    let c = gek.frame.world([EYE_X, gek.eye_h, 0.0]);
    let pad = EYE_R + 0.3;
    let lo = [c[0] - pad, c[1] - pad, -EYE_W - pad];
    let hi = [c[0] + pad, c[1] + pad, EYE_W + pad];
    let mut raw = sculpt::tetra_mesh(lo, hi, 0.035, &field);
    sculpt::relax(&mut raw, &field, 3);
    let nets = sculpt::clean_decimate(&raw, 16_000);
    sculpt::settle(nets, &field, &|_| false)
}

struct Authored {
    d: RingDesign,
    lib: AlphaLibrary,
    comp: Composition,
    solid: csg::Solid,
    bites: Vec<(String, skin::ClampReport)>,
}

fn author(block_out: bool) -> Result<Authored> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let crest = Crest::of(&d)?;
    let bites = band_hide(&mut d, &mut lib, &crest)?;
    let mut comp = Composition { crest_r_at_head_mm: crest.at(theta_of(HEAD_X)), ..Composition::default() };
    let mut gek = Gekko::new(Frame { crest: &crest }, d.inner_radius_mm(), block_out);
    comp.prims = gek.prims.len();
    comp.spots = gek.spots.len();
    let t = Instant::now();
    let r_mid = crest.at(THETA_C);
    let grid = Grid::new(VENT_X - TAIL_LEN - 1.5, HEAD_X + 4.5, d.inner_radius_mm() + BORE_CLEAR_MM - 0.1, r_mid + 2.6, crest.half_w(THETA_C) + 1.0);
    let grid = make_fills(&mut gek, grid);
    comp.fill_nodes = grid.n;
    comp.fill_added_nodes = grid.filled;
    comp.fill_s = t.elapsed().as_secs_f64();
    println!("  eye centre h {:.2}", gek.eye_h);
    println!("  fill grid {:?}: the pull filled {} + {} nodes, {:.1} s", grid.n, grid.filled[0], grid.filled[1], comp.fill_s);
    gek.grid = Some(grid);
    let solid = sculpt_solid(&gek, &mut comp);
    println!("  draft flips {}", comp.draft_flips);
    println!("  sculpt {} raw faces -> {}, {} parting splits, {:.1} mm3, {:.1} s {:?}", comp.raw_faces, comp.faces, comp.parting_splits, comp.volume_mm3, comp.sculpt_s, comp.notes);
    let packed = sculpt::packed(&solid)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe {
        kernel: "sculpt".into(),
        op: "tokay".into(),
        params: json!({"theta_c_deg": THETA_C, "r_ref_mm": R_REF, "step_mm": STEP_MM, "faces": FACES, "fill_lean_deg": FILL_LEAN_DEG, "spots": comp.spots}),
        digest: String::new(),
    };
    doc.append(Feature { id: next, name: "Tokay".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component: joined() })?;
    let eyes = eye_solid(&gek);
    comp.eye_faces = eyes.f.len();
    let recipe = stored::Recipe { kernel: "sculpt".into(), op: "tokay eyes".into(), params: json!({"eye_r_mm": EYE_R, "eye_x_mm": EYE_X, "eye_w_mm": EYE_W}), digest: String::new() };
    let bench = Component { stage: Stage::Bench, bench_notes: "Two eyes, each a ball with an upright slit pupil, cast apart and set into the head at the bench, a quarter of each sunk.".into(), ..joined() };
    doc.append(Feature { id: next + 1, name: "Eyes".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: sculpt::packed(&eyes)? }, component: bench })?;
    let mut id = next + 2;
    for (k, l) in gek.limbs.iter().enumerate() {
        let cut = web_cutter(&gek, l);
        if cut.f.is_empty() {
            continue;
        }
        comp.web_cut_faces.push(cut.f.len());
        let name = format!("Webs, {} {}", if l.hind { "hind" } else { "fore" }, if l.joints[0][2] > 0.0 { "right" } else { "left" });
        let recipe = stored::Recipe { kernel: "sculpt".into(), op: "bench cut".into(), params: json!({"limb": k, "film_mm": 0.03, "skin_mm": 0.025}), digest: String::new() };
        let bench = Component {
            attach: Attach::Cut,
            stage: Stage::Bench,
            bench_notes: "Saw and graver out what the pour filled between the toes and in the limb's crook, down to the band.".into(),
            ..joined()
        };
        doc.append(Feature { id, name, enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: sculpt::packed(&cut)? }, component: bench })?;
        id += 1;
    }
    d.bake_all(&mut lib);
    Ok(Authored { d, lib, comp, solid, bites })
}

// --- Gates -----------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
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

fn release_json(r: &mf::release::ReleaseReport) -> Value {
    json!({
        "status": format!("{:?}", r.status),
        "obstructions": r.obstructions.len(),
        "unresolved_rays": r.unresolved_rays,
        "at": r.obstructions.iter().map(|o| json!({ "depth_mm": o.depth_mm, "area_mm2": o.projected_area_mm2, "theta_deg": o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0), "z_mm": o.world[2] })).collect::<Vec<_>>(),
    })
}

/// The draft rule over the whole field relief: what `skin::draft_clamp` would take from each layer and the composite.
fn clamp_audit(d: &RingDesign, lib: &AlphaLibrary) -> Result<Vec<(String, skin::ClampReport)>> {
    use ringdesign_core::field::Uv;
    let mut bare = d.clone();
    bare.layers.layers.clear();
    bare.cad = None;
    let a = Atlas::of(&bare, 2048, 256)?;
    let ctx = d.field_context();
    let mut out = Vec::new();
    if d.layers.layers.is_empty() {
        return Ok(out);
    }
    for e in &d.layers.layers {
        let mut solo = d.clone();
        solo.layers.layers = vec![e.clone()];
        let mut alpha = a.paint(e.name.clone(), |s| solo.layers.height(Uv { u: ctx.u_of_theta(s.theta), v: s.v }, &ctx, lib) / 2.0);
        out.push((e.name.clone(), skin::draft_clamp(&a, &mut alpha, 2.0)?));
    }
    let mut alpha = a.paint("composite", |s| d.layers.height(Uv { u: ctx.u_of_theta(s.theta), v: s.v }, &ctx, lib) / 2.0);
    out.push(("composite".into(), skin::draft_clamp(&a, &mut alpha, 2.0)?));
    Ok(out)
}

struct Gated {
    json: Value,
    passed: bool,
    built: mesh::BuildResult,
    pattern: mesh::BuildResult,
    inspection: mf::Inspection,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams, label: &str, painted: &[(String, skin::ClampReport)]) -> Result<Gated> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = ms(t);
    let v = &built.report.validation;
    let degenerate = built.report.quality.degenerate_faces;
    let crossings = csg::self_crossings(&solid_of(&built.mesh));
    let parts = part_crossings(&built);
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).count();
    let nearest = built.mesh.vertices.iter().map(|q| (q.0 as f64).hypot(q.1 as f64) - bore).fold(f64::MAX, f64::min);
    let band_field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let mut field = band_field.clone();
    castability::judge_parts(&mut field, d, &built);
    let drag = 100.0 * (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9);
    let dfm = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report(d, 0.0).map_or(0, |r| r.stone_count as usize);
    let preview_stones = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(d, lib, &setup, p)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let r = &inspection.release;
    let bites = clamp_audit(d, lib)?;
    let worst_bite = bites.iter().chain(painted).map(|b| b.1.worst_mm).fold(0.0, f64::max);
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let pv = &pattern.report.validation;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let triangles = built.mesh.faces.len();
    let mut notes = built.solids.notes.clone();
    notes.extend(built.parts.notes.iter().cloned());
    let list = [
        ("watertight, 0 degenerate faces", v.watertight && degenerate == 0),
        ("0 self-crossings on the ring and every made part", crossings == 0 && parts.iter().all(|p| p.1 == 0)),
        ("solids notes empty, every stamp resolved, the tokay and its eyes joined", notes.is_empty() && built.solids.stamped == d.stamps.len() && built.parts.joined == 2),
        ("nothing inside the finger hole", inside == 0),
        ("field Castable under SandTwoPart, band and with the part judged", field.process == CastProcess::SandTwoPart && band_field.verdict == Verdict::Castable && field.verdict == Verdict::Castable),
        ("ray release clean at 0.100 mm", r.obstructions.is_empty() && r.unresolved_rays == 0),
        ("ray release clean at 0.075 mm", release_fine.obstructions.is_empty() && release_fine.unresolved_rays == 0),
        ("draft-clamp bite at most 0.05 mm", worst_bite <= 0.05),
        ("0 DFM findings", dfm.is_empty()),
        ("stones report equals the preview", stones == preview_stones),
        ("within 2 million triangles", triangles <= 2_000_000),
        ("casting pattern closed, 0 degenerate, 0 crossings", pv.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0),
    ];
    let passed = list.iter().all(|g| g.1);
    println!("[{label}] {} triangles in {build_ms:.0} ms", triangles);
    for (g, ok) in &list {
        println!("  {} {g}", if *ok { "pass" } else { "FAIL" });
    }
    println!("  field {} / with part {} (worst draft {:.2} deg, undercut {:.4} mm2, drag {drag:.1}%) {:?}", band_field.verdict.label(), field.verdict.label(), field.worst_draft_deg, field.undercut_area_mm2, field.notes);
    for pv in &field.parts {
        println!("  part {}: undercut {:.4} (silhouette {:.4}), marginal {:.2}, vertical {:.2} of {:.1} mm2, worst {:.2} deg", pv.name, pv.undercut_area_mm2, pv.silhouette_mm2, pv.marginal_area_mm2, pv.vertical_area_mm2, pv.total_area_mm2, pv.worst_draft_deg);
    }
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    for n in &notes {
        println!("  note: {n}");
    }
    println!("  parts joined {}, stamped {}", built.parts.joined, built.solids.stamped);
    for o in r.obstructions.iter().chain(&release_fine.obstructions).take(20) {
        println!("  obstruction {:.3} mm deep at theta {:.1}, z {:.2}, r {:.2}", o.depth_mm, o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0), o.world[2], o.world[0].hypot(o.world[1]));
    }
    let json = json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": triangles, "ms": build_ms },
        "process": format!("{:?}", d.draft.process),
        "sand": format!("{:?}", d.draft.sand),
        "draft_rules": { "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "made_part_crossings": parts, "volume_mm3": built.report.volume_mm3 },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "parts_joined": built.parts.joined, "notes": notes },
        "finger_hole": { "bore_radius_mm": bore, "vertices_inside": inside, "nearest_margin_mm": nearest },
        "field": { "verdict": band_field.verdict.label(), "with_parts_verdict": field.verdict.label(), "process": format!("{:?}", field.process), "worst_draft_deg": field.worst_draft_deg, "undercut_area_mm2": field.undercut_area_mm2, "marginal_area_mm2": field.marginal_area_mm2, "vertical_area_mm2": field.vertical_area_mm2, "total_area_mm2": field.total_area_mm2, "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": field.notes, "parts": field.parts.iter().map(|pv| json!({"name": pv.name, "judged": pv.judged, "undercut_mm2": pv.undercut_area_mm2, "silhouette_mm2": pv.silhouette_mm2, "marginal_mm2": pv.marginal_area_mm2, "vertical_mm2": pv.vertical_area_mm2, "total_mm2": pv.total_area_mm2, "worst_draft_deg": pv.worst_draft_deg, "note": pv.note})).collect::<Vec<_>>() },
        "drag_pct": drag,
        "release_0100": release_json(r),
        "release_0075": release_json(&release_fine),
        "clamp_audit": { "worst_mm": worst_bite, "painted_bites": painted.iter().map(|(n, c)| json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>(), "layers": bites.iter().map(|(n, c)| json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>() },
        "dfm_findings": dfm.iter().map(|f| json!({ "label": f.label, "message": f.message })).collect::<Vec<_>>(),
        "stones": { "report_count": stones, "preview_count": preview_stones },
        "casting_pattern": { "watertight": pv.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings, "triangles": pattern.mesh.faces.len() },
        "gates": list.iter().map(|(g, ok)| json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "gates_passed": passed,
    });
    Ok(Gated { json, passed, built, pattern, inspection })
}

// --- Renders ---------------------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger axis, pitch from it.
const HERO: (f64, f64) = (-0.35, 0.95);
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

/// The faces of `m` with every corner within `radius` of `centre`.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let v = m.vertices[i as usize];
        let d = [v.0 as f64 - centre[0], v.1 as f64 - centre[1], v.2 as f64 - centre[2]];
        dot(d, d) < radius * radius
    };
    let mut remap = vec![u32::MAX; m.vertices.len()];
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        out.faces.push(f.map(|i| {
            if remap[i as usize] == u32::MAX {
                remap[i as usize] = out.vertices.len() as u32;
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals[i as usize]);
            }
            remap[i as usize]
        }));
    }
    out
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
    // No stone: the close-up is the head, from over the snout.
    let t = theta_of(EYE_X).to_radians();
    let head = crop(&built.mesh, [13.6 * t.cos(), 13.6 * t.sin(), 0.0], 7.5);
    render::write_png_parts(out.join("stones.png"), &[render::Part::metal(&head, render::GOLD)], 0.25, 1.15, edge)?;
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
    let Authored { mut d, lib, comp, solid, bites } = author(block_out)?;
    let p = params(draft);
    d.build = p;
    let mut blocks = serde_json::Map::new();
    let main = gates(&d, &lib, p, if draft { "draft 768 x 320" } else { "export 1536 x 448" }, &bites)?;
    let mut passed = main.passed;
    if !draft {
        let dr = gates(&d, &lib, params(true), "draft 768 x 320", &bites)?;
        passed &= dr.passed;
        blocks.insert("draft".into(), dr.json);
        blocks.insert("export".into(), main.json.clone());
    } else {
        blocks.insert("draft".into(), main.json.clone());
    }
    if std::env::var_os("GEKKO_COARSE").is_some() || !draft {
        let co = gates(&d, &lib, coarse_params(), "coarse 384 x 192", &bites)?;
        passed &= co.passed;
        blocks.insert("coarse_384x192".into(), co.json);
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
    let (part_min, part_under) = dfm::part_sections(&solid, None, d.draft.min_section_mm);
    let setup = d.manufacturing.clone().unwrap();
    let bytes = std::fs::metadata(out.join("design.ring.json")).map_or(0, |m| m.len());
    let report = json!({
        "ring": d.name,
        "slug": SLUG,
        "stage": if block_out { "block-out" } else { "full" },
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "stamps": d.stamps.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
        "sculpt": { "open_edges": open_edges, "self_crossings": sculpt_crossings, "part_sections_thinnest_mm": part_min, "part_sections_under_floor_mm2": part_under, "composition": comp },
        "design_bytes": bytes,
        "draft": blocks.get("draft"),
        "export": blocks.get("export"),
        "coarse_384x192": blocks.get("coarse_384x192"),
        "cold_reload": reload,
        "gates_passed": passed,
        "author_s": started.elapsed().as_secs_f64(),
        "manufacturing": mf::package::report(&d, &setup, &main.inspection, false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    stl::write_stl(out.join("finished-metal.stl"), &main.built.mesh, &d.name)?;
    stl::write_stl(out.join("casting-pattern.stl"), &main.pattern.mesh, &format!("{} / casting pattern", d.name))?;
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
