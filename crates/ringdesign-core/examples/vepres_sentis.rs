//! Vepres — Sentis, the briar thicket: four thorned briar canes wound over and under one another round the
//! finger on a hidden heartwood liner, the tangle densest at the crown, where three cut shoots bristle up and one
//! ruby rose-hip sits gripped deep inside by four thorn claws. CAD only, lost wax.
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
const SEAT_THICK_MM: f64 = 2.7;
/// The heartwood liner: a smooth torus whose inside is the bore.
const LINER_MINOR_MM: f64 = 0.55;
/// Four canes, each three waves round the ring.
const CANES: usize = 4;
const WAVES: f64 = 3.0;
/// The cane's section radius, and the radius the bundle winds about.
const CANE_R_MM: f64 = 0.72;
const BUNDLE_R_MM: f64 = 10.85;
/// The bundle's half-height across the finger and half-depth radially: deeper at the crown.
const BUNDLE_RADIAL_MM: f64 = 0.55;
const BUNDLE_ACROSS_MM: f64 = 2.15;
const BUNDLE_CROWN_SWELL_MM: f64 = 0.45;
/// Phase offset that puts canes 0 and 3 on the outside either side of the crown, parted round the hip.
const PHASE0: f64 = 0.75 * PI;
/// Each cane's wander: a phase drift of this many radians, twice round the ring, zero at crown and palm.
const WANDER: [f64; CANES] = [0.30, -0.22, 0.26, -0.30];
/// The cane section's five lobes, as a share of its radius.
const CANE_LOBE: f64 = 0.07;
/// Sweep stations round each closed cane (the kernel's cap).
const STATIONS: usize = 128;
/// Thorns per cane before the inward-facing ones are dropped.
const THORNS_PER_CANE: usize = 8;
/// The hip's length and width.
const HIP_L_MM: f64 = 6.5;
const HIP_W_MM: f64 = 5.0;
/// How deep each thorn's foot sinks into its cane.
const THORN_SINK_MM: f64 = 0.28;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

/// The hip: an oval ruby cabochon, its long axis round the ring.
fn ruby() -> Gem {
    Gem { l_mm: HIP_L_MM, form: GemForm::Cabochon, preview_tint: Some([0.55, 0.03, 0.08]), ..Gem::calibrated(GemCut::Oval, HIP_W_MM) }
}

/// The design with no band: size, process and the analytic seat.
fn ground() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Sentis".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.width_mm = 5.6;
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

// --- The canes ---------------------------------------------------------------------------------------------

/// Cane `i`'s phase round its bundle at `theta_deg`.
fn phase(i: usize, theta_deg: f64) -> f64 {
    let from_crown = (theta_deg - CROWN_DEG).to_radians();
    WAVES * theta_deg.to_radians() + PHASE0 + i as f64 * FRAC_PI_2 + WANDER[i] * (2.0 * from_crown).sin()
}

/// The bundle's half-depth (radial) and half-height (across) at `theta_deg`.
fn bundle(theta_deg: f64) -> (f64, f64) {
    let c = (theta_deg - CROWN_DEG).to_radians().cos();
    (BUNDLE_RADIAL_MM, BUNDLE_ACROSS_MM + BUNDLE_CROWN_SWELL_MM * c)
}

/// Cane `i`'s centre at `theta_deg`: (radius from the finger's axis, z along it).
fn cane_at(i: usize, theta_deg: f64) -> (f64, f64) {
    let (a, b) = bundle(theta_deg);
    let p = phase(i, theta_deg);
    (BUNDLE_R_MM + a * p.cos(), b * p.sin())
}

fn world(theta_deg: f64, (r, z): (f64, f64)) -> [f64; 3] {
    let (s, c) = theta_deg.to_radians().sin_cos();
    [r * c, r * s, z]
}

