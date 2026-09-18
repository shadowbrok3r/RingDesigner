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
use crate::viewport::{GpuMeshRenderer, ShadeMode, paint_callback};

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
    /// The two-finger twist in hand.
    pub twist: Twist,
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
            twist: Twist::default(),
        }
    }
}

impl RingPane {
    /// Draw the pane. Returns whether the camera moved (keep repainting) and
    /// the world ray under a long-press, for the tap probe.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &Arc<Mutex<GpuMeshRenderer>>,
        px_per_mm: Option<f32>,
        lock_orbit: bool,
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
            if let Some(err) = r.failed.as_ref() {
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

        let moved = !lock_orbit && self.handle_touch(ui, &response, rect);
        if moved {
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
        paint_callback(
            ui,
            rect,
            renderer.clone(),
            mvp,
            normal_matrix,
            self.shade,
            ringdesign_core::render::METAL_FINISHES[self.finish.min(6)].1,
            ringdesign_core::render::POLISHES[self.polish.min(2)].1,
            self.wireframe,
            [0.10, 0.10, 0.12],
            self.clip_plane,
            self.focus,
        );
        ViewResponse {
            response,
            rect,
            probe: probe_ray,
        }
    }

    /// One finger orbits, two pinch-zoom and pan. Returns whether anything changed.
    fn handle_touch(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) -> bool {
        let multi = ui.input(|i| i.multi_touch());
        if let Some(mt) = multi {
            // A second finger takes the gesture from the orbit outright, so a pinch never also
            // spins the ring.
            if (mt.zoom_delta - 1.0).abs() > 1e-4 {
                self.camera.zoom_by_factor(mt.zoom_delta);
            }
            if mt.translation_delta != egui::Vec2::ZERO {
                self.camera.pan_by(mt.translation_delta, rect);
            }
            // Two fingers turning about each other roll the view, which is
            // the one way to stand the ring on its head: pitch stops at the poles.
            if !self.navigation.locked {
                self.twist.advance(&mut self.camera.roll, Some(mt.rotation_delta));
            }
            return true;
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
    /// The built mesh, kept for the tap probe's raycast.
    pub mesh: Arc<ringdesign_core::mesh::Mesh>,
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
}

struct Job {
    generation: u64,
    design: RingDesign,
    lib: Arc<AlphaLibrary>,
    params: BuildParams,
    analyze: bool,
    gems: bool,
    view_layer: Option<usize>,
    cuts: Cuts,
}

/// How made settings show in the preview: resolved live into the mesh, and whether their cutters are
/// drawn over it as a ghost. Off, the ring builds as its stock alone, which is also the faster build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cuts {
    pub live: bool,
    pub ghost: bool,
}

impl Default for Cuts {
    fn default() -> Self {
        Self { live: true, ghost: false }
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
                while let Ok(mut job) = jobs_rx.recv() {
                    // Skip stale work: only the newest queued job matters.
                    while let Ok(newer) = jobs_rx.try_recv() {
                        job = newer;
                    }
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
                    let shown = if job.cuts.live { None } else { Some(ringdesign_core::setting::without_solids(&job.design)) };
                    let out = match ringdesign_core::mesh::try_build(shown.as_ref().unwrap_or(&job.design), &job.lib, job.params) {
                        Ok(out) => out,
                        Err(e) => { let _ = error_tx.send((job.generation, e.to_string())); ctx.request_repaint(); continue; }
                    };
                    let cast = job.analyze.then(|| {
                        castability::analyze(
                            &out.mesh,
                            &job.design.draft,
                            job.design.inner_radius_mm(),
                        )
                    });
                    let field = job.analyze.then(|| {
                        field_from_graph.unwrap_or_else(|| {
                            castability::attributed_field_report(
                                &job.design,
                                &job.lib,
                                &job.design.draft,
                                160,
                                112,
                            )
                        })
                    });
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
                    // Isolation is a rendering operation. All reports above use
                    // the complete source; exports never receive this copy.
                    let out = if view_layer.is_some() {
                        ringdesign_core::mesh::build(&visible, &job.lib, job.params)
                    } else {
                        out
                    };
                    let verts = GpuMeshRenderer::stage(
                        &out.mesh,
                        if view_layer.is_some() {
                            None
                        } else {
                            cast.as_ref()
                        },
                        (
                            job.design.inner_radius_mm(),
                            job.design.draft.min_section_mm,
                        ),
                    );
                    let gems = if job.gems {
                        ringdesign_core::gems::preview_vertices(&visible, &job.lib)
                    } else {
                        Vec::new()
                    };
                    let mesh = Arc::new(out.mesh);
                    let done = Done {
                        generation: job.generation,
                        verts,
                        gems,
                        ghost,
                        bounds: mesh.bounds(),
                        mesh: mesh.clone(),
                        triangles: report.validation.triangle_count,
                        volume_mm3: report.volume_mm3,
                        build_ms: report.build_ms,
                        report,
                        cast,
                        field,
                        graph,
                        params: job.params,
                        isolated: view_layer.is_some(),
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
