//! Bestiarium — Arachne, the weaver: a spider at the hub of her own web, clasping the finger, cast in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_arachne
//! target/release/examples/bestiarium_arachne [OUT_DIR] [--draft] [--verify] [--web-preview]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, Blend, BuildParams, FieldContext, Layer, LayerEntry, ProfileLoop,
    ProfileStyle, RingDesign, ShankKind,
    cad::{
        Attach, Component, Document, Feature, MirrorPlane, Operation, PatternKind, Placement,
        SurfaceKind, stored,
    },
    castability::{self, CastProcess},
    csg, dfm,
    field::{Decal, DecalLayer, SeatPadLayer, SeatStyle, SideFacePick, Uv, VGate},
    gem::{Gem, GemCut, GemForm},
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    setting::{self, Plan, SolidKind, Station},
    sketch::{Geometry, Id, Sketch, Workplane},
    skin, stl,
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

/// Ring angle of the garnet carapace, which is also the main orb's hub.
const HUB_DEG: f64 = 64.0;
/// Girdle heights over the bare crest: the carapace, and the prouder abdomen.
const CARAPACE_RISE_MM: f64 = 0.4;
const ABDOMEN_RISE_MM: f64 = 0.4;
/// Clearance round each stone in its pocket, and the pocket floor's height over the girdle, just clear of the bed's crown.
const POCKET_CLEAR_MM: f64 = 0.03;
const POCKET_FLOOR_MM: f64 = 0.01;
/// A body's wall flares outward toward its base by this much; its rim's outer edge is rounded by the other.
const FLARE_DEG: f64 = 9.0;
const RIM_ROUND_MM: f64 = 0.12;
/// Body wall to body wall between carapace and abdomen, at the girdles.
const BODY_GAP_MM: f64 = 1.0;
/// Relief of the crown's radial and capture silk.
const WEB_HEIGHT_MM: f64 = 0.22;
const CAPTURE_HEIGHT_MM: f64 = 0.15;
/// Clear margin before the crown folds, with a further smooth fade inside it.
const EDGE_INSET_MM: f64 = 0.20;
const EDGE_FADE_MM: f64 = 0.40;
/// The main orb round the carapace: its rim, where its spiral starts, its radials and their offset from the ring's axis.
const MAIN_RIM_MM: f64 = 6.2;
const MAIN_SPIRAL_FROM_MM: f64 = 1.9;
const MAIN_PITCH_MM: f64 = 1.15;
const MAIN_RADIALS: usize = 24;
/// The squashed orbs round the ring: ring angle and radials.
const ORBS: [(&str, f64, usize); 3] = [
    ("Orb before her", 15.0, 16),
    ("Orb behind her", 170.0, 16),
    ("Orb at the palm", 270.0, 20),
];
/// The squashed orbs' capture spiral: turns and pitch across the band, and their length over their width.
const ORB_TURNS: f64 = 2.0;
const ORB_PITCH_MM: f64 = 0.6;
const ORB_SQUASH: f64 = 1.45;
/// The frieze of web corners between the orbs: the zig-zag's step along the ring, spokes per corner, and its capture arcs' first radius and pitch.
const FRIEZE_STEP_MM: f64 = 4.4;
const FRIEZE_SPOKES: usize = 3;
const FRIEZE_ARC_FROM_MM: f64 = 1.25;
const FRIEZE_ARC_PITCH_MM: f64 = 1.35;
/// Stroke widths: orb radials and spiral, frames, frieze spokes and capture threads.
const W_ORB_MM: f64 = 0.30;
const W_FRAME_MM: f64 = 0.27;
const W_SPOKE_MM: f64 = 0.23;
const W_CAPTURE_MM: f64 = 0.21;
/// The cheek web's radial pitch and thread widths.
const CHEEK_PITCH_MM: f64 = 1.3;
const CHEEK_RADIAL_MM: f64 = 0.21;
const CHEEK_THREAD_MM: f64 = 0.19;
/// Most a knee may stand past the bare cheek, and the ring's half-reach along the finger (its reach at most 8.8 mm).
const KNEE_PROUD_MM: f64 = 0.6;
const HALF_REACH_MM: f64 = 4.38;
/// The legs' local radius at the knee, the ankle and the metatarsal joint, and at the claw's tip.
const KNEE_MM: f64 = 0.52;
const ANKLE_MM: f64 = 0.47;
const META_MM: f64 = 0.43;
const CLAW_MM: f64 = 0.40;
/// The femur's radius where it leaves its coxa.
const FEMUR_MM: f64 = 0.58;
/// Knuckles over the joint's radius, and the coxa's ball.
const KNUCKLE: f64 = 1.12;
const COXA_BALL_MM: f64 = 0.6;
/// The continuous joint loft replaces this much of each neighbouring segment.
const JOINT_EASE_MM: f64 = 0.35;
/// How far the knee stands over the crown's edge past riding it, so the femur arches up to it.
const KNEE_LIFT_MM: f64 = 1.7;
/// Where the legs root on the carapace's flanks, across the band.
const ROOT_ACROSS_MM: f64 = 1.6;
/// The tarsal claw: the hook's length and its turn toward the band, and how deep its tip sits under the cheek.
const HOOK_MM: f64 = 0.8;
const HOOK_DEG: f64 = 35.0;
const TIP_SINK_MM: f64 = 0.36;
/// Thinnest section any leg may carry, the investment's fill floor.
const MIN_SECTION_MM: f64 = 0.8;

fn draft_params() -> BuildParams {
    BuildParams {
        theta_steps: 768,
        profile_steps: 320,
        ..BuildParams::default()
    }
}
fn export_params() -> BuildParams {
    BuildParams {
        theta_steps: 1536,
        profile_steps: 448,
        ..BuildParams::default()
    }
}

/// The keyed LowDome band, squared at the cheeks: broad and deep under the spider, slim at the palm.
fn band() -> RingDesign {
    let mut d = RingDesign {
        name: "Arachne \u{2014} the weaver".into(),
        ..RingDesign::default()
    };
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 5.6;
    d.profile.thickness_mm = 2.4;
    d.profile.crown_mm = 0.72;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey {
        theta_deg,
        width_scale,
        thickness_scale,
        crown_scale: 1.0,
    };
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
    d.draft.min_section_mm = MIN_SECTION_MM;
    d
}

fn garnet() -> Gem {
    Gem {
        cut: GemCut::Oval,
        w_mm: 3.5,
        l_mm: 5.0,
        form: GemForm::Cabochon,
        preview_tint: Some([0.42, 0.02, 0.05]),
    }
}
fn onyx() -> Gem {
    let mut g = Gem {
        l_mm: 9.0,
        ..Gem::cabochon(GemCut::Oval, 7.0)
    };
    g.preview_tint = Some([0.015, 0.015, 0.02]);
    g
}

/// A stone's bed: a low pad hidden inside the stone's body that holds its girdle `rise` over the crest.
fn bed(theta: f64, v: f64, gem: Gem, rise: f64) -> SeatPadLayer {
    let mut s = SeatPadLayer {
        theta_deg: theta,
        v_mm: v,
        style: SeatStyle::Boss,
        metal_true: true,
        solid: SolidKind::None,
        blend_mm: 0.3,
        crown: 1.0,
        ..Default::default()
    };
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
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
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
fn lerp2(a: P2, b: P2, t: f64) -> P2 {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t)]
}
fn dist2(a: P2, b: P2) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}
/// An angle folded onto (-180, 180].
fn wrap180(a: f64) -> f64 {
    180.0 - (180.0 - a).rem_euclid(360.0)
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
        Self {
            d: d.clone(),
            reference: d.reference_loop(),
            cache: RefCell::new(HashMap::new()),
        }
    }

    fn at(&self, theta: f64) -> Rc<Section> {
        let n = (360.0 / Self::STEP) as i64;
        let k = ((theta / Self::STEP).round() as i64).rem_euclid(n);
        if let Some(s) = self.cache.borrow().get(&k) {
            return s.clone();
        }
        let l = self
            .d
            .section_at(k as f64 * Self::STEP, 256, None, Some(&self.reference));
        let loop_rz: Vec<P2> = l.pts.iter().map(|p| [p.r, p.z]).collect();
        let half_width = loop_rz.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
        let bore_r = loop_rz.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let crest = l
            .pts
            .iter()
            .filter(|p| p.surface)
            .max_by(|a, b| a.r.total_cmp(&b.r))
            .expect("a section with an outer surface");
        let (cz, cv) = (crest.z, crest.v_mm);
        let mut rows: Vec<[f64; 4]> = l
            .pts
            .iter()
            .filter(|p| p.surface && p.z <= cz + 1e-9)
            .map(|p| [(p.v_mm - cv).abs(), p.r, p.z, p.nz])
            .collect();
        rows.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let side = rows
            .iter()
            .find(|p| p[3].abs() >= 80f64.to_radians().sin())
            .copied()
            .unwrap_or(*rows.last().unwrap());
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

    /// Radius and place along the finger of the outer surface `t` of arc from the crest, on the side `t`'s sign names.
    fn surface_at_t(&self, theta: f64, t: f64) -> (f64, f64) {
        let s = self.at(theta);
        let f = &s.flank;
        let ta = t.abs().clamp(0.0, f.last().unwrap()[0]);
        let i = f.partition_point(|p| p[0] < ta).clamp(1, f.len() - 1);
        let (a, b) = (f[i - 1], f[i]);
        let u = ((ta - a[0]) / (b[0] - a[0]).max(1e-12)).clamp(0.0, 1.0);
        let z = lerp(a[2], b[2], u);
        (lerp(a[1], b[1], u), if t > 0.0 { -z } else { z })
    }

    /// Radius of the outer surface `t` of arc from the crest.
    fn r_at_t(&self, theta: f64, t: f64) -> f64 {
        self.surface_at_t(theta, t).0
    }

    /// Radius of the crown where it stands `across` mm from the band's mid-plane.
    fn crown_r(&self, theta: f64, across: f64) -> f64 {
        let s = self.at(theta);
        let z = -across.abs();
        let f = &s.flank;
        for w in f.windows(2) {
            if w[1][2] <= z {
                return lerp(
                    w[0][1],
                    w[1][1],
                    ((z - w[0][2]) / (w[1][2] - w[0][2]).min(-1e-12)).clamp(0.0, 1.0),
                );
            }
        }
        s.edge_r
    }

    /// A point on the low cheek: `share` of its height over the bore, `off` out from its face.
    fn cheek(&self, theta: f64, share: f64, off: f64) -> P3 {
        let s = self.at(theta);
        at_ring(
            theta,
            s.bore_r + share * (s.edge_r - s.bore_r),
            -(s.half_width + off),
        )
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
            if (a[1] > q[1]) != (b[1] > q[1])
                && q[0] < a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
            {
                inside = !inside;
            }
            let ab = [b[0] - a[0], b[1] - a[1]];
            let t = (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1])
                / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-18))
            .clamp(0.0, 1.0);
            near = near.min((q[0] - a[0] - ab[0] * t).hypot(q[1] - a[1] - ab[1] * t));
        }
        if inside { -near } else { near }
    }

    /// The band's half-width at a ring angle, read between the cached sections.
    fn half_width(&self, theta: f64) -> f64 {
        let k = (theta / Self::STEP).floor();
        let u = theta / Self::STEP - k;
        lerp(
            self.at(k * Self::STEP).half_width,
            self.at((k + 1.0) * Self::STEP).half_width,
            u,
        )
    }

    /// The bore wall at a vertex's ring angle and position along the finger.
    fn bore_at(&self, theta: f64, z: f64) -> Option<f64> {
        let s = self.at(theta);
        if z.abs() > s.half_width {
            return None;
        }
        s.loop_rz
            .iter()
            .zip(s.loop_rz.iter().cycle().skip(1))
            .take(s.loop_rz.len())
            .filter_map(|(a, b)| {
                let dz = b[1] - a[1];
                if dz.abs() < 1e-12 || z < a[1].min(b[1]) || z > a[1].max(b[1]) {
                    None
                } else {
                    Some(lerp(a[0], b[0], (z - a[1]) / dz))
                }
            })
            .min_by(f64::total_cmp)
    }

    /// How far a leg may stand past the cheek at a ring angle: within the ring's reach, and never more than 0.8.
    fn proud(&self, theta: f64) -> f64 {
        smin(HALF_REACH_MM - 0.03 - self.half_width(theta), 0.8, 0.05)
    }
}

// --- Bodies -----------------------------------------------------------------

/// One stone and the body built round it, in the stone's own frame.
#[derive(Clone)]
struct Body {
    gem: Gem,
    f: StoneFrame,
    /// Which way along `f.long` the ring angle climbs.
    ahead: f64,
    /// The rim's height over the girdle, as a share of the stone's crown.
    lip: f64,
    /// The body's widest reach past the girdle outline, at its equator.
    widest: f64,
    /// The equator's height over the girdle.
    equator: f64,
    /// The fall from the equator into the band: how far it draws in, and over what depth.
    fall: (f64, f64),
}

impl Body {
    fn semi(&self) -> (f64, f64) {
        (self.gem.l_mm * 0.5, self.gem.w_mm * 0.5)
    }
    /// A local point of the stone's frame in the world.
    fn world(&self, p: P3) -> P3 {
        let f = &self.f;
        std::array::from_fn(|k| {
            f.girdle[k] + f.long[k] * p[0] + f.short[k] * p[1] + f.normal[k] * p[2]
        })
    }
    /// A world point in the stone's frame.
    fn local(&self, w: P3) -> P3 {
        let d = sub3(w, self.f.girdle);
        [
            dot(d, self.f.long),
            dot(d, self.f.short),
            dot(d, self.f.normal),
        ]
    }
    /// Height of the rim's top over the girdle.
    fn rim(&self) -> f64 {
        setting::girdle_half_mm(self.gem) + self.lip * self.gem.crown_mm()
    }
    /// The stone's plan at a height over its girdle, as a share of the girdle's.
    fn stone_scale(&self, z: f64) -> f64 {
        let c = self.gem.crown_mm().max(1e-6);
        (1.0 - (z / c).clamp(0.0, 1.0).powi(2)).sqrt()
    }
    /// The outer surface as (reach past the girdle outline, height over the girdle), from buried in the band up to the rim's top:
    /// one convex fall into the band, rolling through the equator into a wall that flares toward its base, and a rounded rim.
    fn profile(&self) -> Vec<P2> {
        let (inset, depth) = self.fall;
        let slope = FLARE_DEG.to_radians().tan();
        let psi_t = (slope * depth / inset).atan();
        let centre = self.widest - inset;
        let mut rows = vec![[centre, self.equator - depth - 0.45]];
        let n = 11;
        for i in 0..=n {
            let psi = lerp(-0.5 * PI, psi_t, i as f64 / n as f64);
            rows.push([centre + inset * psi.cos(), self.equator + depth * psi.sin()]);
        }
        let [o_t, z_t] = *rows.last().unwrap();
        let top = self.rim() - RIM_ROUND_MM;
        let o_top = o_t - (top - z_t) * slope;
        rows.push([lerp(o_t, o_top, 0.5), lerp(z_t, top, 0.5)]);
        for j in 0..=4 {
            let a = 0.5 * PI * j as f64 / 4.0;
            rows.push([
                o_top - RIM_ROUND_MM * (1.0 - a.cos()),
                top + RIM_ROUND_MM * a.sin(),
            ]);
        }
        rows
    }
    /// The body's outer surface at a height over the girdle, as a reach past the girdle outline; `None` above its rim or below its foot.
    fn wall(&self, z: f64) -> Option<f64> {
        let p = self.profile();
        if z < p[0][1] || z > p.last().unwrap()[1] {
            return None;
        }
        let i = p.partition_point(|r| r[1] < z).clamp(1, p.len() - 1);
        let (a, b) = (p[i - 1], p[i]);
        Some(lerp(
            a[0],
            b[0],
            ((z - a[1]) / (b[1] - a[1]).max(1e-12)).clamp(0.0, 1.0),
        ))
    }
    /// The end of the body facing along the ring at the girdle, `sign` +1 toward higher angles.
    fn end(&self, sign: f64) -> P3 {
        let (a, _) = self.semi();
        self.world([
            sign * self.ahead * (a + self.wall(0.0).unwrap_or(self.widest)),
            0.0,
            0.0,
        ])
    }
    /// How far a world point stands outside the body at its own height, while beside it.
    fn margin(&self, w: P3) -> f64 {
        let q = self.local(w);
        let Some(o) = self.wall(q[2]) else {
            return f64::MAX;
        };
        let (a, b) = self.semi();
        let rho = ((q[0] / (a + o)).powi(2) + (q[1] / (b + o)).powi(2)).sqrt();
        (rho - 1.0) * (b + o)
    }
}

