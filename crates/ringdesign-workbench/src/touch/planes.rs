//! Work planes on a view: each plane's rectangle in the world, where its name is written, which plane a finger takes, and the command a touch makes one with.
use crate::command::{Dimension, Effect, Outcome, Preview, StepInfo, StepInput, Unit, ViewCommand, plane_basis};
use egui::{Pos2, Rect, Vec2};
use ringdesign_core::{
    BuildResult, RingDesign, Vec3,
    cad::{Component, FaceRef, Feature, Operation, PlaneBase},
    sketch::Id,
};

/// A dragged offset moves in steps of this, mm.
pub const OFFSET_STEP_MM: f64 = 0.05;
/// The furthest a plane stands off its face, mm.
pub const MAX_OFFSET_MM: f64 = 50.0;

/// How far past the ring a plane through it reaches, mm.
pub const MARGIN_MM: f64 = 1.5;
/// Half the side of a plane laid square to the band or on a face, mm.
pub const PATCH_MM: f64 = 3.0;
/// Thinner than this on screen a plane is seen edge on, and only its name takes a finger, points.
pub const EDGE_ON_PT: f32 = 18.0;
/// How near a plane's outline a finger must land to take it, points.
pub const REACH_PT: f32 = 14.0;
/// How far round its name a finger still takes a plane, points.
pub const NAME_PAD_PT: f32 = 8.0;

/// A work plane as a view draws it: its feature, its name and its rectangle's corners in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub id: Id,
    pub name: String,
    pub corners: [[f64; 3]; 4],
}

/// The rectangle a plane on `base` through `origin` along `x` and `y` is drawn as over a ring `bounds` holds: a plane through the ring spans it, one on the band or a face is a patch round its origin.
fn rectangle(base: &PlaneBase, origin: [f64; 3], x: [f64; 3], y: [f64; 3], bounds: (Vec3, Vec3)) -> [[f64; 3]; 4] {
    let (lo, hi) = bounds;
    let reach = f64::from(lo.0.abs().max(hi.0.abs()).max(lo.1.abs()).max(hi.1.abs())) + MARGIN_MM;
    let (u, v) = match base {
        PlaneBase::Section { .. } => ([-reach, reach], [f64::from(lo.2) - MARGIN_MM - origin[2], f64::from(hi.2) + MARGIN_MM - origin[2]]),
        PlaneBase::Parting => ([-reach, reach], [-reach, reach]),
        PlaneBase::Tangent { .. } | PlaneBase::Face { .. } => ([-PATCH_MM, PATCH_MM], [-PATCH_MM, PATCH_MM]),
    };
    let at = |a: f64, b: f64| std::array::from_fn(|k| origin[k] + x[k] * a + y[k] * b);
    [at(u[0], v[0]), at(u[1], v[0]), at(u[1], v[1]), at(u[0], v[1])]
}

/// Every enabled work plane `built` carries, in the document's order: a plane through the ring spans it, one on the band or a face is a patch round its origin.
pub fn shapes(design: &RingDesign, built: &BuildResult) -> Vec<Shape> {
    let (Some(doc), Some(e)) = (design.cad.as_ref(), built.parts.evaluated.as_ref()) else { return Vec::new() };
    let bounds = built.mesh.bounds().unwrap_or_default();
    e.planes
        .iter()
        .filter_map(|p| {
            let f = doc.feature(p.id).filter(|f| f.enabled)?;
            let Operation::Plane { base, .. } = &f.operation else { return None };
            Some(Shape { id: p.id, name: f.name.clone(), corners: rectangle(base, p.origin, p.x, p.y, bounds) })
        })
        .collect()
}

/// Makes one work plane by touch: on a planar face of a part, square to the band at a point on it or on the parting plane, each moved along its normal by a drag or a typed offset, or through the finger's axis at a typed angle.
#[derive(Clone, Debug)]
pub struct PlaneCmd {
    base: PlaneBase,
    /// Where the arrow stands and the normal it points along, a drag along which reads the offset.
    along: Option<([f64; 3], [f64; 3])>,
    /// Where along the normal the drag was taken.
    anchor: Option<f64>,
    /// The offset a drag counts from.
    start: f64,
    value: f64,
    typed: bool,
    /// The part the face belongs to, which names the plane.
    part: String,
}

