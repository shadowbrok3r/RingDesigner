//! Sketching in the Ring viewport: a sketch drawn on the face or plane it lies on, with the ring as its underlay.
use egui::{Color32, Event, EventFilter, Id, Key, KeyboardShortcut, Modifiers, Pos2, Rect, Stroke};
use ringdesign_core::cad::edit::CadEdit;
use ringdesign_core::cad::{self, Attach, Component, Feature, Operation, Profile};
use ringdesign_core::sketch::{Region, RegionRef, Sketch, Workplane, anchor, fill};
use ringdesign_workbench::command::{DimEvent, Dimension, DimensionBar, Unit};
use ringdesign_workbench::icons::Icon;
use ringdesign_workbench::sketch_tools::{self, Escaped, Input, Outcome, Snap, SnapCache, Tool, Tools, Underlay};
use ringdesign_workbench::viewport::{Mods, Sel};

use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::command::Took;
use crate::theme;

/// Chord the sketch's curves are drawn and filled within, in screen pixels.
const CHORD_PX: f64 = 0.5;
/// How much the metal under a sketch is dimmed.
const DIM_ALPHA: u8 = 120;

/// A sketch's plane in the world: its origin, unit axes and the normal out of the face.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    n: [f64; 3],
}

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

impl Frame {
    fn new(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Option<Self> {
        let n = unit(cross(x, y))?;
        Some(Self { origin, x, y, n })
    }
    fn point(&self, uv: [f64; 2]) -> [f64; 3] {
        std::array::from_fn(|k| self.origin[k] + self.x[k] * uv[0] + self.y[k] * uv[1])
    }
    fn vector(&self, uv: [f64; 2]) -> [f64; 3] {
        std::array::from_fn(|k| self.x[k] * uv[0] + self.y[k] * uv[1])
    }
    fn local(&self, p: [f64; 3]) -> [f64; 2] {
        let d: [f64; 3] = std::array::from_fn(|k| p[k] - self.origin[k]);
        [dot(d, self.x), dot(d, self.y)]
    }
    /// Where a ray meets the plane, in the plane's own coordinates.
    fn hit(&self, origin: [f64; 3], dir: [f64; 3]) -> Option<[f64; 2]> {
        let along = dot(dir, self.n);
        if along.abs() < 1e-9 {
            return None;
        }
        let t = dot(std::array::from_fn(|k| self.origin[k] - origin[k]), self.n) / along;
        Some(self.local(std::array::from_fn(|k| origin[k] + dir[k] * t)))
    }
    fn workplane(&self) -> Workplane {
        Workplane { origin: self.origin, x: self.x, y: self.y, on_face: None }
    }
}

/// Which plane a sketch was laid on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneKind {
    /// A planar face of a part, moving with it.
    Face,
    /// Square to the band's surface at a point: x round the ring, y along the finger, the normal out.
    Tangent { theta_deg: f64, across_mm: f64 },
    /// Through the finger's axis at an angle: x out from the axis, y along the finger.
    Section { theta_deg: f64, across_mm: f64 },
    /// The sketch's own workplane.
    Own,
}

/// A solid being made from the sketch's regions.
#[derive(Clone, Debug, PartialEq)]
enum SolidStep {
    Extrude,
    /// Revolve: the next click picks the axis.
    PickAxis,
    /// Revolve about the line through `pivot` along `dir`, in the plane's coordinates.
    Revolve { pivot: [f64; 2], dir: [f64; 2], axis: String },
}

/// One region of the sketch, ready to fill.
struct RegionView {
    region: Region,
    triangles: Vec<[[f64; 2]; 3]>,
    polygons: Vec<Vec<[f64; 2]>>,
}

/// A sketch being drawn.
struct Live {
    feature: u64,
    kind: PlaneKind,
    /// Where the camera centres, in the plane's coordinates.
    centre: [f64; 2],
    /// The sketch as the document holds it, and as it is being drawn.
    base: Sketch,
    working: Sketch,
    tools: Tools,
    frame: Option<Frame>,
    /// Why the plane could not be read, in the core's words.
    error: Option<String>,
    /// The build the frame and underlay were read from.
    build: usize,
    /// The face the sketch lies on, as world loops.
    face: Vec<Vec<[f64; 3]>>,
    under: Underlay,
    /// Snap candidates and regions for the working sketch at a version.
    snaps: Option<(u64, SnapCache)>,
    regions: Option<(u64, Vec<RegionView>, Option<String>)>,
    bumps: u64,
    bar: DimensionBar,
    pointer: Option<(Pos2, [f64; 2], Option<Snap>)>,
    anchor: Option<Pos2>,
    hovered: Option<usize>,
    /// The region under the last right-click, and where on the plane the click landed.
    menu_region: Option<usize>,
    menu_at: Option<[f64; 2]>,
    /// The one region the solid being set up sweeps; `None` sweeps every region.
    region_pick: Option<RegionRef>,
    solid: Option<SolidStep>,
    solid_typed: Vec<(&'static str, f64)>,
    /// How the solid being set up meets the ring: joined, cut into the metal, or standing apart.
    attach: Attach,
    /// Why every region at once would not sweep as one solid, read with the regions.
    apart: Option<(u64, Option<String>)>,
    asking: bool,
    dragging: Option<u64>,
    /// How far a pick reaches at the last pointer sample, in millimetres.
    reach: f64,
    /// The viewport holding the keys and the context it lives in.
    host: Option<(Id, egui::Context)>,
}

impl Live {
    fn version(&self) -> u64 {
        self.tools.version() + self.bumps
    }
    fn typed(&self, key: &str) -> Option<f64> {
        self.solid_typed.iter().rev().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
    /// Whether the solid being set up cuts into the metal.
    fn cutting(&self) -> bool {
        self.attach == Attach::Cut
    }
    /// The fields a dimension bar shows: the solid's while one is being made, else the tool's.
    fn dimensions(&self) -> Vec<Dimension> {
        let field = |key: &'static str, label: &'static str, unit: Unit, default: f64| {
            let typed = self.typed(key);
            Dimension { key, label, unit, value: typed.unwrap_or(default), locked: typed.is_some() }
        };
        match &self.solid {
            Some(SolidStep::Extrude) => vec![field("height", if self.cutting() { "Depth" } else { "Height" }, Unit::Mm, 1.0), field("draft", "Draft", Unit::Deg, 0.0)],
            Some(SolidStep::Revolve { .. }) => vec![field("angle", "Angle", Unit::Deg, 360.0)],
            Some(SolidStep::PickAxis) => Vec::new(),
            None => self.tools.dimensions(&self.working),
        }
    }
    fn prompt(&self) -> String {
        let what = if self.region_pick.is_some() { " this region" } else { "" };
        match &self.solid {
            Some(SolidStep::Extrude) if self.cutting() => format!("Cut{what}: type the depth into the metal and the draft; Enter makes it, J joins, cuts or sets it apart"),
            Some(SolidStep::Extrude) => format!("Extrude{what}: type the height and draft; Enter makes it, J joins, cuts or sets it apart"),
            Some(SolidStep::PickAxis) => format!("Revolve{what}: click a line of the sketch, or one of its axes, to turn about"),
            Some(SolidStep::Revolve { axis, .. }) if self.cutting() => format!("Cut{what} by revolving about {axis}: type the angle into the metal; Enter makes it"),
            Some(SolidStep::Revolve { axis, .. }) => format!("Revolve{what} about {axis}: type the angle; Enter makes it, J joins, cuts or sets it apart"),
            None => self.tools.prompt(),
        }
    }
    /// Why every region at once would not sweep as one solid, at the version drawn; `None` when it would.
    fn apart(&self) -> Option<&str> {
        self.apart.as_ref().filter(|(v, _)| *v == self.version()).and_then(|(_, why)| why.as_deref())
    }
    /// Which of the regions the solid sweeps alone, when one is picked.
    fn picked(&self) -> Option<usize> {
        let views = &self.regions.as_ref()?.1;
        let regions: Vec<Region> = views.iter().map(|v| v.region.clone()).collect();
        self.region_pick?.position(&regions).map(|(i, _)| i)
    }
    /// The profile the solid being set up sweeps: the picked region, else every region.
    fn profile(&self) -> Profile {
        match self.region_pick {
            Some(region) => Profile::Region { feature: self.feature, region },
            None => Profile::Feature { feature: self.feature },
        }
    }
    /// Whether the feature holds a sketch of its own that other features sweep.
    fn is_sketch_feature(&self, app: &RingDesignerApp) -> bool {
        app.design.cad.as_ref().and_then(|d| d.feature(self.feature)).is_some_and(|f| matches!(f.operation, Operation::Sketch { .. }))
    }
    fn chord(&self, px_per_mm: f64) -> f64 {
        (CHORD_PX / px_per_mm.max(1e-6)).clamp(1e-3, 0.2)
    }
}

/// What the Ring viewport's sketch mode holds between frames.
#[derive(Default)]
pub struct SketchMode {
    live: Option<Box<Live>>,
}

impl SketchMode {
    /// Whether a sketch is live.
    pub fn is_live(&self) -> bool {
        self.live.is_some()
    }
}

#[cfg(test)]
impl SketchMode {
    /// The feature being sketched.
    pub fn feature(&self) -> Option<u64> {
        self.live.as_ref().map(|l| l.feature)
    }
    /// The sketch as it is being drawn.
    pub fn working(&self) -> Option<&Sketch> {
        self.live.as_ref().map(|l| &l.working)
    }
    /// The tool the sketch is drawn with.
    pub fn tool(&self) -> Option<Tool> {
        self.live.as_ref().map(|l| l.tools.tool)
    }
    /// The plane's normal out of the face, once read.
    pub fn normal(&self) -> Option<[f64; 3]> {
        self.live.as_ref()?.frame.map(|f| f.n)
    }
    /// A point of the plane in the world.
    pub fn world(&self, uv: [f64; 2]) -> Option<[f64; 3]> {
        self.live.as_ref()?.frame.map(|f| f.point(uv))
    }
    /// Whether leaving waits on the question about unfinished changes.
    pub fn asking(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.asking)
    }
    pub fn kind(&self) -> Option<PlaneKind> {
        self.live.as_ref().map(|l| l.kind)
    }
    /// How long the last dimension's solve took.
    pub fn last_solve_ms(&self) -> Option<f64> {
        self.live.as_ref()?.tools.last_solve_ms
    }
    /// Replaces what is being drawn, as though drawn.
    pub fn set_working(&mut self, s: Sketch) {
        if let Some(l) = self.live.as_mut() {
            l.working = s;
            l.bumps += 1;
        }
    }
}

/// Takes back the live sketch's own last edit, or backs a busy tool out; false when no sketch is live.
pub fn undo(app: &mut RingDesignerApp) -> bool {
    let nothing = {
        let Some(live) = app.sketch.live.as_deref_mut() else { return false };
        if live.tools.busy() {
            live.tools.escape(&live.working);
            false
        } else {
            !live.tools.undo(&mut live.working)
        }
    };
    if nothing {
        app.set_status("Nothing to undo in this sketch; finish or leave it to undo the document");
    }
    true
}

/// Puts back the live sketch's last undone edit; false when no sketch is live.
pub fn redo(app: &mut RingDesignerApp) -> bool {
    let nothing = {
        let Some(live) = app.sketch.live.as_deref_mut() else { return false };
        !live.tools.redo(&mut live.working)
    };
    if nothing {
        app.set_status("Nothing to redo in this sketch");
    }
    true
}

/// Whether a sketch is being drawn in the Ring viewport.
pub fn active(app: &RingDesignerApp) -> bool {
    app.sketch.is_live()
}

/// Refuses a second sketch while one is live.
fn busy(app: &mut RingDesignerApp) -> bool {
    if app.sketch.is_live() {
        app.set_status("Finish or leave the sketch being drawn first");
        return true;
    }
    false
}

/// Starts a sketch on a planar face of a part.
pub fn start_on_face(app: &mut RingDesignerApp, pane: usize, feature: u64, face: u32) {
    if busy(app) {
        return;
    }
    let Some(build) = app.build.clone() else {
        app.set_status("The ring has not built yet; sketch once it has");
        return;
    };
    let Some(c) = build.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == feature)) else {
        app.set_status(format!("Feature #{feature} is not a part on the ring to sketch on"));
        return;
    };
    let anchor = match anchor::on_face(&c.frame, feature, &c.body, face as usize) {
        Ok(a) => a,
        Err(e) => {
            app.set_status(format!("{e:#}"));
            return;
        }
    };
    let sketch = Sketch { name: "Sketch".into(), plane: Workplane { on_face: Some(anchor), ..Workplane::default() }, ..Sketch::default() };
    add_and_enter(app, pane, sketch, PlaneKind::Face, [0.0; 2]);
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

