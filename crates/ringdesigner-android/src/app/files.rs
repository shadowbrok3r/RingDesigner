//! File commands shared by the top bar and the saved-design browser.
use super::*;

fn file_action(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let response = ui.button(label);
    crate::editor::layout::record(ui, format!("file/{label}"), response.rect);
    response
}

impl RingApp {
    pub(super) fn header_file_menu(&mut self, ui: &mut egui::Ui, host: &Host) {
        let response = crate::theme::menu_popup(
            ui,
            "File",
            Some(egui::vec2(60.0, 32.0)),
            egui::RectAlign::BOTTOM_START,
            &[egui::RectAlign::BOTTOM_END],
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                let new = ui.menu_button("New design", |ui| {
                    crate::theme::menu_row_style(ui);
                    let height = (crate::theme::content_bounds(ui.ctx()).height() * 0.65)
                        .clamp(100.0, 440.0);
                    crate::theme::scroll_vertical()
                        .max_height(height)
                        .show(ui, |ui| {
                            self.new_design_entries(ui);
                        });
                });
                crate::editor::layout::record(ui, "file/New design", new.response.rect);
                if file_action(ui, "Open saved designs...").clicked() {
                    self.show_files();
                }
                ui.separator();
                self.file_entries(ui, host);
                ui.separator();
                if file_action(ui, "Rename, export & share...").clicked() {
                    self.show_files();
                }
            },
        );
        crate::editor::layout::record(ui, "header/File", response.rect);
    }

    fn show_files(&mut self) {
        if self.editor.isolate {
            self.editor.isolate = false;
            self.request_view_update();
        }
        self.editor.hold_before = false;
        self.editor.palette = None;
        self.tab = Tab::Files;
    }

    pub(super) fn file_entries(&mut self, ui: &mut egui::Ui, host: &Host) {
        let Some(root) = self.data_root.clone() else {
            ui.label("No writable directory");
            return;
        };
        let designs = root.join("designs");
        if file_action(ui, "Save").clicked() {
            let path = crate::util::design_path(&designs, &self.design.name);
            let _ = std::fs::create_dir_all(&designs);
            // Two designs both called "untitled" slug to one path, so a
            // save used to overwrite the other silently. Say so once and
            // let the second tap through.
            if path.exists() && self.overwrite_warned.as_ref() != Some(&path) {
                self.overwrite_warned = Some(path.clone());
                self.status = format!(
                    "{} already exists — Save again to replace it",
                    self.design.name
                );
                host.haptic(Haptic::Warning);
            } else {
                self.overwrite_warned = None;
                self.status = match library::save_design(&path, &self.design) {
                    Ok(()) => {
                        self.prefs.push_recent(&path.to_string_lossy());
                        self.save_prefs();
                        format!("saved {}", path.display())
                    }
                    Err(e) => format!("save failed: {e}"),
                };
                host.haptic(Haptic::Success);
            }
        }
        if file_action(ui, "Save a copy to Downloads")
            .on_hover_text("A copy in shared storage that survives uninstalling the app")
            .clicked()
        {
            let name = format!("{}.ring.json", slug(&self.design.name));
            let copies = root.join("exports");
            let path = copies.join(&name);
            let _ = std::fs::create_dir_all(&copies);
            self.status = match library::save_design(&path, &self.design) {
                Ok(()) => match host.save_to_gallery(
                    path.to_string_lossy().into_owned(),
                    name,
                    "application/json",
                ) {
                    Some(folder) => format!("copy saved to {folder}"),
                    None => "could not write to Downloads".into(),
                },
                Err(e) => format!("save failed: {e}"),
            };
            host.haptic(Haptic::Success);
        }
        if file_action(ui, "Copy design as JSON").clicked() {
            if let Ok(json) = serde_json::to_string_pretty(&self.design) {
                host.copy_text(json);
                self.status = "design copied as JSON".into();
                host.haptic(Haptic::Success);
            }
        }
        if file_action(ui, "Paste design").clicked() {
            match host
                .clipboard_text()
                .and_then(|t| serde_json::from_str::<RingDesign>(&t).ok())
            {
                Some(d) => {
                    self.adopt(d);
                    self.status = "design pasted".into();
                    host.haptic(Haptic::Success);
                }
                None => {
                    self.status = "clipboard is not a design".into();
                    host.haptic(Haptic::Error);
                }
            }
        }
    }

    pub(super) fn new_design_entries(&mut self, ui: &mut egui::Ui) {
        if file_action(ui, "Blank band").clicked() {
            self.load_template_design(RingDesign::default(), "new blank design");
            self.show_new_design();
            ui.close();
        }
        if file_action(ui, "Imported signet base...").clicked() {
            let preset = &ringdesign_core::imported_base::PRESETS[0];
            let mut design = RingDesign::default();
            match preset.load().and_then(|source| {
                ringdesign_core::imported_base::ImportedBase::attach(&mut design, source)
            }) {
                Ok(()) => {
                    design.name = "Imported signet".into();
                    self.load_template_design(design, preset.name);
                    self.show_new_design();
                    ui.close();
                }
                Err(e) => self.status = format!("could not open base: {e}"),
            }
        }
        ui.separator();
        ui.weak("Stock masterworks");
        for template in ringdesign_graph::templates::IMPORTED {
            self.new_graph_entry(ui, template);
        }
        ui.separator();
        ui.weak("Showcase templates");
        for template in ringdesign_graph::templates::SHOWCASE {
            self.new_graph_entry(ui, template);
        }
        ui.separator();
        ui.weak("Starter designs");
        for template in ringdesign_core::templates::all() {
            if file_action(ui, template.name)
                .on_hover_text(template.blurb)
                .clicked()
            {
                self.load_template_design(template.design(), template.name);
                self.show_new_design();
                ui.close();
            }
        }
    }

    fn new_graph_entry(
        &mut self,
        ui: &mut egui::Ui,
        template: &ringdesign_graph::templates::TemplateGraph,
    ) {
        if file_action(ui, template.name).clicked() {
            match template.instantiate(&self.graph.reg, &self.lib) {
                Ok(design) => {
                    self.load_template_design(design, template.name);
                    self.graph.sync(&self.design);
                    if let Some(editor) = &mut self.graph.ed {
                        editor.arrange(&self.graph.reg);
                    }
                    self.show_new_design();
                    ui.close();
                }
                Err(e) => self.status = format!("could not open template: {e}"),
            }
        }
    }

    fn show_new_design(&mut self) {
        self.editor.isolate = false;
        self.editor.palette = None;
        self.editor.mode = Mode::Shape;
        self.editor.sheet = Some(Sheet::Edit);
        self.tab = Tab::Ring;
    }
}
