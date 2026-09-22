//! Thumbnails for the choices a picker offers: metals, polishes, light rigs,
//! shading modes and the standard views.
//!
//! Every one is the same ball under the viewports' own studio, so a chip of a
//! metal is the metal the ring will be and a chip of a shading mode is that
//! mode's own colours on a surface that turns through every normal. Built
//! once per choice and cached as an egui texture, keyed by what drew it.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use ringdesign_core::FaceClass;

use crate::viewport::{FINISHES, LIGHT_RIGS, ShadeMode};

/// A chip's edge in points; the texture is built at twice that.
pub const EDGE: f32 = 20.0;
const PIXELS: usize = 40;

/// The metal ball for a finish, at a polish and under a light rig.
pub fn metal(ctx: &egui::Context, finish: usize, polish: usize, light: usize) -> TextureHandle {
    let key = format!("swatch-metal-{finish}-{polish}-{light}");
    cached(ctx, &key, || {
        let f = FINISHES.get(finish).map(|f| f.rgb).unwrap_or(ringdesign_core::render::GOLD);
        let f0 = reflectance(finish, f);
        let rough = ringdesign_core::render::POLISHES.get(polish).map(|p| p.1).unwrap_or(0.14) as f64;
        let rig = LIGHT_RIGS.get(light).unwrap_or(&LIGHT_RIGS[0]);
        let dir = [rig.dir[0] as f64, rig.dir[1] as f64, rig.dir[2] as f64];
        rgba_image(ringdesign_core::render::metal_ball(PIXELS, f0, rough, dir, rig.ambient as f64))
    })
}

/// The chip for a shading mode: the same ball, painted the way that mode
/// paints the ring.
pub fn shade(ctx: &egui::Context, mode: ShadeMode) -> TextureHandle {
    cached(ctx, &format!("swatch-shade-{}", mode.label()), || {
        if mode == ShadeMode::Metal {
            return rgba_image(ringdesign_core::render::metal_ball(
                PIXELS,
                reflectance(0, FINISHES[0].rgb),
                0.14,
                [-0.38, 0.46, 0.80],
                0.20,
            ));
        }
        ball(|n| {
            let key = [-0.38f32, 0.46, 0.80];
            let lambert = (n[0] * key[0] + n[1] * key[1] + n[2] * key[2]).max(0.0);
            let lit = |c: [f32; 3]| [c[0] * (0.74 + 0.26 * lambert), c[1] * (0.74 + 0.26 * lambert), c[2] * (0.74 + 0.26 * lambert)];
            match mode {
                // The draft ramp over the ball's own slope against a +Z pull.
                ShadeMode::Draft => lit(match n[2].abs() {
                    a if a > 0.50 => FaceClass::Good,
                    a if a > 0.22 => FaceClass::Marginal,
                    a if a > 0.06 => FaceClass::Vertical,
                    _ => FaceClass::Undercut,
                }
                .rgb()),
                // Thin at the rim, thick through the middle.
                ShadeMode::Wall => {
                    let t = n[2].clamp(0.0, 1.0);
                    lit([1.0 - t * 0.78, 0.22 + t * 0.62, 0.24 + t * 0.16])
                }
                // Cope above the parting plane, drag below, the band bright.
                ShadeMode::Halves => {
                    let half = if n[1] > 0.0 { [0.42, 0.62, 0.82] } else { [0.80, 0.62, 0.38] };
                    let band = 1.0 - smoothstep(0.035, 0.09, n[1].abs());
                    let mixed: [f32; 3] = std::array::from_fn(|k| half[k] + ([1.0, 0.92, 0.25][k] - half[k]) * band);
                    [mixed[0] * (0.72 + 0.28 * lambert), mixed[1] * (0.72 + 0.28 * lambert), mixed[2] * (0.72 + 0.28 * lambert)]
                }
                ShadeMode::Normals => [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5],
                ShadeMode::Metal => [0.0; 3],
            }
        })
    })
}

