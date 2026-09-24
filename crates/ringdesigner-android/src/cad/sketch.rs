//! Sketching on the phone's ring: one finger draws on a part's face, a work plane or the band with the ring as the underlay, two fingers pan and zoom, and Finish makes an extrusion or a revolution as one undo step.
use egui::{Color32, Pos2, Rect, Stroke};
use egui_mobile::egui;
use ringdesign_core::{
    cad::{Attach, Operation},
    interaction::pick::Ray,
    sketch::{Id, Region},
};
use ringdesign_workbench::{
    command::{DimEvent, DimensionBar},
    focus::Pose,
    icons::Icon,
    sketch_tools::{self, Escaped, Outcome, Tool},
    touch::{
        self, Signal,
        sketch::{Make, Pad, Place, Stage},
    },
};

use super::{Built, Cad, Request, Then, Took, View, bar};

/// The drawing and editing tools on the sketch's bar, in order; Erase follows them.
pub const TOOLS: [Tool; 11] = Tool::ALL;
/// A view shorter than this, as with the keyboard up, keeps the tools to one row that scrolls, and only while drawing, points.
pub const ROW_VIEW_PT: f32 = 420.0;
/// A view shorter than this leaves no room for the tools beside the stage's bar and fields, points.
pub const TOOLS_VIEW_PT: f32 = 240.0;
/// How much the metal under a sketch is dimmed.
const DIM_ALPHA: u8 = 110;
/// A drawn curve's chord on screen, points.
const CHORD_PT: f64 = 0.6;
/// The solid's arrow past its top on screen, points.
const ARROW_PT: f32 = 64.0;
/// The side of a tool button, points.
const TOOL_PT: f32 = 44.0;
/// The dimension bar's id salt.
const BAR_ID: &str = "phone-sketch-dimensions";
/// The dimension bar's height until it has been drawn once, points.
const FIELDS_PT: f32 = 44.0;

/// The area the sketch's tools are drawn in.
pub fn tools_area() -> egui::Id {
    egui::Id::new("phone-sketch-tools")
}

/// The area the sketch's dimension fields are drawn in.
pub fn fields_area() -> egui::Id {
    egui::Id::new(BAR_ID).with("area")
}

/// What the one finger on the ring is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Finger {
    #[default]
    Up,
    /// Down on the ring, not yet a drag.
    Down,
    /// Drawing, or moving a point.
    Draw,
    /// Dragging the solid's arrow, or turning a revolution.
    Pull,
    /// Panning the view, last seen here.
    Pan(Pos2),
}

/// Which view the stage wants, so a stage change turns the camera once.
fn view_key(pad: &Pad) -> &'static str {
    match pad.stage {
        Stage::Draw | Stage::Offer | Stage::Axis => "square",
        Stage::Extrude if pad.cutting() => "cut",
        Stage::Extrude => "extrude",
        Stage::Revolve { .. } => "revolve",
    }
}

/// A sketch drawn on the phone.
pub struct Live {
    pub pad: Pad,
    bar: DimensionBar,
    /// The build the plane was last read from.
    build: usize,
    /// Leaving with strokes unfinished waits on this question.
    pub asking: bool,
    /// A commit has left for the funnel; the sketch closes when it lands.
    closing: bool,
    /// The view the camera last turned to.
    looked: Option<&'static str>,
    /// The camera's pose when the sketch opened.
    came_from: Pose,
    finger: Finger,
    /// A field that takes the keyboard once the bar is drawn.
    focus: Option<&'static str>,
}

impl Live {
    fn new(pad: Pad, build: usize, came_from: Pose) -> Self {
        Self { pad, bar: DimensionBar::new(BAR_ID), build, asking: false, closing: false, looked: None, came_from, finger: Finger::Up, focus: None }
    }

    /// Whether a finger on the ring is taken by the sketch this frame.
    pub fn holding(&self) -> bool {
        !matches!(self.finger, Finger::Up)
    }
}

/// The world point `p` turned by `deg` about the axis through `p0` along unit `a`.
fn turn(p: [f64; 3], p0: [f64; 3], a: [f64; 3], deg: f64) -> [f64; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    let v: [f64; 3] = std::array::from_fn(|k| p[k] - p0[k]);
    let along = a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
    let x = [a[1] * v[2] - a[2] * v[1], a[2] * v[0] - a[0] * v[2], a[0] * v[1] - a[1] * v[0]];
    std::array::from_fn(|k| p0[k] + v[k] * c + x[k] * s + a[k] * along * (1.0 - c))
}

/// Points per millimetre at the middle of the view.
pub(super) fn px_per_mm(v: &View) -> f64 {
    let (scale, _) = ringdesign_workbench::hover::view_scale(v.rect.center(), &|p| v.ray(p));
    scale.px_per_mm.max(1e-6)
}

/// How far `p` lies from the segment `a`–`b`, points.
fn segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// What a sketch's step said, for the status line: its words, else the prompt.
fn words(out: Outcome, pad: &mut Pad) -> String {
    match out {
        Outcome::Edited(w) | Outcome::Refused(w) => w,
        Outcome::Continue => pad.prompt(),
    }
}

