//! Bestiarium — Manticora, the tail that throws: a scorpion's tail round the finger, cast in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_manticora
//! target/release/examples/bestiarium_manticora [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{
        Attach, Component, ComponentRole, Document, Feature, Joint, Operation, Placement,
        Profile, Stage, builders,
    },
    castability::{self, CastProcess},
    csg, dfm, library, manufacturing as mf,
    field::{Blend, Layer, LayerEntry, SeatRunLayer, SeatStyle, Uv, Window, smoothstep},
    gem::{Gem, GemCut},
    mesh, outline,
    profile::{MAX_PROFILE_STEPS, ShankKey},
    render,
    setting::{self, RowPath, SolidKind, Stamp, StampRow, StampTop},
    sketch::{Geometry, Id, Sketch, Workplane},
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
/// The investment's fill floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
/// Ring angle of the stinger's root, degrees past the top.
const STING_ROOT_DEG: f64 = 22.5;
/// How far the hook's plane leans off the mid-plane about the root's radial, degrees.
const STING_LEAN_DEG: f64 = -15.0;
/// Gap left under the stinger's foot for the solder, mm.
const SOLDER_GAP_MM: f64 = 0.05;
/// The stinger's foot: keel-to-back along the ring by across the band, mm.
const SOCKET_LONG_MM: f64 = 3.1;
const SOCKET_WIDE_MM: f64 = 2.3;
/// How far the socket's top stands over the bulb at its highest point, mm.
const SOCKET_PROUD_MM: f64 = 0.25;
/// How far the socket reaches into the bulb, mm, and its flare there, degrees.
const SOCKET_DEPTH_MM: f64 = 1.6;
const SOCKET_FLARE_DEG: f64 = 14.0;
/// How far the ruby's girdle sits under the collet's own stand-off, mm.
const RUBY_SINK_MM: f64 = 1.15;
/// The collet's wall, mm, and the venom sac's rise under it.
const COLLET_WALL_MM: f64 = 0.55;
const SAC_RISE_MM: f64 = 0.95;
/// The spinel mound: this much wider than its stone, this high, and the metal left between two, mm.
const MOUND_STOCK_MM: f64 = 0.6;
const MOUND_HEIGHT_MM: f64 = 0.72;
const SPINEL_BRIDGE_MM: f64 = 1.9;
/// Least arc from the top to the neck joint, degrees.
const NECK_MIN_DEG: f64 = 24.0;
/// Where the spinels stop down each shoulder, degrees from the top.
const SPINEL_REACH_DEG: f64 = 150.0;
/// Painted layer heights, mm.
const TERGITE_MM: f64 = 0.80;
const PLEURA_MM: f64 = 0.22;
const GRANULE_MM: f64 = 0.10;
const GRAVER_MM: f64 = 0.06;
const ARTICULATION_MM: f64 = 0.45;
/// The atlas the hide is painted on: square texels over the 72 x 6.8 mm chart.
const AW: usize = 2048;
const AH: usize = 192;
/// Across the plate as shares of its rim: the dorsal and lateral carinae.
const DORSAL_Q: f64 = 0.44;
const LATERAL_Q: f64 = 0.88;
/// Down the flank as a share of it: the ventral-lateral carina.
const VENTRAL_U: f64 = 0.78;

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

/// The keyed HighDome tail: a swollen venom bulb at the top, a neck `neck_off` degrees off it, then segments thinning to the palm.
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
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey {
        theta_deg,
        width_scale,
        thickness_scale,
        crown_scale: 1.0,
    };
    let mut keys = vec![key(90.0, 1.30, 1.62), key(270.0, 0.92, 0.92)];
    for (off, w, t) in [
        (0.55 * neck_off, 1.27, 1.57),
        (neck_off, 1.00, 1.08),
        (neck_off + 17.0, 1.13, 1.30),
        (85.0, 1.07, 1.19),
        (120.0, 1.01, 1.08),
        (152.0, 0.95, 0.98),
    ] {
        keys.push(key(90.0 - off, w, t));
        keys.push(key(90.0 + off, w, t));
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

fn snap(p: P2) -> P2 {
    p.map(|v| (v * 1e6).round() / 1e6)
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

/// Unit radial and ring-forward (toward rising angle) directions at `theta`.
fn frame_at(theta: f64) -> (P3, P3) {
    let t = theta.to_radians();
    ([t.cos(), t.sin(), 0.0], [-t.sin(), t.cos(), 0.0])
}

/// Whether a point of the foot plane lies inside the teardrop `long` by `wide`, keel toward +x.
fn in_teardrop(x: f64, y: f64, long: f64, wide: f64) -> bool {
    let (a, b) = (0.5 * long, 0.5 * wide);
    if x < -a || x > a {
        return false;
    }
    let half = if x < -0.12 * a {
        b * (1.0 - ((x + 0.12 * a) / (0.88 * a)).powi(2)).max(0.0).sqrt()
    } else {
        b * (1.0 - (x + 0.12 * a) / (1.12 * a)).max(0.0)
    };
    y.abs() <= half
}

/// The displaced surface: every atlas sample moved out along its normal by the layer stack.
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
            std::array::from_fn(|k| s.p[k] + s.n[k] * h)
        })
        .collect()
}

