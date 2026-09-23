//! GPU mesh renderer and the 3D viewport.
//!
//! The mesh is uploaded once per rebuild as non-indexed triangles carrying
//! position, smooth normal, and the draft class colour, then drawn with a
//! single `glDrawArrays` per frame.

use std::sync::Arc;

use egui_glow::glow;
use glow::HasContext;

use ringdesign_core::castability::{CastReport, FaceClass};
use ringdesign_core::mesh::{Mesh, Vec3};

use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::theme;
use ringdesign_workbench::viewport::{MenuAction, MenuItem, Mods, Sel};
use ringdesign_core::cad::edit::CadEdit;

/// How far from the pointer a part's vertex or edge still answers, in pixels.
pub const APERTURE_PX: f32 = 8.0;
/// The chosen entities' tint and strength: the theme's selection violet.
pub const SELECT_TINT: [f32; 4] = [0.80, 0.57, 0.85, 0.55];
/// The hovered entity's tint and strength: the hover cue's aqua.
pub const HOVER_TINT: [f32; 4] = [0.40, 0.85, 0.83, 0.55];
/// A live command's ghost: the hover aqua, and its opacity.
pub const PREVIEW_TINT: [f32; 4] = [0.40, 0.85, 0.83, 0.45];
/// A ghost painted by its draft classes, opacity.
const DRAFT_GHOST_ALPHA: f32 = 0.6;

/// Floats per vertex: position(3), normal(3), draft colour(3), wall colour(3).
const FLOATS_PER_VERTEX: usize = 12;

const VERTEX_SHADER: &str = r#"#version 330 core

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_color;
layout(location = 3) in vec3 a_color2;
layout(location = 4) in float a_focus;
layout(location = 5) in vec2 a_select;

uniform mat4 u_mvp;
uniform mat3 u_normal_matrix;

out vec3 v_normal;
out vec3 v_color;
out vec3 v_color2;
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
    v_color2 = a_color2;
    // Object-space axial share of the normal: which mould half owns the face.
    v_obj_nz = a_normal.z;
    v_world = a_position;
    // Approximate bore occlusion; assessment colours do not use it.
    v_cavity = smoothstep(0.0, 0.55, -dot(a_normal.xy, normalize(a_position.xy + vec2(0.000001))));
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core

in vec3 v_normal;
in vec3 v_color;
in vec3 v_color2;
in float v_obj_nz;
in vec3 v_world;
in float v_cavity;
in float v_focus;
in vec2 v_select;
uniform vec4 u_clip_plane;
uniform vec4 u_focus;
uniform vec4 u_select;
uniform vec4 u_hover;

uniform int u_mode;
uniform vec3 u_light_dir;
uniform vec3 u_base_color;
uniform float u_ambient;
uniform float u_alpha;

out vec4 frag_color;

// STUDIO_MATERIAL

void main() {
    if (dot(vec4(v_world, 1.0), u_clip_plane) > 0.00001) discard;
    vec3 n = normalize(v_normal);
    vec3 eye = vec3(0.0, 0.0, 1.0);
    vec3 l = normalize(u_light_dir);
    vec3 color;

    if (u_mode == 5) {
        color = studio_gem(n, v_color, l, u_ambient);
    } else if (u_mode == 4) {
        // Cope in cool blue, drag in warm sand, the parting band bright.
        float lambert = max(dot(n, l), 0.0);
        vec3 half_c = v_obj_nz > 0.0 ? vec3(0.42, 0.62, 0.82) : vec3(0.80, 0.62, 0.38);
        float band = 1.0 - smoothstep(0.035, 0.09, abs(v_obj_nz));
        color = mix(half_c, vec3(1.0, 0.92, 0.25), band) * (0.72 + 0.28 * lambert);
    } else if (u_mode == 3) {
        float lambert = max(dot(n, l), 0.0);
        color = v_color2 * (0.74 + 0.26 * lambert);
    } else if (u_mode == 2) {
        color = n * 0.5 + 0.5;
    } else if (u_mode == 1) {
        float lambert = max(dot(n, l), 0.0);
        color = v_color * (0.74 + 0.26 * lambert);
    } else {
        color = studio_metal(n, u_base_color, l, u_ambient, v_cavity);
    }

    // The chosen graph node's reach: its tint over whatever the mode drew,
    // a little of the key light kept so relief still reads under it.
    float focus = clamp(v_focus, 0.0, 1.0) * u_focus.a;
    if (focus > 0.0) {
        float lit = 0.62 + 0.38 * max(dot(n, l), 0.0);
        color = mix(color, u_focus.rgb * lit, focus);
    }
    // The selection over that, and the hover over both: two channels so a
    // chosen face and the one under the pointer crossfade instead of banding.
    float chosen = clamp(v_select.x, 0.0, 1.0) * u_select.a;
    float hovered = clamp(v_select.y, 0.0, 1.0) * u_hover.a;
    if (chosen > 0.0 || hovered > 0.0) {
        float lit = 0.62 + 0.38 * max(dot(n, l), 0.0);
        color = mix(color, u_select.rgb * lit, chosen);
        color = mix(color, u_hover.rgb * lit, hovered);
    }

    frag_color = vec4(color, u_alpha);
}
"#;

