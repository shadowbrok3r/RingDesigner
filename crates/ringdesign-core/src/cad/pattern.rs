//! Patterns of a part as its tessellation carried by rigid motions, work planes, and press-pull.
use super::{
    BuildCtx, Built, Document, EvaluatedComponent, FaceRef, Feature, Operation, Placement, Scope, SurfaceKind, Tessellated, Value, builders::Made, resolve_face, surface_hit, tessellated,
};
use crate::{
    Mesh, RingDesign,
    csg::{self, Solid},
    setting::Named,
    sketch::{FaceFrame, Id, Sketch},
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use cadkernel::brep::{self, Body};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// The key a pattern's copies carry as a mesh.
pub const PATTERN: &str = "pattern";
/// Most instances a pattern holds, its source's own among them.
pub const MAX_PATTERN_COUNT: u32 = 120;
/// Most faces a pattern's copies carry together.
pub const MAX_PATTERN_FACES: usize = 2_000_000;
/// Shortest press-pull either way, mm.
pub const MIN_PULL_MM: f64 = 1e-3;
/// Longest press-pull either way, mm.
pub const MAX_PULL_MM: f64 = 100.0;
/// Crease polylines a pattern keeps for picking and snapping.
const MAX_CREASES: usize = 2048;

fn full_turn() -> f64 {
    360.0
}

/// Where a pattern's copies stand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternKind {
    /// `count` instances round the finger's axis over `span_deg`, the source first; a whole turn steps `span / count`.
    Ring {
        count: u32,
        #[serde(default = "full_turn")]
        span_deg: f64,
    },
    /// `count` instances round the axis of `part`'s own frame, its z through its origin: a stone's table axis through its girdle.
    About {
        part: Id,
        count: u32,
        #[serde(default = "full_turn")]
        span_deg: f64,
    },
    /// One copy reflected across `plane`.
    Mirror { plane: MirrorPlane },
}

/// What a mirror reflects across.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorPlane {
    /// The band's mid-plane, z = 0: from one edge of the band to the other.
    Band,
    /// The plane through the finger's axis at `theta_deg`: round the ring to the far side of that angle, as through a head.
    Section { theta_deg: f64 },
    /// A work plane feature.
    Plane { feature: Id },
}

/// What a work plane is laid on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaneBase {
    /// Through the finger's axis at `theta_deg`: x out from the axis, y along the finger.
    Section { theta_deg: f64 },
    /// Square to the band's surface at a point: the normal out of it, x round the ring, y along the finger.
    Tangent {
        theta_deg: f64,
        #[serde(default)]
        across_mm: f64,
    },
    /// The parting plane, world z = 0, as the snaps read it.
    Parting,
    /// A planar face of a part, signed in the frame the build seated the part by.
    Face { feature: Id, face: FaceRef },
}

/// A work plane as built: its origin, its in-plane axes, and its normal `x × y`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct WorkPlane {
    pub id: Id,
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub normal: [f64; 3],
}

impl WorkPlane {
    /// The plane as a frame: x and y in it, z along its normal.
    pub fn placement(&self) -> brep::Placement {
        brep::Placement { x_axis: self.x, y_axis: self.y, z_axis: self.normal, origin: self.origin }
    }
}

impl PatternKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ring { .. } => "Ring array",
            Self::About { .. } => "Array round a part",
            Self::Mirror { .. } => "Mirror",
        }
    }
    /// The features the copies are placed by, beside the source: the part an array turns about, the plane a mirror reflects across.
    pub fn reads(&self) -> Vec<Id> {
        match self {
            Self::About { part, .. } => vec![*part],
            Self::Mirror { plane: MirrorPlane::Plane { feature } } => vec![*feature],
            Self::Ring { .. } | Self::Mirror { .. } => Vec::new(),
        }
    }
    /// Instances in all, the source's own among them.
    pub fn count(&self) -> u32 {
        match self {
            Self::Ring { count, .. } | Self::About { count, .. } => *count,
            Self::Mirror { .. } => 2,
        }
    }
    /// Each copy's turn in degrees, the source's own left out; refused past the counts and spans a pattern takes.
    pub fn angles(&self) -> Result<Vec<f64>> {
        let (count, span) = match self {
            Self::Ring { count, span_deg } | Self::About { count, span_deg, .. } => (*count, *span_deg),
            Self::Mirror { .. } => return Ok(Vec::new()),
        };
        ensure!((2..=MAX_PATTERN_COUNT).contains(&count), "A pattern holds 2 to {MAX_PATTERN_COUNT} instances, its source among them, not {count}");
        ensure!(span.is_finite() && span.abs() > 1e-6 && span.abs() <= 360.0 + 1e-9, "A pattern spans more than 0° and at most 360° either way");
        let step = if span.abs() >= 360.0 - 1e-9 { span / f64::from(count) } else { span / f64::from(count - 1) };
        Ok((1..count).map(|k| step * f64::from(k)).collect())
    }
}

impl PlaneBase {
    /// The feature a plane lies on, when it lies on a part's face.
    pub fn reads(&self) -> Vec<Id> {
        match self {
            Self::Face { feature, .. } => vec![*feature],
            _ => Vec::new(),
        }
    }
}

type Motion = brep::Placement;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l.is_finite() && l > 1e-12).then(|| v.map(|c| c / l))
}

/// `a` after `b`.
pub fn then(a: &Motion, b: &Motion) -> Motion {
    Motion { x_axis: a.vector(b.x_axis), y_axis: a.vector(b.y_axis), z_axis: a.vector(b.z_axis), origin: a.point(b.origin) }
}

/// The motion undoing a rigid one, a reflection included.
pub fn inverse(m: &Motion) -> Motion {
    let row = |k: usize| [m.x_axis[k], m.y_axis[k], m.z_axis[k]];
    let back = |axis: [f64; 3]| -dot(axis, m.origin);
    Motion { x_axis: row(0), y_axis: row(1), z_axis: row(2), origin: [back(m.x_axis), back(m.y_axis), back(m.z_axis)] }
}

/// A right-handed turn of `deg` about the line through `origin` along `axis`.
pub fn turn_about(origin: [f64; 3], axis: [f64; 3], deg: f64) -> Motion {
    let k = unit(axis).unwrap_or([0.0, 0.0, 1.0]);
    let (s, c) = deg.to_radians().sin_cos();
    let rot = |v: [f64; 3]| -> [f64; 3] {
        let (kv, kxv) = (dot(k, v), cross(k, v));
        std::array::from_fn(|i| v[i] * c + kxv[i] * s + k[i] * kv * (1.0 - c))
    };
    let turned = rot(origin);
    Motion { x_axis: rot([1.0, 0.0, 0.0]), y_axis: rot([0.0, 1.0, 0.0]), z_axis: rot([0.0, 0.0, 1.0]), origin: std::array::from_fn(|i| origin[i] - turned[i]) }
}

/// The reflection across the plane through `origin` square to `normal`.
pub fn reflect(origin: [f64; 3], normal: [f64; 3]) -> Motion {
    let n = unit(normal).unwrap_or([0.0, 0.0, 1.0]);
    let m = |v: [f64; 3]| -> [f64; 3] {
        let d = dot(v, n);
        std::array::from_fn(|i| v[i] - 2.0 * d * n[i])
    };
    let off = 2.0 * dot(origin, n);
    Motion { x_axis: m([1.0, 0.0, 0.0]), y_axis: m([0.0, 1.0, 0.0]), z_axis: m([0.0, 0.0, 1.0]), origin: n.map(|v| v * off) }
}

/// The Ring placement seating `id` and the feature carrying it, through modifiers, band booleans and settings; a stone on a part's face has none.
pub fn seat_of(doc: &Document, id: Id) -> Option<(Id, Placement)> {
    let band = doc.band();
    let mut at = id;
    for _ in 0..32 {
        let f = doc.feature(at)?;
        if matches!(f.component.placement, Placement::Ring { .. }) {
            return Some((at, f.component.placement.clone()));
        }
        at = match &f.operation {
            Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } | Operation::PressPull { source, .. } => *source,
            Operation::Builder { key, on: Some(_), .. } if key == super::builders::STONE => return None,
            Operation::Builder { on: Some(stone), .. } => *stone,
            Operation::Stored { sources, .. } => *sources.first()?,
            Operation::Boolean { a, b, .. } if band.is_some_and(|x| x == *a || x == *b) => {
                if band == Some(*a) {
                    *b
                } else {
                    *a
                }
            }
            _ => return None,
        };
    }
    None
}

/// The reflection a mirror plane stands for; a work plane's frame comes from `frame_of`.
fn mirror_of(plane: &MirrorPlane, frame_of: &dyn Fn(Id) -> Option<Motion>) -> Result<Motion> {
    match plane {
        MirrorPlane::Band => Ok(reflect([0.0; 3], [0.0, 0.0, 1.0])),
        MirrorPlane::Section { theta_deg } => {
            ensure!(theta_deg.is_finite(), "A mirror through the finger's axis needs an angle");
            let (s, c) = theta_deg.to_radians().sin_cos();
            Ok(reflect([0.0; 3], [-s, c, 0.0]))
        }
        MirrorPlane::Plane { feature } => {
            let p = frame_of(*feature).ok_or_else(|| anyhow!("Work plane #{feature} is unavailable or suppressed"))?;
            Ok(reflect(p.origin, p.z_axis))
        }
    }
}

/// The world motions carrying a part onto each copy, before a seated source drops onto the band again.
pub fn world_motions(kind: &PatternKind, frame_of: &dyn Fn(Id) -> Option<Motion>) -> Result<Vec<Motion>> {
    match kind {
        PatternKind::Ring { .. } => Ok(kind.angles()?.into_iter().map(|a| turn_about([0.0; 3], [0.0, 0.0, 1.0], a)).collect()),
        PatternKind::About { part, .. } => {
            let f = frame_of(*part).ok_or_else(|| anyhow!("Part #{part} stands free of the ring and of any stone; array round a stone or a seated part"))?;
            Ok(kind.angles()?.into_iter().map(|a| turn_about(f.origin, f.z_axis, a)).collect())
        }
        PatternKind::Mirror { plane } => Ok(vec![mirror_of(plane, frame_of)?]),
    }
}

/// The motions carrying the placed source onto each copy, a `seat`ed source dropped onto `surface` at each.
pub fn motions(kind: &PatternKind, design: &RingDesign, surface: Option<&Mesh>, seat: Option<(&Placement, &Motion)>, frame_of: &dyn Fn(Id) -> Option<Motion>) -> Result<Vec<Motion>> {
    motions_seated(kind, design, &|p: &Placement| p.frame_on(design, surface), seat, frame_of)
}

/// [`motions`] with each copy's seat read by `seated` rather than off a built surface.
pub fn motions_seated(
    kind: &PatternKind,
    design: &RingDesign,
    seated: &dyn Fn(&Placement) -> Result<Motion>,
    seat: Option<(&Placement, &Motion)>,
    frame_of: &dyn Fn(Id) -> Option<Motion>,
) -> Result<Vec<Motion>> {
    let Some((p, used)) = seat else { return world_motions(kind, frame_of) };
    let Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } = *p else { return world_motions(kind, frame_of) };
    let back = inverse(used);
    match kind {
        PatternKind::Ring { .. } => kind
            .angles()?
            .into_iter()
            .map(|a| {
                let at = Placement::Ring { theta_deg: theta_deg + a, across_mm, height_mm, spin_deg, tilt_deg, cant_deg };
                Ok(then(&seated(&at)?, &back))
            })
            .collect(),
        PatternKind::Mirror { plane: plane @ (MirrorPlane::Band | MirrorPlane::Section { .. }) } => {
            let (theta, across) = match plane {
                MirrorPlane::Section { theta_deg: through } => (2.0 * through - theta_deg, across_mm),
                _ => (theta_deg, -across_mm),
            };
            let q = Placement::Ring { theta_deg: theta, across_mm: across, height_mm, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
            let local = then(&inverse(&q.frame(design)?), &then(&mirror_of(plane, frame_of)?, &p.frame(design)?));
            Ok(vec![then(&seated(&q)?, &then(&local, &back))])
        }
        _ => world_motions(kind, frame_of),
    }
}

