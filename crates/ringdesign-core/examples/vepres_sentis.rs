//! Vepres — Sentis, the briar: two thorned briar canes twined round the finger, parted at the crown round a wild
//! rose's hip (an opaque red cabochon with its dried calyx and stalk), framed by two large five-leaflet rose leaves
//! laid over the top. CAD only, lost wax. (The rethink of 2026-10-03; the four-cane thicket read as a crown of thorns.)
//! cargo build --release -p ringdesign-core --example vepres_sentis
//! target/release/examples/vepres_sentis [OUT_DIR] [--draft] [--verify]
use anyhow::Result;
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, Placement, Stage, SurfaceKind, builders, stored},
    castability, csg, dfm,
    gem::{Gem, GemCut, GemForm},
    library, mesh, render,
    sketch::{Geometry, Sketch, Workplane},
    cad::TwistPath,
    stl,
};
use serde_json::json;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

/// The stage these outputs record.
const STAGE: &str = "round 3";
/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The lost-wax section floor, and the investment's detail floor.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;
/// The crown, where the hip sits.
const CROWN_DEG: f64 = 90.0;
/// The analytic seat surface `Placement::Ring` stands parts on with no band: inner radius plus this thickness.
const SEAT_THICK_MM: f64 = 1.4;
/// Two canes, twined twice round each other round the ring: parted either side of the crown, crossing at the shoulders.
const CANES: usize = 2;
const WAVES: f64 = 2.0;
/// The twine's centre radius and its radial and across half-sizes; it opens across the finger toward the crown.
const TWINE_R_MM: f64 = 10.85;
const TWINE_RADIAL_MM: f64 = 0.5;
const TWINE_ACROSS_MM: f64 = 1.6;
const TWINE_CROWN_OPEN_MM: f64 = 0.95;
/// Each cane's wander, radians of phase, zero at crown and palm.
const WANDER: [f64; CANES] = [0.25, -0.25];
/// Cane radius at the palm and at the crown.
const CANE_PALM_MM: f64 = 0.7;
const CANE_CROWN_MM: f64 = 0.9;
/// The cane section's five lobes, as a share of its radius, and the turns they make round each cane.
const CANE_LOBE: f64 = 0.06;
const CANE_TURNS: f64 = 3.0;
/// Path points round each closed cane.
const STATIONS: usize = 48;
/// Prickles per cane before those facing the finger or under the crown's leaves and hip are dropped.
const THORNS_PER_CANE: usize = 48;
/// How deep each prickle's foot sinks into its cane, as a share of its size.
const THORN_SINK_MM: f64 = 0.26;
/// The hip: an oval cabochon, its length (along the finger) and width.
const HIP_L_MM: f64 = 6.4;
const HIP_W_MM: f64 = 4.8;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

/// The hip: an oval cabochon in the orange-red of a ripe rose hip.
fn hip_gem() -> Gem {
    Gem { l_mm: HIP_L_MM, form: GemForm::Cabochon, preview_tint: Some(HIP_TINT), ..Gem::calibrated(GemCut::Oval, HIP_W_MM) }
}
const HIP_TINT: [f32; 3] = [0.72, 0.20, 0.06];

/// The design with no band: size, process and the analytic seat.
fn ground() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Sentis".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.width_mm = 6.0;
    d.profile.thickness_mm = SEAT_THICK_MM;
    probe::cast_in(&mut d, &probe::wax_setup(0.1));
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d
}

/// The seat radius `Placement::Ring` measures heights from.
fn seat_r(d: &RingDesign) -> f64 {
    d.inner_radius_mm() + d.profile.thickness_mm
}

