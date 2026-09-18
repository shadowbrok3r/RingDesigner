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

/// Real-time studio material shared by the native desktop and Android viewports.
pub const STUDIO_GLSL: &str = include_str!("studio.glsl");

/// Linear normal-incidence reflectance approximations for viewport alloy previews.
pub const METAL_FINISHES: &[(&str, [f32; 3])] = &[
    ("Yellow gold", [1.0, 0.766, 0.336]),
    ("Rose gold", [0.97, 0.55, 0.43]),
    ("Silver", [0.95, 0.93, 0.88]),
    ("White gold", [0.86, 0.84, 0.77]),
    ("Platinum", [0.67, 0.64, 0.59]),
    ("Bronze", [0.75, 0.46, 0.24]),
    ("Brass", [0.91, 0.68, 0.32]),
];

pub const POLISHES: &[(&str, f32)] = &[("Polished", 0.14), ("Satin", 0.38), ("Rough", 0.66)];

/// One thing in the frame: a mesh, its colour, and whether it reads as a
/// stone rather than as metal.
///
/// A ring and the stones set into it are two solids that share no vertices —
/// the stones are never in the `Mesh` and never exported — so a picture of a
/// finished piece is a picture of both, framed on the metal.
pub struct Part<'a> {
    pub mesh: &'a Mesh,
    pub tint: [f32; 3],
    /// Harder specular and a brighter body: a faceted stone under the key
    /// light, against metal's broader sheen.
    pub gem: bool,
    /// Shade from the mesh's own vertex normals rather than facet by facet.
    ///
    /// The band is a smooth surface finely tessellated, so flat shading puts
    /// a visible contour on every ring of triangles — worst exactly where
    /// the surface is nearly flat and the facet normals alternate, which is
    /// the skirt around a seat. A stone is the opposite: its facets are the
    /// point, and interpolating across them would sand the sparkle off.
    pub smooth: bool,
    /// Shade as the viewports do: reflected studio softboxes through the metal's own coloured Fresnel
    /// reflectance, or a dielectric over a tinted body for a stone — [`STUDIO_GLSL`], in Rust. Off, the
    /// plain key light the diagnostic renders use.
    pub studio: bool,
    /// Surface roughness for the studio material; [`POLISHES`] names the usual three.
    pub roughness: f64,
}

impl<'a> Part<'a> {
    /// Metal under the studio. `tint` is the alloy's reflectance; [`GOLD`] reads as yellow gold's.
    pub fn metal(mesh: &'a Mesh, tint: [f32; 3]) -> Self {
        let tint = if tint == GOLD { METAL_FINISHES[0].1 } else { tint };
        Self { mesh, tint, gem: false, smooth: true, studio: true, roughness: POLISHES[0].1 as f64 }
    }

    pub fn stone(mesh: &'a Mesh) -> Self {
        Self { mesh, tint: crate::gems::GEM_TINT, gem: true, smooth: false, studio: true, roughness: 0.06 }
    }
}

// --- The studio, as the viewports' shader has it -------------------------------------------------

type V3 = [f64; 3];
fn v_dot(a: V3, b: V3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn v_cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn v_unit(a: V3) -> V3 {
    let l = v_dot(a, a).sqrt();
    if l > 1e-12 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 0.0, 1.0] }
}
fn sstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn studio_card(ray: V3, centre: V3, extent: [f64; 2], blur: f64) -> f64 {
    let axis = v_unit(centre);
    let right = v_unit(v_cross([0.0, 1.0, 0.0], axis));
    let up = v_cross(axis, right);
    let facing = v_dot(ray, axis);
    let uv = [v_dot(ray, right) / facing.max(0.001), v_dot(ray, up) / facing.max(0.001)];
    let edge = [uv[0].abs() - extent[0], uv[1].abs() - extent[1]];
    let distance = edge[0].max(0.0).hypot(edge[1].max(0.0)) + edge[0].max(edge[1]).min(0.0);
    let q = [uv[0] / extent[0], uv[1] / extent[1]];
    let diffuser = (-0.55 * (q[0] * q[0] + q[1] * q[1])).exp();
    (1.0 - sstep(-blur, blur, distance)) * diffuser * sstep(0.0, 0.15, facing)
}

