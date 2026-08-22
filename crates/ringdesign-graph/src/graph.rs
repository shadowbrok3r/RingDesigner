//! The graph document: nodes, wires, exposed parameters and the mode.
//!
//! This is the truth. Editors rebuild their view from it every frame and
//! write back through the same few operations; `pos` is the only view data
//! it keeps. A [`NodeId`] is handed out once by the graph and never reused,
//! so a wire, an exposed parameter or an undo entry that names a node keeps
//! naming it after any other node is removed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::value::{Literal, ValueKind};
use crate::MAX_NODES;

/// A node's identity, stable for the life of the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Which rules the graph evaluates under.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    /// The castable ring: the design comes with its field verdict and a
    /// file-writing sink refuses a ring that will not release.
    #[default]
    SandRing,
    /// Everything SandRing has, plus the solid kernel and the mesh verifier.
    Free,
}

/// Whether a pin takes one value per run or the whole list at once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Access {
    #[default]
    Item,
    List,
}

/// One node: a registry kind, its parameters, and the literals on its
/// unwired inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// The registry key, e.g. `band.profile`.
    pub kind: String,
    /// Node-level settings that are not pins (a script's source, a
    /// cluster's path), as JSON the kind interprets.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
    /// Values on inputs that carry no wire.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Literal>,
    /// Editor position; the only view data persisted.
    #[serde(default)]
    pub pos: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A connection from one node's output to another node's input, by name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Wire {
    pub from: NodeId,
    pub out: String,
    pub to: NodeId,
    pub input: String,
}

/// An input promoted to the graph's own parameter panel — what a cluster
/// shows when it is used as a node, and what a preset sets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exposed {
    pub node: NodeId,
    pub input: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
}

/// The document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub name: String,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub wires: Vec<Wire>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exposed: Vec<Exposed>,
    /// The next id to hand out; never decremented.
    #[serde(default)]
    pub next_id: u64,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new("Untitled", Mode::SandRing)
    }
}

/// What a graph cannot do or cannot be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphError {
    pub node: Option<NodeId>,
    pub message: String,
}

impl GraphError {
    pub fn at(node: NodeId, message: impl Into<String>) -> Self {
        Self { node: Some(node), message: message.into() }
    }
    pub fn global(message: impl Into<String>) -> Self {
        Self { node: None, message: message.into() }
    }
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node {
            Some(n) => write!(f, "{n}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for GraphError {}

/// One pin as a registry describes it for a particular node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinInfo {
    pub name: String,
    pub kind: ValueKind,
    pub access: Access,
}

/// A node's pins as resolved for that instance, and the modes it may run in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodePins {
    pub inputs: Vec<PinInfo>,
    pub outputs: Vec<PinInfo>,
    pub modes: Vec<Mode>,
}

impl NodePins {
    pub fn input(&self, name: &str) -> Option<&PinInfo> {
        self.inputs.iter().find(|p| p.name == name)
    }
    pub fn output(&self, name: &str) -> Option<&PinInfo> {
        self.outputs.iter().find(|p| p.name == name)
    }
}

/// What validation asks of a registry: the pins a node instance has.
/// `None` means the kind is unknown.
pub trait PinLookup {
    fn pins(&self, node: &Node) -> Option<NodePins>;
}

impl Graph {
    pub fn new(name: impl Into<String>, mode: Mode) -> Self {
        Self { name: name.into(), mode, nodes: Vec::new(), wires: Vec::new(), exposed: Vec::new(), next_id: 1 }
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.node(id).is_some()
    }

    /// Add a node of `kind`; the id is fresh and never reused.
    pub fn add(&mut self, kind: impl Into<String>) -> Result<NodeId, GraphError> {
        if self.nodes.len() >= MAX_NODES {
            return Err(GraphError::global(format!("a graph holds at most {MAX_NODES} nodes")));
        }
        // A loaded file may carry a stale counter; stay above every id.
        let floor = self.nodes.iter().map(|n| n.id.0 + 1).max().unwrap_or(1);
        self.next_id = self.next_id.max(floor);
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(Node {
            id,
            kind: kind.into(),
            params: serde_json::Value::Null,
            inputs: BTreeMap::new(),
            pos: [0.0, 0.0],
            label: None,
        });
        Ok(id)
    }