type P3 = [f64; 3];

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add3(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    a.map(|v| v * k)
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: P3) -> P3 {
    let l = dot(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}
/// Rounded to the micron, so the design file carries no f64 noise.
fn um(a: P3) -> P3 {
    a.map(|v| (v * 1000.0).round() / 1000.0)
}

/// A small deterministic hash in [0, 1).
fn hash(k: u64) -> f64 {
    let mut x = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

// --- The canes ---------------------------------------------------------------------------------------------

/// 1 at the crown, falling to 0 a quarter turn either side.
fn crown_share(theta_deg: f64) -> f64 {
    (theta_deg - CROWN_DEG).to_radians().cos().max(0.0).powi(2)
}

/// Cane `i`'s phase round the twine at `theta_deg`: cane 0 on the high side (+z) at the crown, cane 1 on the low.
fn phase(i: usize, theta_deg: f64) -> f64 {
    let from_crown = (theta_deg - CROWN_DEG).to_radians();
    WAVES * from_crown + FRAC_PI_2 + i as f64 * PI + WANDER[i] * from_crown.sin() * (2.0 * from_crown).cos().abs()
}

/// The twine's radial and across half-sizes at `theta_deg`.
fn twine(theta_deg: f64) -> (f64, f64) {
    (TWINE_RADIAL_MM, TWINE_ACROSS_MM + TWINE_CROWN_OPEN_MM * crown_share(theta_deg))
}

/// Cane `i`'s centre at `theta_deg`: (radius from the finger's axis, z along it).
fn cane_at(i: usize, theta_deg: f64) -> (f64, f64) {
    let (a, b) = twine(theta_deg);
    let p = phase(i, theta_deg);
    (TWINE_R_MM + a * p.cos(), b * p.sin())
}

/// A cane's radius at `theta_deg`: thicker at the crown, thinning toward the palm.
fn cane_r(theta_deg: f64) -> f64 {
    let c = 0.5 + 0.5 * (theta_deg - CROWN_DEG).to_radians().cos();
    CANE_PALM_MM + (CANE_CROWN_MM - CANE_PALM_MM) * c * c
}

fn world(theta_deg: f64, (r, z): (f64, f64)) -> P3 {
    let (s, c) = theta_deg.to_radians().sin_cos();
    [r * c, r * s, z]
}

fn cane_point(i: usize, theta_deg: f64) -> P3 {
    world(theta_deg, cane_at(i, theta_deg))
}

/// Cane `i`'s unit tangent at `theta_deg`.
fn cane_tangent(i: usize, theta_deg: f64) -> P3 {
    let h = 0.05;
    unit(sub(cane_point(i, theta_deg + h), cane_point(i, theta_deg - h)))
}

/// Cane `i`'s outward direction at `theta_deg`: away from the twine's axis, squared to the cane.
fn cane_normal(i: usize, theta_deg: f64) -> P3 {
    let (a, b) = twine(theta_deg);
    let p = phase(i, theta_deg);
    let (n_r, n_z) = (p.cos() / a, p.sin() / b);
    let (s, c) = theta_deg.to_radians().sin_cos();
    let n = unit([n_r * c, n_r * s, n_z]);
    let t = cane_tangent(i, theta_deg);
    unit(sub(n, mul(t, dot(n, t))))
}

/// A closed polygon sketch on `plane`: `pts` in its x, y.
fn poly_on(plane: Workplane, name: &str, pts: &[[f64; 2]]) -> Sketch {
    let mut s = Sketch { plane, ..Sketch::default() };
    s.name = name.into();
    let ids: Vec<_> = pts.iter().map(|p| s.point([(p[0] * 1e4).round() / 1e4, (p[1] * 1e4).round() / 1e4])).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

fn polygon(name: &str, pts: &[[f64; 2]]) -> Sketch {
    poly_on(Workplane::default(), name, pts)
}

/// Cane `i`: one closed twisted sweep through a smooth path, its five-lobed section turning `CANE_TURNS` times
/// round the ring and swelling from palm to crown by a scale law.
fn cane_op(i: usize) -> Operation {
    let start = CROWN_DEG + 180.0 + 9.0 * i as f64;
    let points: Vec<P3> = (0..STATIONS).map(|k| um(cane_point(i, start + 360.0 * k as f64 / STATIONS as f64))).collect();
    let lobed: Vec<[f64; 2]> = (0..30)
        .map(|k| {
            let t = TAU * k as f64 / 30.0;
            let r = CANE_PALM_MM * (1.0 + CANE_LOBE * (5.0 * t).cos());
            [r * t.cos(), r * t.sin()]
        })
        .collect();
    let scale: Vec<[f64; 2]> = (0..=24)
        .map(|k| {
            let share = k as f64 / 24.0;
            [share, ((cane_r(start + 360.0 * share) / CANE_PALM_MM) * 1e4).round() / 1e4]
        })
        .collect();
    Operation::Twist { sketch: polygon("Cane section", &lobed).into(), path: TwistPath::Points { points, smooth: true }, degrees: 360.0 * CANE_TURNS, end_scale: 1.0, scale, closed: true }
}

/// The world frame `(x, y, z, origin)` as a ring placement at `theta_deg`, solved for its leans and spin.
fn placed_as(d: &RingDesign, theta_deg: f64, origin: P3, x: P3, z: P3) -> Placement {
    let (s, c) = theta_deg.to_radians().sin_cos();
    // The seat's frame at theta: x along -Z, y round the ring, z radial.
    let seat = [[0.0, 0.0, -1.0], [-s, c, 0.0], [c, s, 0.0]];
    let y = cross(z, x);
    // The lean in seat coordinates, columns the wanted axes: m = Rz(spin) Ry(cant) Rx(tilt).
    let col = |v: P3| [dot(v, seat[0]), dot(v, seat[1]), dot(v, seat[2])];
    let (cx, cy, cz) = (col(x), col(y), col(z));
    let m = [[cx[0], cy[0], cz[0]], [cx[1], cy[1], cz[1]], [cx[2], cy[2], cz[2]]];
    let cant = (-m[2][0]).clamp(-1.0, 1.0).asin();
    let tilt = m[2][1].atan2(m[2][2]);
    let spin = m[1][0].atan2(m[0][0]);
    let r = origin[0] * c + origin[1] * s;
    let q = |v: f64| (v * 1e4).round() / 1e4;
    Placement::Ring { theta_deg: q(theta_deg), across_mm: q(origin[2]), height_mm: q(r - seat_r(d)), spin_deg: q(spin.to_degrees()), tilt_deg: q(tilt.to_degrees()), cant_deg: q(cant.to_degrees()), level: false }
}

/// A part standing on cane `i` at `theta_deg`: its foot sunk `sink` mm into the cane, its z along the cane's
/// outward normal, its y along the cane, `flip`ped to hook back the other way.
fn on_cane(d: &RingDesign, i: usize, theta_deg: f64, sink: f64, flip: bool) -> Component {
    on_cane_n(d, i, theta_deg, sink, flip, cane_normal(i, theta_deg))
}

/// [`on_cane`] standing along `n` instead of the cane's outward normal.
fn on_cane_n(d: &RingDesign, i: usize, theta_deg: f64, sink: f64, flip: bool, n: P3) -> Component {
    let t = cane_tangent(i, theta_deg);
    let t = if flip { mul(t, -1.0) } else { t };
    let foot = add3(cane_point(i, theta_deg), mul(n, cane_r(theta_deg) - sink));
    let x = cross(t, n);
    let mut comp = Component::default();
    comp.placement = placed_as(d, theta_deg, foot, x, n);
    comp.attach = Attach::Join;
    comp.stage = Stage::Cast;
    comp.blend_mm = THORN_BLEND_MM;
    comp
}
/// The seam bead where each prickle grows out of its cane (C-V4).
const THORN_BLEND_MM: f64 = 0.2;

// --- Prickles --------------------------------------------------------------------------------------------

/// A prickle's centreline in the part's y-z plane: `radial` mm straight out, then an arc of `bend_r` turning
/// `bend_deg` toward +y (along the cane).
fn hook_path(radial: f64, bend_r: f64, bend_deg: f64) -> Sketch {
    let mut s = Sketch { plane: Workplane { x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0], ..Default::default() }, ..Sketch::default() };
    let a = s.point([0.0, 0.0]);
    let b = s.point([0.0, radial]);
    s.entity(Geometry::Line { a, b });
    let centre = s.point([bend_r, radial]);
    let t = (180.0 - bend_deg).to_radians();
    let start = s.point([bend_r + bend_r * t.cos(), radial + bend_r * t.sin()]);
    s.entity(Geometry::Arc { center: centre, start, end: b });
    s
}

/// [`hook_path`] as stations in the part's frame (y along the cane, z out): straight up, then round the hook.
fn hook_points(radial: f64, bend_r: f64, bend_deg: f64) -> TwistPath {
    let r = |v: f64| (v * 1e4).round() / 1e4;
    let mut points = vec![[0.0, 0.0, 0.0]];
    for k in 0..=4 {
        let a = bend_deg.to_radians() * k as f64 / 4.0;
        points.push([0.0, r(bend_r * (1.0 - a.cos())), r(radial + bend_r * a.sin())]);
    }
    TwistPath::Points { points, smooth: true }
}

/// A rose prickle: a broad, flattened foot long along the cane, a short rise and a hard hook back down it.
#[derive(Clone, Copy)]
struct Hook {
    along: f64,
    across: f64,
    radial: f64,
    bend_r: f64,
    bend_deg: f64,
}

const THORN: Hook = Hook { along: 1.35, across: 0.8, radial: 0.45, bend_r: 0.85, bend_deg: 75.0 };
/// Each prickle's tip across, at least.
const THORN_TIP_MM: f64 = 0.26;

/// The prickle's foot: an ellipse `a` mm along the cane and `b` mm across it.
fn ellipse(a: f64, b: f64) -> Sketch {
    let pts: Vec<[f64; 2]> = (0..18).map(|k| {
        let t = TAU * k as f64 / 18.0;
        [0.5 * b * t.cos(), 0.5 * a * t.sin()]
    }).collect();
    polygon("Prickle foot", &pts)
}

/// A prickle `k` times the crown's size, tapering by a law that keeps its foot broad and runs fast to the point.
/// The prickle `k` times the crown's size swept from the shared foot sketch `foot` (drawn at size 1): the foot
/// scales by the law's first knot, so one sketch serves every prickle.
fn thorn(foot: u64, k: f64) -> Operation {
    let h = THORN;
    let r = |v: f64| (v * 1e4).round() / 1e4;
    let tip = (THORN_TIP_MM / (h.across * k)).max(0.15);
    let scale = vec![[0.0, r(k)], [0.3, r(0.7 * k)], [0.7, r(0.48 * k)], [1.0, r(tip * k)]];
    Operation::Twist { sketch: cad::Profile::Feature { feature: foot }, path: hook_points(h.radial * k, h.bend_r * k, h.bend_deg), degrees: 0.0, end_scale: r(tip * k), scale, closed: false }
}

/// Where a prickle stands: cane, angle, size and which way it hooks.
#[derive(Clone, Copy)]
struct ThornAt {
    cane: usize,
    theta: f64,
    scale: f64,
    flip: bool,
    /// Where the cane runs on the inside of the twine, the prickle stands out of the band's side instead.
    side: bool,
}

/// The direction a prickle stands on cane `i` at `theta_deg`: the cane's outward normal, or, where that faces the
/// finger, out of the band's side across the finger.
fn prickle_normal(i: usize, theta_deg: f64, side: bool) -> P3 {
    if !side {
        return cane_normal(i, theta_deg);
    }
    let z = cane_at(i, theta_deg).1;
    let (s, c) = theta_deg.to_radians().sin_cos();
    let out = unit(add3([0.0, 0.0, z.signum()], mul([c, s, 0.0], 0.35)));
    let t = cane_tangent(i, theta_deg);
    unit(sub(out, mul(t, dot(out, t))))
}

/// The prickles: along each cane at an uneven pitch, the crown's 1.4 times the palm's, every one on a cane hooking
/// back the same way; none facing the finger, none under the leaves and hip.
fn thorn_sites() -> Vec<ThornAt> {
    let mut out = Vec::new();
    for i in 0..CANES {
        for k in 0..THORNS_PER_CANE {
            let seed = (i * 100 + k) as u64;
            let theta = (11.0 + 13.0 * i as f64 + 360.0 * (k as f64 + 0.35 * (hash(seed) - 0.5)) / THORNS_PER_CANE as f64).rem_euclid(360.0);
            let n0 = cane_normal(i, theta);
            let (s, c) = theta.to_radians().sin_cos();
            let out_r = n0[0] * c + n0[1] * s;
            let side = out_r < -0.3;
            let n = prickle_normal(i, theta, side);
            let from_crown = ((theta - CROWN_DEG + 540.0).rem_euclid(360.0) - 180.0).abs();
            // Clear of the crossings, where a prickle would run into the other cane.
            let (me, other) = (cane_at(i, theta), cane_at(1 - i, theta));
            let apart = (me.0 - other.0).hypot(me.1 - other.1);
            let crown = 0.5 + 0.5 * (theta - CROWN_DEG).to_radians().cos();
            let scale = 0.92 + 0.36 * crown + 0.05 * (hash(seed + 7) - 0.5);
            // The whole prickle, foot to hooked tip, clear of the other cane.
            let t = cane_tangent(i, theta);
            let foot = add3(cane_point(i, theta), mul(n, cane_r(theta)));
            let reach = |p: P3| (0..=40).map(|j| { let a = theta - 20.0 + j as f64; dot(sub(p, cane_point(1 - i, a)), sub(p, cane_point(1 - i, a))).sqrt() - cane_r(a) }).fold(f64::MAX, f64::min);
            let hook = if i == 1 { -1.0 } else { 1.0 };
            let clear = [0.0, 0.5, 1.0].iter().all(|&f| reach(add3(foot, add3(mul(n, 1.1 * scale * f), mul(t, hook * 1.2 * scale * f * f)))) > 0.22)
                && reach(add3(foot, mul(t, 1.4 * scale))) > 0.2
                && reach(add3(foot, mul(t, -1.4 * scale))) > 0.2;
            let by_shoot = SHOOTS.iter().any(|&(t, c, _)| c == i && ((t - theta + 540.0).rem_euclid(360.0) - 180.0).abs() < 12.0);
            if from_crown < 66.0 || apart < 1.6 || !clear || by_shoot {
                if std::env::var("SENTIS_SITES").is_ok() {
                    eprintln!("cane {i} at {theta:.0}: out {out_r:.2} crown {from_crown:.0} apart {apart:.2} clear {clear} shoot {by_shoot}");
                }
                continue;
            }
            if out.iter().any(|o: &ThornAt| o.cane == i && ((o.theta - theta + 540.0).rem_euclid(360.0) - 180.0).abs() < 7.0) {
                continue;
            }
            out.push(ThornAt { cane: i, theta, scale, flip: i == 1, side });
        }
    }
    out
}

// --- Stored meshes ---------------------------------------------------------------------------------------

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    let v: Vec<P3> = solid.v.iter().map(|p| um(*p)).collect();
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "vepres_sentis".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

/// `s` turned outward if its signed volume says it was built inside out.
fn outward(mut s: csg::Solid) -> csg::Solid {
    let vol: f64 = s.f.iter().map(|f| {
        let (a, b, c) = (s.v[f[0] as usize], s.v[f[1] as usize], s.v[f[2] as usize]);
        dot(a, cross(b, c))
    }).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// A solid lofted through closed `loops` (each the same count, in order) and capped by an apex at each end.
fn lofted(loops: &[Vec<P3>], start: P3, end: P3) -> csg::Solid {
    let n = loops[0].len();
    let mut s = csg::Solid::default();
    s.v.push(start);
    s.v.push(end);
    for l in loops {
        s.v.extend(l.iter().copied());
    }
    let at = |i: usize, j: usize| (2 + i * n + j % n) as u32;
    let m = loops.len();
    for j in 0..n {
        s.f.push([0, at(0, j + 1), at(0, j)]);
        s.f.push([1, at(m - 1, j), at(m - 1, j + 1)]);
    }
    for i in 0..m - 1 {
        for j in 0..n {
            s.f.push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
            s.f.push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
        }
    }
    outward(s)
}

// --- The rose leaves -------------------------------------------------------------------------------------

/// A rose leaflet in its leaf's plan: from `origin` along heading `alpha` (radians from +y toward +x), `len` long
/// and `wide` across; ovate, its finely serrate margin's teeth leaning forward, its top domed up from the margin to
/// a sunk midrib, lifting `lift` radians as it runs out; its foot sunk `sink` mm.
fn leaflet_solid(origin: [f64; 2], alpha: f64, len: f64, wide: f64, lift: f64, sink: f64) -> csg::Solid {
    let steps = 48usize;
    let d = [alpha.sin(), alpha.cos()];
    let perp = [d[1], -d[0]];
    // An ovate blade with an acuminate tip, never narrower than its rounded margin where it closes.
    let smooth = |u: f64| (0.5 * wide * (PI * u.powf(0.72)).sin().powf(1.15)).max(0.55);
    let jitter = 0.35 * (hash((len * 1000.0 + wide * 37.0) as u64) - 0.5);
    let half = |u: f64| {
        // Forward-hooked teeth, a little uneven, fading out toward the base and the tip.
        let t = (u * LEAF_TEETH as f64 + jitter * (u * 7.0).sin()).rem_euclid(1.0).powf(0.7);
        let fade = ((u - 0.18) / 0.3).clamp(0.0, 1.0) * ((0.93 - u) / 0.1).clamp(0.0, 1.0);
        smooth(u) - LEAF_TOOTH_MM * fade * (PI * t).sin().powi(2)
    };
    // Across the blade from the midvein (0) to where the rounded margin starts (1): close in at the vein.
    const ACROSS: [f64; 5] = [0.0, 0.12, 0.4, 0.75, 1.0];
    let (u0, u1) = (0.1, 0.86);
    let centre = |u: f64| -> P3 { [origin[0] + d[0] * u * len, origin[1] + d[1] * u * len, u * len * lift.tan()] };
    let t = LEAF_EDGE_MM;
    let loops: Vec<Vec<P3>> = (0..=steps).map(|i| {
        let u = u0 + (u1 - u0) * i as f64 / steps as f64;
        let c = centre(u);
        let round = 0.5 * (t - LEAF_BEVEL_MM);
        let (w, ws) = ((half(u) - round).max(0.12), (smooth(u) - round).max(0.12));
        let under = sink * (1.0 - u / 0.25).max(0.0);
        // The blade cups: both faces rise toward the margin; the bottom follows the top, so it is a curved plate.
        let cup = |x: f64| LEAF_CUP_MM * (x / ws.max(0.5)).min(1.0).powi(2);
        let top = |x: f64| {
            let rib = LEAF_RIB_MM * (1.0 - (x / 0.4).min(1.0)).powi(2) * (ws / 0.6).min(1.0);
            let vein = LEAF_VEIN_MM * (1.0 - (x / 0.125).min(1.0).powi(2)) * (ws / 0.5).min(1.0) * (u < 0.92) as u8 as f64;
            let bevel = LEAF_BEVEL_MM * ((x - (w - 0.3).max(0.0)) / 0.3).clamp(0.0, 1.0);
            t + cup(x) + rib - vein - bevel
        };
        let at = |v: f64, x: f64, z: f64| -> P3 { [c[0] + perp[0] * v * x, c[1] + perp[1] * v * x, c[2] + z] };
        // The margin, a half-round bead of the bevelled thickness.
        let edge_top = top(w);
        let bead = |v: f64| -> Vec<P3> {
            let (z0, z1) = (cup(w) - under * 0.0, edge_top);
            let (mid, r) = (0.5 * (z0 + z1), 0.5 * (z1 - z0));
            let mut b: Vec<P3> = [62f64, 31.0, 0.0, -31.0, -62.0].iter().map(|deg| { let a = deg.to_radians(); at(v, w + r * a.cos(), mid + r * a.sin()) }).collect();
            if v < 0.0 {
                b.reverse();
            }
            b
        };
        let mut l = Vec::with_capacity(4 * ACROSS.len() + 10);
        l.extend(bead(-1.0));
        for &f in ACROSS.iter().rev() {
            l.push(at(-1.0, f * w, top(f * w)));
        }
        for &f in ACROSS.iter().skip(1) {
            l.push(at(1.0, f * w, top(f * w)));
        }
        l.extend(bead(1.0));
        for &f in ACROSS.iter().rev() {
            l.push(at(1.0, f * w, cup(f * w) - under));
        }
        for &f in ACROSS.iter().skip(1) {
            l.push(at(-1.0, f * w, cup(f * w) - under));
        }
        l
    }).collect();
    // Both ends rounded over: the last loop drawn in toward its centre in steps, so no end is a knife cone.
    let centroid = |l: &Vec<P3>| mul(l.iter().fold([0.0; 3], |a, p| add3(a, *p)), 1.0 / l.len() as f64);
    let cap = |l: &Vec<P3>, ahead: P3| -> Vec<Vec<P3>> {
        let c = centroid(l);
        [(0.93, 0.2), (0.75, 0.38), (0.48, 0.5), (0.18, 0.56)].iter().map(|&(k, f)| l.iter().map(|p| add3(add3(c, mul(sub(*p, c), k)), mul(ahead, f))).collect()).collect()
    };
    let fwd = [d[0], d[1], 0.0];
    let mut all: Vec<Vec<P3>> = cap(&loops[0], mul(fwd, -1.0)).into_iter().rev().collect();
    all.extend(loops.iter().cloned());
    all.extend(cap(&loops[loops.len() - 1], fwd));
    let (a, b) = (centroid(&all[0]), centroid(&all[all.len() - 1]));
    lofted(&all, add3(a, mul(fwd, -0.05)), add3(b, mul(fwd, 0.05)))
}

/// The blade's thickness, its bevel at the margin, the rib's rise and the midvein's depth, the cup's rise at the margin.
const LEAF_BEVEL_MM: f64 = 0.1;
const LEAF_RIB_MM: f64 = 0.12;
const LEAF_VEIN_MM: f64 = 0.15;
const LEAF_CUP_MM: f64 = 0.2;
const LEAF_EDGE_MM: f64 = 1.06;
const LEAF_TEETH: usize = 8;
const LEAF_TOOTH_MM: f64 = 0.08;

/// A rose leaf: the rachis's foot at `theta_deg` on cane `cane`, its plan laid over the crown from `(theta0, z0)`
/// heading round the ring by `dir` (+1 or -1) and leaning `skew` radians across the finger.
struct Leaf {
    cane: usize,
    foot_deg: f64,
    theta0: f64,
    z0: f64,
    dir: f64,
    skew: f64,
}

/// The leaves either side of the hip, their terminal leaflets running away from it down the shoulders.
const LEAVES: [Leaf; 2] = [
    Leaf { cane: 0, foot_deg: CROWN_DEG - 9.0, theta0: CROWN_DEG - 15.0, z0: 0.0, dir: -1.0, skew: 0.0 },
    Leaf { cane: 1, foot_deg: CROWN_DEG + 9.0, theta0: CROWN_DEG + 15.0, z0: -0.9, dir: 1.0, skew: 0.12 },
];
/// The rachis: its length to the terminal leaflet, its radius; the leaflet pairs' stations, angles, sizes.
const RACHIS_MM: f64 = 5.6;
const RACHIS_R_MM: f64 = 0.5;
/// The rachis's height in the leaf's plan: low, so it crosses the leaflets' undersides instead of grazing them.
const RACHIS_Z_MM: f64 = 0.05;
const PAIRS: [(f64, f64, f64, f64); 2] = [(0.9, 76.0, 3.8, 2.6), (3.2, 67.0, 4.0, 2.7)];
const TERMINAL: (f64, f64) = (4.4, 3.0);
/// How far the leaf's underside sits under the highest cane beneath it.
const LEAF_SINK_MM: f64 = 0.5;

/// The top of the canes at `theta_deg`, `z` across the finger.
fn canes_top(theta_deg: f64, z: f64) -> f64 {
    (0..CANES)
        .filter_map(|i| {
            let (r, zc) = cane_at(i, theta_deg);
            let rc = cane_r(theta_deg);
            let dz = z - zc;
            (dz.abs() < rc).then(|| r + (rc * rc - dz * dz).sqrt())
        })
        .fold(TWINE_R_MM, f64::max)
}

/// `a` to the corner `b` to `c` with the corner rounded on radius `r`: the straight run in, the arc, sampled.
fn filleted(a: P3, b: P3, c: P3, r: f64) -> Vec<P3> {
    let (u, v) = (unit(sub(a, b)), unit(sub(c, b)));
    let turn = dot(u, v).clamp(-1.0, 1.0).acos();
    let d = (r / (0.5 * turn).tan()).min(0.45 * dot(sub(a, b), sub(a, b)).sqrt()).min(0.9 * dot(sub(c, b), sub(c, b)).sqrt());
    let (p, q) = (add3(b, mul(u, d)), add3(b, mul(v, d)));
    let mut out = vec![a];
    // A quadratic through the corner between the two tangent points is close to the arc and as smooth.
    out.extend((0..=6).map(|k| { let t = k as f64 / 6.0; add3(add3(mul(p, (1.0 - t) * (1.0 - t)), mul(b, 2.0 * t * (1.0 - t))), mul(q, t * t)) }));
    out
}

/// The shoots: angle, cane, which way they lean; their radius and rise.
const SHOOTS: [(f64, usize, bool); 2] = [(CROWN_DEG - 80.0, 0, true), (CROWN_DEG + 80.0, 1, false)];
const SHOOT_R_MM: f64 = 0.6;
const SHOOT_RISE_MM: f64 = 1.4;

/// Leaf nodes: per cane at an uneven pitch, clear of the crown's leaves, the crossings and the prickles.
fn node_sites(prickles: &[ThornAt]) -> Vec<(usize, usize, f64)> {
    let mut out = Vec::new();
    for i in 0..CANES {
        let mut k = 0;
        for j in 0..48 {
            // Shoulders first, then the palm: the nodes weight toward the crown's flanks.
            let order = [35.0, -35.0, 55.0, -55.0, 75.0, -75.0, 100.0, -100.0, 130.0, -130.0, 160.0, -160.0];
            let Some(&off) = order.get(j / 4) else { break };
            let theta = (CROWN_DEG + off + 3.0 * (j % 4) as f64 + 5.0 * i as f64).rem_euclid(360.0);
            let from_crown = ((theta - CROWN_DEG + 540.0).rem_euclid(360.0) - 180.0).abs();
            let (me, other) = (cane_at(i, theta), cane_at(1 - i, theta));
            let apart = (me.0 - other.0).hypot(me.1 - other.1);
            let near = prickles.iter().any(|p| p.cane == i && ((p.theta - theta + 540.0).rem_euclid(360.0) - 180.0).abs() < 5.0);
            let spaced = out.iter().all(|&(c, _, t): &(usize, usize, f64)| c != i || ((t - theta + 540.0).rem_euclid(360.0) - 180.0).abs() > 34.0);
            if from_crown > 26.0 && from_crown < 175.0 && apart > 2.4 && !near && spaced && k < NODES_PER_CANE {
                out.push((i, k, theta));
                k += 1;
            }
        }
    }
    out
}
const NODES_PER_CANE: usize = 4;

/// A node: a spindle round cane `i`'s centreline, swelling to a collar at its middle, as a stored solid.
fn node_solid(i: usize, theta: f64) -> csg::Solid {
    let r_mid = cane_at(i, theta).0;
    let loops: Vec<Vec<P3>> = [(-1.2, 0.0), (-0.7, 0.35), (-0.3, 0.8), (0.0, 1.0), (0.3, 0.7), (0.75, 0.3), (1.2, 0.0)]
        .iter()
        .map(|&(f, swell)| {
            let at = theta + (f / r_mid).to_degrees();
            let (n, t) = (cane_normal(i, at), cane_tangent(i, at));
            let b = cross(t, n);
            let c = cane_point(i, at);
            let r = cane_r(at) * (1.0 - CANE_LOBE) - 0.03 + (0.03 + CANE_LOBE * cane_r(at) + NODE_SWELL_MM) * swell;
            (0..16).map(|k| { let a = TAU * k as f64 / 16.0; add3(c, add3(mul(n, r * a.cos()), mul(b, r * a.sin()))) }).collect()
        })
        .collect();
    let (a, b) = (cane_point(i, theta - (1.3 / r_mid).to_degrees()), cane_point(i, theta + (1.3 / r_mid).to_degrees()));
    lofted(&loops, a, b)
}

/// A stipule: a small wing out of the node's collar on `side` (+1 or -1 across the cane), leaning back along it.
fn stipule_solid(i: usize, theta: f64, side: f64) -> csg::Solid {
    let (n, t) = (cane_normal(i, theta), cane_tangent(i, theta));
    let b = mul(cross(t, n), side);
    let c = cane_point(i, theta);
    let root = cane_r(theta) + NODE_SWELL_MM - 0.25;
    // Out of the collar and back along the cane, broad and flat, rounded at its end.
    let path: Vec<P3> = (0..=6).map(|k| { let f = k as f64 / 6.0; add3(c, add3(mul(b, root + 0.95 * f), add3(mul(t, -0.55 * f * f), mul(n, 0.25 * f)))) }).collect();
    let m = path.len();
    let loops: Vec<Vec<P3>> = (0..m)
        .map(|k| {
            let tan = unit(sub(path[(k + 1).min(m - 1)], path[k.saturating_sub(1)]));
            let side_w = unit(cross(tan, n));
            let up = unit(cross(side_w, tan));
            let f = k as f64 / (m - 1) as f64;
            let (hw, ht) = (0.56 * (1.0 - 0.22 * f * f), 0.54);
            (0..12).map(|j| { let a = TAU * j as f64 / 12.0; add3(path[k], add3(mul(side_w, hw * a.cos()), mul(up, ht * a.sin()))) }).collect()
        })
        .collect();
    let t0 = unit(sub(path[1], path[0]));
    let t1 = unit(sub(path[m - 1], path[m - 2]));
    lofted(&loops, sub(path[0], mul(t0, 0.1)), add3(path[m - 1], mul(t1, 0.35)))
}
const NODE_SWELL_MM: f64 = 0.22;

/// The hip's receptacle and cup as one urn: swollen under the stone, tapering below into the stalk, its wall
/// rising round the girdle, bellying out a little and turning in over the dome's foot with a rounded rim.
fn urn_solid(h: &HipFrame) -> csg::Solid {
    let gem = hip_gem();
    // The stone lies long round the ring (the frame's v), its width along the finger (u).
    let (a, b) = (0.5 * gem.w_mm, 0.5 * gem.l_mm);
    let gap = 0.09;
    let wall = URN_WALL_MM;
    // (height off the girdle, half-length, half-width) up the outside, over the rim, down the inside to the seat.
    let rim = 0.45;
    let outside = [(-3.25, 0.45, 0.4), (-3.0, 1.15, 0.95), (-2.45, 2.2, 1.65), (-1.7, 3.15, 2.35), (-1.0, a + wall + 0.35, b + wall + 0.3), (-0.4, a + wall + 0.3, b + wall + 0.25), (0.05, a + wall + 0.15, b + wall + 0.12), (rim - 0.12, a + wall + 0.02, b + wall)];
    let over = [(rim + 0.05, a + gap + 0.75 * wall, b + gap + 0.75 * wall), (rim + 0.12, a + gap + 0.45 * wall, b + gap + 0.45 * wall), (rim + 0.05, a + gap + 0.15 * wall, b + gap + 0.15 * wall)];
    let inside = [(rim - 0.12, a + gap, b + gap), (0.15, a + gap + 0.02, b + gap + 0.02), (-0.14, a + gap + 0.02, b + gap + 0.02), (-0.14, 0.65 * a, 0.65 * b), (-0.14, 0.3 * a, 0.3 * b)];
    let n = 32;
    let loops: Vec<Vec<P3>> = outside.iter().chain(over.iter()).chain(inside.iter())
        .map(|&(z, au, av)| (0..n).map(|k| { let t = TAU * k as f64 / n as f64; h.at([au * t.cos(), av * t.sin(), z]) }).collect())
        .collect();
    let mut sol = lofted(&loops, h.at([0.0, 0.0, -3.3]), h.at([0.0, 0.0, -0.14]));
    // `lofted` turns it outward by volume; the cup's floor apex closes the inside.
    sol = outward(sol);
    sol
}
/// The cup's wall, mm.
const URN_WALL_MM: f64 = 0.88;

// --- The hip -----------------------------------------------------------------------------------------------

/// The hip's frame: girdle centre, long axis (toward the calyx), across, and up out of the ring.
struct HipFrame {
    g: P3,
    u: P3,
    v: P3,
    w: P3,
}

/// The hip's girdle over the seat, the long axis's turn from along the finger toward round the ring, and its lift.
const HIP_H_MM: f64 = 2.3;
const HIP_TURN_DEG: f64 = 0.0;

/// The hip's placement: at the crown, its long axis turned `HIP_TURN_DEG` off the finger's axis, girdle level.
fn hip_placement(d: &RingDesign) -> (Placement, HipFrame) {
    let r = seat_r(d) + HIP_H_MM;
    let g = world(CROWN_DEG, (r, 0.0));
    let w = world(CROWN_DEG, (1.0, 0.0));
    let ring = world(CROWN_DEG + 90.0, (1.0, 0.0));
    let t = HIP_TURN_DEG.to_radians();
    let u = unit(add3(mul([0.0, 0.0, 1.0], t.cos()), mul(ring, t.sin())));
    let v = cross(w, u);
    // An oval stone's length runs along its frame's x.
    (placed_as(d, CROWN_DEG, g, u, w), HipFrame { g, u, v, w })
}

impl HipFrame {
    fn at(&self, p: P3) -> P3 {
        add3(self.g, add3(mul(self.u, p[0]), add3(mul(self.v, p[1]), mul(self.w, p[2]))))
    }
}



/// The calyx: five sepal claws gripping the hip's dome.
const SEPALS: usize = 5;
/// The calyx's turn up off the hip's axis.
const CALYX_LIFT_DEG: f64 = 22.0;
const STALK_R_MM: f64 = 0.46;

/// A sepal at angle `alpha` round the hip's long axis, springing from `hub`: out from the boss, then curling back
/// over the end of the hip. Returns its body (0.8 mm and more) and its point (the last millimetre) as two solids.
fn sepal_solids(h: &HipFrame, hub: P3, alpha: f64) -> (csg::Solid, csg::Solid) {
    let e = add3(mul(h.v, alpha.cos()), mul(h.w, alpha.sin()));
    // Upward sepals arch higher to clear the dome.
    let reach = 2.0;
    let path: Vec<P3> = [(0.0, 0.0), (0.7, 0.25), (1.35, 0.2), (reach - 0.1, -0.15), (reach + 0.05, -0.55), (reach, -0.9), (reach - 0.2, -1.15)]
        .iter()
        .map(|&(o, b)| add3(hub, add3(mul(e, o), mul(h.u, b))))
        .collect();
    // Dense stations along the path (quadratic through each middle point), and the section's size along them.
    let mut pts: Vec<P3> = Vec::new();
    for k in 0..path.len() - 1 {
        for j in 0..4 {
            let t = j as f64 / 4.0;
            pts.push(add3(mul(path[k], 1.0 - t), mul(path[k + 1], t)));
        }
    }
    pts.push(*path.last().unwrap());
    // Light smoothing so the polyline turns as a curve.
    for _ in 0..3 {
        let n = pts.len();
        pts = (0..n).map(|i| if i == 0 || i == n - 1 { pts[i] } else { mul(add3(add3(pts[i - 1], mul(pts[i], 2.0)), pts[i + 1]), 0.25) }).collect();
    }
    let m = pts.len();
    let mut cum = vec![0.0];
    for i in 1..m {
        cum.push(cum[i - 1] + dot(sub(pts[i], pts[i - 1]), sub(pts[i], pts[i - 1])).sqrt());
    }
    let total = cum[m - 1];
    let ring = |i: usize| -> Vec<P3> {
        let tan = unit(sub(pts[(i + 1).min(m - 1)], pts[i.saturating_sub(1)]));
        // Width across the sepal's own plane (that of `e` and the hip's axis), so the frame never folds.
        let side = unit(cross(e, h.u));
        let nrm = unit(cross(side, tan));
        let left = total - cum[i];
        // Broad and flat as a sepal is; the full 0.8 mm wall until the last millimetre, then a point.
        let f = (left / 1.0).min(1.0);
        let (hw, ht) = (0.5 * (SEPAL_W_MM * f + 0.24 * (1.0 - f)), 0.5 * (SEPAL_T_MM * f + 0.22 * (1.0 - f)));
        (0..12).map(|j| { let a = TAU * j as f64 / 12.0; add3(pts[i], add3(mul(side, hw * a.cos()), mul(nrm, ht * a.sin()))) }).collect()
    };
    let split = (0..m).find(|&i| total - cum[i] <= 1.0).unwrap_or(m - 2).max(2);
    let body_loops: Vec<Vec<P3>> = (0..=split).map(ring).collect();
    let tip_loops: Vec<Vec<P3>> = (split.saturating_sub(1)..m).map(ring).collect();
    let t0 = unit(sub(pts[1], pts[0]));
    let tb = unit(sub(pts[split], pts[split - 1]));
    let t1 = unit(sub(pts[m - 1], pts[m - 2]));
    (
        lofted(&body_loops, sub(pts[0], mul(t0, 0.1)), add3(pts[split], mul(tb, 0.1))),
        lofted(&tip_loops, sub(pts[split - 1], mul(tb, 0.1)), add3(pts[m - 1], mul(t1, 0.1))),
    )
}
/// A sepal's width and thickness along its body; the boss's diameter and how far past the stone's end it stands.
const SEPAL_W_MM: f64 = 1.0;
const SEPAL_T_MM: f64 = 0.88;

const BOSS_D_MM: f64 = 1.6;

// --- The document ------------------------------------------------------------------------------------------

struct Authored {
    design: RingDesign,
    thorns: usize,
    stone_id: u64,
}

fn author(bare: bool, unbeaded: &[String]) -> Result<Authored> {
    let mut d = ground();
    let mut doc = Document::default();
    let mut next = 0u64;
    let mut add = |doc: &mut Document, name: String, operation: Operation, component: Component| -> Result<u64> {
        next += 1;
        doc.append(Feature { id: next, name, enabled: true, operation, component })?;
        Ok(next)
    };
    let free = Component { attach: Attach::Join, stage: Stage::Cast, role: ComponentRole::Shank, ..Component::default() };
    for i in 0..CANES {
        add(&mut doc, format!("Briar cane {}", i + 1), cane_op(i), free.clone())?;
    }
    if bare {
        d.cad = Some(doc);
        return Ok(Authored { design: d, thorns: 0, stone_id: 0 });
    }
    let sites = thorn_sites();
    let foot = add(&mut doc, "Prickle foot".into(), Operation::Sketch { sketch: ellipse(THORN.along, THORN.across) }, Component::default())?;
    for (k, t) in sites.iter().enumerate() {
        let name = format!("Prickle {} on cane {}", k + 1, t.cane + 1);
        if unbeaded.contains(&format!("omit:{name}")) {
            continue;
        }
        let mut at = on_cane_n(&d, t.cane, t.theta, THORN_SINK_MM * t.scale, t.flip, prickle_normal(t.cane, t.theta, t.side));
        if unbeaded.contains(&name) {
            at.blend_mm = 0.0;
        }
        add(&mut doc, name, thorn(foot, t.scale), at)?;
    }
    // Leaf nodes along each cane, each a swelling with a stipule collar.
    for (i, k, theta) in node_sites(&sites) {
        add(&mut doc, format!("Node {} on cane {}", k + 1, i + 1), stored_op(&node_solid(i, theta), "node", json!({"cane": i + 1}))?, free.clone())?;
        for side in [-1.0, 1.0] {
            let st = stipule_solid(i, theta, side);
            // A stipule that would reach into the finger hole is left off.
            if st.v.iter().any(|p| p[0].hypot(p[1]) < d.inner_radius_mm() + 0.15) {
                continue;
            }
            add(&mut doc, format!("Node {} on cane {}, stipule", k + 1, i + 1), stored_op(&st, "stipule", json!({"cane": i + 1}))?, free.clone())?;
        }
    }
    // Two pruned side shoots at the shoulders, each ending in a square cut.
    for (k, &(theta, cane, flip)) in SHOOTS.iter().enumerate() {
        let (rise, bend_r, bend) = (SHOOT_RISE_MM, 3.0, 22f64.to_radians());
        let at = on_cane(&d, cane, theta, 0.4, flip);
        let at = Component { blend_mm: 0.0, ..at };
        add(&mut doc, format!("Side shoot {}", k + 1), Operation::Twist { sketch: Sketch::circle(SHOOT_R_MM).into(), path: hook_path(rise, bend_r, bend.to_degrees()).into(), degrees: 0.0, end_scale: 0.85, scale: Vec::new(), closed: false }, at.clone())?;
        // A live tip: the shoot swells into a bud and closes to a point, continuing from its end.
        let (sb, cb) = bend.sin_cos();
        let end = [0.0, bend_r * (1.0 - cb), rise + bend_r * sb];
        let dir = [0.0, sb, cb];
        let bud: Vec<[f64; 3]> = (0..=4).map(|j| { let f = 1.3 * j as f64 / 4.0; [0.0, end[1] + dir[1] * (f - 0.15), end[2] + dir[2] * (f - 0.15)] }).collect();
        let scale = vec![[0.0, 0.85], [0.45, 1.05], [0.8, 0.7], [1.0, 0.3]];
        add(&mut doc, format!("Side shoot {}, bud point", k + 1), Operation::Twist { sketch: Sketch::circle(SHOOT_R_MM).into(), path: TwistPath::Points { points: bud, smooth: true }, degrees: 0.0, end_scale: 0.3, scale, closed: false }, at)?;
    }
    // The hip: receptacle, stalk, calyx and stone, all in the hip's frame.
    let (stone_at, h) = hip_placement(&d);
    add(&mut doc, "Hip receptacle and cup".into(), stored_op(&urn_solid(&h), "hip urn", json!({"wall_mm": URN_WALL_MM}))?, free.clone())?;
    // The stalk: out of the receptacle's far end, down and round into the low cane.
    let into = cane_point(1, CROWN_DEG - 14.0);
    let p0 = h.at([-0.6, 0.0, -2.6]);
    let p1 = h.at([-2.4, 0.0, -2.8]);
    let stalk_pts: Vec<P3> = (0..=8).map(|k| { let t = k as f64 / 8.0; um(add3(add3(mul(p0, (1.0 - t) * (1.0 - t)), mul(p1, 2.0 * t * (1.0 - t))), mul(into, t * t))) }).collect();
    add(&mut doc, "Hip stalk".into(), Operation::Twist { sketch: Sketch::circle(STALK_R_MM).into(), path: TwistPath::Points { points: stalk_pts, smooth: true }, degrees: 0.0, end_scale: 1.0, scale: Vec::new(), closed: false }, free.clone())?;
    // The calyx at the hip's distal tip only: a boss past the stone's end and five sepals springing from it, curling
    // back over the end of the dome; each sepal a body at the 0.8 mm wall and a pointed tip.
    let hub = h.at([0.5 * HIP_W_MM + URN_WALL_MM + 0.75, 0.0, 0.45]);
    // The calyx turned up off the hip's axis, so its sepal tips show past the cup's far rim from the face.
    let lift = CALYX_LIFT_DEG.to_radians();
    let h = HipFrame { g: h.g, u: unit(add3(mul(h.u, lift.cos()), mul(h.w, lift.sin()))), v: h.v, w: unit(add3(mul(h.w, lift.cos()), mul(h.u, -lift.sin()))) };
    // The boss: a round knop on the hip's long axis, which the sepals' ring pattern turns about.
    let mut boss_at = free.clone();
    boss_at.placement = placed_as(&d, CROWN_DEG, um(hub), h.v, h.u);
    let boss = add(&mut doc, "Hip calyx, boss".into(), Operation::Sphere { radius_mm: 0.5 * BOSS_D_MM }, boss_at)?;
    let alpha = 0.5 * PI;
    let (body, tip) = sepal_solids(&h, hub, alpha);
    let s1 = add(&mut doc, "Hip calyx, sepal".into(), stored_op(&body, "sepal", json!({"of": SEPALS}))?, free.clone())?;
    let s2 = add(&mut doc, "Hip calyx, sepal point".into(), stored_op(&tip, "sepal point", json!({"of": SEPALS}))?, free.clone())?;
    let round = cad::PatternKind::About { part: boss, count: SEPALS as u32, span_deg: 360.0 };
    add(&mut doc, "Hip calyx, the sepal turned five times round the hip's axis".into(), Operation::Pattern { sources: cad::pattern::Sources(vec![s1]), kind: round.clone() }, free.clone())?;
    add(&mut doc, "Hip calyx, the sepal's tip turned with it: point".into(), Operation::Pattern { sources: cad::pattern::Sources(vec![s2]), kind: round }, free.clone())?;
    let stone = builders::stone_feature(0, hip_gem(), stone_at);
    let stone_id = add(&mut doc, stone.name, stone.operation, stone.component)?;
    let _ = stone_id;
    // The rose leaves: a rachis from the cane up over the crown, two pairs of leaflets and a terminal one.
    let mut leaf_parts = Vec::new();
    for (k, leaf) in LEAVES.iter().enumerate().take(1) {
        let reach_deg = (8.0 / TWINE_R_MM).to_degrees();
        let base = (0..=24)
            .flat_map(|i| (0..=14).map(move |j| (leaf.theta0 + leaf.dir * reach_deg * i as f64 / 24.0, leaf.z0 - 3.5 + 0.5 * j as f64)))
            .map(|(t, zz)| canes_top(t, zz))
            .fold(f64::MIN, f64::max)
            - LEAF_SINK_MM;
        // The plan: y round the ring the leaf's way, x across the finger, z out; bent round the ring.
        let (theta0, z0, dir) = (leaf.theta0, leaf.z0, leaf.dir);
        let to_world = move |p: &P3| -> P3 {
            let phi = theta0.to_radians() + dir * p[1] / base;
            let r = base + p[2];
            [r * phi.cos(), r * phi.sin(), z0 + dir * p[0]]
        };
        let head = leaf.skew;
        let rachis_dir = [head.sin(), head.cos()];
        let along = |s: f64| [rachis_dir[0] * s, rachis_dir[1] * s];
        let mut leaflets = Vec::new();
        for (j, &(s, ang, len, wide)) in PAIRS.iter().enumerate() {
            // One of each pair; its partner is its mirror across the band's mid-plane, where the rachis runs.
            let a = head + ang.to_radians();
            let lift = (15.0 + 3.0 * j as f64).to_radians();
            let o = along(s);
            let o = [o[0] + a.sin() * 0.5, o[1] + a.cos() * 0.5];
            leaflets.push((format!("pair {}", j + 1), leaflet_solid(o, a, len, wide, lift, 0.25 + 0.03 * j as f64)));
        }
        leaflets.push(("terminal".into(), leaflet_solid(along(RACHIS_MM - 1.0), head, TERMINAL.0, TERMINAL.1, 6f64.to_radians(), 0.3)));
        let mut pair_ids = Vec::new();
        for (name, solid) in leaflets {
            let placed = outward(csg::Solid { v: solid.v.iter().map(&to_world).collect(), f: solid.f });
            let id = add(&mut doc, format!("Rose leaf {}, {name} leaflet", k + 1), stored_op(&placed, "leaflet", json!({"leaf": k + 1, "leaflet": name}))?, free.clone())?;
            if name.starts_with("pair") {
                pair_ids.push(id);
            }
            leaf_parts.push(id);
        }
        leaf_parts.push(add(&mut doc, format!("Rose leaf {}, the paired leaflets mirrored across the rachis", k + 1), Operation::Pattern { sources: cad::pattern::Sources(pair_ids), kind: cad::PatternKind::Mirror { plane: cad::pattern::MirrorPlane::Band } }, free.clone())?);
        // The rachis: from inside the cane, up and over into the leaf's plane, along it to the terminal leaflet.
        let foot = cane_point(leaf.cane, leaf.foot_deg);
        let start = to_world(&[0.0, 0.0, RACHIS_Z_MM]);
        // One stalk: petiole straight up out of the cane, turning on a 1.3 mm radius into the rachis along the leaf.
        let run: Vec<P3> = (0..=8).map(|k| to_world(&[along(RACHIS_MM * k as f64 / 8.0)[0], along(RACHIS_MM * k as f64 / 8.0)[1], RACHIS_Z_MM])).collect();
        let mut points = filleted(foot, start, run[1], 1.3);
        points.extend(run[1..].iter().copied());
        let points: Vec<P3> = points.into_iter().map(um).collect();
        let mut section = Sketch::circle(RACHIS_R_MM);
        section.name = "Rachis".into();
        // It tapers away inside the terminal leaflet, so no cut end shows.
        let scale = vec![[0.0, 1.0], [0.82, 1.0], [1.0, 0.55]];
        leaf_parts.push(add(&mut doc, format!("Rose leaf {}, petiole and rachis", k + 1), Operation::Twist { sketch: section.into(), path: TwistPath::Points { points, smooth: true }, degrees: 0.0, end_scale: 0.55, scale, closed: false }, free.clone())?);
    }
    // The second leaf is the first turned half round the hip's own axis: the canes are twined with the same symmetry.
    add(&mut doc, "Rose leaf 2: the leaflets and rachis of leaf 1 turned round the hip".into(), Operation::Pattern { sources: cad::pattern::Sources(leaf_parts), kind: cad::PatternKind::About { part: stone_id, count: 2, span_deg: 360.0 } }, free.clone())?;
    let thorns = doc.features.iter().filter(|f| f.name.starts_with("Prickle ") && f.name.contains(" on cane")).count();
    d.cad = Some(doc);
    Ok(Authored { design: d, thorns, stone_id })
}

// --- Checks ---------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// The finished metal's separate shells: one, or something floats.
fn shells(m: &mesh::Mesh) -> usize {
    let mut parent: Vec<u32> = (0..m.vertices.len() as u32).collect();
    fn find(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for f in &m.faces {
        for k in 1..3 {
            let (a, b) = (find(&mut parent, f[0]), find(&mut parent, f[k]));
            parent[a as usize] = b;
        }
    }
    let mut roots = std::collections::HashSet::new();
    for f in &m.faces {
        roots.insert(find(&mut parent, f[0]));
    }
    roots.len()
}

/// Shells of positive volume (metal), and the volumes of any sealed voids (shells of negative volume) inside it.
fn metal_shells(m: &mesh::Mesh) -> (usize, Vec<f64>) {
    let mut parent: Vec<u32> = (0..m.vertices.len() as u32).collect();
    fn find(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for f in &m.faces {
        for k in 1..3 {
            let (a, b) = (find(&mut parent, f[0]), find(&mut parent, f[k]));
            parent[a as usize] = b;
        }
    }
    let mut vol: std::collections::HashMap<u32, f64> = Default::default();
    for f in &m.faces {
        let p = |i: u32| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] };
        *vol.entry(find(&mut parent, f[0])).or_default() += dot(p(f[0]), cross(p(f[1]), p(f[2]))) / 6.0;
    }
    let metal = vol.values().filter(|v| **v > 0.0).count();
    let voids = vol.values().filter(|v| **v <= 0.0).map(|v| (v.abs() * 1e6).round() / 1e6).collect::<Vec<_>>();
    let ok = voids.iter().all(|v| *v < 0.01);
    (if ok { metal } else { metal + voids.len() }, voids)
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
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
                None => csg::self_crossings(&csg::Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }),
            };
            (c.name.clone(), n)
        })
        .collect()
}

