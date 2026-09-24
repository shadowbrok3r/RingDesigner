//! Application state and the background rebuild pipeline.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::ghost::GhostJudge;
use ringdesign_core::castability::{self, CastReport};
use ringdesign_core::field::{Layer, LayerEntry};
use ringdesign_core::interaction::pick::PickScene;
use ringdesign_core::mesh::{BuildParams, BuildResult};
use ringdesign_core::{RingDesign, library};
use ringdesign_workbench::command::BandSurface;
use ringdesign_workbench::render::{self, StagedEdges};
use ringdesign_workbench::viewport::Selection;
use ringdesign_workbench::viewport::pins::Pin;
use ringdesign_graph::eval::{Evaluator, evaluate_design};
use ringdesign_graph::graph::{Graph, GraphError, NodeId as GraphNodeId};
use ringdesign_graph::registry::Registry;
use ringdesign_graph_ui::Editor;

use crate::alpha_editor::AlphaEditor;
use crate::dock::{Dock, Desktop, DesktopLayout};
use ringdesign_core::history::History;
use crate::mcp_host::McpHost;
use crate::pane::{Layout, Pane, PaneKind, ViewportLayout};
use crate::viewport::GpuMeshRenderer;

pub const DESIGN_STORAGE_KEY: &str = "ring_design";
pub const DOCK_STORAGE_KEY: &str = "panel_dock";
pub const WORKSPACE_STORAGE_KEY: &str = "workspace";

/// Everything about the working environment that should survive a restart —
/// the design and dock already do; this carries the rest.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Workspace {
    #[serde(default = "default_true")]
    pub automatic_updates: bool,
    #[serde(default)]
    pub desktop: Desktop,
    #[serde(default)]
    pub desktops: BTreeMap<Desktop, DesktopLayout>,
    pub preview_params: BuildParams,
    pub export_params: BuildParams,
    pub show_wireframe: bool,
    pub show_grid: bool,
    #[serde(default = "default_true")]
    pub show_gems: bool,
    /// Every CAD part's edges over the metal: the Display menu's Part edges.
    #[serde(default = "default_true")]
    pub show_part_edges: bool,
    /// The work planes drawn over the ring.
    #[serde(default = "default_true")]
    pub show_work_planes: bool,
    /// Resolve made settings into the preview as they are edited.
    #[serde(default = "default_true")]
    pub live_cuts: bool,
    /// Draw the seats' cutters over the ring as a ghost.
    #[serde(default)]
    pub show_cutters: bool,
    /// Index into `METALS` to cut exports oversize for, or none for nominal.
    #[serde(default)]
    pub shrink_metal: Option<usize>,
    #[serde(default)]
    pub as_cast: bool,
    #[serde(default)]
    pub finish: usize,
    #[serde(default)]
    pub polish: usize,
    #[serde(default)]
    pub light: usize,
    /// Design files opened or saved, newest first.
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub document_path: Option<std::path::PathBuf>,
    pub layout: Layout,
    #[serde(default)]
    pub viewport_layout: Option<ViewportLayout>,
    #[serde(default)]
    pub graph_inline_edit: bool,
    pub panes: Vec<Pane>,
    pub active_pane: usize,
    pub mcp_port: u16,
    /// Pins a workspace kept by design file before the design carried them, an unsaved design's under ""; each moves into its design when that is opened.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pins: BTreeMap<String, Vec<Pin>>,
}

fn default_true() -> bool {
    true
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            automatic_updates: true,
            desktop: Desktop::default(),
            desktops: BTreeMap::new(),
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
            show_part_edges: true,
            show_work_planes: true,
            live_cuts: true,
            show_cutters: false,
            shrink_metal: None,
            as_cast: false,
            finish: 0,
            polish: 0,
            light: 0,
            recent: Vec::new(),
            document_path: None,
            layout: Layout::Single,
            viewport_layout: None,
            graph_inline_edit: false,
            panes: Pane::defaults(),
            active_pane: 0,
            mcp_port: ringdesign_mcp::DEFAULT_PORT,
            pins: BTreeMap::new(),
        }
    }
}

impl RingDesignerApp {
    pub fn workspace(&self) -> Workspace {
        Workspace {
            automatic_updates: self.updater.automatic,
            desktop: self.desktop,
            desktops: self.desktops.clone(),
            preview_params: self.preview_params,
            export_params: self.export_params,
            show_wireframe: self.show_wireframe,
            show_grid: self.show_grid,
            show_gems: self.show_gems,
            show_part_edges: self.show_part_edges,
            show_work_planes: !self.command.planes.hidden,
            live_cuts: self.live_cuts,
            show_cutters: self.show_cutters,
            shrink_metal: self.shrink_metal,
            as_cast: self.as_cast,
            finish: self.finish,
            polish: self.polish,
            light: self.light,
            recent: self.recent.clone(),
            document_path: self.document_path.clone(),
            layout: self.layout,
            viewport_layout: Some(self.viewport_layout.clone()),
            graph_inline_edit: self.graph_inline_edit,
            panes: self.panes.clone(),
            active_pane: self.active_pane,
            mcp_port: self.mcp_port,
            pins: self.legacy_pins.clone(),
        }
    }

    /// The pins of the design in hand.
    pub fn pins(&self) -> &[Pin] {
        &self.design.pins
    }

    /// The pins of the design in hand, to add to or clear: an edit of the design, settled into the history like any other.
    pub fn pins_mut(&mut self) -> &mut Vec<Pin> {
        self.history.touch();
        &mut self.design.pins
    }

    /// The key a workspace kept a design file's pins under.
    fn pin_key(&self) -> String {
        self.document_path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Moves the pins the workspace kept for the design's file into the design, when it was just opened and carries none of its own.
    fn carry_legacy_pins(&mut self) {
        self.pins_path = self.document_path.clone();
        let Some(pins) = self.legacy_pins.remove(&self.pin_key()) else { return };
        // A design carried on under a new file name keeps its own, and the file it replaced takes its pins with it.
        if pins.is_empty() || !self.design.pins.is_empty() || self.history.can_undo() || self.history.can_redo() {
            return;
        }
        self.design.pins = pins;
        // They were always this design's: the opened file is where the timeline starts.
        self.history.reset(&self.design);
    }

    /// The design as the session keeps it: versioned by the library's writer, or a refused newer one kept as it came until the design is edited.
    pub fn session_design(&self) -> anyhow::Result<String> {
        if let Some(kept) = &self.refused_session {
            return Ok(kept.clone());
        }
        let mut design = self.design.clone();
        design.embed_alphas(&self.lib);
        library::design_json(&design)
    }
}

/// The status line's word on a session design the library's ladder refused, naming it.
fn refused(text: &str, error: &anyhow::Error) -> String {
    let name = serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v| v.get("name")?.as_str().map(str::to_owned));
    let name = name.map_or_else(|| "The last session's design".to_owned(), |n| format!("The last session's design \"{n}\""));
    format!("{name} did not reopen: {error:#}. A new design is open; the session keeps the old one until you edit.")
}