/// How near a face's own boundary a foot still stands on it, in the face plane's units, mm.
const ON_FACE_MM: f64 = 1e-6;

/// A planar face of a part as built, with its boundary in its own plane: what a copy or a mark standing on the face stays inside.
#[derive(Clone, Debug)]
pub struct FaceOutline {
    profile: brep::PlanarFaceProfile,
}

impl FaceOutline {
    /// Face `face` of `body`, a part seated by `frame`, found again by signature.
    fn of(body: &Body, frame: &brep::Placement, face: &FaceRef, notes: &mut Vec<String>) -> Result<Self> {
        let key = resolve_face(body, face, frame, notes).context("The face a stone stands on")?;
        let profile = brep::planar_face_profile(body, key).ok_or_else(|| anyhow!("Face {} has no boundary the kernel can read", face.ordinal))?;
        Ok(Self { profile })
    }

    /// The face a stone on `seat` stands on, of part `c` as built.
    pub fn of_seat(seat: &super::FaceSeat, c: &EvaluatedComponent) -> Result<Self> {
        if let Some(m) = &c.made {
            bail!("A stone sits on a kernel part's planar face; #{} {} is {}", c.id, c.name, Value::mesh_words(m));
        }
        Self::of(&c.body, &c.frame, &seat.face, &mut Vec::new())
    }

    /// Whether `p`, dropped along the face's normal onto its plane, falls within its boundary and outside its holes; on the edge is on it.
    pub fn holds(&self, p: [f64; 3]) -> bool {
        use cadkernel::geom2d::{Tolerance, contains, distance_to};
        let Some(uv) = self.profile.plane.project(p) else { return false };
        let tol = Tolerance::new(ON_FACE_MM);
        let mut loops = self.profile.loops.iter();
        let inside = loops.next().is_some_and(|outer| contains(outer, uv, tol));
        inside && !loops.any(|hole| contains(hole, uv, tol) && !hole.iter().any(|c| distance_to(c, uv) <= ON_FACE_MM))
    }
}

/// One copy of a pattern: the motion carrying its source there, its turn, and whether it is left out for standing off its face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub motion: Motion,
    /// Degrees round the ring or the part it turns about from the source; 0 for a mirror.
    pub angle_deg: f64,
    /// Its foot falls off the face the stone it carries stands on: the copy is left out.
    pub off_face: bool,
}

/// Every copy of `kind` carried by `motions`, none of them off a face.
fn instances(kind: &PatternKind, motions: Vec<Motion>) -> Result<Vec<Instance>> {
    let angles = if matches!(kind, PatternKind::Mirror { .. }) { vec![0.0; motions.len()] } else { kind.angles()? };
    Ok(motions.into_iter().zip(angles).map(|(motion, angle_deg)| Instance { motion, angle_deg, off_face: false }).collect())
}

/// Each copy of a ring array of a part in `frame`, a stone on `face` or what is built round it: its motion, and whether its foot, the stone's
/// foot on the face carried there, falls outside `outline`.
pub fn face_instances(kind: &PatternKind, frame: &Motion, face: &FaceFrame, outline: &FaceOutline) -> Result<Vec<Instance>> {
    let n = face.normal;
    let height = dot(std::array::from_fn(|k| frame.origin[k] - face.origin[k]), n);
    let foot: [f64; 3] = std::array::from_fn(|k| frame.origin[k] - n[k] * height);
    let angles = kind.angles()?;
    Ok(face_motions(kind, frame, face)?
        .into_iter()
        .zip(angles)
        .map(|(motion, angle_deg)| Instance { motion, angle_deg, off_face: !outline.holds(motion.point(foot)) })
        .collect())
}

/// What a copy left out for standing off its face is said as, on the pattern's status and under its ghost.
pub fn off_face_note(i: &Instance, host: &str) -> String {
    format!("The copy {:.1}° round the ring stands off the face of {host} it would be dropped onto: left out", i.angle_deg)
}

/// The motions carrying a part in `frame`, a stone on `face` or what is built round it, onto each copy of a ring array:
/// each copy turned round the finger's axis and dropped along the face's normal back onto it, its bearing kept in the face.
pub fn face_motions(kind: &PatternKind, frame: &Motion, face: &FaceFrame) -> Result<Vec<Motion>> {
    let n = face.normal;
    let off = |p: [f64; 3]| dot(std::array::from_fn(|k| p[k] - face.origin[k]), n);
    let height = off(frame.origin);
    let back = inverse(frame);
    kind.angles()?
        .into_iter()
        .map(|a| {
            let turn = turn_about([0.0; 3], [0.0, 0.0, 1.0], a);
            let o = turn.point(frame.origin);
            let drop = off(o) - height;
            let x = turn.vector(frame.x_axis);
            let along = dot(x, n);
            let x = unit(std::array::from_fn(|k| x[k] - n[k] * along)).ok_or_else(|| anyhow!("A copy {a:.1}° round the ring would stand on edge to the face it is dropped onto"))?;
            let at = Motion { x_axis: x, y_axis: cross(n, x), z_axis: n, origin: std::array::from_fn(|k| o[k] - n[k] * drop) };
            Ok(then(&at, &back))
        })
        .collect()
}

/// Every copy a pattern of `source` would stand, as the evaluation places them, read off `e`, the parts as built on `surface`: those it leaves
/// out for standing off their face among them.
pub fn copy_instances(design: &RingDesign, surface: Option<&Mesh>, e: &super::Evaluated, source: Id, kind: &PatternKind) -> Result<Vec<Instance>> {
    let doc = design.cad.as_ref().context("No CAD features")?;
    let frame_of = |id: Id| e.components.iter().find(|c| c.id == id).map(|c| c.frame).or_else(|| e.planes.iter().find(|p| p.id == id).map(WorkPlane::placement));
    if let (PatternKind::Ring { .. }, Some((stone, part, seat))) = (kind, super::face_stone(doc, source)) {
        let c = e.components.iter().find(|c| c.id == part).ok_or_else(|| anyhow!("Part #{part} a stone stands on did not build"))?;
        let at = frame_of(stone).ok_or_else(|| anyhow!("Stone #{stone} did not build"))?;
        return face_instances(kind, &at, &seat.face_of(c)?, &FaceOutline::of_seat(&seat, c)?);
    }
    let seat = seat_of(doc, source).map(|(_, p)| p.frame_on(design, surface).map(|used| (p, used))).transpose()?;
    instances(kind, motions(kind, design, surface, seat.as_ref().map(|(p, used)| (p, used)), &frame_of)?)
}

/// The motions a pattern of `source` carries it by, as the evaluation places its copies, read off `e`, the parts as built on `surface`:
/// a copy left out for standing off its face has none.
pub fn copy_motions(design: &RingDesign, surface: Option<&Mesh>, e: &super::Evaluated, source: Id, kind: &PatternKind) -> Result<Vec<Motion>> {
    Ok(copy_instances(design, surface, e, source, kind)?.into_iter().filter(|i| !i.off_face).map(|i| i.motion).collect())
}

/// A pattern of every one of `sources`, carried by the motions the first one's seat gives, as one mesh part.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_sources(
    f: &Feature,
    sources: &[Id],
    kind: &PatternKind,
    design: &RingDesign,
    ctx: &BuildCtx,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    who: &dyn Fn(Id) -> String,
    scope: &Scope,
) -> Result<Built> {
    let Some(&source) = sources.first() else { bail!("A pattern copies at least one part") };
    ensure!(sources.len() <= MAX_PATTERN_SOURCES, "A pattern copies at most {MAX_PATTERN_SOURCES} parts together");
    ensure!(f.component.placement == Placement::Free, "A pattern stands where its copies do; move {} to move them", who(source));
    if let PatternKind::Mirror { plane: MirrorPlane::Plane { feature } } = kind {
        ensure!(
            scope.doc.feature(*feature).is_some_and(|p| matches!(p.operation, Operation::Plane { .. })),
            "{} is not a work plane; a mirror reflects across one",
            who(*feature)
        );
    }
    let mut parts = Vec::with_capacity(sources.len());
    for (k, &id) in sources.iter().enumerate() {
        ensure!(!sources[..k].contains(&id), "{} is named twice among the parts a pattern copies", who(id));
        if scope.doc.feature(id).is_some_and(|s| !s.operation.has_body()) {
            bail!("{} has no body to copy", who(id));
        }
        let value = values.get(&id).ok_or_else(|| anyhow!("Source feature #{id} is unavailable or suppressed"))?;
        parts.push((id, value));
    }
    let frame_of = |id: Id| frames.get(&id).copied();
    let mut notes = Vec::new();
    // A stone on a part's face, and what is built round it, is dropped back onto that face, a copy whose foot falls off it left out
    // and named; a seated source is dropped onto the band.
    let motions = match (kind, super::face_stone(scope.doc, source)) {
        (PatternKind::Ring { .. }, Some((stone, part, seat))) => {
            let (face, part_frame) = super::stood_face(&seat, part, values, frames, who, &mut notes)?;
            let at = frames.get(&stone).copied().ok_or_else(|| anyhow!("{} has no seat to array from", who(stone)))?;
            let Some(Value::Brep(body)) = values.get(&part) else { bail!("{} has no face a stone can stand on", who(part)) };
            let outline = FaceOutline::of(body, &part_frame, &seat.face, &mut Vec::new())?;
            let (kept, off): (Vec<Instance>, Vec<Instance>) = face_instances(kind, &at, &face, &outline)?.into_iter().partition(|i| !i.off_face);
            notes.extend(off.iter().map(|i| off_face_note(i, &who(part))));
            ensure!(!kept.is_empty(), "Every copy of {} stands off the face of {} it would be dropped onto; take a smaller span or fewer copies", who(source), who(part));
            kept.into_iter().map(|i| i.motion).collect()
        }
        _ => {
            let seat = match seat_of(scope.doc, source) {
                Some((_, p)) => Some((p.frame_on(design, ctx.surface)?, p)),
                None => None,
            };
            motions(kind, design, ctx.surface, seat.as_ref().map(|(used, p)| (p, used)), &frame_of)?
        }
    };
    let mut tessellations = Vec::with_capacity(parts.len());
    for (id, value) in parts {
        tessellations.push((id, tessellated(value, scope.sigs.get(&id).copied().unwrap_or_default(), scope.bucket, scope.chord, scope.memo)?, value));
    }
    let faces = tessellations.iter().map(|(_, t, _)| t.mesh.faces.len()).sum::<usize>() * motions.len();
    ensure!(faces <= MAX_PATTERN_FACES, "{} copies of {} would carry {faces} faces, past {MAX_PATTERN_FACES}; take fewer copies or a lighter part", motions.len(), who(source));
    let sources: Vec<(Option<Id>, &Tessellated, &Value)> =
        tessellations.iter().map(|(id, t, v)| ((tessellations.len() > 1).then_some(*id), t.as_ref(), *v)).collect();
    let made = copies(&sources, &motions, kind, &mut notes)?;
    Ok(Built { value: Value::Mesh(Arc::new(made)), frame: None, attach: None, notes })
}

/// Most parts one pattern copies together.
pub const MAX_PATTERN_SOURCES: usize = 16;

/// The parts a pattern copies: read from one `source` and any `sources`, written as one `source` whenever there is only one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "SourcesWire", into = "SourcesWire")]
pub struct Sources(pub Vec<Id>);

#[derive(Serialize, Deserialize)]
struct SourcesWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sources: Vec<Id>,
}

impl From<SourcesWire> for Sources {
    fn from(w: SourcesWire) -> Self {
        Self(w.source.into_iter().chain(w.sources).collect())
    }
}

impl From<Sources> for SourcesWire {
    fn from(s: Sources) -> Self {
        match s.0.as_slice() {
            [one] => Self { source: Some(*one), sources: Vec::new() },
            _ => Self { source: None, sources: s.0 },
        }
    }
}

/// Whether `design` carries a pattern of several parts, in its document or anywhere in its graph.
pub fn several_sources(design: &RingDesign) -> bool {
    let in_document = design.cad.as_ref().is_some_and(|doc| doc.features.iter().any(|f| matches!(&f.operation, Operation::Pattern { sources, .. } if sources.len() > 1)));
    in_document || design.graph.as_ref().is_some_and(several_sources_json)
}

