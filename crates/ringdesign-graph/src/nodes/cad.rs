//! CAD features return designs so desktop, CLI, and manufacturing all build
//! the same source program. Node identities double as stable feature names.
use crate::{
    graph::{Graph, Mode, Node, NodeId},
    registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry},
    value::{Value, ValueKind},
};
use ringdesign_core::{
    RingDesign,
    cad::{Document, Feature, Operation},
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
    let mut component = operation
        .sources()
        .first()
        .and_then(|source| g.node(NodeId(*source)))
        .and_then(|n| serde_json::from_value::<Feature>(n.params.clone()).ok())
        .map(|f| f.component)
        .unwrap_or_default();
    component.ring_anchor_deg = None;
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
}
