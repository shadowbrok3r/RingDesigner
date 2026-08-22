//! Evaluation with implicit lists, attributed errors and a signature cache.
//!
//! One pass in topological order. For each node the inputs are resolved —
//! the wire's value, else the node's literal, else the pin's default — and
//! the node runs once per item of the longest list on its `Item` pins,
//! shorter lists repeating their last item. A `List` pin gets the whole
//! list. An item that fails becomes a `Null` on every output with the error
//! recorded against that item; the other items run. The outputs of a node
//! that ran in list context are lists, so the next node sees items again.
//!
//! Every node carries a *recipe signature*: a hash of its kind, params,
//! literals, the signatures of what feeds it, the mode and the alpha
//! library's epoch. [`Evaluator`] keeps the last outputs per node and
//! reuses them when the signature has not moved, so one edit re-runs the
//! chain below it and nothing else.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ringdesign_core::castability::{FieldReport, attributed_field_report};
use ringdesign_core::{AlphaLibrary, RingDesign};

use crate::MAX_LIST_ITEMS;
use crate::graph::{Access, Graph, GraphError, NodeId};
use crate::registry::{EvalCtx, Inputs, NodeSpec, PinSpec, Registry};
use crate::value::{Literal, Value, ValueKind};

/// The kind of the node whose `design` input is what a SandRing graph is
/// for. Registered by the sink library; named here so the evaluator can
/// find it.
pub const OUTPUT_KIND: &str = "sink.output";
/// The input on that node that carries the design.
pub const OUTPUT_DESIGN_PIN: &str = "design";
/// Errors kept per node; the count keeps going.
pub const MAX_RECORDED_ERRORS: usize = 16;
/// Sampling the field verdict reads at: the GUI worker's numbers.
pub const FIELD_THETA_STEPS: usize = 192;
pub const FIELD_PROFILE_STEPS: usize = 128;

/// What an expression on a pin sees: the node's other inputs by name,
/// and where in the implicit list it is.
#[derive(Clone, Debug, Default)]
pub struct ExprScope {
    pub siblings: BTreeMap<String, Value>,
    pub item: usize,
    pub items: usize,
}

/// Runs `{"expr": …}` pins. The graph crate only defines the hook; the
/// script crate implements it, so the runtime stays free of any language.
pub trait ExprEvaluator: Send + Sync {
    fn eval_expr(&self, code: &str, scope: &ExprScope) -> Result<Value, String>;
}

/// Which nodes an evaluation runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Targets {
    /// The output sink and everything above it; every pure node if there
    /// is no sink.
    Design,
    /// One node and everything above it.
    Node(NodeId),
    /// These nodes and everything above them.
    Nodes(Vec<NodeId>),
    /// Every node without side effects.
    AllPure,
    /// Every node, side effects included.
    Everything,
}

/// How one node's evaluation went.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeStatus {
    /// Items the node ran over (0 for an empty list in).
    pub items: usize,
    /// `(item, message)` for the first [`MAX_RECORDED_ERRORS`] failures.
    pub errors: Vec<(usize, String)>,
    pub error_count: usize,
    pub warnings: Vec<String>,
    pub micros: u64,
    /// The outputs came from the cache; the node did not run.
    pub cached: bool,
    /// A side-effect node the run did not ask for, or a node outside the
    /// targets.
    pub skipped: bool,
}

impl NodeStatus {
    pub fn failed(&self) -> bool {
        self.error_count > 0
    }
}

/// Everything an evaluation produced besides the values themselves.
#[derive(Clone, Debug, Default)]
pub struct EvalReport {
    /// The order nodes were visited.
    pub order: Vec<NodeId>,
    pub status: BTreeMap<NodeId, NodeStatus>,
    /// Output values per node and pin, for badges and for reading results.
    pub values: BTreeMap<NodeId, BTreeMap<String, Value>>,
    /// Validation errors; when present nothing ran.
    pub errors: Vec<GraphError>,
}

impl EvalReport {
    pub fn value(&self, node: NodeId, out: &str) -> Option<&Value> {
        self.values.get(&node)?.get(out)
    }

    pub fn ran(&self) -> Vec<NodeId> {
        self.order.iter().copied().filter(|id| self.status.get(id).is_some_and(|s| !s.cached && !s.skipped)).collect()
    }

    pub fn cached(&self) -> Vec<NodeId> {
        self.order.iter().copied().filter(|id| self.status.get(id).is_some_and(|s| s.cached)).collect()
    }

    /// Whether any node failed on any item.
    pub fn any_failed(&self) -> bool {
        self.status.values().any(NodeStatus::failed)
    }

