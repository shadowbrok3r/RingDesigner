//! Sandboxed rhai for the graph: expression pins and script nodes.
//!
//! One [`ScriptEngine`] per process, built with every cap rhai offers —
//! operations, call depth, expression depth, array, map and string sizes —
//! `eval` disabled and no module resolver, so a script can compute and
//! nothing else. Values cross as plain rhai types (numbers, bools, strings,
//! arrays); a design is a registered handle with the chart's numbers as
//! functions on it; every other handle passes through opaque.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use rhai::{AST, Array, Dynamic, Engine, Scope};
use ringdesign_core::RingDesign;
use ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG;
use ringdesign_graph::MAX_LIST_ITEMS;
use ringdesign_graph::eval::{ExprEvaluator, ExprScope};
use ringdesign_graph::registry::Registry;
use ringdesign_graph::value::Value;

pub mod node;

/// Operations a single evaluation may spend before it is stopped.
pub const MAX_OPERATIONS: u64 = 200_000;
pub const MAX_CALL_LEVELS: usize = 32;
pub const MAX_STRING_BYTES: usize = 64 * 1024;

/// The sandbox.
pub struct ScriptEngine {
    engine: Engine,
    asts: Mutex<HashMap<u64, Arc<AST>>>,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A design handle inside a script.
#[derive(Clone)]
pub struct DesignRef(pub Arc<RingDesign>);

/// Any other handle, carried through untouched.
#[derive(Clone)]
pub struct Opaque(pub Value);

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine
            .set_max_operations(MAX_OPERATIONS)
            .set_max_call_levels(MAX_CALL_LEVELS)
            .set_max_expr_depths(64, 32)
            .set_max_array_size(MAX_LIST_ITEMS)
            .set_max_map_size(1024)
            .set_max_string_size(MAX_STRING_BYTES)
            .set_module_resolver(rhai::module_resolvers::DummyModuleResolver);
        engine.disable_symbol("eval");
        engine.register_type_with_name::<DesignRef>("Design");
        engine.register_type_with_name::<Opaque>("Handle");
        api::register(&mut engine);
        Self { engine, asts: Mutex::new(HashMap::new()) }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Compile once per distinct source.
    pub fn compile(&self, code: &str) -> Result<Arc<AST>, String> {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        code.hash(&mut h);
        let key = h.finish();
        if let Some(ast) = self.asts.lock().ok().and_then(|m| m.get(&key).cloned()) {
            return Ok(ast);
        }
        let ast = Arc::new(self.engine.compile(code).map_err(|e| e.to_string())?);
        if let Ok(mut m) = self.asts.lock() {
            if m.len() > 256 {
                m.clear();
            }
            m.insert(key, ast.clone());
        }
        Ok(ast)
    }

    /// Run `code` with `scope` and return what it evaluates to.
    pub fn eval(&self, code: &str, scope: &mut Scope<'static>) -> Result<Dynamic, String> {
        let ast = self.compile(code)?;
        self.engine.eval_ast_with_scope::<Dynamic>(scope, &ast).map_err(|e| e.to_string())
    }
}

/// A graph value as a script value.
pub fn to_dynamic(v: &Value) -> Dynamic {
    match v {
        Value::Null => Dynamic::UNIT,
        Value::Number(x) => Dynamic::from_float(*x),
        Value::Int(i) => Dynamic::from_int(*i),
        Value::Bool(b) => Dynamic::from_bool(*b),
        Value::Text(s) | Value::AlphaRef(s) => Dynamic::from(s.clone()),
        Value::List(items) => Dynamic::from_array(items.iter().map(to_dynamic).collect()),
        Value::Json(j) => rhai::serde::to_dynamic(&**j).unwrap_or(Dynamic::UNIT),
        Value::Path(p) => Dynamic::from_array(p.iter().map(|q| Dynamic::from_array(vec![Dynamic::from_float(q[0]), Dynamic::from_float(q[1])])).collect()),
        Value::Design(d) => Dynamic::from(DesignRef(d.clone())),
        other => Dynamic::from(Opaque(other.clone())),
    }
}

/// A script value as a graph value.
pub fn from_dynamic(d: Dynamic) -> Value {
    if d.is_unit() {
        return Value::Null;
    }
    if d.is_int() {
        return Value::Int(d.as_int().unwrap_or(0));
    }
    if d.is_float() {
        return Value::Number(d.as_float().unwrap_or(f64::NAN));
    }
    if d.is_bool() {
        return Value::Bool(d.as_bool().unwrap_or(false));
    }
    if d.is_string() {
        return Value::Text(d.into_string().unwrap_or_default());
    }
    if d.is_array() {
        let items: Array = d.into_array().unwrap_or_default();
        return Value::List(items.into_iter().map(from_dynamic).collect());
    }
    if d.is::<DesignRef>() {
        return Value::Design(d.cast::<DesignRef>().0);
    }
    if d.is::<Opaque>() {
        return d.cast::<Opaque>().0;
    }
    if d.is_map() {
        return match rhai::serde::from_dynamic::<serde_json::Value>(&d) {
            Ok(j) => Value::Json(Arc::new(j)),
            Err(_) => Value::Null,
        };
    }
    Value::Text(d.to_string())
}

