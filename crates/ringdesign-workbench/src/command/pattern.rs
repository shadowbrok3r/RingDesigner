//! Commands that repeat or reshape a part: arrays, mirrors and press-pull.
use super::session::{Dimension, Effect, Outcome, Preview, StepInfo, StepInput, Unit, ViewCommand};
use ringdesign_core::cad::{
    Attach, Component, FaceRef, Feature, Operation, PatternKind, Placement, Profile,
    pattern::{MAX_PATTERN_COUNT, MAX_PULL_MM, MIN_PULL_MM},
};
use ringdesign_core::sketch::Id;

/// Instances an array starts with, its source among them.
pub const DEFAULT_COUNT: u32 = 6;
/// Thinnest a pulled primitive may be left, mm.
const MIN_SIZE_MM: f64 = 0.01;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// What copies of `source` are called: "Ring array of Post".
pub fn pattern_name(kind: &PatternKind, source: &Feature) -> String {
    match kind {
        PatternKind::Ring { .. } => format!("Ring array of {}", source.name),
        PatternKind::About { .. } => format!("Array of {}", source.name),
        PatternKind::Mirror { .. } => format!("Mirror of {}", source.name),
    }
}

/// The feature a pattern gesture adds, named and staged for its source and meeting the band by `attach`.
pub fn pattern_feature(id: Id, source: &Feature, attach: Attach, kind: PatternKind) -> Feature {
    let component = Component { placement: Placement::Free, attach, ..source.component.clone() };
    Feature { id, name: pattern_name(&kind, source), enabled: true, operation: Operation::Pattern { source: source.id, kind }, component }
}

/// Copies of a part round the ring, or round a stone or part: the count and the span typed in the dimension bar, Enter adds them.
pub struct ArrayCmd {
    id: Id,
    source: Feature,
    attach: Attach,
    /// The part an array round a stone turns about; `None` turns round the finger.
    about: Option<Id>,
    count: Option<u32>,
    span: Option<f64>,
}

impl ArrayCmd {
    /// An array of `source` added as feature `id`, meeting the band by `attach`.
    pub fn new(id: Id, source: Feature, attach: Attach, about: Option<Id>) -> Self {
        Self { id, source, attach, about, count: None, span: None }
    }
    pub fn count(&self) -> u32 {
        self.count.unwrap_or(DEFAULT_COUNT)
    }
    pub fn span_deg(&self) -> f64 {
        self.span.unwrap_or(360.0)
    }
    pub fn kind(&self) -> PatternKind {
        let (count, span_deg) = (self.count(), self.span_deg());
        match self.about {
            Some(part) => PatternKind::About { part, count, span_deg },
            None => PatternKind::Ring { count, span_deg },
        }
    }
    /// The feature Enter adds.
    pub fn feature(&self) -> Feature {
        pattern_feature(self.id, &self.source, self.attach, self.kind())
    }
    /// Degrees between neighbouring instances: a whole turn shares it out, an open arc ends on its last.
    fn step(&self) -> f64 {
        let (count, span) = (f64::from(self.count()), self.span_deg());
        if span.abs() >= 360.0 - 1e-9 { span / count } else { span / (count - 1.0) }
    }
}

