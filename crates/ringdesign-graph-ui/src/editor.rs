//! The editor widget: build a snarl from the graph, show it, extract it back.
//!
//! [`Editor`] owns a [`Graph`] and the `Snarl` that views it. The snarl is
//! rebuilt whenever the graph is replaced from outside ([`Editor::set_graph`])
//! and extracted after every frame it is shown; when what comes back differs
//! from what went in, the graph is updated and the revision moves. Nodes
//! added in the view carry no id until extraction hands them a fresh one.

use std::collections::BTreeMap;

use egui::{Color32, RichText, Ui};
use egui_snarl::ui::{PinInfo, SnarlStyle, SnarlViewer};
use egui_snarl::{InPin, InPinId, NodeId as SnarlId, OutPin, OutPinId, Snarl};
use ringdesign_graph::graph::{Access, Graph, GraphError, Node, NodeId, Wire};
use ringdesign_graph::registry::{Category, NodeSpec, PinSpec, Registry};
use ringdesign_graph::value::{Literal, ValueKind};

use crate::widgets::{kind_color, pin_widget};

/// A node as the view holds it: the graph's data plus the pins resolved
/// from the registry, so drawing needs no registry at all.
#[derive(Clone, Debug)]
pub struct NodeCard {
    /// The graph id; 0 until extraction assigns one.
    pub id: u64,
    pub kind: String,
    pub label: Option<String>,
    pub title: String,
    pub params: serde_json::Value,
    pub inputs: BTreeMap<String, Literal>,
    pub pins_in: Vec<PinSpec>,
    pub pins_out: Vec<PinSpec>,
    pub doc: String,
    /// Errors and per-item failures attributed to this node.
    pub diag: Vec<String>,
    /// Output value summaries from the last evaluation, by pin.
    pub values: BTreeMap<String, String>,
}

impl NodeCard {
    pub fn from_node(node: &Node, reg: &Registry) -> Self {
        let spec = reg.get(&node.kind);
        let (pins_in, pins_out) = reg.node_pins(node).unwrap_or_default();
        Self {
            id: node.id.0,
            kind: node.kind.clone(),
            label: node.label.clone(),
            title: node.label.clone().unwrap_or_else(|| spec.map(|s| s.label.clone()).unwrap_or_else(|| node.kind.clone())),
            params: node.params.clone(),
            inputs: node.inputs.clone(),
            pins_in,
            pins_out,
            doc: spec.map(|s| s.doc.clone()).unwrap_or_default(),
            diag: Vec::new(),
            values: BTreeMap::new(),
        }
    }

    /// A fresh card for a kind picked from the palette.
    pub fn new_of(spec: &NodeSpec, reg: &Registry) -> Self {
        let node = Node { id: NodeId(0), kind: spec.key.clone(), params: serde_json::Value::Null, inputs: BTreeMap::new(), pos: [0.0; 2], label: None };
        Self::from_node(&node, reg)
    }

    pub fn graph_id(&self) -> Option<NodeId> {
        (self.id != 0).then_some(NodeId(self.id))
    }

    fn to_node(&self, id: NodeId, pos: egui::Pos2) -> Node {
        Node { id, kind: self.kind.clone(), params: self.params.clone(), inputs: self.inputs.clone(), pos: [pos.x.round(), pos.y.round()], label: self.label.clone() }
    }
}

/// Graph ids to snarl ids and back, for one snarl.
#[derive(Clone, Debug, Default)]
pub struct IdMap {
    pub to_snarl: BTreeMap<NodeId, SnarlId>,
    pub to_graph: BTreeMap<SnarlId, NodeId>,
}

/// A snarl viewing `g`, pins resolved through `reg`.
pub fn build_snarl(g: &Graph, reg: &Registry) -> (Snarl<NodeCard>, IdMap) {
    let mut snarl = Snarl::new();
    let mut ids = IdMap::default();
    for n in &g.nodes {
        let sid = snarl.insert_node(egui::pos2(n.pos[0], n.pos[1]), NodeCard::from_node(n, reg));
        ids.to_snarl.insert(n.id, sid);
        ids.to_graph.insert(sid, n.id);
    }
    for w in &g.wires {
        let (Some(&from), Some(&to)) = (ids.to_snarl.get(&w.from), ids.to_snarl.get(&w.to)) else { continue };
        let out = snarl.get_node(from).and_then(|c| c.pins_out.iter().position(|p| p.name == w.out));
        let inp = snarl.get_node(to).and_then(|c| c.pins_in.iter().position(|p| p.name == w.input));
        if let (Some(output), Some(input)) = (out, inp) {
            snarl.connect(OutPinId { node: from, output }, InPinId { node: to, input });
        }
    }
    (snarl, ids)
}