const WIREFRAME_FRAGMENT_SHADER: &str = r#"#version 330 core

uniform vec3 u_wire_color;
in vec3 v_world;
in float v_cavity;
uniform vec4 u_clip_plane;

out vec4 frag_color;

void main() {
    if (dot(vec4(v_world, 1.0), u_clip_plane) > 0.00001) discard;
    frag_color = vec4(u_wire_color, 0.55);
}
"#;

#[derive(Clone, Copy)]
struct GpuResources {
    program: glow::NativeProgram,
    wire_program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,
    gem_vao: glow::NativeVertexArray,
    gem_vbo: glow::NativeBuffer,
    ghost_vao: glow::NativeVertexArray,
    ghost_vbo: glow::NativeBuffer,
    cutter_vao: glow::NativeVertexArray,
    cutter_vbo: glow::NativeBuffer,
    /// A live command's ghost: staged once, moved by a model matrix.
    preview_vao: glow::NativeVertexArray,
    preview_vbo: glow::NativeBuffer,
    /// One float a staged vertex: how far the chosen node reaches it.
    focus_vbo: glow::NativeBuffer,
    /// Two floats a staged vertex: chosen, and under the pointer.
    select_vbo: glow::NativeBuffer,
}

pub struct GpuMeshRenderer {
    resources: Option<GpuResources>,
    vertex_count: i32,
    pending: Option<Vec<f32>>,
    gem_count: i32,
    gem_pending: Option<Vec<f32>>,
    ghost_count: i32,
    ghost_pending: Option<Vec<f32>>,
    cutter_count: i32,
    cutter_pending: Option<Vec<f32>>,
    preview_count: i32,
    preview_pending: Option<Vec<f32>>,
    /// Where the ghost stands: its column-major model matrix and the normals' 3x3; `None` hides it.
    preview_model: Option<([f32; 16], [f32; 9])>,
    /// The ghost shades in its staged draft colours rather than the hover aqua.
    preview_draft: bool,
    /// A focus channel awaiting upload; `Some(empty)` clears it.
    focus_pending: Option<Vec<f32>>,
    /// The uploaded channel covers the uploaded mesh vertex for vertex.
    focus_live: bool,
    /// The selection channel awaiting upload, two floats a staged vertex; `Some(empty)` clears it.
    select_pending: Option<Vec<f32>>,
    select_live: bool,
    depth_checked: bool,
}

// glow handles are u32 integers on native, safe to send across threads.
unsafe impl Send for GpuMeshRenderer {}
unsafe impl Sync for GpuMeshRenderer {}

impl Default for GpuMeshRenderer {
    fn default() -> Self {
        Self {
            resources: None,
            vertex_count: 0,
            pending: None,
            gem_count: 0,
            gem_pending: None,
            ghost_count: 0,
            ghost_pending: None,
            cutter_count: 0,
            cutter_pending: None,
            preview_count: 0,
            preview_pending: None,
            preview_model: None,
            preview_draft: false,
            focus_pending: None,
            focus_live: false,
            select_pending: None,
            select_live: false,
            depth_checked: false,
        }
    }
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

impl GpuMeshRenderer {
    /// Flatten the mesh into an interleaved vertex buffer awaiting upload.
    ///
    /// `wall` is `(inner_radius_mm, min_section_mm)` for the wall-thickness
    /// heatmap colours, baked alongside the draft-class colours so switching
    /// shade modes never re-uploads.
    pub fn prepare_upload(&mut self, mesh: &Mesh, cast: Option<&CastReport>, wall: (f64, f64)) {
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

        self.pending = Some(data);
    }

    /// Preserve sharp CAD corners while keeping smooth analytic faces smooth.
    pub fn prepare_cad(&mut self, mesh: &Mesh) {
        let mut display = Mesh::default();
        for face in &mesh.faces {
            let Some(normal) = mesh.face_normal(face) else {
                continue;
            };
            let first = display.vertices.len() as u32;
            for &id in face {
                let Some(p) = mesh.vertices.get(id as usize) else {
                    continue;
                };
                let smooth = mesh.normals.get(id as usize).copied().unwrap_or(Vec3(
                    normal[0] as f32,
                    normal[1] as f32,
                    normal[2] as f32,
                ));
                let dot = smooth.0 as f64 * normal[0]
                    + smooth.1 as f64 * normal[1]
                    + smooth.2 as f64 * normal[2];
                display.vertices.push(*p);
                display.normals.push(if dot > 0.94 {
                    smooth
                } else {
                    Vec3(normal[0] as f32, normal[1] as f32, normal[2] as f32)
                });
            }
            display.faces.push([first, first + 1, first + 2]);
        }
        self.prepare_upload(&display, None, (0.0, 0.0));
    }

