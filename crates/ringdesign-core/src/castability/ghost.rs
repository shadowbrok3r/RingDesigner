//! A carried part's ghost read by the verdict's face rule: every face classed at the field's parting plane where it stands.

use super::judge::PART_NOISE_MM2;
use super::{BORE_TOL_MM, BoreTrace, FaceClass, RingDesign, read_face};
use crate::interaction::bvh::Bvh;
use crate::mesh::{Mesh, Vec3};
use super::judge::{SILHOUETTE_DEG, SILHOUETTE_MM, chord_lean};
use std::sync::{Arc, OnceLock};

/// Radial lines round the ring the band's crossings are read on.
const COLUMNS: usize = 720;
/// Radial lines per millimetre along the finger.
const ROWS_PER_MM: f64 = 20.0;
/// Most rows of radial lines the band is read on.
const MAX_ROWS: usize = 1200;
/// Most crossings one radial line keeps.
const MAX_CROSSINGS: usize = 6;
/// A cut's face longer than this is split even where its samples agree, mm.
const AGREE_MM: f64 = 0.5;
/// A cut's face this short is read by the metal's depth at its corners, taken as linear across it, mm.
const LEAF_MM: f64 = 0.1;
/// Radial depths past this are read as this, so a line with no metal on it stays finite, mm.
const DEPTH_CAP_MM: f32 = 1.0;
/// Most halvings one face of a cut takes.
const MAX_SPLITS: u32 = 24;
/// How far past a crossing the next cast along its line starts, mm.
const STEP_PAST_MM: f64 = 1e-5;

/// The parting plane, the draft floor and the bore a ghost is read against.
pub struct GhostJudge {
    parting_z: f64,
    min_draft: f64,
    bore_limit: f64,
    trace: BoreTrace,
    /// The band a cut's faces are read inside.
    band: Option<Arc<Mesh>>,
    /// The band's crossings along radial lines, laid out on the first cut read.
    radial: OnceLock<Radial>,
}

/// Where one radial line from the finger's axis crosses the band's surface, outward.
#[derive(Clone, Copy, Debug, Default)]
struct Crossings {
    n: u8,
    r: [f32; MAX_CROSSINGS],
}

/// The band's metal along radial lines from the finger's axis, each line cast once when first read.
struct Radial {
    tree: Arc<Bvh>,
    z0: f64,
    rows: usize,
    /// Nearest and furthest radius of the band's surface.
    r_min: f64,
    r_max: f64,
    cells: Vec<OnceLock<Crossings>>,
}

impl Radial {
    /// The radial lines over `band`, cast through `tree`, a tree built over that band.
    fn of(band: &Mesh, tree: Arc<Bvh>) -> Self {
        let (lo, hi) = band.bounds().unwrap_or_default();
        let rows = ((f64::from(hi.2 - lo.2) * ROWS_PER_MM).ceil() as usize + 1).min(MAX_ROWS);
        let radius = |v: &Vec3| f64::from(v.0).hypot(f64::from(v.1));
        let (r_min, r_max) = band.vertices.iter().map(radius).fold((f64::INFINITY, 0.0f64), |(a, b), r| (a.min(r), b.max(r)));
        Self { tree, z0: f64::from(lo.2), rows, r_min, r_max, cells: (0..rows * COLUMNS).map(|_| OnceLock::new()).collect() }
    }

    /// Every crossing of the line at `theta_deg` and `z`, outward from the axis.
    fn cast(&self, band: &Mesh, theta_deg: f64, z: f64) -> Crossings {
        let (s, c) = theta_deg.to_radians().sin_cos();
        let mut out = Crossings::default();
        let mut at = 0.0;
        while usize::from(out.n) < MAX_CROSSINGS {
            let Some((_, t)) = self.tree.ray(band, [c * at, s * at, z], [c, s, 0.0]) else { break };
            at += t;
            out.r[usize::from(out.n)] = at as f32;
            out.n += 1;
            at += STEP_PAST_MM;
        }
        out
    }

