use super::*;
use egui::{Color32, RichText, Stroke, pos2, vec2};
use ringdesign_core::{
    cad::{self, ComponentRole, Feature, Operation},
    castability::{CastProcess, SandProcess},
    manufacturing::{BoreStrategy, Recipe, Setup},
    sketch::Sketch,
};

fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.02).max_decimals(4));
    });
}
fn xyz(ui: &mut egui::Ui, label: &str, v: &mut [f64; 3]) {
    ui.label(label);
    ui.horizontal_wrapped(|ui| {
        for (i, x) in v.iter_mut().enumerate() {
            ui.add(
                egui::DragValue::new(x)
                    .prefix(["X ", "Y ", "Z "][i])
                    .speed(0.02)
                    .max_decimals(3),
            );
        }
    });
}

impl Workshop {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        source: &mut RingDesign,
        lib: &AlphaLibrary,
    ) -> Events {
        self.session.sync(source);
        let mut events = Events::default();
        let mut worker_failed = false;
        if let Some(worker) = &mut self.worker {
            while let Some(done) = worker.poll() {
                match done {
                    Ok(done) => {
                        if let Some(file) = self.session.receive(source, done) {
                            events.files.push(file);
                        }
                    }
                    Err(e) => {
                        self.session.error = Some(e);
                        self.session.busy = false;
                        worker_failed = true;
                    }
                }
            }
        }
        if worker_failed {
            self.worker = None;
        }
        ui.spacing_mut().interact_size.y = 44.0;
        ui.spacing_mut().button_padding = vec2(12.0, 8.0);
        ui.horizontal_wrapped(|ui| {
            ui.heading("Workshop");
            ui.weak(if self.session.draft.is_some() {
                "Candidate"
            } else {
                "Saved design"
            });
            if self.session.busy {
                ui.spinner();
                ui.label("Evaluating…");
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.session.busy, egui::Button::new("Preview"))
                .clicked()
            {
                self.send(ui, source, lib, Action::Inspect);
            }
            if ui
                .add_enabled(
                    self.session.draft.is_some()
                        && self.session.is_current(source)
                        && !self.session.busy,
                    egui::Button::new("Apply"),
                )
                .clicked()
            {
                events.changed |= self.session.apply(source);
            }
            if ui
                .add_enabled(
                    self.session.draft.is_some() || self.session.busy,
                    egui::Button::new("Cancel"),
                )
                .clicked()
            {
                self.session.cancel();
                self.worker = None;
                self.feature_text_id = None;
            }
            if ui
                .add_enabled(!self.session.undo.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                events.changed |= self.session.undo(source);
            }
            if ui
                .add_enabled(!self.session.redo.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                events.changed |= self.session.redo(source);
            }
        });
        if let Some(e) = &self.session.error {
            ui.colored_label(Color32::from_rgb(255, 120, 135), e);
        }
        if !self.message.is_empty() {
            ui.label(&self.message);
        }
        let current = self.session.is_current(source);
        let before = key(self.session.current(source));
        let mut draft = self.session.current(source).clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if ui.available_width() >= 720.0 {
                    ui.columns(2, |cols| {
                        self.preview(&mut cols[0], current);
                        self.controls(&mut cols[1], &mut draft, source, lib);
                    });
                } else {
                    self.preview(ui, current);
                    ui.separator();
                    self.controls(ui, &mut draft, source, lib);
                }
            });
        if key(&draft) != before {
            self.session.draft = Some(draft);
        }
        events
    }

    fn controls(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut RingDesign,
        source: &RingDesign,
        lib: &AlphaLibrary,
    ) {
        ui.horizontal_wrapped(|ui| {
            for (i, label) in ["Casting", "CAD", "Files"].into_iter().enumerate() {
                ui.selectable_value(&mut self.tab, i, label);
            }
        });
        match self.tab {
            0 => self.casting(ui, d),
            1 => self.cad(ui, d),
            _ => self.files(ui, d, source, lib),
        }
    }

    fn casting(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) {
        let mut setup = d
            .manufacturing
            .clone()
            .unwrap_or_else(|| Setup::from_design(d));
        let old = serde_json::to_vec(&setup).unwrap();
        ui.strong("Process and pattern");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut setup.recipe.process,
                CastProcess::SandTwoPart,
                "Two-part sand",
            );
            ui.selectable_value(&mut setup.recipe.process, CastProcess::LostWax, "Lost wax");
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Delft starting recipe").clicked() {
                setup.recipe = Recipe::sand(SandProcess::DelftClay);
            }
            if ui.button("Petrobond starting recipe").clicked() {
                setup.recipe = Recipe::sand(SandProcess::Petrobond);
            }
        });
        egui::ComboBox::from_id_salt("alloy")
            .selected_text(&setup.recipe.alloy)
            .show_ui(ui, |ui| {
                for m in ringdesign_core::metal::METALS {
                    ui.selectable_value(&mut setup.recipe.alloy, m.name.into(), m.name);
                }
            });
        number(ui, "Shrink %", &mut setup.recipe.shrink_pct);
        if let Some(doc) = &d.cad {
            egui::ComboBox::from_id_salt("component")
                .selected_text(
                    setup
                        .component
                        .map_or("Select component".into(), |id| format!("Component #{id}")),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut setup.component, None, "Automatic (one metal part)");
                    for f in doc
                        .features
                        .iter()
                        .filter(|f| doc.outputs.contains(&f.id) && !f.component.reference)
                    {
                        ui.selectable_value(&mut setup.component, Some(f.id), &f.name);
                    }
                });
        }
        ui.collapsing("Pull and parting", |ui| {
            xyz(ui, "Pull direction", &mut setup.pull);
            ui.horizontal_wrapped(|ui| {
                for (label, v) in [
                    ("X", [1., 0., 0.]),
                    ("Y", [0., 1., 0.]),
                    ("Z", [0., 0., 1.]),
                ] {
                    if ui.button(label).clicked() {
                        setup.pull = v;
                    }
                }
            });
            ui.checkbox(&mut setup.auto_parting, "Automatic parting plane");
            ui.add_enabled_ui(!setup.auto_parting, |ui| {
                number(ui, "Parting mm", &mut setup.parting_mm)
            });
            egui::ComboBox::from_id_salt("bore")
                .selected_text(setup.bore.label())
                .show_ui(ui, |ui| {
                    for b in BoreStrategy::ALL {
                        ui.selectable_value(&mut setup.bore, b, b.label());
                    }
                });
        });
        ui.collapsing("Stock, flask, and shop limits", |ui| {
            for (label, v) in [
                ("Radial stock mm", &mut setup.radial_stock_mm),
                ("Side stock mm", &mut setup.axial_stock_mm),
                ("Bore stock mm", &mut setup.bore_stock_mm),
                ("Flask width mm", &mut setup.flask.width_mm),
                ("Flask length mm", &mut setup.flask.length_mm),
                ("Upper depth mm", &mut setup.flask.cope_mm),
                ("Lower depth mm", &mut setup.flask.drag_mm),
                ("Draft target °", &mut setup.recipe.min_draft_deg),
                ("Minimum wall mm", &mut setup.recipe.min_section_mm),
                ("Minimum detail mm", &mut setup.recipe.min_detail_mm),
                ("Sand web mm", &mut setup.recipe.min_sand_web_mm),
                ("Sand margin mm", &mut setup.recipe.sand_margin_mm),
                ("Ray pitch mm", &mut setup.sample_pitch_mm),
            ] {
                number(ui, label, v);
            }
            ui.label("Bench instructions");
            ui.text_edit_multiline(&mut setup.bench_notes);
        });
        ui.collapsing("Channels cut into the mold", |ui| {
            let mut remove = None;
            for (i, channel) in setup.channels.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    egui::ComboBox::from_id_salt("kind")
                        .selected_text(channel.kind.label())
                        .show_ui(ui, |ui| {
                            for k in mf::ChannelKind::ALL {
                                ui.selectable_value(&mut channel.kind, k, k.label());
                            }
                        });
                    xyz(ui, "Start in pull frame", &mut channel.start);
                    xyz(ui, "End in pull frame", &mut channel.end);
                    number(ui, "Diameter mm", &mut channel.diameter_mm);
                    if ui.button("Remove channel").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                setup.channels.remove(i);
            }
            if setup.channels.len() < 128 && ui.button("Add gate").clicked() {
                setup.channels.push(mf::Channel {
                    kind: mf::ChannelKind::Gate,
                    start: [0., 12., 0.],
                    end: [0., 22., 0.],
                    diameter_mm: 3.0,
                });
            }
        });
        if old != serde_json::to_vec(&setup).unwrap() {
            d.manufacturing = Some(setup.clone());
        }
        ui.collapsing("Repair candidate", |ui| {
            for (i, layer) in d.layers.layers.iter().enumerate() {
                ui.selectable_value(&mut self.layer, i, &layer.name);
            }
            for repair in mf::repair::Repair::ALL {
                if ui.button(repair.label()).clicked() {
                    let suggested = self
                        .session
                        .view
                        .as_ref()
                        .and_then(|v| v.report["release"]["suggested_parting_mm"].as_f64())
                        .unwrap_or(setup.parting_mm);
                    match mf::repair::candidate(d, &setup, Some(self.layer), repair, suggested) {
                        Ok(next) => *d = next,
                        Err(e) => self.session.error = Some(e.to_string()),
                    }
                }
            }
        });
        if self.session.view_key != Some(key(d)) && self.session.view.is_some() {
            ui.colored_label(
                Color32::from_rgb(240, 190, 100),
                "Previous findings — Preview the changed design",
            );
        }
        self.findings(ui);
    }

    fn findings(&mut self, ui: &mut egui::Ui) {
        let Some(view) = &self.session.view else {
            ui.weak("Preview to inspect the actual compensated pattern.");
            return;
        };
        ui.separator();
        if let Some(e) = &view.inspection_error {
            ui.colored_label(Color32::LIGHT_RED, e);
            return;
        }
        let r = &view.report["release"];
        ui.strong(format!(
            "Release: {}",
            r["status"].as_str().unwrap_or("unassessed")
        ));
        ui.label(format!(
            "Ring {:.2} g · charge {:.2} g",
            view.report["cast_ring_grams"].as_f64().unwrap_or(0.0),
            view.report["estimated_charge_grams"]
                .as_f64()
                .unwrap_or(0.0)
        ));
        ui.label(format!(
            "Flask fit: {} · unresolved rays: {}",
            r["fits_flask"], r["unresolved_rays"]
        ));
        if let Some(obs) = r["obstructions"].as_array() {
            ui.label(format!("{} obstruction regions", obs.len()));
            for (i, o) in obs.iter().enumerate() {
                if ui
                    .selectable_label(
                        self.selected_finding == Some(i),
                        format!(
                            "{} · {:.3} mm trapped",
                            o["half"].as_str().unwrap_or("Mold"),
                            o["depth_mm"].as_f64().unwrap_or(0.0)
                        ),
                    )
                    .clicked()
                {
                    self.selected_finding = Some(i);
                    self.stage = Stage::Pattern;
                    self.section_axis = 2;
                    self.section_offset = o["world"][2].as_f64().unwrap_or(0.0);
                    self.section = true;
                }
            }
        }
        for key in ["notes", "sand_findings"] {
            if let Some(notes) = r[key].as_array() {
                for note in notes {
                    ui.label(
                        note.as_str()
                            .unwrap_or_else(|| note["message"].as_str().unwrap_or("")),
                    );
                }
            }
        }
        if !view.report["sampled_local_wall"].is_null() {
            ui.collapsing("Sampled local wall", |ui| {
                ui.label(serde_json::to_string_pretty(&view.report["sampled_local_wall"]).unwrap());
            });
        }
        ui.weak("Sampled release can miss features between rays. Shop trials are still required. Export recomputes at the export mesh resolution.");
    }

    fn cad(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) {
        ui.text_edit_singleline(&mut d.name);
        if d.graph.is_some() {
            ui.label("This design is driven by its graph. Preview before making an editable snapshot; the original remains in Undo.");
            if ui
                .add_enabled(
                    self.session.view_key == Some(key(d)),
                    egui::Button::new("Make editable snapshot"),
                )
                .clicked()
            {
                if let Some(view) = &self.session.view {
                    *d = view.design.clone();
                    d.graph = None;
                    self.feature_text_id = None;
                }
            }
            return;
        }
        ui.collapsing("Start from an example", |ui| {
            for name in cad::examples::NAMES {
                if ui.button(*name).clicked() {
                    match cad::examples::design(name) {
                        Ok(next) => {
                            *d = next;
                            self.feature_text_id = None;
                        }
                        Err(e) => self.session.error = Some(e.to_string()),
                    }
                }
            }
        });
        ui.collapsing("Fit and band", |ui| {
            ui.label(format!(
                "Current bore: {:.4} mm",
                d.size.inner_diameter_mm()
            ));
            number(ui, "New bore mm", &mut self.bore);
            if ui.button("Create resized candidate").clicked() {
                match ringdesign_core::resize::candidate(d, self.bore, &Default::default()) {
                    Ok(next) => {
                        *d = next.design;
                        self.message = next.notes.join("; ");
                        self.feature_text_id = None;
                    }
                    Err(e) => self.session.error = Some(e.to_string()),
                }
            }
            number(ui, "Band width mm", &mut d.profile.width_mm);
            number(ui, "Band thickness mm", &mut d.profile.thickness_mm);
        });
        ui.horizontal_wrapped(|ui| {
            for (name, op) in [
                ("Add shank", Operation::Band),
                ("Add box", Operation::Box { size: [6., 4., 2.] }),
                (
                    "Add extrusion",
                    Operation::Extrude {
                        sketch: Sketch::rectangle(8., 6.),
                        height_mm: 2.,
                        draft_deg: 3.,
                    },
                ),
                (
                    "Add torus",
                    Operation::Torus {
                        major_mm: d.inner_radius_mm() + 1.2,
                        minor_mm: 1.2,
                    },
                ),
            ] {
                if ui.button(name).clicked() {
                    let doc = d.cad.get_or_insert_with(Default::default);
                    let id = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
                    let role = if matches!(op, Operation::Band | Operation::Torus { .. }) {
                        ComponentRole::Shank
                    } else {
                        ComponentRole::Other
                    };
                    let f = Feature {
                        id,
                        name: op.label().into(),
                        enabled: true,
                        operation: op,
                        component: cad::Component {
                            role,
                            ..Default::default()
                        },
                    };
                    match doc.append(f) {
                        Ok(()) => {
                            self.feature = doc.features.len() - 1;
                            self.feature_text_id = None;
                        }
                        Err(e) => self.session.error = Some(e.to_string()),
                    }
                }
            }
        });
        let Some(doc) = &mut d.cad else {
            return;
        };
        for (i, f) in doc.features.iter().enumerate() {
            if ui
                .selectable_value(&mut self.feature, i, format!("#{} {}", f.id, f.name))
                .clicked()
            {
                self.feature_text_id = None;
            }
        }
        let Some(f) = doc.features.get_mut(self.feature) else {
            return;
        };
        ui.separator();
        ui.text_edit_singleline(&mut f.name);
        ui.checkbox(&mut f.enabled, "Feature enabled");
        let mut output = doc.outputs.contains(&f.id);
        if ui.checkbox(&mut output, "Output component").changed() {
            if output {
                doc.outputs.push(f.id);
            } else {
                doc.outputs.retain(|id| *id != f.id);
            }
        }
        operation(ui, &mut f.operation);
        ui.collapsing("Component and assembly", |ui| {
            egui::ComboBox::from_id_salt("role")
                .selected_text(format!("{:?}", f.component.role))
                .show_ui(ui, |ui| {
                    for role in [
                        ComponentRole::Shank,
                        ComponentRole::Head,
                        ComponentRole::Setting,
                        ComponentRole::Inlay,
                        ComponentRole::Stone,
                        ComponentRole::Other,
                    ] {
                        ui.selectable_value(&mut f.component.role, role, format!("{role:?}"));
                    }
                });
            ui.checkbox(
                &mut f.component.reference,
                "Reference stone (excluded from metal)",
            );
            egui::ComboBox::from_id_salt("material")
                .selected_text(&f.component.material)
                .show_ui(ui, |ui| {
                    for m in ringdesign_core::metal::METALS {
                        ui.selectable_value(&mut f.component.material, m.name.into(), m.name);
                    }
                });
            let mut anchor = f.component.ring_anchor_deg.is_some();
            if ui.checkbox(&mut anchor, "Anchor to ring angle").changed() {
                f.component.ring_anchor_deg = anchor.then_some(90.0);
            }
            if let Some(angle) = &mut f.component.ring_anchor_deg {
                number(ui, "Anchor angle °", angle);
                number(ui, "Anchor height mm", &mut f.component.anchor_height_mm);
            }
            ui.text_edit_multiline(&mut f.component.bench_notes);
            if ui
                .button("Use current casting setup for this part")
                .clicked()
            {
                let mut setup = d.manufacturing.clone().unwrap_or_default();
                setup.component = Some(f.id);
                f.component.manufacturing = Some(setup);
            }
        });
        ui.collapsing("Feature source / advanced operations", |ui| {
            ui.weak("Full operation parameters, sketches, constraints, paths, and source identities. Load source creates a candidate; Preview validates it.");
            if self.feature_text_id != Some(f.id) { self.feature_text = serde_json::to_string_pretty(f).unwrap(); self.feature_text_id = Some(f.id); }
            ui.add(egui::TextEdit::multiline(&mut self.feature_text).code_editor().desired_width(f32::INFINITY).desired_rows(12));
            if ui.button("Refresh source from controls").clicked() { self.feature_text = serde_json::to_string_pretty(f).unwrap(); }
            if ui.button("Load feature source").clicked() { match serde_json::from_str::<Feature>(&self.feature_text) { Ok(next) if next.id == f.id => *f = next, Ok(_) => self.session.error = Some("Keep the feature identity; other operations may refer to it".into()), Err(e) => self.session.error = Some(e.to_string()) } }
        });
    }

    fn files(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut RingDesign,
        source: &RingDesign,
        lib: &AlphaLibrary,
    ) {
        ui.strong("Save and manufacture");
        ui.weak("Apply the candidate before exporting. Pattern packages contain the compensated mesh, source, report, and molding sheet. Assembly packages contain nominal STEP/3MF and component review files.");
        ui.add_enabled_ui(self.session.draft.is_none() && !self.session.busy, |ui| {
            for (label, action) in [
                ("Save editable project", Action::Project),
                (
                    "Export pattern package",
                    Action::Pattern { diagnostic: false },
                ),
                (
                    "Export diagnostic pattern",
                    Action::Pattern { diagnostic: true },
                ),
                ("Export CAD assembly", Action::Assembly),
            ] {
                if ui.button(label).clicked() {
                    self.send(ui, source, lib, action);
                }
            }
        });
        ui.separator();
        ui.strong("Import project JSON");
        ui.label("Paste a .ring.json project below, or drop a file onto the browser window.");
        ui.add(
            egui::TextEdit::multiline(&mut self.project_text)
                .desired_width(f32::INFINITY)
                .desired_rows(8),
        );
        if ui.button("Load project candidate").clicked() {
            match ringdesign_core::library::load_design_str(&self.project_text) {
                Ok(next) => {
                    *d = next;
                    self.feature_text_id = None;
                }
                Err(e) => self.session.error = Some(e.to_string()),
            }
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui, current: bool) {
        ui.horizontal_wrapped(|ui| {
            for stage in Stage::ALL {
                ui.selectable_value(&mut self.stage, stage, stage.label());
            }
        });
        let Some(view) = &self.session.view else {
            ui.allocate_space(vec2(ui.available_width(), 160.));
            ui.weak("Choose Preview to build the ring.");
            return;
        };
        let stage_current = self.stage == view.stage;
        ui.label(
            RichText::new(format!(
                "{} view{}",
                view.stage.label(),
                if current && stage_current {
                    ""
                } else {
                    " · previous preview; press Preview"
                }
            ))
            .color(if current && stage_current {
                ui.visuals().text_color()
            } else {
                Color32::from_rgb(240, 190, 100)
            }),
        );
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.section, "Section");
            if self.section {
                for (i, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    ui.selectable_value(&mut self.section_axis, i, label);
                }
                ui.add(
                    egui::DragValue::new(&mut self.section_offset)
                        .speed(0.05)
                        .suffix(" mm"),
                );
            } else {
                ui.weak("Drag to orbit");
            }
        });
        let height = ui.available_width().clamp(200., 440.) * 0.8;
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::drag());
        if response.dragged() && !self.section {
            let delta = ui.input(|i| i.pointer.delta());
            self.yaw += delta.x * 0.008;
            self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8., ui.visuals().extreme_bg_color);
        if self.section {
            let lines =
                cad::measure::section(&view.shape.mesh(), self.section_axis, self.section_offset);
            let extent = lines
                .iter()
                .flatten()
                .flatten()
                .fold(1.0_f64, |a, b| a.max(b.abs())) as f32;
            let scale = rect.width().min(rect.height()) * 0.45 / extent;
            for line in lines {
                painter.line_segment(
                    line.map(|p| rect.center() + vec2(p[0] as f32 * scale, -p[1] as f32 * scale)),
                    Stroke::new(1.8, Color32::from_rgb(70, 210, 195)),
                );
            }
        } else {
            draw(&painter, rect, &view.shape, self.yaw, self.pitch);
        }
        ui.weak("Finished shows the intended final surface. As cast predicts uniform shrink and retains modeled finishing stock.");
    }
}

