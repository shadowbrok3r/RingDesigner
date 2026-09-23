//! GPU mesh renderer, ported from the desktop's `viewport.rs` to OpenGL ES 3.0.
//!
//! Four things differ from the desktop version, all forced by GLES:
//!
//! - **No `glPolygonMode`, no `GL_POLYGON_OFFSET_LINE`.** Neither exists in GLES at any version, and
//!   glow does not degrade gracefully — a missing entry point panics. The desktop called
//!   `polygon_mode(FILL)` unconditionally on the normal draw path, so this is fatal on frame 1 even
//!   with the wireframe off.
//! - **The wireframe is barycentric and single-pass.** The corner index comes from
//!   `gl_VertexID % 3`, which is free because the mesh is non-indexed, so it needs no extra
//!   attribute, no extra buffer and no second draw call. `glLineWidth` was never an option:
//!   `ALIASED_LINE_WIDTH_RANGE` is [1, 1] on Adreno and Mali.
//! - **The `#version` header is chosen at runtime.** `330 core` on desktop, `300 es` on GLES — not
//!   the `140` the in-tree backdrop-blur precedent uses, because all three attributes here are
//!   declared `layout(location = N)`, which needs GL 3.3. `precision highp float` is mandatory
//!   rather than stylistic: GLSL ES 3.00 defines no default float precision for fragment shaders,
//!   and the narrow studio reflections need `highp` regardless.
//! - **Shader failure is recoverable.** The desktop panicked, which on a device is a process kill
//!   with the message only in logcat.

use egui_glow::glow;
use glow::HasContext;

use ringdesign_core::castability::CastReport;
use ringdesign_core::mesh::Mesh;
#[cfg(test)]
use ringdesign_core::mesh::Vec3;

/// Floats per vertex: position(3), normal(3), draft colour(3), wall colour(3).
const FLOATS_PER_VERTEX: usize = 12;

/// Wireframe line half-width, in fragments.
const WIRE_PX: f32 = 0.6;

const VERTEX_BODY: &str = r#"
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_color;
layout(location = 3) in vec3 a_wall;
layout(location = 4) in float a_focus;
layout(location = 5) in vec2 a_select;

uniform mat4 u_mvp;
uniform mat3 u_normal_matrix;

out vec3 v_normal;
out vec3 v_color;
out vec3 v_wall;
out vec3 v_bary;
out float v_obj_nz;
out vec3 v_world;
out float v_cavity;
out float v_focus;
out vec2 v_select;

void main() {
    gl_Position = u_mvp * vec4(a_position, 1.0);
    v_focus = a_focus;
    v_select = a_select;
    v_normal = u_normal_matrix * a_normal;
    v_color = a_color;
    v_wall = a_wall;
    v_obj_nz = a_normal.z;
    v_world = a_position;
    // Approximate bore occlusion; assessment colours do not use it.
    v_cavity = smoothstep(0.0, 0.55, -dot(a_normal.xy, normalize(a_position.xy + vec2(0.000001))));
    // Non-indexed triangles, so the corner index is the vertex index mod 3 and the
    // barycentric coordinate costs nothing to carry.
    int corner = gl_VertexID % 3;
    v_bary = vec3(corner == 0 ? 1.0 : 0.0, corner == 1 ? 1.0 : 0.0, corner == 2 ? 1.0 : 0.0);
}
"#;

const FRAGMENT_BODY: &str = r#"
in vec3 v_normal;
in vec3 v_color;
in vec3 v_wall;
in vec3 v_bary;
in float v_obj_nz;
in vec3 v_world;
in float v_cavity;
in float v_focus;
in vec2 v_select;
uniform vec4 u_clip_plane;
uniform vec4 u_focus;
uniform vec4 u_select;
uniform vec4 u_hover;
uniform float u_alpha;

uniform int u_mode;
uniform vec3 u_light_dir;
uniform vec3 u_base_color;
uniform float u_ambient;
uniform vec3 u_wire_color;
uniform float u_wire_px;

out vec4 frag_color;

// STUDIO_MATERIAL

