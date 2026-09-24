//! Work planes on the phone's ring: drawn as translucent named rectangles, a tap chooses one, a long press opens its menu.
use egui::{Pos2, Rect};
use egui_mobile::egui;
use ringdesign_core::{RingDesign, cad::MirrorPlane, sketch::Id};
use ringdesign_workbench::{
    icons::Icon,
    touch::{self, planes::Drawn},
    viewport::{Sel, Selection},
};

use super::{Cad, Request, Then, View, menu};

/// The colour of a chosen plane, the viewport's selection colour.
const CHOSEN: egui::Color32 = egui::Color32::from_rgb(204, 146, 217);
/// Size of a plane's name, points.
const NAME_PT: f32 = 12.0;

/// Whether the planes are drawn, the one chosen, and the menu a long press opened on one.
#[derive(Clone, Debug, Default)]
pub struct Planes {
    pub hidden: bool,
    pub chosen: Option<Id>,
    pub menu: Option<PlaneMenu>,
}

/// What a row of a plane's menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// A new sketch lying on the plane.
    Sketch,
    /// The chosen part reflected across the plane.
    Mirror,
    /// Stop drawing the planes.
    Hide,
}

/// One row of a plane's menu: its mark and words, what it does, and why not when it is not offered.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub icon: Icon,
    pub label: &'static str,
    pub act: Act,
    pub enabled: bool,
    pub hint: String,
}

/// A long press's menu on work plane `plane`.
#[derive(Clone, Debug)]
pub struct PlaneMenu {
    pub plane: Id,
    /// Where the finger was.
    pub at: Pos2,
    pub heading: String,
    pub rows: Vec<Row>,
    /// The popup as last drawn.
    pub rect: Rect,
    /// Whether the popup hangs below the finger, settled when it is first drawn.
    pub below: Option<bool>,
}

/// The part a plane's mirror reflects: the last one chosen, other than the plane, with a body of its own and not a reference stone; else why not.
pub fn mirrorable(design: &RingDesign, selection: &Selection, plane: Id) -> Result<Id, String> {
    let doc = design.cad.as_ref();
    let part = selection.items.iter().rev().find_map(Sel::feature).filter(|id| *id != plane).and_then(|id| doc?.feature(id));
    match part {
        None => Err("Choose a part on the ring first, then hold the plane".into()),
        Some(f) if f.component.reference => Err("A reference stone is patterned with its setting; choose the setting".into()),
        Some(f) if !f.operation.has_body() => Err(format!("{} has no body to mirror", f.name)),
        Some(f) => Ok(f.id),
    }
}

/// What a plane's menu offers for `plane` with `selection` chosen: sketching on it, mirroring the chosen part across it, hiding the planes.
pub fn rows(design: &RingDesign, selection: &Selection, plane: Id) -> Vec<Row> {
    let mirror = mirrorable(design, selection, plane);
    let hint = match &mirror {
        Ok(id) => format!("{} reflected across {}, one new Mirror feature", name_of(design, *id), name_of(design, plane)),
        Err(why) => why.clone(),
    };
    vec![
        Row { icon: Icon::CadSketch, label: "Sketch on this plane", act: Act::Sketch, enabled: true, hint: "A new sketch lying on this plane, drawn by one finger".to_string() },
        Row { icon: Icon::Mirror, label: "Mirror the chosen part across it", act: Act::Mirror, enabled: mirror.is_ok(), hint },
        Row { icon: Icon::Guides, label: "Hide work planes", act: Act::Hide, enabled: true, hint: "Stop drawing the work planes; the View menu shows them again".to_string() },
    ]
}

/// What a plane's menu did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Act(Act),
    Close,
}

/// Draws a plane's menu by the finger, kept inside `view`; the choice a row made.
pub fn show(ctx: &egui::Context, m: &mut PlaneMenu, view: Rect) -> Option<Choice> {
    let mut chosen = None;
    let rows = m.rows.clone();
    m.rect = menu::popup(ctx, m.at, &mut m.below, Some(&m.heading), rows.len(), view, |ui, width| {
        for r in &rows {
            if menu::row(ui, r.icon, r.label, false, r.enabled, width).clicked() {
                chosen = Some(Choice::Act(r.act));
            }
            if !r.enabled {
                ui.label(egui::RichText::new(&r.hint).size(11.0).color(crate::theme::INK_DIM));
            }
        }
        if menu::row(ui, Icon::Close, "Close", false, true, width).clicked() {
            chosen = Some(Choice::Close);
        }
    });
    chosen
}

