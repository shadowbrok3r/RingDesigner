//! The design, its size and its band section.

use std::sync::Arc;

use ringdesign_core::library;
use ringdesign_core::sizing::RingSize;
use ringdesign_core::{BandProfile, ProfileStyle, RingDesign, ShankStyle};

use super::structs::{StructNode, enum_names};
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn design_new(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut d = RingDesign::default();
    d.name = i.text("name")?.to_string();
    let size = i.number("size")?;
    if !(1.0..=20.0).contains(&size) {
        return Err(NodeError::input("size", format!("{size} is not a US ring size between 1 and 20")));
    }
    d.size = RingSize(size);
    match i.get("profile") {
        Value::Profile(p) => d.profile = *p,
        Value::Null => {}
        other => return Err(NodeError::input("profile", format!("expected a profile, got {}", other.kind()))),
    }
    match i.get("shank") {
        Value::Shank(s) => d.shank = (**s).clone(),
        Value::Null => {}
        other => return Err(NodeError::input("shank", format!("expected a shank, got {}", other.kind()))),
    }
    Ok(Outputs::one("design", d))
}

fn size(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let s = RingSize(i.number("size")?);
    Ok(Outputs::one("inner_diameter_mm", s.inner_diameter_mm())
        .with("inner_circumference_mm", s.inner_circumference_mm())
        .with("bore_radius_mm", s.inner_diameter_mm() * 0.5)
        .with("label", s.display()))
}

fn size_fit(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = i.number("inner_diameter_mm")?;
    if d <= 0.0 {
        return Err(NodeError::input("inner_diameter_mm", "must be positive"));
    }
    Ok(Outputs::one("size", RingSize::from_diameter_mm(d).0))
}

fn profile_prepare(p: &mut BandProfile, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    if let Some(name) = i.get("style").as_text() {
        let style: ProfileStyle = serde_json::from_value(serde_json::Value::String(name.to_string()))
            .map_err(|_| NodeError::input("style", format!("{name:?} is not a profile style")))?;
        p.apply_style(style);
    }
    Ok(())
}

fn profile_finish(p: &mut BandProfile, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    if i.get("flatten_sides").as_bool() == Some(true) {
        p.flatten_sides();
    }
    if p.width_mm <= 0.0 || p.thickness_mm <= 0.0 {
        return Err(NodeError::new(format!("a band {:.2} × {:.2} mm has no metal", p.width_mm, p.thickness_mm)));
    }
    Ok(())
}

