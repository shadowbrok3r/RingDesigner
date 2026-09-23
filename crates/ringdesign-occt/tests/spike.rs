//! The OpenCascade spike through the worker process: fillet, torus-and-cylinder junction, shell and STEP import, measured; `--nocapture` prints the numbers.
#![cfg(feature = "kernel-occt")]
use ringdesign_core::cad::{self, Attach, Component, Document, EvaluatedComponent, Feature, FeatureStatus, Operation, Placement, stored::Packed};
use ringdesign_core::{AlphaLibrary, BuildParams, Mesh, RingDesign, Vec3, sketch::Sketch};
use ringdesign_occt::client::Worker;
use ringdesign_occt::parts;
use ringdesign_occt::protocol::{Built, Request, Response, Tolerance};
use std::f64::consts::PI;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(120);
/// Fine enough that the tessellation's volume lands within a few parts in ten thousand.
const FINE: Tolerance = Tolerance { chord_mm: 0.002, angle_deg: 3.0 };
const SIGNETS: &str = "/home/shadowbroker/jewelry-scan/RING/Signets";

fn worker() -> Worker {
    Worker::at(env!("CARGO_BIN_EXE_occt-worker"))
}
fn params() -> BuildParams {
    BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..BuildParams::default() }
}

/// What one request came back with, and what it cost from the caller's side.
struct Run {
    solids: Vec<Built>,
    notes: Vec<String>,
    kernel_ms: f64,
    wall_ms: f64,
}
fn run(request: &Request) -> Run {
    let started = Instant::now();
    let response = worker().run(request, TIMEOUT).unwrap_or_else(|e| panic!("{} request: {e}", request.op()));
    let wall_ms = started.elapsed().as_secs_f64() * 1e3;
    match response {
        Response::Done { solids, notes, kernel_ms } => Run { solids, notes, kernel_ms, wall_ms },
        Response::Refused { message } => panic!("{} refused: {message}", request.op()),
    }
}
/// A packed mesh as the core's mesh, with its closure checked the way a build would.
fn mesh_of(p: &Packed) -> Mesh {
    let made = p.made().expect("a stored mesh closes");
    let s = made.solid();
    Mesh { vertices: s.v.iter().map(|q| Vec3(q[0] as f32, q[1] as f32, q[2] as f32)).collect(), faces: s.f.clone(), ..Mesh::default() }
}
fn volume_f64(p: &Packed) -> f64 {
    let u = p.decode().unwrap();
    u.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| u.positions[i as usize]);
            (a[0] * (b[1] * c[2] - b[2] * c[1]) + a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        })
        .sum()
}
/// Bytes the packed stream would take deflated, against its base64 length.
fn deflated(p: &Packed) -> usize {
    use base64::Engine as _;
    use std::io::Write as _;
    let raw = base64::engine::general_purpose::STANDARD.decode(p.data.as_bytes()).unwrap();
    let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&raw).unwrap();
    z.finish().unwrap().len()
}
fn report(what: &str, r: &Run, analytic: Option<f64>) {
    for s in &r.solids {
        let mesh = mesh_of(&s.mesh);
        let v = mesh.validate();
        let against = analytic.map_or(String::new(), |a| format!(", analytic {a:.4} (B-rep {:+.2e}, mesh {:+.2e})", (s.brep_volume_mm3 - a) / a, (s.mesh_volume_mm3 - a) / a));
        eprintln!(
            "{what}: {} — {} tris, {} verts, watertight {}, B-rep {:.4} mm³, mesh {:.4} mm³{against}; kernel {:.1} ms, round trip {:.1} ms; packed {} B base64, {} B deflated",
            s.name,
            s.mesh.triangles,
            s.mesh.vertices,
            v.watertight,
            s.brep_volume_mm3,
            s.mesh_volume_mm3,
            r.kernel_ms,
            r.wall_ms,
            s.mesh.data.len(),
            deflated(&s.mesh)
        );
    }
    for n in &r.notes {
        eprintln!("    {n}");
    }
}

