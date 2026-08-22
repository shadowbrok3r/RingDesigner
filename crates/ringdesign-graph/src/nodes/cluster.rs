//! A graph as a node.
//!
//! A cluster node carries the cluster graph in its params — a copy, so a
//! design that embeds its graph still opens on a machine without the
//! cluster file — and takes its pins from that graph's exposed inputs and
//! outputs. Evaluation runs the inner graph with the node's input values
//! injected straight onto the exposed pins (handles included), at one
//! cluster depth more than the graph it sits in.

use std::collections::BTreeMap;

use crate::MAX_CLUSTER_DEPTH;
use crate::eval::{Evaluator, Targets};
use crate::graph::{Graph, Node, NodeId};
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry};
use crate::value::{Value, ValueKind};

pub const CLUSTER_KIND: &str = "cluster";

/// The params a cluster node carries for `cluster`.
pub fn params_for(cluster: &Graph) -> serde_json::Value {
    serde_json::json!({ "cluster": cluster.name, "graph": cluster })
}

/// The embedded graph, if the params hold one.
pub fn embedded(node: &Node) -> Option<Graph> {
    serde_json::from_value(node.params.get("graph")?.clone()).ok()
}

/// Add a cluster node for `cluster` to `g`.
pub fn add_cluster(g: &mut Graph, cluster: &Graph) -> Result<NodeId, crate::graph::GraphError> {
    let id = g.add(CLUSTER_KIND)?;
    let node = g.node_mut(id).expect("just added");
    node.params = params_for(cluster);
    node.label = Some(cluster.name.clone());
    Ok(id)
}

/// Bring a node's embedded copy up to date with `cluster`.
pub fn resync(node: &mut Node, cluster: &Graph) {
    node.params = params_for(cluster);
}

fn pins(_: &NodeSpec, node: &Node, reg: &Registry) -> (Vec<PinSpec>, Vec<PinSpec>) {
    let Some(inner) = embedded(node) else { return (Vec::new(), Vec::new()) };
    let mut ins = Vec::new();
    for e in &inner.exposed {
        let Some(target) = inner.node(e.node) else { continue };
        let spec_pin = reg.node_pins(target).and_then(|(i, _)| i.into_iter().find(|p| p.name == e.input));
        let mut pin = match spec_pin {
            Some(p) => PinSpec { name: e.name.clone(), ..p },
            None => PinSpec::item(e.name.clone(), ValueKind::Any),
        };
        if let Some(lit) = target.inputs.get(&e.input) {
            pin.default = Some(lit.clone());
        }
        if !e.doc.is_empty() {
            pin.doc = e.doc.clone();
        }
        pin.optional = true;
        ins.push(pin);
    }
    let mut outs = Vec::new();
    for o in &inner.outputs {
        let Some(target) = inner.node(o.node) else { continue };
        let spec_pin = reg.node_pins(target).and_then(|(_, p)| p.into_iter().find(|p| p.name == o.out));
        let pin = match spec_pin {
            Some(p) => PinSpec { name: o.name.clone(), ..p },
            None => PinSpec::item(o.name.clone(), ValueKind::Any),
        };
        outs.push(if o.doc.is_empty() { pin } else { pin.doc(o.doc.clone()) });
    }
    (ins, outs)
}

