//! What a node accounts for on the ring.
//!
//! A graph of two hundred nodes evaluates to one design, and the design does
//! not say which node put which metal where. [`effects`] reads it back: a
//! layer node, its entry, its window and the alpha it names all resolve to
//! the same path in the evaluated stack; a head node to the head; a section
//! or shank node to the whole band; a node that only moves build or casting
//! settings to nothing on the metal at all. A host turns that into a
//! highlight on the ring, which is how a node says what it is doing, and
//! where.
//!
//! Resolution is by what a node *produces*, read from the registry's pins,
//! so a kind added later classifies itself. Entries are matched into the
//! evaluated design by value, because a position in a chain of `stack`
//! nodes says nothing once a list or a group is involved.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ringdesign_core::RingDesign;
use ringdesign_core::field::{Layer, LayerEntry, LayerStack};

use crate::graph::{Graph, Node, NodeId};
use crate::registry::Registry;
use crate::value::{Value, ValueKind};

/// How much of the ring a node reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// Feeds nothing that lands on the ring.
    #[default]
    Nothing,
    /// Build, casting or naming fields: no metal moves.
    Settings,
    /// One or more layers of the stack.
    Layers,
    /// The signet head and its shoulders.
    Head,
    /// The section, the shank or the design itself.
    Band,
}

/// A path into `design.layers`: a top-level index, then indices inside groups.
pub type LayerPath = Vec<usize>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeEffect {
    pub scope: Scope,
    pub layers: Vec<LayerPath>,
    /// The layers' names, in the order of `layers`.
    pub names: Vec<String>,
}

impl NodeEffect {
    fn of(scope: Scope) -> Self {
        Self { scope, ..Self::default() }
    }

    /// What the node does, in words.
    pub fn summary(&self) -> String {
        match self.scope {
            Scope::Nothing => "Feeds nothing on the ring yet".into(),
            Scope::Settings => "Build or casting settings; no metal moves".into(),
            Scope::Head => "Shapes the signet head and its shoulders".into(),
            Scope::Band => "Shapes the whole band".into(),
            Scope::Layers => match self.names.as_slice() {
                [] => "Shapes a layer".into(),
                [one] => format!("Shapes the layer \u{201c}{one}\u{201d}"),
                many => {
                    let shown: Vec<&str> = many.iter().take(3).map(String::as_str).collect();
                    let more = many.len().saturating_sub(shown.len());
                    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
                    format!("Shapes {} layers: {}{tail}", many.len(), shown.join(", "))
                }
            },
        }
    }

    fn absorb(&mut self, other: &NodeEffect) {
        self.scope = self.scope.max(other.scope);
        for (path, name) in other.layers.iter().zip(&other.names) {
            if !self.layers.contains(path) {
                self.layers.push(path.clone());
                self.names.push(name.clone());
            }
        }
    }
}

/// An opacity that moves no metal and still counts as present.
pub const MUTED_OPACITY: f64 = 1e-9;

/// The design with the layers at `paths` muted, for a before/after: still
/// in the stack, moving no metal. Muted rather than switched off because a
/// build may size itself by the layers it carries — an imported base
/// subdivides where enabled layers have footprints — and a before/after is
/// only vertex for vertex if both builds cut the same mesh.
pub fn without_layers(design: &RingDesign, paths: &[LayerPath]) -> RingDesign {
    let mut d = design.clone();
    for path in paths {
        if let Some(e) = entry_mut(&mut d.layers, path) {
            e.opacity = e.opacity.min(MUTED_OPACITY);
        }
    }
    d
}

fn entry_mut<'a>(stack: &'a mut LayerStack, path: &[usize]) -> Option<&'a mut LayerEntry> {
    let (first, rest) = path.split_first()?;
    let e = stack.layers.get_mut(*first)?;
    if rest.is_empty() {
        return Some(e);
    }
    match &mut e.layer {
        Layer::Group(g) => entry_mut(&mut g.stack, rest),
        _ => None,
    }
}

