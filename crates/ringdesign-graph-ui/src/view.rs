//! The part of a graph an editor shows: collapsed groups as single nodes, or the inside of one group.

use std::collections::{BTreeMap, BTreeSet};

use ringdesign_graph::graph::{Graph, Node, NodeId, Wire};
use ringdesign_graph::registry::PinSpec;

/// Ids at and above this are view-only nodes that never reach the graph.
pub const PSEUDO_BASE: u64 = 1 << 62;
pub const GROUP_KIND: &str = "@group";
pub const INPUTS_KIND: &str = "@group.inputs";
pub const OUTPUTS_KIND: &str = "@group.outputs";
const INPUTS_ID: u64 = PSEUDO_BASE + (1 << 40);
const OUTPUTS_ID: u64 = INPUTS_ID + 1;

pub fn is_pseudo(id: NodeId) -> bool {
    id.0 >= PSEUDO_BASE
}

/// What a view-only node stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pseudo {
    /// A collapsed group, by index in the graph's groups.
    Group(usize),
    /// Inside a group: the sources outside it.
    Inputs,
    /// Inside a group: the readers outside it.
    Outputs,
}

/// What one pin of a view-only node stands for in the graph.
#[derive(Clone, Debug, PartialEq)]
pub enum Port {
    /// A member's input fed from outside the group.
    In { node: NodeId, input: String },
    /// A member's output read outside the group.
    Out { node: NodeId, out: String },
    /// An outside output feeding members.
    Source { node: NodeId, out: String },
    /// An outside reader of a member's output, every one of them.
    Sinks { to: Vec<(NodeId, String)> },
}

/// One pin of a view-only node.
#[derive(Clone, Debug, PartialEq)]
pub struct PortPin {
    pub spec: PinSpec,
    pub port: Port,
}

/// The view: a graph of real and view-only nodes, and how it maps back.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub graph: Graph,
    /// The group whose inside this is.
    pub entered: Option<usize>,
    pub pseudo: BTreeMap<NodeId, Pseudo>,
    pub ins: BTreeMap<NodeId, Vec<PortPin>>,
    pub outs: BTreeMap<NodeId, Vec<PortPin>>,
    /// Graph wires the view does not show.
    pub kept: Vec<Wire>,
    /// Graph nodes the view does not show.
    pub hidden: BTreeSet<NodeId>,
    /// Which collapsed group folds each hidden node, at the top level.
    pub claimed: BTreeMap<NodeId, usize>,
}

/// A node's pins and title, as the editor resolved them.
pub trait Cards {
    fn pins(&self, id: NodeId) -> (Vec<PinSpec>, Vec<PinSpec>);
    fn title(&self, id: NodeId) -> String;
}

/// The members of group `index` that are still in the graph.
pub fn members(g: &Graph, index: usize) -> Vec<NodeId> {
    g.groups.get(index).map(|gr| gr.nodes.iter().copied().filter(|id| g.contains(*id)).collect()).unwrap_or_default()
}

/// The pseudo id a collapsed group is drawn under.
pub fn group_id(index: usize) -> NodeId {
    NodeId(PSEUDO_BASE + index as u64)
}

/// Where a collapsed group stands: its own position, else its members' top-left.
pub fn group_pos(g: &Graph, index: usize) -> [f32; 2] {
    if let Some(p) = g.groups.get(index).and_then(|gr| gr.pos) {
        return p;
    }
    let pts: Vec<[f32; 2]> = members(g, index).iter().filter_map(|id| g.node(*id)).map(|n| n.pos).collect();
    if pts.is_empty() {
        return [0.0, 0.0];
    }
    [pts.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min)]
}

/// Which collapsed group each node is drawn inside, the first group naming it winning.
pub fn claims(g: &Graph) -> BTreeMap<NodeId, usize> {
    let mut claimed = BTreeMap::new();
    for (i, gr) in g.groups.iter().enumerate().filter(|(_, gr)| gr.collapsed) {
        for id in gr.nodes.iter().filter(|id| g.contains(**id)) {
            claimed.entry(*id).or_insert(i);
        }
    }
    claimed
}

