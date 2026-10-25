//! Group nodes: how a collapsed group draws, what its menu offers, and opening, folding or dissolving one.

use super::*;
use ringdesign_graph::fragment::Fragment;

/// Something done to a group from its node, its menu or the toolbar.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Action {
    Enter(usize),
    Leave,
    /// Show the members in place, framed.
    Expand(usize),
    /// Fold the members into one node.
    Collapse(usize),
    Ungroup(usize),
    /// Delete the group and every node in it.
    Delete(usize),
    Rename(usize, String),
    Duplicate(usize),
}

/// The card a view-only node draws as.
pub(super) fn pseudo_card(view: &View, g: &Graph, n: &Node, store: &BTreeMap<NodeId, NodeCard>) -> NodeCard {
    let pins = |m: &BTreeMap<NodeId, Vec<view::PortPin>>| m.get(&n.id).map(|l| l.iter().map(|p| p.spec.clone()).collect()).unwrap_or_default();
    let title = n.label.clone().unwrap_or_default();
    let doc = match view.pseudo.get(&n.id) {
        Some(Pseudo::Group(i)) => view::members(g, *i).iter().map(|id| store.get(id).map(|c| c.title.clone()).unwrap_or_else(|| id.to_string())).collect::<Vec<_>>().join("\n"),
        Some(Pseudo::Inputs) => "Where the wires entering this group come from.".into(),
        Some(Pseudo::Outputs) => "Where the wires leaving this group go.".into(),
        None => String::new(),
    };
    let mut card = NodeCard {
        id: n.id.0,
        kind: n.kind.clone(),
        label: n.label.clone(),
        title: title.clone(),
        spec_label: title,
        params: serde_json::Value::Null,
        inputs: BTreeMap::new(),
        pins_in: pins(&view.ins),
        pins_out: pins(&view.outs),
        doc,
        diag: Vec::new(),
        values: BTreeMap::new(),
    };
    let diag: BTreeMap<NodeId, Vec<String>> = store.iter().map(|(id, c)| (*id, c.diag.clone())).collect();
    let values: BTreeMap<NodeId, BTreeMap<String, String>> = store.iter().map(|(id, c)| (*id, c.values.clone())).collect();
    let (d, v) = pseudo_marks(view, g, n.id, &card, &diag, &values, store);
    card.diag = d;
    card.values = v;
    card
}

/// A view-only node's diagnostics (its members', named) and value badges (what each pin stands for carries).
pub(super) fn pseudo_marks(
    view: &View,
    g: &Graph,
    vid: NodeId,
    card: &NodeCard,
    diag: &BTreeMap<NodeId, Vec<String>>,
    values: &BTreeMap<NodeId, BTreeMap<String, String>>,
    store: &BTreeMap<NodeId, NodeCard>,
) -> (Vec<String>, BTreeMap<String, String>) {
    let mut d = Vec::new();
    if let Some(Pseudo::Group(i)) = view.pseudo.get(&vid) {
        for m in view::members(g, *i) {
            let title = store.get(&m).map(|c| c.title.clone()).unwrap_or_default();
            d.extend(diag.get(&m).into_iter().flatten().map(|line| format!("{title}: {line}")));
        }
    }
    let mut v = BTreeMap::new();
    for pin in view.outs.get(&vid).into_iter().flatten() {
        if let view::Port::Out { node, out } | view::Port::Source { node, out } = &pin.port {
            if let Some(s) = values.get(node).and_then(|m| m.get(out)) {
                v.insert(pin.spec.name.clone(), s.clone());
            }
        }
    }
    let _ = card;
    (d, v)
}

/// The expanded groups to frame, members as the view shows them.
pub(super) fn frames(g: &Graph, view: &View) -> Vec<(usize, NodeGroup)> {
    if view.entered.is_some() {
        return Vec::new();
    }
    g.groups
        .iter()
        .enumerate()
        .filter(|(_, gr)| !gr.collapsed)
        .map(|(i, gr)| (i, NodeGroup { nodes: gr.nodes.iter().copied().filter(|id| !view.hidden.contains(id)).collect(), ..gr.clone() }))
        .filter(|(_, gr)| !gr.nodes.is_empty())
        .collect()
}

