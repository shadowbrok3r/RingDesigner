//! Basic list utilities. Every `items` pin is list-access: the node sees
//! the whole list once, rather than running per item.

use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry};
use crate::value::{Value, ValueKind};

fn length(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("n", i.list("items").len() as i64))
}

fn item(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let index = i.int("index")?;
    let wrap = i.bool("wrap")?;
    if items.is_empty() {
        return Err(NodeError::input("items", "the list is empty"));
    }
    let n = items.len() as i64;
    let k = if wrap { ((index % n) + n) % n } else if (0..n).contains(&index) { index } else {
        return Err(NodeError::input("index", format!("{index} is outside 0..{n}")));
    };
    Ok(Outputs::one("item", items[k as usize].clone()))
}

fn flatten_into(v: Value, out: &mut Vec<Value>) {
    match v {
        Value::List(items) => items.into_iter().for_each(|x| flatten_into(x, out)),
        other => out.push(other),
    }
}

fn flatten(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut out = Vec::new();
    for v in i.list("items") {
        flatten_into(v, &mut out);
    }
    Ok(Outputs::one("out", out))
}

fn graft(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let out: Vec<Value> = i.list("items").into_iter().map(|v| Value::List(vec![v])).collect();
    Ok(Outputs::one("out", out))
}

fn reverse(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut items = i.list("items");
    items.reverse();
    Ok(Outputs::one("out", items))
}

fn slice(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let n = items.len() as i64;
    let norm = |k: i64| -> usize { (if k < 0 { n + k } else { k }).clamp(0, n) as usize };
    let start = norm(i.int("start")?);
    let end = norm(i.int("end")?);
    let out: Vec<Value> = if end > start { items[start..end].to_vec() } else { Vec::new() };
    Ok(Outputs::one("out", out))
}

fn sort(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut items = i.list("items");
    let descending = i.bool("descending")?;
    let all_numbers = items.iter().all(|v| v.as_number().is_some());
    let all_text = items.iter().all(|v| matches!(v, Value::Text(_)));
    if all_numbers {
        items.sort_by(|a, b| a.as_number().unwrap().total_cmp(&b.as_number().unwrap()));
    } else if all_text {
        items.sort_by(|a, b| a.as_text().unwrap().cmp(b.as_text().unwrap()));
    } else {
        return Err(NodeError::input("items", "sort needs all numbers or all text"));
    }
    if descending {
        items.reverse();
    }
    Ok(Outputs::one("out", items))
}

fn merge(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut out = Vec::new();
    for pin in ["a", "b", "c", "d"] {
        out.extend(i.list(pin));
    }
    Ok(Outputs::one("out", out))
}

fn sum(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let items = i.list("items");
    let mut total = 0.0;
    for (k, v) in items.iter().enumerate() {
        total += v.as_number().ok_or_else(|| NodeError::input("items", format!("item {k} is not a number")))?;
    }
    Ok(Outputs::one("sum", total).with("mean", if items.is_empty() { 0.0 } else { total / items.len() as f64 }))
}

pub fn register(reg: &mut Registry) {
    let items = || PinSpec::list("items", ValueKind::Any).doc("The list.");
    let specs = [
        NodeSpec::new("list.length", "Length", Category::Util)
            .doc("How many items a list has.")
            .input(items())
            .output(PinSpec::item("n", ValueKind::Int).doc("The count."))
            .eval(length),
        NodeSpec::new("list.item", "Item", Category::Util)
            .doc("One item of a list by index; wrapping, a negative index counts from the end.")
            .input(items())
            .input(PinSpec::item("index", ValueKind::Int).default(0i64).doc("Which item."))
            .input(PinSpec::item("wrap", ValueKind::Bool).default(true).doc("Whether the index wraps round the list."))
            .output(PinSpec::item("item", ValueKind::Any).doc("The item."))
            .eval(item),
        NodeSpec::new("list.flatten", "Flatten", Category::Util)
            .doc("Every nested list opened into one flat list.")
            .input(items())
            .output(PinSpec::list("out", ValueKind::Any).doc("The flat list."))
            .eval(flatten),
        NodeSpec::new("list.graft", "Graft", Category::Util)
            .doc("Each item wrapped in its own one-item list, so the next node sees one branch per item.")
            .input(items())
            .output(PinSpec::list("out", ValueKind::Any).doc("The grafted list."))
            .eval(graft),
        NodeSpec::new("list.reverse", "Reverse", Category::Util)
            .doc("The list back to front.")
            .input(items())
            .output(PinSpec::list("out", ValueKind::Any).doc("The reversed list."))
            .eval(reverse),
        NodeSpec::new("list.slice", "Slice", Category::Util)
            .doc("Items from start up to (not including) end; a negative index counts from the end.")
            .input(items())
            .input(PinSpec::item("start", ValueKind::Int).default(0i64).doc("First index kept."))
            .input(PinSpec::item("end", ValueKind::Int).default(-1i64).doc("First index dropped; −1 is the last item."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The slice."))
            .eval(slice),
        NodeSpec::new("list.sort", "Sort", Category::Util)
            .doc("Numbers by value or text alphabetically; a mixed list is refused.")
            .input(items())
            .input(PinSpec::item("descending", ValueKind::Bool).default(false).doc("Largest first."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The sorted list."))
            .eval(sort),
        NodeSpec::new("list.merge", "Merge", Category::Util)
            .doc("Up to four lists (or single values) joined end to end.")
            .input(PinSpec::list("a", ValueKind::Any).doc("First."))
            .input(PinSpec::list("b", ValueKind::Any).doc("Second."))
            .input(PinSpec::list("c", ValueKind::Any).doc("Third."))
            .input(PinSpec::list("d", ValueKind::Any).doc("Fourth."))
            .output(PinSpec::list("out", ValueKind::Any).doc("The joined list."))
            .eval(merge),
        NodeSpec::new("list.sum", "Sum", Category::Util)
            .doc("The total and the mean of a list of numbers.")
            .input(items())
            .output(PinSpec::item("sum", ValueKind::Number).doc("The total."))
            .output(PinSpec::item("mean", ValueKind::Number).doc("The mean; 0 for an empty list."))
            .eval(sum),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}
