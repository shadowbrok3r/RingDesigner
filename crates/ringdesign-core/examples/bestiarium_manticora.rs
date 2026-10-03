//! Bestiarium — Manticora, the tail that throws: a scorpion's tail round the finger, cast in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_manticora
//! target/release/examples/bestiarium_manticora [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{
        Attach, Component, ComponentRole, Document, Feature, Joint, Operation, Placement, Stage,
        SurfaceKind, builders, stored,
    },
    castability::{self, CastProcess},
    csg, dfm, library, manufacturing as mf,
    field::{
        Blend, Layer, LayerEntry, SeatRunLayer, SeatStyle, Uv, Window, smoothstep, wrap_delta,
    },
    gem::{Gem, GemCut},
    mesh, outline,
    profile::{MAX_PROFILE_STEPS, ShankKey},
    render,
    setting::{self, Plan, RowPath, SolidKind, Stamp, StampRow, StampTop, Station},
    sketch::Id,
    skin::{self, Atlas, Hide, Joints, Sample},
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P2 = [f64; 2];
type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// Ring angle of the ruby, degrees: a touch forward of the top, clear of the knob behind it.
const STONE_DEG: f64 = 86.0;
/// The investment's fill floor and detail floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;

/// Ring angle of the knob the aculeus stands on, degrees.
const KNOB_DEG: f64 = 108.0;
/// The knob's plan: semi-axes in the hook's plane (along the ring) and across it, mm.
const KNOB_A_MM: f64 = 1.0;
const KNOB_B_MM: f64 = 1.2;
/// How far the knob's top stands over the highest point of the finished bulb under it, mm.
const KNOB_RISE_MM: f64 = 1.2;
/// The seam bead where the knob meets the bulb, mm.
const KNOB_BLEND_MM: f64 = 0.4;
/// The rolled edges either side of the solder seam, mm: the articulation groove's depth.
const JOINT_ROLL_MM: f64 = 0.18;
/// Gap left under the aculeus's foot for the solder, mm.
const SOLDER_GAP_MM: f64 = 0.05;
/// How far the hook's plane turns off the ring's mid-plane about the chord from its foot to its point, degrees, and the
/// shares of the length over which the turn comes in, so the foot stands straight on its knob.
const STING_LEAN_DEG: f64 = -30.0;
const STING_TWIST: (f64, f64) = (0.1, 0.55);
/// The point's rounding radius and its last bend's radius, mm.
const TIP_RADIUS_MM: f64 = 0.2;
const TIP_BEND_MM: f64 = 0.95;
/// Where the point hangs: along the ring from the ruby's centre toward the knob, and over its table, mm.
const TIP_ALONG_MM: f64 = 0.2;
const TIP_OVER_TABLE_MM: f64 = 1.6;
/// The heading the point is chosen for, degrees round from the foot's axis toward the ruby.
const TIP_HEADING_DEG: f64 = 216.0;
/// Half-sizes of the aculeus's section, in its bending plane and across it, by share of its length: the knob's outline at
/// the foot, the vesicle's widest a quarter along, then one unbroken taper to the point.
const ACULEUS_A: [(f64, f64); 12] = [
    (0.0, 1.0),
    (0.05, 1.22),
    (0.12, 1.42),
    (0.25, 1.5),
    (0.35, 1.30),
    (0.45, 1.02),
    (0.54, 0.74),
    (0.59, 0.56),
    (0.63, 0.43),
    (0.75, 0.42),
    (0.85, 0.412),
    (1.0, 0.405),
];
const ACULEUS_B: [(f64, f64); 12] = [
    (0.0, 1.2),
    (0.05, 1.33),
    (0.12, 1.42),
    (0.25, 1.45),
    (0.35, 1.27),
    (0.45, 1.0),
    (0.54, 0.73),
    (0.59, 0.55),
    (0.63, 0.425),
    (0.75, 0.415),
    (0.85, 0.408),
    (1.0, 0.402),
];
/// How far the vesicle's belly swells to the bend's outside, mm.
const VESICLE_BELLY_MM: f64 = 0.3;
/// Points round the aculeus's section, and its step along the centre line, mm.
const ACULEUS_AROUND: usize = 64;
const ACULEUS_STEP_MM: f64 = 0.04;

/// The collet's wall, its lip as a share of the ruby's crown, and how far its top stands over the bulb's crest, mm.
const COLLET_WALL_MM: f64 = 0.8;
const COLLET_LIP: f64 = 0.35;
const LIP_OVER_CREST_MM: f64 = 0.2;
/// The calyx round the collet: how far under the collet's top its flare leaves the wall, and the seam bead into the bulb, mm.
const CALYX_UNDER_LIP_MM: f64 = 0.35;
const CALYX_BLEND_MM: f64 = 0.3;
/// The bead collar sunk into the calyx under the collet's plain wall: bead diameter, least pitch, sink, and the room left round the telson root, mm.
const COLLAR_BEAD_MM: f64 = 0.4;
const COLLAR_PITCH_MM: f64 = 0.6;
const COLLAR_SINK_MM: f64 = 0.08;
const COLLAR_KNOB_GAP_MM: f64 = 1.7;
/// Least clearance a collar bead keeps from the bulb, mm.
const COLLAR_GROUND_MM: f64 = 0.45;
/// The collet builder's clearance round the girdle, mm.
const COLLET_CLEAR_MM: f64 = 0.03;
/// The dish carved under the ruby: its depth budget and clearance off the pavilion, mm, and the share of the plan its flat floor spans.
const SEAT_MM: f64 = 2.0;
const SEAT_CLEAR_MM: f64 = 0.05;
const SEAT_FLAT: f64 = 0.45;

/// The spinel mound: this much wider than its stone, this high, and the metal left between two, mm.
const MOUND_STOCK_MM: f64 = 0.6;
const MOUND_HEIGHT_MM: f64 = 0.72;
const SPINEL_BRIDGE_MM: f64 = 1.9;
/// Least arc from the top to the neck joint, degrees.
const NECK_MIN_DEG: f64 = 24.0;
/// Where the spinels stop down each shoulder, degrees from the top.
const SPINEL_REACH_DEG: f64 = 150.0;

/// Painted layer heights, mm.
const TERGITE_MM: f64 = 1.12;
const PLEURA_MM: f64 = 0.14;
const GRAVER_MM: f64 = 0.06;
const ARTICULATION_MM: f64 = 0.45;
/// The plate's share of the tergite layer; the carinae take the rest.
const PLATE_SHARE: f64 = 0.70;
/// The dorsal carina's continuous ridge and each tooth over it, as shares of the tergite layer.
const KEEL_RIDGE: f64 = 0.11;
const KEEL_TOOTH: f64 = 0.16;
/// How much narrower a plate stands at its anterior joint than at its widest, as a share of the rim.
const PLAN_TAPER: f64 = 0.14;
/// The articulation carve kept round the palm, and the arc it eases over, degrees from 270.
const PALM_CARVE: f64 = 0.12;
const PALM_EASE_DEG: (f64, f64) = (26.0, 40.0);
/// Ground held flat along the ring round every quill's footprint, mm; it eases back over as much again.
const QUILL_GROUND_MM: f64 = 0.3;
/// The posterior condyle's roll, the plates' plan corners and the carinae's tooth pitch, mm.
const CONDYLE_MM: f64 = 0.5;
const CORNER_MM: f64 = 0.6;
const DENTICLE_PITCH_MM: f64 = 0.9;
/// How far down the flank the plates reach, and the share of it their margin rolls over.
const PLATE_REACH: f64 = 0.9;
const MARGIN_ROLL: f64 = 0.22;
/// The atlas the hide is painted on: square texels over the 72 x 6.8 mm chart.
const AW: usize = 2048;
const AH: usize = 192;
/// Across the plate as a share of its rim: the two dorsal carinae.
const DORSAL_Q: f64 = 0.44;

/// Quill length at the neck, its width, where it stands across the flank as a share of the rim, and its splay, mm and degrees.
const QUILL_MM: f64 = 4.7;
const QUILL_W_MM: f64 = 1.0;
const QUILL_Q: f64 = 1.25;
const QUILL_SPLAY_DEG: f64 = 22.0;
/// Least length of the three quills nearest the head on each row, mm.
const QUILL_HEAD_MM: f64 = 4.0;

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

/// The keyed HighDome tail: a broad venom bulb at the top, a neck `neck_off` degrees off it, then segments thinning to the palm.
fn band(neck_off: f64) -> RingDesign {
    let mut d = RingDesign {
        name: "Manticora \u{2014} the tail that throws".into(),
        ..RingDesign::default()
    };
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::HighDome);
    d.profile.width_mm = 5.4;
    d.profile.thickness_mm = 2.2;
    d.profile.comfort_fit_mm = 0.15;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64, crown_scale: f64| ShankKey {
        theta_deg,
        width_scale,
        thickness_scale,
        crown_scale,
    };
    let mut keys = vec![key(90.0, 1.85, 1.76, 0.56), key(270.0, 0.92, 0.92, 1.0)];
    for (off, w, t, c) in [
        (0.5 * neck_off, 1.78, 1.70, 0.62),
        (0.78 * neck_off, 1.42, 1.45, 0.8),
        (neck_off, 1.00, 1.08, 1.0),
        (neck_off + 17.0, 1.13, 1.30, 1.0),
        (85.0, 1.07, 1.19, 1.0),
        (120.0, 1.01, 1.08, 1.0),
        (152.0, 0.95, 0.98, 1.0),
    ] {
        keys.push(key(90.0 - off, w, t, c));
        keys.push(key(90.0 + off, w, t, c));
    }
    keys.iter_mut()
        .for_each(|k| k.theta_deg = k.theta_deg.rem_euclid(360.0));
    keys.sort_by(|a, b| a.theta_deg.total_cmp(&b.theta_deg));
    d.shank.keys = keys;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

fn ruby() -> Gem {
    Gem {
        l_mm: 7.0,
        preview_tint: Some([0.45, 0.01, 0.04]),
        ..Gem::calibrated(GemCut::Oval, 5.0)
    }
}

fn spinel() -> Gem {
    Gem {
        preview_tint: Some([0.012, 0.012, 0.016]),
        ..Gem::calibrated(GemCut::Princess, 2.0)
    }
}


/// One graded, turned lattice of spinel stations round the whole ring, mirror-true about the top with a joint at the palm.
struct Lattice {
    run: SeatRunLayer,
    stations: Vec<f64>,
    /// Joints on the rising-angle shoulder from the neck to the palm, degrees.
    joints_deg: Vec<f64>,
    /// Where the spinels stop on that shoulder, a joint, degrees.
    spinel_end_deg: f64,
}

impl Lattice {
    fn neck_off(&self) -> f64 {
        self.joints_deg[0] - 90.0
    }
    /// Stations standing between two joints on the rising shoulder, degrees.
    fn plate_centres(&self) -> Vec<f64> {
        let mut out: Vec<f64> = self
            .stations
            .iter()
            .copied()
            .filter(|t| *t > self.joints_deg[0] && *t < 270.0)
            .collect();
        out.sort_by(f64::total_cmp);
        out
    }
}

fn lattice(d: &RingDesign) -> Result<Lattice> {
    let ctx = d.field_context();
    let mut run = SeatRunLayer {
        gem: spinel(),
        bridge_mm: SPINEL_BRIDGE_MM,
        taper: 0.55,
        taper_theta_deg: 90.0,
        tilt_deg: 45.0,
        ..Default::default()
    };
    run.seat.style = SeatStyle::GypsyMound;
    run.seat.v_mm = ctx.crest_v_mm;
    run.seat.solid = SolidKind::Bead;
    run.solve_spacing(&ctx);
    run.seat.diameter_mm = run.gem.w_mm + MOUND_STOCK_MM;
    run.seat.elong = 1.0;
    run.seat.height_mm = MOUND_HEIGHT_MM;
    run.seat.crown = 1.0;
    run.seat.blend_mm = 0.25;
    let k = ctx.arc_scale(run.seat.v_mm);
    let span = run.seat_span_mm().max(0.5) * k;
    let b = run.bridge_mm;
    // Station k sits at warped angle (2k - m) pi / n when c = tan(m pi / 2n): mirror-true about the top,
    // with a joint on the palm when m + n is odd. The constant-bridge law then fixes taper and bridge.
    let circ = ctx.circumference_mm * k;
    let natural = (circ / ((span + b) * (span * (1.0 - run.taper) + b)).sqrt()).round() as i64;
    let mut best: Option<(f64, u32, f64, f64)> = None;
    for n in (natural - 3).max(6)..=natural + 3 {
        for m in 1..n {
            if (m + n) % 2 == 0 {
                continue;
            }
            let c = (m as f64 * PI / (2.0 * n as f64)).tan();
            if !(0.4..1.0).contains(&c) {
                continue;
            }
            let taper = circ * (1.0 / c - c) / (n as f64 * span);
            let bridge = circ / (n as f64 * c) - span;
            if !(0.3..0.75).contains(&taper) || bridge < 1.2 {
                continue;
            }
            let score = (taper - 0.55).abs() + 0.5 * (bridge - SPINEL_BRIDGE_MM).abs();
            if best.is_none_or(|(s, ..)| score < s) {
                best = Some((score, n as u32, taper, bridge));
            }
        }
    }
    let (_, n, taper, bridge) = best.ok_or_else(|| anyhow::anyhow!("No mirror-true spinel lattice"))?;
    run.count = n;
    run.taper = taper;
    run.bridge_mm = bridge;
    let n = run.count as usize;
    let stations: Vec<f64> = (0..n)
        .map(|k| run.theta_of_station(k as f64, &ctx).rem_euclid(360.0))
        .collect();

    for t in &stations {
        let m = (180.0 - t).rem_euclid(360.0);
        ensure!(
            stations
                .iter()
                .any(|u| ringdesign_core::field::wrap_delta(u - m, 360.0).abs() < 1e-6),
            "The spinel lattice is not mirror-true at {t:.4} deg"
        );
    }
    let mut sorted = stations.clone();
    sorted.sort_by(f64::total_cmp);
    let mut joints: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b) = (sorted[i], sorted[(i + 1) % n]);
            let b = if b < a { b + 360.0 } else { b };
            (0.5 * (a + b)).rem_euclid(360.0)
        })
        .filter(|t| *t > 90.0 && *t <= 270.0 + 1e-6)
        .collect();
    joints.sort_by(f64::total_cmp);
    let first = joints
        .iter()
        .position(|t| t - 90.0 >= NECK_MIN_DEG)
        .ok_or_else(|| anyhow::anyhow!("No joint clears the bulb"))?;
    joints.drain(..first);
    ensure!(
        (joints.last().copied().unwrap_or(0.0) - 270.0).abs() < 1e-6,
        "The lattice has no joint at the palm"
    );
    let spinel_end_deg = joints
        .iter()
        .copied()
        .filter(|t| t - 90.0 <= SPINEL_REACH_DEG)
        .fold(joints[0], f64::max);
    println!(
        "  spinel lattice: {n} stations, taper {taper:.4}, bridge {:.3} mm asked {:.2}; neck at {:.2} deg off the top, spinels to {:.2}",
        run.bridge_at(&ctx),
        run.bridge_mm,
        joints[0] - 90.0,
        spinel_end_deg - 90.0
    );
    Ok(Lattice {
        run,
        stations,
        joints_deg: joints,
        spinel_end_deg,
    })
}

