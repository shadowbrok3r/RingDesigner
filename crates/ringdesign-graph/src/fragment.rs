//! Portable selections: internal wires survive; external connections never do.
use crate::{
    graph::{Graph, GraphError, Node, NodeGroup, NodeId, Wire},
    registry::Registry,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fragment {
    pub format: String,
    pub nodes: Vec<Node>,
    pub wires: Vec<Wire>,
    #[serde(default)]
    pub groups: Vec<NodeGroup>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Literal;

    #[test]
    fn copied_selection_keeps_internal_wires_values_and_frames_with_fresh_ids() {
        let reg = Registry::builtin();
        let mut graph = Graph::default();
        let a = graph.add("math.add").unwrap();
        let b = graph.add("math.mul").unwrap();
        let outside = graph.add("math.sub").unwrap();
        graph
            .node_mut(a)
            .unwrap()
            .inputs
            .insert("a".into(), Literal::Number(3.5));
        graph.node_mut(a).unwrap().pos = [10., 20.];
        graph.node_mut(b).unwrap().pos = [210., 80.];
        graph.connect(a, "out", b, "a").unwrap();
        graph.connect(b, "out", outside, "a").unwrap();
        graph.groups.push(NodeGroup {
            name: "Sizing".into(),
            nodes: vec![a, b],
        });
        let fragment = Fragment::capture(&graph, &[a, b]);
        let fragment: Fragment =
            serde_json::from_str(&serde_json::to_string(&fragment).unwrap()).unwrap();
        let pasted = fragment.paste(&mut graph, [50., 100.], &reg).unwrap();
        assert_eq!(pasted.len(), 2);
        assert!(pasted.iter().all(|id| ![a, b, outside].contains(id)));
        assert_eq!(
            graph.node(pasted[0]).unwrap().inputs["a"],
            Literal::Number(3.5)
        );
        assert_eq!(graph.node(pasted[1]).unwrap().pos, [250., 160.]);
        assert_eq!(graph.wire_into(pasted[1], "a").unwrap().from, pasted[0]);
        assert_eq!(graph.wires.len(), 3, "external wire is not copied");
        assert_eq!(graph.groups[1].nodes, pasted);
        assert!(graph.validate(Some(&reg)).is_empty());
    }

    #[test]
    fn invalid_clipboard_never_partially_edits_the_graph() {
        let reg = Registry::builtin();
        let mut graph = crate::templates::simple();
        let before = graph.clone();
        let mut f = Fragment::capture(&graph, &[graph.nodes[0].id, graph.nodes[1].id]);
        f.nodes[1].kind = "not.a.node".into();
        assert!(f.paste(&mut graph, [0., 0.], &reg).is_err());
        assert_eq!(graph, before);
        f.nodes[1].kind = "shank".into();
        f.wires.push(Wire {
            from: f.nodes[0].id,
            out: "profile".into(),
            to: f.nodes[1].id,
            input: "amount".into(),
        });
        assert!(f.paste(&mut graph, [0., 0.], &reg).is_err());
        assert_eq!(graph, before);
    }

    #[test]
    fn collapse_and_delete_preserve_control_limits_and_group_membership() {
        let mut graph = crate::templates::simple();
        let id = graph
            .exposed
            .iter()
            .find(|e| e.name == "Width")
            .unwrap()
            .node;
        graph
            .exposed
            .iter_mut()
            .find(|e| e.name == "Width")
            .unwrap()
            .range = Some([2., 12.]);
        graph.groups.push(NodeGroup {
            name: "Band".into(),
            nodes: vec![id],
        });
        let cluster = graph.collapse(&[id], "Profile").unwrap();
        assert_eq!(
            graph
                .exposed
                .iter()
                .find(|e| e.name == "Width")
                .unwrap()
                .range,
            Some([2., 12.])
        );
        assert_eq!(graph.groups[0].nodes, [cluster]);
        assert!(graph.validate(Some(&Registry::builtin())).is_empty());
        graph.remove(cluster).unwrap();
        assert!(graph.groups.is_empty());
    }
}
impl Fragment {
    pub fn capture(graph: &Graph, ids: &[NodeId]) -> Self {
        let ids: BTreeSet<_> = ids.iter().copied().collect();
        Self {
            format: "ringdesigner-nodes-v1".into(),
            nodes: graph
                .nodes
                .iter()
                .filter(|n| ids.contains(&n.id))
                .cloned()
                .collect(),
            wires: graph
                .wires
                .iter()
                .filter(|w| ids.contains(&w.from) && ids.contains(&w.to))
                .cloned()
                .collect(),
            groups: graph
                .groups
                .iter()
                .filter_map(|g| {
                    let nodes: Vec<_> = g
                        .nodes
                        .iter()
                        .copied()
                        .filter(|id| ids.contains(id))
                        .collect();
                    (!nodes.is_empty()).then(|| NodeGroup {
                        name: g.name.clone(),
                        nodes,
                    })
                })
                .collect(),
        }
    }

    /// Build on a clone so an invalid clipboard cannot partially change a design.
    pub fn paste(
        &self,
        graph: &mut Graph,
        at: [f32; 2],
        reg: &Registry,
    ) -> Result<Vec<NodeId>, GraphError> {
        let bad = GraphError::global;
        if self.format != "ringdesigner-nodes-v1"
            || self.nodes.is_empty()
            || self.nodes.len().saturating_add(graph.nodes.len()) > crate::MAX_NODES
            || at.iter().any(|v| !v.is_finite())
        {
            return Err(bad(
                "Clipboard is empty, invalid, or exceeds the node limit",
            ));
        }
        let ids: BTreeSet<_> = self.nodes.iter().map(|n| n.id).collect();
        if ids.len() != self.nodes.len()
            || self
                .wires
                .iter()
                .any(|w| !ids.contains(&w.from) || !ids.contains(&w.to))
        {
            return Err(bad("Clipboard has duplicate IDs or dangling wires"));
        }
        let mut result = graph.clone();
        let mut remap = BTreeMap::new();
        let origin = self.nodes.iter().fold([f32::INFINITY; 2], |a, n| {
            [a[0].min(n.pos[0]), a[1].min(n.pos[1])]
        });
        for n in &self.nodes {
            let spec = reg
                .get(&n.kind)
                .ok_or_else(|| GraphError::global(format!("Unknown node: {}", n.kind)))?;
            if !spec.modes.contains(&graph.mode) || n.pos.iter().any(|v| !v.is_finite()) {
                return Err(bad("Clipboard has an unavailable node or invalid position"));
            }
            let id = result.add(&n.kind)?;
            let mut copy = n.clone();
            copy.id = id;
            copy.pos = [at[0] + n.pos[0] - origin[0], at[1] + n.pos[1] - origin[1]];
            if copy.pos.iter().any(|v| !v.is_finite()) {
                return Err(bad("Clipboard positions are out of range"));
            }
            *result.node_mut(id).expect("added") = copy;
            remap.insert(n.id, id);
        }
        for w in &self.wires {
            result.connect(remap[&w.from], &w.out, remap[&w.to], &w.input)?;
        }
        for group in &self.groups {
            let nodes: Vec<_> = group
                .nodes
                .iter()
                .filter_map(|id| remap.get(id).copied())
                .collect();
            if !nodes.is_empty() {
                result.groups.push(NodeGroup {
                    name: group.name.clone(),
                    nodes,
                });
            }
        }
        // Only reject new structural failures; incomplete existing graphs remain editable.
        let new_ids: BTreeSet<_> = remap.values().copied().collect();
        if let Some(e) = result
            .validate(Some(reg))
            .into_iter()
            .find(|e| e.node.is_some_and(|id| new_ids.contains(&id)))
        {
            return Err(e);
        }
        *graph = result;
        Ok(self.nodes.iter().map(|n| remap[&n.id]).collect())
    }
}