fn feature_status(built: &mesh::BuildResult) -> Vec<(String, String)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| {
            let s = match &r.status {
                FeatureStatus::Ok => "Ok".to_string(),
                FeatureStatus::Suppressed => "Suppressed".to_string(),
                FeatureStatus::Failed(m) => format!("Failed: {m}"),
                FeatureStatus::Skipped(m) => format!("Skipped: {m}"),
            };
            (format!("#{}", r.id), s)
        })
        .collect()
}

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

/// Metal vertices standing inside each stone, 0.03 mm in from its surface: the stone must sit clear.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    stone_intruders(d, built, &built.mesh.vertices)
}

fn stone_intruders(d: &RingDesign, built: &mesh::BuildResult, vertices: &[ringdesign_core::Vec3]) -> Vec<(String, usize)> {
    ringdesign_core::stones::frames_of(d, ringdesign_core::setstone::record(d, Some(built)).stones)
        .into_iter()
        .map(|(st, f)| {
            let (a, b) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03);
            let (crown, pav) = (st.gem.crown_mm() - 0.03, st.gem.pavilion_mm() - 0.03);
            let inside = vertices
                .iter()
                .filter(|p| {
                    let q = sub([p.0 as f64, p.1 as f64, p.2 as f64], f.girdle);
                    let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                    let e = (x / a).powi(2) + (y / b).powi(2);
                    // Crown: a dome over the girdle; pavilion: a cone to the culet.
                    let inside = if z >= 0.0 { z < crown && e < 1.0 - (z / crown).powi(2) } else { -z < pav && e.sqrt() < 1.0 + z / pav };
                    if inside && std::env::var("SENTIS_DEBUG").is_ok() {
                        println!("      inside the stone at long {x:.3} short {y:.3} up {z:.3}");
                    }
                    inside
                })
                .count();
            (st.label.clone(), inside)
        })
        .collect()
}