/// The two spinel runs, one per shoulder, windowed from the neck to where the spinels stop.
fn spinel_runs(d: &mut RingDesign, lat: &Lattice) {
    let (a, b) = (lat.joints_deg[0], lat.spinel_end_deg);
    for (name, centre) in [
        ("Venom spinels, west", 0.5 * (a + b)),
        ("Venom spinels, east", 180.0 - 0.5 * (a + b)),
    ] {
        let mut e = LayerEntry::new(name, Layer::SeatRun(lat.run.clone()));
        e.window = Window::around(centre.rem_euclid(360.0), b - a);
        e.window.fade_deg = 0.0;
        e.blend = Blend::Max;
        d.layers.layers.push(e);
    }
}

// --- Geometry ----------------------------------------------------------------------

fn add3(a: P3, b: P3, k: f64) -> P3 {
    std::array::from_fn(|i| a[i] + b[i] * k)
}
fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit3(a: P3) -> P3 {
    let l = dot3(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}
/// Unit radial and ring-forward directions at a ring angle.
fn er(theta: f64) -> P3 {
    let t = theta.to_radians();
    [t.cos(), t.sin(), 0.0]
}
fn eth(theta: f64) -> P3 {
    let t = theta.to_radians();
    [-t.sin(), t.cos(), 0.0]
}

/// Outer radius of the bare section at `theta` where it crosses height `z`; `None` past its reach.
fn surface_r(d: &RingDesign, theta: f64, z: f64) -> Option<f64> {
    let reference = d.reference_loop();
    let l = d.section_at(theta, MAX_PROFILE_STEPS, None, Some(&reference));
    let pts: Vec<(f64, f64)> = l.pts.iter().filter(|p| p.surface).map(|p| (p.r, p.z)).collect();
    pts.windows(2)
        .filter_map(|w| {
            let ((r0, z0), (r1, z1)) = (w[0], w[1]);
            ((z0 - z) * (z1 - z) <= 0.0 && z0 != z1).then(|| r0 + (r1 - r0) * (z - z0) / (z1 - z0))
        })
        .reduce(f64::max)
}

fn crest_r(d: &RingDesign, theta: f64) -> f64 {
    surface_r(d, theta, 0.0).unwrap_or(0.0)
}

/// Signed distance from a plan point to the ellipse with semi-axes `a` and `b`, positive outside.
fn ellipse_distance(x: f64, y: f64, a: f64, b: f64) -> f64 {
    let (px, py) = (x.abs(), y.abs());
    let mut t = (py * a).atan2(px * b);
    for _ in 0..16 {
        let (s, c) = t.sin_cos();
        let (rx, ry) = (a * c - px, b * s - py);
        let (dx, dy) = (-a * s, b * c);
        let f = rx * dx + ry * dy;
        let df = dx * dx + dy * dy - rx * a * c - ry * b * s;
        if df.abs() < 1e-12 {
            break;
        }
        t = (t - f / df).clamp(0.0, 0.5 * PI);
    }
    let (s, c) = t.sin_cos();
    let d = (a * c - px).hypot(b * s - py);
    if (px / a).powi(2) + (py / b).powi(2) < 1.0 { -d } else { d }
}

/// The displaced surface: every atlas sample `keep` passes, moved out along its normal by the layer stack.
fn displaced(d: &RingDesign, lib: &AlphaLibrary, a: &Atlas, keep: impl Fn(&Sample) -> bool) -> Vec<P3> {
    let ctx = d.field_context();
    a.samples
        .iter()
        .filter(|s| keep(s))
        .map(|s| {
            let h = d.layers.height(
                Uv {
                    u: s.theta / 360.0 * ctx.circumference_mm,
                    v: s.v,
                },
                &ctx,
                lib,
            );
            add3(s.p, s.n, h)
        })
        .collect()
}

/// Where the ruby, its collet and the pocket under it stand, measured out along the ruby's own axis, mm.
#[derive(Clone, Copy, serde::Serialize)]
struct Head {
    /// The ruby's ring angle, its axis out of the table, and its width axis round the ring toward rising angle.
    theta: f64,
    n: P3,
    w: P3,
    crest_r: f64,
    girdle_r: f64,
    lip_r: f64,
    table_r: f64,
    /// The collet's outer wall in the ruby's plan: semi-axes round the ring and across it.
    wall: P2,
    girdle: P2,
    /// The ruby's placement over the seat dish's flat floor, which lies this far under the girdle's mid-plane.
    stand: f64,
    /// The burnished lip's height over the girdle.
    lip_mm: f64,
}

impl Head {
    /// A point's place in the ruby's plan: round the ring toward rising angle, and across the band.
    fn plan(&self, p: P3) -> P2 {
        [dot3(p, self.w), p[2]]
    }
    fn at(&self, along: f64, across: f64, out: f64) -> P3 {
        add3(add3(self.n.map(|v| v * out), self.w, along), [0.0, 0.0, 1.0], across)
    }
    /// Height over the girdle's mid-plane the band may reach at a point: the pavilion less the clearance, flat over the middle, free past the girdle.
    fn seat_floor(&self, q: P3, g: f64, pavilion: f64) -> f64 {
        let [x, z] = self.plan(q);
        let sc = (x / self.girdle[0]).hypot(z / self.girdle[1]);
        -(g + SEAT_CLEAR_MM) - (1.0 - sc.max(SEAT_FLAT)).max(0.0) * pavilion + 3.0 * smoothstep(1.05, 1.2, sc)
    }
    /// How far the bare surface at a sample must move in along its normal to reach the seat floor, mm.
    fn seat_carve(&self, s: &Sample, g: f64, pavilion: f64) -> f64 {
        let over = |c: f64| {
            let q = add3(s.p, s.n, -c);
            dot3(q, self.n) - self.girdle_r - self.seat_floor(q, g, pavilion)
        };
        if s.p[1] <= 0.0 || over(0.0) <= 0.0 {
            return 0.0;
        }
        if over(SEAT_MM) > 0.0 {
            return SEAT_MM;
        }
        let (mut lo, mut hi) = (0.0, SEAT_MM);
        for _ in 0..48 {
            let mid = 0.5 * (lo + hi);
            if over(mid) > 0.0 { lo = mid } else { hi = mid }
        }
        hi
    }
}

fn head(d: &RingDesign) -> Head {
    let gem = ruby();
    let crest = crest_r(d, STONE_DEG);
    let g = setting::girdle_half_mm(gem);
    let top = g + COLLET_LIP * gem.crown_mm();
    let lip_r = crest + LIP_OVER_CREST_MM;
    let girdle_r = lip_r - top;
    Head {
        theta: STONE_DEG,
        n: er(STONE_DEG),
        w: eth(STONE_DEG),
        crest_r: crest,
        girdle_r,
        lip_r,
        table_r: girdle_r + g + gem.crown_mm(),
        wall: [
            0.5 * gem.w_mm + COLLET_CLEAR_MM + COLLET_WALL_MM,
            0.5 * gem.l_mm + COLLET_CLEAR_MM + COLLET_WALL_MM,
        ],
        girdle: [0.5 * gem.w_mm, 0.5 * gem.l_mm],
        stand: g + SEAT_CLEAR_MM + (1.0 - SEAT_FLAT) * gem.pavilion_mm(),
        lip_mm: top - g,
    }
}

// --- The hide ------------------------------------------------------------------

/// Signed arc along the parting line at a ring angle, read off the hide between columns.
fn along_at(hide: &Hide, width: usize, theta: f64) -> f64 {
    let fx = theta.rem_euclid(360.0) / 360.0 * width as f64;
    let x0 = fx.floor() as usize % width;
    let x1 = (x0 + 1) % width;
    let f = fx - fx.floor();
    let (a, b) = (hide.along[x0], hide.along[x1]);
    if (a - b).abs() > 1.0 {
        if f < 0.5 { a } else { b }
    } else {
        a + (b - a) * f
    }
}

/// A sparse saw-tooth row along a keel, `n` to a plate: one sharp tooth in each pitch, rising toward the condyle, 0..1.
fn denticles(t: f64, n: f64) -> f64 {
    let phase = (t * n).fract();
    if !(0.45..0.84).contains(&phase) {
        0.0
    } else if phase < 0.74 {
        ((phase - 0.45) / 0.29).powf(1.3)
    } else {
        1.0 - (phase - 0.74) / 0.1
    }
}

/// A sample's place on its plate.
#[derive(Clone, Copy)]
struct OnPlate {
    /// Share along the plate from its anterior joint (0) to its posterior condyle (1).
    t: f64,
    /// Across as a share of the rim, signed.
    q: f64,
    /// Across as a share of the half-section from the crest line to the bore edge.
    u: f64,
    /// Across from the crest line, mm, signed.
    across: f64,
    len: f64,
    flank: f64,
    rim: f64,
}

/// A metasomal segment, barrel-shaped in plan, domed along the ring and rolled over its condyle: 0..1.
fn segment(p: OnPlate) -> f64 {
    let (t, q, len, flank) = (p.t, p.q, p.len, p.flank);
    let (x, back, qa) = (t * len, (1.0 - t) * len, q.abs());
    // A tucked ledge rising to a crest two thirds along, settling a little, then rolling over the condyle.
    let crest = 0.66 * len;
    let ledge = 0.3 * smoothstep(0.02, 0.32, x);
    let dome = 0.7 * (0.5 - 0.5 * (PI * (x / crest).min(1.0)).cos());
    let settle = 1.0 - 0.1 * smoothstep(crest, len - CONDYLE_MM, x);
    let roll = if back < CONDYLE_MM {
        (1.0 - (1.0 - back / CONDYLE_MM).powi(2)).max(0.0).sqrt()
    } else {
        1.0
    };
    let along = (ledge + dome) * settle * roll;
    // Rounded plan corners, and the margin rolled onto the flank.
    let near = x.min(back);
    let corner = if near < CORNER_MM {
        CORNER_MM - (CORNER_MM * CORNER_MM - (CORNER_MM - near).powi(2)).max(0.0).sqrt()
    } else {
        0.0
    };
    // The margin stands in from the rim at the anterior joint and has rolled over it onto the flank by 60% along.
    let full = PLATE_REACH * flank;
    let edge = full - (full - (1.0 - PLAN_TAPER) * p.rim) * (1.0 - smoothstep(0.0, 0.6, t)) - corner;
    let sleeve = 1.0 - smoothstep(edge - MARGIN_ROLL * flank, edge, p.across.abs());
    let across = 1.0 - 0.18 * q * q;
    let furrow = 1.0 - 0.1 * (1.0 - smoothstep(0.2, DORSAL_Q - 0.05, qa));
    // Two dorsal carinae: a continuous ridge with a sharp tooth each pitch, kept under the clamp.
    let teeth = (len / DENTICLE_PITCH_MM).round().max(3.0);
    let keels = (-((qa - DORSAL_Q) / 0.06).powi(2)).exp()
        * (KEEL_RIDGE + KEEL_TOOTH * denticles(t, teeth))
        * smoothstep(0.25, 0.7, x)
        * roll;
    ((PLATE_SHARE * along * across * furrow + keels) * sleeve).clamp(0.0, 1.0)
}

/// Two or three soft transverse folds of the pleural membrane per plate, in the strip above the bore edge: 0..1.
fn pleura(t: f64, u: f64, len: f64) -> f64 {
    let n = (len / 1.7).round().clamp(2.0, 3.0);
    let wrinkle = 0.5 - 0.5 * (2.0 * PI * n * t).cos();
    let strip = smoothstep(0.83, 0.89, u) * (1.0 - smoothstep(0.93, 0.99, u));
    let ends = smoothstep(0.1, 0.4, t * len) * smoothstep(0.1, 0.4, (1.0 - t) * len);
    wrinkle * strip * ends
}

/// What painting measured.
#[derive(Default, serde::Serialize)]
struct Hides {
    plates_per_shoulder: usize,
    plate_mm: Vec<f64>,
    neck_along_mm: f64,
    reach_mm: f64,
    rim_mm: Vec<f64>,
    ink: Vec<(String, f64)>,
}

/// A quill's footprint in its own plane, and the atlas column its centre stands on.
struct Ground {
    frame: csg::Frame,
    lo: P2,
    hi: P2,
    column: usize,
}

/// The footprint of every quill struck so far.
fn quill_ground(d: &RingDesign) -> Vec<Ground> {
    let ctx = d.field_context();
    d.stamps
        .iter()
        .filter(|s| s.name.starts_with("Quill"))
        .map(|s| {
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for p in &s.outline {
                lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                hi = [hi[0].max(p[0]), hi[1].max(p[1])];
            }
            Ground {
                frame: s.frame(d, &ctx),
                lo,
                hi,
                column: (s.theta_deg.rem_euclid(360.0) / 360.0 * AW as f64).round() as usize % AW,
            }
        })
        .collect()
}

fn paint(d: &mut RingDesign, lib: &mut AlphaLibrary, lat: &Lattice, hd: &Head, art: &Path, ground: &[Ground]) -> Result<Hides> {
    let a = Atlas::of(d, AW, AH)?;
    let hide = Hide::of(&a);
    let joints = Joints(
        lat.joints_deg
            .iter()
            .map(|t| along_at(&hide, a.width, *t).abs())
            .collect(),
    );
    ensure!(
        joints.0.windows(2).all(|w| w[1] > w[0]),
        "The joints do not run in order along the hide"
    );
    let plate = |s: &Sample| -> Option<OnPlate> {
        let p = hide.at(s);
        let (_, f, len) = joints.at(p.along.abs())?;
        let flank = (p.rim + p.wall).max(0.5);
        Some(OnPlate {
            t: 1.0 - f,
            q: p.across / p.rim.max(0.3),
            u: p.across.abs() / flank,
            across: p.across,
            len,
            flank,
            rim: p.rim.max(0.3),
        })
    };
    // Under every quill and QUILL_GROUND_MM round it the relief is read at the quill's own column: flat along the ring.
    let flat = |s: &Sample, f: &dyn Fn(&Sample) -> f64| -> f64 {
        let mut best: Option<(f64, usize)> = None;
        for g in ground {
            let d = sub3(s.p, g.frame.origin);
            if dot3(d, g.frame.z).abs() > 1.5 {
                continue;
            }
            let (lx, ly) = (dot3(d, g.frame.x), dot3(d, g.frame.y));
            let out = (g.lo[0] - lx).max(lx - g.hi[0]).max(0.0).hypot((g.lo[1] - ly).max(ly - g.hi[1]).max(0.0));
            let w = 1.0 - smoothstep(QUILL_GROUND_MM, 2.0 * QUILL_GROUND_MM, out);
            if w > 0.0 && best.is_none_or(|b| w > b.0) {
                best = Some((w, g.column));
            }
        }
        let v = f(s);
        match best {
            Some((w, column)) => v * (1.0 - w) + f(a.at(column, s.i / a.width)) * w,
            None => v,
        }
    };
    let tergites = a.paint("Manticora tergites", |s| flat(s, &|s| plate(s).map_or(0.0, segment)));
    let pleurae = a.paint("Manticora pleural folds", |s| {
        flat(s, &|s| plate(s).map_or(0.0, |p| pleura(p.t, p.u, p.len)))
    });
    let neck = joints.0[0];
    let articulations = a.paint("Manticora articulations", |s| {
        flat(s, &|s| {
            if s.p[1] > 0.0 && hide.at(s).along.abs() < neck {
                return 0.0;
            }
            let Some(p) = plate(s) else { return 0.0 };
            let (x, back) = (p.t * p.len, (1.0 - p.t) * p.len);
            let v = (1.0 - smoothstep(0.0, 0.4, x)).max(1.0 - smoothstep(0.0, 0.36, back));
            let palm = 1.0 - smoothstep(PALM_EASE_DEG.0, PALM_EASE_DEG.1, wrap_delta(s.theta - 270.0, 360.0).abs());
            v * v * (3.0 - 2.0 * v) * (1.0 - smoothstep(0.78, 0.9, p.u)) * (1.0 - (1.0 - PALM_CARVE) * palm)
        })
    });
    let (g, pavilion) = (setting::girdle_half_mm(ruby()), ruby().pavilion_mm());
    let seat = a.paint("Manticora seat", |s| hd.seat_carve(s, g, pavilion) / SEAT_MM);
    let graver = a.paint("Manticora graver lines", |s| {
        let Some(p) = plate(s) else { return 0.0 };
        let (x, back, qa) = (p.t * p.len, (1.0 - p.t) * p.len, p.q.abs());
        let run = smoothstep(0.35, 0.65, x) * smoothstep(CONDYLE_MM, CONDYLE_MM + 0.2, back);
        (1.0 - smoothstep(0.012, 0.024, (qa - DORSAL_Q).abs())) * run
    });
    let mut out = Hides {
        plates_per_shoulder: joints.plates(),
        plate_mm: joints.0.windows(2).map(|w| w[1] - w[0]).collect(),
        neck_along_mm: joints.0[0],
        reach_mm: hide.reach(),
        ..Hides::default()
    };
    for theta in [lat.joints_deg[0] + 4.0, 180.0, 250.0] {
        let x = (theta / 360.0 * AW as f64).round() as usize % AW;
        out.rim_mm.push(hide.rim[x][1]);
    }
    for (alpha, height, blend, bench) in [
        (tergites, TERGITE_MM, Blend::Max, false),
        (pleurae, PLEURA_MM, Blend::Max, false),
        (articulations, ARTICULATION_MM, Blend::Subtract, false),
        (seat, SEAT_MM, Blend::Subtract, false),
        (graver, GRAVER_MM, Blend::Subtract, true),
    ] {
        let name = alpha.name.clone();
        let png = alpha.to_png16()?;
        std::fs::write(art.join(format!("{}.png", name.to_lowercase().replace(' ', "-"))), &png)?;
        out.ink.push((
            name.clone(),
            alpha.data.iter().map(|v| *v as f64).sum::<f64>() / alpha.data.len() as f64,
        ));
        lib.insert(Alpha::from_png16(&name, &png)?);
        let mut e = skin::hide_layer(d, &name, height, Window::around(90.0, 360.0));
        e.name = name.trim_start_matches("Manticora ").to_string();
        e.name = e.name[..1].to_uppercase() + &e.name[1..];
        e.blend = blend;
        e.bench_only = bench;
        d.layers.layers.push(e);
    }
    Ok(out)
}

// --- The quills ------------------------------------------------------------------

#[derive(Default, serde::Serialize)]
struct Quills {
    count: usize,
    taper: f64,
    /// Worst offset of a quill from its plate's centre, as a share of that plate.
    worst_drift: f64,
    v_offset_mm: f64,
    /// Lengths of the three quills nearest the head, and the narrowest root of any quill, mm.
    head_lengths_mm: Vec<f64>,
    root_min_mm: f64,
    root_max_mm: f64,
}

fn outline_extent(o: &[[f64; 2]]) -> (f64, f64) {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in o {
        (x0, x1, y0, y1) = (x0.min(p[0]), x1.max(p[0]), y0.min(p[1]), y1.max(p[1]));
    }
    (x1 - x0, y1 - y0)
}

fn quill_rows(d: &mut RingDesign, lat: &Lattice) -> Result<Quills> {
    let ctx = d.field_context();
    let a = Atlas::of(d, AW, 256)?;
    let hide = Hide::of(&a);
    let centres = lat.plate_centres();
    ensure!(centres.len() >= 4, "Too few plates for the quill rows");
    let row_centres = &centres[..centres.len() - 1];
    // Chart v a share QUILL_Q of the rim out on the high side, read at the middle plate.
    let mid = row_centres[row_centres.len() / 2];
    let x = (mid / 360.0 * a.width as f64).round() as usize % a.width;
    let rim = hide.rim[x][1];
    let s = (0..a.height)
        .map(|y| a.at(x, y))
        .filter(|s| s.p[2] > 0.0)
        .min_by(|p, q| {
            (hide.at(p).across - QUILL_Q * rim)
                .abs()
                .total_cmp(&(hide.at(q).across - QUILL_Q * rim).abs())
        })
        .ok_or_else(|| anyhow::anyhow!("No flank sample"))?;
    let dv = s.v - ctx.crest_v_mm;
    // One keel from root to point: a ridge falling from the root's rise to the point's.
    let stamp = |name: &str, rot: f64| Stamp {
        name: name.into(),
        theta_deg: 0.0,
        v_mm: 0.0,
        rot_deg: rot,
        outline: outline::quill(QUILL_MM, QUILL_W_MM, 0.14),
        height_mm: 0.25,
        sink_mm: 0.35,
        draft_deg: 0.0,
        cut: false,
        bench: false,
        along_pull: false,
        fine_cap: false,
        tier: 0,
        top: StampTop::Ridge {
            rise_mm: 0.7,
            from: [-0.36 * QUILL_MM, 0.0],
            to: [0.44 * QUILL_MM, 0.0],
            end_mm: 0.08,
        },
    };
    let row = |taper: f64, name: &str, v: f64, rot: f64| StampRow {
        stamp: stamp(name, rot),
        path: RowPath::ChartV { v_mm: v },
        from_deg: row_centres[0],
        to_deg: *row_centres.last().unwrap(),
        count: row_centres.len() as u32,
        taper,
        fold_clear_mm: 0.0,
        mirror_shoulders: true,
    };
    let lengths: Vec<f64> = lat.joints_deg.windows(2).map(|w| w[1] - w[0]).collect();
    let strike = |taper: f64| -> Vec<Stamp> {
        let r = row(taper, "probe", ctx.crest_v_mm + dv, 0.0);
        let mut struck = setting::stamp_row(d, &StampRow { mirror_shoulders: false, ..r });
        struck.sort_by(|a, b| a.theta_deg.total_cmp(&b.theta_deg));
        struck
    };
    let (mut best, mut worst) = (None, f64::MAX);
    for i in 0..=60 {
        let t = i as f64 * 0.01;
        let struck = strike(t);
        if struck.iter().take(3).any(|s| outline_extent(&s.outline).0 < QUILL_HEAD_MM) {
            continue;
        }
        let w = struck
            .iter()
            .zip(row_centres)
            .zip(&lengths)
            .map(|((s, c), l)| (s.theta_deg - c).abs() / l)
            .fold(0.0, f64::max);
        if w < worst {
            (best, worst) = (Some(t), w);
        }
    }
    let best = best.ok_or_else(|| anyhow::anyhow!("No quill taper keeps the head quills {QUILL_HEAD_MM} mm long"))?;
    ensure!(worst <= 0.25, "The quill rows drift {worst:.2} of a plate off the plates");
    let probe = strike(best);
    let before = d.stamps.len();
    for (name, v, rot) in [
        ("Quill: high flank", ctx.crest_v_mm + dv, QUILL_SPLAY_DEG),
        ("Quill: low flank", ctx.crest_v_mm - dv, -QUILL_SPLAY_DEG),
    ] {
        d.stamps.extend(setting::stamp_row(d, &row(best, name, v, rot)));
    }
    let roots: Vec<f64> = d.stamps[before..].iter().map(|s| outline_extent(&s.outline).1).collect();
    Ok(Quills {
        count: d.stamps.len() - before,
        taper: best,
        worst_drift: worst,
        v_offset_mm: dv,
        head_lengths_mm: probe.iter().take(3).map(|s| outline_extent(&s.outline).0).collect(),
        root_min_mm: roots.iter().copied().fold(f64::MAX, f64::min),
        root_max_mm: roots.iter().copied().fold(0.0, f64::max),
    })
}

// --- The telson: knob and aculeus ------------------------------------------------------

/// Monotone cubic through `knots` at `x`, clamped to their range.
fn pchip(knots: &[(f64, f64)], x: f64) -> f64 {
    let n = knots.len();
    let x = x.clamp(knots[0].0, knots[n - 1].0);
    let h: Vec<f64> = (0..n - 1).map(|i| knots[i + 1].0 - knots[i].0).collect();
    let delta: Vec<f64> = (0..n - 1).map(|i| (knots[i + 1].1 - knots[i].1) / h[i]).collect();
    let mut m = vec![0.0; n];
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for i in 1..n - 1 {
        if delta[i - 1] * delta[i] > 0.0 {
            let (w1, w2) = (2.0 * h[i] + h[i - 1], h[i] + 2.0 * h[i - 1]);
            m[i] = (w1 + w2) / (w1 / delta[i - 1] + w2 / delta[i]);
        }
    }
    let i = (0..n - 1).find(|&i| x <= knots[i + 1].0).unwrap_or(n - 2);
    let t = (x - knots[i].0) / h[i];
    let (t2, t3) = (t * t, t * t * t);
    knots[i].1 * (2.0 * t3 - 3.0 * t2 + 1.0)
        + h[i] * m[i] * (t3 - 2.0 * t2 + t)
        + knots[i + 1].1 * (3.0 * t2 - 2.0 * t3)
        + h[i] * m[i + 1] * (t3 - t2)
}

/// The aculeus's centre line: a spiral in the hook's plane whose curvature climbs from `k0` to the point's.
#[derive(Clone, Copy, serde::Serialize)]
struct Spiral {
    length: f64,
    k0: f64,
    p: f64,
}

impl Spiral {
    fn kappa(&self, s: f64) -> f64 {
        self.k0 + (1.0 / TIP_BEND_MM - self.k0) * (s / self.length).clamp(0.0, 1.0).powf(self.p)
    }
    fn heading(&self, s: f64) -> f64 {
        let l = self.length;
        self.k0 * s + (1.0 / TIP_BEND_MM - self.k0) * l / (self.p + 1.0) * (s / l).clamp(0.0, 1.0).powf(self.p + 1.0)
    }
    /// The centre line in the hook's plane from the foot, every `length / n`.
    fn walk(&self, n: usize) -> Vec<P2> {
        let ds = self.length / n as f64;
        let dir = |s: f64| {
            let h = self.heading(s);
            [h.cos(), h.sin()]
        };
        let mut p = [0.0, 0.0];
        let mut out = Vec::with_capacity(n + 1);
        out.push(p);
        for i in 0..n {
            let s = i as f64 * ds;
            let (a, m, b) = (dir(s), dir(s + 0.5 * ds), dir(s + ds));
            p = [
                p[0] + ds / 6.0 * (a[0] + 4.0 * m[0] + b[0]),
                p[1] + ds / 6.0 * (a[1] + 4.0 * m[1] + b[1]),
            ];
            out.push(p);
        }
        out
    }
    fn tip(&self) -> [f64; 3] {
        let w = self.walk(1500);
        let t = w[w.len() - 1];
        [t[0], t[1], self.heading(self.length)]
    }
}

/// The spiral `length` long whose point lands on `target` in the hook's plane: its start curvature and exponent solved.
fn spiral_to(target: P2, length: f64) -> Option<Spiral> {
    let resid = |x: [f64; 2]| -> [f64; 2] {
        let t = Spiral { length, k0: x[0], p: x[1] }.tip();
        [t[0] - target[0], t[1] - target[1]]
    };
    let norm = |r: [f64; 2]| r[0].hypot(r[1]);
    let ok = |x: [f64; 2]| x[0] > 0.0 && x[0] < 1.0 / TIP_BEND_MM && x[1] > 0.3 && x[1] < 16.0;
    let mut x = [0.2, 3.0];
    let mut r = resid(x);
    for ki in 1..48 {
        for pi in 2..=60 {
            let y = [0.02 * ki as f64, 0.25 * pi as f64];
            if !ok(y) {
                continue;
            }
            let ry = resid(y);
            if norm(ry) < norm(r) {
                (x, r) = (y, ry);
            }
        }
    }
    for _ in 0..80 {
        if norm(r) < 1e-9 {
            break;
        }
        let mut j = [[0.0; 2]; 2];
        for c in 0..2 {
            let h = [1e-7, 1e-6][c];
            let mut y = x;
            y[c] += h;
            let ry = resid(y);
            j[0][c] = (ry[0] - r[0]) / h;
            j[1][c] = (ry[1] - r[1]) / h;
        }
        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
        if det.abs() < 1e-14 {
            break;
        }
        let dx = [(j[1][1] * r[0] - j[0][1] * r[1]) / det, (j[0][0] * r[1] - j[1][0] * r[0]) / det];
        let mut lam = 1.0;
        let mut moved = false;
        while lam > 1e-6 {
            let y = [x[0] - lam * dx[0], x[1] - lam * dx[1]];
            if ok(y) {
                let ry = resid(y);
                if norm(ry) < norm(r) {
                    (x, r, moved) = (y, ry, true);
                    break;
                }
            }
            lam *= 0.5;
        }
        if !moved {
            break;
        }
    }
    (norm(r) < 1e-6).then_some(Spiral { length, k0: x[0], p: x[1] })
}

/// Of the spirals 9.5-11 mm long landing the point on `target`, the one whose point heads nearest `TIP_HEADING_DEG`.
fn solve_spiral(target: P2) -> Result<Spiral> {
    (0..=15)
        .filter_map(|k| spiral_to(target, 9.5 + 0.1 * k as f64))
        .min_by(|a, b| {
            let off = |s: &Spiral| (s.heading(s.length).to_degrees() - TIP_HEADING_DEG).abs();
            off(a).total_cmp(&off(b))
        })
        .ok_or_else(|| anyhow::anyhow!("No spiral of 9.5-11 mm lands the aculeus's point at {target:?}"))
}

/// The aculeus as built: its mesh, its line, its sections and the frame its foot stands in.
struct Aculeus {
    solid: csg::Solid,
    spiral: Spiral,
    foot: P3,
    /// The foot's axis, and its section's axes: in the hook's plane toward the ruby, and across it.
    axis: P3,
    toward: P3,
    across: P3,
    /// Arc length, centre, and half-sizes in its bending plane and across it.
    stations: Vec<(f64, P3, f64, f64)>,
    /// Most the section's in-plane half-size takes of the local bend radius.
    fold_share: f64,
}

impl Aculeus {
    fn section_at(&self, s: f64) -> (f64, f64) {
        self.stations
            .iter()
            .min_by(|a, b| (a.0 - s).abs().total_cmp(&(b.0 - s).abs()))
            .map_or((0.0, 0.0), |t| (t.2, t.3))
    }
    fn from_point(&self, back: f64) -> (f64, f64) {
        self.section_at(self.spiral.length - back)
    }
    fn at_share(&self, share: f64) -> (f64, f64) {
        self.section_at(self.spiral.length * share)
    }
    /// Tightest bend radius over the last `span` mm.
    fn tail_bend_mm(&self, span: f64) -> f64 {
        (0..=100)
            .map(|i| 1.0 / self.spiral.kappa(self.spiral.length - span * i as f64 / 100.0))
            .fold(f64::MAX, f64::min)
    }
    /// The vesicle: its widest section, how far from the foot the width falls to 60% of it, and whether it only narrows past it.
    fn vesicle(&self) -> (f64, f64, bool) {
        let w: Vec<(f64, f64)> = self.stations.iter().map(|s| (s.0, 2.0 * s.2.max(s.3))).collect();
        let (ip, peak) = w.iter().enumerate().fold((0, 0.0), |m, (i, x)| if x.1 > m.1 { (i, x.1) } else { m });
        let end = w[ip..].iter().find(|x| x.1 <= 0.6 * peak).map_or(self.spiral.length, |x| x.0);
        (peak, end, w[ip..].windows(2).all(|p| p[1].1 <= p[0].1 + 1e-9))
    }
}

/// The aculeus: a vesicle swelling out of the knob's outline and tapering without a pinch into a slim barb, carried along a
/// spiral solved in the ring's mid-plane whose arch then turns about the chord from its foot to its point.
fn aculeus(foot: P3, axis: P3, toward: P3, target_w: P3) -> Result<Aculeus> {
    let np = unit3(cross3(axis, toward));
    let delta = sub3(target_w, foot);
    ensure!(dot3(delta, np).abs() < 1e-6, "The aculeus's point is off the ring's mid-plane");
    let spiral = solve_spiral([dot3(delta, axis), dot3(delta, toward)])?;
    let len = spiral.length;
    let chord = unit3(delta);
    let lean_at = |s: f64| STING_LEAN_DEG.to_radians() * smoothstep(STING_TWIST.0 * len, STING_TWIST.1 * len, s);
    let turn = |v: P3, lean: f64| -> P3 {
        let ((sl, cl), cv, d) = (lean.sin_cos(), cross3(chord, v), dot3(chord, v));
        std::array::from_fn(|k| v[k] * cl + cv[k] * sl + chord[k] * d * (1.0 - cl))
    };
    let n = 3000;
    let line = spiral.walk(n);
    let at = |s: f64| -> (P3, P3) {
        let f = (s / len * n as f64).clamp(0.0, n as f64);
        let i = (f.floor() as usize).min(n - 1);
        let w = f - i as f64;
        let q = [
            line[i][0] + (line[i + 1][0] - line[i][0]) * w,
            line[i][1] + (line[i + 1][1] - line[i][1]) * w,
        ];
        let h = spiral.heading(s);
        (
            add3(add3(foot, axis, q[0]), toward, q[1]),
            unit3(add3(axis.map(|v| v * h.cos()), toward, h.sin())),
        )
    };
    let sizes = |s: f64| -> (f64, f64) {
        let share = s / len;
        let (a, b) = (pchip(&ACULEUS_A, share), pchip(&ACULEUS_B, share));
        let w = smoothstep(len - 1.4, len - TIP_RADIUS_MM, s);
        (a * (1.0 - w) + TIP_RADIUS_MM * w, b * (1.0 - w) + TIP_RADIUS_MM * w)
    };
    // The rolled foot, as the knob's top is rolled; then the body; then the rounded point.
    let e = JOINT_ROLL_MM;
    let mut rows: Vec<(f64, f64)> = (0..=8)
        .map(|k| {
            let b = k as f64 / 8.0 * 0.5 * PI;
            (e * (1.0 - b.cos()), 1.0 - e * (1.0 - b.sin()) / KNOB_A_MM.min(KNOB_B_MM))
        })
        .collect();
    let body_end = len - TIP_RADIUS_MM;
    let steps = ((body_end - e) / ACULEUS_STEP_MM).ceil() as usize;
    rows.extend((1..=steps).map(|k| (e + (body_end - e) * k as f64 / steps as f64, 1.0)));
    rows.extend((1..10).map(|k| {
        let b = k as f64 / 10.0 * 0.5 * PI;
        (body_end + TIP_RADIUS_MM * b.sin(), b.cos())
    }));
    let m = ACULEUS_AROUND;
    let mut solid = csg::Solid::default();
    let mut stations = Vec::with_capacity(rows.len());
    let mut fold_share: f64 = 0.0;
    let place = |p: P3, lean: f64| add3(foot, turn(sub3(p, foot), lean), 1.0);
    for &(s, scale) in &rows {
        let (c, t) = at(s);
        let (a, b) = sizes(s);
        let (a, b) = (a * scale, b * scale);
        fold_share = fold_share.max(a * spiral.kappa(s));
        let n2 = unit3(cross3(np, t));
        // The belly swells to the bend's outside, away from the ruby.
        let belly = VESICLE_BELLY_MM * smoothstep(0.0, 0.2, s / len) * (1.0 - smoothstep(0.24, 0.5, s / len));
        let lean = lean_at(s);
        let (c, u, v) = (place(add3(c, n2, -belly), lean), turn(n2, lean), turn(np, lean));
        for j in 0..m {
            let al = 2.0 * PI * j as f64 / m as f64;
            solid.v.push(add3(add3(c, u, a * al.cos()), v, b * al.sin()));
        }
        stations.push((s, c, a, b));
    }
    let ring = |r: usize, k: usize| (r * m + k % m) as u32;
    for r in 0..rows.len() - 1 {
        for k in 0..m {
            solid.f.push([ring(r, k), ring(r + 1, k), ring(r + 1, k + 1)]);
            solid.f.push([ring(r, k), ring(r + 1, k + 1), ring(r, k + 1)]);
        }
    }
    let base = solid.v.len() as u32;
    solid.v.push(foot);
    let apex = solid.v.len() as u32;
    solid.v.push(place(at(len).0, lean_at(len)));
    let last = rows.len() - 1;
    for k in 0..m {
        solid.f.push([base, ring(0, k), ring(0, k + 1)]);
        solid.f.push([ring(last, k + 1), ring(last, k), apex]);
    }
    if solid.volume() < 0.0 {
        for f in &mut solid.f {
            f.swap(1, 2);
        }
    }
    ensure!(solid.open_edges() == (0, 0), "The aculeus does not close");
    ensure!(fold_share < 0.9, "The aculeus's section outruns its bend: {fold_share:.2}");
    ensure!(csg::self_crossings(&solid) == 0, "The aculeus crosses itself");
    Ok(Aculeus {
        solid,
        spiral,
        foot,
        axis,
        toward,
        across: np,
        stations,
        fold_share,
    })
}

/// What the knob measures.
#[derive(Default, serde::Serialize)]
struct KnobReport {
    top_r: f64,
    /// How far its axis leans off the radial, across the band, degrees.
    lean_deg: f64,
    /// Its top over the highest and the lowest point of the finished bulb under it, along its axis.
    over_bulb_min_mm: f64,
    over_bulb_max_mm: f64,
    /// Along the ring where it meets the bulb.
    footprint_mm: f64,
    collet_clear_mm: f64,
    neck_clear_mm: f64,
    groove_mm: f64,
    /// The section left at the solder groove, in the hook's plane and across it.
    min_section_mm: f64,
    min_section_across_mm: f64,
}

/// The knob's frame: its top's centre, its axis, and its plan's axes toward the ruby and across.
#[derive(Clone, Copy, serde::Serialize)]
struct KnobFrame {
    top: P3,
    axis: P3,
    toward: P3,
    across: P3,
}

/// The bulb round the knob: its top along the radial at `KNOB_DEG` before the hook turns it, and the finished surface there.
struct KnobSeat {
    top_r: f64,
    surface: Vec<P3>,
}

impl KnobSeat {
    /// Depths of the bulb under a knob's footprint below its top along its axis, within reach of the top.
    fn depths(&self, top: P3, x: P3, y: P3, z: P3) -> Vec<f64> {
        self.surface
            .iter()
            .filter_map(|p| {
                let q = sub3(*p, top);
                let (a, b, h) = (dot3(q, x), dot3(q, y), -dot3(q, z));
                ((a / (KNOB_A_MM + 0.15)).powi(2) + (b / (KNOB_B_MM + 0.15)).powi(2) <= 1.0 && h > -0.5 && h < 3.5).then_some(h)
            })
            .collect()
    }
    /// How far the knob under this aculeus's foot rises over the highest bulb point under it.
    fn rise(&self, ac: &Aculeus) -> f64 {
        self.depths(add3(ac.foot, ac.axis, -SOLDER_GAP_MM), ac.toward, ac.across, ac.axis)
            .into_iter()
            .fold(f64::MAX, f64::min)
    }
}

fn knob_seat(d: &RingDesign, lib: &AlphaLibrary) -> Result<KnobSeat> {
    let (axis, side) = (er(KNOB_DEG), eth(KNOB_DEG));
    let a = Atlas::of(d, AW, 384)?;
    let surface = displaced(d, lib, &a, |s| {
        s.p[1] > 0.0 && wrap_delta(s.theta - KNOB_DEG, 360.0).abs() < 16.0
    });
    let high = surface
        .iter()
        .filter(|p| (dot3(**p, side) / (KNOB_A_MM + 0.15)).powi(2) + (p[2] / (KNOB_B_MM + 0.15)).powi(2) <= 1.0)
        .map(|p| dot3(*p, axis))
        .fold(f64::MIN, f64::max);
    ensure!(high > f64::MIN, "No bulb under the knob");
    Ok(KnobSeat { top_r: high + KNOB_RISE_MM, surface })
}

/// The knob the aculeus stands on: an elliptical boss grown out of the bulb along the aculeus's own foot axis, its top
/// rolled like a condyle into the solder groove and its sides flaring into the bulb.
fn knob(ks: &KnobSeat, ac: &Aculeus, hd: &Head, neck_deg: f64) -> Result<(csg::Solid, KnobReport, KnobFrame)> {
    let (lx, ly, lz) = (ac.toward, ac.across, ac.axis);
    let top = add3(ac.foot, lz, -SOLDER_GAP_MM);
    let frame = KnobFrame { top, axis: lz, toward: lx, across: ly };
    let depths = ks.depths(top, lx, ly, lz);
    ensure!(!depths.is_empty(), "No bulb under the knob");
    let over_min = depths.iter().copied().fold(f64::MAX, f64::min);
    let over_max = depths.iter().copied().fold(f64::MIN, f64::max);
    let mean = depths.iter().sum::<f64>() / depths.len() as f64;
    let e = JOINT_ROLL_MM;
    let mut sec = vec![Station { s: 0.0, o: 0.0, z: 0.0 }];
    sec.extend((0..=6).map(|k| {
        let b = k as f64 / 6.0 * 0.5 * PI;
        Station { s: 1.0, o: -e * (1.0 - b.sin()), z: -e * (1.0 - b.cos()) }
    }));
    let flare = [(0.0, -0.4), (0.04, -0.6), (0.09, -0.8), (0.14, -1.0), (0.19, -1.25), (0.22, -1.55), (0.24, -2.0)];
    sec.extend(flare.iter().map(|&(o, z)| Station { s: 1.0, o, z }));
    let base = (over_max + 0.8).max(2.4);
    sec.push(Station { s: 1.0, o: 0.25, z: -base });
    sec.push(Station { s: 0.0, o: 0.0, z: -base });
    let local = setting::sweep(&Plan::superellipse(KNOB_A_MM, KNOB_B_MM, 2.0), &sec, 96);
    ensure!(dot3(cross3(lx, ly), lz) > 0.99, "The knob's frame is not right-handed");
    let world = |p: P3| add3(add3(add3(top, lx, p[0]), ly, p[1]), lz, p[2]);
    let solid = csg::Solid {
        v: local.v.iter().map(|p| world(*p)).collect(),
        f: local.f.clone(),
    };
    ensure!(solid.open_edges() == (0, 0), "The knob does not close");
    ensure!(csg::self_crossings(&solid) == 0, "The knob crosses itself");
    // Where the flare meets the bulb on average, and the outline there.
    let z_s = -mean;
    let o_s = flare
        .windows(2)
        .find(|w| z_s <= w[0].1 && z_s >= w[1].1)
        .map_or(0.24, |w| w[0].0 + (w[1].0 - w[0].0) * (z_s - w[0].1) / (w[1].1 - w[0].1));
    let outline = |grow: f64, z: f64| -> Vec<P3> {
        (0..128)
            .map(|k| {
                let f = 2.0 * PI * k as f64 / 128.0;
                world([(KNOB_A_MM + grow) * f.cos(), (KNOB_B_MM + grow) * f.sin(), z])
            })
            .collect()
    };
    let collet_clear = outline(0.0, -0.3)
        .iter()
        .map(|p| {
            let [x, z] = hd.plan(*p);
            ellipse_distance(x, z, hd.wall[0], hd.wall[1])
        })
        .fold(f64::MAX, f64::min);
    let far = outline(o_s, z_s)
        .iter()
        .map(|p| (p[1].atan2(p[0]).to_degrees(), p[0].hypot(p[1])))
        .fold((0.0_f64, 0.0_f64), |m, (t, r)| if t > m.0 { (t, r) } else { m });
    let shrink = 1.0 - e / KNOB_A_MM.min(KNOB_B_MM);
    Ok((
        solid,
        KnobReport {
            top_r: dot3(top, top).sqrt(),
            lean_deg: dot3(lz, er(KNOB_DEG)).clamp(-1.0, 1.0).acos().to_degrees(),
            over_bulb_min_mm: over_min,
            over_bulb_max_mm: over_max,
            footprint_mm: 2.0 * (KNOB_A_MM + o_s),
            collet_clear_mm: collet_clear,
            neck_clear_mm: (neck_deg - far.0).to_radians() * far.1,
            groove_mm: e,
            min_section_mm: 2.0 * KNOB_A_MM * shrink,
            min_section_across_mm: 2.0 * KNOB_B_MM * shrink,
        },
        frame,
    ))
}

/// What the calyx measures.
#[derive(Default, serde::Serialize)]
struct CalyxReport {
    /// Plain collet wall left between the calyx and the collet's top, mm.
    plain_wall_mm: f64,
    blend_mm: f64,
    /// The flare's run out from the wall over its first half millimetre down, degrees from vertical.
    flare_deg: f64,
    beads: usize,
    bead_mm: f64,
    /// Least clear gap between two beads of the collar, mm.
    bead_gap_mm: f64,
    slivers_cleaned: usize,
    triangles: usize,
}

/// A closed sphere of `r` round `c`, wound outward.
fn bead(c: P3, r: f64) -> csg::Solid {
    let (rings, around) = (8usize, 16usize);
    let mut s = csg::Solid::default();
    s.v.push([c[0], c[1], c[2] + r]);
    for i in 1..rings {
        let t = PI * i as f64 / rings as f64;
        for j in 0..around {
            let a = 2.0 * PI * j as f64 / around as f64;
            s.v.push([c[0] + r * t.sin() * a.cos(), c[1] + r * t.sin() * a.sin(), c[2] + r * t.cos()]);
        }
    }
    s.v.push([c[0], c[1], c[2] - r]);
    let last = (s.v.len() - 1) as u32;
    let ring = |i: usize, j: usize| (1 + (i - 1) * around + j % around) as u32;
    for j in 0..around {
        s.f.push([0, ring(1, j), ring(1, j + 1)]);
        s.f.push([last, ring(rings - 1, j + 1), ring(rings - 1, j)]);
    }
    for i in 1..rings - 1 {
        for j in 0..around {
            s.f.push([ring(i, j), ring(i + 1, j), ring(i + 1, j + 1)]);
            s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i, j + 1)]);
        }
    }
    s
}