/// Every entry of the stack, groups included, with its path.
fn walk<'a>(stack: &'a LayerStack, prefix: &mut LayerPath, out: &mut Vec<(LayerPath, &'a LayerEntry)>) {
    for (i, e) in stack.layers.iter().enumerate() {
        prefix.push(i);
        out.push((prefix.clone(), e));
        if let Layer::Group(g) = &e.layer {
            walk(&g.stack, prefix, out);
        }
        prefix.pop();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Entry,
    Stack,
    LayerPart,
    Alpha,
    Head,
    Band,
    Set,
    Pass,
}

fn classify(node: &Node, reg: &Registry) -> Class {
    if node.kind == "design.set" {
        return Class::Set;
    }
    if node.kind == "stack" {
        return Class::Stack;
    }
    if node.kind.starts_with("sink.") {
        return Class::Band;
    }
    // These hand on a shank, but the head is what they are for.
    if matches!(node.kind.as_str(), "shank.signet" | "shank.add_head" | "shank.outline") {
        return Class::Head;
    }
    let outs: Vec<ValueKind> = reg.node_pins(node).map(|(_, o)| o.iter().map(|p| p.kind).collect()).unwrap_or_default();
    let has = |k: ValueKind| outs.contains(&k);
    if has(ValueKind::Entry) {
        Class::Entry
    } else if has(ValueKind::Layer) || has(ValueKind::Window) || has(ValueKind::Remap) || has(ValueKind::Gem) || has(ValueKind::Recipe) {
        Class::LayerPart
    } else if has(ValueKind::AlphaSource) || has(ValueKind::AlphaRef) {
        Class::Alpha
    } else if has(ValueKind::Head) || has(ValueKind::Outline) {
        Class::Head
    } else if has(ValueKind::Profile) || has(ValueKind::Shank) || has(ValueKind::Design) {
        Class::Band
    } else {
        Class::Pass
    }
}

/// The reach of a `design.set` pointer.
fn pointer_effect(pointer: &str, entries: &[(LayerPath, &LayerEntry)]) -> NodeEffect {
    let tokens: Vec<&str> = pointer.split('/').filter(|t| !t.is_empty()).collect();
    match tokens.first().copied() {
        Some("layers") => {
            let mut path = LayerPath::new();
            let mut after_layers = false;
            for t in &tokens {
                match (after_layers, t.parse::<usize>()) {
                    (true, Ok(i)) => path.push(i),
                    _ => {}
                }
                after_layers = *t == "layers";
            }
            match entries.iter().find(|(p, _)| *p == path) {
                Some((p, e)) => NodeEffect { scope: Scope::Layers, layers: vec![p.clone()], names: vec![e.name.clone()] },
                None => NodeEffect::of(Scope::Band),
            }
        }
        Some("shank") => match tokens.get(1).copied() {
            Some("head" | "extra_heads" | "custom_outlines") => NodeEffect::of(Scope::Head),
            _ => NodeEffect::of(Scope::Band),
        },
        Some("profile" | "size" | "imported_base") => NodeEffect::of(Scope::Band),
        Some(_) => NodeEffect::of(Scope::Settings),
        None => NodeEffect::of(Scope::Nothing),
    }
}

fn mentions(v: &serde_json::Value, name: &str) -> bool {
    match v {
        serde_json::Value::String(s) => s == name,
        serde_json::Value::Array(a) => a.iter().any(|x| mentions(x, name)),
        serde_json::Value::Object(o) => o.values().any(|x| mentions(x, name)),
        _ => false,
    }
}

struct Resolver<'a> {
    g: &'a Graph,
    reg: &'a Registry,
    values: &'a BTreeMap<NodeId, BTreeMap<String, Value>>,
    entries: Vec<(LayerPath, &'a LayerEntry)>,
    /// Each entry as JSON with its own name removed, for alpha references.
    entry_json: Vec<serde_json::Value>,
    memo: BTreeMap<NodeId, NodeEffect>,
}

impl Resolver<'_> {
    fn layers_effect(&self, hits: impl IntoIterator<Item = usize>) -> NodeEffect {
        let mut e = NodeEffect::default();
        for i in hits {
            let (path, entry) = &self.entries[i];
            if !e.layers.contains(path) {
                e.layers.push(path.clone());
                e.names.push(entry.name.clone());
            }
        }
        if !e.layers.is_empty() {
            e.scope = Scope::Layers;
        }
        e
    }

    /// Where the entry a node produced sits in the evaluated stack.
    fn place_entry(&self, node: &Node) -> NodeEffect {
        let produced: Vec<&LayerEntry> = self
            .values
            .get(&node.id)
            .map(|outs| {
                outs.values()
                    .flat_map(|v| match v {
                        Value::Entry(e) => vec![&**e],
                        Value::List(items) => items.iter().filter_map(|x| if let Value::Entry(e) = x { Some(&**e) } else { None }).collect(),
                        _ => Vec::new(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut hits = Vec::new();
        if produced.is_empty() {
            if let Some(name) = node.inputs.get("name").and_then(|l| serde_json::to_value(l).ok()).and_then(|v| v.as_str().map(str::to_owned)) {
                hits.extend(self.entries.iter().position(|(_, e)| e.name == name));
            }
        }
        for want in produced {
            let named: Vec<usize> = self.entries.iter().enumerate().filter(|(_, (_, e))| e.name == want.name).map(|(i, _)| i).collect();
            let pick = match named.as_slice() {
                [] => None,
                [one] => Some(*one),
                several => {
                    let json = serde_json::to_value(want).ok();
                    several
                        .iter()
                        .copied()
                        .find(|&i| !hits.contains(&i) && serde_json::to_value(self.entries[i].1).ok() == json)
                        .or_else(|| several.iter().copied().find(|i| !hits.contains(i)))
                }
            };
            hits.extend(pick);
        }
        self.layers_effect(hits)
    }

    fn alpha_effect(&self, node: &Node) -> NodeEffect {
        let mut names: BTreeSet<String> = BTreeSet::new();
        if let Some(outs) = self.values.get(&node.id) {
            for v in outs.values() {
                match v {
                    Value::AlphaRef(n) => {
                        names.insert(n.clone());
                    }
                    Value::AlphaSource(src) => {
                        names.insert(src.name().to_string());
                    }
                    _ => {}
                }
            }
        }
        if names.is_empty() {
            if let Some(n) = node.inputs.get("name").and_then(|l| serde_json::to_value(l).ok()).and_then(|v| v.as_str().map(str::to_owned)) {
                names.insert(n);
            }
        }
        let hits: Vec<usize> = self.entry_json.iter().enumerate().filter(|(_, j)| names.iter().any(|n| mentions(j, n))).map(|(i, _)| i).collect();
        self.layers_effect(hits)
    }

    /// The union of what the nearest classified consumers downstream do.
    fn downstream_effect(&mut self, id: NodeId, only_entries: bool) -> NodeEffect {
        let mut out = NodeEffect::default();
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<NodeId> = self.g.wires_from(id).map(|w| w.to).collect();
        while let Some(n) = queue.pop_front() {
            if !seen.insert(n) {
                continue;
            }
            let Some(node) = self.g.node(n) else { continue };
            let class = classify(node, self.reg);
            let stop = if only_entries { matches!(class, Class::Entry | Class::Stack | Class::Band | Class::Set) } else { class != Class::Pass };
            if stop {
                if !only_entries || class == Class::Entry {
                    let e = self.effect(n);
                    out.absorb(&e);
                }
            } else {
                queue.extend(self.g.wires_from(n).map(|w| w.to));
            }
        }
        out
    }

    fn effect(&mut self, id: NodeId) -> NodeEffect {
        if let Some(e) = self.memo.get(&id) {
            return e.clone();
        }
        let Some(node) = self.g.node(id) else { return NodeEffect::default() };
        let e = match classify(node, self.reg) {
            Class::Entry => self.place_entry(node),
            Class::LayerPart => self.downstream_effect(id, true),
            Class::Stack => {
                let feeders: Vec<NodeId> = self.g.wires_into(id).filter(|w| w.input == "entries").map(|w| w.from).collect();
                let mut e = NodeEffect::default();
                for f in feeders {
                    let mut chain = vec![f];
                    chain.extend(self.g.upstream(f));
                    for n in chain {
                        if self.g.node(n).is_some_and(|x| classify(x, self.reg) == Class::Entry) {
                            let sub = self.effect(n);
                            e.absorb(&sub);
                        }
                    }
                }
                e
            }
            Class::Alpha => self.alpha_effect(node),
            Class::Head => NodeEffect::of(Scope::Head),
            Class::Band => NodeEffect::of(Scope::Band),
            Class::Set => {
                let pointer = node.inputs.get("pointer").and_then(|l| serde_json::to_value(l).ok()).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
                pointer_effect(&pointer, &self.entries)
            }
            Class::Pass => self.downstream_effect(id, false),
        };
        self.memo.insert(id, e.clone());
        e
    }
}

/// Every node's reach on the evaluated `design`. `values` are the
/// evaluation's outputs; without them entries fall back to their names.
pub fn effects(g: &Graph, reg: &Registry, values: &BTreeMap<NodeId, BTreeMap<String, Value>>, design: &RingDesign) -> BTreeMap<NodeId, NodeEffect> {
    let mut entries = Vec::new();
    walk(&design.layers, &mut LayerPath::new(), &mut entries);
    let entry_json = entries
        .iter()
        .map(|(_, e)| {
            let mut j = serde_json::to_value(e).unwrap_or_default();
            if let Some(o) = j.as_object_mut() {
                o.remove("name");
            }
            j
        })
        .collect();
    let mut r = Resolver { g, reg, values, entries, entry_json, memo: BTreeMap::new() };
    g.nodes.iter().map(|n| (n.id, r.effect(n.id))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, evaluate_design};
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> (Registry, crate::eval::DesignOut) {
        let reg = Registry::builtin();
        let out = evaluate_design(&mut Evaluator::new(), g, &reg, &AlphaLibrary::builtin(), 0).expect("evaluates");
        (reg, out)
    }

    #[test]
    fn a_layer_its_entry_and_its_window_land_on_the_same_path() {
        let g = crate::templates::graph("Braided band").expect("bundled");
        let (reg, out) = run(&g);
        let fx = effects(&g, &reg, &out.report.values, &out.design);
        let entries = g.entry_nodes();
        assert_eq!(entries.len(), 2);
        let mut seen = Vec::new();
        for e in &entries {
            let eff = &fx[e];
            assert_eq!(eff.scope, Scope::Layers, "{eff:?}");
            assert_eq!(eff.layers.len(), 1);
            for w in g.wires_into(*e) {
                assert_eq!(fx[&w.from].layers, eff.layers, "{} feeds the entry and shares its layer", g.node(w.from).unwrap().kind);
            }
            seen.push(eff.layers[0].clone());
        }
        seen.sort();
        assert_eq!(seen, vec![vec![0], vec![1]], "both layers of the stack are accounted for");
        let kind = |k: &str| g.nodes.iter().find(|n| n.kind == k).unwrap().id;
        assert_eq!(fx[&kind("band.profile")].scope, Scope::Band);
        assert_eq!(fx[&kind("sink.output")].scope, Scope::Band);
        // The fit node's number runs through math into the tiling: a pass-through reaches the layer.
        assert_eq!(fx[&kind("math.mul")].scope, Scope::Layers);
        assert!(fx[&kind("math.mul")].summary().contains("layer"));
    }

    #[test]
    fn every_bundled_graph_accounts_for_every_layer() {
        let lib = AlphaLibrary::builtin();
        let reg = Registry::builtin();
        for t in crate::templates::catalog() {
            let g = t.load();
            let out = evaluate_design(&mut Evaluator::new(), &g, &reg, &lib, 0).unwrap_or_else(|e| panic!("{}: {e}", t.name));
            let fx = effects(&g, &reg, &out.report.values, &out.design);
            assert_eq!(fx.len(), g.nodes.len());
            let mut all = Vec::new();
            walk(&out.design.layers, &mut LayerPath::new(), &mut all);
            let reached: BTreeSet<LayerPath> = g.nodes.iter().filter(|n| n.kind == "entry").flat_map(|n| fx[&n.id].layers.clone()).collect();
            for (path, e) in &all {
                assert!(reached.contains(path), "{}: no entry node accounts for {:?} at {path:?}", t.name, e.name);
            }
            for n in &g.nodes {
                let eff = &fx[&n.id];
                match n.kind.as_str() {
                    "head" => assert_eq!(eff.scope, Scope::Head, "{}", t.name),
                    "band.profile" | "shank" | "design.new" => assert_eq!(eff.scope, Scope::Band, "{}", t.name),
                    "window" | "entry" => assert_eq!((eff.scope, eff.layers.len()), (Scope::Layers, 1), "{}: {:?}", t.name, n.label),
                    _ => {}
                }
                assert!(!eff.summary().is_empty());
            }
        }
    }

    #[test]
    fn artwork_reaches_the_layers_that_name_it_and_settings_reach_none() {
        let g = crate::templates::graph("Solstice").or_else(|| crate::templates::catalog().find(|t| t.slug.contains("solstice")).map(|t| t.load())).expect("a showcase graph");
        let (reg, out) = run(&g);
        let fx = effects(&g, &reg, &out.report.values, &out.design);
        let alphas: Vec<&Node> = g.nodes.iter().filter(|n| n.kind.starts_with("alpha.")).collect();
        assert!(!alphas.is_empty());
        assert!(alphas.iter().any(|n| fx[&n.id].scope == Scope::Layers), "some artwork is used by a layer");
        for n in g.nodes.iter().filter(|n| n.kind == "design.set") {
            let pointer = serde_json::to_value(&n.inputs["pointer"]).unwrap();
            let pointer = pointer.as_str().unwrap();
            let want = if pointer.starts_with("/build") || pointer.starts_with("/draft") || pointer.starts_with("/manufacturing") { Scope::Settings } else { fx[&n.id].scope };
            assert_eq!(fx[&n.id].scope, want, "{pointer}");
        }
        // Switching a node's layers off is a different design only there.
        let some = g.nodes.iter().find(|n| n.kind == "entry").unwrap();
        let paths = &fx[&some.id].layers;
        let off = without_layers(&out.design, paths);
        let muted = entry_mut(&mut off.layers.clone(), &paths[0]).unwrap().clone();
        assert!(muted.enabled && muted.opacity <= MUTED_OPACITY, "muted, not removed: a build still sizes itself by it");
        assert_eq!(off.layers.layers.len(), out.design.layers.layers.len());
    }

    #[test]
    fn a_pointer_names_its_layer_and_a_group_child_keeps_its_depth() {
        let mut d = RingDesign::default();
        let inner = LayerEntry::new("bead", Layer::Milgrain(Default::default()));
        let group = LayerEntry::new("cluster", Layer::Group(ringdesign_core::field::GroupLayer { stack: LayerStack { layers: vec![inner] }, recipe: None }));
        d.layers.layers = vec![LayerEntry::new("rail", Layer::Border(Default::default())), group];
        let mut all = Vec::new();
        walk(&d.layers, &mut LayerPath::new(), &mut all);
        let paths: Vec<LayerPath> = all.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(paths, vec![vec![0], vec![1], vec![1, 0]]);
        assert_eq!(pointer_effect("/layers/layers/1/enabled", &all).names, vec!["cluster"]);
        assert_eq!(pointer_effect("/layers/layers/1/layer/Group/stack/layers/0/opacity", &all).layers, vec![vec![1, 0]]);
        assert_eq!(pointer_effect("/shank/head/rise_mm", &all).scope, Scope::Head);
        assert_eq!(pointer_effect("/profile/width_mm", &all).scope, Scope::Band);
        assert_eq!(pointer_effect("/build/theta_steps", &all).scope, Scope::Settings);
        let off = without_layers(&d, &[vec![1, 0]]);
        let Layer::Group(g) = &off.layers.layers[1].layer else { panic!() };
        assert!(g.stack.layers[0].opacity <= MUTED_OPACITY && off.layers.layers[1].opacity == 1.0);
    }
}
