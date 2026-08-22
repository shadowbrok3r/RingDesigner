//! Text utilities.

use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry};
use crate::value::ValueKind;

fn concat(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let sep = i.text("sep")?;
    Ok(Outputs::one("out", format!("{}{sep}{}", i.text("a")?, i.text("b")?)))
}

fn join(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let sep = i.text("sep")?.to_string();
    let parts: Result<Vec<String>, NodeError> = i
        .list("items")
        .iter()
        .enumerate()
        .map(|(k, v)| match ValueKind::Text.coerce(v.clone()) {
            Ok(t) => Ok(t.as_text().unwrap_or("").to_string()),
            Err(e) => Err(NodeError::input("items", format!("item {k}: {e}"))),
        })
        .collect();
    Ok(Outputs::one("out", parts?.join(&sep)))
}

pub fn register(reg: &mut Registry) {
    let specs = [
        NodeSpec::new("text.concat", "Concat", Category::Util)
            .doc("a and b joined, with a separator between.")
            .input(PinSpec::item("a", ValueKind::Text).default("").doc("First."))
            .input(PinSpec::item("b", ValueKind::Text).default("").doc("Second."))
            .input(PinSpec::item("sep", ValueKind::Text).default("").doc("Put between them."))
            .output(PinSpec::item("out", ValueKind::Text).doc("The joined text."))
            .eval(concat),
        NodeSpec::new("text.join", "Join", Category::Util)
            .doc("A list joined into one text with a separator; numbers are written out.")
            .input(PinSpec::list("items", ValueKind::Any).doc("The parts."))
            .input(PinSpec::item("sep", ValueKind::Text).default(", ").doc("Put between them."))
            .output(PinSpec::item("out", ValueKind::Text).doc("The joined text."))
            .eval(join),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}