    /// The crossings of the line through column `col` and row `row`: a line grazing the surface is cast again a hair either side.
    fn crossings(&self, band: &Mesh, col: usize, row: usize) -> &Crossings {
        self.cells[row * COLUMNS + col].get_or_init(|| {
            let (theta, z) = (col as f64 * 360.0 / COLUMNS as f64, self.z0 + row as f64 / ROWS_PER_MM);
            let mut first = self.cast(band, theta, z);
            if first.n % 2 == 0 {
                return first;
            }
            for dz in [2e-3, -2e-3] {
                let again = self.cast(band, theta, z + dz);
                if again.n % 2 == 0 {
                    return again;
                }
            }
            first.n -= 1;
            first
        })
    }

    /// How deep `p` lies in the metal along the radial line nearest it: positive between an entry and an exit, negative outside, capped either way.
    fn depth(&self, band: &Mesh, p: [f64; 3]) -> f32 {
        let row = ((p[2] - self.z0) * ROWS_PER_MM).round();
        if !(row >= 0.0 && row < self.rows as f64) {
            return -DEPTH_CAP_MM;
        }
        let col = (p[1].atan2(p[0]).to_degrees() * COLUMNS as f64 / 360.0).round().rem_euclid(COLUMNS as f64) as usize % COLUMNS;
        let c = self.crossings(band, col, row as usize);
        let r = p[0].hypot(p[1]) as f32;
        c.r[..usize::from(c.n)].chunks_exact(2).fold(-DEPTH_CAP_MM, |best, w| best.max((r - w[0]).min(w[1] - r))).min(DEPTH_CAP_MM)
    }

    /// Whether `p` lies between an entry and an exit of the radial line nearest it.
    fn inside(&self, band: &Mesh, p: [f64; 3]) -> bool {
        self.depth(band, p) >= 0.0
    }

    /// Whether triangle `t` stands clear of the band: past its reach in radius or along the finger.
    fn clear_of(&self, t: &[[f64; 3]; 3]) -> bool {
        let (z_lo, z_hi) = (t[0][2].min(t[1][2]).min(t[2][2]), t[0][2].max(t[1][2]).max(t[2][2]));
        let z_end = self.z0 + (self.rows - 1) as f64 / ROWS_PER_MM;
        let r_far = t.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max);
        z_hi < self.z0 - 1.0 / ROWS_PER_MM || z_lo > z_end + 1.0 / ROWS_PER_MM || r_far < self.r_min || nearest_to_axis(t) > self.r_max
    }
}

/// The least distance from the finger's axis to triangle `t`: to its shadow on the plane square to the axis.
fn nearest_to_axis(t: &[[f64; 3]; 3]) -> f64 {
    let p = t.map(|v| [v[0], v[1]]);
    let cross = |a: [f64; 2], b: [f64; 2]| a[0] * b[1] - a[1] * b[0];
    let side = |i: usize| cross([p[(i + 1) % 3][0] - p[i][0], p[(i + 1) % 3][1] - p[i][1]], [-p[i][0], -p[i][1]]);
    let s = [side(0), side(1), side(2)];
    if s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0) {
        return 0.0;
    }
    (0..3)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % 3]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let len2 = d[0] * d[0] + d[1] * d[1];
            let k = if len2 > 1e-18 { (-(a[0] * d[0] + a[1] * d[1]) / len2).clamp(0.0, 1.0) } else { 0.0 };
            (a[0] + d[0] * k).hypot(a[1] + d[1] * k)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The share of a triangle where a depth taken as linear across it from its corners' `d` is positive.
fn linear_share(d: [f32; 3]) -> f64 {
    let d = d.map(f64::from);
    let above = d.iter().filter(|v| **v > 0.0).count();
    // The share where one corner stands alone on its side: its depth squared over its differences to the other two.
    let lone = |sign: f64| {
        let i = (0..3).find(|&i| (d[i] > 0.0) == (sign > 0.0)).unwrap_or(0);
        let (a, b, c) = (d[i], d[(i + 1) % 3], d[(i + 2) % 3]);
        let den = (a - b) * (a - c);
        if den.abs() > 1e-18 { (a * a / den).clamp(0.0, 1.0) } else { 0.5 }
    };
    match above {
        0 => 0.0,
        3 => 1.0,
        1 => lone(1.0),
        _ => 1.0 - lone(-1.0),
    }
}