fn operation(ui: &mut egui::Ui, op: &mut Operation) {
    ui.strong(op.label());
    match op {
        Operation::Box { size } => xyz(ui, "Size mm", size),
        Operation::Cylinder {
            radius_mm,
            height_mm,
        } => {
            number(ui, "Radius mm", radius_mm);
            number(ui, "Height mm", height_mm);
        }
        Operation::Sphere { radius_mm } => number(ui, "Radius mm", radius_mm),
        Operation::Torus { major_mm, minor_mm } => {
            number(ui, "Major radius mm", major_mm);
            number(ui, "Tube radius mm", minor_mm);
        }
        Operation::TwistedRing {
            major_mm,
            radial_mm,
            axial_mm,
            turns,
        } => {
            number(ui, "Major radius mm", major_mm);
            number(ui, "Radial mm", radial_mm);
            number(ui, "Axial mm", axial_mm);
            number(ui, "Turns", turns);
        }
        Operation::Extrude {
            height_mm,
            draft_deg,
            ..
        } => {
            number(ui, "Height mm", height_mm);
            number(ui, "Draft °", draft_deg);
        }
        Operation::Revolve {
            pivot,
            axis,
            degrees,
            ..
        } => {
            xyz(ui, "Pivot mm", pivot);
            xyz(ui, "Axis", axis);
            number(ui, "Revolution °", degrees);
        }
        Operation::Transform {
            translation,
            rotation_deg,
            ..
        } => {
            xyz(ui, "Translation mm", translation);
            xyz(ui, "Rotation °", rotation_deg);
        }
        Operation::Fillet { radius_mm, .. } => number(ui, "Radius mm", radius_mm),
        Operation::Chamfer { distance_mm, .. } => number(ui, "Distance mm", distance_mm),
        Operation::Shell { thickness_mm, .. } => number(ui, "Wall mm", thickness_mm),
        Operation::Twist {
            degrees, end_scale, ..
        } => {
            number(ui, "Twist °", degrees);
            number(ui, "End scale", end_scale);
        }
        _ => {}
    }
    if let Some(sketch) = op.sketch_mut() {
        ui.collapsing("Sketch points and workplane", |ui| {
            xyz(ui, "Origin mm", &mut sketch.plane.origin);
            for point in &mut sketch.points { ui.push_id(point.id, |ui| {
                ui.horizontal_wrapped(|ui| { ui.label(format!("#{}", point.id)); for v in &mut point.xy { ui.add(egui::DragValue::new(v).speed(0.05).suffix(" mm")); } ui.checkbox(&mut point.fixed, "Fixed"); });
            }); }
            ui.weak("Constraints are preserved; edit them in Feature source. Preview solves and checks the sketch before generating the solid.");
        });
    }
}

