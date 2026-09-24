//! The 3D viewport, drawn through the renderer the desktop and the phone share
//! (`ringdesign_workbench::render`): the app keeps its camera and UI and hands it buffers.

use std::sync::{Arc, Mutex};

use egui_glow::glow;

use ringdesign_core::castability::{CastReport, FaceClass};
use ringdesign_core::interaction::pick::Entity;
use ringdesign_core::mesh::{Mesh, Vec3};
use ringdesign_workbench::render::{self, EdgeKey, Frame, Look, MeshRenderer, Shade};

use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::theme;
use ringdesign_workbench::viewport::{MenuAction, MenuItem, Mods, Sel};
use ringdesign_core::cad::edit::CadEdit;

pub use render::{HOVER_TINT, SELECT_TINT, wall_color};

/// How far from the pointer a part's vertex or edge still answers, in pixels.
pub const APERTURE_PX: f32 = 8.0;

#[cfg(test)]
thread_local! {
    /// How many times the Ring viewport staged the selection's channel on the UI thread, for the tests to read.
    pub(crate) static SELECT_STAGED_HERE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The desktop's handle on the shared renderer, under the names the app calls it by.
pub struct GpuMeshRenderer {
    inner: MeshRenderer,
    /// Paint times, gathered while `RD_RENDER_STATS` is set.
    timings: Option<render::Timings>,
}

impl Default for GpuMeshRenderer {
    fn default() -> Self {
        let mut inner = MeshRenderer::new(Look::DESKTOP);
        let timing = render::Timing::from_env(std::env::var("RD_RENDER_STATS").ok().as_deref());
        inner.set_timing(timing);
        Self { inner, timings: (timing != render::Timing::Off).then(render::Timings::default) }
    }
}

impl GpuMeshRenderer {
    /// Flattens the mesh with its draft classes and the wall heatmap of
    /// `wall = (inner_radius_mm, min_section_mm)` baked in, awaiting upload.
    pub fn prepare_upload(&mut self, mesh: &Mesh, cast: Option<&CastReport>, wall: (f64, f64)) {
        self.inner.set_mesh(render::stage_mesh(mesh, cast, wall));
    }

    /// A CAD-only ring: sharp corners stay sharp while smooth faces stay smooth.
    pub fn prepare_cad(&mut self, mesh: &Mesh) {
        self.inner.set_mesh(render::stage_cad(mesh));
    }

    /// Queues metal another thread staged with [`render::stage_mesh`] or [`render::stage_cad`].
    pub fn prepare_staged(&mut self, verts: Vec<f32>) {
        self.inner.set_mesh(verts);
    }

    /// Queues `build`'s edges staged by [`render::stage_edges`], so the next [`sync_edges`](Self::sync_edges) finds them current.
    pub fn prepare_edges(&mut self, build: &Arc<ringdesign_core::BuildResult>, staged: render::StagedEdges) {
        self.inner.set_edges(staged, Arc::as_ptr(build) as usize);
    }

    /// Queues edges staged for a view this renderer keys by `key` rather than by a build; empty draws none.
    pub fn prepare_view_edges(&mut self, key: usize, staged: render::StagedEdges) {
        self.inner.set_edges(staged, key);
    }

    /// Per-vertex weights in the order [`prepare_upload`](Self::prepare_upload) emits vertices.
    pub fn stage_focus(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        render::stage_focus(mesh, weight)
    }

    /// Queues a focus channel staged for the mesh on screen; empty clears it.
    pub fn prepare_focus(&mut self, weights: Vec<f32>) {
        self.inner.set_focus(weights);
    }

    /// The selection channel, `(chosen, hovered)` a vertex from `viewport::tint`'s weights.
    pub fn stage_select(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        render::stage_select(mesh, weight)
    }

    /// Queues a selection channel staged by [`stage_select`](Self::stage_select); empty clears it.
    pub fn prepare_select(&mut self, weights: Vec<f32>) {
        self.inner.set_select(weights);
    }

    /// Queues the stone previews; empty clears them.
    pub fn prepare_gems(&mut self, verts: Vec<f32>) {
        self.inner.set_gems(verts);
    }

    /// The seats' cutters, drawn through the metal; empty clears them.
    pub fn prepare_cutters(&mut self, verts: Vec<f32>) {
        self.inner.set_cutters(verts);
    }

    /// The pinned comparison design; empty clears it.
    pub fn prepare_ghost(&mut self, verts: Vec<f32>) {
        self.inner.set_comparison(verts);
    }

    /// A live command's ghost in its own coordinates; empty clears it.
    pub fn prepare_preview(&mut self, verts: Vec<f32>) {
        self.inner.set_preview(verts);
    }

    /// Where the ghost stands: model matrix and normal matrix, column-major; `None` hides it.
    pub fn set_preview_model(&mut self, model: Option<([f32; 16], [f32; 9])>) {
        self.inner.set_preview_model(model);
    }

    /// Whether the ghost shades in its staged draft colours.
    pub fn set_preview_draft(&mut self, draft: bool) {
        self.inner.set_preview_draft(draft);
    }

    /// The ghost's vertices awaiting upload, and whether it shades by draft class.
    #[cfg(test)]
    pub fn staged_preview(&self) -> (Option<&[f32]>, bool) {
        let (verts, _, draft) = self.inner.preview_state();
        (verts, draft)
    }

    /// A part's tessellation, a vertex normal kept only within 20° of its facet's.
    pub fn stage_part(mesh: &Mesh) -> Vec<f32> {
        render::stage_part(mesh)
    }

    /// [`stage_part`](Self::stage_part) with every face in its draft class's colour.
    pub fn stage_part_classes(mesh: &Mesh, classes: &[FaceClass]) -> Vec<f32> {
        render::stage_part_classes(mesh, classes)
    }

    /// A mesh in neutral colours; the pass that draws it supplies its own tint.
    pub fn stage_plain(mesh: &Mesh) -> Vec<f32> {
        render::stage_plain(mesh)
    }

    /// The build the edges were staged for, and each edge's run in them.
    #[cfg(test)]
    pub fn staged_edges(&self) -> (Option<usize>, &[render::EdgeRun]) {
        (self.inner.edges_key(), self.inner.edge_runs())
    }

    /// Keeps the edge pass on the build on screen and the chosen and hovered edges.
    pub fn sync_edges(&mut self, build: &Arc<ringdesign_core::BuildResult>, chosen: &[EdgeKey], hovered: Option<EdgeKey>) -> bool {
        let key = Arc::as_ptr(build) as usize;
        let fresh = self.inner.edges_key() != Some(key);
        let start = (fresh && self.timings.is_some()).then(std::time::Instant::now);
        let staged = self.inner.sync_edges(key, build.parts.evaluated.as_ref(), chosen, hovered);
        if let Some(start) = start {
            eprintln!("render-stats stage-edges runs={} us={:.0}", self.inner.edge_runs().len(), start.elapsed().as_secs_f32() * 1e6);
        }
        staged
    }

    /// Draws the frame, then says once what the context turned out to be.
    fn paint(&mut self, gl: &glow::Context, info: &egui::PaintCallbackInfo, frame: &Frame) {
        let before = self.inner.stats().frames;
        self.inner.paint(gl, info, frame);
        for note in self.inner.take_notes() {
            match note {
                render::GlNote::Ready { version, profile } => log::info!("ring renderer: {version} ({profile:?})"),
                render::GlNote::Depth(bits) if bits <= 0 => log::warn!(
                    "no depth buffer on the default framebuffer ({bits} bits): the ring will draw \
                     see-through. NativeOptions::depth_buffer must be non-zero."
                ),
                render::GlNote::Depth(bits) => log::info!("depth buffer: {bits} bits"),
                render::GlNote::Failed(e) => log::error!("ring renderer: {e}"),
            }
        }
        let stats = self.inner.stats();
        if let Some(t) = self.timings.as_mut().filter(|_| stats.frames > before) {
            if let Some(line) = t.record(&stats) {
                eprintln!("{line}");
            }
        }
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        self.inner.destroy(gl);
    }

    /// Whether paints are being timed, which keeps the viewport repainting.
    pub fn timed(&self) -> bool {
        self.timings.is_some()
    }
}

/// Queues a draw of `renderer` into `rect`.
fn paint_callback(rect: egui::Rect, renderer: Arc<Mutex<GpuMeshRenderer>>, frame: Frame) -> egui::PaintCallback {
    let paint = egui_glow::CallbackFn::new(move |info, painter| {
        if let Ok(mut r) = renderer.lock() {
            r.paint(painter.gl(), &info, &frame);
        }
    });
    egui::PaintCallback { rect, callback: Arc::new(paint) }
}

/// The chosen edges and the edge under the pointer, as the edge pass lights them.
fn lit_edges(app: &RingDesignerApp) -> (Vec<EdgeKey>, Option<EdgeKey>) {
    let chosen = app.selection.items.iter().filter_map(|s| match s {
        Sel::Edge { feature, edge } => Some((*feature, *edge)),
        _ => None,
    });
    let hovered = app.selection.hover.as_ref().and_then(|h| match h.entity {
        Entity::Edge { feature, edge } => Some((feature, edge)),
        _ => None,
    });
    (chosen.collect(), hovered)
}

// --- Metal finishes and lighting -------------------------------------------

/// Display color per alloy family. Rendering only; density stays in core.
pub struct Finish {
    pub name: &'static str,
    pub rgb: [f32; 3],
}

pub const FINISHES: &[Finish] = &[
    Finish {
        name: "Yellow gold",
        rgb: [0.86, 0.70, 0.42],
    },
    Finish {
        name: "Rose gold",
        rgb: [0.84, 0.60, 0.49],
    },
    Finish {
        name: "Silver",
        rgb: [0.79, 0.80, 0.81],
    },
    Finish {
        name: "White gold",
        rgb: [0.83, 0.83, 0.80],
    },
    Finish {
        name: "Platinum",
        rgb: [0.75, 0.76, 0.78],
    },
    Finish {
        name: "Bronze",
        rgb: [0.72, 0.53, 0.35],
    },
    Finish {
        name: "Brass",
        rgb: [0.80, 0.65, 0.36],
    },
];

/// A key-light direction with an ambient floor.
pub struct LightRig {
    pub name: &'static str,
    pub dir: [f32; 3],
    pub ambient: f32,
}

pub const LIGHT_RIGS: &[LightRig] = &[
    LightRig {
        name: "Studio",
        dir: [-0.38, 0.46, 0.80],
        ambient: 0.20,
    },
    LightRig {
        name: "Window",
        dir: [0.62, 0.25, 0.74],
        ambient: 0.28,
    },
    LightRig {
        name: "Low sun",
        dir: [-0.75, -0.18, 0.64],
        ambient: 0.12,
    },
];

// --- Shading modes ---------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShadeMode {
    Metal,
    Draft,
    Wall,
    Halves,
    Normals,
}

