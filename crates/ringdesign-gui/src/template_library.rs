//! Publish snapshots of editable graphs, then start independent designs from them.
use crate::app::RingDesignerApp;
use ringdesign_graph::{
    graph::{Exposed, Graph},
    personal::{self, Template},
    registry::{Registry, Widget},
    value::ValueKind,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
};

struct Entry {
    path: PathBuf,
    name: String,
    description: String,
    png: Vec<u8>,
}
struct Draft {
    name: String,
    description: String,
    graph: Graph,
}
#[derive(Default)]
pub struct State {
    open: bool,
    entries: Vec<Entry>,
    scanning: Option<Receiver<(Vec<Entry>, Vec<String>)>>,
    saving: Option<Receiver<Result<PathBuf, String>>>,
    draft: Option<Draft>,
    publishing: bool,
    message: String,
}
impl State {
    pub fn open(&mut self, ctx: &egui::Context) {
        self.open = true;
        self.refresh(ctx);
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        if self.scanning.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        match std::thread::Builder::new()
            .name("template-library".into())
            .spawn(move || {
                let mut entries = vec![];
                let mut errors = vec![];
                for path in personal::paths(&personal::directory()) {
                    match personal::load(&path) {
                        Ok(t) => entries.push(Entry {
                            path,
                            name: t.name,
                            description: t.description,
                            png: t.thumbnail_png,
                        }),
                        Err(e) => errors.push(format!("{}: {e}", path.display())),
                    }
                }
                let _ = tx.send((entries, errors));
                wake.request_repaint();
            }) {
            Ok(_) => self.scanning = Some(rx),
            Err(e) => self.message = e.to_string(),
        }
    }
}

pub fn publish(app: &mut RingDesignerApp) {
    if app.template_library.saving.is_some() {
        app.template_library.open = true;
        return;
    }
    if let Some(ed) = &app.graph_ed {
        app.template_library.draft = Some(Draft {
            name: app.design.name.clone(),
            description: String::new(),
            graph: ed.graph().clone(),
        });
        app.template_library.message.clear();
    }
}

fn unique_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    personal::directory().join(format!("template-{stamp}.{}", personal::EXT))
}

