//! The Ring viewport's command session: hotkeys, the snapped pointer, the ghost, the dimension bar, the rail and box select.
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Event, EventFilter, Id, Key, Pos2, Rect};
use ringdesign_core::cad::edit::CadEdit;
use ringdesign_core::cad::{Attach, Component, Feature, Operation};
use ringdesign_core::interaction::pick::{Filter, Ray};
use ringdesign_core::{BuildResult, Mesh, sketch::Id as FeatureId};
use ringdesign_workbench::command::{
    AddPrimitiveCmd, Affine, AttachCmd, Axis, BandSurface, DimEvent, DimensionBar, Effect, Grid, MoveCmd, Outcome, PlaceCmd, Primitive, Probe,
    Reading, RotateCmd, ScaleCmd, Session, SnapGeometry, SnapHit, Snapper, StepInput, ViewCommand, catalog, placed_ghost, unit_ghost,
    unit_mesh,
};
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
    /// The band the ring frame is read on, by build and by the band's own inputs; `None` inside for parts only.
    band: Option<(usize, u64, Option<Arc<BandSurface>>)>,
    /// Part vertices and edges to snap to, by build and carried part.
    snaps: Option<(usize, Option<FeatureId>, Vec<[f64; 3]>, Vec<Vec<[f64; 3]>>)>,
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
            units: [None, None, None],
            staged: None,
            linger: None,
            pointer: None,
            anchor: None,
            box_from: None,
            open_menu: false,
        }
    }
}

#[cfg(test)]
impl CommandState {
    /// The band surface the ring frame was last read on, by address.
    pub fn band_surface(&self) -> Option<usize> {
        self.band.as_ref().and_then(|(_, _, b)| b.as_ref()).map(|b| Arc::as_ptr(b) as usize)
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
    let prompt = st.session.prompt();
    app.set_status(prompt);
    if let Some(build) = app.build.clone() {
        band_for(app, &build);
    }
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

/// The band for this build: its own mesh without parts, else a sweep without them kept while the band's inputs hold.
fn band_for(app: &mut RingDesignerApp, build: &Arc<BuildResult>) -> Option<Arc<BandSurface>> {
    let at = build_key(build);
    if let Some((b, _, band)) = &app.command.band
        && *b == at
    {
        return band.clone();
    }
    let mut bare = ringdesign_core::setting::without_solids(&app.design);
    bare.cad = None;
    bare.graph = None;
    let mut params = app.preview_params;
    if app.as_cast {
        params.soften_mm = app.design.draft.min_detail_mm;
    }
    let bare_band = build.parts.features.is_empty();
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_vec(&bare).unwrap_or_default().hash(&mut h);
        serde_json::to_vec(&params).unwrap_or_default().hash(&mut h);
        (Arc::as_ptr(&app.lib) as usize, app.design.band_is_procedural(), bare_band.then_some(at)).hash(&mut h);
        h.finish()
    };
    if let Some((_, k, band)) = &app.command.band
        && *k == key
    {
        let band = band.clone();
        app.command.band = Some((at, key, band.clone()));
        return band;
    }
    let band = if !app.design.band_is_procedural() {
        None
    } else if bare_band {
        Some(Arc::new(BandSurface::new(build.mesh.clone())))
    } else {
        ringdesign_core::mesh::try_build(&bare, &app.lib, params).ok().map(|b| Arc::new(BandSurface::new(b.mesh)))
    };
    app.command.band = Some((at, key, band.clone()));
    band
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
        k if primitive(k).is_some() && cmd.step() > 0 => cmd.preview().ghost.first().map_or(Reading::Surface, |at| Reading::Plane { at: *at }),
        _ => Reading::Surface,
    }
}

