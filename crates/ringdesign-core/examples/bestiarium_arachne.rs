//! Bestiarium — Arachne, the weaver: a spider at the hub of her own orb web, clasping the finger, cast in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_arachne
//! target/release/examples/bestiarium_arachne [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, Blend, BuildParams, FieldContext, Layer, LayerEntry, ProfileLoop, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, MirrorPlane, Operation, PatternKind, Placement, SurfaceKind, stored},
    castability::{self, CastProcess},
    csg, dfm,
    field::{Decal, DecalLayer, SeatPadLayer, SeatStyle, SideFacePick, Uv, VGate},
    gem::{Gem, GemCut, GemForm},
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    setting::{self, SolidKind},
    sketch::{Geometry, Id, Sketch, Workplane},
    stl,
    stones::StoneFrame,
    svg::SvgAlpha,
    tiling::TilingLayer,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::rc::Rc;

type P2 = [f64; 2];
type P3 = [f64; 3];

/// Ring angle of the garnet carapace, which is also the web's hub.
const HUB_DEG: f64 = 64.0;
/// Girdle heights over the bare crest: the carapace, and the prouder abdomen.
const CARAPACE_RISE_MM: f64 = 0.4;
const ABDOMEN_RISE_MM: f64 = 0.6;
/// The collets' wall, which is also the flat of their rim.
const COLLET_WALL_MM: f64 = 0.45;
/// The made collet's clearance round the girdle, as `setting` builds it.
const COLLET_CLEAR_MM: f64 = 0.03;
/// How far the made collet reaches below the girdle.
const COLLET_BASE_MM: f64 = 0.6;
/// Where a collet is trimmed under its girdle, and its body takes over at its widest.
const BODY_FULL_Z: f64 = -0.12;
/// Collet wall to collet wall between carapace and abdomen, at the girdles.
const BODY_GAP_MM: f64 = 1.0;
/// Relief of every web thread.
const WEB_HEIGHT_MM: f64 = 0.30;
/// The orb: its capture spiral's first and last turn, and the reach of its radials.
const SPIRAL_START_MM: f64 = 1.9;
const SPIRAL_PITCH_MM: f64 = 0.9;
const SPIRAL_TURNS: usize = 6;
const RADIALS: usize = 12;
/// The orb's capture rings past its spiral, before the spider and behind her, widening toward the palm.
const FRONT_RINGS: [f64; 7] = [8.6, 10.1, 11.9, 14.0, 16.4, 19.2, 22.4];
const BACK_RINGS: [f64; 6] = [16.4, 19.2, 22.4, 26.0, 30.0, 34.3];
/// Dew beads on the dragline: diameter and pitch.
const DEW_MM: f64 = 0.45;
const DEW_PITCH_MM: f64 = 1.1;
/// Most a knee may stand past the bare cheek, and the ring's half-reach along the finger (its reach at most 8.8 mm).
const KNEE_PROUD_MM: f64 = 0.6;
const HALF_REACH_MM: f64 = 4.38;
/// The legs' taper: at the coxa and at the tarsal claw.
const COXA_MM: f64 = 0.72;
const CLAW_MM: f64 = 0.28;
/// The legs' radius where the femur leaves the carapace, and at the knee; the rest tapers on from the knee.
const EMERGE_MM: f64 = 1.1;
const THIGH_MM: f64 = 0.58;
const KNEE_MM: f64 = 0.52;
/// How far the knee stands over the edge past riding it, so the femur arches up to it.
const KNEE_LIFT_MM: f64 = 0.45;
/// Knuckles over the tube they joint.
const KNUCKLE: f64 = 1.08;
/// Where the legs root on the carapace's flanks, across the band.
const ROOT_ACROSS_MM: f64 = 1.6;
/// How deep the tarsal claw sinks into the cheek.
const CLAW_SINK_MM: f64 = 0.3;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

/// The keyed LowDome band, squared at the cheeks: broad and deep under the spider, slim at the palm.
fn band() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Arachne \u{2014} the weaver".into();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 5.6;
    d.profile.thickness_mm = 2.4;
    d.profile.crown_mm = 0.72;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![
        key(30.0, 1.16, 1.10),
        key(64.0, 1.33, 1.24),
        key(108.0, 1.33, 1.25),
        key(150.0, 1.16, 1.10),
        key(210.0, 0.97, 0.98),
        key(270.0, 0.92, 0.95),
        key(330.0, 0.97, 0.98),
    ];
    CastProcess::LostWax.apply(&mut d.draft);
    d
}

fn garnet() -> Gem {
    Gem { cut: GemCut::Oval, w_mm: 3.5, l_mm: 5.0, form: GemForm::Cabochon, preview_tint: Some([0.42, 0.02, 0.05]) }
}
fn onyx() -> Gem {
    let mut g = Gem::cabochon(GemCut::Oval, 7.8);
    g.preview_tint = Some([0.015, 0.015, 0.02]);
    g
}

/// A stone's bed: a low pad hidden under the stone that holds its girdle `rise` over the crest.
fn bed(theta: f64, v: f64, gem: Gem, rise: f64) -> SeatPadLayer {
    let mut s = SeatPadLayer { theta_deg: theta, v_mm: v, style: SeatStyle::Boss, metal_true: true, solid: SolidKind::None, blend_mm: 0.3, crown: 1.0, ..Default::default() };
    s.fit_stone(gem);
    s.diameter_mm = 0.55 * gem.w_mm;
    s.elong = gem.l_mm / gem.w_mm;
    s.height_mm = rise;
    s.set_depth_mm = Some(0.0);
    s
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add3(a: P3, b: P3, k: f64) -> P3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}
fn cross3(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit3(a: P3) -> P3 {
    let l = dot(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}
fn len3(a: P3) -> f64 {
    dot(a, a).sqrt()
}
/// A world point at a ring angle, radius and place along the finger.
fn at_ring(theta: f64, r: f64, z: f64) -> P3 {
    let (s, c) = theta.to_radians().sin_cos();
    [r * c, r * s, z]
}
fn theta_of(p: P3) -> f64 {
    p[1].atan2(p[0]).to_degrees()
}
fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
/// The smaller of two, rounded over `k`.
fn smin(a: f64, b: f64, k: f64) -> f64 {
    let m = a.min(b);
    m - k * ((-(a - m) / k).exp() + (-(b - m) / k).exp()).ln()
}


/// One section of the bare band: its loop, and its outer surface from the crest down the low cheek.
struct Section {
    loop_rz: Vec<P2>,
    /// Arc from the crest, radius and place along the finger, down the low side to the bore.
    flank: Vec<[f64; 3]>,
    /// Arc from the crest to where the side face begins.
    edge_t: f64,
    half_width: f64,
    edge_r: f64,
    bore_r: f64,
}

/// The bare band's modulated sections, cached every quarter degree.
struct Bare {
    d: RingDesign,
    reference: ProfileLoop,
    cache: RefCell<HashMap<i64, Rc<Section>>>,
}

impl Bare {
    const STEP: f64 = 0.25;

    fn new(d: &RingDesign) -> Self {
        Self { d: d.clone(), reference: d.reference_loop(), cache: RefCell::new(HashMap::new()) }
    }

    fn at(&self, theta: f64) -> Rc<Section> {
        let n = (360.0 / Self::STEP) as i64;
        let k = ((theta / Self::STEP).round() as i64).rem_euclid(n);
        if let Some(s) = self.cache.borrow().get(&k) {
            return s.clone();
        }
        let l = self.d.section_at(k as f64 * Self::STEP, 256, None, Some(&self.reference));
        let loop_rz: Vec<P2> = l.pts.iter().map(|p| [p.r, p.z]).collect();
        let half_width = loop_rz.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
        let bore_r = loop_rz.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let crest = l.pts.iter().filter(|p| p.surface).max_by(|a, b| a.r.total_cmp(&b.r)).expect("a section with an outer surface");
        let (cz, cv) = (crest.z, crest.v_mm);
        let mut rows: Vec<[f64; 4]> = l.pts.iter().filter(|p| p.surface && p.z <= cz + 1e-9).map(|p| [(p.v_mm - cv).abs(), p.r, p.z, p.nz]).collect();
        rows.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let side = rows.iter().find(|p| p[3].abs() >= 80f64.to_radians().sin()).copied().unwrap_or(*rows.last().unwrap());
        let s = Rc::new(Section {
            loop_rz,
            flank: rows.iter().map(|p| [p[0], p[1], p[2]]).collect(),
            edge_t: side[0],
            half_width,
            edge_r: side[1],
            bore_r,
        });
        self.cache.borrow_mut().insert(k, s.clone());
        s
    }

    /// Radius of the outer surface `t` of arc from the crest.
    fn r_at_t(&self, theta: f64, t: f64) -> f64 {
        let s = self.at(theta);
        let f = &s.flank;
        let t = t.abs().clamp(0.0, f.last().unwrap()[0]);
        let i = f.partition_point(|p| p[0] < t).clamp(1, f.len() - 1);
        let (a, b) = (f[i - 1], f[i]);
        lerp(a[1], b[1], ((t - a[0]) / (b[0] - a[0]).max(1e-12)).clamp(0.0, 1.0))
    }

    /// Radius of the crown where it stands `across` mm from the band's mid-plane.
    fn crown_r(&self, theta: f64, across: f64) -> f64 {
        let s = self.at(theta);
        let z = -across.abs();
        let f = &s.flank;
        for w in f.windows(2) {
            if w[1][2] <= z {
                return lerp(w[0][1], w[1][1], ((z - w[0][2]) / (w[1][2] - w[0][2]).min(-1e-12)).clamp(0.0, 1.0));
            }
        }
        s.edge_r
    }

    /// A point on the low cheek: `share` of its height over the bore, `off` out from its face.
    fn cheek(&self, theta: f64, share: f64, off: f64) -> P3 {
        let s = self.at(theta);
        at_ring(theta, s.bore_r + share * (s.edge_r - s.bore_r), -(s.half_width + off))
    }

    /// Distance from a world point to the band at its own angle, negative inside the metal.
    fn signed_distance(&self, w: P3) -> f64 {
        let s = self.at(theta_of(w));
        let q = [w[0].hypot(w[1]), w[2]];
        let pts = &s.loop_rz;
        let mut inside = false;
        let mut near = f64::MAX;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                inside = !inside;
            }
            let ab = [b[0] - a[0], b[1] - a[1]];
            let t = (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-18)).clamp(0.0, 1.0);
            near = near.min((q[0] - a[0] - ab[0] * t).hypot(q[1] - a[1] - ab[1] * t));
        }
        if inside { -near } else { near }
    }

    /// The band's half-width at a ring angle, read between the cached sections.
    fn half_width(&self, theta: f64) -> f64 {
        let k = (theta / Self::STEP).floor();
        let u = theta / Self::STEP - k;
        lerp(self.at(k * Self::STEP).half_width, self.at((k + 1.0) * Self::STEP).half_width, u)
    }

    /// How far a leg may stand past the cheek at a ring angle: within the ring's reach, and never more than 0.8.
    fn proud(&self, theta: f64) -> f64 {
        smin(HALF_REACH_MM - 0.03 - self.half_width(theta), 0.8, 0.05)
    }
}