    /// Human lines: validation errors, then per-node failures and warnings.
    pub fn notes(&self, g: &Graph) -> Vec<String> {
        let mut notes: Vec<String> = self.errors.iter().map(ToString::to_string).collect();
        for id in &self.order {
            let Some(s) = self.status.get(id) else { continue };
            let kind = g.node(*id).map(|n| n.kind.as_str()).unwrap_or("?");
            for (item, msg) in &s.errors {
                notes.push(if s.items > 1 { format!("{id} {kind}: item {item}: {msg}") } else { format!("{id} {kind}: {msg}") });
            }
            if s.error_count > s.errors.len() {
                notes.push(format!("{id} {kind}: {} more items failed", s.error_count - s.errors.len()));
            }
            for w in &s.warnings {
                notes.push(format!("{id} {kind}: {w}"));
            }
        }
        notes
    }
}

struct CacheEntry {
    sig: u64,
    outputs: BTreeMap<String, Value>,
    status: NodeStatus,
}

/// Runs graphs and remembers what each node produced last time.
#[derive(Default)]
pub struct Evaluator {
    cache: HashMap<NodeId, CacheEntry>,
    /// How deep in clusters this evaluator runs; the root is 0.
    pub depth: usize,
    /// Runs expression pins; without one they fail with a clear line.
    pub exprs: Option<Arc<dyn ExprEvaluator>>,
}

