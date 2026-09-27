//! Cataphracta — Moloch, the thorn idol: a thorny devil lying along the crown, cast in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_moloch
//! target/release/examples/cataphracta_moloch [OUT_DIR] [--draft] [--verify]
//!
//! The lizard is one sculpted part: a broad flat body, the false head (the nuchal hump) with its two great spines,
//! a small horned head at the face, four splayed legs clasping the band's cheeks, and a thick spined tail running down
//! the crest toward the palm. Graded cone thorns follow the body: the largest in paired rows down the back, smaller
//! over the flanks, the legs and the tail. The sculpt is a distance field meshed by `sculpt`, joined to a keyed low
//! dome band.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm, library, manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    sculpt::{self, ellipsoid, round_cone, smin},
    skin::Atlas,
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const NAME: &str = "Moloch \u{2014} the thorn idol";
const SLUG: &str = "moloch";
/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The investment's fill floor and detail floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;

/// Ring angle of the body's frame origin, degrees; the head lies toward increasing angle.
const THETA_C: f64 = 78.0;
/// Radius at which the body's frame measures arc along the ring, mm.
const R_REF: f64 = 12.4;
/// Nothing of the sculpt comes nearer the finger axis than the bore plus this, mm.
const BORE_CLEAR_MM: f64 = 0.45;

/// Band half-width at the cheeks, mm.
const HALF_W: f64 = 3.3;

// The thorns: tip radius, the root sunk into the body, the base's share of the length, the lean back toward the tail.
const TIP_MM: f64 = 0.14;
const ROOT_SINK_MM: f64 = 0.22;
const LEAN_DEG: f64 = 30.0;
const THORN_BLEND_MM: f64 = 0.22;
const THORN_GAP_MM: f64 = 0.18;

/// The meshing step and the face budget of the sculpt.
const STEP_MM: f64 = 0.055;
const FACES: usize = 170_000;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The bare band: a low dome squared at the cheeks, a little deeper under the lizard, the palm the reference.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 2.0 * HALF_W;
    d.profile.thickness_mm = 2.0;
    d.profile.crown_mm = 0.55;
    d.profile.comfort_fit_mm = 0.15;
    d.profile.flatten_sides();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale: 1.0, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![key(20.0, 1.08), key(60.0, 1.15), key(120.0, 1.15), key(160.0, 1.06), key(210.0, 1.0), key(270.0, 1.0), key(330.0, 1.02)];
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
fn unit(a: P3) -> P3 {
    mul(a, 1.0 / dot(a, a).sqrt().max(1e-12))
}
fn wrap180(a: f64) -> f64 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// The band's crest radius round the ring, read off the bare atlas, one entry per column.
struct Crest {
    r: Vec<f64>,
}

impl Crest {
    fn of(d: &RingDesign) -> Result<Self> {
        let a = Atlas::of(d, 1440, 256)?;
        let r = (0..a.width).map(|x| (0..a.height).map(|y| { let p = a.at(x, y).p; p[0].hypot(p[1]) }).fold(0.0, f64::max)).collect();
        Ok(Self { r })
    }
    fn at(&self, theta_deg: f64) -> f64 {
        let n = self.r.len();
        let f = theta_deg.rem_euclid(360.0) / 360.0 * n as f64;
        let i = f.floor() as usize % n;
        let t = f - f.floor();
        self.r[i] * (1.0 - t) + self.r[(i + 1) % n] * t
    }
}

/// The body frame: `x` mm of arc along the ring from `THETA_C` (toward the head positive), `h` mm over the band's crest,
/// `w` mm along the finger.
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
        let theta = THETA_C + (q[0] / R_REF).to_degrees();
        let r = self.crest.at(theta) + q[1];
        let t = theta.to_radians();
        [r * t.cos(), r * t.sin(), q[2]]
    }
}

// --- The lizard ------------------------------------------------------------------------------------------------------