/// One stone and the body built round it, in the stone's own frame.
#[derive(Clone)]
struct Body {
    gem: Gem,
    f: StoneFrame,
    /// Which way along `f.long` the ring angle climbs.
    ahead: f64,
    /// The collet's lip as a share of the stone's crown.
    lip: f64,
    /// The body's fall from its widest to the band: how far it draws in, and over what depth.
    fall: (f64, f64),
}

impl Body {
    fn semi(&self) -> (f64, f64) {
        (self.gem.l_mm * 0.5, self.gem.w_mm * 0.5)
    }
    /// A local point of the stone's frame in the world.
    fn world(&self, p: P3) -> P3 {
        let f = &self.f;
        std::array::from_fn(|k| f.girdle[k] + f.long[k] * p[0] + f.short[k] * p[1] + f.normal[k] * p[2])
    }
    /// A world point in the stone's frame.
    fn local(&self, w: P3) -> P3 {
        let d = sub3(w, self.f.girdle);
        [dot(d, self.f.long), dot(d, self.f.short), dot(d, self.f.normal)]
    }
    /// The collet's outer wall at a height over the girdle, as an offset from the girdle outline.
    fn collet_out(&self, z: f64) -> f64 {
        let t = ((z + COLLET_BASE_MM) / (self.rim() - 0.16 + COLLET_BASE_MM)).clamp(0.0, 1.0);
        COLLET_CLEAR_MM + COLLET_WALL_MM * (0.8 + 0.2 * t)
    }
    /// Height of the collet's rim over the girdle.
    fn rim(&self) -> f64 {
        setting::girdle_half_mm(self.gem) + self.lip * self.gem.crown_mm()
    }
    /// The body's widest offset, where the trimmed collet ends, a hair past its wall.
    fn widest(&self) -> f64 {
        self.collet_out(BODY_FULL_Z) + 0.02
    }
    /// The body's outer surface at a height over the girdle, as an offset from the girdle outline; `None` below its fall.
    fn wall(&self, z: f64) -> Option<f64> {
        if z > self.rim() {
            return None;
        }
        if z >= BODY_FULL_Z {
            return Some(self.collet_out(z));
        }
        let (inset, depth) = self.fall;
        let dz = BODY_FULL_Z - z;
        (dz <= depth).then(|| self.widest() - inset * (1.0 - (1.0 - (dz / depth).powi(2)).sqrt()))
    }
    /// The end of the collet facing along the ring, `sign` +1 toward higher angles.
    fn end(&self, sign: f64) -> P3 {
        let (a, _) = self.semi();
        self.world([sign * self.ahead * (a + self.collet_out(0.0)), 0.0, 0.0])
    }
    /// How far a world point stands outside the body at its own height, while beside it.
    fn margin(&self, w: P3) -> f64 {
        let q = self.local(w);
        let Some(o) = self.wall(q[2]) else { return f64::MAX };
        let (a, b) = self.semi();
        let rho = ((q[0] / (a + o)).powi(2) + (q[1] / (b + o)).powi(2)).sqrt();
        (rho - 1.0) * (b + o)
    }
}

/// The two stones' frames as the stone record places them.
fn bodies(d: &RingDesign) -> Result<(Body, Body)> {
    let frames = ringdesign_core::stones::stone_frames(d);
    ensure!(frames.len() == 2, "Arachne sets two stones, not {}", frames.len());
    let body = |i: usize, gem: Gem, lip: f64, fall: (f64, f64)| {
        let f = frames[i].1.clone();
        let tangent = at_ring(theta_of(f.girdle) + 90.0, 1.0, 0.0);
        let ahead = dot(f.long, tangent).signum();
        Body { gem, f, ahead, lip, fall }
    };
    Ok((body(0, garnet(), 0.22, (1.2, 1.0)), body(1, onyx(), 0.16, (1.9, 1.5))))
}

/// The abdomen's ring angle that leaves `BODY_GAP_MM` between the two collets.
fn abdomen_deg(bare: &Bare) -> f64 {
    let rc = bare.at(HUB_DEG).flank[0][1] + CARAPACE_RISE_MM;
    let ra = bare.at(HUB_DEG + 44.0).flank[0][1] + ABDOMEN_RISE_MM;
    let reach = |g: Gem| g.l_mm * 0.5 + COLLET_CLEAR_MM + COLLET_WALL_MM;
    HUB_DEG + ((reach(garnet()) / rc).atan() + (reach(onyx()) / ra).atan() + BODY_GAP_MM / (0.5 * (rc + ra))).to_degrees()
}

// --- The web ----------------------------------------------------------------

/// A thread of web in metal: (ring angle, signed arc from the crest) points, and its width.
struct Thread {
    pts: Vec<P2>,
    w: f64,
    /// Ink, 1 for full relief.
    ink: f64,
}

/// A dew bead on the dragline: its centre and radius.
struct Dew {
    at: P2,
    r: f64,
}

/// The chart the web is drawn into: metal arcs to `u`/`v`, station by station.
struct Chart<'a> {
    bare: &'a Bare,
    ctx: FieldContext,
}

impl Chart<'_> {
    fn u(&self, theta: f64) -> f64 {
        self.ctx.u_of_theta(theta)
    }
    fn v(&self, theta: f64, t: f64) -> f64 {
        self.ctx.crest_v_mm + t / self.ctx.station_stretch(theta.rem_euclid(360.0))
    }
    fn r(&self, theta: f64, t: f64) -> f64 {
        self.bare.r_at_t(theta, t)
    }
    fn edge(&self, theta: f64) -> f64 {
        self.bare.at(theta).edge_t
    }
    /// The ring angle `s` mm of metal along from `theta` at arc `t` off the crest.
    fn step(&self, theta: f64, t: f64, s: f64) -> f64 {
        theta + (s / self.r(theta, t)).to_degrees()
    }
}

/// Splits a thread where it leaves the crown, keeping the runs on it.
fn on_crown(ch: &Chart, t: Thread) -> Vec<Thread> {
    let mut out = Vec::new();
    let mut run: Vec<P2> = Vec::new();
    for p in t.pts {
        if p[1].abs() <= ch.edge(p[0]) - 0.05 {
            run.push(p);
        } else if run.len() > 1 {
            out.push(Thread { pts: std::mem::take(&mut run), w: t.w, ink: t.ink });
        } else {
            run.clear();
        }
    }
    if run.len() > 1 {
        out.push(Thread { pts: run, w: t.w, ink: t.ink });
    }
    out
}

/// The orb round the hub, drawn round in metal: radials, a six-turn capture spiral, and the hub itself.
fn orb(ch: &Chart, hub: f64, out: &mut Vec<Thread>, dew: &mut Vec<Dew>) {
    let at = |s: f64, t: f64| -> P2 { [ch.step(hub, t, s), t] };
    let reach = SPIRAL_START_MM + SPIRAL_PITCH_MM * SPIRAL_TURNS as f64;
    for k in 0..RADIALS {
        let a = (15.0 + 360.0 / RADIALS as f64 * k as f64).to_radians();
        let n = 90;
        let pts = (0..=n).map(|i| 1.0 + (reach + 0.45 - 1.0) * i as f64 / n as f64).map(|rho| at(rho * a.cos(), rho * a.sin())).collect();
        out.extend(on_crown(ch, Thread { pts, w: 0.36, ink: 1.0 }));
    }
    let node = |j: usize| -> P2 {
        let a = (15.0 + 360.0 / RADIALS as f64 * j as f64).to_radians();
        let rho = SPIRAL_START_MM + SPIRAL_PITCH_MM * j as f64 / RADIALS as f64;
        [rho * a.cos(), rho * a.sin()]
    };
    let mut pts = Vec::new();
    for j in 0..RADIALS * SPIRAL_TURNS {
        let (a, b) = (node(j), node(j + 1));
        let mid = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
        let m = mid[0].hypot(mid[1]).max(1e-9);
        for i in 0..8 {
            let u = i as f64 / 8.0;
            let sag = 0.18 * 4.0 * u * (1.0 - u);
            pts.push(at(lerp(a[0], b[0], u) - mid[0] / m * sag, lerp(a[1], b[1], u) - mid[1] / m * sag));
        }
    }
    let end = node(RADIALS * SPIRAL_TURNS);
    pts.push(at(end[0], end[1]));
    out.extend(on_crown(ch, Thread { pts, w: 0.30, ink: 1.0 }));
    dew.push(Dew { at: [hub, 0.0], r: 1.05 });
}