void main() {
    if (dot(vec4(v_world, 1.0), u_clip_plane) > 0.00001) discard;
    vec3 n = normalize(v_normal);
    vec3 eye = vec3(0.0, 0.0, 1.0);
    vec3 l = normalize(u_light_dir);
    vec3 color;

    if (u_mode == 6) {
        // A ghost: faint face on, bright where the surface turns away, so a cutter reads as its outline.
        float rim = 1.0 - abs(n.z);
        frag_color = vec4(u_base_color, u_alpha * (0.16 + 0.84 * rim * rim));
        return;
    }
    if (u_mode == 5) {
        color = studio_gem(n, v_color, l, u_ambient);
    } else if (u_mode == 4) {
        float lambert = max(dot(n, l), 0.0);
        vec3 half_c = v_obj_nz > 0.0 ? vec3(0.42, 0.62, 0.82) : vec3(0.80, 0.62, 0.38);
        float band = 1.0 - smoothstep(0.035, 0.09, abs(v_obj_nz));
        color = mix(half_c, vec3(1.0, 0.92, 0.25), band) * (0.72 + 0.28 * lambert);
    } else if (u_mode == 3) {
        float lambert = max(dot(n, l), 0.0);
        color = v_wall * (0.74 + 0.26 * lambert);
    } else if (u_mode == 2) {
        color = n * 0.5 + 0.5;
    } else if (u_mode == 1) {
        float lambert = max(dot(n, l), 0.0);
        color = v_color * (0.74 + 0.26 * lambert);
    } else {
        color = studio_metal(n, u_base_color, l, u_ambient, v_cavity);
    }

    // The chosen node's reach: its tint over whatever the mode drew, a little
    // of the key light kept so relief still reads under it.
    float focus = clamp(v_focus, 0.0, 1.0) * u_focus.a;
    if (focus > 0.0) {
        float lit = 0.62 + 0.38 * max(dot(n, l), 0.0);
        color = mix(color, u_focus.rgb * lit, focus);
    }
    // The chosen parts over that, and what the finger rests on over both.
    float chosen = clamp(v_select.x, 0.0, 1.0) * u_select.a;
    float hovered = clamp(v_select.y, 0.0, 1.0) * u_hover.a;
    if (chosen > 0.0 || hovered > 0.0) {
        float lit = 0.62 + 0.38 * max(dot(n, l), 0.0);
        color = mix(color, u_select.rgb * lit, chosen);
        color = mix(color, u_hover.rgb * lit, hovered);
    }

    if (u_wire_px > 0.0) {
        vec3 w = fwidth(v_bary) * u_wire_px;
        vec3 a = smoothstep(vec3(0.0), w, v_bary);
        float edge = 1.0 - min(min(a.x, a.y), a.z);
        color = mix(color, u_wire_color, edge * 0.55);
    }

    frag_color = vec4(color, u_alpha);
}
"#;

/// `330 core` on desktop GL, `300 es` on GLES. Classified from the driver's version string the way
/// backdrop-blur's `profile.rs` does it, rather than from a build-time cfg, so one binary is right
/// either way.
fn shader_header(gl: &glow::Context) -> &'static str {
    let version = unsafe { gl.get_parameter_string(glow::VERSION) };
    if version.contains("OpenGL ES") || version.contains("WebGL") {
        "#version 300 es\nprecision highp float;\nprecision highp int;\n"
    } else {
        "#version 330 core\n"
    }
}

/// Uniform locations, resolved once at link time. On mobile drivers
/// `glGetUniformLocation` is a real string comparison, and the desktop did seven of them per frame.
struct Uniforms {
    mvp: Option<glow::NativeUniformLocation>,
    normal_matrix: Option<glow::NativeUniformLocation>,
    mode: Option<glow::NativeUniformLocation>,
    light_dir: Option<glow::NativeUniformLocation>,
    base_color: Option<glow::NativeUniformLocation>,
    ambient: Option<glow::NativeUniformLocation>,
    roughness: Option<glow::NativeUniformLocation>,
    wire_color: Option<glow::NativeUniformLocation>,
    wire_px: Option<glow::NativeUniformLocation>,
    clip_plane: Option<glow::NativeUniformLocation>,
    focus: Option<glow::NativeUniformLocation>,
    select: Option<glow::NativeUniformLocation>,
    hover: Option<glow::NativeUniformLocation>,
    alpha: Option<glow::NativeUniformLocation>,
}

struct GpuResources {
    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,
    gem_vao: glow::NativeVertexArray,
    gem_vbo: glow::NativeBuffer,
    ghost_vao: glow::NativeVertexArray,
    ghost_vbo: glow::NativeBuffer,
    /// A live command's ghost: staged once, moved by a model matrix.
    preview_vao: glow::NativeVertexArray,
    preview_vbo: glow::NativeBuffer,
    /// One float a staged vertex: how far the chosen node reaches it.
    focus_vbo: glow::NativeBuffer,
    /// Two floats a staged vertex: chosen, and under the finger.
    select_vbo: glow::NativeBuffer,
    uniforms: Uniforms,
}

