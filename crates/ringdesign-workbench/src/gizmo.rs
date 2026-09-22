//! The ring-frame gizmo: handles on a chosen part that move and turn it in the ring's own frame.
//!
//! A part seated on the ring gets arrows round the ring, across the band and off the surface, rings
//! that spin, tilt and cant it about the axes those turns turn it about, and the ring dial that slides
//! it round the shank; a free part gets the world's X, Y and Z. Arrows and rings keep their screen
//! size and stand clear of the part on screen; its size grips stand on the part.
use crate::command::snap::wrap360;
use crate::command::{Affine, Axis, BandSurface, Pivot, StepInput, along_line, on_plane, plane_basis, seat};
use crate::grips::{self, Grip};
use egui::{Align2, Color32, FontId, Painter, Pos2, Shape, Stroke};
use ringdesign_core::{
    Mesh, RingDesign,
    cad::{Operation, Placement},
    interaction::pick::Ray,
};

/// An arrow's length on screen.
pub const ARROW_PX: f32 = 56.0;
/// The least gap between the part's origin and an arrow's base on screen.
pub const GAP_PX: f32 = 16.0;
/// A turning ring's least radius on screen.
pub const RING_PX: f32 = 46.0;
/// How far arrows and rings stand clear of the part on screen.
pub const CLEAR_PX: f32 = 10.0;
/// How near a handle the pointer takes it.
pub const HIT_PX: f32 = 7.0;
/// A ring seen nearer edge-on than this share of its face is left out.
const EDGE_ON: f64 = 0.15;
/// An arrow or grip foreshortened past this share of its length is left out.
const END_ON: f64 = 0.25;
const RING_SEGMENTS: usize = 64;
const DIAL_SEGMENTS: usize = 120;

const RED: Color32 = Color32::from_rgb(232, 96, 112);
const GREEN: Color32 = Color32::from_rgb(120, 214, 128);
const BLUE: Color32 = Color32::from_rgb(98, 156, 244);
const DIAL: Color32 = Color32::from_rgb(103, 217, 213);
const GRIP: Color32 = Color32::from_rgb(244, 244, 246);
const HOT: Color32 = Color32::from_rgb(255, 226, 110);

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn add(a: [f64; 3], b: [f64; 3], s: f64) -> [f64; 3] {
    std::array::from_fn(|k| a[k] + b[k] * s)
}

/// One handle of the gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    /// An arrow moving the part along one degree of freedom.
    Move(Axis),
    /// A ring turning the part about one axis.
    Turn(Axis),
    /// The ring dial's marker: the part's angle round the shank.
    Dial,
    /// A size grip, by its index in the gizmo's grips.
    Grip(usize),
}

fn color(handle: Handle) -> Color32 {
    match handle {
        Handle::Move(Axis::Theta | Axis::X) | Handle::Turn(Axis::Cant | Axis::X) => RED,
        Handle::Move(Axis::Height | Axis::Y) | Handle::Turn(Axis::Spin | Axis::Y) => GREEN,
        Handle::Move(_) | Handle::Turn(_) => BLUE,
        Handle::Dial => DIAL,
        Handle::Grip(_) => GRIP,
    }
}

/// A size grip carried into the world by its part's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub grip: Grip,
    pub start: [f64; 3],
    pub at: [f64; 3],
    /// Unit, in the world.
    pub direction: [f64; 3],
}

/// The ring dial: a circle round the finger axis at the part's crest radius, in the plane of its across.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dial {
    pub z: f64,
    pub radius: f64,
    pub theta_deg: f64,
    /// The stand-off the part keeps as it slides.
    pub height_mm: f64,
}

impl Dial {
    /// The angle round the ring a camera ray points at on the dial's plane, rounded to `snap_deg` when given.
    pub fn theta_at(&self, ray: Ray, snap_deg: Option<f64>) -> Option<f64> {
        let p = on_plane(ray, [0.0, 0.0, self.z], [0.0, 0.0, 1.0])?;
        if p[0].hypot(p[1]) < 1e-9 {
            return None;
        }
        let theta = wrap360(p[1].atan2(p[0]).to_degrees());
        Some(match snap_deg.filter(|s| *s > 0.0) {
            Some(s) => wrap360((theta / s).round() * s),
            None => theta,
        })
    }

    /// The dial's point at `theta_deg`.
    pub fn point(&self, theta_deg: f64) -> [f64; 3] {
        let (s, c) = theta_deg.to_radians().sin_cos();
        [self.radius * c, self.radius * s, self.z]
    }
}

/// A chosen part's gizmo in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Gizmo {
    pub origin: [f64; 3],
    /// The part's own frame in the world, which carries its grips.
    pub frame: Affine,
    /// Each arrow's degree of freedom and its unit direction.
    pub arrows: Vec<(Axis, [f64; 3])>,
    /// Each ring's turn and the unit axis it turns about, in the turning command's own order.
    pub rings: Vec<(Axis, [f64; 3])>,
    pub dial: Option<Dial>,
    pub grips: Vec<Placed>,
    /// How far the part reaches from its origin.
    pub reach_mm: f64,
    /// The stand-off a seated part keeps off the surface.
    pub height_mm: f64,
}

/// How far a placed part's tessellation reaches from `origin`.
pub fn reach(mesh: &Mesh, origin: [f64; 3]) -> f64 {
    mesh.vertices.iter().map(|v| (f64::from(v.0) - origin[0]).hypot(f64::from(v.1) - origin[1]).hypot(f64::from(v.2) - origin[2])).fold(0.0, f64::max)
}