impl PlaneCmd {
    /// A plane on planar face `face` of part `feature` called `part`, the face centred at `centre` with outward `normal`.
    pub fn on_face(feature: Id, face: FaceRef, centre: [f64; 3], normal: [f64; 3], part: &str) -> Self {
        Self { base: PlaneBase::Face { feature, face }, along: Some((centre, normal)), anchor: None, start: 0.0, value: 0.0, typed: false, part: part.into() }
    }

    /// A plane through the finger's axis at `theta_deg` round the ring, read on 0–360° as the band's readout is.
    pub fn at_angle(theta_deg: f64) -> Self {
        let theta_deg = theta_deg.rem_euclid(360.0);
        Self { base: PlaneBase::Section { theta_deg }, along: None, anchor: None, start: theta_deg, value: theta_deg, typed: false, part: String::new() }
    }

    /// A plane square to the band at `at`, a point pressed on it with the surface's outward `normal` there.
    pub fn tangent(at: [f64; 3], normal: [f64; 3]) -> Self {
        let theta_deg = at[1].atan2(at[0]).to_degrees().rem_euclid(360.0);
        let base = PlaneBase::Tangent { theta_deg, across_mm: at[2] };
        Self { base, along: Some((at, unit(normal).unwrap_or([0.0, 0.0, 1.0]))), anchor: None, start: 0.0, value: 0.0, typed: false, part: String::new() }
    }

    /// The parting plane, world z = 0, stood `offset_mm` off it; its arrow rises through `at` along the finger's axis.
    pub fn parting(at: [f64; 3], offset_mm: f64) -> Self {
        let offset_mm = if offset_mm.is_finite() { offset_mm.clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM) } else { 0.0 };
        Self { base: PlaneBase::Parting, along: Some(([at[0], at[1], 0.0], [0.0, 0.0, 1.0])), anchor: None, start: offset_mm, value: offset_mm, typed: false, part: String::new() }
    }

    /// The field its number is typed into.
    pub fn key(&self) -> &'static str {
        if self.along.is_some() { "offset" } else { "angle" }
    }

    /// The face's centre and outward normal, for a plane on a face.
    pub fn along(&self) -> Option<([f64; 3], [f64; 3])> {
        self.along
    }

    /// The operation the plane is, with its number as it stands.
    pub fn operation(&self) -> Operation {
        match &self.base {
            PlaneBase::Section { .. } => Operation::Plane { base: PlaneBase::Section { theta_deg: self.value }, offset_mm: 0.0 },
            base => Operation::Plane { base: base.clone(), offset_mm: self.value },
        }
    }

    /// What the plane is called in the document.
    pub fn name(&self) -> String {
        match &self.base {
            PlaneBase::Section { .. } => self.base_name(),
            _ if self.value.abs() < 1e-9 => self.base_name(),
            _ => format!("{} {:+.2} mm", self.base_name(), self.value),
        }
    }

    /// What the plane is laid on, without its offset.
    fn base_name(&self) -> String {
        match &self.base {
            PlaneBase::Section { .. } => format!("Section at {:.0}°", self.value),
            PlaneBase::Tangent { theta_deg, .. } => format!("Tangent at {theta_deg:.0}°"),
            PlaneBase::Parting => "Parting".into(),
            PlaneBase::Face { .. } => format!("On {}", self.part),
        }
    }
}

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| v.map(|x| x / l))
}

/// The in-plane axes of a plane square to the band at `theta_deg` with normal `n`: round the ring, then `n × x`, as the build lays them.
fn tangent_axes(theta_deg: f64, n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let (s, c) = theta_deg.to_radians().sin_cos();
    let round = [-s, c, 0.0];
    let d = round[0] * n[0] + round[1] * n[1] + round[2] * n[2];
    match unit(std::array::from_fn(|k| round[k] - n[k] * d)) {
        Some(x) => (x, [n[1] * x[2] - n[2] * x[1], n[2] * x[0] - n[0] * x[2], n[0] * x[1] - n[1] * x[0]]),
        None => plane_basis(n),
    }
}

