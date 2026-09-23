//! The feature timeline: one chip per CAD feature in document order, carrying its operation's mark,
//! its name and what the build that is current made of it. The strip edits nothing itself: a gesture
//! leaves as an [`Action`], and [`edits`] turns it into the [`CadEdit`]s a host hands its one funnel.
//! Everything but [`show`] is pure, so a test reads the chips, the menu and the drop rules without a
//! window.
use crate::{cad_tools, icons::Icon};
use egui::{
    Color32, CornerRadius, EventFilter, Key, Modifiers, Pos2, Rect, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2, WidgetInfo, WidgetType,
    text::{CCursor, CCursorRange, LayoutJob, TextWrapping},
};
use ringdesign_core::{
    cad::{Document, Evaluated, Feature, FeatureReport, FeatureStatus, Operation, edit::CadEdit},
    sketch::Id,
};
use std::collections::HashMap;

/// A failed feature's mark and border.
pub const FAILED: Color32 = Color32::from_rgb(237, 69, 92);
/// A skipped feature's mark and border.
pub const SKIPPED: Color32 = Color32::from_rgb(242, 194, 61);
/// What suppressing or deleting the procedural shank leaves.
pub const BAND_GONE: &str = "the parts become the whole ring";
/// Widest a chip's name runs on the one-row strip before it is elided.
const MAX_NAME: f32 = 150.0;
const PAD: f32 = 6.0;
const ICON: f32 = 16.0;
const GAP: f32 = 5.0;
const MARK: f32 = 8.0;

/// What the build made of a feature, as the strip shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Ok,
    Suppressed,
    /// Refused or broken, with the message.
    Failed(String),
    /// Not attempted behind a failed or skipped source, naming it.
    Skipped(String),
    /// Not evaluated since the document last changed.
    Pending,
}

impl Status {
    fn of(s: &FeatureStatus) -> Self {
        match s {
            FeatureStatus::Ok => Self::Ok,
            FeatureStatus::Suppressed => Self::Suppressed,
            FeatureStatus::Failed(m) => Self::Failed(m.clone()),
            FeatureStatus::Skipped(r) => Self::Skipped(r.clone()),
        }
    }
    /// The word a chip's accessible label ends in.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Suppressed => "suppressed",
            Self::Failed(_) => "failed",
            Self::Skipped(_) => "skipped",
            Self::Pending => "pending",
        }
    }
    /// The message behind a failure or the reason behind a skip.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Failed(m) | Self::Skipped(m) => Some(m),
            _ => None,
        }
    }
}

/// One feature on the strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub id: Id,
    pub name: String,
    pub icon: Icon,
    pub status: Status,
    pub enabled: bool,
    pub selected: bool,
    /// Past the rollback marker: kept in the document, not built.
    pub after_rollback: bool,
    /// Every feature that reads this one, directly or through another, in document order.
    pub dependents: Vec<Id>,
    /// The features this one reads directly, each once, in the order its operation names them.
    pub sources: Vec<Id>,
    /// The procedural shank the parts stand on.
    pub is_band: bool,
    /// What the feature is, for its hover text.
    pub hint: String,
}

impl Chip {
    /// `<name> · <status>`: what the chip answers to in the accessibility tree.
    pub fn label(&self) -> String {
        let word = if self.after_rollback && self.status == Status::Pending { "rolled back" } else { self.status.word() };
        format!("{} · {word}", self.name)
    }
}

/// What a gesture on the strip asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Select(Id),
    Edit(Id),
    Rename(Id, String),
    Enable(Id, bool),
    Delete(Id),
    DeleteWithDependents(Id),
    /// Sit after `after`, or first.
    Move { id: Id, after: Option<Id> },
    /// Build up to and including a feature, or everything.
    RollTo(Option<Id>),
    Isolate(Id),
}

/// Whether `reports` read as an evaluation of `doc` so far: its leading features, in order, under their names and suppression.
fn describes(doc: &Document, reports: &[FeatureReport]) -> bool {
    reports.len() <= doc.features.len() && reports.iter().zip(&doc.features).all(|(r, f)| r.id == f.id && r.name == f.name && r.suppressed != f.enabled)
}

fn distinct(ids: Vec<Id>) -> Vec<Id> {
    let mut out: Vec<Id> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Each feature's dependents in one pass over the readers graph, as `Document::dependents` gives them one at a time.
fn dependents(doc: &Document) -> Vec<Vec<Id>> {
    let n = doc.features.len();
    let at: HashMap<Id, usize> = doc.features.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
    let mut readers = vec![Vec::new(); n];
    for (i, f) in doc.features.iter().enumerate() {
        for s in f.operation.sources() {
            if let Some(&p) = at.get(&s) {
                if p != i && !readers[p].contains(&i) {
                    readers[p].push(i);
                }
            }
        }
    }
    (0..n)
        .map(|i| {
            let mut seen = vec![false; n];
            let mut stack = readers[i].clone();
            while let Some(j) = stack.pop() {
                if j != i && !seen[j] {
                    seen[j] = true;
                    stack.extend_from_slice(&readers[j]);
                }
            }
            (0..n).filter(|&j| seen[j]).map(|j| doc.features[j].id).collect()
        })
        .collect()
}

fn hint(f: &Feature) -> String {
    if matches!(f.operation, Operation::Band) {
        format!("Feature #{} · the procedural shank the parts stand on; suppress or delete it and {BAND_GONE}", f.id)
    } else {
        format!("Feature #{} · {}", f.id, f.operation.label())
    }
}

/// The strip's chips in document order. Statuses come from `evaluated` only while it reads as an
/// evaluation of `doc`; a feature it did not reach is `Pending`, and a suppressed one says so from
/// the document alone. A caller whose evaluation may be of other parameters passes `None`.
pub fn chips(doc: &Document, evaluated: Option<&Evaluated>, selected: &[Id]) -> Vec<Chip> {
    let reports: &[FeatureReport] = evaluated.map_or(&[], |e| &e.features);
    let reports = if describes(doc, reports) { reports } else { &[] };
    let cut = doc.through.and_then(|t| doc.position(t));
    doc.features
        .iter()
        .zip(dependents(doc))
        .enumerate()
        .map(|(i, (f, dependents))| Chip {
            id: f.id,
            name: f.name.clone(),
            icon: cad_tools::icon(&f.operation),
            status: if f.enabled { reports.get(i).map_or(Status::Pending, |r| Status::of(&r.status)) } else { Status::Suppressed },
            enabled: f.enabled,
            selected: selected.contains(&f.id),
            after_rollback: cut.is_some_and(|c| i > c),
            dependents,
            sources: distinct(f.operation.sources()),
            is_band: matches!(f.operation, Operation::Band),
            hint: hint(f),
        })
        .collect()
}

/// "1 feature depends on this: Chamfer".
fn depend_on_this(names: &[String]) -> String {
    let n = names.len();
    format!("{n} feature{} depend{} on this: {}", if n == 1 { "" } else { "s" }, if n == 1 { "s" } else { "" }, names.join(", "))
}

/// The edits an action stands for, in the order a funnel must apply them: none for the view's own
/// actions, one for a rename, a switch, a move or a rollback, and the dependents first, deepest
/// first, when a feature goes with what reads it. A plain delete of a feature something reads is
/// refused here, naming them.
pub fn edits(doc: &Document, action: &Action) -> Result<Vec<CadEdit>, String> {
    let known = |id: Id| doc.feature(id).ok_or_else(|| format!("No feature #{id} in the document"));
    Ok(match action {
        Action::Select(_) | Action::Edit(_) | Action::Isolate(_) => Vec::new(),
        Action::Rename(id, name) => vec![CadEdit::Rename { id: *id, name: name.clone() }],
        Action::Enable(id, enabled) => vec![CadEdit::Enable { id: *id, enabled: *enabled }],
        Action::Move { id, after } => vec![CadEdit::Move { id: *id, after: *after }],
        Action::RollTo(through) => vec![CadEdit::Through { through: *through }],
        Action::Delete(id) => {
            let f = known(*id)?;
            let readers = doc.dependents(*id);
            if !readers.is_empty() {
                let names: Vec<String> = readers.iter().map(|d| doc.feature(*d).map_or_else(|| format!("#{d}"), |f| f.name.clone())).collect();
                return Err(format!("Delete {}: {}", f.name, depend_on_this(&names)));
            }
            vec![CadEdit::Remove { id: *id }]
        }
        Action::DeleteWithDependents(id) => {
            known(*id)?;
            doc.dependents(*id).into_iter().rev().chain(std::iter::once(*id)).map(|id| CadEdit::Remove { id }).collect()
        }
    })
}

/// Whether the funnel would take `id` to sit after `after` (first on `None`), refused in its own words.
pub fn check_move(chips: &[Chip], id: Id, after: Option<Id>) -> Result<(), String> {
    let who = |v: Id| chips.iter().find(|c| c.id == v).map_or_else(|| format!("#{v}"), |c| format!("#{v} {}", c.name));
    let pos = chips.iter().position(|c| c.id == id).ok_or_else(|| format!("No feature #{id} in the document"))?;
    if let Some(a) = after {
        if a == id {
            return Err(format!("Move {}: after itself", who(id)));
        }
        if !chips.iter().any(|c| c.id == a) {
            return Err(format!("No feature #{a} in the document"));
        }
    }
    let mut order: Vec<Id> = chips.iter().map(|c| c.id).collect();
    order.remove(pos);
    let at = after.and_then(|a| order.iter().position(|v| *v == a)).map_or(0, |p| p + 1);
    order.insert(at, id);
    let index = |v: Id| order.iter().position(|o| *o == v);
    for s in &chips[pos].sources {
        if index(*s).is_some_and(|i| i >= at) {
            return Err(format!("Move {}: it would come before its source {}", who(id), who(*s)));
        }
    }
    for d in &chips[pos].dependents {
        if index(*d).is_some_and(|i| i <= at) {
            return Err(format!("Move {}: it would come after {}, which depends on it", who(id), who(*d)));
        }
    }
    Ok(())
}

/// What a menu item does once chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    Act(Action),
    /// Swap the chip for an inline name field.
    Rename,
}

