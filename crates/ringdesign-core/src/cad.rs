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
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
pub mod assembly;
pub mod builders;
pub mod edit;
pub mod examples;
pub mod measure;
pub mod pattern;
pub mod step;
pub use pattern::{MirrorPlane, PatternKind, PlaneBase, WorkPlane};

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
    /// A part a [`builders`] builder makes: `key` names it, `on` what it stands on (read, not consumed), `params` its settings.
    Builder {
        key: String,
        #[serde(default)]
        on: Option<Id>,
        #[serde(default)]
        params: serde_json::Value,
    },
    /// Copies of `source` placed again, as one part of their own; the source stays a part beside them.
    Pattern {
        source: Id,
        kind: PatternKind,
    },
    /// A plane with no body, for sketches to lie on and mirrors to reflect across.
    Plane {
        base: PlaneBase,
        #[serde(default)]
        offset_mm: f64,
    },
    /// A planar face of a kernel part moved along its outward normal: out for a positive distance, in for a negative one.
    PressPull {
        source: Id,
        face: FaceRef,
        distance_mm: f64,
    },
}
/// The closed profile a feature sweeps: drawn in the feature, a `Sketch` feature named by id, or
/// one region of it. Untagged, so a region (`feature` and `region` keys) is tried before a whole
/// sketch (`feature` alone), and every file written before regions reads as it always did.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Profile {
    /// One region of a `Sketch` feature, swept alone; a twisted sweep or a loft takes its rim and refuses its holes.
    Region { feature: Id, region: crate::sketch::RegionRef },
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
            Self::Feature { feature } | Self::Region { feature, .. } => Some(*feature),
            Self::Inline(_) => None,
        }
    }
    /// The one region this profile names, when it names one.
    pub fn region(&self) -> Option<&crate::sketch::RegionRef> {
        match self {
            Self::Region { region, .. } => Some(region),
            _ => None,
        }
    }
    pub fn sketch_mut(&mut self) -> Option<&mut Sketch> {
        match self {
            Self::Inline(sketch) => Some(sketch),
            Self::Feature { .. } | Self::Region { .. } => None,
        }
    }
    /// The features this profile reads: the sketch it names, or the face its own plane sits on.
    pub fn dependencies(&self) -> Vec<Id> {
        match self {
            Self::Feature { feature } | Self::Region { feature, .. } => vec![*feature],
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
            Self::Builder { key, .. } => builders::label(key),
            Self::Pattern { kind, .. } => kind.label(),
            Self::Plane { .. } => "Work plane",
            Self::PressPull { .. } => "Press-pull",
        }
    }
    /// Whether the feature builds a body of its own: a sketch and a work plane do not.
    pub fn has_body(&self) -> bool {
        !matches!(self, Self::Sketch { .. } | Self::Plane { .. })
    }
    pub fn sources(&self) -> Vec<Id> {
        match self {
            Self::Builder { on, .. } => on.iter().copied().collect(),
            Self::Boolean { a, b, .. } => vec![*a, *b],
            Self::Pattern { source, kind } => std::iter::once(*source).chain(kind.reads()).collect(),
            Self::Plane { base, .. } => base.reads(),
            Self::Fillet { source, .. }
            | Self::Chamfer { source, .. }
            | Self::Shell { source, .. }
            | Self::PressPull { source, .. }
            | Self::Transform { source, .. } => vec![*source],
            Self::Extrude { sketch, .. } | Self::Revolve { sketch, .. } | Self::Sweep { sketch, .. } => sketch.dependencies(),
            Self::Twist { sketch, path, .. } => {
                let mut ids = sketch.dependencies();
                ids.extend(path.plane.on_face.iter().map(|a| a.feature));
                ids
            }
            Self::Loft { sections } => sections.iter().flat_map(Profile::dependencies).collect(),
            Self::Sketch { sketch } => sketch.plane.on_face.iter().map(|a| a.feature).collect(),
            _ => vec![],
        }
    }
    /// The sources this feature replaces among the outputs: never a sketch's face, a pattern's part or a plane's face.
    pub fn consumes(&self) -> Vec<Id> {
        match self {
            Self::Boolean { a, b, .. } => vec![*a, *b],
            Self::Fillet { source, .. }
            | Self::Chamfer { source, .. }
            | Self::Shell { source, .. }
            | Self::PressPull { source, .. }
            | Self::Transform { source, .. } => vec![*source],
            Self::Extrude { sketch, .. }
            | Self::Revolve { sketch, .. }
            | Self::Sweep { sketch, .. }
            | Self::Twist { sketch, .. } => sketch.feature().into_iter().collect(),
            Self::Loft { sections } => sections.iter().filter_map(Profile::feature).collect(),
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
    /// How the part meets the band: beside it, united into it, or subtracted from it.
    pub attach: Attach,
    /// Whether the part is poured with the pattern or added at the bench afterwards.
    pub stage: Stage,
    /// Radius of the seam bead laid along a `Join`/`Cut` junction; 0 lays none.
    pub blend_mm: f64,
}
/// How a component's solid meets the band once both are built.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Attach {
    /// Kept as its own solid beside the band; what a reference stone always is.
    #[default]
    Separate,
    /// United into the band, so the pattern and the verdict see one solid.
    Join,
    /// Subtracted from the band.
    Cut,
}
/// Which manufacturing stage a component belongs to.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Part of the pattern: poured with the ring.
    #[default]
    Cast,
    /// Added after the pour, soldered on or cut in: shown finished, never in a sand pattern.
    Bench,
}
impl Component {
    /// Joined into or cut from the band; a reference stone never is.
    pub fn attaches(&self) -> bool {
        !self.reference && matches!(self.attach, Attach::Join | Attach::Cut)
    }
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
    attach: Attach,
    stage: Stage,
    blend_mm: f64,
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
            attach: c.attach,
            stage: c.stage,
            blend_mm: c.blend_mm,
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
            attach: w.attach,
            stage: w.stage,
            blend_mm: w.blend_mm,
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
    /// [`Placement::frame`] seated on the built surface: a radial ray in the finger's plane at
    /// `theta_deg`, `across_mm` along the finger, meets `surface`, and the part stands `height_mm`
    /// out along the surface normal there with z along it and x along the finger. Without a
    /// surface, or where the ray misses, it is `frame()` bit for bit.
    pub fn frame_on(&self, design: &RingDesign, surface: Option<&Mesh>) -> Result<brep::Placement> {
        let Self::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } = *self else {
            return self.frame(design);
        };
        let Some(mesh) = surface else {
            return self.frame(design);
        };
        ensure!(
            [theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg].iter().all(|v| v.is_finite()),
            "Invalid ring placement"
        );
        let Some((hit, normal)) = surface_hit(mesh, theta_deg, across_mm) else {
            return self.frame(design);
        };
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let z = normal;
        // x runs along the finger as `frame()` has it, squared up to the normal.
        let along = [0.0, 0.0, -1.0];
        let d = dot(along, z);
        let x: [f64; 3] = std::array::from_fn(|k| along[k] - z[k] * d);
        let len = crate::mesh::norm(x);
        if len < 1e-6 {
            return self.frame(design);
        }
        let x = x.map(|v| v / len);
        let y = crate::mesh::cross(z, x);
        let lean = nalgebra::Rotation3::from_euler_angles(
            tilt_deg.to_radians(),
            cant_deg.to_radians(),
            spin_deg.to_radians(),
        );
        let l = lean.matrix();
        let col = |i: usize| -> [f64; 3] { std::array::from_fn(|k| x[k] * l[(0, i)] + y[k] * l[(1, i)] + z[k] * l[(2, i)]) };
        Ok(brep::Placement {
            x_axis: col(0),
            y_axis: col(1),
            z_axis: col(2),
            origin: std::array::from_fn(|k| hit[k] + z[k] * height_mm),
        })
    }
    /// A part-local point in world millimetres.
    pub fn world(&self, design: &RingDesign, p: [f64; 3]) -> Result<[f64; 3]> {
        let f = self.frame(design)?;
        Ok(std::array::from_fn(|k| f.origin[k] + f.x_axis[k] * p[0] + f.y_axis[k] * p[1] + f.z_axis[k] * p[2]))
    }
}
/// A stone's seat on a planar face of the part its builder's `on` names: offsets along [`FaceSeat::axes`], girdle height off it, spin about its normal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FaceSeat {
    pub face: FaceRef,
    #[serde(default)]
    pub u_mm: f64,
    #[serde(default)]
    pub v_mm: f64,
    #[serde(default)]
    pub height_mm: f64,
    #[serde(default)]
    pub spin_deg: f64,
}
impl FaceSeat {
    /// The key a stone builder's parameters carry its seat under.
    pub const KEY: &'static str = "seat";
    /// The seat a stone builder's parameters carry, if any; refused when it does not read or its numbers are not finite.
    pub fn of(params: &serde_json::Value) -> Result<Option<Self>> {
        let Some(v) = params.get(Self::KEY).filter(|v| !v.is_null()) else { return Ok(None) };
        let seat: Self = serde_json::from_value(v.clone()).context("Stone seat: expected a face and four numbers")?;
        ensure!(
            [seat.u_mm, seat.v_mm, seat.height_mm, seat.spin_deg].iter().all(|v| v.is_finite() && v.abs() < 1000.0),
            "Stone seat: a seat on a face needs finite numbers"
        );
        Ok(Some(seat))
    }
    /// `params` with this seat written in.
    pub fn write(&self, params: &mut serde_json::Value) {
        if !params.is_object() {
            *params = serde_json::json!({});
        }
        if let (Some(map), Ok(v)) = (params.as_object_mut(), serde_json::to_value(self)) {
            map.insert(Self::KEY.into(), v);
        }
    }
    /// Along and across `face` of a part seated by `part`: the part's axis lying most nearly on the face, flattened, and the normal crossed with it.
    pub fn axes(face: &crate::sketch::FaceFrame, part: &brep::Placement) -> [[f64; 3]; 2] {
        let n = face.normal;
        let mut best: Option<(f64, [f64; 3])> = None;
        for axis in [part.x_axis, part.y_axis, part.z_axis] {
            let d = (0..3).map(|k| axis[k] * n[k]).sum::<f64>();
            let flat: [f64; 3] = std::array::from_fn(|k| axis[k] - n[k] * d);
            let len = crate::mesh::norm(flat);
            if best.is_none_or(|(b, _)| len > b + 1e-6) {
                best = Some((len, flat));
            }
        }
        let x = best.filter(|(len, _)| *len > 1e-6).map_or(face.x, |(len, v)| v.map(|c| c / len));
        [x, crate::mesh::cross(n, x)]
    }
    /// The frame a part stands by on `face` of a part seated by `part`: z along the normal from its centroid, x turned `spin_deg` from the first axis.
    pub fn frame(&self, face: &crate::sketch::FaceFrame, part: &brep::Placement) -> brep::Placement {
        let [along, across] = Self::axes(face, part);
        let (s, c) = self.spin_deg.to_radians().sin_cos();
        let x: [f64; 3] = std::array::from_fn(|k| along[k] * c + across[k] * s);
        let y: [f64; 3] = std::array::from_fn(|k| across[k] * c - along[k] * s);
        let origin = std::array::from_fn(|k| face.origin[k] + along[k] * self.u_mm + across[k] * self.v_mm + face.normal[k] * self.height_mm);
        brep::Placement { x_axis: x, y_axis: y, z_axis: face.normal, origin }
    }
    /// A seat on planar face `face` of part `c` where `at` projects onto it (its centroid without one), `height_mm` off it, signed in the part's frame.
    pub fn on(c: &EvaluatedComponent, face: u32, at: Option<[f64; 3]>, height_mm: f64) -> Result<Self> {
        let body = match &c.made {
            Some(m) => anyhow::bail!("A stone sits on a kernel part's planar face; #{} {} is {}", c.id, c.name, Value::mesh_words(m)),
            None => &c.body,
        };
        let who = format!("#{} {}", c.id, c.name);
        let key = body.faces.iter().nth(face as usize).map(|(k, _)| k).ok_or_else(|| anyhow::anyhow!("{who} has no face {face}"))?;
        let frame = planar_face_frame(body, key, face as usize, &who)?;
        let [along, across] = Self::axes(&frame, &c.frame);
        let (u_mm, v_mm) = at.map_or((0.0, 0.0), |p| {
            let d: [f64; 3] = std::array::from_fn(|k| p[k] - frame.origin[k]);
            let on = |axis: [f64; 3]| (0..3).map(|k| d[k] * axis[k]).sum::<f64>();
            (on(along), on(across))
        });
        Ok(Self { face: FaceRef::signed(body, face as usize, &c.frame), u_mm, v_mm, height_mm, spin_deg: 0.0 })
    }
}
/// The own frame of face `key` of `body`, face `ordinal` of part `who`, refused by name when it is not planar.
fn planar_face_frame(body: &Body, key: brep::FaceKey, ordinal: usize, who: &str) -> Result<crate::sketch::FaceFrame> {
    let kind = SurfaceKind::of(body.faces.get(key).and_then(|f| body.surfaces.get(f.surface)));
    ensure!(
        kind == SurfaceKind::Plane,
        "Face {ordinal} of {who} is a {} face; a stone sits on a planar one",
        format!("{kind:?}").to_lowercase()
    );
    let profile = brep::planar_face_profile(body, key).ok_or_else(|| anyhow::anyhow!("Face {ordinal} of {who} has no boundary the kernel can read"))?;
    crate::sketch::FaceFrame::of(&profile).with_context(|| format!("Face {ordinal} of {who}"))
}
/// The frame a stone standing on `seat` of part `on` takes from that part as built, its face found again by signature.
fn face_seat_frame(
    seat: &FaceSeat,
    on: Id,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    who: &dyn Fn(Id) -> String,
    notes: &mut Vec<String>,
) -> Result<brep::Placement> {
    let body = match values.get(&on) {
        Some(Value::Brep(body)) => body,
        Some(Value::Mesh(m)) => anyhow::bail!("A stone sits on a kernel part's planar face; {} is {}", who(on), Value::mesh_words(m)),
        None => anyhow::bail!("Source feature #{on} is unavailable or suppressed"),
    };
    let frame = frames.get(&on).copied().unwrap_or(brep::Placement::IDENTITY);
    let key = resolve_face(body, &seat.face, &frame, notes).with_context(|| format!("The face the stone stands on, of {}", who(on)))?;
    let ordinal = body.faces.iter().position(|(k, _)| k == key).unwrap_or(seat.face.ordinal);
    Ok(seat.frame(&planar_face_frame(body, key, ordinal, &who(on))?, &frame))
}
/// A reference stone feature for `gem` standing on `seat`, a planar face of part `on`.
pub fn stone_on_face(id: Id, gem: crate::gem::Gem, on: Id, seat: &FaceSeat) -> Feature {
    let mut f = builders::stone_feature(id, gem, Placement::Free);
    if let Operation::Builder { on: stands, params, .. } = &mut f.operation {
        *stands = Some(on);
        seat.write(params);
    }
    f
}
#[cfg(test)]
mod placement_tests {
    use super::*;
    use crate::gem::{Gem, GemCut};