/// The Court band with a procedural shank and `more` parts.
fn court(more: Vec<Feature>) -> RingDesign {
    let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    for f in more {
        doc.append(f).unwrap();
    }
    d.cad = Some(doc);
    d
}
fn joined(id: u64, name: &str, operation: Operation, placement: Placement) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { attach: Attach::Join, placement, ..Component::default() } }
}
fn evaluated(d: &RingDesign) -> (ringdesign_core::BuildResult, cad::Evaluated) {
    let b = ringdesign_core::mesh::try_build(d, &AlphaLibrary::builtin(), params()).unwrap();
    let e = b.parts.evaluated.clone().unwrap();
    (b, e)
}
fn part(e: &cad::Evaluated, id: u64) -> &EvaluatedComponent {
    e.components.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no part #{id}: {:?}", e.features))
}
/// Edge ordinals of `c` whose ends, in its own frame, `keep` accepts.
fn edges_where(c: &EvaluatedComponent, keep: impl Fn([f64; 3], [f64; 3]) -> bool) -> Vec<usize> {
    let body = parts::local_body(c).unwrap();
    body.edges.iter().enumerate().filter_map(|(i, (key, _))| body.edge_endpoints(key).filter(|(a, b)| keep(*a, *b)).map(|_| i)).collect()
}
fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}
/// The volume a fillet of radius `r` takes off a right-angled edge per millimetre of its length.
fn corner_area(r: f64) -> f64 {
    (1.0 - PI / 4.0) * r * r
}
/// How far from the corner the centroid of that removed section sits.
fn corner_centroid(r: f64) -> f64 {
    r * (10.0 - 3.0 * PI) / (12.0 - 3.0 * PI)
}

#[test]
fn a_ping_crosses_the_process_boundary() {
    let started = Instant::now();
    let response = worker().run(&Request::Ping, TIMEOUT).unwrap();
    eprintln!("ping: {:.1} ms", started.elapsed().as_secs_f64() * 1e3);
    assert!(matches!(response, Response::Done { ref solids, .. } if solids.is_empty()), "{response:?}");
    let refused = worker().run(&Request::Import { step: "ISO-10303-21;\nnot a step file".into(), tolerance: Tolerance::EXPORT }, TIMEOUT).unwrap();
    assert!(matches!(&refused, Response::Refused { message } if message.contains("could not read")), "{refused:?}");
}

