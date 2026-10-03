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
const TWINE_R_MM: f64 = 10.65;
const TWINE_RADIAL_MM: f64 = 0.5;
const TWINE_ACROSS_MM: f64 = 1.6;
const TWINE_CROWN_OPEN_MM: f64 = 0.95;
/// Each cane's wander, radians of phase, zero at crown and palm.
const WANDER: [f64; CANES] = [0.28, -0.22];
/// Cane radius at the palm and at the crown.
const CANE_PALM_MM: f64 = 0.62;
const CANE_CROWN_MM: f64 = 0.8;
/// The cane section's five lobes, as a share of its radius, and the turns they make round each cane.
const CANE_LOBE: f64 = 0.06;
const CANE_TURNS: f64 = 3.0;
/// Path points round each closed cane.
const STATIONS: usize = 180;
/// Prickles per cane before those facing the finger or under the crown's leaves and hip are dropped.
const THORNS_PER_CANE: usize = 22;
/// How deep each prickle's foot sinks into its cane, as a share of its size.
const THORN_SINK_MM: f64 = 0.26;
/// The hip: an oval cabochon, its length and width.
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
const HIP_TINT: [f32; 3] = [0.55, 0.12, 0.06];

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
    let lobed: Vec<[f64; 2]> = (0..40)
        .map(|k| {
            let t = TAU * k as f64 / 40.0;
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
    Placement::Ring { theta_deg, across_mm: origin[2], height_mm: r - seat_r(d), spin_deg: spin.to_degrees(), tilt_deg: tilt.to_degrees(), cant_deg: cant.to_degrees(), level: false }
}

/// A part standing on cane `i` at `theta_deg`: its foot sunk `sink` mm into the cane, its z along the cane's
/// outward normal, its y along the cane, `flip`ped to hook back the other way.
fn on_cane(d: &RingDesign, i: usize, theta_deg: f64, sink: f64, flip: bool) -> Component {
    let n = cane_normal(i, theta_deg);
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
const THORN_BLEND_MM: f64 = 0.18;

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

/// A rose prickle: a broad, flattened foot long along the cane, a short rise and a hard hook back down it.
#[derive(Clone, Copy)]
struct Hook {
    along: f64,
    across: f64,
    radial: f64,
    bend_r: f64,
    bend_deg: f64,
}

const THORN: Hook = Hook { along: 2.6, across: 1.05, radial: 0.4, bend_r: 1.3, bend_deg: 70.0 };
/// Each prickle's tip across, at least.
const THORN_TIP_MM: f64 = 0.42;

/// The prickle's foot: an ellipse `a` mm along the cane and `b` mm across it.
fn ellipse(a: f64, b: f64) -> Sketch {
    let pts: Vec<[f64; 2]> = (0..24).map(|k| {
        let t = TAU * k as f64 / 24.0;
        [0.5 * b * t.cos(), 0.5 * a * t.sin()]
    }).collect();
    polygon("Prickle foot", &pts)
}

/// A prickle `k` times the crown's size, tapering by a law that keeps its foot broad and runs fast to the point.
fn thorn(k: f64) -> Operation {
    let h = THORN;
    let tip = (THORN_TIP_MM / (h.across * k)).max(0.2);
    let scale = vec![[0.0, 1.0], [0.25, 0.62], [0.6, 0.38], [1.0, (tip * 1e4).round() / 1e4]];
    Operation::Twist { sketch: ellipse(h.along * k, h.across * k).into(), path: hook_path(h.radial * k, h.bend_r * k, h.bend_deg).into(), degrees: 0.0, end_scale: tip, scale, closed: false }
}

/// Where a prickle stands: cane, angle, size and which way it hooks.
#[derive(Clone, Copy)]
struct ThornAt {
    cane: usize,
    theta: f64,
    scale: f64,
    flip: bool,
}

/// The prickles: along each cane at an uneven pitch, the crown's 1.4 times the palm's, every one on a cane hooking
/// back the same way; none facing the finger, none under the leaves and hip.
fn thorn_sites() -> Vec<ThornAt> {
    let mut out = Vec::new();
    for i in 0..CANES {
        for k in 0..THORNS_PER_CANE {
            let seed = (i * 100 + k) as u64;
            let theta = (11.0 + 13.0 * i as f64 + 360.0 * (k as f64 + 0.35 * (hash(seed) - 0.5)) / THORNS_PER_CANE as f64).rem_euclid(360.0);
            let n = cane_normal(i, theta);
            let (s, c) = theta.to_radians().sin_cos();
            let out_r = n[0] * c + n[1] * s;
            let from_crown = ((theta - CROWN_DEG + 540.0).rem_euclid(360.0) - 180.0).abs();
            // Clear of the crossings, where a prickle would run into the other cane.
            let (me, other) = (cane_at(i, theta), cane_at(1 - i, theta));
            let apart = (me.0 - other.0).hypot(me.1 - other.1);
            let crown = 0.5 + 0.5 * (theta - CROWN_DEG).to_radians().cos();
            let scale = 0.72 + 0.29 * crown + 0.05 * (hash(seed + 7) - 0.5);
            // The whole prickle, foot to hooked tip, clear of the other cane.
            let t = cane_tangent(i, theta);
            let foot = add3(cane_point(i, theta), mul(n, cane_r(theta)));
            let reach = |p: P3| (0..=40).map(|j| { let a = theta - 20.0 + j as f64; dot(sub(p, cane_point(1 - i, a)), sub(p, cane_point(1 - i, a))).sqrt() - cane_r(a) }).fold(f64::MAX, f64::min);
            let hook = if i == 1 { -1.0 } else { 1.0 };
            let clear = [0.0, 0.5, 1.0].iter().all(|&f| reach(add3(foot, add3(mul(n, 1.1 * scale * f), mul(t, hook * 1.2 * scale * f * f)))) > 0.35)
                && reach(add3(foot, mul(t, 1.4 * scale))) > 0.2
                && reach(add3(foot, mul(t, -1.4 * scale))) > 0.2;
            if out_r < -0.3 || from_crown < 56.0 || apart < 1.8 || !clear {
                continue;
            }
            if out.iter().any(|o: &ThornAt| o.cane == i && ((o.theta - theta + 540.0).rem_euclid(360.0) - 180.0).abs() < 11.0) {
                continue;
            }
            out.push(ThornAt { cane: i, theta, scale, flip: i == 1 });
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
    let (steps, across) = (72usize, 7usize);
    let (edge, rib, keel) = (LEAF_EDGE_MM, LEAF_RIB_MM, 0.1);
    let d = [alpha.sin(), alpha.cos()];
    let perp = [d[1], -d[0]];
    let half = |u: f64| {
        let body = (PI * u.powf(0.8)).sin().powf(0.62);
        let t = (u * LEAF_TEETH as f64).fract();
        0.5 * wide * body * (1.0 - LEAF_TOOTH * t * (u > 0.15 && u < 0.93) as u8 as f64)
    };
    let (u0, u1) = (0.03, 0.95);
    let centre = |u: f64| -> P3 { [origin[0] + d[0] * u * len, origin[1] + d[1] * u * len, u * len * lift.tan()] };
    let loops: Vec<Vec<P3>> = (0..=steps).map(|i| {
        let u = u0 + (u1 - u0) * i as f64 / steps as f64;
        let (c, w) = (centre(u), half(u).max(0.17));
        let under = sink * (1.0 - u / 0.25).max(0.0);
        let mut l = Vec::with_capacity(2 * across + 2);
        for j in 0..=across {
            let v = -1.0 + 2.0 * j as f64 / across as f64;
            let groove = LEAF_GROOVE_MM * (1.0 - (v.abs() / 0.22).min(1.0)).powi(2) * (u < 0.9) as u8 as f64;
            let top = edge + (rib - edge) * (1.0 - v.abs()).powf(1.3) - groove;
            l.push([c[0] + perp[0] * v * w, c[1] + perp[1] * v * w, c[2] + top]);
        }
        for j in (0..=across).rev() {
            let v = -1.0 + 2.0 * j as f64 / across as f64;
            l.push([c[0] + perp[0] * v * w, c[1] + perp[1] * v * w, c[2] - under - keel * (1.0 - v.abs())]);
        }
        l
    }).collect();
    let (a, b) = (centre(0.0), centre(1.0));
    lofted(&loops, [a[0], a[1], a[2] + 0.5 * edge - sink], [b[0], b[1], b[2] + 0.5 * edge])
}

const LEAF_EDGE_MM: f64 = 0.32;
const LEAF_RIB_MM: f64 = 0.66;
const LEAF_TEETH: usize = 9;
const LEAF_TOOTH: f64 = 0.1;
const LEAF_GROOVE_MM: f64 = 0.12;

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
    Leaf { cane: 0, foot_deg: CROWN_DEG - 14.0, theta0: CROWN_DEG - 15.0, z0: 0.9, dir: -1.0, skew: -0.12 },
    Leaf { cane: 1, foot_deg: CROWN_DEG + 14.0, theta0: CROWN_DEG + 15.0, z0: -0.9, dir: 1.0, skew: 0.12 },
];
/// The rachis: its length to the terminal leaflet, its radius; the leaflet pairs' stations, angles, sizes.
const RACHIS_MM: f64 = 5.4;
const RACHIS_R_MM: f64 = 0.43;
const PAIRS: [(f64, f64, f64, f64); 2] = [(1.0, 62.0, 3.3, 2.1), (3.3, 52.0, 3.5, 2.25)];
const TERMINAL: (f64, f64) = (4.2, 2.6);
/// How far the leaf's underside sits under the highest cane beneath it.
const LEAF_SINK_MM: f64 = 0.15;

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

// --- The hip -----------------------------------------------------------------------------------------------

/// The hip's frame: girdle centre, long axis (toward the calyx), across, and up out of the ring.
struct HipFrame {
    g: P3,
    u: P3,
    v: P3,
    w: P3,
}

/// The hip's girdle over the seat, the long axis's turn from along the finger toward round the ring, and its lift.
const HIP_H_MM: f64 = 1.75;
const HIP_TURN_DEG: f64 = 28.0;

/// The hip's placement: at the crown, its long axis turned `HIP_TURN_DEG` off the finger's axis, girdle level.
fn hip_placement(d: &RingDesign) -> (Placement, HipFrame) {
    let r = seat_r(d) + HIP_H_MM;
    let g = world(CROWN_DEG, (r, 0.0));
    let w = world(CROWN_DEG, (1.0, 0.0));
    let ring = world(CROWN_DEG + 90.0, (1.0, 0.0));
    let t = HIP_TURN_DEG.to_radians();
    let u = unit(add3(mul([0.0, 0.0, 1.0], t.cos()), mul(ring, t.sin())));
    let v = cross(w, u);
    // A stone's long axis is its frame's y: the frame's x is y x z.
    (placed_as(d, CROWN_DEG, g, cross(u, w), w), HipFrame { g, u, v, w })
}

impl HipFrame {
    fn at(&self, p: P3) -> P3 {
        add3(self.g, add3(mul(self.u, p[0]), add3(mul(self.v, p[1]), mul(self.w, p[2]))))
    }
    fn plane(&self, p: P3) -> Workplane {
        Workplane { origin: um(self.at(p)), x: self.u, y: self.v, ..Default::default() }
    }
}

/// The receptacle under the stone: an oval cup lofted down from just under the girdle into the canes.
const HIP_CUP: [(f64, f64, f64); 5] = [(-2.3, 1.5, 1.05), (-1.6, 2.45, 1.85), (-0.85, 2.95, 2.2), (-0.3, 3.05, 2.28), (-0.06, 2.98, 2.22)];

fn ellipse_on(plane: Workplane, name: &str, au: f64, av: f64) -> Sketch {
    let pts: Vec<[f64; 2]> = (0..40).map(|k| {
        let t = TAU * k as f64 / 40.0;
        [au * t.cos(), av * t.sin()]
    }).collect();
    poly_on(plane, name, &pts)
}

/// The calyx's sepals: how many, their length out from the hub and back along the hip, section and taper.
const SEPALS: usize = 5;
const SEPAL_REACH_MM: f64 = 2.5;
const SEPAL_BACK_MM: f64 = 1.6;
const SEPAL_SECTION: (f64, f64) = (0.95, 0.55);
/// The calyx hub's radius and its standing-off past the stone's tip.
const HUB_OFF_MM: f64 = 0.78;
const STALK_R_MM: f64 = 0.46;

// --- The document ------------------------------------------------------------------------------------------

struct Authored {
    design: RingDesign,
    thorns: usize,
    stone_id: u64,
}

fn author(bare: bool) -> Result<Authored> {
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
    for (k, t) in sites.iter().enumerate() {
        add(&mut doc, format!("Prickle {} on cane {}", k + 1, t.cane + 1), thorn(t.scale), on_cane(&d, t.cane, t.theta, THORN_SINK_MM * t.scale, t.flip))?;
    }
    // The rose leaves: a rachis from the cane up over the crown, two pairs of leaflets and a terminal one.
    for (k, leaf) in LEAVES.iter().enumerate() {
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
            for side in [-1.0, 1.0] {
                let a = head + side * ang.to_radians();
                let lift = (5.0 + 2.0 * j as f64).to_radians();
                leaflets.push((format!("pair {} {}", j + 1, if side > 0.0 { "right" } else { "left" }), leaflet_solid(along(s), a, len, wide, lift, 0.25 + 0.03 * j as f64 + 0.01 * side)));
            }
        }
        leaflets.push(("terminal".into(), leaflet_solid(along(RACHIS_MM - 0.4), head, TERMINAL.0, TERMINAL.1, 4f64.to_radians(), 0.3)));
        for (name, solid) in leaflets {
            let placed = outward(csg::Solid { v: solid.v.iter().map(&to_world).collect(), f: solid.f });
            add(&mut doc, format!("Rose leaf {}, {name} leaflet", k + 1), stored_op(&placed, "leaflet", json!({"leaf": k + 1, "leaflet": name}))?, free.clone())?;
        }
        // The rachis: from inside the cane, up and over into the leaf's plane, along it to the terminal leaflet.
        let foot = cane_point(leaf.cane, leaf.foot_deg);
        let start = to_world(&[0.0, 0.0, 0.3]);
        // The petiole, straight from inside the cane up to the leaf's foot; then the rachis along the leaf.
        let petiole = vec![um(foot), um(add3(start, mul(sub(start, foot), 0.05)))];
        add(&mut doc, format!("Rose leaf {}, petiole", k + 1), Operation::Twist { sketch: Sketch::circle(RACHIS_R_MM).into(), path: TwistPath::Points { points: petiole, smooth: false }, degrees: 0.0, end_scale: 1.0, scale: Vec::new(), closed: false }, free.clone())?;
        let points: Vec<P3> = (0..=8).map(|k| um(to_world(&[along(RACHIS_MM * k as f64 / 8.0)[0], along(RACHIS_MM * k as f64 / 8.0)[1], 0.3]))).collect();
        let mut section = Sketch::circle(RACHIS_R_MM);
        section.name = "Rachis".into();
        add(&mut doc, format!("Rose leaf {}, rachis", k + 1), Operation::Twist { sketch: section.into(), path: TwistPath::Points { points, smooth: true }, degrees: 0.0, end_scale: 1.0, scale: Vec::new(), closed: false }, free.clone())?;
    }
    // The hip: receptacle, stalk, calyx and stone, all in the hip's frame.
    let (stone_at, h) = hip_placement(&d);
    let sections = HIP_CUP.iter().map(|&(w, au, av)| ellipse_on(h.plane([0.0, 0.0, w]), "Hip receptacle", au, av).into()).collect();
    add(&mut doc, "Hip receptacle".into(), Operation::Loft { sections }, free.clone())?;
    // The stalk: out of the receptacle's far end, down and round into the low cane.
    let into = cane_point(1, CROWN_DEG - 12.0);
    let p0 = h.at([-1.4, 0.0, -1.4]);
    let p1 = h.at([-3.4, 0.0, -1.5]);
    let stalk_pts: Vec<P3> = (0..=8).map(|k| { let t = k as f64 / 8.0; um(add3(add3(mul(p0, (1.0 - t) * (1.0 - t)), mul(p1, 2.0 * t * (1.0 - t))), mul(into, t * t))) }).collect();
    add(&mut doc, "Hip stalk".into(), Operation::Twist { sketch: Sketch::circle(STALK_R_MM).into(), path: TwistPath::Points { points: stalk_pts, smooth: true }, degrees: 0.0, end_scale: 1.0, scale: Vec::new(), closed: false }, free.clone())?;
    // The calyx: a hub past the stone's tip, five sepals flaring out of it and curling back over the hip's shoulders.
    let hub = h.at([0.5 * HIP_L_MM + HUB_OFF_MM, 0.0, 0.05]);
    for k in 0..SEPALS {
        let a = TAU * k as f64 / SEPALS as f64 + 0.5 * PI;
        let out = add3(mul(h.v, a.cos()), mul(h.w, a.sin()));
        let at = |o: f64, b: f64| um(add3(hub, add3(mul(out, o), mul(h.u, b))));
        let points = vec![at(0.0, 0.15), at(0.9, 0.35), at(1.8, 0.1), at(SEPAL_REACH_MM, -0.6), at(SEPAL_REACH_MM + 0.15, -SEPAL_BACK_MM)];
        let (sa, sb) = SEPAL_SECTION;
        let pts: Vec<[f64; 2]> = (0..20).map(|j| { let t = TAU * j as f64 / 20.0; [0.5 * sa * t.cos(), 0.5 * sb * t.sin()] }).collect();
        let scale = vec![[0.0, 0.8], [0.3, 1.1], [0.7, 0.75], [1.0, 0.5]];
        add(&mut doc, format!("Hip sepal {}", k + 1), Operation::Twist { sketch: polygon("Sepal", &pts).into(), path: TwistPath::Points { points, smooth: true }, degrees: 0.0, end_scale: 0.5, scale, closed: false }, free.clone())?;
    }
    let gem = hip_gem();
    next += 1;
    let stone_id = next;
    doc.append(builders::stone_feature(stone_id, gem, stone_at))?;
    next += 1;
    let mut claws = builders::feature_on(next, "Thorn claws", builders::CLAW, stone_id, json!({"prongs": 4, "style": "Thorn", "tip": "Dome", "rails": "None"}));
    claws.component.stage = Stage::Cast;
    doc.append(claws)?;
    d.cad = Some(doc);
    Ok(Authored { design: d, thorns: sites.len(), stone_id })
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
                    if z >= 0.0 { z < crown && e < 1.0 - (z / crown).powi(2) } else { -z < pav && e.sqrt() < 1.0 + z / pav }
                })
                .count();
            (st.label.clone(), inside)
        })
        .collect()
}