/// Crest arc along the ring from the hub, both ways round to the palm and past it.
struct Along {
    theta: Vec<f64>,
    s: Vec<f64>,
}

impl Along {
    fn new(ch: &Chart, hub: f64) -> Self {
        let step = 0.1;
        let n = (400.0 / step) as usize;
        let theta: Vec<f64> = (0..=2 * n).map(|i| hub - 400.0 + i as f64 * step).collect();
        let mut s = vec![0.0; theta.len()];
        for i in n + 1..theta.len() {
            s[i] = s[i - 1] + step.to_radians() * ch.r(theta[i] - 0.5 * step, 0.0);
        }
        for i in (0..n).rev() {
            s[i] = s[i + 1] - step.to_radians() * ch.r(theta[i] + 0.5 * step, 0.0);
        }
        Self { theta, s }
    }
    fn s_of(&self, theta: f64) -> f64 {
        let i = self.theta.partition_point(|t| *t < theta).clamp(1, self.theta.len() - 1);
        lerp(self.s[i - 1], self.s[i], (theta - self.theta[i - 1]) / (self.theta[i] - self.theta[i - 1]))
    }
    fn theta_of(&self, s: f64) -> f64 {
        let i = self.s.partition_point(|t| *t < s).clamp(1, self.s.len() - 1);
        lerp(self.theta[i - 1], self.theta[i], (s - self.s[i - 1]) / (self.s[i] - self.s[i - 1]).max(1e-12))
    }
}

/// One side of the orb past its spiral: radials fanning toward the palm, late radials joining, rings widening and hanging toward the hub.
fn fan(ch: &Chart, along: &Along, hub_s: f64, side: f64, palm_s: f64, radials: &[(f64, f64)], rings: &[f64], out: &mut Vec<Thread>, dew: &mut Vec<Dew>) {
    let reach = (palm_s - hub_s).abs();
    let last = rings.iter().copied().filter(|r| *r < reach - 1.5).fold(0.0, f64::max);
    let at = |rho: f64, phi: f64| -> P2 {
        let s = hub_s + side * rho * phi.to_radians().cos();
        [along.theta_of(s), rho * phi.to_radians().sin()]
    };
    for &(alpha, start) in radials {
        for sign in [-1.0, 1.0] {
            let end = (last + 0.2) / alpha.to_radians().cos();
            let n = ((end - start) / 0.1).ceil().max(2.0) as usize;
            let pts = (0..=n).map(|i| at(lerp(start, end, i as f64 / n as f64), sign * alpha)).collect();
            out.extend(on_crown(ch, Thread { pts, w: if start < 2.0 { 0.34 } else { 0.30 }, ink: 1.0 }));
        }
    }
    for &rho in rings.iter().filter(|r| **r < reach - 1.5) {
        let edge = ch.edge(along.theta_of(hub_s + side * rho));
        let rim = (edge / rho).min(1.0).asin().to_degrees();
        let mut nodes: Vec<f64> = radials.iter().filter(|(a, st)| *st <= rho && *a < rim).flat_map(|(a, _)| [-a, *a]).collect();
        nodes.extend([-rim - 2.0, rim + 2.0]);
        nodes.sort_by(|a, b| a.total_cmp(b));
        let mut pts = Vec::new();
        for (k, w) in nodes.windows(2).enumerate() {
            let chord = rho * (w[1] - w[0]).to_radians();
            let sag = (0.4 * chord).min(0.95);
            for j in 0..12 {
                let u = j as f64 / 12.0;
                pts.push(at(rho - sag * (PI * u).sin(), lerp(w[0], w[1], u)));
            }
            // Dew gathers where the outer threads hang lowest, on every other one.
            let low = at(rho - sag + 0.02, 0.5 * (w[0] + w[1]));
            if rho >= 12.0 && chord >= 0.9 && k == 1 && low[1].abs() < ch.edge(low[0]) - 0.35 {
                dew.push(Dew { at: low, r: 0.2 });
            }
        }
        pts.push(at(rho, *nodes.last().unwrap()));
        out.extend(on_crown(ch, Thread { pts, w: 0.28, ink: 1.0 }));
    }
}

/// Where the orb's two halves meet round the finger: two anchor threads crossed over the palm from the last ring on one side to the last on the other.
fn anchors(ch: &Chart, along: &Along, from: f64, to: f64, out: &mut Vec<Thread>) {
    for sign in [-1.0, 1.0] {
        let n = 160;
        let pts = (0..=n)
            .map(|i| {
                let u = i as f64 / n as f64;
                let th = along.theta_of(lerp(from, to, u));
                [th, sign * lerp(-1.0, 1.0, u) * (ch.edge(th) - 0.1)]
            })
            .collect();
        out.extend(on_crown(ch, Thread { pts, w: 0.32, ink: 1.0 }));
    }
}

/// The dragline trailing from the spinnerets along the crest: a faint thread strung with dew, the beads shrinking as it goes.
fn dragline(ch: &Chart, from: f64, out: &mut Vec<Thread>, dew: &mut Vec<Dew>) {
    let beads = 12;
    let mut th = from;
    let mut pts = vec![[th, 0.0]];
    for k in 0..beads {
        th = ch.step(th, 0.0, DEW_PITCH_MM);
        pts.push([th, 0.0]);
        dew.push(Dew { at: [th, 0.0], r: 0.5 * lerp(DEW_MM, 0.34, k as f64 / (beads - 1) as f64) });
    }
    let fine: Vec<P2> = pts.windows(2).flat_map(|w| (0..6).map(move |j| [lerp(w[0][0], w[1][0], j as f64 / 6.0), 0.0])).collect();
    out.push(Thread { pts: fine, w: 0.2, ink: 0.58 });
}

/// A thread's outline in (ring angle, arc) with round ends, its width held in metal millimetres.
fn outline(ch: &Chart, pts: &[P2], w: f64) -> Vec<P2> {
    let n = pts.len();
    let h = 0.5 * w;
    let frame = |i: usize| -> (P2, P2, f64) {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        let r = ch.r(pts[i][0], pts[i][1]);
        let (ds, dt) = ((b[0] - a[0]).to_radians() * r, b[1] - a[1]);
        let l = ds.hypot(dt).max(1e-12);
        ([ds / l, dt / l], [-dt / l, ds / l], r)
    };
    let shift = |p: P2, v: P2, r: f64, k: f64| -> P2 { [p[0] + (v[0] * k / r).to_degrees(), p[1] + v[1] * k] };
    let (mut left, mut right) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        let (_, nrm, r) = frame(i);
        left.push(shift(pts[i], nrm, r, h));
        right.push(shift(pts[i], nrm, r, -h));
    }
    let cap = |i: usize, dir: f64| -> Vec<P2> {
        let (tan, nrm, r) = frame(i);
        (1..8)
            .map(|j| {
                let a = PI * j as f64 / 8.0;
                let v = [nrm[0] * a.cos() + dir * tan[0] * a.sin(), nrm[1] * a.cos() + dir * tan[1] * a.sin()];
                shift(pts[i], [dir * v[0], dir * v[1]], r, h * dir)
            })
            .collect()
    };
    let mut ring = left;
    ring.extend(cap(n - 1, 1.0));
    ring.extend(right.into_iter().rev());
    ring.extend(cap(0, -1.0));
    ring
}

/// One window of the web as SVG in chart millimetres, from ring angle `lo` to `hi`, `half` of its height either side of the crest.
fn window_svg(ch: &Chart, threads: &[Thread], dew: &[Dew], lo: f64, hi: f64, half: f64) -> (String, f64) {
    let width = ch.u(hi) - ch.u(lo);
    let xy = |p: P2| -> (f64, f64) {
        let th = lo + (p[0] - lo).rem_euclid(360.0);
        let th = if th > hi + 180.0 { th - 360.0 } else { th };
        (ch.u(th) - ch.u(lo), half - (ch.v(th, p[1]) - ch.ctx.crest_v_mm))
    };
    let near = |th: f64| {
        let d = (th - lo).rem_euclid(360.0);
        d <= hi - lo + 2.0 || d >= 358.0
    };
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.4} {h:.4}" width="{width:.4}" height="{h:.4}"><defs><radialGradient id="dew"><stop offset="0" stop-color="#000"/><stop offset="0.55" stop-color="#141414"/><stop offset="1" stop-color="#a0a0a0"/></radialGradient><filter id="soft" filterUnits="userSpaceOnUse" x="0" y="0" width="{width:.4}" height="{h:.4}"><feGaussianBlur stdDeviation="0.07"/></filter></defs><rect width="{width:.4}" height="{h:.4}" fill="#fff"/><g filter="url(#soft)">"##,
        h = 2.0 * half
    );
    for t in threads {
        let mut runs: Vec<Vec<P2>> = vec![Vec::new()];
        for p in &t.pts {
            if near(p[0]) {
                runs.last_mut().unwrap().push(*p);
            } else if !runs.last().unwrap().is_empty() {
                runs.push(Vec::new());
            }
        }
        for run in runs.iter().filter(|r| r.len() > 1) {
            let ring = outline(ch, run, t.w);
            let grey = ((1.0 - t.ink) * 255.0).round() as u8;
            s += &format!(r##"<path fill="#{grey:02x}{grey:02x}{grey:02x}" d="M"##);
            for (i, p) in ring.iter().enumerate() {
                let (x, y) = xy(*p);
                s += &format!("{}{x:.3} {y:.3}", if i == 0 { "" } else { " L" });
            }
            s += r##" Z"/>"##;
        }
    }
    s += "</g>";
    for b in dew.iter().filter(|b| near(b.at[0])) {
        let ring: Vec<P2> = (0..24)
            .map(|j| {
                let a = 2.0 * PI * j as f64 / 24.0;
                let t = b.at[1] + b.r * a.sin();
                [ch.step(b.at[0], t, b.r * a.cos()), t]
            })
            .collect();
        s += r##"<path fill="url(#dew)" d="M"##;
        for (i, p) in ring.iter().enumerate() {
            let (x, y) = xy(*p);
            s += &format!("{}{x:.3} {y:.3}", if i == 0 { "" } else { " L" });
        }
        s += r##" Z"/>"##;
    }
    s += "</svg>";
    (s, width)
}

