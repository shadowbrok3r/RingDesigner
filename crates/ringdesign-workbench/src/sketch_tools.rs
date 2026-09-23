//! Sketch tools shared by the Ring viewport's sketch mode and the CAD pane's canvas: drawing, editing
//! and dimensioning as one state machine over plane millimetres, and the snaps and picks it reads.
use crate::command::{Dimension, Unit};
use crate::icons::Icon;
use ringdesign_core::sketch::{Constraint, Geometry, Id, Measure, Region, Sketch, distance, draw::SAME_MM, query::nearest_on};
use std::collections::BTreeSet;

/// Undo steps a sketch session keeps.
const UNDO_DEPTH: usize = 100;

/// A tool on the sketch toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Line,
    Rectangle,
    Circle,
    Arc,
    Trim,
    Offset,
    Fillet,
    Chamfer,
    Mirror,
    Dimension,
}

impl Tool {
    pub const ALL: [Tool; 11] = [
        Tool::Select,
        Tool::Line,
        Tool::Rectangle,
        Tool::Circle,
        Tool::Arc,
        Tool::Trim,
        Tool::Offset,
        Tool::Fillet,
        Tool::Chamfer,
        Tool::Mirror,
        Tool::Dimension,
    ];
    /// The tools that change what is drawn rather than draw it.
    pub const EDITING: [Tool; 6] = [Tool::Trim, Tool::Offset, Tool::Fillet, Tool::Chamfer, Tool::Mirror, Tool::Dimension];

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Line => "Line",
            Self::Rectangle => "Rectangle",
            Self::Circle => "Circle",
            Self::Arc => "Arc",
            Self::Trim => "Trim",
            Self::Offset => "Offset",
            Self::Fillet => "Fillet corner",
            Self::Chamfer => "Chamfer corner",
            Self::Mirror => "Mirror",
            Self::Dimension => "Dimension",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Self::Select => Icon::Select,
            Self::Line => Icon::Path,
            Self::Rectangle => Icon::Single,
            Self::Circle => Icon::NodeSize,
            Self::Arc => Icon::NodeCurve,
            Self::Trim => Icon::NodeCull,
            Self::Offset => Icon::NodeBorder,
            Self::Fillet => Icon::CadFillet,
            Self::Chamfer => Icon::CadChamfer,
            Self::Mirror => Icon::Mirror,
            Self::Dimension => Icon::NodeLength,
        }
    }
    /// The key that picks the tool while a sketch holds the keys.
    pub fn key(self) -> Option<egui::Key> {
        use egui::Key;
        Some(match self {
            Self::Select => return None,
            Self::Line => Key::L,
            Self::Rectangle => Key::R,
            Self::Circle => Key::C,
            Self::Arc => Key::A,
            Self::Trim => Key::T,
            Self::Offset => Key::O,
            Self::Fillet => Key::F,
            Self::Chamfer => Key::K,
            Self::Mirror => Key::M,
            Self::Dimension => Key::D,
        })
    }
    /// What the tool does, for its tooltip.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Select => "Click a point or curve to choose it, Shift adds; Delete removes what is chosen",
            Self::Line => "Click point after point; a click on the first point or C closes the loop, Escape ends it open",
            Self::Rectangle => "Click two opposite corners, or type the width and height",
            Self::Circle => "Click the centre, then the rim or type the radius",
            Self::Arc => "Click the start, the end, then a point the arc passes through",
            Self::Trim => "Click the span of a curve to cut away between its crossings",
            Self::Offset => "Click a closed loop, then move out or in, or type the distance",
            Self::Fillet => "Click a corner where two lines meet and type the radius",
            Self::Chamfer => "Click a corner where two lines meet and type the distance",
            Self::Mirror => "Click the curves to mirror, press Enter, then click the line to mirror them in",
            Self::Dimension => "Click a line for its length, a circle or arc for its radius, or two points for their distance, then type the value",
        }
    }
}

/// What a snap landed on, highest priority first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,
    Centre,
    Intersection,
    Corner,
    Origin,
    Midpoint,
    Edge,
    Axis,
    Grid,
}

impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint",
            Self::Centre => "centre",
            Self::Intersection => "intersection",
            Self::Corner => "face corner",
            Self::Origin => "origin",
            Self::Midpoint => "midpoint",
            Self::Edge => "edge",
            Self::Axis => "axis",
            Self::Grid => "grid",
        }
    }
    fn tier(self) -> u8 {
        match self {
            Self::Endpoint | Self::Centre | Self::Intersection | Self::Corner | Self::Origin => 0,
            Self::Midpoint => 1,
            Self::Edge | Self::Axis => 2,
            Self::Grid => 3,
        }
    }
}

/// Where the pointer settled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    pub xy: [f64; 2],
    pub kind: SnapKind,
}

/// What a sketch is drawn over, in its own coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Underlay {
    /// The boundary loops of the face the sketch lies on, closed.
    pub face: Vec<Vec<[f64; 2]>>,
    /// Where the sketch's plane cuts the ring.
    pub cut: Vec<[[f64; 2]; 2]>,
}

impl Underlay {
    /// The face's corners: loop vertices where the boundary turns by more than 20 degrees.
    pub fn corners(&self) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for l in &self.face {
            let n = l.len();
            for i in 0..n {
                let (a, p, b) = (l[(i + n - 1) % n], l[i], l[(i + 1) % n]);
                let (u, v) = ([p[0] - a[0], p[1] - a[1]], [b[0] - p[0], b[1] - p[1]]);
                let (lu, lv) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
                if lu > 1e-9 && lv > 1e-9 && (u[0] * v[0] + u[1] * v[1]) / (lu * lv) < 20f64.to_radians().cos() {
                    out.push(p);
                }
            }
        }
        out
    }
}

/// Everything a snap is read against that changes only when the sketch or the underlay does.
#[derive(Clone, Debug, Default)]
pub struct SnapCache {
    pub crossings: Vec<[f64; 2]>,
    pub midpoints: Vec<[f64; 2]>,
    pub centres: Vec<[f64; 2]>,
    pub corners: Vec<[f64; 2]>,
}

impl SnapCache {
    pub fn of(s: &Sketch, under: &Underlay) -> Self {
        Self { crossings: s.crossings(), midpoints: s.midpoints(), centres: s.centres(), corners: under.corners() }
    }
}

