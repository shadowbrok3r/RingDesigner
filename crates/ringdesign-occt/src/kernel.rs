//! The worker's side: cadrum's static OpenCascade builds a request's solids and hands each back tessellated, welded, closed and packed.
use crate::protocol::{Built, Cylinder, EdgeProbe, MARKER, Request, Response, Tolerance, Torus};
use anyhow::{Context, Result, anyhow, ensure};
use cadrum::{DVec3, Edge, Face, Solid, Tessellation};
use ringdesign_core::cad::{SurfaceKind, stored::Packed};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::time::Instant;

/// Farthest a point of one of our edges may lie off the edge OpenCascade reads for it, mm.
pub const MATCH_MM: f64 = 1e-4;
/// Tessellation nodes closer than this are one vertex, mm.
const WELD_MM: f64 = 1e-6;
/// Most solids an import hands back.
const MAX_SOLIDS: usize = 64;

/// Reads one request from stdin and writes the marker and one response to stdout, a worker's whole life; the exit code.
pub fn serve() -> i32 {
    let mut input = String::new();
    let response = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => match serde_json::from_str::<Request>(&input) {
            Ok(request) => run(&request),
            Err(e) => Response::Refused { message: format!("The request did not read: {e}") },
        },
        Err(e) => Response::Refused { message: format!("The request did not arrive: {e}") },
    };
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "\n{MARKER}").and_then(|_| serde_json::to_writer(&mut out, &response).map_err(std::io::Error::from)).and_then(|_| out.flush());
    if written.is_ok() { 0 } else { 1 }
}

/// `request` built by OpenCascade in this process; a panic on the way is a refusal.
pub fn run(request: &Request) -> Response {
    let started = Instant::now();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(request))).unwrap_or_else(|panic| {
        let what = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        Err(anyhow!("OpenCascade panicked: {what}"))
    });
    match outcome {
        Ok((solids, notes)) => Response::Done { solids, notes, kernel_ms: started.elapsed().as_secs_f64() * 1e3 },
        Err(e) => Response::Refused { message: format!("{e:#}") },
    }
}

fn build(request: &Request) -> Result<(Vec<Built>, Vec<String>)> {
    match request {
        Request::Ping => Ok((Vec::new(), Vec::new())),
        Request::Fillet { step, edges, radius_mm, tolerance } => fillet(step, edges, *radius_mm, *tolerance),
        Request::Junction { torus, cylinder, radius_mm, tolerance } => junction(torus, cylinder, *radius_mm, *tolerance),
        Request::Shell { step, open, thickness_mm, tolerance } => shell(step, open, *thickness_mm, *tolerance),
        Request::Import { step, tolerance } => import(step, *tolerance),
    }
}

fn v(p: [f64; 3]) -> DVec3 {
    DVec3::new(p[0], p[1], p[2])
}

fn length(value: f64, what: &str) -> Result<f64> {
    ensure!(value.is_finite() && (1e-4..=100.0).contains(&value), "{what} must lie between 0.0001 and 100 mm, not {value}");
    Ok(value)
}

/// The one solid a STEP file holds.
fn one_solid(step: &str) -> Result<Solid> {
    let mut solids = Solid::read_step(&mut step.as_bytes()).map_err(|e| anyhow!("OpenCascade could not read the part: {e}"))?;
    ensure!(solids.len() == 1, "The part reads as {} solids, not one", solids.len());
    Ok(solids.remove(0))
}

