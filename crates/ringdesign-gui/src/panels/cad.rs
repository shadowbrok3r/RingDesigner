//! A conventional feature tree edits the existing graph. A candidate has its
//! own evaluated solids and renderer; Enter applies one edit, Escape cancels.
use crate::{
    app::RingDesignerApp,
    camera::{OrbitCamera, StandardView},
    pane::PaneKind,
    theme,
    viewport::GpuMeshRenderer,
};
use egui::{Stroke, vec2};
use ringdesign_core::{
    BuildParams, RingDesign,
    cad::{self, Boolean, Evaluated, Feature, Operation},
    sketch::{Constraint, Geometry, Sketch},
};
use ringdesign_graph::{
    graph::{Graph, NodeId},
    nodes::cad as graph_cad,
};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver},
};

mod study;
struct View {
    design: RingDesign,
    evaluated: Evaluated,
    pairs: Vec<cad::assembly::PairReport>,
    walls: Vec<cad::measure::Thickness>,
}
struct Job {
    key: u64,
    receiver: Receiver<Result<View, String>>,
}
pub struct CadState {
    draft: Option<Graph>,
    source: u64,
    selected: Option<NodeId>,
    tab: usize,
    job: Option<Job>,
    view: Option<View>,
    view_key: u64,
    requested: u64,
    renderer: Arc<Mutex<GpuMeshRenderer>>,
    camera: OrbitCamera,
    error: Option<String>,
    message: String,
    json: String,
    json_node: Option<NodeId>,
    tool: usize,
    pending: Vec<u64>,
    point: Option<u64>,
    constraint_kind: usize,
    dimension: f64,
    construction: bool,
    sketch_scale: f32,
    isolated: Option<u64>,
    explode: f64,
    edge: Option<(u64, usize)>,
    rollback: Option<u64>,
    joints: Option<Vec<cad::Joint>>,
    sizes: study::Sizes,
    stages: study::Stages,
    section_axis: usize,
    section_offset: f64,
}
impl Default for CadState {
    fn default() -> Self {
        Self {
            draft: None,
            source: 0,
            selected: None,
            tab: 0,
            job: None,
            view: None,
            view_key: 0,
            requested: 0,
            renderer: Arc::new(Mutex::new(GpuMeshRenderer::default())),
            camera: OrbitCamera::default(),
            error: None,
            message: String::new(),
            json: String::new(),
            json_node: None,
            tool: 0,
            pending: vec![],
            point: None,
            constraint_kind: 0,
            dimension: 6.0,
            construction: false,
            sketch_scale: 25.0,
            isolated: None,
            explode: 0.0,
            edge: None,
            rollback: None,
            joints: None,
            sizes: Default::default(),
            stages: Default::default(),
            section_axis: 2,
            section_offset: 0.0,
        }
    }
}
impl CadState {
    pub fn open_section(&mut self) {
        self.tab = 6;
    }
}
fn hash<T: serde::Serialize>(v: &T) -> u64 {
    ringdesign_core::manufacturing::package::fingerprint(&serde_json::to_vec(v).unwrap_or_default())
}
fn source_key(app: &RingDesignerApp) -> u64 {
    if app.design.graph.is_some() {
        hash(&(&app.design.graph, Arc::as_ptr(&app.lib) as usize))
    } else {
        hash(&(&app.design, Arc::as_ptr(&app.lib) as usize))
    }
}
pub fn report_panel(app: &RingDesignerApp, ui: &mut egui::Ui) {
    let state = &app.cad;
    ui.strong("CAD candidate");
    let key = state
        .draft
        .as_ref()
        .map(|g| hash(&(g, state.rollback)))
        .or_else(|| {
            app.graph_ed
                .as_ref()
                .map(|ed| hash(&(ed.graph(), state.rollback)))
        });
    if let Some(e) = &state.error {
        ui.colored_label(theme::BAD, e);
    }
    if key != Some(state.view_key) {
        ui.weak("Parameters need evaluation; previous preview remains visible");
        return;
    }
    let Some(v) = &state.view else {
        ui.weak("Add a feature and preview it");
        return;
    };
    ui.weak(if state.draft.is_some() {
        "Uncommitted edit — Apply or Cancel"
    } else {
        "Current feature geometry"
    });
    for (index, c) in v.evaluated.components.iter().enumerate() {
        ui.separator();
        ui.strong(&c.name);
        ui.label(if c.settings.reference {
            "Reference stone"
        } else {
            &c.settings.material
        });
        if let Some((lo, hi)) = c.mesh.bounds() {
            ui.label(format!(
                "{:.2} × {:.2} × {:.2} mm",
                hi.0 - lo.0,
                hi.1 - lo.1,
                hi.2 - lo.2
            ));
        }
        ui.label(format!(
            "{:.2} mm³ • {} faces • {} edges",
            c.mesh.volume_mm3(),
            c.body.faces.len(),
            c.body.edges.len()
        ));
        let w = &v.walls[index];
        ui.label(format!(
            "Sampled wall {}; {} below {:.2} mm",
            w.sampled_min_mm
                .map_or("unassessed".into(), |v| format!("{v:.3} mm")),
            w.below_limit,
            w.limit_mm
        ));
        ui.weak(w.note);
    }
    ui.weak("Use Casting for component release, flask fit, and pattern preparation");
}
fn launch(state: &mut CadState, g: Graph, app: &RingDesignerApp, ctx: egui::Context) {
    let rollback = state.rollback;
    let key = hash(&(&g, rollback));
    let lib = app.lib.clone();
    let reg = app.graph_reg.clone();
    let params = BuildParams {
        theta_steps: 128,
        profile_steps: 96,
        refine: None,
        ..app.preview_params
    };
    let (tx, receiver) = mpsc::channel();
    state.job = Some(Job { key, receiver });
    state.requested = key;
    state.error = None;
    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<View> {
                let mut evaluator =
                    ringdesign_graph::eval::Evaluator::with_exprs(ringdesign_script::engine());
                let source = if let Some(id) = rollback {
                    graph_cad::history_design(&mut evaluator, &g, NodeId(id), &reg, &lib)?
                } else {
                    ringdesign_graph::eval::evaluate_design(&mut evaluator, &g, &reg, &lib, 0)?
                        .design
                };
                let mut d = (*source).clone();
                if d.cad.is_none() {
                    let mut doc = cad::Document::default();
                    doc.append(Feature {
                        id: 0,
                        name: "Procedural shank".into(),
                        enabled: true,
                        operation: Operation::Band,
                        component: Default::default(),
                    })?;
                    d.cad = Some(doc);
                }
                d.cad.as_mut().unwrap().through = None;
                let evaluated = cad::evaluate(&d, &lib, params)?;
                let pairs = cad::assembly::inspect(&d, &evaluated);
                let walls = evaluated
                    .components
                    .iter()
                    .map(|c| {
                        cad::measure::thickness(
                            &c.mesh,
                            c.settings
                                .manufacturing
                                .as_ref()
                                .map_or(d.draft.min_section_mm, |s| s.recipe.min_section_mm),
                        )
                    })
                    .collect();
                Ok(View {
                    design: d,
                    evaluated,
                    pairs,
                    walls,
                })
            }))
            .map_err(|_| "CAD operation failed; original design is unchanged".to_string())
            .and_then(|r| r.map_err(|e| format!("{e:#}")));
        let _ = tx.send(result);
        ctx.request_repaint();
    });
}
fn upload(state: &mut CadState, fit: bool) {
    let Some(view) = &state.view else {
        return;
    };
    let mut metal = ringdesign_core::Mesh::default();
    let mut gems = ringdesign_core::Mesh::default();
    for (index, c) in view.evaluated.components.iter().enumerate() {
        if !c.settings.visible || state.isolated.is_some_and(|id| id != c.id) {
            continue;
        }
        let target = if c.settings.reference {
            &mut gems
        } else {
            &mut metal
        };
        let offset = target.vertices.len() as u32;
        target.vertices.extend(
            c.mesh.vertices.iter().map(|v| {
                ringdesign_core::Vec3(v.0, v.1, v.2 + index as f32 * state.explode as f32)
            }),
        );
        target.normals.extend_from_slice(&c.mesh.normals);
        target
            .faces
            .extend(c.mesh.faces.iter().map(|f| f.map(|i| i + offset)));
    }
    if fit {
        state.camera.fit(metal.bounds());
    }
    if let Ok(mut r) = state.renderer.lock() {
        r.prepare_cad(&metal);
        r.prepare_gems(GpuMeshRenderer::stage_plain(&gems));
    }
}
pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let mut state = std::mem::take(&mut app.cad);
    let source = source_key(app);
    if state.source != source {
        state.draft = None;
        state.message.clear();
        state.source = source;
        state.pending.clear();
        state.requested = 0;
        state.view_key = 0;
        state.job = None;
        state.rollback = None;
        state.joints = None;
    }
    let mut original = match app
        .graph_ed
        .as_ref()
        .map(|ed| Ok(ed.graph().clone()))
        .unwrap_or_else(|| graph_cad::from_document(&app.design))
    {
        Ok(g) => g,
        Err(e) => {
            ui.colored_label(theme::BAD, e.to_string());
            app.cad = state;
            return;
        }
    };
    let mut g = state.draft.clone().unwrap_or_else(|| original.clone());
    let before = hash(&(&g, state.rollback));
    ui.horizontal_wrapped(|ui| {
        for (i, name) in [
            "Features",
            "Sketch",
            "Components",
            "Debug",
            "Sizes",
            "Stages",
            "Section",
        ]
        .iter()
        .enumerate()
        {
            ui.selectable_value(&mut state.tab, i, *name);
        }
        ui.weak("Millimeters • editable feature history");
        if ui.small_button("Ring view").clicked() {
            app.focus(PaneKind::Solid);
        }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.menu_button("Add feature", |ui| {
            let previous = state
                .edge
                .map(|(id, _)| id)
                .or_else(|| {
                    state
                        .selected
                        .filter(|id| g.node(*id).is_some_and(|n| n.kind == "cad.feature"))
                        .map(|id| id.0)
                })
                .unwrap_or_else(|| {
                    g.nodes
                        .iter()
                        .rev()
                        .find(|n| n.kind == "cad.feature")
                        .map_or(0, |n| n.id.0)
                });
            for mut op in starters(previous) {
                if ui.button(op.label()).clicked() {
                    if let (
                        Some((_, edge)),
                        Operation::Fillet { edges, .. } | Operation::Chamfer { edges, .. },
                    ) = (state.edge, &mut op)
                    {
                        *edges = vec![edge];
                    }
                    match graph_cad::append(&mut g, op) {
                        Ok(id) => {
                            state.selected = Some(id);
                            state.json_node = None;
                        }
                        Err(e) => state.error = Some(e.to_string()),
                    }
                    ui.close();
                }
            }
        });
        ui.menu_button("Examples", |ui| {
            for name in cad::examples::NAMES {
                if ui.button(*name).clicked() {
                    match cad::examples::design(name)
                        .and_then(|d| graph_cad::from_document(&d).map_err(Into::into))
                    {
                        Ok(next) => {
                            g = next;
                            state.selected = g
                                .nodes
                                .iter()
                                .find(|n| n.kind == "cad.feature")
                                .map(|n| n.id);
                            state.tab = 0;
                            state.rollback = None;
                            state.joints = None;
                            state.error = None;
                        }
                        Err(e) => state.error = Some(e.to_string()),
                    }
                    ui.close();
                }
            }
        });
        if ui.button("Preview").clicked() && state.job.is_none() {
            launch(&mut state, g.clone(), app, ui.ctx().clone());
        }
        let ready = state.rollback.is_none()
            && state.view_key == before
            && state.draft.is_some()
            && state.job.is_none()
            && state.error.is_none();
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.any())
            && !ui.ctx().egui_wants_keyboard_input()
            && state.pending.is_empty();
        if ui
            .add_enabled(ready, egui::Button::new("Apply edit"))
            .clicked()
            || (enter && ready)
        {
            app.history.commit(&app.design);
            if let Some(view) = &state.view {
                let mut applied = view.design.clone();
                applied.manufacturing = app.design.manufacturing.clone();
                applied.casting_trials = app.design.casting_trials.clone();
                app.design = applied;
            }
            app.open_graph(g.clone());
            // The evaluated candidate is already available. Record the whole
            // edit now so immediate Undo does not depend on the rebuild timer.
            app.history.commit(&app.design);
            original = g.clone();
            app.focus(PaneKind::Cad);
            state.draft = None;
            state.source = source_key(app);
            state.message = "Feature edit applied; undo restores its source graph".into();
        } else if enter && state.draft.is_some() && state.job.is_none() {
            launch(&mut state, g.clone(), app, ui.ctx().clone());
        }
        if ui.button("Cancel edit").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            g = original.clone();
            state.draft = None;
            state.pending.clear();
            state.error = None;
            state.requested = 0;
            state.rollback = None;
            state.message = "Candidate discarded".into();
        }
        if state.job.is_some() {
            ui.spinner();
            ui.label("Evaluating solid…");
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(60));
        }
    });
    if let Some(e) = &state.error {
        ui.colored_label(theme::BAD, e);
    }
    if !state.message.is_empty() {
        ui.weak(&state.message);
    }
    let tree = g
        .nodes
        .iter()
        .filter(|n| matches!(n.kind.as_str(), "cad.feature" | "design.resize"))
        .map(|n| {
            (
                n.id,
                n.label.clone().unwrap_or_else(|| {
                    n.params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&n.kind)
                        .to_string()
                }),
            )
        })
        .collect::<Vec<_>>();
    egui::ScrollArea::horizontal()
        .id_salt("feature_tree")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (id, label) in &tree {
                    if ui
                        .selectable_label(
                            state.selected == Some(*id),
                            format!("{} · {}", id.0, label),
                        )
                        .clicked()
                    {
                        state.selected = Some(*id);
                        app.selected_node = Some(*id);
                        state.pending.clear();
                        state.json_node = None;
                    }
                }
            });
        });
    if state.tab <= 3 {
        if let Some(id) = state.selected {
            let bound_operation = g.wire_into(id, "operation").is_some()
                || g.node(id)
                    .is_some_and(|n| n.inputs.contains_key("operation"));
            if let Some(n) = g.node_mut(id) {
                if n.kind == "cad.feature" {
                    if let Ok(mut f) = serde_json::from_value::<Feature>(n.params.clone()) {
                        egui::ScrollArea::vertical().id_salt("feature_properties").max_height(if state.tab==1 {180.0} else {280.0}).show(ui,|ui|{
                        ui.horizontal_wrapped(|ui|{ui.text_edit_singleline(&mut f.name);ui.checkbox(&mut f.enabled,"Enabled");ui.weak(format!("Feature #{}",id.0));});
                        match state.tab {
                            0=>{if bound_operation {ui.weak("Operation is bound to a graph input; edit its upstream parameters in Graph.");} else {operation_ui(ui,&mut f.operation,&tree);}},
                            1=>{if bound_operation {ui.weak("Sketch is controlled by the connected operation input");} else if let Some(sketch)=f.operation.sketch_mut() {sketch_controls(ui,sketch,&mut state);} else {ui.weak("Select an extrusion, revolution, sweep, or loft to edit its sketch");}},
                            2=>component_ui(ui,&mut f),
                            _=>{
                                if state.json_node!=Some(id) {state.json=serde_json::to_string_pretty(&f).unwrap_or_default();state.json_node=Some(id);}
                                ui.weak("Complete source parameters, including sweep paths, loft sections, and topology selections");
                                ui.add(egui::TextEdit::multiline(&mut state.json).font(egui::TextStyle::Monospace).desired_rows(7).desired_width(f32::INFINITY));
                                if ui.button("Read parameters into candidate").clicked() {match serde_json::from_str::<Feature>(&state.json) {Ok(next)=>{f=next;f.id=id.0;},Err(e)=>state.error=Some(e.to_string())}}
                            },
                        }
                    });
                        if state.tab == 1 && !bound_operation {
                            if let Some(s) = f.operation.sketch_mut() {
                                sketch_canvas(ui, s, &mut state);
                            }
                        }
                        n.params = serde_json::to_value(f).unwrap();
                    }
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.weak("Graph source parameters remain editable in the node inspector.");
                        if ui.button("Open graph inspector").clicked() {
                            app.selected_node = Some(id);
                            app.focus(PaneKind::Graph);
                        }
                    });
                }
            }
        }
    }
    if state.tab == 2 {
        joints_ui(ui, &mut g, &mut state);
    }
    let context = state
        .view
        .as_ref()
        .map(|v| v.design.clone())
        .unwrap_or_else(|| app.design.clone());
    if state.tab == 4 {
        study::sizes(app, ui, &mut g, &context, &mut state.sizes);
    }
    if state.tab == 5 {
        study::stages(app, ui, &g, &context, &mut state.stages);
    }
    let current = hash(&(&g, state.rollback));
    if hash(&g) != hash(&original) {
        state.draft = Some(g.clone());
    } else {
        state.draft = None;
    }
    if let Some(job) = &state.job {
        match job.receiver.try_recv() {
            Ok(result) => {
                let job_key = job.key;
                state.job = None;
                if job_key == current {
                    match result {
                        Ok(view) => {
                            state.view = Some(view);
                            state.view_key = current;
                            upload(&mut state, true);
                        }
                        Err(e) => {
                            state.error = Some(e);
                            state.view_key = 0;
                        }
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                state.job = None;
                state.error = Some("CAD worker stopped".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    if (state.view.is_none() || state.draft.is_none())
        && state.job.is_none()
        && state.requested != current
        && g.nodes
            .iter()
            .any(|n| matches!(n.kind.as_str(), "cad.feature" | "design.resize"))
    {
        launch(&mut state, g.clone(), app, ui.ctx().clone());
    }
    if state.tab == 6 {
        if state.view_key != current {
            ui.colored_label(
                theme::WARN,
                "Previous section — preview the changed parameters to update it",
            );
        }
        section_ui(ui, &mut state);
    }
    if state.tab != 1 && state.tab < 4 {
        if state.view_key != current {
            ui.colored_label(
                theme::WARN,
                "Parameters changed — preview to evaluate this candidate",
            );
        }
        let mut redraw = false;
        ui.horizontal_wrapped(|ui| {
            for v in StandardView::ALL {
                if ui.small_button(v.label()).clicked() {
                    state.camera.set_view(*v);
                }
            }
            if ui.small_button("Fit").clicked() {
                redraw = true;
            }
            if ui
                .add_enabled(
                    state.selected.is_some() && state.job.is_none(),
                    egui::Button::new("Preview through selected feature"),
                )
                .clicked()
            {
                state.rollback = state.selected.map(|id| id.0);
                launch(&mut state, g.clone(), app, ui.ctx().clone());
            }
            if state.rollback.is_some() && ui.button("Return to end of history").clicked() {
                state.rollback = None;
                launch(&mut state, g.clone(), app, ui.ctx().clone());
            }
        });
        ui.horizontal_wrapped(|ui| {
            redraw |= ui
                .add(egui::Slider::new(&mut state.explode, 0.0..=20.0).text("Explode mm"))
                .changed();
            if ui.small_button("Show all components").clicked() {
                state.isolated = None;
                redraw = true;
            }
        });
        if let Some(view) = &state.view {
            ui.horizontal_wrapped(|ui| {
                for c in &view.evaluated.components {
                    if ui
                        .selectable_label(state.isolated == Some(c.id), &c.name)
                        .clicked()
                    {
                        state.isolated = Some(c.id);
                        redraw = true;
                    }
                }
            });
            if let Some(f) = state
                .selected
                .and_then(|id| view.evaluated.features.iter().find(|f| f.id == id.0))
            {
                ui.weak(format!(
                    "{} faces • {} selectable edges{}",
                    f.faces,
                    f.edges,
                    if f.suppressed { " • suppressed" } else { "" }
                ));
            }
            let metal: Vec<_> = view
                .evaluated
                .components
                .iter()
                .filter(|c| !c.settings.reference)
                .collect();
            ui.label(format!(
                "{} metal components • {:.2} mm³ • mesh checks passed",
                metal.len(),
                metal.iter().map(|c| c.mesh.volume_mm3()).sum::<f64>()
            ));
        }
        if redraw {
            upload(&mut state, true);
        }
        if state.tab == 2 && state.view_key == current && state.rollback.is_none() {
            if let Some(view) = &state.view {
                egui::CollapsingHeader::new("Interference and assembly clearance").show(ui, |ui| {
                    for pair in &view.pairs {
                        ui.label(format!(
                            "#{} / #{}: overlap {:?} mm³; clearance bound {:?} mm",
                            pair.a, pair.b, pair.interference_mm3, pair.clearance_estimate_mm
                        ));
                        ui.weak(&pair.note);
                    }
                });
            }
            if ui
                .add_enabled(
                    state.draft.is_none() && app.is_current() && app.exporting.is_none(),
                    egui::Button::new("Export component package…"),
                )
                .clicked()
            {
                if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                    let path = parent.join(format!(
                        "assembly-{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis())
                    ));
                    let d = app.design.clone();
                    let lib = app.lib.clone();
                    let params = app.export_params;
                    let ctx = ui.ctx().clone();
                    let (tx, rx) = mpsc::channel();
                    app.exporting = Some(rx);
                    std::thread::spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            cad::assembly::export(&path, &d, &lib, params)
                        }));
                        let message = match result {
                            Ok(Ok(_)) => format!("Assembly saved to {}", path.display()),
                            Ok(Err(e)) => format!("Assembly export failed: {e:#}"),
                            Err(_) => "Assembly export failed during geometry evaluation".into(),
                        };
                        let _ = tx.send(message);
                        ctx.request_repaint();
                    });
                }
            }
        }
        if let Some((id, edge)) = state.edge {
            ui.weak(format!(
                "Selected component #{id}, edge {edge} • Add Fillet or Chamfer to use it"
            ));
        }
        let (rect, response) =
            crate::viewport::candidate_view(ui, state.renderer.clone(), &mut state.camera);
        let project = state.camera.projector(rect);
        let mut nearest = None;
        if let Some(view) = &state.view {
            for (index, c) in view.evaluated.components.iter().enumerate() {
                if !c.settings.visible || state.isolated.is_some_and(|id| id != c.id) {
                    continue;
                }
                for (edge, points) in c.edges.iter().enumerate() {
                    let points: Vec<_> = points
                        .iter()
                        .map(|p| {
                            project.at([
                                p[0] as f32,
                                p[1] as f32,
                                p[2] as f32 + index as f32 * state.explode as f32,
                            ])
                        })
                        .collect();
                    if state.edge == Some((c.id, edge)) {
                        ui.painter_at(rect).add(egui::Shape::line(
                            points.clone(),
                            Stroke::new(2.0, theme::WARN),
                        ));
                    }
                    if response.clicked() {
                        if let Some(cursor) = response.interact_pointer_pos() {
                            for pair in points.windows(2) {
                                let d = pair[1] - pair[0];
                                let t = ((cursor - pair[0]).dot(d) / d.length_sq().max(1e-8))
                                    .clamp(0.0, 1.0);
                                let distance = cursor.distance(pair[0] + d * t);
                                if distance < 10.0
                                    && nearest.is_none_or(|(_, _, best)| distance < best)
                                {
                                    nearest = Some((c.id, edge, distance));
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some((id, edge, _)) = nearest {
            state.edge = Some((id, edge));
            state.selected = Some(NodeId(id));
        }
        if matches!(state.tab, 0 | 2) && state.rollback.is_none() {
            direct_handles(ui, rect, &state, &mut g);
        }
    }
    if hash(&g) != hash(&original) {
        state.draft = Some(g);
    }
    app.cad = state;
}

fn direct_handles(ui: &mut egui::Ui, rect: egui::Rect, state: &CadState, g: &mut Graph) {
    let Some(id) = state.selected else {
        return;
    };
    if g.wire_into(id, "operation").is_some() {
        return;
    }
    let Some(node) = g.node_mut(id) else {
        return;
    };
    if node.inputs.contains_key("operation") {
        return;
    }
    let Ok(mut feature) = serde_json::from_value::<Feature>(node.params.clone()) else {
        return;
    };
    let Some(view) = &state.view else {
        return;
    };
    let projector = state.camera.projector(rect);
    let painter = ui.painter_at(rect);
    let anchor = feature.component.ring_anchor_deg;
    let height = feature.component.anchor_height_mm;
    let world = |p: [f64; 3]| {
        let p = if let Some(theta) = anchor {
            let a = theta.to_radians();
            let r = view.design.inner_radius_mm() + view.design.profile.thickness_mm + height;
            [
                (r + p[2]) * a.cos() - p[1] * a.sin(),
                (r + p[2]) * a.sin() + p[1] * a.cos(),
                -p[0],
            ]
        } else {
            p
        };
        p.map(|v| v as f32)
    };
    let label_row = std::cell::Cell::new(0);
    let grip = |label: &str,
                value: &mut f64,
                start: [f64; 3],
                end: [f64; 3],
                direction: [f64; 3],
                gain: f64,
                minimum: f64,
                units: &str| {
        let a = projector.at(world(start));
        let b = projector.at(world(end));
        if !rect.contains(b) {
            return;
        }
        let unit = projector.at(world(std::array::from_fn(|k| end[k] + direction[k]))) - b;
        let len = unit.length();
        if len < 2.0 {
            return;
        }
        painter.line_segment([a, b], Stroke::new(1.0, theme::INFO));
        painter.circle_filled(b, 5.0, theme::INFO);
        let label_left = b.x > rect.center().x;
        painter.text(
            b + vec2(
                if label_left { -8.0 } else { 8.0 },
                5.0 + label_row.get() as f32 * 15.0,
            ),
            if label_left {
                egui::Align2::RIGHT_TOP
            } else {
                egui::Align2::LEFT_TOP
            },
            format!("{label} {value:.3} {units}"),
            egui::FontId::proportional(12.0),
            theme::TEXT,
        );
        label_row.set(label_row.get() + 1);
        let grip_id = ui.id().with(("dimension_grip", id.0, label));
        let response = ui.interact(
            egui::Rect::from_center_size(b, vec2(16.0, 16.0)),
            grip_id,
            egui::Sense::drag(),
        );
        type Drag = (f64, egui::Pos2, egui::Vec2, f64);
        if response.drag_started() {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(grip_id, (*value, origin, unit, gain)));
            }
        }
        if response.dragged() {
            if let (Some((initial, origin, axis, gain)), Some(pointer)) = (
                ui.ctx().data(|d| d.get_temp::<Drag>(grip_id)),
                response.interact_pointer_pos(),
            ) {
                *value = (initial
                    + ((pointer - origin).dot(axis) / axis.length_sq()) as f64 * gain)
                    .max(minimum);
            }
        }
        if response.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<Drag>(grip_id));
        }
        response.on_hover_text("Drag to change this source dimension. Numeric fields accept exact values; Preview/Enter evaluates, Escape cancels.");
    };
    let handle = |label, value: &mut f64, start, end, direction, gain| {
        grip(label, value, start, end, direction, gain, 0.001, "mm");
    };
    match &mut feature.operation {
        Operation::Box { size } => {
            for axis in 0..3 {
                let mut start = [0.0; 3];
                let mut end = [0.0; 3];
                let mut direction = [0.0; 3];
                start[axis] = -size[axis] / 2.0;
                end[axis] = size[axis] / 2.0;
                direction[axis] = 1.0;
                handle(
                    ["Width X", "Length Y", "Height Z"][axis],
                    &mut size[axis],
                    start,
                    end,
                    direction,
                    2.0,
                );
            }
        }
        Operation::Cylinder {
            radius_mm,
            height_mm,
        } => {
            let r = *radius_mm;
            let h = *height_mm;
            handle(
                "Radius",
                radius_mm,
                [0.0; 3],
                [r, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                1.0,
            );
            handle(
                "Height",
                height_mm,
                [0.0, 0.0, -h / 2.0],
                [0.0, 0.0, h / 2.0],
                [0.0, 0.0, 1.0],
                2.0,
            );
        }
        Operation::Sphere { radius_mm } => {
            let r = *radius_mm;
            handle(
                "Radius",
                radius_mm,
                [0.0; 3],
                [r, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                1.0,
            );
        }
        Operation::Torus { major_mm, minor_mm } => {
            let a = *major_mm;
            let b = *minor_mm;
            handle(
                "Major radius",
                major_mm,
                [0.0; 3],
                [a, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                1.0,
            );
            handle(
                "Tube radius",
                minor_mm,
                [a, 0.0, 0.0],
                [a + b, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                1.0,
            );
        }
        Operation::TwistedRing {
            major_mm,
            radial_mm,
            axial_mm,
            ..
        } => {
            let (r, radial, axial) = (*major_mm, *radial_mm, *axial_mm);
            handle(
                "Major radius",
                major_mm,
                [0.0; 3],
                [r, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                1.0,
            );
            handle(
                "Radial thickness",
                radial_mm,
                [r - radial / 2.0, 0.0, 0.0],
                [r + radial / 2.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                2.0,
            );
            handle(
                "Band width",
                axial_mm,
                [r, 0.0, -axial / 2.0],
                [r, 0.0, axial / 2.0],
                [0.0, 0.0, 1.0],
                2.0,
            );
        }
        Operation::Transform { translation, .. } => {
            let base = *translation;
            for axis in 0..3 {
                let mut end = base;
                let mut direction = [0.0; 3];
                end[axis] += 4.0;
                direction[axis] = 1.0;
                grip(
                    ["Position X", "Position Y", "Position Z"][axis],
                    &mut translation[axis],
                    base,
                    end,
                    direction,
                    1.0,
                    f64::NEG_INFINITY,
                    "mm",
                );
            }
        }
        Operation::Extrude {
            sketch, height_mm, ..
        } => {
            if let Ok(plane) = sketch.plane.plane() {
                if let Some(normal) = plane.normal() {
                    let base = sketch.plane.origin;
                    let end = std::array::from_fn(|i| base[i] + normal[i] * *height_mm);
                    handle("Extrusion", height_mm, base, end, normal, 1.0);
                }
            }
        }
        _ => {}
    }
    if let Some(angle) = &mut feature.component.ring_anchor_deg {
        let radius = view.design.inner_radius_mm() + view.design.profile.thickness_mm + height;
        grip(
            "Ring position",
            angle,
            [0.0; 3],
            [0.0, 2.0, 0.0],
            [0.0, 1.0, 0.0],
            180.0 / (std::f64::consts::PI * radius.max(0.001)),
            f64::NEG_INFINITY,
            "°",
        );
        grip(
            "Radial placement",
            &mut feature.component.anchor_height_mm,
            [0.0, 0.0, -height],
            [0.0; 3],
            [0.0, 0.0, 1.0],
            1.0,
            f64::NEG_INFINITY,
            "mm",
        );
    }
    node.params = serde_json::to_value(feature).unwrap();
}

fn joints_ui(ui: &mut egui::Ui, g: &mut Graph, state: &mut CadState) {
    let Some(view) = &state.view else {
        return;
    };
    let joints = state.joints.get_or_insert_with(|| {
        view.design
            .cad
            .as_ref()
            .map(|d| d.joints.clone())
            .unwrap_or_default()
    });
    egui::CollapsingHeader::new("Solder joints and intended clearance").show(ui, |ui| {
        let mut remove = None;
        for (index, j) in joints.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut j.a).prefix("Part #"));
                    ui.add(egui::DragValue::new(&mut j.b).prefix("to #"));
                    ui.add(
                        egui::DragValue::new(&mut j.clearance_mm)
                            .range(0.0..=10.0)
                            .speed(0.01)
                            .suffix(" mm clearance"),
                    );
                    ui.text_edit_singleline(&mut j.method);
                    if ui.small_button("Remove").clicked() {
                        remove = Some(index);
                    }
                });
                ui.text_edit_singleline(&mut j.notes);
            });
        }
        if let Some(index) = remove {
            joints.remove(index);
        }
        ui.horizontal(|ui| {
            if ui.button("Add joint").clicked() {
                joints.push(cad::Joint {
                    a: view.evaluated.components.first().map_or(0, |c| c.id),
                    b: view.evaluated.components.get(1).map_or(0, |c| c.id),
                    clearance_mm: 0.05,
                    method: "Solder".into(),
                    notes: String::new(),
                });
            }
            if ui.button("Update joints in candidate").clicked() {
                match graph_cad::append_property(
                    g,
                    "/cad/joints",
                    serde_json::to_value(&joints).unwrap(),
                ) {
                    Ok(_) => {}
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
        });
    });
}
fn section_ui(ui: &mut egui::Ui, state: &mut CadState) {
    let Some(view) = &state.view else {
        ui.weak("Preview a solid first");
        return;
    };
    ui.horizontal(|ui| {
        for (axis, name) in ["X", "Y", "Z"].iter().enumerate() {
            ui.selectable_value(&mut state.section_axis, axis, *name);
        }
        number(ui, "Plane offset mm", &mut state.section_offset);
    });
    ui.weak("Section of the evaluated solid • dimensions in millimeters");
    let lines: Vec<_> = view
        .evaluated
        .components
        .iter()
        .filter(|c| c.settings.visible && state.isolated.is_none_or(|id| id == c.id))
        .flat_map(|c| cad::measure::section(&c.mesh, state.section_axis, state.section_offset))
        .collect();
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);
    if lines.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "The section plane misses the solid",
            egui::FontId::proportional(14.0),
            theme::TEXT_DIM,
        );
        return;
    }
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for p in lines.iter().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let scale = ((rect.width() - 70.0) as f64 / (hi[0] - lo[0]).max(1.0))
        .min((rect.height() - 70.0) as f64 / (hi[1] - lo[1]).max(1.0))
        .max(0.1);
    let map = |p: [f64; 2]| {
        rect.center()
            + vec2(
                ((p[0] - (hi[0] + lo[0]) / 2.0) * scale) as f32,
                (-(p[1] - (hi[1] + lo[1]) / 2.0) * scale) as f32,
            )
    };
    for line in lines {
        painter.line_segment(line.map(map), Stroke::new(1.5, theme::ACCENT));
    }
    painter.text(
        rect.left_top() + vec2(12.0, 12.0),
        egui::Align2::LEFT_TOP,
        format!(
            "Section extents {:.3} × {:.3} mm",
            hi[0] - lo[0],
            hi[1] - lo[1]
        ),
        egui::FontId::proportional(14.0),
        theme::TEXT,
    );
}
fn number(ui: &mut egui::Ui, label: &str, v: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(v).speed(0.05).max_decimals(4));
    });
}
fn vector(ui: &mut egui::Ui, label: &str, v: &mut [f64; 3]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        for x in v {
            ui.add(egui::DragValue::new(x).speed(0.1).max_decimals(3));
        }
    });
}
fn source(ui: &mut egui::Ui, label: &str, value: &mut u64, tree: &[(NodeId, String)]) {
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!("{label} #{}", value))
        .show_ui(ui, |ui| {
            for (id, name) in tree {
                ui.selectable_value(value, id.0, format!("#{} {name}", id.0));
            }
        });
}
fn indices(ui: &mut egui::Ui, label: &str, list: &mut Vec<usize>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        for n in list.iter_mut() {
            ui.add(egui::DragValue::new(n).range(0..=100000));
        }
        if ui.small_button("+").clicked() {
            list.push(0);
        }
        if ui.small_button("−").clicked() {
            list.pop();
        }
    });
}
fn operation_ui(ui: &mut egui::Ui, op: &mut Operation, tree: &[(NodeId, String)]) {
    ui.strong(op.label());
    match op {
        Operation::Band => {
            ui.weak("Rebuilds the procedural ring at its current size and profile; source band controls remain in the graph.");
        }
        Operation::Box { size } => vector(ui, "Dimensions mm", size),
        Operation::Cylinder {
            radius_mm,
            height_mm,
        } => {
            number(ui, "Radius mm", radius_mm);
            number(ui, "Height mm", height_mm);
        }
        Operation::Sphere { radius_mm } => number(ui, "Radius mm", radius_mm),
        Operation::TwistedRing {
            major_mm,
            radial_mm,
            axial_mm,
            turns,
        } => {
            number(ui, "Major radius mm", major_mm);
            number(ui, "Radial diameter mm", radial_mm);
            number(ui, "Axial diameter mm", axial_mm);
            number(ui, "Whole / half turns", turns);
        }
        Operation::Torus { major_mm, minor_mm } => {
            number(ui, "Major radius mm", major_mm);
            number(ui, "Tube radius mm", minor_mm);
        }
        Operation::Extrude {
            height_mm,
            draft_deg,
            ..
        } => {
            number(ui, "Height mm", height_mm);
            number(ui, "Taper degrees", draft_deg);
        }
        Operation::Revolve {
            pivot,
            axis,
            degrees,
            ..
        } => {
            vector(ui, "Axis origin mm", pivot);
            vector(ui, "Axis direction", axis);
            number(ui, "Revolution degrees", degrees);
        }
        Operation::Sweep { path, .. } => {
            for (i, p) in path.iter_mut().enumerate() {
                vector(ui, &format!("Station {i} mm"), p);
            }
            if ui.small_button("Add path station").clicked() && path.len() < 128 {
                let p = path.last().copied().unwrap_or([0.0; 3]);
                path.push([p[0], p[1], p[2] + 5.0]);
            }
        }
        Operation::Twist {
            degrees, end_scale, ..
        } => {
            number(ui, "Twist degrees", degrees);
            number(ui, "End scale", end_scale);
            ui.weak("Edit the planar path in Debug; the section must contain straight segments.");
        }
        Operation::Loft { sections } => {
            for (i, s) in sections.iter_mut().enumerate() {
                vector(ui, &format!("Section {i} origin mm"), &mut s.plane.origin);
            }
            ui.weak(
                "Sections must have matching curve counts and winding. Debug exposes each profile.",
            );
        }
        Operation::Boolean { a, b, kind } => {
            source(ui, "First", a, tree);
            source(ui, "Second", b, tree);
            ui.horizontal(|ui| {
                for k in [Boolean::Union, Boolean::Subtract, Boolean::Intersect] {
                    ui.selectable_value(kind, k, format!("{k:?}"));
                }
            });
        }
        Operation::Fillet {
            source: id,
            edges,
            radius_mm,
        } => {
            source(ui, "Source", id, tree);
            indices(ui, "Edge indices (zero based)", edges);
            number(ui, "Radius mm", radius_mm);
            ui.weak("Analytic and prismatic edges are supported; unsupported intersections return a preview error.");
        }
        Operation::Chamfer {
            source: id,
            edges,
            base_face,
            distance_mm,
        } => {
            source(ui, "Source", id, tree);
            indices(ui, "Edge indices", edges);
            ui.add(egui::DragValue::new(base_face).prefix("Base face "));
            number(ui, "Equal distances mm", distance_mm);
        }
        Operation::Shell {
            source: id,
            open_faces,
            thickness_mm,
        } => {
            source(ui, "Source", id, tree);
            indices(ui, "Opening face indices", open_faces);
            number(ui, "Wall mm", thickness_mm);
            ui.weak(
                "Exact boxes, cylinders, and spheres; an empty opening list makes a closed cavity.",
            );
        }
        Operation::Transform {
            source: id,
            translation,
            rotation_deg,
        } => {
            source(ui, "Source", id, tree);
            vector(ui, "Translation mm", translation);
            vector(ui, "Rotation XYZ degrees", rotation_deg);
        }
    }
}
fn component_ui(ui: &mut egui::Ui, f: &mut Feature) {
    let c = &mut f.component;
    egui::ComboBox::from_id_salt("component_role")
        .selected_text(format!("{:?}", c.role))
        .show_ui(ui, |ui| {
            for role in [
                cad::ComponentRole::Shank,
                cad::ComponentRole::Head,
                cad::ComponentRole::Setting,
                cad::ComponentRole::Inlay,
                cad::ComponentRole::Stone,
                cad::ComponentRole::Other,
            ] {
                ui.selectable_value(&mut c.role, role, format!("{role:?}"));
            }
        });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut c.visible, "Visible");
        ui.checkbox(
            &mut c.reference,
            "Reference stone (exclude from metal export)",
        );
    });
    egui::ComboBox::from_id_salt("component_material")
        .selected_text(&c.material)
        .show_ui(ui, |ui| {
            for m in ringdesign_core::metal::METALS {
                ui.selectable_value(&mut c.material, m.name.to_string(), m.name);
            }
        });
    let mut anchored = c.ring_anchor_deg.is_some();
    if ui
        .checkbox(&mut anchored, "Anchor to ring circumference")
        .changed()
    {
        c.ring_anchor_deg = anchored.then_some(90.0);
    }
    if let Some(theta) = &mut c.ring_anchor_deg {
        number(ui, "Ring angle degrees", theta);
        number(ui, "Extra radial height mm", &mut c.anchor_height_mm);
    }
    if c.reference {
        let id = c.stone_id.get_or_insert_with(|| format!("stone-{}", f.id));
        ui.horizontal(|ui| {
            ui.label("Stone identity");
            ui.text_edit_singleline(id);
        });
    }
    let mut own = c.manufacturing.is_some();
    if ui
        .checkbox(&mut own, "Component manufacturing recipe")
        .changed()
    {
        c.manufacturing = own.then(Default::default);
    }
    if let Some(s) = &mut c.manufacturing {
        ui.horizontal_wrapped(|ui| {
            for sand in ringdesign_core::castability::SandProcess::ALL {
                if ui.button(sand.label()).clicked() {
                    s.recipe = ringdesign_core::manufacturing::Recipe::sand(*sand);
                }
            }
            ui.checkbox(&mut s.auto_parting, "Automatic parting");
        });
        number(ui, "Measured shrink %", &mut s.recipe.shrink_pct);
    }
    ui.label("Assembly / bench instructions");
    ui.text_edit_multiline(&mut c.bench_notes);
}
fn starters(source: u64) -> Vec<Operation> {
    let mut section = Sketch::rectangle(2.0, 5.0);
    section.plane = ringdesign_core::sketch::Workplane::section();
    for p in &mut section.points {
        p.xy[0] += 10.0;
    }
    let mut top = Sketch::rectangle(8.0, 6.0);
    top.plane.origin[2] = 5.0;
    vec![
        Operation::Band,
        Operation::TwistedRing {
            major_mm: 10.0,
            radial_mm: 2.0,
            axial_mm: 4.0,
            turns: 1.0,
        },
        Operation::Box {
            size: [8.0, 6.0, 3.0],
        },
        Operation::Cylinder {
            radius_mm: 4.0,
            height_mm: 3.0,
        },
        Operation::Sphere { radius_mm: 3.0 },
        Operation::Torus {
            major_mm: 10.0,
            minor_mm: 1.5,
        },
        Operation::Extrude {
            sketch: Sketch::rectangle(8.0, 6.0),
            height_mm: 3.0,
            draft_deg: 0.0,
        },
        Operation::Revolve {
            sketch: section,
            pivot: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            degrees: 360.0,
        },
        Operation::Sweep {
            sketch: Sketch::circle(1.0),
            path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 5.0], [2.0, 0.0, 8.0]],
        },
        Operation::Loft {
            sections: vec![Sketch::rectangle(10.0, 8.0), top],
        },
        Operation::Boolean {
            a: source,
            b: 0,
            kind: Boolean::Union,
        },
        Operation::Fillet {
            source,
            edges: vec![0],
            radius_mm: 0.5,
        },
        Operation::Chamfer {
            source,
            edges: vec![0],
            base_face: 4,
            distance_mm: 0.3,
        },
        Operation::Shell {
            source,
            open_faces: vec![],
            thickness_mm: 0.8,
        },
        Operation::Transform {
            source,
            translation: [0.0, 0.0, 5.0],
            rotation_deg: [0.0; 3],
        },
    ]
}