impl ViewCommand for ArrayCmd {
    fn key(&self) -> &'static str {
        "array"
    }
    fn title(&self) -> String {
        match self.about {
            Some(_) => "Array round the stone".into(),
            None => "Array round the ring".into(),
        }
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Array", prompt: "Type how many; Tab reaches the span; Enter or click adds the copies" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        vec![
            Dimension { key: "count", label: "Instances", unit: Unit::Count, value: f64::from(self.count()), locked: self.count.is_some() },
            Dimension { key: "span", label: "Span", unit: Unit::Deg, value: self.span_deg(), locked: self.span.is_some() },
        ]
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        match input {
            StepInput::Typed { key: "count", value } => {
                if !(value.is_finite() && value.fract() == 0.0 && (2.0..=f64::from(MAX_PATTERN_COUNT)).contains(value)) {
                    return Outcome::Refused(format!("count: a whole number from 2 to {MAX_PATTERN_COUNT}, the part among them"));
                }
                self.count = Some(*value as u32);
                Outcome::Continue
            }
            StepInput::Typed { key: "span", value } => {
                if !(value.is_finite() && value.abs() > 1e-6 && value.abs() <= 360.0) {
                    return Outcome::Refused("span: more than 0° and at most 360° either way".into());
                }
                self.span = Some(*value);
                Outcome::Continue
            }
            StepInput::Typed { key, .. } => Outcome::Refused(format!("No dimension named {key}")),
            StepInput::Cleared { key } => {
                match *key {
                    "count" => self.count = None,
                    "span" => self.span = None,
                    _ => {}
                }
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => Outcome::Commit(vec![Effect::Add { feature: self.feature() }]),
            StepInput::Lock(_) | StepInput::Unlock => Outcome::Refused("An array has no axis lock".into()),
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Pointer { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        let arc = if self.span_deg().abs() >= 360.0 - 1e-9 { String::new() } else { format!(" over {:.0}°", self.span_deg()) };
        Preview {
            placement: None,
            operation: Some(self.feature().operation),
            ghost: Vec::new(),
            caption: format!("{}: {} in all{arc}, a copy every {:.1}°", self.title(), self.count(), self.step()),
        }
    }
}

/// Which parameter of a primitive a pulled face moves, when one does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pull {
    /// A box's size along its own axis, its seat moved half the pull along the normal so the far face stays.
    BoxSize { axis: usize, sign: f64 },
    /// A cylinder's height, its seat moved half the pull along the normal.
    CylinderHeight { sign: f64 },
    /// An extrusion's height: its base stays on its sketch, and a cut's far face is the one below it.
    ExtrudeHeight,
}

/// The parameter a pull on `face` of `f` sizes; `None` for a face only the kernel can push.
pub fn pull_of(f: &Feature, face: &FaceRef) -> Option<Pull> {
    let n = face.signature.as_ref()?.normal;
    let upright = matches!(f.component.placement, Placement::Ring { tilt_deg, cant_deg, .. } if tilt_deg == 0.0 && cant_deg == 0.0);
    let along_z = n[2].abs() > 1.0 - 1e-6;
    match &f.operation {
        Operation::Box { .. } if upright && along_z => Some(Pull::BoxSize { axis: 2, sign: n[2].signum() }),
        Operation::Cylinder { .. } if upright && along_z => Some(Pull::CylinderHeight { sign: n[2].signum() }),
        Operation::Extrude { sketch: Profile::Inline(s), height_mm, .. } if s.plane.on_face.is_none() => {
            let normal = s.plane.plane().ok()?.normal()?;
            (dot(normal, n) * height_mm.signum() > 1.0 - 1e-6).then_some(Pull::ExtrudeHeight)
        }
        _ => None,
    }
}

/// A seat moved `by` along its normal.
fn lifted(p: &Placement, by: f64) -> Placement {
    match p.clone() {
        Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => {
            Placement::Ring { theta_deg, across_mm, height_mm: height_mm + by, spin_deg, tilt_deg, cant_deg }
        }
        Placement::Free => Placement::Free,
    }
}

/// Pushes or pulls a planar face along its outward normal, as a size edit or a Press-pull feature.
pub struct PressPullCmd {
    target: Feature,
    face: FaceRef,
    attach: Attach,
    centre: [f64; 3],
    normal: [f64; 3],
    fresh_id: Id,
    pull: Option<Pull>,
    anchor: Option<f64>,
    pointer: f64,
    typed: Option<f64>,
}

