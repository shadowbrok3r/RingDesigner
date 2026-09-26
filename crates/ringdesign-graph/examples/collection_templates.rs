//! Package a collection's portable graphs after verifying cold source and mesh parity.
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

use anyhow::{Context, Result, bail, ensure};
use ringdesign_core::{AlphaLibrary, BuildParams, RingDesign, library, manufacturing, mesh};
use ringdesign_graph::{
    eval::{self, Evaluator},
    file,
    graph::Graph,
    lift,
    registry::Registry,
    templates,
    value::{Literal, ValueKind},
};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "ControlSpec")]
enum Control {
    Name(String),
    /// A new control driving every pin `bind` selects, clamped when it carries an interval.
    Bound {
        name: String,
        bind: Vec<Bind>,
        min: Option<f64>,
        max: Option<f64>,
    },
    Bounded {
        name: String,
        min: f64,
        max: f64,
    },
}

/// The pins a bound control drives: nodes of `kind` or labelled `label` whose pins match `with`, at `input`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bind {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    with: serde_json::Map<String, serde_json::Value>,
    input: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ControlSpec {
    Name(String),
    Fields(ControlFields),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlFields {
    name: String,
    #[serde(default)]
    bind: Option<Vec<Bind>>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
}

impl TryFrom<ControlSpec> for Control {
    type Error = String;
    fn try_from(spec: ControlSpec) -> Result<Self, String> {
        let f = match spec {
            ControlSpec::Name(name) => return Ok(Self::Name(name)),
            ControlSpec::Fields(f) => f,
        };
        match (f.bind, f.min, f.max) {
            (Some(bind), min, max) if min.is_some() == max.is_some() => Ok(Self::Bound { name: f.name, bind, min, max }),
            (None, Some(min), Some(max)) => Ok(Self::Bounded { name: f.name, min, max }),
            (None, None, None) => Ok(Self::Name(f.name)),
            _ => Err(format!("{}: an interval needs both min and max", f.name)),
        }
    }
}

impl Control {
    fn name(&self) -> &str {
        match self {
            Self::Name(name) | Self::Bound { name, .. } | Self::Bounded { name, .. } => name,
        }
    }

    fn interval(&self) -> Option<(f64, f64)> {
        match self {
            Self::Bounded { min, max, .. } | Self::Bound { min: Some(min), max: Some(max), .. } => Some((*min, *max)),
            _ => None,
        }
    }
}

/// The literal a pin holds, or its registry default.
fn pin_literal(node: &ringdesign_graph::graph::Node, pin: &str, reg: &Registry) -> Option<(ValueKind, Literal)> {
    let (inputs, _) = reg.node_pins(node)?;
    let spec = inputs.into_iter().find(|p| p.name == pin)?;
    let literal = node.inputs.get(pin).or(spec.default.as_ref())?.clone();
    Some((spec.kind, literal))
}

/// Adds a number node carrying the bound pins' shared value, wires it into each of them and exposes it as `name`.
fn bind_control(graph: &mut Graph, name: &str, bind: &[Bind], reg: &Registry) -> Result<()> {
    let same = |a: &serde_json::Value, b: &serde_json::Value| match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    };
    let mut targets = Vec::new();
    for b in bind {
        ensure!(b.kind.is_some() || b.label.is_some(), "{name}: a binding names a node kind or a label");
        let found = targets.len();
        for n in &graph.nodes {
            let hit = b.kind.as_deref().is_none_or(|k| n.kind == k)
                && b.label.as_deref().is_none_or(|l| n.label.as_deref() == Some(l))
                && b.with.iter().all(|(k, v)| {
                    pin_literal(n, k, reg)
                        .and_then(|(_, l)| serde_json::to_value(l).ok())
                        .is_some_and(|x| same(&x, v))
                });
            if hit {
                targets.push((n.id, b.input.clone()));
            }
        }
        ensure!(targets.len() > found, "{name}: {b:?} binds no pin");
    }
    targets.sort();
    targets.dedup();
    let mut value = None;
    for (id, pin) in &targets {
        ensure!(graph.wire_into(*id, pin).is_none(), "{name}: pin {pin} of node {id} is already driven");
        let node = graph.node(*id).expect("bound node");
        let x = match pin_literal(node, pin, reg) {
            Some((ValueKind::Number | ValueKind::Int, Literal::Number(x))) => x,
            Some((ValueKind::Number | ValueKind::Int, Literal::Int(x))) => x as f64,
            other => bail!("{name}: pin {pin} of node {id} carries {other:?}, not a number"),
        };
        ensure!(x.is_finite(), "{name}: pin {pin} of node {id} is not finite");
        let first = *value.get_or_insert(x);
        ensure!((first - x).abs() < 1e-12, "{name}: its pins carry different values {first} and {x}");
    }
    let value = value.context("no bound pin")?;
    let [x, y] = graph.node(targets[0].0).expect("bound node").pos;
    let source = graph.add("number")?;
    graph.set_input(source, "value", Literal::Number(value))?;
    let node = graph.node_mut(source).expect("new number");
    node.label = Some(name.to_string());
    node.pos = [x - 260.0, y];
    for (id, pin) in &targets {
        graph.clear_input(*id, pin)?;
        graph.connect(source, "out", *id, pin.clone())?;
    }
    graph.expose(source, "value", name)?;
    let exposed = graph.exposed.last_mut().expect("new exposure");
    exposed.doc = format!("Drives {} pins that shared {value}.", targets.len());
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
struct Ring {
    slug: String,
    #[serde(default)]
    authored_graph: Option<PathBuf>,
    #[serde(default)]
    exposed_controls: Option<Vec<Control>>,
    #[serde(default)]
    template_class: Option<String>,
}

#[derive(Deserialize)]
struct Manifest {
    collection: String,
    rings: Vec<Ring>,
}

struct Options {
    collection: String,
    source: PathBuf,
    output: PathBuf,
    templates: PathBuf,
    check_bundled: bool,
    verify_export: bool,
    only: Option<String>,
}

fn safe_slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.as_bytes()[s.len() - 1].is_ascii_alphanumeric()
}

