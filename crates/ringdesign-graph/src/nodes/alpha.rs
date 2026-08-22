//! Alpha sources: what the design carries and bakes into its library.

use std::sync::Arc;

use ringdesign_core::alpha::{ProcRecipe, Procedural};
use ringdesign_core::drawn::{DrawnAlpha, Stroke};
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::text::{TextAlpha, TextFont};
use ringdesign_core::EmbeddedAlpha;

use super::structs::enum_names;
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{AlphaSource, Value, ValueKind};

fn name_of(i: &Inputs) -> Result<String, NodeError> {
    let name = i.text("name")?.trim().to_string();
    if name.is_empty() {
        return Err(NodeError::input("name", "an alpha needs a name for layers to refer to"));
    }
    Ok(name)
}

fn out(src: AlphaSource) -> Outputs {
    let name = src.name().to_string();
    Outputs::one("source", Value::AlphaSource(Arc::new(src))).with("alpha", Value::AlphaRef(name))
}

fn proc_(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let kind_name = i.text("kind")?;
    let kind: Procedural = serde_json::from_value(serde_json::Value::String(kind_name.to_string()))
        .map_err(|_| NodeError::input("kind", format!("{kind_name:?} is not a procedural pattern")))?;
    let repeats = i.int("repeats")?;
    if !(1..=64).contains(&repeats) {
        return Err(NodeError::input("repeats", format!("{repeats} is not between 1 and 64")));
    }
    Ok(out(AlphaSource::Procedural(ProcRecipe {
        name: name_of(i)?,
        kind,
        repeats: repeats as u32,
        quarter_turns: (i.int("quarter_turns")?.rem_euclid(4)) as u32,
        gamma: i.number("gamma")?.clamp(0.1, 10.0),
        invert: i.bool("invert")?,
    })))
}

fn text(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let font_name = i.text("font")?;
    let font: TextFont = serde_json::from_value(serde_json::Value::String(font_name.to_string()))
        .map_err(|_| NodeError::input("font", format!("{font_name:?} is not a bundled font")))?;
    let t = TextAlpha { name: name_of(i)?, text: i.text("text")?.to_string(), font, tracking: i.number("tracking")? };
    if t.is_empty() {
        return Err(NodeError::input("text", "nothing to set"));
    }
    Ok(out(AlphaSource::Text(t)))
}

fn svg(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let s = SvgAlpha { name: name_of(i)?, svg: i.text("svg")?.to_string(), invert: i.bool("invert")? };
    if s.is_empty() {
        return Err(NodeError::input("svg", "no drawing"));
    }
    Ok(out(AlphaSource::Svg(s)))
}

fn drawn(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (w, h) = (i.int("width")?, i.int("height")?);
    if !(8..=4096).contains(&w) || !(8..=4096).contains(&h) {
        return Err(NodeError::input("width", "a drawing is 8..4096 texels each way"));
    }
    let mut d = DrawnAlpha::new(name_of(i)?, w as u32, h as u32);
    d.wrap_x = i.bool("wrap_x")?;
    d.wrap_y = i.bool("wrap_y")?;
    for (k, v) in i.list("strokes").into_iter().enumerate() {
        let j = v.to_json_any().ok_or_else(|| NodeError::input("strokes", format!("item {k} is not a stroke")))?;
        let s: Stroke = serde_json::from_value(j).map_err(|e| NodeError::input("strokes", format!("item {k}: {e}")))?;
        d.strokes.push(s);
    }
    Ok(out(AlphaSource::Drawn(d)))
}

fn png(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let png = i.text("png_base64")?.trim().to_string();
    if png.is_empty() {
        return Err(NodeError::input("png_base64", "no image"));
    }
    Ok(out(AlphaSource::Embedded(EmbeddedAlpha { name: name_of(i)?, png })))
}

fn library(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let name = name_of(i)?;
    if ctx.lib.get(&name).is_none() {
        let mut names = ctx.lib.names();
        names.sort();
        let near: Vec<&String> = names.iter().filter(|n| n.to_lowercase().contains(&name.to_lowercase())).take(6).collect();
        return Err(NodeError::input(
            "name",
            if near.is_empty() { format!("no alpha named {name:?}; the library has {} alphas", names.len()) } else { format!("no alpha named {name:?}; near it: {near:?}") },
        ));
    }
    Ok(Outputs::one("alpha", Value::AlphaRef(name)))
}

