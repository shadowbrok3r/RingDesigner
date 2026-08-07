//! Application state and the background rebuild pipeline.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastReport, Section};
use ringdesign_core::field::{Layer, LayerEntry};
use ringdesign_core::mesh::{BuildParams, BuildResult};
use ringdesign_core::{RingDesign, library};

use crate::alpha_editor::AlphaEditor;
use crate::camera::OrbitCamera;
use crate::mcp_host::McpHost;
use crate::viewport::{GpuMeshRenderer, ShadeMode};

pub const DESIGN_STORAGE_KEY: &str = "ring_design";

/// Quiet period after the last edit before a rebuild fires.
const DEBOUNCE: Duration = Duration::from_millis(90);

/// Longest edge of an uploaded alpha preview texture.
const THUMB_TEXTURE_EDGE: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Solid,
    Unrolled,
    Section,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[Tab::Solid, Tab::Unrolled, Tab::Section];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Solid => "Ring",
            Tab::Unrolled => "Tile Layout",
            Tab::Section => "Cross Section",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tab::Solid => egui_phosphor::regular::CIRCLE_NOTCH,
            Tab::Unrolled => egui_phosphor::regular::GRID_FOUR,
            Tab::Section => egui_phosphor::regular::CHART_LINE,
        }
    }
}

pub struct RingDesignerApp {
    pub design: RingDesign,
    pub lib: Arc<AlphaLibrary>,

    pub build: Option<Arc<BuildResult>>,
    pub cast: Option<CastReport>,
    pub section: Option<Section>,
    pub section_theta_deg: f64,

    /// Resolution used for the interactive viewport.
    pub preview_params: BuildParams,
    /// Resolution used when writing a file.
    pub export_params: BuildParams,

    pub camera: OrbitCamera,
    pub renderer: Arc<Mutex<GpuMeshRenderer>>,
    pub shade: ShadeMode,
    pub show_wireframe: bool,
    pub show_grid: bool,

    pub tab: Tab,
    pub selected_layer: Option<usize>,
    pub library_filter: String,
    /// Clip-and-tile window for harvesting a fragment out of an imported alpha.
    pub alpha_editor: AlphaEditor,
    pub status: String,
    pub auto_rebuild: bool,

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

        let mut app = Self {
            design,
            lib: Arc::new(lib),
            build: None,
            cast: None,
            section: None,
            section_theta_deg: ringdesign_core::profile::TOP_DEG,
            preview_params: BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() },
            export_params: BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() },
            camera: OrbitCamera::default(),
            renderer: Arc::new(Mutex::new(GpuMeshRenderer::default())),
            shade: ShadeMode::Metal,
            show_wireframe: false,
            show_grid: true,
            tab: Tab::Solid,
            selected_layer: None,
            library_filter: String::new(),
            alpha_editor: AlphaEditor::default(),
            status: "Ready".into(),
            auto_rebuild: true,
            mcp: None,
            mcp_port: ringdesign_mcp::DEFAULT_PORT,
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
        if let Some(host) = self.mcp.as_mut() {
            host.push(&self.design);
        }
    }

    /// Queue a rebuild without pushing the design back to the MCP engine.
    fn queue_rebuild(&mut self) {
        self.dirty_at = Some(Instant::now());
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
            Ok(done) => {
                self.in_flight = false;
                if done.generation == self.generation {
                    self.status = format!(
                        "{} tris • {:.2} mm³ • {} ms",
                        done.result.report.validation.triangle_count,
                        done.result.report.volume_mm3,
                        done.result.report.build_ms
                    );
                    if let Ok(mut r) = self.renderer.lock() {
                        r.prepare_upload(&done.result.mesh, Some(&done.cast));
                    }
                    self.camera.fit(done.result.mesh.bounds());
                    self.build = Some(Arc::new(done.result));
                    self.cast = Some(done.cast);
                    self.refresh_section();
                    ctx.request_repaint();
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.status = "Build worker stopped".into();
            }
        }

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

    pub fn refresh_section(&mut self) {
        self.section = Some(castability::section_at(
            &self.design,
            &self.lib,
            self.section_theta_deg,
            self.preview_params.profile_steps.max(128),
        ));
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
                    if done_tx.send(Done { generation: job.generation, result, cast }).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn build worker");
        Self { jobs: jobs_tx, done: done_rx }
    }
}
