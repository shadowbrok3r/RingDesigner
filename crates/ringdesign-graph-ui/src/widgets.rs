//! Inline widgets for unwired pins, chosen from the registry's hints.

use egui::Ui;
use ringdesign_graph::registry::{PinSpec, Widget};
use ringdesign_graph::value::{Literal, ValueKind};

/// Draw the widget for an unwired pin over its literal. Returns whether
/// the literal changed.
pub fn pin_widget(ui: &mut Ui, pin: &PinSpec, literal: &mut Option<Literal>) -> bool {
    let effective = literal.clone().or_else(|| pin.default.clone());
    if let Some(Literal::List(items)) = &effective {
        ui.weak(format!("list ×{}", items.len()));
        return false;
    }
    if let Some(Literal::Json(_)) = &effective {
        ui.weak("json");
        return false;
    }
    let mut changed = false;
    match (&pin.widget, pin.kind) {
        (Widget::Checkbox, _) | (Widget::Auto, ValueKind::Bool) => {
            let mut v = effective.as_ref().and_then(as_bool).unwrap_or(false);
            if ui.checkbox(&mut v, "").changed() {
                *literal = Some(Literal::Bool(v));
                changed = true;
            }
        }
        (Widget::Select(names), _) => {
            let mut v = effective.as_ref().and_then(as_text).unwrap_or_default();
            let id = ui.id().with(&pin.name);
            egui::ComboBox::from_id_salt(id).selected_text(if v.is_empty() { "…".to_string() } else { v.clone() }).width(110.0).show_ui(ui, |ui| {
                for n in names {
                    if ui.selectable_value(&mut v, n.clone(), n).changed() {
                        changed = true;
                    }
                }
            });
            if changed {
                *literal = Some(Literal::Text(v));
            }
        }
        (Widget::Slider { min, max }, _) => {
            let mut v = effective.as_ref().and_then(as_f64).unwrap_or(*min);
            if ui.add_sized([120.0, 18.0], egui::Slider::new(&mut v, *min..=*max).show_value(true)).changed() {
                *literal = Some(number_like(pin.kind, v));
                changed = true;
            }
        }
        (Widget::Mm { min, max }, _) => {
            let mut v = effective.as_ref().and_then(as_f64).unwrap_or(*min);
            if ui.add_sized([90.0, 18.0], egui::DragValue::new(&mut v).range(*min..=*max).speed(0.05).suffix(" mm").fixed_decimals(2)).changed() {
                *literal = Some(number_like(pin.kind, v));
                changed = true;
            }
        }
        (Widget::Angle, _) => {
            let mut v = effective.as_ref().and_then(as_f64).unwrap_or(90.0);
            if ui.add_sized([80.0, 18.0], egui::DragValue::new(&mut v).speed(0.5).suffix("°").fixed_decimals(1)).changed() {
                *literal = Some(number_like(pin.kind, v));
                changed = true;
            }
        }
        (Widget::TextLine, _) | (Widget::Auto, ValueKind::Text | ValueKind::AlphaRef) => {
            let mut v = effective.as_ref().and_then(as_text).unwrap_or_default();
            if ui.add_sized([120.0, 18.0], egui::TextEdit::singleline(&mut v)).changed() {
                *literal = Some(Literal::Text(v));
                changed = true;
            }
        }
        (Widget::TextArea, _) => {
            let mut v = effective.as_ref().and_then(as_text).unwrap_or_default();
            if ui.add_sized([160.0, 60.0], egui::TextEdit::multiline(&mut v)).changed() {
                *literal = Some(Literal::Text(v));
                changed = true;
            }
        }
        (Widget::Auto, ValueKind::Int) => {
            let mut v = effective.as_ref().and_then(as_i64).unwrap_or(0);
            if ui.add_sized([70.0, 18.0], egui::DragValue::new(&mut v).speed(0.1)).changed() {
                *literal = Some(Literal::Int(v));
                changed = true;
            }
        }
        (Widget::Auto, ValueKind::Number) => {
            let mut v = effective.as_ref().and_then(as_f64).unwrap_or(0.0);
            if ui.add_sized([80.0, 18.0], egui::DragValue::new(&mut v).speed(0.05).max_decimals(4)).changed() {
                *literal = Some(Literal::Number(v));
                changed = true;
            }
        }
        _ => {
            // A handle: nothing to type in; the pin wants a wire.
            ui.weak("—");
        }
    }
    changed
}

fn number_like(kind: ValueKind, v: f64) -> Literal {
    if kind == ValueKind::Int { Literal::Int(v.round() as i64) } else { Literal::Number(v) }
}

fn as_f64(l: &Literal) -> Option<f64> {
    match l {
        Literal::Number(x) => Some(*x),
        Literal::Int(i) => Some(*i as f64),
        Literal::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn as_i64(l: &Literal) -> Option<i64> {
    match l {
        Literal::Int(i) => Some(*i),
        Literal::Number(x) => Some(x.round() as i64),
        _ => None,
    }
}

fn as_bool(l: &Literal) -> Option<bool> {
    match l {
        Literal::Bool(b) => Some(*b),
        Literal::Int(i) => Some(*i != 0),
        Literal::Number(x) => Some(*x != 0.0),
        _ => None,
    }
}

fn as_text(l: &Literal) -> Option<String> {
    match l {
        Literal::Text(s) => Some(s.clone()),
        Literal::Number(x) => Some(x.to_string()),
        Literal::Int(i) => Some(i.to_string()),
        Literal::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The colour a pin of this kind draws in.
pub fn kind_color(kind: ValueKind) -> egui::Color32 {
    use ValueKind::*;
    match kind {
        Any => egui::Color32::from_rgb(170, 170, 170),
        Null => egui::Color32::DARK_GRAY,
        Number => egui::Color32::from_rgb(120, 190, 255),
        Int => egui::Color32::from_rgb(90, 160, 235),
        Bool => egui::Color32::from_rgb(235, 120, 120),
        Text | AlphaRef => egui::Color32::from_rgb(230, 200, 110),
        List | Json | Path => egui::Color32::from_rgb(190, 190, 120),
        Design => egui::Color32::from_rgb(255, 215, 120),
        Profile | Shank | Head | Outline => egui::Color32::from_rgb(255, 170, 90),
        Gem => egui::Color32::from_rgb(140, 230, 230),
        Window | Remap => egui::Color32::from_rgb(200, 160, 255),
        Layer | Entry | Stack | Recipe => egui::Color32::from_rgb(140, 220, 140),
        AlphaSource => egui::Color32::from_rgb(220, 190, 150),
        Build | Mesh | Solid => egui::Color32::from_rgb(200, 200, 210),
        Field | Stones => egui::Color32::from_rgb(255, 140, 140),
    }
}