/// The venom calyx: a flared skirt round the collet from just under its top down into the bulb, a bead collar round its upper run.
fn calyx(hd: &Head, ground: &[P3]) -> Result<(csg::Solid, CalyxReport)> {
    let gem = ruby();
    let plan = Plan::of(gem);
    let top = setting::girdle_half_mm(gem) + COLLET_LIP * gem.crown_mm();
    let zc = top - CALYX_UNDER_LIP_MM;
    // The section: offset out from the girdle's outline and height over its plane. The flare is a bell leaving the
    // wall at 29 degrees and steepening toward the bulb, so its foot stays inside the band's width.
    let bell = |depth: f64| 0.80 + 0.45 * (1.0 - (-depth / 0.8).exp());
    let flare: Vec<(f64, f64)> = [0.0, 0.1, 0.25, 0.45, 0.7, 1.0, 1.35, 1.75, 2.3].iter().map(|&h| (bell(h), -h)).collect();
    let mut sec: Vec<(f64, f64)> = vec![(0.6, top - 0.06), (0.74, top - 0.06), (0.74, zc + 0.08)];
    sec.extend(flare.iter().map(|&(o, dz)| (o, zc + dz)));
    sec.push((bell(2.3) - 0.05, -3.0));
    sec.push((0.6, -3.0));
    let n = 288;
    // Local x along the stone's length (across the band), y along its width (round the ring), z up its table.
    let (long, short, up) = ([0.0, 0.0, 1.0], hd.w.map(|v| -v), hd.n);
    ensure!(dot3(cross3(long, short), up) > 0.99, "The calyx's frame is not right-handed");
    let centre = hd.n.map(|v| v * hd.girdle_r);
    let mut solid = csg::Solid::default();
    for &(o, z) in &sec {
        for j in 0..n {
            let phi = 2.0 * PI * j as f64 / n as f64;
            let (p, nn) = (plan.point(phi), plan.normal(phi));
            // The flare reaches a fifth as far toward the knob.
            let off = if o > 0.8 { 0.8 + (o - 0.8) * (1.0 - 0.8 * phi.sin().min(0.0).powi(2)) } else { o };
            let (x, y) = (p[0] + nn[0] * off, p[1] + nn[1] * off);
            solid.v.push(add3(add3(add3(centre, long, x), short, y), up, z));
        }
    }
    let m = sec.len();
    let at = |i: usize, j: usize| ((i % m) * n + j % n) as u32;
    for i in 0..m {
        for j in 0..n {
            solid.f.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            solid.f.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    if solid.volume() < 0.0 {
        for f in &mut solid.f {
            f.swap(1, 2);
        }
    }
    ensure!(solid.open_edges() == (0, 0), "The calyx does not close");
    // The bead collar: sunk into the collet's wall just under its top, evenly round, parted round the telson root and
    // wherever the bulb comes near enough to crowd the calyx's seam bead.
    let r = 0.5 * COLLAR_BEAD_MM;
    let bead_at = |phi: f64| -> P3 {
        let (p, nn) = (plan.point(phi), plan.normal(phi));
        let o = COLLET_CLEAR_MM + COLLET_WALL_MM + r - COLLAR_SINK_MM;
        add3(add3(add3(centre, long, p[0] + nn[0] * o), short, p[1] + nn[1] * o), up, top - 0.02 - r)
    };
    let steps = 4096;
    let line: Vec<P3> = (0..=steps).map(|i| bead_at(2.0 * PI * i as f64 / steps as f64)).collect();
    let mut arc = vec![0.0];
    for w in line.windows(2) {
        arc.push(arc.last().copied().unwrap_or(0.0) + dot3(sub3(w[1], w[0]), sub3(w[1], w[0])).sqrt());
    }
    let point_at = |s: f64| {
        let i = arc.partition_point(|&a| a < s).clamp(1, steps);
        let t = (s - arc[i - 1]) / (arc[i] - arc[i - 1]).max(1e-12);
        add3(line[i - 1], sub3(line[i], line[i - 1]), t)
    };
    let knob = er(KNOB_DEG);
    let total = arc[steps];
    let count = (total / COLLAR_PITCH_MM).floor().max(1.0) as usize;
    let mut centres = Vec::new();
    for i in 0..count {
        let c = point_at(total * i as f64 / count as f64);
        let off_axis = sub3(c, knob.map(|v| v * dot3(c, knob)));
        let clear = ground.iter().map(|g| dot3(sub3(*g, c), sub3(*g, c))).fold(f64::MAX, f64::min).sqrt() - r;
        if dot3(off_axis, off_axis).sqrt() >= COLLAR_KNOB_GAP_MM && clear >= COLLAR_GROUND_MM {
            centres.push(c);
        }
    }
    let gap = centres
        .windows(2)
        .map(|w| dot3(sub3(w[1], w[0]), sub3(w[1], w[0])).sqrt() - COLLAR_BEAD_MM)
        .fold(f64::MAX, f64::min);
    let mut parts = vec![solid];
    parts.extend(centres.iter().map(|&c| bead(c, r)));
    let mut solid = csg::union_all(&parts).map_err(|e| anyhow::anyhow!("The bead collar does not join the calyx: {e:?}"))?;
    let slivers = csg::clean(&mut solid, 2e-5);
    ensure!(solid.open_edges() == (0, 0), "The calyx does not close");
    ensure!(csg::self_crossings(&solid) == 0, "The calyx crosses itself");
    let report = CalyxReport {
        plain_wall_mm: CALYX_UNDER_LIP_MM,
        blend_mm: CALYX_BLEND_MM,
        flare_deg: ((flare[3].0 - flare[0].0) / (flare[0].1 - flare[3].1)).atan().to_degrees(),
        beads: centres.len(),
        bead_mm: COLLAR_BEAD_MM,
        bead_gap_mm: gap,
        slivers_cleaned: slivers,
        triangles: solid.f.len(),
    };
    Ok((solid, report))
}

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe {
            kernel: "bestiarium_manticora".into(),
            op: op.into(),
            params,
            digest: String::new(),
        },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature {
        id,
        name: name.into(),
        enabled: true,
        operation,
        component,
    }
}

/// The CAD parts: the ruby in its venom collet, the seat bur, the knob grown out of the bulb and the separate aculeus on it.
fn parts(d: &mut RingDesign, lib: &AlphaLibrary, hd: &Head, neck_deg: f64) -> Result<(KnobReport, CalyxReport, Aculeus)> {
    let gem = ruby();
    let mut doc = Document::default();
    doc.append(feature(
        1,
        "Tail",
        Operation::Band,
        Component {
            role: ComponentRole::Shank,
            ..Component::default()
        },
    ))?;
    doc.append(builders::stone_feature(
        2,
        gem,
        Placement::Ring {
            theta_deg: STONE_DEG,
            across_mm: 0.0,
            height_mm: hd.stand,
            spin_deg: 90.0,
            tilt_deg: 0.0,
            cant_deg: 0.0,
        },
    ))?;
    doc.append(builders::feature_on(
        3,
        "Venom collet",
        builders::BEZEL,
        2,
        json!({"wall_mm": COLLET_WALL_MM, "lip": COLLET_LIP}),
    ))?;
    doc.append(builders::feature_on(4, "Seat bur", builders::BUR, 2, json!({"through": false})))?;
    let collet_ground = {
        let a = Atlas::of(d, AW, 384)?;
        displaced(d, lib, &a, |s| s.p[1] > 0.0 && wrap_delta(s.theta - STONE_DEG, 360.0).abs() < 24.0)
    };
    let (calyx_solid, cr) = calyx(hd, &collet_ground)?;
    doc.append(feature(
        5,
        "Venom calyx",
        stored_op(
            &calyx_solid,
            "calyx",
            json!({"under_lip_mm": CALYX_UNDER_LIP_MM, "beads": cr.beads, "bead_mm": COLLAR_BEAD_MM, "wall_mm": COLLET_WALL_MM}),
        )?,
        Component {
            attach: Attach::Join,
            stage: Stage::Cast,
            placement: Placement::Free,
            blend_mm: CALYX_BLEND_MM,
            ..Component::default()
        },
    ))?;
    let ks = knob_seat(d, lib)?;
    let (a0, t0) = (er(KNOB_DEG), eth(KNOB_DEG).map(|v| -v));
    let target = hd.at(TIP_ALONG_MM, 0.0, hd.table_r + TIP_OVER_TABLE_MM);
    let mut top_r = ks.top_r;
    let mut ac = aculeus(a0.map(|v| v * (top_r + SOLDER_GAP_MM)), a0, t0, target)?;
    // The foot rides up the radial until the knob stands its rise over the bulb along its own leaning axis.
    for _ in 0..4 {
        let short = KNOB_RISE_MM - ks.rise(&ac);
        if short.abs() < 0.005 {
            break;
        }
        top_r += short / dot3(a0, ac.axis).max(0.3);
        ac = aculeus(a0.map(|v| v * (top_r + SOLDER_GAP_MM)), a0, t0, target)?;
    }
    let (knob_solid, kr, kf) = knob(&ks, &ac, hd, neck_deg)?;
    doc.append(feature(
        6,
        "Aculeus knob",
        stored_op(
            &knob_solid,
            "knob",
            json!({"theta_deg": KNOB_DEG, "plan_mm": [2.0 * KNOB_A_MM, 2.0 * KNOB_B_MM], "frame": kf, "rise_mm": KNOB_RISE_MM, "roll_mm": JOINT_ROLL_MM}),
        )?,
        Component {
            attach: Attach::Join,
            stage: Stage::Cast,
            placement: Placement::Free,
            blend_mm: KNOB_BLEND_MM,
            ..Component::default()
        },
    ))?;
    doc.append(feature(
        7,
        "Aculeus",
        stored_op(
            &ac.solid,
            "aculeus",
            json!({"spiral": ac.spiral, "lean_deg": STING_LEAN_DEG, "belly_mm": VESICLE_BELLY_MM, "tip_bend_mm": TIP_BEND_MM, "tip_radius_mm": TIP_RADIUS_MM, "in_plane_half_mm": ACULEUS_A, "across_half_mm": ACULEUS_B, "foot_roll_mm": JOINT_ROLL_MM}),
        )?,
        Component {
            attach: Attach::Separate,
            stage: Stage::Cast,
            placement: Placement::Free,
            bench_notes: "Cast apart; solder into the knob's groove after the ruby is set and the collet burnished.".into(),
            ..Component::default()
        },
    ))?;
    doc.joints.push(Joint {
        a: 6,
        b: 7,
        clearance_mm: SOLDER_GAP_MM,
        method: "Solder after the ruby is set".into(),
        notes: "The aculeus is its own casting: set and burnish the ruby first, then solder the aculeus's rolled foot into the groove on the knob's rolled top, a segment joint like the tail's.".into(),
    });
    d.cad = Some(doc);
    Ok((kr, cr, ac))
}

// --- The ring ------------------------------------------------------------------

#[derive(serde::Serialize)]
struct Composition {
    hides: Hides,
    quills: Quills,
    head: Head,
    knob: KnobReport,
    calyx: CalyxReport,
    spiral: Spiral,
    aculeus_length_mm: f64,
    aculeus_fold_share: f64,
    spinel_stations: usize,
    spinel_taper: f64,
    neck_off_deg: f64,
}

fn author(art: &Path) -> Result<(RingDesign, AlphaLibrary, Composition, Aculeus, Vec<f64>)> {
    let lat = lattice(&band(30.0))?;
    let mut d = band(lat.neck_off());
    let hd = head(&d);
    let mut lib = AlphaLibrary::builtin();
    let quills = quill_rows(&mut d, &lat)?;
    let ground = quill_ground(&d);
    let hides = paint(&mut d, &mut lib, &lat, &hd, art, &ground)?;
    spinel_runs(&mut d, &lat);
    d.bake_all(&mut lib);
    let (kr, cr, ac) = parts(&mut d, &lib, &hd, lat.joints_deg[0])?;
    let comp = Composition {
        hides,
        quills,
        head: hd,
        knob: kr,
        calyx: cr,
        spiral: ac.spiral,
        aculeus_length_mm: ac.spiral.length,
        aculeus_fold_share: ac.fold_share,
        spinel_stations: lat.run.count as usize,
        spinel_taper: lat.run.taper,
        neck_off_deg: lat.neck_off(),
    };
    Ok((d, lib, comp, ac, lat.joints_deg.clone()))
}

// --- Gates, report and renders ----------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid {
        v: m.vertices
            .iter()
            .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
            .collect(),
        f: m.faces.clone(),
    }
}

