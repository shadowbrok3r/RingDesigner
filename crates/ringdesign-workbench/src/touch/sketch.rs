//! Sketching by touch: the plane a sketch lies on read off the ring as built, the view square to it, a finger's taps and drags read as the sketch tools' tokens, and the edits a finished sketch and the solid made from it leave as.
use super::hit::FINGER_PT;
use super::parts::fresh_ids;
use crate::command::{Dimension, Unit, along_line, on_plane};
use crate::focus::{Pose, view_axes};
use crate::sketch_tools::{self, Escaped, Input, Outcome, Snap, SnapCache, SnapKind, Tool, Tools, Underlay};
use ringdesign_core::{
    BuildResult, Mesh, RingDesign,
    cad::{self, Attach, Component, ComponentRole, FaceRef, Feature, Operation, Profile, edit::CadEdit},
    interaction::pick::Ray,
    sketch::{FaceAnchor, Id, Region, RegionRef, Sketch, Workplane, anchor, fill},
};

/// Grid lines stand at least this far apart on screen, points.
pub const GRID_PT: f64 = 12.0;
/// A dragged height moves in steps of this, mm.
pub const HEIGHT_STEP_MM: f64 = 0.05;
/// A dragged angle moves in steps of this, degrees.
pub const ANGLE_STEP_DEG: f64 = 5.0;
/// The tallest extrusion a finger or a number asks for, mm.
pub const MAX_HEIGHT_MM: f64 = 50.0;
/// How far the extrusion's view tilts off the plane's normal, degrees.
const EXTRUDE_TILT_DEG: f64 = 55.0;
/// The least of an open plane a view frames round its centre, mm.
const MIN_REACH_MM: f64 = 3.0;
/// The least of a face a view frames round its centre, mm: a small face fills the view for a finger.
const MIN_FACE_REACH_MM: f64 = 1.0;
/// Lines and axes take this share of a finger's reach, so a tap between them lands on the grid.
pub const LINE_SHARE: f64 = 0.4;
/// Why a ring of parts alone takes no cut.
const ALL_PARTS: &str = "The ring is all parts: a cut has no band to carve";

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| v.map(|c| c / l))
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A sketch's plane in the world: its origin, unit axes, and the normal `x × y` out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub n: [f64; 3],
}

impl Frame {
    pub fn new(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Option<Self> {
        let n = unit(cross(x, y))?;
        Some(Self { origin, x, y, n })
    }
    /// The world point at plane coordinates `uv`.
    pub fn point(&self, uv: [f64; 2]) -> [f64; 3] {
        std::array::from_fn(|k| self.origin[k] + self.x[k] * uv[0] + self.y[k] * uv[1])
    }
    /// The world direction of plane vector `uv`.
    pub fn vector(&self, uv: [f64; 2]) -> [f64; 3] {
        std::array::from_fn(|k| self.x[k] * uv[0] + self.y[k] * uv[1])
    }
    /// A world point in plane coordinates, projected along the normal.
    pub fn local(&self, p: [f64; 3]) -> [f64; 2] {
        let d = sub(p, self.origin);
        [dot(d, self.x), dot(d, self.y)]
    }
    /// Where a ray meets the plane, in plane coordinates; `None` for a ray running along it.
    pub fn hit(&self, ray: Ray) -> Option<[f64; 2]> {
        let along = dot(ray.direction, self.n);
        if along.abs() < 1e-9 {
            return None;
        }
        let t = dot(sub(self.origin, ray.origin), self.n) / along;
        Some(self.local(std::array::from_fn(|k| ray.origin[k] + ray.direction[k] * t)))
    }
    fn workplane(&self) -> Workplane {
        Workplane { origin: self.origin, x: self.x, y: self.y, on_face: None }
    }
}

/// Where a new sketch lies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// A planar face of a part; the sketch moves with it.
    Face { feature: Id, face: u32 },
    /// A work plane feature.
    Plane(Id),
    /// Square to the band where it was pressed: x round the ring, y along the finger, the normal out.
    Tangent { theta_deg: f64, across_mm: f64 },
    /// Through the finger's axis at the angle it was pressed: x out from the axis, y along the finger.
    Section { theta_deg: f64, across_mm: f64 },
}

/// The plane square to the band at `hit`: x round the ring, y along the finger, the normal out.
fn tangent_plane(hit: [f64; 3], normal: [f64; 3], theta_deg: f64) -> Option<Workplane> {
    let n = unit(normal)?;
    let (s, c) = theta_deg.to_radians().sin_cos();
    let round = [-s, c, 0.0];
    let x = unit(std::array::from_fn(|k| round[k] - n[k] * dot(round, n)))?;
    Some(Workplane { origin: hit, x, y: cross(n, x), on_face: None })
}

/// The plane through the finger's axis at `theta_deg`: x out from the axis, y along the finger.
fn section_plane(theta_deg: f64) -> Workplane {
    let (s, c) = theta_deg.to_radians().sin_cos();
    Workplane { origin: [0.0; 3], x: [c, s, 0.0], y: [0.0, 0.0, 1.0], on_face: None }
}

/// An empty sketch lying on `place` in the ring as `built`, and where on its plane the view centres; refused in the core's words.
pub fn start(built: &BuildResult, place: Place) -> Result<(Sketch, [f64; 2]), String> {
    let mut sketch = Sketch { name: "Sketch".into(), ..Sketch::default() };
    let centre = match place {
        Place::Face { feature, face } => {
            let c = built.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == feature)).ok_or_else(|| format!("Feature #{feature} is not a part on the ring to sketch on"))?;
            let body = c.brep().ok_or_else(|| format!("{} is built as a mesh; a sketch lies on a kernel part's planar face", c.name))?;
            sketch.plane.on_face = Some(anchor::on_face(&c.frame, feature, body, face as usize).map_err(|e| format!("{e:#}"))?);
            [0.0; 2]
        }
        Place::Plane(id) => {
            built.parts.evaluated.as_ref().and_then(|e| e.plane(id)).ok_or_else(|| format!("Work plane #{id} is not in the ring as built yet"))?;
            sketch.plane.on_face = Some(FaceAnchor { feature: id, face: FaceRef::bare(0) });
            [0.0; 2]
        }
        Place::Tangent { theta_deg, across_mm } => {
            let (hit, normal) = cad::surface_hit(&built.mesh, theta_deg, across_mm).ok_or("No band under that point to sketch on")?;
            sketch.plane = tangent_plane(hit, normal, theta_deg).ok_or("The band has no surface direction there to sketch on")?;
            [0.0; 2]
        }
        Place::Section { theta_deg, across_mm } => {
            sketch.plane = section_plane(theta_deg);
            let (hit, _) = cad::surface_hit(&built.mesh, theta_deg, across_mm).ok_or("No band under that point to sketch on")?;
            [dot(hit, sketch.plane.x), hit[2]]
        }
    };
    Ok((sketch, centre))
}

/// The world plane `sketch` lies on and the loops of the face under it, read off the ring as `built`.
pub fn resolve(sketch: &Sketch, built: &BuildResult) -> Result<(Frame, Vec<Vec<[f64; 3]>>), String> {
    let Some(anchor) = &sketch.plane.on_face else {
        let p = sketch.plane.plane().map_err(|e| format!("{e:#}"))?;
        return Frame::new(p.origin, p.x_axis, p.y_axis).map(|f| (f, Vec::new())).ok_or_else(|| "The sketch's plane has no normal".to_string());
    };
    let e = built.parts.evaluated.as_ref().ok_or("The ring has no parts built to sketch on")?;
    if let Some(plane) = e.plane(anchor.feature) {
        let p = cad::pattern::sketch_on_work_plane(sketch, plane).map_err(|e| format!("{e:#}"))?;
        return Frame::new(p.origin, p.x_axis, p.y_axis).map(|f| (f, Vec::new())).ok_or_else(|| "The work plane has no normal".to_string());
    }
    let c = e.components.iter().find(|c| c.id == anchor.feature).ok_or_else(|| format!("Sketch face: feature #{} is not a part on the ring", anchor.feature))?;
    let p = anchor::plane(sketch, &c.body, &c.frame, &mut Vec::new()).map_err(|e| format!("{e:#}"))?;
    let face = fill::face_outline(&c.body, &p, 0.01).unwrap_or_default();
    let frame = Frame::new(p.origin, p.x_axis, p.y_axis).ok_or("The face's plane has no normal")?;
    Ok((frame, face))
}

/// Whether `p` lies inside the closed polygon `l`.
fn inside(l: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut odd = false;
    for i in 0..l.len() {
        let (a, b) = (l[i], l[(i + 1) % l.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            odd = !odd;
        }
    }
    odd
}

/// What a sketch on `frame` is drawn over: the face's loops and where the plane cuts `mesh`, in plane coordinates.
pub fn underlay(frame: &Frame, face: &[Vec<[f64; 3]>], mesh: &Mesh) -> Underlay {
    let face: Vec<Vec<[f64; 2]>> = face.iter().map(|l| l.iter().map(|p| frame.local(*p)).collect()).collect();
    // The face's own triangles lie in the plane; their edges are no underlay and nothing to snap to.
    let on_face = |p: [f64; 2]| face.iter().filter(|l| inside(l, p)).count() % 2 == 1;
    let cut = frame.workplane().plane().map(|p| fill::slice(mesh, &p)).unwrap_or_default();
    let cut = cut.into_iter().filter(|s| !on_face([(s[0][0] + s[1][0]) * 0.5, (s[0][1] + s[1][1]) * 0.5])).collect();
    Underlay { face, cut }
}

/// The pose whose eye stands out along `eye` from `centre`, `up` as near screen up as it stands, `centre` mid-view and `reach_mm` round it framed; the camera orbits `target` and frames `framed_mm` at zoom 1.
pub fn look_along(current: Pose, target: [f32; 3], framed_mm: f32, eye: [f64; 3], up: [f64; 3], centre: [f64; 3], reach_mm: f64) -> Pose {
    let e = unit(eye).unwrap_or([0.0, 0.0, 1.0]).map(|v| v as f32);
    let pitch = e[2].clamp(-1.0, 1.0).asin();
    let yaw = if e[0].hypot(e[1]) > 1e-6 { e[1].atan2(e[0]) } else { current.yaw };
    let d3 = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (s0, u0) = view_axes(yaw, pitch, 0.0);
    let up = up.map(|v| v as f32);
    let roll = (-d3(up, s0)).atan2(d3(up, u0));
    let (s, u) = view_axes(yaw, pitch, roll);
    let off: [f32; 3] = std::array::from_fn(|k| centre[k] as f32 - target[k]);
    let zoom = (framed_mm / (reach_mm.max(0.5) as f32 * 1.6)).clamp(0.15, 24.0);
    Pose { yaw, pitch, roll, zoom, pan: [d3(off, s), d3(off, u)] }
}

/// The grid a finger snaps to at `px_per_mm` points a millimetre: the sketch's own, doubled until its lines stand `GRID_PT` apart.
pub fn grid_mm(s: &Sketch, px_per_mm: f64) -> f64 {
    let mut g = if s.grid_mm.is_finite() && s.grid_mm > 0.0 { s.grid_mm } else { 0.5 };
    for _ in 0..40 {
        if g * px_per_mm >= GRID_PT {
            break;
        }
        g *= 2.0;
    }
    g
}

/// How far a finger's pick reaches on the plane at `px_per_mm` points a millimetre, mm.
pub fn reach_mm(px_per_mm: f64) -> f64 {
    f64::from(FINGER_PT) / px_per_mm.max(1e-6)
}

/// What finishing makes of the sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Make {
    Extrude,
    Revolve,
}

/// Where a sketch session is.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    Draw,
    /// Finished: its regions found; Extrude, Revolve or the sketch alone next, a tap picking one region of several.
    Offer,
    /// An extrusion set by a drag along the normal or a typed height.
    Extrude,
    /// A revolution waiting for its axis.
    Axis,
    /// A revolution about the line through `pivot` along `dir` in the plane, called `axis`.
    Revolve { pivot: [f64; 2], dir: [f64; 2], axis: String },
}

/// A solid's number: dragged, or typed and then held against drags.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Amount {
    value: f64,
    typed: bool,
}

