//! The phone's handle on the renderer it shares with the desktop (`ringdesign_workbench::render`).
//!
//! The shaders are the desktop's own, compiled as GLSL ES 3.00 here; what the phone draws its own
//! way is `Look::PHONE`: the barycentric wireframe (GLES has no `glPolygonMode`, and Adreno and
//! Mali clamp `glLineWidth` to 1), rim-weighted cutters, and a two-sided ghost the wireframe
//! hides. A shader that will not build is said on the pane rather than killing the process.

use std::sync::{Arc, Mutex};

use egui_glow::glow;

use ringdesign_core::castability::CastReport;
use ringdesign_core::mesh::Mesh;
use ringdesign_workbench::render::{self, EdgeKey, Frame, Look, MeshRenderer, Shade};

pub use render::{HOVER_TINT, PREVIEW_TINT, SELECT_TINT, WALL_NEUTRAL, wall_color};

/// The shared renderer under the names the phone calls it by.
pub struct GpuMeshRenderer {
    inner: MeshRenderer,
    /// Paint times, gathered in a build made with `RD_RENDER_STATS` set.
    timings: Option<render::Timings>,
}

impl Default for GpuMeshRenderer {
    fn default() -> Self {
        let mut inner = MeshRenderer::new(Look::PHONE);
        let timing = render::Timing::from_env(option_env!("RD_RENDER_STATS"));
        inner.set_timing(timing);
        Self { inner, timings: (timing != render::Timing::Off).then(render::Timings::default) }
    }
}

impl GpuMeshRenderer {
    /// The focus channel for `mesh`, in the order [`stage`](Self::stage) emits vertices.
    pub fn stage_focus(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        render::stage_focus(mesh, weight)
    }

    /// The mesh as the renderer's buffer, draft colours and the wall heatmap of
    /// `wall = (inner_radius_mm, min_section_mm)` baked in; built on the worker, not the UI thread.
    pub fn stage(mesh: &Mesh, cast: Option<&CastReport>, wall: (f64, f64)) -> Vec<f32> {
        render::stage_mesh(mesh, cast, wall)
    }

    /// Hands the renderer a buffer built by [`stage`](Self::stage).
    pub fn set_pending(&mut self, verts: Vec<f32>) {
        self.inner.set_mesh(verts);
    }

    /// The cutters' ghost, in the stones' layout; empty clears it.
    pub fn set_pending_ghost(&mut self, verts: Vec<f32>) {
        self.inner.set_cutters(verts);
    }

    /// Stone-preview triangles in the same layout; empty clears them.
    pub fn set_pending_gems(&mut self, verts: Vec<f32>) {
        self.inner.set_gems(verts);
    }

    /// A focus channel built by [`stage_focus`](Self::stage_focus); empty clears the highlight.
    pub fn set_pending_focus(&mut self, weights: Vec<f32>) {
        self.inner.set_focus(weights);
    }

