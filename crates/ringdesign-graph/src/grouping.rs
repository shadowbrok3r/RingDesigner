//! Folds a graph's repeated chains (layers, stamps, keyframes, parts, settings) into collapsed groups.

use std::collections::{BTreeMap, BTreeSet};

use crate::graph::{Graph, NodeGroup, NodeId};
use crate::value::Literal;

/// A group's starting node, whether it is a member, and which of its inputs lead into the group.
struct Anchor {
    node: NodeId,
    included: bool,
    only: Option<&'static str>,
    skip: &'static [&'static str],
    name: String,
}

impl Graph {
    /// Adds a collapsed group per ungrouped layer, stamp list, keyframe list, part chain, artwork list and settings block; returns how many.
    pub fn auto_group(&mut self) -> usize {
        let Ok(order) = self.topo() else { return 0 };
        let rank: BTreeMap<NodeId, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut taken: BTreeSet<NodeId> = self.groups.iter().flat_map(|g| g.nodes.iter().copied()).collect();
        let mut added = Vec::new();
        for id in &order {
            if taken.contains(id) {
                continue;
            }
            if let Some(run) = self.cad_run(*id, &taken) {
                let members = self.owned(&run, &["design"], &[], &taken, &rank);
                if members.len() >= 2 {
                    taken.extend(&members);
                    added.push(collapsed(self.run_name(&run), members, &rank));
                }
                continue;
            }
            let Some(anchor) = self.anchor(*id) else { continue };
            let roots: Vec<NodeId> = if anchor.included {
                vec![anchor.node]
            } else {
                self.wires_into(anchor.node).filter(|w| anchor.only.is_none_or(|o| o == w.input)).map(|w| w.from).collect()
            };
            if roots.iter().any(|r| taken.contains(r)) {
                continue;
            }
            let follow = if anchor.included { anchor.only.map(|o| vec![o]).unwrap_or_default() } else { Vec::new() };
            let members = self.owned(&roots, anchor.skip, &follow, &taken, &rank);
            if members.len() < 2 {
                continue;
            }
            let name = if anchor.name.is_empty() { self.layer_name(&members) } else { anchor.name };
            taken.extend(&members);
            added.push(collapsed(name, members, &rank));
        }
        let n = added.len();
        self.groups.extend(added);
        n
    }

    fn anchor(&self, id: NodeId) -> Option<Anchor> {
        let node = self.node(id)?;
        let label = || node.label.clone().unwrap_or_default();
        let a = |included, only, skip, name: String| Some(Anchor { node: id, included, only, skip, name });
        match node.kind.as_str() {
            "stack" => a(true, None, &["stack"], String::new()),
            "design.stamps" => a(true, None, &["design"], "Stamps".into()),
            "design.settings" => a(true, None, &["design"], "Settings".into()),
            "shank" if self.wire_into(id, "keys").is_some() => {
                a(true, Some("keys"), &[], if label().is_empty() { "Shank keyframes".into() } else { label() })
            }
            "design.assemble" if self.wire_into(id, "alphas").is_some() => a(false, Some("alphas"), &[], "Artwork".into()),
            _ => None,
        }
    }

    /// The chain of `cad.feature` nodes starting at `id`, each feeding the next its design.
    fn cad_run(&self, id: NodeId, taken: &BTreeSet<NodeId>) -> Option<Vec<NodeId>> {
        let is_feature = |id: NodeId| self.node(id).is_some_and(|n| n.kind == "cad.feature");
        if !is_feature(id) || self.wire_into(id, "design").is_some_and(|w| is_feature(w.from)) {
            return None;
        }
        let mut run = vec![id];
        let mut at = id;
        loop {
            let mut next = self.wires_from(at).filter(|w| w.out == "design");
            let (Some(w), None) = (next.next(), next.next()) else { break };
            if !is_feature(w.to) || w.input != "design" || taken.contains(&w.to) {
                break;
            }
            at = w.to;
            run.push(at);
        }
        Some(run)
    }

    /// `roots` plus every untaken upstream node read only by members, not through a root's `skip` inputs.
    fn owned(&self, roots: &[NodeId], skip: &[&str], only: &[&str], taken: &BTreeSet<NodeId>, rank: &BTreeMap<NodeId, usize>) -> Vec<NodeId> {
        let mut members: BTreeSet<NodeId> = roots.iter().copied().filter(|r| !taken.contains(r)).collect();
        // Highest topological rank first, so readers are decided before their sources.
        let mut queue: BTreeSet<(std::cmp::Reverse<usize>, NodeId)> = BTreeSet::new();
        for r in &members {
            for w in self.wires_into(*r) {
                let crossing = skip.contains(&w.input.as_str()) || (!only.is_empty() && !only.contains(&w.input.as_str()));
                let chained = roots.contains(&w.from);
                if !crossing && !chained {
                    queue.insert((std::cmp::Reverse(rank.get(&w.from).copied().unwrap_or(0)), w.from));
                }
            }
        }
        while let Some(item) = queue.pop_first() {
            let id = item.1;
            if members.contains(&id) || taken.contains(&id) {
                continue;
            }
            if !self.wires_from(id).all(|w| members.contains(&w.to)) {
                continue;
            }
            members.insert(id);
            for w in self.wires_into(id) {
                queue.insert((std::cmp::Reverse(rank.get(&w.from).copied().unwrap_or(0)), w.from));
            }
        }
        let mut members: Vec<NodeId> = members.into_iter().collect();
        members.sort_by_key(|id| rank.get(id).copied().unwrap_or(usize::MAX));
        members
    }

