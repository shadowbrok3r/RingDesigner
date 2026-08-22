//! Generators: pavé, halo and channel, emitting live groups. The recipe
//! rides in the group, so the app re-solves the layout when the band
//! moves; the evaluator itself never regenerates.

use std::sync::Arc;

use ringdesign_core::field::{SeatStyle, SideFacePick};
use ringdesign_core::gem::Gem;
use ringdesign_core::pave::{self, HaloSpec, PaveRegion, PaveSpec};
use ringdesign_core::RingDesign;

use super::structs::enum_names;
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn design_of(i: &Inputs) -> Result<Arc<RingDesign>, NodeError> {
    match i.get("design") {
        Value::Design(d) => Ok(d.clone()),
        other => Err(NodeError::input("design", format!("expected a design, got {}", other.summary()))),
    }
}

fn gem_of(i: &Inputs, pin: &str) -> Result<Gem, NodeError> {
    match i.get(pin) {
        Value::Gem(g) => Ok(*g),
        Value::Null => Ok(Gem::default()),
        other => Err(NodeError::input(pin, format!("expected a gem, got {}", other.summary()))),
    }
}

fn enum_of<E: serde::de::DeserializeOwned>(i: &Inputs, pin: &str, what: &str) -> Result<E, NodeError> {
    let name = i.text(pin)?;
    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(|_| NodeError::input(pin, format!("{name:?} is not a {what}")))
}

fn pave(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let design = design_of(i)?;
    let region = match i.text("region")? {
        "side_face" => PaveRegion::SideFace(enum_of::<SideFacePick>(i, "side_pick", "side face pick")?),
        "band" => PaveRegion::VBand { center_mm: i.number("band_center_mm")?, width_mm: i.number("band_width_mm")? },
        other => return Err(NodeError::input("region", format!("{other:?} is not side_face or band"))),
    };
    let spec = PaveSpec {
        gem: gem_of(i, "gem")?,
        bridge_mm: i.number("bridge_mm")?,
        theta_deg: i.number("theta_deg")?,
        span_deg: i.number("span_deg")?,
        region,
        stagger: i.bool("stagger")?,
        style: enum_of::<SeatStyle>(i, "style", "seat style")?,
        rot_deg: i.number("rot_deg")?,
        blend_mm: i.number("blend_mm")?,
        recess_mm: i.number("recess_mm")?,
        pinned: Vec::new(),
    };
    let (entry, outcome) = pave::fill(&design, &spec)
        .ok_or_else(|| NodeError::new("the pavé does not fit: no region of that band takes a row of these stones at this bridge"))?;
    Ok(Outputs::one("entry", entry)
        .with("seats", outcome.seats as i64)
        .with("rows", outcome.rows as i64)
        .with("note", outcome.note.unwrap_or_default()))
}

fn halo(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let design = design_of(i)?;
    let spec = HaloSpec {
        center: gem_of(i, "center")?,
        accent: gem_of(i, "accent")?,
        theta_deg: i.number("theta_deg")?,
        v_mm: i.get("v_mm").as_number(),
        gap_mm: i.number("gap_mm")?,
        bridge_mm: i.number("bridge_mm")?,
        count: i.int("count")?.max(0) as u32,
        rot_deg: i.number("rot_deg")?,
    };
    let (entry, accents) = pave::halo(&design, &spec).ok_or_else(|| NodeError::new("the halo does not fit this band: the plate would overrun the band's width"))?;
    Ok(Outputs::one("entry", entry).with("accents", i64::from(accents)))
}

fn channel(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let design = design_of(i)?;
    let gem = gem_of(i, "gem")?;
    let entry = pave::channel_set(&design, gem, i.number("recess_mm")?).ok_or_else(|| {
        NodeError::new(format!(
            "a channel for a {:.1} mm stone needs a thick, squared band: stone plus two rails want about {:.1} mm of side face, and this band's face is not that deep",
            gem.w_mm,
            gem.w_mm + 1.4
        ))
    })?;
    Ok(Outputs::one("entry", entry))
}