/// The two stones' frames as the stone record places them, each with its body.
fn bodies(d: &RingDesign) -> Result<(Body, Body)> {
    let frames = ringdesign_core::stones::stone_frames(d);
    ensure!(
        frames.len() == 2,
        "Arachne sets two stones, not {}",
        frames.len()
    );
    let body = |i: usize, gem: Gem, lip: f64, widest: f64, equator: f64, fall: (f64, f64)| {
        let f = frames[i].1.clone();
        let tangent = at_ring(theta_of(f.girdle) + 90.0, 1.0, 0.0);
        let ahead = dot(f.long, tangent).signum();
        Body {
            gem,
            f,
            ahead,
            lip,
            widest,
            equator,
            fall,
        }
    };
    Ok((
        body(0, garnet(), 0.22, 0.50, -0.2, (1.2, 1.0)),
        body(1, onyx(), 0.16, 0.47, -0.35, (1.9, 1.5)),
    ))
}

/// The abdomen's ring angle that leaves `BODY_GAP_MM` between the two bodies.
fn abdomen_deg(bare: &Bare) -> f64 {
    let rc = bare.at(HUB_DEG).flank[0][1] + CARAPACE_RISE_MM;
    let ra = bare.at(HUB_DEG + 44.0).flank[0][1] + ABDOMEN_RISE_MM;
    let reach = |g: Gem, widest: f64| g.l_mm * 0.5 + widest;
    HUB_DEG
        + ((reach(garnet(), 0.43) / rc).atan()
            + (reach(onyx(), 0.37) / ra).atan()
            + BODY_GAP_MM / (0.5 * (rc + ra)))
            .to_degrees()
}

/// A path point on a 10 nm grid, so the file carries no digits the build cannot see.
fn snap(p: P2) -> P2 {
    p.map(|v| (v * 1e5).round() / 1e5)
}

/// Points round each body's plan: the onyx's and the garnet's.
const ONYX_AROUND: usize = 112;
const GARNET_AROUND: usize = 80;

/// A body's section in `setting::sweep`'s terms: from the pocket floor's centre out to its wall, up the stone's own
/// dome to the rim, over the rim's flat, and down the outside to the axis deep in the band.
fn body_section(body: &Body) -> Vec<Station> {
    let st = |s: f64, o: f64, z: f64| Station { s, o, z };
    let rim = body.rim();
    let f = POCKET_FLOOR_MM;
    let mut out = vec![st(0.0, 0.0, f), st(0.3, 0.0, f), st(0.65, 0.0, f)];
    for i in 0..=3 {
        let z = lerp(f, rim, i as f64 / 3.0);
        out.push(st(body.stone_scale(z), POCKET_CLEAR_MM, z));
    }
    let profile = body.profile();
    out.extend(profile.iter().rev().map(|p| st(1.0, p[0], p[1])));
    out.push(st(0.0, 0.0, profile[0][1]));
    out
}

/// A body swept round its stone's plan as one closed solid, packed as a stored part at the stone.
fn body_part(body: &Body, around: usize) -> Result<Operation> {
    let solid = setting::sweep(&Plan::of(body.gem), &body_section(body), around);
    ensure!(
        solid.open_edges() == (0, 0),
        "{}: the body's sweep does not close",
        body.gem.display()
    );
    ensure!(
        csg::self_crossings(&solid) == 0,
        "{}: the body's sweep crosses itself",
        body.gem.display()
    );
    let flip = dot(cross3(body.f.long, body.f.short), body.f.normal) < 0.0;
    let positions: Vec<P3> = solid.v.iter().map(|p| body.world(*p)).collect();
    let triangles: Vec<[u32; 3]> = solid
        .f
        .iter()
        .map(|t| if flip { [t[0], t[2], t[1]] } else { *t })
        .collect();
    let mesh = stored::Packed::encode(
        &positions,
        &triangles,
        &vec![0; triangles.len()],
        &[SurfaceKind::Freeform],
    )?;
    let recipe = stored::Recipe {
        kernel: "setting".into(),
        op: "body".into(),
        params: serde_json::json!({ "stone": body.gem.display(), "lip": body.lip, "widest_mm": body.widest, "equator_mm": body.equator, "fall_mm": [body.fall.0, body.fall.1], "flare_deg": FLARE_DEG, "around": around }),
        digest: String::new(),
    };
    Ok(Operation::Stored {
        recipe,
        sources: Vec::new(),
        mesh,
    })
}

// --- The web ----------------------------------------------------------------

/// A thread of web in the chart: (ring angle, signed arc from the crest) points, and its width.
#[derive(Clone)]
struct Thread {
    pts: Vec<P2>,
    w: f64,
}

/// A dew bead or a hub's mat: its centre and radius.
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
        let i = self
            .theta
            .partition_point(|t| *t < theta)
            .clamp(1, self.theta.len() - 1);
        lerp(
            self.s[i - 1],
            self.s[i],
            (theta - self.theta[i - 1]) / (self.theta[i] - self.theta[i - 1]),
        )
    }
    fn theta_of(&self, s: f64) -> f64 {
        let i = self
            .s
            .partition_point(|t| *t < s)
            .clamp(1, self.s.len() - 1);
        lerp(
            self.theta[i - 1],
            self.theta[i],
            (s - self.s[i - 1]) / (self.s[i] - self.s[i - 1]).max(1e-12),
        )
    }
}

/// Web space: metal millimetres along the crest from the main hub, and arc across the crown from the crest.
struct Web<'a> {
    ch: &'a Chart<'a>,
    along: Along,
    /// The crest's circumference.
    circ: f64,
}

impl<'a> Web<'a> {
    fn new(ch: &'a Chart<'a>) -> Self {
        let along = Along::new(ch, HUB_DEG);
        let circ = along.s_of(HUB_DEG + 360.0) - along.s_of(HUB_DEG);
        Self { ch, along, circ }
    }
    /// An along-coordinate folded onto the circumference centred on `mid`.
    fn wrap(&self, s: f64, mid: f64) -> f64 {
        mid + (s - mid + 0.5 * self.circ).rem_euclid(self.circ) - 0.5 * self.circ
    }
    fn theta(&self, s: f64) -> f64 {
        self.along.theta_of(self.wrap(s, 0.0))
    }
    fn s_of(&self, theta: f64) -> f64 {
        self.along.s_of(HUB_DEG + wrap180(theta - HUB_DEG))
    }
    /// The crown's half-width at `s`, and the web's inboard anchor line.
    fn edge(&self, s: f64) -> f64 {
        self.ch.edge(self.theta(s))
    }
    fn reach(&self, s: f64) -> f64 {
        self.edge(s) - EDGE_INSET_MM
    }
}

/// A thread in web space; `radial` when it runs from an orb's hub.
struct Strand {
    pts: Vec<P2>,
    w: f64,
    radial: bool,
}

/// Appends points, skipping any that repeats the last.
fn extend(pts: &mut Vec<P2>, more: impl IntoIterator<Item = P2>) {
    for p in more {
        if pts.last().is_none_or(|q| dist2(*q, p) > 1e-6) {
            pts.push(p);
        }
    }
}
/// A straight run from `a` to `b`, sampled every `step` mm.
fn seg(a: P2, b: P2, step: f64) -> Vec<P2> {
    let n = (dist2(a, b) / step).ceil().max(1.0) as usize;
    (0..=n).map(|i| lerp2(a, b, i as f64 / n as f64)).collect()
}
/// A thread from `a` to `b` whose middle hangs `depth` toward `toward`.
fn sagged(a: P2, b: P2, toward: P2, depth: f64, step: f64) -> Vec<P2> {
    let mid = lerp2(a, b, 0.5);
    let d = [toward[0] - mid[0], toward[1] - mid[1]];
    let l = d[0].hypot(d[1]).max(1e-12);
    let c = [
        mid[0] + 2.0 * depth * d[0] / l,
        mid[1] + 2.0 * depth * d[1] / l,
    ];
    let n = (dist2(a, b) / step).ceil().max(2.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            [
                u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0],
                u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1],
            ]
        })
        .collect()
}
/// Splits a strand where `keep` fails, keeping the runs of two points or more.
fn clip(s: Strand, keep: &dyn Fn(P2) -> bool) -> Vec<Strand> {
    let mut out = Vec::new();
    let mut run: Vec<P2> = Vec::new();
    for p in s.pts {
        if keep(p) {
            run.push(p);
        } else if run.len() > 1 {
            out.push(Strand {
                pts: std::mem::take(&mut run),
                w: s.w,
                radial: s.radial,
            });
        } else {
            run.clear();
        }
    }
    if run.len() > 1 {
        out.push(Strand {
            pts: run,
            w: s.w,
            radial: s.radial,
        });
    }
    out
}

/// An orb of the web: a hub, its radials, its capture spiral and its frame, squashed to the crown in metal mm.
struct Orb {
    name: &'static str,
    /// Along the crest from the main hub.
    s: f64,
    /// Half-extents along and across at the spiral's last turn.
    a_s: f64,
    a_t: f64,
    radials: usize,
    offset_deg: f64,
    /// The spiral's first turn, as a share of its last.
    spiral_from: f64,
    turns: f64,
    /// The frame thread's reach, as a share of the spiral's last turn.
    rim: f64,
}

impl Orb {
    fn at(&self, rho: f64, phi: f64) -> P2 {
        [
            self.s + rho * self.a_s * phi.cos(),
            rho * self.a_t * phi.sin(),
        ]
    }
    fn phi(&self, k: usize) -> f64 {
        (self.offset_deg + 360.0 * k as f64 / self.radials as f64).to_radians()
    }
    /// A point's reach from the hub, 1 on the spiral's last turn.
    fn rho(&self, web: &Web, p: P2) -> f64 {
        let ds = web.wrap(p[0], self.s) - self.s;
        (ds / self.a_s).hypot(p[1] / self.a_t)
    }
    fn root_radius(&self) -> f64 {
        if self.name == "Main orb" {
            0.9
        } else if self.radials == 20 {
            1.6
        } else {
            1.3
        }
    }
    /// A squashed orb with two open capture turns outside its separate radial roots.
    fn squashed(web: &Web, name: &'static str, theta: f64, radials: usize) -> Self {
        let a_t = (web.ch.edge(theta) - 0.12).min(2.5);
        Self {
            name,
            s: web.s_of(theta),
            a_s: ORB_SQUASH * a_t,
            a_t,
            radials,
            offset_deg: 180.0 / radials as f64,
            spiral_from: (1.0 - ORB_TURNS * ORB_PITCH_MM / a_t)
                .max((if radials == 20 { 1.65 } else { 1.35 }) / a_t),
            turns: ORB_TURNS,
            rim: 1.0 + 0.5 / a_t,
        }
    }
    fn draw(&self, out: &mut Vec<Strand>) {
        let n = self.radials;
        for k in 0..n {
            let phi = self.phi(k);
            let scale = (self.a_s * phi.cos()).hypot(self.a_t * phi.sin());
            out.push(Strand {
                pts: seg(
                    self.at(self.root_radius() / scale, phi),
                    self.at(self.rim, phi),
                    0.05,
                ),
                w: W_ORB_MM,
                radial: true,
            });
        }
        if self.name == "Main orb" {
            // The concealed hub under the garnet retains its original construction.
            let hub: Vec<P2> = (0..6)
                .map(|k| {
                    let phi = self.phi(0) + k as f64 * 2.0 * PI / 6.0;
                    let r = [0.88, 0.65, 0.83, 0.72, 0.89, 0.70][k];
                    [self.s + r * phi.cos(), r * phi.sin()]
                })
                .collect();
            for k in 0..5 {
                let (a, b) = (hub[k], hub[k + 1]);
                let m = lerp2(lerp2(a, b, 0.45), [self.s, 0.0], 0.23);
                out.push(Strand {
                    pts: [seg(a, m, 0.05), seg(m, b, 0.05)].concat(),
                    w: W_CAPTURE_MM,
                    radial: false,
                });
            }
        } else {
            // Five differently draped threads anchor to six radial roots. The sixth sector stays open:
            // neither a circular hub rim nor a fused starburst can form at the centre.
            let hub: Vec<P2> = (0..6)
                .map(|k| {
                    let phi = self.phi(k * n / 6 + n / 4);
                    let r = self.root_radius() + 0.025;
                    [self.s + r * phi.cos(), r * phi.sin()]
                })
                .collect();
            for k in 0..5 {
                let (a, b) = (hub[k], hub[k + 1]);
                let depth =
                    dist2(lerp2(a, b, 0.5), [self.s, 0.0]) * [0.42, 0.56, 0.35, 0.48, 0.42][k];
                out.push(Strand {
                    pts: sagged(a, b, [self.s, 0.0], depth, 0.035),
                    w: W_CAPTURE_MM,
                    radial: false,
                });
            }
        }
        let total = (n as f64 * self.turns).round() as usize;
        let node = |j: usize| {
            self.at(
                self.spiral_from + (1.0 - self.spiral_from) * j as f64 / total as f64,
                self.phi(j % n),
            )
        };
        let mut pts = Vec::new();
        for j in 0..total {
            let (a, b) = (node(j), node(j + 1));
            extend(
                &mut pts,
                sagged(a, b, [self.s, 0.0], 0.13 * dist2(a, b), 0.05),
            );
        }
        out.push(Strand {
            pts,
            w: W_CAPTURE_MM,
            radial: false,
        });
        let mut ring = Vec::new();
        for k in 0..n {
            extend(
                &mut ring,
                seg(
                    self.at(self.rim, self.phi(k)),
                    self.at(self.rim, self.phi(k + 1)),
                    0.05,
                ),
            );
        }
        out.push(Strand {
            pts: ring,
            w: W_FRAME_MM,
            radial: false,
        });
    }
}

