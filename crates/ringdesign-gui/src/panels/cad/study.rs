//! Size comparisons and manufacturing stages use their own worker and geometry.
use crate::{app::RingDesignerApp, camera::OrbitCamera, theme, viewport::GpuMeshRenderer};
use ringdesign_core::{BuildParams, RingDesign, manufacturing as mf, resize::Policy};
use ringdesign_graph::{graph::Graph, variants::Variant};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver},
};

struct Preview {
    renderer: Arc<Mutex<GpuMeshRenderer>>,
    camera: OrbitCamera,
    display: crate::viewport::CandidateDisplay,
}
impl Preview {
    fn new(mesh: &ringdesign_core::Mesh) -> Self {
        let mut r = GpuMeshRenderer::default();
        r.prepare_cad(mesh);
        let mut camera = OrbitCamera::default();
        camera.fit(mesh.bounds());
        Self {
            renderer: Arc::new(Mutex::new(r)),
            camera,
            display: Default::default(),
        }
    }
}
type BatchResult = Result<Arc<Vec<Variant>>, String>;
#[derive(Default)]
pub struct Sizes {
    source: u64,
    bore: f64,
    context_bore: f64,
    policy: Policy,
    bores: String,
    job: Option<(u64, Receiver<BatchResult>)>,
    variants: Option<Arc<Vec<Variant>>>,
    views: Vec<Preview>,
    error: Option<String>,
    report_key: u64,
}