impl PressPullCmd {
    /// A pull on signed `face` of `target` at its world `centre` and `normal`; a kernel pull becomes feature `fresh_id`.
    pub fn new(target: Feature, face: FaceRef, attach: Attach, centre: [f64; 3], normal: [f64; 3], fresh_id: Id) -> Self {
        let pull = pull_of(&target, &face);
        Self { target, face, attach, centre, normal, fresh_id, pull, anchor: None, pointer: 0.0, typed: None }
    }
    /// How far the face goes out along its normal; negative pushes it in.
    pub fn distance(&self) -> f64 {
        self.typed.unwrap_or(self.pointer)
    }
    pub fn pull(&self) -> Option<Pull> {
        self.pull
    }
    /// The feature the face lies on.
    pub fn target(&self) -> &Feature {
        &self.target
    }
    /// The face's centre and outward normal in the world.
    pub fn face_at(&self) -> ([f64; 3], [f64; 3]) {
        (self.centre, self.normal)
    }
    /// A size grown by `d`, or why the push would flatten the part.
    fn grown(what: &str, size: f64, d: f64) -> Result<f64, String> {
        let next = size + d;
        if next < MIN_SIZE_MM {
            return Err(format!("Pushing {:.2} mm would flatten the {what}; it is {size:.2} mm through", -d));
        }
        Ok(next)
    }
    /// The edits a commit makes: the primitive's size and seat, or a new Press-pull feature.
    pub fn effects(&self) -> Result<Vec<Effect>, String> {
        let d = self.distance();
        if !(d.is_finite() && d.abs() >= MIN_PULL_MM) {
            return Err("Drag the face or type a distance".into());
        }
        let feature = self.target.id;
        let placement = &self.target.component.placement;
        Ok(match (self.pull, &self.target.operation) {
            (Some(Pull::BoxSize { axis, sign }), Operation::Box { size }) => {
                let mut size = *size;
                size[axis] = Self::grown("box", size[axis], d)?;
                vec![Effect::Operation { feature, operation: Operation::Box { size } }, Effect::Placement { feature, placement: lifted(placement, sign * d / 2.0) }]
            }
            (Some(Pull::CylinderHeight { sign }), Operation::Cylinder { radius_mm, height_mm }) => vec![
                Effect::Operation { feature, operation: Operation::Cylinder { radius_mm: *radius_mm, height_mm: Self::grown("cylinder", *height_mm, d)? } },
                Effect::Placement { feature, placement: lifted(placement, sign * d / 2.0) },
            ],
            (Some(Pull::ExtrudeHeight), Operation::Extrude { sketch, height_mm, draft_deg }) => vec![Effect::Operation {
                feature,
                operation: Operation::Extrude { sketch: sketch.clone(), height_mm: height_mm.signum() * Self::grown("extrusion", height_mm.abs(), d)?, draft_deg: *draft_deg },
            }],
            _ => {
                let operation = Operation::PressPull { source: feature, face: self.face.clone(), distance_mm: d };
                let component = Component { placement: Placement::Free, attach: self.attach, ..self.target.component.clone() };
                vec![Effect::Add { feature: Feature { id: self.fresh_id, name: format!("Press-pull of {}", self.target.name), enabled: true, operation, component } }]
            }
        })
    }
}

