//! What nodes exist: their pins, parameters, categories and evaluators.
//!
//! A [`NodeSpec`] is data — a key, a label, a category, the modes it runs
//! in, its pins and a plain function to evaluate one item — so the palette,
//! the inspector, validation and evaluation all read the same table, and a
//! Python or MCP listing of the node library is a walk over it. Script and
//! cluster nodes declare pins per instance through [`NodeSpec::resolve`];
//! everything else has them fixed.

use std::collections::BTreeMap;
use std::sync::Arc;

use ringdesign_core::AlphaLibrary;

use crate::graph::{Access, Mode, Node, NodePins, PinInfo, PinLookup};
use crate::value::{Literal, Value, ValueKind};

/// Where a node sits in the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Source,
    Band,
    Shank,
    Layer,
    Generator,
    Alpha,
    Assembly,
    Sink,
    Util,
    Solid,
}

impl Category {
    pub const ALL: &'static [Category] = &[
        Category::Source,
        Category::Band,
        Category::Shank,
        Category::Layer,
        Category::Generator,
        Category::Alpha,
        Category::Assembly,
        Category::Sink,
        Category::Util,
        Category::Solid,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Source => "Sources",
            Category::Band => "Band",
            Category::Shank => "Shank",
            Category::Layer => "Layers",
            Category::Generator => "Generators",
            Category::Alpha => "Alphas",
            Category::Assembly => "Assembly",
            Category::Sink => "Sinks",
            Category::Util => "Utilities",
            Category::Solid => "Solids",
        }
    }
}

/// How an editor should draw an unwired input.
#[derive(Clone, Debug, PartialEq)]
pub enum Widget {
    /// Pick from the kind.
    Auto,
    Slider { min: f64, max: f64 },
    /// Millimetres, with the sensible range for that dimension.
    Mm { min: f64, max: f64 },
    /// Degrees around the ring.
    Angle,
    Checkbox,
    /// One of these names (an enum pin).
    Select(Vec<String>),
    TextLine,
    TextArea,
}

/// One pin.
#[derive(Clone, Debug, PartialEq)]
pub struct PinSpec {
    pub name: String,
    pub kind: ValueKind,
    pub access: Access,
    pub doc: String,
    /// What the pin reads when nothing is wired and no literal is set.
    pub default: Option<Literal>,
    pub widget: Widget,
    /// `Null` is an answer: the node treats an unset pin as "leave as is"
    /// rather than failing on it.
    pub optional: bool,
}

impl PinSpec {
    pub fn item(name: impl Into<String>, kind: ValueKind) -> Self {
        Self { name: name.into(), kind, access: Access::Item, doc: String::new(), default: None, widget: Widget::Auto, optional: false }
    }

    pub fn list(name: impl Into<String>, kind: ValueKind) -> Self {
        Self { access: Access::List, ..Self::item(name, kind) }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    pub fn default(mut self, value: impl Into<Literal>) -> Self {
        self.default = Some(value.into());
        self
    }

    pub fn widget(mut self, widget: Widget) -> Self {
        self.widget = widget;
        self
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// A text pin restricted to these names: an enum field.
    pub fn select(name: impl Into<String>, names: Vec<String>) -> Self {
        Self::item(name, ValueKind::Text).widget(Widget::Select(names))
    }

    pub fn info(&self) -> PinInfo {
        PinInfo { name: self.name.clone(), kind: self.kind, access: self.access }
    }
}

impl From<f64> for Literal {
    fn from(x: f64) -> Self {
        Literal::Number(x)
    }
}
impl From<i64> for Literal {
    fn from(i: i64) -> Self {
        Literal::Int(i)
    }
}
impl From<bool> for Literal {
    fn from(b: bool) -> Self {
        Literal::Bool(b)
    }
}
impl From<&str> for Literal {
    fn from(s: &str) -> Self {
        Literal::Text(s.into())
    }
}
impl From<String> for Literal {
    fn from(s: String) -> Self {
        Literal::Text(s)
    }
}

/// What one evaluation of a node sees besides its inputs.
pub struct EvalCtx<'a> {
    pub lib: &'a AlphaLibrary,
    pub mode: Mode,
    /// Whether sinks that write files or spawn work may do so this run.
    pub run_side_effects: bool,
    /// Which item of an implicit list this run is, and how many there are.
    pub item: usize,
    pub items: usize,
    /// Notes the node wants shown without failing.
    pub warnings: Vec<String>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(lib: &'a AlphaLibrary, mode: Mode) -> Self {
        Self { lib, mode, run_side_effects: false, item: 0, items: 1, warnings: Vec::new() }
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }
}

/// The resolved input values for one run: one per `Item` pin, the whole
/// list per `List` pin.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub values: BTreeMap<String, Value>,
}

