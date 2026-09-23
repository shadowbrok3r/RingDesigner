//! The Ring viewport's command session: hotkeys, the snapped pointer, the ghost, the dimension bar, the rail, box select,
//! the ring-frame gizmo with the ring dial and the part's grips, and the press-drag-release of an added primitive.
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, EventFilter, Id, Key, Pos2, Rect};
use ringdesign_core::cad::edit::CadEdit;
use ringdesign_core::cad::{Attach, Component, Feature, Operation, Placement, Stage};
use ringdesign_core::castability::CastProcess;
use ringdesign_core::castability::ghost::{GhostJudge, GhostRead};
use ringdesign_core::interaction::pick::{Filter, Ray, ViewScale};
use ringdesign_core::{BuildResult, Mesh, sketch::Id as FeatureId};
use ringdesign_workbench::command::{
    AddPrimitiveCmd, Affine, AttachCmd, Axis, BandSurface, DimEvent, DimensionBar, Dofs, Effect, Grid, GripCmd, MoveCmd, Outcome, PlaceCmd,
    Primitive, Probe, Reading, RingFeatures, RingPoint, RotateCmd, ScaleCmd, Scene, Session, SnapGeometry, SnapHit, Snapper, StepInput,
    ViewCommand, catalog, land, placed_ghost, unit_ghost, unit_mesh,
};
use ringdesign_workbench::viewport::pins::Pin;
use ringdesign_workbench::gizmo::{self, Gizmo, Handle, Layout};
use ringdesign_workbench::icons::Icon;
use ringdesign_workbench::viewport::{Mods, Sel, box_planes};
use ringdesign_workbench::visual::Tool;

use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::pane::PaneKind;
use crate::theme;
use crate::viewport::{APERTURE_PX, GpuMeshRenderer};

/// The grid the pointer snaps to on the ring: 5° round, 0.5 mm across and out.
pub const GRID: Grid = Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.5 };
/// The tool rail's width at the viewport's left edge, its gutter included.
pub const RAIL_W: f32 = 46.0;
/// How long a committed ghost waits for the rebuild before it goes.
const LINGER: Duration = Duration::from_secs(3);
/// A box smaller than this on either side is a click.
const MIN_BOX_PX: f32 = 3.0;

/// A gizmo handle held down: the handle, the gizmo as it stood at the press, and how the drag has gone.
struct GizmoDrag {
    handle: Handle,
    /// The gizmo as it stood at the press, which the drag is read against.
    gizmo: Gizmo,
    press: Pos2,
    /// The pointer has left the press point.
    moved: bool,
    /// The command ended mid-drag; the rest of the press is swallowed.
    ended: bool,
}

/// What the preview buffer holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Staged {
    /// A part's placed tessellation from the build it was taken from.
    Part { build: usize, feature: FeatureId },
    /// A unit primitive.
    Unit(Primitive),
}

/// The session, its dimension bar and everything the viewport keeps for them between frames.
pub struct CommandState {
    pub session: Session,
    pub bar: DimensionBar,
    pub snapper: Snapper,
    /// Armed by B: the next primary drag in the Ring viewport draws a box.
    pub box_armed: bool,
    /// The part the live command acts on, as the document held it when the command started.
    target: Option<Feature>,
    /// Where a scale is dragged from: the part's centre.
    pivot: Option<[f64; 3]>,
    /// The axis X, Y or Z last locked, so a second press unlocks it.
    lock: Option<Axis>,
    /// The band the ring frame is read on, by the build it came with; `None` inside for a ring of parts only.
    band: Option<(usize, Option<Arc<BandSurface>>)>,
    /// Part vertices and edges to snap to, by build and carried part.
    snaps: Option<(usize, Option<FeatureId>, Vec<[f64; 3]>, Vec<Vec<[f64; 3]>>)>,
    /// The ring's own snap targets, by build, carried part and pins.
    features: Option<(usize, Option<FeatureId>, u64, Arc<RingFeatures>)>,
    /// The carried ghost's castability.
    tint: Tint,
    /// Unit box, cylinder and sphere for an add's ghost.
    units: [Option<Mesh>; 3],
    staged: Option<Staged>,
    /// The committed ghost holds until a new build lands: the build it was committed over, and when.
    linger: Option<(usize, Instant)>,
    /// The last pointer sample: screen position, build, step, and what it snapped to.
    pointer: Option<(Pos2, usize, usize, Option<SnapHit>)>,
    /// Where the bar and caption stand: the pointer, held while a field is typed in.
    anchor: Option<Pos2>,
    /// The box's first corner, once its drag has started.
    box_from: Option<Pos2>,
    /// The Shift+A menu opens on the next draw.
    open_menu: bool,
    /// The gizmo handle being dragged.
    gizmo_drag: Option<GizmoDrag>,
    /// The gizmo handle under the pointer.
    gizmo_hot: Option<Handle>,
    /// An added primitive's press on the ring, held down: the step it began on.
    add_press: Option<usize>,
}

impl Default for CommandState {
    fn default() -> Self {
        Self {
            session: Session::default(),
            bar: DimensionBar::new("ring-viewport-dimensions"),
            snapper: Snapper { grid: Some(GRID), crest: true, ..Snapper::default() },
            box_armed: false,
            target: None,
            pivot: None,
            lock: None,
            band: None,
            snaps: None,
            features: None,
            tint: Tint::default(),
            units: [None, None, None],
            staged: None,
            linger: None,
            pointer: None,
            anchor: None,
            box_from: None,
            open_menu: false,
            gizmo_drag: None,
            gizmo_hot: None,
            add_press: None,
        }
    }
}

/// The carried ghost read for castability: the judge by build, where the ghost was read, and what it says.
#[derive(Default)]
struct Tint {
    judge: Option<(usize, Arc<GhostJudge>)>,
    at: Option<(Staged, Affine)>,
    read: Option<GhostRead>,
    /// What the part's stage and the process add to the caption.
    note: &'static str,
}

impl CommandState {
    /// The gizmo handle under the pointer, which the viewport's own hover stands aside for.
    pub fn gizmo_hot(&self) -> Option<Handle> {
        self.gizmo_hot
    }

    /// Whether a press in the viewport belongs to a gizmo handle or to an added primitive.
    pub fn holds_press(&self) -> bool {
        self.gizmo_drag.is_some() || self.add_press.is_some()
    }

    /// What the carried ghost would do in the sand where it stands, while a command carries one.
    pub fn ghost_caption(&self) -> Option<String> {
        let read = self.tint.read.as_ref().filter(|_| self.session.is_live())?;
        Some(format!("Ghost {}{}", read.caption(), self.tint.note))
    }
}

#[cfg(test)]
impl CommandState {
    /// The band surface the ring frame was last read on, by address.
    pub fn band_surface(&self) -> Option<usize> {
        self.band.as_ref().and_then(|(_, b)| b.as_ref()).map(|b| Arc::as_ptr(b) as usize)
    }

    /// The carried ghost's castability as last read.
    pub fn ghost_read(&self) -> Option<&GhostRead> {
        self.tint.read.as_ref()
    }

    /// The last pointer sample's snap.
    pub fn snapped(&self) -> Option<&SnapHit> {
        self.pointer.as_ref().and_then(|(_, _, _, s)| s.as_ref())
    }

    /// The handle being dragged.
    pub fn dragging(&self) -> Option<Handle> {
        self.gizmo_drag.as_ref().filter(|d| !d.ended).map(|d| d.handle)
    }
}

