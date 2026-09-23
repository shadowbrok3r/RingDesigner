//! The GL mesh renderer the desktop and the phone share.
//!
//! One set of shader sources ([`shaders`]) compiled under the header and precision the context
//! asks for, and every pass either app draws: the metal in each shade mode with the wall
//! heatmap's second colour, the focus channel on attribute 4 and the select channel on
//! attribute 5, the stones, the wireframe, the crisp edge pass ([`edges`]), the pinned
//! comparison design, a live command's ghost under a model matrix, and the cutters drawn
//! through the metal. Each app keeps its camera and its UI and hands the renderer its buffers,
//! staged anywhere ([`stage`]), and a [`Frame`] per paint. What an app draws its own way is its
//! [`Look`].
//!
//! Nothing here uses a GL entry point GLES lacks except `glPolygonMode`, which only the line
//! wireframe calls and only on a desktop context, and no line is wider than one pixel: the edges
//! are quads widened in the vertex stage, since Adreno and Mali clamp `glLineWidth` to 1.
pub mod edges;
pub mod shaders;
pub mod stage;

pub use edges::{EdgeKey, EdgeRun, EdgeStyle, StagedEdges, bias_mm, stage_edges, stage_lit, stage_polylines, view_scale};
pub use shaders::Profile;
pub use stage::{MESH_FLOATS, WALL_NEUTRAL, stage_cad, stage_focus, stage_mesh, stage_part, stage_part_classes, stage_plain, stage_select, wall_color};

use egui_glow::glow::{self, HasContext};
use ringdesign_core::cad::Evaluated;

/// The chosen entities' tint and strength: the selection violet.
pub const SELECT_TINT: [f32; 4] = [0.80, 0.57, 0.85, 0.55];
/// The hovered entity's tint and strength: the hover cue's aqua.
pub const HOVER_TINT: [f32; 4] = [0.40, 0.85, 0.83, 0.55];
/// A live command's ghost: the hover aqua, and its opacity.
pub const PREVIEW_TINT: [f32; 4] = [0.40, 0.85, 0.83, 0.45];
/// A ghost painted by its draft classes, opacity.
pub const DRAFT_GHOST_ALPHA: f32 = 0.6;
/// The cutters' tint.
pub const CUTTER_TINT: [f32; 3] = [1.0, 0.34, 0.62];
/// The pinned comparison design's tint and opacity.
pub const COMPARISON_TINT: [f32; 4] = [0.62, 0.72, 0.84, 0.28];
/// The barycentric wireframe's half-width, in fragments.
pub const WIRE_PX: f32 = 0.6;
/// A chosen or hovered edge's opacity where metal hides it.
pub const LIT_HIDDEN_OPACITY: f32 = 0.35;

/// The fragment stage's stone mode.
const MODE_GEM: i32 = 5;
/// The fragment stage's rim-weighted ghost mode.
const MODE_RIM: i32 = 6;

/// How the metal is shaded; the value is the fragment stage's `u_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shade {
    #[default]
    Metal = 0,
    Draft = 1,
    Normals = 2,
    Wall = 3,
    Halves = 4,
}

/// How the wireframe is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    /// `glPolygonMode` lines over the metal in a second, blended pass: desktop GL only, and
    /// barycentric on a context without it.
    Lines,
    /// Each triangle's own barycentric edge, mixed in by the metal pass itself.
    Barycentric,
}

/// How the cutters' ghost is drawn: over everything, blended, the depth test off, since a tool
/// sits inside the metal it removes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cutters {
    /// Shaded as metal in the cutter tint, at this opacity.
    Tinted(f32),
    /// Faint face on and bright where the surface turns away, at this opacity, both sides drawn.
    Rim(f32),
}

/// What an app draws its own way, fixed for its life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub wire: Wire,
    pub cutters: Cutters,
    /// A live command's ghost draws its back faces too.
    pub preview_both_sides: bool,
    /// A live command's ghost hides while the wireframe shows.
    pub preview_hidden_by_wire: bool,
    pub edges: EdgeStyle,
}

impl Look {
    /// The desktop: line wireframe, tinted cutters, a one-sided ghost always shown.
    pub const DESKTOP: Look = Look { wire: Wire::Lines, cutters: Cutters::Tinted(0.34), preview_both_sides: false, preview_hidden_by_wire: false, edges: EdgeStyle::DESKTOP };
    /// The phone: barycentric wireframe, rim-weighted cutters, a two-sided ghost the wireframe hides.
    pub const PHONE: Look = Look { wire: Wire::Barycentric, cutters: Cutters::Rim(0.85), preview_both_sides: true, preview_hidden_by_wire: true, edges: EdgeStyle::PHONE };
}

/// One paint's view and shading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Column-major projection · view.
    pub mvp: [f32; 16],
    /// Column-major 3x3 for normals.
    pub normal_matrix: [f32; 9],
    pub shade: Shade,
    pub base_color: [f32; 3],
    pub roughness: f32,
    pub light_dir: [f32; 3],
    pub ambient: f32,
    /// The wireframe's colour; `None` draws none.
    pub wire: Option<[f32; 3]>,
    /// Whether the stones draw.
    pub gems: bool,
    /// Fragments with `dot((p, 1), plane) > 0` are cut away; zero keeps everything.
    pub clip_plane: [f32; 4],
    /// The focus channel's tint and strength; zero strength is off.
    pub focus: [f32; 4],
    pub select: [f32; 4],
    pub hover: [f32; 4],
    /// Every drawn part's edges; the chosen and hovered edges draw either way.
    pub edges: bool,
}

impl Frame {
    /// Polished metal under the studio key, nothing lit, stones and edges shown.
    pub fn new(mvp: [f32; 16], normal_matrix: [f32; 9]) -> Self {
        Self {
            mvp,
            normal_matrix,
            shade: Shade::Metal,
            base_color: [0.79, 0.80, 0.81],
            roughness: 0.0,
            light_dir: [-0.38, 0.46, 0.80],
            ambient: 0.20,
            wire: None,
            gems: true,
            clip_plane: [0.0; 4],
            focus: [0.0; 4],
            select: [0.0; 4],
            hover: [0.0; 4],
            edges: true,
        }
    }
}

