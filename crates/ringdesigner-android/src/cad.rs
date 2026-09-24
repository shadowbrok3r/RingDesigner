//! The desktop Ring viewport's CAD tools by touch: picking, the depth walk, the long-press menu, the strip, the gizmo, Measure, box select, work planes, sketching, [`commit`].
pub mod bar;
pub mod boxes;
pub mod command;
pub mod measure;
pub mod menu;
pub mod planes;
pub mod sketch;
pub mod stamp;
pub mod strip;

use std::sync::Arc;
use std::time::Duration;

use egui::{Pos2, Rect};
use egui_mobile::egui;
use ringdesign_core::{
    AlphaLibrary, BuildResult, Mesh, RingDesign,
    cad::{
        Attach, Evaluated, MirrorPlane,
        edit::{Applied, CadEdit},
    },
    castability::FieldReport,
    history::History,
    interaction::pick::{Entity, Filter, Pick, PickScene},
    sketch::Id,
};
use ringdesign_workbench::{
    command::BandSurface,
    touch::{self, DepthWalk, Signal, Tracker, boxes::BoxOp},
    viewport::{MenuAction, Mods, Sel, Selection, patterns as keys, pins::Pin},
};

use crate::camera::OrbitCamera;

/// The areas the CAD layer draws over the ring: its menus, a live command's caption and its dimension fields, a mode's bar, and a sketch's tools and fields.
pub fn areas() -> [egui::Id; 6] {
    let [tools, fields] = sketch::areas();
    [menu::area(), command::caption_area(), command::fields_area(), bar::area(), tools, fields]
}

/// Asks for the number keypad while a field of a live command's or a sketch's dimension bar holds the keyboard; whether it did.
pub fn keypad(ctx: &egui::Context) -> bool {
    crate::keypad::fields(ctx, &[command::fields_area(), sketch::fields_area()])
}

/// The stamp window's stamp once the stamps go from `before` to `after`: the same stamp found again, `None` once it is gone.
pub fn stamp_after(window: Option<usize>, before: &[ringdesign_core::setting::Stamp], after: &[ringdesign_core::setting::Stamp]) -> Option<usize> {
    window.and_then(|k| ringdesign_workbench::viewport::made::follow(before, after, k))
}

/// A settled build on screen: the mesh the view draws, with the parts it was made of.
#[derive(Clone)]
pub struct Built(pub Arc<BuildResult>);

impl std::ops::Deref for Built {
    type Target = Mesh;
    fn deref(&self) -> &Mesh {
        &self.0.mesh
    }
}

impl Built {
    /// The build's evaluation of the design's parts, when it has any.
    pub fn evaluated(&self) -> Option<&Evaluated> {
        self.0.parts.evaluated.as_ref()
    }

    /// The build by address, which a stage, a scene or a snap list is kept against.
    pub fn key(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

/// What one CAD commit made.
#[derive(Clone, Debug)]
pub struct Committed {
    pub applied: Vec<Applied>,
    pub label: String,
}

/// Makes `edits` the design as one undo step named by their labels, references signed against `evaluated`; a refusal changes nothing.
pub fn commit(design: &mut RingDesign, history: &mut History, edits: &[CadEdit], evaluated: Option<&Evaluated>) -> Result<Option<Committed>, String> {
    let Some(p) = touch::prepare(design, edits, evaluated)? else { return Ok(None) };
    history.commit(design);
    *design = p.design;
    history.commit_as(design, &p.label);
    Ok(Some(Committed { applied: p.applied, label: p.label }))
}

/// What a gesture asks the app to do that the CAD layer cannot do itself.
#[derive(Clone, Debug)]
pub enum Request {
    /// Edits through the funnel as one undo step, then a part to choose.
    Edit { edits: Vec<CadEdit>, then: Then },
    Status(String),
    FitView,
    ToggleWire,
    /// The phone's feature editor, the Workshop's CAD tab.
    OpenWorkshop,
    /// The Measure tool put away for Select.
    EndMeasure,
    /// A setting the app remembers changed, as the work planes' switch.
    Prefs,
    /// The camera eased to a pose, as a sketch squares the view to its plane.
    Look(ringdesign_workbench::focus::Pose),
    /// The view panned by a finger's travel over `rect`.
    Pan { by: egui::Vec2, rect: Rect },
    /// Part `Some(id)` added to the parts shown alone on the ring, or the whole ring again.
    Isolate(Option<Id>),
    /// Part `id` taken out of the parts shown alone; the whole ring again once none is left.
    TakeOut(Id),
    /// The design's measure pins, the whole list as it now stands.
    Pins(Vec<Pin>),
    /// A struck stamp changed or deleted, one undo step.
    Stamp { index: usize, edit: ringdesign_workbench::viewport::StampEdit },
    /// Stamp `index` open in the stamp window.
    EditStamp(usize),
    /// The layer a seat's made solid stands on, by its path.
    SeatLayer(Vec<usize>),
    /// Live cuts switched.
    LiveCuts,
    /// Show cutters switched.
    Cutters,
}

/// What to choose once an edit lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Keep,
    /// The part the last add in the edits made.
    LastAdded,
    Part(Id),
}

/// What the CAD layer made of a frame's gestures, so the view's own handling stands aside.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Took {
    /// A handle is held: no orbit and no pinch this frame.
    pub hold: bool,
    /// The tap was the CAD layer's, or ended a gesture that was not one: the view's own tap handling skips it.
    pub tap: bool,
    /// The long press opened the menu: the view's probe skips it.
    pub long_press: bool,
}

