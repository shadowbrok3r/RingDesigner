//! Cataphracta — Moloch, the thorn idol: a thorny devil lying along the crown, cast in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_moloch
//! target/release/examples/cataphracta_moloch [OUT_DIR] [--draft] [--verify]
//!
//! The lizard is one sculpted part: a broad flat body, the false head (the nuchal hump) with its two great spines,
//! a small horned wedge of a head scaled with flat plates, four tapering legs ending in thorny devil's feet (five slender
//! keeled toes each, clawed, lying on the crown), and a spined tail running down the crest toward the palm, its thorns
//! lying back along it. Graded cone thorns follow the body: the largest in paired rows down the back, smaller over the
//! flanks and the legs. The sculpt is a distance field meshed by `sculpt`, joined to a keyed low dome band whose crown
//! is wind-rippled sand.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Blend, Layer, LayerEntry},
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    sculpt::{self, ellipsoid, round_cone, smin},
    skin::Atlas,
    stl,
    svg::SvgAlpha,
    tiling::TilingLayer,
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
const THETA_C: f64 = 72.0;
/// Radius at which the body's frame measures arc along the ring, mm.
const R_REF: f64 = 12.4;
/// Nothing of the sculpt comes nearer the finger axis than the bore plus this, mm.
const BORE_CLEAR_MM: f64 = 0.45;

/// Band half-width at the cheeks, mm.
const HALF_W: f64 = 3.3;
// The sand ripples on the band: their height, mm.
/// Tiles round the ring: a short tile keeps the alpha's pixels far under the crests' curvature.
const RIPPLE_TILES: u32 = 8;
const RIPPLE_MM: f64 = 0.22;

// The thorns: tip radius, the root sunk into the body, the base's share of the length, the lean back toward the tail.
const TIP_MM: f64 = 0.14;
/// A cone's root sphere is buried to this share of its radius, so the flank runs straight into the skin with no ball
/// under it.
const ROOT_SINK: f64 = 0.72;
const LEAN_DEG: f64 = 26.0;
const THORN_BLEND_MM: f64 = 0.22;

// The skin's granules: lattice pitch, bump radius and height.
const GRANULE_R_MM: f64 = 0.4;
const GRANULE_MM: f64 = 0.15;
// The hide's tubercles: cell pitch, the width of the groove between two, and their height.
const TUBERCLE_PITCH_MM: f64 = 0.8;
const TUBERCLE_GROOVE_MM: f64 = 0.2;
const TUBERCLE_MM: f64 = 0.2;
/// The head's flat scales: cell pitch and height, mm.
const HEAD_SCALE_PITCH_MM: f64 = 1.15;
const HEAD_SCALE_MM: f64 = 0.13;
/// Round 2's loose granules, off.
const BALL_GRANULES: bool = false;

/// The meshing step and the face budget of the sculpt.
const STEP_MM: f64 = 0.055;
const FACES: usize = 230_000;

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
    // Broad under the lizard, so its legs splay over the crown and grip the cheeks without standing past them.
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![key(10.0, 1.14, 1.06), key(40.0, 1.5, 1.12), key(65.0, 1.45, 1.15), key(110.0, 1.45, 1.15), key(140.0, 1.28, 1.1), key(170.0, 1.06, 1.04), key(210.0, 1.0, 1.0), key(270.0, 1.0, 1.0), key(330.0, 1.0, 1.02)];
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    // `crisp_relief` stays off: the template graph cannot yet carry it (its design node fails upstream), and the
    // ripples' lee faces are gentle enough (0.22 mm over 0.24 mm) not to step at the export grid.
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
    /// Half the band's width along the finger, per column.
    half_w: Vec<f64>,
    /// The crown's radius across the band, per column: (z, r) from the low cheek to the high, where the skin faces out.
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
    /// The bare crown's radius at ring angle `theta_deg` and `z` along the finger, or `None` off the crown.
    fn crown_r(&self, theta_deg: f64, z: f64) -> Option<f64> {
        let n = self.crown.len();
        let col = &self.crown[((theta_deg.rem_euclid(360.0) / 360.0 * n as f64).round() as usize) % n];
        let i = col.iter().position(|c| c.0 >= z)?;
        if i == 0 {
            return None;
        }
        let (a, b) = (col[i - 1], col[i]);
        Some(a.1 + (b.1 - a.1) * (z - a.0) / (b.0 - a.0).max(1e-9))
    }
    fn at(&self, theta_deg: f64) -> f64 {
        Self::read(&self.r, theta_deg)
    }
    /// The crown's height over the crest at a frame `x` and `w`, mm (at or under 0); the cheek's top edge past it.
    fn top_h(&self, x: f64, w: f64) -> f64 {
        let theta = (THETA_C + (x / R_REF).to_degrees()).rem_euclid(360.0);
        let n = self.crown.len();
        let col = &self.crown[((theta / 360.0 * n as f64).round() as usize) % n];
        let r = match col.iter().position(|c| c.0 >= w) {
            Some(0) => col[0].1,
            None => col.last().map_or(0.0, |c| c.1),
            Some(i) => {
                let (a, b) = (col[i - 1], col[i]);
                a.1 + (b.1 - a.1) * (w - a.0) / (b.0 - a.0).max(1e-9)
            }
        };
        r - self.at(theta)
    }
    /// Half the band's width at a frame `x`.
    fn half_w_at_x(&self, x: f64) -> f64 {
        Self::read(&self.half_w, THETA_C + (x / R_REF).to_degrees())
    }
    fn read(v: &[f64], theta_deg: f64) -> f64 {
        let n = v.len();
        let f = theta_deg.rem_euclid(360.0) / 360.0 * n as f64;
        let i = f.floor() as usize % n;
        let t = f - f.floor();
        v[i] * (1.0 - t) + v[(i + 1) % n] * t
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
    Limb,
    Toe,
    Claw,
    Tail,
    HumpSpine,
    Horn,
    Major,
    Minor,
    TailThorn,
    Granule,
    Hide,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Body => "body",
            Kind::Hump => "nuchal hump",
            Kind::Head => "head and neck",
            Kind::Limb => "limbs",
            Kind::Toe => "toes",
            Kind::Claw => "claws",
            Kind::Tail => "tail",
            Kind::HumpSpine => "hump spines",
            Kind::Horn => "brow horns",
            Kind::Major => "major thorns",
            Kind::Minor => "minor thorns",
            Kind::TailThorn => "tail thorns",
            Kind::Granule => "granules",
            Kind::Hide => "hide tubercles",
        }
    }
    /// How a section under the fill floor on this kind is met at the bench, or `None` where none is allowed.
    fn treatment(self) -> Option<&'static str> {
        match self {
            Kind::HumpSpine | Kind::Horn | Kind::Major | Kind::TailThorn => Some(
                "thorn point: the cone's last 0.6 mm tapers under the fill floor to its rounded point (a 0.28 mm tip sphere); the thinnest reading is a ray leaving a facet on the point's flank, stated below as measured; fed through its root (over the floor) from the body and invested point up; a short-filled point is built back with a laser tack and filed to shape",
            ),
            Kind::Minor => Some("minor thorn: relief cast on the body, judged at the 0.15 mm detail floor; its point is left as cast and lightly burnished"),
            Kind::Toe => Some("toe: a slender keeled crest lying on the crown and fused into the band along its whole length; fed from the foot and left as cast"),
            Kind::Claw => Some("claw: a 0.3 mm point, 0.42 mm across at its base, lying on the crown at the end of each toe and fused into the band along its length; like a thorn's point it is under the fill floor by design, fed from the toe and the band it lies on, and a short-filled point is built back with a laser tack"),
            Kind::Granule => Some("granule: a 0.5-0.7 mm bead of hide relief on the body, over the 0.15 mm detail floor; left as cast"),
            Kind::Hide => Some("hide tubercle: 0.2 mm pebbled relief on the thick body, read here where a face's ray crosses a tubercle's own flank and leaves within the relief layer; cast as relief over the 0.15 mm detail floor and left as cast"),
            Kind::Limb => Some("wrist where the toes leave the limb: fed along the limb from the body and left as cast"),
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
    Thorn { a: P3, b: P3, ra: f64, rb: f64, n: P3 },
    /// A ball at a world point.
    Ball { c: P3, r: f64 },
    /// A toe: a rounded crest from frame point `a` to `b` (crest radius `ra` to `rb`) whose flanks flare down into a
    /// keel buried in the band, so every section through it, crest or flank, holds the fill floor.
    Ridge { a: P3, b: P3, ra: f64, rb: f64 },
}

