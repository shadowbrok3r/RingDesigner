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
    /// Hosts with a dedicated inspector can keep node cards compact.
    pub inline_inputs: bool,
    style: SnarlStyle,
    /// A node to centre the view on at the next frame.
    pending_focus: Option<NodeId>,
    /// Fit every node into the view at the next frame.
    pending_fit: bool,
    /// A zoom the host asked for, applied about the view's own centre so the
    /// graph does not slide out from under the reader as the slider moves.
    pending_zoom: Option<f32>,
    /// The press in hand had two fingers down at some point. egui drives its pointer from the first
    /// finger alone, so lifting a pinch whose first finger barely moved reads as a tap — which chose
    /// whatever node was under it, and two quick pinches read as the double click that fits the graph.
    pinched: bool,
    /// The view's transform as of the last frame, for the minimap.
    transform: Option<egui::emath::TSTransform>,
    /// The persistent id the snarl was last shown under, for selection.
    snarl_id: Option<egui::Id>,
    pub show_minimap: bool,
    pub minimap_corner: egui::Align2,
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
    /// Arrange once measured, if the nodes overlap as placed.
    untangle_pending: bool,
    /// Last frame's measures: a node's first frame is narrower than its
    /// second, and a layout from measures still moving overlaps.
    last_sizes: HashMap<NodeId, egui::Vec2>,
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
/// The least a title bar may measure on screen and still be grabbed by a
/// finger, in points.
const MIN_GRAB_PT: f32 = 36.0;

/// The graph-space height that moves a node at this zoom: the title bar,
/// grown so it never falls under a fingertip on screen. Zoomed far out that
/// covers the whole node, which is right — nothing inside it can be read or
/// touched at that size, so all of it is the handle.
pub fn grab_height(scale: f32) -> f32 {
    NODE_HEADER_H.max(MIN_GRAB_PT / scale.max(0.01))
}
/// snarl's node frame margin: a node's stored position is its content
/// origin, the drawn frame starts this far above-left.
const FRAME_MARGIN: f32 = 6.0;