/// The edge of `solid` all of `probe`'s points lie nearest, its ordinal, and how far the farthest point lies off it.
fn find_edge<'a>(solid: &'a Solid, probe: &EdgeProbe) -> Option<(usize, &'a Edge, f64)> {
    solid
        .iter_edge()
        .enumerate()
        .map(|(i, e)| (i, e, probe.points.iter().map(|p| (e.project(v(*p)).0 - v(*p)).length()).fold(0.0, f64::max)))
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

fn fillet(step: &str, probes: &[EdgeProbe], radius_mm: f64, tolerance: Tolerance) -> Result<(Vec<Built>, Vec<String>)> {
    let radius = length(radius_mm, "The fillet radius")?;
    ensure!(!probes.is_empty(), "Choose at least one edge to round");
    let solid = one_solid(step)?;
    let mut chosen: Vec<usize> = Vec::new();
    let mut edges = Vec::new();
    let mut notes = Vec::new();
    for (k, probe) in probes.iter().enumerate() {
        ensure!(!probe.points.is_empty(), "Edge {} carries no points", k + 1);
        let (i, edge, off) = find_edge(&solid, probe).context("OpenCascade reads the part with no edges")?;
        ensure!(off <= MATCH_MM, "Edge {} has no match in OpenCascade's reading of the part: the nearest lies {off:.4} mm off", k + 1);
        if let Some(same) = chosen.iter().position(|c| *c == i) {
            anyhow::bail!("Edges {} and {} are the same edge", same + 1, k + 1);
        }
        chosen.push(i);
        edges.push(edge);
        notes.push(format!("Edge {} is OpenCascade's edge {i}, {off:.1e} mm off", k + 1));
    }
    let rounded = solid.fillet_edges(radius, edges).map_err(|e| anyhow!("{e}"))?;
    Ok((vec![tessellate("Fillet", &rounded, tolerance)?], notes))
}

/// Ids of the edges bounding faces of `solid` that lie on a surface `pick` accepts.
fn edges_of(solid: &Solid, pick: impl Fn(&cadrum::SurfaceKind) -> bool) -> HashSet<u64> {
    solid.iter_face().filter(|f| f.surface().is_some_and(|s| pick(&s.kind))).flat_map(|f| f.iter_edge().map(Edge::id).collect::<Vec<_>>()).collect()
}

fn junction(torus: &Torus, cylinder: &Cylinder, radius_mm: f64, tolerance: Tolerance) -> Result<(Vec<Built>, Vec<String>)> {
    let (major, minor) = (length(torus.major_mm, "The torus's major radius")?, length(torus.minor_mm, "The torus's tube")?);
    ensure!(major > minor, "The torus's major radius must exceed its tube");
    let (r, h) = (length(cylinder.radius_mm, "The cylinder's radius")?, length(cylinder.height_mm, "The cylinder's height")?);
    let radius = length(radius_mm, "The fillet radius")?;
    let z = v(cylinder.frame.z).try_normalize().context("The cylinder's axis is zero")?;
    let ring = Solid::torus(major, minor, DVec3::Z);
    let post = Solid::cylinder(r, z * h).translate(v(cylinder.frame.origin) - z * (h / 2.0));
    let fused: Solid = (&ring + &post).build().map_err(|e| anyhow!("OpenCascade could not fuse the cylinder to the torus: {e}"))?;
    let (on_ring, on_post) = (
        edges_of(&fused, |k| matches!(k, cadrum::SurfaceKind::Torus { .. })),
        edges_of(&fused, |k| matches!(k, cadrum::SurfaceKind::Cylinder { .. })),
    );
    let seam: Vec<&Edge> = fused.iter_edge().filter(|e| on_ring.contains(&e.id()) && on_post.contains(&e.id())).collect();
    ensure!(!seam.is_empty(), "The cylinder does not meet the torus");
    let notes = vec![format!(
        "Fused along {} edge(s): {:.4} mm³ against {:.4} of torus and {:.4} of cylinder",
        seam.len(),
        fused.volume(),
        ring.volume(),
        post.volume()
    )];
    let rounded = fused.fillet_edges(radius, seam).map_err(|e| anyhow!("{e}"))?;
    Ok((vec![tessellate("Junction", &rounded, tolerance)?], notes))
}

fn shell(step: &str, open: &[[f64; 3]], thickness_mm: f64, tolerance: Tolerance) -> Result<(Vec<Built>, Vec<String>)> {
    let wall = length(thickness_mm, "The wall")?;
    let solid = one_solid(step)?;
    let off = |f: &Face, p: [f64; 3]| (f.project(v(p)).0 - v(p)).length();
    let faces: Vec<&Face> = open
        .iter()
        .map(|p| solid.iter_face().min_by(|a, b| off(a, *p).total_cmp(&off(b, *p))).context("OpenCascade reads the part with no faces"))
        .collect::<Result<_>>()?;
    let notes = open.iter().zip(&faces).map(|(p, f)| format!("Left open: the face {:.1e} mm from {p:?}", off(f, *p))).collect();
    let hollow = solid.shell(-wall, faces).map_err(|e| anyhow!("{e}"))?;
    Ok((vec![tessellate("Shell", &hollow, tolerance)?], notes))
}

fn import(step: &str, tolerance: Tolerance) -> Result<(Vec<Built>, Vec<String>)> {
    let solids = Solid::read_step(&mut step.as_bytes()).map_err(|e| anyhow!("OpenCascade could not read the file: {e}"))?;
    ensure!(!solids.is_empty(), "The file holds no solid");
    ensure!(solids.len() <= MAX_SOLIDS, "The file holds {} solids; an import takes at most {MAX_SOLIDS}", solids.len());
    let built = solids.iter().enumerate().map(|(i, s)| tessellate(&format!("Solid {}", i + 1), s, tolerance)).collect::<Result<Vec<_>>>()?;
    Ok((built, vec![format!("{} solid(s) read", solids.len())]))
}

/// A cadrum surface as the kernel names its kind.
fn kind(surface: Option<cadrum::Surface>) -> SurfaceKind {
    match surface.map(|s| s.kind) {
        Some(cadrum::SurfaceKind::Plane) => SurfaceKind::Plane,
        Some(cadrum::SurfaceKind::Cylinder { .. }) => SurfaceKind::Cylinder,
        Some(cadrum::SurfaceKind::Cone { .. }) => SurfaceKind::Cone,
        Some(cadrum::SurfaceKind::Sphere { .. }) => SurfaceKind::Sphere,
        Some(cadrum::SurfaceKind::Torus { .. }) => SurfaceKind::Torus,
        None => SurfaceKind::Freeform,
    }
}

/// Nodes within [`WELD_MM`] of each other as one vertex: the welded positions and each node's vertex.
fn weld(nodes: &[DVec3]) -> (Vec<[f64; 3]>, Vec<u32>) {
    let mut cells: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    let mut positions: Vec<[f64; 3]> = Vec::with_capacity(nodes.len());
    let mut remap = Vec::with_capacity(nodes.len());
    for n in nodes {
        let p = [n.x, n.y, n.z];
        let key = p.map(|c| (c / WELD_MM).floor() as i64);
        let mut found = None;
        'near: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &id in cells.get(&[key[0] + dx, key[1] + dy, key[2] + dz]).into_iter().flatten() {
                        let q = positions[id as usize];
                        if (0..3).map(|k| (q[k] - p[k]).powi(2)).sum::<f64>() <= WELD_MM * WELD_MM {
                            found = Some(id);
                            break 'near;
                        }
                    }
                }
            }
        }
        let id = found.unwrap_or_else(|| {
            positions.push(p);
            cells.entry(key).or_default().push(positions.len() as u32 - 1);
            positions.len() as u32 - 1
        });
        remap.push(id);
    }
    (positions, remap)
}

