//! Shanks, signet heads and head outlines.

use std::sync::Arc;

use ringdesign_core::field::SignetOutline;
use ringdesign_core::library;
use ringdesign_core::profile::{ShankKind, SignetHead};
use ringdesign_core::{CustomOutline, ShankStyle};

use super::band::shank_value;
use super::structs::{StructNode, enum_names};
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn unwrap_shank(v: &Value) -> Option<ShankStyle> {
    match v {
        Value::Shank(s) => Some((**s).clone()),
        _ => None,
    }
}

fn shank_of(i: &Inputs, pin: &str) -> Result<ShankStyle, NodeError> {
    match i.get(pin) {
        Value::Shank(s) => Ok((**s).clone()),
        Value::Null => Ok(ShankStyle::default()),
        other => Err(NodeError::input(pin, format!("expected a shank, got {}", other.kind()))),
    }
}

fn parse_outline(name: &str) -> Result<SignetOutline, NodeError> {
    serde_json::from_value(serde_json::Value::String(name.to_string()))
        .map_err(|_| NodeError::input("outline", format!("{name:?} is not a builtin outline; custom plans come in through shank.outline")))
}

fn shank_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("shank", "Shank", Category::Shank)
            .doc("How the band changes round the ring: a kind, its strength, and the head it carries when it is a signet."),
        "shank",
        ShankStyle::default,
        shank_value,
        unwrap_shank,
    )
    .base("shank", ValueKind::Shank, "Start from this shank; uniform otherwise.")
    .field(PinSpec::select("kind", enum_names(ShankKind::ALL)).doc("The modulation family. Switching to Signet here keeps the head as is; use shank.signet for signet defaults."))
    .field(PinSpec::item("amount", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("The modulation's strength, 0..1."))
    .field(PinSpec::item("waves", ValueKind::Int).doc("Waves round the ring for Wave and Twist; an integer, so the joint closes."))
    .field(PinSpec::item("head", ValueKind::Head).doc("The signet head."))
    .field_at(PinSpec::item("head_theta_deg", ValueKind::Number).widget(Widget::Angle).doc("Where the head sits; 90° is the top."), "/head/theta_deg")
    .field_at(PinSpec::item("head_length_mm", ValueKind::Number).widget(Widget::Mm { min: 2.0, max: 40.0 }).doc("The face's length along the ring, mm."), "/head/length_mm")
    .hidden(&["extra_heads", "keys", "custom_outlines"])
    .build()
}

fn head_outline_check(_: &mut SignetHead, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    if let Some(name) = i.get("outline").as_text() {
        parse_outline(name)?;
    }
    Ok(())
}

fn head_fit(h: &mut SignetHead, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    if let Some(w) = i.get("fit_to_width_mm").as_number() {
        if w <= 0.0 {
            return Err(NodeError::input("fit_to_width_mm", "must be positive"));
        }
        h.fit_length_to(w);
    }
    Ok(())
}

