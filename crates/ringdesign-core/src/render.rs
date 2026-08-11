//! Software z-buffer renders: screenshots, turntables, thumbnails.
//!
//! No GPU and no window — a plain rasterizer over the mesh, which is what
//! lets the CLI, the examples, tests and any future configurator draw a ring
//! anywhere. Orthographic, one directional light, gold by default.

use std::path::Path;

use crate::mesh::Mesh;
use crate::FaceClass;

/// Polished gold, the default tint.
pub const GOLD: [f32; 3] = [0.87, 0.71, 0.43];

/// Gold-shaded render at the given orientation. RGB, row-major.
pub fn render(m: &Mesh, yaw: f64, pitch: f64, w: usize, h: usize) -> Vec<u8> {
    draw(m, yaw, pitch, w, h, None, GOLD)
}

/// Same, tinted — the alloy's colour instead of gold.
pub fn render_tinted(m: &Mesh, yaw: f64, pitch: f64, w: usize, h: usize, tint: [f32; 3]) -> Vec<u8> {
    draw(m, yaw, pitch, w, h, None, tint)
}

/// Faces coloured by castability class instead of metal.
pub fn render_classed(
    m: &Mesh,
    yaw: f64,
    pitch: f64,
    w: usize,
    h: usize,
    classes: Option<&[FaceClass]>,
) -> Vec<u8> {
    draw(m, yaw, pitch, w, h, classes, GOLD)
}

/// Supersampled render: drawn at `ss` times the size and box-filtered down,
/// which is the whole of the antialiasing — a ring is mostly edges.
pub fn render_ss(
    m: &Mesh,
    yaw: f64,
    pitch: f64,
    w: usize,
    h: usize,
    ss: usize,
    tint: [f32; 3],
) -> Vec<u8> {
    let ss = ss.max(1);
    let big = draw(m, yaw, pitch, w * ss, h * ss, None, tint);
    if ss == 1 {
        return big;
    }
    let mut out = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut sum = 0usize;
                for dy in 0..ss {
                    for dx in 0..ss {
                        sum += big[(((y * ss + dy) * w * ss) + x * ss + dx) * 3 + c] as usize;
                    }
                }
                out[(y * w + x) * 3 + c] = (sum / (ss * ss)) as u8;
            }
        }
    }
    out
}

/// One antialiased hero frame to a PNG.
pub fn write_png(
    path: impl AsRef<Path>,
    m: &Mesh,
    yaw: f64,
    pitch: f64,
    edge: usize,
    tint: [f32; 3],
) -> anyhow::Result<()> {
    let img = render_ss(m, yaw, pitch, edge, edge, 3, tint);
    image::save_buffer(path, &img, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// A looping turntable GIF: `frames` yaw steps around one revolution.
pub fn write_turntable_gif(
    path: impl AsRef<Path>,
    m: &Mesh,
    frames: usize,
    edge: usize,
    tint: [f32; 3],
) -> anyhow::Result<()> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};

    let frames = frames.clamp(4, 120);
    let file = std::fs::File::create(path)?;
    let mut enc = GifEncoder::new_with_speed(file, 10);
    enc.set_repeat(Repeat::Infinite)?;
    for k in 0..frames {
        let yaw = k as f64 / frames as f64 * std::f64::consts::TAU;
        let rgb = render_ss(m, yaw, 1.12, edge, edge, 2, tint);
        let mut rgba = RgbaImage::new(edge as u32, edge as u32);
        for (i, p) in rgba.pixels_mut().enumerate() {
            *p = image::Rgba([rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2], 255]);
        }
        enc.encode_frame(Frame::from_parts(
            rgba,
            0,
            0,
            Delay::from_numer_denom_ms(70, 1),
        ))?;
    }
    Ok(())
}