/// The graph the snarl shows, with `template` supplying everything that is
/// not a node or a wire (name, mode, exposures, the id counter). Nodes
/// without an id get fresh ones; exposures of nodes no longer present drop.
pub fn extract_graph(snarl: &Snarl<NodeCard>, template: &Graph) -> Graph {
    let mut g = Graph::new(&template.name, template.mode);
    g.next_id = template.next_id.max(template.nodes.iter().map(|n| n.id.0 + 1).max().unwrap_or(1));
    let mut by_snarl: BTreeMap<SnarlId, NodeId> = BTreeMap::new();
    let mut nodes: Vec<(NodeId, Node)> = Vec::new();
    let mut fresh: Vec<(SnarlId, egui::Pos2, NodeCard)> = Vec::new();
    for (sid, pos, card) in snarl.nodes_pos_ids() {
        match card.graph_id() {
            Some(id) => {
                by_snarl.insert(sid, id);
                nodes.push((id, card.to_node(id, pos)));
            }
            None => fresh.push((sid, pos, card.clone())),
        }
    }
    nodes.sort_by_key(|(id, _)| *id);
    fresh.sort_by_key(|(sid, _, _)| sid.0);
    for (sid, pos, card) in fresh {
        let id = NodeId(g.next_id);
        g.next_id += 1;
        by_snarl.insert(sid, id);
        nodes.push((id, card.to_node(id, pos)));
    }
    g.nodes = nodes.into_iter().map(|(_, n)| n).collect();
    let mut present: Vec<Wire> = snarl
        .wires()
        .filter_map(|(out, inp)| {
            let from = *by_snarl.get(&out.node)?;
            let to = *by_snarl.get(&inp.node)?;
            let out_name = snarl.get_node(out.node)?.pins_out.get(out.output)?.name.clone();
            let in_name = snarl.get_node(inp.node)?.pins_in.get(inp.input)?.name.clone();
            Some(Wire { from, out: out_name, to, input: in_name })
        })
        .collect();
    // The template's order for wires that survive, new ones after, sorted:
    // a view must not invent a revision by reordering what it was given.
    let mut wires: Vec<Wire> = Vec::with_capacity(present.len());
    for w in &template.wires {
        if let Some(i) = present.iter().position(|p| p == w) {
            wires.push(present.remove(i));
        }
    }
    present.sort_by(|a, b| (a.to, &a.input, a.from, &a.out).cmp(&(b.to, &b.input, b.from, &b.out)));
    wires.extend(present);
    g.wires = wires;
    let live: std::collections::BTreeSet<NodeId> = g.nodes.iter().map(|n| n.id).collect();
    g.exposed = template.exposed.iter().filter(|e| live.contains(&e.node)).cloned().collect();
    g.outputs = template.outputs.iter().filter(|e| live.contains(&e.node)).cloned().collect();
    g
}

/// What one frame of the editor reported.
#[derive(Clone, Debug, Default)]
pub struct EditorResponse {
    /// The graph changed this frame.
    pub changed: bool,
    /// The node under the last click, if any.
    pub selected: Option<NodeId>,
    /// A wire the editor refused, in words.
    pub refused: Option<String>,
}

/// The editor: a graph, its snarl, and the bookkeeping between them.
pub struct Editor {
    graph: Graph,
    snarl: Snarl<NodeCard>,
    ids: IdMap,
    /// Moves whenever the graph changes through the editor or set_graph.
    pub revision: u64,
    pub selected: Option<NodeId>,
    pub editable: bool,
    style: SnarlStyle,
    /// A node to centre the view on at the next frame.
    pending_focus: Option<NodeId>,
}