/// What the command layer took from this frame, so the viewport stands aside for it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Took {
    /// A command is live.
    pub live: bool,
    /// The primary click went to the command or the box.
    pub click: bool,
    /// The primary drag draws a box instead of orbiting.
    pub drag: bool,
    /// The secondary click cancelled a command instead of opening the menu.
    pub secondary: bool,
    /// Box select is armed.
    pub boxing: bool,
}

fn build_key(build: &Arc<BuildResult>) -> usize {
    Arc::as_ptr(build) as usize
}

fn primitive(key: &str) -> Option<Primitive> {
    match key {
        "add-box" => Some(Primitive::Box),
        "add-cylinder" => Some(Primitive::Cylinder),
        "add-sphere" => Some(Primitive::Sphere),
        _ => None,
    }
}

fn unit_slot(kind: Primitive) -> usize {
    match kind {
        Primitive::Box => 0,
        Primitive::Cylinder => 1,
        Primitive::Sphere => 2,
    }
}

/// Whether a catalog command acts on a chosen part.
fn needs_part(key: &str) -> bool {
    primitive(key).is_none()
}

/// The keyboard route to a catalog command: its hotkey, or the Shift+A menu and the item in it.
pub fn keys(key: &str) -> Option<&'static str> {
    Some(match key {
        "move" => "G",
        "rotate" => "R",
        "scale" => "S",
        "place" => "P",
        "attach" => "J",
        "add-box" => "Shift+A, Box",
        "add-cylinder" => "Shift+A, Cylinder",
        "add-sphere" => "Shift+A, Sphere",
        _ => return None,
    })
}

/// The feature of the last chosen part, face, edge or vertex.
pub fn selected_part(app: &RingDesignerApp) -> Option<FeatureId> {
    app.selection.items.iter().rev().find_map(Sel::feature)
}

fn feature(app: &RingDesignerApp, id: FeatureId) -> Option<Feature> {
    app.design.cad.as_ref()?.feature(id).cloned()
}

/// Why the catalog command `key` cannot start now; `None` when it can.
pub fn blocked(app: &RingDesignerApp, key: &str) -> Option<String> {
    if !needs_part(key) {
        return None;
    }
    let Some(id) = selected_part(app) else {
        return Some("Select a part first: click one on the ring".into());
    };
    let Some(f) = feature(app, id) else {
        return Some(format!("Part #{id} is not in the document"));
    };
    if key == "attach" && f.component.reference {
        return Some("A reference stone is never metal".into());
    }
    if let (Operation::Pattern { source, .. }, "move" | "rotate" | "scale" | "place") = (&f.operation, key) {
        let from = feature(app, *source).map_or_else(|| format!("#{source}"), |s| format!("\"{}\"", s.name));
        return Some(format!("{} follows its source: move {from} and its copies follow", f.name));
    }
    if key == "scale" && ScaleCmd::new(f.id, f.operation.clone(), [0.0; 3]).is_none() {
        return Some(format!("{} has no size to scale", f.operation.label()));
    }
    None
}

/// Makes a Ring viewport the active pane, opening one when none is on screen.
fn ring_pane(app: &mut RingDesignerApp) {
    let solid = |app: &RingDesignerApp, i: usize| app.panes.get(i).is_some_and(|p| p.kind == PaneKind::Solid && !p.follow_node);
    if app.visible_panes().contains(&app.active_pane) && solid(app, app.active_pane) {
        return;
    }
    match app.visible_panes().into_iter().find(|i| solid(app, *i)) {
        Some(i) => app.active_pane = i,
        None => app.focus(PaneKind::Solid),
    }
}

