//! Stacks, and the design as the thing that carries them.

use std::sync::Arc;

use ringdesign_core::{LayerStack, RingDesign};

use crate::graph::{Node, set_pointer};
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{AlphaSource, Value, ValueKind};

fn design_of(i: &Inputs, pin: &str) -> Result<RingDesign, NodeError> {
    match i.get(pin) {
        Value::Design(d) => Ok((**d).clone()),
        other => Err(NodeError::input(pin, format!("expected a design, got {}", other.summary()))),
    }
}

fn stack(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut s = match i.get("stack") {
        Value::Stack(s) => (**s).clone(),
        Value::Null => LayerStack::default(),
        other => return Err(NodeError::input("stack", format!("expected a stack, got {}", other.summary()))),
    };
    for (k, v) in i.list("entries").into_iter().enumerate() {
        match v {
            Value::Entry(e) => s.layers.push((*e).clone()),
            Value::Stack(inner) => s.layers.extend(inner.layers.iter().cloned()),
            Value::Null => return Err(NodeError::input("entries", format!("item {k} failed upstream"))),
            other => return Err(NodeError::input("entries", format!("item {k} is {}, not an entry", other.summary()))),
        }
    }
    Ok(Outputs::one("stack", Value::Stack(Arc::new(s))))
}

/// Put a source into the design, replacing a same-named one.
fn adopt_source(d: &mut RingDesign, src: &AlphaSource) {
    match src {
        AlphaSource::Procedural(p) => {
            d.recipes.retain(|r| r.name != p.name);
            d.recipes.push(p.clone());
        }
        AlphaSource::Text(t) => {
            d.texts.retain(|x| x.name != t.name);
            d.texts.push(t.clone());
        }
        AlphaSource::Svg(s) => {
            d.svgs.retain(|x| x.name != s.name);
            d.svgs.push(s.clone());
        }
        AlphaSource::Drawn(a) => {
            d.drawn.retain(|x| x.name != a.name);
            d.drawn.push(a.clone());
        }
        AlphaSource::Embedded(e) => {
            d.embedded.retain(|x| x.name != e.name);
            d.embedded.push(e.clone());
        }
    }
}

fn assemble(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut d = design_of(i, "design")?;
    match i.get("stack") {
        Value::Stack(s) => d.layers = (**s).clone(),
        Value::Null => {}
        other => return Err(NodeError::input("stack", format!("expected a stack, got {}", other.summary()))),
    }
    for (k, v) in i.list("alphas").into_iter().enumerate() {
        match v {
            Value::AlphaSource(a) => adopt_source(&mut d, &a),
            Value::Null => return Err(NodeError::input("alphas", format!("item {k} failed upstream"))),
            other => return Err(NodeError::input("alphas", format!("item {k} is {}, not an alpha source", other.summary()))),
        }
    }
    if let Some(name) = i.get("name").as_text() {
        if !name.trim().is_empty() {
            d.name = name.to_string();
        }
    }
    Ok(Outputs::one("design", d))
}

fn design_get(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let pointer = i.text("pointer")?;
    let json = serde_json::to_value(&d).map_err(|e| NodeError::new(e.to_string()))?;
    let v = if pointer.is_empty() { Some(&json) } else { json.pointer(pointer) };
    let v = v.ok_or_else(|| NodeError::input("pointer", format!("nothing at {pointer:?}")))?.clone();
    Ok(Outputs::one("value", v))
}

fn design_set(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let pointer = i.text("pointer")?;
    if pointer.is_empty() {
        return Err(NodeError::input("pointer", "set needs a field pointer such as /profile/width_mm"));
    }
    let value = i.get("value").to_json_any().ok_or_else(|| NodeError::input("value", format!("{} has no JSON form", i.get("value").summary())))?;
    let mut json = serde_json::to_value(&d).map_err(|e| NodeError::new(e.to_string()))?;
    if json.pointer(pointer).is_none() {
        return Err(NodeError::input("pointer", format!("a design has nothing at {pointer:?}")));
    }
    set_pointer(&mut json, pointer, value).map_err(|m| NodeError::input("pointer", m))?;
    let d: RingDesign = serde_json::from_value(json).map_err(|e| NodeError::input("value", format!("the design would not read back: {e}")))?;
    Ok(Outputs::one("design", d))
}

fn design_info(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let ctx = d.field_context();
    Ok(Outputs::one("name", d.name.clone())
        .with("size", d.size.0)
        .with("inner_diameter_mm", d.size.inner_diameter_mm())
        .with("width_mm", d.profile.width_mm)
        .with("thickness_mm", d.profile.thickness_mm)
        .with("circumference_mm", ctx.circumference_mm)
        .with("band_v_len_mm", ctx.band_v_len_mm)
        .with("layers", d.layers.layers.len() as i64)
        .with("stack", Value::Stack(Arc::new(d.layers.clone()))))
}

