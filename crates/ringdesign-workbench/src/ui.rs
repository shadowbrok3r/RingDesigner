use super::*;
use egui::{Color32, RichText, Stroke, pos2, vec2};
use ringdesign_core::{
    cad::{self, ComponentRole, Feature, Operation, Placement},
    castability::{CastProcess, SandProcess},
    manufacturing::{BoreStrategy, Recipe, Setup},
};

fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    crate::controls::row(ui,label,|ui| {
        ui.add(egui::DragValue::new(value).speed(0.02).max_decimals(4));
    });
}
fn xyz(ui: &mut egui::Ui, label: &str, v: &mut [f64; 3]) {
    ui.strong(label);
    for (axis,x) in ["X","Y","Z"].into_iter().zip(v) {
        crate::controls::row(ui,axis,|ui| {ui.add(egui::DragValue::new(x).speed(0.02).max_decimals(3));});
    }
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
        ui.horizontal_wrapped(|ui| {
            ui.heading("Casting workshop");
            ui.weak(if self.session.draft.is_some() {
                "Unapplied changes"
            } else {
                "Current design"
            });
            if self.session.busy {
                ui.spinner();
                ui.label("Checking the pattern…");
            }
        });
        ui.weak("Choose how you cast, check the pattern, then export the workshop files.");
        ui.horizontal_wrapped(|ui| {
            for (i, label) in ["1 Setup", "2 Check & fix", "3 Export"].into_iter().enumerate() {
                ui.selectable_value(&mut self.tab, i, label);
            }
            ui.menu_button("More", |ui| {
                if ui.button("Advanced CAD tools").clicked() { self.tab = 3; self.stage = Stage::Nominal; ui.close(); }
            });
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.session.busy, egui::Button::new("Check pattern"))
                .clicked()
            {
                self.send(ui, source, lib, Action::Inspect);
            }
            if ui
                .add_enabled(
                    self.session.draft.is_some()
                        && self.session.is_current(source)
                        && !self.session.busy,
                    egui::Button::new("Apply changes"),
                )
                .clicked()
            {
                events.changed |= self.session.apply(source);
            }
            if ui
                .add_enabled(
                    self.session.draft.is_some() || self.session.busy,
                    egui::Button::new("Discard changes"),
                )
                .clicked()
            {
                self.session.cancel();
                self.worker = None;
                self.feature_text_id = None;
            }
            ui.menu_button("History", |ui| {
                if ui.add_enabled(!self.session.undo.is_empty(), egui::Button::new("Undo")).clicked() {
                    events.changed |= self.session.undo(source);
                }
                if ui.add_enabled(!self.session.redo.is_empty(), egui::Button::new("Redo")).clicked() {
                    events.changed |= self.session.redo(source);
                }
            });
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
        let mut inspect = false;
        egui::ScrollArea::vertical()
            .id_salt(("workshop-step", self.tab))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if ui.available_width() >= 720.0 {
                    ui.columns(2, |cols| {
                        self.preview(&mut cols[0], current);
                        inspect |= self.controls(&mut cols[1], &mut draft, source, lib);
                    });
                } else {
                    inspect |= self.controls(ui, &mut draft, source, lib);
                    ui.separator();
                    self.preview(ui, current);
                }
            });
        if key(&draft) != before {
            self.session.draft = Some(draft);
        }
        if inspect {
            self.send(ui, source, lib, Action::Inspect);
        }
        events
    }

    fn controls(
        &mut self,
        ui: &mut egui::Ui,
        d: &mut RingDesign,
        source: &RingDesign,
        lib: &AlphaLibrary,
    ) -> bool {
        let mut inspect = false;
        match self.tab {
            0 => {
                ui.strong("Choose your material and process");
                ui.label("Start with a recipe and your alloy. The defaults are ready for a first check.");
                self.casting(ui, d);
                if ui.add_enabled(!self.session.busy, egui::Button::new("Continue to check & fix")).clicked() {
                    self.tab = 1;
                    self.stage = Stage::Pattern;
                    inspect = true;
                }
            }
            1 => {
                ui.strong("Can the pattern leave the mold?");
                ui.label("Select a problem to locate it. Try a correction, check it, then apply the result when ready.");
                if self.session.view_key != Some(key(d)) && self.session.view.is_some() {
                    ui.colored_label(Color32::from_rgb(240, 190, 100), "Previous findings — use Check pattern to update them.");
                }
                self.findings(ui);
                self.repairs(ui, d);
                if ui.button("Continue to export").clicked() { self.tab = 2; }
            }
            2 => {
                self.channels(ui, d);
                self.files(ui, d, source, lib);
            }
            _ => self.cad(ui, d),
        }
        ui.collapsing("What do these terms mean?", |ui| {
            for (term, meaning) in [
                ("Pattern", "The master used to make the mold cavity. It includes shrink allowance and finishing stock."),
                ("Release / obstruction", "Whether the pattern lifts out without catching or breaking the sand."),
                ("Parting plane", "Where the two mold halves meet."),
                ("Draft", "A slight slope that helps a wall slide out of the mold."),
                ("Shrink allowance", "Extra size to compensate for metal contracting as it cools."),
                ("Finishing stock", "Extra metal left for polishing, filing, or reaming."),
                ("Gate / vent", "A passage for metal to enter, or air to escape."),
            ] { ui.strong(term); ui.label(meaning); }
        });
        inspect
    }

    fn casting(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) {
        let mut setup = d
            .manufacturing
            .clone()
            .unwrap_or_else(|| Setup::from_design(d));
        let old = serde_json::to_vec(&setup).unwrap();
        let old_process = setup.recipe.process;
        ui.strong("Casting process");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut setup.recipe.process,
                CastProcess::SandTwoPart,
                "Two-part sand",
            );
            ui.selectable_value(&mut setup.recipe.process, CastProcess::LostWax, "Lost wax");
        });
        if setup.recipe.process != old_process {
            let mut limits = d.draft.clone();
            setup.recipe.process.apply(&mut limits);
            if setup.recipe.process == CastProcess::SandTwoPart {
                setup.recipe.sand.unwrap_or(SandProcess::DelftClay).apply(&mut limits);
            }
            setup.recipe.min_section_mm = limits.min_section_mm;
            setup.recipe.min_detail_mm = limits.min_detail_mm;
            setup.recipe.min_draft_deg = limits.min_draft_deg;
        }
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
        number(ui, "Shrink allowance %", &mut setup.recipe.shrink_pct);
        if let Some(doc) = &d.cad {
            let band = doc.band().is_some();
            let apart: Vec<u64> = doc
                .attachments()
                .into_iter()
                .filter(|(_, a, _)| *a == ringdesign_core::cad::Attach::Separate)
                .map(|(id, ..)| id)
                .collect();
            let whole = if band { "The ring: band with its joined and cut parts" } else { "Automatic (one metal part)" };
            egui::ComboBox::from_id_salt("component")
                .selected_text(setup.component.map_or(whole.into(), |id| format!("Component #{id}")))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut setup.component, None, whole);
                    for f in doc
                        .features
                        .iter()
                        .filter(|f| doc.outputs.contains(&f.id) && !f.component.reference && (!band || apart.contains(&f.id)))
                    {
                        ui.selectable_value(&mut setup.component, Some(f.id), &f.name);
                    }
                });
        }
        ui.collapsing("Advanced: mold opening direction", |ui| {
            ui.weak("Choose the direction the pattern lifts out and where the mold halves meet.");
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
        ui.collapsing("Advanced: finishing stock, mold size & limits", |ui| {
            ui.weak("Finishing stock adds metal for later polishing or reaming. The flask is the box holding the sand.");
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
        if old != serde_json::to_vec(&setup).unwrap() {
            d.manufacturing = Some(setup);
        }
    }

    fn channels(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) {
        let mut setup = d.manufacturing.clone().unwrap_or_else(|| Setup::from_design(d));
        let old = serde_json::to_vec(&setup).unwrap();
        ui.collapsing("Optional: metal and air channels", |ui| {
            ui.weak("A gate feeds the cavity; a vent lets air escape. These channels are cut into the sand.");
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
            d.manufacturing = Some(setup);
        }
    }

    fn repairs(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) {
        let setup = d.manufacturing.clone().unwrap_or_else(|| Setup::from_design(d));
        ui.collapsing("Try a correction", |ui| {
            ui.weak("Changes stay in this preview until you choose Apply changes. Discard changes returns to your design.");
            if d.graph.is_some() {
                ui.weak("This design is controlled by its graph. Edit the graph or bake it in the Graph workspace before changing its geometry.");
            }
            ui.strong("Layer to adjust");
            for (i, layer) in d.layers.layers.iter().enumerate() {
                ui.selectable_value(&mut self.layer, i, &layer.name);
            }
            for repair in mf::repair::Repair::ALL {
                let needs_layer = matches!(repair, mf::repair::Repair::ReduceRelief | mf::repair::Repair::MoveToSide | mf::repair::Repair::DeferToBench);
                let enabled = !self.session.busy
                    && (d.graph.is_none() || repair == mf::repair::Repair::SuggestedParting)
                    && (!needs_layer || self.layer < d.layers.layers.len())
                    && (repair != mf::repair::Repair::SuggestedParting || self.session.view_key == Some(key(d)));
                if ui.add_enabled(enabled, egui::Button::new(repair.label())).clicked() {
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
    }

    fn findings(&mut self, ui: &mut egui::Ui) {
        let Some(view) = &self.session.view else {
            ui.weak("Choose Check pattern to inspect the pattern at its casting size.");
            return;
        };
        ui.separator();
        if let Some(e) = &view.inspection_error {
            ui.colored_label(Color32::LIGHT_RED, e);
            return;
        }
        let r = &view.report["release"];
        ui.strong(match r["status"].as_str().unwrap_or("") {
            "Clear" => "No sampled obstruction",
            "Blocked" => "Pattern is obstructed",
            "Review" => "Review needed",
            "NotApplicable" => "Mold pull not required",
            _ => "Pattern needs checking",
        });
        ui.label(match r["status"].as_str().unwrap_or("") {
            "Clear" => "The sampled pattern can withdraw. Review wall thickness and fine detail before exporting.",
            "Blocked" => "Some surfaces catch in the sand. Select a problem below and try a correction.",
            "Review" => "Some areas need a closer look. Review the findings before exporting.",
            "NotApplicable" => "Two-part mold release does not apply to this process. Check thickness and detail before export.",
            _ => "The pattern could not be checked. Review the error and setup, then check again.",
        });
        ui.label(format!(
            "Ring {:.2} g · charge {:.2} g",
            view.report["cast_ring_grams"].as_f64().unwrap_or(0.0),
            view.report["estimated_charge_grams"]
                .as_f64()
                .unwrap_or(0.0)
        ));
        ui.label(if r["fits_flask"].as_bool() == Some(true) { "Pattern fits inside the mold box." } else { "Pattern does not fit the mold box. Increase its size in Setup." });
        if let Some(obs) = r["obstructions"].as_array() {
            ui.label(if obs.is_empty() { "No surfaces caught in the sampled release check.".into() } else { format!("{} areas may catch in the mold", obs.len()) });
            for (i, o) in obs.iter().enumerate() {
                if ui
                    .selectable_label(
                        self.selected_finding == Some(i),
                        format!(
                            "{} · {:.3} mm trapped",
                            match o["half"].as_str().unwrap_or("") {
                                "Cope" => "Upper mold",
                                "Drag" => "Lower mold",
                                _ => "Mold",
                            },
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
        if let Some(area) = r["low_draft_area_mm2"].as_f64().filter(|area| *area > 0.0) {
            ui.label(format!("Some walls have less slope than the recipe recommends ({area:.2} mm²). Review the pattern finish and opening direction."));
        }
        if let Some(notes) = r["sand_findings"].as_array() {
            for note in notes {
                ui.label(note.as_str().unwrap_or_else(|| note["message"].as_str().unwrap_or("")));
            }
        }
        if let Some(wall) = view.report["radial_wall_mm"].as_f64() {
            ui.label(format!("Radial wall: {wall:.2} mm · target {:.2} mm", view.report["radial_wall_limit_mm"].as_f64().unwrap_or(0.0)));
        }
        if let Some(notes) = view.report["detail_findings"].as_array() {
            for note in notes.iter().filter_map(|n| n.as_str()) { ui.label(note); }
        }
        ui.collapsing("Technical inspection details", |ui| {
            if let Some(notes) = r["notes"].as_array() {
                for note in notes.iter().filter_map(|n| n.as_str()) { ui.label(note); }
            }
            ui.label(format!("Unresolved samples: {}", r["unresolved_rays"]));
            if !view.report["sampled_local_wall"].is_null() {
                ui.label(serde_json::to_string_pretty(&view.report["sampled_local_wall"]).unwrap());
            }
        });
        ui.weak("Export runs a fresh check at the export resolution.");
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
        ui.weak("Create a solid, edit its fields, then Preview and Apply.");
        ui.menu_button((icons::Icon::Files.image(ui,20.), "Example projects"), |ui| {
            for name in cad::examples::NAMES {
                if cad_tools::example_button(ui,name).clicked() {
                    match cad::examples::design(name) {
                        Ok(next) => { *d = next; self.feature = 0; self.feature_text_id = None; }
                        Err(e) => self.session.error = Some(e.to_string()),
                    }
                    ui.close();
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
            for (modify,title,icon) in [(false,"Create",icons::Icon::Add),(true,"Modify",icons::Icon::CadFillet)] {
                ui.menu_button((icon.image(ui,20.),title), |ui| {
                    let ids: Vec<u64> = d.cad.as_ref().map(|doc| doc.features.iter().filter(|f| f.enabled).map(|f|f.id).collect()).unwrap_or_default();
                    let first = d.cad.as_ref().and_then(|doc|doc.features.get(self.feature)).map(|f|f.id).or(ids.last().copied()).unwrap_or(0);
                    let second = ids.iter().rev().copied().find(|id|*id != first).unwrap_or(0);
                    for op in cad_tools::starters(first,second).into_iter().filter(|op|cad_tools::modify(op)==modify) {
                        let valid = !modify || (ids.contains(&first) && (!matches!(op,Operation::Boolean{..}) || second != 0));
                        let response = ui.add_enabled(valid,egui::Button::new((cad_tools::icon(&op).image(ui,20.),op.label())))
                            .on_hover_text(cad_tools::hint(&op)).on_disabled_hover_text("Create the source solids first; booleans need two different solids.");
                        if response.clicked() {
                            let doc = d.cad.get_or_insert_with(Default::default);
                            let id = doc.features.iter().map(|f|f.id).max().unwrap_or(0)+1;
                            let mut component=op.sources().first().and_then(|id|doc.features.iter().find(|f|f.id==*id)).map(|f|f.component.clone()).unwrap_or_default();
                            if matches!(op,Operation::Band | Operation::Torus{..} | Operation::TwistedRing{..}) {component.role=ComponentRole::Shank;}
                            component.placement=Placement::Free;
                            let f = Feature { id,name:op.label().into(),enabled:true,operation:op,component };
                            match doc.append(f) {
                                Ok(()) => {self.feature = doc.features.len()-1;self.feature_text_id=None;}
                                Err(e) => self.session.error=Some(e.to_string()),
                            }
                            ui.close();
                        }
                    }
                });
            }
        });
        let Some(doc) = &mut d.cad else {
            return;
        };
        for (i, f) in doc.features.iter().enumerate() {
            if ui
                .selectable_value(&mut self.feature, i, (cad_tools::icon(&f.operation).image(ui,20.), format!("#{} {}", f.id, f.name)))
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
            cad_tools::seat(ui, &mut f.component.placement, &mut f.operation);
            cad_tools::attachment(ui, &mut f.component);
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
        ui.strong("Take the pattern to the workshop");
        ui.label("The pattern package includes the mesh at casting size, your source design, the check report, and a printable molding sheet.");
        if self.session.draft.is_some() || key(d) != key(source) {
            ui.colored_label(Color32::from_rgb(240, 190, 100), "Check pattern, then Apply changes before exporting.");
        }
        ui.add_enabled_ui(self.session.draft.is_none() && key(d) == key(source) && !self.session.busy, |ui| {
            for (label, action) in [
                (
                    "Export pattern package",
                    Action::Pattern { diagnostic: false },
                ),
                ("Save editable project", Action::Project),
            ] {
                if ui.button(label).clicked() {
                    self.send(ui, source, lib, action);
                }
            }
            ui.collapsing("Advanced exports", |ui| {
                ui.weak("Diagnostic export includes blocked results for investigation. CAD assembly contains nominal STEP/3MF and component review files.");
                for (label, action) in [
                    ("Export diagnostic pattern", Action::Pattern { diagnostic: true }),
                    ("Export CAD assembly", Action::Assembly),
                ] {
                    if ui.button(label).clicked() { self.send(ui, source, lib, action); }
                }
            });
        });
        ui.collapsing("Advanced: import project JSON", |ui| {
            ui.label("Paste a .ring.json project below to preview it before applying.");
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
        });
    }

    fn preview(&mut self, ui: &mut egui::Ui, current: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button((icons::Icon::Layers.image(ui,20.),self.stage.label()),|ui| {
                for stage in Stage::ALL { if ui.selectable_value(&mut self.stage,stage,stage.label()).clicked(){ui.close();} }
            });
            if icons::compact(ui,icons::Icon::Fit,false).clicked() {self.zoom=1.;self.pan=egui::Vec2::ZERO;}
        });
        let Some(view) = &self.session.view else {
            ui.allocate_space(vec2(ui.available_width(), 160.));
            ui.weak("Choose Check pattern to build this preview.");
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
                    " · previous preview; choose Check pattern"
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
        // The cube owns a foreground Area, so it cannot inherit this scroll
        // area's clip. Hide it when scrolling would cover the fixed app chrome.
        let navigator_bounds = egui::Rect::from_min_size(
            rect.right_top() + vec2(-110., 7.),
            vec2(110., 112. + 2. * ui.spacing().interact_size.y.max(28.)),
        );
        let nav = if !self.section && ui.clip_rect().contains_rect(navigator_bounds) {
            Some(navigation::show_camera(ui, rect, ui.id().with("workshop-cube"),
                &mut self.navigation, [self.yaw,self.pitch,self.roll], 90.))
        } else { None };
        if let Some(action) = nav.as_ref().and_then(|nav|nav.action) {
            [self.yaw,self.pitch,self.roll] = action.apply([self.yaw,self.pitch,self.roll],90.);
            if action.recentres() {self.pan=egui::Vec2::ZERO;}
        }
        let blocked = ui.input(|i|i.pointer.press_origin().or(i.pointer.interact_pos()))
            .is_some_and(|p|nav.as_ref().is_some_and(|nav|nav.rect.contains(p)) || ui.ctx().layer_id_at(p).is_some_and(|layer|layer!=ui.layer_id()));
        if !blocked && !self.section {
            if response.dragged() {
                let delta=ui.input(|i|i.pointer.delta());
                if self.navigation.locked || ui.input(|i|i.modifiers.shift || i.pointer.middle_down()) { self.pan+=delta; }
                else {self.yaw-=delta.x*0.008;self.pitch+=delta.y*0.008;}
            }
            if response.hovered() {self.zoom=(self.zoom*(ui.input(|i|i.smooth_scroll_delta.y)*0.002).exp()).clamp(0.2,12.);}
            if let Some(touch)=ui.input(|i|i.multi_touch()) {self.zoom=(self.zoom*touch.zoom_delta).clamp(0.2,12.);self.pan+=touch.translation_delta;}
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
            draw(&painter, rect, &view.shape, [self.yaw,self.pitch,self.roll],self.zoom,self.pan);
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
            in_plane,
            ..
        } => {
            let (at, along) = if *in_plane { ("Axis origin in the sketch's plane mm", "Axis direction in the sketch's plane") } else { ("Pivot mm", "Axis") };
            xyz(ui, at, pivot);
            xyz(ui, along, axis);
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
            degrees, end_scale, scale, closed, path, ..
        } => {
            number(ui, "Twist °", degrees);
            if scale.is_empty() {
                number(ui, "End scale", end_scale);
            } else {
                ui.weak(format!("Scale law of {} knots; edit it in Feature source.", scale.len()));
            }
            ui.checkbox(closed, "Closed round the path");
            if let cad::TwistPath::Points { points, smooth } = path {
                ui.checkbox(smooth, "Smooth through the points");
                for (i,p) in points.iter_mut().enumerate() { xyz(ui,&format!("Point {i} mm"),p); }
            }
        }
        Operation::Sweep { path, closed, twist_deg, end_scale, .. } => {
            match path {
                cad::SweepPath::Points(points) => {
                    for (i,p) in points.iter_mut().enumerate() { xyz(ui,&format!("Station {i} mm"),p); }
                }
                cad::SweepPath::Sketch { feature, entity, lift_mm } => {
                    ui.weak(format!("Along entity #{entity} of sketch #{feature}; it follows every edit to it."));
                    number(ui, "Lift mm", lift_mm);
                }
            }
            ui.checkbox(closed, "Closed round the path");
            number(ui, "Twist °", twist_deg);
            number(ui, "End scale", end_scale);
        }
        Operation::Loft { sections } => {
            for (i,p) in sections.iter_mut().enumerate() { if let Some(s) = p.sketch_mut() { xyz(ui,&format!("Section {i} origin"),&mut s.plane.origin); } else { ui.weak(format!("Section {i}: sketch feature #{}", p.feature().unwrap_or(0))); } }
        }
        Operation::Sketch { .. } => { ui.weak("A closed profile for other features to extrude, revolve, sweep or loft; edit it below."); }
        Operation::Boolean {a,b,..} => { ui.label(format!("Solids #{a} and #{b}. Change references in Feature source.")); }
        Operation::Band => { ui.weak("Uses the ring's fit, profile and ornament."); }
        Operation::Builder { .. } => { ui.weak("Built round its stone; edit its settings in Feature source."); }
        Operation::Pattern { kind, .. } => match kind {
            cad::PatternKind::Ring { count, span_deg } | cad::PatternKind::About { count, span_deg, .. } => {
                crate::controls::row(ui, "Instances", |ui| {
                    ui.add(egui::DragValue::new(count).range(2..=cad::pattern::MAX_PATTERN_COUNT));
                });
                number(ui, "Span °", span_deg);
            }
            cad::PatternKind::Mirror { .. } => { ui.weak("One reflected copy; change what it reflects across in Feature source."); }
        },
        Operation::Plane { offset_mm, .. } => number(ui, "Offset mm", offset_mm),
        Operation::PressPull { distance_mm, .. } => number(ui, "Distance mm", distance_mm),
        Operation::Stored { recipe, mesh, .. } => { ui.weak(format!("{} triangles {} made; run it again where {} is to change it.", mesh.triangles, recipe.kernel_name(), recipe.kernel_name())); }
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

fn draw(p: &egui::Painter, rect: egui::Rect, shape: &job::Shape, angles: [f32;3], zoom: f32, pan: egui::Vec2) {
    let [yaw,pitch,roll]=angles;
    if shape.vertices.is_empty() {
        return;
    }
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let rotated: Vec<_> = shape
        .vertices
        .iter()
        .map(|v| {
            let x = -sy*v[0]+cy*v[1];
            let y = -sp*cy*v[0]-sp*sy*v[1]+cp*v[2];
            let z = cp*cy*v[0]+cp*sy*v[1]+sp*v[2];
            let (sr,cr)=roll.sin_cos();
            [cr*x+sr*y,-sr*x+cr*y,z]
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
        * 0.86 * zoom;
    let project = |v: [f32; 3]| {
        pos2(
            rect.center().x + pan.x + (v[0] - (lo[0] + hi[0]) * 0.5) * scale,
            rect.center().y + pan.y - (v[1] - (lo[1] + hi[1]) * 0.5) * scale,
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