fn draw(p: &egui::Painter, rect: egui::Rect, shape: &job::Shape, yaw: f32, pitch: f32) {
    if shape.vertices.is_empty() {
        return;
    }
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let rotated: Vec<_> = shape
        .vertices
        .iter()
        .map(|v| {
            let x = cy * v[0] - sy * v[1];
            let y = sy * v[0] + cy * v[1];
            [x, cp * y - sp * v[2], sp * y + cp * v[2]]
        })
        .collect();
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for v in &rotated {
        for i in 0..3 {
            lo[i] = lo[i].min(v[i]);
            hi[i] = hi[i].max(v[i]);
        }
    }
    let scale = (rect.width() / (hi[0] - lo[0]).max(1.0))
        .min(rect.height() / (hi[1] - lo[1]).max(1.0))
        * 0.86;
    let project = |v: [f32; 3]| {
        pos2(
            rect.center().x + (v[0] - (lo[0] + hi[0]) * 0.5) * scale,
            rect.center().y - (v[1] - (lo[1] + hi[1]) * 0.5) * scale,
        )
    };
    let mut faces: Vec<_> = shape.faces.iter().collect();
    faces.sort_by(|a, b| {
        a.iter()
            .map(|i| rotated[*i as usize][2])
            .sum::<f32>()
            .total_cmp(&b.iter().map(|i| rotated[*i as usize][2]).sum::<f32>())
    });
    let mut mesh = egui::Mesh::default();
    for f in faces {
        let v = f.map(|i| rotated[i as usize]);
        let a = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
        let b = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let len = n.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
        let light =
            (0.45 + 0.5 * ((n[0] * 0.3 + n[1] * 0.4 + n[2] * 0.86) / len).abs()).clamp(0., 1.);
        let color = Color32::from_rgb(
            (224. * light) as u8,
            (212. * light) as u8,
            (189. * light) as u8,
        );
        let base = mesh.vertices.len() as u32;
        for v in v {
            mesh.colored_vertex(project(v), color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    p.add(egui::Shape::mesh(mesh));
}