/// What a primitive belongs to, for the land-width census.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
enum Kind {
    Body,
    Hump,
    Head,
    Eye,
    Limb,
    Toe,
    Tail,
    HumpSpine,
    Horn,
    Major,
    Minor,
    TailThorn,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Body => "body",
            Kind::Hump => "nuchal hump",
            Kind::Head => "head and neck",
            Kind::Eye => "eyes",
            Kind::Limb => "limbs",
            Kind::Toe => "toes",
            Kind::Tail => "tail",
            Kind::HumpSpine => "hump spines",
            Kind::Horn => "brow horns",
            Kind::Major => "major thorns",
            Kind::Minor => "minor thorns",
            Kind::TailThorn => "tail thorns",
        }
    }
    /// How a section under the fill floor on this kind is met at the bench, or `None` where none is allowed.
    fn treatment(self) -> Option<&'static str> {
        match self {
            Kind::HumpSpine | Kind::Horn | Kind::Major | Kind::TailThorn => Some(
                "thorn point: the cone's last 0.6 mm tapers under the fill floor to a 0.28 mm rounded tip; fed through its root (at or over the floor) from the body, invested point up; a short-filled point is dressed with a needle file",
            ),
            Kind::Minor => Some("minor thorn: relief cast on the body, judged at the 0.15 mm detail floor; its point is left as cast and lightly burnished"),
            Kind::Toe => Some("toe tip: fed from the wrist; the claws are cleaned up with a graver and left sharp"),
            Kind::Limb => Some("wrist where the toes part: the crotches are opened with a graver after the pour"),
            Kind::Tail => Some("tail tip: fed along the tail from the body; the rounded end is dressed with a file"),
            _ => None,
        }
    }
}

/// One shape of the lizard, in the body frame or (thorns) in world millimetres.
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// An ellipsoid at a frame point, semi-axes along x, h, w; `snout` narrows its w toward +x.
    Egg { c: P3, r: P3, snout: f64 },
    /// A rounded cone between two frame points.
    Limb { a: P3, b: P3, ra: f64, rb: f64 },
    /// A rounded cone between two world points.
    Thorn { a: P3, b: P3, ra: f64, rb: f64 },
}

#[derive(Clone, Copy, Debug)]
struct Prim {
    kind: Kind,
    shape: Shape,
    /// Blend radius into what came before, mm.
    blend: f64,
}

impl Prim {
    fn eval(&self, q: P3, p: P3) -> f64 {
        match self.shape {
            Shape::Egg { c, r, snout } => {
                let d = sub(q, c);
                let narrow = if snout > 0.0 { 1.0 - snout * (d[0] / r[0]).clamp(0.0, 1.0) } else { 1.0 };
                ellipsoid(d, [r[0], r[1], r[2] * narrow])
            }
            Shape::Limb { a, b, ra, rb } => round_cone(q, a, b, ra, rb),
            Shape::Thorn { a, b, ra, rb } => round_cone(p, a, b, ra, rb),
        }
    }
    /// World bounding box, grown by `pad`.
    fn bounds(&self, frame: &Frame, pad: f64) -> (P3, P3) {
        let pts: Vec<(P3, f64)> = match self.shape {
            Shape::Egg { c, r, .. } => {
                let m = r[0].max(r[1]).max(r[2]);
                vec![(frame.world(c), m * 1.3)]
            }
            Shape::Limb { a, b, ra, rb } => vec![(frame.world(a), ra * 1.3), (frame.world(b), rb * 1.3)],
            Shape::Thorn { a, b, ra, rb } => vec![(a, ra), (b, rb)],
        };
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for (c, r) in &pts {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k] - r - pad);
                hi[k] = hi[k].max(c[k] + r + pad);
            }
        }
        if let Shape::Limb { .. } | Shape::Egg { .. } = self.shape {
            // A frame shape bends with the ring: pad the box for the bend over its span.
            for k in 0..3 {
                lo[k] -= 0.6;
                hi[k] += 0.6;
            }
        }
        (lo, hi)
    }
}

/// The lizard: the body's shapes blended in order, then the thorns found through a grid of world cells.
struct Lizard<'a> {
    frame: Frame<'a>,
    body: Vec<Prim>,
    thorns: Vec<Prim>,
    cell: f64,
    grid: std::collections::HashMap<[i32; 3], Vec<u32>>,
    bore_r: f64,
}

