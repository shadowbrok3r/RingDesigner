//! Move, rotate and scale in the ring frame, place on the ring, click-drag primitives, the attach cycle and grip drags.
use super::ring::{Affine, angle_about, transform};
use super::session::{Axis, CommandInfo, Dimension, Effect, Outcome, Preview, StepInfo, StepInput, Unit, ViewCommand};
use super::snap::{RingPoint, wrap360};
use crate::gizmo::{Dial, Gizmo, Handle};
use crate::grips::{self, Grip};
use crate::icons::Icon;
use ringdesign_core::cad::{Attach, Component, Document, EvaluatedComponent, FaceSeat, Feature, Operation, Placement, builders};

/// One degree of freedom: the pointer's value unless a typed one holds it.
#[derive(Clone, Debug)]
struct Dof {
    key: &'static str,
    label: &'static str,
    unit: Unit,
    axis: Axis,
    /// Whether only a size above zero may be typed.
    positive: bool,
    pointer: f64,
    typed: Option<f64>,
}
impl Dof {
    fn new(axis: Axis, key: &'static str, label: &'static str, unit: Unit, pointer: f64) -> Self {
        Self { key, label, unit, axis, positive: false, pointer, typed: None }
    }
    fn size(axis: Axis, key: &'static str, label: &'static str, pointer: f64) -> Self {
        Self { positive: true, ..Self::new(axis, key, label, Unit::Mm, pointer) }
    }
    fn value(&self) -> f64 {
        self.typed.unwrap_or(self.pointer)
    }
    fn dimension(&self) -> Dimension {
        Dimension { key: self.key, label: self.label, unit: self.unit, value: self.value(), locked: self.typed.is_some() }
    }
}
/// Applies a typed or cleared value to the degree of freedom it names; `None` for any other token.
fn edit(dofs: &mut [Dof], input: &StepInput) -> Option<Outcome> {
    match input {
        StepInput::Typed { key, value } => Some(match dofs.iter_mut().find(|d| d.key == *key) {
            Some(_) if !value.is_finite() => Outcome::Refused(format!("{key}: not a number")),
            Some(d) if d.positive && *value <= 0.0 => Outcome::Refused(format!("{key}: must be above zero")),
            Some(d) => {
                d.typed = Some(*value);
                Outcome::Continue
            }
            None => Outcome::Refused(format!("No dimension named {key}")),
        }),
        StepInput::Cleared { key } => {
            if let Some(d) = dofs.iter_mut().find(|d| d.key == *key) {
                d.typed = None;
            }
            Some(Outcome::Continue)
        }
        _ => None,
    }
}
/// An angle in [-180, 180], unchanged when already inside.
fn wrap180(d: f64) -> f64 {
    if (-180.0..=180.0).contains(&d) { d } else { (d + 180.0).rem_euclid(360.0) - 180.0 }
}
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn lock_note(lock: Option<Axis>) -> String {
    lock.map(|a| format!(" · {} locked", a.key())).unwrap_or_default()
}
fn snap_note(snapped: &Option<String>) -> String {
    snapped.as_ref().map(|s| format!(" · {s}")).unwrap_or_default()
}
fn locks_only(title: &str, dofs: &[Dof]) -> Outcome {
    let names: Vec<_> = dofs.iter().map(|d| d.axis.key()).collect();
    Outcome::Refused(format!("{title} locks {}", names.join(", ")))
}
fn caption(dofs: &[Dof]) -> String {
    dofs.iter().map(|d| format!("{} {}", d.label, d.unit.format(d.value()))).collect::<Vec<_>>().join(" · ")
}
/// A new `Transform` feature standing for a moved or turned free part, carrying the part's component.
fn wrapped(id: u64, source: u64, component: &Component, translation: [f64; 3], rotation_deg: [f64; 3]) -> Feature {
    let operation = Operation::Transform { source, translation, rotation_deg };
    Feature { id, name: operation.label().into(), enabled: true, operation, component: component.clone() }
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The stone `f` is built round, when `f` is a builder part standing on one: G, R, P and the gizmo on `f` act on that stone and `f` follows it.
pub fn moved_by<'a>(doc: &'a Document, f: &Feature) -> Option<&'a Feature> {
    let Operation::Builder { key, on: Some(stone), .. } = &f.operation else { return None };
    let s = doc.feature(*stone)?;
    let is_stone = matches!(&s.operation, Operation::Builder { key, .. } if key == builders::STONE);
    (key != builders::STONE && is_stone).then_some(s)
}

/// A stone standing on a part's face as the build seated it: its feature, its seat, and the face's own axes at the stone.
#[derive(Clone, Debug)]
pub struct FaceHold {
    pub feature: Feature,
    pub seat: FaceSeat,
    /// The girdle's centre in the world.
    pub origin: [f64; 3],
    /// Along the face as the seat's `u` runs, in the world.
    pub along: [f64; 3],
    /// Across the face as the seat's `v` runs, in the world.
    pub across: [f64; 3],
    /// The face's outward normal, which the stone's table faces along.
    pub normal: [f64; 3],
}

impl FaceHold {
    /// `f` held where its build `c` seated it, when it is a stone standing on a part's face.
    pub fn of(f: &Feature, c: &EvaluatedComponent) -> Option<Self> {
        let Operation::Builder { key, on: Some(_), params } = &f.operation else { return None };
        if key != builders::STONE || c.id != f.id {
            return None;
        }
        let seat = FaceSeat::of(params).ok()??;
        let (s, co) = seat.spin_deg.to_radians().sin_cos();
        let (x, y) = (c.frame.x_axis, c.frame.y_axis);
        let along = std::array::from_fn(|k| x[k] * co - y[k] * s);
        let across = std::array::from_fn(|k| x[k] * s + y[k] * co);
        Some(Self { feature: f.clone(), seat, origin: c.frame.origin, along, across, normal: c.frame.z_axis })
    }

    /// Where the seat meets the face, under the girdle's centre.
    pub fn foot(&self) -> [f64; 3] {
        std::array::from_fn(|k| self.origin[k] - self.normal[k] * self.seat.height_mm)
    }

    /// The seat slid `du` along and `dv` across the face, lifted `dh` off it and spun `dspin` degrees about its normal.
    pub fn moved(&self, du: f64, dv: f64, dh: f64, dspin: f64) -> FaceSeat {
        let s = &self.seat;
        FaceSeat { face: s.face.clone(), u_mm: s.u_mm + du, v_mm: s.v_mm + dv, height_mm: s.height_mm + dh, spin_deg: wrap180(s.spin_deg + dspin) }
    }

    /// The stone's builder standing on `seat`.
    pub fn operation(&self, seat: &FaceSeat) -> Operation {
        let mut op = self.feature.operation.clone();
        if let Operation::Builder { params, .. } = &mut op {
            seat.write(params);
        }
        op
    }

    /// The seat a stone's builder operation carries.
    pub fn seat_in(op: &Operation) -> Option<FaceSeat> {
        match op {
            Operation::Builder { params, .. } => FaceSeat::of(params).ok().flatten(),
            _ => None,
        }
    }

    /// The stone's frame standing on `seat` of the face as held.
    fn frame(&self, seat: &FaceSeat) -> Affine {
        let (s, c) = seat.spin_deg.to_radians().sin_cos();
        let x = std::array::from_fn(|k| self.along[k] * c + self.across[k] * s);
        let y = std::array::from_fn(|k| self.across[k] * c - self.along[k] * s);
        let foot = self.foot();
        let (du, dv) = (seat.u_mm - self.seat.u_mm, seat.v_mm - self.seat.v_mm);
        let origin = std::array::from_fn(|k| foot[k] + self.along[k] * du + self.across[k] * dv + self.normal[k] * seat.height_mm);
        Affine::frame(x, y, self.normal, origin)
    }

    /// The map carrying the stone as held to where `seat` stands it.
    pub fn map(&self, seat: &FaceSeat) -> Option<Affine> {
        Some(self.frame(seat).mul(&self.frame(&self.seat).inverse()?))
    }

    /// The map carrying the stone to where a command's preview stands it.
    pub fn ghost(&self, preview: &Preview) -> Option<Affine> {
        self.map(&Self::seat_in(preview.operation.as_ref()?)?)
    }

    /// The gizmo on the stone: arrows along, across and off the face, the ring spinning it on the face, and the dial round the finger through its foot.
    pub fn gizmo(&self, reach_mm: f64) -> Gizmo {
        let foot = self.foot();
        Gizmo {
            origin: self.origin,
            frame: self.frame(&self.seat),
            arrows: vec![(Axis::X, self.along), (Axis::Y, self.across), (Axis::Z, self.normal)],
            rings: vec![(Axis::Spin, self.normal)],
            dial: Some(Dial { z: foot[2], radius: foot[0].hypot(foot[1]), theta_deg: wrap360(foot[1].atan2(foot[0]).to_degrees()), height_mm: 0.0 }),
            grips: Vec::new(),
            reach_mm,
            height_mm: 0.0,
        }
    }

    /// The command a drag of the face gizmo's `handle` runs, the axis it locks, and the dimension a number typed during it goes to.
    pub fn drag(&self, handle: Handle) -> Option<(Box<dyn ViewCommand>, Option<Axis>, &'static str)> {
        Some(match handle {
            Handle::Move(axis) => (Box::new(MoveCmd::on_face(self.clone())), Some(axis), match axis {
                Axis::X => "u",
                Axis::Y => "v",
                _ => "height",
            }),
            Handle::Turn(_) => (Box::new(RotateCmd::on_face(self.clone())), Some(Axis::Spin), "spin"),
            // Round the finger through the foot, dropped along the normal onto the face.
            Handle::Dial => (Box::new(MoveCmd::on_face(self.clone())), None, "u"),
            Handle::Grip(_) => return None,
        })
    }
}

/// Within this many mm or degrees a moved value reads as where it began or where it snapped.
const SETTLE: f64 = 1e-4;

/// Where the pointer was when a command first saw it.
#[derive(Clone, Copy, Debug)]
struct Anchor {
    world: [f64; 3],
    theta: f64,
    across: f64,
    height: f64,
}

/// How a free part's move lands: its own `Transform` edited, or the part wrapped in a new one.
#[derive(Clone, Debug)]
enum FreeMove {
    Wrap(Component),
    Edit { source: u64, translation: [f64; 3], rotation_deg: [f64; 3] },
}