/// What a ghost would do in the sand where it stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GhostRead {
    /// The class of every face of the ghost's mesh, in its order.
    pub classes: Vec<FaceClass>,
    /// Undercut that locks: the undercut class less the facets spanning the parting plane.
    pub undercut_mm2: f64,
    /// Undercut facets spanning the parting plane that lean less than their own chord.
    pub silhouette_mm2: f64,
    pub marginal_mm2: f64,
    pub vertical_mm2: f64,
    pub total_mm2: f64,
    /// Most negative draft over the undercut that locks, degrees; 0 when none does.
    pub worst_deg: f64,
}

impl GhostRead {
    /// Whether the ghost locks the mould past the noise the verdict reports and never gates on.
    pub fn locks(&self) -> bool {
        self.undercut_mm2 > PART_NOISE_MM2
    }

    /// What the ghost would do: "would lock 2.1 mm² at -35°", or "pulls clean".
    pub fn caption(&self) -> String {
        if self.locks() { format!("would lock {:.1} mm² at {:.0}°", self.undercut_mm2, self.worst_deg) } else { "pulls clean".into() }
    }
}

impl GhostJudge {
    /// Reads at `parting_z_mm` against the design's draft floor, with the bore traced on `band` when there is one.
    pub fn new(design: &RingDesign, band: Option<&Mesh>, parting_z_mm: f64) -> Self {
        Self::shared(design, band.map(|m| Arc::new(m.clone())), parting_z_mm)
    }

    /// [`GhostJudge::new`] over a band the caller already shares.
    pub fn shared(design: &RingDesign, band: Option<Arc<Mesh>>, parting_z_mm: f64) -> Self {
        Self {
            parting_z: if parting_z_mm.is_finite() { parting_z_mm } else { 0.0 },
            min_draft: design.draft.min_draft_deg.max(0.0),
            bore_limit: design.inner_radius_mm().max(0.0) + BORE_TOL_MM,
            trace: band.as_deref().map_or(BoreTrace { z_lo: 0.0, span: 1.0, radius: Vec::new() }, BoreTrace::of),
            band: band.filter(|b| !b.faces.is_empty()),
            radial: OnceLock::new(),
        }
    }

    /// [`GhostJudge::shared`] with the band's radial lines laid out now, their tree built on the caller's thread rather than on the first cut read.
    pub fn prepared(design: &RingDesign, band: Option<Arc<Mesh>>, parting_z_mm: f64) -> Self {
        let tree = band.as_deref().map(|b| Arc::new(Bvh::build(b)));
        Self::prepared_with(design, band, tree, parting_z_mm)
    }

    /// [`GhostJudge::prepared`] over a tree the caller already built on `band`, such as the ring frame's own.
    pub fn prepared_with(design: &RingDesign, band: Option<Arc<Mesh>>, tree: Option<Arc<Bvh>>, parting_z_mm: f64) -> Self {
        let judge = Self::shared(design, band, parting_z_mm);
        if let (Some(band), Some(tree)) = (judge.band.as_deref(), tree) {
            let _ = judge.radial.set(Radial::of(band, tree));
        }
        judge
    }

    /// Whether a cut read finds the band's radial lines laid out, so it builds no tree of its own.
    pub fn is_prepared(&self) -> bool {
        self.band.is_none() || self.radial.get().is_some()
    }

    /// The band and its radial crossings, laid out on first use; `None` without a band.
    fn radial(&self) -> Option<(&Mesh, &Radial)> {
        let band = self.band.as_deref()?;
        Some((band, self.radial.get_or_init(|| Radial::of(band, Arc::new(Bvh::build(band))))))
    }

    /// Whether `p` lies in the band's metal, read off the radial line through it; `None` without a band.
    pub fn in_band(&self, p: [f64; 3]) -> Option<bool> {
        let (band, radial) = self.radial()?;
        Some(radial.inside(band, p))
    }

    /// The share of triangle `t` in the band's metal: split along its longest edge until its corners and centroid can speak for it.
    fn share_in_band(&self, band: &Mesh, radial: &Radial, t: [[f64; 3]; 3], depth: u32) -> f64 {
        let corners = t.map(|p| radial.depth(band, p));
        self.share_of(band, radial, t, corners, depth)
    }