impl Cad {
    /// Whether a sketch is being drawn.
    pub fn sketching(&self) -> bool {
        self.sketch.is_some()
    }

    /// The sketch being drawn.
    pub fn sketch(&self) -> Option<&Live> {
        self.sketch.as_deref()
    }

    pub fn sketch_mut(&mut self) -> Option<&mut Live> {
        self.sketch.as_deref_mut()
    }

    /// Starts a new sketch lying on `place` in the ring on screen.
    pub(super) fn start_sketch(&mut self, v: &View, place: Place) {
        let Some(build) = v.build else { return self.status("The ring has not built yet; sketch once it has") };
        match touch::sketch::start(&build.0, place) {
            Ok((sketch, centre)) => self.enter_sketch(v, build, Pad::new(None, sketch, centre, Some(place))),
            Err(why) => self.status(why),
        }
    }

    /// Opens Sketch feature `id` to draw on again, as the next frame over the ring.
    pub fn open_sketch_later(&mut self, id: Id) {
        self.open_pending = Some(id);
    }

    /// Opens Sketch feature `id` to draw on again.
    pub(super) fn open_sketch(&mut self, v: &View, id: Id) {
        let Some(f) = v.design.cad.as_ref().and_then(|d| d.feature(id)) else { return self.status(format!("Sketch #{id} is not in the document")) };
        let Operation::Sketch { sketch } = &f.operation else { return self.status(format!("{} is not a sketch", f.name)) };
        let Some(build) = v.build else { return self.status("The ring has not built yet; sketch once it has") };
        let pad = Pad::new(Some(id), sketch.clone(), [0.0; 2], None);
        self.enter_sketch(v, build, pad);
    }

    fn enter_sketch(&mut self, v: &View, build: &Built, mut pad: Pad) {
        self.live.cancel(&mut self.requests);
        self.menu = None;
        self.planes.menu = None;
        if self.boxing.on {
            self.boxing.toggle();
        }
        if v.measuring {
            self.requests.push(Request::EndMeasure);
        }
        pad.read(&build.0);
        let said = match pad.error() {
            Some(why) => why.to_string(),
            None => format!("Sketching · {}", pad.prompt()),
        };
        self.sketch = Some(Box::new(Live::new(pad, build.key(), v.camera.pose())));
        self.sketch_look(v);
        self.status(said);
    }

    /// Turns the camera to the view the sketch's stage wants.
    pub fn sketch_look(&mut self, v: &View) {
        let Some(live) = self.sketch.as_mut() else { return };
        let Some(look) = live.pad.look() else { return };
        let cam = v.camera;
        let pose = touch::sketch::look_along(cam.pose(), cam.target, cam.half_extent() * cam.zoom, look.eye, look.up, look.centre, look.reach_mm);
        live.looked = Some(view_key(&live.pad));
        self.requests.push(Request::Look(pose));
    }

    /// A funnel commit the sketch sent has landed or been refused: landed, the sketch closes; refused, drawing goes on.
    pub fn edit_landed(&mut self, ok: bool) {
        let Some(live) = self.sketch.as_mut().filter(|l| l.closing) else { return };
        if !ok {
            live.closing = false;
            return;
        }
        self.close_sketch();
    }

    /// Ends the sketch without keeping what it drew.
    pub fn drop_sketch(&mut self) {
        self.sketch = None;
    }

    /// Ends the sketch and turns the camera back to where it was when the sketch opened.
    fn close_sketch(&mut self) {
        if let Some(live) = self.sketch.take() {
            self.requests.push(Request::Look(live.came_from));
        }
    }

    /// The back key while sketching: one level back, the sketch itself last.
    pub fn sketch_back(&mut self) {
        let Some(live) = self.sketch.as_mut() else { return };
        if live.asking {
            live.asking = false;
            return self.status("Draw on");
        }
        let out = live.pad.escape();
        live.bar.reset();
        match out {
            Escaped::Out => self.sketch_leave(),
            _ => {
                let said = live.pad.prompt();
                self.status(said);
            }
        }
    }

    /// Takes back the sketch's own last edit; false when nothing is left to take back in it.
    pub fn sketch_undo(&mut self) -> bool {
        self.sketch.as_mut().is_some_and(|l| l.pad.undo())
    }

    /// Puts back the sketch's last edit taken back.
    pub fn sketch_redo(&mut self) -> bool {
        self.sketch.as_mut().is_some_and(|l| l.pad.redo())
    }

    /// Where the solid's arrow runs on screen while an extrusion is set: from its top out along the normal.
    pub(super) fn arrow(live: &mut Live, v: &View, px: f64) -> Option<(Pos2, Pos2)> {
        if live.pad.stage != Stage::Extrude {
            return None;
        }
        let n = live.pad.rise()?;
        let base = live.pad.anchor()?;
        let h = live.pad.height_mm();
        let proj = v.camera.projector(v.rect);
        let at = |t: f64| proj.at(std::array::from_fn(|k| (base[k] + n[k] * t) as f32));
        Some((at(h), at(h + f64::from(ARROW_PT) / px)))
    }