/// Unique pin names, the plain name first and the node's title in front where two collide.
fn name_pins(pins: &mut [PortPin], who: impl Fn(&Port) -> String) {
    let plain: Vec<String> = pins.iter().map(|p| p.spec.name.clone()).collect();
    let mut used: BTreeSet<String> = BTreeSet::new();
    for (i, pin) in pins.iter_mut().enumerate() {
        let clash = plain.iter().filter(|n| **n == plain[i]).count() > 1;
        let mut name = if clash { format!("{} · {}", who(&pin.port), plain[i]) } else { plain[i].clone() };
        let mut k = 2;
        while !used.insert(name.clone()) {
            name = format!("{} {k}", if clash { format!("{} · {}", who(&pin.port), plain[i]) } else { plain[i].clone() });
            k += 1;
        }
        pin.spec.name = name;
    }
}

fn pseudo_node(id: NodeId, kind: &str, label: &str, pos: [f32; 2]) -> Node {
    Node { id, kind: kind.into(), params: serde_json::Value::Null, inputs: BTreeMap::new(), pos, label: Some(label.into()) }
}

fn spec_of<'a>(pins: &'a [PinSpec], name: &str) -> Option<&'a PinSpec> {
    pins.iter().find(|p| p.name == name)
}

/// The view of `g` with `entered` open, or with every collapsed group folded when `None`.
pub fn project(g: &Graph, entered: Option<usize>, cards: &dyn Cards) -> View {
    let entered = entered.filter(|i| !members(g, *i).is_empty());
    match entered {
        Some(index) => project_inside(g, index, cards),
        None => project_top(g, cards),
    }
}

fn base_graph(g: &Graph) -> Graph {
    Graph { nodes: Vec::new(), wires: Vec::new(), exposed: Vec::new(), outputs: Vec::new(), groups: Vec::new(), ..g.clone() }
}

fn project_top(g: &Graph, cards: &dyn Cards) -> View {
    let claimed = claims(g);
    if claimed.is_empty() {
        return View { graph: g.clone(), ..View::default() };
    }
    let mut view = View { graph: base_graph(g), hidden: claimed.keys().copied().collect(), claimed: claimed.clone(), ..View::default() };
    view.graph.nodes = g.nodes.iter().filter(|n| !claimed.contains_key(&n.id)).cloned().collect();
    view.graph.nodes.sort_by_key(|n| n.id);
    let collapsed: BTreeSet<usize> = claimed.values().copied().collect();
    let mut ins: BTreeMap<usize, Vec<PortPin>> = BTreeMap::new();
    let mut outs: BTreeMap<usize, Vec<PortPin>> = BTreeMap::new();
    let order = g.topo().unwrap_or_else(|_| g.nodes.iter().map(|n| n.id).collect());
    let rank: BTreeMap<NodeId, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut wires: Vec<Wire> = g.wires.clone();
    wires.sort_by_key(|w| (rank.get(&w.to).copied(), rank.get(&w.from).copied()));
    for w in &wires {
        let (from, to) = (claimed.get(&w.from).copied(), claimed.get(&w.to).copied());
        if from.is_some() && from == to {
            continue;
        }
        if let Some(i) = to {
            let port = Port::In { node: w.to, input: w.input.clone() };
            let list = ins.entry(i).or_default();
            if !list.iter().any(|p| p.port == port) {
                if let Some(spec) = spec_of(&cards.pins(w.to).0, &w.input) {
                    list.push(PortPin { spec: spec.clone(), port });
                }
            }
        }
        if let Some(i) = from {
            let port = Port::Out { node: w.from, out: w.out.clone() };
            let list = outs.entry(i).or_default();
            if !list.iter().any(|p| p.port == port) {
                if let Some(spec) = spec_of(&cards.pins(w.from).1, &w.out) {
                    list.push(PortPin { spec: spec.clone(), port });
                }
            }
        }
    }
    let who = |p: &Port| match p {
        Port::In { node, .. } | Port::Out { node, .. } | Port::Source { node, .. } => cards.title(*node),
        Port::Sinks { .. } => String::new(),
    };
    for i in &collapsed {
        let id = group_id(*i);
        let mut pins_in = ins.remove(i).unwrap_or_default();
        let mut pins_out = outs.remove(i).unwrap_or_default();
        name_pins(&mut pins_in, who);
        name_pins(&mut pins_out, who);
        view.graph.nodes.push(pseudo_node(id, GROUP_KIND, &g.groups[*i].name, group_pos(g, *i)));
        view.pseudo.insert(id, Pseudo::Group(*i));
        view.ins.insert(id, pins_in);
        view.outs.insert(id, pins_out);
    }
    let pin_name = |id: NodeId, port: &Port, inputs: bool| -> Option<String> {
        let list = if inputs { view.ins.get(&id) } else { view.outs.get(&id) };
        list.and_then(|l| l.iter().find(|p| p.port == *port)).map(|p| p.spec.name.clone())
    };
    let mut shown = Vec::new();
    for w in &g.wires {
        let (from, to) = (claimed.get(&w.from).copied(), claimed.get(&w.to).copied());
        if from.is_some() && from == to {
            view.kept.push(w.clone());
            continue;
        }
        let (f, out) = match from {
            Some(i) => (group_id(i), pin_name(group_id(i), &Port::Out { node: w.from, out: w.out.clone() }, false)),
            None => (w.from, Some(w.out.clone())),
        };
        let (t, input) = match to {
            Some(i) => (group_id(i), pin_name(group_id(i), &Port::In { node: w.to, input: w.input.clone() }, true)),
            None => (w.to, Some(w.input.clone())),
        };
        match (out, input) {
            (Some(out), Some(input)) => shown.push(Wire { from: f, out, to: t, input }),
            _ => view.kept.push(w.clone()),
        }
    }
    view.graph.wires = shown;
    let live: BTreeSet<NodeId> = view.graph.nodes.iter().map(|n| n.id).collect();
    view.graph.exposed = g.exposed.iter().filter(|e| live.contains(&e.node)).cloned().collect();
    view.graph.outputs = g.outputs.iter().filter(|e| live.contains(&e.node)).cloned().collect();
    view.graph.groups = g
        .groups
        .iter()
        .filter(|gr| !gr.collapsed)
        .filter_map(|gr| {
            let nodes: Vec<NodeId> = gr.nodes.iter().copied().filter(|id| live.contains(id)).collect();
            (!nodes.is_empty()).then(|| ringdesign_graph::graph::NodeGroup { nodes, ..gr.clone() })
        })
        .collect();
    view
}