/// Which region a graph-space point falls on, over the drawn node frames.
/// Any header wins over any body, so a title-bar grab can always move a
/// node where nodes overlap.
pub fn classify_point(gp: egui::Pos2, frames: impl IntoIterator<Item = egui::Rect>, grab_h: f32) -> DragKind {
    let mut on_body = false;
    for frame in frames {
        if !frame.contains(gp) {
            continue;
        }
        let header = egui::Rect::from_min_size(frame.min, egui::vec2(frame.width(), grab_h.min(frame.height())));
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
            inline_inputs: true,
            style: crate::style::snarl_style(),
            pending_focus: None,
            pending_fit: false,
            pinched: false,
            transform: None,
            snarl_id: None,
            show_minimap: true,
            minimap_corner: egui::Align2::LEFT_TOP,
            sizes: HashMap::new(),
            arranged_sizes: HashMap::new(),
            refine_left: 0,
            layout_changed: false,
            untangle_pending: false,
            last_sizes: HashMap::new(),
            drag_kind: DragKind::None,
            pending_zoom: None,
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
            self.pinched = false;
        }
        if ctx.input(|i| i.multi_touch().is_some()) {
            self.pinched = true;
        }
        if pressed {
            self.drag_kind = match (origin, self.transform) {
                (Some(p), Some(t)) if viewport.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background) => {
                    classify_point(t.inverse() * p, self.node_frames(), grab_height(t.scaling))
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
        let mut first_values = false;
        for (sid, card) in self.snarl.nodes_ids_mut() {
            let was_empty = card.values.is_empty();
            card.values.clear();
            if let Some(v) = self.ids.to_graph.get(&sid).and_then(|gid| values.get(gid)) {
                first_values |= was_empty && !v.is_empty();
                card.values = v.clone();
            }
        }
        // The first evaluation adds badges that can widen generated cards.
        // Wait for their new measurements, then repair only actual overlaps.
        if first_values { self.arrange_if_tangled(); }
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

    /// The view's current zoom, 1.0 being a node at its drawn size.
    /// `None` until a frame has drawn and left its transform.
    pub fn zoom(&self) -> Option<f32> {
        self.transform.map(|t| t.scaling)
    }

    /// Zoom to this scale at the next frame, about the view's centre.
    pub fn set_zoom(&mut self, scale: f32) {
        self.pending_zoom = Some(scale.clamp(crate::style::MIN_SCALE, crate::style::MAX_SCALE));
    }

    /// The nodes in the order a reader steps through them: evaluation
    /// order, so a walk runs from the sources to the output.
    pub fn walk_order(&self) -> Vec<NodeId> {
        self.graph.topo().unwrap_or_else(|_| self.graph.nodes.iter().map(|n| n.id).collect())
    }

    /// Focus the node `delta` steps along [`Editor::walk_order`] from the
    /// chosen one, wrapping; with nothing chosen, the first (or last).
    pub fn step(&mut self, delta: isize) -> Option<NodeId> {
        let order = self.walk_order();
        if order.is_empty() {
            return None;
        }
        let n = order.len() as isize;
        let at = match self.selected.and_then(|s| order.iter().position(|id| *id == s)) {
            Some(i) => (i as isize + delta).rem_euclid(n),
            None if delta < 0 => n - 1,
            None => 0,
        };
        let id = order[at as usize];
        self.focus(id);
        Some(id)
    }

    /// Where the chosen node sits in the walk, one-based, and the walk's length.
    pub fn walk_position(&self) -> Option<(usize, usize)> {
        let order = self.walk_order();
        let at = order.iter().position(|id| Some(*id) == self.selected)?;
        Some((at + 1, order.len()))
    }

    /// The nodes wired into `id` and out of it, each with the pin on `id`
    /// the wire uses, in pin order.
    pub fn neighbours(&self, id: NodeId) -> (Vec<(String, NodeId)>, Vec<(String, NodeId)>) {
        let card = self.card(id);
        let pin_rank = |pins: Option<&Vec<PinSpec>>, name: &str| pins.and_then(|p| p.iter().position(|x| x.name == name)).unwrap_or(usize::MAX);
        let mut ins: Vec<(String, NodeId)> = self.graph.wires_into(id).map(|w| (w.input.clone(), w.from)).collect();
        ins.sort_by_key(|(pin, from)| (pin_rank(card.map(|c| &c.pins_in), pin), *from));
        let mut outs: Vec<(String, NodeId)> = self.graph.wires_from(id).map(|w| (w.out.clone(), w.to)).collect();
        outs.sort_by_key(|(pin, to)| (pin_rank(card.map(|c| &c.pins_out), pin), *to));
        (ins, outs)
    }

    /// A node's title as the canvas shows it.
    pub fn title_of(&self, id: NodeId) -> String {
        self.card(id).map(|c| c.title.clone()).or_else(|| self.graph.node(id).map(|n| n.kind.clone())).unwrap_or_default()
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
            zoom: self.pending_zoom.take(),
            rows: HashMap::new(),
            reg,
            editable: self.editable,
            inline_inputs: self.inline_inputs,
            clicked: None,
            refused: None,
            search: String::new(),
            ids: &self.ids,
            focus,
            fit,
            viewport,
            seen_transform: None,
            collapse_request: None,
            mode: self.graph.mode,
            selected: self.selected.and_then(|g| self.ids.to_snarl.get(&g).copied()),
            sizes: &mut self.sizes,
            pan,
            pinched: self.pinched,
        };
        // A double tap is not a way to ask for the whole graph on a touch screen: two quick pinches make
        // one. The Fit button does it there; a mouse keeps snarl's double click.
        self.style.centering = Some(!self.pinched && !ui.input(|i| i.has_touch_screen()));
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
        let settled = self.all_measured() && sizes_agree(&self.last_sizes, &self.sizes);
        self.last_sizes = self.sizes.clone();
        let mut untangled = false;
        if self.untangle_pending && settled {
            self.untangle_pending = false;
            if self.refine_left == 0 && self.tangled() {
                self.arrange(reg);
                self.layout_changed = true;
                untangled = true;
            }
        } else if self.untangle_pending {
            ui.ctx().request_repaint();
        }
        // The frame that laid out has not measured anything since.
        if !untangled && self.refine_left > 0 && self.all_measured() {
            if sizes_agree(&self.arranged_sizes, &self.sizes) {
                self.refine_left = 0;
                // Fit only after the final card sizes and column positions settle.
                self.pending_fit = true;
            } else {
                self.refine_left -= 1;
                self.lay_out(reg);
                self.pending_fit = true;
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

    /// The graph's exposed controls, independent of canvas zoom. Both
    /// desktop and touch hosts use the same widgets and edit path.
    pub fn parameters_ui(&mut self, reg: &Registry, ui: &mut Ui) -> bool {
        let exposed = self.graph.exposed.clone();
        let mut changed = false;
        egui::ScrollArea::vertical().id_salt("graph-parameters").max_height(220.0).show(ui, |ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 180.0).clamp(60.0, 140.0);
            egui::Grid::new("graph-parameter-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                for e in exposed {
                    let Some(node) = self.graph.node(e.node) else { continue };
                    let Some((pins, _)) = reg.node_pins(node) else { continue };
                    let Some(pin) = pins.into_iter().find(|p| p.name == e.input) else { continue };
                    let wired = self.graph.wire_into(e.node, &e.input).is_some();
                    let mut literal = node.inputs.get(&e.input).cloned();
                    ui.add(egui::Label::new(&e.name).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(if e.doc.is_empty() { &pin.doc } else { &e.doc });
                    let response = ui.push_id((e.node.0, &e.input), |ui| {
                        ui.add_enabled_ui(self.editable && !wired, |ui| pin_widget(ui, &pin, &mut literal)).inner
                    });
                    if wired {
                        response.response.on_hover_text("This input is driven by a wire; edit its source node.");
                    }
                    if response.inner {
                        changed |= self.set_input(e.node, &e.input, literal);
                    }
                    ui.end_row();
                }
            });
        });
        changed
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

    /// Arrange the graph once its nodes have been measured, but only if it
    /// needs it: a lifted or generated graph is placed on a nominal grid its
    /// real nodes overflow, while a graph someone has tidied is left alone.
    pub fn arrange_if_tangled(&mut self) {
        self.untangle_pending = true;
    }

    /// Whether the nodes overlap as placed, by their measured frames.
    fn tangled(&self) -> bool {
        let frames = self.node_frames();
        let overlapping = frames.iter().enumerate().filter(|(i, a)| frames.iter().enumerate().any(|(j, b)| *i != j && a.shrink(2.0).intersects(b.shrink(2.0)))).count();
        overlapping > (frames.len() / 8).max(1)
    }

    /// Lay the nodes out from their measured sizes — see [`layout_columns`]
    /// — so nothing overlaps. Nodes not yet drawn take the nominal footprint
    /// and a second pass follows once they have been.
    pub fn arrange(&mut self, reg: &Registry) {
        self.refine_left = 3;
        self.lay_out(reg);
        self.pending_fit = true;
    }

    fn lay_out(&mut self, reg: &Registry) {
        const FLOW_GAP: f32 = 60.0;
        const CROSS_GAP: f32 = 24.0;
        let Ok(order) = self.graph.topo() else { return };
        let pin_index = |id: NodeId, input: &str| {
            self.graph.node(id).and_then(|n| reg.node_pins(n)).and_then(|(pins, _)| pins.iter().position(|p| p.name == input)).unwrap_or(0)
        };
        let columns = layout_columns(&self.graph, &order, pin_index);
        let size = |id: NodeId| self.sizes.get(&id).copied().unwrap_or(NODE_SIZE);
        let mut x = 0.0f32;
        let mut pos: BTreeMap<NodeId, [f32; 2]> = BTreeMap::new();
        for column in &columns {
            // Top-aligned, so the spine of the flow runs straight along the
            // first row and each branch hangs under the place it joins.
            let mut y = 0.0f32;
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
    }
}

/// Below this a node cannot be read, and choosing one zooms in to it.
const FOCUS_MIN_SCALE: f32 = 0.45;

/// The view that puts `frame` (graph space) in front of the reader: wide
/// enough to read, the title in sight. Any readable zoom the node fits at is
/// kept — never zoomed out from — so stepping through nodes does not fight a
/// chosen zoom; a tall node is pinned by its title rather than centred on
/// rows off screen.
pub fn focus_transform(viewport: egui::Rect, frame: egui::Rect, current: f32) -> (f32, egui::Vec2) {
    const PAD: f32 = 28.0;
    let fit_w = (viewport.width() - 2.0 * PAD).max(60.0) / frame.width().max(1.0);
    let readable = fit_w.clamp(FOCUS_MIN_SCALE, 1.0);
    let scale = if current >= FOCUS_MIN_SCALE && frame.width() * current <= viewport.width() - PAD { current } else { readable };
    let x = viewport.center().x - frame.center().x * scale;
    let y = if frame.height() * scale + 2.0 * PAD <= viewport.height() { viewport.center().y - frame.center().y * scale } else { viewport.top() + PAD - frame.top() * scale };
    (scale, egui::vec2(x, y))
}

/// The columns of a left-to-right layout, each node one column before the
/// first node that consumes it. Sources therefore sit beside what they feed
/// — a layer, its window and its entry next to their place in the stack —
/// instead of every source piling into the first column while a chain runs
/// off to the right. One longest chain takes the first row of every column,
/// so the spine of the flow is a straight line and each branch hangs under
/// the place it joins; the rest follow their consumer's row and then its
/// input order, so wires run across rather than through each other.
pub fn layout_columns(g: &Graph, order: &[NodeId], pin_index: impl Fn(NodeId, &str) -> usize) -> Vec<Vec<NodeId>> {
    let mut tail: BTreeMap<NodeId, usize> = BTreeMap::new();
    for id in order.iter().rev() {
        let t = g.wires_from(*id).filter_map(|w| tail.get(&w.to)).max().map(|m| m + 1).unwrap_or(0);
        tail.insert(*id, t);
    }
    let deepest = tail.values().copied().max().unwrap_or(0);
    let mut head: BTreeMap<NodeId, usize> = BTreeMap::new();
    for id in order {
        let h = g.wires_into(*id).filter_map(|w| head.get(&w.from)).max().map(|m| m + 1).unwrap_or(0);
        head.insert(*id, h);
    }
    let mut columns: Vec<Vec<NodeId>> = vec![Vec::new(); deepest + 1];
    for id in order {
        let ends_flow = g.wires_from(*id).next().is_none();
        let is_sink = g.node(*id).is_some_and(|n| n.kind.starts_with("sink."));
        // A node nothing consumes is not the end of the flow unless it is a
        // sink: it stays where its own inputs put it.
        let col = if ends_flow && !is_sink { head[id].min(deepest) } else { deepest - tail[id] };
        columns[col].push(*id);
    }
    let mut spine: std::collections::BTreeSet<NodeId> = std::collections::BTreeSet::new();
    let mut at = order.iter().copied().filter(|id| tail[id] == 0 && head[id] == deepest).min_by_key(|id| !g.node(*id).is_some_and(|n| n.kind.starts_with("sink.")));
    while let Some(id) = at {
        spine.insert(id);
        at = g.wires_into(id).filter(|w| head[&w.from] + 1 == head[&id]).min_by_key(|w| pin_index(id, &w.input)).map(|w| w.from);
    }
    let rank: BTreeMap<NodeId, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut row: BTreeMap<NodeId, f32> = BTreeMap::new();
    for column in columns.iter_mut().rev() {
        let mut keyed: Vec<(NodeId, f32)> = column
            .iter()
            .map(|id| {
                let by_consumer = g.wires_from(*id).filter_map(|w| row.get(&w.to).map(|r| r + 0.01 * pin_index(w.to, &w.input) as f32)).fold(f32::INFINITY, f32::min);
                (*id, if spine.contains(id) { -1.0 } else { by_consumer })
            })
            .collect();
        keyed.sort_by(|a, b| a.1.total_cmp(&b.1).then(rank[&a.0].cmp(&rank[&b.0])));
        *column = keyed.iter().map(|(id, _)| *id).collect();
        for (i, id) in column.iter().enumerate() {
            row.insert(*id, i as f32);
        }
    }
    columns.retain(|c| !c.is_empty());
    columns
}

/// Two measure snapshots describe the same canvas: same nodes, none moved
/// by more than a unit.
fn sizes_agree(a: &HashMap<NodeId, egui::Vec2>, b: &HashMap<NodeId, egui::Vec2>) -> bool {
    a.len() == b.len() && a.iter().all(|(id, s)| b.get(id).is_some_and(|p| (*s - *p).abs().max_elem() <= 1.0))
}

struct Viewer<'a> {
    reg: &'a Registry,
    /// The gesture in hand had two fingers: nothing it does is a tap.
    pinched: bool,
    editable: bool,
    inline_inputs: bool,
    clicked: Option<SnarlId>,
    refused: Option<String>,
    search: String,
    ids: &'a IdMap,
    focus: Option<SnarlId>,
    fit: Option<egui::Rect>,
    viewport: egui::Rect,
    seen_transform: Option<egui::emath::TSTransform>,
    collapse_request: Option<SnarlId>,
    mode: ringdesign_graph::graph::Mode,
    /// The chosen node, for its rim.
    selected: Option<SnarlId>,
    /// Measured node sizes, filled as nodes are drawn.
    sizes: &'a mut HashMap<NodeId, egui::Vec2>,
    /// A zoom the host asked for, applied about the viewport's centre.
    zoom: Option<f32>,
    /// Each node's widest pin row, so its widgets share one right edge.
    /// Read off the specs alone, never back from the layout it decides.
    rows: HashMap<SnarlId, f32>,
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
    /// A small map of every node and the current view, in the host's corner.
    fn paint_minimap(&self, ui: &Ui, viewport: egui::Rect) {
        let Some(bounds) = node_bounds(&self.snarl, &self.sizes, &self.ids) else { return };
        if self.snarl.node_ids().count() < 2 || !viewport.is_finite() || viewport.width() < 160.0 || viewport.height() < 160.0 {
            return;
        }
        let pad = bounds.expand(60.0);
        let w = (viewport.width() * 0.30).clamp(96.0, 200.0);
        let h = (w * (pad.height() / pad.width().max(1.0)).clamp(0.35, 1.4)).clamp(60.0, 200.0);
        let map = self.minimap_corner.align_size_within_rect(egui::vec2(w, h), viewport.shrink(10.0));
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

/// The widest label-plus-widget a node's input rows need, capped at the field
/// width. Every widget of known width is then right-aligned to it.
fn row_width(ui: &Ui, card: &NodeCard) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    let font = egui::TextStyle::Body.resolve(ui.style());
    card.pins_in
        .iter()
        .filter_map(|spec| {
            let w = crate::widgets::widget_width(spec, card.inputs.get(&spec.name))?;
            let name = if spec.widget == ringdesign_graph::registry::Widget::Image { "Image" } else { &spec.name };
            let label = ui.painter().layout_no_wrap(name.to_owned(), font.clone(), crate::style::INK).size().x;
            Some(label + gap + w)
        })
        .fold(0.0_f32, f32::max)
        .min(crate::style::NODE_FIELD_W)
}

fn pin_info(pin: &PinSpec) -> PinInfo {
    crate::style::pin_info(pin.kind, pin.access)
}

impl SnarlViewer<NodeCard> for Viewer<'_> {
    fn title(&mut self, node: &NodeCard) -> String {
        node.title.clone()
    }

    fn show_header(&mut self, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        // Text selection steals title drags from snarl's header interaction.
        ui.add(egui::Label::new(&snarl[node].title).selectable(false).sense(egui::Sense::hover()));
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
        // The node's row width is a function of its own specs, so it is
        // measured once per node — on its first pin — and never read back
        // from the layout it decides.
        if pin.id.input == 0 {
            let width = row_width(ui, &snarl[pin.id.node]);
            self.rows.insert(pin.id.node, width);
        }
        let row = self.rows.get(&pin.id.node).copied().unwrap_or(0.0);
        let card = &mut snarl[pin.id.node];
        let Some(spec) = card.pins_in.get(pin.id.input).cloned() else { return PinInfo::circle() };
        ui.set_max_width(crate::style::NODE_FIELD_W);
        ui.horizontal(|ui| {
            let label = ui.label(RichText::new(if spec.widget == ringdesign_graph::registry::Widget::Image { "Image" } else { &spec.name }).color(crate::style::INK));
            let label_w = label.rect.width();
            label.on_hover_text(format!("{}\n{}", spec.kind.label(), spec.doc));
            if pin.remotes.is_empty() && self.editable && self.inline_inputs {
                // Temporarily move the literal; a PNG can contain megabytes.
                let mut lit = card.inputs.remove(&spec.name);
                // The label stays at the left; a widget of known width is
                // pushed out to the node's own right edge by a spacer, not by
                // a right-to-left layout — that one reverses a slider's own
                // parts and leaves its rail drawn back over the label.
                if let Some(w) = crate::widgets::widget_width(&spec, lit.as_ref()) {
                    ui.add_space((row - label_w - ui.spacing().item_spacing.x - w).max(0.0));
                }
                pin_widget(ui, &spec, &mut lit);
                if let Some(literal) = lit { card.inputs.insert(spec.name.clone(), literal); }
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
        if let Some(scale) = self.zoom.take() {
            // About the middle of the view: a slider that slid the graph
            // sideways as it zoomed would be unusable.
            let centre = self.viewport.center().to_vec2();
            let graph = (centre - to_global.translation) / to_global.scaling;
            to_global.scaling = scale;
            to_global.translation = centre - graph * scale;
        }
        if let Some(sid) = self.focus.take() {
            if let Some(info) = snarl.get_node_info(sid) {
                let size = self.ids.to_graph.get(&sid).and_then(|g| self.sizes.get(g)).copied().unwrap_or(NODE_SIZE);
                let frame = egui::Rect::from_min_size(info.pos - egui::vec2(FRAME_MARGIN, FRAME_MARGIN), size);
                // A focus centres the node. The zoom it arrives at stays the
                // reader's wherever the node is already readable there.
                let (scale, translation) = focus_transform(self.viewport, frame, to_global.scaling);
                to_global.scaling = scale;
                to_global.translation = translation;
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
        // `rect` is in graph space and the pointer is on the screen: the two
        // only agree at the identity view. Unmapped, a tap anywhere — on the
        // host's own buttons too — chose whichever node's graph rect happened
        // to hold that screen position.
        if let (Some(p), Some(t)) = (ui.input(|i| i.pointer.hover_pos()), self.seen_transform) {
            let on_canvas = self.viewport.contains(p) && ui.ctx().layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background);
            if on_canvas && rect.contains(t.inverse() * p) && !self.pinched {
                ui.painter().rect_stroke(rect, crate::style::NODE_CORNER, egui::Stroke::new(1.7 / t.scaling.max(0.1), crate::style::AQUA), egui::StrokeKind::Inside);
                if !ui.ctx().egui_is_using_pointer() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            }
        }
        let tap = ui.input(|i| if i.pointer.primary_clicked() && !self.pinched { i.pointer.interact_pos() } else { None });
        if let (Some(p), Some(t)) = (tap, self.seen_transform) {
            let on_canvas = self.viewport.contains(p) && ui.ctx().layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background);
            if on_canvas && rect.contains(t.inverse() * p) {
                self.clicked = Some(node);
            }
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
                    let mark = crate::marks::of(spec);
                    if crate::marks::button(ui, mark, &format!("{}  ({})", spec.label, spec.key)).on_hover_text(&spec.doc).clicked() {
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
            crate::marks::submenu(ui, *cat, |ui| {
                for spec in in_cat {
                    let mark = crate::marks::of(spec);
                    if crate::marks::button(ui, mark, &spec.label).on_hover_text(format!("{}\n{}", spec.key, spec.doc)).clicked() {
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
        let mut viewer = Viewer { reg: &reg, editable: true, inline_inputs: true, clicked: None, refused: None, search: String::new(), ids: &ids, focus: None, fit: None, viewport: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0)), seen_transform: None, collapse_request: None, mode: Mode::SandRing , selected: None, sizes: &mut HashMap::new(), rows: HashMap::new(), zoom: None, pan: egui::Vec2::ZERO , pinched: false };
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
        let h = grab_height(1.5);
        assert_eq!(h, NODE_HEADER_H, "zoomed in, the title bar is the handle");
        assert_eq!(classify_point(egui::pos2(20.0, 10.0), frames(), h), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), frames(), h), DragKind::Body);
        assert_eq!(classify_point(egui::pos2(400.0, 400.0), frames(), h), DragKind::Canvas);
        // Where a's body overlaps b's header, the header wins.
        assert_eq!(classify_point(egui::pos2(160.0, 110.0), frames(), h), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(5.0, 5.0), std::iter::empty(), h), DragKind::Canvas);
    }

    #[test]
    fn a_title_bar_never_shrinks_under_a_fingertip() {
        // At a third of full size a 30-unit bar is 10 points on screen; the
        // handle grows to stay 36, and far enough out it is the whole node.
        let a = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 120.0));
        for scale in [0.05_f32, 0.2, 0.33, 0.8, 1.0, 2.5] {
            assert!(grab_height(scale) * scale >= MIN_GRAB_PT.min(NODE_HEADER_H * scale).max(MIN_GRAB_PT - 0.01) - 0.01 || scale > 1.2, "{scale}");
            assert!(grab_height(scale) >= NODE_HEADER_H);
        }
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), [a], grab_height(0.33)), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(20.0, 119.0), [a], grab_height(0.2)), DragKind::Header, "the whole node moves it");
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), [a], grab_height(1.0)), DragKind::Body);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn arranged(name: &str) -> (Editor, Registry) {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph(name).unwrap_or_else(|| panic!("{name} is bundled"));
        let mut ed = Editor::new(g, &reg);
        ed.arrange(&reg);
        (ed, reg)
    }

    #[test]
    fn a_source_sits_one_column_before_what_it_feeds() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Braided band").expect("bundled");
        let order = g.topo().unwrap();
        let cols = layout_columns(&g, &order, |id, input| g.node(id).and_then(|n| reg.node_pins(n)).and_then(|(p, _)| p.iter().position(|x| x.name == input)).unwrap_or(0));
        let col_of = |id: NodeId| cols.iter().position(|c| c.contains(&id)).unwrap();
        assert_eq!(cols.iter().map(Vec::len).sum::<usize>(), g.nodes.len(), "every node is placed once");
        for w in &g.wires {
            assert!(col_of(w.from) < col_of(w.to), "{:?} flows left to right", w);
        }
        // As late as possible: a node with consumers is adjacent to the nearest one.
        for n in &g.nodes {
            if let Some(nearest) = g.wires_from(n.id).map(|w| col_of(w.to)).min() {
                assert_eq!(col_of(n.id) + 1, nearest, "{} sits beside its consumer", n.kind);
            }
        }
        let sink = g.nodes.iter().find(|n| n.kind == "sink.output").unwrap().id;
        assert_eq!(col_of(sink), cols.len() - 1);
    }

    #[test]
    fn a_long_stack_is_a_ribbon_not_a_wall() {
        // A lifted showcase graph is a chain of stacks with a layer, a
        // window and an entry per link. By depth from the sources every one
        // of those lands in the first columns; beside their consumers no
        // column holds more than a few.
        let (ed, _) = arranged("Aster Atelier");
        let mut by_x: BTreeMap<i64, usize> = BTreeMap::new();
        for n in &ed.graph().nodes {
            *by_x.entry(n.pos[0] as i64).or_default() += 1;
        }
        let tallest = by_x.values().copied().max().unwrap();
        assert!(tallest <= 12, "{tallest} nodes in one column of {}", ed.graph().nodes.len());
        let spine: Vec<&Node> = ed.graph().nodes.iter().filter(|n| n.kind == "stack").collect();
        assert!(spine.iter().all(|n| n.pos[1] == 0.0), "the stack chain runs straight along the first row");
    }

    #[test]
    fn a_grid_the_nodes_overflow_is_untangled_once_and_a_tidy_graph_is_left_alone() {
        let reg = Registry::builtin();
        let mut g = ringdesign_graph::templates::graph("Shouldered cushion signet").expect("bundled");
        // The lift's nominal grid: real nodes are several cells tall.
        for (i, n) in g.nodes.iter_mut().enumerate() {
            n.pos = [(i % 3) as f32 * 240.0, (i / 3) as f32 * 150.0];
        }
        let settle = |ed: Editor| {
            let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
                |ui, ed: &mut Editor| {
                    ed.show(&reg, ui, "untangle-editor");
                },
                ed,
            );
            harness.run_steps(8);
            harness
        };
        let mut ed = Editor::new(g, &reg);
        ed.arrange_if_tangled();
        let harness = settle(ed);
        let ed = harness.state();
        assert!(!ed.tangled(), "arranged on open");
        let tidy = ed.graph().clone();
        drop(harness);
        let mut ed = Editor::new(tidy.clone(), &reg);
        ed.arrange_if_tangled();
        let harness = settle(ed);
        assert_eq!(harness.state().graph(), &tidy, "a graph that does not overlap is not rearranged");
    }

    #[test]
    fn nothing_overlaps_after_arrange_settles() {
        let (mut ed, reg) = arranged("Shouldered cushion signet");
        for _ in 0..6 {
            let mut harness = egui_kittest::Harness::new_ui(|ui| {
                ed.show(&reg, ui, "arrange-editor");
            });
            harness.set_size(egui::vec2(1200.0, 800.0));
            harness.run();
        }
        assert!(ed.all_measured());
        let frames = ed.node_frames();
        for (i, a) in frames.iter().enumerate() {
            for b in &frames[i + 1..] {
                assert!(!a.shrink(1.0).intersects(b.shrink(1.0)), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn nodes_keep_their_width_inside_a_host_that_wraps() {
        // The phone's shell wraps every label; inside the editor that used
        // to fold a title to one letter per line and the node never recovered.
        let reg = Registry::builtin();
        let measure = |wrap: Option<egui::TextWrapMode>| {
            let mut ed = Editor::new(ringdesign_graph::templates::simple(), &reg);
            for _ in 0..4 {
                let mut harness = egui_kittest::Harness::new_ui(|ui| {
                    ui.style_mut().wrap_mode = wrap;
                    ed.show(&reg, ui, "wrapping-host");
                });
                harness.set_size(egui::vec2(420.0, 360.0));
                harness.run();
            }
            ed
        };
        let plain = measure(None);
        let wrapped = measure(Some(egui::TextWrapMode::Wrap));
        assert!(plain.all_measured() && wrapped.all_measured());
        for (id, size) in &plain.sizes {
            let got = wrapped.sizes[id];
            assert!((got - *size).abs().max_elem() <= 1.0, "{} is {size:?} alone and {got:?} in a host that wraps", plain.title_of(*id));
        }
    }

    #[test]
    fn a_tap_chooses_the_node_under_it_on_screen_not_in_graph_space() {
        let reg = Registry::builtin();
        let mut g = Graph::new("t", ringdesign_graph::graph::Mode::SandRing);
        let a = g.add("number").unwrap();
        let b = g.add("number").unwrap();
        g.node_mut(a).unwrap().pos = [40.0, 60.0];
        g.node_mut(b).unwrap().pos = [900.0, 700.0];
        let mut ed = Editor::new(g, &reg);
        // Put `b` in the middle of the view; `a` is now far off screen.
        ed.focus(b);
        let size = egui::vec2(800.0, 600.0);
        // One context throughout: the view's transform lives in its memory.
        let mut harness = egui_kittest::Harness::builder().with_size(size).build_ui_state(
            |ui, ed: &mut Editor| {
                ed.show(&reg, ui, "tap-editor");
            },
            ed,
        );
        harness.run();
        harness.state_mut().selected = None;
        let t = harness.state().transform.expect("a frame leaves its transform");
        let b_on_screen = t * (egui::pos2(900.0, 700.0) + egui::vec2(20.0, 10.0));
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains(b_on_screen));
        // `a`'s graph rect holds (60, 75); that *screen* point is empty canvas.
        let decoy = egui::pos2(60.0, 75.0);
        assert!((t * decoy - decoy).length() > 200.0, "the view is far from the identity");
        let tap = |harness: &mut egui_kittest::Harness<'_, Editor>, at: egui::Pos2| {
            harness.input_mut().events.push(egui::Event::PointerMoved(at));
            harness.input_mut().events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
            harness.step();
            harness.input_mut().events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
            harness.run();
        };
        tap(&mut harness, decoy);
        assert_eq!(harness.state().selected, None, "empty canvas chooses nothing");

        // A pinch whose first finger stays put, lifted on the node, as egui-winit reports it: the pointer
        // follows the first finger, so its release is a click — and it must not choose anything, or move
        // the view it just zoomed.
        let touch = |id: u64, phase: egui::TouchPhase, pos: egui::Pos2| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None };
        let other = b_on_screen + egui::vec2(90.0, 40.0);
        harness.input_mut().events.extend([
            touch(0, egui::TouchPhase::Start, b_on_screen),
            egui::Event::PointerMoved(b_on_screen),
            egui::Event::PointerButton { pos: b_on_screen, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            touch(1, egui::TouchPhase::Start, other),
        ]);
        harness.step();
        // One move: kittest steps a quarter second, and a click is a release inside 0.8 s of its press.
        harness.input_mut().events.push(touch(1, egui::TouchPhase::Move, other + egui::vec2(36.0, 24.0)));
        harness.step();
        let zoomed = harness.state().transform.unwrap();
        assert!(zoomed.scaling > t.scaling * 1.05, "the pinch zoomed in");
        assert!(harness.state().ids.to_snarl.get(&b).and_then(|sid| harness.state().snarl.get_node_info(*sid)).is_some_and(|info| {
            egui::Rect::from_min_size(info.pos, harness.state().sizes.get(&b).copied().unwrap_or(NODE_SIZE)).contains(zoomed.inverse() * b_on_screen)
        }), "the first finger is still on the node, so its release would choose it");
        let zoomed = zoomed.scaling;
        harness.input_mut().events.extend([
            touch(1, egui::TouchPhase::End, other + egui::vec2(36.0, 24.0)),
            touch(0, egui::TouchPhase::End, b_on_screen),
            egui::Event::PointerButton { pos: b_on_screen, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
            egui::Event::PointerGone,
        ]);
        harness.run();
        assert_eq!(harness.state().selected, None, "a pinch is not a tap");
        assert_eq!(harness.state().transform.unwrap().scaling, zoomed, "and lifting it leaves the zoom where it was");

        tap(&mut harness, b_on_screen);
        assert_eq!(harness.state().selected, Some(b));
    }

    #[test]
    fn stepping_walks_the_flow_and_focus_frames_the_node() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Braided band").expect("bundled");
        let mut ed = Editor::new(g, &reg);
        let order = ed.walk_order();
        assert_eq!(ed.step(1), Some(order[0]));
        assert_eq!(ed.step(1), Some(order[1]));
        assert_eq!(ed.walk_position(), Some((2, order.len())));
        assert_eq!(ed.step(-1), Some(order[0]));
        assert_eq!(ed.step(-1), Some(*order.last().unwrap()), "the walk wraps");
        let entry = ed.graph().entry_nodes()[0];
        let (ins, outs) = ed.neighbours(entry);
        assert!(ins.iter().any(|(pin, _)| pin == "layer") && ins.iter().any(|(pin, _)| pin == "window") || ins.iter().any(|(pin, _)| pin == "layer"));
        assert!(outs.iter().all(|(pin, to)| pin == "entry" && ed.graph().node(*to).is_some_and(|n| n.kind == "stack")));

        // A readable zoom is kept; an unreadable one is replaced; a tall node is pinned by its title.
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(400.0, 320.0));
        let node = egui::Rect::from_min_size(egui::pos2(1000.0, 500.0), egui::vec2(280.0, 160.0));
        let (scale, t) = focus_transform(view, node, 0.8);
        assert_eq!(scale, 0.8);
        let on_screen = (node.center().to_vec2() * scale + t).to_pos2();
        assert!((on_screen - view.center()).length() < 0.5, "{on_screen:?}");
        let (scale, _) = focus_transform(view, node, 0.06);
        assert!((0.45..=1.0).contains(&scale), "{scale}");
        // Zoomed in past the old 1.25 cap on a node that still fits: the zoom is the reader's.
        let small = egui::Rect::from_min_size(egui::pos2(1000.0, 500.0), egui::vec2(120.0, 60.0));
        assert_eq!(focus_transform(view, small, 2.2).0, 2.2);
        let tall = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(280.0, 900.0));
        let (scale, t) = focus_transform(view, tall, 1.0);
        let top = tall.top() * scale + t.y;
        assert!(top >= view.top() && top <= view.top() + 40.0, "the title is in sight: {top}");
    }
}