impl ViewCommand for PressPullCmd {
    fn key(&self) -> &'static str {
        "press-pull"
    }
    fn title(&self) -> String {
        "Press-pull".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Press-pull", prompt: "Drag along the face's normal or type the distance, negative to push in; Enter or click sets it" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        vec![Dimension { key: "distance", label: "Distance", unit: Unit::Mm, value: self.distance(), locked: self.typed.is_some() }]
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        match input {
            StepInput::Pointer { world, .. } => {
                let along = dot(std::array::from_fn(|k| world[k] - self.centre[k]), self.normal);
                let from = *self.anchor.get_or_insert(along);
                self.pointer = along - from;
                Outcome::Continue
            }
            StepInput::Typed { key: "distance", value } => {
                if !(value.is_finite() && value.abs() <= MAX_PULL_MM) {
                    return Outcome::Refused(format!("distance: at most {MAX_PULL_MM} mm either way"));
                }
                self.typed = Some(*value);
                Outcome::Continue
            }
            StepInput::Typed { key, .. } => Outcome::Refused(format!("No dimension named {key}")),
            StepInput::Cleared { .. } => {
                self.typed = None;
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => match self.effects() {
                Ok(effects) => Outcome::Commit(effects),
                Err(why) => Outcome::Refused(why),
            },
            StepInput::Lock(_) | StepInput::Unlock => Outcome::Refused("A press-pull moves along its face's normal".into()),
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
        }
    }
    fn preview(&self) -> Preview {
        let d = self.distance();
        let tip: [f64; 3] = std::array::from_fn(|k| self.centre[k] + self.normal[k] * d);
        let what = match self.pull {
            Some(_) => format!("sizes {}", self.target.name),
            None => "a Press-pull feature".into(),
        };
        let (operation, placement) = match self.effects().ok().as_deref() {
            Some([Effect::Operation { operation, .. }, Effect::Placement { placement, .. }]) => (Some(operation.clone()), Some(placement.clone())),
            Some([Effect::Operation { operation, .. }]) => (Some(operation.clone()), None),
            Some([Effect::Add { feature }]) => (Some(feature.operation.clone()), None),
            _ => (None, None),
        };
        Preview { placement, operation, ghost: vec![self.centre, tip], caption: format!("Press-pull {} {:.2} mm · {what}", if d < 0.0 { "in" } else { "out" }, d.abs()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::session::Session;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, RingDesign,
        cad::{self, Document, FeatureStatus, MirrorPlane, PlaneBase, face_signature},
        sketch::Sketch,
    };

    fn part(id: Id, name: &str, operation: Operation, placement: Placement) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation, component: Component { attach: Attach::Join, placement, ..Component::default() } }
    }
    fn effects(o: Outcome) -> Vec<Effect> {
        match o {
            Outcome::Commit(e) => e,
            other => panic!("expected a commit, got {other:?}"),
        }
    }
    fn refused(o: Outcome) -> String {
        match o {
            Outcome::Refused(why) => why,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
    /// The one feature a commit adds.
    fn added(o: Outcome) -> Feature {
        match effects(o).as_slice() {
            [Effect::Add { feature }] => feature.clone(),
            other => panic!("expected one added feature, got {other:?}"),
        }
    }
    /// The operation and the seat a commit writes to one feature.
    fn sized(o: Outcome) -> (Operation, Placement) {
        match effects(o).as_slice() {
            [Effect::Operation { operation, .. }, Effect::Placement { placement, .. }] => (operation.clone(), placement.clone()),
            other => panic!("expected an operation and a placement, got {other:?}"),
        }
    }
    /// The part evaluated alone on the default ring, and the ordinal and signed reference of its face looking along `dir` in its own frame.
    fn face(f: &Feature, dir: [f64; 3]) -> (usize, FaceRef) {
        let mut doc = Document::default();
        doc.append(f.clone()).unwrap();
        let e = cad::evaluate(&RingDesign { cad: Some(doc), ..RingDesign::default() }, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        let c = &e.components[0];
        let i = (0..c.body.faces.len()).find(|i| face_signature(&c.body, *i, &c.frame).is_some_and(|s| dot(s.normal, dir) > 0.99)).unwrap();
        (i, FaceRef::signed(&c.body, i, &c.frame))
    }

    #[test]
    fn an_array_types_its_count_and_span_and_adds_one_pattern_feature_named_for_its_source() {
        let post = part(4, "Post", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, Placement::ring(90.0, 0.5));
        let mut s = Session::default();
        s.start(Box::new(ArrayCmd::new(9, post.clone(), Attach::Join, None)));
        assert_eq!(s.prompt(), "Array round the ring: Type how many; Tab reaches the span; Enter or click adds the copies");
        assert_eq!(s.preview().unwrap().caption, "Array round the ring: 6 in all, a copy every 60.0°");
        assert_eq!(refused(s.feed(StepInput::Typed { key: "count", value: 1.0 })), "count: a whole number from 2 to 120, the part among them");
        assert!(s.feed(StepInput::Typed { key: "count", value: 2.5 }).is_refused());
        assert!(s.feed(StepInput::Typed { key: "count", value: 121.0 }).is_refused());
        assert!(s.feed(StepInput::Typed { key: "span", value: 0.0 }).is_refused());
        assert!(matches!(s.feed(StepInput::Typed { key: "count", value: 3.0 }), Outcome::Continue));
        assert_eq!(s.dimensions().iter().map(|d| (d.key, d.value, d.locked)).collect::<Vec<_>>(), [("count", 3.0, true), ("span", 360.0, false)]);
        assert_eq!(s.preview().unwrap().caption, "Array round the ring: 3 in all, a copy every 120.0°");
        s.feed(StepInput::Typed { key: "span", value: 90.0 });
        assert_eq!(s.preview().unwrap().caption, "Array round the ring: 3 in all over 90°, a copy every 45.0°");
        let feature = added(s.enter());
        assert_eq!((feature.id, feature.name.as_str(), feature.component.attach, feature.component.placement.clone()), (9, "Ring array of Post", Attach::Join, Placement::Free));
        assert!(matches!(&feature.operation, Operation::Pattern { source: 4, kind: PatternKind::Ring { count: 3, span_deg } } if *span_deg == 90.0));
        assert!(!s.is_live(), "one commit ends it");
        // Round a stone the count is typed the same way; Escape clears it, then cancels.
        let mut s = Session::default();
        s.start(Box::new(ArrayCmd::new(9, post.clone(), Attach::Join, Some(2))));
        s.feed(StepInput::Typed { key: "count", value: 4.0 });
        assert!(matches!(s.escape(), Outcome::Continue) && s.dimensions()[0].value == 6.0, "the typed count clears first");
        assert!(matches!(s.escape(), Outcome::Cancelled));
        let mut c = ArrayCmd::new(9, post.clone(), Attach::Separate, Some(2));
        c.feed(&StepInput::Typed { key: "count", value: 6.0 });
        let feature = added(c.feed(&StepInput::Click));
        assert_eq!(feature.name, "Array of Post");
        assert!(matches!(feature.operation, Operation::Pattern { source: 4, kind: PatternKind::About { part: 2, count: 6, .. } }));
        assert_eq!(feature.component.attach, Attach::Separate, "an array meets the band as the build reads its source");
        let mirror = pattern_feature(11, &post, Attach::Join, PatternKind::Mirror { plane: MirrorPlane::Band });
        assert_eq!((mirror.name.as_str(), mirror.operation.sources()), ("Mirror of Post", vec![4]));
    }

    #[test]
    fn a_pulled_end_of_a_seated_box_sizes_it_and_holds_its_far_face_and_any_other_face_becomes_a_press_pull() {
        let seat = Placement::ring(90.0, 0.8);
        let block = part(2, "Block", Operation::Box { size: [2.0, 1.5, 1.0] }, seat.clone());
        let (_, top) = face(&block, [0.0, 0.0, 1.0]);
        let (_, bottom) = face(&block, [0.0, 0.0, -1.0]);
        let (side_ordinal, side) = face(&block, [1.0, 0.0, 0.0]);
        assert_eq!(pull_of(&block, &top), Some(Pull::BoxSize { axis: 2, sign: 1.0 }));
        assert_eq!(pull_of(&block, &bottom), Some(Pull::BoxSize { axis: 2, sign: -1.0 }));
        assert_eq!(pull_of(&block, &side), None);
        let mut s = Session::default();
        s.start(Box::new(PressPullCmd::new(block.clone(), top.clone(), Attach::Join, [0.0, 10.5, 0.0], [0.0, 1.0, 0.0], 7)));
        assert_eq!(refused(s.enter()), "Drag the face or type a distance");
        // The pointer reads along the normal from where it first stood.
        s.feed(StepInput::Pointer { world: [0.0, 10.5, 0.0], normal: [0.0, 1.0, 0.0], theta_deg: 90.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: false });
        s.feed(StepInput::Pointer { world: [0.3, 11.3, 0.1], normal: [0.0, 1.0, 0.0], theta_deg: 88.0, across_mm: 0.1, height_mm: 0.8, snapped: None, dragging: false });
        assert!((s.dimensions()[0].value - 0.8).abs() < 1e-12);
        assert!(matches!(s.feed(StepInput::Lock(crate::command::Axis::X)), Outcome::Refused(_)));
        s.feed(StepInput::Typed { key: "distance", value: 0.5 });
        let p = s.preview().unwrap();
        assert_eq!((p.caption.as_str(), p.ghost.len()), ("Press-pull out 0.50 mm · sizes Block", 2));
        let committed = effects(s.enter());
        let [Effect::Operation { feature: 2, operation: Operation::Box { size } }, Effect::Placement { feature: 2, placement }] = committed.as_slice() else { panic!("{committed:?}") };
        let height = |p: &Placement| match p {
            Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 } => *height_mm,
            other => panic!("{other:?}"),
        };
        assert!(*size == [2.0, 1.5, 1.5] && (height(placement) - 1.05).abs() < 1e-12, "the top rises 0.5 and the bottom stays: {size:?} {placement:?}");
        // The bottom pulled out goes down, and a push that would flatten the box is refused by name.
        let mut c = PressPullCmd::new(block.clone(), bottom, Attach::Join, [0.0; 3], [0.0, -1.0, 0.0], 7);
        c.feed(&StepInput::Typed { key: "distance", value: 0.4 });
        let (_, placement) = sized(c.feed(&StepInput::Confirm));
        assert!((height(&placement) - 0.6).abs() < 1e-12, "{placement:?}");
        let mut c = PressPullCmd::new(block.clone(), top.clone(), Attach::Join, [0.0; 3], [0.0, 1.0, 0.0], 7);
        c.feed(&StepInput::Typed { key: "distance", value: -1.5 });
        assert_eq!(refused(c.feed(&StepInput::Confirm)), "Pushing 1.50 mm would flatten the box; it is 1.00 mm through");
        // A side face, a tilted box and a free one are pushed by the kernel.
        let mut c = PressPullCmd::new(block.clone(), side.clone(), Attach::Join, [0.0; 3], [1.0, 0.0, 0.0], 7);
        c.feed(&StepInput::Typed { key: "distance", value: 0.3 });
        let feature = added(c.feed(&StepInput::Confirm));
        assert_eq!((feature.id, feature.name.as_str(), feature.component.placement.clone(), feature.component.attach), (7, "Press-pull of Block", Placement::Free, Attach::Join));
        assert!(matches!(&feature.operation, Operation::PressPull { source: 2, face, distance_mm } if face.ordinal == side_ordinal && *distance_mm == 0.3));
        let tilted = Feature { component: Component { placement: Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 0.8, spin_deg: 0.0, tilt_deg: 10.0, cant_deg: 0.0 }, ..block.component.clone() }, ..block.clone() };
        assert_eq!(pull_of(&tilted, &face(&tilted, [0.0, 0.0, 1.0]).1), None);
        let free = Feature { component: Component::default(), ..block.clone() };
        assert_eq!(pull_of(&free, &face(&free, [0.0, 0.0, 1.0]).1), None);
        // A cylinder's end and an extrusion's far cap are sizes too; its base is the kernel's.
        let drum = part(3, "Drum", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, seat.clone());
        assert_eq!(pull_of(&drum, &face(&drum, [0.0, 0.0, 1.0]).1), Some(Pull::CylinderHeight { sign: 1.0 }));
        let boss = part(5, "Boss", Operation::Extrude { sketch: Sketch::rectangle(2.0, 1.0).into(), height_mm: 1.5, draft_deg: 0.0 }, Placement::Free);
        assert_eq!(pull_of(&boss, &face(&boss, [0.0, 0.0, 1.0]).1), Some(Pull::ExtrudeHeight));
        assert_eq!(pull_of(&boss, &face(&boss, [0.0, 0.0, -1.0]).1), None);
        // A cut's far cap faces down: pulled out 0.5 its 1.5 mm grows to 2.0 and stays a cut; its plane's cap is the kernel's.
        let pocket = part(6, "Pocket", Operation::Extrude { sketch: Sketch::rectangle(2.0, 1.0).into(), height_mm: -1.5, draft_deg: 0.0 }, Placement::Free);
        let (_, floor) = face(&pocket, [0.0, 0.0, -1.0]);
        assert_eq!(pull_of(&pocket, &floor), Some(Pull::ExtrudeHeight));
        assert_eq!(pull_of(&pocket, &face(&pocket, [0.0, 0.0, 1.0]).1), None);
        let mut c = PressPullCmd::new(pocket.clone(), floor.clone(), Attach::Cut, [0.0, 0.0, -1.5], [0.0, 0.0, -1.0], 7);
        c.feed(&StepInput::Typed { key: "distance", value: 0.5 });
        let e = effects(c.feed(&StepInput::Confirm));
        assert!(matches!(e.as_slice(), [Effect::Operation { feature: 6, operation: Operation::Extrude { height_mm, .. } }] if *height_mm == -2.0), "{e:?}");
        let mut c = PressPullCmd::new(pocket, floor, Attach::Cut, [0.0, 0.0, -1.5], [0.0, 0.0, -1.0], 7);
        c.feed(&StepInput::Typed { key: "distance", value: -2.0 });
        assert_eq!(refused(c.feed(&StepInput::Confirm)), "Pushing 2.00 mm would flatten the extrusion; it is 1.50 mm through");
    }

    #[test]
    fn sizing_a_box_and_pushing_its_face_through_the_kernel_are_the_same_metal() {
        let lib = AlphaLibrary::builtin();
        let block = part(2, "Block", Operation::Box { size: [2.0, 1.5, 1.0] }, Placement::ring(90.0, 0.8));
        let (_, top) = face(&block, [0.0, 0.0, 1.0]);
        let volume = |features: Vec<Feature>| {
            let mut doc = Document::default();
            for f in features {
                doc.append(f).unwrap();
            }
            let e = cad::evaluate(&RingDesign { cad: Some(doc), ..RingDesign::default() }, &lib, BuildParams::default()).unwrap();
            assert!(e.failures().is_empty(), "{:?}", e.failures());
            let c = e.components.last().unwrap();
            (c.mesh.volume_mm3(), c.mesh.bounds().unwrap())
        };
        let mut c = PressPullCmd::new(block.clone(), top.clone(), Attach::Join, [0.0; 3], [0.0, 1.0, 0.0], 7);
        c.feed(&StepInput::Typed { key: "distance", value: 0.5 });
        let (operation, placement) = sized(c.feed(&StepInput::Confirm));
        let sized = volume(vec![Feature { operation, component: Component { placement, ..block.component.clone() }, ..block.clone() }]);
        let pushed = volume(vec![block.clone(), part(7, "Pull", Operation::PressPull { source: 2, face: top, distance_mm: 0.5 }, Placement::Free)]);
        assert!((sized.0 - 4.5).abs() < 1e-6 && (pushed.0 - 4.5).abs() < 1e-6, "{} {}", sized.0, pushed.0);
        let (a, b) = (sized.1, pushed.1);
        for (p, q) in [(a.0, b.0), (a.1, b.1)] {
            assert!((p.0 - q.0).abs() < 1e-5 && (p.1 - q.1).abs() < 1e-5 && (p.2 - q.2).abs() < 1e-5, "{a:?} against {b:?}");
        }
    }

    #[test]
    fn a_work_planes_chip_carries_its_mark_and_a_press_pull_whose_face_is_gone_shows_why() {
        let lib = AlphaLibrary::builtin();
        let block = part(2, "Block", Operation::Box { size: [2.0, 1.5, 1.0] }, Placement::Free);
        let (ordinal, top) = face(&block, [0.0, 0.0, 1.0]);
        let mut doc = Document::default();
        doc.append(block.clone()).unwrap();
        doc.append(part(3, "Section", Operation::Plane { base: PlaneBase::Section { theta_deg: 90.0 }, offset_mm: 0.0 }, Placement::Free)).unwrap();
        doc.append(part(4, "Pull", Operation::PressPull { source: 2, face: top, distance_mm: 0.5 }, Placement::Free)).unwrap();
        let chips = |doc: &Document| {
            let e = cad::evaluate(&RingDesign { cad: Some(doc.clone()), ..RingDesign::default() }, &lib, BuildParams::default()).unwrap();
            crate::timeline::chips(doc, Some(&e), &[])
        };
        let ok = chips(&doc);
        assert_eq!((ok[1].icon, &ok[1].status, ok[1].label()), (crate::icons::Icon::Section, &crate::timeline::Status::Ok, "Section · ok".to_string()));
        assert_eq!((ok[2].icon, &ok[2].status), (crate::icons::Icon::Raise, &crate::timeline::Status::Ok));
        // The block become a sphere has no top: the pull fails in the reference's own words, and its chip says so.
        doc.features[0].operation = Operation::Sphere { radius_mm: 1.0 };
        let gone = chips(&doc);
        let expected = format!("Press-pull face: Face {ordinal} is no longer a Plane face; the source changed underneath, pick it again");
        assert_eq!(gone[2].status, crate::timeline::Status::Failed(expected.clone()));
        let e = cad::evaluate(&RingDesign { cad: Some(doc.clone()), ..RingDesign::default() }, &lib, BuildParams::default()).unwrap();
        assert_eq!(e.status_of(4), Some(&FeatureStatus::Failed(expected)));
    }
}