impl ShadeMode {
    pub const ALL: &'static [ShadeMode] = &[
        ShadeMode::Metal,
        ShadeMode::Draft,
        ShadeMode::Wall,
        ShadeMode::Halves,
        ShadeMode::Normals,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShadeMode::Metal => "Polished metal",
            ShadeMode::Draft => "Draft check",
            ShadeMode::Wall => "Wall thickness",
            ShadeMode::Halves => "Cope / drag",
            ShadeMode::Normals => "Normals",
        }
    }

    fn gl_mode(self) -> Shade {
        match self {
            ShadeMode::Metal => Shade::Metal,
            ShadeMode::Draft => Shade::Draft,
            ShadeMode::Wall => Shade::Wall,
            ShadeMode::Halves => Shade::Halves,
            ShadeMode::Normals => Shade::Normals,
        }
    }
}

// --- Viewport --------------------------------------------------------------

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    use ringdesign_workbench::visual::{Pointer, Tool};
    let follow_node = app.panes[pane].follow_node;
    let active = pane == app.active_pane && !follow_node;
    if active {
        app.visual.poll();
        if app.visual.tool != Tool::Select {
            let mut open = true;
            egui::Window::new(app.visual.tool.label())
                .frame(egui::Frame::window(ui.style()).fill(theme::FLOAT))
                .id(egui::Id::new("direct-viewport-inspector"))
                .open(&mut open)
                .default_width(180.0).min_width(150.0)
                .pivot(egui::Align2::RIGHT_TOP)
                .default_pos(ui.max_rect().right_top() + egui::vec2(-12.0, 44.0))
                .constrain_to(ui.ctx().content_rect())
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(460.0)
                        .show(ui, |ui| {
                            app.visual.controls(ui, &app.design, &app.lib);
                            if let Some(layer) = app.visual.apply_controls(&mut app.design) {
                                app.selected_layer = Some(layer);
                                app.mark_dirty();
                                app.history.commit(&app.design);
                            }
                            if app.visual.tool == Tool::Clearance
                                && app.visual.stone_controls(ui, &mut app.design)
                            {
                                app.mark_dirty();
                            }
                        });
                });
            if !open {
                app.visual.select(Tool::Select);
            }
        }
        stamp_inspector(app, ui);
    }
    let mould_active = active && app.visual.tool == Tool::Mould && app.visual.study.is_some();
    if let Some((previous, camera)) = app.mould_camera {
        if app.visual.tool != Tool::Mould
            || app.visual.study.is_none()
            || previous != app.active_pane
        {
            if let Some(p) = app.panes.get_mut(previous) {
                p.camera = camera;
            }
            app.mould_camera = None;
        }
    }
    if mould_active {
        if let Some(study) = &app.visual.study {
            if app.mould_serial != app.visual.study_serial {
                if let Ok(mut renderer) = app.mould_renderer.lock() {
                    renderer.prepare_upload(
                        &study.pattern,
                        None,
                        (
                            app.design.inner_radius_mm() * study.scale,
                            app.design.draft.min_section_mm,
                        ),
                    );
                    renderer.prepare_gems(Vec::new());
                }
                app.mould_serial = app.visual.study_serial;
            }
            if app.mould_camera.is_none() {
                app.mould_camera = Some((pane, app.panes[pane].camera));
                app.panes[pane].camera.zoom = 1.0;
                let bounds = study.pattern.bounds().map(|(a, b)| {
                    let p = study.report.frame.z.map(|v| v.abs() as f32 * 16.0);
                    (
                        Vec3(a.0 - p[0], a.1 - p[1], a.2 - p[2]),
                        Vec3(b.0 + p[0], b.1 + p[1], b.2 + p[2]),
                    )
                });
                app.panes[pane].camera.fit(bounds);
            }
        }
    }
    if app.visual.wants_repaint() {
        ui.ctx().request_repaint();
    }
    let (rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return;
    }

    if !follow_node && app.band_paint && !response.hovered() && !app.panes[pane].navigation.locked {
        let camera = &mut app.panes[pane].camera;
        if let Some(pose)=ringdesign_workbench::paint_preview::follow(ui,camera.pose(),camera.target,camera.half_extent()*camera.zoom/1.15) {
            camera.set_pose(pose); app.panes[pane].turn=None;
        }
    }
    let head = app.design.shank.head.theta_deg as f32;
    let camera = app.panes[pane].camera;
    let nav = if follow_node {
        ringdesign_workbench::navigation::Response { rect: egui::Rect::NOTHING, action: None, changed: false, controls: Vec::new() }
    } else {
        ringdesign_workbench::navigation::show(ui, rect, ui.id().with(("desktop-view", pane)),
            &mut app.panes[pane].navigation, [camera.yaw, camera.pitch, camera.roll], head)
    };
    if let Some(action) = nav.action {
        let angles = action.apply([camera.yaw, camera.pitch, camera.roll], head);
        if action.recentres() {
            // A view from the cube eases in, as a chosen node's does, about the ring's own middle again.
            let cam = &mut app.panes[pane].camera;
            if let Some(ring) = app.build.as_ref().and_then(|b| b.mesh.bounds()) {
                cam.refit(ring);
            }
            cam.pivot_home();
            let from = cam.pose();
            app.panes[pane].turn = Some(ringdesign_workbench::focus::Turn::new(from, ringdesign_workbench::focus::Pose { yaw: angles[0], pitch: angles[1], roll: angles[2], pan: [0.0; 2], ..from }));
        } else {
            app.panes[pane].turn = None;
            app.panes[pane].camera.yaw = angles[0];
            app.panes[pane].camera.pitch = angles[1];
            app.panes[pane].camera.roll = angles[2];
        }
    }
    if let Some(turn) = app.panes[pane].turn {
        let (pose, arrived) = turn.now();
        app.panes[pane].camera.set_pose(pose);
        if arrived {
            app.panes[pane].turn = None;
        }
        ui.ctx().request_repaint();
    }
    // A live sketch, then the command layer, take their keys, the pointer and their clicks before the viewport's own handling.
    let sketching = active && crate::sketch_mode::active(app);
    let took = if sketching {
        crate::sketch_mode::input(app, ui, pane, rect, &response)
    } else if active {
        crate::command::input(app, ui, pane, rect, &response)
    } else {
        crate::command::Took::default()
    };
    // Work planes answer the pointer where their outline or name is, as the gizmo's handles do.
    let planes = if follow_node || app.command.planes.hidden { Vec::new() } else { plane_shapes(app) };
    let plane_proj = app.panes[pane].camera.projector(rect);
    let plane_painter = ui.painter_at(rect);
    let plane_pointer = active && !took.live && !took.boxing && !sketching && app.visual.tool == Tool::Select && app.command.gizmo_hot().is_none();
    let plane_under = |pos: Option<egui::Pos2>| pos.filter(|_| plane_pointer).and_then(|p| plane_at(&planes, &plane_proj, &plane_painter, p));
    app.command.planes.hot = plane_under(response.hover_pos());
    if response.secondary_clicked() && !took.secondary {
        app.command.planes.menu = plane_under(response.interact_pointer_pos());
        // What the menu is about: the best pick under the pointer, with the band's own raycast as
        // the fallback before the first scene is built.
        let camera = app.panes[pane].camera;
        let under = response.interact_pointer_pos().and_then(|pos| {
            let ray = |p: egui::Pos2| camera.ray(rect, p);
            match (&app.pick_scene, &app.build) {
                (Some(scene), _) => app.picks_now(ringdesign_workbench::hover::pick_at(scene, pos, &ray, APERTURE_PX, app.selection.filter)).into_iter().next(),
                (None, Some(b)) => {
                    let (origin, direction) = ray(pos);
                    ringdesign_core::interaction::picking::raycast(&b.mesh, origin, direction).map(|(face, point)| ringdesign_core::interaction::pick::Pick {
                        entity: ringdesign_core::interaction::pick::Entity::Band,
                        world: point.map(f64::from),
                        normal: b.mesh.face_normal(&b.mesh.faces[face]).unwrap_or([0.0, 0.0, 1.0]),
                        depth: 0.0,
                        px: 0.0,
                    })
                }
                (None, None) => None,
            }
        });
        app.selection.under = under;
        app.cad.ring_menu_hit = app.selection.under.as_ref().map(|p| p.world.map(|v| v as f32));
    }
    if !took.secondary && !took.live {
        response.context_menu(|ui| {
            ui.set_min_width(190.);
            match app.command.planes.menu {
                Some(plane) => plane_menu(app, ui, pane, plane),
                None => show_menu(app, ui, pane),
            }
        });
    }
    let shift = ui.input(|i| i.modifiers.shift);
    let explicit_navigation = shift || ui.input(|i| i.pointer.middle_down() || i.multi_touch().is_some_and(|m| m.num_touches >= 2));
    let camera = app.panes[pane].camera;
    let projector = camera.projector(rect);
    let floating_blocked = ui.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos())).is_some_and(|p| {
        nav.rect.contains(p) || ui.ctx().layer_id_at(p).is_some_and(|layer| layer != ui.layer_id())
    });
    let blocked = active && app.build.as_ref().is_some_and(|b| {
        app.visual.route_pointer(ui, rect, &b.mesh, |p| projector.at(p.map(|v| v as f32)),
            |p| camera.ray(rect, p), explicit_navigation, !floating_blocked)
    });
    let navigating = explicit_navigation || app.visual.navigating();
    let locked = app.panes[pane].navigation.locked;
    let scroll = if response.hovered() {
        ui.input(|i| i.smooth_scroll_delta.y)
    } else {
        0.0
    };
    {
        let Some(cam) = app.panes.get_mut(pane).map(|p| &mut p.camera) else {
            return;
        };
        if response.dragged_by(egui::PointerButton::Primary) && !blocked && !floating_blocked && !follow_node && !took.drag {
            let delta = response.drag_delta();
            if shift || locked {
                cam.pan_by(delta, rect);
            } else {
                cam.orbit(delta);
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) && !follow_node {
            cam.pan_by(response.drag_delta(), rect);
        }
        if scroll != 0.0 {
            cam.zoom_by(scroll);
        }
    }
    let camera = app.panes[pane].camera;
    let shade = if mould_active {
        ShadeMode::Metal
    } else {
        app.panes[pane].shade
    };
    let proj = camera.projector(rect);

    if response.clicked() && !took.click && (!active || app.visual.tool == Tool::Select) {
        if let Some(pos) = response.interact_pointer_pos() {
            app.active_pane = pane;
            if let Some(plane) = plane_under(Some(pos)) {
                choose_plane(app, &planes, plane);
            } else {
                app.command.planes.chosen = None;
                let mods = ui.input(|i| ringdesign_workbench::viewport::Mods { shift: i.modifiers.shift, ctrl: i.modifiers.command, alt: i.modifiers.alt });
                select_click(app, camera, rect, pos, mods);
            }
        }
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);

    if app.show_grid {
        draw_grid(app, &painter, &proj, camera.half_extent());
    }

    if app.build.is_some() {
        let (mvp, normal_matrix) = camera.matrices(rect);
        let rig = &LIGHT_RIGS[app.light.min(LIGHT_RIGS.len() - 1)];
        let renderer = if mould_active {
            app.mould_renderer.clone()
        } else {
            app.renderer.clone()
        };
        let frame = Frame {
            shade: shade.gl_mode(),
            base_color: ringdesign_core::render::METAL_FINISHES[app.finish.min(FINISHES.len() - 1)].1,
            roughness: ringdesign_core::render::POLISHES[app.polish.min(2)].1,
            light_dir: rig.dir,
            ambient: rig.ambient,
            wire: app.show_wireframe.then(|| rgb_of(theme::TEXT_DIM)),
            gems: app.show_gems,
            clip_plane: if active { app.visual.clip() } else { [0.0; 4] },
            focus: if mould_active { [0.0; 4] } else { app.node_focus.tint },
            select: if mould_active { [0.0; 4] } else { SELECT_TINT },
            hover: if mould_active { [0.0; 4] } else { HOVER_TINT },
            edges: !mould_active && app.show_part_edges,
            ..Frame::new(mvp, normal_matrix)
        };
        if renderer.lock().is_ok_and(|r| r.timed()) {
            ui.ctx().request_repaint();
        }
        painter.add(paint_callback(rect, renderer, frame));
        // What the chosen graph node does, said on the ring it is shown on.
        if let Some(words) = app.node_words() {
            let galley = painter.layout_no_wrap(words, egui::FontId::proportional(12.0), egui::Color32::from_rgb(255, 110, 168));
            let at = egui::pos2(rect.center().x - galley.size().x * 0.5, rect.bottom() - 46.0);
            painter.rect_filled(egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(7.0, 4.0)), 5.0, egui::Color32::from_black_alpha(190));
            painter.galley(at, galley, egui::Color32::WHITE);
        }
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Building…",
            egui::FontId::proportional(15.0),
            theme::TEXT_DIM,
        );
    }

    // The tool rail stands at the left edge; the corner overlays start beside it.
    let overlay = if follow_node { rect } else { rect.with_min_x(rect.left() + crate::command::RAIL_W) };
    if app.show_grid {
        draw_axes(&painter, &proj, overlay);
    }

    draw_section_marker(app, &painter, &proj);
    draw_legend(app, shade, &painter, overlay);
    draw_probe(app, &painter, &proj, rect);
    if !follow_node {
        crate::ring_snaps::draw_pins(app, &painter, &proj, rect);
    }
    // Measure picks off the pick scene here; every other tool draws itself.
    if active && !floating_blocked && app.visual.tool == Tool::Measure {
        crate::ring_snaps::measure(app, ui, pane, rect, &response, &painter, &proj);
    } else if active && !floating_blocked {
        if let Some(build) = app.build.clone() {
            let camera = app.panes[pane].camera;
            let proj = camera.projector(rect);
            let edit = app.visual.draw(
                ui,
                rect,
                &response,
                &mut app.design,
                &app.lib,
                &build.mesh,
                |p| proj.at(p.map(|v| v as f32)),
                |p| camera.ray(rect, p),
                Pointer {
                    navigating,
                    ..Default::default()
                },
            );
            if let Some(index) = edit.drawing {
                let alpha = app.design.drawn[index].rasterize();
                app.library_mut().insert(alpha);
            }
            if let Some(layer) = edit.layer {
                app.selected_layer = Some(layer);
            }
            if edit.changed() {
                app.mark_dirty();
                app.history.commit(&app.design);
                ui.ctx().request_repaint();
            }
        }
    }

    if app.band_paint { ringdesign_workbench::paint_preview::draw(ui,rect,|p|proj.at(p)); }

    // A hot gizmo handle stands in for the scene's hover.
    if active && app.visual.tool == Tool::Select && !took.live && !took.boxing && app.command.gizmo_hot().is_none() && app.command.planes.hot.is_none() {
        app.hovered_node = None;
        // The scene answers first; a band under the pointer falls through to the layer caption and
        // the node highlight it always had.
        let picks = app.pick_scene.clone().and_then(|scene| ringdesign_workbench::hover::picks(ui, rect, &response, &scene, |p| camera.ray(rect, p), APERTURE_PX, app.selection.filter)).map(|p| app.picks_now(p));
        let on_band = picks.as_ref().is_none_or(|p| p.first().is_none_or(|p| p.entity == ringdesign_core::interaction::pick::Entity::Band));
        app.selection.hovered(picks.unwrap_or_default());
        if on_band {
            if let Some(build) = &app.build {
                let hit = ringdesign_workbench::hover::show(ui, rect, &response, &app.design, &app.lib, &build.mesh,
                    |p| camera.ray(rect,p), |p| proj.at(p), |hit| {
                        let layer = ringdesign_workbench::focus::layer_behind(&app.design,&app.lib,hit);
                        if app.graph_driven() { layer.and_then(|i| app.node_for_layer(i)).map(|id| app.graph_ed.as_ref().and_then(|ed| ed.card(id)).map_or("Graph feature".into(), |card| card.title.clone())) }
                        else if app.design.band_is_procedural() { Some(layer.map_or("Band / shape".into(), |i| app.design.layers.layers[i].name.clone())) } else { None }
                    });
                app.hovered_node = hit.and_then(|hit| ringdesign_workbench::focus::layer_behind(&app.design,&app.lib,&hit)).and_then(|i| app.node_for_layer(i));
            }
        } else if let Some(h) = &app.selection.hover {
            let (at, of) = app.selection.depth();
            let mut text = ringdesign_workbench::viewport::label(&h.entity, &app.design, app.build.as_deref());
            if of > 1 {
                text = format!("{text} · Tab {}/{of}", at + 1);
            }
            ringdesign_workbench::hover::caption(&painter, overlay, &text, ringdesign_workbench::hover::AQUA);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.hovered() && app.selection.stack().len() > 1 && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
            app.selection.cycle();
        }
    } else if active {
        app.selection.hovered(Vec::new());
        if let Some(s) = app.command.planes.hot.and_then(|id| planes.iter().find(|s| s.id == id)) {
            ringdesign_workbench::hover::caption(&painter, overlay, &format!("Work plane: {} · right-click to sketch on it or mirror across it", s.name), ringdesign_workbench::hover::AQUA);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    // The selection's channel, restaged when it changes; a new build brings its own from the worker while the selection held still.
    if let Some(build) = &app.build {
        let key = Arc::as_ptr(build) as usize;
        if app.selection.needs_stage(key) {
            #[cfg(test)]
            SELECT_STAGED_HERE.with(|c| c.set(c.get() + 1));
            let weights = ringdesign_workbench::viewport::tint(&app.selection, build);
            let staged = if weights.is_empty() { Vec::new() } else { GpuMeshRenderer::stage_select(&build.mesh, &weights) };
            if let Ok(mut r) = app.renderer.lock() {
                r.prepare_select(staged);
            }
            ui.ctx().request_repaint();
        }
        // The edge pass follows the build on screen and the chosen and hovered edges.
        let (chosen, hovered) = lit_edges(app);
        if app.renderer.lock().is_ok_and(|mut r| r.sync_edges(build, &chosen, hovered)) {
            ui.ctx().request_repaint();
        }
    }
    draw_selection(app, &painter, &proj, rect);
    draw_planes(app, ui, pane, &planes, &painter, &proj);
    if !follow_node {
        crate::command::draw(app, ui, pane, &response, &painter, &proj, active);
        crate::sketch_mode::draw(app, ui, pane, &response, &painter, &proj, active);
    }
    let label = {
        let mut s = String::from("Ring viewport");
        if let Some(h) = &app.selection.hover {
            s.push_str(&format!(" · hovering {}", ringdesign_workbench::viewport::label(&h.entity, &app.design, app.build.as_deref())));
        }
        if !app.selection.items.is_empty() {
            s.push_str(&format!(" · {} selected", app.selection.items.len()));
        }
        if let Some(plane) = app.command.planes.hot.and_then(|id| planes.iter().find(|p| p.id == id)) {
            s.push_str(&format!(" · work plane {} under the pointer", plane.name));
        }
        if let Some(c) = app.command.session.command() {
            s.push_str(&format!(" · {} live", c.title()));
        }
        if let Some(ghost) = app.command.ghost_caption() {
            s.push_str(&format!(" · {ghost}"));
        }
        if app.command.box_armed {
            s.push_str(" · box select armed");
        }
        if crate::sketch_mode::active(app) {
            s.push_str(" · sketching");
        }
        s
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));

    if active && app.panes[pane].navigation.magnifier && !floating_blocked && !navigating {
        if let Some((contact, reach)) = app.visual.placement_focus(ui, rect) {
            ringdesign_workbench::loupe::show(ui, rect, contact, reach, &[nav.rect]);
        }
    }
    painter.text(
        rect.right_bottom() - egui::vec2(12.0, 9.0),
        egui::Align2::RIGHT_BOTTOM,
        if follow_node { "Selected feature · scroll to zoom" } else if locked { "View locked • Drag empty space to pan • Scroll to zoom" } else if rect.width() < 420. { "Drag to orbit · scroll to zoom" } else { "Drag empty space to orbit • Shift-drag to pan • Scroll to zoom" },
        egui::FontId::proportional(11.0),
        theme::TEXT_DIM,
    );
}

