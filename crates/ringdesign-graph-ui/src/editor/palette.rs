//! The add-node popup: a search field that has the keyboard, a list the arrow keys walk, and help on the node in hand.

use super::*;
use ringdesign_graph::graph::Mode;
use ringdesign_graph::registry::Category;

fn favorites_id() -> egui::Id {
    egui::Id::unique("ringdesigner-favorite-nodes")
}

fn recent_id() -> egui::Id {
    egui::Id::unique("ringdesigner-recent-nodes")
}

/// The nodes most graphs are made of, offered before anything is typed.
const COMMON: &[&str] = &[
    "layer.tiling", "layer.seat", "layer.border", "layer.decals", "layer.milgrain", "layer.flutes", "entry", "window", "stack", "gem", "alpha.svg", "alpha.png", "alpha.proc",
    "band.profile", "shank", "design.assemble", "list.merge", "number", "math.mul",
];
const RECENT_MAX: usize = 8;

/// Which list the popup shows when nothing is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Section {
    #[default]
    Common,
    Recent,
    Favorites,
    Category(Category),
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::Common => "Common",
            Section::Recent => "Recent",
            Section::Favorites => "Favorites",
            Section::Category(c) => c.label(),
        }
    }
}

/// One popup's state, kept while it is open.
#[derive(Clone, Debug, Default)]
pub(super) struct State {
    pub query: String,
    /// The row the arrow keys are on.
    pub hi: usize,
    pub section: Section,
    /// Give the search field the keyboard, the caret at the end of the text, on the next draw.
    pub focus: bool,
    /// Where the popup stands on the screen, and where the node it adds goes in the graph.
    pub screen: egui::Pos2,
    pub graph: egui::Pos2,
    /// The frame the popup was last drawn in, to tell a fresh opening from a kept one.
    pub drawn: u64,
}

impl State {
    pub fn opened(screen: egui::Pos2, graph: egui::Pos2, query: String, focus: bool) -> Self {
        Self { query, screen, graph, focus, section: Section::Common, ..Self::default() }
    }
}

/// What the popup did this frame.
#[derive(Clone, Debug, Default)]
pub(super) struct Outcome {
    pub chosen: Option<String>,
    pub close: bool,
}

/// How well `spec` answers every word typed: higher is better, `None` when a word misses.
fn score(spec: &NodeSpec, words: &[String]) -> Option<i32> {
    let label = spec.label.to_lowercase();
    let key = spec.key.to_lowercase();
    let cat = spec.category.label().to_lowercase();
    let about = ringdesign_graph::guide::node(&spec.key).map(|g| g.line()).unwrap_or_else(|| spec.doc.clone()).to_lowercase();
    let mut total = 0;
    for w in words {
        let s = if label == *w || key == *w {
            100
        } else if label.starts_with(w.as_str()) {
            80
        } else if label.split(|c: char| !c.is_alphanumeric()).any(|part| part.starts_with(w.as_str())) {
            60
        } else if key.split(['.', '_']).any(|part| part.starts_with(w.as_str())) {
            50
        } else if label.contains(w.as_str()) || key.contains(w.as_str()) {
            35
        } else if cat.starts_with(w.as_str()) {
            20
        } else if about.contains(w.as_str()) {
            8
        } else {
            return None;
        };
        total += s;
    }
    Some(total)
}

/// The nodes matching `query`, best first.
fn search<'a>(specs: &[&'a NodeSpec], query: &str) -> Vec<&'a NodeSpec> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(i32, &NodeSpec)> = specs.iter().filter_map(|s| score(s, &words).map(|v| (v, *s))).collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.label.len().cmp(&b.1.label.len())).then(a.1.label.cmp(&b.1.label)));
    hits.into_iter().map(|(_, s)| s).collect()
}

fn remembered(ui: &Ui, id: egui::Id) -> Vec<String> {
    ui.data_mut(|d| d.get_persisted::<Vec<String>>(id)).unwrap_or_default()
}

fn favorites(ui: &Ui) -> BTreeSet<String> {
    ui.data_mut(|d| d.get_persisted::<BTreeSet<String>>(favorites_id())).unwrap_or_default()
}