    /// Remove a node and every wire and exposure that names it.
    pub fn remove(&mut self, id: NodeId) -> Result<Node, GraphError> {
        let i = self.nodes.iter().position(|n| n.id == id).ok_or_else(|| GraphError::at(id, "no such node"))?;
        let node = self.nodes.remove(i);
        self.wires.retain(|w| w.from != id && w.to != id);
        self.exposed.retain(|e| e.node != id);
        Ok(node)
    }

    /// Wire `from.out` into `to.input`. An input carries one wire: an
    /// existing one is displaced and returned. A literal on that input is
    /// left in place and shadowed while the wire exists.
    pub fn connect(
        &mut self,
        from: NodeId,
        out: impl Into<String>,
        to: NodeId,
        input: impl Into<String>,
    ) -> Result<Option<Wire>, GraphError> {
        if !self.contains(from) {
            return Err(GraphError::at(from, "no such node"));
        }
        if !self.contains(to) {
            return Err(GraphError::at(to, "no such node"));
        }
        if from == to {
            return Err(GraphError::at(to, "a node cannot feed itself"));
        }
        let input = input.into();
        if self.reaches(to, from) {
            return Err(GraphError::at(to, format!("wiring {from} into {input} would make a cycle")));
        }
        let displaced = self.disconnect(to, &input);
        self.wires.push(Wire { from, out: out.into(), to, input });
        Ok(displaced)
    }

    /// Drop the wire into `to.input`, if any.
    pub fn disconnect(&mut self, to: NodeId, input: &str) -> Option<Wire> {
        let i = self.wires.iter().position(|w| w.to == to && w.input == input)?;
        Some(self.wires.remove(i))
    }

    /// The wire feeding `to.input`.
    pub fn wire_into(&self, to: NodeId, input: &str) -> Option<&Wire> {
        self.wires.iter().find(|w| w.to == to && w.input == input)
    }

    pub fn wires_into(&self, to: NodeId) -> impl Iterator<Item = &Wire> {
        self.wires.iter().filter(move |w| w.to == to)
    }

    pub fn wires_from(&self, from: NodeId) -> impl Iterator<Item = &Wire> {
        self.wires.iter().filter(move |w| w.from == from)
    }

    /// Whether `to` is downstream of `from` (or is `from`).
    pub fn reaches(&self, from: NodeId, to: NodeId) -> bool {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([from]);
        while let Some(n) = queue.pop_front() {
            if n == to {
                return true;
            }
            if seen.insert(n) {
                queue.extend(self.wires_from(n).map(|w| w.to));
            }
        }
        false
    }

    /// Set a literal on an unwired input (or behind a wire, shadowed).
    pub fn set_input(&mut self, id: NodeId, input: impl Into<String>, value: Literal) -> Result<(), GraphError> {
        let node = self.node_mut(id).ok_or_else(|| GraphError::at(id, "no such node"))?;
        node.inputs.insert(input.into(), value);
        Ok(())
    }

    pub fn clear_input(&mut self, id: NodeId, input: &str) -> Result<Option<Literal>, GraphError> {
        let node = self.node_mut(id).ok_or_else(|| GraphError::at(id, "no such node"))?;
        Ok(node.inputs.remove(input))
    }

    /// Set a value inside `params` at an RFC 6901 pointer (`/a/b`),
    /// creating objects along the way. An empty pointer replaces the whole.
    pub fn set_param(&mut self, id: NodeId, pointer: &str, value: serde_json::Value) -> Result<(), GraphError> {
        let node = self.node_mut(id).ok_or_else(|| GraphError::at(id, "no such node"))?;
        set_pointer(&mut node.params, pointer, value).map_err(|m| GraphError::at(id, m))
    }

    pub fn param(&self, id: NodeId, pointer: &str) -> Option<&serde_json::Value> {
        let node = self.node(id)?;
        if pointer.is_empty() { Some(&node.params) } else { node.params.pointer(pointer) }
    }

    /// Promote an input to the graph's parameter panel under `name`.
    pub fn expose(&mut self, node: NodeId, input: impl Into<String>, name: impl Into<String>) -> Result<(), GraphError> {
        if !self.contains(node) {
            return Err(GraphError::at(node, "no such node"));
        }
        let (input, name) = (input.into(), name.into());
        if let Some(e) = self.exposed.iter().find(|e| e.name == name) {
            if e.node != node || e.input != input {
                return Err(GraphError::at(node, format!("{name:?} is already exposed from {}.{}", e.node, e.input)));
            }
            return Ok(());
        }
        self.exposed.retain(|e| !(e.node == node && e.input == input));
        self.exposed.push(Exposed { node, input, name, doc: String::new() });
        Ok(())
    }