/// The mesh's closed shells: faces grouped by the vertices they share.
fn shells(m: &mesh::Mesh) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..m.vertices.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for f in &m.faces {
        let a = root(&mut parent, f[0] as usize);
        for &j in &f[1..] {
            let b = root(&mut parent, j as usize);
            parent[b] = a;
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for (k, f) in m.faces.iter().enumerate() {
        groups.entry(root(&mut parent, f[0] as usize)).or_default().push(k);
    }
    groups.into_values().collect()
}

/// The faces `faces` of `m` as a mesh of their own.
fn submesh(m: &mesh::Mesh, faces: &[usize]) -> mesh::Mesh {
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for &k in faces {
        let g = m.faces[k].map(|i| {
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

/// Every CAD part's self-crossings, as placed.
fn made_parts(built: &mesh::BuildResult) -> Vec<(String, usize)> {
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

/// Every seat's made solids and every stamp's solid, checked closed and uncrossed.
fn made_solids(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    let unstruck = solid_of(&mesh::try_build(&setting::without_solids(d), lib, params)?.mesh);
    let ctx = d.field_context();
    for stamp in &d.stamps {
        let part = stamp
            .solid(&stamp.frame(d, &ctx), &unstruck)
            .map_err(anyhow::Error::msg)?;
        let c = part.check(true);
        out.push(json!({"name": stamp.name, "self_crossings": c.self_crossings, "zero_area_faces": c.zero_area_faces, "open_edges": c.open_edges, "repeated_edges": c.repeated_edges}));
    }
    for (stone, _) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let fit = setting::Fit {
            surface_z: stone.seat.height_mm - stand,
            through_mm: None,
            prongs: 0,
        };
        let parts = setting::parts(stone.gem, stone.seat.solid, fit).map_err(|e| anyhow::anyhow!("{e}"))?;
        for part in parts.add.iter().chain(&parts.cut) {
            let c = part.check(true);
            out.push(json!({"name": stone.label, "self_crossings": c.self_crossings, "zero_area_faces": c.zero_area_faces, "open_edges": c.open_edges, "repeated_edges": c.repeated_edges}));
        }
    }
    Ok(out)
}

fn sub3(a: P3, b: P3) -> P3 {
    std::array::from_fn(|i| a[i] - b[i])
}

/// Distance from `p` to the triangle `abc`.
fn point_triangle(p: P3, a: P3, b: P3, c: P3) -> f64 {
    let (ab, ac, ap) = (sub3(b, a), sub3(c, a), sub3(p, a));
    let (d1, d2) = (dot3(ab, ap), dot3(ac, ap));
    let closest = if d1 <= 0.0 && d2 <= 0.0 {
        a
    } else {
        let bp = sub3(p, b);
        let (d3, d4) = (dot3(ab, bp), dot3(ac, bp));
        let cp = sub3(p, c);
        let (d5, d6) = (dot3(ab, cp), dot3(ac, cp));
        let (vc, vb, va) = (d1 * d4 - d3 * d2, d5 * d2 - d1 * d6, d3 * d6 - d5 * d4);
        if d3 >= 0.0 && d4 <= d3 {
            b
        } else if d6 >= 0.0 && d5 <= d6 {
            c
        } else if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            let t = d1 / (d1 - d3);
            std::array::from_fn(|k| a[k] + ab[k] * t)
        } else if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            let t = d2 / (d2 - d6);
            std::array::from_fn(|k| a[k] + ac[k] * t)
        } else if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
            let t = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            std::array::from_fn(|k| b[k] + (c[k] - b[k]) * t)
        } else {
            let den = 1.0 / (va + vb + vc);
            let (v, w) = (vb * den, vc * den);
            std::array::from_fn(|k| a[k] + ab[k] * v + ac[k] * w)
        }
    };
    let d = sub3(p, closest);
    dot3(d, d).sqrt()
}

/// The nearest a set of points comes to a mesh's triangles, and the pair: a grid of cells `cell` mm.
fn nearest(points: &[P3], m: &mesh::Mesh, reach: f64) -> Option<(f64, P3, P3)> {
    let cell = reach.max(0.25);
    let key = |p: P3| p.map(|v| (v / cell).floor() as i64);
    let mut grid: std::collections::HashMap<[i64; 3], Vec<usize>> = Default::default();
    let tri = |k: usize| m.faces[k].map(|i| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    });
    for k in 0..m.faces.len() {
        let t = tri(k);
        let lo = std::array::from_fn::<f64, 3, _>(|j| t[0][j].min(t[1][j]).min(t[2][j]));
        let hi = std::array::from_fn::<f64, 3, _>(|j| t[0][j].max(t[1][j]).max(t[2][j]));
        let (a, b) = (key(lo), key(hi));
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    grid.entry([x, y, z]).or_default().push(k);
                }
            }
        }
    }
    let mut best: Option<(f64, P3, P3)> = None;
    for &p in points {
        let c = key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) else { continue };
                    for &k in list {
                        let [a, b, cc] = tri(k);
                        let dist = point_triangle(p, a, b, cc);
                        if best.is_none_or(|(d, ..)| dist < d) {
                            best = Some((dist, p, [(a[0] + b[0] + cc[0]) / 3.0, (a[1] + b[1] + cc[1]) / 3.0, (a[2] + b[2] + cc[2]) / 3.0]));
                        }
                    }
                }
            }
        }
    }
    best
}