impl ExprEvaluator for ScriptEngine {
    fn eval_expr(&self, code: &str, scope: &ExprScope) -> Result<Value, String> {
        let mut s = Scope::new();
        for (k, v) in &scope.siblings {
            s.push_dynamic(k.clone(), to_dynamic(v));
        }
        s.push("i", scope.item as i64);
        s.push("n", scope.items as i64);
        self.eval(code, &mut s).map(from_dynamic)
    }
}

/// The process-wide sandbox.
pub fn engine() -> Arc<ScriptEngine> {
    static ENGINE: OnceLock<Arc<ScriptEngine>> = OnceLock::new();
    ENGINE.get_or_init(|| Arc::new(ScriptEngine::new())).clone()
}

/// The builtin node library plus the script node.
pub fn registry() -> Registry {
    let mut reg = Registry::builtin();
    node::register(&mut reg, engine());
    reg
}

/// The small API scripts see: interpolation, series, list folds, and the
/// chart's numbers off a design.
mod api {
    use super::*;

    pub fn register(engine: &mut Engine) {
        engine.register_fn("lerp", |a: f64, b: f64, t: f64| a + (b - a) * t);
        engine.register_fn("remap", |x: f64, a: f64, b: f64, c: f64, d: f64| if (b - a).abs() < 1e-300 { c } else { c + (d - c) * (x - a) / (b - a) });
        engine.register_fn("clamp", |x: f64, lo: f64, hi: f64| x.clamp(lo.min(hi), hi.max(lo)));
        engine.register_fn("deg", |x: f64| x.to_degrees());
        engine.register_fn("rad", |x: f64| x.to_radians());
        engine.register_fn("series", |start: f64, step: f64, count: i64| -> Array {
            let n = count.clamp(0, MAX_LIST_ITEMS as i64) as usize;
            (0..n).map(|k| Dynamic::from_float(start + step * k as f64)).collect()
        });
        engine.register_fn("linspace", |a: f64, b: f64, count: i64| -> Array {
            let n = count.clamp(0, MAX_LIST_ITEMS as i64) as usize;
            match n {
                0 => Vec::new(),
                1 => vec![Dynamic::from_float(a)],
                n => (0..n).map(|k| Dynamic::from_float(a + (b - a) * k as f64 / (n - 1) as f64)).collect(),
            }
        });
        engine.register_fn("sum", |a: Array| a.iter().filter_map(num).sum::<f64>());
        engine.register_fn("mean", |a: Array| {
            let xs: Vec<f64> = a.iter().filter_map(num).collect();
            if xs.is_empty() { 0.0 } else { xs.iter().sum::<f64>() / xs.len() as f64 }
        });
        engine.register_fn("minmax", |a: Array| -> Array {
            let xs: Vec<f64> = a.iter().filter_map(num).collect();
            let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if xs.is_empty() { Vec::new() } else { vec![Dynamic::from_float(lo), Dynamic::from_float(hi)] }
        });
        engine.register_fn("circumference", |d: DesignRef| d.0.field_context().circumference_mm);
        engine.register_fn("inner_diameter", |d: DesignRef| d.0.size.inner_diameter_mm());
        engine.register_fn("band_v_len", |d: DesignRef| d.0.field_context().band_v_len_mm);
        engine.register_fn("crest_v", |d: DesignRef| d.0.field_context().crest_v_mm);
        engine.register_fn("width", |d: DesignRef| d.0.profile.width_mm);
        engine.register_fn("thickness", |d: DesignRef| d.0.profile.thickness_mm);
        engine.register_fn("size", |d: DesignRef| d.0.size.0);
        engine.register_fn("name", |d: DesignRef| d.0.name.clone());
        engine.register_fn("side_faces", |d: DesignRef| -> Array {
            let ctx = d.0.field_context();
            let Some(faces) = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG) else { return Vec::new() };
            [faces.low, faces.high]
                .into_iter()
                .flatten()
                .map(|(lo, hi)| Dynamic::from_array(vec![Dynamic::from_float(lo), Dynamic::from_float(hi)]))
                .collect()
        });
    }

    fn num(d: &Dynamic) -> Option<f64> {
        if d.is_float() { d.as_float().ok() } else if d.is_int() { d.as_int().ok().map(|i| i as f64) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::ProfileStyle;

    #[test]
    fn the_sandbox_stops_a_runaway_script_and_refuses_eval() {
        let e = ScriptEngine::new();
        let mut s = Scope::new();
        let err = e.eval("loop {}", &mut s).unwrap_err();
        assert!(err.contains("Too many operations"), "{err}");
        let err = e.eval("eval(\"1 + 1\")", &mut s).unwrap_err();
        assert!(!err.is_empty());
        let err = e.eval("import \"fs\" as f; f::read(\"/etc/passwd\")", &mut s).unwrap_err();
        assert!(!err.is_empty());
        let err = e.eval("let a = []; loop { a.push(1); }", &mut s).unwrap_err();
        assert!(err.contains("Size of array") || err.contains("Too many"), "{err}");
    }

    #[test]
    fn values_cross_both_ways_and_the_api_reads_the_chart() {
        let e = ScriptEngine::new();
        let mut s = Scope::new();
        assert_eq!(from_dynamic(e.eval("lerp(0.0, 10.0, 0.25)", &mut s).unwrap()), Value::Number(2.5));
        assert_eq!(from_dynamic(e.eval("series(0.0, 30.0, 4)", &mut s).unwrap()), Value::from(vec![0.0, 30.0, 60.0, 90.0]));
        assert_eq!(from_dynamic(e.eval("mean([1, 2, 3, 4])", &mut s).unwrap()), Value::Number(2.5));
        assert_eq!(from_dynamic(e.eval("minmax([3.0, 1.0, 2.0])", &mut s).unwrap()), Value::from(vec![1.0, 3.0]));
        assert_eq!(from_dynamic(e.eval("clamp(5.0, 0.0, 1.0)", &mut s).unwrap()), Value::Number(1.0));
        assert_eq!(from_dynamic(e.eval("\"a\" + \"b\"", &mut s).unwrap()), Value::Text("ab".into()));
        assert_eq!(from_dynamic(e.eval("()", &mut s).unwrap()), Value::Null);
        assert_eq!(from_dynamic(e.eval("#{a: 1}", &mut s).unwrap()), Value::Json(Arc::new(serde_json::json!({"a": 1}))));

        // A dome has no side face; a squared band has two.
        let dome = RingDesign::default();
        let mut squared = RingDesign::default();
        squared.profile.apply_style(ProfileStyle::Flat);
        squared.profile.flatten_sides();
        let mut s = Scope::new();
        s.push_dynamic("dome", to_dynamic(&Value::from(dome)));
        s.push_dynamic("sq", to_dynamic(&Value::from(squared)));
        assert_eq!(from_dynamic(e.eval("side_faces(dome)", &mut s).unwrap()), Value::List(vec![]));
        match from_dynamic(e.eval("side_faces(sq)", &mut s).unwrap()) {
            Value::List(spans) => assert_eq!(spans.len(), 2, "{spans:?}"),
            other => panic!("{other:?}"),
        }
        let w = from_dynamic(e.eval("width(sq) * 0.5", &mut s).unwrap());
        assert_eq!(w, Value::Number(3.0));
        assert!(matches!(from_dynamic(e.eval("sq", &mut s).unwrap()), Value::Design(_)), "a handle comes back as itself");
    }

    #[test]
    fn an_expression_pin_sees_its_siblings_and_the_item_index() {
        use ringdesign_graph::eval::{Evaluator, Targets};
        use ringdesign_graph::graph::Graph;
        use ringdesign_graph::value::Literal;
        use ringdesign_core::AlphaLibrary;
        let reg = registry();
        let mut g = Graph::default();
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "width_mm", Literal::List(vec![Literal::Number(6.0), Literal::Number(9.0)])).unwrap();
        g.set_input(p, "thickness_mm", Literal::expr("width_mm / 3.0 + i * 0.1")).unwrap();
        assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
        let r = Evaluator::with_exprs(engine()).evaluate(&g, &reg, &AlphaLibrary::default(), 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        match r.value(p, "profile") {
            Some(Value::List(items)) => {
                let t: Vec<f64> = items.iter().map(|v| if let Value::Profile(bp) = v { bp.thickness_mm } else { f64::NAN }).collect();
                assert!((t[0] - 2.0).abs() < 1e-9 && (t[1] - 3.1).abs() < 1e-9, "{t:?}");
            }
            other => panic!("{other:?}"),
        }
        // Without an engine the pin fails in words; a bad expression names itself.
        let r = Evaluator::new().evaluate(&g, &reg, &AlphaLibrary::default(), 0, Targets::AllPure);
        assert!(r.status[&p].errors[0].1.contains("no expression engine"), "{:?}", r.status[&p].errors);
        g.set_input(p, "thickness_mm", Literal::expr("width_mm /")).unwrap();
        let r = Evaluator::with_exprs(engine()).evaluate(&g, &reg, &AlphaLibrary::default(), 0, Targets::AllPure);
        assert!(r.status[&p].errors[0].1.starts_with("thickness_mm: expression:"), "{:?}", r.status[&p].errors);
    }
}
