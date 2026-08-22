//! The script node: pins declared in header comments, a body that reads
//! its inputs as variables and leaves its outputs as variables.
//!
//! ```text
//! // in: a: Number = 1.0, b: List<Number>
//! // out: h: Number, names: List<Text>
//! let h = a * 2.0;
//! let names = b.map(|x| `n${x}`);
//! ```

use std::sync::Arc;

use rhai::Scope;
use ringdesign_graph::graph::{Access, Node};
use ringdesign_graph::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use ringdesign_graph::value::{Literal, ValueKind};

use crate::{ScriptEngine, from_dynamic, to_dynamic};

pub const SCRIPT_KIND: &str = "script";

/// The default body a new script node carries.
pub const DEFAULT_SOURCE: &str = "// in: a: Number = 1.0, b: Number = 2.0\n// out: sum: Number\nlet sum = a + b;\n";

/// A parsed header line.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub inputs: Vec<PinSpec>,
    pub outputs: Vec<PinSpec>,
}

fn kind_of(name: &str) -> Option<(ValueKind, Access)> {
    let name = name.trim();
    if let Some(inner) = name.strip_prefix("List<").and_then(|s| s.strip_suffix('>')) {
        return kind_of(inner).map(|(k, _)| (k, Access::List));
    }
    let k = match name {
        "Number" | "number" | "float" => ValueKind::Number,
        "Int" | "int" | "integer" => ValueKind::Int,
        "Bool" | "bool" | "boolean" => ValueKind::Bool,
        "Text" | "text" | "string" => ValueKind::Text,
        "Any" | "any" => ValueKind::Any,
        "Design" => ValueKind::Design,
        "Profile" => ValueKind::Profile,
        "Shank" => ValueKind::Shank,
        "Head" => ValueKind::Head,
        "Gem" => ValueKind::Gem,
        "Window" => ValueKind::Window,
        "Layer" => ValueKind::Layer,
        "Entry" => ValueKind::Entry,
        "Stack" => ValueKind::Stack,
        "Alpha" | "AlphaRef" => ValueKind::AlphaRef,
        "Mesh" => ValueKind::Mesh,
        "Field" => ValueKind::Field,
        "Path" => ValueKind::Path,
        "Json" | "json" => ValueKind::Json,
        "List" | "list" => ValueKind::List,
        _ => return None,
    };
    Some((k, Access::Item))
}

/// Parse the `// in:` and `// out:` lines at the top of a script.
pub fn parse_header(source: &str) -> Result<Header, String> {
    let mut h = Header { inputs: Vec::new(), outputs: Vec::new() };
    for (ln, line) in source.lines().enumerate() {
        let t = line.trim();
        let (is_in, rest) = if let Some(r) = t.strip_prefix("// in:") {
            (true, r)
        } else if let Some(r) = t.strip_prefix("// out:") {
            (false, r)
        } else if t.starts_with("//") || t.is_empty() {
            continue;
        } else {
            break;
        };
        for decl in rest.split(',').map(str::trim).filter(|d| !d.is_empty()) {
            let (name_ty, default) = match decl.split_once('=') {
                Some((a, b)) => (a.trim(), Some(b.trim())),
                None => (decl, None),
            };
            let (name, ty) = name_ty.split_once(':').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((name_ty.trim(), "Any"));
            if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Err(format!("line {}: {name:?} is not a pin name", ln + 1));
            }
            let (kind, access) = kind_of(ty).ok_or_else(|| format!("line {}: {ty:?} is not a pin type", ln + 1))?;
            let mut pin = if access == Access::List { PinSpec::list(name, kind) } else { PinSpec::item(name, kind) };
            pin.doc = format!("Script {} {}", if is_in { "input" } else { "output" }, name);
            if let Some(d) = default {
                let lit: Literal = serde_json::from_str(d).map_err(|e| format!("line {}: default {d:?}: {e}", ln + 1))?;
                pin.default = Some(lit);
            } else if is_in {
                pin = pin.optional();
            }
            if is_in { h.inputs.push(pin) } else { h.outputs.push(pin) }
        }
    }
    Ok(h)
}

fn source_of(node: &Node) -> &str {
    node.params.get("source").and_then(|v| v.as_str()).unwrap_or(DEFAULT_SOURCE)
}

fn pins(_: &NodeSpec, node: &Node, _: &Registry) -> (Vec<PinSpec>, Vec<PinSpec>) {
    let (mut inputs, outputs) = match parse_header(source_of(node)) {
        Ok(h) => (h.inputs, h.outputs),
        Err(_) => (Vec::new(), Vec::new()),
    };
    // A literal left on a pin the header no longer names stays a pin, so
    // the graph still validates and the node reports the header itself.
    for k in node.inputs.keys() {
        if !inputs.iter().any(|p| &p.name == k) {
            inputs.push(PinSpec::item(k.clone(), ValueKind::Any).doc("Not in the header").optional());
        }
    }
    (inputs, outputs)
}