/// What the context turned out to be, said once so an app can log it.
#[derive(Clone, Debug, PartialEq)]
pub enum GlNote {
    /// The programs built: the driver's version string and the dialect it took.
    Ready { version: String, profile: Profile },
    /// Bits of depth on the framebuffer painted into; zero draws the ring see-through.
    Depth(i32),
    /// A program would not build; nothing draws.
    Failed(String),
}

/// What the last paints cost. Times need [`MeshRenderer::set_timing`]: each timed span finishes
/// the GL queue before and after, so a time includes the GPU's work.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub frames: u64,
    /// The last paint, microseconds.
    pub paint_us: f32,
    /// The last measured edge pass, microseconds: GPU time where the context has timer queries.
    pub edges_us: f32,
    /// Whether [`edges_us`](Self::edges_us) is a GPU timer query's rather than a finished queue's.
    pub edges_gpu: bool,
    /// Whether the last paint uploaded a buffer.
    pub uploaded: bool,
    /// The last edge upload's bytes and microseconds.
    pub edge_upload: (usize, f32),
    pub mesh_vertices: i32,
    pub edge_vertices: i32,
    pub lit_vertices: i32,
}

/// A buffer's pending contents and the vertices uploaded.
#[derive(Debug, Default)]
struct Slot {
    pending: Option<Vec<f32>>,
    count: i32,
}

/// Which spans of a paint are timed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Timing {
    #[default]
    Off,
    /// The whole paint, finished either side.
    Paint,
    /// The whole paint and the edge pass on its own, each finished either side.
    Passes,
}

impl Timing {
    /// Reads `RD_RENDER_STATS`: `passes` times the edge pass too, any other value the paint alone.
    pub fn from_env(value: Option<&str>) -> Self {
        match value {
            None => Self::Off,
            Some("passes") => Self::Passes,
            Some(_) => Self::Paint,
        }
    }
}

/// A per-vertex channel of the metal: pending contents, and whether the upload covers the mesh.
#[derive(Debug, Default)]
struct Channel {
    pending: Option<Vec<f32>>,
    live: bool,
}

/// The build and the choice the lit edges were staged for.
#[derive(Clone, Debug, PartialEq)]
struct LitFor {
    build: usize,
    chosen: Vec<EdgeKey>,
    hovered: Option<EdgeKey>,
}

/// The one mesh renderer: buffers awaiting upload, the GL objects once a context has been seen, and the look.
pub struct MeshRenderer {
    look: Look,
    res: Option<Resources>,
    failed: Option<String>,
    notes: Vec<GlNote>,
    depth_checked: bool,
    mesh: Slot,
    gems: Slot,
    cutters: Slot,
    comparison: Slot,
    preview: Slot,
    edges: Slot,
    lit: Slot,
    focus: Channel,
    select: Channel,
    preview_model: Option<([f32; 16], [f32; 9])>,
    preview_draft: bool,
    edges_for: Option<usize>,
    lit_for: Option<LitFor>,
    runs: Vec<EdgeRun>,
    timing: Timing,
    timer: Option<GpuTimer>,
    timer_tried: bool,
    stats: Stats,
}