/// G: slide a seated part round the ring, across it and off its surface; a free part moves in world x, y, z.
pub struct MoveCmd {
    feature: u64,
    base: Placement,
    fresh_id: u64,
    free: FreeMove,
    anchor: Option<Anchor>,
    world: [f64; 3],
    delta: [f64; 3],
    lock: Option<Axis>,
    dofs: Vec<Dof>,
    /// What the landing last snapped to, and where.
    snapped: Option<String>,
    landed: Option<RingPoint>,
    /// A stone standing on a part's face, which slides on that face.
    face: Option<FaceHold>,
}
impl MoveCmd {
    /// Slides a stone standing on a part's face along and across it, lifts it off it on a locked normal, and writes its seat.
    pub fn on_face(hold: FaceHold) -> Self {
        let dofs = vec![
            Dof::new(Axis::X, "u", "Δalong", Unit::Mm, 0.0),
            Dof::new(Axis::Y, "v", "Δacross", Unit::Mm, 0.0),
            Dof::new(Axis::Z, "height", "Δheight", Unit::Mm, 0.0),
        ];
        Self { face: Some(hold.clone()), dofs, ..Self::new(hold.feature.id, Placement::Free, hold.feature.id) }
    }
    /// A free part's move becomes a new `Transform` feature numbered `fresh_id`.
    pub fn new(feature: u64, base: Placement, fresh_id: u64) -> Self {
        let dofs = match base {
            Placement::Ring { .. } => vec![
                Dof::new(Axis::Theta, "theta", "Δθ", Unit::Deg, 0.0),
                Dof::new(Axis::Across, "across", "Δacross", Unit::Mm, 0.0),
                Dof::new(Axis::Height, "height", "Δheight", Unit::Mm, 0.0),
            ],
            Placement::Free => vec![
                Dof::new(Axis::X, "x", "Δx", Unit::Mm, 0.0),
                Dof::new(Axis::Y, "y", "Δy", Unit::Mm, 0.0),
                Dof::new(Axis::Z, "z", "Δz", Unit::Mm, 0.0),
            ],
        };
        let free = FreeMove::Wrap(Component::default());
        Self { feature, base, fresh_id, free, anchor: None, world: [0.0; 3], delta: [0.0; 3], lock: None, dofs, snapped: None, landed: None, face: None }
    }
    /// Moves a document feature: a free `Transform` is edited in place, any other free part keeps its component.
    pub fn of(f: &Feature, fresh_id: u64) -> Self {
        let mut cmd = Self::new(f.id, f.component.placement.clone(), fresh_id);
        cmd.free = match f.operation {
            Operation::Transform { source, translation, rotation_deg } => FreeMove::Edit { source, translation, rotation_deg },
            _ => FreeMove::Wrap(f.component.clone()),
        };
        cmd
    }
    fn apply_lock(&mut self) {
        // Unlocked, a stone on a face slides on it and keeps its height.
        let sliding = self.face.is_some();
        for (i, d) in self.dofs.iter_mut().enumerate() {
            let follows = self.lock.map_or(!(sliding && d.axis == Axis::Z), |l| l == d.axis);
            d.pointer = if follows { self.delta[i] } else { 0.0 };
        }
    }
    fn effect(&self) -> Effect {
        let v = |i: usize| self.dofs[i].value();
        if let Some(h) = &self.face {
            return Effect::Operation { feature: self.feature, operation: h.operation(&h.moved(v(0), v(1), v(2), 0.0)) };
        }
        // A value within `SETTLE` of where it began, or of where the landing snapped, takes that value.
        let settle = |value: f64, from: f64, k: usize| {
            let landed = self.landed.map(|p| [p.theta_deg, p.across_mm, p.height_mm][k]);
            let off = |at: f64| if k == 0 { wrap180(value - at) } else { value - at };
            std::iter::once(from).chain(landed).find(|at| off(*at).abs() < SETTLE).unwrap_or(value)
        };
        match (&self.base, &self.free) {
            (Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg }, _) => Effect::Placement {
                feature: self.feature,
                placement: Placement::Ring {
                    theta_deg: settle(wrap360(theta_deg + v(0)), *theta_deg, 0),
                    across_mm: settle(across_mm + v(1), *across_mm, 1),
                    height_mm: settle(height_mm + v(2), *height_mm, 2),
                    spin_deg: *spin_deg,
                    tilt_deg: *tilt_deg,
                    cant_deg: *cant_deg,
                },
            },
            (Placement::Free, FreeMove::Edit { source, translation, rotation_deg }) => Effect::Operation {
                feature: self.feature,
                operation: Operation::Transform {
                    source: *source,
                    translation: std::array::from_fn(|k| translation[k] + v(k)),
                    rotation_deg: *rotation_deg,
                },
            },
            (Placement::Free, FreeMove::Wrap(component)) => {
                Effect::Add { feature: wrapped(self.fresh_id, self.feature, component, [v(0), v(1), v(2)], [0.0; 3]) }
            }
        }
    }
}
impl ViewCommand for MoveCmd {
    fn key(&self) -> &'static str {
        "move"
    }
    fn title(&self) -> String {
        "Move".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Move", prompt: "Drag; lock an axis; type a distance; Enter or click confirms" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        self.dofs.iter().map(Dof::dimension).collect()
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        if let Some(o) = edit(&mut self.dofs, input) {
            return o;
        }
        match *input {
            StepInput::Pointer { world, theta_deg, across_mm, height_mm, ref snapped, .. } => {
                let a = *self.anchor.get_or_insert(Anchor { world, theta: theta_deg, across: across_mm, height: height_mm });
                let d: [f64; 3] = std::array::from_fn(|k| world[k] - a.world[k]);
                self.delta = match (&self.face, &self.base) {
                    (Some(h), _) => [dot(d, h.along), dot(d, h.across), dot(d, h.normal)],
                    (None, Placement::Ring { .. }) => [wrap180(theta_deg - a.theta), across_mm - a.across, height_mm - a.height],
                    (None, Placement::Free) => d,
                };
                self.world = world;
                self.snapped = snapped.as_ref().map(|s| s.label.clone());
                self.landed = snapped.as_ref().map(|s| s.ring);
                self.apply_lock();
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => Outcome::Commit(vec![self.effect()]),
            StepInput::Lock(axis) => {
                if !self.dofs.iter().any(|d| d.axis == axis) {
                    return locks_only("Move", &self.dofs);
                }
                self.lock = Some(axis);
                self.apply_lock();
                Outcome::Continue
            }
            StepInput::Unlock => {
                self.lock = None;
                self.apply_lock();
                Outcome::Continue
            }
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Typed { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        let mut p = Preview::default();
        match self.effect() {
            Effect::Placement { placement, .. } => p.placement = Some(placement),
            Effect::Operation { operation, .. } => p.operation = Some(operation),
            Effect::Add { feature } => p.operation = Some(feature.operation),
            Effect::Attach { .. } | Effect::Stage { .. } => {}
        }
        if let Some(a) = self.anchor {
            p.ghost = vec![a.world, self.world];
        }
        p.caption = format!("Move {}{}{}", caption(&self.dofs), snap_note(&self.snapped), lock_note(self.lock));
        p
    }
}

/// A gizmo ring's centre and the world axis each of a turn's three dimensions is about, in their order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pivot {
    pub centre: [f64; 3],
    pub axes: [[f64; 3]; 3],
}

/// R: spin a seated part about its normal, lean it along (tilt) or across (cant) the ring; a free part turns about a world axis.
pub struct RotateCmd {
    feature: u64,
    base: Placement,
    fresh_id: u64,
    component: Component,
    anchor: Option<Anchor>,
    world: [f64; 3],
    /// The pointer's sweep since the anchor for each of the three turns, in degrees.
    sweep: [f64; 3],
    lock: Option<Axis>,
    dofs: Vec<Dof>,
    /// Read the pointer as its angle about these axes instead of off the ring frame.
    pivot: Option<Pivot>,
    /// A stone standing on a part's face, which spins on that face.
    face: Option<FaceHold>,
}
impl RotateCmd {
    /// Spins a stone standing on a part's face about the face's normal through its girdle's centre, and writes its seat.
    pub fn on_face(hold: FaceHold) -> Self {
        let pivot = Pivot { centre: hold.origin, axes: [hold.normal; 3] };
        let dofs = vec![Dof::new(Axis::Spin, "spin", "Δspin", Unit::Deg, 0.0)];
        Self { pivot: Some(pivot), face: Some(hold.clone()), dofs, ..Self::new(hold.feature.id, Placement::Free, hold.feature.id) }
    }
    /// A free part's turn becomes a new `Transform` feature numbered `fresh_id`.
    pub fn new(feature: u64, base: Placement, fresh_id: u64) -> Self {
        let dofs = match base {
            Placement::Ring { .. } => vec![
                Dof::new(Axis::Spin, "spin", "Δspin", Unit::Deg, 0.0),
                Dof::new(Axis::Tilt, "tilt", "Δtilt", Unit::Deg, 0.0),
                Dof::new(Axis::Cant, "cant", "Δcant", Unit::Deg, 0.0),
            ],
            Placement::Free => vec![
                Dof::new(Axis::X, "x", "Δx", Unit::Deg, 0.0),
                Dof::new(Axis::Y, "y", "Δy", Unit::Deg, 0.0),
                Dof::new(Axis::Z, "z", "Δz", Unit::Deg, 0.0),
            ],
        };
        let component = Component::default();
        Self { feature, base, fresh_id, component, anchor: None, world: [0.0; 3], sweep: [0.0; 3], lock: None, dofs, pivot: None, face: None }
    }
    /// Turns a document feature; a free part keeps its component on the `Transform` that turns it.
    pub fn of(f: &Feature, fresh_id: u64) -> Self {
        Self { component: f.component.clone(), ..Self::new(f.id, f.component.placement.clone(), fresh_id) }
    }
    /// Reads the pointer as its angle about the pivot's axes, as a gizmo ring is dragged; a free part then turns about the pivot's centre.
    pub fn about(self, pivot: Pivot) -> Self {
        Self { pivot: Some(pivot), ..self }
    }
    /// The axis the pointer turns: the lock, else spin on the ring or on a face and z in the world.
    fn active(&self) -> Axis {
        self.lock.unwrap_or(match self.base {
            _ if self.face.is_some() => Axis::Spin,
            Placement::Ring { .. } => Axis::Spin,
            Placement::Free => Axis::Z,
        })
    }
    fn apply_sweep(&mut self) {
        let active = self.active();
        for (d, s) in self.dofs.iter_mut().zip(self.sweep) {
            d.pointer = if d.axis == active { s } else { 0.0 };
        }
    }
    /// Degrees swept since the anchor: round the seat (spin), toward the pointer (tilt, cant), or round each world axis (free).
    fn sweeps(&self, a: Anchor, now: Anchor) -> [f64; 3] {
        if let Some(p) = self.pivot {
            return std::array::from_fn(|k| wrap180(angle_about(now.world, p.centre, p.axes[k]) - angle_about(a.world, p.centre, p.axes[k])));
        }
        let deg = |from: f64, to: f64| wrap180((to - from).to_degrees());
        match self.base {
            Placement::Ring { theta_deg, across_mm, .. } => {
                // Round the seat in the part's own x (−finger) and y (+θ) plane, the arc at the pointer's radius.
                let round = |p: Anchor| {
                    let r = p.world[0].hypot(p.world[1]);
                    (r * wrap180(p.theta - theta_deg).to_radians()).atan2(-(p.across - across_mm))
                };
                let r = now.world[0].hypot(now.world[1]).max(1e-9);
                [deg(round(a), round(now)), -wrap180(now.theta - a.theta), -(now.across - a.across).atan2(r).to_degrees()]
            }
            Placement::Free => {
                let about = |p: [f64; 3]| [p[2].atan2(p[1]), p[0].atan2(p[2]), p[1].atan2(p[0])];
                let (from, to) = (about(a.world), about(now.world));
                std::array::from_fn(|k| deg(from[k], to[k]))
            }
        }
    }
    fn effect(&self) -> Effect {
        let v = |i: usize| self.dofs[i].value();
        if let Some(h) = &self.face {
            return Effect::Operation { feature: self.feature, operation: h.operation(&h.moved(0.0, 0.0, 0.0, v(0))) };
        }
        match self.base {
            Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => Effect::Placement {
                feature: self.feature,
                placement: Placement::Ring {
                    theta_deg,
                    across_mm,
                    height_mm,
                    spin_deg: wrap180(spin_deg + v(0)),
                    tilt_deg: wrap180(tilt_deg + v(1)),
                    cant_deg: wrap180(cant_deg + v(2)),
                },
            },
            Placement::Free => {
                let rotation = [v(0), v(1), v(2)];
                // About a pivot the turn carries its centre back to where it was.
                let translation = self.pivot.map_or([0.0; 3], |p| {
                    let turned = transform([0.0; 3], rotation).apply(p.centre);
                    std::array::from_fn(|k| p.centre[k] - turned[k])
                });
                Effect::Add { feature: wrapped(self.fresh_id, self.feature, &self.component, translation, rotation) }
            }
        }
    }
}
impl ViewCommand for RotateCmd {
    fn key(&self) -> &'static str {
        "rotate"
    }
    fn title(&self) -> String {
        "Rotate".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Rotate", prompt: "Circle the part to spin it; lock tilt or cant to lean it; type degrees; Enter or click confirms" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        self.dofs.iter().map(Dof::dimension).collect()
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        if let Some(o) = edit(&mut self.dofs, input) {
            return o;
        }
        match *input {
            StepInput::Pointer { world, theta_deg, across_mm, height_mm, .. } => {
                let now = Anchor { world, theta: theta_deg, across: across_mm, height: height_mm };
                let a = *self.anchor.get_or_insert(now);
                self.sweep = self.sweeps(a, now);
                self.world = world;
                self.apply_sweep();
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => Outcome::Commit(vec![self.effect()]),
            StepInput::Lock(axis) => {
                if !self.dofs.iter().any(|d| d.axis == axis) {
                    return locks_only("Rotate", &self.dofs);
                }
                self.lock = Some(axis);
                self.apply_sweep();
                Outcome::Continue
            }
            StepInput::Unlock => {
                self.lock = None;
                self.apply_sweep();
                Outcome::Continue
            }
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Typed { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        let mut p = Preview::default();
        match self.effect() {
            Effect::Placement { placement, .. } => p.placement = Some(placement),
            Effect::Add { feature } => p.operation = Some(feature.operation),
            Effect::Operation { operation, .. } => p.operation = Some(operation),
            Effect::Attach { .. } | Effect::Stage { .. } => {}
        }
        if let Some(a) = self.anchor {
            p.ghost = vec![a.world, self.world];
        }
        p.caption = format!("Rotate {}{}", caption(&self.dofs), lock_note(self.lock));
        p
    }
}

/// S: resize a primitive by the pointer's distance ratio from its pivot, a typed factor or a typed size; x, y, z locks narrow the factor.
pub struct ScaleCmd {
    feature: u64,
    op: Operation,
    pivot: [f64; 3],
    anchor: Option<f64>,
    world: [f64; 3],
    factor: f64,
    typed_factor: Option<f64>,
    lock: Option<Axis>,
    /// Per dimension: its size at the start and the axis locks the factor reaches it through.
    base: Vec<(f64, &'static [Axis])>,
    dofs: Vec<Dof>,
}
const MIN_DIM_MM: f64 = 0.01;
const XYZ: &[Axis] = &[Axis::X, Axis::Y, Axis::Z];
impl ScaleCmd {
    /// `None` for an operation with no size of its own; `pivot` is the part's centre in the world.
    pub fn new(feature: u64, op: Operation, pivot: [f64; 3]) -> Option<Self> {
        let d = |axis, key, label, v: f64, axes: &'static [Axis]| (Dof::size(axis, key, label, v), (v, axes));
        let dims = match &op {
            Operation::Box { size } => vec![
                d(Axis::X, "x", "X", size[0], &[Axis::X]),
                d(Axis::Y, "y", "Y", size[1], &[Axis::Y]),
                d(Axis::Z, "z", "Z", size[2], &[Axis::Z]),
            ],
            Operation::Cylinder { radius_mm, height_mm } => {
                vec![d(Axis::X, "radius", "Radius", *radius_mm, &[Axis::X, Axis::Y]), d(Axis::Z, "height", "Height", *height_mm, &[Axis::Z])]
            }
            Operation::Sphere { radius_mm } => vec![d(Axis::X, "radius", "Radius", *radius_mm, XYZ)],
            Operation::Torus { major_mm, minor_mm } => {
                vec![d(Axis::X, "minor", "Minor", *minor_mm, XYZ), d(Axis::Y, "major", "Major", *major_mm, &[])]
            }
            // A cut runs against its plane's normal: its size is the depth, its sign kept apart.
            Operation::Extrude { height_mm, .. } => vec![d(Axis::Z, "height", "Height", height_mm.abs(), &[Axis::Z])],
            _ => return None,
        };
        let (dofs, base) = dims.into_iter().unzip();
        Some(Self { feature, op, pivot, anchor: None, world: pivot, factor: 1.0, typed_factor: None, lock: None, base, dofs })
    }
    fn factor(&self) -> f64 {
        self.typed_factor.unwrap_or(self.factor).max(MIN_DIM_MM)
    }
    fn apply_factor(&mut self) {
        let f = self.factor();
        for (d, (size, axes)) in self.dofs.iter_mut().zip(&self.base) {
            let reached = self.lock.map_or(!axes.is_empty(), |l| axes.contains(&l));
            d.pointer = if reached { size * f } else { *size };
        }
    }
    fn operation(&self) -> Operation {
        let v = |i: usize| self.dofs[i].value().max(MIN_DIM_MM);
        match &self.op {
            Operation::Box { .. } => Operation::Box { size: [v(0), v(1), v(2)] },
            Operation::Cylinder { .. } => Operation::Cylinder { radius_mm: v(0), height_mm: v(1) },
            Operation::Sphere { .. } => Operation::Sphere { radius_mm: v(0) },
            Operation::Torus { .. } => Operation::Torus { minor_mm: v(0), major_mm: v(1) },
            Operation::Extrude { sketch, height_mm, draft_deg } => Operation::Extrude { sketch: sketch.clone(), height_mm: height_mm.signum() * v(0), draft_deg: *draft_deg },
            other => other.clone(),
        }
    }
}
impl ViewCommand for ScaleCmd {
    fn key(&self) -> &'static str {
        "scale"
    }
    fn title(&self) -> String {
        "Scale".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Scale", prompt: "Drag from the part; lock an axis; type a factor or a size; Enter or click confirms" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        let factor = Dimension { key: "factor", label: "Factor", unit: Unit::Ratio, value: self.factor(), locked: self.typed_factor.is_some() };
        std::iter::once(factor).chain(self.dofs.iter().map(Dof::dimension)).collect()
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        match input {
            StepInput::Typed { key: "factor", value } => {
                if !(value.is_finite() && *value > 0.0) {
                    return Outcome::Refused("factor: must be above zero".into());
                }
                self.typed_factor = Some(*value);
                self.apply_factor();
                return Outcome::Continue;
            }
            StepInput::Cleared { key: "factor" } => {
                self.typed_factor = None;
                self.apply_factor();
                return Outcome::Continue;
            }
            _ => {}
        }
        if let Some(o) = edit(&mut self.dofs, input) {
            return o;
        }
        match *input {
            StepInput::Pointer { world, .. } => {
                let d = dist(world, self.pivot);
                let a = *self.anchor.get_or_insert(d.max(1e-9));
                self.factor = d / a;
                self.world = world;
                self.apply_factor();
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => {
                Outcome::Commit(vec![Effect::Operation { feature: self.feature, operation: self.operation() }])
            }
            StepInput::Lock(axis) => {
                if !XYZ.contains(&axis) {
                    return Outcome::Refused("Scale locks x, y or z".into());
                }
                if !self.base.iter().any(|(_, axes)| axes.contains(&axis)) {
                    return Outcome::Refused(format!("{} has no size along {}", self.op.label(), axis.key()));
                }
                self.lock = Some(axis);
                self.apply_factor();
                Outcome::Continue
            }
            StepInput::Unlock => {
                self.lock = None;
                self.apply_factor();
                Outcome::Continue
            }
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Typed { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        Preview {
            placement: None,
            operation: Some(self.operation()),
            ghost: if self.anchor.is_some() { vec![self.pivot, self.world] } else { vec![] },
            caption: format!("Scale ×{:.3} {}{}", self.factor(), caption(&self.dofs), lock_note(self.lock)),
        }
    }
}

/// Seat a part where the pointer meets the ring, keeping its spin, tilt and cant.
pub struct PlaceCmd {
    feature: u64,
    kept: [f64; 3],
    /// Whether the part already stands on the ring.
    seated: bool,
    world: Option<[f64; 3]>,
    snapped: Option<String>,
    lock: Option<Axis>,
    dofs: Vec<Dof>,
}
impl PlaceCmd {
    pub fn new(feature: u64, base: Placement) -> Self {
        let (at, kept, seated) = match base {
            Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => {
                ([theta_deg, across_mm, height_mm], [spin_deg, tilt_deg, cant_deg], true)
            }
            Placement::Free => ([90.0, 0.0, 0.0], [0.0; 3], false),
        };
        let dofs = vec![
            Dof::new(Axis::Theta, "theta", "θ", Unit::Deg, at[0]),
            Dof::new(Axis::Across, "across", "Across", Unit::Mm, at[1]),
            Dof::new(Axis::Height, "height", "Height", Unit::Mm, at[2]),
        ];
        Self { feature, kept, seated, world: None, snapped: None, lock: None, dofs }
    }
    fn placement(&self) -> Placement {
        Placement::Ring {
            theta_deg: wrap360(self.dofs[0].value()),
            across_mm: self.dofs[1].value(),
            height_mm: self.dofs[2].value(),
            spin_deg: self.kept[0],
            tilt_deg: self.kept[1],
            cant_deg: self.kept[2],
        }
    }
}
impl ViewCommand for PlaceCmd {
    fn key(&self) -> &'static str {
        "place"
    }
    fn title(&self) -> String {
        "Place on ring".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Place", prompt: "Point at the ring; type θ, across or height; click or Enter seats the part" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        self.dofs.iter().map(Dof::dimension).collect()
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        if let Some(o) = edit(&mut self.dofs, input) {
            return o;
        }
        match input {
            StepInput::Pointer { world, theta_deg, across_mm, height_mm, snapped, .. } => {
                for (d, v) in self.dofs.iter_mut().zip([*theta_deg, *across_mm, *height_mm]) {
                    if self.lock.is_none_or(|l| l == d.axis) {
                        d.pointer = v;
                    }
                }
                self.world = Some(*world);
                self.snapped = snapped.as_ref().map(|s| s.label.clone());
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => {
                if !self.seated && self.world.is_none() && self.dofs[0].typed.is_none() {
                    return Outcome::Refused("Point at the ring or type θ".into());
                }
                Outcome::Commit(vec![Effect::Placement { feature: self.feature, placement: self.placement() }])
            }
            StepInput::Lock(axis) => {
                if !self.dofs.iter().any(|d| d.axis == *axis) {
                    return locks_only("Place", &self.dofs);
                }
                self.lock = Some(*axis);
                Outcome::Continue
            }
            StepInput::Unlock => {
                self.lock = None;
                Outcome::Continue
            }
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Typed { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        let placement = self.placement();
        let snap = self.snapped.as_ref().map(|s| format!(" · {s}")).unwrap_or_default();
        let [t, a, h] = [0, 1, 2].map(|i| self.dofs[i].value());
        Preview {
            caption: format!("Place θ {:.1}° · across {a:.2} mm · height {h:.2} mm{snap}{}", wrap360(t), lock_note(self.lock)),
            placement: Some(placement),
            operation: None,
            ghost: self.world.into_iter().collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Box,
    Cylinder,
    Sphere,
}

/// Click the centre, drag the radius (a box's half-size), drag the height; a typed value replaces the drag.
pub struct AddPrimitiveCmd {
    kind: Primitive,
    id: u64,
    step: usize,
    centre: Option<[f64; 3]>,
    world: [f64; 3],
    dofs: Vec<Dof>,
    /// What the centre last snapped to.
    snapped: Option<String>,
}
const MIN_DRAG_MM: f64 = 0.05;
impl AddPrimitiveCmd {
    /// `id` is the feature id the added part takes; the integrator allocates it.
    pub fn new(kind: Primitive, id: u64) -> Self {
        let dofs = vec![
            Dof::new(Axis::Theta, "theta", "θ", Unit::Deg, 90.0),
            Dof::new(Axis::Across, "across", "Across", Unit::Mm, 0.0),
            Dof::size(Axis::X, "radius", if kind == Primitive::Box { "Half-size" } else { "Radius" }, 1.0),
            Dof::size(Axis::Z, "height", "Height", 1.0),
        ];
        Self { kind, id, step: 0, centre: None, world: [0.0; 3], dofs, snapped: None }
    }
    fn has_height(&self) -> bool {
        self.kind != Primitive::Sphere
    }
    fn operation(&self) -> Operation {
        let r = self.dofs[2].value().max(MIN_DIM_MM);
        let h = self.dofs[3].value().max(MIN_DIM_MM);
        match self.kind {
            Primitive::Box => Operation::Box { size: [2.0 * r, 2.0 * r, h] },
            Primitive::Cylinder => Operation::Cylinder { radius_mm: r, height_mm: h },
            Primitive::Sphere => Operation::Sphere { radius_mm: r },
        }
    }
    fn placement(&self) -> Placement {
        Placement::Ring {
            theta_deg: wrap360(self.dofs[0].value()),
            across_mm: self.dofs[1].value(),
            height_mm: 0.0,
            spin_deg: 0.0,
            tilt_deg: 0.0,
            cant_deg: 0.0,
        }
    }
    fn feature(&self) -> Feature {
        let operation = self.operation();
        Feature {
            id: self.id,
            name: operation.label().into(),
            enabled: true,
            operation,
            component: Component { placement: self.placement(), attach: Attach::Join, ..Component::default() },
        }
    }
}
impl ViewCommand for AddPrimitiveCmd {
    fn key(&self) -> &'static str {
        match self.kind {
            Primitive::Box => "add-box",
            Primitive::Cylinder => "add-cylinder",
            Primitive::Sphere => "add-sphere",
        }
    }
    fn title(&self) -> String {
        match self.kind {
            Primitive::Box => "Add box",
            Primitive::Cylinder => "Add cylinder",
            Primitive::Sphere => "Add sphere",
        }
        .into()
    }
    fn step(&self) -> usize {
        self.step
    }
    fn steps(&self) -> Vec<StepInfo> {
        let size = if self.kind == Primitive::Box { "Half-size" } else { "Radius" };
        let mut s = vec![
            StepInfo { name: "Centre", prompt: "Click where the part sits on the ring" },
            StepInfo { name: size, prompt: "Drag the size or type it; Tab reaches the height" },
        ];
        if self.has_height() {
            s.push(StepInfo { name: "Height", prompt: "Drag the height or type it; Enter or click adds the part" });
        }
        s
    }
    /// The step's own fields first, then the later steps' so Tab can fill them ahead.
    fn dimensions(&self) -> Vec<Dimension> {
        let range = match self.step {
            0 => 0..2,
            1 if self.has_height() => 2..4,
            1 => 2..3,
            _ => 3..4,
        };
        self.dofs[range].iter().map(Dof::dimension).collect()
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        if let Some(o) = edit(&mut self.dofs, input) {
            return o;
        }
        match *input {
            StepInput::Pointer { world, theta_deg, across_mm, ref snapped, .. } => {
                self.world = world;
                match (self.step, self.centre) {
                    (0, _) => {
                        self.centre = Some(world);
                        self.dofs[0].pointer = theta_deg;
                        self.dofs[1].pointer = across_mm;
                        self.snapped = snapped.as_ref().map(|s| s.label.clone());
                    }
                    (n, Some(c)) => self.dofs[if n == 1 { 2 } else { 3 }].pointer = dist(world, c).max(MIN_DRAG_MM),
                    (_, None) => {}
                }
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => match self.step {
                0 => {
                    if self.centre.is_none() && self.dofs[0].typed.is_none() {
                        return Outcome::Refused("Point at the ring or type θ".into());
                    }
                    self.step = 1;
                    Outcome::NextStep
                }
                1 if self.has_height() && self.dofs[3].typed.is_none() => {
                    self.step = 2;
                    Outcome::NextStep
                }
                _ => Outcome::Commit(vec![Effect::Add { feature: self.feature() }]),
            },
            StepInput::Lock(_) | StepInput::Unlock => Outcome::Refused("Add has no axis lock".into()),
            StepInput::Back => {
                if self.step == 0 {
                    return Outcome::Cancelled;
                }
                self.step -= 1;
                Outcome::Continue
            }
            StepInput::Cancel => Outcome::Cancelled,
            StepInput::Typed { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        let (r, h) = (self.dofs[2].value(), self.dofs[3].value());
        let size = match self.kind {
            Primitive::Sphere => format!("r {r:.2} mm"),
            Primitive::Cylinder => format!("r {r:.2} × h {h:.2} mm"),
            Primitive::Box => format!("{:.2} × {:.2} × h {h:.2} mm", 2.0 * r, 2.0 * r),
        };
        Preview {
            placement: Some(self.placement()),
            operation: Some(self.operation()),
            ghost: self.centre.into_iter().chain((self.step > 0).then_some(self.world)).collect(),
            caption: format!("{} {size} at {:.1}°{}", self.title(), wrap360(self.dofs[0].value()), snap_note(&self.snapped)),
        }
    }
}

/// Cycle how a part meets the band: a click steps Separate, Join, Cut; Enter commits.
pub struct AttachCmd {
    feature: u64,
    attach: Attach,
}
const ATTACHES: [Attach; 3] = [Attach::Separate, Attach::Join, Attach::Cut];
impl AttachCmd {
    pub fn new(feature: u64, attach: Attach) -> Self {
        Self { feature, attach }
    }
    fn index(&self) -> usize {
        ATTACHES.iter().position(|a| *a == self.attach).unwrap_or(0)
    }
}
impl ViewCommand for AttachCmd {
    fn key(&self) -> &'static str {
        "attach"
    }
    fn title(&self) -> String {
        "Attach".into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Attach", prompt: "Click to cycle Separate, Join, Cut; type 0, 1 or 2; Enter commits" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        vec![Dimension { key: "attach", label: "Attach", unit: Unit::Count, value: self.index() as f64, locked: false }]
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        match input {
            StepInput::Click => {
                self.attach = ATTACHES[(self.index() + 1) % ATTACHES.len()];
                Outcome::Continue
            }
            StepInput::Typed { key: "attach", value } => match ATTACHES.get(value.round() as usize) {
                Some(a) if *value >= -0.5 => {
                    self.attach = *a;
                    Outcome::Continue
                }
                _ => Outcome::Refused("attach: 0 Separate, 1 Join, 2 Cut".into()),
            },
            StepInput::Typed { key, .. } => Outcome::Refused(format!("No dimension named {key}")),
            StepInput::Confirm => Outcome::Commit(vec![Effect::Attach { feature: self.feature, attach: self.attach }]),
            StepInput::Lock(_) | StepInput::Unlock => Outcome::Refused("Attach has no axis lock".into()),
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Pointer { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        let name = match self.attach {
            Attach::Separate => "Separate",
            Attach::Join => "Join",
            Attach::Cut => "Cut",
        };
        Preview { caption: format!("Attach: {name}"), ..Preview::default() }
    }
}

/// Drags one size grip of a part along its line; a typed value replaces the drag.
pub struct GripCmd {
    feature: u64,
    op: Operation,
    grip: Grip,
    /// The grip and the unit direction it moves along, in the world.
    at: [f64; 3],
    direction: [f64; 3],
    /// How far along the line the pointer was when the drag began, and where.
    anchor: Option<(f64, [f64; 3])>,
    world: [f64; 3],
    dof: Dof,
}
impl GripCmd {
    /// `frame` carries the part's own frame into the world; `None` for an operation without the grip.
    pub fn new(feature: u64, op: Operation, key: &str, frame: &Affine) -> Option<Self> {
        let grip = grips::grips(&op).into_iter().find(|g| g.key == key)?;
        let at = frame.apply(grip.at);
        let d = frame.turn(grip.direction);
        let len = d.iter().map(|v| v * v).sum::<f64>().sqrt();
        if !(len > 1e-12) {
            return None;
        }
        let direction = d.map(|v| v / len);
        let dof = Dof { key: grip.key, label: grip.label, unit: grip.unit, axis: Axis::Height, positive: grip.minimum > 0.0, pointer: grip.value, typed: None };
        Some(Self { feature, op, grip, at, direction, anchor: None, world: at, dof })
    }
    fn value(&self) -> f64 {
        self.dof.value().max(self.grip.minimum)
    }
    fn operation(&self) -> Operation {
        grips::with(&self.op, self.grip.key, self.value()).unwrap_or_else(|| self.op.clone())
    }
}
impl ViewCommand for GripCmd {
    fn key(&self) -> &'static str {
        "grip"
    }
    fn title(&self) -> String {
        self.grip.label.into()
    }
    fn step(&self) -> usize {
        0
    }
    fn steps(&self) -> Vec<StepInfo> {
        vec![StepInfo { name: "Grip", prompt: "Drag the grip along its line; type a value; release or Enter sets it" }]
    }
    fn dimensions(&self) -> Vec<Dimension> {
        vec![self.dof.dimension()]
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        if let Some(o) = edit(std::slice::from_mut(&mut self.dof), input) {
            return o;
        }
        match *input {
            StepInput::Pointer { world, .. } => {
                let along: f64 = (0..3).map(|k| (world[k] - self.at[k]) * self.direction[k]).sum();
                let (from, _) = *self.anchor.get_or_insert((along, world));
                self.dof.pointer = self.grip.dragged(self.grip.value, along - from);
                self.world = world;
                Outcome::Continue
            }
            StepInput::Click | StepInput::Confirm => Outcome::Commit(vec![Effect::Operation { feature: self.feature, operation: self.operation() }]),
            StepInput::Lock(_) | StepInput::Unlock => Outcome::Refused("A grip moves along its own line".into()),
            StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
            StepInput::Typed { .. } | StepInput::Cleared { .. } => Outcome::Continue,
        }
    }
    fn preview(&self) -> Preview {
        Preview {
            placement: None,
            operation: Some(self.operation()),
            ghost: self.anchor.map(|(_, press)| vec![press, self.world]).unwrap_or_default(),
            caption: format!("{} {}", self.grip.label, self.grip.unit.format(self.value())),
        }
    }
}

/// Every command with its hotkey and mark, in tool-rail order.
pub fn catalog() -> Vec<CommandInfo> {
    let ring = Placement::ring(90.0, 0.0);
    let info = |c: &dyn ViewCommand, hotkey, icon| CommandInfo { key: c.key(), title: c.title(), hotkey, steps: c.steps(), icon };
    vec![
        info(&MoveCmd::new(0, ring.clone(), 0), Some('G'), Icon::Move),
        info(&RotateCmd::new(0, ring.clone(), 0), Some('R'), Icon::Rotate),
        info(&ScaleCmd::new(0, Operation::Sphere { radius_mm: 1.0 }, [0.0; 3]).expect("a sphere scales"), Some('S'), Icon::Scale),
        info(&PlaceCmd::new(0, ring), Some('P'), Icon::CadPlace),
        info(&AddPrimitiveCmd::new(Primitive::Box, 0), None, Icon::CadBox),
        info(&AddPrimitiveCmd::new(Primitive::Cylinder, 0), None, Icon::CadCylinder),
        info(&AddPrimitiveCmd::new(Primitive::Sphere, 0), None, Icon::CadSphere),
        info(&AttachCmd::new(0, Attach::Separate), Some('J'), Icon::CadUnion),
    ]
}

/// The commands that repeat or reshape a chosen part, with their hotkeys and marks: an array round the ring, and press-pull on a chosen face.
pub fn reshaping() -> Vec<CommandInfo> {
    use super::pattern::{ArrayCmd, PressPullCmd};
    let part = Feature { id: 0, name: String::new(), enabled: true, operation: Operation::Box { size: [1.0; 3] }, component: Component::default() };
    let info = |c: &dyn ViewCommand, hotkey, icon| CommandInfo { key: c.key(), title: c.title(), hotkey, steps: c.steps(), icon };
    let face = ringdesign_core::cad::FaceRef::bare(0);
    vec![
        info(&ArrayCmd::new(0, part.clone(), Attach::Join, None), Some('A'), Icon::Array),
        info(&PressPullCmd::new(part, face, Attach::Join, [0.0; 3], [0.0, 0.0, 1.0], 0), Some('Q'), Icon::Raise),
    ]
}

/// Every command the tool rail carries, in its order: the catalog's, then those that repeat or reshape a part.
pub fn rail() -> Vec<CommandInfo> {
    catalog().into_iter().chain(reshaping()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::session::Session;
    use ringdesign_core::RingDesign;

    #[test]
    fn a_head_its_seat_and_its_azures_move_by_the_stone_they_are_built_round_and_nothing_else_does() {
        let d = ringdesign_core::cad::examples::design("claw-solitaire").unwrap();
        let mut doc = d.cad.unwrap();
        doc.append(builders::feature_on(5, "Azures", builders::AZURE, 2, serde_json::json!({}))).unwrap();
        // A builder named on something that is not a stone moves by itself.
        doc.append(builders::feature_on(6, "Stray head", builders::CLAW, 5, serde_json::json!({}))).unwrap();
        let by = |id: u64| moved_by(&doc, doc.feature(id).unwrap()).map(|s| s.id);
        assert_eq!([1, 2, 3, 4, 5, 6].map(by), [None, None, Some(2), Some(2), Some(2), None]);
    }

    /// The pointer on a 9.5 mm crest at `theta`, `across` along the finger, `height` off the surface.
    fn pointer(theta: f64, across: f64, height: f64) -> StepInput {
        let a = theta.to_radians();
        StepInput::Pointer {
            world: [9.5 * a.cos(), 9.5 * a.sin(), across],
            normal: [a.cos(), a.sin(), 0.0],
            theta_deg: theta,
            across_mm: across,
            height_mm: height,
            snapped: None,
            dragging: false,
        }
    }
    /// The pointer at a world point, its ring reading taken as θ 90 on the crest.
    fn at(world: [f64; 3]) -> StepInput {
        StepInput::Pointer { world, normal: [0.0, 0.0, 1.0], theta_deg: 90.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: true }
    }
    fn ring() -> Placement {
        Placement::Ring { theta_deg: 90.0, across_mm: 0.5, height_mm: 0.2, spin_deg: 10.0, tilt_deg: 0.0, cant_deg: 0.0 }
    }
    fn values(cmd: &dyn ViewCommand) -> Vec<(&'static str, f64, bool)> {
        cmd.dimensions().into_iter().map(|d| (d.key, d.value, d.locked)).collect()
    }
    fn effects(o: Outcome) -> Vec<Effect> {
        match o {
            Outcome::Commit(effects) => effects,
            other => panic!("expected a commit, got {other:?}"),
        }
    }
    fn placement_of(o: Outcome) -> Placement {
        match effects(o).as_slice() {
            [Effect::Placement { placement, .. }] => placement.clone(),
            other => panic!("expected one placement, got {other:?}"),
        }
    }
    fn added(o: Outcome) -> Feature {
        match effects(o).as_slice() {
            [Effect::Add { feature }] => feature.clone(),
            other => panic!("expected one added feature, got {other:?}"),
        }
    }
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_rail_carries_the_catalog_then_the_array_and_press_pull_each_with_its_own_key_and_mark() {
        let rail = rail();
        let rows: Vec<(&str, &str, Option<char>, Icon)> = rail.iter().skip(catalog().len()).map(|c| (c.key, c.title.as_str(), c.hotkey, c.icon)).collect();
        assert_eq!(rows, [("array", "Array round the ring", Some('A'), Icon::Array), ("press-pull", "Press-pull", Some('Q'), Icon::Raise)]);
        let keys: std::collections::HashSet<char> = rail.iter().filter_map(|c| c.hotkey).collect();
        assert_eq!(keys.len(), rail.iter().filter(|c| c.hotkey.is_some()).count(), "no two commands share a key");
        let marks: std::collections::HashSet<Icon> = rail.iter().map(|c| c.icon).collect();
        assert_eq!(marks.len(), rail.len(), "every command on the rail has its own mark");
        assert!(rail.iter().all(|c| !c.steps.is_empty()));
    }

    #[test]
    fn move_with_theta_locked_and_twelve_typed_commits_102_exactly() {
        let mut s = Session::default();
        s.start(Box::new(MoveCmd::new(3, ring(), 99)));
        s.feed(pointer(90.0, 0.5, 0.2));
        assert!(matches!(s.feed(StepInput::Lock(Axis::X)), Outcome::Refused(m) if m == "Move locks theta, across, height"));
        assert!(matches!(s.feed(StepInput::Lock(Axis::Theta)), Outcome::Continue));
        s.feed(pointer(100.0, 2.0, 1.0));
        assert_eq!(values(s.command().unwrap()), [("theta", 10.0, false), ("across", 0.0, false), ("height", 0.0, false)]);
        assert!(matches!(s.feed(StepInput::Typed { key: "theta", value: 12.0 }), Outcome::Continue));
        s.feed(pointer(130.0, 3.0, 2.0));
        assert_eq!(values(s.command().unwrap()), [("theta", 12.0, true), ("across", 0.0, false), ("height", 0.0, false)]);
        let p = s.preview().unwrap();
        assert_eq!(p.caption, "Move Δθ 12.0° · Δacross 0.00 mm · Δheight 0.00 mm · theta locked");
        assert_eq!(p.ghost.len(), 2);
        assert_eq!(p.placement.as_ref().and_then(Placement::theta_deg), Some(102.0));
        let placement = placement_of(s.enter());
        assert_eq!(placement, Placement::Ring { theta_deg: 102.0, across_mm: 0.5, height_mm: 0.2, spin_deg: 10.0, tilt_deg: 0.0, cant_deg: 0.0 });
        assert!(!s.is_live());
    }

    #[test]
    fn move_unlocked_follows_all_three_and_wraps_the_angle_across_the_seam() {
        let mut c = MoveCmd::new(3, ring(), 99);
        c.feed(&pointer(350.0, 0.0, 0.0));
        c.feed(&pointer(10.0, 0.25, -0.1));
        assert_eq!(values(&c), [("theta", 20.0, false), ("across", 0.25, false), ("height", -0.1, false)]);
        let p = placement_of(c.feed(&StepInput::Click));
        assert_eq!(p.theta_deg(), Some(110.0));
        assert!(matches!(p, Placement::Ring { across_mm, height_mm, .. } if close(across_mm, 0.75) && close(height_mm, 0.1)));
        // Past the seam the committed angle comes back into [0, 360).
        let mut c = MoveCmd::new(3, Placement::ring(350.0, 0.0), 99);
        c.feed(&StepInput::Typed { key: "theta", value: 25.0 });
        assert_eq!(placement_of(c.feed(&StepInput::Confirm)).theta_deg(), Some(15.0));
        assert!(matches!(MoveCmd::new(3, ring(), 99).feed(&StepInput::Typed { key: "theta", value: f64::NAN }), Outcome::Refused(_)));
    }

    #[test]
    fn enter_with_no_input_commits_the_placement_unchanged() {
        let mut s = Session::default();
        s.start(Box::new(MoveCmd::new(3, ring(), 99)));
        assert_eq!(placement_of(s.enter()), ring());
        s.start(Box::new(RotateCmd::new(3, ring(), 99)));
        assert_eq!(placement_of(s.enter()), ring());
        s.start(Box::new(PlaceCmd::new(3, ring())));
        assert_eq!(placement_of(s.enter()), ring());
        s.start(Box::new(MoveCmd::new(3, ring(), 99)));
        assert!(matches!(s.escape(), Outcome::Cancelled), "escape on the only step cancels");
        assert!(!s.is_live());
    }

    #[test]
    fn a_free_part_moves_in_world_xyz_on_a_transform_that_keeps_its_component() {
        let part = Feature {
            id: 3,
            name: "Bezel".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 },
            component: Component { attach: Attach::Join, ..Component::default() },
        };
        let mut c = MoveCmd::of(&part, 99);
        c.feed(&at([1.0, 2.0, 3.0]));
        c.feed(&at([2.0, 2.5, 3.0]));
        c.feed(&StepInput::Lock(Axis::Y));
        assert_eq!(values(&c), [("x", 0.0, false), ("y", 0.5, false), ("z", 0.0, false)]);
        c.feed(&StepInput::Typed { key: "z", value: -1.0 });
        assert!(matches!(c.preview().operation, Some(Operation::Transform { .. })));
        let f = added(c.feed(&StepInput::Confirm));
        assert_eq!((f.id, f.name.as_str(), f.component.attach), (99, "Place component", Attach::Join));
        assert!(matches!(f.operation, Operation::Transform { source: 3, translation: [0.0, 0.5, -1.0], rotation_deg: [0.0, 0.0, 0.0] }));
        // A part that is already a Transform is edited, not wrapped again.
        let moved = Feature { id: 99, operation: f.operation.clone(), ..f };
        let mut c = MoveCmd::of(&moved, 100);
        c.feed(&StepInput::Typed { key: "x", value: 2.0 });
        let e = effects(c.feed(&StepInput::Confirm));
        assert!(
            matches!(e.as_slice(), [Effect::Operation { feature: 99, operation: Operation::Transform { source: 3, translation: [2.0, 0.5, -1.0], .. } }]),
            "{e:?}"
        );
    }

    #[test]
    fn rotate_spins_as_the_pointer_circles_the_seat_and_types_the_rest() {
        let seat = Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 0.0, spin_deg: 10.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let mut c = RotateCmd::new(3, seat.clone(), 99);
        // From the −finger side of the seat round to its +θ side is a quarter turn about the normal.
        c.feed(&pointer(90.0, -1.0, 0.0));
        let arc = (1.0f64 / 9.5).to_degrees();
        c.feed(&pointer(90.0 + arc, 0.0, 0.0));
        let [spin, tilt, cant] = [0, 1, 2].map(|i| values(&c)[i].1);
        assert!(close(spin, 90.0) && tilt == 0.0 && cant == 0.0, "{:?}", values(&c));
        assert!(matches!(c.feed(&StepInput::Lock(Axis::Theta)), Outcome::Refused(m) if m == "Rotate locks spin, tilt, cant"));
        c.feed(&StepInput::Typed { key: "cant", value: -5.0 });
        let p = placement_of(c.feed(&StepInput::Confirm));
        assert!(matches!(p, Placement::Ring { spin_deg, tilt_deg: 0.0, cant_deg: -5.0, theta_deg: 90.0, .. } if close(spin_deg, 100.0)), "{p:?}");
        // Typed turns add exactly and come back into [-180, 180].
        let mut c = RotateCmd::new(3, seat, 99);
        c.feed(&StepInput::Typed { key: "spin", value: 175.0 });
        c.feed(&StepInput::Typed { key: "tilt", value: 12.5 });
        let p = placement_of(c.feed(&StepInput::Click));
        assert!(matches!(p, Placement::Ring { spin_deg: -175.0, tilt_deg: 12.5, .. }), "{p:?}");
    }

    #[test]
    fn a_locked_lean_tips_the_part_toward_the_pointer() {
        let d = RingDesign::default();
        let seat = Placement::ring(90.0, 0.0);
        let tip = |p: &Placement| p.frame(&d).unwrap().z_axis;
        let mut c = RotateCmd::new(3, seat.clone(), 99);
        c.feed(&StepInput::Lock(Axis::Tilt));
        c.feed(&pointer(90.0, 0.0, 0.0));
        c.feed(&pointer(80.0, 0.0, 0.0));
        let tilted = placement_of(c.feed(&StepInput::Confirm));
        // Ten degrees toward smaller θ, which at the top of the ring is +x.
        let z = tip(&tilted);
        assert!(close(z[0], 10f64.to_radians().sin()) && close(z[1], 10f64.to_radians().cos()), "{z:?}");
        let mut c = RotateCmd::new(3, seat, 99);
        c.feed(&StepInput::Lock(Axis::Cant));
        c.feed(&pointer(90.0, 0.0, 0.0));
        c.feed(&pointer(90.0, 9.5, 0.0));
        let canted = placement_of(c.feed(&StepInput::Confirm));
        assert!(matches!(canted, Placement::Ring { cant_deg, .. } if close(cant_deg, -45.0)), "{canted:?}");
        let z = tip(&canted);
        assert!(z[2] > 0.7 && close(z[2], z[1]), "leans toward +finger: {z:?}");
    }

    #[test]
    fn a_free_part_turns_about_the_locked_world_axis_through_the_origin() {
        let mut c = RotateCmd::new(3, Placement::Free, 99);
        c.feed(&at([9.5, 0.0, 0.0]));
        c.feed(&at([0.0, 9.5, 0.0]));
        assert_eq!(values(&c), [("x", 0.0, false), ("y", 0.0, false), ("z", 90.0, false)], "a quarter turn round the finger axis");
        let mut c = RotateCmd::new(3, Placement::Free, 99);
        c.feed(&StepInput::Lock(Axis::X));
        c.feed(&at([0.0, 9.5, 0.0]));
        c.feed(&at([0.0, 0.0, 9.5]));
        assert_eq!(values(&c), [("x", 90.0, false), ("y", 0.0, false), ("z", 0.0, false)], "from +y to +z is a quarter turn about x");
        let f = added(c.feed(&StepInput::Click));
        assert!(matches!(f.operation, Operation::Transform { source: 3, translation: [0.0, 0.0, 0.0], rotation_deg: [90.0, 0.0, 0.0] }), "{f:?}");
    }

    #[test]
    fn scale_reads_the_distance_ratio_locks_an_axis_and_types_a_size_or_a_factor() {
        let mut c = ScaleCmd::new(5, Operation::Cylinder { radius_mm: 2.0, height_mm: 3.0 }, [0.0; 3]).unwrap();
        assert_eq!(values(&c), [("factor", 1.0, false), ("radius", 2.0, false), ("height", 3.0, false)]);
        c.feed(&at([1.0, 0.0, 0.0]));
        c.feed(&at([1.5, 0.0, 0.0]));
        assert_eq!(values(&c), [("factor", 1.5, false), ("radius", 3.0, false), ("height", 4.5, false)]);
        c.feed(&StepInput::Lock(Axis::Z));
        assert_eq!(values(&c), [("factor", 1.5, false), ("radius", 2.0, false), ("height", 4.5, false)]);
        c.feed(&StepInput::Lock(Axis::Y));
        assert_eq!(values(&c), [("factor", 1.5, false), ("radius", 3.0, false), ("height", 3.0, false)], "y reaches a radius");
        c.feed(&StepInput::Lock(Axis::Z));
        c.feed(&StepInput::Typed { key: "radius", value: 1.25 });
        c.feed(&StepInput::Typed { key: "factor", value: 2.0 });
        assert_eq!(values(&c), [("factor", 2.0, true), ("radius", 1.25, true), ("height", 6.0, false)]);
        assert!(matches!(c.feed(&StepInput::Typed { key: "height", value: 0.0 }), Outcome::Refused(m) if m == "height: must be above zero"));
        let e = effects(c.feed(&StepInput::Confirm));
        let [Effect::Operation { feature: 5, operation: Operation::Cylinder { radius_mm, height_mm } }] = e.as_slice() else { panic!("{e:?}") };
        assert_eq!((*radius_mm, *height_mm), (1.25, 6.0));
        let mut b = ScaleCmd::new(5, Operation::Box { size: [1.0, 2.0, 3.0] }, [0.0; 3]).unwrap();
        b.feed(&at([0.0, 2.0, 0.0]));
        b.feed(&at([0.0, 1.0, 0.0]));
        assert!(matches!(b.preview().operation, Some(Operation::Box { size: [0.5, 1.0, 1.5] })));
        let mut t = ScaleCmd::new(5, Operation::Torus { major_mm: 9.0, minor_mm: 1.0 }, [0.0; 3]).unwrap();
        t.feed(&StepInput::Typed { key: "factor", value: 3.0 });
        assert!(matches!(t.preview().operation, Some(Operation::Torus { major_mm: 9.0, minor_mm: 3.0 })), "the factor scales the minor only");
        assert!(matches!(t.feed(&StepInput::Typed { key: "factor", value: 0.0 }), Outcome::Refused(_)));
        assert!(ScaleCmd::new(5, Operation::Band, [0.0; 3]).is_none());
        let sketch = ringdesign_core::sketch::Sketch::default();
        let mut e = ScaleCmd::new(5, Operation::Extrude { sketch: sketch.into(), height_mm: 2.0, draft_deg: 3.0 }, [0.0; 3]).unwrap();
        assert!(matches!(e.feed(&StepInput::Lock(Axis::X)), Outcome::Refused(m) if m == "Extrude has no size along x"));
        assert!(matches!(e.feed(&StepInput::Lock(Axis::Spin)), Outcome::Refused(m) if m == "Scale locks x, y or z"));
        // A cut 1 mm into the metal scales its depth and stays a cut: twice is −2, typed 0.4 is −0.4.
        let mut cut = ScaleCmd::new(5, Operation::Extrude { sketch: ringdesign_core::cad::Profile::Feature { feature: 4 }, height_mm: -1.0, draft_deg: 3.0 }, [0.0; 3]).unwrap();
        assert_eq!(values(&cut), [("factor", 1.0, false), ("height", 1.0, false)]);
        assert!(matches!(cut.preview().operation, Some(Operation::Extrude { height_mm, .. }) if height_mm == -1.0), "{:?}", cut.preview().operation);
        cut.feed(&StepInput::Typed { key: "factor", value: 2.0 });
        assert!(matches!(cut.preview().operation, Some(Operation::Extrude { height_mm, draft_deg, .. }) if height_mm == -2.0 && draft_deg == 3.0), "{:?}", cut.preview().operation);
        cut.feed(&StepInput::Typed { key: "height", value: 0.4 });
        let e = effects(cut.feed(&StepInput::Confirm));
        assert!(matches!(e.as_slice(), [Effect::Operation { feature: 5, operation: Operation::Extrude { height_mm, .. } }] if *height_mm == -0.4), "{e:?}");
        let mut tiny = ScaleCmd::new(5, Operation::Sphere { radius_mm: 1.0 }, [0.0; 3]).unwrap();
        tiny.feed(&at([1.0, 0.0, 0.0]));
        tiny.feed(&at([0.0, 0.0, 0.0]));
        assert!(matches!(tiny.preview().operation, Some(Operation::Sphere { radius_mm }) if radius_mm == MIN_DIM_MM), "a dimension never collapses");
    }

    #[test]
    fn place_seats_the_pointer_point_and_keeps_the_leans() {
        let mut c = PlaceCmd::new(4, Placement::Free);
        assert!(matches!(c.feed(&StepInput::Lock(Axis::Spin)), Outcome::Refused(_)));
        assert!(matches!(c.feed(&StepInput::Confirm), Outcome::Refused(m) if m == "Point at the ring or type θ"), "a free part has no seat to keep");
        c.feed(&pointer(45.0, 0.3, 0.1));
        let p = placement_of(c.feed(&StepInput::Click));
        assert_eq!(p, Placement::Ring { theta_deg: 45.0, across_mm: 0.3, height_mm: 0.1, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 });
        let mut c = PlaceCmd::new(4, Placement::Ring { theta_deg: 0.0, across_mm: 0.0, height_mm: 0.0, spin_deg: 33.0, tilt_deg: 4.0, cant_deg: -2.0 });
        c.feed(&StepInput::Lock(Axis::Theta));
        c.feed(&pointer(200.0, 1.0, 1.0));
        c.feed(&StepInput::Typed { key: "height", value: 0.4 });
        assert_eq!(c.preview().caption, "Place θ 200.0° · across 0.00 mm · height 0.40 mm · theta locked");
        let p = placement_of(c.feed(&StepInput::Confirm));
        assert_eq!(p, Placement::Ring { theta_deg: 200.0, across_mm: 0.0, height_mm: 0.4, spin_deg: 33.0, tilt_deg: 4.0, cant_deg: -2.0 });
        let mut typed = PlaceCmd::new(4, Placement::Free);
        typed.feed(&StepInput::Typed { key: "theta", value: -30.0 });
        assert_eq!(placement_of(typed.feed(&StepInput::Confirm)), Placement::ring(330.0, 0.0), "a typed angle seats a free part");
    }

    #[test]
    fn a_typed_cylinder_commits_at_the_clicked_angle_joined() {
        let mut s = Session::default();
        s.start(Box::new(AddPrimitiveCmd::new(Primitive::Cylinder, 7)));
        assert!(matches!(s.feed(StepInput::Click), Outcome::Refused(_)), "no centre yet");
        s.feed(pointer(90.0, 0.0, 0.0));
        assert!(matches!(s.feed(StepInput::Click), Outcome::NextStep));
        assert_eq!(s.command().unwrap().step(), 1);
        assert_eq!(s.dimensions().iter().map(|d| d.key).collect::<Vec<_>>(), ["radius", "height"]);
        assert_eq!(s.prompt(), "Add cylinder: Radius — Drag the size or type it; Tab reaches the height");
        assert!(matches!(s.feed(StepInput::Typed { key: "radius", value: -1.5 }), Outcome::Refused(m) if m == "radius: must be above zero"));
        s.feed(StepInput::Typed { key: "radius", value: 1.5 });
        s.feed(StepInput::Typed { key: "height", value: 3.0 });
        let f = added(s.enter());
        assert_eq!((f.id, f.name.as_str(), f.enabled), (7, "Cylinder", true));
        assert!(matches!(f.operation, Operation::Cylinder { radius_mm: 1.5, height_mm: 3.0 }));
        assert_eq!(f.component.placement, Placement::ring(90.0, 0.0));
        assert_eq!(f.component.attach, Attach::Join);
        assert_eq!(
            s.history(),
            [
                "start add-cylinder",
                "click",
                "pointer θ=90.00 v=0.000 h=0.000 at 0.000,9.500,0.000",
                "click",
                "typed radius=-1.5",
                "typed radius=1.5",
                "typed height=3",
                "confirm"
            ]
        );
    }

    #[test]
    fn a_dragged_box_and_a_sphere_take_their_sizes_from_the_pointer_distance() {
        let mut c = AddPrimitiveCmd::new(Primitive::Box, 8);
        assert!(matches!(c.feed(&StepInput::Lock(Axis::X)), Outcome::Refused(_)));
        c.feed(&at([0.0, 9.5, 0.25]));
        c.feed(&StepInput::Click);
        c.feed(&at([2.0, 9.5, 0.25]));
        assert_eq!(values(&c), [("radius", 2.0, false), ("height", 1.0, false)]);
        assert!(matches!(c.feed(&StepInput::Click), Outcome::NextStep));
        assert_eq!((c.step(), values(&c)), (2, vec![("height", 1.0, false)]), "the height step shows its own field");
        c.feed(&at([0.0, 9.5, 2.75]));
        assert_eq!(c.preview().caption, "Add box 4.00 × 4.00 × h 2.50 mm at 90.0°");
        assert_eq!(c.preview().ghost.len(), 2);
        assert!(matches!(c.feed(&StepInput::Back), Outcome::Continue));
        assert_eq!(c.step(), 1);
        c.feed(&StepInput::Click);
        let f = added(c.feed(&StepInput::Confirm));
        assert!(matches!(f.operation, Operation::Box { size: [4.0, 4.0, 2.5] }));
        assert_eq!(f.component.placement, Placement::ring(90.0, 0.0));
        let mut s = AddPrimitiveCmd::new(Primitive::Sphere, 9);
        assert_eq!(s.steps().len(), 2);
        s.feed(&pointer(180.0, 0.0, 0.0));
        s.feed(&StepInput::Click);
        assert_eq!(s.dimensions().len(), 1);
        s.feed(&at([-9.5, 0.75, 0.0]));
        let f = added(s.feed(&StepInput::Click));
        assert!(matches!(f.operation, Operation::Sphere { radius_mm } if close(radius_mm, 0.75)));
        assert_eq!(f.component.placement.theta_deg(), Some(180.0));
        assert!(matches!(AddPrimitiveCmd::new(Primitive::Sphere, 9).feed(&StepInput::Back), Outcome::Cancelled));
    }

    #[test]
    fn the_escape_ladder_runs_through_a_primitive_and_clears_a_size_it_backs_into() {
        let mut s = Session::default();
        s.start(Box::new(AddPrimitiveCmd::new(Primitive::Cylinder, 7)));
        s.feed(pointer(90.0, 0.0, 0.0));
        s.feed(StepInput::Click);
        s.feed(StepInput::Typed { key: "radius", value: 2.5 });
        assert!(matches!(s.enter(), Outcome::NextStep), "with no height typed Enter moves on to it");
        s.feed(StepInput::Typed { key: "height", value: 4.0 });
        s.escape();
        assert_eq!(values(s.command().unwrap()), [("height", 1.0, false)], "the height step's own field clears first");
        s.escape();
        assert_eq!(s.command().unwrap().step(), 1);
        assert_eq!(values(s.command().unwrap()), [("radius", 2.5, true), ("height", 1.0, false)]);
        s.escape();
        assert_eq!(values(s.command().unwrap()), [("radius", 1.0, false), ("height", 1.0, false)], "the radius typed a step ago clears next");
        s.escape();
        assert_eq!(s.command().unwrap().step(), 0);
        assert!(matches!(s.escape(), Outcome::Cancelled));
    }

    /// The Court band with a 4 x 6 x 1.5 mm plate joined on its top, a 3 mm stone on the plate's top 0.4 mm round the ring from its centre, and the stone's four-claw head.
    fn stone_on_plate() -> RingDesign {
        use ringdesign_core::cad::{Document, face_signature, stone_on_face};
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        let feature = |id, name: &str, operation, component| Feature { id, name: name.into(), enabled: true, operation, component };
        doc.append(feature(1, "Procedural shank", Operation::Band, Component::default())).unwrap();
        let plate = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.65), ..Component::default() };
        doc.append(feature(2, "Plate", Operation::Box { size: [4.0, 6.0, 1.5] }, plate)).unwrap();
        d.cad = Some(doc.clone());
        let e = ringdesign_core::cad::evaluate(&d, &lib, ringdesign_core::BuildParams::default()).unwrap();
        let host = e.components.iter().find(|c| c.id == 2).unwrap();
        let top = (0..host.body.faces.len()).find(|i| face_signature(&host.body, *i, &host.frame).is_some_and(|s| s.normal[2] > 0.99)).unwrap() as u32;
        let gem = ringdesign_core::gem::Gem::calibrated(ringdesign_core::gem::GemCut::Round, 3.0);
        let seat = FaceSeat { v_mm: 0.4, ..FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap() };
        doc.append(stone_on_face(3, gem, 2, &seat)).unwrap();
        doc.append(builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }))).unwrap();
        d.cad = Some(doc);
        d
    }
    /// The design evaluated, and the stone's and its head's frames as built.
    fn built_frames(d: &RingDesign) -> (ringdesign_core::cad::Evaluated, Affine, Affine) {
        let e = ringdesign_core::cad::evaluate(d, &ringdesign_core::AlphaLibrary::builtin(), ringdesign_core::BuildParams::default()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let frame = |id| {
            let f = e.components.iter().find(|c| c.id == id).unwrap().frame;
            Affine::frame(f.x_axis, f.y_axis, f.z_axis, f.origin)
        };
        let (stone, head) = (frame(3), frame(4));
        (e, stone, head)
    }
    fn close3(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }
    /// The one effect a commit makes.
    fn one(o: Outcome) -> Effect {
        let mut e = effects(o);
        assert_eq!(e.len(), 1, "{e:?}");
        e.remove(0)
    }
    fn plus(p: [f64; 3], terms: &[([f64; 3], f64)]) -> [f64; 3] {
        std::array::from_fn(|k| p[k] + terms.iter().map(|(v, s)| v[k] * s).sum::<f64>())
    }

    #[test]
    fn g_and_r_on_a_stone_on_a_face_edit_its_seat_and_the_head_built_on_it_follows() {
        let d = stone_on_plate();
        let (e, stone, head) = built_frames(&d);
        let f = d.cad.as_ref().unwrap().feature(3).unwrap().clone();
        let hold = FaceHold::of(&f, e.components.iter().find(|c| c.id == 3).unwrap()).expect("a stone on a face");
        // The hold reads the face's own axes back off the stone as built, and stands it where the build did.
        assert!(close3(hold.foot(), plus(hold.origin, &[(hold.normal, -hold.seat.height_mm)]), 0.0));
        assert!(dot(hold.along, hold.across).abs() < 1e-12 && dot(hold.along, hold.normal).abs() < 1e-12 && (dot(hold.along, hold.along) - 1.0).abs() < 1e-12);
        let held = hold.map(&hold.seat).unwrap();
        assert!(close3(held.apply(hold.origin), hold.origin, 1e-12) && close3(held.turn(hold.normal), hold.normal, 1e-12));
        assert!(FaceHold::of(&d.cad.as_ref().unwrap().feature(4).unwrap().clone(), e.components.iter().find(|c| c.id == 4).unwrap()).is_none(), "a head is no stone");
        // G slides the stone along and across the face with the pointer and keeps its height; typed along, it goes exactly there.
        let at = |w: [f64; 3]| StepInput::Pointer { world: w, normal: hold.normal, theta_deg: 90.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: false };
        let mut s = Session::default();
        s.start(Box::new(MoveCmd::on_face(hold.clone())));
        s.feed(at(hold.origin));
        s.feed(at(plus(hold.origin, &[(hold.along, 0.3), (hold.across, -0.1), (hold.normal, 0.5)])));
        let dims = |s: &Session| s.dimensions().into_iter().map(|d| (d.key, (d.value * 1e9).round() / 1e9)).collect::<Vec<_>>();
        assert_eq!(dims(&s), [("u", 0.3), ("v", -0.1), ("height", 0.0)]);
        assert_eq!(s.preview().unwrap().caption, "Move Δalong 0.30 mm · Δacross -0.10 mm · Δheight 0.00 mm");
        assert!(matches!(s.feed(StepInput::Lock(Axis::Z)), Outcome::Continue));
        assert_eq!(dims(&s), [("u", 0.0), ("v", 0.0), ("height", 0.5)], "locked to the normal it lifts off the face");
        s.feed(StepInput::Unlock);
        s.feed(StepInput::Typed { key: "u", value: 0.4 });
        let ghost = hold.ghost(&s.preview().unwrap()).unwrap();
        let Effect::Operation { feature: 3, operation } = one(s.enter()) else { panic!("the seat is edited, the stone is never wrapped in a Transform") };
        let seat = FaceHold::seat_in(&operation).unwrap();
        assert!((seat.u_mm - hold.seat.u_mm - 0.4).abs() < 1e-12 && (seat.v_mm - hold.seat.v_mm + 0.1).abs() < 1e-12, "{seat:?}");
        assert_eq!((seat.height_mm, seat.spin_deg, &seat.face), (hold.seat.height_mm, 0.0, &hold.seat.face));
        // Built again, the stone stands 0.4 mm along and 0.1 mm back across the face, as its ghost showed, and its head stands in its frame.
        let mut moved = d.clone();
        moved.cad.as_mut().unwrap().apply(&ringdesign_core::cad::edit::CadEdit::Operation { id: 3, operation }).unwrap();
        let (_, stone2, head2) = built_frames(&moved);
        let expected = plus(stone.origin(), &[(hold.along, 0.4), (hold.across, -0.1)]);
        assert!(close3(stone2.origin(), expected, 1e-9), "{:?} against {expected:?}", stone2.origin());
        assert!(close3(ghost.apply(stone.origin()), stone2.origin(), 1e-9) && (0..3).all(|i| close3(stone2.axis(i), stone.axis(i), 1e-12)));
        assert_eq!(head2, stone2, "the head is built in its stone's frame");
        assert!(close3(head2.origin(), plus(head.origin(), &[(hold.along, 0.4), (hold.across, -0.1)]), 1e-9));
        // R spins it on the face: a quarter turn of the pointer about the normal through the girdle's centre is 90° of seat.
        let mut r = RotateCmd::on_face(hold.clone());
        r.feed(&at(plus(hold.origin, &[(hold.along, 1.0)])));
        r.feed(&at(plus(hold.origin, &[(hold.across, 1.0)])));
        assert_eq!(values(&r).iter().map(|v| (v.0, (v.1 * 1e9).round() / 1e9)).collect::<Vec<_>>(), [("spin", 90.0)]);
        assert!(matches!(r.feed(&StepInput::Lock(Axis::Tilt)), Outcome::Refused(m) if m == "Rotate locks spin"));
        let ghost = hold.ghost(&r.preview()).unwrap();
        let Effect::Operation { feature: 3, operation } = one(r.feed(&StepInput::Confirm)) else { panic!() };
        let seat = FaceHold::seat_in(&operation).unwrap();
        assert!((seat.spin_deg - 90.0).abs() < 1e-9 && seat.u_mm == hold.seat.u_mm && seat.v_mm == hold.seat.v_mm);
        let mut spun = d.clone();
        spun.cad.as_mut().unwrap().apply(&ringdesign_core::cad::edit::CadEdit::Operation { id: 3, operation }).unwrap();
        let (_, stone3, head3) = built_frames(&spun);
        assert!(close3(stone3.origin(), stone.origin(), 1e-9) && close3(stone3.axis(0), stone.axis(1), 1e-9) && close3(stone3.axis(1), stone.axis(0).map(|v| -v), 1e-9), "{stone3:?}");
        assert!(close3(ghost.turn(stone.axis(0)), stone3.axis(0), 1e-9));
        assert_eq!(head3, stone3);
    }

    #[test]
    fn the_face_gizmos_arrows_ring_and_dial_drive_the_seat_and_the_dial_drops_the_turned_foot_onto_the_face() {
        use crate::command::ring::{along_line, on_plane};
        use ringdesign_core::interaction::pick::Ray;
        let d = stone_on_plate();
        let (e, _, _) = built_frames(&d);
        let f = d.cad.as_ref().unwrap().feature(3).unwrap().clone();
        let hold = FaceHold::of(&f, e.components.iter().find(|c| c.id == 3).unwrap()).unwrap();
        let g = hold.gizmo(2.0);
        assert_eq!(g.arrows, vec![(Axis::X, hold.along), (Axis::Y, hold.across), (Axis::Z, hold.normal)]);
        assert_eq!((g.rings.clone(), g.origin), (vec![(Axis::Spin, hold.normal)], hold.origin));
        let foot = hold.foot();
        let dial = g.dial.unwrap();
        assert!(close3(dial.point(dial.theta_deg), foot, 1e-9), "the dial runs through the stone's foot");
        // The along arrow: a camera ray crossing its line 0.25 mm on reads 0.25 mm of seat.
        let (cmd, lock, key) = hold.drag(Handle::Move(Axis::X)).unwrap();
        assert_eq!((cmd.key(), lock, key), ("move", Some(Axis::X), "u"));
        let mut s = Session::default();
        s.start(cmd);
        s.feed(StepInput::Lock(Axis::X));
        let aim = |p: [f64; 3]| Ray { origin: plus(p, &[(hold.normal, 30.0)]), direction: hold.normal.map(|v| -v) };
        let seen = |p: [f64; 3]| {
            let t = along_line(aim(p), g.origin, hold.along).unwrap();
            StepInput::Pointer { world: plus(g.origin, &[(hold.along, t)]), normal: hold.along, theta_deg: 0.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: true }
        };
        s.feed(seen(plus(g.origin, &[(hold.along, 0.5), (hold.across, 0.2)])));
        s.feed(seen(plus(g.origin, &[(hold.along, 0.75), (hold.across, 0.9)])));
        let Effect::Operation { operation, .. } = one(s.enter()) else { panic!() };
        let seat = FaceHold::seat_in(&operation).unwrap();
        assert!((seat.u_mm - hold.seat.u_mm - 0.25).abs() < 1e-9 && seat.v_mm == hold.seat.v_mm, "{seat:?}");
        // The ring spins it about the face's normal.
        let (cmd, lock, key) = hold.drag(Handle::Turn(Axis::Spin)).unwrap();
        assert_eq!((cmd.key(), lock, key), ("rotate", Some(Axis::Spin), "spin"));
        let mut s = Session::default();
        s.start(cmd);
        let on_ring = |p: [f64; 3]| StepInput::Pointer { world: on_plane(aim(p), g.origin, hold.normal).unwrap(), normal: hold.normal, theta_deg: 0.0, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: true };
        s.feed(on_ring(plus(g.origin, &[(hold.along, 1.0)])));
        s.feed(on_ring(plus(g.origin, &[(hold.along, -1.0), (hold.across, 1e-9)])));
        let Effect::Operation { operation, .. } = one(s.enter()) else { panic!() };
        assert!((FaceHold::seat_in(&operation).unwrap().spin_deg.abs() - 180.0).abs() < 1e-6);
        // The dial slides the stone round the finger: its foot turned 5° and dropped along the normal back onto the face.
        let (cmd, lock, _) = hold.drag(Handle::Dial).unwrap();
        assert_eq!((cmd.key(), lock), ("move", None));
        let mut s = Session::default();
        s.start(cmd);
        let dialled = |deg: f64| StepInput::Pointer { world: dial.point(deg), normal: [0.0, 0.0, 1.0], theta_deg: deg, across_mm: dial.z, height_mm: 0.0, snapped: None, dragging: true };
        s.feed(dialled(dial.theta_deg));
        s.feed(dialled(dial.theta_deg + 5.0));
        let Effect::Operation { operation, .. } = one(s.enter()) else { panic!() };
        let seat = FaceHold::seat_in(&operation).unwrap();
        let turned = dial.point(dial.theta_deg + 5.0);
        let dropped = plus(turned, &[(hold.normal, -dot(std::array::from_fn(|k| turned[k] - foot[k]), hold.normal))]);
        let landed = plus(foot, &[(hold.along, seat.u_mm - hold.seat.u_mm), (hold.across, seat.v_mm - hold.seat.v_mm)]);
        assert!(close3(landed, dropped, 1e-9) && seat.height_mm == hold.seat.height_mm, "{landed:?} against {dropped:?}");
        assert!(hold.drag(Handle::Grip(0)).is_none());
    }

    #[test]
    fn attach_cycles_on_click_types_by_index_and_commits_on_enter() {
        let mut s = Session::default();
        s.start(Box::new(AttachCmd::new(2, Attach::Separate)));
        s.feed(StepInput::Click);
        assert_eq!(s.preview().unwrap().caption, "Attach: Join");
        s.feed(StepInput::Click);
        assert_eq!(s.preview().unwrap().caption, "Attach: Cut");
        s.feed(StepInput::Click);
        assert_eq!(s.preview().unwrap().caption, "Attach: Separate");
        assert!(matches!(s.feed(StepInput::Typed { key: "attach", value: 3.0 }), Outcome::Refused(_)));
        assert!(matches!(s.feed(StepInput::Typed { key: "attach", value: -1.0 }), Outcome::Refused(_)));
        s.feed(StepInput::Typed { key: "attach", value: 1.0 });
        assert_eq!(s.dimensions()[0].value, 1.0);
        let e = effects(s.enter());
        assert!(matches!(e.as_slice(), [Effect::Attach { feature: 2, attach: Attach::Join }]));
    }
}