impl Editor {
    pub fn new(graph: Graph, reg: &Registry) -> Self {
        let (snarl, ids) = build_snarl(&graph, reg);
        Self { graph, snarl, ids, revision: 0, selected: None, editable: true, style: SnarlStyle::new(), pending_focus: None }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn snarl(&self) -> &Snarl<NodeCard> {
        &self.snarl
    }

    /// Replace the graph from outside (a file, history, MCP) and rebuild the view.
    pub fn set_graph(&mut self, graph: Graph, reg: &Registry) {
        let (snarl, ids) = build_snarl(&graph, reg);
        self.graph = graph;
        self.snarl = snarl;
        self.ids = ids;
        self.revision += 1;
        if let Some(sel) = self.selected {
            if !self.graph.contains(sel) {
                self.selected = None;
            }
        }
    }

    /// Re-resolve pins (the registry changed) without losing the view.
    pub fn refresh(&mut self, reg: &Registry) {
        let graph = self.graph.clone();
        self.set_graph(graph, reg);
    }

    /// Attach diagnostics to the cards they name.
    pub fn set_diagnostics(&mut self, errors: &[GraphError], per_node: &BTreeMap<NodeId, Vec<String>>) {
        for (sid, card) in self.snarl.nodes_ids_mut() {
            card.diag.clear();
            if let Some(gid) = self.ids.to_graph.get(&sid) {
                card.diag.extend(errors.iter().filter(|e| e.node == Some(*gid)).map(|e| e.message.clone()));
                if let Some(lines) = per_node.get(gid) {
                    card.diag.extend(lines.iter().cloned());
                }
            }
        }
    }

    /// Attach output value summaries for badges.
    pub fn set_values(&mut self, values: &BTreeMap<NodeId, BTreeMap<String, String>>) {
        for (sid, card) in self.snarl.nodes_ids_mut() {
            card.values.clear();
            if let Some(v) = self.ids.to_graph.get(&sid).and_then(|gid| values.get(gid)) {
                card.values = v.clone();
            }
        }
    }

    /// Centre the view on a node at the next frame, and select it.
    pub fn focus(&mut self, id: NodeId) {
        if self.graph.contains(id) {
            self.pending_focus = Some(id);
            self.selected = Some(id);
        }
    }

    /// Draw the editor and extract any change.
    pub fn show(&mut self, reg: &Registry, ui: &mut Ui, id_salt: &str) -> EditorResponse {
        let focus = self.pending_focus.take().and_then(|id| self.ids.to_snarl.get(&id).copied());
        let mut viewer = Viewer {
            reg,
            editable: self.editable,
            clicked: None,
            refused: None,
            search: String::new(),
            ids: &self.ids,
            focus,
            viewport_center: ui.available_rect_before_wrap().center(),
        };
        self.snarl.show(&mut viewer, &self.style, id_salt, ui);
        let clicked = viewer.clicked;
        let refused = viewer.refused.take();
        let mut resp = EditorResponse { refused, ..Default::default() };
        if let Some(sid) = clicked {
            self.selected = self.ids.to_graph.get(&sid).copied();
        }
        resp.selected = self.selected;
        let extracted = extract_graph(&self.snarl, &self.graph);
        if extracted != self.graph {
            self.graph = extracted;
            self.revision += 1;
            resp.changed = true;
            // Fresh nodes got ids; re-key the map without rebuilding the view.
            let (_, ids) = build_snarl(&self.graph, reg);
            let _ = ids;
            self.ids = IdMap::default();
            for (sid, card) in self.snarl.nodes_ids_mut() {
                if let Some(gid) = card.graph_id() {
                    self.ids.to_snarl.insert(gid, sid);
                    self.ids.to_graph.insert(sid, gid);
                }
            }
            self.assign_fresh_ids();
        }
        resp
    }

    /// Cards that were inserted this frame carry id 0 in the snarl; give
    /// them the ids extraction chose, so the next frame agrees.
    fn assign_fresh_ids(&mut self) {
        let mut fresh: Vec<SnarlId> = self.snarl.nodes_ids_mut().filter(|(_, c)| c.id == 0).map(|(sid, _)| sid).collect();
        fresh.sort_by_key(|s| s.0);
        let unmapped: Vec<NodeId> = self.graph.nodes.iter().map(|n| n.id).filter(|id| !self.ids.to_snarl.contains_key(id)).collect();
        for (sid, gid) in fresh.into_iter().zip(unmapped) {
            if let Some(card) = self.snarl.get_node_mut(sid) {
                card.id = gid.0;
            }
            self.ids.to_snarl.insert(gid, sid);
            self.ids.to_graph.insert(sid, gid);
        }
    }

    /// Insert a node of `kind` at a view position; it gets its id on the
    /// next extraction (or [`Editor::commit`]).
    pub fn insert(&mut self, kind: &str, pos: [f32; 2], reg: &Registry) -> bool {
        let Some(spec) = reg.get(kind) else { return false };
        self.snarl.insert_node(egui::pos2(pos[0], pos[1]), NodeCard::new_of(spec, reg));
        true
    }

    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(&sid) = self.ids.to_snarl.get(&id) else { return false };
        self.snarl.remove_node(sid);
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.commit();
        true
    }