/// One line of a chip's context menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: String,
    pub icon: Icon,
    pub choice: Choice,
    pub enabled: bool,
    pub hint: String,
}

impl Item {
    fn new(label: impl Into<String>, icon: Icon, choice: Choice, hint: impl Into<String>) -> Self {
        Self { label: label.into(), icon, choice, enabled: true, hint: hint.into() }
    }
    /// Disabled, saying why.
    fn refused(mut self, why: impl Into<String>) -> Self {
        self.enabled = false;
        self.hint = why.into();
        self
    }
}

/// What a right-click on chip `index` offers, every edit also reachable here without a drag.
pub fn menu(chips: &[Chip], index: usize, rollback: Option<Id>) -> Vec<Item> {
    let Some(c) = chips.get(index) else { return Vec::new() };
    let name = |id: Id| chips.iter().find(|x| x.id == id).map_or_else(|| format!("#{id}"), |x| x.name.clone());
    let readers: Vec<String> = c.dependents.iter().map(|d| name(*d)).collect();
    let mut items = vec![
        Item::new("Edit feature", Icon::Panel, Choice::Act(Action::Edit(c.id)), "Open the feature in the CAD pane with its parameters"),
        Item::new("Rename…", Icon::Engrave, Choice::Rename, "Type a new name; Enter keeps it, Escape leaves the old one"),
    ];
    items.push(if c.enabled {
        let hint = if c.is_band { format!("Keep the shank in the document but build without it: {BAND_GONE}") } else { "Keep the feature in the document but build without it".into() };
        Item::new("Suppress", Icon::Ghost, Choice::Act(Action::Enable(c.id, false)), hint)
    } else {
        Item::new("Unsuppress", Icon::Check, Choice::Act(Action::Enable(c.id, true)), "Build the feature again")
    });
    let delete_hint = if c.is_band { format!("Remove the procedural shank: {BAND_GONE}") } else { "Remove the feature; Undo brings it back".into() };
    let delete = Item::new("Delete", Icon::Delete, Choice::Act(Action::Delete(c.id)), delete_hint);
    items.push(if readers.is_empty() { delete } else { delete.refused(depend_on_this(&readers)) });
    if !readers.is_empty() {
        let n = readers.len();
        let mut hint = format!("Remove {} and {}", c.name, readers.join(", "));
        if c.is_band {
            hint.push_str(&format!("; {BAND_GONE}"));
        }
        items.push(Item::new(format!("Delete with {n} dependent{}", if n == 1 { "" } else { "s" }), Icon::Delete, Choice::Act(Action::DeleteWithDependents(c.id)), hint));
    }
    if rollback != Some(c.id) {
        items.push(Item::new("Roll back to here", Icon::History, Choice::Act(Action::RollTo(Some(c.id))), "Build up to this feature; the ones after it stay in the document"));
    }
    if rollback.is_some() {
        items.push(Item::new("Return to end", Icon::Reset, Choice::Act(Action::RollTo(None)), "Build every feature again"));
    }
    let step = |after: Option<Id>, label: &str, hint: &str| {
        let item = Item::new(label, Icon::Move, Choice::Act(Action::Move { id: c.id, after }), hint);
        match check_move(chips, c.id, after) {
            Ok(()) => item,
            Err(why) => item.refused(why),
        }
    };
    items.push(match index {
        0 => Item::new("Move earlier", Icon::Move, Choice::Act(Action::Move { id: c.id, after: None }), "").refused("Already the first feature"),
        _ => step(index.checked_sub(2).map(|j| chips[j].id), "Move earlier", "Swap places with the feature before it"),
    });
    items.push(match chips.get(index + 1) {
        Some(next) => step(Some(next.id), "Move later", "Swap places with the feature after it"),
        None => Item::new("Move later", Icon::Move, Choice::Act(Action::Move { id: c.id, after: Some(c.id) }), "").refused("Already the last feature"),
    });
    items.push(Item::new("Isolate", Icon::Layers, Choice::Act(Action::Isolate(c.id)), "Show this part alone in the CAD pane"));
    items
}

/// What a drag carries: a chip, or the rollback marker.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Carry {
    Chip(Id),
    Marker,
}

/// A drag's payload, bound to the strip it left so it lands only there.
#[derive(Clone, Copy, Debug)]
struct Carried {
    strip: egui::Id,
    what: Carry,
}