/// A toe's flanks widen this much along the finger per mm of depth, and its keel reaches this far under the crest, mm.
const RIDGE_FLARE: f64 = 0.5;
const RIDGE_KEEL_MM: f64 = 0.75;

/// The 2D uneven capsule: a circle of `r1` at the origin and one of `r2` at `(0, h)`, joined by tangent flanks.
fn uneven_capsule(px: f64, py: f64, r1: f64, r2: f64, h: f64) -> f64 {
    let px = px.abs();
    let b = (r1 - r2) / h;
    let a = (1.0 - b * b).max(0.0).sqrt();
    let k = -b * px + a * py;
    if k < 0.0 {
        px.hypot(py) - r1
    } else if k > a * h {
        px.hypot(py - h) - r2
    } else {
        px * a + py * b - r1
    }
}

/// A toe's crest swept in plan from `a` to `b`: the section is an uneven capsule, crest round on top, keel below.
fn ridge(q: P3, a: P3, b: P3, ra: f64, rb: f64) -> f64 {
    let (ax, aw, bx, bw) = (a[0], a[2], b[0], b[2]);
    let (dx, dw) = (bx - ax, bw - aw);
    let t = (((q[0] - ax) * dx + (q[2] - aw) * dw) / (dx * dx + dw * dw).max(1e-12)).clamp(0.0, 1.0);
    let c = [ax + dx * t, a[1] + (b[1] - a[1]) * t, aw + dw * t];
    let r = ra + (rb - ra) * t;
    let u = (q[0] - c[0]).hypot(q[2] - c[2]);
    let v = q[1] - c[1];
    let d = RIDGE_KEEL_MM;
    uneven_capsule(u, v + d, r + RIDGE_FLARE * d, r, d)
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
            // Cut at the root's centre across the axis, so a buried root never shows through a thin flank's far side.
            Shape::Thorn { a, b, ra, rb, n } => round_cone(p, a, b, ra, rb).max(-dot(sub(p, a), n)),
            Shape::Ball { c, r } => dot(sub(p, c), sub(p, c)).sqrt() - r,
            Shape::Ridge { a, b, ra, rb } => ridge(q, a, b, ra, rb),
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
            Shape::Thorn { a, b, ra, rb, .. } => vec![(a, ra), (b, rb)],
            Shape::Ball { c, r } => vec![(c, r)],
            Shape::Ridge { a, b, ra, rb } => {
                let keel = RIDGE_KEEL_MM * (1.0 + RIDGE_FLARE);
                vec![(frame.world(a), ra + keel), (frame.world(b), rb + keel)]
            }
        };
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for (c, r) in &pts {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k] - r - pad);
                hi[k] = hi[k].max(c[k] + r + pad);
            }
        }
        if let Shape::Limb { .. } | Shape::Egg { .. } | Shape::Ridge { .. } = self.shape {
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
        let skin_f = f;
        if let Some(list) = self.grid.get(&self.key(p)) {
            for &i in list {
                let t = &self.thorns[i as usize];
                f = smin(f, t.eval(q, p), t.blend);
            }
        }
        // The skin: a close field of pebbled tubercles with grooves between them, running up into each cone's fillet so
        // no cone stands on a smooth dome.
        if f.abs() < 0.5 {
            // Faded off the toes, so each stays a clean finger, out of the crease where a limb leaves the body, and off
            // the cones themselves.
            let near = |k: Kind| self.body.iter().filter(|b| b.kind == k).map(|b| b.eval(q, p)).fold(f64::MAX, f64::min);
            let (toe, limb, trunk) = (near(Kind::Toe).min(near(Kind::Claw)), near(Kind::Limb), near(Kind::Body));
            let crease = ((limb.max(trunk) - 0.15) / 0.5).clamp(0.0, 1.0);
            // Up into each cone's fillet, so no cone stands on a smooth dome, and off the cone's flank above it.
            let on_cone = ((0.42 - skin_f) / 0.22).clamp(0.0, 1.0);
            // The head carries a mosaic of flat, angular scales instead of round tubercles.
            let on_head = ((q[0] - 6.8) / 0.5).clamp(0.0, 1.0);
            let fade = ((toe - 0.25) / 0.6).clamp(0.0, 1.0) * crease * on_cone;
            // Finer and lower on the slender limbs, and gone by the wrist, so no knob stands where the toes begin.
            let on_limb = ((trunk - limb) / 0.4).clamp(0.0, 1.0);
            let wrist = ((toe - 0.3) / 0.9).clamp(0.0, 1.0);
            let round = (1.0 - on_limb) * TUBERCLE_MM * tubercles(p, TUBERCLE_PITCH_MM, TUBERCLE_GROOVE_MM, 0.45) + on_limb * wrist * 0.07 * tubercles(p, 0.45, 0.1, 0.45);
            let skin = (1.0 - on_head) * round + on_head * HEAD_SCALE_MM * tubercles(p, HEAD_SCALE_PITCH_MM, 0.1, 0.0);
            f -= fade * skin;
        }
        f.max(self.bore_r + BORE_CLEAR_MM - p[0].hypot(p[1]))
    }
    /// Whether a new cone stands free: its point clear of everything already there, and every point already there
    /// clear of it, by `gap`.
    fn fits(&self, t: &Prim, gap: f64) -> bool {
        let Shape::Thorn { a, b, ra, rb, n } = t.shape else { return true };
        // Nothing on the steep ends of the body, where the skin faces along the ring.
        let radial = unit([foot_of(a, n, ra)[0], foot_of(a, n, ra)[1], 0.0]);
        if dot(n, radial) < 0.4 {
            return false;
        }
        // No crevice under a leaning cone: its flank stands clear of the skin four fifths of the way up.
        let foot = add(a, mul(n, ROOT_SINK * ra));
        let m = add(foot, mul(sub(b, foot), 0.8));
        let along = (dot(sub(m, a), sub(m, a)) / dot(sub(b, a), sub(b, a)).max(1e-12)).sqrt().min(1.0);
        if self.body_at(self.frame.local(m), m) - (ra + (rb - ra) * along) < 0.05 {
            return false;
        }
        if self.field(b) < gap {
            return false;
        }
        // The root must sit in thick body, not on a thin rim.
        let under = sub(a, mul(unit(sub(b, a)), 0.35));
        if self.body_at(self.frame.local(under), under) > -0.05 {
            return false;
        }
        self.thorns.iter().all(|o| match o.shape {
            Shape::Thorn { b: tip, .. } => t.eval(self.frame.local(tip), tip) > gap,
            _ => true,
        })
    }
    /// Whether a new cone's point is clear of every cone already there, and theirs of it, by `gap`.
    fn clear_of_thorns(&self, t: &Prim, gap: f64) -> bool {
        let Shape::Thorn { b, .. } = t.shape else { return true };
        let q = self.frame.local(b);
        self.thorns.iter().all(|o| match o.shape {
            Shape::Thorn { b: tip, .. } => o.eval(q, b) > gap && t.eval(self.frame.local(tip), tip) > gap,
            _ => true,
        })
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
const TAIL_FROM: f64 = -7.4;
const TAIL_LEN: f64 = 15.5;
fn tail_at(t: f64) -> (P3, f64) {
    let x = TAIL_FROM - TAIL_LEN * t;
    let r = 1.4 * (1.0 - t).powf(0.9) + 0.45 * t;
    ([x, 0.1 * r - 0.05, 0.15 * (2.4 * t).sin()], r)
}

/// The tail's section: its height as a share of its width, a flattened oval lying down on the crown.
const TAIL_FLAT: f64 = 0.72;

/// A limb's radius at the shoulder, the elbow and the wrist, mm.
const LIMB_R: (f64, f64, f64) = (0.62, 0.5, 0.42);
/// The toes, from the inner to the outer, as each foot reaches: heading from the limb's own reach (degrees, positive
/// outward) and the crest's length before the claw, mm (the claw adds `CLAW_MM`). The middle three are the longest.
const FORE_TOES: [(f64, f64); 5] = [(-38.0, 0.8), (-10.0, 1.0), (17.0, 1.3), (43.0, 1.15), (69.0, 0.75)];
/// The hind feet lie between the tail and the band's edge: a narrower fan reaching back, the fourth toe the longest.
const HIND_TOES: [(f64, f64); 5] = [(-10.0, 0.7), (10.0, 1.05), (31.0, 1.15), (52.0, 0.95), (78.0, 0.6)];
/// Where a toe leaves the wrist, how far it bends away from the fan's middle over its last half (each step), and how near the
/// band's edge any of it may come, mm and degrees.
const TOE_ROOT_MM: f64 = 0.5;
const TOE_BEND_DEG: f64 = 8.0;
const TOE_EDGE_MM: f64 = 0.9;
/// The shortest a toe's crest may be before its claw, mm.
const TOE_MIN_MM: f64 = 0.35;
/// A toe's crest radius at its root, knuckle, claw base and claw point, and the crest's centre against the crown there:
/// at the crown along the toe, then the claw dips into the sand.
const TOE_CREST_R: [f64; 3] = [0.34, 0.3, 0.24];
const TOE_DROP_MM: [f64; 4] = [0.0, 0.0, 0.06, -0.02];
/// A claw's radius at its base and its point, mm.
const CLAW_R: (f64, f64) = (0.2, 0.05);
/// How far a claw reaches past its toe's crest centre, mm: about half of it shows past the crest's round end.
const CLAW_MM: f64 = 0.75;

/// Front and hind legs: where each shoulder stands along the body, and which way the limb reaches.
const LEGS: [(f64, f64); 2] = [(3.4, 1.0), (-4.6, -1.0)];

/// A leg's shoulder, elbow and wrist in the frame: out past the body's edge, over the band's edge and down its cheek.
fn leg_joints(crest: &Crest, x0: f64, dir: f64, s: f64) -> (P3, P3, P3) {
    let (xe, xw) = (x0 + dir * 1.5, x0 + dir * if dir > 0.0 { 2.9 } else { 2.5 });
    // The hind hands rest a little further in from the edge, room for the outer toes between the tail and the edge.
    let ww = crest.half_w_at_x(xw) - if dir < 0.0 { FOOT_IN_MM + 0.2 } else { FOOT_IN_MM };
    ([x0, 0.6, 3.0 * s], [xe, 1.15, (crest.half_w_at_x(xe) - 0.95) * s], [xw, crest.top_h(xw, ww * s) + 0.3, ww * s])
}

/// How far in from the band's edge each hand rests on the crown, mm.
const FOOT_IN_MM: f64 = 2.2;

/// The hide's tubercles, 0..1: cells of a jittered 3D lattice cut by the skin, each a low dome with a groove along
/// every border between two cells (Worley's second-minus-first distance).
fn tubercles(p: P3, g: f64, groove: f64, dome: f64) -> f64 {
    let c: [i64; 3] = std::array::from_fn(|k| (p[k] / g).floor() as i64);
    let (mut f1, mut f2) = (f64::MAX, f64::MAX);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cell = [c[0] + dx, c[1] + dy, c[2] + dz];
                let key = (cell[0].wrapping_mul(73_856_093) ^ cell[1].wrapping_mul(19_349_663) ^ cell[2].wrapping_mul(83_492_791)) as usize;
                let o: P3 = std::array::from_fn(|k| (cell[k] as f64 + 0.1 + 0.8 * hash(key, 50 + k as u64)) * g);
                let d = dot(sub(p, o), sub(p, o)).sqrt();
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    let t = ((f2 - f1) / groove).clamp(0.0, 1.0);
    let plateau = t * t * (3.0 - 2.0 * t);
    plateau * (1.0 - dome * (f1 / (0.6 * g)).min(1.0).powi(2))
}

/// The lizard's soft body: trunk, false head, neck, head and brows, four legs with their toes, and the tail.
fn body_prims(crest: &Crest) -> Vec<Prim> {
    let egg = |kind, c: P3, r: P3, snout: f64, blend: f64| Prim { kind, shape: Shape::Egg { c, r, snout }, blend };
    let limb = |kind, a: P3, b: P3, ra: f64, rb: f64, blend: f64| Prim { kind, shape: Shape::Limb { a, b, ra, rb }, blend };
    let mut v = vec![
        // The trunk: broad, flat and round, sunk into the crown.
        egg(Kind::Body, [-1.0, 0.35, 0.0], [7.0, 2.0, 3.95], 0.0, 0.0),
        // The false head: a smooth dome on the nape, standing clear of the real head behind a dip.
        egg(Kind::Hump, [5.35, 1.75, 0.0], [1.85, 1.55, 1.8], 0.0, 1.0),
        egg(Kind::Head, [7.5, 0.8, 0.0], [1.2, 1.0, 1.6], 0.0, 0.7),
        // The head: small, low and wedge-shaped, narrowing in plan to a blunt snout.
        egg(Kind::Head, [8.95, 0.7, 0.0], [1.75, 0.82, 1.45], 0.4, 0.6),
        // The brows the horns rise from, blended in so no socket opens beside a horn.
        egg(Kind::Head, [8.75, 1.05, 1.0], [0.85, 0.4, 0.5], 0.0, 0.6),
        egg(Kind::Head, [8.75, 1.05, -1.0], [0.85, 0.4, 0.5], 0.0, 0.6),
        // The snout, blunt and dipped to meet the band.
        egg(Kind::Head, [10.1, 0.45, 0.0], [0.85, 0.55, 0.85], 0.0, 0.8),
    ];
    // What the toes must keep clear of: the trunk, hump, head and tail, without the legs.
    let core: Vec<Prim> = v.iter().copied().chain((0..=26).map(|k| {
        let (c, r) = tail_at(k as f64 / 26.0);
        egg(Kind::Tail, c, [TAIL_LEN / 26.0 + 0.25, TAIL_FLAT * r, r], 0.0, 0.35)
    })).collect();
    // The trunk itself is where the legs come from: a toe keeps clear of the hump, the head and the tail.
    // The tail sits low and wide, so a toe keeps further from it, where a keel could open a crevice under its flank.
    // Under the crown the keel keeps clear of the trunk's sunk underside too.
    let trunk_at = |q: P3| core.iter().filter(|b| b.kind == Kind::Body).map(|b| b.eval(q, [0.0; 3])).fold(f64::MAX, f64::min);
    let core_at = |q: P3| core.iter().filter(|b| b.kind != Kind::Body).map(|b| b.eval(q, [0.0; 3]) - if b.kind == Kind::Tail { 0.0 } else { -0.3 }).fold(f64::MAX, f64::min);
    // Four legs: a limb tapering from the flank over the band's edge onto the crown, ending in a thorny devil's foot:
    // five slender, slightly curved toes lying on the crown, each a keeled crest ending in a claw that dips into the sand.
    for s in [1.0, -1.0] {
        for (x0, dir) in LEGS {
            let (shoulder, elbow, wrist) = leg_joints(crest, x0, dir, s);
            v.push(limb(Kind::Limb, shoulder, elbow, LIMB_R.0, LIMB_R.1, 0.7));
            v.push(limb(Kind::Limb, elbow, wrist, LIMB_R.1, LIMB_R.2, 0.5));
            // A sole buried in the band under the wrist, knitting the toes' keels to the limb and the trunk's sunk
            // underside, so no crevice opens between them under the crown. Its top stays 0.25 mm under the sand.
            v.push(egg(Kind::Limb, [wrist[0], crest.top_h(wrist[0], wrist[2]) - 0.6, wrist[2]], [1.0, 0.35, 0.9], 0.0, 0.3));
            let fan = if dir > 0.0 { FORE_TOES } else { HIND_TOES };
            let centre = 0.5 * (fan[0].0 + fan[4].0);
            // The claws already laid on this foot, sampled along their length.
            let mut siblings: Vec<(f64, f64)> = Vec::new();
            for (ang, len) in fan {
                // Each toe curves gently away from the middle of the fan, so neighbouring claws part instead of crossing.
                // The hind fan is narrow between the tail and the edge: its toes run straight.
                let bend = if dir < 0.0 { 0.0 } else if ang >= centre { -TOE_BEND_DEG } else { TOE_BEND_DEG };
                let heading = |deg: f64| {
                    let a = deg.to_radians();
                    (dir * a.cos(), s * a.sin())
                };
                // Root, knuckle, the claw's base and its point.
                let path = |ang: f64, len: f64| {
                    let (hx, hw) = heading(ang);
                    let root = (wrist[0] + hx * TOE_ROOT_MM, wrist[2] + hw * TOE_ROOT_MM);
                    let mut pts = vec![root];
                    // The claw reaches past the round end of the toe's crest; a short toe has a shorter claw.
                    let claw = CLAW_MM.min(0.35 + 0.5 * len);
                    for (share, extra, bend) in [(0.55, 0.0, 0.0), (0.45, 0.0, bend), (0.0, claw, 2.0 * bend)] {
                        let (px, pw) = *pts.last().unwrap();
                        let (hx, hw) = heading(ang + bend);
                        let step = share * len + extra;
                        pts.push((px + hx * step, pw + hw * step));
                    }
                    pts
                };
                let fits = |pts: &[(f64, f64)]| {
                    let on_crown = pts.iter().all(|&(x, w)| crest.half_w_at_x(x) - w.abs() >= TOE_EDGE_MM);
                    // Clear at the crown, and the keel clear under it, so no thin crevice opens between them.
                    let along: Vec<(f64, f64)> = (3..=12).map(|k| {
                        let f = k as f64 / 12.0 * 3.0;
                        let (i, t) = ((f.floor() as usize).min(2), f - (f.floor()).min(2.0));
                        (pts[i].0 + (pts[i + 1].0 - pts[i].0) * t, pts[i].1 + (pts[i + 1].1 - pts[i].1) * t)
                    }).collect();
                    on_crown && along.iter().all(|&(x, w)| {
                        let h = crest.top_h(x, w);
                        // Clear of its own forearm too, so no wedge opens under the limb.
                        ((x - wrist[0]).hypot(w - wrist[2]) < 0.9 || round_cone([x, h, w], elbow, wrist, LIMB_R.1, LIMB_R.2) > 0.45) &&
                        core_at([x, h, w]) > 0.95 && core_at([x, h - 0.4, w]) > 0.75 && core_at([x, h - RIDGE_KEEL_MM, w]) > 0.95
                            && trunk_at([x, h, w]) > 0.75 && trunk_at([x, h - 0.4, w]) > 0.6 && trunk_at([x, h - RIDGE_KEEL_MM, w]) > 0.75
                    }) && along[6..].iter().all(|&(x, w)| siblings.iter().all(|&(sx, sw)| (x - sx).hypot(w - sw) > 0.45))
                };
                // Shortened until it stays on the crown, its keel inside the cheek and clear of the body; a toe that
                // cannot fit at its shortest turns a few degrees and tries again.
                let mut found = None;
                'turn: for turn in [0.0, 6.0, -6.0, 12.0, -12.0, 18.0, -18.0] {
                    let mut l = len;
                    while l > TOE_MIN_MM - 1e-9 {
                        let pts = path(ang + turn, l);
                        if fits(&pts) {
                            found = Some((ang + turn, l, pts));
                            break 'turn;
                        }
                        l -= 0.05;
                    }
                }
                let placed = found.is_some();
                let (ang, len, pts) = found.unwrap_or_else(|| (ang, TOE_MIN_MM, path(ang, TOE_MIN_MM)));
                if std::env::var("MOLOCH_DEBUG").is_ok() {
                    println!("  toe at x {:.1} side {s}: heading {ang}, length {len:.2}, fits {placed}", x0);
                }
                siblings.extend((9..=12).map(|k| {
                    let f = k as f64 / 12.0 * 3.0;
                    let (i, t) = ((f.floor() as usize).min(2), f - (f.floor()).min(2.0));
                    (pts[i].0 + (pts[i + 1].0 - pts[i].0) * t, pts[i].1 + (pts[i + 1].1 - pts[i].1) * t)
                }));
                let at = |i: usize| {
                    let (x, w) = pts[i];
                    [x, crest.top_h(x, w) + TOE_DROP_MM[i], w]
                };
                // The inner toes leave the wrist beside the trunk: a fuller root fillet there closes the slit between them.
                for k in 0..2 {
                    v.push(Prim { kind: Kind::Toe, shape: Shape::Ridge { a: at(k), b: at(k + 1), ra: TOE_CREST_R[k], rb: TOE_CREST_R[k + 1] }, blend: if k > 0 { 0.1 } else if ang < centre { 0.45 } else { 0.2 } });
                }
                // The claw: a short pointed cone from the toe's end, narrower than the toe, its point pressed into the sand.
                v.push(limb(Kind::Claw, at(2), at(3), CLAW_R.0, CLAW_R.1, 0.18));
            }
        }
    }
    // The tail: from the vent down the crest toward the palm, a flattened oval lying down on the crown and tapering,
    // a chain of blended eggs.
    let n = 26;
    for k in 0..=n {
        let (c, r) = tail_at(k as f64 / n as f64);
        let seg = TAIL_LEN / n as f64;
        v.push(egg(Kind::Tail, c, [seg.max(0.6 * r) + 0.25, TAIL_FLAT * r, r], 0.0, if k == 0 { 1.1 } else { 0.35 }));
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

/// The thorns placed by hand: the false head's two spines, the paired shoulder and hip spines, and the paired rows
/// down the back and flanks, graded from the shoulders to the hips.
fn major_plans() -> Vec<Plan> {
    let mut out = Vec::new();
    let pair = |out: &mut Vec<Plan>, x: f64, w: f64, len: f64, kind: Kind, splay: f64| {
        out.push(Plan { x, w, len, kind, splay });
        out.push(Plan { x, w: -w, len, kind, splay: -splay });
    };
    pair(&mut out, 5.4, 0.72, 1.75, Kind::HumpSpine, 0.4);
    // Small cones along each brow, behind the horn.
    for (x, w) in [(8.3, 1.15), (7.8, 1.2), (7.35, 1.25)] {
        pair(&mut out, x, w, 0.45, Kind::Minor, 0.6);
    }
    // The great shoulder and hip spines.
    pair(&mut out, 1.9, 2.1, 1.8, Kind::Major, 0.45);
    pair(&mut out, -4.3, 2.0, 1.0, Kind::Major, 0.1);
    for (x, len) in [(3.4, 1.3), (0.2, 1.45), (-2.2, 1.3), (-5.4, 0.8)] {
        pair(&mut out, x, 0.8, len, Kind::Major, 0.12);
    }
    for (x, len) in [(-0.9, 1.35), (-2.6, 1.2)] {
        pair(&mut out, x, 2.55, len, Kind::Major, 0.15);
    }
    // The flank row stands up off the flank, not out over the cheek.
    for (x, len) in [(0.6, 0.9), (-1.9, 0.85), (-5.2, 0.55)] {
        pair(&mut out, x, 3.45, len, Kind::Major, 0.2);
    }
    out
}

/// A cone rooted at world `p` on a surface of normal `n`, leaned back toward the tail and out by `splay`.
fn cone(p: P3, n: P3, len: f64, root_r: f64, kind: Kind, splay: f64, lean_deg: f64) -> Prim {
    let theta = p[1].atan2(p[0]);
    // Toward the tail: decreasing ring angle.
    let back = [theta.sin(), -theta.cos(), 0.0];
    let dir = unit(add(add(n, mul(back, lean_deg.to_radians().tan())), [0.0, 0.0, splay * 0.5]));
    Prim { kind, shape: Shape::Thorn { a: sub(p, mul(n, ROOT_SINK * root_r)), b: add(p, mul(dir, len)), ra: root_r, rb: TIP_MM, n }, blend: THORN_BLEND_MM }
}

/// Broad-based cones: the root's radius as a share of the length, so a cone is never taller than it is wide.
fn root_of(kind: Kind, len: f64) -> f64 {
    match kind {
        Kind::Minor => 0.62 * len,
        Kind::TailThorn => 0.6 * len,
        Kind::HumpSpine => 0.36 * len,
        Kind::Horn => 0.5 * len,
        _ => 0.55 * len,
    }
}

/// Strike one thorn at a frame plan point on the body's top, or at a given frame surface point.
fn thorn_at(liz: &Lizard, x: f64, w: f64, len: f64, kind: Kind, splay: f64, surface: Option<P3>) -> Option<(Prim, P3, f64)> {
    let q = surface.or_else(|| liz.top(x, w))?;
    let p = liz.frame.world(q);
    let r = root_of(kind, len);
    let mut c = cone(p, liz.normal(p), len, r, kind, splay, LEAN_DEG);
    if matches!(kind, Kind::Major | Kind::HumpSpine) {
        c.blend = 0.28;
    }
    Some((c, p, r))
}

/// A tail thorn rooted at frame surface point `q`, a share `t` down the tail: its axis lies up and back along the tail,
/// `TAIL_THORN_OFF_DEG` off the tail's own line, so it never points out past the band's edge.
fn tail_thorn(liz: &Lizard, q: P3, t: f64, len: f64) -> Option<(Prim, P3, f64)> {
    let p = liz.frame.world(q);
    let n = liz.normal(p);
    let (c0, _) = tail_at(t);
    let (c1, _) = tail_at((t + 0.02).min(1.0));
    let along = unit(sub(liz.frame.world(c1), liz.frame.world(c0)));
    let up = unit([p[0], p[1], 0.0]);
    let off = TAIL_THORN_OFF_DEG.to_radians();
    let dir = unit(add(mul(along, off.cos()), mul(up, off.sin())));
    let r = root_of(Kind::TailThorn, len);
    Some((Prim { kind: Kind::TailThorn, shape: Shape::Thorn { a: sub(p, mul(n, ROOT_SINK * r)), b: add(p, mul(dir, len)), ra: r, rb: TIP_MM, n }, blend: THORN_BLEND_MM }, p, r))
}

/// How far a tail thorn rises off the tail's line, degrees.
const TAIL_THORN_OFF_DEG: f64 = 22.0;

/// A horn bent back over the brow: a stout cone up and out, then a second leaning back to the point.
fn horn(liz: &Lizard, x: f64, w: f64, len: f64) -> Option<[Prim; 2]> {
    let q = liz.top(x, w)?;
    let p = liz.frame.world(q);
    let n = liz.normal(p);
    let theta = p[1].atan2(p[0]);
    let back = [theta.sin(), -theta.cos(), 0.0];
    let out = [0.0, 0.0, w.signum()];
    // Up off the ring, not off the head's steep flank, so neither horn juts sideways like a snout cone.
    let up = unit([p[0], p[1], 0.0]);
    let d1 = unit(add(add(up, mul(back, 0.45)), mul(out, 0.22)));
    let knee = add(p, mul(d1, 0.55 * len));
    let d2 = unit(add(add(up, mul(back, 1.3)), mul(out, 0.18)));
    let tip = add(knee, mul(d2, 0.55 * len));
    let root = 0.42 * len;
    Some([
        Prim { kind: Kind::Horn, shape: Shape::Thorn { a: sub(p, mul(n, ROOT_SINK * root)), b: knee, ra: root, rb: 0.5 * root, n }, blend: 0.4 },
        Prim { kind: Kind::Horn, shape: Shape::Thorn { a: knee, b: tip, ra: 0.5 * root, rb: TIP_MM, n: d1 }, blend: 0.1 },
    ])
}

/// Where a cone's root meets the skin.
fn foot_of(a: P3, n: P3, ra: f64) -> P3 {
    add(a, mul(n, ROOT_SINK * ra))
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
    band_hide: Option<(String, [f64; 2], u32, u32)>,
    decimation: (usize, f64),
    /// The furthest the sculpt stands past a cheek of the band, mm.
    past_cheek_mm: f64,
    /// Cones dropped where the decimated sculpt crossed itself.
    dropped: Vec<String>,
}

fn build_lizard<'a>(frame: Frame<'a>, bore_r: f64) -> Lizard<'a> {
    let prims = body_prims(frame.crest);
    let mut liz = Lizard::new(frame, prims, bore_r);
    let mut placed: Vec<(P3, f64)> = Vec::new();
    let clear = |placed: &[(P3, f64)], p: P3, r: f64, gap: f64| placed.iter().all(|(q, rq)| dot(sub(p, *q), sub(p, *q)).sqrt() >= r + rq + gap);
    for s in [1.0, -1.0] {
        if let Some(h) = horn(&liz, 8.9, 1.05 * s, 1.9) {
            if let Shape::Thorn { a, .. } = h[0].shape {
                placed.push((a, 0.75));
            }
            for t in h {
                liz.push_thorn(t);
            }
        }
    }
    for pl in major_plans() {
        if let Some((t, p, r)) = thorn_at(&liz, pl.x, pl.w, pl.len, pl.kind, pl.splay, None) {
            placed.push((p, 0.8 * r));
            liz.push_thorn(t);
        }
    }
    // The tail's whorls: cones over its top, every ring turned half a step, graded to the tip, each lying up and back
    // along the tail, never out over the cheek.
    for k in 0..13 {
        let t = (k as f64 + 0.5) / 13.5;
        let (c, r) = tail_at(t);
        // The last stretch is a bare rounded taper: no thorn stands at the tip.
        if t > 0.8 {
            continue;
        }
        let len = 0.55 - 0.25 * t;
        let turn = if k % 2 == 0 { 0.0 } else { 0.5 };
        for j in [-1.0, 0.0, 1.0] {
            let phi = ((j + turn) * 30.0f64).to_radians();
            if phi.abs() > 0.55 {
                continue;
            }
            let q = [c[0], c[1] + TAIL_FLAT * r * phi.cos(), c[2] + r * phi.sin()];
            if let Some((th, p, rr)) = tail_thorn(&liz, q, t, len) {
                // A thorn lying along the tail sits close over it by design: only its point must stand clear of the
                // thorns already there.
                if clear(&placed, p, rr, 0.12) && liz.clear_of_thorns(&th, 0.2) {
                    placed.push((p, 0.8 * rr));
                    liz.push_thorn(th);
                }
            }
        }
    }
    // The limbs' own thorns: small cones along the top and outer side of each limb.
    for s in [1.0, -1.0] {
        for (x0, dir) in LEGS {
            let (sh, el, wr) = leg_joints(liz.frame.crest, x0, dir, s);
            for (from, to, ra, rb, t, len) in [(sh, el, LIMB_R.0, LIMB_R.1, 0.4, 0.42), (el, wr, LIMB_R.1, LIMB_R.2, 0.4, 0.36)] {
                let c: P3 = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t);
                let r = ra + (rb - ra) * t;
                let q = add(c, mul(unit([0.0, 0.9, 0.35 * s]), r));
                if let Some((th, p, rr)) = thorn_at(&liz, 0.0, 0.0, len, Kind::Minor, -0.3 * s, Some(q)) {
                    if clear(&placed, p, rr, 0.1) && liz.fits(&th, 0.3) {
                        placed.push((p, 0.8 * rr));
                        liz.push_thorn(th);
                    }
                }
            }
        }
    }
    // Minor thorns over whatever the majors leave, off the legs: a jittered lattice over the body's plan, graded
    // smaller to the flanks, each with a land round it; then a tier of small knobs in the lands left over.
    for (tier, pitch, gap, base) in [(0u64, 0.66, 0.3, 0.85), (1, 0.42, 0.14, 0.5)] {
        let mut cands: Vec<(f64, f64, usize)> = Vec::new();
        let mut k = 0usize;
        let mut x = 8.2;
        while x > -8.4 {
            let mut w = -4.4;
            while w < 4.4 {
                cands.push((x + 0.5 * pitch * (hash(k, 1 + 10 * tier) - 0.5), w + 0.5 * pitch * (hash(k, 2 + 10 * tier) - 0.5), k));
                k += 1;
                w += pitch;
            }
            x -= pitch;
        }
        cands.sort_by_key(|c| (hash(c.2, 3 + 10 * tier) * 1e12) as u64);
        for (x, w, k) in cands {
            // The false head and the head stay smooth domes, crowned only by their own spines and horns.
            if x > 3.9 {
                continue;
            }
            let Some(q) = liz.top(x, w) else { continue };
            if q[1] < -0.1 {
                continue;
            }
            let p = liz.frame.world(q);
            let near_leg = liz.body.iter().filter(|b| matches!(b.kind, Kind::Limb | Kind::Toe | Kind::Claw)).map(|b| b.eval(q, p)).fold(f64::MAX, f64::min);
            if near_leg < 0.35 {
                continue;
            }
            let edge = (w.abs() / 4.0).min(1.0);
            let len = (base - 0.3 * base * edge) * (0.8 + 0.3 * hash(k, 4 + 10 * tier));
            let r = root_of(Kind::Minor, len);
            if !clear(&placed, p, 0.8 * r, gap) {
                continue;
            }
            if let Some((th, p, rr)) = thorn_at(&liz, x, w, len, Kind::Minor, 0.3 * w.signum() * edge, Some(q)).filter(|t| liz.fits(&t.0, 0.3)) {
                placed.push((p, 0.8 * rr));
                liz.push_thorn(th);
            }
        }
    }
    // The hide between them: a close field of granules over the body, head, limbs and tail.
    let mut cands: Vec<(f64, f64, usize)> = Vec::new();
    let mut k = 0usize;
    let mut x = 11.6;
    while x > TAIL_FROM - TAIL_LEN - 0.6 {
        let mut w = -4.6;
        while w < 4.6 {
            cands.push((x + 0.3 * (hash(k, 31) - 0.5), w + 0.3 * (hash(k, 32) - 0.5), k));
            k += 1;
            w += 0.3;
        }
        x -= 0.3;
    }
    cands.sort_by_key(|c| (hash(c.2, 33) * 1e12) as u64);
    let mut why = [0usize; 3];
    // The tubercle skin replaced the loose granules of round 2.
    if !BALL_GRANULES {
        cands.clear();
    }
    for (x, w, k) in cands {
        // The head stays smooth: granules there read as eyes.
        if x > 7.4 {
            continue;
        }
        let Some(q) = liz.top(x, w) else { why[0] += 1; continue };
        if q[1] < -0.3 {
            why[1] += 1;
            continue;
        }
        let p = liz.frame.world(q);
        let r = GRANULE_R_MM * (0.85 + 0.3 * hash(k, 34));
        let base = (r * r - (r - GRANULE_MM) * (r - GRANULE_MM)).sqrt();
        if !clear(&placed, p, base, 0.03) {
            why[2] += 1;
            continue;
        }
        let n = liz.normal(p);
        placed.push((p, base));
        liz.push_thorn(Prim { kind: Kind::Granule, shape: Shape::Ball { c: sub(p, mul(n, r - GRANULE_MM)), r }, blend: 0.08 });
    }
    if std::env::var("MOLOCH_DEBUG").is_ok() {
        println!("  granule rejects: no top {}, low {}, crowded {}", why[0], why[1], why[2]);
    }
    liz
}

