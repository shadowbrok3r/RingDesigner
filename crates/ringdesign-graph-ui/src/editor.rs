//! The editor widget: build a snarl from the graph, show it, extract it back.
//!
//! [`Editor`] owns a [`Graph`] and the `Snarl` that views it. The snarl is
//! rebuilt whenever the graph is replaced from outside ([`Editor::set_graph`])
//! and extracted after every frame it is shown; when what comes back differs
//! from what went in, the graph is updated and the revision moves. Nodes
//! added in the view carry no id until extraction hands them a fresh one.

use std::collections::{BTreeMap, HashMap};

use egui::{Color32, RichText, Ui};
use egui_snarl::ui::{BackgroundPattern, PinInfo, SnarlStyle, SnarlViewer};
use egui_snarl::{InPin, InPinId, NodeId as SnarlId, OutPin, OutPinId, Snarl};
use ringdesign_graph::graph::{Access, Graph, GraphError, Node, NodeId, Wire};
use ringdesign_graph::registry::{Category, NodeSpec, PinSpec, Registry};
use ringdesign_graph::value::{Literal, ValueKind};

use crate::widgets::pin_widget;

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
    /// Fit every node into the view at the next frame.
    pending_fit: bool,
    /// The view's transform as of the last frame, for the minimap.
    transform: Option<egui::emath::TSTransform>,
    /// The persistent id the snarl was last shown under, for selection.
    snarl_id: Option<egui::Id>,
    pub show_minimap: bool,
    /// Every node's drawn size, by graph id so a rebuilt snarl keeps them.
    sizes: HashMap<NodeId, egui::Vec2>,
    /// The sizes the last arrange laid out from, and how many correction
    /// passes it may still take: snarl's first frame measures a node before
    /// its widgets have settled, so a layout is re-run while the measures
    /// it used disagree with the current ones.
    arranged_sizes: HashMap<NodeId, egui::Vec2>,
    refine_left: u8,
    /// The editor moved nodes itself this frame; reported as a change.
    layout_changed: bool,
    /// Where the current drag began, held for the whole drag.
    drag_kind: DragKind,
}

/// Where a canvas press landed — decides whether a drag moves a node, pans,
/// or wires. A drag that starts on a node's body pans and the node stays
/// put, so a finger that misses a pin scrolls the view instead of dragging
/// the node around; only the title bar moves a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragKind {
    None,
    /// On a node's title bar: the node moves.
    Header,
    /// On a node below its title: the view pans, the node's move is vetoed.
    Body,
    /// Empty canvas or a pin: left to snarl.
    Canvas,
}

/// Graph-space height of a node's title bar, for header-only dragging.
const NODE_HEADER_H: f32 = 30.0;
/// snarl's node frame margin: a node's stored position is its content
/// origin, the drawn frame starts this far above-left.
const FRAME_MARGIN: f32 = 6.0;

/// Which region a graph-space point falls on, over the drawn node frames.
/// Any header wins over any body, so a title-bar grab can always move a
/// node where nodes overlap.
pub fn classify_point(gp: egui::Pos2, frames: impl IntoIterator<Item = egui::Rect>) -> DragKind {
    let mut on_body = false;
    for frame in frames {
        if !frame.contains(gp) {
            continue;
        }
        let header = egui::Rect::from_min_size(frame.min, egui::vec2(frame.width(), NODE_HEADER_H.min(frame.height())));
        if header.contains(gp) {
            return DragKind::Header;
        }
        on_body = true;
    }
    if on_body { DragKind::Body } else { DragKind::Canvas }
}

impl Editor {
    pub fn new(graph: Graph, reg: &Registry) -> Self {
        let (snarl, ids) = build_snarl(&graph, reg);
        Self {
            graph,
            snarl,
            ids,
            revision: 0,
            selected: None,
            editable: true,
            style: crate::style::snarl_style(),
            pending_focus: None,
            pending_fit: false,
            transform: None,
            snarl_id: None,
            show_minimap: true,
            sizes: HashMap::new(),
            arranged_sizes: HashMap::new(),
            refine_left: 0,
            layout_changed: false,
            drag_kind: DragKind::None,
        }
    }

