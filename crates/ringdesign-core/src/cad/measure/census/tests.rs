use super::super::{CensusOptions, ThinKind, Thickness, census, census_until, thickness};
use super::{MIN_FACE_HEIGHT_MM, Probe, Reading, pitch_for, sample};
use crate::mesh::{Mesh, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// A census's reads of its cancel flag on one thread while recorded.
#[derive(Default)]
struct Checks {
    /// When each read was made.
    at: Vec<Instant>,
    /// The read at which the census raises its own flag.
    raise_at: Option<usize>,
}

thread_local! {
    static CHECKS: std::cell::RefCell<Option<Checks>> = const { std::cell::RefCell::new(None) };
}

/// Records this thread's reads of the cancel flag until [`checks`], the census raising its own flag as it makes read `raise_at`.
fn record_checks(raise_at: Option<usize>) {
    CHECKS.with(|c| *c.borrow_mut() = Some(Checks { at: Vec::new(), raise_at }));
}

/// When this thread's reads were made since [`record_checks`]; ends the record.
fn checks() -> Vec<Instant> {
    CHECKS.with(|c| c.borrow_mut().take().map_or_else(Vec::new, |c| c.at))
}

/// Notes a read of the cancel flag on this thread, raising it if it is the armed read.
pub(super) fn checked(cancel: Option<&AtomicBool>) {
    CHECKS.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            if c.raise_at == Some(c.at.len())
                && let Some(flag) = cancel
            {
                flag.store(true, Ordering::Relaxed);
            }
            c.at.push(Instant::now());
        }
    });
}

const FLOOR: f64 = 0.8;

fn mesh_of(points: &[[f64; 3]], faces: Vec<[u32; 3]>) -> Mesh {
    let m = Mesh {
        vertices: points.iter().map(|p| Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces,
        ..Default::default()
    };
    assert!(m.validate().watertight, "test solid is not closed");
    // Every edge runs once each way, so the faces agree, and they face out.
    let mut directed: Vec<(u32, u32)> = m.faces.iter().flat_map(|f| [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])]).collect();
    directed.sort_unstable();
    assert!(directed.windows(2).all(|w| w[0] != w[1]), "test solid's faces disagree");
    assert!(m.volume_mm3() > 0.0, "test solid faces in");
    m
}

/// Ear clipping of a simple counter-clockwise polygon.
fn ears(poly: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut left: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::new();
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n)
            .find(|&i| {
                let (a, b, c) = (left[(i + n - 1) % n], left[i], left[(i + 1) % n]);
                cross(poly[a], poly[b], poly[c]) > 1e-12
                    && left.iter().all(|&j| {
                        j == a || j == b || j == c || {
                            let p = poly[j];
                            !(cross(poly[a], poly[b], p) >= 0.0 && cross(poly[b], poly[c], p) >= 0.0 && cross(poly[c], poly[a], p) >= 0.0)
                        }
                    })
            })
            .expect("polygon has an ear");
        out.push([left[(ear + n - 1) % n], left[ear], left[(ear + 1) % n]]);
        left.remove(ear);
    }
    out.push([left[0], left[1], left[2]]);
    out
}