impl<'a> Lizard<'a> {
    fn new(frame: Frame<'a>, body: Vec<Prim>, bore_r: f64) -> Self {
        Self { frame, body, thorns: Vec::new(), cell: 1.0, grid: Default::default(), bore_r }
    }
    fn key(&self, p: P3) -> [i32; 3] {
        std::array::from_fn(|k| (p[k] / self.cell).floor() as i32)
    }
    fn push_thorn(&mut self, t: Prim) {
        let i = self.thorns.len() as u32;
        let (lo, hi) = t.bounds(&self.frame, 1.0);
        let (a, b) = (self.key(lo), self.key(hi));
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    self.grid.entry([x, y, z]).or_default().push(i);
                }
            }
        }
        self.thorns.push(t);
    }
    /// The body without thorns, in the frame.
    fn body_at(&self, q: P3, p: P3) -> f64 {
        let mut f = f64::MAX;
        for s in &self.body {
            let v = s.eval(q, p);
            f = if f == f64::MAX { v } else { smin(f, v, s.blend) };
        }
        f
    }
    fn field(&self, p: P3) -> f64 {
        let q = self.frame.local(p);
        let mut f = self.body_at(q, p);
        if let Some(list) = self.grid.get(&self.key(p)) {
            for &i in list {
                let t = &self.thorns[i as usize];
                f = smin(f, t.eval(q, p), t.blend);
            }
        }
        f.max(self.bore_r + BORE_CLEAR_MM - p[0].hypot(p[1]))
    }
    /// Which shape is nearest the surface at `p`.
    fn kind_at(&self, p: P3) -> Kind {
        let q = self.frame.local(p);
        let mut best = (f64::MAX, Kind::Body);
        for s in self.body.iter().chain(self.thorns.iter()) {
            let v = s.eval(q, p).abs();
            if v < best.0 {
                best = (v, s.kind);
            }
        }
        best.1
    }
    /// The body's top over a plan point, by bisection down from above: the frame point on the surface.
    fn top(&self, x: f64, w: f64) -> Option<P3> {
        let at = |h: f64| {
            let q = [x, h, w];
            self.body_at(q, self.frame.world(q))
        };
        let (mut hi, mut lo): (f64, f64) = (6.0, 6.0);
        while at(lo) > 0.0 {
            lo -= 0.1;
            if lo < -2.0 {
                return None;
            }
        }
        hi = hi.min(lo + 0.1);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if at(m) > 0.0 { hi = m } else { lo = m }
        }
        Some([x, 0.5 * (lo + hi), w])
    }
    /// World outward normal of the body at a world point.
    fn normal(&self, p: P3) -> P3 {
        let f = |p: P3| self.body_at(self.frame.local(p), p);
        let e = 1e-3;
        unit(std::array::from_fn(|k| {
            let (mut a, mut b) = (p, p);
            a[k] += e;
            b[k] -= e;
            (f(a) - f(b)) / (2.0 * e)
        }))
    }
}

/// Tail: its centre line and radius at a share of its length from the vent.
const TAIL_FROM: f64 = -7.6;
const TAIL_LEN: f64 = 17.0;
fn tail_at(t: f64) -> (P3, f64) {
    let x = TAIL_FROM - TAIL_LEN * t;
    let r = 1.7 * (1.0 - t).powf(0.85) + 0.45 * t;
    ([x, 0.55 * r - 0.05, 0.55 * (2.6 * t).sin()], r)
}

/// A leg's shoulder, elbow and wrist in the frame: out over the band's edge and down its cheek.
fn leg_joints(x0: f64, dir: f64, s: f64) -> (P3, P3, P3) {
    ([x0, 0.6, 3.0 * s], [x0 + dir * 1.4, 0.55, 5.3 * s], [x0 + dir * 2.4, -0.95, (HALF_W + 0.2) * s])
}