impl Amount {
    fn new(value: f64) -> Self {
        Self { value, typed: false }
    }
    fn dimension(&self, key: &'static str, label: &'static str, unit: Unit) -> Dimension {
        Dimension { key, label, unit, value: self.value, locked: self.typed }
    }
}

/// A finger's hold on the plane.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Held {
    /// A point dragged by hand, the solver keeping its constraints.
    Point(Id),
    /// The tool's pointer, clicking where the finger lifts.
    Aim,
}

/// Where the view stands for a stage: its eye out along `eye`, `up` up the screen, `centre` mid-view and `reach_mm` round it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub eye: [f64; 3],
    pub up: [f64; 3],
    pub centre: [f64; 3],
    pub reach_mm: f64,
}

/// Phrases a desktop prompt or refusal speaks that a finger does differently.
const TOUCH_WORDS: [(&str, &str); 10] = [
    ("Shift adds, Delete removes", "tap again to let it go, drag a point to move it"),
    ("the first point or C closes, Escape ends it open", "the first point or Close loop closes it, Back ends it open"),
    ("move out or in", "drag out or in"),
    ("Move off the loop", "Tap or drag off the loop"),
    ("then press Enter", "then Done"),
    ("Enter or a click", "Done or a tap"),
    ("click or Enter", "a tap or Done"),
    ("Enter holds it", "Done holds it"),
    ("click", "tap"),
    ("Click", "Tap"),
];

/// `words` as a finger does what they ask.
fn touch_words(words: String) -> String {
    TOUCH_WORDS.iter().fold(words, |w, (desk, touch)| w.replace(desk, touch))
}

/// A tool's outcome with a refusal said in a finger's words.
fn by_finger(out: Outcome) -> Outcome {
    match out {
        Outcome::Refused(why) => Outcome::Refused(touch_words(why)),
        other => other,
    }
}

/// A sketch drawn by touch.
pub struct Pad {
    /// The Sketch feature it is drawn into; `None` until a commit adds it.
    pub feature: Option<Id>,
    /// Where a new sketch was laid, which the band's two planes switch between.
    pub place: Option<Place>,
    /// The sketch as the document holds it, and as it is drawn.
    pub base: Sketch,
    pub working: Sketch,
    pub tools: Tools,
    /// Taps delete what they land on.
    pub erase: bool,
    pub stage: Stage,
    /// Where on the plane the view centres.
    pub centre: [f64; 2],
    /// The region the solid sweeps alone; `None` sweeps every region.
    pub pick: Option<RegionRef>,
    /// How the solid meets the ring: joined, cut into the metal, or standing apart.
    pub attach: Attach,
    /// The finger's last place on the plane and what it settled on.
    pub pointer: Option<([f64; 2], Option<Snap>)>,
    frame: Option<Frame>,
    face: Vec<Vec<[f64; 3]>>,
    under: Underlay,
    error: Option<String>,
    snaps: Option<(u64, SnapCache)>,
    regions: Option<(u64, Result<Vec<Region>, String>)>,
    /// Why every region together would not sweep as one solid, when it would not.
    whole: Option<(u64, Option<String>)>,
    height: Amount,
    draft: Amount,
    degrees: Amount,
    /// How far the extrusion's top stood from the finger along the normal when it took the arrow, mm.
    grip: Option<f64>,
    held: Option<Held>,
    bumps: u64,
}

impl Pad {
    /// A session on `sketch`, drawn into feature `feature` when the document already holds it.
    pub fn new(feature: Option<Id>, sketch: Sketch, centre: [f64; 2], place: Option<Place>) -> Self {
        Self {
            feature,
            place,
            base: sketch.clone(),
            working: sketch,
            tools: Tools::default(),
            erase: false,
            stage: Stage::Draw,
            centre,
            pick: None,
            attach: Attach::Join,
            pointer: None,
            frame: None,
            face: Vec::new(),
            under: Underlay::default(),
            error: None,
            snaps: None,
            regions: None,
            whole: None,
            height: Amount::new(1.0),
            draft: Amount::new(0.0),
            degrees: Amount::new(360.0),
            grip: None,
            held: None,
            bumps: 0,
        }
    }

    /// Counts every change to what is drawn.
    pub fn version(&self) -> u64 {
        self.tools.version() + self.bumps
    }

    /// Whether the drawing differs from what the document holds.
    pub fn dirty(&self) -> bool {
        match self.feature {
            None => !self.working.entities.is_empty(),
            Some(_) => self.working != self.base,
        }
    }