/// The chosen parts' tint and strength: the desktop's selection violet.
pub const SELECT_TINT: [f32; 4] = [0.80, 0.57, 0.85, 0.55];
/// What the finger rests on: the hover cue's aqua.
pub const HOVER_TINT: [f32; 4] = [0.40, 0.85, 0.83, 0.55];
/// A live command's ghost: the hover aqua, and its opacity.
pub const PREVIEW_TINT: [f32; 4] = [0.40, 0.85, 0.83, 0.45];
/// A ghost painted by its draft classes, opacity.
const DRAFT_GHOST_ALPHA: f32 = 0.6;

#[derive(Default)]
pub struct GpuMeshRenderer {
    resources: Option<GpuResources>,
    vertex_count: i32,
    gem_count: i32,
    ghost_count: i32,
    preview_count: i32,
    pending: Option<Vec<f32>>,
    pending_gems: Option<Vec<f32>>,
    pending_ghost: Option<Vec<f32>>,
    /// A live command's ghost awaiting upload; `Some(empty)` clears it.
    pending_preview: Option<Vec<f32>>,
    /// Where the ghost stands: its column-major model matrix and the normals' 3x3; `None` hides it.
    preview_model: Option<([f32; 16], [f32; 9])>,
    /// The ghost shades in its staged draft colours rather than the aqua.
    preview_draft: bool,
    /// A focus channel awaiting upload; `Some(empty)` clears it.
    pending_focus: Option<Vec<f32>>,
    /// The uploaded focus channel covers the uploaded mesh vertex for vertex.
    focus_live: bool,
    /// The selection channel awaiting upload, two floats a staged vertex; `Some(empty)` clears it.
    pending_select: Option<Vec<f32>>,
    select_live: bool,
    depth_checked: bool,
    /// Set once if the shaders will not build, so the pane can say so instead of drawing nothing.
    pub failed: Option<String>,
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

// glow handles are u32 integers on native, safe to send across threads.
unsafe impl Send for GpuMeshRenderer {}
unsafe impl Sync for GpuMeshRenderer {}

impl GpuMeshRenderer {
    /// Flatten the mesh into an interleaved vertex buffer awaiting upload. Runs on the build
    /// worker, not the UI thread — at Preview resolution this fills ~12 MB, which is more than a
    /// frame's budget.
    /// `wall` is `(inner_radius_mm, min_section_mm)`, baked alongside the
    /// draft colours so switching shade modes never re-uploads.
    /// The focus channel for `mesh`: `weight[v]` of each vertex, in the
    /// order [`stage`](Self::stage) emits them — the same faces skipped, so
    /// the two buffers stay vertex for vertex.
    pub fn stage_focus(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        let mut data = Vec::with_capacity(mesh.faces.len() * 3);
        for face in &mesh.faces {
            if face.iter().any(|&vi| !mesh.vertices.get(vi as usize).is_some_and(|p| p.is_finite())) {
                continue;
            }
            data.extend(face.iter().map(|&vi| weight.get(vi as usize).copied().unwrap_or(0.0)));
        }
        data
    }

    pub fn stage(mesh: &Mesh, cast: Option<&CastReport>, wall: (f64, f64)) -> Vec<f32> {
        let (inner_r, min_section) = wall;
        let mut data: Vec<f32> = Vec::with_capacity(mesh.faces.len() * 3 * FLOATS_PER_VERTEX);

        let mut hard = 0;
        'faces: for (i, face) in mesh.faces.iter().enumerate() {
            // A solid's faces keep their creases; the band shades from its own vertex normals.
            let corners = mesh.face_normals(i, &mut hard);
            let rgb = match cast {
                Some(c) => c.classes.get(i).map_or([1.0; 3], |k| k.rgb()),
                None => [1.0; 3],
            };
            let mut tri = [[0.0f32; FLOATS_PER_VERTEX]; 3];
            for (k, &vi) in face.iter().enumerate() {
                let Some(p) = mesh.vertices.get(vi as usize).filter(|p| p.is_finite()) else {
                    continue 'faces;
                };
                let n = corners[k];
                // Radial metal under this vertex; the bore itself (facing
                // inward) is not a wall and sits out in neutral grey.
                let r = (p.0 as f64).hypot(p.1 as f64);
                let inward = (n.0 as f64 * p.0 as f64 + n.1 as f64 * p.1 as f64) < 0.0;
                let w = if inward {
                    WALL_NEUTRAL
                } else {
                    wall_color(r - inner_r, min_section)
                };
                tri[k] = [
                    p.0, p.1, p.2, n.0, n.1, n.2, rgb[0], rgb[1], rgb[2], w[0], w[1], w[2],
                ];
            }
            for v in &tri {
                data.extend_from_slice(v);
            }
        }
        data
    }