/// The preview stones welded, with how many separate stones they make.
fn preview_stones(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> (Vec<(mesh::Mesh, [f32; 3])>, usize) {
    let mut count = 0;
    let groups = ringdesign_core::gems::built_meshes(d, lib, built)
        .into_iter()
        .map(|(m, tint)| {
            let mut out = mesh::Mesh::default();
            let mut index = std::collections::HashMap::new();
            for f in m.faces {
                let face = f.map(|i| {
                    let p = m.vertices[i as usize];
                    let key = [p.0, p.1, p.2].map(|x| (x * 1e5).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        out.vertices.push(p);
                        out.normals.push(m.normals[i as usize]);
                        (out.vertices.len() - 1) as u32
                    })
                });
                out.faces.push(face);
            }
            count += shells(&out).len();
            (out, tint)
        })
        .collect();
    (groups, count)
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

/// The camera for each named view: yaw about the head's axis, pitch toward the finger's.
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", -0.65, 0.6),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.75, 0.6),
    ("reverse", PI - 0.5, 0.35),
];

/// Studio-gold renders with stones set, a close-up of the head, and the bare tail against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])], neck_off: f64, edge: usize) -> Result<()> {
    let mut parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    parts.extend(gems.iter().map(|(m, tint)| render::Part::tinted_stone(m, *tint)));
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let head = crop(&built.mesh, [0.0, 13.0, 0.0], 12.0);
    let mut close = vec![render::Part::metal(&head, render::GOLD), render::Part::metal(&built.mesh, render::GOLD)];
    close.extend(gems.iter().map(|(m, tint)| render::Part::tinted_stone(m, *tint)));
    render::write_png_parts(out.join("stones.png"), &close, -0.45, 0.75, edge)?;
    let bare = mesh::try_build(&band(neck_off), lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    Ok(())
}