    /// [`Self::share_in_band`] with the corners' depths already read, which the halves share with their parent.
    fn share_of(&self, band: &Mesh, radial: &Radial, t: [[f64; 3]; 3], corners: [f32; 3], depth: u32) -> f64 {
        if radial.clear_of(&t) {
            return 0.0;
        }
        let len2 = |i: usize| (0..3).map(|k| (t[(i + 1) % 3][k] - t[i][k]).powi(2)).sum::<f64>();
        let i = (0..3).max_by(|a, b| len2(*a).total_cmp(&len2(*b))).unwrap_or(0);
        let long = len2(i).sqrt();
        if long <= LEAF_MM || depth == 0 {
            return linear_share(corners);
        }
        let centroid = std::array::from_fn(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
        let inside = corners.iter().filter(|c| **c >= 0.0).count() + usize::from(radial.inside(band, centroid));
        if (inside == 0 || inside == 4) && long <= AGREE_MM {
            return inside as f64 / 4.0;
        }
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        let mid: [f64; 3] = std::array::from_fn(|n| 0.5 * (t[i][n] + t[j][n]));
        let at_mid = radial.depth(band, mid);
        let first = self.share_of(band, radial, [t[i], mid, t[k]], [corners[i], at_mid, corners[k]], depth - 1);
        let second = self.share_of(band, radial, [mid, t[j], t[k]], [at_mid, corners[j], corners[k]], depth - 1);
        0.5 * (first + second)
    }

    /// Every face of `mesh` carried by `model` (rows of `[L | t]`) and read; `inside_out` reads a cut's faces as the walls it leaves, those in the band's metal alone.
    pub fn read(&self, mesh: &Mesh, model: &[[f64; 4]; 3], inside_out: bool) -> GhostRead {
        let m = model;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        // A mirroring map or a cut reverses the winding.
        let flip = (det < 0.0) != inside_out;
        let placed = Mesh {
            vertices: mesh
                .vertices
                .iter()
                .map(|v| {
                    let p = [f64::from(v.0), f64::from(v.1), f64::from(v.2)];
                    let at = |r: usize| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3];
                    Vec3(at(0) as f32, at(1) as f32, at(2) as f32)
                })
                .collect(),
            faces: if flip { mesh.faces.iter().map(|f| [f[0], f[2], f[1]]).collect() } else { mesh.faces.clone() },
            ..Default::default()
        };
        let mut out = GhostRead { classes: Vec::with_capacity(placed.faces.len()), ..Default::default() };
        let mut worst = 0.0f64;
        // Facets across the plane leaning less than a chord can, with their area and draft, for the corner test.
        let mut across: Vec<(usize, f64, f64)> = Vec::new();
        for (fi, f) in placed.faces.iter().enumerate() {
            // A cut leaves walls only where it runs through the band's metal.
            let share = match (inside_out.then(|| self.radial()).flatten(), placed.triangle(f)) {
                (Some((band, radial)), Some((a, b, c))) => self.share_in_band(band, radial, [a, b, c], MAX_SPLITS),
                _ => 1.0,
            };
            let read = (share > 0.0).then(|| read_face(&placed, f, self.parting_z, self.min_draft, self.bore_limit, &self.trace)).flatten();
            let Some(mut r) = read else {
                out.classes.push(FaceClass::Good);
                continue;
            };
            r.area *= share;
            out.total_mm2 += r.area;
            match r.class {
                FaceClass::Undercut => {
                    let spans = placed.triangle(f).is_some_and(|(a, b, c)| {
                        let (lo, hi) = (a[2].min(b[2]).min(c[2]), a[2].max(b[2]).max(c[2]));
                        lo <= self.parting_z + SILHOUETTE_MM && hi >= self.parting_z - SILHOUETTE_MM
                    });
                    if spans && r.draft > -SILHOUETTE_DEG {
                        across.push((fi, r.area, r.draft));
                    } else {
                        out.undercut_mm2 += r.area;
                        worst = worst.min(r.draft);
                    }
                }
                FaceClass::Marginal => out.marginal_mm2 += r.area,
                FaceClass::Vertical if !r.bore => out.vertical_mm2 += r.area,
                FaceClass::Vertical | FaceClass::Good => {}
            }
            out.classes.push(r.class);
        }
        if !across.is_empty() {
            let corners = Corners::of(&placed);
            for (fi, area, draft) in across {
                let chord = placed.triangle(&placed.faces[fi]).is_some_and(|(a, b, c)| chord_lean([a, b, c], corners.at(fi), self.parting_z));
                if chord {
                    out.silhouette_mm2 += area;
                } else {
                    out.undercut_mm2 += area;
                    worst = worst.min(draft);
                }
            }
        }
        out.worst_deg = worst;
        out
    }
}

