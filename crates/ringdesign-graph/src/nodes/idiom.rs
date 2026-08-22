//! The list idioms a dataflow graph lives on: weave, entwine, cull,
//! partition, dispatch, shift, split, sort by keys, repeat, a gate, the
//! polar array, text formatting, JSON access and a conditional.
//!
//! Semantics follow Grasshopper's documented behaviour where one exists and
//! are pinned by the tests; the cases the documentation leaves open (an
//! exhausted weave stream, a partition size list, the polar array's ends)
//! are stated in each node's doc and re-pinned against measured ground
//! truth when it lands. `polar.array` is the integer lattice — an exact
//! division of the span — never a relaxation, because `u` wraps and a
//! count that divides the circle is what closes a pattern on itself.

use std::sync::Arc;

use crate::MAX_LIST_ITEMS;
use crate::graph::{Node, set_pointer};
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn ints(i: &Inputs, pin: &str) -> Result<Vec<i64>, NodeError> {
    i.list(pin).iter().enumerate().map(|(k, v)| v.as_int().ok_or_else(|| NodeError::input(pin, format!("item {k} is not an integer")))).collect()
}

fn bools(i: &Inputs, pin: &str) -> Result<Vec<bool>, NodeError> {
    i.list(pin).iter().enumerate().map(|(k, v)| v.as_bool().ok_or_else(|| NodeError::input(pin, format!("item {k} is not a boolean")))).collect()
}

fn weave(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let pattern = ints(i, "pattern")?;
    let streams: Vec<Vec<Value>> = ["s0", "s1", "s2", "s3"].iter().map(|p| i.list(p)).collect();
    if pattern.is_empty() {
        return Err(NodeError::input("pattern", "the pattern is empty"));
    }
    for (k, p) in pattern.iter().enumerate() {
        if !(0..4).contains(p) {
            return Err(NodeError::input("pattern", format!("item {k} is {p}; streams are 0..4")));
        }
    }
    let mut cursors = [0usize; 4];
    let mut out = Vec::new();
    let total: usize = streams.iter().map(Vec::len).sum();
    let mut step = 0usize;
    // An exhausted stream's pattern slots are skipped; the weave ends when
    // every stream is spent, so the output is every item exactly once.
    while out.len() < total && step < total * pattern.len() + pattern.len() {
        let s = pattern[step % pattern.len()] as usize;
        step += 1;
        if let Some(v) = streams[s].get(cursors[s]) {
            out.push(v.clone());
            cursors[s] += 1;
        }
    }
    Ok(Outputs::one("out", out))
}

fn entwine(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let branches: Vec<Value> = ["a", "b", "c", "d"].iter().map(|p| Value::List(i.list(p))).collect();
    Ok(Outputs::one("out", branches))
}

fn cull_pattern(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let pattern = bools(i, "pattern")?;
    if pattern.is_empty() {
        return Err(NodeError::input("pattern", "the pattern is empty"));
    }
    let out: Vec<Value> = items.into_iter().enumerate().filter(|(k, _)| pattern[k % pattern.len()]).map(|(_, v)| v).collect();
    Ok(Outputs::one("out", out))
}

fn cull_index(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let n = items.len() as i64;
    let wrap = i.bool("wrap")?;
    let mut drop = std::collections::BTreeSet::new();
    for (k, idx) in ints(i, "indices")?.into_iter().enumerate() {
        let j = if wrap {
            if n == 0 { continue } else { ((idx % n) + n) % n }
        } else if (0..n).contains(&idx) {
            idx
        } else {
            return Err(NodeError::input("indices", format!("item {k}: {idx} is outside 0..{n}")));
        };
        drop.insert(j as usize);
    }
    let out: Vec<Value> = items.into_iter().enumerate().filter(|(k, _)| !drop.contains(k)).map(|(_, v)| v).collect();
    Ok(Outputs::one("out", out))
}

fn cull_nth(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let n = i.int("n")?;
    if n < 1 {
        return Err(NodeError::input("n", format!("{n} is not a positive count")));
    }
    let out: Vec<Value> = items.into_iter().enumerate().filter(|(k, _)| (*k as i64 + 1) % n != 0).map(|(_, v)| v).collect();
    Ok(Outputs::one("out", out))
}