/// What dropping `what` into gap `k` (0 before the first chip, `chips.len()` after the last) does:
/// `None` where it already sits, else the action or the funnel's refusal. `marker` is the gap the
/// rollback marker sits in.
fn drop_action(chips: &[Chip], what: Carry, k: usize, marker: usize) -> Option<Result<Action, String>> {
    match what {
        Carry::Chip(id) => {
            let i = chips.iter().position(|c| c.id == id)?;
            if k == i || k == i + 1 {
                return None;
            }
            let after = k.checked_sub(1).map(|j| chips[j].id);
            Some(check_move(chips, id, after).map(|()| Action::Move { id, after }))
        }
        Carry::Marker if k == marker => None,
        Carry::Marker if k == 0 => Some(Err("The rollback marker stays after at least one feature".into())),
        Carry::Marker => Some(Ok(Action::RollTo((k < chips.len()).then(|| chips[k - 1].id)))),
    }
}

/// The gap a point falls in: how many chips' centres lie before it along the strip.
fn gap_at(rects: &[Rect], p: Pos2, vertical: bool) -> usize {
    rects.iter().filter(|r| if vertical { r.center().y < p.y } else { r.center().x < p.x }).count()
}

/// Where gap `k` sits along the strip.
fn gap_line(rects: &[Rect], k: usize, vertical: bool) -> f32 {
    let lo = |r: &Rect| if vertical { r.top() } else { r.left() };
    let hi = |r: &Rect| if vertical { r.bottom() } else { r.right() };
    match (k.checked_sub(1).and_then(|j| rects.get(j)), rects.get(k)) {
        (Some(a), Some(b)) => (hi(a) + lo(b)) * 0.5,
        (Some(a), None) => hi(a) + 2.0,
        (None, Some(b)) => lo(b) - 2.0,
        (None, None) => 0.0,
    }
}

/// The strips drawn last pass, each with its layer, and a Delete claimed over one of them this pass.
#[derive(Clone, Default)]
struct Strips {
    next: Vec<(Rect, egui::LayerId)>,
    shown: Vec<(Rect, egui::LayerId)>,
    claimed: Option<Pos2>,
}

fn strips_id() -> egui::Id {
    egui::Id::new("cad-timeline-strips")
}

/// Takes a bare Delete pressed over a strip before the host's own shortcuts read the pass's keys.
struct DeleteOverStrip;

impl egui::Plugin for DeleteOverStrip {
    fn debug_name(&self) -> &'static str {
        "cad-timeline-delete"
    }
    fn on_begin_pass(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let shown = ctx.data_mut(|d| {
            let s = d.get_temp_mut_or_default::<Strips>(strips_id());
            s.shown = std::mem::take(&mut s.next);
            s.claimed = None;
            s.shown.clone()
        });
        let Some(p) = ctx.pointer_hover_pos() else { return };
        let top = ctx.layer_id_at(p).unwrap_or_else(egui::LayerId::background);
        if !shown.iter().any(|(r, layer)| r.contains(p) && top == *layer) {
            return;
        }
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete)) {
            ctx.data_mut(|d| d.get_temp_mut_or_default::<Strips>(strips_id()).claimed = Some(p));
        }
    }
}

/// What a strip remembers between passes: the chip being renamed, its text, and whether the field has had focus.
#[derive(Clone, Default)]
struct Memory {
    rename: Option<(Id, String, bool)>,
}

/// Lays `text` out on one line no wider than `width`, elided.
fn one_line(ui: &Ui, text: &str, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = LayoutJob::single_section(text.to_owned(), egui::TextFormat::simple(font, color));
    job.wrap = TextWrapping { max_width: width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    ui.painter().layout_job(job)
}

fn chip_height(ui: &Ui) -> f32 {
    (ui.spacing().interact_size.y - 4.0).max(20.0)
}

/// One chip: allocated, painted, and answering to its label; its bare name rides along as a label node.
fn chip(ui: &mut Ui, c: &Chip, vertical: bool) -> Response {
    let dim = !c.enabled || c.after_rollback;
    let alpha = if dim { 0.45 } else { 1.0 };
    let marked = !matches!(c.status, Status::Ok | Status::Suppressed);
    let fixed = PAD + ICON + GAP + if marked { MARK + GAP } else { 0.0 } + PAD;
    let room = if vertical { (ui.available_width() - fixed).max(24.0) } else { MAX_NAME };
    let galley = one_line(ui, &c.name, ui.visuals().text_color().gamma_multiply(alpha), room);
    let width = if vertical { ui.available_width().max(fixed + 24.0) } else { fixed + galley.size().x };
    // A finger's drag scrolls the strip; a pointer's drag reorders the chip.
    let sense = if ui.input(|i| i.any_touches()) { Sense::click() } else { Sense::click_and_drag() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, chip_height(ui)), sense);
    let name_rect = Rect::from_min_size(Pos2::new(rect.left() + PAD + ICON + GAP, rect.center().y - galley.size().y * 0.5), galley.size());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let (fill, stroke) = if c.selected {
            (v.selection.bg_fill, v.selection.stroke)
        } else if response.hovered() {
            (v.widgets.hovered.weak_bg_fill, v.widgets.hovered.bg_stroke)
        } else {
            (v.widgets.inactive.weak_bg_fill, v.widgets.inactive.bg_stroke)
        };
        let stroke = match &c.status {
            Status::Failed(_) => Stroke::new(1.5, FAILED),
            Status::Skipped(_) => Stroke::new(1.0, SKIPPED),
            _ => stroke,
        };
        let weak = v.weak_text_color();
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(4), fill.gamma_multiply(if dim { 0.6 } else { 1.0 }), stroke, StrokeKind::Inside);
        let icon_rect = Rect::from_center_size(Pos2::new(rect.left() + PAD + ICON * 0.5, rect.center().y), Vec2::splat(ICON));
        c.icon.image(ui, ICON).tint(c.icon.color().gamma_multiply(alpha)).paint_at(ui, icon_rect);
        painter.galley(name_rect.min, galley, weak);
        if !c.enabled {
            painter.hline(name_rect.x_range(), name_rect.center().y, Stroke::new(1.0, weak.gamma_multiply(0.8)));
        }
        let mark = Pos2::new(rect.right() - PAD - MARK * 0.5, rect.center().y);
        match &c.status {
            Status::Failed(_) => {
                painter.circle_filled(mark, 4.0, FAILED);
            }
            Status::Skipped(_) => {
                painter.circle_stroke(mark, 3.5, Stroke::new(1.6, SKIPPED));
            }
            Status::Pending => {
                painter.circle_stroke(mark, 3.0, Stroke::new(1.0, weak));
            }
            Status::Ok | Status::Suppressed => {}
        }
    }
    let label = c.label();
    response.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, c.selected, &label));
    ui.interact(name_rect, response.id.with("name"), Sense::hover())
        .widget_info(|| WidgetInfo::selected(WidgetType::Label, true, c.selected, &c.name));
    response
}

/// The chip's hover text: its name, what became of it, and what it is.
fn tooltip(ui: &mut Ui, c: &Chip) {
    ui.set_max_width(320.0);
    ui.strong(&c.name);
    match &c.status {
        Status::Failed(m) => {
            ui.colored_label(FAILED, format!("Failed: {m}"));
        }
        Status::Skipped(r) => {
            ui.colored_label(SKIPPED, format!("Skipped: {r}"));
        }
        Status::Pending if c.after_rollback => {
            ui.weak("Rolled back: kept in the document, not built");
        }
        Status::Pending => {
            ui.weak("Waiting for the build");
        }
        Status::Suppressed => {
            ui.weak("Suppressed: kept in the document, not built");
        }
        Status::Ok => {}
    }
    ui.weak(&c.hint);
    if !c.dependents.is_empty() {
        ui.weak(format!("Read by {} later feature{}", c.dependents.len(), if c.dependents.len() == 1 { "" } else { "s" }));
    }
    ui.weak("Click to select, double-click to edit, drag to reorder, right-click for more");
}