    /// Hand the renderer a buffer built by [`stage`](Self::stage) on the worker.
    pub fn set_pending(&mut self, verts: Vec<f32>) {
        self.pending = Some(verts);
    }

    /// Queue stone-preview triangles in the same layout. Empty clears them.
    /// The cutters' ghost, in the stones' layout; empty clears it.
    pub fn set_pending_ghost(&mut self, verts: Vec<f32>) {
        self.pending_ghost = Some(verts);
    }

    pub fn set_pending_gems(&mut self, verts: Vec<f32>) {
        self.pending_gems = Some(verts);
    }

    /// Queue a focus channel built by [`stage_focus`](Self::stage_focus)
    /// for the mesh on screen. Empty clears the highlight.
    pub fn set_pending_focus(&mut self, weights: Vec<f32>) {
        self.pending_focus = Some(weights);
    }

    pub fn has_focus(&self) -> bool {
        self.focus_live || self.pending_focus.as_ref().is_some_and(|w| !w.is_empty())
    }

    /// The selection channel in [`stage`](Self::stage)'s vertex order: `(chosen, under the finger)` from `viewport::tint`'s weights.
    pub fn stage_select(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        let mut data = Vec::with_capacity(mesh.faces.len() * 6);
        for face in &mesh.faces {
            if face.iter().any(|&vi| !mesh.vertices.get(vi as usize).is_some_and(|p| p.is_finite())) {
                continue;
            }
            for &vi in face {
                let w = weight.get(vi as usize).copied().unwrap_or(0.0);
                data.push(if (0.5..1.5).contains(&w) { 1.0 } else { 0.0 });
                data.push(if w >= 1.5 { 1.0 } else { 0.0 });
            }
        }
        data
    }

    /// Queue a selection channel built by [`stage_select`](Self::stage_select). Empty clears it.
    pub fn set_pending_select(&mut self, weights: Vec<f32>) {
        self.pending_select = Some(weights);
    }

    /// Whether a selection tint is uploaded or on its way.
    pub fn has_select(&self) -> bool {
        self.select_live || self.pending_select.as_ref().is_some_and(|w| !w.is_empty())
    }

    /// Queue a live command's ghost in its own coordinates, staged by [`stage_part`](Self::stage_part). Empty clears it.
    pub fn set_pending_preview(&mut self, verts: Vec<f32>) {
        self.pending_preview = Some(verts);
    }

    /// Where the ghost stands: model matrix and normal matrix, column-major; `None` hides it.
    pub fn set_preview_model(&mut self, model: Option<([f32; 16], [f32; 9])>) {
        self.preview_model = model;
    }

    /// Whether the ghost shades in its staged draft colours.
    pub fn set_preview_draft(&mut self, draft: bool) {
        self.preview_draft = draft;
    }

    /// The ghost as it stands: its vertices awaiting upload, where it is drawn, and whether it shades by draft class.
    pub fn preview_state(&self) -> (Option<&[f32]>, Option<([f32; 16], [f32; 9])>, bool) {
        (self.pending_preview.as_deref(), self.preview_model, self.preview_draft)
    }

    /// A part's tessellation in the interleaved layout, a vertex normal kept only within 20° of its facet's.
    pub fn stage_part(mesh: &Mesh) -> Vec<f32> {
        Self::stage_part_colored(mesh, |_| [1.0; 3])
    }

    /// [`stage_part`](Self::stage_part) with every face in its draft class's colour.
    pub fn stage_part_classes(mesh: &Mesh, classes: &[ringdesign_core::FaceClass]) -> Vec<f32> {
        Self::stage_part_colored(mesh, |i| classes.get(i).map_or([1.0; 3], |k| k.rgb()))
    }

    /// A part's tessellation with face `i` in `color(i)`.
    fn stage_part_colored(mesh: &Mesh, color: impl Fn(usize) -> [f32; 3]) -> Vec<f32> {
        let mut data = Vec::with_capacity(mesh.faces.len() * 3 * FLOATS_PER_VERTEX);
        for (i, face) in mesh.faces.iter().enumerate() {
            let Some(facet) = mesh.face_normal(face) else { continue };
            let facet = ringdesign_core::mesh::Vec3(facet[0] as f32, facet[1] as f32, facet[2] as f32);
            let Some(points) = face.iter().map(|&vi| mesh.vertices.get(vi as usize).filter(|p| p.is_finite()).copied()).collect::<Option<Vec<_>>>() else { continue };
            let [r, g, b] = color(i);
            for (p, &vi) in points.iter().zip(face) {
                let n = match mesh.normals.get(vi as usize) {
                    Some(n) if n.is_finite() && n.0 * facet.0 + n.1 * facet.1 + n.2 * facet.2 > 0.94 => *n,
                    _ => facet,
                };
                data.extend_from_slice(&[p.0, p.1, p.2, n.0, n.1, n.2, r, g, b, 1.0, 1.0, 1.0]);
            }
        }
        data
    }