    /// A frame's gestures while a sketch is live: one finger draws, drags the solid's arrow or pans; two fingers are the view's.
    pub(super) fn sketch_gestures(&mut self, ui: &egui::Ui, v: &View, signals: Vec<Signal>) -> Took {
        let mut took = Took { hold: false, tap: true, long_press: true };
        let mut said: Vec<String> = Vec::new();
        let Some(live) = self.sketch.as_mut() else { return took };
        let ray = |p: Pos2| {
            let (o, d) = v.ray(p);
            Ray { origin: o.map(f64::from), direction: d.map(f64::from) }
        };
        let px = px_per_mm(v);
        let frame = live.pad.frame().copied();
        let on_plane = |p: Pos2| frame.and_then(|f| f.hit(ray(p)));
        let arrow = Self::arrow(live, v, px);
        for signal in signals {
            match signal {
                Signal::Press(p) => {
                    let over = ui.ctx().layer_id_at(p).is_some_and(|layer| layer != ui.layer_id());
                    let mine = v.active && v.rect.contains(p) && !over && !v.covered.iter().any(|r| r.contains(p));
                    let on_arrow = arrow.is_some_and(|(a, b)| segment(p, a, b) <= touch::FINGER_PT);
                    let turning = matches!(live.pad.stage, Stage::Revolve { .. });
                    live.finger = match (mine, on_arrow || turning) {
                        (false, _) => Finger::Up,
                        (true, true) => Finger::Pull,
                        (true, false) => Finger::Down,
                    };
                }
                Signal::DragStart { from, at } => match live.finger {
                    Finger::Pull => {
                        // Grips where the finger landed, then follows it.
                        live.pad.pull(ray(from));
                        live.pad.pull(ray(at));
                    }
                    Finger::Down => {
                        let taken = match (on_plane(from), on_plane(at)) {
                            (Some(a), Some(b)) => live.pad.drag_start(a, b, px),
                            _ => false,
                        };
                        live.finger = if taken { Finger::Draw } else { Finger::Pan(at) };
                        if !taken {
                            self.requests.push(Request::Pan { by: at - from, rect: v.rect });
                        }
                    }
                    _ => {}
                },
                Signal::DragMove { at } => match live.finger {
                    Finger::Pull => {
                        live.pad.pull(ray(at));
                    }
                    Finger::Draw => {
                        if let Some(xy) = on_plane(at) {
                            live.pad.drag_move(xy, px);
                        }
                    }
                    Finger::Pan(last) => {
                        self.requests.push(Request::Pan { by: at - last, rect: v.rect });
                        live.finger = Finger::Pan(at);
                    }
                    _ => {}
                },
                Signal::DragEnd { at } => {
                    match live.finger {
                        Finger::Pull => {
                            live.pad.pull(ray(at));
                            live.pad.pull_end();
                            said.push(live.pad.prompt());
                        }
                        Finger::Draw => match on_plane(at) {
                            Some(xy) => {
                                let out = live.pad.drag_end(xy, px);
                                said.push(words(out, &mut live.pad));
                            }
                            None => live.pad.let_go(),
                        },
                        Finger::Pan(last) => self.requests.push(Request::Pan { by: at - last, rect: v.rect }),
                        _ => {}
                    }
                    live.finger = Finger::Up;
                }
                Signal::Tap(p) => {
                    live.pad.pull_end();
                    match live.finger {
                        // A tap on the arrow waits for a typed height.
                        Finger::Pull if live.pad.stage == Stage::Extrude => live.focus = Some("height"),
                        Finger::Down => {
                            if let Some(xy) = on_plane(p) {
                                let out = live.pad.tap(xy, px);
                                // A corner chosen for a round or a bevel waits for its size on the keyboard.
                                if out == Outcome::Continue && live.pad.tools.busy() {
                                    live.focus = match live.pad.tools.tool {
                                        Tool::Fillet => Some("radius"),
                                        Tool::Chamfer => Some("distance"),
                                        _ => live.focus,
                                    };
                                }
                                said.push(words(out, &mut live.pad));
                            }
                        }
                        _ => {}
                    }
                    live.finger = Finger::Up;
                }
                Signal::Pinch | Signal::Cancel => {
                    live.pad.let_go();
                    live.pad.pull_end();
                    live.finger = Finger::Up;
                }
                Signal::LongPress(_) => {}
                Signal::Released => live.finger = Finger::Up,
            }
        }
        took.hold = live.holding();
        for s in said {
            self.status(s);
        }
        took
    }