/// A mesh's corner normals as a resolved part's are: each corner averages the faces round it within the crease angle of its own, weighted by their angle there.
struct Corners<'a> {
    mesh: &'a Mesh,
    normals: Vec<Option<[f64; 3]>>,
    /// Faces round each vertex, `around[start[v]..start[v + 1]]`.
    start: Vec<u32>,
    around: Vec<u32>,
}

impl<'a> Corners<'a> {
    fn of(mesh: &'a Mesh) -> Self {
        let n = mesh.vertices.len();
        let mut start = vec![0u32; n + 1];
        for f in &mesh.faces {
            for &v in f.iter().filter(|v| (**v as usize) < n) {
                start[v as usize + 1] += 1;
            }
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut around = vec![0u32; start[n] as usize];
        for (fi, f) in mesh.faces.iter().enumerate() {
            for &v in f.iter().filter(|v| (**v as usize) < n) {
                around[fill[v as usize] as usize] = fi as u32;
                fill[v as usize] += 1;
            }
        }
        Self { mesh, normals: mesh.faces.iter().map(|f| mesh.face_normal(f)).collect(), start, around }
    }

    /// The three corner normals of face `fi`; its own normal where a corner has nothing to average.
    fn at(&self, fi: usize) -> [Vec3; 3] {
        let cos_crease = crate::parts::CREASE_DEG.to_radians().cos();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let mine = self.normals[fi].unwrap_or([0.0, 0.0, 1.0]);
        let face = self.mesh.faces[fi];
        std::array::from_fn(|k| {
            let v = face[k] as usize;
            let mut sum = [0.0; 3];
            for &g in self.around.get(self.start[v] as usize..self.start[v + 1] as usize).unwrap_or(&[]) {
                let Some(theirs) = self.normals[g as usize].filter(|t| dot(mine, *t) >= cos_crease) else { continue };
                let w = self.angle_at(g as usize, v as u32);
                sum = std::array::from_fn(|i| sum[i] + theirs[i] * w);
            }
            let len = dot(sum, sum).sqrt();
            let n = if len > 1e-12 { sum.map(|c| c / len) } else { mine };
            Vec3(n[0] as f32, n[1] as f32, n[2] as f32)
        })
    }

    /// Face `fi`'s interior angle at vertex `v`, radians.
    fn angle_at(&self, fi: usize, v: u32) -> f64 {
        let f = self.mesh.faces[fi];
        let Some(k) = f.iter().position(|x| *x == v) else { return 0.0 };
        let Some((a, b, c)) = self.mesh.triangle(&[f[k], f[(k + 1) % 3], f[(k + 2) % 3]]) else { return 0.0 };
        let unit = |p: [f64; 3]| {
            let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            if l > 1e-300 { p.map(|c| c / l) } else { p }
        };
        let (e1, e2) = (unit([b[0] - a[0], b[1] - a[1], b[2] - a[2]]), unit([c[0] - a[0], c[1] - a[1], c[2] - a[2]]));
        (e1[0] * e2[0] + e1[1] * e2[1] + e1[2] * e2[2]).clamp(-1.0, 1.0).acos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed axis-aligned box from `lo` to `hi`, wound outward.
    fn cube(lo: [f32; 3], hi: [f32; 3]) -> Mesh {
        let v = |x: usize, y: usize, z: usize| Vec3([lo[0], hi[0]][x], [lo[1], hi[1]][y], [lo[2], hi[2]][z]);
        let vertices = vec![v(0, 0, 0), v(1, 0, 0), v(1, 1, 0), v(0, 1, 0), v(0, 0, 1), v(1, 0, 1), v(1, 1, 1), v(0, 1, 1)];
        let faces = vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [2, 3, 7], [2, 7, 6], [1, 2, 6], [1, 6, 5], [3, 0, 4], [3, 4, 7]];
        Mesh { vertices, faces, ..Default::default() }
    }
    const IDENTITY: [[f64; 4]; 3] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
    fn lifted(dz: f64) -> [[f64; 4]; 3] {
        [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, dz]]
    }