/// A plain band seen from a standard view: what that view frames.
pub fn view_chip(ctx: &egui::Context, view: crate::camera::StandardView) -> TextureHandle {
    use crate::camera::StandardView as V;
    cached(ctx, &format!("swatch-view-{}", view.label()), || {
        // The software renderer turns the object: yaw about the finger axis,
        // then pitch away from looking straight down it.
        let (yaw, pitch) = match view {
            V::Face => (0.0, 0.0),
            V::Edge => (0.0, std::f64::consts::FRAC_PI_2),
            V::Profile => (std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2),
            V::Iso => (0.45, 1.0),
        };
        let design = ringdesign_core::RingDesign::default();
        let lib = ringdesign_core::AlphaLibrary::default();
        let built = ringdesign_core::mesh::build(
            &design,
            &lib,
            ringdesign_core::BuildParams { theta_steps: 128, profile_steps: 40, ..Default::default() },
        );
        let rgb = ringdesign_core::render::render_ss(&built.mesh, yaw, pitch, PIXELS, PIXELS, 2, ringdesign_core::render::GOLD);
        ColorImage {
            size: [PIXELS, PIXELS],
            pixels: rgb.chunks_exact(3).map(|p| Color32::from_rgb(p[0], p[1], p[2])).collect(),
            source_size: egui::vec2(PIXELS as f32, PIXELS as f32),
        }
    })
}

/// A selectable row: chip, then label.
pub fn row(ui: &mut egui::Ui, texture: &TextureHandle, label: &str, selected: bool) -> egui::Response {
    ui.add(
        egui::Button::image_and_text(
            egui::Image::new((texture.id(), texture.size_vec2())).fit_to_exact_size(egui::vec2(EDGE, EDGE)),
            label,
        )
        .selected(selected)
        .min_size(egui::vec2(ui.available_width().min(190.0), 0.0)),
    )
}

/// The alloy's linear reflectance, which is what the studio shades against —
/// the viewport's own `rgb` is the sRGB swatch the picker used to show.
fn reflectance(finish: usize, fallback: [f32; 3]) -> [f32; 3] {
    ringdesign_core::render::METAL_FINISHES
        .get(finish)
        .map(|f| f.1)
        .unwrap_or(fallback)
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A ball shaded by a per-normal colour, transparent off its rim.
fn ball(shade: impl Fn([f32; 3]) -> [f32; 3]) -> ColorImage {
    let r = PIXELS as f32 * 0.5;
    let mut pixels = vec![Color32::TRANSPARENT; PIXELS * PIXELS];
    for y in 0..PIXELS {
        for x in 0..PIXELS {
            let (u, v) = ((x as f32 + 0.5 - r) / (r - 0.5), (r - 0.5 - y as f32) / (r - 0.5));
            let d2 = u * u + v * v;
            if d2 >= 1.0 {
                continue;
            }
            let c = shade([u, v, (1.0 - d2).sqrt()]);
            let a = (((1.0 - d2.sqrt()) * (r - 0.5)).clamp(0.0, 1.0) * 255.0) as u8;
            let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
            pixels[y * PIXELS + x] = Color32::from_rgba_unmultiplied(byte(c[0]), byte(c[1]), byte(c[2]), a);
        }
    }
    ColorImage { size: [PIXELS, PIXELS], pixels, source_size: egui::vec2(PIXELS as f32, PIXELS as f32) }
}

fn rgba_image(rgba: Vec<u8>) -> ColorImage {
    ColorImage {
        size: [PIXELS, PIXELS],
        pixels: rgba
            .chunks_exact(4)
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
        source_size: egui::vec2(PIXELS as f32, PIXELS as f32),
    }
}

/// Textures live in egui's memory under their own key, so a chip is built
/// once per choice for the life of the app.
fn cached(ctx: &egui::Context, key: &str, build: impl FnOnce() -> ColorImage) -> TextureHandle {
    let id = egui::Id::new(("swatch", key));
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return t;
    }
    let t = ctx.load_texture(key, build(), TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}