    /// Draws the live sketch over the ring, its tools, its bar and its fields, and serves their buttons.
    pub(super) fn draw_sketch(&mut self, ui: &mut egui::Ui, v: &View) {
        let Some(live) = self.sketch.as_mut() else { return };
        if let Some(build) = v.build
            && build.key() != live.build
        {
            live.pad.read(&build.0);
            live.build = build.key();
        }
        let painter = ui.painter_at(v.rect);
        painter.rect_filled(v.rect, 0.0, Color32::from_black_alpha(DIM_ALPHA));
        let px = px_per_mm(v);
        if let Some(frame) = live.pad.frame().copied() {
            let proj = v.camera.projector(v.rect);
            let to = |uv: [f64; 2]| proj.at(frame.point(uv).map(|x| x as f32));
            let world = |w: [f64; 3]| proj.at(w.map(|x| x as f32));
            let chord = (CHORD_PT / px).clamp(1e-3, 0.2);
            grid(&painter, v, &live.pad, &frame, px, &to);
            for seg in &live.pad.underlay().cut {
                painter.line_segment([to(seg[0]), to(seg[1])], Stroke::new(1.0, crate::theme::AQUA.gamma_multiply(0.45)));
            }
            for l in &live.pad.underlay().face {
                let mut points: Vec<Pos2> = l.iter().map(|p| to(*p)).collect();
                points.extend(points.first().copied());
                painter.add(egui::Shape::line(points, Stroke::new(2.0, crate::theme::AQUA)));
            }
            let solid = live.pad.stage != Stage::Draw;
            let swept = if solid { live.pad.swept() } else { Vec::new() };
            let all: Vec<Region> = live.pad.regions().map(<[Region]>::to_vec).unwrap_or_default();
            for r in &all {
                let lit = swept.iter().any(|s| s == r);
                fill(&painter, r, chord, if lit { crate::theme::VIOLET.gamma_multiply(0.40) } else { crate::theme::VIOLET.gamma_multiply(0.12) }, &to);
            }
            let s = &live.pad.working;
            for e in &s.entities {
                let chosen = live.pad.tools.is_chosen(e.id);
                let stroke = if chosen {
                    Stroke::new(3.0, crate::theme::PINK_BRIGHT)
                } else if e.construction {
                    Stroke::new(1.2, crate::theme::INK_DIM)
                } else {
                    Stroke::new(2.2, crate::theme::INK)
                };
                for l in s.polylines(e.id, chord) {
                    let points: Vec<Pos2> = l.iter().map(|p| to(*p)).collect();
                    if e.construction && !chosen {
                        painter.extend(egui::Shape::dashed_line(&points, stroke, 5.0, 4.0));
                    } else {
                        painter.add(egui::Shape::line(points, stroke));
                    }
                }
            }
            for p in &s.points {
                let chosen = live.pad.tools.chosen_points.contains(&p.id);
                painter.circle_filled(to(p.xy), if chosen { 5.0 } else { 3.0 }, if chosen { crate::theme::PINK_BRIGHT } else { crate::theme::INK });
            }
            for a in sketch_tools::annotations(s) {
                let (p, q) = (to(a.from), to(a.to));
                let along = (q - p).normalized();
                painter.text(p + (q - p) * 0.5 + egui::vec2(-along.y, along.x) * 14.0, egui::Align2::CENTER_CENTER, &a.text, egui::FontId::proportional(12.0), crate::theme::AQUA_BRIGHT);
            }
            let preview = live.pad.tools.preview(s);
            if live.pad.stage == Stage::Draw {
                for stroke in &preview.strokes {
                    painter.add(egui::Shape::line(stroke.iter().map(|p| to(*p)).collect(), Stroke::new(1.5, crate::theme::AQUA)));
                }
                for g in &preview.ghost {
                    painter.add(egui::Shape::line(g.iter().map(|p| to(*p)).collect(), Stroke::new(2.2, crate::theme::AQUA)));
                }
                for m in &preview.marks {
                    painter.circle_stroke(to(*m), 5.0, Stroke::new(1.5, crate::theme::AQUA));
                }
                if let (Finger::Draw, Some((_, Some(snap)))) = (live.finger, live.pad.pointer) {
                    let p = to(snap.xy);
                    let r = 7.0;
                    painter.add(egui::Shape::closed_line(vec![p + egui::vec2(0.0, -r), p + egui::vec2(r, 0.0), p + egui::vec2(0.0, r), p + egui::vec2(-r, 0.0)], Stroke::new(2.0, crate::theme::AQUA_BRIGHT)));
                    painter.text(p + egui::vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, snap.kind.label(), egui::FontId::proportional(12.0), crate::theme::AQUA_BRIGHT);
                }
            }
            match live.pad.stage.clone() {
                Stage::Extrude => {
                    // A cut's far end is its floor, under the plane.
                    let (h, rise) = (live.pad.height_mm(), live.pad.rise().unwrap_or(frame.n));
                    let up = |uv: [f64; 2]| world(std::array::from_fn(|k| frame.point(uv)[k] + rise[k] * h));
                    for r in &swept {
                        for poly in r.polygons(chord) {
                            let mut top: Vec<Pos2> = poly.iter().map(|p| up(*p)).collect();
                            top.extend(top.first().copied());
                            painter.add(egui::Shape::line(top, Stroke::new(2.0, crate::theme::AQUA_BRIGHT)));
                            for p in poly.iter().step_by((poly.len() / 8).max(1)) {
                                painter.line_segment([to(*p), up(*p)], Stroke::new(1.2, crate::theme::AQUA.gamma_multiply(0.8)));
                            }
                        }
                    }
                    if let Some((a, b)) = Self::arrow(live, v, px) {
                        let lit = live.finger == Finger::Pull;
                        let color = if lit { Color32::from_rgb(255, 226, 110) } else { crate::theme::AQUA };
                        painter.line_segment([a, b], Stroke::new(if lit { 4.0 } else { 3.0 }, color));
                        painter.circle(b, 10.0, color, Stroke::new(1.5, Color32::BLACK));
                    }
                }
                Stage::Revolve { pivot, dir, .. } => {
                    // A cut turns the other way round the same line, into the metal.
                    let (p0, a) = live.pad.revolution().unwrap_or((frame.point(pivot), frame.vector(dir)));
                    let far = 40.0;
                    let ends = [frame.point([pivot[0] - dir[0] * far, pivot[1] - dir[1] * far]), frame.point([pivot[0] + dir[0] * far, pivot[1] + dir[1] * far])];
                    painter.extend(egui::Shape::dashed_line(&[world(ends[0]), world(ends[1])], Stroke::new(1.8, crate::theme::AQUA), 7.0, 5.0));
                    let deg = live.pad.degrees();
                    for r in &swept {
                        for poly in r.polygons(chord) {
                            let at_end: Vec<Pos2> = poly.iter().chain(poly.first()).map(|p| world(turn(frame.point(*p), p0, a, deg))).collect();
                            painter.add(egui::Shape::line(at_end, Stroke::new(2.0, crate::theme::AQUA_BRIGHT)));
                            for p in poly.iter().step_by((poly.len() / 6).max(1)) {
                                let arc: Vec<Pos2> = (0..=24).map(|k| world(turn(frame.point(*p), p0, a, deg * f64::from(k) / 24.0))).collect();
                                painter.add(egui::Shape::line(arc, Stroke::new(1.0, crate::theme::AQUA.gamma_multiply(0.7))));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let services = self.sketch_bars(ui, v);
        self.serve_sketch(v, services);
        // A stage that wants another view turns the camera to it once.
        if let Some(live) = self.sketch.as_mut() {
            let want = view_key(&live.pad);
            if live.pad.frame().is_some() && live.looked != Some(want) {
                self.sketch_look(v);
            }
        }
    }

    /// The tools at the view's top, the stage's bar at its foot and the dimension fields between; what their buttons asked for.
    /// As the keyboard shortens the view the tools keep to one scrolling row, and only while drawing, and give up their room before the bar and fields do.
    fn sketch_bars(&mut self, ui: &mut egui::Ui, v: &View) -> Vec<Ask> {
        let mut asked = Vec::new();
        let Some(live) = self.sketch.as_mut() else { return asked };
        let ctx = ui.ctx().clone();
        let drawing = live.pad.stage == Stage::Draw && !live.asking;
        // The tools, a thumb high, clear of the navigator.
        let right = v.covered.iter().filter(|r| r.top() < v.rect.top() + 120.0 && r.left() > v.rect.center().x).map(|r| r.left()).fold(v.rect.right(), f32::min);
        let width = (right - v.rect.left() - 16.0).max(TOOL_PT * 4.0);
        let row = v.rect.height() < ROW_VIEW_PT;
        let mut tools_bottom = v.rect.top();
        if !row || drawing && v.rect.height() >= TOOLS_VIEW_PT {
            let specs = tool_buttons(&live.pad, drawing);
            let buttons = |ui: &mut egui::Ui, asked: &mut Vec<Ask>| {
                for t in &specs {
                    if t.gap {
                        ui.separator();
                    }
                    let b = egui::Button::image(t.icon.image(ui, 22.0)).selected(t.on).min_size(egui::vec2(TOOL_PT, TOOL_PT)).frame_when_inactive(true);
                    let r = ui.add_enabled(t.enabled, b);
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, t.enabled, &t.name));
                    if r.clicked() {
                        asked.push(t.ask.clone());
                    }
                }
            };
            let shown = egui::Area::new(tools_area()).order(egui::Order::Foreground).fixed_pos(v.rect.left_top() + egui::vec2(8.0, 8.0)).constrain_to(v.rect).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(Color32::from_rgba_unmultiplied(12, 12, 18, 232)).inner_margin(4).show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                    if row {
                        egui::ScrollArea::horizontal().id_salt("phone-sketch-tool-row").show(ui, |ui| ui.horizontal(|ui| buttons(ui, &mut asked)));
                    } else {
                        ui.horizontal_wrapped(|ui| buttons(ui, &mut asked));
                    }
                });
            });
            tools_bottom = shown.response.rect.bottom();
        }
        // The stage's words and buttons at the view's foot.
        let mut lines = vec![(live.pad.prompt(), crate::theme::AQUA)];
        let caption = match live.pad.stage {
            Stage::Draw => live.pad.tools.preview(&live.pad.working).caption,
            Stage::Extrude if live.pad.cutting() => format!("Depth {:.2} mm", live.pad.height_mm()),
            Stage::Extrude => format!("Height {:.2} mm", live.pad.height_mm()),
            Stage::Revolve { .. } => format!("Angle {:.0}°", live.pad.degrees()),
            _ => String::new(),
        };
        lines.push((caption, crate::theme::INK));
        if let Some(why) = live.pad.error() {
            lines.push((why.to_string(), crate::theme::PINK_BRIGHT));
        } else if live.pad.stage == Stage::Offer
            && let Err(why) = live.pad.regions()
        {
            lines.push((why.to_string(), crate::theme::PINK_BRIGHT));
        }
        if live.asking {
            lines = vec![("This sketch has strokes not yet kept: keep them, drop them, or go on drawing".into(), crate::theme::PINK_BRIGHT)];
        }
        let (buttons, asks): (Vec<bar::Button>, Vec<Ask>) = stage_buttons(live, v.design).into_iter().unzip();
        if let Some(i) = bar::show(&ctx, v.rect, v.covered, &lines, &buttons) {
            asked.push(asks[i].clone());
        }
        // The fields stand over the bar and under the tools, and never above the view.
        let mut dims = live.pad.dimensions();
        if !dims.is_empty() {
            let shown = ctx.memory(|m| m.area_rect(bar::area())).unwrap_or(v.rect);
            let tall = ctx.memory(|m| m.area_rect(fields_area())).map_or(FIELDS_PT, |r| r.height());
            let y = (shown.top() - 6.0 - tall).max(tools_bottom + 4.0).max(v.rect.top() + 4.0);
            let anchor = egui::pos2(v.rect.left() + 8.0, y) - egui::vec2(18.0, 18.0);
            for e in live.bar.show(&ctx, anchor, v.rect, &mut dims) {
                asked.push(match e {
                    DimEvent::Typed { key, value } => Ask::Typed(key, value),
                    DimEvent::Cleared { key } => Ask::Cleared(key),
                    DimEvent::Confirm => Ask::FieldDone,
                    DimEvent::Escape => Ask::Back,
                    DimEvent::Focused { .. } => continue,
                });
            }
            if let Some(key) = live.focus.take() {
                live.bar.focus_field(&ctx, key);
            }
        } else {
            live.bar.reset();
            live.focus = None;
        }
        asked
    }

