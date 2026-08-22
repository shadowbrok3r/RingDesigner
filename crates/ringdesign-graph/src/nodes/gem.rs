//! Stones as stock: the gem record the seats and reports read.

use ringdesign_core::gem::{Gem, GemCut, GemForm};

use super::structs::{StructNode, enum_names};
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn parse_cut(name: &str) -> Result<GemCut, NodeError> {
    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(|_| NodeError::input("cut", format!("{name:?} is not a gem cut")))
}

fn parse_form(name: &str) -> Result<GemForm, NodeError> {
    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(|_| NodeError::input("form", format!("{name:?} is not a gem form")))
}

fn calibrated(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let cut = parse_cut(i.text("cut")?)?;
    let w = i.number("w_mm")?;
    if !(0.5..=30.0).contains(&w) {
        return Err(NodeError::input("w_mm", format!("{w} mm is not a stone width between 0.5 and 30")));
    }
    let gem = match parse_form(i.text("form")?)? {
        GemForm::Faceted => Gem::calibrated(cut, w),
        GemForm::Cabochon => Gem::cabochon(cut, w),
    };
    Ok(Outputs::one("gem", gem).with("l_mm", gem.l_mm).with("carats", gem.carats()))
}

fn gem_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("gem", "Gem", Category::Layer).doc("A stone by its record: cut, width, length and form. gem.calibrated fills the length from the cut's own aspect."),
        "gem",
        Gem::default,
        Value::Gem,
        |v| match v {
            Value::Gem(g) => Some(*g),
            _ => None,
        },
    )
    .base("gem", ValueKind::Gem, "Start from this stone.")
    .field(PinSpec::select("cut", enum_names(GemCut::ALL)).doc("The cut."))
    .field(PinSpec::item("w_mm", ValueKind::Number).widget(Widget::Mm { min: 0.5, max: 30.0 }).doc("Width (the short axis), mm."))
    .field(PinSpec::item("l_mm", ValueKind::Number).widget(Widget::Mm { min: 0.5, max: 40.0 }).doc("Length (the long axis), mm."))
    .field(PinSpec::select("form", enum_names(GemForm::ALL)).doc("Faceted, or a flat-backed cabochon."))
    .build()
}

pub fn register(reg: &mut Registry) {
    let specs = [
        gem_node(),
        NodeSpec::new("gem.calibrated", "Calibrated gem", Category::Layer)
            .doc("A stone of a cut and width with the length the cut's calibrated aspect gives it, faceted or cabochon.")
            .input(PinSpec::select("cut", enum_names(GemCut::ALL)).default("Round").doc("The cut."))
            .input(PinSpec::item("w_mm", ValueKind::Number).default(3.0).widget(Widget::Mm { min: 0.5, max: 30.0 }).doc("Width, mm."))
            .input(PinSpec::select("form", enum_names(GemForm::ALL)).default("Faceted").doc("Faceted, or a flat-backed cabochon."))
            .output(PinSpec::item("gem", ValueKind::Gem).doc("The stone."))
            .output(PinSpec::item("l_mm", ValueKind::Number).doc("Its length, mm."))
            .output(PinSpec::item("carats", ValueKind::Number).doc("Estimated weight, ct."))
            .eval(calibrated),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}