    pub fn has_mesh(&self) -> bool {
        self.vertex_count > 0 || self.pending.is_some()
    }

    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        gl: &glow::Context,
        info: egui::PaintCallbackInfo,
        mvp: &[f32; 16],
        normal_matrix: &[f32; 9],
        mode: i32,
        base_color: [f32; 3],
        roughness: f32,
        wireframe: bool,
        wire_color: [f32; 3],
        clip_plane: [f32; 4],
        focus: [f32; 4],
    ) {
        self.ensure_resources(gl);
        self.warn_if_no_depth_buffer(gl);
        let Some(res) = self.resources.as_ref() else { return };

        if let Some(verts) = self.pending.take() {
            self.vertex_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            // A channel staged for the last mesh says nothing about this one.
            self.focus_live = false;
            self.select_live = false;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(weights) = self.pending_focus.take() {
            self.focus_live = !weights.is_empty() && weights.len() as i32 == self.vertex_count;
            if self.focus_live {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.focus_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&weights), glow::STATIC_DRAW);
                    gl.bind_buffer(glow::ARRAY_BUFFER, None);
                }
            }
        }
        if let Some(weights) = self.pending_select.take() {
            self.select_live = !weights.is_empty() && weights.len() as i32 == self.vertex_count * 2;
            if self.select_live {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.select_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&weights), glow::STATIC_DRAW);
                    gl.bind_buffer(glow::ARRAY_BUFFER, None);
                }
            }
        }
        if let Some(verts) = self.pending_preview.take() {
            self.preview_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.preview_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::DYNAMIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(verts) = self.pending_gems.take() {
            self.gem_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.gem_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }

        if let Some(verts) = self.pending_ghost.take() {
            self.ghost_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.ghost_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }

        if self.vertex_count == 0 {
            return;
        }

        unsafe {
            let vp = info.viewport_in_pixels();
            gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
            gl.scissor(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);

            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.depth_func(glow::LESS);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.enable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(res.program));
            gl.bind_vertex_array(Some(res.vao));

            let u = &res.uniforms;
            gl.uniform_matrix_4_f32_slice(u.mvp.as_ref(), false, mvp);
            gl.uniform_matrix_3_f32_slice(u.normal_matrix.as_ref(), false, normal_matrix);
            gl.uniform_1_i32(u.mode.as_ref(), mode);
            gl.uniform_3_f32(u.light_dir.as_ref(), -0.38, 0.46, 0.80);
            gl.uniform_3_f32(u.base_color.as_ref(), base_color[0], base_color[1], base_color[2]);
            gl.uniform_1_f32(u.ambient.as_ref(), 0.20);
            gl.uniform_1_f32(u.roughness.as_ref(), roughness);
            gl.uniform_3_f32(u.wire_color.as_ref(), wire_color[0], wire_color[1], wire_color[2]);
            gl.uniform_1_f32(u.wire_px.as_ref(), if wireframe { WIRE_PX } else { 0.0 });
            gl.uniform_4_f32_slice(u.clip_plane.as_ref(), &clip_plane);
            gl.uniform_1_f32(u.alpha.as_ref(), 1.0);
            // Without a channel the attribute is a constant zero: no buffer
            // is read, so a stale one can never be read past its end.
            let lit = self.focus_live && focus[3] > 0.0;
            gl.uniform_4_f32_slice(u.focus.as_ref(), &if lit { focus } else { [0.0; 4] });
            if lit {
                gl.enable_vertex_attrib_array(4);
            } else {
                gl.disable_vertex_attrib_array(4);
                gl.vertex_attrib_1_f32(4, 0.0);
            }
            // The selection rides its own buffer on attribute 5, gated the same way.
            let chosen = self.select_live;
            gl.uniform_4_f32_slice(u.select.as_ref(), &if chosen { SELECT_TINT } else { [0.0; 4] });
            gl.uniform_4_f32_slice(u.hover.as_ref(), &if chosen { HOVER_TINT } else { [0.0; 4] });
            if chosen {
                gl.enable_vertex_attrib_array(5);
            } else {
                gl.disable_vertex_attrib_array(5);
                gl.vertex_attrib_2_f32(5, 0.0, 0.0);
            }

            gl.draw_arrays(glow::TRIANGLES, 0, self.vertex_count);
            gl.uniform_4_f32_slice(u.select.as_ref(), &[0.0; 4]);
            gl.uniform_4_f32_slice(u.hover.as_ref(), &[0.0; 4]);

            // Stones ride in a second buffer with the same program: dielectric
            // shading, their own tint, whatever the ring's mode is.
            if self.gem_count > 0 {
                gl.uniform_4_f32_slice(u.focus.as_ref(), &[0.0; 4]);
                gl.uniform_1_i32(u.mode.as_ref(), 5);
                gl.uniform_3_f32(
                    u.base_color.as_ref(),
                    ringdesign_core::gems::GEM_TINT[0],
                    ringdesign_core::gems::GEM_TINT[1],
                    ringdesign_core::gems::GEM_TINT[2],
                );
                gl.bind_vertex_array(Some(res.gem_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.gem_count);
            }

            // A live command's ghost: its own buffer under a model matrix, translucent and depth-tested, both sides drawn.
            if let (true, Some((model, normal))) = (self.preview_count > 0 && !wireframe, self.preview_model) {
                gl.uniform_4_f32_slice(u.focus.as_ref(), &[0.0; 4]);
                gl.uniform_matrix_4_f32_slice(u.mvp.as_ref(), false, &mul4(mvp, &model));
                gl.uniform_matrix_3_f32_slice(u.normal_matrix.as_ref(), false, &mul3(normal_matrix, &normal));
                // Mode 1 shades the staged draft colours.
                gl.uniform_1_i32(u.mode.as_ref(), if self.preview_draft { 1 } else { 0 });
                gl.uniform_3_f32(u.base_color.as_ref(), PREVIEW_TINT[0], PREVIEW_TINT[1], PREVIEW_TINT[2]);
                gl.uniform_1_f32(u.alpha.as_ref(), if self.preview_draft { DRAFT_GHOST_ALPHA } else { PREVIEW_TINT[3] });
                gl.uniform_4_f32_slice(u.clip_plane.as_ref(), &[0.0; 4]);
                gl.uniform_1_f32(u.wire_px.as_ref(), 0.0);
                gl.disable(glow::CULL_FACE);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.bind_vertex_array(Some(res.preview_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.preview_count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.enable(glow::CULL_FACE);
                gl.uniform_matrix_4_f32_slice(u.mvp.as_ref(), false, mvp);
                gl.uniform_matrix_3_f32_slice(u.normal_matrix.as_ref(), false, normal_matrix);
                gl.uniform_1_f32(u.alpha.as_ref(), 1.0);
                gl.uniform_4_f32_slice(u.clip_plane.as_ref(), &clip_plane);
            }

            // The cutters' ghost: over everything, blended, writing no depth — the tool is inside
            // the metal it removes, and a depth test would hide exactly what is being asked for.
            if self.ghost_count > 0 {
                gl.uniform_4_f32_slice(u.focus.as_ref(), &[0.0; 4]);
                gl.uniform_1_f32(u.wire_px.as_ref(), 0.0);
                gl.uniform_1_i32(u.mode.as_ref(), 6);
                gl.uniform_3_f32(u.base_color.as_ref(), 1.0, 0.34, 0.62);
                gl.uniform_1_f32(u.alpha.as_ref(), 0.85);
                gl.disable(glow::DEPTH_TEST);
                gl.depth_mask(false);
                gl.disable(glow::CULL_FACE);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.bind_vertex_array(Some(res.ghost_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.ghost_count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
            }

            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::SCISSOR_TEST);
        }
    }

    /// Depth testing is silently a no-op on a window with no depth attachment, which reads as a
    /// see-through ring rather than as an error. Checked once.
    ///
    /// The attachment type is queried first: on GLES 3.0, asking for `DEPTH_SIZE` of a `NONE`
    /// attachment raises `GL_INVALID_OPERATION`, and egui_glow's post-callback error check would
    /// then report it every frame.
    fn warn_if_no_depth_buffer(&mut self, gl: &glow::Context) {
        if self.depth_checked {
            return;
        }
        self.depth_checked = true;
        let kind = unsafe {
            gl.get_framebuffer_attachment_parameter_i32(
                glow::FRAMEBUFFER,
                glow::DEPTH,
                glow::FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE,
            )
        };
        let bits = if kind == glow::NONE as i32 {
            0
        } else {
            unsafe {
                gl.get_framebuffer_attachment_parameter_i32(
                    glow::FRAMEBUFFER,
                    glow::DEPTH,
                    glow::FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE,
                )
            }
        };
        if bits <= 0 {
            log::warn!(
                "no depth buffer on the default framebuffer ({bits} bits): the ring will draw \
                 see-through. app! needs its third argument, e.g. app!(App::new, Backend::Glow, 24)."
            );
        } else {
            log::info!("depth buffer: {bits} bits");
        }
    }

    fn ensure_resources(&mut self, gl: &glow::Context) {
        if self.resources.is_some() || self.failed.is_some() {
            return;
        }

        let header = shader_header(gl);
        log::info!("GL_VERSION: {}", unsafe { gl.get_parameter_string(glow::VERSION) });

        let program = match compile_program(
            gl,
            &format!("{header}{VERTEX_BODY}"),
            &format!("{header}{}", FRAGMENT_BODY.replace("// STUDIO_MATERIAL", ringdesign_core::render::STUDIO_GLSL)),
        ) {
            Ok(p) => p,
            Err(e) => {
                log::error!("ring shader: {e}");
                self.failed = Some(e);
                return;
            }
        };

        let (Some(vao), Some(vbo), Some(gem_vao), Some(gem_vbo), Some(ghost_vao), Some(ghost_vbo), Some(focus_vbo)) = (
            unsafe { gl.create_vertex_array() }.ok(),
            unsafe { gl.create_buffer() }.ok(),
            unsafe { gl.create_vertex_array() }.ok(),
            unsafe { gl.create_buffer() }.ok(),
            unsafe { gl.create_vertex_array() }.ok(),
            unsafe { gl.create_buffer() }.ok(),
            unsafe { gl.create_buffer() }.ok(),
        ) else {
            self.failed = Some("could not create VAO/VBO".into());
            return;
        };
        let (Some(preview_vao), Some(preview_vbo), Some(select_vbo)) = (
            unsafe { gl.create_vertex_array() }.ok(),
            unsafe { gl.create_buffer() }.ok(),
            unsafe { gl.create_buffer() }.ok(),
        ) else {
            self.failed = Some("could not create the ghost's VAO/VBO".into());
            return;
        };

        let uniforms = unsafe {
            Uniforms {
                mvp: gl.get_uniform_location(program, "u_mvp"),
                normal_matrix: gl.get_uniform_location(program, "u_normal_matrix"),
                mode: gl.get_uniform_location(program, "u_mode"),
                light_dir: gl.get_uniform_location(program, "u_light_dir"),
                base_color: gl.get_uniform_location(program, "u_base_color"),
                ambient: gl.get_uniform_location(program, "u_ambient"),
                roughness: gl.get_uniform_location(program, "u_roughness"),
                wire_color: gl.get_uniform_location(program, "u_wire_color"),
                wire_px: gl.get_uniform_location(program, "u_wire_px"),
                clip_plane: gl.get_uniform_location(program, "u_clip_plane"),
                focus: gl.get_uniform_location(program, "u_focus"),
                select: gl.get_uniform_location(program, "u_select"),
                hover: gl.get_uniform_location(program, "u_hover"),
                alpha: gl.get_uniform_location(program, "u_alpha"),
            }
        };

        unsafe {
            let f = std::mem::size_of::<f32>() as i32;
            let stride = FLOATS_PER_VERTEX as i32 * f;
            for (va, vb) in [(vao, vbo), (gem_vao, gem_vbo), (ghost_vao, ghost_vbo), (preview_vao, preview_vbo)] {
                gl.bind_vertex_array(Some(va));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vb));
                for (loc, offset) in [(0, 0), (1, 3 * f), (2, 6 * f), (3, 9 * f)] {
                    gl.enable_vertex_attrib_array(loc);
                    gl.vertex_attrib_pointer_f32(loc, 3, glow::FLOAT, false, stride, offset);
                }
                gl.bind_vertex_array(None);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
            // The focus channel rides the ring's VAO from a buffer of its
            // own, so a highlight is one small upload and never a re-stage.
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(focus_vbo));
            gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, f, 0);
            // The selection likewise, two floats a vertex on attribute 5.
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(select_vbo));
            gl.vertex_attrib_pointer_f32(5, 2, glow::FLOAT, false, 2 * f, 0);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }

        self.resources = Some(GpuResources { program, vao, vbo, gem_vao, gem_vbo, ghost_vao, ghost_vbo, preview_vao, preview_vbo, focus_vbo, select_vbo, uniforms });
    }
}

