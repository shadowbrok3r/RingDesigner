//! The editor widget: project the graph into a view, show it on a snarl, merge the edits back.
//!
//! [`Editor`] owns a [`Graph`] (the truth), the [`View`] of it being shown and
//! the `Snarl` that draws that view. The view folds collapsed groups into one
//! node each, or shows the inside of one entered group; edits made on the
//! snarl are extracted every frame and merged back into the graph.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use egui::{Color32, RichText, Ui};
use egui_snarl::ui::{AnyPins, BackgroundPattern, PinInfo, PinWireInfo, SnarlPin, SnarlStyle, SnarlViewer};
use egui_snarl::{InPin, InPinId, NodeId as SnarlId, OutPin, OutPinId, Snarl};
use ringdesign_graph::graph::{Access, Graph, GraphError, Node, NodeGroup, NodeId, Wire};
use ringdesign_graph::registry::{NodeSpec, PinSpec, Registry};
use ringdesign_graph::value::{Literal, ValueKind};

use crate::view::{self, Pseudo, View};
use crate::widgets::pin_widget;
mod commands;
mod gesture;
mod groups;
pub mod help;
mod overlay;
mod palette;

pub use gesture::{DragKind, classify_point, grab_height};
use gesture::Gesture;

/// A node as the view holds it: the graph's data plus the pins resolved
/// from the registry, so drawing needs no registry at all.
#[derive(Clone, Debug)]
pub struct NodeCard {
    /// The graph id; 0 until extraction assigns one.
    pub id: u64,
    pub kind: String,
    pub label: Option<String>,
    pub title: String,
    /// The kind's own name, shown when the node carries no label.
    pub spec_label: String,
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
        let spec_label = spec.map(|s| s.label.clone()).unwrap_or_else(|| node.kind.clone());
        Self {
            id: node.id.0,
            kind: node.kind.clone(),
            label: node.label.clone(),
            title: node.label.clone().unwrap_or_else(|| spec_label.clone()),
            spec_label,
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

    /// Whether this card is a view-only node: a collapsed group or a group's ports.
    pub fn is_pseudo(&self) -> bool {
        self.kind.starts_with('@')
    }

    fn to_node(&self, id: NodeId, pos: egui::Pos2) -> Node {
        Node { id, kind: self.kind.clone(), params: self.params.clone(), inputs: self.inputs.clone(), pos: [pos.x.round(), pos.y.round()], label: self.label.clone() }
    }

    /// The card with the graph node's own data over it.
    fn over(&self, n: &Node) -> Self {
        Self {
            id: n.id.0,
            label: n.label.clone(),
            title: n.label.clone().unwrap_or_else(|| self.spec_label.clone()),
            params: n.params.clone(),
            inputs: n.inputs.clone(),
            ..self.clone()
        }
    }
}

/// Graph ids to snarl ids and back, for one snarl.
#[derive(Clone, Debug, Default)]
pub struct IdMap {
    pub to_snarl: BTreeMap<NodeId, SnarlId>,
    pub to_graph: BTreeMap<SnarlId, NodeId>,
}

/// A snarl of `cards`, wired as `g` says, pins found by name.
fn snarl_of(g: &Graph, card: impl Fn(&Node) -> NodeCard) -> (Snarl<NodeCard>, IdMap) {
    let mut snarl = Snarl::new();
    let mut ids = IdMap::default();
    for n in &g.nodes {
        let sid = snarl.insert_node(egui::pos2(n.pos[0], n.pos[1]), card(n));
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

/// A snarl viewing `g` as it is, every node shown, pins resolved through `reg`.
pub fn build_snarl(g: &Graph, reg: &Registry) -> (Snarl<NodeCard>, IdMap) {
    snarl_of(g, |n| NodeCard::from_node(n, reg))
}

/// The graph the snarl shows, with `template` supplying everything that is
/// not a node or a wire (name, mode, exposures, the id counter). Nodes
/// without an id get fresh ones; exposures of nodes no longer present drop.
pub fn extract_graph(snarl: &Snarl<NodeCard>, template: &Graph) -> Graph {
    let mut g = Graph::new(&template.name, template.mode);
    g.next_id = template.next_id.max(template.nodes.iter().map(|n| n.id).filter(|id| !view::is_pseudo(*id)).map(|id| id.0 + 1).max().unwrap_or(1));
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
    let live: BTreeSet<NodeId> = g.nodes.iter().map(|n| n.id).collect();
    g.exposed = template.exposed.iter().filter(|e| live.contains(&e.node)).cloned().collect();
    g.outputs = template.outputs.iter().filter(|e| live.contains(&e.node)).cloned().collect();
    g.groups = template.groups.clone();
    for group in &mut g.groups {
        group.nodes.retain(|id| live.contains(id));
    }
    g.groups.retain(|group| !group.nodes.is_empty());
    g
}

/// Whether an output of `out` may be wired into `input`.
pub fn fits(out: ValueKind, input: &PinSpec) -> bool {
    input.kind.accepts(out) || input.kind == ValueKind::List
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

/// Every node's card, for the nodes the view does not show.
pub(crate) struct Store<'a>(&'a BTreeMap<NodeId, NodeCard>);

impl view::Cards for Store<'_> {
    fn pins(&self, id: NodeId) -> (Vec<PinSpec>, Vec<PinSpec>) {
        self.0.get(&id).map(|c| (c.pins_in.clone(), c.pins_out.clone())).unwrap_or_default()
    }
    fn title(&self, id: NodeId) -> String {
        self.0.get(&id).map(|c| c.title.clone()).unwrap_or_default()
    }
}

/// The editor: a graph, the view of it, its snarl, and the bookkeeping between them.
pub struct Editor {
    graph: Graph,
    view: View,
    /// Every graph node's card; the snarl's own cards are newer for the nodes it shows.
    store: BTreeMap<NodeId, NodeCard>,
    snarl: Snarl<NodeCard>,
    ids: IdMap,
    /// Moves whenever the graph changes through the editor or set_graph.
    pub revision: u64,
    pub selected: Option<NodeId>,
    pub editable: bool,
    /// Hosts with a dedicated inspector can keep node cards compact.
    pub inline_inputs: bool,
    style: SnarlStyle,
    /// The add-node popup opened on the canvas, if open.
    palette: Option<palette::State>,
    /// The add-node menu a dropped wire opens.
    wire_palette: palette::State,
    group_name: String,
    command_error: Option<String>,
    /// A node to centre the view on at the next frame.
    pending_focus: Option<NodeId>,
    /// Fit every node into the view at the next frame.
    pending_fit: bool,
    /// A zoom the host asked for, applied about the view's centre.
    pending_zoom: Option<f32>,
    /// The press in hand had two fingers down at some point; nothing it does is a tap.
    pinched: bool,
    /// The view's transform as of the last frame.
    transform: Option<egui::emath::TSTransform>,
    /// The persistent id the snarl was last shown under, for selection.
    snarl_id: Option<egui::Id>,
    pub show_minimap: bool,
    pub minimap_corner: egui::Align2,
    /// The minimap's screen rect and its graph-to-screen transform, as last drawn.
    minimap: Option<(egui::Rect, egui::emath::TSTransform)>,
    /// Every node's drawn size, by view id so a rebuilt snarl keeps them.
    sizes: HashMap<NodeId, egui::Vec2>,
    /// The sizes the last arrange laid out from, and how many correction passes it may still take.
    arranged_sizes: HashMap<NodeId, egui::Vec2>,
    refine_left: u8,
    /// The editor moved nodes itself this frame; reported as a change.
    layout_changed: bool,
    /// Arrange once measured, if the nodes overlap as placed.
    untangle_pending: bool,
    /// Arrange at the next frame: folded groups without a place of their own sit where their members happen to be.
    arrange_pending: bool,
    /// Last frame's measures.
    last_sizes: HashMap<NodeId, egui::Vec2>,
    /// The press in hand and what it is doing.
    gesture: Gesture,
    /// Every pin's rect as last drawn, in graph space.
    pin_rects: Vec<(PinRef, egui::Rect)>,
    /// Diagnostics and value summaries from the host, by graph node.
    diag: BTreeMap<NodeId, Vec<String>>,
    values: BTreeMap<NodeId, BTreeMap<String, String>>,
    /// The node kind whose help is open.
    help: Option<String>,
    /// The browse-all view of the help window.
    help_browse: help::Browse,
}

/// One pin of one snarl node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PinRef {
    pub node: SnarlId,
    pub input: bool,
    pub index: usize,
}

/// Graph-space height of a node's title bar, for header-only dragging.
const NODE_HEADER_H: f32 = 30.0;
/// snarl's node frame margin: a node's stored position is its content
/// origin, the drawn frame starts this far above-left.
const FRAME_MARGIN: f32 = 6.0;

impl Editor {
    pub fn new(graph: Graph, reg: &Registry) -> Self {
        let mut ed = Self {
            graph: Graph::default(),
            view: View::default(),
            store: BTreeMap::new(),
            snarl: Snarl::new(),
            ids: IdMap::default(),
            revision: 0,
            selected: None,
            editable: true,
            inline_inputs: true,
            style: crate::style::snarl_style(),
            palette: None,
            wire_palette: palette::State::default(),
            group_name: "New group".into(),
            command_error: None,
            pending_focus: None,
            pending_fit: false,
            pending_zoom: None,
            pinched: false,
            transform: None,
            snarl_id: None,
            show_minimap: true,
            minimap_corner: egui::Align2::LEFT_TOP,
            minimap: None,
            sizes: HashMap::new(),
            arranged_sizes: HashMap::new(),
            refine_left: 0,
            layout_changed: false,
            untangle_pending: false,
            arrange_pending: false,
            last_sizes: HashMap::new(),
            gesture: Gesture::default(),
            pin_rects: Vec::new(),
            diag: BTreeMap::new(),
            values: BTreeMap::new(),
            help: None,
            help_browse: help::Browse::default(),
        };
        ed.adopt(graph, reg);
        ed.revision = 0;
        ed
    }