impl Cad {
    /// Every plane the build on screen carries, on screen with its name, the names clear of what covers the view and of each other: none while hidden.
    pub(super) fn drawn_planes(&self, painter: &egui::Painter, v: &View) -> Vec<(Drawn, String)> {
        let Some(build) = v.build.filter(|_| !self.planes.hidden) else { return Vec::new() };
        let proj = v.camera.projector(v.rect);
        let shapes = touch::planes::shapes(v.design, &build.0);
        let sized: Vec<_> = shapes.iter().map(|s| (s, painter.layout_no_wrap(s.name.clone(), egui::FontId::proportional(NAME_PT), crate::theme::INK).size())).collect();
        let drawn = touch::planes::lay_out(&sized, |w| proj.at(w.map(|x| x as f32)), v.rect, v.covered);
        drawn.into_iter().zip(shapes).map(|(d, s)| (d, s.name)).collect()
    }

    /// The plane a finger at `p` takes: by its name, or by its outline unless `on_part` says a part lies under the finger.
    pub(super) fn plane_at(&self, painter: &egui::Painter, v: &View, p: Pos2, on_part: bool) -> Option<Id> {
        let drawn: Vec<Drawn> = self.drawn_planes(painter, v).into_iter().map(|(d, _)| d).collect();
        if on_part { touch::planes::name_at(&drawn, p) } else { touch::planes::at(&drawn, p, touch::planes::REACH_PT) }
    }

    /// Chooses work plane `id`, the parts chosen left as they are, and says what it offers.
    pub(super) fn choose_plane(&mut self, v: &View, id: Id) {
        self.planes.chosen = Some(id);
        self.walk.forget();
        let name = name_of(v.design, id);
        self.status(format!("Work plane {name}: hold it to sketch on it or mirror the chosen part across it"));
    }

    /// Opens plane `id`'s menu under the finger at `p`.
    pub(super) fn open_plane_menu(&mut self, v: &View, id: Id, p: Pos2) {
        self.menu = None;
        let heading = format!("Work plane: {}", name_of(v.design, id));
        self.planes.menu = Some(PlaneMenu { plane: id, at: p, heading, rows: rows(v.design, &self.selection, id), rect: Rect::NOTHING, below: None });
    }

    /// Every plane as a translucent rectangle with its name, the chosen one and the one whose menu is open lit.
    pub(super) fn draw_planes(&self, painter: &egui::Painter, v: &View) {
        let open = self.planes.menu.as_ref().map(|m| m.plane);
        for (d, name) in self.drawn_planes(painter, v) {
            let chosen = self.planes.chosen == Some(d.id);
            let lit = open == Some(d.id);
            let color = if chosen { CHOSEN } else { crate::theme::AQUA };
            let (fill, width, line) = if lit || chosen { (0.14, 2.5, 1.0) } else { (0.06, 1.5, 0.6) };
            painter.add(egui::Shape::convex_polygon(d.corners.to_vec(), color.gamma_multiply(fill), egui::Stroke::new(width, color.gamma_multiply(line))));
            let galley = painter.layout_no_wrap(name, egui::FontId::proportional(NAME_PT), color);
            painter.rect_filled(d.name.expand2(egui::vec2(4.0, 2.0)), 3.0, egui::Color32::from_black_alpha(160));
            painter.galley(d.name.min, galley, color);
        }
    }

    /// Serves a plane menu's row: a mirror leaves as a request for the funnel, hiding asks for the setting to be kept.
    pub(super) fn plane_act(&mut self, v: &View, plane: Id, act: Act) {
        match act {
            Act::Sketch => self.start_sketch(v, touch::sketch::Place::Plane(plane)),
            Act::Hide => {
                self.planes.hidden = true;
                self.planes.chosen = None;
                self.requests.push(Request::Prefs);
                self.status("Work planes hidden: the View menu shows them again");
            }
            Act::Mirror => match mirrorable(v.design, &self.selection, plane).and_then(|part| self.mirror_across(v, part, plane)) {
                Ok(r) => self.requests.push(r),
                Err(why) => self.status(why),
            },
        }
    }

    /// Part `part` reflected across work plane `plane`, meeting the band as the part does.
    fn mirror_across(&self, v: &View, part: Id, plane: Id) -> Result<Request, String> {
        let f = v.design.cad.as_ref().and_then(|d| d.feature(part)).cloned().ok_or_else(|| format!("Part #{part} is not in the document"))?;
        let attach = v.build.and_then(|b| b.evaluated()).and_then(|e| e.components.iter().find(|c| c.id == part)).map_or(f.component.attach, |c| c.attach);
        let edit = touch::parts::mirror(v.design, &f, attach, MirrorPlane::Plane { feature: plane })?;
        Ok(Request::Edit { edits: vec![edit], then: Then::LastAdded })
    }
}

/// A feature's name as the document holds it.
fn name_of(design: &RingDesign, id: Id) -> String {
    design.cad.as_ref().and_then(|d| d.feature(id)).map_or_else(|| format!("#{id}"), |f| f.name.clone())
}