/// A counter-clockwise outline in (x, z) swept along y from 0 to `depth`.
fn prism(outline: &[[f64; 2]], depth: f64) -> Mesh {
    let n = outline.len() as u32;
    let mut points: Vec<[f64; 3]> = outline.iter().map(|p| [p[0], 0.0, p[1]]).collect();
    points.extend(outline.iter().map(|p| [p[0], depth, p[1]]));
    let mut faces = Vec::new();
    for t in ears(outline) {
        let t = t.map(|i| i as u32);
        faces.push(t);
        faces.push([t[0] + n, t[2] + n, t[1] + n]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push([i, j + n, j]);
        faces.push([i, i + n, j + n]);
    }
    mesh_of(&points, faces)
}

/// A counter-clockwise profile in (r, z) turned about the z axis; a point at r = 0 is a pole.
fn lathe(profile: &[[f64; 2]], segments: u32) -> Mesh {
    let mut points = Vec::new();
    let ring: Vec<Vec<u32>> = profile
        .iter()
        .map(|p| {
            if p[0] == 0.0 {
                points.push([0.0, 0.0, p[1]]);
                vec![points.len() as u32 - 1; segments as usize]
            } else {
                (0..segments)
                    .map(|k| {
                        let a = std::f64::consts::TAU * k as f64 / segments as f64;
                        points.push([p[0] * a.cos(), p[0] * a.sin(), p[1]]);
                        points.len() as u32 - 1
                    })
                    .collect()
            }
        })
        .collect();
    let mut faces = Vec::new();
    for i in 0..profile.len() {
        let j = (i + 1) % profile.len();
        for k in 0..segments as usize {
            let l = (k + 1) % segments as usize;
            let (a, b, c, d) = (ring[i][k], ring[j][k], ring[j][l], ring[i][l]);
            if b != c {
                faces.push([a, c, b]);
            }
            if a != d {
                faces.push([a, d, c]);
            }
        }
    }
    mesh_of(&points, faces)
}

fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Mesh {
    let outline = [[lo[0], lo[2]], [hi[0], lo[2]], [hi[0], hi[2]], [lo[0], hi[2]]];
    let mut m = prism(&outline, hi[1] - lo[1]);
    for v in &mut m.vertices {
        v.1 += lo[1] as f32;
    }
    m
}

/// Several closed solids as one mesh, each its own shell.
fn shells(parts: &[Mesh]) -> Mesh {
    let mut m = Mesh::default();
    for p in parts {
        let base = m.vertices.len() as u32;
        m.vertices.extend(&p.vertices);
        m.faces.extend(p.faces.iter().map(|f| f.map(|i| i + base)));
    }
    m
}

fn at_floor(m: &Mesh) -> Thickness {
    let t = census(m, &CensusOptions::floor(FLOOR));
    assert!(t.assessed && t.unresolved == 0, "{t:?}");
    t
}

#[test]
fn a_knife_edged_plate_reads_edges_only() {
    // A 2 mm plate bevelled over its last 1.5 mm to a knife at x = 8.
    let plate = prism(&[[0.0, -1.0], [6.5, -1.0], [8.0, 0.0], [6.5, 1.0], [0.0, 1.0]], 6.0);
    let t = at_floor(&plate);
    assert!(t.clean(), "{t:?}");
    assert!(t.walls.is_empty() && t.below_limit == 0 && t.wall_area_mm2 == 0.0);
    assert!(t.edge_below_limit > 0 && !t.edges.is_empty());
    assert!(t.sampled_min_mm.unwrap() < 0.15, "{:?}", t.sampled_min_mm);
    // The section closes at the knife and reaches the floor 0.6 mm back from it.
    for z in &t.edges {
        assert!(z.point[0] > 8.0 - FLOOR, "{z:?}");
        assert!(z.depth_mm.unwrap() < 0.75, "{z:?}");
    }
    // Two bevels, each a strip about a third of a millimetre wide reading under the floor.
    assert!(t.edge_area_mm2 > 2.0 * 6.0 * 0.25 && t.edge_area_mm2 < 2.0 * 6.0 * 0.45, "{}", t.edge_area_mm2);
    assert!(t.rays > 1_000 && t.internal == 0);
}

#[test]
fn a_long_taper_is_a_wall() {
    // The same plate bevelled over 4 mm: the section stays under the floor 1.6 mm back from the knife.
    let plate = prism(&[[0.0, -1.0], [4.0, -1.0], [8.0, 0.0], [4.0, 1.0], [0.0, 1.0]], 6.0);
    let t = at_floor(&plate);
    assert!(!t.clean() && t.below_limit > 0, "{t:?}");
    assert!(t.walls.iter().all(|z| z.point[0] > 6.0), "{:?}", t.walls);
    // With the reach opened to two floors it is an edge again.
    let wide = census(&plate, &CensusOptions { edge_reach_mm: Some(2.0 * FLOOR), ..CensusOptions::floor(FLOOR) });
    assert!(wide.clean(), "{wide:?}");
}

#[test]
fn a_web_between_two_blocks_is_a_wall() {
    // Two 3 x 4 x 4 blocks joined by a 0.5 mm web spanning 3 mm.
    let h = [
        [-3.0, -2.0],
        [0.0, -2.0],
        [0.0, -0.25],
        [3.0, -0.25],
        [3.0, -2.0],
        [6.0, -2.0],
        [6.0, 2.0],
        [3.0, 2.0],
        [3.0, 0.25],
        [0.0, 0.25],
        [0.0, 2.0],
        [-3.0, 2.0],
    ];
    let t = at_floor(&prism(&h, 4.0));
    assert!(!t.clean() && t.below_limit > 0, "{t:?}");
    let web = 2.0 * 3.0 * 4.0;
    assert!(t.wall_area_mm2 > 0.8 * web && t.wall_area_mm2 < 1.05 * web, "{} of {web} mm²", t.wall_area_mm2);
    assert!((t.walls[0].thinnest_mm - 0.5).abs() < 0.01, "{:?}", t.walls[0]);
    for p in t.walls.iter().map(|z| z.point).chain(t.edges.iter().map(|z| z.point)) {
        assert!(p[0] > -1e-6 && p[0] < 3.0 + 1e-6 && p[2].abs() < 0.25 + 1e-6, "{p:?}");
    }
}

#[test]
fn a_cone_tip_is_an_edge() {
    // A 3 mm rod topped by a cone of 35° half-angle: 0.8 mm across 0.57 mm below the tip.
    let tip = 3.0 + 1.5 / 35f64.to_radians().tan();
    let rod = lathe(&[[0.0, 0.0], [1.5, 0.0], [1.5, 3.0], [0.0, tip]], 96);
    let t = at_floor(&rod);
    assert!(t.clean() && t.walls.is_empty(), "{t:?}");
    assert!(!t.edges.is_empty() && t.edges.iter().all(|z| z.point[2] > tip - 0.75), "{:?}", t.edges);
    assert!(t.edges.iter().all(|z| z.depth_mm.unwrap() < 0.7), "{:?}", t.edges);
}

#[test]
fn a_thin_lipped_collet_reads_its_lip_as_an_edge_and_its_body_clean() {
    // A 1 mm wall whose outside closes in over its top half millimetre to a 0.25 mm lip.
    let collet = lathe(&[[2.0, 0.0], [3.0, 0.0], [3.0, 2.5], [2.25, 3.0], [2.0, 3.0]], 128);
    let t = at_floor(&collet);
    assert!(t.clean() && t.walls.is_empty(), "{t:?}");
    assert!(!t.edges.is_empty());
    for z in &t.edges {
        assert!(z.point[2] > 2.55, "{z:?}");
    }
    // A 0.4 mm lip standing 0.4 mm proud is an edge; the same lip standing 1.4 mm is a wall.
    let short = at_floor(&lathe(&[[2.0, 0.0], [3.0, 0.0], [3.0, 2.6], [2.4, 2.6], [2.4, 3.0], [2.0, 3.0]], 128));
    assert!(short.clean() && !short.edges.is_empty(), "{short:?}");
    let tall = at_floor(&lathe(&[[2.0, 0.0], [3.0, 0.0], [3.0, 1.6], [2.4, 1.6], [2.4, 3.0], [2.0, 3.0]], 128));
    assert!(!tall.clean(), "{tall:?}");
    assert!(tall.walls.iter().all(|z| z.point[2] > 1.55), "{:?}", tall.walls);
    assert!(tall.wall_area_mm2 > 0.8 * (std::f64::consts::TAU * (2.0 + 2.4) * 1.4), "{}", tall.wall_area_mm2);
}

#[test]
fn shells_that_overlap_or_touch_do_not_read_their_contact() {
    let a = cuboid([0.0, 0.0, 0.0], [4.0, 4.0, 2.0]);
    for z0 in [1.99, 2.0] {
        let b = cuboid([0.5, 0.5, z0], [3.5, 3.5, 4.0]);
        let both = shells(&[a.clone(), b]);
        // A ray up from the second block's floor meets the first block's top: the 0.01 mm a face-by-face read reports.
        let bvh = crate::interaction::bvh::Bvh::build(&both);
        let (_, naive) = bvh.ray(&both, [2.0, 2.0, z0], [0.0, 0.0, 1.0]).unwrap();
        let expected = if z0 < 2.0 { 2.0 - z0 } else { 2.0 };
        assert!((naive - expected).abs() < 1e-5, "{naive}");
        let t = at_floor(&both);
        assert!(t.internal > 0, "{t:?}");
        assert!(t.clean() && t.edges.is_empty() && t.walls.is_empty(), "{t:?}");
        assert!(t.sampled_min_mm.unwrap() > 2.0 - 1e-3, "{t:?}");
    }
}

#[test]
fn a_ray_that_starts_on_a_face_never_reads_that_face() {
    // Every sample of a box, read one at a time: nothing nearer than its true section.
    let boxed = cuboid([0.0, 0.0, 0.0], [3.0, 2.0, 1.0]);
    let probe = Probe::new(&boxed, FLOOR, FLOOR);
    for s in sample(&boxed, 0.1, None).unwrap() {
        let Reading::Section { t, .. } = probe.read(&s) else { panic!("{s:?}") };
        assert!(t > 1.0 - 1e-6, "{s:?} read {t}");
    }
    let total: f64 = sample(&boxed, 0.1, None).unwrap().iter().map(|s| s.area).sum();
    assert!((total - 2.0 * (6.0 + 3.0 + 2.0)).abs() < 1e-9, "{total}");
}

/// A 2 mm prism whose outline has a 20° ridge between a 3 mm face and a face 1.3e-4 mm across, and that outline as the mesh holds it.
fn crease_wedge() -> (Mesh, Vec<[f64; 2]>) {
    let dir = |deg: f64| [deg.to_radians().cos(), deg.to_radians().sin()];
    let (u2, u3, u0, u1) = (dir(26.0), dir(120.0), dir(215.0), dir(226.0));
    let (l2, h) = (3.0, 1.3e-4);
    // Lengths of the two closing edges, from l2 * u2 + l3 * u3 + l0 * u0 + h * u1 = 0.
    let rhs = [-(l2 * u2[0] + h * u1[0]), -(l2 * u2[1] + h * u1[1])];
    let det = u3[0] * u0[1] - u0[0] * u3[1];
    let (l3, l0) = ((rhs[0] * u0[1] - u0[0] * rhs[1]) / det, (u3[0] * rhs[1] - rhs[0] * u3[1]) / det);
    assert!(l3 > 0.0 && l0 > 0.0, "{l3} {l0}");
    let r = [0.0, 0.0];
    let b = [r[0] + l2 * u2[0], r[1] + l2 * u2[1]];
    let q = [b[0] + l3 * u3[0], b[1] + l3 * u3[1]];
    let s = [q[0] + l0 * u0[0], q[1] + l0 * u0[1]];
    let outline: Vec<[f64; 2]> = [r, b, q, s].iter().map(|p| p.map(|v| v as f32 as f64)).collect();
    (prism(&outline, 2.0), outline)
}

/// The exit along a side-face sample's inward normal from the outline itself: the nearest outline edge it crosses past its start.
fn outline_section(outline: &[[f64; 2]], o: [f64; 2], d: [f64; 2]) -> f64 {
    let mut best = f64::INFINITY;
    for i in 0..outline.len() {
        let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let den = d[0] * e[1] - d[1] * e[0];
        if den.abs() < 1e-300 {
            continue;
        }
        let w = [a[0] - o[0], a[1] - o[1]];
        let (t, u) = ((w[0] * e[1] - w[1] * e[0]) / den, (w[0] * d[1] - w[1] * d[0]) / den);
        if t > 1e-12 && (-1e-12..=1.0 + 1e-12).contains(&u) {
            best = best.min(t);
        }
    }
    best
}

#[test]
fn a_ray_beside_an_acute_crease_reads_its_true_section_alone_or_among_shells() {
    let (wedge, outline) = crease_wedge();
    // The ridge's short face is a face whose normal is trusted, and the only one facing -x.
    let ridge = [outline[3], outline[0]];
    let across = ((ridge[1][0] - ridge[0][0]).powi(2) + (ridge[1][1] - ridge[0][1]).powi(2)).sqrt();
    assert!(across > MIN_FACE_HEIGHT_MM && across < 1.5 * MIN_FACE_HEIGHT_MM, "{across}");
    let far = cuboid([10.0, 0.0, 0.0], [12.0, 2.0, 2.0]);
    for (mode, mesh) in [("alone", wedge.clone()), ("among shells", shells(&[wedge, far]))] {
        let probe = Probe::new(&mesh, FLOOR, FLOOR);
        let (mut on_ridge, mut shortest) = (0, f64::INFINITY);
        for s in sample(&mesh, 0.1, None).unwrap().iter().filter(|s| s.p[0] < 5.0) {
            let truth = if s.n[1].abs() > 0.5 {
                2.0
            } else {
                outline_section(&outline, [s.p[0], s.p[2]], [-s.n[0], -s.n[2]])
            };
            let Reading::Section { t, .. } = probe.read(s) else { panic!("{mode}: {s:?} not read, its true section {truth}") };
            assert!(t > truth - 1e-9 && t < truth + 1e-9, "{mode}: {s:?} read {t} against {truth}");
            if s.n[0] < -0.7 {
                on_ridge += 1;
                shortest = shortest.min(truth);
            }
        }
        // The short face is read, across the ridge, where its true section is under a ten-thousandth.
        assert!(on_ridge > 0 && shortest < 1e-4, "{mode}: {on_ridge} samples on the short face, shortest {shortest}");
    }
}

/// `f` on a pool of its own, so no other test's work queues ahead of its parallel reads.
fn on_own_pool<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    #[cfg(feature = "parallel")]
    return rayon::ThreadPoolBuilder::new().num_threads(4).build().expect("a pool").install(f);
    #[cfg(not(feature = "parallel"))]
    f()
}

