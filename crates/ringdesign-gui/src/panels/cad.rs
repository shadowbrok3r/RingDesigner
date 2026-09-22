//! A conventional feature tree edits the existing graph. A candidate has its
//! own evaluated solids and renderer; Enter applies one edit, Escape cancels.
use crate::{
    app::RingDesignerApp,
    camera::OrbitCamera,
    pane::PaneKind,
    theme,
    viewport::GpuMeshRenderer,
};
use egui::{Stroke, vec2};
use ringdesign_workbench::{cad_tools, icons::{self, Icon}};
use ringdesign_core::{
    BuildParams, RingDesign,
    cad::{self, Boolean, EdgeRef, Evaluated, FaceRef, Feature, Operation, Placement, Profile},
    sketch::{Constraint, Geometry, Sketch},
};
use ringdesign_graph::{
    graph::{Graph, NodeId},
    nodes::cad as graph_cad,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
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
    cancel: Arc<AtomicBool>,
}
/// One running evaluation plus the abandoned ones still inside a kernel call.
const MAX_WORKERS: usize = 3;
struct Live(Arc<AtomicUsize>);
impl Drop for Live {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
/// What another pane or the palette asks of the CAD pane.
pub enum CadRequest {
    /// A new feature, optionally anchored at a ring angle and radial height.
    Add { operation: Operation, anchor: Option<(f64, f64)> },
    Preview,
    Apply,
    Discard,
}
/// What the pointer was over when a context menu opened.
#[derive(Clone, Copy)]
struct MenuHit {
    part: u64,
    point: Option<[f32; 3]>,
    edge: Option<usize>,
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
    display: crate::viewport::CandidateDisplay,
    changed_at: f64,
    observed: u64,
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
    escape_requested: bool,
    delete_requested: bool,
    sketch_scale: f32,
    sketch_pan: egui::Vec2,
    entity: Option<u64>,
    workers: Arc<AtomicUsize>,
    discarded: Option<Graph>,
    requests: Vec<CadRequest>,
    menu_hit: Option<MenuHit>,
    pub ring_menu_hit: Option<[f32; 3]>,
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
            display: Default::default(),
            changed_at: 0.0,
            observed: 0,
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
            escape_requested: false,
            delete_requested: false,
            sketch_scale: 25.0,
            sketch_pan: egui::Vec2::ZERO,
            entity: None,
            workers: Default::default(),
            discarded: None,
            requests: Vec::new(),
            menu_hit: None,
            ring_menu_hit: None,
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
    /// Escape from the global shortcut router: backs out of pending sketch picks, then a history
    /// rollback. It never discards the candidate.
    pub fn cancel_shortcut(&mut self) -> bool {
        let editing = self.rollback.is_some() || !self.pending.is_empty();
        self.escape_requested |= editing;
        editing
    }
    /// Delete from the global shortcut router: the selected sketch point or entity.
    pub fn delete_shortcut(&mut self) -> bool {
        let editing = self.tab == 1 && (self.point.is_some() || self.entity.is_some());
        self.delete_requested |= editing;
        editing
    }
    pub fn request(&mut self, request: CadRequest) {
        self.requests.push(request);
    }
    pub fn open_section(&mut self) {
        self.tab = 6;
    }
    /// The feature the tree has chosen.
    pub fn selected_feature(&self) -> Option<u64> {
        self.selected.map(|id| id.0)
    }
    /// The last evaluation or apply failure.
    pub fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
fn hash<T: serde::Serialize>(v: &T) -> u64 {
    ringdesign_core::manufacturing::package::fingerprint(&serde_json::to_vec(v).unwrap_or_default())
}
/// Commit the previewed document onto a design with no graph; the lifted graph is what the pane edits next.
fn apply_plain(app: &mut RingDesignerApp, state: &CadState) -> Result<Graph, String> {
    let view = state.view.as_ref().ok_or("Preview the candidate first")?;
    let mut applied = app.design.clone();
    applied.cad = view.design.cad.clone();
    // The preview evaluates the whole history; a persisted rollback survives only while it still names a feature.
    if let (Some(doc), Some(through)) = (applied.cad.as_mut(), app.design.cad.as_ref().and_then(|d| d.through)) {
        doc.through = doc.features.iter().any(|f| f.id == through).then_some(through);
    }
    let lifted = graph_cad::from_document(&applied).map_err(|e| e.to_string())?;
    app.design = applied;
    app.mark_dirty();
    Ok(lifted)
}
fn source_key(app: &RingDesignerApp) -> u64 {
    if app.design.graph.is_some() {
        hash(&(&app.design.graph, Arc::as_ptr(&app.lib) as usize))
    } else {
        hash(&(&app.design, Arc::as_ptr(&app.lib) as usize))
    }
}
/// Queue a request and bring the CAD pane up to serve it.
pub fn ask(app: &mut RingDesignerApp, request: CadRequest) {
    app.cad.request(request);
    app.focus(PaneKind::Cad);
}
/// Add the starter feature called `label`, seated at a ring angle and radial height when given.
pub fn add_starter(app: &mut RingDesignerApp, label: &str, anchor: Option<(f64, f64)>) {
    if let Some(operation) = cad_tools::starters(0, 0).into_iter().find(|op| op.label() == label) {
        ask(app, CadRequest::Add { operation, anchor });
    }
}
/// Starters that make sense seated on the ring's surface.
pub const PLACEABLE: [&str; 6] = ["Box", "Cylinder", "Sphere", "Extrude", "Sweep", "Loft"];
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
    if let Some(job) = state.job.take() {
        job.cancel.store(true, Ordering::Relaxed);
    }
    if state.workers.load(Ordering::Relaxed) >= MAX_WORKERS {
        state.message = "Waiting for an earlier evaluation to stop".into();
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        return;
    }
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
    let cancel = Arc::new(AtomicBool::new(false));
    state.job = Some(Job { key, receiver, cancel: cancel.clone() });
    state.requested = key;
    state.error = None;
    state.workers.fetch_add(1, Ordering::Relaxed);
    let live = Live(state.workers.clone());
    std::thread::spawn(move || {
        let _live = live;
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
                let evaluated = cad::evaluate_with(&d, &lib, params, &cad::BuildCtx { cancel: &cancel })?;
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
/// A reference to `edge` of the evaluated part `part` that remembers what it points at.
fn signed_edge(state: &CadState, part: u64, edge: usize) -> EdgeRef {
    state
        .view
        .as_ref()
        .and_then(|v| {
            let c = v.evaluated.components.iter().find(|c| c.id == part)?;
            let frame = c.settings.placement.frame(&v.design).ok()?;
            Some(EdgeRef::signed(&c.body, edge, &frame))
        })
        .unwrap_or_else(|| EdgeRef::bare(edge))
}
/// Append one feature to the candidate and select it; an anchor seats it on the ring.
fn add_feature(state: &mut CadState, g: &mut Graph, operation: Operation, anchor: Option<(f64, f64)>) {
    match graph_cad::append(g, operation) {
        Ok(id) => {
            if let (Some((theta, height)), Some(node)) = (anchor, g.node_mut(id)) {
                if let Ok(mut f) = serde_json::from_value::<Feature>(node.params.clone()) {
                    f.component.placement = Placement::ring(theta, height);
                    node.params = serde_json::to_value(f).unwrap();
                }
            }
            state.selected = Some(id);
            state.json_node = None;
            state.tab = 0;
            state.rollback = None;
        }
        Err(e) => state.error = Some(e.to_string()),
    }
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
        state.view = None;
        state.selected = None;
        state.edge = None;
        state.isolated = None;
        if let Some(job) = state.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        state.rollback = None;
        state.joints = None;
        state.discarded = None;
        state.entity = None;
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
    let (mut preview_requested, mut apply_requested, mut discard_requested) = (false, false, false);
    for request in std::mem::take(&mut state.requests) {
        match request {
            CadRequest::Add { operation, anchor } => {
                // A part seated on the ring needs the ring beside it.
                if anchor.is_some() && !g.nodes.iter().any(|n| n.kind == "cad.feature") {
                    add_feature(&mut state, &mut g, Operation::Band, None);
                }
                add_feature(&mut state, &mut g, operation, anchor);
            }
            CadRequest::Preview => preview_requested = true,
            CadRequest::Apply => apply_requested = true,
            CadRequest::Discard => discard_requested = true,
        }
    }
    let before = hash(&(&g, state.rollback));
    ui.horizontal_wrapped(|ui| {
        for (modify, title, icon) in [(false, "Create", Icon::Add), (true, "Modify", Icon::CadFillet)] {
            ui.menu_button((icon.image(ui, 18.), title), |ui| {
                ui.set_min_width(210.);
                let ids: Vec<_> = g.nodes.iter().filter(|n| n.kind == "cad.feature")
                    .map(|n| n.id.0).collect();
                let previous = state.edge.map(|(id, _)| id)
                    .or(state.selected.map(|id| id.0).filter(|id| ids.contains(id)))
                    .or(ids.last().copied()).unwrap_or(0);
                let second = ids.iter().rev().copied().find(|id| *id != previous).unwrap_or(0);
                for mut op in cad_tools::starters(previous, second).into_iter().filter(|op| cad_tools::modify(op) == modify) {
                    let valid = cad_tools::unavailable(&op).is_none() && (!modify || (ids.contains(&previous)
                        && (!matches!(op, Operation::Boolean {..}) || second != 0)));
                    let hint = if let Some(reason) = cad_tools::unavailable(&op) {reason} else if valid { cad_tools::hint(&op) } else if matches!(op, Operation::Boolean {..}) {
                        "Create two different solids before using a boolean."
                    } else { "Create or select a solid first." };
                    if ui.add_enabled(valid, egui::Button::new((cad_tools::icon(&op).image(ui, 20.), op.label())))
                        .on_disabled_hover_text(hint).on_hover_text(hint).clicked() {
                        if let (Some((part, edge)), Operation::Fillet {edges, ..} | Operation::Chamfer {edges, ..}) = (state.edge, &mut op) { *edges = vec![signed_edge(&state, part, edge)]; }
                        add_feature(&mut state, &mut g, op, None);
                        ui.close();
                    }
                }
                if !modify {
                    ui.separator();
                    ui.menu_button((Icon::Files.image(ui, 18.), "Example projects"), |ui| {
                        for name in cad::examples::NAMES {
                            if cad_tools::example_button(ui, name).clicked() {
                                match cad::examples::design(name).and_then(|d| graph_cad::from_document(&d).map_err(Into::into)) {
                                    Ok(next) => { g = next; state.selected = g.nodes.iter().find(|n| n.kind == "cad.feature").map(|n| n.id);
                                        state.tab = 0; state.rollback = None; state.joints = None; state.error = None;
                                        state.view = None; state.edge = None; state.isolated = None; }
                                    Err(e) => state.error = Some(e.to_string()),
                                }
                                ui.close();
                            }
                        }
                    });
                }
            });
        }
        ui.menu_button((Icon::CadSketch.image(ui, 18.), "Sketch"), |ui| {
            let editable = state.selected.and_then(|id| g.node(id))
                .and_then(|n| serde_json::from_value::<Feature>(n.params.clone()).ok())
                .is_some_and(|mut f| f.operation.sketch_mut().is_some());
            if ui.add_enabled(editable, egui::Button::new("Edit selected sketch"))
                .on_disabled_hover_text("Select a sketch, extrusion, revolve, sweep, twist or loft feature first.").clicked() {
                state.tab = 1; ui.close();
            }
            if ui.button("Return to solid view").clicked() { state.tab = 0; ui.close(); }
        });
        ui.menu_button((Icon::Panel.image(ui, 18.), "Inspect"), |ui| {
            for (tab, label) in [(0,"Feature properties"),(2,"Components & assembly"),(6,"Section"),(4,"Size study"),(5,"Manufacturing stages"),(3,"Advanced source")] {
                if ui.selectable_value(&mut state.tab, tab, label).clicked() { ui.close(); }
            }
            ui.separator();
            if ui.button("Return to Ring viewport").clicked() { app.focus(PaneKind::Solid); ui.close(); }
        });
        ui.separator();
        if ui.add_enabled(state.job.is_none(), egui::Button::new((Icon::Rebuild.image(ui, 18.), "Preview")))
            .on_hover_text("Evaluate the candidate (Enter)")
            .on_disabled_hover_text("The current solid is still evaluating.").clicked() || (preview_requested && state.job.is_none()) {
            launch(&mut state, g.clone(), app, ui.ctx().clone());
        }
        let ready = state.rollback.is_none()
            && state.view_key == before
            && state.draft.is_some()
            && state.job.is_none()
            && state.error.is_none();
        // Enter only previews, and never from the sketch tab where it belongs to the drawing tools.
        let typing = ui.ctx().egui_wants_keyboard_input();
        let enter = !typing && state.tab != 1 && state.pending.is_empty()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.any());
        let apply_key = !typing && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command_only());
        if ui
            .add_enabled(ready, egui::Button::new((Icon::Check.image(ui, 18.), "Apply")))
            .on_hover_text("Commit the previewed candidate (Ctrl+Enter)")
            .on_disabled_hover_text("Apply becomes available after a changed candidate previews successfully at the end of history.")
            .clicked()
            || ((apply_key || apply_requested) && ready)
        {
            app.history.commit(&app.design);
            let committed = if app.design.graph.is_some() {
                if let Some(view) = &state.view {
                    let mut applied = view.design.clone();
                    applied.manufacturing = app.design.manufacturing.clone();
                    applied.casting_trials = app.design.casting_trials.clone();
                    app.design = applied;
                }
                app.set_graph(g.clone());
                state.message = "Feature edit applied; undo restores its source graph".into();
                Some(g.clone())
            } else {
                match apply_plain(app, &state) {
                    Ok(lifted) => {
                        state.message = "Feature edit applied; undo restores the design".into();
                        Some(lifted)
                    }
                    Err(e) => {
                        state.error = Some(e);
                        None
                    }
                }
            };
            if let Some(lifted) = committed {
                // The evaluated candidate is already available. Record the whole
                // edit now so immediate Undo does not depend on the rebuild timer.
                app.history.commit(&app.design);
                // The preview already shows the committed document.
                let key = hash(&(&lifted, state.rollback));
                state.view_key = key;
                state.requested = key;
                original = lifted;
                g = original.clone();
                state.draft = None;
                state.discarded = None;
                state.source = source_key(app);
            }
        } else if enter && state.draft.is_some() && state.job.is_none() {
            launch(&mut state, g.clone(), app, ui.ctx().clone());
        }
        if std::mem::take(&mut state.escape_requested) {
            if !state.pending.is_empty() {
                state.pending.clear();
            } else if state.rollback.take().is_some() {
                launch(&mut state, g.clone(), app, ui.ctx().clone());
            }
        }
        if ui.add_enabled(state.draft.is_some() || state.rollback.is_some() || state.job.is_some(), egui::Button::new((Icon::Close.image(ui, 18.), "Cancel")))
            .on_hover_text("Stop the evaluation and set the candidate aside; it can be restored until the next edit").clicked() || discard_requested {
            if state.draft.is_some() {
                state.discarded = Some(g.clone());
            }
            if let Some(job) = state.job.take() {
                job.cancel.store(true, Ordering::Relaxed);
            }
            g = original.clone();
            state.draft = None;
            state.pending.clear();
            state.error = None;
            state.requested = 0;
            state.rollback = None;
            state.message = "Candidate set aside".into();
        }
        if state.draft.is_none() && state.discarded.is_some()
            && ui.button((Icon::History.image(ui, 18.), "Restore discarded candidate")).clicked() {
            g = state.discarded.take().unwrap();
            state.message.clear();
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
    if state.selected.is_none() { state.selected = tree.first().map(|(id, _)| *id); }
    egui::Panel::left(ui.id().with("cad-inspector"))
        .resizable(true).default_size(280.).size_range(230.0..=460.)
        .frame(egui::Frame::new().inner_margin(8).fill(theme::PANEL))
        .show(ui, |ui| {
        ui.strong(match state.tab { 1 => "CAD · Sketch", 2 => "CAD · Components", 3 => "CAD · Source", 4 => "CAD · Sizes", 5 => "CAD · Stages", 6 => "CAD · Section", _ => "CAD · Features" });
        ui.weak("1 Create   2 Edit & preview   3 Apply");
        egui::ScrollArea::vertical().id_salt("cad-inspector-scroll").auto_shrink([false,false]).show(ui, |ui| {
        egui::CollapsingHeader::new("Feature history").default_open(true).show(ui, |ui| {
            if tree.is_empty() { ui.label("The current ring is the source. Add a Procedural shank to modify it as a CAD solid, or start from an Example project."); }
            for (id, label) in &tree {
                let icon = g.node(*id).and_then(|n| serde_json::from_value::<Feature>(n.params.clone()).ok())
                    .map_or(Icon::Graph, |f| cad_tools::icon(&f.operation));
                if ui.add_sized([ui.available_width(), ui.spacing().interact_size.y],
                    egui::Button::new((icon.image(ui, 18.), label.as_str())).selected(state.selected == Some(*id)).right_text(egui::Atom::grow()))
                    .on_hover_text(format!("Feature #{}", id.0)).clicked() {
                    state.selected = Some(*id); app.selected_node = Some(*id);
                    state.pending.clear(); state.json_node = None;
                }
            }
        });
        ui.separator();
    if state.tab <= 3 {
        if let Some(id) = state.selected {
            let bound_operation = g.wire_into(id, "operation").is_some()
                || g.node(id)
                    .is_some_and(|n| n.inputs.contains_key("operation"));
            if let Some(n) = g.node_mut(id) {
                if n.kind == "cad.feature" {
                    if let Ok(mut f) = serde_json::from_value::<Feature>(n.params.clone()) {
                        ui.vertical(|ui|{
                        ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(f32::INFINITY));
                        ui.checkbox(&mut f.enabled,"Enabled");
                        ui.weak(cad_tools::hint(&f.operation));
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
        });
    });
    let current = hash(&(&g, state.rollback));
    if hash(&g) != hash(&original) {
        state.draft = Some(g.clone());
        state.discarded = None;
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
                            let first = state.view.is_none();
                            state.view = Some(view);
                            state.view_key = current;
                            upload(&mut state, first);
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
    let now = ui.input(|i| i.time);
    if state.observed != current { state.observed = current; state.changed_at = now; }
    // A changed candidate supersedes the evaluation in flight; `launch` stops it.
    if state.requested != current {
        if now - state.changed_at >= 0.25 && !ui.input(|i| i.pointer.any_down()) {
            launch(&mut state, g.clone(), app, ui.ctx().clone());
        } else { ui.ctx().request_repaint_after(std::time::Duration::from_millis(80)); }
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
    if state.tab == 1 {
        if let Some(id) = state.selected {
            if g.wire_into(id, "operation").is_none() {
                if let Some(n) = g.node_mut(id) {
                    if let Ok(mut f) = serde_json::from_value::<Feature>(n.params.clone()) {
                        if let Some(sketch) = f.operation.sketch_mut() { sketch_canvas(ui, sketch, &mut state); }
                        n.params = serde_json::to_value(f).unwrap();
                    }
                }
            }
        }
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
    if state.tab != 1 && state.tab != 6 {
        if state.view_key != current {
            ui.colored_label(
                theme::WARN,
                "Parameters changed — preview to evaluate this candidate",
            );
        }
        let mut redraw = false;
        egui::Panel::bottom(ui.id().with("cad-view-footer"))
            .frame(egui::Frame::new().inner_margin(6).fill(theme::PANEL)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.menu_button((Icon::View.image(ui,18.), "View"), |ui| {
                    for view in ringdesign_workbench::navigation::View::ALL {
                        if ui.button(view.label()).clicked() {
                            let angles = ringdesign_workbench::navigation::Action::View(view)
                                .apply([state.camera.yaw,state.camera.pitch,state.camera.roll],app.design.shank.head.theta_deg as f32);
                            let from = state.camera.pose();
                            state.display.turn = Some(ringdesign_workbench::focus::Turn::new(from,
                                ringdesign_workbench::focus::Pose { yaw:angles[0],pitch:angles[1],roll:angles[2],pan:[0.;2],..from }));
                            ui.close();
                        }
                    }
                    ui.checkbox(&mut state.display.navigation.locked, "Lock orbit (drag to pan)");
                });
                if icons::compact(ui,Icon::Fit,false).clicked() { redraw = true; }
                if icons::compact(ui,Icon::Wire,state.display.wire).clicked() {state.display.wire = !state.display.wire;}
                if icons::compact(ui,Icon::Grid,state.display.grid).clicked() {state.display.grid = !state.display.grid;}
                ui.menu_button((Icon::Layers.image(ui,18.), "Display"), |ui| {
                    if ui.button("Show all components").clicked() { state.isolated = None; redraw = true; }
                    if let Some(view) = &state.view {
                        for c in &view.evaluated.components {
                            if ui.selectable_label(state.isolated == Some(c.id), &c.name).clicked() {state.isolated = Some(c.id); redraw = true;}
                        }
                    }
                    redraw |= ringdesign_workbench::controls::row(ui,"Explode",|ui| ui.add(egui::DragValue::new(&mut state.explode).range(0.0..=20.0).speed(0.1).suffix(" mm"))).changed();
                });
                ui.menu_button((Icon::History.image(ui,18.), "History"), |ui| {
                    if ui.add_enabled(state.selected.is_some() && state.job.is_none(), egui::Button::new("Preview through selected feature")).clicked() {
                        state.rollback = state.selected.map(|id| id.0); launch(&mut state,g.clone(),app,ui.ctx().clone()); ui.close();
                    }
                    if ui.add_enabled(state.rollback.is_some() && state.job.is_none(),egui::Button::new("Return to end of history")).clicked() {
                        state.rollback = None; launch(&mut state,g.clone(),app,ui.ctx().clone()); ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(if state.job.is_some() { "Evaluating…" } else if state.view_key != current { "Preview pending" } else if state.draft.is_some() { "Candidate · Apply to save" } else { "Committed · mm" });
                });
            });
        });
        if redraw { upload(&mut state, true); }
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
        state.display.finish=app.finish; state.display.polish=app.polish; state.display.light=app.light; state.display.show_gems=app.show_gems;
        let (rect, response) =
            crate::viewport::candidate_view(ui, state.renderer.clone(), &mut state.camera, &mut state.display, app.design.shank.head.theta_deg as f32);
        let project = state.camera.projector(rect);
        let mut nearest = None;
        let mut hovered_edge = Vec::new();
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
                    if response.hovered() || response.clicked() {
                        if let Some(cursor) = response.hover_pos().or(response.interact_pointer_pos()) {
                            for pair in points.windows(2) {
                                let d = pair[1] - pair[0];
                                let t = ((cursor - pair[0]).dot(d) / d.length_sq().max(1e-8))
                                    .clamp(0.0, 1.0);
                                let distance = cursor.distance(pair[0] + d * t);
                                if distance < 10.0
                                    && nearest.is_none_or(|(_, _, best)| distance < best)
                                {
                                    nearest = Some((c.id, edge, distance));
                                    hovered_edge=points.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
        // The nearest visible part under a screen point, with where the ray met it.
        let pick = |state: &CadState, pos: egui::Pos2| -> Option<(u64, [f32; 3])> {
            let view = state.view.as_ref()?;
            let (origin,direction)=state.camera.ray(rect,pos);
            view.evaluated.components.iter().enumerate()
                .filter(|(_,c)|c.settings.visible && state.isolated.is_none_or(|id|id==c.id))
                .filter_map(|(index,c)| {
                    let shifted=[origin[0],origin[1],origin[2]-index as f32*state.explode as f32];
                    ringdesign_core::interaction::picking::raycast(&c.mesh,shifted,direction).map(|(_,point)|
                        (c.id,point,(0..3).map(|k|(point[k]-shifted[k])*direction[k]).sum::<f32>()))
                }).min_by(|a,b|a.2.total_cmp(&b.2)).map(|(id,point,_)|(id,point))
        };
        if let Some((id, edge, _)) = nearest {
            ui.painter_at(rect).add(egui::Shape::line(hovered_edge,Stroke::new(2.,theme::INFO)));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if response.clicked() {state.edge=Some((id,edge));state.selected=Some(NodeId(id));}
        } else if response.clicked() {
            if let Some((id,_))=response.interact_pointer_pos().and_then(|pos|pick(&state,pos)) {state.selected=Some(NodeId(id));state.edge=None;}
        }
        if response.secondary_clicked() {
            let under = response.interact_pointer_pos().and_then(|pos| pick(&state, pos));
            state.menu_hit = match (under, nearest) {
                (Some((part, point)), edge) => Some(MenuHit { part, point: Some(point), edge: edge.filter(|(id, _, _)| *id == part).map(|(_, e, _)| e) }),
                (None, Some((part, edge, _))) => Some(MenuHit { part, point: None, edge: Some(edge) }),
                (None, None) => None,
            };
        }
        let hit = state.menu_hit;
        let ring_radius = state.view.as_ref().map(|v| v.design.inner_radius_mm() + v.design.profile.thickness_mm);
        let part_name = hit.and_then(|h| state.view.as_ref()?.evaluated.components.iter().find(|c| c.id == h.part).map(|c| c.name.clone()));
        let mut refit = false;
        response.context_menu(|ui| {
            ui.set_min_width(190.);
            if let Some(hit) = hit {
                ui.weak(part_name.as_deref().unwrap_or("Component"));
                if let (Some(point), Some(radius)) = (hit.point, ring_radius) {
                    ui.menu_button((Icon::Add.image(ui, 18.), "Add here"), |ui| {
                        for op in cad_tools::starters(0, 0).into_iter().filter(|op| PLACEABLE.contains(&op.label())) {
                            if ui.button((cad_tools::icon(&op).image(ui, 18.), op.label())).clicked() {
                                let (x, y) = (point[0] as f64, point[1] as f64);
                                add_feature(&mut state, &mut g, op, Some((y.atan2(x).to_degrees(), x.hypot(y) - radius)));
                                ui.close();
                            }
                        }
                    });
                }
                if let Some(edge) = hit.edge {
                    for op in cad_tools::starters(hit.part, 0) {
                        let mut op = op;
                        if let Operation::Fillet { edges, .. } | Operation::Chamfer { edges, .. } = &mut op {
                            *edges = vec![signed_edge(&state, hit.part, edge)];
                            if ui.button((cad_tools::icon(&op).image(ui, 18.), format!("{} this edge", op.label()))).clicked() {
                                state.edge = Some((hit.part, edge));
                                add_feature(&mut state, &mut g, op, None);
                                ui.close();
                            }
                        }
                    }
                }
                if ui.button((Icon::Panel.image(ui, 18.), "Select feature")).clicked() {
                    state.selected = Some(NodeId(hit.part));
                    state.tab = 0;
                    ui.close();
                }
                if ui.button((Icon::Layers.image(ui, 18.), "Isolate")).clicked() {
                    state.isolated = Some(hit.part);
                    refit = true;
                    ui.close();
                }
            }
            if state.isolated.is_some() && ui.button((Icon::Layers.image(ui, 18.), "Show all components")).clicked() {
                state.isolated = None;
                refit = true;
                ui.close();
            }
            ui.separator();
            if ui.button((Icon::Fit.image(ui, 18.), "Fit view")).clicked() {
                refit = true;
                ui.close();
            }
            ui.checkbox(&mut state.display.wire, "Wireframe");
            ui.checkbox(&mut state.display.grid, "Grid");
        });
        if refit { upload(&mut state, true); }
        if matches!(state.tab, 0 | 2) && state.rollback.is_none() {
            direct_handles(ui, rect, &state, &mut g);
        }
    }
    });
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
    let placement = feature.component.placement.clone();
    let world = |p: [f64; 3]| placement.world(&view.design, p).unwrap_or(p).map(|v| v as f32);
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
        let grip_id = ui.id().with(("dimension_grip", id.0, label));
        let response = ui.interact(
            egui::Rect::from_center_size(b, vec2(16.0, 16.0)),
            grip_id,
            egui::Sense::drag(),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            painter.text(b + vec2(10., -12.), egui::Align2::LEFT_BOTTOM,
                format!("{label} {value:.3} {units}"), egui::FontId::proportional(12.), theme::TEXT);
        }
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
            // Only a sketch drawn here on its own plane has a plane the panel can read.
            if let Some(plane) = sketch.sketch_mut().filter(|s| s.plane.on_face.is_none()).map(|s| s.plane.clone()) {
                if let Some(normal) = plane.plane().ok().and_then(|p| p.normal()) {
                    let base = plane.origin;
                    let end = std::array::from_fn(|i| base[i] + normal[i] * *height_mm);
                    handle("Extrusion", height_mm, base, end, normal, 1.0);
                }
            }
        }
        _ => {}
    }
    if let Placement::Ring { theta_deg, height_mm, .. } = &mut feature.component.placement {
        let height = *height_mm;
        let radius = view.design.inner_radius_mm() + view.design.profile.thickness_mm + height;
        grip(
            "Ring position",
            theta_deg,
            [0.0; 3],
            [0.0, 2.0, 0.0],
            [0.0, 1.0, 0.0],
            180.0 / (std::f64::consts::PI * radius.max(0.001)),
            f64::NEG_INFINITY,
            "°",
        );
        grip(
            "Radial placement",
            height_mm,
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
    ringdesign_workbench::controls::named(ui, label, label, |ui| {
        ui.add(egui::DragValue::new(v).speed(0.05).max_decimals(4))
    });
}
fn vector(ui: &mut egui::Ui, label: &str, v: &mut [f64; 3]) {
    ui.strong(label);
    for (axis, x) in ["X", "Y", "Z"].into_iter().zip(v) {
        ringdesign_workbench::controls::named(ui, axis, &format!("{label} {axis}"), |ui| {
            ui.add(egui::DragValue::new(x).speed(0.1).max_decimals(3))
        });
    }
}

/// Where a profile comes from: drawn in this feature, or a Sketch feature earlier in the tree.
fn profile_source(ui: &mut egui::Ui, label: &str, profile: &mut Profile, tree: &[(NodeId, String)]) {
    let current = profile.feature();
    let shown = match current {
        None => "Drawn in this feature".to_string(),
        Some(id) => format!("Sketch #{id}"),
    };
    ringdesign_workbench::controls::row(ui, label, |ui| {
        egui::ComboBox::from_id_salt(("profile-source", label))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "Drawn in this feature").clicked() && current.is_some() {
                    *profile = Sketch::rectangle(8.0, 6.0).into();
                }
                for (id, name) in tree {
                    if ui.selectable_label(current == Some(id.0), format!("#{} {name}", id.0)).clicked() {
                        *profile = Profile::Feature { feature: id.0 };
                    }
                }
            });
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
/// Edge references by ordinal; a typed ordinal forgets the pick it replaced.
fn edge_refs(ui: &mut egui::Ui, label: &str, list: &mut Vec<EdgeRef>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        for (i, r) in list.iter_mut().enumerate() {
            let name = format!("{label} {}", i + 1);
            let response = ui.add(egui::DragValue::new(&mut r.ordinal).range(0..=100000));
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, &name));
            if response.changed() {
                r.signature = None;
            }
            if let Some(s) = &r.signature {
                response.on_hover_text(format!("Picked: {}", s.describe()));
            }
        }
        if ui.small_button("+").clicked() {
            list.push(EdgeRef::bare(0));
        }
        if ui.small_button("−").clicked() {
            list.pop();
        }
    });
}
fn face_ref(ui: &mut egui::Ui, name: &str, r: &mut FaceRef) {
    let response = ui.add(egui::DragValue::new(&mut r.ordinal).range(0..=100000).prefix(format!("{name} ")));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, name));
    if response.changed() {
        r.signature = None;
    }
}
fn face_refs(ui: &mut egui::Ui, label: &str, list: &mut Vec<FaceRef>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        for (i, r) in list.iter_mut().enumerate() {
            face_ref(ui, &format!("{label} {}", i + 1), r);
        }
        if ui.small_button("+").clicked() {
            list.push(FaceRef::bare(0));
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
        Operation::Sketch { .. } => {
            ui.weak("A closed profile with no body of its own; draw it in the Sketch tab and extrude, revolve, sweep or loft it from another feature.");
        }
        Operation::Extrude {
            sketch,
            height_mm,
            draft_deg,
        } => {
            profile_source(ui, "Profile", sketch, tree);
            number(ui, "Height mm", height_mm);
            number(ui, "Taper degrees", draft_deg);
        }
        Operation::Revolve {
            sketch,
            pivot,
            axis,
            degrees,
        } => {
            profile_source(ui, "Profile", sketch, tree);
            vector(ui, "Axis origin mm", pivot);
            vector(ui, "Axis direction", axis);
            number(ui, "Revolution degrees", degrees);
        }
        Operation::Sweep { sketch, path } => {
            profile_source(ui, "Section", sketch, tree);
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
            for (i, p) in sections.iter_mut().enumerate() {
                profile_source(ui, &format!("Section {i}"), p, tree);
                if let Some(s) = p.sketch_mut() {
                    vector(ui, &format!("Section {i} origin mm"), &mut s.plane.origin);
                }
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
            edge_refs(ui, "Edge", edges);
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
            edge_refs(ui, "Edge", edges);
            face_ref(ui, "Base face", base_face);
            number(ui, "Equal distances mm", distance_mm);
        }
        Operation::Shell {
            source: id,
            open_faces,
            thickness_mm,
        } => {
            source(ui, "Source", id, tree);
            face_refs(ui, "Opening face", open_faces);
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
    cad_tools::placement(ui, &mut c.placement);
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

fn sketch_controls(ui: &mut egui::Ui, s: &mut Sketch, state: &mut CadState) {
    ui.menu_button((Icon::Files.image(ui, 18.), "Profile file"), |ui| {
        if ui.button((Icon::Files.image(ui, 18.), "Import SVG / DXF…")).clicked() {
            ui.close();
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
        if ui.button((Icon::Export.image(ui, 18.), "Export profile…")).clicked() {
            ui.close();
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
        ringdesign_workbench::controls::row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(ui.available_width()));
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.menu_button((Icon::CadSketch.image(ui, 18.), "Reset profile"), |ui| {
        if ui.button("Rectangle").clicked() {
            ui.close();
            let plane = s.plane.clone();
            *s = Sketch::rectangle(8.0, 6.0);
            s.plane = plane;
            state.pending.clear();
        }
        if ui.button("Circle").clicked() {
            ui.close();
            let plane = s.plane.clone();
            *s = Sketch::circle(3.0);
            s.plane = plane;
            state.pending.clear();
        }
        if ui.button("Empty").on_hover_text("Remove every point, entity and constraint, and draw your own").clicked() {
            ui.close();
            s.points.clear();
            s.entities.clear();
            s.constraints.clear();
            state.pending.clear();
            state.point = None;
            state.entity = None;
        }
        });
        if ui.add_enabled(state.point.is_some() || state.entity.is_some(), egui::Button::new((Icon::Delete.image(ui, 18.), "Delete selected")))
            .on_disabled_hover_text("Click a point or a curve in the canvas first").clicked() {
            state.delete_requested = true;
        }
        if ui.button((Icon::Check.image(ui, 18.), "Solve")).on_hover_text("Solve the sketch constraints").clicked() {
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
        ui.weak("Starter profiles hold their dimensions. Change the distance constraints to resize them; Solve restores those dimensions after a point drag.");
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
            if let Constraint::Distance { a, b, mm } = c {
                ringdesign_workbench::controls::row(ui, &format!("Distance #{a} to #{b}"), |ui| {
                    ui.add(egui::DragValue::new(mm).range(0.0001..=1000.0).speed(0.05).suffix(" mm"));
                });
            } else {
                ui.label(format!("{i}: {c:?}"));
            }
        }
        let mut remove = None;
        for e in &mut s.entities {
            ui.horizontal(|ui| {
                if ui.selectable_label(state.entity == Some(e.id), format!("Entity #{} {:?}", e.id, e.geometry)).clicked() {
                    state.entity = Some(e.id);
                    state.point = None;
                }
                ui.checkbox(&mut e.construction, "Construction");
                if ui.small_button("Delete").clicked() {
                    remove = Some(e.id);
                }
            });
        }
        if let Some(id) = remove {
            s.remove_entity(id);
            state.entity = None;
            state.pending.clear();
        }
        if ui.add_enabled(!s.constraints.is_empty(), egui::Button::new((Icon::Delete.image(ui, 18.), "Remove last constraint"))).clicked() {
            s.constraints.pop();
        }
    });
    let tools = ["Select", "Line", "Polyline", "Circle", "Arc", "Cubic"];
    ui.horizontal_wrapped(|ui| {
        ui.menu_button((Icon::CadSketch.image(ui, 18.), format!("Tool: {}", tools[state.tool])), |ui| {
            for (i, label) in tools.iter().enumerate() {
                if ui.selectable_value(&mut state.tool, i, *label).clicked() {
                    state.pending.clear();
                    ui.close();
                }
            }
        });
        ui.checkbox(&mut state.construction, "Construction");
    });
    ringdesign_workbench::controls::row(ui, "Grid", |ui| { ui.add(egui::DragValue::new(&mut s.grid_mm).range(0.01..=10.0).speed(0.01).suffix(" mm")); });
    ringdesign_workbench::controls::row(ui, "Constraint", |ui| {
        egui::ComboBox::from_id_salt("constraint_kind")
            .selected_text(["Distance", "Horizontal", "Vertical", "Coincident", "Symmetry", "Tangent"][state.constraint_kind])
            .show_ui(ui, |ui| {
                for (i, label) in ["Distance", "Horizontal", "Vertical", "Coincident", "Symmetry", "Tangent"].iter().enumerate() {
                    ui.selectable_value(&mut state.constraint_kind, i, *label);
                }
            });
    });
    ui.add_enabled_ui(state.constraint_kind == 0, |ui| {
        ringdesign_workbench::controls::row(ui, "Distance", |ui| { ui.add(egui::DragValue::new(&mut state.dimension).range(0.001..=1000.0).speed(0.1).suffix(" mm")); });
    });
    let needed = match state.constraint_kind { 4 => 3, 5 => 4, _ => 2 };
    if ui.add_enabled(state.pending.len() >= needed,
        egui::Button::new((Icon::Check.image(ui, 18.), "Constrain points")))
        .on_disabled_hover_text(format!("Shift-click {needed} points in order; tangent uses line endpoints, circle center, and tangent point")).clicked() {
        let p = &state.pending;
        s.constraints.push(match state.constraint_kind {
            0 => Constraint::Distance { a:p[0], b:p[1], mm:state.dimension },
            1 => Constraint::Horizontal(p[0],p[1]),
            2 => Constraint::Vertical(p[0],p[1]),
            3 => Constraint::Coincident(p[0],p[1]),
            4 => Constraint::Symmetry { a:p[0], b:p[1], center:p[2] },
            _ => Constraint::Tangent { a:p[0], b:p[1], center:p[2], at:p[3] },
        });
        state.pending.clear();
    }
}

fn sketch_canvas(ui: &mut egui::Ui, s: &mut Sketch, state: &mut CadState) {
    ui.weak("Millimeters • click to draw • click the first point or press C to close a polyline, Enter leaves it open • Shift-click points for constraints • drag points to edit • drag empty space, Shift-drag or middle-drag to pan • wheel to zoom • Delete removes the selection");
    let (rect, response) = ui.allocate_exact_size(
        ui.available_size().max(vec2(200.0, 180.0)),
        egui::Sense::click_and_drag(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Sketch canvas"));
    if std::mem::take(&mut state.delete_requested) {
        if let Some(id) = state.entity.take() {
            s.remove_entity(id);
        } else if let Some(id) = state.point.take() {
            s.remove_point(id);
        }
        state.pending.clear();
    }
    if response.hovered() {
        let old = state.sketch_scale;
        let new = (old * (1.0 + ui.input(|i| i.smooth_scroll_delta.y) * 0.002)).clamp(2.0, 300.0);
        if let Some(cursor) = response.hover_pos().filter(|_| new != old) {
            // Keep the millimetre under the cursor where it is.
            state.sketch_pan += (cursor - rect.center() - state.sketch_pan) * (1.0 - new / old);
        }
        state.sketch_scale = new;
    }
    let shift = ui.input(|i| i.modifiers.shift);
    let scale = state.sketch_scale;
    let centre = rect.center() + state.sketch_pan;
    let map = |p: [f64; 2]| centre + vec2(p[0] as f32, -p[1] as f32) * scale;
    let inverse = |p: egui::Pos2| {
        [
            (p.x - centre.x) as f64 / scale as f64,
            -(p.y - centre.y) as f64 / scale as f64,
        ]
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);
    let grid = (s.grid_mm as f32 * scale).max(8.0);
    let columns = ((rect.left() - centre.x) / grid).floor() as i32..=((rect.right() - centre.x) / grid).ceil() as i32;
    for i in columns {
        let x = centre.x + i as f32 * grid;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(if i == 0 { 1.0 } else { 0.5 }, theme::GRID),
        );
    }
    let rows = ((rect.top() - centre.y) / grid).floor() as i32..=((rect.bottom() - centre.y) / grid).ceil() as i32;
    for i in rows {
        let y = centre.y + i as f32 * grid;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(if i == 0 { 1.0 } else { 0.5 }, theme::GRID),
        );
    }
    // Screen polylines per entity, kept for picking.
    let mut drawn: Vec<(u64, Vec<egui::Pos2>)> = Vec::new();
    for e in &s.entities {
        for curve in s.curves_of(e).unwrap_or_default() {
            let points: Vec<egui::Pos2> = curve.tessellate_within(0.02).into_iter().map(map).collect();
            painter.add(egui::Shape::line(
                points.clone(),
                Stroke::new(
                    if e.construction { 1.0 } else { 2.0 },
                    if state.entity == Some(e.id) {
                        theme::WARN
                    } else if e.construction {
                        theme::TEXT_DIM
                    } else {
                        theme::ACCENT
                    },
                ),
            ));
            drawn.push((e.id, points));
        }
    }
    // The shape being drawn, with a rubber band to the cursor.
    if state.tool != 0 && !state.pending.is_empty() {
        let mut preview: Vec<egui::Pos2> = state.pending.iter().filter_map(|id| s.at(*id).ok()).map(map).collect();
        preview.extend(response.hover_pos());
        painter.add(egui::Shape::line(preview, Stroke::new(1.0, theme::INFO)));
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
        if response.drag_started_by(egui::PointerButton::Primary) && state.tool == 0 {
            state.point = if shift { None } else { nearest };
        }
        let moving = state.tool == 0 && !shift && state.point.is_some() && response.dragged_by(egui::PointerButton::Primary);
        if moving {
            if let Some(p) = s.points.iter_mut().find(|p| Some(p.id) == state.point) {
                p.xy = xy;
            }
        } else if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && (shift || state.tool == 0))
        {
            state.sketch_pan += response.drag_delta();
        }
        if response.clicked() {
            if shift {
                if let Some(id) = nearest {
                    if !state.pending.contains(&id) {
                        state.pending.push(id);
                    }
                }
            } else if state.tool == 0 {
                state.point = nearest;
                state.entity = if nearest.is_some() { None } else {
                    drawn.iter().filter_map(|(id, points)| {
                        points.windows(2).map(|w| {
                            let d = w[1] - w[0];
                            let t = ((pos - w[0]).dot(d) / d.length_sq().max(1e-8)).clamp(0.0, 1.0);
                            pos.distance(w[0] + d * t)
                        }).min_by(f32::total_cmp).map(|distance| (*id, distance))
                    }).filter(|(_, distance)| *distance < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
                };
            } else if state.tool == 2 && state.pending.len() >= 3 && nearest == state.pending.first().copied() {
                finish_polyline(s, state, true);
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
    if state.tool == 2 && !ui.ctx().egui_wants_keyboard_input() {
        let (close, leave_open) = ui.input(|i| (i.key_pressed(egui::Key::C), i.key_pressed(egui::Key::Enter)));
        if close && state.pending.len() >= 3 {
            finish_polyline(s, state, true);
        } else if leave_open && state.pending.len() >= 2 {
            finish_polyline(s, state, false);
        }
    }
}

fn finish_polyline(s: &mut Sketch, state: &mut CadState, closed: bool) {
    let points = std::mem::take(&mut state.pending);
    let id = s.entity(Geometry::Polyline { points, closed });
    if let Some(e) = s.entities.iter_mut().find(|e| e.id == id) {
        e.construction = state.construction;
    }
}