fn project_inside(g: &Graph, index: usize, cards: &dyn Cards) -> View {
    let inside: BTreeSet<NodeId> = members(g, index).into_iter().collect();
    let mut view = View { graph: base_graph(g), entered: Some(index), ..View::default() };
    view.hidden = g.nodes.iter().map(|n| n.id).filter(|id| !inside.contains(id)).collect();
    view.graph.nodes = g.nodes.iter().filter(|n| inside.contains(&n.id)).cloned().collect();
    view.graph.nodes.sort_by_key(|n| n.id);
    let mut sources: Vec<PortPin> = Vec::new();
    let mut sinks: Vec<(NodeId, String, PortPin)> = Vec::new();
    for w in &g.wires {
        match (inside.contains(&w.from), inside.contains(&w.to)) {
            (true, true) => {}
            (false, true) => {
                let port = Port::Source { node: w.from, out: w.out.clone() };
                if !sources.iter().any(|p| p.port == port) {
                    if let Some(spec) = spec_of(&cards.pins(w.from).1, &w.out) {
                        sources.push(PortPin { spec: spec.clone(), port });
                    }
                }
            }
            (true, false) => match sinks.iter_mut().find(|(n, o, _)| *n == w.from && *o == w.out) {
                Some((_, _, pin)) => {
                    if let Port::Sinks { to } = &mut pin.port {
                        to.push((w.to, w.input.clone()));
                    }
                }
                None => {
                    if let Some(spec) = spec_of(&cards.pins(w.from).1, &w.out) {
                        sinks.push((w.from, w.out.clone(), PortPin { spec: spec.clone(), port: Port::Sinks { to: vec![(w.to, w.input.clone())] } }));
                    }
                }
            },
            (false, false) => view.kept.push(w.clone()),
        }
    }
    for pin in &mut sources {
        if let Port::Source { node, .. } = &pin.port {
            pin.spec.name = format!("{} · {}", cards.title(*node), pin.spec.name);
        }
    }
    for (node, _, pin) in &mut sinks {
        pin.spec.name = format!("{} · {}", cards.title(*node), pin.spec.name);
    }
    name_pins(&mut sources, |_| String::new());
    let mut outputs: Vec<PortPin> = sinks.iter().map(|(_, _, p)| p.clone()).collect();
    name_pins(&mut outputs, |_| String::new());
    let pts: Vec<[f32; 2]> = view.graph.nodes.iter().map(|n| n.pos).collect();
    let min_x = pts.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = pts.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    let min_y = pts.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let mut shown: Vec<Wire> = g.wires.iter().filter(|w| inside.contains(&w.from) && inside.contains(&w.to)).cloned().collect();
    if !sources.is_empty() {
        let id = NodeId(INPUTS_ID);
        view.graph.nodes.push(pseudo_node(id, INPUTS_KIND, "Group inputs", [min_x - 340.0, min_y]));
        view.pseudo.insert(id, Pseudo::Inputs);
        for w in g.wires.iter().filter(|w| !inside.contains(&w.from) && inside.contains(&w.to)) {
            let port = Port::Source { node: w.from, out: w.out.clone() };
            if let Some(pin) = sources.iter().find(|p| p.port == port) {
                shown.push(Wire { from: id, out: pin.spec.name.clone(), to: w.to, input: w.input.clone() });
            }
        }
        view.outs.insert(id, sources);
        view.ins.insert(id, Vec::new());
    }
    if !outputs.is_empty() {
        let id = NodeId(OUTPUTS_ID);
        view.graph.nodes.push(pseudo_node(id, OUTPUTS_KIND, "Group outputs", [max_x + 320.0, min_y]));
        view.pseudo.insert(id, Pseudo::Outputs);
        for ((node, out, _), pin) in sinks.iter().zip(&outputs) {
            shown.push(Wire { from: *node, out: out.clone(), to: id, input: pin.spec.name.clone() });
        }
        view.ins.insert(id, outputs);
        view.outs.insert(id, Vec::new());
    }
    view.graph.wires = shown;
    view.graph.exposed = g.exposed.iter().filter(|e| inside.contains(&e.node)).cloned().collect();
    view.graph.outputs = g.outputs.iter().filter(|e| inside.contains(&e.node)).cloned().collect();
    view
}