/// The inline name field over a chip: the new name on Enter or a click away, `Err(())` on Escape.
fn rename_field(ui: &mut Ui, strip: egui::Id, c: &Chip, text: &mut String, focused: &mut bool, vertical: bool) -> (Response, Option<Result<String, ()>>) {
    let id = strip.with(("rename", c.id));
    let width = if vertical { ui.available_width() } else { MAX_NAME + PAD + ICON + GAP };
    let before = text.clone();
    let out = TextEdit::singleline(text)
        .id(id)
        .desired_width(width - 8.0)
        .min_size(Vec2::new(width, chip_height(ui)))
        .event_filter(EventFilter { escape: true, ..Default::default() })
        .show(ui);
    let response = out.response.response;
    let name = format!("Rename {}", c.name);
    let now = text.clone();
    response.widget_info(|| WidgetInfo { label: Some(name.clone()), ..WidgetInfo::text_edit(true, before.clone(), now.clone(), String::new()) });
    if !*focused {
        *focused = true;
        response.request_focus();
        let mut state = out.state;
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()))));
        state.store(ui.ctx(), id);
        return (response, None);
    }
    if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
        response.surrender_focus();
        ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        return (response, Some(Err(())));
    }
    if response.lost_focus() {
        ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        return (response, Some(Ok(text.trim().to_owned())));
    }
    if !response.has_focus() {
        return (response, Some(Err(())));
    }
    (response, None)
}

/// The rollback marker: a bar between two chips, or a faint one after the last while nothing is rolled back.
fn marker(ui: &mut Ui, rolled: bool, vertical: bool) -> Response {
    let h = chip_height(ui);
    let size = if vertical { Vec2::new(ui.available_width(), 8.0) } else { Vec2::new(10.0, h) };
    let (rect, response) = ui.allocate_exact_size(size, Sense::drag());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let color = if rolled { v.selection.stroke.color } else { v.weak_text_color().gamma_multiply(0.6) };
        let color = if response.hovered() || response.dragged() { v.widgets.hovered.bg_stroke.color } else { color };
        let painter = ui.painter();
        if vertical {
            painter.hline(rect.x_range(), rect.center().y, Stroke::new(2.0, color));
            painter.rect_filled(Rect::from_center_size(Pos2::new(rect.left() + 6.0, rect.center().y), Vec2::new(10.0, 6.0)), CornerRadius::same(2), color);
        } else {
            painter.vline(rect.center().x, rect.y_range(), Stroke::new(2.0, color));
            painter.rect_filled(Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 3.0), Vec2::new(8.0, 6.0)), CornerRadius::same(2), color);
        }
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, if rolled { "Rollback marker" } else { "End of the timeline" }));
    response.on_hover_text(if rolled {
        "Rollback marker: the features after it stay in the document but are not built. Drag it, or right-click a feature to roll back or return to the end."
    } else {
        "End of the timeline: drag this marker back over features to roll back to them"
    })
}

/// Paints what a drag carries under the pointer.
fn ghost(ui: &Ui, strip: egui::Id, text: &str, at: Pos2) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, strip.with("ghost")));
    let galley = one_line(ui, text, ui.visuals().text_color(), MAX_NAME);
    let rect = Rect::from_min_size(at + Vec2::new(12.0, 8.0), galley.size() + Vec2::new(2.0 * PAD, 8.0));
    painter.rect(rect, CornerRadius::same(4), ui.visuals().selection.bg_fill.gamma_multiply(0.9), ui.visuals().selection.stroke, StrokeKind::Inside);
    painter.galley(rect.min + Vec2::new(PAD, 4.0), galley, ui.visuals().text_color());
}