fn partition(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let sizes = ints(i, "size")?;
    if sizes.is_empty() || sizes.iter().any(|s| *s < 1) {
        return Err(NodeError::input("size", "sizes must be positive"));
    }
    // The size list repeats: {2, 3} partitions as 2, 3, 2, 3 …; the last
    // chunk keeps whatever is left.
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut k = 0usize;
    while at < items.len() {
        let size = sizes[k % sizes.len()] as usize;
        let end = (at + size).min(items.len());
        out.push(Value::List(items[at..end].to_vec()));
        at = end;
        k += 1;
    }
    Ok(Outputs::one("out", out))
}

fn dispatch(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let pattern = bools(i, "pattern")?;
    if pattern.is_empty() {
        return Err(NodeError::input("pattern", "the pattern is empty"));
    }
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (k, v) in items.into_iter().enumerate() {
        if pattern[k % pattern.len()] { a.push(v) } else { b.push(v) }
    }
    Ok(Outputs::one("a", a).with("b", b))
}

fn shift(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut items = i.list("items");
    let offset = i.int("offset")?;
    let wrap = i.bool("wrap")?;
    let n = items.len() as i64;
    if n == 0 {
        return Ok(Outputs::one("out", items));
    }
    // A positive shift moves the first items to the end: {a, b, c} by 1 is {b, c, a}.
    if wrap {
        let k = (((offset % n) + n) % n) as usize;
        items.rotate_left(k);
    } else if offset >= 0 {
        items.drain(..(offset.min(n)) as usize);
    } else {
        let keep = (n + offset).max(0) as usize;
        items.truncate(keep);
    }
    Ok(Outputs::one("out", items))
}

fn split(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let n = items.len() as i64;
    let idx = i.int("index")?;
    let at = (if idx < 0 { n + idx } else { idx }).clamp(0, n) as usize;
    Ok(Outputs::one("a", items[..at].to_vec()).with("b", items[at..].to_vec()))
}

fn sort_keys(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let keys = i.list("keys");
    let values = i.list("values");
    let mut order: Vec<usize> = (0..keys.len()).collect();
    let all_numbers = keys.iter().all(|k| k.as_number().is_some());
    let all_text = keys.iter().all(|k| matches!(k, Value::Text(_)));
    if all_numbers {
        order.sort_by(|a, b| keys[*a].as_number().unwrap().total_cmp(&keys[*b].as_number().unwrap()));
    } else if all_text {
        order.sort_by(|a, b| keys[*a].as_text().unwrap().cmp(keys[*b].as_text().unwrap()));
    } else {
        return Err(NodeError::input("keys", "keys must be all numbers or all text"));
    }
    let sorted_keys: Vec<Value> = order.iter().map(|k| keys[*k].clone()).collect();
    // Values shorter than the keys repeat their last, as an item pin would.
    let sorted_values: Vec<Value> = if values.is_empty() { Vec::new() } else { order.iter().map(|k| values[(*k).min(values.len() - 1)].clone()).collect() };
    Ok(Outputs::one("keys", sorted_keys).with("values", sorted_values))
}

fn repeat(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let count = i.int("count")?;
    if count < 0 {
        return Err(NodeError::input("count", format!("{count} is negative")));
    }
    let count = (count as usize).min(MAX_LIST_ITEMS);
    if items.is_empty() {
        return Ok(Outputs::one("out", Vec::<Value>::new()));
    }
    let out: Vec<Value> = (0..count).map(|k| items[k % items.len()].clone()).collect();
    Ok(Outputs::one("out", out))
}

fn gate(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let open = i.bool("open")?;
    // Closed, the gate emits an empty list: everything below runs zero
    // times, which is what "nothing" means in this graph.
    Ok(Outputs::one("out", if open { i.get("value").clone() } else { Value::List(Vec::new()) }))
}

