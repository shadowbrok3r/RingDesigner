//! Render-only faceted stone previews for the viewport.
//!
//! Stones are never in the `Mesh` and never exported — sand casts the stock
//! and the bench sets the stones. This builds their triangles on the CPU each
//! rebuild, one faceted brilliant per stone-bearing seat station, positioned
//! by evaluating the displaced surface exactly where the seat is, and hands
//! the viewport an interleaved buffer in the ring's own vertex layout.

use crate::alpha::AlphaLibrary;
use crate::castability::section_at;
use crate::field::{FieldContext, Layer, LayerEntry, LayerStack, SeatStyle, Uv};
use crate::gem::{Gem, GemCut};
use crate::RingDesign;

#[cfg(test)]
use crate::render as raster;

/// Diamond-ish preview tint; the shader's key light and specular do the rest.
pub const GEM_TINT: [f32; 3] = [0.72, 0.82, 0.92];

/// Interleaved `position(3) normal(3) color(3) color2(3)` triangles for every
/// stone, matching the ring buffer's layout.
pub fn preview_vertices(design: &RingDesign, lib: &AlphaLibrary) -> Vec<f32> {
    let ctx = design.field_context();
    let mut out = Vec::new();
    walk(design, lib, &ctx, &design.layers, &mut out);
    out
}

fn walk(
    design: &RingDesign,
    lib: &AlphaLibrary,
    ctx: &FieldContext,
    stack: &LayerStack,
    out: &mut Vec<f32>,
) {
    for entry in &stack.layers {
        if !entry.enabled {
            continue;
        }
        match &entry.layer {
            Layer::SeatPad(seat) => {
                if let Some(gem) = seat.gem {
                    if kept(entry, ctx, seat.theta_deg, seat.v_mm) {
                        place(design, lib, ctx, seat.theta_deg, seat.v_mm, gem, seat.style, out);
                    }
                }
            }
            Layer::SeatRun(run) => {
                let n = run.count.clamp(1, 200);
                let mut seat = run.seat;
                seat.fit_stone(run.gem);
                for k in 0..n {
                    let theta = k as f64 * 360.0 / n as f64;
                    if kept(entry, ctx, theta, seat.v_mm) {
                        place(design, lib, ctx, theta, seat.v_mm, run.gem, seat.style, out);
                    }
                }
            }
            Layer::Group(g) => walk(design, lib, ctx, &g.stack, out),
            _ => {}
        }
    }
}

fn kept(entry: &LayerEntry, ctx: &FieldContext, theta_deg: f64, v_mm: f64) -> bool {
    let uv = Uv { u: ctx.u_of_theta(theta_deg.rem_euclid(360.0)), v: v_mm };
    entry.window.mask(uv, ctx) > 0.5
}