/// Where the pointer settles: the nearest candidate of the highest tier within `reach`, the grid last.
pub fn snap(s: &Sketch, under: &Underlay, cache: &SnapCache, xy: [f64; 2], reach: f64, grid: Option<f64>) -> Option<Snap> {
    let mut best: Option<(u8, f64, Snap)> = None;
    let mut offer = |p: [f64; 2], kind: SnapKind| {
        let d = distance(p, xy);
        if !(d <= reach) {
            return;
        }
        let key = (kind.tier(), d);
        if best.is_none_or(|(t, b, _)| key.0 < t || (key.0 == t && key.1 < b)) {
            best = Some((key.0, key.1, Snap { xy: p, kind }));
        }
    };
    for c in &cache.centres {
        offer(*c, SnapKind::Centre);
    }
    for p in &s.points {
        if !cache.centres.iter().any(|c| distance(*c, p.xy) <= SAME_MM) {
            offer(p.xy, SnapKind::Endpoint);
        }
    }
    for x in &cache.crossings {
        offer(*x, SnapKind::Intersection);
    }
    for c in &cache.corners {
        offer(*c, SnapKind::Corner);
    }
    offer([0.0, 0.0], SnapKind::Origin);
    for m in &cache.midpoints {
        offer(*m, SnapKind::Midpoint);
    }
    for l in &under.face {
        let mut closed = l.clone();
        closed.extend(l.first().copied());
        if let Some((p, _)) = nearest_on(&closed, xy) {
            offer(p, SnapKind::Edge);
        }
    }
    for seg in &under.cut {
        if let Some((p, _)) = nearest_on(seg, xy) {
            offer(p, SnapKind::Edge);
        }
    }
    offer([xy[0], 0.0], SnapKind::Axis);
    offer([0.0, xy[1]], SnapKind::Axis);
    if let Some(g) = grid.filter(|g| g.is_finite() && *g > 0.0) {
        offer(xy.map(|v| (v / g).round() * g), SnapKind::Grid);
    }
    best.map(|(_, _, s)| s)
}

/// What the pointer is over in a sketch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Point(Id),
    Curve { entity: Id, at: [f64; 2] },
}

/// A point within `reach` of `xy`, else the nearest curve within it.
pub fn pick(s: &Sketch, xy: [f64; 2], reach: f64) -> Option<Hit> {
    if let Some(p) = s.nearest_point(xy, reach) {
        return Some(Hit::Point(p));
    }
    s.nearest_entity(xy, reach).map(|n| Hit::Curve { entity: n.entity, at: n.at })
}

/// Removes the entities, then the points with everything drawn through them; how many went.
pub fn delete(s: &mut Sketch, points: &BTreeSet<Id>, entities: &BTreeSet<Id>) -> usize {
    let before = s.entities.len() + s.points.len();
    for id in entities {
        s.remove_entity(*id);
    }
    for id in points {
        if s.points.iter().any(|p| p.id == *id) {
            s.remove_point(*id);
        }
    }
    before - (s.entities.len() + s.points.len())
}

/// A dimension drawn on the canvas: between two places, with its value.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub text: String,
}

/// Every distance constraint as the dimension it reads as: a radius where it runs from a centre to its rim.
pub fn annotations(s: &Sketch) -> Vec<Annotation> {
    let centre_of = |a: Id, b: Id| {
        s.entities.iter().any(|e| match &e.geometry {
            Geometry::Circle { center, rim } => (*center, *rim) == (a, b) || (*center, *rim) == (b, a),
            Geometry::Arc { center, start, end } => (a == *center && (b == *start || b == *end)) || (b == *center && (a == *start || a == *end)),
            _ => false,
        })
    };
    let mut out = Vec::new();
    for c in &s.constraints {
        let Constraint::Distance { a, b, mm } = c else { continue };
        let (Ok(p), Ok(q)) = (s.at(*a), s.at(*b)) else { continue };
        let text = if centre_of(*a, *b) { format!("R{mm:.2}") } else { format!("{mm:.2}") };
        // Skips an arc's second radius.
        if out.iter().any(|x: &Annotation| x.text == text && (x.from == p || x.to == p || x.from == q || x.to == q) && text.starts_with('R')) {
            continue;
        }
        out.push(Annotation { from: p, to: q, text });
    }
    out
}

/// One token a sketch tool reads.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// The pointer on the plane, where a snap settled it, and how far a pick reaches.
    Pointer { raw: [f64; 2], snapped: Option<Snap>, reach: f64 },
    /// A click where the pointer is; `add` keeps what was chosen.
    Click { add: bool },
    Typed { key: &'static str, value: f64 },
    Cleared { key: &'static str },
    Confirm,
    /// Closes the line being drawn back to its first point.
    Close,
}

/// What a token did.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Continue,
    /// The sketch changed, in these words.
    Edited(String),
    /// Nothing changed, and why.
    Refused(String),
}

/// How far the escape ladder went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escaped {
    /// A typed value was cleared.
    Field,
    /// The tool's step went back, or the choice was cleared.
    Step,
    /// The tool went back to Select.
    Tool,
    /// Nothing was left to back out of: the sketch itself is next.
    Out,
}

/// What a live tool draws: the rubber band, its anchors, the result a confirm gives, and its numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preview {
    pub strokes: Vec<Vec<[f64; 2]>>,
    pub marks: Vec<[f64; 2]>,
    pub ghost: Vec<Vec<[f64; 2]>>,
    pub caption: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Step {
    #[default]
    Idle,
    /// A line being drawn: its first point and the last one placed.
    Chain { first: [f64; 2], last: [f64; 2], lines: usize },
    /// A rectangle's first corner.
    Corner([f64; 2]),
    /// A circle's centre.
    Centre([f64; 2]),
    /// An arc's start, then its start and end.
    ArcStart([f64; 2]),
    ArcEnd([f64; 2], [f64; 2]),
    /// The loop an offset moves, and the region it bounds.
    Loop { ids: Vec<Id>, region: Box<Region> },
    /// The corner a fillet or chamfer cuts.
    CornerPoint(Id),
    /// The curves to mirror are chosen; the next click is the axis.
    MirrorAxis,
    /// A distance's first point.
    FirstPoint(Id),
    /// What a dimension holds.
    Measure(Measure),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pointer {
    raw: [f64; 2],
    at: [f64; 2],
    reach: f64,
}

/// The tool state one sketch session keeps between frames.
#[derive(Clone, Debug)]
pub struct Tools {
    pub tool: Tool,
    /// New geometry is drawn as construction.
    pub construction: bool,
    /// The points and curves chosen, for Delete and Mirror.
    pub chosen_points: BTreeSet<Id>,
    pub chosen_entities: BTreeSet<Id>,
    step: Step,
    pointer: Option<Pointer>,
    /// Typed values of the live step, latest last.
    typed: Vec<(&'static str, f64)>,
    undo: Vec<Sketch>,
    redo: Vec<Sketch>,
    /// The fillet radius, chamfer distance and offset last used.
    radius: f64,
    chamfer: f64,
    offset: f64,
    version: u64,
    ghost: Option<Sketch>,
    /// The sketch as the drag under way found it.
    dragged: Option<Sketch>,
    /// How long the last dimension's solve took, in milliseconds.
    pub last_solve_ms: Option<f64>,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            construction: false,
            chosen_points: BTreeSet::new(),
            chosen_entities: BTreeSet::new(),
            step: Step::Idle,
            pointer: None,
            typed: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            radius: 1.0,
            chamfer: 0.5,
            offset: 0.5,
            version: 0,
            ghost: None,
            dragged: None,
            last_solve_ms: None,
        }
    }
}

fn unit(v: [f64; 2]) -> Option<[f64; 2]> {
    let l = v[0].hypot(v[1]);
    (l > 1e-12 && l.is_finite()).then(|| [v[0] / l, v[1] / l])
}