/// Whose the gesture in hand is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Owner {
    #[default]
    Ring,
    /// It began on something drawn over the ring, or with the CAD layer stood aside.
    Elsewhere,
    /// It began on the ring and is spent: a press that closed the menu, a finger that lifted a command's handle.
    Swallowed,
}

/// Everything a frame's gestures are read against.
pub struct View<'a> {
    pub rect: Rect,
    pub camera: &'a OrbitCamera,
    pub design: &'a RingDesign,
    pub lib: &'a AlphaLibrary,
    pub build: Option<&'a Built>,
    pub field: Option<&'a FieldReport>,
    /// Presses landing on these belong to what is drawn over the ring: the navigator, the floating tools.
    pub covered: &'a [Rect],
    /// Whether the view takes CAD gestures this frame: the Select or Measure tool out and nothing else holding the ring.
    pub active: bool,
    /// The Measure tool is out: a tap measures instead of choosing.
    pub measuring: bool,
    /// The build switches a seat's and a stamp's menu rows toggle.
    pub switches: ringdesign_workbench::viewport::Switches,
}

impl View<'_> {
    /// The world ray under a screen point.
    pub fn ray(&self, p: Pos2) -> ([f32; 3], [f32; 3]) {
        self.camera.ray(self.rect, p)
    }
}

/// A live command's view of the ring, borrowed field by field so the command itself stays free to change.
macro_rules! ctx {
    ($s:ident, $v:expr) => {
        $crate::cad::command::Ctx {
            rect: $v.rect,
            camera: $v.camera,
            design: $v.design,
            build: $v.build,
            band: $s.band.as_ref(),
            field: $v.field,
            pins: &$v.design.pins,
            selection: &$s.selection,
            gizmo: !$v.measuring && !$s.boxing.on,
        }
    };
}
pub(crate) use ctx;

/// The CAD layer over the phone's ring view.
#[derive(Default)]
pub struct Cad {
    pub selection: Selection,
    walk: DepthWalk,
    tracker: Tracker,
    /// The egui pass the tracker was last fed in.
    pass: Option<u64>,
    owner: Owner,
    pub menu: Option<menu::Menu>,
    pub live: command::Live,
    pub measuring: measure::Measuring,
    pub boxing: boxes::Boxing,
    pub planes: planes::Planes,
    scene: Option<Arc<PickScene>>,
    band: Option<Arc<BandSurface>>,
    /// The build the scene and the band came with.
    build_key: usize,
    requests: Vec<Request>,
    /// The point on the ring under the finger that opened the last menu a row was chosen from, with the surface's normal there.
    pressed: Option<([f64; 3], [f64; 3])>,
    /// The sketch being drawn, which takes the ring's gestures while it lives.
    sketch: Option<Box<sketch::Live>>,
    /// A Sketch feature to open on the next frame over the ring.
    open_pending: Option<Id>,
    /// The parts shown alone on the ring, as the app last said; none for the whole ring.
    pub isolated: Vec<Id>,
}