/// The strip, one row (`vertical` false) or one chip per line. Returns what its gestures ask for.
pub fn show(ui: &mut Ui, chips: &[Chip], rollback: Option<Id>, vertical: bool) -> Vec<Action> {
    let ctx = ui.ctx().clone();
    ctx.add_plugin(DeleteOverStrip);
    let strip = ui.make_persistent_id("cad-timeline");
    let mut memory: Memory = ui.data(|d| d.get_temp(strip)).unwrap_or_default();
    if memory.rename.as_ref().is_some_and(|(id, ..)| !chips.iter().any(|c| c.id == *id)) {
        memory.rename = None;
    }
    let marker_gap = rollback.and_then(|r| chips.iter().position(|c| c.id == r)).map_or(chips.len(), |i| i + 1);
    let mut actions = Vec::new();
    let mut rects = Vec::with_capacity(chips.len());
    // The chips' and marker's union, not the layout's whole cross extent.
    let mut span = Rect::NOTHING;
    let mut chosen = None;
    let layout = if vertical { egui::Layout::top_down(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Center) };
    ui.with_layout(layout, |ui| {
        if vertical {
            ui.spacing_mut().item_spacing.y = 2.0;
        }
        for (i, c) in chips.iter().enumerate() {
            if i == marker_gap {
                let m = marker(ui, true, vertical);
                span = span.union(m.rect);
                if m.drag_started() {
                    egui::DragAndDrop::set_payload(&ctx, Carried { strip, what: Carry::Marker });
                }
            }
            if let Some((id, text, focused)) = memory.rename.as_mut().filter(|(id, ..)| *id == c.id) {
                let id = *id;
                let (r, done) = rename_field(ui, strip, c, text, focused, vertical);
                rects.push(r.rect);
                span = span.union(r.rect);
                match done {
                    Some(Ok(name)) => {
                        if !name.is_empty() && name != c.name {
                            actions.push(Action::Rename(id, name));
                        }
                        memory.rename = None;
                    }
                    Some(Err(())) => memory.rename = None,
                    None => {}
                }
                continue;
            }
            let r = chip(ui, c, vertical);
            rects.push(r.rect);
            span = span.union(r.rect);
            if r.drag_started() {
                egui::DragAndDrop::set_payload(&ctx, Carried { strip, what: Carry::Chip(c.id) });
            }
            if r.double_clicked() {
                actions.push(Action::Edit(c.id));
            } else if r.clicked() {
                actions.push(Action::Select(c.id));
            }
            r.context_menu(|ui| {
                ui.set_min_width(210.0);
                for item in menu(chips, i, rollback) {
                    let button = egui::Button::new((item.icon.image(ui, 16.0), item.label.as_str()));
                    if ui.add_enabled(item.enabled, button).on_hover_text(&item.hint).on_disabled_hover_text(&item.hint).clicked() {
                        chosen = Some((c.id, c.name.clone(), item.choice.clone()));
                        ui.close();
                    }
                }
            });
            if !egui::DragAndDrop::has_any_payload(&ctx) {
                r.on_hover_ui(|ui| tooltip(ui, c));
            }
        }
        if marker_gap == chips.len() {
            let m = marker(ui, false, vertical);
            span = span.union(m.rect);
            if m.drag_started() {
                egui::DragAndDrop::set_payload(&ctx, Carried { strip, what: Carry::Marker });
            }
        }
    });
    match chosen {
        Some((id, name, Choice::Rename)) => memory.rename = Some((id, name, false)),
        Some((_, _, Choice::Act(a))) => actions.push(a),
        None => {}
    }
    let clip = ui.clip_rect();
    let hit = if vertical { span.expand2(Vec2::new(0.0, 3.0)) } else { Rect::from_x_y_ranges(clip.x_range(), span.y_range().expand(3.0)) }.intersect(clip);
    if let Some(carried) = egui::DragAndDrop::payload::<Carried>(&ctx).filter(|c| c.strip == strip) {
        let pointer = ctx.pointer_latest_pos();
        let text = match carried.what {
            Carry::Chip(id) => chips.iter().find(|c| c.id == id).map_or_else(String::new, |c| c.name.clone()),
            Carry::Marker => "Rollback marker".to_owned(),
        };
        if let Some(p) = pointer {
            ghost(ui, strip, &text, p);
        }
        if let Some(p) = pointer.filter(|p| hit.contains(*p)) {
            let k = gap_at(&rects, p, vertical);
            let verdict = drop_action(chips, carried.what, k, marker_gap);
            if let Some(verdict) = &verdict {
                let color = if verdict.is_ok() { ui.visuals().selection.stroke.color } else { FAILED };
                let at = gap_line(&rects, k, vertical);
                if vertical {
                    ui.painter().hline(hit.x_range(), at, Stroke::new(2.0, color));
                } else {
                    ui.painter().vline(at, span.y_range(), Stroke::new(2.0, color));
                }
                if let Err(reason) = verdict {
                    egui::Tooltip::always_open(ctx.clone(), ui.layer_id(), strip.with("refused"), egui::PopupAnchor::Pointer).show(|ui| {
                        ui.colored_label(FAILED, reason);
                    });
                }
            }
            if ctx.input(|i| i.pointer.any_released()) {
                egui::DragAndDrop::clear_payload(&ctx);
                if let Some(Ok(a)) = verdict {
                    actions.push(a);
                }
            }
        }
    }
    let claimed = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Strips>(strips_id());
        s.next.push((hit, ui.layer_id()));
        let mine = s.claimed.is_some_and(|p| hit.contains(p));
        if mine {
            s.claimed = None;
        }
        mine
    });
    if claimed {
        actions.extend(chips.iter().rev().filter(|c| c.selected).map(|c| Action::Delete(c.id)));
    }
    ui.data_mut(|d| d.insert_temp(strip, memory));
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        AlphaLibrary, BuildParams, RingDesign,
        cad::{self, Attach, Component, EdgeRef, FaceRef, Placement},
    };

    fn feature(id: Id, name: &str, operation: Operation) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation, component: Component::default() }
    }
    /// Band + Cylinder + Fillet on an edge the cylinder has not got + Chamfer on the Fillet + Box.
    fn document() -> Document {
        let mut doc = Document::default();
        doc.append(feature(1, "Procedural shank", Operation::Band)).unwrap();
        let mut post = feature(2, "Cylinder", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 });
        post.component.attach = Attach::Join;
        post.component.placement = Placement::ring(90.0, 0.25);
        doc.append(post).unwrap();
        doc.append(feature(3, "Fillet", Operation::Fillet { source: 2, edges: vec![EdgeRef::bare(999)], radius_mm: 0.3 })).unwrap();
        doc.append(feature(4, "Chamfer", Operation::Chamfer { source: 3, edges: vec![EdgeRef::bare(0)], base_face: FaceRef::bare(0), distance_mm: 0.2 })).unwrap();
        doc.append(feature(5, "Box", Operation::Box { size: [2.0, 2.0, 1.0] })).unwrap();
        doc
    }
    fn evaluate(doc: &Document) -> Evaluated {
        let d = RingDesign { cad: Some(doc.clone()), ..Default::default() };
        cad::evaluate(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 96, profile_steps: 48, refine: None, ..Default::default() }).unwrap()
    }

    #[test]
    fn chips_read_the_document_in_order_with_what_the_evaluation_made_of_each() {
        let doc = document();
        let e = evaluate(&doc);
        let chips = chips(&doc, Some(&e), &[2]);
        let labels: Vec<String> = chips.iter().map(Chip::label).collect();
        assert_eq!(labels, ["Procedural shank · ok", "Cylinder · ok", "Fillet · failed", "Chamfer · skipped", "Box · ok"]);
        assert_eq!(chips.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        let Status::Failed(why) = &chips[2].status else { unreachable!() };
        assert!(why.contains("999"), "{why}");
        assert_eq!(chips[3].status, Status::Skipped("source #3 Fillet failed".into()));
        assert!(chips[0].is_band && !chips[1].is_band);
        assert!(chips[0].hint.contains(BAND_GONE), "{}", chips[0].hint);
        assert_eq!((chips[0].icon, chips[2].icon, chips[4].icon), (Icon::CadBand, Icon::CadFillet, Icon::CadBox));
        assert_eq!(chips.iter().map(|c| c.selected).collect::<Vec<_>>(), [false, true, false, false, false]);
        assert_eq!(chips[1].dependents, [3, 4], "the chamfer reads the cylinder through the fillet");
        assert_eq!((chips[3].sources.as_slice(), chips[4].sources.as_slice()), ([3].as_slice(), [].as_slice()));
        assert!(chips.iter().all(|c| !c.after_rollback));
    }

    #[test]
    fn a_document_changed_since_its_evaluation_shows_pending_never_a_stale_red_or_green() {
        let doc = document();
        let e = evaluate(&doc);
        let words = |doc: &Document| chips(doc, Some(&e), &[]).iter().map(|c| c.status.word()).collect::<Vec<_>>();
        assert_eq!(words(&doc), ["ok", "ok", "failed", "skipped", "ok"]);
        assert_eq!(chips(&doc, None, &[]).iter().map(|c| c.status.word()).collect::<Vec<_>>(), ["pending"; 5]);
        // Moved, renamed, switched or shortened: nothing the evaluation said still holds.
        for edit in [
            CadEdit::Move { id: 5, after: None },
            CadEdit::Rename { id: 2, name: "Post".into() },
            CadEdit::Enable { id: 3, enabled: false },
            CadEdit::Remove { id: 5 },
        ] {
            let mut changed = doc.clone();
            changed.apply(&edit).unwrap();
            let got = words(&changed);
            let expected: Vec<&str> = changed.features.iter().map(|f| if f.enabled { "pending" } else { "suppressed" }).collect();
            assert_eq!(got, expected, "{edit:?}");
        }
        // A feature added after the ones evaluated is the only one waiting.
        let mut longer = doc.clone();
        longer.apply(&CadEdit::Add { feature: feature(0, "Sphere", Operation::Sphere { radius_mm: 1.0 }), after: None }).unwrap();
        assert_eq!(words(&longer), ["ok", "ok", "failed", "skipped", "ok", "pending"]);
        // A rolled-back document's evaluation stops at the marker; what lies past it waits and says so.
        let mut rolled = doc.clone();
        rolled.through = Some(2);
        let e = evaluate(&rolled);
        let c = chips(&rolled, Some(&e), &[]);
        assert_eq!(c.iter().map(Chip::label).collect::<Vec<_>>(), ["Procedural shank · ok", "Cylinder · ok", "Fillet · rolled back", "Chamfer · rolled back", "Box · rolled back"]);
        assert_eq!(c.iter().map(|c| c.after_rollback).collect::<Vec<_>>(), [false, false, true, true, true]);
    }

    #[test]
    fn dependents_match_the_documents_own_on_every_example() {
        let mut docs: Vec<Document> = cad::examples::NAMES.iter().map(|n| cad::examples::design(n).unwrap().cad.unwrap()).collect();
        docs.push(document());
        for doc in &docs {
            for c in chips(doc, None, &[]) {
                assert_eq!(c.dependents, doc.dependents(c.id), "#{}", c.id);
                assert_eq!(c.sources, doc.sources_of(c.id), "#{}", c.id);
            }
        }
    }

    #[test]
    fn every_action_becomes_the_edits_a_funnel_applies() {
        let doc = document();
        assert_eq!(edits(&doc, &Action::Select(2)), Ok(vec![]));
        assert_eq!(edits(&doc, &Action::Edit(2)), Ok(vec![]));
        assert_eq!(edits(&doc, &Action::Isolate(2)), Ok(vec![]));
        assert_eq!(edits(&doc, &Action::Rename(5, "Plate".into())), Ok(vec![CadEdit::Rename { id: 5, name: "Plate".into() }]));
        assert_eq!(edits(&doc, &Action::Enable(2, false)), Ok(vec![CadEdit::Enable { id: 2, enabled: false }]));
        assert_eq!(edits(&doc, &Action::Move { id: 5, after: Some(1) }), Ok(vec![CadEdit::Move { id: 5, after: Some(1) }]));
        assert_eq!(edits(&doc, &Action::RollTo(Some(2))), Ok(vec![CadEdit::Through { through: Some(2) }]));
        assert_eq!(edits(&doc, &Action::RollTo(None)), Ok(vec![CadEdit::Through { through: None }]));
        assert_eq!(edits(&doc, &Action::Delete(4)), Ok(vec![CadEdit::Remove { id: 4 }]));
        assert_eq!(edits(&doc, &Action::Delete(3)), Err("Delete Fillet: 1 feature depends on this: Chamfer".into()));
        assert_eq!(edits(&doc, &Action::Delete(2)), Err("Delete Cylinder: 2 features depend on this: Fillet, Chamfer".into()));
        assert_eq!(edits(&doc, &Action::Delete(9)), Err("No feature #9 in the document".into()));
        assert_eq!(edits(&doc, &Action::DeleteWithDependents(3)), Ok(vec![CadEdit::Remove { id: 4 }, CadEdit::Remove { id: 3 }]));
        let deep = edits(&doc, &Action::DeleteWithDependents(2)).unwrap();
        assert_eq!(deep, [CadEdit::Remove { id: 4 }, CadEdit::Remove { id: 3 }, CadEdit::Remove { id: 2 }]);
        // Each lands through the document's own funnel, in the order given.
        let mut after = doc.clone();
        for e in &deep {
            after.apply(e).unwrap();
        }
        assert_eq!(after.features.iter().map(|f| f.id).collect::<Vec<_>>(), [1, 5]);
        // A move is passed on as asked; the funnel's own validation refuses one past a source.
        let early = edits(&doc, &Action::Move { id: 4, after: Some(2) }).unwrap();
        let refused = doc.clone().apply(&early[0]).unwrap_err().to_string();
        assert_eq!(refused, "Move #4 Chamfer: it would come before its source #3 Fillet");
        for action in [Action::Rename(5, "Plate".into()), Action::Enable(3, false), Action::Move { id: 5, after: None }, Action::RollTo(Some(3)), Action::Delete(5)] {
            let mut d = doc.clone();
            for e in edits(&doc, &action).unwrap() {
                d.apply(&e).unwrap_or_else(|err| panic!("{action:?}: {err}"));
            }
        }
    }

    #[test]
    fn the_strip_refuses_exactly_the_moves_the_funnel_refuses_in_its_words() {
        let mut docs: Vec<Document> = cad::examples::NAMES.iter().map(|n| cad::examples::design(n).unwrap().cad.unwrap()).collect();
        docs.push(document());
        let mut refused = 0;
        for doc in &docs {
            let chips = chips(doc, None, &[]);
            let ids: Vec<Option<Id>> = std::iter::once(None).chain(doc.features.iter().map(|f| Some(f.id))).collect();
            for f in &doc.features {
                for after in &ids {
                    let funnel = doc.clone().apply(&CadEdit::Move { id: f.id, after: *after }).map(|_| ()).map_err(|e| e.to_string());
                    assert_eq!(check_move(&chips, f.id, *after), funnel, "#{} after {after:?}", f.id);
                    refused += usize::from(funnel.is_err());
                }
            }
        }
        assert!(refused > 10, "the sweep refuses something: {refused}");
    }

    #[test]
    fn a_drop_lands_where_it_is_let_go_and_a_refused_one_says_why() {
        let doc = document();
        let chips = chips(&doc, None, &[]);
        let end = chips.len();
        // Onto itself, either side: nothing to do.
        assert_eq!(drop_action(&chips, Carry::Chip(5), 4, end), None);
        assert_eq!(drop_action(&chips, Carry::Chip(5), 5, end), None);
        assert_eq!(drop_action(&chips, Carry::Chip(5), 0, end), Some(Ok(Action::Move { id: 5, after: None })));
        assert_eq!(drop_action(&chips, Carry::Chip(5), 1, end), Some(Ok(Action::Move { id: 5, after: Some(1) })));
        assert_eq!(drop_action(&chips, Carry::Chip(2), 5, end), Some(Err("Move #2 Cylinder: it would come after #3 Fillet, which depends on it".into())));
        assert_eq!(drop_action(&chips, Carry::Chip(4), 2, end), Some(Err("Move #4 Chamfer: it would come before its source #3 Fillet".into())));
        // The marker rolls to the chip before the gap, returns at the end, and never passes the first chip.
        assert_eq!(drop_action(&chips, Carry::Marker, 2, end), Some(Ok(Action::RollTo(Some(2)))));
        assert_eq!(drop_action(&chips, Carry::Marker, end, end), None);
        assert_eq!(drop_action(&chips, Carry::Marker, end, 2), Some(Ok(Action::RollTo(None))));
        assert!(drop_action(&chips, Carry::Marker, 0, end).is_some_and(|r| r.is_err()));
        // Gaps read off the chips' centres.
        let rects: Vec<Rect> = (0..3).map(|i| Rect::from_min_size(Pos2::new(i as f32 * 50.0, 0.0), Vec2::new(40.0, 20.0))).collect();
        assert_eq!([10.0, 30.0, 95.0, 200.0].map(|x| gap_at(&rects, Pos2::new(x, 10.0), false)), [0, 1, 2, 3]);
        assert_eq!((gap_line(&rects, 0, false), gap_line(&rects, 1, false), gap_line(&rects, 3, false)), (-2.0, 45.0, 142.0));
    }

    #[test]
    fn the_menu_says_what_each_edit_will_do_and_why_one_cannot() {
        let doc = document();
        let chips = chips(&doc, None, &[]);
        let labels = |items: &[Item]| items.iter().map(|i| (i.label.clone(), i.enabled)).collect::<Vec<_>>();
        let fillet = menu(&chips, 2, None);
        assert_eq!(
            labels(&fillet),
            [
                ("Edit feature".to_owned(), true),
                ("Rename…".to_owned(), true),
                ("Suppress".to_owned(), true),
                ("Delete".to_owned(), false),
                ("Delete with 1 dependent".to_owned(), true),
                ("Roll back to here".to_owned(), true),
                ("Move earlier".to_owned(), false),
                ("Move later".to_owned(), false),
                ("Isolate".to_owned(), true),
            ]
        );
        assert_eq!(fillet[3].hint, "1 feature depends on this: Chamfer");
        assert_eq!(fillet[4].choice, Choice::Act(Action::DeleteWithDependents(3)));
        assert_eq!(fillet[6].hint, "Move #3 Fillet: it would come before its source #2 Cylinder");
        assert_eq!(fillet[7].hint, "Move #3 Fillet: it would come after #4 Chamfer, which depends on it");
        // The band's own suppress and delete say what follows.
        let band = menu(&chips, 0, Some(2));
        assert!(band[2].label == "Suppress" && band[2].hint.contains(BAND_GONE), "{:?}", band[2]);
        assert!(band[3].label == "Delete" && band[3].enabled && band[3].hint.contains(BAND_GONE), "{:?}", band[3]);
        assert!(band.iter().any(|i| i.label == "Return to end" && i.choice == Choice::Act(Action::RollTo(None))));
        assert_eq!(band.iter().find(|i| i.label == "Move earlier").map(|i| i.enabled), Some(false));
        let box_ = menu(&chips, 4, Some(5));
        assert!(!box_.iter().any(|i| i.label == "Roll back to here"), "already the marker");
        assert_eq!(box_.iter().find(|i| i.label == "Move earlier").map(|i| i.choice.clone()), Some(Choice::Act(Action::Move { id: 5, after: Some(3) })));
        let mut off = doc.clone();
        off.features[4].enabled = false;
        assert_eq!(menu(&super::chips(&off, None, &[]), 4, None)[2].choice, Choice::Act(Action::Enable(5, true)));
    }

    /// Sixty features: the shank, then primitives each followed by a place and a union of the pair.
    fn sixty() -> (Document, Evaluated) {
        let mut doc = Document::default();
        doc.append(feature(1, "Procedural shank", Operation::Band)).unwrap();
        let mut id = 1;
        while doc.features.len() < 60 {
            id += 1;
            let base = id;
            doc.append(feature(base, &format!("Post {base}"), Operation::Cylinder { radius_mm: 0.5, height_mm: 1.0 })).unwrap();
            if doc.features.len() < 60 {
                id += 1;
                doc.append(feature(id, &format!("Place {id}"), Operation::Transform { source: base, translation: [0.0, 0.0, 1.0], rotation_deg: [0.0; 3] })).unwrap();
            }
            if doc.features.len() < 60 && base > 2 {
                id += 1;
                doc.append(feature(id, &format!("Union {id}"), Operation::Boolean { a: base - 1, b: id - 1, kind: cad::Boolean::Union })).unwrap_or(());
            }
        }
        let reports = doc
            .features
            .iter()
            .enumerate()
            .map(|(i, f)| FeatureReport {
                id: f.id,
                name: f.name.clone(),
                faces: 6,
                edges: 12,
                suppressed: false,
                notes: vec![],
                status: if i % 17 == 5 { FeatureStatus::Failed("Edge 9 is unavailable".into()) } else { FeatureStatus::Ok },
            })
            .collect();
        (doc, Evaluated { components: vec![], features: reports, band: Some(1), planes: vec![] })
    }

    /// Mean wall time of `f` over `n` runs, in microseconds.
    fn mean_us(n: u32, mut f: impl FnMut()) -> f64 {
        let t = std::time::Instant::now();
        for _ in 0..n {
            f();
        }
        t.elapsed().as_secs_f64() * 1e6 / f64::from(n)
    }

    #[test]
    fn what_a_sixty_feature_strip_costs_a_frame() {
        let (doc, e) = sixty();
        assert_eq!(doc.features.len(), 60);
        let chips_us = mean_us(400, || {
            std::hint::black_box(chips(&doc, Some(&e), &[7]));
        });
        let chips = chips(&doc, Some(&e), &[7]);
        assert!(chips.iter().any(|c| c.status.word() == "failed") && chips[1].dependents.len() > 3);
        let frame = |vertical: bool, accesskit: bool| {
            let ctx = egui::Context::default();
            if accesskit {
                ctx.enable_accesskit();
            }
            let input = || egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))), ..Default::default() };
            let run = |with: bool| {
                let mut out = ctx.run_ui(input(), |ui| {
                    if with {
                        ui.set_max_width(if vertical { 280.0 } else { 1400.0 });
                        egui::ScrollArea::both().show(ui, |ui| std::hint::black_box(show(ui, &chips, Some(40), vertical)));
                    }
                });
                out.textures_delta.clear();
            };
            for _ in 0..5 {
                run(true);
            }
            let bare = mean_us(100, || run(false));
            mean_us(100, || run(true)) - bare
        };
        let (row, list, row_ak) = (frame(false, false), frame(true, false), frame(false, true));
        let lifted = ringdesign_graph::nodes::cad::from_document(&RingDesign { cad: Some(doc.clone()), ..Default::default() }).unwrap();
        let read_us = mean_us(100, || {
            std::hint::black_box(ringdesign_graph::nodes::cad::document(&lifted).unwrap());
        });
        eprintln!("60 features: chips() {chips_us:.1} µs; show() one row {row:.0} µs, as a list {list:.0} µs, one row with AccessKit {row_ak:.0} µs; the CAD pane's document read {read_us:.0} µs");
        assert!(chips_us < 2_000.0 && row < 20_000.0 && list < 20_000.0, "{chips_us} {row} {list}");
    }

    mod widget {
        use super::*;
        use egui::{Event, PointerButton};
        use egui_kittest::{Harness, kittest::Queryable};

        struct Host {
            chips: Vec<Chip>,
            rollback: Option<Id>,
            vertical: bool,
            actions: Vec<Action>,
            /// Deletes the host's own shortcut router saw, reading the key before the strip draws.
            host_deletes: usize,
            /// Where a window covering the strip stands, when one does.
            cover: Option<Pos2>,
        }
        fn harness(vertical: bool) -> Harness<'static, Host> {
            let doc = document();
            let e = evaluate(&doc);
            let host = Host { chips: chips(&doc, Some(&e), &[]), rollback: None, vertical, actions: vec![], host_deletes: 0, cover: None };
            // 50 ms per event: two clicks land inside egui's 0.3 s double-click window.
            let mut h = Harness::builder().with_size([900.0, 400.0]).with_step_dt(0.05).build_ui_state(
                |ui, host: &mut Host| {
                    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete)) {
                        host.host_deletes += 1;
                    }
                    ui.set_max_width(if host.vertical { 260.0 } else { 900.0 });
                    let actions = show(ui, &host.chips, host.rollback, host.vertical);
                    host.actions.extend(actions);
                    if let Some(at) = host.cover {
                        egui::Area::new(egui::Id::new("cover")).fixed_pos(at).show(ui.ctx(), |ui| {
                            ui.allocate_exact_size(Vec2::new(300.0, 80.0), Sense::hover());
                        });
                    }
                },
                host,
            );
            h.run_steps(2);
            h
        }
        fn rect(h: &Harness<'_, Host>, label: &str) -> Rect {
            h.get_by_label(label).rect()
        }
        fn press(h: &mut Harness<'_, Host>, at: Pos2, pressed: bool) {
            h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            h.run_steps(1);
        }
        /// Presses at `from` and carries the pointer to `to` in steps, still held.
        fn carry(h: &mut Harness<'_, Host>, from: Pos2, to: Pos2) {
            h.event(Event::PointerMoved(from));
            h.run_steps(1);
            press(h, from, true);
            for t in [0.25, 0.5, 0.75, 1.0] {
                h.event(Event::PointerMoved(from + (to - from) * t));
                h.run_steps(1);
            }
        }
        fn take(h: &mut Harness<'_, Host>) -> Vec<Action> {
            std::mem::take(&mut h.state_mut().actions)
        }

        #[test]
        fn a_click_selects_a_double_click_edits_and_the_menu_suppresses() {
            let mut h = harness(false);
            for label in ["Procedural shank · ok", "Cylinder · ok", "Fillet · failed", "Chamfer · skipped", "Box · ok"] {
                h.get_by_label(label);
            }
            h.get_by_label("Box · ok").click();
            h.run_steps(10);
            assert_eq!(take(&mut h), [Action::Select(5)]);
            h.get_by_label("Box · ok").click();
            h.run_steps(1);
            h.get_by_label("Box · ok").click();
            h.run_steps(2);
            assert_eq!(take(&mut h), [Action::Select(5), Action::Edit(5)]);
            h.get_by_label("Fillet · failed").click_secondary();
            h.run_steps(2);
            use egui_kittest::kittest::NodeT;
            assert!(h.get_by_label("Delete").accesskit_node().is_disabled(), "a feature something reads is not deleted alone");
            h.get_by_label("Delete with 1 dependent").click();
            h.run_steps(2);
            assert_eq!(take(&mut h), [Action::DeleteWithDependents(3)]);
            h.get_by_label("Cylinder · ok").click_secondary();
            h.run_steps(2);
            h.get_by_label("Suppress").click();
            h.run_steps(2);
            assert_eq!(take(&mut h), [Action::Enable(2, false)]);
            assert!(!egui::Popup::is_any_open(&h.ctx));
        }

        #[test]
        fn rename_is_an_inline_field_enter_keeps_it_and_escape_leaves_the_old_name() {
            let mut h = harness(false);
            h.get_by_label("Box · ok").click_secondary();
            h.run_steps(2);
            h.get_by_label("Rename…").click();
            h.run_steps(3);
            assert!(h.get_by_label("Rename Box").is_focused(), "the field opens focused");
            h.get_by_label("Rename Box").type_text("Plate");
            h.run_steps(1);
            h.key_press(Key::Enter);
            h.run_steps(2);
            assert_eq!(take(&mut h), [Action::Rename(5, "Plate".into())]);
            assert!(h.query_by_label("Rename Box").is_none());
            h.get_by_label("Box · ok").click_secondary();
            h.run_steps(2);
            h.get_by_label("Rename…").click();
            h.run_steps(3);
            h.get_by_label("Rename Box").type_text("Scrap");
            h.run_steps(1);
            h.key_press(Key::Escape);
            h.run_steps(2);
            assert_eq!(take(&mut h), []);
            assert!(h.query_by_label("Rename Box").is_none(), "escape closes the field");
            h.get_by_label("Box · ok");
        }

        #[test]
        fn a_drag_reorders_and_a_refused_drop_says_why_and_applies_nothing() {
            let mut h = harness(false);
            let (bx, cyl) = (rect(&h, "Box · ok"), rect(&h, "Cylinder · ok"));
            // Onto the left of the cylinder: after the band.
            carry(&mut h, bx.center(), Pos2::new(cyl.left() + cyl.width() * 0.2, cyl.center().y));
            press(&mut h, Pos2::new(cyl.left() + cyl.width() * 0.2, cyl.center().y), false);
            h.run_steps(1);
            assert_eq!(take(&mut h), [Action::Move { id: 5, after: Some(1) }]);
            // The chamfer before its own source: refused while held, and nothing on release.
            let (chamfer, fillet) = (rect(&h, "Chamfer · skipped"), rect(&h, "Fillet · failed"));
            let over = Pos2::new(fillet.left() + fillet.width() * 0.2, fillet.center().y);
            carry(&mut h, chamfer.center(), over);
            h.run_steps(1);
            assert!(h.query_by_label_contains("it would come before its source #3 Fillet").is_some(), "the refusal is the drop's hover text");
            press(&mut h, over, false);
            h.run_steps(1);
            assert_eq!(take(&mut h), []);
            assert!(h.query_by_label_contains("it would come before its source").is_none());
            // The end marker dragged back over the fillet rolls the build back to the cylinder.
            let end = rect(&h, "End of the timeline");
            carry(&mut h, end.center(), Pos2::new(fillet.left() + 4.0, fillet.center().y));
            press(&mut h, Pos2::new(fillet.left() + 4.0, fillet.center().y), false);
            h.run_steps(1);
            assert_eq!(take(&mut h), [Action::RollTo(Some(2))]);
        }

        #[test]
        fn delete_over_the_strip_removes_the_chosen_chip_and_anywhere_else_stays_the_hosts() {
            let mut h = harness(false);
            h.state_mut().chips[4].selected = true;
            h.run_steps(2);
            let bx = rect(&h, "Box · ok");
            h.event(Event::PointerMoved(bx.center()));
            h.run_steps(2);
            h.key_press(Key::Delete);
            h.run_steps(2);
            assert_eq!((take(&mut h), h.state().host_deletes), (vec![Action::Delete(5)], 0));
            h.event(Event::PointerMoved(Pos2::new(450.0, 300.0)));
            h.run_steps(2);
            h.key_press(Key::Delete);
            h.run_steps(2);
            assert_eq!((take(&mut h), h.state().host_deletes), (vec![], 1), "off the strip the key is the host's");
            // A window standing over the strip keeps the key too.
            h.state_mut().cover = Some(bx.left_top() - Vec2::new(10.0, 10.0));
            h.run_steps(3);
            h.event(Event::PointerMoved(bx.center()));
            h.run_steps(2);
            h.key_press(Key::Delete);
            h.run_steps(2);
            assert_eq!((take(&mut h), h.state().host_deletes), (vec![], 2), "the window is on top");
        }

        #[test]
        fn a_vertical_list_carries_the_same_chips_and_reorders_by_height() {
            let mut h = harness(true);
            let (bx, band, cyl) = (rect(&h, "Box · ok"), rect(&h, "Procedural shank · ok"), rect(&h, "Cylinder · ok"));
            assert!(band.bottom() <= cyl.top() && cyl.bottom() <= bx.top(), "one chip per line, in order");
            assert!((band.width() - bx.width()).abs() < 0.5, "every line is as wide as the list");
            use egui_kittest::kittest::NodeT;
            let named = h.get_all_by_label("Cylinder").next().expect("the bare name rides along").accesskit_node().toggled();
            assert_eq!(named, Some(egui::accesskit::Toggled::False));
            let to = Pos2::new(cyl.center().x, cyl.top() + 2.0);
            carry(&mut h, bx.center(), to);
            press(&mut h, to, false);
            h.run_steps(1);
            assert_eq!(take(&mut h), [Action::Move { id: 5, after: Some(1) }]);
        }
    }
}