impl Gizmo {
    /// A part seated on `surface` as the build seats it: arrows along its seat's tangent, across and normal, rings for spin, tilt and cant, and the dial.
    pub fn on_ring(design: &RingDesign, surface: Option<&BandSurface>, placement: &Placement, reach_mm: f64) -> Option<Self> {
        let Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, .. } = *placement else { return None };
        let bare = Placement::Ring { theta_deg, across_mm, height_mm, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let s = seat(design, surface, &bare)?;
        let placed = seat(design, surface, placement)?;
        let (x, y, z) = (s.axis(0), s.axis(1), s.axis(2));
        let origin = s.origin();
        // Spin turns about the seat's normal, cant about the spun tangent, tilt about the leaned part's own x.
        let (sin, cos) = spin_deg.to_radians().sin_cos();
        let cant = std::array::from_fn(|k| cos * y[k] - sin * x[k]);
        let foot = add(origin, z, -height_mm);
        Some(Self {
            origin,
            frame: placed,
            arrows: vec![(Axis::Theta, y), (Axis::Across, x.map(|v| -v)), (Axis::Height, z)],
            rings: vec![(Axis::Spin, z), (Axis::Tilt, placed.axis(0)), (Axis::Cant, cant)],
            dial: Some(Dial { z: across_mm, radius: foot[0].hypot(foot[1]), theta_deg, height_mm }),
            grips: Vec::new(),
            reach_mm,
            height_mm,
        })
    }

    /// A free part, built where it stands: the world's axes through its centre.
    pub fn free(centre: [f64; 3], reach_mm: f64) -> Self {
        let axes = [(Axis::X, [1.0, 0.0, 0.0]), (Axis::Y, [0.0, 1.0, 0.0]), (Axis::Z, [0.0, 0.0, 1.0])];
        Self { origin: centre, frame: Affine::IDENTITY, arrows: axes.to_vec(), rings: axes.to_vec(), dial: None, grips: Vec::new(), reach_mm, height_mm: 0.0 }
    }

    /// The part's size grips carried into the world by its frame; position grips are the arrows' work.
    pub fn with_grips(mut self, op: &Operation) -> Self {
        let frame = self.frame;
        self.grips = grips::grips(op)
            .into_iter()
            .filter(|g| !g.position)
            .filter_map(|grip| {
                let d = frame.turn(grip.direction);
                let len = dot(d, d).sqrt();
                (len > 1e-12).then(|| Placed { start: frame.apply(grip.start), at: frame.apply(grip.at), direction: d.map(|v| v / len), grip })
            })
            .collect();
        self
    }

    /// Where a drag of a ring turns the part about, for the turning command.
    pub fn pivot(&self) -> Pivot {
        let axis = |k: usize| self.rings.get(k).map_or([0.0, 0.0, 1.0], |r| r.1);
        Pivot { centre: self.origin, axes: [axis(0), axis(1), axis(2)] }
    }

    pub fn label(&self, handle: Handle) -> String {
        match handle {
            Handle::Move(Axis::Theta) => "Gizmo: move round the ring".into(),
            Handle::Move(Axis::Across) => "Gizmo: move across the band".into(),
            Handle::Move(Axis::Height) => "Gizmo: move off the surface".into(),
            Handle::Move(a) => format!("Gizmo: move along {}", a.key().to_uppercase()),
            Handle::Turn(Axis::Spin) => "Gizmo: spin".into(),
            Handle::Turn(Axis::Tilt) => "Gizmo: tilt along the ring".into(),
            Handle::Turn(Axis::Cant) => "Gizmo: cant across the band".into(),
            Handle::Turn(a) => format!("Gizmo: turn about {}", a.key().to_uppercase()),
            Handle::Dial => "Gizmo: slide round the shank".into(),
            Handle::Grip(i) => format!("Grip: {}", self.grips.get(i).map_or("size", |g| g.grip.label)),
        }
    }