    /// Serves what the sketch's buttons and fields asked for.
    pub(super) fn serve_sketch(&mut self, v: &View, asked: Vec<Ask>) {
        for ask in asked {
            let Some(live) = self.sketch.as_mut() else { return };
            let said = match ask {
                Ask::Tool(t) => {
                    live.pad.set_tool(t);
                    live.bar.reset();
                    Some(live.pad.prompt())
                }
                Ask::Erase => {
                    live.pad.set_erase();
                    Some(live.pad.prompt())
                }
                Ask::Undo => Some(if live.pad.undo() { "Took back the sketch's last edit".into() } else { "Nothing to take back in this sketch".into() }),
                Ask::Redo => Some(if live.pad.redo() { "Put the sketch's edit back".into() } else { "Nothing to put back".into() }),
                Ask::Construction => {
                    let out = live.pad.tools.toggle_construction(&mut live.pad.working);
                    Some(match out {
                        Outcome::Continue if live.pad.tools.construction => "New curves are drawn as construction: guides, never profile".into(),
                        Outcome::Continue => "New curves are drawn as profile".into(),
                        other => words(other, &mut live.pad),
                    })
                }
                Ask::Look => {
                    live.looked = None;
                    None
                }
                Ask::Switch => match v.build.map(|b| live.pad.switch_plane(&b.0)) {
                    Some(Ok(w)) => {
                        live.looked = None;
                        Some(w.to_string())
                    }
                    Some(Err(why)) => Some(why),
                    None => None,
                },
                Ask::Close => {
                    let out = live.pad.close();
                    Some(words(out, &mut live.pad))
                }
                Ask::Confirm | Ask::FieldDone if live.pad.stage == Stage::Draw => {
                    let out = live.pad.confirm();
                    Some(words(out, &mut live.pad))
                }
                Ask::FieldDone => {
                    self.sketch_commit(v);
                    None
                }
                Ask::Confirm => None,
                Ask::Back => {
                    let out = live.pad.escape();
                    live.bar.reset();
                    match out {
                        Escaped::Out => {
                            self.sketch_leave();
                            None
                        }
                        _ => Some(live.pad.prompt()),
                    }
                }
                Ask::Finish => {
                    let out = live.pad.finish();
                    Some(words(out, &mut live.pad))
                }
                Ask::Make(m) => {
                    let out = live.pad.make(m);
                    Some(words(out, &mut live.pad))
                }
                Ask::Attach(a) => {
                    let out = live.pad.set_attach(v.design, a);
                    Some(words(out, &mut live.pad))
                }
                Ask::Keep => {
                    live.asking = false;
                    self.sketch_commit(v);
                    None
                }
                Ask::Drop => {
                    self.close_sketch();
                    Some("Left the sketch; its strokes were dropped".into())
                }
                Ask::DrawOn => {
                    live.asking = false;
                    Some(live.pad.prompt())
                }
                Ask::Leave => {
                    self.sketch_leave();
                    None
                }
                Ask::Typed(key, value) => {
                    let out = live.pad.typed(key, value);
                    match out {
                        Outcome::Continue => None,
                        other => Some(words(other, &mut live.pad)),
                    }
                }
                Ask::Cleared(key) => {
                    live.pad.cleared(key);
                    None
                }
            };
            if let Some(s) = said {
                self.status(s);
            }
        }
    }

