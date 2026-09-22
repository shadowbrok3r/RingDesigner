//! The inspector for a part a builder makes: the stone it stands on, then every parameter the builder's
//! schema names, each drawn with its own widget, unit and range. A parameter the part leaves unset is sized
//! from its stone at evaluation, and says so.
use crate::theme;
use ringdesign_core::cad::builders::{self, Kind, Param};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_graph::graph::NodeId;
use ringdesign_workbench::controls;
use serde_json::{Value, json};

/// Draws the builder's inspector and edits `on` and `params` in place.
pub fn ui(ui: &mut egui::Ui, key: &str, on: &mut Option<u64>, params: &mut Value, tree: &[(NodeId, String)]) {
    let Some(spec) = builders::spec(key) else {
        ui.colored_label(theme::BAD, format!("No builder called {key}"));
        return;
    };
    ui.weak(spec.hint);
    if spec.on_stone {
        let shown = on
            .and_then(|id| tree.iter().find(|(n, _)| n.0 == id).map(|(_, name)| format!("#{id} {name}")))
            .unwrap_or_else(|| "Choose a stone".into());
        controls::row(ui, "Stone", |ui| {
            egui::ComboBox::from_id_salt("builder-stone").selected_text(shown).show_ui(ui, |ui| {
                for (id, name) in tree {
                    if ui.selectable_label(*on == Some(id.0), format!("#{} {name}", id.0)).clicked() {
                        *on = Some(id.0);
                    }
                }
            });
        });
    }
    if !params.is_object() {
        *params = json!({});
    }
    // A stone's own values are its gem; a setting's unset ones are sized from its stone when it is evaluated.
    let stone = key == builders::STONE;
    let gem = if stone { builders::gem_of(params).unwrap_or_default() } else { Gem::calibrated(GemCut::Round, 6.5) };
    for p in builders::schema(key, gem) {
        param(ui, &p, params, !stone);
    }
    if let Err(e) = builders::check_params(key, gem, params) {
        ui.colored_label(theme::WARN, e);
    }
}

/// One parameter's row: its widget when the part sets it, with Auto to unset it; else "auto" and Set.
fn param(ui: &mut egui::Ui, p: &Param, params: &mut Value, auto: bool) {
    let label = if p.unit.is_empty() { p.label.to_string() } else { format!("{} ({})", p.label, p.unit) };
    let Some(map) = params.as_object_mut() else { return };
    let Some(value) = map.get(p.key).cloned() else {
        controls::row(ui, &label, |ui| {
            if ui.small_button("Set").on_hover_text("Write this value into the part; unset, it is sized from the stone").clicked() {
                map.insert(p.key.to_string(), p.default.clone());
            }
            ui.weak("auto");
        });
        return;
    };
    let (mut next, mut unset) = (None, false);
    controls::row(ui, &label, |ui| {
        if auto && ui.small_button("Auto").on_hover_text("Size it from the stone again").clicked() {
            unset = true;
        }
        match p.kind {
            Kind::Number => {
                let mut v = value.as_f64().unwrap_or(p.min);
                let before = v;
                let r = ui.add(egui::DragValue::new(&mut v).range(p.min..=p.max).speed(0.01).max_decimals(3));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, &label));
                if v != before {
                    next = Some(json!(v));
                }
            }
            Kind::Whole if p.max - p.min <= 8.0 => {
                let current = value.as_f64().unwrap_or(p.min).round() as i64;
                for n in (p.min as i64..=p.max as i64).rev() {
                    let r = ui.selectable_label(current == n, n.to_string());
                    let name = format!("{} {n}", p.label);
                    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, current == n, &name));
                    if r.clicked() && current != n {
                        next = Some(json!(n));
                    }
                }
            }
            Kind::Whole => {
                let mut v = value.as_f64().unwrap_or(p.min).round() as i64;
                let before = v;
                let r = ui.add(egui::DragValue::new(&mut v).range(p.min as i64..=p.max as i64));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, &label));
                if v != before {
                    next = Some(json!(v));
                }
            }
            Kind::Choice(names) => {
                let current = value.as_str().unwrap_or("").to_string();
                egui::ComboBox::from_id_salt(("builder-choice", p.key)).selected_text(current.clone()).show_ui(ui, |ui| {
                    for name in names {
                        if ui.selectable_label(current == *name, *name).clicked() && current != *name {
                            next = Some(json!(name));
                        }
                    }
                });
            }
            Kind::Flag => {
                let mut v = value.as_bool().unwrap_or(false);
                let r = ui.checkbox(&mut v, "");
                r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, v, &label));
                if r.changed() {
                    next = Some(json!(v));
                }
            }
        }
    });
    if unset {
        map.remove(p.key);
    } else if let Some(v) = next {
        map.insert(p.key.to_string(), v);
    }
}