/// The lizard's soft body: trunk, hump, neck, head, eyes, four legs with their toes, and the tail.
fn body_prims() -> Vec<Prim> {
    let egg = |kind, c: P3, r: P3, snout: f64, blend: f64| Prim { kind, shape: Shape::Egg { c, r, snout }, blend };
    let limb = |kind, a: P3, b: P3, ra: f64, rb: f64, blend: f64| Prim { kind, shape: Shape::Limb { a, b, ra, rb }, blend };
    let mut v = vec![
        // The trunk: broad, flat and round, sunk into the crown.
        egg(Kind::Body, [-1.0, 0.35, 0.0], [7.3, 2.0, 4.4], 0.0, 0.0),
        // The false head: a round knob on the nape, standing over the neck.
        egg(Kind::Hump, [5.5, 1.95, 0.0], [1.65, 1.6, 1.75], 0.0, 0.9),
        egg(Kind::Head, [7.2, 1.05, 0.0], [1.6, 1.25, 2.05], 0.0, 0.8),
        // The head: small, blunt, narrowing to the snout.
        egg(Kind::Head, [9.4, 0.95, 0.0], [2.45, 1.2, 2.05], 0.4, 0.7),
        egg(Kind::Eye, [9.55, 1.55, 1.52], [0.52, 0.5, 0.5], 0.0, 0.15),
        egg(Kind::Eye, [9.55, 1.55, -1.52], [0.52, 0.5, 0.5], 0.0, 0.15),
    ];
    // Four legs: an upper limb out over the band's edge, a fore limb down the cheek, four toes spread on it.
    for s in [1.0, -1.0] {
        for (fore, x0) in [(true, 3.6), (false, -5.4)] {
            let dir = if fore { 1.0 } else { -1.0 };
            let (shoulder, elbow, wrist) = leg_joints(x0, dir, s);
            v.push(limb(Kind::Limb, shoulder, elbow, 1.0, 0.78, 0.6));
            v.push(limb(Kind::Limb, elbow, wrist, 0.78, 0.6, 0.35));
            for (ang, len) in [(-38.0, 0.95), (-12.0, 1.2), (14.0, 1.25), (40.0, 1.0)] {
                let a: f64 = f64::to_radians(ang);
                let tip = [wrist[0] + dir * (0.35 + len) * a.cos(), wrist[1] + (0.35 + len) * a.sin() - 0.1, (HALF_W + 0.04) * s];
                let root = [wrist[0] + dir * 0.25 * a.cos(), wrist[1] + 0.25 * a.sin(), (HALF_W + 0.14) * s];
                v.push(limb(Kind::Toe, root, tip, 0.42, 0.32, 0.2));
            }
        }
    }
    // The tail: from the vent down the crest toward the palm, thick and tapering, swaying a little.
    let n = 12;
    for k in 0..n {
        let ((a, ra), (b, rb)) = (tail_at(k as f64 / n as f64), tail_at((k + 1) as f64 / n as f64));
        v.push(limb(Kind::Tail, a, b, ra, rb, if k == 0 { 1.0 } else { 0.2 }));
    }
    v
}

/// A thorn's plan: frame x and w, length, kind, and how far it leans out along w.
struct Plan {
    x: f64,
    w: f64,
    len: f64,
    kind: Kind,
    splay: f64,
}

/// The thorns placed by hand: the hump's two spines, the brow horns, the paired rows down the back and flanks, the
/// tail's pairs.
fn major_plans() -> Vec<Plan> {
    let mut out = Vec::new();
    let pair = |out: &mut Vec<Plan>, x: f64, w: f64, len: f64, kind: Kind, splay: f64| {
        out.push(Plan { x, w, len, kind, splay });
        out.push(Plan { x, w: -w, len, kind, splay: -splay });
    };
    pair(&mut out, 5.55, 0.75, 2.3, Kind::HumpSpine, 0.3);
    pair(&mut out, 9.2, 1.45, 1.9, Kind::Horn, 1.1);
    // Paired rows down the back: dorsal, dorsolateral, lateral; graded from the shoulders to the hips.
    for (x, len) in [(3.2, 1.7), (0.6, 1.75), (-2.0, 1.65), (-4.6, 1.45)] {
        pair(&mut out, x, 1.1, len, Kind::Major, 0.12);
    }
    for (x, len) in [(1.9, 1.55), (-0.7, 1.6), (-3.3, 1.45), (-5.8, 1.15)] {
        pair(&mut out, x, 2.65, len, Kind::Major, 0.35);
    }
    for (x, len) in [(0.7, 1.1), (-2.0, 1.15), (-4.6, 1.0)] {
        pair(&mut out, x, 3.8, len, Kind::Major, 0.6);
    }
    // A thorn on each elbow.
    for (x0, dir) in [(3.6, 1.0), (-5.4, -1.0)] {
        let (_, e, _) = leg_joints(x0, dir, 1.0);
        pair(&mut out, e[0], e[2] - 0.1, 0.85, Kind::Minor, 0.6);
    }
    out
}