#[test]
fn a_kernel_parts_chosen_edges_fillet_through_opencascade_and_the_part_joins_the_band() {
    let block = joined(2, "Block", Operation::Box { size: [4.0, 6.0, 2.0] }, Placement::ring(90.0, 0.0));
    let post = joined(3, "Post", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Placement::ring(200.0, 0.4));
    let d = court(vec![block, post]);
    let (_, e) = evaluated(&d);
    let (block, post) = (part(&e, 2), part(&e, 3));
    // One 4 mm edge along the block's top, and the post's whole top rim.
    let along = edges_where(block, |a, b| a[2] > 0.99 && b[2] > 0.99 && (gap(a, b) - 4.0).abs() < 1e-9);
    let rim = edges_where(post, |a, b| a[2] > 1.249 && b[2] > 1.249);
    assert_eq!((along.len(), rim.is_empty()), (2, false), "{along:?} {rim:?}");
    for tolerance in [Tolerance::EXPORT, FINE] {
        let (request, _) = parts::fillet(block, &along[..1], 0.5, tolerance).unwrap();
        let r = run(&request);
        let analytic = 48.0 - corner_area(0.5) * 4.0;
        report(&format!("fillet block edge r 0.5, chord {}", tolerance.chord_mm), &r, Some(analytic));
        let s = &r.solids[0];
        assert!((s.brep_volume_mm3 - analytic).abs() < 1e-6 * analytic, "{} against {analytic}", s.brep_volume_mm3);
        assert!((s.mesh_volume_mm3 - analytic).abs() < 2e-3 * analytic);
        assert!(mesh_of(&s.mesh).validate().watertight);
        let (request, _) = parts::fillet(post, &rim, 0.3, tolerance).unwrap();
        let r = run(&request);
        let analytic = PI * 1.5 * 1.5 * 2.5 - corner_area(0.3) * 2.0 * PI * (1.5 - corner_centroid(0.3));
        report(&format!("fillet post rim r 0.3, chord {}", tolerance.chord_mm), &r, Some(analytic));
        let s = &r.solids[0];
        assert!((s.brep_volume_mm3 - analytic).abs() < 1e-6 * analytic, "{} against {analytic}", s.brep_volume_mm3);
        assert!((s.mesh_volume_mm3 - analytic).abs() < 5e-3 * analytic);
    }
    // The pure kernel on the same rim, for comparison.
    let mut kernel = d.clone();
    let rims: Vec<cad::EdgeRef> = rim.iter().map(|i| cad::EdgeRef::bare(*i)).collect();
    kernel.cad.as_mut().unwrap().append(Feature { id: 4, name: "Fillet".into(), enabled: true, operation: Operation::Fillet { source: 3, edges: rims, radius_mm: 0.3 }, component: Component::default() }).unwrap();
    let pure = cad::evaluate(&kernel, &AlphaLibrary::builtin(), params()).unwrap();
    eprintln!("pure kernel, post rim r 0.3: {:?}", pure.status_of(4));
    // Kept as a stored mesh, the rounded block joins the band on a build with no OpenCascade in it.
    let (request, recipe) = parts::fillet(block, &along[..1], 0.5, Tolerance::EXPORT).unwrap();
    let r = run(&request);
    let doc = d.cad.as_ref().unwrap();
    let mut feature = parts::stored_feature(doc, &[2], &request, recipe, &r.solids[0], doc.feature(2).unwrap().component.clone());
    feature.id = 5;
    let mut kept = d.clone();
    kept.cad.as_mut().unwrap().append(feature).unwrap();
    let started = Instant::now();
    let (b, e) = evaluated(&kept);
    let build_ms = started.elapsed().as_secs_f64() * 1e3;
    assert!(b.report.validation.watertight && b.parts.notes.is_empty(), "{:?} {:?}", b.report.validation, b.parts.notes);
    assert_eq!((b.parts.joined, e.components.iter().map(|c| c.id).collect::<Vec<_>>()), (2, vec![3, 5]), "the rounded block replaces the block");
    let rounded = part(&e, 5);
    assert_eq!(rounded.frame, block.frame, "it stands in the block's seat");
    let f = ringdesign_core::castability::judged_field_report(&kept, &AlphaLibrary::builtin(), &kept.draft, 192, 128, Some(&b));
    let judged = f.parts.iter().find(|p| p.feature == 5).expect("judged");
    eprintln!("stored fillet on the Court band: build {build_ms:.1} ms, ring {} tris, watertight; verdict {:?}, part undercut {:.3} mm² of {:.1}", b.mesh.faces.len(), f.verdict, judged.undercut_area_mm2, judged.total_area_mm2);
    let text = serde_json::to_string(&kept).unwrap();
    assert_eq!(serde_json::to_string(&serde_json::from_str::<RingDesign>(&text).unwrap()).unwrap(), text, "reopens bit for bit");
}