/// Starts a sketch on a plane through a point of the band.
pub fn start_on_plane(app: &mut RingDesignerApp, pane: usize, theta_deg: f64, across_mm: f64) {
    if busy(app) {
        return;
    }
    let Some(build) = app.build.clone() else {
        app.set_status("The ring has not built yet; sketch once it has");
        return;
    };
    let Some((hit, normal)) = cad::surface_hit(&build.mesh, theta_deg, across_mm) else {
        app.set_status("No band under that point to sketch on");
        return;
    };
    let Some(plane) = tangent_plane(hit, normal, theta_deg) else {
        app.set_status("The band has no surface direction there to sketch on");
        return;
    };
    let sketch = Sketch { name: "Sketch".into(), plane, ..Sketch::default() };
    add_and_enter(app, pane, sketch, PlaneKind::Tangent { theta_deg, across_mm }, [0.0; 2]);
}

/// Starts sketching on a feature that already carries a sketch.
pub fn start_on_feature(app: &mut RingDesignerApp, pane: usize, feature: u64) -> bool {
    if busy(app) {
        return false;
    }
    let Some(f) = app.design.cad.as_ref().and_then(|d| d.feature(feature)) else {
        app.set_status(format!("Feature #{feature} is not in the document; apply it first"));
        return false;
    };
    let mut operation = f.operation.clone();
    let Some(sketch) = operation.sketch_mut().cloned() else {
        app.set_status(format!("{} has no sketch to draw", f.name));
        return false;
    };
    let kind = if sketch.plane.on_face.is_some() { PlaneKind::Face } else { PlaneKind::Own };
    enter(app, pane, feature, sketch, kind, [0.0; 2]);
    true
}

/// Starts sketching `feature` in a Ring viewport on screen, bringing one up when none is.
pub fn start_in_ring(app: &mut RingDesignerApp, feature: u64) -> bool {
    let ring = |app: &RingDesignerApp, i: usize| app.panes.get(i).is_some_and(|p| p.kind == crate::pane::PaneKind::Solid && !p.follow_node);
    let shown = app.visible_panes();
    let pane = match shown.iter().copied().find(|i| *i == app.active_pane && ring(app, *i)).or_else(|| shown.iter().copied().find(|i| ring(app, *i))) {
        Some(i) => i,
        None => {
            app.focus(crate::pane::PaneKind::Solid);
            app.active_pane
        }
    };
    start_on_feature(app, pane, feature)
}

/// Adds a Sketch feature through the funnel and starts drawing it.
fn add_and_enter(app: &mut RingDesignerApp, pane: usize, sketch: Sketch, kind: PlaneKind, centre: [f64; 2]) {
    crate::command::cancel(app);
    let feature = Feature { id: 0, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch: sketch.clone() }, component: Component::default() };
    let Ok(applied) = crate::cad_edit::apply(app, &[CadEdit::Add { feature, after: None }]) else { return };
    let Some(id) = applied.first().and_then(|a| a.id) else { return };
    enter(app, pane, id, sketch, kind, centre);
}

fn enter(app: &mut RingDesignerApp, pane: usize, feature: u64, sketch: Sketch, kind: PlaneKind, centre: [f64; 2]) {
    crate::command::cancel(app);
    app.sketch.live = Some(Box::new(Live {
        feature,
        kind,
        centre,
        base: sketch.clone(),
        working: sketch,
        tools: Tools::default(),
        frame: None,
        error: None,
        build: 0,
        face: Vec::new(),
        under: Underlay::default(),
        snaps: None,
        regions: None,
        bumps: 0,
        bar: DimensionBar::new(("sketch-dimensions", pane)),
        pointer: None,
        anchor: None,
        hovered: None,
        menu_region: None,
        menu_at: None,
        region_pick: None,
        solid: None,
        solid_typed: Vec::new(),
        attach: Attach::Join,
        apart: None,
        asking: false,
        dragging: None,
        reach: 0.2,
        host: None,
    }));
    app.active_pane = pane;
    if app.visual.tool != ringdesign_workbench::visual::Tool::Select {
        app.visual.select(ringdesign_workbench::visual::Tool::Select);
    }
    refresh_frame(app);
    look_at(app, pane);
    let words = app.sketch.live.as_ref().map(|l| l.error.clone().unwrap_or_else(|| format!("Sketching · {}", l.prompt()))).unwrap_or_default();
    app.set_status(words);
}

