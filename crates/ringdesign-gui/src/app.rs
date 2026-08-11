//! Application state and the background rebuild pipeline.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastReport};
use ringdesign_core::field::{Layer, LayerEntry};
use ringdesign_core::mesh::{BuildParams, BuildResult};
use ringdesign_core::{RingDesign, library};

use crate::alpha_editor::AlphaEditor;
use crate::mcp_host::McpHost;
use crate::dock::Dock;
use crate::history::History;
use crate::pane::{Layout, Pane, PaneKind};
use crate::viewport::GpuMeshRenderer;

pub const DESIGN_STORAGE_KEY: &str = "ring_design";
pub const DOCK_STORAGE_KEY: &str = "panel_dock";
pub const WORKSPACE_STORAGE_KEY: &str = "workspace";

/// Everything about the working environment that should survive a restart —
/// the design and dock already do; this carries the rest.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Workspace {
    pub preview_params: BuildParams,
    pub export_params: BuildParams,
    pub show_wireframe: bool,
    pub show_grid: bool,
    #[serde(default = "default_true")]
    pub show_gems: bool,
    /// Index into `METALS` to cut exports oversize for, or none for nominal.
    #[serde(default)]
    pub shrink_metal: Option<usize>,
    #[serde(default)]
    pub as_cast: bool,
    #[serde(default)]
    pub finish: usize,
    #[serde(default)]
    pub light: usize,
    pub layout: Layout,
    pub panes: Vec<Pane>,
    pub active_pane: usize,
    pub mcp_port: u16,
}

fn default_true() -> bool {
    true
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            preview_params: BuildParams {
                theta_steps: 384,
                profile_steps: 144,
                ..Default::default()
            },
            export_params: BuildParams {
                theta_steps: 1024,
                profile_steps: 320,
                ..Default::default()
            },
            show_wireframe: false,
            show_grid: true,
            show_gems: true,
            shrink_metal: None,
            as_cast: false,
            finish: 0,
            light: 0,
            layout: Layout::Single,
            panes: Pane::defaults(),
            active_pane: 0,
            mcp_port: ringdesign_mcp::DEFAULT_PORT,
        }
    }
}

impl RingDesignerApp {
    pub fn workspace(&self) -> Workspace {
        Workspace {
            preview_params: self.preview_params,
            export_params: self.export_params,
            show_wireframe: self.show_wireframe,
            show_grid: self.show_grid,
            show_gems: self.show_gems,
            shrink_metal: self.shrink_metal,
            as_cast: self.as_cast,
            finish: self.finish,
            light: self.light,
            layout: self.layout,
            panes: self.panes.clone(),
            active_pane: self.active_pane,
            mcp_port: self.mcp_port,
        }
    }
}

/// Quiet period after the last edit before a rebuild fires.
const DEBOUNCE: Duration = Duration::from_millis(90);

/// Longest edge of an uploaded alpha preview texture.
const THUMB_TEXTURE_EDGE: usize = 128;

pub struct RingDesignerApp {
    pub design: RingDesign,
    pub lib: Arc<AlphaLibrary>,

    pub build: Option<Arc<BuildResult>>,
    pub cast: Option<CastReport>,
    pub field: Option<ringdesign_core::castability::FieldReport>,
    pub stones: Option<ringdesign_core::stones::StonesReport>,

    /// Resolution used for the interactive viewport.
    pub preview_params: BuildParams,
    /// Resolution used when writing a file.
    pub export_params: BuildParams,

    pub renderer: Arc<Mutex<GpuMeshRenderer>>,
    pub show_wireframe: bool,
    pub show_grid: bool,
    /// Stone previews in the viewport — render only, never in the mesh.
    pub show_gems: bool,
    /// Export patterns oversize for this metal's shrink, or nominal.
    pub shrink_metal: Option<usize>,
    /// Soften the preview at the sand's detail radius — see the pour early.
    pub as_cast: bool,
    /// Index into [`viewport::FINISHES`].
    pub finish: usize,
    /// Index into [`viewport::LIGHT_RIGS`].
    pub light: usize,

    /// One per quadrant, whatever the layout currently shows.
    pub panes: Vec<Pane>,
    pub layout: Layout,
    /// Pane the toolbar's view controls act on.
    pub active_pane: usize,
    /// Frame the next completed build. Set on new/open, never on rebuilds.
    pub fit_pending: bool,
    /// Where each tool panel is docked, and how tall.
    pub dock: Dock,
    pub selected_layer: Option<usize>,
    pub library_filter: String,
    /// Clip-and-tile window for harvesting a fragment out of an imported alpha.
    pub alpha_editor: AlphaEditor,
    /// Inscriptions window, toggled from the library panel.
    pub text_editor_open: bool,
    pub status: String,
    pub auto_rebuild: bool,
    /// Named undo timeline over the design.
    pub history: History,