fn sketch_controls(ui: &mut egui::Ui, s: &mut Sketch, state: &mut CadState) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("Import SVG / DXF…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Vector profiles", &["svg", "dxf"])
                .pick_file()
            {
                let result = (|| -> anyhow::Result<Sketch> {
                    let text = std::fs::read_to_string(&path)?;
                    if path.extension().is_some_and(|e| e == "svg") {
                        ringdesign_core::sketch::exchange::import_svg(&text)
                    } else {
                        ringdesign_core::sketch::exchange::import_dxf(&text)
                    }
                })();
                match result {
                    Ok(mut next) => {
                        next.plane = s.plane.clone();
                        *s = next;
                        state.pending.clear();
                    }
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
        }
        if ui.button("Export profile…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG", &["svg"])
                .add_filter("DXF", &["dxf"])
                .set_file_name("profile.svg")
                .save_file()
            {
                let result = (|| -> anyhow::Result<()> {
                    let text = if path.extension().is_some_and(|e| e == "dxf") {
                        ringdesign_core::sketch::exchange::dxf(s)?
                    } else {
                        ringdesign_core::sketch::exchange::svg(s)?
                    };
                    ringdesign_core::library::write_atomic(&path, text.as_bytes())?;
                    Ok(())
                })();
                match result {
                    Ok(()) => state.message = "Profile exported in millimeters".into(),
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Sketch");
        ui.text_edit_singleline(&mut s.name);
        if ui.button("Rectangle").clicked() {
            let plane = s.plane.clone();
            *s = Sketch::rectangle(8.0, 6.0);
            s.plane = plane;
            state.pending.clear();
        }
        if ui.button("Circle").clicked() {
            let plane = s.plane.clone();
            *s = Sketch::circle(3.0);
            s.plane = plane;
            state.pending.clear();
        }
        if ui.button("Solve constraints").clicked() {
            match s.solve() {
                Ok(solution) => {
                    state.message = format!(
                        "Residual {:.6} mm • {} free dimensions",
                        solution.residual_mm, solution.remaining_dof
                    );
                    *s = solution.sketch;
                }
                Err(e) => state.error = Some(e.to_string()),
            }
        }
    });
    egui::CollapsingHeader::new("Workplane and point dimensions").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Head plan XY").clicked() {
                s.plane = Default::default();
            }
            if ui.button("Ring section XZ").clicked() {
                s.plane = ringdesign_core::sketch::Workplane::section();
            }
        });
        vector(ui, "Origin mm", &mut s.plane.origin);
        vector(ui, "X direction", &mut s.plane.x);
        vector(ui, "Y direction", &mut s.plane.y);
        for p in &mut s.points {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.point, Some(p.id), format!("#{}", p.id));
                for v in &mut p.xy {
                    ui.add(egui::DragValue::new(v).speed(0.05).max_decimals(4));
                }
                ui.checkbox(&mut p.fixed, "Fixed");
            });
        }
        for (i, c) in s.constraints.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{i}: {c:?}"));
                if let Constraint::Distance { mm, .. } = c {
                    ui.add(
                        egui::DragValue::new(mm)
                            .range(0.0001..=1000.0)
                            .speed(0.05)
                            .suffix(" mm"),
                    );
                }
            });
        }
        for e in &mut s.entities {
            ui.horizontal(|ui| {
                ui.label(format!("Entity #{} {:?}", e.id, e.geometry));
                ui.checkbox(&mut e.construction, "Construction");
            });
        }
        if ui.small_button("Remove last constraint").clicked() {
            s.constraints.pop();
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (i, label) in ["Select", "Line", "Polyline", "Circle", "Arc", "Cubic"]
            .iter()
            .enumerate()
        {
            if ui.selectable_value(&mut state.tool, i, *label).changed() {
                state.pending.clear();
            }
        }
        ui.checkbox(&mut state.construction, "Construction");
        ui.add(
            egui::DragValue::new(&mut s.grid_mm)
                .range(0.01..=10.0)
                .prefix("Grid ")
                .suffix(" mm"),
        );
    });
    ui.horizontal_wrapped(|ui|{
        egui::ComboBox::from_id_salt("constraint_kind").selected_text(["Distance","Horizontal","Vertical","Coincident","Symmetry","Tangent"][state.constraint_kind]).show_ui(ui,|ui|{for (i,label) in ["Distance","Horizontal","Vertical","Coincident","Symmetry","Tangent"].iter().enumerate() {ui.selectable_value(&mut state.constraint_kind,i,*label);}});
        ui.add(egui::DragValue::new(&mut state.dimension).speed(0.1).suffix(" mm"));
        if ui.button("Constrain selected points").clicked() {
            let p=&state.pending;let needed=match state.constraint_kind {4=>3,5=>4,_=>2};
            if p.len()>=needed {s.constraints.push(match state.constraint_kind {0=>Constraint::Distance {a:p[0],b:p[1],mm:state.dimension},1=>Constraint::Horizontal(p[0],p[1]),2=>Constraint::Vertical(p[0],p[1]),3=>Constraint::Coincident(p[0],p[1]),4=>Constraint::Symmetry {a:p[0],b:p[1],center:p[2]},_=>Constraint::Tangent {a:p[0],b:p[1],center:p[2],at:p[3]}});state.pending.clear();}
            else {state.error=Some(format!("Shift-click {needed} points in order; tangent uses line endpoints, circle center, and tangent point"));}
        }
    });
}
fn sketch_canvas(ui: &mut egui::Ui, s: &mut Sketch, state: &mut CadState) {
    ui.weak("Millimeters • click to draw • Shift-click points for constraints • drag points to edit • wheel to zoom • Enter closes a polyline");
    let (rect, response) = ui.allocate_exact_size(
        ui.available_size().max(vec2(200.0, 180.0)),
        egui::Sense::click_and_drag(),
    );
    if response.hovered() {
        state.sketch_scale = (state.sketch_scale
            * (1.0 + ui.input(|i| i.smooth_scroll_delta.y) * 0.002))
            .clamp(2.0, 300.0);
    }
    let scale = state.sketch_scale;
    let map = |p: [f64; 2]| rect.center() + vec2(p[0] as f32, -p[1] as f32) * scale;
    let inverse = |p: egui::Pos2| {
        [
            (p.x - rect.center().x) as f64 / scale as f64,
            -(p.y - rect.center().y) as f64 / scale as f64,
        ]
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);
    let grid = (s.grid_mm as f32 * scale).max(8.0);
    for i in -100..=100 {
        let x = rect.center().x + i as f32 * grid;
        let y = rect.center().y + i as f32 * grid;
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                Stroke::new(0.5, theme::GRID),
            );
        }
        if y >= rect.top() && y <= rect.bottom() {
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                Stroke::new(0.5, theme::GRID),
            );
        }
    }
    for e in &s.entities {
        for curve in s.curves_of(e).unwrap_or_default() {
            let points = curve.tessellate_within(0.02);
            painter.add(egui::Shape::line(
                points.into_iter().map(map).collect(),
                Stroke::new(
                    if e.construction { 1.0 } else { 2.0 },
                    if e.construction {
                        theme::TEXT_DIM
                    } else {
                        theme::ACCENT
                    },
                ),
            ));
        }
    }
    for p in &s.points {
        painter.circle_filled(
            map(p.xy),
            4.0,
            if state.pending.contains(&p.id) || state.point == Some(p.id) {
                theme::WARN
            } else {
                theme::TEXT
            },
        );
        painter.text(
            map(p.xy) + vec2(5.0, -5.0),
            egui::Align2::LEFT_BOTTOM,
            p.id.to_string(),
            egui::FontId::proportional(10.0),
            theme::TEXT_DIM,
        );
    }
    if let Some(pos) = response.interact_pointer_pos() {
        let nearest = s
            .points
            .iter()
            .filter(|p| map(p.xy).distance(pos) < 10.0)
            .min_by(|a, b| {
                map(a.xy)
                    .distance_sq(pos)
                    .total_cmp(&map(b.xy).distance_sq(pos))
            })
            .map(|p| p.id);
        let raw = inverse(pos);
        let anchor = state.pending.last().and_then(|id| s.at(*id).ok());
        let snapped = s.snap(raw, anchor, 8.0 / scale as f64);
        let xy = snapped.as_ref().map_or(raw, |s| s.xy);
        if let Some(snap) = &snapped {
            painter.circle_stroke(map(snap.xy), 7.0, Stroke::new(1.0, theme::INFO));
            painter.text(
                map(snap.xy) + vec2(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                snap.kind,
                egui::FontId::proportional(12.0),
                theme::INFO,
            );
        }
        if response.drag_started() && state.tool == 0 {
            state.point = nearest;
        }
        if response.dragged() && state.tool == 0 {
            if let Some(id) = state.point {
                if let Some(p) = s.points.iter_mut().find(|p| p.id == id) {
                    p.xy = xy;
                }
            }
        }
        if response.clicked() {
            if ui.input(|i| i.modifiers.shift) {
                if let Some(id) = nearest {
                    if !state.pending.contains(&id) {
                        state.pending.push(id);
                    }
                }
            } else if state.tool == 0 {
                state.point = nearest;
            } else {
                let xy = if state.tool == 4 && state.pending.len() == 2 {
                    let center = s.at(state.pending[0]).unwrap();
                    let start = s.at(state.pending[1]).unwrap();
                    let r = (start[0] - center[0]).hypot(start[1] - center[1]);
                    let theta = (xy[1] - center[1]).atan2(xy[0] - center[0]);
                    [center[0] + r * theta.cos(), center[1] + r * theta.sin()]
                } else {
                    xy
                };
                let id = if state.tool == 4 && state.pending.len() == 2 {
                    s.point(xy)
                } else {
                    nearest.unwrap_or_else(|| s.point(xy))
                };
                state.pending.push(id);
                let p = &state.pending;
                let entity = match state.tool {
                    1 if p.len() == 2 => Some(Geometry::Line { a: p[0], b: p[1] }),
                    3 if p.len() == 2 => Some(Geometry::Circle {
                        center: p[0],
                        rim: p[1],
                    }),
                    4 if p.len() == 3 => Some(Geometry::Arc {
                        center: p[0],
                        start: p[1],
                        end: p[2],
                    }),
                    5 if p.len() == 4 => Some(Geometry::Bezier {
                        points: [p[0], p[1], p[2], p[3]],
                    }),
                    _ => None,
                };
                if let Some(geometry) = entity {
                    let id = s.entity(geometry);
                    s.entities
                        .iter_mut()
                        .find(|e| e.id == id)
                        .unwrap()
                        .construction = state.construction;
                    state.pending.clear();
                }
            }
        }
    }
    if state.tool == 2 && state.pending.len() >= 3 && ui.input(|i| i.key_pressed(egui::Key::Enter))
    {
        let points = std::mem::take(&mut state.pending);
        let id = s.entity(Geometry::Polyline {
            points,
            closed: true,
        });
        s.entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .construction = state.construction;
    }
}