fn polar_array(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let count = i.int("count")?;
    if count < 1 {
        return Err(NodeError::input("count", format!("{count} is not a positive count")));
    }
    let count = (count as usize).min(MAX_LIST_ITEMS);
    let start = i.number("start_deg")?;
    let span = i.number("span_deg")?;
    // A full turn divides exactly into `count` stations with no duplicate
    // at the joint; a partial arc includes both ends.
    let full = (span.abs() - 360.0).abs() < 1e-9;
    let step = if full { span / count as f64 } else if count > 1 { span / (count - 1) as f64 } else { 0.0 };
    let angles: Vec<Value> = (0..count).map(|k| Value::Number(start + step * k as f64)).collect();
    Ok(Outputs::one("angles", angles).with("step_deg", step))
}

fn format(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let template = i.text("template")?.to_string();
    let mut out = template.clone();
    for (k, pin) in ["a", "b", "c", "d"].iter().enumerate() {
        let v = i.get(pin);
        let text = match v {
            Value::Null => String::new(),
            Value::Text(s) | Value::AlphaRef(s) => s.clone(),
            other => ValueKind::Text.coerce(other.clone()).ok().and_then(|t| t.as_text().map(str::to_string)).unwrap_or_else(|| other.summary()),
        };
        out = out.replace(&format!("{{{k}}}"), &text).replace(&format!("{{{pin}}}"), &text);
    }
    Ok(Outputs::one("out", out))
}

fn json_get(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let json = i.get("json").to_json_any().ok_or_else(|| NodeError::input("json", "no JSON form"))?;
    let pointer = i.text("pointer")?;
    let v = if pointer.is_empty() { Some(&json) } else { json.pointer(pointer) };
    Ok(Outputs::one("value", v.cloned().ok_or_else(|| NodeError::input("pointer", format!("nothing at {pointer:?}")))?))
}

fn json_set(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut json = i.get("json").to_json_any().ok_or_else(|| NodeError::input("json", "no JSON form"))?;
    let pointer = i.text("pointer")?;
    let value = i.get("value").to_json_any().ok_or_else(|| NodeError::input("value", "no JSON form"))?;
    set_pointer(&mut json, pointer, value).map_err(|m| NodeError::input("pointer", m))?;
    Ok(Outputs::one("json", json))
}

fn if_(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let c = i.bool("cond")?;
    Ok(Outputs::one("out", if c { i.get("a").clone() } else { i.get("b").clone() }))
}