    const PLATE: Id = 2;
    const STONE: Id = 3;

    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..BuildParams::default() }
    }
    fn gem() -> Gem {
        Gem::calibrated(GemCut::Round, 3.0)
    }
    /// The Court band with a plate of `size` joined at `theta`, and whatever else `more` adds.
    fn plated(size: [f64; 3], theta: f64, more: Vec<Feature>) -> RingDesign {
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let component = Component { attach: Attach::Join, placement: Placement::ring(theta, 0.0), ..Component::default() };
        doc.append(Feature { id: PLATE, name: "Plate".into(), enabled: true, operation: Operation::Box { size }, component }).unwrap();
        for f in more {
            doc.append(f).unwrap();
        }
        d.cad = Some(doc);
        d
    }
    fn built(d: &RingDesign) -> crate::mesh::BuildResult {
        crate::mesh::try_build(d, &AlphaLibrary::builtin(), params()).unwrap()
    }
    fn part(b: &crate::mesh::BuildResult, id: Id) -> EvaluatedComponent {
        b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("#{id}: {:?}", b.parts.notes)).clone()
    }
    /// The plate's face whose outward normal points furthest from the finger's axis.
    fn top(plate: &EvaluatedComponent) -> u32 {
        let out = |f: u32| {
            let key = plate.body.faces.iter().nth(f as usize).unwrap().0;
            planar_face_frame(&plate.body, key, f as usize, "plate").map_or(f64::MIN, |fr| {
                let r = fr.origin[0].hypot(fr.origin[1]);
                (fr.normal[0] * fr.origin[0] + fr.normal[1] * fr.origin[1]) / r
            })
        };
        (0..plate.body.faces.len() as u32).max_by(|a, b| out(*a).total_cmp(&out(*b))).unwrap()
    }
    fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
        (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
    }

    #[test]
    fn a_stone_on_a_plates_top_rides_the_plate_when_it_moves_or_grows() {
        let size = [4.0, 6.0, 2.0];
        let bare = built(&plated(size, 90.0, vec![]));
        let plate = part(&bare, PLATE);
        let face = top(&plate);
        let h = builders::stand_off_mm("claw4", gem());
        // Seated 0.4 mm round the ring from the face's centre, its girdle the claws' stand-off over it.
        let key = plate.body.faces.iter().nth(face as usize).unwrap().0;
        let own = planar_face_frame(&plate.body, key, face as usize, "plate").unwrap();
        let [_, round] = FaceSeat::axes(&own, &plate.frame);
        let at: [f64; 3] = std::array::from_fn(|k| own.origin[k] + round[k] * 0.4);
        let seat = FaceSeat::on(&plate, face, Some(at), h).unwrap();
        assert!(seat.face.signature.is_some() && seat.u_mm.abs() < 1e-9 && (seat.v_mm - 0.4).abs() < 1e-9, "{seat:?}");
        let stone = stone_on_face(STONE, gem(), PLATE, &seat);
        assert_eq!(stone.operation.sources(), vec![PLATE], "the plate is the stone's source, so the funnel and the cache see it");
        let on = |size, theta| {
            let b = built(&plated(size, theta, vec![stone.clone()]));
            assert!(b.report.validation.watertight && b.parts.notes.is_empty(), "{:?} {:?}", b.report.validation, b.parts.notes);
            assert_eq!((b.parts.joined, b.parts.references), (1, 1));
            (part(&b, STONE), part(&b, PLATE))
        };
        let (s0, p0) = on(size, 90.0);
        let face0 = planar_face_frame(&p0.body, key, face as usize, "plate").unwrap();
        let [_, round0] = FaceSeat::axes(&face0, &p0.frame);
        let expect: [f64; 3] = std::array::from_fn(|k| face0.origin[k] + round0[k] * 0.4 + face0.normal[k] * h);
        assert!(gap(s0.frame.origin, expect) < 1e-9, "{:?} against {expect:?}", s0.frame.origin);
        assert!(gap(s0.frame.z_axis, face0.normal) < 1e-12, "the stone's table faces along the face's normal");
        assert_eq!(s0.made.as_ref().and_then(|m| m.seat).map(|s| s.surface_z), Some(-h), "the metal under the girdle is the face");
        // The plate moved 25° round the ring carries the stone with it.
        let (s1, _) = on(size, 65.0);
        let turn = |p: [f64; 3], deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
        };
        // Measured 0.00055 mm: the swept band's facets at the two angles.
        let rode = gap(s1.frame.origin, turn(s0.frame.origin, -25.0));
        assert!(rode < 2e-3, "{rode:.5} mm off the plate's own turn");
        // The plate grown a millimetre about its centre lifts its top, and the stone, half of it.
        let (s2, _) = on([4.0, 6.0, 3.0], 90.0);
        let lift: [f64; 3] = std::array::from_fn(|k| s2.frame.origin[k] - s0.frame.origin[k]);
        assert!(gap(lift, face0.normal.map(|v| v * 0.5)) < 1e-9, "{lift:?}");
        // Grown across the band past its length, the face's longest edge turns a quarter; the stone keeps its bearing and its place.
        let (s3, p3) = on([6.5, 6.0, 2.0], 90.0);
        let face3 = planar_face_frame(&p3.body, key, face as usize, "plate").unwrap();
        let swing = (0..3).map(|k| face3.x[k] * face0.x[k]).sum::<f64>();
        assert!(swing.abs() < 1e-9, "the longest edge ran round the ring and now runs across it: {swing}");
        assert!(gap(s3.frame.x_axis, s0.frame.x_axis) < 1e-9 && gap(s3.frame.origin, s0.frame.origin) < 1e-9, "{:?} against {:?}", s3.frame, s0.frame);
    }

    #[test]
    fn claws_round_a_stone_on_a_plate_stand_on_the_plate_not_the_band_under_it() {
        let size = [4.0, 6.0, 2.0];
        let plate = part(&built(&plated(size, 90.0, vec![])), PLATE);
        let h = builders::stand_off_mm("claw4", gem());
        let seat = FaceSeat::on(&plate, top(&plate), None, h).unwrap();
        let mut next = 4;
        let settings = builders::setting_features("claw4", STONE, gem(), false, &mut || {
            next += 1;
            next - 1
        })
        .unwrap();
        let mut more = vec![stone_on_face(STONE, gem(), PLATE, &seat)];
        more.extend(settings);
        let b = built(&plated(size, 90.0, more));
        assert!(b.report.validation.watertight, "{:?} {:?}", b.report.validation, b.parts.notes);
        assert_eq!((b.parts.joined, b.parts.cut, b.parts.references), (2, 1, 1), "{:?}", b.parts.notes);
        let (stone, head) = (part(&b, STONE), part(&b, 4));
        assert_eq!(head.frame, stone.frame, "the head stands in its stone's frame");
        let f = stone.frame;
        let low = head.trace.positions.iter().map(|p| (0..3).map(|k| (p[k] - f.origin[k]) * f.z_axis[k]).sum::<f64>()).fold(f64::MAX, f64::min);
        // The feet end 0.90 mm into the plate; read off the band alone they ran through it to 1.70.
        assert!((low + h + 0.90).abs() < 0.02, "the claws end {low:.3} mm under the girdle, over a plate top at {:.3}", -h);
    }

    #[test]
    fn a_ray_down_a_swept_meridian_lands_on_the_near_surface_not_across_the_finger_hole() {
        // Size 9 on a 256-step sweep: the ray at 90° slipped through the crest seam onto the far side's bore.
        let mut d = examples::design("claw-solitaire").unwrap();
        d.size = crate::sizing::RingSize(9.0);
        let params = BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..d.build };
        let band = crate::mesh::try_build_pattern(&d, &AlphaLibrary::builtin(), params).unwrap().band.unwrap();
        // The pattern's band carries the bench bur's drill mark at 90°, so the crest there stands proud.
        let (crest, mark) = (d.inner_radius_mm() + d.profile.thickness_mm, 0.3);
        for theta in [0.0, 90.0, 180.0, 270.0] {
            let (hit, n) = surface_hit(&band, theta, 0.0).unwrap();
            let (s, c) = f64::to_radians(theta).sin_cos();
            let along = hit[0] * c + hit[1] * s;
            assert!(along > crest - 0.01 && along < crest + mark && n[0] * c + n[1] * s > 0.98, "{theta}°: {hit:?} {n:?} against a crest at {crest:.4}");
        }
    }

    #[test]
    fn a_stone_asks_for_a_planar_face_and_a_free_placement_by_name() {
        let plate = part(&built(&plated([4.0, 6.0, 2.0], 90.0, vec![])), PLATE);
        let mut d = plated([4.0, 6.0, 2.0], 90.0, vec![]);
        let post = Feature { id: 4, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Component { placement: Placement::ring(200.0, 0.5), attach: Attach::Join, ..Component::default() } };
        d.cad.as_mut().unwrap().append(post).unwrap();
        let cylinder = part(&built(&d), 4);
        let side = (0..cylinder.body.faces.len() as u32).find(|f| cylinder.trace.face_kind[*f as usize] == SurfaceKind::Cylinder).unwrap();
        let refused = FaceSeat::on(&cylinder, side, None, 1.0).unwrap_err().to_string();
        assert_eq!(refused, format!("Face {side} of #4 Post is a cylinder face; a stone sits on a planar one"));
        let seat = FaceSeat::on(&plate, top(&plate), None, 1.0).unwrap();
        let mut stone = stone_on_face(STONE, gem(), PLATE, &seat);
        stone.component.placement = Placement::ring(90.0, 1.0);
        let e = evaluate(&plated([4.0, 6.0, 2.0], 90.0, vec![stone]), &AlphaLibrary::builtin(), params()).unwrap();
        assert_eq!(e.first_error().unwrap(), "Feature #3 — Round 3 mm: A stone on a face of #2 Plate stands where that face is; its placement stays free");
        // A seat reads back from the parameters it was written into, and a malformed one is refused.
        let mut params = builders::stone_params(gem());
        seat.write(&mut params);
        assert_eq!(FaceSeat::of(&params).unwrap(), Some(seat));
        params[FaceSeat::KEY]["u_mm"] = serde_json::json!(f64::NAN.to_string());
        assert!(FaceSeat::of(&params).is_err());
    }

    #[test]
    fn a_bare_seat_is_signed_at_commit_in_the_frame_its_part_was_seated_by() {
        let b = built(&plated([4.0, 6.0, 2.0], 90.0, vec![]));
        let plate = part(&b, PLATE);
        let signed = FaceSeat::on(&plate, top(&plate), None, 1.0).unwrap();
        let bare = FaceSeat { face: FaceRef::bare(signed.face.ordinal), ..signed.clone() };
        let mut op = stone_on_face(STONE, gem(), PLATE, &bare).operation;
        let e = b.parts.evaluated.as_ref().unwrap();
        assert_eq!(sign_refs(&mut op, e), 1);
        let Operation::Builder { params, .. } = &op else { unreachable!() };
        assert_eq!(FaceSeat::of(params).unwrap(), Some(signed));
        assert_eq!(sign_refs(&mut op, e), 0, "a signed seat is left as it is");
    }
}
/// Where a radial ray in the finger's plane at `theta_deg`, `z = across_mm`, first meets the
/// surface from outside, with the smooth outward normal there (the face's own when the mesh
/// carries no vertex normals). A ray exactly on the crest loop slips between the faces that
/// share it and lands on the bore beyond, so it runs a tenth of a micron off the plane first
/// and a hit on a face turned away from it is a miss.
pub fn surface_hit(mesh: &Mesh, theta_deg: f64, across_mm: f64) -> Option<([f64; 3], [f64; 3])> {
    let (lo, hi) = mesh.bounds()?;
    // Past every vertex's radius: at 45° a ring reaches further than either axis extent.
    let far = (lo.0.abs().max(hi.0.abs()) as f64).hypot(lo.1.abs().max(hi.1.abs()) as f64) + 1.0;
    let (sin, cos) = theta_deg.to_radians().sin_cos();
    let direction = [(-cos) as f32, (-sin) as f32, 0.0];
    // Rays along a swept meridian can slip between its faces, so the last tries step off it sideways.
    for (dz, side) in [(1e-4, 0.0), (-1e-4, 0.0), (0.0, 0.0), (1e-3, 0.0), (-1e-3, 0.0), (1e-4, 1e-4), (-1e-4, -1e-4), (1e-3, -1e-3)] {
        let origin = [(far * cos - side * sin) as f32, (far * sin + side * cos) as f32, (across_mm + dz) as f32];
        let Some((face, p)) = crate::interaction::picking::raycast(mesh, origin, direction) else {
            continue;
        };
        let f = mesh.faces[face];
        let (a, b, c) = mesh.triangle(&f)?;
        let facet = crate::mesh::cross(crate::mesh::sub(b, a), crate::mesh::sub(c, a));
        if facet[0] * cos + facet[1] * sin <= 0.0 {
            continue;
        }
        let p = p.map(f64::from);
        // A hit past the finger's axis came through the surface at theta, not onto it.
        if p[0] * cos + p[1] * sin <= 0.0 {
            continue;
        }
        let mut n = facet;
        if mesh.normals.len() == mesh.vertices.len() {
            // Barycentric weights of the hit in its triangle.
            let (v0, v1, v2) = (crate::mesh::sub(b, a), crate::mesh::sub(c, a), crate::mesh::sub(p, a));
            let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
            let (d00, d01, d11, d20, d21) = (dot(v0, v0), dot(v0, v1), dot(v1, v1), dot(v2, v0), dot(v2, v1));
            let denom = d00 * d11 - d01 * d01;
            if denom.abs() > 1e-18 {
                let wb = ((d11 * d20 - d01 * d21) / denom).clamp(0.0, 1.0);
                let wc = ((d00 * d21 - d01 * d20) / denom).clamp(0.0, 1.0);
                let wa = (1.0 - wb - wc).clamp(0.0, 1.0);
                let at = |i: u32| { let v = mesh.normals[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] };
                let (na, nb, nc) = (at(f[0]), at(f[1]), at(f[2]));
                let smooth: [f64; 3] = std::array::from_fn(|k| wa * na[k] + wb * nb[k] + wc * nc[k]);
                if crate::mesh::norm(smooth) > 1e-6 {
                    n = smooth;
                }
            }
        }
        let len = crate::mesh::norm(n);
        if len < 1e-12 {
            continue;
        }
        let mut n = n.map(|v| v / len);
        // Outward is toward the ray's origin.
        if n[0] * cos + n[1] * sin < 0.0 {
            n = n.map(|v| -v);
        }
        return Some((p, n));
    }
    None
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
/// Signs every bare face and edge reference in `op` in the seated frame of the part `e` built; the number signed.
pub fn sign_refs(op: &mut Operation, e: &Evaluated) -> usize {
    let part = |id: Id| e.components.iter().find(|c| c.id == id).and_then(|c| Some((c.brep()?, c.frame)));
    let edge = |r: &mut EdgeRef, id: Id| -> usize {
        let Some((body, frame)) = part(id).filter(|_| r.signature.is_none()) else { return 0 };
        r.signature = edge_signature(body, r.ordinal, &frame);
        usize::from(r.signature.is_some())
    };
    let face = |r: &mut FaceRef, id: Id| -> usize {
        let Some((body, frame)) = part(id).filter(|_| r.signature.is_none()) else { return 0 };
        r.signature = face_signature(body, r.ordinal, &frame);
        usize::from(r.signature.is_some())
    };
    let mut signed = 0;
    let mut anchors: Vec<&mut crate::sketch::FaceAnchor> = Vec::new();
    match op {
        Operation::Fillet { source, edges, .. } => signed += edges.iter_mut().map(|r| edge(r, *source)).sum::<usize>(),
        Operation::Chamfer { source, edges, base_face, .. } => signed += edges.iter_mut().map(|r| edge(r, *source)).sum::<usize>() + face(base_face, *source),
        Operation::Shell { source, open_faces, .. } => signed += open_faces.iter_mut().map(|r| face(r, *source)).sum::<usize>(),
        Operation::PressPull { source, face: r, .. } => signed += face(r, *source),
        Operation::Plane { base: PlaneBase::Face { feature, face: r }, .. } => signed += face(r, *feature),
        Operation::Sketch { sketch } => anchors.extend(sketch.plane.on_face.as_mut()),
        Operation::Extrude { sketch, .. } | Operation::Revolve { sketch, .. } | Operation::Sweep { sketch, .. } => {
            anchors.extend(sketch.sketch_mut().and_then(|s| s.plane.on_face.as_mut()));
        }
        Operation::Twist { sketch, path, .. } => {
            anchors.extend(sketch.sketch_mut().and_then(|s| s.plane.on_face.as_mut()));
            anchors.extend(path.plane.on_face.as_mut());
        }
        Operation::Loft { sections } => anchors.extend(sections.iter_mut().filter_map(|p| p.sketch_mut()?.plane.on_face.as_mut())),
        _ => {}
    }
    for anchor in anchors {
        signed += face(&mut anchor.face, anchor.feature);
    }
    // A stone's seat names a face of the part its builder stands on.
    if let Operation::Builder { on: Some(part), params, .. } = op {
        if let Ok(Some(mut seat)) = FaceSeat::of(params) {
            let n = face(&mut seat.face, *part);
            if n > 0 {
                seat.write(params);
            }
            signed += n;
        }
    }
    signed
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
            attach: Attach::Separate,
            stage: Stage::Cast,
            blend_mm: 0.0,
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
        for id in f.operation.consumes() {
            self.outputs.retain(|v| *v != id);
        }
        // A sketch or a work plane has no body to output.
        if f.operation.has_body() {
            self.outputs.push(f.id);
        }
        self.features.push(f);
        Ok(())
    }
    /// The enabled `Band` feature, if the document anchors its parts on the procedural ring.
    pub fn band(&self) -> Option<Id> {
        self.features.iter().find(|f| f.enabled && matches!(f.operation, Operation::Band)).map(|f| f.id)
    }
    /// Whether the document is the whole ring: no enabled `Band` feature, and a feature that builds a body.
    pub fn replaces_band(&self) -> bool {
        self.band().is_none() && self.features.iter().any(|f| f.enabled && f.operation.has_body())
    }
    /// How each output part meets the band, read off the document: a boolean against the band is the
    /// attachment it stands for, anything else its component's own. Reference parts are left out.
    pub fn attachments(&self) -> Vec<(Id, Attach, Stage)> {
        let band = self.band();
        self.outputs
            .iter()
            .filter_map(|id| self.features.iter().find(|f| f.id == *id && f.enabled))
            .filter(|f| !f.component.reference && f.operation.has_body() && !matches!(f.operation, Operation::Band))
            .map(|f| {
                let attach = match &f.operation {
                    Operation::Boolean { a, b, kind } if band.is_some_and(|band| band == *a || band == *b) => match kind {
                        Boolean::Union => Attach::Join,
                        Boolean::Subtract => Attach::Cut,
                        Boolean::Intersect => f.component.attach,
                    },
                    _ => f.component.attach,
                };
                (f.id, attach, f.component.stage)
            })
            .collect()
    }
    /// The parts a sand pattern leaves to the bench dropped from the outputs, named; nothing under lost wax.
    pub fn leave_bench_parts_out(&mut self, sand: bool) -> Vec<String> {
        if !sand {
            return Vec::new();
        }
        let mut names = Vec::new();
        self.outputs.retain(|id| {
            let bench = self.features.iter().any(|f| f.id == *id && f.enabled && !f.component.reference && f.component.stage == Stage::Bench);
            if bench {
                names.push(self.features.iter().find(|f| f.id == *id).map(|f| f.name.clone()).unwrap_or_default());
            }
            !bench
        });
        names
    }
}
/// What a feature evaluates to: a kernel body, or the mesh a builder made.
#[derive(Clone, Debug)]
pub enum Value {
    Brep(Body),
    Mesh(Arc<builders::Made>),
}
impl Value {
    /// The kernel body, when there is one.
    pub fn brep(&self) -> Option<&Body> {
        match self {
            Self::Brep(b) => Some(b),
            Self::Mesh(_) => None,
        }
    }
    /// What a builder made, when a builder made it.
    pub fn made(&self) -> Option<&Arc<builders::Made>> {
        match self {
            Self::Mesh(m) => Some(m),
            Self::Brep(_) => None,
        }
    }
    /// What a mesh value is, in a refusal's words: copies a pattern placed, or a builder's part.
    fn mesh_words(m: &builders::Made) -> &'static str {
        if m.key == pattern::PATTERN { "a mesh of placed copies" } else { "a mesh a builder made" }
    }
    /// Faces as the part names them: the body's faces, or the mesh's patches.
    pub fn faces(&self) -> usize {
        match self {
            Self::Brep(b) => b.faces.len(),
            Self::Mesh(m) => m.named.names.len(),
        }
    }
    /// Edges as the part names them: the body's edges, or the mesh's creases.
    pub fn edges(&self) -> usize {
        match self {
            Self::Brep(b) => b.edges.len(),
            Self::Mesh(m) => m.creases.len(),
        }
    }
    fn bytes(&self) -> usize {
        match self {
            Self::Brep(b) => body_bytes(b),
            Self::Mesh(m) => m.named.solid.v.len() * 24 + m.named.solid.f.len() * 16 + m.creases.iter().map(|l| l.len() * 24).sum::<usize>(),
        }
    }
}
/// One output part as evaluated; a part a builder made has an empty `body` and its value in `made`.
#[derive(Clone, Debug)]
pub struct EvaluatedComponent {
    pub id: Id,
    pub name: String,
    pub settings: Component,
    /// The kernel body; empty for a part a builder made.
    pub body: Body,
    pub mesh: Mesh,
    pub edges: Vec<Vec<[f64; 3]>>,
    pub trace: PartTrace,
    /// How the part meets the band: its component's own, or what a boolean against the band said.
    pub attach: Attach,
    pub stage: Stage,
    /// What a builder made, placed where the part stands; `None` for a kernel body.
    pub made: Option<Arc<builders::Made>>,
    /// The frame the build seated the body by, which references to its faces and edges are signed in.
    pub frame: brep::Placement,
}
impl EvaluatedComponent {
    /// The kernel body, unless a builder made the part as a mesh.
    pub fn brep(&self) -> Option<&Body> {
        self.made.is_none().then_some(&self.body)
    }
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
    /// What each face ordinal is called, for a part a builder made ("Claw 3", "Bearing"); empty for a kernel body.
    pub patches: Vec<String>,
}
impl PartTrace {
    /// The face ordinal a triangle came from, if it has one.
    pub fn face_of(&self, triangle: usize) -> Option<u32> {
        self.tri_face.get(triangle).copied().filter(|f| *f != u32::MAX)
    }
    /// The name of face ordinal `face`, when a builder named it.
    pub fn patch(&self, face: u32) -> Option<&str> {
        self.patches.get(face as usize).map(String::as_str)
    }
}
#[derive(Clone, Debug)]
pub struct Evaluated {
    pub components: Vec<EvaluatedComponent>,
    pub features: Vec<FeatureReport>,
    /// The `Band` feature the parts are anchored on; `None` when the document is the whole ring.
    pub band: Option<Id>,
    /// Every work plane built, in document order.
    pub planes: Vec<WorkPlane>,
}
impl Evaluated {
    /// The work plane feature `id` built.
    pub fn plane(&self, id: Id) -> Option<&WorkPlane> {
        self.planes.iter().find(|p| p.id == id)
    }
    /// The status the evaluation gave feature `id`.
    pub fn status_of(&self, id: Id) -> Option<&FeatureStatus> {
        self.features.iter().find(|r| r.id == id).map(|r| &r.status)
    }
    /// Every failed feature with its message, in document order; a skipped feature is not a failure.
    pub fn failures(&self) -> Vec<(Id, &str)> {
        self.features
            .iter()
            .filter_map(|r| match &r.status {
                FeatureStatus::Failed(message) => Some((r.id, message.as_str())),
                _ => None,
            })
            .collect()
    }
    /// The first failure as one line, `Feature #3 — Fillet: <message>`.
    pub fn first_error(&self) -> Option<String> {
        self.features.iter().find_map(|r| match &r.status {
            FeatureStatus::Failed(message) => Some(format!("Feature #{} — {}: {message}", r.id, r.name)),
            _ => None,
        })
    }
}
/// What became of a feature in an evaluation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum FeatureStatus {
    Ok,
    Suppressed,
    /// Refused or broken, with the message; it built nothing.
    Failed(String),
    /// Not attempted because a source failed, was skipped, or was suppressed and passed nothing through; names it.
    Skipped(String),
}
impl FeatureStatus {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
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
    /// Built, suppressed, failed with a message, or skipped behind a failure.
    pub status: FeatureStatus,
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
        Profile::Feature { feature } | Profile::Region { feature, .. } => sketches
            .get(feature)
            .ok_or_else(|| anyhow::anyhow!("Sketch feature #{feature} is unavailable or suppressed")),
    }
}
/// The plane a sketch lies on: its own, or its workplane read in the frame of the planar face of
/// an earlier feature it is anchored to, found again by signature when the source has changed.
fn plane_of(
    sketch: &Sketch,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    notes: &mut Vec<String>,
) -> Result<cadkernel::space::Plane> {
    let Some(anchor) = &sketch.plane.on_face else {
        return sketch.plane.plane();
    };
    let body = match values.get(&anchor.feature) {
        Some(Value::Brep(body)) => body,
        Some(Value::Mesh(m)) if m.key == pattern::PATTERN => anyhow::bail!(
            "Sketch face: feature #{} is {}; sketch on a kernel part's planar face",
            anchor.feature,
            Value::mesh_words(m)
        ),
        Some(Value::Mesh(m)) => anyhow::bail!(
            "Sketch face: feature #{} is a {} a builder made, a mesh; sketch on a kernel part's planar face",
            anchor.feature,
            builders::label(&m.key).to_lowercase()
        ),
        // A work plane holds a frame and no body.
        None => match frames.get(&anchor.feature) {
            Some(plane) => return pattern::on_plane(sketch, plane),
            None => anyhow::bail!("Sketch face: feature #{} is unavailable or suppressed", anchor.feature),
        },
    };
    let frame = frames.get(&anchor.feature).copied().unwrap_or(brep::Placement::IDENTITY);
    sketch_plane(sketch, body, &frame, notes)
}
/// The world plane `sketch` lies on: its own, or, when it is anchored to a face, that face of
/// `body` (the anchored feature's body as built, seated by `frame`) found again by signature, with
/// any finding written to `notes`. What a canvas draws a face sketch in, and what the evaluation sweeps.
pub fn sketch_plane(
    sketch: &Sketch,
    body: &Body,
    frame: &brep::Placement,
    notes: &mut Vec<String>,
) -> Result<cadkernel::space::Plane> {
    let Some(anchor) = &sketch.plane.on_face else {
        return sketch.plane.plane();
    };
    let key = resolve_face(body, &anchor.face, frame, notes).context("Sketch face")?;
    let kind = SurfaceKind::of(body.faces.get(key).and_then(|f| body.surfaces.get(f.surface)));
    ensure!(
        kind == SurfaceKind::Plane,
        "Sketch face {} of feature #{}: sketch planes need a planar face: {}",
        anchor.face.ordinal,
        anchor.feature,
        format!("{kind:?}").to_lowercase()
    );
    let face = brep::planar_face_profile(body, key)
        .ok_or_else(|| anyhow::anyhow!("Sketch face {} of feature #{} has no boundary the kernel can read", anchor.face.ordinal, anchor.feature))?;
    sketch.plane.on_frame(&crate::sketch::FaceFrame::of(&face).context("Sketch face")?)
}
/// A sketch validated and, when it lies on a face, laid onto that face's plane in world millimetres.
fn laid(
    sketch: &Sketch,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    notes: &mut Vec<String>,
) -> Result<Sketch> {
    sketch.validate()?;
    if sketch.plane.on_face.is_none() {
        return Ok(sketch.clone());
    }
    let plane = plane_of(sketch, values, frames, notes)?;
    let mut out = sketch.clone();
    out.plane = crate::sketch::Workplane { origin: plane.origin, x: plane.x_axis, y: plane.y_axis, on_face: None };
    Ok(out)
}
/// The regions a profile sweeps: every one a `Sketch` feature holds, the one it names, or the
/// single one of an inline sketch; a region found again only by its point says so in `notes`.
fn regions_of(p: &Profile, sketch: &Sketch, notes: &mut Vec<String>) -> Result<Vec<crate::sketch::Region>> {
    match p {
        Profile::Feature { .. } => sketch.profile_regions(),
        Profile::Region { feature, region } => {
            let (found, note) = sketch.region_of(region).with_context(|| format!("Sketch #{feature}"))?;
            notes.extend(note.map(|n| format!("Sketch #{feature}: {n}")));
            Ok(vec![found])
        }
        Profile::Inline(_) => Ok(vec![sketch.profile_region()?]),
    }
}
/// The one closed loop a twisted sweep or a loft section takes: the named region's rim, refused with holes in it, else the profile's own loop.
fn region_loop(p: &Profile, sketch: &Sketch, what: &str, notes: &mut Vec<String>) -> Result<Vec<cadkernel::geom2d::Curve>> {
    let Profile::Region { feature, .. } = p else { return sketch.profile_curves() };
    let region = regions_of(p, sketch, notes)?.into_iter().next().context("A region profile names no region")?;
    let holes = region.holes.len();
    ensure!(holes == 0, "{what}: the region of sketch #{feature} has {holes} hole{}; it takes one closed loop", if holes == 1 { "" } else { "s" });
    Ok(region.outer)
}
/// A value as a named csg solid: a builder's own, or a kernel body tessellated at the export chord, a patch per face.
fn named_of(value: &Value) -> Result<crate::setting::Named> {
    match value {
        Value::Mesh(m) => Ok(m.named.clone()),
        Value::Brep(body) => {
            let (mesh, trace) = tessellate_traced(body, 0.015)?;
            let faces = trace.face_kind.len() as u32;
            let patch = trace.tri_face.iter().map(|f| if *f == u32::MAX { faces } else { *f }).collect();
            let mut names: Vec<String> = (0..faces).map(|k| format!("Face {k}")).collect();
            names.push("Seam".into());
            Ok(crate::setting::Named { solid: crate::csg::Solid { v: trace.positions, f: mesh.faces }, patch, names })
        }
    }
}
/// The placement `outer` after `inner`: `inner` first, then `outer`.
fn compose(outer: &brep::Placement, inner: &brep::Placement) -> brep::Placement {
    let turn = |v: [f64; 3]| -> [f64; 3] { std::array::from_fn(|k| outer.x_axis[k] * v[0] + outer.y_axis[k] * v[1] + outer.z_axis[k] * v[2]) };
    let o = turn(inner.origin);
    brep::Placement { x_axis: turn(inner.x_axis), y_axis: turn(inner.y_axis), z_axis: turn(inner.z_axis), origin: std::array::from_fn(|k| outer.origin[k] + o[k]) }
}
fn body_for(
    op: &Operation,
    values: &BTreeMap<Id, Value>,
    sketches: &BTreeMap<Id, Sketch>,
    frames: &BTreeMap<Id, brep::Placement>,
    params: BuildParams,
    who: &dyn Fn(Id) -> String,
    notes: &mut Vec<String>,
) -> Result<Value> {
    let value = |id: &Id| values.get(id).ok_or_else(|| anyhow::anyhow!("Source feature #{id} is unavailable or suppressed"));
    // A kernel operation reads kernel bodies; a part a builder made is a mesh, refused by name.
    let source = |id: &Id| match value(id)? {
        Value::Brep(body) => Ok(body),
        Value::Mesh(m) => Err(anyhow::anyhow!("{} works on kernel bodies, and {} is {}", op.label(), who(*id), Value::mesh_words(m))),
    };
    // References are signed in the source part's own frame, so the placement its body was seated by is taken back off.
    let frame_of = |id: &Id| -> brep::Placement { frames.get(id).cloned().unwrap_or(brep::Placement::IDENTITY) };
    fn edges(body: &Body, frame: &brep::Placement, refs: &[EdgeRef], notes: &mut Vec<String>) -> Result<Vec<brep::EdgeKey>> {
        ensure!(!refs.is_empty(), "Select at least one edge");
        refs.iter().map(|r| resolve_edge(body, r, frame, notes)).collect()
    }
    let body: Result<Body> = match op {
        // The band is the anchor the parts stand on, never a kernel body: it is joined in the mesh stage.
        Operation::Band => anyhow::bail!("The procedural shank is not a body; parts are joined to it after the build"),
        Operation::Builder { key, .. } => anyhow::bail!("{} is built round its stone, not by the kernel", builders::label(key)),
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
            sketch: from,
            height_mm,
            draft_deg,
        } => {
            let sketch = profile(from, sketches)?;
            let p = plane_of(sketch, values, frames, notes)?;
            let h = positive(*height_mm, "Height")?;
            ensure!(
                draft_deg.is_finite() && draft_deg.abs() < 80.0,
                "Draft must be below 80 degrees"
            );
            crate::sketch::solid::extrude(
                p,
                &regions_of(from, sketch, notes)?,
                p.normal().unwrap().map(|v| v * h),
                draft_deg.to_radians(),
            )
        }
        Operation::Revolve {
            sketch: from,
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
            let sketch = profile(from, sketches)?;
            crate::sketch::solid::revolve(
                plane_of(sketch, values, frames, notes)?,
                &regions_of(from, sketch, notes)?,
                *pivot,
                *axis,
                degrees.to_radians(),
            )
        }
        Operation::Sweep { sketch: from, path } => {
            ensure!(
                path.len() >= 2 && path.len() <= 128,
                "Sweep needs 2–128 path stations"
            );
            coords(path)?;
            let sketch = profile(from, sketches)?;
            let plane = plane_of(sketch, values, frames, notes)?;
            // One region of several sweeps its own loops, holes and all; any other profile is its one closed loop.
            let wires = match from {
                Profile::Region { .. } => regions_of(from, sketch, notes)?.into_iter().flat_map(|r| r.loops()).collect(),
                _ => vec![sketch.profile_curves()?],
            };
            maybe(
                brep::sweep_path(
                    plane,
                    &wires,
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
            sketch: from,
            path,
            degrees,
            end_scale,
        } => {
            ensure!(
                degrees.is_finite() && degrees.abs() <= 3600.0,
                "Twist exceeds ten turns"
            );
            positive(*end_scale, "End scale")?;
            let sketch = profile(from, sketches)?;
            let plane = plane_of(sketch, values, frames, notes)?;
            let outline = region_loop(from, sketch, "Twisted sweep", notes)?;
            maybe(
                brep::sweep_along_deformed(
                    plane,
                    &outline,
                    plane_of(path, values, frames, notes)?,
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
                    let plane = plane_of(s, values, frames, notes)?;
                    Ok((plane, region_loop(p, s, "Loft", notes)?))
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
            let (va, vb) = (value(a)?, value(b)?);
            if va.made().is_some() || vb.made().is_some() {
                // A mesh operand goes through csg, every face keeping the patch it came from.
                let op = match kind {
                    Boolean::Union => crate::csg::Op::Union,
                    Boolean::Subtract => crate::csg::Op::Subtract,
                    Boolean::Intersect => crate::csg::Op::Intersect,
                };
                let named = builders::combined(&named_of(va)?, &named_of(vb)?, op).with_context(|| format!("{} of {} and {}", op_label(*kind), who(*a), who(*b)))?;
                let gem = va.made().and_then(|m| m.gem).or_else(|| vb.made().and_then(|m| m.gem));
                return Ok(Value::Mesh(Arc::new(builders::Made::of(op_label(*kind), named, gem)?)));
            }
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
                &edges(b, &frame_of(id), refs, notes)?,
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
            let frame = frame_of(id);
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
            let frame = frame_of(id);
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
        } => match value(id)? {
            Value::Mesh(m) => return Ok(Value::Mesh(Arc::new(m.placed(&rotate_place(*translation, *rotation_deg)?)))),
            Value::Brep(body) => maybe(brep::transform(body, &rotate_place(*translation, *rotation_deg)?), "Placement"),
        },
        Operation::PressPull { source: id, face, distance_mm } => {
            let b = source(id)?;
            pattern::press_pull(b, face, *distance_mm, &frame_of(id), notes)
        }
        Operation::Pattern { .. } => anyhow::bail!("A pattern is built from its source's copies"),
        Operation::Plane { .. } => anyhow::bail!("A work plane has no body of its own"),
    };
    body.map(Value::Brep)
}
fn op_label(kind: Boolean) -> &'static str {
    match kind {
        Boolean::Union => "Union",
        Boolean::Subtract => "Subtract",
        Boolean::Intersect => "Intersect",
    }
}

/// Largest operand the analytic boolean accepts. Measured by `examples/kernel_probe.rs` on a
/// faceted torus with a cylinder through its tube: 256 faces 0.3 s, 576 faces 3.8 s, 1024 faces
/// 14 s, every one refused as `CutRefused`; 2304 faces and up never return. A faceted band is 24k.
pub const MAX_ANALYTIC_BOOLEAN_FACES: usize = 500;
/// The error text of an evaluation stopped through its `BuildCtx`.
pub const CANCELLED: &str = "CAD evaluation cancelled";
/// Bodies and tessellations a default [`Cache`] holds before the least recently used is dropped.
pub const CACHE_ENTRIES: usize = 512;
/// Estimated bytes a default [`Cache`] holds before the least recently used entry is dropped.
pub const CACHE_BYTES: usize = 256 << 20;

/// Cooperative stop for a running evaluation, polled between features and tessellations, and the
/// built surface a ring placement drops onto.
pub struct BuildCtx<'a> {
    pub cancel: &'a std::sync::atomic::AtomicBool,
    /// The band as built, for `Placement::Ring` to seat parts on; `None` falls back to the reference crest.
    pub surface: Option<&'a Mesh>,
}
impl<'a> BuildCtx<'a> {
    pub fn new(cancel: &'a std::sync::atomic::AtomicBool) -> Self {
        Self { cancel, surface: None }
    }
    pub fn with_surface(self, surface: &'a Mesh) -> Self {
        Self { surface: Some(surface), ..self }
    }
    fn check(&self) -> Result<()> {
        ensure!(!self.cancel.load(std::sync::atomic::Ordering::Relaxed), CANCELLED);
        Ok(())
    }
}

/// The cache an evaluation reads and fills, and the epoch of the surface it seats ring placements on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Memo<'a> {
    /// Bodies and tessellations remembered by recipe signature; `None` builds everything.
    pub cache: Option<&'a Mutex<Cache>>,
    /// Identity of the surface ring placements drop onto, hashed into their signatures.
    pub surface_epoch: u64,
}
impl<'a> Memo<'a> {
    pub fn new(cache: &'a Mutex<Cache>) -> Self {
        Self { cache: Some(cache), surface_epoch: 0 }
    }
    pub fn with_epoch(self, surface_epoch: u64) -> Self {
        Self { surface_epoch, ..self }
    }
}

