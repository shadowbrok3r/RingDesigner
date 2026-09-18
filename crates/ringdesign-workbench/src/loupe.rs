//! A GPU-only crop of the finished viewport, including its actual egui overlay.
//! No readback, second mesh build, or approximation of the placement is needed.
use egui::{Color32, Pos2, Rect, Stroke};
use egui_glow::glow::{self, HasContext};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Gpu {
    resources: Option<Resources>,
    error: Option<String>,
}
struct Resources {
    program: glow::Program,
    vao: glow::VertexArray,
    texture: glow::Texture,
    framebuffer: glow::Framebuffer,
    size: [i32; 2],
}

/// `reach` is how far the placement being made extends from the contact in
/// points, so the crop holds the whole stamp instead of clipping its edges.
pub fn show(ui: &egui::Ui, viewport: Rect, contact: Pos2, reach: f32, obstacles: &[Rect]) -> Rect {
    let rect = crate::navigation::loupe_rect(contact, viewport, obstacles, reach);
    let zoom = crate::navigation::loupe_zoom(rect, reach);
    let source = Rect::from_center_size(contact, rect.size() / zoom);
    let id = ui.id().with("placement-loupe");
    let gpu = ui.data_mut(|d| d.get_temp_mut_or_default::<Arc<Mutex<Gpu>>>(id).clone());
    let error = gpu.lock().ok().and_then(|g| g.error.clone());
    // Tooltip order draws after the viewport, its overlay, and floating tools.
    // The callback is an egui batching barrier: the copied pixels are this frame.
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(egui::Order::Tooltip, id));
    let callback = egui_glow::CallbackFn::new(move |info, painter| {
        if let Ok(mut gpu) = gpu.lock() {
            if gpu.error.is_some() {
                return;
            }
            // SAFETY: egui invokes this on the GL thread with its current context.
            let result = unsafe { gpu.paint(painter.gl(), info, source, viewport) };
            if let Err(error) = result {
                gpu.error = Some(error);
            }
        }
    });
    painter.add(egui::PaintCallback {
        rect,
        callback: Arc::new(callback),
    });
    let aqua = Color32::from_rgb(43, 226, 214);
    painter.circle_stroke(
        rect.center(),
        rect.width() * 0.5 - 1.0,
        Stroke::new(2.0, aqua),
    );
    let c = rect.center();
    for (a, b) in [
        ([-9.0, 0.0], [-3.0, 0.0]),
        ([3.0, 0.0], [9.0, 0.0]),
        ([0.0, -9.0], [0.0, -3.0]),
        ([0.0, 3.0], [0.0, 9.0]),
    ] {
        let points = [c + egui::vec2(a[0], a[1]), c + egui::vec2(b[0], b[1])];
        painter.line_segment(points, Stroke::new(3.0, Color32::BLACK));
        painter.line_segment(points, Stroke::new(1.0, Color32::WHITE));
    }
    let label = Rect::from_center_size(
        rect.center_bottom() - egui::vec2(0.0, 10.0),
        egui::vec2(38.0, 16.0),
    );
    painter.rect_filled(label, 5.0, Color32::from_rgb(24, 22, 32));
    painter.text(
        label.center(),
        egui::Align2::CENTER_CENTER,
        format!("{zoom:.1}×"),
        egui::FontId::proportional(11.0),
        aqua,
    );
    if error.is_some() {
        painter.text(
            c,
            egui::Align2::CENTER_CENTER,
            "Loupe unavailable",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );
    }
    rect
}

impl Gpu {
    unsafe fn paint(
        &mut self,
        gl: &glow::Context,
        info: egui::PaintCallbackInfo,
        source: Rect,
        bounds: Rect,
    ) -> Result<(), String> {
        unsafe {
            if self.resources.is_none() {
                self.resources = Some(Resources::new(gl)?);
            }
            let r = self.resources.as_mut().unwrap();
            let scale = info.pixels_per_point;
            let height = info.screen_size_px[1] as i32;
            let clipped = source.intersect(bounds).intersect(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(info.screen_size_px[0] as f32 / scale, height as f32 / scale),
            ));
            let x = (clipped.left() * scale).ceil() as i32;
            let y = (clipped.top() * scale).ceil() as i32;
            let w = ((clipped.right() * scale).floor() as i32 - x).max(1);
            let h = ((clipped.bottom() * scale).floor() as i32 - y).max(1);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(r.texture));
            if r.size != [w, h] {
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    w,
                    h,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                r.size = [w, h];
            }
            let draw = gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING);
            let read = gl.get_parameter_framebuffer(glow::READ_FRAMEBUFFER_BINDING);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, draw);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(r.framebuffer));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(r.texture),
                0,
            );
            if gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw);
                gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read);
                return Err("Magnifier framebuffer is incomplete".into());
            }
            gl.disable(glow::SCISSOR_TEST);
            // Same-sized blit also resolves multisampled desktop framebuffers.
            gl.blit_framebuffer(
                x,
                height - y - h,
                x + w,
                height - y,
                0,
                0,
                w,
                h,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read);
            gl.enable(glow::SCISSOR_TEST);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(r.program));
            gl.bind_vertex_array(Some(r.vao));
            gl.uniform_1_i32(gl.get_uniform_location(r.program, "image").as_ref(), 0);
            // The requested crop stays centred on the contact even at mesh-pane edges.
            gl.uniform_4_f32(
                gl.get_uniform_location(r.program, "crop").as_ref(),
                (source.left() * scale - x as f32) / w as f32,
                (y as f32 + h as f32 - source.bottom() * scale) / h as f32,
                source.width() * scale / w as f32,
                source.height() * scale / h as f32,
            );
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
            // egui restores program, viewport, blend and buffer bindings after callbacks.
            Ok(())
        }
    }
}
impl Resources {
    unsafe fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let header = if gl.get_parameter_string(glow::VERSION).contains("OpenGL ES") {
                "#version 300 es\nprecision highp float;\n"
            } else {
                "#version 330 core\n"
            };
            let program = gl.create_program()?;
            for (kind, body) in [
                (
                    glow::VERTEX_SHADER,
                    "out vec2 uv; void main(){ vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2); uv=p; gl_Position=vec4(p*2.0-1.0,0.0,1.0); }",
                ),
                (
                    glow::FRAGMENT_SHADER,
                    "in vec2 uv; uniform sampler2D image; uniform vec4 crop; out vec4 color; void main(){ float r=length(uv*2.0-1.0); if(r>1.0) discard; vec2 t=crop.xy+uv*crop.zw; vec3 c=any(lessThan(t,vec2(0.0)))||any(greaterThan(t,vec2(1.0)))?vec3(0.07):texture(image,t).rgb; float a=1.0-smoothstep(1.0-fwidth(r),1.0,r); color=vec4(c*a,a); }",
                ),
            ] {
                let shader = match gl.create_shader(kind) {
                    Ok(s) => s,
                    Err(e) => {
                        gl.delete_program(program);
                        return Err(e);
                    }
                };
                gl.shader_source(shader, &format!("{header}{body}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let e = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(e);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let e = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(e);
            }
            let vao = gl.create_vertex_array()?;
            let texture = gl.create_texture()?;
            let framebuffer = gl.create_framebuffer()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            // One cache per viewport/context. Context teardown reclaims these small
            // resources alongside the mesh renderer; no GL calls on a UI/drop thread.
            Ok(Self {
                program,
                vao,
                texture,
                framebuffer,
                size: [0; 2],
            })
        }
    }
}