pub fn sizes(
    app: &mut RingDesignerApp,
    ui: &mut egui::Ui,
    g: &mut Graph,
    d: &RingDesign,
    s: &mut Sizes,
) {
    let source = super::hash(&(
        &g,
        Arc::as_ptr(&app.lib) as usize,
        &app.design.manufacturing,
    ));
    if source != s.source {
        s.source = source;
        s.bore = d.size.inner_diameter_mm();
        s.job = None;
        s.variants = None;
        s.views.clear();
        s.error = None;
        if s.bores.is_empty() {
            s.bores = format!("{:.3},{:.3},{:.3}", s.bore - 0.5, s.bore, s.bore + 0.5);
        }
    }
    let bore = d.size.inner_diameter_mm();
    if (s.context_bore - bore).abs() > 1e-8 {
        s.context_bore = bore;
        s.bore = bore;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Nominal bore");
        ui.add(
            egui::DragValue::new(&mut s.bore)
                .speed(0.025)
                .max_decimals(4)
                .suffix(" mm"),
        );
        let mut circumference = s.bore * std::f64::consts::PI;
        if ui
            .add(
                egui::DragValue::new(&mut circumference)
                    .speed(0.1)
                    .max_decimals(3)
                    .prefix("Circumference ")
                    .suffix(" mm"),
            )
            .changed()
        {
            s.bore = circumference / std::f64::consts::PI;
        }
        if let Ok(size) = ringdesign_core::resize::size_from_bore(s.bore) {
            ui.label(size.display());
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut s.policy.preserve_head, "Preserve head dimensions");
        ui.checkbox(
            &mut s.policy.preserve_ornament_pitch,
            "Preserve ornament pitch",
        );
        ui.weak("Measured stones stay fixed; seats and live generators are recalculated");
    });
    ui.weak("This is nominal fit. Wide-band preference, comfort profile, shrink, and finishing stock remain separate controls.");
    if ui.button("Add resize to candidate").clicked() {
        match ringdesign_core::resize::size_from_bore(s.bore).and_then(|_| {
            ringdesign_graph::nodes::cad::append_resize(g, s.bore, &s.policy).map_err(Into::into)
        }) {
            Ok(_) => {}
            Err(e) => s.error = Some(e.to_string()),
        }
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label("Compare bores mm");
        ui.text_edit_singleline(&mut s.bores);
        if ui
            .add_enabled(s.job.is_none(), egui::Button::new("Build size comparison"))
            .clicked()
        {
            let bores = s
                .bores
                .split(',')
                .map(|v| v.trim().parse::<f64>())
                .collect::<Result<Vec<_>, _>>();
            match bores {
                Err(e) => s.error = Some(format!("Enter comma-separated bore diameters: {e}")),
                Ok(bores) => {
                    let key = super::hash(&(source, &s.bores, &s.policy));
                    let (tx, rx) = mpsc::channel();
                    s.job = Some((key, rx));
                    s.error = None;
                    let graph = g.clone();
                    let mut source = d.clone();
                    source.manufacturing = app.design.manufacturing.clone();
                    let lib = app.lib.clone();
                    let reg = app.graph_reg.clone();
                    let params = BuildParams {
                        theta_steps: 128,
                        profile_steps: 96,
                        refine: None,
                        ..app.preview_params
                    };
                    let policy = s.policy.clone();
                    let ctx = ui.ctx().clone();
                    std::thread::spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || -> anyhow::Result<_> {
                                let mut ev = ringdesign_graph::eval::Evaluator::with_exprs(
                                    ringdesign_script::engine(),
                                );
                                let out = ringdesign_graph::eval::evaluate_design(
                                    &mut ev, &graph, &reg, &lib, 0,
                                )?;
                                let mut d = (*out.design).clone();
                                d.graph = Some(serde_json::to_value(&graph)?);
                                d.manufacturing = source.manufacturing;
                                d.casting_trials = source.casting_trials;
                                Ok(Arc::new(ringdesign_graph::variants::evaluate(
                                    &d, &bores, &policy, &lib, params, &reg, &mut ev,
                                )?))
                            },
                        ))
                        .map_err(|_| "Size comparison failed during geometry evaluation".into())
                        .and_then(|r| r.map_err(|e| format!("{e:#}")));
                        let _ = tx.send(result);
                        ctx.request_repaint();
                    });
                }
            }
        }
        if s.job.is_some() {
            ui.spinner();
            ui.label("Checking variants and casting components…");
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(80));
        }
    });
    let current = super::hash(&(source, &s.bores, &s.policy));
    if let Some((key, rx)) = &s.job {
        match rx.try_recv() {
            Ok(result) => {
                let key = *key;
                s.job = None;
                if current == key {
                    match result {
                        Ok(variants) => {
                            s.views = variants.iter().map(|v| Preview::new(&v.mesh)).collect();
                            s.variants = Some(variants);
                            s.report_key = key;
                        }
                        Err(e) => s.error = Some(e),
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                s.job = None;
                s.error = Some("Size worker stopped".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    if let Some(e) = &s.error {
        ui.colored_label(theme::BAD, e);
    }
    if s.report_key != current {
        return;
    }
    if let Some(variants) = &s.variants {
        if ui
            .add_enabled(
                app.exporting.is_none(),
                egui::Button::new("Export compared sizes and reports…"),
            )
            .clicked()
        {
            if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                let path = parent.join(format!(
                    "sizes-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis())
                ));
                let variants = variants.clone();
                let lib = app.lib.clone();
                let ctx = ui.ctx().clone();
                let (tx, rx) = mpsc::channel();
                app.exporting = Some(rx);
                std::thread::spawn(move || {
                    let result = ringdesign_graph::variants::export(&path, &variants, &lib);
                    let message = match result {
                        Ok(()) => format!("Size batch saved to {}", path.display()),
                        Err(e) => format!("Size export failed: {e:#}"),
                    };
                    let _ = tx.send(message);
                    ctx.request_repaint();
                });
            }
        }
        egui::ScrollArea::both()
            .id_salt("size_variants")
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    for (index, v) in variants.iter().enumerate() {
                        ui.allocate_ui_with_layout(
                            egui::vec2(290.0, 430.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.strong(format!(
                                    "Bore {:.3} mm",
                                    v.design.size.inner_diameter_mm()
                                ));
                                ui.label(format!(
                                    "{} • {:.2} g",
                                    v.design.size.display(),
                                    v.report["grams"].as_f64().unwrap_or(0.0)
                                ));
                                if let Some(reports) = v.report["manufacturing"].as_array() {
                                    for r in reports {
                                        if let Some(e) = r["unassessed"].as_str() {
                                            ui.colored_label(theme::WARN, e);
                                        } else {
                                            let state = r["release"]["status"]
                                                .as_str()
                                                .unwrap_or("Unassessed");
                                            let color = if matches!(state, "Blocked" | "Invalid") {
                                                theme::BAD
                                            } else {
                                                theme::WARN
                                            };
                                            ui.colored_label(
                                                color,
                                                format!(
                                                    "Part {}: {state}",
                                                    r["setup"]["component"]
                                                ),
                                            );
                                        }
                                    }
                                }
                                if ui.button("Use this size as candidate").clicked() {
                                    if let Some(graph) = &v.design.graph {
                                        match serde_json::from_value(graph.clone()) {
                                            Ok(next) => *g = next,
                                            Err(e) => s.error = Some(e.to_string()),
                                        }
                                    }
                                }
                                let view = &mut s.views[index];
                                crate::viewport::candidate_view(
                                    ui,
                                    view.renderer.clone(),
                                    &mut view.camera, &mut view.display, d.shank.head.theta_deg as f32,
                                );
                            },
                        );
                    }
                })
            });
    }
}