/// The world centre of a built part, where a scale is dragged from.
fn centre_of(app: &RingDesignerApp, id: FeatureId) -> Option<[f64; 3]> {
    let c = app.build.as_ref()?.parts.evaluated.as_ref()?.components.iter().find(|c| c.id == id)?;
    let (lo, hi) = c.mesh.bounds()?;
    Some([(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5])
}

/// Starts catalog command `key` on the chosen part for its hotkey, rail slot and palette entry; the status line says why not.
pub fn start(app: &mut RingDesignerApp, key: &str) -> bool {
    if let Some(why) = blocked(app, key) {
        app.set_status(why);
        return false;
    }
    ring_pane(app);
    if app.visual.tool != Tool::Select {
        app.visual.select(Tool::Select);
    }
    let fresh = app.design.cad.as_ref().map_or(1, |d| d.fresh_id());
    let target = selected_part(app).and_then(|id| feature(app, id)).filter(|_| needs_part(key));
    let pivot = target.as_ref().and_then(|f| centre_of(app, f.id));
    let cmd: Box<dyn ViewCommand> = match (key, &target) {
        ("move", Some(f)) => Box::new(MoveCmd::of(f, fresh)),
        ("rotate", Some(f)) => Box::new(RotateCmd::of(f, fresh)),
        ("scale", Some(f)) => match ScaleCmd::new(f.id, f.operation.clone(), pivot.unwrap_or([0.0; 3])) {
            Some(c) => Box::new(c),
            None => return false,
        },
        ("place", Some(f)) => Box::new(PlaceCmd::new(f.id, f.component.placement.clone())),
        ("attach", Some(f)) => {
            // One press steps the part on to its next attachment and commits it.
            let (id, attach) = (f.id, f.component.attach);
            let st = &mut app.command;
            st.session.start(Box::new(AttachCmd::new(id, attach)));
            st.session.feed(StepInput::Click);
            let out = st.session.feed(StepInput::Confirm);
            outcome(app, out);
            return true;
        }
        (k, None) if primitive(k).is_some() => {
            // A plain ring's first part brings its shank with it; the shank takes id 1.
            let empty = app.design.cad.as_ref().is_none_or(|d| d.features.is_empty());
            Box::new(AddPrimitiveCmd::new(primitive(k).unwrap_or(Primitive::Box), if empty { 2 } else { fresh }))
        }
        _ => return false,
    };
    let st = &mut app.command;
    st.session.start(cmd);
    st.target = target;
    st.pivot = pivot;
    st.lock = None;
    st.pointer = None;
    st.linger = None;
    st.box_armed = false;
    st.bar.reset();
    st.tint.at = None;
    st.tint.read = None;
    let prompt = st.session.prompt();
    app.set_status(prompt);
    true
}

/// Ends a live command without committing it; whether one was live.
pub fn cancel(app: &mut RingDesignerApp) -> bool {
    if !app.command.session.is_live() {
        return false;
    }
    let out = app.command.session.feed(StepInput::Cancel);
    outcome(app, out);
    true
}

/// Does what a command's outcome asks: commit through the funnel, say a refusal, drop a cancelled ghost.
fn outcome(app: &mut RingDesignerApp, out: Outcome) {
    match out {
        Outcome::Commit(effects) => commit(app, effects),
        Outcome::Refused(why) => app.set_status(why),
        Outcome::Cancelled => {
            app.command.target = None;
            app.command.linger = None;
            app.set_status("Cancelled; nothing changed");
        }
        Outcome::NextStep => {
            let prompt = app.command.session.prompt();
            app.set_status(prompt);
        }
        Outcome::Continue => {}
    }
}

/// Whether a feature builds a body of its own, as the first part on a plain ring does.
fn is_body(f: &Feature) -> bool {
    f.operation.sources().is_empty() && !matches!(f.operation, Operation::Band | Operation::Sketch { .. })
}

/// A command's effects as edits, applied through the funnel as one undo step; an added part becomes the selection.
fn commit(app: &mut RingDesignerApp, effects: Vec<Effect>) {
    let mut edits = Vec::new();
    let mut added = false;
    for effect in effects {
        match effect {
            Effect::Placement { feature, placement } => edits.push(CadEdit::Placement { id: feature, placement }),
            Effect::Operation { feature, operation } => edits.push(CadEdit::Operation { id: feature, operation }),
            Effect::Attach { feature, attach } => edits.push(CadEdit::Attach { id: feature, attach }),
            Effect::Stage { feature, stage } => edits.push(CadEdit::Stage { id: feature, stage }),
            Effect::Add { mut feature } => {
                let doc = app.design.cad.as_ref();
                if doc.is_none_or(|d| d.features.is_empty()) && is_body(&feature) {
                    let id = if feature.id == 1 { 2 } else { 1 };
                    let shank = Feature { id, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() };
                    edits.push(CadEdit::Add { feature: shank, after: None });
                } else if doc.is_some_and(|d| d.band().is_none()) && is_body(&feature) {
                    // A ring of parts only keeps a new part separate.
                    feature.component.attach = Attach::Separate;
                }
                added = true;
                edits.push(CadEdit::Add { feature, after: None });
            }
        }
    }
    let build = app.build.as_ref().map(build_key).unwrap_or(0);
    if let Ok(applied) = crate::cad_edit::apply(app, &edits) {
        if let Some(id) = applied.last().and_then(|a| a.id).filter(|_| added) {
            app.selection.click(Some(Sel::Part(id)), Mods::default());
        }
        app.command.linger = Some((build, Instant::now()));
    } else {
        app.command.linger = None;
    }
    app.command.target = None;
}

/// Ends a live command whose part has left the document.
fn drop_orphan(app: &mut RingDesignerApp) {
    let gone = app.command.target.as_ref().is_some_and(|t| feature(app, t.id).is_none());
    if gone && app.command.session.is_live() {
        app.command.session.feed(StepInput::Cancel);
        app.command.target = None;
        app.set_status("The part the command was carrying is gone");
    }
}

/// The ring frame the worker read on the build that just landed.
pub fn band_landed(app: &mut RingDesignerApp, band: Option<Arc<BandSurface>>) {
    let at = app.build.as_ref().map(build_key).unwrap_or(0);
    app.command.band = Some((at, band));
}

/// The ring frame for this build: the worker's, else the surface over the band the build swept, read here once.
fn band_for(app: &mut RingDesignerApp, build: &Arc<BuildResult>) -> Option<Arc<BandSurface>> {
    let at = build_key(build);
    if let Some((b, band)) = &app.command.band
        && *b == at
        && (band.is_some() || build.band.is_none())
    {
        return band.clone();
    }
    let band = build.band.clone().map(|mesh| Arc::new(BandSurface::shared(mesh)));
    app.command.band = Some((at, band.clone()));
    band
}

/// A hash of the pins, which the ring's features are kept by.
fn pins_key(pins: &[Pin]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in pins {
        p.name.hash(&mut h);
        p.world.map(f64::to_bits).hash(&mut h);
    }
    h.finish()
}

/// The ring's own snap targets for this build, less the carried part: named angles, the parting line, side faces, stones, parts and pins.
fn features_for(app: &mut RingDesignerApp, build: &Arc<BuildResult>) -> Arc<RingFeatures> {
    let at = build_key(build);
    let carried = app.command.target.as_ref().map(|f| f.id);
    let pins = pins_key(app.pins());
    if let Some((b, c, p, f)) = &app.command.features
        && (*b, *c, *p) == (at, carried, pins)
    {
        return f.clone();
    }
    let parting = app.field.as_ref().map_or(0.0, |f| f.parting_z_mm);
    let features = RingFeatures::of(&app.design, parting)
        .with_stones(&app.design)
        .with_parts(build.parts.evaluated.as_ref(), carried)
        .with_points(app.pins().iter().map(Pin::target));
    let features = Arc::new(features);
    app.command.features = Some((at, carried, pins, features.clone()));
    features
}

/// Where a point of the band snaps among the ring's features and the parts' own points, off the grid: what Measure reads.
pub fn snap_band_point(app: &mut RingDesignerApp, build: &Arc<BuildResult>, world: [f64; 3], view: ViewScale) -> Option<SnapHit> {
    let band = band_for(app, build);
    snaps_for(app, build);
    let features = features_for(app, build);
    let (design, st) = (&app.design, &app.command);
    let (vertices, edges) = st.snaps.as_ref().map_or((&[][..], &[][..]), |(_, _, v, e)| (v.as_slice(), e.as_slice()));
    let nominal = design.inner_radius_mm() + design.profile.thickness_mm;
    let world_of = |p: RingPoint| band.as_deref().and_then(|b| b.world(p));
    let scene = Scene { view, aperture_px: APERTURE_PX, geometry: SnapGeometry { vertices, edges }, features: &features, design: Some(design), world_of: &world_of };
    let ring = ringdesign_workbench::command::ring_point(world, band.as_deref(), nominal);
    Snapper { grid: None, ..st.snapper }.snap_ring(world, ring, Dofs::ALL, &scene)
}

/// The ring coordinates the live command lands its part on, when it lands one: a seated part's move, a place, an add's centre.
fn landing_dofs(app: &RingDesignerApp) -> Option<Dofs> {
    let cmd = app.command.session.command()?;
    let seated = app.command.target.as_ref().is_some_and(|f| matches!(f.component.placement, Placement::Ring { .. }));
    match cmd.key() {
        "move" if seated => Some(Dofs::of(app.command.lock)),
        "place" => Some(Dofs::of(app.command.lock)),
        k if primitive(k).is_some() && cmd.step() == 0 => Some(Dofs::ALL),
        _ => None,
    }
}

/// Feeds `token` to the live command, landing its part on the best snap of `dofs` over the ring's features; the hit it landed on.
fn feed_landed(app: &mut RingDesignerApp, build: &Arc<BuildResult>, token: StepInput, view: ViewScale, dofs: Dofs, snapper: Snapper) -> (Outcome, Option<SnapHit>) {
    let band = band_for(app, build);
    snaps_for(app, build);
    let features = features_for(app, build);
    let (design, st) = (&app.design, &mut app.command);
    let (vertices, edges) = st.snaps.as_ref().map_or((&[][..], &[][..]), |(_, _, v, e)| (v.as_slice(), e.as_slice()));
    let nominal = design.inner_radius_mm() + design.profile.thickness_mm;
    let world_of = |p: RingPoint| match band.as_deref() {
        Some(b) => b.world(p),
        None => {
            let (s, c) = p.theta_deg.to_radians().sin_cos();
            let r = nominal + p.height_mm;
            Some([r * c, r * s, p.across_mm])
        }
    };
    let scene = Scene { view, aperture_px: APERTURE_PX, geometry: SnapGeometry { vertices, edges }, features: &features, design: Some(design), world_of: &world_of };
    let hit = std::cell::RefCell::new(None);
    let snap = |p: RingPoint| {
        let h = snapper.snap_ring(world_of(p)?, p, dofs, &scene);
        hit.replace(h.clone());
        h
    };
    let out = land(&mut st.session, token, &snap);
    (out, hit.take())
}

/// Part vertices and edges the pointer may snap to: every part's but the carried one's.
fn snaps_for(app: &mut RingDesignerApp, build: &Arc<BuildResult>) {
    let key = build_key(build);
    let carried = app.command.target.as_ref().map(|f| f.id);
    if app.command.snaps.as_ref().is_some_and(|(k, c, ..)| *k == key && *c == carried) {
        return;
    }
    let (mut vertices, mut edges) = (Vec::new(), Vec::new());
    if let Some(e) = &build.parts.evaluated {
        for c in e.components.iter().filter(|c| Some(c.id) != carried && !c.settings.reference) {
            vertices.extend_from_slice(&c.trace.vertices);
            edges.extend(c.edges.iter().cloned());
        }
    }
    app.command.snaps = Some((key, carried, vertices, edges));
}

/// How the live command reads the pointer: sizes on the view plane through their anchor, the rest on the metal.
fn reading(st: &CommandState) -> Reading {
    let Some(cmd) = st.session.command() else { return Reading::Surface };
    match cmd.key() {
        "scale" => st.pivot.map_or(Reading::Surface, |at| Reading::Plane { at }),
        "press-pull" => cmd.preview().ghost.first().map_or(Reading::Surface, |at| Reading::Plane { at: *at }),
        k if primitive(k).is_some() && cmd.step() > 0 => cmd.preview().ghost.first().map_or(Reading::Surface, |at| Reading::Plane { at: *at }),
        _ => Reading::Surface,
    }
}

/// Reads the pointer at `pos` into the live command, landing a carried part on the ring's snaps; `free` leaves the snaps off.
fn sample(app: &mut RingDesignerApp, pane: usize, rect: Rect, pos: Pos2, free: bool) {
    let (Some(scene), Some(build)) = (app.pick_scene.clone(), app.build.clone()) else { return };
    let band = band_for(app, &build);
    snaps_for(app, &build);
    let camera = app.panes[pane].camera;
    let at = |p: Pos2| camera.ray(rect, p);
    let (view, ray) = ringdesign_workbench::hover::view_scale(pos, &at);
    let picks = scene.pick(ray, &view, 0.0, Filter { vertices: false, edges: false, ..Filter::default() });
    let lands = landing_dofs(app).filter(|_| !free && reading(&app.command) == Reading::Surface);
    let token = {
        let st = &app.command;
        let (vertices, edges) = st.snaps.as_ref().map_or((&[][..], &[][..]), |(_, _, v, e)| (v.as_slice(), e.as_slice()));
        let probe = Probe {
            design: &app.design,
            surface: band.as_deref(),
            snapper: (!free && lands.is_none()).then_some(st.snapper),
            geometry: SnapGeometry { vertices, edges },
            view,
            aperture_px: APERTURE_PX,
            carried: st.target.as_ref().map(|f| f.id),
        };
        probe.token(reading(st), &picks, ray)
    };
    let Some(token) = token else { return };
    let (out, snap) = match lands {
        Some(dofs) => {
            let snapper = app.command.snapper;
            feed_landed(app, &build, token, view, dofs, snapper)
        }
        None => {
            let snap = match &token {
                StepInput::Pointer { snapped, .. } => snapped.clone(),
                _ => None,
            };
            (app.command.session.feed(token), snap)
        }
    };
    let step = app.command.session.command().map_or(0, |c| c.step());
    app.command.pointer = Some((pos, build_key(&build), step, snap));
    outcome(app, out);
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

/// The axis X, Y or Z locks: θ, height and across on the ring, cant, spin and tilt when turning, else the world's.
fn axis_for(cmd: &dyn ViewCommand, key: Key) -> Option<Axis> {
    let dims = cmd.dimensions();
    let has = |k: &str| dims.iter().any(|d| d.key == k);
    let k = match key {
        Key::X => 0,
        Key::Y => 1,
        Key::Z => 2,
        _ => return None,
    };
    Some(if has("theta") {
        [Axis::Theta, Axis::Height, Axis::Across][k]
    } else if has("spin") {
        [Axis::Cant, Axis::Spin, Axis::Tilt][k]
    } else {
        [Axis::X, Axis::Y, Axis::Z][k]
    })
}

/// What X, Y and Z lock for the live command, as the caption says it.
fn axis_hint(cmd: &dyn ViewCommand) -> Option<&'static str> {
    let dims = cmd.dimensions();
    let has = |k: &str| dims.iter().any(|d| d.key == k);
    if primitive(cmd.key()).is_some() || cmd.key() == "grip" {
        None
    } else if has("theta") {
        Some("X θ · Y height · Z across")
    } else if has("spin") {
        Some("X cant · Y spin · Z tilt")
    } else if has("x") || has("factor") {
        Some("X · Y · Z lock an axis")
    } else {
        None
    }
}

fn menu_id(pane: usize) -> Id {
    Id::new(("ring-add-menu", pane))
}

/// The active Ring viewport's command input: held keys, the dimension bar, the pointer, clicks and the box drag.
pub fn input(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, rect: Rect, response: &egui::Response) -> Took {
    let ctx = ui.ctx().clone();
    let id = response.id;
    drop_orphan(app);
    app.command.bar.set_host(Some(id));
    hold_keys(app, &ctx, id);
    let hover = ui.input(|i| (!i.pointer.any_down()).then(|| i.pointer.hover_pos()).flatten()).filter(|p| rect.contains(*p) && response.hovered());
    if !app.command.bar.has_focus(&ctx)
        && let Some(p) = hover
    {
        app.command.anchor = Some(p);
    }
    let mut took = Took::default();
    let free = ui.input(|i| i.modifiers.command);
    // The pointer before the bar: a gizmo handle, then an added primitive's press, then the live command's hover.
    let gizmo = gizmo_input(app, ui, pane, rect, response, hover, free);
    let add = !gizmo && add_input(app, ui, pane, rect, response, free);
    if gizmo || add {
        took.click = true;
        took.drag = true;
    }
    // The live command reads the pointer under the dimension bar too; the bar holds still there so a field can be clicked.
    let reading = hover.or_else(|| ui.input(|i| (!i.pointer.any_down()).then(|| i.pointer.hover_pos()).flatten()).filter(|p| rect.contains(*p) && app.command.bar.covers(&ctx, *p)));
    if app.command.session.is_live()
        && !gizmo
        && let Some(pos) = reading
    {
        let build = app.build.as_ref().map(build_key).unwrap_or(0);
        let step = app.command.session.command().map_or(0, |c| c.step());
        let moved = app.command.pointer.as_ref().is_none_or(|(p, b, s, _)| p.distance(pos) > 0.25 || *b != build || *s != step);
        if moved {
            sample(app, pane, rect, pos, free);
        }
    }
    // Then the bar: a field being typed in takes its own keys.
    if app.command.session.is_live() {
        let mut dims = app.command.session.dimensions();
        let anchor = app.command.anchor.unwrap_or(rect.center());
        for e in app.command.bar.show(&ctx, anchor, rect, &mut dims) {
            let out = match e {
                DimEvent::Typed { key, value } => app.command.session.feed(StepInput::Typed { key, value }),
                DimEvent::Cleared { key } => app.command.session.feed(StepInput::Cleared { key }),
                DimEvent::Confirm => app.command.session.enter(),
                DimEvent::Escape => app.command.session.escape(),
                DimEvent::Focused { .. } => Outcome::Continue,
            };
            outcome(app, out);
        }
    }
    let owns_keys = ctx.memory(|m| m.focused()).is_none_or(|f| f == id) && !egui::Popup::is_id_open(&ctx, menu_id(pane));
    if owns_keys {
        keys_of_frame(app, ui, hover.is_some() || response.contains_pointer());
    }
    // A click where the pointer landed; the right button cancels.
    if app.command.session.is_live() && !gizmo {
        if response.clicked() && !add {
            if let Some(pos) = response.interact_pointer_pos() {
                sample(app, pane, rect, pos, free);
            }
            if app.command.session.is_live() {
                let out = app.command.session.feed(StepInput::Click);
                outcome(app, out);
            }
            took.click = true;
        }
        if response.secondary_clicked() {
            let out = app.command.session.feed(StepInput::Cancel);
            outcome(app, out);
            app.command.add_press = None;
            took.secondary = true;
        }
    }
    // Box select: the armed drag draws a box instead of orbiting.
    if app.command.box_armed {
        if response.drag_started_by(egui::PointerButton::Primary) {
            app.command.box_from = ui.input(|i| i.pointer.press_origin());
        }
        if app.command.box_from.is_some() {
            took.drag = true;
            if response.drag_stopped() {
                let to = response.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.latest_pos()));
                let mods = ui.input(|i| Mods { shift: i.modifiers.shift, ctrl: i.modifiers.command, alt: i.modifiers.alt });
                if let (Some(from), Some(to)) = (app.command.box_from, to) {
                    finish_box(app, pane, rect, from, to, mods);
                }
                app.command.box_from = None;
                app.command.box_armed = false;
            }
        } else if response.clicked() {
            // A click without a drag disarms the box and selects as usual.
            app.command.box_armed = false;
        }
    }
    hold_keys(app, &ctx, id);
    took.live = app.command.session.is_live() || gizmo;
    took.boxing = app.command.box_armed;
    took
}