/// Each part's thinnest metal in the finished ring: up to 320 rays a part, cast inward from the centres of the
/// finished faces it owns (`Mesh::origin`) through the whole finished mesh. Ends buried in another part are gone
/// after the union, so they no longer read as thin walls the way rays on a part's own mesh do.
fn walls(d: &RingDesign, built: &mesh::BuildResult) -> Vec<serde_json::Value> {
    let m = &built.mesh;
    let owner = |v: u32| -> Option<u64> {
        let o = *m.origin.get(v as usize)?;
        let j = o.checked_sub(mesh::SOLID_VERTEX + built.parts.first)?;
        built.parts.features.get(j as usize).copied()
    };
    let tris: Vec<[P3; 3]> = m.faces.iter().map(|f| f.map(|i| { let p = m.vertices[i as usize]; [p.0 as f64, p.1 as f64, p.2 as f64] })).collect();
    let doc = d.cad.as_ref();
    let mut by: std::collections::BTreeMap<u64, Vec<usize>> = Default::default();
    for (i, f) in m.faces.iter().enumerate() {
        if let (Some(a), Some(b), Some(c)) = (owner(f[0]), owner(f[1]), owner(f[2])) {
            if a == b && b == c {
                by.entry(a).or_default().push(i);
            }
        }
    }
    let mut out = Vec::new();
    for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
        if c.settings.reference || c.attach == Attach::Cut || c.name.contains("cabochon") || doc.and_then(|dd| dd.feature(c.id)).is_none() {
            continue;
        }
        let floor = if pointed(&c.name) { MIN_DETAIL_MM } else { MIN_SECTION_MM };
        let finished = by.get(&c.id).map(|faces| sample_walls(&tris, faces, floor, "finished"));
        let mut row = match finished.filter(|r| r["rays"].as_u64().unwrap_or(0) >= 200) {
            // Sampled where the part shows in the finished metal, through the whole finished mesh.
            Some(r) => r,
            // A part too little of which shows in the finished metal for 200 rays is sampled on its own solid.
            None => {
                let own: Vec<[P3; 3]> = c.mesh.faces.iter().map(|f| f.map(|i| { let p = c.mesh.vertices[i as usize]; [p.0 as f64, p.1 as f64, p.2 as f64] })).collect();
                let all: Vec<usize> = (0..own.len()).collect();
                sample_walls(&own, &all, floor, "own solid, before the union (too little of it shows in the finished metal for 200 rays)")
            }
        };
        row["part"] = json!(c.name);
        out.push(row);
    }
    out
}