/// The frieze round the ring: a zig-zag frame from edge to edge, each of its triangles a web corner anchored on the edge,
/// its spokes fanning across the crown and its capture threads hanging toward the anchor. Returns the zig-zag's step.
fn frieze(web: &Web, out: &mut Vec<Strand>) -> f64 {
    let m = 2 * ((web.circ / FRIEZE_STEP_MM / 2.0).round().max(2.0) as i64);
    let q = web.circ / m as f64;
    let vtx = |k: i64| -> P2 {
        let s = k as f64 * q;
        [
            s,
            if k.rem_euclid(2) == 0 { 1.0 } else { -1.0 } * web.reach(s),
        ]
    };
    for k in 0..m {
        out.push(Strand {
            pts: seg(vtx(k), vtx(k + 1), 0.05),
            w: W_FRAME_MM,
            radial: false,
        });
    }
    for k in 0..m {
        let apex = vtx(k);
        let (l, r) = (vtx(k - 1), vtx(k + 1));
        let side = l[1].signum();
        let feet: Vec<P2> = (0..=FRIEZE_SPOKES + 1)
            .map(|j| {
                let s = lerp(l[0], r[0], j as f64 / (FRIEZE_SPOKES + 1) as f64);
                [s, side * web.reach(s)]
            })
            .collect();
        for foot in &feet[1..=FRIEZE_SPOKES] {
            out.push(Strand {
                pts: seg(apex, *foot, 0.05),
                w: W_SPOKE_MM,
                radial: false,
            });
        }
        // Capture arcs round the anchor, each hanging toward it between spokes; an arc past the shortest spoke breaks where it leaves the crown.
        let longest = feet.iter().map(|f| dist2(apex, *f)).fold(0.0, f64::max);
        // Alternate corners start half a pitch apart, or the arcs of neighbours line up into rails along the ring.
        let mut radius = FRIEZE_ARC_FROM_MM
            + if k.rem_euclid(2) == 0 {
                0.0
            } else {
                0.5 * FRIEZE_ARC_PITCH_MM
            };
        while radius < longest - 0.4 {
            let p: Vec<P2> = feet
                .iter()
                .map(|f| {
                    let d = dist2(apex, *f);
                    lerp2(apex, *f, radius / d)
                })
                .collect();
            let mut pts = Vec::new();
            for w in p.windows(2) {
                extend(
                    &mut pts,
                    sagged(w[0], w[1], apex, 0.14 * dist2(w[0], w[1]), 0.05),
                );
            }
            out.push(Strand {
                pts,
                w: W_CAPTURE_MM,
                radial: false,
            });
            radius += FRIEZE_ARC_PITCH_MM;
        }
    }
    q
}

/// What the crown web measures.
#[derive(Default, serde::Serialize)]
struct WebStats {
    orbs: Vec<(String, f64, usize)>,
    frieze_step_mm: f64,
    frieze_corners: usize,
    /// The longest visible stretch of any thread but an orb's radials within 15 degrees of the ring's own direction, metal mm.
    longest_parallel_run_mm: f64,
    /// The same for the orbs' radials, which converge on their hubs.
    longest_orb_radial_along_ring_mm: f64,
    windows: Vec<String>,
    svg_bytes: usize,
    free_hub_root_gap_mm: f64,
}

/// An open polyline with every point dropped that sits within `tol` of the chord its kept neighbours make.
fn simplify(pts: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    fn rdp(p: &[(f64, f64)], tol: f64, out: &mut Vec<(f64, f64)>) {
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy).max(1e-12);
        let (mut worst, mut at) = (0.0, 0);
        for (i, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
            let d = ((q.0 - a.0) * dy - (q.1 - a.1) * dx).abs() / l;
            if d > worst {
                worst = d;
                at = i;
            }
        }
        if worst > tol {
            rdp(&p[..=at], tol, out);
            rdp(&p[at..], tol, out);
        } else {
            out.push(b);
        }
    }
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    rdp(pts, tol, &mut out);
    out
}

/// One window of the web as SVG in chart millimetres, from ring angle `lo` to `hi`, `half` of its height either side of the crest.
/// Each thread is stroked a piece at a time, the piece's chart width solved so it holds the thread's width in metal.
fn window_svg(
    ch: &Chart,
    threads: &[Thread],
    dew: &[Dew],
    lo: f64,
    hi: f64,
    half: f64,
) -> (String, f64) {
    let width = ch.u(hi) - ch.u(lo);
    let r_ref = ch.ctx.circumference_mm / (2.0 * PI);
    let unwrap = |th: f64| {
        let th = lo + (th - lo).rem_euclid(360.0);
        if th > hi + 180.0 { th - 360.0 } else { th }
    };
    let xy = |p: P2| -> (f64, f64) {
        let th = unwrap(p[0]);
        (
            ch.u(th) - ch.u(lo),
            half - (ch.v(th, p[1]) - ch.ctx.crest_v_mm),
        )
    };
    let near = |th: f64| {
        let d = (th - lo).rem_euclid(360.0);
        d <= hi - lo + 2.0 || d >= 358.0
    };
    let um = |v: f64| (v * 1000.0).round() as i64;
    let text = |v: i64| {
        format!(
            "{}{}.{:03}",
            if v < 0 { "-" } else { "" },
            v.abs() / 1000,
            v.abs() % 1000
        )
    };
    let mut groups: std::collections::BTreeMap<(i64, u8), String> =
        std::collections::BTreeMap::new();
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
            let mut start = 0;
            while start + 1 < run.len() {
                let (mut end, mut len, mut along, mut across) = (start, 0.0, 0.0, 0.0);
                while end + 1 < run.len() && len < 1.5 {
                    let (a, b) = (run[end], run[end + 1]);
                    let ds = wrap180(b[0] - a[0]).to_radians() * ch.r(a[0], a[1]);
                    let dt = b[1] - a[1];
                    let l = ds.hypot(dt);
                    if l > 1e-12 {
                        along += ds * ds / l;
                        across += dt * dt / l;
                    }
                    len += l;
                    end += 1;
                }
                let mid = run[(start + end) / 2];
                let ku = ch.r(mid[0], mid[1]) / r_ref;
                let kv = ch.ctx.station_stretch(mid[0].rem_euclid(360.0));
                let (c2, s2) = if len > 1e-12 {
                    (along / len, across / len)
                } else {
                    (1.0, 0.0)
                };
                let wc = t.w / (kv * kv * c2 + ku * ku * s2).sqrt();
                let pts: Vec<(f64, f64)> = run[start..=end].iter().map(|p| xy(*p)).collect();
                let tone = 0;
                let path = groups
                    .entry(((wc / 0.005).round() as i64, tone))
                    .or_default();
                let mut last = (0i64, 0i64);
                for (i, p) in simplify(&pts, 0.002).iter().enumerate() {
                    let q = (um(p.0), um(p.1));
                    if i == 0 {
                        *path += &format!("M{} {}", text(q.0), text(q.1));
                    } else if q != last {
                        *path += &format!("l{} {}", text(q.0 - last.0), text(q.1 - last.1));
                    }
                    last = q;
                }
                start = end;
            }
        }
    }
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.4} {h:.4}" width="{width:.4}" height="{h:.4}"><defs><radialGradient id="dew"><stop offset="0" stop-color="#000"/><stop offset="0.55" stop-color="#141414"/><stop offset="1" stop-color="#a0a0a0"/></radialGradient><filter id="soft" filterUnits="userSpaceOnUse" x="0" y="0" width="{width:.4}" height="{h:.4}"><feGaussianBlur stdDeviation="0.012"/></filter>"##,
        h = 2.0 * half
    );
    for (i, (_, path)) in groups.iter().enumerate() {
        s += &format!(r##"<path id="thread{i}" d="{path}"/>"##);
    }
    s += &format!(
        r##"</defs><rect width="{width:.4}" height="{:.4}" fill="#fff"/><g filter="url(#soft)" fill="none" stroke-linecap="round" stroke-linejoin="round">"##,
        2.0 * half
    );
    // Nested contours sample h = sqrt(1 - x²) across the whole wire. SVG <use> keeps
    // the geometry compact; height-ordered painting also preserves every crossover.
    // The finest centre contour is under 0.02 mm wide and is softened into the peak.
    for k in 0..16 {
        let share = 1.0 - k as f64 / 16.0;
        let height = (1.0 - share * share).sqrt();
        let tone = (255.0 * (1.0 - height)).round() as u8;
        s += &format!(r##"<g stroke="#{tone:02x}{tone:02x}{tone:02x}">"##);
        for (i, ((key, _), _)) in groups.iter().enumerate() {
            s += &format!(
                r##"<use href="#thread{i}" stroke-width="{:.4}"/>"##,
                *key as f64 * 0.005 * share
            );
        }
        s += "</g>";
    }
    s += "</g>";
    for b in dew.iter().filter(|b| near(b.at[0])) {
        let (x, y) = xy(b.at);
        let ku = ch.r(b.at[0], b.at[1]) / r_ref;
        let kv = ch.ctx.station_stretch(b.at[0].rem_euclid(360.0));
        s += &format!(
            r##"<ellipse cx="{x:.3}" cy="{y:.3}" rx="{:.3}" ry="{:.3}" fill="url(#dew)"/>"##,
            b.r / ku,
            b.r / kv
        );
    }
    s += "</svg>";
    (s, width)
}

/// The cheek silk has upright radials and two draped catenary rows.
fn cheek_svg(cell_w: f64, cell_h: f64) -> String {
    let periods = (cell_w / CHEEK_PITCH_MM).round().max(1.0) as i64;
    let pitch = cell_w / periods as f64;
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cell_w:.4} {cell_h:.4}" width="{cell_w:.4}" height="{cell_h:.4}"><rect width="{cell_w:.4}" height="{cell_h:.4}" fill="#fff"/><g fill="none" stroke="#000" stroke-linecap="round" stroke-linejoin="round">"##
    );
    for k in -1..=periods + 1 {
        let x = k as f64 * pitch;
        s += &format!(
            r##"<path stroke-width="{CHEEK_RADIAL_MM}" d="M{x:.4} -0.1 V{:.4}"/>"##,
            cell_h + 0.1
        );
        for share in [0.35, 0.70] {
            let base = cell_h * share;
            let a = pitch * 0.5;
            let mut path = String::new();
            for j in 0..=12 {
                let u = j as f64 / 12.0;
                let y =
                    base + 0.25 * (1.0 - ((2.0 * u - 1.0).cosh() - 1.0) / (1.0f64.cosh() - 1.0));
                path += &format!(
                    "{}{:.4} {y:.4}",
                    if j == 0 { "M" } else { " L" },
                    x + 2.0 * a * u
                );
            }
            s += &format!(r##"<path stroke="#555" stroke-width="{CHEEK_THREAD_MM}" d="{path}"/>"##);
        }
    }
    s += "</g></svg>";
    s
}

/// The cheek web on both side faces.
fn cheek_web(d: &mut RingDesign) -> Result<(f64, f64)> {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Cheek web", &ctx);
    ensure!(
        t.fit_to_side_faces(&ctx, ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG),
        "The band carries no side face for the web"
    );
    t.repeats_around = 8;
    t.height_mm = 0.30;
    t.continuous = true;
    t.feather_mm = 0.0;
    t.edge_mm = 0.15;
    let (cw, ch) = t.cell_size(&ctx);
    println!("  cheek web cell {cw:.2} x {ch:.2} mm");
    d.svgs.push(SvgAlpha {
        name: "Cheek web".into(),
        svg: cheek_svg(cw, ch),
        invert: false,
    });
    let mut e = LayerEntry::new("Cheek web, draped silk", Layer::Tiling(t));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok((cw, ch))
}

/// The web in web space and what it measures: the main orb round the carapace, three squashed orbs round the ring,
/// the frieze of web corners between them all, and a short dragline from the spinnerets.
fn web_strands(
    web: &Web,
    spinnerets: f64,
    covered: &dyn Fn(P2) -> bool,
) -> (Vec<Strand>, Vec<Dew>, WebStats) {
    let main = Orb {
        name: "Main orb",
        s: 0.0,
        a_s: MAIN_RIM_MM,
        a_t: MAIN_RIM_MM,
        radials: MAIN_RADIALS,
        offset_deg: 180.0 / MAIN_RADIALS as f64,
        spiral_from: MAIN_SPIRAL_FROM_MM / MAIN_RIM_MM,
        turns: (MAIN_RIM_MM - MAIN_SPIRAL_FROM_MM) / MAIN_PITCH_MM,
        rim: 1.0 + 0.45 / MAIN_RIM_MM,
    };
    let mut orbs = vec![main];
    orbs.extend(
        ORBS.iter()
            .map(|(name, theta, radials)| Orb::squashed(web, name, *theta, *radials)),
    );
    let mut raw = Vec::new();
    let mut dots = Vec::new();
    let mut stats = WebStats {
        orbs: orbs
            .iter()
            .map(|o| {
                (
                    o.name.to_string(),
                    web.theta(o.s).rem_euclid(360.0),
                    o.radials,
                )
            })
            .collect(),
        free_hub_root_gap_mm: orbs[1..]
            .iter()
            .map(|o| 2.0 * o.root_radius() * (PI / o.radials as f64).sin() - W_ORB_MM)
            .fold(f64::MAX, f64::min),
        ..WebStats::default()
    };
    let on_crown = |p: P2| p[1].abs() <= web.reach(p[0]) + 1e-6;
    // The squashed orbs take precedence over the main orb where their frames overlap it.
    for (k, o) in orbs.iter().enumerate() {
        let mut mine = Vec::new();
        o.draw(&mut mine);
        let keep = |p: P2| {
            on_crown(p) && (k > 0 || orbs[1..].iter().all(|q| q.rho(web, p) >= q.rim + 0.02))
        };
        raw.extend(mine.into_iter().flat_map(|s| clip(s, &keep)));
    }
    let mut lattice = Vec::new();
    stats.frieze_step_mm = frieze(web, &mut lattice);
    stats.frieze_corners = (web.circ / stats.frieze_step_mm).round() as usize;
    let clear = |p: P2| on_crown(p) && orbs.iter().all(|o| o.rho(web, p) >= o.rim + 0.02);
    raw.extend(lattice.into_iter().flat_map(|s| clip(s, &clear)));
    // Nothing is drawn where the bodies stand, so they meet a smooth crown.
    let mut raw: Vec<Strand> = raw
        .into_iter()
        .flat_map(|s| clip(s, &|p| !covered(p)))
        .collect();
    // The dragline: a few beads of dew on the thread the spider pays out behind her.
    let from = web.s_of(spinnerets);
    let mut line = vec![[from, 0.0]];
    for k in 0..3 {
        let at = from + 0.55 + 0.75 * k as f64;
        dots.push(Dew {
            at: [at, 0.0],
            r: 0.5 * lerp(0.46, 0.36, k as f64 / 2.0),
        });
        line.push([at, 0.0]);
    }
    raw.push(Strand {
        pts: line
            .windows(2)
            .flat_map(|w| seg(w[0], w[1], 0.05))
            .collect(),
        w: 0.21,
        radial: false,
    });
    let run_along = |s: &Strand| {
        let (mut run, mut worst) = (0.0f64, 0.0f64);
        for w in s.pts.windows(2) {
            let (ds, dt) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            if dt.abs() <= ds.abs() * 15f64.to_radians().tan() && !covered(w[0]) && !covered(w[1]) {
                run += ds.hypot(dt);
                if run > worst && run > 2.5 && std::env::var("ARACHNE_DEBUG").is_ok() {
                    println!(
                        "    parallel run {run:.2} mm reaching ({:.2}, {:.2}) at {:.1} deg, width {}",
                        w[1][0],
                        w[1][1],
                        web.theta(w[1][0]).rem_euclid(360.0),
                        s.w
                    );
                }
                worst = worst.max(run);
            } else {
                run = 0.0;
            }
        }
        worst
    };
    stats.longest_parallel_run_mm = raw
        .iter()
        .filter(|s| !s.radial)
        .map(run_along)
        .fold(0.0, f64::max);
    stats.longest_orb_radial_along_ring_mm = raw
        .iter()
        .filter(|s| s.radial)
        .map(run_along)
        .fold(0.0, f64::max);
    (raw, dots, stats)
}