    /// The drawn frames of every node, in graph space.
    fn node_frames(&self) -> Vec<egui::Rect> {
        self.snarl
            .nodes_pos_ids()
            .map(|(sid, pos, _)| egui::Rect::from_min_size(pos - egui::vec2(FRAME_MARGIN, FRAME_MARGIN), self.size_of(sid)))
            .collect()
    }

    /// Classifies a press this frame and returns `(pan, veto)`: the screen
    /// delta to pan by, and whether every node move this frame is undone.
    /// A body drag pans; a locked editor pans on any node drag and vetoes
    /// every move; header, canvas and pin drags are left to snarl.
    fn drag_gate(&mut self, ctx: &egui::Context, viewport: egui::Rect) -> (egui::Vec2, bool) {
        let (pressed, down, origin, delta, zooming) = ctx.input(|i| {
            (i.pointer.any_pressed(), i.pointer.any_down(), i.pointer.press_origin(), i.pointer.delta(), (i.zoom_delta() - 1.0).abs() > f32::EPSILON || i.multi_touch().is_some())
        });
        if pressed {
            self.drag_kind = match (origin, self.transform) {
                (Some(p), Some(t)) if viewport.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background) => {
                    classify_point(t.inverse() * p, self.node_frames())
                }
                _ => DragKind::None,
            };
        }
        if !down {
            self.drag_kind = DragKind::None;
        }
        // During a pinch the primary pointer still reports a delta; adding a
        // pan on top of the zoom drifts the view.
        let d = if delta.is_finite() && !zooming { delta } else { egui::Vec2::ZERO };
        match (self.editable, self.drag_kind) {
            (false, DragKind::Header | DragKind::Body) => (d, true),
            (false, _) => (egui::Vec2::ZERO, true),
            (true, DragKind::Body) => (d, true),
            (true, _) => (egui::Vec2::ZERO, false),
        }
    }

    /// A node's drawn size, or the nominal footprint before it has been drawn.
    fn size_of(&self, sid: SnarlId) -> egui::Vec2 {
        self.ids.to_graph.get(&sid).and_then(|g| self.sizes.get(g)).copied().unwrap_or(NODE_SIZE)
    }

    fn all_measured(&self) -> bool {
        self.graph.nodes.iter().all(|n| self.sizes.contains_key(&n.id))
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

    /// Fit the whole graph into the view at the next frame.
    pub fn fit(&mut self) {
        self.pending_fit = true;
    }

    /// The nodes the view has selected (rubber-band or click), as graph ids.
    pub fn selected_nodes(&self, ctx: &egui::Context) -> Vec<NodeId> {
        let Some(id) = self.snarl_id else { return Vec::new() };
        egui_snarl::ui::get_selected_nodes(id, ctx).into_iter().filter_map(|sid| self.ids.to_graph.get(&sid).copied()).collect()
    }

    /// Fold nodes into a cluster node and rebuild the view.
    pub fn collapse(&mut self, ids: &[NodeId], name: &str, reg: &Registry) -> Result<NodeId, GraphError> {
        let mut g = self.graph.clone();
        let cid = g.collapse(ids, name)?;
        self.set_graph(g, reg);
        self.selected = Some(cid);
        Ok(cid)
    }

    /// Fold a node and everything feeding it into a cluster.
    pub fn collapse_upstream(&mut self, id: NodeId, name: &str, reg: &Registry) -> Result<NodeId, GraphError> {
        let mut ids = self.graph.upstream(id);
        ids.push(id);
        self.collapse(&ids, name, reg)
    }

    /// Draw the editor and extract any change.
    pub fn show(&mut self, reg: &Registry, ui: &mut Ui, id_salt: &str) -> EditorResponse {
        let focus = self.pending_focus.take().and_then(|id| self.ids.to_snarl.get(&id).copied());
        let viewport = ui.available_rect_before_wrap();
        let fit = std::mem::take(&mut self.pending_fit).then_some(viewport);
        self.snarl_id = Some(ui.make_persistent_id(id_salt));
        // Header-only dragging: a body drag pans, and any node move that did
        // not start on a title bar is undone from this snapshot after the
        // frame — the same mechanism a locked editor freezes every node with.
        let (pan, veto) = self.drag_gate(ui.ctx(), viewport);
        let saved: Option<Vec<(SnarlId, egui::Pos2)>> = veto.then(|| self.snarl.nodes_pos_ids().map(|(id, pos, _)| (id, pos)).collect());
        let mut viewer = Viewer {
            reg,
            editable: self.editable,
            clicked: None,
            refused: None,
            search: String::new(),
            ids: &self.ids,
            focus,
            fit,
            viewport_center: viewport.center(),
            seen_transform: None,
            collapse_request: None,
            mode: self.graph.mode,
            selected: self.selected.and_then(|g| self.ids.to_snarl.get(&g).copied()),
            sizes: &mut self.sizes,
            pan,
        };
        // That app's visuals on everything inside the editor, and nothing outside it.
        ui.scope(|ui| {
            crate::style::apply_visuals(ui.style_mut());
            self.snarl.show(&mut viewer, &self.style, id_salt, ui);
        });
        if let Some(saved) = saved {
            for (id, pos) in saved {
                if let Some(info) = self.snarl.get_node_info_mut(id) {
                    info.pos = pos;
                }
            }
        }
        let clicked = viewer.clicked;
        let refused = viewer.refused.take();
        let collapse_request = viewer.collapse_request.take();
        self.transform = viewer.seen_transform.or(self.transform);
        // The first arrange ran on nominal sizes; once every node has been
        // drawn once, lay them out again from what they really measure.
        if self.refine_left > 0 && self.all_measured() {
            if sizes_agree(&self.arranged_sizes, &self.sizes) {
                self.refine_left = 0;
            } else {
                self.refine_left -= 1;
                self.lay_out(reg);
                self.layout_changed = true;
            }
        }
        if self.show_minimap {
            self.paint_minimap(ui, viewport);
        }
        if let Some(sid) = collapse_request {
            if let Some(gid) = self.ids.to_graph.get(&sid).copied() {
                let name = format!("Cluster {}", self.graph.nodes.iter().filter(|n| n.kind == "cluster").count() + 1);
                let _ = self.collapse_upstream(gid, &name, reg);
                return EditorResponse { changed: true, selected: self.selected, refused: None };
            }
        }
        let mut resp = EditorResponse { refused, changed: std::mem::take(&mut self.layout_changed), ..Default::default() };
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

    /// Lay the nodes out by depth from their measured sizes — columns by
    /// longest path, nodes stacked within a column, columns centred across
    /// the flow — so nothing overlaps. Nodes not yet drawn take the nominal
    /// footprint and a second pass follows once they have been.
    pub fn arrange(&mut self, reg: &Registry) {
        self.refine_left = 3;
        self.lay_out(reg);
    }

    fn lay_out(&mut self, reg: &Registry) {
        const FLOW_GAP: f32 = 60.0;
        const CROSS_GAP: f32 = 24.0;
        let Ok(order) = self.graph.topo() else { return };
        let mut depth: BTreeMap<NodeId, usize> = BTreeMap::new();
        for id in &order {
            let d = self.graph.wires_into(*id).filter_map(|w| depth.get(&w.from)).max().map(|m| m + 1).unwrap_or(0);
            depth.insert(*id, d);
        }
        let deepest = depth.values().copied().max().unwrap_or(0);
        let mut columns: Vec<Vec<NodeId>> = vec![Vec::new(); deepest + 1];
        for id in &order {
            columns[depth[id]].push(*id);
        }
        // Each column ordered by where its feeders sit, so wires run across.
        let mut row: BTreeMap<NodeId, f32> = BTreeMap::new();
        for column in columns.iter_mut() {
            let mut keyed: Vec<(NodeId, f32)> = column
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let feeders: Vec<f32> = self.graph.wires_into(*id).filter_map(|w| row.get(&w.from).copied()).collect();
                    let key = if feeders.is_empty() { i as f32 } else { feeders.iter().sum::<f32>() / feeders.len() as f32 };
                    (*id, key)
                })
                .collect();
            keyed.sort_by(|a, b| a.1.total_cmp(&b.1));
            *column = keyed.iter().map(|(id, _)| *id).collect();
            for (i, id) in column.iter().enumerate() {
                row.insert(*id, i as f32);
            }
        }
        let size = |id: NodeId| self.sizes.get(&id).copied().unwrap_or(NODE_SIZE);
        let mut x = 0.0f32;
        let mut pos: BTreeMap<NodeId, [f32; 2]> = BTreeMap::new();
        for column in &columns {
            if column.is_empty() {
                continue;
            }
            let total: f32 = column.iter().map(|id| size(*id).y + CROSS_GAP).sum::<f32>() - CROSS_GAP;
            let mut y = -total / 2.0;
            for id in column {
                pos.insert(*id, [x, y]);
                y += size(*id).y + CROSS_GAP;
            }
            x += column.iter().map(|id| size(*id).x).fold(1.0f32, f32::max) + FLOW_GAP;
        }
        let mut g = self.graph.clone();
        for n in g.nodes.iter_mut() {
            if let Some(p) = pos.get(&n.id) {
                n.pos = [p[0].round(), p[1].round()];
            }
        }
        self.arranged_sizes = self.sizes.clone();
        self.set_graph(g, reg);
        self.pending_fit = true;
    }
}