/// One feature as built: its placed value, seat frame, band attachment and reference notes.
#[derive(Clone, Debug)]
struct Built {
    value: Value,
    frame: Option<brep::Placement>,
    attach: Option<Attach>,
    notes: Vec<String>,
}
/// One body tessellated at one chord.
#[derive(Debug)]
struct Tessellated {
    mesh: Mesh,
    trace: PartTrace,
    edges: Vec<Vec<[f64; 3]>>,
}
/// The tessellation bucket a builder's mesh is kept under: it is its own at every chord.
const MESH_BUCKET: u8 = 2;
impl Tessellated {
    /// A builder's part as a component's mesh: its own faces, each naming its patch, and its creases for edges.
    fn of_made(made: &builders::Made) -> Self {
        let s = made.solid();
        let mut mesh = Mesh { vertices: s.v.iter().map(|p| Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), faces: s.f.clone(), ..Mesh::default() };
        mesh.normals = normals(&mesh);
        let trace = PartTrace {
            tri_face: made.named.patch.clone(),
            positions: s.v.clone(),
            face_kind: made.kinds.clone(),
            vertices: made.corners(),
            patches: made.named.names.clone(),
        };
        Self { mesh, trace, edges: made.creases.clone() }
    }
    /// Heap the arrays reserve.
    fn bytes(&self) -> usize {
        fn heap<T>(v: &Vec<T>) -> usize {
            v.capacity() * std::mem::size_of::<T>()
        }
        let (m, t) = (&self.mesh, &self.trace);
        heap(&m.vertices)
            + heap(&m.normals)
            + heap(&m.faces)
            + heap(&m.corner_normals)
            + heap(&m.origin)
            + heap(&t.tri_face)
            + heap(&t.positions)
            + heap(&t.face_kind)
            + heap(&t.vertices)
            + heap(&t.patches)
            + t.patches.iter().map(String::capacity).sum::<usize>()
            + heap(&self.edges)
            + self.edges.iter().map(heap).sum::<usize>()
    }
}
/// A body's kernel arenas, estimated high per face, edge and vertex.
fn body_bytes(body: &Body) -> usize {
    body.faces.len() * 1024 + body.edges.len() * 256 + body.vertices.len() * 64
}
/// A value tessellated at `chord`, remembered under its recipe signature.
fn tessellated(value: &Value, sig: u64, bucket: u8, chord: f64, memo: Memo) -> Result<Arc<Tessellated>> {
    let key = (sig, if value.made().is_some() { MESH_BUCKET } else { bucket });
    if let Some(t) = memo.cache.and_then(|c| c.lock().ok()?.mesh(key)) {
        return Ok(t);
    }
    let t = Arc::new(match value {
        Value::Mesh(made) => Tessellated::of_made(made),
        Value::Brep(body) => {
            let (mesh, trace) = tessellate_traced(body, chord)?;
            let edges = if body.edges.len() <= 512 {
                body.edges.iter().map(|(key, _)| brep::edge_points(body, key, 0.02).unwrap_or_default()).collect()
            } else {
                Vec::new()
            };
            Tessellated { mesh, trace, edges }
        }
    });
    if let Some(Ok(mut c)) = memo.cache.map(Mutex::lock) {
        c.keep_mesh(key, t.clone());
    }
    Ok(t)
}
/// The document, cache, recipe signatures and chord one feature's build reads beyond its sources.
struct Scope<'a> {
    doc: &'a Document,
    memo: Memo<'a>,
    sigs: &'a BTreeMap<Id, u64>,
    chord: f64,
    bucket: u8,
}
/// A remembered body or tessellation.
#[derive(Clone, Debug)]
enum Slot {
    Body(Arc<Result<Built, String>>),
    Mesh(Arc<Tessellated>),
}
/// A body by recipe signature, or a tessellation by body signature and chord bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key {
    Body(u64),
    Mesh(u64, u8),
}
/// Bodies (failures included) and tessellations by recipe signature, LRU-bounded by entries and estimated bytes.
#[derive(Debug)]
pub struct Cache {
    slots: HashMap<Key, (Slot, usize)>,
    /// Least recently used first.
    order: VecDeque<Key>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    hits: u64,
    misses: u64,
}
impl Default for Cache {
    fn default() -> Self {
        Self::with_limits(CACHE_ENTRIES, CACHE_BYTES)
    }
}
impl Cache {
    /// A cache of at most `entries` bodies and tessellations and about `bytes` of them.
    pub fn with_limits(entries: usize, bytes: usize) -> Self {
        Self { slots: HashMap::new(), order: VecDeque::new(), bytes: 0, max_entries: entries, max_bytes: bytes, hits: 0, misses: 0 }
    }
    /// Lookups answered from the cache, bodies and tessellations together.
    pub fn hits(&self) -> u64 {
        self.hits
    }
    /// Lookups that had to build.
    pub fn misses(&self) -> u64 {
        self.misses
    }
    /// Bodies and tessellations held.
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    /// Estimated bytes held.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    /// Drop every entry and zero the counters; the limits stay.
    pub fn clear(&mut self) {
        *self = Self::with_limits(self.max_entries, self.max_bytes);
    }
    fn get(&mut self, key: Key) -> Option<Slot> {
        let Some((slot, _)) = self.slots.get(&key) else {
            self.misses += 1;
            return None;
        };
        let slot = slot.clone();
        self.hits += 1;
        if let Some(i) = self.order.iter().position(|k| *k == key) {
            self.order.remove(i);
            self.order.push_back(key);
        }
        Some(slot)
    }
    fn keep(&mut self, key: Key, slot: Slot, bytes: usize) {
        if let Some((_, old)) = self.slots.remove(&key) {
            self.bytes -= old;
            self.order.retain(|k| *k != key);
        }
        // An entry over the whole byte bound is not kept, and evicts nothing.
        if bytes > self.max_bytes || self.max_entries == 0 {
            return;
        }
        self.slots.insert(key, (slot, bytes));
        self.bytes += bytes;
        self.order.push_back(key);
        while self.order.len() > self.max_entries || self.bytes > self.max_bytes {
            let Some(old) = self.order.pop_front() else { break };
            if let Some((_, b)) = self.slots.remove(&old) {
                self.bytes -= b;
            }
        }
    }
    fn body(&mut self, sig: u64) -> Option<Arc<Result<Built, String>>> {
        match self.get(Key::Body(sig))? {
            Slot::Body(built) => Some(built),
            Slot::Mesh(_) => None,
        }
    }
    fn keep_body(&mut self, sig: u64, built: Arc<Result<Built, String>>) {
        let bytes = 64 + match &*built {
            Ok(b) => b.value.bytes() + b.notes.iter().map(String::len).sum::<usize>(),
            Err(message) => message.len(),
        };
        self.keep(Key::Body(sig), Slot::Body(built), bytes);
    }
    fn mesh(&mut self, key: (u64, u8)) -> Option<Arc<Tessellated>> {
        match self.get(Key::Mesh(key.0, key.1))? {
            Slot::Mesh(t) => Some(t),
            Slot::Body(_) => None,
        }
    }
    fn keep_mesh(&mut self, key: (u64, u8), tessellated: Arc<Tessellated>) {
        let bytes = 64 + tessellated.bytes();
        self.keep(Key::Mesh(key.0, key.1), Slot::Mesh(tessellated), bytes);
    }
}

