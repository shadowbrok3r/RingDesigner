//! Struck outlines, shaped tops and rows as editable source nodes.

use std::sync::Arc;
use ringdesign_core::{outline, setting::{self, RowPath, Stamp, StampRow, StampTop}};
use crate::{graph::Node, registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry}, value::{Value, ValueKind}};
use super::structs::StructNode;

fn blank() -> Stamp {
    Stamp { name: "Stamp".into(), theta_deg: 90.0, v_mm: 0.0, rot_deg: 0.0, outline: outline::circle(1.0), height_mm: 0.3, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: StampTop::Flat }
}
fn read<T: serde::de::DeserializeOwned>(value: &Value, pin: &str) -> Result<T, NodeError> {
    serde_json::from_value(value.to_json_any().ok_or_else(|| NodeError::input(pin, "expected source data"))?)
        .map_err(|e| NodeError::input(pin, e.to_string()))
}
fn json<T: serde::Serialize>(value: T) -> Value { serde_json::to_value(value).expect("source data").into() }
fn validate(stamp: &mut Stamp, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    if !i.get("top").is_null() { stamp.top = read(i.get("top"), "top")?; }
    if !i.get("tier").is_null() {
        stamp.tier = u8::try_from(i.int("tier")?).map_err(|_| NodeError::input("tier", "must be from 0 through 255"))?;
    }
    if !(3..=setting::MAX_STAMP_POINTS).contains(&stamp.outline.len()) || stamp.outline.iter().flatten().any(|x| !x.is_finite()) {
        return Err(NodeError::input("outline", "expected a finite closed polygon within the stamp point limit"));
    }
    if outline::area(&stamp.outline) <= 0.0 || outline::self_crossing(&stamp.outline).is_some() {
        return Err(NodeError::input("outline", "expected a counter-clockwise polygon without crossings"));
    }
    Ok(())
}
fn apply(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let Value::Design(d) = i.get("design") else { return Err(NodeError::input("design", "connect a design")) };
    let mut d = (**d).clone();
    let stamps: Vec<Stamp> = i.list("stamps").iter().map(|v| read(v, "stamps")).collect::<Result<_, _>>()?;
    if i.bool("replace")? { d.stamps = stamps; } else { d.stamps.extend(stamps); }
    Ok(Outputs::one("design", d))
}
fn row(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let Value::Design(d) = i.get("design") else { return Err(NodeError::input("design", "connect the supporting design")) };
    let path = match i.text("path")? {
        "PartingLine" => RowPath::PartingLine,
        "ChartV" => RowPath::ChartV { v_mm: i.number("v_mm")? },
        "SideFace" => RowPath::SideFace { high: i.bool("high")?, frac: i.number("fraction")? },
        _ => return Err(NodeError::input("path", "unknown row path")),
    };
    let count = u32::try_from(i.int("count")?).map_err(|_| NodeError::input("count", "must be a nonnegative count"))?;
    if !(1..=setting::MAX_ROW_STAMPS).contains(&count) { return Err(NodeError::input("count", "row needs from 1 through 400 stations")); }
    let row = StampRow { stamp: read(i.get("stamp"), "stamp")?, path, from_deg: i.number("from_deg")?, to_deg: i.number("to_deg")?, count, taper: i.number("taper")?, fold_clear_mm: i.number("fold_clear_mm")?, mirror_shoulders: i.bool("mirror_shoulders")? };
    Ok(Outputs::one("stamps", Value::List(setting::stamp_row(d, &row).into_iter().map(json).collect())))
}
fn top(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let p = |pin| i.number(pin);
    let top = match i.text("shape")? {
        "Flat" => StampTop::Flat,
        "Gable" => StampTop::Gable { rise_mm: p("rise_mm")?, axis_deg: p("axis_deg")? },
        "Ridge" => StampTop::Ridge { rise_mm: p("rise_mm")?, from: [p("from_x")?, p("from_y")?], to: [p("to_x")?, p("to_y")?], end_mm: p("end_mm")? },
        "Cone" => StampTop::Cone { apex_mm: p("apex_mm")?, at: [p("at_x")?, p("at_y")?], tip_mm: p("tip_mm")? },
        "Dome" => StampTop::Dome { crown_mm: p("crown_mm")? },
        "Taper" => StampTop::Taper { axis_deg: p("axis_deg")?, tip_mm: p("tip_mm")? },
        _ => return Err(NodeError::input("shape", "unknown stamp top")),
    };
    Ok(Outputs::one("top", json(top)))
}
pub fn register(reg: &mut Registry) {
    let mut stamp = StructNode::new(NodeSpec::new("stamp", "Struck stamp", Category::Layer).doc("A true closed outline struck into or onto the ring with a shaped top."), "stamp", blank, json::<Stamp>, |v| read(v, "stamp").ok())
        .base("stamp", ValueKind::Json, "Optional source stamp to edit.")
        .field(PinSpec::item("name", ValueKind::Text).doc("Stamp family and caption."))
        .field(PinSpec::item("outline", ValueKind::Path).doc("Closed outline in local millimetres."))
        .extra(PinSpec::item("top", ValueKind::Json).doc("Shape supplied by stamp.top; flat when absent."))
        .extra(PinSpec::item("tier", ValueKind::Int).doc("Build against the band at zero, or against all lower tiers."));
    for (pin, doc) in [("theta_deg", "Angle around the ring, degrees."), ("v_mm", "Position across the surface chart, mm."), ("rot_deg", "Turn in the outline plane, degrees."), ("height_mm", "Height over the surface, mm."), ("sink_mm", "Depth under the surface, mm."), ("draft_deg", "Wall draft angle, degrees.")] {
        stamp = stamp.field(PinSpec::item(pin, ValueKind::Number).doc(doc));
    }
    for (pin, doc) in [("cut", "Subtract this stamp from the metal."), ("bench", "Add after casting."), ("along_pull", "Strike along the mould pull on a side face.")] {
        stamp = stamp.field(PinSpec::item(pin, ValueKind::Bool).doc(doc));
    }
    reg.register(stamp.finish(validate).build()).expect("unique");
    reg.register(NodeSpec::new("design.stamps", "Apply stamps", Category::Assembly)
        .doc("Add an ordered list of struck outlines to a design.")
        .input(PinSpec::item("design", ValueKind::Design).doc("Supporting design."))
        .input(PinSpec::list("stamps", ValueKind::Json).doc("Stamps in authoring order."))
        .input(PinSpec::item("replace", ValueKind::Bool).default(false).doc("Replace the current stamps instead of appending."))
        .output(PinSpec::item("design", ValueKind::Design).doc("Design carrying the stamps."))
        .eval(apply)).expect("unique");
    let mut spec = NodeSpec::new("stamp.top", "Stamp top", Category::Layer)
        .doc("Shape a stamp's top or the floor of a cut.")
        .input(PinSpec::select("shape", ["Flat", "Gable", "Ridge", "Cone", "Dome", "Taper"].map(String::from).into()).default("Flat").doc("Top construction."));
    for (pin, default) in [("rise_mm", 0.4), ("axis_deg", 0.0), ("from_x", -0.5), ("from_y", 0.0), ("to_x", 0.5), ("to_y", 0.0), ("end_mm", 0.1), ("apex_mm", 0.5), ("at_x", 0.0), ("at_y", 0.0), ("tip_mm", 0.2), ("crown_mm", 0.3)] {
        spec = spec.input(PinSpec::item(pin, ValueKind::Number).default(default).doc(format!("{} in the stamp's local plane.", pin.replace('_', " "))));
    }
    reg.register(spec.output(PinSpec::item("top", ValueKind::Json).doc("Stamp top source." )).eval(top)).expect("unique");
    let mut spec = NodeSpec::new("stamp.row", "Stamp row", Category::Layer)
        .doc("Place a tapered stamp family along the parting line, chart or a side face.")
        .input(PinSpec::item("design", ValueKind::Design).doc("The surface supporting the row."))
        .input(PinSpec::item("stamp", ValueKind::Json).doc("The family stamp."))
        .input(PinSpec::select("path", ["PartingLine", "ChartV", "SideFace"].map(String::from).into()).default("PartingLine").doc("Surface path construction."))
        .input(PinSpec::item("count", ValueKind::Int).default(12i64).doc("Station count, at most 400."))
        .input(PinSpec::item("high", ValueKind::Bool).default(true).doc("Use the high-v side face."))
        .input(PinSpec::item("mirror_shoulders", ValueKind::Bool).default(false).doc("Reflect the row about the head."));
    for (pin, default) in [("v_mm", 0.0), ("fraction", 0.5), ("from_deg", 0.0), ("to_deg", 180.0), ("taper", 0.0), ("fold_clear_mm", 0.0)] {
        spec = spec.input(PinSpec::item(pin, ValueKind::Number).default(default).doc(pin.replace('_', " ")));
    }
    reg.register(spec.output(PinSpec::list("stamps", ValueKind::Json).doc("Placed family stamps.")).eval(row)).expect("unique");
    register_outlines(reg);
}