/// Two measure snapshots describe the same canvas: same nodes, none moved
/// by more than a unit.
fn sizes_agree(a: &HashMap<NodeId, egui::Vec2>, b: &HashMap<NodeId, egui::Vec2>) -> bool {
    a.len() == b.len() && a.iter().all(|(id, s)| b.get(id).is_some_and(|p| (*s - *p).abs().max_elem() <= 1.0))
}

struct Viewer<'a> {
    reg: &'a Registry,
    editable: bool,
    clicked: Option<SnarlId>,
    refused: Option<String>,
    search: String,
    ids: &'a IdMap,
    focus: Option<SnarlId>,
    fit: Option<egui::Rect>,
    viewport_center: egui::Pos2,
    seen_transform: Option<egui::emath::TSTransform>,
    collapse_request: Option<SnarlId>,
    mode: ringdesign_graph::graph::Mode,
    /// The chosen node, for its rim.
    selected: Option<SnarlId>,
    /// Measured node sizes, filled as nodes are drawn.
    sizes: &'a mut HashMap<NodeId, egui::Vec2>,
    /// Screen delta to pan the view by this frame.
    pan: egui::Vec2,
}

/// The footprint a node is assumed to take before it has been drawn.
const NODE_SIZE: egui::Vec2 = egui::vec2(220.0, 140.0);