/// `solid` tessellated at `tolerance`, welded, refused unless it closes, and packed.
pub fn tessellate(name: &str, solid: &Solid, tolerance: Tolerance) -> Result<Built> {
    ensure!(tolerance.chord_mm.is_finite() && (1e-4..=1.0).contains(&tolerance.chord_mm), "The chord must lie between 0.0001 and 1 mm");
    ensure!(tolerance.angle_deg.is_finite() && (0.5..=45.0).contains(&tolerance.angle_deg), "The facet turn must lie between 0.5° and 45°");
    let options = Tessellation { deflection_linear: tolerance.chord_mm, deflection_angular: tolerance.angle_deg.to_radians(), relative_linear: false };
    let mesh = Solid::mesh([solid], options).map_err(|e| anyhow!("OpenCascade could not tessellate the {}: {e}", name.to_lowercase()))?;
    let faces: Vec<&Face> = solid.iter_face().collect();
    let ordinal: HashMap<u64, u32> = faces.iter().enumerate().map(|(i, f)| (f.id(), i as u32)).collect();
    let kinds: Vec<SurfaceKind> = faces.iter().map(|f| kind(f.surface())).collect();
    let (positions, remap) = weld(&mesh.vertices);
    let mut triangles = Vec::with_capacity(mesh.indices.len() / 3);
    let mut face_of = Vec::with_capacity(mesh.indices.len() / 3);
    for (t, corners) in mesh.indices.chunks_exact(3).enumerate() {
        let c = [remap[corners[0]], remap[corners[1]], remap[corners[2]]];
        if c[0] == c[1] || c[1] == c[2] || c[0] == c[2] {
            continue;
        }
        triangles.push(c);
        face_of.push(*mesh.face_ids.get(t).and_then(|id| ordinal.get(id)).context("A triangle names a face the solid does not hold")?);
    }
    let check = ringdesign_core::Mesh { vertices: positions.iter().map(|p| ringdesign_core::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), faces: triangles.clone(), ..Default::default() }.validate();
    ensure!(
        check.watertight,
        "OpenCascade's tessellation of the {} leaves {} open and {} non-manifold edges",
        name.to_lowercase(),
        check.boundary_edges,
        check.non_manifold_edges
    );
    let mesh_volume_mm3 = triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| v(positions[i as usize]));
            a.dot(b.cross(c)) / 6.0
        })
        .sum();
    Ok(Built {
        name: name.to_string(),
        brep_volume_mm3: solid.volume(),
        mesh_volume_mm3,
        faces: faces.len(),
        edges: solid.iter_edge().count(),
        mesh: Packed::encode(&positions, &triangles, &face_of, &kinds)?,
    })
}