    /// Leaves the sketch, asking first when strokes would be lost.
    fn sketch_leave(&mut self) {
        let Some(live) = self.sketch.as_mut() else { return };
        if live.pad.dirty() {
            live.asking = true;
            self.status("This sketch has strokes not yet kept: keep them, drop them, or go on drawing");
            return;
        }
        self.close_sketch();
        self.status("Left the sketch");
    }

    /// Sends what finishing leaves the document as to the funnel, one undo step; the sketch closes once it lands.
    pub(super) fn sketch_commit(&mut self, v: &View) {
        let Some(live) = self.sketch.as_mut() else { return };
        match live.pad.edits(v.design) {
            Ok(edits) if edits.is_empty() => {
                self.close_sketch();
                self.status("Left the sketch; nothing had changed");
            }
            Ok(edits) => {
                let then = if live.pad.solid(0).is_some() { Then::LastAdded } else { Then::Keep };
                live.closing = true;
                self.requests.push(Request::Edit { edits, then });
            }
            Err(why) => self.status(why),
        }
    }
}

/// What a sketch's button or field asked for.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Ask {
    Tool(Tool),
    Erase,
    Undo,
    Redo,
    Construction,
    Look,
    Switch,
    Close,
    Confirm,
    Back,
    Finish,
    Make(Make),
    /// How the solid being set meets the ring.
    Attach(Attach),
    /// Keep what is drawn: the sketch alone, or with the solid being set.
    Keep,
    Drop,
    DrawOn,
    Leave,
    Typed(&'static str, f64),
    Cleared(&'static str),
    /// Enter in a field: the step's numbers made while drawing, the solid made while one is set.
    FieldDone,
}

/// One of the sketch's tool buttons: its mark, its name, whether it is lit and offered, what it asks for, and whether a gap stands before it.
pub(super) struct ToolButton {
    pub icon: Icon,
    pub name: String,
    pub on: bool,
    pub enabled: bool,
    pub ask: Ask,
    pub gap: bool,
}

/// The sketch's tool buttons in order: every drawing and editing tool and Erase, then undo, redo, construction, the look and, on the band, the plane switch.
pub(super) fn tool_buttons(pad: &Pad, drawing: bool) -> Vec<ToolButton> {
    let button = |icon: Icon, name: String, on: bool, enabled: bool, ask: Ask| ToolButton { icon, name, on, enabled, ask, gap: false };
    let mut out: Vec<ToolButton> = TOOLS.iter().map(|t| button(t.icon(), format!("{} tool", t.label()), drawing && !pad.erase && pad.tools.tool == *t, drawing, Ask::Tool(*t))).collect();
    out.push(button(Icon::Delete, "Erase tool".into(), drawing && pad.erase, drawing, Ask::Erase));
    out.push(ToolButton { gap: true, ..button(Icon::Undo, "Undo sketch edit".into(), false, drawing && pad.tools.can_undo(), Ask::Undo) });
    out.push(button(Icon::Redo, "Redo sketch edit".into(), false, drawing && pad.tools.can_redo(), Ask::Redo));
    out.push(button(Icon::Guides, "Construction".into(), pad.tools.construction, drawing, Ask::Construction));
    out.push(button(Icon::View, "Look at the sketch".into(), false, true, Ask::Look));
    if matches!(pad.place, Some(Place::Tangent { .. } | Place::Section { .. })) {
        out.push(button(Icon::Section, "Switch plane".into(), false, drawing, Ask::Switch));
    }
    out
}

/// The buttons at the view's foot for the stage in hand, and what each asks for; a solid being set offers Join, Cut and Separate, the one chosen ticked.
pub(super) fn stage_buttons(live: &mut Live, design: &ringdesign_core::RingDesign) -> Vec<(bar::Button, Ask)> {
    let b = |icon: Icon, label: &'static str, enabled: bool| bar::Button { icon, label, checked: false, enabled };
    if live.asking {
        return vec![(b(Icon::Save, "Keep", true), Ask::Keep), (b(Icon::Delete, "Drop", true), Ask::Drop), (b(Icon::CadSketch, "Draw on", true), Ask::DrawOn)];
    }
    let regions = live.pad.regions().is_ok_and(|r| !r.is_empty());
    match live.pad.stage {
        Stage::Draw => {
            let busy = live.pad.tools.busy();
            let chain = busy && live.pad.tools.tool == Tool::Line;
            // Mirror's curves, once chosen, wait for Done before the line to mirror them in.
            let mirror = !busy && live.pad.tools.tool == Tool::Mirror && !live.pad.tools.chosen_entities.is_empty();
            let mut row = Vec::new();
            if chain {
                row.push((b(Icon::Path, "Close loop", true), Ask::Close));
            }
            if busy || mirror {
                row.push((b(Icon::Check, "Done", true), Ask::Confirm));
                row.push((b(Icon::Collapse, "Back", true), Ask::Back));
            }
            row.push((b(Icon::CadSketch, "Finish", true), Ask::Finish));
            row.push((b(Icon::Close, "Leave", true), Ask::Leave));
            row
        }
        Stage::Offer => vec![
            (b(Icon::CadExtrude, "Extrude", regions), Ask::Make(Make::Extrude)),
            (b(Icon::CadRevolve, "Revolve", regions), Ask::Make(Make::Revolve)),
            (b(Icon::Save, "Keep sketch", true), Ask::Keep),
            (b(Icon::Collapse, "Back", true), Ask::Back),
        ],
        Stage::Extrude | Stage::Revolve { .. } => {
            let attach = live.pad.attach;
            let cut = !design.cad.as_ref().is_some_and(|d| d.replaces_band());
            let way = |icon: Icon, label: &'static str, of: Attach, enabled: bool| (bar::Button { icon, label, checked: attach == of, enabled }, Ask::Attach(of));
            let make = if live.pad.stage == Stage::Extrude { "Extrude" } else { "Revolve" };
            vec![
                way(Icon::CadUnion, "Join", Attach::Join, true),
                way(Icon::CadSubtract, "Cut", Attach::Cut, cut),
                way(Icon::CadPlace, "Separate", Attach::Separate, true),
                (b(Icon::Check, make, true), Ask::Keep),
                (b(Icon::Collapse, "Back", true), Ask::Back),
            ]
        }
        Stage::Axis => vec![(b(Icon::Collapse, "Back", true), Ask::Back)],
    }
}

/// The plane's grid over the view, its axes lit.
fn grid(painter: &egui::Painter, v: &View, pad: &Pad, frame: &touch::sketch::Frame, px: f64, to: &impl Fn([f64; 2]) -> Pos2) {
    let corners: Vec<[f64; 2]> = [v.rect.left_top(), v.rect.right_top(), v.rect.right_bottom(), v.rect.left_bottom()]
        .into_iter()
        .filter_map(|p| {
            let (o, d) = v.ray(p);
            frame.hit(Ray { origin: o.map(f64::from), direction: d.map(f64::from) })
        })
        .collect();
    if corners.len() != 4 {
        return;
    }
    let g = touch::sketch::grid_mm(&pad.working, px);
    let lo = corners.iter().fold([f64::INFINITY; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
    let hi = corners.iter().fold([f64::NEG_INFINITY; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
    let (i0, i1) = ((lo[0] / g).floor() as i64, (hi[0] / g).ceil() as i64);
    let (j0, j1) = ((lo[1] / g).floor() as i64, (hi[1] / g).ceil() as i64);
    if i1 - i0 > 300 || j1 - j0 > 300 {
        return;
    }
    for i in i0..=i1 {
        let x = i as f64 * g;
        let stroke = if i == 0 { Stroke::new(1.3, crate::theme::AQUA.gamma_multiply(0.6)) } else { Stroke::new(0.6, Color32::from_white_alpha(20)) };
        painter.line_segment([to([x, lo[1]]), to([x, hi[1]])], stroke);
    }
    for j in j0..=j1 {
        let y = j as f64 * g;
        let stroke = if j == 0 { Stroke::new(1.3, crate::theme::PINK.gamma_multiply(0.6)) } else { Stroke::new(0.6, Color32::from_white_alpha(20)) };
        painter.line_segment([to([lo[0], y]), to([hi[0], y])], stroke);
    }
}

/// A region filled in `color`.
fn fill(painter: &egui::Painter, r: &Region, chord: f64, color: Color32, to: &impl Fn([f64; 2]) -> Pos2) {
    let mut mesh = egui::Mesh::default();
    for t in r.triangles(chord) {
        let base = mesh.vertices.len() as u32;
        for p in t {
            mesh.colored_vertex(to(p), color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// The area a view reserves for the sketch's own layers.
pub fn areas() -> [egui::Id; 2] {
    [tools_area(), fields_area()]
}

/// The screen rect area `id` covered in the last pass, when it was drawn there, for a test to read.
pub fn drawn_rect(ctx: &egui::Context, id: egui::Id) -> Option<Rect> {
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    ctx.memory(|m| m.areas().visible_last_frame(&layer).then(|| m.area_rect(id)).flatten())
}