#[test]
fn the_torus_and_cylinder_junction_the_kernel_refuses_fuses_and_rounds_in_opencascade() {
    let (major, minor) = (9.85, 1.2);
    let mut d = RingDesign::default();
    let mut doc = Document::default();
    let shank = Component { role: cad::ComponentRole::Shank, ..Component::default() };
    doc.append(Feature { id: 1, name: "Shank".into(), enabled: true, operation: Operation::Torus { major_mm: major, minor_mm: minor }, component: shank }).unwrap();
    doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.5 }, component: Component { placement: Placement::ring(90.0, 0.0), ..Component::default() } }).unwrap();
    d.cad = Some(doc);
    // The post's foot `sink` into the tube's crest, however the seat is read.
    let e = cad::evaluate(&d, &AlphaLibrary::builtin(), params()).unwrap();
    let r0 = part(&e, 2).frame.origin[1];
    let sunk = |d: &mut RingDesign, sink: f64| d.cad.as_mut().unwrap().features[1].component.placement = Placement::ring(90.0, major + minor + 1.25 - sink - r0);
    // A foot 0.25 into the crest pokes out of the tube's sides, the seam runs into its rim, and the fillet does not complete.
    sunk(&mut d, 0.25);
    let e = cad::evaluate(&d, &AlphaLibrary::builtin(), params()).unwrap();
    let (request, _) = parts::junction(d.cad.as_ref().unwrap(), 1, part(&e, 2), 0.4, Tolerance::EXPORT).unwrap();
    let rim = worker().run(&request, TIMEOUT).unwrap();
    assert!(matches!(&rim, Response::Refused { message } if message.contains("radius=0.4 does not fit the local geometry on 4 edge(s)")), "{rim:?}");
    // Half a millimetre in, the foot is inside the tube and the seam closes round the post.
    sunk(&mut d, 0.5);
    let mut refused = d.clone();
    refused.cad.as_mut().unwrap().append(Feature { id: 3, name: "Union".into(), enabled: true, operation: Operation::Boolean { a: 1, b: 2, kind: cad::Boolean::Union }, component: Component::default() }).unwrap();
    let started = Instant::now();
    let pure = cad::evaluate(&refused, &AlphaLibrary::builtin(), params()).unwrap();
    let status = pure.status_of(3).cloned();
    eprintln!("pure kernel torus ∪ cylinder: {:.1} ms, {status:?}", started.elapsed().as_secs_f64() * 1e3);
    assert!(matches!(status, Some(FeatureStatus::Failed(_))), "{status:?}");
    let e = cad::evaluate(&d, &AlphaLibrary::builtin(), params()).unwrap();
    let post = part(&e, 2);
    assert!((post.frame.origin[1] - (major + minor + 0.75)).abs() < 1e-6, "{:?}", post.frame.origin);
    for tolerance in [Tolerance::EXPORT, FINE] {
        let (request, _) = parts::junction(d.cad.as_ref().unwrap(), 1, post, 0.4, tolerance).unwrap();
        let r = run(&request);
        report(&format!("junction r 0.4, chord {}", tolerance.chord_mm), &r, None);
        let s = &r.solids[0];
        let (torus, cylinder) = (2.0 * PI * PI * major * minor * minor, PI * 0.8 * 0.8 * 2.5);
        // The fused solid is under the two apart by their overlap; the fillet adds a little back into the corner.
        let fused: f64 = r.notes[0].split(": ").nth(1).and_then(|t| t.split(' ').next()).and_then(|v| v.parse().ok()).unwrap();
        assert!(fused < torus + cylinder && fused > torus, "{fused} against {torus} + {cylinder}");
        assert!(s.brep_volume_mm3 > fused && s.brep_volume_mm3 - fused < 0.5, "{} against {fused}", s.brep_volume_mm3);
        assert!((s.mesh_volume_mm3 - s.brep_volume_mm3).abs() < 1e-2 * s.brep_volume_mm3, "{} against {}", s.mesh_volume_mm3, s.brep_volume_mm3);
        assert!(mesh_of(&s.mesh).validate().watertight);
    }
    // Kept, it replaces both parts and is the ring.
    let (request, recipe) = parts::junction(d.cad.as_ref().unwrap(), 1, post, 0.4, Tolerance::EXPORT).unwrap();
    let r = run(&request);
    let doc = d.cad.as_ref().unwrap();
    let mut feature = parts::stored_feature(doc, &[1, 2], &request, recipe, &r.solids[0], Component::default());
    feature.id = 3;
    let mut kept = d.clone();
    kept.cad.as_mut().unwrap().append(feature).unwrap();
    assert_eq!(kept.cad.as_ref().unwrap().outputs, vec![3]);
    let (b, _) = evaluated(&kept);
    assert!(b.report.validation.watertight, "{:?}", b.report.validation);
    assert!((b.mesh.volume_mm3() - volume_f64(&r.solids[0].mesh)).abs() < 1e-3, "{} against {}", b.mesh.volume_mm3(), volume_f64(&r.solids[0].mesh));
    let verdict = ringdesign_core::castability::analyze(&b.mesh, &kept.draft, kept.inner_radius_mm());
    eprintln!("stored junction as the ring: {} tris, watertight, mesh verdict {:?} at {:.3}% undercut", b.mesh.faces.len(), verdict.verdict, verdict.undercut_fraction() * 100.0);
}