fn run(ctx: &mut EvalCtx<'_>, node: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut inner = embedded(node).ok_or_else(|| NodeError::new("the cluster node carries no graph; re-sync it from its cluster"))?;
    if ctx.depth + 1 > MAX_CLUSTER_DEPTH {
        return Err(NodeError::new(format!("clusters nest deeper than {MAX_CLUSTER_DEPTH}")));
    }
    // The mode is the outer graph's, so its rules hold all the way down.
    inner.mode = ctx.mode;
    let mut injected: BTreeMap<(NodeId, String), Value> = BTreeMap::new();
    for e in &inner.exposed {
        let v = i.get(&e.name);
        if !v.is_null() {
            injected.insert((e.node, e.input.clone()), v.clone());
        }
    }
    let targets: Vec<NodeId> = inner.outputs.iter().map(|o| o.node).collect();
    if targets.is_empty() {
        return Err(NodeError::new(format!("cluster {:?} exposes no outputs", inner.name)));
    }
    let mut ev = Evaluator::new();
    ev.depth = ctx.depth + 1;
    ev.exprs = ctx.exprs.clone();
    let report = ev.evaluate_injected(&inner, ctx.reg, ctx.lib, ctx.lib_epoch, Targets::Nodes(targets), &injected);
    if let Some(e) = report.errors.first() {
        return Err(NodeError::new(format!("inside {:?}: {e}", inner.name)));
    }
    let notes = report.notes(&inner);
    // A cluster is one unit: a failure anywhere inside fails this item,
    // rather than letting a Null quietly become a default downstream.
    if report.any_failed() {
        let first = report
            .order
            .iter()
            .filter_map(|id| report.status.get(id).filter(|s| s.failed()).map(|s| (id, s)))
            .map(|(id, s)| format!("{id} {}: {}", inner.node(*id).map(|n| n.kind.as_str()).unwrap_or("?"), s.errors.first().map(|e| e.1.clone()).unwrap_or_default()))
            .next()
            .unwrap_or_default();
        return Err(NodeError::new(format!("inside {:?}: {first}", inner.name)));
    }
    for n in &notes {
        ctx.warn(format!("inside {:?}: {n}", inner.name));
    }
    let mut out = Outputs::default();
    for o in &inner.outputs {
        let v = report.value(o.node, &o.out).cloned().unwrap_or(Value::Null);
        out.values.insert(o.name.clone(), v);
    }
    Ok(out)
}