fn run(engine: &ScriptEngine, ctx: &mut EvalCtx<'_>, node: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let source = source_of(node);
    let header = parse_header(source).map_err(|e| NodeError::new(format!("header: {e}")))?;
    let mut scope = Scope::new();
    for pin in &header.inputs {
        scope.push_dynamic(pin.name.clone(), to_dynamic(i.get(&pin.name)));
    }
    scope.push("i", ctx.item as i64);
    scope.push("n", ctx.items as i64);
    let _ = engine.eval(source, &mut scope).map_err(NodeError::new)?;
    let mut out = Outputs::default();
    for pin in &header.outputs {
        let v = scope.get_value::<rhai::Dynamic>(&pin.name).map(from_dynamic).ok_or_else(|| NodeError::new(format!("the script never set `{}`", pin.name)))?;
        let v = pin.kind.coerce(v).map_err(|e| NodeError::new(format!("{}: {e}", pin.name)))?;
        out.values.insert(pin.name.clone(), v);
    }
    Ok(out)
}

/// Params for a new script node with `source`.
pub fn params_for(source: &str) -> serde_json::Value {
    serde_json::json!({ "source": source })
}

pub fn register(reg: &mut Registry, engine: Arc<ScriptEngine>) {
    let spec = NodeSpec::new(SCRIPT_KIND, "Script", Category::Util)
        .doc("A rhai script with pins declared in its header: `// in: a: Number = 1.0, b: List<Number>` and `// out: h: Number`. Inputs arrive as variables, outputs are read back from variables; `i` and `n` say which item this is.")
        .input(PinSpec::item("source", ValueKind::Text).widget(Widget::TextArea).doc("Unused pin: the source lives in params").optional())
        .resolve(pins)
        .eval(move |ctx, node, i| run(&engine, ctx, node, i));
    // The source is a param, not a pin: drop the placeholder input so the
    // header alone decides the pins.
    let spec = NodeSpec { inputs: Vec::new(), ..spec };
    reg.register(spec).expect("unique");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::AlphaLibrary;
    use ringdesign_graph::eval::{Evaluator, Targets};
    use ringdesign_graph::graph::Graph;
    use ringdesign_graph::value::Value;

    #[test]
    fn headers_declare_pins_and_name_their_errors_by_line() {
        let h = parse_header("// in: a: Number = 1.0, b: List<Number>\n// out: h: Number, names: List<Text>\nlet h = a;").unwrap();
        assert_eq!(h.inputs.len(), 2);
        assert_eq!((h.inputs[0].kind, h.inputs[0].access, h.inputs[0].default.clone()), (ValueKind::Number, Access::Item, Some(Literal::Number(1.0))));
        assert_eq!((h.inputs[1].kind, h.inputs[1].access), (ValueKind::Number, Access::List));
        assert!(h.inputs[1].optional);
        assert_eq!(h.outputs.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["h", "names"]);
        let err = parse_header("// in: a: Number\n// out: 1bad: Number").unwrap_err();
        assert!(err.starts_with("line 2:"), "{err}");
        let err = parse_header("// in: a: Quaternion").unwrap_err();
        assert!(err.contains("line 1") && err.contains("Quaternion"), "{err}");
        let err = parse_header("// in: a: Number = nope").unwrap_err();
        assert!(err.contains("line 1") && err.contains("default"), "{err}");
    }

    #[test]
    fn a_script_node_maps_over_a_list_and_reports_its_failures() {
        let reg = crate::registry();
        let lib = AlphaLibrary::default();
        let mut g = Graph::default();
        let s = g.add(SCRIPT_KIND).unwrap();
        g.node_mut(s).unwrap().params = params_for("// in: a: Number = 1.0, k: Int = 3\n// out: h: Number, tag: Text\nlet h = a * k;\nlet tag = `item ${i} of ${n}`;\n");
        g.set_input(s, "a", Literal::List((1..=5).map(|x| Literal::Number(x as f64)).collect())).unwrap();
        assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
        let r = Evaluator::with_exprs(crate::engine()).evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(r.value(s, "h"), Some(&Value::from(vec![3.0, 6.0, 9.0, 12.0, 15.0])), "five in, five out");
        match r.value(s, "tag") {
            Some(Value::List(t)) => assert_eq!(t[4], Value::Text("item 4 of 5".into())),
            other => panic!("{other:?}"),
        }
        // A list pin sees the whole list once.
        let l = g.add(SCRIPT_KIND).unwrap();
        g.node_mut(l).unwrap().params = params_for("// in: xs: List<Number>\n// out: total: Number\nlet total = sum(xs);\n");
        g.connect(s, "h", l, "xs").unwrap();
        let r = Evaluator::with_exprs(crate::engine()).evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert_eq!(r.value(l, "total"), Some(&Value::Number(45.0)));
        // A script that forgets its output, a runaway, and a bad header all say so.
        g.node_mut(s).unwrap().params = params_for("// out: h: Number\nlet q = 1;\n");
        let r = Evaluator::with_exprs(crate::engine()).evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(r.status[&s].errors[0].1.contains("never set `h`"), "{:?}", r.status[&s].errors);
        g.node_mut(s).unwrap().params = params_for("// out: h: Number\nloop {}\n");
        let r = Evaluator::with_exprs(crate::engine()).evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(r.status[&s].errors[0].1.contains("Too many operations"), "{:?}", r.status[&s].errors);
        // A header that fails to parse declares no outputs, so the wire below
        // would dangle; the node still reports the header on its own.
        g.disconnect(l, "xs");
        g.node_mut(s).unwrap().params = params_for("// in: a: Nope\nlet h = 1;\n");
        let r = Evaluator::with_exprs(crate::engine()).evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(r.status[&s].errors[0].1.starts_with("header: line 1"), "{:?}", r.status[&s].errors);
    }
}