/// The sculpt: meshed, relaxed, decimated to budget and settled.
fn sculpt_solid(liz: &mut Lizard, comp: &mut Composition) -> csg::Solid {
    let t = std::time::Instant::now();
    for attempt in 0.. {
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
        let mut raw = sculpt::tetra_mesh(lo, hi, STEP_MM, &field);
        comp.sculpt_raw_faces = raw.f.len();
        // Relaxing can fold a thin crease through itself: where it does, put back the unrelaxed vertices round each
        // crossing and try again, widening the patch.
        let unrelaxed = raw.clone();
        sculpt::relax(&mut raw, &field, 3);
        for reach in [0.3, 0.6, 1.2, 1e9] {
            let sites = sculpt::crossing_sites(&raw);
            if sites.is_empty() {
                break;
            }
            comp.dropped.push(format!("relax undone within {reach} mm of {} crossings", sites.len()));
            for (v, orig) in raw.v.iter_mut().zip(&unrelaxed.v) {
                if sites.iter().any(|s| dot(sub(*v, *s), sub(*v, *s)) < reach * reach) {
                    *v = *orig;
                }
            }
        }
        // Decimate to the budget; where that crosses itself, drop the cone at the crossing and mesh again.
        let mut tried = None;
        let mut sites = Vec::new();
        for (k, cap) in [2e-3, 1e-3, 5e-4].into_iter().enumerate() {
            let d = sculpt::decimate(&raw, FACES, cap, 2.0 + k as f64, 18.0, 35.0);
            let here = sculpt::crossing_sites(&d);
            if here.is_empty() {
                comp.decimation = (FACES, cap);
                tried = Some(d);
                break;
            }
            if k == 0 {
                sites = here;
            }
        }
        let nets = if let Some(d) = tried {
            d
        } else if attempt < 3 {
            let p = sites[0];
            let q = liz.frame.local(p);
            // The nearest small cone or granule there goes first; a hand-placed thorn only when nothing else is near.
            let mut near: Vec<(f64, usize)> = (0..liz.thorns.len()).map(|i| (liz.thorns[i].eval(q, p).abs(), i)).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let small = |k: Kind| matches!(k, Kind::Minor | Kind::Granule | Kind::TailThorn);
            let worst = near.iter().find(|(d, i)| *d < 0.6 && small(liz.thorns[*i].kind)).or(near.first().filter(|(d, _)| *d < 0.4)).map(|x| x.1);
            if worst.is_none() {
                println!("  crossing without a cone at x {:.2} h {:.2} w {:.2}", q[0], q[1], q[2]);
                comp.dropped.push(format!("no cone at the crossing at x {:.2} h {:.2} w {:.2}; decimated with backed-off caps", q[0], q[1], q[2]));
                let mut done = None;
                'caps: for target in [FACES + FACES / 10, FACES + FACES / 4] {
                    for cap in [1e-3, 5e-4, 2e-4] {
                        let d = sculpt::decimate(&raw, target, cap, 3.0, 18.0, 35.0);
                        if csg::self_crossings(&d) == 0 {
                            comp.decimation = (target, cap);
                            done = Some(d);
                            break 'caps;
                        }
                    }
                }
                let nets = done.unwrap_or_else(|| sculpt::clean_decimate(&raw, FACES));
                let s = sculpt::settle(nets, &field, &|_| false);
                comp.sculpt_faces = s.f.len();
                comp.sculpt_volume_mm3 = sculpt::closure(&s).1;
                comp.sculpt_s = t.elapsed().as_secs_f64();
                return s;
            }
            if let Some(i) = worst {
                let gone = liz.thorns[i];
                let fl = |w: P3| liz.frame.local(w);
                let (ga, gb) = if let Shape::Thorn { a, b, .. } = gone.shape { (fl(a), fl(b)) } else { ([0.0; 3], [0.0; 3]) };
                comp.dropped.push(format!("{} at {:.1} deg (crossing at x {:.2} h {:.2} w {:.2}; cone from x {:.2} h {:.2} w {:.2} to x {:.2} h {:.2} w {:.2})", gone.kind.label(), p[1].atan2(p[0]).to_degrees(), q[0], q[1], q[2], ga[0], ga[1], ga[2], gb[0], gb[1], gb[2]));
                let mut rest = std::mem::take(&mut liz.thorns);
                rest.remove(i);
                liz.grid.clear();
                for t in rest {
                    liz.push_thorn(t);
                }
            }
            continue;
        } else {
            sculpt::clean_decimate(&raw, FACES)
        };
        let s = sculpt::settle(nets, &field, &|_| false);
        comp.sculpt_faces = s.f.len();
        comp.sculpt_volume_mm3 = sculpt::closure(&s).1;
        comp.sculpt_s = t.elapsed().as_secs_f64();
        return s;
    }
    unreachable!()
}