pub fn register(reg: &mut Registry) {
    let side_picks: Vec<String> = enum_names(&[SideFacePick::Low, SideFacePick::High, SideFacePick::Wider, SideFacePick::Both]);
    let specs = [
        NodeSpec::new("gen.pave", "Pavé", Category::Generator)
            .doc("Gypsy seats packed over an arc of a side face or a v-band, hex-staggered, wrap-exact on a full ring. A live group: the app re-packs it when the band moves.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design the pavé sits on."))
            .input(PinSpec::item("gem", ValueKind::Gem).optional().doc("The melee; the default 3 mm round if unset."))
            .input(PinSpec::item("bridge_mm", ValueKind::Number).default(0.4).widget(Widget::Mm { min: 0.2, max: 2.0 }).doc("Metal between stones, mm."))
            .input(PinSpec::item("theta_deg", ValueKind::Number).default(90.0).widget(Widget::Angle).doc("Arc centre."))
            .input(PinSpec::item("span_deg", ValueKind::Number).default(360.0).widget(Widget::Slider { min: 10.0, max: 360.0 }).doc("Arc width; 360 is the whole ring."))
            .input(PinSpec::select("region", vec!["side_face".into(), "band".into()]).default("side_face").doc("Where across the band."))
            .input(PinSpec::select("side_pick", side_picks.clone()).default("Wider").doc("Which side face."))
            .input(PinSpec::item("band_center_mm", ValueKind::Number).default(0.0).doc("Band region centre, mm of section arc."))
            .input(PinSpec::item("band_width_mm", ValueKind::Number).default(3.0).doc("Band region width, mm."))
            .input(PinSpec::item("stagger", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Hex-stagger the rows."))
            .input(PinSpec::select("style", enum_names(SeatStyle::ALL)).default("GypsyMound").doc("The seat style."))
            .input(PinSpec::item("rot_deg", ValueKind::Number).default(0.0).doc("Stone bearing, degrees."))
            .input(PinSpec::item("blend_mm", ValueKind::Number).default(0.5).doc("Seat skirt, mm."))
            .input(PinSpec::item("recess_mm", ValueKind::Number).default(0.5).doc("Bezel recess, mm."))
            .output(PinSpec::item("entry", ValueKind::Entry).doc("The live group."))
            .output(PinSpec::item("seats", ValueKind::Int).doc("Seats placed."))
            .output(PinSpec::item("rows", ValueKind::Int).doc("Rows packed."))
            .output(PinSpec::item("note", ValueKind::Text).doc("What the packer had to say, if anything."))
            .eval(pave),
        NodeSpec::new("gen.halo", "Halo", Category::Generator)
            .doc("A centre stone on a gypsy plate ringed by melee markers, cast as one clean dome. A live group.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("center", ValueKind::Gem).optional().doc("The centre stone."))
            .input(PinSpec::item("accent", ValueKind::Gem).optional().doc("The melee."))
            .input(PinSpec::item("theta_deg", ValueKind::Number).default(90.0).widget(Widget::Angle).doc("Where round the ring."))
            .input(PinSpec::item("v_mm", ValueKind::Number).optional().doc("Where across the band; the crest if unset."))
            .input(PinSpec::item("gap_mm", ValueKind::Number).default(0.5).doc("Centre to melee gap, mm."))
            .input(PinSpec::item("bridge_mm", ValueKind::Number).default(0.4).doc("Melee bridge, mm."))
            .input(PinSpec::item("count", ValueKind::Int).default(0i64).doc("Melee count; 0 lets the ring decide."))
            .input(PinSpec::item("rot_deg", ValueKind::Number).default(0.0).doc("Centre stone bearing, degrees."))
            .output(PinSpec::item("entry", ValueKind::Entry).doc("The live group."))
            .output(PinSpec::item("accents", ValueKind::Int).doc("Melee placed."))
            .eval(halo),
        NodeSpec::new("gen.channel", "Channel set", Category::Generator)
            .doc("Two rails flanking a recessed channel on the wider side face — a thick-band feature. A live group.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("gem", ValueKind::Gem).optional().doc("The stone the channel is cut for."))
            .input(PinSpec::item("recess_mm", ValueKind::Number).default(0.5).doc("Channel depth, mm."))
            .output(PinSpec::item("entry", ValueKind::Entry).doc("The live group."))
            .eval(channel),
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
    use crate::registry::Registry;
    use crate::value::Literal;
    use ringdesign_core::pave::GenRecipe;
    use ringdesign_core::{AlphaLibrary, Layer};

    fn squared_design(g: &mut Graph, width: f64, thickness: f64) -> crate::graph::NodeId {
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(width)).unwrap();
        g.set_input(p, "thickness_mm", Literal::Number(thickness)).unwrap();
        g.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        d
    }

    #[test]
    fn generators_emit_live_groups_and_say_when_they_do_not_fit() {
        let mut g = Graph::default();
        let d = squared_design(&mut g, 7.0, 4.0);
        let gem = g.add("gem.calibrated").unwrap();
        g.set_input(gem, "w_mm", Literal::Number(1.5)).unwrap();
        let pave_ = g.add("gen.pave").unwrap();
        g.connect(d, "design", pave_, "design").unwrap();
        g.connect(gem, "gem", pave_, "gem").unwrap();
        g.set_input(pave_, "span_deg", Literal::Number(90.0)).unwrap();
        let centre = g.add("gem.calibrated").unwrap();
        g.set_input(centre, "w_mm", Literal::Number(2.5)).unwrap();
        let melee = g.add("gem.calibrated").unwrap();
        g.set_input(melee, "w_mm", Literal::Number(1.0)).unwrap();
        let halo_ = g.add("gen.halo").unwrap();
        g.connect(d, "design", halo_, "design").unwrap();
        g.connect(centre, "gem", halo_, "center").unwrap();
        g.connect(melee, "gem", halo_, "accent").unwrap();
        let channel_ = g.add("gen.channel").unwrap();
        g.connect(d, "design", channel_, "design").unwrap();
        g.connect(gem, "gem", channel_, "gem").unwrap();
        let thin = squared_design(&mut g, 3.0, 1.2);
        let no_channel = g.add("gen.channel").unwrap();
        g.connect(thin, "design", no_channel, "design").unwrap();
        let r = Evaluator::new().evaluate(&g, &Registry::builtin(), &AlphaLibrary::builtin(), 0, Targets::AllPure);
        let Some(Value::Entry(pe)) = r.value(pave_, "entry") else { panic!("{:?}", r.status[&pave_]) };
        match &pe.layer {
            Layer::Group(grp) => {
                assert!(matches!(grp.recipe, Some(GenRecipe::Pave(_))), "the group carries its recipe");
                assert!(!grp.stack.layers.is_empty());
            }
            other => panic!("{other:?}"),
        }
        assert!(r.value(pave_, "seats").unwrap().as_int().unwrap() > 0);
        let Some(Value::Entry(he)) = r.value(halo_, "entry") else { panic!("{:?}", r.status[&halo_]) };
        assert!(matches!(&he.layer, Layer::Group(grp) if matches!(grp.recipe, Some(GenRecipe::Halo(_)))));
        assert!(r.value(halo_, "accents").unwrap().as_int().unwrap() > 0);
        let Some(Value::Entry(ce)) = r.value(channel_, "entry") else { panic!("{:?}", r.status[&channel_]) };
        assert!(matches!(&ce.layer, Layer::Group(grp) if matches!(grp.recipe, Some(GenRecipe::Channel(_)))));
        assert!(r.status[&no_channel].errors[0].1.contains("squared band"), "{:?}", r.status[&no_channel].errors);
    }
}