impl Inputs {
    pub fn get(&self, name: &str) -> &Value {
        static NULL: Value = Value::Null;
        self.values.get(name).unwrap_or(&NULL)
    }

    pub fn number(&self, name: &str) -> Result<f64, NodeError> {
        self.get(name).as_number().ok_or_else(|| NodeError::input(name, "expected a number"))
    }

    pub fn int(&self, name: &str) -> Result<i64, NodeError> {
        self.get(name).as_int().ok_or_else(|| NodeError::input(name, "expected an integer"))
    }

    pub fn bool(&self, name: &str) -> Result<bool, NodeError> {
        self.get(name).as_bool().ok_or_else(|| NodeError::input(name, "expected a boolean"))
    }

    pub fn text(&self, name: &str) -> Result<&str, NodeError> {
        self.get(name).as_text().ok_or_else(|| NodeError::input(name, "expected text"))
    }

    pub fn list(&self, name: &str) -> Vec<Value> {
        match self.get(name) {
            Value::List(items) => items.clone(),
            Value::Null => Vec::new(),
            other => vec![other.clone()],
        }
    }
}

/// What a node produced: one value per output pin.
#[derive(Clone, Debug, Default)]
pub struct Outputs {
    pub values: BTreeMap<String, Value>,
}

impl Outputs {
    pub fn one(name: impl Into<String>, value: impl Into<Value>) -> Self {
        let mut o = Self::default();
        o.values.insert(name.into(), value.into());
        o
    }

    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.values.insert(name.into(), value.into());
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }
}

/// Why a node's run failed; attributed to an input when one is to blame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeError {
    pub input: Option<String>,
    pub message: String,
}