/// Every node's rect from its measured size, in graph space.
fn node_bounds(snarl: &Snarl<NodeCard>, sizes: &HashMap<NodeId, egui::Vec2>, ids: &IdMap) -> Option<egui::Rect> {
    let mut rect: Option<egui::Rect> = None;
    for (sid, pos, _) in snarl.nodes_pos_ids() {
        let size = ids.to_graph.get(&sid).and_then(|g| sizes.get(g)).copied().unwrap_or(NODE_SIZE);
        let r = egui::Rect::from_min_size(pos, size);
        rect = Some(rect.map_or(r, |b| b.union(r)));
    }
    rect
}

impl Editor {
    /// A small map of every node and the current view, in the top-left
    /// corner as that app keeps it.
    fn paint_minimap(&self, ui: &Ui, viewport: egui::Rect) {
        let Some(bounds) = node_bounds(&self.snarl, &self.sizes, &self.ids) else { return };
        if self.snarl.node_ids().count() < 2 || !viewport.is_finite() || viewport.width() < 160.0 || viewport.height() < 160.0 {
            return;
        }
        let pad = bounds.expand(60.0);
        let w = (viewport.width() * 0.30).clamp(96.0, 200.0);
        let h = (w * (pad.height() / pad.width().max(1.0)).clamp(0.35, 1.4)).clamp(60.0, 200.0);
        let map = egui::Rect::from_min_size(viewport.left_top() + egui::vec2(10.0, 10.0), egui::vec2(w, h));
        let painter = ui.painter().with_clip_rect(viewport);
        painter.rect_filled(map, 4.0, Color32::from_black_alpha(170));
        let scale = (map.size() / pad.size()).min_elem();
        let tf = egui::emath::TSTransform::new(map.center().to_vec2() - pad.center().to_vec2() * scale, scale);
        for (sid, pos, card) in self.snarl.nodes_pos_ids() {
            let mut m = tf * egui::Rect::from_min_size(pos, self.size_of(sid));
            if m.width() < 2.0 || m.height() < 2.0 {
                m = egui::Rect::from_center_size(m.center(), m.size().max(egui::vec2(2.0, 2.0)));
            }
            let selected = self.ids.to_graph.get(&sid).is_some_and(|g| self.selected == Some(*g));
            let color = if !card.diag.is_empty() {
                crate::style::ERROR
            } else if selected {
                Color32::from_rgb(110, 170, 255)
            } else {
                Color32::from_gray(150)
            };
            painter.rect_filled(m.intersect(map), 1.0, color);
        }
        if let Some(t) = self.transform {
            let inv = t.inverse();
            let view = egui::Rect::from_min_max(inv * viewport.min, inv * viewport.max);
            painter.rect_stroke((tf * view).intersect(map), 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        painter.rect_stroke(map, 4.0, egui::Stroke::new(1.0, Color32::from_gray(90)), egui::StrokeKind::Inside);
    }
}

fn pin_info(pin: &PinSpec) -> PinInfo {
    crate::style::pin_info(pin.kind, pin.access)
}

impl SnarlViewer<NodeCard> for Viewer<'_> {
    fn title(&mut self, node: &NodeCard) -> String {
        node.title.clone()
    }

    fn node_frame(&mut self, default: egui::Frame, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], snarl: &Snarl<NodeCard>) -> egui::Frame {
        let trouble = snarl.get_node(node).is_some_and(|c| !c.diag.is_empty());
        crate::style::node_frame(default, self.selected == Some(node), trouble)
    }