    /// The drawn frames of every node, in graph space.
    fn node_frames(&self) -> Vec<egui::Rect> {
        self.snarl.nodes_pos_ids().map(|(sid, pos, _)| egui::Rect::from_min_size(pos - egui::vec2(FRAME_MARGIN, FRAME_MARGIN), self.size_of(sid))).collect()
    }

    /// A node's drawn size, or the nominal footprint before it has been drawn.
    fn size_of(&self, sid: SnarlId) -> egui::Vec2 {
        self.ids.to_graph.get(&sid).and_then(|g| self.sizes.get(g)).copied().unwrap_or(NODE_SIZE)
    }

    fn all_measured(&self) -> bool {
        self.view.graph.nodes.iter().all(|n| self.sizes.contains_key(&n.id))
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn snarl(&self) -> &Snarl<NodeCard> {
        &self.snarl
    }

    /// The view being shown: the graph with its collapsed groups folded, or one group's inside.
    pub fn view(&self) -> &View {
        &self.view
    }

    /// Replace the graph from outside (a file, history, MCP) and rebuild the view.
    pub fn set_graph(&mut self, graph: Graph, reg: &Registry) {
        self.adopt(graph, reg);
        if let Some(sel) = self.selected {
            if !self.graph.contains(sel) {
                self.selected = None;
            }
        }
    }

    fn adopt(&mut self, graph: Graph, reg: &Registry) {
        self.store = graph.nodes.iter().map(|n| (n.id, NodeCard::from_node(n, reg))).collect();
        self.graph = graph;
        self.apply_marks();
        self.rebuild();
        self.revision += 1;
        if self.graph.groups.iter().any(|g| g.collapsed && g.pos.is_none()) {
            self.arrange_pending = true;
        }
    }

    /// Re-project the graph and rebuild the snarl, keeping the group in hand open if it still exists.
    fn rebuild(&mut self) {
        for (sid, card) in self.snarl.nodes_ids_mut() {
            if let Some(gid) = self.ids.to_graph.get(&sid).copied().filter(|g| !view::is_pseudo(*g)) {
                if self.graph.contains(gid) {
                    self.store.insert(gid, card.clone());
                }
            }
        }
        self.store.retain(|id, _| self.graph.contains(*id));
        let entered = self.view.entered.filter(|i| !view::members(&self.graph, *i).is_empty());
        self.view = view::project(&self.graph, entered, &Store(&self.store));
        let (snarl, ids) = snarl_of(&self.view.graph, |n| self.view_card(n));
        self.snarl = snarl;
        self.ids = ids;
    }

    /// The card a view node draws as.
    fn view_card(&self, n: &Node) -> NodeCard {
        if view::is_pseudo(n.id) {
            return groups::pseudo_card(&self.view, &self.graph, n, &self.store);
        }
        match self.store.get(&n.id) {
            Some(c) => c.over(n),
            None => NodeCard { id: n.id.0, kind: n.kind.clone(), label: n.label.clone(), title: n.label.clone().unwrap_or_else(|| n.kind.clone()), spec_label: n.kind.clone(), params: n.params.clone(), inputs: n.inputs.clone(), pins_in: Vec::new(), pins_out: Vec::new(), doc: String::new(), diag: Vec::new(), values: BTreeMap::new() },
        }
    }

    /// Re-resolve pins (the registry changed) without losing the view.
    pub fn refresh(&mut self, reg: &Registry) {
        let graph = self.graph.clone();
        self.set_graph(graph, reg);
    }

    /// Attach diagnostics to the cards they name.
    pub fn set_diagnostics(&mut self, errors: &[GraphError], per_node: &BTreeMap<NodeId, Vec<String>>) {
        self.diag.clear();
        for e in errors {
            if let Some(n) = e.node {
                self.diag.entry(n).or_default().push(e.message.clone());
            }
        }
        for (n, lines) in per_node {
            self.diag.entry(*n).or_default().extend(lines.iter().cloned());
        }
        self.apply_marks();
    }

    /// Attach output value summaries for badges.
    pub fn set_values(&mut self, values: &BTreeMap<NodeId, BTreeMap<String, String>>) {
        let first_values = self.values.is_empty() && !values.is_empty();
        self.values = values.clone();
        self.apply_marks();
        // The first evaluation adds badges that can widen generated cards.
        // Wait for their new measurements, then repair only actual overlaps.
        if first_values {
            self.arrange_if_tangled();
        }
    }

    /// Diagnostics and values onto every card: the stored ones, the drawn ones, and each group's sum of its members.
    fn apply_marks(&mut self) {
        for (id, card) in self.store.iter_mut() {
            card.diag = self.diag.get(id).cloned().unwrap_or_default();
            card.values = self.values.get(id).cloned().unwrap_or_default();
        }
        for (sid, card) in self.snarl.nodes_ids_mut() {
            let Some(vid) = self.ids.to_graph.get(&sid).copied() else { continue };
            if view::is_pseudo(vid) {
                let (diag, values) = groups::pseudo_marks(&self.view, &self.graph, vid, card, &self.diag, &self.values, &self.store);
                card.diag = diag;
                card.values = values;
            } else {
                card.diag = self.diag.get(&vid).cloned().unwrap_or_default();
                card.values = self.values.get(&vid).cloned().unwrap_or_default();
            }
        }
    }

    /// The view node that shows graph node `id`: itself, or the collapsed group folding it.
    fn view_node_of(&self, id: NodeId) -> Option<NodeId> {
        if self.ids.to_snarl.contains_key(&id) {
            return Some(id);
        }
        if self.view.entered.is_some() {
            return None;
        }
        self.view.claimed.get(&id).map(|i| view::group_id(*i))
    }

    /// The graph node a view node stands for when it is chosen.
    fn graph_node_of(&self, vid: NodeId) -> Option<NodeId> {
        match self.view.pseudo.get(&vid) {
            None => Some(vid),
            Some(Pseudo::Group(i)) => view::representative(&self.graph, *i),
            Some(_) => None,
        }
    }

    /// Centre the view on a node at the next frame, and select it. A node folded
    /// into a collapsed group centres the group; one outside the group in hand leaves it.
    pub fn focus(&mut self, id: NodeId) {
        if !self.graph.contains(id) {
            return;
        }
        if self.view_node_of(id).is_none() && self.view.entered.is_some() {
            self.view.entered = None;
            self.rebuild();
        }
        self.pending_focus = Some(id);
        self.selected = Some(id);
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

    /// The nodes in the order a reader steps through them: what the view shows, in
    /// evaluation order, a collapsed group standing as its representative.
    pub fn walk_order(&self) -> Vec<NodeId> {
        let order = self.view.graph.topo().unwrap_or_else(|_| self.view.graph.nodes.iter().map(|n| n.id).collect());
        order.into_iter().filter_map(|vid| self.graph_node_of(vid)).collect()
    }

    /// The walk entry standing for `id`: itself, or its collapsed group's representative.
    fn walk_entry(&self, id: NodeId) -> Option<NodeId> {
        self.view_node_of(id).and_then(|vid| self.graph_node_of(vid))
    }

    /// Focus the node `delta` steps along [`Editor::walk_order`] from the
    /// chosen one, wrapping; with nothing chosen, the first (or last).
    pub fn step(&mut self, delta: isize) -> Option<NodeId> {
        let order = self.walk_order();
        if order.is_empty() {
            return None;
        }
        let n = order.len() as isize;
        let here = self.selected.and_then(|s| self.walk_entry(s));
        let at = match here.and_then(|s| order.iter().position(|id| *id == s)) {
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
        let here = self.selected.and_then(|s| self.walk_entry(s))?;
        let at = order.iter().position(|id| *id == here)?;
        Some((at + 1, order.len()))
    }

    /// The nodes wired into `id` and out of it as the view shows them, each with the pin on `id` the wire uses.
    pub fn neighbours(&self, id: NodeId) -> (Vec<(String, NodeId)>, Vec<(String, NodeId)>) {
        let Some(vid) = self.view_node_of(id) else { return Default::default() };
        let card = self.ids.to_snarl.get(&vid).and_then(|sid| self.snarl.get_node(*sid));
        let pin_rank = |pins: Option<&Vec<PinSpec>>, name: &str| pins.and_then(|p| p.iter().position(|x| x.name == name)).unwrap_or(usize::MAX);
        let g = &self.view.graph;
        let mut ins: Vec<(String, NodeId)> = g.wires_into(vid).filter_map(|w| Some((w.input.clone(), self.graph_node_of(w.from)?))).collect();
        ins.sort_by_key(|(pin, from)| (pin_rank(card.map(|c| &c.pins_in), pin), *from));
        let mut outs: Vec<(String, NodeId)> = g.wires_from(vid).filter_map(|w| Some((w.out.clone(), self.graph_node_of(w.to)?))).collect();
        outs.sort_by_key(|(pin, to)| (pin_rank(card.map(|c| &c.pins_out), pin), *to));
        (ins, outs)
    }

    /// A node's title as the canvas shows it; a node folded into a collapsed group answers with the group.
    pub fn title_of(&self, id: NodeId) -> String {
        if let Some(vid) = self.view_node_of(id).filter(|v| view::is_pseudo(*v)) {
            if let Some(Pseudo::Group(i)) = self.view.pseudo.get(&vid) {
                return format!("{} (group)", self.graph.groups[*i].name);
            }
        }
        self.card(id).map(|c| c.title.clone()).or_else(|| self.graph.node(id).map(|n| n.kind.clone())).unwrap_or_default()
    }

    /// Whether Escape means something here: leaving a group or closing the add-node popup.
    pub fn wants_escape(&self) -> bool {
        self.view.entered.is_some() || self.palette.is_some()
    }

    /// The group whose inside is shown, if any.
    pub fn entered_group(&self) -> Option<&NodeGroup> {
        self.view.entered.and_then(|i| self.graph.groups.get(i))
    }

    /// Show the inside of group `index`.
    pub fn enter_group(&mut self, index: usize) {
        if view::members(&self.graph, index).is_empty() {
            return;
        }
        self.view.entered = Some(index);
        self.rebuild();
        self.clear_selection();
        if self.sprawled() {
            self.arrange_pending = true;
        } else {
            self.arrange_if_tangled();
        }
    }

    /// Whether the view's nodes cover far more ground than a tidy layout of them would.
    fn sprawled(&self) -> bool {
        let g = &self.view.graph;
        let Ok(order) = g.topo() else { return false };
        let columns = layout_columns(g, &order, |_, _| 0);
        let tidy = egui::vec2(columns.len() as f32 * (NODE_SIZE.x + 60.0), columns.iter().map(Vec::len).max().unwrap_or(0) as f32 * (NODE_SIZE.y + 24.0));
        let mut spread = egui::Rect::NOTHING;
        for n in &g.nodes {
            spread = spread.union(egui::Rect::from_min_size(egui::pos2(n.pos[0], n.pos[1]), self.sizes.get(&n.id).copied().unwrap_or(NODE_SIZE)));
        }
        spread.is_positive() && spread.area() > 2.0 * tidy.x * tidy.y
    }

    /// Back to the whole graph.
    pub fn leave_group(&mut self) {
        let Some(index) = self.view.entered.take() else { return };
        self.rebuild();
        self.clear_selection();
        if self.graph.groups.get(index).is_some_and(|g| g.collapsed) {
            self.selected = view::representative(&self.graph, index);
        }
    }

    fn clear_selection(&mut self) {
        self.gesture.clear_selection = true;
    }

    /// The nodes the view has selected (rubber-band or click), as graph ids; a selected group gives its members.
    pub fn selected_nodes(&self, ctx: &egui::Context) -> Vec<NodeId> {
        let Some(id) = self.snarl_id else { return Vec::new() };
        let mut out = Vec::new();
        for sid in egui_snarl::ui::get_selected_nodes(id, ctx) {
            let Some(vid) = self.ids.to_graph.get(&sid).copied() else { continue };
            match self.view.pseudo.get(&vid) {
                None => out.push(vid),
                Some(Pseudo::Group(i)) => out.extend(view::members(&self.graph, *i)),
                Some(_) => {}
            }
        }
        out
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

    /// The snarl id salt for the view in hand: the inside of a group keeps its own camera.
    fn view_salt(&self, id_salt: &str) -> String {
        match self.view.entered {
            Some(i) => format!("{id_salt}/group-{i}"),
            None => id_salt.to_string(),
        }
    }

    /// Draw the editor and extract any change.
    pub fn show(&mut self, reg: &Registry, ui: &mut Ui, id_salt: &str) -> EditorResponse {
        let mut tools_changed = self.tools_ui(reg, ui);
        if std::mem::take(&mut self.arrange_pending) {
            self.arrange(reg);
            tools_changed = true;
        }
        let viewport = ui.available_rect_before_wrap();
        let snarl_id = ui.make_persistent_id(self.view_salt(id_salt));
        self.snarl_id = Some(snarl_id);
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.gesture.clear_selection) {
            egui_snarl::ui::set_selected_nodes(snarl_id, &ctx, std::iter::empty());
        }
        let frame = self.gesture_begin(&ctx, viewport);
        let focus = self.pending_focus.take().and_then(|id| self.view_node_of(id)).and_then(|vid| self.ids.to_snarl.get(&vid).copied());
        let fit = std::mem::take(&mut self.pending_fit).then_some(viewport);
        let saved: Option<Vec<(SnarlId, egui::Pos2)>> = frame.veto.then(|| self.snarl.nodes_pos_ids().map(|(id, pos, _)| (id, pos)).collect());
        let pins = Rc::new(RefCell::new(Vec::new()));
        let hint = self.wire_hint(&ctx);
        let selected_view = self.selected.and_then(|g| self.view_node_of(g)).and_then(|v| self.ids.to_snarl.get(&v).copied());
        let mut viewer = Viewer {
            theme_ctx: ctx.clone(),
            zoom: self.pending_zoom.take(),
            rows: HashMap::new(),
            reg,
            editable: self.editable,
            inline_inputs: self.inline_inputs,
            clicked: None,
            refused: None,
            palette: &mut self.wire_palette,
            ids: &self.ids,
            view: &self.view,
            groups: groups::frames(&self.graph, &self.view),
            frame_titles: Vec::new(),
            focus,
            fit,
            viewport,
            seen_transform: None,
            collapse_request: None,
            group_action: None,
            help_request: None,
            mode: self.graph.mode,
            selected: selected_view,
            sizes: &mut self.sizes,
            frame,
            pinched: self.pinched,
            pins: pins.clone(),
            hint,
            out_rows: Vec::new(),
        };
        // A double tap is not a way to ask for the whole graph on a touch screen: two quick pinches make
        // one. The Fit button does it there; a mouse keeps snarl's double click.
        self.style.centering = Some(!self.pinched && !ui.input(|i| i.has_touch_screen()));
        self.style.bg_frame = Some(egui::Frame::new().fill(ui.visuals().panel_fill));
        // That app's visuals on everything inside the editor, and nothing outside it.
        let background = ui
            .scope(|ui| {
                crate::style::apply_visuals(ui.style_mut());
                egui_snarl::ui::SnarlWidget::new().id(snarl_id).style(self.style).show(&mut self.snarl, &mut viewer, ui)
            })
            .inner;
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
        let group_action = viewer.group_action.take();
        let help_request = viewer.help_request.take();
        let frame_titles = std::mem::take(&mut viewer.frame_titles);
        self.transform = viewer.seen_transform.or(self.transform);
        drop(viewer);
        self.pin_rects = pins.take();
        let title_click = self.gesture_end(&ctx, &frame_titles);
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
            ctx.request_repaint();
        }
        // Layout passes settle on their own, not on the next input event.
        if self.refine_left > 0 || self.pending_fit {
            ctx.request_repaint();
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
        self.paint_overlays(ui, viewport);
        let mut changed = tools_changed | std::mem::take(&mut self.layout_changed);
        changed |= self.palette_ui(&ctx, reg, viewport, &background);
        if let Some(kind) = help_request {
            self.help = Some(kind);
        }
        self.help_ui(&ctx, reg);
        if let Some(action) = group_action.or(title_click.map(groups::Action::Collapse)) {
            changed |= self.apply_group_action(action, &ctx, reg);
        }
        if let Some(sid) = collapse_request {
            if let Some(gid) = self.ids.to_graph.get(&sid).copied().filter(|g| !view::is_pseudo(*g)) {
                let name = format!("Cluster {}", self.graph.nodes.iter().filter(|n| n.kind == "cluster").count() + 1);
                let _ = self.collapse_upstream(gid, &name, reg);
                return EditorResponse { changed: true, selected: self.selected, refused: None };
            }
        }
        let mut resp = EditorResponse { refused, changed, ..Default::default() };
        if let Some(sid) = clicked {
            if let Some(vid) = self.ids.to_graph.get(&sid).copied() {
                if let Some(g) = self.graph_node_of(vid) {
                    self.selected = Some(g);
                }
            }
        }
        if let Some(cleared) = self.gesture.cleared.take() {
            if cleared {
                self.selected = None;
            }
        }
        if let Some(only) = self.gesture.boxed.take() {
            self.selected = only;
        }
        resp.selected = self.selected;
        resp.changed |= self.sync();
        resp
    }

    /// Extract the snarl, merge any edit into the graph, and re-project if the view's shape moved.
    fn sync(&mut self) -> bool {
        let extracted = extract_graph(&self.snarl, &self.view.graph);
        if extracted == self.view.graph {
            return false;
        }
        let entered = self.entered_group().map(|g| g.name.clone());
        let merged = view::merge(&self.graph, &self.view, &extracted);
        // Fresh nodes got ids; give their cards the ids extraction chose.
        self.assign_fresh_ids(&extracted);
        for n in &merged.nodes {
            if !self.store.contains_key(&n.id) {
                if let Some(card) = self.ids.to_snarl.get(&n.id).and_then(|sid| self.snarl.get_node(*sid)) {
                    self.store.insert(n.id, card.clone());
                }
            }
        }
        let changed = merged != self.graph;
        self.graph = merged;
        if changed {
            self.revision += 1;
        }
        // The group in hand is gone once its last member is.
        if self.view.entered.is_some() && self.entered_group().map(|g| g.name.clone()) != entered {
            self.view.entered = None;
            self.rebuild();
            return changed;
        }
        let fresh = view::project(&self.graph, self.view.entered, &Store(&self.store));
        let mut expected = fresh.graph.clone();
        for n in &mut expected.nodes {
            if matches!(fresh.pseudo.get(&n.id), Some(Pseudo::Inputs | Pseudo::Outputs)) {
                if let Some(e) = extracted.node(n.id) {
                    n.pos = e.pos;
                }
            }
        }
        if expected == extracted {
            self.view = View { graph: expected, ..fresh };
        } else {
            self.rebuild();
        }
        changed
    }

    /// Cards inserted this frame carry id 0 in the snarl; give them the ids extraction chose.
    fn assign_fresh_ids(&mut self, extracted: &Graph) {
        let mut fresh: Vec<SnarlId> = self.snarl.nodes_ids_mut().filter(|(_, c)| c.id == 0).map(|(sid, _)| sid).collect();
        fresh.sort_by_key(|s| s.0);
        let known: BTreeSet<NodeId> = self.ids.to_snarl.keys().copied().collect();
        let unmapped: Vec<NodeId> = extracted.nodes.iter().map(|n| n.id).filter(|id| !known.contains(id)).collect();
        for (sid, gid) in fresh.into_iter().zip(unmapped) {
            if let Some(card) = self.snarl.get_node_mut(sid) {
                card.id = gid.0;
            }
            self.ids.to_snarl.insert(gid, sid);
            self.ids.to_graph.insert(sid, gid);
        }
        let live: BTreeSet<SnarlId> = self.snarl.node_ids().map(|(sid, _)| sid).collect();
        self.ids.to_graph.retain(|sid, _| live.contains(sid));
        let live_graph: BTreeSet<NodeId> = self.ids.to_graph.values().copied().collect();
        self.ids.to_snarl.retain(|g, _| live_graph.contains(g));
    }

    /// Insert a node of `kind` at a view position; it gets its id on the
    /// next extraction (or [`Editor::commit`]).
    pub fn insert(&mut self, kind: &str, pos: [f32; 2], reg: &Registry) -> bool {
        let Some(spec) = reg.get(kind) else { return false };
        self.snarl.insert_node(egui::pos2(pos[0], pos[1]), NodeCard::new_of(spec, reg));
        true
    }

    pub fn remove(&mut self, id: NodeId) -> bool {
        if !self.graph.contains(id) {
            return false;
        }
        match self.ids.to_snarl.get(&id).copied() {
            Some(sid) => {
                self.snarl.remove_node(sid);
            }
            None => {
                let _ = self.graph.remove(id);
                self.revision += 1;
                self.rebuild();
            }
        }
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.commit();
        true
    }

    /// Extract the view now and adopt any change; true if the graph moved.
    pub fn commit(&mut self) -> bool {
        self.sync()
    }

    /// A node's card: the drawn one where the view shows it, the stored one where it does not.
    pub fn card(&self, id: NodeId) -> Option<&NodeCard> {
        self.ids.to_snarl.get(&id).and_then(|sid| self.snarl.get_node(*sid)).or_else(|| self.store.get(&id))
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.graph.node(id)
    }

    /// Set (or clear) a literal on a node's input, through the view so the
    /// card and the graph agree.
    pub fn set_input(&mut self, id: NodeId, input: &str, value: Option<Literal>) -> bool {
        if let Some(&sid) = self.ids.to_snarl.get(&id) {
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
            return self.commit();
        }
        let Some(node) = self.graph.node_mut(id) else { return false };
        let before = node.inputs.get(input).cloned();
        if before == value {
            return false;
        }
        match &value {
            Some(v) => {
                node.inputs.insert(input.to_string(), v.clone());
            }
            None => {
                node.inputs.remove(input);
            }
        }
        if let Some(card) = self.store.get_mut(&id) {
            card.inputs = node.inputs.clone();
        }
        self.revision += 1;
        true
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
                    let Some(mut pin) = pins.into_iter().find(|p| p.name == e.input) else { continue };
                    if let Some([min, max]) = e.range.filter(|r| r[0].is_finite() && r[1].is_finite() && r[0] < r[1]) {
                        pin.widget = match pin.widget {
                            ringdesign_graph::registry::Widget::Mm { .. } => ringdesign_graph::registry::Widget::Mm { min, max },
                            _ => ringdesign_graph::registry::Widget::Slider { min, max },
                        };
                    }
                    let wired = self.graph.wire_into(e.node, &e.input).is_some();
                    let mut literal = node.inputs.get(&e.input).cloned();
                    let name = ui.add(egui::Label::new(&e.name).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(if e.doc.is_empty() { &pin.doc } else { &e.doc }).id;
                    let response = ui.push_id((e.node.0, &e.input), |ui| ui.label_inputs_by(name, |ui| ui.add_enabled_ui(self.editable && !wired, |ui| pin_widget(ui, &pin, &mut literal)).inner));
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
        let label = label.filter(|l| !l.trim().is_empty());
        if let Some(&sid) = self.ids.to_snarl.get(&id) {
            if let Some(card) = self.snarl.get_node_mut(sid) {
                card.title = label.clone().unwrap_or_else(|| card.spec_label.clone());
                card.label = label;
            }
            return self.commit();
        }
        let Some(node) = self.graph.node_mut(id) else { return false };
        if node.label == label {
            return false;
        }
        node.label = label.clone();
        if let Some(card) = self.store.get_mut(&id) {
            card.title = label.clone().unwrap_or_else(|| card.spec_label.clone());
            card.label = label;
        }
        self.revision += 1;
        true
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

    /// Lay the view's nodes out from their measured sizes — see [`layout_columns`]
    /// — so nothing overlaps. Nodes not yet drawn take the nominal footprint
    /// and a second pass follows once they have been.
    pub fn arrange(&mut self, reg: &Registry) {
        self.refine_left = 3;
        self.lay_out(reg);
        self.pending_fit = true;
    }

    fn lay_out(&mut self, _reg: &Registry) {
        const FLOW_GAP: f32 = 60.0;
        const CROSS_GAP: f32 = 24.0;
        self.sync();
        let g = &self.view.graph;
        let Ok(order) = g.topo() else { return };
        let pin_index = |id: NodeId, input: &str| self.ids.to_snarl.get(&id).and_then(|sid| self.snarl.get_node(*sid)).and_then(|c| c.pins_in.iter().position(|p| p.name == input)).unwrap_or(0);
        let columns = layout_columns(g, &order, pin_index);
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
        let mut edited = g.clone();
        for n in edited.nodes.iter_mut() {
            if let Some(p) = pos.get(&n.id) {
                n.pos = [p[0].round(), p[1].round()];
            }
        }
        let merged = view::merge(&self.graph, &self.view, &edited);
        self.arranged_sizes = self.sizes.clone();
        if merged != self.graph {
            self.graph = merged;
            self.revision += 1;
        }
        let ports: BTreeMap<NodeId, [f32; 2]> = edited.nodes.iter().filter(|n| matches!(self.view.pseudo.get(&n.id), Some(Pseudo::Inputs | Pseudo::Outputs))).map(|n| (n.id, n.pos)).collect();
        self.rebuild();
        for (vid, p) in ports {
            if let Some(info) = self.ids.to_snarl.get(&vid).copied().and_then(|sid| self.snarl.get_node_info_mut(sid)) {
                info.pos = egui::pos2(p[0], p[1]);
            }
            if let Some(n) = self.view.graph.node_mut(vid) {
                n.pos = p;
            }
        }
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
        let is_sink = g.node(*id).is_some_and(|n| n.kind.starts_with("sink.") || n.kind == view::OUTPUTS_KIND);
        // A node nothing consumes is not the end of the flow unless it is a
        // sink: it stays where its own inputs put it.
        let col = if ends_flow && !is_sink { head[id].min(deepest) } else { deepest - tail[id] };
        columns[col].push(*id);
    }
    let mut spine: BTreeSet<NodeId> = BTreeSet::new();
    let sinkish = |id: &NodeId| g.node(*id).is_some_and(|n| n.kind.starts_with("sink.") || n.kind == view::OUTPUTS_KIND);
    let mut at = order.iter().copied().filter(|id| tail[id] == 0 && head[id] == deepest).min_by_key(|id| !sinkish(id));
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

/// A wire being dragged from a pin: what it carries and which way it runs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WireHint {
    pub from: PinRef,
    /// The kind the dragged end needs a partner for: the output's kind, or the input's.
    pub kind: ValueKind,
    pub spec_access: Access,
}

/// How a pin reads while a wire is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Plain,
    Fits,
    Clashes,
    Source,
}

/// A pin that records where it was drawn and shows whether a dragged wire fits it.
struct Pin {
    info: PinInfo,
    at: PinRef,
    mark: Mark,
    rec: Rc<RefCell<Vec<(PinRef, egui::Rect)>>>,
}

impl SnarlPin for Pin {
    fn draw(self, snarl_style: &SnarlStyle, style: &egui::Style, rect: egui::Rect, painter: &egui::Painter) -> PinWireInfo {
        self.rec.borrow_mut().push((self.at, rect));
        let fill = self.info.fill.unwrap_or(Color32::GRAY);
        match self.mark {
            Mark::Plain => self.info.draw(snarl_style, style, rect, painter),
            Mark::Fits | Mark::Source => {
                let ring = if self.mark == Mark::Fits { crate::style::AQUA } else { crate::style::PINK };
                painter.circle_stroke(rect.center(), rect.width() * 0.5 + 4.0, egui::Stroke::new(2.0, ring));
                self.info.draw(snarl_style, style, rect.scale_from_center(1.15), painter)
            }
            Mark::Clashes => {
                let faded = self.info.with_fill(fill.gamma_multiply(0.25)).with_wire_color(fill);
                faded.draw(snarl_style, style, rect.scale_from_center(0.8), painter)
            }
        }
    }
}

struct Viewer<'a> {
    theme_ctx: egui::Context,
    reg: &'a Registry,
    /// The gesture in hand had two fingers: nothing it does is a tap.
    pinched: bool,
    editable: bool,
    inline_inputs: bool,
    clicked: Option<SnarlId>,
    refused: Option<String>,
    /// The dropped-wire menu's search.
    palette: &'a mut palette::State,
    ids: &'a IdMap,
    view: &'a View,
    /// Expanded group frames to draw, by group index, members as the view shows them.
    groups: Vec<(usize, NodeGroup)>,
    /// Each drawn frame's title strip, graph space, by group index.
    frame_titles: Vec<(usize, egui::Rect)>,
    focus: Option<SnarlId>,
    fit: Option<egui::Rect>,
    viewport: egui::Rect,
    seen_transform: Option<egui::emath::TSTransform>,
    collapse_request: Option<SnarlId>,
    group_action: Option<groups::Action>,
    help_request: Option<String>,
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
    /// What the gesture in hand does to the view this frame.
    frame: gesture::Frame,
    pins: Rc<RefCell<Vec<(PinRef, egui::Rect)>>>,
    hint: Option<WireHint>,
    /// Each output row as laid out, for its name to be painted at the node's left edge.
    out_rows: Vec<OutRow>,
}

/// One output's row: where it landed, and the name painted along it.
struct OutRow {
    node: SnarlId,
    row: egui::Rect,
    name: String,
    tint: Color32,
    tip: String,
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

fn label_width(ui: &Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, crate::style::INK).size().x
}

/// The font an output's name is drawn in: what the node gives, a little larger and bold.
fn output_font(ui: &Ui) -> egui::FontId {
    let base = egui::TextStyle::Body.resolve(ui.style());
    egui::FontId::new(base.size + 1.5, base.family)
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
            Some(label_width(ui, name, font.clone()) + gap + w)
        })
        .fold(0.0_f32, f32::max)
        .min(crate::style::NODE_FIELD_W)
}

fn pin_info(pin: &PinSpec) -> PinInfo {
    crate::style::pin_info(pin.kind, pin.access)
}

/// How a pin tells what it takes or gives, in words.
pub fn pin_tip(spec: &PinSpec, input: bool) -> String {
    let many = if spec.access == Access::List { "a list of" } else { "one" };
    let kind = spec.kind.label();
    let mut tip = format!("{} — {many} {kind}", spec.name);
    if !spec.doc.is_empty() {
        tip.push('\n');
        tip.push_str(&spec.doc);
    }
    if let Some(about) = ringdesign_graph::guide::kind(kind) {
        tip.push_str("\n\n");
        tip.push_str(about.summary);
    }
    tip.push_str(if input { "\n\nDrag from this pin to an output that gives it, or to empty space to add one." } else { "\n\nDrag from this pin to an input that takes it, or to empty space to add one." });
    tip
}

impl Viewer<'_> {
    fn mark(&self, node: SnarlId, input: bool, index: usize, spec: &PinSpec) -> Mark {
        let Some(h) = self.hint else { return Mark::Plain };
        if h.from == (PinRef { node, input, index }) {
            return Mark::Source;
        }
        if h.from.input == input || h.from.node == node {
            return Mark::Clashes;
        }
        let ok = if input { fits(h.kind, spec) } else { fits(spec.kind, &PinSpec { kind: h.kind, access: h.spec_access, ..PinSpec::item("", h.kind) }) };
        if ok { Mark::Fits } else { Mark::Clashes }
    }