pub fn register(reg: &mut Registry) {
    let specs = [
        NodeSpec::new("stack", "Stack", Category::Assembly)
            .doc("Entries in order, bottom first. A bare layer becomes an entry named after its kind; a stack splices in.")
            .input(PinSpec::item("stack", ValueKind::Stack).optional().doc("Append to this stack."))
            .input(PinSpec::list("entries", ValueKind::Entry).doc("The entries (or layers) to add."))
            .output(PinSpec::item("stack", ValueKind::Stack).doc("The stack."))
            .eval(stack),
        NodeSpec::new("design.assemble", "Assemble", Category::Assembly)
            .doc("The design with its layer stack and the alpha sources its layers name; sources bake into the library when the design loads.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design to fill."))
            .input(PinSpec::item("stack", ValueKind::Stack).optional().doc("The layer stack; the design's own if unset."))
            .input(PinSpec::list("alphas", ValueKind::AlphaSource).doc("Alpha sources the layers refer to by name."))
            .input(PinSpec::item("name", ValueKind::Text).optional().widget(Widget::TextLine).doc("Rename the design."))
            .output(PinSpec::item("design", ValueKind::Design).doc("The assembled design."))
            .eval(assemble),
        NodeSpec::new("design.get", "Design field", Category::Assembly)
            .doc("Read any field of the design by JSON pointer (/profile/width_mm, /shank/head/outline).")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("pointer", ValueKind::Text).default("/profile/width_mm").widget(Widget::TextLine).doc("An RFC 6901 pointer."))
            .output(PinSpec::item("value", ValueKind::Json).doc("The field, as JSON."))
            .eval(design_get),
        NodeSpec::new("design.set", "Set design field", Category::Assembly)
            .doc("Write any existing field of the design by JSON pointer — the escape hatch for what has no node yet.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("pointer", ValueKind::Text).default("").widget(Widget::TextLine).doc("An RFC 6901 pointer to an existing field."))
            .input(PinSpec::item("value", ValueKind::Any).doc("The new value."))
            .output(PinSpec::item("design", ValueKind::Design).doc("The changed design."))
            .eval(design_set),
        NodeSpec::new("design.info", "Design info", Category::Assembly)
            .doc("What a design is: name, size, band dimensions, chart lengths, and its stack.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .output(PinSpec::item("name", ValueKind::Text).doc("Its name."))
            .output(PinSpec::item("size", ValueKind::Number).doc("US ring size."))
            .output(PinSpec::item("inner_diameter_mm", ValueKind::Number).doc("Bore diameter, mm."))
            .output(PinSpec::item("width_mm", ValueKind::Number).doc("Band width, mm."))
            .output(PinSpec::item("thickness_mm", ValueKind::Number).doc("Band thickness, mm."))
            .output(PinSpec::item("circumference_mm", ValueKind::Number).doc("The chart's u length: circumference at the crest, mm."))
            .output(PinSpec::item("band_v_len_mm", ValueKind::Number).doc("The chart's v length: section arc edge to edge, mm."))
            .output(PinSpec::item("layers", ValueKind::Int).doc("How many entries the stack holds."))
            .output(PinSpec::item("stack", ValueKind::Stack).doc("The stack."))
            .eval(design_info),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets};
    use crate::graph::Graph;
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    #[test]
    fn stacks_assemble_and_pointers_read_and_write_the_design() {
        let mut g = Graph::default();
        let d = g.add("design.new").unwrap();
        let m = g.add("layer.milgrain").unwrap();
        let b = g.add("layer.border").unwrap();
        let st = g.add("stack").unwrap();
        let entries = g.add("list.merge").unwrap();
        g.connect(m, "layer", entries, "a").unwrap();
        g.connect(b, "layer", entries, "b").unwrap();
        g.connect(entries, "out", st, "entries").unwrap();
        let asm = g.add("design.assemble").unwrap();
        g.connect(d, "design", asm, "design").unwrap();
        g.connect(st, "stack", asm, "stack").unwrap();
        let get = g.add("design.get").unwrap();
        g.connect(asm, "design", get, "design").unwrap();
        g.set_input(get, "pointer", Literal::Text("/layers/layers/1/name".into())).unwrap();
        let set = g.add("design.set").unwrap();
        g.connect(asm, "design", set, "design").unwrap();
        g.set_input(set, "pointer", Literal::Text("/profile/width_mm".into())).unwrap();
        g.set_input(set, "value", Literal::Number(9.5)).unwrap();
        let info = g.add("design.info").unwrap();
        g.connect(set, "design", info, "design").unwrap();
        let bad = g.add("design.set").unwrap();
        g.connect(asm, "design", bad, "design").unwrap();
        g.set_input(bad, "pointer", Literal::Text("/profile/nope".into())).unwrap();
        g.set_input(bad, "value", Literal::Number(1.0)).unwrap();
        let r = Evaluator::new().evaluate(&g, &Registry::builtin(), &AlphaLibrary::default(), 0, Targets::AllPure);
        let Some(Value::Stack(s)) = r.value(st, "stack") else { panic!("{:?}", r.status[&st]) };
        assert_eq!(s.layers.len(), 2, "bare layers coerce into entries");
        assert_eq!((s.layers[0].name.as_str(), s.layers[1].name.as_str()), ("Milgrain", "Border"));
        assert_eq!(r.value(get, "value"), Some(&Value::Json(Arc::new(serde_json::json!("Border")))));
        assert_eq!(r.value(info, "width_mm"), Some(&Value::Number(9.5)));
        assert_eq!(r.value(info, "layers"), Some(&Value::Int(2)));
        assert!(r.value(info, "circumference_mm").unwrap().as_number().unwrap() > 50.0);
        assert!(r.status[&bad].errors[0].1.contains("nothing at"), "{:?}", r.status[&bad].errors);
    }
}