/// A hash of every vertex, normal and face a ring placement's ray drop reads.
pub fn surface_epoch(mesh: &Mesh) -> u64 {
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    let mut fold = |word: u64| x = (x ^ word).wrapping_mul(0x0000_0100_0000_01b3);
    fold(mesh.vertices.len() as u64);
    fold(mesh.normals.len() as u64);
    fold(mesh.faces.len() as u64);
    for v in mesh.vertices.iter().chain(&mesh.normals) {
        fold(u64::from(v.0.to_bits()) | u64::from(v.1.to_bits()) << 32);
        fold(u64::from(v.2.to_bits()));
    }
    for f in &mesh.faces {
        fold(u64::from(f[0]) | u64::from(f[1]) << 32);
        fold(u64::from(f[2]));
    }
    x
}

/// Per feature, a hash of what builds its body: operation, enabled, placement, sources' hashes, and the surface, radii or steps it reads.
fn signatures(doc: &Document, design: &RingDesign, params: BuildParams, surface_epoch: u64) -> BTreeMap<Id, u64> {
    let mut sigs = BTreeMap::new();
    for f in &doc.features {
        let mut h = std::hash::DefaultHasher::new();
        serde_json::to_vec(&(f.enabled, &f.operation, &f.component.placement)).unwrap_or_default().hash(&mut h);
        for s in f.operation.sources() {
            match sigs.get(&s) {
                Some(v) => (1u8, *v).hash(&mut h),
                None => (0u8, s).hash(&mut h),
            }
        }
        // Builders, patterns and tangent planes read the surface and the bore below it.
        let reads_surface = matches!(
            f.operation,
            Operation::Builder { .. } | Operation::Pattern { .. } | Operation::Plane { base: PlaneBase::Tangent { .. }, .. }
        );
        if f.component.placement != Placement::Free || reads_surface {
            surface_epoch.hash(&mut h);
            design.inner_radius_mm().to_bits().hash(&mut h);
            design.profile.thickness_mm.to_bits().hash(&mut h);
        }
        if matches!(f.operation, Operation::TwistedRing { .. }) {
            (params.theta_steps, params.profile_steps).hash(&mut h);
        }
        // A pattern's copies are its source's tessellation at this build's chord.
        if matches!(f.operation, Operation::Pattern { .. }) {
            (params.theta_steps >= 512).hash(&mut h);
        }
        sigs.insert(f.id, h.finish());
    }
    sigs
}

/// The first source that failed, was skipped, or was suppressed without passing a body through, named.
fn skipped_by(op: &Operation, status: &BTreeMap<Id, FeatureStatus>, doc: &Document, passed: impl Fn(Id) -> bool) -> Option<String> {
    for s in op.sources() {
        let why = match status.get(&s) {
            Some(FeatureStatus::Failed(_)) => "failed",
            Some(FeatureStatus::Skipped(_)) => "was skipped",
            Some(FeatureStatus::Suppressed) if !passed(s) => "was suppressed",
            _ => continue,
        };
        let name = doc.features.iter().find(|f| f.id == s).map(|f| f.name.as_str()).unwrap_or_default();
        return Some(format!("source #{s} {name} {why}"));
    }
    None
}