    /// Reads the plane and what it is drawn over off `built`; the core's words when it cannot.
    pub fn read(&mut self, built: &BuildResult) {
        match resolve(&self.working, built) {
            Ok((frame, face)) => {
                self.under = underlay(&frame, &face, &built.mesh);
                self.face = face;
                self.frame = Some(frame);
                self.error = None;
                self.snaps = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// The loops of the face the sketch lies on, in the world.
    pub fn face(&self) -> &[Vec<[f64; 3]>] {
        &self.face
    }

    pub fn underlay(&self) -> &Underlay {
        &self.under
    }

    /// Why the plane could not be read.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The regions the solved sketch bounds, or why it bounds none.
    pub fn regions(&mut self) -> Result<&[Region], &str> {
        let v = self.version();
        if self.regions.as_ref().is_none_or(|(k, _)| *k != v) {
            self.regions = Some((v, self.working.profile_regions().map_err(|e| format!("{e:#}"))));
        }
        match &self.regions.as_ref().expect("filled above").1 {
            Ok(r) => Ok(r.as_slice()),
            Err(e) => Err(e.as_str()),
        }
    }

    /// Why every region at once would not sweep as one solid, as where the curves between them branch; `None` when it would.
    pub fn apart(&mut self) -> Option<String> {
        let v = self.version();
        if self.whole.as_ref().is_none_or(|(k, _)| *k != v) {
            self.whole = Some((v, self.working.sweep_regions().err().map(|e| format!("{e:#}"))));
        }
        self.whole.as_ref().expect("filled above").1.clone()
    }

    /// The regions a solid sweeps: the one picked, else every one.
    pub fn swept(&mut self) -> Vec<Region> {
        let pick = self.pick;
        let Ok(regions) = self.regions() else { return Vec::new() };
        match pick.and_then(|p| p.position(regions)) {
            Some((i, _)) => vec![regions[i].clone()],
            None => regions.to_vec(),
        }
    }

    /// A point well inside the biggest region the solid sweeps, in plane coordinates.
    fn inside(&mut self) -> Option<[f64; 2]> {
        let swept = self.swept();
        swept.iter().max_by(|a, b| a.area().total_cmp(&b.area()))?.inside()
    }

    /// Where the solid being set stands out of the plane: a point well inside the biggest region it sweeps.
    pub fn anchor(&mut self) -> Option<[f64; 3]> {
        let frame = self.frame?;
        Some(frame.point(self.inside()?))
    }

    /// Where a finger at `xy` settles at `px_per_mm`: a point within its reach, a line or axis within a narrower band, else the grid; a dragged point snaps only to what it cannot move.
    pub fn snap(&mut self, xy: [f64; 2], px_per_mm: f64) -> Option<Snap> {
        let reach = reach_mm(px_per_mm);
        let grid = Some(grid_mm(&self.working, px_per_mm));
        let held = matches!(self.held, Some(Held::Point(_)));
        let v = self.version();
        if !held && self.snaps.as_ref().is_none_or(|(k, _)| *k != v) {
            self.snaps = Some((v, SnapCache::of(&self.working, &self.under)));
        }
        let still = held.then(|| SnapCache { corners: self.under.corners(), ..SnapCache::default() });
        let blank = Sketch::default();
        let (sketch, cache) = match &still {
            Some(still) => (&blank, still),
            None => (&self.working, &self.snaps.as_ref().expect("filled above").1),
        };
        let point = |s: &Snap| matches!(s.kind, SnapKind::Endpoint | SnapKind::Centre | SnapKind::Intersection | SnapKind::Corner | SnapKind::Origin | SnapKind::Midpoint);
        let on_grid = grid.map(|g| xy.map(|v| (v / g).round() * g)).filter(|p| ringdesign_core::sketch::distance(*p, xy) <= reach).map(|xy| Snap { xy, kind: SnapKind::Grid });
        sketch_tools::snap(sketch, &self.under, cache, xy, reach, None)
            .filter(point)
            .or_else(|| sketch_tools::snap(sketch, &self.under, cache, xy, reach * LINE_SHARE, None))
            .or(on_grid)
    }

    /// Feeds the finger at `xy` to the tool as its pointer.
    fn aim(&mut self, xy: [f64; 2], px_per_mm: f64) {
        let snapped = self.snap(xy, px_per_mm);
        self.pointer = Some((xy, snapped));
        self.tools.feed(&mut self.working, Input::Pointer { raw: xy, snapped, reach: reach_mm(px_per_mm) });
    }

    /// Picks a drawing tool; any step the last one was in is dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        self.erase = false;
        self.held = None;
        self.tools.set_tool(tool);
    }

    /// Turns Erase on: a tap deletes the curve or point under it.
    pub fn set_erase(&mut self) {
        self.set_tool(Tool::Select);
        self.erase = true;
    }

    /// A tap at plane point `xy` at `px_per_mm`: Erase deletes what it lands on, a finished sketch picks a region or an axis, else the tool clicks there.
    pub fn tap(&mut self, xy: [f64; 2], px_per_mm: f64) -> Outcome {
        self.held = None;
        match self.stage {
            Stage::Draw => {}
            Stage::Offer | Stage::Extrude => return self.pick_region(xy),
            Stage::Axis => return self.pick_axis(xy, px_per_mm),
            Stage::Revolve { .. } => return Outcome::Continue,
        }
        let reach = reach_mm(px_per_mm);
        if self.erase {
            return self.erase_at(xy, reach);
        }
        self.aim(xy, px_per_mm);
        // In Select a tap on something toggles it and a tap on nothing lets everything go.
        let add = self.tools.tool == Tool::Select && sketch_tools::pick(&self.working, xy, reach).is_some();
        by_finger(self.tools.feed(&mut self.working, Input::Click { add }))
    }

    /// Deletes the curve under `xy`, else the point, as one undo step.
    fn erase_at(&mut self, xy: [f64; 2], reach: f64) -> Outcome {
        let (points, entities) = match self.working.nearest_entity(xy, reach) {
            Some(near) => (Vec::new(), vec![near.entity]),
            None => match self.working.nearest_point(xy, reach) {
                Some(p) => (vec![p], Vec::new()),
                None => return Outcome::Refused("Nothing under the finger to delete".into()),
            },
        };
        self.tools.chosen_points = points.into_iter().collect();
        self.tools.chosen_entities = entities.into_iter().collect();
        self.tools.delete_chosen(&mut self.working)
    }

    /// A finger down at `from` has moved to `at`: in Select it takes the point there, a drawing tool starts its shape where it came down and a busy tool aims; false when it takes nothing, and the view pans instead.
    pub fn drag_start(&mut self, from: [f64; 2], at: [f64; 2], px_per_mm: f64) -> bool {
        self.held = None;
        if self.stage != Stage::Draw || self.erase {
            return false;
        }
        let reach = reach_mm(px_per_mm);
        if self.tools.tool == Tool::Select && !self.tools.busy() {
            let Some(p) = self.working.nearest_point(from, reach).filter(|p| self.tools.begin_drag(&self.working, *p)) else { return false };
            self.held = Some(Held::Point(p));
            self.drag_move(at, px_per_mm);
            return true;
        }
        let drawing = matches!(self.tools.tool, Tool::Line | Tool::Rectangle | Tool::Circle | Tool::Arc);
        if !self.tools.busy() {
            if !drawing {
                return false;
            }
            // The shape starts where the finger came down.
            self.aim(from, px_per_mm);
            self.tools.feed(&mut self.working, Input::Click { add: false });
        }
        self.held = Some(Held::Aim);
        self.drag_move(at, px_per_mm);
        true
    }

    /// The dragging finger moved to `at`.
    pub fn drag_move(&mut self, at: [f64; 2], px_per_mm: f64) {
        match self.held {
            Some(Held::Point(p)) => {
                let snapped = self.snap(at, px_per_mm);
                self.pointer = Some((at, snapped));
                self.tools.drag_to(&mut self.working, p, snapped.map_or(at, |s| s.xy));
            }
            Some(Held::Aim) => self.aim(at, px_per_mm),
            None => {}
        }
    }

    /// The dragging finger lifted at `at`: a moved point is one undo step, an aim clicks there.
    pub fn drag_end(&mut self, at: [f64; 2], px_per_mm: f64) -> Outcome {
        match self.held {
            Some(Held::Point(_)) => {
                self.drag_move(at, px_per_mm);
                self.held = None;
                self.tools.end_drag(&self.working);
                Outcome::Edited("Point moved".into())
            }
            Some(Held::Aim) => {
                self.aim(at, px_per_mm);
                self.held = None;
                by_finger(self.tools.feed(&mut self.working, Input::Click { add: false }))
            }
            None => Outcome::Continue,
        }
    }

    /// A second finger or the system took the touch: a dragged point stays where it was taken, an aim waits for the next finger.
    pub fn let_go(&mut self) {
        if matches!(self.held, Some(Held::Point(_))) {
            self.tools.end_drag(&self.working);
        }
        self.held = None;
    }

    /// Whether a finger holds a point or aims a tool.
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// Closes the line being drawn back to its first point.
    pub fn close(&mut self) -> Outcome {
        by_finger(self.tools.feed(&mut self.working, Input::Close))
    }

    pub fn undo(&mut self) -> bool {
        self.held = None;
        self.tools.undo(&mut self.working)
    }

    pub fn redo(&mut self) -> bool {
        self.held = None;
        self.tools.redo(&mut self.working)
    }

    /// A number typed into the field `key`: the solid's own while one is set, else the tool's.
    pub fn typed(&mut self, key: &'static str, value: f64) -> Outcome {
        let amount = match (&self.stage, key) {
            (Stage::Extrude, "height") if value > 0.0 && value <= MAX_HEIGHT_MM => &mut self.height,
            (Stage::Extrude, "height") if self.cutting() => return Outcome::Refused(format!("A cut goes more than 0 and at most {MAX_HEIGHT_MM} mm deep")),
            (Stage::Extrude, "height") => return Outcome::Refused(format!("An extrusion stands more than 0 and at most {MAX_HEIGHT_MM} mm high")),
            (Stage::Extrude, "draft") if value.abs() < 80.0 => &mut self.draft,
            (Stage::Extrude, "draft") => return Outcome::Refused("Draft must be below 80 degrees".into()),
            (Stage::Revolve { .. }, "angle") if value > 0.0 && value <= 360.0 => &mut self.degrees,
            (Stage::Revolve { .. }, "angle") => return Outcome::Refused("A revolution turns more than 0 and at most 360 degrees".into()),
            (Stage::Draw, _) => return self.tools.feed(&mut self.working, Input::Typed { key, value }),
            _ => return Outcome::Refused(format!("No number called {key} here")),
        };
        *amount = Amount { value, typed: true };
        Outcome::Continue
    }

    /// The field `key` emptied: a dragged amount takes over again.
    pub fn cleared(&mut self, key: &'static str) {
        match (&self.stage, key) {
            (Stage::Extrude, "height") => self.height.typed = false,
            (Stage::Extrude, "draft") => self.draft = Amount::new(0.0),
            (Stage::Revolve { .. }, "angle") => self.degrees.typed = false,
            (Stage::Draw, _) => {
                self.tools.feed(&mut self.working, Input::Cleared { key });
            }
            _ => {}
        }
    }

    /// Done while drawing: the tool's step made with its typed numbers.
    pub fn confirm(&mut self) -> Outcome {
        if self.stage != Stage::Draw {
            return Outcome::Continue;
        }
        by_finger(self.tools.feed(&mut self.working, Input::Confirm))
    }

    /// The numbers the stage in hand takes, for the dimension bar.
    pub fn dimensions(&self) -> Vec<Dimension> {
        match &self.stage {
            Stage::Draw => self.tools.dimensions(&self.working),
            Stage::Extrude => vec![self.height.dimension("height", if self.cutting() { "Depth" } else { "Height" }, Unit::Mm), self.draft.dimension("draft", "Draft", Unit::Deg)],
            Stage::Revolve { .. } => vec![self.degrees.dimension("angle", "Angle", Unit::Deg)],
            Stage::Offer | Stage::Axis => Vec::new(),
        }
    }

    /// Back one level: a typed number, the tool's step or its tool, the solid's step, the finish; `Out` once only the sketch itself is left.
    pub fn escape(&mut self) -> Escaped {
        self.held = None;
        let latest = [("height", self.height.typed), ("draft", self.draft.typed), ("angle", self.degrees.typed)];
        match self.stage.clone() {
            Stage::Draw if self.erase => {
                self.erase = false;
                Escaped::Tool
            }
            Stage::Draw => self.tools.escape(&self.working),
            Stage::Extrude | Stage::Revolve { .. } if latest.iter().any(|(_, t)| *t) => {
                self.height.typed = false;
                self.draft = Amount::new(0.0);
                self.degrees.typed = false;
                Escaped::Field
            }
            Stage::Revolve { .. } => {
                self.stage = Stage::Axis;
                Escaped::Step
            }
            Stage::Extrude | Stage::Axis => {
                self.stage = Stage::Offer;
                Escaped::Step
            }
            Stage::Offer => {
                self.stage = Stage::Draw;
                self.pick = None;
                Escaped::Step
            }
        }
    }

    /// Finish: any step in hand dropped, the sketch solved and its regions found; with none the sketch can still be kept alone.
    pub fn finish(&mut self) -> Outcome {
        self.held = None;
        self.erase = false;
        let tool = self.tools.tool;
        self.tools.set_tool(tool);
        if self.working.entities.is_empty() {
            return Outcome::Refused("Nothing is drawn yet: draw a closed shape, then Finish".into());
        }
        self.stage = Stage::Offer;
        self.pick = None;
        match self.regions() {
            Ok([]) => Outcome::Refused("No closed region yet: keep the sketch as it is, or go back and close a loop to make a solid".into()),
            Ok(r) => {
                let n = r.len();
                Outcome::Edited(if n == 1 { "One closed region: Extrude or Revolve it, or keep the sketch".into() } else { format!("{n} closed regions: tap one to make it alone, then Extrude or Revolve, or keep the sketch") })
            }
            Err(why) => Outcome::Refused(format!("{why}; keep the sketch as it is, or go back and mend it")),
        }
    }

    /// Starts making `make` from the regions: an extrusion at once, a revolution once its axis is tapped; regions sharing a curve are made one at a time.
    pub fn make(&mut self, make: Make) -> Outcome {
        if self.regions().map_or(true, |r| r.is_empty()) {
            return Outcome::Refused("A solid is made from closed regions; go back and close a loop".into());
        }
        if self.pick.is_none()
            && let Some(why) = self.apart()
        {
            return Outcome::Refused(format!("{why}; or tap one region to make it alone"));
        }
        self.stage = match make {
            Make::Extrude => Stage::Extrude,
            Make::Revolve => Stage::Axis,
        };
        Outcome::Continue
    }

    /// Picks the region under `xy` to sweep alone, or lets the pick go when it is the one picked or no region is there.
    fn pick_region(&mut self, xy: [f64; 2]) -> Outcome {
        let picked = self.pick;
        let Ok(regions) = self.regions() else { return Outcome::Continue };
        if regions.len() < 2 {
            return Outcome::Continue;
        }
        let under = regions.iter().position(|r| r.contains(xy));
        let same = under.is_some() && picked.and_then(|p| p.position(regions)).map(|(i, _)| i) == under;
        let next = under.filter(|_| !same).and_then(|i| RegionRef::among(regions, i, xy));
        let n = regions.len();
        // Regions sharing a curve are only ever made one at a time.
        if next.is_none() && self.stage != Stage::Offer && self.apart().is_some() {
            return Outcome::Refused("These regions share a curve: one is made at a time, so tap another to change it".into());
        }
        self.pick = next;
        Outcome::Edited(match next {
            Some(_) => "That region alone".into(),
            None => format!("Every region: all {n}"),
        })
    }

    /// Takes the axis a revolution turns about from a tap at `xy`: a line of the sketch there, else the plane's own axis nearest it.
    fn pick_axis(&mut self, xy: [f64; 2], px_per_mm: f64) -> Outcome {
        let reach = reach_mm(px_per_mm);
        let from_line = self.working.nearest_entity(xy, reach).and_then(|near| {
            let line: Vec<[f64; 2]> = self.working.polylines(near.entity, 0.01).into_iter().flatten().collect();
            let ([a, b], true) = ([line.first()?, line.last()?], line.len() == 2) else { return None };
            let d = [b[0] - a[0], b[1] - a[1]];
            let l = d[0].hypot(d[1]);
            (l > 1e-9).then(|| (*a, [d[0] / l, d[1] / l], format!("line #{}", near.entity)))
        });
        let (to_y, to_x) = (xy[0].abs(), xy[1].abs());
        let picked = from_line.or_else(|| {
            (to_y.min(to_x) <= reach * 3.0).then(|| if to_y <= to_x { ([0.0; 2], [0.0, 1.0], "the sketch's y axis".to_string()) } else { ([0.0; 2], [1.0, 0.0], "the sketch's x axis".to_string()) })
        });
        match picked {
            Some((pivot, dir, axis)) => {
                // A positive turn swings the region out of the plane toward its normal.
                let w = self.inside().map_or([0.0; 2], |p| [p[0] - pivot[0], p[1] - pivot[1]]);
                let dir = if dir[0] * w[1] - dir[1] * w[0] < 0.0 { [-dir[0], -dir[1]] } else { dir };
                let words = format!("Revolving about {axis}");
                self.stage = Stage::Revolve { pivot, dir, axis };
                Outcome::Edited(words)
            }
            None => Outcome::Refused("Tap a straight line of the sketch, or near one of its axes".into()),
        }
    }

    /// Chooses how the solid meets the ring; a cut is refused on a ring that is all parts, where there is no band to carve.
    pub fn set_attach(&mut self, design: &RingDesign, attach: Attach) -> Outcome {
        if attach == Attach::Cut && design.cad.as_ref().is_some_and(|d| d.replaces_band()) {
            return Outcome::Refused(ALL_PARTS.into());
        }
        self.attach = attach;
        self.grip = None;
        Outcome::Edited(
            match attach {
                Attach::Join => "Join: the solid is united with the band and what is joined to it",
                Attach::Cut => "Cut: the solid carves into the band and the part it stands on",
                Attach::Separate => "Separate: the solid stands apart as a casting of its own",
            }
            .into(),
        )
    }

    /// Whether the solid being set cuts into the metal.
    pub fn cutting(&self) -> bool {
        self.attach == Attach::Cut
    }

    /// The way an extrusion grows from the plane: out along its normal, against it into the metal for a cut.
    pub fn rise(&self) -> Option<[f64; 3]> {
        let n = self.frame?.n;
        Some(if self.cutting() { n.map(|v| -v) } else { n })
    }

    /// The line a revolution turns about in the world, signed so a positive turn swings a cut into the metal: its pivot and axis.
    pub fn revolution(&self) -> Option<([f64; 3], [f64; 3])> {
        let f = self.frame?;
        let Stage::Revolve { pivot, dir, .. } = &self.stage else { return None };
        let a = f.vector(*dir);
        Some((f.point(*pivot), if self.cutting() { a.map(|v| -v) } else { a }))
    }

    /// Reads the solid's number off a finger's ray: an extrusion's height moved as far along its rise as the finger has moved since it took the arrow, a revolution's angle round its axis where the finger stands; false when a typed number holds it.
    pub fn pull(&mut self, ray: Ray) -> bool {
        let Some(base) = self.anchor() else { return false };
        match self.stage.clone() {
            Stage::Extrude if !self.height.typed => {
                let Some(rise) = self.rise() else { return false };
                let Some(t) = along_line(ray, base, rise) else { return false };
                let grip = *self.grip.get_or_insert(self.height.value - t);
                self.height.value = (((t + grip) / HEIGHT_STEP_MM).round() * HEIGHT_STEP_MM).clamp(HEIGHT_STEP_MM, MAX_HEIGHT_MM);
                true
            }
            Stage::Revolve { .. } if !self.degrees.typed => {
                let Some((p0, a)) = self.revolution() else { return false };
                let Some(hit) = on_plane(ray, base, a) else { return false };
                let Some(deg) = turned(p0, a, base, hit) else { return false };
                let snapped = (deg / ANGLE_STEP_DEG).round() * ANGLE_STEP_DEG;
                self.degrees.value = if snapped <= 0.0 { 360.0 } else { snapped.min(360.0) };
                true
            }
            _ => false,
        }
    }

    /// The finger let go of the solid's arrow: the next one takes it where it lands.
    pub fn pull_end(&mut self) {
        self.grip = None;
    }

    /// The height an extrusion stands, mm.
    pub fn height_mm(&self) -> f64 {
        self.height.value
    }

    /// The angle a revolution turns, degrees.
    pub fn degrees(&self) -> f64 {
        self.degrees.value
    }

    /// The operation the solid being set makes of sketch feature `sketch`; `None` while no solid is set.
    /// A cut extrusion runs from the plane it was drawn on down into the metal, its walls opening toward the plane by the draft;
    /// a revolution's line is read in the sketch's plane, so it moves with the face the sketch lies on.
    pub fn solid(&self, sketch: Id) -> Option<Operation> {
        self.frame?;
        let profile = match self.pick {
            Some(region) => Profile::Region { feature: sketch, region },
            None => Profile::Feature { feature: sketch },
        };
        match &self.stage {
            Stage::Extrude if self.cutting() => Some(Operation::Extrude { sketch: profile, height_mm: -self.height.value, draft_deg: self.draft.value }),
            Stage::Extrude => Some(Operation::Extrude { sketch: profile, height_mm: self.height.value, draft_deg: self.draft.value }),
            Stage::Revolve { pivot, dir, .. } => {
                let sign = if self.cutting() { -1.0 } else { 1.0 };
                Some(Operation::Revolve { sketch: profile, pivot: [pivot[0], pivot[1], 0.0], axis: [dir[0] * sign, dir[1] * sign, 0.0], degrees: self.degrees.value, in_plane: true })
            }
            _ => None,
        }
    }

    /// What finishing leaves `design` as, one funnel commit: the sketch added or its feature replaced, then the solid the stage sets, attached as chosen; empty when nothing changes.
    /// The solid reads the sketch where it was drawn, whichever way it runs and whatever else reads it.
    pub fn edits(&self, design: &RingDesign) -> Result<Vec<CadEdit>, String> {
        let mut next = fresh_ids(design);
        let mut edits = Vec::new();
        let id = match self.feature {
            Some(id) => {
                let f = design.cad.as_ref().and_then(|d| d.feature(id)).ok_or_else(|| format!("Sketch #{id} left the document"))?;
                let mut operation = f.operation.clone();
                let Some(sketch) = operation.sketch_mut() else { return Err(format!("{} holds no sketch", f.name)) };
                if *sketch != self.working {
                    *sketch = self.working.clone();
                    edits.push(CadEdit::Operation { id, operation });
                }
                id
            }
            None if self.working.entities.is_empty() => return Ok(Vec::new()),
            None => {
                let id = next();
                let feature = Feature { id, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch: self.working.clone() }, component: Component::default() };
                edits.push(CadEdit::Add { feature, after: None });
                id
            }
        };
        if let Some(operation) = self.solid(id) {
            if self.pick.is_none()
                && let Err(why) = self.working.sweep_regions()
            {
                return Err(format!("{why:#}; tap one region to make it alone"));
            }
            let (shank, fallback) = shank_for_body(design, &mut next);
            // A ring of parts alone has no band to join to or carve.
            let attach = match (fallback, self.attach) {
                (Attach::Separate, Attach::Cut) => return Err(ALL_PARTS.into()),
                (Attach::Separate, _) => Attach::Separate,
                (_, chosen) => chosen,
            };
            edits.extend(shank);
            let name = if attach == Attach::Cut { format!("{} cut", operation.label()) } else { operation.label().to_string() };
            edits.push(CadEdit::Add { feature: Feature { id: next(), name, enabled: true, operation, component: Component { attach, ..Component::default() } }, after: None });
        }
        Ok(edits)
    }

    /// What the stage in hand asks of a finger, in a finger's words.
    pub fn prompt(&mut self) -> String {
        let what = if self.pick.is_some() { " this region" } else { "" };
        let apart = self.stage == Stage::Offer && self.pick.is_none() && self.apart().is_some();
        match self.stage.clone() {
            Stage::Draw if self.erase => "Erase: tap a curve or a point to delete it".into(),
            Stage::Draw => {
                let mut words = touch_words(self.tools.prompt());
                let drawing = matches!(self.tools.tool, Tool::Line | Tool::Rectangle | Tool::Circle | Tool::Arc);
                if drawing && !self.tools.busy() {
                    words.push_str(", or drag from it");
                }
                words
            }
            Stage::Offer => match self.regions() {
                Ok([]) | Err(_) => "Keep the sketch as it is, or Back to close a loop".into(),
                Ok([_]) => "Extrude or Revolve the region, or keep the sketch".into(),
                Ok(_) if apart => "These regions share a curve: tap one to make it alone, then Extrude or Revolve, or keep the sketch".into(),
                Ok(_) => format!("Extrude or Revolve{}, or keep the sketch; tap a region to make it alone", if what.is_empty() { " every region" } else { what }),
            },
            Stage::Extrude if self.cutting() => format!("Cut{what}: drag the arrow into the metal or type the depth, then Extrude"),
            Stage::Extrude => format!("Extrude{what}: drag the arrow or type the height, then Extrude"),
            Stage::Axis => format!("Revolve{what}: tap a straight line of the sketch, or near one of its axes, to turn about"),
            Stage::Revolve { axis, .. } if self.cutting() => format!("Cut{what} by revolving about {axis}: drag round into the metal or type the angle, then Revolve"),
            Stage::Revolve { axis, .. } => format!("Revolve{what} about {axis}: drag round or type the angle, then Revolve"),
        }
    }

    /// What the view shows for the stage in hand: square to the plane while drawing, tilted to watch an extrusion rise or a revolution turn.
    pub fn look(&mut self) -> Option<Look> {
        let f = self.frame?;
        let least = if self.face.is_empty() { MIN_REACH_MM } else { MIN_FACE_REACH_MM };
        let reach = self.face.iter().flatten().map(|w| f.local(*w)).chain(self.working.points.iter().map(|p| p.xy)).map(|uv| ringdesign_core::sketch::distance(uv, self.centre)).fold(least, f64::max);
        let square = Look { eye: f.n, up: f.y, centre: f.point(self.centre), reach_mm: reach };
        match self.stage.clone() {
            Stage::Draw | Stage::Offer | Stage::Axis => Some(square),
            Stage::Extrude => {
                let (s, c) = EXTRUDE_TILT_DEG.to_radians().sin_cos();
                let base = self.anchor().unwrap_or(square.centre);
                let (h, rise) = (self.height.value, self.rise().unwrap_or(f.n));
                let eye = std::array::from_fn(|k| f.n[k] * c - f.y[k] * s);
                let up = std::array::from_fn(|k| f.n[k] * s + f.y[k] * c);
                let centre = std::array::from_fn(|k| base[k] + rise[k] * h * 0.5);
                Some(Look { eye, up, centre, reach_mm: reach.max(h) })
            }
            Stage::Revolve { .. } => {
                let (p0, a) = self.revolution()?;
                let base = self.anchor().unwrap_or(square.centre);
                let foot: [f64; 3] = std::array::from_fn(|k| p0[k] + a[k] * dot(sub(base, p0), a));
                let side = unit(sub(base, foot)).unwrap_or(f.y);
                let eye = unit(std::array::from_fn(|k| f.n[k] + a[k] * 0.8)).unwrap_or(f.n);
                let r = dot(sub(base, foot), sub(base, foot)).sqrt();
                Some(Look { eye, up: side, centre: foot, reach_mm: (r * 1.5).max(reach) })
            }
        }
    }

    /// Moves a sketch laid on the band between the plane square to it and the one through the finger's axis; false for a sketch laid anywhere else.
    pub fn switch_plane(&mut self, built: &BuildResult) -> Result<&'static str, String> {
        let (theta_deg, across_mm, section) = match self.place {
            Some(Place::Tangent { theta_deg, across_mm }) => (theta_deg, across_mm, true),
            Some(Place::Section { theta_deg, across_mm }) => (theta_deg, across_mm, false),
            _ => return Err("Only a sketch laid on the band switches between its planes".into()),
        };
        let place = if section { Place::Section { theta_deg, across_mm } } else { Place::Tangent { theta_deg, across_mm } };
        let (fresh, centre) = start(built, place)?;
        self.working.plane = fresh.plane;
        self.place = Some(place);
        self.centre = centre;
        self.bumps += 1;
        self.frame = None;
        self.read(built);
        Ok(if section { "Sketching on the section through the finger's axis" } else { "Sketching on the plane square to the band" })
    }
}