    /// Per-vertex weights in the order [`prepare_upload`](Self::prepare_upload)
    /// emits vertices — the same faces skipped, so the two stay vertex for vertex.
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

    /// Queue a focus channel staged for the mesh on screen. Empty clears it.
    pub fn prepare_focus(&mut self, weights: Vec<f32>) {
        self.focus_pending = Some(weights);
    }

    /// The selection channel in the emitted vertex order: `(chosen, hovered)` a vertex from the
    /// `0 / 1 / 2` weights `viewport::tint` gives, the same faces skipped as [`stage_focus`].
    pub fn stage_select(mesh: &Mesh, weight: &[f32]) -> Vec<f32> {
        let mut data = Vec::with_capacity(mesh.faces.len() * 6);
        for face in &mesh.faces {
            if face.iter().any(|&vi| !mesh.vertices.get(vi as usize).is_some_and(|p| p.is_finite())) {
                continue;
            }
            for &vi in face {
                let w = weight.get(vi as usize).copied().unwrap_or(0.0);
                data.push(if w >= 0.5 && w < 1.5 { 1.0 } else { 0.0 });
                data.push(if w >= 1.5 { 1.0 } else { 0.0 });
            }
        }
        data
    }

    /// Queue a selection channel staged by [`stage_select`]. Empty clears it.
    pub fn prepare_select(&mut self, weights: Vec<f32>) {
        self.select_pending = Some(weights);
    }

    /// Queue the stone-preview triangles built by [`crate::gems`]. An empty
    /// buffer clears them.
    pub fn prepare_gems(&mut self, verts: Vec<f32>) {
        self.gem_pending = Some(verts);
    }

    /// Queue the pinned comparison mesh, in the same layout. Empty clears it.
    /// The seats' cutters, drawn through the metal; empty clears them.
    pub fn prepare_cutters(&mut self, verts: Vec<f32>) {
        self.cutter_pending = Some(verts);
    }

    pub fn prepare_ghost(&mut self, verts: Vec<f32>) {
        self.ghost_pending = Some(verts);
    }

    /// Queue a live command's ghost in its own coordinates. Empty clears it.
    pub fn prepare_preview(&mut self, verts: Vec<f32>) {
        self.preview_pending = Some(verts);
    }

    /// Where the queued ghost stands: model matrix and normal matrix, column-major; `None` hides it.
    pub fn set_preview_model(&mut self, model: Option<([f32; 16], [f32; 9])>) {
        self.preview_model = model;
    }

    /// Whether the ghost shades in its staged draft colours.
    pub fn set_preview_draft(&mut self, draft: bool) {
        self.preview_draft = draft;
    }

