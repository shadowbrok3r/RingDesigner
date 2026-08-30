//! The node library.
//!
//! One file per family; each exposes `register(reg)`. Keys are
//! `family.name`, labels are what the palette shows, and every spec
//! carries a doc line, because the palette, the MCP listing and the Python
//! module all read the same table.

use crate::registry::Registry;

pub mod alpha;
pub mod assembly;
pub mod band;
pub mod cluster;
pub mod gem;
pub mod generator;
pub mod idiom;
pub mod layer;
pub mod list;
pub mod math;
pub mod shank;
pub mod sink;
#[cfg(feature = "kernel-manifold")]
pub mod solid;
pub mod source;
pub mod structs;
pub mod text;

/// Register every builtin node kind.
pub fn register_all(reg: &mut Registry) {
    source::register(reg);
    math::register(reg);
    list::register(reg);
    text::register(reg);
    band::register(reg);
    shank::register(reg);
    gem::register(reg);
    layer::register(reg);
    assembly::register(reg);
    generator::register(reg);
    alpha::register(reg);
    sink::register(reg);
    cluster::register(reg);
    idiom::register(reg);
    #[cfg(feature = "kernel-manifold")]
    solid::register(reg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Access;
    use crate::value::ValueKind;
    use std::collections::BTreeSet;

    /// Every real node covers its struct.
    ///
    /// CLAUDE.md credits the `struct_node!` guard with making it so "a field
    /// added to the core cannot go unnoticed", and the guard was a
    /// `debug_assert!` inside `StructNode::build` — compiled out in release,
    /// and never walked by any test. `coverage_names_what_a_node_forgot_or_invented`
    /// checks the *helper* against two synthetic nodes and passed the whole
    /// time `head` was missing `crest_round_mm`; what actually caught that was
    /// the registry panicking inside an unrelated test.
    ///
    /// This is the assertion that should have caught it: build the real
    /// registry and read the failures back by name.
    #[test]
    fn every_node_covers_its_struct() {
        let log = crate::nodes::structs::coverage_failures();
        log.lock().expect("coverage log").clear();
        let reg = Registry::builtin();
        assert!(reg.len() >= 40, "the registry built: {} kinds", reg.len());
        let failures = log.lock().expect("coverage log").clone();
        assert!(
            failures.is_empty(),
            "{} node(s) do not cover their struct:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }

    /// The table is consistent: every spec documented, pins uniquely named,
    /// every scalar item input given a default so an unwired node still runs.
    #[test]
    fn the_builtin_table_is_consistent() {
        let reg = Registry::builtin();
        assert!(reg.len() >= 40, "{} kinds", reg.len());
        for key in reg.keys() {
            let spec = reg.get(key).unwrap();
            assert!(!spec.doc.is_empty(), "{key} has no doc");
            assert!(!spec.label.is_empty(), "{key} has no label");
            assert!(key.contains('.') || matches!(key, "number" | "int" | "bool" | "text" | "series" | "range" | "shank" | "head" | "gem" | "entry" | "window" | "stack" | "cluster"), "{key} is not family.name");
            // Inputs and outputs are separate namespaces: a wire names one of each.
            for pins in [&spec.inputs, &spec.outputs] {
                let mut seen = BTreeSet::new();
                for p in pins {
                    assert!(seen.insert(&p.name), "{key} names pin {:?} twice", p.name);
                    assert!(!p.doc.is_empty(), "{key}.{} has no doc", p.name);
                }
            }
            for p in &spec.inputs {
                let scalar = matches!(p.kind, ValueKind::Number | ValueKind::Int | ValueKind::Bool | ValueKind::Text);
                if scalar && p.access == Access::Item && !p.optional {
                    assert!(p.default.is_some(), "{key}.{} has no default", p.name);
                }
            }
        }
    }
}

#[cfg(test)]
mod acceptance {
    use crate::eval::{Evaluator, Targets};
    use crate::graph::{Graph, NodeId};
    use crate::registry::Registry;
    use crate::value::{Literal, Value};
    use crate::MAX_LIST_ITEMS;
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::default(), 0, Targets::AllPure)
    }

    fn nums(xs: &[f64]) -> Literal {
        Literal::List(xs.iter().map(|x| Literal::Number(*x)).collect())
    }

    fn list_of(v: Option<&Value>) -> Vec<f64> {
        match v {
            Some(Value::List(items)) => items.iter().map(|x| x.as_number().unwrap_or(f64::NAN)).collect(),
            Some(other) => vec![other.as_number().unwrap_or(f64::NAN)],
            None => vec![],
        }
    }

    #[test]
    fn the_headline_case_runs_through_real_nodes() {
        let mut g = Graph::default();
        let a = g.add("math.add").unwrap();
        g.set_input(a, "a", nums(&[1.0, 2.0, 3.0])).unwrap();
        g.set_input(a, "b", nums(&[10.0])).unwrap();
        let r = run(&g);
        assert_eq!(list_of(r.value(a, "out")), vec![11.0, 12.0, 13.0]);
    }

    #[test]
    fn series_and_range_count_and_clamp() {
        let mut g = Graph::default();
        let s = g.add("series").unwrap();
        g.set_input(s, "start", Literal::Number(0.0)).unwrap();
        g.set_input(s, "step", Literal::Number(30.0)).unwrap();
        g.set_input(s, "count", Literal::Int(4)).unwrap();
        let r_ = g.add("range").unwrap();
        g.set_input(r_, "count", Literal::Int(5)).unwrap();
        let one = g.add("range").unwrap();
        g.set_input(one, "count", Literal::Int(1)).unwrap();
        let big = g.add("series").unwrap();
        g.set_input(big, "count", Literal::Int(MAX_LIST_ITEMS as i64 * 2)).unwrap();
        let neg = g.add("series").unwrap();
        g.set_input(neg, "count", Literal::Int(-1)).unwrap();
        let r = run(&g);
        assert_eq!(list_of(r.value(s, "out")), vec![0.0, 30.0, 60.0, 90.0]);
        assert_eq!(list_of(r.value(r_, "out")), vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(list_of(r.value(one, "out")), vec![0.0]);
        assert_eq!(list_of(r.value(big, "out")).len(), MAX_LIST_ITEMS);
        assert!(r.status[&big].warnings[0].contains("clamped"));
        assert!(list_of(r.value(neg, "out")).is_empty(), "a negative count generates nothing");
        assert!(r.status[&neg].warnings[0].contains("negative"));
    }

    #[test]
    fn math_is_in_degrees_and_attributes_its_failures() {
        let mut g = Graph::default();
        let sin = g.add("math.sin").unwrap();
        g.set_input(sin, "x", Literal::Number(90.0)).unwrap();
        let m = g.add("math.mod").unwrap();
        g.set_input(m, "a", Literal::Number(-30.0)).unwrap();
        g.set_input(m, "b", Literal::Number(360.0)).unwrap();
        let d = g.add("math.div").unwrap();
        g.set_input(d, "a", nums(&[1.0, 2.0])).unwrap();
        g.set_input(d, "b", nums(&[2.0, 0.0])).unwrap();
        let rm = g.add("math.remap").unwrap();
        g.set_input(rm, "x", Literal::Number(5.0)).unwrap();
        g.set_input(rm, "from_max", Literal::Number(10.0)).unwrap();
        g.set_input(rm, "to_min", Literal::Number(90.0)).unwrap();
        g.set_input(rm, "to_max", Literal::Number(270.0)).unwrap();
        let cmp = g.add("math.compare").unwrap();
        g.set_input(cmp, "a", nums(&[1.0, 2.0, 3.0])).unwrap();
        g.set_input(cmp, "b", Literal::Number(2.0)).unwrap();
        g.set_input(cmp, "op", Literal::Text(">=".into())).unwrap();
        let sq = g.add("math.sqrt").unwrap();
        g.set_input(sq, "x", Literal::Number(-1.0)).unwrap();
        let r = run(&g);
        assert!((r.value(sin, "out").unwrap().as_number().unwrap() - 1.0).abs() < 1e-12);
        assert_eq!(r.value(m, "out"), Some(&Value::Number(330.0)));
        let q = list_of(r.value(d, "out"));
        assert_eq!(q[0], 0.5);
        assert!(q[1].is_nan(), "the zero-divisor item is Null");
        assert_eq!(r.status[&d].errors, vec![(1, "b: division by zero".to_string())]);
        assert_eq!(r.value(rm, "out"), Some(&Value::Number(180.0)));
        assert_eq!(r.value(cmp, "out"), Some(&Value::List(vec![Value::Bool(false), Value::Bool(true), Value::Bool(true)])));
        assert_eq!(r.status[&sq].errors[0].1, "x: negative");
    }

    #[test]
    fn list_utilities_do_what_they_say() {
        let mut g = Graph::default();
        let s = g.add("series").unwrap();
        g.set_input(s, "count", Literal::Int(5)).unwrap();
        let wire = |g: &mut Graph, kind: &str| -> NodeId {
            let n = g.add(kind).unwrap();
            g.connect(s, "out", n, "items").unwrap();
            n
        };
        let len = wire(&mut g, "list.length");
        let item = wire(&mut g, "list.item");
        g.set_input(item, "index", Literal::Int(-1)).unwrap();
        let strict = wire(&mut g, "list.item");
        g.set_input(strict, "index", Literal::Int(7)).unwrap();
        g.set_input(strict, "wrap", Literal::Bool(false)).unwrap();
        let rev = wire(&mut g, "list.reverse");
        let sl = wire(&mut g, "list.slice");
        g.set_input(sl, "start", Literal::Int(1)).unwrap();
        g.set_input(sl, "end", Literal::Int(-1)).unwrap();
        let graft = wire(&mut g, "list.graft");
        let flat = g.add("list.flatten").unwrap();
        g.connect(graft, "out", flat, "items").unwrap();
        let sorted = g.add("list.sort").unwrap();
        g.set_input(sorted, "items", nums(&[3.0, 1.0, 2.0])).unwrap();
        g.set_input(sorted, "descending", Literal::Bool(true)).unwrap();
        let mixed = g.add("list.sort").unwrap();
        g.set_input(mixed, "items", Literal::List(vec![Literal::Number(1.0), Literal::Text("a".into())])).unwrap();
        let merged = g.add("list.merge").unwrap();
        g.connect(s, "out", merged, "a").unwrap();
        g.set_input(merged, "b", Literal::Number(99.0)).unwrap();
        let total = wire(&mut g, "list.sum");
        let joined = wire(&mut g, "text.join");
        g.set_input(joined, "sep", Literal::Text("-".into())).unwrap();
        let r = run(&g);
        assert_eq!(r.value(len, "n"), Some(&Value::Int(5)));
        assert_eq!(r.value(item, "item"), Some(&Value::Number(4.0)), "−1 wraps to the last");
        assert!(r.status[&strict].errors[0].1.contains("outside"));
        assert_eq!(list_of(r.value(rev, "out")), vec![4.0, 3.0, 2.0, 1.0, 0.0]);
        assert_eq!(list_of(r.value(sl, "out")), vec![1.0, 2.0, 3.0]);
        match r.value(graft, "out") {
            Some(Value::List(outer)) => assert_eq!(outer[2], Value::List(vec![Value::Number(2.0)])),
            other => panic!("{other:?}"),
        }
        assert_eq!(list_of(r.value(flat, "out")), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(list_of(r.value(sorted, "out")), vec![3.0, 2.0, 1.0]);
        assert!(r.status[&mixed].errors[0].1.contains("all numbers or all text"));
        assert_eq!(list_of(r.value(merged, "out")), vec![0.0, 1.0, 2.0, 3.0, 4.0, 99.0]);
        assert_eq!(r.value(total, "sum"), Some(&Value::Number(10.0)));
        assert_eq!(r.value(total, "mean"), Some(&Value::Number(2.0)));
        assert_eq!(r.value(joined, "out"), Some(&Value::Text("0-1-2-3-4".into())));
    }
}