    pub fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }

    /// The selection channel in [`stage`](Self::stage)'s vertex order: `(chosen, under the finger)` from `viewport::tint`'s weights.
    pub fn stage_select(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        render::stage_select(mesh, weight)
    }

    /// A selection channel built by [`stage_select`](Self::stage_select); empty clears it.
    pub fn set_pending_select(&mut self, weights: Vec<f32>) {
        self.inner.set_select(weights);
    }

    /// Whether a selection tint is uploaded or on its way.
    pub fn has_select(&self) -> bool {
        self.inner.has_select()
    }

    /// A live command's ghost in its own coordinates, staged by [`stage_part`](Self::stage_part); empty clears it.
    pub fn set_pending_preview(&mut self, verts: Vec<f32>) {
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

    /// The ghost as it stands: its vertices awaiting upload, where it is drawn, and whether it shades by draft class.
    pub fn preview_state(&self) -> (Option<&[f32]>, Option<([f32; 16], [f32; 9])>, bool) {
        self.inner.preview_state()
    }

    /// A part's tessellation, a vertex normal kept only within 20° of its facet's.
    pub fn stage_part(mesh: &Mesh) -> Vec<f32> {
        render::stage_part(mesh)
    }

    /// [`stage_part`](Self::stage_part) with every face in its draft class's colour.
    pub fn stage_part_classes(mesh: &Mesh, classes: &[ringdesign_core::FaceClass]) -> Vec<f32> {
        render::stage_part_classes(mesh, classes)
    }

    pub fn has_mesh(&self) -> bool {
        self.inner.has_mesh()
    }

    /// Why nothing draws, if the shaders would not build.
    pub fn failed(&self) -> Option<&str> {
        self.inner.failed()
    }

    /// Whether paints are being timed, which keeps the pane repainting.
    pub fn timed(&self) -> bool {
        self.timings.is_some()
    }

    /// Keeps the edge pass on the build on screen and the chosen and hovered edges.
    pub fn sync_edges(&mut self, build: &Arc<ringdesign_core::BuildResult>, chosen: &[EdgeKey], hovered: Option<EdgeKey>) -> bool {
        let key = Arc::as_ptr(build) as usize;
        let fresh = self.inner.edges_key() != Some(key);
        let start = (fresh && self.timings.is_some()).then(std::time::Instant::now);
        let staged = self.inner.sync_edges(key, build.parts.evaluated.as_ref(), chosen, hovered);
        if let Some(start) = start {
            log::info!("render-stats stage-edges runs={} us={:.0}", self.inner.edge_runs().len(), start.elapsed().as_secs_f32() * 1e6);
        }
        staged
    }

    /// Draws the frame, then says once what the context turned out to be.
    fn paint(&mut self, gl: &glow::Context, info: &egui::PaintCallbackInfo, frame: &Frame) {
        let before = self.inner.stats().frames;
        self.inner.paint(gl, info, frame);
        for note in self.inner.take_notes() {
            match note {
                render::GlNote::Ready { version, profile } => log::info!("GL_VERSION: {version} ({profile:?})"),
                render::GlNote::Depth(bits) if bits <= 0 => log::warn!(
                    "no depth buffer on the default framebuffer ({bits} bits): the ring will draw \
                     see-through. app! needs its third argument, e.g. app!(App::new, Backend::Glow, 24)."
                ),
                render::GlNote::Depth(bits) => log::info!("depth buffer: {bits} bits"),
                render::GlNote::Failed(e) => log::error!("ring shader: {e}"),
            }
        }
        let stats = self.inner.stats();
        if let Some(t) = self.timings.as_mut().filter(|_| stats.frames > before) {
            if let Some(line) = t.record(&stats) {
                log::info!("{line}");
            }
        }
    }
}

/// How the solid pane shades the mesh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShadeMode {
    #[default]
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
            ShadeMode::Metal => "Metal",
            ShadeMode::Draft => "Draft",
            ShadeMode::Wall => "Wall",
            ShadeMode::Halves => "Halves",
            ShadeMode::Normals => "Normals",
        }
    }

    fn code(self) -> Shade {
        match self {
            ShadeMode::Metal => Shade::Metal,
            ShadeMode::Draft => Shade::Draft,
            ShadeMode::Normals => Shade::Normals,
            ShadeMode::Wall => Shade::Wall,
            ShadeMode::Halves => Shade::Halves,
        }
    }
}

/// How the pane draws this frame, beyond the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneLook {
    pub shade: ShadeMode,
    pub base_color: [f32; 3],
    pub roughness: f32,
    /// The wireframe's colour; `None` draws none.
    pub wire: Option<[f32; 3]>,
    pub clip_plane: [f32; 4],
    pub focus: [f32; 4],
    /// Every part's edges; a chosen one draws either way.
    pub edges: bool,
}