impl MeshRenderer {
    pub fn new(look: Look) -> Self {
        Self {
            look,
            res: None,
            failed: None,
            notes: Vec::new(),
            depth_checked: false,
            mesh: Slot::default(),
            gems: Slot::default(),
            cutters: Slot::default(),
            comparison: Slot::default(),
            preview: Slot::default(),
            edges: Slot::default(),
            lit: Slot::default(),
            focus: Channel::default(),
            select: Channel::default(),
            preview_model: None,
            preview_draft: false,
            edges_for: None,
            lit_for: None,
            runs: Vec::new(),
            timing: Timing::Off,
            timer: None,
            timer_tried: false,
            stats: Stats::default(),
        }
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    /// The metal, staged by [`stage_mesh`] or [`stage_cad`]; the channels staged for the last mesh are dropped with it.
    pub fn set_mesh(&mut self, verts: Vec<f32>) {
        self.mesh.pending = Some(verts);
    }

    /// Whether a mesh is uploaded or on its way.
    pub fn has_mesh(&self) -> bool {
        self.mesh.count > 0 || self.mesh.pending.is_some()
    }

    /// The stones, in the metal's layout; empty clears them.
    pub fn set_gems(&mut self, verts: Vec<f32>) {
        self.gems.pending = Some(verts);
    }

    /// The seats' cutters, in the metal's layout; empty clears them.
    pub fn set_cutters(&mut self, verts: Vec<f32>) {
        self.cutters.pending = Some(verts);
    }

    /// The pinned comparison design, in the metal's layout; empty clears it.
    pub fn set_comparison(&mut self, verts: Vec<f32>) {
        self.comparison.pending = Some(verts);
    }

    /// A live command's ghost in its own coordinates, staged by [`stage_part`]; empty clears it.
    pub fn set_preview(&mut self, verts: Vec<f32>) {
        self.preview.pending = Some(verts);
    }

    /// Where the ghost stands: its model matrix and the normals' 3x3, column-major; `None` hides it.
    pub fn set_preview_model(&mut self, model: Option<([f32; 16], [f32; 9])>) {
        self.preview_model = model;
    }

    /// Whether the ghost shades in its staged draft colours rather than the aqua.
    pub fn set_preview_draft(&mut self, draft: bool) {
        self.preview_draft = draft;
    }

    /// The ghost as it stands: its vertices awaiting upload, where it is drawn, and whether it shades by draft class.
    pub fn preview_state(&self) -> (Option<&[f32]>, Option<([f32; 16], [f32; 9])>, bool) {
        (self.preview.pending.as_deref(), self.preview_model, self.preview_draft)
    }

    /// A focus channel staged by [`stage_focus`] for the mesh on screen; empty clears it.
    pub fn set_focus(&mut self, weights: Vec<f32>) {
        self.focus.pending = Some(weights);
    }

    /// Whether a focus tint is uploaded or on its way.
    pub fn has_focus(&self) -> bool {
        self.focus.live || self.focus.pending.as_ref().is_some_and(|w| !w.is_empty())
    }

    /// A select channel staged by [`stage_select`]; empty clears it.
    pub fn set_select(&mut self, weights: Vec<f32>) {
        self.select.pending = Some(weights);
    }

    /// Whether a selection tint is uploaded or on its way.
    pub fn has_select(&self) -> bool {
        self.select.live || self.select.pending.as_ref().is_some_and(|w| !w.is_empty())
    }

    /// Every part's edges for the build `key` names, staged by [`stage_edges`].
    pub fn set_edges(&mut self, staged: StagedEdges, key: usize) {
        self.runs = staged.runs;
        self.edges.pending = Some(staged.verts);
        self.edges_for = Some(key);
        self.lit_for = None;
    }

    /// The build the edges were staged for.
    pub fn edges_key(&self) -> Option<usize> {
        self.edges_for
    }

    /// Each edge's run in the staged buffer.
    pub fn edge_runs(&self) -> &[EdgeRun] {
        &self.runs
    }

    /// Keeps the edge buffers in step with the build on screen and what is chosen: a new build
    /// restages every edge, a new choice or hover restages the lit ones; the same frame again
    /// stages nothing. Returns whether anything was staged.
    pub fn sync_edges(&mut self, key: usize, evaluated: Option<&Evaluated>, chosen: &[EdgeKey], hovered: Option<EdgeKey>) -> bool {
        let mut staged = false;
        if self.edges_for != Some(key) {
            self.set_edges(evaluated.map(stage_edges).unwrap_or_default(), key);
            staged = true;
        }
        let want = LitFor { build: key, chosen: chosen.to_vec(), hovered };
        if self.lit_for.as_ref() != Some(&want) {
            let verts = match evaluated {
                Some(e) if !chosen.is_empty() || hovered.is_some() => stage_lit(e, chosen, hovered, &self.look.edges),
                _ => Vec::new(),
            };
            self.lit.pending = Some(verts);
            self.lit_for = Some(want);
            staged = true;
        }
        staged
    }

    /// The lit edges awaiting upload, if a choice changed since the last paint.
    pub fn pending_lit(&self) -> Option<&[f32]> {
        self.lit.pending.as_deref()
    }

    /// Why nothing draws, if the programs would not build.
    pub fn failed(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// What the context said since the last call.
    pub fn take_notes(&mut self) -> Vec<GlNote> {
        std::mem::take(&mut self.notes)
    }

    /// Times each paint's spans, finishing the GL queue either side of each.
    pub fn set_timing(&mut self, timing: Timing) {
        self.timing = timing;
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Deletes the GL objects and forgets every buffer; the next paint builds afresh.
    pub fn destroy(&mut self, gl: &glow::Context) {
        if let Some(res) = self.res.take() {
            unsafe { res.delete(gl) };
        }
        if let Some(timer) = self.timer.take() {
            unsafe { timer.delete(gl) };
        }
        let look = self.look;
        let timing = self.timing;
        *self = Self::new(look);
        self.timing = timing;
    }

    /// Draws the frame into the callback's viewport: uploads what is pending, then every pass.
    pub fn paint(&mut self, gl: &glow::Context, info: &egui::PaintCallbackInfo, f: &Frame) {
        self.ensure(gl);
        self.check_depth(gl);
        let Some(res) = self.res.as_ref() else { return };
        let timing = self.timing != Timing::Off;
        let passes = self.timing == Timing::Passes;
        if passes && !self.timer_tried {
            self.timer_tried = true;
            let queries = res.profile == Profile::Core || gl.supported_extensions().contains("GL_EXT_disjoint_timer_query");
            self.timer = if queries { unsafe { GpuTimer::new(gl) } } else { None };
        }
        if let Some(us) = self.timer.as_mut().and_then(|t| unsafe { t.poll(gl) }) {
            self.stats.edges_us = us;
            self.stats.edges_gpu = true;
        }
        if timing {
            unsafe { gl.finish() };
        }
        let start = timing.then(std::time::Instant::now);

        let mut uploaded = false;
        unsafe {
            if let Some(verts) = self.mesh.pending.take() {
                // A channel staged for the last mesh says nothing about this one.
                self.focus.live = false;
                self.select.live = false;
                self.mesh.count = upload(gl, res.ring.vbo, &verts, MESH_FLOATS, glow::STATIC_DRAW);
                uploaded = true;
            }
            if let Some(weights) = self.focus.pending.take() {
                self.focus.live = !weights.is_empty() && weights.len() as i32 == self.mesh.count;
                if self.focus.live {
                    upload(gl, res.focus_vbo, &weights, 1, glow::STATIC_DRAW);
                }
                uploaded = true;
            }
            if let Some(weights) = self.select.pending.take() {
                self.select.live = !weights.is_empty() && weights.len() as i32 == self.mesh.count * 2;
                if self.select.live {
                    upload(gl, res.select_vbo, &weights, 2, glow::STATIC_DRAW);
                }
                uploaded = true;
            }
            for (slot, pair, usage) in [
                (&mut self.gems, res.gems, glow::STATIC_DRAW),
                (&mut self.cutters, res.cutters, glow::STATIC_DRAW),
                (&mut self.comparison, res.comparison, glow::STATIC_DRAW),
                (&mut self.preview, res.preview, glow::DYNAMIC_DRAW),
            ] {
                if let Some(verts) = slot.pending.take() {
                    slot.count = upload(gl, pair.vbo, &verts, MESH_FLOATS, usage);
                    uploaded = true;
                }
            }
            if let Some(verts) = self.edges.pending.take() {
                let t = timing.then(std::time::Instant::now);
                self.edges.count = upload(gl, res.edges.vbo, &verts, edges::EDGE_FLOATS, glow::STATIC_DRAW);
                if let Some(t) = t {
                    gl.finish();
                    self.stats.edge_upload = (std::mem::size_of_val(verts.as_slice()), t.elapsed().as_secs_f32() * 1e6);
                }
                uploaded = true;
            }
            if let Some(verts) = self.lit.pending.take() {
                self.lit.count = upload(gl, res.lit.vbo, &verts, edges::LIT_FLOATS, glow::DYNAMIC_DRAW);
                uploaded = true;
            }
        }
        self.stats.uploaded = uploaded;
        self.stats.mesh_vertices = self.mesh.count;
        self.stats.edge_vertices = self.edges.count;
        self.stats.lit_vertices = self.lit.count;
        if self.mesh.count == 0 {
            return;
        }

        let look = self.look;
        let m = &res.mesh;
        unsafe {
            let vp = info.viewport_in_pixels();
            gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
            let clip = info.clip_rect_in_pixels();
            gl.scissor(clip.left_px, clip.from_bottom_px, clip.width_px, clip.height_px);

            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.depth_func(glow::LESS);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.enable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            // The metal.
            gl.use_program(Some(m.program));
            gl.bind_vertex_array(Some(res.ring.vao));
            gl.uniform_matrix_4_f32_slice(m.mvp.as_ref(), false, &f.mvp);
            gl.uniform_matrix_3_f32_slice(m.normal_matrix.as_ref(), false, &f.normal_matrix);
            gl.uniform_1_i32(m.mode.as_ref(), f.shade as i32);
            gl.uniform_3_f32_slice(m.light_dir.as_ref(), &f.light_dir);
            gl.uniform_3_f32_slice(m.base_color.as_ref(), &f.base_color);
            gl.uniform_1_f32(m.ambient.as_ref(), f.ambient);
            gl.uniform_1_f32(m.roughness.as_ref(), f.roughness);
            gl.uniform_3_f32_slice(m.wire_color.as_ref(), &f.wire.unwrap_or([0.0; 3]));
            let barycentric = f.wire.is_some() && (look.wire == Wire::Barycentric || res.wire.is_none());
            gl.uniform_1_f32(m.wire_px.as_ref(), if barycentric { WIRE_PX } else { 0.0 });
            gl.uniform_4_f32_slice(m.clip_plane.as_ref(), &f.clip_plane);
            gl.uniform_1_f32(m.alpha.as_ref(), 1.0);
            // Without a channel the attribute is a constant zero, so a stale buffer is never read past its end.
            let lit = self.focus.live && f.focus[3] > 0.0;
            gl.uniform_4_f32_slice(m.focus.as_ref(), &if lit { f.focus } else { [0.0; 4] });
            if lit {
                gl.enable_vertex_attrib_array(4);
            } else {
                gl.disable_vertex_attrib_array(4);
                gl.vertex_attrib_1_f32(4, 0.0);
            }
            let chosen = self.select.live && (f.select[3] > 0.0 || f.hover[3] > 0.0);
            gl.uniform_4_f32_slice(m.select.as_ref(), &if chosen { f.select } else { [0.0; 4] });
            gl.uniform_4_f32_slice(m.hover.as_ref(), &if chosen { f.hover } else { [0.0; 4] });
            if chosen {
                gl.enable_vertex_attrib_array(5);
            } else {
                gl.disable_vertex_attrib_array(5);
                gl.vertex_attrib_2_f32(5, 0.0, 0.0);
            }
            gl.draw_arrays(glow::TRIANGLES, 0, self.mesh.count);
            gl.uniform_4_f32_slice(m.focus.as_ref(), &[0.0; 4]);
            gl.uniform_4_f32_slice(m.select.as_ref(), &[0.0; 4]);
            gl.uniform_4_f32_slice(m.hover.as_ref(), &[0.0; 4]);

            // The line wireframe, a second pass over the same triangles.
            if let (Some(color), Some(w), false) = (f.wire, res.wire.as_ref(), barycentric) {
                gl.use_program(Some(w.program));
                gl.uniform_matrix_4_f32_slice(w.mvp.as_ref(), false, &f.mvp);
                gl.uniform_3_f32_slice(w.wire_color.as_ref(), &color);
                gl.uniform_4_f32_slice(w.clip_plane.as_ref(), &f.clip_plane);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.enable(glow::POLYGON_OFFSET_LINE);
                gl.polygon_offset(-1.0, -1.0);
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
                gl.draw_arrays(glow::TRIANGLES, 0, self.mesh.count);
                gl.disable(glow::POLYGON_OFFSET_LINE);
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
                gl.disable(glow::BLEND);
                gl.use_program(Some(m.program));
            }

            // The stones: the dielectric mode in their own tint, flat facets doing the sparkle.
            if f.gems && self.gems.count > 0 {
                gl.uniform_1_i32(m.mode.as_ref(), MODE_GEM);
                gl.uniform_3_f32_slice(m.base_color.as_ref(), &ringdesign_core::gems::GEM_TINT);
                gl.bind_vertex_array(Some(res.gems.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.gems.count);
            }
            gl.uniform_1_f32(m.wire_px.as_ref(), 0.0);

            // The edges, over the opaque metal and stones, under the translucent ghosts.
            let shown = !(look.preview_hidden_by_wire && f.wire.is_some());
            let ghost = self.preview.count > 0 && shown && self.preview_model.is_some();
            let later = self.comparison.count > 0 || ghost || self.cutters.count > 0;
            let every = f.edges && self.edges.count > 0;
            if every || self.lit.count > 0 {
                let queried = passes && self.timer.as_mut().is_some_and(|t| t.begin(gl));
                if passes && !queried {
                    gl.finish();
                }
                let t = (passes && !queried).then(std::time::Instant::now);
                let e = &res.edge;
                gl.use_program(Some(e.program));
                gl.uniform_matrix_4_f32_slice(e.mvp.as_ref(), false, &f.mvp);
                gl.uniform_2_f32(e.viewport.as_ref(), vp.width_px as f32, vp.height_px as f32);
                gl.uniform_1_f32(e.px_per_pt.as_ref(), info.pixels_per_point);
                let (toward, px_mm) = view_scale(&f.mvp, [vp.width_px as f32, vp.height_px as f32]).unwrap_or(([0.0; 3], 0.0));
                gl.uniform_3_f32_slice(e.toward_eye.as_ref(), &toward);
                gl.uniform_1_f32(e.bias_mm.as_ref(), bias_mm(px_mm));
                gl.uniform_4_f32_slice(e.clip_plane.as_ref(), &f.clip_plane);
                gl.depth_func(glow::LEQUAL);
                gl.depth_mask(false);
                gl.disable(glow::CULL_FACE);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.uniform_1_f32(e.opacity.as_ref(), 1.0);
                if every {
                    gl.bind_vertex_array(Some(res.edges.vao));
                    gl.vertex_attrib_1_f32(edges::ATTR_WIDTH, look.edges.width_pt);
                    let [r, g, b, a] = look.edges.color;
                    gl.vertex_attrib_4_f32(edges::ATTR_COLOR, r, g, b, a);
                    gl.draw_arrays(glow::TRIANGLES, 0, self.edges.count);
                }
                // A lit edge shows whole: faint where metal hides it, full where it does not.
                if self.lit.count > 0 {
                    gl.bind_vertex_array(Some(res.lit.vao));
                    gl.disable(glow::DEPTH_TEST);
                    gl.uniform_1_f32(e.opacity.as_ref(), LIT_HIDDEN_OPACITY);
                    gl.draw_arrays(glow::TRIANGLES, 0, self.lit.count);
                    gl.enable(glow::DEPTH_TEST);
                    gl.uniform_1_f32(e.opacity.as_ref(), 1.0);
                    gl.draw_arrays(glow::TRIANGLES, 0, self.lit.count);
                }
                gl.depth_mask(true);
                if later {
                    gl.depth_func(glow::LESS);
                    gl.enable(glow::CULL_FACE);
                    gl.disable(glow::BLEND);
                    gl.use_program(Some(m.program));
                }
                if queried {
                    if let Some(timer) = self.timer.as_mut() {
                        timer.end(gl);
                    }
                }
                if let Some(t) = t {
                    gl.finish();
                    self.stats.edges_us = t.elapsed().as_secs_f32() * 1e6;
                    self.stats.edges_gpu = false;
                }
            }

            // The pinned comparison design: translucent, depth-tested, writing no depth.
            if self.comparison.count > 0 {
                let [r, g, b, a] = COMPARISON_TINT;
                gl.uniform_1_i32(m.mode.as_ref(), Shade::Metal as i32);
                gl.uniform_3_f32(m.base_color.as_ref(), r, g, b);
                gl.uniform_1_f32(m.alpha.as_ref(), a);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.bind_vertex_array(Some(res.comparison.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.comparison.count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.uniform_1_f32(m.alpha.as_ref(), 1.0);
            }

            // A live command's ghost: its own buffer under a model matrix, translucent and depth-tested.
            if let (true, Some((model, normal))) = (ghost, self.preview_model) {
                gl.uniform_matrix_4_f32_slice(m.mvp.as_ref(), false, &mul4(&f.mvp, &model));
                gl.uniform_matrix_3_f32_slice(m.normal_matrix.as_ref(), false, &mul3(&f.normal_matrix, &normal));
                // Draft mode shades the staged class colours.
                gl.uniform_1_i32(m.mode.as_ref(), if self.preview_draft { Shade::Draft as i32 } else { Shade::Metal as i32 });
                gl.uniform_3_f32(m.base_color.as_ref(), PREVIEW_TINT[0], PREVIEW_TINT[1], PREVIEW_TINT[2]);
                gl.uniform_1_f32(m.alpha.as_ref(), if self.preview_draft { DRAFT_GHOST_ALPHA } else { PREVIEW_TINT[3] });
                gl.uniform_4_f32_slice(m.clip_plane.as_ref(), &[0.0; 4]);
                if look.preview_both_sides {
                    gl.disable(glow::CULL_FACE);
                }
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.bind_vertex_array(Some(res.preview.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.preview.count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.enable(glow::CULL_FACE);
                gl.uniform_matrix_4_f32_slice(m.mvp.as_ref(), false, &f.mvp);
                gl.uniform_matrix_3_f32_slice(m.normal_matrix.as_ref(), false, &f.normal_matrix);
                gl.uniform_1_f32(m.alpha.as_ref(), 1.0);
                gl.uniform_4_f32_slice(m.clip_plane.as_ref(), &f.clip_plane);
            }

            // The cutters, last and through everything.
            if self.cutters.count > 0 {
                let [r, g, b] = CUTTER_TINT;
                gl.uniform_3_f32(m.base_color.as_ref(), r, g, b);
                match look.cutters {
                    Cutters::Tinted(a) => {
                        gl.uniform_1_i32(m.mode.as_ref(), Shade::Metal as i32);
                        gl.uniform_1_f32(m.alpha.as_ref(), a);
                    }
                    Cutters::Rim(a) => {
                        gl.uniform_1_i32(m.mode.as_ref(), MODE_RIM);
                        gl.uniform_1_f32(m.alpha.as_ref(), a);
                        gl.disable(glow::CULL_FACE);
                    }
                }
                gl.disable(glow::DEPTH_TEST);
                gl.depth_mask(false);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.bind_vertex_array(Some(res.cutters.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.cutters.count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.uniform_1_f32(m.alpha.as_ref(), 1.0);
            }

            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::SCISSOR_TEST);
            if let Some(start) = start {
                gl.finish();
                self.stats.paint_us = start.elapsed().as_secs_f32() * 1e6;
            }
        }
        self.stats.frames += 1;
    }

    /// Builds the programs and buffers on the first paint, or records why it cannot.
    fn ensure(&mut self, gl: &glow::Context) {
        if self.res.is_some() || self.failed.is_some() {
            return;
        }
        let version = unsafe { gl.get_parameter_string(glow::VERSION) };
        let profile = Profile::from_version(&version);
        match unsafe { Resources::new(gl, profile, self.look.wire == Wire::Lines) } {
            Ok(res) => {
                self.res = Some(res);
                self.notes.push(GlNote::Ready { version, profile });
            }
            Err(e) => {
                self.failed = Some(e.clone());
                self.notes.push(GlNote::Failed(e));
            }
        }
    }

    /// Reads the depth attachment once: a window without one draws the ring see-through.
    ///
    /// The attachment's type is asked first, since on GLES 3.0 asking the size of a `NONE`
    /// attachment raises `GL_INVALID_OPERATION`, which egui_glow's error check reports every frame.
    fn check_depth(&mut self, gl: &glow::Context) {
        if self.depth_checked {
            return;
        }
        self.depth_checked = true;
        let kind = unsafe { gl.get_framebuffer_attachment_parameter_i32(glow::FRAMEBUFFER, glow::DEPTH, glow::FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE) };
        let bits = if kind == glow::NONE as i32 {
            0
        } else {
            unsafe { gl.get_framebuffer_attachment_parameter_i32(glow::FRAMEBUFFER, glow::DEPTH, glow::FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE) }
        };
        self.notes.push(GlNote::Depth(bits));
    }
}

/// Paints gathered per window, frames that uploaded a buffer reported apart.
#[derive(Clone, Debug, Default)]
pub struct Timings {
    paint: Vec<f32>,
    edges: Vec<f32>,
}

/// Paints a [`Timings`] window gathers before it reports.
pub const TIMING_WINDOW: usize = 240;

impl Timings {
    /// Adds a timed paint: a line for a frame that uploaded, and one per full window of the rest.
    pub fn record(&mut self, s: &Stats) -> Option<String> {
        if s.uploaded {
            return Some(format!(
                "render-stats upload-frame verts={} us={:.0} edge_verts={} edge_upload_bytes={} edge_upload_us={:.0}",
                s.mesh_vertices, s.paint_us, s.edge_vertices, s.edge_upload.0, s.edge_upload.1
            ));
        }
        self.paint.push(s.paint_us);
        self.edges.push(s.edges_us);
        if self.paint.len() < TIMING_WINDOW {
            return None;
        }
        let (paint, edges) = (summary(&mut self.paint), summary(&mut self.edges));
        let line = format!(
            "render-stats new verts={} frames={} mean_us={:.0} p50_us={:.0} p90_us={:.0} edge_verts={} lit_verts={} edges_mean_us={:.1} edges_p50_us={:.1} edges_p90_us={:.1} edges_clock={}",
            s.mesh_vertices, TIMING_WINDOW, paint[0], paint[1], paint[2], s.edge_vertices, s.lit_vertices, edges[0], edges[1], edges[2],
            if s.edges_gpu { "gpu" } else { "sync" }
        );
        self.paint.clear();
        self.edges.clear();
        Some(line)
    }
}

/// Mean, median and 90th percentile.
fn summary(samples: &mut [f32]) -> [f32; 3] {
    if samples.is_empty() {
        return [0.0; 3];
    }
    samples.sort_by(f32::total_cmp);
    let n = samples.len();
    [samples.iter().sum::<f32>() / n as f32, samples[n / 2], samples[n * 9 / 10]]
}

/// Uploads `data` into `vbo`; the vertices it holds at `floats` a vertex.
unsafe fn upload(gl: &glow::Context, vbo: glow::Buffer, data: &[f32], floats: usize, usage: u32) -> i32 {
    unsafe {
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes(data), usage);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
    }
    (data.len() / floats) as i32
}

fn bytes(data: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and every byte pattern is a valid u8.
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), std::mem::size_of_val(data)) }
}

/// Column-major `a · b` for 4x4 matrices.
fn mul4(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    std::array::from_fn(|i| {
        let (col, row) = (i / 4, i % 4);
        (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum()
    })
}

/// Column-major `a · b` for 3x3 matrices.
fn mul3(a: &[f32; 9], b: &[f32; 9]) -> [f32; 9] {
    std::array::from_fn(|i| {
        let (col, row) = (i / 3, i % 3);
        (0..3).map(|k| a[k * 3 + row] * b[col * 3 + k]).sum()
    })
}

/// Time-elapsed queries in flight round the edge pass.
const TIMER_QUERIES: usize = 4;

/// GPU time-elapsed queries round the edge pass, read back paints later so nothing waits on them.
struct GpuTimer {
    queries: [glow::Query; TIMER_QUERIES],
    issued: [bool; TIMER_QUERIES],
    next: usize,
}

impl GpuTimer {
    unsafe fn new(gl: &glow::Context) -> Option<Self> {
        let mut queries = Vec::with_capacity(TIMER_QUERIES);
        for _ in 0..TIMER_QUERIES {
            queries.push(unsafe { gl.create_query() }.ok()?);
        }
        Some(Self { queries: queries.try_into().ok()?, issued: [false; TIMER_QUERIES], next: 0 })
    }

    /// Starts timing into the next query, unless it is still in flight.
    unsafe fn begin(&mut self, gl: &glow::Context) -> bool {
        if self.issued[self.next] {
            return false;
        }
        unsafe { gl.begin_query(glow::TIME_ELAPSED, self.queries[self.next]) };
        true
    }

    unsafe fn end(&mut self, gl: &glow::Context) {
        unsafe { gl.end_query(glow::TIME_ELAPSED) };
        self.issued[self.next] = true;
        self.next = (self.next + 1) % TIMER_QUERIES;
    }

    /// The newest finished measurement, microseconds; the ones read are free again.
    unsafe fn poll(&mut self, gl: &glow::Context) -> Option<f32> {
        let mut newest = None;
        for k in 0..TIMER_QUERIES {
            let i = (self.next + k) % TIMER_QUERIES;
            if self.issued[i] && unsafe { gl.get_query_parameter_u32(self.queries[i], glow::QUERY_RESULT_AVAILABLE) } != 0 {
                newest = Some(unsafe { gl.get_query_parameter_u32(self.queries[i], glow::QUERY_RESULT) } as f32 / 1000.0);
                self.issued[i] = false;
            }
        }
        newest
    }

    unsafe fn delete(self, gl: &glow::Context) {
        for q in self.queries {
            unsafe { gl.delete_query(q) };
        }
    }
}

/// A vertex array and the buffer it reads.
#[derive(Clone, Copy)]
struct Pair {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
}

type Loc = Option<glow::UniformLocation>;

struct MeshProgram {
    program: glow::Program,
    mvp: Loc,
    normal_matrix: Loc,
    mode: Loc,
    light_dir: Loc,
    base_color: Loc,
    ambient: Loc,
    roughness: Loc,
    wire_color: Loc,
    wire_px: Loc,
    clip_plane: Loc,
    focus: Loc,
    select: Loc,
    hover: Loc,
    alpha: Loc,
}

struct WireProgram {
    program: glow::Program,
    mvp: Loc,
    wire_color: Loc,
    clip_plane: Loc,
}

struct EdgeProgram {
    program: glow::Program,
    mvp: Loc,
    viewport: Loc,
    px_per_pt: Loc,
    toward_eye: Loc,
    bias_mm: Loc,
    clip_plane: Loc,
    opacity: Loc,
}

/// Every GL object the renderer owns, uniform locations resolved once at link time.
struct Resources {
    profile: Profile,
    mesh: MeshProgram,
    wire: Option<WireProgram>,
    edge: EdgeProgram,
    ring: Pair,
    gems: Pair,
    cutters: Pair,
    comparison: Pair,
    preview: Pair,
    focus_vbo: glow::Buffer,
    select_vbo: glow::Buffer,
    edges: Pair,
    lit: Pair,
}

impl Resources {
    unsafe fn new(gl: &glow::Context, profile: Profile, lines: bool) -> Result<Self, String> {
        let mut built: Vec<glow::Program> = Vec::new();
        let mut build = |what: &str, vert: &str, frag: &str| -> Result<glow::Program, String> {
            match unsafe { compile(gl, &profile.source(vert), &profile.source(frag)) } {
                Ok(p) => {
                    built.push(p);
                    Ok(p)
                }
                Err(e) => {
                    for p in built.drain(..) {
                        unsafe { gl.delete_program(p) };
                    }
                    Err(format!("{what}: {e}"))
                }
            }
        };
        let mesh_program = build("metal", shaders::MESH_VS, &shaders::mesh_fs())?;
        let wire_program = match lines && profile.has_polygon_mode() {
            true => Some(build("wireframe", shaders::MESH_VS, shaders::WIRE_FS)?),
            false => None,
        };
        let edge_program = build("edges", shaders::EDGE_VS, shaders::EDGE_FS)?;
        let loc = |p: glow::Program, name: &str| unsafe { gl.get_uniform_location(p, name) };
        let pair = || -> Result<Pair, String> { unsafe { Ok(Pair { vao: gl.create_vertex_array()?, vbo: gl.create_buffer()? }) } };
        let (ring, gems, cutters, comparison, preview, edges, lit) = (pair()?, pair()?, pair()?, pair()?, pair()?, pair()?, pair()?);
        let (focus_vbo, select_vbo) = unsafe { (gl.create_buffer()?, gl.create_buffer()?) };
        let f = std::mem::size_of::<f32>() as i32;
        unsafe {
            for p in [ring, gems, cutters, comparison, preview] {
                gl.bind_vertex_array(Some(p.vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(p.vbo));
                let stride = MESH_FLOATS as i32 * f;
                for (index, offset) in [(0, 0), (1, 3 * f), (2, 6 * f), (3, 9 * f)] {
                    gl.enable_vertex_attrib_array(index);
                    gl.vertex_attrib_pointer_f32(index, 3, glow::FLOAT, false, stride, offset);
                }
            }
            // The channels ride the ring's own vertex array from buffers of their own, so a highlight is one small upload.
            gl.bind_vertex_array(Some(ring.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(focus_vbo));
            gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, f, 0);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(select_vbo));
            gl.vertex_attrib_pointer_f32(5, 2, glow::FLOAT, false, 2 * f, 0);
            // Every edge shares one width and colour, read as constants; a lit edge carries its own.
            for (p, floats, own) in [(edges, edges::EDGE_FLOATS, false), (lit, edges::LIT_FLOATS, true)] {
                gl.bind_vertex_array(Some(p.vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(p.vbo));
                let stride = floats as i32 * f;
                for (index, size, offset) in [(0, 3, 0), (1, 3, 3 * f), (2, 2, 6 * f)] {
                    gl.enable_vertex_attrib_array(index);
                    gl.vertex_attrib_pointer_f32(index, size, glow::FLOAT, false, stride, offset);
                }
                if own {
                    gl.enable_vertex_attrib_array(edges::ATTR_WIDTH);
                    gl.vertex_attrib_pointer_f32(edges::ATTR_WIDTH, 1, glow::FLOAT, false, stride, 8 * f);
                    gl.enable_vertex_attrib_array(edges::ATTR_COLOR);
                    gl.vertex_attrib_pointer_f32(edges::ATTR_COLOR, 4, glow::FLOAT, false, stride, 9 * f);
                }
            }
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        let m = mesh_program;
        Ok(Self {
            profile,
            mesh: MeshProgram {
                program: m,
                mvp: loc(m, "u_mvp"),
                normal_matrix: loc(m, "u_normal_matrix"),
                mode: loc(m, "u_mode"),
                light_dir: loc(m, "u_light_dir"),
                base_color: loc(m, "u_base_color"),
                ambient: loc(m, "u_ambient"),
                roughness: loc(m, "u_roughness"),
                wire_color: loc(m, "u_wire_color"),
                wire_px: loc(m, "u_wire_px"),
                clip_plane: loc(m, "u_clip_plane"),
                focus: loc(m, "u_focus"),
                select: loc(m, "u_select"),
                hover: loc(m, "u_hover"),
                alpha: loc(m, "u_alpha"),
            },
            wire: wire_program.map(|w| WireProgram { program: w, mvp: loc(w, "u_mvp"), wire_color: loc(w, "u_wire_color"), clip_plane: loc(w, "u_clip_plane") }),
            edge: EdgeProgram {
                program: edge_program,
                mvp: loc(edge_program, "u_mvp"),
                viewport: loc(edge_program, "u_viewport"),
                px_per_pt: loc(edge_program, "u_px_per_pt"),
                toward_eye: loc(edge_program, "u_toward_eye"),
                bias_mm: loc(edge_program, "u_bias_mm"),
                clip_plane: loc(edge_program, "u_clip_plane"),
                opacity: loc(edge_program, "u_opacity"),
            },
            ring,
            gems,
            cutters,
            comparison,
            preview,
            focus_vbo,
            select_vbo,
            edges,
            lit,
        })
    }

    unsafe fn delete(self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.mesh.program);
            if let Some(w) = self.wire {
                gl.delete_program(w.program);
            }
            gl.delete_program(self.edge.program);
            for p in [self.ring, self.gems, self.cutters, self.comparison, self.preview, self.edges, self.lit] {
                gl.delete_vertex_array(p.vao);
                gl.delete_buffer(p.vbo);
            }
            gl.delete_buffer(self.focus_vbo);
            gl.delete_buffer(self.select_vbo);
        }
    }
}

/// Links a program from two sources, deleting whatever it made when a stage will not build.
unsafe fn compile(gl: &glow::Context, vert: &str, frag: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut stages = Vec::with_capacity(2);
        for (kind, src, what) in [(glow::VERTEX_SHADER, vert, "vertex"), (glow::FRAGMENT_SHADER, frag, "fragment")] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                for s in stages {
                    gl.delete_shader(s);
                }
                gl.delete_program(program);
                return Err(format!("{what} shader: {log}"));
            }
            gl.attach_shader(program, shader);
            stages.push(shader);
        }
        gl.link_program(program);
        let linked = gl.get_program_link_status(program);
        let log = if linked { String::new() } else { gl.get_program_info_log(program) };
        for shader in stages {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        if !linked {
            gl.delete_program(program);
            return Err(format!("link: {log}"));
        }
        Ok(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_looks_differ_only_where_the_apps_did() {
        let (d, p) = (Look::DESKTOP, Look::PHONE);
        assert_eq!((d.wire, p.wire), (Wire::Lines, Wire::Barycentric));
        assert_eq!((d.cutters, p.cutters), (Cutters::Tinted(0.34), Cutters::Rim(0.85)));
        assert!(!d.preview_both_sides && p.preview_both_sides);
        assert!(!d.preview_hidden_by_wire && p.preview_hidden_by_wire);
        assert_eq!((d.edges.color, d.edges.chosen, d.edges.hover), (p.edges.color, p.edges.chosen, p.edges.hover));
        assert_eq!([Shade::Metal, Shade::Draft, Shade::Normals, Shade::Wall, Shade::Halves].map(|s| s as i32), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn a_timing_window_reports_once_full_and_an_upload_at_once() {
        let mut t = Timings::default();
        let paint = |us: f32, uploaded: bool| Stats { paint_us: us, edges_us: us / 10.0, uploaded, mesh_vertices: 36, edge_vertices: 6, ..Stats::default() };
        let upload = t.record(&paint(5000.0, true)).expect("an upload is said at once");
        assert!(upload.starts_with("render-stats upload-frame verts=36 us=5000"), "{upload}");
        let lines: Vec<String> = (1..=TIMING_WINDOW).filter_map(|i| t.record(&paint(i as f32, false))).collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("frames=240 mean_us=120 p50_us=121 p90_us=217"), "{}", lines[0]);
        assert!(lines[0].contains("edge_verts=6") && lines[0].contains("edges_p50_us=12"), "{}", lines[0]);
        assert_eq!(t.record(&paint(1.0, false)), None, "the window starts over");
        assert_eq!([None, Some("1"), Some("passes")].map(Timing::from_env), [Timing::Off, Timing::Paint, Timing::Passes]);
    }

    #[test]
    fn a_new_build_restages_every_edge_and_a_new_choice_only_the_lit_ones() {
        let d = ringdesign_core::cad::examples::design("claw-solitaire").unwrap();
        let built = ringdesign_core::mesh::try_build(&d, &ringdesign_core::AlphaLibrary::default(), ringdesign_core::mesh::BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap();
        let e = built.parts.evaluated.as_ref().unwrap();
        let head = e.components.iter().find(|c| c.name == "Four-claw head").unwrap().id;
        let mut r = MeshRenderer::new(Look::DESKTOP);
        assert!(r.sync_edges(1, Some(e), &[], None));
        assert_eq!(r.edges_key(), Some(1));
        assert!(!r.edge_runs().is_empty());
        assert_eq!(r.pending_lit(), Some(&[][..]), "nothing chosen lights nothing");
        assert!(!r.sync_edges(1, Some(e), &[], None), "the same frame again stages nothing");
        assert!(r.sync_edges(1, Some(e), &[(head, 0)], None));
        let lit = r.pending_lit().unwrap().len();
        assert_eq!(lit % (edges::LIT_FLOATS * edges::SEGMENT_VERTICES), 0);
        assert!(lit > 0);
        assert!(r.sync_edges(1, Some(e), &[(head, 0)], Some((head, 1))), "a hover restages");
        assert!(r.pending_lit().unwrap().len() > lit);
        assert!(r.sync_edges(2, None, &[], None), "a build without parts clears them");
        assert!(r.edge_runs().is_empty());
    }
}
