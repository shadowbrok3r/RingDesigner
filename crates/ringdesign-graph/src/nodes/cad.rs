//! CAD features return designs so desktop, CLI, and manufacturing all build
//! the same source program. Node identities double as stable feature names.
use crate::{
    graph::{Graph, GraphError, Mode, Node, NodeId},
    registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry},
    value::{Literal, Value, ValueKind},
};
use ringdesign_core::{
    RingDesign,
    cad::{
        Document, Feature, Operation,
        edit::{Applied, CadEdit},
    },
    sketch::Id,
};

fn source(_: &mut EvalCtx<'_>, n: &Node, _: &Inputs) -> Result<Outputs, NodeError> {
    let mut d: RingDesign = serde_json::from_value(n.params.clone())
        .map_err(|e| NodeError::new(format!("CAD source design: {e}")))?;
    d.graph = None;
    d.cad = None;
    Ok(Outputs::one("design", d))
}
fn feature(_: &mut EvalCtx<'_>, n: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut d = match i.get("design") {
        Value::Design(d) => (**d).clone(),
        _ => return Err(NodeError::input("design", "Connect a source design")),
    };
    let mut f: Feature = serde_json::from_value(n.params.clone())
        .map_err(|e| NodeError::new(format!("CAD feature: {e}")))?;
    f.id = n.id.0;
    if !matches!(i.get("operation"), Value::Null) {
        f.operation = serde_json::from_value(
            i.get("operation")
                .to_json_any()
                .ok_or_else(|| NodeError::input("operation", "Expected operation JSON"))?,
        )
        .map_err(|e| NodeError::input("operation", e.to_string()))?;
    }
    f.enabled &= i.bool("enabled")?;
    d.cad
        .get_or_insert_with(Document::default)
        .append(f)
        .map_err(|e| NodeError::new(e.to_string()))?;
    Ok(Outputs::one("design", d))
}
fn resize(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = match i.get("design") {
        Value::Design(d) => d,
        _ => return Err(NodeError::input("design", "Connect a design")),
    };
    let policy = ringdesign_core::resize::Policy {
        preserve_head: i.bool("preserve_head")?,
        preserve_ornament_pitch: i.bool("preserve_pitch")?,
        ..Default::default()
    };
    let candidate = ringdesign_core::resize::candidate(d, i.number("bore_mm")?, &policy)
        .map_err(|e| NodeError::new(e.to_string()))?;
    Ok(Outputs::one("design", candidate.design))
}
pub fn register(reg: &mut Registry) {
    reg.register(NodeSpec::new("cad.source","CAD source",Category::Assembly).doc("The nominal ring parameters behind a Free-mode feature history; source design in node settings.").output(PinSpec::item("design",ValueKind::Design).doc("Source parameters and an empty feature program.")).eval(source)).expect("unique");
    reg.register(NodeSpec::new("cad.feature","CAD feature",Category::Assembly).doc("Append an analytic CAD feature. Source geometry and component settings remain editable in the feature tree.").input(PinSpec::item("design",ValueKind::Design).doc("Previous feature or nominal design.")).input(PinSpec::item("operation",ValueKind::Json).optional().doc("Optional operation JSON from upstream parameters; absent uses the feature editor's source recipe.")).input(PinSpec::item("enabled",ValueKind::Bool).default(true).doc("Suppress this feature without deleting its settings.")).output(PinSpec::item("design",ValueKind::Design).doc("Design with appended feature.")).eval(feature)).expect("unique");
    reg.register(NodeSpec::new("manufacturing.inspect","Inspect pattern",Category::Util).doc("Shared arbitrary-pull release, stock, flask and wall inspection on the prepared pattern.").input(PinSpec::item("design",ValueKind::Design).doc("Evaluated source design and manufacturing setup.")).output(PinSpec::item("report",ValueKind::Json).doc("Identified sampled manufacturing report.")).eval(inspect_pattern)).expect("unique");
    reg.register(
        NodeSpec::new("cad.inspect", "Inspect assembly", Category::Util)
            .doc("Measure evaluated components and check assembly interference.")
            .input(PinSpec::item("design", ValueKind::Design).doc("Evaluated CAD source design."))
            .output(
                PinSpec::item("report", ValueKind::Json)
                    .doc("Component identities, actual dimensions, volume, and assembly findings."),
            )
            .eval(inspect_cad),
    )
    .expect("unique");
    reg.register(NodeSpec::new("design.resize","Resize ring",Category::Assembly).doc("Rebuild the shank to an exact bore, preserving measured stones and optionally head dimensions and ornament pitch.").input(PinSpec::item("design",ValueKind::Design).doc("Source ring design.")).input(PinSpec::item("bore_mm",ValueKind::Number).default(18.0).doc("Exact nominal bore diameter in millimeters.")).input(PinSpec::item("preserve_head",ValueKind::Bool).default(true).doc("Keep the existing head dimensions.")).input(PinSpec::item("preserve_pitch",ValueKind::Bool).default(true).doc("Adjust closed repeat counts to retain ornament pitch.")).output(PinSpec::item("design",ValueKind::Design).doc("Resized source geometry.")).eval(resize)).expect("unique");
}
fn inspect_pattern(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = match i.get("design") {
        Value::Design(d) => d,
        _ => return Err(NodeError::input("design", "Connect a design")),
    };
    let setup = d
        .manufacturing
        .clone()
        .unwrap_or_else(|| ringdesign_core::manufacturing::Setup::from_design(d));
    let report = ringdesign_core::manufacturing::inspect(d, ctx.lib, &setup, d.build)
        .map_err(|e| NodeError::new(e.to_string()))?;
    Ok(Outputs::one(
        "report",
        ringdesign_core::manufacturing::package::report(d, &setup, &report, true),
    ))
}
fn inspect_cad(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = match i.get("design") {
        Value::Design(d) => d,
        _ => return Err(NodeError::input("design", "Connect a CAD design")),
    };
    let e = ringdesign_core::cad::evaluate(d, ctx.lib, d.build)
        .map_err(|e| NodeError::new(e.to_string()))?;
    Ok(Outputs::one(
        "report",
        ringdesign_core::cad::assembly::manifest(d, &e),
    ))
}
pub fn append_resize(
    g: &mut Graph,
    bore_mm: f64,
    policy: &ringdesign_core::resize::Policy,
) -> Result<NodeId, crate::graph::GraphError> {
    let output = g
        .nodes
        .iter()
        .find(|n| n.kind == crate::eval::OUTPUT_KIND)
        .map(|n| n.id);
    let previous = output
        .and_then(|id| {
            g.wire_into(id, crate::eval::OUTPUT_DESIGN_PIN)
                .map(|w| w.from)
        })
        .or_else(|| g.nodes.last().map(|n| n.id))
        .ok_or_else(|| crate::graph::GraphError::global("Missing design source"))?;
    let id = g.add("design.resize")?;
    g.set_input(id, "bore_mm", crate::value::Literal::Number(bore_mm))?;
    g.set_input(
        id,
        "preserve_head",
        crate::value::Literal::Bool(policy.preserve_head),
    )?;
    g.set_input(
        id,
        "preserve_pitch",
        crate::value::Literal::Bool(policy.preserve_ornament_pitch),
    )?;
    g.connect(previous, "design", id, "design")?;
    let sink = if let Some(sink) = output {
        sink
    } else {
        g.add(crate::eval::OUTPUT_KIND)?
    };
    g.connect(id, "design", sink, crate::eval::OUTPUT_DESIGN_PIN)?;
    Ok(id)
}
/// Normal property controls become explicit source edits in the graph.
pub fn append_property(
    g: &mut Graph,
    path: &str,
    value: serde_json::Value,
) -> Result<NodeId, crate::graph::GraphError> {
    let output = g
        .nodes
        .iter()
        .find(|n| n.kind == crate::eval::OUTPUT_KIND)
        .map(|n| n.id);
    let previous = output
        .and_then(|id| {
            g.wire_into(id, crate::eval::OUTPUT_DESIGN_PIN)
                .map(|w| w.from)
        })
        .or_else(|| g.nodes.last().map(|n| n.id))
        .ok_or_else(|| crate::graph::GraphError::global("Missing design source"))?;
    let id = g.add("design.set")?;
    g.set_input(id, "pointer", crate::value::Literal::Text(path.into()))?;
    g.node_mut(id).unwrap().params = serde_json::json!({"json_value":value});
    g.connect(previous, "design", id, "design")?;
    let sink = if let Some(sink) = output {
        sink
    } else {
        g.add(crate::eval::OUTPUT_KIND)?
    };
    g.connect(id, "design", sink, crate::eval::OUTPUT_DESIGN_PIN)?;
    Ok(id)
}
/// Inspect the design at a history node before downstream resizes/property edits.
/// The source graph is unchanged and downstream failures do not hide this state.
pub fn history_design(
    evaluator: &mut crate::eval::Evaluator,
    g: &Graph,
    node: NodeId,
    registry: &Registry,
    lib: &ringdesign_core::AlphaLibrary,
) -> Result<std::sync::Arc<RingDesign>, crate::graph::GraphError> {
    let report = evaluator.evaluate(g, registry, lib, 0, crate::eval::Targets::Node(node));
    if let Some(error) = report.errors.first() {
        return Err(error.clone());
    }
    match report.value(node, "design") {
        Some(crate::value::Value::Design(d)) => Ok(d.clone()),
        Some(crate::value::Value::List(values)) if values.len() == 1 => match &values[0] {
            crate::value::Value::Design(d) => Ok(d.clone()),
            _ => Err(crate::graph::GraphError::at(
                node,
                "History node did not produce a design",
            )),
        },
        _ => Err(crate::graph::GraphError::at(
            node,
            "Choose a history node producing one design",
        )),
    }
}