fn local_file(root: &Path, file: &Path) -> Result<PathBuf> {
    ensure!(
        !file.as_os_str().is_empty()
            && file.components().all(|c| matches!(c, Component::Normal(_))),
        "expected a relative file inside {}: {}",
        root.display(),
        file.display()
    );
    let path = root.join(file).canonicalize()?;
    ensure!(
        path.starts_with(root.canonicalize()?),
        "file leaves source directory: {}",
        file.display()
    );
    Ok(path)
}

fn options(repo: &Path) -> Result<Options> {
    let mut args = std::env::args().skip(1);
    let collection = args.next().context("usage: collection_templates COLLECTION [SOURCE_DIR] [--output-dir DIR] [--templates-dir DIR] [--only SLUG] [--check-bundled] [--verify-export]")?;
    ensure!(
        safe_slug(&collection),
        "invalid collection name {collection:?}"
    );
    let mut source = None;
    let mut output = None;
    let mut graphs = None;
    let mut only = None;
    let mut check_bundled = false;
    let mut verify_export = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => {
                output = Some(PathBuf::from(
                    args.next().context("--output-dir needs a path")?,
                ))
            }
            "--templates-dir" => {
                graphs = Some(PathBuf::from(
                    args.next().context("--templates-dir needs a path")?,
                ))
            }
            "--only" => only = Some(args.next().context("--only needs a slug")?),
            "--check-bundled" => check_bundled = true,
            "--verify-export" => verify_export = true,
            _ if !arg.starts_with('-') && source.is_none() => source = Some(PathBuf::from(arg)),
            _ => bail!("unknown argument {arg:?}"),
        }
    }
    let source = source.unwrap_or_else(|| repo.join("showcase").join(&collection));
    let templates = graphs.unwrap_or_else(|| {
        output
            .as_ref()
            .map_or_else(|| repo.join("graphs/templates"), |p| p.join("templates"))
    });
    Ok(Options {
        collection,
        output: output.unwrap_or_else(|| source.clone()),
        source,
        templates,
        check_bundled,
        verify_export,
        only,
    })
}