/// Every vertex of the finished mesh nearer the finger axis than the bore allows.
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

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

/// The faces of `m` within `radius` of `centre`, as a mesh of their own, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let faces: Vec<usize> = (0..m.faces.len()).filter(|k| m.faces[*k].iter().all(|&i| near(i))).collect();
    submesh(m, &faces)
}

/// A CAD part's placed mesh, by name.
fn component_mesh(built: &mesh::BuildResult, name: &str) -> Option<mesh::Mesh> {
    let c = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .find(|c| c.name == name)?;
    Some(mesh::Mesh {
        vertices: c
            .trace
            .positions
            .iter()
            .map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32))
            .collect(),
        faces: c.mesh.faces.clone(),
        ..mesh::Mesh::default()
    })
}

fn points_of(m: &mesh::Mesh) -> Vec<P3> {
    m.vertices
        .iter()
        .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64])
        .collect()
}

/// How close the aculeus comes to the collet, the ruby and the rest of the ring, and the solder gap at its foot.
#[derive(Default, serde::Serialize)]
struct StingClearance {
    to_collet_mm: f64,
    collet_pair: [P3; 2],
    to_ruby_mm: f64,
    ruby_pair: [P3; 2],
    /// To the ring beyond the joint (2.2 mm and more from the foot).
    to_ring_mm: f64,
    ring_pair: [P3; 2],
    /// Across the articulation groove: between 0.3 and 2.2 mm from the foot.
    groove_clear_mm: f64,
    foot_gap_mm: f64,
}

fn sting_clearance(sting: &mesh::Mesh, ring: &mesh::Mesh, collet: Option<&mesh::Mesh>, ruby: Option<&mesh::Mesh>, foot: P3) -> StingClearance {
    let pts = points_of(sting);
    let from_foot = |p: &P3| dot3(sub3(*p, foot), sub3(*p, foot)).sqrt();
    let pick = |f: &dyn Fn(f64) -> bool| -> Vec<P3> { pts.iter().copied().filter(|p| f(from_foot(p))).collect() };
    let mut out = StingClearance::default();
    let reach = 2.5;
    if let Some(c) = collet {
        let (d, a, b) = nearest(&pts, c, reach).unwrap_or((reach, [0.0; 3], [0.0; 3]));
        (out.to_collet_mm, out.collet_pair) = (d, [a, b]);
    }
    if let Some(r) = ruby {
        let (d, a, b) = nearest(&pts, r, reach).unwrap_or((reach, [0.0; 3], [0.0; 3]));
        (out.to_ruby_mm, out.ruby_pair) = (d, [a, b]);
    }
    let (d, a, b) = nearest(&pick(&|d| d > 2.2), ring, reach).unwrap_or((reach, [0.0; 3], [0.0; 3]));
    (out.to_ring_mm, out.ring_pair) = (d, [a, b]);
    out.groove_clear_mm = nearest(&pick(&|d| d > 0.3 && d <= 2.2), ring, 1.0).map_or(1.0, |t| t.0);
    out.foot_gap_mm = nearest(&pick(&|d| d <= 0.3), ring, 0.6).map_or(0.6, |t| t.0);
    out
}