    /// Embedded MCP server, `None` until the user starts it.
    pub mcp: Option<McpHost>,
    pub mcp_port: u16,
    /// Why the last start attempt failed to bind.
    pub mcp_error: Option<String>,

    egui_ctx: egui::Context,
    thumbs: HashMap<String, egui::TextureHandle>,
    worker: Worker,
    dirty_at: Option<Instant>,
    in_flight: bool,
    generation: u64,
}

impl RingDesignerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut lib = AlphaLibrary::builtin();
        for dir in library::alpha_dirs() {
            match lib.load_dir(&dir) {
                Ok(n) if n > 0 => log::info!("loaded {n} alphas from {}", dir.display()),
                Ok(_) => {}
                Err(e) => log::warn!("alpha library {}: {e}", dir.display()),
            }
        }

        let design = cc
            .storage
            .and_then(|s| s.get_string(DESIGN_STORAGE_KEY))
            .and_then(|j| serde_json::from_str::<RingDesign>(&j).ok())
            .unwrap_or_default();
        design.bake_drawn(&mut lib);
        design.bake_texts(&mut lib);

        let workspace = cc
            .storage
            .and_then(|s| s.get_string(WORKSPACE_STORAGE_KEY))
            .and_then(|j| serde_json::from_str::<Workspace>(&j).ok());
        // A restored workspace keeps its cameras; a fresh start frames the
        // first build.
        let fit_pending = workspace.is_none();
        let ws = workspace.unwrap_or_default();

        let design_for_history = design.clone();
        let mut app = Self {
            design,
            lib: Arc::new(lib),
            build: None,
            cast: None,
            field: None,
            stones: None,
            preview_params: ws.preview_params,
            export_params: ws.export_params,
            renderer: Arc::new(Mutex::new(GpuMeshRenderer::default())),
            show_wireframe: ws.show_wireframe,
            show_gems: ws.show_gems,
            shrink_metal: ws.shrink_metal,
            as_cast: ws.as_cast,
            show_grid: ws.show_grid,
            finish: ws.finish,
            light: ws.light,
            panes: ws.panes,
            layout: ws.layout,
            active_pane: ws.active_pane,
            fit_pending,
            dock: cc
                .storage
                .and_then(|s| s.get_string(DOCK_STORAGE_KEY))
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default(),
            selected_layer: None,
            library_filter: String::new(),
            alpha_editor: AlphaEditor::default(),
            text_editor_open: false,
            status: "Ready".into(),
            auto_rebuild: true,
            history: History::new(&design_for_history),

            mcp: None,
            mcp_port: ws.mcp_port,
            mcp_error: None,
            egui_ctx: cc.egui_ctx.clone(),
            thumbs: HashMap::new(),
            worker: Worker::spawn(),
            dirty_at: None,
            in_flight: false,
            generation: 0,
        };
        app.mark_dirty();
        app
    }

    /// Queue a rebuild after the debounce window and publish the design to the
    /// MCP engine.
    pub fn mark_dirty(&mut self) {
        self.dirty_at = Some(Instant::now());
        // Only notes that something moved; the snapshot waits for the edit to
        // settle, so one slider drag is one history entry.
        self.history.touch();
        if let Some(host) = self.mcp.as_mut() {
            host.push(&self.design);
        }
    }

    /// Queue a rebuild without pushing the design back to the MCP engine.
    fn queue_rebuild(&mut self) {
        self.dirty_at = Some(Instant::now());
        self.history.touch();
    }

    pub fn is_building(&self) -> bool {
        self.in_flight
    }

    pub fn wants_repaint(&self) -> bool {
        self.in_flight || (self.auto_rebuild && self.dirty_at.is_some())
    }

    /// Poll the worker, fire debounced rebuilds, and refresh the section slice.
    pub fn tick(&mut self, ctx: &egui::Context) {
        if self.mcp.as_mut().is_some_and(|h| h.poll(&mut self.design)) {
            if self.selected_layer.is_some_and(|i| i >= self.design.layers.layers.len()) {
                self.selected_layer = None;
            }
            self.status = "Design edited over MCP".into();
            self.queue_rebuild();
        }

        match self.worker.done.try_recv() {
            Ok(mut done) => {
                self.in_flight = false;
                if done.generation == self.generation {
                    let r = &done.result.report;
                    self.status = match r.refine {
                        Some(s) => format!(
                            "{} tris • within {:.3} mm • {:.2} mm³ • {} ms",
                            r.validation.triangle_count,
                            s.worst_error_mm,
                            r.volume_mm3,
                            r.build_ms
                        ),
                        None => format!(
                            "{} tris • {:.2} mm³ • {} ms",
                            r.validation.triangle_count, r.volume_mm3, r.build_ms
                        ),
                    };
                    if let Ok(mut r) = self.renderer.lock() {
                        r.prepare_upload(
                            &done.result.mesh,
                            Some(&done.cast),
                            (self.design.inner_radius_mm(), self.design.draft.min_section_mm),
                        );
                        r.prepare_gems(std::mem::take(&mut done.gems));
                    }
                    // Fit only on the first build of a design; a rebuild that
                    // re-framed the view would stomp the user's own framing
                    // on every edit.
                    if self.fit_pending {
                        self.fit_pending = false;
                        let bounds = done.result.mesh.bounds();
                        for pane in &mut self.panes {
                            pane.camera.fit(bounds);
                        }
                    }
                    self.build = Some(Arc::new(done.result));
                    self.cast = Some(done.cast);
                    self.field = Some(done.field);
                    self.stones = done.stones;
                    self.refresh_sections();
                    ctx.request_repaint();
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.status = "Build worker stopped".into();
            }
        }

        // Diffed against the last committed design, so an edit is recorded
        // however it arrived — a panel, an MCP client, a loaded file.
        let design = self.design.clone();
        self.history.commit_if_settled(&design);

        if let Some(at) = self.dirty_at {
            if self.auto_rebuild && !self.in_flight && at.elapsed() >= DEBOUNCE {
                self.dirty_at = None;
                self.dispatch(self.preview_params);
            }
        }
    }

    /// Force a rebuild now, ignoring the debounce.
    pub fn rebuild_now(&mut self) {
        self.dirty_at = None;
        self.dispatch(self.preview_params);
    }

    fn dispatch(&mut self, params: BuildParams) {
        self.generation += 1;
        self.in_flight = true;
        self.status = "Building…".into();
        let mut params = params;
        // As-cast: the preview mushes at the sand's own detail radius.
        // Display only — exports build from their own params.
        if self.as_cast {
            params.soften_mm = self.design.draft.min_detail_mm;
        }
        let job = Job {
            generation: self.generation,
            design: self.design.clone(),
            lib: self.lib.clone(),
            params,
        };
        if self.worker.jobs.send(job).is_err() {
            self.in_flight = false;
            self.status = "Build worker stopped".into();
        }
    }

    /// Build synchronously at export resolution, for writing a file.
    pub fn build_for_export(&self) -> BuildResult {
        ringdesign_core::mesh::build(&self.design, &self.lib, self.export_params)
    }

    /// Reslice every pane showing a cross-section.
    pub fn refresh_sections(&mut self) {
        for i in 0..self.panes.len() {
            if self.panes[i].kind == PaneKind::Section {
                self.refresh_section(i);
            }
        }
    }

    pub fn refresh_section(&mut self, pane: usize) {
        let Some(theta) = self.panes.get(pane).map(|p| p.section_theta_deg) else { return };
        let steps = self.preview_params.profile_steps.max(128);
        let s = castability::section_at(&self.design, &self.lib, theta, steps);
        if let Some(p) = self.panes.get_mut(pane) {
            p.section = Some(s);
        }
    }

    /// Show `kind` in the active pane, so a control elsewhere can bring a view
    /// up without guessing which quadrant the user is looking at.
    pub fn focus(&mut self, kind: PaneKind) {
        let i = self.active_pane.min(self.panes.len().saturating_sub(1));
        if let Some(p) = self.panes.get_mut(i) {
            p.kind = kind;
        }
        if kind == PaneKind::Section {
            self.refresh_section(i);
        }
    }

    // --- History -----------------------------------------------------------

    pub fn undo(&mut self) {
        if let Some(d) = self.history.undo() {
            self.apply_history(d, "Undo");
        }
    }

    pub fn redo(&mut self) {
        if let Some(d) = self.history.redo() {
            self.apply_history(d, "Redo");
        }
    }

    pub fn jump_history(&mut self, index: usize) {
        if let Some(d) = self.history.jump_to(index) {
            self.apply_history(d, "History");
        }
    }

    /// Take a design back off the timeline. Goes around `mark_dirty` so the
    /// restore is not itself recorded as an edit.
    fn apply_history(&mut self, design: RingDesign, what: &str) {
        self.design = design;
        if self.selected_layer.is_some_and(|i| i >= self.design.layers.layers.len()) {
            self.selected_layer = None;
        }
        self.status = format!("{what}: {}", self.history.undo_label().unwrap_or("start"));
        self.dirty_at = Some(Instant::now());
        if let Some(host) = self.mcp.as_mut() {
            host.push(&self.design);
        }
    }

    // --- Layer stack -------------------------------------------------------

    pub fn add_layer(&mut self, name: impl Into<String>, layer: Layer) {
        self.design.layers.layers.push(LayerEntry::new(name, layer));
        self.selected_layer = Some(self.design.layers.layers.len() - 1);
        self.mark_dirty();
    }

    pub fn remove_layer(&mut self, i: usize) {
        if i < self.design.layers.layers.len() {
            self.design.layers.layers.remove(i);
            self.selected_layer = None;
            self.mark_dirty();
        }
    }

    pub fn move_layer(&mut self, i: usize, delta: isize) {
        let n = self.design.layers.layers.len();
        let j = i as isize + delta;
        if i < n && j >= 0 && (j as usize) < n {
            self.design.layers.layers.swap(i, j as usize);
            self.selected_layer = Some(j as usize);
            self.mark_dirty();
        }
    }

    pub fn duplicate_layer(&mut self, i: usize) {
        if let Some(e) = self.design.layers.layers.get(i).cloned() {
            let mut copy = e;
            copy.name = format!("{} copy", copy.name);
            self.design.layers.layers.insert(i + 1, copy);
            self.selected_layer = Some(i + 1);
            self.mark_dirty();
        }
    }

    // --- Alpha library -----------------------------------------------------

    pub fn library_mut(&mut self) -> &mut AlphaLibrary {
        Arc::make_mut(&mut self.lib)
    }

    /// Cached grayscale preview texture for an alpha.
    pub fn thumbnail(&mut self, ctx: &egui::Context, name: &str) -> Option<egui::TextureId> {
        if let Some(t) = self.thumbs.get(name) {
            return Some(t.id());
        }
        let alpha = self.lib.get(name)?;
        // Downscaled first: the grid draws these at a few dozen pixels, and a
        // full-resolution copy costs a large transient plus the VRAM it lands in.
        let (tw, th, bytes) = alpha.thumbnail_rgba8(THUMB_TEXTURE_EDGE);
        if tw == 0 || th == 0 {
            return None;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([tw, th], &bytes);
        let handle = ctx.load_texture(
            format!("alpha:{name}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        let id = handle.id();
        self.thumbs.insert(name.to_string(), handle);
        Some(id)
    }

    pub fn forget_thumbnail(&mut self, name: &str) {
        self.thumbs.remove(name);
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    // --- MCP server --------------------------------------------------------

    /// Serve the live design over MCP on `mcp_port`, seeded with the design.
    pub fn start_mcp(&mut self) {
        match McpHost::start(&self.design, self.mcp_port, self.egui_ctx.clone()) {
            Ok(host) => {
                self.status = format!("MCP server on http://{}/", host.addr());
                self.mcp_error = None;
                self.mcp = Some(host);
            }
            Err(e) => {
                log::warn!("MCP server failed to start on port {}: {e}", self.mcp_port);
                self.status = format!("MCP server failed: {e}");
                self.mcp_error = Some(e.to_string());
            }
        }
    }

    /// Drop the server, closing the listener.
    pub fn stop_mcp(&mut self) {
        if self.mcp.take().is_some() {
            self.status = "MCP server stopped".into();
        }
    }

    pub fn mcp_addr(&self) -> Option<std::net::SocketAddr> {
        self.mcp.as_ref().map(|h| h.addr())
    }
}

// --- Background build worker -----------------------------------------------

struct Job {
    generation: u64,
    design: RingDesign,
    lib: Arc<AlphaLibrary>,
    params: BuildParams,
}

struct Done {
    generation: u64,
    result: BuildResult,
    cast: CastReport,
    field: ringdesign_core::castability::FieldReport,
    stones: Option<ringdesign_core::stones::StonesReport>,
    gems: Vec<f32>,
}

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<Done>,
}

impl Worker {
    fn spawn() -> Self {
        let (jobs_tx, jobs_rx) = channel::<Job>();
        let (done_tx, done_rx) = channel::<Done>();
        std::thread::Builder::new()
            .name("ring-build".into())
            .spawn(move || {
                while let Ok(mut job) = jobs_rx.recv() {
                    // Skip stale work: only the newest queued job matters.
                    while let Ok(newer) = jobs_rx.try_recv() {
                        job = newer;
                    }
                    let result = ringdesign_core::mesh::build(&job.design, &job.lib, job.params);
                    let cast = castability::analyze(
                        &result.mesh,
                        &job.design.draft,
                        job.design.inner_radius_mm(),
                    );
                    // The verdict itself comes from the surface, at a fixed
                    // sampling so it cannot wobble with preview quality; any
                    // undercut arrives located and blamed.
                    let field = castability::attributed_field_report(
                        &job.design,
                        &job.lib,
                        &job.design.draft,
                        192,
                        128,
                    );
                    let stones = ringdesign_core::stones::report(&job.design, field.parting_z_mm);
                    let gems = crate::gems::preview_vertices(&job.design, &job.lib);
                    if done_tx
                        .send(Done { generation: job.generation, result, cast, field, stones, gems })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("spawn build worker");
        Self { jobs: jobs_tx, done: done_rx }
    }
}