fn profile_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("band.profile", "Band profile", Category::Band)
            .doc("The band's cross-section: a style preset, then any field set explicitly. Unset pins keep the base's values."),
        "profile",
        BandProfile::default,
        Value::Profile,
        |v| match v {
            Value::Profile(p) => Some(*p),
            _ => None,
        },
    )
    .base("profile", ValueKind::Profile, "Start from this section; the default band otherwise.")
    .field(PinSpec::select("style", enum_names(ProfileStyle::ALL)).doc("Section family: sets the drop law, crown and edge before the pins below."))
    .field(PinSpec::item("width_mm", ValueKind::Number).widget(Widget::Mm { min: 1.0, max: 25.0 }).doc("Width along the finger, mm."))
    .field(PinSpec::item("thickness_mm", ValueKind::Number).widget(Widget::Mm { min: 0.6, max: 8.0 }).doc("Bore to crest, mm."))
    .field(PinSpec::item("crown_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 6.0 }).doc("How much of the thickness is the dome, mm."))
    .field(PinSpec::item("shape_a", ValueKind::Number).doc("Superellipse drop exponent a."))
    .field(PinSpec::item("shape_b", ValueKind::Number).doc("Superellipse drop exponent b."))
    .field(PinSpec::item("crest_bias", ValueKind::Number).widget(Widget::Slider { min: -1.0, max: 1.0 }).doc("Crest offset across the band, −1..1."))
    .field(PinSpec::item("edge_round_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 2.0 }).doc("Edge fillet, mm."))
    .field(PinSpec::item("comfort_fit_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 1.0 }).doc("Comfort-fit bore widening at the edges, mm."))
    .field(PinSpec::item("side_draft_deg", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 30.0 }).doc("Side face draft, degrees."))
    .extra(PinSpec::item("flatten_sides", ValueKind::Bool).widget(Widget::Checkbox).doc("Square the side faces for ornament: zero side draft, small edge fillet."))
    .hidden(&["flange", "drop_curve", "morph"])
    .prepare(profile_prepare)
    .finish(profile_finish)
    .build()
}

fn profile_library(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let name = i.text("name")?;
    let entries = library::list_profiles();
    let Some((_, shape)) = entries.iter().find(|(n, _)| n == name) else {
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        return Err(NodeError::input("name", format!("no saved profile {name:?}; the library has {names:?}")));
    };
    let out = match i.get("profile") {
        Value::Profile(base) => {
            let mut p = *base;
            p.apply_shape(shape);
            p
        }
        Value::Null => *shape,
        other => return Err(NodeError::input("profile", format!("expected a profile, got {}", other.kind()))),
    };
    Ok(Outputs::one("profile", out))
}

pub fn register(reg: &mut Registry) {
    let specs = [
        NodeSpec::new("design.new", "New design", Category::Band)
            .doc("A ring design: a name, a US size, and optionally a band section and a shank. Layers come later in the chain.")
            .input(PinSpec::item("name", ValueKind::Text).default("Untitled").widget(Widget::TextLine).doc("The design's name."))
            .input(PinSpec::item("size", ValueKind::Number).default(7.0).widget(Widget::Slider { min: 3.0, max: 15.0 }).doc("US ring size."))
            .input(PinSpec::item("profile", ValueKind::Profile).optional().doc("The band section; the default band if unset."))
            .input(PinSpec::item("shank", ValueKind::Shank).optional().doc("The shank; uniform if unset."))
            .output(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .eval(design_new),
        NodeSpec::new("band.size", "Ring size", Category::Band)
            .doc("A US ring size as millimetres: inner diameter, inner circumference, bore radius.")
            .input(PinSpec::item("size", ValueKind::Number).default(7.0).widget(Widget::Slider { min: 3.0, max: 15.0 }).doc("US ring size."))
            .output(PinSpec::item("inner_diameter_mm", ValueKind::Number).doc("Bore diameter, mm."))
            .output(PinSpec::item("inner_circumference_mm", ValueKind::Number).doc("Bore circumference, mm."))
            .output(PinSpec::item("bore_radius_mm", ValueKind::Number).doc("Bore radius, mm."))
            .output(PinSpec::item("label", ValueKind::Text).doc("The size as shown in the app."))
            .eval(size),
        NodeSpec::new("band.size.fit", "Size from diameter", Category::Band)
            .doc("The US size a bore diameter corresponds to.")
            .input(PinSpec::item("inner_diameter_mm", ValueKind::Number).default(17.35).widget(Widget::Mm { min: 10.0, max: 30.0 }).doc("Bore diameter, mm."))
            .output(PinSpec::item("size", ValueKind::Number).doc("US ring size."))
            .eval(size_fit),
        profile_node(),
        NodeSpec::new("band.profile.library", "Saved profile", Category::Band)
            .doc("A section from the user's profile library by name. With a base profile, applies the saved shape while keeping the base's width and thickness — a profile is a section, never a size.")
            .input(PinSpec::item("name", ValueKind::Text).default("").widget(Widget::TextLine).doc("The saved profile's name."))
            .input(PinSpec::item("profile", ValueKind::Profile).optional().doc("Keep this band's width and thickness."))
            .output(PinSpec::item("profile", ValueKind::Profile).doc("The section."))
            .eval(profile_library),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}

/// A shank handle for tests and sibling modules.
pub(crate) fn shank_value(s: ShankStyle) -> Value {
    Value::Shank(Arc::new(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets};
    use crate::graph::Graph;
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::default(), 0, Targets::AllPure)
    }

    #[test]
    fn a_profile_node_applies_its_style_before_its_pins() {
        let mut g = Graph::default();
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(7.0)).unwrap();
        g.set_input(p, "crown_mm", Literal::Number(0.9)).unwrap();
        g.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let r = run(&g);
        let Some(Value::Profile(bp)) = r.value(p, "profile") else { panic!("{:?}", r.status[&p]) };
        let mut want = BandProfile::default();
        want.apply_style(ProfileStyle::Flat);
        assert_eq!(bp.style, ProfileStyle::Flat);
        assert_eq!((bp.shape_a, bp.shape_b), (want.shape_a, want.shape_b), "the style's drop law");
        assert_eq!(bp.width_mm, 7.0);
        assert_eq!(bp.crown_mm, 0.9, "an explicit crown survives the style");
        assert_eq!(bp.side_draft_deg, 0.0, "flattened");
        // A style nobody has is refused by name; a dead band too.
        g.set_input(p, "style", Literal::Text("Octagonal".into())).unwrap();
        let r = run(&g);
        assert!(r.status[&p].errors[0].1.contains("not a profile style"));
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(0.0)).unwrap();
        let r = run(&g);
        assert!(r.status[&p].errors[0].1.contains("no metal"));
    }

    #[test]
    fn size_and_design_nodes_agree_with_the_core() {
        let mut g = Graph::default();
        let s = g.add("band.size").unwrap();
        g.set_input(s, "size", Literal::Number(7.0)).unwrap();
        let f = g.add("band.size.fit").unwrap();
        g.connect(s, "inner_diameter_mm", f, "inner_diameter_mm").unwrap();
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "width_mm", Literal::Number(5.0)).unwrap();
        let d = g.add("design.new").unwrap();
        g.set_input(d, "name", Literal::Text("Court".into())).unwrap();
        g.set_input(d, "size", Literal::Number(8.5)).unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        let r = run(&g);
        let rs = RingSize(7.0);
        assert_eq!(r.value(s, "inner_diameter_mm"), Some(&Value::Number(rs.inner_diameter_mm())));
        assert_eq!(r.value(s, "bore_radius_mm"), Some(&Value::Number(rs.inner_diameter_mm() * 0.5)));
        assert!((r.value(f, "size").unwrap().as_number().unwrap() - 7.0).abs() < 1e-9, "round trip");
        let Some(Value::Design(design)) = r.value(d, "design") else { panic!("{:?}", r.status[&d]) };
        assert_eq!(design.name, "Court");
        assert_eq!(design.size, RingSize(8.5));
        assert_eq!(design.profile.width_mm, 5.0);
        assert_eq!(design.inner_radius_mm(), RingSize(8.5).inner_diameter_mm() * 0.5);
        g.set_input(d, "size", Literal::Number(40.0)).unwrap();
        let r = run(&g);
        assert!(r.status[&d].errors[0].1.contains("US ring size"));
    }
}
