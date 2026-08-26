//! Literal sources and number generators.

use crate::MAX_LIST_ITEMS;
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn number(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", i.number("value")?))
}

fn int(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", i.int("value")?))
}

fn bool_(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", i.bool("value")?))
}

fn text(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", i.text("value")?.to_string()))
}

/// A count for a generated list, clamped to the cap with a warning.
fn capped_count(ctx: &mut EvalCtx<'_>, i: &Inputs, pin: &str) -> Result<usize, NodeError> {
    let n = i.int(pin)?;
    if n < 0 {
        ctx.warn(format!("{pin} {n} is negative: nothing generated"));
        return Ok(0);
    }
    let n = n as usize;
    if n > MAX_LIST_ITEMS {
        ctx.warn(format!("{pin} {n} clamped to {MAX_LIST_ITEMS}"));
        return Ok(MAX_LIST_ITEMS);
    }
    Ok(n)
}

fn series(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let start = i.number("start")?;
    let step = i.number("step")?;
    let count = capped_count(ctx, i, "count")?;
    let out: Vec<Value> = (0..count).map(|k| Value::Number(start + step * k as f64)).collect();
    Ok(Outputs::one("out", out))
}

fn range(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let start = i.number("start")?;
    let end = i.number("end")?;
    let count = capped_count(ctx, i, "count")?;
    let out: Vec<Value> = match count {
        0 => Vec::new(),
        1 => vec![Value::Number(start)],
        n => (0..n).map(|k| Value::Number(start + (end - start) * k as f64 / (n - 1) as f64)).collect(),
    };
    Ok(Outputs::one("out", out))
}

pub fn register(reg: &mut Registry) {
    let specs = [
        NodeSpec::new("number", "Number", Category::Source)
            .doc("A number, or a list of them.")
            .input(PinSpec::item("value", ValueKind::Number).default(0.0).doc("The value."))
            .output(PinSpec::item("out", ValueKind::Number).doc("The value, unchanged."))
            .eval(number),
        NodeSpec::new("int", "Integer", Category::Source)
            .doc("A whole number: a count, a repeat, an index.")
            .input(PinSpec::item("value", ValueKind::Int).default(1i64).doc("The value."))
            .output(PinSpec::item("out", ValueKind::Int).doc("The value, unchanged."))
            .eval(int),
        NodeSpec::new("bool", "Boolean", Category::Source)
            .doc("True or false.")
            .input(PinSpec::item("value", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("The value."))
            .output(PinSpec::item("out", ValueKind::Bool).doc("The value, unchanged."))
            .eval(bool_),
        NodeSpec::new("text", "Text", Category::Source)
            .doc("A piece of text: a name, an inscription, an enum choice.")
            .input(PinSpec::item("value", ValueKind::Text).default("").widget(Widget::TextLine).doc("The text."))
            .output(PinSpec::item("out", ValueKind::Text).doc("The text, unchanged."))
            .eval(text),
        NodeSpec::new("series", "Series", Category::Source)
            .doc("Count numbers from a start by a step: 0, 30, 60 … for stations round the ring.")
            .input(PinSpec::item("start", ValueKind::Number).default(0.0).doc("The first number."))
            .input(PinSpec::item("step", ValueKind::Number).default(1.0).doc("Added for each next number."))
            .input(PinSpec::item("count", ValueKind::Int).default(10i64).doc("How many; capped at the list limit."))
            .output(PinSpec::list("out", ValueKind::Number).doc("The numbers."))
            .eval(series),
        NodeSpec::new("range", "Range", Category::Source)
            .doc("Count evenly spaced numbers from a start to an end, both included. Grasshopper's Range counts steps, one fewer.")
            .input(PinSpec::item("start", ValueKind::Number).default(0.0).doc("The first number."))
            .input(PinSpec::item("end", ValueKind::Number).default(1.0).doc("The last number."))
            .input(PinSpec::item("count", ValueKind::Int).default(10i64).doc("How many; capped at the list limit. One gives the start alone."))
            .output(PinSpec::list("out", ValueKind::Number).doc("The numbers."))
            .eval(range),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}
