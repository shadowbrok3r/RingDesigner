//! Analytic feature recipes evaluated by the Rust CAD kernel. Graph nodes own
//! editable recipes; a RingDesign carries their evaluated, serializable result.
//! Unsupported operations return located errors, never substitute geometry.
use crate::{
    AlphaLibrary, BuildParams, Mesh, RingDesign, Vec3,
    manufacturing::Setup,
    sketch::{Id, Sketch},
};
use anyhow::{Context, Result, ensure};
use cadkernel::brep::{self, Body, make, mesh::TessellationTolerance};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub mod assembly;
pub mod examples;
pub mod measure;
pub mod step;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Boolean {
    Union,
    Subtract,
    Intersect,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    Band,
    Box {
        size: [f64; 3],
    },
    Cylinder {
        radius_mm: f64,
        height_mm: f64,
    },
    Sphere {
        radius_mm: f64,
    },
    Torus {
        major_mm: f64,
        minor_mm: f64,
    },
    TwistedRing {
        major_mm: f64,
        radial_mm: f64,
        axial_mm: f64,
        turns: f64,
    },
    Extrude {
        sketch: Sketch,
        height_mm: f64,
        draft_deg: f64,
    },
    Revolve {
        sketch: Sketch,
        pivot: [f64; 3],
        axis: [f64; 3],
        degrees: f64,
    },
    Sweep {
        sketch: Sketch,
        path: Vec<[f64; 3]>,
    },
    Twist {
        sketch: Sketch,
        path: Sketch,
        degrees: f64,
        end_scale: f64,
    },
    Loft {
        sections: Vec<Sketch>,
    },
    Boolean {
        a: Id,
        b: Id,
        kind: Boolean,
    },
    Fillet {
        source: Id,
        edges: Vec<usize>,
        radius_mm: f64,
    },
    Chamfer {
        source: Id,
        edges: Vec<usize>,
        base_face: usize,
        distance_mm: f64,
    },
    Shell {
        source: Id,
        open_faces: Vec<usize>,
        thickness_mm: f64,
    },
    Transform {
        source: Id,
        translation: [f64; 3],
        rotation_deg: [f64; 3],
    },
}
impl Operation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Band => "Procedural shank",
            Self::Box { .. } => "Box",
            Self::Cylinder { .. } => "Cylinder",
            Self::Sphere { .. } => "Sphere",
            Self::Torus { .. } => "Torus",
            Self::TwistedRing { .. } => "Twisted ring",
            Self::Extrude { .. } => "Extrude",
            Self::Revolve { .. } => "Revolve",
            Self::Sweep { .. } => "Sweep",
            Self::Twist { .. } => "Twisted sweep",
            Self::Loft { .. } => "Loft",
            Self::Boolean { kind, .. } => match kind {
                Boolean::Union => "Union",
                Boolean::Subtract => "Subtract",
                Boolean::Intersect => "Intersect",
            },
            Self::Fillet { .. } => "Fillet",
            Self::Chamfer { .. } => "Chamfer",
            Self::Shell { .. } => "Shell",
            Self::Transform { .. } => "Place component",
        }
    }
    pub fn sources(&self) -> Vec<Id> {
        match self {
            Self::Boolean { a, b, .. } => vec![*a, *b],
            Self::Fillet { source, .. }
            | Self::Chamfer { source, .. }
            | Self::Shell { source, .. }
            | Self::Transform { source, .. } => vec![*source],
            _ => vec![],
        }
    }
    pub fn sketch_mut(&mut self) -> Option<&mut Sketch> {
        match self {
            Self::Extrude { sketch, .. }
            | Self::Revolve { sketch, .. }
            | Self::Sweep { sketch, .. }
            | Self::Twist { sketch, .. } => Some(sketch),
            Self::Loft { sections } => sections.first_mut(),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Component {
    pub role: ComponentRole,
    pub material: String,
    pub manufacturing: Option<Setup>,
    pub visible: bool,
    pub reference: bool,
    pub stone_id: Option<String>,
    pub bench_notes: String,
    /// A ring angle follows resizing without stretching the component itself.
    pub ring_anchor_deg: Option<f64>,
    pub anchor_height_mm: f64,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentRole {
    Shank,
    Head,
    Setting,
    Inlay,
    Stone,
    #[default]
    Other,
}
impl Default for Component {
    fn default() -> Self {
        Self {
            role: ComponentRole::Other,
            material: "Silver 925".into(),
            manufacturing: None,
            visible: true,
            reference: false,
            stone_id: None,
            bench_notes: String::new(),
            ring_anchor_deg: None,
            anchor_height_mm: 0.0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feature {
    pub id: Id,
    pub name: String,
    pub enabled: bool,
    pub operation: Operation,
    #[serde(default)]
    pub component: Component,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Joint {
    pub a: Id,
    pub b: Id,
    pub clearance_mm: f64,
    pub method: String,
    pub notes: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub features: Vec<Feature>,
    pub outputs: Vec<Id>,
    pub joints: Vec<Joint>,
    pub through: Option<Id>,
}
impl Document {
    pub fn append(&mut self, f: Feature) -> Result<()> {
        ensure!(self.features.len() < 256, "At most 256 CAD features");
        ensure!(
            !self.features.iter().any(|v| v.id == f.id),
            "Duplicate feature identity #{}",
            f.id
        );
        for id in f.operation.sources() {
            self.outputs.retain(|v| *v != id);
        }
        self.outputs.push(f.id);
        self.features.push(f);
        Ok(())
    }
}
pub struct EvaluatedComponent {
    pub id: Id,
    pub name: String,
    pub settings: Component,
    pub body: Body,
    pub mesh: Mesh,
    pub edges: Vec<Vec<[f64; 3]>>,
}
pub struct Evaluated {
    pub components: Vec<EvaluatedComponent>,
    pub features: Vec<FeatureReport>,
}
#[derive(Clone, Debug, Serialize)]
pub struct FeatureReport {
    pub id: Id,
    pub name: String,
    pub faces: usize,
    pub edges: usize,
    pub suppressed: bool,
}

fn positive(value: f64, name: &str) -> Result<f64> {
    ensure!(
        value.is_finite() && value > 1e-5 && value < 10000.0,
        "{name} must be positive and below 10000 mm"
    );
    Ok(value)
}
fn coords(values: &[[f64; 3]]) -> Result<()> {
    ensure!(
        values
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() < 10000.0),
        "Invalid CAD coordinates"
    );
    Ok(())
}
fn maybe(body: Option<Body>, label: &str) -> Result<Body> {
    body.ok_or_else(|| anyhow::anyhow!("{label}: unsupported or degenerate geometry"))
}
fn rotate_place(translation: [f64; 3], degrees: [f64; 3]) -> Result<brep::Placement> {
    coords(&[translation, degrees])?;
    let r = nalgebra::Rotation3::from_euler_angles(
        degrees[0].to_radians(),
        degrees[1].to_radians(),
        degrees[2].to_radians(),
    );
    let col = |i| std::array::from_fn(|j| r.matrix()[(j, i)]);
    Ok(brep::Placement {
        x_axis: col(0),
        y_axis: col(1),
        z_axis: col(2),
        origin: translation,
    })
}
fn body_for(
    op: &Operation,
    bodies: &BTreeMap<Id, Body>,
    design: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
) -> Result<Body> {
    let source = |id: &Id| {
        bodies
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Source feature #{id} is unavailable or suppressed"))
    };
    let edges = |body: &Body, indices: &Vec<usize>| -> Result<Vec<brep::EdgeKey>> {
        let keys: Vec<_> = body.edges.iter().map(|(k, _)| k).collect();
        ensure!(!indices.is_empty(), "Select at least one edge");
        indices
            .iter()
            .map(|i| {
                keys.get(*i).copied().ok_or_else(|| {
                    anyhow::anyhow!("Edge {i} is unavailable; reselect after changing the source")
                })
            })
            .collect()
    };
    match op {
        Operation::Band => {
            let mut nominal = design.clone();
            nominal.cad = None;
            let resolved = crate::manufacturing::source_library(&nominal, lib);
            let built = crate::mesh::build_band(
                &nominal,
                &resolved,
                BuildParams {
                    refine: None,
                    ..params
                },
            );
            let vertices: Vec<_> = built
                .mesh
                .vertices
                .iter()
                .map(|p| [p.0 as f64, p.1 as f64, p.2 as f64])
                .collect();
            let faces: Vec<Vec<usize>> = built
                .mesh
                .faces
                .iter()
                .map(|f| f.map(|v| v as usize).to_vec())
                .collect();
            maybe(make::faceted_solid(&vertices, &faces), "Procedural shank")
        }
        Operation::Box { size } => {
            for x in size {
                positive(*x, "Box dimension")?;
            }
            maybe(make::cuboid(size.map(|v| -v / 2.0), *size), "Box")
        }
        Operation::Cylinder {
            radius_mm,
            height_mm,
        } => maybe(
            make::cylinder(
                [0.0, 0.0, -positive(*height_mm, "Height")? / 2.0],
                positive(*radius_mm, "Radius")?,
                *height_mm,
            ),
            "Cylinder",
        ),
        Operation::Sphere { radius_mm } => maybe(
            make::sphere([0.0; 3], positive(*radius_mm, "Radius")?),
            "Sphere",
        ),
        Operation::Torus { major_mm, minor_mm } => {
            ensure!(
                *major_mm > *minor_mm,
                "Torus major radius must exceed tube radius"
            );
            maybe(
                make::torus(
                    [0.0; 3],
                    positive(*major_mm, "Major radius")?,
                    positive(*minor_mm, "Tube radius")?,
                ),
                "Torus",
            )
        }
        Operation::TwistedRing {
            major_mm,
            radial_mm,
            axial_mm,
            turns,
        } => {
            for (v, name) in [
                (*major_mm, "Major radius"),
                (*radial_mm, "Radial diameter"),
                (*axial_mm, "Axial diameter"),
            ] {
                positive(v, name)?;
            }
            ensure!(
                *major_mm > radial_mm.max(*axial_mm) / 2.0,
                "Twisted section crosses the ring axis"
            );
            ensure!(
                turns.is_finite()
                    && turns.abs() <= 8.0
                    && (turns * 2.0 - (turns * 2.0).round()).abs() < 1e-8,
                "A closed elliptical section needs whole or half turns, at most eight"
            );
            let nt = params.theta_steps.clamp(128, 1024);
            let np = params.profile_steps.clamp(32, 128);
            let np = np + np % 2;
            let mut vertices = Vec::new();
            let mut faces = Vec::new();
            for i in 0..nt {
                let a = std::f64::consts::TAU * i as f64 / nt as f64;
                let twist = a * turns;
                for j in 0..np {
                    let p = std::f64::consts::TAU * j as f64 / np as f64;
                    let x = radial_mm * 0.5 * p.cos();
                    let y = axial_mm * 0.5 * p.sin();
                    let r = major_mm + x * twist.cos() - y * twist.sin();
                    vertices.push([r * a.cos(), r * a.sin(), x * twist.sin() + y * twist.cos()]);
                }
            }
            let seam = if (turns.round() - turns).abs() > 1e-8 {
                np / 2
            } else {
                0
            };
            for i in 0..nt {
                for j in 0..np {
                    let next = (i + 1) % nt;
                    let nj = if next == 0 { (j + seam) % np } else { j };
                    faces.push(vec![i * np + j, next * np + nj, next * np + (nj + 1) % np]);
                    faces.push(vec![
                        i * np + j,
                        next * np + (nj + 1) % np,
                        i * np + (j + 1) % np,
                    ]);
                }
            }
            maybe(make::faceted_solid(&vertices, &faces), "Twisted ring")
        }
        Operation::Extrude {
            sketch,
            height_mm,
            draft_deg,
        } => {
            let p = sketch.plane.plane()?;
            let h = positive(*height_mm, "Height")?;
            ensure!(
                draft_deg.is_finite() && draft_deg.abs() < 80.0,
                "Draft must be below 80 degrees"
            );
            maybe(
                brep::extrude_tapered(
                    p,
                    &sketch.solved_curves()?,
                    p.normal().unwrap().map(|v| v * h),
                    draft_deg.to_radians(),
                ),
                "Extrusion",
            )
        }
        Operation::Revolve {
            sketch,
            pivot,
            axis,
            degrees,
        } => {
            coords(&[*pivot, *axis])?;
            ensure!(
                degrees.is_finite() && *degrees > 0.0 && *degrees <= 360.0,
                "Revolution must be between 0 and 360 degrees"
            );
            ensure!(crate::mesh::norm(*axis) > 1e-8, "Revolution axis is zero");
            maybe(
                brep::revolve(
                    sketch.plane.plane()?,
                    &sketch.solved_curves()?,
                    *pivot,
                    *axis,
                    degrees.to_radians(),
                ),
                "Revolution",
            )
        }
        Operation::Sweep { sketch, path } => {
            ensure!(
                path.len() >= 2 && path.len() <= 128,
                "Sweep needs 2–128 path stations"
            );
            coords(path)?;
            maybe(
                brep::sweep_path(
                    sketch.plane.plane()?,
                    &[sketch.solved_curves()?],
                    brep::SweepPath::Polyline3d {
                        points: path,
                        closed: false,
                    },
                    brep::SweepOptions::default(),
                ),
                "Sweep",
            )
        }
        Operation::Twist {
            sketch,
            path,
            degrees,
            end_scale,
        } => {
            ensure!(
                degrees.is_finite() && degrees.abs() <= 3600.0,
                "Twist exceeds ten turns"
            );
            positive(*end_scale, "End scale")?;
            maybe(
                brep::sweep_along_deformed(
                    sketch.plane.plane()?,
                    &sketch.solved_curves()?,
                    path.plane.plane()?,
                    &path.solved_curves()?,
                    0.0,
                    degrees.to_radians(),
                    *end_scale,
                ),
                "Twisted sweep (polygon sections)",
            )
        }
        Operation::Loft { sections } => {
            ensure!(
                sections.len() >= 2 && sections.len() <= 32,
                "Loft needs 2–32 compatible sections"
            );
            let profiles = sections
                .iter()
                .map(|s| Ok((s.plane.plane()?, s.solved_curves()?)))
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                profiles.iter().all(|(_, p)| p.len() == profiles[0].1.len()),
                "Loft sections need matching curve counts and winding"
            );
            maybe(brep::loft(&profiles), "Loft")
        }
        Operation::Boolean { a, b, kind } => {
            ensure!(a != b, "Boolean sources must be different");
            let kind = match kind {
                Boolean::Union => brep::Operation::Union,
                Boolean::Subtract => brep::Operation::Difference,
                Boolean::Intersect => brep::Operation::Intersection,
            };
            brep::combine(source(a)?.clone(), source(b)?.clone(), kind, 1e-6)
                .map_err(|e| anyhow::anyhow!("Boolean cannot resolve this intersection: {e:?}"))
        }
        Operation::Fillet {
            source: id,
            edges: indices,
            radius_mm,
        } => {
            let b = source(id)?;
            brep::fillet_edges(
                b,
                &edges(b, indices)?,
                positive(*radius_mm, "Fillet radius")?,
            )
            .map_err(|e| anyhow::anyhow!("Fillet is unsupported for these edges/radius: {e:?}"))
        }
        Operation::Chamfer {
            source: id,
            edges: indices,
            base_face,
            distance_mm,
        } => {
            let b = source(id)?;
            let face = b
                .faces
                .iter()
                .nth(*base_face)
                .map(|(k, _)| k)
                .context("Chamfer base face is unavailable")?;
            let mm = positive(*distance_mm, "Chamfer")?;
            brep::chamfer_edges(b, &edges(b, indices)?, face, mm, mm)
                .map_err(|e| anyhow::anyhow!("Chamfer is unsupported: {e:?}"))
        }
        Operation::Shell {
            source: id,
            open_faces,
            thickness_mm,
        } => {
            let b = source(id)?;
            let faces = open_faces
                .iter()
                .map(|i| {
                    b.faces
                        .iter()
                        .nth(*i)
                        .map(|(k, _)| k)
                        .context("Shell opening face is unavailable")
                })
                .collect::<Result<Vec<_>>>()?;
            brep::shell(b, &faces, positive(*thickness_mm, "Shell thickness")?).map_err(|e| {
                anyhow::anyhow!("Shell supports analytic boxes, cylinders, and spheres: {e:?}")
            })
        }
        Operation::Transform {
            source: id,
            translation,
            rotation_deg,
        } => maybe(
            brep::transform(source(id)?, &rotate_place(*translation, *rotation_deg)?),
            "Placement",
        ),
    }
}

pub fn evaluate(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Evaluated> {
    let doc = design.cad.as_ref().context("No CAD features")?;
    ensure!(doc.features.len() <= 256, "Too many CAD features");
    ensure!(
        doc.through
            .is_none_or(|id| doc.features.iter().any(|f| f.id == id)),
        "Rollback feature does not exist"
    );
    ensure!(
        doc.outputs
            .iter()
            .all(|id| doc.features.iter().any(|f| f.id == *id)),
        "An output references a missing feature"
    );
    for j in &doc.joints {
        ensure!(
            j.a != j.b
                && j.clearance_mm.is_finite()
                && j.clearance_mm >= 0.0
                && doc.features.iter().any(|f| f.id == j.a)
                && doc.features.iter().any(|f| f.id == j.b),
            "Joint references missing/identical parts or an invalid clearance"
        );
    }
    let mut bodies = BTreeMap::new();
    let mut metadata = BTreeMap::new();
    let mut reports = Vec::new();
    let mut ids = BTreeSet::new();
    let mut available_outputs = Vec::new();
    for f in &doc.features {
        ensure!(ids.insert(f.id), "Duplicate feature #{}", f.id);
        if !f.enabled {
            if let Some(id) = f.operation.sources().first() {
                if let Some(body) = bodies.get(id).cloned() {
                    bodies.insert(f.id, body);
                    metadata.insert(f.id, f);
                }
            }
            reports.push(FeatureReport {
                id: f.id,
                name: f.name.clone(),
                faces: 0,
                edges: 0,
                suppressed: true,
            });
        } else {
            let mut body = body_for(&f.operation, &bodies, design, lib, params)
                .with_context(|| format!("Feature #{} — {}", f.id, f.name))?;
            ensure!(
                body.validate().is_empty(),
                "Feature #{} — {} generated invalid topology: {:?}",
                f.id,
                f.name,
                body.validate()
            );
            if let Some(theta) = f.component.ring_anchor_deg {
                ensure!(
                    theta.is_finite() && f.component.anchor_height_mm.is_finite(),
                    "Invalid ring anchor"
                );
                let a = theta.to_radians();
                let r = design.inner_radius_mm()
                    + design.profile.thickness_mm
                    + f.component.anchor_height_mm;
                body = maybe(
                    brep::transform(
                        &body,
                        &rotate_place([r * a.cos(), r * a.sin(), 0.0], [0.0, 90.0, theta])?,
                    ),
                    "Ring anchor",
                )?;
            }
            reports.push(FeatureReport {
                id: f.id,
                name: f.name.clone(),
                faces: body.faces.len(),
                edges: body.edges.len(),
                suppressed: false,
            });
            bodies.insert(f.id, body);
            metadata.insert(f.id, f);
        }
        for id in f.operation.sources() {
            available_outputs.retain(|v| *v != id);
        }
        if bodies.contains_key(&f.id) {
            available_outputs.push(f.id);
        }
        if doc.through == Some(f.id) {
            break;
        }
    }
    let output = if doc.through.is_some() {
        &available_outputs
    } else {
        &doc.outputs
    };
    let mut components = Vec::new();
    for id in output {
        let Some(body) = bodies.get(id) else {
            continue;
        };
        let f = metadata[id];
        let mesh = tessellate(
            body,
            if params.theta_steps >= 512 {
                0.015
            } else {
                0.04
            },
        )
        .with_context(|| format!("Feature #{id} — {}", f.name))?;
        let edges = if body.edges.len() <= 512 {
            body.edges
                .iter()
                .map(|(key, _)| brep::edge_points(body, key, 0.02).unwrap_or_default())
                .collect()
        } else {
            Vec::new()
        };
        components.push(EvaluatedComponent {
            id: *id,
            name: f.name.clone(),
            settings: f.component.clone(),
            body: body.clone(),
            mesh,
            edges,
        });
    }
    ensure!(!components.is_empty(), "No active CAD components");
    Ok(Evaluated {
        components,
        features: reports,
    })
}

pub fn tessellate(body: &Body, chord_mm: f64) -> Result<Mesh> {
    // Spline caps use angular sampling too; tighten it with export tolerance.
    let angle = (0.15 * (chord_mm / 0.04).sqrt()).clamp(0.01, 0.15);
    let display = brep::mesh::tessellate(
        body,
        TessellationTolerance::new(angle, 1e-7).with_chordal_deflection(chord_mm),
    );
    ensure!(
        display.missing_faces.is_empty(),
        "Kernel could not tessellate {} faces",
        display.missing_faces.len()
    );
    let mut mesh = Mesh::default();
    let mut index: BTreeMap<[i64; 3], Vec<u32>> = BTreeMap::new();
    let mut remap = Vec::new();
    let mut precise: Vec<[f64; 3]> = Vec::new();
    // Adjacent analytic faces carry independent normal vertices. Weld their
    // coincident coordinates before manifold checks and manufacturing export.
    for p in &display.mesh.positions {
        ensure!(
            p.iter().all(|v| v.is_finite()),
            "Non-finite solid tessellation"
        );
        let key = p.map(|v| (v * 1e6).floor() as i64);
        let mut found = None;
        // Adjacent cells matter: two coincident samples can straddle a grid
        // boundary. Welding by a rounded key alone leaves false open seams.
        'neighbors: for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if let Some(ids) = index.get(&[key[0] + x, key[1] + y, key[2] + z]) {
                        for id in ids {
                            if precise[*id as usize]
                                .iter()
                                .zip(p)
                                .map(|(a, b)| (a - b).powi(2))
                                .sum::<f64>()
                                <= 1e-12
                            {
                                found = Some(*id);
                                break 'neighbors;
                            }
                        }
                    }
                }
            }
        }
        let id = found.unwrap_or_else(|| {
            let id = mesh.vertices.len() as u32;
            mesh.vertices
                .push(Vec3(p[0] as f32, p[1] as f32, p[2] as f32));
            precise.push(*p);
            index.entry(key).or_default().push(id);
            id
        });
        remap.push(id);
    }
    for f in display.mesh.triangles {
        let f = f.map(|i| remap[i]);
        if f[0] != f[1] && f[1] != f[2] && f[2] != f[0] {
            mesh.faces.push(f);
        }
    }
    ensure!(!mesh.faces.is_empty(), "Solid is empty");
    if !mesh.validate().watertight {
        stitch_chord_gaps(&mut mesh, body, chord_mm);
    }
    let validation = mesh.validate();
    ensure!(
        validation.watertight,
        "Solid tessellation has {} open and {} nonmanifold edges",
        validation.boundary_edges,
        validation.non_manifold_edges
    );
    mesh.normals = normals(&mesh);
    Ok(mesh)
}
/// The kernel can refine one side of a shared spline boundary more than the
/// planar cap, leaving triangular slivers between two chord approximations.
/// Close only three-edge gaps whose entire width is within the requested chord
/// tolerance. No vertices move; larger/general holes remain export errors.
fn stitch_chord_gaps(mesh: &mut Mesh, body: &Body, chord_mm: f64) {
    if !body.validate().is_empty() {
        return;
    }
    let planes: Vec<_> = body
        .surfaces
        .iter()
        .filter_map(|(_, surface)| {
            if let brep::Surface::Plane(p) = surface {
                Some(*p)
            } else {
                None
            }
        })
        .collect();
    let mut edges: BTreeMap<(u32, u32), Vec<(u32, u32)>> = BTreeMap::new();
    for f in &mesh.faces {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push((a, b));
        }
    }
    if edges.values().any(|e| e.len() > 2) {
        return;
    }
    let mut boundary: BTreeSet<_> = edges
        .values()
        .filter(|e| e.len() == 1)
        .map(|e| e[0])
        .collect();
    if boundary.len() > 12_000 {
        return;
    }
    let mut adjacent: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (a, b) in &boundary {
        adjacent.entry(*a).or_default().push(*b);
    }
    let candidates: Vec<_> = boundary.iter().copied().collect();
    for (a, b) in candidates {
        if !boundary.contains(&(a, b)) {
            continue;
        }
        let Some(next) = adjacent.get(&b) else {
            continue;
        };
        for &c in next {
            if !boundary.contains(&(b, c)) || !boundary.contains(&(c, a)) || a == c {
                continue;
            }
            let Some((pa, pb, pc)) = mesh.triangle(&[a, b, c]) else {
                continue;
            };
            if !planes.iter().any(|p| {
                p.normal().is_some_and(|n| {
                    [pa, pb, pc].iter().all(|v| {
                        crate::mesh::sub(*v, p.origin)
                            .iter()
                            .zip(n)
                            .map(|(a, b)| a * b)
                            .sum::<f64>()
                            .abs()
                            < 1e-5
                    })
                })
            }) {
                continue;
            }
            let longest = [
                crate::mesh::norm(crate::mesh::sub(pa, pb)),
                crate::mesh::norm(crate::mesh::sub(pb, pc)),
                crate::mesh::norm(crate::mesh::sub(pc, pa)),
            ]
            .into_iter()
            .fold(0.0, f64::max);
            let twice_area = crate::mesh::norm(crate::mesh::cross(
                crate::mesh::sub(pb, pa),
                crate::mesh::sub(pc, pa),
            ));
            if twice_area > 1e-12 && twice_area / longest.max(1e-12) <= chord_mm {
                mesh.faces.push([a, c, b]);
                boundary.remove(&(a, b));
                boundary.remove(&(b, c));
                boundary.remove(&(c, a));
                break;
            }
        }
    }
}
pub fn normals(mesh: &Mesh) -> Vec<Vec3> {
    let mut n = vec![[0.0_f64; 3]; mesh.vertices.len()];
    for f in &mesh.faces {
        if let Some((a, b, c)) = mesh.triangle(f) {
            let v = crate::mesh::cross(crate::mesh::sub(b, a), crate::mesh::sub(c, a));
            for &id in f {
                for k in 0..3 {
                    n[id as usize][k] += v[k];
                }
            }
        }
    }
    n.into_iter()
        .map(|v| {
            let len = crate::mesh::norm(v).max(1e-12);
            Vec3(
                (v[0] / len) as f32,
                (v[1] / len) as f32,
                (v[2] / len) as f32,
            )
        })
        .collect()
}
pub fn combined(e: &Evaluated, include_reference: bool) -> Mesh {
    let mut mesh = Mesh::default();
    for c in &e.components {
        if c.settings.reference && !include_reference {
            continue;
        }
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend_from_slice(&c.mesh.vertices);
        mesh.normals.extend_from_slice(&c.mesh.normals);
        mesh.faces
            .extend(c.mesh.faces.iter().map(|f| f.map(|i| i + offset)));
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    fn design(ops: Vec<Operation>) -> RingDesign {
        let mut d = RingDesign::default();
        let mut doc = Document::default();
        for (i, op) in ops.into_iter().enumerate() {
            doc.append(Feature {
                id: i as u64 + 1,
                name: op.label().into(),
                enabled: true,
                operation: op,
                component: Component::default(),
            })
            .unwrap();
        }
        d.cad = Some(doc);
        d
    }
    #[test]
    fn analytic_primitives_and_hollow_box_are_closed_and_measured() {
        let lib = AlphaLibrary::builtin();
        let d = design(vec![
            Operation::Box { size: [10.0; 3] },
            Operation::Shell {
                source: 1,
                open_faces: vec![],
                thickness_mm: 1.0,
            },
        ]);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        let mesh = &e.components[0].mesh;
        assert!(mesh.validate().watertight);
        assert!((mesh.volume_mm3() - 488.0).abs() < 0.01);
    }
    #[test]
    fn extrusion_uses_analytic_sketch_dimensions() {
        let d = design(vec![Operation::Extrude {
            sketch: Sketch::rectangle(6.0, 4.0),
            height_mm: 3.0,
            draft_deg: 0.0,
        }]);
        let e = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        assert!((e.components[0].mesh.volume_mm3() - 72.0).abs() < 0.01);
    }
    #[test]
    fn missing_sources_are_located_and_source_document_is_unchanged() {
        let d = design(vec![Operation::Fillet {
            source: 99,
            edges: vec![0],
            radius_mm: 1.0,
        }]);
        let before = serde_json::to_string(&d).unwrap();
        let err = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default())
            .err()
            .unwrap();
        assert!(format!("{err:#}").contains("#99"));
        assert_eq!(before, serde_json::to_string(&d).unwrap());
    }
    #[test]
    fn primitives_revolve_loft_and_blends_produce_closed_meshes() {
        let mut section = Sketch::rectangle(2.0, 4.0);
        section.plane = crate::sketch::Workplane::section();
        for p in &mut section.points {
            p.xy[0] += 10.0;
        }
        let mut top = Sketch::rectangle(6.0, 4.0);
        top.plane.origin[2] = 5.0;
        let cases = vec![
            vec![Operation::Cylinder {
                radius_mm: 3.0,
                height_mm: 4.0,
            }],
            vec![Operation::Sphere { radius_mm: 3.0 }],
            vec![Operation::Torus {
                major_mm: 10.0,
                minor_mm: 1.0,
            }],
            vec![Operation::Revolve {
                sketch: section,
                pivot: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                degrees: 360.0,
            }],
            vec![Operation::Loft {
                sections: vec![Sketch::rectangle(8.0, 6.0), top],
            }],
            vec![
                Operation::Box { size: [8.0; 3] },
                Operation::Fillet {
                    source: 1,
                    edges: vec![0],
                    radius_mm: 0.5,
                },
            ],
            vec![
                Operation::Box { size: [8.0; 3] },
                Operation::Chamfer {
                    source: 1,
                    edges: vec![0],
                    base_face: 4,
                    distance_mm: 0.5,
                },
            ],
            vec![Operation::Sweep {
                sketch: Sketch::circle(1.0),
                path: vec![[0.0; 3], [0.0, 0.0, 5.0]],
            }],
        ];
        for ops in cases {
            let label = ops.last().unwrap().label();
            let d = design(ops);
            let e = evaluate(
                &d,
                &AlphaLibrary::builtin(),
                BuildParams {
                    theta_steps: 128,
                    profile_steps: 64,
                    refine: None,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("{label}: {e:#}"));
            assert!(
                e.components.iter().all(|c| c.mesh.validate().watertight),
                "{label}"
            );
        }
    }
    #[test]
    fn subtract_overlapping_boxes_and_half_twist_are_closed() {
        let d = design(vec![
            Operation::Box { size: [8.0; 3] },
            Operation::Box {
                size: [4.0, 4.0, 12.0],
            },
            Operation::Boolean {
                a: 1,
                b: 2,
                kind: Boolean::Subtract,
            },
        ]);
        let e = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        assert!((e.components[0].mesh.volume_mm3() - 384.0).abs() < 0.01);
        let d = design(vec![Operation::TwistedRing {
            major_mm: 10.0,
            radial_mm: 2.0,
            axial_mm: 4.0,
            turns: 0.5,
        }]);
        let e = evaluate(
            &d,
            &AlphaLibrary::builtin(),
            BuildParams {
                theta_steps: 128,
                profile_steps: 64,
                refine: None,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(e.components[0].mesh.validate().watertight);
    }
    #[test]
    fn cubic_extrusion_has_shared_closed_chords_and_correct_volume() {
        let mut s = Sketch::default();
        let p = [
            [-3.0, -2.0],
            [3.0, -2.0],
            [3.0, 2.0],
            [1.0, 4.0],
            [-1.0, 4.0],
            [-3.0, 2.0],
        ]
        .map(|p| s.point(p));
        s.entity(crate::sketch::Geometry::Line { a: p[0], b: p[1] });
        s.entity(crate::sketch::Geometry::Line { a: p[1], b: p[2] });
        s.entity(crate::sketch::Geometry::Bezier {
            points: [p[2], p[3], p[4], p[5]],
        });
        s.entity(crate::sketch::Geometry::Line { a: p[5], b: p[0] });
        let d = design(vec![Operation::Extrude {
            sketch: s,
            height_mm: 3.0,
            draft_deg: 0.0,
        }]);
        let e = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        let mesh = &e.components[0].mesh;
        assert!(mesh.validate().watertight);
        assert!((mesh.volume_mm3() - 90.0).abs() < 0.25);
        assert!(
            step::export(&e, "Cubic extrusion")
                .unwrap()
                .contains("B_SPLINE_SURFACE")
        );
    }
}