/// The world ray under a screen point of the pane.
fn ray_at(app: &RingDesignerApp, pane: usize, rect: Rect, p: Pos2) -> Ray {
    let (o, d) = app.panes[pane].camera.ray(rect, p);
    Ray { origin: o.map(f64::from), direction: d.map(f64::from) }
}

/// The chosen part and its gizmo: one part chosen, nothing live, the Select tool out and no sketch drawn.
fn gizmo_of(app: &mut RingDesignerApp) -> Option<(FeatureId, Gizmo)> {
    if app.command.session.is_live() || app.command.box_armed || app.visual.tool != Tool::Select || crate::sketch_mode::active(app) {
        return None;
    }
    let id = app.selection.one_part()?;
    let f = feature(app, id)?;
    if matches!(f.operation, Operation::Band | Operation::Sketch { .. } | Operation::Pattern { .. }) {
        return None;
    }
    let build = app.build.clone()?;
    let band = band_for(app, &build);
    let part = build.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == id));
    let gizmo = match &f.component.placement {
        placement @ Placement::Ring { .. } => {
            let g = Gizmo::on_ring(&app.design, band.as_deref(), placement, 0.0)?;
            let reach_mm = part.map_or(0.0, |c| gizmo::reach(&c.mesh, g.origin));
            Gizmo { reach_mm, ..g }
        }
        Placement::Free => {
            let (lo, hi) = part?.mesh.bounds()?;
            let centre = [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5];
            Gizmo::free(centre, part.map_or(0.0, |c| gizmo::reach(&c.mesh, centre)))
        }
    };
    Some((id, gizmo.with_grips(&f.operation)))
}

