//! The 3D ring pane, and the background rebuild that feeds it.
//!
//! Touch mapping follows the Nomad convention the framework's stylus probe makes possible: one
//! finger orbits, two fingers pinch-zoom and pan. The desktop's shift-drag and scroll-wheel have no
//! touch equivalent and are gone.
//!
//! The worker differs from the desktop's in two measured ways. `Mesh::validate` is 26–43% of its
//! job and re-proves watertightness the sweep guarantees by construction, so it is skipped;
//! `castability::analyze` is another ~30% and only matters when you stop moving, so it is deferred
//! to the settled build. And the vertex buffer is staged here rather than on the UI thread — at
//! 384x144 it is ~12 MB, which is more than a frame.

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use egui_mobile::egui;
use ringdesign_core::RingDesign;
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastReport, FieldReport};
use ringdesign_core::mesh::{BuildParams, Vec3};

use crate::camera::OrbitCamera;
use crate::viewport::{GpuMeshRenderer, PaneLook, ShadeMode, paint_callback};

/// Lightweight mesh while editing. Settled previews use the selected detail tier.
pub const PREVIEW: BuildParams = BuildParams {
    theta_steps: 384,
    profile_steps: 144,
    min_wall_mm: 0.5,
    adaptive: false,
    refine: None,
    soften_mm: 0.0,
};
/// 655k triangles; also the default settled viewport quality.
pub const EXPORT: BuildParams = BuildParams {
    theta_steps: 1024,
    profile_steps: 320,
    min_wall_mm: 0.5,
    adaptive: false,
    refine: None,
    soften_mm: 0.0,
};

/// The expanded triangle buffer at showcase quality is approximately 189 MiB.
pub const SHOWCASE: BuildParams = BuildParams {
    theta_steps: 1536,
    profile_steps: 448,
    ..EXPORT
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewQuality {
    Fast,
    #[default]
    Detailed,
    Showcase,
}

impl PreviewQuality {
    pub const ALL: [Self; 3] = [Self::Fast, Self::Detailed, Self::Showcase];

    pub fn label(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Detailed => "Detailed",
            Self::Showcase => "Showcase",
        }
    }

    pub fn params(self, settled: bool) -> BuildParams {
        if !settled { return PREVIEW; }
        match self {
            Self::Fast => PREVIEW,
            Self::Detailed => EXPORT,
            Self::Showcase => SHOWCASE,
        }
    }
}

/// Legacy software-export tint. The GPU viewport uses linear metal reflectance.
pub const METAL_TINT: [f32; 3] = [0.86, 0.80, 0.62];

pub struct ViewResponse {
    pub response: egui::Response,
    pub rect: egui::Rect,
    pub probe: Option<([f32; 3], [f32; 3])>,
}

pub struct RingPane {
    pub camera: OrbitCamera,
    pub navigation: ringdesign_workbench::navigation::Settings,
    pub shade: ShadeMode,
    pub wireframe: bool,
    pub finish: usize,
    pub polish: usize,
    /// Render at true physical size using the panel's real pixel density.
    pub actual_size: bool,
    pub clip_plane: [f32; 4],
    /// Tint and strength of the chosen node's highlight; zero strength is off.
    pub focus: [f32; 4],
    /// Every part's edges over the metal; a chosen edge draws either way.
    pub edges: bool,
    /// The two-finger twist in hand.
    pub twist: Twist,
    /// Where the fingers of the two-finger gesture in hand last stood.
    pinched: Option<egui::Pos2>,
}

/// A pinch is never quite straight, so a twist turns nothing until the
/// fingers have turned `TWIST_DEAD` about each other; let go near a quarter
/// turn and the view settles onto it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Twist {
    turned: f32,
    engaged: bool,
    settle: Option<f32>,
}

const TWIST_DEAD: f32 = 0.14;
const TWIST_SNAP: f32 = 0.26;

impl Twist {
    /// The roll after this frame's `delta`, `None` with the fingers lifted. Returns whether it moved.
    pub fn advance(&mut self, roll: &mut f32, delta: Option<f32>) -> bool {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        let wrap = |a: f32| (a + PI).rem_euclid(TAU) - PI;
        match delta {
            Some(d) => {
                self.settle = None;
                if !self.engaged {
                    self.turned += d;
                    if self.turned.abs() < TWIST_DEAD { return false; }
                    self.engaged = true;
                }
                *roll = wrap(*roll + d);
                d != 0.0
            }
            None => {
                if self.engaged {
                    let nearest = (*roll / FRAC_PI_2).round() * FRAC_PI_2;
                    if (*roll - nearest).abs() < TWIST_SNAP { self.settle = Some(wrap(nearest)); }
                }
                self.engaged = false;
                self.turned = 0.0;
                let Some(to) = self.settle else { return false };
                let gap = wrap(to - *roll);
                if gap.abs() < 0.002 {
                    *roll = to;
                    self.settle = None;
                } else {
                    *roll = wrap(*roll + gap * 0.3);
                }
                true
            }
        }
    }
}