    fn draw_background(&mut self, _background: Option<&BackgroundPattern>, viewport: &egui::Rect, _snarl_style: &SnarlStyle, _style: &egui::Style, painter: &egui::Painter, _snarl: &Snarl<NodeCard>) {
        let scale = self.seen_transform.map(|t| t.scaling).unwrap_or(1.0);
        crate::style::paint_canvas(painter, *viewport, scale);
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
        ui.set_max_width(crate::style::NODE_FIELD_W);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&spec.name).color(crate::style::INK)).on_hover_text(format!("{}\n{}", spec.kind.label(), spec.doc));
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
                ui.label(RichText::new(v).small().color(crate::style::INK_DIM));
            }
            ui.label(RichText::new(&spec.name).color(crate::style::INK)).on_hover_text(format!("{}\n{}", spec.kind.label(), spec.doc));
        });
        pin_info(&spec)
    }

    fn has_on_hover_popup(&mut self, node: &NodeCard) -> bool {
        !node.diag.is_empty()
    }

    fn show_on_hover_popup(&mut self, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        for d in &snarl[node].diag {
            ui.colored_label(crate::style::ERROR, d);
        }
    }

    fn current_transform(&mut self, to_global: &mut egui::emath::TSTransform, snarl: &mut Snarl<NodeCard>) {
        if let Some(viewport) = self.fit.take() {
            if let Some(bounds) = node_bounds(snarl, self.sizes, self.ids) {
                let pad = bounds.expand(40.0);
                let scale = (viewport.width() / pad.width().max(1.0)).min(viewport.height() / pad.height().max(1.0)).clamp(crate::style::MIN_SCALE, crate::style::MAX_SCALE);
                to_global.scaling = scale;
                to_global.translation = viewport.center().to_vec2() - pad.center().to_vec2() * scale;
            }
        }
        if let Some(sid) = self.focus.take() {
            if let Some(info) = snarl.get_node_info(sid) {
                // The node's top-left, pushed a little so the header sits
                // near the centre rather than the corner.
                let anchor = info.pos + egui::vec2(110.0, 40.0);
                let scale = to_global.scaling.max(0.1);
                to_global.translation = self.viewport_center.to_vec2() - anchor.to_vec2() * scale;
            }
        }
        if self.pan != egui::Vec2::ZERO {
            to_global.translation += self.pan;
        }
        self.seen_transform = Some(*to_global);
    }

    fn final_node_rect(&mut self, node: SnarlId, rect: egui::Rect, ui: &mut Ui, _snarl: &mut Snarl<NodeCard>) {
        if let Some(gid) = self.ids.to_graph.get(&node) {
            // Clamped so one pathological measure cannot throw the layout.
            self.sizes.insert(*gid, rect.size().min(egui::vec2(1600.0, 3000.0)));
        }
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
        let specs = self.reg.list(self.mode);
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
        if ui.button("Collapse with upstream into a cluster").on_hover_text("This node and everything feeding it become one cluster node").clicked() {
            self.collapse_request = Some(node);
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
        let mut viewer = Viewer { reg: &reg, editable: true, clicked: None, refused: None, search: String::new(), ids: &ids, focus: None, fit: None, viewport_center: egui::Pos2::ZERO, seen_transform: None, collapse_request: None, mode: Mode::SandRing , selected: None, sizes: &mut HashMap::new(), pan: egui::Vec2::ZERO };
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

        // Fit is consumed by a frame too, and leaves a transform the minimap reads.
        ed.fit();
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            ed.show(&reg, ui, "focus-editor");
        });
        harness.set_size(egui::vec2(900.0, 600.0));
        harness.run();
        drop(harness);
        assert!(!ed.pending_fit);
        assert!(ed.transform.is_some());
        assert!(ed.snarl_id.is_some());

        // Collapsing a layer's entry with its upstream leaves the graph evaluable.
        let g = ringdesign_graph::templates::graph("Braided band").unwrap();
        let mut ed = Editor::new(g, &reg);
        let entry = ed.graph().entry_nodes()[1];
        let cid = ed.collapse_upstream(entry, "Milgrain cluster", &reg).unwrap();
        assert!(ed.graph().validate(Some(&reg)).is_empty(), "{:?}", ed.graph().validate(Some(&reg)));
        assert_eq!(ed.selected, Some(cid));
        let out = ringdesign_graph::eval::evaluate_design(&mut ringdesign_graph::eval::Evaluator::new(), ed.graph(), &reg, &ringdesign_core::AlphaLibrary::builtin(), 0).unwrap();
        assert_eq!(out.design.layers.layers.len(), 2, "both layers still arrive: {:?}", out.notes);
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