/// The graph wires a view wire stands for.
fn translate(view: &View, w: &Wire) -> Vec<Wire> {
    let port = |id: NodeId, name: &str, inputs: bool| -> Option<&Port> {
        let list = if inputs { view.ins.get(&id) } else { view.outs.get(&id) }?;
        list.iter().find(|p| p.spec.name == name).map(|p| &p.port)
    };
    let from = if is_pseudo(w.from) {
        match port(w.from, &w.out, false) {
            Some(Port::Out { node, out }) | Some(Port::Source { node, out }) => (*node, out.clone()),
            _ => return Vec::new(),
        }
    } else {
        (w.from, w.out.clone())
    };
    let to: Vec<(NodeId, String)> = if is_pseudo(w.to) {
        match port(w.to, &w.input, true) {
            Some(Port::In { node, input }) => vec![(*node, input.clone())],
            Some(Port::Sinks { to }) => to.clone(),
            _ => return Vec::new(),
        }
    } else {
        vec![(w.to, w.input.clone())]
    };
    to.into_iter().map(|(to, input)| Wire { from: from.0, out: from.1.clone(), to, input }).collect()
}

/// `root` with the edits made to `view` (now `edited`) applied: nodes kept, moved, added, removed, rewired.
pub fn merge(root: &Graph, view: &View, edited: &Graph) -> Graph {
    let mut g = root.clone();
    let shown: BTreeSet<NodeId> = view.graph.nodes.iter().map(|n| n.id).filter(|id| !is_pseudo(*id)).collect();
    let now: BTreeMap<NodeId, &Node> = edited.nodes.iter().filter(|n| !is_pseudo(n.id)).map(|n| (n.id, n)).collect();
    for (id, n) in &now {
        match g.node_mut(*id) {
            Some(slot) => *slot = (*n).clone(),
            None => {
                g.nodes.push((*n).clone());
                g.next_id = g.next_id.max(id.0 + 1);
                if let Some(gr) = view.entered.and_then(|i| g.groups.get_mut(i)) {
                    gr.nodes.push(*id);
                }
            }
        }
    }
    let mut gone: BTreeSet<NodeId> = shown.iter().copied().filter(|id| !now.contains_key(id)).collect();
    for (id, pseudo) in &view.pseudo {
        if let Pseudo::Group(i) = pseudo {
            match edited.node(*id) {
                Some(n) => {
                    if let Some(gr) = g.groups.get_mut(*i) {
                        if group_pos(root, *i) != n.pos || gr.pos.is_some() {
                            gr.pos = Some(n.pos);
                        }
                    }
                }
                None => gone.extend(members(root, *i)),
            }
        }
    }
    // A port node deleted by itself takes no wires with it.
    let mut edited_wires: Vec<Wire> = edited.wires.clone();
    for (id, pseudo) in &view.pseudo {
        if matches!(pseudo, Pseudo::Inputs | Pseudo::Outputs) && edited.node(*id).is_none() {
            edited_wires.extend(view.graph.wires.iter().filter(|w| w.from == *id || w.to == *id).cloned());
        }
    }
    let mut fresh: Vec<Wire> = Vec::new();
    for w in &edited_wires {
        for t in translate(view, w) {
            if let Some(i) = fresh.iter().position(|f| f.to == t.to && f.input == t.input) {
                fresh.remove(i);
            }
            fresh.push(t);
        }
    }
    let fed: BTreeSet<(NodeId, String)> = fresh.iter().map(|w| (w.to, w.input.clone())).collect();
    let mut wires: Vec<Wire> = Vec::with_capacity(root.wires.len());
    for w in &root.wires {
        if let Some(i) = fresh.iter().position(|f| f == w) {
            wires.push(fresh.remove(i));
        } else if view.kept.contains(w) && !fed.contains(&(w.to, w.input.clone())) {
            wires.push(w.clone());
        }
    }
    fresh.sort_by(|a, b| (a.to, &a.input, a.from, &a.out).cmp(&(b.to, &b.input, b.from, &b.out)));
    wires.extend(fresh);
    g.wires = wires;
    for id in gone {
        let _ = g.remove(id);
    }
    let live: BTreeSet<NodeId> = g.nodes.iter().map(|n| n.id).collect();
    g.wires.retain(|w| live.contains(&w.from) && live.contains(&w.to));
    g
}