#[test]
fn a_raised_flag_stops_the_census_at_its_next_read() {
    // The all-wall tube: every sample marches, the census's costliest reads.
    let tube = lathe(&[[9.0, 0.0], [9.5, 0.0], [9.5, 6.0], [9.0, 6.0]], 512);
    let options = CensusOptions::floor(FLOOR);
    on_own_pool(|| {
        // Left alone, it never runs an eighth of the census between two reads of its flag: about one in fifty measured.
        record_checks(None);
        let started = Instant::now();
        let whole = census_until(&tube, &options, &AtomicBool::new(false)).expect("never raised");
        let ended = Instant::now();
        let reads = checks();
        let marks: Vec<Instant> = std::iter::once(started).chain(reads.iter().copied()).chain(std::iter::once(ended)).collect();
        let longest = marks.windows(2).map(|w| w[1] - w[0]).max().unwrap();
        eprintln!("census {:?}, {} reads of its flag, longest between two {longest:?}", ended - started, reads.len());
        assert!(whole.assessed && whole.rays > 50_000 && reads.len() > 40, "{} reads: {whole:?}", reads.len());
        assert!(longest * 8 < ended - started, "ran {longest:?} without reading its flag, in a census of {:?}", ended - started);
        // Raised as it bins its surface, and as it reads its samples, it stops at that read.
        for at in [5, reads.len() - 10] {
            record_checks(Some(at));
            let stop = AtomicBool::new(false);
            assert!(census_until(&tube, &options, &stop).is_none(), "raised at read {at}");
            assert!(stop.load(Ordering::Relaxed));
            assert_eq!(checks().len(), at + 1, "no read after the one that raised it");
        }
    });
}