/// Up to 320 inward rays from the faces `faces` of `tris`, at least 200 where the part has them, each ray cast
/// through every triangle within 3 mm. A face touching a hard edge (neighbours turning over 50 degrees) is not a
/// ray start: its ray would measure the distance to the edge, not a wall. Those skipped rays are counted.
fn sample_walls(tris: &[[P3; 3]], faces: &[usize], floor: f64, on: &str) -> serde_json::Value {
    let normal = |t: &[P3; 3]| unit(cross(sub(t[1], t[0]), sub(t[2], t[0])));
    let bounds = |t: &[P3; 3]| -> (P3, P3) { (std::array::from_fn(|k| t.iter().map(|p| p[k]).fold(f64::MAX, f64::min)), std::array::from_fn(|k| t.iter().map(|p| p[k]).fold(f64::MIN, f64::max))) };
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for &i in faces {
        let (a, b) = bounds(&tris[i]);
        for k in 0..3 { lo[k] = lo[k].min(a[k] - 3.0); hi[k] = hi[k].max(b[k] + 3.0); }
    }
    let near: Vec<usize> = (0..tris.len()).filter(|&j| { let (a, b) = bounds(&tris[j]); (0..3).all(|k| b[k] >= lo[k] && a[k] <= hi[k]) }).collect();
    // Hard edges among the nearby faces: shared edges by quantised position.
    let key = |p: P3| p.map(|v| (v * 1e5).round() as i64);
    let mut edges: std::collections::HashMap<([i64; 3], [i64; 3]), Vec<usize>> = Default::default();
    for &j in &near {
        for k in 0..3 {
            let (a, b) = (key(tris[j][k]), key(tris[j][(k + 1) % 3]));
            edges.entry(if a < b { (a, b) } else { (b, a) }).or_default().push(j);
        }
    }
    let mut hard_face = std::collections::HashSet::new();
    for fs in edges.values() {
        if fs.len() == 2 && dot(normal(&tris[fs[0]]), normal(&tris[fs[1]])) < 50f64.to_radians().cos() {
            hard_face.insert(fs[0]);
            hard_face.insert(fs[1]);
        }
    }
    let (mut least, mut at, mut rays, mut below, mut slivers, mut hard) = (f64::MAX, [0.0; 3], 0usize, 0usize, 0usize, 0usize);
    let stride = (faces.len() / 640).max(1);
    // Ray starts: each face's centroid on a first pass; a small part with fewer faces than rays wanted is sampled
    // again from three more points inside each face.
    let starts: [[f64; 3]; 4] = [[1.0 / 3.0; 3], [0.6, 0.2, 0.2], [0.2, 0.6, 0.2], [0.2, 0.2, 0.6]];
    'passes: for (offset, bary) in (0..stride).map(|o| (o, starts[0])).chain((1..4).map(|k| (0, starts[k]))) {
        for &i in faces.iter().skip(offset).step_by(stride) {
            if rays >= 320 {
                break 'passes;
            }
            if hard_face.contains(&i) {
                hard += 1;
                continue;
            }
            let [a, b, c] = tris[i];
            let n = cross(sub(b, a), sub(c, a));
            if dot(n, n) < 1e-16 { continue; }
            let dir = mul(unit(n), -1.0);
            let o = add3(add3(mul(a, bary[0]), mul(b, bary[1])), mul(c, bary[2]));
            let hit = near.iter().filter(|&&j| j != i).filter_map(|&j| {
                let [p, q, r] = tris[j];
                let (e1, e2) = (sub(q, p), sub(r, p));
                let hv = cross(dir, e2);
                let det = dot(e1, hv);
                if det.abs() < 1e-14 { return None; }
                let sv = sub(o, p);
                let u = dot(sv, hv) / det;
                if !(0.0..=1.0).contains(&u) { return None; }
                let qv = cross(sv, e1);
                let v = dot(dir, qv) / det;
                if v < 0.0 || u + v > 1.0 { return None; }
                let t = dot(e2, qv) / det;
                (t > 1e-5).then_some(t)
            }).fold(f64::MAX, f64::min);
            if hit == f64::MAX { continue; }
            // A ray stopped within 0.03 mm has met a fold where two surfaces of the union cross at a seam, not a wall.
            if hit < SLIVER_MM { slivers += 1; continue; }
            rays += 1;
            below += usize::from(hit < floor);
            if hit < least { least = hit; at = o; }
        }
        if rays >= 200 {
            break;
        }
    }
    json!({"sampled_min_mm": least, "at": um(at), "rays": rays, "below_floor": below, "floor_mm": floor, "seam_folds_skipped": slivers, "hard_edge_rays_skipped": hard, "sampled_on": on})
}