pub fn show(app: &mut RingDesignerApp, ctx: &egui::Context) {
    let mut state = std::mem::take(&mut app.template_library);
    if let Some(result) = state.scanning.as_ref().and_then(|r| match r.try_recv() {
        Ok(result) => Some(result),
        Err(mpsc::TryRecvError::Empty) => None,
        Err(mpsc::TryRecvError::Disconnected) => Some((
            vec![],
            vec!["Library worker stopped. Refresh to retry.".into()],
        )),
    }) {
        state.scanning = None;
        state.entries = result.0;
        if !result.1.is_empty() {
            state.message = result.1.join("\n");
        }
    }
    if let Some(result) = state.saving.as_ref().and_then(|r| match r.try_recv() {
        Ok(result) => Some(result),
        Err(mpsc::TryRecvError::Empty) => None,
        Err(mpsc::TryRecvError::Disconnected) => {
            Some(Err("Template worker stopped. Try saving again.".into()))
        }
    }) {
        state.saving = None;
        match result {
            Ok(path) => {
                state.message = if path.parent() == Some(personal::directory().as_path()) {
                    "Saved to My templates. Use it to start a new, independent design.".into()
                } else {
                    format!("Exported {}", path.display())
                };
                if state.publishing {
                    state.draft = None;
                }
                state.publishing = false;
                state.open(ctx);
            }
            Err(e) => {
                state.publishing = false;
                state.message = format!("Could not save template: {e}");
            }
        }
    }
    if state.saving.is_some() || state.scanning.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    let mut load = None;
    let mut refresh = false;
    let mut import = None;
    let mut export = None;
    egui::Window::new("My templates")
        .open(&mut state.open)
        .default_pos(ctx.content_rect().center() - egui::vec2(290., 240.))
        .default_width(580.)
        .show(ctx, |ui| {
            ui.label("Reusable ring builders with your controls, defaults and artwork.");
            ui.horizontal(|ui| {
                refresh |= ui.button("Refresh library").clicked();
                if ui
                    .add_enabled(
                        state.saving.is_none(),
                        egui::Button::new("Import template…"),
                    )
                    .clicked()
                {
                    import = rfd::FileDialog::new()
                        .add_filter("Ring template", &["json"])
                        .pick_file();
                }
                if state.scanning.is_some() || state.saving.is_some() {
                    ui.spinner();
                }
            });
            if state.entries.is_empty() && state.scanning.is_none() {
                ui.weak("In Graph, choose Save as template to add your first ring builder.");
            }
            egui::ScrollArea::vertical()
                .max_height(460.)
                .show(ui, |ui| {
                    for entry in &state.entries {
                        ui.push_id(&entry.path, |ui| {
                            ui.horizontal(|ui| {
                                thumbnail(ui, entry);
                                ui.vertical(|ui| {
                                    ui.strong(&entry.name);
                                    ui.label(&entry.description);
                                    ui.horizontal(|ui| {
                                        if ui.button("Use template").clicked() {
                                            load = Some(entry.path.clone());
                                        }
                                        if ui
                                            .add_enabled(
                                                state.saving.is_none(),
                                                egui::Button::new("Export copy…"),
                                            )
                                            .clicked()
                                        {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .set_file_name(format!("ring.{}", personal::EXT))
                                                .save_file()
                                            {
                                                export = Some((entry.path.clone(), path));
                                            }
                                        }
                                    });
                                });
                            });
                            ui.separator();
                        });
                    }
                });
            if !state.message.is_empty() {
                ui.label(&state.message);
            }
        });
    if let Some((source, destination)) = import.map(|path| (path, unique_path())).or(export) {
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        match std::thread::Builder::new()
            .name("template-transfer".into())
            .spawn(move || {
                let result = personal::load(&source)
                    .and_then(|t| t.save_new(&destination))
                    .map(|()| destination)
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
                wake.request_repaint();
            }) {
            Ok(_) => {
                state.saving = Some(rx);
                state.message = "Copying template…".into();
            }
            Err(e) => state.message = e.to_string(),
        }
    }
    if refresh {
        state.refresh(ctx);
    }
    if let Some(path) = load {
        state.open = false;
        app.open_file(path);
    }

    let mut save = false;
    let mut keep = true;
    if let Some(draft) = &mut state.draft {
        egui::Window::new("Save as template")
            .open(&mut keep)
            .default_pos(ctx.content_rect().center() - egui::vec2(310., 330.))
            .default_width(620.)
            .show(ctx, |ui| {
                ui.label("Choose the controls someone will adjust when reusing this ring.");
                ui.weak(
                    "Defaults below belong to this template; your current design stays as it is.",
                );
                ui.add_enabled_ui(state.saving.is_none(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut draft.name);
                    });
                    ui.add(
                        egui::TextEdit::multiline(&mut draft.description)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .hint_text("Describe when to use this builder"),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("template-controls")
                        .max_height(400.)
                        .show(ui, |ui| controls(ui, &mut draft.graph, &app.graph_reg));
                    ui.separator();
                    ui.label(format!(
                        "{} named controls • editable graph and artwork included",
                        draft.graph.exposed.len()
                    ));
                    save = ui
                        .add_enabled(
                            !draft.name.trim().is_empty(),
                            egui::Button::new("Save to library"),
                        )
                        .clicked();
                });
                if state.saving.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Checking defaults and creating a preview…");
                    });
                }
                if !state.message.is_empty() {
                    ui.label(&state.message);
                }
            });
    }
    if !keep {
        state.draft = None;
    }
    if save {
        let draft = state.draft.as_ref().expect("save has draft");
        let (graph, name, description) = (
            draft.graph.clone(),
            draft.name.clone(),
            draft.description.clone(),
        );
        let (lib, reg, source, wake) = (
            app.lib.clone(),
            app.graph_reg.clone(),
            app.design.clone(),
            ctx.clone(),
        );
        let (tx, rx) = mpsc::channel();
        match std::thread::Builder::new()
            .name("template-publish".into())
            .spawn(move || {
                let result = (|| -> anyhow::Result<PathBuf> {
                    // Validate ranges before evaluation, so invalid defaults never enter a builder.
                    Template::new(&name, &description, &source, &graph, &lib)?;
                    let mut ev =
                        ringdesign_graph::eval::Evaluator::with_exprs(ringdesign_script::engine());
                    let opened = ringdesign_graph::templates::open_document(
                        graph.clone(),
                        &reg,
                        &lib,
                        &mut ev,
                        Arc::new(|_| {}),
                        &Arc::default(),
                    )?;
                    let lib = opened.library.as_deref().unwrap_or(&lib);
                    let mut design = opened.design;
                    design.manufacturing = source.manufacturing;
                    design.draft = source.draft;
                    let mesh = ringdesign_core::mesh::try_build(
                        &design,
                        lib,
                        ringdesign_core::mesh::BuildParams {
                            theta_steps: 192,
                            profile_steps: 96,
                            ..Default::default()
                        },
                    )?;
                    let mut template = Template::new(&name, &description, &design, &graph, lib)?;
                    template.thumbnail_png = ringdesign_core::render::png_bytes(
                        &mesh.mesh,
                        0.55,
                        1.12,
                        192,
                        [0.85, 0.68, 0.4],
                    )?;
                    let path = unique_path();
                    template.save_new(&path)?;
                    Ok(path)
                })()
                .map_err(|e| e.to_string());
                let _ = tx.send(result);
                wake.request_repaint();
            }) {
            Ok(_) => {
                state.saving = Some(rx);
                state.publishing = true;
                state.message.clear();
            }
            Err(e) => state.message = e.to_string(),
        }
    }
    app.template_library = state;
}