/// Put `key` first in the recently added list.
pub(super) fn remember(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|d| {
        let mut list = d.get_persisted::<Vec<String>>(recent_id()).unwrap_or_default();
        list.retain(|k| k != key);
        list.insert(0, key.to_string());
        list.truncate(RECENT_MAX);
        d.insert_persisted(recent_id(), list);
    });
}

/// The popup's content. `fits` keeps only nodes a dragged wire can join and says through which pin.
pub(super) fn body(ui: &mut Ui, reg: &Registry, mode: Mode, st: &mut State, fits: Option<&dyn Fn(&NodeSpec) -> Option<String>>, narrow: bool) -> Outcome {
    let mut out = Outcome::default();
    let (down, up, enter, esc) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if esc {
        out.close = true;
        return out;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(if fits.is_some() { "Add a connected node" } else { "Add a node" }).strong());
        ui.weak("Up/Down to choose, Enter to add, Esc to close");
    });
    let edit_id = ui.scope_id().with("node-search");
    let field = ui.add(egui::TextEdit::singleline(&mut st.query).id(edit_id).hint_text("Type a name, a type or what it does").desired_width(if narrow { ui.available_width() } else { 520.0 }));
    if st.focus {
        field.request_focus();
        if let Some(mut s) = egui::TextEdit::load_state(ui.ctx(), edit_id) {
            let end = egui::text::CCursor::new(st.query.chars().count());
            s.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
            s.store(ui.ctx(), edit_id);
        }
        st.focus = false;
    }
    if field.changed() {
        st.hi = 0;
    }
    let all: Vec<&NodeSpec> = reg.list(mode).into_iter().filter(|s| fits.is_none_or(|f| f(s).is_some())).collect();
    let needle = st.query.trim().to_string();
    let favs = favorites(ui);
    let recent = remembered(ui, recent_id());
    let in_section = |section: Section| -> Vec<&NodeSpec> {
        match section {
            Section::Common => COMMON.iter().filter_map(|k| all.iter().find(|s| s.key == *k).copied()).collect(),
            Section::Recent => recent.iter().filter_map(|k| all.iter().find(|s| s.key == *k).copied()).collect(),
            Section::Favorites => all.iter().filter(|s| favs.contains(&s.key)).copied().collect(),
            Section::Category(c) => all.iter().filter(|s| s.category == c).copied().collect(),
        }
    };
    let mut sections: Vec<Section> = Vec::new();
    if fits.is_none() {
        sections.push(Section::Common);
    }
    if !recent.is_empty() {
        sections.push(Section::Recent);
    }
    if !favs.is_empty() {
        sections.push(Section::Favorites);
    }
    sections.extend(Category::ALL.iter().copied().map(Section::Category));
    sections.retain(|s| !in_section(*s).is_empty());
    if !sections.contains(&st.section) {
        st.section = sections.first().copied().unwrap_or_default();
    }
    let list: Vec<&NodeSpec> = if needle.is_empty() { in_section(st.section) } else { search(&all, &needle) };
    if !list.is_empty() {
        if down {
            st.hi = (st.hi + 1).min(list.len() - 1);
        }
        if up {
            st.hi = st.hi.saturating_sub(1);
        }
        st.hi = st.hi.min(list.len() - 1);
        if enter {
            out.chosen = Some(list[st.hi].key.clone());
        }
    }
    let moved = down || up;
    let pick = |ui: &mut Ui, st: &mut State| {
        for s in &sections {
            if ui.selectable_label(st.section == *s, s.label()).clicked() {
                st.section = *s;
                st.hi = 0;
            }
        }
    };
    let (hovered, clicked) = if narrow {
        if needle.is_empty() {
            ui.horizontal_wrapped(|ui| pick(ui, st));
        }
        let hit = rows(ui, &list, st.hi, moved, &needle, narrow);
        if let Some(spec) = list.get(hit.0.unwrap_or(st.hi)) {
            ui.separator();
            summary(ui, spec, fits);
        }
        hit
    } else {
        ui.horizontal_top(|ui| {
            if needle.is_empty() {
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    pick(ui, st);
                });
                ui.separator();
            }
            let hit = ui.vertical(|ui| rows(ui, &list, st.hi, moved, &needle, narrow)).inner;
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(280.0);
                if let Some(spec) = list.get(hit.0.unwrap_or(st.hi)) {
                    details(ui, spec, fits, &favs);
                }
            });
            hit
        })
        .inner
    };
    let _ = hovered;
    if clicked.is_some() {
        out.chosen = clicked;
    }
    out
}

