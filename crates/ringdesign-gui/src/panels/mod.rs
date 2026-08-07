//! Window layout and the top-level chrome.

pub mod design;
pub mod layers;
pub mod library;
pub mod report;
pub mod section;
pub mod unrolled;

use egui_phosphor::regular as icon;

use crate::app::{RingDesignerApp, Tab};
use crate::camera::StandardView;
use crate::viewport;
use crate::{export, theme};

pub fn render(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    egui::Panel::top(egui::Id::new("toolbar")).show(ui, |ui| toolbar(app, ui));
    egui::Panel::bottom(egui::Id::new("status")).show(ui, |ui| status_bar(app, ui));

    egui::Panel::left(egui::Id::new("left"))
        .default_size(316.0)
        .size_range(egui::Rangef::new(266.0, 460.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                design::ui(app, ui);
                ui.add_space(6.0);
                ui.separator();
                layers::ui(app, ui);
            });
        });

    egui::Panel::right(egui::Id::new("right"))
        .default_size(318.0)
        .size_range(egui::Rangef::new(268.0, 460.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                report::ui(app, ui);
                ui.add_space(6.0);
                ui.separator();
                library::ui(app, ui);
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::VIEWPORT_BG))
        .show(ui, |ui| match app.tab {
            Tab::Solid => viewport::ui(app, ui),
            Tab::Unrolled => unrolled::ui(app, ui),
            Tab::Section => section::ui(app, ui),
        });
}

fn toolbar(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        ui.menu_button(format!("{} File", icon::FOLDER_OPEN), |ui| {
            if ui.button(format!("{} New", icon::FILE_PLUS)).clicked() {
                app.design = ringdesign_core::RingDesign::default();
                app.selected_layer = None;
                app.mark_dirty();
                ui.close();
            }
            if ui.button(format!("{} Open…", icon::FOLDER_OPEN)).clicked() {
                export::open_design(app);
                ui.close();
            }
            if ui.button(format!("{} Save As…", icon::FLOPPY_DISK)).clicked() {
                export::save_design(app);
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} Export STL…", icon::EXPORT)).clicked() {
                export::export_stl(app);
                ui.close();
            }
            if ui.button(format!("{} Export OBJ…", icon::EXPORT)).clicked() {
                export::export_obj(app);
                ui.close();
            }
        });

        ui.separator();

        for &tab in Tab::ALL {
            let selected = app.tab == tab;
            if ui
                .selectable_label(selected, format!("{} {}", tab.icon(), tab.label()))
                .clicked()
            {
                app.tab = tab;
                if tab == Tab::Section {
                    app.refresh_section();
                }
            }
        }

        ui.separator();

        if app.tab == Tab::Solid {
            for &v in StandardView::ALL {
                if ui.small_button(v.label()).clicked() {
                    app.camera.set_view(v);
                }
            }
            if ui
                .small_button(format!("{} Reset", icon::ARROW_COUNTER_CLOCKWISE))
                .clicked()
            {
                app.camera.reset();
                app.camera.fit(app.build.as_ref().and_then(|b| b.mesh.bounds()));
            }
            ui.separator();
            ui.checkbox(&mut app.show_wireframe, "Wire");
            ui.checkbox(&mut app.show_grid, "Grid");
            egui::ComboBox::from_id_salt("shade")
                .selected_text(app.shade.label())
                .width(126.0)
                .show_ui(ui, |ui| {
                    for &m in viewport::ShadeMode::ALL {
                        ui.selectable_value(&mut app.shade, m, m.label());
                    }
                });
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.is_building() {
                ui.add(egui::Spinner::new().size(14.0));
            }
            if ui
                .button(format!("{} Rebuild", icon::ARROWS_CLOCKWISE))
                .on_hover_text("Rebuild the mesh now")
                .clicked()
            {
                app.rebuild_now();
            }
            ui.checkbox(&mut app.auto_rebuild, "Auto");

            let mut preset = current_preset(app);
            egui::ComboBox::from_id_salt("quality")
                .selected_text(preset_label(app))
                .width(120.0)
                .show_ui(ui, |ui| {
                    for (i, (name, t, p)) in
                        ringdesign_core::mesh::BuildParams::PRESETS.iter().enumerate()
                    {
                        if ui
                            .selectable_value(&mut preset, i, format!("{name} • {}k tris", t * p * 2 / 1000))
                            .clicked()
                        {
                            app.preview_params.theta_steps = *t;
                            app.preview_params.profile_steps = *p;
                            app.mark_dirty();
                        }
                    }
                });
            ui.label(egui::RichText::new("Preview").color(theme::TEXT_DIM));

            ui.separator();
            mcp_control(app, ui);
        });
    });
    ui.add_space(3.0);
}