impl Default for RingPane {
    fn default() -> Self {
        Self {
            camera: OrbitCamera::default(),
            navigation: Default::default(),
            shade: ShadeMode::default(),
            wireframe: false,
            finish: 0,
            polish: 0,
            actual_size: false,
            clip_plane: [0.0; 4],
            focus: [0.0; 4],
            edges: true,
            twist: Twist::default(),
            pinched: None,
        }
    }
}

impl RingPane {
    /// Draws the pane, a pinch leaving the pivot on `pivot`'s metal when one is given; the response and the world ray under a long press, for the tap probe.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &Arc<Mutex<GpuMeshRenderer>>,
        px_per_mm: Option<f32>,
        lock_orbit: bool,
        pivot: Option<&ringdesign_core::Mesh>,
    ) -> ViewResponse {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if !ui.is_rect_visible(rect) {
            return ViewResponse {
                response,
                rect,
                probe: None,
            };
        }

        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 18, 20));

        if let Ok(r) = renderer.lock() {
            if let Some(err) = r.failed() {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("shader failed\n{err}"),
                    egui::FontId::monospace(12.0),
                    egui::Color32::from_rgb(240, 120, 120),
                );
                return ViewResponse {
                    response,
                    rect,
                    probe: None,
                };
            }
            if !r.has_mesh() {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "building…",
                    egui::FontId::proportional(14.0),
                    egui::Color32::GRAY,
                );
                return ViewResponse {
                    response,
                    rect,
                    probe: None,
                };
            }
        }

        let moved = !lock_orbit && self.handle_touch(ui, &response, rect, pivot);
        if moved || renderer.lock().is_ok_and(|r| r.timed()) {
            ui.ctx().request_repaint();
        }
        let probe_ray = response
            .long_touched()
            .then(|| response.interact_pointer_pos())
            .flatten()
            .map(|pos| self.camera.ray(rect, pos));

        // True scale: the camera is orthographic and already in millimetres, so physical size is
        // one zoom value rather than a different render path.
        if let (true, Some(ppmm)) = (self.actual_size, px_per_mm) {
            let ppp = ui.ctx().pixels_per_point();
            let pt_per_mm = ppmm / ppp;
            let half_mm = rect.width().min(rect.height()) * 0.5 / pt_per_mm;
            self.camera.set_half_extent(half_mm);
        }

        let (mvp, normal_matrix) = self.camera.matrices(rect);
        let look = PaneLook {
            shade: self.shade,
            base_color: ringdesign_core::render::METAL_FINISHES[self.finish.min(6)].1,
            roughness: ringdesign_core::render::POLISHES[self.polish.min(2)].1,
            wire: self.wireframe.then_some([0.10, 0.10, 0.12]),
            clip_plane: self.clip_plane,
            focus: self.focus,
            edges: self.edges,
        };
        paint_callback(ui, rect, renderer.clone(), mvp, normal_matrix, look);
        ViewResponse {
            response,
            rect,
            probe: probe_ray,
        }
    }

    /// One finger orbits; two zoom, pan and twist about the metal under them, and leave the pivot on the ring when they lift. Returns whether anything changed.
    fn handle_touch(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, pivot: Option<&ringdesign_core::Mesh>) -> bool {
        let multi = pinch_in(ui, rect);
        if let Some(mt) = multi {
            // A second finger takes the gesture from the orbit outright, so a pinch never also
            // spins the ring. What lay under the fingers stays under them as they zoom, twist and travel.
            let (held, _) = self.camera.ray(rect, mt.center_pos - mt.translation_delta);
            if (mt.zoom_delta - 1.0).abs() > 1e-4 {
                self.camera.zoom_by_factor(mt.zoom_delta);
            }
            // Two fingers turning about each other roll the view, which is
            // the one way to stand the ring on its head: pitch stops at the poles.
            if !self.navigation.locked {
                self.twist.advance(&mut self.camera.roll, Some(mt.rotation_delta));
            }
            self.camera.keep_under(held, mt.center_pos, rect);
            self.pinched = Some(mt.center_pos);
            return true;
        }
        // The fingers lifted: the orbit turns about the metal they zoomed in on from now on, before a twist settles about it.
        if let Some(at) = self.pinched.take()
            && let Some(mesh) = pivot
        {
            self.settle_pivot(mesh, rect, at);
        }
        let settling = self.twist.advance(&mut self.camera.roll, None);
        if response.dragged() {
            if self.navigation.locked {
                self.camera.pan_by(response.drag_delta(), rect);
            } else {
                self.camera.orbit(response.drag_delta());
            }
            return true;
        }
        settling
    }

    /// Leaves the pivot on the metal of `mesh` under fingers that lifted at `at`, else under the view's middle, else on the middle's ray at the pivot's depth; nothing on screen moves.
    pub fn settle_pivot(&mut self, mesh: &ringdesign_core::Mesh, rect: egui::Rect, at: egui::Pos2) {
        let clock = std::time::Instant::now();
        let ray = |p: egui::Pos2| {
            let (o, d) = self.camera.ray(rect, p);
            ringdesign_core::interaction::pick::Ray { origin: o.map(f64::from), direction: d.map(f64::from) }
        };
        let found = ringdesign_workbench::touch::view::pivot_after_pinch(mesh, ray(at), ray(rect.center()));
        match found {
            Some(p) => self.camera.pivot_on(p.map(|v| v as f32)),
            None => self.camera.pivot_to_middle(rect),
        }
        log::info!("pinch pivot {:?} over {} tris in {:.1} ms", self.camera.target, mesh.faces.len(), clock.elapsed().as_secs_f64() * 1e3);
    }
}