/// Candidate display controls share navigation behavior with the main viewport.
pub struct CandidateDisplay {
    pub navigation: ringdesign_workbench::navigation::Settings,
    pub turn: Option<ringdesign_workbench::focus::Turn>,
    pub wire: bool,
    pub grid: bool,
    pub finish: usize,
    pub polish: usize,
    pub light: usize,
    pub show_gems: bool,
}
impl Default for CandidateDisplay {
    fn default() -> Self { Self { navigation: Default::default(), turn: None, wire: false, grid: true, finish: 2, polish: 0, light: 0, show_gems: true } }
}

/// Independent CAD buffers keep candidate edits separate from committed geometry.
pub fn candidate_view(
    ui: &mut egui::Ui,
    renderer: Arc<std::sync::Mutex<GpuMeshRenderer>>,
    camera: &mut crate::camera::OrbitCamera,
    display: &mut CandidateDisplay,
    head: f32,
) -> (egui::Rect, egui::Response) {
    let available = ui.available_rect_before_wrap().intersect(ui.clip_rect()).size().max(egui::vec2(1.,1.));
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
    let nav = ringdesign_workbench::navigation::show_camera(ui,rect,ui.id().with("cad-cube"),
        &mut display.navigation,[camera.yaw,camera.pitch,camera.roll],head);
    if let Some(action) = nav.action {
        let angles = action.apply([camera.yaw,camera.pitch,camera.roll],head);
        if action.recentres() {
            let from = camera.pose();
            display.turn = Some(ringdesign_workbench::focus::Turn::new(from,
                ringdesign_workbench::focus::Pose { yaw:angles[0],pitch:angles[1],roll:angles[2],pan:[0.;2],..from }));
        } else {
            display.turn = None;
            [camera.yaw,camera.pitch,camera.roll] = angles;
        }
    }
    let blocked = ui.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos())).is_some_and(|p|
        nav.rect.contains(p) || ui.ctx().layer_id_at(p).is_some_and(|layer| layer != ui.layer_id()));
    if !blocked {
        if response.dragged_by(egui::PointerButton::Primary) || response.dragged_by(egui::PointerButton::Middle) {
            display.turn = None;
            let delta = ui.input(|i| i.pointer.delta());
            if display.navigation.locked || ui.input(|i| i.modifiers.shift || i.pointer.middle_down()) {
                camera.pan_by(delta,rect);
            } else { camera.orbit(delta); }
        }
        if response.hovered() { camera.zoom_by(ui.input(|i| i.smooth_scroll_delta.y)); }
    }
    if let Some(turn) = display.turn {
        let (pose, arrived) = turn.now(); camera.set_pose(pose);
        if arrived {display.turn = None;}
        ui.ctx().request_repaint();
    }
    let (mvp,normal) = camera.matrices(rect);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect,0.,theme::VIEWPORT_BG);
    let project = camera.projector(rect);
    if display.grid {
        let step = grid_step(camera.half_extent());
        let extent = step * 12.;
        for i in -12..=12 {
            let v = i as f32 * step;
            let stroke = egui::Stroke::new(1.,if i == 0 {theme::ACCENT_DIM.gamma_multiply(0.4)} else {theme::GRID});
            painter.line_segment([project.at([-extent,v,0.]),project.at([extent,v,0.])],stroke);
            painter.line_segment([project.at([v,-extent,0.]),project.at([v,extent,0.])],stroke);
        }
    }
    let rig = &LIGHT_RIGS[display.light.min(LIGHT_RIGS.len() - 1)];
    let frame = Frame {
        base_color: ringdesign_core::render::METAL_FINISHES[display.finish.min(FINISHES.len() - 1)].1,
        roughness: ringdesign_core::render::POLISHES[display.polish.min(2)].1,
        light_dir: rig.dir,
        ambient: rig.ambient,
        wire: display.wire.then_some([0.3, 0.3, 0.3]),
        gems: display.show_gems,
        ..Frame::new(mvp, normal)
    };
    painter.add(paint_callback(rect, renderer, frame));
    draw_axes(&painter,&project,rect);
    painter.text(rect.left_bottom()+egui::vec2(12.,-14.),egui::Align2::LEFT_BOTTOM,
        "Orthographic · Drag orbit · Shift / middle drag pan · Scroll zoom",
        egui::FontId::proportional(11.),theme::TEXT_DIM);
    (rect,response)
}