/// The gizmo on the pane's screen.
fn layout_of(app: &RingDesignerApp, pane: usize, rect: Rect, gizmo: &Gizmo) -> Layout {
    on_screen(app, pane, rect, |view| gizmo.layout(view))
}

/// What a handle's drag does, for the status line.
fn hint(handle: Handle) -> &'static str {
    match handle {
        Handle::Move(_) => "drag along the arrow · type a distance · Esc or right-click cancels",
        Handle::Turn(_) => "drag round the ring · type degrees · Esc or right-click cancels",
        Handle::Dial => "drag round the ring on the 5° grid, Ctrl for free · type θ · Esc or right-click cancels",
        Handle::Grip(_) => "drag the grip · type a size · Esc or right-click cancels",
    }
}

/// Starts the command a handle drags, locked to the handle, and reads the press as its first pointer.
fn start_drag(app: &mut RingDesignerApp, pane: usize, rect: Rect, id: FeatureId, gizmo: Gizmo, handle: Handle, at: Pos2, free: bool) {
    let Some(f) = feature(app, id) else { return };
    let fresh = app.design.cad.as_ref().map_or(1, |d| d.fresh_id());
    let (cmd, lock): (Box<dyn ViewCommand>, Option<Axis>) = match handle {
        Handle::Move(axis) => (Box::new(MoveCmd::of(&f, fresh)), Some(axis)),
        Handle::Turn(axis) => (Box::new(RotateCmd::of(&f, fresh).about(gizmo.pivot())), Some(axis)),
        Handle::Dial => (Box::new(PlaceCmd::new(f.id, f.component.placement.clone())), Some(Axis::Theta)),
        Handle::Grip(i) => match gizmo.grips.get(i).and_then(|g| GripCmd::new(f.id, f.operation.clone(), g.grip.key, &gizmo.frame)) {
            Some(c) => (Box::new(c), None),
            None => return,
        },
    };
    let st = &mut app.command;
    st.session.start(cmd);
    if let Some(axis) = lock {
        st.session.feed(StepInput::Lock(axis));
    }
    st.target = Some(f);
    st.pivot = None;
    st.lock = lock;
    st.pointer = None;
    st.linger = None;
    st.box_armed = false;
    st.gizmo_hot = None;
    st.tint.at = None;
    st.tint.read = None;
    st.bar.reset();
    st.bar.prefer(gizmo.key(handle));
    // The drag is measured from where the handle was taken.
    feed_drag(app, pane, rect, at, &gizmo, handle, free);
    app.command.gizmo_drag = Some(GizmoDrag { handle, gizmo, press: at, moved: false, ended: false });
    let status = format!("{} · {}", app.command.session.prompt(), hint(handle));
    app.set_status(status);
}

/// Feeds a gizmo drag's pointer at `at`: an arrow or the dial lands the part on the ring's snaps, the dial on its 5° grid too.
fn feed_drag(app: &mut RingDesignerApp, pane: usize, rect: Rect, at: Pos2, gizmo: &Gizmo, handle: Handle, free: bool) -> Option<Outcome> {
    let token = gizmo.token(handle, ray_at(app, pane, rect, at), None)?;
    let (out, snap) = match (gizmo.dofs(handle), app.build.clone(), free) {
        (Some(dofs), Some(build), false) => {
            let camera = app.panes[pane].camera;
            let (view, _) = ringdesign_workbench::hover::view_scale(at, &|p| camera.ray(rect, p));
            let grid = (handle == Handle::Dial).then_some(Grid { theta_deg: GRID.theta_deg, across_mm: 0.0, height_mm: 0.0 });
            let snapper = Snapper { grid, ..app.command.snapper };
            feed_landed(app, &build, token, view, dofs, snapper)
        }
        _ => (app.command.session.feed(token), None),
    };
    let key = app.build.as_ref().map(build_key).unwrap_or(0);
    app.command.pointer = Some((at, key, 0, snap));
    Some(out)
}