/// Reads the sketch's plane and what it is drawn over from the build on screen, when that build is new.
fn refresh_frame(app: &mut RingDesignerApp) {
    let key = app.build.as_ref().map_or(0, |b| std::sync::Arc::as_ptr(b) as usize);
    let Some(live) = app.sketch.live.as_deref() else { return };
    if live.frame.is_some() && live.build == key {
        return;
    }
    let resolved = resolve(app, &live.working);
    let cut = |frame: &Frame| -> Vec<[[f64; 2]; 2]> {
        let (Some(build), Ok(plane)) = (app.build.as_ref(), frame.workplane().plane()) else { return Vec::new() };
        fill::slice(&build.mesh, &plane)
    };
    let (outcome, cut) = match resolved {
        Ok((frame, face)) => {
            let c = cut(&frame);
            (Ok((frame, face)), c)
        }
        Err(e) => (Err(e), Vec::new()),
    };
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    live.build = key;
    match outcome {
        Ok((frame, face)) => {
            live.under = Underlay { face: face.iter().map(|l| l.iter().map(|p| frame.local(*p)).collect()).collect(), cut };
            live.face = face;
            live.frame = Some(frame);
            live.error = None;
            live.snaps = None;
        }
        Err(e) => live.error = Some(e),
    }
}

/// The world plane `sketch` lies on and the face it is drawn over, read off the build on screen.
fn resolve(app: &RingDesignerApp, sketch: &Sketch) -> Result<(Frame, Vec<Vec<[f64; 3]>>), String> {
    let Some(anchor) = &sketch.plane.on_face else {
        let p = sketch.plane.plane().map_err(|e| format!("{e:#}"))?;
        return Frame::new(p.origin, p.x_axis, p.y_axis).map(|f| (f, Vec::new())).ok_or_else(|| "The sketch's plane has no normal".to_string());
    };
    let build = app.build.as_ref().ok_or("The ring has not built yet")?;
    if let Some(plane) = build.parts.evaluated.as_ref().and_then(|e| e.plane(anchor.feature)) {
        let p = ringdesign_core::cad::pattern::sketch_on_work_plane(sketch, plane).map_err(|e| format!("{e:#}"))?;
        let frame = Frame::new(p.origin, p.x_axis, p.y_axis).ok_or("The work plane has no normal")?;
        return Ok((frame, Vec::new()));
    }
    let c = build
        .parts
        .evaluated
        .as_ref()
        .and_then(|e| e.components.iter().find(|c| c.id == anchor.feature))
        .ok_or_else(|| format!("Sketch face: feature #{} is not a part on the ring", anchor.feature))?;
    let p = anchor::plane(sketch, &c.body, &c.frame, &mut Vec::new()).map_err(|e| format!("{e:#}"))?;
    let face = fill::face_outline(&c.body, &p, 0.01).unwrap_or_default();
    let frame = Frame::new(p.origin, p.x_axis, p.y_axis).ok_or("The face's plane has no normal")?;
    Ok((frame, face))
}

/// Eases the pane's camera to look straight at the sketch's plane, its y up the screen and its centre in the middle.
fn look_at(app: &mut RingDesignerApp, pane: usize) {
    let Some(live) = app.sketch.live.as_deref() else { return };
    let Some(f) = live.frame else { return };
    let Some(p) = app.panes.get(pane) else { return };
    let cam = p.camera;
    let n = f.n.map(|v| v as f32);
    let pitch = n[2].clamp(-1.0, 1.0).asin();
    let yaw = n[1].atan2(n[0]);
    let (s0, u0) = ringdesign_workbench::focus::view_axes(yaw, pitch, 0.0);
    let y = f.y.map(|v| v as f32);
    let d3 = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let roll = (-d3(y, s0)).atan2(d3(y, u0));
    let (s, u) = ringdesign_workbench::focus::view_axes(yaw, pitch, roll);
    let centre = f.point(live.centre).map(|v| v as f32);
    let off: [f32; 3] = std::array::from_fn(|k| centre[k] - cam.target[k]);
    let reach = live
        .face
        .iter()
        .flatten()
        .map(|w| f.local(*w))
        .chain(live.working.points.iter().map(|p| p.xy))
        .map(|uv| ringdesign_core::sketch::distance(uv, live.centre))
        .fold(3.0_f64, f64::max);
    let zoom = (cam.half_extent() * cam.zoom / (reach as f32 * 1.6)).clamp(0.15, 24.0);
    let to = ringdesign_workbench::focus::Pose { yaw, pitch, roll, zoom, pan: [d3(off, s), d3(off, u)] };
    app.panes[pane].turn = Some(ringdesign_workbench::focus::Turn::new(cam.pose(), to));
}

/// The sketch an operation draws in itself, as `Operation::sketch_mut` finds it.
fn sketch_of(op: &Operation) -> Option<&Sketch> {
    match op {
        Operation::Sketch { sketch } => Some(sketch),
        Operation::Extrude { sketch, .. } | Operation::Revolve { sketch, .. } | Operation::Sweep { sketch, .. } | Operation::Twist { sketch, .. } => match sketch {
            Profile::Inline(s) => Some(s),
            Profile::Feature { .. } | Profile::Region { .. } => None,
        },
        Operation::Loft { sections } => sections.first().and_then(|p| match p {
            Profile::Inline(s) => Some(s),
            Profile::Feature { .. } | Profile::Region { .. } => None,
        }),
        _ => None,
    }
}

/// Keeps the sketch in step with the document: gone means sketch mode ends, changed underneath means adopting it.
fn follow(app: &mut RingDesignerApp) -> bool {
    let Some(live) = app.sketch.live.as_deref() else { return false };
    let doc = app.design.cad.as_ref().and_then(|d| d.feature(live.feature)).and_then(|f| sketch_of(&f.operation));
    let Some(doc) = doc else {
        leave(app, "The sketch left the document; sketch mode ended");
        return false;
    };
    if *doc == live.base {
        return true;
    }
    let doc = doc.clone();
    let Some(live) = app.sketch.live.as_deref_mut() else { return false };
    if live.working == live.base {
        live.working = doc.clone();
        live.tools.reset();
    }
    if doc.plane != live.base.plane {
        live.frame = None;
    }
    live.base = doc;
    live.bumps += 1;
    true
}

/// Ends sketch mode and lets go of the keys.
fn leave(app: &mut RingDesignerApp, words: &str) {
    if let Some(live) = app.sketch.live.take()
        && let Some((id, ctx)) = &live.host
    {
        ctx.data_mut(|d| d.remove::<Host>(host_key()));
        if ctx.memory(|m| m.focused()) == Some(*id) {
            ctx.memory_mut(|m| m.surrender_focus(*id));
        }
    }
    app.set_status(words);
}

/// The feature's operation with the working sketch in it.
fn with_working(app: &RingDesignerApp) -> Option<(u64, Operation)> {
    let live = app.sketch.live.as_deref()?;
    let mut operation = app.design.cad.as_ref()?.feature(live.feature)?.operation.clone();
    *operation.sketch_mut()? = live.working.clone();
    Some((live.feature, operation))
}

/// Commits the working sketch through the funnel as one edit, then leaves; whether it landed.
pub fn finish(app: &mut RingDesignerApp) -> bool {
    let Some(live) = app.sketch.live.as_deref() else { return false };
    if live.working == live.base {
        leave(app, "Sketch finished; nothing had changed");
        return true;
    }
    let Some((id, operation)) = with_working(app) else {
        leave(app, "The sketch left the document; sketch mode ended");
        return false;
    };
    if crate::cad_edit::apply(app, &[CadEdit::Operation { id, operation }]).is_err() {
        return false;
    }
    leave(app, "Sketch finished");
    true
}

/// The escape ladder's last rung: leaves, or asks first when strokes are not finished.
fn leave_or_ask(app: &mut RingDesignerApp) {
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    if live.working != live.base {
        live.asking = true;
        app.set_status("This sketch has unfinished changes: finish it, discard them, or keep drawing");
        return;
    }
    leave(app, "Left the sketch");
}