pub fn start(d: &RingDesign) -> Result<Graph, crate::graph::GraphError> {
    let mut g = Graph::new(&d.name, Mode::Free);
    let id = g.add("cad.source")?;
    let mut d = d.clone();
    d.graph = None;
    d.cad = None;
    g.node_mut(id).unwrap().params =
        serde_json::to_value(d).map_err(|e| crate::graph::GraphError::global(e.to_string()))?;
    Ok(g)
}
/// Lift a saved CAD program into one graph node per source feature, preserving
/// feature identities and component references. Source id zero is reserved.
pub fn from_document(d: &RingDesign) -> Result<Graph, crate::graph::GraphError> {
    let Some(doc) = &d.cad else {
        return start(d);
    };
    let ids: std::collections::BTreeSet<_> = doc.features.iter().map(|f| f.id).collect();
    if ids.len() != doc.features.len() {
        return Err(crate::graph::GraphError::global(
            "Duplicate CAD feature identities",
        ));
    }
    let mut g = start(d)?;
    g.nodes[0].id = NodeId(0);
    let mut previous = NodeId(0);
    for f in &doc.features {
        if f.id == 0 {
            return Err(crate::graph::GraphError::global(
                "CAD feature zero is reserved for the source",
            ));
        }
        let temporary = g.add("cad.feature")?;
        let position = g.nodes.len() as f32 * 220.0;
        let node = g.node_mut(temporary).unwrap();
        node.id = NodeId(f.id);
        node.params = serde_json::to_value(f).unwrap();
        node.pos = [position, 100.0];
        g.connect(previous, "design", NodeId(f.id), "design")?;
        previous = NodeId(f.id);
    }
    for (path, value) in [
        ("/cad/outputs", serde_json::to_value(&doc.outputs).unwrap()),
        ("/cad/joints", serde_json::to_value(&doc.joints).unwrap()),
        ("/cad/through", serde_json::to_value(doc.through).unwrap()),
    ] {
        append_property(&mut g, path, value)?;
    }
    Ok(g)
}
/// Chains after `from` one `cad.feature` node per feature, each carrying its feature's id, then the
/// document's outputs, joints and rollback as property nodes; returns the chain's last node.
pub fn chain_document(g: &mut Graph, from: NodeId, doc: &Document) -> Result<NodeId, GraphError> {
    let mut previous = from;
    for f in &doc.features {
        if g.node(NodeId(f.id)).is_some() {
            return Err(GraphError::global(format!("CAD feature #{} would take the id of a node already in the graph", f.id)));
        }
        let temporary = g.add("cad.feature")?;
        let node = g.node_mut(temporary).expect("added");
        node.id = NodeId(f.id);
        node.params = serde_json::to_value(f).map_err(|e| GraphError::global(e.to_string()))?;
        g.connect(previous, "design", NodeId(f.id), "design")?;
        previous = NodeId(f.id);
    }
    for (path, value) in [
        (OUTPUTS, serde_json::to_value(&doc.outputs)),
        (JOINTS, serde_json::to_value(&doc.joints)),
        (THROUGH, serde_json::to_value(doc.through)),
    ] {
        let id = g.add("design.set")?;
        g.set_input(id, "pointer", Literal::Text(path.into()))?;
        g.node_mut(id).expect("added").params = serde_json::json!({ "json_value": value.map_err(|e| GraphError::global(e.to_string()))? });
        g.connect(previous, "design", id, "design")?;
        previous = id;
    }
    Ok(previous)
}
pub fn append(g: &mut Graph, operation: Operation) -> Result<NodeId, crate::graph::GraphError> {
    let output = g
        .nodes
        .iter()
        .find(|n| n.kind == crate::eval::OUTPUT_KIND)
        .map(|n| n.id);
    let previous = output
        .and_then(|id| {
            g.wire_into(id, crate::eval::OUTPUT_DESIGN_PIN)
                .map(|w| w.from)
        })
        .or_else(|| g.nodes.last().map(|n| n.id))
        .ok_or_else(|| crate::graph::GraphError::global("Graph needs a design source"))?;
    // A part built round a stone takes its builder's own component, never the stone's.
    let mut component = match &operation {
        Operation::Builder { key, .. } => ringdesign_core::cad::builders::component(key),
        _ => operation
            .sources()
            .first()
            .and_then(|source| g.node(NodeId(*source)))
            .and_then(|n| serde_json::from_value::<Feature>(n.params.clone()).ok())
            .map(|f| f.component)
            .unwrap_or_default(),
    };
    component.placement = ringdesign_core::cad::Placement::Free;
    if matches!(
        operation,
        Operation::Band
            | Operation::Torus { .. }
            | Operation::TwistedRing { .. }
            | Operation::Revolve { .. }
    ) {
        component.role = ringdesign_core::cad::ComponentRole::Shank;
    }
    g.mode = Mode::Free;
    let id = g.add("cad.feature")?;
    let f = Feature {
        id: id.0,
        name: operation.label().into(),
        enabled: true,
        operation,
        component,
    };
    g.node_mut(id).unwrap().params = serde_json::to_value(f).unwrap();
    g.node_mut(id).unwrap().pos = [g.nodes.len() as f32 * 220.0, 100.0];
    g.connect(previous, "design", id, "design")?;
    let sink = if let Some(sink) = output {
        sink
    } else {
        g.add(crate::eval::OUTPUT_KIND)?
    };
    g.connect(id, "design", sink, crate::eval::OUTPUT_DESIGN_PIN)?;
    Ok(id)
}
/// The document fields the lift writes through `design.set` nodes.
const OUTPUTS: &str = "/cad/outputs";
const JOINTS: &str = "/cad/joints";
const THROUGH: &str = "/cad/through";
/// The document field a node writes, if it is one of the lift's property nodes.
fn property_of(n: &Node) -> Option<&'static str> {
    if n.kind != "design.set" {
        return None;
    }
    match n.inputs.get("pointer") {
        Some(Literal::Text(p)) => [OUTPUTS, JOINTS, THROUGH].into_iter().find(|k| k == p),
        _ => None,
    }
}
fn is_feature(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.kind == "cad.feature")
}
fn is_property(g: &Graph, id: NodeId) -> bool {
    g.node(id).and_then(property_of).is_some()
}
/// A refusal naming a node whose effect on the document is only known by evaluating it.
fn unreadable(id: NodeId, what: impl std::fmt::Display) -> GraphError {
    GraphError::at(id, format!("{what}; edit it in the graph"))
}
/// The JSON a literal hands an item pin; `None` for an expression, or a list the node would run per item.
fn literal_json(l: &Literal) -> Option<serde_json::Value> {
    Some(match l {
        Literal::Null => serde_json::Value::Null,
        Literal::Bool(b) => serde_json::json!(b),
        Literal::Int(i) => serde_json::json!(i),
        Literal::Number(x) => serde_json::json!(x),
        Literal::Text(s) => serde_json::json!(s),
        Literal::Json(v) => v.clone(),
        Literal::List(_) | Literal::Expr(_) => return None,
    })
}
/// The nodes a design passes through by `design` wires to the sink (else the last node), source first.
fn design_chain(g: &Graph) -> Vec<NodeId> {
    let end = g
        .nodes
        .iter()
        .find(|n| n.kind == crate::eval::OUTPUT_KIND)
        .map(|n| n.id)
        .or_else(|| g.nodes.last().map(|n| n.id));
    let mut chain = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut at = end;
    while let Some(id) = at {
        if !seen.insert(id) {
            break;
        }
        chain.push(id);
        at = g.wire_into(id, "design").map(|w| w.from);
    }
    chain.reverse();
    chain
}
/// The feature a chain's `cad.feature` node appends, read as the node evaluates it.
fn chain_feature(g: &Graph, n: &Node) -> Result<Feature, GraphError> {
    if g.wire_into(n.id, "operation").is_some() {
        return Err(unreadable(n.id, "this feature takes its operation from a wire"));
    }
    if n.inputs.get("operation").is_some_and(|l| *l != Literal::Null) {
        return Err(unreadable(n.id, "this feature takes its operation from its pin"));
    }
    if g.wire_into(n.id, "enabled").is_some() {
        return Err(unreadable(n.id, "this feature is suppressed by a wire"));
    }
    let pin = match n.inputs.get("enabled") {
        None => true,
        Some(Literal::Bool(b)) => *b,
        Some(Literal::Int(i)) => *i != 0,
        Some(Literal::Number(x)) => *x != 0.0,
        Some(_) => return Err(unreadable(n.id, "this feature's enabled pin holds no boolean")),
    };
    let mut f: Feature = serde_json::from_value(n.params.clone()).map_err(|e| GraphError::at(n.id, format!("CAD feature: {e}")))?;
    f.id = n.id.0;
    f.enabled &= pin;
    Ok(f)
}
/// The document field a chain's `design.set` node writes and its value; a write elsewhere under `/cad` is refused.
fn chain_property(g: &Graph, n: &Node) -> Result<Option<(&'static str, serde_json::Value)>, GraphError> {
    if g.wire_into(n.id, "pointer").is_some() {
        return Err(unreadable(n.id, "this node writes a field a wire names"));
    }
    let Some(pointer) = property_of(n) else {
        return match n.inputs.get("pointer") {
            Some(Literal::Text(p)) if p == "/cad" || p.starts_with("/cad/") => Err(unreadable(n.id, format!("this node writes {p} whole"))),
            _ => Ok(None),
        };
    };
    if let Some(v) = n.params.get("json_value") {
        return Ok(Some((pointer, v.clone())));
    }
    if g.wire_into(n.id, "value").is_some() {
        return Err(unreadable(n.id, format!("{pointer} comes from a wire")));
    }
    let value = n.inputs.get("value").and_then(literal_json).ok_or_else(|| GraphError::at(n.id, format!("{pointer} carries no value")))?;
    Ok(Some((pointer, value)))
}
/// The document a graph's design chain encodes, and the chain's nodes.
struct Chain {
    nodes: Vec<NodeId>,
    doc: Document,
}
fn read_chain(g: &Graph) -> Result<Chain, GraphError> {
    let nodes = design_chain(g);
    let mut doc = Document::default();
    for &id in &nodes {
        let n = g.node(id).ok_or_else(|| GraphError::at(id, "no such node"))?;
        if n.kind == "cad.feature" {
            doc.append(chain_feature(g, n)?).map_err(|e| GraphError::at(id, e.to_string()))?;
        } else if n.kind == "design.set" {
            let Some((pointer, value)) = chain_property(g, n)? else { continue };
            let parse = |e: serde_json::Error| GraphError::at(id, format!("{pointer}: {e}"));
            match pointer {
                OUTPUTS => doc.outputs = serde_json::from_value(value).map_err(parse)?,
                JOINTS => doc.joints = serde_json::from_value(value).map_err(parse)?,
                _ => doc.through = serde_json::from_value(value).map_err(parse)?,
            }
        }
    }
    Ok(Chain { nodes, doc })
}
/// The document a graph evaluates to, read off its nodes; empty where the evaluated design has none.
pub fn document(g: &Graph) -> Result<Document, GraphError> {
    read_chain(g).map(|c| c.doc)
}
/// The output a chain node hands its design on: the one its wire into `to` uses, else `design`.
fn design_out(g: &Graph, from: NodeId, to: Option<NodeId>) -> String {
    to.and_then(|to| g.wire_into(to, "design"))
        .filter(|w| w.from == from)
        .map_or_else(|| "design".into(), |w| w.out.clone())
}
/// Wires `node` into the chain directly after `pred`.
fn insert_after(g: &mut Graph, nodes: &mut Vec<NodeId>, pred: NodeId, node: NodeId) -> Result<(), GraphError> {
    let i = nodes.iter().position(|n| *n == pred).ok_or_else(|| GraphError::at(pred, "not on the design chain"))?;
    let out = design_out(g, pred, nodes.get(i + 1).copied());
    if let Some(&next) = nodes.get(i + 1) {
        g.connect(node, "design", next, "design")?;
    }
    g.connect(pred, out, node, "design")?;
    let pos = g.node(pred).map(|n| n.pos).unwrap_or_default();
    if let Some(n) = g.node_mut(node) {
        n.pos = [pos[0] + 220.0, pos[1]];
    }
    nodes.insert(i + 1, node);
    Ok(())
}
/// Takes `node` out of the chain and wires its neighbours together.
fn splice_out(g: &mut Graph, nodes: &mut Vec<NodeId>, node: NodeId) -> Result<(), GraphError> {
    let i = nodes.iter().position(|n| *n == node).ok_or_else(|| GraphError::at(node, "not on the design chain"))?;
    let up = g.disconnect(node, "design");
    if let Some(&down) = nodes.get(i + 1) {
        g.disconnect(down, "design");
        if let Some(up) = up {
            g.connect(up.from, up.out, down, "design")?;
        }
    }
    nodes.remove(i);
    Ok(())
}
/// Where an added feature goes: after the last feature, else before the first property node or the sink.
fn feature_end(g: &Graph, nodes: &[NodeId]) -> Option<NodeId> {
    if let Some(last) = nodes.iter().rev().find(|id| is_feature(g, **id)) {
        return Some(*last);
    }
    let stop = nodes
        .iter()
        .position(|id| is_property(g, *id) || g.node(*id).is_some_and(|n| n.kind == crate::eval::OUTPUT_KIND))
        .unwrap_or(nodes.len());
    stop.checked_sub(1).map(|k| nodes[k])
}
/// The edited document an edit leaves, judged before the graph is touched.
struct Plan {
    nodes: Vec<NodeId>,
    doc: Document,
    applied: Applied,
}
fn plan(g: &Graph, edit: &CadEdit) -> Result<Plan, GraphError> {
    let Chain { nodes, mut doc } = read_chain(g)?;
    if nodes.is_empty() {
        return Err(GraphError::global("Graph needs a design source"));
    }
    if let CadEdit::Add { feature, .. } = edit {
        if feature.id != 0 && doc.feature(feature.id).is_none() && g.contains(NodeId(feature.id)) {
            return Err(GraphError::at(NodeId(feature.id), format!("Feature identity #{} names another node", feature.id)));
        }
    }
    let applied = doc.apply(edit).map_err(|e| GraphError::global(e.to_string()))?;
    Ok(Plan { nodes, doc, applied })
}
/// Rewires the chain and rewrites its params to encode a planned edit.
fn encode(g: &mut Graph, plan: Plan, edit: &CadEdit) -> Result<Applied, GraphError> {
    let Plan { mut nodes, mut doc, mut applied } = plan;
    match edit {
        CadEdit::Add { feature, after } => {
            let node = g.add("cad.feature")?;
            let node = if feature.id == 0 {
                node
            } else {
                g.node_mut(node).expect("added").id = NodeId(feature.id);
                g.next_id = g.next_id.max(feature.id + 1);
                NodeId(feature.id)
            };
            // Renames the document's placeholder id to the node's id.
            let placed = applied.id.expect("an add names its feature");
            for f in doc.features.iter_mut().filter(|f| f.id == placed) {
                f.id = node.0;
            }
            for o in doc.outputs.iter_mut().filter(|o| **o == placed) {
                *o = node.0;
            }
            applied.id = Some(node.0);
            let pred = match after {
                Some(a) => NodeId(*a),
                None => feature_end(g, &nodes).ok_or_else(|| GraphError::global("Graph needs a design source"))?,
            };
            insert_after(g, &mut nodes, pred, node)?;
            if !g.nodes.iter().any(|n| n.kind == crate::eval::OUTPUT_KIND) {
                // Feeds a new sink from the chain's last feature or property node.
                let end = *nodes.iter().rev().find(|id| is_feature(g, **id) || is_property(g, **id)).expect("the chain holds the new node");
                let sink = g.add(crate::eval::OUTPUT_KIND)?;
                g.connect(end, "design", sink, crate::eval::OUTPUT_DESIGN_PIN)?;
                let at = nodes.iter().position(|n| *n == end).expect("on the chain") + 1;
                nodes.truncate(at);
                nodes.push(sink);
            }
        }
        CadEdit::Remove { id } => {
            splice_out(g, &mut nodes, NodeId(*id))?;
            g.remove(NodeId(*id))?;
        }
        CadEdit::Move { id, after } => {
            let node = NodeId(*id);
            let at = nodes.iter().position(|n| *n == node).ok_or_else(|| GraphError::at(node, "not on the design chain"))?;
            let source = at.checked_sub(1).map(|k| nodes[k]).ok_or_else(|| GraphError::at(node, "has no design source"))?;
            splice_out(g, &mut nodes, node)?;
            let pred = match after {
                Some(a) => NodeId(*a),
                None => nodes
                    .iter()
                    .position(|n| is_feature(g, *n))
                    .and_then(|i| i.checked_sub(1))
                    .map_or(source, |k| nodes[k]),
            };
            insert_after(g, &mut nodes, pred, node)?;
        }
        _ => {}
    }
    settle_properties(g, &mut nodes, &doc)?;
    write_features(g, &doc, edit)?;
    Ok(applied)
}
/// Keeps one property node per field after the last feature, patched or added as the document needs; none once empty.
fn settle_properties(g: &mut Graph, nodes: &mut Vec<NodeId>, doc: &Document) -> Result<(), GraphError> {
    let found: Vec<(NodeId, &'static str)> = nodes.iter().filter_map(|id| g.node(*id).and_then(property_of).map(|p| (*id, p))).collect();
    let mut kept = Vec::new();
    for (i, (id, pointer)) in found.iter().enumerate() {
        // Drops every property node once no feature is left, and any a later node of its field overwrites.
        if doc.features.is_empty() || found[i + 1..].iter().any(|(_, p)| p == pointer) {
            splice_out(g, nodes, *id)?;
            g.remove(*id)?;
        } else {
            kept.push((*id, *pointer));
        }
    }
    let Some(last) = nodes.iter().rposition(|id| is_feature(g, *id)).map(|i| nodes[i]) else {
        return Ok(());
    };
    // Moves property nodes that precede a feature to after the last feature, in order.
    let mut anchor = last;
    for (id, _) in &kept {
        let at = nodes.iter().position(|n| n == id).expect("kept on the chain");
        let end = nodes.iter().position(|n| *n == last).expect("the last feature is on the chain");
        if at < end {
            splice_out(g, nodes, *id)?;
            insert_after(g, nodes, anchor, *id)?;
            anchor = *id;
        }
    }
    let mut replay = Document::default();
    for f in &doc.features {
        replay.append(f.clone()).map_err(|e| GraphError::at(NodeId(f.id), e.to_string()))?;
    }
    let json = |v: Result<serde_json::Value, serde_json::Error>| v.map_err(|e| GraphError::global(e.to_string()));
    for (pointer, value, needed) in [
        (OUTPUTS, json(serde_json::to_value(&doc.outputs))?, doc.outputs != replay.outputs),
        (JOINTS, json(serde_json::to_value(&doc.joints))?, !doc.joints.is_empty()),
        (THROUGH, json(serde_json::to_value(doc.through))?, doc.through.is_some()),
    ] {
        if let Some((id, _)) = kept.iter().find(|(_, p)| *p == pointer) {
            let n = g.node_mut(*id).expect("kept on the chain");
            if n.params.get("json_value") != Some(&value) {
                if !n.params.is_object() {
                    n.params = serde_json::json!({});
                }
                n.params.as_object_mut().expect("an object").insert("json_value".into(), value);
            }
        } else if needed {
            let pred = nodes
                .iter()
                .rev()
                .find(|id| is_feature(g, **id) || is_property(g, **id))
                .copied()
                .expect("the document has a feature");
            let id = g.add("design.set")?;
            g.set_input(id, "pointer", Literal::Text(pointer.into()))?;
            g.node_mut(id).expect("added").params = serde_json::json!({ "json_value": value });
            insert_after(g, nodes, pred, id)?;
        }
    }
    Ok(())
}
/// Rewrites each feature node's params; the `enabled` pin follows when the edit switches it or it holds a literal.
fn write_features(g: &mut Graph, doc: &Document, edit: &CadEdit) -> Result<(), GraphError> {
    for f in &doc.features {
        let id = NodeId(f.id);
        let value = serde_json::to_value(f).map_err(|e| GraphError::at(id, e.to_string()))?;
        let n = g
            .node_mut(id)
            .filter(|n| n.kind == "cad.feature")
            .ok_or_else(|| GraphError::at(id, "no CAD feature node carries this feature"))?;
        if n.params != value {
            n.params = value;
        }
        let switched = matches!(edit, CadEdit::Enable { id: target, .. } if *target == f.id);
        if switched || n.inputs.contains_key("enabled") {
            n.inputs.insert("enabled".into(), Literal::Bool(f.enabled));
        }
    }
    Ok(())
}
/// Runs `f` on the graph, or leaves the graph as it was when `f` fails.
fn transact<T>(g: &mut Graph, f: impl FnOnce(&mut Graph) -> Result<T, GraphError>) -> Result<T, GraphError> {
    let before = g.clone();
    let result = f(g);
    if result.is_err() {
        *g = before;
    }
    result
}
/// Applies a [`CadEdit`] to a graph's CAD chain so it evaluates to what [`Document::apply`] gives, or leaves it untouched.
pub fn apply_edit(g: &mut Graph, edit: &CadEdit) -> Result<Applied, GraphError> {
    let plan = plan(g, edit)?;
    transact(g, |g| encode(g, plan, edit))
}
/// Removes a feature and everything that reads it through the graph, dependents first; the ids removed.
pub fn remove_with_dependents(g: &mut Graph, id: Id) -> Result<Vec<Id>, GraphError> {
    let removed = read_chain(g)?.doc.remove_with_dependents(id).map_err(|e| GraphError::global(e.to_string()))?;
    transact(g, |g| {
        for r in &removed {
            let edit = CadEdit::Remove { id: *r };
            let plan = plan(g, &edit)?;
            encode(g, plan, &edit)?;
        }
        Ok(removed)
    })
}
/// The one edit funnel: edits a graph-driven design's graph for the worker to evaluate, else a plain design's document.
pub fn edit_design(d: &mut RingDesign, edit: &CadEdit) -> anyhow::Result<Applied> {
    use serde::Deserialize;
    let Some(json) = d.graph.as_ref() else {
        return d.apply_cad_edit(edit);
    };
    let mut g = Graph::deserialize(json).map_err(|e| anyhow::anyhow!("The design's graph would not read: {e}"))?;
    let plan = plan(&g, edit)?;
    let applied = encode(&mut g, plan, edit)?;
    d.graph = Some(serde_json::to_value(&g)?);
    Ok(applied)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_stops_before_later_resizes_and_their_errors() {
        let d = ringdesign_core::cad::examples::design("two-part-signet").unwrap();
        let mut g = from_document(&d).unwrap();
        let shank = NodeId(d.cad.as_ref().unwrap().features[0].id);
        append_resize(&mut g, 19.123, &Default::default()).unwrap();
        append_resize(&mut g, 0.1, &Default::default()).unwrap();
        let before = g.clone();
        let mut ev = crate::eval::Evaluator::new();
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        assert!(crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).is_err());
        let earlier = history_design(&mut ev, &g, shank, &reg, &lib).unwrap();
        assert_eq!(earlier.size, d.size);
        let doc = earlier.cad.as_ref().unwrap();
        assert_eq!(doc.features.len(), 1);
        assert_eq!(doc.outputs, vec![shank.0]);
        assert_eq!(
            serde_json::to_value(&doc.features[0]).unwrap(),
            serde_json::to_value(&d.cad.as_ref().unwrap().features[0]).unwrap()
        );
        assert_eq!(g, before);
    }
    #[test]
    fn a_builder_appended_round_a_stone_takes_its_own_component_never_the_stones() {
        use ringdesign_core::cad::{Attach, ComponentRole, builders};
        let d = ringdesign_core::cad::examples::design("claw-solitaire").unwrap();
        let mut g = from_document(&d).unwrap();
        let id = append(&mut g, Operation::Builder { key: builders::HALO.into(), on: Some(2), params: serde_json::json!({}) }).unwrap();
        let f: Feature = serde_json::from_value(g.node(id).unwrap().params.clone()).unwrap();
        assert_eq!((f.component.reference, f.component.attach, f.component.role), (false, Attach::Join, ComponentRole::Setting));
        assert_eq!(f.name, "Halo");
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let out = crate::eval::evaluate_design(&mut crate::eval::Evaluator::new(), &g, &reg, &lib, 0).unwrap();
        let doc = out.design.cad.clone().unwrap();
        assert_eq!(doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![1, 2, 3, 4, id.0]);
        assert_eq!(doc.dependents(2), vec![3, 4, id.0], "every setting reads its stone");
        assert!(doc.outputs.contains(&2), "and the stone stays a part beside them");
    }
    #[test]
    fn features_recompute_and_suppression_is_persistent() {
        let d = RingDesign::default();
        let mut g = start(&d).unwrap();
        let id = append(&mut g, Operation::Box { size: [4.0; 3] }).unwrap();
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let mut ev = crate::eval::Evaluator::new();
        let out = crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).unwrap();
        assert_eq!(out.design.cad.as_ref().unwrap().features[0].id, id.0);
        g.set_input(id, "enabled", crate::value::Literal::Bool(false))
            .unwrap();
        let out = crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).unwrap();
        assert!(!out.design.cad.as_ref().unwrap().features[0].enabled);
    }
    #[test]
    fn lifting_preserves_components_outputs_joints_and_resize_provenance() {
        let d = ringdesign_core::cad::examples::design("solitaire").unwrap();
        let mut g: Graph =
            serde_json::from_value(serde_json::to_value(from_document(&d).unwrap()).unwrap())
                .unwrap();
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let mut ev = crate::eval::Evaluator::new();
        let out = crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).unwrap();
        assert_eq!(
            serde_json::to_value(&d.cad).unwrap(),
            serde_json::to_value(&out.design.cad).unwrap()
        );
        append_resize(&mut g, 19.123, &Default::default()).unwrap();
        g = serde_json::from_value(serde_json::to_value(g).unwrap()).unwrap();
        let resized = crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).unwrap();
        assert!((resized.design.size.inner_diameter_mm() - 19.123).abs() < 1e-9);
        let stone = |d: &RingDesign| {
            serde_json::to_value(
                d.cad
                    .as_ref()
                    .unwrap()
                    .features
                    .iter()
                    .find(|f| f.component.reference)
                    .unwrap(),
            )
            .unwrap()
        };
        assert_eq!(stone(&d), stone(&resized.design));
        assert!(g.nodes.iter().any(|n| n.kind == "design.resize"));
    }
    #[test]
    fn a_graph_built_by_append_takes_edits_and_grows_property_nodes_only_when_it_must() {
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let mut ev = crate::eval::Evaluator::new();
        let mut g = start(&RingDesign::default()).unwrap();
        let a = append(&mut g, Operation::Box { size: [4.0; 3] }).unwrap();
        let b = append(&mut g, Operation::Sphere { radius_mm: 1.5 }).unwrap();
        assert_eq!(document(&g).unwrap().outputs, vec![a.0, b.0]);
        assert!(!g.nodes.iter().any(|n| n.kind == "design.set"));
        // An add at the end keeps the replayed outputs: no property node is needed.
        let cyl = Feature { id: 0, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Default::default() };
        let c = apply_edit(&mut g, &CadEdit::Add { feature: cyl.clone(), after: None }).unwrap().id.unwrap();
        assert!(!g.nodes.iter().any(|n| n.kind == "design.set"));
        // One in the middle pushes its id last in the outputs, which only a property node can say.
        let d = apply_edit(&mut g, &CadEdit::Add { feature: cyl, after: Some(a.0) }).unwrap().id.unwrap();
        let doc = document(&g).unwrap();
        assert_eq!(doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![a.0, d, b.0, c]);
        assert_eq!(doc.outputs, vec![a.0, b.0, c, d]);
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "design.set").count(), 1);
        let out = crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).unwrap();
        assert_eq!(serde_json::to_value(&out.design.cad).unwrap(), serde_json::to_value(Some(&doc)).unwrap());
        // Suppression rides the pin as well as the params.
        apply_edit(&mut g, &CadEdit::Enable { id: d, enabled: false }).unwrap();
        assert_eq!(g.node(NodeId(d)).unwrap().inputs.get("enabled"), Some(&crate::value::Literal::Bool(false)));
        assert!(!document(&g).unwrap().feature(d).unwrap().enabled);
        apply_edit(&mut g, &CadEdit::Enable { id: d, enabled: true }).unwrap();
        assert!(document(&g).unwrap().feature(d).unwrap().enabled);
        // Emptying the document takes its property nodes with it, and the graph still evaluates.
        for id in [d, c, b.0, a.0] {
            apply_edit(&mut g, &CadEdit::Remove { id }).unwrap();
        }
        assert!(g.nodes.iter().all(|n| n.kind != "design.set" && n.kind != "cad.feature"));
        let out = crate::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0).unwrap();
        assert!(out.design.cad.is_none());
        let e = apply_edit(&mut g, &CadEdit::Rename { id: a.0, name: "x".into() }).unwrap_err();
        assert!(e.message.contains("No feature"), "{e}");
    }
    #[test]
    fn a_chain_read_only_by_evaluating_refuses_every_edit_and_names_the_node() {
        let boxed = || {
            let mut g = start(&RingDesign::default()).unwrap();
            let a = append(&mut g, Operation::Box { size: [4.0; 3] }).unwrap();
            (g, a)
        };
        let mut cases = Vec::new();
        let (mut g, a) = boxed();
        let src = g.add("design.get").unwrap();
        g.connect(src, "value", a, "operation").unwrap();
        cases.push((g, a, a, "takes its operation from a wire"));
        let (mut g, a) = boxed();
        let sphere = serde_json::to_value(Operation::Sphere { radius_mm: 1.0 }).unwrap();
        g.set_input(a, "operation", Literal::Json(sphere)).unwrap();
        cases.push((g, a, a, "takes its operation from its pin"));
        let (mut g, a) = boxed();
        let src = g.add("design.get").unwrap();
        g.connect(src, "value", a, "enabled").unwrap();
        cases.push((g, a, a, "suppressed by a wire"));
        let (mut g, a) = boxed();
        g.set_input(a, "enabled", Literal::Text("no".into())).unwrap();
        cases.push((g, a, a, "holds no boolean"));
        let (mut g, a) = boxed();
        let whole = append_property(&mut g, "/cad", serde_json::to_value(Document::default()).unwrap()).unwrap();
        cases.push((g, a, whole, "writes /cad whole"));
        let (mut g, a) = boxed();
        let outputs = append_property(&mut g, OUTPUTS, serde_json::json!([a.0])).unwrap();
        g.node_mut(outputs).unwrap().params = serde_json::Value::Null;
        let src = g.add("design.get").unwrap();
        g.connect(src, "value", outputs, "value").unwrap();
        cases.push((g, a, outputs, "/cad/outputs comes from a wire"));
        for (mut g, feature, blamed, expected) in cases {
            let before = g.clone();
            let e = apply_edit(&mut g, &CadEdit::Rename { id: feature.0, name: "x".into() }).unwrap_err();
            assert!(e.message.contains(expected) && e.message.ends_with("edit it in the graph"), "{e}");
            assert_eq!(e.node, Some(blamed), "{e}");
            assert_eq!(g, before);
        }
    }
    #[test]
    fn an_asked_for_identity_is_kept_unless_another_node_carries_it() {
        let mut g = start(&RingDesign::default()).unwrap();
        append(&mut g, Operation::Box { size: [4.0; 3] }).unwrap();
        let sink = g.nodes.iter().find(|n| n.kind == crate::eval::OUTPUT_KIND).unwrap().id;
        let mut post = Feature { id: sink.0, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Default::default() };
        let before = g.clone();
        let e = apply_edit(&mut g, &CadEdit::Add { feature: post.clone(), after: None }).unwrap_err();
        assert!(e.message.contains("names another node"), "{e}");
        assert_eq!(g, before);
        post.id = 40;
        assert_eq!(apply_edit(&mut g, &CadEdit::Add { feature: post, after: None }).unwrap().id, Some(40));
        assert!(g.node(NodeId(40)).is_some_and(|n| n.kind == "cad.feature") && g.next_id == 41);
        assert!(document(&g).unwrap().outputs.ends_with(&[40]));
    }
    #[test]
    fn the_lifts_property_nodes_stay_one_per_field_after_the_last_feature() {
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let d = ringdesign_core::cad::examples::design("two-part-signet").unwrap();
        let mut g = from_document(&d).unwrap();
        // Adds a second joints node and a part after it, as the CAD pane's buttons do.
        append_property(&mut g, JOINTS, serde_json::json!([])).unwrap();
        let post = append(&mut g, Operation::Cylinder { radius_mm: 0.5, height_mm: 3.0 }).unwrap();
        let mut doc = document(&g).unwrap();
        assert!(doc.joints.is_empty());
        assert_eq!(doc.outputs, vec![1, 2, post.0]);
        let rename = CadEdit::Rename { id: 1, name: "Hoop".into() };
        doc.apply(&rename).unwrap();
        apply_edit(&mut g, &rename).unwrap();
        let chain = design_chain(&g);
        let fields: Vec<_> = chain.iter().filter_map(|id| g.node(*id).and_then(property_of)).collect();
        assert_eq!(fields, vec![OUTPUTS, THROUGH, JOINTS], "the stale joints node is gone and the rest follow the part");
        let last = chain.iter().rposition(|id| is_feature(&g, *id)).unwrap();
        assert!(chain.iter().position(|id| is_property(&g, *id)).unwrap() > last);
        let out = crate::eval::evaluate_design(&mut crate::eval::Evaluator::new(), &g, &reg, &lib, 0).unwrap();
        assert_eq!(serde_json::to_string(&out.design.cad).unwrap(), serde_json::to_string(&Some(&doc)).unwrap());
    }
    #[test]
    fn rewiring_keeps_the_output_a_design_leaves_by() {
        let mut g = start(&RingDesign::default()).unwrap();
        let a = append(&mut g, Operation::Box { size: [4.0; 3] }).unwrap();
        let b = append(&mut g, Operation::Sphere { radius_mm: 1.5 }).unwrap();
        // Heads the chain with an output named `ring`, as a cluster may.
        let src = g.nodes[0].id;
        g.connect(src, "ring", a, "design").unwrap();
        let wire = |g: &Graph, to: NodeId| g.wire_into(to, "design").map(|w| (w.from, w.out.clone()));
        apply_edit(&mut g, &CadEdit::Move { id: b.0, after: None }).unwrap();
        assert_eq!(wire(&g, b), Some((src, "ring".into())));
        assert_eq!(wire(&g, a), Some((b, "design".into())));
        apply_edit(&mut g, &CadEdit::Remove { id: b.0 }).unwrap();
        assert_eq!(wire(&g, a), Some((src, "ring".into())));
    }
    #[test]
    fn an_add_on_a_bare_source_grows_the_sink_append_would() {
        let reg = Registry::builtin();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let mut g = start(&RingDesign::default()).unwrap();
        let post = Feature { id: 0, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Default::default() };
        let id = apply_edit(&mut g, &CadEdit::Add { feature: post, after: None }).unwrap().id.unwrap();
        let sink = g.nodes.iter().find(|n| n.kind == crate::eval::OUTPUT_KIND).unwrap().id;
        assert_eq!(g.wire_into(sink, crate::eval::OUTPUT_DESIGN_PIN).unwrap().from, NodeId(id));
        let out = crate::eval::evaluate_design(&mut crate::eval::Evaluator::new(), &g, &reg, &lib, 0).unwrap();
        let doc = out.design.cad.as_ref().unwrap();
        assert_eq!((doc.features.len(), doc.outputs.clone()), (1, vec![id]));
    }
}
