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
use ringdesign_workbench::{
    cad_tools,
    command::{DimEvent, DimensionBar},
    icons::{self, Icon},
    render::{self, EdgeRun, StagedEdges},
    sketch_tools::{self, Input, Outcome, SnapCache, Tool, Underlay},
    timeline::{self, Action},
    viewport::{Mods, Sel},
};
use ringdesign_core::{
    BuildParams, RingDesign,
    cad::{self, Attach, Boolean, EdgeRef, Evaluated, FaceRef, Feature, Operation, Placement, Profile, edit::CadEdit},
    mesh::BuildResult,
    sketch::{Constraint, Geometry, RegionRef, Sketch},
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
    /// The whole ring as the Ring viewport builds it: band, seats, stamps and the resolved parts.
    built: Option<BuildResult>,
    /// Why the whole ring did not build, while the parts alone still show.
    build_error: Option<String>,
    /// The stone previews standing on the built ring's seats.
    gems: Vec<f32>,
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
    /// A modifier on `part`'s edge, signed against the evaluated body before it is added.
    Modify { operation: Operation, part: u64, edge: usize },
    /// Choose a feature in the tree, from the Ring viewport's menu.
    Select { feature: u64 },
    /// Show one part alone.
    Isolate { feature: u64 },
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
    /// The built ring itself rather than a part: the band is an anchor with no component.
    band: bool,
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
    /// Show the evaluated parts alone instead of the whole built ring.
    parts_only: bool,
    /// The Part edges switch as the view's edges were last staged.
    edges_shown: bool,
    /// The editing tools the canvas shares with the Ring viewport's sketch mode, and the feature they edit.
    shared: sketch_tools::Tools,
    shared_for: Option<NodeId>,
    shared_escape: bool,
    bar: DimensionBar,
    /// Where the 3D canvas was last laid out.
    canvas: egui::Rect,
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
            parts_only: false,
            edges_shown: true,
            shared: sketch_tools::Tools::default(),
            shared_for: None,
            shared_escape: false,
            bar: DimensionBar::new("cad-sketch-dimensions"),
            canvas: egui::Rect::NOTHING,
        }
    }
}
impl CadState {
    /// The edge the canvas has picked, as its part and that part's edge.
    pub(crate) fn picked_edge(&self) -> Option<(u64, usize)> {
        self.edge
    }
    #[cfg(test)]
    pub(crate) fn pick_edge(&mut self, part: u64, edge: usize) {
        self.edge = Some((part, edge));
    }
    /// The 3D canvas as laid out this frame, above the view's footer; `None` on the tabs that draw none.
    pub(crate) fn canvas(&self) -> Option<egui::Rect> {
        (!matches!(self.tab, 1 | 6) && self.canvas.is_positive()).then_some(self.canvas)
    }