/// Escape: the solid step, then the tool's ladder, then the sketch itself.
fn escape(app: &mut RingDesignerApp) {
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    if live.asking {
        live.asking = false;
        app.set_status("Keep drawing");
        return;
    }
    if live.solid.take().is_some() {
        live.solid_typed.clear();
        live.region_pick = None;
        let prompt = live.prompt();
        app.set_status(prompt);
        return;
    }
    match live.tools.escape(&live.working) {
        Escaped::Out => leave_or_ask(app),
        _ => {
            let prompt = live.prompt();
            app.set_status(prompt);
        }
    }
}

/// Says what a tool's token did.
fn report(app: &mut RingDesignerApp, out: Outcome) {
    match out {
        Outcome::Edited(words) | Outcome::Refused(words) => app.set_status(words),
        Outcome::Continue => {}
    }
}

fn set_tool(app: &mut RingDesignerApp, tool: Tool) {
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    live.solid = None;
    live.tools.set_tool(tool);
    live.bar.reset();
    let prompt = live.prompt();
    app.set_status(prompt);
}

/// Adds a shank when the document has none but its parts stand on the band, and says how a new body attaches.
fn shank_for_body(app: &RingDesignerApp) -> (Vec<CadEdit>, Attach) {
    let Some(doc) = app.design.cad.as_ref() else { return (Vec::new(), Attach::Separate) };
    if doc.band().is_some() {
        return (Vec::new(), Attach::Join);
    }
    if doc.replaces_band() {
        return (Vec::new(), Attach::Separate);
    }
    // A document of sketches alone gains the procedural shank before its first body.
    let shank = Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() };
    (vec![CadEdit::Add { feature: shank, after: None }], Attach::Join)
}

/// Why a ring of parts alone takes no cut.
const ALL_PARTS: &str = "The ring is all parts: a cut has no band to carve";

/// Makes the extrusion or revolution being set up, with any unfinished strokes, as one edit: a cut
/// extrusion runs from the plane down into the metal, and a cut revolution turns into it.
fn commit_solid(app: &mut RingDesignerApp) {
    let Some(live) = app.sketch.live.as_deref() else { return };
    let Some(frame) = live.frame else { return };
    if live.region_pick.is_none()
        && let Some(why) = live.apart()
    {
        let words = format!("{why}; right-click one region to make it alone");
        app.set_status(words);
        return;
    }
    let cut = live.cutting();
    let operation = match &live.solid {
        Some(SolidStep::Extrude) => {
            let height = live.typed("height").unwrap_or(1.0);
            let draft_deg = live.typed("draft").unwrap_or(0.0);
            Operation::Extrude { sketch: live.profile(), height_mm: if cut { -height } else { height }, draft_deg }
        }
        Some(SolidStep::Revolve { pivot, dir, .. }) => {
            let axis = frame.vector(*dir);
            Operation::Revolve {
                sketch: live.profile(),
                pivot: frame.point(*pivot),
                axis: if cut { axis.map(|v| -v) } else { axis },
                degrees: live.typed("angle").unwrap_or(360.0),
            }
        }
        _ => return,
    };
    let chosen = live.attach;
    let dirty = live.working != live.base;
    let (shank, fallback) = shank_for_body(app);
    // A ring of parts alone has no band to join to or carve.
    let attach = match (fallback, chosen) {
        (Attach::Separate, Attach::Cut) => {
            app.set_status(ALL_PARTS);
            return;
        }
        (Attach::Separate, _) => Attach::Separate,
        (_, chosen) => chosen,
    };
    let label = if attach == Attach::Cut { format!("{} cut", operation.label()) } else { operation.label().to_string() };
    let mut edits = Vec::new();
    if dirty && let Some((id, sketch)) = with_working(app) {
        edits.push(CadEdit::Operation { id, operation: sketch });
    }
    edits.extend(shank);
    edits.push(CadEdit::Add { feature: Feature { id: 0, name: label.clone(), enabled: true, operation, component: Component { attach, ..Component::default() } }, after: None });
    let Ok(applied) = crate::cad_edit::apply(app, &edits) else { return };
    if let Some(id) = applied.last().and_then(|a| a.id) {
        app.selection.click(Some(Sel::Part(id)), Mods::default());
    }
    leave(app, &format!("{label} made from the sketch"));
}

/// The axis a revolve turns about: a line of the sketch under the pointer, else the plane's own axis
/// nearest it; directed so a positive turn swings the region it sweeps out of the plane toward its normal.
fn pick_axis(live: &Live) -> Option<([f64; 2], [f64; 2], String)> {
    let (_, raw, _) = live.pointer?;
    let reach = live.reach;
    let mut picked = None;
    if let Some(near) = live.working.nearest_entity(raw, reach) {
        let line = live.working.polylines(near.entity, 0.01).into_iter().flatten().collect::<Vec<_>>();
        if let [a, .., b] = line.as_slice() {
            let d = [b[0] - a[0], b[1] - a[1]];
            let l = d[0].hypot(d[1]);
            if l > 1e-9 && line.len() == 2 {
                picked = Some((*a, [d[0] / l, d[1] / l], format!("line #{}", near.entity)));
            }
        }
    }
    let (to_y, to_x) = (raw[0].abs(), raw[1].abs());
    if picked.is_none() && to_y.min(to_x) <= reach * 3.0 {
        picked = Some(if to_y <= to_x { ([0.0; 2], [0.0, 1.0], "the sketch's y axis".into()) } else { ([0.0; 2], [1.0, 0.0], "the sketch's x axis".into()) });
    }
    let (pivot, dir, axis) = picked?;
    let views = live.regions.as_ref().map(|(_, v, _)| v.as_slice()).unwrap_or_default();
    let swept = live.picked().and_then(|i| views.get(i)).or_else(|| views.iter().max_by(|a, b| a.region.area().total_cmp(&b.region.area())));
    let w = swept.and_then(|v| v.region.inside()).map_or([0.0; 2], |p| [p[0] - pivot[0], p[1] - pivot[1]]);
    let dir = if dir[0] * w[1] - dir[1] * w[0] < 0.0 { [-dir[0], -dir[1]] } else { dir };
    Some((pivot, dir, axis))
}

/// What the plugin claimed for the sketch before the app's own shortcuts read the pass's keys.
#[derive(Clone, Copy, Debug, Default)]
struct Claimed {
    delete: bool,
    undo: bool,
    redo: bool,
}

/// The viewport that held the keys for a sketch, and the pass it last did.
#[derive(Clone, Copy, Debug)]
struct Host {
    id: Id,
    pass: u64,
}

fn host_key() -> Id {
    Id::new("sketch-mode-host")
}
fn claim_key() -> Id {
    Id::new("sketch-mode-claimed")
}

const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

/// Takes Delete and the undo keys for a live sketch before the host's own shortcuts read the pass's keys.
struct SketchKeys;

impl egui::Plugin for SketchKeys {
    fn debug_name(&self) -> &'static str {
        "sketch-mode-keys"
    }
    fn on_begin_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(host) = ctx.data(|d| d.get_temp::<Host>(host_key())) else { return };
        if host.pass + 1 != ctx.cumulative_pass_nr() {
            return;
        }
        if ctx.memory(|m| m.focused()).is_some_and(|f| f != host.id) || egui::Popup::is_any_open(&ctx) {
            return;
        }
        let claimed = ctx.input_mut(|i| Claimed {
            redo: i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT),
            undo: i.consume_shortcut(&UNDO),
            delete: i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace),
        });
        ctx.data_mut(|d| d.insert_temp(claim_key(), claimed));
    }
}

/// Takes the first press of `key` with exactly `shift` held and no other modifier.
fn take(ui: &egui::Ui, key: Key, shift: bool) -> bool {
    ui.input_mut(|i| {
        let at = i.events.iter().position(|e| {
            matches!(e, Event::Key { key: k, pressed: true, repeat: false, modifiers, .. } if *k == key && modifiers.shift == shift && !modifiers.alt && !modifiers.ctrl && !modifiers.command && !modifiers.mac_cmd)
        });
        at.map(|at| i.events.remove(at)).is_some()
    })
}