/// Ground grid on the sand plane, under the ring.
fn draw_grid(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector, half: f32) {
    let step = grid_step(half);
    let lines = ((half * 1.6 / step).ceil() as i32).clamp(4, 30);
    let z = app
        .build
        .as_ref()
        .and_then(|b| b.mesh.bounds())
        .map_or(0.0, |(min, _)| min.2);

    let extent = lines as f32 * step;
    let minor = egui::Stroke::new(1.0, theme::GRID);
    let major = egui::Stroke::new(1.0, theme::ACCENT_DIM.gamma_multiply(0.40));

    for i in -lines..=lines {
        let t = i as f32 * step;
        let stroke = if i == 0 { major } else { minor };
        painter.line_segment([proj.at([-extent, t, z]), proj.at([extent, t, z])], stroke);
        painter.line_segment([proj.at([t, -extent, z]), proj.at([t, extent, z])], stroke);
    }
}

/// Grid spacing in mm, chosen so roughly nine lines cross the view.
fn grid_step(half_extent: f32) -> f32 {
    for step in [0.5f32, 1.0, 2.0, 5.0, 10.0, 20.0] {
        if half_extent / step <= 9.0 {
            return step;
        }
    }
    50.0
}

/// Corner axis indicator oriented by the current view.
fn draw_axes(painter: &egui::Painter, proj: &Projector, rect: egui::Rect) {
    let origin = proj.at([0.0, 0.0, 0.0]);
    let axes = [
        ([1.0f32, 0.0, 0.0], "X", theme::BAD),
        ([0.0, 1.0, 0.0], "Y", theme::GOOD),
        ([0.0, 0.0, 1.0], "Z", theme::INFO),
    ];

    let mut dirs = [egui::Vec2::ZERO; 3];
    let mut longest = 1e-6f32;
    for (k, (axis, _, _)) in axes.iter().enumerate() {
        dirs[k] = proj.at(*axis) - origin;
        longest = longest.max(dirs[k].length());
    }

    let scale = 26.0 / longest;
    let centre = egui::pos2(rect.left() + 46.0, rect.bottom() - 46.0);
    painter.circle_filled(centre, 2.0, theme::TEXT_DIM);

    for (k, (_, name, color)) in axes.iter().enumerate() {
        let tip = centre + dirs[k] * scale;
        painter.line_segment([centre, tip], egui::Stroke::new(1.6, *color));
        painter.text(
            centre + dirs[k] * scale * 1.3,
            egui::Align2::CENTER_CENTER,
            *name,
            egui::FontId::proportional(10.0),
            *color,
        );
    }
}