/// The result rows: the highlighted one ringed, kept in view as the keys move it. Returns the row under the pointer and any row clicked.
fn rows(ui: &mut Ui, list: &[&NodeSpec], hi: usize, moved: bool, needle: &str, narrow: bool) -> (Option<usize>, Option<String>) {
    let mut hovered = None;
    let mut clicked = None;
    egui::ScrollArea::vertical().id_salt("node-results").max_height(if narrow { 220.0 } else { 340.0 }).auto_shrink([false, true]).show(ui, |ui| {
        ui.set_min_width(if narrow { ui.available_width() } else { 250.0 });
        if list.is_empty() {
            ui.weak(if needle.is_empty() { "Nothing here in this graph's mode." } else { "No node matches. Try fewer letters, or a type such as window or number." });
        }
        for (i, spec) in list.iter().enumerate() {
            let on = i == hi;
            let r = ui
                .horizontal(|ui| {
                    let row = crate::marks::button(ui, crate::marks::of(spec), &spec.label);
                    if on {
                        ui.painter().rect_stroke(row.rect.expand(1.0), 4.0, egui::Stroke::new(1.5, crate::style::AQUA), egui::StrokeKind::Outside);
                    }
                    if !needle.is_empty() {
                        ui.label(RichText::new(spec.category.label()).small().color(crate::style::INK_DIM));
                    }
                    row
                })
                .inner;
            if on && moved {
                r.scroll_to_me(None);
            }
            if r.hovered() {
                hovered = Some(i);
            }
            if r.clicked() {
                clicked = Some(spec.key.clone());
            }
        }
    });
    (hovered, clicked)
}

/// A node's name and what it does, in one paragraph.
fn summary(ui: &mut Ui, spec: &NodeSpec, fits: Option<&dyn Fn(&NodeSpec) -> Option<String>>) {
    ui.label(RichText::new(&spec.label).strong());
    let about = ringdesign_graph::guide::node(&spec.key).map(|g| g.line()).unwrap_or_else(|| spec.doc.clone());
    ui.label(RichText::new(about).small());
    if let Some(pin) = fits.and_then(|f| f(spec)) {
        ui.label(RichText::new(format!("Connects through {pin}")).small().color(crate::style::AQUA));
    }
}

/// The highlighted node in full: what it does, what it takes, what it gives.
fn details(ui: &mut Ui, spec: &NodeSpec, fits: Option<&dyn Fn(&NodeSpec) -> Option<String>>, favs: &BTreeSet<String>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&spec.label).heading().strong());
        let pinned = favs.contains(&spec.key);
        if ui.small_button(if pinned { "Unpin" } else { "Pin" }).on_hover_text("Keep this node under Favorites").clicked() {
            let mut favs = favs.clone();
            if pinned {
                favs.remove(&spec.key);
            } else {
                favs.insert(spec.key.clone());
            }
            ui.data_mut(|d| d.insert_persisted(favorites_id(), favs));
        }
    });
    ui.label(RichText::new(format!("{}  ·  {}", spec.category.label(), spec.key)).small().monospace().color(crate::style::INK_DIM));
    let about = ringdesign_graph::guide::node(&spec.key).map(|g| g.line()).unwrap_or_else(|| spec.doc.clone());
    ui.label(about);
    if let Some(pin) = fits.and_then(|f| f(spec)) {
        ui.label(RichText::new(format!("Connects through {pin}")).color(crate::style::AQUA));
    }
    let pins = |ui: &mut Ui, title: &str, list: &[PinSpec]| {
        if list.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label(RichText::new(title).small().strong());
        for p in list.iter().take(10) {
            ui.horizontal(|ui| {
                help::kind_dot(ui, p.kind);
                ui.label(RichText::new(&p.name).small());
                ui.label(RichText::new(help::kind_words(p.kind, p.access)).small().color(crate::style::INK_DIM));
            });
        }
        if list.len() > 10 {
            ui.label(RichText::new(format!("… and {} more", list.len() - 10)).small().color(crate::style::INK_DIM));
        }
    };
    pins(ui, "Takes", &spec.inputs);
    pins(ui, "Gives", &spec.outputs);
}