/// One stone: find the displaced surface under the seat, build a frame on it,
/// and append the faceted mesh with the girdle settled into the seat.
fn place(
    design: &RingDesign,
    lib: &AlphaLibrary,
    ctx: &FieldContext,
    theta_deg: f64,
    v_mm: f64,
    gem: Gem,
    style: SeatStyle,
    out: &mut Vec<f32>,
) {
    let section = section_at(design, lib, theta_deg, 160);
    let surface: Vec<_> = section.points.iter().filter(|p| p.surface).collect();
    if surface.len() < 2 {
        return;
    }
    // `v` along the section's own surface, matched to the seat's reference v
    // the same normalized way the layer itself is evaluated.
    let total: f64 = surface.windows(2).map(|w| seg(w[0].r, w[0].z, w[1].r, w[1].z)).sum();
    let target = (v_mm / ctx.band_v_len_mm.max(1e-9)).clamp(0.0, 1.0) * total;
    let mut acc = 0.0;
    let mut best = surface[0];
    for w in surface.windows(2) {
        acc += seg(w[0].r, w[0].z, w[1].r, w[1].z);
        best = w[1];
        if acc >= target {
            break;
        }
    }

    let (sin_t, cos_t) = theta_deg.to_radians().sin_cos();
    let pos = [best.r * cos_t, best.r * sin_t, best.z];
    let n = normalize([best.nr * cos_t, best.nr * sin_t, best.nz]);
    // Around-ring tangent, squared against the normal; the stone's length
    // runs along the ring like the seats are spaced.
    let ring = [-sin_t, cos_t, 0.0];
    let t = normalize(reject(ring, n));
    let b = cross(n, t);

    // The girdle settles into the seat: below the rim of a bezel's pocket,
    // a whisker into a drilled pad — the pavilion disappears into the metal
    // and the crown stands proud, which is what a set stone looks like.
    let settle = match style {
        SeatStyle::Bezel => 0.35 * gem.depth_mm(),
        _ => 0.22 * gem.depth_mm(),
    };
    let centre = [
        pos[0] - n[0] * settle,
        pos[1] - n[1] * settle,
        pos[2] - n[2] * settle,
    ];

    for (p0, p1, p2) in facets(gem) {
        let world = |p: [f64; 3]| -> [f64; 3] {
            [
                centre[0] + t[0] * p[0] + b[0] * p[1] + n[0] * p[2],
                centre[1] + t[1] * p[0] + b[1] * p[1] + n[1] * p[2],
                centre[2] + t[2] * p[0] + b[2] * p[1] + n[2] * p[2],
            ]
        };
        let (w0, w1, w2) = (world(p0), world(p1), world(p2));
        let fn3 = normalize(cross(sub(w1, w0), sub(w2, w0)));
        for w in [w0, w1, w2] {
            out.extend_from_slice(&[
                w[0] as f32,
                w[1] as f32,
                w[2] as f32,
                fn3[0] as f32,
                fn3[1] as f32,
                fn3[2] as f32,
                GEM_TINT[0],
                GEM_TINT[1],
                GEM_TINT[2],
                GEM_TINT[0],
                GEM_TINT[1],
                GEM_TINT[2],
            ]);
        }
    }
}

/// The faceted solid in the stone's own frame: x along the length, y across,
/// z up the crown. Girdle at z = 0, table above, culet below. Flat facets —
/// the sparkle is per-face normals under the viewport's key light.
fn facets(gem: Gem) -> Vec<([f64; 3], [f64; 3], [f64; 3])> {
    let (hw, hl) = (gem.w_mm * 0.5, gem.l_mm * 0.5);
    let depth = gem.depth_mm();
    let crown = depth * 0.35;
    let pav = depth * 0.65;
    // Girdle plan as a superellipse: 2 is round, higher squares the corners.
    let exp = match gem.cut {
        GemCut::Round | GemCut::Oval | GemCut::Pear => 2.0,
        GemCut::Cushion | GemCut::Trillion => 3.2,
        GemCut::Princess | GemCut::Emerald | GemCut::Baguette => 6.0,
        GemCut::Marquise => 1.5,
    };
    const SEG: usize = 16;
    let ring = |scale: f64, z: f64| -> Vec<[f64; 3]> {
        (0..SEG)
            .map(|i| {
                let a = (i as f64 + 0.5) / SEG as f64 * std::f64::consts::TAU;
                let (s, c) = a.sin_cos();
                let m = (c.abs().powf(exp) + s.abs().powf(exp)).powf(-1.0 / exp);
                [c * m * hl * scale, s * m * hw * scale, z]
            })
            .collect()
    };

    let girdle = ring(1.0, 0.0);
    let bezel = ring(0.78, crown * 0.55);
    let table = ring(0.52, crown);
    let culet = [0.0, 0.0, -pav];
    let tc = [0.0, 0.0, crown];

    let mut tris = Vec::with_capacity(SEG * 6);
    for i in 0..SEG {
        let j = (i + 1) % SEG;
        // Crown in two bands, then the table fan.
        tris.push((girdle[i], girdle[j], bezel[j]));
        tris.push((girdle[i], bezel[j], bezel[i]));
        tris.push((bezel[i], bezel[j], table[j]));
        tris.push((bezel[i], table[j], table[i]));
        tris.push((table[i], table[j], tc));
        // Pavilion fan to the culet.
        tris.push((girdle[j], girdle[i], culet));
    }
    tris
}