/// The cheek tile: radials pointing at the finger, capture threads hanging between them as catenaries and climbing a row per tile.
fn cheek_svg(cell_w: f64, cell_h: f64, radials: usize, pitch: f64) -> String {
    let (radial_w, thread_w) = (0.34, 0.23);
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cell_w:.4} {cell_h:.4}" width="{cell_w:.4}" height="{cell_h:.4}"><rect width="{cell_w:.4}" height="{cell_h:.4}" fill="#fff"/><g fill="none" stroke="#000" stroke-linecap="round">"##
    );
    let step = cell_w / radials as f64;
    for j in 0..=radials {
        let x = j as f64 * step;
        s += &format!(r##"<line x1="{x:.4}" y1="-1" x2="{x:.4}" y2="{:.4}" stroke-width="{radial_w}"/>"##, cell_h + 1.0);
    }
    let rows = (cell_h / pitch).ceil() as i32 + 3;
    for m in -2..rows {
        for j in 0..radials {
            let (x0, x1) = (j as f64 * step, (j + 1) as f64 * step);
            let y = |x: f64| m as f64 * pitch + pitch * x / cell_w + 0.5 * pitch;
            let (y0, y1) = (y(x0), y(x1));
            let sag = 1.24 * pitch;
            s += &format!(r##"<path d="M{x0:.4} {y0:.4} Q{:.4} {:.4} {x1:.4} {y1:.4}" stroke-width="{thread_w}"/>"##, 0.5 * (x0 + x1), 0.5 * (y0 + y1) - 2.0 * sag);
        }
    }
    s += "</g></svg>";
    s
}

/// The cheek web on both side faces.
fn cheek_web(d: &mut RingDesign) -> Result<()> {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Cheek web", &ctx);
    ensure!(t.fit_to_side_faces(&ctx, ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG), "The band carries no side face for the web");
    t.repeats_around = 8;
    t.height_mm = 0.32;
    t.continuous = true;
    t.feather_mm = 0.0;
    t.edge_mm = 0.08;
    let (cw, ch) = t.cell_size(&ctx);
    println!("  cheek web cell {cw:.2} x {ch:.2} mm");
    d.svgs.push(SvgAlpha { name: "Cheek web".into(), svg: cheek_svg(cw, ch, 3, 1.0), invert: false });
    let mut e = LayerEntry::new("Cheek web, radials to the finger", Layer::Tiling(t));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok(())
}

/// The crown web: the orb round the hub, its ladder down both shoulders to the palm, and the dragline, in four decals.
fn crown_web(d: &mut RingDesign, bare: &Bare, abdomen: &Body) -> Result<f64> {
    let ch = Chart { bare, ctx: d.field_context() };
    let (a, _) = abdomen.semi();
    let spinnerets = theta_of(abdomen.world([abdomen.ahead * (a + abdomen.collet_out(0.0) + 0.3), 0.0, 0.0]));
    let mut threads = Vec::new();
    let mut dew = Vec::new();
    orb(&ch, HUB_DEG, &mut threads, &mut dew);
    let along = Along::new(&ch, HUB_DEG);
    let hub_s = along.s_of(HUB_DEG);
    let primary = [(15.0, 1.0), (45.0, 1.0), (75.0, 1.0)];
    let (palm_front, palm_back) = (along.s_of(270.0 - 360.0), along.s_of(270.0));
    fan(&ch, &along, hub_s, -1.0, palm_front, &[primary[0], primary[1], primary[2], (3.0, 9.0)], &FRONT_RINGS, &mut threads, &mut dew);
    fan(&ch, &along, hub_s, 1.0, palm_back, &[primary[0], primary[1], primary[2], (2.2, 12.0)], &BACK_RINGS, &mut threads, &mut dew);
    let last_front = FRONT_RINGS.iter().copied().fold(0.0, f64::max);
    let last_back = BACK_RINGS.iter().copied().fold(0.0, f64::max);
    // The back's last ring, read on the front's side of the palm.
    let circumference = palm_back - palm_front;
    anchors(&ch, &along, hub_s - last_front, hub_s + last_back - circumference, &mut threads);
    dragline(&ch, spinnerets, &mut threads, &mut dew);
    let half = (0..360).map(|k| ch.edge(k as f64) / ch.ctx.station_stretch(k as f64)).fold(0.0, f64::max) + 0.4;
    let span = 42.0;
    let mut windows = vec![(HUB_DEG - span, HUB_DEG + span, "Orb web round the spider".to_string())];
    let (lo, hi) = (HUB_DEG + span - 360.0, HUB_DEG - span);
    for k in 0..3 {
        let (a, b) = (lerp(lo, hi, k as f64 / 3.0), lerp(lo, hi, (k + 1) as f64 / 3.0));
        windows.push((a, b, format!("Orb web {}", ["behind her", "at the palm", "before her"][k])));
    }
    for (lo, hi, name) in windows {
        let (lo, hi) = (lo - 2.5, hi + 2.5);
        let (svg, width) = window_svg(&ch, &threads, &dew, lo, hi, half);
        d.svgs.push(SvgAlpha { name: name.clone(), svg, invert: false });
        let stamp = Decal { theta_deg: (0.5 * (lo + hi)).rem_euclid(360.0), v_mm: ch.ctx.crest_v_mm, size_mm: width, rotation_deg: 0.0, height_mm: WEB_HEIGHT_MM, flip: false };
        let mut e = LayerEntry::new(name.clone(), Layer::Decals(DecalLayer { alpha: name, decals: vec![stamp], feather_mm: 0.05, invert: false }));
        e.blend = Blend::Max;
        d.layers.layers.push(e);
    }
    Ok(spinnerets)
}

// --- Bodies -----------------------------------------------------------------

/// A made collet with a thin wall, from `setting`'s own section, packed as a stored part at the stone.
fn collet(body: &Body) -> Result<Operation> {
    let named = setting::collet_named(body.gem, COLLET_WALL_MM, body.lip, -COLLET_BASE_MM);
    let reach = body.gem.l_mm + 4.0;
    let slab = cuboid([-reach, -reach, -COLLET_BASE_MM - 1.0], [reach, reach, BODY_FULL_Z]);
    let solid = csg::combine(&named.solid, &slab, csg::Op::Subtract).map_err(|e| anyhow::anyhow!("{}: the collet will not trim under its girdle: {e}", body.gem.display()))?;
    ensure!(csg::self_crossings(&solid) == 0, "{}: the trimmed collet crosses itself", body.gem.display());
    let flip = dot(cross3(body.f.long, body.f.short), body.f.normal) < 0.0;
    let positions: Vec<P3> = solid.v.iter().map(|p| body.world(*p)).collect();
    let triangles: Vec<[u32; 3]> = solid.f.iter().map(|t| if flip { [t[0], t[2], t[1]] } else { *t }).collect();
    let mesh = stored::Packed::encode(&positions, &triangles, &vec![0; triangles.len()], &[SurfaceKind::Freeform])?;
    let recipe = stored::Recipe {
        kernel: "setting".into(),
        op: "collet".into(),
        params: serde_json::json!({ "stone": body.gem.display(), "wall_mm": COLLET_WALL_MM, "lip": body.lip, "base_mm": COLLET_BASE_MM }),
        digest: String::new(),
    };
    Ok(Operation::Stored { recipe, sources: Vec::new(), mesh })
}

/// An outward-wound box.
fn cuboid(lo: P3, hi: P3) -> csg::Solid {
    let v: Vec<P3> = (0..8).map(|i: usize| std::array::from_fn(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })).collect();
    let quads: [[u32; 4]; 6] = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
    let f = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
    csg::Solid { v, f }
}

/// Sides round each row of a body's loft.
const BELLY_SIDES: usize = 144;