pub fn register(reg: &mut Registry) {
    let items = || PinSpec::list("items", ValueKind::Any).doc("The list.");
    let specs = [
        NodeSpec::new("list.weave", "Weave", Category::Util)
            .doc("Interleave up to four streams by a pattern of stream numbers (0..3), repeating it. An exhausted stream's slots are skipped, so every item comes out exactly once.")
            .input(PinSpec::list("pattern", ValueKind::Int).doc("Stream numbers, e.g. 0,1,1."))
            .input(PinSpec::list("s0", ValueKind::Any).doc("Stream 0."))
            .input(PinSpec::list("s1", ValueKind::Any).doc("Stream 1."))
            .input(PinSpec::list("s2", ValueKind::Any).doc("Stream 2."))
            .input(PinSpec::list("s3", ValueKind::Any).doc("Stream 3."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The woven list."))
            .eval(weave),
        NodeSpec::new("list.entwine", "Entwine", Category::Util)
            .doc("Up to four lists as the branches of one list of lists.")
            .input(PinSpec::list("a", ValueKind::Any).doc("Branch 0."))
            .input(PinSpec::list("b", ValueKind::Any).doc("Branch 1."))
            .input(PinSpec::list("c", ValueKind::Any).doc("Branch 2."))
            .input(PinSpec::list("d", ValueKind::Any).doc("Branch 3."))
            .output(PinSpec::list("out", ValueKind::Any).doc("A list of the four lists."))
            .eval(entwine),
        NodeSpec::new("list.cull_pattern", "Cull pattern", Category::Util)
            .doc("Keep the items where a repeating boolean pattern is true.")
            .input(items())
            .input(PinSpec::list("pattern", ValueKind::Bool).doc("True keeps, false culls; repeats along the list."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The kept items."))
            .eval(cull_pattern),
        NodeSpec::new("list.cull_index", "Cull index", Category::Util)
            .doc("Drop the items at these indices; wrapping, a negative index counts from the end.")
            .input(items())
            .input(PinSpec::list("indices", ValueKind::Int).doc("Indices to drop."))
            .input(PinSpec::item("wrap", ValueKind::Bool).default(true).doc("Whether indices wrap round the list."))
            .output(PinSpec::list("out", ValueKind::Any).doc("What is left."))
            .eval(cull_index),
        NodeSpec::new("list.cull_nth", "Cull nth", Category::Util)
            .doc("Drop every n-th item (the n-th, 2n-th, …).")
            .input(items())
            .input(PinSpec::item("n", ValueKind::Int).default(2i64).doc("Which items go."))
            .output(PinSpec::list("out", ValueKind::Any).doc("What is left."))
            .eval(cull_nth),
        NodeSpec::new("list.partition", "Partition", Category::Util)
            .doc("Cut a list into chunks of these sizes, the size list repeating; the last chunk keeps what is left.")
            .input(items())
            .input(PinSpec::list("size", ValueKind::Int).doc("Chunk sizes, e.g. 3 or 2,3."))
            .output(PinSpec::list("out", ValueKind::Any).doc("A list of chunks."))
            .eval(partition),
        NodeSpec::new("list.dispatch", "Dispatch", Category::Util)
            .doc("Send each item to A where a repeating pattern is true and to B where it is false.")
            .input(items())
            .input(PinSpec::list("pattern", ValueKind::Bool).doc("True to A, false to B; repeats."))
            .output(PinSpec::list("a", ValueKind::Any).doc("The true items."))
            .output(PinSpec::list("b", ValueKind::Any).doc("The false items."))
            .eval(dispatch),
        NodeSpec::new("list.shift", "Shift", Category::Util)
            .doc("Rotate a list: a positive offset moves the first items to the end; without wrap they are dropped instead.")
            .input(items())
            .input(PinSpec::item("offset", ValueKind::Int).default(1i64).doc("How far."))
            .input(PinSpec::item("wrap", ValueKind::Bool).default(true).doc("Rotate rather than drop."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The shifted list."))
            .eval(shift),
        NodeSpec::new("list.split", "Split", Category::Util)
            .doc("The list before an index and from it on; a negative index counts from the end.")
            .input(items())
            .input(PinSpec::item("index", ValueKind::Int).default(1i64).doc("Where to cut."))
            .output(PinSpec::list("a", ValueKind::Any).doc("Before the index."))
            .output(PinSpec::list("b", ValueKind::Any).doc("From the index on."))
            .eval(split),
        NodeSpec::new("list.sort_keys", "Sort by keys", Category::Util)
            .doc("Sort keys (numbers or text) and carry the values along.")
            .input(PinSpec::list("keys", ValueKind::Any).doc("The keys."))
            .input(PinSpec::list("values", ValueKind::Any).doc("Values in the keys' order."))
            .output(PinSpec::list("keys", ValueKind::Any).doc("The sorted keys."))
            .output(PinSpec::list("values", ValueKind::Any).doc("The values, reordered."))
            .eval(sort_keys),
        NodeSpec::new("list.repeat", "Repeat", Category::Util)
            .doc("Repeat a list's items until it is this long.")
            .input(items())
            .input(PinSpec::item("count", ValueKind::Int).default(10i64).doc("The length wanted; capped at the list limit."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The repeated list."))
            .eval(repeat),
        NodeSpec::new("stream.gate", "Gate", Category::Util)
            .doc("Pass a value through while open; closed, emit an empty list so everything below runs zero times.")
            .input(PinSpec::item("value", ValueKind::Any).doc("What passes."))
            .input(PinSpec::item("open", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Open or closed."))
            .output(PinSpec::item("out", ValueKind::Any).doc("The value, or nothing."))
            .eval(gate),
        NodeSpec::new("polar.array", "Polar array", Category::Util)
            .doc("Angles round the ring on the integer lattice: a full turn divides exactly into count stations with no duplicate at the joint; a partial arc includes both ends. Never a relaxation.")
            .input(PinSpec::item("count", ValueKind::Int).default(12i64).doc("Stations."))
            .input(PinSpec::item("start_deg", ValueKind::Number).default(90.0).widget(Widget::Angle).doc("The first station; 90° is the top."))
            .input(PinSpec::item("span_deg", ValueKind::Number).default(360.0).widget(Widget::Slider { min: -360.0, max: 360.0 }).doc("The arc covered; 360 is the whole ring."))
            .output(PinSpec::list("angles", ValueKind::Number).doc("The station angles, degrees."))
            .output(PinSpec::item("step_deg", ValueKind::Number).doc("The angle between stations."))
            .eval(polar_array),
        NodeSpec::new("text.format", "Format", Category::Util)
            .doc("Fill {0}..{3} (or {a}..{d}) in a template with values written out.")
            .input(PinSpec::item("template", ValueKind::Text).default("{0}").widget(Widget::TextLine).doc("The template."))
            .input(PinSpec::item("a", ValueKind::Any).optional().doc("{0}"))
            .input(PinSpec::item("b", ValueKind::Any).optional().doc("{1}"))
            .input(PinSpec::item("c", ValueKind::Any).optional().doc("{2}"))
            .input(PinSpec::item("d", ValueKind::Any).optional().doc("{3}"))
            .output(PinSpec::item("out", ValueKind::Text).doc("The text."))
            .eval(format),
        NodeSpec::new("json.get", "JSON get", Category::Util)
            .doc("A field of a JSON value (or any serializable handle) by RFC 6901 pointer.")
            .input(PinSpec::item("json", ValueKind::Any).doc("The value."))
            .input(PinSpec::item("pointer", ValueKind::Text).default("").widget(Widget::TextLine).doc("The pointer; empty for the whole."))
            .output(PinSpec::item("value", ValueKind::Json).doc("The field."))
            .eval(json_get),
        NodeSpec::new("json.set", "JSON set", Category::Util)
            .doc("A JSON value with a field set at a pointer, objects created on the way.")
            .input(PinSpec::item("json", ValueKind::Any).doc("The value."))
            .input(PinSpec::item("pointer", ValueKind::Text).default("").widget(Widget::TextLine).doc("The pointer."))
            .input(PinSpec::item("value", ValueKind::Any).doc("The new field."))
            .output(PinSpec::item("json", ValueKind::Json).doc("The changed value."))
            .eval(json_set),
        NodeSpec::new("flow.if", "If", Category::Util)
            .doc("a when the condition holds, else b — per item.")
            .input(PinSpec::item("cond", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("The condition."))
            .input(PinSpec::item("a", ValueKind::Any).optional().doc("When true."))
            .input(PinSpec::item("b", ValueKind::Any).optional().doc("When false."))
            .output(PinSpec::item("out", ValueKind::Any).doc("a or b."))
            .eval(if_),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
    let _ = Arc::new(0);
}

#[cfg(test)]
mod tests {
    use crate::eval::{Evaluator, Targets};
    use crate::graph::{Graph, NodeId};
    use crate::registry::Registry;
    use crate::value::{Literal, Value};
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::default(), 0, Targets::AllPure)
    }

    fn texts(xs: &[&str]) -> Literal {
        Literal::List(xs.iter().map(|s| Literal::Text(s.to_string())).collect())
    }
    fn ints(xs: &[i64]) -> Literal {
        Literal::List(xs.iter().map(|x| Literal::Int(*x)).collect())
    }
    fn bools(xs: &[bool]) -> Literal {
        Literal::List(xs.iter().map(|x| Literal::Bool(*x)).collect())
    }
    fn as_texts(v: Option<&Value>) -> Vec<String> {
        match v {
            Some(Value::List(items)) => items.iter().map(|x| x.summary().trim_matches('"').to_string()).collect(),
            other => panic!("{other:?}"),
        }
    }
    fn node(g: &mut Graph, kind: &str, pins: &[(&str, Literal)]) -> NodeId {
        let id = g.add(kind).unwrap();
        for (k, v) in pins {
            g.set_input(id, *k, v.clone()).unwrap();
        }
        id
    }

    #[test]
    fn weave_entwine_cull_partition_and_dispatch() {
        let mut g = Graph::default();
        let w = node(&mut g, "list.weave", &[("pattern", ints(&[0, 1, 1])), ("s0", texts(&["a", "b", "c", "d", "e", "f"])), ("s1", texts(&["x", "y", "z"]))]);
        let w2 = node(&mut g, "list.weave", &[("pattern", ints(&[0, 1])), ("s0", texts(&["a", "b", "c"])), ("s1", texts(&["x"]))]);
        let e = node(&mut g, "list.entwine", &[("a", texts(&["a", "b"])), ("b", texts(&["x", "y", "z"]))]);
        let cp = node(&mut g, "list.cull_pattern", &[("items", texts(&["a", "b", "c", "d", "e", "f", "g"])), ("pattern", bools(&[true, false, false]))]);
        let ci = node(&mut g, "list.cull_index", &[("items", texts(&["a", "b", "c", "d", "e", "f", "g"])), ("indices", ints(&[0, -1, 9]))]);
        let ci_strict = node(&mut g, "list.cull_index", &[("items", texts(&["a", "b", "c"])), ("indices", ints(&[5])), ("wrap", Literal::Bool(false))]);
        let cn = node(&mut g, "list.cull_nth", &[("items", texts(&["a", "b", "c", "d", "e", "f", "g"])), ("n", Literal::Int(2))]);
        let p1 = node(&mut g, "list.partition", &[("items", ints(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])), ("size", ints(&[3]))]);
        let p2 = node(&mut g, "list.partition", &[("items", ints(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])), ("size", ints(&[2, 3]))]);
        let d = node(&mut g, "list.dispatch", &[("items", texts(&["a", "b", "c", "d"])), ("pattern", bools(&[true, false]))]);
        let r = run(&g);
        assert!(!r.any_failed() || r.status[&ci_strict].failed(), "{:?}", r.notes(&g));
        assert_eq!(as_texts(r.value(w, "out")), ["a", "x", "y", "b", "z", "c", "d", "e", "f"]);
        assert_eq!(as_texts(r.value(w2, "out")), ["a", "x", "b", "c"], "an exhausted stream's slots are skipped");
        match r.value(e, "out") {
            Some(Value::List(b)) => {
                assert_eq!(b.len(), 4);
                assert_eq!(as_texts(Some(&b[1])), ["x", "y", "z"]);
                assert_eq!(b[3], Value::List(vec![]));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(as_texts(r.value(cp, "out")), ["a", "d", "g"]);
        assert_eq!(as_texts(r.value(ci, "out")), ["b", "d", "e", "f"], "0, the last, and 9 mod 7 = 2 went");
        assert!(r.status[&ci_strict].errors[0].1.contains("outside"));
        assert_eq!(as_texts(r.value(cn, "out")), ["a", "c", "e", "g"]);
        let chunks = |v: Option<&Value>| -> Vec<usize> { match v { Some(Value::List(c)) => c.iter().map(|x| x.as_list().map_or(0, |l| l.len())).collect(), _ => vec![] } };
        assert_eq!(chunks(r.value(p1, "out")), [3, 3, 3, 1]);
        assert_eq!(chunks(r.value(p2, "out")), [2, 3, 2, 3]);
        assert_eq!(as_texts(r.value(d, "a")), ["a", "c"]);
        assert_eq!(as_texts(r.value(d, "b")), ["b", "d"]);
    }

    #[test]
    fn shift_split_sort_repeat_gate_and_polar_array() {
        let mut g = Graph::default();
        let sh = node(&mut g, "list.shift", &[("items", texts(&["a", "b", "c"])), ("offset", Literal::Int(1))]);
        let shm = node(&mut g, "list.shift", &[("items", texts(&["a", "b", "c"])), ("offset", Literal::Int(-1))]);
        let shd = node(&mut g, "list.shift", &[("items", texts(&["a", "b", "c"])), ("offset", Literal::Int(1)), ("wrap", Literal::Bool(false))]);
        let sp = node(&mut g, "list.split", &[("items", texts(&["a", "b", "c", "d", "e"])), ("index", Literal::Int(2))]);
        let so = node(&mut g, "list.sort_keys", &[("keys", ints(&[3, 1, 2])), ("values", texts(&["c", "a", "b"]))]);
        let sot = node(&mut g, "list.sort_keys", &[("keys", texts(&["b", "a", "c"]))]);
        let rp = node(&mut g, "list.repeat", &[("items", texts(&["a", "b"])), ("count", Literal::Int(5))]);
        let open = node(&mut g, "stream.gate", &[("value", Literal::Number(7.0))]);
        let closed = node(&mut g, "stream.gate", &[("value", Literal::Number(7.0)), ("open", Literal::Bool(false))]);
        let below = g.add("math.add").unwrap();
        g.connect(closed, "out", below, "a").unwrap();
        let full = node(&mut g, "polar.array", &[("count", Literal::Int(4))]);
        let part = node(&mut g, "polar.array", &[("count", Literal::Int(3)), ("start_deg", Literal::Number(0.0)), ("span_deg", Literal::Number(180.0))]);
        let one = node(&mut g, "polar.array", &[("count", Literal::Int(1))]);
        let r = run(&g);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(as_texts(r.value(sh, "out")), ["b", "c", "a"]);
        assert_eq!(as_texts(r.value(shm, "out")), ["c", "a", "b"]);
        assert_eq!(as_texts(r.value(shd, "out")), ["b", "c"]);
        assert_eq!(as_texts(r.value(sp, "a")), ["a", "b"]);
        assert_eq!(as_texts(r.value(sp, "b")), ["c", "d", "e"]);
        assert_eq!(r.value(so, "keys"), Some(&Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])));
        assert_eq!(as_texts(r.value(so, "values")), ["a", "b", "c"]);
        assert_eq!(as_texts(r.value(sot, "keys")), ["a", "b", "c"]);
        assert_eq!(as_texts(r.value(rp, "out")), ["a", "b", "a", "b", "a"]);
        assert_eq!(r.value(open, "out"), Some(&Value::Number(7.0)));
        assert_eq!(r.value(closed, "out"), Some(&Value::List(vec![])));
        assert_eq!(r.status[&below].items, 0, "nothing below a closed gate runs");
        let nums = |v: Option<&Value>| -> Vec<f64> { match v { Some(Value::List(a)) => a.iter().map(|x| x.as_number().unwrap()).collect(), _ => vec![] } };
        assert_eq!(nums(r.value(full, "angles")), [90.0, 180.0, 270.0, 360.0], "a full turn: count stations, none repeated at the joint");
        assert_eq!(nums(r.value(part, "angles")), [0.0, 90.0, 180.0], "a partial arc includes both ends");
        assert_eq!(nums(r.value(one, "angles")), [90.0]);
    }

    #[test]
    fn format_json_and_if() {
        let mut g = Graph::default();
        let f = node(&mut g, "text.format", &[("template", Literal::Text("{0} x {b} = {2}".into())), ("a", Literal::Number(2.0)), ("b", Literal::Int(3)), ("c", Literal::Number(6.0))]);
        let jg = node(&mut g, "json.get", &[("json", Literal::Json(serde_json::json!({"a": {"b": [1, 2]}}))), ("pointer", Literal::Text("/a/b/1".into()))]);
        let js = node(&mut g, "json.set", &[("json", Literal::Json(serde_json::json!({"a": 1}))), ("pointer", Literal::Text("/c/d".into())), ("value", Literal::Text("x".into()))]);
        let i1 = node(&mut g, "flow.if", &[("cond", bools(&[true, false, true])), ("a", Literal::Text("yes".into())), ("b", Literal::Text("no".into()))]);
        let p = g.add("band.profile").unwrap();
        let jp = g.add("json.get").unwrap();
        g.connect(p, "profile", jp, "json").unwrap();
        g.set_input(jp, "pointer", Literal::Text("/width_mm".into())).unwrap();
        let r = run(&g);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(r.value(f, "out"), Some(&Value::Text("2 x 3 = 6".into())));
        assert_eq!(r.value(jg, "value"), Some(&Value::Json(std::sync::Arc::new(serde_json::json!(2)))));
        assert_eq!(r.value(js, "json"), Some(&Value::Json(std::sync::Arc::new(serde_json::json!({"a": 1, "c": {"d": "x"}})))));
        assert_eq!(as_texts(r.value(i1, "out")), ["yes", "no", "yes"]);
        assert!(matches!(r.value(jp, "value"), Some(Value::Json(j)) if j.as_f64().is_some()), "a handle reads as JSON");
    }
}