#[derive(Default)]
pub struct Stages {
    source: u64,
    component: Option<u64>,
    selected: usize,
    job: Option<(u64, Receiver<Result<mf::stages::Stages, String>>)>,
    data: Option<mf::stages::Stages>,
    views: Vec<Preview>,
    error: Option<String>,
    report_key: u64,
}
pub fn stages(app: &RingDesignerApp, ui: &mut egui::Ui, g: &Graph, d: &RingDesign, s: &mut Stages) {
    let source = super::hash(&(g, &app.design.manufacturing, Arc::as_ptr(&app.lib) as usize));
    if source != s.source {
        s.source = source;
        s.job = None;
        s.data = None;
        s.views.clear();
        s.error = None;
    }
    if let Some(doc) = &d.cad {
        egui::ComboBox::from_id_salt("stage_component")
            .selected_text(
                s.component
                    .map_or("Choose component".into(), |id| format!("Component #{id}")),
            )
            .show_ui(ui, |ui| {
                for f in doc
                    .features
                    .iter()
                    .filter(|f| doc.outputs.contains(&f.id) && !f.component.reference)
                {
                    ui.selectable_value(&mut s.component, Some(f.id), &f.name);
                }
            });
    }
    let current = super::hash(&(source, s.component));
    if ui
        .add_enabled(s.job.is_none(), egui::Button::new("Prepare stage previews"))
        .clicked()
    {
        let mut setup = s
            .component
            .and_then(|id| {
                d.cad
                    .as_ref()?
                    .features
                    .iter()
                    .find(|f| f.id == id)?
                    .component
                    .manufacturing
                    .clone()
            })
            .or_else(|| app.design.manufacturing.clone())
            .unwrap_or_else(|| mf::Setup::from_design(d));
        setup.component = s.component;
        let (tx, rx) = mpsc::channel();
        s.job = Some((current, rx));
        s.error = None;
        let g = g.clone();
        let lib = app.lib.clone();
        let reg = app.graph_reg.clone();
        let params = BuildParams {
            theta_steps: 128,
            profile_steps: 96,
            refine: None,
            ..app.preview_params
        };
        let ctx = ui.ctx().clone();
        std::thread::spawn(move || {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<_> {
                    let mut ev =
                        ringdesign_graph::eval::Evaluator::with_exprs(ringdesign_script::engine());
                    let out = ringdesign_graph::eval::evaluate_design(&mut ev, &g, &reg, &lib, 0)?;
                    mf::stages::evaluate(&out.design, &lib, &setup, params)
                }))
                .map_err(|_| "Stage preparation failed".into())
                .and_then(|r| r.map_err(|e| format!("{e:#}")));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    if let Some((key, rx)) = &s.job {
        match rx.try_recv() {
            Ok(result) => {
                let key = *key;
                s.job = None;
                if key == current {
                    match result {
                        Ok(data) => {
                            s.views = [&data.nominal, &data.pattern, &data.as_cast, &data.finished]
                                .into_iter()
                                .map(Preview::new)
                                .collect();
                            s.data = Some(data);
                            s.report_key = key;
                        }
                        Err(e) => s.error = Some(e),
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                s.job = None;
                s.error = Some("Stage worker stopped".into());
            }
            Err(mpsc::TryRecvError::Empty) => {
                ui.spinner();
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(80));
            }
        }
    }
    if let Some(e) = &s.error {
        ui.colored_label(theme::BAD, e);
    }
    if s.report_key != current {
        return;
    }
    if let Some(data) = &s.data {
        ui.horizontal_wrapped(|ui| {
            for (i, label) in [
                "Nominal target",
                "Compensated pattern",
                "Expected as-cast",
                "Finished target",
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(&mut s.selected, i, *label);
            }
        });
        for note in &data.notes {
            ui.weak(note);
        }
        let mesh = [&data.nominal, &data.pattern, &data.as_cast, &data.finished][s.selected];
        if let Some((lo, hi)) = mesh.bounds() {
            ui.label(format!(
                "{:.3} × {:.3} × {:.3} mm • {:.3} mm³",
                hi.0 - lo.0,
                hi.1 - lo.1,
                hi.2 - lo.2,
                mesh.volume_mm3()
            ));
        }
        let v = &mut s.views[s.selected];
        crate::viewport::candidate_view(ui, v.renderer.clone(), &mut v.camera, &mut v.display, d.shank.head.theta_deg as f32);
    }
}