/// A gizmo handle's drag, or a press on one; whether this frame's press is the gizmo's.
fn gizmo_input(app: &mut RingDesignerApp, ui: &egui::Ui, pane: usize, rect: Rect, response: &egui::Response, hover: Option<Pos2>, free: bool) -> bool {
    if let Some(mut drag) = app.command.gizmo_drag.take() {
        let (released, down, secondary, pos) = ui.input(|i| (i.pointer.primary_released(), i.pointer.primary_down(), i.pointer.secondary_pressed(), i.pointer.interact_pos()));
        drag.ended |= !app.command.session.is_live();
        if !drag.ended && secondary {
            let out = app.command.session.feed(StepInput::Cancel);
            outcome(app, out);
            drag.ended = true;
        }
        if !drag.ended
            && let Some(p) = pos
        {
            drag.moved |= p.distance(drag.press) > 2.0;
            if let Some(out) = feed_drag(app, pane, rect, p, &drag.gizmo, drag.handle, free) {
                outcome(app, out);
            }
        }
        if down && !released {
            app.command.gizmo_drag = Some(drag);
            return true;
        }
        if !drag.ended && app.command.session.is_live() {
            let typed = app.command.session.dimensions().iter().any(|d| d.locked);
            if drag.moved || typed {
                let out = app.command.session.enter();
                outcome(app, out);
            } else {
                // Taken and let go in place: nothing changes.
                app.command.session.feed(StepInput::Cancel);
                app.command.target = None;
                app.set_status(format!("{}: {}", drag.gizmo.label(drag.handle), hint(drag.handle)));
            }
        }
        return true;
    }
    // A press with Shift or Alt held goes to the selection, handle or not.
    let selecting = ui.input(|i| i.modifiers.shift || i.modifiers.alt);
    let Some((id, gizmo)) = gizmo_of(app).filter(|_| !selecting) else {
        app.command.gizmo_hot = None;
        return false;
    };
    let layout = layout_of(app, pane, rect, &gizmo);
    app.command.gizmo_hot = hover.and_then(|p| layout.hit(p));
    let (pressed, origin) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.press_origin()));
    let Some(at) = origin.filter(|o| pressed && response.hovered() && rect.contains(*o)) else { return false };
    let Some(handle) = layout.hit(at) else { return false };
    start_drag(app, pane, rect, id, gizmo, handle, at, free);
    app.command.gizmo_drag.is_some()
}

/// An added primitive's press: at the first step it seats the base and its drag and release size it; later it is the step's click.
fn add_input(app: &mut RingDesignerApp, ui: &egui::Ui, pane: usize, rect: Rect, response: &egui::Response, free: bool) -> bool {
    let Some(step) = app.command.session.command().filter(|c| primitive(c.key()).is_some()).map(|c| c.step()) else {
        app.command.add_press = None;
        return false;
    };
    let (pressed, released, origin, pos) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.press_origin(), i.pointer.interact_pos()));
    let Some(began) = app.command.add_press else {
        let Some(at) = origin.filter(|o| pressed && response.hovered() && rect.contains(*o)) else { return false };
        if step == 0 {
            sample(app, pane, rect, at, free);
            let out = app.command.session.feed(StepInput::Click);
            let seated = matches!(out, Outcome::NextStep);
            outcome(app, out);
            if !seated {
                return false;
            }
        }
        app.command.add_press = Some(step);
        return true;
    };
    if let Some(p) = pos {
        let build = app.build.as_ref().map(build_key).unwrap_or(0);
        let moved = app.command.pointer.as_ref().is_none_or(|(q, b, _, _)| q.distance(p) > 0.25 || *b != build);
        if moved {
            sample(app, pane, rect, p, free);
        }
    }
    if released || !ui.input(|i| i.pointer.primary_down()) {
        app.command.add_press = None;
        // A plain click that seated the base leaves the size to the pointer; any other release is the step's click.
        if (began > 0 || !response.clicked()) && app.command.session.is_live() {
            let out = app.command.session.feed(StepInput::Click);
            outcome(app, out);
        }
    }
    true
}