    pub fn unexpose(&mut self, name: &str) -> Option<Exposed> {
        let i = self.exposed.iter().position(|e| e.name == name)?;
        Some(self.exposed.remove(i))
    }

    /// Nodes in an order every wire runs forward, or the node a cycle
    /// passes through. Ties break by id, so the order is deterministic.
    pub fn topo(&self) -> Result<Vec<NodeId>, GraphError> {
        let mut indeg: BTreeMap<NodeId, usize> = self.nodes.iter().map(|n| (n.id, 0)).collect();
        let mut out: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        for w in &self.wires {
            if !indeg.contains_key(&w.from) {
                continue;
            }
            if let Some(d) = indeg.get_mut(&w.to) {
                *d += 1;
                out.entry(w.from).or_default().push(w.to);
            }
        }
        let mut ready: BTreeSet<NodeId> = indeg.iter().filter(|(_, d)| **d == 0).map(|(id, _)| *id).collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(&id) = ready.iter().next() {
            ready.remove(&id);
            order.push(id);
            if let Some(next) = out.get(&id) {
                for &to in next {
                    let d = indeg.get_mut(&to).expect("counted");
                    *d -= 1;
                    if *d == 0 {
                        ready.insert(to);
                    }
                }
            }
        }
        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            let stuck = indeg.iter().find(|(_, d)| **d > 0).map(|(id, _)| *id).expect("a node is stuck");
            Err(GraphError::at(stuck, "the graph has a cycle through this node"))
        }
    }

    /// Every node this one depends on, nearest first.
    pub fn upstream(&self, id: NodeId) -> Vec<NodeId> {
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::from([id]);
        while let Some(n) = queue.pop_front() {
            for w in self.wires_into(n) {
                if seen.insert(w.from) {
                    order.push(w.from);
                    queue.push_back(w.from);
                }
            }
        }
        order
    }

    /// Every node that depends on this one, nearest first.
    pub fn downstream(&self, id: NodeId) -> Vec<NodeId> {
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::from([id]);
        while let Some(n) = queue.pop_front() {
            for w in self.wires_from(n) {
                if seen.insert(w.to) {
                    order.push(w.to);
                    queue.push_back(w.to);
                }
            }
        }
        order
    }