    /// The ghost's vertices awaiting upload, and whether it shades by draft class.
    #[cfg(test)]
    pub fn staged_preview(&self) -> (Option<&[f32]>, bool) {
        (self.preview_pending.as_deref(), self.preview_draft)
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
            let facet = Vec3(facet[0] as f32, facet[1] as f32, facet[2] as f32);
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

    /// Flatten a mesh into the interleaved layout with neutral colours — the
    /// ghost pass supplies its own tint.
    pub fn stage_plain(mesh: &ringdesign_core::Mesh) -> Vec<f32> {
        let mut data = Vec::with_capacity(mesh.faces.len() * 3 * FLOATS_PER_VERTEX);
        'faces: for face in &mesh.faces {
            let mut tri = [[0.0f32; FLOATS_PER_VERTEX]; 3];
            for (k, &vi) in face.iter().enumerate() {
                let Some(p) = mesh.vertices.get(vi as usize).filter(|p| p.is_finite()) else {
                    continue 'faces;
                };
                let n = match mesh.normals.get(vi as usize) {
                    Some(n) if n.is_finite() => *n,
                    _ => ringdesign_core::Vec3(0.0, 0.0, 1.0),
                };
                tri[k] = [p.0, p.1, p.2, n.0, n.1, n.2, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
            }
            for v in &tri {
                data.extend_from_slice(v);
            }
        }
        data
    }

    /// Draw the mesh. Called from inside the paint callback.
    fn paint(
        &mut self,
        gl: &glow::Context,
        info: egui::PaintCallbackInfo,
        mvp: &[f32; 16],
        normal_matrix: &[f32; 9],
        mode: i32,
        base_color: [f32; 3],
        roughness: f32,
        light_dir: [f32; 3],
        ambient: f32,
        wireframe: bool,
        wire_color: [f32; 3],
        show_gems: bool,
        clip_plane: [f32; 4],
        focus: [f32; 4],
        select: [f32; 4],
        hover: [f32; 4],
    ) {
        unsafe { self.ensure_resources(gl) };
        let Some(res) = self.resources else { return };

        if let Some(verts) = self.pending.take() {
            // A channel staged for the last mesh says nothing about this one.
            self.focus_live = false;
            self.select_live = false;
            self.vertex_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(weights) = self.focus_pending.take() {
            self.focus_live = !weights.is_empty() && weights.len() as i32 == self.vertex_count;
            if self.focus_live {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.focus_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&weights), glow::STATIC_DRAW);
                    gl.bind_buffer(glow::ARRAY_BUFFER, None);
                }
            }
        }
        if let Some(weights) = self.select_pending.take() {
            self.select_live = !weights.is_empty() && weights.len() as i32 == self.vertex_count * 2;
            if self.select_live {
                unsafe {
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.select_vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&weights), glow::STATIC_DRAW);
                    gl.bind_buffer(glow::ARRAY_BUFFER, None);
                }
            }
        }
        if let Some(verts) = self.cutter_pending.take() {
            self.cutter_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.cutter_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(verts) = self.ghost_pending.take() {
            self.ghost_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.ghost_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(verts) = self.gem_pending.take() {
            self.gem_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.gem_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::STATIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(verts) = self.preview_pending.take() {
            self.preview_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.preview_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_u8_slice(&verts), glow::DYNAMIC_DRAW);
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }

        if self.vertex_count == 0 {
            return;
        }

        self.warn_if_no_depth_buffer(gl);

        unsafe {
            let vp = info.viewport_in_pixels();
            gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
            let clip = info.clip_rect_in_pixels();
            gl.scissor(
                clip.left_px,
                clip.from_bottom_px,
                clip.width_px,
                clip.height_px,
            );

            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(true);
            gl.depth_func(glow::LESS);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.enable(glow::SCISSOR_TEST);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(res.program));
            gl.bind_vertex_array(Some(res.vao));

            let loc = gl.get_uniform_location(res.program, "u_mvp");
            gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, mvp);
            let loc = gl.get_uniform_location(res.program, "u_normal_matrix");
            gl.uniform_matrix_3_f32_slice(loc.as_ref(), false, normal_matrix);
            let loc = gl.get_uniform_location(res.program, "u_mode");
            gl.uniform_1_i32(loc.as_ref(), mode);
            let loc = gl.get_uniform_location(res.program, "u_light_dir");
            gl.uniform_3_f32(loc.as_ref(), light_dir[0], light_dir[1], light_dir[2]);
            let loc = gl.get_uniform_location(res.program, "u_base_color");
            gl.uniform_3_f32(loc.as_ref(), base_color[0], base_color[1], base_color[2]);
            let loc = gl.get_uniform_location(res.program, "u_ambient");
            gl.uniform_1_f32(loc.as_ref(), ambient);
            let loc = gl.get_uniform_location(res.program, "u_roughness");
            gl.uniform_1_f32(loc.as_ref(), roughness);
            let loc = gl.get_uniform_location(res.program, "u_alpha");
            gl.uniform_1_f32(loc.as_ref(), 1.0);
            let loc = gl.get_uniform_location(res.program, "u_clip_plane");
            gl.uniform_4_f32_slice(loc.as_ref(), &clip_plane);
            // Without a channel the attribute is a constant zero: no buffer
            // is read, so a stale one can never be read past its end.
            let lit = self.focus_live && focus[3] > 0.0;
            let focus_loc = gl.get_uniform_location(res.program, "u_focus");
            gl.uniform_4_f32_slice(focus_loc.as_ref(), &if lit { focus } else { [0.0; 4] });
            if lit {
                gl.enable_vertex_attrib_array(4);
            } else {
                gl.disable_vertex_attrib_array(4);
                gl.vertex_attrib_1_f32(4, 0.0);
            }
            // The selection rides its own buffer on attribute 5, gated the same way.
            let chosen = self.select_live && (select[3] > 0.0 || hover[3] > 0.0);
            let select_loc = gl.get_uniform_location(res.program, "u_select");
            let hover_loc = gl.get_uniform_location(res.program, "u_hover");
            gl.uniform_4_f32_slice(select_loc.as_ref(), &if chosen { select } else { [0.0; 4] });
            gl.uniform_4_f32_slice(hover_loc.as_ref(), &if chosen { hover } else { [0.0; 4] });
            if chosen {
                gl.enable_vertex_attrib_array(5);
            } else {
                gl.disable_vertex_attrib_array(5);
                gl.vertex_attrib_2_f32(5, 0.0, 0.0);
            }

            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.draw_arrays(glow::TRIANGLES, 0, self.vertex_count);
            gl.uniform_4_f32_slice(focus_loc.as_ref(), &[0.0; 4]);
            gl.uniform_4_f32_slice(select_loc.as_ref(), &[0.0; 4]);
            gl.uniform_4_f32_slice(hover_loc.as_ref(), &[0.0; 4]);

            // The cutters: through the metal, since a tool sits inside what it removes.
            if self.cutter_count > 0 {
                gl.use_program(Some(res.program));
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), 0);
                let loc = gl.get_uniform_location(res.program, "u_base_color");
                gl.uniform_3_f32(loc.as_ref(), 1.0, 0.34, 0.62);
                let loc = gl.get_uniform_location(res.program, "u_alpha");
                gl.uniform_1_f32(loc.as_ref(), 0.34);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.disable(glow::DEPTH_TEST);
                gl.bind_vertex_array(Some(res.cutter_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.cutter_count);
                gl.enable(glow::DEPTH_TEST);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.bind_vertex_array(Some(res.vao));
                let loc = gl.get_uniform_location(res.program, "u_alpha");
                gl.uniform_1_f32(loc.as_ref(), 1.0);
            }

            // The pinned comparison ghost: last, translucent, no depth
            // writes, so it reads as a spectre around the live metal.
            if self.ghost_count > 0 {
                gl.use_program(Some(res.program));
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), 0);
                let loc = gl.get_uniform_location(res.program, "u_base_color");
                gl.uniform_3_f32(loc.as_ref(), 0.62, 0.72, 0.84);
                let loc = gl.get_uniform_location(res.program, "u_alpha");
                gl.uniform_1_f32(loc.as_ref(), 0.28);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.bind_vertex_array(Some(res.ghost_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.ghost_count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.bind_vertex_array(Some(res.vao));
                let loc = gl.get_uniform_location(res.program, "u_alpha");
                gl.uniform_1_f32(loc.as_ref(), 1.0);
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), mode);
                let loc = gl.get_uniform_location(res.program, "u_base_color");
                gl.uniform_3_f32(loc.as_ref(), base_color[0], base_color[1], base_color[2]);
            }

            // A live command's ghost: its own buffer under a model matrix, translucent and depth-tested.
            if let (true, Some((model, normal))) = (self.preview_count > 0, self.preview_model) {
                gl.use_program(Some(res.program));
                let mvp_loc = gl.get_uniform_location(res.program, "u_mvp");
                gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &mul4(mvp, &model));
                let normal_loc = gl.get_uniform_location(res.program, "u_normal_matrix");
                gl.uniform_matrix_3_f32_slice(normal_loc.as_ref(), false, &mul3(normal_matrix, &normal));
                // Mode 1 shades the staged draft colours.
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), if self.preview_draft { 1 } else { 0 });
                let loc = gl.get_uniform_location(res.program, "u_base_color");
                gl.uniform_3_f32(loc.as_ref(), PREVIEW_TINT[0], PREVIEW_TINT[1], PREVIEW_TINT[2]);
                let alpha_loc = gl.get_uniform_location(res.program, "u_alpha");
                gl.uniform_1_f32(alpha_loc.as_ref(), if self.preview_draft { DRAFT_GHOST_ALPHA } else { PREVIEW_TINT[3] });
                let clip_loc = gl.get_uniform_location(res.program, "u_clip_plane");
                gl.uniform_4_f32_slice(clip_loc.as_ref(), &[0.0; 4]);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
                gl.bind_vertex_array(Some(res.preview_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.preview_count);
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
                gl.bind_vertex_array(Some(res.vao));
                gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, mvp);
                gl.uniform_matrix_3_f32_slice(normal_loc.as_ref(), false, normal_matrix);
                gl.uniform_1_f32(alpha_loc.as_ref(), 1.0);
                gl.uniform_4_f32_slice(clip_loc.as_ref(), &clip_plane);
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), mode);
                let loc = gl.get_uniform_location(res.program, "u_base_color");
                gl.uniform_3_f32(loc.as_ref(), base_color[0], base_color[1], base_color[2]);
            }

            if wireframe {
                gl.use_program(Some(res.wire_program));
                let loc = gl.get_uniform_location(res.wire_program, "u_mvp");
                gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, mvp);
                let loc = gl.get_uniform_location(res.wire_program, "u_wire_color");
                let clip_loc = gl.get_uniform_location(res.wire_program, "u_clip_plane");
                gl.uniform_4_f32_slice(clip_loc.as_ref(), &clip_plane);
                gl.uniform_3_f32(loc.as_ref(), wire_color[0], wire_color[1], wire_color[2]);

                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.enable(glow::POLYGON_OFFSET_LINE);
                gl.polygon_offset(-1.0, -1.0);
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
                gl.draw_arrays(glow::TRIANGLES, 0, self.vertex_count);

                gl.disable(glow::POLYGON_OFFSET_LINE);
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
                gl.disable(glow::BLEND);
            }

            // Stones ride on top: same program in the dielectric mode with
            // their own tint, flat facet normals doing the sparkle. Preview
            // only — they are not in the mesh and never export.
            if show_gems && self.gem_count > 0 {
                gl.use_program(Some(res.program));
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), 5);
                let loc = gl.get_uniform_location(res.program, "u_base_color");
                gl.uniform_3_f32(
                    loc.as_ref(),
                    crate::gems::GEM_TINT[0],
                    crate::gems::GEM_TINT[1],
                    crate::gems::GEM_TINT[2],
                );
                gl.bind_vertex_array(Some(res.gem_vao));
                gl.draw_arrays(glow::TRIANGLES, 0, self.gem_count);
            }

            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::SCISSOR_TEST);
        }
    }

    /// Depth testing is silently a no-op on a window with no depth attachment,
    /// which reads as a see-through ring rather than as an error. Checked once.
    fn warn_if_no_depth_buffer(&mut self, gl: &glow::Context) {
        if self.depth_checked {
            return;
        }
        self.depth_checked = true;
        let bits = unsafe {
            gl.get_framebuffer_attachment_parameter_i32(
                glow::FRAMEBUFFER,
                glow::DEPTH,
                glow::FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE,
            )
        };
        if bits <= 0 {
            log::warn!(
                "no depth buffer on the default framebuffer ({bits} bits): the ring will draw \
                 see-through. NativeOptions::depth_buffer must be non-zero."
            );
        } else {
            log::info!("depth buffer: {bits} bits");
        }
    }

    unsafe fn ensure_resources(&mut self, gl: &glow::Context) {
        if self.resources.is_some() {
            return;
        }

        let program = unsafe {
            compile_program(
                gl,
                VERTEX_SHADER,
                &FRAGMENT_SHADER
                    .replace("// STUDIO_MATERIAL", ringdesign_core::render::STUDIO_GLSL),
            )
        };
        let wire_program = unsafe { compile_program(gl, VERTEX_SHADER, WIREFRAME_FRAGMENT_SHADER) };
        let vao = unsafe { gl.create_vertex_array() }.expect("create VAO");
        let vbo = unsafe { gl.create_buffer() }.expect("create VBO");
        let gem_vao = unsafe { gl.create_vertex_array() }.expect("create gem VAO");
        let gem_vbo = unsafe { gl.create_buffer() }.expect("create gem VBO");
        let ghost_vao = unsafe { gl.create_vertex_array() }.expect("create ghost VAO");
        let ghost_vbo = unsafe { gl.create_buffer() }.expect("create ghost VBO");
        let cutter_vao = unsafe { gl.create_vertex_array() }.expect("create cutter VAO");
        let cutter_vbo = unsafe { gl.create_buffer() }.expect("create cutter VBO");
        let preview_vao = unsafe { gl.create_vertex_array() }.expect("create preview VAO");
        let preview_vbo = unsafe { gl.create_buffer() }.expect("create preview VBO");
        let focus_vbo = unsafe { gl.create_buffer() }.expect("create focus VBO");
        let select_vbo = unsafe { gl.create_buffer() }.expect("create select VBO");

        unsafe {
            for (vao, vbo) in [(vao, vbo), (gem_vao, gem_vbo), (ghost_vao, ghost_vbo), (cutter_vao, cutter_vbo), (preview_vao, preview_vbo)] {
                gl.bind_vertex_array(Some(vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

                let f = std::mem::size_of::<f32>() as i32;
                let stride = FLOATS_PER_VERTEX as i32 * f;
                for (loc, offset) in [(0, 0), (1, 3 * f), (2, 6 * f), (3, 9 * f)] {
                    gl.enable_vertex_attrib_array(loc);
                    gl.vertex_attrib_pointer_f32(loc, 3, glow::FLOAT, false, stride, offset);
                }
            }

            // The focus channel rides the ring's VAO from a buffer of its
            // own, so a highlight is one small upload and never a re-stage.
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(focus_vbo));
            gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, std::mem::size_of::<f32>() as i32, 0);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(select_vbo));
            gl.vertex_attrib_pointer_f32(5, 2, glow::FLOAT, false, 2 * std::mem::size_of::<f32>() as i32, 0);

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }

        self.resources = Some(GpuResources {
            program,
            wire_program,
            vao,
            vbo,
            gem_vao,
            gem_vbo,
            ghost_vao,
            ghost_vbo,
            cutter_vao,
            cutter_vbo,
            preview_vao,
            preview_vbo,
            focus_vbo,
            select_vbo,
        });
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        if let Some(res) = self.resources.take() {
            unsafe {
                gl.delete_program(res.program);
                gl.delete_program(res.wire_program);
                gl.delete_vertex_array(res.vao);
                gl.delete_buffer(res.vbo);
                gl.delete_vertex_array(res.gem_vao);
                gl.delete_buffer(res.gem_vbo);
                gl.delete_vertex_array(res.ghost_vao);
                gl.delete_buffer(res.ghost_vbo);
                gl.delete_vertex_array(res.cutter_vao);
                gl.delete_buffer(res.cutter_vbo);
                gl.delete_vertex_array(res.preview_vao);
                gl.delete_buffer(res.preview_vbo);
                gl.delete_buffer(res.focus_vbo);
                gl.delete_buffer(res.select_vbo);
            }
        }
        self.preview_count = 0;
        self.preview_pending = None;
        self.preview_model = None;
        self.focus_pending = None;
        self.focus_live = false;
        self.select_pending = None;
        self.select_live = false;
        self.vertex_count = 0;
        self.pending = None;
        self.gem_count = 0;
        self.gem_pending = None;
        self.ghost_count = 0;
        self.ghost_pending = None;
        self.cutter_count = 0;
        self.cutter_pending = None;
    }
}