/// Whether `v` holds a pattern of several parts anywhere: an object keyed `Pattern` whose `source` and `sources` name more than one.
pub fn several_sources_json(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(map) => {
            let pattern = map.get("Pattern");
            let one = pattern.and_then(|p| p.get("source")).is_some_and(|s| !s.is_null());
            let listed = pattern.and_then(|p| p.get("sources")).and_then(serde_json::Value::as_array).map_or(0, Vec::len);
            let many = usize::from(one) + listed > 1;
            many || map.values().any(several_sources_json)
        }
        serde_json::Value::Array(items) => items.iter().any(several_sources_json),
        _ => false,
    }
}

impl Sources {
    /// The part whose seat the copies are carried from.
    pub fn first(&self) -> Option<Id> {
        self.0.first().copied()
    }
}

impl From<Id> for Sources {
    fn from(id: Id) -> Self {
        Self(vec![id])
    }
}

impl std::ops::Deref for Sources {
    type Target = [Id];
    fn deref(&self) -> &[Id] {
        &self.0
    }
}

/// Named solids joined end to end, each keeping its patches.
fn concat(pieces: Vec<Named>) -> Named {
    let mut out = Named::default();
    for p in pieces {
        let base = out.names.len() as u32;
        out.solid.push(&p.solid);
        out.patch.extend(p.patch.iter().map(|i| i + base));
        out.names.extend(p.names);
    }
    out
}

/// Each source's tessellation carried by each motion as one named mesh, reflections wound outward; a source's id
/// prefixes its patches when there are several.
fn copies(sources: &[(Option<Id>, &Tessellated, &Value)], motions: &[Motion], kind: &PatternKind, notes: &mut Vec<String>) -> Result<Made> {
    let label = |k: usize| match kind {
        PatternKind::Mirror { .. } => "Mirror".to_string(),
        _ => format!("Copy {}", k + 1),
    };
    let mut kind_of: HashMap<String, SurfaceKind> = HashMap::new();
    let mut pieces = Vec::with_capacity(motions.len() * sources.len());
    let mut labels = Vec::with_capacity(pieces.capacity());
    let mut creases = Vec::new();
    let mut stations = Vec::new();
    for (k, m) in motions.iter().enumerate() {
        let flip = m.reflects();
        for &(id, t, source) in sources {
            let seam = t.trace.face_kind.len() as u32;
            let (names, kinds): (Vec<String>, Vec<SurfaceKind>) = match source {
                Value::Mesh(m) => (m.named.names.clone(), m.kinds.clone()),
                Value::Brep(_) => (
                    (0..seam).map(|f| format!("face {f}")).chain(["seam".to_string()]).collect(),
                    t.trace.face_kind.iter().copied().chain([SurfaceKind::Freeform]).collect(),
                ),
            };
            let patch: Vec<u32> = match source {
                Value::Mesh(m) => m.named.patch.clone(),
                Value::Brep(_) => (0..t.mesh.faces.len()).map(|tri| t.trace.face_of(tri).unwrap_or(seam)).collect(),
            };
            let solid = Solid {
                v: t.trace.positions.iter().map(|p| m.point(*p)).collect(),
                f: t.mesh.faces.iter().map(|f| if flip { [f[0], f[2], f[1]] } else { *f }).collect(),
            };
            let piece = match id {
                Some(id) => format!("{}, #{id}", label(k)),
                None => label(k),
            };
            let own: Vec<String> = names.iter().map(|n| format!("{piece}, {n}")).collect();
            for (n, s) in own.iter().zip(&kinds) {
                kind_of.insert(n.clone(), *s);
            }
            pieces.push(Named { solid, patch, names: own });
            labels.push(piece);
            creases.extend(t.edges.iter().map(|l| l.iter().map(|p| m.point(*p)).collect::<Vec<_>>()));
            if let Value::Mesh(src) = source {
                stations.extend(src.stations.iter().map(|p| m.point(*p)));
            }
        }
    }
    // Copies whose boxes meet are united.
    let refs: Vec<&Solid> = pieces.iter().map(|p| &p.solid).collect();
    let groups = csg::cluster(&refs, 0.0);
    let mut united = Vec::with_capacity(groups.len());
    for group in groups {
        let mut acc = pieces[group[0]].clone();
        for &g in &group[1..] {
            match acc.clone().union(&pieces[g]) {
                Ok(u) => acc = u,
                Err(e) => {
                    notes.push(format!("{} overlaps the copies before it and would not unite with them ({e}); it stays a shell of its own", labels[g]));
                    united.push(pieces[g].clone());
                }
            }
        }
        united.push(acc);
    }
    let named = concat(united);
    let (open, repeated) = named.solid.open_edges();
    ensure!(open == 0 && repeated == 0 && !named.solid.is_empty(), "The copies did not close ({open} open edges, {repeated} repeated)");
    let kinds = named.names.iter().map(|n| kind_of.get(n).copied().unwrap_or(SurfaceKind::Freeform)).collect();
    creases.truncate(MAX_CREASES);
    // Every copy carries the stone its first source to hold one was made for.
    let gem = sources.iter().find_map(|(_, _, v)| v.made().and_then(|m| m.gem));
    Ok(Made { key: PATTERN.to_string(), named, kinds, creases, gem, seat: None, stations })
}