fn head_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("head", "Signet head", Category::Shank).doc("A signet head: outline, size, stand-off and construction (prism, cut dome, or the lofted body)."),
        "head",
        SignetHead::lofted,
        Value::Head,
        |v| match v {
            Value::Head(h) => Some(*h),
            _ => None,
        },
    )
    .base("head", ValueKind::Head, "Start from this head; the lofted oval otherwise.")
    .field(PinSpec::select("outline", enum_names(SignetOutline::ALL)).doc("The face's plan, one of the builtins."))
    .field(PinSpec::item("theta_deg", ValueKind::Number).widget(Widget::Angle).doc("Where the head sits; 90° is the top."))
    .field(PinSpec::item("length_mm", ValueKind::Number).widget(Widget::Mm { min: 2.0, max: 40.0 }).doc("The face's length along the ring, mm."))
    .field(PinSpec::item("rise_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 8.0 }).doc("How far the table stands over the shank, mm."))
    .field(PinSpec::item("shoulder_deg", ValueKind::Number).widget(Widget::Slider { min: 5.0, max: 120.0 }).doc("The arc over which the crest falls back to the shank."))
    .field(PinSpec::item("swell_deg", ValueKind::Number).widget(Widget::Slider { min: 10.0, max: 170.0 }).doc("The arc the width takes to come back; scaled with the head if unset."))
    .field(PinSpec::item("body_fair", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("How much the body is faired under the face, 0..1."))
    .field(PinSpec::item("table_flat", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("1 is a true plane to engrave."))
    .field(PinSpec::item("table_dome_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 3.0 }).doc("A cabochon cap on the table; on a lofted head, the apex height of the smooth table."))
    .field(PinSpec::item("hollow_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 4.0 }).doc("A scoop from the finger hole up into the head's belly, mm; lightens a heavy head."))
    .field(PinSpec::item("rim_round_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 2.0 }).doc("The table rim's fillet, mm."))
    .field(PinSpec::item("crest_round_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 3.0 }).doc("Fillet on the crest's corner where the plate's climb meets the shoulder's dive — the loft's answer to the prism's smin(climb, dive, rim). 0 keeps the factory rebuilds bit-identical."))
    .field(PinSpec::item("dome", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("The cut-dome construction; takes precedence over the loft."))
    .field(PinSpec::item("loft", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("The lofted body; 1 for a new signet."))
    .field(PinSpec::item("loft_frontal_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 20.0 }).doc("Body growth along the ring under the table, mm."))
    .field(PinSpec::item("loft_lateral_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 20.0 }).doc("Body growth across the band under the table, mm."))
    .extra(PinSpec::item("fit_to_width_mm", ValueKind::Number).widget(Widget::Mm { min: 1.0, max: 25.0 }).doc("Size the face to a band this wide, by the outline's own aspect."))
    .prepare(head_outline_check)
    .finish(head_fit)
    .build()
}

fn shank_signet(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut s = shank_of(i, "shank")?;
    let width = i.number("band_width_mm")?;
    if width <= 0.0 {
        return Err(NodeError::input("band_width_mm", "must be positive"));
    }
    s.apply_signet(width);
    if let Some(name) = i.get("outline").as_text() {
        let o = parse_outline(name)?;
        s.head.outline = o;
        s.head.fit_length_to(width);
        s.head.dome = s.suggest_dome(o);
    }
    Ok(Outputs::one("shank", shank_value(s)))
}

fn shank_add_head(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut s = shank_of(i, "shank")?;
    match i.get("head") {
        Value::Head(h) => s.extra_heads.push(*h),
        other => return Err(NodeError::input("head", format!("expected a head, got {}", other.kind()))),
    }
    Ok(Outputs::one("shank", shank_value(s)))
}

fn shank_outline(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut s = shank_of(i, "shank")?;
    let outline = match i.get("outline") {
        Value::Outline(o) => o.clone(),
        other => return Err(NodeError::input("outline", format!("expected an outline, got {}", other.kind()))),
    };
    let o = s.adopt_outline((*outline).clone());
    s.head.outline = o;
    if let Some(w) = i.get("band_width_mm").as_number() {
        s.head.fit_length_to(w);
    }
    s.head.dome = s.suggest_dome(o);
    Ok(Outputs::one("shank", shank_value(s)).with("outline_index", i64::from(match o {
        SignetOutline::Custom(k) => k,
        _ => 0,
    })))
}

fn outline_custom(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let name = i.text("name")?.to_string();
    let pts: Vec<[f64; 2]> = match i.get("points") {
        Value::Path(p) => (**p).clone(),
        Value::Null => Vec::new(),
        other => return Err(NodeError::input("points", format!("expected a path, got {}", other.kind()))),
    };
    let mut o = CustomOutline::from_points(name, &pts).ok_or_else(|| NodeError::input("points", format!("{} points do not make a plan; at least 8 round a closed shape", pts.len())))?;
    let across = i.bool("symmetric_across")?;
    let along = i.bool("symmetric_along")?;
    if across || along {
        o.symmetrize(across, along);
    }
    if let Some(r) = i.get("fair_r").as_number() {
        o.fair_r = r.clamp(0.0, 3.0);
    }
    Ok(Outputs::one("outline", Value::Outline(Arc::new(o))))
}

fn outline_library(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let name = i.text("name")?;
    let entries = library::list_outlines();
    let Some(o) = entries.into_iter().find(|o| o.name == name) else {
        let names: Vec<String> = library::list_outlines().into_iter().map(|o| o.name).collect();
        return Err(NodeError::input("name", format!("no saved outline {name:?}; the library has {names:?}")));
    };
    Ok(Outputs::one("outline", Value::Outline(Arc::new(o))))
}

pub fn register(reg: &mut Registry) {
    let specs = [
        shank_node(),
        head_node(),
        NodeSpec::new("shank.signet", "Signet shank", Category::Shank)
            .doc("A shank made a signet the way the app does it: the signet taper, the face fitted to the band, the lofted body — then an optional builtin outline.")
            .input(PinSpec::item("shank", ValueKind::Shank).optional().doc("Start from this shank."))
            .input(PinSpec::item("band_width_mm", ValueKind::Number).default(12.0).widget(Widget::Mm { min: 1.0, max: 25.0 }).doc("The band's width, which sizes the face."))
            .input(PinSpec::select("outline", enum_names(SignetOutline::ALL)).optional().doc("A builtin outline to fit, with the app's dome suggestion."))
            .output(PinSpec::item("shank", ValueKind::Shank).doc("The signet shank."))
            .eval(shank_signet),
        NodeSpec::new("shank.add_head", "Add head", Category::Shank)
            .doc("A second (or third) head on the shank, at its own angle.")
            .input(PinSpec::item("shank", ValueKind::Shank).optional().doc("The shank to add to."))
            .input(PinSpec::item("head", ValueKind::Head).doc("The extra head."))
            .output(PinSpec::item("shank", ValueKind::Shank).doc("The shank with the head."))
            .eval(shank_add_head),
        NodeSpec::new("shank.outline", "Custom outline on shank", Category::Shank)
            .doc("Carry an imported plan into the shank and put it on the head, with the app's dome suggestion for lobed plans.")
            .input(PinSpec::item("shank", ValueKind::Shank).optional().doc("The shank."))
            .input(PinSpec::item("outline", ValueKind::Outline).doc("The plan."))
            .input(PinSpec::item("band_width_mm", ValueKind::Number).optional().widget(Widget::Mm { min: 1.0, max: 25.0 }).doc("Fit the face to a band this wide."))
            .output(PinSpec::item("shank", ValueKind::Shank).doc("The shank carrying the plan."))
            .output(PinSpec::item("outline_index", ValueKind::Int).doc("The plan's slot in the shank's registry."))
            .eval(shank_outline),
        NodeSpec::new("outline.custom", "Outline from points", Category::Shank)
            .doc("A head plan from a closed point list (any units; the box is normalized), faired with the rolling ball like the builtins.")
            .input(PinSpec::item("name", ValueKind::Text).default("Custom").widget(Widget::TextLine).doc("The plan's name."))
            .input(PinSpec::item("points", ValueKind::Path).doc("The boundary, [x, y] pairs round the shape."))
            .input(PinSpec::item("symmetric_across", ValueKind::Bool).default(false).widget(Widget::Checkbox).doc("Fold symmetric across the band."))
            .input(PinSpec::item("symmetric_along", ValueKind::Bool).default(false).widget(Widget::Checkbox).doc("Fold symmetric along the ring."))
            .input(PinSpec::item("fair_r", ValueKind::Number).optional().widget(Widget::Slider { min: 0.0, max: 3.0 }).doc("The fairing ball's radius in half-lengths; the importer's own choice if unset."))
            .output(PinSpec::item("outline", ValueKind::Outline).doc("The plan."))
            .eval(outline_custom),
        NodeSpec::new("outline.library", "Saved outline", Category::Shank)
            .doc("A head plan from the user's outline library by name.")
            .input(PinSpec::item("name", ValueKind::Text).default("").widget(Widget::TextLine).doc("The saved outline's name."))
            .output(PinSpec::item("outline", ValueKind::Outline).doc("The plan."))
            .eval(outline_library),
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
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::default(), 0, Targets::AllPure)
    }

    #[test]
    fn a_signet_built_through_nodes_is_lofted_and_fields_clean() {
        let mut g = Graph::default();
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(12.0)).unwrap();
        g.set_input(p, "thickness_mm", Literal::Number(1.8)).unwrap();
        g.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let s = g.add("shank.signet").unwrap();
        g.set_input(s, "band_width_mm", Literal::Number(12.0)).unwrap();
        g.set_input(s, "outline", Literal::Text("Cushion".into())).unwrap();
        let h = g.add("head").unwrap();
        g.set_input(h, "rise_mm", Literal::Number(0.5)).unwrap();
        let sh = g.add("shank").unwrap();
        g.connect(s, "shank", sh, "shank").unwrap();
        g.connect(h, "head", sh, "head").unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        g.connect(sh, "shank", d, "shank").unwrap();
        let reg = Registry::builtin();
        let out = evaluate_design(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
        assert!(out.notes.is_empty(), "{:?}", out.notes);
        let design = &out.design;
        assert_eq!(design.shank.kind, ShankKind::Signet);
        // The head node started from the lofted default and set the rise;
        // wired into the shank it replaced the signet's own head outright.
        assert_eq!(design.shank.head.loft, 1.0);
        assert_eq!(design.shank.head.rise_mm, 0.5);
        assert_eq!(design.shank.head.outline, SignetOutline::Oval, "the head pin replaces the head");
        assert_ne!(out.field.verdict, ringdesign_core::castability::Verdict::NotCastable, "{:?}", out.field.notes);

        // Without the head pin, the signet node's fitted cushion stands.
        g.disconnect(sh, "head");
        let out = evaluate_design(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
        assert_eq!(out.design.shank.head.outline, SignetOutline::Cushion);
        let want = 12.0 * SignetOutline::Cushion.head_aspect();
        assert!((out.design.shank.head.length_mm - want).abs() < 1e-9);
        assert_eq!(out.design.shank.head.loft, 1.0);
    }

    #[test]
    fn heads_patch_and_fit_and_outlines_are_named() {
        let mut g = Graph::default();
        let h = g.add("head").unwrap();
        g.set_input(h, "outline", Literal::Text("Heart".into())).unwrap();
        g.set_input(h, "fit_to_width_mm", Literal::Number(10.0)).unwrap();
        let h2 = g.add("head").unwrap();
        g.connect(h, "head", h2, "head").unwrap();
        g.set_input(h2, "dome", Literal::Number(1.0)).unwrap();
        let r = run(&g);
        let Some(Value::Head(a)) = r.value(h, "head") else { panic!("{:?}", r.status[&h]) };
        assert_eq!(a.outline, SignetOutline::Heart);
        assert!((a.length_mm - 10.0 * SignetOutline::Heart.head_aspect()).abs() < 1e-9);
        let Some(Value::Head(b)) = r.value(h2, "head") else { panic!() };
        assert_eq!((b.outline, b.dome, b.length_mm), (SignetOutline::Heart, 1.0, a.length_mm), "a patch over the first head");
        g.set_input(h, "outline", Literal::Text("Custom".into())).unwrap();
        let r = run(&g);
        assert!(r.status[&h].errors[0].1.contains("not a builtin outline"), "{:?}", r.status[&h].errors);
    }

    #[test]
    fn a_drawn_plan_lands_on_the_head_and_the_dome_suggestion_follows() {
        // A clipped four-lobed star: deeply lobed, so the app would put it
        // on the cut dome.
        let mut pts = Vec::new();
        for k in 0..360 {
            let t = (k as f64).to_radians();
            let r = 0.62 + 0.38 * (4.0 * t).cos();
            pts.push([r * t.cos(), r * t.sin()]);
        }
        let mut g = Graph::default();
        let o = g.add("outline.custom").unwrap();
        g.set_input(o, "name", Literal::Text("Star".into())).unwrap();
        g.set_input(o, "points", Literal::Json(serde_json::json!(pts))).unwrap();
        g.set_input(o, "symmetric_across", Literal::Bool(true)).unwrap();
        let s = g.add("shank.signet").unwrap();
        g.set_input(s, "band_width_mm", Literal::Number(9.0)).unwrap();
        let so = g.add("shank.outline").unwrap();
        g.connect(s, "shank", so, "shank").unwrap();
        g.connect(o, "outline", so, "outline").unwrap();
        g.set_input(so, "band_width_mm", Literal::Number(9.0)).unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(so, "shank", d, "shank").unwrap();
        let r = run(&g);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        let Some(Value::Design(design)) = r.value(d, "design") else { panic!() };
        assert!(matches!(design.shank.head.outline, SignetOutline::Custom(_)));
        assert_eq!(design.shank.custom_outlines.len(), 1);
        assert_eq!(design.shank.custom_outlines[0].name, "Star");
        let plan = design.shank.head.outline;
        assert_eq!(design.shank.head.dome, design.shank.suggest_dome(plan), "the node applies the app's dome suggestion");
        assert_eq!(design.shank.head.dome, 1.0, "and four deep lobes are the lobed signature");
        assert_eq!(r.value(so, "outline_index"), Some(&Value::Int(0)));
        // Too few points is refused by name.
        g.set_input(o, "points", Literal::Json(serde_json::json!([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]))).unwrap();
        let r = run(&g);
        assert!(r.status[&o].errors[0].1.contains("at least 8"));
    }
}