pub fn register(reg: &mut Registry) {
    reg.register(
        NodeSpec::new(CLUSTER_KIND, "Cluster", Category::Assembly)
            .doc("A saved graph used as one node: its exposed inputs are the pins, its exposed outputs the results. The graph rides in the node, so the design stays whole.")
            .resolve(pins)
            .eval(run),
    )
    .expect("unique");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::Preset;
    use crate::graph::Mode;
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    /// A cluster: width in, a flat-sided design out.
    fn band_cluster() -> Graph {
        let mut c = Graph::new("Squared band", Mode::SandRing);
        let p = c.add("band.profile").unwrap();
        c.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        c.set_input(p, "width_mm", Literal::Number(6.0)).unwrap();
        c.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let d = c.add("design.new").unwrap();
        c.connect(p, "profile", d, "profile").unwrap();
        c.expose(p, "width_mm", "Width").unwrap();
        c.expose(d, "name", "Name").unwrap();
        c.expose(p, "profile", "Base profile").unwrap();
        c.expose_output(d, "design", "design").unwrap();
        c.expose_output(p, "profile", "profile").unwrap();
        c
    }

    #[test]
    fn a_cluster_node_takes_its_pins_from_the_graph_and_runs_it() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::default();
        let cluster = band_cluster();
        let mut g = Graph::default();
        let n = add_cluster(&mut g, &cluster).unwrap();
        let (ins, outs) = reg.node_pins(g.node(n).unwrap()).unwrap();
        assert_eq!(ins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Width", "Name", "Base profile"]);
        assert_eq!(ins[0].kind, ValueKind::Number);
        assert_eq!(ins[0].default, Some(Literal::Number(6.0)), "the inner literal is the pin's default");
        assert_eq!(ins[2].kind, ValueKind::Profile);
        assert_eq!(outs.iter().map(|p| (p.name.as_str(), p.kind)).collect::<Vec<_>>(), vec![("design", ValueKind::Design), ("profile", ValueKind::Profile)]);
        assert!(g.validate(Some(&reg)).is_empty());

        // Unset: the cluster's own literals.
        let info = g.add("design.info").unwrap();
        g.connect(n, "design", info, "design").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(r.value(info, "width_mm"), Some(&Value::Number(6.0)));

        // Set, and as a list: implicit lists pass through a cluster.
        g.set_input(n, "Width", Literal::List(vec![Literal::Number(5.0), Literal::Number(9.0)])).unwrap();
        g.set_input(n, "Name", Literal::Text("Wide".into())).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        match r.value(info, "width_mm") {
            Some(Value::List(v)) => assert_eq!(v, &vec![Value::Number(5.0), Value::Number(9.0)]),
            other => panic!("{other:?}"),
        }
        assert_eq!(r.value(info, "name"), Some(&Value::List(vec![Value::Text("Wide".into()), Value::Text("Wide".into())])));

        // A handle goes in through an exposed pin.
        let base = g.add("band.profile").unwrap();
        g.set_input(base, "thickness_mm", Literal::Number(3.3)).unwrap();
        g.connect(base, "profile", n, "Base profile").unwrap();
        g.set_input(n, "Width", Literal::Number(7.0)).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(r.value(info, "thickness_mm"), Some(&Value::Number(3.3)));
        assert_eq!(r.value(info, "width_mm"), Some(&Value::Number(7.0)));

        // A preset sets the exposed inputs and reports what it could not.
        let preset = Preset { name: "Eight".into(), cluster: cluster.name.clone(), values: [("Width".to_string(), Literal::Number(8.0)), ("Gone".to_string(), Literal::Null)].into_iter().collect(), doc: String::new() };
        let unknown = preset.apply(g.node_mut(n).unwrap(), &reg);
        assert_eq!(unknown, vec!["Gone"]);
        assert_eq!(g.node(n).unwrap().inputs.get("Width"), Some(&Literal::Number(8.0)));

        // A failure inside is reported from inside.
        g.set_input(n, "Width", Literal::Number(0.0)).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(r.status[&n].failed());
        assert!(r.status[&n].errors[0].1.contains("inside \"Squared band\""), "{:?}", r.status[&n].errors);
    }

    #[test]
    fn clusters_nest_to_the_cap_and_carry_the_outer_mode() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::default();
        // Wrap the band cluster in itself, MAX_CLUSTER_DEPTH + 1 times.
        let mut inner = band_cluster();
        for level in 0..=MAX_CLUSTER_DEPTH {
            let mut outer = Graph::new(format!("Level {level}"), Mode::SandRing);
            let n = add_cluster(&mut outer, &inner).unwrap();
            outer.expose(n, "Width", "Width").unwrap();
            outer.expose_output(n, "design", "design").unwrap();
            inner = outer;
        }
        let mut g = Graph::default();
        let n = add_cluster(&mut g, &inner).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(r.status[&n].failed());
        assert!(r.status[&n].errors[0].1.contains("deeper than"), "{:?}", r.status[&n].errors);

        // A Free-only node inside a cluster is refused in a SandRing graph.
        let mut c = Graph::new("Free inside", Mode::Free);
        let d = c.add("design.new").unwrap();
        let b = c.add("sink.build").unwrap();
        c.connect(d, "design", b, "design").unwrap();
        c.set_input(b, "preset", Literal::Text("Draft".into())).unwrap();
        let mv = c.add("sink.mesh_verdict").unwrap();
        c.connect(b, "mesh", mv, "mesh").unwrap();
        c.connect(d, "design", mv, "design").unwrap();
        c.expose_output(mv, "verdict", "verdict").unwrap();
        let mut g = Graph::new("outer", Mode::SandRing);
        let n = add_cluster(&mut g, &c).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(r.status[&n].errors[0].1.contains("does not run in SandRing"), "{:?}", r.status[&n].errors);
        g.mode = Mode::Free;
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert!(matches!(r.value(n, "verdict"), Some(Value::Text(_))));
    }
}