/// Which pin of a new `spec` node a dropped wire would join, by index and name.
fn joins(spec: &NodeSpec, snarl: &Snarl<NodeCard>, pins: &AnyPins<'_>) -> Option<(usize, String)> {
    match pins {
        AnyPins::Out(pins) => {
            let from = pins.first()?;
            let kind = snarl.get_node(from.node)?.pins_out.get(from.output)?.kind;
            spec.inputs.iter().enumerate().find(|(_, p)| p.kind != ValueKind::Any && fits(kind, p)).or_else(|| spec.inputs.iter().enumerate().find(|(_, p)| fits(kind, p))).map(|(i, p)| (i, p.name.clone()))
        }
        AnyPins::In(pins) => {
            let to = pins.first()?;
            let pin = snarl.get_node(to.node)?.pins_in.get(to.input)?;
            spec.outputs.iter().enumerate().find(|(_, p)| fits(p.kind, pin)).map(|(i, p)| (i, p.name.clone()))
        }
    }
}

impl Viewer<'_> {
    /// The menu a wire dropped on empty canvas opens: nodes that can take it, joined when chosen.
    pub(super) fn wire_menu(&mut self, pos: egui::Pos2, ui: &mut Ui, snarl: &mut Snarl<NodeCard>, pins: &AnyPins<'_>) {
        let frame = ui.ctx().cumulative_frame_nr();
        if self.palette.drawn + 1 < frame {
            *self.palette = State::opened(egui::Pos2::ZERO, pos, String::new(), !ui.input(|i| i.has_touch_screen()));
        }
        self.palette.drawn = frame;
        let narrow = ui.ctx().content_rect().width() < 640.0;
        let snarl_ref: &Snarl<NodeCard> = snarl;
        let fits_fn = |spec: &NodeSpec| joins(spec, snarl_ref, pins).map(|(_, name)| name);
        let out = body(ui, self.reg, self.mode, self.palette, Some(&fits_fn), narrow);
        if out.close {
            ui.close();
            return;
        }
        let Some(key) = out.chosen else { return };
        let Some(spec) = self.reg.get(&key) else { return };
        let link = joins(spec, snarl, pins).map(|(i, _)| i);
        let node = snarl.insert_node(pos, NodeCard::new_of(spec, self.reg));
        match (pins, link) {
            (AnyPins::Out(pins), Some(input)) => {
                let from = OutPin { id: pins[0], remotes: vec![] };
                let to = InPin { id: InPinId { node, input }, remotes: vec![] };
                self.connect(&from, &to, snarl);
            }
            (AnyPins::In(pins), Some(output)) => {
                let from = OutPin { id: OutPinId { node, output }, remotes: vec![] };
                let to = InPin { id: pins[0], remotes: vec![] };
                self.connect(&from, &to, snarl);
            }
            _ => {}
        }
        remember(ui.ctx(), &key);
        *self.palette = State::default();
        ui.close();
    }
}

impl Editor {
    /// Open the add-node popup at a screen point, the search already holding `query`.
    pub(super) fn open_palette(&mut self, at: egui::Pos2, query: String, focus: bool) {
        let graph = self.transform.map_or(at, |t| t.inverse() * at);
        self.palette = Some(State::opened(at, graph, query, focus));
    }