#[test]
fn a_cushion_head_shells_in_opencascade_where_the_kernel_takes_only_boxes_cylinders_and_spheres() {
    let (w, h, corner, tall, wall) = (12.0, 10.0, 2.0, 3.0, 0.6);
    let mut cushion = Sketch::rectangle(w, h);
    for id in cushion.points.iter().map(|p| p.id).collect::<Vec<_>>() {
        cushion.fillet_corner(id, corner).unwrap();
    }
    let head = joined(2, "Head", Operation::Extrude { sketch: cushion.into(), height_mm: tall, draft_deg: 0.0 }, Placement::ring(90.0, -0.8));
    let d = court(vec![head]);
    let (_, e) = evaluated(&d);
    let head = part(&e, 2);
    let area = |w: f64, h: f64, r: f64| w * h - (4.0 - PI) * r * r;
    let solid = area(w, h, corner) * tall;
    assert!((head.mesh.volume_mm3() - solid).abs() < 0.02 * solid, "{} against {solid}", head.mesh.volume_mm3());
    let mut kernel = d.clone();
    kernel.cad.as_mut().unwrap().append(Feature { id: 3, name: "Shell".into(), enabled: true, operation: Operation::Shell { source: 2, open_faces: vec![], thickness_mm: wall }, component: Component::default() }).unwrap();
    let pure = cad::evaluate(&kernel, &AlphaLibrary::builtin(), params()).unwrap();
    eprintln!("pure kernel shell of the cushion head: {:?}", pure.status_of(3));
    // Open where the head meets the band: the face on its own plane z = 0.
    let analytic = solid - area(w - 2.0 * wall, h - 2.0 * wall, corner - wall) * (tall - wall);
    for tolerance in [Tolerance::EXPORT, FINE] {
        let (request, _) = parts::shell(head, &[[0.0, 0.0, 0.0]], wall, tolerance).unwrap();
        let r = run(&request);
        report(&format!("shell cushion head 0.6 wall, chord {}", tolerance.chord_mm), &r, Some(analytic));
        let s = &r.solids[0];
        assert!((s.brep_volume_mm3 - analytic).abs() < 1e-5 * analytic, "{} against {analytic}", s.brep_volume_mm3);
        assert!((s.mesh_volume_mm3 - analytic).abs() < 2e-3 * analytic);
        assert!(mesh_of(&s.mesh).validate().watertight);
    }
}