/// Two fingers whose gesture began on `rect`, and on nothing drawn over it. egui reports one pinch for
/// the whole screen, so read raw a pinch on the graph sheet zoomed and rolled the ring as well.
pub fn pinch_in(ui: &egui::Ui, rect: egui::Rect) -> Option<egui::MultiTouchInfo> {
    // Counted before anything else: the frame the fingers land is the one with no gesture yet.
    let at = first_finger(ui)?;
    let mt = ui.input(|i| i.multi_touch())?;
    let mine = rect.contains(at) && ui.ctx().layer_id_at(at).is_none_or(|l| l == ui.layer_id());
    mine.then_some(mt)
}

/// Where the finger that began the gesture in hand came down. Not egui's `start_pos`: that is the
/// pointer as of the frame before, so two fingers landing in one frame are placed where the last
/// gesture was.
fn first_finger(ui: &egui::Ui) -> Option<egui::Pos2> {
    #[derive(Clone, Default)]
    struct Fingers {
        down: Vec<egui::TouchId>,
        first: Option<egui::Pos2>,
        pass: Option<u64>,
    }
    let pass = ui.ctx().cumulative_pass_nr();
    let (events, touching): (Vec<(egui::TouchId, egui::TouchPhase, egui::Pos2)>, bool) = ui.input(|i| {
        let events = i
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Touch { id, phase, pos, .. } => Some((*id, *phase, *pos)),
                _ => None,
            })
            .collect();
        (events, i.any_touches())
    });
    ui.ctx().data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Fingers>(egui::Id::new("ringdesigner-fingers"));
        // Every pane asks each pass; the pass's touches are counted once.
        if f.pass != Some(pass) {
            // A pass nobody asked about may have lifted or landed fingers: what is down is unknown, and
            // a gesture the tracker did not see begin belongs to no one.
            if f.pass.is_none_or(|p| p + 1 != pass) {
                f.down.clear();
                f.first = None;
            }
            f.pass = Some(pass);
            for (id, phase, pos) in events {
                match phase {
                    egui::TouchPhase::Start => {
                        if f.down.is_empty() {
                            f.first = Some(pos);
                        }
                        f.down.push(id);
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => f.down.retain(|t| *t != id),
                    egui::TouchPhase::Move => {}
                }
            }
            // egui knows when every finger is up; a pass nobody asked about cannot leave one down here.
            if !touching {
                f.down.clear();
            }
        }
        f.first
    })
}

// --- Background rebuild ------------------------------------------------------

