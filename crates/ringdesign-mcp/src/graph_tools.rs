//! The `graph_*` tools: the design's graph, edited and evaluated over MCP.
//!
//! The graph lives on the design (`RingDesign::graph`) in the shared
//! engine, exactly where the GUI keeps it, so every tool here reads it off
//! the design, edits it and stores it back through `set_design` — whose
//! generation bump is what a GUI sharing the engine polls for. Evaluation
//! goes through one evaluator with the script engine attached, so
//! expression pins and script nodes work over MCP as they do in the app.

use std::sync::{Mutex, OnceLock};

use ringdesign_core::RingDesign;
use ringdesign_graph::eval::{Evaluator, Targets, evaluate_design};
use ringdesign_graph::file;
use ringdesign_graph::graph::{Graph, Mode, NodeId};
use ringdesign_graph::nodes::cluster;
use ringdesign_graph::registry::{Category, Registry};
use ringdesign_graph::value::Literal;
use ringdesign_graph::{lift, templates};
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::RingDesignServer;

fn registry() -> &'static Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(ringdesign_script::registry)
}

fn evaluator() -> &'static Mutex<Evaluator> {
    static EV: OnceLock<Mutex<Evaluator>> = OnceLock::new();
    EV.get_or_init(|| Mutex::new(Evaluator::with_exprs(ringdesign_script::engine())))
}

fn bad(msg: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(msg.into(), None)
}

fn graph_of(d: &RingDesign) -> Result<Graph, ErrorData> {
    let Some(json) = &d.graph else {
        return Err(bad("this design has no graph yet: call graph_new, graph_from_design or graph_open_template"));
    };
    serde_json::from_value(json.clone()).map_err(|e| bad(format!("the design's graph does not parse: {e}")))
}

fn store(e: &mut ringdesign_core::DesignEngine, g: &Graph) -> Result<(), ErrorData> {
    let mut d = e.design().clone();
    d.graph = Some(serde_json::to_value(g).map_err(|e| bad(e.to_string()))?);
    e.set_design(d);
    Ok(())
}

fn parse_mode(s: Option<&str>) -> Result<Mode, ErrorData> {
    match s.map(|m| m.trim().to_lowercase()).as_deref() {
        None | Some("") | Some("sandring") | Some("sand_ring") | Some("sand") => Ok(Mode::SandRing),
        Some("free") => Ok(Mode::Free),
        Some(other) => Err(bad(format!("{other:?} is not a mode; use SandRing or Free"))),
    }
}

fn literal_of(v: serde_json::Value) -> Result<Literal, ErrorData> {
    serde_json::from_value(v).map_err(|e| bad(format!("not a literal: {e}")))
}

fn lit_json(l: &Literal) -> serde_json::Value {
    serde_json::to_value(l).unwrap_or(serde_json::Value::Null)
}

// --- Results ---------------------------------------------------------------