impl Cad {
    /// A new build landed with its scene and ring frame; choices whose part or plane is gone are let go.
    pub fn landed(&mut self, build: &Built, scene: Option<Arc<PickScene>>, band: Option<Arc<BandSurface>>, judge: Option<Arc<ringdesign_core::castability::ghost::GhostJudge>>, design: &RingDesign) {
        self.scene = scene;
        self.band = band;
        self.build_key = build.key();
        let doc = design.cad.as_ref();
        let gone = |s: &Sel| s.feature().is_some_and(|id| doc.and_then(|d| d.feature(id)).is_none());
        if self.selection.items.iter().any(gone) {
            let kept: Vec<Sel> = self.selection.items.iter().filter(|s| !gone(s)).cloned().collect();
            self.selection.clear();
            for s in kept {
                self.selection.click(Some(s), Mods { shift: true, ..Mods::default() });
            }
        }
        let plane_gone = |id: Id| doc.and_then(|d| d.feature(id)).is_none();
        if self.planes.chosen.is_some_and(plane_gone) {
            self.planes.chosen = None;
        }
        if self.planes.menu.as_ref().is_some_and(|m| plane_gone(m.plane)) {
            self.planes.menu = None;
        }
        self.live.landed(build, judge);
    }

    /// The pick scene over the build on screen.
    pub fn scene(&self) -> Option<&Arc<PickScene>> {
        self.scene.as_ref()
    }

    /// The ring frame parts are seated on.
    pub fn band(&self) -> Option<&Arc<BandSurface>> {
        self.band.as_ref()
    }