/// A finished build, with the vertex buffer already staged off the UI thread.
pub struct Done {
    pub generation: u64,
    pub verts: Vec<f32>,
    /// Stone-preview triangles, empty when the toggle is off.
    pub gems: Vec<f32>,
    /// The seats' cutters as a ghost, empty unless asked for.
    pub ghost: Vec<f32>,
    /// The build on screen: its mesh, kept for the tap probe's raycast, and the parts it was made of.
    pub build: Arc<ringdesign_core::BuildResult>,
    /// One pick scene over the build, for a design with parts or stones to choose.
    pub scene: Option<Arc<ringdesign_core::interaction::pick::PickScene>>,
    /// The ring frame parts are seated on, over the build's own band; `None` without parts.
    pub band: Option<Arc<ringdesign_workbench::command::BandSurface>>,
    /// The carried ghost's judge over that band, its radial lines laid out on the worker; settled builds only.
    pub judge: Option<Arc<ringdesign_core::castability::ghost::GhostJudge>>,
    pub bounds: Option<(Vec3, Vec3)>,
    pub triangles: usize,
    pub volume_mm3: f64,
    pub build_ms: u128,
    /// The build's own report — dimensions, volume, mesh quality. The worker
    /// used to destructure three numbers out of it and drop the rest, so the
    /// Report sheet had to rebuild what was already in hand.
    pub report: ringdesign_core::mesh::Report,
    /// Only present on a settled build — analyze is ~30% of the worker and is not worth paying
    /// while a slider is still moving.
    pub cast: Option<CastReport>,
    /// The authoritative verdict, sampled off the surface itself: no facet
    /// phantoms, undercuts arrive located and blamed, and the thinnest wall
    /// rides along. Settled builds only, like `cast`.
    pub field: Option<FieldReport>,
    /// The graph's evaluation, when the design carries one.
    pub graph: Option<crate::graph::GraphDone>,
    /// What `mesh` was built with, so a before/after can be built to match.
    pub params: BuildParams,
    /// The mesh shows one isolated layer rather than the design.
    pub isolated: bool,
    /// The parts the mesh shows alone, of those asked for; empty for the whole ring.
    pub alone: Vec<ringdesign_core::sketch::Id>,
    /// The parts asked for that could not stand alone.
    pub left_out: Vec<ringdesign_core::sketch::Id>,
    /// Why they could not.
    pub alone_note: Option<String>,
    /// Where the worker's time went, milliseconds.
    pub timings: Timings,
}

/// A build's time on the worker, milliseconds: the whole job, the vertex buffer and stones staged, and the pick scene.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timings {
    pub worker_ms: f64,
    pub stage_ms: f64,
    pub scene_ms: f64,
}

struct Job {
    generation: u64,
    design: RingDesign,
    lib: Arc<AlphaLibrary>,
    params: BuildParams,
    analyze: bool,
    gems: bool,
    view_layer: Option<usize>,
    view_parts: Vec<ringdesign_core::sketch::Id>,
    cuts: Cuts,
}

/// How made settings show in the preview: resolved live into the mesh, and whether their cutters are
/// drawn over it as a ghost. Off, the ring builds as its stock alone, which is also the faster build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cuts {
    pub live: bool,
    pub ghost: bool,
    /// With live cuts off, still strike the stamps: the reel shows them before the seats are cut.
    pub stamps: bool,
}

impl Default for Cuts {
    fn default() -> Self {
        Self { live: true, ghost: false, stamps: false }
    }
}

pub struct Worker {
    jobs: Sender<Job>,
    pub done: Receiver<Done>,
    pub errors: Receiver<(u64, String)>,
    detail_done: Receiver<(u64, Vec<ringdesign_core::dfm::DfmFinding>)>,
}