fn studio_environment(ray: V3, roughness: f64, key: V3, ambient: f64) -> V3 {
    let blur = 0.14 + roughness * roughness * 1.8;
    let ceiling = sstep(-0.45, 0.95, ray[1]);
    let mut room: V3 = std::array::from_fn(|k| [0.035, 0.04, 0.05][k] + ([0.38, 0.39, 0.42][k] - [0.035, 0.04, 0.05][k]) * ceiling);
    let glow = ray[2].max(0.0).powi(3);
    let gain = (ambient / 0.20).clamp(0.5, 2.0);
    for k in 0..3 {
        room[k] = (room[k] + [0.16, 0.15, 0.13][k] * glow) * gain;
    }
    // A broad overhead box, a tall strip, a cool rim and a warm floor bounce, as the shader lays them.
    for (colour, centre, extent) in [
        ([3.6, 3.45, 3.15], key, [0.48, 0.85]),
        ([2.5, 2.65, 2.9], [0.9, 0.2, 0.4], [0.13, 1.35]),
        ([1.6, 1.7, 1.85], [-0.5, 0.7, -0.8], [0.9, 0.26]),
        ([0.65, 0.58, 0.46], [0.2, -0.8, 0.55], [1.3, 0.22]),
    ] {
        let c = studio_card(ray, centre, extent, blur);
        for k in 0..3 { room[k] += colour[k] * c; }
    }
    let wide = roughness * roughness * 0.55;
    std::array::from_fn(|k| room[k] + ([0.62, 0.64, 0.68][k] - room[k]) * wide)
}