/// The stinger's frame at its root: the radial, the direction its hook leans over the ruby, and the foot's two axes (keel outward).
struct StingFrame {
    er: P3,
    lean: P3,
    x: P3,
    y: P3,
}

fn sting_frame(theta: f64) -> StingFrame {
    let t = theta.to_radians();
    let (er, eth) = ([t.cos(), t.sin(), 0.0], [-t.sin(), t.cos(), 0.0]);
    let (s, c) = STING_LEAN_DEG.to_radians().sin_cos();
    StingFrame {
        er,
        lean: [-c * eth[0], -c * eth[1], s],
        x: [c * eth[0], c * eth[1], -s],
        y: [s * eth[0], s * eth[1], c],
    }
}

fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Distance out along the radial at `theta` at which a flat foot `long` by `wide` clears the finished surface everywhere.
fn foot_clearance(d: &RingDesign, lib: &AlphaLibrary, a: &Atlas, theta: f64, long: f64, wide: f64) -> f64 {
    let f = sting_frame(theta);
    displaced(d, lib, a, |s| ringdesign_core::field::wrap_delta(s.theta - theta, 360.0).abs() < 12.0)
        .into_iter()
        .filter(|p| in_teardrop(dot3(*p, f.x), dot3(*p, f.y), long + 0.3, wide + 0.3))
        .map(|p| dot3(p, f.er))
        .fold(0.0, f64::max)
}

/// A cubic Bézier chain through `knots`, each with its tangent (scaled by a third of the chord).
fn hermite_chain(knots: &[(P2, P2)]) -> Vec<[P2; 4]> {
    knots
        .windows(2)
        .map(|w| {
            let ((a, ta), (b, tb)) = (w[0], w[1]);
            let chord = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt() / 3.0;
            let n = |t: P2| {
                let l = t[0].hypot(t[1]).max(1e-12);
                [t[0] / l, t[1] / l]
            };
            let (ua, ub) = (n(ta), n(tb));
            [
                a,
                [a[0] + ua[0] * chord, a[1] + ua[1] * chord],
                [b[0] - ub[0] * chord, b[1] - ub[1] * chord],
                b,
            ]
        })
        .collect()
}

fn bezier(c: &[P2; 4], t: f64) -> P2 {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    std::array::from_fn(|k| a * c[0][k] + b * c[1][k] + cc * c[2][k] + d * c[3][k])
}