/// Queues the mesh draw as an egui paint callback covering `rect`.
///
/// Multiple callbacks coexist safely: `egui_glow::Painter` dispatches per primitive and calls
/// `prepare_painting` to restore its own state after each one.
pub fn paint_callback(ui: &egui::Ui, rect: egui::Rect, renderer: Arc<Mutex<GpuMeshRenderer>>, mvp: [f32; 16], normal_matrix: [f32; 9], look: PaneLook) {
    let frame = Frame {
        shade: look.shade.code(),
        base_color: look.base_color,
        roughness: look.roughness,
        wire: look.wire,
        clip_plane: look.clip_plane,
        focus: look.focus,
        select: SELECT_TINT,
        hover: HOVER_TINT,
        edges: look.edges,
        ..Frame::new(mvp, normal_matrix)
    };
    let cb = egui_glow::CallbackFn::new(move |info, painter| {
        if let Ok(mut r) = renderer.lock() {
            r.paint(painter.gl(), &info, &frame);
        }
    });
    ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::mesh::Vec3;

    #[test]
    fn shade_modes_have_distinct_codes() {
        let codes: Vec<i32> = ShadeMode::ALL.iter().map(|m| m.code() as i32).collect();
        assert_eq!(codes, vec![0, 1, 3, 4, 2]);
    }

    #[test]
    fn staging_emits_three_vertices_per_face() {
        let mesh = Mesh {
            vertices: vec![Vec3(0.0, 0.0, 0.0), Vec3(1.0, 0.0, 0.0), Vec3(0.0, 1.0, 0.0)],
            normals: vec![Vec3(0.0, 0.0, 1.0); 3],
            faces: vec![[0, 1, 2]],
            ..Default::default()
        };
        let data = GpuMeshRenderer::stage(&mesh, None, (8.5, 0.8));
        assert_eq!(data.len(), 3 * render::MESH_FLOATS);
        // Position of the second vertex.
        assert_eq!(data[render::MESH_FLOATS], 1.0);
        // No cast report means white.
        assert_eq!(data[6], 1.0);
    }

    #[test]
    fn the_focus_channel_rides_vertex_for_vertex_with_the_staged_mesh() {
        let mesh = Mesh {
            vertices: vec![Vec3(0.0, 0.0, 0.0), Vec3(1.0, 0.0, 0.0), Vec3(0.0, 1.0, 0.0), Vec3(f32::NAN, 0.0, 0.0)],
            normals: vec![Vec3(0.0, 0.0, 1.0); 4],
            faces: vec![[0, 1, 2], [0, 1, 3], [2, 1, 0]],
            ..Default::default()
        };
        let staged = GpuMeshRenderer::stage(&mesh, None, (8.5, 0.8));
        let focus = GpuMeshRenderer::stage_focus(&mesh, &[0.0, 0.5, 1.0, 9.0]);
        assert_eq!(focus.len() * render::MESH_FLOATS, staged.len(), "the face with a bad vertex is skipped in both");
        assert_eq!(focus, vec![0.0, 0.5, 1.0, 1.0, 0.5, 0.0]);
        let mut r = GpuMeshRenderer::default();
        assert!(!r.has_focus());
        r.set_pending_focus(focus);
        assert!(r.has_focus());
        r.set_pending_focus(Vec::new());
        assert!(!r.has_focus(), "an empty channel clears the highlight");
    }

    #[test]
    fn a_face_referencing_a_missing_vertex_is_dropped_not_panicked() {
        let mesh = Mesh {
            vertices: vec![Vec3(0.0, 0.0, 0.0)],
            normals: vec![Vec3(0.0, 0.0, 1.0)],
            faces: vec![[0, 9, 9]],
            ..Default::default()
        };
        assert!(GpuMeshRenderer::stage(&mesh, None, (8.5, 0.8)).is_empty());
    }

    #[test]
    fn the_phone_draws_its_own_look_through_the_shared_renderer() {
        let r = GpuMeshRenderer::default();
        assert_eq!(*r.inner.look(), Look::PHONE);
        assert!(r.failed().is_none() && !r.has_mesh());
    }
}