/// One enabled feature's body, validated and seated; a boolean against the band is its other operand with an attachment.
fn build_feature(
    f: &Feature,
    design: &RingDesign,
    ctx: &BuildCtx,
    params: BuildParams,
    band: Option<Id>,
    values: &BTreeMap<Id, Value>,
    sketches: &BTreeMap<Id, Sketch>,
    frames: &BTreeMap<Id, brep::Placement>,
    who: &dyn Fn(Id) -> String,
    scope: &Scope,
) -> Result<Built> {
    if let Operation::Builder { key, on, params: settings } = &f.operation {
        return build_made(f, key, *on, settings, design, ctx, values, frames, who, scope.doc);
    }
    if let Operation::Pattern { source, kind } = &f.operation {
        return pattern::build(f, *source, kind, design, ctx, values, frames, who, scope);
    }
    let mut notes = Vec::new();
    let mut frame = None;
    let mut attach = None;
    let against_band = match &f.operation {
        Operation::Boolean { a, b, kind } if band.is_some_and(|id| id == *a || id == *b) => Some((*a, *b, *kind)),
        _ => None,
    };
    // A moved part's frame moves with it.
    if let Operation::Transform { source, translation, rotation_deg } = &f.operation {
        if let Some(inner) = frames.get(source) {
            frame = Some(compose(&rotate_place(*translation, *rotation_deg)?, inner));
        }
    }
    // A part reshaped where it stands keeps the frame its source was seated by.
    if let Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } | Operation::PressPull { source, .. } =
        &f.operation
    {
        frame = frames.get(source).copied();
    }
    let mut value = if let Some((a, b, kind)) = against_band {
        let band_id = band.unwrap_or_default();
        ensure!(a != b, "Boolean sources must be different");
        let other = if a == band_id { b } else { a };
        attach = Some(match kind {
            Boolean::Union => Attach::Join,
            Boolean::Subtract => {
                ensure!(
                    a == band_id,
                    "subtracting the procedural shank from a part is not supported; subtract the part from the shank to cut it"
                );
                Attach::Cut
            }
            Boolean::Intersect => anyhow::bail!(
                "intersecting with the procedural shank is not supported; Union joins a part to it and Subtract cuts one from it"
            ),
        });
        frame = frames.get(&other).cloned();
        values
            .get(&other)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("source feature #{other} is unavailable or suppressed"))?
    } else {
        body_for(&f.operation, values, sketches, frames, params, who, &mut notes)?
    };
    if let Value::Brep(body) = &value {
        let faults = body.validate();
        ensure!(faults.is_empty(), "generated invalid topology: {faults:?}");
    }
    if f.component.placement != Placement::Free {
        let seat = f.component.placement.frame_on(design, ctx.surface)?;
        (value, frame) = match value {
            Value::Brep(body) => (Value::Brep(maybe(brep::transform(&body, &seat), "Ring placement")?), Some(seat)),
            // A mesh's own frame, a stone's, moves with it.
            Value::Mesh(m) => (Value::Mesh(Arc::new(m.placed(&seat))), Some(frame.map_or(seat, |inner| compose(&seat, &inner)))),
        };
    }
    Ok(Built { value, frame, attach, notes })
}

/// The seat frame turned a quarter about its normal, so a stone's length runs round the ring at `spin` zero.
fn stone_frame(seat: &brep::Placement) -> brep::Placement {
    brep::Placement { x_axis: seat.y_axis, y_axis: seat.x_axis.map(|v| -v), z_axis: seat.z_axis, origin: seat.origin }
}

/// How far over a stone's girdle plane a floor probe starts down, mm.
const PROBE_ABOVE_MM: f64 = 8.0;
/// How far below the metal under a stone's centre a floor probe still reads metal, mm.
const PROBE_BELOW_MM: f64 = 6.0;

/// The surface round a stone in its own frame, bucketed over its girdle plane for straight-down probes above `below`.
struct Ground {
    faces: Vec<[[f64; 3]; 3]>,
    lo: [f64; 2],
    cell: f64,
    cols: usize,
    rows: usize,
    buckets: Vec<Vec<u32>>,
    below: f64,
}
impl Ground {
    fn new(mesh: &Mesh, frame: &brep::Placement, radius: f64, below: f64) -> Self {
        let local = |p: [f64; 3]| -> [f64; 3] {
            let d: [f64; 3] = std::array::from_fn(|k| p[k] - frame.origin[k]);
            let along = |axis: [f64; 3]| (0..3).map(|k| d[k] * axis[k]).sum::<f64>();
            [along(frame.x_axis), along(frame.y_axis), along(frame.z_axis)]
        };
        let faces: Vec<[[f64; 3]; 3]> = mesh
            .faces
            .iter()
            .filter_map(|f| {
                let (a, b, c) = mesh.triangle(f)?;
                let t = [local(a), local(b), local(c)];
                let near = t.iter().any(|p| p[0].hypot(p[1]) < radius) && t.iter().any(|p| p[2] > below);
                near.then_some(t)
            })
            .collect();
        let cell = 0.25;
        let lo = [-radius - cell, -radius - cell];
        let cols = ((2.0 * radius) / cell).ceil() as usize + 3;
        let mut buckets = vec![Vec::new(); cols * cols];
        for (i, t) in faces.iter().enumerate() {
            let (x0, x1) = (t.iter().map(|p| p[0]).fold(f64::MAX, f64::min), t.iter().map(|p| p[0]).fold(f64::MIN, f64::max));
            let (y0, y1) = (t.iter().map(|p| p[1]).fold(f64::MAX, f64::min), t.iter().map(|p| p[1]).fold(f64::MIN, f64::max));
            let span = |v0: f64, v1: f64, o: f64| (((v0 - o) / cell).floor().max(0.0) as usize, (((v1 - o) / cell).floor().max(0.0) as usize).min(cols - 1));
            let ((c0, c1), (r0, r1)) = (span(x0, x1, lo[0]), span(y0, y1, lo[1]));
            for r in r0..=r1.max(r0) {
                for c in c0..=c1.max(c0) {
                    if let Some(b) = buckets.get_mut(r * cols + c) {
                        b.push(i as u32);
                    }
                }
            }
        }
        Self { faces, lo, cell, cols, rows: cols, buckets, below }
    }

    /// The highest metal straight under point `p` of the girdle plane, in the stone's frame.
    fn floor(&self, p: [f64; 2]) -> Option<f64> {
        let (c, r) = (((p[0] - self.lo[0]) / self.cell).floor(), ((p[1] - self.lo[1]) / self.cell).floor());
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        let mut best: Option<f64> = None;
        for &i in &self.buckets[r as usize * self.cols + c as usize] {
            let [a, b, t] = self.faces[i as usize];
            let det = (b[0] - a[0]) * (t[1] - a[1]) - (t[0] - a[0]) * (b[1] - a[1]);
            if det.abs() < 1e-18 {
                continue;
            }
            let u = ((p[0] - a[0]) * (t[1] - a[1]) - (t[0] - a[0]) * (p[1] - a[1])) / det;
            let v = ((b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1])) / det;
            const SLACK: f64 = 1e-9;
            if u < -SLACK || v < -SLACK || u + v > 1.0 + SLACK {
                continue;
            }
            let z = a[2] + (b[2] - a[2]) * u + (t[2] - a[2]) * v;
            if z <= PROBE_ABOVE_MM && z > self.below && best.is_none_or(|h| z > h) {
                best = Some(z);
            }
        }
        best
    }
}

/// Where the metal is under a stone in `frame`, and how far a pilot runs from its girdle past the bore.
fn seat_at(frame: &brep::Placement, placement: &Placement, design: &RingDesign, surface: Option<&Mesh>) -> builders::Seat {
    let dropped = surface.and_then(|mesh| Ground::new(mesh, frame, 1.0, -PROBE_ABOVE_MM).floor([0.0, 0.0]));
    let surface_z = dropped.unwrap_or(match placement {
        Placement::Ring { height_mm, .. } => -height_mm,
        Placement::Free => 0.0,
    });
    seat_over(frame, surface_z, design)
}

/// A stone in `frame` over metal `surface_z` down its axis, and how far a pilot runs from its girdle past the bore.
fn seat_over(frame: &brep::Placement, surface_z: f64, design: &RingDesign) -> builders::Seat {
    let (o, z) = (frame.origin, frame.z_axis);
    let s: [f64; 3] = std::array::from_fn(|k| o[k] + z[k] * surface_z);
    let r = s[0].hypot(s[1]);
    let outward = if r > 1e-9 { (z[0] * s[0] + z[1] * s[1]) / r } else { 0.0 };
    let through_mm = (outward > 0.75).then(|| -surface_z + (r - design.inner_radius_mm()).max(0.0) / outward + 0.6);
    builders::Seat { surface_z, through_mm }
}

/// The part stone `stone` sits on a face of, tessellated where it stands.
fn stood_on(doc: &Document, stone: Id, values: &BTreeMap<Id, Value>) -> Option<Mesh> {
    let Operation::Builder { on: Some(part), .. } = &doc.feature(stone)?.operation else { return None };
    tessellate(values.get(part)?.brep()?, 0.04).ok()
}

/// A builder's part: a stone seated by its own placement or on a part's face, or a setting made in the frame of the stone it stands on.
#[allow(clippy::too_many_arguments)]
fn build_made(
    f: &Feature,
    key: &str,
    on: Option<Id>,
    settings: &serde_json::Value,
    design: &RingDesign,
    ctx: &BuildCtx,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    who: &dyn Fn(Id) -> String,
    doc: &Document,
) -> Result<Built> {
    let spec = builders::spec(key).ok_or_else(|| {
        anyhow::anyhow!("No builder called {key}; choose {}", builders::SPECS.iter().map(|s| s.key).collect::<Vec<_>>().join(", "))
    })?;
    let mut notes = Vec::new();
    let (gem, frame, seat) = if spec.on_stone {
        let stone = on.ok_or_else(|| anyhow::anyhow!("{} is built round a stone; choose the stone it stands on", spec.label))?;
        ensure!(f.component.placement == Placement::Free, "{} stands in its stone's frame; move the stone to move it", spec.label);
        let made = match values.get(&stone) {
            Some(Value::Mesh(m)) if m.key == builders::STONE => m,
            Some(_) => anyhow::bail!("{} is not a stone; a {} is built round a stone part", who(stone), spec.label.to_lowercase()),
            None => anyhow::bail!("source feature #{stone} is unavailable or suppressed"),
        };
        let gem = made.gem.ok_or_else(|| anyhow::anyhow!("{} carries no gem", who(stone)))?;
        (gem, frames.get(&stone).copied().unwrap_or(brep::Placement::IDENTITY), made.seat.unwrap_or_default())
    } else if let Some(part) = on {
        let gem = builders::gem_of(settings)?;
        ensure!(f.component.placement == Placement::Free, "A stone on a face of {} stands where that face is; its placement stays free", who(part));
        let seat = FaceSeat::of(settings)?.ok_or_else(|| anyhow::anyhow!("A stone standing on {} names no face of it to sit on", who(part)))?;
        let frame = face_seat_frame(&seat, part, values, frames, who, &mut notes)?;
        (gem, frame, seat_over(&frame, -seat.height_mm, design))
    } else {
        let gem = builders::gem_of(settings)?;
        let frame = match &f.component.placement {
            Placement::Free => brep::Placement::IDENTITY,
            p => stone_frame(&p.frame_on(design, ctx.surface)?),
        };
        (gem, frame, seat_at(&frame, &f.component.placement, design, ctx.surface))
    };
    // Claws and a bezel's wall reach the metal wherever they stand: the band, and the part their stone sits on.
    let (reach, below) = (gem.l_mm.max(gem.w_mm) * 0.5 + 3.0, seat.surface_z - PROBE_BELOW_MM);
    let mut grounds = Vec::new();
    if spec.on_stone {
        grounds.extend(ctx.surface.map(|mesh| Ground::new(mesh, &frame, reach, below)));
        grounds.extend(on.and_then(|stone| stood_on(doc, stone, values)).map(|mesh| Ground::new(&mesh, &frame, reach, below)));
    }
    let probe = |p: [f64; 2]| grounds.iter().filter_map(|g| g.floor(p)).reduce(f64::max);
    let floor: Option<crate::setting::Floor> = grounds.iter().any(|g| !g.faces.is_empty()).then_some(&probe as crate::setting::Floor);
    let made = builders::build(key, gem, settings, seat, floor)?;
    Ok(Built { value: Value::Mesh(Arc::new(made.placed(&frame))), frame: Some(frame), attach: None, notes })
}

