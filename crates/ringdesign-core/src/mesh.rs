//! Sweep the cross-section, displace it by the height field, and triangulate.
//!
//! Both grid directions wrap — the cross-section is a closed loop and the sweep
//! closes at 360° — so the result has torus topology and is watertight with no
//! cap geometry and no special cases.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::adaptive::Spacing;
use crate::alpha::AlphaLibrary;
use crate::field::Uv;
use crate::metal::{MetalWeight, metal_table};
use crate::profile::ProfileLoop;
use crate::RingDesign;

/// Metal that must remain between a displaced surface and the bore, mm.
pub const MIN_WALL_MM: f64 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3(pub f32, pub f32, pub f32);

impl Vec3 {
    pub fn is_finite(&self) -> bool {
        self.0.is_finite() && self.1.is_finite() && self.2.is_finite()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    /// Smooth per-vertex normals, parallel to `vertices`.
    pub normals: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self.vertices.iter();
        let first = *it.next()?;
        let (mut min, mut max) = (first, first);
        for v in it {
            min.0 = min.0.min(v.0);
            min.1 = min.1.min(v.1);
            min.2 = min.2.min(v.2);
            max.0 = max.0.max(v.0);
            max.1 = max.1.max(v.1);
            max.2 = max.2.max(v.2);
        }
        Some((min, max))
    }

    /// Enclosed volume in mm³, from the signed tetrahedron sum.
    pub fn volume_mm3(&self) -> f64 {
        let mut acc = 0.0f64;
        for f in &self.faces {
            let (Some(a), Some(b), Some(c)) = (
                self.vertices.get(f[0] as usize),
                self.vertices.get(f[1] as usize),
                self.vertices.get(f[2] as usize),
            ) else {
                continue;
            };
            let (a, b, c) = (
                [a.0 as f64, a.1 as f64, a.2 as f64],
                [b.0 as f64, b.1 as f64, b.2 as f64],
                [c.0 as f64, c.1 as f64, c.2 as f64],
            );
            acc += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
        (acc / 6.0).abs()
    }

    /// Total surface area in mm².
    pub fn surface_area_mm2(&self) -> f64 {
        self.faces
            .iter()
            .filter_map(|f| self.triangle(f))
            .map(|(a, b, c)| {
                let e1 = sub(b, a);
                let e2 = sub(c, a);
                norm(cross(e1, e2)) * 0.5
            })
            .sum()
    }

    pub fn triangle(&self, f: &[u32; 3]) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
        let a = self.vertices.get(f[0] as usize)?;
        let b = self.vertices.get(f[1] as usize)?;
        let c = self.vertices.get(f[2] as usize)?;
        Some((
            [a.0 as f64, a.1 as f64, a.2 as f64],
            [b.0 as f64, b.1 as f64, b.2 as f64],
            [c.0 as f64, c.1 as f64, c.2 as f64],
        ))
    }

    /// Outward unit normal of a face, or `None` for a degenerate triangle.
    pub fn face_normal(&self, f: &[u32; 3]) -> Option<[f64; 3]> {
        let (a, b, c) = self.triangle(f)?;
        let n = cross(sub(b, a), sub(c, a));
        let len = norm(n);
        (len > 1e-12).then(|| [n[0] / len, n[1] / len, n[2] / len])
    }