    /// Open the popup on a right-click, a long touch or typing over the canvas, draw it, and add what it chose.
    pub(super) fn palette_ui(&mut self, ctx: &egui::Context, reg: &Registry, viewport: egui::Rect, background: &egui::Response) -> bool {
        if !self.editable {
            self.palette = None;
            return false;
        }
        let touch = ctx.input(|i| i.has_touch_screen());
        if background.secondary_clicked() || background.long_touched() {
            let at = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(viewport.center());
            self.open_palette(at, String::new(), !touch);
        }
        let hover = ctx.input(|i| i.pointer.hover_pos()).filter(|p| viewport.contains(*p) && ctx.layer_id_at(*p).is_none_or(|l| l.order == egui::Order::Background));
        if self.palette.is_none() && !ctx.egui_wants_keyboard_input() && !egui::Popup::is_any_open(ctx) {
            if let Some(p) = hover {
                let typed = ctx.input_mut(|i| {
                    let text: String = i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.as_str()) } else { None }).collect();
                    if text.chars().next().is_some_and(|c| c.is_alphanumeric()) && !i.modifiers.command {
                        i.events.retain(|e| !matches!(e, egui::Event::Text(_)));
                        text
                    } else {
                        String::new()
                    }
                });
                if !typed.is_empty() {
                    self.open_palette(p, typed, true);
                }
            }
        }
        let Some(st) = self.palette.as_mut() else { return false };
        let narrow = viewport.width() < 640.0;
        let mut out = Outcome::default();
        let id = self.snarl_id.unwrap_or(egui::Id::NULL).with("add-node");
        let area = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(st.screen).constrain(true).show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_max_width(if narrow { (viewport.width() - 24.0).max(200.0) } else { 720.0 });
                out = body(ui, reg, self.graph.mode, st, None, narrow);
            });
        });
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|p| !area.response.rect.contains(p)));
        if let Some(key) = out.chosen {
            let at = st.graph;
            if let Some(spec) = reg.get(&key) {
                self.snarl.insert_node(at, NodeCard::new_of(spec, reg));
                remember(ctx, &key);
            }
            self.palette = None;
        } else if out.close || outside {
            self.palette = None;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn a_search_ranks_names_before_descriptions() {
        let reg = Registry::builtin();
        let all = reg.list(Mode::SandRing);
        let hits = search(&all, "window");
        assert_eq!(hits.first().map(|s| s.key.as_str()), Some("window"));
        let hits = search(&all, "seat");
        assert!(hits.first().is_some_and(|s| s.key.starts_with("layer.seat")), "{:?}", hits.iter().map(|s| &s.key).collect::<Vec<_>>());
        assert!(search(&all, "zzqx").is_empty());
        // Every word must land somewhere.
        let hits = search(&all, "tiling fit");
        assert_eq!(hits.first().map(|s| s.key.as_str()), Some("layer.tiling.fit"));
    }

    #[test]
    fn typing_over_the_canvas_opens_the_popup_and_enter_adds_the_first_match() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::simple();
        let before = g.nodes.len();
        let mut h = Harness::builder().with_size([1100., 700.]).build_ui_state(
            |ui, ed: &mut Editor| {
                ed.show(&reg, ui, "palette-test");
            },
            Editor::new(g, &reg),
        );
        h.run();
        h.hover_at(egui::pos2(600., 450.));
        h.run();
        h.input_mut().events.push(egui::Event::Text("windo".into()));
        h.run();
        assert!(h.state().palette.as_ref().is_some_and(|p| p.query == "windo"), "the popup opened with what was typed");
        h.input_mut().events.push(egui::Event::Text("w".into()));
        h.run();
        assert_eq!(h.state().palette.as_ref().map(|p| p.query.as_str()), Some("window"), "the search field has the keyboard");
        h.key_press(egui::Key::Enter);
        h.run();
        assert!(h.state().palette.is_none(), "choosing closes the popup and clears the search");
        assert_eq!(h.state().graph().nodes.len(), before + 1);
        assert!(h.state().graph().nodes.iter().any(|n| n.kind == "window"));
    }

    #[test]
    fn arrow_keys_walk_the_list_and_escape_closes() {
        let reg = Registry::builtin();
        let mut h = Harness::builder().with_size([1100., 700.]).build_ui_state(
            |ui, ed: &mut Editor| {
                ed.show(&reg, ui, "palette-keys");
            },
            Editor::new(ringdesign_graph::templates::simple(), &reg),
        );
        h.run();
        h.hover_at(egui::pos2(600., 450.));
        h.run();
        h.input_mut().events.push(egui::Event::Text("math".into()));
        h.run();
        h.key_press(egui::Key::ArrowDown);
        h.run();
        h.key_press(egui::Key::ArrowDown);
        h.run();
        assert_eq!(h.state().palette.as_ref().map(|p| p.hi), Some(2));
        h.key_press(egui::Key::ArrowUp);
        h.run();
        assert_eq!(h.state().palette.as_ref().map(|p| p.hi), Some(1));
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(h.state().palette.is_none());
    }
}