impl NodeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { input: None, message: message.into() }
    }
    pub fn input(input: impl Into<String>, message: impl Into<String>) -> Self {
        Self { input: Some(input.into()), message: message.into() }
    }
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.input {
            Some(i) => write!(f, "{i}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for NodeError {}

impl From<String> for NodeError {
    fn from(s: String) -> Self {
        NodeError::new(s)
    }
}
impl From<&str> for NodeError {
    fn from(s: &str) -> Self {
        NodeError::new(s)
    }
}
impl From<anyhow::Error> for NodeError {
    fn from(e: anyhow::Error) -> Self {
        NodeError::new(format!("{e:#}"))
    }
}

/// Evaluate one item of a node. A closure, so an adapter built over a
/// table (a struct's fields, a script's pins) can carry it.
pub type EvalFn = Arc<dyn Fn(&mut EvalCtx<'_>, &Node, &Inputs) -> Result<Outputs, NodeError> + Send + Sync>;
/// Pins for one instance, read from its params.
pub type ResolveFn = fn(&NodeSpec, &Node) -> (Vec<PinSpec>, Vec<PinSpec>);
/// Rewrite a node saved under an older graph format version.
pub type MigrateFn = fn(&mut Node, u32);

/// One node kind.
#[derive(Clone)]
pub struct NodeSpec {
    /// The registry key, e.g. `band.profile`; what a file stores.
    pub key: String,
    pub label: String,
    pub category: Category,
    pub modes: Vec<Mode>,
    pub inputs: Vec<PinSpec>,
    pub outputs: Vec<PinSpec>,
    pub doc: String,
    /// Runs only when the evaluation asks for side effects.
    pub side_effect: bool,
    pub eval: EvalFn,
    pub resolve: Option<ResolveFn>,
    pub migrate: Option<MigrateFn>,
}

impl std::fmt::Debug for NodeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeSpec")
            .field("key", &self.key)
            .field("category", &self.category)
            .field("inputs", &self.inputs.iter().map(|p| &p.name).collect::<Vec<_>>())
            .field("outputs", &self.outputs.iter().map(|p| &p.name).collect::<Vec<_>>())
            .finish()
    }
}

fn eval_nothing(_: &mut EvalCtx<'_>, _: &Node, _: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::default())
}

impl NodeSpec {
    pub fn new(key: impl Into<String>, label: impl Into<String>, category: Category) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            category,
            modes: vec![Mode::SandRing, Mode::Free],
            inputs: Vec::new(),
            outputs: Vec::new(),
            doc: String::new(),
            side_effect: false,
            eval: Arc::new(eval_nothing),
            resolve: None,
            migrate: None,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    pub fn input(mut self, pin: PinSpec) -> Self {
        self.inputs.push(pin);
        self
    }

    pub fn output(mut self, pin: PinSpec) -> Self {
        self.outputs.push(pin);
        self
    }

    pub fn modes(mut self, modes: &[Mode]) -> Self {
        self.modes = modes.to_vec();
        self
    }

    pub fn free_only(self) -> Self {
        self.modes(&[Mode::Free])
    }

    pub fn side_effect(mut self) -> Self {
        self.side_effect = true;
        self
    }

    pub fn eval(mut self, f: impl Fn(&mut EvalCtx<'_>, &Node, &Inputs) -> Result<Outputs, NodeError> + Send + Sync + 'static) -> Self {
        self.eval = Arc::new(f);
        self
    }

    pub fn resolve(mut self, f: ResolveFn) -> Self {
        self.resolve = Some(f);
        self
    }

    pub fn migrate(mut self, f: MigrateFn) -> Self {
        self.migrate = Some(f);
        self
    }

    /// The pins this instance has: the static ones, or what `resolve`
    /// reads from the node's params.
    pub fn pins_for(&self, node: &Node) -> (Vec<PinSpec>, Vec<PinSpec>) {
        match self.resolve {
            Some(f) => f(self, node),
            None => (self.inputs.clone(), self.outputs.clone()),
        }
    }
}

/// The node library.
#[derive(Default)]
pub struct Registry {
    specs: BTreeMap<String, NodeSpec>,
}

impl Registry {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Every node this build knows.
    pub fn builtin() -> Self {
        let mut reg = Self::default();
        crate::nodes::register_all(&mut reg);
        reg
    }

    /// Add a kind; a second spec under the same key is refused.
    pub fn register(&mut self, spec: NodeSpec) -> Result<(), String> {
        if spec.key.is_empty() {
            return Err("a node spec needs a key".into());
        }
        if self.specs.contains_key(&spec.key) {
            return Err(format!("{:?} is already registered", spec.key));
        }
        for p in spec.inputs.iter().chain(&spec.outputs) {
            if p.name.is_empty() {
                return Err(format!("{:?} has an unnamed pin", spec.key));
            }
        }
        self.specs.insert(spec.key.clone(), spec);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&NodeSpec> {
        self.specs.get(key)
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.specs.keys().map(String::as_str)
    }

    /// The palette for a mode: by category, then label.
    pub fn list(&self, mode: Mode) -> Vec<&NodeSpec> {
        let mut v: Vec<&NodeSpec> = self.specs.values().filter(|s| s.modes.contains(&mode)).collect();
        v.sort_by(|a, b| a.category.cmp(&b.category).then_with(|| a.label.cmp(&b.label)));
        v
    }

    /// The resolved pins of a node, or `None` for an unknown kind.
    pub fn node_pins(&self, node: &Node) -> Option<(Vec<PinSpec>, Vec<PinSpec>)> {
        self.get(&node.kind).map(|s| s.pins_for(node))
    }
}

impl PinLookup for Registry {
    fn pins(&self, node: &Node) -> Option<NodePins> {
        let spec = self.get(&node.kind)?;
        let (inputs, outputs) = spec.pins_for(node);
        Some(NodePins {
            inputs: inputs.iter().map(PinSpec::info).collect(),
            outputs: outputs.iter().map(PinSpec::info).collect(),
            modes: spec.modes.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    fn number(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        Ok(Outputs::one("out", i.number("value")?))
    }

    fn add(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
        Ok(Outputs::one("sum", i.number("a")? + i.number("b")?))
    }

    /// A script-like node: pins named in params.
    fn script_pins(_: &NodeSpec, node: &Node) -> (Vec<PinSpec>, Vec<PinSpec>) {
        let names = |key: &str| -> Vec<PinSpec> {
            node.params
                .get(key)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).map(|n| PinSpec::item(n, ValueKind::Number)).collect())
                .unwrap_or_default()
        };
        (names("in"), names("out"))
    }

    fn specs() -> Registry {
        let mut reg = Registry::empty();
        reg.register(
            NodeSpec::new("number", "Number", Category::Source)
                .input(PinSpec::item("value", ValueKind::Number).default(0.0).widget(Widget::Slider { min: -10.0, max: 10.0 }))
                .output(PinSpec::item("out", ValueKind::Number))
                .eval(number),
        )
        .unwrap();
        reg.register(
            NodeSpec::new("math.add", "Add", Category::Util)
                .input(PinSpec::item("a", ValueKind::Number).default(0.0))
                .input(PinSpec::item("b", ValueKind::Number).default(0.0))
                .output(PinSpec::item("sum", ValueKind::Number))
                .eval(add),
        )
        .unwrap();
        reg.register(NodeSpec::new("script", "Script", Category::Util).resolve(script_pins)).unwrap();
        reg.register(NodeSpec::new("solid.box", "Box", Category::Solid).free_only().output(PinSpec::item("solid", ValueKind::Solid)))
            .unwrap();
        reg.register(NodeSpec::new("sink.export", "Export", Category::Sink).side_effect().input(PinSpec::item("mesh", ValueKind::Mesh)))
            .unwrap();
        reg
    }

    #[test]
    fn registration_is_unique_and_listing_follows_mode_and_category() {
        let mut reg = specs();
        assert_eq!(reg.len(), 5);
        let dup = reg.register(NodeSpec::new("number", "Again", Category::Source));
        assert!(dup.unwrap_err().contains("already registered"));
        assert!(reg.register(NodeSpec::new("", "x", Category::Util)).is_err());
        assert!(reg.register(NodeSpec::new("bad", "x", Category::Util).input(PinSpec::item("", ValueKind::Any))).is_err());
        let sand: Vec<&str> = reg.list(Mode::SandRing).iter().map(|s| s.key.as_str()).collect();
        assert_eq!(sand, vec!["number", "sink.export", "math.add", "script"], "category order, then label");
        let free: Vec<&str> = reg.list(Mode::Free).iter().map(|s| s.key.as_str()).collect();
        assert!(free.contains(&"solid.box") && !sand.contains(&"solid.box"));
        assert!(reg.get("sink.export").unwrap().side_effect);
        assert_eq!(reg.get("number").unwrap().inputs[0].widget, Widget::Slider { min: -10.0, max: 10.0 });
        assert_eq!(reg.get("number").unwrap().inputs[0].default, Some(Literal::Number(0.0)));
    }

    #[test]
    fn instance_pins_come_from_params_and_validate_a_graph() {
        let reg = specs();
        let mut g = Graph::default();
        let n = g.add("number").unwrap();
        let s = g.add("script").unwrap();
        let a = g.add("math.add").unwrap();
        g.set_param(s, "/in", serde_json::json!(["x"])).unwrap();
        g.set_param(s, "/out", serde_json::json!(["y"])).unwrap();
        g.connect(n, "out", s, "x").unwrap();
        g.connect(s, "y", a, "a").unwrap();
        assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
        // Rename the script's output and the wire is now dangling by name.
        g.set_param(s, "/out", serde_json::json!(["z"])).unwrap();
        let errs = g.validate(Some(&reg));
        assert!(errs.iter().any(|e| e.message.contains("no output named \"y\"")), "{errs:?}");
        let (ins, outs) = reg.node_pins(g.node(s).unwrap()).unwrap();
        assert_eq!(ins.len(), 1);
        assert_eq!(outs[0].name, "z");
        assert!(reg.node_pins(&Node { kind: "nope".into(), ..g.node(s).unwrap().clone() }).is_none());
    }

    #[test]
    fn eval_functions_read_inputs_and_attribute_errors() {
        let reg = specs();
        let lib = AlphaLibrary::default();
        let mut ctx = EvalCtx::new(&lib, Mode::SandRing);
        let node = Node { id: crate::graph::NodeId(1), kind: "math.add".into(), params: serde_json::Value::Null, inputs: Default::default(), pos: [0.0; 2], label: None };
        let mut inputs = Inputs::default();
        inputs.values.insert("a".into(), Value::Number(1.5));
        inputs.values.insert("b".into(), Value::Int(2));
        let out = (reg.get("math.add").unwrap().eval)(&mut ctx, &node, &inputs).unwrap();
        assert_eq!(out.get("sum"), Some(&Value::Number(3.5)));
        inputs.values.insert("b".into(), Value::Text("x".into()));
        let err = (reg.get("math.add").unwrap().eval)(&mut ctx, &node, &inputs).unwrap_err();
        assert_eq!(err.input.as_deref(), Some("b"));
        assert_eq!(err.to_string(), "b: expected a number");
        ctx.warn("note");
        assert_eq!(ctx.warnings, vec!["note"]);
        assert_eq!(inputs.list("a"), vec![Value::Number(1.5)], "a single value reads as a one-item list");
        assert!(inputs.list("missing").is_empty());
    }

    #[test]
    fn every_category_has_a_label_and_the_builtin_registry_loads() {
        for &c in Category::ALL {
            assert!(!c.label().is_empty());
        }
        let _ = Registry::builtin();
    }
}