/// A binary STL's triangles.
fn read_stl(path: &str) -> Vec<[[f64; 3]; 3]> {
    let bytes = std::fs::read(path).unwrap();
    let n = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 84 + n * 50, "a binary STL");
    (0..n)
        .map(|t| {
            let at = 84 + t * 50 + 12;
            std::array::from_fn(|c| std::array::from_fn(|k| f32::from_le_bytes(bytes[at + c * 12 + k * 4..at + c * 12 + k * 4 + 4].try_into().unwrap()) as f64))
        })
        .collect()
}
/// Triangles grouped by the connected piece they belong to, welding corners at a micron.
fn pieces(tris: &[[[f64; 3]; 3]]) -> Vec<Vec<usize>> {
    use std::collections::HashMap;
    let key = |p: [f64; 3]| p.map(|c| (c * 1e3).round() as i64);
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn root(p: &mut Vec<usize>, mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut first: HashMap<[i64; 3], usize> = HashMap::new();
    for (t, tri) in tris.iter().enumerate() {
        for p in tri {
            match first.get(&key(*p)) {
                Some(&o) => {
                    let (a, b) = (root(&mut parent, o), root(&mut parent, t));
                    parent[a.max(b)] = a.min(b);
                }
                None => {
                    first.insert(key(*p), t);
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for t in 0..tris.len() {
        let r = root(&mut parent, t);
        groups.entry(r).or_default().push(t);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by_key(|g| g[0]);
    out
}
fn signed_volume(tris: impl Iterator<Item = [[f64; 3]; 3]>) -> f64 {
    tris.map(|[a, b, c]| (a[0] * (b[1] * c[2] - b[2] * c[1]) + a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0).sum()
}
fn bounds(points: impl Iterator<Item = [f64; 3]>) -> ([f64; 3], [f64; 3]) {
    points.fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), p| (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k]))))
}
/// Distance from `p` to triangle `t`.
fn to_triangle(p: [f64; 3], t: &[[f64; 3]; 3]) -> f64 {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (ab, ac, ap) = (sub(t[1], t[0]), sub(t[2], t[0]), sub(p, t[0]));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    let closest = |q: [f64; 3]| gap(p, q);
    if d1 <= 0.0 && d2 <= 0.0 {
        return closest(t[0]);
    }
    let bp = sub(p, t[1]);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return closest(t[1]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return closest(std::array::from_fn(|k| t[0][k] + ab[k] * v));
    }
    let cp = sub(p, t[2]);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return closest(t[2]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return closest(std::array::from_fn(|k| t[0][k] + ac[k] * w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return closest(std::array::from_fn(|k| t[1][k] + (t[2][k] - t[1][k]) * w));
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    closest(std::array::from_fn(|k| t[0][k] + ab[k] * v + ac[k] * w))
}
/// Mean and largest distance from every `n`th point of `from` to the surface `to`, through a 0.5 mm grid of its triangles.
fn deviation(from: &[[f64; 3]], to: &[[[f64; 3]; 3]], n: usize) -> (f64, f64) {
    use std::collections::HashMap;
    let cell = 0.5;
    let at = |p: [f64; 3]| p.map(|c| (c / cell).floor() as i64);
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, t) in to.iter().enumerate() {
        let (lo, hi) = bounds(t.iter().copied());
        let (a, b) = (at(lo), at(hi));
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    grid.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let (mut sum, mut worst, mut count) = (0.0, 0.0f64, 0usize);
    for p in from.iter().step_by(n.max(1)) {
        let c = at(*p);
        let mut best = f64::MAX;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    for &i in grid.get(&[c[0] + x, c[1] + y, c[2] + z]).into_iter().flatten() {
                        best = best.min(to_triangle(*p, &to[i]));
                    }
                }
            }
        }
        if best < f64::MAX {
            sum += best;
            worst = worst.max(best);
            count += 1;
        }
    }
    (sum / count.max(1) as f64, worst)
}

#[test]
fn a_bought_signet_step_imports_as_mesh_parts_that_match_its_stl() {
    let step_path = format!("{SIGNETS}/Heart-signet-ring-size7to11.STEP");
    let Ok(step) = std::fs::read(&step_path) else {
        eprintln!("{step_path} is not on this machine; the import is not measured");
        return;
    };
    let step = String::from_utf8_lossy(&step).into_owned();
    let stl = read_stl(&format!("{SIGNETS}/Heart-signet-ring-size7to11.STL"));
    let rings = pieces(&stl);
    let r = run(&Request::Import { step, tolerance: Tolerance::EXPORT });
    report("import heart signet 7-11", &r, None);
    assert_eq!((r.solids.len(), rings.len()), (5, 5), "five sizes in each file");
    let centre = |lo: [f64; 3], hi: [f64; 3]| -> [f64; 3] { std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0) };
    let span = |lo: [f64; 3], hi: [f64; 3]| -> [f64; 3] { std::array::from_fn(|k| hi[k] - lo[k]) };
    for s in &r.solids {
        let u = s.mesh.decode().unwrap();
        let (lo, hi) = bounds(u.positions.iter().copied());
        // The STL piece whose box is the nearest size: the STL lays its rings out apart, the STEP about one origin.
        let size_gap = |g: &Vec<usize>| {
            let (l, h) = bounds(g.iter().flat_map(|t| stl[*t]));
            (0..3).map(|k| (span(l, h)[k] - span(lo, hi)[k]).abs()).fold(0.0, f64::max)
        };
        let piece = rings.iter().min_by(|a, b| size_gap(a).total_cmp(&size_gap(b))).unwrap();
        let (plo, phi) = bounds(piece.iter().flat_map(|t| stl[*t]));
        let shift: [f64; 3] = std::array::from_fn(|k| centre(lo, hi)[k] - centre(plo, phi)[k]);
        let tris: Vec<[[f64; 3]; 3]> = piece.iter().map(|t| stl[*t].map(|p| std::array::from_fn(|k| p[k] + shift[k]))).collect();
        let stl_volume = signed_volume(tris.iter().copied());
        let ours: Vec<[[f64; 3]; 3]> = u.triangles.iter().map(|t| t.map(|i| u.positions[i as usize])).collect();
        let stl_points: Vec<[f64; 3]> = tris.iter().flatten().copied().collect();
        let (mean, worst) = deviation(&stl_points, &ours, 7);
        let (back_mean, back_worst) = deviation(&u.positions, &tris, 7);
        eprintln!(
            "    {} ({:.2} x {:.2} x {:.2} mm): {} tris against the STL's {}, volume {:.3} (B-rep {:.3}) against {stl_volume:.3} mm³ ({:+.3}%), box off by {:.4} mm once moved {:.3?}; STL to ours mean {mean:.4} max {worst:.4} mm, ours to STL mean {back_mean:.4} max {back_worst:.4} mm",
            s.name,
            span(lo, hi)[0],
            span(lo, hi)[1],
            span(lo, hi)[2],
            s.mesh.triangles,
            tris.len(),
            s.mesh_volume_mm3,
            s.brep_volume_mm3,
            (s.mesh_volume_mm3 - stl_volume) / stl_volume * 100.0,
            size_gap(piece),
            shift
        );
        assert!((s.mesh_volume_mm3 - stl_volume).abs() < 0.01 * stl_volume, "{} against {stl_volume}", s.mesh_volume_mm3);
        assert!(size_gap(piece) < 0.05 && worst < 0.1 && back_worst < 0.1, "{} {worst} {back_worst}", size_gap(piece));
        assert!(mesh_of(&s.mesh).validate().watertight);
    }
    // The size-7 ring as a part of its own: a ring of parts only, built and read by the mesh verdict with no OpenCascade.
    let smallest = r.solids.iter().min_by(|a, b| {
        let x = |s: &Built| {
            let (lo, hi) = bounds(s.mesh.decode().unwrap().positions.into_iter());
            hi[0] - lo[0]
        };
        x(a).total_cmp(&x(b))
    }).unwrap();
    let mut d = RingDesign::default();
    let mut doc = Document::default();
    let mut feature = parts::stored_feature(&doc, &[], &Request::Import { step: String::new(), tolerance: Tolerance::EXPORT }, serde_json::json!({ "file": "Heart-signet-ring-size7to11.STEP", "solid": smallest.name }), smallest, Component::default());
    feature.id = 1;
    doc.append(feature).unwrap();
    d.cad = Some(doc);
    let started = Instant::now();
    let (b, _) = evaluated(&d);
    let verdict = ringdesign_core::castability::analyze(&b.mesh, &d.draft, 8.6);
    eprintln!(
        "stored heart signet size 7 as a ring: build {:.1} ms, {} tris, watertight {}, {:.3} mm³, mesh verdict {:?} at {:.3}% undercut; design file {} KB",
        started.elapsed().as_secs_f64() * 1e3,
        b.mesh.faces.len(),
        b.report.validation.watertight,
        b.mesh.volume_mm3(),
        verdict.verdict,
        verdict.undercut_fraction() * 100.0,
        serde_json::to_string(&d).unwrap().len() / 1024
    );
    assert!(b.report.validation.watertight);
}