/// The signed angle from `from` to `to` round the axis through `p0` along `a`, in (0, 360] degrees; `None` for a point on the axis.
fn turned(p0: [f64; 3], a: [f64; 3], from: [f64; 3], to: [f64; 3]) -> Option<f64> {
    let a = unit(a)?;
    let off = |p: [f64; 3]| {
        let d = sub(p, p0);
        let along = dot(d, a);
        unit(std::array::from_fn(|k| d[k] - a[k] * along))
    };
    let (u, v) = (off(from)?, off(to)?);
    let deg = dot(cross(u, v), a).atan2(dot(u, v)).to_degrees();
    Some(if deg <= 0.0 { deg + 360.0 } else { deg })
}

/// The procedural shank a document without one gains before its first body, and how that body meets the band.
fn shank_for_body(design: &RingDesign, next: &mut impl FnMut() -> Id) -> (Option<CadEdit>, Attach) {
    match design.cad.as_ref() {
        Some(doc) if doc.band().is_some() => (None, Attach::Join),
        Some(doc) if doc.replaces_band() => (None, Attach::Separate),
        _ => {
            let component = Component { role: ComponentRole::Shank, ..Component::default() };
            let feature = Feature { id: next(), name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component };
            (Some(CadEdit::Add { feature, after: None }), Attach::Join)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::touch::funnel::prepare;
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Document, Placement},
        mesh,
        sketch::{Geometry, distance},
        templates,
    };

    /// Points a millimetre the tests draw at, as a phone's view of a part face does.
    const PX: f64 = 40.0;

    fn court() -> RingDesign {
        templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }
    fn params() -> BuildParams {
        BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() }
    }
    /// The Court band with a 6 × 4 × 2 mm box joined at its top.
    fn boxed() -> RingDesign {
        let mut d = court();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
        let component = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Box".into(), enabled: true, operation: Operation::Box { size: [6.0, 4.0, 2.0] }, component }).unwrap();
        d.cad = Some(doc);
        d
    }
    /// The planar face of part `id` facing out along +y, the ring's top.
    fn top_face(built: &BuildResult, id: Id) -> u32 {
        let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap();
        (0..c.trace.face_kind.len() as u32).find(|f| cad::pattern::planar_face(c, *f).is_ok_and(|(_, _, n)| n[1] > 0.99)).expect("the box has a face on top")
    }
    /// A pad on the box's top face, its plane read.
    fn on_top(d: &RingDesign) -> (BuildResult, Pad) {
        let built = mesh::build(d, &AlphaLibrary::builtin(), params());
        let face = top_face(&built, 2);
        let (sketch, centre) = start(&built, Place::Face { feature: 2, face }).unwrap();
        let mut pad = Pad::new(None, sketch, centre, Some(Place::Face { feature: 2, face }));
        pad.read(&built);
        assert!(pad.error().is_none(), "{:?}", pad.error());
        (built, pad)
    }
    fn tap(pad: &mut Pad, xy: [f64; 2]) -> Outcome {
        pad.tap(xy, PX)
    }

    #[test]
    fn taps_draw_every_shape_a_polyline_closes_on_its_first_point_and_a_tap_erases() {
        let (_, mut pad) = on_top(&boxed());
        // The face's own loop lies under the sketch, 6 × 4 about its centre.
        assert_eq!(pad.underlay().face.len(), 1);
        let face = &pad.underlay().face[0];
        let span = |k: usize| face.iter().map(|p| p[k]).fold(f64::MIN, f64::max) - face.iter().map(|p| p[k]).fold(f64::MAX, f64::min);
        let mut spans = [span(0), span(1)];
        spans.sort_by(f64::total_cmp);
        assert!((spans[0] - 4.0).abs() < 1e-6 && (spans[1] - 6.0).abs() < 1e-6, "{spans:?}");
        // The face's own triangles lie in its plane; none of their edges is left inside it to snap a finger to.
        let half = |k: usize| face.iter().map(|p| p[k].abs()).fold(0.0, f64::max) - 0.1;
        let (hx, hy) = (half(0), half(1));
        let within = |p: [f64; 2]| p[0].abs() < hx && p[1].abs() < hy;
        assert!(pad.underlay().cut.iter().all(|s| !within([(s[0][0] + s[1][0]) * 0.5, (s[0][1] + s[1][1]) * 0.5])), "{:?}", pad.underlay().cut);
        // A finger's whole reach finds the grid; lines and axes answer only nearer, points as far as the grid.
        assert_eq!(pad.snap([1.18, -1.36], PX).map(|s| (s.kind, s.xy)), Some((SnapKind::Grid, [1.0, -1.5])));
        assert_eq!(pad.snap([0.12, -1.23], PX).map(|s| s.kind), Some(SnapKind::Axis), "0.12 mm from the y axis");
        assert_eq!(pad.snap([0.3, -1.23], PX).map(|s| (s.kind, s.xy)), Some((SnapKind::Grid, [0.5, -1.0])), "0.3 mm off the axis is past a line's band");
        // Two taps a rectangle; they land on the half-millimetre grid.
        pad.set_tool(Tool::Rectangle);
        assert_eq!(tap(&mut pad, [-2.03, -0.52]), Outcome::Continue);
        assert!(matches!(tap(&mut pad, [-0.49, 0.98]), Outcome::Edited(w) if w.starts_with("Rectangle 1.500 × 1.500")));
        assert_eq!((pad.working.entities.len(), pad.working.points.len()), (4, 4));
        // Taps chain a polyline; a tap within a finger of the first point closes it on that point.
        pad.set_tool(Tool::Line);
        for p in [[0.5, -1.0], [2.5, -1.0], [2.5, 1.0]] {
            tap(&mut pad, p);
        }
        assert!(matches!(tap(&mut pad, [0.62, -0.93]), Outcome::Edited(w) if w == "Closed the loop"), "a finger's reach catches the first point");
        assert!(!pad.tools.busy());
        assert_eq!(pad.working.entities.len(), 7);
        let regions = pad.regions().unwrap().to_vec();
        assert_eq!(regions.len(), 2);
        let mut areas: Vec<f64> = regions.iter().map(Region::area).collect();
        areas.sort_by(f64::total_cmp);
        assert!((areas[0] - 2.0).abs() < 1e-9 && (areas[1] - 2.25).abs() < 1e-9, "{areas:?}");
        // A circle from its centre to its rim, the rim on the y axis's snap, and an arc through three taps.
        pad.set_tool(Tool::Circle);
        tap(&mut pad, [-1.0, -2.5]);
        assert!(matches!(tap(&mut pad, [0.04, -2.5]), Outcome::Edited(w) if w == "Circle R1.000 mm"));
        pad.set_tool(Tool::Arc);
        for p in [[1.0, 1.5], [2.0, 1.5]] {
            assert_eq!(tap(&mut pad, p), Outcome::Continue);
        }
        assert!(matches!(tap(&mut pad, [1.5, 2.0]), Outcome::Edited(w) if w == "Arc"));
        let arcs = || pad.working.entities.iter().filter(|e| matches!(e.geometry, Geometry::Arc { .. })).count();
        assert_eq!(arcs(), 1);
        // Erase takes the circle under a tap on its rim, and says so; a tap on nothing deletes nothing.
        pad.set_erase();
        let circles = |p: &Pad| p.working.entities.iter().filter(|e| matches!(e.geometry, Geometry::Circle { .. })).count();
        assert_eq!(circles(&pad), 1);
        let (points, entities) = (pad.working.points.len(), pad.working.entities.len());
        assert!(matches!(tap(&mut pad, [-1.0, -1.52]), Outcome::Edited(w) if w == "Deleted 3 from the sketch"), "the circle and the two points only it named");
        assert_eq!((circles(&pad), pad.working.points.len(), pad.working.entities.len()), (0, points - 2, entities - 1));
        assert!(matches!(tap(&mut pad, [-2.9, 1.9]), Outcome::Refused(_)));
        // Undo puts it back.
        assert!(pad.undo());
        assert_eq!(circles(&pad), 1);
        assert!(pad.prompt().starts_with("Erase"));
        pad.set_tool(Tool::Line);
        assert_eq!(pad.prompt(), "Line: tap the first point, or drag from it");
    }

    #[test]
    fn a_corner_fillet_takes_a_typed_radius_and_a_drag_draws_or_moves_what_it_starts_on() {
        let (_, mut pad) = on_top(&boxed());
        // A drag from corner to corner is a rectangle.
        pad.set_tool(Tool::Rectangle);
        assert!(pad.drag_start([-1.0, -1.0], [0.0, 0.0], PX));
        pad.drag_move([0.5, 0.5], PX);
        assert!(!pad.tools.preview(&pad.working).strokes.is_empty(), "the rubber band follows the finger");
        assert!(matches!(pad.drag_end([1.02, 0.97], PX), Outcome::Edited(w) if w == "Rectangle 2.000 × 2.000 mm"));
        // A tap on a corner, the radius typed, Done: the corner rounds by a quarter circle.
        pad.set_tool(Tool::Fillet);
        assert_eq!(tap(&mut pad, [0.98, 1.02]), Outcome::Continue);
        assert_eq!(pad.dimensions()[0].key, "radius");
        pad.typed("radius", 0.5);
        assert!(matches!(pad.confirm(), Outcome::Edited(w) if w == "Fillet 0.500 mm"));
        let area = pad.regions().unwrap()[0].area();
        assert!((area - (4.0 - 0.25 * (1.0 - std::f64::consts::FRAC_PI_4))).abs() < 1e-6, "{area}");
        // In Select a drag from a point carries it, and a drag from nothing is the view's.
        pad.set_tool(Tool::Select);
        assert!(!pad.drag_start([2.5, 2.5], [2.6, 2.6], PX), "nothing under the finger: the view pans");
        let corner = pad.working.points.iter().find(|p| distance(p.xy, [-1.0, -1.0]) < 1e-9).unwrap().id;
        assert!(pad.drag_start([-1.0, -1.0], [-1.2, -1.2], PX));
        assert!(pad.holding());
        assert!(matches!(pad.drag_end([-1.52, -1.48], PX), Outcome::Edited(_)));
        let moved = pad.working.at(corner).unwrap();
        assert!(distance(moved, [-1.5, -1.5]) < 1e-6, "the point lands on the grid: {moved:?}");
        assert!(pad.undo(), "the drag is one undo step");
        assert!(distance(pad.working.at(corner).unwrap(), [-1.0, -1.0]) < 1e-9);
        // A line typed by its length and angle.
        pad.set_tool(Tool::Line);
        tap(&mut pad, [-2.5, 0.0]);
        pad.typed("length", 1.25);
        pad.typed("angle", 90.0);
        assert!(matches!(pad.confirm(), Outcome::Edited(w) if w == "Line"));
        assert!(pad.working.points.iter().any(|p| distance(p.xy, [-2.5, 1.25]) < 1e-12));
    }

    #[test]
    fn a_tap_picks_one_region_of_several_to_sweep_alone_and_again_lets_it_go() {
        let (_, mut pad) = on_top(&boxed());
        assert!(matches!(pad.finish(), Outcome::Refused(_)), "nothing drawn, nothing to finish");
        assert_eq!(pad.stage, Stage::Draw);
        pad.set_tool(Tool::Rectangle);
        for (a, b) in [([-2.5, -1.5], [-0.5, 0.5]), ([0.5, -1.0], [2.5, 1.0])] {
            tap(&mut pad, a);
            tap(&mut pad, b);
        }
        assert!(matches!(pad.finish(), Outcome::Edited(w) if w.starts_with("2 closed regions")));
        assert_eq!(pad.stage, Stage::Offer);
        assert!(matches!(tap(&mut pad, [1.5, 0.0]), Outcome::Edited(w) if w == "That region alone"));
        let pick = pad.pick.expect("the right square is picked");
        assert_eq!(pad.swept().len(), 1);
        assert!(pad.swept()[0].contains([1.5, 0.0]));
        assert_eq!(pad.make(Make::Extrude), Outcome::Continue);
        let d = boxed();
        let edits = pad.edits(&d).unwrap();
        let CadEdit::Add { feature: sketch, .. } = &edits[0] else { panic!("{edits:?}") };
        let CadEdit::Add { feature: solid, .. } = edits.last().unwrap() else { panic!("{edits:?}") };
        assert!(matches!(&solid.operation, Operation::Extrude { sketch: Profile::Region { feature, region }, .. } if *feature == sketch.id && *region == pick));
        // A tap on the picked region again, or on nothing, sweeps every region.
        assert!(matches!(tap(&mut pad, [1.0, 0.5]), Outcome::Edited(w) if w == "Every region: all 2"));
        assert!(pad.pick.is_none());
        tap(&mut pad, [-1.5, -0.5]);
        assert!(pad.pick.is_some());
        tap(&mut pad, [0.0, 2.0]);
        assert!(pad.pick.is_none() && pad.swept().len() == 2);
        assert!(matches!(pad.solid(sketch.id), Some(Operation::Extrude { sketch: Profile::Feature { .. }, .. })));
        // Back walks out to the drawing again.
        assert_eq!(pad.escape(), Escaped::Step);
        assert_eq!(pad.stage, Stage::Offer);
        assert_eq!(pad.escape(), Escaped::Step);
        assert_eq!(pad.stage, Stage::Draw);
    }

    #[test]
    fn a_finished_extrude_on_a_boxs_face_stands_on_it_and_adds_its_volume_as_one_commit() {
        let d = boxed();
        let (built, mut pad) = on_top(&d);
        let before = built.mesh.volume_mm3();
        // A tap for the first corner, the sides typed, Done.
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [-1.0, -0.5]);
        pad.typed("width", 2.0);
        pad.typed("height", 1.5);
        assert!(matches!(pad.confirm(), Outcome::Edited(_)));
        assert!(matches!(pad.finish(), Outcome::Edited(w) if w.starts_with("One closed region")));
        assert_eq!(pad.make(Make::Extrude), Outcome::Continue);
        // A drag along the normal moves the height as far as the finger moves, in steps; a typed one holds against it.
        let base = pad.anchor().unwrap();
        let n = pad.frame().unwrap().n;
        let side = pad.frame().unwrap().x;
        let ray = |h: f64| Ray { origin: std::array::from_fn(|k| base[k] + n[k] * h + side[k] * 30.0), direction: side.map(|v| -v) };
        assert!(pad.pull(ray(2.234)));
        assert!((pad.height_mm() - 1.0).abs() < 1e-9, "the arrow taken where the finger landed moves nothing: {}", pad.height_mm());
        assert!(pad.pull(ray(2.468)));
        assert!((pad.height_mm() - 1.25).abs() < 1e-9, "{}", pad.height_mm());
        pad.pull_end();
        pad.typed("height", 0.8);
        assert!(!pad.pull(ray(3.0)), "a typed height holds");
        assert_eq!(pad.height_mm(), 0.8);
        let edits = pad.edits(&d).unwrap();
        let p = prepare(&d, &edits, built.parts.evaluated.as_ref()).unwrap().unwrap();
        assert_eq!(p.label, "Add Sketch · Add Extrude", "one commit, one undo step");
        let doc = p.design.cad.as_ref().unwrap();
        let (sketch, solid) = (p.applied[0].id.unwrap(), p.applied[1].id.unwrap());
        assert!(matches!(doc.feature(sketch).unwrap().operation, Operation::Sketch { .. }));
        assert_eq!(doc.feature(solid).unwrap().component.attach, Attach::Join);
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        let c = after.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == solid).expect("the extrusion built");
        let v = c.mesh.volume_mm3();
        assert!((v - 2.4).abs() < 2.4e-4, "2 × 1.5 × 0.8 = 2.4 mm³, got {v}");
        // It stands on the face: its lowest point along the normal is the face's own plane.
        let face = pad.frame().unwrap().origin;
        let rise = |p: &ringdesign_core::Vec3| dot(sub([p.0 as f64, p.1 as f64, p.2 as f64], face), n);
        let low = c.mesh.vertices.iter().map(rise).fold(f64::MAX, f64::min);
        let high = c.mesh.vertices.iter().map(rise).fold(f64::MIN, f64::max);
        assert!(low.abs() < 1e-4 && (high - 0.8).abs() < 1e-4, "{low} to {high}");
        let grew = after.mesh.volume_mm3() - before;
        assert!((grew - 2.4).abs() < 0.024, "joined into the ring it adds its own volume: {grew}");
    }

    #[test]
    fn a_revolve_takes_its_axis_from_a_tap_and_its_angle_from_a_finger_round_it() {
        let (_, mut pad) = on_top(&boxed());
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [0.5, -0.5]);
        tap(&mut pad, [1.5, 0.5]);
        pad.finish();
        assert_eq!(pad.make(Make::Revolve), Outcome::Continue);
        assert_eq!(pad.stage, Stage::Axis);
        assert!(matches!(tap(&mut pad, [2.9, 2.9]), Outcome::Refused(_)), "no line and no axis near");
        assert!(matches!(tap(&mut pad, [0.05, 1.7]), Outcome::Edited(w) if w == "Revolving about the sketch's y axis"));
        let f = *pad.frame().unwrap();
        let base = pad.anchor().unwrap();
        // A finger a quarter turn round the axis toward the plane's normal, the way a positive turn swings the region: 90°.
        let quarter = f.point([0.0, f.local(base)[1]]);
        let to: [f64; 3] = std::array::from_fn(|k| quarter[k] + f.n[k] * 1.0);
        let ray = Ray { origin: std::array::from_fn(|k| to[k] + f.y[k] * 20.0), direction: f.y.map(|v| -v) };
        assert!(pad.pull(ray));
        assert!((pad.degrees() - 90.0).abs() < 1e-9, "{}", pad.degrees());
        let Some(Operation::Revolve { pivot, axis: local, degrees, in_plane: true, .. }) = pad.solid(7) else { panic!() };
        assert!(pivot == [0.0; 3] && local[0] == 0.0 && local[1].abs() == 1.0 && local[2] == 0.0 && degrees == 90.0, "the sketch's y axis, read in its plane");
        let (at, axis) = pad.revolution().unwrap();
        assert!(distance(f.local(at), [0.0, 0.0]) < 1e-9 && sub(axis, f.vector([local[0], local[1]])).iter().all(|v| v.abs() < 1e-12));
        let off = sub(base, quarter);
        assert!(dot(cross(axis, off), f.n) > 0.0, "a positive turn swings the region out of the face");
        // The other side of the axis reads the long way round.
        let back: [f64; 3] = std::array::from_fn(|k| quarter[k] - f.n[k] * 1.0);
        assert!(pad.pull(Ray { origin: std::array::from_fn(|k| back[k] + f.y[k] * 20.0), direction: f.y.map(|v| -v) }));
        assert!((pad.degrees() - 270.0).abs() < 1e-9, "{}", pad.degrees());
        assert!(matches!(pad.typed("angle", 400.0), Outcome::Refused(_)));
        assert_eq!(pad.typed("angle", 180.0), Outcome::Continue);
        assert_eq!(pad.dimensions()[0].value, 180.0);
        assert_eq!(pad.escape(), Escaped::Field);
        assert_eq!(pad.escape(), Escaped::Step);
        assert_eq!(pad.stage, Stage::Axis);
    }

    #[test]
    fn the_view_stands_square_to_the_plane_with_its_y_up_and_tilts_to_watch_a_solid_rise() {
        let (_, mut pad) = on_top(&boxed());
        let f = *pad.frame().unwrap();
        let look = pad.look().unwrap();
        assert_eq!((look.eye, look.up), (f.n, f.y));
        let current = Pose { yaw: 0.3, pitch: 0.2, roll: 0.0, zoom: 1.0, pan: [0.0; 2] };
        let pose = look_along(current, [0.0; 3], 14.0, look.eye, look.up, look.centre, look.reach_mm);
        let (s, u) = view_axes(pose.yaw, pose.pitch, pose.roll);
        let d = |a: [f32; 3], b: [f64; 3]| f64::from(a[0]) * b[0] + f64::from(a[1]) * b[1] + f64::from(a[2]) * b[2];
        assert!(d(u, f.y) > 0.9999 && d(s, f.x) > 0.9999, "y up the screen, x to the right: {u:?} {s:?}");
        // The pan puts the plane's centre in the middle of the view.
        assert!((f64::from(pose.pan[0]) - d(s, look.centre)).abs() < 1e-5 && (f64::from(pose.pan[1]) - d(u, look.centre)).abs() < 1e-5);
        assert!((pose.zoom - 14.0 / (look.reach_mm as f32 * 1.6)).abs() < 1e-5);
        // Setting an extrusion tilts the eye off the normal so the height shows.
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [-1.0, -1.0]);
        tap(&mut pad, [1.0, 1.0]);
        pad.finish();
        pad.make(Make::Extrude);
        let tilted = pad.look().unwrap();
        let off_normal = dot(unit(tilted.eye).unwrap(), f.n).acos().to_degrees();
        assert!((off_normal - EXTRUDE_TILT_DEG).abs() < 1e-6, "{off_normal}");
        assert!(dot(tilted.up, f.n) > 0.0, "the extrusion rises up the screen");
    }

    #[test]
    fn a_sketch_on_the_band_switches_to_the_section_and_a_work_plane_carries_one() {
        let d = boxed();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let place = Place::Tangent { theta_deg: 30.0, across_mm: 0.0 };
        let (sketch, centre) = start(&built, place).unwrap();
        let mut pad = Pad::new(None, sketch, centre, Some(place));
        pad.read(&built);
        let f = *pad.frame().unwrap();
        let (s, c) = 30f64.to_radians().sin_cos();
        assert!(dot(f.n, [c, s, 0.0]) > 0.999 && f.y[2].abs() > 0.999, "square to the band, y along the finger: {f:?}");
        assert_eq!(pad.switch_plane(&built).unwrap(), "Sketching on the section through the finger's axis");
        let f = *pad.frame().unwrap();
        assert!(dot(f.x, [c, s, 0.0]) > 0.999 && f.origin == [0.0; 3]);
        let hit = cad::surface_hit(&built.mesh, 30.0, 0.0).unwrap().0;
        let centre = f.point(pad.centre);
        assert!((0..3).all(|k| (centre[k] - hit[k]).abs() < 1e-6), "the view centres where the band was pressed: {centre:?} {hit:?}");
        assert!(!pad.underlay().cut.is_empty(), "the section cuts the band");
        // A plane through the finger's axis carries a sketch too.
        let mut doc = d.cad.clone().unwrap();
        doc.append(Feature { id: 3, name: "Section at 90°".into(), enabled: true, operation: Operation::Plane { base: cad::PlaneBase::Section { theta_deg: 90.0 }, offset_mm: 0.0 }, component: Component::default() }).unwrap();
        let mut planed = d.clone();
        planed.cad = Some(doc);
        let built = mesh::build(&planed, &AlphaLibrary::builtin(), params());
        let (sketch, _) = start(&built, Place::Plane(3)).unwrap();
        assert_eq!(sketch.plane.on_face.as_ref().map(|a| a.feature), Some(3));
        let (frame, face) = resolve(&sketch, &built).unwrap();
        assert!(face.is_empty() && (frame.n[0] - 1.0).abs() < 1e-9, "{frame:?}");
        assert!(start(&built, Place::Plane(9)).is_err());
        assert!(pad.switch_plane(&built).is_ok(), "a band sketch switches back");
        let mut planar = Pad::new(None, sketch, [0.0; 2], Some(Place::Plane(3)));
        assert!(planar.switch_plane(&built).is_err());
    }

    #[test]
    fn a_sketch_already_in_the_document_is_replaced_and_a_body_on_a_plain_band_brings_its_shank() {
        // On a plain band with no parts, a plate on the band's top brings the procedural shank with it.
        let d = court();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let place = Place::Tangent { theta_deg: 90.0, across_mm: 0.0 };
        let (sketch, centre) = start(&built, place).unwrap();
        let mut pad = Pad::new(None, sketch, centre, Some(place));
        pad.read(&built);
        assert!(pad.edits(&d).unwrap().is_empty(), "nothing drawn, nothing to commit");
        pad.set_tool(Tool::Circle);
        tap(&mut pad, [0.0, 0.0]);
        tap(&mut pad, [1.0, 0.0]);
        pad.finish();
        pad.make(Make::Extrude);
        pad.typed("height", 0.5);
        let edits = pad.edits(&d).unwrap();
        let names: Vec<&str> = edits.iter().filter_map(|e| if let CadEdit::Add { feature, .. } = e { Some(feature.name.as_str()) } else { None }).collect();
        assert_eq!(names, ["Sketch", "Procedural shank", "Extrude"]);
        let p = prepare(&d, &edits, None).unwrap().unwrap();
        let doc = p.design.cad.as_ref().unwrap();
        assert!(doc.band().is_some() && doc.features.last().unwrap().component.attach == Attach::Join);
        // Reopened, the sketch's feature is replaced in place and the extrude stays as it was.
        let sketch_id = p.applied[0].id.unwrap();
        let Operation::Sketch { sketch } = doc.feature(sketch_id).unwrap().operation.clone() else { panic!() };
        let mut again = Pad::new(Some(sketch_id), sketch, [0.0; 2], None);
        assert!(!again.dirty());
        assert!(again.edits(&p.design).unwrap().is_empty());
        again.set_tool(Tool::Line);
        tap(&mut again, [2.0, 0.0]);
        tap(&mut again, [3.0, 0.0]);
        assert!(again.dirty());
        let edits = again.edits(&p.design).unwrap();
        assert!(matches!(&edits[..], [CadEdit::Operation { id, operation: Operation::Sketch { sketch } }] if *id == sketch_id && sketch.entities.len() == 2));
    }

    /// Every point the sketch's curves pass through, in plane coordinates.
    fn traced(s: &Sketch) -> Vec<[f64; 2]> {
        s.entities.iter().flat_map(|e| s.polylines(e.id, 0.01)).flatten().collect()
    }

    /// The areas of the regions the pad's sketch bounds, smallest first.
    fn areas(pad: &mut Pad) -> Vec<f64> {
        let mut a: Vec<f64> = pad.regions().unwrap().iter().map(Region::area).collect();
        a.sort_by(f64::total_cmp);
        a
    }

    /// A 2 × 2 mm square about the face's centre, by two taps.
    fn square(pad: &mut Pad) {
        pad.set_tool(Tool::Rectangle);
        tap(pad, [-1.0, -1.0]);
        tap(pad, [1.0, 1.0]);
    }

    #[test]
    fn trim_cuts_the_span_a_tap_lands_on_back_to_its_crossing() {
        let (_, mut pad) = on_top(&boxed());
        // A line across and one up the y axis, each left open by Back, crossing at (0, 0.5).
        pad.set_tool(Tool::Line);
        for (a, b) in [([-2.0, 0.5], [2.0, 0.5]), ([0.0, -1.5], [0.0, 1.5])] {
            tap(&mut pad, a);
            tap(&mut pad, b);
            assert_eq!(pad.escape(), Escaped::Step);
        }
        let reach = |pad: &Pad| traced(&pad.working).iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!((reach(&pad) - 2.0).abs() < 1e-9);
        pad.set_tool(Tool::Trim);
        assert_eq!(pad.prompt(), "Trim: tap the span to cut away");
        // A tap on nothing says so in a finger's words and trims nothing.
        assert_eq!(tap(&mut pad, [2.5, -1.8]), Outcome::Refused("Tap on the span of a curve to trim".into()));
        // A tap on the right arm takes it back to the crossing; the left arm and the upright stay.
        assert!(matches!(tap(&mut pad, [1.4, 0.55]), Outcome::Edited(w) if w == "Trimmed"));
        assert!(reach(&pad).abs() < 1e-9, "nothing reaches past the crossing now: {}", reach(&pad));
        let left = traced(&pad.working).iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        assert!((left + 2.0).abs() < 1e-9, "{left}");
        assert!(pad.undo(), "a trim is one undo step");
        assert!((reach(&pad) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn offset_takes_its_loop_from_a_tap_and_its_distance_from_a_second_tap_a_drag_or_the_keyboard() {
        let (_, mut pad) = on_top(&boxed());
        square(&mut pad);
        pad.set_tool(Tool::Offset);
        assert_eq!(tap(&mut pad, [2.5, 1.9]), Outcome::Refused("Tap a curve of a closed loop to offset".into()));
        // A tap on the square's right edge takes its loop and waits for how far.
        assert_eq!(tap(&mut pad, [1.02, 0.3]), Outcome::Continue);
        assert!(pad.tools.busy());
        assert_eq!(pad.dimensions()[0].key, "distance");
        assert_eq!(pad.prompt(), "Offset: drag out or in, or type the distance; a tap or Done makes it");
        // A second tap half a millimetre outside, where the grid holds it, makes the loop there.
        assert!(matches!(tap(&mut pad, [1.5, 0.3]), Outcome::Edited(w) if w == "Offset 0.500 mm"));
        assert_eq!(areas(&mut pad), [5.0], "a 3 mm square round the 2 mm one: one frame, the first square its hole");
        // Typed: a quarter millimetre inside the first square, a region again inside its hole.
        tap(&mut pad, [1.02, 0.3]);
        assert_eq!(pad.typed("distance", -0.25), Outcome::Continue);
        assert!(matches!(pad.confirm(), Outcome::Edited(w) if w == "Offset -0.250 mm"));
        assert_eq!(areas(&mut pad), [2.25, 5.0]);
        // Dragged: out from the 3 mm square's edge, lifted where the grid holds it half a millimetre off.
        assert_eq!(tap(&mut pad, [1.52, 0.3]), Outcome::Continue);
        assert!(pad.drag_start([1.52, 0.3], [1.8, 0.3], PX), "a busy offset takes the drag");
        pad.drag_move([2.1, 0.3], PX);
        assert!(!pad.tools.preview(&pad.working).ghost.is_empty(), "the offset follows the finger");
        assert!(matches!(pad.drag_end([2.02, 0.3], PX), Outcome::Edited(w) if w == "Offset 0.500 mm"));
        assert_eq!(areas(&mut pad), [1.75, 7.0], "a 4 mm square round the 3 mm one: the frames nest by turns");
    }

    #[test]
    fn chamfer_takes_its_corner_from_a_tap_and_its_distance_from_the_keyboard() {
        let (_, mut pad) = on_top(&boxed());
        square(&mut pad);
        pad.set_tool(Tool::Chamfer);
        assert_eq!(tap(&mut pad, [0.0, 2.2]), Outcome::Refused("Tap a corner where two lines meet".into()));
        assert_eq!(tap(&mut pad, [0.98, 1.02]), Outcome::Continue);
        assert_eq!(pad.dimensions()[0].key, "distance");
        assert_eq!(pad.prompt(), "Chamfer corner: type the distance; Done or a tap cuts the corner");
        assert_eq!(pad.typed("distance", 0.5), Outcome::Continue);
        assert!(matches!(pad.confirm(), Outcome::Edited(w) if w == "Chamfer 0.500 mm"));
        let area = areas(&mut pad)[0];
        assert!((area - (4.0 - 0.125)).abs() < 1e-9, "the corner's half-millimetre triangle is gone: {area}");
        assert_eq!(pad.working.entities.len(), 5, "four sides and the chamfer across the corner");
    }

    #[test]
    fn mirror_takes_its_curves_from_taps_then_done_then_its_line_from_a_tap() {
        let (_, mut pad) = on_top(&boxed());
        // A right triangle left of the y axis, closed on its first point.
        pad.set_tool(Tool::Line);
        for p in [[-2.0, -1.0], [-1.0, -1.0], [-1.0, 0.5]] {
            tap(&mut pad, p);
        }
        assert!(matches!(tap(&mut pad, [-2.0, -1.0]), Outcome::Edited(w) if w == "Closed the loop"));
        // A construction line up the y axis to mirror in.
        pad.tools.construction = true;
        tap(&mut pad, [0.0, -1.5]);
        tap(&mut pad, [0.0, 1.5]);
        pad.escape();
        pad.tools.construction = false;
        pad.set_tool(Tool::Mirror);
        assert_eq!(pad.prompt(), "Mirror: tap the curves to mirror, then Done");
        assert_eq!(pad.confirm(), Outcome::Refused("Tap the curves to mirror first".into()));
        // A tap on each of the triangle's sides chooses it.
        for p in [[-1.5, -1.0], [-1.0, -0.2], [-1.5, -0.25]] {
            assert_eq!(tap(&mut pad, p), Outcome::Continue);
        }
        assert_eq!(pad.tools.chosen_entities.len(), 3);
        assert_eq!(pad.confirm(), Outcome::Continue, "Done asks for the line");
        assert_eq!(pad.prompt(), "Mirror: tap the line to mirror in");
        assert!(matches!(tap(&mut pad, [0.0, 1.0]), Outcome::Edited(w) if w == "Mirrored 3 curves"));
        // The copy stands across the axis: two triangles of 0.75 mm² each.
        assert_eq!(areas(&mut pad), [0.75, 0.75]);
        let right = traced(&pad.working).iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!((right - 2.0).abs() < 1e-9, "{right}");
        assert!(pad.tools.chosen_entities.is_empty() && !pad.tools.busy());
    }

    /// How many faces of `mesh` lie in `frame`'s plane over the places `over` holds: metal left as a skin over a cut's mouth.
    fn skin(mesh: &Mesh, frame: &Frame, over: impl Fn([f64; 2]) -> bool) -> usize {
        mesh.faces
            .iter()
            .filter(|f| {
                let q: Vec<[f64; 3]> = f.iter().map(|i| mesh.vertices[*i as usize]).map(|v| [f64::from(v.0), f64::from(v.1), f64::from(v.2)]).collect();
                let centre: [f64; 3] = std::array::from_fn(|k| (q[0][k] + q[1][k] + q[2][k]) / 3.0);
                q.iter().all(|p| dot(sub(*p, frame.origin), frame.n).abs() < 1e-4) && over(frame.local(centre))
            })
            .count()
    }

    /// The pad on the box's top with a 2 × 1.5 mm rectangle on it, finished and set to extrude.
    fn extruding(d: &RingDesign) -> (BuildResult, Pad) {
        let (built, mut pad) = on_top(d);
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [-1.0, -0.5]);
        pad.typed("width", 2.0);
        pad.typed("height", 1.5);
        pad.confirm();
        pad.finish();
        assert_eq!(pad.make(Make::Extrude), Outcome::Continue);
        (built, pad)
    }

    #[test]
    fn a_cut_extrusion_carves_the_part_it_stands_on_by_its_depth_as_one_commit() {
        let d = boxed();
        let (built, mut pad) = extruding(&d);
        let before = built.mesh.volume_mm3();
        let n = pad.frame().unwrap().n;
        assert_eq!(pad.rise(), Some(n), "joined, the arrow points out of the face");
        assert!(matches!(pad.set_attach(&d, Attach::Cut), Outcome::Edited(w) if w.starts_with("Cut:")));
        assert_eq!(pad.rise(), Some(n.map(|v| -v)), "cut, it points into the box");
        assert_eq!(pad.dimensions()[0].label, "Depth");
        assert_eq!(pad.prompt(), "Cut: drag the arrow into the metal or type the depth, then Extrude");
        // A finger along the arrow deepens the cut by as far as it moves.
        let base = pad.anchor().unwrap();
        let side = pad.frame().unwrap().x;
        let ray = |depth: f64| Ray { origin: std::array::from_fn(|k| base[k] - n[k] * depth + side[k] * 30.0), direction: side.map(|v| -v) };
        assert!(pad.pull(ray(0.4)) && pad.pull(ray(0.65)));
        assert!((pad.height_mm() - 1.25).abs() < 1e-9, "{}", pad.height_mm());
        pad.pull_end();
        assert_eq!(pad.typed("height", 1.0), Outcome::Continue);
        let edits = pad.edits(&d).unwrap();
        let p = prepare(&d, &edits, built.parts.evaluated.as_ref()).unwrap().unwrap();
        assert_eq!(p.label, "Add Sketch · Add Extrude cut", "one commit, one undo step");
        let doc = p.design.cad.as_ref().unwrap();
        let (sketch, cut) = (p.applied[0].id.unwrap(), p.applied[1].id.unwrap());
        // The sketch stays where it was drawn, on the box's face; the cut runs 1 mm down from it.
        let Operation::Sketch { sketch: s } = &doc.feature(sketch).unwrap().operation else { panic!() };
        assert!(s.plane.on_face.is_some() && s.plane.origin == [0.0; 3], "{:?}", s.plane);
        let f = doc.feature(cut).unwrap();
        assert_eq!(f.component.attach, Attach::Cut);
        assert!(matches!(f.operation, Operation::Extrude { sketch: Profile::Feature { feature }, height_mm, .. } if feature == sketch && height_mm == -1.0));
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        assert_eq!((after.parts.joined, after.parts.cut), (1, 1), "{:?}", after.parts.notes);
        let c = after.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == cut).expect("the cut built");
        let face = pad.frame().unwrap().origin;
        let rise = |p: &ringdesign_core::Vec3| dot(sub([p.0 as f64, p.1 as f64, p.2 as f64], face), n);
        let (low, high) = c.mesh.vertices.iter().map(rise).fold((f64::MAX, f64::MIN), |(l, h), r| (l.min(r), h.max(r)));
        assert!((low + 1.0).abs() < 1e-4 && (high - cad::CUT_CLEAR_MM).abs() < 1e-4, "the tool runs {low} to {high} along the face's normal");
        let taken = before - after.mesh.volume_mm3();
        assert!((taken - 3.0).abs() < 1e-3, "2 × 1.5 × 1 = 3 mm³ carved out of the box: {taken}");
        assert!(after.report.validation.watertight);
        assert_eq!(skin(&after.mesh, pad.frame().unwrap(), |uv| uv[0].abs() < 0.95 && (uv[1] - 0.25).abs() < 0.7), 0, "the cut opens through the face");
    }

    #[test]
    fn a_cut_on_the_band_carves_into_it_and_a_separate_solid_stands_apart() {
        let d = court();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let before = built.mesh.volume_mm3();
        let place = Place::Tangent { theta_deg: 90.0, across_mm: 0.0 };
        let (sketch, centre) = start(&built, place).unwrap();
        let mut pad = Pad::new(None, sketch, centre, Some(place));
        pad.read(&built);
        pad.set_tool(Tool::Circle);
        tap(&mut pad, [0.0, 0.0]);
        tap(&mut pad, [1.0, 0.0]);
        pad.finish();
        pad.make(Make::Extrude);
        pad.typed("height", 0.3);
        pad.set_attach(&d, Attach::Cut);
        let edits = pad.edits(&d).unwrap();
        let names: Vec<&str> = edits.iter().filter_map(|e| if let CadEdit::Add { feature, .. } = e { Some(feature.name.as_str()) } else { None }).collect();
        assert_eq!(names, ["Sketch", "Procedural shank", "Extrude cut"], "a plain band gains its shank before the cut");
        let p = prepare(&d, &edits, None).unwrap().unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        assert_eq!(after.parts.cut, 1, "{:?}", after.parts.notes);
        // The disc's 0.94 mm³ to 0.3 mm under the crest, less what the dome falls away under the plane.
        let taken = before - after.mesh.volume_mm3();
        assert!((taken - 0.737).abs() < 0.01, "a 1 mm disc cut 0.3 mm into the crest takes {taken:.4} mm³");
        assert!(after.report.validation.watertight);
        // Set apart, the same disc stands out of the plane as a casting of its own.
        assert!(matches!(pad.set_attach(&d, Attach::Separate), Outcome::Edited(w) if w.starts_with("Separate:")));
        let p = prepare(&d, &pad.edits(&d).unwrap(), None).unwrap().unwrap();
        let apart = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        assert_eq!((apart.parts.separate, apart.parts.cut, apart.parts.joined), (1, 0, 0));
        let own = std::f64::consts::PI * 0.3;
        let grew = apart.mesh.volume_mm3() - before;
        assert!((grew - own).abs() < 0.02 * own, "π × 1² × 0.3 = {own:.4} mm³ beside the band: {grew}");
        // A ring that is all parts has no band to carve.
        let mut parts_only = boxed();
        parts_only.cad.as_mut().unwrap().features.retain(|f| !matches!(f.operation, Operation::Band));
        assert_eq!(pad.set_attach(&parts_only, Attach::Cut), Outcome::Refused(ALL_PARTS.into()));
    }

    #[test]
    fn a_revolution_cut_turns_into_the_metal_and_a_cut_reads_a_sketch_another_feature_reads_where_it_is() {
        let d = boxed();
        let (_, mut pad) = on_top(&d);
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [0.5, -0.5]);
        tap(&mut pad, [1.5, 0.5]);
        pad.finish();
        pad.make(Make::Revolve);
        tap(&mut pad, [0.05, 1.7]);
        let (pivot, axis) = pad.revolution().unwrap();
        pad.set_attach(&d, Attach::Cut);
        assert_eq!(pad.revolution(), Some((pivot, axis.map(|v| -v))), "a cut turns the other way round the same line");
        let f = *pad.frame().unwrap();
        let Some(Operation::Revolve { axis: local, in_plane: true, .. }) = pad.solid(7) else { panic!() };
        let cut_axis = f.vector([local[0], local[1]]);
        assert!(sub(cut_axis, axis.map(|v| -v)).iter().all(|v| v.abs() < 1e-12) && local[2] == 0.0);
        // A positive turn of the cut swings the region into the box, against the face's normal.
        let base = pad.anchor().unwrap();
        assert!(dot(cross(cut_axis, sub(base, pivot)), f.n) < 0.0);
        // Half a turn carves a half ring of the region round the y axis out of the box.
        pad.typed("angle", 180.0);
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let p = prepare(&d, &pad.edits(&d).unwrap(), built.parts.evaluated.as_ref()).unwrap().unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        let taken = built.mesh.volume_mm3() - after.mesh.volume_mm3();
        assert_eq!(after.parts.cut, 1, "{:?}", after.parts.notes);
        assert!((taken - std::f64::consts::PI).abs() < 0.01, "π/2 × (1.5² − 0.5²) × 1 = π mm³ out of the box: {taken:.4}");
        // It starts, and ends where the half turn brings it back to the face, clear of the face: no skin over either mouth.
        assert_eq!(skin(&after.mesh, &f, |uv| (uv[0].abs() - 1.0).abs() < 0.45 && uv[1].abs() < 0.45), 0);
        // A sketch an extrusion already reads is cut from as it is: no copy, no plane moved.
        let (built, mut first) = extruding(&d);
        first.typed("height", 0.8);
        let p = prepare(&d, &first.edits(&d).unwrap(), built.parts.evaluated.as_ref()).unwrap().unwrap();
        let (sketch_id, joined) = (p.applied[0].id.unwrap(), p.applied[1].id.unwrap());
        let Operation::Sketch { sketch } = p.design.cad.as_ref().unwrap().feature(sketch_id).unwrap().operation.clone() else { panic!() };
        let mut again = Pad::new(Some(sketch_id), sketch.clone(), [0.0; 2], None);
        again.read(&mesh::build(&p.design, &AlphaLibrary::builtin(), params()));
        again.finish();
        again.make(Make::Extrude);
        again.set_attach(&p.design, Attach::Cut);
        again.typed("height", 0.3);
        let edits = again.edits(&p.design).unwrap();
        let [CadEdit::Add { feature: cut, .. }] = &edits[..] else { panic!("{edits:?}") };
        assert!(matches!(&cut.operation, Operation::Extrude { sketch: Profile::Feature { feature }, height_mm, .. } if *feature == sketch_id && *height_mm == -0.3));
        let q = prepare(&p.design, &edits, None).unwrap().unwrap();
        let doc = q.design.cad.as_ref().unwrap();
        assert!(matches!(&doc.feature(sketch_id).unwrap().operation, Operation::Sketch { sketch: s } if *s == sketch), "the drawn sketch keeps its plane");
        assert!(matches!(&doc.feature(joined).unwrap().operation, Operation::Extrude { sketch: Profile::Feature { feature }, .. } if *feature == sketch_id));
        assert_eq!(doc.features.iter().filter(|f| matches!(f.operation, Operation::Sketch { .. })).count(), 1, "one sketch, read by both");
        let after = mesh::build(&q.design, &AlphaLibrary::builtin(), params());
        assert_eq!((after.parts.joined, after.parts.cut), (2, 1), "{:?}", after.parts.notes);
    }

    #[test]
    fn a_revolution_cut_turns_about_a_line_in_its_sketch_plane_and_follows_the_face_it_was_drawn_on() {
        let d = boxed();
        let (built, mut pad) = on_top(&d);
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [0.5, -0.5]);
        tap(&mut pad, [1.5, 0.5]);
        pad.finish();
        pad.make(Make::Revolve);
        tap(&mut pad, [0.05, 1.7]);
        pad.set_attach(&d, Attach::Cut);
        pad.typed("angle", 180.0);
        let p = prepare(&d, &pad.edits(&d).unwrap(), built.parts.evaluated.as_ref()).unwrap().unwrap();
        let turn = p.applied[1].id.unwrap();
        let Operation::Revolve { pivot, axis, in_plane: true, .. } = p.design.cad.as_ref().unwrap().feature(turn).unwrap().operation else { panic!() };
        assert!(pivot == [0.0; 3] && axis[0] == 0.0 && axis[1].abs() == 1.0 && axis[2] == 0.0, "the sketch's own y axis: {pivot:?} {axis:?}");
        // The box grows a millimetre about its centre: its top rises half of it, the sketch with it, and the cut's line with the sketch.
        let grown = |d: &RingDesign, cut: bool| {
            let mut d = d.clone();
            let doc = d.cad.as_mut().unwrap();
            doc.features.iter_mut().find(|f| f.id == 2).unwrap().operation = Operation::Box { size: [6.0, 4.0, 3.0] };
            doc.features.iter_mut().find(|f| f.id == turn).unwrap().enabled = cut;
            d
        };
        let plain = mesh::build(&grown(&p.design, false), &AlphaLibrary::builtin(), params());
        let after = mesh::build(&grown(&p.design, true), &AlphaLibrary::builtin(), params());
        assert_eq!(after.parts.cut, 1, "{:?}", after.parts.notes);
        let taken = plain.mesh.volume_mm3() - after.mesh.volume_mm3();
        assert!((taken - std::f64::consts::PI).abs() < 0.01, "π mm³ out of the grown box: {taken:.4}");
        let (_, sketch) = p.design.cad.as_ref().unwrap().features.iter().find_map(|f| match &f.operation {
            Operation::Sketch { sketch } => Some((f.id, sketch.clone())),
            _ => None,
        }).unwrap();
        let (risen, _) = resolve(&sketch, &after).unwrap();
        let f = *pad.frame().unwrap();
        assert!((dot(sub(risen.origin, f.origin), f.n) - 0.5).abs() < 1e-9, "the face rose half a millimetre");
        let mouth = |uv: [f64; 2]| (uv[0].abs() - 1.0).abs() < 0.45 && uv[1].abs() < 0.45;
        assert_eq!(skin(&after.mesh, &risen, mouth), 0, "the cut opens through the risen face");
        // The same line in the world stays where it was drawn, half a millimetre under the risen face, and no profile turns about a line off its plane.
        let mut world = p.design.clone();
        let (at, line) = pad.revolution().unwrap();
        let Operation::Revolve { pivot, axis, in_plane, .. } = &mut world.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == turn).unwrap().operation else { panic!() };
        (*pivot, *axis, *in_plane) = (at, line, false);
        let stayed = mesh::build(&grown(&world, true), &AlphaLibrary::builtin(), params());
        assert_eq!(stayed.parts.cut, 0);
        assert!(stayed.parts.notes.iter().any(|n| n.ends_with("Revolution: unsupported or degenerate geometry")), "{:?}", stayed.parts.notes);
        assert!((stayed.mesh.volume_mm3() - plain.mesh.volume_mm3()).abs() < 1e-6, "nothing is carved from the grown box");
    }

    #[test]
    fn a_cut_standing_on_a_part_set_apart_carves_it_as_well_as_the_band() {
        // The box set apart from the band, as a casting of its own.
        let mut d = boxed();
        d.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 2).unwrap().component.attach = Attach::Separate;
        let (built, mut pad) = extruding(&d);
        assert_eq!((built.parts.separate, built.parts.joined), (1, 0));
        pad.set_attach(&d, Attach::Cut);
        pad.typed("height", 0.8);
        let p = prepare(&d, &pad.edits(&d).unwrap(), built.parts.evaluated.as_ref()).unwrap().unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        assert_eq!((after.parts.separate, after.parts.cut), (1, 1), "{:?}", after.parts.notes);
        let taken = built.mesh.volume_mm3() - after.mesh.volume_mm3();
        assert!((taken - 2.4).abs() < 1e-3, "2 × 1.5 × 0.8 = 2.4 mm³ out of the box standing apart: {taken}");
        assert!(after.report.validation.watertight);
        assert_eq!(skin(&after.mesh, pad.frame().unwrap(), |uv| uv[0].abs() < 0.95 && (uv[1] - 0.25).abs() < 0.7), 0);
    }

    #[test]
    fn regions_sharing_a_curve_are_made_one_at_a_time_and_a_tap_names_each_by_its_own_side() {
        let d = boxed();
        let (built, mut pad) = on_top(&d);
        // A 2 × 1.5 rectangle with a line across it from the bottom side to the top, ending on both.
        pad.set_tool(Tool::Rectangle);
        tap(&mut pad, [-1.0, -0.5]);
        pad.typed("width", 2.0);
        pad.typed("height", 1.5);
        pad.confirm();
        let (a, b) = (pad.working.place([0.5, -0.5]), pad.working.place([0.5, 1.0]));
        pad.working.add_line(a, b, false).unwrap();
        pad.bumps += 1;
        assert!(matches!(pad.finish(), Outcome::Edited(w) if w.starts_with("2 closed regions")));
        assert_eq!(pad.prompt(), "These regions share a curve: tap one to make it alone, then Extrude or Revolve, or keep the sketch");
        let refused = pad.make(Make::Extrude);
        assert!(matches!(&refused, Outcome::Refused(w) if w.contains("joins 3 curves") && w.ends_with("tap one region to make it alone")), "{refused:?}");
        assert_eq!(pad.stage, Stage::Offer);
        // A tap on the wider part picks it; the line they share never names it.
        assert!(matches!(tap(&mut pad, [-0.3, 0.2]), Outcome::Edited(w) if w == "That region alone"));
        let pick = pad.pick.unwrap();
        assert!(pad.working.entities.iter().any(|e| e.id == pick.entity && matches!(e.geometry, ringdesign_core::sketch::Geometry::Line { .. })));
        assert_eq!(pad.make(Make::Extrude), Outcome::Continue);
        assert!(matches!(tap(&mut pad, [-0.3, 0.2]), Outcome::Refused(_)), "the pick is not let go while the regions share a curve");
        pad.typed("height", 0.4);
        let p = prepare(&d, &pad.edits(&d).unwrap(), built.parts.evaluated.as_ref()).unwrap().unwrap();
        let after = mesh::build(&p.design, &AlphaLibrary::builtin(), params());
        let grew = after.mesh.volume_mm3() - built.mesh.volume_mm3();
        assert!((grew - 1.5 * 1.5 * 0.4).abs() < 1e-3, "1.5 × 1.5 × 0.4 = 0.9 mm³ on the face: {grew}");
    }
}
