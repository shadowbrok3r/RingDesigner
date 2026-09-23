//! A carried part's ghost read by the verdict's face rule: every face classed at the field's parting plane where it stands.

use super::judge::PART_NOISE_MM2;
use super::{BORE_TOL_MM, BoreTrace, FaceClass, RingDesign, read_face};
use crate::mesh::{Mesh, Vec3};

/// An undercut facet reaching this close to the parting plane spans it, mm.
const SILHOUETTE_MM: f64 = 0.005;
/// Lean a facet spanning the parting plane may show as its own chord, degrees.
const SILHOUETTE_DEG: f64 = 5.0;

/// The parting plane, the draft floor and the bore a ghost is read against.
pub struct GhostJudge {
    parting_z: f64,
    min_draft: f64,
    bore_limit: f64,
    trace: BoreTrace,
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
        Self {
            parting_z: if parting_z_mm.is_finite() { parting_z_mm } else { 0.0 },
            min_draft: design.draft.min_draft_deg.max(0.0),
            bore_limit: design.inner_radius_mm().max(0.0) + BORE_TOL_MM,
            trace: band.map_or(BoreTrace { z_lo: 0.0, span: 1.0, radius: Vec::new() }, BoreTrace::of),
        }
    }

    /// Every face of `mesh` carried by `model` (rows of `[L | t]`) and read; `inside_out` reads a cut's faces as the walls it leaves.
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
        for f in &placed.faces {
            let Some(r) = read_face(&placed, f, self.parting_z, self.min_draft, self.bore_limit, &self.trace) else {
                out.classes.push(FaceClass::Good);
                continue;
            };
            out.total_mm2 += r.area;
            match r.class {
                FaceClass::Undercut => {
                    let spans = placed.triangle(f).is_some_and(|(a, b, c)| {
                        let (lo, hi) = (a[2].min(b[2]).min(c[2]), a[2].max(b[2]).max(c[2]));
                        lo <= self.parting_z + SILHOUETTE_MM && hi >= self.parting_z - SILHOUETTE_MM
                    });
                    if spans && r.draft > -SILHOUETTE_DEG {
                        out.silhouette_mm2 += r.area;
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
        out.worst_deg = worst;
        out
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
}