/// Strike one thorn at a frame plan point on the body: rooted on the surface, along its normal leaned back toward the
/// tail and out by `splay`.
fn thorn_at(liz: &Lizard, x: f64, w: f64, len: f64, kind: Kind, splay: f64, surface: Option<P3>) -> Option<(Prim, P3, f64)> {
    let q = surface.or_else(|| liz.top(x, w))?;
    let p = liz.frame.world(q);
    let n = liz.normal(p);
    let theta = p[1].atan2(p[0]);
    // Toward the tail: decreasing ring angle.
    let back = [theta.sin(), -theta.cos(), 0.0];
    let dir = unit(add(add(n, mul(back, LEAN_DEG.to_radians().tan())), [0.0, 0.0, splay * 0.5]));
    let root_r = if matches!(kind, Kind::Minor) { 0.46 * len } else { 0.42 * len };
    let a = sub(p, mul(n, ROOT_SINK_MM));
    let b = add(p, mul(dir, len));
    Some((Prim { kind, shape: Shape::Thorn { a, b, ra: root_r, rb: TIP_MM }, blend: THORN_BLEND_MM }, p, root_r))
}

/// Deterministic jitter in 0..1.
fn hash(k: usize, salt: u64) -> f64 {
    let mut x = (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// What the author put down, for the report.
#[derive(Default, serde::Serialize)]
struct Composition {
    crest_r_at_face_mm: f64,
    thorns_by_kind: Vec<(String, usize)>,
    thorn_lengths_mm: [f64; 2],
    sculpt_raw_faces: usize,
    sculpt_faces: usize,
    sculpt_volume_mm3: f64,
    sculpt_box_mm: [P3; 2],
    sculpt_s: f64,
}

fn build_lizard<'a>(frame: Frame<'a>, bore_r: f64) -> Lizard<'a> {
    let mut liz = Lizard::new(frame, body_prims(), bore_r);
    let mut placed: Vec<(P3, f64)> = Vec::new();
    let clear = |placed: &[(P3, f64)], p: P3, r: f64| placed.iter().all(|(q, rq)| dot(sub(p, *q), sub(p, *q)).sqrt() >= r + rq + THORN_GAP_MM);
    for pl in major_plans() {
        if let Some((t, p, r)) = thorn_at(&liz, pl.x, pl.w, pl.len, pl.kind, pl.splay, None) {
            placed.push((p, r));
            liz.push_thorn(t);
        }
    }
    // The tail's pairs: on its top either side, graded to the tip.
    for k in 0..12 {
        let t = (k as f64 + 0.45) / 12.0;
        let (c, r) = tail_at(t);
        let len = 1.3 - 0.75 * t;
        for s in [1.0, -1.0] {
            let w = c[2] + s * 0.55 * r;
            if let Some((th, p, rr)) = thorn_at(&liz, c[0], w, len, Kind::TailThorn, 0.35 * s, None) {
                if clear(&placed, p, rr) {
                    placed.push((p, rr));
                    liz.push_thorn(th);
                }
            }
        }
    }
    // Minor thorns over whatever the majors leave: a jittered lattice over the body's plan, graded smaller to the flanks.
    let mut cands: Vec<(f64, f64, usize)> = Vec::new();
    let mut k = 0usize;
    let mut x = 8.0;
    while x > -8.6 {
        let mut w = -5.4;
        while w < 5.4 {
            cands.push((x + 0.5 * (hash(k, 1) - 0.5), w + 0.5 * (hash(k, 2) - 0.5), k));
            k += 1;
            w += 0.62;
        }
        x -= 0.62;
    }
    cands.sort_by_key(|c| (hash(c.2, 3) * 1e12) as u64);
    for (x, w, k) in cands {
        let Some(q) = liz.top(x, w) else { continue };
        if q[1] < -0.25 {
            continue;
        }
        let edge = (w.abs() / 4.4).min(1.0);
        let len = (0.9 - 0.3 * edge) * (0.8 + 0.35 * hash(k, 4));
        let p = liz.frame.world(q);
        let r = 0.46 * len;
        if !clear(&placed, p, r) {
            continue;
        }
        if let Some((th, p, rr)) = thorn_at(&liz, x, w, len, Kind::Minor, 0.25 * w.signum() * edge, Some(q)) {
            placed.push((p, rr));
            liz.push_thorn(th);
        }
    }
    liz
}