/// Draft colour key in draft mode, otherwise the size and overall dimensions.
fn draw_legend(app: &RingDesignerApp, shade: ShadeMode, painter: &egui::Painter, rect: egui::Rect) {
    let mut rows: Vec<(Option<egui::Color32>, String, egui::Color32)> = Vec::new();

    match (shade, app.cast.as_ref()) {
        (ShadeMode::Draft, Some(cast)) => {
            for (class, count) in [
                (FaceClass::Good, cast.good),
                (FaceClass::Marginal, cast.marginal),
                (FaceClass::Vertical, cast.vertical),
                (FaceClass::Undercut, cast.undercut),
            ] {
                rows.push((
                    Some(theme::class_color(class)),
                    format!("{} • {}", class.label(), count),
                    theme::TEXT,
                ));
            }
            // These are this build's faces, which is the right thing to paint
            // and the wrong thing to judge from: an irregular mesh reports a
            // phantom along the crest line that does not fall with resolution.
            rows.push((
                None,
                "mesh faces — the verdict is field-sampled".into(),
                theme::TEXT_DIM,
            ));
        }
        (ShadeMode::Wall, _) => {
            let m = app.design.draft.min_section_mm;
            let swatch = |t: f64| {
                let c = wall_color(t, m);
                egui::Color32::from_rgb(
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                )
            };
            rows.push((
                Some(swatch(m * 0.5)),
                format!("under {m:.1} mm — will not fill"),
                theme::TEXT,
            ));
            rows.push((
                Some(swatch(m * 1.5)),
                format!("{m:.1}–{:.1} mm — thin", m * 2.0),
                theme::TEXT,
            ));
            rows.push((Some(swatch(m * 2.7)), "comfortable".into(), theme::TEXT));
            rows.push((Some(swatch(m * 6.5)), "heavy".into(), theme::TEXT));
            if let Some(f) = app.field.as_ref() {
                rows.push((
                    None,
                    format!(
                        "thinnest {:.2} mm at {:.0}°",
                        f.thinnest_wall_mm, f.thinnest_wall_theta_deg
                    ),
                    theme::TEXT_DIM,
                ));
            }
        }
        _ => {
            let Some(build) = app.build.as_ref() else {
                return;
            };
            let r = &build.report;
            rows.push((None, app.design.size.display(), theme::TEXT));
            rows.push((
                None,
                format!(
                    "{:.2} mm outside dia • {:.2} mm wide",
                    r.outer_diameter_mm, r.band_width_mm
                ),
                theme::TEXT_DIM,
            ));
            rows.push((
                None,
                format!(
                    "{:.2} x {:.2} x {:.2} mm overall",
                    r.bounds_mm[0], r.bounds_mm[1], r.bounds_mm[2]
                ),
                theme::TEXT_DIM,
            ));
        }
    }

    if rows.is_empty() {
        return;
    }

    let font = egui::FontId::proportional(11.0);
    let galleys: Vec<_> = rows
        .iter()
        .map(|(_, text, color)| painter.layout_no_wrap(text.clone(), font.clone(), *color))
        .collect();

    let swatch = 9.0f32;
    let text_x = if rows.iter().any(|(c, _, _)| c.is_some()) {
        swatch + 7.0
    } else {
        0.0
    };
    let line_h = 16.0f32;
    let pad = egui::vec2(9.0, 7.0);
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + text_x;
    let at = rect.left_top() + egui::vec2(12.0, 12.0);
    let panel = egui::Rect::from_min_size(
        at,
        egui::vec2(width, line_h * rows.len() as f32) + pad * 2.0,
    );

    painter.rect_filled(panel, 5.0, theme::PANEL.gamma_multiply(0.88));
    painter.rect_stroke(
        panel,
        5.0,
        egui::Stroke::new(1.0, theme::GRID),
        egui::StrokeKind::Inside,
    );

    for (i, ((color, _, text_color), galley)) in rows.iter().zip(galleys).enumerate() {
        let y = at.y + pad.y + i as f32 * line_h;
        if let Some(c) = color {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(at.x + pad.x, y + (line_h - swatch) * 0.5),
                    egui::Vec2::splat(swatch),
                ),
                2.0,
                *c,
            );
        }
        let ty = y + (line_h - galley.size().y) * 0.5;
        painter.galley(egui::pos2(at.x + pad.x + text_x, ty), galley, *text_color);
    }
}

fn rgb_of(c: egui::Color32) -> [f32; 3] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
    ]
}

// --- Surface probe -----------------------------------------------------------

fn probe_click(
    app: &mut RingDesignerApp,
    camera: crate::camera::OrbitCamera,
    rect: egui::Rect,
    pos: egui::Pos2,
) {
    let Some(build) = app.build.clone() else {
        return;
    };
    let (origin, dir) = camera.ray(rect, pos);
    let Some(hit) = ringdesign_core::interaction::picking::hit(&app.design, &app.lib, &build.mesh, origin, dir) else {
        app.clear_selection();
        return;
    };
    let world = hit.world;

    let theta = hit.theta_deg;
    let v_mm = hit.v_mm;
    let h = hit.relief_mm;
    let fi = hit.face;
    let class = app
        .cast
        .as_ref()
        .and_then(|c| c.classes.get(fi))
        .map(|k| k.label())
        .unwrap_or("—");

    let layer = ringdesign_workbench::focus::layer_behind(&app.design, &app.lib, &hit);
    let named = layer.map(|i| (i, app.design.layers.layers[i].name.clone()));
    app.selected_layer = layer;
    if app.graph_ed.is_some() {
        let node = layer.and_then(|i| app.node_for_layer(i));
        app.selected_node = node;
        if let Some(ed) = app.graph_ed.as_mut() {
            ed.selected = node;
            if let Some(node) = node { ed.focus(node); }
        }
        if node.is_some() && !app.dock.is_open(crate::dock::ToolKind::Node) {
            app.dock.open_on(crate::dock::ToolKind::Node, crate::dock::Side::Right);
        }
    } else {
        let tool = if layer.is_some() { crate::dock::ToolKind::Layers } else { crate::dock::ToolKind::Design };
        if !app.dock.is_open(tool) { app.dock.open_on(tool, crate::dock::Side::Left); }
    }

    let text = format!(
        "{:.0}° • v {:.2} • relief {:+.2} mm • wall {:.2} mm • {}{}",
        theta,
        v_mm,
        h,
        hit.radial_wall_mm,
        class,
        named.map(|(_, n)| format!(" • {n}")).unwrap_or_default()
    );
    app.set_status(text.clone());
    app.probe = Some((world, text));
}

