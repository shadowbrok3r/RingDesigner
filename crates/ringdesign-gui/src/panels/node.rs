//! The Node tool: the selected node's pins, values and diagnostics.

use egui_phosphor::regular as icon;
use ringdesign_graph::value::Literal;
use ringdesign_graph_ui::widgets::{kind_color, pin_widget};

use crate::app::RingDesignerApp;
use crate::theme;

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let mut changed = false;
    if let Some(ed) = app.graph_ed.as_mut() {
        if !ed.graph().exposed.is_empty() {
            egui::CollapsingHeader::new("Graph parameters").default_open(app.selected_node.is_none()).show(ui, |ui| {
                changed = ed.parameters_ui(&app.graph_reg, ui);
            });
        }
    }
    if changed { app.graph_changed(); }
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
    let Some(params) = ed.node(id).map(|node| node.params.clone()) else { return };
    let wires = ed.graph().wires.clone();
    let exposures = ed.graph().exposed.clone();
    let reg = app.graph_reg.clone();

    let mut help = false;
    let mut open_group = None;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&card.title).strong());
        ui.weak(format!("{}  {}", card.kind, id));
        help = ui.small_button(format!("{} Help", icon::QUESTION)).on_hover_text("What this node does, what it takes and gives, and what connects to it (F1 on the canvas)").clicked();
    });
    match ringdesign_graph::guide::node(&card.kind) {
        Some(guide) => {
            ui.label(egui::RichText::new(guide.line()).small().color(theme::TEXT_DIM));
        }
        None if !card.doc.is_empty() => {
            ui.label(egui::RichText::new(&card.doc).small().color(theme::TEXT_DIM));
        }
        None => {}
    }
    if let Some(index) = ed.group_of(id) {
        let group = ed.graph().groups[index].name.clone();
        let inside = ed.entered_group().is_some_and(|g| g.name == group);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} In the group {group}", icon::SELECTION)).small().color(theme::TEXT_DIM));
            if !inside && ui.small_button("Open group").clicked() {
                open_group = Some(index);
            }
        });
    }
    if help {
        if let Some(ed) = app.graph_ed.as_mut() {
            ed.show_help(&card.kind);
        }
    }
    if let Some(index) = open_group {
        if let Some(ed) = app.graph_ed.as_mut() {
            ed.open_group(Some(index));
            ed.focus(id);
        }
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
    ui.vertical(|ui| {
        for pin in &card.pins_in {
            ui.add_space(6.0);
            let label = ui.label(egui::RichText::new(pretty(&pin.name)).color(kind_color(pin.kind))).on_hover_text(format!("{}\n{}", pin.kind.label(), pin.doc)).id;
            ui.horizontal_wrapped(|ui| {
            match wires.iter().find(|wire| wire.to == id && wire.input == pin.name) {
                Some(w) => {
                    ui.weak(format!("{} from {}.{}", icon::PLUGS_CONNECTED, w.from, w.out));
                    ui.label("");
                }
                None => {
                    let mut lit: Option<Literal> = card.inputs.get(&pin.name).cloned();
                    let spec = pin.clone();
                    if ui.label_inputs_by(label, |ui| pin_widget(ui, &spec, &mut lit)) {
                        if let Some(ed) = app.graph_ed.as_mut() {
                            if ed.set_input(id, &pin.name, lit) {
                                app.graph_changed();
                            }
                        }
                    }
                    let exposed = exposures.iter().find(|e| e.node == id && e.input == pin.name).map(|e| e.name.clone());
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
            });
            ui.separator();
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
    if !params.is_null() {
        ui.separator();
        ui.collapsing("Params", |ui| {
            let text = serde_json::to_string_pretty(&params).unwrap_or_default();
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
    if key == "png_base64" { return "Image".into(); }
    let base = key.trim_end_matches("_mm").trim_end_matches("_deg").replace('_', " ");
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => key.to_string(),
    }
}