fn register_outlines(reg: &mut Registry) {
    for (family, defaults) in [
        ("keel", vec![("length", 3.0), ("width", 1.2), ("tip", 0.2)]),
        ("lanceolate", vec![("length", 3.0), ("width", 1.2), ("tip", 0.2)]),
        ("quill", vec![("length", 3.0), ("width", 1.2), ("tip", 0.2)]),
        ("leaf", vec![("length", 3.0), ("width", 1.4), ("count", 5.0), ("depth", 0.2), ("lean_deg", 15.0)]),
        ("blossom", vec![("petals", 5.0), ("diameter", 3.0)]),
        ("fork", vec![("length", 3.0), ("spread_deg", 40.0), ("stem_width", 0.5), ("arm_width", 0.4)]),
        ("spiral", vec![("turns", 1.0), ("start_radius", 0.5), ("end_radius", 2.0), ("start_width", 0.5), ("end_width", 0.3)]),
        ("rounded_triangle", vec![("width", 2.0), ("height", 3.0), ("radius", 0.2)]),
        ("comb_lobe", vec![("length", 3.0), ("width", 1.2), ("round", 0.2)]),
        ("moon", vec![("radius", 2.0), ("lit", 0.5), ("horn", 0.85)]),
        ("jaw", vec![("radius", 3.0), ("width", 0.7), ("sweep_deg", 120.0), ("teeth", 5.0), ("tooth_mm", 0.3)]),
    ] {
        let mut spec = NodeSpec::new(format!("stamp.outline.{family}"), format!("Stamp {}", family.replace('_', " ")), Category::Layer).doc("A closed, millimetre-true outline from the shared motif library.");
        for (pin, default) in defaults {
            let pin_spec = if matches!(pin, "count" | "petals" | "teeth") {
                PinSpec::item(pin, ValueKind::Int).default(default as i64)
            } else { PinSpec::item(pin, ValueKind::Number).default(default) };
            spec = spec.input(pin_spec.doc(pin.replace('_', " ")));
        }
        if family == "leaf" { spec = spec.input(PinSpec::select("margin", ["Entire", "Serrate", "Lobed", "Holly"].map(String::from).into()).default("Entire").doc("Leaf edge construction.")); }
        reg.register(spec.output(PinSpec::item("outline", ValueKind::Path).doc("Closed boundary in millimetres."))
            .eval(move |_, _, i| {
                let p = |pin| {
                    let value = i.number(pin)?;
                    if !value.is_finite() || value.abs() > 1000.0 { return Err(NodeError::input(pin, "must be finite and within 1000 mm or degrees")); }
                    Ok(value)
                };
                let count = |pin| -> Result<u32, NodeError> {
                    let n = i.int(pin)?;
                    if !(1..=64).contains(&n) { return Err(NodeError::input(pin, "count must be from 1 through 64")); }
                    Ok(n as u32)
                };
                let points = match family {
                    "keel" => outline::keel(p("length")?, p("width")?, p("tip")?),
                    "lanceolate" => outline::lanceolate(p("length")?, p("width")?, p("tip")?),
                    "quill" => outline::quill(p("length")?, p("width")?, p("tip")?),
                    "leaf" => {
                        let margin = match i.text("margin")? {
                            "Entire" => outline::Margin::Entire,
                            "Serrate" => outline::Margin::Serrate { teeth: count("count")?, depth_mm: p("depth")?, lean_deg: p("lean_deg")? },
                            "Lobed" => outline::Margin::Lobed { lobes: count("count")?, depth: p("depth")? },
                            "Holly" => outline::Margin::Holly { spines: count("count")?, depth_mm: p("depth")?, lean_deg: p("lean_deg")? },
                            _ => return Err(NodeError::input("margin", "unknown leaf margin")),
                        };
                        outline::leaf(margin, p("length")?, p("width")?)
                    },
                    "blossom" => outline::blossom(count("petals")?, p("diameter")?),
                    "fork" => outline::fork(p("length")?, p("spread_deg")?, p("stem_width")?, p("arm_width")?),
                    "spiral" => outline::spiral(p("turns")?, p("start_radius")?, p("end_radius")?, p("start_width")?, p("end_width")?),
                    "rounded_triangle" => outline::rounded_triangle(p("width")?, p("height")?, p("radius")?),
                    "comb_lobe" => outline::comb_lobe(p("length")?, p("width")?, p("round")?),
                    "moon" => {
                        let raw = setting::moon_outline(p("radius")?, p("lit")?, p("horn")?);
                        let mut sampled = Vec::new();
                        for (a, b) in raw.iter().zip(raw.iter().cycle().skip(1)).take(raw.len()) {
                            let count = ((a[0] - b[0]).hypot(a[1] - b[1]) / outline::STEP).ceil().max(1.0) as usize;
                            if sampled.len() + count > setting::MAX_STAMP_POINTS { return Err(NodeError::input("radius", "moon exceeds the stamp point limit")); }
                            sampled.extend((0..count).map(|j| {
                                let t = j as f64 / count as f64;
                                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
                            }));
                        }
                        sampled
                    },
                    "jaw" => outline::jaw(p("radius")?, p("width")?, p("sweep_deg")?, count("teeth")?, p("tooth_mm")?),
                    _ => unreachable!(),
                };
                outline::check(&points).map_err(NodeError::new)?;
                Ok(Outputs::one("outline", Value::Path(Arc::new(points))))
            })).expect("unique");
    }
}