/// A closed polygonal ellipse on the stone's girdle plane, `z` over it.
fn row(body: &Body, a: f64, b: f64, z: f64) -> Sketch {
    let f = &body.f;
    let mut s = Sketch { name: "Body row".into(), plane: Workplane { origin: add3(f.girdle, f.normal, z), x: f.long, y: f.short, on_face: None }, ..Sketch::default() };
    let ids: Vec<Id> = (0..BELLY_SIDES)
        .map(|i| {
            let t = (i as f64 + 0.5) / BELLY_SIDES as f64 * std::f64::consts::TAU;
            s.point(snap([a * t.cos(), b * t.sin()]))
        })
        .collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

/// A body under its trimmed collet: hidden in the collet's wall at the girdle, then one convex fall from the collet's foot into the band.
fn belly(body: &Body) -> Result<Operation> {
    let (a0, b0) = body.semi();
    let o_top = body.collet_out(-0.08) - 0.06;
    let mut rows: Vec<(f64, f64, f64)> = vec![(a0 + o_top, b0 + o_top, -0.08)];
    let (inset, depth) = body.fall;
    let n = 12;
    for i in 0..=n {
        let phi = 0.5 * PI * i as f64 / n as f64;
        let o = body.widest() - inset * (1.0 - phi.cos());
        rows.push((a0 + o, b0 + o, BODY_FULL_Z - depth * phi.sin()));
    }
    ensure!(body.widest() >= body.collet_out(BODY_FULL_Z) + 0.015, "{}: the body stands inside its collet's foot", body.gem.display());
    rows.reverse();
    if dot(cross3(body.f.long, body.f.short), body.f.normal) < 0.0 {
        rows.reverse();
    }
    Ok(Operation::Loft { sections: rows.into_iter().map(|(a, b, z)| row(body, a, b, z).into()).collect() })
}

// --- The legs ---------------------------------------------------------------

/// One leg: where it roots on the carapace, which way it reaches, and where its joints and claw land on the cheek.
#[derive(Clone, Copy)]
struct LegSpec {
    name: &'static str,
    /// Along the ring from the carapace's centre, metal mm; negative is forward.
    root_along: f64,
    /// Off the ring's forward axis in plan, degrees.
    reach_deg: f64,
    /// The claw's ring angle from the hub.
    claw_deg: f64,
    /// Heights on the cheek, as shares of it over the bore: the ankle, the metatarsal joint and the claw.
    shares: [f64; 3],
}

const LEGS: [LegSpec; 4] = [
    LegSpec { name: "Leg I", root_along: -1.35, reach_deg: 25.0, claw_deg: -58.0, shares: [0.74, 0.48, 0.26] },
    LegSpec { name: "Leg II", root_along: -0.45, reach_deg: 45.0, claw_deg: -36.0, shares: [0.68, 0.44, 0.18] },
    LegSpec { name: "Leg III", root_along: 0.45, reach_deg: 132.0, claw_deg: 36.0, shares: [0.68, 0.44, 0.18] },
    LegSpec { name: "Leg IV", root_along: 1.35, reach_deg: 146.0, claw_deg: 88.0, shares: [0.76, 0.5, 0.26] },
];

/// A planar path as a chain of cubics in its plane, with the tube's radius at each end.
struct LegPath {
    plane: Workplane,
    pieces: Vec<[P2; 4]>,
    r0: f64,
    r1: f64,
}

impl LegPath {
    fn world(&self, p: P2) -> P3 {
        let w = &self.plane;
        std::array::from_fn(|k| w.origin[k] + w.x[k] * p[0] + w.y[k] * p[1])
    }
    fn length(&self) -> f64 {
        self.pieces.iter().map(|c| (0..20).map(|i| dist2(bezier(c, i as f64 / 20.0), bezier(c, (i + 1) as f64 / 20.0))).sum::<f64>()).sum()
    }
    /// Points along the path's centre line in the world, with the tube's radius there.
    fn samples(&self) -> Vec<(P3, f64)> {
        let n = self.pieces.len() * 8;
        (0..=n)
            .map(|i| {
                let k = (i / 8).min(self.pieces.len() - 1);
                let u = if i == n { 1.0 } else { (i % 8) as f64 / 8.0 };
                (self.world(bezier(&self.pieces[k], u)), lerp(self.r0, self.r1, i as f64 / n as f64))
            })
            .collect()
    }
    fn sketch(&self, name: &str) -> Sketch {
        let mut s = Sketch { name: name.into(), plane: self.plane.clone(), ..Sketch::default() };
        let mut last: Option<Id> = None;
        for c in &self.pieces {
            let a = last.unwrap_or_else(|| s.point(c[0]));
            let (b, cc, e) = (s.point(c[1]), s.point(c[2]), s.point(c[3]));
            s.entity(Geometry::Bezier { points: [a, b, cc, e] });
            last = Some(e);
        }
        s
    }
    /// The tube's circle square to the path where it starts.
    fn section(&self) -> Sketch {
        let c = &self.pieces[0];
        let at = self.world(c[0]);
        let d = [c[1][0] - c[0][0], c[1][1] - c[0][1]];
        let t = unit3(add3(self.plane.x.map(|v| v * d[0]), self.plane.y, d[1]));
        let helper = if t[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
        let x = unit3(cross3(helper, t));
        let y = cross3(t, x);
        let mut s = Sketch::circle(self.r0);
        s.plane = Workplane { origin: at, x, y, on_face: None };
        s
    }
    /// Tightest bend over the tube's radius where it bends; a tube folds under 1.
    fn bend_ratio(&self) -> f64 {
        let n = self.pieces.len() as f64;
        self.pieces.iter().enumerate().map(|(k, c)| tightest_bend(c) / lerp(self.r0, self.r1, k as f64 / n)).fold(f64::MAX, f64::min)
    }
}

/// A loft row's coordinate on a 10 nm grid, so the file carries no digits the build cannot see.
fn snap(p: P2) -> P2 {
    p.map(|v| (v * 1e5).round() / 1e5)
}
fn dist2(a: P2, b: P2) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}
fn bezier(c: &[P2; 4], t: f64) -> P2 {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [a * c[0][0] + b * c[1][0] + cc * c[2][0] + d * c[3][0], a * c[0][1] + b * c[1][1] + cc * c[2][1] + d * c[3][1]]
}
/// The smallest radius of curvature along a cubic.
fn tightest_bend(c: &[P2; 4]) -> f64 {
    (0..=100)
        .map(|i| {
            let t = i as f64 / 100.0;
            let u = 1.0 - t;
            let d1: P2 = std::array::from_fn(|k| 3.0 * (u * u * (c[1][k] - c[0][k]) + 2.0 * u * t * (c[2][k] - c[1][k]) + t * t * (c[3][k] - c[2][k])));
            let d2: P2 = std::array::from_fn(|k| 6.0 * (u * (c[2][k] - 2.0 * c[1][k] + c[0][k]) + t * (c[3][k] - 2.0 * c[2][k] + c[1][k])));
            d1[0].hypot(d1[1]).powi(3) / (d1[0] * d2[1] - d1[1] * d2[0]).abs().max(1e-12)
        })
        .fold(f64::MAX, f64::min)
}

/// A smooth chain of cubics through points, tangents by central differences.
fn through(pts: &[P2]) -> Vec<[P2; 4]> {
    let n = pts.len();
    let m = |j: usize| -> P2 {
        let (a, b) = (pts[j.saturating_sub(1)], pts[(j + 1).min(n - 1)]);
        let k = if j == 0 || j == n - 1 { 1.0 } else { 0.5 };
        [(b[0] - a[0]) * k, (b[1] - a[1]) * k]
    };
    (0..n - 1)
        .map(|i| {
            let (m0, m1) = (m(i), m(i + 1));
            [pts[i], [pts[i][0] + m0[0] / 3.0, pts[i][1] + m0[1] / 3.0], [pts[i + 1][0] - m1[0] / 3.0, pts[i + 1][1] - m1[1] / 3.0], pts[i + 1]]
        })
        .collect()
}

/// One solved leg and what it measures.
struct Leg {
    segments: Vec<(&'static str, LegPath)>,
    joints: Vec<(&'static str, P3, f64)>,
    knee_deg: f64,
    knee_proud: f64,
    most_proud: f64,
    least_share: f64,
    abdomen_margin: f64,
    length: f64,
}

/// A plane through `o` holding direction `a` and the finger's axis, `x` along `a` squared to the axis, `y` out of the low cheek.
fn cheek_plane(o: P3, a: P3) -> Workplane {
    let x = unit3([a[0], a[1], 0.0]);
    Workplane { origin: o, x, y: [0.0, 0.0, -1.0], on_face: None }
}

/// A run down the cheek from `a` to `b`: a chord in the plane holding it and the finger's axis, standing `out(share of the run, ring angle)` from the band's mid-plane.
fn cheek_run(a: P3, b: P3, r0: f64, r1: f64, out: &dyn Fn(f64, f64) -> f64) -> LegPath {
    let plane = cheek_plane(a, sub3(b, a));
    let end = [dot(sub3(b, a), plane.x), dot(sub3(b, a), plane.y)];
    let n = 16;
    let mut pts: Vec<P2> = (0..=n)
        .map(|i| {
            let u = i as f64 / n as f64;
            let x = end[0] * u;
            [x, out(u, theta_of(add3(a, plane.x, x))) - a[2].abs()]
        })
        .collect();
    pts[0] = [0.0, 0.0];
    pts[n] = end;
    LegPath { plane, pieces: through(&pts), r0, r1 }
}

/// The femur as one cubic from root to knee: bending gently, lying on the crown, rising to the knee, under the garnet's girdle.
fn fit_femur(l: f64, yk: f64, surface: &dyn Fn(f64) -> f64, r_at: &dyn Fn(f64) -> f64, garnet_local: &dyn Fn(P2) -> P3, semi: (f64, f64)) -> Option<[P2; 4]> {
    let (a, b) = (semi.0 + 0.05, semi.1 + 0.05);
    let mut best: Option<(f64, [P2; 4])> = None;
    for a0 in (-6..=8).map(|k| k as f64 * 5.0) {
        for a1 in (-18..=2).map(|k| k as f64 * 5.0) {
            for h0 in [0.25, 0.3, 0.35, 0.4, 0.45] {
                'candidate: for h1 in [0.25, 0.3, 0.35, 0.4, 0.45] {
                    let (s0, c0) = a0.to_radians().sin_cos();
                    let (s1, c1) = a1.to_radians().sin_cos();
                    let c = [[0.0, 0.0], [h0 * l * c0, h0 * l * s0], [l - h1 * l * c1, yk - h1 * l * s1], [l, yk]];
                    if tightest_bend(&c) < 1.2 * COXA_MM {
                        continue;
                    }
                    let mut cost = 0.0;
                    for i in 0..=40 {
                        let p = bezier(&c, i as f64 / 40.0);
                        let r = r_at(p[0]);
                        let q = garnet_local(p);
                        if (q[0] / a).powi(2) + (q[1] / b).powi(2) < 1.0 && q[2] + r > -0.03 {
                            continue 'candidate;
                        }
                        if p[0] > 0.3 * l && p[0] < 0.85 * l {
                            cost += (p[1] - (surface(p[0]) + r - 0.15 + KNEE_LIFT_MM * smoothstep(0.3 * l, l, p[0]))).powi(2);
                        }
                        if p[0] > 0.25 * l {
                            cost += 20.0 * ((surface(p[0]) + 0.25 * r) - p[1]).max(0.0).powi(2);
                        }
                    }
                    if best.as_ref().is_none_or(|(k, _)| cost < *k) {
                        best = Some((cost, c));
                    }
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Solves one low-side leg: the femur over the crown to a knee on the edge, the tibia over it, the metatarsus and tarsus down the cheek to a sunk claw.
fn solve_leg(spec: &LegSpec, bare: &Bare, carapace: &Body, abdomen: &Body, length: f64) -> Result<Leg> {
    let knee_at = 0.36 * length;
    let r_at = |s: f64| {
        if s < EMERGE_MM {
            lerp(COXA_MM, THIGH_MM, s / EMERGE_MM)
        } else if s < knee_at {
            lerp(THIGH_MM, KNEE_MM, (s - EMERGE_MM) / (knee_at - EMERGE_MM))
        } else {
            lerp(KNEE_MM, CLAW_MM, ((s - knee_at) / (length - knee_at)).clamp(0.0, 1.0))
        }
    };
    let hub = bare.at(HUB_DEG);
    let crest = hub.flank[0][1];
    let root_deg = HUB_DEG + (spec.root_along / crest).to_degrees();
    let root = at_ring(root_deg, bare.crown_r(root_deg, ROOT_ACROSS_MM) - 0.3, -ROOT_ACROSS_MM);
    let (sin, cos) = spec.reach_deg.to_radians().sin_cos();
    // The knee: where the femur's line in plan meets the edge, riding the corner.
    let mid_r = 0.5 * (crest + hub.edge_r);
    let mut knee_deg = HUB_DEG;
    for _ in 0..6 {
        let hw = bare.at(knee_deg).half_width;
        let along = spec.root_along - cos / sin * (hw - 0.05 - ROOT_ACROSS_MM);
        knee_deg = HUB_DEG + (along / mid_r).to_degrees();
    }
    let ks = bare.at(knee_deg);
    let femur_guess = len3(sub3(at_ring(knee_deg, ks.edge_r, -ks.half_width), root)) * 1.05;
    let r_knee = r_at(femur_guess);
    let knee = at_ring(knee_deg, ks.edge_r + 0.28 + KNEE_LIFT_MM, -(ks.half_width + KNEE_PROUD_MM - KNUCKLE * r_knee));
    // Femur: in the upright plane through root and knee, lying on the crown.
    let up = unit3(at_ring(0.5 * (root_deg + knee_deg), 1.0, 0.0));
    let chord = sub3(knee, root);
    let x = unit3(add3(chord, up, -dot(chord, up)));
    let y = unit3(add3(up, x, -dot(up, x)));
    let fplane = Workplane { origin: root, x, y, on_face: None };
    let to_world = |p: P2| add3(add3(root, x, p[0]), y, p[1]);
    let l = dot(chord, x);
    let yk = dot(chord, y);
    let n = 24;
    let mut last_surface = 0.0;
    let surface: Vec<P2> = (0..=n)
        .map(|i| {
            let xi = l * i as f64 / n as f64;
            let (mut lo, mut hi) = (-3.0, 3.0);
            if bare.signed_distance(to_world([xi, lo])) < 0.0 && bare.signed_distance(to_world([xi, hi])) > 0.0 {
                for _ in 0..40 {
                    let m = 0.5 * (lo + hi);
                    if bare.signed_distance(to_world([xi, m])) < 0.0 { lo = m } else { hi = m }
                }
                last_surface = 0.5 * (lo + hi);
            }
            [xi, last_surface]
        })
        .collect();
    let surface_at = |xi: f64| {
        let i = ((xi / l * n as f64).floor() as usize).min(n - 1);
        let u = (xi / l * n as f64 - i as f64).clamp(0.0, 1.0);
        lerp(surface[i][1], surface[i + 1][1], u)
    };
    let femur_curve = fit_femur(l, yk, &surface_at, &|xi| r_at(xi * 1.05), &|p| carapace.local(to_world(p)), carapace.semi())
        .ok_or_else(|| anyhow::anyhow!("{}: no femur arcs gently from under the carapace to its knee", spec.name))?;
    let femur = LegPath { plane: fplane, pieces: vec![femur_curve], r0: COXA_MM, r1: r_knee };
    // Down the cheek: the ankle, the metatarsal joint and the claw, each tube held a steady stand-off off the face.
    let claw_deg = HUB_DEG + spec.claw_deg;
    let span = claw_deg - knee_deg;
    let (ankle_deg, meta_deg) = (knee_deg + 0.22 * span, knee_deg + 0.6 * span);
    let mut s = femur.length();
    let mut guess = |from: P3, to: P3| {
        s += len3(sub3(to, from)) * 1.03;
        (s, r_at(s))
    };
    let stand = |r: f64, th: f64| bare.half_width(th) + smin(r - 0.15, bare.proud(th) - KNUCKLE * r, 0.05);
    let ankle_pt = |r: f64| {
        let p = bare.cheek(ankle_deg, spec.shares[0], 0.0);
        [p[0], p[1], -stand(r, ankle_deg)]
    };
    let (_, r_ankle) = guess(knee, ankle_pt(r_knee));
    let ankle = ankle_pt(r_ankle);
    let meta_pt = |r: f64| {
        let p = bare.cheek(meta_deg, spec.shares[1], 0.0);
        [p[0], p[1], -stand(r, meta_deg)]
    };
    let (s_meta, r_meta) = guess(ankle, meta_pt(r_ankle));
    let meta = meta_pt(r_meta);
    let claw = bare.cheek(claw_deg, spec.shares[2], -CLAW_SINK_MM);
    let (s_claw, _) = guess(meta, claw);
    let tibia = cheek_run(knee, ankle, r_knee, r_ankle, &|u, th| {
        let r = lerp(r_knee, r_ankle, u);
        lerp(knee[2].abs(), stand(r, th), 1.0 - (1.0 - u).powf(2.2))
    });
    let metatarsus = cheek_run(ankle, meta, r_ankle, r_meta, &|u, th| stand(lerp(r_ankle, r_meta, u), th));
    let run = s_claw - s_meta;
    let tarsus = cheek_run(meta, claw, r_meta, CLAW_MM, &|u, th| {
        let r = lerp(r_meta, CLAW_MM, u);
        let dive = smoothstep(1.0 - (1.8 / run).min(0.85), 1.0, u);
        lerp(stand(r, th), bare.half_width(th) - CLAW_SINK_MM, dive)
    });
    // What the leg measures.
    let total = femur.length() + tibia.length() + metatarsus.length() + tarsus.length();
    let knee_proud = knee[2].abs() + KNUCKLE * r_knee - ks.half_width;
    let (mut most_proud, mut least_share) = (0.0f64, f64::MAX);
    for (p, r) in [&tibia, &metatarsus, &tarsus].into_iter().flat_map(|path| path.samples()) {
        let sec = bare.at(theta_of(p));
        most_proud = most_proud.max(p[2].abs() + r - sec.half_width);
        least_share = least_share.min((p[0].hypot(p[1]) - sec.bore_r) / (sec.edge_r - sec.bore_r));
    }
    let segments = vec![("femur", femur), ("tibia", tibia), ("metatarsus", metatarsus), ("tarsus", tarsus)];
    let abdomen_margin = segments.iter().flat_map(|(_, path)| path.samples()).map(|(p, r)| abdomen.margin(p) - r).fold(f64::MAX, f64::min);
    Ok(Leg {
        segments,
        joints: vec![("knee", knee, KNUCKLE * r_knee), ("ankle", ankle, KNUCKLE * r_ankle), ("metatarsal joint", meta, KNUCKLE * r_meta)],
        knee_deg,
        knee_proud,
        most_proud,
        least_share,
        abdomen_margin,
        length: total,
    })
}

/// The four legs on the low side, each solved again until its taper runs its own length.
fn solve_legs(bare: &Bare, carapace: &Body, abdomen: &Body) -> Result<Vec<(LegSpec, Leg)>> {
    LEGS.iter()
        .map(|spec| {
            let mut leg = solve_leg(spec, bare, carapace, abdomen, 10.0)?;
            for _ in 0..3 {
                leg = solve_leg(spec, bare, carapace, abdomen, leg.length)?;
            }
            for (part, p) in &leg.segments {
                if std::env::var("ARACHNE_DEBUG").is_ok() {
                    let bends: Vec<String> = p.pieces.iter().map(|c| format!("({:.2},{:.2}):{:.2}", c[0][0], c[0][1], tightest_bend(c))).collect();
                    println!("    {} {part}: {}", spec.name, bends.join(" "));
                }
                ensure!(p.bend_ratio() >= 1.15, "{} {part} bends at {:.2} of its radius", spec.name, p.bend_ratio());
            }
            let radii: Vec<String> = leg.segments.iter().map(|(_, p)| format!("{:.2}", p.r0)).chain([format!("{CLAW_MM:.2}")]).collect();
            let bends: Vec<String> = leg.segments.iter().map(|(_, p)| format!("{:.1}", p.bend_ratio())).collect();
            println!(
                "  {}: knee at {:.1} deg standing {:.2} proud; {:.1} mm long; radii {}; bends {}; most proud {:.2}; lowest share {:.2}; abdomen margin {}",
                spec.name,
                leg.knee_deg,
                leg.knee_proud,
                leg.length,
                radii.join(" "),
                bends.join(" "),
                leg.most_proud,
                leg.least_share,
                if leg.abdomen_margin < 1e9 { format!("{:.2}", leg.abdomen_margin) } else { "none".into() },
            );
            Ok((*spec, leg))
        })
        .collect()
}

// --- Assembly ---------------------------------------------------------------

/// Appends features to the design's parts, numbering them on.
struct Parts<'a> {
    doc: &'a mut Document,
    next: Id,
}

impl Parts<'_> {
    fn add(&mut self, name: impl Into<String>, operation: Operation, component: Component) -> Result<Id> {
        let id = self.next;
        self.next += 1;
        self.doc.append(Feature { id, name: name.into(), enabled: true, operation, component })?;
        Ok(id)
    }
    /// A ball of `radius` at a world point, joined.
    fn ball(&mut self, name: &str, centre: P3, radius: f64) -> Result<Id> {
        let s = self.add(format!("{name} ball"), Operation::Sphere { radius_mm: radius }, Component::default())?;
        self.add(name, Operation::Transform { source: s, translation: centre, rotation_deg: [0.0; 3] }, joined(0.0))
    }
}

fn joined(blend_mm: f64) -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm, ..Component::default() }
}

/// What the spider measures, for the report.
#[derive(Default, serde::Serialize)]
struct Spider {
    carapace_deg: f64,
    abdomen_deg: f64,
    spinneret_deg: f64,
    knee_deg: Vec<f64>,
    claw_deg: Vec<f64>,
    knee_proud_mm: f64,
    /// The highest knee's top over the carapace collet's rim, along the ring's radius.
    knee_over_rim_mm: f64,
    leg_most_proud_mm: f64,
    leg_lowest_cheek_share: f64,
    leg_abdomen_margin_mm: f64,
    leg_lengths_mm: Vec<f64>,
    femur_tibia_min_diameter_mm: f64,
    claw_diameter_mm: f64,
    tightest_bend_ratio: f64,
}

fn author() -> Result<(RingDesign, AlphaLibrary, Spider)> {
    let mut d = band();
    let ctx = d.field_context();
    println!("  reference crest v {:.2} of {:.2}, crest r {:.2}", ctx.crest_v_mm, ctx.band_v_len_mm, ctx.crest_radius_mm);
    let bare = Bare::new(&d);
    let abdomen_at = abdomen_deg(&bare);
    let mut head = LayerEntry::new("Carapace bed, garnet", Layer::SeatPad(bed(HUB_DEG, ctx.crest_v_mm, garnet(), CARAPACE_RISE_MM)));
    head.blend = Blend::SmoothMax;
    let mut tail = LayerEntry::new("Abdomen bed, onyx", Layer::SeatPad(bed(abdomen_at, ctx.crest_v_mm, onyx(), ABDOMEN_RISE_MM)));
    tail.blend = Blend::SmoothMax;
    cheek_web(&mut d)?;
    d.layers.layers.push(head);
    d.layers.layers.push(tail);
    let (carapace, abdomen) = bodies(&d)?;
    let spinneret_deg = crown_web(&mut d, &bare, &abdomen)?;
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    println!("  carapace at {HUB_DEG:.1} deg, abdomen at {abdomen_at:.1} deg, spinnerets at {spinneret_deg:.1} deg");

    let legs = solve_legs(&bare, &carapace, &abdomen)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let mut parts = Parts { doc, next };
    parts.add("Carapace body", belly(&carapace)?, joined(0.0))?;
    parts.add("Abdomen body", belly(&abdomen)?, joined(0.0))?;
    parts.add("Carapace collet, garnet", collet(&carapace)?, joined(0.0))?;
    parts.add("Abdomen collet, onyx", collet(&abdomen)?, joined(0.0))?;
    // The pedicel: a waist between the collets, a little under the girdles.
    let (ec, ea) = (carapace.end(1.0), abdomen.end(-1.0));
    let mid: P3 = std::array::from_fn(|k| 0.5 * (ec[k] + ea[k]));
    parts.ball("Pedicel", add3(mid, unit3([mid[0], mid[1], 0.0]), -0.35), 0.7)?;
    let sp = bare.at(spinneret_deg);
    parts.ball("Spinnerets", at_ring(spinneret_deg, sp.flank[0][1] + 0.18, 0.0), 0.5)?;
    let mut spider = Spider { knee_over_rim_mm: f64::MIN, carapace_deg: HUB_DEG, abdomen_deg: abdomen_at, spinneret_deg, tightest_bend_ratio: f64::MAX, femur_tibia_min_diameter_mm: f64::MAX, leg_abdomen_margin_mm: f64::MAX, leg_lowest_cheek_share: f64::MAX, claw_diameter_mm: 2.0 * CLAW_MM, ..Spider::default() };
    for (spec, leg) in &legs {
        let mut outputs = Vec::new();
        for (part, path) in &leg.segments {
            let op = Operation::Twist { sketch: path.section().into(), path: path.sketch(&format!("{} {part} path", spec.name)), degrees: 0.0, end_scale: path.r1 / path.r0 };
            outputs.push(parts.add(format!("{} {part}, low side", spec.name), op, joined(0.0))?);
        }
        for (joint, centre, radius) in &leg.joints {
            outputs.push(parts.ball(&format!("{} {joint}, low side", spec.name), *centre, *radius)?);
        }
        for source in outputs {
            let name = parts.doc.feature(source).map(|f| f.name.replace("low side", "high side")).unwrap_or_default();
            parts.add(name, Operation::Pattern { source, kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, joined(0.0))?;
        }
        spider.knee_deg.push(leg.knee_deg);
        spider.claw_deg.push(HUB_DEG + spec.claw_deg);
        spider.knee_proud_mm = spider.knee_proud_mm.max(leg.knee_proud);
        let (knee, radius) = (leg.joints[0].1, leg.joints[0].2);
        let rim = carapace.world([0.0, 0.0, carapace.rim()]);
        spider.knee_over_rim_mm = spider.knee_over_rim_mm.max(knee[0].hypot(knee[1]) + radius - rim[0].hypot(rim[1]));
        spider.leg_most_proud_mm = spider.leg_most_proud_mm.max(leg.most_proud);
        spider.leg_lowest_cheek_share = spider.leg_lowest_cheek_share.min(leg.least_share);
        spider.leg_abdomen_margin_mm = spider.leg_abdomen_margin_mm.min(leg.abdomen_margin);
        spider.leg_lengths_mm.push(leg.length);
        spider.femur_tibia_min_diameter_mm = spider.femur_tibia_min_diameter_mm.min(2.0 * leg.segments[1].1.r1);
        spider.tightest_bend_ratio = leg.segments.iter().map(|(_, p)| p.bend_ratio()).fold(spider.tightest_bend_ratio, f64::min);
    }
    Ok((d, lib, spider))
}

// --- Gates, report and renders ----------------------------------------------

/// The ring's gates and figures, as report.json carries them.
#[derive(serde::Serialize)]
struct Report {
    name: String,
    process: String,
    size: String,
    bore_mm: f64,
    build: [usize; 2],
    triangles: usize,
    build_s: f64,
    watertight: bool,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    made_parts: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    parts_joined: usize,
    field_verdict: String,
    field_notes: Vec<String>,
    /// Undercut read on the CAD parts against a two-part pull; reported under lost wax, never gating.
    parts_undercut_mm2: f64,
    parts_area_mm2: f64,
    undercut_percent: f64,
    worst_draft_deg: f64,
    thinnest_wall_mm: f64,
    thinnest_wall_theta_deg: f64,
    investment_min_section_mm: f64,
    clamp_bite_mm: Option<f64>,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    metal_inside_stones: Vec<(String, usize)>,
    stone_carats: f64,
    stone_warnings: Vec<String>,
    closest_stones: Option<String>,
    /// Along the finger, the whole ring's reach and the head's bare width.
    z_extent_mm: f64,
    head_width_mm: f64,
    spider: Spider,
    cheek_web_ink: f64,
    crown_web_ink: Vec<(String, f64)>,
    longest_bare_crown_run_mm: f64,
    layers: Vec<String>,
    cad_features: usize,
    grams_18k: f64,
    cold_reload_identical: Option<bool>,
    gates_passed: bool,
}

/// The preview stones split by tint, each welded and smoothed so a cabochon reads as polished.
fn stones(d: &RingDesign, lib: &AlphaLibrary) -> Vec<(mesh::Mesh, [f32; 3])> {
    let v = ringdesign_core::gems::preview_vertices(d, lib);
    const STRIDE: usize = 12;
    let mut groups: Vec<([f32; 3], Vec<[f32; 3]>)> = Vec::new();
    for tri in v.chunks_exact(STRIDE * 3) {
        let tint = [tri[6], tri[7], tri[8]];
        let at = match groups.iter().position(|(t, _)| *t == tint) {
            Some(i) => i,
            None => {
                groups.push((tint, Vec::new()));
                groups.len() - 1
            }
        };
        for k in 0..3 {
            groups[at].1.push([tri[k * STRIDE], tri[k * STRIDE + 1], tri[k * STRIDE + 2]]);
        }
    }
    groups
        .into_iter()
        .map(|(tint, pts)| {
            let mut m = mesh::Mesh::default();
            let mut index = std::collections::HashMap::new();
            for tri in pts.chunks_exact(3) {
                let f = std::array::from_fn(|k| {
                    let key = tri[k].map(|c| (c * 1e4).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        m.vertices.push(mesh::Vec3(tri[k][0], tri[k][1], tri[k][2]));
                        (m.vertices.len() - 1) as u32
                    })
                });
                m.faces.push(f);
            }
            m.normals = vec![mesh::Vec3(0.0, 0.0, 0.0); m.vertices.len()];
            for f in &m.faces {
                let [a, b, c] = f.map(|i| m.vertices[i as usize]);
                let (e1, e2) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                for &i in f {
                    let v = &mut m.normals[i as usize];
                    v.0 += n[0];
                    v.1 += n[1];
                    v.2 += n[2];
                }
            }
            for n in &mut m.normals {
                let l = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-12);
                *n = mesh::Vec3(n.0 / l, n.1 / l, n.2 / l);
            }
            (m, tint)
        })
        .collect()
}

/// The faces of `m` within `radius` of `centre`, as a mesh of their own, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
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


/// Studio-gold renders with stones: named views, a spider close-up, and bare stock against the finished ring.
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let gems = stones(d, lib);
    let stone_parts = || {
        gems.iter()
            .map(|(m, tint)| {
                let mut p = render::Part::stone(m);
                p.tint = *tint;
                p.smooth = true;
                p
            })
            .collect::<Vec<_>>()
    };
    let mut parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    parts.extend(stone_parts());
    for (name, yaw, pitch) in [
        ("hero", 0.55, 0.95),
        ("face", 0.0, PI * 0.5),
        ("palm", PI, 1.05),
        ("side", 0.0, 0.0),
        ("shoulder", -0.9, 0.62),
        ("reverse", 1.6, 0.8),
    ] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The close-up frames on the metal round the stones and draws the whole ring.
    let frames = ringdesign_core::stones::stone_frames(d);
    let centre: P3 = std::array::from_fn(|k| frames.iter().map(|(_, f)| f.girdle[k]).sum::<f64>() / frames.len().max(1) as f64);
    let spider = crop(&built.mesh, centre, 11.0);
    let mut close = vec![render::Part::metal(&spider, render::GOLD), render::Part::metal(&built.mesh, render::GOLD)];
    close.extend(stone_parts());
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = band();
    bare.name = d.name.clone();
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    Ok(())
}

/// Every made part's self-crossings: the CAD parts as placed, the collets among them.
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

/// Metal vertices standing inside each cabochon, 0.03 mm in from its surface: the stones must sit clear.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    ringdesign_core::stones::stone_frames(d)
        .into_iter()
        .map(|(st, f)| {
            let (a, b, h) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03, st.gem.depth_mm() - 0.03);
            let inside = built
                .mesh
                .vertices
                .iter()
                .filter(|p| {
                    let q = sub3([p.0 as f64, p.1 as f64, p.2 as f64], f.girdle);
                    let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                    z > 0.03 && z < h && (x / a).powi(2) + (y / b).powi(2) < 1.0 - (z / h).powi(2)
                })
                .count();
            (st.label, inside)
        })
        .collect()
}