#[cfg(all(test, feature = "shot"))]
mod shot {
    /// `RD_GRAPH_SHOT=/some/dir cargo test -p ringdesign-graph-ui --features shot shot_the_editor`
    /// renders the editor over the Court band's graph through wgpu and
    /// writes `graph.png` there, for an eyeball pass on the look.
    #[test]
    fn shot_the_editor() {
        let Some(dir) = std::env::var_os("RD_GRAPH_SHOT") else { return };
        let reg = ringdesign_graph::registry::Registry::builtin();
        let (_, g) = ringdesign_graph::templates::all().into_iter().next().expect("a template graph");
        let mut ed = super::Editor::new(g, &reg);
        ed.arrange(&reg);
        ed.fit();
        ed.selected = ed.graph().nodes.get(2).map(|n| n.id);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui(|ui| {
                ed.show(&reg, ui, "shot");
            });
        harness.run_steps(8);
        let img = harness.render().expect("wgpu renders offscreen");
        let path = std::path::Path::new(&dir).join("graph.png");
        img.save(&path).expect("png");
        eprintln!("wrote {}", path.display());
    }
}

#[cfg(test)]
mod drag_tests {
    use super::*;

    #[test]
    fn a_press_is_classified_by_where_it_lands() {
        let a = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 120.0));
        let b = egui::Rect::from_min_size(egui::pos2(150.0, 100.0), egui::vec2(200.0, 120.0));
        let frames = || [a, b];
        assert_eq!(classify_point(egui::pos2(20.0, 10.0), frames()), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), frames()), DragKind::Body);
        assert_eq!(classify_point(egui::pos2(400.0, 400.0), frames()), DragKind::Canvas);
        // Where a's body overlaps b's header, the header wins.
        assert_eq!(classify_point(egui::pos2(160.0, 110.0), frames()), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(5.0, 5.0), std::iter::empty()), DragKind::Canvas);
    }
}