fn manifest(opts: &Options) -> Result<Manifest> {
    let path = opts.source.join("collection.json");
    let legacy = !path.exists() && opts.collection == "reptilia";
    let manifest = if legacy {
        Manifest {
            collection: "reptilia".into(),
            rings: ["ecdysis", "tessera", "lorica", "ophidian", "varanus"]
                .map(|slug| Ring {
                    slug: slug.into(),
                    authored_graph: None,
                    exposed_controls: None,
                    template_class: None,
                })
                .into(),
        }
    } else {
        serde_json::from_slice::<Manifest>(
            &std::fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )?
    };
    ensure!(
        manifest.collection == opts.collection,
        "collection name differs from manifest"
    );
    ensure!(
        !manifest.rings.is_empty() && manifest.rings.len() <= 100,
        "a collection has 1–100 rings"
    );
    let mut names = HashSet::new();
    for ring in &manifest.rings {
        ensure!(
            safe_slug(&ring.slug) && names.insert(&ring.slug),
            "invalid or repeated slug {:?}",
            ring.slug
        );
        ensure!(
            legacy || ring.exposed_controls.is_some(),
            "{}: declare exposed_controls explicitly; [] exposes no controls",
            ring.slug
        );
        ensure!(
            ring.template_class
                .as_deref()
                .is_none_or(|c| matches!(c, "procedural" | "stock" | "painted")),
            "{}: template_class must be procedural, stock, or painted",
            ring.slug
        );
    }
    Ok(manifest)
}