    /// Escape from the global shortcut router: backs out of pending sketch picks, then a history
    /// rollback. It never discards the candidate.
    pub fn cancel_shortcut(&mut self) -> bool {
        if self.tab == 1 && self.tool >= CANVAS_TOOLS.len() && (self.shared.busy() || !self.shared.chosen_entities.is_empty()) {
            self.shared_escape = true;
            return true;
        }
        let editing = self.rollback.is_some() || !self.pending.is_empty();
        self.escape_requested |= editing;
        editing
    }
    /// Delete from the global shortcut router: the selected sketch point or entity.
    pub fn delete_shortcut(&mut self) -> bool {
        let chosen = !self.shared.chosen_points.is_empty() || !self.shared.chosen_entities.is_empty();
        let editing = self.tab == 1 && (self.point.is_some() || self.entity.is_some() || chosen);
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
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn selected_feature(&self) -> Option<u64> {
        self.selected.map(|id| id.0)
    }
    /// The last evaluation or apply failure.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
#[cfg(test)]
impl CadState {
    /// Chooses `feature` and opens its sketch on the canvas.
    pub fn open_sketch(&mut self, feature: u64) {
        self.selected = Some(NodeId(feature));
        self.tab = 1;
    }
    /// Where a sketch point shows on the canvas drawn in `rect`.
    pub fn canvas_point(&self, rect: egui::Rect, xy: [f64; 2]) -> egui::Pos2 {
        rect.center() + self.sketch_pan + vec2(xy[0] as f32, -xy[1] as f32) * self.sketch_scale
    }
    /// What the shared tool asks for next, and what the pane last said.
    pub fn tool_words(&self) -> (String, String) {
        (self.shared.prompt(), self.message.clone())
    }
    /// The sketch the candidate holds in `feature`.
    pub fn candidate_sketch(&self, feature: u64) -> Option<Sketch> {
        let n = self.draft.as_ref()?.node(NodeId(feature))?;
        let mut f: Feature = serde_json::from_value(n.params.clone()).ok()?;
        f.operation.sketch_mut().cloned()
    }
    /// The 3D canvas's rect and its scale at the centre in pixels per millimetre.
    pub fn canvas_scale(&self) -> (egui::Rect, f64) {
        let rect = self.canvas;
        let (view, _) = ringdesign_workbench::hover::view_scale(rect.center(), &|p| self.camera.ray(rect, p));
        (rect, view.px_per_mm)
    }
    /// The operation the candidate holds in `feature`.
    pub fn candidate_operation(&self, feature: u64) -> Option<Operation> {
        let n = self.draft.as_ref()?.node(NodeId(feature))?;
        serde_json::from_value::<Feature>(n.params.clone()).ok().map(|f| f.operation)
    }
    /// The edge runs the view's renderer holds, and every drawn part's runs in the view's evaluation once there is one.
    pub fn edge_runs(&self) -> (Vec<EdgeRun>, Option<Vec<EdgeRun>>) {
        let held = self.renderer.lock().map(|r| r.staged_edges().1.to_vec()).unwrap_or_default();
        (held, self.view.as_ref().map(|v| render::stage_edges(&v.evaluated).runs))
    }
    /// The evaluated parts' ids and names.
    pub fn parts(&self) -> Vec<(u64, String)> {
        self.view.as_ref().map_or_else(Vec::new, |v| v.evaluated.components.iter().map(|c| (c.id, c.name.clone())).collect())
    }
    /// Spreads the parts `mm` apart, as the Display menu's Explode field does.
    pub fn explode_by(&mut self, mm: f64) {
        self.explode = mm;
        upload(self, false);
    }
    /// The 3D canvas's camera, and whether a turn is still easing it.
    pub fn view_camera(&self) -> (OrbitCamera, bool) {
        (self.camera, self.display.turn.is_some())
    }
    /// Chooses `feature` in the tree, as a click on it does.
    pub fn choose_feature(&mut self, feature: u64) {
        self.selected = Some(NodeId(feature));
    }
    /// The part `id`'s bounds as the canvas draws it.
    pub fn drawn_bounds(&self, id: u64) -> Option<(ringdesign_core::Vec3, ringdesign_core::Vec3)> {
        drawn_part(self, id).map(|(b, _)| b)
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
/// Add the modifier called `label` (Fillet or Chamfer) on `part`'s edge, signed by the pane.
pub fn add_modifier(app: &mut RingDesignerApp, label: &str, part: u64, edge: usize) {
    if let Some(operation) = cad_tools::starters(part, 0).into_iter().find(|op| op.label() == label) {
        ask(app, CadRequest::Modify { operation, part, edge });
    }
}
/// Starters that make sense seated on the ring's surface.
pub const PLACEABLE: [&str; 6] = ringdesign_workbench::viewport::menu::PLACEABLE;
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
    if let Some(b) = &v.built {
        ui.label(format!(
            "Whole ring {:.2} mm³ • {} tris • {} ms",
            b.report.volume_mm3, b.report.validation.triangle_count, b.report.build_ms
        ));
    }
    if let Some(e) = &v.build_error {
        ui.colored_label(theme::WARN, e);
    }
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
            c.brep().map_or(c.trace.patches.len(), |b| b.faces.len()),
            c.brep().map_or(c.edges.len(), |b| b.edges.len())
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
    // The whole ring builds at the preview quality the Ring viewport shows it at.
    let ring_params = BuildParams { refine: None, ..app.preview_params };
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
                // The ring is what the Ring viewport will show; the parts it resolved stay for the inspector,
                // placed on the built surface. A ring of parts only is evaluated on its own.
                let (mut built, build_error) = match ringdesign_core::mesh::try_build_with(&d, &lib, ring_params, &cancel) {
                    Ok(b) => (Some(b), None),
                    Err(e) => (None, Some(format!("Whole ring did not build: {e:#}"))),
                };
                let evaluated = match built.as_mut().and_then(|b| b.parts.evaluated.take()) {
                    Some(e) => e,
                    None => cad::evaluate_with(&d, &lib, params, &cad::BuildCtx { cancel: &cancel, surface: None })?,
                };
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
                let gems = built.as_ref().map_or_else(Vec::new, |_| ringdesign_core::gems::preview_vertices(&d, &lib));
                Ok(View {
                    design: d,
                    evaluated,
                    pairs,
                    walls,
                    built,
                    build_error,
                    gems,
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
            Some(EdgeRef::signed(c.brep()?, edge, &c.frame))
        })
        .unwrap_or_else(|| EdgeRef::bare(edge))
}
/// Whether the candidate carries a procedural shank feature.
fn has_band(g: &Graph) -> bool {
    g.nodes.iter().filter(|n| n.kind == "cad.feature").any(|n| {
        serde_json::from_value::<Feature>(n.params.clone())
            .is_ok_and(|f| matches!(f.operation, Operation::Band))
    })
}
/// Append one feature to the candidate and select it; an anchor seats it on the ring.
/// A new solid beside a procedural shank starts joined to it.
fn add_feature(state: &mut CadState, g: &mut Graph, mut operation: Operation, anchor: Option<(f64, f64)>) {
    // Bare face and edge references are signed against the evaluation from before the feature consumes its source.
    if let Some(v) = &state.view {
        cad::sign_refs(&mut operation, &v.evaluated);
    }
    // The first solid added to a plain ring brings the procedural shank with it, so the part stands
    // beside the band instead of replacing it; a ring already made of parts only stays that way.
    let body = operation.sources().is_empty() && !matches!(operation, Operation::Band | Operation::Sketch { .. });
    if body && !g.nodes.iter().any(|n| n.kind == "cad.feature") {
        if let Err(e) = graph_cad::append(g, Operation::Band) {
            state.error = Some(e.to_string());
            return;
        }
    }
    let shank = has_band(g);
    match graph_cad::append(g, operation) {
        Ok(id) => {
            if let Some(node) = g.node_mut(id) {
                if let Ok(mut f) = serde_json::from_value::<Feature>(node.params.clone()) {
                    if let Some((theta, height)) = anchor {
                        f.component.placement = Placement::ring(theta, height);
                    }
                    let solid = f.operation.sources().is_empty()
                        && !matches!(f.operation, Operation::Band | Operation::Sketch { .. });
                    if shank && solid && !f.component.reference {
                        f.component.attach = Attach::Join;
                    }
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
/// Stages the view's metal, stones and edges, refitting the camera to the metal when `fit`; the metal's bounds as drawn.
fn upload(state: &mut CadState, fit: bool) -> Option<(ringdesign_core::Vec3, ringdesign_core::Vec3)> {
    let Some(view) = &state.view else {
        return None;
    };
    let mut metal = ringdesign_core::Mesh::default();
    let mut gems = ringdesign_core::Mesh::default();
    // Isolating or exploding reads the parts one by one, which only the parts view draws.
    let whole = view
        .built
        .as_ref()
        .filter(|_| !state.parts_only && state.isolated.is_none() && state.explode <= 0.0);
    for (index, c) in view.evaluated.components.iter().enumerate() {
        if !c.settings.visible || state.isolated.is_some_and(|id| id != c.id) {
            continue;
        }
        if whole.is_some() && !c.settings.reference {
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
    let metal = whole.map_or(&metal, |b| &b.mesh);
    let bounds = metal.bounds();
    if fit {
        state.camera.pivot_home();
        state.camera.fit(bounds);
    }
    let edges = view_edges(view, whole.is_some(), state);
    if let Ok(mut r) = state.renderer.lock() {
        r.prepare_cad(metal);
        let mut stones = GpuMeshRenderer::stage_plain(&gems);
        if whole.is_some() {
            stones.extend_from_slice(&view.gems);
        }
        r.prepare_gems(stones);
        r.prepare_view_edges(state.view_key as usize, edges);
    }
    bounds
}

/// The part `id` as `upload` draws it, exploded along with the rest; `None` when it is hidden, isolated away or no part.
fn drawn_part(state: &CadState, id: u64) -> Option<((ringdesign_core::Vec3, ringdesign_core::Vec3), String)> {
    let view = state.view.as_ref()?;
    let (index, c) = view.evaluated.components.iter().enumerate()
        .find(|(_, c)| c.id == id && c.settings.visible && state.isolated.is_none_or(|i| i == c.id))?;
    let (lo, hi) = c.mesh.bounds()?;
    let dz = index as f32 * state.explode as f32;
    Some(((ringdesign_core::Vec3(lo.0, lo.1, lo.2 + dz), ringdesign_core::Vec3(hi.0, hi.1, hi.2 + dz)), c.name.clone()))
}

/// The drawn part a feature chosen in the history stands for: its own, else the part that feature became by the features consuming it.
fn chosen_part(state: &CadState, id: u64) -> Option<((ringdesign_core::Vec3, ringdesign_core::Vec3), String)> {
    let doc = state.view.as_ref()?.design.cad.as_ref();
    let mut at = id;
    for _ in 0..=doc.map_or(0, |d| d.features.len()) {
        if let Some(part) = drawn_part(state, at) {
            return Some(part);
        }
        at = doc?.features.iter().find(|f| f.enabled && f.operation.consumes().contains(&at))?.id;
    }
    None
}

/// Eases the view onto the part chosen in the history as drawn when `chosen` and there is one, else all the metal drawn, the pivot moved onto its middle.
fn fit_view(state: &mut CadState, chosen: bool) -> Option<ringdesign_workbench::touch::view::Framed> {
    use ringdesign_workbench::touch::view::Framed;
    let shown = upload(state, false)?;
    let part = state.selected.filter(|_| chosen).and_then(|id| chosen_part(state, id.0));
    let alone = state.isolated.and_then(|id| drawn_part(state, id));
    state.camera.refit(shown);
    let (bounds, framed) = match (part, alone) {
        (Some((b, name)), _) => (b, Framed::Chosen(vec![name])),
        (None, Some((_, name))) => (shown, Framed::Alone(vec![name])),
        (None, None) => (shown, Framed::Ring),
    };
    let to = state.camera.framing(bounds);
    state.display.turn = Some(ringdesign_workbench::focus::Turn::new(state.camera.pose(), to));
    Some(framed)
}

/// The view's parts' edges for the edge pass: none while a part is isolated, the parts are exploded or the Part edges
/// switch is off; drawn alone, only the shown parts'.
fn view_edges(view: &View, whole: bool, state: &CadState) -> StagedEdges {
    if !state.edges_shown || state.isolated.is_some() || state.explode > 0.0 {
        return StagedEdges::default();
    }
    let staged = render::stage_edges(&view.evaluated);
    if whole {
        return staged;
    }
    keep_runs(staged, |id| view.evaluated.components.iter().any(|c| c.id == id && c.settings.visible))
}

/// `staged` with only the runs of the parts `keep` names, renumbered.
fn keep_runs(staged: StagedEdges, keep: impl Fn(u64) -> bool) -> StagedEdges {
    if staged.runs.iter().all(|r| keep(r.key.0)) {
        return staged;
    }
    let floats = render::edges::EDGE_FLOATS;
    let mut out = StagedEdges { dropped: staged.dropped, ..StagedEdges::default() };
    for run in staged.runs.iter().filter(|r| keep(r.key.0)) {
        let first = (out.verts.len() / floats) as u32;
        out.verts.extend_from_slice(&staged.verts[run.first as usize * floats..(run.first + run.count) as usize * floats]);
        out.runs.push(EdgeRun { first, ..*run });
    }
    out
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
    let mut isolate_requested = false;
    for request in std::mem::take(&mut state.requests) {
        match request {
            CadRequest::Add { operation, anchor } => add_feature(&mut state, &mut g, operation, anchor),
            CadRequest::Modify { mut operation, part, edge } => {
                state.edge = Some((part, edge));
                if let Operation::Fillet { edges, .. } | Operation::Chamfer { edges, .. } = &mut operation {
                    *edges = vec![signed_edge(&state, part, edge)];
                }
                add_feature(&mut state, &mut g, operation, None);
            }
            CadRequest::Select { feature } => {
                state.selected = Some(NodeId(feature));
                state.tab = 0;
                state.edge = None;
            }
            CadRequest::Isolate { feature } => {
                state.selected = Some(NodeId(feature));
                state.isolated = Some(feature);
                state.tab = 0;
                isolate_requested = true;
            }
            CadRequest::Preview => preview_requested = true,
            CadRequest::Apply => apply_requested = true,
            CadRequest::Discard => discard_requested = true,
        }
    }
    if isolate_requested {
        upload(&mut state, true);
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
                    let valid = !modify || (ids.contains(&previous)
                        && (!matches!(op, Operation::Boolean {..}) || second != 0));
                    let hint = if valid { cad_tools::hint(&op) } else if matches!(op, Operation::Boolean {..}) {
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
            let committed = state.selected.is_some_and(|id| app.design.cad.as_ref().and_then(|d| d.feature(id.0)).is_some_and(|f| f.operation.clone().sketch_mut().is_some()));
            if ui.add_enabled(editable && committed, egui::Button::new((Icon::View.image(ui, 18.), "Draw in the Ring viewport")))
                .on_hover_text("Sketch it where it lies on the ring, the ring drawn under it")
                .on_disabled_hover_text("Apply the sketch first: the Ring viewport draws the sketch the design holds").clicked() {
                if let Some(id) = state.selected { crate::sketch_mode::start_in_ring(app, id.0); }
                ui.close();
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
                    applied.pins = app.design.pins.clone();
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
    // The candidate's sketch features, read only while an extrusion or revolution is chosen, for its profile to take one region of.
    let sweeps = state.selected.and_then(|id| g.node(id)).is_some_and(|n| ["Extrude", "Revolve"].iter().any(|k| n.params["operation"].get(k).is_some()));
    let sketches: Vec<(u64, Sketch)> = if sweeps {
        g.nodes
            .iter()
            .filter(|n| n.kind == "cad.feature")
            .filter_map(|n| match serde_json::from_value::<Feature>(n.params.clone()).ok()?.operation {
                Operation::Sketch { sketch } => Some((n.id.0, sketch)),
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    egui::Panel::left(ui.id().with("cad-inspector"))
        .resizable(true).default_size(280.).size_range(230.0..=460.)
        .frame(egui::Frame::new().inner_margin(8).fill(theme::PANEL))
        .show(ui, |ui| {
        ui.strong(match state.tab { 1 => "CAD · Sketch", 2 => "CAD · Components", 3 => "CAD · Source", 4 => "CAD · Sizes", 5 => "CAD · Stages", 6 => "CAD · Section", _ => "CAD · Features" });
        ui.weak("1 Create   2 Edit & preview   3 Apply");
        egui::ScrollArea::vertical().id_salt("cad-inspector-scroll").auto_shrink([false,false]).show(ui, |ui| {
        egui::CollapsingHeader::new("Feature history").default_open(true).show(ui, |ui| {
            if tree.is_empty() { ui.label("The current ring is the source. Add a Procedural shank to modify it as a CAD solid, or start from an Example project."); }
            history(app, ui, &mut state, &mut g, &original, &tree);
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
                            0=>{if bound_operation {ui.weak("Operation is bound to a graph input; edit its upstream parameters in Graph.");} else {operation_ui(ui,&mut f.operation,&tree,&sketches);}},
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
                        if let Some(sketch) = f.operation.sketch_mut() {
                            if state.shared_for != Some(id) {
                                state.shared.reset();
                                state.shared_for = Some(id);
                            }
                            sketch_canvas(ui, sketch, &mut state);
                        }
                        n.params = serde_json::to_value(f).unwrap();
                    }
                }
            }
        }
    }
    if state.tab == 6 {
        if let Some((color, text)) = stale_banner(&state, current, "Previous section — preview the changed parameters to update it") {
            ui.colored_label(color, text);
        }
        section_ui(ui, &mut state);
    }
    if state.tab != 1 && state.tab != 6 {
        // Laid over the canvas once it is placed, so a note coming or going never resizes it under a drag.
        let mut notes: Vec<(egui::Color32, String)> = Vec::new();
        if let Some((color, text)) = stale_banner(&state, current, "Parameters changed — preview to evaluate this candidate") {
            notes.push((color, text.into()));
        }
        if let Some(e) = state.view.as_ref().and_then(|v| v.build_error.as_deref()) {
            notes.push((theme::WARN, e.into()));
        }
        let mut redraw = false;
        let mut fit_all = false;
        egui::Panel::bottom(ui.id().with("cad-view-footer"))
            .frame(egui::Frame::new().inner_margin(6).fill(theme::PANEL)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.menu_button((Icon::View.image(ui,18.), "View"), |ui| {
                    for view in ringdesign_workbench::navigation::View::ALL {
                        if ui.button(view.label()).clicked() {
                            let angles = ringdesign_workbench::navigation::Action::View(view)
                                .apply([state.camera.yaw,state.camera.pitch,state.camera.roll],app.design.shank.head.theta_deg as f32);
                            state.camera.pivot_home();
                            let from = state.camera.pose();
                            state.display.turn = Some(ringdesign_workbench::focus::Turn::new(from,
                                ringdesign_workbench::focus::Pose { yaw:angles[0],pitch:angles[1],roll:angles[2],pan:[0.;2],..from }));
                            ui.close();
                        }
                    }
                    ui.checkbox(&mut state.display.navigation.locked, "Lock orbit (drag to pan)");
                });
                if icons::compact(ui,Icon::Fit,false).clicked() { fit_all = true; }
                if icons::compact(ui,Icon::Wire,state.display.wire).clicked() {state.display.wire = !state.display.wire;}
                if icons::compact(ui,Icon::Grid,state.display.grid).clicked() {state.display.grid = !state.display.grid;}
                let ring_built = state.view.as_ref().is_some_and(|v| v.built.is_some());
                if ui.add_enabled(ring_built, egui::Checkbox::new(&mut state.parts_only, "Parts only"))
                    .on_hover_text("Show the evaluated parts alone; off, the whole ring as the Ring viewport builds it")
                    .on_disabled_hover_text("The whole ring did not build, so only the parts are shown").changed() { redraw = true; }
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
        if fit_all && let Some(framed) = fit_view(&mut state, false) { app.set_status(framed.said()); }
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
            notes.push((theme::TEXT_DIM, format!("Selected component #{id}, edge {edge} • Add Fillet or Chamfer to use it")));
        }
        state.display.finish=app.finish; state.display.polish=app.polish; state.display.light=app.light; state.display.show_gems=app.show_gems;
        let edges_shown = app.show_part_edges;
        if edges_shown != state.edges_shown {
            state.edges_shown = edges_shown;
            upload(&mut state, false);
        }
        let (rect, response) =
            crate::viewport::candidate_view(ui, state.renderer.clone(), &mut state.camera, &mut state.display, app.design.shank.head.theta_deg as f32);
        state.canvas = rect;
        overlay_notes(ui, rect, &notes);
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
            // With no part under the cursor the built ring answers, so a part can still be added on the band.
            let on_ring = under.is_none().then(|| response.interact_pointer_pos().and_then(|pos| {
                let built = state.view.as_ref()?.built.as_ref()?;
                let (origin, direction) = state.camera.ray(rect, pos);
                ringdesign_core::interaction::picking::raycast(&built.mesh, origin, direction).map(|(_, point)| point)
            })).flatten();
            state.menu_hit = match (under, nearest, on_ring) {
                (Some((part, point)), edge, _) => Some(MenuHit { part, point: Some(point), edge: edge.filter(|(id, _, _)| *id == part).map(|(_, e, _)| e), band: false }),
                (None, Some((part, edge, _)), _) => Some(MenuHit { part, point: None, edge: Some(edge), band: false }),
                (None, None, Some(point)) => Some(MenuHit { part: 0, point: Some(point), edge: None, band: true }),
                (None, None, None) => None,
            };
        }
        let hit = state.menu_hit;
        let ring_radius = state.view.as_ref().map(|v| v.design.inner_radius_mm() + v.design.profile.thickness_mm);
        let part_name = hit.and_then(|h| state.view.as_ref()?.evaluated.components.iter().find(|c| c.id == h.part).map(|c| c.name.clone()));
        let mut refit = false;
        let mut fit = false;
        response.context_menu(|ui| {
            ui.set_min_width(190.);
            if let Some(hit) = hit {
                ui.weak(if hit.band { "Band" } else { part_name.as_deref().unwrap_or("Component") });
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
                if !hit.band && ui.button((Icon::Panel.image(ui, 18.), "Select feature")).clicked() {
                    state.selected = Some(NodeId(hit.part));
                    state.tab = 0;
                    ui.close();
                }
                if !hit.band && ui.button((Icon::Layers.image(ui, 18.), "Isolate")).clicked() {
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
            if ui.button((Icon::Fit.image(ui, 18.), "Fit view")).on_hover_text("Frame the chosen part, else all the metal shown").clicked() {
                fit = true;
                ui.close();
            }
            ui.checkbox(&mut state.display.wire, "Wireframe");
            ui.checkbox(&mut state.display.grid, "Grid");
        });
        if refit { upload(&mut state, true); }
        if fit && let Some(framed) = fit_view(&mut state, true) { app.set_status(framed.said()); }
        if matches!(state.tab, 0 | 2) && state.rollback.is_none() {
            direct_handles(ui, rect, &state, &mut g);
        }
    }
    });
    if hash(&g) != hash(&original) {
        state.draft = Some(g);
    }
    app.cad = state;
    crate::occt::cad_pane(app, ui);
}

/// `notes` on a plate over the top of the canvas at `rect`, taking none of its room.
fn overlay_notes(ui: &mut egui::Ui, rect: egui::Rect, notes: &[(egui::Color32, String)]) {
    if notes.is_empty() {
        return;
    }
    let mut over = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::Frame::new().fill(theme::FLOAT.gamma_multiply(0.9)).corner_radius(4).inner_margin(6).show(&mut over, |ui| {
        for (color, text) in notes {
            ui.add(egui::Label::new(egui::RichText::new(text).color(*color)).selectable(false));
        }
    });
}

/// What the viewer says while its evaluation is not of the candidate shown: that the first one is on its
/// way until it lands (a failure says itself above), then `changed`.
fn stale_banner(state: &CadState, current: u64, changed: &'static str) -> Option<(egui::Color32, &'static str)> {
    if state.view_key == current {
        return None;
    }
    match &state.view {
        None if state.error.is_none() => Some((theme::INFO, "Evaluating the candidate…")),
        None => None,
        Some(_) => Some((theme::WARN, changed)),
    }
}

/// One history row the timeline does not draw: a resize, or every node of a chain that is not plain features.
fn history_row(app: &mut RingDesignerApp, ui: &mut egui::Ui, state: &mut CadState, g: &Graph, id: NodeId, label: &str) {
    let icon = g.node(id).and_then(|n| serde_json::from_value::<Feature>(n.params.clone()).ok()).map_or(Icon::Graph, |f| cad_tools::icon(&f.operation));
    if ui
        .add_sized([ui.available_width(), ui.spacing().interact_size.y], egui::Button::new((icon.image(ui, 18.), label)).selected(state.selected == Some(id)).right_text(egui::Atom::grow()))
        .on_hover_text(format!("Feature #{}", id.0))
        .clicked()
    {
        state.selected = Some(id);
        app.selected_node = Some(id);
        state.pending.clear();
        state.json_node = None;
    }
}

/// Chooses a feature in the tree and on the Ring viewport.
fn choose(app: &mut RingDesignerApp, state: &mut CadState, id: u64) {
    state.selected = Some(NodeId(id));
    app.selected_node = Some(NodeId(id));
    state.pending.clear();
    state.json_node = None;
    app.selection.click(Some(Sel::Part(id)), Mods::default());
}

/// The feature history as the timeline's list: chips read off the candidate's document, statuses from
/// the pane's evaluation while it is of this candidate, and resizes as rows of their own.
fn history(app: &mut RingDesignerApp, ui: &mut egui::Ui, state: &mut CadState, g: &mut Graph, original: &Graph, tree: &[(NodeId, String)]) {
    let doc = match graph_cad::document(g) {
        Ok(doc) => doc,
        Err(e) => {
            ui.weak(format!("This history is more than a chain of features ({e}); its steps are listed as nodes."));
            for (id, label) in tree {
                history_row(app, ui, state, g, *id, label);
            }
            return;
        }
    };
    if !doc.features.is_empty() {
        let key = hash(&(&*g, state.rollback));
        let evaluated = state.view.as_ref().filter(|_| state.view_key == key).map(|v| &v.evaluated);
        let selected: Vec<u64> = state.selected.map(|id| id.0).into_iter().collect();
        let chips = timeline::chips(&doc, evaluated, &selected);
        for action in timeline::show(ui, &chips, doc.through, true) {
            serve(app, state, g, original, &doc, action);
        }
    }
    for (id, label) in tree.iter().filter(|(id, _)| doc.feature(id.0).is_none()) {
        history_row(app, ui, state, g, *id, label);
    }
}

/// One timeline action in the pane. Structural edits go through the funnel while nothing is pending,
/// and onto the candidate's graph while one is, so a candidate is never thrown away by them.
fn serve(app: &mut RingDesignerApp, state: &mut CadState, g: &mut Graph, original: &Graph, doc: &cad::Document, action: Action) {
    match action {
        Action::Select(id) => choose(app, state, id),
        Action::Edit(id) => {
            choose(app, state, id);
            state.tab = 0;
            state.edge = None;
        }
        Action::Isolate(id) => {
            choose(app, state, id);
            state.isolated = Some(id);
            state.tab = 0;
            upload(state, true);
        }
        action => {
            let edits = match timeline::edits(doc, &action) {
                Ok(edits) if !edits.is_empty() => edits,
                Ok(_) => return,
                Err(reason) => {
                    app.set_status(reason);
                    return;
                }
            };
            if hash(&*g) != hash(original) {
                let before = g.clone();
                let mut labels = Vec::with_capacity(edits.len());
                for edit in &edits {
                    match graph_cad::apply_edit(g, edit) {
                        Ok(applied) => labels.push(applied.label),
                        Err(e) => {
                            *g = before;
                            app.set_status(e.to_string());
                            return;
                        }
                    }
                }
                app.set_status(format!("{} · in the candidate", labels.join(" · ")));
            } else if crate::cad_edit::apply(app, &edits).is_ok() {
                // Keeps the pane's choices across its own commit; the changed candidate re-evaluates.
                state.source = source_key(app);
            } else {
                return;
            }
            let gone: Vec<u64> = edits.iter().filter_map(|e| match e {
                CadEdit::Remove { id } => Some(*id),
                _ => None,
            }).collect();
            if state.selected.is_some_and(|id| gone.contains(&id.0)) {
                state.selected = None;
            }
            if state.isolated.is_some_and(|id| gone.contains(&id)) {
                state.isolated = None;
            }
            if state.edge.is_some_and(|(id, _)| gone.contains(&id)) {
                state.edge = None;
            }
            if state.rollback.is_some_and(|id| gone.contains(&id)) {
                state.rollback = None;
            }
        }
    }
}

fn direct_handles(ui: &mut egui::Ui, rect: egui::Rect, state: &CadState, g: &mut Graph) {
    use ringdesign_workbench::{command::Unit, grips::{self, Grip}};
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
    // One grip drawn and dragged on screen: the value it would set, when a drag moved it.
    let grip = |spec: &Grip| -> Option<f64> {
        let a = projector.at(world(spec.start));
        let b = projector.at(world(spec.at));
        if !rect.contains(b) {
            return None;
        }
        let unit = projector.at(world(std::array::from_fn(|k| spec.at[k] + spec.direction[k]))) - b;
        if unit.length() < 2.0 {
            return None;
        }
        painter.line_segment([a, b], Stroke::new(1.0, theme::INFO));
        painter.circle_filled(b, 5.0, theme::INFO);
        let grip_id = ui.id().with(("dimension_grip", id.0, spec.label));
        let response = ui.interact(egui::Rect::from_center_size(b, vec2(16.0, 16.0)), grip_id, egui::Sense::drag());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("Dimension grip: {}", spec.label)));
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            painter.text(b + vec2(10., -12.), egui::Align2::LEFT_BOTTOM, format!("{} {:.3} {}", spec.label, spec.value, spec.unit.suffix()), egui::FontId::proportional(12.), theme::TEXT);
        }
        type Drag = (f64, egui::Pos2, egui::Vec2);
        if response.drag_started()
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, (spec.value, origin, unit)));
        }
        let mut dragged = None;
        if response.dragged()
            && let (Some((initial, origin, axis)), Some(pointer)) = (ui.ctx().data(|d| d.get_temp::<Drag>(grip_id)), response.interact_pointer_pos())
        {
            // The pointer's move along the grip's screen axis, in millimetres along its line.
            dragged = Some(spec.dragged(initial, f64::from((pointer - origin).dot(axis) / axis.length_sq())));
        }
        if response.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<Drag>(grip_id));
        }
        response.on_hover_text("Drag to change this source dimension. Numeric fields accept exact values; Preview/Enter evaluates, Escape cancels.");
        dragged
    };
    // The operation's own grips, from the list the Ring viewport shows too.
    for spec in grips::grips(&feature.operation) {
        if let Some(value) = grip(&spec)
            && let Some(op) = grips::with(&feature.operation, spec.key, value)
        {
            feature.operation = op;
        }
    }
    if let Placement::Ring { theta_deg, height_mm, .. } = &mut feature.component.placement {
        let height = *height_mm;
        let radius = view.design.inner_radius_mm() + view.design.profile.thickness_mm + height;
        let round = Grip {
            key: "theta",
            label: "Ring position",
            unit: Unit::Deg,
            value: *theta_deg,
            start: [0.0; 3],
            at: [0.0, 2.0, 0.0],
            direction: [0.0, 1.0, 0.0],
            gain: 180.0 / (std::f64::consts::PI * radius.max(0.001)),
            minimum: f64::NEG_INFINITY,
            position: true,
        };
        if let Some(v) = grip(&round) {
            *theta_deg = v;
        }
        let out = Grip { key: "height", label: "Radial placement", unit: Unit::Mm, value: height, start: [0.0, 0.0, -height], at: [0.0; 3], direction: [0.0, 0.0, 1.0], gain: 1.0, minimum: f64::NEG_INFINITY, position: true };
        if let Some(v) = grip(&out) {
            *height_mm = v;
        }
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

/// Where a profile comes from: drawn in this feature, a Sketch feature earlier in the tree, or,
/// where `sketches` lists the tree's sketches, one region of one that holds several.
fn profile_source(ui: &mut egui::Ui, label: &str, profile: &mut Profile, tree: &[(NodeId, String)], sketches: Option<&[(u64, Sketch)]>) {
    let current = profile.feature();
    let regions_of = |id: u64| sketches.and_then(|s| s.iter().find(|(f, _)| *f == id)).and_then(|(_, s)| s.profile_regions().ok());
    let shown = match &*profile {
        Profile::Inline(_) => "Drawn in this feature".to_string(),
        Profile::Feature { feature } => format!("Sketch #{feature}"),
        Profile::Region { feature, region } => match regions_of(*feature).and_then(|r| region.position(&r).map(|(i, _)| (i, r.len()))) {
            Some((i, n)) => format!("Sketch #{feature} · region {} of {n}", i + 1),
            None => format!("Sketch #{feature} · one region"),
        },
    };
    ringdesign_workbench::controls::row(ui, label, |ui| {
        egui::ComboBox::from_id_salt(("profile-source", label))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "Drawn in this feature").clicked() && current.is_some() {
                    *profile = Sketch::rectangle(8.0, 6.0).into();
                }
                for (id, name) in tree {
                    let whole = matches!(profile, Profile::Feature { feature } if *feature == id.0);
                    if ui.selectable_label(whole, format!("#{} {name}", id.0)).clicked() {
                        *profile = Profile::Feature { feature: id.0 };
                    }
                    let Some(regions) = regions_of(id.0).filter(|r| r.len() > 1) else { continue };
                    let picked = match &*profile {
                        Profile::Region { feature, region } if *feature == id.0 => region.position(&regions).map(|(i, _)| i),
                        _ => None,
                    };
                    for (k, r) in regions.iter().enumerate() {
                        let words = format!("#{} {name} · region {} of {} ({:.2} mm²)", id.0, k + 1, regions.len(), r.area());
                        if ui.selectable_label(picked == Some(k), words).clicked()
                            && let Some(region) = RegionRef::of(r)
                        {
                            *profile = Profile::Region { feature: id.0, region };
                        }
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
fn operation_ui(ui: &mut egui::Ui, op: &mut Operation, tree: &[(NodeId, String)], sketches: &[(u64, Sketch)]) {
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
            profile_source(ui, "Profile", sketch, tree, Some(sketches));
            number(ui, "Height mm", height_mm);
            number(ui, "Taper degrees", draft_deg);
        }
        Operation::Revolve {
            sketch,
            pivot,
            axis,
            degrees,
            in_plane,
        } => {
            profile_source(ui, "Profile", sketch, tree, Some(sketches));
            let (origin, direction) = if *in_plane { ("Axis origin in the sketch's plane mm", "Axis direction in the sketch's plane") } else { ("Axis origin mm", "Axis direction") };
            vector(ui, origin, pivot);
            vector(ui, direction, axis);
            number(ui, "Revolution degrees", degrees);
        }
        Operation::Sweep { sketch, path } => {
            profile_source(ui, "Section", sketch, tree, None);
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
            ui.weak("Edit the planar path in Debug; the section stands square to the path at its start.");
        }
        Operation::Loft { sections } => {
            for (i, p) in sections.iter_mut().enumerate() {
                profile_source(ui, &format!("Section {i}"), p, tree, None);
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
        Operation::Builder { key, on, params } => crate::panels::builder::ui(ui, key, on, params, tree),
        Operation::Pattern { source: id, kind } => {
            source(ui, "Source", id, tree);
            match kind {
                cad::PatternKind::Ring { count, span_deg } | cad::PatternKind::About { count, span_deg, .. } => {
                    ui.add(egui::DragValue::new(count).range(2..=cad::pattern::MAX_PATTERN_COUNT).prefix("Instances "));
                    number(ui, "Span degrees", span_deg);
                }
                cad::PatternKind::Mirror { .. } => {
                    ui.weak("One reflected copy; what it reflects across is in Advanced source.");
                }
            }
        }
        Operation::Plane { offset_mm, .. } => number(ui, "Offset mm", offset_mm),
        Operation::PressPull { source: id, face, distance_mm } => {
            source(ui, "Source", id, tree);
            face_ref(ui, "Face", face);
            number(ui, "Distance mm", distance_mm);
        }
        Operation::Stored { recipe, mesh, .. } => {
            ui.weak(format!("{} triangles {} made; run it again where {} is to change it.", mesh.triangles, recipe.kernel_name(), recipe.kernel_name()));
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
        if ui
            .checkbox(&mut c.reference, "Reference stone (exclude from metal export)")
            .changed()
            && c.reference
        {
            c.attach = Attach::Separate;
        }
    });
    egui::ComboBox::from_id_salt("component_material")
        .selected_text(&c.material)
        .show_ui(ui, |ui| {
            for m in ringdesign_core::metal::METALS {
                ui.selectable_value(&mut c.material, m.name.to_string(), m.name);
            }
        });
    cad_tools::seat(ui, &mut c.placement, &mut f.operation);
    cad_tools::attachment(ui, c);
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
    ui.horizontal_wrapped(|ui| {
        ui.menu_button((Icon::CadSketch.image(ui, 18.), format!("Tool: {}", tool_label(state.tool))), |ui| {
            for i in 0..CANVAS_TOOLS.len() + Tool::EDITING.len() {
                let label = tool_label(i);
                let r = match shared_tool(i) {
                    Some(t) => ui.add(egui::Button::selectable(state.tool == i, (t.icon().image(ui, 18.), label))).on_hover_text(t.hint()),
                    None => ui.selectable_label(state.tool == i, label),
                };
                if r.clicked() {
                    state.tool = i;
                    state.pending.clear();
                    if let Some(t) = shared_tool(i) {
                        state.shared.set_tool(t);
                    }
                    ui.close();
                }
            }
        });
        ui.checkbox(&mut state.construction, "Construction");
    });
    if shared_tool(state.tool).is_some() {
        ui.weak(state.shared.prompt());
    }
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
        let mut points = std::mem::take(&mut state.shared.chosen_points);
        let mut entities = std::mem::take(&mut state.shared.chosen_entities);
        points.extend(state.point.take());
        entities.extend(state.entity.take());
        sketch_tools::delete(s, &points, &entities);
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
                    if state.entity == Some(e.id) || state.shared.is_chosen(e.id) {
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
    for a in sketch_tools::annotations(s) {
        let (p, q) = (map(a.from), map(a.to));
        let along = (q - p).normalized();
        painter.text(p + (q - p) * 0.5 + vec2(-along.y, along.x) * 12.0, egui::Align2::CENTER_CENTER, &a.text, egui::FontId::proportional(11.0), theme::INFO);
    }
    if shared_tool(state.tool).is_some() {
        shared_canvas(ui, s, state, &response, rect, &map, &inverse);
        return;
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

/// The canvas's own drawing tools; the editing tools it shares with sketch mode follow them in its list.
const CANVAS_TOOLS: [&str; 6] = ["Select", "Line", "Polyline", "Circle", "Arc", "Cubic"];

/// The shared tool at place `i` of the canvas's list, past its own drawing tools.
fn shared_tool(i: usize) -> Option<Tool> {
    i.checked_sub(CANVAS_TOOLS.len()).and_then(|k| Tool::EDITING.get(k).copied())
}

fn tool_label(i: usize) -> &'static str {
    shared_tool(i).map_or_else(|| CANVAS_TOOLS.get(i).copied().unwrap_or("Select"), Tool::label)
}

/// Says what a shared tool did on the pane's message line.
fn say(state: &mut CadState, out: Outcome) {
    if let Outcome::Edited(words) | Outcome::Refused(words) = out {
        state.message = words;
    }
}

/// The canvas under a shared editing tool: the pointer snapped and fed, clicks, typed values, keys and the preview.
fn shared_canvas(
    ui: &mut egui::Ui,
    s: &mut Sketch,
    state: &mut CadState,
    response: &egui::Response,
    rect: egui::Rect,
    map: &impl Fn([f64; 2]) -> egui::Pos2,
    inverse: &impl Fn(egui::Pos2) -> [f64; 2],
) {
    let painter = ui.painter_at(rect);
    let reach = 8.0 / f64::from(state.sketch_scale.max(1e-3));
    let pointer = response.hover_pos().or_else(|| response.interact_pointer_pos());
    if let Some(pos) = pointer {
        let raw = inverse(pos);
        let under = Underlay::default();
        let snapped = sketch_tools::snap(s, &under, &SnapCache::of(s, &under), raw, reach, Some(s.grid_mm));
        state.shared.feed(s, Input::Pointer { raw, snapped, reach });
        if let Some(snap) = snapped {
            painter.circle_stroke(map(snap.xy), 7.0, Stroke::new(1.0, theme::INFO));
            painter.text(map(snap.xy) + vec2(10.0, 10.0), egui::Align2::LEFT_TOP, snap.kind.label(), egui::FontId::proportional(12.0), theme::INFO);
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) || response.dragged_by(egui::PointerButton::Middle) {
        state.sketch_pan += response.drag_delta();
    }
    if response.clicked() {
        // A click on the canvas releases whatever held the keys.
        if let Some(f) = ui.ctx().memory(|m| m.focused()) {
            ui.ctx().memory_mut(|m| m.surrender_focus(f));
        }
        let add = ui.input(|i| i.modifiers.shift);
        let out = state.shared.feed(s, Input::Click { add });
        say(state, out);
    }
    let mut dims = state.shared.dimensions(s);
    if !dims.is_empty() {
        let anchor = pointer.unwrap_or(rect.center());
        state.bar.set_host(Some(response.id));
        for e in state.bar.show(ui.ctx(), anchor, rect, &mut dims) {
            let out = match e {
                DimEvent::Typed { key, value } => state.shared.feed(s, Input::Typed { key, value }),
                DimEvent::Cleared { key } => state.shared.feed(s, Input::Cleared { key }),
                DimEvent::Confirm => state.shared.feed(s, Input::Confirm),
                DimEvent::Escape => {
                    state.shared.escape(s);
                    Outcome::Continue
                }
                DimEvent::Focused { .. } => Outcome::Continue,
            };
            say(state, out);
        }
    }
    if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.any()) {
        let out = state.shared.feed(s, Input::Confirm);
        say(state, out);
    }
    if std::mem::take(&mut state.shared_escape) {
        state.shared.escape(s);
    }
    let preview = state.shared.preview(s);
    for stroke in &preview.strokes {
        painter.add(egui::Shape::line(stroke.iter().map(|p| map(*p)).collect(), Stroke::new(1.0, theme::INFO)));
    }
    for stroke in &preview.ghost {
        painter.add(egui::Shape::line(stroke.iter().map(|p| map(*p)).collect(), Stroke::new(2.0, theme::INFO)));
    }
    for m in &preview.marks {
        painter.circle_stroke(map(*m), 5.0, Stroke::new(1.5, theme::INFO));
    }
    let words = if preview.caption.is_empty() { state.shared.prompt() } else { format!("{} · {}", state.shared.prompt(), preview.caption) };
    painter.text(rect.left_top() + vec2(12.0, 12.0), egui::Align2::LEFT_TOP, words, egui::FontId::proportional(12.0), theme::TEXT);
}

fn finish_polyline(s: &mut Sketch, state: &mut CadState, closed: bool) {
    let points = std::mem::take(&mut state.pending);
    let id = s.entity(Geometry::Polyline { points, closed });
    if let Some(e) = s.entities.iter_mut().find(|e| e.id == id) {
        e.construction = state.construction;
    }
}