/// Share of an alpha's texels that are ink.
fn ink(lib: &AlphaLibrary, name: &str) -> f64 {
    lib.get(name).map_or(0.0, |a| a.data.iter().filter(|v| **v >= 0.5).count() as f64 / a.data.len().max(1) as f64)
}

/// The longest run of bare polished crown along the crest and two lines either side, clear of the two bodies, metal mm.
fn longest_bare_run(d: &RingDesign, lib: &AlphaLibrary, spider: &Spider) -> f64 {
    let ctx = d.field_context();
    let bare = Bare::new(d);
    let body = |th: f64| {
        let wrap = |a: f64| (a + 180.0).rem_euclid(360.0) - 180.0;
        wrap(th - spider.carapace_deg) > -16.0 && wrap(th - spider.abdomen_deg) < 28.0
    };
    let mut worst: f64 = 0.0;
    for share in [0.0, 0.5, -0.5] {
        let mut run = 0.0;
        let step = 0.25;
        for k in 0..(720.0 / step) as usize {
            let th = k as f64 * step;
            let t = share * bare.at(th).edge_t;
            let v = ctx.crest_v_mm + t / ctx.station_stretch(th.rem_euclid(360.0));
            let h = d.layers.height(Uv { u: ctx.u_of_theta(th.rem_euclid(360.0)), v }, &ctx, lib);
            let ds = step.to_radians() * bare.r_at_t(th, t);
            if h < 0.03 && !body(th) {
                run += ds;
            } else {
                if run > worst && std::env::var("ARACHNE_DEBUG").is_ok() {
                    println!("    bare run {run:.1} mm ending at {:.1} deg on share {share}", th.rem_euclid(360.0));
                }
                worst = worst.max(run);
                run = 0.0;
            }
        }
    }
    worst
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/arachne"));
    std::fs::create_dir_all(&out)?;
    println!("Arachne");
    let (d, lib, spider) = author()?;
    let params = if draft { draft_params() } else { export_params() };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    println!("  {} triangles in {build_s:.1} s; watertight {}; degenerate {}", built.mesh.faces.len(), v.watertight, built.report.quality.degenerate_faces);
    let made_parts = crossings(&built);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let previewed = stones(&d, &lib).len();
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let mut warnings: Vec<String> = stones_report.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let zs = built.mesh.vertices.iter().map(|p| p.2 as f64);
    let z_extent = zs.clone().fold(f64::MIN, f64::max) - zs.fold(f64::MAX, f64::min);
    let head_width = 2.0 * Bare::new(&d).at(90.0).half_width;
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let crown_web_ink: Vec<(String, f64)> = d.svgs.iter().filter(|s| s.name != "Cheek web").map(|s| (s.name.clone(), ink(&lib, &s.name))).collect();
    let bare_run = longest_bare_run(&d, &lib, &spider);
    let mut report = Report {
        name: d.name.clone(),
        process: d.draft.process.label().into(),
        size: d.size.display(),
        bore_mm: built.report.inner_diameter_mm,
        build: [params.theta_steps, params.profile_steps],
        triangles: built.mesh.faces.len(),
        build_s,
        watertight: v.watertight,
        boundary_edges: v.boundary_edges,
        non_manifold_edges: v.non_manifold_edges,
        degenerate_faces: built.report.quality.degenerate_faces,
        made_parts,
        solids_notes: built.solids.notes.clone(),
        parts_notes: built.parts.notes.clone(),
        parts_joined: built.parts.joined,
        field_verdict: field.verdict.label().into(),
        field_notes: field.notes.clone(),
        parts_undercut_mm2: field.parts.iter().map(|p| p.undercut_area_mm2).sum(),
        parts_area_mm2: field.parts.iter().map(|p| p.total_area_mm2).sum(),
        undercut_percent: field.undercut_fraction() * 100.0,
        worst_draft_deg: field.worst_draft_deg,
        thinnest_wall_mm: field.thinnest_wall_mm,
        thinnest_wall_theta_deg: field.thinnest_wall_theta_deg,
        investment_min_section_mm: d.draft.min_section_mm,
        clamp_bite_mm: None,
        dfm_findings: findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        stones_reported: stones_report.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        metal_inside_stones: metal_in_stones(&d, &built),
        stone_carats: stones_report.as_ref().map_or(0.0, |s| s.total_carats),
        stone_warnings: warnings,
        closest_stones: stones_report.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm)),
        z_extent_mm: z_extent,
        head_width_mm: head_width,
        spider,
        cheek_web_ink: ink(&lib, "Cheek web"),
        crown_web_ink,
        longest_bare_crown_run_mm: bare_run,
        layers: d.layers.layers.iter().map(|e| e.name.clone()).collect(),
        cad_features: d.cad.as_ref().map_or(0, |c| c.features.len()),
        grams_18k: grams,
        cold_reload_identical: cold,
        gates_passed: false,
    };
    report.gates_passed = report.watertight
        && report.degenerate_faces == 0
        && report.made_parts.iter().all(|(_, n)| *n == 0)
        && report.solids_notes.is_empty()
        && report.parts_notes.is_empty()
        && report.field_verdict == castability::Verdict::Castable.label()
        && report.thinnest_wall_mm >= 0.8
        && report.dfm_findings.is_empty()
        && report.stones_reported as usize == report.stones_previewed
        && report.metal_inside_stones.iter().all(|(_, n)| *n == 0)
        && report.z_extent_mm <= 8.8
        && report.spider.knee_proud_mm <= KNEE_PROUD_MM + 0.01
        && report.spider.femur_tibia_min_diameter_mm >= 0.85
        && report.cold_reload_identical != Some(false);
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    let art = out.join("artwork");
    let _ = std::fs::remove_dir_all(&art);
    std::fs::create_dir_all(&art)?;
    for svg in &d.svgs {
        std::fs::write(art.join(format!("{}.svg", svg.name.to_lowercase().replace([' ', ','], "-").replace("--", "-"))), &svg.svg)?;
    }
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    }
    renders(&out, &d, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} ({:.4}% undercut), thinnest wall {:.2} mm at {:.0} deg; dfm {}; stones {} reported, {} previewed; made parts {} with crossings; solids notes {}; parts notes {}",
        report.field_verdict,
        report.undercut_percent,
        report.thinnest_wall_mm,
        report.thinnest_wall_theta_deg,
        report.dfm_findings.len(),
        report.stones_reported,
        report.stones_previewed,
        report.made_parts.iter().filter(|(_, n)| *n > 0).count(),
        report.solids_notes.len(),
        report.parts_notes.len()
    );
    println!(
        "  z extent {:.2} mm over a {:.2} mm head; knees {:.2} proud, legs {:.2}; cheek ink {:.2}; crown ink {:?}; longest bare crown run {:.1} mm",
        report.z_extent_mm, report.head_width_mm, report.spider.knee_proud_mm, report.spider.leg_most_proud_mm, report.cheek_web_ink, report.crown_web_ink, report.longest_bare_crown_run_mm
    );
    for (stone, n) in &report.metal_inside_stones {
        println!("    {stone}: {n} metal vertices inside the stone");
    }
    for f in &report.dfm_findings {
        println!("    dfm: {f}");
    }
    for n in report.parts_notes.iter().chain(&report.solids_notes) {
        println!("    note: {n}");
    }
    println!("  gates {}", if report.gates_passed { "passed" } else { "FAILED" });
    ensure!(report.gates_passed, "Arachne failed its gates; see {}", out.join("report.json").display());
    Ok(())
}