#[test]
fn a_plate_turned_to_the_cube_diagonal_keeps_near_the_sample_budget() {
    // A 40 x 40 x 1 plate whose broad faces face (1, 1, 1).
    let mut plate = cuboid([-20.0, -20.0, -0.5], [20.0, 20.0, 0.5]);
    let (axis, angle) = ([-1.0 / 2f64.sqrt(), 1.0 / 2f64.sqrt(), 0.0], (1.0 / 3f64.sqrt()).acos());
    let (sin, cos) = angle.sin_cos();
    for v in &mut plate.vertices {
        let p = [v.0 as f64, v.1 as f64, v.2 as f64];
        let k_x_p = [axis[1] * p[2] - axis[2] * p[1], axis[2] * p[0] - axis[0] * p[2], axis[0] * p[1] - axis[1] * p[0]];
        let k_dot_p = axis[0] * p[0] + axis[1] * p[1] + axis[2] * p[2];
        let r: [f64; 3] = std::array::from_fn(|i| p[i] * cos + k_x_p[i] * sin + axis[i] * k_dot_p * (1.0 - cos));
        *v = Vec3(r[0] as f32, r[1] as f32, r[2] as f32);
    }
    let area: f64 = 2.0 * 40.0 * 40.0 + 4.0 * 40.0;
    let most = 20_000.0;
    // Turned so, the plate takes well over one sample per pitch² of its area.
    let plain = (area / most).sqrt();
    let crowded = sample(&plate, plain, None).unwrap().len() as f64;
    assert!(crowded > 1.3 * most, "{crowded} samples at the unwidened pitch");
    let pitch = pitch_for(None, FLOOR, area, most);
    let n = sample(&plate, pitch, None).unwrap().len() as f64;
    assert!(n <= 1.05 * most && n > 0.7 * most, "{n} samples for a budget of {most} at pitch {pitch}");
}