fn compile_program(
    gl: &glow::Context,
    vert_src: &str,
    frag_src: &str,
) -> Result<glow::NativeProgram, String> {
    let program = unsafe { gl.create_program() }?;

    let mut shaders = Vec::with_capacity(2);
    for (kind, src, what) in [
        (glow::VERTEX_SHADER, vert_src, "vertex"),
        (glow::FRAGMENT_SHADER, frag_src, "fragment"),
    ] {
        let shader = unsafe { gl.create_shader(kind) }?;
        unsafe {
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
        }
        if !unsafe { gl.get_shader_compile_status(shader) } {
            let log = unsafe { gl.get_shader_info_log(shader) };
            unsafe { gl.delete_shader(shader) };
            for s in shaders {
                unsafe { gl.delete_shader(s) };
            }
            unsafe { gl.delete_program(program) };
            return Err(format!("{what} shader: {log}"));
        }
        unsafe { gl.attach_shader(program, shader) };
        shaders.push(shader);
    }

    unsafe { gl.link_program(program) };
    if !unsafe { gl.get_program_link_status(program) } {
        let log = unsafe { gl.get_program_info_log(program) };
        unsafe { gl.delete_program(program) };
        return Err(format!("link: {log}"));
    }

    for shader in shaders {
        unsafe {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
    }
    Ok(program)
}

fn as_u8_slice<T: Copy>(data: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
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

    fn code(self) -> i32 {
        match self {
            ShadeMode::Metal => 0,
            ShadeMode::Draft => 1,
            ShadeMode::Normals => 2,
            ShadeMode::Wall => 3,
            ShadeMode::Halves => 4,
        }
    }
}

/// Wall-heatmap colour for a radial thickness, linear RGB — the desktop's
/// ramp: red at the minimum fill section, amber to twice it, green beyond,
/// easing into blue-grey for comfortably thick metal.
pub fn wall_color(thickness_mm: f64, min_section_mm: f64) -> [f32; 3] {
    let m = min_section_mm.max(0.05);
    let t = (thickness_mm / m).max(0.0);
    let lerp3 = |a: [f32; 3], b: [f32; 3], k: f64| {
        let k = k.clamp(0.0, 1.0) as f32;
        [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
    };
    const RED: [f32; 3] = [0.93, 0.27, 0.36];
    const AMBER: [f32; 3] = [0.95, 0.76, 0.24];
    const GREEN: [f32; 3] = [0.32, 0.78, 0.45];
    const THICK: [f32; 3] = [0.36, 0.55, 0.72];
    if t <= 1.0 {
        RED
    } else if t <= 2.0 {
        lerp3(RED, AMBER, t - 1.0)
    } else if t <= 3.5 {
        lerp3(AMBER, GREEN, (t - 2.0) / 1.5)
    } else {
        lerp3(GREEN, THICK, (t - 3.5) / 2.5)
    }
}

/// Bore and inward faces sit out of the heatmap in a neutral grey.
pub const WALL_NEUTRAL: [f32; 3] = [0.42, 0.42, 0.45];

/// Queue the mesh draw as an egui paint callback covering `rect`.
///
/// Multiple callbacks coexist safely: `egui_glow::Painter` dispatches per primitive and calls
/// `prepare_painting` to restore its own state after each one.
#[allow(clippy::too_many_arguments)]
pub fn paint_callback(
    ui: &egui::Ui,
    rect: egui::Rect,
    renderer: std::sync::Arc<std::sync::Mutex<GpuMeshRenderer>>,
    mvp: [f32; 16],
    normal_matrix: [f32; 9],
    shade: ShadeMode,
    base_color: [f32; 3],
    roughness: f32,
    wireframe: bool,
    wire_color: [f32; 3],
    clip_plane: [f32; 4],
    focus: [f32; 4],
) {
    let cb = egui_glow::CallbackFn::new(move |info, painter| {
        if let Ok(mut r) = renderer.lock() {
            r.paint(
                painter.gl(),
                info,
                &mvp,
                &normal_matrix,
                shade.code(),
                base_color,
                roughness,
                wireframe,
                wire_color,
                clip_plane,
                focus,
            );
        }
    });
    ui.painter().add(egui::PaintCallback { rect, callback: std::sync::Arc::new(cb) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shade_modes_have_distinct_codes() {
        let codes: Vec<i32> = ShadeMode::ALL.iter().map(|m| m.code()).collect();
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
        assert_eq!(data.len(), 3 * FLOATS_PER_VERTEX);
        // Position of the second vertex.
        assert_eq!(data[FLOATS_PER_VERTEX], 1.0);
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
        assert_eq!(focus.len() * FLOATS_PER_VERTEX, staged.len(), "the face with a bad vertex is skipped in both");
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
}