/// Wind ripples in sand, as one SVG tile `w` by `h` mm of `RIPPLE_TILES` round the crown.
///
/// Every crest is one stretch of a single line `s`, with crest `i` running `s` from `i` to `i + 1` across the tile, so
/// it runs on into crest `i + 1` of the next tile and the ripples wind round the ring at a shallow angle. Everything
/// about a crest is a smooth function of `s`, so it is seamless at the joins: where it lies (the spacing to the next
/// crest wanders between 1.0 and 1.8 mm), how it wavers, and how tall it stands (some crests sink away and pick up
/// again). The profile is a dune ripple's: a long gentle windward slope, a rounded crest and a short steep lee, drawn
/// as nested bands of ink so the height climbs in fine even steps under a light blur.
fn ripples_svg(w: f64, h: f64) -> String {
    use std::fmt::Write;
    let tau = std::f64::consts::TAU;
    // The spacing to the next crest, and the crest line's place across the tile at `s`, mm.
    let spacing = |s: f64| 1.7 + 0.18 * (tau * 0.37 * s + 0.4).sin() + 0.06 * (tau * 1.13 * s + 2.2).sin();
    let place = |s: f64| {
        // The integral of the spacing, so neighbouring crests sit one spacing apart.
        1.7 * s - 0.18 / (tau * 0.37) * ((tau * 0.37 * s + 0.4).cos() - 0.4f64.cos()) - 0.06 / (tau * 1.13) * ((tau * 1.13 * s + 2.2).cos() - 2.2f64.cos())
    };
    // Close to whole cycles per crest, so neighbouring crests sway nearly in step: one coherent sinuous field, as
    // wind ripples run, rather than lines drifting across each other like grain.
    let waver = |s: f64| 0.16 * (tau * 2.06 * s + 0.3).sin() + 0.05 * (tau * 1.03 * s + 1.7).sin();
    // The crest's height, 0..1: it rises and falls along the crest, and sinks away where the crest breaks.
    let breaks = [0.31, 2.47, 4.36, 6.94];
    let tall = |s: f64| {
        let base = 0.86 + 0.12 * (tau * 0.83 * s + 1.1).sin();
        let gap = breaks.iter().map(|&b| { let d = ((s - b) / 0.075).powi(2); (-d).exp() }).fold(0.0, f64::max);
        (base * (1.0 - gap)).clamp(0.0, 1.0)
    };
    // Crest i covers s in [i, i + 1]; the tile shows every crest whose line crosses it.
    let s_of = |y: f64| {
        // place() is increasing: find s with place(s) = y by bisection.
        let (mut lo, mut hi) = (-20.0, 40.0);
        for _ in 0..60 {
            let m = 0.5 * (lo + hi);
            if place(m) < y { lo = m } else { hi = m }
        }
        lo
    };
    let (i0, i1) = ((s_of(-2.5) - 1.0).floor() as i64, (s_of(h + 2.5) + 1.0).ceil() as i64);
    const LEVELS: usize = 14;
    const STEP_MM: f64 = 0.1;
    let mut body = String::new();
    // Each crest as nested bands: level j covers where the profile stands at least (j - 0.5) / LEVELS of the crest, and
    // its opacity makes the stack's ink exactly j / LEVELS there.
    for i in i0..=i1 {
        for j in 1..=LEVELS {
            let level = (j as f64 - 0.5) / LEVELS as f64;
            let opacity = 1.0 / (LEVELS - j + 1) as f64;
            let mut runs: Vec<Vec<(f64, f64, f64)>> = vec![Vec::new()];
            let mut u = -0.2;
            while u <= w + 0.2 {
                let s = i as f64 + u / w;
                let (y, gap, a) = (place(s) + waver(s), spacing(s), tall(s));
                if a > level {
                    // Windward reach on the low side, lee on the high side: height = a (1 - (d / reach)^p).
                    let share = 1.0 - level / a;
                    let wind = 0.62 * gap * share.powf(1.0 / 1.7);
                    let lee = 0.14 * gap * share.powf(1.0 / 1.6);
                    runs.last_mut().unwrap().push((u, y - wind, y + lee));
                } else if !runs.last().unwrap().is_empty() {
                    runs.push(Vec::new());
                }
                u += STEP_MM;
            }
            for run in runs.iter().filter(|r| r.len() > 1) {
                let mut d = String::new();
                for (k, (u, top, _)) in run.iter().enumerate() {
                    let _ = write!(d, "{}{u:.2} {top:.2}", if k == 0 { "M" } else { " L" });
                }
                for (u, _, bottom) in run.iter().rev() {
                    let _ = write!(d, " L{u:.2} {bottom:.2}");
                }
                let _ = write!(body, r##"<path d="{d}Z" fill="#000" fill-opacity="{opacity:.4}"/>"##);
            }
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs><filter id="soft" x="-0.1" y="-0.1" width="1.2" height="1.2"><feGaussianBlur stdDeviation="0.045"/></filter></defs><g filter="url(#soft)">{body}</g></svg>"##
    )
}

/// The band's own ground: the desert the devil lies on, wind ripples in sand winding round the crown and running on
/// under the lizard, so it lies in the ground; the cheeks stay polished.
fn band_hide(d: &mut RingDesign, lib: &mut AlphaLibrary, comp: &mut Composition, art: &Path) -> Result<()> {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Sand ripples", &ctx);
    let faces = ctx.side_faces_std();
    let (lo, hi) = faces.and_then(|f| Some((f.low?.1, f.high?.0))).unwrap_or((0.0, ctx.band_v_len_mm));
    t.v_center_mm = 0.5 * (lo + hi);
    t.v_span_mm = hi - lo - 0.3;
    t.repeats_around = RIPPLE_TILES;
    t.rows = 1;
    t.height_mm = RIPPLE_MM;
    t.feather_mm = 0.0;
    let (cw, ch) = t.cell_size(&ctx);
    let svg = ripples_svg(cw, ch);
    std::fs::write(art.join("sand-ripples.svg"), &svg)?;
    d.svgs.push(SvgAlpha { name: "Sand ripples".into(), svg, invert: false });
    let mut e = LayerEntry::new("Sand ripples", Layer::Tiling(t));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    d.bake_all(lib);
    comp.band_hide = Some(("sand ripples".into(), [cw, ch], RIPPLE_TILES, 1));
    Ok(())
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

type Census = Vec<(Kind, f64, f64, P3)>;

fn author(art: &Path) -> Result<(RingDesign, AlphaLibrary, Composition, csg::Solid, Census)> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let crest = Crest::of(&d)?;
    let mut comp = Composition { crest_r_at_face_mm: crest.at(90.0), ..Composition::default() };
    let mut liz = build_lizard(Frame { crest: &crest }, d.inner_radius_mm());
    band_hide(&mut d, &mut lib, &mut comp, &art)?;
    if let Ok(at) = std::env::var("MOLOCH_PROBE_AT") {
        let v: Vec<f64> = at.split(',').filter_map(|x| x.parse().ok()).collect();
        let t = v[0].to_radians();
        let p = [v[1] * t.cos(), v[1] * t.sin(), v[2]];
        let q = liz.frame.local(p);
        println!("  probe at x {:.2} h {:.2} w {:.2}: field {:.3}", q[0], q[1], q[2], liz.field(p));
        let mut near: Vec<(f64, String)> = liz.body.iter().chain(liz.thorns.iter()).map(|s| (s.eval(q, p), format!("{:?} {:?}", s.kind, s.shape))).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for n in near.iter().take(5) {
            println!("    {:.3} {}", n.0, n.1);
        }
    }
    if std::env::var("MOLOCH_CONE_DEBUG").is_ok() {
        for t in liz.thorns.iter().filter(|t| t.kind == Kind::Major).take(3) {
            if let Shape::Thorn { a, b, .. } = t.shape {
                for k in 0..=6 {
                    let p = add(a, mul(sub(b, a), k as f64 / 6.0));
                    println!("  cone {k}: field {:.3} body {:.3} self {:.3}", liz.field(p), liz.body_at(liz.frame.local(p), p), t.eval(liz.frame.local(p), p));
                }
            }
        }
    }
    if std::env::var("MOLOCH_PLAN_ONLY").is_ok() {
        println!("  plan: {} cones", liz.thorns.len());
        std::process::exit(0);
    }
    let solid = sculpt_solid(&mut liz, &mut comp);
    let mut by_kind: std::collections::BTreeMap<String, usize> = Default::default();
    let (mut lmin, mut lmax) = (f64::MAX, 0.0f64);
    for t in &liz.thorns {
        *by_kind.entry(t.kind.label().into()).or_default() += 1;
        if let Shape::Thorn { a, b, ra, .. } = t.shape {
            let l = dot(sub(b, a), sub(b, a)).sqrt() - ROOT_SINK * ra;
            lmin = lmin.min(l);
            lmax = lmax.max(l);
        }
    }
    comp.thorns_by_kind = by_kind.into_iter().collect();
    comp.thorn_lengths_mm = [lmin, lmax];
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
    // How far the sculpt stands past the band's cheeks along the finger.
    comp.past_cheek_mm = solid.v.iter().map(|p| p[2].abs() - Crest::read(&crest.half_w, p[1].atan2(p[0]).to_degrees())).fold(f64::MIN, f64::max);
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
    let census = land_census(&solid, &kinds, &|p| liz.body_at(liz.frame.local(p), p));
    Ok((d, lib, comp, solid, census))
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
/// `skin` is the smooth body's field (no tubercles, no thorns): a face on the body whose ray leaves the metal still
/// within the tubercles' height of the skin crossed a tubercle's own flank, which is relief, not a section.
fn land_census(solid: &csg::Solid, kinds: &[(Kind, P3)], skin: &dyn Fn(P3) -> f64) -> Vec<(Kind, f64, f64, P3)> {
    use ringdesign_core::interaction::bvh::Bvh;
    let m = mesh::Mesh {
        vertices: solid.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces: solid.f.clone(),
        ..Default::default()
    };
    let bvh = Bvh::build(&m);
    const IN: f64 = 1e-4;
    let mut out: std::collections::BTreeMap<u8, (Kind, f64, f64, P3)> = Default::default();
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
        let mut kind = kinds[fi].0;
        let exit = add(o, mul(inward, t));
        if section < MIN_SECTION_MM && matches!(kind, Kind::Body | Kind::Hump | Kind::Head | Kind::Limb | Kind::Tail) && skin(exit) > -(TUBERCLE_MM + 0.12) {
            kind = Kind::Hide;
        }
        let e = out.entry(kind as u8).or_insert((kind, f64::MAX, 0.0, [0.0; 3]));
        if section < e.1 {
            e.1 = section;
            e.3 = kinds[fi].1;
        }
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

/// Where the close-up centres: frame x along the body (between the false head and the head), and its radius, mm.
const HEAD_CLOSE_X: f64 = 5.2;
const HEAD_CLOSE_R: f64 = 12.6;

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

/// The finished mesh as the renders shade it. The sculpt is a tetrahedral meshing of a distance field: along the hide's
/// tubercle edges its triangles zig-zag in folds under 0.05 mm, and a crease normal on each draws it as crumpled foil.
/// Faces standing clear of the bare crown by more than 0.3 mm, above the sand ripples (the lizard's own free surface) are shaded with the
/// mesh's smooth vertex normals; every face at or near the band, where the toes, body and tail meet the sand, keeps its
/// crease normals, so that junction stays crisp. Geometry is untouched: only the shading normals differ.
fn shading_mesh(m: &mesh::Mesh, crest: &Crest) -> mesh::Mesh {
    let mut out = m.clone();
    let free = |i: u32| {
        let v = m.vertices[i as usize];
        let (x, y, z) = (v.0 as f64, v.1 as f64, v.2 as f64);
        crest.crown_r(y.atan2(x).to_degrees(), z).is_some_and(|r| x.hypot(y) > r + 0.3)
    };
    out.corner_normals.retain(|(fi, _)| !m.faces[*fi as usize].iter().all(|&i| free(i)));
    out
}

/// Studio-gold renders, the 300 px read, a contact sheet and the bare band against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let shaded = shading_mesh(&built.mesh, &Crest::of(&band())?);
    let parts = vec![render::Part::metal(&shaded, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The stones view on a ring without stones: the horned head, the false head and a fore foot, close, from over the
    // snout; the whole ring drawn and the camera framed on them, so no cropped edge or lost crease shades the relief.
    let theta = THETA_C + (HEAD_CLOSE_X / R_REF).to_degrees();
    let t = theta.to_radians();
    let centre = [HEAD_CLOSE_R * t.cos(), HEAD_CLOSE_R * t.sin(), 0.0];
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(theta) - 0.45, 0.95, render::Framing::new(centre, 7.0), edge)?;
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
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/cataphracta/art").join(SLUG);
    std::fs::create_dir_all(&art)?;
    let (d, lib, comp, solid, census) = author(&art)?;
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
    let census = census;
    let unnamed: Vec<String> = census.iter().filter(|c| c.2 > 0.0 && c.0.treatment().is_none()).map(|c| format!("{}: {:.3} mm2 under, thinnest {:.2}", c.0.label(), c.2, c.1)).collect();
    let lands = json!({
        "floor_mm": MIN_SECTION_MM,
        "detail_floor_mm": MIN_DETAIL_MM,
        "method": "dfm::part_sections on the sculpted part (one ray per face along its inward normal), and the same per face gathered by the shape nearest each face",
        "part_sections": {"part": "Thorny devil", "thinnest_mm": part_min, "under_floor_mm2": part_under, "area_mm2": solid_area(&solid)},
        "by_kind": census.iter().map(|c| json!({"kind": c.0.label(), "thinnest_mm": c.1, "thinnest_at_deg_r_z": [c.3[1].atan2(c.3[0]).to_degrees(), c.3[0].hypot(c.3[1]), c.3[2]], "under_floor_mm2": c.2, "treatment": if c.2 > 0.0 { c.0.treatment().map(|t| format!("{t}. Measured thinnest section: {:.3} mm.", c.1)) } else { None }})).collect::<Vec<_>>(),
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
