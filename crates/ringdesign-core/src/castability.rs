//! Sand-casting feasibility: draft angles, undercuts, and cross-sections.
//!
//! The ring lies flat in the sand. The mould parts on a plane perpendicular to
//! Z and pulls in ±Z: cope upward from everything above the parting plane, drag
//! downward from everything below it.
//!
//! A face releases when its outward normal leans away from the parting plane by
//! at least the minimum draft angle. A face whose normal leans back *toward*
//! the parting plane is an undercut and will lock in the sand.

use serde::{Deserialize, Serialize};

use crate::adaptive::Spacing;
use crate::alpha::AlphaLibrary;
use crate::field::Uv;
use crate::mesh::{Mesh, cross, norm, sub};
use crate::RingDesign;

/// Draft this close to zero is called a wall parallel to the pull, degrees.
const VERTICAL_TOL_DEG: f64 = 0.5;
/// Undercut share of the total area above which nothing will release.
const NOT_CASTABLE_FRACTION: f64 = 0.01;
/// Under-drafted share of the total area tolerated before the verdict drops.
const DRAG_FRACTION: f64 = 0.12;
/// Radial slack when deciding a face belongs to the bore, mm.
const BORE_TOL_MM: f64 = 0.3;
/// Height bands the bore radius is traced over.
const BORE_BINS: usize = 128;
/// Parting heights tried per scan pass. Odd, so the middle is always a candidate.
const PARTING_CANDIDATES: usize = 65;
/// Share of the axial span skipped at each end when measuring wall thickness.
const WALL_EDGE_MARGIN: f64 = 0.02;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DraftSettings {
    /// Height of the parting plane, mm. Ignored while `auto_parting` is set.
    pub parting_z_mm: f64,
    /// Draft below which a wall is called marginal, degrees.
    pub min_draft_deg: f64,
    /// Put the parting plane at the widest silhouette of the ring.
    pub auto_parting: bool,
    /// Thinnest section that will reliably fill, mm.
    pub min_section_mm: f64,
}