    fn pin(&self, spec: &PinSpec, node: SnarlId, input: bool, index: usize) -> Pin {
        Pin { info: pin_info(spec), at: PinRef { node, input, index }, mark: self.mark(node, input, index, spec), rec: self.pins.clone() }
    }

    fn blank_pin(&self, node: SnarlId, input: bool, index: usize) -> Pin {
        Pin { info: PinInfo::circle(), at: PinRef { node, input, index }, mark: Mark::Plain, rec: self.pins.clone() }
    }

    fn pseudo(&self, node: SnarlId) -> Option<Pseudo> {
        self.ids.to_graph.get(&node).and_then(|vid| self.view.pseudo.get(vid)).copied()
    }
}

impl SnarlViewer<NodeCard> for Viewer<'_> {
    fn title(&mut self, node: &NodeCard) -> String {
        node.title.clone()
    }

    fn show_header(&mut self, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        let card = &snarl[node];
        match self.pseudo(node) {
            Some(Pseudo::Group(i)) => {
                if let Some(a) = groups::header(ui, card, i) {
                    self.group_action = Some(a);
                }
            }
            Some(_) => {
                ui.add(egui::Label::new(RichText::new(&card.title).italics()).selectable(false).sense(egui::Sense::hover()));
            }
            None => {
                // Text selection steals title drags from snarl's header interaction.
                let title = ui.add(egui::Label::new(&card.title).selectable(false).sense(egui::Sense::hover()));
                let summary = ringdesign_graph::guide::node(&card.kind).map(|g| g.summary.to_string()).unwrap_or_else(|| card.doc.clone());
                let kind = card.spec_label.clone();
                title.on_hover_ui(|ui| {
                    ui.set_max_width(320.0);
                    ui.label(RichText::new(&kind).strong());
                    if !summary.is_empty() {
                        ui.label(&summary);
                    }
                    ui.weak("Press F1 or right-click for this node's help.");
                });
                let help = ui.add(egui::Button::new(RichText::new("?").small()).small().frame(false)).on_hover_text("What this node does");
                if help.clicked() {
                    self.help_request = Some(snarl[node].kind.clone());
                }
            }
        }
    }