/// The sculpt: meshed, relaxed, decimated to budget and settled.
fn sculpt_solid(liz: &Lizard, comp: &mut Composition) -> csg::Solid {
    let field = |p: P3| liz.field(p);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for s in liz.body.iter().chain(liz.thorns.iter()) {
        let (a, b) = s.bounds(&liz.frame, 0.3);
        for k in 0..3 {
            lo[k] = lo[k].min(a[k]);
            hi[k] = hi[k].max(b[k]);
        }
    }
    comp.sculpt_box_mm = [lo, hi];
    let t = std::time::Instant::now();
    let mut raw = sculpt::tetra_mesh(lo, hi, STEP_MM, &field);
    comp.sculpt_raw_faces = raw.f.len();
    sculpt::relax(&mut raw, &field, 3);
    let nets = sculpt::clean_decimate(&raw, FACES);
    let s = sculpt::settle(nets, &field, &|_| false);
    comp.sculpt_faces = s.f.len();
    comp.sculpt_volume_mm3 = sculpt::closure(&s).1;
    comp.sculpt_s = t.elapsed().as_secs_f64();
    s
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

fn author() -> Result<(RingDesign, AlphaLibrary, Composition, csg::Solid, Vec<(Kind, P3)>)> {
    let mut d = band();
    let lib = AlphaLibrary::builtin();
    let crest = Crest::of(&d)?;
    let mut comp = Composition { crest_r_at_face_mm: crest.at(90.0), ..Composition::default() };
    let liz = build_lizard(Frame { crest: &crest }, d.inner_radius_mm());
    let mut by_kind: std::collections::BTreeMap<String, usize> = Default::default();
    let (mut lmin, mut lmax) = (f64::MAX, 0.0f64);
    for t in &liz.thorns {
        *by_kind.entry(t.kind.label().into()).or_default() += 1;
        if let Shape::Thorn { a, b, .. } = t.shape {
            let l = dot(sub(b, a), sub(b, a)).sqrt() - ROOT_SINK_MM;
            lmin = lmin.min(l);
            lmax = lmax.max(l);
        }
    }
    comp.thorns_by_kind = by_kind.into_iter().collect();
    comp.thorn_lengths_mm = [lmin, lmax];
    let solid = sculpt_solid(&liz, &mut comp);
    // Each face's kind, for the land census: the shape nearest the surface at its centroid.
    let kinds: Vec<(Kind, P3)> = solid
        .f
        .iter()
        .map(|t| {
            let c = t.iter().fold([0.0; 3], |s, &i| add(s, solid.v[i as usize]));
            let c = mul(c, 1.0 / 3.0);
            (liz.kind_at(c), c)
        })
        .collect();
    let packed = sculpt::packed(&solid)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe {
        kernel: "sculpt".into(),
        op: "thorny devil".into(),
        params: json!({"theta_c_deg": THETA_C, "r_ref_mm": R_REF, "step_mm": STEP_MM, "faces": FACES, "thorns": liz.thorns.len(), "tip_mm": TIP_MM, "lean_deg": LEAN_DEG}),
        digest: String::new(),
    };
    doc.append(Feature { id: next, name: "Thorny devil".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component: joined() })?;
    Ok((d, lib, comp, solid, kinds))
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

/// Every vertex nearer the finger axis than the bore allows.
fn bore_intrusion(d: &RingDesign, m: &mesh::Mesh) -> (f64, usize) {
    let bore = d.inner_radius_mm();
    let mut least = f64::MAX;
    let mut inside = 0;
    for v in &m.vertices {
        let r = (v.0 as f64).hypot(v.1 as f64);
        least = least.min(r);
        inside += usize::from(r < bore - 0.01);
    }
    (least, inside)
}

/// The sculpt's sections face by face (as `dfm::part_sections` reads them), gathered by kind: the thinnest, and the
/// area under the fill floor.
fn land_census(solid: &csg::Solid, kinds: &[(Kind, P3)]) -> Vec<(Kind, f64, f64)> {
    use ringdesign_core::interaction::bvh::Bvh;
    let m = mesh::Mesh {
        vertices: solid.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces: solid.f.clone(),
        ..Default::default()
    };
    let bvh = Bvh::build(&m);
    const IN: f64 = 1e-4;
    let mut out: std::collections::BTreeMap<u8, (Kind, f64, f64)> = Default::default();
    for (fi, f) in solid.f.iter().enumerate() {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let twice = dot(n, n).sqrt();
        if !(twice > 1e-14) {
            continue;
        }
        let inward = mul(n, -1.0 / twice);
        let o = add(mul(add(add(a, b), c), 1.0 / 3.0), mul(inward, IN));
        let Some((_, t)) = bvh.ray(&m, o, inward) else { continue };
        let section = t + IN;
        let kind = kinds[fi].0;
        let e = out.entry(kind as u8).or_insert((kind, f64::MAX, 0.0));
        e.1 = e.1.min(section);
        if section < MIN_SECTION_MM {
            e.2 += 0.5 * twice;
        }
    }
    out.into_values().collect()
}

/// The camera for each named view: yaw about the finger axis, pitch from it toward the head.
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.15, 0.85),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.75, 0.62),
    ("reverse", PI - 0.5, 0.35),
];

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