pub fn evaluate(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Evaluated> {
    let never = std::sync::atomic::AtomicBool::new(false);
    evaluate_with(design, lib, params, &BuildCtx::new(&never))
}

/// [`evaluate_memo`] with nothing remembered.
pub fn evaluate_with(
    design: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    ctx: &BuildCtx,
) -> Result<Evaluated> {
    evaluate_memo(design, lib, params, ctx, Memo::default())
}

/// Evaluate the document feature by feature: a failure stays on its feature and skips what it feeds; `Err` for an unreadable document, a raised flag, or nothing at all.
pub fn evaluate_memo(
    design: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    ctx: &BuildCtx,
    memo: Memo,
) -> Result<Evaluated> {
    let doc = design.cad.as_ref().context("No CAD features")?;
    ensure!(doc.features.len() <= 256, "Too many CAD features");
    let mut ids = BTreeSet::new();
    for f in &doc.features {
        ensure!(ids.insert(f.id), "Duplicate feature #{}", f.id);
    }
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
    let _ = lib;
    let chord = if params.theta_steps >= 512 { 0.015 } else { 0.04 };
    let bucket = u8::from(params.theta_steps >= 512);
    let sigs = signatures(doc, design, params, memo.surface_epoch);
    let scope = Scope { doc, memo, sigs: &sigs, chord, bucket };
    let who = |id: Id| doc.feature(id).map_or_else(|| format!("#{id}"), |f| format!("#{id} {}", f.name));
    let mut values: BTreeMap<Id, Value> = BTreeMap::new();
    let mut sketches: BTreeMap<Id, Sketch> = BTreeMap::new();
    let mut planes: Vec<WorkPlane> = Vec::new();
    let mut metadata = BTreeMap::new();
    // The frame each body was seated by, and each work plane's own.
    let mut frames: BTreeMap<Id, brep::Placement> = BTreeMap::new();
    // The attachment a boolean against the band stands for, by the boolean's id.
    let mut attached: BTreeMap<Id, Attach> = BTreeMap::new();
    let mut band: Option<Id> = None;
    let mut reports: Vec<FeatureReport> = Vec::new();
    let mut status: BTreeMap<Id, FeatureStatus> = BTreeMap::new();
    let mut available_outputs = Vec::new();
    for f in &doc.features {
        ctx.check()?;
        let mut report = FeatureReport {
            id: f.id,
            name: f.name.clone(),
            faces: 0,
            edges: 0,
            suppressed: !f.enabled,
            notes: Vec::new(),
            status: FeatureStatus::Ok,
        };
        if !f.enabled {
            // A suppressed feature passes the first body it consumes, and that body's frame, through.
            if let Some(id) = f.operation.consumes().first() {
                if let Some(value) = values.get(id).cloned() {
                    values.insert(f.id, value);
                    metadata.insert(f.id, f);
                    if let Some(frame) = frames.get(id).cloned() {
                        frames.insert(f.id, frame);
                    }
                }
            }
            report.status = FeatureStatus::Suppressed;
        } else if let Some(why) = skipped_by(&f.operation, &status, doc, |s| values.contains_key(&s) || sketches.contains_key(&s) || frames.contains_key(&s)) {
            report.status = FeatureStatus::Skipped(why);
        } else if let Operation::Plane { base, offset_mm } = &f.operation {
            // A work plane builds a frame and no body.
            match pattern::work_plane(f.id, base, *offset_mm, design, ctx.surface, &values, &frames, &mut report.notes) {
                Ok(plane) => {
                    frames.insert(f.id, plane.placement());
                    planes.push(plane);
                    metadata.insert(f.id, f);
                }
                Err(e) => report.status = FeatureStatus::Failed(format!("{e:#}")),
            }
        } else if let Operation::Sketch { sketch } = &f.operation {
            // A sketch on a face takes its plane from the face as built now, so what sweeps it follows the face.
            match laid(sketch, &values, &frames, &mut report.notes) {
                Ok(sketch) => {
                    sketches.insert(f.id, sketch);
                    metadata.insert(f.id, f);
                }
                Err(e) => report.status = FeatureStatus::Failed(format!("{e:#}")),
            }
        } else if matches!(f.operation, Operation::Band) {
            // The band is an anchor: it builds no body, and a boolean against it is an attachment.
            if band.is_some() {
                report.status = FeatureStatus::Failed("a document carries one procedural shank".into());
            } else {
                band = Some(f.id);
                metadata.insert(f.id, f);
            }
        } else {
            let sig = sigs[&f.id];
            let remembered = memo.cache.and_then(|c| c.lock().ok()?.body(sig));
            let outcome = match remembered {
                Some(outcome) => outcome,
                None => {
                    let outcome = Arc::new(
                        build_feature(f, design, ctx, params, band, &values, &sketches, &frames, &who, &scope).map_err(|e| format!("{e:#}")),
                    );
                    if let Some(Ok(mut c)) = memo.cache.map(Mutex::lock) {
                        c.keep_body(sig, outcome.clone());
                    }
                    outcome
                }
            };
            match &*outcome {
                Ok(built) => {
                    report.faces = built.value.faces();
                    report.edges = built.value.edges();
                    report.notes = built.notes.clone();
                    if let Some(frame) = built.frame {
                        frames.insert(f.id, frame);
                    }
                    if let Some(attach) = built.attach {
                        attached.insert(f.id, attach);
                    }
                    values.insert(f.id, built.value.clone());
                    metadata.insert(f.id, f);
                }
                Err(message) => report.status = FeatureStatus::Failed(message.clone()),
            }
        }
        status.insert(f.id, report.status.clone());
        reports.push(report);
        for id in f.operation.consumes() {
            available_outputs.retain(|v| *v != id);
        }
        if values.contains_key(&f.id) {
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
        let Some(value) = values.get(id) else {
            continue;
        };
        let f = metadata[id];
        let tessellated = match tessellated(value, sigs[id], bucket, chord, memo) {
            Ok(t) => t,
            Err(e) => {
                if let Some(r) = reports.iter_mut().find(|r| r.id == *id) {
                    r.status = FeatureStatus::Failed(format!("{e:#}"));
                }
                continue;
            }
        };
        components.push(EvaluatedComponent {
            id: *id,
            name: f.name.clone(),
            settings: f.component.clone(),
            body: value.brep().cloned().unwrap_or_else(Body::new),
            mesh: tessellated.mesh.clone(),
            edges: tessellated.edges.clone(),
            trace: tessellated.trace.clone(),
            attach: attached.get(id).copied().unwrap_or(f.component.attach),
            stage: f.component.stage,
            made: value.made().cloned(),
            frame: frames.get(id).copied().unwrap_or(brep::Placement::IDENTITY),
        });
    }
    // Nothing built, nothing failed and no band is not a ring.
    let failed = reports.iter().any(|r| matches!(r.status, FeatureStatus::Failed(_) | FeatureStatus::Skipped(_)));
    ensure!(!components.is_empty() || band.is_some() || failed, "No active CAD components");
    Ok(Evaluated {
        components,
        features: reports,
        band,
        planes,
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
    Ok((mesh, PartTrace { tri_face, positions: precise, face_kind, vertices, patches: Vec::new() }))
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
        let e = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        assert!(e.components.is_empty());
        assert_eq!(e.failures().len(), 1);
        assert!(e.first_error().unwrap().contains("#99"), "{:?}", e.first_error());
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
    fn a_sweep_a_twist_and_a_loft_take_one_region_of_several() {
        use crate::sketch::{Geometry, RegionRef, Workplane};
        let lib = AlphaLibrary::builtin();
        // Three regions on the plane z = 0: a 2 mm square, a 3 mm square, and a 6 mm square round a 2 mm hole.
        let mut s = Sketch::default();
        s.add_rectangle([1.0, 1.0], [3.0, 3.0], false).unwrap();
        let three = s.add_rectangle([5.0, -1.5], [8.0, 1.5], false).unwrap()[0];
        let framed = s.add_rectangle([10.0, 10.0], [16.0, 16.0], false).unwrap()[0];
        s.add_rectangle([12.0, 12.0], [14.0, 14.0], false).unwrap();
        let regions = s.profile_regions().unwrap();
        assert_eq!(regions.len(), 3);
        let pick = |e: Id| Profile::Region { feature: 1, region: RegionRef::of(regions.iter().find(|r| r.entities[..r.rim].contains(&e)).unwrap()).unwrap() };
        // The sketch as feature #1 and `op` as #2: #2's volume and whether it closed, or why it failed.
        let run = |op: Operation| -> std::result::Result<(f64, bool), String> {
            let e = evaluate(&design(vec![Operation::Sketch { sketch: s.clone() }, op]), &lib, BuildParams::default()).unwrap();
            match e.status_of(2) {
                Some(FeatureStatus::Ok) => e.components.iter().find(|c| c.id == 2).map(|c| (c.mesh.volume_mm3(), c.mesh.validate().watertight)).ok_or_else(|| "no part".into()),
                other => Err(format!("{other:?}")),
            }
        };
        let near = |got: std::result::Result<(f64, bool), String>, want: f64| {
            let (v, closed) = got.unwrap_or_else(|e| panic!("{e}"));
            assert!(closed && (v - want).abs() < 1e-3 * want, "{v} against {want}");
        };
        // Swept 4 mm up the finger: the 3 mm square alone, and the framed square with its hole kept.
        let up = vec![[0.0; 3], [0.0, 0.0, 4.0]];
        near(run(Operation::Sweep { sketch: pick(three), path: up.clone() }), 9.0 * 4.0);
        near(run(Operation::Sweep { sketch: pick(framed), path: up.clone() }), (36.0 - 4.0) * 4.0);
        // The whole sketch is four loops, which a sweep never took.
        assert!(run(Operation::Sweep { sketch: Profile::Feature { feature: 1 }, path: up }).unwrap_err().contains("4 separate loops"));
        // Twisted along a 5 mm line up the finger without a turn: the 3 mm square's prism.
        let mut line = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let (a, b) = (line.point([0.0, 0.0]), line.point([0.0, 5.0]));
        line.entity(Geometry::Line { a, b });
        let twist = |from: Profile| Operation::Twist { sketch: from, path: line.clone(), degrees: 0.0, end_scale: 1.0 };
        near(run(twist(pick(three))), 9.0 * 5.0);
        let holed = run(twist(pick(framed))).unwrap_err();
        assert!(holed.contains("Twisted sweep: the region of sketch #1 has 1 hole; it takes one closed loop"), "{holed}");
        // Lofted from the 3 mm square to the same square 5 mm over it: a prism again.
        let mut top = Sketch::rectangle(3.0, 3.0);
        top.plane.origin = [6.5, 0.0, 5.0];
        near(run(Operation::Loft { sections: vec![pick(three), top.clone().into()] }), 9.0 * 5.0);
        let holed = run(Operation::Loft { sections: vec![pick(framed), top.into()] }).unwrap_err();
        assert!(holed.contains("Loft: the region of sketch #1 has 1 hole; it takes one closed loop"), "{holed}");
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
            .unwrap()
            .first_error()
            .unwrap();
        assert!(error.contains("2 separate loops"), "{error}");
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
        // A disabled sketch takes its extrusions down with it, by name: skipped, since nothing failed.
        let mut off = doc.clone();
        off.features[0].enabled = false;
        d.cad = Some(off);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert_eq!(e.status_of(1), Some(&FeatureStatus::Suppressed));
        for id in [2, 3] {
            assert_eq!(e.status_of(id), Some(&FeatureStatus::Skipped("source #1 Sketch was suppressed".into())));
        }
        assert!(e.failures().is_empty() && e.first_error().is_none());
        assert!(e.components.is_empty());
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
            Operation::TwistedRing { major_mm: 10.0, radial_mm: 2.0, axial_mm: 4.0, turns: 0.5 },
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
        let e = evaluate(&d, &AlphaLibrary::builtin(), params).unwrap();
        let error = e.first_error().unwrap();
        assert!(error.contains("faceted solid"), "{error}");
        assert!(matches!(e.status_of(3), Some(FeatureStatus::Failed(_))));
        assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
    }
    #[test]
    fn the_band_is_an_anchor_and_a_boolean_against_it_is_an_attachment() {
        let lib = AlphaLibrary::builtin();
        let cylinder = || Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 };
        let boolean = |a, b, kind| Operation::Boolean { a, b, kind };
        // Union(band, part): the part's body comes out as the boolean's, attached Join; the band builds nothing.
        let d = design(vec![Operation::Band, cylinder(), boolean(1, 2, Boolean::Union)]);
        let started = std::time::Instant::now();
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert!(started.elapsed().as_millis() < 2000, "no faceted band went into the kernel: {:?}", started.elapsed());
        assert_eq!(e.band, Some(1));
        assert_eq!(e.features[0].faces, 0);
        assert_eq!(e.components.len(), 1);
        assert_eq!((e.components[0].id, e.components[0].attach, e.components[0].stage), (3, Attach::Join, Stage::Cast));
        assert!((e.components[0].mesh.volume_mm3() - std::f64::consts::PI * 9.0 * 2.5).abs() < 0.3);
        assert!(!d.cad.as_ref().unwrap().replaces_band() && d.band_is_procedural());
        assert_eq!(d.cad.as_ref().unwrap().attachments(), vec![(3, Attach::Join, Stage::Cast)]);
        // Subtract(band, part) cuts; the other way round, and Intersect, are refused by name.
        let d = design(vec![Operation::Band, cylinder(), boolean(1, 2, Boolean::Subtract)]);
        assert_eq!(evaluate(&d, &lib, BuildParams::default()).unwrap().components[0].attach, Attach::Cut);
        let d = design(vec![Operation::Band, cylinder(), boolean(2, 1, Boolean::Subtract)]);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        let error = e.first_error().unwrap();
        assert!(error.starts_with("Feature #3 — Subtract: subtracting the procedural shank"), "{error}");
        assert!(e.components.is_empty() && e.band == Some(1));
        let d = design(vec![Operation::Band, cylinder(), boolean(1, 2, Boolean::Intersect)]);
        let error = evaluate(&d, &lib, BuildParams::default()).unwrap().first_error().unwrap();
        assert!(error.contains("intersecting with the procedural shank"), "{error}");
        // A plain part beside the band keeps its component's own attach; a band alone is a ring with nothing on it.
        let mut d = design(vec![Operation::Band, cylinder()]);
        d.cad.as_mut().unwrap().features[1].component.attach = Attach::Cut;
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert_eq!((e.components.len(), e.components[0].attach), (1, Attach::Cut));
        let d = design(vec![Operation::Band]);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert!(e.components.is_empty() && e.band == Some(1));
        assert!(d.band_is_procedural());
        // Without a Band feature the document is the whole ring, as the shipped examples are.
        let d = design(vec![cylinder()]);
        assert!(d.cad.as_ref().unwrap().replaces_band() && !d.band_is_procedural());
        assert!(RingDesign::default().band_is_procedural());
        let mut only_sketch = Document::default();
        only_sketch.append(Feature { id: 1, name: "s".into(), enabled: true, operation: Operation::Sketch { sketch: Sketch::rectangle(2.0, 2.0) }, component: Component::default() }).unwrap();
        assert!(!only_sketch.replaces_band(), "a sketch alone builds nothing and replaces nothing");
    }
    #[test]
    fn a_ring_placement_drops_onto_the_built_surface() {
        let lib = AlphaLibrary::builtin();
        let params = BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() };
        // The reference-crest anchor buries a part's foot on a signet's shoulder; the surface drop does not.
        let heart = crate::templates::all().iter().find(|t| t.name == "Heart signet").unwrap().design();
        let built = crate::mesh::try_build(&heart, &lib, params).unwrap();
        let seat = Placement::ring(45.0, 0.0);
        let reference = seat.frame(&heart).unwrap();
        let dropped = seat.frame_on(&heart, Some(&built.mesh)).unwrap();
        let radius = |p: [f64; 3]| p[0].hypot(p[1]);
        let (hit, normal) = surface_hit(&built.mesh, 45.0, 0.0).unwrap();
        let foot_gap = radius(dropped.origin) - radius(hit);
        assert!(foot_gap.abs() < 0.02, "the foot stands on the surface: {foot_gap:.4}");
        assert!((radius(hit) - radius(reference.origin)).abs() > 1.0, "the reference formula was millimetres off here: {:.2}", radius(hit) - radius(reference.origin));
        assert!(dropped.z_axis.iter().zip(normal).all(|(a, b)| (a - b).abs() < 1e-9), "z along the hit normal");
        assert!(dropped.x_axis[2] < -0.9, "x runs along the finger: {:?}", dropped.x_axis);
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(dot(dropped.x_axis, dropped.z_axis).abs() < 1e-9 && dot(dropped.y_axis, dropped.z_axis).abs() < 1e-9);
        // On a plain band the drop is the reference frame to a hundredth, and the stand-off rides the normal.
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let built = crate::mesh::try_build(&court, &lib, params).unwrap();
        let seat = Placement::ring(90.0, 0.4);
        let a = seat.frame(&court).unwrap();
        let b = seat.frame_on(&court, Some(&built.mesh)).unwrap();
        assert!((0..3).all(|k| (a.origin[k] - b.origin[k]).abs() < 0.02), "{:?} vs {:?}", a.origin, b.origin);
        // The grid normal at the snapped crest row tilts by its neighbours' uneven spacing: 0.94° at 256x128.
        assert!((0..3).all(|k| (a.z_axis[k] - b.z_axis[k]).abs() < 0.02), "{:?} vs {:?}", a.z_axis, b.z_axis);
        // No surface, or a ray that misses, is the reference frame exactly.
        let c = seat.frame_on(&court, None).unwrap();
        assert!(a.origin == c.origin && a.x_axis == c.x_axis && a.y_axis == c.y_axis && a.z_axis == c.z_axis);
        let off = Placement::Ring { theta_deg: 90.0, across_mm: 40.0, height_mm: 0.4, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        assert_eq!(off.frame_on(&court, Some(&built.mesh)).unwrap().origin, off.frame(&court).unwrap().origin);
        // Evaluating with the surface in the context seats the body there.
        let mut d = court.clone();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "boss".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 1.0 }, component: Component { placement: Placement::ring(90.0, 0.5), ..Default::default() } }).unwrap();
        d.cad = Some(doc);
        let never = std::sync::atomic::AtomicBool::new(false);
        let e = evaluate_with(&d, &lib, params, &BuildCtx { cancel: &never, surface: Some(&built.mesh) }).unwrap();
        let (lo, hi) = e.components[0].mesh.bounds().unwrap();
        let (hit, _) = surface_hit(&built.mesh, 90.0, 0.0).unwrap();
        assert!((lo.1 as f64 - hit[1]).abs() < 0.02 && (hi.1 as f64 - hit[1] - 1.0).abs() < 0.02, "{lo:?} {hi:?} on {hit:?}");
    }
    #[test]
    fn a_raised_flag_stops_the_evaluation_between_features() {
        let d = design(vec![Operation::Box { size: [4.0; 3] }]);
        let stop = std::sync::atomic::AtomicBool::new(true);
        let error = evaluate_with(
            &d,
            &AlphaLibrary::builtin(),
            BuildParams::default(),
            &BuildCtx { cancel: &stop, surface: None },
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), CANCELLED);
    }
    #[test]
    fn a_component_reads_separate_and_cast_unless_the_file_says_otherwise() {
        let bare: Component = serde_json::from_str("{}").unwrap();
        assert_eq!(bare.attach, Attach::Separate);
        assert_eq!(bare.stage, Stage::Cast);
        assert_eq!(bare.blend_mm, 0.0);
        assert!(!bare.attaches());
        let joined = Component { attach: Attach::Join, stage: Stage::Bench, blend_mm: 0.3, ..Default::default() };
        let json = serde_json::to_value(&joined).unwrap();
        assert_eq!(json["attach"], "join");
        assert_eq!(json["stage"], "bench");
        assert_eq!(json["blend_mm"], 0.3);
        let back: Component = serde_json::from_value(json).unwrap();
        assert_eq!((back.attach, back.stage, back.blend_mm), (Attach::Join, Stage::Bench, 0.3));
        assert!(back.attaches());
        let cut: Component = serde_json::from_str(r#"{"attach": "cut"}"#).unwrap();
        assert!(cut.attaches());
        // A reference stone is never metal, whatever its attach says.
        let stone = Component { attach: Attach::Join, reference: true, ..Default::default() };
        assert!(!stone.attaches());
        // A v4 file folds its anchor into a placement and takes the new defaults beside it.
        let legacy: Component = serde_json::from_str(r#"{"ring_anchor_deg": 30.0, "anchor_height_mm": 0.5, "role": "Head"}"#).unwrap();
        assert_eq!(legacy.placement, Placement::ring(30.0, 0.5));
        assert_eq!(legacy.role, ComponentRole::Head);
        assert_eq!((legacy.attach, legacy.stage, legacy.blend_mm), (Attach::Separate, Stage::Cast, 0.0));
        // The shipped examples stay beside the band and build the same parts they did.
        for name in crate::cad::examples::NAMES.iter().filter(|n| !crate::cad::examples::design(n).unwrap().band_is_procedural()) {
            let d = crate::cad::examples::design(name).unwrap();
            let doc = d.cad.as_ref().unwrap();
            assert!(doc.features.iter().all(|f| f.component.attach == Attach::Separate && f.component.stage == Stage::Cast), "{name}");
            let json = serde_json::to_value(doc).unwrap();
            let back: Document = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(serde_json::to_value(&back).unwrap(), json, "{name}");
            let e = evaluate(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
            assert!(!e.components.is_empty(), "{name}");
            assert!(e.components.iter().all(|c| c.mesh.volume_mm3() > 0.001), "{name}");
            assert!(e.failures().is_empty() && e.features.iter().all(|r| r.status.is_ok()), "{name}: {:?}", e.failures());
        }
    }
    #[test]
    fn a_failed_feature_keeps_its_failure_and_takes_only_what_stands_on_it() {
        let lib = AlphaLibrary::builtin();
        let d = design(vec![
            Operation::Band,
            Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 },
            Operation::Fillet { source: 2, edges: vec![EdgeRef::bare(999)], radius_mm: 0.3 },
            Operation::Box { size: [4.0; 3] },
            Operation::Chamfer { source: 3, edges: vec![EdgeRef::bare(0)], base_face: FaceRef::bare(0), distance_mm: 0.2 },
        ]);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert_eq!(e.band, Some(1));
        assert_eq!(e.status_of(1), Some(&FeatureStatus::Ok));
        assert_eq!(e.status_of(2), Some(&FeatureStatus::Ok));
        assert_eq!(e.status_of(4), Some(&FeatureStatus::Ok));
        let Some(FeatureStatus::Failed(message)) = e.status_of(3) else { panic!("{:?}", e.status_of(3)) };
        assert_eq!(message, "Edge 999 is unavailable; reselect after changing the source");
        let Some(FeatureStatus::Skipped(why)) = e.status_of(5) else { panic!("{:?}", e.status_of(5)) };
        assert_eq!(why, "source #3 Fillet failed");
        assert!(e.features.iter().all(|r| !r.suppressed && r.notes.is_empty()));
        assert_eq!(e.features.iter().filter(|r| r.status.is_ok()).count(), 3);
        // The box is the one output left standing; the skipped chamfer holds no body.
        assert_eq!(e.components.iter().map(|c| c.id).collect::<Vec<_>>(), vec![4]);
        assert!((e.components[0].mesh.volume_mm3() - 64.0).abs() < 1e-6);
        assert_eq!(e.failures(), vec![(3, "Edge 999 is unavailable; reselect after changing the source")]);
        assert_eq!(e.first_error().unwrap(), "Feature #3 — Fillet: Edge 999 is unavailable; reselect after changing the source");
        assert_eq!(e.status_of(9), None);
        // A sketch on a failed feature's face is skipped, and so is what sweeps it.
        let mut sketch = Sketch::rectangle(2.0, 2.0);
        sketch.plane.on_face = Some(crate::sketch::FaceAnchor { feature: 2, face: FaceRef::bare(0) });
        let d = design(vec![
            Operation::Box { size: [6.0; 3] },
            Operation::Fillet { source: 1, edges: vec![EdgeRef::bare(999)], radius_mm: 0.3 },
            Operation::Sketch { sketch },
            Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: 1.0, draft_deg: 0.0 },
            Operation::Sphere { radius_mm: 1.0 },
        ]);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert_eq!(e.status_of(3), Some(&FeatureStatus::Skipped("source #2 Fillet failed".into())));
        assert_eq!(e.status_of(4), Some(&FeatureStatus::Skipped("source #3 Sketch was skipped".into())));
        assert_eq!((e.failures().len(), e.components.iter().map(|c| c.id).collect::<Vec<_>>()), (1, vec![5]));
        // A document whose only body failed is still an evaluation, with the failure on the feature.
        let d = design(vec![Operation::Fillet { source: 99, edges: vec![EdgeRef::bare(0)], radius_mm: 1.0 }]);
        let e = evaluate(&d, &lib, BuildParams::default()).unwrap();
        assert!(e.components.is_empty() && e.band.is_none() && e.failures().len() == 1);
        // Document-level faults are still refused outright: a duplicate id, a missing output.
        let mut dup = design(vec![Operation::Box { size: [4.0; 3] }, Operation::Sphere { radius_mm: 1.0 }]);
        dup.cad.as_mut().unwrap().features[1].id = 1;
        assert!(evaluate(&dup, &lib, BuildParams::default()).unwrap_err().to_string().contains("Duplicate feature #1"));
        let mut gone = design(vec![Operation::Box { size: [4.0; 3] }]);
        gone.cad.as_mut().unwrap().outputs.push(7);
        assert!(evaluate(&gone, &lib, BuildParams::default()).unwrap_err().to_string().contains("missing feature"));
        // A second procedural shank is a failed feature, not a failed document.
        let e = evaluate(&design(vec![Operation::Band, Operation::Band]), &lib, BuildParams::default()).unwrap();
        assert_eq!((e.band, e.failures().len()), (Some(1), 1));
        // An empty document is not a ring.
        let e = evaluate(&design(vec![]), &lib, BuildParams::default()).unwrap_err().to_string();
        assert_eq!(e, "No active CAD components");
        // The status travels in the serialized report.
        let report: serde_json::Value = serde_json::to_value(&evaluate(&d, &lib, BuildParams::default()).unwrap().features).unwrap();
        assert_eq!(report[0]["status"]["Failed"].as_str().unwrap(), "Source feature #99 is unavailable or suppressed");
    }
    #[test]
    fn the_cache_reuses_unchanged_features_and_rebuilds_downstream_of_an_edit() {
        let lib = AlphaLibrary::builtin();
        let params = BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() };
        let never = std::sync::atomic::AtomicBool::new(false);
        let ctx = BuildCtx::new(&never);
        let cache = Mutex::new(Cache::default());
        let memo = Memo::new(&cache);
        let tally = |cache: &Mutex<Cache>| {
            let c = cache.lock().unwrap();
            (c.hits(), c.misses())
        };
        let counts = || tally(&cache);
        let mut d = design(vec![
            Operation::Box { size: [8.0; 3] },
            Operation::Fillet { source: 1, edges: vec![EdgeRef::bare(0)], radius_mm: 0.5 },
            Operation::Cylinder { radius_mm: 2.0, height_mm: 3.0 },
        ]);
        // Cold: three bodies and two tessellations built.
        let a = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (0, 5));
        assert_eq!(cache.lock().unwrap().len(), 5);
        // Warm, unchanged: every lookup answers, and the result is the same bytes.
        let b = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (5, 5));
        assert!(a.components.iter().zip(&b.components).all(|(p, q)| p.mesh.vertices == q.mesh.vertices && p.mesh.faces == q.mesh.faces));
        assert_eq!(a.features.len(), b.features.len());
        // Editing the fillet rebuilds and re-tessellates it alone: the box and the cylinder answer.
        d.cad.as_mut().unwrap().features[1].operation = Operation::Fillet { source: 1, edges: vec![EdgeRef::bare(0)], radius_mm: 0.8 };
        let c = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (8, 7));
        assert!(c.components[0].mesh.volume_mm3() < b.components[0].mesh.volume_mm3());
        // Editing the box invalidates the fillet standing on it; the cylinder still answers.
        d.cad.as_mut().unwrap().features[0].operation = Operation::Box { size: [9.0; 3] };
        evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (10, 10));
        // A rename, a material and an attachment build nothing, and the component carries the new ones.
        let post = &mut d.cad.as_mut().unwrap().features[2];
        post.name = "post".into();
        post.component.material = "Gold 18k".into();
        post.component.attach = Attach::Cut;
        let renamed = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (15, 10));
        let c = renamed.components.iter().find(|c| c.id == 3).unwrap();
        assert_eq!((c.name.as_str(), c.settings.material.as_str(), c.attach), ("post", "Gold 18k", Attach::Cut));
        // An export chord re-tessellates every output and rebuilds no body.
        let export = BuildParams { theta_steps: 1024, profile_steps: 384, ..params };
        evaluate_memo(&d, &lib, export, &ctx, memo).unwrap();
        assert_eq!(counts(), (18, 12));
        // Without a cache the same document evaluates to the same components.
        let plain = evaluate_with(&d, &lib, export, &ctx).unwrap();
        let cached = evaluate_memo(&d, &lib, export, &ctx, memo).unwrap();
        assert!(plain.components.iter().zip(&cached.components).all(|(p, q)| p.mesh.vertices == q.mesh.vertices && p.trace.tri_face == q.trace.tri_face));
        // A failure is remembered too, and a feature skipped behind it is never looked up.
        let mut broken = design(vec![
            Operation::Box { size: [8.0; 3] },
            Operation::Fillet { source: 1, edges: vec![EdgeRef::bare(999)], radius_mm: 0.5 },
            Operation::Chamfer { source: 2, edges: vec![EdgeRef::bare(0)], base_face: FaceRef::bare(0), distance_mm: 0.2 },
        ]);
        let fresh = Mutex::new(Cache::default());
        let memo = Memo::new(&fresh);
        let e = evaluate_memo(&broken, &lib, params, &ctx, memo).unwrap();
        assert!(matches!(e.status_of(2), Some(FeatureStatus::Failed(_))) && matches!(e.status_of(3), Some(FeatureStatus::Skipped(_))));
        assert_eq!(tally(&fresh), (0, 2));
        evaluate_memo(&broken, &lib, params, &ctx, memo).unwrap();
        assert_eq!(tally(&fresh), (2, 2));
        // A ring placement is re-seated on a new surface epoch and not otherwise.
        broken.cad.as_mut().unwrap().features.truncate(1);
        broken.cad.as_mut().unwrap().outputs = vec![1];
        broken.cad.as_mut().unwrap().features[0].component.placement = Placement::ring(90.0, 0.5);
        let seated = Mutex::new(Cache::default());
        let on = Memo::new(&seated).with_epoch(1);
        evaluate_memo(&broken, &lib, params, &ctx, on).unwrap();
        evaluate_memo(&broken, &lib, params, &ctx, on).unwrap();
        assert_eq!(tally(&seated), (2, 2));
        evaluate_memo(&broken, &lib, params, &ctx, on.with_epoch(2)).unwrap();
        assert_eq!(tally(&seated), (2, 4));
        // The epoch is the surface's own: bit-identical meshes agree, a moved vertex does not.
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let built = crate::mesh::try_build(&court, &lib, params).unwrap();
        let again = crate::mesh::try_build(&court, &lib, params).unwrap();
        assert_eq!(surface_epoch(&built.mesh), surface_epoch(&again.mesh));
        let mut moved = built.mesh.clone();
        moved.vertices[7].1 += 1e-3;
        assert_ne!(surface_epoch(&built.mesh), surface_epoch(&moved));
        assert_ne!(surface_epoch(&built.mesh), surface_epoch(&Mesh::default()));
        // The entries are bounded, least recently used out first: a hit keeps an entry in.
        let mut c = Cache::default();
        for i in 0..(CACHE_ENTRIES as u64 + 3) {
            c.keep_body(i, Arc::new(Err(String::new())));
        }
        assert_eq!(c.len(), CACHE_ENTRIES);
        assert!(c.body(0).is_none() && c.body(3).is_some());
        c.keep_body(9_000, Arc::new(Err(String::new())));
        assert!(c.body(3).is_some() && c.body(4).is_none() && c.len() == CACHE_ENTRIES);
        c.clear();
        assert!(c.is_empty() && c.bytes() == 0 && c.hits() == 0);
        // The bytes are bounded too, and an entry over the whole bound is not kept and evicts nothing.
        let mut c = Cache::with_limits(8, 1000);
        let failure = |n: usize| Arc::new(Err("x".repeat(n)));
        c.keep_body(1, failure(400));
        c.keep_body(2, failure(400));
        assert_eq!((c.len(), c.bytes()), (2, 928));
        c.keep_body(3, failure(400));
        assert!(c.body(1).is_none() && c.body(2).is_some() && c.body(3).is_some() && c.bytes() == 928);
        c.keep_body(4, failure(2000));
        assert!(c.body(4).is_none() && c.len() == 2 && c.bytes() == 928);
        // A real tessellation is sized from its arrays.
        let mut c = Cache::default();
        let e = evaluate(&design(vec![Operation::Box { size: [4.0; 3] }]), &lib, params).unwrap();
        let box_ = &e.components[0];
        let t = Arc::new(Tessellated { mesh: box_.mesh.clone(), trace: box_.trace.clone(), edges: box_.edges.clone() });
        let arrays = t.bytes();
        assert!(arrays >= box_.mesh.faces.len() * 12 + box_.mesh.vertices.len() * 12, "{arrays}");
        c.keep_mesh((7, 0), t);
        assert_eq!(c.bytes(), 64 + arrays);
        // The GUI worker holds one across threads.
        fn shared<T: Send + Sync>() {}
        shared::<Mutex<Cache>>();
    }
}