fn seg(r0: f64, z0: f64, r1: f64, z1: f64) -> f64 {
    ((r1 - r0).powi(2) + (z1 - z0).powi(2)).sqrt()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn reject(v: [f64; 3], n: [f64; 3]) -> [f64; 3] {
    let d = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
    [v[0] - n[0] * d, v[1] - n[1] * d, v[2] - n[2] * d]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-12 { [v[0] / l, v[1] / l, v[2] / l] } else { [0.0, 0.0, 1.0] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{SeatPadLayer, SeatRunLayer};
    use crate::LayerEntry;

    /// With `RD_GEM_SHEET=/some/dir`, renders the ring with the preview
    /// stones merged in, using the same software rasterizer the examples use
    /// — the eyeball check for position, scale and orientation.
    #[test]
    fn stones_land_on_their_seats() {
        let lib = AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        d.profile.width_mm = 8.0;
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.5);
        run.seat.v_mm = ctx.band_v_len_mm * 0.5;
        run.solve_spacing(&ctx);
        d.layers.layers.push(LayerEntry::new("Eternity", Layer::SeatRun(run)));

        let built = crate::mesh::build(
            &d,
            &lib,
            crate::BuildParams {
                theta_steps: 384,
                profile_steps: 144,
                ..Default::default()
            },
        );
        let verts = preview_vertices(&d, &lib);
        assert!(!verts.is_empty());

        // Every stone's girdle centre must sit within its own depth of the
        // displaced surface: on the seat, not floating or buried in the bore.
        let crest = built.reference.crest_radius_mm;
        let inner = d.inner_radius_mm();
        let mut tri_r = Vec::new();
        for v in verts.chunks(12) {
            tri_r.push((v[0] as f64).hypot(v[1] as f64));
        }
        let max_r = tri_r.iter().cloned().fold(0.0, f64::max);
        let min_r = tri_r.iter().cloned().fold(f64::MAX, f64::min);
        assert!(max_r < crest + 3.0, "stones float: {max_r:.2} vs crest {crest:.2}");
        assert!(min_r > inner - 1.0, "stones sunk past the bore: {min_r:.2}");

        if let Ok(dir) = std::env::var("RD_GEM_SHEET") {
            let mut mesh = built.mesh.clone();
            let base = mesh.vertices.len() as u32;
            for (i, v) in verts.chunks(12).enumerate() {
                mesh.vertices.push(crate::mesh::Vec3(v[0], v[1], v[2]));
                mesh.normals.push(crate::mesh::Vec3(v[3], v[4], v[5]));
                if i % 3 == 2 {
                    let k = base + i as u32;
                    mesh.faces.push([k - 2, k - 1, k]);
                }
            }
            for (tag, yaw, pitch) in [("hero", 0.5, 1.15), ("top", 0.0, 1.571)] {
                let img = raster::render(&mesh, yaw, pitch, 820, 820);
                let mut ppm = format!("P6\n820 820\n255\n").into_bytes();
                ppm.extend_from_slice(&img);
                std::fs::write(format!("{dir}/gems_{tag}.ppm"), ppm).unwrap();
            }
        }
    }

    #[test]
    fn stones_make_triangles_and_bare_designs_make_none() {
        let lib = AlphaLibrary::builtin();
        let d = RingDesign::default();
        assert!(preview_vertices(&d, &lib).is_empty());

        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.seat.v_mm = ctx.band_v_len_mm * 0.5;
        run.solve_spacing(&ctx);
        let count = run.count;
        d.layers.layers.push(LayerEntry::new("Run", Layer::SeatRun(run)));
        let v = preview_vertices(&d, &lib);
        assert!(!v.is_empty());
        assert_eq!(v.len() % 12, 0);
        // Every station carries the same facet count.
        let per = v.len() / count as usize;
        assert_eq!(v.len(), per * count as usize);

        // A pad without a stone previews nothing.
        let mut d2 = RingDesign::default();
        d2.layers
            .layers
            .push(LayerEntry::new("Empty pad", Layer::SeatPad(SeatPadLayer::default())));
        assert!(preview_vertices(&d2, &lib).is_empty());
    }

    #[test]
    fn the_facets_close_around_every_cut() {
        for &cut in GemCut::ALL {
            let g = Gem::calibrated(cut, 4.0);
            let tris = facets(g);
            assert!(tris.len() >= 32, "{cut:?}");
            // Signed volume of the fan-closed solid is positive: outward wound.
            let mut vol = 0.0;
            for (a, b, c) in &tris {
                vol += a[0] * (b[1] * c[2] - b[2] * c[1])
                    - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]);
            }
            assert!(vol > 0.0, "{cut:?} wound inward: {vol}");
        }
    }
}