impl Worker {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (jobs_tx, jobs_rx) = channel::<Job>();
        let (done_tx, done_rx) = channel::<Done>();
        let (error_tx, error_rx) = channel();
        // Fine-detail measurement of a large painted alpha can take seconds.
        // It must block neither input nor the next geometry preview.
        let (detail_tx, detail_rx) = channel::<(u64, RingDesign, Arc<AlphaLibrary>)>();
        let (detail_done_tx, detail_done_rx) = channel();
        let detail_ctx = ctx.clone();
        std::thread::Builder::new()
            .name("ring-detail".into())
            .spawn(move || {
                while let Ok(mut job) = detail_rx.recv() {
                    while let Ok(newer) = detail_rx.try_recv() {
                        job = newer;
                    }
                    let findings = ringdesign_core::dfm::findings_in(&job.1, &job.2);
                    if detail_done_tx.send((job.0, findings)).is_err() {
                        break;
                    }
                    detail_ctx.request_repaint();
                }
            })
            .expect("spawn detail worker");
        std::thread::Builder::new()
            .name("ring-build".into())
            .spawn(move || {
                let mut runner = crate::graph::GraphRunner::new();
                // The last band's ring frame, by the band's epoch.
                let mut last_band: Option<(u64, Arc<ringdesign_workbench::command::BandSurface>)> = None;
                while let Ok(mut job) = jobs_rx.recv() {
                    // Skip stale work: only the newest queued job matters.
                    while let Ok(newer) = jobs_rx.try_recv() {
                        job = newer;
                    }
                    let clock = std::time::Instant::now();
                    // A graph-driven design is evaluated first; a failed
                    // evaluation builds the last good design instead.
                    let mut graph = runner.run(&job.design, &job.lib);
                    if let Some(g) = &graph {
                        if g.ok {
                            job.design = g.design.clone();
                            if let Some(lib) = &g.baked_library {
                                job.lib = lib.clone();
                            }
                        }
                    }
                    let field_from_graph = graph.as_mut().and_then(|g| g.field.take());
                    if job.analyze {
                        let _ =
                            detail_tx.send((job.generation, job.design.clone(), job.lib.clone()));
                    }
                    // The cutters are read off the whole design; with live cuts off only the stock is built.
                    let ghost = if job.cuts.ghost { ringdesign_core::setting::ghost_vertices(&job.design, &job.lib) } else { Vec::new() };
                    let shown = (!job.cuts.live).then(|| {
                        let mut d = ringdesign_core::setting::without_solids(&job.design);
                        if job.cuts.stamps {
                            d.stamps = job.design.stamps.clone();
                        }
                        d
                    });
                    let out = match ringdesign_core::mesh::try_build(shown.as_ref().unwrap_or(&job.design), &job.lib, job.params) {
                        Ok(out) => out,
                        Err(e) => { let _ = error_tx.send((job.generation, e.to_string())); ctx.request_repaint(); continue; }
                    };
                    let report = out.report.clone();
                    let view_layer = job
                        .view_layer
                        .filter(|&i| i < job.design.layers.layers.len());
                    let visible = if let Some(index) = view_layer {
                        let mut design = shown.clone().unwrap_or_else(|| job.design.clone());
                        for (i, entry) in design.layers.layers.iter_mut().enumerate() {
                            entry.enabled &= i == index;
                        }
                        std::borrow::Cow::Owned(design)
                    } else {
                        std::borrow::Cow::Borrowed(&job.design)
                    };
                    // Isolation is a rendering operation: the reports read the complete source, and exports never see these copies.
                    let layer_build = view_layer.is_some().then(|| ringdesign_core::mesh::build(&visible, &job.lib, job.params));
                    let parts = job.design.cad.is_some();
                    // Parts that cannot stand alone are left out and said; when none can, the whole ring shows.
                    let (mut left_out, mut alone_note) = (Vec::new(), None);
                    let alone = if job.view_parts.is_empty() || view_layer.is_some() || !parts {
                        None
                    } else {
                        match ringdesign_workbench::touch::isolate::alone(&out, &job.view_parts) {
                            Ok(a) => {
                                if !a.left_out.is_empty() {
                                    alone_note = Some(a.left_out.iter().map(|(_, why)| why.as_str()).collect::<Vec<_>>().join(" · "));
                                    left_out = a.left_out.iter().map(|(id, _)| *id).collect();
                                }
                                Some((a.build, a.shown))
                            }
                            Err(why) => {
                                alone_note = Some(why);
                                left_out = job.view_parts.clone();
                                None
                            }
                        }
                    };
                    let plain = view_layer.is_some() || alone.is_some();
                    // Parts and stones are chosen through one scene over what is on screen; a plain band needs none.
                    let choosable = parts || !ringdesign_core::setstone::set_stones(&visible).is_empty();
                    let (field, cast, verts, gems, scene, stage_ms, scene_ms) = std::thread::scope(|s| {
                        let display = alone.as_ref().map(|(b, _)| b).or(layer_build.as_ref()).unwrap_or(&out);
                        // The pick scene builds on its own thread beside the verdict and the staging, which never read it.
                        let picking = s.spawn(|| {
                            let clock = std::time::Instant::now();
                            let scene = choosable.then(|| Arc::new(ringdesign_core::interaction::pick::PickScene::build(display, &visible)));
                            (scene, clock.elapsed().as_secs_f64() * 1e3)
                        });
                        // The verdict first, its CAD parts judged on the whole build; the draft colours then paint at its plane.
                        let field = job.analyze.then(|| {
                            let mut f = field_from_graph.unwrap_or_else(|| {
                                castability::attributed_field_report(
                                    &job.design,
                                    &job.lib,
                                    &job.design.draft,
                                    160,
                                    112,
                                )
                            });
                            castability::judge_parts(&mut f, &job.design, &out);
                            f
                        });
                        let cast = field.as_ref().map(|f| {
                            castability::analyze_at(
                                &out.mesh,
                                &job.design.draft,
                                job.design.inner_radius_mm(),
                                f.parting_z_mm,
                            )
                        });
                        let staging = std::time::Instant::now();
                        let verts = GpuMeshRenderer::stage(
                            &display.mesh,
                            if plain { None } else { cast.as_ref() },
                            (
                                job.design.inner_radius_mm(),
                                job.design.draft.min_section_mm,
                            ),
                        );
                        // Every stone where the build stands it, each in its own colour: never metal, never exported.
                        let gems = if job.gems && alone.is_none() {
                            ringdesign_core::gems::built_vertices(&visible, &job.lib, display)
                        } else {
                            Vec::new()
                        };
                        let stage_ms = staging.elapsed().as_secs_f64() * 1e3;
                        // A panic building the scene fails this build, as one here does.
                        let (scene, scene_ms) = picking.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
                        (field, cast, verts, gems, scene, stage_ms, scene_ms)
                    });
                    let (alone, shown) = alone.map_or((None, Vec::new()), |(b, shown)| (Some(b), shown));
                    let mut out = alone.or(layer_build).unwrap_or(out);
                    // A design with parts keeps the band they stand on as its ring frame; one without drops the copy.
                    if !parts {
                        out.band = None;
                    }
                    let band = out.band.clone().map(|mesh| {
                        let epoch = ringdesign_core::cad::surface_epoch(&mesh);
                        match &last_band {
                            Some((e, surface)) if *e == epoch => surface.clone(),
                            _ => {
                                let surface = Arc::new(ringdesign_workbench::command::BandSurface::shared(mesh));
                                last_band = Some((epoch, surface.clone()));
                                surface
                            }
                        }
                    });
                    // The ghost's judge reads the band through the ring frame's own tree, at the verdict's parting plane.
                    let judge = band.as_ref().zip(field.as_ref()).map(|(surface, f)| {
                        Arc::new(ringdesign_core::castability::ghost::GhostJudge::prepared_with(
                            &job.design,
                            Some(surface.shared_mesh().clone()),
                            Some(surface.tree().clone()),
                            f.parting_z_mm,
                        ))
                    });
                    let build = Arc::new(out);
                    let done = Done {
                        generation: job.generation,
                        verts,
                        gems,
                        ghost,
                        bounds: build.mesh.bounds(),
                        build,
                        scene,
                        band,
                        judge,
                        triangles: report.validation.triangle_count,
                        volume_mm3: report.volume_mm3,
                        build_ms: report.build_ms,
                        report,
                        cast,
                        field,
                        graph,
                        params: job.params,
                        isolated: view_layer.is_some(),
                        alone: shown,
                        left_out,
                        alone_note,
                        timings: Timings { worker_ms: clock.elapsed().as_secs_f64() * 1e3, stage_ms, scene_ms },
                    };
                    if done_tx.send(done).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn build worker");
        Self {
            jobs: jobs_tx,
            done: done_rx,
            errors: error_rx,
            detail_done: detail_done_rx,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        generation: u64,
        design: &RingDesign,
        lib: &Arc<AlphaLibrary>,
        params: BuildParams,
        analyze: bool,
        gems: bool,
        view_layer: Option<usize>,
        view_parts: &[ringdesign_core::sketch::Id],
        cuts: Cuts,
    ) -> bool {
        self.jobs
            .send(Job {
                generation,
                design: design.clone(),
                lib: lib.clone(),
                params,
                analyze,
                gems,
                view_layer,
                view_parts: view_parts.to_vec(),
                cuts,
            })
            .is_ok()
    }

    pub fn poll(&self) -> Option<Done> {
        match self.done.try_recv() {
            Ok(d) => Some(d),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
    pub fn poll_detail(&self) -> Option<(u64, Vec<ringdesign_core::dfm::DfmFinding>)> {
        self.detail_done.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_twist_ignores_a_crooked_pinch_then_follows_the_fingers_and_settles_on_a_quarter() {
        use std::f32::consts::PI;
        let mut twist = Twist::default();
        let mut roll = 0.0f32;
        // A pinch wanders a few degrees either way and turns nothing.
        for d in [0.03, -0.02, 0.04, -0.05, 0.02] {
            assert!(!twist.advance(&mut roll, Some(d)));
        }
        assert_eq!(roll, 0.0);
        assert!(!twist.advance(&mut roll, None), "and nothing settles after it");
        // A deliberate half turn, clockwise, stopped a little short.
        let step = 0.05;
        let mut turned = 0.0;
        while turned < PI - 0.12 {
            twist.advance(&mut roll, Some(step));
            turned += step;
        }
        assert!(roll > 2.6 && roll < PI, "{roll}");
        // Fingers up: it eases the rest of the way and stops exactly upside down.
        let mut frames = 0;
        while twist.advance(&mut roll, None) {
            frames += 1;
            assert!(frames < 60);
        }
        assert!((roll.abs() - PI).abs() < 1e-6 && frames > 2, "{roll} after {frames}");
        // Let go between quarters and it stays where it was left.
        let mut free = Twist::default();
        let mut roll = 0.0f32;
        for _ in 0..16 { free.advance(&mut roll, Some(0.05)); }
        let left = roll;
        assert!(!free.advance(&mut roll, None) && roll == left && left > 0.5, "{left}");
    }

    #[test]
    fn a_pinch_belongs_to_the_pane_it_started_on() {
        let ctx = egui::Context::default();
        let pane = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        let touch = |id: u64, phase: egui::TouchPhase, pos: egui::Pos2| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None };
        let mut seen = Vec::new();
        for (start, apart) in [(egui::pos2(200.0, 150.0), 40.0), (egui::pos2(200.0, 600.0), 40.0)] {
            // As egui-winit reports two fingers: the pointer follows the first, and a gesture only starts
            // where there is a pointer.
            let press = |pressed| egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            // Both fingers in one frame, the case egui's own start position gets wrong.
            let mut frames = vec![vec![touch(0, egui::TouchPhase::Start, start), egui::Event::PointerMoved(start), press(true), touch(1, egui::TouchPhase::Start, start + egui::vec2(apart, 0.0))]];
            frames.push(Vec::new());
            for k in 1..=3 {
                frames.push(vec![touch(1, egui::TouchPhase::Move, start + egui::vec2(apart * (1.0 + 0.4 * k as f32), 0.0))]);
            }
            let mut got = None;
            for events in frames {
                let input = egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 800.0))), ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    // egui may run a frame's closure twice; keep the largest zoom any pass saw.
                    if let Some(mt) = pinch_in(ui, pane) { got = Some(got.unwrap_or(1.0f32).max(mt.zoom_delta)); }
                });
                out.textures_delta.clear();
            }
            ctx.run_ui(egui::RawInput { events: vec![touch(0, egui::TouchPhase::End, start), touch(1, egui::TouchPhase::End, start), press(false), egui::Event::PointerGone], ..Default::default() }, |_| {}).textures_delta.clear();
            seen.push(got);
        }
        assert!(seen[0].is_some_and(|z| z > 1.0), "a pinch on the ring zooms it: {:?}", seen[0]);
        assert_eq!(seen[1], None, "a pinch on the sheet below leaves the ring alone");
    }