fn draw(
    m: &Mesh,
    yaw: f64,
    pitch: f64,
    w: usize,
    h: usize,
    classes: Option<&[FaceClass]>,
    tint: [f32; 3],
) -> Vec<u8> {
    let Some((min, max)) = m.bounds() else {
        return vec![18u8; w * h * 3];
    };
    let c = [
        (min.0 + max.0) as f64 * 0.5,
        (min.1 + max.1) as f64 * 0.5,
        (min.2 + max.2) as f64 * 0.5,
    ];
    let ext = ((max.0 - min.0).max(max.1 - min.1).max(max.2 - min.2)) as f64;
    let scale = w as f64 / (ext * 1.25);

    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let xf = |p: [f64; 3]| -> [f64; 3] {
        let (x, y, z) = (p[0] - c[0], p[1] - c[1], p[2] - c[2]);
        let (x1, y1) = (x * cy - y * sy, x * sy + y * cy);
        let (y2, z2) = (y1 * cp - z * sp, y1 * sp + z * cp);
        [x1, y2, z2]
    };

    let mut depth = vec![f64::NEG_INFINITY; w * h];
    let mut img = vec![18u8; w * h * 3];
    let light = {
        let l: [f64; 3] = [-0.35, -0.5, 0.79];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };

    for (fi, f) in m.faces.iter().enumerate() {
        let Some((a, b, cc)) = m.triangle(f) else { continue };
        let (ta, tb, tc) = (xf(a), xf(b), xf(cc));
        let n = {
            let e1 = [tb[0] - ta[0], tb[1] - ta[1], tb[2] - ta[2]];
            let e2 = [tc[0] - ta[0], tc[1] - ta[1], tc[2] - ta[2]];
            let v = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l < 1e-12 { continue } else { [v[0] / l, v[1] / l, v[2] / l] }
        };
        if n[2] <= 0.0 {
            continue;
        }
        let px = |p: [f64; 3]| (p[0] * scale + w as f64 * 0.5, -p[1] * scale + h as f64 * 0.5);
        let (p0, p1, p2) = (px(ta), px(tb), px(tc));
        let minx = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as usize;
        let maxx = (p0.0.max(p1.0).max(p2.0).ceil() as usize).min(w - 1);
        let miny = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as usize;
        let maxy = (p0.1.max(p1.1).max(p2.1).ceil() as usize).min(h - 1);
        let area = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
        if area.abs() < 1e-9 {
            continue;
        }
        let diff = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let spec = diff.powf(28.0);
        let shade = 0.16 + 0.72 * diff;
        let base = match classes {
            Some(cs) => cs.get(fi).map(|c| c.rgb()).unwrap_or(tint),
            None => tint,
        };
        let lit = if classes.is_some() { 0.35 + 0.65 * diff } else { shade };
        let hl = if classes.is_some() { 0.0 } else { spec * 0.85 };
        let col = [
            ((base[0] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ((base[1] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ((base[2] as f64 * lit + hl) * 255.0).min(255.0) as u8,
        ];
        for y in miny..=maxy {
            for x in minx..=maxx {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((p1.0 - p0.0) * (fy - p0.1) - (fx - p0.0) * (p1.1 - p0.1)) / area;
                let w1 = ((fx - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (fy - p0.1)) / area;
                if w0 < 0.0 || w1 < 0.0 || w0 + w1 > 1.0 {
                    continue;
                }
                let z = ta[2] + w1 * (tb[2] - ta[2]) + w0 * (tc[2] - ta[2]);
                let i = y * w + x;
                if z > depth[i] {
                    depth[i] = z;
                    img[i * 3] = col[0];
                    img[i * 3 + 1] = col[1];
                    img[i * 3 + 2] = col[2];
                }
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha::AlphaLibrary;
    use crate::mesh::{build, BuildParams};
    use crate::RingDesign;

    #[test]
    fn a_render_lights_the_ring_and_a_turntable_gif_writes() {
        let d = RingDesign::default();
        let out = build(
            &d,
            &AlphaLibrary::default(),
            BuildParams { theta_steps: 64, profile_steps: 32, ..Default::default() },
        );
        let img = render(&out.mesh, 0.55, 1.12, 160, 160);
        assert_eq!(img.len(), 160 * 160 * 3);
        // Something brighter than the background made it to the canvas.
        assert!(img.iter().any(|&v| v > 100));

        let dir = std::env::temp_dir().join("ringdesign-render-test");
        std::fs::create_dir_all(&dir).unwrap();
        let gif = dir.join("turntable.gif");
        write_turntable_gif(&gif, &out.mesh, 6, 96, GOLD).unwrap();
        assert!(std::fs::metadata(&gif).unwrap().len() > 1024);
        let png = dir.join("hero.png");
        write_png(&png, &out.mesh, 0.55, 1.12, 96, GOLD).unwrap();
        assert!(std::fs::metadata(&png).unwrap().len() > 500);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
