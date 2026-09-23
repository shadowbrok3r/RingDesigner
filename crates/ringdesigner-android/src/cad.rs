//! The desktop Ring viewport's CAD tools by touch: picking, the depth walk, the long-press menu, the strip, the gizmo, [`commit`].
pub mod command;
pub mod menu;
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
    touch::{self, DepthWalk, Signal, Tracker},
    viewport::{MenuAction, Mods, Sel, Selection, patterns as keys, pins::Pin},
};

use crate::camera::OrbitCamera;

/// The areas the CAD layer draws over the ring: its menu, a live command's caption and its dimension fields.
pub fn areas() -> [egui::Id; 3] {
    [menu::area(), command::caption_area(), command::fields_area()]
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
    /// Whether the view takes CAD gestures this frame: the Select tool out and nothing else holding the ring.
    pub active: bool,
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
        command::Ctx { rect: $v.rect, camera: $v.camera, design: $v.design, build: $v.build, band: $s.band.as_ref(), field: $v.field, pins: &$s.pins, selection: &$s.selection }
    };
}

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
    pub pins: Vec<Pin>,
    scene: Option<Arc<PickScene>>,
    band: Option<Arc<BandSurface>>,
    /// The build the scene and the band came with.
    build_key: usize,
    requests: Vec<Request>,
    /// The point on the ring under the finger that opened the last menu a row was chosen from.
    pressed: Option<[f64; 3]>,
}