    #[test]
    fn a_pinch_keeps_the_metal_under_the_fingers_and_leaves_the_pivot_on_it() {
        use ringdesign_core::{AlphaLibrary, templates};
        let d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let built = ringdesign_core::mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() });
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 800.0));
        let mut pane = RingPane::default();
        pane.camera.fit(built.mesh.bounds());
        // The metal the fingers come down on: the crest of the band's near side, off the middle of the view.
        let (start, metal) = (200..300)
            .step_by(5)
            .find_map(|theta| {
                let (p, _) = ringdesign_core::cad::surface_hit(&built.mesh, f64::from(theta), 0.0)?;
                let p = p.map(|v| v as f32);
                let at = pane.camera.projector(rect).at(p);
                let (o, dir) = pane.camera.ray(rect, at);
                let (_, first) = ringdesign_core::interaction::picking::raycast(&built.mesh, o, dir)?;
                let seen = (0..3).map(|i| (first[i] - p[i]).powi(2)).sum::<f32>().sqrt() < 0.05;
                (seen && (at - rect.center()).length() > 60.0 && rect.shrink(80.0).contains(at)).then_some((at, first))
            })
            .expect("the band's near side is in view");
        let ctx = egui::Context::default();
        let touch = |id: u64, phase: egui::TouchPhase, pos: egui::Pos2| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None };
        let run = |pane: &mut RingPane, events: Vec<egui::Event>, pivot: Option<&ringdesign_core::Mesh>| {
            let input = egui::RawInput { events, screen_rect: Some(rect), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                let (_, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                pane.handle_touch(ui, &response, rect, pivot);
            });
            out.textures_delta.clear();
        };
        let press = |pressed| egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        // Two fingers 40 pt either side of the metal, spreading to 140 while their middle travels 60 right and 30 up.
        let fingers = |k: f32| (start + egui::vec2(60.0, -30.0) * k, 40.0 + 100.0 * k);
        run(&mut pane, vec![touch(0, egui::TouchPhase::Start, start - egui::vec2(40.0, 0.0)), egui::Event::PointerMoved(start), press(true), touch(1, egui::TouchPhase::Start, start + egui::vec2(40.0, 0.0))], None);
        run(&mut pane, Vec::new(), None);
        let (mut old, mut last) = (pane.camera, (start, 40.0));
        for k in 1..=10 {
            let (c, h) = fingers(k as f32 / 10.0);
            run(&mut pane, vec![touch(0, egui::TouchPhase::Move, c - egui::vec2(h, 0.0)), touch(1, egui::TouchPhase::Move, c + egui::vec2(h, 0.0))], None);
            let under = pane.camera.projector(rect).at(metal);
            assert!((under - c).length() < 0.5, "step {k}: the metal stands at {under:?}, the fingers at {c:?}");
            // As it was read before: a zoom about the middle of the view, then the fingers' travel.
            old.zoom_by_factor(h / last.1);
            old.pan_by(c - last.0, rect);
            last = (c, h);
        }
        assert!((pane.camera.zoom - 3.5).abs() < 0.01, "{}", pane.camera.zoom);
        let (c, _) = fingers(1.0);
        // Read the old way the metal runs out 2.5 times its offset from the middle of the view, less the travel the zooms stretched.
        let drifted = (old.projector(rect).at(metal) - c).length();
        let stretched: f32 = (1..=10).map(|k| 140.0 / (40.0 + 10.0 * k as f32)).sum();
        let expected = ((start - rect.center()) * 2.5 + egui::vec2(6.0, -3.0) * (stretched - 10.0)).length();
        assert!((drifted - expected).abs() < 1.0 && drifted > 100.0, "read the old way the metal ends {drifted} pt from the fingers, {expected} expected");
        // The fingers lift: the pivot lands on the metal they held, and nothing on screen moves.
        run(&mut pane, vec![touch(0, egui::TouchPhase::End, c), touch(1, egui::TouchPhase::End, c), press(false), egui::Event::PointerGone], Some(&built.mesh));
        let pivot = pane.camera.target;
        let off = (0..3).map(|i| (pivot[i] - metal[i]).powi(2)).sum::<f32>().sqrt();
        assert!(off < 0.05, "the pivot {pivot:?} against the metal {metal:?}");
        assert!((pane.camera.projector(rect).at(metal) - c).length() < 0.5);
        // A turn now keeps that metal where it stands; about the ring's middle it would have swung away.
        let mut kept = pane.camera;
        kept.orbit(egui::vec2(-80.0, 30.0));
        assert!((kept.projector(rect).at(metal) - c).length() < 0.5);
        let mut swung = pane.camera;
        swung.pivot_home();
        swung.orbit(egui::vec2(-80.0, 30.0));
        let away = (swung.projector(rect).at(metal) - c).length();
        assert!(away > 100.0, "about the ring's middle the metal swings {away} pt");
    }

    #[test]
    fn the_preview_preset_is_the_desktops_own() {
        assert_eq!(PREVIEW.theta_steps, 384);
        assert_eq!(PREVIEW.profile_steps, 144);
        assert_eq!(PREVIEW.triangle_estimate(), 110_592);
    }

    #[test]
    fn the_staged_preview_buffer_stays_under_the_memory_ceiling() {
        // 48 bytes a vertex, three a triangle. Only PREVIEW is ever staged —
        // exports build inline and write files without touching the GPU.
        let bytes = PREVIEW.triangle_estimate() * 3 * 48;
        assert!(
            bytes < 80 * 1024 * 1024,
            "{bytes} bytes is too much to re-upload"
        );
    }

    #[test]
    fn neither_preset_asks_for_a_refined_build() {
        assert!(PREVIEW.refine.is_none());
        assert!(EXPORT.refine.is_none());
    }
}