/// Cane `i`'s closed centreline, starting at `start_deg`, its last station its first.
fn cane_path(i: usize, start_deg: f64) -> Vec<[f64; 3]> {
    let mut path: Vec<[f64; 3]> = (0..STATIONS - 1)
        .map(|k| {
            let t = start_deg + 360.0 * k as f64 / (STATIONS - 1) as f64;
            world(t, cane_at(i, t))
        })
        .collect();
    path.push(path[0]);
    path
}

/// Cane `i`'s unit tangent at `theta_deg`, in world.
fn cane_tangent(i: usize, theta_deg: f64) -> [f64; 3] {
    let h = 0.05;
    let (a, b) = (world(theta_deg - h, cane_at(i, theta_deg - h)), world(theta_deg + h, cane_at(i, theta_deg + h)));
    unit(sub(b, a))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}

/// Cane `i`'s outward direction at `theta_deg`: away from the bundle's axis, squared to the cane, in world.
fn cane_normal(i: usize, theta_deg: f64) -> [f64; 3] {
    let (a, b) = bundle(theta_deg);
    let p = phase(i, theta_deg);
    let (n_r, n_z) = (p.cos() / a, p.sin() / b);
    let (s, c) = theta_deg.to_radians().sin_cos();
    let n = unit([n_r * c, n_r * s, n_z]);
    let t = cane_tangent(i, theta_deg);
    let d = dot(n, t);
    unit([n[0] - t[0] * d, n[1] - t[1] * d, n[2] - t[2] * d])
}

/// A part standing on cane `i` at `theta_deg`: its foot sunk `sink` mm into the cane's surface, its z along the
/// cane's outward normal there, its y round the ring, turned `spin` about its own axis.
fn on_cane(d: &RingDesign, i: usize, theta_deg: f64, sink: f64, spin: f64) -> Component {
    let n = cane_normal(i, theta_deg);
    let centre = world(theta_deg, cane_at(i, theta_deg));
    let foot: [f64; 3] = std::array::from_fn(|k| centre[k] + n[k] * (CANE_R_MM - sink));
    let (s, c) = theta_deg.to_radians().sin_cos();
    // The seat's frame: x along -Z, y round the ring, z radial.
    let (radial, tangential) = ([c, s, 0.0], [-s, c, 0.0]);
    let local = [-n[2], dot(n, tangential), dot(n, radial)];
    let tilt = (-local[1]).asin();
    let cant = local[0].atan2(local[2]);
    let r = foot[0] * c + foot[1] * s;
    let mut comp = Component::default();
    comp.placement = Placement::Ring { theta_deg, across_mm: foot[2], height_mm: r - seat_r(d), spin_deg: spin, tilt_deg: tilt.to_degrees(), cant_deg: cant.to_degrees() };
    comp.attach = Attach::Join;
    comp.stage = Stage::Cast;
    comp
}

// --- Thorns and shoots -------------------------------------------------------------------------------------

/// A thorn's centreline in the part's tangent-radial plane: `radial` mm straight out, then an arc of `bend_r`
/// turning `bend_deg` round the ring.
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

/// A rose prickle: a broad flattened foot `along` x `across` mm, swept up `radial` mm and round a `bend_r` hook of
/// `bend_deg`, tapering to `end_scale` of the foot.
#[derive(Clone, Copy)]
struct Hook {
    along: f64,
    across: f64,
    radial: f64,
    bend_r: f64,
    bend_deg: f64,
    end_scale: f64,
}

/// A briar prickle at the crown's size: a broad, flattened foot long along the cane, a short rise, a hard hook.
const THORN: Hook = Hook { along: 2.7, across: 1.0, radial: 0.35, bend_r: 1.25, bend_deg: 72.0, end_scale: 0.3 };
/// Each prickle's tip across, at least: the investment's detail with margin.
const THORN_TIP_MM: f64 = 0.42;

impl Hook {
    fn scaled(self, k: f64) -> Hook {
        Hook { along: self.along * k, across: self.across * k, radial: self.radial * k, bend_r: self.bend_r * k, end_scale: (THORN_TIP_MM / (self.across * k)).max(0.18), ..self }
    }
}