fn controls(graph: &mut Graph, allow: Option<&[Control]>, reg: &Registry) -> Result<()> {
    let Some(allow) = allow else { return Ok(()) };
    let mut names = HashSet::new();
    let mut kept = Vec::new();
    for control in allow {
        ensure!(
            names.insert(control.name()),
            "duplicate exposed control {:?}",
            control.name()
        );
        if let Control::Bound { name, bind, .. } = control {
            bind_control(graph, name, bind, reg)?;
        }
        let mut exposed = graph
            .exposed
            .iter()
            .find(|e| e.name == control.name())
            .cloned()
            .with_context(|| format!("unknown exposed control {:?}", control.name()))?;
        if let Some((min, max)) = control.interval() {
            ensure!(
                min.is_finite() && max.is_finite() && min <= max,
                "{}: invalid control interval",
                control.name()
            );
            ensure!(
                graph.wire_into(exposed.node, &exposed.input).is_none(),
                "{}: bounded control is already driven",
                control.name()
            );
            let node = graph
                .node(exposed.node)
                .context("exposed node is missing")?;
            let (inputs, _) = reg
                .node_pins(node)
                .context("exposed node kind is missing")?;
            let pin = inputs
                .iter()
                .find(|p| p.name == exposed.input)
                .context("exposed input is missing")?;
            ensure!(
                pin.kind == ValueKind::Number,
                "{}: intervals require a numeric scalar input",
                control.name()
            );
            let initial = node
                .inputs
                .get(&exposed.input)
                .or(pin.default.as_ref())
                .context("bounded control needs a literal default")?;
            let value = match initial {
                Literal::Number(n) => *n,
                Literal::Int(n) => *n as f64,
                _ => bail!(
                    "{}: bounded control needs a numeric literal",
                    control.name()
                ),
            };
            ensure!(
                value.is_finite() && value >= min && value <= max,
                "{}: current value {value} lies outside [{min}, {max}]",
                control.name()
            );
            let position = node.pos;
            let clamp = graph.add("math.clamp")?;
            graph.set_input(clamp, "x", Literal::Number(value))?;
            graph.set_input(clamp, "min", Literal::Number(min))?;
            graph.set_input(clamp, "max", Literal::Number(max))?;
            let node = graph.node_mut(clamp).expect("new clamp");
            node.label = Some(format!("{} · verified range", control.name()));
            node.pos = [position[0] - 260.0, position[1]];
            graph.connect(clamp, "out", exposed.node, exposed.input.clone())?;
            exposed.node = clamp;
            exposed.input = "x".into();
            exposed.doc =
                format!("Verified interval {min}–{max}; clamped before geometry evaluation.");
        }
        kept.push(exposed);
    }
    graph.exposed = kept;
    ensure!(
        graph.validate(Some(reg)).is_empty(),
        "control policy produced an invalid graph"
    );
    Ok(())
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn source_equal(a: &RingDesign, b: &RingDesign) -> Result<bool> {
    Ok(serde_json::to_value(a)? == serde_json::to_value(b)?)
}

fn graph_patches(graph: &Graph, depth: usize) -> Result<Vec<serde_json::Value>> {
    ensure!(
        depth <= ringdesign_graph::MAX_CLUSTER_DEPTH,
        "cluster depth exceeds the host limit"
    );
    let mut patches = Vec::new();
    for node in &graph.nodes {
        if node.kind == "design.set" {
            patches.push(json!({"graph": graph.name, "node": node.id, "pointer": node.inputs.get("pointer")}));
        }
        if node.kind == "cluster" {
            let inner = ringdesign_graph::nodes::cluster::embedded(node)
                .context("cluster has no embedded graph")?;
            patches.extend(graph_patches(&inner, depth + 1)?);
        }
    }
    Ok(patches)
}

fn verify(opts: &Options, ring: &Ring, repo: &Path, reg: &Registry) -> Result<serde_json::Value> {
    let source = opts.source.join(&ring.slug);
    let start = Instant::now();
    let original = library::load_design(source.join("design.ring.json"))?;
    let source_read_ms = ms(start);
    let carried_graph = original.graph.clone();
    let mut design = templates::refine_sources(&original);
    design.graph = None;
    let reloaded = library::load_design_str(&library::design_json(&design)?)?;
    ensure!(
        source_equal(&design, &reloaded)?,
        "{}: portable design reload changed source",
        ring.slug
    );
    let lib = manufacturing::source_library(&design, &AlphaLibrary::default()).into_owned();
    let start = Instant::now();
    let (mut graph, method) = if let Some(path) = &ring.authored_graph {
        (
            file::load_graph(local_file(&source, path)?, Some(reg))?,
            "authored_file",
        )
    } else if let Some(value) = carried_graph {
        (
            file::load_graph_str(&serde_json::to_string(&value)?, Some(reg))?,
            "carried_graph",
        )
    } else {
        (lift::from_design(&design, reg, &lib)?, "lift")
    };
    let graph_creation_ms = ms(start);
    controls(&mut graph, ring.exposed_controls.as_deref(), reg)?;
    let text = file::graph_to_string(&graph)?;
    let start = Instant::now();
    let graph = file::load_graph_str(&text, Some(reg))?;
    let read_ms = ms(start);
    let mut evaluator = Evaluator::with_exprs(ringdesign_script::engine());
    let empty = AlphaLibrary::default();
    let start = Instant::now();
    let (cold_design, report) =
        eval::design_of(&mut evaluator, &graph, reg, &empty, empty.revision())?;
    let evaluate_ms = ms(start);
    ensure!(
        report.notes(&graph).is_empty(),
        "{}: {:?}",
        ring.slug,
        report.notes(&graph)
    );
    ensure!(
        source_equal(&cold_design, &design)?,
        "{}: graph changed source",
        ring.slug
    );
    let start = Instant::now();
    let mut cold = AlphaLibrary::default();
    cold_design
        .unpack_and_bake_observed(&mut cold, &|_, _| {}, &AtomicBool::new(false))
        .context("cold bake cancelled")?;
    let bake_ms = ms(start);
    let start = Instant::now();
    let mut second = AlphaLibrary::default();
    cold_design
        .unpack_and_bake_observed(&mut second, &|_, _| {}, &AtomicBool::new(false))
        .context("warm bake cancelled")?;
    let bake_again_ms = ms(start);
    let params = BuildParams {
        theta_steps: 384,
        profile_steps: 144,
        ..Default::default()
    };
    let start = Instant::now();
    mesh::try_build(&cold_design, &cold, params)?;
    let first_build_ms = ms(start);
    let start = Instant::now();
    let findings = ringdesign_core::dfm::findings_in(&cold_design, &cold);
    let detail_ms = ms(start);
    let start = Instant::now();
    ringdesign_core::dfm::findings_in(&cold_design, &cold);
    let detail_again_ms = ms(start);
    let start = Instant::now();
    let first = eval::judge(&mut evaluator, cold_design.clone(), report, &graph, &cold);
    let verdict_ms = ms(start);
    let start = Instant::now();
    let again = eval::evaluate_design_onto(
        &mut evaluator,
        &graph,
        reg,
        &empty,
        first.baked_library.as_deref().unwrap_or(&cold),
    )?;
    let rebuild_ms = ms(start);
    ensure!(
        source_equal(&again.design, &design)?,
        "{}: warm rebuild changed source",
        ring.slug
    );
    let parity_params = if opts.verify_export {
        BuildParams {
            theta_steps: 1536,
            profile_steps: 448,
            refine: None,
            ..design.build
        }
    } else {
        design.build
    };
    let before = mesh::try_build(&design, &lib, parity_params)?;
    let after = mesh::try_build(&cold_design, &cold, parity_params)?;
    ensure!(
        before.mesh.vertices == after.mesh.vertices
            && before.mesh.faces == after.mesh.faces
            && before.mesh.normals == after.mesh.normals,
        "{}: graph changed geometry",
        ring.slug
    );
    let editable = RingDesign {
        graph: Some(serde_json::to_value(&graph)?),
        ..design.clone()
    };
    let editable_text = library::design_json(&editable)?;
    let editable_reload = library::load_design_str(&editable_text)?;
    ensure!(
        source_equal(&editable, &editable_reload)?,
        "{}: editable file changed source",
        ring.slug
    );
    let editable_graph = file::load_graph_str(
        &serde_json::to_string(
            editable_reload
                .graph
                .as_ref()
                .context("editable file lost graph")?,
        )?,
        Some(reg),
    )?;
    let (editable_result, _) = eval::design_of(
        &mut Evaluator::with_exprs(ringdesign_script::engine()),
        &editable_graph,
        reg,
        &empty,
        empty.revision(),
    )?;
    ensure!(
        source_equal(&editable_result, &design)?,
        "{}: editable file changed graph result",
        ring.slug
    );
    let patches = graph_patches(&graph, 0)?;
    ensure!(
        patches.len() <= 4,
        "{}: {} design.set patches exceed four",
        ring.slug,
        patches.len()
    );
    let class = ring
        .template_class
        .as_deref()
        .unwrap_or(if !design.embedded.is_empty() {
            "painted"
        } else if design.imported_base.is_some() {
            "stock"
        } else {
            "procedural"
        });
    let budget = match class {
        "stock" => 1_000_000,
        "painted" => 3_000_000,
        _ => 300_000,
    };
    let over_budget = text.len() > budget;
    if over_budget {
        eprintln!(
            "{}: {class} graph is {} bytes (budget {budget}); review required",
            ring.slug,
            text.len()
        );
    }
    let graph_name = format!("{}-{}.graph.json", ring.slug, opts.collection);
    if opts.check_bundled {
        ensure!(
            text == std::fs::read_to_string(repo.join("graphs/templates").join(&graph_name))?,
            "{}: graph differs from bundled bytes",
            ring.slug
        );
    }
    let packaging = json!({
        "cold_design_reload": true, "cold_graph_reload": true, "editable_graph_reload": true,
        "source_identical": true, "vertices_faces_normals_identical": true, "source_method": method,
        "geometry_check_resolution": {"theta_steps": parity_params.theta_steps, "profile_steps": parity_params.profile_steps, "refine": parity_params.refine},
        "export_geometry_verified": opts.verify_export,
        "triangles": after.mesh.faces.len(), "nodes": graph.nodes.len(), "design_set_patches": patches,
        "exposed_controls": graph.exposed.iter().map(|p| &p.name).collect::<Vec<_>>(),
        "exposed_control_policy": ring.exposed_controls.as_ref().map(|_| "explicit_allowlist").unwrap_or("legacy_reptilia"),
        "template_bytes": text.len(), "editable_design_bytes": editable_text.len(), "template_class": class,
        "size_budget_bytes": budget, "size_review_required": over_budget, "template_gate_passed": !over_budget, "detail_findings": findings.len(),
        "open_ms": { "unpack": serde_json::Value::Null, "read": read_ms, "evaluate": evaluate_ms, "bake": bake_ms,
            "bake_again": bake_again_ms, "first_build": first_build_ms, "detail": detail_ms, "detail_again": detail_again_ms, "verdict": verdict_ms, "rebuild": rebuild_ms },
        "timing_context": "Uncompressed graph and an empty library; process caches may be warm after source verification; preview 384x144; verdict 192x128.",
        "authoring_ms": { "source_read": source_read_ms, "graph_creation": graph_creation_ms }
    });
    // Keep the author's release, clamp-bite and manufacturing evidence when
    // refreshing the independently checked graph and timing fields.
    let prior = source.join("verification.json");
    let mut verification = if prior.exists() {
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(prior)?)?
    } else {
        json!({})
    };
    let evidence = verification
        .as_object_mut()
        .context("verification.json must contain an object")?;
    evidence.extend(packaging.as_object().expect("packaging object").clone());
    // A ring's own geometry gates and this template gate make its whole verdict.
    if let Some(geometry) = evidence.get("geometry_gates_passed").and_then(|v| v.as_bool()) {
        evidence.insert("gates_passed".into(), json!(geometry && !over_budget));
        evidence.remove("status");
    }
    let output = opts.output.join(&ring.slug);
    std::fs::create_dir_all(&output)?;
    std::fs::create_dir_all(&opts.templates)?;
    std::fs::write(opts.templates.join(graph_name), &text)?;
    std::fs::write(output.join("template.graph.json"), text)?;
    std::fs::write(output.join("editable-graph.ring.json"), editable_text)?;
    std::fs::write(
        output.join("verification.json"),
        serde_json::to_vec_pretty(&verification)?,
    )?;
    Ok(verification)
}

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let opts = options(&repo)?;
    let manifest = manifest(&opts)?;
    let reg = ringdesign_script::registry();
    let rings: Vec<_> = manifest
        .rings
        .iter()
        .filter(|r| opts.only.as_ref().is_none_or(|slug| slug == &r.slug))
        .collect();
    ensure!(
        !rings.is_empty(),
        "requested ring is not in this collection"
    );
    for ring in rings {
        let result = verify(&opts, ring, &repo, &reg).with_context(|| ring.slug.clone())?;
        println!(
            "{}: {} nodes; {} controls; {} identical triangles; {} graph bytes",
            ring.slug,
            result["nodes"],
            result["exposed_controls"].as_array().map_or(0, Vec::len),
            result["triangles"],
            result["template_bytes"]
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> (RingDesign, Registry, Graph) {
        let d = RingDesign::default();
        let reg = ringdesign_script::registry();
        let graph = lift::from_design(&d, &reg, &AlphaLibrary::default()).unwrap();
        (d, reg, graph)
    }

    #[test]
    fn empty_allowlist_removes_controls_without_changing_editable_geometry() {
        let (d, reg, mut graph) = baseline();
        let nodes = graph.nodes.clone();
        controls(&mut graph, Some(&[]), &reg).unwrap();
        assert!(graph.exposed.is_empty());
        assert_eq!(graph.nodes, nodes);
        let (got, _) = eval::design_of(
            &mut Evaluator::with_exprs(ringdesign_script::engine()),
            &graph,
            &reg,
            &AlphaLibrary::default(),
            0,
        )
        .unwrap();
        assert!(source_equal(&d, &got).unwrap());
    }

    #[test]
    fn bounded_control_cannot_drive_geometry_outside_its_verified_interval() {
        let (d, reg, mut graph) = baseline();
        // Packaging also accepts authored clusters, whose pins are dynamic.
        let ids = graph
            .nodes
            .iter()
            .filter(|n| n.kind == "band.profile")
            .map(|n| n.id)
            .collect::<Vec<_>>();
        graph.collapse(&ids, "Authored band").unwrap();
        let exposed = graph
            .exposed
            .iter()
            .find(|e| e.name == "Band width")
            .unwrap()
            .clone();
        let allow = [Control::Bounded {
            name: exposed.name,
            min: d.profile.width_mm - 1.0,
            max: d.profile.width_mm + 1.0,
        }];
        controls(&mut graph, Some(&allow), &reg).unwrap();
        let input = graph.exposed[0].clone();
        graph
            .set_input(input.node, &input.input, Literal::Number(100.0))
            .unwrap();
        let (got, _) = eval::design_of(
            &mut Evaluator::with_exprs(ringdesign_script::engine()),
            &graph,
            &reg,
            &AlphaLibrary::default(),
            0,
        )
        .unwrap();
        assert_eq!(got.profile.width_mm, d.profile.width_mm + 1.0);
        assert_eq!(graph.exposed.len(), 1);
    }

    /// Two milgrain rows sharing one bead height, lifted.
    fn two_rows() -> (RingDesign, Registry, Graph) {
        use ringdesign_core::{Layer, LayerEntry, field::MilgrainLayer};
        let mut d = RingDesign::default();
        for (name, v_mm) in [("Row, low", 0.6), ("Row, high", 1.4)] {
            let row = MilgrainLayer { v_mm, height_mm: 0.25, ..MilgrainLayer::default() };
            d.layers.layers.push(LayerEntry::new(name, Layer::Milgrain(row)));
        }
        let reg = ringdesign_script::registry();
        let graph = lift::from_design(&d, &reg, &AlphaLibrary::default()).unwrap();
        (d, reg, graph)
    }

    fn evaluated(graph: &Graph, reg: &Registry) -> RingDesign {
        let (got, report) = eval::design_of(
            &mut Evaluator::with_exprs(ringdesign_script::engine()),
            graph,
            reg,
            &AlphaLibrary::default(),
            0,
        )
        .unwrap();
        assert!(report.notes(graph).is_empty(), "{:?}", report.notes(graph));
        (*got).clone()
    }

    fn heights(graph: &Graph, reg: &Registry) -> Vec<f64> {
        evaluated(graph, reg)
            .layers
            .layers
            .iter()
            .map(|e| match &e.layer {
                ringdesign_core::Layer::Milgrain(m) => m.height_mm,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    fn milgrain(input: &str, with: serde_json::Value) -> Bind {
        Bind {
            kind: Some("layer.milgrain".into()),
            label: None,
            with: with.as_object().cloned().unwrap_or_default(),
            input: input.into(),
        }
    }

    #[test]
    fn a_bound_control_drives_every_pin_it_binds_inside_its_verified_interval() {
        let (d, reg, mut graph) = two_rows();
        let allow = [Control::Bound {
            name: "Bead height".into(),
            bind: vec![milgrain("height_mm", json!({}))],
            min: Some(0.2),
            max: Some(0.35),
        }];
        controls(&mut graph, Some(&allow), &reg).unwrap();
        assert!(source_equal(&d, &evaluated(&graph, &reg)).unwrap(), "binding changed the design");
        assert_eq!(graph.exposed.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Bead height"]);
        let input = graph.exposed[0].clone();
        graph.set_input(input.node, &input.input, Literal::Number(0.3)).unwrap();
        assert_eq!(heights(&graph, &reg), [0.3, 0.3]);
        graph.set_input(input.node, &input.input, Literal::Number(5.0)).unwrap();
        assert_eq!(heights(&graph, &reg), [0.35, 0.35]);
        // A filter on another pin selects one row; the other keeps its height.
        let (_, _, mut graph) = two_rows();
        let allow = [Control::Bound {
            name: "High row".into(),
            bind: vec![milgrain("height_mm", json!({"v_mm": 1.4}))],
            min: None,
            max: None,
        }];
        controls(&mut graph, Some(&allow), &reg).unwrap();
        let input = graph.exposed[0].clone();
        graph.set_input(input.node, &input.input, Literal::Number(0.4)).unwrap();
        assert_eq!(heights(&graph, &reg), [0.25, 0.4]);
    }

    #[test]
    fn a_bound_control_refuses_unbound_mixed_or_out_of_range_pins_and_malformed_entries() {
        let (_, reg, graph) = two_rows();
        let bound = |bind: Bind, name: &str, min: Option<f64>, max: Option<f64>| {
            let allow = [Control::Bound { name: name.into(), bind: vec![bind], min, max }];
            controls(&mut graph.clone(), Some(&allow), &reg)
        };
        let nowhere = Bind { label: Some("No such row".into()), ..milgrain("height_mm", json!({})) };
        assert!(bound(nowhere, "Bead height", None, None).is_err(), "binds nothing");
        assert!(bound(milgrain("v_mm", json!({})), "Rows", None, None).is_err(), "pins disagree");
        assert!(bound(milgrain("height_mm", json!({})), "Bead height", Some(0.3), Some(0.4)).is_err(), "value outside");
        let taken = graph.exposed[0].name.clone();
        assert!(bound(milgrain("height_mm", json!({})), &taken, None, None).is_err(), "name taken");
        for entry in [
            json!({"name": "X", "bind": [{"kind": "layer.milgrain", "inputs": "height_mm"}]}),
            json!({"name": "X", "bind": [{"kind": "layer.milgrain", "input": "height_mm"}], "max": 1.0}),
            json!({"name": "X", "min": 1.0}),
        ] {
            assert!(serde_json::from_value::<Control>(entry.clone()).is_err(), "{entry}");
        }
        let read = |entry| serde_json::from_value::<Control>(entry).unwrap();
        assert!(matches!(read(json!("X")), Control::Name(_)));
        assert!(matches!(read(json!({"name": "X", "min": 0.0, "max": 1.0})), Control::Bounded { .. }));
        assert!(matches!(
            read(json!({"name": "X", "bind": [{"label": "Row, low", "input": "height_mm"}], "min": 0.0, "max": 1.0})),
            Control::Bound { min: Some(_), max: Some(_), .. }
        ));
    }

    #[test]
    fn allowlists_refuse_misspellings_duplicates_and_changed_defaults() {
        let (d, reg, graph) = baseline();
        assert!(
            controls(
                &mut graph.clone(),
                Some(&[Control::Name("Typo".into())]),
                &reg
            )
            .is_err()
        );
        let name = graph.exposed[0].name.clone();
        assert!(
            controls(
                &mut graph.clone(),
                Some(&[Control::Name(name.clone()), Control::Name(name)]),
                &reg
            )
            .is_err()
        );
        let width = graph
            .exposed
            .iter()
            .find(|e| e.input == "width_mm")
            .unwrap();
        assert!(
            controls(
                &mut graph.clone(),
                Some(&[Control::Bounded {
                    name: width.name.clone(),
                    min: d.profile.width_mm + 1.0,
                    max: d.profile.width_mm + 2.0
                }]),
                &reg
            )
            .is_err()
        );
    }

    #[test]
    fn authored_expression_graph_opens_with_the_hosts_script_engine() {
        let (d, reg, mut graph) = baseline();
        let profile = graph
            .nodes
            .iter()
            .find(|n| n.kind == "band.profile")
            .unwrap()
            .id;
        graph
            .set_input(
                profile,
                "width_mm",
                Literal::expr(format!("{} + 0.0", d.profile.width_mm)),
            )
            .unwrap();
        let graph =
            file::load_graph_str(&file::graph_to_string(&graph).unwrap(), Some(&reg)).unwrap();
        let (got, report) = eval::design_of(
            &mut Evaluator::with_exprs(ringdesign_script::engine()),
            &graph,
            &reg,
            &AlphaLibrary::default(),
            0,
        )
        .unwrap();
        assert!(report.notes(&graph).is_empty());
        assert!(source_equal(&d, &got).unwrap());
    }

    #[test]
    fn authored_script_graph_is_preserved_by_the_whole_packaging_path() {
        let (mut d, reg, _) = baseline();
        d.build.theta_steps = 32;
        d.build.profile_steps = 16;
        let mut graph = lift::from_design(&d, &reg, &AlphaLibrary::default()).unwrap();
        let profile = graph
            .nodes
            .iter()
            .find(|n| n.kind == "band.profile")
            .unwrap()
            .id;
        let script = graph.add("script").unwrap();
        graph.node_mut(script).unwrap().params = json!({"source": format!("// in: width: Number = {}\n// out: result: Number\nlet result = width;\n", d.profile.width_mm)});
        graph
            .connect(script, "result", profile, "width_mm")
            .unwrap();
        graph.exposed.clear();
        let temp = std::env::temp_dir().join(format!(
            "ringdesign-collection-script-{}",
            std::process::id()
        ));
        let source = temp.join("input/ring");
        std::fs::create_dir_all(&source).unwrap();
        library::save_design(source.join("design.ring.json"), &d).unwrap();
        std::fs::write(
            source.join("verification.json"),
            r#"{"clamp_bite_mm":0.38,"stone_count":8,"template_bytes":-1,"geometry_gates_passed":true,"gates_passed":false,"status":"template pending"}"#,
        )
        .unwrap();
        file::save_graph(source.join("authored.graph.json"), &graph).unwrap();
        let opts = Options {
            collection: "probe".into(),
            source: temp.join("input"),
            output: temp.join("output"),
            templates: temp.join("graphs"),
            check_bundled: false,
            verify_export: false,
            only: None,
        };
        let ring = Ring {
            slug: "ring".into(),
            authored_graph: Some("authored.graph.json".into()),
            exposed_controls: Some(vec![]),
            template_class: Some("procedural".into()),
        };
        let result = verify(&opts, &ring, &temp, &reg).unwrap();
        let saved =
            file::load_graph(opts.templates.join("ring-probe.graph.json"), Some(&reg)).unwrap();
        assert_eq!(saved, graph);
        assert_eq!(result["source_method"], "authored_file");
        assert_eq!(result["vertices_faces_normals_identical"], true);
        assert_eq!(result["clamp_bite_mm"], 0.38);
        assert_eq!(result["stone_count"], 8);
        assert_eq!(result["gates_passed"], true);
        assert!(result.get("status").is_none());
        assert!(result["template_bytes"].as_u64().unwrap() > 0);
        std::fs::remove_dir_all(temp).unwrap();
    }
}