/// A click on the Select tool: the picks under the pointer join the hover stack, the modifiers
/// decide what the choice does to the selection, and a plain click on the band keeps the readout
/// and the layer selection it always had.
fn select_click(app: &mut RingDesignerApp, camera: crate::camera::OrbitCamera, rect: egui::Rect, pos: egui::Pos2, mods: Mods) {
    let Some(scene) = app.pick_scene.clone() else {
        if !mods.shift && !mods.ctrl {
            probe_click(app, camera, rect, pos);
        }
        return;
    };
    let ray = |p: egui::Pos2| camera.ray(rect, p);
    let picks = app.picks_now(ringdesign_workbench::hover::pick_at(&scene, pos, &ray, APERTURE_PX, app.selection.filter));
    app.selection.hovered(picks);
    let (origin, direction) = ray(pos);
    let chart = app.build.as_ref().and_then(|b| ringdesign_core::interaction::picking::hit(&app.design, &app.lib, &b.mesh, origin, direction)).map(|h| (h.theta_deg, h.v_mm));
    app.selection.choose(mods, |w| chart.unwrap_or_else(|| (w[1].atan2(w[0]).to_degrees(), 0.0)));
    let plain = !mods.shift && !mods.ctrl;
    match app.selection.items.last() {
        None if plain => app.clear_selection(),
        Some(Sel::BandPoint { .. }) if plain => probe_click(app, camera, rect, pos),
        Some(sel) => {
            let what = ringdesign_workbench::viewport::selection::describe(sel, &app.design, app.build.as_deref());
            let n = app.selection.items.len();
            app.set_status(if n > 1 { format!("{what} · {n} selected") } else { what });
        }
        None => {}
    }
}

/// The right-click menu: what the selection can do, said by `context_items` and drawn with its icons.
fn show_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    let under = app.selection.under.clone();
    let switches = ringdesign_workbench::viewport::Switches { live_cuts: app.live_cuts, show_cutters: app.show_cutters };
    let items = ringdesign_workbench::viewport::context_items_in(&app.selection, under.as_ref(), &app.design, switches);
    if let Some(heading) = ringdesign_workbench::viewport::heading(&app.selection, under.as_ref(), &app.design) {
        ui.weak(heading);
    }
    let mut chosen: Option<MenuAction> = None;
    let mut i = 0;
    while i < items.len() {
        match items[i].submenu {
            Some(sub) => {
                let end = items[i..].iter().position(|x| x.submenu != Some(sub)).map_or(items.len(), |k| i + k);
                let group = &items[i..end];
                let icon = group.iter().find(|x| x.checked).map_or(group[0].icon, |x| x.icon);
                ui.menu_button((icon.image(ui, 18.), sub), |ui| {
                    ui.set_min_width(170.);
                    for item in group {
                        if menu_button(ui, item) {
                            chosen = Some(item.action.clone());
                            ui.close();
                        }
                    }
                });
                i = end;
            }
            None => {
                if menu_button(ui, &items[i]) {
                    chosen = Some(items[i].action.clone());
                    ui.close();
                }
                i += 1;
            }
        }
    }
    // The work planes' switch, when the build carries any.
    if app.build.as_ref().and_then(|b| b.parts.evaluated.as_ref()).is_some_and(|e| !e.planes.is_empty()) {
        let shown = !app.command.planes.hidden;
        let button = egui::Button::selectable(shown, (ringdesign_workbench::icons::Icon::Guides.image(ui, 18.), "Work planes"));
        if ui.add(button).on_hover_text("Draw the work planes over the ring, each named; right-click one to sketch on it or mirror across it").clicked() {
            app.command.planes.hidden = shown;
            ui.close();
        }
    }
    if let Some(action) = chosen {
        act(app, pane, action);
    }
}

fn menu_button(ui: &mut egui::Ui, item: &MenuItem) -> bool {
    let button = egui::Button::selectable(item.checked, (item.icon.image(ui, 18.), item.label.as_str()));
    ui.add_enabled(item.enabled, button).on_hover_text(item.hint).on_disabled_hover_text(item.hint).clicked()
}

/// Route a menu action: parts and edges go to the CAD pane, attachment and stage edit a plain
/// design in place, the rest is the view.
fn act(app: &mut RingDesignerApp, pane: usize, action: MenuAction) {
    use crate::panels::cad::{self, CadRequest};
    match action {
        MenuAction::AddPartHere { theta_deg, height_mm, label } => cad::add_starter(app, label, Some((theta_deg, height_mm))),
        MenuAction::EditFeature(id) => {
            app.selection.click(Some(Sel::Part(id)), Mods::default());
            cad::ask(app, CadRequest::Select { feature: id });
        }
        MenuAction::Attach(id, attach) => set_component(app, id, CadEdit::Attach { id, attach }),
        MenuAction::Stage(id, stage) => set_component(app, id, CadEdit::Stage { id, stage }),
        MenuAction::FilletEdge { feature, edge } => cad::add_modifier(app, "Fillet", feature, edge as usize),
        MenuAction::ChamferEdge { feature, edge } => cad::add_modifier(app, "Chamfer", feature, edge as usize),
        MenuAction::IsolateInCad(id) => cad::ask(app, CadRequest::Isolate { feature: id }),
        MenuAction::FitView => fit_view(app, pane),
        MenuAction::OpenCad => app.focus(crate::pane::PaneKind::Cad),
        MenuAction::ToggleWire => app.show_wireframe = !app.show_wireframe,
        MenuAction::ToggleGrid => app.show_grid = !app.show_grid,
        MenuAction::SketchOnFace { feature, face } => crate::sketch_mode::start_on_face(app, pane, feature, face),
        MenuAction::SketchOnPlane { theta_deg, across_mm } => crate::sketch_mode::start_on_plane(app, pane, theta_deg, across_mm),
        MenuAction::AddStone { theta_deg, height_mm, key } => crate::stone_tools::add_stone(app, theta_deg, height_mm, key),
        MenuAction::Setting { part, stone, key } => crate::stone_tools::setting(app, part, stone, key),
        MenuAction::PinHere { world } => crate::ring_snaps::pin_here(app, pane, world),
        MenuAction::ClearPins => crate::ring_snaps::clear_pins(app),
        MenuAction::Pattern { feature, key } => crate::patterns::start(app, pane, feature, key),
        MenuAction::PressPull { feature, face } => crate::patterns::press_pull(app, pane, feature, face),
        MenuAction::AddStoneOnFace { feature, face, key } => crate::stone_tools::add_stone_on_face(app, feature, face, key),
        MenuAction::CutHere { theta_deg, across_mm, key } => crate::cutter_tools::cut_here(app, theta_deg, across_mm, key),
        MenuAction::UnderStone { stone, key } => crate::cutter_tools::under_stone(app, stone, key),
        MenuAction::SeatLayer(path) => seat_layer(app, &path),
        MenuAction::ToggleLiveCuts => {
            app.live_cuts = !app.live_cuts;
            app.mark_dirty();
        }
        MenuAction::ToggleCutters => {
            app.show_cutters = !app.show_cutters;
            app.mark_dirty();
        }
        MenuAction::EditStamp(index) => app.stamp_inspector = Some(index),
        MenuAction::Stamp { index, edit } => stamp_edit(app, index, &edit),
    }
}

/// Eases the pane onto the chosen parts, seats, stamps and stones, else the whole ring, the pivot moved onto their middle.
pub(crate) fn fit_view(app: &mut RingDesignerApp, pane: usize) {
    let Some(build) = app.build.clone() else { return };
    let Some((bounds, framed)) = ringdesign_workbench::touch::view::framed(&build, &app.design, &app.selection.items, &[]) else { return };
    let Some(p) = app.panes.get_mut(pane) else { return };
    if let Some(ring) = build.mesh.bounds() {
        p.camera.refit(ring);
    }
    let to = p.camera.framing(bounds);
    p.turn = Some(ringdesign_workbench::focus::Turn::new(p.camera.pose(), to));
    app.set_status(framed.said());
}

/// The layer a seat's made solid stands on, chosen in the Layers tool, or its node on a driven design.
fn seat_layer(app: &mut RingDesignerApp, path: &[usize]) {
    let Some(&top) = path.first() else { return };
    if app.graph_driven() {
        app.edit_in_graph(top);
        return;
    }
    app.selected_layer = Some(top);
    if !app.dock.is_open(crate::dock::ToolKind::Layers) {
        app.dock.open_on(crate::dock::ToolKind::Layers, crate::dock::Side::Left);
    }
    let name = ringdesign_workbench::viewport::selection::entry_at(&app.design, path).map_or_else(|| format!("#{top}"), |e| e.name.clone());
    app.set_status(format!("Layer \"{name}\" chosen in the Layers tool"));
}