    /// Extract the view now and adopt any change; true if the graph moved.
    pub fn commit(&mut self) -> bool {
        let extracted = extract_graph(&self.snarl, &self.graph);
        if extracted == self.graph {
            return false;
        }
        self.graph = extracted;
        self.revision += 1;
        self.ids = IdMap::default();
        for (sid, card) in self.snarl.nodes_ids_mut() {
            if let Some(gid) = card.graph_id() {
                self.ids.to_snarl.insert(gid, sid);
                self.ids.to_graph.insert(sid, gid);
            }
        }
        self.assign_fresh_ids();
        true
    }

    pub fn card(&self, id: NodeId) -> Option<&NodeCard> {
        self.ids.to_snarl.get(&id).and_then(|sid| self.snarl.get_node(*sid))
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.graph.node(id)
    }

    /// Set (or clear) a literal on a node's input, through the view so the
    /// card and the graph agree.
    pub fn set_input(&mut self, id: NodeId, input: &str, value: Option<Literal>) -> bool {
        let Some(&sid) = self.ids.to_snarl.get(&id) else { return false };
        if let Some(card) = self.snarl.get_node_mut(sid) {
            match value {
                Some(v) => {
                    card.inputs.insert(input.to_string(), v);
                }
                None => {
                    card.inputs.remove(input);
                }
            }
        }
        self.commit()
    }

    pub fn set_label(&mut self, id: NodeId, label: Option<String>) -> bool {
        let Some(&sid) = self.ids.to_snarl.get(&id) else { return false };
        if let Some(card) = self.snarl.get_node_mut(sid) {
            card.label = label.clone().filter(|l| !l.trim().is_empty());
            card.title = card.label.clone().unwrap_or_else(|| card.kind.clone());
        }
        self.commit()
    }

    /// Promote an input to the graph's parameter panel.
    pub fn expose(&mut self, id: NodeId, input: &str, name: &str) -> Result<(), GraphError> {
        self.graph.expose(id, input, name)?;
        self.revision += 1;
        Ok(())
    }

    pub fn unexpose(&mut self, name: &str) -> bool {
        let hit = self.graph.unexpose(name).is_some();
        if hit {
            self.revision += 1;
        }
        hit
    }

    /// Lay the nodes out by depth, as the template files are.
    pub fn arrange(&mut self, reg: &Registry) {
        let mut g = self.graph.clone();
        ringdesign_graph::templates::arrange(&mut g);
        self.set_graph(g, reg);
    }
}

struct Viewer<'a> {
    reg: &'a Registry,
    editable: bool,
    clicked: Option<SnarlId>,
    refused: Option<String>,
    search: String,
    ids: &'a IdMap,
    focus: Option<SnarlId>,
    viewport_center: egui::Pos2,
}

fn pin_info(pin: &PinSpec) -> PinInfo {
    let color = kind_color(pin.kind);
    let info = match pin.access {
        Access::Item => PinInfo::circle(),
        Access::List => PinInfo::square(),
    };
    info.with_fill(color).with_stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
}