/// Reads the pointer at `pos` into the live command; `free` leaves the snaps off.
fn sample(app: &mut RingDesignerApp, pane: usize, rect: Rect, pos: Pos2, free: bool) {
    let (Some(scene), Some(build)) = (app.pick_scene.clone(), app.build.clone()) else { return };
    let band = band_for(app, &build);
    snaps_for(app, &build);
    let camera = app.panes[pane].camera;
    let at = |p: Pos2| camera.ray(rect, p);
    let (view, ray) = ringdesign_workbench::hover::view_scale(pos, &at);
    let picks = scene.pick(ray, &view, 0.0, Filter { vertices: false, edges: false, ..Filter::default() });
    let st = &app.command;
    let (vertices, edges) = st.snaps.as_ref().map_or((&[][..], &[][..]), |(_, _, v, e)| (v.as_slice(), e.as_slice()));
    let probe = Probe {
        design: &app.design,
        surface: band.as_deref(),
        snapper: (!free).then_some(st.snapper),
        geometry: SnapGeometry { vertices, edges },
        view,
        aperture_px: APERTURE_PX,
        carried: st.target.as_ref().map(|f| f.id),
    };
    let Some(token) = probe.token(reading(st), &picks, ray) else { return };
    let snap = match &token {
        StepInput::Pointer { snapped, .. } => snapped.clone(),
        _ => None,
    };
    let out = app.command.session.feed(token);
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
    if primitive(cmd.key()).is_some() {
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
    // The bar first: a field being typed in takes its own keys.
    if app.command.session.is_live() {
        let mut dims = app.command.session.dimensions();
        let anchor = app.command.anchor.unwrap_or(rect.center());
        for e in app.command.bar.show(&ctx, anchor, &mut dims) {
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
    let mut took = Took::default();
    // The pointer, then a click where it lands; the right button cancels.
    if app.command.session.is_live() {
        let free = ui.input(|i| i.modifiers.command);
        if let Some(pos) = hover {
            let build = app.build.as_ref().map(build_key).unwrap_or(0);
            let step = app.command.session.command().map_or(0, |c| c.step());
            let moved = app.command.pointer.as_ref().is_none_or(|(p, b, s, _)| p.distance(pos) > 0.25 || *b != build || *s != step);
            if moved {
                sample(app, pane, rect, pos, free);
            }
        }
        if response.clicked() {
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
    took.live = app.command.session.is_live();
    took.boxing = app.command.box_armed;
    took
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
    if !over || app.visual.tool != Tool::Select {
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
            if let Ok(mut r) = app.renderer.lock() {
                r.prepare_preview(Vec::new());
                r.set_preview_model(None);
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
    if app.command.staged != Some(want) {
        let verts = match want {
            Staged::Unit(kind) => {
                let slot = &mut app.command.units[unit_slot(kind)];
                if slot.is_none() {
                    *slot = unit_mesh(kind);
                }
                slot.as_ref().map(GpuMeshRenderer::stage_part).unwrap_or_default()
            }
            Staged::Part { feature, .. } => build
                .parts
                .evaluated
                .as_ref()
                .and_then(|e| e.components.iter().find(|c| c.id == feature))
                .map(|c| GpuMeshRenderer::stage_part(&c.mesh))
                .unwrap_or_default(),
        };
        if let Ok(mut r) = app.renderer.lock() {
            r.prepare_preview(verts);
        }
        app.command.staged = Some(want);
    }
    if let Ok(mut r) = app.renderer.lock() {
        r.set_preview_model(model.as_ref().map(gl_model));
    }
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
    // The anchor and the pointer the command reads from.
    if let [a, b, ..] = preview.ghost.as_slice() {
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
    let mut lines = vec![app.command.session.prompt(), preview.caption.clone()];
    if let Some(hint) = axis_hint(cmd) {
        lines.push(format!("{hint} · Ctrl frees the snap · Esc backs out"));
    }
    let anchor = app.command.anchor.unwrap_or(rect.center());
    caption(painter, rect, anchor, &lines);
}

/// The live command's words by the pointer: its prompt, its numbers and locks, and its keys.
fn caption(painter: &egui::Painter, rect: Rect, anchor: Pos2, lines: &[String]) {
    let font = egui::FontId::proportional(12.0);
    let galleys: Vec<_> = lines.iter().filter(|l| !l.is_empty()).enumerate().map(|(i, l)| painter.layout_no_wrap(l.clone(), font.clone(), if i == 1 { theme::TEXT } else { ringdesign_workbench::hover::AQUA })).collect();
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