/// One change to stamp `index` of the design, one History entry, the selection and the inspector kept on the stamps that remain.
pub(crate) fn stamp_edit(app: &mut RingDesignerApp, index: usize, edit: &ringdesign_workbench::viewport::StampEdit) {
    app.history.commit(&app.design);
    let label = match ringdesign_workbench::viewport::made::edit(&mut app.design, index, edit) {
        Ok(label) => label,
        Err(why) => {
            app.set_status(why);
            return;
        }
    };
    if *edit == ringdesign_workbench::viewport::StampEdit::Delete {
        let shift = |k: usize| (k != index).then(|| if k > index { k - 1 } else { k });
        app.selection.items = std::mem::take(&mut app.selection.items).into_iter().filter_map(|s| match s {
            Sel::Stamp(k) => shift(k).map(Sel::Stamp),
            s => Some(s),
        }).collect();
        app.selection.hovered(Vec::new());
        app.stamp_inspector = app.stamp_inspector.and_then(shift);
    }
    app.mark_dirty();
    app.history.commit_as(&app.design, &label);
    app.set_status(label);
}

/// The inspector over the chosen stamp: its name and where it stands, each field one History entry once let go.
fn stamp_inspector(app: &mut RingDesignerApp, ui: &egui::Ui) {
    let Some(index) = app.stamp_inspector.filter(|i| *i < app.design.stamps.len()) else {
        app.stamp_inspector = None;
        return;
    };
    let driven = app.graph_driven();
    let mut stamp = app.design.stamps[index].clone();
    let mut open = true;
    let mut done = ringdesign_workbench::viewport::made::Inspected::default();
    egui::Window::new("Stamp")
        .id(egui::Id::new("stamp-inspector"))
        .frame(egui::Frame::window(ui.style()).fill(theme::FLOAT))
        .open(&mut open)
        .default_width(220.0)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .default_pos(ui.max_rect().right_bottom() + egui::vec2(-12.0, -30.0))
        .constrain_to(ui.ctx().content_rect())
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if driven {
                ui.weak(ringdesign_workbench::viewport::made::DRIVEN);
            }
            done = ui.add_enabled_ui(!driven, |ui| ringdesign_workbench::viewport::made::inspector(ui, &mut stamp)).inner;
        });
    if done.changed {
        app.design.stamps[index] = stamp;
        app.mark_dirty();
    }
    if done.settled {
        let name = app.design.stamps[index].name.clone();
        app.history.commit_as(&app.design, &format!("Edit stamp \"{name}\""));
    }
    if !open {
        app.stamp_inspector = None;
    }
}

/// Changes a part's attachment or stage through the edit funnel; a reference part is never metal.
fn set_component(app: &mut RingDesignerApp, id: u64, edit: CadEdit) {
    if app.design.cad.as_ref().and_then(|d| d.feature(id)).is_some_and(|f| f.component.reference) {
        app.set_status("A reference stone is never metal");
        return;
    }
    let _ = crate::cad_edit::apply(app, &[edit]);
}

/// The chosen and hovered vertices and band points, drawn over the ring; the edge pass lights the edges.
fn draw_selection(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector, rect: egui::Rect) {
    let Some(build) = app.build.as_deref() else { return };
    let evaluated = build.parts.evaluated.as_ref();
    let mark = |sel: &Sel, color: egui::Color32, width: f32| match sel {
        Sel::Vertex { feature, vertex } => {
            if let Some(v) = evaluated.and_then(|e| e.components.iter().find(|c| c.id == *feature)).and_then(|c| c.trace.vertices.get(*vertex as usize)) {
                let p = proj.at([v[0] as f32, v[1] as f32, v[2] as f32]);
                if rect.contains(p) {
                    painter.circle_stroke(p, 5.0, egui::Stroke::new(width, color));
                }
            }
        }
        Sel::BandPoint { world, .. } => {
            let p = proj.at([world[0] as f32, world[1] as f32, world[2] as f32]);
            if rect.contains(p) {
                painter.circle_stroke(p, 4.0, egui::Stroke::new(width, color));
            }
        }
        _ => {}
    };
    for item in &app.selection.items {
        mark(item, theme::SELECT, 2.0);
    }
    // A hovered band point already has the pointer's own cue.
    if let Some(h) = app.selection.hover.as_ref().filter(|h| h.entity != ringdesign_core::interaction::pick::Entity::Band) {
        mark(&Sel::of(h, |w| (w[1].atan2(w[0]).to_degrees(), 0.0)), ringdesign_workbench::hover::AQUA, 2.5);
    }
}

/// Where a cross-section view is cutting, drawn on the ring it cuts.
///
/// The slice is read from whichever section pane is on screen, so dragging
/// its angle moves the outline here in the same frame — a section is much
/// easier to read once you can see where on the ring it was taken.
fn draw_section_marker(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector) {
    let Some(section) = app.section_on_screen() else { return };
    let theta = (section.theta_deg as f32).to_radians();
    let (sin, cos) = theta.sin_cos();
    // A section point is (r, z) in its own plane; the plane stands at this
    // angle about the finger axis, which is where the ring is cut.
    let at = |r: f64, z: f64| proj.at([r as f32 * cos, r as f32 * sin, z as f32]);
    let ring: Vec<egui::Pos2> = section.points.iter().map(|p| at(p.r, p.z)).collect();
    if ring.len() < 3 {
        return;
    }
    // The cut face first, so the outline reads over it.
    painter.add(egui::Shape::convex_polygon(
        ring.clone(),
        theme::ACCENT.gamma_multiply(0.20),
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::closed_line(
        ring,
        egui::Stroke::new(1.8, theme::ACCENT),
    ));
    // A tick out past the metal says which way the slice faces.
    let out = section.max_r + (section.max_r - section.min_r).max(1.0) * 0.45;
    painter.line_segment(
        [at(section.max_r, section.parting_z_mm), at(out, section.parting_z_mm)],
        egui::Stroke::new(1.2, theme::ACCENT.gamma_multiply(0.7)),
    );
    painter.text(
        at(out, section.parting_z_mm),
        egui::Align2::LEFT_BOTTOM,
        format!("{:.0}°", section.theta_deg),
        egui::FontId::proportional(11.0),
        theme::ACCENT,
    );
}

fn draw_probe(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector, rect: egui::Rect) {
    if let Some((world, text)) = &app.probe {
        let p = proj.at(*world);
        if rect.contains(p) {
            painter.circle_stroke(p, 5.0, egui::Stroke::new(1.6, theme::ACCENT));
            painter.circle_filled(p, 1.6, theme::ACCENT);
            let galley =
                painter.layout_no_wrap(text.clone(), egui::FontId::proportional(11.0), theme::TEXT);
            let at = egui::pos2(
                (p.x + 10.0).min(rect.right() - galley.size().x - 6.0),
                (p.y - 18.0).max(rect.top() + 4.0),
            );
            let bg = egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(5.0, 3.0));
            painter.rect_filled(bg, 3.0, theme::PANEL.gamma_multiply(0.9));
            painter.galley(at, galley, theme::TEXT);
        }
    }
}

// --- Work planes --------------------------------------------------------------

/// How far past the ring a plane through it reaches, mm.
const PLANE_MARGIN_MM: f64 = 1.5;
/// Half the side of a plane laid square to the band or on a face, mm.
const PLANE_PATCH_MM: f64 = 3.0;
/// How near a plane's outline the pointer must come to take it, points.
const PLANE_REACH_PX: f32 = 6.0;
/// Thinner than this on screen a plane is seen edge on, and only its name takes the pointer, points.
const PLANE_EDGE_ON_PX: f32 = 18.0;

/// The Ring viewport's work planes: whether they are drawn, the one under the pointer, the one chosen, and the one a right-click opened on.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlaneView {
    pub hidden: bool,
    pub hot: Option<u64>,
    pub chosen: Option<u64>,
    pub menu: Option<u64>,
}

/// A work plane as the viewport draws it: its feature, its name and its rectangle's corners in the world.
#[derive(Clone, Debug)]
pub struct PlaneShape {
    pub id: u64,
    pub name: String,
    pub corners: [[f64; 3]; 4],
}