    /// A layer group's name: its entries' names, else its layer's label.
    fn layer_name(&self, members: &[NodeId]) -> String {
        let named = |id: &NodeId| {
            let n = self.node(*id)?;
            match n.inputs.get("name") {
                Some(Literal::Text(t)) if !t.trim().is_empty() => Some(t.trim().to_string()),
                _ => n.label.as_deref().map(|l| l.split(" / ").next().unwrap_or(l).trim().to_string()).filter(|l| !l.is_empty()),
            }
        };
        let entries: Vec<String> = members.iter().filter(|id| self.node(**id).is_some_and(|n| n.kind == "entry")).filter_map(named).collect();
        match entries.as_slice() {
            [one] => one.clone(),
            [first, rest @ ..] if !rest.is_empty() => format!("{first} + {} more", rest.len()),
            _ => members
                .iter()
                .filter(|id| self.node(**id).is_some_and(|n| n.kind.starts_with("layer.")))
                .find_map(named)
                .unwrap_or_else(|| "Layer".into()),
        }
    }

    fn run_name(&self, run: &[NodeId]) -> String {
        if run.len() == 1 { "Part".into() } else { format!("Parts ({})", run.len()) }
    }
}

fn collapsed(name: String, nodes: Vec<NodeId>, rank: &BTreeMap<NodeId, usize>) -> NodeGroup {
    let mut nodes = nodes;
    nodes.sort_by_key(|id| rank.get(id).copied().unwrap_or(usize::MAX));
    NodeGroup { collapsed: true, ..NodeGroup::new(name, nodes) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;

    fn kinds(g: &Graph, group: &NodeGroup) -> Vec<String> {
        group.nodes.iter().map(|id| g.node(*id).unwrap().kind.clone()).collect()
    }

    #[test]
    fn each_layer_becomes_a_group_and_shared_sources_stay_out() {
        let mut g = crate::templates::graph("Gypsy trio").unwrap();
        g.groups.clear();
        let before = g.clone();
        let added = g.auto_group();
        let names: Vec<&str> = g.groups.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Centre", "Right", "Left", "Settings"]);
        assert_eq!(added, 4);
        let mut first = kinds(&g, &g.groups[0]);
        first.sort();
        assert_eq!(first, ["entry", "gem", "layer.seat", "stack", "window"]);
        assert!(g.groups.iter().all(|x| x.collapsed));
        // Nothing else moved, and the graph still evaluates to the same design.
        assert_eq!((g.nodes.clone(), g.wires.clone()), (before.nodes.clone(), before.wires.clone()));
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let run = |g: &Graph| serde_json::to_string(&*crate::eval::evaluate_design(&mut crate::eval::Evaluator::new(), g, &reg, &lib, 0).unwrap().design).unwrap();
        assert_eq!(run(&g), run(&before));
        // A second pass adds nothing.
        assert_eq!(g.auto_group(), 0);
    }

    #[test]
    fn a_source_two_layers_read_is_left_between_them() {
        let mut g = Graph::default();
        let shared = g.add("alpha.proc").unwrap();
        let mut last = None;
        for name in ["A", "B"] {
            let layer = g.add("layer.tiling").unwrap();
            let entry = g.add("entry").unwrap();
            let stack = g.add("stack").unwrap();
            g.set_input(entry, "name", Literal::Text(name.into())).unwrap();
            g.connect(shared, "source", layer, "alpha").ok();
            g.connect(layer, "layer", entry, "layer").unwrap();
            g.connect(entry, "entry", stack, "entries").unwrap();
            if let Some(prev) = last {
                g.connect(prev, "stack", stack, "stack").unwrap();
            }
            last = Some(stack);
        }
        g.auto_group();
        assert_eq!(g.groups.len(), 2);
        assert!(g.groups.iter().all(|x| !x.nodes.contains(&shared)), "{:?}", g.groups);
        assert!(g.groups.iter().all(|x| x.nodes.len() == 3));
    }

    #[test]
    fn a_chain_of_cad_features_is_one_group() {
        let mut g = crate::templates::graph("Trilogy").unwrap();
        g.groups.clear();
        g.auto_group();
        let parts = g.groups.iter().find(|x| x.name.starts_with("Parts")).expect("a parts group");
        assert_eq!(parts.nodes.iter().filter(|id| g.node(**id).unwrap().kind == "cad.feature").count(), 10);
        assert!(parts.nodes.iter().all(|id| g.node(*id).unwrap().kind.starts_with("cad.")));
    }
}

#[cfg(test)]
mod catalog_tests {
    #[test]
    fn every_large_template_opens_as_a_short_chain() {
        for (name, g) in crate::templates::all() {
            let grouped: std::collections::BTreeSet<_> = g.groups.iter().filter(|x| x.collapsed).flat_map(|x| x.nodes.iter().copied()).collect();
            let top = g.nodes.len() - grouped.len() + g.groups.iter().filter(|x| x.collapsed).count();
            eprintln!("{name:45} {:4} nodes -> {:3} on top ({} groups)", g.nodes.len(), top, g.groups.len());
            if g.nodes.len() >= 40 {
                assert!(top * 3 <= g.nodes.len(), "{name}: {top} of {} nodes still on top", g.nodes.len());
            }
        }
    }
}
