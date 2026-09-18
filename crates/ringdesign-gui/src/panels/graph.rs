//! The Graph pane: the node editor over the design's graph.

use egui_phosphor::regular as icon;

use crate::app::RingDesignerApp;
use crate::theme;

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    if app.graph_ed.is_none() {
        empty_state(app, ui);
        return;
    }
    let mut parameters_changed = false;
    egui::Panel::top(egui::Id::new(("graph_bar", pane)))
        .frame(egui::Frame::NONE.fill(theme::PANEL).inner_margin(egui::Margin::symmetric(6, 3)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(format!("{} Arrange", icon::ARROWS_OUT_LINE_HORIZONTAL)).on_hover_text("Lay the nodes out by depth").clicked() {
                    app.arrange_graph();
                }
                if ui.button(format!("{} Fit", icon::ARROWS_IN)).on_hover_text("Fit the whole graph in view").clicked() {
                    if let Some(ed) = app.graph_ed.as_mut() {
                        ed.fit();
                    }
                }
                let selection = app.graph_ed.as_ref().map(|ed| ed.selected_nodes(ui.ctx())).unwrap_or_default();
                if ui.add_enabled(!selection.is_empty(), egui::Button::new(format!("{} Collapse {}", icon::PACKAGE, selection.len()))).on_hover_text("Fold the selected nodes into one cluster node").clicked() {
                    app.collapse_nodes(&selection);
                }
                if ui.button(format!("{} Bake", icon::FIRE)).on_hover_text("Drop the graph; keep the design as last evaluated").clicked() {
                    app.bake_graph();
                    return;
                }
                let nodes = app.graph_ed.as_ref().map(|e| e.graph().nodes.len()).unwrap_or(0);
                ui.weak(format!("{nodes} nodes"));
                if !app.graph_errors.is_empty() {
                    ui.colored_label(theme::WARN, format!("{} {}", icon::WARNING, app.graph_errors.join("; ")));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak("right-click: add node • drag pins to wire • click a node to inspect it");
                });
            });
            // Step through the flow without hunting for each node on the canvas.
            if let Some(ed) = app.graph_ed.as_mut() {
                ui.scope(|ui| {
                    ui.set_max_width(520.0);
                    if let Some(id) = ringdesign_graph_ui::navigator(ed, ui).moved {
                        app.selected_node = Some(id);
                    }
                });
            }
            if let Some(ed) = app.graph_ed.as_mut() {
                if !ed.graph().exposed.is_empty() {
                    egui::CollapsingHeader::new("Parameters").default_open(true).show(ui, |ui| {
                        parameters_changed = ed.parameters_ui(&app.graph_reg, ui);
                    });
                }
            }
        });
    let reg = app.graph_reg.clone();
    // The editor leaves the app while it draws, as the dock's tree does, so
    // the response can act on the app afterwards.
    let Some(mut ed) = app.graph_ed.take() else { return };
    let resp = egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG))
        .show(ui, |ui| ed.show(&reg, ui, &format!("graph-pane-{pane}")))
        .inner;
    app.graph_ed = Some(ed);
    if resp.selected != app.selected_node {
        app.selected_node = resp.selected;
    }
    if let Some(r) = resp.refused {
        app.set_status(format!("Wire refused: {r}"));
    }
    if resp.changed || parameters_changed {
        app.graph_changed();
    }
}

fn empty_state(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new(format!("{} No graph behind this design yet", icon::GRAPH)).size(16.0).color(theme::TEXT_DIM));
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Convert the design into nodes you can rewire, or start from a graph.").color(theme::TEXT_DIM));
        ui.add_space(16.0);
        if ui.button(format!("{} Convert this design to a graph", icon::ARROW_RIGHT)).clicked() {
            app.convert_to_graph();
        }
        ui.add_space(6.0);
        if ui.button(format!("{} Start from the simple graph", icon::PLUS)).clicked() {
            app.open_graph(ringdesign_graph::templates::simple());
        }
        ui.add_space(6.0);
        ui.menu_button(format!("{} Open a template graph", icon::FOLDER_OPEN), |ui| {
            for template in ringdesign_graph::templates::catalog() {
                if ui.button(template.name).clicked() {
                    match template.instantiate(&app.graph_reg, &app.lib) {
                        Ok(design) => {
                            app.design = design;
                            app.selected_layer = None;
                            let restored = app.design.clone();
                            restored.unpack_embedded(app.library_mut());
                            restored.bake_all(app.library_mut());
                            app.sync_graph();
                            app.arrange_graph();
                            app.show_graph_pane();
                            app.mark_dirty();
                        }
                        Err(e) => app.set_status(format!("Could not open template: {e}")),
                    }
                    ui.close();
                }
            }
        });
    });
}