impl ViewCommand for PlaneCmd {
    fn key(&self) -> &'static str {
        "work-plane"
    }
    fn title(&self) -> String {
        "Work plane".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        let prompt = match &self.base {
            PlaneBase::Section { .. } => "type the angle round the ring, then Done",
            PlaneBase::Tangent { .. } => "drag the arrow or type the offset off the band, then Done",
            PlaneBase::Parting => "drag the arrow or type the height off the parting plane, then Done",
            PlaneBase::Face { .. } => "drag the arrow or type the offset from the face, then Done",
        };
        vec![StepInfo { name: "Work plane", prompt }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        let (label, unit) = if self.along.is_some() { ("Offset", Unit::Mm) } else { ("Angle", Unit::Deg) };
        vec![Dimension { key: PlaneCmd::key(self), label, unit, value: self.value, locked: self.typed }]
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        match input {
            StepInput::Pointer { world, .. } => {
                if let (Some((centre, normal)), false) = (self.along, self.typed) {
                    let along: f64 = (0..3).map(|k| (world[k] - centre[k]) * normal[k]).sum();
                    let from = *self.anchor.get_or_insert(along);
                    self.value = (self.start + ((along - from) / OFFSET_STEP_MM).round() * OFFSET_STEP_MM).clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM);
                }
                Outcome::Continue
            }
            StepInput::Typed { key, value } if *key == PlaneCmd::key(self) => {
                let ok = value.is_finite() && (self.along.is_none() || value.abs() <= MAX_OFFSET_MM);
                if !ok {
                    return Outcome::Refused(format!("An offset stands at most {MAX_OFFSET_MM} mm either way"));
                }
                self.value = if self.along.is_some() { *value } else { value.rem_euclid(360.0) };
                self.typed = true;
                Outcome::Continue
            }
            StepInput::Typed { key, .. } => Outcome::Refused(format!("No number called {key} here")),
            StepInput::Cleared { .. } => {
                self.typed = false;
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => {
                let feature = Feature { id: 0, name: self.name(), enabled: true, operation: self.operation(), component: Component::default() };
                Outcome::Commit(vec![Effect::Add { feature }])
            }
            StepInput::Lock(_) | StepInput::Unlock => Outcome::Refused("A work plane moves along its face's normal or round the ring".into()),
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
        }
    }
    fn preview(&self) -> Preview {
        let caption = match (&self.base, self.along) {
            (PlaneBase::Section { .. }, _) | (_, None) => self.name(),
            (PlaneBase::Tangent { .. }, Some(_)) => format!("{} · {:+.2} mm off the band", self.base_name(), self.value),
            (PlaneBase::Parting, Some(_)) => format!("{} · z {:+.2} mm", self.base_name(), self.value),
            (PlaneBase::Face { .. }, Some(_)) => format!("{} · {:+.2} mm off the face", self.base_name(), self.value),
        };
        Preview { operation: Some(self.operation()), caption, ..Preview::default() }
    }
}

/// The rectangle a work plane `operation` stands as over `built` before the build has it: a plane on a face or square to the band round `along`'s point moved along its normal, the parting plane or a section through the ring.
pub fn preview_shape(operation: &Operation, along: Option<([f64; 3], [f64; 3])>, built: &BuildResult) -> Option<[[f64; 3]; 4]> {
    let Operation::Plane { base, offset_mm } = operation else { return None };
    let moved = |centre: [f64; 3], normal: [f64; 3]| std::array::from_fn(|k| centre[k] + normal[k] * offset_mm);
    let (origin, x, y) = match (base, along) {
        (PlaneBase::Section { theta_deg }, _) => {
            let (s, c) = theta_deg.to_radians().sin_cos();
            ([0.0; 3], [c, s, 0.0], [0.0, 0.0, 1.0])
        }
        (PlaneBase::Parting, _) => ([0.0, 0.0, *offset_mm], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        (PlaneBase::Tangent { theta_deg, .. }, Some((centre, normal))) => {
            let (x, y) = tangent_axes(*theta_deg, normal);
            (moved(centre, normal), x, y)
        }
        (_, Some((centre, normal))) => {
            let (x, y) = plane_basis(normal);
            (moved(centre, normal), x, y)
        }
        _ => return None,
    };
    Some(rectangle(base, origin, x, y, built.mesh.bounds().unwrap_or_default()))
}

/// A plane on screen: its corners and the rectangle its name is written in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub id: Id,
    pub corners: [Pos2; 4],
    pub name: Rect,
}

