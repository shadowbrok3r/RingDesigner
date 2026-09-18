//! The mold-pull envelope of a native stock section. Each half is a radial
//! height surface over the mold plane, so no relief island floats above sand.
//! Original sections supply every coordinate; there is no procedural signet loft.
use super::*;

pub(super) fn build(
    d: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    surface: &Surface,
) -> Result<(Mesh, f64, f64)> {
    let nt = params.theta_steps.clamp(24, 4096);
    let np = params.profile_steps.clamp(24, 1024);
    ensure!(
        nt * np * 2 < 2_000_000,
        "Sand envelope exceeds the 2 million triangle budget"
    );
    let no = (np * 3 / 4 / 2) * 2;
    let nb = np - no;
    let half = no / 2;
    let ctx = d.field_context();
    let native = ctx.imported_surface.as_ref().unwrap();
    // What is poured and what the bench cuts afterwards. The envelope is the sand's: it supports the
    // cast relief, and would close every line a graver is still to cut, so the bench's work is laid on
    // the supported surface rather than under it.
    let (mut cast, mut bench) = (d.layers.clone(), d.layers.clone());
    let any_bench = d.layers.layers.iter().any(|e| e.enabled && e.bench_only);
    for e in &mut cast.layers { e.enabled &= !e.bench_only; }
    for e in &mut bench.layers { e.enabled &= e.bench_only; }
    #[cfg(feature = "parallel")]
    use rayon::prelude::*;
    #[cfg(feature = "parallel")]
    let iter = (0..nt).into_par_iter();
    #[cfg(not(feature = "parallel"))]
    let iter = 0..nt;
    let row = |i| -> Result<(Vec<Vec3>, Vec<Vec3>, f64, f64, f64)> {
        let theta = i as f64 * 360. / nt as f64;
        let (sin, cos) = theta.to_radians().sin_cos();
        let station = theta / 360. * STATIONS as f64;
        let si = station.floor() as usize;
        let sf = station.fract();
        // Find the actual centre of the interpolated chart. Interpolating
        // source arc fractions can leave the neighbouring row across Z=0.
        let (mut left, mut right) = (0., 1.);
        for _ in 0..28 {
            let f = (left + right) * 0.5;
            if native.point(theta, f)[2] < 0. {
                left = f;
            } else {
                right = f;
            }
        }
        let middle = (left + right) * 0.5;
        let mut outer = Vec::with_capacity(no + 1);
        let mut bench_r: Vec<f64> = Vec::with_capacity(no + 1);
        let mut rest = Vec::with_capacity(np);
        let mut hi = 0_f64;
        let mut lo = 0_f64;
        for j in 0..=no {
            let f = if j <= half {
                middle * j as f64 / half as f64
            } else {
                middle + (1. - middle) * (j - half) as f64 / half as f64
            };
            let frame = native.frame(theta, f);
            let p = frame.point;
            let r = p[0].hypot(p[1]);
            rest.push(Vec3((r * cos) as f32, (r * sin) as f32, p[2] as f32));
            let path_length = surface.paths[si].length * (1. - sf)
                + surface.paths[(si + 1) % STATIONS].length * sf;
            let weight = (f.min(1. - f) * path_length / crate::profile::EDGE_FADE_MM).clamp(0., 1.);
            let h = if r < d.inner_radius_mm() + 0.15 {
                0.
            } else {
                crate::mesh::soft_height(
                    &cast,
                    crate::Uv {
                        u: ctx.u_of_theta(theta),
                        v: f * ctx.band_v_len_mm,
                    },
                    &ctx,
                    lib,
                    params.soften_mm,
                ) * weight
            };
            let h = if h.is_finite() { h } else { 0. };
            let cut = if any_bench && r >= d.inner_radius_mm() + 0.15 {
                crate::mesh::soft_height(&bench, crate::Uv { u: ctx.u_of_theta(theta), v: f * ctx.band_v_len_mm }, &ctx, lib, params.soften_mm) * weight
            } else {
                0.
            };
            bench_r.push(if cut.is_finite() { cut * (frame.normal[0] * cos + frame.normal[1] * sin) } else { 0. });
            hi = hi.max(h);
            lo = lo.min(h);
            let nr = frame.normal[0] * cos + frame.normal[1] * sin;
            let r_new =
                (r + h * nr).max((d.inner_radius_mm() + params.min_wall_mm.max(0.05)).min(r));
            let dz = (h * frame.normal[2] * crate::field::smoothstep(0., 0.3, p[2].abs()))
                .clamp(-p[2].abs() * 0.5, p[2].abs() * 0.5);
            // Roll the bore edge into the stock rather than ending a rounded
            // shank in a thin wedge. The original outer chart stops near the
            // bore cylinder; joining that cut directly to a new bore can leave
            // a knife edge even when the radial wall is comfortably thick.
            // This local comfort roll removes that edge without touching the
            // face or changing the nominal opening at the parting plane.
            let roll = 0.45 * (1. - ((r - d.inner_radius_mm()) / 0.8).clamp(0., 1.)).powi(2);
            let roll = roll.min(p[2].abs() * 0.25);
            let z = if j == half {
                0.
            } else {
                p[2] + dz - p[2].signum() * roll
            };
            outer.push([r_new, z]);
        }
        // The smallest radial envelope containing the designed surface.
        // Quiet slopes and existing monotone relief remain unchanged.
        let mut added = 0_f64;
        for j in 1..=half {
            let next = outer[j][0].max(outer[j - 1][0] + 1e-7);
            added = added.max(next - outer[j][0]);
            outer[j][0] = next;
        }
        for j in (half..no).rev() {
            let next = outer[j][0].max(outer[j + 1][0] + 1e-7);
            added = added.max(next - outer[j][0]);
            outer[j][0] = next;
        }
        ensure!(
            added <= 1.0,
            "Sand withdrawal support would add more than 1 mm; reduce relief or choose different stock"
        );
        let floor = d.inner_radius_mm() + params.min_wall_mm.max(0.05);
        for (p, cut) in outer.iter_mut().zip(&bench_r) {
            if *cut != 0. {
                hi = hi.max(*cut);
                lo = lo.min(*cut);
                p[0] = (p[0] + cut).max(floor.min(p[0]));
            }
        }
        // A calibrated, mildly relieved bore meets the exact stock rim. Its
        // radius grows away from the parting plane, avoiding imported polygon
        // noise that can trap a thin collar of sand at nominal finger size.
        let lower = outer[0];
        let upper = outer[no];
        let mut points: Vec<_> = outer
            .into_iter()
            .map(|[r, z]| Vec3((r * cos) as f32, (r * sin) as f32, z as f32))
            .collect();
        for j in 1..nb {
            let f = j as f64 / nb as f64;
            let end = if f <= 0.5 { upper } else { lower };
            let z = if f <= 0.5 {
                upper[1] * (1. - 2. * f)
            } else {
                lower[1] * (2. * f - 1.)
            };
            let h = end[1].abs().max(1e-6);
            let edge = (end[0] - d.inner_radius_mm()).max(0.);
            let draft = (edge * 0.4).min(h * 0.012);
            let r = d.inner_radius_mm()
                + draft * (z.abs() / h)
                + (edge - draft) * crate::field::smoothstep((h - 0.35).max(h * 0.5), h, z.abs());
            let p = Vec3((r * cos) as f32, (r * sin) as f32, z as f32);
            points.push(p);
            rest.push(p);
        }
        Ok((points, rest, hi, lo, added))
    };
    let rows = iter.map(row).collect::<Result<Vec<_>>>()?;
    let mut mesh = Mesh::default();
    let mut rest = Vec::with_capacity(nt * np);
    let mut hi = 0_f64;
    let mut lo = 0_f64;
    let mut added = 0_f64;
    for (row, original, h, l, a) in rows {
        mesh.vertices.extend(row);
        rest.extend(original);
        hi = hi.max(h);
        lo = lo.min(l);
        added = added.max(a);
    }
    mesh.faces.reserve(nt * np * 2);
    for i in 0..nt {
        for j in 0..np {
            let a = (i * np + j) as u32;
            let b = (((i + 1) % nt) * np + j) as u32;
            let c = (i * np + (j + 1) % np) as u32;
            let e = (((i + 1) % nt) * np + (j + 1) % np) as u32;
            mesh.faces.extend([[a, b, c], [b, e, c]]);
        }
    }
    let rest_geometric = smooth_normals(&rest, &mesh.faces);
    let mut baseline = rest_geometric.clone();
    // Smooth only the unornamented stock normals. Keep the relief's normal
    // delta intact, so fine beadwork stays sharp without striping the shank.
    for _ in 0..5 {
        let previous = baseline.clone();
        for i in 0..nt {
            for j in 0..np {
                let at = i * np + j;
                let mut n = previous[at];
                n = Vec3(n.0 * 4., n.1 * 4., n.2 * 4.);
                for k in [
                    ((i + 1) % nt) * np + j,
                    ((i + nt - 1) % nt) * np + j,
                    i * np + (j + 1) % np,
                    i * np + (j + np - 1) % np,
                ] {
                    let v = previous[k];
                    n.0 += v.0;
                    n.1 += v.1;
                    n.2 += v.2;
                }
                baseline[at] = unit(n);
            }
        }
    }
    mesh.normals = smooth_normals(&mesh.vertices, &mesh.faces)
        .iter()
        .zip(&rest_geometric)
        .zip(&baseline)
        .map(|((n, r), b)| unit(Vec3(b.0 + n.0 - r.0, b.1 + n.1 - r.1, b.2 + n.2 - r.2)))
        .collect();
    log::info!("Imported sand envelope: maximum radial support {added:.4} mm");
    Ok((mesh, hi + added, lo))
}