    fn node_frame(&mut self, default: egui::Frame, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], snarl: &Snarl<NodeCard>) -> egui::Frame {
        let trouble = snarl.get_node(node).is_some_and(|c| !c.diag.is_empty());
        let ctx = &self.theme_ctx;
        let visuals = ctx.global_style();
        let chosen = self.selected == Some(node);
        let boxed = self.frame.boxed.is_some_and(|b| snarl.get_node_info(node).is_some_and(|info| {
            let size = self.ids.to_graph.get(&node).and_then(|g| self.sizes.get(g)).copied().unwrap_or(NODE_SIZE);
            b.intersects(egui::Rect::from_min_size(info.pos, size))
        }));
        let color = if trouble {
            egui_mobile_theme::color(ctx, egui_mobile_theme::Role::Danger)
        } else if chosen || boxed {
            egui_mobile_theme::color(ctx, egui_mobile_theme::Role::Accent)
        } else {
            visuals.visuals.window_stroke.color
        };
        let frame = default.fill(visuals.visuals.window_fill).corner_radius(visuals.visuals.window_corner_radius).stroke(egui::Stroke::new(if trouble || chosen || boxed { 2. } else { 1. }, color));
        match self.pseudo(node) {
            Some(Pseudo::Group(_)) => groups::group_frame(frame, ctx, trouble || chosen || boxed),
            Some(_) => frame.fill(crate::style::AQUA.gamma_multiply(0.10)),
            None => frame,
        }
    }

    fn draw_background(&mut self, _background: Option<&BackgroundPattern>, viewport: &egui::Rect, _snarl_style: &SnarlStyle, _style: &egui::Style, painter: &egui::Painter, snarl: &Snarl<NodeCard>) {
        let scale = self.seen_transform.map(|t| t.scaling).unwrap_or(1.0);
        crate::style::paint_canvas(painter, *viewport, scale);
        for (index, group) in &self.groups {
            let mut bounds: Option<egui::Rect> = None;
            for id in &group.nodes {
                let Some(info) = self.ids.to_snarl.get(id).and_then(|sid| snarl.get_node_info(*sid)) else { continue };
                let rect = egui::Rect::from_min_size(info.pos, self.sizes.get(id).copied().unwrap_or(NODE_SIZE));
                bounds = Some(bounds.map_or(rect, |r| r.union(rect)));
            }
            if let Some(mut rect) = bounds.map(|r| r.expand(16.0)) {
                rect.min.y -= 26.0;
                painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(80, 145, 160, 18));
                painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, crate::style::AQUA.gamma_multiply(0.5)), egui::StrokeKind::Inside);
                let title = format!("{}   (click to fold)", group.name);
                let text = painter.text(rect.min + egui::vec2(10.0, 6.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(14.0), crate::style::AQUA);
                self.frame_titles.push((*index, text.expand(4.0)));
            }
        }
    }

    fn inputs(&mut self, node: &NodeCard) -> usize {
        node.pins_in.len()
    }

    fn outputs(&mut self, node: &NodeCard) -> usize {
        node.pins_out.len()
    }

    fn has_body(&mut self, node: &NodeCard) -> bool {
        node.kind == view::GROUP_KIND
    }

    fn show_body(&mut self, node: SnarlId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        if let Some(Pseudo::Group(i)) = self.pseudo(node) {
            if let Some(a) = groups::body(ui, &snarl[node], i) {
                self.group_action = Some(a);
            }
        }
    }

    fn show_input(&mut self, pin: &InPin, ui: &mut Ui, snarl: &mut Snarl<NodeCard>) -> impl SnarlPin + 'static {
        // The node's row width is a function of its own specs, so it is
        // measured once per node — on its first pin — and never read back
        // from the layout it decides.
        if pin.id.input == 0 {
            let width = row_width(ui, &snarl[pin.id.node]);
            self.rows.insert(pin.id.node, width);
        }
        let row = self.rows.get(&pin.id.node).copied().unwrap_or(0.0);
        let pseudo = self.pseudo(pin.id.node).is_some();
        let card = &mut snarl[pin.id.node];
        let Some(spec) = card.pins_in.get(pin.id.input).cloned() else { return self.blank_pin(pin.id.node, true, pin.id.input) };
        let mark = self.mark(pin.id.node, true, pin.id.input, &spec);
        ui.set_max_width(crate::style::NODE_FIELD_W);
        ui.horizontal(|ui| {
            let ink = if mark == Mark::Clashes { crate::style::INK_DIM.gamma_multiply(0.6) } else { crate::style::INK };
            let label = ui.label(RichText::new(if spec.widget == ringdesign_graph::registry::Widget::Image { "Image" } else { &spec.name }).color(ink));
            let label_w = label.rect.width();
            let label_id = label.id;
            label.on_hover_text(pin_tip(&spec, true));
            if pin.remotes.is_empty() && self.editable && self.inline_inputs && !pseudo {
                // Temporarily move the literal; a PNG can contain megabytes.
                let mut lit = card.inputs.remove(&spec.name);
                // The label stays at the left; a widget of known width is
                // pushed out to the node's own right edge by a spacer, not by
                // a right-to-left layout — that one reverses a slider's own
                // parts and leaves its rail drawn back over the label.
                if let Some(w) = crate::widgets::widget_width(&spec, lit.as_ref()) {
                    ui.add_space((row - label_w - ui.spacing().item_spacing.x - w).max(0.0));
                }
                ui.label_inputs_by(label_id, |ui| pin_widget(ui, &spec, &mut lit));
                if let Some(literal) = lit {
                    card.inputs.insert(spec.name.clone(), literal);
                }
            }
        });
        self.pin(&spec, pin.id.node, true, pin.id.input)
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut Ui, snarl: &mut Snarl<NodeCard>) -> impl SnarlPin + 'static {
        let card = &snarl[pin.id.node];
        let Some(spec) = card.pins_out.get(pin.id.output).cloned() else { return self.blank_pin(pin.id.node, false, pin.id.output) };
        let mark = self.mark(pin.id.node, false, pin.id.output, &spec);
        let value = card.values.get(&spec.name).cloned();
        let font = output_font(ui);
        let name_w = label_width(ui, &spec.name, font.clone());
        // The name's width is held beside the pin; final_node_rect paints the name at the node's left edge.
        let row = ui
            .horizontal(|ui| {
                if let Some(v) = value {
                    ui.label(RichText::new(v).small().color(crate::style::INK_DIM));
                }
                ui.add_space(name_w);
            })
            .response
            .rect;
        let tint = if mark == Mark::Clashes { crate::style::INK_DIM.gamma_multiply(0.6) } else { crate::widgets::kind_color(spec.kind) };
        self.out_rows.push(OutRow { node: pin.id.node, row, name: spec.name.clone(), tint, tip: pin_tip(&spec, false) });
        self.pin(&spec, pin.id.node, false, pin.id.output)
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
        if let Some(t) = self.frame.freeze {
            *to_global = t;
        }
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
        if self.frame.pan != egui::Vec2::ZERO {
            to_global.translation += self.frame.pan;
        }
        if let Some((factor, at)) = self.frame.wheel {
            let scale = (to_global.scaling * factor).clamp(crate::style::MIN_SCALE, crate::style::MAX_SCALE);
            let k = scale / to_global.scaling;
            to_global.translation = at.to_vec2() + (to_global.translation - at.to_vec2()) * k;
            to_global.scaling = scale;
        }
        if let Some(centre) = self.frame.centre {
            to_global.translation = self.viewport.center().to_vec2() - centre.to_vec2() * to_global.scaling;
        }
        self.seen_transform = Some(*to_global);
    }

    fn final_node_rect(&mut self, node: SnarlId, rect: egui::Rect, ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        if let Some(gid) = self.ids.to_graph.get(&node) {
            // Clamped so one pathological measure cannot throw the layout.
            self.sizes.insert(*gid, rect.size().min(egui::vec2(1600.0, 3000.0)));
        }
        // What the node gives reads from its left edge, bold and in its kind's colour.
        let font = output_font(ui);
        for row in self.out_rows.extract_if(.., |r| r.node == node) {
            let at = egui::pos2(rect.left() + FRAME_MARGIN + 4.0, row.row.center().y);
            let text = ui.painter().text(at, egui::Align2::LEFT_CENTER, &row.name, font.clone(), row.tint);
            ui.painter().text(at + egui::vec2(0.4, 0.0), egui::Align2::LEFT_CENTER, &row.name, font.clone(), row.tint);
            ui.interact(text, ui.scope_id().with(("output-name", node, row.name.as_str())), egui::Sense::hover()).on_hover_text(row.tip);
        }
        // `rect` is in graph space and the pointer is on the screen: the two
        // only agree at the identity view. Unmapped, a tap anywhere — on the
        // host's own buttons too — chose whichever node's graph rect happened
        // to hold that screen position.
        if let (Some(p), Some(t)) = (ui.input(|i| i.pointer.hover_pos()), self.seen_transform) {
            let on_canvas = self.viewport.contains(p) && ui.ctx().layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background);
            if on_canvas && rect.contains(t.inverse() * p) && !self.pinched {
                ui.painter().rect_stroke(rect, crate::style::NODE_CORNER, egui::Stroke::new(1.7 / t.scaling.max(0.1), crate::style::AQUA), egui::StrokeKind::Inside);
                if !ui.ctx().egui_is_using_pointer() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
                    if let Some(Pseudo::Group(i)) = self.pseudo(node) {
                        self.group_action = Some(groups::Action::Enter(i));
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::F1)) && self.pseudo(node).is_none() {
                    self.help_request = Some(snarl[node].kind.clone());
                }
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
        if !fits(out_kind, &in_pin) {
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
        // The editor draws its own add-node popup, so typing can open it too.
        false
    }

    fn has_dropped_wire_menu(&mut self, pins: AnyPins<'_>, _: &mut Snarl<NodeCard>) -> bool {
        self.editable && match pins { AnyPins::In(p) => p.len() == 1, AnyPins::Out(p) => p.len() == 1 }
    }

    fn show_dropped_wire_menu(&mut self, pos: egui::Pos2, ui: &mut Ui, pins: AnyPins<'_>, snarl: &mut Snarl<NodeCard>) {
        self.wire_menu(pos, ui, snarl, &pins);
    }

    fn has_node_menu(&mut self, _node: &NodeCard) -> bool {
        self.editable
    }

    fn show_node_menu(&mut self, node: SnarlId, inputs: &[InPin], outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<NodeCard>) {
        match self.pseudo(node) {
            Some(Pseudo::Group(i)) => {
                if let Some(a) = groups::menu(ui, &snarl[node], i) {
                    self.group_action = Some(a);
                }
                return;
            }
            Some(_) => {
                ui.weak("The wires that cross into or out of this group.");
                return;
            }
            None => {}
        }
        if ui.button("What does this node do?").clicked() {
            self.help_request = Some(snarl[node].kind.clone());
            ui.close();
            return;
        }
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

    /// A template with its groups left open, so every node is drawn.
    pub(crate) fn flat(name: &str) -> Graph {
        let mut g = ringdesign_graph::templates::graph(name).unwrap_or_else(|| panic!("{name} is bundled"));
        g.groups.clear();
        g
    }

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
    fn a_folded_template_round_trips_through_its_view_untouched() {
        let reg = Registry::builtin();
        for (name, g) in ringdesign_graph::templates::all() {
            let mut ed = Editor::new(g.clone(), &reg);
            assert!(!ed.commit(), "{name}: drawing the view invents an edit");
            assert_eq!(ed.graph(), &g, "{name}");
            if !g.groups.is_empty() {
                assert!(ed.view().graph.nodes.len() < g.nodes.len(), "{name}: the groups fold");
            }
        }
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
        let view = View { graph: g.clone(), ..View::default() };
        let mut palette = palette::State::default();
        let mut viewer = Viewer {
            theme_ctx: egui::Context::default(),
            reg: &reg,
            editable: true,
            inline_inputs: true,
            clicked: None,
            refused: None,
            palette: &mut palette,
            ids: &ids,
            view: &view,
            groups: Vec::new(),
            frame_titles: Vec::new(),
            focus: None,
            fit: None,
            viewport: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0)),
            seen_transform: None,
            collapse_request: None,
            group_action: None,
            help_request: None,
            mode: Mode::SandRing,
            selected: None,
            sizes: &mut HashMap::new(),
            rows: HashMap::new(),
            zoom: None,
            frame: gesture::Frame::default(),
            pinched: false,
            pins: Rc::default(),
            hint: None,
            out_rows: Vec::new(),
        };
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
    fn a_folded_node_edits_through_the_graph_and_shows_its_group() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Gypsy trio").unwrap();
        let mut ed = Editor::new(g, &reg);
        let seat = ed.graph().nodes.iter().find(|n| n.kind == "layer.seat").unwrap().id;
        assert!(ed.view().hidden.contains(&seat), "the seat is folded into its layer's group");
        assert!(ed.card(seat).is_some_and(|c| c.kind == "layer.seat"), "a folded node still has its card");
        assert!(ed.set_input(seat, "diameter_mm", Some(Literal::Number(6.1))));
        assert_eq!(ed.graph().node(seat).unwrap().inputs["diameter_mm"], Literal::Number(6.1));
        assert_eq!(ed.card(seat).unwrap().inputs["diameter_mm"], Literal::Number(6.1));
        assert_eq!(ed.title_of(seat), "Centre (group)");
        assert_eq!(ed.walk_order().len(), ed.view().graph.nodes.len());
    }

    #[test]
    fn entering_a_group_shows_its_inside_and_leaving_restores_the_folded_view() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Gypsy trio").unwrap();
        let mut ed = Editor::new(g.clone(), &reg);
        let top = ed.view().graph.nodes.len();
        ed.enter_group(0);
        assert_eq!(ed.entered_group().map(|g| g.name.as_str()), Some("Centre"));
        let inside: Vec<String> = ed.view().graph.nodes.iter().map(|n| n.kind.clone()).collect();
        assert!(inside.contains(&"layer.seat".to_string()) && inside.contains(&view::INPUTS_KIND.to_string()) == false || inside.contains(&view::OUTPUTS_KIND.to_string()), "{inside:?}");
        // A node added inside joins the group.
        assert!(ed.insert("number", [0.0, 0.0], &reg));
        assert!(ed.commit());
        let added = ed.graph().nodes.iter().find(|n| n.kind == "number").unwrap().id;
        assert!(ed.graph().groups[0].nodes.contains(&added));
        ed.leave_group();
        assert!(ed.entered_group().is_none());
        assert_eq!(ed.view().graph.nodes.len(), top, "the number is folded into the group");
        // Focusing a member from outside centres its group.
        let seat = g.nodes.iter().find(|n| n.kind == "layer.seat").unwrap().id;
        ed.focus(seat);
        assert_eq!(ed.view_node_of(seat), Some(view::group_id(0)));
    }

    #[test]
    fn focus_selects_and_is_consumed_by_a_frame() {
        let reg = Registry::builtin();
        let g = flat("Braided band");
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
        let g = flat("Braided band");
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
    /// renders the editor over a template graph through wgpu and writes
    /// `graph.png` there, for an eyeball pass on the look.
    #[test]
    fn shot_the_editor() {
        let Some(dir) = std::env::var_os("RD_GRAPH_SHOT") else { return };
        let reg = ringdesign_graph::registry::Registry::builtin();
        let name = std::env::var("RD_GRAPH_SHOT_TEMPLATE").unwrap_or_else(|_| "Gypsy trio".into());
        let g = ringdesign_graph::templates::graph(&name).expect("a template graph");
        let mut ed = super::Editor::new(g, &reg);
        if let Ok(i) = std::env::var("RD_GRAPH_SHOT_ENTER").map(|s| s.parse::<usize>().unwrap_or(0)) {
            ed.enter_group(i);
        }
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
mod layout_tests {
    use super::tests::flat;
    use super::*;

    fn arranged(name: &str) -> (Editor, Registry) {
        let reg = Registry::builtin();
        let mut ed = Editor::new(flat(name), &reg);
        ed.arrange(&reg);
        (ed, reg)
    }

    #[test]
    fn a_source_sits_one_column_before_what_it_feeds() {
        let reg = Registry::builtin();
        let g = flat("Braided band");
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
    fn folded_layers_arrange_as_a_chain_of_groups() {
        let reg = Registry::builtin();
        let g = ringdesign_graph::templates::graph("Aster Atelier").unwrap();
        let mut ed = Editor::new(g, &reg);
        ed.arrange(&reg);
        let rows: Vec<f32> = ed.view().graph.nodes.iter().filter(|n| n.kind == view::GROUP_KIND).map(|n| n.pos[1]).collect();
        assert!(rows.iter().filter(|y| **y == 0.0).count() >= rows.len() / 2, "the layer groups run along the first row: {rows:?}");
        assert!(ed.graph().groups.iter().all(|gr| gr.pos.is_some()), "arranging the folded view places every group");
    }

    #[test]
    fn a_grid_the_nodes_overflow_is_untangled_once_and_a_tidy_graph_is_left_alone() {
        let reg = Registry::builtin();
        let mut g = flat("Shouldered cushion signet");
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
        // The minimap takes presses of its own; keep it off the decoy.
        ed.show_minimap = false;
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
        let g = flat("Braided band");
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