/// Every enabled work plane the last build carries: a plane through the ring spans it, one on the band or a face is a patch round its origin.
pub fn plane_shapes(app: &RingDesignerApp) -> Vec<PlaneShape> {
    use ringdesign_core::cad::{Operation, PlaneBase};
    let (Some(build), Some(doc)) = (app.build.as_deref(), app.design.cad.as_ref()) else { return Vec::new() };
    let Some(e) = build.parts.evaluated.as_ref() else { return Vec::new() };
    let (lo, hi) = build.mesh.bounds().unwrap_or_default();
    let reach = f64::from(lo.0.abs().max(hi.0.abs()).max(lo.1.abs()).max(hi.1.abs())) + PLANE_MARGIN_MM;
    e.planes
        .iter()
        .filter_map(|p| {
            let f = doc.feature(p.id).filter(|f| f.enabled)?;
            let Operation::Plane { base, .. } = &f.operation else { return None };
            let (x, y) = match base {
                PlaneBase::Section { .. } => ([-reach, reach], [f64::from(lo.2) - PLANE_MARGIN_MM - p.origin[2], f64::from(hi.2) + PLANE_MARGIN_MM - p.origin[2]]),
                PlaneBase::Parting => ([-reach, reach], [-reach, reach]),
                PlaneBase::Tangent { .. } | PlaneBase::Face { .. } => ([-PLANE_PATCH_MM, PLANE_PATCH_MM], [-PLANE_PATCH_MM, PLANE_PATCH_MM]),
            };
            let at = |u: f64, v: f64| std::array::from_fn(|k| p.origin[k] + p.x[k] * u + p.y[k] * v);
            Some(PlaneShape { id: p.id, name: f.name.clone(), corners: [at(x[0], y[0]), at(x[1], y[0]), at(x[1], y[1]), at(x[0], y[1])] })
        })
        .collect()
}

/// A plane's corners on screen.
fn plane_screen(shape: &PlaneShape, proj: &Projector) -> [egui::Pos2; 4] {
    shape.corners.map(|c| proj.at(c.map(|v| v as f32)))
}

/// Where a plane's name is written: over its top corner on screen, the left of two level ones.
fn plane_label(painter: &egui::Painter, shape: &PlaneShape, pts: &[egui::Pos2; 4], color: egui::Color32) -> (egui::Rect, Arc<egui::Galley>) {
    let top = pts.iter().copied().fold(pts[0], |a, b| if b.y < a.y - 0.5 || ((b.y - a.y).abs() <= 0.5 && b.x < a.x) { b } else { a });
    let galley = painter.layout_no_wrap(shape.name.clone(), egui::FontId::proportional(11.0), color);
    let at = top + egui::vec2(4.0, -4.0 - galley.size().y);
    (egui::Rect::from_min_size(at, galley.size()), galley)
}

/// Whether a plane's rectangle on screen is thinner than [`PLANE_EDGE_ON_PX`] across: its area over its longer side.
fn edge_on(pts: &[egui::Pos2; 4]) -> bool {
    let (u, v) = (pts[1] - pts[0], pts[3] - pts[0]);
    let longest = u.length().max(v.length());
    longest <= f32::EPSILON || (u.x * v.y - u.y * v.x).abs() / longest < PLANE_EDGE_ON_PX
}

/// Distance from `p` to the segment from `a` to `b`, points.
fn to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let (ab, ap) = (b - a, p - a);
    let t = if ab.length_sq() > 1e-9 { (ap.dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance(p)
}

/// The plane whose outline or name lies under `pos`, the nearest of those within reach; a plane seen edge on answers by its name alone.
fn plane_at(shapes: &[PlaneShape], proj: &Projector, painter: &egui::Painter, pos: egui::Pos2) -> Option<u64> {
    shapes
        .iter()
        .filter_map(|s| {
            let pts = plane_screen(s, proj);
            // Seen edge on the outline is a line across the ring, and leaves the pointer to the ring.
            let edge = if edge_on(&pts) { f32::INFINITY } else { (0..4).map(|i| to_segment(pos, pts[i], pts[(i + 1) % 4])).fold(f32::INFINITY, f32::min) };
            let (label, _) = plane_label(painter, s, &pts, theme::TEXT);
            let d = if label.expand(2.0).contains(pos) { 0.0 } else { edge };
            (d <= PLANE_REACH_PX).then_some((s.id, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Every plane as a translucent rectangle with its name, the one under the pointer and the chosen one lit, each named for a reader.
fn draw_planes(app: &RingDesignerApp, ui: &egui::Ui, pane: usize, shapes: &[PlaneShape], painter: &egui::Painter, proj: &Projector) {
    let view = app.command.planes;
    let reader = ui.ctx().accesskit_node_builder(ui.id(), |_| ()).is_some();
    for s in shapes {
        let pts = plane_screen(s, proj);
        let lit = view.hot == Some(s.id) || view.menu == Some(s.id);
        let chosen = view.chosen == Some(s.id);
        let color = if chosen { theme::SELECT } else { ringdesign_workbench::hover::AQUA };
        let (fill, width, line) = if lit || chosen { (0.14, 2.0, 1.0) } else { (0.06, 1.2, 0.6) };
        painter.add(egui::Shape::convex_polygon(pts.to_vec(), color.gamma_multiply(fill), egui::Stroke::new(width, color.gamma_multiply(line))));
        let (rect, galley) = plane_label(painter, s, &pts, color);
        painter.rect_filled(rect.expand2(egui::vec2(3.0, 1.0)), 3.0, egui::Color32::from_black_alpha(150));
        painter.galley(rect.min, galley, color);
        if reader {
            let r = ui.interact(rect, ui.id().with(("work-plane", pane, s.id)), egui::Sense::hover());
            let label = format!("Work plane: {}", s.name);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.as_str()));
        }
    }
}

/// Chooses work plane `id`, leaving the chosen parts as they are.
fn choose_plane(app: &mut RingDesignerApp, shapes: &[PlaneShape], id: u64) {
    app.command.planes.chosen = Some(id);
    let name = shapes.iter().find(|s| s.id == id).map_or_else(|| format!("#{id}"), |s| s.name.clone());
    app.set_status(format!("Work plane {name}: right-click it to sketch on it or mirror the chosen part across it"));
}

/// The right-click menu on a work plane: sketch on it, mirror the chosen part across it, or hide the planes.
fn plane_menu(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize, plane: u64) {
    use ringdesign_workbench::icons::Icon;
    let doc = app.design.cad.as_ref();
    let name = doc.and_then(|d| d.feature(plane)).map_or_else(|| format!("#{plane}"), |f| f.name.clone());
    ui.weak(format!("Work plane: {name}"));
    let part = crate::command::selected_part(app).filter(|id| *id != plane).and_then(|id| doc?.feature(id).cloned());
    let hint = match &part {
        None => "Choose a part on the ring first, then right-click the plane".to_string(),
        Some(f) if f.component.reference => "A reference stone is patterned with its setting; choose the setting".to_string(),
        Some(f) if !f.operation.has_body() => format!("{} has no body to mirror", f.name),
        Some(f) => format!("{} reflected across {name}, one new Mirror feature", f.name),
    };
    let mirrorable = part.as_ref().filter(|f| !f.component.reference && f.operation.has_body()).map(|f| f.id);
    let button = |ui: &mut egui::Ui, icon: Icon, label: &str| egui::Button::image_and_text(icon.image(ui, 18.), label.to_string());
    let sketch = button(ui, Icon::CadSketch, "Sketch on this plane");
    if ui.add(sketch).on_hover_text("A new sketch lying on this plane, drawn in the Ring viewport").clicked() {
        ui.close();
        sketch_on_work_plane(app, pane, plane);
    }
    let mirror = button(ui, Icon::Mirror, "Mirror the chosen part across it");
    if ui.add_enabled(mirrorable.is_some(), mirror).on_hover_text(&hint).on_disabled_hover_text(&hint).clicked() {
        ui.close();
        if let Some(id) = mirrorable {
            crate::patterns::mirror_across(app, id, plane);
        }
    }
    let hide = button(ui, Icon::Guides, "Hide work planes");
    if ui.add(hide).on_hover_text("Stop drawing the work planes; the right-click menu on the ring shows them again").clicked() {
        ui.close();
        app.command.planes.hidden = true;
        app.command.planes.menu = None;
    }
}

/// Adds a Sketch feature lying on work plane `plane` through the funnel and starts drawing it.
fn sketch_on_work_plane(app: &mut RingDesignerApp, pane: usize, plane: u64) {
    use ringdesign_core::cad::{Component, FaceRef, Feature, Operation};
    use ringdesign_core::sketch::{FaceAnchor, Sketch};
    if crate::sketch_mode::active(app) {
        app.set_status("Finish or leave the sketch being drawn first");
        return;
    }
    crate::command::cancel(app);
    let mut sketch = Sketch { name: "Sketch".into(), ..Sketch::default() };
    sketch.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
    let feature = Feature { id: 0, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch }, component: Component::default() };
    let Ok(applied) = crate::cad_edit::apply(app, &[CadEdit::Add { feature, after: None }]) else { return };
    if let Some(id) = applied.first().and_then(|a| a.id) {
        crate::sketch_mode::start_on_feature(app, pane, id);
    }
}