/// The crown web as decals, window by window round the ring.
fn crown_web(
    d: &mut RingDesign,
    bare: &Bare,
    carapace: &Body,
    abdomen: &Body,
    preview: Option<&Path>,
) -> Result<(WebStats, f64)> {
    let ch = Chart {
        bare,
        ctx: d.field_context(),
    };
    let web = Web::new(&ch);
    let (a, _) = abdomen.semi();
    let spinnerets =
        theta_of(abdomen.world([abdomen.ahead * (a + abdomen.widest + 0.3), 0.0, 0.0]));
    let covered = |p: P2| {
        let th = web.theta(p[0]);
        let (r, z) = bare.surface_at_t(th, p[1]);
        let w = at_ring(th, r, z);
        carapace.margin(w) < -0.05 || abdomen.margin(w) < -0.05
    };
    let (strands, dots, mut stats) = web_strands(&web, spinnerets, &covered);
    if let Some(path) = preview {
        preview_svg(&web, &strands, &dots, path)?;
    }
    // Ring angles run on continuously along each thread, across the seam where web space folds.
    let threads: Vec<Thread> = strands
        .iter()
        .map(|s| {
            let mut pts: Vec<P2> = Vec::with_capacity(s.pts.len());
            for p in &s.pts {
                let th = web.theta(p[0]);
                let th = pts.last().map_or(th, |q| q[0] + wrap180(th - q[0]));
                pts.push([th, p[1]]);
            }
            Thread { pts, w: s.w }
        })
        .collect();
    let dew: Vec<Dew> = dots
        .iter()
        .map(|b| Dew {
            at: [web.theta(b.at[0]), b.at[1]],
            r: b.r,
        })
        .collect();
    let half = (0..360)
        .map(|k| (ch.edge(k as f64) - EDGE_INSET_MM) / ch.ctx.station_stretch(k as f64))
        .fold(0.0, f64::max)
        + 0.35;
    let windows = [
        (34.0, 94.0, "Web round the spider"),
        (94.0, 140.0, "Web behind the spinnerets"),
        (140.0, 200.0, "Orb behind her"),
        (200.0, 245.0, "Web toward the palm"),
        (245.0, 295.0, "Orb at the palm"),
        (295.0, 345.0, "Web past the palm"),
        (345.0, 394.0, "Orb before her"),
    ];
    for (lo, hi, name) in windows {
        let (lo, hi) = (lo - 2.5, hi + 2.5);
        for fine in [false, true] {
            let threads: Vec<_> = threads
                .iter()
                .filter(|t| (t.w <= W_CAPTURE_MM + 1e-9) == fine)
                .cloned()
                .collect();
            let name = if fine {
                format!("{name}, capture silk")
            } else {
                name.to_string()
            };
            let (svg, width) =
                window_svg(&ch, &threads, if fine { &[] } else { &dew }, lo, hi, half);
            stats.svg_bytes += svg.len();
            stats.windows.push(name.clone());
            d.svgs.push(SvgAlpha {
                name: name.clone(),
                svg,
                invert: false,
            });
            let stamp = Decal {
                theta_deg: (0.5 * (lo + hi)).rem_euclid(360.0),
                v_mm: ch.ctx.crest_v_mm,
                size_mm: width,
                rotation_deg: 0.0,
                height_mm: if fine {
                    CAPTURE_HEIGHT_MM
                } else {
                    WEB_HEIGHT_MM
                },
                flip: false,
            };
            let mut e = LayerEntry::new(
                &name,
                Layer::Decals(DecalLayer {
                    alpha: name.clone(),
                    decals: vec![stamp],
                    feather_mm: 0.05,
                    invert: false,
                }),
            );
            e.blend = Blend::Max;
            d.layers.layers.push(e);
        }
    }
    Ok((stats, spinnerets))
}

/// The whole crown web unrolled in metal millimetres, for looking at before any build.
fn preview_svg(web: &Web, strands: &[Strand], dots: &[Dew], path: &Path) -> Result<()> {
    let (x0, x1) = (-0.5 * web.circ, 0.5 * web.circ);
    let top = (0..720)
        .map(|k| web.edge(x0 + (x1 - x0) * k as f64 / 720.0))
        .fold(0.0, f64::max)
        + 1.0;
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x0:.3} {:.3} {:.3} {:.3}" width="{:.0}" height="{:.0}"><rect x="{x0:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="#1a1a1a"/>"##,
        -top,
        x1 - x0,
        2.0 * top,
        (x1 - x0) * 40.0,
        2.0 * top * 40.0,
        -top,
        x1 - x0,
        2.0 * top
    );
    let mut edge_hi = String::from("M");
    let mut edge_lo = String::from("M");
    for k in 0..=720 {
        let x = x0 + (x1 - x0) * k as f64 / 720.0;
        let e = web.edge(x);
        edge_hi += &format!("{}{x:.3} {:.3}", if k == 0 { "" } else { " L" }, -e);
        edge_lo += &format!("{}{x:.3} {:.3}", if k == 0 { "" } else { " L" }, e);
    }
    s += &format!(
        r##"<path d="{edge_hi}" stroke="#555" stroke-width="0.05" fill="none"/><path d="{edge_lo}" stroke="#555" stroke-width="0.05" fill="none"/>"##
    );
    for st in strands {
        let mut runs: Vec<Vec<P2>> = vec![Vec::new()];
        for w in st.pts.windows(2) {
            runs.last_mut().unwrap().push(w[0]);
            if (web.wrap(w[1][0], 0.0) - web.wrap(w[0][0], 0.0)).abs() > 0.5 * web.circ {
                runs.push(Vec::new());
            }
        }
        runs.last_mut().unwrap().push(*st.pts.last().unwrap());
        for run in runs.iter().filter(|r| r.len() > 1) {
            let d: Vec<String> = run
                .iter()
                .map(|p| format!("{:.3} {:.3}", web.wrap(p[0], 0.0), -p[1]))
                .collect();
            s += &format!(
                r##"<path d="M{}" stroke="#e8c870" stroke-width="{}" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##,
                d.join(" L"),
                st.w
            );
        }
    }
    for b in dots {
        s += &format!(
            r##"<circle cx="{:.3}" cy="{:.3}" r="{:.3}" fill="#fff0a0"/>"##,
            web.wrap(b.at[0], 0.0),
            -b.at[1],
            b.r
        );
    }
    s += "</svg>";
    std::fs::write(path, s)?;
    Ok(())
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
    /// Where the ankle and the metatarsal joint fall, as shares of the arc from knee to claw.
    at: [f64; 2],
}

const LEGS: [LegSpec; 4] = [
    LegSpec {
        name: "Leg I",
        root_along: -1.35,
        reach_deg: 21.0,
        claw_deg: -60.0,
        shares: [0.86, 0.68, 0.60],
        at: [0.2, 0.5],
    },
    LegSpec {
        name: "Leg II",
        root_along: -0.45,
        reach_deg: 33.0,
        claw_deg: -44.0,
        shares: [0.86, 0.70, 0.62],
        at: [0.2, 0.5],
    },
    LegSpec {
        name: "Leg III",
        root_along: 0.45,
        reach_deg: 130.0,
        claw_deg: 44.0,
        shares: [0.86, 0.70, 0.62],
        at: [0.2, 0.5],
    },
    // The hind leg grips the upper cheek behind the abdomen.
    LegSpec {
        name: "Leg IV",
        root_along: 1.35,
        reach_deg: 135.5,
        claw_deg: 60.0,
        shares: [0.90, 0.74, 0.66],
        at: [0.24, 0.53],
    },
];

/// A planar path as a chain of cubics in its plane, with the tube's radius at each end.
#[derive(Clone)]
struct LegPath {
    plane: Workplane,
    pieces: Vec<[P2; 4]>,
    r0: f64,
    r1: f64,
}

fn bezier(c: &[P2; 4], t: f64) -> P2 {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        a * c[0][0] + b * c[1][0] + cc * c[2][0] + d * c[3][0],
        a * c[0][1] + b * c[1][1] + cc * c[2][1] + d * c[3][1],
    ]
}
fn piece_length(c: &[P2; 4]) -> f64 {
    (0..40)
        .map(|i| dist2(bezier(c, i as f64 / 40.0), bezier(c, (i + 1) as f64 / 40.0)))
        .sum()
}
/// The smallest radius of curvature along a cubic.
fn tightest_bend(c: &[P2; 4]) -> f64 {
    (0..=100)
        .map(|i| {
            let t = i as f64 / 100.0;
            let u = 1.0 - t;
            let d1: P2 = std::array::from_fn(|k| {
                3.0 * (u * u * (c[1][k] - c[0][k])
                    + 2.0 * u * t * (c[2][k] - c[1][k])
                    + t * t * (c[3][k] - c[2][k]))
            });
            let d2: P2 = std::array::from_fn(|k| {
                6.0 * (u * (c[2][k] - 2.0 * c[1][k] + c[0][k])
                    + t * (c[3][k] - 2.0 * c[2][k] + c[1][k]))
            });
            d1[0].hypot(d1[1]).powi(3) / (d1[0] * d2[1] - d1[1] * d2[0]).abs().max(1e-12)
        })
        .fold(f64::MAX, f64::min)
}

impl LegPath {
    fn world(&self, p: P2) -> P3 {
        let w = &self.plane;
        std::array::from_fn(|k| w.origin[k] + w.x[k] * p[0] + w.y[k] * p[1])
    }
    fn length(&self) -> f64 {
        self.pieces.iter().map(piece_length).sum()
    }
    /// Points along the path's centre line in the world, with the tube's radius there.
    fn samples(&self) -> Vec<(P3, f64)> {
        let n = self.pieces.len() * 8;
        (0..=n)
            .map(|i| {
                let k = (i / 8).min(self.pieces.len() - 1);
                let u = if i == n { 1.0 } else { (i % 8) as f64 / 8.0 };
                (
                    self.world(bezier(&self.pieces[k], u)),
                    lerp(self.r0, self.r1, i as f64 / n as f64),
                )
            })
            .collect()
    }
    fn sketch(&self, name: &str) -> Sketch {
        let mut s = Sketch {
            name: name.into(),
            plane: self.plane.clone(),
            ..Sketch::default()
        };
        let mut last: Option<Id> = None;
        for c in &self.pieces {
            let a = last.unwrap_or_else(|| s.point(snap(c[0])));
            let (b, cc, e) = (
                s.point(snap(c[1])),
                s.point(snap(c[2])),
                s.point(snap(c[3])),
            );
            s.entity(Geometry::Bezier {
                points: [a, b, cc, e],
            });
            last = Some(e);
        }
        s
    }
    /// Tightest bend over the tube's widest radius; a tube folds under 1.
    fn bend_ratio(&self) -> f64 {
        self.pieces
            .iter()
            .map(tightest_bend)
            .fold(f64::MAX, f64::min)
            / self.r0.max(self.r1)
    }
    fn station(&self, t: f64) -> LoftStation {
        let q = t.clamp(0.0, 1.0) * self.pieces.len() as f64;
        let k = (q.floor() as usize).min(self.pieces.len() - 1);
        let (v, u) = (q - k as f64, 1.0 - (q - k as f64));
        let c = &self.pieces[k];
        let d: P2 = std::array::from_fn(|j| {
            3.0 * (u * u * (c[1][j] - c[0][j])
                + 2.0 * u * v * (c[2][j] - c[1][j])
                + v * v * (c[3][j] - c[2][j]))
        });
        LoftStation {
            p: self.world(bezier(c, v)),
            tangent: unit3(add3(self.plane.x.map(|x| x * d[0]), self.plane.y, d[1])),
            r: lerp(self.r0, self.r1, t),
        }
    }
}

#[derive(Clone, Copy)]
struct LoftStation {
    p: P3,
    tangent: P3,
    r: f64,
}

/// Invert true arc length along a segment, rather than trimming an arbitrary Bezier parameter.
struct ArcPath<'a> {
    path: &'a LegPath,
    lengths: Vec<f64>,
}

impl<'a> ArcPath<'a> {
    fn new(path: &'a LegPath) -> Self {
        let n = (path.length() / 0.01).ceil().max(100.0) as usize;
        let mut lengths = vec![0.0];
        let mut last = path.station(0.0).p;
        for k in 1..=n {
            let p = path.station(k as f64 / n as f64).p;
            lengths.push(lengths.last().unwrap() + len3(sub3(p, last)));
            last = p;
        }
        Self { path, lengths }
    }
    fn length(&self) -> f64 {
        *self.lengths.last().unwrap()
    }
    fn at(&self, arc: f64) -> LoftStation {
        let arc = arc.clamp(0.0, self.length());
        let k = self
            .lengths
            .partition_point(|s| *s < arc)
            .clamp(1, self.lengths.len() - 1);
        let u = (arc - self.lengths[k - 1]) / (self.lengths[k] - self.lengths[k - 1]).max(1e-12);
        self.path
            .station((k as f64 - 1.0 + u) / (self.lengths.len() - 1) as f64)
    }
}

fn cubic3(c: &[P3; 4], t: f64) -> P3 {
    let u = 1.0 - t;
    std::array::from_fn(|k| {
        u * u * u * c[0][k]
            + 3.0 * u * u * t * c[1][k]
            + 3.0 * u * t * t * c[2][k]
            + t * t * t * c[3][k]
    })
}

fn cubic3_tangent(c: &[P3; 4], t: f64) -> P3 {
    let u = 1.0 - t;
    unit3(std::array::from_fn(|k| {
        3.0 * (u * u * (c[1][k] - c[0][k])
            + 2.0 * u * t * (c[2][k] - c[1][k])
            + t * t * (c[3][k] - c[2][k]))
    }))
}

fn bend3(c: &[P3; 4]) -> f64 {
    (0..=160)
        .map(|i| {
            let t = i as f64 / 160.0;
            let u = 1.0 - t;
            let a: P3 = std::array::from_fn(|k| {
                3.0 * (u * u * (c[1][k] - c[0][k])
                    + 2.0 * u * t * (c[2][k] - c[1][k])
                    + t * t * (c[3][k] - c[2][k]))
            });
            let b: P3 = std::array::from_fn(|k| {
                6.0 * (u * (c[2][k] - 2.0 * c[1][k] + c[0][k])
                    + t * (c[3][k] - 2.0 * c[2][k] + c[1][k]))
            });
            len3(a).powi(3) / len3(cross3(a, b)).max(1e-12)
        })
        .fold(f64::MAX, f64::min)
}