impl Tools {
    /// Counts every change the tools made, undo and redo included.
    pub fn version(&self) -> u64 {
        self.version
    }
    /// Whether a tool is between its first click and its last.
    pub fn busy(&self) -> bool {
        self.step != Step::Idle
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Picks a tool, dropping any step the last one was in; the choice stays.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.step = Step::Idle;
        self.typed.clear();
        self.ghost = None;
    }
    /// Forgets the steps, the choice and the undo history, as for a different sketch.
    pub fn reset(&mut self) {
        *self = Self { tool: self.tool, construction: self.construction, radius: self.radius, chamfer: self.chamfer, offset: self.offset, version: self.version + 1, ..Self::default() };
    }
    fn typed(&self, key: &str) -> Option<f64> {
        self.typed.iter().rev().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
    fn at(&self) -> Option<[f64; 2]> {
        self.pointer.map(|p| p.at)
    }

    /// Runs `f` on a copy of the sketch and keeps it, with an undo step, only when it succeeds.
    fn edit<T>(&mut self, s: &mut Sketch, f: impl FnOnce(&mut Sketch) -> anyhow::Result<T>) -> Result<T, String> {
        let mut next = s.clone();
        let out = f(&mut next).map_err(|e| format!("{e:#}"))?;
        next.validate().map_err(|e| format!("{e:#}"))?;
        self.undo.push(std::mem::replace(s, next));
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.version += 1;
        Ok(out)
    }
    fn done(&mut self, result: Result<String, String>) -> Outcome {
        self.ghost = None;
        match result {
            Ok(words) => {
                self.typed.clear();
                Outcome::Edited(words)
            }
            Err(why) => Outcome::Refused(why),
        }
    }

    /// Takes the last edit back.
    pub fn undo(&mut self, s: &mut Sketch) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(s, prev));
        self.after_history(s);
        true
    }
    pub fn redo(&mut self, s: &mut Sketch) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(s, next));
        self.after_history(s);
        true
    }
    fn after_history(&mut self, s: &Sketch) {
        self.version += 1;
        self.step = Step::Idle;
        self.typed.clear();
        self.ghost = None;
        self.dragged = None;
        self.chosen_points.retain(|id| s.points.iter().any(|p| p.id == *id));
        self.chosen_entities.retain(|id| s.entities.iter().any(|e| e.id == *id));
    }

    /// Starts moving `point` by hand; false for a point that is missing or fixed.
    pub fn begin_drag(&mut self, s: &Sketch, point: Id) -> bool {
        if !s.points.iter().any(|p| p.id == point && !p.fixed) {
            return false;
        }
        self.dragged = Some(s.clone());
        true
    }
    /// Moves `point` to `xy` and solves with it held there; a place the drawing or its constraints cannot reach is not taken.
    pub fn drag_to(&mut self, s: &mut Sketch, point: Id, xy: [f64; 2]) {
        let Some(i) = s.points.iter().position(|p| p.id == point && !p.fixed) else { return };
        let mut next = s.clone();
        next.points[i].xy = xy;
        next.points[i].fixed = true;
        let Ok(solved) = next.solve() else { return };
        let mut next = solved.sketch;
        next.points[i].fixed = false;
        if next.validate().is_err() {
            return;
        }
        *s = next;
        self.version += 1;
    }
    /// Ends a drag: one that moved anything is one undo step.
    pub fn end_drag(&mut self, s: &Sketch) {
        let Some(before) = self.dragged.take() else { return };
        if before != *s {
            self.undo.push(before);
            if self.undo.len() > UNDO_DEPTH {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Removes what is chosen.
    pub fn delete_chosen(&mut self, s: &mut Sketch) -> Outcome {
        if self.chosen_points.is_empty() && self.chosen_entities.is_empty() {
            return Outcome::Refused("Choose a point or a curve to delete first".into());
        }
        let (points, entities) = (std::mem::take(&mut self.chosen_points), std::mem::take(&mut self.chosen_entities));
        let result = self.edit(s, |s| Ok(delete(s, &points, &entities))).map(|n| format!("Deleted {n} from the sketch"));
        self.step = Step::Idle;
        self.done(result)
    }

    /// Turns the chosen curves to construction and back, or with none chosen, what is drawn next.
    pub fn toggle_construction(&mut self, s: &mut Sketch) -> Outcome {
        if self.chosen_entities.is_empty() {
            self.construction = !self.construction;
            return Outcome::Continue;
        }
        let ids = self.chosen_entities.clone();
        let result = self.edit(s, |s| {
            for e in s.entities.iter_mut().filter(|e| ids.contains(&e.id)) {
                e.construction = !e.construction;
            }
            Ok(())
        });
        self.done(result.map(|_| "Construction toggled".into()))
    }

    /// The escape ladder: a typed value, then the step or the choice, then the tool; `Out` once nothing is left.
    pub fn escape(&mut self, s: &Sketch) -> Escaped {
        if self.typed.pop().is_some() {
            self.refresh(Some(s));
            return Escaped::Field;
        }
        if self.step != Step::Idle {
            self.step = match self.step {
                Step::ArcEnd(a, _) => Step::ArcStart(a),
                _ => Step::Idle,
            };
            self.ghost = None;
            return Escaped::Step;
        }
        if !self.chosen_points.is_empty() || !self.chosen_entities.is_empty() {
            self.chosen_points.clear();
            self.chosen_entities.clear();
            return Escaped::Step;
        }
        if self.tool != Tool::Select {
            self.set_tool(Tool::Select);
            return Escaped::Tool;
        }
        Escaped::Out
    }

    /// Feeds one token.
    pub fn feed(&mut self, s: &mut Sketch, input: Input) -> Outcome {
        match input {
            Input::Pointer { raw, snapped, reach } => {
                self.pointer = Some(Pointer { raw, at: snapped.map_or(raw, |p| p.xy), reach });
                self.refresh(Some(&*s));
                Outcome::Continue
            }
            Input::Typed { key, value } => {
                self.typed.retain(|(k, _)| *k != key);
                self.typed.push((key, value));
                self.refresh(Some(&*s));
                Outcome::Continue
            }
            Input::Cleared { key } => {
                self.typed.retain(|(k, _)| *k != key);
                self.refresh(Some(&*s));
                Outcome::Continue
            }
            Input::Click { add } => self.click(s, add),
            Input::Confirm => self.confirm(s),
            Input::Close => self.close(s),
        }
    }

    fn click(&mut self, s: &mut Sketch, add: bool) -> Outcome {
        let Some(p) = self.pointer else { return Outcome::Continue };
        let at = p.at;
        match self.tool {
            Tool::Select => {
                if !add {
                    self.chosen_points.clear();
                    self.chosen_entities.clear();
                }
                match pick(s, p.raw, p.reach) {
                    Some(Hit::Point(id)) => toggle(&mut self.chosen_points, id),
                    Some(Hit::Curve { entity, .. }) => toggle(&mut self.chosen_entities, entity),
                    None => {}
                }
                Outcome::Continue
            }
            Tool::Line => match self.step {
                Step::Idle => {
                    self.step = Step::Chain { first: at, last: at, lines: 0 };
                    Outcome::Continue
                }
                _ => self.place_line(s),
            },
            Tool::Rectangle => match self.step {
                Step::Corner(_) => self.place_rectangle(s),
                _ => {
                    self.step = Step::Corner(at);
                    Outcome::Continue
                }
            },
            Tool::Circle => match self.step {
                Step::Centre(_) => self.place_circle(s),
                _ => {
                    self.step = Step::Centre(at);
                    Outcome::Continue
                }
            },
            Tool::Arc => match self.step.clone() {
                Step::ArcStart(a) if distance(a, at) > SAME_MM => {
                    self.step = Step::ArcEnd(a, at);
                    Outcome::Continue
                }
                Step::ArcStart(_) => Outcome::Continue,
                Step::ArcEnd(a, b) => {
                    let construction = self.construction;
                    let result = self.edit(s, |s| s.add_arc_through(a, b, at, construction)).map(|_| "Arc".to_string());
                    if result.is_ok() {
                        self.step = Step::Idle;
                    }
                    self.done(result)
                }
                _ => {
                    self.step = Step::ArcStart(at);
                    Outcome::Continue
                }
            },
            Tool::Trim => {
                let Some(near) = s.nearest_entity(p.raw, p.reach) else { return Outcome::Refused("Click on the span of a curve to trim".into()) };
                let result = self.edit(s, |s| s.trim(near.entity, near.at)).map(|_| "Trimmed".to_string());
                self.done(result)
            }
            Tool::Offset => match self.step {
                Step::Loop { .. } => self.confirm(s),
                _ => {
                    let Some(near) = s.nearest_entity(p.raw, p.reach) else { return Outcome::Refused("Click a curve of a closed loop to offset".into()) };
                    match loop_region(s, near.entity) {
                        Ok((ids, region)) => {
                            self.step = Step::Loop { ids, region: Box::new(region) };
                            self.refresh(Some(&*s));
                            Outcome::Continue
                        }
                        Err(why) => Outcome::Refused(why),
                    }
                }
            },
            Tool::Fillet | Tool::Chamfer => match self.step {
                Step::CornerPoint(_) => self.confirm(s),
                _ => {
                    let Some(id) = s.nearest_point(p.raw, p.reach) else { return Outcome::Refused("Click a corner where two lines meet".into()) };
                    // Tries a hair of a cut to check the corner.
                    let mut probe = s.clone();
                    let tried = if self.tool == Tool::Fillet { probe.fillet_corner(id, 1e-3).map(|_| ()) } else { probe.chamfer_corner(id, 1e-3).map(|_| ()) };
                    match tried {
                        Err(e) => Outcome::Refused(format!("{e:#}")),
                        Ok(()) => {
                            self.step = Step::CornerPoint(id);
                            self.refresh(Some(&*s));
                            Outcome::Continue
                        }
                    }
                }
            },
            Tool::Mirror => match self.step {
                Step::MirrorAxis => {
                    let Some(near) = s.nearest_entity(p.raw, p.reach) else { return Outcome::Refused("Click the line to mirror in".into()) };
                    let axis = near.entity;
                    let chosen: Vec<Id> = self.chosen_entities.iter().copied().filter(|id| *id != axis).collect();
                    let result = self.edit(s, |s| s.mirror(&chosen, axis)).map(|made| format!("Mirrored {} curves", made.len()));
                    if result.is_ok() {
                        self.step = Step::Idle;
                        self.chosen_entities.clear();
                    }
                    self.done(result)
                }
                _ => {
                    match s.nearest_entity(p.raw, p.reach) {
                        Some(near) => toggle(&mut self.chosen_entities, near.entity),
                        None if !add => self.chosen_entities.clear(),
                        None => {}
                    }
                    Outcome::Continue
                }
            },
            Tool::Dimension => match self.step.clone() {
                Step::Measure(_) => self.confirm(s),
                Step::FirstPoint(a) => match s.nearest_point(p.raw, p.reach) {
                    Some(b) if b != a => {
                        self.step = Step::Measure(Measure::Distance { a, b });
                        Outcome::Continue
                    }
                    _ => Outcome::Refused("Click a second point for the distance between them".into()),
                },
                _ => {
                    if let Some(a) = s.nearest_point(p.raw, p.reach) {
                        self.step = Step::FirstPoint(a);
                        return Outcome::Continue;
                    }
                    let Some(near) = s.nearest_entity(p.raw, p.reach) else { return Outcome::Refused("Click a line, a circle or arc, or a point".into()) };
                    match s.measure_of(near.entity, near.at) {
                        Ok(m) => {
                            self.step = Step::Measure(m);
                            Outcome::Continue
                        }
                        Err(e) => Outcome::Refused(format!("{e:#}")),
                    }
                }
            },
        }
    }

    fn confirm(&mut self, s: &mut Sketch) -> Outcome {
        match self.step.clone() {
            Step::Chain { .. } => self.place_line(s),
            Step::Corner(_) => self.place_rectangle(s),
            Step::Centre(_) => self.place_circle(s),
            Step::Loop { ids, .. } => {
                let Some(d) = self.offset_distance() else { return Outcome::Refused("Move off the loop or type a distance".into()) };
                let result = self.edit(s, |s| s.offset(&ids, d)).map(|_| format!("Offset {d:.3} mm"));
                if result.is_ok() {
                    self.offset = d;
                    self.step = Step::Idle;
                }
                self.done(result)
            }
            Step::CornerPoint(id) => {
                let fillet = self.tool == Tool::Fillet;
                let v = self.typed(if fillet { "radius" } else { "distance" }).unwrap_or(if fillet { self.radius } else { self.chamfer });
                let result = if fillet {
                    self.edit(s, |s| s.fillet_corner(id, v)).map(|_| format!("Fillet {v:.3} mm"))
                } else {
                    self.edit(s, |s| s.chamfer_corner(id, v)).map(|_| format!("Chamfer {v:.3} mm"))
                };
                if result.is_ok() {
                    if fillet { self.radius = v } else { self.chamfer = v }
                    self.step = Step::Idle;
                }
                self.done(result)
            }
            Step::Measure(m) => {
                let value = match self.typed(measure_key(&m)) {
                    Some(v) => v,
                    None => match s.measured(&m) {
                        Ok(v) => v,
                        Err(e) => return Outcome::Refused(format!("{e:#}")),
                    },
                };
                let clock = std::time::Instant::now();
                let result = self.edit(s, |s| s.dimension(&m, value)).map(|_| format!("{} {value:.3} mm", m.label()));
                self.last_solve_ms = Some(clock.elapsed().as_secs_f64() * 1e3);
                if result.is_ok() {
                    self.step = Step::Idle;
                }
                self.done(result)
            }
            Step::Idle if self.tool == Tool::Mirror => {
                if self.chosen_entities.is_empty() {
                    return Outcome::Refused("Click the curves to mirror first".into());
                }
                self.step = Step::MirrorAxis;
                Outcome::Continue
            }
            _ => Outcome::Continue,
        }
    }

    fn close(&mut self, s: &mut Sketch) -> Outcome {
        let Step::Chain { first, last, lines } = self.step else { return Outcome::Continue };
        if lines < 2 || distance(first, last) <= SAME_MM {
            return Outcome::Refused("A loop needs two lines before it can close".into());
        }
        let construction = self.construction;
        let result = self.edit(s, |s| {
            let (a, b) = (s.place(last), s.place(first));
            s.add_line(a, b, construction)
        });
        if result.is_ok() {
            self.step = Step::Idle;
        }
        self.done(result.map(|_| "Closed the loop".into()))
    }

    /// Where the line being drawn would end: the pointer, or the typed length and angle from the last point.
    fn line_end(&self) -> Option<[f64; 2]> {
        let Step::Chain { last, .. } = self.step else { return None };
        let at = self.at().unwrap_or(last);
        let (dx, dy) = (at[0] - last[0], at[1] - last[1]);
        let angle = self.typed("angle").map_or(dy.atan2(dx), f64::to_radians);
        let length = self.typed("length").unwrap_or(dx.hypot(dy));
        Some([last[0] + length * angle.cos(), last[1] + length * angle.sin()])
    }

    fn place_line(&mut self, s: &mut Sketch) -> Outcome {
        let Step::Chain { first, last, lines } = self.step else { return Outcome::Continue };
        let Some(end) = self.line_end() else { return Outcome::Continue };
        if distance(end, last) <= SAME_MM {
            return Outcome::Continue;
        }
        let (construction, typed) = (self.construction, self.typed("length"));
        let result = self.edit(s, |s| {
            let (a, b) = (s.place(last), s.place(end));
            let id = s.add_line(a, b, construction)?;
            if let Some(mm) = typed {
                s.constraints.push(Constraint::Distance { a, b, mm });
            }
            Ok(id)
        });
        let closes = lines >= 1 && distance(end, first) <= SAME_MM;
        match result {
            Ok(_) => {
                self.step = if closes { Step::Idle } else { Step::Chain { first, last: end, lines: lines + 1 } };
                self.typed.clear();
                Outcome::Edited(if closes { "Closed the loop".into() } else { "Line".into() })
            }
            Err(why) => Outcome::Refused(why),
        }
    }

    /// The rectangle's far corner: toward the pointer, its sides the typed width and height where typed.
    fn rectangle_corner(&self) -> Option<[f64; 2]> {
        let Step::Corner(a) = self.step else { return None };
        let at = self.at().unwrap_or(a);
        let side = |k: usize, key: &str| {
            let d = at[k] - a[k];
            let sign = if d < 0.0 { -1.0 } else { 1.0 };
            a[k] + self.typed(key).map_or(d, |v| sign * v.abs())
        };
        Some([side(0, "width"), side(1, "height")])
    }

    fn place_rectangle(&mut self, s: &mut Sketch) -> Outcome {
        let (Step::Corner(a), Some(b)) = (self.step.clone(), self.rectangle_corner()) else { return Outcome::Continue };
        let (construction, width, height) = (self.construction, self.typed("width"), self.typed("height"));
        let result = self.edit(s, |s| {
            let lines = s.add_rectangle(a, b, construction)?;
            for (line, mm) in [(lines[0], width), (lines[1], height)] {
                if let (Some(mm), Some(Geometry::Line { a, b })) = (mm, s.entities.iter().find(|e| e.id == line).map(|e| e.geometry.clone())) {
                    s.constraints.push(Constraint::Distance { a, b, mm: mm.abs() });
                }
            }
            Ok(())
        });
        if result.is_ok() {
            self.step = Step::Idle;
        }
        self.done(result.map(|_| format!("Rectangle {:.3} × {:.3} mm", (b[0] - a[0]).abs(), (b[1] - a[1]).abs())))
    }

    /// The circle's rim: toward the pointer, at the typed radius where typed.
    fn circle_rim(&self) -> Option<([f64; 2], f64)> {
        let Step::Centre(c) = self.step else { return None };
        let at = self.at().unwrap_or(c);
        let r = self.typed("radius").unwrap_or(distance(at, c));
        let dir = unit([at[0] - c[0], at[1] - c[1]]).unwrap_or([1.0, 0.0]);
        Some(([c[0] + dir[0] * r, c[1] + dir[1] * r], r))
    }

    fn place_circle(&mut self, s: &mut Sketch) -> Outcome {
        let (Step::Centre(c), Some((rim, r))) = (self.step.clone(), self.circle_rim()) else { return Outcome::Continue };
        let (construction, typed) = (self.construction, self.typed("radius"));
        let result = self.edit(s, |s| {
            let id = s.add_circle(c, rim, construction)?;
            if let (Some(mm), Some(Geometry::Circle { center, rim })) = (typed, s.entities.iter().find(|e| e.id == id).map(|e| e.geometry.clone())) {
                s.constraints.push(Constraint::Distance { a: center, b: rim, mm });
            }
            Ok(())
        });
        if result.is_ok() {
            self.step = Step::Idle;
        }
        self.done(result.map(|_| format!("Circle R{r:.3} mm")))
    }

    /// The offset's distance: typed, else the pointer's from the loop, outward positive.
    fn offset_distance(&self) -> Option<f64> {
        let Step::Loop { region, .. } = &self.step else { return None };
        if let Some(d) = self.typed("distance") {
            return (d.abs() > 1e-6).then_some(d);
        }
        let at = self.at()?;
        let polygons = region.polygons(0.01);
        let d = polygons.iter().filter_map(|l| {
            let mut closed = l.clone();
            closed.extend(l.first().copied());
            nearest_on(&closed, at).map(|(_, d)| d)
        }).fold(f64::INFINITY, f64::min);
        (d.is_finite() && d > 1e-6).then(|| if region.contains(at) { -d } else { d })
    }

    /// Recomputes what a confirm would give, for the preview.
    fn refresh(&mut self, s: Option<&Sketch>) {
        let Some(s) = s else {
            self.ghost = None;
            return;
        };
        self.ghost = match self.step.clone() {
            Step::Loop { ids, .. } => self.offset_distance().and_then(|d| {
                let mut g = s.clone();
                g.offset(&ids, d).ok().map(|_| g)
            }),
            Step::CornerPoint(id) => {
                let mut g = s.clone();
                let ok = if self.tool == Tool::Fillet {
                    g.fillet_corner(id, self.typed("radius").unwrap_or(self.radius)).is_ok()
                } else {
                    g.chamfer_corner(id, self.typed("distance").unwrap_or(self.chamfer)).is_ok()
                };
                ok.then_some(g)
            }
            _ => None,
        };
    }

    /// The numbers the live step takes, for a dimension bar.
    pub fn dimensions(&self, s: &Sketch) -> Vec<Dimension> {
        let field = |key: &'static str, label: &'static str, unit: Unit, live: f64| {
            let typed = self.typed(key);
            Dimension { key, label, unit, value: typed.unwrap_or(live), locked: typed.is_some() }
        };
        match &self.step {
            Step::Chain { last, .. } => {
                let at = self.at().unwrap_or(*last);
                vec![
                    field("length", "Length", Unit::Mm, distance(at, *last)),
                    field("angle", "Angle", Unit::Deg, (at[1] - last[1]).atan2(at[0] - last[0]).to_degrees()),
                ]
            }
            Step::Corner(a) => {
                let at = self.at().unwrap_or(*a);
                vec![field("width", "Width", Unit::Mm, (at[0] - a[0]).abs()), field("height", "Height", Unit::Mm, (at[1] - a[1]).abs())]
            }
            Step::Centre(c) => vec![field("radius", "Radius", Unit::Mm, distance(self.at().unwrap_or(*c), *c))],
            Step::Loop { .. } => vec![field("distance", "Distance", Unit::Mm, self.offset_distance().unwrap_or(self.offset))],
            Step::CornerPoint(_) if self.tool == Tool::Fillet => vec![field("radius", "Radius", Unit::Mm, self.radius)],
            Step::CornerPoint(_) => vec![field("distance", "Distance", Unit::Mm, self.chamfer)],
            Step::Measure(m) => vec![field(measure_key(m), m.label(), Unit::Mm, s.measured(m).unwrap_or(0.0))],
            _ => Vec::new(),
        }
    }

    /// What the live step draws.
    pub fn preview(&self, s: &Sketch) -> Preview {
        let mut out = Preview::default();
        let at = self.at();
        match &self.step {
            Step::Chain { last, .. } => {
                out.marks.push(*last);
                if let Some(end) = self.line_end() {
                    out.strokes.push(vec![*last, end]);
                    out.caption = format!("{:.3} mm", distance(*last, end));
                }
            }
            Step::Corner(a) => {
                if let Some(b) = self.rectangle_corner() {
                    out.strokes.push(vec![*a, [b[0], a[1]], b, [a[0], b[1]], *a]);
                    out.caption = format!("{:.3} × {:.3} mm", (b[0] - a[0]).abs(), (b[1] - a[1]).abs());
                }
                out.marks.push(*a);
            }
            Step::Centre(c) => {
                if let Some((_, r)) = self.circle_rim() {
                    out.strokes.push((0..=64).map(|k| {
                        let t = k as f64 / 64.0 * std::f64::consts::TAU;
                        [c[0] + r * t.cos(), c[1] + r * t.sin()]
                    }).collect());
                    out.caption = format!("R{r:.3} mm");
                }
                out.marks.push(*c);
            }
            Step::ArcStart(a) => {
                out.marks.push(*a);
                out.strokes.extend(at.map(|p| vec![*a, p]));
            }
            Step::ArcEnd(a, b) => {
                out.marks.extend([*a, *b]);
                let mut g = Sketch::default();
                if let Some(p) = at {
                    if let Ok(id) = g.add_arc_through(*a, *b, p, false) {
                        out.strokes.extend(g.polylines(id, 0.01));
                    }
                }
            }
            Step::CornerPoint(id) => out.marks.extend(s.at(*id).ok()),
            Step::FirstPoint(id) => {
                let a = s.at(*id).ok();
                out.marks.extend(a);
                if let (Some(a), Some(p)) = (a, at) {
                    out.strokes.push(vec![a, p]);
                }
            }
            Step::Measure(m) => {
                let points: Vec<[f64; 2]> = m.pairs().first().map(|(a, b)| [*a, *b]).into_iter().flatten().filter_map(|id| s.at(id).ok()).collect();
                out.strokes.push(points.clone());
                out.marks.extend(points);
                out.caption = format!("{} {:.3} mm", m.label(), self.typed(measure_key(m)).unwrap_or(s.measured(m).unwrap_or(0.0)));
            }
            Step::Loop { .. } => {
                if let Some(d) = self.offset_distance() {
                    out.caption = format!("{d:+.3} mm");
                }
            }
            Step::MirrorAxis | Step::Idle => {}
        }
        if let Some(g) = &self.ghost {
            for e in &g.entities {
                let same = s.entities.iter().any(|o| o.id == e.id && o.geometry == e.geometry && same_points(s, g, &e.geometry));
                if !same {
                    out.ghost.extend(g.polylines(e.id, 0.01));
                }
            }
        }
        out
    }

    /// The live tool's words: what to click next.
    pub fn prompt(&self) -> String {
        let step = match (&self.tool, &self.step) {
            (Tool::Select, _) => "click a point or curve to choose it; Shift adds, Delete removes",
            (Tool::Line, Step::Chain { lines: 0, .. }) => "click the next point, or type its length and angle",
            (Tool::Line, Step::Chain { .. }) => "click the next point; the first point or C closes, Escape ends it open",
            (Tool::Line, _) => "click the first point",
            (Tool::Rectangle, Step::Corner(_)) => "click the opposite corner, or type the width and height",
            (Tool::Rectangle, _) => "click the first corner",
            (Tool::Circle, Step::Centre(_)) => "click the rim, or type the radius",
            (Tool::Circle, _) => "click the centre",
            (Tool::Arc, Step::ArcStart(_)) => "click the end",
            (Tool::Arc, Step::ArcEnd(..)) => "click a point the arc passes through",
            (Tool::Arc, _) => "click the start",
            (Tool::Trim, _) => "click the span to cut away",
            (Tool::Offset, Step::Loop { .. }) => "move out or in, or type the distance; click or Enter makes it",
            (Tool::Offset, _) => "click a closed loop",
            (Tool::Fillet, Step::CornerPoint(_)) => "type the radius; Enter or a click rounds the corner",
            (Tool::Chamfer, Step::CornerPoint(_)) => "type the distance; Enter or a click cuts the corner",
            (Tool::Fillet | Tool::Chamfer, _) => "click a corner where two lines meet",
            (Tool::Mirror, Step::MirrorAxis) => "click the line to mirror in",
            (Tool::Mirror, _) => "click the curves to mirror, then press Enter",
            (Tool::Dimension, Step::FirstPoint(_)) => "click the second point",
            (Tool::Dimension, Step::Measure(_)) => "type the value; Enter holds it",
            (Tool::Dimension, _) => "click a line, a circle or arc, or a point",
        };
        format!("{}: {step}", self.tool.label())
    }

    /// Whether `id` is chosen, or picked for the live step.
    pub fn is_chosen(&self, id: Id) -> bool {
        self.chosen_entities.contains(&id) || matches!(&self.step, Step::Loop { ids, .. } if ids.contains(&id))
    }
}

/// The field key a measure's dimension is typed into.
fn measure_key(m: &Measure) -> &'static str {
    match m {
        Measure::Length { .. } => "length",
        Measure::Distance { .. } => "distance",
        Measure::Radius { .. } => "radius",
    }
}

fn toggle(set: &mut BTreeSet<Id>, id: Id) {
    if !set.remove(&id) {
        set.insert(id);
    }
}

/// Whether every point of `g` sits where it did in `s`.
fn same_points(s: &Sketch, g: &Sketch, geometry: &Geometry) -> bool {
    geometry.points().iter().all(|id| s.at(*id).ok() == g.at(*id).ok())
}

/// The closed loop `entity` runs through and the region it bounds on its own.
fn loop_region(s: &Sketch, entity: Id) -> Result<(Vec<Id>, Region), String> {
    let ids = s.loop_through(entity).map_err(|e| format!("{e:#}"))?;
    let mut only = s.clone();
    only.entities.retain(|e| ids.contains(&e.id));
    for e in &mut only.entities {
        e.construction = false;
    }
    let mut regions = only.profile_regions().map_err(|e| format!("{e:#}"))?;
    if regions.len() != 1 {
        return Err("Offset takes one closed loop".into());
    }
    Ok((ids, regions.remove(0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn at(t: &mut Tools, s: &mut Sketch, xy: [f64; 2]) {
        t.feed(s, Input::Pointer { raw: xy, snapped: None, reach: 0.2 });
    }
    fn click(t: &mut Tools, s: &mut Sketch, xy: [f64; 2]) -> Outcome {
        at(t, s, xy);
        t.feed(s, Input::Click { add: false })
    }
    fn length(s: &Sketch, id: Id) -> f64 {
        s.curves_of(s.entities.iter().find(|e| e.id == id).unwrap()).unwrap().iter().map(|c| c.length()).sum()
    }

    #[test]
    fn a_rectangle_dimensioned_filleted_and_trimmed_through_the_tools() {
        let (mut t, mut s) = (Tools::default(), Sketch::default());
        t.set_tool(Tool::Rectangle);
        click(&mut t, &mut s, [-2.0, -1.5]);
        at(&mut t, &mut s, [2.0, 1.5]);
        assert_eq!(t.dimensions(&s).iter().map(|d| (d.key, d.value)).collect::<Vec<_>>(), [("width", 4.0), ("height", 3.0)]);
        assert_eq!(t.preview(&s).strokes[0].len(), 5);
        assert!(matches!(t.feed(&mut s, Input::Click { add: false }), Outcome::Edited(_)));
        assert_eq!((s.entities.len(), s.points.len()), (4, 4));
        // Dimension the bottom to 3: the solver moves the corners and the square holds.
        t.set_tool(Tool::Dimension);
        assert_eq!(click(&mut t, &mut s, [0.0, -1.5]), Outcome::Continue);
        assert_eq!(t.dimensions(&s)[0].label, "Length");
        t.feed(&mut s, Input::Typed { key: "length", value: 3.0 });
        assert!(matches!(t.feed(&mut s, Input::Confirm), Outcome::Edited(w) if w == "Length 3.000 mm"));
        let bottom = s.entities[0].id;
        assert!((length(&s, bottom) - 3.0).abs() < 1e-5, "{}", length(&s, bottom));
        assert!(t.last_solve_ms.is_some());
        // Fillet the top right corner at 0.5: a quarter arc of length pi/4.
        t.set_tool(Tool::Fillet);
        let corner = s.points[2].xy;
        assert_eq!(click(&mut t, &mut s, corner), Outcome::Continue);
        t.feed(&mut s, Input::Typed { key: "radius", value: 0.5 });
        assert!(!t.preview(&s).ghost.is_empty(), "the rounded corner shows before it is made");
        assert!(matches!(t.feed(&mut s, Input::Confirm), Outcome::Edited(_)));
        let arc = s.entities.iter().find(|e| matches!(e.geometry, Geometry::Arc { .. })).unwrap().id;
        assert!((length(&s, arc) - PI / 4.0).abs() < 1e-6, "{}", length(&s, arc));
        assert_eq!(s.profile_regions().unwrap().len(), 1);
        // Undo takes the fillet back, redo puts it again.
        assert!(t.undo(&mut s));
        assert!(!s.entities.iter().any(|e| matches!(e.geometry, Geometry::Arc { .. })));
        assert!(t.redo(&mut s));
        assert!(s.entities.iter().any(|e| e.id == arc));
        // A trim of a line nothing crosses takes it whole.
        let left = s.entities[3].id;
        let end = s.polylines(left, 0.01)[0][0];
        let on = [end[0], end[1] - 0.5];
        t.set_tool(Tool::Trim);
        assert!(matches!(click(&mut t, &mut s, on), Outcome::Edited(_)));
        assert!(!s.entities.iter().any(|e| e.id == left));
    }

    #[test]
    fn a_line_chain_closes_on_its_first_point_or_on_c_and_escape_backs_out_step_by_step() {
        let (mut t, mut s) = (Tools::default(), Sketch::default());
        t.set_tool(Tool::Line);
        for p in [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]] {
            click(&mut t, &mut s, p);
        }
        assert_eq!(s.entities.len(), 2);
        // The first point, reached by its own snap, closes the loop.
        t.feed(&mut s, Input::Pointer { raw: [0.05, 0.05], snapped: Some(Snap { xy: [0.0, 0.0], kind: SnapKind::Endpoint }), reach: 0.2 });
        assert!(matches!(t.feed(&mut s, Input::Click { add: false }), Outcome::Edited(w) if w == "Closed the loop"));
        assert!(!t.busy() && s.entities.len() == 3 && s.points.len() == 3);
        assert_eq!(s.profile_regions().unwrap().len(), 1);
        // A typed length and angle place the next point exactly; C closes.
        let mut s = Sketch::default();
        click(&mut t, &mut s, [10.0, 0.0]);
        at(&mut t, &mut s, [11.0, 0.3]);
        t.feed(&mut s, Input::Typed { key: "length", value: 2.0 });
        t.feed(&mut s, Input::Typed { key: "angle", value: 90.0 });
        t.feed(&mut s, Input::Confirm);
        assert!(distance(s.points[1].xy, [10.0, 2.0]) < 1e-12, "{:?}", s.points[1].xy);
        assert!(s.constraints.iter().any(|c| matches!(c, Constraint::Distance { mm, .. } if *mm == 2.0)));
        assert!(matches!(t.feed(&mut s, Input::Close), Outcome::Refused(_)), "one line cannot close");
        click(&mut t, &mut s, [12.0, 2.0]);
        assert!(matches!(t.feed(&mut s, Input::Close), Outcome::Edited(_)));
        assert_eq!(s.profile_regions().unwrap().len(), 1);
        // Escape: a typed field, then the step, then the tool, then out.
        click(&mut t, &mut s, [20.0, 0.0]);
        t.feed(&mut s, Input::Typed { key: "length", value: 1.0 });
        assert_eq!(t.escape(&s), Escaped::Field);
        assert!(t.busy());
        assert_eq!(t.escape(&s), Escaped::Step);
        assert!(!t.busy());
        assert_eq!(t.escape(&s), Escaped::Tool);
        assert_eq!(t.tool, Tool::Select);
        assert_eq!(t.escape(&s), Escaped::Out);
    }

    #[test]
    fn select_and_delete_offset_a_loop_mirror_and_measure_two_points() {
        let (mut t, mut s) = (Tools::default(), Sketch::default());
        s.add_rectangle([0.0, 0.0], [4.0, 2.0], false).unwrap();
        let top = s.entities[2].id;
        // Offset: pick the loop, move 1 mm out, click.
        t.set_tool(Tool::Offset);
        assert_eq!(click(&mut t, &mut s, [2.0, 0.05]), Outcome::Continue);
        assert!(t.is_chosen(top), "the whole loop is picked");
        at(&mut t, &mut s, [2.0, -1.0]);
        assert_eq!(t.dimensions(&s)[0].value, 1.0);
        assert!(!t.preview(&s).ghost.is_empty());
        at(&mut t, &mut s, [2.0, 0.5]);
        assert_eq!(t.dimensions(&s)[0].value, -0.5, "inside is negative");
        t.feed(&mut s, Input::Typed { key: "distance", value: 1.0 });
        assert!(matches!(t.feed(&mut s, Input::Confirm), Outcome::Edited(w) if w == "Offset 1.000 mm"));
        let r = s.profile_regions().unwrap();
        assert_eq!((r.len(), r[0].holes.len()), (1, 1));
        assert!((r[0].area() - (6.0 * 4.0 - 8.0)).abs() < 1e-9, "{}", r[0].area());
        // Select a line and delete it.
        t.set_tool(Tool::Select);
        click(&mut t, &mut s, [2.0, 2.02]);
        assert_eq!(t.chosen_entities, BTreeSet::from([top]));
        let n = s.entities.len();
        assert!(matches!(t.delete_chosen(&mut s), Outcome::Edited(_)));
        assert_eq!(s.entities.len(), n - 1);
        assert!(matches!(t.delete_chosen(&mut s), Outcome::Refused(_)));
        // Mirror a line in a construction axis.
        let mut s = Sketch::default();
        let p = [[1.0, 0.0], [2.0, 1.0], [0.0, -1.0], [0.0, 3.0]].map(|xy| s.point(xy));
        let line = s.draw(Geometry::Line { a: p[0], b: p[1] }, false);
        let axis = s.draw(Geometry::Line { a: p[2], b: p[3] }, true);
        t.set_tool(Tool::Mirror);
        click(&mut t, &mut s, [1.5, 0.5]);
        assert!(t.chosen_entities.contains(&line));
        t.feed(&mut s, Input::Confirm);
        assert!(matches!(click(&mut t, &mut s, [0.0, 2.0]), Outcome::Edited(_)));
        assert_eq!(s.entities.len(), 3);
        assert!(s.points.iter().any(|q| distance(q.xy, [-2.0, 1.0]) < 1e-12));
        assert!(s.entities.iter().any(|e| e.id == axis));
        // Dimension two points apart.
        t.set_tool(Tool::Dimension);
        click(&mut t, &mut s, [1.0, 0.0]);
        click(&mut t, &mut s, [-1.0, 0.0]);
        assert_eq!(t.dimensions(&s)[0].label, "Distance");
        t.feed(&mut s, Input::Typed { key: "distance", value: 3.0 });
        t.feed(&mut s, Input::Confirm);
        let (a, b) = (s.points.iter().find(|q| q.id == p[0]).unwrap().xy, s.points.iter().find(|q| distance(q.xy, [-1.0, 0.0]) < 1.0 && q.id != p[0]).unwrap().xy);
        assert!((distance(a, b) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn the_snap_prefers_points_to_edges_to_the_grid_and_reads_the_face_under_the_sketch() {
        let mut s = Sketch::default();
        s.add_rectangle([0.0, 0.0], [4.0, 2.0], false).unwrap();
        let under = Underlay { face: vec![vec![[-5.0, -5.0], [5.0, -5.0], [5.0, 5.0], [-5.0, 5.0]]], cut: vec![] };
        let cache = SnapCache::of(&s, &under);
        let snap = |xy: [f64; 2], reach: f64| super::snap(&s, &under, &cache, xy, reach, Some(0.5)).map(|p| (p.kind, p.xy));
        assert_eq!(snap([3.9, 0.1], 0.3), Some((SnapKind::Endpoint, [4.0, 0.0])));
        assert_eq!(snap([2.05, 0.1], 0.3), Some((SnapKind::Midpoint, [2.0, 0.0])));
        assert_eq!(snap([4.9, 3.0], 0.3), Some((SnapKind::Edge, [5.0, 3.0])));
        assert_eq!(snap([4.8, -4.9], 0.3), Some((SnapKind::Corner, [5.0, -5.0])));
        assert_eq!(snap([0.1, -0.1], 0.3), Some((SnapKind::Endpoint, [0.0, 0.0])), "the rectangle's corner outranks the origin it sits on");
        assert_eq!(snap([-2.1, 0.1], 0.3), Some((SnapKind::Axis, [-2.1, 0.0])));
        assert_eq!(snap([-2.6, 3.1], 0.3), Some((SnapKind::Grid, [-2.5, 3.0])));
        assert_eq!(snap([-2.73, 3.23], 0.1), None, "beyond every reach it stays free");
        assert_eq!(under.corners().len(), 4);
        // Where curves cross is an intersection, and a centre is not an endpoint.
        let mut s = Sketch::default();
        s.add_circle([0.0, 0.0], [1.0, 0.0], false).unwrap();
        let p = [[-2.0, 0.5], [2.0, 0.5]].map(|xy| s.point(xy));
        s.draw(Geometry::Line { a: p[0], b: p[1] }, false);
        let cache = SnapCache::of(&s, &Underlay::default());
        let x = 0.75f64.sqrt();
        let hit = super::snap(&s, &Underlay::default(), &cache, [x + 0.05, 0.45], 0.2, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert!(distance(hit.xy, [x, 0.5]) < 1e-9);
        assert_eq!(super::snap(&s, &Underlay::default(), &cache, [0.05, -0.05], 0.2, None).unwrap().kind, SnapKind::Centre);
        assert_eq!(pick(&s, [0.0, 0.52], 0.1), Some(Hit::Curve { entity: s.entities[1].id, at: [0.0, 0.5] }));
        assert_eq!(annotations(&s), Vec::new());
    }

    #[test]
    fn a_dragged_corner_carries_its_rectangle_and_a_held_side_and_the_drag_is_one_undo_step() {
        let (mut t, mut s) = (Tools::default(), Sketch::default());
        s.add_rectangle([0.0, 0.0], [4.0, 2.0], false).unwrap();
        let before = s.clone();
        let ids: Vec<Id> = s.points.iter().map(|p| p.id).collect();
        let xy = |s: &Sketch| -> Vec<[f64; 2]> { s.points.iter().map(|p| p.xy).collect() };
        let near = |a: &[[f64; 2]], b: [[f64; 2]; 4]| a.iter().zip(b).all(|(p, q)| distance(*p, q) < 1e-5);
        // The corner lands where it is dragged, and its neighbours follow so the rectangle stays square.
        assert!(t.begin_drag(&s, ids[2]));
        for k in 1..=10 {
            t.drag_to(&mut s, ids[2], [4.0 + 0.1 * f64::from(k), 2.0 + 0.05 * f64::from(k)]);
        }
        t.end_drag(&s);
        assert!(near(&xy(&s), [[0.0, 0.0], [5.0, 0.0], [5.0, 2.5], [0.0, 2.5]]), "{:?}", xy(&s));
        assert!(!s.points[2].fixed);
        assert!((s.profile_regions().unwrap()[0].area() - 12.5).abs() < 1e-4);
        assert!(t.undo(&mut s));
        assert_eq!(s, before);
        assert!(!t.can_undo(), "the whole drag was one step");
        // With the bottom held at 4 mm the far side follows the drag.
        s.dimension(&Measure::Length { a: ids[0], b: ids[1] }, 4.0).unwrap();
        assert!(t.begin_drag(&s, ids[2]));
        for k in 1..=10 {
            t.drag_to(&mut s, ids[2], [4.0 + 0.2 * f64::from(k), 2.0 + 0.1 * f64::from(k)]);
        }
        t.end_drag(&s);
        assert!(near(&xy(&s), [[2.0, 0.0], [6.0, 0.0], [6.0, 3.0], [2.0, 3.0]]), "{:?}", xy(&s));
        // A fixed point does not drag, and a place the constraints cannot reach is not taken.
        s.points[0].fixed = true;
        assert!(!t.begin_drag(&s, ids[0]));
        let held = s.clone();
        let steps = t.version();
        assert!(t.begin_drag(&s, ids[2]));
        t.drag_to(&mut s, ids[2], [7.0, 3.0]);
        t.end_drag(&s);
        assert_eq!((&s, t.version()), (&held, steps));
        assert!(t.undo(&mut s) && !t.can_undo(), "a drag that moved nothing left no step");
    }
}
