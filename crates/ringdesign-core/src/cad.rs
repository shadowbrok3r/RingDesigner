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
    /// A closed profile with no body of its own, for other features to sweep.
    Sketch {
        sketch: Sketch,
    },
    Extrude {
        sketch: Profile,
        height_mm: f64,
        draft_deg: f64,
    },
    Revolve {
        sketch: Profile,
        pivot: [f64; 3],
        axis: [f64; 3],
        degrees: f64,
    },
    Sweep {
        sketch: Profile,
        path: Vec<[f64; 3]>,
    },
    Twist {
        sketch: Profile,
        path: Sketch,
        degrees: f64,
        end_scale: f64,
    },
    Loft {
        sections: Vec<Profile>,
    },
    Boolean {
        a: Id,
        b: Id,
        kind: Boolean,
    },
    Fillet {
        source: Id,
        edges: Vec<EdgeRef>,
        radius_mm: f64,
    },
    Chamfer {
        source: Id,
        edges: Vec<EdgeRef>,
        base_face: FaceRef,
        distance_mm: f64,
    },
    Shell {
        source: Id,
        open_faces: Vec<FaceRef>,
        thickness_mm: f64,
    },
    Transform {
        source: Id,
        translation: [f64; 3],
        rotation_deg: [f64; 3],
    },
}
/// The closed profile a feature sweeps: drawn in the feature, or a `Sketch` feature named by id.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Profile {
    Feature { feature: Id },
    Inline(Sketch),
}
impl From<Sketch> for Profile {
    fn from(sketch: Sketch) -> Self {
        Self::Inline(sketch)
    }
}
impl Profile {
    pub fn feature(&self) -> Option<Id> {
        match self {
            Self::Feature { feature } => Some(*feature),
            Self::Inline(_) => None,
        }
    }
    pub fn sketch_mut(&mut self) -> Option<&mut Sketch> {
        match self {
            Self::Inline(sketch) => Some(sketch),
            Self::Feature { .. } => None,
        }
    }
    /// The features this profile reads: the sketch it names, or the face its own plane sits on.
    pub fn dependencies(&self) -> Vec<Id> {
        match self {
            Self::Feature { feature } => vec![*feature],
            Self::Inline(sketch) => sketch.plane.on_face.iter().map(|a| a.feature).collect(),
        }
    }
}
impl Operation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Band => "Procedural shank",
            Self::Sketch { .. } => "Sketch",
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
            Self::Extrude { sketch, .. }
            | Self::Revolve { sketch, .. }
            | Self::Sweep { sketch, .. }
            | Self::Twist { sketch, .. } => sketch.dependencies(),
            Self::Loft { sections } => sections.iter().flat_map(Profile::dependencies).collect(),
            Self::Sketch { sketch } => sketch.plane.on_face.iter().map(|a| a.feature).collect(),
            _ => vec![],
        }
    }
    pub fn sketch_mut(&mut self) -> Option<&mut Sketch> {
        match self {
            Self::Sketch { sketch } => Some(sketch),
            Self::Extrude { sketch, .. }
            | Self::Revolve { sketch, .. }
            | Self::Sweep { sketch, .. }
            | Self::Twist { sketch, .. } => sketch.sketch_mut(),
            Self::Loft { sections } => sections.first_mut().and_then(Profile::sketch_mut),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, from = "ComponentWire")]