/// Smallest radius of curvature along a cubic.
fn tightest_bend(c: &[P2; 4]) -> f64 {
    (0..=200)
        .map(|i| {
            let t = i as f64 / 200.0;
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

/// The stinger's centre line in the parting plane, and what it measures.
struct Sting {
    pieces: Vec<[P2; 4]>,
    /// The foot's centre, the socket's top under it, and their ring angle.
    foot: P3,
    socket: P3,
    root_theta: f64,
    length_mm: f64,
    tightest_bend_mm: f64,
    socket_proud_mm: f64,
}

/// The aculeus: up off the bulb behind the collet, over the ruby, its point hanging over the far rim.
fn sting_path(d: &RingDesign, lib: &AlphaLibrary, a: &Atlas, lip_r: f64, table_r: f64) -> Sting {
    let root_theta = 90.0 + STING_ROOT_DEG;
    let clear = foot_clearance(d, lib, a, root_theta, SOCKET_LONG_MM, SOCKET_WIDE_MM);
    let socket_r = clear + SOCKET_PROUD_MM;
    let foot_r = socket_r + SOLDER_GAP_MM;
    let f = sting_frame(root_theta);
    // In the hook's own plane: out along the radial, and toward the ruby at the lean.
    let at = |psi: f64, r: f64| {
        let t = psi.to_radians();
        [r * t.cos(), r * t.sin()]
    };
    let dir = |psi: f64, radial: f64, toward: f64| {
        let t = psi.to_radians();
        [radial * t.cos() - toward * t.sin(), radial * t.sin() + toward * t.cos()]
    };
    let psi = |theta: f64| root_theta - theta;
    let knots = [
        (at(0.0, foot_r), dir(0.0, 1.0, 0.0)),
        (at(1.2, foot_r + 1.7), dir(1.2, 1.0, 0.55)),
        (at(psi(98.0), table_r + 2.2), dir(psi(98.0), 0.25, 1.0)),
        (at(psi(84.5), table_r + 1.6), dir(psi(84.5), -0.75, 1.0)),
        (at(psi(79.0), lip_r + 0.95), dir(psi(79.0), -1.0, 0.3)),
    ];
    let pieces: Vec<[P2; 4]> = hermite_chain(&knots).into_iter().map(|c| c.map(snap)).collect();
    let length_mm = pieces
        .iter()
        .map(|c| {
            (0..64)
                .map(|i| {
                    let (a, b) = (bezier(c, i as f64 / 64.0), bezier(c, (i + 1) as f64 / 64.0));
                    (b[0] - a[0]).hypot(b[1] - a[1])
                })
                .sum::<f64>()
        })
        .sum();
    let tightest_bend_mm = pieces.iter().map(tightest_bend).fold(f64::MAX, f64::min);
    let foot = pieces[0][0][0];
    Sting {
        foot: f.er.map(|v| v * foot),
        socket: f.er.map(|v| v * socket_r),
        root_theta,
        pieces,
        length_mm,
        tightest_bend_mm,
        socket_proud_mm: socket_r - crest_r(d, root_theta),
    }
}

fn path_sketch(s: &Sting) -> Sketch {
    let f = sting_frame(s.root_theta);
    let mut k = Sketch {
        name: "Aculeus path".into(),
        plane: Workplane {
            origin: [0.0; 3],
            x: f.er,
            y: f.lean,
            on_face: None,
        },
        ..Sketch::default()
    };
    let mut last: Option<Id> = None;
    for c in &s.pieces {
        let a = last.unwrap_or_else(|| k.point(c[0]));
        let (b, cc, e) = (k.point(c[1]), k.point(c[2]), k.point(c[3]));
        k.entity(Geometry::Bezier {
            points: [a, b, cc, e],
        });
        last = Some(e);
    }
    k
}

/// A keeled teardrop `long` by `wide` square to the radial at `theta`, centred on `origin`, its keel toward rising angle.
fn section_sketch(name: &str, origin: P3, theta: f64, long: f64, wide: f64) -> Sketch {
    let f = sting_frame(theta);
    let mut k = Sketch {
        name: name.into(),
        plane: Workplane {
            origin,
            x: f.x,
            y: f.y,
            on_face: None,
        },
        ..Sketch::default()
    };
    let (a, b) = (0.5 * long, 0.5 * wide);
    let keel = k.point([a, 0.0]);
    let top = k.point([-0.12 * a, b]);
    let back = k.point([-a, 0.0]);
    let bottom = k.point([-0.12 * a, -b]);
    let p = |k: &mut Sketch, x: f64, y: f64| k.point([x, y]);
    let (c1, c2) = (p(&mut k, 0.62 * a, 0.42 * b), p(&mut k, 0.34 * a, b));
    k.entity(Geometry::Bezier { points: [keel, c1, c2, top] });
    let (c1, c2) = (p(&mut k, -0.62 * a, b), p(&mut k, -a, 0.58 * b));
    k.entity(Geometry::Bezier { points: [top, c1, c2, back] });
    let (c1, c2) = (p(&mut k, -a, -0.58 * b), p(&mut k, -0.62 * a, -b));
    k.entity(Geometry::Bezier { points: [back, c1, c2, bottom] });
    let (c1, c2) = (p(&mut k, 0.34 * a, -b), p(&mut k, 0.62 * a, -0.42 * b));
    k.entity(Geometry::Bezier { points: [bottom, c1, c2, keel] });
    k
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

/// The CAD parts: the ruby in its venom collet, the seat bur, the joined socket and the separate stinger.
fn parts(d: &mut RingDesign, lib: &AlphaLibrary) -> Result<Sting> {
    let gem = ruby();
    let a = Atlas::of(d, AW, 256)?;
    let top_r = displaced(d, lib, &a, |s| s.i % a.width == a.width / 4)
        .into_iter()
        .map(|p| p[0].hypot(p[1]))
        .fold(0.0, f64::max);
    let stand = builders::stand_off_mm(builders::BEZEL, gem) - RUBY_SINK_MM;
    let girdle_r = top_r + stand;
    let lip_r = girdle_r + 0.35 * gem.crown_mm();
    let table_r = girdle_r + gem.crown_mm();
    println!(
        "  bulb crest r {top_r:.3}, girdle r {girdle_r:.3}, lip r {lip_r:.3}, table r {table_r:.3}"
    );
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
            theta_deg: 90.0,
            across_mm: 0.0,
            height_mm: stand,
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
        json!({"wall_mm": COLLET_WALL_MM, "lip": 0.35}),
    ))?;
    doc.append(builders::feature_on(
        4,
        "Seat bur",
        builders::BUR,
        2,
        json!({"through": false}),
    ))?;
    let sting = sting_path(d, lib, &a, lip_r, table_r);
    let (er, _) = frame_at(sting.root_theta);
    let depth = SOCKET_DEPTH_MM + sting.socket_proud_mm;
    let base = [
        sting.socket[0] - er[0] * depth,
        sting.socket[1] - er[1] * depth,
        0.0,
    ];
    let flare = 1.0 + depth * SOCKET_FLARE_DEG.to_radians().tan() * 2.0 / SOCKET_WIDE_MM;
    doc.append(feature(
        5,
        "Socket section",
        Operation::Sketch {
            sketch: section_sketch(
                "Socket section",
                base,
                sting.root_theta,
                SOCKET_LONG_MM * flare,
                SOCKET_WIDE_MM * flare,
            ),
        },
        Component::default(),
    ))?;
    let mut rise = Sketch {
        name: "Socket rise".into(),
        ..Sketch::default()
    };
    let (a, b) = (rise.point([base[0], base[1]]), rise.point([sting.socket[0], sting.socket[1]]));
    rise.entity(Geometry::Line { a, b });
    doc.append(feature(
        6,
        "Aculeus socket",
        Operation::Twist {
            sketch: Profile::Feature { feature: 5 },
            path: rise,
            degrees: 0.0,
            end_scale: 1.0 / flare,
        },
        Component {
            attach: Attach::Join,
            stage: Stage::Cast,
            placement: Placement::Free,
            blend_mm: 0.0,
            ..Component::default()
        },
    ))?;
    doc.append(feature(
        7,
        "Aculeus section",
        Operation::Sketch {
            sketch: section_sketch("Aculeus section", sting.foot, sting.root_theta, SOCKET_LONG_MM, SOCKET_WIDE_MM),
        },
        Component::default(),
    ))?;
    doc.append(feature(
        8,
        "Aculeus",
        Operation::Twist {
            sketch: Profile::Feature { feature: 7 },
            path: path_sketch(&sting),
            degrees: 18.0,
            end_scale: 0.08,
        },
        Component {
            attach: Attach::Separate,
            stage: Stage::Cast,
            placement: Placement::Free,
            bench_notes: "Cast apart; solder to the socket after the ruby is set and the collet burnished.".into(),
            ..Component::default()
        },
    ))?;
    doc.joints.push(Joint {
        a: 6,
        b: 8,
        clearance_mm: SOLDER_GAP_MM,
        method: "Solder after the ruby is set".into(),
        notes: "The aculeus is its own casting: set and burnish the ruby first, then solder the stinger's foot to its socket behind the collet.".into(),
    });
    d.cad = Some(doc);
    Ok(sting)
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

/// A saw-tooth row of denticles along a keel, `n` to a plate, each leaning toward the lip: 0..1.
fn denticles(t: f64, n: f64) -> f64 {
    let phase = (t * n).fract();
    smoothstep(0.0, 0.72, phase) * (1.0 - smoothstep(0.78, 0.98, phase))
}

/// How far down the flank a plate reaches at `t`, as a share of the flank: lobed, shortest at the joints.
fn plate_reach(x: f64, back: f64) -> f64 {
    0.70 + 0.24 * smoothstep(0.1, 1.1, x.min(back)).sqrt()
}

/// A metasomal segment at `t` from its anterior joint (0) to its posterior lip (1), on a plate `len` mm long,
/// `q` across as a share of the rim and `u` as a share of the whole flank down to the bore edge: 0..1.
fn segment(t: f64, q: f64, u: f64, len: f64) -> f64 {
    let (x, back, qa) = (t * len, (1.0 - t) * len, q.abs());
    // Constricted at both joints, swelling to the posterior third, a flared lip, then the drop into the joint.
    let tuck = smoothstep(0.02, 0.2, x);
    let swell = 0.30 + 0.58 * smoothstep(0.1, 0.72 * len, x).powf(0.7);
    let lip = 0.16 * (-((back - 0.36) / 0.14).powi(2)).exp();
    let drop = smoothstep(0.08, 0.24, back);
    let barrel = (swell + lip) * drop * (0.25 + 0.75 * tuck);
    // The plate wraps the tube to a lobed margin that rolls off onto the membrane.
    let reach = plate_reach(x, back);
    let sleeve = 1.0 - smoothstep(reach - 0.1, reach, u);
    let furrow = 1.0 - 0.14 * (1.0 - smoothstep(0.2, DORSAL_Q - 0.05, qa));
    let keel_run = smoothstep(0.12, 0.5, x) * drop;
    let teeth = (len / 0.95).round().max(3.0);
    let keel = |d: f64, h: f64, w: f64| (-(d / w).powi(2)).exp() * (h + 0.2 * denticles(t, teeth));
    let dorsal = keel(qa - DORSAL_Q, 0.2, 0.06);
    let lateral = keel(qa - LATERAL_Q, 0.15, 0.07);
    let keels = (dorsal + lateral) * keel_run;
    ((barrel * furrow + keels) * sleeve).clamp(0.0, 1.0)
}

/// A fold of the pleural membrane either side of each joint, down the lower flank: 0..1.
fn pleura(t: f64, u: f64, len: f64) -> f64 {
    let near = (t * len).min((1.0 - t) * len);
    let fold = if near < 0.8 {
        smoothstep(0.15, 0.55, 0.5 - 0.5 * (2.0 * PI * near / 0.8).cos())
    } else {
        0.0
    };
    fold * smoothstep(0.68, 0.71, u) * (1.0 - smoothstep(0.9, 0.93, u))
}

/// The nearest granule of a jittered hex lattice of `pitch` in hide millimetres: its centre, radius and cell.
fn granule_cell(along: f64, across: f64, pitch: f64, radius: f64) -> (P2, f64, i64, i64) {
    let row = (across / (pitch * 0.866)).round();
    let shift = if (row as i64).rem_euclid(2) == 0 { 0.0 } else { 0.5 * pitch };
    let col = ((along - shift) / pitch).round();
    let (i, j) = (col as i64, row as i64);
    let cx = col * pitch + shift + (skin::hash(i, j) - 0.5) * 0.24 * pitch;
    let cy = row * pitch * 0.866 + (skin::hash(j, i + 7) - 0.5) * 0.24 * pitch;
    ([cx, cy], radius * (0.85 + 0.3 * skin::hash(i + 13, j)), i, j)
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

fn paint(d: &mut RingDesign, lib: &mut AlphaLibrary, lat: &Lattice, art: &Path) -> Result<Hides> {
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
    let plate = |s: &Sample| -> Option<(f64, f64, f64, f64)> {
        let p = hide.at(s);
        let (_, f, len) = joints.at(p.along.abs())?;
        Some((1.0 - f, p.across / p.rim.max(0.3), len, p.across.abs() / (p.rim + p.wall).max(0.5)))
    };
    let tergites = a.paint("Manticora tergites", |s| {
        plate(s).map_or(0.0, |(t, q, len, u)| segment(t, q, u, len))
    });
    let pleurae = a.paint("Manticora pleural folds", |s| {
        plate(s).map_or(0.0, |(t, _, len, u)| pleura(t, u, len))
    });
    let articulations = a.paint("Manticora articulations", |s| {
        let Some((t, _, len, u)) = plate(s) else { return 0.0 };
        let (x, back) = (t * len, (1.0 - t) * len);
        let v = (1.0 - smoothstep(0.0, 0.34, x)).max(1.0 - smoothstep(0.0, 0.3, back));
        v * v * (3.0 - 2.0 * v) * (1.0 - smoothstep(0.9, 0.99, u))
    });
    let granules = a.paint("Manticora granulation", |s| {
        let p = hide.at(s);
        let ([cx, cy], r, i, j) = granule_cell(p.along.abs(), p.across, 0.48, 0.18);
        let d = (p.along.abs() - cx).hypot(p.across - cy) / r;
        if d >= 1.0 || skin::hash(i + 101, j) >= 0.72 {
            return 0.0;
        }
        // Kept or dropped whole, by where its centre stands on the plate.
        let Some((_, f, len)) = joints.at(cx) else { return 0.0 };
        let (x, back) = ((1.0 - f) * len, f * len);
        let (qa, u) = (cy.abs() / p.rim.max(0.3), cy.abs() / (p.rim + p.wall).max(0.5));
        let on = x > 0.45
            && back > 0.75
            && (qa - DORSAL_Q).abs() > 0.13
            && (qa - LATERAL_Q).abs() > 0.13
            && qa > DORSAL_Q + 0.08
            && u < VENTRAL_U - 0.05
            && u < plate_reach(x, back) - 0.12;
        if on { (1.0 - d * d).sqrt() } else { 0.0 }
    });
    let ruby = ruby();
    let (sac_along, sac_across) = (0.5 * ruby.w_mm + COLLET_WALL_MM + 0.45, 0.5 * ruby.l_mm + COLLET_WALL_MM + 0.45);
    let sac = a.paint("Manticora venom sac", |s| {
        let p = hide.at(s);
        let (x, y) = (p.along / sac_along, p.across / sac_across);
        let q = (x.powi(4) + y.powi(4)).powf(0.25);
        let fall = 1.0 - smoothstep(1.0, 1.0 + 1.5 / sac_along, q);
        let u = p.across.abs() / (p.rim + p.wall).max(0.5);
        fall * (1.0 - smoothstep(0.82, 0.97, u))
    });
    let graver = a.paint("Manticora graver lines", |s| {
        let Some((t, q, len, _)) = plate(s) else { return 0.0 };
        let (x, back, qa) = (t * len, (1.0 - t) * len, q.abs());
        let run = smoothstep(0.3, 0.6, x) * smoothstep(0.45, 0.6, back);
        let line = |c: f64| 1.0 - smoothstep(0.012, 0.024, (qa - c).abs());
        (line(DORSAL_Q).max(line(LATERAL_Q)) * run * (1.0 - smoothstep(0.96, 1.0, qa))).max(
            (1.0 - smoothstep(0.03, 0.06, (back - 0.55).abs())) * (1.0 - smoothstep(0.9, 0.97, qa)),
        )
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
        (sac, SAC_RISE_MM, Blend::Max, false),
        (granules, GRANULE_MM, Blend::Add, false),
        (graver, GRAVER_MM, Blend::Subtract, true),
    ] {
        let name = alpha.name.clone();
        let png = alpha.to_png16()?;
        std::fs::write(
            art.join(format!("{}.png", name.to_lowercase().replace(' ', "-"))),
            &png,
        )?;
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

/// Quill length at the neck, mm; the rows grade it with the plates.
const QUILL_MM: f64 = 3.4;
const QUILL_W_MM: f64 = 0.95;
/// Where the quills stand across the flank, as a share of the rim.
const QUILL_Q: f64 = 1.25;
/// How far each quill turns out toward its own band edge, degrees.
const QUILL_SPLAY_DEG: f64 = 22.0;

#[derive(Default, serde::Serialize)]
struct Quills {
    count: usize,
    taper: f64,
    /// Worst offset of a quill from its plate's centre, as a share of that plate.
    worst_drift: f64,
    v_offset_mm: f64,
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
    let stamp = |name: &str, rot: f64| Stamp {
        name: name.into(),
        theta_deg: 0.0,
        v_mm: 0.0,
        rot_deg: rot,
        outline: outline::quill(QUILL_MM, QUILL_W_MM, 0.14),
        height_mm: 0.22,
        sink_mm: 0.35,
        draft_deg: 0.0,
        cut: false,
        bench: false,
        along_pull: false,
        tier: 0,
        top: StampTop::Cone {
            apex_mm: 1.0,
            at: [0.18 * QUILL_MM, 0.0],
            tip_mm: 0.12,
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
    let drift = |taper: f64| -> f64 {
        let r = row(taper, "probe", ctx.crest_v_mm + dv, 0.0);
        let mut struck: Vec<f64> = setting::stamp_row(d, &StampRow { mirror_shoulders: false, ..r })
            .iter()
            .map(|s| s.theta_deg)
            .collect();
        struck.sort_by(f64::total_cmp);
        struck
            .iter()
            .zip(row_centres)
            .zip(&lengths)
            .map(|((s, c), l)| (s - c).abs() / l)
            .fold(0.0, f64::max)
    };
    let (mut best, mut worst) = (0.0, f64::MAX);
    for i in 0..=70 {
        let t = i as f64 * 0.01;
        let w = drift(t);
        if w < worst {
            (best, worst) = (t, w);
        }
    }
    ensure!(worst <= 0.25, "The quill rows drift {worst:.2} of a plate off the plates");
    let before = d.stamps.len();
    for (name, v, rot) in [
        ("Quill: high flank", ctx.crest_v_mm + dv, QUILL_SPLAY_DEG),
        ("Quill: low flank", ctx.crest_v_mm - dv, -QUILL_SPLAY_DEG),
    ] {
        d.stamps.extend(setting::stamp_row(d, &row(best, name, v, rot)));
    }
    Ok(Quills {
        count: d.stamps.len() - before,
        taper: best,
        worst_drift: worst,
        v_offset_mm: dv,
    })
}

// --- The ring ------------------------------------------------------------------

#[derive(Default, serde::Serialize)]
struct Composition {
    hides: Hides,
    quills: Quills,
    stinger_length_mm: f64,
    stinger_tightest_bend_mm: f64,
    socket_proud_mm: f64,
    spinel_stations: usize,
    spinel_taper: f64,
    neck_off_deg: f64,
}

fn author(art: &Path) -> Result<(RingDesign, AlphaLibrary, Composition)> {
    let lat = lattice(&band(30.0))?;
    let mut d = band(lat.neck_off());
    let mut lib = AlphaLibrary::builtin();
    let hides = paint(&mut d, &mut lib, &lat, art)?;
    spinel_runs(&mut d, &lat);
    let quills = quill_rows(&mut d, &lat)?;
    d.bake_all(&mut lib);
    let sting = parts(&mut d, &lib)?;
    Ok((
        d,
        lib,
        Composition {
            hides,
            quills,
            stinger_length_mm: sting.length_mm,
            stinger_tightest_bend_mm: sting.tightest_bend_mm,
            socket_proud_mm: sting.socket_proud_mm,
            spinel_stations: lat.run.count as usize,
            spinel_taper: lat.run.taper,
            neck_off_deg: lat.neck_off(),
        },
    ))
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
    for (stone, frame) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let fit = setting::Fit {
            surface_z: stone.seat.height_mm - stand,
            through_mm: None,
            prongs: 0,
        };
        let _ = frame;
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

/// How close the stinger comes to the ring and to the ruby, away from its own foot.
#[derive(Default, serde::Serialize)]
struct StingClearance {
    to_metal_mm: f64,
    metal_at: P3,
    to_ruby_mm: f64,
    ruby_at: P3,
    /// Ring angle of the closest approach to the metal, degrees.
    theta_deg: f64,
    foot_gap_mm: f64,
}

fn sting_clearance(built: &mesh::BuildResult, sting_shell: &mesh::Mesh, ring: &mesh::Mesh, ruby: Option<&mesh::Mesh>, foot: P3) -> StingClearance {
    let pts: Vec<P3> = sting_shell
        .vertices
        .iter()
        .map(|v| [v.0 as f64, v.1 as f64, v.2 as f64])
        .collect();
    let away: Vec<P3> = pts.iter().copied().filter(|p| dot3(sub3(*p, foot), sub3(*p, foot)).sqrt() > 2.2).collect();
    let near: Vec<P3> = pts.iter().copied().filter(|p| dot3(sub3(*p, foot), sub3(*p, foot)).sqrt() <= 2.2).collect();
    let _ = built;
    let mut out = StingClearance::default();
    if let Some((d, p, _)) = nearest(&away, ring, 1.5) {
        out.to_metal_mm = d;
        out.metal_at = p;
        out.theta_deg = p[1].atan2(p[0]).to_degrees().rem_euclid(360.0);
    } else {
        out.to_metal_mm = 1.5;
    }
    if let Some(r) = ruby {
        match nearest(&pts, r, 1.5) {
            Some((d, p, _)) => {
                out.to_ruby_mm = d;
                out.ruby_at = p;
            }
            None => out.to_ruby_mm = 1.5,
        }
    }
    out.foot_gap_mm = nearest(&near, ring, 0.6).map_or(0.6, |(d, ..)| d);
    out
}

/// The section through the finger axis at `theta`, as the section pane cuts it: ring, stinger and ruby.
fn section_svg(path: &Path, theta: f64, shells: &[(&mesh::Mesh, &str)], mark: Option<(P3, f64)>) -> Result<()> {
    let t = theta.to_radians();
    let (er, en) = ([t.cos(), t.sin(), 0.0], [-t.sin(), t.cos(), 0.0]);
    let (r0, r1, z0, z1) = (9.0, 20.0, -6.5, 6.5);
    let scale = 60.0;
    let map = |p: P3| ((dot3(p, er) - r0) * scale, (z1 - p[2]) * scale);
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
            if cut.len() == 2 && dot3(cut[0], er) > 0.0 {
                let ((x0, y0), (x1, y1)) = (map(cut[0]), map(cut[1]));
                d.push_str(&format!("M{x0:.2} {y0:.2}L{x1:.2} {y1:.2}"));
            }
        }
        body.push_str(&format!("<path d=\"{d}\" stroke=\"{colour}\" stroke-width=\"1.4\" fill=\"none\"/>\n"));
    }
    if let Some((p, gap)) = mark {
        let (x, y) = map(p);
        body.push_str(&format!("<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"7\" stroke=\"#e0453a\" stroke-width=\"2\" fill=\"none\"/><text x=\"{:.1}\" y=\"{:.1}\" fill=\"#e0453a\" font-family=\"sans-serif\" font-size=\"22\">{gap:.3} mm</text>\n", x + 12.0, y - 10.0));
    }
    let (w, h) = ((r1 - r0) * scale, (z1 - z0) * scale);
    std::fs::write(
        path,
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\"><rect width=\"100%\" height=\"100%\" fill=\"#111\"/><text x=\"16\" y=\"34\" fill=\"#ddd\" font-family=\"sans-serif\" font-size=\"24\">Section at {theta:.1} deg: ring gold, stinger white, ruby red; radius right, finger axis down</text>\n{body}</svg>"),
    )?;
    Ok(())
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
    ("hero", 0.3, 0.45),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", -0.9, 0.62),
    ("reverse", PI - 0.5, 0.35),
];

/// Studio-gold renders with stones set, a close-up of the head, and the bare tail against the finished ring.
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])], neck_off: f64, edge: usize) -> Result<()> {
    let mut parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    parts.extend(gems.iter().map(|(m, tint)| render::Part::tinted_stone(m, *tint)));
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let head = crop(&built.mesh, [0.0, 13.0, 0.0], 12.0);
    let mut close = vec![render::Part::metal(&head, render::GOLD), render::Part::metal(&built.mesh, render::GOLD)];
    close.extend(gems.iter().map(|(m, tint)| render::Part::tinted_stone(m, *tint)));
    render::write_png_parts(out.join("stones.png"), &close, 0.4, 0.75, edge)?;
    let bare = mesh::try_build(&band(neck_off), lib, draft_params())?;
    let _ = d;
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
    let (d, lib, comp) = author(&art)?;
    let author_s = started.elapsed().as_secs_f64();
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let quality = built.report.quality;
    println!(
        "  {} triangles in {build_s:.1} s (authored in {author_s:.1} s); watertight {watertight}; degenerate {degenerate}; self-crossings {crossings}; stamps {}/{}; seats {}",
        built.mesh.faces.len(),
        built.solids.stamped,
        d.stamps.len(),
        built.solids.resolved
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
    ensure!(groups.len() == 2, "Expected the ring and the stinger as two shells, found {}", groups.len());
    let (sting_i, ring_i) = if far(&groups[0]) > far(&groups[1]) { (0, 1) } else { (1, 0) };
    let sting_shell = submesh(&built.mesh, &groups[sting_i]);
    let ring_shell = submesh(&built.mesh, &groups[ring_i]);
    let foot = {
        let s = sting_frame(90.0 + STING_ROOT_DEG);
        let r = built.mesh.vertices.iter().filter(|_| false).count() as f64;
        let _ = r;
        s.er.map(|v| v * (crest_r(&d, 90.0 + STING_ROOT_DEG) + comp.socket_proud_mm + SOLDER_GAP_MM))
    };
    let clearance = sting_clearance(&built, &sting_shell, &ring_shell, ruby_mesh, foot);
    let mut section_shells = vec![(&ring_shell, "#d9b76a"), (&sting_shell, "#f4f4f4")];
    if let Some(r) = ruby_mesh {
        section_shells.push((r, "#e0453a"));
    }
    section_svg(
        &out.join("stinger-section.svg"),
        clearance.theta_deg,
        &section_shells,
        Some((clearance.metal_at, clearance.to_metal_mm)),
    )?;
    println!(
        "  stinger: {:.3} mm clear of the metal at {:.1} deg, {:.3} mm clear of the ruby, foot gap {:.3} mm",
        clearance.to_metal_mm, clearance.theta_deg, clearance.to_ruby_mm, clearance.foot_gap_mm
    );
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
    // Stamps at the coarse pitch: phantoms move with resolution, real faults converge.
    let coarse_params = BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() };
    let coarse = mesh::try_build(&d, &lib, coarse_params)?;
    let (cw, cd, cx) = geometry(&coarse.mesh);
    let coarse_ok = cw && cd == 0 && cx == 0 && coarse.solids.stamped == d.stamps.len() && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}, stamps {}/{}", coarse.solids.stamped, d.stamps.len());
    // The investment pattern: the ring's own casting; the stinger pours apart.
    let mut setup = mf::Setup::from_design(&d);
    setup.recipe.name = "Manticora / investment / Gold 18k".into();
    setup.recipe.alloy = "Gold 18k".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(1.3, |m| m.shrink_pct);
    setup.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    setup.bench_notes = "Invest the tail with its collet, seats and quills. Cast the aculeus apart. Bead-set the spinels, set and burnish the ruby, then solder the aculeus to its socket.".into();
    let prepared = mf::prepare(&d, &lib, &setup, params)?;
    let (pw, pd, px) = geometry(&prepared.mesh);
    let aculeus_shells = shells(&prepared.mesh).len();
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
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview, no warnings", reported == previewed && warnings.is_empty()),
        ("stinger clears the collet and the ruby by 0.3 mm", clearance.to_metal_mm >= 0.3 && clearance.to_ruby_mm >= 0.3),
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
        "stinger": {"clearance": clearance, "length_mm": comp.stinger_length_mm, "tightest_bend_mm": comp.stinger_tightest_bend_mm, "foot_mm": [SOCKET_LONG_MM, SOCKET_WIDE_MM], "tip_mm": [SOCKET_LONG_MM * 0.08, SOCKET_WIDE_MM * 0.08], "twist_deg": 18.0, "lean_deg": STING_LEAN_DEG, "socket_proud_mm": comp.socket_proud_mm, "section": "stinger-section.svg"},
        "field": {"verdict": field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "worst_draft_deg": field.worst_draft_deg, "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "notes": field.notes, "parts_undercut_mm2": field.parts.iter().map(|p| p.undercut_area_mm2).sum::<f64>(), "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed, "carats": stones.as_ref().map_or(0.0, |s| s.total_carats), "tight_pairs": stones.as_ref().map_or(0, |s| s.tight_pairs), "closest": stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm)), "crowding": stones.as_ref().map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm)).collect::<Vec<_>>()), "warnings": warnings},
        "composition": comp,
        "grams_18k": grams,
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "stamped": coarse.solids.stamped},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len(), "shells": aculeus_shells, "scale": prepared.scale, "notes": prepared.notes},
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
    renders(&out, &d, &lib, &built, &gems, comp.neck_off_deg, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} ({:.4}% undercut), thinnest wall {:.2} mm at {:.0} deg; dfm {}; stones {reported} reported, {previewed} previewed; {:.2} g in 18k",
        field.verdict.label(),
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

/// The faces of `m` within `radius` of `centre`, as a mesh of their own, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let faces: Vec<usize> = (0..m.faces.len()).filter(|k| m.faces[*k].iter().all(|&i| near(i))).collect();
    submesh(m, &faces)
}