/// Per-gram metal prices, read from `prices.json` beside the designs
/// folder: `{"Silver 925": 1.2, "Gold 14k": 55.0}`.
fn load_prices() -> std::collections::HashMap<String, f64> {
    let path = library::default_design_dir().with_file_name("prices.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Quiet period after the last edit before a rebuild fires.
const DEBOUNCE: Duration = Duration::from_millis(90);

/// How often a part being read is looked for while nothing else asks for a frame.
const IMPORT_POLL: Duration = Duration::from_millis(100);

/// A part file read: the stored feature and what the reader noted, or why it made none.
pub type PartRead = Result<(ringdesign_core::cad::Feature, Vec<String>), String>;

/// A part file being read on a thread of its own.
pub struct PendingImport {
    /// The file's name.
    pub file: String,
    /// What reads it.
    pub reader: &'static str,
    pub started: Instant,
    answer: Receiver<PartRead>,
}

impl PendingImport {
    /// What the status line says while it is read.
    pub fn words(&self) -> String {
        format!("Reading {} in {}… {:.1} s — Cancel import stops it", self.file, self.reader, self.started.elapsed().as_secs_f64())
    }
}

/// Longest edge of an uploaded alpha preview texture.
const THUMB_TEXTURE_EDGE: usize = 128;

pub struct RingDesignerApp {
    pub updater: crate::updater::Updater,
    pub install_update: bool,
    pub desktop: Desktop,
    pub desktops: BTreeMap<Desktop, DesktopLayout>,
    pub design: RingDesign,
    pub visual: ringdesign_workbench::visual::Visual,
    pub construction: ringdesign_workbench::construction::Guide,
    pub mould_renderer: Arc<Mutex<GpuMeshRenderer>>,
    pub mould_serial: u64,
    pub mould_camera: Option<(usize,crate::camera::OrbitCamera)>,
    pub lib: Arc<AlphaLibrary>,

    pub build: Option<Arc<BuildResult>>,
    /// The pick scene over `build`, rebuilt with it; what the Ring viewport hovers and selects through.
    pub pick_scene: Option<Arc<PickScene>>,
    /// What the Ring viewport has chosen and is hovering.
    pub selection: Selection,
    /// The Ring viewport's command session, its dimension bar and box select.
    pub command: crate::command::CommandState,
    /// The sketch being drawn in the Ring viewport, if any.
    pub sketch: crate::sketch_mode::SketchMode,
    pub cast: Option<CastReport>,
    pub field: Option<ringdesign_core::castability::FieldReport>,
    pub stones: Option<ringdesign_core::stones::StonesReport>,
    /// Slowest-freezing slice, from the settled build's Chvorinov scan.
    pub hot_spot: Option<(f64, f64)>,
    pub casting: crate::panels::casting::CastingState,
    pub cad: crate::panels::cad::CadState,

    /// Resolution used for the interactive viewport.
    pub preview_params: BuildParams,
    /// Resolution used when writing a file.
    pub export_params: BuildParams,

    pub renderer: Arc<Mutex<GpuMeshRenderer>>,
    pub show_wireframe: bool,
    pub show_grid: bool,
    /// Stone previews in the viewport — render only, never in the mesh.
    pub show_gems: bool,
    /// Resolve made settings into the preview as they are edited.
    pub live_cuts: bool,
    /// Draw the seats' cutters over the ring as a ghost.
    pub show_cutters: bool,
    /// Export patterns oversize for this metal's shrink, or nominal.
    pub shrink_metal: Option<usize>,
    /// Soften the preview at the sand's detail radius — see the pour early.
    pub as_cast: bool,
    /// The unrolled pane paints strokes instead of dragging layers.
    pub band_paint: bool,
    /// Brush radius as a fraction of the band's circumference.
    pub brush_frac: f32,
    /// Depth scale, 0..1 of the 1.6 mm ceiling.
    pub brush_depth: f64,
    /// Feather, 0 hard to 1 soft.
    pub brush_soft: f32,
    pub brush_erase: bool,
    /// Last probe click in the 3D view: world position and its readout.
    pub hovered_node: Option<GraphNodeId>,
    pub probe: Option<([f32; 3], String)>,
    /// The pinned comparison: the design as it was when pinned. Its mesh
    /// rides the viewport as a translucent ghost; the section view overlays
    /// its outline dashed.
    pub pinned: Option<ringdesign_core::RingDesign>,
    /// The auto-pavé dialog, open with its working spec.
    pub pave_open: bool,
    /// Per-gram metal prices from `prices.json` beside the designs folder;
    /// empty when the file is absent. Weights always show either way.
    pub prices: std::collections::HashMap<String, f64>,
    /// A background export in flight: its completion message arrives here.
    pub exporting: Option<std::sync::mpsc::Receiver<String>>,
    /// A part file being read off the UI thread, applied when it lands.
    pub importing: Option<PendingImport>,
    /// The stamp whose inspector is open, by its index in the design's stamps.
    pub stamp_inspector: Option<usize>,
    /// The Ctrl+K command palette.
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_selection: usize,
    pub pave_spec: ringdesign_core::pave::PaveSpec,
    /// The user's saved cross-sections, loaded from `library::profile_dir()`.
    pub saved_profiles: Vec<(String, ringdesign_core::BandProfile)>,
    /// Name for the next "Save profile" — the box beside the button.
    pub profile_save_name: String,
    /// Index into [`viewport::FINISHES`].
    pub finish: usize,
    pub polish: usize,
    /// Index into [`viewport::LIGHT_RIGS`].
    pub light: usize,
    /// Design files opened or saved, newest first.
    pub recent: Vec<String>,
    pub document_path: Option<std::path::PathBuf>,

    /// One per quadrant, whatever the layout currently shows.
    pub panes: Vec<Pane>,
    pub layout: Layout,
    pub viewport_layout: ViewportLayout,
    pub graph_inline_edit: bool,
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
    /// Generated-texture window, toggled from the library panel.
    pub texture: crate::comfy_texture::TextureGen,
    /// Inscriptions window, toggled from the library panel.
    pub text_editor_open: bool,
    /// Parameterized-generator window, toggled from the library panel.
    pub recipe_editor_open: bool,
    pub status: String,
    pub auto_rebuild: bool,
    /// Named undo timeline over the design.
    pub history: History,
    /// The node library, built once; the worker builds its own.
    pub graph_reg: Arc<Registry>,
    /// The editor over `design.graph`, present while the design is graph-driven.
    pub graph_ed: Option<Editor>,
    /// `design.graph` as last synced into the editor.
    pub graph_json: Option<serde_json::Value>,
    /// Graph-level errors from the last evaluation, when nothing ran.
    pub graph_errors: Vec<String>,
    /// What each node reaches on the ring, as of the last evaluation.
    pub graph_effects: BTreeMap<GraphNodeId, ringdesign_graph::focus::NodeEffect>,
    /// The chosen node's highlight on the ring.
    pub node_focus: NodeFocus,
    pub selected_node: Option<GraphNodeId>,

    /// Pins an older workspace kept by design file, each moved into its design when that design is opened.
    legacy_pins: BTreeMap<String, Vec<Pin>>,
    /// The design file whose kept pins were last looked for.
    pins_path: Option<std::path::PathBuf>,
    /// Every CAD part's edges over the metal, in the Ring viewport and the CAD pane alike.
    pub show_part_edges: bool,
    /// A session design the library's ladder refused, kept as it came and written back until the design is edited.
    refused_session: Option<String>,
    /// Said on the status line again once the next build lands, where the build's own line would cover it.
    notice: Option<String>,

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
    last_build_valid: bool,
    generation: u64,
}

impl RingDesignerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut lib = AlphaLibrary::installed();

        // The session's design comes back through the files' own ladder; one it refuses is said by name and kept.
        let session = cc.storage.and_then(|s| s.get_string(DESIGN_STORAGE_KEY));
        let (design, refused_session) = match session.map(|text| (library::load_design_str(&text), text)) {
            Some((Ok(d), _)) => (d, None),
            Some((Err(e), text)) => (RingDesign::default(), Some((refused(&text, &e), text))),
            None => (RingDesign::default(), None),
        };
        // `open_design_path` unpacks first and bakes second. This path only
        // baked, so a design restored by restarting the app came back without
        // the embedded art it was saved with — the strokes and imported masks
        // simply were not in the library any more.
        design.unpack_embedded(&mut lib);
        design.bake_all(&mut lib);

        let workspace = cc
            .storage
            .and_then(|s| s.get_string(WORKSPACE_STORAGE_KEY))
            .and_then(|j| serde_json::from_str::<Workspace>(&j).ok());
        // A restored workspace keeps its cameras; a fresh start frames the
        // first build.
        let fit_pending = workspace.is_none();
        let mut ws = workspace.unwrap_or_default();
        if ws.desktop == Desktop::Graph && ws.viewport_layout.is_none() {
            ws.layout = Layout::GraphReview;
            ws.panes = crate::pane::graph_panes();
            ws.active_pane = 2;
        }
        ws.panes.resize_with(4, || Pane::defaults().remove(0));
        ws.panes.truncate(4);
        ws.active_pane = ws.active_pane.min(ws.layout.count() - 1);

        let design_for_history = design.clone();
        let mut app = Self {
            updater: crate::updater::Updater::new(ws.automatic_updates),
            install_update: false,
            desktop: ws.desktop,
            desktops: ws.desktops,
            design,
            visual: Default::default(),
            construction: Default::default(),
            mould_renderer: Arc::new(Mutex::new(GpuMeshRenderer::default())),
            mould_serial: 0,
            mould_camera: None,
            lib: Arc::new(lib),
            build: None,
            pick_scene: None,
            selection: Selection::default(),
            command: Default::default(),
            sketch: Default::default(),
            cast: None,
            field: None,
            stones: None,
            hot_spot: None,
            casting: Default::default(),
            cad: Default::default(),
            preview_params: ws.preview_params,
            export_params: ws.export_params,
            renderer: Arc::new(Mutex::new(GpuMeshRenderer::default())),
            show_wireframe: ws.show_wireframe,
            show_gems: ws.show_gems,
            live_cuts: ws.live_cuts,
            show_cutters: ws.show_cutters,
            shrink_metal: ws.shrink_metal,
            as_cast: ws.as_cast,
            band_paint: false,
            brush_frac: 0.012,
            brush_depth: 0.6,
            brush_soft: 0.35,
            brush_erase: false,
            hovered_node: None,
            probe: None,
            pinned: None,
            pave_open: false,
            prices: load_prices(),
            exporting: None,
            importing: None,
            stamp_inspector: None,
            palette_open: false,
            palette_query: String::new(),
            palette_selection: 0,
            pave_spec: ringdesign_core::pave::PaveSpec::default(),
            saved_profiles: ringdesign_core::library::list_profiles(),
            profile_save_name: String::new(),
            show_grid: ws.show_grid,
            finish: ws.finish,
            polish: ws.polish,
            light: ws.light,
            recent: ws.recent,
            document_path: ws.document_path,
            panes: ws.panes,
            layout: ws.layout,
            viewport_layout: ws.viewport_layout.filter(|v| v.valid_for(ws.layout)).unwrap_or_else(|| ViewportLayout::new(ws.layout)),
            graph_inline_edit: ws.graph_inline_edit,
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
            texture: Default::default(),
            text_editor_open: false,
            recipe_editor_open: false,
            status: "Ready".into(),
            auto_rebuild: true,
            history: History::new(&design_for_history),
            graph_reg: Arc::new(ringdesign_script::registry()),
            graph_ed: None,
            graph_json: None,
            graph_errors: Vec::new(),
            graph_effects: BTreeMap::new(),
            node_focus: NodeFocus::default(),
            selected_node: None,
            legacy_pins: ws.pins,
            pins_path: None,
            show_part_edges: ws.show_part_edges,
            refused_session: None,
            notice: None,

            mcp: None,
            mcp_port: ws.mcp_port,
            mcp_error: None,
            egui_ctx: cc.egui_ctx.clone(),
            thumbs: HashMap::new(),
            worker: Worker::spawn(),
            dirty_at: None,
            in_flight: false,
            last_build_valid: false,
            generation: 0,
        };
        app.command.planes.hidden = !ws.show_work_planes;
        app.dock.catch_up(app.desktop);
        app.restore_desktop_view();
        app.carry_legacy_pins();
        app.mark_dirty();
        if let Some((said, text)) = refused_session {
            app.status = said.clone();
            app.notice = Some(said);
            app.refused_session = Some(text);
        }
        app
    }

    /// Pin the current design as the comparison ghost, or clear it.
    pub fn toggle_pin(&mut self) {
        if self.pinned.take().is_some() {
            if let Ok(mut r) = self.renderer.lock() {
                r.prepare_ghost(Vec::new());
            }
            self.set_status("Comparison unpinned");
            return;
        }
        let out = ringdesign_core::mesh::build(&self.design, &self.lib, self.preview_params);
        if let Ok(mut r) = self.renderer.lock() {
            r.prepare_ghost(crate::viewport::GpuMeshRenderer::stage_plain(&out.mesh));
        }
        self.pinned = Some(self.design.clone());
        self.set_status("Pinned — the ghost holds this shape while you edit");
    }

    /// Record a design file at the head of the recents, newest first.
    pub fn push_recent(&mut self, path: &std::path::Path) {
        let p = path.to_string_lossy().into_owned();
        self.recent.retain(|r| r != &p);
        self.recent.insert(0, p);
        self.recent.truncate(10);
    }

    /// Reap a finished background export, if any.
    pub fn poll_export(&mut self) {
        let done = match self.exporting.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(msg) => Some(msg),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some("export thread died".into())
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            },
            None => None,
        };
        if let Some(msg) = done {
            self.exporting = None;
            self.set_status(msg);
        }
    }

    /// Reads part file `file` on a thread of its own with `read`, named by `reader`; it lands joined at the top as one History entry.
    #[cfg(any(test, feature = "kernel-occt"))]
    pub fn start_import(&mut self, file: String, reader: &'static str, read: impl FnOnce() -> PartRead + Send + 'static) {
        if let Some(p) = &self.importing {
            self.set_status(format!("{} is still being read; cancel it before importing another part", p.file));
            return;
        }
        let (tx, rx) = channel();
        let wake = self.egui_ctx.clone();
        let spawned = std::thread::Builder::new().name("part-import".into()).spawn(move || {
            let _ = tx.send(read());
            wake.request_repaint();
        });
        if let Err(e) = spawned {
            self.set_status(format!("{file} could not be read: {e}"));
            return;
        }
        self.importing = Some(PendingImport { file, reader, started: Instant::now(), answer: rx });
        self.status = self.importing.as_ref().map(PendingImport::words).unwrap_or_default();
    }

    /// Stops waiting for the part being read: what it reads is dropped when it lands.
    pub fn cancel_import(&mut self) {
        if let Some(p) = self.importing.take() {
            self.set_status(format!("Stopped reading {}: nothing was imported", p.file));
        }
    }

    /// Takes in a part that has been read, or says on the status line what is still being read.
    fn poll_import(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.importing else { return };
        match p.answer.try_recv() {
            Ok(read) => {
                self.importing = None;
                match read {
                    Ok(read) => crate::export::land_part(self, read),
                    Err(why) => self.set_status(why),
                }
            }
            Err(TryRecvError::Empty) => {
                self.status = p.words();
                crate::export::import_plate(self, ctx);
                ctx.request_repaint_after(IMPORT_POLL);
            }
            Err(TryRecvError::Disconnected) => {
                let file = p.file.clone();
                self.importing = None;
                self.set_status(format!("Reading {file} stopped without an answer: nothing was imported"));
            }
        }
    }

    /// Queue a rebuild after the debounce window and publish the design to the
    /// MCP engine.
    pub fn mark_dirty(&mut self) {
        // An edit is the user moving on from a refused session design.
        self.refused_session = None;
        self.notice = None;
        self.visual.invalidate();
        // A layer that reads a distance field and has not got one falls back
        // to brightness-as-height without a word, so turning "Crisp edge" on
        // looked like it did nothing. The check is a map lookup per
        // edge-enabled layer; the bake — which deep-copies the library
        // through `Arc::make_mut` — only runs when one is actually absent.
        if self.design.sdfs_missing(&self.lib) {
            let design = self.design.clone();
            design.bake_sdfs(self.library_mut());
        }
        // Live generator groups own their stacks: any edit that moves the
        // ground under one — the profile, the shank, the process, or the
        // recipe itself — re-solves it here, at edit time. Builds never
        // regenerate; a design file renders as saved. A recipe that no
        // longer fits refuses non-destructively and says so.
        let has_live = self.design.layers.layers.iter().any(|e| {
            matches!(&e.layer, ringdesign_core::Layer::Group(g) if g.recipe.is_some())
        });
        // A graph-driven design is re-evaluated whole on the worker; its
        // generators are nodes there, so nothing regenerates here.
        if has_live && self.design.graph.is_none() {
            for note in ringdesign_core::pave::regenerate_live(&mut self.design) {
                self.set_status(note);
            }
        }
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
        self.refused_session = None;
        self.notice = None;
        self.visual.invalidate();
        self.dirty_at = Some(Instant::now());
        self.history.touch();
    }

    pub fn is_building(&self) -> bool {
        self.in_flight
    }

    pub fn is_current(&self) -> bool { self.last_build_valid && !self.in_flight && self.dirty_at.is_none() }

    pub fn wants_repaint(&self) -> bool {
        self.in_flight || (self.auto_rebuild && self.dirty_at.is_some())
    }

    /// Poll the worker, fire debounced rebuilds, and refresh the section slice.
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.sync_graph();
        // A design opened from another file takes the pins an older workspace kept for that file.
        if self.document_path != self.pins_path {
            self.carry_legacy_pins();
        }
        if self.mcp.as_mut().is_some_and(|h| h.poll(&mut self.design)) {
            if self
                .selected_layer
                .is_some_and(|i| i >= self.design.layers.layers.len())
            {
                self.selected_layer = None;
            }
            self.status = "Design edited over MCP".into();
            self.queue_rebuild();
        }

        match self.worker.done.try_recv() {
            Ok(WorkerMsg::Failed { generation, message }) => {
                self.in_flight = false;
                if generation == self.generation {
                    self.last_build_valid = false;
                    self.set_status(format!(
                        "Build failed: {message}. The design is unchanged — the last good \
                         geometry is still on screen."
                    ));
                }
            }
            Ok(WorkerMsg::Done(mut done)) => {
                self.in_flight = false;
                if done.generation == self.generation && self.dirty_at.is_none() {
                    self.last_build_valid = done.graph.as_ref().is_none_or(|g|g.ok);
                    let r = &done.result.report;
                    self.status = match r.refine {
                        Some(s) => format!(
                            "{} tris • within {:.3} mm • {:.2} mm³ • {} ms",
                            r.validation.triangle_count, s.worst_error_mm, r.volume_mm3, r.build_ms
                        ),
                        None => format!(
                            "{} tris • {:.2} mm³ • {} ms",
                            r.validation.triangle_count, r.volume_mm3, r.build_ms
                        ),
                    };
                    if let Some(said) = self.notice.take() {
                        self.status = said;
                    }
                    let build = Arc::new(done.result);
                    // The selection's channel as the worker staged it holds while the selection lights the same as at dispatch.
                    let selected = self.selection.tints_as(&done.selection).then(|| std::mem::take(&mut done.select));
                    // The worker staged every buffer; the UI thread only hands them over for upload.
                    if let Ok(mut r) = self.renderer.lock() {
                        r.prepare_staged(std::mem::take(&mut done.metal));
                        r.prepare_edges(&build, std::mem::take(&mut done.edges));
                        r.prepare_gems(std::mem::take(&mut done.gems));
                        r.prepare_cutters(std::mem::take(&mut done.cutters));
                        if let Some(select) = selected {
                            r.prepare_select(select);
                            self.selection.staged(Arc::as_ptr(&build) as usize);
                        }
                    }
                    // Fit only on the first build of a design; a rebuild that
                    // re-framed the view would stomp the user's own framing
                    // on every edit.
                    if self.fit_pending {
                        self.fit_pending = false;
                        let bounds = build.mesh.bounds();
                        for pane in &mut self.panes {
                            pane.camera.fit(bounds);
                        }
                    }
                    self.build = Some(build);
                    crate::command::band_landed(self, done.band.take(), done.judge.take());
                    self.node_focus.mesh_generation = done.generation;
                    self.node_focus.mesh_params = Some(done.params);
                    self.visual.mesh_changed();
                    self.cast = Some(done.cast);
                    self.field = Some(done.field);
                    self.stones = done.stones;
                    self.hot_spot = done.hot_spot;
                    if let Some(gd) = done.graph {
                        if gd.ok {
                            if let Some(lib) = gd.baked_library {
                                self.lib = lib;
                                self.thumbs.clear();
                            }
                            // The evaluated design, under whatever the graph
                            // has become since the job was queued, and the pins as they stand.
                            let graph = self.design.graph.take();
                            let pins = std::mem::take(&mut self.design.pins);
                            self.design = gd.design;
                            self.design.graph = graph;
                            self.design.pins = pins;
                        }
                        self.graph_errors = gd.errors.iter().map(ToString::to_string).collect();
                        if gd.ok {
                            self.graph_effects = gd.effects;
                        }
                        if let Some(ed) = &mut self.graph_ed {
                            ed.set_values(&gd.values);
                            ed.set_diagnostics(&gd.errors, &gd.notes);
                        }
                        if !gd.ok {
                            self.status = format!("Graph: {}", self.graph_errors.join("; "));
                        }
                    }
                    // The pick scene follows the mesh on screen, over the design as evaluated.
                    self.pick_scene = Some(done.pick);
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
        self.sync_node_focus(ctx);

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
        // A part still being read holds the status line over a build's.
        self.poll_import(ctx);
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
            graph: self.design.graph.as_ref().and_then(|j| serde_json::from_value::<Graph>(j.clone()).ok()),
            live_cuts: self.live_cuts,
            show_cutters: self.show_cutters,
            selection: self.selection.clone(),
        };
        if self.worker.jobs.send(job).is_err() {
            self.in_flight = false;
            self.status = "Build worker stopped".into();
        }
    }

    /// Reslice every cross-section on screen; one out of sight is resliced by the layout that shows it.
    pub fn refresh_sections(&mut self) {
        for i in self.visible_panes() {
            if self.panes.get(i).is_some_and(|p| p.kind == PaneKind::Section) {
                self.refresh_section(i);
            }
        }
    }

    pub fn refresh_section(&mut self, pane: usize) {
        let Some(theta) = self.panes.get(pane).map(|p| p.section_theta_deg) else {
            return;
        };
        let steps = self.preview_params.profile_steps.max(128);
        let s = castability::section_at(&self.design, &self.lib, theta, steps);
        if let Some(p) = self.panes.get_mut(pane) {
            p.section = Some(s);
        }
    }

    /// Put `kind` in pane `i`.
    ///
    /// A cross-section arrives *beside* the ring rather than over it: a
    /// section is read against the shape it cuts, so asking for one from a
    /// single view splits the window and keeps the ring on the left.
    pub fn set_pane_kind(&mut self, i: usize, kind: PaneKind) {
        if kind == PaneKind::Section && self.layout == Layout::Single && self.panes.len() > 1 {
            self.set_layout(Layout::SplitH);
            self.panes[0].kind = PaneKind::Solid;
            self.panes[1].kind = PaneKind::Section;
            self.active_pane = 1;
            self.refresh_section(1);
            return;
        }
        let Some(pane) = self.panes.get_mut(i) else { return };
        pane.kind = kind;
        self.active_pane = i;
        if kind == PaneKind::Section {
            self.refresh_section(i);
        }
    }

    /// Show `kind` in the active pane, so a control elsewhere can bring a view
    /// up without guessing which quadrant the user is looking at.
    pub fn focus(&mut self, kind: PaneKind) {
        let desktop = match kind {
            PaneKind::Graph => Desktop::Graph,
            PaneKind::Unrolled => Desktop::Surface,
            PaneKind::Casting => Desktop::Casting,
            PaneKind::Cad => Desktop::Cad,
            _ => Desktop::Model,
        };
        self.switch_desktop(desktop);
        let i = self.panes.iter().take(self.layout.count()).position(|p| p.kind == kind)
            .unwrap_or(self.active_pane.min(self.layout.count() - 1).min(self.panes.len().saturating_sub(1)));
        self.active_pane = i;
        if let Some(p) = self.panes.get_mut(i) {
            p.kind = kind;
        }
        if kind == PaneKind::Section {
            self.refresh_section(i);
        }
    }

    pub fn switch_desktop(&mut self, desktop: Desktop) {
        if self.desktop == desktop { return; }
        let previous = DesktopLayout { dock: self.dock.clone(), panes: self.panes.clone(), layout: self.layout, active: self.active_pane, viewport_layout: Some(self.viewport_layout.clone()) };
        self.desktops.insert(self.desktop, previous);
        let mut next = self.desktops.remove(&desktop).unwrap_or_else(|| DesktopLayout::new(desktop));
        if desktop == Desktop::Graph && next.viewport_layout.is_none() {
            next = DesktopLayout::new(desktop);
        }
        next.dock.catch_up(desktop);
        next.panes.resize_with(4, || Pane::defaults().remove(0));
        next.panes.truncate(4);
        self.dock = next.dock;
        self.panes = next.panes;
        self.layout = next.layout;
        self.viewport_layout = next.viewport_layout.filter(|v| v.valid_for(next.layout)).unwrap_or_else(|| ViewportLayout::new(next.layout));
        self.active_pane = next.active.min(self.layout.count() - 1);
        self.desktop = desktop;
        self.restore_desktop_view();
        self.construction.open = false;
        self.hovered_node = None;
        self.node_focus.asked = None;
        self.node_focus.aim_pending = self.selected_node.is_some();
        self.band_paint = false;
        self.visual.select(ringdesign_workbench::visual::Tool::Select);
        self.refresh_sections();
    }

    /// A stored layout can have lost the workspace's own view; put it back in a visible pane.
    pub fn restore_desktop_view(&mut self) {
        let kind = self.desktop.pane();
        let shown = self.visible_panes();
        if shown.iter().any(|i| self.panes.get(*i).is_some_and(|p| p.kind == kind)) {
            return;
        }
        let Some(i) = shown.iter().copied().find(|i| *i == self.active_pane).or(shown.first().copied()) else { return };
        if let Some(pane) = self.panes.get_mut(i) {
            pane.kind = kind;
            self.active_pane = i;
        }
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
        self.viewport_layout = ViewportLayout::new(layout);
        if layout == Layout::GraphReview {
            let free_camera = self.panes.iter().find(|p| p.kind == PaneKind::Solid && !p.follow_node).map(|p| p.camera);
            self.panes = crate::pane::graph_panes();
            if let Some(camera) = free_camera { self.panes[0].camera = camera; }
            self.active_pane = 2;
            self.node_focus.asked = None;
            self.node_focus.aim_pending = self.selected_node.is_some();
        } else {
            for pane in &mut self.panes {
                if pane.follow_node { pane.follow_node = false; pane.navigation.locked = false; }
            }
            self.active_pane = self.active_pane.min(layout.count() - 1);
        }
        self.refresh_sections();
    }

    pub fn surface_edit_reason(&self) -> Option<&'static str> {
        if self.graph_driven() { Some("This design is generated by nodes. Edit its graph, or make it directly editable.") }
        else if matches!(self.desktop, Desktop::Graph | Desktop::Casting | Desktop::Cad) { Some("Surface editing is available in the Model and Surface workspaces.") }
        else if ringdesign_workbench::cad_tools::replaces_band(&self.design) { Some(ringdesign_workbench::cad_tools::PARTS_ONLY) }
        else { None }
    }

    pub fn clear_selection(&mut self) {
        // Clears Measure's picks first, keeping the tool.
        if self.visual.tool == ringdesign_workbench::visual::Tool::Measure && !self.visual.measurement.picks.is_empty() {
            self.visual.measurement.clear();
            self.set_status("Measurement cleared");
            return;
        }
        self.hovered_node = None;
        self.selection.clear();
        self.selected_node = None;
        if let Some(ed) = &mut self.graph_ed { ed.selected = None; }
        self.selected_layer = None;
        self.visual.selected_stone = None;
        self.probe = None;
        self.visual.select(ringdesign_workbench::visual::Tool::Select);
        self.set_status("Selection cleared — drag empty space to orbit");
    }

    // --- History -----------------------------------------------------------

    /// Takes back the last edit; with a viewport command live, takes back the command instead.
    pub fn undo(&mut self) {
        if crate::command::cancel(self) || crate::sketch_mode::undo(self) {
            return;
        }
        self.history.commit(&self.design);
        if let Some(d) = self.history.undo() {
            self.apply_history(d, "Undo");
        }
    }

    pub fn redo(&mut self) {
        crate::command::cancel(self);
        if crate::sketch_mode::redo(self) {
            return;
        }
        self.history.commit(&self.design);
        if let Some(d) = self.history.redo() {
            self.apply_history(d, "Redo");
        }
    }

    pub fn jump_history(&mut self, index: usize) {
        if crate::sketch_mode::active(self) {
            self.set_status("Finish or leave the sketch before stepping through the history");
            return;
        }
        crate::command::cancel(self);
        if let Some(d) = self.history.jump_to(index) {
            self.apply_history(d, "History");
        }
    }

    /// Take a design back off the timeline. Goes around `mark_dirty` so the
    /// restore is not itself recorded as an edit.
    fn apply_history(&mut self, design: RingDesign, what: &str) {
        self.design = design;
        // Strokes, inscriptions and SVG art travel in the design as source
        // data and live in the shared library as rasters. Restoring the one
        // without re-deriving the other left painted metal on the band after
        // Ctrl+Z, because the old raster was still what the layers read.
        let restored = self.design.clone();
        restored.unpack_embedded(self.library_mut());
        restored.bake_all(self.library_mut());
        if self
            .selected_layer
            .is_some_and(|i| i >= self.design.layers.layers.len())
        {
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

    /// Reorder by drag: lift `from` out and insert it at `to`.
    pub fn move_layer_to(&mut self, from: usize, to: usize) {
        let n = self.design.layers.layers.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let e = self.design.layers.layers.remove(from);
        self.design.layers.layers.insert(to, e);
        self.selected_layer = Some(to);
        self.mark_dirty();
    }

    /// Solo a layer — everything else mutes — or restore all when it is
    /// already the only one enabled.
    pub fn solo_layer(&mut self, i: usize) {
        let layers = &mut self.design.layers.layers;
        if i >= layers.len() {
            return;
        }
        let already = layers
            .iter()
            .enumerate()
            .all(|(j, e)| e.enabled == (j == i));
        for (j, e) in layers.iter_mut().enumerate() {
            e.enabled = already || j == i;
        }
        self.selected_layer = Some(i);
        self.mark_dirty();
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
        let handle = ctx.load_texture(format!("alpha:{name}"), image, egui::TextureOptions::LINEAR);
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

// --- The graph behind the design ------------------------------------------

impl RingDesignerApp {
    /// Whether the design is driven by a graph right now.
    /// Put this workspace's panels and views back where they start.
    ///
    /// One implementation for the View menu, the layout cluster and the
    /// command palette: a workspace that has been rearranged — panels docked
    /// elsewhere, views closed or dragged about — comes back in a click.
    pub fn restore_default_layout(&mut self) {
        let defaults = DesktopLayout::new(self.desktop);
        self.dock = defaults.dock;
        self.panes = defaults.panes;
        self.set_layout(defaults.layout);
        self.active_pane = defaults.active;
        self.set_status(format!("{} workspace back to its default layout", self.desktop.label()));
    }

    /// The slice a cross-section view on screen is showing, if one is.
    ///
    /// The active pane wins where several are open, so the one being read is
    /// the one marked on the ring.
    pub fn section_on_screen(&self) -> Option<&ringdesign_core::castability::Section> {
        let is_section = |i: &usize| self.panes.get(*i).is_some_and(|p| p.kind == PaneKind::Section);
        let shown = self.visible_panes();
        let at = shown
            .iter()
            .copied()
            .find(|i| *i == self.active_pane && is_section(i))
            .or_else(|| shown.iter().copied().find(is_section))?;
        self.panes.get(at)?.section.as_ref()
    }

    /// Which panes are on screen.
    ///
    /// The tree is the truth while it holds panes — a closed one must count
    /// as gone — but `panes` takes the tree out for the length of its own
    /// draw, so the preset answers for the frames it is away. Everything
    /// asking what is visible goes through here, code inside a pane included.
    pub fn visible_panes(&self) -> Vec<usize> {
        let shown = self.viewport_layout.shown();
        if shown.is_empty() { (0..self.layout.count()).collect() } else { shown }
    }

    /// Is a 3D view of the ring on screen?
    pub fn showing_a_ring(&self) -> bool {
        self.visible_panes()
            .into_iter()
            .any(|i| self.panes.get(i).is_some_and(|p| p.kind == PaneKind::Solid))
    }

    pub fn graph_driven(&self) -> bool {
        self.design.graph.is_some()
    }

    /// Keep the editor in step with `design.graph`, whichever side moved:
    /// history, MCP, a file, a template — anything that replaces the design
    /// replaces the graph the editor shows.
    pub fn sync_graph(&mut self) {
        if self.design.graph == self.graph_json {
            return;
        }
        self.graph_json = self.design.graph.clone();
        let parsed = self.design.graph.as_ref().and_then(|j| serde_json::from_value::<Graph>(j.clone()).ok());
        match parsed {
            Some(g) => {
                match &mut self.graph_ed {
                    Some(ed) => ed.set_graph(g, &self.graph_reg),
                    None => self.graph_ed = Some(Editor::new(g, &self.graph_reg)),
                }
                // A lifted graph sits on a nominal grid its nodes overflow.
                if let Some(ed) = &mut self.graph_ed {
                    ed.arrange_if_tangled();
                }
            }
            None => {
                self.graph_ed = None;
                self.selected_node = None;
            }
        }
        if let (Some(ed), Some(sel)) = (&self.graph_ed, self.selected_node) {
            if ed.node(sel).is_none() {
                self.selected_node = None;
            }
        }
    }

    /// What the chosen node does, in words: its reach, then what was measured.
    pub fn node_words(&self) -> Option<String> {
        let effect = self.graph_effects.get(&self.selected_node?)?;
        Some(if self.node_focus.words.is_empty() { effect.summary() } else { format!("{} \u{b7} {}", effect.summary(), self.node_focus.words) })
    }

    /// The node to open for the layer at `index` of the evaluated stack:
    /// the layer's own node, where its numbers are, else its entry.
    pub fn node_for_layer(&self, index: usize) -> Option<GraphNodeId> {
        let g = self.graph_ed.as_ref()?.graph();
        let entry = g.nodes.iter().filter(|n| n.kind == "entry").map(|n| n.id).find(|id| self.graph_effects.get(id).is_some_and(|e| e.layers.iter().any(|p| p.as_slice() == [index])))?;
        Some(g.wire_into(entry, "layer").map_or(entry, |w| w.from))
    }

    /// Keeps the ring's highlight in step with the chosen node: asks for one
    /// when the node or the mesh under it changes, takes the newest result,
    /// and carries the flash frame to frame.
    fn sync_node_focus(&mut self, ctx: &egui::Context) {
        let want = if self.graph_ed.is_some() { self.hovered_node.or(self.selected_node) } else { None };
        if want != self.node_focus.shown {
            self.node_focus.shown = want;
            self.node_focus.asked = None;
            self.node_focus.words.clear();
            self.node_focus.chosen_at = Some(Instant::now());
            self.node_focus.aim_pending = want.is_some() && self.hovered_node.is_none();
            if let Ok(mut r) = self.renderer.lock() {
                r.prepare_focus(Vec::new());
            }
        }
        if self.node_focus.worker.is_none() {
            let wake = ctx.clone();
            self.node_focus.worker = Some(ringdesign_workbench::focus::Worker::spawn(move || wake.request_repaint(), GpuMeshRenderer::stage_focus));
            self.node_focus.follow = true;
        }
        let mut arrived = Vec::new();
        if let Some(worker) = &self.node_focus.worker {
            while let Some(hl) = worker.poll() {
                arrived.push(hl);
            }
        }
        for hl in arrived {
            if Some((hl.node, hl.generation)) != self.node_focus.asked || want != Some(hl.node) {
                continue;
            }
            if let Some(effect) = self.graph_effects.get(&hl.node) {
                self.node_focus.words = hl.words(effect);
            }
            let first_selection = std::mem::take(&mut self.node_focus.aim_pending);
            let has_feature_view = self.panes.iter().take(self.layout.count()).any(|p| p.kind == PaneKind::Solid && p.follow_node);
            if self.hovered_node.is_none() {
                if let Some(aim) = &hl.aim {
                    for pane in self.panes.iter_mut().take(self.layout.count()).filter(|p| p.kind == PaneKind::Solid) {
                        if pane.follow_node || (!has_feature_view && first_selection && self.node_focus.follow) {
                            pane.turn = Some(ringdesign_workbench::focus::Turn::new(pane.camera.pose(), pane.camera.aimed_at(aim)));
                        }
                    }
                }
            }
            if let Ok(mut r) = self.renderer.lock() {
                r.prepare_focus(hl.staged);
            }
            ctx.request_repaint();
        }
        let Some(node) = want else {
            self.node_focus.tint = [0.0; 4];
            return;
        };
        let key = (node, self.node_focus.mesh_generation);
        let settled = self.dirty_at.is_none() && !self.in_flight;
        if settled && self.node_focus.asked != Some(key) && self.design.band_is_procedural() {
            if let (Some(worker), Some(build), Some(params), Some(effect)) = (self.node_focus.worker.as_ref(), self.build.as_ref(), self.node_focus.mesh_params, self.graph_effects.get(&node)) {
                // The graph's JSON can run to megabytes of embedded artwork,
                // and a before/after build reads none of it.
                let graph = self.design.graph.take();
                let design = self.design.clone();
                self.design.graph = graph;
                let view_yaw = self.panes.iter().find(|p| p.kind == PaneKind::Solid).map_or(0.0, |p| p.camera.yaw);
                let mesh = Arc::new(build.mesh.clone());
                if worker.request(ringdesign_workbench::focus::Request { node, generation: key.1, design, lib: self.lib.clone(), params, mesh, effect: effect.clone(), view_yaw }) {
                    self.node_focus.asked = Some(key);
                }
            }
        }
        let since = self.node_focus.chosen_at.map_or(f32::MAX, |at| at.elapsed().as_secs_f32());
        let (strength, moving) = ringdesign_workbench::focus::pulse(since);
        self.node_focus.tint = if self.hovered_node.is_some() { [0.40, 0.85, 0.83, 0.65] } else { [1.0, 0.24, 0.545, strength] };
        if moving {
            ctx.request_repaint();
        }
    }

    /// The editor moved the graph: write it into the design and rebuild.
    pub fn graph_changed(&mut self) {
        let Some(ed) = &self.graph_ed else { return };
        let json = serde_json::to_value(ed.graph()).ok();
        let moved_only = ringdesign_core::history::graph_layout_only(self.design.graph.as_ref(), json.as_ref());
        self.design.graph = json.clone();
        self.graph_json = json;
        if !moved_only {
            self.mark_dirty();
        }
    }

    /// Lift the design into a graph that evaluates back to it exactly, and
    /// show it.
    pub fn convert_to_graph(&mut self) {
        match ringdesign_graph::lift::from_design(&self.design, &self.graph_reg, &self.lib) {
            Ok(g) => {
                self.design.graph = serde_json::to_value(&g).ok();
                self.sync_graph();
                self.show_graph_pane();
                self.set_status("Converted to a graph — the panels follow it now");
                self.mark_dirty();
            }
            Err(e) => self.set_status(format!("Could not convert: {e}")),
        }
    }

    /// Open a starter or template graph as the design's graph.
    pub fn open_graph(&mut self, g: Graph) {
        self.document_path = None;
        self.design.name = g.name.clone();
        self.set_graph(g);
        self.show_graph_pane();
    }

    /// Replace the design's graph in place: the same document in the same panes.
    pub fn set_graph(&mut self, g: Graph) {
        self.design.graph = serde_json::to_value(&g).ok();
        self.sync_graph();
        self.mark_dirty();
    }

    /// Drop the graph; the design stays exactly as last evaluated.
    pub fn bake_graph(&mut self) {
        if !self.is_current() { self.set_status("Wait for a successful rebuild before baking the graph"); return; }
        if self.design.graph.take().is_some() {
            self.graph_json = None;
            self.graph_ed = None;
            self.selected_node = None;
            self.graph_errors.clear();
            self.set_status("Baked: the graph is gone and the design is yours to edit");
            self.mark_dirty();
        }
    }

    /// Make a graph pane the active one, turning the active pane into one
    /// if none shows the graph.
    pub fn show_graph_pane(&mut self) {
        self.focus(PaneKind::Graph);
        if !self.dock.is_open(crate::dock::ToolKind::Node) {
            self.dock.open_on(crate::dock::ToolKind::Node, crate::dock::Side::Right);
        }
    }

    pub fn arrange_graph(&mut self) {
        let reg = self.graph_reg.clone();
        if let Some(ed) = &mut self.graph_ed {
            ed.arrange(&reg);
        }
        self.graph_changed();
    }

    /// Jump to the node that produced the k-th layer of the stack.
    pub fn edit_in_graph(&mut self, layer: usize) {
        let Some(ed) = &mut self.graph_ed else { return };
        let entries = ed.graph().entry_nodes();
        match entries.get(layer).copied() {
            Some(id) => {
                ed.focus(id);
                self.selected_node = Some(id);
                self.show_graph_pane();
                if !self.dock.is_open(crate::dock::ToolKind::Node) {
                    self.dock.open_on(crate::dock::ToolKind::Node, crate::dock::Side::Right);
                }
            }
            None => self.set_status("No entry node for that layer; the graph builds its stack another way"),
        }
    }

    /// Fold these nodes into a cluster node.
    pub fn collapse_nodes(&mut self, ids: &[GraphNodeId]) {
        let reg = self.graph_reg.clone();
        let Some(ed) = &mut self.graph_ed else { return };
        let name = format!("Cluster {}", ed.graph().nodes.iter().filter(|n| n.kind == "cluster").count() + 1);
        match ed.collapse(ids, &name, &reg) {
            Ok(cid) => {
                self.selected_node = Some(cid);
                self.graph_changed();
            }
            Err(e) => self.set_status(format!("Could not collapse: {e}")),
        }
    }

    pub fn delete_selected_node(&mut self) {
        let Some(id) = self.selected_node.take() else { return };
        if let Some(ed) = &mut self.graph_ed {
            if ed.remove(id) {
                self.graph_changed();
            }
        }
    }
}

// --- Background build worker -----------------------------------------------

/// What a ghost's judge reads: the band's epoch, then the parting plane, the draft floor and the bore radius as bits.
type JudgeKey = (u64, u64, u64, u64);

struct Job {
    generation: u64,
    design: RingDesign,
    lib: Arc<AlphaLibrary>,
    params: BuildParams,
    /// The design's graph, evaluated before the build when present.
    graph: Option<Graph>,
    /// Resolve made settings into the preview mesh.
    live_cuts: bool,
    /// Stage the seats' cutters as a ghost.
    show_cutters: bool,
    /// The Ring viewport's selection at dispatch, whose channel is staged over the new build.
    selection: Selection,
}

/// What evaluating a job's graph produced.
pub struct GraphDone {
    pub design: RingDesign,
    /// What each node reaches on the evaluated design.
    pub effects: BTreeMap<GraphNodeId, ringdesign_graph::focus::NodeEffect>,
    pub baked_library: Option<Arc<AlphaLibrary>>,
    pub values: BTreeMap<GraphNodeId, BTreeMap<String, String>>,
    pub notes: BTreeMap<GraphNodeId, Vec<String>>,
    pub errors: Vec<GraphError>,
    /// The design above is the evaluation's; false means the last good
    /// design was built instead.
    pub ok: bool,
}

struct Done {
    generation: u64,
    /// What `result` was built with, so a before/after can be built to match.
    params: BuildParams,
    result: BuildResult,
    cast: CastReport,
    field: ringdesign_core::castability::FieldReport,
    stones: Option<ringdesign_core::stones::StonesReport>,
    /// Slowest-freezing slice: `(theta, modulus mm)` off the Chvorinov scan.
    hot_spot: Option<(f64, f64)>,
    gems: Vec<f32>,
    /// The seats' cutters as a ghost, empty unless asked for.
    cutters: Vec<f32>,
    /// The metal staged for upload: draft classes and the wall heatmap baked in, or a CAD-only ring's creases.
    metal: Vec<f32>,
    /// Every drawn part's edges staged for the edge pass.
    edges: StagedEdges,
    /// The pick scene over `result`, over the design as evaluated.
    pick: Arc<PickScene>,
    graph: Option<GraphDone>,
    /// The ring frame over the build's band, the one before it when the band did not change.
    band: Option<Arc<BandSurface>>,
    /// The carried ghost's judge over that band at the verdict's parting plane, its radial lines laid out on the ring frame's tree.
    judge: Option<Arc<GhostJudge>>,
    /// The selection the job was dispatched with, and its channel staged over the new mesh.
    selection: Selection,
    select: Vec<f32>,
}

/// The highlight a chosen graph node casts on the ring, and what it was
/// measured against.
#[derive(Default)]
pub struct NodeFocus {
    worker: Option<ringdesign_workbench::focus::Worker>,
    mesh_generation: u64,
    mesh_params: Option<BuildParams>,
    /// `(node, mesh generation)` the highlight on screen, or on its way, is for.
    asked: Option<(GraphNodeId, u64)>,
    shown: Option<GraphNodeId>,
    chosen_at: Option<Instant>,
    /// The next highlight to arrive also turns the cameras.
    aim_pending: bool,
    /// The measured half of the caption.
    pub words: String,
    /// Tint and strength for the renderer; zero strength is off.
    pub tint: [f32; 4],
    /// Turn the ring to the chosen node.
    pub follow: bool,
}

/// What comes back from the build thread.
///
/// The loop used to be unguarded, so a panic anywhere in `mesh::build`,
/// `attribute_undercuts`, `stones::report` or the gem preview killed the
/// thread; `jobs_tx.send` then failed for the rest of the session and the app
/// simply stopped rebuilding, with no error and no way back short of a
/// restart. A panic is now one failed build.
enum WorkerMsg {
    Done(Box<Done>),
    Failed { generation: u64, message: String },
}

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<WorkerMsg>,
}

impl Worker {
    fn spawn() -> Self {
        let (jobs_tx, jobs_rx) = channel::<Job>();
        let (done_tx, done_rx) = channel::<WorkerMsg>();
        std::thread::Builder::new()
            .name("ring-build".into())
            .spawn(move || {
                let reg = ringdesign_script::registry();
                let mut evaluator = Evaluator::with_exprs(ringdesign_script::engine());
                // CAD bodies and tessellations an edit leaves alone come back from here on the next build.
                let cache = Mutex::new(ringdesign_core::cad::Cache::default());
                let never = std::sync::atomic::AtomicBool::new(false);
                // The last band's ring frame, by the band's epoch, and the ghost's judge over it, by what it reads.
                let mut last_band: Option<(u64, Arc<BandSurface>)> = None;
                let mut last_judge: Option<(JudgeKey, Arc<GhostJudge>)> = None;
                while let Ok(mut job) = jobs_rx.recv() {
                    // Skip stale work: only the newest queued job matters.
                    while let Ok(newer) = jobs_rx.try_recv() {
                        job = newer;
                    }
                    let generation = job.generation;
                    // One panic used to end the session's rebuilding: the
                    // thread died, `jobs_tx.send` failed from then on, and
                    // the app went quietly read-only. Now it is one failed
                    // build with a message.
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Done, String> {
                        // A graph-driven design is evaluated first; the library's
                        // identity is its epoch, so a replaced library re-runs it.
                        let mut graph_done = None;
                        let mut field_from_graph = None;
                        if let Some(g) = &job.graph {
                            let epoch = Arc::as_ptr(&job.lib) as usize as u64;
                            match evaluate_design(&mut evaluator, g, &reg, &job.lib, epoch) {
                                Ok(out) => {
                                    let mut d = (*out.design).clone();
                                    d.graph = job.design.graph.clone();
                                    d.manufacturing = job.design.manufacturing.clone();
                                    d.casting_trials = job.design.casting_trials.clone();
                                    d.pins = job.design.pins.clone();
                                    let values = out
                                        .report
                                        .values
                                        .iter()
                                        .map(|(id, outs)| (*id, outs.iter().map(|(k, v)| (k.clone(), v.summary())).collect()))
                                        .collect();
                                    let notes = out
                                        .report
                                        .status
                                        .iter()
                                        .filter(|(_, s)| !s.errors.is_empty() || !s.warnings.is_empty())
                                        .map(|(id, s)| {
                                            let mut lines: Vec<String> = s.errors.iter().map(|(i, m)| if s.items > 1 { format!("item {i}: {m}") } else { m.clone() }).collect();
                                            lines.extend(s.warnings.iter().cloned());
                                            (*id, lines)
                                        })
                                        .collect();
                                    if let Some(lib) = &out.baked_library {
                                        job.lib = lib.clone();
                                    }
                                    let effects = ringdesign_graph::focus::effects(g, &reg, &out.report.values, &out.design);
                                    graph_done = Some(GraphDone { design: d.clone(), effects, baked_library: out.baked_library, values, notes, errors: Vec::new(), ok: true });
                                    field_from_graph = Some(out.field);
                                    job.design = d;
                                }
                                Err(e) => {
                                    graph_done = Some(GraphDone { design: job.design.clone(), effects: BTreeMap::new(), baked_library: None, values: BTreeMap::new(), notes: BTreeMap::new(), errors: vec![e], ok: false });
                                }
                            }
                        }
                        let cutters = if job.show_cutters { ringdesign_core::setting::ghost_vertices(&job.design, &job.lib) } else { Vec::new() };
                        let memo = ringdesign_core::cad::Memo::new(&cache);
                        let result = if job.live_cuts {
                            ringdesign_core::mesh::try_build_memo(&job.design, &job.lib, job.params, &never, memo)
                        } else {
                            // Seats uncut and parts unjoined: the cheap build while a stroke is live.
                            let stock = ringdesign_workbench::cad_tools::without_joins(&ringdesign_core::setting::without_solids(&job.design));
                            ringdesign_core::mesh::try_build_memo(&stock, &job.lib, job.params, &never, memo)
                        }
                        .map_err(|e| format!("{e:#}"))?;
                        // The pick scene, the ring frame and the selection's channel build on threads of their own beside the verdict and the staging, which read none of them.
                        let (pick, band, judge, select, field, cast, stones, hot_spot, gems, metal, edges) = std::thread::scope(|s| {
                            let pick = s.spawn(|| PickScene::build(&result, &job.design));
                            // Every design's ring frame over the build's own band; an unchanged band keeps its surface.
                            let last_band = &mut last_band;
                            let band = s.spawn(|| {
                                result.band.clone().map(|mesh| {
                                    let epoch = ringdesign_core::cad::surface_epoch(&mesh);
                                    match &*last_band {
                                        Some((e, surface)) if *e == epoch => (epoch, surface.clone()),
                                        _ => {
                                            let surface = Arc::new(BandSurface::shared(mesh));
                                            *last_band = Some((epoch, surface.clone()));
                                            (epoch, surface)
                                        }
                                    }
                                })
                            });
                            let select = s.spawn(|| {
                                let weights = ringdesign_workbench::viewport::tint(&job.selection, &result);
                                if weights.is_empty() { Vec::new() } else { render::stage_select(&result.mesh, &weights) }
                            });
                            // The verdict itself comes from the surface, at a fixed
                            // sampling so it cannot wobble with preview quality; any
                            // undercut arrives located and blamed.
                            let mut field = match field_from_graph {
                                Some(f) => f,
                                None => castability::attributed_field_report(&job.design, &job.lib, &job.design.draft, 192, 128),
                            };
                            // Parts are judged only on a build that joins them; with live cuts off they keep the design-only note.
                            if job.live_cuts {
                                castability::judge_parts(&mut field, &job.design, &result);
                            }
                            // The carried ghost's judge reads the band through the ring frame's own tree at the verdict's parting plane; the same band and plane keep the last one.
                            let band = band.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
                            let parting = field.parting_z_mm;
                            let judge = band.clone().map(|(epoch, surface)| {
                                let key: JudgeKey = (epoch, parting.to_bits(), job.design.draft.min_draft_deg.to_bits(), job.design.inner_radius_mm().to_bits());
                                let (last_judge, design) = (&mut last_judge, &job.design);
                                s.spawn(move || match &*last_judge {
                                    Some((k, judge)) if *k == key => judge.clone(),
                                    _ => {
                                        let judge = Arc::new(GhostJudge::prepared_with(design, Some(surface.shared_mesh().clone()), Some(surface.tree().clone()), parting));
                                        *last_judge = Some((key, judge.clone()));
                                        judge
                                    }
                                })
                            });
                            // The draft colours paint at the verdict's own parting plane.
                            let cast = castability::analyze_at(
                                &result.mesh,
                                &job.design.draft,
                                job.design.inner_radius_mm(),
                                field.parting_z_mm,
                            );
                            let stones = ringdesign_core::stones::report(&job.design, field.parting_z_mm);
                            let hot_spot = castability::modulus_scan(&job.design, &job.lib, 64)
                                .into_iter()
                                .max_by(|a, b| a.1.total_cmp(&b.1));
                            let mut gems = crate::gems::preview_vertices(&job.design, &job.lib);
                            // Stones set as CAD parts draw with the preview stones: never metal, never exported.
                            for c in result.parts.evaluated.iter().flat_map(|e| &e.components).filter(|c| c.settings.reference) {
                                let t = crate::gems::GEM_TINT;
                                for f in &c.mesh.faces {
                                    let n = c.mesh.face_normal(f).unwrap_or([0.0, 0.0, 1.0]).map(|v| v as f32);
                                    for &i in f {
                                        let p = c.mesh.vertices[i as usize];
                                        gems.extend_from_slice(&[p.0, p.1, p.2, n[0], n[1], n[2], t[0], t[1], t[2], t[0], t[1], t[2]]);
                                    }
                                }
                            }
                            let metal = if job.design.band_is_procedural() {
                                render::stage_mesh(&result.mesh, Some(&cast), (job.design.inner_radius_mm(), job.design.draft.min_section_mm))
                            } else {
                                render::stage_cad(&result.mesh)
                            };
                            let edges = result.parts.evaluated.as_ref().map(render::stage_edges).unwrap_or_default();
                            // A panic on any of them fails this build, as one here does.
                            let pick = pick.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
                            let judge = judge.map(|j| j.join().unwrap_or_else(|p| std::panic::resume_unwind(p)));
                            let select = select.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
                            (Arc::new(pick), band.map(|(_, surface)| surface), judge, select, field, cast, stones, hot_spot, gems, metal, edges)
                        });
                        Ok(Done {
                            generation,
                            params: job.params,
                            result,
                            cast,
                            field,
                            hot_spot,
                            stones,
                            gems,
                            cutters,
                            metal,
                            edges,
                            pick,
                            graph: graph_done,
                            band,
                            judge,
                            selection: job.selection,
                            select,
                        })
                    }));
                    // A panic inside a cache update leaves it poisoned; it starts again empty.
                    if cache.is_poisoned() {
                        cache.clear_poison();
                        if let Ok(mut c) = cache.lock() {
                            c.clear();
                        }
                    }
                    let msg = match outcome {
                        Ok(Ok(done)) => WorkerMsg::Done(Box::new(done)),
                        Ok(Err(message)) => WorkerMsg::Failed { generation, message },
                        Err(p) => WorkerMsg::Failed {
                            generation,
                            message: p
                                .downcast_ref::<&str>()
                                .map(|s| s.to_string())
                                .or_else(|| p.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "panicked".into()),
                        },
                    };
                    if done_tx.send(msg).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn build worker");
        Self {
            jobs: jobs_tx,
            done: done_rx,
        }
    }
}