#[derive(Debug, Serialize, JsonSchema)]
pub struct GraphChange {
    pub generation: u64,
    pub nodes: usize,
    pub wires: usize,
    pub applied: Vec<String>,
    /// Validation errors, if any; an evaluable graph has none.
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PinDesc {
    pub name: String,
    pub kind: String,
    pub access: String,
    /// `#id.out` feeding this input, if wired.
    pub wired_from: Option<String>,
    /// The literal on this input, if set.
    pub literal: Option<serde_json::Value>,
    pub doc: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NodeDesc {
    pub id: u64,
    pub kind: String,
    pub label: Option<String>,
    pub pos: [f32; 2],
    pub inputs: Vec<PinDesc>,
    pub outputs: Vec<PinDesc>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GraphDescription {
    pub name: String,
    pub mode: String,
    pub nodes: Vec<NodeDesc>,
    pub wires: Vec<String>,
    pub exposed: Vec<String>,
    pub outputs: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct KindDesc {
    pub key: String,
    pub label: String,
    pub category: String,
    pub doc: String,
    pub modes: Vec<String>,
    pub side_effect: bool,
    pub inputs: Vec<PinDesc>,
    pub outputs: Vec<PinDesc>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct KindList {
    pub count: usize,
    pub kinds: Vec<KindDesc>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NodeStatusDesc {
    pub id: u64,
    pub kind: String,
    pub items: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub cached: bool,
    pub skipped: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GraphEvalResult {
    pub ok: bool,
    pub generation: u64,
    pub verdict: Option<String>,
    pub undercut_pct: Option<f64>,
    pub worst_draft_deg: Option<f64>,
    pub thinnest_wall_mm: Option<f64>,
    pub notes: Vec<String>,
    pub nodes: Vec<NodeStatusDesc>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ClusterDesc {
    pub name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PresetDesc {
    pub name: String,
    pub cluster: String,
    pub values: serde_json::Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ClusterList {
    pub clusters: Vec<ClusterDesc>,
    pub presets: Vec<PresetDesc>,
    pub templates: Vec<String>,
}

// --- Params ----------------------------------------------------------------

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct GraphNewParams {
    /// The graph's name; the design's name if unset.
    pub name: Option<String>,
    /// SandRing (default) or Free.
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphPathParams {
    /// Absolute path of a .graph.json file.
    pub path: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct GraphListNodesParams {
    /// One of Sources, Band, Shank, Layers, Generators, Alphas, Assembly, Sinks, Utilities, Solids.
    pub category: Option<String>,
    /// Substring of the key or label.
    pub query: Option<String>,
    /// SandRing (default) or Free: the mode the kinds must run in.
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphAddNodeParams {
    /// The registry key, e.g. band.profile (see graph_list_nodes).
    pub kind: String,
    pub label: Option<String>,
    /// Editor position.
    pub pos: Option<[f32; 2]>,
    /// Literals for inputs, as an object of pin name to value.
    pub inputs: Option<serde_json::Value>,
    /// Node params (a script's source, for instance).
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphNodeParams {
    pub id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphConnectParams {
    pub from: u64,
    pub out: String,
    pub to: u64,
    pub input: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphDisconnectParams {
    pub to: u64,
    pub input: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphSetParamParams {
    pub id: u64,
    /// An RFC 6901 pointer into the node's params, e.g. /source.
    pub pointer: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphSetInputParams {
    pub id: u64,
    pub input: String,
    /// The literal (number, integer, boolean, text, list, or JSON). Omit both value and expr to clear.
    pub value: Option<serde_json::Value>,
    /// An expression instead of a value, evaluated per item with the node's other inputs in scope.
    pub expr: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphExposeParams {
    pub id: u64,
    pub input: String,
    /// The name on the graph's parameter panel.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphPresetParams {
    /// The preset's name (see graph_list_clusters).
    pub name: String,
    /// The cluster node to set.
    pub node: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphClusterParams {
    /// The cluster's name (see graph_list_clusters).
    pub name: String,
    pub pos: Option<[f32; 2]>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphTemplateParams {
    /// "Simple", or a template name such as "Court band".
    pub name: String,
}

// --- Descriptions ------------------------------------------------------------

fn describe(g: &Graph, reg: &Registry) -> GraphDescription {
    let nodes = g
        .nodes
        .iter()
        .map(|n| {
            let (ins, outs) = reg.node_pins(n).unwrap_or_default();
            NodeDesc {
                id: n.id.0,
                kind: n.kind.clone(),
                label: n.label.clone(),
                pos: n.pos,
                inputs: ins
                    .iter()
                    .map(|p| PinDesc {
                        name: p.name.clone(),
                        kind: p.kind.label().into(),
                        access: format!("{:?}", p.access).to_lowercase(),
                        wired_from: g.wire_into(n.id, &p.name).map(|w| format!("{}.{}", w.from, w.out)),
                        literal: n.inputs.get(&p.name).map(lit_json),
                        doc: p.doc.clone(),
                    })
                    .collect(),
                outputs: outs
                    .iter()
                    .map(|p| PinDesc { name: p.name.clone(), kind: p.kind.label().into(), access: format!("{:?}", p.access).to_lowercase(), wired_from: None, literal: None, doc: p.doc.clone() })
                    .collect(),
            }
        })
        .collect();
    GraphDescription {
        name: g.name.clone(),
        mode: format!("{:?}", g.mode),
        nodes,
        wires: g.wires.iter().map(|w| format!("{}.{} -> {}.{}", w.from, w.out, w.to, w.input)).collect(),
        exposed: g.exposed.iter().map(|e| format!("{} = {}.{}", e.name, e.node, e.input)).collect(),
        outputs: g.outputs.iter().map(|e| format!("{} = {}.{}", e.name, e.node, e.out)).collect(),
        errors: g.validate(Some(reg)).iter().map(ToString::to_string).collect(),
    }
}

fn kinds(reg: &Registry, mode: Mode, category: Option<Category>, query: &str) -> KindList {
    let q = query.to_lowercase();
    let kinds: Vec<KindDesc> = reg
        .list(mode)
        .into_iter()
        .filter(|s| category.is_none_or(|c| s.category == c))
        .filter(|s| q.is_empty() || s.key.to_lowercase().contains(&q) || s.label.to_lowercase().contains(&q))
        .map(|s| KindDesc {
            key: s.key.clone(),
            label: s.label.clone(),
            category: s.category.label().into(),
            doc: s.doc.clone(),
            modes: s.modes.iter().map(|m| format!("{m:?}")).collect(),
            side_effect: s.side_effect,
            inputs: s
                .inputs
                .iter()
                .map(|p| PinDesc { name: p.name.clone(), kind: p.kind.label().into(), access: format!("{:?}", p.access).to_lowercase(), wired_from: None, literal: p.default.as_ref().map(lit_json), doc: p.doc.clone() })
                .collect(),
            outputs: s
                .outputs
                .iter()
                .map(|p| PinDesc { name: p.name.clone(), kind: p.kind.label().into(), access: format!("{:?}", p.access).to_lowercase(), wired_from: None, literal: None, doc: p.doc.clone() })
                .collect(),
        })
        .collect();
    KindList { count: kinds.len(), kinds }
}

fn parse_category(s: &str) -> Result<Category, ErrorData> {
    Category::ALL
        .iter()
        .copied()
        .find(|c| c.label().eq_ignore_ascii_case(s.trim()) || format!("{c:?}").eq_ignore_ascii_case(s.trim()))
        .ok_or_else(|| bad(format!("{s:?} is not a category; one of {:?}", Category::ALL.iter().map(|c| c.label()).collect::<Vec<_>>())))
}

impl RingDesignServer {
    fn change(&self, e: &ringdesign_core::DesignEngine, g: &Graph, applied: Vec<String>) -> GraphChange {
        self.touch();
        GraphChange {
            generation: e.generation(),
            nodes: g.nodes.len(),
            wires: g.wires.len(),
            applied,
            errors: g.validate(Some(registry())).iter().map(ToString::to_string).collect(),
        }
    }

    /// Edit the design's graph in place and store it back.
    fn edit(&self, f: impl FnOnce(&mut Graph) -> Result<Vec<String>, ErrorData>) -> Result<Json<GraphChange>, ErrorData> {
        let mut e = self.engine.lock();
        let mut g = graph_of(e.design())?;
        let applied = f(&mut g)?;
        store(&mut e, &g)?;
        Ok(Json(self.change(&e, &g, applied)))
    }

    /// Replace the design's graph outright.
    fn adopt(&self, g: Graph, applied: Vec<String>) -> Result<Json<GraphChange>, ErrorData> {
        let mut e = self.engine.lock();
        store(&mut e, &g)?;
        Ok(Json(self.change(&e, &g, applied)))
    }

    pub(crate) fn graph_value(&self) -> serde_json::Value {
        let e = self.engine.lock();
        serde_json::json!({ "generation": e.generation(), "graph": e.design().graph })
    }

    pub(crate) fn node_kinds_value(&self) -> serde_json::Value {
        serde_json::to_value(kinds(registry(), Mode::Free, None, "")).unwrap_or(serde_json::Value::Null)
    }
}

#[tool_router(router = graph_router, vis = "pub")]
impl RingDesignServer {
    #[tool(
        description = "Start an empty graph behind the current design (SandRing mode unless told Free), replacing any graph it had. A graph is a dataflow document: nodes are the core's own operations, wires carry their values, and graph_evaluate runs it to produce the design with its castability verdict. Typical shape: band.profile -> design.new -> (layer nodes -> entry -> stack -> design.assemble) -> sink.output. Read ring://graph/nodes or call graph_list_nodes for the node library."
    )]
    async fn graph_new(&self, Parameters(p): Parameters<GraphNewParams>) -> Result<Json<GraphChange>, ErrorData> {
        let mode = parse_mode(p.mode.as_deref())?;
        let name = p.name.unwrap_or_else(|| self.engine.lock().design().name.clone());
        self.adopt(Graph::new(name, mode), vec![format!("mode={mode:?}")])
    }

    #[tool(
        description = "Lift the current design into a graph that evaluates back to it exactly: the nodes a person would wire (section, shank, heads, one node per layer with its gating, alpha sources, stack, assembly, output) plus design.set patches for whatever the nodes cannot express. The design is unchanged; from here, edits go through the graph."
    )]
    async fn graph_from_design(&self) -> Result<Json<GraphChange>, ErrorData> {
        let (g, lifted) = {
            let e = self.engine.lock();
            let g = lift::from_design(e.design(), registry(), &e.library_arc()).map_err(|e| bad(e.to_string()))?;
            let n = g.nodes.len();
            (g, n)
        };
        self.adopt(g, vec![format!("lifted {lifted} nodes")])
    }

    #[tool(description = "Put one of the bundled graphs behind the design: \"Simple\" (size, section, shank, an empty stack, the output — with the design panel's knobs exposed) or a template name such as \"Court band\", \"Heart signet\", \"Braided band\". Evaluate it with graph_evaluate.")]
    async fn graph_open_template(&self, Parameters(p): Parameters<GraphTemplateParams>) -> Result<Json<GraphChange>, ErrorData> {
        let g = if p.name.eq_ignore_ascii_case("simple") {
            templates::simple()
        } else {
            templates::graph(&p.name).ok_or_else(|| bad(format!("no template graph {:?}; one of Simple, {}", p.name, templates::BUNDLED.iter().map(|t| t.name).collect::<Vec<_>>().join(", "))))?
        };
        self.adopt(g, vec![format!("template={}", p.name)])
    }

    #[tool(description = "Drop the design's graph. The design stays exactly as last evaluated and the panels edit it directly again.")]
    async fn graph_bake(&self) -> Result<Json<GraphChange>, ErrorData> {
        let mut e = self.engine.lock();
        let mut d = e.design().clone();
        let had = d.graph.take().is_some();
        e.set_design(d);
        self.touch();
        Ok(Json(GraphChange { generation: e.generation(), nodes: 0, wires: 0, applied: vec![if had { "baked".into() } else { "no graph to bake".into() }], errors: Vec::new() }))
    }

    #[tool(description = "Load a .graph.json file as the design's graph (walking the file's version ladder).")]
    async fn graph_load(&self, Parameters(p): Parameters<GraphPathParams>) -> Result<Json<GraphChange>, ErrorData> {
        let g = file::load_graph(&p.path, Some(registry())).map_err(|e| bad(format!("{}: {e:#}", p.path)))?;
        self.adopt(g, vec![format!("loaded {}", p.path)])
    }

    #[tool(description = "Save the design's graph to a .graph.json file with its format version.")]
    async fn graph_save(&self, Parameters(p): Parameters<GraphPathParams>) -> Result<Json<GraphChange>, ErrorData> {
        let e = self.engine.lock();
        let g = graph_of(e.design())?;
        file::save_graph(&p.path, &g).map_err(|er| bad(format!("{}: {er:#}", p.path)))?;
        Ok(Json(GraphChange { generation: e.generation(), nodes: g.nodes.len(), wires: g.wires.len(), applied: vec![format!("saved {}", p.path)], errors: Vec::new() }))
    }

    #[tool(description = "The design's graph in full: every node with its pins (kind, what feeds it, its literal), the wires, the exposed parameters and outputs, and validation errors. Node ids are stable; pins are named.")]
    async fn graph_describe(&self) -> Result<Json<GraphDescription>, ErrorData> {
        let e = self.engine.lock();
        let g = graph_of(e.design())?;
        Ok(Json(describe(&g, registry())))
    }

    #[tool(description = "The node library: every kind the runtime knows with its category, doc, pins (kind, item or list access, default) and outputs. Filter by category (Sources, Band, Shank, Layers, Generators, Alphas, Assembly, Sinks, Utilities, Solids), a substring, and the mode it must run in.")]
    async fn graph_list_nodes(&self, Parameters(p): Parameters<GraphListNodesParams>) -> Result<Json<KindList>, ErrorData> {
        let mode = parse_mode(p.mode.as_deref())?;
        let category = p.category.as_deref().map(parse_category).transpose()?;
        Ok(Json(kinds(registry(), mode, category, p.query.as_deref().unwrap_or(""))))
    }

    #[tool(description = "Add a node of a kind to the graph, optionally with a label, an editor position, literals on its inputs (an object of pin name to value) and params (a script's source under /source, for instance). Returns the new node's id in `applied` as node=<id>.")]
    async fn graph_add_node(&self, Parameters(p): Parameters<GraphAddNodeParams>) -> Result<Json<GraphChange>, ErrorData> {
        let reg = registry();
        let spec = reg.get(&p.kind).ok_or_else(|| bad(format!("no node kind {:?}; see graph_list_nodes", p.kind)))?;
        let key = spec.key.clone();
        self.edit(move |g| {
            let id = g.add(&key).map_err(|e| bad(e.to_string()))?;
            let mut applied = vec![format!("node={}", id.0)];
            if let Some(node) = g.node_mut(id) {
                node.label = p.label.clone().filter(|l| !l.trim().is_empty());
                if let Some(pos) = p.pos {
                    node.pos = pos;
                }
                if let Some(params) = p.params.clone() {
                    node.params = params;
                }
            }
            if let Some(serde_json::Value::Object(map)) = p.inputs.clone() {
                for (k, v) in map {
                    g.set_input(id, k.clone(), literal_of(v)?).map_err(|e| bad(e.to_string()))?;
                    applied.push(format!("{k} set"));
                }
            }
            Ok(applied)
        })
    }

    #[tool(description = "Remove a node and every wire and exposure that names it.")]
    async fn graph_remove_node(&self, Parameters(p): Parameters<GraphNodeParams>) -> Result<Json<GraphChange>, ErrorData> {
        self.edit(move |g| {
            g.remove(NodeId(p.id)).map_err(|e| bad(e.to_string()))?;
            Ok(vec![format!("removed {}", p.id)])
        })
    }

    #[tool(description = "Wire an output into an input by name (from.out -> to.input). An input carries one wire: an existing one is replaced. Refuses self-wires and cycles; kinds are checked by graph_describe's errors and at evaluation.")]
    async fn graph_connect(&self, Parameters(p): Parameters<GraphConnectParams>) -> Result<Json<GraphChange>, ErrorData> {
        self.edit(move |g| {
            let displaced = g.connect(NodeId(p.from), p.out.clone(), NodeId(p.to), p.input.clone()).map_err(|e| bad(e.to_string()))?;
            let mut applied = vec![format!("{}.{} -> {}.{}", p.from, p.out, p.to, p.input)];
            if let Some(w) = displaced {
                applied.push(format!("displaced {}.{}", w.from, w.out));
            }
            Ok(applied)
        })
    }

    #[tool(description = "Drop the wire into an input; the input's literal, if any, takes over.")]
    async fn graph_disconnect(&self, Parameters(p): Parameters<GraphDisconnectParams>) -> Result<Json<GraphChange>, ErrorData> {
        self.edit(move |g| {
            let had = g.disconnect(NodeId(p.to), &p.input).is_some();
            Ok(vec![if had { format!("disconnected {}.{}", p.to, p.input) } else { format!("{}.{} had no wire", p.to, p.input) }])
        })
    }

    #[tool(description = "Set a literal on an input (a number, integer, boolean, text, list or JSON), or an expression with `expr` (evaluated per item with the node's other inputs in scope, plus i and n), or clear the input by giving neither. A wire on the input shadows its literal.")]
    async fn graph_set_input(&self, Parameters(p): Parameters<GraphSetInputParams>) -> Result<Json<GraphChange>, ErrorData> {
        self.edit(move |g| {
            let id = NodeId(p.id);
            if !g.contains(id) {
                return Err(bad(format!("no node {}", p.id)));
            }
            let applied = match (p.value.clone(), p.expr.clone()) {
                (_, Some(code)) => {
                    g.set_input(id, p.input.clone(), Literal::expr(code.clone())).map_err(|e| bad(e.to_string()))?;
                    format!("{}.{} = expr {code:?}", p.id, p.input)
                }
                (Some(v), None) => {
                    g.set_input(id, p.input.clone(), literal_of(v)?).map_err(|e| bad(e.to_string()))?;
                    format!("{}.{} set", p.id, p.input)
                }
                (None, None) => {
                    g.clear_input(id, &p.input).map_err(|e| bad(e.to_string()))?;
                    format!("{}.{} cleared", p.id, p.input)
                }
            };
            Ok(vec![applied])
        })
    }

    #[tool(description = "Set a value inside a node's params at an RFC 6901 pointer (creating objects on the way): a script node's /source, a cluster's embedded graph.")]
    async fn graph_set_param(&self, Parameters(p): Parameters<GraphSetParamParams>) -> Result<Json<GraphChange>, ErrorData> {
        self.edit(move |g| {
            g.set_param(NodeId(p.id), &p.pointer, p.value.clone()).map_err(|e| bad(e.to_string()))?;
            Ok(vec![format!("{} {} set", p.id, p.pointer)])
        })
    }

    #[tool(description = "Promote an input to the graph's parameter panel under a name — what a cluster shows as a pin and a preset sets.")]
    async fn graph_expose(&self, Parameters(p): Parameters<GraphExposeParams>) -> Result<Json<GraphChange>, ErrorData> {
        self.edit(move |g| {
            g.expose(NodeId(p.id), p.input.clone(), p.name.clone()).map_err(|e| bad(e.to_string()))?;
            Ok(vec![format!("exposed {}.{} as {:?}", p.id, p.input, p.name)])
        })
    }

    #[tool(description = "The user's saved clusters (graphs used as nodes) and presets (named values for a cluster's exposed inputs), and the bundled template graphs.")]
    async fn graph_list_clusters(&self) -> Result<Json<ClusterList>, ErrorData> {
        let reg = registry();
        let clusters = file::list_clusters(Some(reg))
            .into_iter()
            .map(|c| ClusterDesc { name: c.name.clone(), inputs: c.exposed.iter().map(|e| e.name.clone()).collect(), outputs: c.outputs.iter().map(|o| o.name.clone()).collect() })
            .collect();
        let presets = file::list_presets().into_iter().map(|p| PresetDesc { name: p.name, cluster: p.cluster, values: serde_json::to_value(&p.values).unwrap_or_default() }).collect();
        Ok(Json(ClusterList { clusters, presets, templates: templates::BUNDLED.iter().map(|t| t.name.to_string()).collect() }))
    }

    #[tool(description = "Add a saved cluster as a node; its exposed inputs and outputs are the pins, and the cluster's graph rides in the node.")]
    async fn graph_add_cluster(&self, Parameters(p): Parameters<GraphClusterParams>) -> Result<Json<GraphChange>, ErrorData> {
        let c = file::load_cluster(&p.name, Some(registry())).ok_or_else(|| bad(format!("no saved cluster {:?}; see graph_list_clusters", p.name)))?;
        self.edit(move |g| {
            let id = cluster::add_cluster(g, &c).map_err(|e| bad(e.to_string()))?;
            if let (Some(pos), Some(node)) = (p.pos, g.node_mut(id)) {
                node.pos = pos;
            }
            Ok(vec![format!("node={}", id.0), format!("cluster={}", c.name)])
        })
    }

    #[tool(description = "Set a preset's values on a cluster node's exposed inputs; names the preset had no pin for come back in `applied`.")]
    async fn graph_apply_preset(&self, Parameters(p): Parameters<GraphPresetParams>) -> Result<Json<GraphChange>, ErrorData> {
        let preset = file::list_presets().into_iter().find(|x| x.name == p.name).ok_or_else(|| bad(format!("no preset {:?}", p.name)))?;
        self.edit(move |g| {
            let node = g.node_mut(NodeId(p.node)).ok_or_else(|| bad(format!("no node {}", p.node)))?;
            let unknown = preset.apply(node, registry());
            let mut applied = vec![format!("preset={}", preset.name)];
            if !unknown.is_empty() {
                applied.push(format!("no pin for {unknown:?}"));
            }
            Ok(applied)
        })
    }

    #[tool(
        description = "Evaluate the design's graph. On success the engine's design becomes what the graph produced (its graph kept) and the result carries the field verdict — undercut share, worst draft, thinnest wall, notes — plus every node's status (items run, errors, warnings, cached). On a validation failure nothing changes and the errors are listed. Side-effect sinks (export, save, render) do not run here."
    )]
    async fn graph_evaluate(&self) -> Result<Json<GraphEvalResult>, ErrorData> {
        let mut e = self.engine.lock();
        let g = graph_of(e.design())?;
        let lib = e.library_arc();
        let epoch = std::sync::Arc::as_ptr(&lib) as usize as u64;
        let reg = registry();
        let mut ev = evaluator().lock().map_err(|_| bad("the evaluator is poisoned"))?;
        let nodes_of = |g: &Graph, report: &ringdesign_graph::eval::EvalReport| -> Vec<NodeStatusDesc> {
            report
                .order
                .iter()
                .filter_map(|id| {
                    let s = report.status.get(id)?;
                    Some(NodeStatusDesc {
                        id: id.0,
                        kind: g.node(*id).map(|n| n.kind.clone()).unwrap_or_default(),
                        items: s.items,
                        errors: s.errors.iter().map(|(i, m)| if s.items > 1 { format!("item {i}: {m}") } else { m.clone() }).collect(),
                        warnings: s.warnings.clone(),
                        cached: s.cached,
                        skipped: s.skipped,
                    })
                })
                .collect()
        };
        match evaluate_design(&mut ev, &g, reg, &lib, epoch) {
            Ok(out) => {
                let mut d = (*out.design).clone();
                d.graph = e.design().graph.clone();
                e.set_design(d);
                self.touch();
                Ok(Json(GraphEvalResult {
                    ok: true,
                    generation: e.generation(),
                    verdict: Some(out.field.verdict.label().into()),
                    undercut_pct: Some(out.field.undercut_fraction() * 100.0),
                    worst_draft_deg: Some(out.field.worst_draft_deg),
                    thinnest_wall_mm: Some(out.field.thinnest_wall_mm),
                    notes: out.notes.iter().cloned().chain(out.field.notes.iter().cloned()).collect(),
                    nodes: nodes_of(&g, &out.report),
                }))
            }
            Err(err) => {
                let report = ev.evaluate(&g, reg, &lib, epoch, Targets::AllPure);
                let mut notes = vec![err.to_string()];
                notes.extend(report.errors.iter().map(ToString::to_string));
                Ok(Json(GraphEvalResult { ok: false, generation: e.generation(), verdict: None, undercut_pct: None, worst_draft_deg: None, thinnest_wall_mm: None, notes, nodes: nodes_of(&g, &report) }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::DesignEngine;
    use ringdesign_core::alpha::AlphaLibrary;

    fn server() -> RingDesignServer {
        RingDesignServer::new(DesignEngine::shared(AlphaLibrary::builtin()))
    }

    fn node_id(c: &GraphChange) -> u64 {
        c.applied.iter().find_map(|a| a.strip_prefix("node=")).and_then(|s| s.parse().ok()).expect("node id")
    }

    #[test]
    fn the_graph_router_lists_its_tools_with_object_schemas() {
        let tools = RingDesignServer::graph_router().list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        for expected in [
            "graph_new", "graph_from_design", "graph_open_template", "graph_bake", "graph_load", "graph_save", "graph_describe", "graph_list_nodes",
            "graph_add_node", "graph_remove_node", "graph_connect", "graph_disconnect", "graph_set_input", "graph_set_param", "graph_expose",
            "graph_list_clusters", "graph_add_cluster", "graph_apply_preset", "graph_evaluate",
        ] {
            assert!(names.contains(&expected), "{expected} missing from {names:?}");
        }
        for t in &tools {
            assert_eq!(t.input_schema.get("type").and_then(|v| v.as_str()), Some("object"), "{}", t.name);
        }
        let all = RingDesignServer::tool_router() + RingDesignServer::graph_router();
        assert!(all.list_all().len() > 40);
    }

    #[tokio::test]
    async fn the_court_band_built_by_tools_equals_the_template_byte_for_byte() {
        let s = server();
        let gen0 = s.engine.lock().generation();
        let c = s.graph_new(Parameters(GraphNewParams { name: Some("Court band".into()), mode: None })).await.unwrap().0;
        assert_eq!(c.nodes, 0);
        assert!(s.engine.lock().generation() > gen0, "a graph edit moves the generation the GUI polls");
        let p = node_id(&s.graph_add_node(Parameters(GraphAddNodeParams { kind: "band.profile".into(), label: None, pos: None, inputs: Some(serde_json::json!({"style": "LowDome", "width_mm": 4.0, "thickness_mm": 2.0})), params: None })).await.unwrap().0);
        let d = node_id(&s.graph_add_node(Parameters(GraphAddNodeParams { kind: "design.new".into(), label: None, pos: None, inputs: Some(serde_json::json!({"name": "Court band"})), params: None })).await.unwrap().0);
        let o = node_id(&s.graph_add_node(Parameters(GraphAddNodeParams { kind: "sink.output".into(), label: None, pos: None, inputs: None, params: None })).await.unwrap().0);
        s.graph_connect(Parameters(GraphConnectParams { from: p, out: "profile".into(), to: d, input: "profile".into() })).await.unwrap();
        let c = s.graph_connect(Parameters(GraphConnectParams { from: d, out: "design".into(), to: o, input: "design".into() })).await.unwrap().0;
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        let desc = s.graph_describe().await.unwrap().0;
        assert_eq!(desc.nodes.len(), 3);
        assert!(desc.nodes[0].inputs.iter().any(|pin| pin.name == "width_mm" && pin.literal == Some(serde_json::json!(4.0))));
        assert_eq!(desc.wires.len(), 2);
        let r = s.graph_evaluate().await.unwrap().0;
        assert!(r.ok, "{:?}", r.notes);
        assert_eq!(r.verdict.as_deref(), Some("Castable"));
        let got = {
            let e = s.engine.lock();
            let mut d = e.design().clone();
            assert!(d.graph.is_some(), "the graph stays on the design");
            d.graph = None;
            serde_json::to_string(&d).unwrap()
        };
        let want = serde_json::to_string(&ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()).unwrap();
        assert_eq!(got, want);

        // An expression pin through MCP, and a refused wire.
        s.graph_set_input(Parameters(GraphSetInputParams { id: p, input: "thickness_mm".into(), value: None, expr: Some("width_mm / 2.0".into()) })).await.unwrap();
        let r = s.graph_evaluate().await.unwrap().0;
        assert!(r.ok, "{:?}", r.notes);
        assert_eq!(s.engine.lock().design().profile.thickness_mm, 2.0);
        let err = s.graph_connect(Parameters(GraphConnectParams { from: o, out: "design".into(), to: p, input: "profile".into() })).await.err();
        assert!(err.is_some(), "a cycle is refused");
        // Bake leaves the evaluated design and drops the graph.
        let c = s.graph_bake().await.unwrap().0;
        assert_eq!(c.applied, vec!["baked"]);
        assert!(s.engine.lock().design().graph.is_none());
        assert!(s.graph_describe().await.is_err(), "no graph to describe");
        // A template opens and lifts back.
        s.graph_open_template(Parameters(GraphTemplateParams { name: "Braided band".into() })).await.unwrap();
        let r = s.graph_evaluate().await.unwrap().0;
        assert!(r.ok && r.nodes.len() >= 10, "{:?}", r.notes);
        let c = s.graph_from_design().await.unwrap().0;
        assert!(c.nodes >= 10);
        let kinds = s.graph_list_nodes(Parameters(GraphListNodesParams { category: Some("Layers".into()), query: None, mode: None })).await.unwrap().0;
        assert!(kinds.kinds.iter().any(|k| k.key == "layer.milgrain"));
        assert!(s.graph_list_nodes(Parameters(GraphListNodesParams { category: Some("Nope".into()), query: None, mode: None })).await.is_err());
    }
}