#[test]
fn an_open_or_empty_mesh_is_not_assessed() {
    let mut open = cuboid([0.0; 3], [1.0; 3]);
    open.faces.pop();
    for m in [Mesh::default(), open] {
        let t = thickness(&m, FLOOR);
        assert!(!t.assessed && !t.clean() && t.rays == 0, "{t:?}");
    }
}

#[test]
fn edges_are_named_kinds_and_serialise() {
    let t = at_floor(&prism(&[[0.0, -1.0], [6.5, -1.0], [8.0, 0.0], [6.5, 1.0], [0.0, 1.0]], 6.0));
    let json = serde_json::to_value(&t).unwrap();
    assert_eq!(json["edges"][0]["kind"], "edge");
    assert!(json["walls"].as_array().unwrap().is_empty());
    assert_eq!(t.edges[0].kind, ThinKind::Edge);
}

#[test]
fn a_million_and_a_half_faces_are_read_in_seconds() {
    let lib = crate::AlphaLibrary::builtin();
    let band = crate::mesh::build(
        &crate::RingDesign::default(),
        &lib,
        crate::BuildParams { theta_steps: 2048, profile_steps: 384, ..crate::BuildParams::default() },
    )
    .mesh;
    assert!(band.faces.len() > 1_500_000, "{}", band.faces.len());
    let started = std::time::Instant::now();
    let t = thickness(&band, FLOOR);
    let secs = started.elapsed().as_secs_f64();
    eprintln!("{} faces: {secs:.2} s, {} samples, {} edge samples, {:?}", band.faces.len(), t.rays, t.edge_below_limit, t.sampled_min_mm);
    assert!(t.assessed && t.unresolved == 0 && t.internal == 0 && t.rays > 100_000, "{t:?}");
    // The band's two bore edges are its only thin zones, and they close within the reach.
    assert!(t.clean() && t.edges.len() == 2, "{t:?}");
    assert!(secs < 30.0, "{secs} s");
}