    /// The dimension a number typed during the handle's drag goes to.
    pub fn key(&self, handle: Handle) -> Option<&'static str> {
        match handle {
            Handle::Move(a) | Handle::Turn(a) => Some(a.key()),
            Handle::Dial => Some("theta"),
            Handle::Grip(i) => self.grips.get(i).map(|g| g.grip.key),
        }
    }

    /// The pointer a drag of `handle` reads off a camera ray: an arrow's or grip's nearest line point, a ring's plane crossing, or the dial's snapped angle.
    pub fn token(&self, handle: Handle, ray: Ray, snap_deg: Option<f64>) -> Option<StepInput> {
        let pointer = |world: [f64; 3], normal: [f64; 3], theta_deg: f64, height_mm: f64| StepInput::Pointer {
            world,
            normal,
            theta_deg,
            across_mm: world[2],
            height_mm,
            snapped: None,
            dragging: true,
        };
        let theta = |w: [f64; 3]| wrap360(w[1].atan2(w[0]).to_degrees());
        match handle {
            Handle::Move(axis) => {
                let (_, dir) = *self.arrows.iter().find(|a| a.0 == axis)?;
                let t = along_line(ray, self.origin, dir)?;
                let world = add(self.origin, dir, t);
                let height = self.height_mm + if axis == Axis::Height { t } else { 0.0 };
                Some(pointer(world, dir, theta(world), height))
            }
            Handle::Turn(axis) => {
                let (_, n) = *self.rings.iter().find(|r| r.0 == axis)?;
                let world = on_plane(ray, self.origin, n)?;
                Some(pointer(world, n, theta(world), self.height_mm))
            }
            Handle::Dial => {
                let dial = self.dial?;
                let at = dial.theta_at(ray, snap_deg)?;
                let world = dial.point(at);
                let (s, c) = at.to_radians().sin_cos();
                Some(pointer(world, [c, s, 0.0], at, dial.height_mm))
            }
            Handle::Grip(i) => {
                let g = self.grips.get(i)?;
                let t = along_line(ray, g.at, g.direction)?;
                let world = add(g.at, g.direction, t);
                Some(pointer(world, g.direction, theta(world), self.height_mm))
            }
        }
    }

    /// Lays the gizmo out on screen: its handles, the dial's scale and the grips' dimension lines.
    pub fn layout(&self, view: &View) -> Layout {
        let px = view.px_per_mm.max(1e-9);
        let fwd = view.forward;
        let project = |p: [f64; 3]| (view.project)(p);
        let reach_px = (self.reach_mm * px) as f32;
        let mut marks = Vec::new();
        let share = |d: [f64; 3]| (1.0 - dot(d, fwd).powi(2)).max(0.0).sqrt();
        let base_px = GAP_PX.max(reach_px + CLEAR_PX);
        for &(axis, dir) in &self.arrows {
            let f = share(dir);
            if f < END_ON {
                continue;
            }
            let mm = |screen: f32| f64::from(screen) / (px * f);
            let (base, tip) = (project(add(self.origin, dir, mm(base_px))), project(add(self.origin, dir, mm(base_px + ARROW_PX))));
            marks.push((Handle::Move(axis), Mark::Arrow { base, tip }));
        }
        let ring_mm = f64::from(RING_PX.max(reach_px + CLEAR_PX)) / px;
        for &(axis, n) in &self.rings {
            if dot(n, fwd).abs() < EDGE_ON {
                continue;
            }
            let (u, v) = plane_basis(n);
            let (points, far) = (0..=RING_SEGMENTS)
                .map(|i| {
                    let a = i as f64 / RING_SEGMENTS as f64 * std::f64::consts::TAU;
                    let off: [f64; 3] = std::array::from_fn(|k| ring_mm * (u[k] * a.cos() + v[k] * a.sin()));
                    (project(add(self.origin, off, 1.0)), dot(off, fwd) > 0.0)
                })
                .unzip();
            marks.push((Handle::Turn(axis), Mark::Ring { points, far }));
        }
        let dial = self.dial.filter(|_| fwd[2].abs() >= EDGE_ON).map(|dial| {
            let circle: Vec<([f64; 3], Pos2)> = (0..=DIAL_SEGMENTS)
                .map(|i| {
                    let p = dial.point(i as f64 * 360.0 / DIAL_SEGMENTS as f64);
                    (p, project(p))
                })
                .collect();
            let far = circle.iter().map(|(p, _)| dot([p[0], p[1], 0.0], fwd) > 0.0).collect();
            let out = |theta: f64, screen: f32| {
                let (s, c) = theta.to_radians().sin_cos();
                project(add(dial.point(theta), [c, s, 0.0], f64::from(screen) / px))
            };
            let ticks = (0..24).map(|i| f64::from(i) * 15.0).map(|t| (project(dial.point(t)), out(t, if t % 45.0 == 0.0 { 9.0 } else { 5.0 }))).collect();
            let labels = (0..8).map(|i| f64::from(i) * 45.0).map(|t| (out(t, 20.0), format!("{t:.0}°"))).collect();
            marks.push((Handle::Dial, Mark::Dot { at: project(dial.point(dial.theta_deg)), radius: 6.0 }));
            DialMarks { circle: circle.into_iter().map(|(_, s)| s).collect(), far, ticks, labels }
        });
        let mut lines = Vec::new();
        for (i, g) in self.grips.iter().enumerate() {
            if share(g.direction) < END_ON {
                continue;
            }
            lines.push((project(g.start), project(g.at)));
            marks.push((Handle::Grip(i), Mark::Dot { at: project(g.at), radius: 4.5 }));
        }
        Layout { marks, dial, lines }
    }
}

/// How the gizmo meets the screen: world to screen, the direction the camera looks, and the scale.
pub struct View<'a> {
    pub project: &'a dyn Fn([f64; 3]) -> Pos2,
    pub forward: [f64; 3],
    pub px_per_mm: f64,
}

/// A handle as drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Arrow { base: Pos2, tip: Pos2 },
    /// A closed loop, with which of its segments run behind the part.
    Ring { points: Vec<Pos2>, far: Vec<bool> },
    Dot { at: Pos2, radius: f32 },
}

/// The dial's circle and scale on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct DialMarks {
    pub circle: Vec<Pos2>,
    pub far: Vec<bool>,
    pub ticks: Vec<(Pos2, Pos2)>,
    pub labels: Vec<(Pos2, String)>,
}

fn segment_px(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

fn polyline_px(p: Pos2, points: &[Pos2]) -> f32 {
    points.windows(2).map(|w| segment_px(p, w[0], w[1])).fold(f32::INFINITY, f32::min)
}

/// The gizmo on screen: what is drawn and what a press can take.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub marks: Vec<(Handle, Mark)>,
    pub dial: Option<DialMarks>,
    /// The grips' dimension lines.
    pub lines: Vec<(Pos2, Pos2)>,
}

impl Layout {
    /// How far `p` is from a handle's mark, less the head start small targets get over long ones.
    fn distance(mark: &Mark, handle: Handle, p: Pos2) -> Option<f32> {
        let (d, tolerance, favour) = match mark {
            Mark::Arrow { base, tip } => (segment_px(p, *base, *tip), HIT_PX, 0.0),
            Mark::Ring { points, .. } => (polyline_px(p, points), HIT_PX, 0.0),
            Mark::Dot { at, radius } => (p.distance(*at), radius + 4.0, if matches!(handle, Handle::Grip(_)) { 3.0 } else { 2.0 }),
        };
        (d <= tolerance).then_some(d - favour)
    }

    /// The handle a press at `p` takes, the nearest when several are in reach.
    pub fn hit(&self, p: Pos2) -> Option<Handle> {
        self.marks.iter().filter_map(|(h, m)| Self::distance(m, *h, p).map(|d| (*h, d))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| h)
    }