/// Keeps the keys on the viewport while a sketch is live, a dimension field aside.
fn hold(live: &Live, ctx: &egui::Context, id: Id) {
    if live.bar.has_focus(ctx) {
        return;
    }
    if ctx.memory(|m| m.focused()).is_none() {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    ctx.memory_mut(|m| m.set_focus_lock_filter(id, EventFilter { tab: true, escape: true, horizontal_arrows: true, vertical_arrows: true }));
}

/// The grid a pointer snaps to at this scale: the sketch's own, doubled until its lines stand 8 px apart.
fn grid_mm(s: &Sketch, px_per_mm: f64) -> f64 {
    let mut g = if s.grid_mm.is_finite() && s.grid_mm > 0.0 { s.grid_mm } else { 0.5 };
    let mut guard = 0;
    while g * px_per_mm < 8.0 && guard < 40 {
        g *= 2.0;
        guard += 1;
    }
    g
}

/// The working sketch's snap candidates and regions, recomputed when it changed.
fn caches(live: &mut Live, chord: f64) {
    let version = live.version();
    if live.snaps.as_ref().is_none_or(|(v, _)| *v != version) {
        live.snaps = Some((version, SnapCache::of(&live.working, &live.under)));
    }
    if live.regions.as_ref().is_none_or(|(v, ..)| *v != version) {
        let (views, error) = match live.working.profile_regions() {
            Ok(regions) => (
                regions
                    .into_iter()
                    .map(|region| RegionView { triangles: region.triangles(chord), polygons: region.polygons(chord), region })
                    .collect(),
                None,
            ),
            Err(e) => (Vec::new(), Some(format!("{e:#}"))),
        };
        let apart = if views.len() > 1 { live.working.sweep_regions().err().map(|e| format!("{e:#}")) } else { None };
        live.regions = Some((version, views, error));
        live.apart = Some((version, apart));
    }
}

/// Reads the pointer at `pos` onto the plane, snapped unless Ctrl frees it, and feeds it to the tool.
fn sample(app: &mut RingDesignerApp, pane: usize, rect: Rect, pos: Pos2, free: bool) -> Option<[f64; 2]> {
    let camera = app.panes.get(pane)?.camera;
    let live = app.sketch.live.as_deref_mut()?;
    let frame = live.frame?;
    let (o, d) = camera.ray(rect, pos);
    let raw = frame.hit(o.map(f64::from), d.map(f64::from))?;
    let (view, _) = ringdesign_workbench::hover::view_scale(pos, &|p| camera.ray(rect, p));
    let px = view.px_per_mm.max(1e-6);
    let reach = f64::from(crate::viewport::APERTURE_PX) / px;
    live.reach = reach;
    let chord = live.chord(px);
    caches(live, chord);
    let grid = grid_mm(&live.working, px);
    let snapped = if free {
        None
    } else if live.dragging.is_some() {
        // A dragged point snaps only to what the drag cannot move.
        let still = SnapCache { corners: live.under.corners(), ..SnapCache::default() };
        sketch_tools::snap(&Sketch::default(), &live.under, &still, raw, reach, Some(grid))
    } else {
        live.snaps.as_ref().and_then(|(_, c)| sketch_tools::snap(&live.working, &live.under, c, raw, reach, Some(grid)))
    };
    let out = live.tools.feed(&mut live.working, Input::Pointer { raw, snapped, reach });
    live.pointer = Some((pos, raw, snapped));
    live.hovered = live.regions.as_ref().and_then(|(_, views, _)| views.iter().position(|v| v.region.contains(raw)));
    report(app, out);
    Some(raw)
}

/// The sketch's keys while the viewport holds them.
fn keys(app: &mut RingDesignerApp, ui: &egui::Ui) {
    let Some(live) = app.sketch.live.as_deref() else { return };
    if live.asking {
        if take(ui, Key::Escape, false) {
            escape(app);
        }
        return;
    }
    if take(ui, Key::Escape, false) {
        escape(app);
    }
    if take(ui, Key::Enter, false) {
        let Some(live) = app.sketch.live.as_deref_mut() else { return };
        if matches!(live.solid, Some(SolidStep::Extrude | SolidStep::Revolve { .. })) {
            commit_solid(app);
        } else {
            let out = live.tools.feed(&mut live.working, Input::Confirm);
            report(app, out);
        }
    }
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    let dims = live.dimensions();
    if let (Some(first), Some(last)) = (dims.first(), dims.last()) {
        if take(ui, Key::Tab, false) {
            live.bar.focus_field(ui.ctx(), first.key);
        } else if take(ui, Key::Tab, true) {
            live.bar.focus_field(ui.ctx(), last.key);
        }
    }
    if live.solid.is_some() && take(ui, Key::J, false) {
        let next = match live.attach {
            Attach::Join => Attach::Cut,
            Attach::Cut => Attach::Separate,
            Attach::Separate => Attach::Join,
        };
        set_attach(app, next);
        return;
    }
    if take(ui, Key::X, false) {
        match live.tools.toggle_construction(&mut live.working) {
            Outcome::Continue => {
                let words = if live.tools.construction { "New curves are drawn as construction" } else { "New curves are drawn as profile" };
                app.set_status(words);
            }
            out => report(app, out),
        }
    }
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    // C closes a line being drawn, else picks the circle.
    if live.tools.tool == Tool::Line && live.tools.busy() && take(ui, Key::C, false) {
        let out = live.tools.feed(&mut live.working, Input::Close);
        report(app, out);
        return;
    }
    for tool in Tool::ALL {
        if let Some(key) = tool.key()
            && take(ui, key, false)
        {
            set_tool(app, tool);
        }
    }
}

/// Takes the pointer and keys while a sketch is live, before the command layer sees them.
pub fn input(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, rect: Rect, response: &egui::Response) -> Took {
    let mut took = Took { live: true, ..Took::default() };
    let ctx = ui.ctx().clone();
    ctx.add_plugin(SketchKeys);
    if crate::command::cancel(app) {
        app.set_status("Finish the sketch before starting a command");
    }
    if !follow(app) {
        return took;
    }
    refresh_frame(app);
    let id = response.id;
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(host_key(), Host { id, pass }));
    let claimed = ctx.data_mut(|d| d.remove_temp::<Claimed>(claim_key())).unwrap_or_default();
    let Some(live) = app.sketch.live.as_deref_mut() else { return took };
    live.host = Some((id, ctx.clone()));
    live.bar.set_host(Some(id));
    hold(live, &ctx, id);
    let hover = ui.input(|i| (!i.pointer.any_down()).then(|| i.pointer.hover_pos()).flatten()).filter(|p| rect.contains(*p) && response.hovered());
    if !live.bar.has_focus(&ctx)
        && let Some(p) = hover
    {
        live.anchor = Some(p);
    }
    if live.asking {
        took.click = response.clicked();
        took.drag = response.dragged();
        took.secondary = response.secondary_clicked();
        keys(app, ui);
        return took;
    }
    // The bar first: a field being typed in takes its own keys.
    let mut dims = live.dimensions();
    if !dims.is_empty() {
        let anchor = live.anchor.unwrap_or(rect.center());
        for e in live.bar.show(&ctx, anchor, rect, &mut dims) {
            let Some(live) = app.sketch.live.as_deref_mut() else { break };
            match e {
                DimEvent::Typed { key, value } if live.solid.is_some() => {
                    live.solid_typed.retain(|(k, _)| *k != key);
                    live.solid_typed.push((key, value));
                }
                DimEvent::Cleared { key } if live.solid.is_some() => live.solid_typed.retain(|(k, _)| *k != key),
                DimEvent::Typed { key, value } => {
                    live.tools.feed(&mut live.working, Input::Typed { key, value });
                }
                DimEvent::Cleared { key } => {
                    live.tools.feed(&mut live.working, Input::Cleared { key });
                }
                DimEvent::Confirm if live.solid.is_some() => commit_solid(app),
                DimEvent::Confirm => {
                    let out = live.tools.feed(&mut live.working, Input::Confirm);
                    report(app, out);
                }
                DimEvent::Escape => escape(app),
                DimEvent::Focused { .. } => {}
            }
        }
    }
    if !app.sketch.is_live() {
        return took;
    }
    let owns_keys = ctx.memory(|m| m.focused()).is_none_or(|f| f == id) && !egui::Popup::is_any_open(&ctx);
    if owns_keys {
        keys(app, ui);
    }
    if claimed.delete
        && let Some(live) = app.sketch.live.as_deref_mut()
    {
        let out = live.tools.delete_chosen(&mut live.working);
        report(app, out);
    }
    if claimed.undo {
        undo(app);
    } else if claimed.redo {
        redo(app);
    }
    if !app.sketch.is_live() {
        return took;
    }
    // The pointer, under the dimension bar too, then a click where it lands; Ctrl frees the snap.
    let free = ui.input(|i| i.modifiers.command);
    let under_bar = |p: &Pos2| rect.contains(*p) && app.sketch.live.as_ref().is_some_and(|l| l.bar.covers(&ctx, *p));
    let reading = hover.or_else(|| ui.input(|i| (!i.pointer.any_down()).then(|| i.pointer.hover_pos()).flatten()).filter(under_bar));
    if let Some(pos) = reading {
        let moved = app.sketch.live.as_ref().and_then(|l| l.pointer).is_none_or(|(p, ..)| p.distance(pos) > 0.25);
        if moved {
            sample(app, pane, rect, pos, free);
        }
    }
    if response.clicked() {
        took.click = true;
        if let Some(pos) = response.interact_pointer_pos() {
            sample(app, pane, rect, pos, free);
        }
        click(app, ui.input(|i| i.modifiers.shift));
    }
    // A drag in Select moves the point it starts on, Alt orbits, anything else pans.
    let alt = ui.input(|i| i.modifiers.alt);
    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(live) = app.sketch.live.as_deref_mut()
    {
        live.dragging = None;
        if live.tools.tool == Tool::Select
            && let (Some(origin), Some(f)) = (ui.input(|i| i.pointer.press_origin()), live.frame)
        {
            let camera = app.panes[pane].camera;
            let (o, d) = camera.ray(rect, origin);
            if let Some(uv) = f.hit(o.map(f64::from), d.map(f64::from)) {
                live.dragging = live.working.nearest_point(uv, live.reach.max(1e-6)).filter(|p| live.tools.begin_drag(&live.working, *p));
            }
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) && !alt {
        took.drag = true;
        let dragging = app.sketch.live.as_ref().and_then(|l| l.dragging);
        match (dragging, response.interact_pointer_pos()) {
            (Some(point), Some(pos)) => {
                if let Some(xy) = sample(app, pane, rect, pos, free)
                    && let Some(live) = app.sketch.live.as_deref_mut()
                {
                    let at = live.pointer.and_then(|(_, _, s)| s).map_or(xy, |s| s.xy);
                    live.tools.drag_to(&mut live.working, point, at);
                }
            }
            _ => app.panes[pane].camera.pan_by(response.drag_delta(), rect),
        }
    }
    if response.drag_stopped()
        && let Some(live) = app.sketch.live.as_deref_mut()
        && live.dragging.take().is_some()
    {
        live.tools.end_drag(&live.working);
    }
    if response.secondary_clicked() {
        took.secondary = true;
        if let Some(pos) = response.interact_pointer_pos() {
            sample(app, pane, rect, pos, true);
        }
        if let Some(live) = app.sketch.live.as_deref_mut() {
            live.menu_region = live.hovered;
            live.menu_at = live.pointer.map(|(_, raw, _)| raw);
        }
    }
    response.context_menu(|ui| menu(app, ui, pane));
    if let Some(live) = app.sketch.live.as_deref() {
        hold(live, &ctx, id);
    }
    took
}

/// A primary click: the axis a revolve waits on, else the tool's.
fn click(app: &mut RingDesignerApp, add: bool) {
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    match live.solid {
        Some(SolidStep::PickAxis) => {
            match pick_axis(live) {
                Some((pivot, dir, axis)) => {
                    live.solid = Some(SolidStep::Revolve { pivot, dir, axis });
                    let prompt = live.prompt();
                    app.set_status(prompt);
                }
                None => app.set_status("Click a line of the sketch, or near one of its axes"),
            }
            return;
        }
        Some(_) => {
            commit_solid(app);
            return;
        }
        None => {}
    }
    let out = live.tools.feed(&mut live.working, Input::Click { add });
    match out {
        Outcome::Continue => {
            let prompt = live.prompt();
            app.set_status(prompt);
        }
        other => report(app, other),
    }
}

/// The right-click menu inside sketch mode: the region's solids, then the sketch's own actions.
fn menu(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    ui.set_min_width(190.0);
    let Some(live) = app.sketch.live.as_deref() else { return };
    let region = live.menu_region;
    let solids = live.is_sketch_feature(app);
    // The region under the click, named by a side only it runs along and the point clicked in it.
    let regions: Vec<Region> = live.regions.as_ref().map(|(_, v, _)| v.iter().map(|v| v.region.clone()).collect()).unwrap_or_default();
    let pick = region.zip(live.menu_at).and_then(|(i, at)| RegionRef::among(&regions, i, at));
    let apart = live.apart().map(|why| format!("{why}; make one region at a time"));
    if let Some(i) = region {
        ui.weak(format!("Region {} of the sketch", i + 1));
        let why = (!solids).then_some("A sketch drawn inside a feature already makes its solid");
        let items = [
            (Icon::CadExtrude, "Extrude", SolidStep::Extrude, false, "Push the sketch's regions out along the plane's normal: type the height and draft"),
            (Icon::CadExtrude, "Extrude this region", SolidStep::Extrude, true, "Push this region alone out along the plane's normal; it stays this region as the sketch changes"),
            (Icon::CadRevolve, "Revolve…", SolidStep::PickAxis, false, "Turn the sketch's regions about a line you click next"),
            (Icon::CadRevolve, "Revolve this region…", SolidStep::PickAxis, true, "Turn this region alone about a line you click next"),
        ];
        for (icon, label, step, alone, hint) in items {
            let hint = why.or(apart.as_deref().filter(|_| !alone)).unwrap_or(hint);
            let offered = why.is_none() && if alone { pick.is_some() } else { apart.is_none() };
            if ui.add_enabled(offered, egui::Button::image_and_text(icon.image(ui, 18.0), label)).on_hover_text(hint).on_disabled_hover_text(hint).clicked() {
                begin_solid(app, step, if alone { pick } else { None });
                ui.close();
            }
        }
        ui.separator();
    }
    if ui.add(egui::Button::image_and_text(Icon::Check.image(ui, 18.0), "Finish sketch")).clicked() {
        finish(app);
        ui.close();
    }
    if ui.add(egui::Button::image_and_text(Icon::View.image(ui, 18.0), "Look at the sketch")).clicked() {
        look_at(app, pane);
        ui.close();
    }
    if ui.add(egui::Button::image_and_text(Icon::Close.image(ui, 18.0), "Leave sketch")).clicked() {
        leave_or_ask(app);
        ui.close();
    }
}

/// Starts setting up a solid from the sketch: every region, or the one `pick` names; joined to the band, or apart on a ring of parts alone.
fn begin_solid(app: &mut RingDesignerApp, step: SolidStep, pick: Option<RegionRef>) {
    let (_, attach) = shank_for_body(app);
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    live.solid = Some(step);
    live.region_pick = pick;
    live.attach = attach;
    live.solid_typed.clear();
    live.bar.reset();
    let prompt = live.prompt();
    app.set_status(prompt);
}

/// Chooses how the solid being set up meets the ring; a cut is refused on a ring that is all parts, where there is no band to carve.
fn set_attach(app: &mut RingDesignerApp, attach: Attach) {
    if attach == Attach::Cut && app.design.cad.as_ref().is_some_and(|d| d.replaces_band()) {
        app.set_status(ALL_PARTS);
        return;
    }
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    live.attach = attach;
    let words = match attach {
        Attach::Join => "Join: the solid is united with the band and what is joined to it",
        Attach::Cut => "Cut: the solid carves into the band and the part it stands on",
        Attach::Separate => "Separate: the solid stands apart as a casting of its own",
    };
    let said = format!("{words} · {}", live.prompt());
    app.set_status(said);
}

/// One toolbar button: its mark, its name to a reader and its tooltip.
fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, selected: bool, enabled: bool) -> bool {
    let button = egui::Button::image(icon.image(ui, 20.0)).selected(selected).min_size(egui::vec2(30.0, 30.0)).image_tint_follows_text_color(false);
    let r = ui.add_enabled(enabled, button);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    r.on_hover_text(tip).on_disabled_hover_text(tip).clicked()
}