    /// Everything wrong with the document, structurally and — given a
    /// registry — by kind, pin and type. Empty means evaluable.
    pub fn validate(&self, reg: Option<&dyn PinLookup>) -> Vec<GraphError> {
        let mut errs = Vec::new();
        if self.nodes.len() > MAX_NODES {
            errs.push(GraphError::global(format!("{} nodes; the cap is {MAX_NODES}", self.nodes.len())));
        }
        let mut ids = BTreeSet::new();
        for n in &self.nodes {
            if !ids.insert(n.id) {
                errs.push(GraphError::at(n.id, "duplicate node id"));
            }
        }
        let mut seen_inputs = BTreeSet::new();
        for w in &self.wires {
            if !ids.contains(&w.from) {
                errs.push(GraphError::at(w.to, format!("input {:?} is wired from a node that does not exist ({})", w.input, w.from)));
                continue;
            }
            if !ids.contains(&w.to) {
                errs.push(GraphError::at(w.from, format!("output {:?} is wired to a node that does not exist ({})", w.out, w.to)));
                continue;
            }
            if w.from == w.to {
                errs.push(GraphError::at(w.to, "a node feeds itself"));
            }
            if !seen_inputs.insert((w.to, w.input.clone())) {
                errs.push(GraphError::at(w.to, format!("input {:?} has more than one wire", w.input)));
            }
        }
        for e in &self.exposed {
            if !ids.contains(&e.node) {
                errs.push(GraphError::global(format!("exposed {:?} names a node that does not exist ({})", e.name, e.node)));
            }
        }
        if let Err(e) = self.topo() {
            errs.push(e);
        }
        if let Some(reg) = reg {
            let pins: BTreeMap<NodeId, Option<NodePins>> = self.nodes.iter().map(|n| (n.id, reg.pins(n))).collect();
            for n in &self.nodes {
                match &pins[&n.id] {
                    None => errs.push(GraphError::at(n.id, format!("unknown node kind {:?}", n.kind))),
                    Some(p) => {
                        if !p.modes.contains(&self.mode) {
                            errs.push(GraphError::at(n.id, format!("{:?} does not run in {:?} mode", n.kind, self.mode)));
                        }
                        for (name, lit) in &n.inputs {
                            match p.input(name) {
                                None => errs.push(GraphError::at(n.id, format!("no input named {name:?}"))),
                                Some(pin) => {
                                    let from = lit.kind();
                                    let ok = pin.kind.accepts(from)
                                        || (from == ValueKind::List && pin.access == Access::List)
                                        || from == ValueKind::List;
                                    if !ok {
                                        errs.push(GraphError::at(n.id, format!("input {name:?} takes {}, not {}", pin.kind.label(), from.label())));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for w in &self.wires {
                let (Some(Some(fp)), Some(Some(tp))) = (pins.get(&w.from), pins.get(&w.to)) else { continue };
                let out = fp.output(&w.out);
                let input = tp.input(&w.input);
                match (out, input) {
                    (None, _) => errs.push(GraphError::at(w.from, format!("no output named {:?}", w.out))),
                    (_, None) => errs.push(GraphError::at(w.to, format!("no input named {:?}", w.input))),
                    (Some(o), Some(i)) => {
                        if !i.kind.accepts(o.kind) && !(i.kind == ValueKind::List) {
                            errs.push(GraphError::at(
                                w.to,
                                format!("input {:?} takes {}, but {}.{} is {}", w.input, i.kind.label(), w.from, w.out, o.kind.label()),
                            ));
                        }
                    }
                }
            }
            for e in &self.exposed {
                if let Some(Some(p)) = pins.get(&e.node) {
                    if p.input(&e.input).is_none() {
                        errs.push(GraphError::at(e.node, format!("exposed {:?} names no input {:?}", e.name, e.input)));
                    }
                }
            }
        }
        errs
    }
}

/// Set `value` at an RFC 6901 pointer inside `root`, creating objects on
/// the way. Array indices must already exist.
pub fn set_pointer(root: &mut serde_json::Value, pointer: &str, value: serde_json::Value) -> Result<(), String> {
    if pointer.is_empty() {
        *root = value;
        return Ok(());
    }
    if !pointer.starts_with('/') {
        return Err(format!("pointer {pointer:?} must start with '/'"));
    }
    let tokens: Vec<String> = pointer[1..]
        .split('/')
        .map(|t| t.replace("~1", "/").replace("~0", "~"))
        .collect();
    let mut cur = root;
    for (i, tok) in tokens.iter().enumerate() {
        let last = i + 1 == tokens.len();
        if cur.is_null() {
            *cur = serde_json::Value::Object(Default::default());
        }
        match cur {
            serde_json::Value::Object(map) => {
                if last {
                    map.insert(tok.clone(), value);
                    return Ok(());
                }
                cur = map.entry(tok.clone()).or_insert(serde_json::Value::Null);
            }
            serde_json::Value::Array(items) => {
                let idx: usize = tok.parse().map_err(|_| format!("{tok:?} is not an array index"))?;
                let slot = items.get_mut(idx).ok_or_else(|| format!("index {idx} is past the end"))?;
                if last {
                    *slot = value;
                    return Ok(());
                }
                cur = slot;
            }
            other => return Err(format!("cannot descend into {other} at {tok:?}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Specs;
    impl PinLookup for Specs {
        fn pins(&self, node: &Node) -> Option<NodePins> {
            let pin = |n: &str, k: ValueKind, a: Access| PinInfo { name: n.into(), kind: k, access: a };
            Some(match node.kind.as_str() {
                "number" => NodePins { inputs: vec![pin("value", ValueKind::Number, Access::Item)], outputs: vec![pin("out", ValueKind::Number, Access::Item)], modes: vec![Mode::SandRing, Mode::Free] },
                "add" => NodePins {
                    inputs: vec![pin("a", ValueKind::Number, Access::Item), pin("b", ValueKind::Number, Access::Item)],
                    outputs: vec![pin("sum", ValueKind::Number, Access::Item)],
                    modes: vec![Mode::SandRing, Mode::Free],
                },
                "text" => NodePins { inputs: vec![pin("value", ValueKind::Text, Access::Item)], outputs: vec![pin("out", ValueKind::Text, Access::Item)], modes: vec![Mode::SandRing, Mode::Free] },
                "solid.box" => NodePins { inputs: vec![], outputs: vec![pin("solid", ValueKind::Solid, Access::Item)], modes: vec![Mode::Free] },
                _ => return None,
            })
        }
    }

    #[test]
    fn ids_are_stable_and_never_reused() {
        let mut g = Graph::default();
        let a = g.add("number").unwrap();
        let b = g.add("number").unwrap();
        let c = g.add("add").unwrap();
        assert_eq!((a, b, c), (NodeId(1), NodeId(2), NodeId(3)));
        g.connect(a, "out", c, "a").unwrap();
        g.connect(b, "out", c, "b").unwrap();
        g.remove(b).unwrap();
        assert_eq!(g.node(c).unwrap().id, c, "c keeps its id after b goes");
        assert_eq!(g.wires.len(), 1, "b's wire went with it");
        let d = g.add("number").unwrap();
        assert_eq!(d, NodeId(4), "ids are never reused");
        // A file with a stale counter still hands out fresh ids.
        g.next_id = 0;
        assert_eq!(g.add("number").unwrap(), NodeId(5));
    }

    #[test]
    fn one_wire_per_input_and_no_cycles() {
        let mut g = Graph::default();
        let a = g.add("number").unwrap();
        let b = g.add("number").unwrap();
        let c = g.add("add").unwrap();
        assert_eq!(g.connect(a, "out", c, "a").unwrap(), None);
        let displaced = g.connect(b, "out", c, "a").unwrap().expect("the first wire is displaced");
        assert_eq!(displaced.from, a);
        assert_eq!(g.wires.len(), 1);
        assert_eq!(g.wire_into(c, "a").unwrap().from, b);
        let back = g.connect(c, "sum", b, "value");
        assert!(back.is_err(), "c -> b would close a cycle through b -> c");
        assert!(g.connect(c, "sum", c, "a").is_err(), "no self wires");
        assert!(g.connect(NodeId(99), "out", c, "b").is_err());
        assert_eq!(g.disconnect(c, "a").unwrap().from, b);
        assert!(g.disconnect(c, "a").is_none());
    }

    #[test]
    fn topo_runs_every_wire_forward_and_names_a_cycle() {
        let mut g = Graph::default();
        let a = g.add("number").unwrap();
        let b = g.add("number").unwrap();
        let c = g.add("add").unwrap();
        let d = g.add("add").unwrap();
        g.connect(b, "out", d, "a").unwrap();
        g.connect(c, "sum", d, "b").unwrap();
        g.connect(a, "out", c, "a").unwrap();
        g.connect(b, "out", c, "b").unwrap();
        let order = g.topo().unwrap();
        let pos = |id: NodeId| order.iter().position(|&x| x == id).unwrap();
        for w in &g.wires {
            assert!(pos(w.from) < pos(w.to), "{w:?} runs backward in {order:?}");
        }
        assert_eq!(order, vec![a, b, c, d], "ties break by id");
        assert_eq!(g.upstream(d), vec![b, c, a]);
        assert_eq!(g.downstream(a), vec![c, d]);
        // A cycle smuggled in by hand is found by validate and topo.
        g.wires.push(Wire { from: d, out: "sum".into(), to: c, input: "a".into() });
        let e = g.topo().unwrap_err();
        assert!(e.message.contains("cycle"), "{e}");
        let errs = g.validate(None);
        assert!(errs.iter().any(|e| e.message.contains("cycle")), "{errs:?}");
        assert!(errs.iter().any(|e| e.message.contains("more than one wire")), "fan-in on c.a: {errs:?}");
    }

    #[test]
    fn validation_reads_kinds_pins_types_and_modes() {
        let mut g = Graph::default();
        let n = g.add("number").unwrap();
        let t = g.add("text").unwrap();
        let s = g.add("add").unwrap();
        let bogus = g.add("no.such").unwrap();
        let solid = g.add("solid.box").unwrap();
        g.connect(t, "out", s, "a").unwrap();
        g.connect(n, "out", s, "b").unwrap();
        g.set_input(n, "value", Literal::Number(2.0)).unwrap();
        g.set_input(n, "nope", Literal::Number(2.0)).unwrap();
        g.set_input(s, "a", Literal::Text("x".into())).unwrap();
        g.expose(n, "value", "Width").unwrap();
        g.expose(t, "missing", "Label").unwrap();
        g.wires.push(Wire { from: n, out: "nothing".into(), to: s, input: "b".into() });
        let errs = g.validate(Some(&Specs));
        let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        let has = |needle: &str| msgs.iter().any(|m| m.contains(needle));
        assert!(has("unknown node kind \"no.such\""), "{msgs:?}");
        assert!(has("does not run in SandRing"), "{msgs:?}");
        assert!(has("takes number, but #2.out is text"), "{msgs:?}");
        assert!(has("no input named \"nope\""), "{msgs:?}");
        assert!(has("input \"a\" takes number, not text"), "{msgs:?}");
        assert!(has("no output named \"nothing\""), "{msgs:?}");
        assert!(has("more than one wire"), "{msgs:?}");
        assert!(has("exposed \"Label\" names no input"), "{msgs:?}");
        let _ = (bogus, solid);
        // The same graph in Free mode keeps every error but the mode one.
        g.mode = Mode::Free;
        let errs2 = g.validate(Some(&Specs));
        assert_eq!(errs2.len(), errs.len() - 1, "{errs2:?}");
        // A clean graph validates clean.
        let mut clean = Graph::default();
        let a = clean.add("number").unwrap();
        let b = clean.add("number").unwrap();
        let c = clean.add("add").unwrap();
        clean.connect(a, "out", c, "a").unwrap();
        clean.connect(b, "out", c, "b").unwrap();
        clean.set_input(a, "value", Literal::Int(1)).unwrap();
        assert!(clean.validate(Some(&Specs)).is_empty());
    }

    #[test]
    fn params_take_pointers_and_files_round_trip() {
        let mut g = Graph::new("Court band", Mode::SandRing);
        let n = g.add("script").unwrap();
        g.set_param(n, "/source", serde_json::json!("h = a * 2")).unwrap();
        // Objects are created along the way; "0" is a key until an array is there.
        g.set_param(n, "/pins/in/0", serde_json::json!("a")).unwrap();
        assert_eq!(g.param(n, "/pins/in"), Some(&serde_json::json!({"0": "a"})));
        g.set_param(n, "/pins/in", serde_json::json!(["a"])).unwrap();
        g.set_param(n, "/pins/in/0", serde_json::json!("b")).unwrap();
        assert_eq!(g.param(n, "/pins/in/0"), Some(&serde_json::json!("b")));
        assert!(g.set_param(n, "/pins/in/5", serde_json::json!("z")).is_err(), "past the end of an array");
        assert!(g.set_param(n, "pins", serde_json::json!(1)).is_err(), "a pointer starts with '/'");
        assert_eq!(g.param(n, "/source").unwrap(), "h = a * 2");
        g.set_input(n, "a", Literal::Number(1.5)).unwrap();
        g.node_mut(n).unwrap().pos = [10.0, 20.0];
        g.node_mut(n).unwrap().label = Some("double".into());
        g.expose(n, "a", "A").unwrap();
        let text = serde_json::to_string_pretty(&g).unwrap();
        let back: Graph = serde_json::from_str(&text).unwrap();
        assert_eq!(back, g);
        assert!(text.contains("\"a\": 1.5"), "{text}");
        assert!(!text.contains("\"label\": null"), "absent fields stay absent");
        // A minimal file reads with defaults.
        let min: Graph = serde_json::from_str(r#"{"name":"x","nodes":[{"id":7,"kind":"number"}]}"#).unwrap();
        assert_eq!(min.mode, Mode::SandRing);
        assert_eq!(min.nodes[0].id, NodeId(7));
        let mut min = min;
        assert_eq!(min.add("number").unwrap(), NodeId(8), "fresh ids stay above a file's");
    }

    #[test]
    fn exposure_is_unique_by_name_and_follows_removal() {
        let mut g = Graph::default();
        let a = g.add("number").unwrap();
        let b = g.add("number").unwrap();
        g.expose(a, "value", "Width").unwrap();
        g.expose(a, "value", "Width").unwrap();
        assert!(g.expose(b, "value", "Width").is_err(), "a name exposes one input");
        g.expose(a, "value", "W").unwrap();
        assert_eq!(g.exposed.len(), 1, "re-exposing an input renames it");
        assert_eq!(g.exposed[0].name, "W");
        g.remove(a).unwrap();
        assert!(g.exposed.is_empty());
        assert!(g.unexpose("W").is_none());
    }

    #[test]
    fn the_node_cap_holds() {
        let mut g = Graph::default();
        for _ in 0..MAX_NODES {
            g.add("number").unwrap();
        }
        assert!(g.add("number").is_err());
    }
}