    /// The handles drawn, in drawing order.
    pub fn handles(&self) -> impl Iterator<Item = Handle> + '_ {
        self.marks.iter().map(|(h, _)| *h)
    }

    /// The layout with only `handle` in it, as a drag draws it.
    pub fn only(&self, handle: Handle) -> Self {
        Self { marks: self.marks.iter().filter(|(h, _)| *h == handle).cloned().collect(), dial: self.dial.clone().filter(|_| handle == Handle::Dial), lines: Vec::new() }
    }

    /// A point on the handle that a press takes it at rather than another; `None` for a handle not drawn.
    pub fn anchor(&self, handle: Handle) -> Option<Pos2> {
        let (_, mark) = self.marks.iter().find(|(h, _)| *h == handle)?;
        let candidates: Vec<Pos2> = match mark {
            Mark::Arrow { base, tip } => (0..=8).map(|i| base.lerp(*tip, 0.3 + 0.08 * i as f32)).collect(),
            Mark::Ring { points, far } => points.iter().zip(far).step_by(4).filter(|(_, f)| !**f).map(|(p, _)| *p).collect(),
            Mark::Dot { at, .. } => return Some(*at),
        };
        // The candidate furthest from every other handle.
        let clearance = |p: Pos2| {
            self.marks
                .iter()
                .filter(|(h, _)| *h != handle)
                .map(|(_, m)| match m {
                    Mark::Arrow { base, tip } => segment_px(p, *base, *tip),
                    Mark::Ring { points, .. } => polyline_px(p, points),
                    Mark::Dot { at, .. } => p.distance(*at),
                })
                .fold(f32::INFINITY, f32::min)
        };
        candidates.into_iter().max_by(|a, b| clearance(*a).total_cmp(&clearance(*b)))
    }
}

/// Draws the layout over the metal: `hot` is under the pointer, `active` is being dragged.
pub fn paint(painter: &Painter, layout: &Layout, hot: Option<Handle>, active: Option<Handle>) {
    let lit = |h: Handle| Some(h) == hot || Some(h) == active;
    if let Some(d) = &layout.dial {
        let stroke = |far: bool| Stroke::new(1.2, DIAL.gamma_multiply(if far { 0.3 } else { 0.7 }));
        for (w, far) in d.circle.windows(2).zip(&d.far) {
            painter.line_segment([w[0], w[1]], stroke(*far));
        }
        for (a, b) in &d.ticks {
            painter.line_segment([*a, *b], Stroke::new(1.0, DIAL.gamma_multiply(0.8)));
        }
        for (at, text) in &d.labels {
            painter.text(*at, Align2::CENTER_CENTER, text, FontId::proportional(10.0), DIAL.gamma_multiply(0.9));
        }
    }
    for (a, b) in &layout.lines {
        painter.line_segment([*a, *b], Stroke::new(1.0, GRIP.gamma_multiply(0.45)));
    }
    for (h, mark) in &layout.marks {
        let c = if lit(*h) { HOT } else { color(*h) };
        let width = if lit(*h) { 3.5 } else { 2.2 };
        match mark {
            Mark::Arrow { base, tip } => {
                let dir = (*tip - *base).normalized();
                let side = egui::vec2(-dir.y, dir.x);
                let neck = *tip - dir * 11.0;
                painter.line_segment([*base, neck], Stroke::new(width, c));
                painter.add(Shape::convex_polygon(vec![*tip, neck + side * 5.5, neck - side * 5.5], c, Stroke::NONE));
            }
            Mark::Ring { points, far } => {
                for (w, far) in points.windows(2).zip(far) {
                    painter.line_segment([w[0], w[1]], Stroke::new(width, if *far { c.gamma_multiply(0.4) } else { c }));
                }
            }
            Mark::Dot { at, radius } => match h {
                Handle::Grip(_) => {
                    let r = if lit(*h) { radius + 1.5 } else { *radius };
                    painter.rect(egui::Rect::from_center_size(*at, egui::Vec2::splat(r * 2.0)), 1.0, c, Stroke::new(1.0, Color32::from_black_alpha(200)), egui::StrokeKind::Outside);
                }
                _ => {
                    painter.circle(*at, if lit(*h) { radius + 2.0 } else { *radius }, c, Stroke::new(1.5, Color32::from_black_alpha(200)));
                }
            },
        }
    }
}

