//! Our parts as requests, and what a worker built as features that keep it, with no OpenCascade in it.
use crate::protocol::{Built, Cylinder, EdgeProbe, Frame, Request, Tolerance, Torus};
use anyhow::{Context, Result, ensure};
use cadkernel::brep;
use ringdesign_core::cad::{self, Component, Document, EdgeRef, Evaluated, EvaluatedComponent, Feature, Operation, Placement, stored::Recipe};
use ringdesign_core::sketch::Id;

/// Points a probe carries along an edge: its ends, its quarters and its middle.
const PROBE_POINTS: usize = 5;

/// The part's kernel body taken back into the frame the build seated it by.
pub fn local_body(c: &EvaluatedComponent) -> Result<brep::Body> {
    let body = c.brep().with_context(|| format!("#{} {} is a mesh; OpenCascade reshapes a kernel part", c.id, c.name))?;
    brep::transform(body, &cad::pattern::inverse(&c.frame)).context("The part would not move into its own frame")
}

/// `body`, part `c` in its own frame, as a STEP file through the kernel's own export.
pub fn step_of(c: &EvaluatedComponent, body: brep::Body) -> Result<String> {
    let mut local = c.clone();
    local.body = body;
    local.made = None;
    let e = Evaluated { components: vec![local], features: Vec::new(), band: None, planes: Vec::new() };
    cad::step::export_with(&e, &c.name, &|_| true, &[])
}

/// Edge `ordinal` of `body` as points along it.
pub fn probe(body: &brep::Body, ordinal: usize) -> Result<EdgeProbe> {
    let (key, _) = body.edges.iter().nth(ordinal).with_context(|| format!("The part has no edge {ordinal}"))?;
    let points = brep::edge_points(body, key, 0.02).filter(|p| !p.is_empty()).with_context(|| format!("Edge {ordinal} has no points the kernel can sample"))?;
    let last = points.len() - 1;
    Ok(EdgeProbe { points: (0..PROBE_POINTS).map(|k| points[last * k / (PROBE_POINTS - 1)]).collect() })
}

/// Edges `edges` of part `c` rounded by `radius_mm`, as a request, and the recipe's parameters: the edges signed in the part's frame.
pub fn fillet(c: &EvaluatedComponent, edges: &[usize], radius_mm: f64, tolerance: Tolerance) -> Result<(Request, serde_json::Value)> {
    ensure!(!edges.is_empty(), "Choose at least one edge of #{} {} to round", c.id, c.name);
    let body = local_body(c)?;
    let probes = edges.iter().map(|e| probe(&body, *e)).collect::<Result<Vec<_>>>()?;
    let signed: Vec<EdgeRef> = edges.iter().map(|e| EdgeRef::signed(&c.body, *e, &c.frame)).collect();
    let params = serde_json::json!({ "edges": signed, "radius_mm": radius_mm, "tolerance": tolerance });
    Ok((Request::Fillet { step: step_of(c, body)?, edges: probes, radius_mm, tolerance }, params))
}

/// Part `c` hollowed to `thickness_mm` walls, the faces nearest `open` (in its own frame) left open, as a request and its parameters.
pub fn shell(c: &EvaluatedComponent, open: &[[f64; 3]], thickness_mm: f64, tolerance: Tolerance) -> Result<(Request, serde_json::Value)> {
    let body = local_body(c)?;
    let params = serde_json::json!({ "open": open, "thickness_mm": thickness_mm, "tolerance": tolerance });
    Ok((Request::Shell { step: step_of(c, body)?, open: open.to_vec(), thickness_mm, tolerance }, params))
}

/// A torus feature and a cylinder part seated on the ring, fused and rounded by `radius_mm`, as a request and its parameters.
pub fn junction(doc: &Document, torus: Id, cylinder: &EvaluatedComponent, radius_mm: f64, tolerance: Tolerance) -> Result<(Request, serde_json::Value)> {
    let Some(Operation::Torus { major_mm, minor_mm }) = doc.feature(torus).map(|f| &f.operation) else {
        anyhow::bail!("#{torus} is not a torus");
    };
    ensure!(doc.feature(torus).is_some_and(|f| f.component.placement == Placement::Free), "The torus #{torus} stands free, round the finger's axis");
    let Some(Operation::Cylinder { radius_mm: r, height_mm: h }) = doc.feature(cylinder.id).map(|f| &f.operation) else {
        anyhow::bail!("#{} {} is not a cylinder", cylinder.id, cylinder.name);
    };
    let f = cylinder.frame;
    let (torus, cylinder) = (
        Torus { major_mm: *major_mm, minor_mm: *minor_mm },
        Cylinder { radius_mm: *r, height_mm: *h, frame: Frame { origin: f.origin, x: f.x_axis, y: f.y_axis, z: f.z_axis } },
    );
    let params = serde_json::json!({ "torus": torus, "cylinder": cylinder, "radius_mm": radius_mm, "tolerance": tolerance });
    Ok((Request::Junction { torus, cylinder, radius_mm, tolerance }, params))
}

/// A feature keeping what the worker built for `request` in place of `sources`, with the recipe to run it again.
pub fn stored_feature(doc: &Document, sources: &[Id], request: &Request, params: serde_json::Value, built: &Built, component: Component) -> Feature {
    let digest = if sources.is_empty() { String::new() } else { cad::stored::digest(doc, sources) };
    let recipe = Recipe { kernel: "occt".into(), op: request.op().into(), params, digest };
    Feature {
        id: 0,
        name: recipe.label().into(),
        enabled: true,
        operation: Operation::Stored { recipe, sources: sources.to_vec(), mesh: built.mesh.clone() },
        component: Component { placement: if sources.is_empty() { component.placement } else { Placement::Free }, ..component },
    }
}