impl Cad {
    /// A new build landed with its scene and ring frame; choices whose part is gone are let go.
    pub fn landed(&mut self, build: &Built, scene: Option<Arc<PickScene>>, band: Option<Arc<BandSurface>>, design: &RingDesign) {
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
        self.live.landed(build);
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

    /// Lets go of every choice, the depth walk and an open menu.
    pub fn clear(&mut self) {
        self.selection.clear();
        self.walk.forget();
        self.menu = None;
    }

    /// Chooses part `id` alone, as a tap on it would.
    pub fn choose(&mut self, id: Id) {
        self.selection.click(Some(Sel::Part(id)), Mods::default());
        self.walk.forget();
    }

    /// Everything under the finger at `p`, whole parts first.
    fn picks(&self, v: &View, p: Pos2) -> Vec<Pick> {
        let Some(scene) = &self.scene else { return Vec::new() };
        let ray = |q: Pos2| v.ray(q);
        let (view, r) = ringdesign_workbench::hover::view_scale(p, &ray);
        touch::coarse_first(scene.pick(r, &view, touch::APERTURE_PT, Filter::default()))
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
        let mut took = Took::default();
        for signal in signals {
            match signal {
                Signal::Press(p) => {
                    let over = ui.ctx().layer_id_at(p).is_some_and(|layer| layer != ui.layer_id());
                    self.owner = if !v.active || !v.rect.contains(p) || over || v.covered.iter().any(|r| r.contains(p)) { Owner::Elsewhere } else { Owner::Ring };
                    if self.owner == Owner::Ring && self.menu.take().is_some() {
                        // A press on the ring beside an open menu closes it and goes no further.
                        self.owner = Owner::Swallowed;
                    }
                    if self.owner == Owner::Ring {
                        let c = ctx!(self, v);
                        self.live.press(p, &c, &mut self.requests);
                    }
                }
                Signal::DragStart { at, .. } | Signal::DragMove { at } if self.owner == Owner::Ring => {
                    let c = ctx!(self, v);
                    self.live.drag(at, &c, &mut self.requests);
                }
                Signal::DragEnd { at } if self.owner == Owner::Ring => {
                    let c = ctx!(self, v);
                    self.live.release(at, true, &c, &mut self.requests);
                }
                Signal::Tap(p) => match self.owner {
                    Owner::Elsewhere => {}
                    Owner::Swallowed => took.tap = true,
                    Owner::Ring if self.live.tap_through() => took.tap = self.tap(v, p),
                    Owner::Ring if self.live.holding() => {
                        let c = ctx!(self, v);
                        self.live.release(p, false, &c, &mut self.requests);
                        took.tap = true;
                    }
                    Owner::Ring if self.live.is_live() => took.tap = true,
                    Owner::Ring => took.tap = self.tap(v, p),
                },
                Signal::LongPress(p) if self.owner == Owner::Ring && !self.live.is_live() => {
                    self.open_menu(v, p);
                    took.long_press = true;
                }
                Signal::Released if self.owner != Owner::Elsewhere => {
                    // A lift after a long press is not a tap, whatever the view's own click says.
                    took.tap = true;
                    if !self.live.tap_through() && self.live.holding() {
                        let c = ctx!(self, v);
                        self.live.release(v.rect.center(), false, &c, &mut self.requests);
                    }
                }
                Signal::Pinch | Signal::Cancel => {
                    if self.live.holding() {
                        self.live.let_go(&mut self.requests);
                    }
                }
                _ => {}
            }
        }
        took.hold = self.live.holding();
        took
    }

    /// Chooses the part, face, edge or vertex under a tap at `p`, one deeper on the same spot; whether the tap was taken.
    fn tap(&mut self, v: &View, p: Pos2) -> bool {
        let picks = self.picks(v, p);
        let cad = picks.first().is_some_and(|k| matches!(k.entity, Entity::Part { .. } | Entity::Face { .. } | Entity::Edge { .. } | Entity::Vertex { .. }));
        if !cad {
            self.walk.forget();
            self.selection.clear();
            return false;
        }
        let at = self.walk.tap(p, &picks).unwrap_or(0);
        let sel = Sel::of(&picks[at], |w| (w[1].atan2(w[0]).to_degrees(), 0.0));
        self.selection.click(Some(sel.clone()), Mods::default());
        self.selection.hovered(Vec::new());
        let (depth, of) = self.walk.depth();
        let what = ringdesign_workbench::viewport::selection::describe(&sel, v.design, v.build.map(|b| b.0.as_ref()));
        self.status(if of > 1 { format!("{what} · {} of {of}: tap the same spot for the next", depth + 1) } else { what });
        true
    }

    /// Opens the menu for the last thing chosen, as the selection bar's button asks, at the middle of the ring.
    pub fn open_for_choice(&mut self, v: &View) {
        if self.menu.is_some() {
            self.menu = None;
            return;
        }
        let items = menu::phone_items(ringdesign_workbench::viewport::context_items(&self.selection, None, v.design));
        let heading = ringdesign_workbench::viewport::heading(&self.selection, None, v.design);
        self.menu = Some(menu::Menu::new(v.rect.center(), heading, items));
    }

    /// Opens the menu for what lies under the finger at `p`.
    fn open_menu(&mut self, v: &View, p: Pos2) {
        let picks = self.picks(v, p);
        let under = touch::menu_pick(&picks, &self.selection.items).or_else(|| if self.scene.is_none() { Self::band_pick(v, p) } else { None });
        let items = menu::phone_items(ringdesign_workbench::viewport::context_items(&self.selection, under.as_ref(), v.design));
        let on_band = under.as_ref().is_some_and(|u| u.entity == Entity::Band);
        let heading = ringdesign_workbench::viewport::heading(&self.selection, under.as_ref(), v.design).or_else(|| {
            let mesh = v.build.filter(|_| on_band)?;
            let (origin, direction) = v.ray(p);
            let h = ringdesign_core::interaction::picking::hit(v.design, v.lib, mesh, origin, direction)?;
            Some(format!("Band at {:.0}° · wall {:.2} mm · relief {:+.2} mm", h.theta_deg, h.radial_wall_mm, h.relief_mm))
        });
        let mut menu = menu::Menu::new(p, heading, items);
        menu.world = under.map(|u| u.world);
        self.menu = Some(menu);
    }

    /// Draws the chosen edges and vertices, the pins, the gizmo, a live command's ghost and bars, and the menu, serving its row.
    pub fn draw(&mut self, ui: &mut egui::Ui, v: &View, renderer: &std::sync::Mutex<crate::viewport::GpuMeshRenderer>) {
        let projector = v.camera.projector(v.rect);
        let painter = ui.painter_at(v.rect);
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
        let c = ctx!(self, v);
        self.live.draw(ui, &c, renderer, &mut self.requests);
        let chosen = self.menu.as_mut().and_then(|m| menu::show(ui.ctx(), m, v.rect));
        match chosen {
            Some(menu::Choice::Act(action)) => {
                self.pressed = self.menu.take().and_then(|m| m.world);
                self.act(v, action);
            }
            Some(menu::Choice::Close) => self.menu = None,
            None => {}
        }
    }

    /// The chosen edges and vertices, a chosen band point, and the pins, drawn over the ring.
    fn draw_marks(&self, painter: &egui::Painter, v: &View, proj: &crate::camera::Projector) {
        let at = |p: [f64; 3]| proj.at(p.map(|x| x as f32));
        let evaluated = v.build.and_then(Built::evaluated);
        let color = egui::Color32::from_rgb(204, 146, 217);
        for item in &self.selection.items {
            match item {
                Sel::Edge { feature, edge } => {
                    if let Some(poly) = evaluated.and_then(|e| e.components.iter().find(|c| c.id == *feature)).and_then(|c| c.edges.get(*edge as usize)) {
                        painter.add(egui::Shape::line(poly.iter().map(|p| at(*p)).collect(), egui::Stroke::new(3.0, color)));
                    }
                }
                Sel::Vertex { feature, vertex } => {
                    if let Some(p) = evaluated.and_then(|e| e.components.iter().find(|c| c.id == *feature)).and_then(|c| c.trace.vertices.get(*vertex as usize)) {
                        painter.circle_stroke(at(*p), 7.0, egui::Stroke::new(3.0, color));
                    }
                }
                _ => {}
            }
        }
        for pin in &self.pins {
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
            MenuAction::AddPartHere { theta_deg, height_mm, label } => touch::parts::part_here(design, label, theta_deg, height_mm).map(|(edits, id)| edit(edits, Then::Part(id))),
            MenuAction::AddStone { theta_deg, key, .. } => touch::parts::stone_here(design, theta_deg, key).map(|(edits, id)| edit(edits, Then::Part(id))),
            MenuAction::AddStoneOnFace { feature, face, key } => {
                let built = v.build.and_then(|b| b.0.parts.evaluated.as_ref());
                touch::parts::stone_on_face(design, built, feature, face, self.pressed.take(), key).map(|(edits, id)| edit(edits, Then::Part(id)))
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
                let pin = Pin::at(world, ringdesign_workbench::viewport::pins::next_number(&self.pins));
                let said = format!("{} at {:.0}°", pin.name, pin.theta_deg);
                self.pins.push(pin);
                Ok(Request::Status(said))
            }
            MenuAction::ClearPins => {
                let n = self.pins.len();
                self.pins.clear();
                Ok(Request::Status(format!("{n} pin{} taken off the ring", if n == 1 { "" } else { "s" })))
            }
            MenuAction::Pattern { feature, key } => self.pattern(v, feature, key),
            MenuAction::PressPull { feature, face } => {
                let c = ctx!(self, v);
                self.live.press_pull(&c, feature, face).map(Request::Status)
            }
            MenuAction::IsolateInCad(_)
            | MenuAction::ToggleGrid
            | MenuAction::SketchOnFace { .. }
            | MenuAction::SketchOnPlane { .. }
            | MenuAction::CutHere { .. }
            | MenuAction::UnderStone { .. } => {
                unreachable!("not_here answered for it")
            }
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

/// Refuses to make a reference stone metal.
fn reference_refused(design: &RingDesign, id: Id) -> Result<(), String> {
    if design.cad.as_ref().and_then(|d| d.feature(id)).is_some_and(|f| f.component.reference) {
        return Err("A reference stone is never metal".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