/// A collapsed group's frame: tinted, with a heavier rim than a node.
pub(super) fn group_frame(frame: egui::Frame, ctx: &egui::Context, active: bool) -> egui::Frame {
    let accent = egui_mobile_theme::color(ctx, egui_mobile_theme::Role::Accent);
    let rim = if active { accent } else { crate::style::AQUA.gamma_multiply(0.75) };
    frame.fill(crate::style::AQUA.gamma_multiply(0.08)).stroke(egui::Stroke::new(if active { 2.5 } else { 1.6 }, rim))
}

/// A group's title row: its name and a button that opens it.
pub(super) fn header(ui: &mut Ui, card: &NodeCard, index: usize) -> Option<Action> {
    let mut act = None;
    ui.add(egui::Label::new(RichText::new(&card.title).strong().color(crate::style::AQUA)).selectable(false).sense(egui::Sense::hover()))
        .on_hover_text("A group: several nodes folded into one. Double-click or press Open to work inside it.");
    if ui.small_button("Open").on_hover_text("Show the nodes inside this group").clicked() {
        act = Some(Action::Enter(index));
    }
    act
}

/// A group's body: how many nodes it holds and the first few of them.
pub(super) fn body(ui: &mut Ui, card: &NodeCard, index: usize) -> Option<Action> {
    let names: Vec<&str> = card.doc.lines().collect();
    ui.vertical(|ui| {
        ui.label(RichText::new(format!("{} nodes inside", names.len())).small().color(crate::style::INK_DIM));
        for name in names.iter().take(4) {
            ui.label(RichText::new(format!("• {name}")).small().color(crate::style::INK_DIM));
        }
        if names.len() > 4 {
            ui.label(RichText::new(format!("… and {} more", names.len() - 4)).small().color(crate::style::INK_DIM));
        }
        ui.label(RichText::new("Double-click to open").small().italics().color(crate::style::INK_DIM));
    });
    let _ = index;
    None
}

/// The menu a collapsed group's node opens.
pub(super) fn menu(ui: &mut Ui, card: &NodeCard, index: usize) -> Option<Action> {
    let mut act = None;
    if ui.button("Open group").clicked() {
        act = Some(Action::Enter(index));
    }
    if ui.button("Show its nodes here").on_hover_text("Unfold the group in place, framed").clicked() {
        act = Some(Action::Expand(index));
    }
    if ui.button("Duplicate group").clicked() {
        act = Some(Action::Duplicate(index));
    }
    let id = ui.scope_id().with(("rename-group", index));
    let mut name = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| card.title.clone());
    ui.horizontal(|ui| {
        let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(150.0)).on_hover_text("Rename the group");
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
            act = Some(Action::Rename(index, name.trim().to_string()));
        }
        if ui.small_button("Rename").clicked() && !name.trim().is_empty() {
            act = Some(Action::Rename(index, name.trim().to_string()));
        }
    });
    ui.data_mut(|d| d.insert_temp(id, name));
    ui.separator();
    if ui.button("Ungroup").on_hover_text("Keep the nodes, drop the group").clicked() {
        act = Some(Action::Ungroup(index));
    }
    if ui.button("Delete group and its nodes").clicked() {
        act = Some(Action::Delete(index));
    }
    if act.is_some() {
        ui.close();
    }
    act
}