/// The floating sketch toolbar at the top of the viewport.
fn toolbar(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, rect: Rect) {
    let Some(live) = app.sketch.live.as_deref() else { return };
    let (tool, construction, chosen) = (live.tools.tool, live.tools.construction, !live.tools.chosen_entities.is_empty() || !live.tools.chosen_points.is_empty());
    let (can_undo, can_redo, kind) = (live.tools.can_undo(), live.tools.can_redo(), live.kind);
    let solid = live.solid.is_some().then_some(live.attach);
    let can_cut = !app.design.cad.as_ref().is_some_and(|d| d.replaces_band());
    let mut chosen_tool = None;
    let mut action: Option<&'static str> = None;
    let left = rect.left() + crate::command::RAIL_W + 6.0;
    egui::Area::new(Id::new(("sketch-toolbar", pane)))
        .order(egui::Order::Middle)
        .fixed_pos(egui::pos2(left, rect.top() + 6.0))
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).fill(theme::FLOAT).inner_margin(4).show(ui, |ui| {
                // Width that stops short of the navigation cube.
                ui.set_max_width((rect.right() - 124.0 - left).max(160.0));
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                ui.horizontal_wrapped(|ui| {
                    for t in Tool::ALL {
                        let key = t.key().map_or(String::new(), |k| format!(" ({})", k.name()));
                        if tool_button(ui, t.icon(), &format!("{} tool", t.label()), &format!("{}{key}\n{}", t.label(), t.hint()), tool == t, true) {
                            chosen_tool = Some(t);
                        }
                    }
                    ui.separator();
                    if tool_button(ui, Icon::Guides, "Construction", "Construction (X): chosen curves turn to construction and back; with none chosen, new curves are drawn as construction", construction, true) {
                        action = Some("construction");
                    }
                    if tool_button(ui, Icon::Delete, "Delete chosen", "Delete (Del): remove the chosen points and curves", false, chosen) {
                        action = Some("delete");
                    }
                    if tool_button(ui, Icon::Undo, "Undo sketch edit", "Undo (Ctrl+Z) the last edit inside this sketch", false, can_undo) {
                        action = Some("undo");
                    }
                    if tool_button(ui, Icon::Redo, "Redo sketch edit", "Redo (Ctrl+Shift+Z) inside this sketch", false, can_redo) {
                        action = Some("redo");
                    }
                    ui.separator();
                    if tool_button(ui, Icon::View, "Look at the sketch", "Turn the view square to the sketch's plane", false, true) {
                        action = Some("look");
                    }
                    match kind {
                        PlaneKind::Tangent { .. } => {
                            if tool_button(ui, Icon::Section, "Section plane", "Sketch on the plane through the finger's axis here instead: x out from the axis, y along the finger, for a profile to revolve round the ring", false, true) {
                                action = Some("section");
                            }
                        }
                        PlaneKind::Section { .. } => {
                            if tool_button(ui, Icon::Surface, "Tangent plane", "Sketch on the plane square to the band here instead: x round the ring, y along the finger", false, true) {
                                action = Some("tangent");
                            }
                        }
                        _ => {}
                    }
                    if let Some(attach) = solid {
                        ui.separator();
                        let ways = [
                            (Icon::CadUnion, "Join", Attach::Join, "Join (J): the solid is united with the band and what is joined to it", true),
                            (Icon::CadSubtract, "Cut", Attach::Cut, if can_cut { "Cut (J): the solid carves into the metal, down from the plane it was drawn on" } else { ALL_PARTS }, can_cut),
                            (Icon::CadPlace, "Separate", Attach::Separate, "Separate (J): the solid stands apart as a casting of its own", true),
                        ];
                        for (icon, label, of, tip, enabled) in ways {
                            if tool_button(ui, icon, label, tip, attach == of, enabled) {
                                action = Some(match of {
                                    Attach::Join => "join",
                                    Attach::Cut => "cut",
                                    Attach::Separate => "separate",
                                });
                            }
                        }
                    }
                    ui.separator();
                    if tool_button(ui, Icon::Check, "Finish sketch", "Finish: commit the sketch as one edit and leave sketch mode", false, true) {
                        action = Some("finish");
                    }
                    if tool_button(ui, Icon::Close, "Leave sketch", "Leave sketch mode; asks first when strokes are unfinished", false, true) {
                        action = Some("leave");
                    }
                });
            });
        });
    if let Some(t) = chosen_tool {
        set_tool(app, t);
    }
    match action {
        Some("construction") => {
            if let Some(live) = app.sketch.live.as_deref_mut() {
                let out = live.tools.toggle_construction(&mut live.working);
                report(app, out);
            }
        }
        Some("delete") => {
            if let Some(live) = app.sketch.live.as_deref_mut() {
                let out = live.tools.delete_chosen(&mut live.working);
                report(app, out);
            }
        }
        Some("undo") => {
            if let Some(live) = app.sketch.live.as_deref_mut() {
                live.tools.undo(&mut live.working);
            }
        }
        Some("redo") => {
            if let Some(live) = app.sketch.live.as_deref_mut() {
                live.tools.redo(&mut live.working);
            }
        }
        Some("look") => look_at(app, pane),
        Some("join") => set_attach(app, Attach::Join),
        Some("cut") => set_attach(app, Attach::Cut),
        Some("separate") => set_attach(app, Attach::Separate),
        Some("section") | Some("tangent") => switch_plane(app, pane, action == Some("section")),
        Some("finish") => {
            finish(app);
        }
        Some("leave") => leave_or_ask(app),
        _ => {}
    }
}