/// A closed polygon sketch on `plane`: `pts` in its x, y.
fn poly_on(plane: Workplane, name: &str, pts: &[[f64; 2]]) -> Sketch {
    let mut s = Sketch { plane, ..Sketch::default() };
    s.name = name.into();
    let ids: Vec<_> = pts.iter().map(|p| s.point(*p)).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

/// A closed polygon sketch on the default plane: `pts` in its x, y.
fn polygon(name: &str, pts: &[[f64; 2]]) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    let ids: Vec<_> = pts.iter().map(|p| s.point(*p)).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

/// An ellipse `a` mm along the hook's turn (round the ring) and `b` mm across it.
fn ellipse(a: f64, b: f64) -> Sketch {
    let pts: Vec<[f64; 2]> = (0..24).map(|k| {
        let t = TAU * k as f64 / 24.0;
        [0.5 * b * t.cos(), 0.5 * a * t.sin()]
    }).collect();
    polygon("Thorn foot", &pts)
}

fn thorn(h: Hook) -> Operation {
    Operation::Twist { sketch: ellipse(h.along, h.across).into(), path: hook_path(h.radial, h.bend_r, h.bend_deg), degrees: 0.0, end_scale: h.end_scale }
}

/// A five-point star `across` mm over its points, its valleys at `inner` of that.
fn star(across: f64, inner: f64) -> Sketch {
    let pts: Vec<[f64; 2]> = (0..10).map(|k| {
        let t = TAU * k as f64 / 10.0;
        let r = 0.5 * across * if k % 2 == 0 { 1.0 } else { inner };
        [r * t.cos(), r * t.sin()]
    }).collect();
    polygon("Shoot section", &pts)
}

/// A small deterministic hash in [0, 1).
fn hash(k: u64) -> f64 {
    let mut x = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Where a thorn stands: cane, angle, size and which way it hooks.
#[derive(Clone, Copy)]
struct ThornAt {
    cane: usize,
    theta: f64,
    scale: f64,
    spin: f64,
}

/// The thorns: along each cane at an uneven pitch, larger at the crown, dropped where the cane faces the finger
/// or runs under the hip.
fn thorn_sites() -> Vec<ThornAt> {
    let mut out = Vec::new();
    for i in 0..CANES {
        for k in 0..THORNS_PER_CANE {
            let seed = (i * 100 + k) as u64;
            let theta = (17.0 * i as f64 + 360.0 * (k as f64 + 0.35 * (hash(seed) - 0.5)) / THORNS_PER_CANE as f64).rem_euclid(360.0);
            let n = cane_normal(i, theta);
            let (s, c) = theta.to_radians().sin_cos();
            let out_r = n[0] * c + n[1] * s;
            // Facing the finger, or tucked under the hip's footprint.
            let from_crown = ((theta - CROWN_DEG + 540.0).rem_euclid(360.0) - 180.0).abs();
            if out_r < -0.25 || from_crown < 11.0 {
                continue;
            }
            // Crown prickles 1.4 times the palm's; every prickle on a cane hooks back the way that cane grew.
            let crown = 0.5 + 0.5 * (theta - CROWN_DEG).to_radians().cos();
            let scale = 0.75 + 0.3 * crown + 0.06 * (hash(seed + 7) - 0.5);
            let spin = if i % 2 == 0 { 0.0 } else { 180.0 };
            out.push(ThornAt { cane: i, theta, scale, spin });
        }
    }
    out
}

/// The outermost cane at `theta_deg`.
fn outer_cane(theta_deg: f64) -> usize {
    (0..CANES).max_by(|a, b| cane_at(*a, theta_deg).0.total_cmp(&cane_at(*b, theta_deg).0)).unwrap_or(0)
}

/// The cane nearest `z` across the finger, among those on the outside half of the bundle at `theta_deg`.
fn cane_near(theta_deg: f64, z: f64) -> usize {
    (0..CANES)
        .filter(|i| cane_at(*i, theta_deg).0 >= BUNDLE_R_MM - 0.1)
        .min_by(|a, b| (cane_at(*a, theta_deg).1 - z).abs().total_cmp(&(cane_at(*b, theta_deg).1 - z).abs()))
        .unwrap_or_else(|| outer_cane(theta_deg))
}

/// The cane at `theta_deg` whose surface faces most nearly outward, preferring those near `z` across the finger.
fn top_cane(theta_deg: f64, z: f64) -> usize {
    let (s, c) = theta_deg.to_radians().sin_cos();
    let out = |i: usize| {
        let n = cane_normal(i, theta_deg);
        n[0] * c + n[1] * s
    };
    let score = |i: usize| out(i) - 0.15 * (cane_at(i, theta_deg).1 - z).abs();
    (0..CANES).max_by(|a, b| score(*a).total_cmp(&score(*b))).unwrap_or(0)
}

// --- The hip, its calyx and the leaves --------------------------------------------------------------------

type P3 = [f64; 3];

/// An ellipse polygon of `n` points, `ax` mm half across x and `ay` half along y, on the plane `h` mm up z.
fn ellipse_at(name: &str, ax: f64, ay: f64, h: f64, n: usize) -> Sketch {
    let mut s = Sketch { plane: Workplane { origin: [0.0, 0.0, h], ..Default::default() }, ..Sketch::default() };
    s.name = name.into();
    let ids: Vec<_> = (0..n).map(|k| {
        let t = TAU * k as f64 / n as f64;
        s.point([ax * t.cos(), ay * t.sin()])
    }).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

/// The hip's receptacle: an oval urn lofted under the stone, its flat top at the seat radius, its foot down among
/// the canes; long round the ring, as the hip is.
fn hip_cup() -> Operation {
    let sections = HIP_URN.iter().map(|&(h, ax, ay)| ellipse_at("Hip urn", ax, ay, h, 40).into()).collect();
    Operation::Loft { sections }
}

/// The urn's sections: height under the seat, half across the finger, half round the ring.
const HIP_URN: [(f64, f64, f64); 5] = [(-2.3, 1.05, 1.3), (-1.6, 1.75, 2.25), (-0.75, 2.25, 2.95), (-0.22, 2.4, 3.12), (-0.05, 2.37, 3.07)];

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "vepres_sentis".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

/// `s` turned outward if its signed volume says it was built inside out.
fn outward(mut s: csg::Solid) -> csg::Solid {
    let vol: f64 = s.f.iter().map(|f| {
        let (a, b, c) = (s.v[f[0] as usize], s.v[f[1] as usize], s.v[f[2] as usize]);
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
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

/// The dried calyx at the hip's tip: five slender sepals radiating from a hub `outer` mm across their points, each
/// thinning from `root` to `tip` mm and curling `curl` mm back away from the hip; its axis along z.
fn calyx_solid(outer: f64, inner: f64, root: f64, tip: f64, curl: f64) -> csg::Solid {
    let (n, k) = (60usize, 4usize);
    let rim = |a: f64| {
        let lobe = (0.5 + 0.5 * (5.0 * a).cos()).powf(3.0);
        inner + (0.5 * outer - inner) * lobe
    };
    let rmax = 0.5 * outer;
    let mid = |r: f64| curl * (r / rmax).powi(2);
    let thick = |r: f64| root + (tip - root) * (r / rmax);
    let mut s = csg::Solid::default();
    s.v.push([0.0, 0.0, mid(0.0) - 0.5 * root]);
    s.v.push([0.0, 0.0, mid(0.0) + 0.5 * root]);
    let idx = |face: usize, ring: usize, j: usize| (2 + face * k * n + (ring - 1) * n + j % n) as u32;
    for face in 0..2 {
        for ring in 1..=k {
            for j in 0..n {
                let a = TAU * j as f64 / n as f64;
                let r = rim(a) * ring as f64 / k as f64;
                let z = mid(r) + if face == 0 { -0.5 } else { 0.5 } * thick(r);
                s.v.push([r * a.cos(), r * a.sin(), z]);
            }
        }
    }
    for j in 0..n {
        s.f.push([0, idx(0, 1, j + 1), idx(0, 1, j)]);
        s.f.push([1, idx(1, 1, j), idx(1, 1, j + 1)]);
    }
    for ring in 1..k {
        for j in 0..n {
            s.f.push([idx(0, ring, j), idx(0, ring, j + 1), idx(0, ring + 1, j + 1)]);
            s.f.push([idx(0, ring, j), idx(0, ring + 1, j + 1), idx(0, ring + 1, j)]);
            s.f.push([idx(1, ring, j), idx(1, ring + 1, j + 1), idx(1, ring, j + 1)]);
            s.f.push([idx(1, ring, j), idx(1, ring + 1, j), idx(1, ring + 1, j + 1)]);
        }
    }
    for j in 0..n {
        s.f.push([idx(0, k, j), idx(0, k, j + 1), idx(1, k, j + 1)]);
        s.f.push([idx(0, k, j), idx(1, k, j + 1), idx(1, k, j)]);
    }
    outward(s)
}

/// A briar leaflet in its leaf's plan: from `origin` along heading `alpha` (radians from +y toward +x), `len` long
/// and `wide` across, its serrate margin's teeth leaning forward, its top domed from a raised midrib down to the
/// margin, lifting `lift` radians as it runs out; its underside sunk `sink` mm at the foot.
fn leaflet_solid(origin: [f64; 2], alpha: f64, len: f64, wide: f64, lift: f64, sink: f64) -> csg::Solid {
    let (steps, across) = (40usize, 6usize);
    let (edge, rib, keel) = (LEAF_EDGE_MM, LEAF_RIB_MM, 0.12);
    let d = [alpha.sin(), alpha.cos()];
    let perp = [d[1], -d[0]];
    let half = |u: f64| {
        let body = (PI * u).sin().powf(0.7) * (1.0 - 0.25 * u);
        let t = (u * LEAF_TEETH as f64).fract();
        0.5 * wide * body * (1.0 - LEAF_TOOTH * t * (u > 0.12 && u < 0.94) as u8 as f64)
    };
    let (u0, u1) = (0.03, 0.95);
    let centre = |u: f64| -> P3 { [origin[0] + d[0] * u * len, origin[1] + d[1] * u * len, u * len * lift.tan()] };
    let loops: Vec<Vec<P3>> = (0..=steps).map(|i| {
        let u = u0 + (u1 - u0) * i as f64 / steps as f64;
        let (c, w) = (centre(u), half(u).max(0.16));
        // The foot sinks into the cane and rises clear of it within the first fifth of the leaflet.
        let under = sink * (1.0 - u / 0.2).max(0.0);
        let mut l = Vec::with_capacity(2 * across + 2);
        for j in 0..=across {
            let v = -1.0 + 2.0 * j as f64 / across as f64;
            let top = edge + (rib - edge) * (1.0 - v.abs()).powf(1.3);
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

/// Leaflet thickness at the margin and over the midrib; teeth per leaflet and their depth as a share of the width.
const LEAF_EDGE_MM: f64 = 0.3;
const LEAF_RIB_MM: f64 = 0.62;
const LEAF_TEETH: usize = 6;
const LEAF_TOOTH: f64 = 0.16;

/// The shoots' radius, rise before they lean, and their end's share of the foot.
const SHOOT_R_MM: f64 = 0.62;
const SHOOT_RISE_MM: f64 = 2.8;
const SHOOT_END: f64 = 0.78;
/// The shoots: angle, the side of the bundle they stand from, and which way they lean.
const SHOOTS: [(f64, f64, f64); 3] = [(CROWN_DEG - 30.0, 1.4, 180.0), (CROWN_DEG + 31.0, -1.4, 0.0), (CROWN_DEG + 44.0, 1.6, 0.0)];
/// The leaves: angle, side of the bundle, heading in the leaf's plan (degrees from round the ring, +theta, toward b = t x n).
const LEAVES: [(f64, f64, f64); 3] = [(CROWN_DEG - 21.0, 0.8, 190.0), (CROWN_DEG + 24.0, -0.8, 10.0), (CROWN_DEG - 45.0, -0.8, 168.0)];
/// The hip's girdle over the seat, and its tilt up out of the thicket.
const HIP_H_MM: f64 = 1.4;
/// How far the calyx turns up past the hip's own axis.
const CALYX_RISE_DEG: f64 = 40.0;
const HIP_TILT_DEG: f64 = 28.0;
/// The calyx across its sepals' points; the stalk's radius.
const CALYX_MM: f64 = 4.4;
const STALK_R_MM: f64 = 0.42;

/// The hip's frame: its girdle `HIP_H_MM` over the seat at the crown, its long axis round the ring, tilted up out of
/// the thicket so its calyx end rises toward the eye and its stalk end dips into the canes.
fn hip_placement() -> Placement {
    Placement::Ring { theta_deg: CROWN_DEG, across_mm: 0.0, height_mm: HIP_H_MM, spin_deg: 0.0, tilt_deg: HIP_TILT_DEG, cant_deg: 0.0 }
}

/// A point of the hip's frame in world millimetres.
fn hip_world(d: &RingDesign, p: P3) -> P3 {
    hip_placement().world(d, p).unwrap_or(p)
}

/// The hip's stalk: from inside the receptacle's low end, out and down in a curve into the cane below.
fn stalk_path(d: &RingDesign) -> Vec<[f64; 3]> {
    let p0 = hip_world(d, [0.0, -1.3, -1.2]);
    let mid = hip_world(d, [0.0, -3.4, -1.3]);
    let theta1 = CROWN_DEG - 22.0;
    let cane = cane_near(theta1, 0.0);
    let p1 = world(theta1, cane_at(cane, theta1));
    (0..=12)
        .map(|k| {
            let t = k as f64 / 12.0;
            std::array::from_fn(|i| (1.0 - t) * (1.0 - t) * p0[i] + 2.0 * t * (1.0 - t) * mid[i] + t * t * p1[i])
        })
        .collect()
}

// --- The document ------------------------------------------------------------------------------------------

struct Authored {
    design: RingDesign,
    thorns: usize,
    stone_id: u64,
}

fn author() -> Result<Authored> {
    let mut d = ground();
    let mut doc = Document::default();
    let mut next = 0u64;
    let mut add = |doc: &mut Document, name: String, operation: Operation, component: Component| -> Result<u64> {
        next += 1;
        doc.append(Feature { id: next, name, enabled: true, operation, component })?;
        Ok(next)
    };
    let free = Component { attach: Attach::Join, stage: Stage::Cast, role: ComponentRole::Shank, ..Component::default() };
    let liner_major = d.inner_radius_mm() + LINER_MINOR_MM;
    add(&mut doc, "Heartwood liner".into(), Operation::Torus { major_mm: liner_major, minor_mm: LINER_MINOR_MM }, free.clone())?;
    for i in 0..CANES {
        let start = 250.0 + 13.0 * i as f64;
        // A briar cane is faintly angled: five low lobes round its section.
        let lobed: Vec<[f64; 2]> = (0..40).map(|k| {
            let t = TAU * k as f64 / 40.0;
            let r = CANE_R_MM * (1.0 + CANE_LOBE * (5.0 * t).cos());
            [r * t.cos(), r * t.sin()]
        }).collect();
        let section = polygon("Cane section", &lobed);
        add(&mut doc, format!("Briar cane {}", i + 1), Operation::Sweep { sketch: section.into(), path: cane_path(i, start) }, free.clone())?;
    }
    let sites = thorn_sites();
    for (k, t) in sites.iter().enumerate() {
        add(&mut doc, format!("Thorn {} on cane {}", k + 1, t.cane + 1), thorn(THORN.scaled(t.scale)), on_cane(&d, t.cane, t.theta, THORN_SINK_MM * t.scale, t.spin))?;
    }
    // Three cut shoots standing up out of the tangle round the hip, each leaning away from it, its end pruned
    // on a slant by a cut in its own frame.
    for (k, (theta, z, spin)) in SHOOTS.into_iter().enumerate() {
        let cane = top_cane(theta, z);
        let (rise, bend_r, bend) = (SHOOT_RISE_MM, 4.0, 16f64.to_radians());
        let op = Operation::Twist { sketch: Sketch::circle(SHOOT_R_MM).into(), path: hook_path(rise, bend_r, bend.to_degrees()), degrees: 0.0, end_scale: SHOOT_END };
        let at = on_cane(&d, cane, theta, 0.35, spin);
        add(&mut doc, format!("Cut shoot {}", k + 1), op, at.clone())?;
        // The end and its heading in the shoot's frame (y round the ring, z out), then the slanted cut plane.
        let tip = [0.0, bend_r * (1.0 - bend.cos()), rise + bend_r * bend.sin()];
        let slant = bend + 38f64.to_radians();
        let n = [0.0, slant.sin(), slant.cos()];
        let o: P3 = std::array::from_fn(|i| tip[i] - [0.0, bend.sin(), bend.cos()][i] * 0.3);
        let plane = Workplane { origin: o, x: [1.0, 0.0, 0.0], y: [0.0, n[2], -n[1]], ..Default::default() };
        let cutter = poly_on(plane, "Pruning cut", &[[-1.5, -1.5], [1.5, -1.5], [1.5, 1.5], [-1.5, 1.5]]);
        add(&mut doc, format!("Cut shoot {}: pruning cut", k + 1), Operation::Extrude { sketch: cutter.into(), height_mm: 2.0, draft_deg: 0.0 }, Component { attach: Attach::Cut, ..at })?;
    }
    // The hip: its receptacle among the canes, the stalk it hangs from, the dried calyx at its tip.
    let hip_frame = Component { attach: Attach::Join, stage: Stage::Cast, placement: hip_placement(), ..Component::default() };
    add(&mut doc, "Hip receptacle".into(), hip_cup(), hip_frame.clone())?;
    add(&mut doc, "Hip stalk".into(), Operation::Sweep { sketch: Sketch::circle(STALK_R_MM).into(), path: stalk_path(&d) }, free.clone())?;
    let calyx = calyx_solid(CALYX_MM, 0.55, 0.65, 0.36, 0.9);
    // The calyx's axis along the ring (+y of the hip's frame), its hub just past the stone's tip.
    let (hub_y, hub_h) = (0.5 * HIP_L_MM + 0.55, 0.25);
    // Turned a further CALYX_RISE_DEG up out of the hip's axis, so the star of sepals faces the eye from above.
    let (sb, cb) = CALYX_RISE_DEG.to_radians().sin_cos();
    let calyx = csg::Solid { v: calyx.v.iter().map(|p| { let (y, z) = (p[2], -p[1]); [p[0], y * cb - z * sb + hub_y, y * sb + z * cb + hub_h] }).collect(), f: calyx.f };
    add(&mut doc, "Hip calyx".into(), stored_op(&calyx, "calyx", json!({"across_mm": CALYX_MM, "sepals": 5}))?, hip_frame)?;
    // Pinnate briar leaves beside the hip: a terminal leaflet and a pair of laterals from one foot.
    for (k, &(theta, z, alpha_deg)) in LEAVES.iter().enumerate() {
        let cane = top_cane(theta, z);
        let a = alpha_deg.to_radians();
        let d0 = [a.sin(), a.cos()];
        let lift = 7f64.to_radians();
        let parts = [
            ("terminal", leaflet_solid([0.0, 0.0], a, 3.8, 2.0, lift, 0.32)),
            ("left", leaflet_solid([d0[0] * 0.95, d0[1] * 0.95], a + 0.85, 2.9, 1.6, lift * 1.2, 0.27)),
            ("right", leaflet_solid([d0[0] * 1.0, d0[1] * 1.0], a - 0.85, 2.9, 1.6, lift * 0.85, 0.37)),
        ];
        // The leaf's plan laid on the cane: z out along its normal, y round the ring toward the leaf's heading.
        let n = cane_normal(cane, theta);
        let (sn, cs) = theta.to_radians().sin_cos();
        let ring = [-sn, cs, 0.0];
        let d_ = dot(ring, n);
        let t = unit([ring[0] - n[0] * d_, ring[1] - n[1] * d_, ring[2] - n[2] * d_]);
        let b = [t[1] * n[2] - t[2] * n[1], t[2] * n[0] - t[0] * n[2], t[0] * n[1] - t[1] * n[0]];
        let c = world(theta, cane_at(cane, theta));
        let foot: P3 = std::array::from_fn(|i| c[i] + n[i] * CANE_R_MM);
        for (name, solid) in parts {
            let placed = csg::Solid { v: solid.v.iter().map(|p| std::array::from_fn(|i| foot[i] + b[i] * p[0] + t[i] * p[1] + n[i] * p[2])).collect(), f: solid.f };
            add(&mut doc, format!("Briar leaf {}, {name} leaflet", k + 1), stored_op(&placed, "leaflet", json!({"leaf": k + 1, "leaflet": name}))?, free.clone())?;
        }
    }
    let gem = ruby();
    let stand = gem.pavilion_mm() + 0.05;
    next += 1;
    let stone_id = next;
    let _ = stand;
    doc.append(builders::stone_feature(stone_id, gem, hip_placement()))?;
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
    name.contains("Thorn") || name.contains("calyx") || name.contains("leaflet")
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

/// The faces of `m` whose every corner lies within `half_deg` of `theta` round the ring and `over` mm out.
fn wedge(m: &mesh::Mesh, theta: f64, half_deg: f64, over: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        let t = (p.1 as f64).atan2(p.0 as f64).to_degrees();
        let d = (t - theta + 540.0).rem_euclid(360.0) - 180.0;
        d.abs() <= half_deg && (p.0 as f64).hypot(p.1 as f64) >= over
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals[i as usize]);
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
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
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The hip close: the crown's metal round it, and the stone.
    let near = wedge(&finished.metal, CROWN_DEG, 32.0, 0.0);
    let mut close = vec![render::Part::metal(&near, render::GOLD)];
    close.extend(finished.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    render::write_png_parts(out.join("stones.png"), &close, -0.45, 0.85, edge)?;
    // Bare against finished: the heartwood liner alone beside the thicket.
    let liner = wedge(&finished.metal, 0.0, 181.0, 0.0);
    let _ = liner;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let mut bare_doc = ground();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Heartwood liner".into(), enabled: true, operation: Operation::Torus { major_mm: bare_doc.inner_radius_mm() + LINER_MINOR_MM, minor_mm: LINER_MINOR_MM }, component: Component { attach: Attach::Join, ..Component::default() } })?;
    bare_doc.cad = Some(doc);
    let bare = mesh::try_build(&bare_doc, &AlphaLibrary::builtin(), draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    render::write_png_parts(out.join("face-300.png"), &parts, VIEWS[1].1, VIEWS[1].2, 300)?;
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
    let Authored { design: d, thorns, stone_id } = author()?;
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
        ("solids and parts notes empty, every feature Ok", solids_notes.is_empty() && parts_notes.is_empty() && status.iter().all(|(_, s)| s == "Ok")),
        ("nothing enters the finger hole", inside == 0),
        ("every part's ray-sampled wall at the 0.8 mm section, its pointed details (thorns, shoots) at the 0.15 mm detail floor", walls_ok(&wall)),
        ("lost-wax verdict Castable with the 0.8 mm section", field.process == castability::CastProcess::LostWax && field.verdict == castability::Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM),
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
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings},
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