/// Start/stop toggle for the embedded MCP server.
fn mcp_control(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    if let Some(addr) = app.mcp_addr() {
        if ui
            .button(egui::RichText::new(format!("{} MCP", icon::PLUGS_CONNECTED)).color(theme::GOOD))
            .on_hover_text(format!("Serving http://{addr}/ — click to stop"))
            .clicked()
        {
            app.stop_mcp();
        }
        ui.label(egui::RichText::new(addr.to_string()).color(theme::TEXT_DIM));
        return;
    }

    if ui
        .button(format!("{} MCP", icon::PLUGS))
        .on_hover_text("Serve this design to an agent over MCP on 127.0.0.1")
        .clicked()
    {
        app.start_mcp();
    }
    ui.add(
        egui::DragValue::new(&mut app.mcp_port)
            .range(1..=65535)
            .speed(1.0),
    )
    .on_hover_text("Listen port");

    if let Some(err) = app.mcp_error.clone() {
        let short: String = err.chars().take(34).collect();
        ui.label(egui::RichText::new(format!("{} {short}", icon::WARNING)).color(theme::BAD))
            .on_hover_text(err);
    }
}

fn current_preset(app: &RingDesignerApp) -> usize {
    ringdesign_core::mesh::BuildParams::PRESETS
        .iter()
        .position(|(_, t, p)| *t == app.preview_params.theta_steps && *p == app.preview_params.profile_steps)
        .unwrap_or(usize::MAX)
}

fn preset_label(app: &RingDesignerApp) -> String {
    match current_preset(app) {
        usize::MAX => format!(
            "{}x{}",
            app.preview_params.theta_steps, app.preview_params.profile_steps
        ),
        i => ringdesign_core::mesh::BuildParams::PRESETS[i].0.to_string(),
    }
}

fn status_bar(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let (glyph, color) = match app.cast.as_ref().map(|c| c.verdict) {
            Some(v) => (
                match v {
                    ringdesign_core::castability::Verdict::Castable => icon::CHECK_CIRCLE,
                    _ => icon::WARNING,
                },
                theme::verdict_color(v),
            ),
            None => (icon::CIRCLE_DASHED, theme::TEXT_DIM),
        };
        if let Some(c) = app.cast.as_ref() {
            ui.label(egui::RichText::new(format!("{glyph} {}", c.verdict.label())).color(color));
        } else {
            ui.label(egui::RichText::new(format!("{glyph} —")).color(color));
        }

        ui.separator();
        ui.label(egui::RichText::new(&app.status).color(theme::TEXT_DIM));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(b) = &app.build {
                let r = &b.report;
                ui.label(
                    egui::RichText::new(format!(
                        "{} • {:.1} x {:.1} mm • {:.2} g in 14k",
                        app.design.size.display(),
                        r.outer_diameter_mm,
                        r.band_width_mm,
                        r.metals
                            .iter()
                            .find(|m| m.metal == "Gold 14k")
                            .map(|m| m.grams)
                            .unwrap_or(0.0)
                    ))
                    .color(theme::TEXT_DIM),
                );
            }
        });
    });
    ui.add_space(2.0);
}