fn controls(ui: &mut egui::Ui, graph: &mut Graph, reg: &Registry) {
    // Resolve pin lists before mutating values; dynamic cluster pins come from their saved graph.
    let candidates: Vec<_> = graph
        .nodes
        .iter()
        .filter_map(|n| {
            let (pins, _) = reg.node_pins(n)?;
            let pins: Vec<_> = pins
                .into_iter()
                .filter(|p| {
                    matches!(
                        p.kind,
                        ValueKind::Number
                            | ValueKind::Int
                            | ValueKind::Bool
                            | ValueKind::Text
                            | ValueKind::AlphaRef
                    ) && graph.wire_into(n.id, &p.name).is_none()
                })
                .collect();
            let label = n.label.clone().unwrap_or_else(|| {
                reg.get(&n.kind)
                    .map_or_else(|| n.kind.clone(), |s| s.label.clone())
            });
            (!pins.is_empty()).then_some((n.id, label, pins))
        })
        .collect();
    for (id, label, pins) in candidates {
        egui::CollapsingHeader::new(label)
            .id_salt(("template-node", id))
            .show(ui, |ui| {
                for mut pin in pins {
                    ui.push_id(pin.name.clone(), |ui| {
                        let index = graph
                            .exposed
                            .iter()
                            .position(|e| e.node == id && e.input == pin.name);
                        let mut exposed = index.is_some();
                        if ui.checkbox(&mut exposed, &pin.name).changed() {
                            if exposed {
                                graph.exposed.push(Exposed {
                                    name: pin.name.clone(),
                                    node: id,
                                    input: pin.name.clone(),
                                    doc: pin.doc.clone(),
                                    range: None,
                                });
                            } else {
                                graph
                                    .exposed
                                    .retain(|e| !(e.node == id && e.input == pin.name));
                            }
                        }
                        if let Some(index) = graph
                            .exposed
                            .iter()
                            .position(|e| e.node == id && e.input == pin.name)
                        {
                            let e = &mut graph.exposed[index];
                            ui.horizontal(|ui| {
                                ui.label("Control name");
                                ui.text_edit_singleline(&mut e.name);
                            });
                            ui.add(
                                egui::TextEdit::singleline(&mut e.doc)
                                    .desired_width(400.)
                                    .hint_text("What does this control change?"),
                            );
                            if matches!(pin.kind, ValueKind::Number | ValueKind::Int) {
                                let mut limits = e.range.is_some();
                                if ui.checkbox(&mut limits, "Custom limits").changed() {
                                    e.range = limits.then(|| match pin.widget {
                                        Widget::Mm { min, max } | Widget::Slider { min, max } => {
                                            [min, max]
                                        }
                                        _ => [0., 100.],
                                    });
                                }
                                if let Some([min, max]) = &mut e.range {
                                    ui.horizontal(|ui| {
                                        ui.label("Minimum");
                                        ui.add(egui::DragValue::new(min).speed(0.1));
                                        ui.label("Maximum");
                                        ui.add(egui::DragValue::new(max).speed(0.1));
                                    });
                                }
                            }
                            if let Some([min, max]) = e
                                .range
                                .filter(|r| r[0].is_finite() && r[1].is_finite() && r[0] < r[1])
                            {
                                pin.widget = match pin.widget {
                                    Widget::Mm { .. } => Widget::Mm { min, max },
                                    _ => Widget::Slider { min, max },
                                };
                            }
                            let node = graph.node_mut(id).expect("resolved node");
                            let mut value = node.inputs.get(&pin.name).cloned();
                            ui.horizontal(|ui| {
                                ui.label("Default");
                                if ringdesign_graph_ui::widgets::pin_widget(ui, &pin, &mut value) {
                                    if let Some(v) = value {
                                        node.inputs.insert(pin.name.clone(), v);
                                    } else {
                                        node.inputs.remove(&pin.name);
                                    }
                                }
                            });
                            ui.separator();
                        }
                    });
                }
            });
    }
}

fn thumbnail(ui: &mut egui::Ui, entry: &Entry) {
    let id = egui::Id::new(("personal-preview", &entry.path));
    let cached = ui.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id));
    let texture = cached.unwrap_or_else(|| {
        let texture = (|| {
            let mut reader = image::ImageReader::new(std::io::Cursor::new(&entry.png))
                .with_guessed_format()
                .ok()?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(512);
            limits.max_image_height = Some(512);
            limits.max_alloc = Some(4 * 1024 * 1024);
            reader.limits(limits);
            let img = reader.decode().ok()?.into_rgba8();
            Some(ui.ctx().load_texture(
                entry.path.display().to_string(),
                egui::ColorImage::from_rgba_unmultiplied(
                    [img.width() as usize, img.height() as usize],
                    &img,
                ),
                egui::TextureOptions::LINEAR,
            ))
        })();
        ui.data_mut(|d| d.insert_temp(id, texture.clone()));
        texture
    });
    if let Some(texture) = texture {
        ui.image((texture.id(), egui::vec2(96., 96.)));
    }
}