impl Editor {
    /// Do what a group's node, menu or the toolbar asked; true when the graph changed.
    pub(super) fn apply_group_action(&mut self, action: Action, _ctx: &egui::Context, reg: &Registry) -> bool {
        self.sync();
        let exists = |i: usize| i < self.graph.groups.len();
        match action {
            Action::Enter(i) if exists(i) => {
                self.enter_group(i);
                false
            }
            Action::Leave => {
                self.leave_group();
                false
            }
            Action::Expand(i) if exists(i) => {
                if self.view.entered == Some(i) {
                    self.view.entered = None;
                }
                self.graph.groups[i].collapsed = false;
                self.revision += 1;
                self.rebuild();
                true
            }
            Action::Collapse(i) if exists(i) => {
                self.graph.groups[i].pos = None;
                self.graph.groups[i].pos = Some(view::group_pos(&self.graph, i));
                self.graph.groups[i].collapsed = true;
                self.revision += 1;
                self.rebuild();
                true
            }
            Action::Ungroup(i) if exists(i) => {
                self.graph.groups.remove(i);
                self.view.entered = match self.view.entered {
                    Some(e) if e == i => None,
                    Some(e) if e > i => Some(e - 1),
                    e => e,
                };
                self.revision += 1;
                self.rebuild();
                true
            }
            Action::Delete(i) if exists(i) => {
                let members = view::members(&self.graph, i);
                let mut g = self.graph.clone();
                for id in &members {
                    let _ = g.remove(*id);
                }
                if g.groups.get(i).is_some_and(|gr| gr.name == self.graph.groups[i].name) {
                    g.groups.remove(i);
                }
                if self.view.entered == Some(i) {
                    self.view.entered = None;
                }
                if self.selected.is_some_and(|s| members.contains(&s)) {
                    self.selected = None;
                }
                self.set_graph(g, reg);
                true
            }
            Action::Rename(i, name) if exists(i) => {
                self.graph.groups[i].name = name;
                self.revision += 1;
                self.rebuild();
                true
            }
            Action::Duplicate(i) if exists(i) => {
                let members = view::members(&self.graph, i);
                let fragment = Fragment::capture(&self.graph, &members);
                let at = view::group_pos(&self.graph, i);
                let origin = fragment.nodes.iter().fold([f32::INFINITY; 2], |a, n| [a[0].min(n.pos[0]), a[1].min(n.pos[1])]);
                let mut g = self.graph.clone();
                match fragment.paste(&mut g, [origin[0] + 60.0, origin[1] + 60.0], reg) {
                    Ok(_) => {
                        if let Some(copy) = g.groups.last_mut() {
                            copy.name = format!("{} copy", self.graph.groups[i].name);
                            copy.pos = Some([at[0] + 60.0, at[1] + 60.0]);
                        }
                        self.set_graph(g, reg);
                        true
                    }
                    Err(e) => {
                        self.command_error = Some(e.to_string());
                        false
                    }
                }
            }
            _ => false,
        }
    }

    /// Fold these nodes into one group node; true when the graph changed.
    pub fn group_nodes(&mut self, ids: &[NodeId], name: &str) -> bool {
        self.group_selection(ids, name, true)
    }

    /// The group a node belongs to, by index, the first naming it.
    pub fn group_of(&self, id: NodeId) -> Option<usize> {
        self.graph.groups.iter().position(|g| g.nodes.contains(&id))
    }

    /// Open the group at this index, or leave the one in hand with `None`.
    pub fn open_group(&mut self, index: Option<usize>) {
        match index {
            Some(i) => self.enter_group(i),
            None => self.leave_group(),
        }
    }

    /// Group the selection, folded or framed.
    pub(super) fn group_selection(&mut self, ids: &[NodeId], name: &str, collapsed: bool) -> bool {
        self.sync();
        let ids: Vec<NodeId> = ids.iter().copied().filter(|id| self.graph.contains(*id)).collect();
        if ids.is_empty() {
            return false;
        }
        let taken: BTreeSet<NodeId> = ids.iter().copied().collect();
        for gr in &mut self.graph.groups {
            gr.nodes.retain(|id| !taken.contains(id));
        }
        let entered_name = self.entered_group().map(|g| g.name.clone());
        self.graph.groups.retain(|gr| !gr.nodes.is_empty());
        self.graph.groups.push(NodeGroup { collapsed, ..NodeGroup::new(name, ids) });
        let index = self.graph.groups.len() - 1;
        if collapsed {
            self.graph.groups[index].pos = Some(view::group_pos(&self.graph, index));
        }
        if self.view.entered.is_some() {
            self.view.entered = entered_name.and_then(|n| self.graph.groups.iter().position(|g| g.name == n));
        }
        self.revision += 1;
        self.rebuild();
        true
    }

    /// Fold every layer chain, stamp list and settings block not grouped yet.
    pub(super) fn auto_group(&mut self) -> bool {
        self.sync();
        let added = self.graph.auto_group();
        if added > 0 {
            self.revision += 1;
            self.view.entered = None;
            self.rebuild();
            self.arrange_pending = true;
        }
        added > 0
    }

    /// Fold or unfold every group.
    pub(super) fn fold_all(&mut self, collapsed: bool) -> bool {
        self.sync();
        let mut changed = false;
        for gr in &mut self.graph.groups {
            if gr.collapsed != collapsed {
                gr.collapsed = collapsed;
                changed = true;
            }
        }
        if changed {
            self.view.entered = None;
            self.revision += 1;
            self.rebuild();
            self.arrange_pending = true;
        }
        changed
    }
}