/// The member that stands for a group when it is chosen: its layer, else its entry, else its last node.
pub fn representative(g: &Graph, index: usize) -> Option<NodeId> {
    let m = members(g, index);
    let kind = |id: &NodeId| g.node(*id).map(|n| n.kind.clone()).unwrap_or_default();
    m.iter().find(|id| kind(id).starts_with("layer.")).or_else(|| m.iter().find(|id| kind(id) == "entry")).or_else(|| m.last()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_graph::registry::Registry;

    struct Reg(Registry, Graph);
    impl Cards for Reg {
        fn pins(&self, id: NodeId) -> (Vec<PinSpec>, Vec<PinSpec>) {
            self.1.node(id).and_then(|n| self.0.node_pins(n)).unwrap_or_default()
        }
        fn title(&self, id: NodeId) -> String {
            self.1.node(id).map(|n| n.label.clone().unwrap_or_else(|| n.kind.clone())).unwrap_or_default()
        }
    }

    fn trio() -> Graph {
        ringdesign_graph::templates::graph("Gypsy trio").unwrap()
    }

    #[test]
    fn a_graph_without_collapsed_groups_is_its_own_view() {
        let mut g = trio();
        for gr in &mut g.groups {
            gr.collapsed = false;
        }
        let cards = Reg(Registry::builtin(), g.clone());
        let v = project(&g, None, &cards);
        assert_eq!(v.graph, g);
        assert_eq!(merge(&g, &v, &v.graph), g);
    }

    #[test]
    fn collapsed_layers_read_as_a_chain_and_an_untouched_view_merges_back_exactly() {
        let g = trio();
        let cards = Reg(Registry::builtin(), g.clone());
        let v = project(&g, None, &cards);
        let groups: Vec<&Node> = v.graph.nodes.iter().filter(|n| n.kind == GROUP_KIND).collect();
        assert_eq!(groups.len(), 4);
        let centre = groups[0].id;
        assert_eq!(v.outs[&centre].iter().map(|p| p.spec.name.as_str()).collect::<Vec<_>>(), ["stack"]);
        let right = groups[1].id;
        assert_eq!(v.ins[&right].iter().map(|p| p.spec.name.as_str()).collect::<Vec<_>>(), ["stack"]);
        assert!(v.graph.wires.iter().any(|w| w.from == centre && w.to == right), "the layers chain group to group");
        assert!(v.graph.validate(None).is_empty(), "{:?}", v.graph.validate(None));
        assert_eq!(merge(&g, &v, &v.graph), g);
    }

    #[test]
    fn wiring_a_group_pin_wires_the_member_it_stands_for() {
        let g = trio();
        let cards = Reg(Registry::builtin(), g.clone());
        let v = project(&g, None, &cards);
        let groups: Vec<NodeId> = v.graph.nodes.iter().filter(|n| n.kind == GROUP_KIND).map(|n| n.id).collect();
        // Feed the last layer straight from the first, skipping the middle one.
        let mut edited = v.graph.clone();
        edited.wires.retain(|w| w.to != groups[2]);
        edited.wires.push(Wire { from: groups[0], out: "stack".into(), to: groups[2], input: "stack".into() });
        let merged = merge(&g, &v, &edited);
        let left_stack = members(&g, 2).into_iter().find(|id| g.node(*id).unwrap().kind == "stack").unwrap();
        let centre_stack = members(&g, 0).into_iter().find(|id| g.node(*id).unwrap().kind == "stack").unwrap();
        assert_eq!(merged.wire_into(left_stack, "stack").map(|w| w.from), Some(centre_stack));
        assert_eq!(merged.wires.len(), g.wires.len(), "one wire replaced another");
        assert_eq!(merged.nodes, g.nodes);
    }

    #[test]
    fn moving_a_group_moves_it_and_deleting_one_deletes_its_members() {
        let g = trio();
        let cards = Reg(Registry::builtin(), g.clone());
        let v = project(&g, None, &cards);
        let first = v.graph.nodes.iter().find(|n| n.kind == GROUP_KIND).unwrap().id;
        let mut edited = v.graph.clone();
        edited.node_mut(first).unwrap().pos = [500.0, 900.0];
        let moved = merge(&g, &v, &edited);
        assert_eq!(moved.groups[0].pos, Some([500.0, 900.0]));
        assert_eq!(moved.nodes, g.nodes, "members stay where they are inside");
        edited.nodes.retain(|n| n.id != first);
        edited.wires.retain(|w| w.from != first && w.to != first);
        let cut = merge(&g, &v, &edited);
        assert_eq!(cut.nodes.len(), g.nodes.len() - members(&g, 0).len());
        assert!(cut.validate(None).is_empty(), "{:?}", cut.validate(None));
    }

    #[test]
    fn inside_a_group_the_outside_is_ports_and_new_nodes_join_the_group() {
        let g = trio();
        let cards = Reg(Registry::builtin(), g.clone());
        let v = project(&g, Some(1), &cards);
        assert_eq!(v.entered, Some(1));
        let inputs = v.graph.nodes.iter().find(|n| n.kind == INPUTS_KIND).expect("the stack comes in from outside").id;
        let outputs = v.graph.nodes.iter().find(|n| n.kind == OUTPUTS_KIND).expect("the stack leaves").id;
        assert_eq!(v.outs[&inputs].len(), 1);
        assert_eq!(v.ins[&outputs].len(), 1);
        assert_eq!(merge(&g, &v, &v.graph), g, "an untouched inside merges back exactly");
        let mut edited = v.graph.clone();
        let fresh = NodeId(v.graph.next_id);
        edited.nodes.push(Node { id: fresh, kind: "number".into(), params: serde_json::Value::Null, inputs: BTreeMap::new(), pos: [0.0, 0.0], label: None });
        edited.next_id += 1;
        let merged = merge(&g, &v, &edited);
        assert!(merged.groups[1].nodes.contains(&fresh));
        assert!(merged.contains(fresh));
        // Cutting the wire into the outputs port disconnects the readers outside.
        let mut cut = v.graph.clone();
        cut.wires.retain(|w| w.to != outputs);
        let merged = merge(&g, &v, &cut);
        let left_stack = members(&g, 2).into_iter().find(|id| g.node(*id).unwrap().kind == "stack").unwrap();
        assert!(merged.wire_into(left_stack, "stack").is_none());
    }

    #[test]
    fn a_group_stands_for_its_layer() {
        let g = trio();
        let rep = representative(&g, 0).unwrap();
        assert_eq!(g.node(rep).unwrap().kind, "layer.seat");
    }
}
