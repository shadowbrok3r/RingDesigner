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
            Some(egui::vec2(72.0, ui.spacing().interact_size.y)),
            egui::RectAlign::BOTTOM_START,
            &[egui::RectAlign::BOTTOM_END],
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                if file_action(ui, "New design...").clicked() {
                    self.show_templates();
                }
                if file_action(ui, "Open saved designs...").clicked() {
                    self.show_files();
                }
                ui.separator();
                self.file_entries(ui, host);
                ui.separator();
                if file_action(ui, "Rename, export & share...").clicked() {
                    self.show_files();
                }
                ui.separator();
                if file_action(ui, "Feature request / bug report...").clicked() { ringdesign_workbench::feedback::open(ui.ctx()); }
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
                self.status = format!("saving {}…", path.display());
                self.save_to(path, crate::worker::Purpose::Save);
            }
        }
        if file_action(ui, "Save a copy to Downloads")
            .on_hover_text("A copy in shared storage that survives uninstalling the app")
            .clicked()
        {
            let name = format!("{}.ring.json", slug(&self.design.name));
            let copies = root.join("exports");
            let _ = std::fs::create_dir_all(&copies);
            self.status = "saving a copy…".into();
            self.save_to(copies.join(&name), crate::worker::Purpose::Downloads);
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

    pub(super) fn show_templates(&mut self) {
        self.show_files();
        self.tab = Tab::Templates;
    }

    pub(super) fn templates_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let back = ui.button("Back to ring");
            crate::editor::layout::record(ui, "templates/Back to ring", back.rect);
            if back.clicked() { self.tab = Tab::Ring; }
        });
        match crate::editor::catalog::show(ui) {
            Some(crate::editor::catalog::Choice::Blank) => {
                self.load_template_design(RingDesign::default(), "new blank design");
                self.show_new_design();
            }
            Some(crate::editor::catalog::Choice::Template(template)) => {
                let opening = template.open(self.graph.reg.clone(), self.lib.clone(), Self::opening_wake(ui.ctx()));
                self.start_opening(opening, Lands::Template { new_design: true });
            }
            None => {}
        }
    }


    pub(super) fn show_new_design(&mut self) {
        self.editor.isolate = false;
        self.editor.palette = None;
        self.editor.mode = Mode::Shape;
        self.editor.sheet = Some(Sheet::Edit);
        self.tab = Tab::Ring;
    }

    /// The part files on offer from the app's imports and exports and from Downloads, listed again at most every two seconds while shown.
    fn part_files(&mut self, root: &std::path::Path) -> Vec<crate::import::PartFile> {
        if self.part_files.0.is_none_or(|at| at.elapsed() > Duration::from_secs(2)) {
            let folders = [("App imports", root.join("imports")), ("App exports", root.join("exports")), ("Downloads", std::path::PathBuf::from(crate::import::DOWNLOADS))];
            self.part_files = (Some(Instant::now()), crate::import::candidates(&folders));
        }
        self.part_files.1.clone()
    }

    /// The Files tab's import rows: every part file found with its Import, and where parts are looked for.
    pub(super) fn import_rows(&mut self, ui: &mut egui::Ui, root: &std::path::Path) {
        let busy = self.importing.is_some();
        let files = self.part_files(root);
        if files.is_empty() {
            ui.label(egui::RichText::new("No STL, OBJ or STEP in the app's imports or exports, or in Downloads").small().weak());
        }
        for f in files {
            let too_big = crate::import::too_big(&f.name, f.bytes);
            ui.horizontal_wrapped(|ui| {
                let import = ui.add_enabled(!busy && too_big.is_none(), egui::Button::new(format!("Import {}", f.name)));
                crate::editor::layout::record(ui, format!("file/Import {}", f.name), import.rect);
                if import.clicked() {
                    self.status = format!("Reading {}…", f.name);
                    self.importing = Some(crate::import::spawn(f.path.clone(), ui.ctx().clone()));
                }
                let size = if f.bytes < 1024 * 1024 { format!("{:.0} KB", f.bytes as f64 / 1024.0) } else { format!("{:.1} MB", f.bytes as f64 / 1048576.0) };
                let note = if too_big.is_some() { " · too big to read here" } else { "" };
                ui.label(egui::RichText::new(format!("{} · {size}{note}", f.folder)).small().weak());
            });
        }
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("reading the part…").small().weak());
            });
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        ui.label(egui::RichText::new(format!("A part lands at the top of the ring, joined, as one undo step. Parts are looked for in {} and Downloads.", root.join("imports").display())).small().weak());
    }

    /// A part file read off the UI thread lands: joined at the top of the ring as one undo step, and the ring shown.
    pub(super) fn poll_import(&mut self, host: &Host) {
        let Some(rx) = self.importing.as_ref() else { return };
        let read = match rx.try_recv() {
            Ok(read) => read,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.importing = None;
                return;
            }
        };
        self.importing = None;
        let (feature, notes) = match read.part {
            Ok(part) => part,
            Err(why) => {
                self.status = why;
                host.haptic(Haptic::Error);
                return;
            }
        };
        let name = feature.name.clone();
        let edits = ringdesign_core::cad::stored::import_edits(&self.design, feature);
        let Some(added) = self.cad_edit(&edits, crate::cad::Then::LastAdded) else {
            host.haptic(Haptic::Error);
            return;
        };
        self.isolate_made(&added);
        let said: String = notes.iter().map(|n| format!(" • {n}")).collect();
        self.status = format!("Imported {name} at the top of the ring, joined: drag its arrows to move it{said}");
        self.tab = Tab::Ring;
        host.haptic(Haptic::Success);
    }
}