impl Default for DraftSettings {
    fn default() -> Self {
        Self {
            parting_z_mm: 0.0,
            min_draft_deg: 3.0,
            auto_parting: true,
            min_section_mm: 0.7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceClass {
    /// Draft at or above the minimum: pulls cleanly.
    Good,
    /// Some draft, but under the minimum: will drag on the sand.
    Marginal,
    /// Parallel to the pull: releases only with a perfect mould.
    Vertical,
    /// Leans back under itself: locks in the sand.
    Undercut,
}

impl FaceClass {
    pub fn label(self) -> &'static str {
        match self {
            FaceClass::Good => "Good draft",
            FaceClass::Marginal => "Marginal",
            FaceClass::Vertical => "Vertical wall",
            FaceClass::Undercut => "Undercut",
        }
    }

    /// Display colour as linear RGB, 0..1.
    pub fn rgb(self) -> [f32; 3] {
        match self {
            FaceClass::Good => [0.32, 0.78, 0.45],
            FaceClass::Marginal => [0.95, 0.76, 0.24],
            FaceClass::Vertical => [0.36, 0.60, 0.92],
            FaceClass::Undercut => [0.93, 0.27, 0.36],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Castable,
    Marginal,
    NotCastable,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Castable => "Castable",
            Verdict::Marginal => "Castable with care",
            Verdict::NotCastable => "Will not release",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CastReport {
    /// Class of every face, parallel to `mesh.faces`.
    pub classes: Vec<FaceClass>,
    pub good: usize,
    pub marginal: usize,
    pub vertical: usize,
    pub undercut: usize,
    pub undercut_area_mm2: f64,
    pub marginal_area_mm2: f64,
    pub total_area_mm2: f64,
    /// Most negative draft found, degrees. Negative means undercut.
    pub worst_draft_deg: f64,
    pub parting_z_mm: f64,
    pub verdict: Verdict,
    /// Plain-language findings for the report panel.
    pub notes: Vec<String>,
}

impl CastReport {
    pub fn undercut_fraction(&self) -> f64 {
        if self.total_area_mm2 <= 0.0 {
            0.0
        } else {
            self.undercut_area_mm2 / self.total_area_mm2
        }
    }
}

/// Signed draft angle of a face, degrees.
///
/// Positive means the normal leans away from the parting plane on the same
/// side as the face, so it pulls. Zero is a wall parallel to the pull.
/// Negative is an undercut.
///
/// `normal` must be a unit outward normal, `centroid_z` the face centre height.
/// Faces on the bore are excluded by the caller, not here.
pub fn draft_angle(normal: [f64; 3], centroid_z: f64, parting_z: f64) -> f64 {
    let len = norm(normal);
    if !len.is_finite() || len <= 1e-12 {
        return 0.0;
    }
    let nz = normal[2] / len;
    let pull = if centroid_z >= parting_z { 1.0 } else { -1.0 };
    (nz * pull).clamp(-1.0, 1.0).asin().to_degrees()
}

/// Class of a face from its signed draft.
fn classify(draft_deg: f64, min_draft_deg: f64) -> FaceClass {
    if !draft_deg.is_finite() || draft_deg.abs() <= VERTICAL_TOL_DEG {
        FaceClass::Vertical
    } else if draft_deg < 0.0 {
        FaceClass::Undercut
    } else if draft_deg < min_draft_deg {
        FaceClass::Marginal
    } else {
        FaceClass::Good
    }
}

/// Height with the least undercut area, from samples of `(nz, centroid_z, area)`.
/// Ties go to the candidate nearest the middle of the span.
fn best_parting_z(samples: &[(f64, f64, f64)], z_lo: f64, z_hi: f64) -> f64 {
    let mid = 0.5 * (z_lo + z_hi);
    if samples.is_empty() || !mid.is_finite() || z_hi - z_lo <= 1e-9 {
        return if mid.is_finite() { mid } else { 0.0 };
    }
    let coarse = scan_parting_z(samples, z_lo, z_hi, mid);
    // Refine within one step of the coarse best, onto the crest line itself.
    let step = (z_hi - z_lo) / (PARTING_CANDIDATES - 1) as f64;
    scan_parting_z(samples, (coarse - step).max(z_lo), (coarse + step).min(z_hi), mid)
}

/// Least-undercut candidate over a height range, breaking ties toward `mid`.
fn scan_parting_z(samples: &[(f64, f64, f64)], z_lo: f64, z_hi: f64, mid: f64) -> f64 {
    let limit = -VERTICAL_TOL_DEG.to_radians().sin();
    let mut best = mid;
    let mut best_area = f64::MAX;
    let mut best_dist = f64::MAX;
    for i in 0..PARTING_CANDIDATES {
        let t = i as f64 / (PARTING_CANDIDATES - 1) as f64;
        let cand = z_lo + (z_hi - z_lo) * t;
        let mut area = 0.0;
        for &(nz, cz, a) in samples {
            let pulled = if cz >= cand { nz } else { -nz };
            if pulled < limit {
                area += a;
            }
        }
        let dist = (cand - mid).abs();
        if area < best_area - 1e-9 || (area <= best_area + 1e-9 && dist < best_dist) {
            best = cand;
            best_area = area;
            best_dist = dist;
        }
    }
    best
}

/// Radius of the innermost inward-facing surface at each height: the bore,
/// carried wherever a comfort-fit dome takes it.
///
/// A fixed radial tolerance around the nominal bore is not enough, because a
/// comfort-fit bore domes outward by up to its own depth toward both edges.
struct BoreTrace {
    z_lo: f64,
    span: f64,
    /// Bore radius per band, or `f64::MIN` where the hole does not reach.
    radius: Vec<f64>,
}

impl BoreTrace {
    /// Trace the hole out from its narrowest band, stopping where the innermost
    /// surface jumps clear of it.
    fn of(mesh: &Mesh) -> Self {
        let empty = Self { z_lo: 0.0, span: 1.0, radius: Vec::new() };
        let Some((lo, hi)) = mesh.bounds() else {
            return empty;
        };
        let (z_lo, z_hi) = (lo.2 as f64, hi.2 as f64);
        if !z_lo.is_finite() || !z_hi.is_finite() {
            return empty;
        }
        let span = (z_hi - z_lo).max(1e-9);

        let mut inner = vec![f64::MAX; BORE_BINS];
        for f in &mesh.faces {
            let (Some(n), Some((a, b, c))) = (mesh.face_normal(f), mesh.triangle(f)) else {
                continue;
            };
            let cx = (a[0] + b[0] + c[0]) / 3.0;
            let cy = (a[1] + b[1] + c[1]) / 3.0;
            let cz = (a[2] + b[2] + c[2]) / 3.0;
            let radius = cx.hypot(cy);
            if radius <= 1e-9 || !cz.is_finite() || n[0] * cx + n[1] * cy >= 0.0 {
                continue;
            }
            inner[bin(cz, z_lo, span)] = inner[bin(cz, z_lo, span)].min(radius);
        }

        let seed = (0..BORE_BINS)
            .filter(|&k| inner[k] < f64::MAX)
            .min_by(|&a, &b| inner[a].total_cmp(&inner[b]));
        let Some(seed) = seed else {
            return empty;
        };

        let mut radius = vec![f64::MIN; BORE_BINS];
        radius[seed] = inner[seed];
        for k in (0..seed).rev() {
            match walk(inner[k], radius[k + 1]) {
                Some(r) => radius[k] = r,
                None => break,
            }
        }
        for k in seed + 1..BORE_BINS {
            match walk(inner[k], radius[k - 1]) {
                Some(r) => radius[k] = r,
                None => break,
            }
        }
        Self { z_lo, span, radius }
    }

    /// Radius up to which an inward-facing face at this height is still bore.
    fn limit(&self, z: f64) -> f64 {
        if self.radius.is_empty() || !z.is_finite() {
            return f64::MIN;
        }
        let r = self.radius[bin(z, self.z_lo, self.span)];
        if r == f64::MIN { f64::MIN } else { r + BORE_TOL_MM }
    }
}

/// Band index of a height within a span.
fn bin(z: f64, z_lo: f64, span: f64) -> usize {
    let t = ((z - z_lo) / span * (BORE_BINS - 1) as f64).max(0.0);
    (t as usize).min(BORE_BINS - 1)
}

/// Next bore radius along the trace, or `None` where it breaks away.
fn walk(inner: f64, prev: f64) -> Option<f64> {
    if inner == f64::MAX {
        return Some(prev);
    }
    ((inner - prev).abs() <= BORE_TOL_MM).then_some(inner)
}

/// Parting height that maximizes released area: the widest silhouette.
///
/// The bore is left out of the scan: a through hole never locks, so it must not
/// pull the plane off the silhouette.
pub fn suggest_parting_z(mesh: &Mesh) -> f64 {
    let bore = BoreTrace::of(mesh);
    let mut samples: Vec<(f64, f64, f64)> = Vec::with_capacity(mesh.faces.len());
    let mut z_lo = f64::MAX;
    let mut z_hi = f64::MIN;
    for f in &mesh.faces {
        let (Some(n), Some((a, b, c))) = (mesh.face_normal(f), mesh.triangle(f)) else {
            continue;
        };
        let area = norm(cross(sub(b, a), sub(c, a))) * 0.5;
        if !area.is_finite() {
            continue;
        }
        z_lo = z_lo.min(a[2].min(b[2]).min(c[2]));
        z_hi = z_hi.max(a[2].max(b[2]).max(c[2]));
        let cx = (a[0] + b[0] + c[0]) / 3.0;
        let cy = (a[1] + b[1] + c[1]) / 3.0;
        let cz = (a[2] + b[2] + c[2]) / 3.0;
        let radius = cx.hypot(cy);
        let inward = radius > 1e-9 && (n[0] * cx + n[1] * cy) / radius < 0.0;
        if !(inward && radius <= bore.limit(cz)) {
            samples.push((n[2], cz, area));
        }
    }
    if samples.is_empty() {
        return 0.0;
    }
    best_parting_z(&samples, z_lo, z_hi)
}

/// Classify every face of the mesh.
///
/// `bore_radius_mm` is the nominal finger-hole radius. Faces facing inward on
/// the hole — that radius, or wherever a comfort-fit dome carries it — are the
/// bore, which a jeweller reams or which casts as a through hole, so they are
/// reported separately rather than as undercuts.
pub fn analyze(mesh: &Mesh, settings: &DraftSettings, bore_radius_mm: f64) -> CastReport {
    let bore = BoreTrace::of(mesh);
    let parting_z = if settings.auto_parting {
        suggest_parting_z(mesh)
    } else if settings.parting_z_mm.is_finite() {
        settings.parting_z_mm
    } else {
        0.0
    };
    let min_draft = settings.min_draft_deg.max(0.0);
    let bore_limit = bore_radius_mm.max(0.0) + BORE_TOL_MM;

    let mut classes = Vec::with_capacity(mesh.faces.len());
    let (mut good, mut marginal, mut vertical, mut undercut) = (0usize, 0usize, 0usize, 0usize);
    let mut undercut_area = 0.0;
    let mut marginal_area = 0.0;
    let mut vertical_outer_area = 0.0;
    let mut total_area = 0.0;
    let mut worst = f64::MAX;
    let mut bore_faces = 0usize;
    let mut bore_drafted = 0usize;
    let mut bore_area = 0.0;
    let mut bore_best_draft = 0.0f64;

    for f in &mesh.faces {
        let (Some(n), Some((a, b, c))) = (mesh.face_normal(f), mesh.triangle(f)) else {
            classes.push(FaceClass::Good);
            good += 1;
            continue;
        };
        let area = norm(cross(sub(b, a), sub(c, a))) * 0.5;
        let area = if area.is_finite() { area } else { 0.0 };
        let cx = (a[0] + b[0] + c[0]) / 3.0;
        let cy = (a[1] + b[1] + c[1]) / 3.0;
        let cz = (a[2] + b[2] + c[2]) / 3.0;
        let radius = cx.hypot(cy);
        let inward = radius > 1e-9 && (n[0] * cx + n[1] * cy) / radius < 0.0;
        let draft = draft_angle(n, cz, parting_z);

        let is_bore = inward && radius <= bore_limit.max(bore.limit(cz));
        let class = if is_bore {
            // The hole is classed no worse than a vertical wall.
            bore_faces += 1;
            bore_area += area;
            if draft > VERTICAL_TOL_DEG {
                bore_drafted += 1;
                bore_best_draft = bore_best_draft.max(draft);
            }
            if draft >= min_draft { FaceClass::Good } else { FaceClass::Vertical }
        } else {
            worst = worst.min(draft);
            classify(draft, min_draft)
        };

        total_area += area;
        match class {
            FaceClass::Good => good += 1,
            FaceClass::Marginal => {
                marginal += 1;
                marginal_area += area;
            }
            FaceClass::Vertical => {
                vertical += 1;
                if !is_bore {
                    vertical_outer_area += area;
                }
            }
            FaceClass::Undercut => {
                undercut += 1;
                undercut_area += area;
            }
        }
        classes.push(class);
    }
    let worst_draft_deg = if worst == f64::MAX { 0.0 } else { worst };

    let frac = |a: f64| if total_area > 0.0 { a / total_area } else { 0.0 };
    let undercut_frac = frac(undercut_area);
    let marginal_frac = frac(marginal_area);
    let vertical_frac = frac(vertical_outer_area);
    // Everything outside the bore that is under the minimum draft drags.
    let drag_frac = marginal_frac + vertical_frac;
    let verdict = if undercut_frac > NOT_CASTABLE_FRACTION {
        Verdict::NotCastable
    } else if undercut_area > 0.0 || drag_frac > DRAG_FRACTION {
        Verdict::Marginal
    } else {
        Verdict::Castable
    };

    let mut notes = Vec::new();
    notes.push(format!(
        "Parting plane at z = {parting_z:+.2} mm{}: the cope pulls +Z off everything above it, the drag -Z off everything below.",
        if settings.auto_parting { ", chosen automatically" } else { "" }
    ));

    if undercut > 0 {
        notes.push(format!(
            "{undercut} faces undercut, {:.1}% of the surface ({undercut_area:.1} mm2). The worst leans {:.1} deg back under itself. Cut the relief height until its walls slope at least {min_draft:.0} deg, or move that pattern onto the side faces, which pull straight out.",
            undercut_frac * 100.0,
            -worst_draft_deg
        ));
    } else {
        notes.push(format!(
            "No undercuts: all {:.0} mm2 of surface clears a two-part pull at this parting height.",
            total_area
        ));
    }

    if marginal_frac > 0.005 {
        notes.push(format!(
            "{:.1}% of the surface ({marginal_area:.1} mm2) carries less than {min_draft:.1} deg of draft and will drag on the sand. Raise the crown or add side draft to steepen it.",
            marginal_frac * 100.0
        ));
    }

    if vertical_frac > 0.005 {
        notes.push(format!(
            "{:.1}% of the surface stands parallel to the pull, the crest line where the dome is tangent to the parting plane. It releases from a clean mould but will not take deep relief.",
            vertical_frac * 100.0
        ));
    }

    if bore_faces > 0 {
        let drafted = bore_drafted as f64 / bore_faces as f64;
        if drafted > 0.25 {
            notes.push(format!(
                "The comfort-fit bore widens toward both edges: {:.0}% of the finger hole carries up to {bore_best_draft:.1} deg of draft, so it releases on both halves.",
                drafted * 100.0
            ));
        } else {
            notes.push(format!(
                "The finger hole ({:.0}% of the surface) is a straight through-hole with no draft. It cores in the sand or gets reamed at the bench, so it is a vertical wall, never an undercut.",
                frac(bore_area) * 100.0
            ));
        }
    }
    notes.truncate(6);

    CastReport {
        classes,
        good,
        marginal,
        vertical,
        undercut,
        undercut_area_mm2: undercut_area,
        marginal_area_mm2: marginal_area,
        total_area_mm2: total_area,
        worst_draft_deg,
        parting_z_mm: parting_z,
        verdict,
        notes,
    }
}

// --- Cross-section ---------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct SectionPoint {
    pub r: f64,
    pub z: f64,
    /// Outward 2D normal in the (r, z) plane.
    pub nr: f64,
    pub nz: f64,
    /// Signed draft of this segment, degrees.
    pub draft_deg: f64,
    pub class: FaceClass,
    /// Whether this point lies on the displaceable outer surface.
    pub surface: bool,
}

impl Default for FaceClass {
    fn default() -> Self {
        FaceClass::Good
    }
}

/// A radial slice through the ring at one angle, with per-segment draft.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Section {
    pub theta_deg: f64,
    pub points: Vec<SectionPoint>,
    pub parting_z_mm: f64,
    pub min_r: f64,
    pub max_r: f64,
    pub min_z: f64,
    pub max_z: f64,
    /// Thinnest radial wall found in this slice, mm.
    pub min_wall_mm: f64,
    pub undercut_count: usize,
}

/// Extract the displaced cross-section at a ring angle.
///
/// This evaluates the same profile and height field the mesh build uses, so the
/// section view and the solid always agree. It probes the field to recover the
/// build's sample spacing; a caller holding a [`Spacing`] already should use
/// [`section_at_spaced`] and skip that.
pub fn section_at(
    design: &RingDesign,
    lib: &AlphaLibrary,
    theta_deg: f64,
    steps: usize,
) -> Section {
    let spacing = design
        .build
        .adaptive
        .then(|| Spacing::compute(design, &design.field_context(), lib, 1));
    section_at_spaced(design, lib, theta_deg, steps, spacing.as_ref())
}

/// [`section_at`] against a spacing the caller already computed.
///
/// The vertices must land where the mesh's do, or the section view reports a
/// shape the solid does not have.
pub fn section_at_spaced(
    design: &RingDesign,
    lib: &AlphaLibrary,
    theta_deg: f64,
    steps: usize,
    spacing: Option<&Spacing>,
) -> Section {
    let n = steps.clamp(24, 4096);
    let inner_r = design.inner_radius_mm();
    let reference = design.reference_loop();
    let ctx = design.field_context();
    let m = design.shank.modulation(theta_deg, inner_r, reference.crest_radius_mm);
    let loop_i = design.profile.sample_spaced(inner_r, n, &m, spacing.map(|s| &s.v));
    if loop_i.len() < 3 {
        return Section { theta_deg, ..Default::default() };
    }
    let u = ctx.u_of_theta(theta_deg);
    let min_wall = design.build.min_wall_mm.max(0.05);
    let surface_len = loop_i.surface_len_mm.max(1e-9);

    // --- Displace, exactly as mesh::build does for one angular step. ---
    let mut points: Vec<SectionPoint> = Vec::with_capacity(loop_i.len());
    for p in &loop_i.pts {
        let mut h = if p.surface && p.weight > 0.0 {
            let v = p.v_mm / surface_len * ctx.band_v_len_mm;
            design.layers.height(Uv { u, v }, &ctx, lib) * p.weight
        } else {
            0.0
        };
        if !h.is_finite() {
            h = 0.0;
        }
        let mut r = p.r + h * p.nr;
        let z = p.z + h * p.nz;
        // Identical to the clamp in `mesh::build`, including the base-profile
        // floor, or the section reports a shape the solid does not have.
        if p.surface {
            r = r.max((inner_r + min_wall).min(p.r));
        }
        points.push(SectionPoint { r, z, surface: p.surface, ..Default::default() });
    }

    // --- Normals of the displaced loop; the profile's own no longer apply. ---
    let count = points.len();
    for i in 0..count {
        let prev = points[(i + count - 1) % count];
        let next = points[(i + 1) % count];
        let dr = next.r - prev.r;
        let dz = next.z - prev.z;
        let len = (dr * dr + dz * dz).sqrt().max(1e-12);
        points[i].nr = dz / len;
        points[i].nz = -dr / len;
    }

    let mut min_r = f64::MAX;
    let mut max_r = f64::MIN;
    let mut min_z = f64::MAX;
    let mut max_z = f64::MIN;
    for p in &points {
        min_r = min_r.min(p.r);
        max_r = max_r.max(p.r);
        min_z = min_z.min(p.z);
        max_z = max_z.max(p.z);
    }

    let parting_z_mm = if design.draft.auto_parting {
        // Weight every segment by the area it sweeps, as the mesh scan does.
        let mut samples: Vec<(f64, f64, f64)> = Vec::with_capacity(count);
        for i in 0..count {
            let a = points[i];
            let b = points[(i + 1) % count];
            let (dr, dz) = (b.r - a.r, b.z - a.z);
            let len = (dr * dr + dz * dz).sqrt();
            if len <= 1e-12 {
                continue;
            }
            let area = std::f64::consts::TAU * ((a.r + b.r) * 0.5).max(0.0) * len;
            samples.push((-dr / len, (a.z + b.z) * 0.5, area));
        }
        best_parting_z(&samples, min_z, max_z)
    } else if design.draft.parting_z_mm.is_finite() {
        design.draft.parting_z_mm
    } else {
        0.0
    };

    let min_draft = design.draft.min_draft_deg.max(0.0);
    let mut undercut_count = 0;
    for p in points.iter_mut() {
        // The swept 3D normal is (nr cos, nr sin, nz); this one shares its z.
        p.draft_deg = draft_angle([p.nr, 0.0, p.nz], p.z, parting_z_mm);
        p.class = if p.surface {
            classify(p.draft_deg, min_draft)
        } else if p.draft_deg >= min_draft {
            FaceClass::Good
        } else {
            FaceClass::Vertical
        };
        if p.class == FaceClass::Undercut {
            undercut_count += 1;
        }
    }

    Section {
        theta_deg,
        min_wall_mm: thinnest_wall(&points, inner_r, min_z, max_z),
        points,
        parting_z_mm,
        min_r,
        max_r,
        min_z,
        max_z,
        undercut_count,
    }
}

/// Thinnest radial distance from the displaced outer surface to the bore at the
/// same height. The closing corners are skipped: there the section pinches to
/// zero by construction.
fn thinnest_wall(points: &[SectionPoint], inner_r: f64, z_lo: f64, z_hi: f64) -> f64 {
    let surface: Vec<(f64, f64)> =
        points.iter().filter(|p| p.surface).map(|p| (p.z, p.r)).collect();
    let margin = (z_hi - z_lo).max(0.0) * WALL_EDGE_MARGIN;
    let mut wall = f64::MAX;
    for b in points.iter().filter(|p| !p.surface) {
        if b.z < z_lo + margin || b.z > z_hi - margin {
            continue;
        }
        let mut outer = f64::MIN;
        for w in surface.windows(2) {
            let ((z0, r0), (z1, r1)) = (w[0], w[1]);
            if b.z < z0.min(z1) || b.z > z0.max(z1) {
                continue;
            }
            let t = if (z1 - z0).abs() > 1e-12 { (b.z - z0) / (z1 - z0) } else { 0.0 };
            outer = outer.max(r0 + (r1 - r0) * t);
        }
        if outer > f64::MIN {
            wall = wall.min(outer - b.r);
        }
    }
    if wall < f64::MAX {
        return wall;
    }
    let min_surface_r = surface.iter().map(|&(_, r)| r).fold(f64::MAX, f64::min);
    if min_surface_r < f64::MAX { min_surface_r - inner_r } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Layer, LayerEntry, SeatPadLayer};
    use crate::mesh::{BuildParams, BuildResult};
    use crate::profile::TOP_DEG;

    const STEPS: BuildParams = BuildParams { theta_steps: 128, profile_steps: 96, min_wall_mm: 0.5, adaptive: true, refine: None };

    fn built(design: &RingDesign) -> BuildResult {
        crate::mesh::build(design, &AlphaLibrary::default(), STEPS)
    }

    /// Relief on a face square to the pull moves along the pull, and the walls
    /// it raises are parallel to it, so nothing can lean back under the mould.
    /// The same relief on the crest is a wall on a wall.
    #[test]
    fn relief_holds_on_a_squared_side_face_where_it_ruins_the_crest() {
        let lib = AlphaLibrary::builtin();
        let steps = BuildParams { theta_steps: 384, profile_steps: 160, min_wall_mm: 0.5, adaptive: true, refine: None };
        let mut base = RingDesign::default();
        base.profile.apply_style(crate::ProfileStyle::Flat);
        base.profile.flatten_sides();
        let ctx = base.field_context();

        let mut side = crate::tiling::TilingLayer::default_for("Rope", &ctx);
        assert!(
            side.fit_to_side_faces(&ctx, crate::field::SIDE_FACE_MIN_DRAFT_DEG),
            "squared sides should expose a face to fit to"
        );
        side.height_mm = 0.5;

        // Same tiles, same height, moved onto the crest.
        let mut crest = side.clone();
        crest.mirror_v = false;
        crest.v_center_mm = ctx.crest_v_mm;

        let run = |t: &crate::tiling::TilingLayer| {
            let mut d = base.clone();
            d.layers.layers.push(LayerEntry::new("orn", Layer::Tiling(t.clone())));
            let b = crate::mesh::build(&d, &lib, steps);
            assert!(b.report.max_relief_mm > 0.4, "the relief never reached the mesh");
            analyze(&b.mesh, &d.draft, d.inner_radius_mm())
        };

        let on_side = run(&side);
        let on_crest = run(&crest);
        assert_eq!(
            on_side.undercut, 0,
            "{} faces undercut on a squared side face, worst {:+.2} deg",
            on_side.undercut, on_side.worst_draft_deg
        );
        assert!(
            on_crest.undercut_fraction() > 0.005,
            "the crest control did not undercut, so the comparison proves nothing"
        );
    }

    /// A flat-topped boss sitting to one side of the crest, so its near wall
    /// leans back under the +Z pull.
    fn undercutting_design() -> RingDesign {
        let mut d = RingDesign {
            build: STEPS,
            draft: DraftSettings { auto_parting: false, parting_z_mm: 0.0, ..Default::default() },
            ..Default::default()
        };
        let ctx = d.field_context();
        d.layers.layers.push(LayerEntry::new(
            "boss",
            Layer::SeatPad(SeatPadLayer {
                theta_deg: TOP_DEG,
                v_mm: ctx.crest_v_mm + ctx.band_v_len_mm * 0.22,
                diameter_mm: 4.0,
                height_mm: 1.6,
                crown: 0.0,
                blend_mm: 0.0,
            }),
        ));
        d
    }

    #[test]
    fn draft_flips_sign_across_the_parting_plane() {
        // Above the plane the cope pulls +Z.
        assert!((draft_angle([0.0, 0.0, 1.0], 1.0, 0.0) - 90.0).abs() < 1e-9);
        assert!((draft_angle([0.0, 0.0, -1.0], 1.0, 0.0) + 90.0).abs() < 1e-9);
        // Below it the drag pulls -Z, so the same normals swap sign.
        assert!((draft_angle([0.0, 0.0, -1.0], -1.0, 0.0) - 90.0).abs() < 1e-9);
        assert!((draft_angle([0.0, 0.0, 1.0], -1.0, 0.0) + 90.0).abs() < 1e-9);
        // A wall parallel to the pull has no draft on either side.
        assert!(draft_angle([1.0, 0.0, 0.0], 5.0, 0.0).abs() < 1e-9);
        assert!(draft_angle([1.0, 0.0, 0.0], -5.0, 0.0).abs() < 1e-9);
        // The plane itself moves with the setting.
        assert!(draft_angle([0.0, 0.0, 1.0], 1.0, 4.0) < 0.0);
    }

    #[test]
    fn draft_is_the_angle_off_the_parting_plane() {
        let a = 20f64.to_radians();
        let n = [a.cos(), 0.0, a.sin()];
        assert!((draft_angle(n, 1.0, 0.0) - 20.0).abs() < 1e-9);
        // Length is normalized out, and a degenerate normal is not a panic.
        assert!((draft_angle([2.0 * a.cos(), 0.0, 2.0 * a.sin()], 1.0, 0.0) - 20.0).abs() < 1e-9);
        assert_eq!(draft_angle([0.0, 0.0, 0.0], 1.0, 0.0), 0.0);
        assert_eq!(draft_angle([f64::NAN, 0.0, 1.0], 1.0, 0.0), 0.0);
    }

    #[test]
    fn classes_cover_every_face_and_counts_match() {
        let d = RingDesign::default();
        let out = built(&d);
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert_eq!(rep.classes.len(), out.mesh.faces.len());
        assert_eq!(rep.good + rep.marginal + rep.vertical + rep.undercut, rep.classes.len());
        assert_eq!(rep.undercut, rep.classes.iter().filter(|c| **c == FaceClass::Undercut).count());
        assert!(rep.total_area_mm2 > 0.0);
        assert!(rep.undercut_area_mm2 + rep.marginal_area_mm2 <= rep.total_area_mm2 + 1e-6);
    }

    #[test]
    fn plain_domed_band_is_castable() {
        let d = RingDesign::default();
        let out = built(&d);
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert_eq!(rep.undercut, 0, "{:?}", rep.notes);
        assert_eq!(rep.undercut_area_mm2, 0.0);
        assert_eq!(rep.undercut_fraction(), 0.0);
        assert_eq!(rep.verdict, Verdict::Castable, "{:?}", rep.notes);
        assert!((2..=6).contains(&rep.notes.len()), "{:?}", rep.notes);
    }

    #[test]
    fn every_profile_style_is_free_of_undercuts() {
        for &style in crate::profile::ProfileStyle::ALL {
            let mut d = RingDesign::default();
            d.profile.apply_style(style);
            let out = built(&d);
            let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
            assert_eq!(rep.undercut, 0, "{:?} undercuts: {:?}", style, rep.notes);
        }
    }

    #[test]
    fn a_straight_walled_boss_undercuts() {
        let d = undercutting_design();
        let out = built(&d);
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert!(rep.undercut > 0, "boss did not undercut: {:?}", rep.notes);
        assert!(rep.undercut_area_mm2 > 0.0);
        assert!(rep.worst_draft_deg < -10.0, "worst draft {}", rep.worst_draft_deg);
        assert_ne!(rep.verdict, Verdict::Castable);
        assert!(rep.notes.iter().any(|n| n.contains("undercut")), "{:?}", rep.notes);
    }

    #[test]
    fn the_bore_is_never_an_undercut() {
        // Parting well off centre puts the whole lower bore against its pull.
        let mut d = RingDesign::default();
        d.draft.auto_parting = false;
        d.draft.parting_z_mm = 2.0;
        let out = built(&d);
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        let inner = d.inner_radius_mm();
        for (f, class) in out.mesh.faces.iter().zip(&rep.classes) {
            let Some((a, b, c)) = out.mesh.triangle(f) else { continue };
            let cx = (a[0] + b[0] + c[0]) / 3.0;
            let cy = (a[1] + b[1] + c[1]) / 3.0;
            if cx.hypot(cy) <= inner + 0.1 {
                assert_ne!(*class, FaceClass::Undercut, "bore face reported as an undercut");
            }
        }
        assert!(rep.notes.iter().any(|n| n.contains("bore") || n.contains("finger hole")));
    }

    #[test]
    fn auto_parting_lands_on_the_crest_of_a_symmetric_band() {
        let d = RingDesign::default();
        let out = built(&d);
        let z = suggest_parting_z(&out.mesh);
        assert!(z.abs() < 0.2, "parting plane drifted off the crest: {z}");
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert_eq!(rep.parting_z_mm, z);
    }

    #[test]
    fn a_fixed_parting_height_is_reported_verbatim() {
        let mut d = RingDesign::default();
        d.draft.auto_parting = false;
        d.draft.parting_z_mm = 1.25;
        let out = built(&d);
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert_eq!(rep.parting_z_mm, 1.25);
        // Pulling from the wrong height locks the band that faces the plane.
        assert!(rep.undercut > 0);
    }

    #[test]
    fn section_brackets_the_band() {
        let d = RingDesign::default();
        let s = section_at(&d, &AlphaLibrary::default(), TOP_DEG, 128);
        assert_eq!(s.points.len(), 128);
        assert!(s.points.iter().any(|p| p.surface) && s.points.iter().any(|p| !p.surface));

        let inner = d.inner_radius_mm();
        let hw = d.profile.width_mm * 0.5;
        assert!((s.min_r - inner).abs() < 0.05, "bore radius {}", s.min_r);
        assert!(
            (s.max_r - (inner + d.profile.thickness_mm)).abs() < 0.05,
            "crest radius {}",
            s.max_r
        );
        assert!(s.min_z >= -hw - 1e-6 && s.min_z < -hw * 0.9, "min z {}", s.min_z);
        assert!(s.max_z <= hw + 1e-6 && s.max_z > hw * 0.9, "max z {}", s.max_z);
        assert!(s.min_wall_mm > 0.1 && s.min_wall_mm < d.profile.thickness_mm);
    }

    #[test]
    fn a_plain_section_has_no_undercuts_and_drafts_away_from_the_crest() {
        let d = RingDesign::default();
        let s = section_at(&d, &AlphaLibrary::default(), TOP_DEG, 192);
        assert_eq!(s.undercut_count, 0);
        assert!(s.points.iter().all(|p| p.class != FaceClass::Undercut));
        // Outward normals on the surface, inward on the bore.
        let crest = s.points.iter().filter(|p| p.surface).max_by(|a, b| a.r.total_cmp(&b.r)).unwrap();
        assert!(crest.nr > 0.5);
        // Both flanks of the dome draft away from the plane at the crest.
        let hw = d.profile.width_mm * 0.5;
        for band in [(0.25 * hw, 0.7 * hw), (-0.7 * hw, -0.25 * hw)] {
            let flank = s
                .points
                .iter()
                .filter(|p| p.surface && p.z > band.0 && p.z < band.1)
                .min_by(|a, b| a.draft_deg.total_cmp(&b.draft_deg))
                .unwrap();
            assert!(flank.draft_deg > 3.0, "flank draft {} at z {}", flank.draft_deg, flank.z);
            assert_eq!(flank.class, FaceClass::Good);
        }
    }

    #[test]
    fn the_section_matches_the_mesh_it_slices() {
        let d = undercutting_design();
        let out = built(&d);
        let s = section_at(&d, &AlphaLibrary::default(), 0.0, STEPS.profile_steps);
        assert_eq!(s.points.len(), STEPS.profile_steps);
        // theta = 0 is the first swept slice of the mesh.
        for (i, p) in s.points.iter().enumerate() {
            let v = out.mesh.vertices[i];
            let r = ((v.0 as f64).powi(2) + (v.1 as f64).powi(2)).sqrt();
            assert!((r - p.r).abs() < 1e-3, "point {i}: r {r} vs {}", p.r);
            assert!((v.2 as f64 - p.z).abs() < 1e-3, "point {i}: z {} vs {}", v.2, p.z);
        }
    }

    #[test]
    fn a_boss_wall_shows_up_in_the_section() {
        let d = undercutting_design();
        let lib = AlphaLibrary::default();
        let plain = section_at(&RingDesign::default(), &lib, TOP_DEG, 192);
        let s = section_at(&d, &lib, TOP_DEG, 192);
        assert!(s.max_r > plain.max_r + 1.0, "boss did not raise the section");
        assert!(s.undercut_count > 0, "boss wall did not read as an undercut");
        assert!(s.min_wall_mm > 0.0);
    }

    #[test]
    fn a_section_survives_every_shank_style_and_a_wrapped_angle() {
        for &kind in crate::profile::ShankKind::ALL {
            let mut d = RingDesign::default();
            d.shank.kind = kind;
            d.shank.amount = 1.0;
            for theta in [-450.0, 0.0, 37.5, TOP_DEG, 720.0] {
                let s = section_at(&d, &AlphaLibrary::default(), theta, 64);
                assert_eq!(s.points.len(), 64, "{kind:?} at {theta}");
                assert!(s.min_r.is_finite() && s.max_r > s.min_r, "{kind:?} at {theta}");
                assert!(s.points.iter().all(|p| p.r.is_finite() && p.z.is_finite()));
                assert!(s.min_wall_mm.is_finite());
            }
        }
    }

    #[test]
    fn probe_comfort_bore() {
        let mut d = RingDesign::default();
        d.build = STEPS;
        d.profile.apply_style(crate::profile::ProfileStyle::Flat);
        d.profile.comfort_fit_mm = 0.8;
        d.draft.auto_parting = false;
        d.draft.parting_z_mm = 2.0;
        let out = built(&d);
        println!("edge_t {}", d.profile.edge_thickness_mm());
        let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        let inner = d.inner_radius_mm();
        let mut bad = 0;
        let mut worst_r: f64 = 0.0;
        for (f, class) in out.mesh.faces.iter().zip(&rep.classes) {
            let Some((a, b, c)) = out.mesh.triangle(f) else { continue };
            let cx = (a[0] + b[0] + c[0]) / 3.0;
            let cy = (a[1] + b[1] + c[1]) / 3.0;
            let r = cx.hypot(cy);
            if r <= inner + 0.9 && *class == FaceClass::Undercut {
                bad += 1;
                worst_r = worst_r.max(r - inner);
            }
        }
        println!("comfort bore undercut faces: {bad}, worst dr {worst_r}");
        assert_eq!(bad, 0, "comfort-fit bore reported as undercut");
    }

    #[test]
    fn probe_section_vs_mesh_offaxis() {
        use crate::tiling::TilingLayer;
        for kind in crate::profile::ShankKind::ALL {
            let mut d = RingDesign { build: STEPS, ..Default::default() };
            d.shank.kind = *kind;
            d.shank.amount = 1.0;
            let lib = AlphaLibrary::builtin();
            let name = lib.names()[0].clone();
            let ctx = d.field_context();
            d.layers.layers.push(LayerEntry::new(
                "tile",
                Layer::Tiling(TilingLayer::default_for(name, &ctx)),
            ));
            let out = crate::mesh::build(&d, &lib, STEPS);
            for i in [0usize, 17, 64, 111] {
                // Adaptive spacing means ring `i` is not at i/n of a turn.
                let theta = out.spacing.theta[i] * 360.0;
                let s = section_at_spaced(
                    &d,
                    &lib,
                    theta,
                    STEPS.profile_steps,
                    Some(&out.spacing),
                );
                let mut worst = 0.0f64;
                for (j, p) in s.points.iter().enumerate() {
                    let v = out.mesh.vertices[i * STEPS.profile_steps + j];
                    let r = ((v.0 as f64).powi(2) + (v.1 as f64).powi(2)).sqrt();
                    worst = worst.max((r - p.r).abs()).max((v.2 as f64 - p.z).abs());
                }
                println!("{kind:?} i={i} theta={theta} worst dev {worst}");
                assert!(worst < 1e-3, "{kind:?} at theta {theta}: dev {worst}");
            }
        }
    }

    #[test]
    fn probe_hostile() {
        let lib = AlphaLibrary::default();
        let mut cases: Vec<(&str, RingDesign)> = Vec::new();
        let mut d = RingDesign::default();
        d.size = crate::RingSize(0.0);
        cases.push(("size 0", d));
        let mut d = RingDesign::default();
        d.size = crate::RingSize(-100.0);
        cases.push(("negative size", d));
        let mut d = RingDesign::default();
        d.profile.width_mm = 0.0;
        d.profile.thickness_mm = 0.0;
        cases.push(("zero section", d));
        let mut d = RingDesign::default();
        d.profile.width_mm = -5.0;
        d.profile.thickness_mm = -2.0;
        d.profile.crown_mm = -1.0;
        cases.push(("negative section", d));
        let mut d = RingDesign::default();
        d.profile.width_mm = f64::NAN;
        d.profile.thickness_mm = f64::NAN;
        cases.push(("nan section", d));
        let mut d = RingDesign::default();
        d.draft.min_draft_deg = f64::NAN;
        d.draft.parting_z_mm = f64::NAN;
        d.draft.auto_parting = false;
        cases.push(("nan draft", d));
        let mut d = RingDesign::default();
        d.build.min_wall_mm = f64::NAN;
        cases.push(("nan min wall", d));
        let mut d = RingDesign::default();
        d.profile.comfort_fit_mm = f64::INFINITY;
        d.profile.side_draft_deg = f64::NAN;
        cases.push(("inf comfort", d));

        for (name, d) in cases {
            for theta in [0.0, f64::NAN, f64::INFINITY, 1e18] {
                for steps in [0usize, 1, 24, 300] {
                    let s = section_at(&d, &lib, theta, steps);
                    println!(
                        "{name} theta {theta} steps {steps}: pts {} minr {} wall {} part {}",
                        s.points.len(), s.min_r, s.min_wall_mm, s.parting_z_mm
                    );
                }
            }
            let out = crate::mesh::build(&d, &lib, STEPS);
            let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
            println!("{name}: verdict {:?} total {} worst {}", rep.verdict, rep.total_area_mm2, rep.worst_draft_deg);
        }
        // Degenerate mesh: everything collapsed onto the axis.
        let mesh = Mesh {
            vertices: vec![crate::mesh::Vec3(0.0, 0.0, 0.0); 3],
            normals: vec![],
            faces: vec![[0, 1, 2], [0, 0, 0], [9, 9, 9]],
        };
        let rep = analyze(&mesh, &DraftSettings::default(), 0.0);
        println!("degenerate: {:?} {}", rep.verdict, rep.classes.len());
        let nanmesh = Mesh {
            vertices: vec![crate::mesh::Vec3(f32::NAN, 0.0, 0.0); 3],
            normals: vec![],
            faces: vec![[0, 1, 2]],
        };
        let rep = analyze(&nanmesh, &DraftSettings::default(), f64::NAN);
        println!("nan mesh: {:?} {}", rep.verdict, rep.parting_z_mm);
    }

    #[test]
    fn probe_bias() {
        for bias in [-0.9, -0.5, 0.0, 0.5, 0.9] {
            for comfort in [0.0, 0.25, 0.9] {
                let mut d = RingDesign { build: STEPS, ..Default::default() };
                d.profile.thickness_mm = 3.0;
                d.profile.apply_style(crate::profile::ProfileStyle::DShape);
                d.profile.crest_bias = bias;
                d.profile.comfort_fit_mm = comfort;
                let out = built(&d);
                let rep = analyze(&out.mesh, &d.draft, d.inner_radius_mm());
                let crest_z = {
                    let l = d.reference_loop();
                    l.pts.iter().filter(|p| p.surface).max_by(|a, b| a.r.total_cmp(&b.r)).unwrap().z
                };
                println!(
                    "bias {bias} comfort {comfort}: parting {:.3} crest_z {:.3} undercut {} ({:.2}%) verdict {:?}",
                    rep.parting_z_mm, crest_z, rep.undercut, rep.undercut_fraction() * 100.0, rep.verdict
                );
            }
        }
    }

    #[test]
    fn an_empty_mesh_reports_nothing_rather_than_panicking() {
        let mesh = Mesh::default();
        let rep = analyze(&mesh, &DraftSettings::default(), 8.65);
        assert!(rep.classes.is_empty());
        assert_eq!(rep.total_area_mm2, 0.0);
        assert_eq!(rep.undercut_fraction(), 0.0);
        assert_eq!(rep.verdict, Verdict::Castable);
        assert_eq!(suggest_parting_z(&mesh), 0.0);
    }
}