/// Draws the arc a ring drag has swept, from where it was taken to where it is.
pub fn paint_sweep(painter: &Painter, gizmo: &Gizmo, handle: Handle, from: [f64; 3], to: [f64; 3], view: &View) {
    let Handle::Turn(axis) = handle else { return };
    let Some((_, n)) = gizmo.rings.iter().find(|r| r.0 == axis) else { return };
    let (u, v) = plane_basis(*n);
    let angle = |p: [f64; 3]| {
        let w: [f64; 3] = std::array::from_fn(|k| p[k] - gizmo.origin[k]);
        dot(w, v).atan2(dot(w, u))
    };
    let (a, b) = (angle(from), angle(to));
    let sweep = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    let reach_px = (gizmo.reach_mm * view.px_per_mm) as f32;
    let r = f64::from(RING_PX.max(reach_px + CLEAR_PX)) / view.px_per_mm.max(1e-9);
    let steps = ((sweep.abs() / std::f64::consts::TAU * RING_SEGMENTS as f64).ceil() as usize).max(1);
    let mut fan = vec![(view.project)(gizmo.origin)];
    fan.extend((0..=steps).map(|i| {
        let t = a + sweep * i as f64 / steps as f64;
        (view.project)(std::array::from_fn(|k| gizmo.origin[k] + r * (u[k] * t.cos() + v[k] * t.sin())))
    }));
    let c = color(handle);
    for w in fan[1..].windows(2) {
        painter.add(Shape::convex_polygon(vec![fan[0], w[0], w[1]], c.gamma_multiply(0.18), Stroke::NONE));
    }
    painter.line_segment([fan[0], fan[1]], Stroke::new(1.2, c));
    if let Some(last) = fan.last() {
        painter.line_segment([fan[0], *last], Stroke::new(1.2, c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Effect, GripCmd, MoveCmd, Outcome, PlaceCmd, RotateCmd, Session, ViewCommand, angle_about};
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Attach, Component, Feature},
        mesh, templates,
    };

    /// An orthographic camera looking along `forward` with `up` on screen, 20 px/mm, centred on `at`.
    struct Camera {
        forward: [f64; 3],
        right: [f64; 3],
        up: [f64; 3],
        at: [f64; 3],
    }
    const PX: f64 = 20.0;
    const CENTRE: Pos2 = Pos2::new(400.0, 300.0);
    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    }
    impl Camera {
        fn new(forward: [f64; 3], up: [f64; 3], at: [f64; 3]) -> Self {
            Self { forward, right: cross(forward, up), up, at }
        }
        fn project(&self, p: [f64; 3]) -> Pos2 {
            let d: [f64; 3] = std::array::from_fn(|k| p[k] - self.at[k]);
            CENTRE + egui::vec2((dot(d, self.right) * PX) as f32, (-dot(d, self.up) * PX) as f32)
        }
        fn ray(&self, s: Pos2) -> Ray {
            let (x, y) = (f64::from(s.x - CENTRE.x) / PX, -f64::from(s.y - CENTRE.y) / PX);
            Ray { origin: std::array::from_fn(|k| self.at[k] + self.right[k] * x + self.up[k] * y - self.forward[k] * 50.0), direction: self.forward }
        }
        fn layout(&self, g: &Gizmo) -> Layout {
            g.layout(&View { project: &|p| self.project(p), forward: self.forward, px_per_mm: PX })
        }
    }
    fn court() -> RingDesign {
        templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }
    fn band(d: &RingDesign) -> BandSurface {
        BandSurface::new(mesh::build(d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() }).mesh)
    }
    fn close3(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }
    fn commit(o: Outcome) -> Vec<Effect> {
        match o {
            Outcome::Commit(e) => e,
            other => panic!("expected a commit, got {other:?}"),
        }
    }
    fn placement_of(o: Outcome) -> Placement {
        match commit(o).as_slice() {
            [Effect::Placement { placement, .. }] => placement.clone(),
            other => panic!("{other:?}"),
        }
    }
    fn added(o: Outcome) -> Feature {
        match commit(o).as_slice() {
            [Effect::Add { feature }] => feature.clone(),
            other => panic!("{other:?}"),
        }
    }
    fn post(placement: Placement) -> Feature {
        Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, component: Component { placement, attach: Attach::Join, ..Component::default() } }
    }

    #[test]
    fn the_projections_read_a_line_a_plane_and_an_angle_the_right_way_round() {
        // A ray down −y crossing the line x = 3 along z at z = 2: two mm past the point it starts from.
        let ray = Ray { origin: [3.0, 40.0, 2.0], direction: [0.0, -1.0, 0.0] };
        assert!((along_line(ray, [3.0, 10.0, 0.0], [0.0, 0.0, 1.0]).unwrap() - 2.0).abs() < 1e-12);
        assert!((along_line(ray, [3.0, 10.0, 0.0], [0.0, 0.0, -4.0]).unwrap() + 2.0).abs() < 1e-12, "the direction is taken as a unit");
        assert!(along_line(ray, [3.0, 10.0, 0.0], [0.0, 1.0, 0.0]).is_none(), "a line along the ray has no nearest point");
        assert_eq!(on_plane(ray, [0.0, 10.0, 0.0], [0.0, 1.0, 0.0]), Some([3.0, 10.0, 2.0]));
        assert!(on_plane(ray, [0.0, 10.0, 0.0], [1.0, 0.0, 0.0]).is_none(), "a plane seen edge-on is not crossed");
        // Right-handed: a quarter turn about +z carries +u to +v.
        for axis in [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.3, -0.5, 0.8]] {
            let (u, v) = plane_basis(axis);
            let n = cross(u, v);
            let l = dot(axis, axis).sqrt();
            assert!(close3(n, axis.map(|x| x / l), 1e-12) && dot(u, v).abs() < 1e-12);
            let c = [1.0, 2.0, 3.0];
            assert!((angle_about(add(c, u, 2.0), c, axis)).abs() < 1e-12);
            assert!((angle_about(add(c, v, 0.5), c, axis) - 90.0).abs() < 1e-9);
        }
    }

    #[test]
    fn a_seated_part_gets_the_ring_frame_and_the_dial_at_its_crest_radius() {
        let d = court();
        let b = band(&d);
        let g = Gizmo::on_ring(&d, Some(&b), &Placement::ring(90.0, 0.25), 1.95).unwrap();
        let (hit, n) = b.hit(90.0, 0.0).unwrap();
        assert!(close3(g.origin, add(hit, n, 0.25), 1e-9), "the gizmo stands where the build seats the part");
        let arrow = |a: Axis| g.arrows.iter().find(|x| x.0 == a).unwrap().1;
        let ring = |a: Axis| g.rings.iter().find(|x| x.0 == a).unwrap().1;
        // At the top: round the ring is −x, across is the finger axis, off the surface is +y, to the seat's own
        // normal, which `surface_hit` reads a tenth of a micron off the crest and so leans a degree along the finger.
        assert!(close3(arrow(Axis::Theta), [-1.0, 0.0, 0.0], 0.02) && close3(arrow(Axis::Across), [0.0, 0.0, 1.0], 0.02) && close3(arrow(Axis::Height), [0.0, 1.0, 0.0], 0.02));
        assert!(close3(ring(Axis::Spin), [0.0, 1.0, 0.0], 0.02) && close3(ring(Axis::Tilt), [0.0, 0.0, -1.0], 0.02) && close3(ring(Axis::Cant), [-1.0, 0.0, 0.0], 0.02));
        assert!(dot(arrow(Axis::Across), arrow(Axis::Height)).abs() < 1e-12, "across runs square to the normal");
        assert!(close3(arrow(Axis::Height), n, 1e-12), "off the surface is the surface's own normal");
        let dial = g.dial.unwrap();
        assert!((dial.radius - hit[0].hypot(hit[1])).abs() < 1e-9 && dial.z == 0.0 && dial.theta_deg == 90.0);
        let r = d.inner_radius_mm() + d.profile.thickness_mm;
        assert!((dial.radius - r).abs() < 0.01, "on a plain band the crest: {} vs {r}", dial.radius);
        assert!(Gizmo::on_ring(&d, Some(&b), &Placement::Free, 1.0).is_none());
        assert_eq!(g.label(Handle::Move(Axis::Theta)), "Gizmo: move round the ring");
        assert_eq!(g.key(Handle::Turn(Axis::Cant)), Some("cant"));
        assert_eq!(Gizmo::free([0.0; 3], 1.0).label(Handle::Turn(Axis::Z)), "Gizmo: turn about Z");
    }

    #[test]
    fn the_round_the_ring_arrow_moves_the_part_by_what_its_axis_projection_says() {
        let d = court();
        let b = band(&d);
        let target = post(Placement::ring(90.0, 0.25));
        let g = Gizmo::on_ring(&d, Some(&b), &target.component.placement, 1.95).unwrap();
        // Looking down −y at the top: screen right is −x, which is round the ring.
        let cam = Camera::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0], g.origin);
        let layout = cam.layout(&g);
        let press = layout.anchor(Handle::Move(Axis::Theta)).unwrap();
        assert_eq!(layout.hit(press), Some(Handle::Move(Axis::Theta)));
        let to = press + egui::vec2(40.0, 0.0);
        let mut s = Session::default();
        s.start(Box::new(MoveCmd::of(&target, 9)));
        s.feed(StepInput::Lock(Axis::Theta));
        s.feed(g.token(Handle::Move(Axis::Theta), cam.ray(press), None).unwrap());
        s.feed(g.token(Handle::Move(Axis::Theta), cam.ray(to), None).unwrap());
        let r0 = g.origin[0].hypot(g.origin[1]);
        let t = |p: Pos2| f64::from(p.x - CENTRE.x) / PX;
        let expected = (t(to) / r0).atan().to_degrees() - (t(press) / r0).atan().to_degrees();
        let moved = placement_of(s.enter()).theta_deg().unwrap() - 90.0;
        assert!((moved - expected).abs() < 1e-4, "{moved} vs {expected}");
        assert!(moved > 5.0 && moved < 40.0 / PX / r0 * 180.0 / std::f64::consts::PI, "two millimetres along the tangent, read out past the part: {moved}");
    }

    #[test]
    fn the_height_arrow_and_the_across_arrow_move_exactly_as_far_as_the_pointer_along_them() {
        let d = court();
        let b = band(&d);
        let target = post(Placement::Ring { theta_deg: 90.0, across_mm: 0.4, height_mm: 0.25, spin_deg: 20.0, tilt_deg: 0.0, cant_deg: 0.0 });
        let g = Gizmo::on_ring(&d, Some(&b), &target.component.placement, 1.95).unwrap();
        // Looking down the finger from +z: off the surface is up the screen.
        let cam = Camera::new([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], g.origin);
        let layout = cam.layout(&g);
        assert!(layout.anchor(Handle::Move(Axis::Across)).is_none(), "an arrow pointing at the eye is left out");
        let press = layout.anchor(Handle::Move(Axis::Height)).unwrap();
        let mut c = MoveCmd::of(&target, 9);
        c.feed(&StepInput::Lock(Axis::Height));
        c.feed(&g.token(Handle::Move(Axis::Height), cam.ray(press), None).unwrap());
        c.feed(&g.token(Handle::Move(Axis::Height), cam.ray(press + egui::vec2(3.0, -30.0)), None).unwrap());
        let p = placement_of(c.feed(&StepInput::Confirm));
        // The nearest point of the normal's line: the screen move along it, over its share of the screen.
        let n = g.arrows[2].1;
        let expected = 0.25 + (3.0 * n[0] + 30.0 * n[1]) / PX / (1.0 - n[2] * n[2]);
        assert!(n[2] > 0.01, "off the crest the normal leans along the finger: {n:?}");
        assert!(matches!(p, Placement::Ring { height_mm, across_mm, theta_deg, spin_deg, .. } if (height_mm - expected).abs() < 1e-6 && across_mm == 0.4 && theta_deg == 90.0 && spin_deg == 20.0), "{p:?} vs {expected}");
        // From the side the across arrow reads the finger axis: the screen move along it, times its share of the finger.
        let side = Camera::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], g.origin);
        let press = side.layout(&g).anchor(Handle::Move(Axis::Across)).unwrap();
        let mut c = MoveCmd::of(&target, 9);
        c.feed(&StepInput::Lock(Axis::Across));
        c.feed(&g.token(Handle::Move(Axis::Across), side.ray(press), None).unwrap());
        c.feed(&g.token(Handle::Move(Axis::Across), side.ray(press + egui::vec2(-24.0, 0.0)), None).unwrap());
        let p = placement_of(c.feed(&StepInput::Confirm));
        let a = g.arrows[1].1;
        let expected = 0.4 - 24.0 / PX * a[2] * a[2];
        assert!(matches!(p, Placement::Ring { across_mm, height_mm, .. } if (across_mm - expected).abs() < 1e-6 && height_mm == 0.25), "{p:?} vs {expected}");
    }

    /// The frame a placement gives, as three axes.
    fn axes(d: &RingDesign, b: &BandSurface, p: &Placement) -> [[f64; 3]; 3] {
        let f = seat(d, Some(b), p).unwrap();
        [f.axis(0), f.axis(1), f.axis(2)]
    }
    /// `v` turned by `deg` about the unit `axis`.
    fn turned(v: [f64; 3], axis: [f64; 3], deg: f64) -> [f64; 3] {
        let (s, c) = deg.to_radians().sin_cos();
        let k = cross(axis, v);
        let d = dot(axis, v);
        std::array::from_fn(|i| v[i] * c + k[i] * s + axis[i] * d * (1.0 - c))
    }

    #[test]
    fn each_ring_turns_the_part_about_its_own_axis_by_the_angle_dragged_round_it() {
        let d = court();
        let b = band(&d);
        let base = Placement::Ring { theta_deg: 100.0, across_mm: 0.3, height_mm: 0.1, spin_deg: 25.0, tilt_deg: -8.0, cant_deg: 12.0 };
        let target = post(base.clone());
        let g = Gizmo::on_ring(&d, Some(&b), &base, 1.95).unwrap();
        for (k, axis) in [Axis::Spin, Axis::Tilt, Axis::Cant].into_iter().enumerate() {
            let n = g.rings[k].1;
            let (u, v) = plane_basis(n);
            // Ray straight down the ring's axis onto its plane, from 30° round to 120°.
            let on = |deg: f64| {
                let (s, c) = deg.to_radians().sin_cos();
                let p: [f64; 3] = std::array::from_fn(|i| g.origin[i] + 2.0 * (u[i] * c + v[i] * s));
                Ray { origin: add(p, n, 30.0), direction: n.map(|x| -x) }
            };
            let mut c = RotateCmd::of(&target, 9).about(g.pivot());
            c.feed(&StepInput::Lock(axis));
            c.feed(&g.token(Handle::Turn(axis), on(30.0), None).unwrap());
            c.feed(&g.token(Handle::Turn(axis), on(120.0), None).unwrap());
            assert!((c.dimensions()[k].value - 90.0).abs() < 1e-9, "{axis:?}: {:?}", c.dimensions());
            let after = placement_of(c.feed(&StepInput::Confirm));
            let (before, now) = (axes(&d, &b, &base), axes(&d, &b, &after));
            for i in 0..3 {
                assert!(close3(now[i], turned(before[i], n, 90.0), 1e-9), "{axis:?} axis {i}: {:?} vs {:?}", now[i], turned(before[i], n, 90.0));
            }
        }
    }

    #[test]
    fn a_free_part_moves_along_world_axes_and_turns_about_its_own_centre() {
        let part = Feature { id: 4, name: "Stud".into(), enabled: true, operation: Operation::Sphere { radius_mm: 1.0 }, component: Component::default() };
        let centre = [0.0, 12.0, 1.0];
        let g = Gizmo::free(centre, 1.0);
        let cam = Camera::new([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], centre);
        let layout = cam.layout(&g);
        assert!(layout.anchor(Handle::Move(Axis::Z)).is_none() && layout.anchor(Handle::Turn(Axis::X)).is_none(), "{:?}", layout.handles().collect::<Vec<_>>());
        let press = layout.anchor(Handle::Move(Axis::X)).unwrap();
        let mut c = MoveCmd::of(&part, 9);
        c.feed(&StepInput::Lock(Axis::X));
        c.feed(&g.token(Handle::Move(Axis::X), cam.ray(press), None).unwrap());
        c.feed(&g.token(Handle::Move(Axis::X), cam.ray(press + egui::vec2(30.0, 17.0)), None).unwrap());
        let feature = added(c.feed(&StepInput::Confirm));
        assert!(matches!(feature.operation, Operation::Transform { source: 4, translation, .. } if close3(translation, [1.5, 0.0, 0.0], 1e-5)), "{:?}", feature.operation);
        // A quarter turn about z through the centre leaves the centre where it was.
        let press = layout.anchor(Handle::Turn(Axis::Z)).unwrap();
        let o = cam.project(centre);
        let quarter = o + (press - o).rot90();
        let mut c = RotateCmd::of(&part, 9).about(g.pivot());
        c.feed(&StepInput::Lock(Axis::Z));
        c.feed(&g.token(Handle::Turn(Axis::Z), cam.ray(press), None).unwrap());
        c.feed(&g.token(Handle::Turn(Axis::Z), cam.ray(quarter), None).unwrap());
        let feature = added(c.feed(&StepInput::Confirm));
        let Operation::Transform { translation, rotation_deg, .. } = feature.operation else { panic!() };
        assert!((rotation_deg[2].abs() - 90.0).abs() < 1e-6 && rotation_deg[0] == 0.0 && rotation_deg[1] == 0.0, "{rotation_deg:?}");
        let moved = crate::command::ring::transform(translation, rotation_deg).apply(centre);
        assert!(close3(moved, centre, 1e-9), "the centre stays put: {moved:?}");
    }

    #[test]
    fn the_dial_slides_the_part_round_the_shank_on_the_five_degree_grid_and_keeps_the_rest() {
        let d = court();
        let b = band(&d);
        let base = Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 0.25, spin_deg: 15.0, tilt_deg: 2.0, cant_deg: -3.0 };
        let g = Gizmo::on_ring(&d, Some(&b), &base, 1.95).unwrap();
        let dial = g.dial.unwrap();
        let cam = Camera::new([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]);
        let at = |deg: f64| cam.ray(cam.project(dial.point(deg)));
        assert!((dial.theta_at(at(62.4), None).unwrap() - 62.4).abs() < 1e-3);
        assert_eq!(dial.theta_at(at(62.4), Some(5.0)), Some(60.0));
        assert_eq!(dial.theta_at(at(358.0), Some(5.0)), Some(0.0), "the grid wraps");
        let layout = cam.layout(&g);
        let marker = layout.anchor(Handle::Dial).unwrap();
        assert!(marker.distance(cam.project(dial.point(90.0))) < 1e-3);
        assert_eq!(layout.hit(marker), Some(Handle::Dial));
        let dial_marks = layout.dial.as_ref().unwrap();
        assert_eq!((dial_marks.ticks.len(), dial_marks.labels.len()), (24, 8));
        assert_eq!(dial_marks.labels[2].1, "90°");
        let mut c = PlaceCmd::new(2, base.clone());
        c.feed(&StepInput::Lock(Axis::Theta));
        c.feed(&g.token(Handle::Dial, cam.ray(marker), Some(5.0)).unwrap());
        c.feed(&g.token(Handle::Dial, at(61.3), Some(5.0)).unwrap());
        let p = placement_of(c.feed(&StepInput::Confirm));
        assert_eq!(p, Placement::Ring { theta_deg: 60.0, across_mm: 0.0, height_mm: 0.25, spin_deg: 15.0, tilt_deg: 2.0, cant_deg: -3.0 });
        // Seen edge-on the dial has no angle to give and is not drawn.
        let edge = Camera::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0], g.origin);
        assert!(edge.layout(&g).dial.is_none() && edge.layout(&g).anchor(Handle::Dial).is_none());
        assert!(dial.theta_at(edge.ray(edge.project(g.origin)), Some(5.0)).is_none());
    }

    #[test]
    fn a_grip_drag_sizes_the_seated_part_along_its_own_line() {
        let d = court();
        let b = band(&d);
        let target = post(Placement::ring(90.0, 0.25));
        let frame = seat(&d, Some(&b), &target.component.placement).unwrap();
        let g = Gizmo::on_ring(&d, Some(&b), &target.component.placement, 1.95).unwrap().with_grips(&target.operation);
        assert_eq!(g.frame, frame, "the part's frame is the placed seat");
        assert_eq!(g.grips.iter().map(|p| p.grip.key).collect::<Vec<_>>(), ["radius", "height"]);
        // The radius grip stands on the part's x, which on the ring runs down the finger.
        assert!(close3(g.grips[0].direction, [0.0, 0.0, -1.0], 0.02) && close3(g.grips[0].at, add(g.origin, g.grips[0].direction, 1.5), 1e-12));
        let cam = Camera::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0], g.origin);
        let layout = cam.layout(&g);
        assert!(layout.anchor(Handle::Grip(1)).is_none(), "the height grip points at the eye");
        let press = layout.anchor(Handle::Grip(0)).unwrap();
        assert_eq!(layout.hit(press), Some(Handle::Grip(0)), "a grip wins at its own dot");
        assert_eq!(g.label(Handle::Grip(0)), "Grip: Radius");
        let mut s = Session::default();
        s.start(Box::new(GripCmd::new(2, target.operation.clone(), "radius", &frame).unwrap()));
        s.feed(g.token(Handle::Grip(0), cam.ray(press), None).unwrap());
        s.feed(g.token(Handle::Grip(0), cam.ray(press + egui::vec2(4.0, 10.0)), None).unwrap());
        assert!((s.dimensions()[0].value - 2.0).abs() < 1e-3, "ten pixels down the screen is half a millimetre more: {:?}", s.dimensions());
        assert!(s.preview().unwrap().caption.starts_with("Radius 2.00 mm"));
        s.feed(g.token(Handle::Grip(0), cam.ray(press + egui::vec2(0.0, -200.0)), None).unwrap());
        assert_eq!(s.dimensions()[0].value, grips::MIN_SIZE_MM, "a size never drags through zero");
        assert!(matches!(s.feed(StepInput::Typed { key: "radius", value: -1.0 }), Outcome::Refused(_)));
        s.feed(StepInput::Typed { key: "radius", value: 1.2 });
        assert!(matches!(s.feed(StepInput::Lock(Axis::X)), Outcome::Refused(_)));
        let e = commit(s.enter());
        assert!(matches!(e.as_slice(), [crate::command::Effect::Operation { feature: 2, operation: Operation::Cylinder { radius_mm, height_mm } }] if *radius_mm == 1.2 && *height_mm == 2.5), "{e:?}");
        assert!(GripCmd::new(2, Operation::Band, "radius", &frame).is_none());
    }

    #[test]
    fn handles_keep_a_screen_size_and_stand_clear_of_a_part_bigger_than_it() {
        let d = court();
        let b = band(&d);
        let small = Gizmo::on_ring(&d, Some(&b), &Placement::ring(90.0, 0.25), 0.2).unwrap();
        let cam = Camera::new([0.0, -1.0, 0.0], [0.0, 0.0, 1.0], small.origin);
        let o = cam.project(small.origin);
        let arrow = |l: &Layout, a: Axis| match l.marks.iter().find(|(h, _)| *h == Handle::Move(a)).map(|m| m.1.clone()) {
            Some(Mark::Arrow { base, tip }) => (o.distance(base), base.distance(tip)),
            other => panic!("{other:?}"),
        };
        let ring = |l: &Layout, a: Axis| match l.marks.iter().find(|(h, _)| *h == Handle::Turn(a)).map(|m| m.1.clone()) {
            Some(Mark::Ring { points, .. }) => points.iter().map(|p| o.distance(*p)).fold(0.0f32, f32::max),
            other => panic!("{other:?}"),
        };
        let l = cam.layout(&small);
        let (base, len) = arrow(&l, Axis::Theta);
        assert!((base - GAP_PX).abs() < 1e-3 && (len - ARROW_PX).abs() < 1e-3, "{base} {len}");
        assert!((ring(&l, Axis::Spin) - RING_PX).abs() < 0.01);
        // A part 5 mm across its reach stands 100 px out on screen: the handles clear it.
        let big = Gizmo { reach_mm: 5.0, ..small.clone() };
        let l = cam.layout(&big);
        let (base, len) = arrow(&l, Axis::Across);
        assert!((base - (100.0 + CLEAR_PX)).abs() < 1e-3 && (len - ARROW_PX).abs() < 1e-3, "{base} {len}");
        assert!((ring(&l, Axis::Spin) - (100.0 + CLEAR_PX)).abs() < 0.01);
        // Only the spin ring faces a camera looking down the normal; the other two are edge-on.
        let turns: Vec<_> = l.marks.iter().filter_map(|(h, _)| if let Handle::Turn(a) = h { Some(*a) } else { None }).collect();
        assert_eq!(turns, [Axis::Spin]);
        // Every handle drawn can be taken at its own anchor.
        for h in l.handles() {
            assert_eq!(l.anchor(h).and_then(|at| l.hit(at)), Some(h), "{h:?}");
        }
        assert_eq!(l.hit(o), None, "the part itself is not a handle");
    }
}