/// The work plane `base` names, moved `offset_mm` along its normal.
#[allow(clippy::too_many_arguments)]
pub(super) fn work_plane(
    id: Id,
    base: &PlaneBase,
    offset_mm: f64,
    design: &RingDesign,
    surface: Option<&Mesh>,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    notes: &mut Vec<String>,
) -> Result<WorkPlane> {
    ensure!(offset_mm.is_finite() && offset_mm.abs() <= 1000.0, "A work plane's offset must be a finite number of millimetres");
    let (origin, x, y, normal) = match base {
        PlaneBase::Section { theta_deg } => {
            ensure!(theta_deg.is_finite(), "A section plane needs an angle");
            let (s, c) = theta_deg.to_radians().sin_cos();
            ([0.0; 3], [c, s, 0.0], [0.0, 0.0, 1.0], [s, -c, 0.0])
        }
        PlaneBase::Tangent { theta_deg, across_mm } => {
            ensure!(theta_deg.is_finite() && across_mm.is_finite(), "A plane square to the band needs a point on it");
            let (s, c) = theta_deg.to_radians().sin_cos();
            let (hit, n) = surface.and_then(|m| surface_hit(m, *theta_deg, *across_mm)).unwrap_or_else(|| {
                let r = design.inner_radius_mm() + design.profile.thickness_mm;
                ([r * c, r * s, *across_mm], [c, s, 0.0])
            });
            let round = [-s, c, 0.0];
            let d = dot(round, n);
            let x = unit(std::array::from_fn(|k| round[k] - n[k] * d)).context("The band has no direction round the ring there")?;
            (hit, x, cross(n, x), n)
        }
        PlaneBase::Parting => ([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        PlaneBase::Face { feature, face } => {
            let body = match values.get(feature) {
                Some(Value::Brep(body)) => body,
                Some(Value::Mesh(m)) => bail!("Work plane face: feature #{feature} is {}; a work plane lies on a kernel part's planar face", Value::mesh_words(m)),
                None => bail!("Work plane face: feature #{feature} is unavailable or suppressed"),
            };
            let frame = frames.get(feature).copied().unwrap_or(brep::Placement::IDENTITY);
            let key = resolve_face(body, face, &frame, notes).context("Work plane face")?;
            let kind = SurfaceKind::of(body.faces.get(key).and_then(|f| body.surfaces.get(f.surface)));
            ensure!(
                kind == SurfaceKind::Plane,
                "Work plane face {} of feature #{feature}: a work plane lies on a planar face, and this one is a {}",
                face.ordinal,
                format!("{kind:?}").to_lowercase()
            );
            let profile = brep::planar_face_profile(body, key).ok_or_else(|| anyhow!("Work plane face {} of feature #{feature} has no boundary the kernel can read", face.ordinal))?;
            let frame = FaceFrame::of(&profile).context("Work plane face")?;
            (frame.origin, frame.x, frame.y, frame.normal)
        }
    };
    Ok(WorkPlane { id, origin: std::array::from_fn(|k| origin[k] + normal[k] * offset_mm), x, y, normal })
}

/// The world plane `sketch` lies on when it is laid on the work plane whose frame is `plane`.
pub(super) fn on_plane(sketch: &Sketch, plane: &brep::Placement) -> Result<cadkernel::space::Plane> {
    sketch.plane.on_frame(&FaceFrame { origin: plane.origin, x: plane.x_axis, y: plane.y_axis, normal: plane.z_axis })
}

/// The world plane `sketch` lies on when it is laid on `plane`: what a canvas draws it in.
pub fn sketch_on_work_plane(sketch: &Sketch, plane: &WorkPlane) -> Result<cadkernel::space::Plane> {
    on_plane(sketch, &plane.placement())
}

/// Planar face `face` of a part signed in its seated frame, with its world centre and outward normal.
pub fn planar_face(c: &EvaluatedComponent, face: u32) -> Result<(FaceRef, [f64; 3], [f64; 3])> {
    if let Some(m) = &c.made {
        bail!("Press-pull moves a kernel part's faces; #{} {} is {}", c.id, c.name, Value::mesh_words(m));
    }
    let body = &c.body;
    let key = body.faces.iter().nth(face as usize).map(|(k, _)| k).ok_or_else(|| anyhow!("#{} {} has no face {face}", c.id, c.name))?;
    let kind = SurfaceKind::of(body.faces.get(key).and_then(|f| body.surfaces.get(f.surface)));
    ensure!(kind == SurfaceKind::Plane, "Press-pull moves planar faces; face {face} of #{} {} is a {}", c.id, c.name, format!("{kind:?}").to_lowercase());
    let profile = brep::planar_face_profile(body, key).ok_or_else(|| anyhow!("Face {face} of #{} {} has no boundary the kernel can read", c.id, c.name))?;
    let frame = FaceFrame::of(&profile).context("Press-pull face")?;
    Ok((FaceRef::signed(body, face as usize, &c.frame), frame.origin, frame.normal))
}

/// `body` with a planar face moved `distance` along its outward normal, or a prism added or cut on it.
pub(super) fn press_pull(body: &Body, face: &FaceRef, distance: f64, frame: &brep::Placement, notes: &mut Vec<String>) -> Result<Body> {
    ensure!(
        distance.is_finite() && distance.abs() >= MIN_PULL_MM && distance.abs() <= MAX_PULL_MM,
        "A press-pull moves its face {MIN_PULL_MM} to {MAX_PULL_MM} mm either way"
    );
    let key = resolve_face(body, face, frame, notes).context("Press-pull face")?;
    let kind = SurfaceKind::of(body.faces.get(key).and_then(|f| body.surfaces.get(f.surface)));
    ensure!(kind == SurfaceKind::Plane, "Press-pull face {}: press-pull moves planar faces, and this one is a {}", face.ordinal, format!("{kind:?}").to_lowercase());
    if let Some(moved) = brep::presspull_face(body, key, distance, brep::PresspullMode::Offset) {
        return Ok(moved);
    }
    let prism = brep::presspull_face(body, key, distance, brep::PresspullMode::Extrude)
        .ok_or_else(|| anyhow!("Press-pull cannot move face {} by {distance:.3} mm: the kernel could neither move it nor stand a prism on it", face.ordinal))?;
    notes.push(format!(
        "Face {}'s neighbours could not be carried with it; a prism {:.3} mm deep was {} instead",
        face.ordinal,
        distance.abs(),
        if distance > 0.0 { "added" } else { "cut" }
    ));
    Ok(prism)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{
        self, Attach, Cache, Component, ComponentRole, EdgeRef, Evaluated, EvaluatedComponent, FeatureStatus, Memo, Profile, builders, edge_signature, edit::CadEdit, evaluate,
        face_signature,
    };
    use crate::gem::{Gem, GemCut};
    use crate::sketch::FaceAnchor;
    use crate::{AlphaLibrary, BuildParams, templates};
    use cadkernel::brep::make;
    use std::f64::consts::PI;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;

    const IDENTITY: brep::Placement = brep::Placement::IDENTITY;

    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }
    }
    fn template(name: &str) -> RingDesign {
        templates::all().iter().find(|t| t.name == name).unwrap().design()
    }
    fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation, component }
    }
    fn joined(placement: Placement) -> Component {
        Component { attach: Attach::Join, placement, ..Component::default() }
    }
    fn band() -> Feature {
        feature(1, "Procedural shank", Operation::Band, Component { role: ComponentRole::Shank, ..Component::default() })
    }
    fn with(mut d: RingDesign, features: Vec<Feature>) -> RingDesign {
        let mut doc = Document::default();
        for f in features {
            doc.append(f).unwrap();
        }
        d.cad = Some(doc);
        d
    }
    /// The design's band built without its parts: the surface its parts drop onto.
    fn bare(d: &RingDesign, lib: &AlphaLibrary) -> Mesh {
        let mut plain = d.clone();
        plain.cad = None;
        crate::mesh::try_build(&plain, lib, params()).unwrap().mesh
    }
    fn on(d: &RingDesign, lib: &AlphaLibrary, surface: &Mesh) -> Evaluated {
        let never = AtomicBool::new(false);
        cad::evaluate_with(d, lib, params(), &BuildCtx::new(&never).with_surface(surface)).unwrap()
    }
    fn component(e: &Evaluated, id: Id) -> &EvaluatedComponent {
        e.components.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no part #{id}: {:?}", e.failures()))
    }
    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|k| a[k] - b[k])
    }
    fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
        dot(sub(a, b), sub(a, b)).sqrt()
    }
    fn centroid(points: &[[f64; 3]]) -> [f64; 3] {
        let n = points.len().max(1) as f64;
        std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f64>() / n)
    }
    /// The centroid of the vertices of every face whose patch is named `prefix…`: one copy of a pattern.
    fn copy_centroid(made: &Made, prefix: &str) -> [f64; 3] {
        let n = &made.named;
        let mut used = vec![false; n.solid.v.len()];
        for (f, p) in n.solid.f.iter().zip(&n.patch) {
            if n.names[*p as usize].starts_with(prefix) {
                for v in f {
                    used[*v as usize] = true;
                }
            }
        }
        let points: Vec<[f64; 3]> = n.solid.v.iter().zip(&used).filter(|(_, u)| **u).map(|(p, _)| *p).collect();
        assert!(!points.is_empty(), "no faces named {prefix}");
        centroid(&points)
    }
    /// Pieces of a mesh joined by shared vertices: one for a ring that holds together.
    fn pieces(m: &Mesh) -> usize {
        let mut root: Vec<usize> = (0..m.vertices.len()).collect();
        fn find(root: &mut [usize], mut i: usize) -> usize {
            while root[i] != i {
                root[i] = root[root[i]];
                i = root[i];
            }
            i
        }
        for f in &m.faces {
            for k in 1..3 {
                let (a, b) = (find(&mut root, f[0] as usize), find(&mut root, f[k] as usize));
                root[a.max(b)] = a.min(b);
            }
        }
        let used: std::collections::HashSet<usize> = m.faces.iter().flatten().map(|v| find(&mut root, *v as usize)).collect();
        used.len()
    }
    /// The planar face whose outward normal, in `frame`, lies along `dir`.
    fn face_along(body: &Body, frame: &brep::Placement, dir: [f64; 3]) -> usize {
        (0..body.faces.len()).find(|i| face_signature(body, *i, frame).is_some_and(|s| s.kind == SurfaceKind::Plane && dot(s.normal, dir) > 0.99)).unwrap()
    }
    fn failed(e: &Evaluated, id: Id) -> String {
        match e.status_of(id) {
            Some(FeatureStatus::Failed(why)) => why.clone(),
            other => panic!("#{id} did not fail: {other:?}"),
        }
    }

    #[test]
    fn a_pattern_reads_one_source_or_several_and_writes_one_as_it_always_did() {
        let kind = PatternKind::Ring { count: 3, span_deg: 360.0 };
        let old = r#"{"Pattern":{"source":3,"kind":{"ring":{"count":3,"span_deg":360.0}}}}"#;
        assert_eq!(serde_json::to_string(&Operation::Pattern { sources: 3.into(), kind: kind.clone() }).unwrap(), old, "one source writes the bytes it always did");
        let Operation::Pattern { sources, .. } = serde_json::from_str(old).unwrap() else { panic!() };
        assert_eq!(sources, Sources(vec![3]), "and an old file reads as one source");
        let two = serde_json::to_string(&Operation::Pattern { sources: Sources(vec![3, 4]), kind }).unwrap();
        assert!(two.starts_with(r#"{"Pattern":{"sources":[3,4],"kind""#), "{two}");
        let Operation::Pattern { sources, .. } = serde_json::from_str(&two).unwrap() else { panic!() };
        assert_eq!((&sources[..], sources.first()), (&[3, 4][..], Some(3)), "two sources round-trip");
        let both = r#"{"Pattern":{"source":3,"sources":[4],"kind":{"mirror":{"plane":"band"}}}}"#;
        let Operation::Pattern { sources, .. } = serde_json::from_str(both).unwrap() else { panic!() };
        assert_eq!(sources, Sources(vec![3, 4]));
    }

    #[test]
    fn a_head_and_its_halo_array_as_one_pattern_their_stones_with_them_and_the_file_says_so() {
        let lib = AlphaLibrary::builtin();
        let gem = Gem::calibrated(GemCut::Round, 5.0);
        let mut next = 2;
        let mut features = vec![band(), builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm("claw4", gem)))];
        features.extend(builders::setting_features("halo", 2, gem, false, &mut || { next += 1; next }).unwrap());
        let sources = Sources(features.iter().filter(|f| matches!(&f.operation, Operation::Builder { key, .. } if key != builders::STONE && key != builders::BUR)).map(|f| f.id).collect());
        assert_eq!(sources.len(), 2, "the head and the halo");
        let array = feature(9, "Heads and halos", Operation::Pattern { sources: sources.clone(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } }, joined(Placement::Free));
        features.push(array);
        let d = with(template("Court band"), features);
        let surface = bare(&d, &lib);
        let e = on(&d, &lib, &surface);
        let made = component(&e, 9).made.clone().unwrap();
        for id in sources.iter() {
            assert!(made.named.names.iter().any(|n| n.starts_with(&format!("Copy 2, #{id}, "))), "#{id} is copied");
        }
        let (melee, _) = builders::halo_melee(gem, &serde_json::json!({})).unwrap();
        let stones = crate::setstone::set_stones(&d);
        let per = 1 + stones.iter().filter(|s| s.gem == melee).count() / 3;
        assert_eq!(stones.len(), 3 * per, "each copy carries the centre and its melee");
        let text = crate::library::design_json(&d).unwrap();
        assert!(text.contains(r#""sources""#), "several sources are written as such");
        assert_eq!(crate::library::format_version_for(&d), crate::library::FORMAT_VERSION, "and fenced from builds that would read one");
        let back = crate::library::load_design_str(&text).unwrap();
        let Operation::Pattern { sources: again, .. } = &back.cad.as_ref().unwrap().feature(9).unwrap().operation else { panic!() };
        assert_eq!(again, &sources, "and reopen as they were");
        let mut single = d.clone();
        let Operation::Pattern { sources: s, .. } = &mut single.cad.as_mut().unwrap().features.last_mut().unwrap().operation else { panic!() };
        s.0.truncate(1);
        assert_eq!(crate::library::format_version_for(&single), crate::library::PLAIN_FORMAT_VERSION, "one source is the plain format");
    }

    #[test]
    fn two_sources_are_carried_by_one_motion_set_and_keep_their_stone() {
        let gem = Gem::calibrated(GemCut::Round, 5.0);
        let seat = builders::Seat { surface_z: -builders::stand_off_mm("claw4", gem), through_mm: None };
        let head = builders::build(builders::CLAW, gem, &serde_json::json!({ "prongs": 4 }), seat, None).unwrap();
        let halo = builders::build(builders::HALO, gem, &serde_json::json!({}), seat, None).unwrap();
        let (th, tl) = (Tessellated::of_made(&head), Tessellated::of_made(&halo));
        let (vh, vl) = (Value::Mesh(Arc::new(head.clone())), Value::Mesh(Arc::new(halo.clone())));
        let motions = [turn_about([0.0, -30.0, 0.0], [0.0, 0.0, 1.0], 0.0), turn_about([0.0, -30.0, 0.0], [0.0, 0.0, 1.0], 90.0)];
        let kind = PatternKind::Ring { count: 3, span_deg: 180.0 };
        let mut notes = Vec::new();
        let made = copies(&[(Some(3), &th, &vh), (Some(4), &tl, &vl)], &motions, &kind, &mut notes).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        for copy in ["Copy 1", "Copy 2"] {
            assert!(made.named.names.iter().any(|n| n == &format!("{copy}, #3, Claw 1")), "{copy}: the head");
            assert!(made.named.names.iter().any(|n| n == &format!("{copy}, #4, Halo rail")), "{copy}: the halo");
        }
        assert_eq!(made.stations.len(), 2 * halo.stations.len(), "each copy carries the halo's melee");
        assert_eq!(made.gem, Some(gem), "and the stone its head was made for");
        let one = copies(&[(None, &th, &vh)], &motions, &kind, &mut notes).unwrap();
        assert!(one.named.names.iter().any(|n| n == "Copy 2, Claw 1"), "one source names its patches as it always did");
        assert!(made.named.solid.volume() > one.named.solid.volume(), "the halo adds its own metal");
    }

    #[test]
    fn six_prongs_from_one_stand_round_the_stone_at_sixty_degree_steps_and_join_the_band_in_one_piece() {
        let lib = AlphaLibrary::builtin();
        let mut d = template("Court band");
        d.profile.width_mm = 8.0;
        d.profile.thickness_mm = 2.5;
        let surface = bare(&d, &lib);
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let stand = builders::stand_off_mm("claw4", gem);
        let stone = builders::stone_feature(2, gem, Placement::ring(90.0, stand));
        let girdle = component(&on(&with(d.clone(), vec![band(), stone.clone()]), &lib, &surface), 2).frame;
        // A post beside the girdle along the stone's axis, from 0.8 mm over the girdle to 1 mm into the band.
        let length = stand + 1.8;
        let centre: [f64; 3] = std::array::from_fn(|k| girdle.origin[k] + girdle.x_axis[k] * (gem.w_mm / 2.0 + 0.45) + girdle.z_axis[k] * (0.8 - length / 2.0));
        let d6 = with(
            d.clone(),
            vec![
                band(),
                stone,
                feature(3, "Post", Operation::Cylinder { radius_mm: 0.45, height_mm: length }, Component::default()),
                feature(4, "Prong", Operation::Transform { source: 3, translation: centre, rotation_deg: [-90.0, 0.0, 0.0] }, joined(Placement::Free)),
                feature(5, "Prongs", Operation::Pattern { sources: 4.into(), kind: PatternKind::About { part: 2, count: 6, span_deg: 360.0 } }, joined(Placement::Free)),
            ],
        );
        let e = on(&d6, &lib, &surface);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let axis = component(&e, 2).frame;
        let made = component(&e, 5).made.clone().unwrap();
        assert_eq!((made.key.as_str(), component(&e, 5).body.faces.len()), (PATTERN, 0), "copies are a mesh part");
        let about = |p: [f64; 3]| {
            let w = sub(p, axis.origin);
            let (u, v) = (dot(w, axis.x_axis), dot(w, axis.y_axis));
            (v.atan2(u).to_degrees(), u.hypot(v), dot(w, axis.z_axis))
        };
        let (a0, r0, h0) = about(centroid(&component(&e, 4).trace.positions));
        for k in 1..6 {
            let (ak, rk, hk) = about(copy_centroid(&made, &format!("Copy {k}, ")));
            let step = (ak - a0).rem_euclid(360.0);
            assert!((step - 60.0 * k as f64).abs() < 1e-6, "copy {k} stands {step}° round the stone");
            assert!((rk - r0).abs() < 1e-6 && (hk - h0).abs() < 1e-6, "copy {k}: {rk} {hk} against {r0} {h0}");
        }
        // Joined: one watertight piece of metal carrying the prong and its five copies; the stone is never metal.
        let bare_volume = crate::mesh::try_build(&d, &lib, params()).unwrap().report.volume_mm3;
        let built = crate::mesh::try_build(&d6, &lib, params()).unwrap();
        let v = &built.report.validation;
        assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!((built.parts.joined, built.parts.references, built.parts.features.clone()), (2, 1, vec![4, 5]));
        assert_eq!(pieces(&built.mesh), 1, "every prong reaches the band");
        let post = PI * 0.45 * 0.45 * length;
        let gained = built.report.volume_mm3 - bare_volume;
        eprintln!("six prongs: {gained:.3} mm³ gained against six whole posts' {:.3}, {} faces", 6.0 * post, built.mesh.faces.len());
        assert!(gained > 5.0 * post && gained < 6.0 * post, "{gained:.3} of {:.3}", 6.0 * post);
    }

    #[test]
    fn a_head_arrayed_three_times_round_the_ring_stands_its_stand_off_off_the_built_surface_at_each_copy() {
        let lib = AlphaLibrary::builtin();
        // A head by the corner of a signet's table arrayed onto its shank: each copy dropped onto the band at its own angle.
        let heart = template("Heart signet");
        let surface = bare(&heart, &lib);
        let h = 0.7;
        let seat = Placement::ring(65.0, h);
        let kind = PatternKind::Ring { count: 3, span_deg: 360.0 };
        let head = feature(2, "Head", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, Component { role: ComponentRole::Head, ..joined(seat.clone()) });
        let d = with(heart.clone(), vec![band(), head, feature(3, "Heads", Operation::Pattern { sources: 2.into(), kind: kind.clone() }, joined(Placement::Free))]);
        let e = on(&d, &lib, &surface);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let used = seat.frame_on(&d, Some(&surface)).unwrap();
        let moved = motions(&kind, &d, Some(&surface), Some((&seat, &used)), &|_| None).unwrap();
        assert_eq!(moved.len(), 2);
        let made = component(&e, 3).made.clone().unwrap();
        let source = centroid(&component(&e, 2).trace.positions);
        for (k, m) in moved.iter().enumerate() {
            let theta = 65.0 + 120.0 * (k + 1) as f64;
            let (hit, n) = cad::surface_hit(&surface, theta, 0.0).unwrap();
            let origin = m.point(used.origin);
            let stand_off = dot(sub(origin, hit), n);
            assert!((stand_off - h).abs() < 0.02, "copy {} at {theta}°: {stand_off:.4} off the surface", k + 1);
            assert!(dist(sub(origin, n.map(|v| v * h)), hit) < 0.02, "its foot is where the ray at {theta}° meets the band");
            assert!((hit[1].atan2(hit[0]).to_degrees().rem_euclid(360.0) - theta.rem_euclid(360.0)).abs() < 1e-3);
            // The evaluated copy is the source carried by that motion.
            assert!(dist(copy_centroid(&made, &format!("Copy {}, ", k + 1)), m.point(source)) < 1e-9);
            // Turned in the world alone it would not stand on the shank: the table is not the shank.
            let turned = turn_about([0.0; 3], [0.0, 0.0, 1.0], 120.0 * (k + 1) as f64).point(used.origin);
            assert!((dot(sub(turned, hit), n) - h).abs() > 0.1, "the signet's shank sits where the table does");
        }
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight && built.parts.notes.is_empty(), "{:?} {:?}", built.report.validation, built.parts.notes);
        assert_eq!(built.parts.joined, 2);
        // A claw head is arrayed by the seat of the stone it stands on.
        let court = template("Court band");
        let surface = bare(&court, &lib);
        let gem = Gem::calibrated(GemCut::Round, 5.0);
        let stand = builders::stand_off_mm("claw4", gem);
        let stone_seat = Placement::ring(90.0, stand);
        let claws = builders::feature_on(3, "Four-claw head", builders::CLAW, 2, serde_json::json!({ "prongs": 4 }));
        let d = with(
            court.clone(),
            vec![band(), builders::stone_feature(2, gem, stone_seat.clone()), claws, feature(4, "Heads", Operation::Pattern { sources: 3.into(), kind: kind.clone() }, joined(Placement::Free))],
        );
        assert_eq!(seat_of(d.cad.as_ref().unwrap(), 3), Some((2, stone_seat.clone())));
        let e = on(&d, &lib, &surface);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let used = stone_seat.frame_on(&d, Some(&surface)).unwrap();
        let moved = motions(&kind, &d, Some(&surface), Some((&stone_seat, &used)), &|_| None).unwrap();
        let made = component(&e, 4).made.clone().unwrap();
        let head = centroid(&component(&e, 3).trace.positions);
        for (k, m) in moved.iter().enumerate() {
            let theta = 90.0 + 120.0 * (k + 1) as f64;
            let (hit, n) = cad::surface_hit(&surface, theta, 0.0).unwrap();
            let stand_off = dot(sub(m.point(used.origin), hit), n);
            assert!((stand_off - stand).abs() < 0.02, "head {} at {theta}°: its stone's girdle {stand_off:.4} off the band against {stand:.4}", k + 1);
            assert!(dist(copy_centroid(&made, &format!("Copy {}, ", k + 1)), m.point(head)) < 1e-9);
        }
    }

    #[test]
    fn a_ring_array_of_a_stone_on_a_plate_drops_each_copy_back_onto_the_plate_and_not_onto_the_band() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let surface = bare(&court, &lib);
        // A plate 14 mm round the ring on the top of the Court band, its foot 0.1 mm into the crown.
        let seat = Placement::ring(90.0, 0.65);
        let plate = feature(2, "Plate", Operation::Box { size: [4.0, 14.0, 1.5] }, joined(seat.clone()));
        let alone = on(&with(court.clone(), vec![band(), plate.clone()]), &lib, &surface);
        let host = component(&alone, 2);
        let top = face_along(&host.body, &host.frame, [0.0, 0.0, 1.0]) as u32;
        let gem = Gem::calibrated(GemCut::Round, 1.5);
        let h = builders::stand_off_mm("claw4", gem);
        let stood = cad::FaceSeat::on(host, top, None, h).unwrap();
        let claws = builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }));
        // Three stones 12° apart, the first the plate's own, and their heads.
        let kind = PatternKind::Ring { count: 3, span_deg: 24.0 };
        let stones = feature(5, "Stones", Operation::Pattern { sources: 3.into(), kind: kind.clone() }, Component { reference: true, ..Component::default() });
        let heads = feature(6, "Heads", Operation::Pattern { sources: 4.into(), kind: kind.clone() }, joined(Placement::Free));
        let d = with(court.clone(), vec![band(), plate, cad::stone_on_face(3, gem, 2, &stood), claws, stones, heads]);
        assert_eq!(seat_of(d.cad.as_ref().unwrap(), 4), None, "a stone on a face has no seat of its own on the band");
        let e = on(&d, &lib, &surface);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let face = stood.face_of(component(&e, 2)).unwrap();
        let (n, frame) = (face.normal, component(&e, 3).frame);
        let height = |p: [f64; 3]| dot(sub(p, face.origin), n);
        let moved = face_motions(&kind, &frame, &face).unwrap();
        // The ghost reads the evaluation's own motions off the parts as built.
        assert_eq!(copy_motions(&d, Some(&surface), &e, 4, &kind).unwrap(), moved);
        assert_eq!(copy_motions(&d, Some(&surface), &e, 3, &kind).unwrap(), moved);
        // Carried by the plate's seat dropped onto the band, as they were, each copy leaned off the plate's top.
        let old = motions(&kind, &d, Some(&surface), Some((&seat, &component(&e, 2).frame)), &|_| None).unwrap();
        let (stone_copies, head_copies) = (component(&e, 5).made.clone().unwrap(), component(&e, 6).made.clone().unwrap());
        let (stone, head) = (centroid(&component(&e, 3).trace.positions), centroid(&component(&e, 4).trace.positions));
        for (k, (m, was)) in moved.iter().zip(&old).enumerate() {
            let (o, z) = (m.point(frame.origin), m.vector(frame.z_axis));
            let (wo, wz) = (was.point(frame.origin), was.vector(frame.z_axis));
            let lean = |z: [f64; 3]| dot(z, n).clamp(-1.0, 1.0).acos().to_degrees();
            eprintln!("copy {} at {}°: {:.6} mm over the plate, leaning {:.4}°; riding the plate's seat it stood {:.4} mm over it, leaning {:.2}°", k + 1, 12 * (k + 1), height(o), lean(z), height(wo), lean(wz));
            assert!((height(o) - h).abs() < 1e-9 && lean(z) < 1e-6, "copy {}: {:.6} over the plate", k + 1, height(o));
            assert!((height(wo) - h).abs() > 0.2 && lean(wz) > 11.0, "copy {}: the plate's seat carried it {:.4} over the plate at {:.2}°", k + 1, height(wo), lean(wz));
            // Its foot is on the plate's top, inside the plate's 14 mm, and the copies are the source carried by these motions.
            let foot = m.point(sub(frame.origin, n.map(|v| v * h)));
            assert!(height(foot).abs() < 1e-9 && dot(sub(foot, face.origin), component(&e, 2).frame.y_axis).abs() < 6.0, "{foot:?}");
            assert!(dist(copy_centroid(&stone_copies, &format!("Copy {}, ", k + 1)), m.point(stone)) < 1e-9);
            assert!(dist(copy_centroid(&head_copies, &format!("Copy {}, ", k + 1)), m.point(head)) < 1e-9);
        }
        // Built, every head's claws reach the plate: one piece of metal.
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight && built.parts.notes.is_empty(), "{:?} {:?}", built.report.validation, built.parts.notes);
        assert_eq!((built.parts.joined, built.parts.references), (3, 2));
        assert_eq!(pieces(&built.mesh), 1);
    }

    #[test]
    fn a_face_arrays_copy_whose_foot_falls_off_the_plate_is_left_out_and_named_and_one_with_none_left_fails() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let surface = bare(&court, &lib);
        let plate = feature(2, "Plate", Operation::Box { size: [4.0, 14.0, 1.5] }, joined(Placement::ring(90.0, 0.65)));
        let alone = on(&with(court.clone(), vec![band(), plate.clone()]), &lib, &surface);
        let host = component(&alone, 2);
        let top = face_along(&host.body, &host.frame, [0.0, 0.0, 1.0]) as u32;
        let gem = Gem::calibrated(GemCut::Round, 1.5);
        let stood = cad::FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
        let claws = builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }));
        // Four heads 24° apart over 72°: the stone 12.9 mm from the finger's axis, the second copy's foot lands 9.6 mm along a plate 7 mm either side.
        let kind = PatternKind::Ring { count: 4, span_deg: 72.0 };
        let heads = feature(6, "Heads", Operation::Pattern { sources: 4.into(), kind: kind.clone() }, joined(Placement::Free));
        let d = with(court.clone(), vec![band(), plate.clone(), cad::stone_on_face(3, gem, 2, &stood), claws.clone(), heads]);
        let e = on(&d, &lib, &surface);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let face = stood.face_of(component(&e, 2)).unwrap();
        let outline = FaceOutline::of_seat(&stood, component(&e, 2)).unwrap();
        let along = component(&e, 2).frame.y_axis;
        let at = |mm: f64| std::array::from_fn(|k| face.origin[k] + along[k] * mm);
        assert_eq!([0.0, 6.9, 7.1, -7.1].map(|mm| outline.holds(at(mm))), [true, true, false, false], "the top is 14 mm round the ring");
        let all = copy_instances(&d, Some(&surface), &e, 4, &kind).unwrap();
        let foot = |i: &Instance| {
            let frame = component(&e, 3).frame;
            let h = dot(sub(frame.origin, face.origin), face.normal);
            dot(sub(i.motion.point(sub(frame.origin, face.normal.map(|v| v * h))), face.origin), along)
        };
        eprintln!("heads at {:?}°: feet {:?} mm along the plate's top", all.iter().map(|i| i.angle_deg).collect::<Vec<_>>(), all.iter().map(|i| (foot(i) * 1000.0).round() / 1000.0).collect::<Vec<_>>());
        assert_eq!(all.iter().map(|i| (i.angle_deg, i.off_face)).collect::<Vec<_>>(), [(24.0, false), (48.0, true), (72.0, true)]);
        assert!(foot(&all[0]).abs() < 6.0 && foot(&all[1]).abs() > 7.0);
        assert_eq!(copy_motions(&d, Some(&surface), &e, 4, &kind).unwrap(), [all[0].motion], "the ghost's motions are the kept copy's");
        // Built, the heads' pattern carries the one copy left on the plate, and its status names the two it left out.
        let report = e.features.iter().find(|r| r.id == 6).unwrap();
        assert_eq!(report.status, FeatureStatus::Ok);
        assert_eq!(report.notes.iter().filter(|n| n.contains("stands off the face of")).map(|n| n.split('°').next().unwrap().to_owned()).collect::<Vec<_>>(), ["The copy 48.0", "The copy 72.0"], "{:?}", report.notes);
        let made = component(&e, 6).made.clone().unwrap();
        assert!(made.named.names.iter().all(|n| n.starts_with("Copy 1, ")), "one copy: {:?}", made.named.names.first());
        let (head, n) = (centroid(&component(&e, 4).trace.positions), &face.normal);
        assert!(dist(copy_centroid(&made, "Copy 1, "), all[0].motion.point(head)) < 1e-9 && n[2].abs() < 1.0);
        // A single copy 90° round is off the plate: the pattern has nothing left to stand and fails by name.
        let lone = PatternKind::Ring { count: 2, span_deg: 90.0 };
        let heads = feature(6, "Heads", Operation::Pattern { sources: 4.into(), kind: lone }, joined(Placement::Free));
        let d = with(court, vec![band(), plate, cad::stone_on_face(3, gem, 2, &stood), claws, heads]);
        let why = failed(&on(&d, &lib, &surface), 6);
        assert!(why.starts_with("Every copy of") && why.contains("stands off the face of"), "{why}");
    }

    #[test]
    fn a_part_mirrored_across_the_band_lands_at_minus_across_with_its_volume_wound_outward_and_the_verdict_judges_both() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let surface = bare(&court, &lib);
        let seat = Placement::Ring { theta_deg: 90.0, across_mm: 1.0, height_mm: 0.3, spin_deg: 25.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let mirror = |plane| Operation::Pattern { sources: 2.into(), kind: PatternKind::Mirror { plane } };
        let d = with(
            court.clone(),
            vec![band(), feature(2, "Block", Operation::Box { size: [1.2, 0.8, 1.0] }, joined(seat)), feature(3, "Mirror of Block", mirror(MirrorPlane::Band), joined(Placement::Free))],
        );
        let e = on(&d, &lib, &surface);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let a = component(&e, 2);
        let made = component(&e, 3).made.clone().unwrap();
        let block = Solid { v: a.trace.positions.clone(), f: a.mesh.faces.clone() };
        let (va, vb) = (block.volume(), made.solid().volume());
        assert!(va > 0.0 && vb > 0.0, "both wound outward: {va} {vb}");
        assert!((vb / va - 1.0).abs() < 1e-9, "{vb} against {va}");
        let (ca, cb) = (centroid(&block.v), centroid(&made.solid().v));
        assert!((ca[2] - 1.0).abs() < 0.3 && (cb[2] + ca[2]).abs() < 0.02 && (cb[0] - ca[0]).abs() < 0.02 && (cb[1] - ca[1]).abs() < 0.02, "{ca:?} {cb:?}");
        // On a band symmetric about its mid-plane the mirror is the plain reflection, point for point.
        let worst = block.v.iter().zip(&made.solid().v).map(|(p, q)| dist([p[0], p[1], -p[2]], *q)).fold(0.0, f64::max);
        assert!(worst < 0.02, "{worst}");
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight && built.parts.notes.is_empty(), "{:?} {:?}", built.report.validation, built.parts.notes);
        let f = crate::castability::judged_field_report(&d, &lib, &d.draft, 96, 64, Some(&built));
        let judged: Vec<Id> = f.parts.iter().filter(|p| p.judged && p.total_area_mm2 > 0.5).map(|p| p.feature).collect();
        assert_eq!(judged, vec![2, 3], "{:?}", f.parts);
        let area = |id: Id| f.parts.iter().find(|p| p.feature == id).unwrap().total_area_mm2;
        assert!((area(2) / area(3) - 1.0).abs() < 0.1, "{} against {}", area(2), area(3));
        // Through the head: a part at 60° lands at 120°.
        let seat = Placement::ring(60.0, 0.3);
        let d = with(
            court.clone(),
            vec![
                band(),
                feature(2, "Block", Operation::Box { size: [1.2, 0.8, 1.0] }, joined(seat)),
                feature(3, "Mirror of Block", mirror(MirrorPlane::Section { theta_deg: 90.0 }), joined(Placement::Free)),
            ],
        );
        let e = on(&d, &lib, &surface);
        let (c, m) = (centroid(&component(&e, 2).trace.positions), centroid(&component(&e, 3).made.clone().unwrap().solid().v));
        let theta = |p: [f64; 3]| p[1].atan2(p[0]).to_degrees();
        assert!((theta(c) - 60.0).abs() < 0.5 && (theta(m) - 120.0).abs() < 0.5 && (m[2] - c[2]).abs() < 1e-3, "{c:?} {m:?}");
        assert!((c[0].hypot(c[1]) - m[0].hypot(m[1])).abs() < 0.02);
    }

    #[test]
    fn press_pull_moves_a_box_top_so_its_volume_grows_by_the_area_times_the_distance_and_refuses_by_name() {
        let lib = AlphaLibrary::builtin();
        let block = make::cuboid([-2.0, -1.5, -1.0], [4.0, 3.0, 2.0]).unwrap();
        let top = FaceRef::signed(&block, face_along(&block, &IDENTITY, [0.0, 0.0, 1.0]), &IDENTITY);
        let pulled = |face: FaceRef, distance_mm: f64, source: Operation| {
            with(RingDesign::default(), vec![feature(1, "Block", source, Component::default()), feature(2, "Pull", Operation::PressPull { source: 1, face, distance_mm }, Component::default())])
        };
        let boxed = || Operation::Box { size: [4.0, 3.0, 2.0] };
        for (distance, volume) in [(0.5, 30.0), (-0.5, 18.0), (2.0, 48.0)] {
            let e = evaluate(&pulled(top.clone(), distance, boxed()), &lib, params()).unwrap();
            assert!(e.failures().is_empty(), "{distance}: {:?}", e.failures());
            let c = component(&e, 2);
            assert!((c.mesh.volume_mm3() - volume).abs() < 1e-6, "{distance}: {}", c.mesh.volume_mm3());
            assert_eq!((c.body.faces.len(), c.body.edges.len()), (6, 12), "the face moved and its neighbours followed");
            assert!(e.features[1].notes.is_empty() && c.mesh.validate().watertight);
            assert_eq!(e.components.iter().map(|c| c.id).collect::<Vec<_>>(), vec![2], "the pull takes the box's place");
        }
        // A box seated on the ring is pulled in its own frame, and the pull stands where the box did.
        let seat = Placement::ring(90.0, 1.0);
        let frame = seat.frame(&RingDesign::default()).unwrap();
        let mut d = pulled(top.clone(), 0.5, boxed());
        d.cad.as_mut().unwrap().features[0].component.placement = seat;
        let e = evaluate(&d, &lib, params()).unwrap();
        assert!((component(&e, 2).mesh.volume_mm3() - 30.0).abs() < 1e-6 && component(&e, 2).frame == frame);
        // The face a gesture picks: signed in the part's own frame, its centre and outward normal in the world.
        let (signed, centre, normal) = planar_face(component(&e, 2), top.ordinal as u32).unwrap();
        assert!(dist(centre, frame.point([0.0, 0.0, 1.5])) < 1e-9 && dist(normal, frame.z_axis) < 1e-9, "{centre:?} {normal:?}");
        assert!(signed.signature.as_ref().is_some_and(|s| dist(s.normal, [0.0, 0.0, 1.0]) < 1e-9), "{signed:?}");
        // Refused by name: no distance, pushed right through, a curved face, and a mesh a builder made.
        let why = failed(&evaluate(&pulled(top.clone(), 0.0, boxed()), &lib, params()).unwrap(), 2);
        assert_eq!(why, format!("A press-pull moves its face {MIN_PULL_MM} to {MAX_PULL_MM} mm either way"));
        let why = failed(&evaluate(&pulled(top.clone(), -2.5, boxed()), &lib, params()).unwrap(), 2);
        assert!(why.starts_with("Press-pull cannot move face"), "{why}");
        let drum = make::cylinder([0.0, 0.0, -1.0], 1.5, 2.0).unwrap();
        let wall = (0..drum.faces.len()).find(|i| face_signature(&drum, *i, &IDENTITY).is_some_and(|s| s.kind == SurfaceKind::Cylinder)).unwrap();
        let on_drum = evaluate(&pulled(FaceRef::signed(&drum, wall, &IDENTITY), 0.5, Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 }), &lib, params()).unwrap();
        assert_eq!(failed(&on_drum, 2), format!("Press-pull face {wall}: press-pull moves planar faces, and this one is a cylinder"));
        let drum_part = evaluate(&with(RingDesign::default(), vec![feature(1, "Drum", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 }, Component::default())]), &lib, params()).unwrap();
        assert_eq!(planar_face(component(&drum_part, 1), wall as u32).unwrap_err().to_string(), format!("Press-pull moves planar faces; face {wall} of #1 Drum is a cylinder"));
        let stone = builders::stone_feature(1, Gem::calibrated(GemCut::Round, 5.0), Placement::Free);
        let d = with(RingDesign::default(), vec![stone.clone(), feature(2, "Pull", Operation::PressPull { source: 1, face: FaceRef::bare(2), distance_mm: 0.5 }, Component::default())]);
        let e = evaluate(&d, &lib, params()).unwrap();
        assert_eq!(failed(&e, 2), "Press-pull works on kernel bodies, and #1 Round 5 mm is a mesh a builder made");
        let alone = evaluate(&with(RingDesign::default(), vec![stone]), &lib, params()).unwrap();
        assert_eq!(planar_face(component(&alone, 1), 2).unwrap_err().to_string(), "Press-pull moves a kernel part's faces; #1 Round 5 mm is a mesh a builder made");
    }

    #[test]
    fn a_fillet_keeps_its_edge_through_every_resize_seat_and_pull_and_a_removed_edge_fails_it_by_name() {
        let lib = AlphaLibrary::builtin();
        let r = 0.4;
        // What a round of radius r takes off each millimetre of a square edge.
        let wedge = (1.0 - PI / 4.0) * r * r;
        let seated = |theta: f64| Placement::ring(theta, 2.0);
        let frame = seated(90.0).frame(&RingDesign::default()).unwrap();
        let block = brep::transform(&make::cuboid([-2.0; 3], [4.0; 3]).unwrap(), &frame).unwrap();
        // An edge running along the part's own x, picked on the box as it first stood.
        let along_x = (0..block.edges.len())
            .find(|i| edge_signature(&block, *i, &frame).is_some_and(|s| s.direction[0].abs() > 0.99 && s.midpoint[1] > 0.0 && s.midpoint[2] > 0.0))
            .unwrap();
        let edge = EdgeRef::signed(&block, along_x, &frame);
        let rounded = |size: [f64; 3], theta: f64| {
            with(
                RingDesign::default(),
                vec![
                    feature(1, "Block", Operation::Box { size }, Component { placement: seated(theta), ..Component::default() }),
                    feature(2, "Round", Operation::Fillet { source: 1, edges: vec![edge.clone()], radius_mm: r }, Component::default()),
                ],
            )
        };
        for (size, theta) in [([4.0; 3], 90.0), ([5.0, 4.0, 3.0], 90.0), ([6.0, 2.0, 2.5], 30.0), ([3.0, 5.0, 4.0], 200.0), ([2.5, 3.5, 6.0], 315.0)] {
            let e = evaluate(&rounded(size, theta), &lib, params()).unwrap();
            assert!(e.failures().is_empty(), "{size:?} at {theta}°: {:?}", e.failures());
            assert!(e.features[1].notes.is_empty(), "{size:?} at {theta}°: {:?}", e.features[1].notes);
            let removed = size.iter().product::<f64>() - component(&e, 2).mesh.volume_mm3();
            assert!((removed / (wedge * size[0]) - 1.0).abs() < 0.03, "{size:?} at {theta}°: {removed:.4} off, a round along x takes {:.4}", wedge * size[0]);
        }
        // The same edge through a press-pull of the top: a round up the side grows with the pull and is never another edge.
        let top = FaceRef::signed(&block, face_along(&block, &frame, [0.0, 0.0, 1.0]), &frame);
        let pulled_block = |d: f64| {
            vec![
                feature(1, "Block", Operation::Box { size: [4.0; 3] }, Component { placement: seated(90.0), ..Component::default() }),
                feature(2, "Pull", Operation::PressPull { source: 1, face: top.clone(), distance_mm: d }, Component::default()),
            ]
        };
        let first = evaluate(&with(RingDesign::default(), pulled_block(0.5)), &lib, params()).unwrap();
        let pulled = component(&first, 2);
        let up = (0..pulled.body.edges.len())
            .find(|i| edge_signature(&pulled.body, *i, &pulled.frame).is_some_and(|s| s.direction[2].abs() > 0.99 && s.midpoint[0] > 0.0 && s.midpoint[1] > 0.0))
            .unwrap();
        let side = EdgeRef::signed(&pulled.body, up, &pulled.frame);
        for d in [0.5, 1.2, -0.6] {
            let mut features = pulled_block(d);
            features.push(feature(3, "Round", Operation::Fillet { source: 2, edges: vec![side.clone()], radius_mm: r }, Component::default()));
            let e = evaluate(&with(RingDesign::default(), features), &lib, params()).unwrap();
            assert!(e.failures().is_empty() && e.features[2].notes.is_empty(), "{d}: {:?} {:?}", e.failures(), e.features[2].notes);
            let removed = 16.0 * (4.0 + d) - component(&e, 3).mesh.volume_mm3();
            assert!((removed / (wedge * (4.0 + d)) - 1.0).abs() < 0.03, "pulled {d}: {removed:.4} off against {:.4}", wedge * (4.0 + d));
        }
        // A box turned into a drum has no such edge: the fillet fails in the reference's own words.
        let mut drum = rounded([4.0; 3], 90.0);
        drum.cad.as_mut().unwrap().features[0].operation = Operation::Cylinder { radius_mm: 2.0, height_mm: 4.0 };
        let why = failed(&evaluate(&drum, &lib, params()).unwrap(), 2);
        assert_eq!(why, format!("Edge {along_x} is no longer a line between plane and plane; the source changed underneath, pick it again"));
    }

    #[test]
    fn a_bare_reference_is_signed_in_the_frame_its_part_was_seated_by_and_then_holds_through_a_resize() {
        let lib = AlphaLibrary::builtin();
        let block = |size: [f64; 3], theta: f64| feature(1, "Block", Operation::Box { size }, Component { placement: Placement::ring(theta, 1.5), ..Component::default() });
        let e = evaluate(&with(RingDesign::default(), vec![block([4.0, 3.0, 2.0], 120.0)]), &lib, params()).unwrap();
        let c = component(&e, 1);
        let top = face_along(&c.body, &c.frame, [0.0, 0.0, 1.0]);
        let rim = (0..c.body.edges.len())
            .find(|i| edge_signature(&c.body, *i, &c.frame).is_some_and(|s| s.direction[0].abs() > 0.99 && s.midpoint[1] > 0.0 && s.midpoint[2] > 0.0))
            .unwrap();
        let mut anchored = Sketch::circle(0.5);
        anchored.plane.on_face = Some(FaceAnchor { feature: 1, face: FaceRef::bare(top) });
        let mut ops = vec![
            Operation::Chamfer { source: 1, edges: vec![EdgeRef::bare(rim)], base_face: FaceRef::bare(top), distance_mm: 0.3 },
            Operation::PressPull { source: 1, face: FaceRef::bare(top), distance_mm: 0.4 },
            Operation::Plane { base: PlaneBase::Face { feature: 1, face: FaceRef::bare(top) }, offset_mm: 0.0 },
            Operation::Sketch { sketch: anchored },
            Operation::Shell { source: 1, open_faces: vec![FaceRef::bare(top)], thickness_mm: 0.3 },
        ];
        assert_eq!(ops.iter_mut().map(|op| cad::sign_refs(op, &e)).collect::<Vec<_>>(), [2, 1, 1, 1, 1]);
        // Signed in the part's own frame: the top looks up the part's own z, wherever on the ring it stands.
        let Operation::PressPull { face, .. } = &ops[1] else { unreachable!() };
        assert!(face.signature.as_ref().is_some_and(|s| s.normal[2] > 1.0 - 1e-9), "{face:?}");
        // Signed once is signed; a part the evaluation did not build leaves its reference bare.
        assert_eq!(cad::sign_refs(&mut ops[1], &e), 0);
        let mut elsewhere = Operation::PressPull { source: 9, face: FaceRef::bare(0), distance_mm: 0.4 };
        assert_eq!(cad::sign_refs(&mut elsewhere, &e), 0);
        // The signed chamfer keeps its edge and its base face when the block grows and moves round the ring.
        for (size, theta) in [([6.0, 3.0, 2.0], 200.0), ([4.0, 5.0, 3.0], 30.0)] {
            let d = with(RingDesign::default(), vec![block(size, theta), feature(2, "Chamfer", ops[0].clone(), Component::default())]);
            let e = evaluate(&d, &lib, params()).unwrap();
            assert!(e.failures().is_empty() && e.features[1].notes.is_empty(), "{size:?} at {theta}°: {:?} {:?}", e.failures(), e.features[1].notes);
            let removed = size.iter().product::<f64>() - component(&e, 2).mesh.volume_mm3();
            assert!((removed - 0.045 * size[0]).abs() < 1e-4, "{size:?}: {removed} off, a 0.3 mm chamfer along x takes {}", 0.045 * size[0]);
        }
    }

    #[test]
    fn work_planes_carry_sketches_and_mirrors_and_a_face_that_is_gone_fails_them_by_name() {
        let lib = AlphaLibrary::builtin();
        let block = make::cuboid([-2.0, -1.5, -1.0], [4.0, 3.0, 2.0]).unwrap();
        let top = FaceRef::signed(&block, face_along(&block, &IDENTITY, [0.0, 0.0, 1.0]), &IDENTITY);
        let plane = |base: PlaneBase, offset_mm: f64| Operation::Plane { base, offset_mm };
        let mut sketch = Sketch::rectangle(2.0, 2.0);
        sketch.plane.on_face = Some(FaceAnchor { feature: 3, face: FaceRef::bare(0) });
        let features = |first: Operation| {
            vec![
                feature(1, "Block", first, Component::default()),
                feature(2, "Section at 90°", plane(PlaneBase::Section { theta_deg: 90.0 }, 0.0), Component::default()),
                feature(3, "Over the top", plane(PlaneBase::Face { feature: 1, face: top.clone() }, 0.5), Component::default()),
                feature(4, "Parting", plane(PlaneBase::Parting, 0.0), Component::default()),
                feature(5, "Sketch", Operation::Sketch { sketch: sketch.clone() }, Component::default()),
                feature(6, "Boss", Operation::Extrude { sketch: Profile::Feature { feature: 5 }, height_mm: 1.0, draft_deg: 0.0 }, Component::default()),
                feature(7, "Moved", Operation::Transform { source: 6, translation: [3.0, 0.0, 0.0], rotation_deg: [0.0; 3] }, Component::default()),
                feature(8, "Mirror of Moved", Operation::Pattern { sources: 7.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 2 } } }, Component::default()),
                feature(9, "Mirror across the top", Operation::Pattern { sources: 1.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 3 } } }, Component::default()),
            ]
        };
        let d = with(RingDesign::default(), features(Operation::Box { size: [4.0, 3.0, 2.0] }));
        let doc = d.cad.as_ref().unwrap();
        assert_eq!(doc.outputs, vec![1, 7, 8, 9], "a work plane has no body to output");
        assert!(!with(RingDesign::default(), vec![feature(1, "Parting", plane(PlaneBase::Parting, 0.0), Component::default())]).cad.unwrap().replaces_band());
        let e = evaluate(&d, &lib, params()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let close = |a: [f64; 3], b: [f64; 3]| dist(a, b) < 1e-9;
        let section = e.plane(2).unwrap();
        assert!(close(section.origin, [0.0; 3]) && close(section.x, [0.0, 1.0, 0.0]) && close(section.y, [0.0, 0.0, 1.0]) && close(section.normal, [1.0, 0.0, 0.0]), "{section:?}");
        let over = e.plane(3).unwrap();
        assert!(close(over.origin, [0.0, 0.0, 1.5]) && close(over.normal, [0.0, 0.0, 1.0]), "{over:?}");
        let parting = e.plane(4).unwrap();
        assert!(close(parting.normal, [0.0, 0.0, 1.0]) && close(parting.origin, [0.0; 3]));
        assert_eq!(e.planes.iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 3, 4]);
        // The sketch lies on the plane half a millimetre over the top, and its boss stands on it.
        let boss = component(&e, 7).mesh.bounds().unwrap();
        assert!((boss.0.2 - 1.5).abs() < 1e-6 && (boss.1.2 - 2.5).abs() < 1e-6 && (boss.0.0 - 2.0).abs() < 1e-6, "{boss:?}");
        // Mirrored across the section at 90°, the plane x = 0: the boss lands on the other side.
        let mirrored = component(&e, 8).mesh.bounds().unwrap();
        assert!((mirrored.0.0 + 4.0).abs() < 1e-6 && (mirrored.1.0 + 2.0).abs() < 1e-6 && (mirrored.0.2 - 1.5).abs() < 1e-6, "{mirrored:?}");
        // Mirrored across the plane over the top: the block stands on its head over it.
        let over_top = component(&e, 9).mesh.bounds().unwrap();
        assert!((over_top.0.2 - 2.0).abs() < 1e-6 && (over_top.1.2 - 4.0).abs() < 1e-6, "{over_top:?}");
        // A sphere has no top face: the plane on it fails by name, and what lies on it or reflects across it is skipped.
        let d = with(RingDesign::default(), features(Operation::Sphere { radius_mm: 2.0 }));
        let e = evaluate(&d, &lib, params()).unwrap();
        assert_eq!(failed(&e, 3), format!("Work plane face: Face {} is no longer a Plane face; the source changed underneath, pick it again", top.ordinal));
        assert_eq!(e.status_of(5), Some(&FeatureStatus::Skipped("source #3 Over the top failed".into())));
        assert_eq!(e.status_of(9), Some(&FeatureStatus::Skipped("source #3 Over the top failed".into())));
        assert_eq!(e.status_of(2), Some(&FeatureStatus::Ok));
        assert_eq!(e.planes.iter().map(|p| p.id).collect::<Vec<_>>(), vec![2, 4]);
        // Square to the band at a point: on the built surface there, else on the reference crest.
        let court = template("Court band");
        let surface = bare(&court, &lib);
        let tangent = |surface: Option<&Mesh>| work_plane(1, &PlaneBase::Tangent { theta_deg: 60.0, across_mm: 0.5 }, 0.0, &court, surface, &BTreeMap::new(), &BTreeMap::new(), &mut Vec::new()).unwrap();
        let (on_band, bare_crest) = (tangent(Some(&surface)), tangent(None));
        let (hit, n) = cad::surface_hit(&surface, 60.0, 0.5).unwrap();
        assert!(close(on_band.origin, hit) && close(on_band.normal, n), "{on_band:?}");
        assert!(dot(on_band.x, n).abs() < 1e-12 && dot(on_band.y, n).abs() < 1e-12 && on_band.x[2].abs() < 1e-5 && on_band.y[2] > 0.95, "x round the ring, y up the finger: {on_band:?}");
        let r = court.inner_radius_mm() + court.profile.thickness_mm;
        assert!(dist(bare_crest.origin, [r * 0.5, r * 3f64.sqrt() / 2.0, 0.5]) < 1e-9);
        // A mirror across a feature that is not a work plane says so.
        let mut d = with(RingDesign::default(), features(Operation::Box { size: [4.0, 3.0, 2.0] }));
        d.cad.as_mut().unwrap().features[7].operation = Operation::Pattern { sources: 7.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 1 } } };
        assert_eq!(failed(&evaluate(&d, &lib, params()).unwrap(), 8), "#1 Block is not a work plane; a mirror reflects across one");
    }

    #[test]
    fn a_pattern_is_cached_on_its_parameters_its_source_and_the_chord() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let surface = bare(&court, &lib);
        let never = AtomicBool::new(false);
        let ctx = BuildCtx::new(&never).with_surface(&surface);
        let cache = Mutex::new(Cache::default());
        let memo = Memo::new(&cache).with_epoch(cad::surface_epoch(&surface));
        let tally = || {
            let c = cache.lock().unwrap();
            (c.hits(), c.misses())
        };
        let post = |r: f64| feature(2, "Post", Operation::Cylinder { radius_mm: r, height_mm: 2.0 }, joined(Placement::ring(90.0, 0.8)));
        let ring = |count: u32| feature(3, "Posts", Operation::Pattern { sources: 2.into(), kind: PatternKind::Ring { count, span_deg: 360.0 } }, joined(Placement::Free));
        let run = |d: &RingDesign, p: BuildParams| cad::evaluate_memo(d, &lib, p, &ctx, memo).unwrap();
        // Cold: the post, its tessellation read by the pattern, the pattern; the post's own output answers.
        run(&with(court.clone(), vec![band(), post(1.0), ring(3)]), params());
        assert_eq!(tally(), (1, 4));
        run(&with(court.clone(), vec![band(), post(1.0), ring(3)]), params());
        assert_eq!(tally(), (5, 4), "an unchanged pattern builds nothing");
        // Another count rebuilds the pattern alone; another post rebuilds both.
        let e = run(&with(court.clone(), vec![band(), post(1.0), ring(4)]), params());
        assert_eq!(tally(), (8, 6));
        assert_eq!(component(&e, 3).made.as_ref().unwrap().count("Copy "), 3 * 3, "three copies of a drum's three faces");
        run(&with(court.clone(), vec![band(), post(1.2), ring(4)]), params());
        assert_eq!(tally(), (9, 10));
        // The export chord re-tessellates the post and rebuilds the copies of it.
        run(&with(court.clone(), vec![band(), post(1.2), ring(4)]), BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() });
        assert_eq!(tally(), (11, 13));
    }

    #[test]
    fn a_pattern_refuses_what_it_cannot_place_by_name() {
        let lib = AlphaLibrary::builtin();
        let cylinder = || Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 };
        let design = |kind: PatternKind, placement: Placement| {
            with(
                RingDesign::default(),
                vec![
                    feature(1, "Post", cylinder(), Component::default()),
                    feature(2, "Plan", Operation::Sketch { sketch: Sketch::circle(1.0) }, Component::default()),
                    feature(3, "Posts", Operation::Pattern { sources: 1.into(), kind }, Component { placement, ..Component::default() }),
                ],
            )
        };
        let ring = |count: u32, span_deg: f64| PatternKind::Ring { count, span_deg };
        let why = |kind: PatternKind, placement: Placement| failed(&evaluate(&design(kind, placement), &lib, params()).unwrap(), 3);
        assert_eq!(why(ring(1, 360.0), Placement::Free), "A pattern holds 2 to 120 instances, its source among them, not 1");
        assert_eq!(why(ring(121, 360.0), Placement::Free), "A pattern holds 2 to 120 instances, its source among them, not 121");
        assert_eq!(why(ring(4, 0.0), Placement::Free), "A pattern spans more than 0° and at most 360° either way");
        assert_eq!(why(ring(4, f64::NAN), Placement::Free), "A pattern spans more than 0° and at most 360° either way");
        assert_eq!(why(ring(4, 360.0), Placement::ring(90.0, 0.0)), "A pattern stands where its copies do; move #1 Post to move them");
        assert_eq!(why(PatternKind::About { part: 1, count: 3, span_deg: 360.0 }, Placement::Free), "Part #1 stands free of the ring and of any stone; array round a stone or a seated part");
        let mut d = design(ring(3, 360.0), Placement::Free);
        d.cad.as_mut().unwrap().features[2].operation = Operation::Pattern { sources: 2.into(), kind: ring(3, 360.0) };
        assert_eq!(failed(&evaluate(&d, &lib, params()).unwrap(), 3), "#2 Plan has no body to copy");
        // Angles: a whole turn closes on itself, an open arc ends on its last.
        assert_eq!(ring(4, 360.0).angles().unwrap(), vec![90.0, 180.0, 270.0]);
        assert_eq!(ring(3, 90.0).angles().unwrap(), vec![45.0, 90.0]);
        assert_eq!(ring(4, -360.0).angles().unwrap(), vec![-90.0, -180.0, -270.0]);
        // A free part arrayed round the ring is turned about the finger's axis.
        let e = evaluate(&design(ring(4, 360.0), Placement::Free), &lib, params()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        assert_eq!(e.components.iter().map(|c| c.id).collect::<Vec<_>>(), vec![1, 3], "the source stays a part beside its copies");
        // Copies that meet are united: four drums turned about their own axis are one drum's worth four times over.
        let made = component(&e, 3).made.clone().unwrap();
        assert_eq!(made.solid().open_edges(), (0, 0));
        assert!(e.features[2].notes.iter().all(|n| n.contains("stays a shell")), "{:?}", e.features[2].notes);
    }

    /// Costs for the report: `cargo test -p ringdesign-core measured_patterns -- --ignored --nocapture`.
    #[test]
    #[ignore = "timings only"]
    fn measured_patterns() {
        let lib = AlphaLibrary::builtin();
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let stand = builders::stand_off_mm("claw4", gem);
        let stone = || builders::stone_feature(2, gem, Placement::ring(90.0, stand));
        let claws = || builders::feature_on(3, "Four-claw head", builders::CLAW, 2, serde_json::json!({ "prongs": 4 }));
        let post = || feature(4, "Post", Operation::Cylinder { radius_mm: 0.45, height_mm: 3.0 }, joined(Placement::ring(80.0, 1.2)));
        let cases: Vec<(&str, Vec<Feature>)> = vec![
            ("posts: ring array x12", vec![band(), post(), feature(5, "Posts", Operation::Pattern { sources: 4.into(), kind: PatternKind::Ring { count: 12, span_deg: 360.0 } }, joined(Placement::Free))]),
            ("claw head: ring array x3", vec![band(), stone(), claws(), feature(5, "Heads", Operation::Pattern { sources: 3.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } }, joined(Placement::Free))]),
            ("post: round the stone x6", vec![band(), stone(), post(), feature(5, "Prongs", Operation::Pattern { sources: 4.into(), kind: PatternKind::About { part: 2, count: 6, span_deg: 360.0 } }, joined(Placement::Free))]),
            ("post: mirror across the band", vec![band(), post(), feature(5, "Mirror", Operation::Pattern { sources: 4.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, joined(Placement::Free))]),
        ];
        let mut court = template("Court band");
        court.profile.width_mm = 8.0;
        for (label, p) in [("preview 256x128", params()), ("export 1024x384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() })] {
            let mut plain = court.clone();
            plain.cad = None;
            let surface = crate::mesh::try_build(&plain, &lib, p).unwrap().mesh;
            let never = AtomicBool::new(false);
            for (name, features) in &cases {
                let d = with(court.clone(), features.clone());
                let best = (0..3)
                    .map(|_| {
                        let started = std::time::Instant::now();
                        let e = cad::evaluate_with(&d, &lib, p, &BuildCtx::new(&never).with_surface(&surface)).unwrap();
                        assert!(e.failures().is_empty(), "{name}: {:?}", e.failures());
                        started.elapsed().as_secs_f64() * 1e3
                    })
                    .fold(f64::MAX, f64::min);
                let started = std::time::Instant::now();
                let built = crate::mesh::try_build(&d, &lib, p).unwrap();
                let ms = started.elapsed().as_secs_f64() * 1e3;
                let copies = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 5).map_or(0, |c| c.mesh.faces.len());
                eprintln!("{label:<16} {name:<30} evaluate {best:>7.1} ms, ring {ms:>7.1} ms (parts {:>5} ms), copies {copies} faces, ring {} faces, watertight {}, notes {:?}", built.parts.ms, built.mesh.faces.len(), built.report.validation.watertight, built.parts.notes);
            }
        }
    }

    #[test]
    fn the_new_operations_read_and_write_as_json_and_name_what_they_read() {
        let pattern = Operation::Pattern { sources: 3.into(), kind: PatternKind::About { part: 2, count: 6, span_deg: 360.0 } };
        let text = serde_json::to_string(&pattern).unwrap();
        assert_eq!(text, r#"{"Pattern":{"source":3,"kind":{"about":{"part":2,"count":6,"span_deg":360.0}}}}"#);
        let short: Operation = serde_json::from_str(r#"{"Pattern":{"source":3,"kind":{"ring":{"count":5}}}}"#).unwrap();
        assert!(matches!(short, Operation::Pattern { kind: PatternKind::Ring { count: 5, span_deg }, .. } if span_deg == 360.0), "a span left out is a whole turn");
        let mirror = Operation::Pattern { sources: 3.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Band } };
        assert_eq!(serde_json::to_string(&mirror).unwrap(), r#"{"Pattern":{"source":3,"kind":{"mirror":{"plane":"band"}}}}"#);
        let plane = Operation::Plane { base: PlaneBase::Face { feature: 3, face: FaceRef::bare(4) }, offset_mm: 0.5 };
        assert_eq!(serde_json::to_string(&plane).unwrap(), r#"{"Plane":{"base":{"face":{"feature":3,"face":{"ordinal":4}}},"offset_mm":0.5}}"#);
        let pull = Operation::PressPull { source: 3, face: FaceRef::bare(5), distance_mm: -0.25 };
        for op in [pattern.clone(), mirror.clone(), plane.clone(), pull.clone()] {
            let text = serde_json::to_string(&op).unwrap();
            assert_eq!(serde_json::to_string(&serde_json::from_str::<Operation>(&text).unwrap()).unwrap(), text);
        }
        assert_eq!((pattern.sources(), pattern.consumes(), pattern.label()), (vec![3, 2], vec![], "Array round a part"));
        assert_eq!((mirror.sources(), mirror.consumes(), mirror.label()), (vec![3], vec![], "Mirror"));
        let across = Operation::Pattern { sources: 3.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 7 } } };
        assert_eq!(across.sources(), vec![3, 7]);
        assert_eq!((plane.sources(), plane.consumes(), plane.label(), plane.has_body()), (vec![3], vec![], "Work plane", false));
        assert_eq!((pull.sources(), pull.consumes(), pull.label(), pull.has_body()), (vec![3], vec![3], "Press-pull", true));
        // The funnel keeps a work plane out of the outputs, whatever it is edited to.
        let mut doc = Document::default();
        doc.apply(&CadEdit::Add { feature: feature(1, "Post", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, Component::default()), after: None }).unwrap();
        doc.apply(&CadEdit::Add { feature: feature(2, "Section", Operation::Plane { base: PlaneBase::Section { theta_deg: 90.0 }, offset_mm: 0.0 }, Component::default()), after: None }).unwrap();
        assert_eq!(doc.outputs, vec![1]);
        let refused = doc.apply(&CadEdit::Outputs { outputs: vec![1, 2] }).unwrap_err().to_string();
        assert_eq!(refused, "#2 Section is a work plane and has no body to output");
        doc.apply(&CadEdit::Operation { id: 2, operation: Operation::Sphere { radius_mm: 1.0 } }).unwrap();
        assert_eq!(doc.outputs, vec![1, 2]);
        doc.apply(&CadEdit::Operation { id: 2, operation: Operation::Plane { base: PlaneBase::Parting, offset_mm: 0.0 } }).unwrap();
        assert_eq!(doc.outputs, vec![1]);
        assert_eq!(CadEdit::Add { feature: feature(0, "", Operation::Pattern { sources: 1.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } }, Component::default()), after: None }.label(), "Add Ring array");
    }
}