    /// What gestures asked for since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    fn status(&mut self, text: impl Into<String>) {
        self.requests.push(Request::Status(text.into()));
    }

    /// Lets go of every choice, the depth walk, an open menu, the chosen plane and the measurement.
    pub fn clear(&mut self) {
        self.selection.clear();
        self.walk.forget();
        self.menu = None;
        self.planes.chosen = None;
        self.planes.menu = None;
        self.measuring.measure.clear();
        self.measuring.missed = false;
    }

    /// Chooses part `id` alone, as a tap on it would.
    pub fn choose(&mut self, id: Id) {
        self.selection.click(Some(Sel::Part(id)), Mods::default());
        self.walk.forget();
    }

    /// Chooses what a landed edit made: a work plane on the planes layer, anything else as a part.
    pub fn choose_made(&mut self, design: &RingDesign, id: Id) {
        let plane = design.cad.as_ref().and_then(|d| d.feature(id)).is_some_and(|f| matches!(f.operation, ringdesign_core::cad::Operation::Plane { .. }));
        if plane {
            self.planes.chosen = Some(id);
            self.walk.forget();
        } else {
            self.choose(id);
        }
    }

    /// Everything under the finger at `p`, whole parts first.
    fn picks(&self, v: &View, p: Pos2) -> Vec<Pick> {
        self.picks_of(v, p, Filter::default())
    }

    /// What of `filter`'s classes lies under the finger at `p`, whole parts first.
    fn picks_of(&self, v: &View, p: Pos2, filter: Filter) -> Vec<Pick> {
        let Some(scene) = &self.scene else { return Vec::new() };
        let ray = |q: Pos2| v.ray(q);
        let (view, r) = ringdesign_workbench::hover::view_scale(p, &ray);
        touch::coarse_first(scene.pick(r, &view, touch::APERTURE_PT, filter))
    }

    /// The band under the finger when there is no scene to ask: the plain ring's own raycast.
    fn band_pick(v: &View, p: Pos2) -> Option<Pick> {
        let mesh = v.build?;
        let (origin, direction) = v.ray(p);
        let (face, point) = ringdesign_core::interaction::picking::raycast(mesh, origin, direction)?;
        let normal = mesh.face_normal(&mesh.faces[face]).unwrap_or([0.0, 0.0, 1.0]);
        Some(Pick { entity: Entity::Band, world: point.map(f64::from), normal, depth: 0.0, px: 0.0 })
    }

    /// Reads the frame's touches before the view orbits: a handle holds the view, a lift commits, a tap chooses, a long press opens the menu.
    pub fn frame(&mut self, ui: &egui::Ui, v: &View) -> Took {
        let (events, now, touching) = ui.input(|i| (i.events.clone(), i.time, i.any_touches() || i.pointer.any_down()));
        // A pass the ring view skipped drops the gesture in hand.
        let pass = ui.ctx().cumulative_pass_nr();
        let gap = self.pass.is_some_and(|p| p + 1 != pass);
        self.pass = Some(pass);
        let mut signals: Vec<Signal> = if gap { self.tracker.reset().into_iter().collect() } else { Vec::new() };
        signals.extend(self.tracker.feed_events(&events, now));
        signals.extend(self.tracker.settle(touching));
        if self.tracker.waiting() {
            ui.ctx().request_repaint_after(Duration::from_millis(40));
        }
        if let Some(id) = self.open_pending.take() {
            self.open_sketch(v, id);
        }
        // A live sketch takes every gesture on the ring.
        if self.sketch.is_some() {
            return self.sketch_gestures(ui, v, signals);
        }
        let mut took = Took::default();
        // A live command ends when Measure or box select takes the ring.
        let boxing = self.boxing.on && !v.measuring;
        if (v.measuring || boxing) && self.live.is_live() {
            self.live.cancel(&mut self.requests);
        }
        for signal in signals {
            match signal {
                Signal::Press(p) => {
                    let over = ui.ctx().layer_id_at(p).is_some_and(|layer| layer != ui.layer_id());
                    self.owner = if !v.active || !v.rect.contains(p) || over || v.covered.iter().any(|r| r.contains(p)) { Owner::Elsewhere } else { Owner::Ring };                    if self.owner == Owner::Ring {
                        // A press on the ring beside an open menu closes it and goes no further.
                        let ring_menu = self.menu.take().is_some();
                        let plane_menu = self.planes.menu.take().is_some();
                        if ring_menu || plane_menu {
                            self.owner = Owner::Swallowed;
                        }
                    }
                    if self.owner == Owner::Ring {
                        if boxing {
                            self.boxing.press();
                        } else if !v.measuring {
                            let c = ctx!(self, v);
                            self.live.press(p, &c, &mut self.requests);
                        }
                    }
                }
                Signal::DragStart { from, at } if self.owner == Owner::Ring => {
                    if !self.boxing.start(from, at) {
                        let c = ctx!(self, v);
                        self.live.drag(at, &c, &mut self.requests);
                    }
                }
                Signal::DragMove { at } if self.owner == Owner::Ring => {
                    if !self.boxing.follow(at) {
                        let c = ctx!(self, v);
                        self.live.drag(at, &c, &mut self.requests);
                    }
                }
                Signal::DragEnd { at } if self.owner == Owner::Ring => match self.boxing.lift(at) {
                    Some(d) => {
                        let said = self.finish_box(v, d);
                        self.status(said);
                    }
                    None => {
                        let c = ctx!(self, v);
                        self.live.release(at, true, &c, &mut self.requests);
                    }
                },
                Signal::Tap(p) => {
                    self.boxing.let_go();
                    match self.owner {
                        Owner::Elsewhere => {}
                        Owner::Swallowed => took.tap = true,
                        Owner::Ring if v.measuring => took.tap = self.measure_tap(v, p),
                        Owner::Ring if self.live.tap_through() => took.tap = self.tap(ui.painter(), v, p),
                        Owner::Ring if self.live.holding() => {
                            let c = ctx!(self, v);
                            self.live.release(p, false, &c, &mut self.requests);
                            took.tap = true;
                        }
                        Owner::Ring if self.live.is_live() => took.tap = true,
                        Owner::Ring => took.tap = self.tap(ui.painter(), v, p),
                    }
                }
                Signal::LongPress(p) if self.owner == Owner::Ring && !self.live.is_live() => {
                    self.boxing.let_go();
                    let on_part = self.picks(v, p).first().is_some_and(|k| is_part(&k.entity));
                    match self.plane_at(ui.painter(), v, p, on_part).filter(|_| !v.measuring) {
                        Some(plane) => self.open_plane_menu(v, plane, p),
                        None => self.open_menu(v, p),
                    }
                    took.long_press = true;
                }
                Signal::Released if self.owner != Owner::Elsewhere => {
                    // A lift after a long press is not a tap, whatever the view's own click says.
                    took.tap = true;
                    self.boxing.let_go();
                    // A finger held still over a primitive being dragged out leaves its size alone.
                    if self.live.sizing() {
                        self.live.let_go(&mut self.requests);
                    } else if !self.live.tap_through() && self.live.holding() {
                        let c = ctx!(self, v);
                        self.live.release(v.rect.center(), false, &c, &mut self.requests);
                    }
                }
                Signal::Pinch | Signal::Cancel => {
                    self.boxing.let_go();
                    if self.live.holding() {
                        self.live.let_go(&mut self.requests);
                    }
                }
                _ => {}
            }
        }
        took.hold = self.live.holding() || self.boxing.holding();
        took
    }

    /// Chooses the plane, part, face, edge or vertex under a tap at `p`, one deeper on the same spot; with box select on, only what it takes, and its op adds or takes away; whether the tap was taken.
    fn tap(&mut self, painter: &egui::Painter, v: &View, p: Pos2) -> bool {
        let picks = if self.boxing.on { self.picks_of(v, p, self.boxing.takes.filter()) } else { self.picks(v, p) };
        let cad = picks.first().is_some_and(|k| is_part(&k.entity));
        if let Some(plane) = self.plane_at(painter, v, p, cad) {
            self.choose_plane(v, plane);
            return true;
        }
        self.planes.chosen = None;
        let op = if self.boxing.on { self.boxing.op } else { BoxOp::Replace };
        if !cad {
            self.walk.forget();
            if op == BoxOp::Replace {
                self.selection.clear();
            }
            return false;
        }
        let at = if op == BoxOp::Replace {
            self.walk.tap(p, &picks).unwrap_or(0)
        } else {
            self.walk.forget();
            0
        };
        let sel = Sel::of(&picks[at], |w| (w[1].atan2(w[0]).to_degrees(), 0.0));
        self.selection.click(Some(sel.clone()), op.mods());
        self.selection.hovered(Vec::new());
        let (depth, of) = self.walk.depth();
        let what = ringdesign_workbench::viewport::selection::describe(&sel, v.design, v.build.map(|b| b.0.as_ref()));
        let n = self.selection.items.len();
        self.status(match op {
            BoxOp::Add => format!("{what} added · {n} chosen"),
            BoxOp::Remove => format!("{what} taken out · {n} chosen"),
            BoxOp::Replace if of > 1 => format!("{what} · {} of {of}: tap the same spot for the next", depth + 1),
            BoxOp::Replace => what,
        });
        true
    }

    /// Opens the menu for the last thing chosen, as the selection bar's button asks, at the middle of the ring.
    pub fn open_for_choice(&mut self, v: &View) {
        self.planes.menu = None;
        if self.menu.is_some() {
            self.menu = None;
            return;
        }
        let items = menu::phone_items(ringdesign_workbench::viewport::context_items_in(&self.selection, None, v.design, v.switches), &self.isolated);
        let heading = ringdesign_workbench::viewport::heading(&self.selection, None, v.design);
        let mut menu = menu::Menu::new(v.rect.center(), heading, items);
        menu.extras = menu::extras(None, self.selection.items.last(), v.build.and_then(Built::evaluated), &self.isolated);
        self.menu = Some(menu);
    }

    /// Opens the menu for what lies under the finger at `p`.
    fn open_menu(&mut self, v: &View, p: Pos2) {
        self.planes.menu = None;
        let picks = self.picks(v, p);
        let under = touch::menu_pick(&picks, &self.selection.items).or_else(|| if self.scene.is_none() { Self::band_pick(v, p) } else { None });
        let items = menu::phone_items(ringdesign_workbench::viewport::context_items_in(&self.selection, under.as_ref(), v.design, v.switches), &self.isolated);
        let on_band = under.as_ref().is_some_and(|u| u.entity == Entity::Band);
        let heading = ringdesign_workbench::viewport::heading(&self.selection, under.as_ref(), v.design).or_else(|| {
            let mesh = v.build.filter(|_| on_band)?;
            let (origin, direction) = v.ray(p);
            let h = ringdesign_core::interaction::picking::hit(v.design, v.lib, mesh, origin, direction)?;
            Some(format!("Band at {:.0}° · wall {:.2} mm · relief {:+.2} mm", h.theta_deg, h.radial_wall_mm, h.relief_mm))
        });
        let mut menu = menu::Menu::new(p, heading, items);
        menu.extras = menu::extras(under.as_ref(), self.selection.items.last(), v.build.and_then(Built::evaluated), &self.isolated);
        menu.under = under.map(|u| (u.world, u.normal));
        self.menu = Some(menu);
    }

    /// Serves one of the phone's own menu rows: a work plane waits for its number, the whole ring comes back.
    fn extra(&mut self, v: &View, e: menu::Extra) {
        match e {
            menu::Extra::PlaneOnFace { feature, face } => {
                let part = v.design.cad.as_ref().and_then(|d| d.feature(feature)).map(|f| f.name.clone());
                let c = v.build.and_then(Built::evaluated).and_then(|e| e.components.iter().find(|c| c.id == feature));
                let (Some(name), Some(c)) = (part, c) else { return self.status(format!("Part #{feature} is not on the ring as built")) };
                match ringdesign_core::cad::pattern::planar_face(c, face) {
                    Ok((signed, centre, normal)) => {
                        let cx = ctx!(self, v);
                        let said = self.live.plane(&cx, touch::planes::PlaneCmd::on_face(feature, signed, centre, normal, &name));
                        self.status(said);
                    }
                    Err(e) => self.status(format!("{e:#}")),
                }
            }
            menu::Extra::PlaneAtAngle { theta_deg } => {
                let cx = ctx!(self, v);
                let said = self.live.plane(&cx, touch::planes::PlaneCmd::at_angle(theta_deg));
                self.status(said);
            }
            menu::Extra::PlaneTangent { at, normal } => {
                let cx = ctx!(self, v);
                let said = self.live.plane(&cx, touch::planes::PlaneCmd::tangent(at, normal));
                self.status(said);
            }
            menu::Extra::PlaneParting { at } => {
                // It starts where the verdict parts the mould.
                let parting = v.field.map_or(0.0, |f| f.parting_z_mm);
                let cx = ctx!(self, v);
                let said = self.live.plane(&cx, touch::planes::PlaneCmd::parting(at, parting));
                self.status(said);
            }
            menu::Extra::ShowAll => self.requests.push(Request::Isolate(None)),
            menu::Extra::TakeOut(id) => self.requests.push(Request::TakeOut(id)),
        }
    }

    /// Draws the work planes, the chosen edges and vertices, the pins, the gizmo, a live command's ghost and bars, a measurement or a box and its mode's bar, and the menus, serving their rows.
    pub fn draw(&mut self, ui: &mut egui::Ui, v: &View, renderer: &std::sync::Mutex<crate::viewport::GpuMeshRenderer>) {
        let projector = v.camera.projector(v.rect);
        let painter = ui.painter_at(v.rect);
        self.draw_planes(&painter, v);
        if self.sketch.is_some() {
            if let Some(build) = v.build {
                let (chosen, hovered) = lit_edges(&self.selection);
                if renderer.lock().is_ok_and(|mut r| r.sync_edges(&build.0, &chosen, hovered)) {
                    ui.ctx().request_repaint();
                }
            }
            self.draw_sketch(ui, v);
            keypad(ui.ctx());
            return;
        }
        self.draw_marks(&painter, v, &projector);
        if let Some(build) = v.build
            && self.selection.needs_stage(build.key())
        {
            let weights = ringdesign_workbench::viewport::tint(&self.selection, &build.0);
            let staged = if weights.is_empty() { Vec::new() } else { crate::viewport::GpuMeshRenderer::stage_select(build, &weights) };
            if let Ok(mut r) = renderer.lock() {
                r.set_pending_select(staged);
            }
            ui.ctx().request_repaint();
        }
        // The edge pass follows the build on screen and the chosen and pressed edges.
        if let Some(build) = v.build {
            let (chosen, hovered) = lit_edges(&self.selection);
            if renderer.lock().is_ok_and(|mut r| r.sync_edges(&build.0, &chosen, hovered)) {
                ui.ctx().request_repaint();
            }
        }
        let c = ctx!(self, v);
        self.live.draw(ui, &c, renderer, &mut self.requests);
        if v.measuring {
            self.draw_measure(&painter, v);
        }
        self.draw_box(&painter);
        if !self.live.is_live() {
            if v.measuring {
                self.measure_bar(ui.ctx(), v);
            } else if self.boxing.on {
                self.box_bar(ui.ctx(), v);
            }
        }
        let chosen = self.menu.as_mut().and_then(|m| menu::show(ui.ctx(), m, v.rect));
        match chosen {
            Some(menu::Choice::Act(action)) => {
                self.pressed = self.menu.take().and_then(|m| m.under);
                self.act(v, action);
            }
            Some(menu::Choice::Extra(e)) => {
                self.menu = None;
                self.extra(v, e);
            }
            Some(menu::Choice::Close) => self.menu = None,
            None => {}
        }
        match self.planes.menu.as_mut().and_then(|m| planes::show(ui.ctx(), m, v.rect)) {
            Some(planes::Choice::Act(act)) => {
                if let Some(m) = self.planes.menu.take() {
                    self.plane_act(v, m.plane, act);
                }
            }
            Some(planes::Choice::Close) => self.planes.menu = None,
            None => {}
        }
        keypad(ui.ctx());
    }

    /// The chosen vertices and the pins, drawn over the ring; the edge pass lights the chosen edges.
    fn draw_marks(&self, painter: &egui::Painter, v: &View, proj: &crate::camera::Projector) {
        let at = |p: [f64; 3]| proj.at(p.map(|x| x as f32));
        let evaluated = v.build.and_then(Built::evaluated);
        let color = egui::Color32::from_rgb(204, 146, 217);
        for item in &self.selection.items {
            match item {
                Sel::Vertex { feature, vertex } => {
                    if let Some(p) = evaluated.and_then(|e| e.components.iter().find(|c| c.id == *feature)).and_then(|c| c.trace.vertices.get(*vertex as usize)) {
                        painter.circle_stroke(at(*p), 7.0, egui::Stroke::new(3.0, color));
                    }
                }
                _ => {}
            }
        }
        for pin in &v.design.pins {
            let p = at(pin.world);
            if v.rect.contains(p) {
                painter.circle(p, 5.0, crate::theme::AQUA, egui::Stroke::new(1.5, egui::Color32::BLACK));
                painter.text(p + egui::vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, &pin.name, egui::FontId::proportional(11.0), crate::theme::AQUA);
            }
        }
    }

    /// Serves a menu row: edits leave as requests for the funnel, the rest acts on the view or says why not.
    pub fn act(&mut self, v: &View, action: MenuAction) {
        if let Some(why) = menu::not_here(&action) {
            self.status(why);
            return;
        }
        let design = v.design;
        let edit = |edits: Vec<CadEdit>, then: Then| Request::Edit { edits, then };
        let request = match action {
            // A box, a cylinder or a sphere is dragged out from where the band was pressed; anything else lands there as it is.
            MenuAction::AddPartHere { label, .. } if touch::primitive::kind(label).is_some() && self.pressed.is_some() => {
                let (Some(kind), Some((at, normal))) = (touch::primitive::kind(label), self.pressed.take()) else { return };
                let cx = ctx!(self, v);
                self.live.add_primitive(&cx, kind, at, normal).map(Request::Status)
            }
            MenuAction::AddPartHere { theta_deg, height_mm, label } => touch::parts::part_here(design, label, theta_deg, height_mm).map(|(edits, id)| edit(edits, Then::Part(id))),
            MenuAction::AddStone { theta_deg, key, .. } => touch::parts::stone_here(design, theta_deg, key).map(|(edits, id)| edit(edits, Then::Part(id))),
            MenuAction::AddStoneOnFace { feature, face, key } => {
                let built = v.build.and_then(|b| b.0.parts.evaluated.as_ref());
                touch::parts::stone_on_face(design, built, feature, face, self.pressed.take().map(|(at, _)| at), key).map(|(edits, id)| edit(edits, Then::Part(id)))
            }
            MenuAction::CutHere { theta_deg, across_mm, key } => {
                ringdesign_workbench::viewport::cutters::cut_here(design, theta_deg, across_mm, self.pressed.take(), key).map(|(edits, id)| edit(edits, Then::Part(id)))
            }
            MenuAction::UnderStone { stone, key } => {
                let built = v.build.and_then(|b| b.0.parts.evaluated.as_ref());
                ringdesign_workbench::viewport::cutters::under_stone(design, built, stone, key).map(|(edits, id)| edit(edits, Then::Part(id)))
            }
            MenuAction::Setting { part, stone, key } => touch::parts::setting(design, v.build.map(|b| &b.0.mesh), part, stone.as_deref(), key).map(|(edits, head)| edit(edits, head.map_or(Then::Keep, Then::Part))),
            MenuAction::Attach(id, attach) => reference_refused(design, id).map(|()| edit(vec![CadEdit::Attach { id, attach }], Then::Keep)),
            MenuAction::Stage(id, stage) => reference_refused(design, id).map(|()| edit(vec![CadEdit::Stage { id, stage }], Then::Keep)),
            MenuAction::FilletEdge { feature, edge } => touch::parts::modifier(design, "Fillet", feature, edge).map(|e| edit(vec![e], Then::LastAdded)),
            MenuAction::ChamferEdge { feature, edge } => touch::parts::modifier(design, "Chamfer", feature, edge).map(|e| edit(vec![e], Then::LastAdded)),
            MenuAction::EditFeature(id) => {
                self.choose(id);
                self.requests.push(Request::OpenWorkshop);
                Ok(Request::Status(format!("{}: edit its numbers on the Workshop's CAD tab", ringdesign_workbench::viewport::selection::feature_name(id, design, v.build.map(|b| b.0.as_ref())))))
            }
            MenuAction::OpenCad => Ok(Request::OpenWorkshop),
            MenuAction::FitView => Ok(Request::FitView),
            MenuAction::ToggleWire => Ok(Request::ToggleWire),
            MenuAction::PinHere { world } => {
                let mut pins = v.design.pins.clone();
                let pin = Pin::at(world, ringdesign_workbench::viewport::pins::next_number(&pins));
                let said = format!("{} at {:.0}°", pin.name, pin.theta_deg);
                pins.push(pin);
                self.requests.push(Request::Pins(pins));
                Ok(Request::Status(said))
            }
            MenuAction::ClearPins => {
                let n = v.design.pins.len();
                self.requests.push(Request::Pins(Vec::new()));
                Ok(Request::Status(format!("{n} pin{} taken off the ring", if n == 1 { "" } else { "s" })))
            }
            MenuAction::Pattern { feature, key } => self.pattern(v, feature, key),
            MenuAction::PressPull { feature, face } => {
                let c = ctx!(self, v);
                self.live.press_pull(&c, feature, face).map(Request::Status)
            }
            MenuAction::SketchOnFace { feature, face } => {
                self.start_sketch(v, touch::sketch::Place::Face { feature, face });
                return;
            }
            MenuAction::SketchOnPlane { theta_deg, across_mm } => {
                self.start_sketch(v, touch::sketch::Place::Tangent { theta_deg, across_mm });
                return;
            }
            MenuAction::IsolateInCad(id) => Ok(Request::Isolate(Some(id))),
            MenuAction::ToggleGrid => unreachable!("not_here answered for it"),
            MenuAction::SeatLayer(path) => Ok(Request::SeatLayer(path)),
            MenuAction::ToggleLiveCuts => Ok(Request::LiveCuts),
            MenuAction::ToggleCutters => Ok(Request::Cutters),
            MenuAction::EditStamp(index) => Ok(Request::EditStamp(index)),
            MenuAction::Stamp { index, edit } => Ok(Request::Stamp { index, edit }),
        };
        match request {
            Ok(r) => self.requests.push(r),
            Err(why) => self.status(why),
        }
    }

    /// An array or a mirror of part `feature`: a mirror lands at once, an array waits for its count.
    fn pattern(&mut self, v: &View, feature: Id, key: &'static str) -> Result<Request, String> {
        let design = v.design;
        let f = design.cad.as_ref().and_then(|d| d.feature(feature)).cloned().ok_or_else(|| format!("Part #{feature} is not in the document"))?;
        let evaluated = v.build.and_then(Built::evaluated).ok_or("The ring has not built yet; try once it has")?;
        let c = evaluated.components.iter().find(|c| c.id == feature).ok_or_else(|| format!("#{feature} {} did not build; mend it on the strip first", f.name))?;
        if f.component.reference {
            return Err("A reference stone is patterned with its setting; pattern the setting instead".into());
        }
        let attach: Attach = c.attach;
        match key {
            keys::MIRROR_BAND => touch::parts::mirror(design, &f, attach, MirrorPlane::Band).map(|e| Request::Edit { edits: vec![e], then: Then::LastAdded }),
            keys::MIRROR_HEAD => touch::parts::mirror(design, &f, attach, MirrorPlane::Section { theta_deg: design.shank.head.theta_deg }).map(|e| Request::Edit { edits: vec![e], then: Then::LastAdded }),
            keys::RING_ARRAY | keys::STONE_ARRAY => {
                let about = if key == keys::STONE_ARRAY {
                    Some(touch::parts::stone_by(design, evaluated, &f, c).ok_or_else(|| format!("#{} {} stands on no stone and by none within {} mm; set a stone first", f.id, f.name, touch::parts::STONE_REACH_MM))?)
                } else {
                    None
                };
                let cx = ctx!(self, v);
                Ok(Request::Status(self.live.array(&cx, f, attach, about)))
            }
            _ => Err(format!("No pattern called {key}")),
        }
    }
}

/// Whether a pick is a part or one of its faces, edges or vertices.
fn is_part(e: &Entity) -> bool {
    matches!(e, Entity::Part { .. } | Entity::Face { .. } | Entity::Edge { .. } | Entity::Vertex { .. })
}

/// The chosen edges and the edge under the finger, as the edge pass lights them.
fn lit_edges(selection: &Selection) -> (Vec<ringdesign_workbench::render::EdgeKey>, Option<ringdesign_workbench::render::EdgeKey>) {
    let chosen = selection.items.iter().filter_map(|s| match s {
        Sel::Edge { feature, edge } => Some((*feature, *edge)),
        _ => None,
    });
    let hovered = selection.hover.as_ref().and_then(|h| match h.entity {
        Entity::Edge { feature, edge } => Some((feature, edge)),
        _ => None,
    });
    (chosen.collect(), hovered)
}

/// Refuses to make a reference stone metal.
fn reference_refused(design: &RingDesign, id: Id) -> Result<(), String> {
    if design.cad.as_ref().and_then(|d| d.feature(id)).is_some_and(|f| f.component.reference) {
        return Err("A reference stone is never metal".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