/// Rays stopped closer than this have met a seam fold of the union, not a wall.
const SLIVER_MM: f64 = 0.03;

/// Whether a part is a pointed detail, judged at the detail floor: thorns, shoots and the claws' points.
fn pointed(name: &str) -> bool {
    name.contains("Prickle") || name.ends_with("point")
}

fn walls_ok(walls: &[serde_json::Value]) -> bool {
    walls.iter().all(|w| w["sampled_min_mm"].as_f64().zip(w["floor_mm"].as_f64()).is_some_and(|(m, f)| m >= f))
}

// --- Renders ---------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger's axis, pitch toward it.
const VIEWS: [(&str, f64, f64); 6] = [("hero", -0.35, 0.9), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", 0.75, 0.6), ("reverse", PI - 0.5, 0.35)];

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Hero, face and side at 300 px on one sheet.
fn contact(path: &Path, parts: &[render::Part<'_>]) -> Result<()> {
    let edge = 300;
    let shots: Vec<Vec<u8>> = [VIEWS[0], VIEWS[1], VIEWS[3]].iter().map(|(_, y, p)| render::render_parts_ss(parts, *y, *p, edge, edge, 3)).collect();
    let mut out = vec![0u8; edge * 3 * edge * 3];
    for y in 0..edge {
        for (k, s) in shots.iter().enumerate() {
            out[y * edge * 9 + k * edge * 3..y * edge * 9 + (k + 1) * edge * 3].copy_from_slice(&s[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, (edge * 3) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The face view framed on the ring's whole width, down onto the crown.
fn face_framing() -> render::Framing {
    render::Framing::new(world(CROWN_DEG, (TWINE_R_MM, 0.0)), 11.0)
}

fn renders(out: &Path, finished: &render::Finished, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    if let Ok(list) = std::env::var("SENTIS_TRY") {
        for (k, pair) in list.split(';').enumerate() {
            let v: Vec<f64> = pair.split(',').filter_map(|x| x.parse().ok()).collect();
            render::write_png_parts(out.join(format!("try-{k}.png")), &parts, v[0], v[1], 400)?;
        }
        return Ok(());
    }
    for (name, yaw, pitch) in VIEWS {
        if name == "face" {
            render::write_png_framed(out.join("face.png"), &parts, yaw, pitch, face_framing(), edge)?;
        } else {
            render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
        }
    }
    // The hip close and the leaf close, framed on the whole parts (#248), never a cropped mesh.
    let crown = world(CROWN_DEG, (TWINE_R_MM + 1.5, 0.0));
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(CROWN_DEG) - 0.45, 0.85, render::Framing::new(crown, 6.0), edge)?;
    render::write_png_framed(out.join("leaf-close.png"), &parts, render::yaw_facing(CROWN_DEG - 22.0), 1.2, render::Framing::new(world(CROWN_DEG - 22.0, (TWINE_R_MM + 1.0, 0.0)), 6.0), edge)?;
    // Bare against finished: the two canes alone beside the finished ring.
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let bare = mesh::try_build(&author(true, &[])?.design, &AlphaLibrary::builtin(), draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    render::write_png_framed(out.join("face-300.png"), &parts, VIEWS[1].1, VIEWS[1].2, face_framing(), 300)?;
    contact(&out.join("contact-300.png"), &parts)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/sentis"));
    std::fs::create_dir_all(&out)?;
    println!("Sentis");
    let started = std::time::Instant::now();
    if std::env::var("SENTIS_LEAFLET").is_ok() {
        let sol = leaflet_solid([0.0, 0.0], 0.0, TERMINAL.0, TERMINAL.1, 6f64.to_radians(), 0.3);
        let tris: Vec<[P3; 3]> = sol.f.iter().map(|f| f.map(|i| sol.v[i as usize])).collect();
        let mut found: Vec<(f64, P3, P3)> = Vec::new();
        for (i, t) in tris.iter().enumerate() {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            if dot(n, n) < 1e-16 { continue; }
            let dir = mul(unit(n), -1.0);
            let o = mul(add3(add3(t[0], t[1]), t[2]), 1.0 / 3.0);
            let hit = tris.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, q)| {
                let (e1, e2) = (sub(q[1], q[0]), sub(q[2], q[0]));
                let hv = cross(dir, e2);
                let det = dot(e1, hv);
                if det.abs() < 1e-14 { return None; }
                let sv = sub(o, q[0]);
                let u = dot(sv, hv) / det;
                if !(0.0..=1.0).contains(&u) { return None; }
                let qv = cross(sv, e1);
                let v = dot(dir, qv) / det;
                if v < 0.0 || u + v > 1.0 { return None; }
                let tt = dot(e2, qv) / det;
                (tt > 1e-5).then_some(tt)
            }).fold(f64::MAX, f64::min);
            found.push((hit, o, dir));
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        for f in found.iter().take(6) { println!("    leaflet thin {:.3} at {:?} dir {:?}", f.0, um(f.1), um(f.2)); }
        return Ok(());
    }
    // A prickle whose seam bead the kernel cannot lay (it folds or pinches where the prickle meets a crossing or a
    // node) is authored again with a crisp root: found on a first build, so the second is clean.
    let first = author(false, &[])?;
    let probe_build = mesh::try_build(&first.design, &AlphaLibrary::builtin(), draft_params())?;
    let unbeaded: Vec<String> = probe_build.parts.notes.iter().filter(|n| n.contains("fillet")).filter_map(|n| n.split(':').next().map(str::to_string)).filter(|n| n.starts_with("Prickle")).collect();
    println!("  prickles laid without a bead: {unbeaded:?}");
    // A prickle that fuses into the other cane or a node leaves a thin fin in the union: found on the first build
    // and left out; leaving one out can bare a fin under its neighbour, so the probe repeats until none is left.
    let mut skip = unbeaded.clone();
    let mut fins: Vec<String> = Vec::new();
    let mut probe = (first.design, probe_build);
    for _ in 0..4 {
        let found: Vec<String> = walls(&probe.0, &probe.1).iter().filter(|w| w["part"].as_str().is_some_and(|n| n.starts_with("Prickle")) && w["sampled_min_mm"].as_f64().is_some_and(|m| m < MIN_DETAIL_MM)).filter_map(|w| w["part"].as_str().map(str::to_string)).collect();
        if found.is_empty() { break; }
        skip.extend(found.iter().map(|f| format!("omit:{f}")));
        fins.extend(found);
        let next = author(false, &skip)?.design;
        let next_build = mesh::try_build(&next, &AlphaLibrary::builtin(), draft_params())?;
        probe = (next, next_build);
    }
    println!("  prickles left out for a fin: {fins:?}");
    let Authored { design: d, thorns, stone_id } = author(false, &skip)?;
    let lib = AlphaLibrary::builtin();
    let author_s = started.elapsed().as_secs_f64();
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    println!(
        "  {} triangles in {build_s:.1} s; watertight {watertight}; degenerate {degenerate}; self-crossings {crossings}; notes {:?} {:?}",
        built.mesh.faces.len(),
        built.solids.notes,
        built.parts.notes
    );
    if std::env::var("SENTIS_DEBUG").is_ok() {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            if let Some((lo, hi)) = c.mesh.bounds() {
                let rmin = c.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
                let intruding: usize = stone_intruders(&d, &built, &c.mesh.vertices).iter().map(|x| x.1).sum();
                if intruding > 0 {
                    println!("    {} has {intruding} vertices inside the stone", c.name);
                }
                if c.name.contains("Hip") || rmin < d.inner_radius_mm() {
                    println!("    {}: {:?} .. {:?}, nearest the axis {rmin:.2}", c.name, lo, hi);
                }
            }
        }
        let near = built.mesh.vertices.iter().filter(|p| p.0 < -2.7 && p.1 > 12.5).count();
        println!("    finished vertices beyond the hip's tip: {near}");
    }
    let made = made_parts(&built);
    let (shell_count, voids) = metal_shells(&built.mesh);
    if std::env::var("SENTIS_DEBUG").is_ok() {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            if let Some((lo, hi)) = c.mesh.bounds() {
                if lo.0 < -11.0 && hi.0 > -11.05 && lo.1 < 3.5 && hi.1 > 3.47 && lo.2 < 0.88 && hi.2 > 0.86 {
                    println!("    near the shard: {} ({} shells alone)", c.name, shells(&c.mesh));
                }
            }
        }
        // Each finished shell's extent.
        let mut by: std::collections::HashMap<u32, (usize, [f32; 3], [f32; 3])> = Default::default();
        let mut parent: Vec<u32> = (0..built.mesh.vertices.len() as u32).collect();
        fn find(p: &mut [u32], mut x: u32) -> u32 { while p[x as usize] != x { p[x as usize] = p[p[x as usize] as usize]; x = p[x as usize]; } x }
        for f in &built.mesh.faces { for k in 1..3 { let (a, b) = (find(&mut parent, f[0]), find(&mut parent, f[k])); parent[a as usize] = b; } }
        for f in &built.mesh.faces {
            let r = find(&mut parent, f[0]);
            let p = built.mesh.vertices[f[0] as usize];
            let e = by.entry(r).or_insert((0, [f32::MAX; 3], [f32::MIN; 3]));
            e.0 += 1;
            for (k, v) in [p.0, p.1, p.2].into_iter().enumerate() { e.1[k] = e.1[k].min(v); e.2[k] = e.2[k].max(v); }
        }
        for (r, (n, lo, hi)) in by {
            // Signed volume of the shell: negative is a void sealed inside the metal.
            let vol: f64 = built.mesh.faces.iter().filter(|f| find(&mut parent, f[0]) == r).map(|f| {
                let p = |i: u32| { let v = built.mesh.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] };
                dot(p(f[0]), cross(p(f[1]), p(f[2]))) / 6.0
            }).sum();
            println!("    shell of {n} faces, volume {vol:.4}: {lo:?} .. {hi:?}");
        }
    }
    println!("  metal shells {shell_count}, sealed voids {voids:?}");
    let (solids_notes, parts_notes) = (built.solids.notes.clone(), built.parts.notes.clone());
    let status = feature_status(&built);
    for (n, s) in &status {
        if s != "Ok" {
            println!("    feature {n}: {s}");
        }
    }
    for (n, c) in made.iter().filter(|(_, c)| *c > 0) {
        println!("    part {n}: {c} self-crossings");
    }
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let wall = walls(&d, &built);
    println!("  field {} thinnest wall {:.2} mm at {:.0} deg", field.verdict.label(), field.thinnest_wall_mm, field.thinnest_wall_theta_deg);
    for w in &wall {
        if !walls_ok(std::slice::from_ref(w)) {
            println!("    thin: {w}");
        }
    }
    let findings = dfm::findings_in(&d, &lib);
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let crowding = stones.as_ref().map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm)).collect::<Vec<_>>()).unwrap_or_default();
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    let in_stones = metal_in_stones(&d, &built);
    println!("  stones reported {reported}, previewed {previewed}; metal inside {:?}; crowding {:?}", in_stones, crowding);
    let coarse = mesh::try_build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() })?;
    let (cw, cd, cx) = geometry(&coarse.mesh);
    let coarse_ok = cw && cd == 0 && cx == 0 && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}; notes {:?} {:?}", coarse.solids.notes, coarse.parts.notes);
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    println!("  casting pattern: {} triangles, watertight {pw}, degenerate {pd}, crossings {px}", pattern.mesh.faces.len());
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let _ = std::fs::remove_file(out.join("design.ring.json.bak"));
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let design_format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = ringdesign_core::manufacturing::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let triangles = built.mesh.faces.len();
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings", made.iter().all(|(_, n)| *n == 0)),
        ("one metal shell: nothing floats (sealed voids under 0.01 mm3 are reported, not counted)", shell_count == 1),
        ("solids and parts notes empty, every feature Ok", solids_notes.is_empty() && parts_notes.is_empty() && status.iter().all(|(_, s)| s == "Ok")),
        ("nothing enters the finger hole", inside == 0),
        ("every metal part's ray-sampled wall recorded with at least 200 rays: canes, nodes, stipules, shoots, rachis, stalk, hip cup, calyx boss, sepal bodies and every leaflet at the 0.8 mm section; prickle, sepal and shoot-bud points at the 0.15 mm detail floor", walls_ok(&wall) && wall.iter().all(|w| w["rays"].as_u64().unwrap_or(0) >= 200)),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed && reported == 1),
        ("no metal inside a stone", in_stones.iter().all(|(_, n)| *n == 0)),
        ("crowding census clean", crowding.is_empty()),
        ("metal-in-stone check ran on every stone", in_stones.len() == reported),
        ("gates hold at 384 x 192", coarse_ok),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "stage": STAGE,
        "process": d.draft.process.label(),
        "draft": serde_json::to_value(&d.draft)?,
        "alloy_for_weight": "Gold 18k",
        "grams_18k": grams,
        "size": d.size.display(),
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": triangles, "build_s": build_s, "author_s": author_s},
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings, "shells": shell_count, "sealed_voids_mm3": voids},
        "made_parts": made,
        "features": status,
        "thorns": thorns,
        "stone_feature": stone_id,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "notes": field.notes},
        "ray_walls": wall,
        "lost_wax_note": "Lost wax (Logan, 2026-10-03): 0.8 mm minimum section, no pull rule. The band-field verdict reads 'Castable with care' for every CAD-only design by construction (castability.rs: no procedural band to sample), so the lost-wax gate is the wall gate. Walls are sampled on the finished mesh: 200 to 320 inward rays per part from the faces it owns, cast through the whole finished metal; a part too little of which shows for 200 rays is sampled on its own solid and says so (sampled_on). Ray starts on a face touching a hard edge (over 50 degrees) are skipped and counted per part (hard_edge_rays_skipped), as are rays stopped within 0.03 mm at a seam fold. Every body and blade is judged at 0.8 mm: canes, nodes, stipules, shoots, rachis and petiole, stalk, hip cup, calyx boss, sepal bodies and all ten leaflets. Only tapering points are judged at the 0.15 mm detail floor, as Manticora's aculeus was: the prickles, the sepal points (the last millimetre of each sepal, a separate part) and the shoot-bud points.",
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "notes": {"solids": solids_notes, "parts": parts_notes},
        "stones": {"reported": reported, "previewed": previewed, "metal_inside": in_stones, "crowding": crowding},
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "notes": [&coarse.solids.notes, &coarse.parts.notes]},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "design": {"bytes": text.len(), "format_version": design_format, "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let report = if draft {
        report
    } else {
        let mut r = report;
        if let Ok(old) = std::fs::read_to_string(out.join("draft-gates.json")) {
            r["draft_build"] = serde_json::from_str(&old)?;
        }
        r
    };
    if draft {
        std::fs::write(out.join("draft-gates.json"), serde_json::to_vec_pretty(&report)?)?;
    }
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let finished = render::finished_from(&d, &lib, built);
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &finished.metal, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Sentis / investment pattern")?;
        let mut entries = Vec::new();
        for (m, tint) in &finished.stones {
            stl::write_stl(out.join("reference-hip.stl"), m, "Sentis reference hip")?;
            let gem = hip_gem();
            entries.push(json!({"mesh": "reference-hip.stl", "name": "Rose hip: opaque orange-red cabochon (carnelian or opaque garnet)", "cut": builders::gem_label(gem), "l_mm": gem.l_mm, "w_mm": gem.w_mm, "tint": tint, "ior": 1.54, "dispersion": 0.0, "roughness": 0.25, "transmission": 0.05}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": entries}))?)?;
    }
    renders(&out, &finished, if draft { 1000 } else { 1600 })?;
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    if !gates.iter().all(|(_, p)| *p) {
        println!("Sentis failed a gate; see {}", out.join("report.json").display());
    }
    Ok(())
}