unsafe fn compile_program(
    gl: &glow::Context,
    vert_src: &str,
    frag_src: &str,
) -> glow::NativeProgram {
    let program = unsafe { gl.create_program() }.expect("create program");

    let mut shaders = Vec::with_capacity(2);
    for (kind, src, what) in [
        (glow::VERTEX_SHADER, vert_src, "vertex"),
        (glow::FRAGMENT_SHADER, frag_src, "fragment"),
    ] {
        let shader = unsafe { gl.create_shader(kind) }.expect("create shader");
        unsafe {
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
        }
        if !unsafe { gl.get_shader_compile_status(shader) } {
            panic!("{what} shader error: {}", unsafe {
                gl.get_shader_info_log(shader)
            });
        }
        unsafe { gl.attach_shader(program, shader) };
        shaders.push(shader);
    }

    unsafe { gl.link_program(program) };
    if !unsafe { gl.get_program_link_status(program) } {
        panic!("program link error: {}", unsafe {
            gl.get_program_info_log(program)
        });
    }

    for shader in shaders {
        unsafe {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
    }

    program
}

fn as_u8_slice<T: Copy>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
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

    fn gl_mode(self) -> i32 {
        match self {
            ShadeMode::Metal => 0,
            ShadeMode::Draft => 1,
            ShadeMode::Wall => 3,
            ShadeMode::Halves => 4,
            ShadeMode::Normals => 2,
        }
    }
}