impl Drawn {
    /// `shape` projected by `project`, its name `text` points in size written over its top corner, the left of two level ones;
    /// where that lands on one of `clear`, such as the navigator, over the next corner down clear of them all, else under what it lands on.
    pub fn new(shape: &Shape, project: impl Fn([f64; 3]) -> Pos2, text: Vec2, clear: &[Rect]) -> Self {
        let corners = shape.corners.map(project);
        let top = corners.iter().copied().fold(corners[0], |a, b| if b.y < a.y - 0.5 || ((b.y - a.y).abs() <= 0.5 && b.x < a.x) { b } else { a });
        let over = |c: Pos2| Rect::from_min_size(c + egui::vec2(4.0, -4.0 - text.y), text);
        let first = over(top);
        let blocked = |r: &Rect| clear.iter().find(|c| c.intersects(*r)).copied();
        let Some(hit) = blocked(&first) else { return Self { id: shape.id, corners, name: first } };
        let mut rest: Vec<Pos2> = corners.iter().copied().filter(|c| *c != top).collect();
        rest.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        let name = rest.into_iter().map(over).find(|r| blocked(r).is_none()).unwrap_or_else(|| first.translate(egui::vec2(0.0, hit.bottom() + 4.0 - first.top())));
        Self { id: shape.id, corners, name }
    }

    /// Whether the rectangle is thinner than [`EDGE_ON_PT`] across on screen: its area over its longer side.
    pub fn edge_on(&self) -> bool {
        let p = &self.corners;
        let (u, v) = (p[1] - p[0], p[3] - p[0]);
        let longest = u.length().max(v.length());
        longest <= f32::EPSILON || (u.x * v.y - u.y * v.x).abs() / longest < EDGE_ON_PT
    }

    /// How far `p` lies from the plane's outline, zero on its name; seen edge on only the name answers.
    pub fn distance(&self, p: Pos2) -> f32 {
        if self.name.expand(NAME_PAD_PT).contains(p) {
            return 0.0;
        }
        if self.edge_on() {
            return f32::INFINITY;
        }
        (0..4).map(|i| segment(p, self.corners[i], self.corners[(i + 1) % 4])).fold(f32::INFINITY, f32::min)
    }
}

fn segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 1e-9 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// The plane a finger at `p` takes: the nearest whose name or outline lies within `reach` points.
pub fn at(drawn: &[Drawn], p: Pos2, reach: f32) -> Option<Id> {
    drawn.iter().map(|d| (d.id, d.distance(p))).filter(|(_, d)| *d <= reach).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
}