    #[test]
    fn a_flat_wall_leaning_back_across_the_plane_locks_in_the_ghost_as_in_the_verdict() {
        let d = RingDesign::default();
        let judge = GhostJudge::new(&d, None, 0.0);
        let post = cube([-1.0, 12.0, -1.0], [1.0, 14.0, 1.0]);
        // Turned 3° about the ring's radius, each wall facing round the ring leans back over its far half.
        let (s, c) = 3f64.to_radians().sin_cos();
        let turned = [[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0]];
        let read = judge.read(&post, &turned, false);
        // Its own lean, not a chord's: the triangle across the plane on each wall's far side locks, 2 mm² apiece.
        assert!(read.silhouette_mm2 < 1e-9 && (read.undercut_mm2 - 4.0).abs() < 1e-5 && (read.worst_deg + 3.0).abs() < 1e-4, "{read:?}");
        assert!(read.locks());
    }

    #[test]
    fn a_box_straddling_the_plane_pulls_clean_and_lifted_off_it_locks_its_underside() {
        let d = RingDesign::default();
        let judge = GhostJudge::new(&d, None, 0.0);
        // Two millimetres square on the crest at the top of the ring, half above the plane and half below.
        let post = cube([-1.0, 12.0, -1.0], [1.0, 14.0, 1.0]);
        let clean = judge.read(&post, &IDENTITY, false);
        assert_eq!(clean.classes.len(), 12);
        assert!(!clean.locks() && clean.undercut_mm2 == 0.0, "{clean:?}");
        assert_eq!(clean.caption(), "pulls clean");
        assert!((clean.total_mm2 - 24.0).abs() < 1e-6 && (clean.vertical_mm2 - 16.0).abs() < 1e-6);
        // Carried 1.5 mm up the finger the whole box stands in the cope: its underside faces the drag and locks.
        let off = judge.read(&post, &lifted(1.5), false);
        let under: Vec<usize> = (0..12).filter(|&i| off.classes[i] == FaceClass::Undercut).collect();
        assert_eq!(under, [0, 1], "the two triangles facing -Z");
        assert!((off.undercut_mm2 - 4.0).abs() < 1e-6 && off.worst_deg == -90.0, "{off:?}");
        assert_eq!(off.caption(), "would lock 4.0 mm² at -90°");
        // Read against a plane raised with it, it straddles again.
        assert!(!GhostJudge::new(&d, None, 1.5).read(&post, &lifted(1.5), false).locks());
    }

