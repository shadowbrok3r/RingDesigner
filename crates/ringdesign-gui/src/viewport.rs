//! GPU mesh renderer and the 3D viewport.
//!
//! The mesh is uploaded once per rebuild as non-indexed triangles carrying
//! position, smooth normal, and the draft class colour, then drawn with a
//! single `glDrawArrays` per frame.

use std::sync::Arc;

use egui_glow::glow;
use glow::HasContext;

use ringdesign_core::castability::{CastReport, FaceClass};
use ringdesign_core::field::Uv;
use ringdesign_core::mesh::{Mesh, Vec3};

use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::theme;

/// Floats per vertex: position(3), normal(3), draft colour(3), wall colour(3).
const FLOATS_PER_VERTEX: usize = 12;

const VERTEX_SHADER: &str = r#"#version 330 core

layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_color;
layout(location = 3) in vec3 a_color2;

uniform mat4 u_mvp;
uniform mat3 u_normal_matrix;

out vec3 v_normal;
out vec3 v_color;
out vec3 v_color2;

void main() {
    gl_Position = u_mvp * vec4(a_position, 1.0);
    v_normal = u_normal_matrix * a_normal;
    v_color = a_color;
    v_color2 = a_color2;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core

in vec3 v_normal;
in vec3 v_color;
in vec3 v_color2;

uniform int u_mode;
uniform vec3 u_light_dir;
uniform vec3 u_base_color;
uniform float u_ambient;

out vec4 frag_color;

const vec3 FILL_DIR = vec3(-0.52, -0.38, 0.42);
const vec3 HIGHLIGHT = vec3(1.0, 0.96, 0.88);

void main() {
    vec3 n = normalize(v_normal);
    vec3 eye = vec3(0.0, 0.0, 1.0);
    vec3 l = normalize(u_light_dir);
    vec3 color;

    if (u_mode == 3) {
        float lambert = max(dot(n, l), 0.0);
        color = v_color2 * (0.74 + 0.26 * lambert);
    } else if (u_mode == 2) {
        color = n * 0.5 + 0.5;
    } else if (u_mode == 1) {
        float lambert = max(dot(n, l), 0.0);
        color = v_color * (0.74 + 0.26 * lambert);
    } else {
        float key = pow(dot(n, l) * 0.5 + 0.5, 1.7);
        float fill = max(dot(n, normalize(FILL_DIR)), 0.0) * 0.22;
        vec3 h = normalize(l + eye);
        float spec = pow(max(dot(n, h), 0.0), 58.0) * 0.85;
        float rim = pow(1.0 - max(dot(n, eye), 0.0), 3.5) * 0.30;
        color = u_base_color * (u_ambient + (1.0 - u_ambient) * key + fill + rim)
              + HIGHLIGHT * spec;
    }

    frag_color = vec4(color, 1.0);
}
"#;

const WIREFRAME_FRAGMENT_SHADER: &str = r#"#version 330 core

uniform vec3 u_wire_color;

out vec4 frag_color;

void main() {
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
}

pub struct GpuMeshRenderer {
    resources: Option<GpuResources>,
    vertex_count: i32,
    pending: Option<Vec<f32>>,
    gem_count: i32,
    gem_pending: Option<Vec<f32>>,
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
            depth_checked: false,
        }
    }
}