/// Studio-gold renders, the 300 px read, a contact sheet and the bare band against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The stones view on a ring without stones: the false head and the horned head, close, from over the snout.
    render::write_png_parts(out.join("stones.png"), &parts, -0.95, 1.05, edge)?;
    let bare = mesh::try_build(&band(), lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let fin_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    let mut pair = vec![0u8; edge * 2 * edge * 3];
    paste(&mut pair, edge * 2, &bare_img, edge, 0, 0);
    paste(&mut pair, edge * 2, &fin_img, edge, edge, 0);
    image::save_buffer(out.join("bare-vs-finished.png"), &pair, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    render::write_png_parts(out.join("face-300.png"), &parts, VIEWS[1].1, VIEWS[1].2, 300)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    let mut sheet = vec![0u8; 900 * 600 * 3];
    for (k, (_, yaw, pitch)) in VIEWS.iter().enumerate() {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3);
        paste(&mut sheet, 900, &img, 300, (k % 3) * 300, (k / 3) * 300);
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 900, 600, image::ColorType::Rgb8)?;
    Ok(())
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
    fn json(&self) -> serde_json::Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "notes": self.notes, "made_part_crossings": self.parts, "parts_joined": self.joined})
    }
    fn line(&self) -> String {
        format!("{} tris, watertight {}, degenerate {}, crossings {}, notes {:?}, parts {:?}, joined {}", self.triangles, self.watertight, self.degenerate, self.crossings, self.notes, self.parts, self.joined)
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/cataphracta").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("{NAME}");
    let started = std::time::Instant::now();
    let (d, lib, comp, solid, kinds) = author()?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  crest r {:.2} at the face; thorns {:?}, lengths {:.2}-{:.2}; sculpt {} raw faces -> {}, {:.1} mm3, {:.1} s",
        comp.crest_r_at_face_mm, comp.thorns_by_kind, comp.thorn_lengths_mm[0], comp.thorn_lengths_mm[1], comp.sculpt_raw_faces, comp.sculpt_faces, comp.sculpt_volume_mm3, comp.sculpt_s
    );
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let main_pass = Pass::of(&built);
    println!("  {}x{}: {} ({build_s:.1} s)", params.theta_steps, params.profile_steps, main_pass.line());
    let coarse_pass = Pass::of(&mesh::try_build(&d, &lib, coarse_params())?);
    println!("  384x192: {}", coarse_pass.line());
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let sculpt_crossings = csg::self_crossings(&solid);
    let (open_edges, _) = sculpt::closure(&solid);
    // Lost wax: the verdict rides on fill; the two-part pull is only reported.
    let band_field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    // The investment's lands: the sculpt's sections against the fill floor, each shortfall named with its treatment.
    let (part_min, part_under) = dfm::part_sections(&solid, None, MIN_SECTION_MM);
    let census = land_census(&solid, &kinds);
    let unnamed: Vec<String> = census.iter().filter(|c| c.2 > 0.0 && c.0.treatment().is_none()).map(|c| format!("{}: {:.3} mm2 under, thinnest {:.2}", c.0.label(), c.2, c.1)).collect();
    let lands = json!({
        "floor_mm": MIN_SECTION_MM,
        "detail_floor_mm": MIN_DETAIL_MM,
        "method": "dfm::part_sections on the sculpted part (one ray per face along its inward normal), and the same per face gathered by the shape nearest each face",
        "part_sections": {"part": "Thorny devil", "thinnest_mm": part_min, "under_floor_mm2": part_under, "area_mm2": solid_area(&solid)},
        "by_kind": census.iter().map(|c| json!({"kind": c.0.label(), "thinnest_mm": c.1, "under_floor_mm2": c.2, "treatment": if c.2 > 0.0 { c.0.treatment() } else { None }})).collect::<Vec<_>>(),
        "unnamed_under_floor": unnamed,
        "band_thinnest_wall_mm": band_field.thinnest_wall_mm,
    });
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", main_pass.watertight && main_pass.degenerate == 0 && main_pass.crossings == 0),
        ("the sculpted part closed and uncrossed, as made and as placed", open_edges == 0 && sculpt_crossings == 0 && main_pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, the part joined", main_pass.notes.is_empty() && main_pass.joined == 1),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax field verdict Castable with the 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && band_field.verdict == Verdict::Castable && band_field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("every section under 0.8 mm named with its bench treatment", unnamed.is_empty()),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("gates hold at 384 x 192", coarse_pass.ok()),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within 2 million triangles", main_pass.triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "draft": {"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "gates_that_apply": "lost wax: geometry, bore, field fill verdict at 0.8 mm, land widths, DFM, stones, 384 x 192, pattern, triangles, cold reload. The sand gates (ray release, draft-clamp bites, two-part Castable, parting_monotone) do not apply; the two-part undercut is reported as a number.",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": main_pass.triangles, "build_s": build_s, "author_s": author_s},
        "main": main_pass.json(),
        "coarse_384x192": coarse_pass.json(),
        "sculpt": {"open_edges": open_edges, "self_crossings": sculpt_crossings, "faces": solid.f.len(), "raw_faces": comp.sculpt_raw_faces, "volume_mm3": comp.sculpt_volume_mm3},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "band_verdict": band_field.verdict.label(), "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": field.notes},
        "land_widths": lands,
        "two_part_undercut": {
            "band_percent": band_field.undercut_fraction() * 100.0,
            "with_parts_percent": field.undercut_fraction() * 100.0,
            "parts_undercut_mm2": field.parts.iter().map(|p| p.undercut_area_mm2).sum::<f64>(),
            "parts_area_mm2": field.parts.iter().map(|p| p.total_area_mm2).sum::<f64>(),
            "note": "reported only: lost wax judges fill and detail, never the pull",
        },
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "composition": comp,
        "design": {"bytes": text.len()},
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let name = if draft { "report-draft.json" } else { "report.json" };
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Moloch / lost-wax pattern")?;
    }
    if std::env::var("MOLOCH_VIEWS").is_ok() {
        let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
        let mut sheet = vec![0u8; 1200 * 900 * 3];
        for (k, (yaw, pitch)) in [(-0.9, 0.7), (-0.6, 0.8), (-0.42, 0.9), (-0.2, 1.0), (0.2, 0.9), (0.45, 0.8), (-1.2, 0.6), (-0.6, 1.2), (0.0, 1.2), (-1.6, 0.5), (1.2, 0.6), (0.0, 0.45)].iter().enumerate() {
            let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 2);
            paste(&mut sheet, 1200, &img, 300, (k % 4) * 300, (k / 4) * 300);
        }
        image::save_buffer(out.join("views-probe.png"), &sheet, 1200, 900, image::ColorType::Rgb8)?;
    }
    renders(&out, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} (band {}, thinnest {:.2} mm; two-part undercut {:.3}% band, {:.3}% with parts); dfm {}; pattern {pw}/{pd}/{px}; bore nearest {least_r:.3} of {:.3}; part sections min {part_min:.3}, {part_under:.2} mm2 under",
        field.verdict.label(),
        band_field.verdict.label(),
        band_field.thinnest_wall_mm,
        band_field.undercut_fraction() * 100.0,
        field.undercut_fraction() * 100.0,
        findings.len(),
        d.inner_radius_mm()
    );
    for c in &census {
        println!("    lands {}: thinnest {:.3}, {:.3} mm2 under", c.0.label(), c.1, c.2);
    }
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in &field.notes {
        println!("    field: {n}");
    }
    for (g, p) in &gates {
        println!("  {} {g}", if *p { "pass" } else { "FAIL" });
    }
    ensure!(!text.is_empty());
    Ok(())
}

fn solid_area(s: &csg::Solid) -> f64 {
    s.f.iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| s.v[i as usize]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            0.5 * dot(n, n).sqrt()
        })
        .sum()
}