pub fn register(reg: &mut Registry) {
    let specs = [
        NodeSpec::new("alpha.proc", "Procedural alpha", Category::Alpha)
            .doc("A builtin seamless pattern with knobs that cannot break its seam: integer repeats, quarter turns, gamma.")
            .input(PinSpec::item("name", ValueKind::Text).default("Pattern").widget(Widget::TextLine).doc("The name layers refer to."))
            .input(PinSpec::select("kind", enum_names(Procedural::ALL)).default("Waves").doc("The pattern."))
            .input(PinSpec::item("repeats", ValueKind::Int).default(1i64).doc("Periods per tile, 1..64."))
            .input(PinSpec::item("quarter_turns", ValueKind::Int).default(0i64).doc("Rotation in quarter turns."))
            .input(PinSpec::item("gamma", ValueKind::Number).default(1.0).widget(Widget::Slider { min: 0.1, max: 4.0 }).doc("Value gamma."))
            .input(PinSpec::item("invert", ValueKind::Bool).default(false).widget(Widget::Checkbox).doc("Invert."))
            .output(PinSpec::item("source", ValueKind::AlphaSource).doc("The source, for design.assemble."))
            .output(PinSpec::item("alpha", ValueKind::AlphaRef).doc("Its name, for layers."))
            .eval(proc_),
        NodeSpec::new("alpha.text", "Text alpha", Category::Alpha)
            .doc("An inscription set in a bundled font, rasterized on load.")
            .input(PinSpec::item("name", ValueKind::Text).default("Inscription").widget(Widget::TextLine).doc("The name layers refer to."))
            .input(PinSpec::item("text", ValueKind::Text).default("").widget(Widget::TextLine).doc("The words."))
            .input(PinSpec::select("font", enum_names(TextFont::ALL)).default("Serif").doc("The face."))
            .input(PinSpec::item("tracking", ValueKind::Number).default(0.0).widget(Widget::Slider { min: -0.2, max: 1.0 }).doc("Letter spacing, em."))
            .output(PinSpec::item("source", ValueKind::AlphaSource).doc("The source, for design.assemble."))
            .output(PinSpec::item("alpha", ValueKind::AlphaRef).doc("Its name, for layers."))
            .eval(text),
        NodeSpec::new("alpha.svg", "SVG alpha", Category::Alpha)
            .doc("Vector art as relief: ink coverage reads as height; <text> is deliberately not rendered.")
            .input(PinSpec::item("name", ValueKind::Text).default("Art").widget(Widget::TextLine).doc("The name layers refer to."))
            .input(PinSpec::item("svg", ValueKind::Text).default("").widget(Widget::TextArea).doc("The SVG document."))
            .input(PinSpec::item("invert", ValueKind::Bool).default(false).widget(Widget::Checkbox).doc("Carve instead of raise."))
            .output(PinSpec::item("source", ValueKind::AlphaSource).doc("The source, for design.assemble."))
            .output(PinSpec::item("alpha", ValueKind::AlphaRef).doc("Its name, for layers."))
            .eval(svg),
        NodeSpec::new("alpha.drawn", "Drawn alpha", Category::Alpha)
            .doc("Brush strokes on a canvas, the band-painting format both apps share.")
            .input(PinSpec::item("name", ValueKind::Text).default("band").widget(Widget::TextLine).doc("The name layers refer to."))
            .input(PinSpec::item("width", ValueKind::Int).default(2048i64).doc("Canvas width, texels."))
            .input(PinSpec::item("height", ValueKind::Int).default(320i64).doc("Canvas height, texels."))
            .input(PinSpec::item("wrap_x", ValueKind::Bool).default(true).doc("Seam-wrap round the ring."))
            .input(PinSpec::item("wrap_y", ValueKind::Bool).default(false).doc("Seam-wrap across the band."))
            .input(PinSpec::list("strokes", ValueKind::Json).doc("Strokes as JSON ({points: [[x, y, pressure]…], radius, soft, erase})."))
            .output(PinSpec::item("source", ValueKind::AlphaSource).doc("The source, for design.assemble."))
            .output(PinSpec::item("alpha", ValueKind::AlphaRef).doc("Its name, for layers."))
            .eval(drawn),
        NodeSpec::new("alpha.png", "Embedded PNG alpha", Category::Alpha)
            .doc("A height image carried in the design as base64 PNG, so the file survives moving machines.")
            .input(PinSpec::item("name", ValueKind::Text).default("Image").widget(Widget::TextLine).doc("The name layers refer to."))
            .input(PinSpec::item("png_base64", ValueKind::Text).default("").widget(Widget::TextArea).doc("The PNG, base64."))
            .output(PinSpec::item("source", ValueKind::AlphaSource).doc("The source, for design.assemble."))
            .output(PinSpec::item("alpha", ValueKind::AlphaRef).doc("Its name, for layers."))
            .eval(png),
        NodeSpec::new("alpha.library", "Library alpha", Category::Alpha)
            .doc("An alpha already in the library — a builtin or one in the user's alpha folder — by name.")
            .input(PinSpec::item("name", ValueKind::Text).default("Scales").widget(Widget::TextLine).doc("The library name."))
            .output(PinSpec::item("alpha", ValueKind::AlphaRef).doc("The name, checked."))
            .eval(library),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets, evaluate_design};
    use crate::graph::Graph;
    use crate::registry::Registry;
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    #[test]
    fn sources_assemble_into_the_design_and_bake_for_the_verdict() {
        let mut g = Graph::default();
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(7.0)).unwrap();
        g.set_input(p, "thickness_mm", Literal::Number(2.0)).unwrap();
        g.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        let motto = g.add("alpha.text").unwrap();
        g.set_input(motto, "name", Literal::Text("Motto".into())).unwrap();
        g.set_input(motto, "text", Literal::Text("ever".into())).unwrap();
        let waves = g.add("alpha.proc").unwrap();
        g.set_input(waves, "name", Literal::Text("Ripple".into())).unwrap();
        g.set_input(waves, "repeats", Literal::Int(3)).unwrap();
        let tiling = g.add("layer.tiling.fit").unwrap();
        g.connect(d, "design", tiling, "design").unwrap();
        g.connect(motto, "alpha", tiling, "alpha").unwrap();
        g.set_input(tiling, "square_cells", Literal::Bool(false)).unwrap();
        let entry = g.add("entry").unwrap();
        g.connect(tiling, "layer", entry, "layer").unwrap();
        let stack = g.add("stack").unwrap();
        g.connect(entry, "entry", stack, "entries").unwrap();
        let alphas = g.add("list.merge").unwrap();
        g.connect(motto, "source", alphas, "a").unwrap();
        g.connect(waves, "source", alphas, "b").unwrap();
        let asm = g.add("design.assemble").unwrap();
        g.connect(d, "design", asm, "design").unwrap();
        g.connect(stack, "stack", asm, "stack").unwrap();
        g.connect(alphas, "out", asm, "alphas").unwrap();
        g.set_input(asm, "name", Literal::Text("Posy".into())).unwrap();
        let out = g.add(crate::eval::OUTPUT_KIND).unwrap();
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        // The sink is registered by M2.5; until then a test spec stands in.
        let mut reg2 = Registry::empty();
        for key in reg.keys() {
            reg2.register(reg.get(key).unwrap().clone()).unwrap();
        }
        reg2.register(NodeSpec::new(crate::eval::OUTPUT_KIND, "Output", Category::Sink).input(PinSpec::item(crate::eval::OUTPUT_DESIGN_PIN, ValueKind::Design))).unwrap();
        g.connect(asm, "design", out, crate::eval::OUTPUT_DESIGN_PIN).unwrap();
        let res = evaluate_design(&mut Evaluator::new(), &g, &reg2, &lib, 0).unwrap();
        assert_eq!(res.design.name, "Posy");
        assert_eq!(res.design.texts.len(), 1);
        assert_eq!(res.design.recipes.len(), 1);
        assert_eq!(res.design.layers.layers.len(), 1);
        assert_eq!(res.design.layers.layers[0].name, "Tiling");
        let tiling_alpha = match &res.design.layers.layers[0].layer {
            ringdesign_core::Layer::Tiling(t) => t.alpha.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(tiling_alpha, "Motto");
        assert!(res.notes.iter().any(|n| n.contains("no alpha named \"Motto\" in the library now")), "{:?}", res.notes);
        assert_ne!(res.field.verdict, ringdesign_core::castability::Verdict::NotCastable, "{:?}", res.field.notes);
        // The baked motto was read: the field is not flat where the text sits.
        let mut baked = lib.clone();
        res.design.bake_all(&mut baked);
        assert!(baked.get("Motto").is_some() && baked.get("Ripple").is_some());

        // A same-named source replaces rather than duplicates.
        let again = g.add("design.assemble").unwrap();
        g.connect(asm, "design", again, "design").unwrap();
        g.connect(motto, "source", again, "alphas").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg2, &lib, 0, Targets::AllPure);
        let Some(Value::Design(d2)) = r.value(again, "design") else { panic!() };
        assert_eq!(d2.texts.len(), 1);
    }

    #[test]
    fn library_alphas_are_checked_and_bad_sources_refused() {
        let mut g = Graph::default();
        let ok = g.add("alpha.library").unwrap();
        let bad = g.add("alpha.library").unwrap();
        g.set_input(bad, "name", Literal::Text("scal".into())).unwrap();
        let svg = g.add("alpha.svg").unwrap();
        let drawn = g.add("alpha.drawn").unwrap();
        g.set_input(drawn, "strokes", Literal::Json(serde_json::json!([{"points": [[0.1, 0.1, 1.0], [0.5, 0.5, 1.0]], "radius": 8.0, "soft": 0.5, "erase": false}]))).unwrap();
        let r = Evaluator::new().evaluate(&g, &Registry::builtin(), &AlphaLibrary::builtin(), 0, Targets::AllPure);
        assert_eq!(r.value(ok, "alpha"), Some(&Value::AlphaRef("Scales".into())));
        assert!(r.status[&bad].errors[0].1.contains("Scales"), "near-misses are suggested: {:?}", r.status[&bad].errors);
        assert!(r.status[&svg].errors[0].1.contains("no drawing"));
        match r.value(drawn, "source") {
            Some(Value::AlphaSource(a)) => match &**a {
                AlphaSource::Drawn(d) => assert_eq!(d.strokes.len(), 1),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?} {:?}", r.status[&drawn]),
        }
    }
}