    /// Every edge of a closed mesh is shared by exactly two triangles.
    pub fn validate(&self) -> Validation {
        use std::collections::HashMap;
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for f in &self.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let key = if a < b { (a, b) } else { (b, a) };
                *edges.entry(key).or_insert(0) += 1;
            }
        }
        let boundary_edges = edges.values().filter(|&&c| c == 1).count();
        let non_manifold_edges = edges.values().filter(|&&c| c > 2).count();
        Validation {
            watertight: boundary_edges == 0 && non_manifold_edges == 0,
            triangle_count: self.faces.len(),
            vertex_count: self.vertices.len(),
            boundary_edges,
            non_manifold_edges,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Validation {
    pub watertight: bool,
    pub triangle_count: usize,
    pub vertex_count: usize,
    pub boundary_edges: usize,
    pub non_manifold_edges: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BuildParams {
    /// Sweep steps around the ring.
    pub theta_steps: usize,
    /// Vertices around the cross-section loop.
    pub profile_steps: usize,
    /// Metal kept between a displaced surface and the bore, mm.
    pub min_wall_mm: f64,
    /// Place the same number of sample lines by where the detail is instead of
    /// at equal spacing. Off by default — see the `adaptive` module for the
    /// measurements, which show it losing on any design carrying relief.
    #[serde(default)]
    pub adaptive: bool,
}

impl Default for BuildParams {
    fn default() -> Self {
        Self {
            theta_steps: 512,
            profile_steps: 192,
            min_wall_mm: MIN_WALL_MM,
            adaptive: false,
        }
    }
}

impl BuildParams {
    pub const PRESETS: &'static [(&'static str, usize, usize)] = &[
        ("Draft", 192, 96),
        ("Preview", 384, 144),
        ("Fine", 512, 192),
        ("Export", 1024, 320),
        ("Maximum", 1536, 448),
    ];

    pub fn triangle_estimate(&self) -> usize {
        self.theta_steps * self.profile_steps * 2
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub validation: Validation,
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    /// Overall size (x, y, z) in mm.
    pub bounds_mm: [f64; 3],
    pub inner_diameter_mm: f64,
    pub outer_diameter_mm: f64,
    pub band_width_mm: f64,
    /// Peak displacement applied by the layer stack, mm.
    pub max_relief_mm: f64,
    /// Deepest engraved displacement, mm.
    pub min_relief_mm: f64,
    pub metals: Vec<MetalWeight>,
    pub build_ms: u128,
}

pub struct BuildResult {
    pub mesh: Mesh,
    pub report: Report,
    /// The unmodulated cross-section, reused by the section view.
    pub reference: ProfileLoop,
    /// Where this build put its sample lines. The section view needs it to
    /// slice the solid rather than an independent approximation of it.
    pub spacing: Spacing,
}

/// Build the ring mesh from a design.
pub fn build(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> BuildResult {
    let started = std::time::Instant::now();

    let n_theta = params.theta_steps.clamp(24, 4096);
    let n_prof = params.profile_steps.clamp(24, 1024);
    let inner_r = design.inner_radius_mm();

    let reference = design.reference_loop();
    let ctx = design.field_context();
    let min_wall = params.min_wall_mm.max(0.05);

    // Both directions stay regular and wrapping, so this only moves the sample
    // lines — the grid is still a torus and still watertight by construction.
    let spacing = if params.adaptive {
        Spacing::compute(design, &ctx, lib, n_theta)
    } else {
        Spacing::uniform(n_theta)
    };
    // `None` is the whole equal-arc-length path, not a flat density: with
    // adaptive off the cross-section must not redistribute by curvature either.
    let field_v = params.adaptive.then_some(&spacing.v);

    // --- Sweep: one displaced cross-section per angular step. ---
    let rings: Vec<RingSlice> = (0..n_theta)
        .into_par_iter()
        .map(|i| {
            let frac = spacing.theta[i];
            let theta = frac * 360.0;
            let (sin_t, cos_t) = theta.to_radians().sin_cos();
            let m = design.shank.modulation(theta, reference.crest_radius_mm);
            let loop_i = design.profile.sample_spaced(inner_r, n_prof, &m, field_v);
            let u = frac * ctx.circumference_mm;

            let mut verts = Vec::with_capacity(n_prof);
            let mut hi = 0.0f64;
            let mut lo = 0.0f64;

            for p in &loop_i.pts {
                let h = if p.surface && p.weight > 0.0 {
                    let v_norm = p.v_mm / loop_i.surface_len_mm.max(1e-9);
                    let uv = Uv { u, v: v_norm * ctx.band_v_len_mm };
                    design.layers.height(uv, &ctx, lib) * p.weight
                } else {
                    0.0
                };
                hi = hi.max(h);
                lo = lo.min(h);

                let mut r = p.r + h * p.nr;
                let z = p.z + h * p.nz;
                // Never eat into the finger hole. Floored at the base profile
                // where that already sits inside the wall — the side faces meet
                // the bore at the comfort radius, and a bare floor would push
                // their inner corner outward on a ring carrying no relief.
                if p.surface {
                    r = r.max((inner_r + min_wall).min(p.r));
                }
                verts.push(Vec3(
                    (r * cos_t) as f32,
                    (r * sin_t) as f32,
                    z as f32,
                ));
            }
            RingSlice { verts, hi, lo }
        })
        .collect();

    // --- Triangulate the torus grid. ---
    let p = n_prof;
    let mut vertices: Vec<Vec3> = Vec::with_capacity(n_theta * p);
    let mut max_relief = 0.0f64;
    let mut min_relief = 0.0f64;
    for slice in &rings {
        vertices.extend_from_slice(&slice.verts);
        max_relief = max_relief.max(slice.hi);
        min_relief = min_relief.min(slice.lo);
    }

    let idx = |i: usize, j: usize| -> u32 { (i * p + j) as u32 };
    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(n_theta * p * 2);
    for i in 0..n_theta {
        let i1 = (i + 1) % n_theta;
        for j in 0..p {
            let j1 = (j + 1) % p;
            let a = idx(i, j);
            let b = idx(i1, j);
            let c = idx(i1, j1);
            let d = idx(i, j1);
            faces.push([a, b, c]);
            faces.push([a, c, d]);
        }
    }

    let normals = smooth_normals(&vertices, &faces);
    let mesh = Mesh { vertices, normals, faces };

    let bounds = mesh.bounds().unwrap_or_default();
    let bounds_mm = [
        (bounds.1.0 - bounds.0.0) as f64,
        (bounds.1.1 - bounds.0.1) as f64,
        (bounds.1.2 - bounds.0.2) as f64,
    ];
    let volume = mesh.volume_mm3();

    let report = Report {
        validation: mesh.validate(),
        volume_mm3: volume,
        surface_area_mm2: mesh.surface_area_mm2(),
        bounds_mm,
        inner_diameter_mm: design.size.inner_diameter_mm(),
        outer_diameter_mm: bounds_mm[0].max(bounds_mm[1]),
        band_width_mm: bounds_mm[2],
        max_relief_mm: max_relief,
        min_relief_mm: min_relief,
        metals: metal_table(volume),
        build_ms: started.elapsed().as_millis(),
    };

    BuildResult { mesh, report, reference, spacing }
}

struct RingSlice {
    verts: Vec<Vec3>,
    hi: f64,
    lo: f64,
}

/// Area-weighted vertex normals: face normals are accumulated unnormalized, so
/// larger triangles carry proportionally more weight.
fn smooth_normals(vertices: &[Vec3], faces: &[[u32; 3]]) -> Vec<Vec3> {
    let mut acc = vec![[0.0f64; 3]; vertices.len()];
    for f in faces {
        let (Some(a), Some(b), Some(c)) = (
            vertices.get(f[0] as usize),
            vertices.get(f[1] as usize),
            vertices.get(f[2] as usize),
        ) else {
            continue;
        };
        let a = [a.0 as f64, a.1 as f64, a.2 as f64];
        let b = [b.0 as f64, b.1 as f64, b.2 as f64];
        let c = [c.0 as f64, c.1 as f64, c.2 as f64];
        let n = cross(sub(b, a), sub(c, a));
        for &v in f {
            let s = &mut acc[v as usize];
            s[0] += n[0];
            s[1] += n[1];
            s[2] += n[2];
        }
    }
    acc.into_iter()
        .map(|n| {
            let len = norm(n);
            if len > 1e-12 {
                Vec3((n[0] / len) as f32, (n[1] / len) as f32, (n[2] / len) as f32)
            } else {
                Vec3(0.0, 0.0, 1.0)
            }
        })
        .collect()
}

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::TOP_DEG;

    fn draft_build(design: &RingDesign) -> BuildResult {
        build(
            design,
            &AlphaLibrary::builtin(),
            BuildParams { theta_steps: 96, profile_steps: 64, min_wall_mm: MIN_WALL_MM, adaptive: true },
        )
    }

    #[test]
    fn plain_band_is_watertight() {
        let out = draft_build(&RingDesign::default());
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        assert_eq!(out.report.validation.boundary_edges, 0);
        assert_eq!(out.report.validation.non_manifold_edges, 0);
    }

    #[test]
    fn faces_wind_outward() {
        let out = draft_build(&RingDesign::default());
        // A correctly wound closed mesh has positive signed volume; compare the
        // signed sum against the absolute value the report uses.
        let mut signed = 0.0f64;
        for f in &out.mesh.faces {
            let (a, b, c) = out.mesh.triangle(f).unwrap();
            signed += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
        assert!(signed > 0.0, "faces are wound inward (signed volume {signed})");
    }

    #[test]
    fn volume_is_close_to_the_analytic_torus() {
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::profile::ProfileStyle::HalfRound);
        d.profile.comfort_fit_mm = 0.0;
        d.profile.side_draft_deg = 0.0;
        d.profile.edge_round_mm = 0.0;
        let out = draft_build(&d);
        // Half-round band: annulus of width W and thickness T, minus the crown
        // that the dome removes. Bracket generously; this only catches gross
        // scaling or unit errors.
        let r_in = d.inner_radius_mm();
        let w = d.profile.width_mm;
        let t = d.profile.thickness_mm;
        let solid = std::f64::consts::PI * ((r_in + t).powi(2) - r_in.powi(2)) * w;
        assert!(
            out.report.volume_mm3 > solid * 0.4 && out.report.volume_mm3 < solid * 1.05,
            "volume {} out of range for slab {solid}",
            out.report.volume_mm3
        );
    }

    #[test]
    fn every_profile_style_builds_watertight() {
        for &style in crate::profile::ProfileStyle::ALL {
            let mut d = RingDesign::default();
            d.profile.apply_style(style);
            let out = draft_build(&d);
            assert!(out.report.validation.watertight, "{:?}: {:?}", style, out.report.validation);
        }
    }

    #[test]
    fn every_shank_style_builds_watertight() {
        for &kind in crate::profile::ShankKind::ALL {
            let mut d = RingDesign::default();
            d.shank.kind = kind;
            d.shank.amount = 1.0;
            let out = draft_build(&d);
            assert!(out.report.validation.watertight, "{:?}: {:?}", kind, out.report.validation);
        }
    }

    /// A default band carrying a signet table centred on the crest at the top.
    fn signet_design() -> (RingDesign, crate::field::SignetLayer) {
        use crate::field::{Layer, LayerEntry, SignetLayer};
        let mut d = RingDesign::default();
        let s = SignetLayer { v_mm: d.field_context().crest_v_mm, ..SignetLayer::default() };
        d.layers.layers.push(LayerEntry::new("Signet", Layer::Signet(s)));
        (d, s)
    }

    #[test]
    fn scratch_signet_flat_top_proof() {
        use crate::field::wrap_delta;
        let (d, s) = signet_design();
        let inner_r = d.inner_radius_mm();
        let ctx = d.field_context();
        let lib = AlphaLibrary::builtin();
        let (n_theta, n_prof) = (1024usize, 384usize);
        let reference = d.profile.sample(inner_r, n_prof);
        let u0 = ctx.u_of_theta(s.theta_deg);

        // World points of the surface inside the dead-flat part of the table.
        let mut pts: Vec<[f64; 3]> = Vec::new();
        let mut shell_lo = f64::MAX;
        let mut shell_hi = f64::MIN;
        for i in 0..n_theta {
            let frac = i as f64 / n_theta as f64;
            let theta = frac * 360.0;
            let (sin_t, cos_t) = theta.to_radians().sin_cos();
            let m = d.shank.modulation(theta, reference.crest_radius_mm);
            let loop_i = d.profile.sample_mod(inner_r, n_prof, &m);
            let u = frac * ctx.circumference_mm;
            for p in &loop_i.pts {
                if !p.surface || p.weight <= 0.0 {
                    continue;
                }
                let v = p.v_mm / loop_i.surface_len_mm.max(1e-9) * ctx.band_v_len_mm;
                let du = wrap_delta(u - u0, ctx.circumference_mm);
                if s.outline_distance(du, v - s.v_mm) > s.top_flat {
                    continue;
                }
                let h = d.layers.height(Uv { u, v }, &ctx, &lib) * p.weight;
                let r = p.r + h * p.nr;
                // Offset of the displaced point from the bare band, mm.
                shell_lo = shell_lo.min(h * (p.nr * p.nr + p.nz * p.nz).sqrt());
                shell_hi = shell_hi.max(h * (p.nr * p.nr + p.nz * p.nz).sqrt());
                pts.push([r * cos_t, r * sin_t, p.z + h * p.nz]);
            }
        }
        assert!(pts.len() > 500, "too few table samples: {}", pts.len());

        // The table faces +Y at theta = 90, so a true flat is a constant y.
        let ys: Vec<f64> = pts.iter().map(|p| p[1]).collect();
        let y_lo = ys.iter().cloned().fold(f64::MAX, f64::min);
        let y_hi = ys.iter().cloned().fold(f64::MIN, f64::max);
        let xs: Vec<f64> = pts.iter().map(|p| p[0]).collect();
        let zs: Vec<f64> = pts.iter().map(|p| p[2]).collect();
        let span_x = xs.iter().cloned().fold(f64::MIN, f64::max)
            - xs.iter().cloned().fold(f64::MAX, f64::min);
        let span_z = zs.iter().cloned().fold(f64::MIN, f64::max)
            - zs.iter().cloned().fold(f64::MAX, f64::min);

        println!("== signet table, {} samples over the dead-flat region ==", pts.len());
        println!("table extent: {span_x:.3} mm around the ring x {span_z:.3} mm across the band");
        println!("height field over the flat: constant {:.6} mm", s.height_mm);
        println!("world y (the table's own normal direction): {y_lo:.4} .. {y_hi:.4} mm");
        println!("FLATNESS ERROR: {:.4} mm ({:.0} um)", y_hi - y_lo, (y_hi - y_lo) * 1000.0);
        let r_crest = ctx.crest_radius_mm + s.height_mm;
        println!(
            "sag a cylinder of radius {r_crest:.3} mm gives over {span_x:.3} mm of chord: {:.4} mm",
            r_crest - (r_crest * r_crest - (span_x * 0.5).powi(2)).max(0.0).sqrt()
        );

        // The height field itself is dead flat; the mesh is not.
        let mut hs: Vec<f64> = Vec::new();
        for i in 0..64 {
            for j in 0..64 {
                let du = (i as f64 / 63.0 - 0.5) * s.length_mm * s.top_flat;
                let dv = (j as f64 / 63.0 - 0.5) * s.width_mm * s.top_flat;
                if s.outline_distance(du, dv) > s.top_flat {
                    continue;
                }
                hs.push(s.height(Uv { u: u0 + du, v: s.v_mm + dv }, &ctx));
            }
        }
        let h_lo = hs.iter().cloned().fold(f64::MAX, f64::min);
        let h_hi = hs.iter().cloned().fold(f64::MIN, f64::max);
        println!("field height variation over {} grid samples: {:.3e} mm", hs.len(), h_hi - h_lo);
        println!(
            "normal offset from the bare band: {shell_lo:.6} .. {shell_hi:.6} mm (spread {:.3e})",
            shell_hi - shell_lo
        );
        // The field deliberately varies: it compensates the band's curvature so
        // the resulting surface is a plane. A constant field would leave a dome.
        assert!(
            h_hi - h_lo > 1e-6,
            "the height field is constant, so the table just rides the band's curve"
        );
        assert!(
            shell_hi - shell_lo > 1e-6,
            "the table is a uniform offset of the band, which cannot be flat"
        );
        assert!(
            y_hi - y_lo < 0.05,
            "table is {:.4} mm out of flat; a graver needs a true surface",
            y_hi - y_lo
        );
    }

    #[test]
    fn scratch_signet_watertight() {
        let (d, _) = signet_design();
        let out = draft_build(&d);
        let rep = crate::castability::analyze(
            &out.mesh,
            &crate::DraftSettings::default(),
            d.inner_radius_mm(),
        );
        println!(
            "signet design: watertight {} | undercut {} faces {:.3}% | {:?}",
            out.report.validation.watertight,
            rep.undercut,
            rep.undercut_fraction() * 100.0,
            rep.verdict
        );
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
    }

    /// A band carrying a tiled alpha over the crown and milgrain on both edges:
    /// detail concentrated across `v`, which is what adaptive spacing is for.
    fn ornamented_design(lib: &AlphaLibrary) -> RingDesign {
        use crate::field::{Layer, LayerEntry, MilgrainLayer};
        use crate::tiling::TilingLayer;
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::profile::ProfileStyle::DShape);
        let ctx = d.field_context();
        let name = lib.names()[0].clone();
        d.layers
            .layers
            .push(LayerEntry::new("tile", Layer::Tiling(TilingLayer::default_for(name, &ctx))));
        d.layers.layers.push(LayerEntry::new(
            "milgrain",
            Layer::Milgrain(MilgrainLayer { v_mm: 0.55, ..MilgrainLayer::default() }),
        ));
        d
    }

    fn params(theta: usize, prof: usize, adaptive: bool) -> BuildParams {
        BuildParams { theta_steps: theta, profile_steps: prof, min_wall_mm: MIN_WALL_MM, adaptive }
    }

    #[test]
    fn adaptive_spacing_stays_watertight_on_an_ornamented_band() {
        let lib = AlphaLibrary::builtin();
        let d = ornamented_design(&lib);
        for &(t, p) in &[(96usize, 64usize), (192, 96), (384, 144)] {
            let out = build(&d, &lib, params(t, p, true));
            let v = out.report.validation;
            assert!(v.watertight, "{t}x{p}: {v:?}");
            assert_eq!(v.triangle_count, t * p * 2, "{t}x{p} lost triangles");
        }
    }

    /// The bore is a cylinder that needs almost no samples across `v`, and
    /// equal-arc-length spacing spends a third of the budget on it anyway.
    #[test]
    fn adaptive_spacing_moves_the_budget_off_the_bore() {
        let lib = AlphaLibrary::builtin();
        let d = ornamented_design(&lib);
        let count = |adaptive: bool| {
            let p = params(192, 96, adaptive);
            let sp = adaptive.then(|| {
                crate::adaptive::Spacing::compute(&d, &d.field_context(), &lib, 1)
            });
            let s = crate::castability::section_at_spaced(&d, &lib, TOP_DEG, p.profile_steps, sp.as_ref());
            s.points.iter().filter(|q| q.surface).count()
        };
        let (even, detail) = (count(false), count(true));
        println!("surface samples out of 96: equal-arc {even}, by detail {detail}");
        assert!(
            detail > even + 96 / 10,
            "detail-driven spacing gave the surface {detail} of 96, equal-arc gave {even}"
        );
    }

    #[test]
    fn bore_stays_at_the_requested_size() {
        let d = RingDesign::default();
        let out = draft_build(&d);
        let target = d.inner_radius_mm();
        let min_r = out
            .mesh
            .vertices
            .iter()
            .map(|v| ((v.0 as f64).powi(2) + (v.1 as f64).powi(2)).sqrt())
            .fold(f64::MAX, f64::min);
        assert!(
            (min_r - target).abs() < 0.05,
            "bore radius {min_r} drifted from {target}"
        );
    }
}