/// Gives the viewport the keys while a command is live or a box is armed, and takes them back after.
fn hold_keys(app: &RingDesignerApp, ctx: &egui::Context, id: Id) {
    let holding = app.command.session.is_live() || app.command.box_armed;
    let focused = ctx.memory(|m| m.focused());
    if holding && !app.command.bar.has_focus(ctx) {
        if focused.is_none() {
            ctx.memory_mut(|m| m.request_focus(id));
        }
        ctx.memory_mut(|m| m.set_focus_lock_filter(id, EventFilter { tab: true, escape: true, horizontal_arrows: true, vertical_arrows: true }));
    } else if !holding && focused == Some(id) {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
}

/// The keys the viewport answers while it holds them: the ladder, the axis locks, and the hotkeys that start a command.
fn keys_of_frame(app: &mut RingDesignerApp, ui: &egui::Ui, over: bool) {
    let live = app.command.session.is_live();
    if live {
        if take(ui, Key::Escape, false) {
            let out = app.command.session.escape();
            outcome(app, out);
        }
        if take(ui, Key::Enter, false) {
            let out = app.command.session.enter();
            outcome(app, out);
        }
        let dims = app.command.session.dimensions();
        if let (Some(first), Some(last)) = (dims.first(), dims.last()) {
            if take(ui, Key::Tab, false) {
                app.command.bar.focus_field(ui.ctx(), first.key);
            } else if take(ui, Key::Tab, true) {
                app.command.bar.focus_field(ui.ctx(), last.key);
            }
        }
        for key in [Key::X, Key::Y, Key::Z] {
            if !take(ui, key, false) {
                continue;
            }
            let Some(axis) = app.command.session.command().and_then(|c| axis_for(c, key)) else { continue };
            let unlock = app.command.lock == Some(axis);
            let out = app.command.session.feed(if unlock { StepInput::Unlock } else { StepInput::Lock(axis) });
            if !out.is_refused() {
                app.command.lock = (!unlock).then_some(axis);
            }
            outcome(app, out);
        }
    } else if app.command.box_armed && take(ui, Key::Escape, false) {
        app.command.box_armed = false;
        app.command.box_from = None;
        app.set_status("Box select put away");
    }
    if !over || app.visual.tool != Tool::Select || app.command.holds_press() {
        return;
    }
    if take(ui, Key::A, true) {
        app.command.open_menu = true;
    }
    for (key, name) in [(Key::G, "move"), (Key::R, "rotate"), (Key::S, "scale"), (Key::P, "place"), (Key::J, "attach")] {
        if take(ui, key, false) {
            start(app, name);
        }
    }
    if take(ui, Key::B, false) && !app.command.session.is_live() {
        app.command.box_armed = true;
        app.command.box_from = None;
        app.set_status("Box select: drag left to right for a window, right to left for a crossing · Shift adds, Ctrl removes · Esc puts it away");
    }
}

/// Selects what a dragged box holds: a window left to right, a crossing right to left.
fn finish_box(app: &mut RingDesignerApp, pane: usize, rect: Rect, from: Pos2, to: Pos2, mods: Mods) {
    let r = Rect::from_two_pos(from, to);
    let Some(scene) = app.pick_scene.clone() else { return };
    if r.width() < MIN_BOX_PX || r.height() < MIN_BOX_PX {
        return;
    }
    let camera = app.panes[pane].camera;
    let ray = |p: Pos2| {
        let (o, d) = camera.ray(rect, p);
        Ray { origin: o.map(f64::from), direction: d.map(f64::from) }
    };
    let planes = box_planes([ray(r.left_top()), ray(r.right_top()), ray(r.right_bottom()), ray(r.left_bottom())]);
    let crossing = to.x < from.x;
    let caught = scene.box_select(planes, crossing, app.selection.filter);
    app.selection.boxed(&caught, mods);
    let n = app.selection.items.len();
    app.set_status(format!("{} box: {n} selected", if crossing { "Crossing" } else { "Window" }));
}

/// Stages the ghost once and sets this frame's matrix: the carried part to its preview, or a unit primitive sized and seated.
fn ghost(app: &mut RingDesignerApp) {
    let Some(build) = app.build.clone() else { return };
    let live = app.command.session.is_live();
    if !live {
        let done = app.command.linger.is_none_or(|(b, at)| b != build_key(&build) || at.elapsed() > LINGER);
        if done && (app.command.staged.is_some() || app.command.linger.is_some()) {
            app.command.staged = None;
            app.command.linger = None;
            app.command.tint.read = None;
            app.command.tint.at = None;
            if let Ok(mut r) = app.renderer.lock() {
                r.prepare_preview(Vec::new());
                r.set_preview_model(None);
                r.set_preview_draft(false);
            }
        }
        return;
    }
    let Some(preview) = app.command.session.preview() else { return };
    let band = band_for(app, &build);
    let key = app.command.session.command().map(|c| c.key()).unwrap_or_default();
    let (want, model) = match (primitive(key), &app.command.target) {
        (Some(kind), _) => (Staged::Unit(kind), unit_ghost(&app.design, band.as_deref(), &preview)),
        (None, Some(t)) => (Staged::Part { build: build_key(&build), feature: t.id }, placed_ghost(&app.design, band.as_deref(), t, &preview)),
        (None, None) => return,
    };
    if let Staged::Unit(kind) = want {
        let slot = &mut app.command.units[unit_slot(kind)];
        if slot.is_none() {
            *slot = unit_mesh(kind);
        }
    }
    let part = match want {
        Staged::Part { feature, .. } => build.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == feature)),
        Staged::Unit(_) => None,
    };
    // A reference stone is not read.
    let judge = (part.is_none_or(|c| !c.settings.reference) && model.is_some()).then(|| judge_for(app, &build, band.as_deref()));
    let mesh: Option<&Mesh> = match want {
        Staged::Unit(kind) => app.command.units[unit_slot(kind)].as_ref(),
        Staged::Part { .. } => part.map(|c| &c.mesh),
    };
    let mut restage = app.command.staged != Some(want);
    let tinted = match (mesh, model, judge) {
        (Some(mesh), Some(m), Some(judge)) => {
            if app.command.tint.at != Some((want, m)) {
                let read = judge.read(mesh, &m.0, part.is_some_and(|c| c.attach == Attach::Cut));
                restage |= app.command.tint.read.as_ref().is_none_or(|r| r.classes != read.classes);
                app.command.tint.read = Some(read);
                app.command.tint.at = Some((want, m));
                let sand = app.design.draft.process == CastProcess::SandTwoPart;
                app.command.tint.note = match (sand, part.map(|c| c.stage)) {
                    (false, _) => " (lost wax: read, not judged)",
                    (true, Some(Stage::Bench)) => " (bench: soldered on after the pour)",
                    _ => "",
                };
            }
            true
        }
        _ => {
            restage |= app.command.tint.read.is_some();
            app.command.tint.read = None;
            app.command.tint.at = None;
            false
        }
    };
    // Staged again when the part changes or the classes painting it do.
    if restage {
        let verts = match (mesh, app.command.tint.read.as_ref()) {
            (Some(mesh), Some(read)) => GpuMeshRenderer::stage_part_classes(mesh, &read.classes),
            (Some(mesh), None) => GpuMeshRenderer::stage_part(mesh),
            (None, _) => Vec::new(),
        };
        if let Ok(mut r) = app.renderer.lock() {
            r.prepare_preview(verts);
        }
        app.command.staged = Some(want);
    }
    if let Ok(mut r) = app.renderer.lock() {
        r.set_preview_model(model.as_ref().map(gl_model));
        r.set_preview_draft(tinted);
    }
}

/// The ghost's judge for this build: the field verdict's parting plane and the band's bore.
fn judge_for(app: &mut RingDesignerApp, build: &Arc<BuildResult>, band: Option<&BandSurface>) -> Arc<GhostJudge> {
    let at = build_key(build);
    if let Some((b, judge)) = &app.command.tint.judge
        && *b == at
    {
        return judge.clone();
    }
    let parting = app.field.as_ref().map_or(0.0, |f| f.parting_z_mm);
    let judge = Arc::new(GhostJudge::new(&app.design, band.map(BandSurface::mesh), parting));
    app.command.tint.judge = Some((at, judge.clone()));
    judge
}

/// A map as the renderer takes it: the column-major 4x4 and the normals' column-major 3x3.
pub fn gl_model(m: &Affine) -> ([f32; 16], [f32; 9]) {
    let c = m.cofactors();
    let sign = if m.determinant() < 0.0 { -1.0 } else { 1.0 };
    let mut n = [0.0f32; 9];
    for col in 0..3 {
        for row in 0..3 {
            n[col * 3 + row] = (c[row][col] * sign) as f32;
        }
    }
    (m.gl(), n)
}