/// Reinhard, then the sRGB curve: the shader's display transform.
fn studio_display(linear: V3) -> [u8; 3] {
    std::array::from_fn(|k| {
        let m = (linear[k].max(0.0)) / (1.0 + linear[k].max(0.0));
        let e = if m < 0.0031308 { 12.92 * m } else { 1.055 * m.powf(1.0 / 2.4) - 0.055 };
        (e.clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

const STUDIO_KEY: V3 = [-0.38, 0.46, 0.80];
const STUDIO_AMBIENT: f64 = 0.20;

fn studio_reflect(n: V3) -> V3 {
    // reflect((0, 0, -1), n)
    let d = -n[2];
    [-2.0 * d * n[0], -2.0 * d * n[1], -1.0 - 2.0 * d * n[2]]
}

fn studio_metal(n: V3, f0: [f32; 3], roughness: f64) -> [u8; 3] {
    let nv = n[2].clamp(0.0, 1.0);
    let env = studio_environment(studio_reflect(n), roughness, STUDIO_KEY, STUDIO_AMBIENT);
    let rim = (1.0 - nv).powi(5);
    studio_display(std::array::from_fn(|k| env[k] * (f0[k] as f64 + (1.0 - f0[k] as f64) * rim)))
}

fn studio_gem(n: V3, tint: [f32; 3]) -> [u8; 3] {
    let nv = n[2].clamp(0.0, 1.0);
    let fresnel = 0.055 + 0.945 * (1.0 - nv).powi(5);
    let env = studio_environment(studio_reflect(n), 0.06, STUDIO_KEY, STUDIO_AMBIENT);
    let body = 0.12 + 0.65 * v_dot(n, v_unit(STUDIO_KEY)).max(0.0);
    studio_display(std::array::from_fn(|k| tint[k] as f64 * body * (1.0 - fresnel) + env[k] * fresnel))
}

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
    downsample(&big, w, h, ss)
}

/// The box filter that is the whole of the antialiasing.
fn downsample(big: &[u8], w: usize, h: usize, ss: usize) -> Vec<u8> {
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

/// Supersampled render of several parts, framed on the first.
pub fn render_parts_ss(
    parts: &[Part],
    yaw: f64,
    pitch: f64,
    w: usize,
    h: usize,
    ss: usize,
) -> Vec<u8> {
    let ss = ss.max(1);
    let big = draw_parts(parts, yaw, pitch, w * ss, h * ss, None);
    if ss == 1 {
        return big;
    }
    downsample(&big, w, h, ss)
}

/// One antialiased hero frame of several parts to a PNG.
pub fn write_png_parts(
    path: impl AsRef<Path>,
    parts: &[Part],
    yaw: f64,
    pitch: f64,
    edge: usize,
) -> anyhow::Result<()> {
    let img = render_parts_ss(parts, yaw, pitch, edge, edge, 3);
    image::save_buffer(path, &img, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// One antialiased hero frame as PNG bytes.
pub fn png_bytes(m: &Mesh, yaw: f64, pitch: f64, edge: usize, tint: [f32; 3]) -> anyhow::Result<Vec<u8>> {
    let img = render_ss(m, yaw, pitch, edge, edge, 3, tint);
    let mut out = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(&mut out, &img, edge as u32, edge as u32, image::ColorType::Rgb8, image::ImageFormat::Png)?;
    Ok(out.into_inner())
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
    std::fs::write(path, png_bytes(m, yaw, pitch, edge, tint)?)?;
    Ok(())
}

/// A looping turntable GIF as bytes: `frames` yaw steps around one revolution.
pub fn turntable_gif_bytes(m: &Mesh, frames: usize, edge: usize, tint: [f32; 3]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    encode_turntable(&mut out, m, frames, edge, tint)?;
    Ok(out)
}

/// A looping turntable GIF: `frames` yaw steps around one revolution.
pub fn write_turntable_gif(
    path: impl AsRef<Path>,
    m: &Mesh,
    frames: usize,
    edge: usize,
    tint: [f32; 3],
) -> anyhow::Result<()> {
    encode_turntable(std::fs::File::create(path)?, m, frames, edge, tint)
}

fn encode_turntable<W: std::io::Write>(w: W, m: &Mesh, frames: usize, edge: usize, tint: [f32; 3]) -> anyhow::Result<()> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};

    let frames = frames.clamp(4, 120);
    let mut enc = GifEncoder::new_with_speed(w, 10);
    enc.set_repeat(Repeat::Infinite)?;
    for k in 0..frames {
        let yaw = k as f64 / frames as f64 * std::f64::consts::TAU;
        let rgb = render_ss(m, yaw, 1.12, edge, edge, 2, tint);
        let mut rgba = RgbaImage::new(edge as u32, edge as u32);
        for (i, p) in rgba.pixels_mut().enumerate() {
            *p = image::Rgba([rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2], 255]);
        }
        enc.encode_frame(Frame::from_parts(rgba, 0, 0, Delay::from_numer_denom_ms(70, 1)))?;
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
    let part = Part { smooth: classes.is_none(), studio: classes.is_none(), ..Part::metal(m, tint) };
    draw_parts(&[part], yaw, pitch, w, h, classes)
}

/// Several parts into one frame, depth-sorted against each other and framed
/// on the first — the metal, so adding stones cannot move the ring.
fn draw_parts(
    parts: &[Part],
    yaw: f64,
    pitch: f64,
    w: usize,
    h: usize,
    classes: Option<&[FaceClass]>,
) -> Vec<u8> {
    let Some(first) = parts.first() else {
        return vec![18u8; w * h * 3];
    };
    let Some((min, max)) = first.mesh.bounds() else {
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
    let rot = |p: [f64; 3]| -> [f64; 3] {
        let (x1, y1) = (p[0] * cy - p[1] * sy, p[0] * sy + p[1] * cy);
        let (y2, z2) = (y1 * cp - p[2] * sp, y1 * sp + p[2] * cp);
        [x1, y2, z2]
    };
    let xf = |p: [f64; 3]| -> [f64; 3] { rot([p[0] - c[0], p[1] - c[1], p[2] - c[2]]) };

    let mut depth = vec![f64::NEG_INFINITY; w * h];
    let mut img = vec![18u8; w * h * 3];
    let light = {
        let l: [f64; 3] = [-0.35, -0.5, 0.79];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };

    for (part, f, fi) in parts
        .iter()
        .flat_map(|p| p.mesh.faces.iter().enumerate().map(move |(i, f)| (p, f, i)))
    {
        let m = part.mesh;
        let tint = part.tint;
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
        let base = match classes {
            Some(cs) if !part.gem => cs.get(fi).map(|c| c.rgb()).unwrap_or(tint),
            _ => tint,
        };
        let flat = classes.is_some() && !part.gem;
        // The mesh's own vertex normals, rotated into view. Only meaningful
        // when the mesh actually carries them.
        let smooth = part.smooth
            && m.normals.len() == m.vertices.len()
            && f.iter().all(|&i| (i as usize) < m.normals.len());
        let vn = smooth.then(|| {
            // Binary search rather than a cursor: faces arrive part by part, culled and skipped.
            let corners = m.corner_normals.binary_search_by_key(&(fi as u32), |c| c.0).ok().map(|k| m.corner_normals[k].1);
            let g = |k: usize| {
                let nv = corners.map_or(m.normals[f[k] as usize], |c| c[k]);
                rot([nv.0 as f64, nv.1 as f64, nv.2 as f64])
            };
            [g(0), g(1), g(2)]
        });
        // Flat shading, for the facet case and as the fallback.
        let shade_of = |nn: [f64; 3]| -> [u8; 3] {
            if part.studio && !flat {
                return if part.gem { studio_gem(nn, tint) } else { studio_metal(nn, tint, part.roughness) };
            }
            let d = (nn[0] * light[0] + nn[1] * light[1] + nn[2] * light[2]).max(0.0);
            // A stone's facets are flat and small, so a tight specular over
            // a bright body is what reads as sparkle; metal wants the broad
            // sheen.
            let spec = d.powf(if part.gem { 90.0 } else { 28.0 });
            let body = if part.gem { 0.34 + 0.60 * d } else { 0.16 + 0.72 * d };
            let lit = if flat { 0.35 + 0.65 * d } else { body };
            let hl = if flat {
                0.0
            } else if part.gem {
                spec * 1.25
            } else {
                spec * 0.85
            };
            [
                ((base[0] as f64 * lit + hl) * 255.0).min(255.0) as u8,
                ((base[1] as f64 * lit + hl) * 255.0).min(255.0) as u8,
                ((base[2] as f64 * lit + hl) * 255.0).min(255.0) as u8,
            ]
        };
        let col = shade_of(n);
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
                    // Same weights the depth uses: a = 1 - w0 - w1, b = w1,
                    // c = w0.
                    let px = match &vn {
                        Some([na, nb, nc]) => {
                            let wa = 1.0 - w0 - w1;
                            let mut v = [
                                na[0] * wa + nb[0] * w1 + nc[0] * w0,
                                na[1] * wa + nb[1] * w1 + nc[1] * w0,
                                na[2] * wa + nb[2] * w1 + nc[2] * w0,
                            ];
                            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                            if l > 1e-9 {
                                v = [v[0] / l, v[1] / l, v[2] / l];
                                shade_of(v)
                            } else {
                                col
                            }
                        }
                        None => col,
                    };
                    img[i * 3] = px[0];
                    img[i * 3 + 1] = px[1];
                    img[i * 3 + 2] = px[2];
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

#[cfg(test)]
mod byte_tests {
    use super::*;

    #[test]
    fn the_byte_writers_match_the_file_writers() {
        let d = crate::RingDesign::default();
        let lib = crate::AlphaLibrary::builtin();
        let out = crate::mesh::build(&d, &lib, crate::mesh::BuildParams { theta_steps: 48, profile_steps: 24, ..Default::default() });
        let png = png_bytes(&out.mesh, 0.5, 1.1, 32, GOLD).unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
        let gif = turntable_gif_bytes(&out.mesh, 4, 24, GOLD).unwrap();
        assert_eq!(&gif[..6], b"GIF89a");
        let dir = std::env::temp_dir().join(format!("rd-render-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_png(dir.join("h.png"), &out.mesh, 0.5, 1.1, 32, GOLD).unwrap();
        write_turntable_gif(dir.join("t.gif"), &out.mesh, 4, 24, GOLD).unwrap();
        assert_eq!(std::fs::read(dir.join("h.png")).unwrap(), png);
        assert_eq!(std::fs::read(dir.join("t.gif")).unwrap(), gif);
        let _ = std::fs::remove_dir_all(dir);
    }
}