#[cfg(test)]
mod sketch_tests {
    use super::*;
    use crate::sketch::{FaceAnchor, FaceFrame, Geometry, Workplane};
    use std::f64::consts::PI;
    use std::sync::atomic::AtomicBool;

    fn feature(id: Id, operation: Operation) -> Feature {
        Feature { id, name: operation.label().into(), enabled: true, operation, component: Component::default() }
    }
    fn design_of(features: Vec<Feature>, outputs: Vec<Id>) -> RingDesign {
        let mut doc = Document::default();
        for f in features {
            doc.append(f).unwrap();
        }
        assert_eq!(doc.outputs, outputs, "a face a sketch lies on stays an output");
        RingDesign { cad: Some(doc), ..RingDesign::default() }
    }
    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }
    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|k| a[k] - b[k])
    }
    /// The planar face whose outward normal, in the part's own frame, lies along `dir`.
    fn face_along(body: &Body, frame: &brep::Placement, dir: [f64; 3]) -> usize {
        (0..body.faces.len())
            .find(|i| face_signature(body, *i, frame).is_some_and(|s| s.kind == SurfaceKind::Plane && dot(s.normal, dir) > 0.99))
            .unwrap()
    }
    /// The frame of the planar face whose outward normal lies along `dir` in the world.
    fn frame_along(body: &Body, dir: [f64; 3]) -> FaceFrame {
        let face = body
            .faces
            .iter()
            .filter_map(|(k, _)| brep::planar_face_profile(body, k))
            .find(|p| dot(p.outward, dir) > 0.99)
            .unwrap();
        FaceFrame::of(&face).unwrap()
    }
    fn extrude(from: Profile, height_mm: f64) -> Operation {
        Operation::Extrude { sketch: from, height_mm, draft_deg: 0.0 }
    }

    #[test]
    fn a_sketch_on_a_face_stands_its_extrusion_there_and_follows_the_face() {
        let lib = AlphaLibrary::builtin();
        let params = BuildParams::default();
        let seat = Placement::ring(90.0, 0.0);
        let boxed = |h: f64| {
            let mut f = feature(1, Operation::Box { size: [8.0, 6.0, h] });
            f.component.placement = seat.clone();
            f
        };
        let d = design_of(vec![boxed(2.0)], vec![1]);
        let frame = seat.frame(&d).unwrap();
        let block = evaluate(&d, &lib, params).unwrap().components.remove(0).body;
        let top = face_along(&block, &frame, [0.0, 0.0, 1.0]);
        let anchor = FaceAnchor { feature: 1, face: FaceRef::signed(&block, top, &frame) };
        // A 2 mm circle on the top face, extruded 1 by a feature that names the sketch.
        let mut circle = Sketch::circle(1.0);
        circle.plane.on_face = Some(anchor.clone());
        let build = |h: f64| {
            let d = design_of(
                vec![boxed(h), feature(2, Operation::Sketch { sketch: circle.clone() }), feature(3, extrude(Profile::Feature { feature: 2 }, 1.0))],
                vec![1, 3],
            );
            let e = evaluate(&d, &lib, params).unwrap();
            assert!(e.failures().is_empty(), "{:?}", e.failures());
            e
        };
        let out = frame.z_axis;
        let down = out.map(|v| -v);
        let e = build(2.0);
        let face = frame_along(&e.components[0].body, out);
        let base = frame_along(&e.components[1].body, down).origin;
        let centre: [f64; 3] = std::array::from_fn(|k| frame.origin[k] + out[k]);
        assert!(sub(face.origin, centre).iter().all(|v| v.abs() < 1e-9), "{:?} vs {centre:?}", face.origin);
        assert!(sub(base, face.origin).iter().all(|v| v.abs() < 1e-6), "the base centre is the face centroid: {base:?} vs {:?}", face.origin);
        assert!(dot(face.x, frame.x_axis).abs() > 1.0 - 1e-12, "x runs along the 8 mm edge");
        // The plane a canvas would draw the sketch in, read straight off the box.
        let mut notes = Vec::new();
        let drawn = sketch_plane(&circle, &e.components[0].body, &frame, &mut notes).unwrap();
        assert!(sub(drawn.origin, face.origin).iter().all(|v| v.abs() < 1e-12) && notes.is_empty());
        assert!(dot(drawn.normal().unwrap(), out) > 1.0 - 1e-12 && dot(drawn.x_axis, face.x) > 1.0 - 1e-12);
        let post = &e.components[1].mesh;
        assert!(post.validate().watertight);
        assert!((post.volume_mm3() / PI - 1.0).abs() < 0.005, "{}", post.volume_mm3());
        // The box grows about its centre: its top rises half the growth and the post rises with it.
        for (h, rise) in [(3.0, 0.5), (4.0, 1.0)] {
            let moved = frame_along(&build(h).components[1].body, down).origin;
            let step = sub(moved, base);
            assert!((dot(step, out) - rise).abs() < 1e-9, "{h}: {step:?}");
            assert!(dot(step, frame.x_axis).abs() < 1e-9 && dot(step, frame.y_axis).abs() < 1e-9, "{h}: {step:?}");
        }
        // An inline sketch 2 mm off centre lands 2 mm along the longest edge, in the face.
        let mut square = Sketch::rectangle(2.0, 2.0);
        for p in &mut square.points {
            p.xy[0] += 2.0;
        }
        square.plane.on_face = Some(anchor.clone());
        let d = design_of(vec![boxed(2.0), feature(2, extrude(square.into(), 1.0))], vec![1, 2]);
        let e = evaluate(&d, &lib, params).unwrap();
        let foot = sub(frame_along(&e.components[1].body, down).origin, face.origin);
        assert!((dot(foot, frame.x_axis).abs() - 2.0).abs() < 1e-9, "{foot:?}");
        assert!(dot(foot, frame.y_axis).abs() < 1e-9 && dot(foot, out).abs() < 1e-9, "{foot:?}");
        // A workplane origin is an offset read in the face's frame.
        let mut lifted = Sketch::circle(1.0);
        lifted.plane = Workplane { origin: [0.0, 0.0, 0.5], on_face: Some(anchor), ..Workplane::default() };
        let d = design_of(vec![boxed(2.0), feature(2, extrude(lifted.into(), 1.0))], vec![1, 2]);
        let e = evaluate(&d, &lib, params).unwrap();
        let foot = sub(frame_along(&e.components[1].body, down).origin, face.origin);
        assert!((dot(foot, out) - 0.5).abs() < 1e-9, "{foot:?}");
    }

    #[test]
    fn a_face_gone_or_curved_fails_the_sketch_and_skips_what_sweeps_it() {
        let lib = AlphaLibrary::builtin();
        let params = BuildParams::default();
        let identity = brep::Placement::IDENTITY;
        let block = make::cuboid([-4.0, -3.0, -1.0], [8.0, 6.0, 2.0]).unwrap();
        let side = face_along(&block, &identity, [1.0, 0.0, 0.0]);
        let mut sketch = Sketch::rectangle(1.0, 1.0);
        sketch.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::signed(&block, side, &identity) });
        let doc = |first: Operation, sketch: Sketch| {
            design_of(
                vec![feature(1, first), feature(2, Operation::Sketch { sketch }), feature(3, extrude(Profile::Feature { feature: 2 }, 1.0))],
                vec![1, 3],
            )
        };
        let cube = || Operation::Box { size: [8.0, 6.0, 2.0] };
        let drum = || Operation::Cylinder { radius_mm: 3.0, height_mm: 2.0 };
        let e = evaluate(&doc(cube(), sketch.clone()), &lib, params).unwrap();
        assert!(e.failures().is_empty() && e.features[1].notes.is_empty(), "{:?}", e.failures());
        // The box turned into a drum has no face looking along x: the sketch fails and its extrusion is skipped.
        let e = evaluate(&doc(drum(), sketch.clone()), &lib, params).unwrap();
        let Some(FeatureStatus::Failed(why)) = e.status_of(2) else { panic!("{:?}", e.status_of(2)) };
        assert_eq!(why, &format!("Sketch face: Face {side} is no longer a Plane face; the source changed underneath, pick it again"));
        assert_eq!(e.status_of(3), Some(&FeatureStatus::Skipped("source #2 Sketch failed".into())));
        assert_eq!(e.components.iter().map(|c| c.id).collect::<Vec<_>>(), vec![1]);
        // A curved face is refused by its kind.
        let round = make::cylinder([0.0, 0.0, -1.0], 3.0, 2.0).unwrap();
        let wall = (0..round.faces.len())
            .find(|i| face_signature(&round, *i, &identity).is_some_and(|s| s.kind == SurfaceKind::Cylinder))
            .unwrap();
        sketch.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::signed(&round, wall, &identity) });
        let e = evaluate(&doc(drum(), sketch.clone()), &lib, params).unwrap();
        let Some(FeatureStatus::Failed(why)) = e.status_of(2) else { panic!("{:?}", e.status_of(2)) };
        assert_eq!(why, &format!("Sketch face {wall} of feature #1: sketch planes need a planar face: cylinder"));
        // A signature kept under the wrong ordinal is found again, and the sketch says so.
        let wrong = (side + 1) % block.faces.len();
        sketch.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef { ordinal: wrong, ..FaceRef::signed(&block, side, &identity) } });
        let e = evaluate(&doc(cube(), sketch), &lib, params).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        assert_eq!(
            e.features[1].notes,
            vec![format!("Face {wrong} is now face {side}; the source changed underneath and the same face was found again")]
        );
    }

    #[test]
    fn growing_the_box_rebuilds_what_stands_on_its_face_and_an_edit_elsewhere_does_not() {
        let lib = AlphaLibrary::builtin();
        let params = BuildParams::default();
        let never = AtomicBool::new(false);
        let ctx = BuildCtx::new(&never);
        let cache = Mutex::new(Cache::default());
        let memo = Memo::new(&cache);
        let counts = || {
            let c = cache.lock().unwrap();
            (c.hits(), c.misses())
        };
        let identity = brep::Placement::IDENTITY;
        let block = make::cuboid([-4.0, -3.0, -1.0], [8.0, 6.0, 2.0]).unwrap();
        let mut circle = Sketch::circle(1.0);
        let top = face_along(&block, &identity, [0.0, 0.0, 1.0]);
        circle.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::signed(&block, top, &identity) });
        let mut d = design_of(
            vec![
                feature(1, Operation::Box { size: [8.0, 6.0, 2.0] }),
                feature(2, Operation::Sketch { sketch: circle }),
                feature(3, extrude(Profile::Feature { feature: 2 }, 1.0)),
                feature(4, Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }),
            ],
            vec![1, 3, 4],
        );
        let floor = |e: &Evaluated| e.components.iter().find(|c| c.id == 3).unwrap().mesh.bounds().unwrap().0 .2 as f64;
        // Cold: three bodies and three tessellations; a sketch holds no body.
        let e = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (0, 6));
        assert!((floor(&e) - 1.0).abs() < 1e-6);
        evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (6, 6));
        // Growing the box rebuilds it and the extrusion on its face; the cylinder answers.
        d.cad.as_mut().unwrap().features[0].operation = Operation::Box { size: [8.0, 6.0, 3.0] };
        let e = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (8, 10));
        assert!((floor(&e) - 1.5).abs() < 1e-6, "the post rose with the face: {}", floor(&e));
        // An edit elsewhere rebuilds only itself.
        d.cad.as_mut().unwrap().features[3].operation = Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 };
        let e = evaluate_memo(&d, &lib, params, &ctx, memo).unwrap();
        assert_eq!(counts(), (12, 12));
        assert!((floor(&e) - 1.5).abs() < 1e-6);
        // The recipe of everything that reads the face carries the box's.
        let sigs = signatures(d.cad.as_ref().unwrap(), &d, params, 0);
        let mut grown = d.clone();
        grown.cad.as_mut().unwrap().features[0].operation = Operation::Box { size: [8.0, 6.0, 3.5] };
        let after = signatures(grown.cad.as_ref().unwrap(), &grown, params, 0);
        assert_eq!([1, 2, 3, 4].map(|id| sigs[&id] != after[&id]), [true, true, true, false]);
    }

    #[test]
    fn a_washer_extrudes_with_its_hole_and_separate_regions_as_separate_lumps() {
        let lib = AlphaLibrary::builtin();
        let export = BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() };
        let mut washer = Sketch::default();
        for r in [3.0, 2.0] {
            let centre = washer.point([0.0; 2]);
            let rim = washer.point([r, 0.0]);
            washer.entity(Geometry::Circle { center: centre, rim });
        }
        let expected = PI * (9.0 - 4.0) * 2.0;
        for inline in [true, false] {
            let d = if inline {
                design_of(vec![feature(2, extrude(washer.clone().into(), 2.0))], vec![2])
            } else {
                design_of(vec![feature(1, Operation::Sketch { sketch: washer.clone() }), feature(2, extrude(Profile::Feature { feature: 1 }, 2.0))], vec![2])
            };
            let e = evaluate(&d, &lib, export).unwrap();
            assert!(e.failures().is_empty(), "{:?}", e.failures());
            let c = &e.components[0];
            assert!(c.mesh.validate().watertight);
            assert_eq!(c.body.roots.len(), 1);
            let v = c.mesh.volume_mm3();
            assert!((v / expected - 1.0).abs() < 0.005, "{v} against {expected}");
        }
        // Drafted, the outer wall leans in and the hole's leans out: less metal, still closed.
        let d = design_of(vec![feature(1, Operation::Extrude { sketch: washer.into(), height_mm: 2.0, draft_deg: 5.0 })], vec![1]);
        let e = evaluate(&d, &lib, export).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let c = &e.components[0];
        assert!(c.mesh.validate().watertight && c.mesh.volume_mm3() < expected * 0.97, "{}", c.mesh.volume_mm3());
        // Two squares drawn as one sketch feature are two lumps of one body; drawn inline they are refused as before.
        let mut two = Sketch::default();
        for x in [0.0, 5.0] {
            let p = [[x, 0.0], [x + 2.0, 0.0], [x + 2.0, 2.0], [x, 2.0]].map(|p| two.point(p));
            two.entity(Geometry::Polyline { points: p.to_vec(), closed: true });
        }
        let d = design_of(vec![feature(1, Operation::Sketch { sketch: two.clone() }), feature(2, extrude(Profile::Feature { feature: 1 }, 1.5))], vec![2]);
        let e = evaluate(&d, &lib, export).unwrap();
        let c = &e.components[0];
        assert_eq!(c.body.roots.len(), 2);
        assert!(c.body.validate().is_empty() && c.mesh.validate().watertight);
        assert!((c.mesh.volume_mm3() - 12.0).abs() < 1e-6, "{}", c.mesh.volume_mm3());
        let d = design_of(vec![feature(1, extrude(two.into(), 1.5))], vec![1]);
        let error = evaluate(&d, &lib, export).unwrap().first_error().unwrap();
        assert!(error.contains("2 separate loops"), "{error}");
        // Revolved, a square with a square hole keeps a void on a full turn and opens through both caps on a half.
        let mut frame = Sketch { plane: Workplane::section(), ..Sketch::default() };
        for (lo, hi) in [([2.0, 0.0], [5.0, 4.0]), ([3.0, 1.0], [4.0, 3.0])] {
            let p = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]].map(|p| frame.point(p));
            frame.entity(Geometry::Polyline { points: p.to_vec(), closed: true });
        }
        for (degrees, share) in [(360.0, 1.0), (180.0, 0.5)] {
            let op = Operation::Revolve { sketch: frame.clone().into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees };
            let e = evaluate(&design_of(vec![feature(1, op)], vec![1]), &lib, export).unwrap();
            assert!(e.failures().is_empty(), "{degrees}: {:?}", e.failures());
            let c = &e.components[0];
            let expected = 70.0 * PI * share;
            assert!(c.mesh.validate().watertight, "{degrees}");
            assert!((c.mesh.volume_mm3() / expected - 1.0).abs() < 0.005, "{degrees}: {} against {expected}", c.mesh.volume_mm3());
        }
    }

    #[test]
    fn a_face_a_sketch_lies_on_is_read_and_never_consumed() {
        let mut anchored = Sketch::rectangle(1.0, 1.0);
        anchored.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::bare(4) });
        let on_face = extrude(anchored.clone().into(), 1.0);
        assert_eq!((on_face.sources(), on_face.consumes()), (vec![1], vec![]));
        let sketch = Operation::Sketch { sketch: anchored.clone() };
        assert_eq!((sketch.sources(), sketch.consumes()), (vec![1], vec![]));
        let by_id = extrude(Profile::Feature { feature: 2 }, 1.0);
        assert_eq!((by_id.sources(), by_id.consumes()), (vec![2], vec![2]));
        let twist = Operation::Twist { sketch: Sketch::circle(0.5).into(), path: anchored, degrees: 90.0, end_scale: 1.0 };
        assert_eq!((twist.sources(), twist.consumes()), (vec![1], vec![]));
        let fillet = Operation::Fillet { source: 3, edges: vec![EdgeRef::bare(0)], radius_mm: 0.2 };
        assert_eq!(fillet.sources(), fillet.consumes());
        // The edit funnel reads the anchor: the box cannot go while a sketch stands on it.
        let mut doc = Document::default();
        let mut sketch = Sketch::circle(1.0);
        sketch.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::bare(4) });
        doc.append(feature(1, Operation::Box { size: [4.0; 3] })).unwrap();
        doc.append(feature(2, Operation::Sketch { sketch })).unwrap();
        doc.append(feature(3, extrude(Profile::Feature { feature: 2 }, 1.0))).unwrap();
        assert_eq!(doc.dependents(1), vec![2, 3]);
        let error = doc.apply(&edit::CadEdit::Remove { id: 1 }).unwrap_err().to_string();
        assert!(error.contains("2 depend on it"), "{error}");
        assert_eq!(doc.outputs, vec![1, 3], "the box stays a part beside what stands on it");
    }

    #[test]
    fn every_edit_keeps_the_box_a_sketch_stands_on_among_the_outputs() {
        use edit::CadEdit;
        let lib = AlphaLibrary::builtin();
        let params = BuildParams::default();
        let seat = Placement::ring(90.0, 0.0);
        let mut block = feature(1, Operation::Box { size: [8.0, 6.0, 2.0] });
        block.component.placement = seat.clone();
        let d = design_of(vec![block.clone()], vec![1]);
        let body = evaluate(&d, &lib, params).unwrap().components.remove(0).body;
        let top = face_along(&body, &seat.frame(&d).unwrap(), [0.0, 0.0, 1.0]);
        let mut circle = Sketch::circle(1.0);
        circle.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::signed(&body, top, &seat.frame(&d).unwrap()) });
        let post = feature(2, extrude(circle.clone().into(), 1.0));
        let mut doc = Document::default();
        doc.apply(&CadEdit::Add { feature: block, after: None }).unwrap();
        doc.apply(&CadEdit::Add { feature: post, after: None }).unwrap();
        assert_eq!(doc.outputs, vec![1, 2], "added through the funnel");
        let parts = |doc: &Document| {
            let d = RingDesign { cad: Some(doc.clone()), ..RingDesign::default() };
            let e = evaluate(&d, &lib, params).unwrap();
            assert!(e.failures().is_empty(), "{:?}", e.failures());
            e.components.iter().map(|c| c.id).collect::<Vec<_>>()
        };
        assert_eq!(parts(&doc), vec![1, 2]);
        doc.apply(&CadEdit::Enable { id: 2, enabled: false }).unwrap();
        assert_eq!(parts(&doc), vec![1], "a suppressed post passes nothing through, and the box is still there");
        doc.apply(&CadEdit::Enable { id: 2, enabled: true }).unwrap();
        doc.apply(&CadEdit::Through { through: Some(2) }).unwrap();
        assert_eq!(parts(&doc), vec![1, 2], "rolled back to the post");
        doc.apply(&CadEdit::Through { through: None }).unwrap();
        // A post that consumed the box by a boolean gives it back when it only stands on it again.
        doc.apply(&CadEdit::Add { feature: feature(3, Operation::Cylinder { radius_mm: 0.5, height_mm: 4.0 }), after: Some(1) }).unwrap();
        doc.apply(&CadEdit::Operation { id: 2, operation: Operation::Boolean { a: 1, b: 3, kind: Boolean::Subtract } }).unwrap();
        assert_eq!(doc.outputs, vec![2]);
        doc.apply(&CadEdit::Operation { id: 2, operation: extrude(circle.into(), 1.0) }).unwrap();
        assert_eq!(doc.outputs, vec![2, 1, 3]);
        doc.apply(&CadEdit::Remove { id: 2 }).unwrap();
        assert_eq!(doc.outputs, vec![1, 3], "removing what stands on the box leaves the box");
    }
}
