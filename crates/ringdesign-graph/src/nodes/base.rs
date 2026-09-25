//! Bundled factory stock with editable dimensions and a portable surface chart.

use ringdesign_core::{ProfileStyle, RingDesign, imported_base::{ImportedBase, PRESETS, PresetSource, SurfaceChart}};
use crate::{graph::Node, registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget}, value::{Value, ValueKind}};

fn preset(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let preset = PRESETS.iter().find(|p| p.id == i.text("id").unwrap_or(""))
        .ok_or_else(|| NodeError::input("id", "choose a bundled stock from 001 through 020"))?;
    let sand = i.get("sand").as_bool().unwrap_or_else(|| preset.sand_safe());
    let mut d = match i.get("design") {
        Value::Design(d) => (**d).clone(),
        Value::Null => RingDesign::default(),
        _ => return Err(NodeError::input("design", "expected a design")),
    };
    let source = PresetSource { preset: preset.id.into(), sand_master: sand }.load()
        .map_err(|e| NodeError::input("id", e.to_string()))?;
    let preserve = i.bool("preserve_parameters")?;
    let previous = d.clone();
    ImportedBase::attach(&mut d, source).map_err(|e| NodeError::new(e.to_string()))?;
    if preserve {
        d.size = previous.size;
        d.profile = previous.profile;
        d.shank = previous.shank;
        d.build = previous.build;
    } else {
        let width = d.profile.width_mm;
        d.profile.apply_style(ProfileStyle::Flat);
        d.profile.width_mm = width;
        d.profile.edge_round_mm = 0.3;
        d.profile.comfort_fit_mm = 0.1;
    }
    for (pin, target) in [("face_length_mm", &mut d.shank.head.length_mm), ("face_width_mm", &mut d.profile.width_mm)] {
        if let Some(value) = i.get(pin).as_number() {
            if !value.is_finite() || value <= 0.0 { return Err(NodeError::input(pin, "must be a positive finite dimension")); }
            *target = value;
        }
    }
    if let Some(bore) = i.get("bore_mm").as_number() {
        if !bore.is_finite() || bore <= 0.0 { return Err(NodeError::input("bore_mm", "must be a positive finite dimension")); }
        d.size = ringdesign_core::RingSize((bore * std::f64::consts::PI - 36.5) / 2.55);
    }
    let chart = if i.bool("chart_enabled")? {
        if i.get("chart").is_null() {
            Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() })
        } else {
            Some(serde_json::from_value(i.get("chart").to_json_any().ok_or_else(|| NodeError::input("chart", "expected a chart"))?)
                .map_err(|e| NodeError::input("chart", e.to_string()))?)
        }
    } else { None };
    let base = d.imported_base.as_mut().expect("attached");
    base.chart = chart;
    base.bare = i.bool("bare")?;
    base.sand_envelope = i.get("sand_envelope").as_bool().unwrap_or(sand);
    d.imported_base.as_ref().expect("attached").validate_design(&d).map_err(|e| NodeError::new(e.to_string()))?;
    if !preserve {
        if sand {
            ringdesign_core::castability::CastProcess::SandTwoPart.apply(&mut d.draft);
            ringdesign_core::castability::SandProcess::DelftClay.apply(&mut d.draft);
        }
        else { ringdesign_core::castability::CastProcess::LostWax.apply(&mut d.draft); }
    }
    Ok(Outputs::one("design", d))
}

pub fn register(reg: &mut Registry) {
    reg.register(NodeSpec::new("base.preset", "Factory stock", Category::Band)
        .doc("Attach a bundled factory master or its drafted sand master; dimensions stay in millimetres.")
        .input(PinSpec::item("design", ValueKind::Design).optional().doc("Design to receive the factory stock."))
        .input(PinSpec::select("id", PRESETS.iter().map(|p| p.id.into()).collect()).default("001").doc("Factory stock number."))
        .input(PinSpec::item("sand", ValueKind::Bool).optional().doc("Use the drafted sand master; defaults to the stock's plan symmetry."))
        .input(PinSpec::item("preserve_parameters", ValueKind::Bool).default(false).doc("Keep the incoming band's dimensions and profile when attaching."))
        .input(PinSpec::item("face_length_mm", ValueKind::Number).optional().widget(Widget::Mm { min: 2.0, max: 40.0 }).doc("Override face length, mm."))
        .input(PinSpec::item("face_width_mm", ValueKind::Number).optional().widget(Widget::Mm { min: 2.0, max: 40.0 }).doc("Override face width, mm."))
        .input(PinSpec::item("bore_mm", ValueKind::Number).optional().doc("Override the nominal finger bore, mm."))
        .input(PinSpec::item("chart", ValueKind::Json).optional().doc("Authored relief chart; current stock section when absent."))
        .input(PinSpec::item("chart_enabled", ValueKind::Bool).default(true).doc("Carry a relief chart."))
        .input(PinSpec::item("bare", ValueKind::Bool).default(false).doc("Hide ornament for stock inspection."))
        .input(PinSpec::item("sand_envelope", ValueKind::Bool).optional().doc("Support relief for a two-part pull; defaults to the chosen sand mode."))
        .output(PinSpec::item("design", ValueKind::Design).doc("The design on factory stock."))
        .eval(preset)).expect("unique");
}