impl GpuMeshRenderer {
    /// Flatten the mesh into an interleaved vertex buffer awaiting upload.
    ///
    /// `wall` is `(inner_radius_mm, min_section_mm)` for the wall-thickness
    /// heatmap colours, baked alongside the draft-class colours so switching
    /// shade modes never re-uploads.
    pub fn prepare_upload(&mut self, mesh: &Mesh, cast: Option<&CastReport>, wall: (f64, f64)) {
        let (inner_r, min_section) = wall;
        let mut data: Vec<f32> =
            Vec::with_capacity(mesh.faces.len() * 3 * FLOATS_PER_VERTEX);

        'faces: for (i, face) in mesh.faces.iter().enumerate() {
            let rgb = match cast {
                Some(c) => c.classes.get(i).map_or([1.0; 3], |k| k.rgb()),
                None => [1.0; 3],
            };

            let mut tri = [[0.0f32; FLOATS_PER_VERTEX]; 3];
            for (k, &vi) in face.iter().enumerate() {
                let Some(p) = mesh.vertices.get(vi as usize).filter(|p| p.is_finite()) else {
                    continue 'faces;
                };
                let n = match mesh.normals.get(vi as usize) {
                    Some(n) if n.is_finite() => *n,
                    _ => Vec3(0.0, 0.0, 1.0),
                };
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

    /// Queue the stone-preview triangles built by [`crate::gems`]. An empty
    /// buffer clears them.
    pub fn prepare_gems(&mut self, verts: Vec<f32>) {
        self.gem_pending = Some(verts);
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
        light_dir: [f32; 3],
        ambient: f32,
        wireframe: bool,
        wire_color: [f32; 3],
        show_gems: bool,
    ) {
        unsafe { self.ensure_resources(gl) };
        let Some(res) = self.resources else { return };

        if let Some(verts) = self.pending.take() {
            self.vertex_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    as_u8_slice(&verts),
                    glow::STATIC_DRAW,
                );
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
        }
        if let Some(verts) = self.gem_pending.take() {
            self.gem_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(res.gem_vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    as_u8_slice(&verts),
                    glow::STATIC_DRAW,
                );
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
            gl.scissor(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);

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

            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.draw_arrays(glow::TRIANGLES, 0, self.vertex_count);

            if wireframe {
                gl.use_program(Some(res.wire_program));
                let loc = gl.get_uniform_location(res.wire_program, "u_mvp");
                gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, mvp);
                let loc = gl.get_uniform_location(res.wire_program, "u_wire_color");
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

            // Stones ride on top: same program in the metal-shaded mode with
            // their own tint, flat facet normals doing the sparkle. Preview
            // only — they are not in the mesh and never export.
            if show_gems && self.gem_count > 0 {
                gl.use_program(Some(res.program));
                let loc = gl.get_uniform_location(res.program, "u_mode");
                gl.uniform_1_i32(loc.as_ref(), 0);
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

        let program = unsafe { compile_program(gl, VERTEX_SHADER, FRAGMENT_SHADER) };
        let wire_program =
            unsafe { compile_program(gl, VERTEX_SHADER, WIREFRAME_FRAGMENT_SHADER) };
        let vao = unsafe { gl.create_vertex_array() }.expect("create VAO");
        let vbo = unsafe { gl.create_buffer() }.expect("create VBO");
        let gem_vao = unsafe { gl.create_vertex_array() }.expect("create gem VAO");
        let gem_vbo = unsafe { gl.create_buffer() }.expect("create gem VBO");

        unsafe {
            for (vao, vbo) in [(vao, vbo), (gem_vao, gem_vbo)] {
                gl.bind_vertex_array(Some(vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

                let f = std::mem::size_of::<f32>() as i32;
                let stride = FLOATS_PER_VERTEX as i32 * f;
                for (loc, offset) in [(0, 0), (1, 3 * f), (2, 6 * f), (3, 9 * f)] {
                    gl.enable_vertex_attrib_array(loc);
                    gl.vertex_attrib_pointer_f32(loc, 3, glow::FLOAT, false, stride, offset);
                }
            }

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }

        self.resources =
            Some(GpuResources { program, wire_program, vao, vbo, gem_vao, gem_vbo });
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
            }
        }
        self.vertex_count = 0;
        self.pending = None;
        self.gem_count = 0;
        self.gem_pending = None;
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
            panic!("{what} shader error: {}", unsafe { gl.get_shader_info_log(shader) });
        }
        unsafe { gl.attach_shader(program, shader) };
        shaders.push(shader);
    }

    unsafe { gl.link_program(program) };
    if !unsafe { gl.get_program_link_status(program) } {
        panic!("program link error: {}", unsafe { gl.get_program_info_log(program) });
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
    unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            std::mem::size_of_val(data),
        )
    }
}

// --- Metal finishes and lighting -------------------------------------------

/// Display color per alloy family. Rendering only; density stays in core.
pub struct Finish {
    pub name: &'static str,
    pub rgb: [f32; 3],
}

pub const FINISHES: &[Finish] = &[
    Finish { name: "Yellow gold", rgb: [0.86, 0.70, 0.42] },
    Finish { name: "Rose gold", rgb: [0.84, 0.60, 0.49] },
    Finish { name: "Silver", rgb: [0.79, 0.80, 0.81] },
    Finish { name: "White gold", rgb: [0.83, 0.83, 0.80] },
    Finish { name: "Platinum", rgb: [0.75, 0.76, 0.78] },
    Finish { name: "Bronze", rgb: [0.72, 0.53, 0.35] },
    Finish { name: "Brass", rgb: [0.80, 0.65, 0.36] },
];

/// A key-light direction with an ambient floor.
pub struct LightRig {
    pub name: &'static str,
    pub dir: [f32; 3],
    pub ambient: f32,
}

pub const LIGHT_RIGS: &[LightRig] = &[
    LightRig { name: "Studio", dir: [-0.38, 0.46, 0.80], ambient: 0.20 },
    LightRig { name: "Window", dir: [0.62, 0.25, 0.74], ambient: 0.28 },
    LightRig { name: "Low sun", dir: [-0.75, -0.18, 0.64], ambient: 0.12 },
];

// --- Shading modes ---------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShadeMode {
    Metal,
    Draft,
    Wall,
    Normals,
}