/// Wall-heatmap colour for a radial thickness, linear RGB.
///
/// Red at the minimum fill section and under, amber to twice it, then easing
/// through green into a quiet blue-grey for comfortably thick metal.
pub fn wall_color(thickness_mm: f64, min_section_mm: f64) -> [f32; 3] {
    let m = min_section_mm.max(0.05);
    let t = (thickness_mm / m).max(0.0);
    let lerp3 = |a: [f32; 3], b: [f32; 3], k: f64| {
        let k = k.clamp(0.0, 1.0) as f32;
        [
            a[0] + (b[0] - a[0]) * k,
            a[1] + (b[1] - a[1]) * k,
            a[2] + (b[2] - a[2]) * k,
        ]
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
                .default_pos(ui.max_rect().right_top() - egui::vec2(315.0, -180.0))
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
            // A view from the cube eases in, as a chosen node's does.
            let from = camera.pose();
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
    if response.secondary_clicked() && !took.secondary {
        // What the menu is about: the best pick under the pointer, with the band's own raycast as
        // the fallback before the first scene is built.
        let camera = app.panes[pane].camera;
        let under = response.interact_pointer_pos().and_then(|pos| {
            let ray = |p: egui::Pos2| camera.ray(rect, p);
            match (&app.pick_scene, &app.build) {
                (Some(scene), _) => ringdesign_workbench::hover::pick_at(scene, pos, &ray, APERTURE_PX, app.selection.filter).into_iter().next(),
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
            show_menu(app, ui, pane);
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
            let mods = ui.input(|i| ringdesign_workbench::viewport::Mods { shift: i.modifiers.shift, ctrl: i.modifiers.command, alt: i.modifiers.alt });
            select_click(app, camera, rect, pos, mods);
        }
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::VIEWPORT_BG);

    if app.show_grid {
        draw_grid(app, &painter, &proj, camera.half_extent());
    }

    if app.build.is_some() {
        let (mvp, normal_matrix) = camera.matrices(rect);
        let mode = shade.gl_mode();
        let base_color =
            ringdesign_core::render::METAL_FINISHES[app.finish.min(FINISHES.len() - 1)].1;
        let roughness = ringdesign_core::render::POLISHES[app.polish.min(2)].1;
        let rig = &LIGHT_RIGS[app.light.min(LIGHT_RIGS.len() - 1)];
        let (light_dir, ambient) = (rig.dir, rig.ambient);
        let wireframe = app.show_wireframe;
        let wire_color = rgb_of(theme::TEXT_DIM);
        let show_gems = app.show_gems;
        let renderer = if mould_active {
            app.mould_renderer.clone()
        } else {
            app.renderer.clone()
        };
        let clip_plane = if active { app.visual.clip() } else { [0.0; 4] };
        let focus = if mould_active { [0.0; 4] } else { app.node_focus.tint };
        let (select, hover) = if mould_active { ([0.0; 4], [0.0; 4]) } else { (SELECT_TINT, HOVER_TINT) };

        let callback = egui_glow::CallbackFn::new(move |info, glow_painter| {
            if let Ok(mut r) = renderer.lock() {
                r.paint(
                    glow_painter.gl(),
                    info,
                    &mvp,
                    &normal_matrix,
                    mode,
                    base_color,
                    roughness,
                    light_dir,
                    ambient,
                    wireframe,
                    wire_color,
                    show_gems,
                    clip_plane,
                    focus,
                    select,
                    hover,
                );
            }
        });
        painter.add(egui::PaintCallback {
            rect,
            callback: Arc::new(callback),
        });
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
    if active && app.visual.tool == Tool::Select && !took.live && !took.boxing && app.command.gizmo_hot().is_none() {
        app.hovered_node = None;
        // The scene answers first; a band under the pointer falls through to the layer caption and
        // the node highlight it always had.
        let picks = app.pick_scene.clone().and_then(|scene| ringdesign_workbench::hover::picks(ui, rect, &response, &scene, |p| camera.ray(rect, p), APERTURE_PX, app.selection.filter));
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
    }
    // The selection's channel, restaged when it or the mesh changes; the node focus keeps its own.
    if let Some(build) = &app.build {
        let key = Arc::as_ptr(build) as usize;
        if app.selection.needs_stage(key) {
            let weights = ringdesign_workbench::viewport::tint(&app.selection, build);
            let staged = if weights.is_empty() { Vec::new() } else { GpuMeshRenderer::stage_select(&build.mesh, &weights) };
            if let Ok(mut r) = app.renderer.lock() {
                r.prepare_select(staged);
            }
            ui.ctx().request_repaint();
        }
    }
    draw_selection(app, &painter, &proj, rect);
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
    let wire = display.wire;
    let base_color = ringdesign_core::render::METAL_FINISHES[display.finish.min(FINISHES.len()-1)].1;
    let roughness = ringdesign_core::render::POLISHES[display.polish.min(2)].1;
    let rig=&LIGHT_RIGS[display.light.min(LIGHT_RIGS.len()-1)];
    let (light_dir,ambient,show_gems)=(rig.dir,rig.ambient,display.show_gems);
    let callback = egui_glow::CallbackFn::new(move |info,glow_painter| {
        if let Ok(mut r) = renderer.lock() {
            r.paint(glow_painter.gl(),info,&mvp,&normal,0,base_color,roughness,
                light_dir,ambient,wire,[0.3,0.3,0.3],show_gems,[0.;4],[0.;4],[0.;4],[0.;4]);
        }
    });
    painter.add(egui::PaintCallback {rect,callback:Arc::new(callback)});
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
    let picks = ringdesign_workbench::hover::pick_at(&scene, pos, &ray, APERTURE_PX, app.selection.filter);
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
    let items = ringdesign_workbench::viewport::context_items(&app.selection, under.as_ref(), &app.design);
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
        MenuAction::FitView => {
            let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
            app.panes[pane].camera.fit(bounds);
        }
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

/// The chosen and hovered edges, vertices and band points, drawn over the ring.
fn draw_selection(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector, rect: egui::Rect) {
    let Some(build) = app.build.as_deref() else { return };
    let evaluated = build.parts.evaluated.as_ref();
    let mark = |sel: &Sel, color: egui::Color32, width: f32| match sel {
        Sel::Edge { feature, edge } => {
            if let Some(poly) = evaluated.and_then(|e| e.components.iter().find(|c| c.id == *feature)).and_then(|c| c.edges.get(*edge as usize)) {
                let points: Vec<egui::Pos2> = poly.iter().map(|p| proj.at([p[0] as f32, p[1] as f32, p[2] as f32])).collect();
                painter.add(egui::Shape::line(points, egui::Stroke::new(width, color)));
            }
        }
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