/// Moves a plane sketch between the plane square to the band and the one through the finger's axis.
fn switch_plane(app: &mut RingDesignerApp, pane: usize, section: bool) {
    let Some(live) = app.sketch.live.as_deref() else { return };
    let (PlaneKind::Tangent { theta_deg, across_mm } | PlaneKind::Section { theta_deg, across_mm }) = live.kind else { return };
    let Some(build) = app.build.clone() else { return };
    let Some((hit, normal)) = cad::surface_hit(&build.mesh, theta_deg, across_mm) else { return };
    let (plane, kind, centre) = if section {
        let p = section_plane(theta_deg);
        let centre = [dot(hit, p.x), hit[2]];
        (p, PlaneKind::Section { theta_deg, across_mm }, centre)
    } else {
        let Some(p) = tangent_plane(hit, normal, theta_deg) else { return };
        (p, PlaneKind::Tangent { theta_deg, across_mm }, [0.0; 2])
    };
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    live.working.plane = plane;
    live.kind = kind;
    live.centre = centre;
    live.frame = None;
    live.bumps += 1;
    refresh_frame(app);
    look_at(app, pane);
    app.set_status(if section { "Sketching on the section through the finger's axis" } else { "Sketching on the plane square to the band" });
}

/// The question asked before unfinished strokes are dropped.
fn ask(app: &mut RingDesignerApp, ui: &mut egui::Ui, rect: Rect) {
    let mut choice = None;
    egui::Area::new(Id::new("sketch-leave-question"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.center() - egui::vec2(170.0, 50.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).fill(theme::FLOAT).inner_margin(10).show(ui, |ui| {
                ui.set_width(340.0);
                ui.strong("This sketch has unfinished changes");
                ui.weak("Finishing commits them as one edit; discarding drops them and keeps what the document holds.");
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::image_and_text(Icon::Check.image(ui, 18.0), "Finish and leave")).clicked() {
                        choice = Some(0);
                    }
                    if ui.add(egui::Button::image_and_text(Icon::Delete.image(ui, 18.0), "Discard changes")).clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Keep drawing").clicked() {
                        choice = Some(2);
                    }
                });
            });
        });
    match choice {
        Some(0) => {
            if let Some(live) = app.sketch.live.as_deref_mut() {
                live.asking = false;
            }
            finish(app);
        }
        Some(1) => leave(app, "Left the sketch; its unfinished changes were discarded"),
        Some(2) => {
            if let Some(live) = app.sketch.live.as_deref_mut() {
                live.asking = false;
            }
            app.set_status("Keep drawing");
        }
        _ => {}
    }
}

/// Words by the pointer: the prompt, the live numbers and what the snap caught.
fn caption(painter: &egui::Painter, rect: Rect, anchor: Pos2, lines: &[String]) {
    let font = egui::FontId::proportional(12.0);
    let galleys: Vec<_> = lines
        .iter()
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, l)| painter.layout_no_wrap(l.clone(), font.clone(), if i == 0 { ringdesign_workbench::hover::AQUA } else { theme::TEXT }))
        .collect();
    if galleys.is_empty() {
        return;
    }
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y + 2.0).sum();
    let mut at = egui::pos2((anchor.x + 18.0).min(rect.right() - width - 8.0).max(rect.left() + 8.0), (anchor.y + 22.0).min(rect.bottom() - height - 8.0).max(rect.top() + 48.0));
    painter.rect_filled(Rect::from_min_size(at, egui::vec2(width, height)).expand(5.0), 4.0, Color32::from_black_alpha(190));
    for g in galleys {
        let h = g.size().y + 2.0;
        painter.galley(at, g, theme::TEXT);
        at.y += h;
    }
}