/// Draws the rail on every Ring viewport, and on the active one the ghost, snap marker, caption, add menu and box.
pub fn draw(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, response: &egui::Response, painter: &egui::Painter, proj: &Projector, active: bool) {
    let rect = response.rect;
    rail(app, ui, pane, rect);
    if !active {
        // A viewport that is not the active pane lets go of the keys.
        if ui.ctx().memory(|m| m.focused()) == Some(response.id) {
            ui.ctx().memory_mut(|m| m.surrender_focus(response.id));
        }
        return;
    }
    ghost(app);
    add_menu(app, ui, pane);
    gizmo_draw(app, ui, pane, rect, painter);
    if let Some(from) = app.command.box_from
        && let Some(to) = ui.input(|i| i.pointer.latest_pos())
    {
        let r = Rect::from_two_pos(from, to);
        let window = to.x >= from.x;
        let stroke = egui::Stroke::new(1.5, ringdesign_workbench::hover::AQUA);
        painter.rect_filled(r, 0.0, ringdesign_workbench::hover::AQUA.gamma_multiply(0.08));
        if window {
            painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
        } else {
            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            painter.extend(egui::Shape::dashed_line(&corners, stroke, 6.0, 4.0));
        }
    }
    if app.command.box_armed {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let Some(cmd) = app.command.session.command() else { return };
    let preview = cmd.preview();
    let turning = app.command.gizmo_drag.as_ref().is_some_and(|d| matches!(d.handle, Handle::Turn(_)));
    // The anchor and the pointer the command reads from; a turning ring draws its own sweep.
    if let (false, [a, b, ..]) = (turning, preview.ghost.as_slice()) {
        let (pa, pb) = (proj.at(a.map(|v| v as f32)), proj.at(b.map(|v| v as f32)));
        painter.extend(egui::Shape::dashed_line(&[pa, pb], egui::Stroke::new(1.2, theme::ACCENT), 5.0, 4.0));
        painter.circle_filled(pa, 3.0, theme::ACCENT);
    }
    if let Some((_, _, _, Some(snap))) = &app.command.pointer {
        let p = proj.at(snap.world.map(|v| v as f32));
        if rect.contains(p) {
            let s = 5.0;
            painter.add(egui::Shape::closed_line(vec![p + egui::vec2(0.0, -s), p + egui::vec2(s, 0.0), p + egui::vec2(0.0, s), p + egui::vec2(-s, 0.0)], egui::Stroke::new(2.0, theme::ACCENT)));
            painter.text(p + egui::vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, &snap.label, egui::FontId::proportional(11.0), theme::ACCENT);
        }
    }
    let aqua = ringdesign_workbench::hover::AQUA;
    let mut lines = vec![(app.command.session.prompt(), aqua), (preview.caption.clone(), theme::TEXT)];
    // What the carried ghost would do in the sand, in the draft colours' own red and green.
    if let (Some(text), Some(read)) = (app.command.ghost_caption(), app.command.tint.read.as_ref()) {
        let class = if read.locks() { ringdesign_core::FaceClass::Undercut } else { ringdesign_core::FaceClass::Good };
        let [r, g, b] = class.rgb().map(|c| (c * 255.0).round() as u8);
        lines.push((text, egui::Color32::from_rgb(r, g, b)));
    }
    if let Some(hint) = axis_hint(cmd) {
        lines.push((format!("{hint} · Ctrl frees the snap · Esc backs out"), aqua));
    }
    let anchor = app.command.anchor.unwrap_or(rect.center());
    caption(painter, rect, anchor, &lines);
}

/// Runs `f` with the pane's screen as the gizmo reads it.
fn on_screen<R>(app: &RingDesignerApp, pane: usize, rect: Rect, f: impl FnOnce(&gizmo::View) -> R) -> R {
    let camera = app.panes[pane].camera;
    let proj = camera.projector(rect);
    let (view, ray) = ringdesign_workbench::hover::view_scale(rect.center(), &|p| camera.ray(rect, p));
    let project = |p: [f64; 3]| proj.at(p.map(|v| v as f32));
    f(&gizmo::View { project: &project, forward: ray.direction, px_per_mm: view.px_per_mm })
}

/// The gizmo over the metal: the dragged handle alone while one is held, else every handle of the chosen part, each named for a reader.
fn gizmo_draw(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, rect: Rect, painter: &egui::Painter) {
    if let Some(drag) = app.command.gizmo_drag.as_ref().filter(|d| !d.ended) {
        let (handle, mut gizmo) = (drag.handle, drag.gizmo.clone());
        let preview = app.command.session.preview().unwrap_or_default();
        // The dial's marker rides the angle being previewed.
        if let (Handle::Dial, Some(d), Some(theta)) = (handle, gizmo.dial.as_mut(), preview.placement.as_ref().and_then(Placement::theta_deg)) {
            d.theta_deg = theta;
        }
        on_screen(app, pane, rect, |view| {
            gizmo::paint(painter, &gizmo.layout(view).only(handle), None, Some(handle));
            if let [from, to, ..] = preview.ghost.as_slice() {
                gizmo::paint_sweep(painter, &gizmo, handle, *from, *to, view);
            }
        });
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return;
    }
    let Some((_, gizmo)) = gizmo_of(app) else {
        app.command.gizmo_hot = None;
        return;
    };
    let layout = layout_of(app, pane, rect, &gizmo);
    let hot = app.command.gizmo_hot;
    gizmo::paint(painter, &layout, hot, None);
    // Each handle a hover-only widget at its anchor, while an accessibility tree is being built.
    if ui.ctx().accesskit_node_builder(ui.id(), |_| ()).is_some() {
        for handle in layout.handles() {
            let (Some(at), label) = (layout.anchor(handle), gizmo.label(handle)) else { continue };
            let r = ui.interact(Rect::from_center_size(at, egui::Vec2::splat(12.0)), ui.id().with(("gizmo-handle", pane, label.as_str())), egui::Sense::hover());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.as_str()));
        }
    }
    if let Some(h) = hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        let text = match h {
            Handle::Grip(i) => gizmo.grips.get(i).map_or_else(String::new, |g| format!("{} {}", g.grip.label, g.grip.unit.format(g.grip.value))),
            _ => gizmo.label(h).trim_start_matches("Gizmo: ").to_owned(),
        };
        let overlay = rect.with_min_x(rect.left() + RAIL_W);
        ringdesign_workbench::hover::caption(painter, overlay, &text, ringdesign_workbench::hover::AQUA);
    }
}

/// The live command's words by the pointer: its prompt, its numbers and locks, what its ghost would do, and its keys.
fn caption(painter: &egui::Painter, rect: Rect, anchor: Pos2, lines: &[(String, egui::Color32)]) {
    let font = egui::FontId::proportional(12.0);
    let galleys: Vec<_> = lines.iter().filter(|(l, _)| !l.is_empty()).map(|(l, c)| painter.layout_no_wrap(l.clone(), font.clone(), *c)).collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y + 2.0).sum();
    let mut at = egui::pos2((anchor.x + 18.0).min(rect.right() - width - 8.0).max(rect.left() + 8.0), (anchor.y - 10.0 - height).max(rect.top() + 6.0));
    painter.rect_filled(Rect::from_min_size(at, egui::vec2(width, height)).expand(5.0), 4.0, egui::Color32::from_black_alpha(190));
    for g in galleys {
        let h = g.size().y + 2.0;
        painter.galley(at, g, theme::TEXT);
        at.y += h;
    }
}

/// Shift+A: an add menu at the pointer.
fn add_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    let open = std::mem::take(&mut app.command.open_menu);
    let popup = egui::Popup::new(menu_id(pane), ui.ctx().clone(), egui::PopupAnchor::PointerFixed, ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)));
    let mut chosen = None;
    popup.show(|ui| {
        ui.set_min_width(160.0);
        ui.weak("Add a part on the ring");
        for (key, icon, label) in [("add-box", Icon::CadBox, "Box"), ("add-cylinder", Icon::CadCylinder, "Cylinder"), ("add-sphere", Icon::CadSphere, "Sphere")] {
            if ui.add(egui::Button::image_and_text(icon.image(ui, 18.0), label)).on_hover_text("Click its centre on the ring, then drag or type its size").clicked() {
                chosen = Some(key);
                ui.close();
            }
        }
    });
    if let Some(key) = chosen {
        start(app, key);
    }
}

/// What a rail slot is called, to a reader and in its tooltip: the command and its key.
pub fn rail_label(title: &str, key: &str) -> String {
    format!("{title}  ({})", keys(key).unwrap_or(""))
}

/// The tool rail: every catalog command as its mark, down the viewport's left edge.
fn rail(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, rect: Rect) {
    let entries = catalog();
    let side = 30.0;
    let height = entries.len() as f32 * (side + 2.0) + 12.0;
    let top = (rect.center().y - height * 0.5).max(rect.top() + 8.0);
    let live = app.command.session.command().map(|c| c.key());
    let mut chosen = None;
    egui::Area::new(Id::new(("tool-rail", pane)))
        .order(egui::Order::Middle)
        .fixed_pos(egui::pos2(rect.left() + 5.0, top))
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).fill(theme::FLOAT).inner_margin(4).show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                for c in &entries {
                    let why = blocked(app, c.key);
                    let tip = rail_label(&c.title, c.key);
                    let button = egui::Button::image(c.icon.image(ui, 20.0)).selected(live == Some(c.key)).min_size(egui::vec2(side, side)).image_tint_follows_text_color(false);
                    let r = ui.add_enabled(why.is_none(), button);
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, why.is_none(), tip.as_str()));
                    let r = r.on_hover_text(&tip).on_disabled_hover_text(format!("{tip}\n{}", why.clone().unwrap_or_default()));
                    if r.clicked() {
                        chosen = Some(c.key);
                    }
                }
            });
        });
    if let Some(key) = chosen {
        app.active_pane = pane;
        start(app, key);
    }
}