impl Evaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_exprs(exprs: Arc<dyn ExprEvaluator>) -> Self {
        Self { exprs: Some(exprs), ..Self::default() }
    }

    /// Forget every cached output.
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    pub fn cached_nodes(&self) -> usize {
        self.cache.len()
    }

    /// Evaluate `targets` of `g`. `lib_epoch` changes whenever the alpha
    /// library's contents do, so nodes that read it re-run.
    pub fn evaluate(&mut self, g: &Graph, reg: &Registry, lib: &AlphaLibrary, lib_epoch: u64, targets: Targets) -> EvalReport {
        self.evaluate_injected(g, reg, lib, lib_epoch, targets, &BTreeMap::new())
    }

    /// [`Evaluator::evaluate`] with values pushed straight onto inputs —
    /// how a cluster's pins reach the nodes inside it, handles included.
    /// An injected input overrides its wire and its literal.
    pub fn evaluate_injected(
        &mut self,
        g: &Graph,
        reg: &Registry,
        lib: &AlphaLibrary,
        lib_epoch: u64,
        targets: Targets,
        injected: &BTreeMap<(NodeId, String), Value>,
    ) -> EvalReport {
        let mut report = EvalReport::default();
        report.errors = g.validate(Some(reg));
        if !report.errors.is_empty() {
            return report;
        }
        let order = match g.topo() {
            Ok(o) => o,
            Err(e) => {
                report.errors.push(e);
                return report;
            }
        };
        let live: BTreeSet<NodeId> = g.nodes.iter().map(|n| n.id).collect();
        self.cache.retain(|id, _| live.contains(id));

        let run_side_effects = targets == Targets::Everything;
        let wanted = self.target_set(g, reg, &order, &targets);

        let mut sigs: BTreeMap<NodeId, u64> = BTreeMap::new();
        for &id in &order {
            let node = g.node(id).expect("in order");
            let mut h = std::hash::DefaultHasher::new();
            node.kind.hash(&mut h);
            node.params.to_string().hash(&mut h);
            serde_json::to_string(&node.inputs).unwrap_or_default().hash(&mut h);
            g.mode.hash(&mut h);
            lib_epoch.hash(&mut h);
            self.depth.hash(&mut h);
            let mut wires: Vec<_> = g.wires_into(id).collect();
            wires.sort_by(|a, b| a.input.cmp(&b.input));
            for w in wires {
                w.input.hash(&mut h);
                w.out.hash(&mut h);
                sigs.get(&w.from).copied().unwrap_or(0).hash(&mut h);
            }
            for ((nid, pin), v) in injected {
                if *nid == id {
                    pin.hash(&mut h);
                    v.summary().hash(&mut h);
                    // A handle's identity, not its summary, is what moves.
                    (v as *const Value as usize).hash(&mut h);
                }
            }
            sigs.insert(id, h.finish());
        }

        for &id in &order {
            report.order.push(id);
            if !wanted.contains(&id) {
                report.status.insert(id, NodeStatus { skipped: true, ..Default::default() });
                continue;
            }
            let node = g.node(id).expect("in order");
            let spec = reg.get(&node.kind).expect("validated");
            let sig = sigs[&id];
            if spec.side_effect && !run_side_effects {
                let outputs: BTreeMap<String, Value> = spec.pins_for(node, reg).1.into_iter().map(|p| (p.name, Value::Null)).collect();
                report.values.insert(id, outputs);
                report.status.insert(id, NodeStatus { skipped: true, ..Default::default() });
                continue;
            }
            if !spec.side_effect {
                if let Some(entry) = self.cache.get(&id) {
                    if entry.sig == sig {
                        report.values.insert(id, entry.outputs.clone());
                        report.status.insert(id, NodeStatus { cached: true, micros: 0, ..entry.status.clone() });
                        continue;
                    }
                }
            }
            let (outputs, status) = run_node(g, reg, spec, node, &report.values, lib, lib_epoch, run_side_effects, self.depth, injected, self.exprs.as_ref());
            report.values.insert(id, outputs.clone());
            report.status.insert(id, status.clone());
            if !spec.side_effect {
                self.cache.insert(id, CacheEntry { sig, outputs, status });
            }
        }
        report
    }

    fn target_set(&self, g: &Graph, reg: &Registry, order: &[NodeId], targets: &Targets) -> BTreeSet<NodeId> {
        let pure = |id: &NodeId| g.node(*id).and_then(|n| reg.get(&n.kind)).is_some_and(|s| !s.side_effect);
        let closure = |roots: Vec<NodeId>| -> BTreeSet<NodeId> {
            let mut set: BTreeSet<NodeId> = roots.iter().copied().collect();
            for r in roots {
                set.extend(g.upstream(r));
            }
            set
        };
        match targets {
            Targets::Everything => order.iter().copied().collect(),
            Targets::AllPure => order.iter().copied().filter(pure).collect(),
            Targets::Node(id) => closure(vec![*id]),
            Targets::Nodes(ids) => closure(ids.clone()),
            Targets::Design => {
                let sinks: Vec<NodeId> = g.nodes.iter().filter(|n| n.kind == OUTPUT_KIND).map(|n| n.id).collect();
                if sinks.is_empty() { order.iter().copied().filter(pure).collect() } else { closure(sinks) }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn tick() -> Option<std::time::Instant> {
    Some(std::time::Instant::now())
}
#[cfg(target_arch = "wasm32")]
fn tick() -> Option<std::time::Instant> {
    None
}
fn elapsed_micros(t: Option<std::time::Instant>) -> u64 {
    t.map(|t| t.elapsed().as_micros() as u64).unwrap_or(0)
}

/// One resolved input: its items, whether it arrived as a list, and the
/// coercion failures per item.
struct Resolved {
    items: Vec<Value>,
    is_list: bool,
    access: Access,
    /// Items this node's own coercion failed; they are not run again.
    failed: Vec<bool>,
}

#[allow(clippy::too_many_arguments)]
fn run_node(
    g: &Graph,
    reg: &Registry,
    spec: &NodeSpec,
    node: &crate::graph::Node,
    values: &BTreeMap<NodeId, BTreeMap<String, Value>>,
    lib: &AlphaLibrary,
    lib_epoch: u64,
    run_side_effects: bool,
    depth: usize,
    injected: &BTreeMap<(NodeId, String), Value>,
    exprs: Option<&Arc<dyn ExprEvaluator>>,
) -> (BTreeMap<String, Value>, NodeStatus) {
    let start = tick();
    let mut status = NodeStatus::default();
    let (in_pins, out_pins) = spec.pins_for(node, reg);
    // Expression pins run per item once the other inputs are in scope.
    let mut expr_pins: Vec<(PinSpec, String)> = Vec::new();
    let record = |status: &mut NodeStatus, item: usize, msg: String| {
        status.error_count += 1;
        if status.errors.len() < MAX_RECORDED_ERRORS {
            status.errors.push((item, msg));
        }
    };

    let mut resolved: BTreeMap<String, Resolved> = BTreeMap::new();
    for pin in &in_pins {
        if injected.get(&(node.id, pin.name.clone())).is_none() && g.wire_into(node.id, &pin.name).is_none() {
            if let Some(code) = node.inputs.get(&pin.name).and_then(Literal::as_expr) {
                expr_pins.push((pin.clone(), code.to_string()));
                continue;
            }
        }
        let raw: Value = match injected.get(&(node.id, pin.name.clone())) {
            Some(v) => v.clone(),
            None => match g.wire_into(node.id, &pin.name) {
                Some(w) => values.get(&w.from).and_then(|o| o.get(&w.out)).cloned().unwrap_or(Value::Null),
                None => match node.inputs.get(&pin.name) {
                    Some(lit) => Value::from(lit.clone()),
                    None => pin.default.clone().map(Value::from).unwrap_or(Value::Null),
                },
            },
        };
        let (mut items, is_list) = match raw {
            Value::List(v) => (v, true),
            // A JSON array on a list pin is a list of JSON items.
            Value::Json(j) if pin.access == Access::List && j.is_array() => {
                (j.as_array().map(|a| a.iter().cloned().map(|x| Value::Json(Arc::new(x))).collect()).unwrap_or_default(), true)
            }
            Value::Null if pin.access == Access::List => (Vec::new(), true),
            other => (vec![other], false),
        };
        if items.len() > MAX_LIST_ITEMS {
            status.warnings.push(format!("{}: {} items, kept the first {MAX_LIST_ITEMS}", pin.name, items.len()));
            items.truncate(MAX_LIST_ITEMS);
        }
        let mut failed = vec![false; items.len()];
        for (j, item) in items.iter_mut().enumerate() {
            if item.is_null() {
                continue;
            }
            let v = std::mem::replace(item, Value::Null);
            match pin.kind.coerce(v) {
                Ok(v) => *item = v,
                Err(e) => {
                    failed[j] = true;
                    record(&mut status, j, format!("{}: {e}", pin.name));
                }
            }
        }
        resolved.insert(pin.name.clone(), Resolved { items, is_list, access: pin.access, failed });
    }

    let item_pins: Vec<&Resolved> = resolved.values().filter(|r| r.access == Access::Item).collect();
    let list_context = item_pins.iter().any(|r| r.is_list);
    let n = if item_pins.is_empty() {
        1
    } else if item_pins.iter().any(|r| r.items.is_empty()) {
        0
    } else {
        item_pins.iter().map(|r| r.items.len()).max().unwrap_or(1)
    };
    status.items = n;

    let mut columns: BTreeMap<String, Vec<Value>> = out_pins.iter().map(|p| (p.name.clone(), Vec::with_capacity(n))).collect();
    let mut ctx = EvalCtx::new(lib, reg, g.mode);
    ctx.run_side_effects = run_side_effects;
    ctx.depth = depth;
    ctx.lib_epoch = lib_epoch;
    ctx.exprs = exprs.cloned();
    ctx.items = n;
    for t in 0..n {
        ctx.item = t;
        let poisoned = resolved.values().any(|r| r.access == Access::Item && r.failed[t.min(r.failed.len() - 1)]);
        if poisoned {
            for p in &out_pins {
                columns.get_mut(&p.name).expect("declared").push(Value::Null);
            }
            continue;
        }
        let mut inputs = Inputs::default();
        for (name, r) in &resolved {
            let v = match r.access {
                Access::Item => r.items[t.min(r.items.len() - 1)].clone(),
                Access::List => Value::List(r.items.clone()),
            };
            inputs.values.insert(name.clone(), v);
        }
        let mut expr_failed = false;
        for (pin, code) in &expr_pins {
            let result = match exprs {
                Some(ev) => ev.eval_expr(code, &ExprScope { siblings: inputs.values.clone(), item: t, items: n }),
                None => Err("no expression engine is attached to this evaluation".to_string()),
            };
            match result.and_then(|v| pin.kind.coerce(v).map_err(|e| e.to_string())) {
                Ok(v) => {
                    inputs.values.insert(pin.name.clone(), v);
                }
                Err(e) => {
                    record(&mut status, t, format!("{}: expression: {e}", pin.name));
                    expr_failed = true;
                }
            }
        }
        if expr_failed {
            for p in &out_pins {
                columns.get_mut(&p.name).expect("declared").push(Value::Null);
            }
            continue;
        }
        match (spec.eval)(&mut ctx, node, &inputs) {
            Ok(outs) => {
                for p in &out_pins {
                    columns.get_mut(&p.name).expect("declared").push(outs.get(&p.name).cloned().unwrap_or(Value::Null));
                }
            }
            Err(e) => {
                record(&mut status, t, e.to_string());
                for p in &out_pins {
                    columns.get_mut(&p.name).expect("declared").push(Value::Null);
                }
            }
        }
    }
    for w in ctx.warnings.drain(..) {
        if status.warnings.len() < MAX_RECORDED_ERRORS && !status.warnings.contains(&w) {
            status.warnings.push(w);
        }
    }

    let outputs: BTreeMap<String, Value> = columns
        .into_iter()
        .map(|(name, col)| {
            let v = if list_context || n != 1 { Value::List(col) } else { col.into_iter().next().unwrap_or(Value::Null) };
            (name, v)
        })
        .collect();
    status.micros = elapsed_micros(start);
    (outputs, status)
}

/// A SandRing evaluation's answer: the design and the verdict it was
/// judged by, never one without the other.
#[derive(Clone, Debug)]
pub struct DesignOut {
    pub design: Arc<RingDesign>,
    pub field: FieldReport,
    /// Evaluation notes: per-item failures and warnings.
    pub notes: Vec<String>,
    pub report: EvalReport,
}

/// Evaluate the design a graph is for and judge it. The design is what
/// feeds the output sink's `design` input, or, without a sink, the last
/// single design any node produced.
pub fn evaluate_design(ev: &mut Evaluator, g: &Graph, reg: &Registry, lib: &AlphaLibrary, lib_epoch: u64) -> Result<DesignOut, GraphError> {
    let report = ev.evaluate(g, reg, lib, lib_epoch, Targets::Design);
    if let Some(e) = report.errors.first() {
        return Err(e.clone());
    }
    let design = find_design(g, &report)?;
    let notes = report.notes(g);
    let has_sources = !(design.texts.is_empty() && design.svgs.is_empty() && design.drawn.is_empty() && design.recipes.is_empty());
    let field = if has_sources {
        let mut baked = lib.clone();
        design.bake_all(&mut baked);
        attributed_field_report(&design, &baked, &design.draft, FIELD_THETA_STEPS, FIELD_PROFILE_STEPS)
    } else {
        attributed_field_report(&design, lib, &design.draft, FIELD_THETA_STEPS, FIELD_PROFILE_STEPS)
    };
    Ok(DesignOut { design, field, notes, report })
}

fn find_design(g: &Graph, report: &EvalReport) -> Result<Arc<RingDesign>, GraphError> {
    let sinks: Vec<&crate::graph::Node> = g.nodes.iter().filter(|n| n.kind == OUTPUT_KIND).collect();
    if let Some(sink) = sinks.first() {
        let w = g
            .wire_into(sink.id, OUTPUT_DESIGN_PIN)
            .ok_or_else(|| GraphError::at(sink.id, format!("nothing is wired into {OUTPUT_DESIGN_PIN:?}")))?;
        return match report.value(w.from, &w.out) {
            Some(Value::Design(d)) => Ok(d.clone()),
            Some(Value::List(items)) => match items.as_slice() {
                [Value::Design(d)] => Ok(d.clone()),
                _ => Err(GraphError::at(sink.id, format!("{} designs reach the output; a ring is one design", items.len()))),
            },
            Some(Value::Null) | None => Err(GraphError::at(w.from, "the design failed upstream")),
            Some(other) => Err(GraphError::at(sink.id, format!("{OUTPUT_DESIGN_PIN:?} carries {}, not a design", other.kind()))),
        };
    }
    for id in report.order.iter().rev() {
        if let Some(outs) = report.values.get(id) {
            for v in outs.values() {
                if let Value::Design(d) = v {
                    return Ok(d.clone());
                }
            }
        }
    }
    Err(GraphError::global("no node in this graph produces a design"))
}

impl std::fmt::Display for ValueKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Mode, Node, Wire};
    use crate::registry::{Category, NodeError, Outputs, PinSpec};
    use crate::value::Literal;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static EXPORTS: AtomicUsize = AtomicUsize::new(0);

    fn number(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        Ok(Outputs::one("out", i.number("value")?))
    }
    fn add(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        Ok(Outputs::one("sum", i.number("a")? + i.number("b")?))
    }
    fn div(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        let b = i.number("b")?;
        if b == 0.0 {
            return Err(NodeError::input("b", "division by zero"));
        }
        Ok(Outputs::one("quot", i.number("a")? / b))
    }
    fn len(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        Ok(Outputs::one("n", i.list("items").len() as i64))
    }
    fn series(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        let count = i.int("count")?.max(0) as usize;
        let count = if count > MAX_LIST_ITEMS {
            ctx.warn(format!("count {count} clamped to {MAX_LIST_ITEMS}"));
            MAX_LIST_ITEMS
        } else {
            count
        };
        Ok(Outputs::one("out", (0..count).map(|k| Value::Number(k as f64)).collect::<Vec<_>>()))
    }
    fn design(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        let mut d = RingDesign::default();
        d.name = i.text("name")?.to_string();
        Ok(Outputs::one("design", d))
    }
    fn export(ctx: &mut EvalCtx<'_>, _: &Node, _: &Inputs) -> Result<Outputs, NodeError> {
        assert!(ctx.run_side_effects);
        EXPORTS.fetch_add(1, Ordering::SeqCst);
        Ok(Outputs::default())
    }

    fn reg() -> Registry {
        let mut r = Registry::empty();
        r.register(NodeSpec::new("number", "Number", Category::Source).input(PinSpec::item("value", ValueKind::Number).default(0.0)).output(PinSpec::item("out", ValueKind::Number)).eval(number)).unwrap();
        r.register(NodeSpec::new("add", "Add", Category::Util).input(PinSpec::item("a", ValueKind::Number).default(0.0)).input(PinSpec::item("b", ValueKind::Number).default(0.0)).output(PinSpec::item("sum", ValueKind::Number)).eval(add)).unwrap();
        r.register(NodeSpec::new("div", "Divide", Category::Util).input(PinSpec::item("a", ValueKind::Number).default(1.0)).input(PinSpec::item("b", ValueKind::Number).default(1.0)).output(PinSpec::item("quot", ValueKind::Number)).eval(div)).unwrap();
        r.register(NodeSpec::new("len", "Length", Category::Util).input(PinSpec::list("items", ValueKind::Any)).output(PinSpec::item("n", ValueKind::Int)).eval(len)).unwrap();
        r.register(NodeSpec::new("series", "Series", Category::Source).input(PinSpec::item("count", ValueKind::Int).default(3i64)).output(PinSpec::list("out", ValueKind::Number)).eval(series)).unwrap();
        r.register(NodeSpec::new("design", "Design", Category::Band).input(PinSpec::item("name", ValueKind::Text).default("Untitled")).output(PinSpec::item("design", ValueKind::Design)).eval(design)).unwrap();
        r.register(NodeSpec::new(OUTPUT_KIND, "Output", Category::Sink).input(PinSpec::item(OUTPUT_DESIGN_PIN, ValueKind::Design))).unwrap();
        r.register(NodeSpec::new("export", "Export", Category::Sink).side_effect().input(PinSpec::item("what", ValueKind::Any)).eval(export)).unwrap();
        r
    }

    fn lib() -> AlphaLibrary {
        AlphaLibrary::default()
    }

    fn lit_list(xs: &[f64]) -> Literal {
        Literal::List(xs.iter().map(|x| Literal::Number(*x)).collect())
    }

    #[test]
    fn longest_list_matching_repeats_the_last_item() {
        let reg = reg();
        let mut g = Graph::default();
        let a = g.add("add").unwrap();
        g.set_input(a, "a", lit_list(&[1.0, 2.0, 3.0])).unwrap();
        g.set_input(a, "b", lit_list(&[10.0])).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert!(r.errors.is_empty());
        assert_eq!(r.value(a, "sum"), Some(&Value::from(vec![11.0, 12.0, 13.0])));
        assert_eq!(r.status[&a].items, 3);

        // Two scalars give a scalar, not a one-item list.
        g.set_input(a, "a", Literal::Number(2.0)).unwrap();
        g.set_input(a, "b", Literal::Number(3.0)).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(a, "sum"), Some(&Value::Number(5.0)));

        // A one-item list keeps the node in list context.
        g.set_input(a, "a", lit_list(&[2.0])).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(a, "sum"), Some(&Value::from(vec![5.0])));

        // Defaults fill an unwired, unset pin.
        let b = g.add("add").unwrap();
        g.set_input(b, "a", lit_list(&[1.0, 2.0])).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(b, "sum"), Some(&Value::from(vec![1.0, 2.0])));
    }

    #[test]
    fn empty_in_is_empty_out() {
        let reg = reg();
        let mut g = Graph::default();
        let a = g.add("add").unwrap();
        let n = g.add("number").unwrap();
        g.set_input(a, "a", Literal::List(vec![])).unwrap();
        g.set_input(a, "b", lit_list(&[1.0, 2.0])).unwrap();
        g.connect(a, "sum", n, "value").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(a, "sum"), Some(&Value::List(vec![])));
        assert_eq!(r.status[&a].items, 0);
        assert_eq!(r.value(n, "out"), Some(&Value::List(vec![])), "emptiness flows on");
        assert!(!r.any_failed());
    }

    #[test]
    fn a_failed_item_is_null_and_its_siblings_continue() {
        let reg = reg();
        let mut g = Graph::default();
        let d = g.add("div").unwrap();
        let a = g.add("add").unwrap();
        g.set_input(d, "a", lit_list(&[1.0, 2.0, 3.0])).unwrap();
        g.set_input(d, "b", lit_list(&[1.0, 0.0, 2.0])).unwrap();
        g.connect(d, "quot", a, "a").unwrap();
        g.set_input(a, "b", Literal::Number(1.0)).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(d, "quot"), Some(&Value::List(vec![Value::Number(1.0), Value::Null, Value::Number(1.5)])));
        assert_eq!(r.status[&d].errors, vec![(1, "b: division by zero".to_string())]);
        assert_eq!(r.status[&d].error_count, 1);
        // Downstream, the Null item fails where it is read and the rest run.
        assert_eq!(r.value(a, "sum"), Some(&Value::List(vec![Value::Number(2.0), Value::Null, Value::Number(2.5)])));
        assert_eq!(r.status[&a].errors, vec![(1, "a: expected a number".to_string())]);
        let notes = r.notes(&g);
        assert!(notes.iter().any(|n| n == "#1 div: item 1: b: division by zero"), "{notes:?}");

        // A value that cannot be coerced fails its item at the pin.
        let mut g2 = Graph::default();
        let a2 = g2.add("add").unwrap();
        g2.set_input(a2, "a", Literal::List(vec![Literal::Number(1.0), Literal::Text("x".into()), Literal::Number(3.0)])).unwrap();
        let r = Evaluator::new().evaluate(&g2, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(a2, "sum"), Some(&Value::List(vec![Value::Number(1.0), Value::Null, Value::Number(3.0)])));
        assert_eq!(r.status[&a2].errors[0], (1, "a: cannot take text as number".to_string()));
        assert_eq!(r.status[&a2].errors.len(), 1, "the pin failure alone; a poisoned item does not run");
    }

    #[test]
    fn nested_lists_pass_whole_and_list_pins_take_the_whole_list() {
        let reg = reg();
        let mut g = Graph::default();
        let s = g.add("series").unwrap();
        let l = g.add("len").unwrap();
        g.set_input(s, "count", Literal::Int(5)).unwrap();
        g.connect(s, "out", l, "items").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(l, "n"), Some(&Value::Int(5)), "a list pin sees all five");
        assert_eq!(r.status[&l].items, 1, "and runs once");

        // Series over a list of counts nests: two lists, passed whole.
        g.set_input(s, "count", Literal::List(vec![Literal::Int(2), Literal::Int(3)])).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        match r.value(s, "out") {
            Some(Value::List(outer)) => {
                assert_eq!(outer.len(), 2);
                assert_eq!(outer[1], Value::from(vec![0.0, 1.0, 2.0]));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(r.value(l, "n"), Some(&Value::Int(2)), "the outer list's length");

        // An item pin handed a nested list gets the inner list as one item.
        let a = g.add("add").unwrap();
        g.connect(s, "out", a, "a").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.status[&a].items, 2);
        assert_eq!(r.status[&a].error_count, 2, "a list is not a number");
        assert!(r.status[&a].errors[0].1.contains("cannot take list as number"), "{:?}", r.status[&a].errors);

        // A scalar on a list pin reads as a one-item list.
        let l2 = g.add("len").unwrap();
        g.set_input(l2, "items", Literal::Number(7.0)).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(l2, "n"), Some(&Value::Int(1)));
        // And nothing on a list pin is an empty list, not a Null item.
        let l3 = g.add("len").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(l3, "n"), Some(&Value::Int(0)));
    }

    #[test]
    fn the_cache_reruns_exactly_the_chain_below_an_edit() {
        let reg = reg();
        let mut g = Graph::default();
        let n1 = g.add("number").unwrap();
        let a1 = g.add("add").unwrap();
        let a2 = g.add("add").unwrap();
        let a3 = g.add("add").unwrap();
        let n9 = g.add("number").unwrap();
        g.set_input(n1, "value", Literal::Number(1.0)).unwrap();
        g.connect(n1, "out", a1, "a").unwrap();
        g.connect(a1, "sum", a2, "a").unwrap();
        g.connect(a2, "sum", a3, "a").unwrap();
        g.set_input(a3, "b", Literal::Number(100.0)).unwrap();
        let mut ev = Evaluator::new();
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.ran().len(), 5);
        assert_eq!(r.value(a3, "sum"), Some(&Value::Number(101.0)));
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.ran(), Vec::<NodeId>::new(), "nothing moved, nothing runs");
        assert_eq!(r.cached().len(), 5);
        assert_eq!(r.value(a3, "sum"), Some(&Value::Number(101.0)), "cached values are served");

        g.set_input(n1, "value", Literal::Number(2.0)).unwrap();
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.ran(), vec![n1, a1, a2, a3], "the chain, and only the chain");
        assert_eq!(r.cached(), vec![n9]);
        assert_eq!(r.value(a3, "sum"), Some(&Value::Number(102.0)));

        g.set_input(a3, "b", Literal::Number(0.0)).unwrap();
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.ran(), vec![a3]);

        // Rewiring moves the signature too.
        g.connect(n9, "out", a2, "b").unwrap();
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.ran(), vec![a2, a3]);

        // A new library epoch re-runs everything; a removed node leaves the cache.
        let r = ev.evaluate(&g, &reg, &lib(), 1, Targets::AllPure);
        assert_eq!(r.ran().len(), 5);
        g.remove(n9).unwrap();
        ev.evaluate(&g, &reg, &lib(), 1, Targets::AllPure);
        assert_eq!(ev.cached_nodes(), 4);
    }

    #[test]
    fn cycles_and_fan_in_stop_evaluation_with_named_errors() {
        let reg = reg();
        let mut g = Graph::default();
        let a = g.add("add").unwrap();
        let b = g.add("add").unwrap();
        g.connect(a, "sum", b, "a").unwrap();
        g.wires.push(Wire { from: b, out: "sum".into(), to: a, input: "a".into() });
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert!(r.errors.iter().any(|e| e.message.contains("cycle")), "{:?}", r.errors);
        assert!(r.values.is_empty(), "nothing ran");
        let mut g2 = Graph::default();
        let n = g2.add("number").unwrap();
        let m = g2.add("number").unwrap();
        let s = g2.add("add").unwrap();
        g2.wires.push(Wire { from: n, out: "out".into(), to: s, input: "a".into() });
        g2.wires.push(Wire { from: m, out: "out".into(), to: s, input: "a".into() });
        let r = Evaluator::new().evaluate(&g2, &reg, &lib(), 0, Targets::AllPure);
        assert!(r.errors.iter().any(|e| e.node == Some(s) && e.message.contains("more than one wire")), "{:?}", r.errors);
    }

    #[test]
    fn oversize_lists_clamp_and_warn() {
        let reg = reg();
        let mut g = Graph::default();
        let s = g.add("series").unwrap();
        let l = g.add("len").unwrap();
        g.set_input(s, "count", Literal::Int(MAX_LIST_ITEMS as i64 + 5)).unwrap();
        g.connect(s, "out", l, "items").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.value(l, "n"), Some(&Value::Int(MAX_LIST_ITEMS as i64)));
        assert!(r.status[&s].warnings[0].contains("clamped"), "{:?}", r.status[&s].warnings);
        // A literal list past the cap is cut at the pin, with a warning.
        let a = g.add("add").unwrap();
        g.set_input(a, "a", Literal::List(vec![Literal::Number(1.0); MAX_LIST_ITEMS + 3])).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert_eq!(r.status[&a].items, MAX_LIST_ITEMS);
        assert!(r.status[&a].warnings[0].contains("kept the first"), "{:?}", r.status[&a].warnings);
    }

    #[test]
    fn side_effects_run_only_when_asked_and_targets_scope_the_run() {
        let reg = reg();
        let mut g = Graph::default();
        let n = g.add("number").unwrap();
        let e = g.add("export").unwrap();
        let lone = g.add("number").unwrap();
        g.connect(n, "out", e, "what").unwrap();
        let before = EXPORTS.load(Ordering::SeqCst);
        let mut ev = Evaluator::new();
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::AllPure);
        assert!(r.status[&e].skipped);
        assert_eq!(EXPORTS.load(Ordering::SeqCst), before);
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::Everything);
        assert!(!r.status[&e].skipped && !r.status[&e].cached);
        assert_eq!(EXPORTS.load(Ordering::SeqCst), before + 1);
        let r = ev.evaluate(&g, &reg, &lib(), 0, Targets::Everything);
        assert_eq!(EXPORTS.load(Ordering::SeqCst), before + 2, "a side effect is never served from the cache");
        assert!(r.status[&n].cached);
        // Targets::Node runs the node and what feeds it, nothing else.
        let r = Evaluator::new().evaluate(&g, &reg, &lib(), 0, Targets::Node(n));
        assert!(!r.status[&n].skipped);
        assert!(r.status[&lone].skipped);
        assert!(r.status[&e].skipped);
    }

    #[test]
    fn evaluate_design_returns_the_design_with_its_verdict() {
        let reg = reg();
        let mut g = Graph::new("plain", Mode::SandRing);
        let d = g.add("design").unwrap();
        let out = g.add(OUTPUT_KIND).unwrap();
        let lone = g.add("number").unwrap();
        g.set_input(d, "name", Literal::Text("Court".into())).unwrap();
        g.connect(d, "design", out, OUTPUT_DESIGN_PIN).unwrap();
        let mut ev = Evaluator::new();
        let res = evaluate_design(&mut ev, &g, &reg, &lib(), 0).unwrap();
        assert_eq!(res.design.name, "Court");
        assert_ne!(res.field.verdict, ringdesign_core::castability::Verdict::NotCastable);
        assert!(res.field.total_area_mm2 > 0.0);
        assert!(res.report.status[&lone].skipped, "Design targets only what feeds the sink");
        assert!(res.notes.is_empty(), "{:?}", res.notes);

        // Without a sink, the last design produced is the one.
        let mut g2 = Graph::default();
        let d2 = g2.add("design").unwrap();
        g2.set_input(d2, "name", Literal::Text("Loose".into())).unwrap();
        let res = evaluate_design(&mut Evaluator::new(), &g2, &reg, &lib(), 0).unwrap();
        assert_eq!(res.design.name, "Loose");

        // A sink fed a list of designs, or nothing, is refused by name.
        g.set_input(d, "name", Literal::List(vec![Literal::Text("a".into()), Literal::Text("b".into())])).unwrap();
        let err = evaluate_design(&mut ev, &g, &reg, &lib(), 0).unwrap_err();
        assert!(err.message.contains("2 designs"), "{err}");
        let g3 = Graph::default();
        let err = evaluate_design(&mut Evaluator::new(), &g3, &reg, &lib(), 0).unwrap_err();
        assert!(err.message.contains("no node"), "{err}");
    }
}