/// Draws the live sketch over the metal.
pub fn draw(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, response: &egui::Response, painter: &egui::Painter, proj: &Projector, active: bool) {
    if !app.sketch.is_live() || !follow(app) {
        return;
    }
    let rect = response.rect;
    let camera = app.panes[pane].camera;
    let Some(live) = app.sketch.live.as_deref_mut() else { return };
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(DIM_ALPHA));
    let Some(f) = live.frame else {
        let words = live.error.clone().unwrap_or_else(|| "Reading the sketch's plane…".into());
        caption(painter, rect, rect.center(), &[words]);
        if active {
            toolbar(app, ui, pane, rect);
        }
        return;
    };
    let (view, _) = ringdesign_workbench::hover::view_scale(rect.center(), &|p| camera.ray(rect, p));
    let px = view.px_per_mm.max(1e-6);
    let chord = live.chord(px);
    caches(live, chord);
    let to = |uv: [f64; 2]| proj.at(f.point(uv).map(|v| v as f32));
    // The plane's grid over what the view shows of it.
    let corners: Vec<[f64; 2]> = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]
        .into_iter()
        .filter_map(|p| {
            let (o, d) = camera.ray(rect, p);
            f.hit(o.map(f64::from), d.map(f64::from))
        })
        .collect();
    if corners.len() == 4 {
        let g = grid_mm(&live.working, px);
        let lo = corners.iter().fold([f64::INFINITY; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
        let hi = corners.iter().fold([f64::NEG_INFINITY; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
        let (i0, i1) = ((lo[0] / g).floor() as i64, (hi[0] / g).ceil() as i64);
        let (j0, j1) = ((lo[1] / g).floor() as i64, (hi[1] / g).ceil() as i64);
        if i1 - i0 < 400 && j1 - j0 < 400 {
            for i in i0..=i1 {
                let x = i as f64 * g;
                let stroke = if i == 0 { Stroke::new(1.2, theme::GOOD.gamma_multiply(0.7)) } else { Stroke::new(0.6, theme::GRID) };
                painter.line_segment([to([x, lo[1]]), to([x, hi[1]])], stroke);
            }
            for j in j0..=j1 {
                let y = j as f64 * g;
                let stroke = if j == 0 { Stroke::new(1.2, theme::BAD.gamma_multiply(0.7)) } else { Stroke::new(0.6, theme::GRID) };
                painter.line_segment([to([lo[0], y]), to([hi[0], y])], stroke);
            }
        }
    }
    // The ring the plane cuts, then the face the sketch lies on, lit.
    for seg in &live.under.cut {
        painter.line_segment([to(seg[0]), to(seg[1])], Stroke::new(1.0, ringdesign_workbench::hover::AQUA.gamma_multiply(0.45)));
    }
    for l in &live.under.face {
        let mut points: Vec<Pos2> = l.iter().map(|p| to(*p)).collect();
        points.extend(points.first().copied());
        painter.add(egui::Shape::line(points, Stroke::new(2.0, ringdesign_workbench::hover::AQUA)));
    }
    // Regions, the one under the pointer lit.
    if let Some((_, views, _)) = &live.regions {
        for (i, v) in views.iter().enumerate() {
            let fill = if live.hovered == Some(i) || live.menu_region == Some(i) && live.solid.is_some() { theme::ACCENT.gamma_multiply(0.38) } else { theme::ACCENT.gamma_multiply(0.14) };
            let mut mesh = egui::Mesh::default();
            for t in &v.triangles {
                let base = mesh.vertices.len() as u32;
                for p in t {
                    mesh.colored_vertex(to(*p), fill);
                }
                mesh.add_triangle(base, base + 1, base + 2);
            }
            painter.add(egui::Shape::mesh(mesh));
        }
    }
    // The extrusion being set up, as its outline at the base and at its height, below the plane for a cut.
    if let (Some(SolidStep::Extrude), Some((_, views, _))) = (&live.solid, &live.regions) {
        let h = live.typed("height").unwrap_or(1.0) * if live.cutting() { -1.0 } else { 1.0 };
        let up = |uv: [f64; 2]| {
            let p = f.point(uv);
            proj.at(std::array::from_fn(|k| (p[k] + f.n[k] * h) as f32))
        };
        let picked = live.picked();
        let swept = |i: usize| live.region_pick.is_none() || picked == Some(i);
        for v in views.iter().enumerate().filter(|(i, _)| swept(*i)).map(|(_, v)| v) {
            for poly in &v.polygons {
                let mut top: Vec<Pos2> = poly.iter().map(|p| up(*p)).collect();
                top.extend(top.first().copied());
                painter.add(egui::Shape::line(top, Stroke::new(1.5, theme::INFO)));
                let step = (poly.len() / 8).max(1);
                for p in poly.iter().step_by(step) {
                    painter.line_segment([to(*p), up(*p)], Stroke::new(1.0, theme::INFO.gamma_multiply(0.7)));
                }
            }
        }
    }
    if let Some(SolidStep::Revolve { pivot, dir, .. }) = &live.solid {
        let far = 50.0;
        painter.extend(egui::Shape::dashed_line(&[to([pivot[0] - dir[0] * far, pivot[1] - dir[1] * far]), to([pivot[0] + dir[0] * far, pivot[1] + dir[1] * far])], Stroke::new(1.5, theme::INFO), 6.0, 4.0));
    }
    // The sketch itself.
    for e in &live.working.entities {
        let chosen = live.tools.is_chosen(e.id);
        let stroke = if chosen {
            Stroke::new(2.5, theme::WARN)
        } else if e.construction {
            Stroke::new(1.0, theme::TEXT_DIM)
        } else {
            Stroke::new(2.0, theme::ACCENT)
        };
        for l in live.working.polylines(e.id, chord) {
            let points: Vec<Pos2> = l.iter().map(|p| to(*p)).collect();
            if e.construction && !chosen {
                painter.extend(egui::Shape::dashed_line(&points, stroke, 5.0, 4.0));
            } else {
                painter.add(egui::Shape::line(points, stroke));
            }
        }
    }
    for p in &live.working.points {
        let chosen = live.tools.chosen_points.contains(&p.id);
        painter.circle_filled(to(p.xy), if chosen { 4.0 } else { 2.5 }, if chosen { theme::WARN } else { theme::TEXT });
    }
    for a in sketch_tools::annotations(&live.working) {
        let (p, q) = (to(a.from), to(a.to));
        let mid = p + (q - p) * 0.5;
        let along = (q - p).normalized();
        let side = egui::vec2(-along.y, along.x) * 12.0;
        painter.text(mid + side, egui::Align2::CENTER_CENTER, &a.text, egui::FontId::proportional(11.0), theme::INFO);
    }
    let preview = live.tools.preview(&live.working);
    for s in &preview.strokes {
        painter.add(egui::Shape::line(s.iter().map(|p| to(*p)).collect(), Stroke::new(1.2, theme::INFO)));
    }
    for s in &preview.ghost {
        painter.add(egui::Shape::line(s.iter().map(|p| to(*p)).collect(), Stroke::new(2.0, theme::INFO)));
    }
    for m in &preview.marks {
        painter.circle_stroke(to(*m), 4.0, Stroke::new(1.5, theme::INFO));
    }
    if let Some((_, _, Some(snap))) = live.pointer {
        let p = to(snap.xy);
        if rect.contains(p) {
            let s = 5.0;
            painter.add(egui::Shape::closed_line(vec![p + egui::vec2(0.0, -s), p + egui::vec2(s, 0.0), p + egui::vec2(0.0, s), p + egui::vec2(-s, 0.0)], Stroke::new(2.0, theme::ACCENT)));
            painter.text(p + egui::vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, snap.kind.label(), egui::FontId::proportional(11.0), theme::ACCENT);
        }
    }
    if !active {
        return;
    }
    let mut lines = vec![live.prompt(), preview.caption.clone()];
    if let Some(e) = live.error.clone() {
        lines.push(e);
    }
    if live.solid.is_none()
        && !live.tools.busy()
        && let Some((_, _, Some(why))) = &live.regions
        && live.tools.tool == Tool::Select
    {
        lines.push(why.clone());
    }
    let anchor = live.anchor.unwrap_or(rect.center());
    let asking = live.asking;
    caption(painter, rect, anchor, &lines);
    toolbar(app, ui, pane, rect);
    if asking {
        ask(app, ui, rect);
    }
}