    #[test]
    fn a_mirrored_or_cut_ghost_is_read_facing_the_right_way() {
        let d = RingDesign::default();
        let judge = GhostJudge::new(&d, None, 0.0);
        let post = cube([-1.0, 12.0, 0.5], [1.0, 14.0, 2.5]);
        let up = judge.read(&post, &IDENTITY, false);
        assert!((up.undercut_mm2 - 4.0).abs() < 1e-6);
        // Mirrored through the plane it stands in the drag, its top now facing +Z below the plane.
        let mirror = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0]];
        let down = judge.read(&post, &mirror, false);
        assert!((down.undercut_mm2 - 4.0).abs() < 1e-6 && down.classes[0] == FaceClass::Undercut, "{down:?}");
        // As a cut the box leaves a pocket whose roof faces down into it: the cope side's roof locks.
        let pocket = judge.read(&post, &IDENTITY, true);
        assert_eq!((0..12).filter(|&i| pocket.classes[i] == FaceClass::Undercut).collect::<Vec<_>>(), [2, 3]);
    }

    #[test]
    fn a_pilot_holes_ghost_reads_only_the_walls_it_leaves_in_the_band() {
        use crate::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage};
        use crate::castability::judged_field_report;
        use std::time::Instant;
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 384, profile_steps: 144, ..crate::BuildParams::default() };
        let mut band = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        band.profile.width_mm = 6.0;
        // A radial pilot centred on the crest: 1 mm long reaches half into the 2 mm band, 6 mm long runs through it into the finger.
        for (length, walls_deep) in [(1.0, 0.5), (6.0, 2.0)] {
            let mut doc = Document::default();
            doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
            let pilot = Component { attach: Attach::Cut, stage: Stage::Cast, placement: Placement::ring(90.0, 0.0), ..Component::default() };
            doc.append(Feature { id: 3, name: "Pilot".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.5, height_mm: length }, component: pilot }).unwrap();
            let d = RingDesign { cad: Some(doc), ..band.clone() };
            let built = crate::mesh::try_build(&d, &lib, params).unwrap();
            let field = judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&built));
            // The verdict's undercut class holds the silhouette facets inside it; the ghost's splits them out.
            let part = field.parts.iter().find(|p| p.feature == 3).unwrap();
            let (judged, walls_in) = (part.undercut_area_mm2, part.total_area_mm2);
            let cutter = &built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap().mesh;
            let judge = GhostJudge::shared(&d, built.band.clone(), field.parting_z_mm);
            let t = Instant::now();
            let ghost = judge.read(cutter, &IDENTITY, true);
            let first_ms = t.elapsed().as_secs_f64() * 1e3;
            // Carried 0.02 mm a frame along the finger, as a drag reads it.
            let frames = 20;
            let t = Instant::now();
            for i in 0..frames {
                std::hint::black_box(judge.read(cutter, &lifted(0.02 * f64::from(i)), true));
            }
            let frame_ms = t.elapsed().as_secs_f64() * 1e3 / f64::from(frames);
            let t = Instant::now();
            for _ in 0..frames {
                std::hint::black_box(judge.read(cutter, &IDENTITY, true));
            }
            let still_ms = t.elapsed().as_secs_f64() * 1e3 / f64::from(frames);
            let every_face = GhostJudge::new(&d, None, field.parting_z_mm).read(cutter, &IDENTITY, true);
            let walls = std::f64::consts::TAU * 0.5 * walls_deep;
            let class = ghost.undercut_mm2 + ghost.silhouette_mm2;
            eprintln!(
                "{length} mm pilot, {} faces: the ghost reads {:.3} mm² of walls, {class:.3} undercut ({:.3} locking); the built ring {walls_in:.3} of walls, {judged:.3} undercut ({:.3} locking); every face of the cutter read {:.3} undercut; 2π·r·depth {walls:.3}; first read {first_ms:.1} ms, then {frame_ms:.2} ms a moved frame, {still_ms:.2} ms a still one",
                cutter.faces.len(),
                ghost.total_mm2,
                ghost.undercut_mm2,
                judged - part.silhouette_mm2,
                every_face.undercut_mm2 + every_face.silhouette_mm2
            );
            assert!((ghost.total_mm2 - walls_in).abs() < 0.01 * walls_in, "walls {} against {walls_in}", ghost.total_mm2);
            assert!((class - judged).abs() < 0.01 * judged, "undercut {class} against {judged}");
            assert!(every_face.undercut_mm2 + every_face.silhouette_mm2 > 1.5 * judged, "every face read over-reports: {} against {judged}", every_face.undercut_mm2);
            assert_eq!(ghost.classes.len(), cutter.faces.len());
            assert!(ghost.caption().starts_with("would lock"), "{}", ghost.caption());
            // Carried 4 mm clear of the band the same cutter cuts nothing and reads clean.
            let clear = judge.read(cutter, &[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 4.0 + length], [0.0, 0.0, 1.0, 0.0]], true);
            assert!(clear.undercut_mm2 == 0.0 && clear.total_mm2 == 0.0 && clear.caption() == "pulls clean", "{clear:?}");
        }
        // Metal read off the radial lines: the crest's middle, the finger hole, and above the band.
        let judge = GhostJudge::shared(&band, Some(std::sync::Arc::new(crate::mesh::try_build(&band, &lib, params).unwrap().mesh)), 0.0);
        let r = band.inner_radius_mm();
        assert_eq!([r + 0.5, r - 0.5, r + band.profile.thickness_mm + 0.5].map(|y| judge.in_band([0.0, y, 0.0])), [Some(true), Some(false), Some(false)]);
        assert_eq!(GhostJudge::new(&band, None, 0.0).in_band([0.0, r + 0.5, 0.0]), None);
        // A through-seat bur under a 6.5 mm round on the Court band: what a drag of the claw solitaire's cutter costs a frame.
        let solitaire = RingDesign { cad: crate::cad::examples::design("claw-solitaire").unwrap().cad, ..band.clone() };
        let built = crate::mesh::try_build(&solitaire, &lib, params).unwrap();
        let bur = &built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 4).unwrap().mesh;
        let judge = GhostJudge::shared(&solitaire, built.band.clone(), 0.0);
        let t = Instant::now();
        let first = judge.read(bur, &IDENTITY, true);
        let first_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        for i in 0..20 {
            std::hint::black_box(judge.read(bur, &lifted(0.02 * f64::from(i)), true));
        }
        let frame_ms = t.elapsed().as_secs_f64() * 1e3 / 20.0;
        let whole = GhostJudge::new(&solitaire, None, 0.0).read(bur, &IDENTITY, true);
        eprintln!("seat bur, {} faces: {:.3} mm² of its {:.3} runs through the band; first read {first_ms:.1} ms, then {frame_ms:.2} ms a moved frame", bur.faces.len(), first.total_mm2, whole.total_mm2);
        assert!(first.total_mm2 > 0.0 && first.total_mm2 < 0.8 * whole.total_mm2, "{} of {}", first.total_mm2, whole.total_mm2);
        // A depth linear across a face: a lone corner's share is its depth squared over its differences to the others.
        assert_eq!([linear_share([1.0, -1.0, -1.0]), linear_share([1.0, 1.0, -1.0]), linear_share([-1.0, -2.0, -0.5]), linear_share([0.5, 2.0, 1.0])], [0.25, 0.75, 0.0, 1.0]);
        assert!((linear_share([2.0, -1.0, -1.0]) - 4.0 / 9.0).abs() < 1e-12);
    }

    #[test]
    fn a_prepared_judge_reads_a_cut_as_the_lazy_one_does_with_its_tree_built_before_the_first_read() {
        use std::time::Instant;
        let lib = crate::AlphaLibrary::builtin();
        let mut band = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        band.profile.width_mm = 6.0;
        let solitaire = RingDesign { cad: crate::cad::examples::design("claw-solitaire").unwrap().cad, ..band };
        for (name, params) in [("preview", crate::BuildParams { theta_steps: 384, profile_steps: 144, ..crate::BuildParams::default() }), ("export", crate::BuildParams { theta_steps: 1024, profile_steps: 320, ..crate::BuildParams::default() })] {
            let built = crate::mesh::try_build(&solitaire, &lib, params).unwrap();
            let bur = &built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 4).unwrap().mesh;
            let lazy = GhostJudge::shared(&solitaire, built.band.clone(), 0.0);
            assert!(!lazy.is_prepared(), "the lazy judge lays its lines out on the first cut read");
            let t = Instant::now();
            let from_lazy = lazy.read(bur, &IDENTITY, true);
            let lazy_ms = t.elapsed().as_secs_f64() * 1e3;
            assert!(lazy.is_prepared());
            let t = Instant::now();
            let prepared = GhostJudge::prepared(&solitaire, built.band.clone(), 0.0);
            let prepare_ms = t.elapsed().as_secs_f64() * 1e3;
            assert!(prepared.is_prepared(), "the tree is built before any read");
            let t = Instant::now();
            let from_prepared = prepared.read(bur, &IDENTITY, true);
            let read_ms = t.elapsed().as_secs_f64() * 1e3;
            assert_eq!(from_prepared, from_lazy, "{name}: the same walls, classes and areas either way");
            // A tree built over the same band by someone else, as the ring frame's, is shared rather than built again.
            let tree = std::sync::Arc::new(Bvh::build(built.band.as_deref().unwrap()));
            let t = Instant::now();
            let shared = GhostJudge::prepared_with(&solitaire, built.band.clone(), Some(tree), 0.0);
            let shared_ms = t.elapsed().as_secs_f64() * 1e3;
            assert_eq!(shared.read(bur, &IDENTITY, true), from_lazy);
            eprintln!(
                "{name}: the seat bur's first cut read {lazy_ms:.1} ms lazily; prepared off the reading thread {prepare_ms:.1} ms, then its first read {read_ms:.1} ms; over a shared tree the judge costs {shared_ms:.2} ms"
            );
        }
        // Without a band there is nothing to lay out, and a judge reads every face.
        let bare = GhostJudge::prepared(&solitaire, None, 0.0);
        assert!(bare.is_prepared() && bare.in_band([0.0, 9.0, 0.0]).is_none());
    }
}