/// How the ruby sits in its metal: the volume the two share, and how far metal reaches into the stone.
#[derive(Default, serde::Serialize)]
struct StoneSeat {
    shared_mm3: Option<f64>,
    metal_depth_mm: f64,
    metal_vertices_inside: usize,
    deepest_at: P3,
}

fn stone_seat(ruby: &mesh::Mesh, ring: &mesh::Mesh) -> StoneSeat {
    let stone = solid_of(ruby);
    let shared_mm3 = csg::combine(&stone, &solid_of(ring), csg::Op::Intersect).ok().map(|s| s.volume());
    let Some((lo, hi)) = stone.bounds() else { return StoneSeat::default() };
    let mut out = StoneSeat { shared_mm3, ..StoneSeat::default() };
    for p in points_of(ring) {
        if (0..3).any(|k| p[k] < lo[k] || p[k] > hi[k]) || csg::inside(&stone, p) != Some(true) {
            continue;
        }
        out.metal_vertices_inside += 1;
        let depth = stone.f.iter().map(|f| {
            let [a, b, c] = f.map(|i| stone.v[i as usize]);
            point_triangle(p, a, b, c)
        }).fold(f64::MAX, f64::min);
        if depth > out.metal_depth_mm {
            (out.metal_depth_mm, out.deepest_at) = (depth, p);
        }
    }
    out
}

/// Share of the ruby's girdle oval, seen from the face, that the aculeus covers.
fn face_coverage(sting: &mesh::Mesh, hd: &Head) -> f64 {
    let (a, b) = (hd.girdle[0], hd.girdle[1]);
    let step = 0.01;
    let (nx, nz) = ((2.0 * a / step) as usize + 1, (2.0 * b / step) as usize + 1);
    let mut covered = vec![false; nx * nz];
    for f in &sting.faces {
        let q = f.map(|i| {
            let v = sting.vertices[i as usize];
            hd.plan([v.0 as f64, v.1 as f64, v.2 as f64])
        });
        let (x0, x1) = (q.iter().map(|p| p[0]).fold(f64::MAX, f64::min), q.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
        let (z0, z1) = (q.iter().map(|p| p[1]).fold(f64::MAX, f64::min), q.iter().map(|p| p[1]).fold(f64::MIN, f64::max));
        if x1 < -a || x0 > a || z1 < -b || z0 > b {
            continue;
        }
        let area = (q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (q[1][1] - q[0][1]);
        if area.abs() < 1e-12 {
            continue;
        }
        let i0 = (((x0 + a) / step).floor().max(0.0)) as usize;
        let i1 = ((((x1 + a) / step).ceil()) as usize).min(nx - 1);
        let k0 = (((z0 + b) / step).floor().max(0.0)) as usize;
        let k1 = ((((z1 + b) / step).ceil()) as usize).min(nz - 1);
        for i in i0..=i1 {
            for k in k0..=k1 {
                let (x, z) = (-a + i as f64 * step, -b + k as f64 * step);
                let w0 = ((q[1][0] - x) * (q[2][1] - z) - (q[2][0] - x) * (q[1][1] - z)) / area;
                let w1 = ((q[2][0] - x) * (q[0][1] - z) - (q[0][0] - x) * (q[2][1] - z)) / area;
                if w0 >= 0.0 && w1 >= 0.0 && w0 + w1 <= 1.0 {
                    covered[i * nz + k] = true;
                }
            }
        }
    }
    let (mut inside, mut hit) = (0usize, 0usize);
    for i in 0..nx {
        for k in 0..nz {
            let (x, z) = (-a + i as f64 * step, -b + k as f64 * step);
            if (x / a).powi(2) + (z / b).powi(2) <= 1.0 {
                inside += 1;
                hit += usize::from(covered[i * nz + k]);
            }
        }
    }
    hit as f64 / inside.max(1) as f64
}

/// The finished outline's crest radius every half degree down the west shoulder, and each joint's dip under its neighbours' crests.
/// The outer silhouette's radius every half degree from the west neck round the palm to the east neck, and the dip at
/// every joint of both shoulders.
fn crest_table(ring: &mesh::Mesh, west: &[f64]) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let (lo, hi) = (west[0], 540.0 - west[0]);
    let bins = ((hi - lo) / 0.5).round() as usize + 1;
    let mut best = vec![0.0_f64; bins];
    for v in &ring.vertices {
        let mut t = (v.1 as f64).atan2(v.0 as f64).to_degrees().rem_euclid(360.0);
        if t < lo - 0.25 {
            t += 360.0;
        }
        if t > hi + 0.25 {
            continue;
        }
        let i = (((t - lo) / 0.5).round() as isize).clamp(0, bins as isize - 1) as usize;
        best[i] = best[i].max((v.0 as f64).hypot(v.1 as f64));
    }
    let table: Vec<[f64; 2]> = best.iter().enumerate().map(|(i, r)| [lo + 0.5 * i as f64, *r]).collect();
    let mut joints = west.to_vec();
    joints.extend(west.iter().rev().filter(|j| (**j - 270.0).abs() > 1e-6).map(|j| 540.0 - j));
    let span = |a: f64, b: f64, f: fn(f64, f64) -> f64, init: f64| {
        table.iter().filter(|p| p[0] >= a && p[0] <= b).map(|p| p[1]).fold(init, f)
    };
    let dips = joints
        .iter()
        .enumerate()
        .map(|(k, &j)| {
            let low = span(j - 0.75, j + 0.75, f64::min, f64::MAX);
            let before = (k > 0).then(|| span(joints[k - 1], j, f64::max, 0.0));
            let after = (k + 1 < joints.len()).then(|| span(j, joints[k + 1], f64::max, 0.0));
            let crest = match (before, after) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => low,
            };
            [j.rem_euclid(360.0), crest - low]
        })
        .collect();
    (table.iter().map(|p| [p[0].rem_euclid(360.0), p[1]]).collect(), dips)
}