/// Each CAD part's thinnest metal as surface-normal rays find it (`cad::measure::thickness`).
fn walls(built: &mesh::BuildResult) -> Vec<serde_json::Value> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| !c.settings.reference && c.attach != Attach::Cut)
        .map(|c| {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            json!({"part": c.name, "sampled_min_mm": t.sampled_min_mm, "rays": t.rays, "below_floor": t.below_limit, "unresolved": t.unresolved})
        })
        .collect()
}

/// Whether a part is a pointed detail, judged at the detail floor: thorns, shoots and the claws' points.
fn pointed(name: &str) -> bool {
    name.contains("Prickle") || name.contains("sepal") || name.contains("leaflet") || name.contains("claws")
}

fn walls_ok(walls: &[serde_json::Value]) -> bool {
    walls.iter().all(|w| {
        let name = w["part"].as_str().unwrap_or("");
        w["sampled_min_mm"].as_f64().is_some_and(|m| m >= if pointed(name) { MIN_DETAIL_MM } else { MIN_SECTION_MM })
    })
}

// --- Renders ---------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger's axis, pitch toward it.
const VIEWS: [(&str, f64, f64); 6] = [("hero", -0.65, 0.6), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", 0.75, 0.6), ("reverse", PI - 0.5, 0.35)];

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
    render::Framing::new(world(CROWN_DEG, (TWINE_R_MM, 0.0)), 12.6)
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
    let bare = mesh::try_build(&author(true)?.design, &AlphaLibrary::builtin(), draft_params())?;
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
    let Authored { design: d, thorns, stone_id } = author(false)?;
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
    let shell_count = shells(&built.mesh);
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
        for (_, (n, lo, hi)) in by { println!("    shell of {n} faces: {lo:?} .. {hi:?}"); }
    }
    println!("  shells {shell_count}");
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
    let wall = walls(&built);
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
        ("one shell: nothing floats", shell_count == 1),
        ("solids and parts notes empty, every feature Ok", solids_notes.is_empty() && parts_notes.is_empty() && status.iter().all(|(_, s)| s == "Ok")),
        ("nothing enters the finger hole", inside == 0),
        ("every part's ray-sampled wall at the 0.8 mm section, its pointed details (thorns, shoots) at the 0.15 mm detail floor", walls_ok(&wall)),
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
        "stage": "block-out",
        "process": d.draft.process.label(),
        "draft": serde_json::to_value(&d.draft)?,
        "alloy_for_weight": "Gold 18k",
        "grams_18k": grams,
        "size": d.size.display(),
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": triangles, "build_s": build_s, "author_s": author_s},
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings, "shells": shell_count},
        "made_parts": made,
        "features": status,
        "thorns": thorns,
        "stone_feature": stone_id,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "notes": field.notes},
        "ray_walls": wall,
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
            stl::write_stl(out.join("reference-ruby.stl"), m, "Sentis reference ruby")?;
            entries.push(json!({"mesh": "reference-ruby.stl", "name": "Ruby", "cut": "Round 4.0", "tint": tint, "ior": 1.77, "dispersion": 0.018, "roughness": 0.065, "transmission": 0.72}));
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