impl ShadeMode {
    pub const ALL: &'static [ShadeMode] =
        &[ShadeMode::Metal, ShadeMode::Draft, ShadeMode::Wall, ShadeMode::Normals];

    pub fn label(self) -> &'static str {
        match self {
            ShadeMode::Metal => "Polished metal",
            ShadeMode::Draft => "Draft check",
            ShadeMode::Wall => "Wall thickness",
            ShadeMode::Normals => "Normals",
        }
    }

    fn gl_mode(self) -> i32 {
        match self {
            ShadeMode::Metal => 0,
            ShadeMode::Draft => 1,
            ShadeMode::Wall => 3,
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

// --- Viewport --------------------------------------------------------------

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    let (rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let shift = ui.input(|i| i.modifiers.shift);
    let scroll = if response.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
    {
        let Some(cam) = app.panes.get_mut(pane).map(|p| &mut p.camera) else { return };
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            if shift {
                cam.pan_by(delta, rect.height());
            } else {
                cam.orbit(delta);
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            cam.pan_by(response.drag_delta(), rect.height());
        }
        if scroll != 0.0 {
            cam.zoom_by(scroll);
        }
    }
    let camera = app.panes[pane].camera;
    let shade = app.panes[pane].shade;
    let proj = camera.projector(rect);

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            probe_click(app, camera, rect, pos, ui.input(|i| i.modifiers.shift));
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
        let base_color = FINISHES[app.finish.min(FINISHES.len() - 1)].rgb;
        let rig = &LIGHT_RIGS[app.light.min(LIGHT_RIGS.len() - 1)];
        let (light_dir, ambient) = (rig.dir, rig.ambient);
        let wireframe = app.show_wireframe;
        let wire_color = rgb_of(theme::TEXT_DIM);
        let show_gems = app.show_gems;
        let renderer = app.renderer.clone();

        let callback = egui_glow::CallbackFn::new(move |info, glow_painter| {
            if let Ok(mut r) = renderer.lock() {
                r.paint(
                    glow_painter.gl(),
                    info,
                    &mvp,
                    &normal_matrix,
                    mode,
                    base_color,
                    light_dir,
                    ambient,
                    wireframe,
                    wire_color,
                    show_gems,
                );
            }
        });
        painter.add(egui::PaintCallback { rect, callback: Arc::new(callback) });
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Building…",
            egui::FontId::proportional(15.0),
            theme::TEXT_DIM,
        );
    }

    if app.show_grid {
        draw_axes(&painter, &proj, rect);
    }

    draw_legend(app, shade, &painter, rect);
    draw_probe(app, &painter, &proj, rect);

    painter.text(
        rect.right_bottom() - egui::vec2(12.0, 9.0),
        egui::Align2::RIGHT_BOTTOM,
        "Drag to orbit • Shift-drag to pan • Scroll to zoom",
        egui::FontId::proportional(11.0),
        theme::TEXT_DIM,
    );
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
fn draw_legend(
    app: &RingDesignerApp,
    shade: ShadeMode,
    painter: &egui::Painter,
    rect: egui::Rect,
) {
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
            rows.push((Some(swatch(m * 0.5)), format!("under {m:.1} mm — will not fill"), theme::TEXT));
            rows.push((Some(swatch(m * 1.5)), format!("{m:.1}–{:.1} mm — thin", m * 2.0), theme::TEXT));
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
            let Some(build) = app.build.as_ref() else { return };
            let r = &build.report;
            rows.push((None, app.design.size.display(), theme::TEXT));
            rows.push((
                None,
                format!("{:.2} mm outside dia • {:.2} mm wide", r.outer_diameter_mm, r.band_width_mm),
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
    let text_x = if rows.iter().any(|(c, _, _)| c.is_some()) { swatch + 7.0 } else { 0.0 };
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

/// Nearest triangle of the built mesh under the ray, by walking every face —
/// on a click, not a hover, so 110k Möller-Trumbore tests are a millisecond
/// well spent and no BVH earns its keep.
fn raycast(mesh: &Mesh, origin: [f32; 3], dir: [f32; 3]) -> Option<(usize, [f32; 3], f32)> {
    let o = [origin[0] as f64, origin[1] as f64, origin[2] as f64];
    let d = [dir[0] as f64, dir[1] as f64, dir[2] as f64];
    let mut best: Option<(usize, f64)> = None;
    for (fi, f) in mesh.faces.iter().enumerate() {
        let Some((a, b, c)) = mesh.triangle(f) else { continue };
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let p = [
            d[1] * e2[2] - d[2] * e2[1],
            d[2] * e2[0] - d[0] * e2[2],
            d[0] * e2[1] - d[1] * e2[0],
        ];
        let det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
        if det.abs() < 1e-12 {
            continue;
        }
        let inv = 1.0 / det;
        let t_vec = [o[0] - a[0], o[1] - a[1], o[2] - a[2]];
        let u = (t_vec[0] * p[0] + t_vec[1] * p[1] + t_vec[2] * p[2]) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = [
            t_vec[1] * e1[2] - t_vec[2] * e1[1],
            t_vec[2] * e1[0] - t_vec[0] * e1[2],
            t_vec[0] * e1[1] - t_vec[1] * e1[0],
        ];
        let v = (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) * inv;
        if t > 1e-6 && best.map_or(true, |(_, bt)| t < bt) {
            best = Some((fi, t));
        }
    }
    best.map(|(fi, t)| {
        (
            fi,
            [
                (o[0] + d[0] * t) as f32,
                (o[1] + d[1] * t) as f32,
                (o[2] + d[2] * t) as f32,
            ],
            t as f32,
        )
    })
}

fn probe_click(
    app: &mut RingDesignerApp,
    camera: crate::camera::OrbitCamera,
    rect: egui::Rect,
    pos: egui::Pos2,
    shift: bool,
) {
    let Some(build) = app.build.clone() else { return };
    let (origin, dir) = camera.ray(rect, pos);
    let Some((fi, world, _)) = raycast(&build.mesh, origin, dir) else {
        if !shift {
            app.probe = None;
        }
        return;
    };

    if shift {
        if app.pins.len() >= 2 {
            app.pins.clear();
        }
        app.pins.push(world);
        if app.pins.len() == 2 {
            let (a, b) = (app.pins[0], app.pins[1]);
            let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2))
                .sqrt();
            app.set_status(format!("Pin to pin: {d:.2} mm"));
        }
        return;
    }

    // Where on the band the hit is, in the field's own coordinates.
    let theta = (world[1] as f64).atan2(world[0] as f64).to_degrees().rem_euclid(360.0);
    let r = (world[0] as f64).hypot(world[1] as f64);
    let inner_r = app.design.inner_radius_mm();
    let ctx = app.design.field_context();
    let section = ringdesign_core::castability::section_at(&app.design, &app.lib, theta, 160);
    let surface: Vec<_> = section.points.iter().filter(|p| p.surface).collect();
    let mut v_mm = 0.0;
    if surface.len() >= 2 {
        let total: f64 = surface
            .windows(2)
            .map(|w| ((w[1].r - w[0].r).powi(2) + (w[1].z - w[0].z).powi(2)).sqrt())
            .sum();
        let mut acc = 0.0;
        let mut best_d = f64::MAX;
        let mut at = 0.0;
        for w in surface.windows(2) {
            let seg = ((w[1].r - w[0].r).powi(2) + (w[1].z - w[0].z).powi(2)).sqrt();
            acc += seg;
            let d = (w[1].r - r).powi(2) + (w[1].z - world[2] as f64).powi(2);
            if d < best_d {
                best_d = d;
                at = acc;
            }
        }
        v_mm = at / total.max(1e-9) * ctx.band_v_len_mm;
    }

    let uv = Uv { u: ctx.u_of_theta(theta), v: v_mm };
    let h = app.design.layers.height(uv, &ctx, &app.lib);
    let class = app
        .cast
        .as_ref()
        .and_then(|c| c.classes.get(fi))
        .map(|k| k.label())
        .unwrap_or("—");

    // The topmost layer with any say here becomes the selection.
    let named: Option<(usize, String)>;
    let mut found = None;
    for (i, e) in app.design.layers.layers.iter().enumerate().rev() {
        if !e.enabled {
            continue;
        }
        let m = e.window.mask(uv, &ctx) * e.opacity.max(0.0);
        if m <= 1e-4 {
            continue;
        }
        if e.layer.height(uv, &ctx, &app.lib).abs() * m > 5e-3 {
            found = Some((i, e.name.clone()));
            break;
        }
    }
    named = found;
    if let Some((i, _)) = named {
        app.selected_layer = Some(i);
    }

    let text = format!(
        "{:.0}° • v {:.2} • relief {:+.2} mm • wall {:.2} mm • {}{}",
        theta,
        v_mm,
        h,
        r - inner_r,
        class,
        named.map(|(_, n)| format!(" • {n}")).unwrap_or_default()
    );
    app.set_status(text.clone());
    app.probe = Some((world, text));
}

fn draw_probe(app: &RingDesignerApp, painter: &egui::Painter, proj: &Projector, rect: egui::Rect) {
    if let Some((world, text)) = &app.probe {
        let p = proj.at(*world);
        if rect.contains(p) {
            painter.circle_stroke(p, 5.0, egui::Stroke::new(1.6, theme::ACCENT));
            painter.circle_filled(p, 1.6, theme::ACCENT);
            let galley = painter.layout_no_wrap(
                text.clone(),
                egui::FontId::proportional(11.0),
                theme::TEXT,
            );
            let at = egui::pos2(
                (p.x + 10.0).min(rect.right() - galley.size().x - 6.0),
                (p.y - 18.0).max(rect.top() + 4.0),
            );
            let bg = egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(5.0, 3.0));
            painter.rect_filled(bg, 3.0, theme::PANEL.gamma_multiply(0.9));
            painter.galley(at, galley, theme::TEXT);
        }
    }
    for pin in &app.pins {
        let p = proj.at(*pin);
        if rect.contains(p) {
            painter.circle_stroke(p, 5.0, egui::Stroke::new(1.6, theme::WARN));
            painter.circle_filled(p, 1.6, theme::WARN);
        }
    }
    if app.pins.len() == 2 {
        let (a, b) = (proj.at(app.pins[0]), proj.at(app.pins[1]));
        painter.line_segment([a, b], egui::Stroke::new(1.2, theme::WARN));
        let d = {
            let (p, q) = (app.pins[0], app.pins[1]);
            ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
        };
        let mid = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5 - 10.0);
        painter.text(
            mid,
            egui::Align2::CENTER_BOTTOM,
            format!("{d:.2} mm"),
            egui::FontId::proportional(11.0),
            theme::WARN,
        );
    }
}
