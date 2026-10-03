use super::super::{CensusOptions, ThinKind, Thickness, census, thickness};
use super::{Probe, Reading, sample};
use crate::mesh::{Mesh, Vec3};

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
fn a_ray_that_starts_on_a_face_never_reads_that_face_or_its_neighbours() {
    // Every sample of a box, read one at a time: nothing nearer than its true section.
    let boxed = cuboid([0.0, 0.0, 0.0], [3.0, 2.0, 1.0]);
    let probe = Probe::new(&boxed, FLOOR, FLOOR);
    for s in sample(&boxed, 0.1) {
        let Reading::Section { t, .. } = probe.read(&s) else { panic!("{s:?}") };
        assert!(t > 1.0 - 1e-6, "{s:?} read {t}");
    }
    let total: f64 = sample(&boxed, 0.1).iter().map(|s| s.area).sum();
    assert!((total - 2.0 * (6.0 + 3.0 + 2.0)).abs() < 1e-9, "{total}");
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