impl SnarlViewer<NodeCard> for Viewer<'_> {
    fn title(&mut self, node: &NodeCard) -> String {
        node.title.clone()
    }

    fn node_frame(&mut self, default: egui::Frame, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], snarl: &Snarl<NodeCard>) -> egui::Frame {
        match snarl.get_node(node) {
            Some(c) if !c.diag.is_empty() => default.stroke(egui::Stroke::new(2.0, Color32::from_rgb(220, 70, 70))),
            Some(c) if self.ids.to_graph.get(&node).is_some_and(|g| c.graph_id() == Some(*g)) => default,
            _ => default,
        }
    }

    fn inputs(&mut self, node: &NodeCard) -> usize {
        node.pins_in.len()
    }

    fn outputs(&mut self, node: &NodeCard) -> usize {
        node.pins_out.len()
    }

    fn show_input(&mut self, pin: &InPin, ui: &mut Ui, snarl: &mut Snarl<NodeCard>) -> impl egui_snarl::ui::SnarlPin + 'static {
        let card = &mut snarl[pin.id.node];
        let Some(spec) = card.pins_in.get(pin.id.input).cloned() else { return PinInfo::circle() };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&spec.name).small().color(kind_color(spec.kind))).on_hover_text(format!("{}\n{}", spec.kind.label(), spec.doc));
            if pin.remotes.is_empty() && self.editable {
                let mut lit = card.inputs.get(&spec.name).cloned();
                if pin_widget(ui, &spec, &mut lit) {
                    match lit {
                        Some(l) => {
                            card.inputs.insert(spec.name.clone(), l);
                        }
                        None => {
                            card.inputs.remove(&spec.name);
                        }
                    }
                }
            }
        });
        pin_info(&spec)
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut Ui, snarl: &mut Snarl<NodeCard>) -> impl egui_snarl::ui::SnarlPin + 'static {
        let card = &snarl[pin.id.node];
        let Some(spec) = card.pins_out.get(pin.id.output).cloned() else { return PinInfo::circle() };
        ui.horizontal(|ui| {
            if let Some(v) = card.values.get(&spec.name) {
                ui.weak(RichText::new(v).small());
            }
            ui.label(RichText::new(&spec.name).small().color(kind_color(spec.kind))).on_hover_text(format!("{}\n{}", spec.kind.label(), spec.doc));
        });
        pin_info(&spec)
    }

    fn has_on_hover_popup(&mut self, node: &NodeCard) -> bool {
        !node.diag.is_empty()
    }

    fn show_on_hover_popup(&mut self, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        for d in &snarl[node].diag {
            ui.colored_label(Color32::from_rgb(220, 90, 90), d);
        }
    }

    fn current_transform(&mut self, to_global: &mut egui::emath::TSTransform, snarl: &mut Snarl<NodeCard>) {
        if let Some(sid) = self.focus.take() {
            if let Some(info) = snarl.get_node_info(sid) {
                // The node's top-left, pushed a little so the header sits
                // near the centre rather than the corner.
                let anchor = info.pos + egui::vec2(110.0, 40.0);
                let scale = to_global.scaling.max(0.1);
                to_global.translation = self.viewport_center.to_vec2() - anchor.to_vec2() * scale;
            }
        }
    }

    fn final_node_rect(&mut self, node: SnarlId, rect: egui::Rect, ui: &mut Ui, _snarl: &mut Snarl<NodeCard>) {
        let clicked = ui.input(|i| i.pointer.primary_clicked() && i.pointer.interact_pos().is_some_and(|p| rect.contains(p)));
        if clicked {
            self.clicked = Some(node);
        }
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeCard>) {
        if !self.editable {
            return;
        }
        let out_kind = snarl.get_node(from.id.node).and_then(|c| c.pins_out.get(from.id.output)).map(|p| p.kind);
        let in_pin = snarl.get_node(to.id.node).and_then(|c| c.pins_in.get(to.id.input)).cloned();
        let (Some(out_kind), Some(in_pin)) = (out_kind, in_pin) else { return };
        let ok = in_pin.kind.accepts(out_kind) || in_pin.access == Access::List || in_pin.kind == ValueKind::List;
        if !ok {
            self.refused = Some(format!("{} takes {}, not {}", in_pin.name, in_pin.kind.label(), out_kind.label()));
            return;
        }
        if from.id.node == to.id.node {
            self.refused = Some("a node cannot feed itself".into());
            return;
        }
        snarl.drop_inputs(to.id);
        snarl.connect(from.id, to.id);
    }

    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<NodeCard>) {
        if self.editable {
            snarl.disconnect(from.id, to.id);
        }
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<NodeCard>) {
        if self.editable {
            snarl.drop_outputs(pin.id);
        }
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<NodeCard>) {
        if self.editable {
            snarl.drop_inputs(pin.id);
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<NodeCard>) -> bool {
        self.editable
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        ui.set_min_width(220.0);
        ui.label(RichText::new("Add node").small().weak());
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("search…"));
        let mode = ringdesign_graph::graph::Mode::SandRing;
        let specs = self.reg.list(mode);
        let needle = self.search.trim().to_lowercase();
        if !needle.is_empty() {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for spec in specs.iter().filter(|s| s.key.to_lowercase().contains(&needle) || s.label.to_lowercase().contains(&needle)) {
                    if ui.button(format!("{}  ({})", spec.label, spec.key)).on_hover_text(&spec.doc).clicked() {
                        snarl.insert_node(pos, NodeCard::new_of(spec, self.reg));
                        self.search.clear();
                        ui.close();
                    }
                }
            });
            return;
        }
        for cat in Category::ALL {
            let in_cat: Vec<&&NodeSpec> = specs.iter().filter(|s| s.category == *cat).collect();
            if in_cat.is_empty() {
                continue;
            }
            ui.menu_button(cat.label(), |ui| {
                for spec in in_cat {
                    if ui.button(&spec.label).on_hover_text(format!("{}\n{}", spec.key, spec.doc)).clicked() {
                        snarl.insert_node(pos, NodeCard::new_of(spec, self.reg));
                        ui.close();
                    }
                }
            });
        }
    }

    fn has_node_menu(&mut self, _node: &NodeCard) -> bool {
        self.editable
    }

    fn show_node_menu(&mut self, node: SnarlId, inputs: &[InPin], outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        if ui.button("Delete").clicked() {
            snarl.remove_node(node);
            ui.close();
            return;
        }
        if ui.button("Duplicate").clicked() {
            if let Some(info) = snarl.get_node_info(node) {
                let pos = info.pos + egui::vec2(40.0, 40.0);
                let mut card = info.value.clone();
                card.id = 0;
                card.diag.clear();
                card.values.clear();
                snarl.insert_node(pos, card);
            }
            ui.close();
            return;
        }
        if ui.button("Disconnect all").clicked() {
            for p in inputs {
                snarl.drop_inputs(p.id);
            }
            for p in outputs {
                snarl.drop_outputs(p.id);
            }
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_graph::graph::Mode;

    #[test]
    fn extract_of_build_is_the_graph_for_every_template() {
        let reg = Registry::builtin();
        for (name, g) in ringdesign_graph::templates::all() {
            let (snarl, _) = build_snarl(&g, &reg);
            let back = extract_graph(&snarl, &g);
            assert_eq!(back, g, "{name}");
        }
        let simple = ringdesign_graph::templates::simple();
        let (snarl, _) = build_snarl(&simple, &reg);
        assert_eq!(extract_graph(&snarl, &simple), simple);
    }

    #[test]
    fn a_palette_insert_gets_a_fresh_id_and_a_refused_wire_leaves_none() {
        let reg = Registry::builtin();
        let mut g = Graph::new("t", Mode::SandRing);
        let n = g.add("number").unwrap();
        let t = g.add("text").unwrap();
        let a = g.add("math.add").unwrap();
        g.connect(n, "out", a, "a").unwrap();
        let mut ed = Editor::new(g.clone(), &reg);
        assert!(ed.insert("math.mul", [300.0, 0.0], &reg));
        let extracted = extract_graph(ed.snarl(), ed.graph());
        assert_eq!(extracted.nodes.len(), 4);
        let fresh = extracted.nodes.iter().find(|x| x.kind == "math.mul").unwrap();
        assert_eq!(fresh.id, NodeId(4), "above every id the graph handed out");
        assert_eq!(extracted.next_id, 5);

        // Text -> Number is refused by the viewer; Number -> Number replaces.
        let (mut snarl, ids) = build_snarl(&g, &reg);
        let mut viewer = Viewer { reg: &reg, editable: true, clicked: None, refused: None, search: String::new(), ids: &ids, focus: None, viewport_center: egui::Pos2::ZERO };
        let text_out = OutPin { id: OutPinId { node: ids.to_snarl[&t], output: 0 }, remotes: vec![] };
        let add_b = InPin { id: InPinId { node: ids.to_snarl[&a], input: 1 }, remotes: vec![] };
        viewer.connect(&text_out, &add_b, &mut snarl);
        assert!(viewer.refused.as_deref().unwrap_or("").contains("takes number"), "{:?}", viewer.refused);
        let back = extract_graph(&snarl, &g);
        assert_eq!(back.wires.len(), 1, "no wire was made");
        let num_out = OutPin { id: OutPinId { node: ids.to_snarl[&n], output: 0 }, remotes: vec![] };
        let add_a = InPin { id: InPinId { node: ids.to_snarl[&a], input: 0 }, remotes: vec![] };
        viewer.refused = None;
        viewer.connect(&num_out, &add_a, &mut snarl);
        assert!(viewer.refused.is_none());
        let back = extract_graph(&snarl, &g);
        assert_eq!(back.wires.len(), 1, "one wire per input: the new one replaced the old");
        assert!(back.validate(Some(&reg)).is_empty());
    }

    #[test]
    fn editing_a_literal_on_a_card_moves_the_graph() {
        let reg = Registry::builtin();
        let mut g = Graph::new("t", Mode::SandRing);
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "width_mm", Literal::Number(6.0)).unwrap();
        let mut ed = Editor::new(g.clone(), &reg);
        let sid = ed.ids.to_snarl[&p];
        ed.snarl.get_node_mut(sid).unwrap().inputs.insert("width_mm".into(), Literal::Number(8.0));
        let extracted = extract_graph(ed.snarl(), ed.graph());
        assert_eq!(extracted.node(p).unwrap().inputs.get("width_mm"), Some(&Literal::Number(8.0)));
        assert_ne!(extracted, g);
        // set_graph rebuilds the view and bumps the revision; removal drops exposures.
        g.expose(p, "width_mm", "Width").unwrap();
        ed.set_graph(g.clone(), &reg);
        assert_eq!(ed.revision, 1);
        assert!(ed.remove(p));
        let extracted = extract_graph(ed.snarl(), ed.graph());
        assert!(extracted.nodes.is_empty() && extracted.exposed.is_empty());
    }

    #[test]
    fn focus_selects_and_is_consumed_by_a_frame() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Braided band").unwrap();
        let entries = g.entry_nodes();
        assert_eq!(entries.len(), 2, "two layers, two entry nodes, in stack order");
        assert!(g.node(entries[0]).unwrap().inputs.get("name") == Some(&Literal::Text("Braid".into())));
        let mut ed = Editor::new(g, &reg);
        ed.focus(entries[1]);
        assert_eq!(ed.selected, Some(entries[1]));
        assert!(ed.pending_focus.is_some());
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            ed.show(&reg, ui, "focus-editor");
        });
        harness.set_size(egui::vec2(900.0, 600.0));
        harness.run();
        drop(harness);
        assert!(ed.pending_focus.is_none(), "one frame consumes the focus");
        ed.focus(NodeId(999));
        assert!(ed.pending_focus.is_none(), "an unknown node is ignored");
    }

    #[test]
    fn the_editor_draws_headless_and_finds_its_nodes() {
        let reg = Registry::builtin();
        let simple = ringdesign_graph::templates::simple();
        let mut ed = Editor::new(simple, &reg);
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            ed.show(&reg, ui, "test-editor");
        });
        harness.set_size(egui::vec2(1400.0, 700.0));
        harness.run();
        use egui_kittest::kittest::Queryable;
        for want in ["Band profile", "Shank", "New design", "Output"] {
            assert!(harness.query_by_label(want).is_some(), "{want} not drawn");
        }
    }
}