#[test]
fn a_thin_sheet_ring_is_wall_everywhere() {
    // A 0.5 mm tube 6 mm tall: every sample on its two faces is under the floor and none of it is an edge.
    let tube = lathe(&[[9.0, 0.0], [9.5, 0.0], [9.5, 6.0], [9.0, 6.0]], 512);
    let started = std::time::Instant::now();
    let t = at_floor(&tube);
    eprintln!("tube: {:.2} s, {} samples, {} wall, {} edge", started.elapsed().as_secs_f64(), t.rays, t.below_limit, t.edge_below_limit);
    let faces = std::f64::consts::TAU * (9.0 + 9.5) * 6.0;
    assert!(t.wall_area_mm2 > 0.95 * faces, "{} of {faces} mm²", t.wall_area_mm2);
    assert!(t.walls[0].span_mm > 18.0 && (t.walls[0].thinnest_mm - 0.5).abs() < 0.01, "{:?}", t.walls[0]);
}

/// A 6 x 6 x 2 block carrying a fin `thick` wide and `tall` high along its length, turned `deg` about the fin's normal.
fn fin_block(thick: f64, tall: f64, deg: f64) -> Mesh {
    let (a, b) = (3.0 - 0.5 * thick, 3.0 + 0.5 * thick);
    let mut m = prism(&[[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [b, 2.0], [b, 2.0 + tall], [a, 2.0 + tall], [a, 2.0], [0.0, 2.0]], 6.0);
    let (s, c) = deg.to_radians().sin_cos();
    for v in &mut m.vertices {
        let (y, z) = (v.1 as f64, v.2 as f64);
        (v.1, v.2) = ((y * c - z * s) as f32, (y * s + z * c) as f32);
    }
    m
}

/// A 3 mm rod carrying a pin `dia` across and `long` high on its end.
fn pinned_rod(dia: f64, long: f64) -> Mesh {
    let r = 0.5 * dia;
    lathe(&[[0.0, 0.0], [1.5, 0.0], [1.5, 3.0], [r, 3.0], [r, 3.0 + long], [0.0, 3.0 + long]], 96)
}

#[test]
fn a_fin_pin_or_lip_taller_than_it_is_thick_is_a_wall() {
    for (what, m) in [
        ("0.05 mm fin 0.7 mm tall", fin_block(0.05, 0.7, 0.0)),
        ("0.3 mm lip 0.82 mm tall", fin_block(0.3, 0.82, 0.0)),
        ("0.3 mm lip 0.5 mm tall", fin_block(0.3, 0.5, 0.0)),
        ("0.1 mm pin 0.6 mm long", pinned_rod(0.1, 0.6)),
        ("0.15 mm pin 0.7 mm long", pinned_rod(0.15, 0.7)),
    ] {
        let t = at_floor(&m);
        assert!(!t.clean() && !t.walls.is_empty(), "{what}: {t:?}");
        assert!(t.walls.iter().all(|z| z.point[2] > 2.0 - 1e-6), "{what}: {:?}", t.walls);
    }
    // A lip no taller than it is thick is fed from the body behind it.
    let t = at_floor(&fin_block(0.4, 0.3, 0.0));
    assert!(t.clean() && !t.edges.is_empty(), "{t:?}");
}

#[test]
fn a_lip_reads_the_same_at_every_turn_about_its_normal() {
    for deg in [0.0, 7.0, 11.25, 16.0, 22.5, 30.0, 33.75, 45.0, 61.0, 90.0] {
        let short = at_floor(&fin_block(0.6, 0.62, deg));
        assert!(short.clean() && !short.edges.is_empty(), "{deg}°: {short:?}");
        let tall = at_floor(&fin_block(0.6, 0.75, deg));
        assert!(!tall.clean() && tall.wall_area_mm2 > 0.8 * 2.0 * 6.0 * 0.75, "{deg}°: {tall:?}");
    }
}

#[test]
fn a_shell_flush_inside_another_reads_as_their_union() {
    // A block inside another with their tops flush: the union is the outer block, a 2 mm slab.
    let outer = cuboid([0.0, 0.0, 0.0], [4.0, 4.0, 2.0]);
    let inner = cuboid([1.0, 1.0, 1.0], [3.0, 3.0, 2.0]);
    let t = at_floor(&shells(&[outer, inner]));
    assert!(t.internal > 0 && t.clean() && t.walls.is_empty() && t.edges.is_empty(), "{t:?}");
    assert!(t.sampled_min_mm.unwrap() > 2.0 - 1e-3, "{t:?}");
}

#[test]
fn a_plate_between_flush_pairs_reads_its_own_section() {
    // A 0.3 mm plate with a pair of overlapping blocks above and below it, each pair flush on the face toward it.
    let plate = cuboid([0.0, 0.0, 0.0], [4.0, 4.0, 0.3]);
    let pairs = [
        cuboid([0.0, 0.0, 1.0], [4.0, 4.0, 2.0]),
        cuboid([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]),
        cuboid([0.0, 0.0, -2.0], [4.0, 4.0, -1.0]),
        cuboid([1.0, 1.0, -3.0], [3.0, 3.0, -1.0]),
    ];
    let alone = at_floor(&shells(&[plate.clone(), cuboid([10.0, 0.0, 0.0], [12.0, 2.0, 2.0])]));
    let mut parts = vec![plate];
    parts.extend(pairs);
    let sandwich = at_floor(&shells(&parts));
    for t in [&alone, &sandwich] {
        assert!(!t.clean() && (t.walls[0].thinnest_mm - 0.3).abs() < 1e-3, "{t:?}");
    }
    assert!((sandwich.wall_area_mm2 - alone.wall_area_mm2).abs() < 1e-6, "{} against {}", sandwich.wall_area_mm2, alone.wall_area_mm2);
}