/// The plane whose name lies under a finger at `p`: the one a plane answers by when a part under the finger outranks its outline.
pub fn name_at(drawn: &[Drawn], p: Pos2) -> Option<Id> {
    drawn.iter().find(|d| d.name.expand(NAME_PAD_PT).contains(p)).map(|d| d.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Attach, Component, Document, Feature, Placement},
        mesh, templates,
    };

    fn plane(id: Id, name: &str, base: PlaneBase) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation: Operation::Plane { base, offset_mm: 0.0 }, component: Component::default() }
    }

    /// The Court band with a post at its top and three work planes: through 0°, the parting plane, and square to the band at 90°.
    fn design() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let post = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: post }).unwrap();
        doc.append(plane(3, "Section at 0°", PlaneBase::Section { theta_deg: 0.0 })).unwrap();
        doc.append(plane(4, "Parting", PlaneBase::Parting)).unwrap();
        doc.append(plane(5, "Tangent at 90°", PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 })).unwrap();
        let mut off = plane(6, "Hidden", PlaneBase::Section { theta_deg: 45.0 });
        off.enabled = false;
        doc.append(off).unwrap();
        d.cad = Some(doc);
        d
    }

    #[test]
    fn a_plane_through_the_ring_spans_it_and_one_on_the_band_is_a_patch() {
        let d = design();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..BuildParams::default() });
        let s = shapes(&d, &built);
        assert_eq!(s.iter().map(|s| (s.id, s.name.as_str())).collect::<Vec<_>>(), [(3, "Section at 0°"), (4, "Parting"), (5, "Tangent at 90°")], "a disabled plane is not drawn");
        let (lo, hi) = built.mesh.bounds().unwrap();
        let reach = f64::from(lo.0.abs().max(hi.0.abs()).max(lo.1.abs()).max(hi.1.abs())) + MARGIN_MM;
        // The section through 0° lies in y = 0, past the ring by the margin every way.
        let section = &s[0].corners;
        assert!(section.iter().all(|c| c[1].abs() < 1e-9), "{section:?}");
        let xs: Vec<f64> = section.iter().map(|c| c[0]).collect();
        assert!((xs.iter().cloned().fold(f64::MIN, f64::max) - reach).abs() < 1e-9 && (xs.iter().cloned().fold(f64::MAX, f64::min) + reach).abs() < 1e-9, "{xs:?} against {reach}");
        let zs: Vec<f64> = section.iter().map(|c| c[2]).collect();
        assert!((zs.iter().cloned().fold(f64::MIN, f64::max) - (f64::from(hi.2) + MARGIN_MM)).abs() < 1e-9);
        // The parting plane lies in z = 0 and spans the ring both ways.
        assert!(s[1].corners.iter().all(|c| c[2].abs() < 1e-9 && (c[0].abs() - reach).abs() < 1e-9 && (c[1].abs() - reach).abs() < 1e-9), "{:?}", s[1].corners);
        // Square to the band at the top: a 6 mm patch standing on the crest, square to y.
        let top = &s[2].corners;
        let side = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt();
        assert!((side(top[0], top[1]) - 2.0 * PATCH_MM).abs() < 1e-9 && (side(top[1], top[2]) - 2.0 * PATCH_MM).abs() < 1e-9);
        let y = top[0][1];
        assert!(top.iter().all(|c| (c[1] - y).abs() < 1e-4) && y > 9.0, "{top:?}");
    }

    #[test]
    fn a_touch_makes_a_work_plane_off_a_flat_face_by_a_drag_or_a_typed_offset_or_through_the_axis_at_a_typed_angle() {
        use crate::command::Session;
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let block = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Block".into(), enabled: true, operation: Operation::Box { size: [4.0, 4.0, 2.0] }, component: block }).unwrap();
        d.cad = Some(doc);
        let params = BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..BuildParams::default() };
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params);
        let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap();
        let (face, centre, normal) = (0..c.trace.face_kind.len() as u32).filter_map(|f| ringdesign_core::cad::pattern::planar_face(c, f).ok()).find(|(_, _, n)| n[1] > 0.99).expect("the block's top");
        let mut s = Session::default();
        s.start(Box::new(PlaneCmd::on_face(2, face, centre, normal, "Block")));
        let at = |t: f64| StepInput::Pointer { world: std::array::from_fn(|k| centre[k] + normal[k] * (t + 0.3)), normal, theta_deg: 90.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: true };
        // A drag along the normal counts from where it was taken, in twentieths of a millimetre.
        s.feed(at(0.0));
        s.feed(at(0.73));
        assert_eq!(s.dimensions()[0].key, "offset");
        assert!((s.dimensions()[0].value - 0.75).abs() < 1e-9, "{:?}", s.dimensions());
        // A typed offset holds against the finger.
        assert!(matches!(s.feed(StepInput::Typed { key: "offset", value: 60.0 }), Outcome::Refused(_)));
        s.feed(StepInput::Typed { key: "offset", value: 0.5 });
        s.feed(at(2.0));
        assert_eq!((s.dimensions()[0].value, s.dimensions()[0].locked), (0.5, true));
        let corners = preview_shape(&s.preview().unwrap().operation.unwrap(), Some((centre, normal)), &built).unwrap();
        let off = |p: [f64; 3]| (0..3).map(|k| (p[k] - centre[k]) * normal[k]).sum::<f64>();
        assert!(corners.iter().all(|p| (off(*p) - 0.5).abs() < 1e-9), "the patch stands 0.5 mm off the face: {corners:?}");
        let Outcome::Commit(effects) = s.enter() else { panic!("Done makes the plane") };
        let (edits, added) = crate::touch::parts::effect_edits(&d, effects.clone());
        assert!(added && edits.len() == 1);
        let plain = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let (first, _) = crate::touch::parts::effect_edits(&plain, effects);
        use ringdesign_core::cad::edit::CadEdit;
        assert!(matches!(&first[..], [CadEdit::Add { feature: shank, .. }, CadEdit::Add { .. }] if matches!(shank.operation, Operation::Band)), "a plain ring's first plane brings its shank, or the ring would not build: {first:?}");
        let p = crate::touch::prepare(&d, &edits, built.parts.evaluated.as_ref()).unwrap().unwrap();
        assert_eq!(p.label, "Add On Block +0.50 mm");
        let id = p.applied[0].id.unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params);
        let plane = *after.parts.evaluated.as_ref().unwrap().plane(id).expect("the plane builds");
        let want: [f64; 3] = std::array::from_fn(|k| centre[k] + normal[k] * 0.5);
        assert!((0..3).all(|k| (plane.origin[k] - want[k]).abs() < 1e-6) && (0..3).map(|k| plane.normal[k] * normal[k]).sum::<f64>() > 0.999999, "{plane:?}");
        assert_eq!(shapes(&p.design, &after).iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["On Block +0.50 mm"]);
        // Through the finger's axis: the angle pressed and the one typed, both read on 0–360° as the band's readout is.
        assert_eq!((PlaneCmd::at_angle(-77.0).dimensions()[0].value, PlaneCmd::at_angle(-77.0).name()), (283.0, "Section at 283°".to_string()));
        s.start(Box::new(PlaneCmd::at_angle(88.0)));
        assert_eq!((s.dimensions()[0].key, s.dimensions()[0].value), ("angle", 88.0));
        s.feed(StepInput::Typed { key: "angle", value: -30.0 });
        assert_eq!(s.dimensions()[0].value, 330.0);
        s.feed(StepInput::Typed { key: "angle", value: 390.0 });
        assert_eq!(s.dimensions()[0].value, 30.0);
        let corners = preview_shape(&s.preview().unwrap().operation.unwrap(), None, &built).unwrap();
        let (sin, cos) = 30f64.to_radians().sin_cos();
        assert!(corners.iter().all(|p| (p[0] * sin - p[1] * cos).abs() < 1e-9), "the section stands through the axis at 30°");
        let Outcome::Commit(effects) = s.enter() else { panic!() };
        let [Effect::Add { feature }] = &effects[..] else { panic!("{effects:?}") };
        assert_eq!(feature.name, "Section at 30°");
        assert!(matches!(feature.operation, Operation::Plane { base: PlaneBase::Section { theta_deg }, offset_mm } if theta_deg == 30.0 && offset_mm == 0.0));
    }

    #[test]
    fn a_touch_lays_a_plane_square_to_the_band_where_it_pressed_and_one_on_the_parting_plane_each_offset_by_a_drag_or_a_number() {
        use crate::command::Session;
        let d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let params = BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..BuildParams::default() };
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params);
        // Pressed on the band 60° round and half a millimetre across.
        let (at, n) = ringdesign_core::cad::surface_hit(&built.mesh, 60.0, 0.5).expect("the band is there");
        let cmd = PlaneCmd::tangent(at, n);
        assert_eq!((cmd.key(), cmd.name()), ("offset", "Tangent at 60°".to_string()));
        let op = cmd.operation();
        assert!(matches!(op, Operation::Plane { base: PlaneBase::Tangent { theta_deg, across_mm }, offset_mm } if (theta_deg - 60.0).abs() < 1e-4 && (across_mm - 0.5).abs() < 1e-3 && offset_mm == 0.0), "{op:?} from {at:?}");
        let mut s = Session::default();
        s.start(Box::new(cmd));
        s.feed(StepInput::Typed { key: "offset", value: 0.3 });
        assert_eq!(s.preview().unwrap().caption, "Tangent at 60° · +0.30 mm off the band");
        let op = s.preview().unwrap().operation.unwrap();
        let corners = preview_shape(&op, Some((at, n)), &built).unwrap();
        let off = |p: [f64; 3]| (0..3).map(|k| (p[k] - at[k]) * n[k]).sum::<f64>();
        assert!(corners.iter().all(|p| (off(*p) - 0.3).abs() < 1e-9), "the patch stands 0.3 mm off the band: {corners:?}");
        let Outcome::Commit(effects) = s.enter() else { panic!("Done makes the plane") };
        let (edits, added) = crate::touch::parts::effect_edits(&d, effects);
        assert!(added && edits.len() == 2, "the plain ring's first plane brings its shank: {edits:?}");
        let p = crate::touch::prepare(&d, &edits, None).unwrap().unwrap();
        assert_eq!(p.label, "Add Procedural shank · Add Tangent at 60° +0.30 mm");
        let id = p.applied.last().and_then(|a| a.id).unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params);
        let plane = *after.parts.evaluated.as_ref().unwrap().plane(id).expect("the plane builds");
        // The build lays it where the preview drew it, the pressed point 0.3 mm out along the band's normal, to its own ray's 0.1 µm step along the finger.
        let want: [f64; 3] = std::array::from_fn(|k| at[k] + n[k] * 0.3);
        assert!((0..3).all(|k| (plane.origin[k] - want[k]).abs() < 2e-4), "{plane:?} against {want:?}");
        assert!((0..3).map(|k| plane.normal[k] * n[k]).sum::<f64>() > 0.999999);
        let centre: [f64; 3] = std::array::from_fn(|k| corners.iter().map(|c| c[k]).sum::<f64>() / 4.0);
        assert!((0..3).all(|k| (centre[k] - want[k]).abs() < 1e-9));
        let x = [corners[1][0] - corners[0][0], corners[1][1] - corners[0][1], corners[1][2] - corners[0][2]];
        assert!(((0..3).map(|k| x[k] * plane.x[k]).sum::<f64>() - 2.0 * PATCH_MM).abs() < 1e-6, "the patch's sides run round the ring as the plane's own x: {x:?} {:?}", plane.x);
        // On the parting plane: a drag up the finger's axis counts from where it was taken, in twentieths of a millimetre.
        let mut s = Session::default();
        s.start(Box::new(PlaneCmd::parting(at, 0.0)));
        let up = |z: f64| StepInput::Pointer { world: [at[0], at[1], z], normal: [0.0, 0.0, 1.0], theta_deg: 60.0, across_mm: z, height_mm: 0.0, snapped: None, dragging: true };
        s.feed(up(0.12));
        s.feed(up(0.63));
        assert_eq!((s.dimensions()[0].key, s.dimensions()[0].value), ("offset", 0.5));
        let op = s.preview().unwrap().operation.unwrap();
        let corners = preview_shape(&op, None, &built).unwrap();
        assert!(corners.iter().all(|c| (c[2] - 0.5).abs() < 1e-12), "{corners:?}");
        let reach = corners.iter().map(|c| c[0].abs().max(c[1].abs())).fold(0.0, f64::max);
        assert!(reach > 10.0, "it spans the ring: {reach}");
        let Outcome::Commit(effects) = s.enter() else { panic!() };
        let (edits, _) = crate::touch::parts::effect_edits(&d, effects);
        let p = crate::touch::prepare(&d, &edits, None).unwrap().unwrap();
        assert_eq!(p.label, "Add Procedural shank · Add Parting +0.50 mm");
        let id = p.applied.last().and_then(|a| a.id).unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params);
        let plane = *after.parts.evaluated.as_ref().unwrap().plane(id).unwrap();
        assert_eq!((plane.origin, plane.normal), ([0.0, 0.0, 0.5], [0.0, 0.0, 1.0]));
        // Where the verdict parts a mould off z = 0, the plane starts there and a drag counts on from it.
        let raised = PlaneCmd::parting(at, 0.3);
        assert_eq!((raised.name(), raised.dimensions()[0].value), ("Parting +0.30 mm".to_string(), 0.3));
        let mut s = Session::default();
        s.start(Box::new(raised));
        s.feed(up(0.0));
        s.feed(up(0.2));
        assert!((s.dimensions()[0].value - 0.5).abs() < 1e-12, "{:?}", s.dimensions());
        assert_eq!(PlaneCmd::parting(at, 0.0).name(), "Parting");
    }

    fn drawn(corners: [Pos2; 4]) -> Drawn {
        let shape = Shape { id: 7, name: "P".into(), corners: [[0.0; 3]; 4] };
        let mut d = Drawn::new(&shape, |_| Pos2::ZERO, vec2(30.0, 14.0), &[]);
        d.corners = corners;
        let top = corners.iter().copied().fold(corners[0], |a, b| if b.y < a.y - 0.5 || ((b.y - a.y).abs() <= 0.5 && b.x < a.x) { b } else { a });
        d.name = Rect::from_min_size(top + vec2(4.0, -18.0), vec2(30.0, 14.0));
        d
    }

    #[test]
    fn its_name_stands_over_the_top_corner_the_left_of_two_level_ones() {
        let shape = Shape { id: 3, name: "Section".into(), corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]] };
        // Screen y grows down, so the corners at z = 1 stand highest; of those two the left one carries the name.
        let d = Drawn::new(&shape, |c| pos2(100.0 + c[0] as f32 * 80.0, 300.0 - c[2] as f32 * 60.0), vec2(50.0, 14.0), &[]);
        assert_eq!(d.corners, [pos2(100.0, 300.0), pos2(180.0, 300.0), pos2(180.0, 240.0), pos2(100.0, 240.0)]);
        assert_eq!(d.name, Rect::from_min_size(pos2(104.0, 222.0), vec2(50.0, 14.0)));
    }

    #[test]
    fn its_name_moves_clear_of_the_navigator_to_the_next_corner_down_or_under_it() {
        let shape = Shape { id: 3, name: "Section".into(), corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]] };
        let project = |c: [f64; 3]| pos2(100.0 + c[0] as f32 * 80.0, 300.0 - c[2] as f32 * 60.0);
        // A 92-point navigator whose corner covers the top-left corner's name.
        let navigator = Rect::from_min_size(pos2(60.0, 150.0), vec2(92.0, 92.0));
        let d = Drawn::new(&shape, project, vec2(50.0, 14.0), &[navigator]);
        assert_eq!(d.name, Rect::from_min_size(pos2(184.0, 222.0), vec2(50.0, 14.0)), "over the top-right corner");
        assert!(!d.name.intersects(navigator));
        // Covering the whole top edge sends it to the lower-left corner.
        let wide = Rect::from_min_size(pos2(60.0, 150.0), vec2(200.0, 92.0));
        let d = Drawn::new(&shape, project, vec2(50.0, 14.0), &[wide]);
        assert_eq!(d.name, Rect::from_min_size(pos2(104.0, 282.0), vec2(50.0, 14.0)));
        // Covering every corner, it stands under what it landed on, beside the top corner.
        let all = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 305.0));
        let d = Drawn::new(&shape, project, vec2(50.0, 14.0), &[all]);
        assert_eq!(d.name, Rect::from_min_size(pos2(104.0, 309.0), vec2(50.0, 14.0)));
        // A finger still takes the plane by the moved name.
        let d = Drawn::new(&shape, project, vec2(50.0, 14.0), &[navigator]);
        assert_eq!(name_at(&[d], d.name.center()), Some(3));
    }

    #[test]
    fn a_finger_takes_a_plane_by_its_outline_or_its_name_and_one_seen_edge_on_by_its_name_alone() {
        let square = drawn([pos2(100.0, 100.0), pos2(300.0, 100.0), pos2(300.0, 300.0), pos2(100.0, 300.0)]);
        assert!(!square.edge_on());
        // 10 pt outside its right side, inside the reach; its middle is the ring's, not the plane's.
        assert_eq!(at(&[square], pos2(310.0, 200.0), REACH_PT), Some(7));
        assert_eq!(at(&[square], pos2(200.0, 200.0), REACH_PT), None);
        assert_eq!(at(&[square], pos2(318.0, 200.0), REACH_PT), None, "18 pt off is past a finger's reach of 14");
        // Its name answers anywhere on it, and a finger's pad round it.
        assert_eq!(square.distance(pos2(120.0, 88.0)), 0.0);
        assert_eq!(square.distance(pos2(120.0, 76.0)), 0.0, "{:?}", square.name);
        // Seen edge on it is a line across the ring: only its name takes a finger.
        let line = drawn([pos2(100.0, 200.0), pos2(300.0, 204.0), pos2(300.0, 210.0), pos2(100.0, 206.0)]);
        assert!(line.edge_on());
        assert_eq!(at(&[line], pos2(200.0, 203.0), REACH_PT), None);
        assert_eq!(at(&[line], line.name.center(), REACH_PT), Some(7));
        // By name alone, as when a part under the finger outranks the outline.
        assert_eq!(name_at(&[square], pos2(310.0, 200.0)), None, "the outline does not answer");
        assert_eq!(name_at(&[square], square.name.center()), Some(7));
        // Two in reach: the nearer outline wins.
        let mut near = square;
        near.id = 8;
        near.corners = square.corners.map(|c| c + vec2(12.0, 0.0));
        near.name = square.name.translate(vec2(0.0, -300.0));
        assert_eq!(at(&[square, near], pos2(313.0, 200.0), REACH_PT), Some(8), "1 pt from the second's side, 13 from the first's");
        assert_eq!(at(&[square, near], pos2(301.0, 200.0), REACH_PT), Some(7));
        assert_eq!(at(&[], pos2(0.0, 0.0), REACH_PT), None);
    }
}