pub struct Component {
    pub role: ComponentRole,
    pub material: String,
    pub manufacturing: Option<Setup>,
    pub visible: bool,
    pub reference: bool,
    pub stone_id: Option<String>,
    pub bench_notes: String,
    /// Where the built part stands; `Ring` follows resizing without stretching the part.
    pub placement: Placement,
}
/// The component as files before format 5 wrote it, with the anchor as two loose numbers.
#[derive(Deserialize)]
#[serde(default)]
struct ComponentWire {
    role: ComponentRole,
    material: String,
    manufacturing: Option<Setup>,
    visible: bool,
    reference: bool,
    stone_id: Option<String>,
    bench_notes: String,
    placement: Placement,
    ring_anchor_deg: Option<f64>,
    anchor_height_mm: f64,
}
impl Default for ComponentWire {
    fn default() -> Self {
        let c = Component::default();
        Self {
            role: c.role,
            material: c.material,
            manufacturing: c.manufacturing,
            visible: c.visible,
            reference: c.reference,
            stone_id: c.stone_id,
            bench_notes: c.bench_notes,
            placement: c.placement,
            ring_anchor_deg: None,
            anchor_height_mm: 0.0,
        }
    }
}
impl From<ComponentWire> for Component {
    fn from(w: ComponentWire) -> Self {
        let placement = match (w.placement, w.ring_anchor_deg) {
            (Placement::Free, Some(theta)) => Placement::ring(theta, w.anchor_height_mm),
            (placement, _) => placement,
        };
        Self {
            role: w.role,
            material: w.material,
            manufacturing: w.manufacturing,
            visible: w.visible,
            reference: w.reference,
            stone_id: w.stone_id,
            bench_notes: w.bench_notes,
            placement,
        }
    }
}
/// Where a component stands once its feature has built it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    /// As built, in world millimetres.
    #[default]
    Free,
    /// Seated on the ring's outer surface: `theta_deg` round the ring, `across_mm` along the
    /// finger from the band's mid-plane, `height_mm` out along the surface normal; then the part
    /// is turned about that normal (`spin`), leaned along the ring (`tilt`) and across it (`cant`).
    Ring {
        theta_deg: f64,
        #[serde(default)]
        across_mm: f64,
        #[serde(default)]
        height_mm: f64,
        #[serde(default)]
        spin_deg: f64,
        #[serde(default)]
        tilt_deg: f64,
        #[serde(default)]
        cant_deg: f64,
    },
}
impl Placement {
    pub fn ring(theta_deg: f64, height_mm: f64) -> Self {
        Self::Ring {
            theta_deg,
            across_mm: 0.0,
            height_mm,
            spin_deg: 0.0,
            tilt_deg: 0.0,
            cant_deg: 0.0,
        }
    }
    pub fn theta_deg(&self) -> Option<f64> {
        match self {
            Self::Ring { theta_deg, .. } => Some(*theta_deg),
            Self::Free => None,
        }
    }
    /// The rigid motion that takes the built part to where it stands.
    pub fn frame(&self, design: &RingDesign) -> Result<brep::Placement> {
        match *self {
            Self::Free => Ok(brep::Placement::IDENTITY),
            Self::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => {
                ensure!(
                    [theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg].iter().all(|v| v.is_finite()),
                    "Invalid ring placement"
                );
                let a = theta_deg.to_radians();
                let r = design.inner_radius_mm() + design.profile.thickness_mm + height_mm;
                // The part's z goes radial and its x runs along the finger; the leans turn it in place first.
                let seat = nalgebra::Rotation3::from_euler_angles(0.0, std::f64::consts::FRAC_PI_2, a);
                let lean = nalgebra::Rotation3::from_euler_angles(
                    tilt_deg.to_radians(),
                    cant_deg.to_radians(),
                    spin_deg.to_radians(),
                );
                let m = seat * lean;
                let col = |i| std::array::from_fn(|j| m.matrix()[(j, i)]);
                Ok(brep::Placement {
                    x_axis: col(0),
                    y_axis: col(1),
                    z_axis: col(2),
                    origin: [r * a.cos(), r * a.sin(), across_mm],
                })
            }
        }
    }
    /// A part-local point in world millimetres.
    pub fn world(&self, design: &RingDesign, p: [f64; 3]) -> Result<[f64; 3]> {
        let f = self.frame(design)?;
        Ok(std::array::from_fn(|k| f.origin[k] + f.x_axis[k] * p[0] + f.y_axis[k] * p[1] + f.z_axis[k] * p[2]))
    }
}
/// A world point or direction taken back into a placed part's own frame.
fn unplace(f: &brep::Placement, p: [f64; 3], point: bool) -> [f64; 3] {
    let d: [f64; 3] = if point { std::array::from_fn(|k| p[k] - f.origin[k]) } else { p };
    let dot = |axis: [f64; 3]| axis.iter().zip(d).map(|(a, b)| a * b).sum::<f64>();
    [dot(f.x_axis), dot(f.y_axis), dot(f.z_axis)]
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    Freeform,
}
impl SurfaceKind {
    fn of(surface: Option<&brep::Surface>) -> Self {
        match surface {
            Some(brep::Surface::Plane(_)) => Self::Plane,
            Some(brep::Surface::Cylinder(_)) => Self::Cylinder,
            Some(brep::Surface::Cone(_)) => Self::Cone,
            Some(brep::Surface::Sphere(_)) => Self::Sphere,
            Some(brep::Surface::Torus(_)) => Self::Torus,
            _ => Self::Freeform,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    Line,
    Circle,
    Ellipse,
    Spline,
    Freeform,
}
/// What an edge was when it was picked, in its part's own frame: enough to notice the source
/// changing underneath a fillet, and to find the edge again when only its ordinal moved.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EdgeSignature {
    pub faces: [SurfaceKind; 2],
    pub curve: CurveKind,
    pub direction: [f64; 3],
    pub midpoint: [f64; 3],
}
impl EdgeSignature {
    /// The same edge, allowing a resize or a draft to turn it a little.
    pub fn matches(&self, other: &Self) -> bool {
        self.faces == other.faces && self.curve == other.curve && self.alignment(other) >= 0.9
    }
    fn alignment(&self, other: &Self) -> f64 {
        self.direction.iter().zip(other.direction).map(|(a, b)| a * b).sum::<f64>().abs()
    }
    fn drift(&self, other: &Self) -> f64 {
        self.midpoint.iter().zip(other.midpoint).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt()
    }
    pub fn describe(&self) -> String {
        format!("{:?} between {:?} and {:?}", self.curve, self.faces[0], self.faces[1]).to_lowercase()
    }
}
/// What a face was when it was picked, in its part's own frame.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FaceSignature {
    pub kind: SurfaceKind,
    pub normal: [f64; 3],
    pub centre: [f64; 3],
}
impl FaceSignature {
    pub fn matches(&self, other: &Self) -> bool {
        self.kind == other.kind && self.alignment(other) >= 0.9
    }
    fn alignment(&self, other: &Self) -> f64 {
        self.normal.iter().zip(other.normal).map(|(a, b)| a * b).sum::<f64>().abs()
    }
    fn drift(&self, other: &Self) -> f64 {
        self.centre.iter().zip(other.centre).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt()
    }
}
/// One edge of a source body: its ordinal in the body's iteration order, and what stood there
/// when it was picked. A bare number in a file is an unsigned ordinal, as format 4 wrote them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(from = "EdgeRefWire")]
pub struct EdgeRef {
    pub ordinal: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<EdgeSignature>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum EdgeRefWire {
    Ordinal(usize),
    Full {
        ordinal: usize,
        #[serde(default)]
        signature: Option<EdgeSignature>,
    },
}
impl From<EdgeRefWire> for EdgeRef {
    fn from(w: EdgeRefWire) -> Self {
        match w {
            EdgeRefWire::Ordinal(ordinal) => Self { ordinal, signature: None },
            EdgeRefWire::Full { ordinal, signature } => Self { ordinal, signature },
        }
    }
}
impl EdgeRef {
    pub fn bare(ordinal: usize) -> Self {
        Self { ordinal, signature: None }
    }
    /// A reference that remembers the edge it points at, taken in the part's own frame.
    pub fn signed(body: &Body, ordinal: usize, frame: &brep::Placement) -> Self {
        Self { ordinal, signature: edge_signature(body, ordinal, frame) }
    }
}
/// One face of a source body, referenced like an edge.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(from = "FaceRefWire")]
pub struct FaceRef {
    pub ordinal: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<FaceSignature>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum FaceRefWire {
    Ordinal(usize),
    Full {
        ordinal: usize,
        #[serde(default)]
        signature: Option<FaceSignature>,
    },
}
impl From<FaceRefWire> for FaceRef {
    fn from(w: FaceRefWire) -> Self {
        match w {
            FaceRefWire::Ordinal(ordinal) => Self { ordinal, signature: None },
            FaceRefWire::Full { ordinal, signature } => Self { ordinal, signature },
        }
    }
}
impl FaceRef {
    pub fn bare(ordinal: usize) -> Self {
        Self { ordinal, signature: None }
    }
    pub fn signed(body: &Body, ordinal: usize, frame: &brep::Placement) -> Self {
        Self { ordinal, signature: face_signature(body, ordinal, frame) }
    }
}
/// The signature of the edge at `ordinal`, with its geometry taken back through `frame`.
pub fn edge_signature(body: &Body, ordinal: usize, frame: &brep::Placement) -> Option<EdgeSignature> {
    let (key, edge) = body.edges.iter().nth(ordinal)?;
    let mut faces: Vec<SurfaceKind> = edge
        .coedges
        .iter()
        .filter_map(|c| body.coedges.get(*c))
        .filter_map(|c| body.loops.get(c.owner))
        .filter_map(|l| body.faces.get(l.owner))
        .map(|f| SurfaceKind::of(body.surfaces.get(f.surface)))
        .collect();
    faces.sort();
    faces.resize(2, SurfaceKind::Freeform);
    let curve = match body.curves.get(edge.curve)? {
        brep::Curve3::Line(_) => CurveKind::Line,
        brep::Curve3::Circle(_) => CurveKind::Circle,
        brep::Curve3::Ellipse(_) => CurveKind::Ellipse,
        brep::Curve3::PlanarSpline { .. } => CurveKind::Spline,
        brep::Curve3::Nurbs(_) => CurveKind::Freeform,
    };
    let (a, b) = body.edge_endpoints(key)?;
    let (a, b) = (unplace(frame, a, true), unplace(frame, b, true));
    let d: [f64; 3] = std::array::from_fn(|k| b[k] - a[k]);
    let len = crate::mesh::norm(d);
    // A closed edge has no chord; its direction is the plane it lies in.
    let direction = if len > 1e-9 {
        d.map(|v| v / len)
    } else {
        let n = body.curves.get(edge.curve).and_then(|c| match c {
            brep::Curve3::Circle(c) => c.plane.normal(),
            brep::Curve3::Ellipse(e) => e.plane.normal(),
            _ => None,
        })?;
        unplace(frame, n, false)
    };
    Some(EdgeSignature {
        faces: [faces[0], faces[1]],
        curve,
        direction,
        midpoint: std::array::from_fn(|k| (a[k] + b[k]) * 0.5),
    })
}
/// The signature of the face at `ordinal`: its surface kind, and its normal and centre at the
/// mean of its boundary vertices, taken back through `frame`.
pub fn face_signature(body: &Body, ordinal: usize, frame: &brep::Placement) -> Option<FaceSignature> {
    let (key, face) = body.faces.iter().nth(ordinal)?;
    let surface = body.surfaces.get(face.surface)?;
    let points: Vec<[f64; 3]> = body
        .face_coedges(key)
        .into_iter()
        .filter_map(|c| body.coedge_vertices(c))
        .filter_map(|(v, _)| body.vertices.get(v))
        .map(|v| v.point)
        .collect();
    if points.is_empty() {
        return None;
    }
    let n = points.len() as f64;
    let centre: [f64; 3] = std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f64>() / n);
    let (u, v) = surface.parameters_at(centre).unwrap_or((0.0, 0.0));
    let mut normal = surface.normal_at(u, v)?;
    if !face.forward {
        normal = normal.map(|v| -v);
    }
    Some(FaceSignature {
        kind: SurfaceKind::of(Some(surface)),
        normal: unplace(frame, normal, false),
        centre: unplace(frame, centre, true),
    })
}
/// The edge a reference names now: its ordinal when the signature still holds, else the edge
/// whose signature matches best, else an error that says what was expected.
fn resolve_edge(body: &Body, r: &EdgeRef, frame: &brep::Placement, notes: &mut Vec<String>) -> Result<brep::EdgeKey> {
    let keys: Vec<_> = body.edges.iter().map(|(k, _)| k).collect();
    let at = |i: usize| {
        keys.get(i).copied().ok_or_else(|| {
            anyhow::anyhow!("Edge {i} is unavailable; reselect after changing the source")
        })
    };
    let Some(expected) = &r.signature else {
        return at(r.ordinal);
    };
    if edge_signature(body, r.ordinal, frame).is_some_and(|now| expected.matches(&now)) {
        return at(r.ordinal);
    }
    let best = (0..keys.len())
        .filter_map(|i| edge_signature(body, i, frame).map(|s| (i, s)))
        .filter(|(_, s)| expected.matches(s))
        .min_by(|a, b| expected.drift(&a.1).total_cmp(&expected.drift(&b.1)));
    match best {
        Some((i, _)) => {
            notes.push(format!("Edge {} is now edge {i}; the source changed underneath and the same edge was found again", r.ordinal));
            at(i)
        }
        None => anyhow::bail!(
            "Edge {} is no longer a {}; the source changed underneath, pick it again",
            r.ordinal,
            expected.describe()
        ),
    }
}
fn resolve_face(body: &Body, r: &FaceRef, frame: &brep::Placement, notes: &mut Vec<String>) -> Result<brep::FaceKey> {
    let keys: Vec<_> = body.faces.iter().map(|(k, _)| k).collect();
    let at = |i: usize| {
        keys.get(i).copied().ok_or_else(|| {
            anyhow::anyhow!("Face {i} is unavailable; reselect after changing the source")
        })
    };
    let Some(expected) = &r.signature else {
        return at(r.ordinal);
    };
    if face_signature(body, r.ordinal, frame).is_some_and(|now| expected.matches(&now)) {
        return at(r.ordinal);
    }
    let best = (0..keys.len())
        .filter_map(|i| face_signature(body, i, frame).map(|s| (i, s)))
        .filter(|(_, s)| expected.matches(s))
        .min_by(|a, b| expected.drift(&a.1).total_cmp(&expected.drift(&b.1)));
    match best {
        Some((i, _)) => {
            notes.push(format!("Face {} is now face {i}; the source changed underneath and the same face was found again", r.ordinal));
            at(i)
        }
        None => anyhow::bail!(
            "Face {} is no longer a {:?} face; the source changed underneath, pick it again",
            r.ordinal,
            expected.kind
        ),
    }
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
            placement: Placement::Free,
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
        // A sketch has no body to output.
        if !matches!(f.operation, Operation::Sketch { .. }) {
            self.outputs.push(f.id);
        }
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
    pub trace: PartTrace,
}
/// What the tessellation knows about the body it came from, so a triangle answers to a face and
/// a hover can name an edge or a vertex. Ordinals index the body's own iteration order.
#[derive(Clone, Debug, Default)]
pub struct PartTrace {
    /// The face ordinal behind each triangle; `u32::MAX` for a stitched gap.
    pub tri_face: Vec<u32>,
    /// Vertex positions in f64, welded like the mesh.
    pub positions: Vec<[f64; 3]>,
    /// The kind of surface under each face ordinal, for tinting and for what an edit may do.
    pub face_kind: Vec<SurfaceKind>,
    /// The body's vertices, for snapping.
    pub vertices: Vec<[f64; 3]>,
}
impl PartTrace {
    /// The face ordinal a triangle came from, if it has one.
    pub fn face_of(&self, triangle: usize) -> Option<u32> {
        self.tri_face.get(triangle).copied().filter(|f| *f != u32::MAX)
    }
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
    /// What the evaluation had to say about the feature, such as a reference found again.
    #[serde(default)]
    pub notes: Vec<String>,
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
/// The sketch a profile names, drawn in place or held by an earlier `Sketch` feature.
fn profile<'a>(p: &'a Profile, sketches: &'a BTreeMap<Id, Sketch>) -> Result<&'a Sketch> {
    match p {
        Profile::Inline(sketch) => Ok(sketch),
        Profile::Feature { feature } => sketches
            .get(feature)
            .ok_or_else(|| anyhow::anyhow!("Sketch feature #{feature} is unavailable or suppressed")),
    }
}
/// The plane a sketch lies on: its own, or the planar face of an earlier feature it is anchored to.
fn plane_of(
    sketch: &Sketch,
    bodies: &BTreeMap<Id, Body>,
    metadata: &BTreeMap<Id, &Feature>,
    design: &RingDesign,
    notes: &mut Vec<String>,
) -> Result<cadkernel::space::Plane> {
    let Some(anchor) = &sketch.plane.on_face else {
        return sketch.plane.plane();
    };
    let body = bodies
        .get(&anchor.feature)
        .ok_or_else(|| anyhow::anyhow!("Sketch face: feature #{} is unavailable or suppressed", anchor.feature))?;
    let frame = metadata
        .get(&anchor.feature)
        .map_or(Ok(brep::Placement::IDENTITY), |f| f.component.placement.frame(design))?;
    let key = resolve_face(body, &anchor.face, &frame, notes).context("Sketch face")?;
    let face = brep::planar_face_profile(body, key)
        .ok_or_else(|| anyhow::anyhow!("Sketch face {} of feature #{} is not planar", anchor.face.ordinal, anchor.feature))?;
    sketch.plane.on(face.plane.origin, face.outward)
}
fn body_for(
    op: &Operation,
    bodies: &BTreeMap<Id, Body>,
    sketches: &BTreeMap<Id, Sketch>,
    metadata: &BTreeMap<Id, &Feature>,
    design: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    notes: &mut Vec<String>,
) -> Result<Body> {
    let source = |id: &Id| {
        bodies
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Source feature #{id} is unavailable or suppressed"))
    };
    // References are signed in the source part's own frame, so its placement is taken back off.
    let frame_of = |id: &Id| -> Result<brep::Placement> {
        metadata
            .get(id)
            .map_or(Ok(brep::Placement::IDENTITY), |f| f.component.placement.frame(design))
    };
    fn edges(body: &Body, frame: &brep::Placement, refs: &[EdgeRef], notes: &mut Vec<String>) -> Result<Vec<brep::EdgeKey>> {
        ensure!(!refs.is_empty(), "Select at least one edge");
        refs.iter().map(|r| resolve_edge(body, r, frame, notes)).collect()
    }
    match op {
        Operation::Band => {
            let mut nominal = design.clone();
            nominal.cad = None;
            let resolved = crate::manufacturing::source_library(&nominal, lib);
            let built = crate::mesh::try_build(
                &nominal,
                &resolved,
                BuildParams {
                    refine: None,
                    ..params
                },
            )?;
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
        Operation::Sketch { .. } => anyhow::bail!("A sketch has no body of its own"),
        Operation::Extrude {
            sketch,
            height_mm,
            draft_deg,
        } => {
            let sketch = profile(sketch, sketches)?;
            let p = plane_of(sketch, bodies, metadata, design, notes)?;
            let h = positive(*height_mm, "Height")?;
            ensure!(
                draft_deg.is_finite() && draft_deg.abs() < 80.0,
                "Draft must be below 80 degrees"
            );
            maybe(
                brep::extrude_tapered(
                    p,
                    &sketch.profile_curves()?,
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
            let sketch = profile(sketch, sketches)?;
            maybe(
                brep::revolve(
                    plane_of(sketch, bodies, metadata, design, notes)?,
                    &sketch.profile_curves()?,
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
            let sketch = profile(sketch, sketches)?;
            maybe(
                brep::sweep_path(
                    plane_of(sketch, bodies, metadata, design, notes)?,
                    &[sketch.profile_curves()?],
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
            let sketch = profile(sketch, sketches)?;
            maybe(
                brep::sweep_along_deformed(
                    plane_of(sketch, bodies, metadata, design, notes)?,
                    &sketch.profile_curves()?,
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
                .map(|p| {
                    let s = profile(p, sketches)?;
                    Ok((plane_of(s, bodies, metadata, design, notes)?, s.profile_curves()?))
                })
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                profiles.iter().all(|(_, p)| p.len() == profiles[0].1.len()),
                "Loft sections need matching curve counts and winding"
            );
            maybe(brep::loft(&profiles), "Loft")
        }
        Operation::Boolean { a, b, kind } => {
            ensure!(a != b, "Boolean sources must be different");
            for id in [a, b] {
                let faces = source(id)?.faces.len();
                ensure!(
                    faces <= MAX_ANALYTIC_BOOLEAN_FACES,
                    "Feature #{id} is a faceted solid of {faces} faces; the analytic kernel cannot combine it in usable time. Keep it as a separate component"
                );
            }
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
            edges: refs,
            radius_mm,
        } => {
            let b = source(id)?;
            brep::fillet_edges(
                b,
                &edges(b, &frame_of(id)?, refs, notes)?,
                positive(*radius_mm, "Fillet radius")?,
            )
            .map_err(|e| anyhow::anyhow!("Fillet is unsupported for these edges/radius: {e:?}"))
        }
        Operation::Chamfer {
            source: id,
            edges: refs,
            base_face,
            distance_mm,
        } => {
            let b = source(id)?;
            let frame = frame_of(id)?;
            let face = resolve_face(b, base_face, &frame, notes).context("Chamfer base face")?;
            let mm = positive(*distance_mm, "Chamfer")?;
            brep::chamfer_edges(b, &edges(b, &frame, refs, notes)?, face, mm, mm)
                .map_err(|e| anyhow::anyhow!("Chamfer is unsupported: {e:?}"))
        }
        Operation::Shell {
            source: id,
            open_faces,
            thickness_mm,
        } => {
            let b = source(id)?;
            let frame = frame_of(id)?;
            let faces = open_faces
                .iter()
                .map(|r| resolve_face(b, r, &frame, notes).context("Shell opening face"))
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

/// Largest operand the analytic boolean accepts. Measured by `examples/kernel_probe.rs` on a
/// faceted torus with a cylinder through its tube: 256 faces 0.3 s, 576 faces 3.8 s, 1024 faces
/// 14 s, every one refused as `CutRefused`; 2304 faces and up never return. A faceted band is 24k.
pub const MAX_ANALYTIC_BOOLEAN_FACES: usize = 500;
/// The error text of an evaluation stopped through its `BuildCtx`.
pub const CANCELLED: &str = "CAD evaluation cancelled";

/// Cooperative stop for a running evaluation, polled between features and tessellations.
pub struct BuildCtx<'a> {
    pub cancel: &'a std::sync::atomic::AtomicBool,
}
impl BuildCtx<'_> {
    fn check(&self) -> Result<()> {
        ensure!(!self.cancel.load(std::sync::atomic::Ordering::Relaxed), CANCELLED);
        Ok(())
    }
}

pub fn evaluate(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Evaluated> {
    let never = std::sync::atomic::AtomicBool::new(false);
    evaluate_with(design, lib, params, &BuildCtx { cancel: &never })
}

pub fn evaluate_with(
    design: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    ctx: &BuildCtx,
) -> Result<Evaluated> {
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
    let mut sketches: BTreeMap<Id, Sketch> = BTreeMap::new();
    let mut metadata = BTreeMap::new();
    let mut reports = Vec::new();
    let mut ids = BTreeSet::new();
    let mut available_outputs = Vec::new();
    for f in &doc.features {
        ctx.check()?;
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
                notes: Vec::new(),
            });
        } else if let Operation::Sketch { sketch } = &f.operation {
            sketch
                .validate()
                .with_context(|| format!("Feature #{} — {}", f.id, f.name))?;
            sketches.insert(f.id, sketch.clone());
            metadata.insert(f.id, f);
            reports.push(FeatureReport {
                id: f.id,
                name: f.name.clone(),
                faces: 0,
                edges: 0,
                suppressed: false,
                notes: Vec::new(),
            });
        } else {
            let mut notes = Vec::new();
            let mut body = body_for(&f.operation, &bodies, &sketches, &metadata, design, lib, params, &mut notes)
                .with_context(|| format!("Feature #{} — {}", f.id, f.name))?;
            ensure!(
                body.validate().is_empty(),
                "Feature #{} — {} generated invalid topology: {:?}",
                f.id,
                f.name,
                body.validate()
            );
            if f.component.placement != Placement::Free {
                let frame = f.component.placement.frame(design)?;
                body = maybe(brep::transform(&body, &frame), "Ring placement")?;
            }
            reports.push(FeatureReport {
                id: f.id,
                name: f.name.clone(),
                faces: body.faces.len(),
                edges: body.edges.len(),
                suppressed: false,
                notes,
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
        ctx.check()?;
        let Some(body) = bodies.get(id) else {
            continue;
        };
        let f = metadata[id];
        let (mesh, trace) = tessellate_traced(
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
            trace,
        });
    }
    ensure!(!components.is_empty(), "No active CAD components");
    Ok(Evaluated {
        components,
        features: reports,
    })
}

pub fn tessellate(body: &Body, chord_mm: f64) -> Result<Mesh> {
    tessellate_traced(body, chord_mm).map(|(mesh, _)| mesh)
}
/// Tessellate and keep the kernel's triangle-to-face map through the weld and the stitch.
pub fn tessellate_traced(body: &Body, chord_mm: f64) -> Result<(Mesh, PartTrace)> {
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
    let face_ordinal: BTreeMap<u32, u32> = body
        .faces
        .iter()
        .enumerate()
        .map(|(i, (key, _))| (key.slot(), i as u32))
        .collect();
    let face_kind = body
        .faces
        .iter()
        .map(|(_, face)| SurfaceKind::of(body.surfaces.get(face.surface)))
        .collect();
    let mut tri_face = Vec::with_capacity(display.mesh.triangles.len());
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
    for (t, f) in display.mesh.triangles.iter().enumerate() {
        let f = f.map(|i| remap[i]);
        if f[0] != f[1] && f[1] != f[2] && f[2] != f[0] {
            mesh.faces.push(f);
            tri_face.push(display.triangle_faces.get(t).and_then(|k| face_ordinal.get(&k.slot()).copied()).unwrap_or(u32::MAX));
        }
    }
    ensure!(!mesh.faces.is_empty(), "Solid is empty");
    if !mesh.validate().watertight {
        stitch_chord_gaps(&mut mesh, body, chord_mm);
        tri_face.resize(mesh.faces.len(), u32::MAX);
    }
    let validation = mesh.validate();
    ensure!(
        validation.watertight,
        "Solid tessellation has {} open and {} nonmanifold edges",
        validation.boundary_edges,
        validation.non_manifold_edges
    );
    mesh.normals = normals(&mesh);
    let vertices = body
        .vertices
        .iter()
        .map(|(_, v)| v.point)
        .collect();
    Ok((mesh, PartTrace { tri_face, positions: precise, face_kind, vertices }))
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
            sketch: Sketch::rectangle(6.0, 4.0).into(),
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
            edges: vec![EdgeRef::bare(0)],
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
                sketch: section.into(),
                pivot: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                degrees: 360.0,
            }],
            vec![Operation::Loft {
                sections: vec![Sketch::rectangle(8.0, 6.0).into(), top.into()],
            }],
            vec![
                Operation::Box { size: [8.0; 3] },
                Operation::Fillet {
                    source: 1,
                    edges: vec![EdgeRef::bare(0)],
                    radius_mm: 0.5,
                },
            ],
            vec![
                Operation::Box { size: [8.0; 3] },
                Operation::Chamfer {
                    source: 1,
                    edges: vec![EdgeRef::bare(0)],
                    base_face: FaceRef::bare(4),
                    distance_mm: 0.5,
                },
            ],
            vec![Operation::Sweep {
                sketch: Sketch::circle(1.0).into(),
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
            sketch: s.into(),
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
    #[test]
    fn a_profile_drawn_out_of_order_extrudes_and_two_loops_say_so() {
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [6.0, 0.0], [0.0, 4.0]].map(|p| s.point(p));
        s.entity(crate::sketch::Geometry::Line { a: p[0], b: p[1] });
        s.entity(crate::sketch::Geometry::Line { a: p[0], b: p[2] });
        s.entity(crate::sketch::Geometry::Line { a: p[2], b: p[1] });
        let extrude = |sketch: Sketch| Operation::Extrude {
            sketch: sketch.into(),
            height_mm: 2.0,
            draft_deg: 0.0,
        };
        let lib = AlphaLibrary::builtin();
        let e = evaluate(&design(vec![extrude(s.clone())]), &lib, BuildParams::default()).unwrap();
        assert!((e.components[0].mesh.volume_mm3() - 24.0).abs() < 1e-3);
        let round = evaluate(&design(vec![extrude(Sketch::circle(3.0))]), &lib, BuildParams::default()).unwrap();
        let volume = round.components[0].mesh.volume_mm3();
        assert!((volume - std::f64::consts::PI * 9.0 * 2.0).abs() < 0.3, "{volume}");
        let q = [[10.0, 0.0], [12.0, 0.0], [11.0, 2.0]].map(|p| s.point(p));
        s.entity(crate::sketch::Geometry::Polyline {
            points: q.to_vec(),
            closed: true,
        });
        let error = evaluate(&design(vec![extrude(s)]), &lib, BuildParams::default())
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("2 separate loops"), "{error:#}");
    }
    #[test]
    fn a_traced_tessellation_names_the_face_and_kind_behind_every_triangle() {
        let d = design(vec![
            Operation::Box { size: [4.0, 6.0, 2.0] },
            Operation::Cylinder {
                radius_mm: 2.0,
                height_mm: 3.0,
            },
        ]);
        let e = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        let cube = &e.components[0];
        assert_eq!(cube.trace.tri_face.len(), cube.mesh.faces.len());
        let faces: BTreeSet<_> = cube.trace.tri_face.iter().copied().collect();
        assert_eq!(faces.len(), 6, "{faces:?}");
        assert!(cube.trace.face_kind.iter().all(|k| *k == SurfaceKind::Plane));
        assert_eq!(cube.trace.vertices.len(), 8);
        // Every triangle of one face shares that face's plane normal.
        for (t, f) in cube.mesh.faces.iter().enumerate() {
            let (a, b, c) = cube.mesh.triangle(f).unwrap();
            let n = crate::mesh::cross(crate::mesh::sub(b, a), crate::mesh::sub(c, a));
            let face = cube.trace.face_of(t).unwrap() as usize;
            let same: Vec<_> = cube.trace.tri_face.iter().enumerate().filter(|(_, o)| **o as usize == face).map(|(i, _)| i).collect();
            for i in same {
                let (p, q, r) = cube.mesh.triangle(&cube.mesh.faces[i]).unwrap();
                let m = crate::mesh::cross(crate::mesh::sub(q, p), crate::mesh::sub(r, p));
                let dot: f64 = n.iter().zip(m).map(|(x, y)| x * y).sum();
                assert!(dot > 0.0, "face {face} mixes orientations");
            }
        }
        let kinds: BTreeSet<_> = e.components[1].trace.face_kind.iter().map(|k| format!("{k:?}")).collect();
        assert_eq!(kinds, BTreeSet::from(["Cylinder".to_string(), "Plane".to_string()]));
        assert_eq!(e.components[1].trace.positions.len(), e.components[1].mesh.vertices.len());
    }
    #[test]
    fn a_signed_reference_is_found_again_when_its_ordinal_moves_and_refused_when_its_edge_is_gone() {
        let cube = make::cuboid([-2.0; 3], [4.0; 3]).unwrap();
        let identity = brep::Placement::IDENTITY;
        let keys: Vec<_> = cube.edges.iter().map(|(k, _)| k).collect();
        let picked = EdgeRef::signed(&cube, 5, &identity);
        let expected = picked.signature.clone().unwrap();
        assert_eq!(expected.faces, [SurfaceKind::Plane, SurfaceKind::Plane]);
        assert_eq!(expected.curve, CurveKind::Line);
        let mut notes = Vec::new();
        assert_eq!(resolve_edge(&cube, &picked, &identity, &mut notes).unwrap(), keys[5]);
        assert!(notes.is_empty());
        // The same signature stored under the wrong ordinal finds its edge and says so.
        let moved = EdgeRef { ordinal: 0, signature: Some(expected.clone()) };
        let found = resolve_edge(&cube, &moved, &identity, &mut notes).unwrap();
        assert_eq!(found, keys[5], "{notes:?}");
        assert_eq!(notes.len(), 1, "{notes:?}");
        // A rim of a cylinder is no edge of a cube.
        let cylinder = make::cylinder([0.0; 3], 2.0, 3.0).unwrap();
        let rim = EdgeRef::signed(&cylinder, 0, &identity);
        assert_eq!(rim.signature.as_ref().unwrap().curve, CurveKind::Circle);
        let error = resolve_edge(&cube, &rim, &identity, &mut notes).err().unwrap().to_string();
        assert!(error.contains("no longer a circle"), "{error}");
        // A signature is taken in the part's own frame, so seating the part does not change it.
        let d = RingDesign::default();
        let seat = Placement::ring(37.0, 0.4);
        let placed = brep::transform(&cube, &seat.frame(&d).unwrap()).unwrap();
        let there = EdgeRef::signed(&placed, 5, &seat.frame(&d).unwrap()).signature.unwrap();
        assert!(expected.matches(&there), "{expected:?} vs {there:?}");
        assert!(expected.drift(&there) < 1e-6);
        // Faces answer the same way, with their outward normal.
        let top = FaceRef::signed(&cube, 5, &identity);
        let sig = top.signature.clone().unwrap();
        assert_eq!(sig.kind, SurfaceKind::Plane);
        assert!((crate::mesh::norm(sig.normal) - 1.0).abs() < 1e-9);
        let wrong = FaceRef { ordinal: 0, signature: Some(sig) };
        let face_keys: Vec<_> = cube.faces.iter().map(|(k, _)| k).collect();
        assert_eq!(resolve_face(&cube, &wrong, &identity, &mut notes).unwrap(), face_keys[5]);
    }
    #[test]
    fn a_ring_placement_frame_is_the_anchor_frame_it_replaced() {
        let d = RingDesign::default();
        let theta: f64 = 130.0;
        let height = 0.7;
        let a = theta.to_radians();
        let r = d.inner_radius_mm() + d.profile.thickness_mm + height;
        let old = rotate_place([r * a.cos(), r * a.sin(), 0.0], [0.0, 90.0, theta]).unwrap();
        let new = Placement::ring(theta, height).frame(&d).unwrap();
        for (p, q) in [(old.x_axis, new.x_axis), (old.y_axis, new.y_axis), (old.z_axis, new.z_axis), (old.origin, new.origin)] {
            assert!(p.iter().zip(q).all(|(x, y)| (x - y).abs() < 1e-12), "{p:?} vs {q:?}");
        }
        // The part's z stands out of the ring, its x runs along the finger, and the leans turn it in place.
        let out = Placement::ring(theta, 0.0).world(&d, [0.0, 0.0, 1.0]).unwrap();
        let origin = Placement::ring(theta, 0.0).world(&d, [0.0; 3]).unwrap();
        let radial = [a.cos(), a.sin(), 0.0];
        assert!((0..3).all(|k| (out[k] - origin[k] - radial[k]).abs() < 1e-12));
        let along = Placement::ring(theta, 0.0).world(&d, [1.0, 0.0, 0.0]).unwrap();
        assert!((along[2] - origin[2] + 1.0).abs() < 1e-12);
        let spun = Placement::Ring { theta_deg: theta, across_mm: 0.5, height_mm: 0.0, spin_deg: 90.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let x = spun.world(&d, [1.0, 0.0, 0.0]).unwrap();
        let o = spun.world(&d, [0.0; 3]).unwrap();
        assert!((o[2] - 0.5).abs() < 1e-12);
        // A quarter spin about the normal takes the part's x onto its y: the ring's tangent.
        let tangent = [-a.sin(), a.cos(), 0.0];
        assert!((0..3).all(|k| (x[k] - o[k] - tangent[k]).abs() < 1e-9), "{x:?} {o:?}");
        // The evaluated anchor of a shipped example is unchanged by the migration.
        let e = evaluate(&crate::cad::examples::design("solitaire").unwrap(), &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        let bezel = e.components.iter().find(|c| c.name == "Open bezel stock").unwrap();
        let (lo, hi) = bezel.mesh.bounds().unwrap();
        assert!(hi.1 > 9.0 && lo.1 > 8.0, "the bezel stands over the top of the ring: {lo:?} {hi:?}");
    }
    #[test]
    fn a_sketch_feature_is_extruded_by_id_and_a_sketch_on_a_face_stands_on_it() {
        let lib = AlphaLibrary::builtin();
        // A plain sketch, extruded twice by id: the sketch has no body, the extrusions do.
        let mut doc = Document::default();
        let add = |doc: &mut Document, id: Id, operation: Operation| {
            doc.append(Feature { id, name: operation.label().into(), enabled: true, operation, component: Component::default() }).unwrap();
        };
        add(&mut doc, 1, Operation::Sketch { sketch: Sketch::rectangle(4.0, 3.0) });
        add(&mut doc, 2, Operation::Extrude { sketch: Profile::Feature { feature: 1 }, height_mm: 2.0, draft_deg: 0.0 });
        add(&mut doc, 3, Operation::Revolve { sketch: Profile::Feature { feature: 1 }, pivot: [6.0, 0.0, 0.0], axis: [0.0, 1.0, 0.0], degrees: 360.0 });
        assert_eq!(doc.outputs, vec![2, 3], "a sketch is never an output");
        let mut d = RingDesign::default();
        d.cad = Some(doc.clone());
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert_eq!(e.features[0].faces, 0);
        let volumes: Vec<f64> = e.components.iter().map(|c| c.mesh.volume_mm3()).collect();
        assert!((volumes[0] - 24.0).abs() < 1e-3, "{volumes:?}");
        assert!(volumes[1] > 24.0, "{volumes:?}");
        // Text says which sketch a profile came from, and reads back the same.
        let text = serde_json::to_string(&doc.features[1].operation).unwrap();
        assert!(text.contains(r#""sketch":{"feature":1}"#), "{text}");
        assert_eq!(serde_json::to_string(&serde_json::from_str::<Operation>(&text).unwrap()).unwrap(), text);
        let inline: Operation = serde_json::from_str(r#"{"Extrude":{"sketch":{"name":"x","points":[],"entities":[]},"height_mm":1.0,"draft_deg":0.0}}"#).unwrap();
        let Operation::Extrude { sketch: Profile::Inline(s), .. } = inline else { panic!("an inline sketch stays inline") };
        assert_eq!(s.name, "x");
        // A disabled sketch takes its extrusion down with it, by name.
        let mut off = doc.clone();
        off.features[0].enabled = false;
        d.cad = Some(off);
        let error = format!("{:#}", evaluate(&d, &lib, BuildParams::default()).err().unwrap());
        assert!(error.contains("Sketch feature #1 is unavailable"), "{error}");
        // A sketch on the top face of a box extrudes outward from that face, wherever the box stands.
        let mut doc = Document::default();
        add(&mut doc, 1, Operation::Box { size: [6.0, 6.0, 4.0] });
        let seat = Placement::ring(90.0, 0.0);
        doc.features[0].component.placement = seat.clone();
        let mut d = RingDesign::default();
        d.cad = Some(doc.clone());
        let boxed = evaluate(&d, &lib, BuildParams::default()).unwrap();
        let body = &boxed.components[0].body;
        let frame = seat.frame(&d).unwrap();
        // The face whose outward normal is the part's own +z, found by signature in the part's frame.
        let top = (0..body.faces.len())
            .find(|i| face_signature(body, *i, &frame).is_some_and(|s| s.normal[2] > 0.9))
            .expect("a top face");
        let mut sketch = Sketch::rectangle(2.0, 2.0);
        sketch.plane.on_face = Some(crate::sketch::FaceAnchor { feature: 1, face: FaceRef::signed(body, top, &frame) });
        add(&mut doc, 2, Operation::Extrude { sketch: sketch.into(), height_mm: 1.5, draft_deg: 0.0 });
        assert_eq!(doc.features[1].operation.sources(), vec![1], "the face is a dependency");
        d.cad = Some(doc);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        let boss = e.components.iter().find(|c| c.id == 2).unwrap();
        assert!((boss.mesh.volume_mm3() - 6.0).abs() < 1e-3);
        // The box's top face stands 2 mm out along the ring's radial at 90°: the boss starts there.
        let (lo, hi) = boss.mesh.bounds().unwrap();
        let top_r = d.inner_radius_mm() + d.profile.thickness_mm + 2.0;
        assert!((lo.1 as f64 - top_r).abs() < 1e-3 && (hi.1 as f64 - top_r - 1.5).abs() < 1e-3, "{lo:?} {hi:?} {top_r}");
    }
    #[test]
    fn a_faceted_operand_is_refused_before_the_kernel_is_asked() {
        let d = design(vec![
            Operation::Band,
            Operation::Cylinder {
                radius_mm: 3.0,
                height_mm: 3.0,
            },
            Operation::Boolean {
                a: 1,
                b: 2,
                kind: Boolean::Union,
            },
        ]);
        let started = std::time::Instant::now();
        let params = BuildParams {
            theta_steps: 128,
            profile_steps: 96,
            ..BuildParams::default()
        };
        let error = evaluate(&d, &AlphaLibrary::builtin(), params).err().unwrap();
        assert!(format!("{error:#}").contains("faceted solid"), "{error:#}");
        assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
    }
    #[test]
    fn a_raised_flag_stops_the_evaluation_between_features() {
        let d = design(vec![Operation::Box { size: [4.0; 3] }]);
        let stop = std::sync::atomic::AtomicBool::new(true);
        let error = evaluate_with(
            &d,
            &AlphaLibrary::builtin(),
            BuildParams::default(),
            &BuildCtx { cancel: &stop },
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), CANCELLED);
    }
}
