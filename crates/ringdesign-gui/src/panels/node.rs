//! The Node tool: the selected node's pins, values and diagnostics.

use egui_phosphor::regular as icon;
use ringdesign_graph::value::Literal;
use ringdesign_graph_ui::widgets::{kind_color, pin_widget};

use crate::app::RingDesignerApp;
use crate::theme;

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let Some(ed) = app.graph_ed.as_ref() else {
        ui.weak("No graph. Open the Graph pane to convert the design or start one.");
        return;
    };
    let Some(id) = app.selected_node else {
        ui.weak("Click a node in the Graph pane to inspect it.");
        return;
    };
    let Some(card) = ed.card(id).cloned() else {
        ui.weak("The selected node is gone.");
        return;
    };
    let Some(node) = ed.node(id).cloned() else { return };
    let graph = ed.graph().clone();
    let reg = app.graph_reg.clone();

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&card.title).strong());
        ui.weak(format!("{}  {}", card.kind, id));
    });
    if !card.doc.is_empty() {
        ui.label(egui::RichText::new(&card.doc).small().color(theme::TEXT_DIM));
    }
    let mut label = card.label.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Label");
        if ui.add(egui::TextEdit::singleline(&mut label).desired_width(140.0)).lost_focus() {
            if let Some(ed) = app.graph_ed.as_mut() {
                if ed.set_label(id, Some(label.clone())) {
                    app.graph_changed();
                }
            }
        }
    });
    if !card.diag.is_empty() {
        ui.separator();
        for d in &card.diag {
            ui.colored_label(theme::WARN, format!("{} {d}", icon::WARNING));
        }
    }
    ui.separator();
    ui.label(egui::RichText::new("Inputs").strong());
    egui::Grid::new(("node_inputs", id)).num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
        for pin in &card.pins_in {
            ui.label(egui::RichText::new(&pin.name).color(kind_color(pin.kind))).on_hover_text(format!("{}\n{}", pin.kind.label(), pin.doc));
            match graph.wire_into(id, &pin.name) {
                Some(w) => {
                    ui.weak(format!("{} from {}.{}", icon::PLUGS_CONNECTED, w.from, w.out));
                    ui.label("");
                }
                None => {
                    let mut lit: Option<Literal> = node.inputs.get(&pin.name).cloned();
                    let before = lit.clone();
                    let spec = pin.clone();
                    pin_widget(ui, &spec, &mut lit);
                    if lit != before {
                        if let Some(ed) = app.graph_ed.as_mut() {
                            if ed.set_input(id, &pin.name, lit) {
                                app.graph_changed();
                            }
                        }
                    }
                    let exposed = graph.exposed.iter().find(|e| e.node == id && e.input == pin.name).map(|e| e.name.clone());
                    match exposed {
                        Some(name) => {
                            if ui.small_button(format!("{} {name}", icon::PUSH_PIN)).on_hover_text("Exposed on the graph's panel; click to withdraw").clicked() {
                                if let Some(ed) = app.graph_ed.as_mut() {
                                    ed.unexpose(&name);
                                    app.graph_changed();
                                }
                            }
                        }
                        None => {
                            if ui.small_button(icon::PUSH_PIN_SLASH).on_hover_text("Expose this input on the graph's panel").clicked() {
                                if let Some(ed) = app.graph_ed.as_mut() {
                                    let name = pretty(&pin.name);
                                    if ed.expose(id, &pin.name, &name).is_ok() {
                                        app.graph_changed();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ui.end_row();
        }
    });
    if !card.pins_out.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Outputs").strong());
        egui::Grid::new(("node_outputs", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            for pin in &card.pins_out {
                ui.label(egui::RichText::new(&pin.name).color(kind_color(pin.kind))).on_hover_text(format!("{}\n{}", pin.kind.label(), pin.doc));
                match card.values.get(&pin.name) {
                    Some(v) => ui.weak(v),
                    None => ui.weak("—"),
                };
                ui.end_row();
            }
        });
    }
    if !node.params.is_null() {
        ui.separator();
        ui.collapsing("Params", |ui| {
            let text = serde_json::to_string_pretty(&node.params).unwrap_or_default();
            let shown: String = text.lines().take(40).collect::<Vec<_>>().join("\n");
            ui.add(egui::Label::new(egui::RichText::new(shown).monospace().small()).wrap());
        });
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(format!("{} Delete node", icon::TRASH)).clicked() {
            app.delete_selected_node();
        }
        let _ = &reg;
    });
}

/// `width_mm` -> `Width`.
fn pretty(key: &str) -> String {
    let base = key.trim_end_matches("_mm").trim_end_matches("_deg").replace('_', " ");
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => key.to_string(),
    }
}