fn joint_curve(a: LoftStation, b: LoftStation) -> ([P3; 4], f64) {
    let chord = len3(sub3(b.p, a.p));
    (48..=90)
        .map(|h| {
            let h = chord * h as f64 / 200.0;
            let c = [a.p, add3(a.p, a.tangent, h), add3(b.p, b.tangent, -h), b.p];
            let bend = bend3(&c);
            (c, bend)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

/// Drop oversampled stations only when their centre line and taper stay within 2 microns of the retained chords.
/// Curved joints retain more sections than the long, almost straight limb runs.
fn reduce_limb(points: &[P3], radii: &[f64]) -> Vec<usize> {
    fn keep(points: &[P3], radii: &[f64], base: usize, out: &mut Vec<usize>) {
        let last = points.len() - 1;
        let chord = sub3(points[last], points[0]);
        let norm = dot(chord, chord).max(1e-12);
        let mut worst = 0.0;
        let mut at = 0;
        for i in 1..last {
            let t = (dot(sub3(points[i], points[0]), chord) / norm).clamp(0.0, 1.0);
            let error = len3(sub3(points[i], add3(points[0], chord, t)))
                .max((radii[i] - lerp(radii[0], radii[last], t)).abs());
            if error > worst {
                worst = error;
                at = i;
            }
        }
        if worst > 0.002 {
            keep(&points[..=at], &radii[..=at], base, out);
            keep(&points[at..], &radii[at..], base + at, out);
        } else {
            out.push(base + last);
        }
    }
    let mut indices = vec![0];
    keep(points, radii, 0, &mut indices);
    indices
}

/// A ring-local loft with exact curve tangents at its adaptive sections. Using the
/// neighbouring chords as tangents after simplification would kink unequal spans.
fn limb_tube(points: &[P3], radii: &[f64], tangents: &[P3]) -> csg::Solid {
    const AROUND: usize = 24;
    let mut solid = csg::Solid::default();
    let first = tangents[0];
    let seed = if first[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let mut across = unit3(add3(seed, first, -dot(seed, first)));
    for ((p, r), t) in points.iter().zip(radii).zip(tangents) {
        across = unit3(add3(across, *t, -dot(across, *t)));
        let other = cross3(across, *t);
        for k in 0..AROUND {
            let (s, c) = (2.0 * PI * (k as f64 + 0.1234) / AROUND as f64).sin_cos();
            solid.v.push(add3(add3(*p, across, r * c), other, r * s));
        }
    }
    let at = |row: usize, col: usize| (row * AROUND + col % AROUND) as u32;
    for row in 0..points.len() - 1 {
        for col in 0..AROUND {
            solid
                .f
                .push([at(row, col), at(row + 1, col), at(row + 1, col + 1)]);
            solid
                .f
                .push([at(row, col), at(row + 1, col + 1), at(row, col + 1)]);
        }
    }
    let (start, end) = (solid.v.len() as u32, solid.v.len() as u32 + 1);
    solid.v.extend([points[0], *points.last().unwrap()]);
    for col in 0..AROUND {
        solid.f.push([start, at(0, col), at(0, col + 1)]);
        solid.f.push([
            at(points.len() - 1, col),
            end,
            at(points.len() - 1, col + 1),
        ]);
    }
    solid
}

/// One closed, continuous limb. Each joint is a tangent-matched local loft, with
/// smooth radial swelling over 0.35 mm on each adjoining segment, not a sphere union.
fn leg_part(leg: &Leg, abdomen: &Body) -> Result<(Operation, f64, f64)> {
    let arcs: Vec<_> = leg.sweeps.iter().map(|(_, p)| ArcPath::new(p)).collect();
    let mut points = Vec::new();
    let mut radii = Vec::new();
    let mut tangents = Vec::new();
    let mut joint_bend = f64::MAX;
    let mut push = |p: P3, r: f64, tangent: P3| {
        if points.last().is_none_or(|q| len3(sub3(p, *q)) > 1e-8) {
            points.push(p);
            radii.push(r);
            tangents.push(tangent);
        }
    };
    for (k, arc) in arcs.iter().enumerate() {
        let start = if k == 0 { 0.0 } else { JOINT_EASE_MM };
        let end = arc.length()
            - if k + 1 == arcs.len() {
                0.0
            } else {
                JOINT_EASE_MM
            };
        ensure!(
            end > start,
            "a limb segment is too short for its joint lofts"
        );
        let n = ((end - start) / 0.06).ceil().max(1.0) as usize;
        for j in 0..=n {
            let s = arc.at(lerp(start, end, j as f64 / n as f64));
            push(s.p, s.r, s.tangent);
        }
        if let Some(next) = arcs.get(k + 1) {
            let (a, b) = (arc.at(end), next.at(JOINT_EASE_MM));
            // Search only the Hermite handles, keeping both endpoints and tangents exact.
            // This finds the gentlest local bend without changing the solved limb paths.
            let (curve, bend) = joint_curve(a, b);
            let length: f64 = (0..100)
                .map(|j| {
                    len3(sub3(
                        cubic3(&curve, (j + 1) as f64 / 100.0),
                        cubic3(&curve, j as f64 / 100.0),
                    ))
                })
                .sum();
            let dr0 = (a.r - arc.at(end - 0.01).r) / 0.01 * length;
            let dr1 = (next.at(JOINT_EASE_MM + 0.01).r - b.r) / 0.01 * length;
            let swelling = (KNUCKLE - 1.0) * arc.path.r1;
            let radius = |t: f64| {
                let t2 = t * t;
                let t3 = t2 * t;
                (2.0 * t3 - 3.0 * t2 + 1.0) * a.r
                    + (t3 - 2.0 * t2 + t) * dr0
                    + (-2.0 * t3 + 3.0 * t2) * b.r
                    + (t3 - t2) * dr1
                    + 16.0 * swelling * t2 * (1.0 - t).powi(2)
            };
            let max_radius = (0..=100)
                .map(|i| radius(i as f64 / 100.0))
                .fold(0.0, f64::max);
            let ratio = bend / max_radius;
            ensure!(
                ratio >= 1.25,
                "{} joint loft bends at {ratio:.3} of its swollen radius",
                leg.sweeps[k].0
            );
            joint_bend = joint_bend.min(ratio);
            let n = (length / 0.025).ceil().max(20.0) as usize;
            for j in 1..=n {
                let t = j as f64 / n as f64;
                push(cubic3(&curve, t), radius(t), cubic3_tangent(&curve, t));
            }
        }
    }
    let kept = reduce_limb(&points, &radii);
    let positions: Vec<_> = kept.iter().map(|i| points[*i]).collect();
    let radii: Vec<_> = kept.iter().map(|i| radii[*i]).collect();
    let tangents: Vec<_> = kept.iter().map(|i| tangents[*i]).collect();
    let solid = limb_tube(&positions, &radii, &tangents);
    ensure!(
        solid.open_edges() == (0, 0),
        "the continuous limb does not close"
    );
    ensure!(
        csg::self_crossings(&solid) == 0,
        "the continuous limb crosses itself"
    );
    let (nearest, margin) = solid
        .v
        .iter()
        .map(|p| (*p, abdomen.margin(*p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    ensure!(
        margin >= 0.40,
        "the continuous limb has only {margin:.3} mm clearance to the abdomen at {nearest:?}"
    );
    let mesh = stored::Packed::encode(
        &solid.v,
        &solid.f,
        &vec![0; solid.f.len()],
        &[SurfaceKind::Freeform],
    )?;
    let recipe = stored::Recipe {
        kernel: "bestiarium_arachne".into(),
        op: "continuous_limb".into(),
        params: serde_json::json!({
            "joint_ease_mm": JOINT_EASE_MM, "knuckle_scale": KNUCKLE, "around": 24, "dense_station_error_mm": 0.002,
            "sweeps": leg.sweeps.iter().map(|(name, path)| serde_json::json!({ "path": path.sketch(name), "radii_mm": [path.r0, path.r1] })).collect::<Vec<_>>()
        }),
        digest: String::new(),
    };
    Ok((
        Operation::Stored {
            recipe,
            sources: Vec::new(),
            mesh,
        },
        joint_bend,
        margin,
    ))
}

/// One solved leg and what it measures.
struct Leg {
    /// Each sweep: the segment it belongs to and its path.
    sweeps: Vec<(String, LegPath)>,
    joints: Vec<(&'static str, P3, f64)>,
    coxa: (P3, f64),
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
    Workplane {
        origin: o,
        x,
        y: [0.0, 0.0, -1.0],
        on_face: None,
    }
}

/// A run down the cheek from `a` to `b`: a chord in the plane holding it and the finger's axis, standing `out(share of the run, ring angle)` from the band's mid-plane.
fn cheek_run(a: P3, b: P3, r0: f64, r1: f64, out: &dyn Fn(f64, f64) -> f64) -> LegPath {
    let plane = cheek_plane(a, sub3(b, a));
    let end = [dot(sub3(b, a), plane.x), dot(sub3(b, a), plane.y)];
    let y_at = |u: f64| out(u, theta_of(add3(a, plane.x, end[0] * u))) - a[2].abs();
    let a1 = y_at(1.0 / 3.0) - end[1] / 27.0;
    let a2 = y_at(2.0 / 3.0) - 8.0 * end[1] / 27.0;
    let c = [
        [0.0, 0.0],
        [end[0] / 3.0, 3.0 * a1 - 1.5 * a2],
        [2.0 * end[0] / 3.0, -1.5 * a1 + 3.0 * a2],
        end,
    ];
    LegPath {
        plane,
        pieces: vec![c],
        r0,
        r1,
    }
}

/// The femur as one cubic from root to knee: bending gently, lying on the crown, arching up to the knee, under the garnet's girdle.
fn fit_femur(
    l: f64,
    yk: f64,
    surface: &dyn Fn(f64) -> f64,
    r_at: &dyn Fn(f64) -> f64,
    garnet_local: &dyn Fn(P2) -> P3,
    semi: (f64, f64),
    r0: f64,
    joint_ok: &dyn Fn(&[P2; 4]) -> bool,
) -> Option<[P2; 4]> {
    let (a, b) = (semi.0 + 0.05, semi.1 + 0.05);
    let mut best: Option<(f64, [P2; 4])> = None;
    for a0 in (-6..=12).map(|k| k as f64 * 5.0) {
        for a1 in (-18..=2).map(|k| k as f64 * 5.0) {
            for h0 in [0.20, 0.25, 0.3, 0.35, 0.4, 0.45, 0.50, 0.55] {
                'candidate: for h1 in [
                    0.25, 0.3, 0.35, 0.4, 0.45, 0.50, 0.55, 0.60, 0.65, 0.70, 0.75,
                ] {
                    let (s0, c0) = a0.to_radians().sin_cos();
                    let (s1, c1) = a1.to_radians().sin_cos();
                    let c = [
                        [0.0, 0.0],
                        [h0 * l * c0, h0 * l * s0],
                        [l - h1 * l * c1, yk - h1 * l * s1],
                        [l, yk],
                    ];
                    if tightest_bend(&c) < 1.25 * r0 {
                        continue;
                    }
                    let mut cost = 0.0;
                    for i in 0..=40 {
                        let p = bezier(&c, i as f64 / 40.0);
                        let r = r_at(p[0]);
                        let q = garnet_local(p);
                        if (q[0] / a).powi(2) + (q[1] / b).powi(2) < 1.0 && q[2] + r > -0.05 {
                            continue 'candidate;
                        }
                        if p[0] > 0.3 * l && p[0] < 0.85 * l {
                            cost += (p[1]
                                - (surface(p[0]) + r - 0.15
                                    + KNEE_LIFT_MM * smoothstep(0.3 * l, l, p[0])))
                            .powi(2);
                        }
                        if p[0] > 0.25 * l {
                            cost += 20.0 * ((surface(p[0]) + 0.25 * r) - p[1]).max(0.0).powi(2);
                        }
                    }
                    if best.as_ref().is_none_or(|(k, _)| cost < *k) && joint_ok(&c) {
                        best = Some((cost, c));
                    }
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Solves one low-side leg: the femur arching from the coxa to a high knee over the edge, the tibia down over it,
/// the metatarsus and tarsus down the cheek, the tarsus hooking its claw into the metal.
fn solve_leg(spec: &LegSpec, bare: &Bare, carapace: &Body, abdomen: &Body) -> Result<Leg> {
    let hub = bare.at(HUB_DEG);
    let crest = hub.flank[0][1];
    let root_deg = HUB_DEG + (spec.root_along / crest).to_degrees();
    let root = at_ring(
        root_deg,
        bare.crown_r(root_deg, ROOT_ACROSS_MM) - 0.3,
        -ROOT_ACROSS_MM,
    );
    let (sin, cos) = spec.reach_deg.to_radians().sin_cos();
    // The knee: where the femur's line in plan meets the edge, standing high over the corner.
    let mid_r = 0.5 * (crest + hub.edge_r);
    let mut knee_deg = HUB_DEG;
    for _ in 0..6 {
        let hw = bare.at(knee_deg).half_width;
        let along = spec.root_along - cos / sin * (hw - 0.05 - ROOT_ACROSS_MM);
        knee_deg = HUB_DEG + (along / mid_r).to_degrees();
    }
    let ks = bare.at(knee_deg);
    let knee_ball = KNUCKLE * KNEE_MM;
    let knee = at_ring(
        knee_deg,
        ks.edge_r + 0.28 + KNEE_LIFT_MM,
        -(ks.half_width + KNEE_PROUD_MM - knee_ball),
    );
    // Femur: in the upright plane through root and knee, lying on the crown and arching to the knee.
    let (femur_r0, femur_r1) = (FEMUR_MM, KNEE_MM);
    let up = unit3(at_ring(0.5 * (root_deg + knee_deg), 1.0, 0.0));
    let chord = sub3(knee, root);
    let x = unit3(add3(chord, up, -dot(chord, up)));
    let y = unit3(add3(up, x, -dot(up, x)));
    let fplane = Workplane {
        origin: root,
        x,
        y,
        on_face: None,
    };
    let to_world = |p: P2| add3(add3(root, x, p[0]), y, p[1]);
    let l = dot(chord, x);
    let yk = dot(chord, y);
    let n = 24;
    let mut last_surface = 0.0;
    let surface: Vec<P2> = (0..=n)
        .map(|i| {
            let xi = l * i as f64 / n as f64;
            let (mut lo, mut hi) = (-3.0, 3.0);
            if bare.signed_distance(to_world([xi, lo])) < 0.0
                && bare.signed_distance(to_world([xi, hi])) > 0.0
            {
                for _ in 0..40 {
                    let m = 0.5 * (lo + hi);
                    if bare.signed_distance(to_world([xi, m])) < 0.0 {
                        lo = m
                    } else {
                        hi = m
                    }
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
    // Down the cheek: the ankle, the metatarsal joint and the claw, each tube held a steady stand-off off the face.
    let claw_deg = HUB_DEG + spec.claw_deg;
    let span = claw_deg - knee_deg;
    let (ankle_deg, meta_deg) = (knee_deg + spec.at[0] * span, knee_deg + spec.at[1] * span);
    let stand =
        |r: f64, th: f64| bare.half_width(th) + smin(r - 0.18, bare.proud(th) - KNUCKLE * r, 0.05);
    let joint = |deg: f64, share: f64, r: f64| {
        let p = bare.cheek(deg, share, 0.0);
        [p[0], p[1], -stand(r, deg)]
    };
    let ankle = joint(ankle_deg, spec.shares[0], ANKLE_MM);
    let meta = joint(meta_deg, spec.shares[1], META_MM);
    let tibia = cheek_run(knee, ankle, KNEE_MM, ANKLE_MM, &|u, th| {
        lerp(
            knee[2].abs(),
            stand(ANKLE_MM, th),
            1.0 - (1.0 - u).powf(2.2),
        )
    });
    let metatarsus = cheek_run(ankle, meta, ANKLE_MM, META_MM, &|_, th| stand(META_MM, th));
    let tibia_arc = ArcPath::new(&tibia);
    let joint_ok = |c: &[P2; 4]| {
        let candidate = LegPath {
            plane: fplane.clone(),
            pieces: vec![*c],
            r0: femur_r0,
            r1: femur_r1,
        };
        // Check the entire circular section against the stone, not just its centre.
        // A small outward allowance covers tessellation and keeps a visible setting clearance.
        let normal = unit3(cross3(fplane.x, fplane.y));
        let (ga, gb, gh) = (
            carapace.gem.l_mm * 0.5 + 0.02,
            carapace.gem.w_mm * 0.5 + 0.02,
            carapace.gem.depth_mm() + 0.02,
        );
        for i in 0..=80 {
            let st = candidate.station(i as f64 / 80.0);
            let across = cross3(st.tangent, normal);
            for j in 0..32 {
                let (s, c) = (j as f64 * 2.0 * PI / 32.0).sin_cos();
                let q = carapace.local(add3(add3(st.p, normal, st.r * c), across, st.r * s));
                if q[2] > 0.0
                    && q[2] < gh
                    && (q[0] / ga).powi(2) + (q[1] / gb).powi(2) < 1.0 - (q[2] / gh).powi(2)
                {
                    return false;
                }
            }
        }
        let arc = ArcPath::new(&candidate);
        let (a, b) = (
            arc.at(arc.length() - JOINT_EASE_MM),
            tibia_arc.at(JOINT_EASE_MM),
        );
        let (_, bend) = joint_curve(a, b);
        bend / (KNUCKLE * a.r.max(b.r)) >= 1.25
    };
    let femur_curve = fit_femur(
        l,
        yk,
        &surface_at,
        &|xi| lerp(femur_r0, femur_r1, xi / l),
        &|p| carapace.local(to_world(p)),
        carapace.semi(),
        femur_r0,
        &joint_ok,
    )
    .ok_or_else(|| {
        anyhow::anyhow!(
            "{}: no femur arcs gently from under the carapace to its knee",
            spec.name
        )
    })?;
    let femur = LegPath {
        plane: fplane,
        pieces: vec![femur_curve],
        r0: femur_r0,
        r1: femur_r1,
    };
    // The coxa: a ball where the femur leaves the carapace's body.
    let coxa = femur
        .samples()
        .into_iter()
        .find(|(p, _)| carapace.margin(*p) >= 0.0)
        .map_or(root, |(p, _)| p);
    // The tarsus: along the cheek, easing down onto it, then the hook turning its tip into the metal.
    let claw = bare.cheek(claw_deg, spec.shares[2], 0.0);
    let turn = HOOK_DEG.to_radians();
    let bend_r = HOOK_MM / turn;
    let sink = |th: f64| bare.half_width(th) - TIP_SINK_MM;
    let hook_start = |th: f64| (bare.half_width(th) + CLAW_MM - 0.16).min(stand(META_MM, th));
    let drop_arc = bend_r * (1.0 - turn.cos());
    let dive = ((hook_start(claw_deg) - drop_arc - sink(claw_deg)) / turn.sin()).max(0.2);
    let run_x = bend_r * turn.sin() + dive * turn.cos();
    let plane = cheek_plane(meta, sub3(claw, meta));
    let reach = dot(sub3(claw, meta), plane.x);
    let pre_x = reach - run_x;
    ensure!(
        pre_x > 0.8,
        "{}: the tarsus is too short for its hook ({reach:.2} mm to the claw, {run_x:.2} for the hook)",
        spec.name
    );
    // One cubic along the cheek, leaving the metatarsal joint and arriving level at the hook.
    let level = [
        pre_x,
        hook_start(theta_of(add3(meta, plane.x, pre_x))) - meta[2].abs(),
    ];
    let mut pieces = vec![[
        [0.0, 0.0],
        [pre_x / 3.0, 0.0],
        [2.0 * pre_x / 3.0, level[1]],
        level,
    ]];
    // The hook and the dive as one cubic: level where it leaves the run, turned the hook's angle into the metal at the tip.
    let tip = [level[0] + run_x, level[1] - drop_arc - dive * turn.sin()];
    let reach_hook = dist2(level, tip);
    pieces.push([
        level,
        [level[0] + 0.42 * reach_hook, level[1]],
        [
            tip[0] - 0.42 * reach_hook * turn.cos(),
            tip[1] + 0.42 * reach_hook * turn.sin(),
        ],
        tip,
    ]);
    let tarsus = LegPath {
        plane,
        pieces,
        r0: META_MM,
        r1: CLAW_MM,
    };
    // Each limb segment is one tapered sweep.
    let sweeps = vec![
        ("femur".to_string(), femur.clone()),
        ("tibia".to_string(), tibia.clone()),
        ("metatarsus".to_string(), metatarsus.clone()),
        ("tarsus".to_string(), tarsus.clone()),
    ];
    // What the leg measures.
    let total = femur.length() + tibia.length() + metatarsus.length() + tarsus.length();
    let knee_proud = knee[2].abs() + knee_ball - ks.half_width;
    let (mut most_proud, mut least_share) = (0.0f64, f64::MAX);
    for (p, r) in [&tibia, &metatarsus, &tarsus]
        .into_iter()
        .flat_map(|path| path.samples())
    {
        let sec = bare.at(theta_of(p));
        most_proud = most_proud.max(p[2].abs() + r - sec.half_width);
        least_share = least_share.min((p[0].hypot(p[1]) - sec.bore_r) / (sec.edge_r - sec.bore_r));
    }
    if std::env::var("ARACHNE_DEBUG").is_ok() {
        for (name, path) in &sweeps {
            if let Some((p, r)) = path.samples().into_iter().min_by(|(a, ar), (b, br)| {
                (abdomen.margin(*a) - ar).total_cmp(&(abdomen.margin(*b) - br))
            }) {
                println!(
                    "    {} {name}: abdomen margin {:.3} at {:?}",
                    spec.name,
                    abdomen.margin(p) - r,
                    p
                );
            }
        }
    }
    let abdomen_margin = sweeps
        .iter()
        .flat_map(|(_, path)| path.samples())
        .map(|(p, r)| abdomen.margin(p) - r)
        .fold(f64::MAX, f64::min);
    Ok(Leg {
        sweeps,
        joints: vec![
            ("knee", knee, knee_ball),
            ("ankle", ankle, KNUCKLE * ANKLE_MM),
            ("metatarsal joint", meta, KNUCKLE * META_MM),
        ],
        coxa: (coxa, COXA_BALL_MM),
        knee_deg,
        knee_proud,
        most_proud,
        least_share,
        abdomen_margin,
        length: total,
    })
}

/// The four legs on the low side.
fn solve_legs(bare: &Bare, carapace: &Body, abdomen: &Body) -> Result<Vec<(LegSpec, Leg)>> {
    LEGS.iter()
        .map(|spec| {
            let leg = solve_leg(spec, bare, carapace, abdomen)?;
            for (part, p) in &leg.sweeps {
                if std::env::var("ARACHNE_DEBUG").is_ok() {
                    let bends: Vec<String> = p.pieces.iter().map(|c| format!("({:.2},{:.2}):{:.2}", c[0][0], c[0][1], tightest_bend(c))).collect();
                    println!("    {} {part}: {}", spec.name, bends.join(" "));
                }
                ensure!(p.bend_ratio() >= 1.25, "{} {part} bends at {:.2} of its radius", spec.name, p.bend_ratio());
            }
            let radii: Vec<String> = leg.sweeps.iter().map(|(_, p)| format!("{:.2}-{:.2}", p.r0, p.r1)).collect();
            println!(
                "  {}: knee at {:.1} deg standing {:.2} proud; {:.1} mm long; radii {}; most proud {:.2}; lowest share {:.2}; abdomen margin {}",
                spec.name,
                leg.knee_deg,
                leg.knee_proud,
                leg.length,
                radii.join(" "),
                leg.most_proud,
                leg.least_share,
                if leg.abdomen_margin < 1e9 { format!("{:.2}", leg.abdomen_margin) } else { "none".into() },
            );
            Ok((*spec, leg))
        })
        .collect()
}

/// Web masks fade before both folds and clear every leg's surface footprint.
fn web_masks(
    d: &mut RingDesign,
    lib: &mut AlphaLibrary,
    bare: &Bare,
    legs: &[(LegSpec, Leg)],
) -> Result<()> {
    let atlas = skin::Atlas::of(d, 2048, 512)?;
    let ctx = d.field_context();
    let sections: Vec<(f64, f64, f64, f64)> = (0..atlas.width)
        .map(|i| {
            let theta = i as f64 / atlas.width as f64 * 360.0;
            let s = bare.at(theta);
            (s.edge_t, s.edge_r, s.bore_r, ctx.station_stretch(theta))
        })
        .collect();
    let samples: Vec<Vec<(P3, f64)>> = legs
        .iter()
        .flat_map(|(_, leg)| leg.sweeps.iter().map(|(_, path)| path.samples()))
        .collect();
    let cheek_samples: Vec<Vec<(P3, f64)>> = legs
        .iter()
        .flat_map(|(_, leg)| leg.sweeps.iter().skip(1).map(|(_, path)| path.samples()))
        .collect();
    let clearance = |p: P3| {
        let p = [p[0], p[1], -p[2].abs()];
        samples
            .iter()
            .flat_map(|s| s.windows(2))
            .map(|w| {
                let ab = sub3(w[1].0, w[0].0);
                let t = (dot(sub3(p, w[0].0), ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
                len3(sub3(p, add3(w[0].0, ab, t))) - lerp(w[0].1, w[1].1, t)
            })
            .fold(f64::MAX, f64::min)
    };
    let cheek_clearance = |s: &skin::Sample| {
        let r = s.p[0].hypot(s.p[1]);
        cheek_samples
            .iter()
            .flat_map(|p| p.windows(2))
            .map(|w| {
                let (a, b) = (theta_of(w[0].0), theta_of(w[1].0));
                let span = wrap180(b - a);
                let t =
                    (wrap180(s.theta - a) / span.abs().max(1e-12) * span.signum()).clamp(0.0, 1.0);
                let centre = add3(w[0].0, sub3(w[1].0, w[0].0), t);
                let reach = lerp(w[0].1, w[1].1, t) + 0.20;
                let along = wrap180(s.theta - theta_of(centre)).to_radians().abs() * r;
                smoothstep(reach, reach + 0.15, along).max(smoothstep(
                    reach,
                    reach + 0.15,
                    r - centre[0].hypot(centre[1]),
                ))
            })
            .fold(1.0, f64::min)
    };
    for (name, crown) in [
        ("Crown web clearance", true),
        ("Cheek web clearance", false),
    ] {
        let mut alpha = atlas.paint(name, |s| {
            let (edge, edge_r, bore, stretch) = sections[s.i % atlas.width];
            let r = s.p[0].hypot(s.p[1]);
            let fade = if crown {
                smoothstep(
                    EDGE_INSET_MM,
                    EDGE_INSET_MM + EDGE_FADE_MM,
                    edge - ((s.v - ctx.crest_v_mm) * stretch).abs(),
                )
            } else {
                smoothstep(0.30, 0.50, r - bore) * smoothstep(0.0, 0.20, edge_r - r)
            };
            fade * if crown {
                smoothstep(0.20, 0.35, clearance(s.p))
            } else {
                cheek_clearance(s)
            }
        });
        for v in &mut alpha.data {
            *v = (*v * 255.0).round() / 255.0;
        }
        lib.insert(Alpha::from_png16(name, &alpha.to_png16()?)?);
    }
    for e in &mut d.layers.layers {
        if e.name.starts_with("Cheek web") {
            e.mask = Some("Cheek web clearance".into());
        } else if matches!(e.layer, Layer::Decals(_)) {
            e.mask = Some("Crown web clearance".into());
        }
    }
    d.embed_alphas(lib);
    d.embedded.retain(|a| a.name.ends_with("web clearance"));
    Ok(())
}

// --- Assembly ---------------------------------------------------------------

/// Appends features to the design's parts, numbering them on.
struct Parts<'a> {
    doc: &'a mut Document,
    next: Id,
}

impl Parts<'_> {
    fn add(
        &mut self,
        name: impl Into<String>,
        operation: Operation,
        component: Component,
    ) -> Result<Id> {
        let id = self.next;
        self.next += 1;
        self.doc.append(Feature {
            id,
            name: name.into(),
            enabled: true,
            operation,
            component,
        })?;
        Ok(id)
    }
    /// A ball of `radius` at a world point, joined.
    fn ball(&mut self, name: &str, centre: P3, radius: f64) -> Result<Id> {
        let s = self.add(
            format!("{name} ball"),
            Operation::Sphere { radius_mm: radius },
            Component::default(),
        )?;
        self.add(
            name,
            Operation::Transform {
                source: s,
                translation: centre,
                rotation_deg: [0.0; 3],
            },
            joined(),
        )
    }
}

fn joined() -> Component {
    Component {
        attach: Attach::Join,
        placement: Placement::Free,
        blend_mm: 0.0,
        ..Component::default()
    }
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
    /// The lowest knee's top over the carapace's rim, along the ring's radius.
    knee_over_rim_mm: f64,
    leg_most_proud_mm: f64,
    leg_lowest_cheek_share: f64,
    leg_abdomen_margin_mm: f64,
    leg_lengths_mm: Vec<f64>,
    /// The thinnest section any leg sweep carries, and the claw's tip.
    leg_min_section_mm: f64,
    claw_diameter_mm: f64,
    tightest_bend_ratio: f64,
    joint_loft_bend_ratio: f64,
    joint_ease_mm: f64,
    leg_vertex_abdomen_margin_mm: f64,
    /// Rim-flat widths of the two collets and their walls' flare.
    rim_flat_mm: Vec<f64>,
    wall_flare_deg: f64,
}

fn author(
    preview: Option<&Path>,
) -> Result<(RingDesign, AlphaLibrary, Spider, WebStats, Vec<Body>)> {
    let mut d = band();
    let ctx = d.field_context();
    println!(
        "  reference crest v {:.2} of {:.2}, crest r {:.2}",
        ctx.crest_v_mm, ctx.band_v_len_mm, ctx.crest_radius_mm
    );
    let bare = Bare::new(&d);
    let abdomen_at = abdomen_deg(&bare);
    let mut head = LayerEntry::new(
        "Carapace bed, garnet",
        Layer::SeatPad(bed(HUB_DEG, ctx.crest_v_mm, garnet(), CARAPACE_RISE_MM)),
    );
    head.blend = Blend::SmoothMax;
    let mut tail = LayerEntry::new(
        "Abdomen bed, onyx",
        Layer::SeatPad(bed(abdomen_at, ctx.crest_v_mm, onyx(), ABDOMEN_RISE_MM)),
    );
    tail.blend = Blend::SmoothMax;
    cheek_web(&mut d)?;
    d.layers.layers.push(head);
    d.layers.layers.push(tail);
    let (carapace, abdomen) = bodies(&d)?;
    let (web, spinneret_deg) = crown_web(&mut d, &bare, &carapace, &abdomen, preview)?;
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    println!(
        "  carapace at {HUB_DEG:.1} deg, abdomen at {abdomen_at:.1} deg, spinnerets at {spinneret_deg:.1} deg"
    );
    println!(
        "  web: {} orbs, frieze of {} corners at {:.2} mm, longest parallel run {:.2} mm, {} KB of SVG",
        web.orbs.len(),
        web.frieze_corners,
        web.frieze_step_mm,
        web.longest_parallel_run_mm,
        web.svg_bytes / 1024
    );
    if preview.is_some() {
        return Ok((d, lib, Spider::default(), web, vec![carapace, abdomen]));
    }

    let legs = solve_legs(&bare, &carapace, &abdomen)?;
    web_masks(&mut d, &mut lib, &bare, &legs)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature {
            id: 1,
            name: "Procedural shank".into(),
            enabled: true,
            operation: Operation::Band,
            component: Component::default(),
        })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let mut parts = Parts { doc, next };
    parts.add(
        "Carapace, round the garnet",
        body_part(&carapace, GARNET_AROUND)?,
        joined(),
    )?;
    parts.add(
        "Abdomen, round the onyx",
        body_part(&abdomen, ONYX_AROUND)?,
        joined(),
    )?;
    // The pedicel: a waist between the bodies, a little under the girdles.
    let (ec, ea) = (carapace.end(1.0), abdomen.end(-1.0));
    let mid: P3 = std::array::from_fn(|k| 0.5 * (ec[k] + ea[k]));
    parts.ball(
        "Pedicel",
        add3(mid, unit3([mid[0], mid[1], 0.0]), -0.35),
        0.7,
    )?;
    let sp = bare.at(spinneret_deg);
    parts.ball(
        "Spinnerets",
        at_ring(spinneret_deg, sp.flank[0][1] + 0.18, 0.0),
        0.5,
    )?;
    let rim_flat = |b: &Body| {
        let (_, b0) = b.semi();
        let outer = b.profile().last().unwrap()[0];
        outer - (POCKET_CLEAR_MM - b0 * (1.0 - b.stone_scale(b.rim())))
    };
    let mut spider = Spider {
        knee_over_rim_mm: f64::MAX,
        carapace_deg: HUB_DEG,
        abdomen_deg: abdomen_at,
        spinneret_deg,
        tightest_bend_ratio: f64::MAX,
        joint_loft_bend_ratio: f64::MAX,
        joint_ease_mm: JOINT_EASE_MM,
        leg_vertex_abdomen_margin_mm: f64::MAX,
        leg_min_section_mm: f64::MAX,
        leg_abdomen_margin_mm: f64::MAX,
        leg_lowest_cheek_share: f64::MAX,
        claw_diameter_mm: 2.0 * CLAW_MM,
        rim_flat_mm: vec![rim_flat(&carapace), rim_flat(&abdomen)],
        wall_flare_deg: FLARE_DEG,
        ..Spider::default()
    };
    for (spec, leg) in &legs {
        let mut outputs = vec![parts.ball(
            &format!("{} coxa, low side", spec.name),
            leg.coxa.0,
            leg.coxa.1,
        )?];
        let (op, bend, margin) = leg_part(leg, &abdomen)?;
        println!(
            "  {} continuous limb: joint bend ratio {bend:.3}, actual vertex abdomen margin {}",
            spec.name,
            if margin < 1e9 {
                format!("{margin:.3} mm")
            } else {
                "none".into()
            }
        );
        outputs.push(parts.add(
            format!("{} continuous limb, low side", spec.name),
            op,
            joined(),
        )?);
        spider.joint_loft_bend_ratio = spider.joint_loft_bend_ratio.min(bend);
        spider.leg_vertex_abdomen_margin_mm = spider.leg_vertex_abdomen_margin_mm.min(margin);
        for source in outputs {
            let name = parts
                .doc
                .feature(source)
                .map(|f| f.name.replace("low side", "high side"))
                .unwrap_or_default();
            parts.add(
                name,
                Operation::Pattern {
                    sources: source.into(),
                    kind: PatternKind::Mirror {
                        plane: MirrorPlane::Band,
                    },
                },
                joined(),
            )?;
        }
        spider.knee_deg.push(leg.knee_deg);
        spider.claw_deg.push(HUB_DEG + spec.claw_deg);
        spider.knee_proud_mm = spider.knee_proud_mm.max(leg.knee_proud);
        let (knee, radius) = (leg.joints[0].1, leg.joints[0].2);
        let rim = carapace.world([0.0, 0.0, carapace.rim()]);
        spider.knee_over_rim_mm = spider
            .knee_over_rim_mm
            .min(knee[0].hypot(knee[1]) + radius - rim[0].hypot(rim[1]));
        spider.leg_most_proud_mm = spider.leg_most_proud_mm.max(leg.most_proud);
        spider.leg_lowest_cheek_share = spider.leg_lowest_cheek_share.min(leg.least_share);
        spider.leg_abdomen_margin_mm = spider.leg_abdomen_margin_mm.min(leg.abdomen_margin);
        spider.leg_lengths_mm.push(leg.length);
        spider.leg_min_section_mm = leg
            .sweeps
            .iter()
            .map(|(_, p)| 2.0 * p.r0.min(p.r1))
            .fold(spider.leg_min_section_mm, f64::min);
        spider.tightest_bend_ratio = leg
            .sweeps
            .iter()
            .map(|(_, p)| p.bend_ratio())
            .fold(spider.tightest_bend_ratio.min(bend), f64::min);
    }
    Ok((d, lib, spider, web, vec![carapace, abdomen]))
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
    min_angle_deg: f64,
    worst_aspect: f64,
    /// Faces under a twentieth of a degree at their sharpest corner, by the part they came from.
    slivers_by_part: Vec<(String, usize)>,
    made_parts: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    parts_joined: usize,
    parts_cut: usize,
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
    web: WebStats,
    cheek_web_ink: f64,
    crown_web_ink: Vec<(String, f64)>,
    longest_bare_crown_run_mm: f64,
    /// The widest disc of bare polished crown anywhere clear of the spider's bodies: the web's largest cell.
    largest_bare_cell_mm: f64,
    largest_bare_cell_deg: f64,
    layers: Vec<String>,
    cad_features: usize,
    design_bytes: u64,
    design_format: u64,
    embedded_alphas: usize,
    export_clean: ExportClean,
    grams_18k: f64,
    cold_reload_identical: Option<bool>,
    bore_clearance: Vec<(String, f64, usize)>,
    mesh_self_crossings: usize,
    geometry_gates_passed: bool,
    template_gate_passed: Option<bool>,
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
            groups[at]
                .1
                .push([tri[k * STRIDE], tri[k * STRIDE + 1], tri[k * STRIDE + 2]]);
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
                let (e1, e2) = (
                    [b.0 - a.0, b.1 - a.1, b.2 - a.2],
                    [c.0 - a.0, c.1 - a.1, c.2 - a.2],
                );
                let n = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
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
        (p.0 as f64 - centre[0])
            .hypot(p.1 as f64 - centre[1])
            .hypot(p.2 as f64 - centre[2])
            < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(
                    m.normals
                        .get(i as usize)
                        .copied()
                        .unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)),
                );
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
        out[y * edge * 6..y * edge * 6 + edge * 3]
            .copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6]
            .copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(
        path,
        &out,
        (edge * 2) as u32,
        edge as u32,
        image::ColorType::Rgb8,
    )?;
    Ok(())
}

/// Studio-gold renders with stones: named views, a spider close-up, and bare stock against the finished ring.
fn renders(
    out: &Path,
    d: &RingDesign,
    lib: &AlphaLibrary,
    built: &mesh::BuildResult,
    edge: usize,
) -> Result<()> {
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
    let centre: P3 = std::array::from_fn(|k| {
        frames.iter().map(|(_, f)| f.girdle[k]).sum::<f64>() / frames.len().max(1) as f64
    });
    let spider = crop(&built.mesh, centre, 11.0);
    let mut close = vec![
        render::Part::metal(&spider, render::GOLD),
        render::Part::metal(&built.mesh, render::GOLD),
    ];
    close.extend(stone_parts());
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = band();
    bare.name = d.name.clone();
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let bare_img = render::render_parts_ss(
        &[render::Part::metal(&b.mesh, render::GOLD)],
        0.55,
        0.95,
        edge,
        edge,
        3,
    );
    let finished_img = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    side_by_side(
        &out.join("bare-vs-finished.png"),
        &bare_img,
        &finished_img,
        edge,
    )?;
    Ok(())
}

/// Every made part's self-crossings: the CAD parts as placed.
fn crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| {
            let n = match &c.made {
                Some(m) => csg::self_crossings(m.solid()),
                None => csg::self_crossings(&csg::Solid {
                    v: c.trace.positions.clone(),
                    f: c.mesh.faces.clone(),
                }),
            };
            (c.name.clone(), n)
        })
        .collect()
}

/// Every made-part vertex inside the band's axial span clears the local bore by 0.10 mm.
fn bore_clearance(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, f64, usize)> {
    let bare = Bare::new(d);
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| {
            let mut minimum = f64::MAX;
            let mut below = 0;
            for p in &c.trace.positions {
                if let Some(bore) = bare.bore_at(theta_of(*p), p[2]) {
                    let margin = p[0].hypot(p[1]) - bore;
                    minimum = minimum.min(margin);
                    below += usize::from(margin < 0.10);
                }
            }
            (c.name.clone(), minimum, below)
        })
        .collect()
}

/// Faces sharper than a twentieth of a degree at their sharpest corner, counted by the part their vertices came from.
fn slivers(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    let m = &built.mesh;
    let names: HashMap<Id, String> = d
        .cad
        .as_ref()
        .map(|c| c.features.iter().map(|f| (f.id, f.name.clone())).collect())
        .unwrap_or_default();
    let owner = |v: u32| -> String {
        match m.origin.get(v as usize) {
            Some(&o) if o >= mesh::SOLID_VERTEX => {
                let i = (o - mesh::SOLID_VERTEX) as i64 - built.parts.first as i64;
                usize::try_from(i)
                    .ok()
                    .and_then(|i| built.parts.features.get(i))
                    .and_then(|id| names.get(id))
                    .cloned()
                    .unwrap_or_else(|| "a seat or stamp".into())
            }
            Some(_) => "band".into(),
            None => "unknown".into(),
        }
    };
    let mut count: HashMap<String, usize> = HashMap::new();
    for f in &m.faces {
        let p = f
            .map(|i| m.vertices[i as usize])
            .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
        let mut sharpest = 180.0f64;
        for k in 0..3 {
            let (a, b) = (sub3(p[(k + 1) % 3], p[k]), sub3(p[(k + 2) % 3], p[k]));
            let (la, lb) = (len3(a), len3(b));
            if la < 1e-12 || lb < 1e-12 {
                sharpest = 0.0;
                continue;
            }
            sharpest = sharpest.min((dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees());
        }
        if sharpest < 0.05 {
            let mut who: Vec<String> = f.iter().map(|&v| owner(v)).collect();
            who.sort();
            who.dedup();
            *count.entry(who.join(" / ")).or_default() += 1;
        }
    }
    let mut out: Vec<(String, usize)> = count.into_iter().collect();
    out.sort_by_key(|o| std::cmp::Reverse(o.1));
    if std::env::var("ARACHNE_DEBUG").is_ok() {
        let mut degenerate: HashMap<String, usize> = HashMap::new();
        for f in &m.faces {
            let p = f
                .map(|i| m.vertices[i as usize])
                .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
            let area = 0.5 * len3(cross3(sub3(p[1], p[0]), sub3(p[2], p[0])));
            if area < 1e-10 {
                let mut who: Vec<String> = f.iter().map(|&v| owner(v)).collect();
                who.sort();
                who.dedup();
                *degenerate
                    .entry(format!(
                        "{} at ({:.3}, {:.3}, {:.3})",
                        who.join(" / "),
                        p[0][0],
                        p[0][1],
                        p[0][2]
                    ))
                    .or_default() += 1;
            }
        }
        for (k, n) in degenerate {
            println!("    degenerate: {n} on {k}");
        }
        let mut worst: Vec<(f64, usize)> = m
            .faces
            .iter()
            .enumerate()
            .map(|(k, f)| {
                let p = f
                    .map(|i| m.vertices[i as usize])
                    .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
                let e = [
                    len3(sub3(p[1], p[0])),
                    len3(sub3(p[2], p[1])),
                    len3(sub3(p[0], p[2])),
                ];
                let longest = e[0].max(e[1]).max(e[2]);
                let area = 0.5 * len3(cross3(sub3(p[1], p[0]), sub3(p[2], p[0])));
                (longest * longest / (2.0 * area).max(1e-30), k)
            })
            .collect();
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (aspect, k) in worst.iter().take(6) {
            let f = m.faces[*k];
            let p = f
                .map(|i| m.vertices[i as usize])
                .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
            let e = [
                len3(sub3(p[1], p[0])),
                len3(sub3(p[2], p[1])),
                len3(sub3(p[0], p[2])),
            ];
            let who: Vec<String> = f.iter().map(|&v| owner(v)).collect();
            println!(
                "    worst face: aspect {aspect:.0}, edges {:.6} {:.6} {:.6} at ({:.3}, {:.3}, {:.3}) on {}",
                e[0],
                e[1],
                e[2],
                p[0][0],
                p[0][1],
                p[0][2],
                who.join(" / ")
            );
        }
        let mut wide: Vec<(f64, u32)> = m
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| (v.2.abs() as f64, i as u32))
            .collect();
        wide.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (z, i) in wide.iter().take(4) {
            let v = m.vertices[*i as usize];
            println!(
                "    widest: |z| {z:.3} at {:.1} deg on {}",
                (v.1 as f64).atan2(v.0 as f64).to_degrees(),
                owner(*i)
            );
        }
    }
    out
}

/// Slivers thinner than this are collapsed or flipped away in the exported mesh.
const CLEAN_MM: f64 = 1e-3;

/// What the export's clean did.
#[derive(serde::Serialize)]
struct ExportClean {
    eps_mm: f64,
    faces_removed: usize,
    watertight: bool,
    degenerate_faces: usize,
    min_angle_deg: f64,
    worst_aspect: f64,
}

/// The built mesh with every sliver thinner than `eps` collapsed or flipped away, as the export writes it.
fn cleaned(m: &mesh::Mesh, eps: f64) -> (mesh::Mesh, ExportClean) {
    let mut solid = csg::Solid {
        v: m.vertices
            .iter()
            .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
            .collect(),
        f: m.faces.clone(),
    };
    let faces_removed = csg::clean(&mut solid, eps);
    let mut out = mesh::Mesh {
        vertices: solid
            .v
            .iter()
            .map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32))
            .collect(),
        faces: solid.f.clone(),
        ..mesh::Mesh::default()
    };
    let mut normals = vec![[0.0f64; 3]; solid.v.len()];
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let n = cross3(sub3(b, a), sub3(c, a));
        for &i in f {
            normals[i as usize] = add3(normals[i as usize], n, 1.0);
        }
    }
    out.normals = normals
        .iter()
        .map(|n| unit3(*n))
        .map(|n| mesh::Vec3(n[0] as f32, n[1] as f32, n[2] as f32))
        .collect();
    let q = out.quality();
    let stats = ExportClean {
        eps_mm: eps,
        faces_removed,
        watertight: out.validate().watertight,
        degenerate_faces: q.degenerate_faces,
        min_angle_deg: q.min_angle_deg,
        worst_aspect: q.worst_aspect,
    };
    (out, stats)
}

/// Metal vertices standing inside each cabochon, 0.03 mm in from its surface: the stones must sit clear.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    ringdesign_core::stones::stone_frames(d)
        .into_iter()
        .map(|(st, f)| {
            let (a, b, h) = (
                st.gem.l_mm * 0.5 - 0.03,
                st.gem.w_mm * 0.5 - 0.03,
                st.gem.depth_mm() - 0.03,
            );
            let inside = built
                .mesh
                .vertices
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    let q = sub3([p.0 as f64, p.1 as f64, p.2 as f64], f.girdle);
                    let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                    let inside = z > 0.03
                        && z < h
                        && (x / a).powi(2) + (y / b).powi(2) < 1.0 - (z / h).powi(2);
                    if inside && std::env::var("ARACHNE_DEBUG").is_ok() {
                        let owner = built
                            .mesh
                            .origin
                            .get(*i)
                            .and_then(|o| {
                                o.checked_sub(mesh::SOLID_VERTEX + built.parts.first as u32)
                            })
                            .and_then(|j| built.parts.features.get(j as usize))
                            .and_then(|id| d.cad.as_ref()?.feature(*id))
                            .map(|f| f.name.as_str())
                            .unwrap_or("band or seat");
                        println!(
                            "    {} intruded by {owner} at ({x:.3}, {y:.3}, {z:.3})",
                            st.label
                        );
                    }
                    inside
                })
                .count();
            (st.label, inside)
        })
        .collect()
}

/// Share of an alpha's texels that are ink.
fn ink(lib: &AlphaLibrary, name: &str) -> f64 {
    lib.get(name).map_or(0.0, |a| {
        a.data.iter().filter(|v| **v >= 0.5).count() as f64 / a.data.len().max(1) as f64
    })
}

/// The crown sampled on a metal grid: whether each point is bare polish, or covered by the web, the bodies or the band's edge.
struct CrownGrid {
    step: f64,
    cols: usize,
    rows: usize,
    /// Row 0 is `-top` across; true where the crown is bare.
    bare: Vec<bool>,
    top: f64,
    theta: Vec<f64>,
}

fn crown_grid(d: &RingDesign, lib: &AlphaLibrary, bodies: &[Body]) -> CrownGrid {
    let ctx = d.field_context();
    let bare_band = Bare::new(d);
    let ch = Chart {
        bare: &bare_band,
        ctx: d.field_context(),
    };
    let web = Web::new(&ch);
    let step = 0.05;
    let cols = (web.circ / step).round() as usize;
    let top = (0..360).map(|k| ch.edge(k as f64)).fold(0.0, f64::max);
    let rows = (2.0 * top / step).ceil() as usize + 1;
    let theta: Vec<f64> = (0..cols)
        .map(|i| web.theta(-0.5 * web.circ + i as f64 * step))
        .collect();
    let mut bare = vec![false; cols * rows];
    for (i, &th) in theta.iter().enumerate() {
        let edge = ch.edge(th) - 0.15;
        let u = ctx.u_of_theta(th.rem_euclid(360.0));
        let stretch = ctx.station_stretch(th.rem_euclid(360.0));
        for j in 0..rows {
            let t = -top + j as f64 * step;
            if t.abs() > edge {
                continue;
            }
            let (r, z) = bare_band.surface_at_t(th, t);
            let w = at_ring(th, r, z);
            if bodies.iter().any(|b| b.margin(w) < 0.0) {
                continue;
            }
            let h = d.layers.height(
                Uv {
                    u,
                    v: ctx.crest_v_mm + t / stretch,
                },
                &ctx,
                lib,
            );
            bare[j * cols + i] = h < 0.03;
        }
    }
    CrownGrid {
        step,
        cols,
        rows,
        bare,
        top,
        theta,
    }
}

/// Squared distances to the nearest zero, one dimension (Felzenszwalb and Huttenlocher).
fn edt_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let inf = 1e20;
    let mut first = true;
    for q in 0..n {
        if f[q] >= inf {
            continue;
        }
        if first {
            v[0] = q;
            k = 0;
            z[0] = f64::NEG_INFINITY;
            z[1] = f64::INFINITY;
            first = false;
            continue;
        }
        loop {
            let p = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64))
                / (2.0 * q as f64 - 2.0 * p as f64);
            if s <= z[k] && k > 0 {
                k -= 1;
                continue;
            }
            if s <= z[k] {
                v[0] = q;
                z[0] = f64::NEG_INFINITY;
                z[1] = f64::INFINITY;
                k = 0;
                break;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    if first {
        return vec![inf; n];
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f64 - p as f64).powi(2) + f[p];
    }
    d
}

/// The widest bare disc on the crown, diameter mm, and the ring angle it sits at.
fn largest_bare_cell(g: &CrownGrid) -> (f64, f64) {
    let pad = (3.0 / g.step) as usize;
    let w = g.cols + 2 * pad;
    let inf = 1e20;
    let mut f = vec![0.0; w * g.rows];
    for j in 0..g.rows {
        for i in 0..w {
            let c = (i + g.cols - pad) % g.cols;
            f[j * w + i] = if g.bare[j * g.cols + c] { inf } else { 0.0 };
        }
    }
    let mut col = vec![0.0; g.rows];
    for i in 0..w {
        for j in 0..g.rows {
            col[j] = f[j * w + i];
        }
        let d = edt_1d(&col);
        for j in 0..g.rows {
            f[j * w + i] = d[j];
        }
    }
    let (mut best, mut at) = (0.0f64, 0usize);
    for j in 0..g.rows {
        let d = edt_1d(&f[j * w..(j + 1) * w]);
        for (i, di) in d.iter().enumerate().skip(pad).take(g.cols) {
            if *di > best && *di < inf {
                best = *di;
                at = i - pad;
            }
        }
    }
    (2.0 * best.sqrt() * g.step, g.theta[at].rem_euclid(360.0))
}

/// The longest run of bare polished crown along the crest and two lines either side, clear of the bodies, metal mm.
fn longest_bare_run(g: &CrownGrid) -> f64 {
    let mut worst: f64 = 0.0;
    for share in [0.0, 0.5, -0.5] {
        let j = (((share * (g.top - 0.15)) + g.top) / g.step).round() as usize;
        let mut run = 0.0;
        for i in 0..2 * g.cols {
            if g.bare[j * g.cols + i % g.cols] {
                run += g.step;
                if run > worst && run > 3.5 && i < g.cols && std::env::var("ARACHNE_DEBUG").is_ok()
                {
                    println!(
                        "    bare run {run:.2} mm ends at {:.2} deg, crown share {share}",
                        g.theta[i]
                    );
                }
                worst = worst.max(run);
            } else {
                run = 0.0;
            }
        }
    }
    worst.min(g.cols as f64 * g.step)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let web_only = args.iter().any(|a| a == "--web-preview");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/arachne")
        });
    std::fs::create_dir_all(&out)?;
    println!("Arachne");
    if web_only {
        let path = out.join("web-preview.svg");
        let (d, ..) = author(Some(&path))?;
        for svg in &d.svgs {
            std::fs::write(
                out.join(format!(
                    "{}.svg",
                    svg.name
                        .to_lowercase()
                        .replace([' ', ','], "-")
                        .replace("--", "-")
                )),
                &svg.svg,
            )?;
        }
        println!("  wrote {}", path.display());
        return Ok(());
    }
    let (d, lib, spider, web, bodies) = author(None)?;
    let params = if draft {
        draft_params()
    } else {
        export_params()
    };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let quality = built.report.quality;
    println!(
        "  {} triangles in {build_s:.1} s; watertight {}; degenerate {}; sharpest corner {:.5} deg, worst aspect {:.0}",
        built.mesh.faces.len(),
        v.watertight,
        quality.degenerate_faces,
        quality.min_angle_deg,
        quality.worst_aspect
    );
    let made_parts = crossings(&built);
    let bore_clearance = bore_clearance(&d, &built);
    let mesh_self_crossings = csg::self_crossings(&csg::Solid {
        v: built
            .mesh
            .vertices
            .iter()
            .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64])
            .collect(),
        f: built.mesh.faces.clone(),
    });
    let slivers_by_part = slivers(&d, &built);
    let (export_mesh, export_clean) = cleaned(&built.mesh, CLEAN_MM);
    println!(
        "  export clean at {:.4} mm: {} faces gone; watertight {}; degenerate {}; sharpest corner {:.4} deg, worst aspect {:.0}",
        export_clean.eps_mm,
        export_clean.faces_removed,
        export_clean.watertight,
        export_clean.degenerate_faces,
        export_clean.min_angle_deg,
        export_clean.worst_aspect
    );
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let previewed = stones(&d, &lib).len();
    let grams = built
        .report
        .metals
        .iter()
        .find(|m| m.metal == "Gold 18k")
        .map_or(0.0, |m| m.grams);
    let mut warnings: Vec<String> = stones_report
        .iter()
        .flat_map(|s| {
            s.seats
                .iter()
                .flat_map(|seat| seat.warnings.iter().cloned())
        })
        .collect();
    warnings.dedup();
    let zs = built.mesh.vertices.iter().map(|p| p.2 as f64);
    let z_extent = zs.clone().fold(f64::MIN, f64::max) - zs.fold(f64::MAX, f64::min);
    let head_width = 2.0 * Bare::new(&d).at(90.0).half_width;
    library::save_design(out.join("design.ring.json"), &d)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let design_bytes = text.len() as u64;
    let design_format = serde_json::from_str::<serde_json::Value>(&text)?
        .get("format_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices
            && rebuilt.mesh.faces == built.mesh.faces
            && rebuilt.mesh.normals == built.mesh.normals;
        println!(
            "  cold reload with an empty library: {}",
            if same { "identical" } else { "DIFFERENT" }
        );
        Some(same)
    } else {
        None
    };
    let crown_web_ink: Vec<(String, f64)> = d
        .svgs
        .iter()
        .filter(|s| s.name != "Cheek web")
        .map(|s| (s.name.clone(), ink(&lib, &s.name)))
        .collect();
    let grid = crown_grid(&d, &lib, &bodies);
    let bare_run = longest_bare_run(&grid);
    let (cell, cell_deg) = largest_bare_cell(&grid);
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
        degenerate_faces: quality.degenerate_faces,
        min_angle_deg: quality.min_angle_deg,
        worst_aspect: quality.worst_aspect,
        slivers_by_part,
        made_parts,
        solids_notes: built.solids.notes.clone(),
        parts_notes: built.parts.notes.clone(),
        parts_joined: built.parts.joined,
        parts_cut: built.parts.cut,
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
        dfm_findings: findings
            .iter()
            .map(|f| format!("{}: {}", f.label, f.message))
            .collect(),
        stones_reported: stones_report.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        metal_inside_stones: metal_in_stones(&d, &built),
        stone_carats: stones_report.as_ref().map_or(0.0, |s| s.total_carats),
        stone_warnings: warnings,
        closest_stones: stones_report
            .as_ref()
            .and_then(|s| s.closest.as_ref())
            .map(|p| {
                format!(
                    "{} to {}: {:.2} mm at the girdle, {:.2} mm deep",
                    p.a, p.b, p.gap_mm, p.gap_deep_mm
                )
            }),
        z_extent_mm: z_extent,
        head_width_mm: head_width,
        spider,
        web,
        cheek_web_ink: ink(&lib, "Cheek web"),
        crown_web_ink,
        longest_bare_crown_run_mm: bare_run,
        largest_bare_cell_mm: cell,
        largest_bare_cell_deg: cell_deg,
        layers: d.layers.layers.iter().map(|e| e.name.clone()).collect(),
        cad_features: d.cad.as_ref().map_or(0, |c| c.features.len()),
        design_bytes,
        design_format,
        embedded_alphas: d.embedded.len(),
        export_clean,
        grams_18k: grams,
        cold_reload_identical: cold,
        bore_clearance,
        mesh_self_crossings,
        geometry_gates_passed: false,
        template_gate_passed: None,
        gates_passed: false,
    };
    report.geometry_gates_passed = report.watertight
        && report.degenerate_faces == 0
        && report.mesh_self_crossings == 0
        && report.bore_clearance.iter().all(|(_, _, n)| *n == 0)
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
        && report.spider.knee_over_rim_mm >= 0.4
        && report.spider.leg_min_section_mm >= MIN_SECTION_MM - 1e-9
        && report.spider.claw_diameter_mm >= MIN_SECTION_MM - 1e-9
        && report.spider.tightest_bend_ratio >= 1.25
        && report.spider.leg_lowest_cheek_share >= 0.35
        && report.spider.leg_abdomen_margin_mm >= 0.40
        && report.spider.leg_vertex_abdomen_margin_mm >= 0.40
        && report.spider.leg_lengths_mm[3] <= 13.0
        && report.longest_bare_crown_run_mm <= 3.5
        && report.largest_bare_cell_mm <= 2.0
        && report.web.longest_parallel_run_mm <= 3.0
        && report.web.free_hub_root_gap_mm >= 0.15
        && report.export_clean.watertight
        && report.export_clean.degenerate_faces == 0
        && report.cold_reload_identical != Some(false);
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(
        out.join("verification.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "design_bytes": design_bytes,
            "geometry_gates_passed": report.geometry_gates_passed,
            "cold_design_reload": cold,
            "template_gate_passed": null,
            "gates_passed": false,
            "status": "Template lift and cold graph validation pending for this design"
        }))?,
    )?;
    std::fs::write(
        out.join("mesh.json"),
        serde_json::to_vec_pretty(&built.report)?,
    )?;
    let art = out.join("artwork");
    let _ = std::fs::remove_dir_all(&art);
    std::fs::create_dir_all(&art)?;
    for svg in &d.svgs {
        std::fs::write(
            art.join(format!(
                "{}.svg",
                svg.name
                    .to_lowercase()
                    .replace([' ', ','], "-")
                    .replace("--", "-")
            )),
            &svg.svg,
        )?;
    }
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &export_mesh, &d.name)?;
        let mut setup = mf::Setup::from_design(&d);
        setup.recipe.name = "Arachne / investment / Gold 18k".into();
        setup.recipe.alloy = "Gold 18k".into();
        setup.recipe.sand = None;
        setup.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").unwrap().shrink_pct;
        setup.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
        setup.bench_notes = "Investment cast the body and limbs. Clean investment from the web and limb clearances. Finish the two collet bearings for the actual garnet and onyx, then set after casting.".into();
        let prepared = mf::prepare(&d, &lib, &setup, params)?;
        let (pattern, clean) = cleaned(&prepared.mesh, CLEAN_MM);
        let pattern_crossings = csg::self_crossings(&csg::Solid {
            v: pattern
                .vertices
                .iter()
                .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
                .collect(),
            f: pattern.faces.clone(),
        });
        ensure!(
            clean.watertight && clean.degenerate_faces == 0 && pattern_crossings == 0,
            "Prepared investment pattern failed its mesh gates"
        );
        stl::write_stl(out.join("casting-pattern.stl"), &pattern, &d.name)?;
        std::fs::write(
            out.join("pattern-report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "setup": setup, "scale": prepared.scale, "triangles": pattern.faces.len(),
                "clean": clean, "self_crossings": pattern_crossings, "notes": prepared.notes,
                "bench_layers": prepared.bench_layers
            }))?,
        )?;
        for (stone, tint) in stones(&d, &lib) {
            let name = if tint[0] > 0.1 {
                "reference-garnet.stl"
            } else {
                "reference-onyx.stl"
            };
            stl::write_stl(out.join(name), &stone, "Arachne reference stone")?;
        }
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
        "  z extent {:.2} mm over a {:.2} mm head; knees {:.2} proud and {:.2} over the rim, legs {:.2}; thinnest leg {:.2} mm; cheek ink {:.2}; longest bare crown run {:.1} mm; widest bare cell {:.2} mm at {:.0} deg; design {} KB at format {}",
        report.z_extent_mm,
        report.head_width_mm,
        report.spider.knee_proud_mm,
        report.spider.knee_over_rim_mm,
        report.spider.leg_most_proud_mm,
        report.spider.leg_min_section_mm,
        report.cheek_web_ink,
        report.longest_bare_crown_run_mm,
        report.largest_bare_cell_mm,
        report.largest_bare_cell_deg,
        report.design_bytes / 1024,
        report.design_format
    );
    for (who, n) in report.slivers_by_part.iter().take(8) {
        println!("    slivers: {n} on {who}");
    }
    for (stone, n) in &report.metal_inside_stones {
        println!("    {stone}: {n} metal vertices inside the stone");
    }
    for f in &report.dfm_findings {
        println!("    dfm: {f}");
    }
    for n in report.parts_notes.iter().chain(&report.solids_notes) {
        println!("    note: {n}");
    }
    println!(
        "  geometry gates {}; template gate pending",
        if report.geometry_gates_passed {
            "passed"
        } else {
            "FAILED"
        }
    );
    ensure!(
        report.geometry_gates_passed,
        "Arachne failed its geometry gates; see {}",
        out.join("report.json").display()
    );
    Ok(())
}