/// The section through the finger axis at `theta`: every shell cut, the aculeus's line projected and dashed, and the marked gaps.
fn section_svg(
    path: &Path,
    theta: f64,
    title: &str,
    shells: &[(&mesh::Mesh, &str)],
    line: &[(P3, f64)],
    marks: &[(P3, P3, f64)],
) -> Result<()> {
    let (erv, en) = (er(theta), eth(theta));
    let (r0, r1, z0, z1) = (10.0, 19.0, -5.5, 5.5);
    let scale = 70.0;
    let map = |p: P3| ((dot3(p, erv) - r0) * scale, (z1 - p[2]) * scale);
    let mut body = String::new();
    for (m, colour) in shells {
        let mut d = String::new();
        for f in &m.faces {
            let q = f.map(|i| {
                let v = m.vertices[i as usize];
                [v.0 as f64, v.1 as f64, v.2 as f64]
            });
            let s = q.map(|p| dot3(p, en));
            let mut cut = Vec::new();
            for k in 0..3 {
                let (a, b) = (k, (k + 1) % 3);
                if (s[a] > 0.0) != (s[b] > 0.0) {
                    let u = s[a] / (s[a] - s[b]);
                    cut.push(std::array::from_fn::<f64, 3, _>(|j| q[a][j] + (q[b][j] - q[a][j]) * u));
                }
            }
            if cut.len() == 2 && dot3(cut[0], erv) > 0.0 {
                let ((x0, y0), (x1, y1)) = (map(cut[0]), map(cut[1]));
                d.push_str(&format!("M{x0:.2} {y0:.2}L{x1:.2} {y1:.2}"));
            }
        }
        body.push_str(&format!("<path d=\"{d}\" stroke=\"{colour}\" stroke-width=\"1.4\" fill=\"none\"/>\n"));
    }
    if !line.is_empty() {
        let pts: Vec<String> = line.iter().map(|(p, _)| {
            let (x, y) = map(*p);
            format!("{x:.1},{y:.1}")
        }).collect();
        body.push_str(&format!("<polyline points=\"{}\" stroke=\"#bbbbbb\" stroke-dasharray=\"6 5\" stroke-width=\"1.2\" fill=\"none\"/>\n", pts.join(" ")));
        for (p, r) in line.iter().step_by(12) {
            let (x, y) = map(*p);
            body.push_str(&format!("<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"{:.1}\" stroke=\"#888888\" stroke-dasharray=\"3 4\" stroke-width=\"0.8\" fill=\"none\"/>\n", r * scale));
        }
    }
    for (k, (a, b, gap)) in marks.iter().enumerate() {
        let ((x0, y0), (x1, y1)) = (map(*a), map(*b));
        body.push_str(&format!("<line x1=\"{x0:.1}\" y1=\"{y0:.1}\" x2=\"{x1:.1}\" y2=\"{y1:.1}\" stroke=\"#e0453a\" stroke-width=\"2\"/><circle cx=\"{x0:.1}\" cy=\"{y0:.1}\" r=\"5\" stroke=\"#e0453a\" stroke-width=\"2\" fill=\"none\"/><text x=\"{:.1}\" y=\"{:.1}\" fill=\"#e0453a\" font-family=\"sans-serif\" font-size=\"18\">{gap:.2} mm</text>\n", x0 + 10.0, y0 - 8.0 + 26.0 * k as f64));
    }
    let (w, h) = ((r1 - r0) * scale, (z1 - z0) * scale);
    std::fs::write(
        path,
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\"><rect width=\"100%\" height=\"100%\" fill=\"#111\"/><text x=\"14\" y=\"28\" fill=\"#ddd\" font-family=\"sans-serif\" font-size=\"18\">{title}</text><text x=\"14\" y=\"50\" fill=\"#999\" font-family=\"sans-serif\" font-size=\"14\">ring gold, aculeus white, ruby red; radius to the right</text><text x=\"14\" y=\"70\" fill=\"#999\" font-family=\"sans-serif\" font-size=\"14\">dashed: the aculeus's line and sections, projected</text>\n{body}</svg>"),
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/manticora"));
    let art = out.join("artwork");
    let _ = std::fs::remove_dir_all(&art);
    std::fs::create_dir_all(&art)?;
    println!("Manticora");
    let started = std::time::Instant::now();
    let (d, lib, comp, ac, joints) = author(&art)?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  aculeus heads {:.1} deg at its point",
        ac.spiral.heading(ac.spiral.length).to_degrees()
    );
    println!(
        "  knob: top r {:.3}, {:.2}-{:.2} mm over the bulb, footprint {:.2} mm, {:.2} mm off the collet wall, {:.2} mm off the neck; aculeus {:.2} mm, spiral k0 {:.4} p {:.3}, fold share {:.2}",
        comp.knob.top_r,
        comp.knob.over_bulb_min_mm,
        comp.knob.over_bulb_max_mm,
        comp.knob.footprint_mm,
        comp.knob.collet_clear_mm,
        comp.knob.neck_clear_mm,
        ac.spiral.length,
        ac.spiral.k0,
        ac.spiral.p,
        ac.fold_share
    );
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let quality = built.report.quality;
    println!(
        "  {} triangles in {build_s:.1} s (authored in {author_s:.1} s); watertight {watertight}; degenerate {degenerate}; self-crossings {crossings}; stamps {}/{}; seats {}; notes {:?} {:?}",
        built.mesh.faces.len(),
        built.solids.stamped,
        d.stamps.len(),
        built.solids.resolved,
        built.solids.notes,
        built.parts.notes
    );
    let made = made_parts(&built);
    let solids = made_solids(&d, &lib, params)?;
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let (gems, previewed) = preview_stones(&d, &lib, &built);
    let ruby_mesh = gems.iter().find(|(_, t)| t[0] > 0.1).map(|(m, _)| m);
    let groups = shells(&built.mesh);
    let far = |g: &Vec<usize>| {
        g.iter()
            .flat_map(|k| built.mesh.faces[*k])
            .map(|i| {
                let v = built.mesh.vertices[i as usize];
                (v.0 as f64).hypot(v.1 as f64)
            })
            .fold(0.0, f64::max)
    };
    ensure!(groups.len() == 2, "Expected the ring and the aculeus as two shells, found {}", groups.len());
    let (sting_i, ring_i) = if far(&groups[0]) > far(&groups[1]) { (0, 1) } else { (1, 0) };
    let sting_shell = submesh(&built.mesh, &groups[sting_i]);
    let ring_shell = submesh(&built.mesh, &groups[ring_i]);
    let collet = component_mesh(&built, "Venom collet");
    let clearance = sting_clearance(&sting_shell, &ring_shell, collet.as_ref(), ruby_mesh, ac.foot);
    let coverage = face_coverage(&sting_shell, &comp.head);
    let seat = ruby_mesh.map(|r| stone_seat(r, &ring_shell)).unwrap_or_default();
    println!("  ruby seat: shared {:?} mm3, metal {:.3} mm into the stone at {:?} ({} vertices)", seat.shared_mm3, seat.metal_depth_mm, seat.deepest_at.map(|v| (v * 1000.0).round() / 1000.0), seat.metal_vertices_inside);
    let (crests, dips) = crest_table(&ring_shell, &joints);
    let least_dip = dips.iter().map(|d| d[1]).fold(f64::MAX, f64::min);
    let line: Vec<(P3, f64)> = ac.stations.iter().step_by(4).map(|s| (s.1, s.2.max(s.3))).collect();
    let mut section_shells = vec![(&ring_shell, "#d9b76a"), (&sting_shell, "#f4f4f4")];
    if let Some(r) = ruby_mesh {
        section_shells.push((r, "#e0453a"));
    }
    let closest = if clearance.to_collet_mm <= clearance.to_ruby_mm { clearance.collet_pair } else { clearance.ruby_pair };
    let closest_theta = closest[0][1].atan2(closest[0][0]).to_degrees();
    section_svg(
        &out.join("stinger-section.svg"),
        closest_theta,
        &format!("Section at {closest_theta:.1} deg, the aculeus's closest approach"),
        &section_shells,
        &line,
        &[(clearance.collet_pair[0], clearance.collet_pair[1], clearance.to_collet_mm), (clearance.ruby_pair[0], clearance.ruby_pair[1], clearance.to_ruby_mm)],
    )?;
    section_svg(
        &out.join("knob-section.svg"),
        KNOB_DEG,
        &format!("Section at {KNOB_DEG:.1} deg through the knob, the solder groove and the aculeus's foot"),
        &section_shells,
        &line,
        &[],
    )?;
    section_svg(
        &out.join("stinger-section-90.svg"),
        90.0,
        "Section at 90.0 deg through the ruby, the collet's lip and the barb",
        &section_shells,
        &line,
        &[(clearance.collet_pair[0], clearance.collet_pair[1], clearance.to_collet_mm), (clearance.ruby_pair[0], clearance.ruby_pair[1], clearance.to_ruby_mm)],
    )?;
    let (w25, w60) = (ac.at_share(0.25), ac.at_share(0.60));
    let ratio = w25.0 / w60.0.max(1e-9);
    let over_ruby: Vec<&(f64, P3, f64, f64)> = ac
        .stations
        .iter()
        .filter(|s| {
            let [x, z] = comp.head.plan(s.1);
            (x / comp.head.girdle[0]).powi(2) + (z / comp.head.girdle[1]).powi(2) <= 1.0
        })
        .collect();
    let over_ruby_width = over_ruby.iter().map(|s| 2.0 * s.3).fold(0.0, f64::max);
    let over_ruby_from = over_ruby.iter().map(|s| s.0 / ac.spiral.length).fold(1.0, f64::min);
    let vesicle = ac.vesicle();
    // The centre line seen from the face: how far it bows off the chord from its foot to its point.
    let seen: Vec<P2> = ac.stations.iter().map(|s| [s.1[0], s.1[2]]).collect();
    let (f0, f1) = (seen[0], seen[seen.len() - 1]);
    let (cx, cz) = (f1[0] - f0[0], f1[1] - f0[1]);
    let cl = cx.hypot(cz).max(1e-9);
    let sagitta = seen.iter().map(|q| ((q[0] - f0[0]) * cz - (q[1] - f0[1]) * cx).abs() / cl).fold(0.0, f64::max);
    let point = ac.stations[ac.stations.len() - 1].1;
    let point_off = { let [x, z] = comp.head.plan(point); x.hypot(z) };
    let point_up = dot3(point, comp.head.n) - comp.head.table_r;
    println!(
        "  aculeus: {:.3} mm off the collet, {:.3} off the ruby, {:.3} off the ring beyond the joint, groove {:.3}, foot gap {:.3}; covers {:.1}% of the ruby; 25%/60% width {:.2}/{:.2} = {ratio:.2}; widest over the ruby {over_ruby_width:.2} from {over_ruby_from:.2} of its length; vesicle {:.2} wide, {:.2} long, aspect {:.2}, narrowing only {}; face sagitta {sagitta:.2}; point {point_off:.2} off the ruby's centre, {point_up:.2} over its table; last 3 mm bend {:.2} mm",
        clearance.to_collet_mm,
        clearance.to_ruby_mm,
        clearance.to_ring_mm,
        clearance.groove_clear_mm,
        clearance.foot_gap_mm,
        100.0 * coverage,
        2.0 * w25.0,
        2.0 * w60.0,
        vesicle.0,
        vesicle.1,
        vesicle.1 / vesicle.0.max(1e-9),
        vesicle.2,
        ac.tail_bend_mm(3.0)
    );
    let band_field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let warnings: Vec<String> = stones
        .iter()
        .flat_map(|r| r.seats.iter().flat_map(|s| s.warnings.iter().map(|w| format!("{}: {w}", s.label))))
        .collect();
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    // The lands the investment has to fill, against its floors, with the named exceptions.
    let (a5, a2) = (ac.from_point(5.0), ac.from_point(2.0));
    let foot = ac.section_at(0.0);
    let lands = json!({
        "floor_mm": MIN_SECTION_MM,
        "detail_floor_mm": MIN_DETAIL_MM,
        "collet_wall_mm": COLLET_WALL_MM,
        "collet_wall_buried_foot_mm": 0.8 * COLLET_WALL_MM,
        "collet_lip": {"height_mm": comp.head.lip_mm, "exception": "the burnished bezel lip: pushed over the ruby's crown at the bench, thinner than the fill floor by design"},
        "knob_groove_mm": [comp.knob.min_section_mm, comp.knob.min_section_across_mm],
        "aculeus_foot_mm": [2.0 * foot.0, 2.0 * foot.1],
        "aculeus_5mm_from_point_mm": [2.0 * a5.0, 2.0 * a5.1],
        "aculeus_2mm_from_point_mm": [2.0 * a2.0, 2.0 * a2.1],
        "aculeus_point": {"diameter_mm": 2.0 * TIP_RADIUS_MM, "exception": "the rounded point of the separately cast aculeus, over the detail floor"},
        "quill_root_mm": [comp.quills.root_min_mm, comp.quills.root_max_mm],
        "quill_note": "relief stamps on the band, judged at the detail floor",
        "collar_bead_mm": comp.calyx.bead_mm,
        "collar_gap_mm": comp.calyx.bead_gap_mm,
        "collar_note": "relief beads sunk into the calyx, judged at the detail floor",
    });
    let lands_ok = COLLET_WALL_MM >= MIN_SECTION_MM
        && comp.knob.min_section_mm >= 1.6
        && comp.knob.min_section_across_mm >= 1.6
        && 2.0 * foot.0.min(foot.1) >= 1.6
        && 2.0 * a5.0.min(a5.1) >= MIN_SECTION_MM
        && 2.0 * a2.0.min(a2.1) >= MIN_SECTION_MM
        && 2.0 * foot.0.min(foot.1) >= MIN_SECTION_MM
        && 2.0 * TIP_RADIUS_MM >= MIN_DETAIL_MM
        && comp.quills.root_min_mm >= MIN_DETAIL_MM
        && comp.calyx.bead_mm >= MIN_DETAIL_MM
        && comp.calyx.bead_gap_mm >= MIN_DETAIL_MM;
    // Stamps at the coarse pitch: phantoms move with resolution, real faults converge.
    let coarse_params = BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() };
    let coarse = mesh::try_build(&d, &lib, coarse_params)?;
    let (cw, cd, cx) = geometry(&coarse.mesh);
    let coarse_ok = cw && cd == 0 && cx == 0 && coarse.solids.stamped == d.stamps.len() && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}, stamps {}/{}; notes {:?} {:?}", coarse.solids.stamped, d.stamps.len(), coarse.solids.notes, coarse.parts.notes);
    // The investment pattern: the ring's own casting; the aculeus pours apart.
    let mut setup = mf::Setup::from_design(&d);
    setup.recipe.name = "Manticora / investment / Gold 18k".into();
    setup.recipe.alloy = "Gold 18k".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(1.3, |m| m.shrink_pct);
    setup.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    setup.bench_notes = "Invest the tail with its collet, knob, seats and quills. Cast the aculeus apart. Bead-set the spinels, set and burnish the ruby, then solder the aculeus into the knob's groove.".into();
    let prepared = mf::prepare(&d, &lib, &setup, params)?;
    let (pw, pd, px) = geometry(&prepared.mesh);
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let design_format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
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
    let spinels = d.layers.layers.iter().filter(|e| matches!(e.layer, Layer::SeatRun(_))).count();
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part, seat solid and stamp closed without crossings", made.iter().all(|(_, n)| *n == 0) && solids.iter().all(|g| g["self_crossings"] == 0 && g["open_edges"] == 0 && g["repeated_edges"] == 0 && g["zero_area_faces"] == 0)),
        ("solids and parts notes empty, every stamp and seat resolved", built.solids.notes.is_empty() && built.parts.notes.is_empty() && built.solids.stamped == d.stamps.len() && spinels == 2),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax verdict Castable with the 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == castability::Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("CAD lands at or above the floor, or excepted", lands_ok),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview, no warnings", reported == previewed && warnings.is_empty()),
        ("aculeus clears the collet and the ruby by 1.0 mm", clearance.to_collet_mm >= 1.0 && clearance.to_ruby_mm >= 1.0),
        ("the ruby seats: metal reaches at most 0.05 mm into it", ruby_mesh.is_some() && seat.metal_depth_mm <= 0.05),
        ("investment pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("gates hold at 384 x 192", coarse_ok),
        ("export build within the 2 million triangle budget", built.mesh.faces.len() <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "alloy_for_weight": "Gold 18k",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "build_s": build_s, "author_s": author_s},
        "geometry": {"watertight": watertight, "boundary_edges": built.report.validation.boundary_edges, "non_manifold_edges": built.report.validation.non_manifold_edges, "degenerate_faces": degenerate, "min_angle_deg": quality.min_angle_deg, "worst_aspect": quality.worst_aspect, "self_crossings": crossings, "shells": groups.len(), "volume_mm3": built.report.volume_mm3},
        "made_parts": made,
        "made_solids": solids,
        "solids": {"resolved": built.solids.resolved, "stamped": built.solids.stamped, "stamps": d.stamps.len(), "notes": built.solids.notes, "parts_notes": built.parts.notes, "parts_separate": built.parts.separate, "parts_joined": built.parts.joined, "parts_cut": built.parts.cut},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "land_widths": lands,
        "aculeus": {
            "clearance": clearance,
            "face_coverage_of_ruby": coverage,
            "width_at_25_percent_mm": 2.0 * w25.0,
            "width_at_60_percent_mm": 2.0 * w60.0,
            "width_ratio_25_to_60": ratio,
            "peak_width_mm": vesicle.0,
            "vesicle_length_mm": vesicle.1,
            "vesicle_aspect": vesicle.1 / vesicle.0.max(1e-9),
            "width_monotone_after_peak": vesicle.2,
            "face_sagitta_mm": sagitta,
            "point_from_ruby_centre_mm": point_off,
            "point_over_table_mm": point_up,
            "over_ruby_from_share": over_ruby_from,
            "twist_shares": STING_TWIST,
            "widest_over_ruby_mm": over_ruby_width,
            "tightest_bend_last_3mm": ac.tail_bend_mm(3.0),
            "point_diameter_mm": 2.0 * TIP_RADIUS_MM,
            "length_mm": ac.spiral.length,
            "spiral": ac.spiral,
            "fold_share": ac.fold_share,
            "lean_deg": STING_LEAN_DEG,
            "sections": ["stinger-section.svg", "stinger-section-90.svg", "knob-section.svg"],
        },
        "knob": comp.knob,
        "ruby_seat": seat,
        "undercut": {
            "band_percent": band_field.undercut_fraction() * 100.0,
            "band_worst_draft_deg": band_field.worst_draft_deg,
            "with_parts_percent": field.undercut_fraction() * 100.0,
            "with_parts_worst_draft_deg": field.worst_draft_deg,
            "note": "reported only: lost wax judges fill and detail, never the pull",
        },
        "crests": {
            "table_deg_r": crests,
            "joint_dips_mm": dips,
            "west_dips_mm": dips.iter().filter(|d| d[0] > 116.0 && d[0] <= 270.0).collect::<Vec<_>>(),
            "east_dips_mm": dips.iter().filter(|d| d[0] > 270.0 || d[0] < 64.0).collect::<Vec<_>>(),
            "least_dip_mm": least_dip,
        },
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "notes": field.notes, "parts_undercut_mm2": field.parts.iter().map(|p| p.undercut_area_mm2).sum::<f64>(), "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed, "carats": stones.as_ref().map_or(0.0, |s| s.total_carats), "tight_pairs": stones.as_ref().map_or(0, |s| s.tight_pairs), "closest": stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm)), "crowding": stones.as_ref().map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm)).collect::<Vec<_>>()), "warnings": warnings},
        "composition": comp,
        "grams_18k": grams,
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "stamped": coarse.solids.stamped, "notes": [&coarse.solids.notes, &coarse.parts.notes]},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len(), "shells": shells(&prepared.mesh).len(), "scale": prepared.scale, "notes": prepared.notes},
        "design": {"bytes": text.len(), "format_version": design_format, "embedded_alphas": d.layers.referenced_alphas().len(), "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Manticora / investment pattern")?;
        stl::write_stl(out.join("aculeus.stl"), &sting_shell, "Manticora / aculeus, cast apart")?;
        let mut entries = Vec::new();
        for (m, tint) in &gems {
            let (file, name, ior, dispersion, transmission) = if tint[0] > 0.1 {
                ("reference-ruby.stl", "Ruby", 1.77, 0.018, 0.72)
            } else {
                ("reference-spinel.stl", "Black spinel", 1.72, 0.020, 0.0)
            };
            stl::write_stl(out.join(file), m, &format!("Manticora reference {name}"))?;
            entries.push(json!({"mesh": file, "name": name, "tint": tint, "ior": ior, "dispersion": dispersion, "roughness": 0.065, "transmission": transmission}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": entries}))?)?;
    }
    renders(&out, &lib, &built, &gems, comp.neck_off_deg, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} (band {:.3}%, with parts {:.3}%), thinnest wall {:.2} mm at {:.0} deg; dfm {}; stones {reported} reported, {previewed} previewed; least joint dip {least_dip:.2} mm; lands {lands_ok}; {:.2} g in 18k",
        field.verdict.label(),
        band_field.undercut_fraction() * 100.0,
        field.undercut_fraction() * 100.0,
        field.thinnest_wall_mm,
        field.thinnest_wall_theta_deg,
        findings.len(),
        grams
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for w in report["stones"]["warnings"].as_array().into_iter().flatten() {
        println!("    stone: {w}");
    }
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Manticora failed a gate; see {}", out.join("report.json").display());
    Ok(())
}
