use crate::mesh::{Mesh, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub axis: usize,
    pub offset: f64,
    pub flip: bool,
}
impl Default for Plane {
    fn default() -> Self {
        Self {
            axis: 2,
            offset: 0.0,
            flip: false,
        }
    }
}
impl Plane {
    pub fn normal(self) -> [f64; 3] {
        let mut n = [0.0; 3];
        n[self.axis.min(2)] = 1.0;
        n
    }
    /// Shader discards the positive side. Zero normal disables clipping.
    pub fn uniform(self) -> [f32; 4] {
        let s = if self.flip { -1.0 } else { 1.0 };
        let mut p = [0.0; 4];
        p[self.axis.min(2)] = s;
        p[3] = -self.offset as f32 * s;
        p
    }
    pub fn range(self, mesh: &Mesh) -> [f64; 2] {
        mesh.bounds()
            .map(|(a, b)| [coords(a)[self.axis.min(2)], coords(b)[self.axis.min(2)]])
            .unwrap_or([-1.0, 1.0])
    }
}
pub fn coords(v: Vec3) -> [f64; 3] {
    [v.0 as f64, v.1 as f64, v.2 as f64]
}
#[derive(Default)]
pub struct Cut {
    pub segments: Vec<[[f64; 3]; 2]>,
    pub inward: Vec<[f64; 3]>,
    pub length_mm: f64,
}
impl Cut {
    /// Even/odd scan slabs triangulate the planar cut, retaining holes. Slabs
    /// end at every segment endpoint, so no boundary bends within a slab.
    pub fn cap(&self, axis: usize) -> Vec<[[f64; 3]; 3]> {
        let axes: Vec<_> = (0..3).filter(|&i| i != axis.min(2)).collect();
        let (x, y) = (axes[0], axes[1]);
        let mut xs: Vec<_> = self
            .segments
            .iter()
            .flat_map(|s| s.iter().map(|p| p[x]))
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
        let mut triangles = Vec::new();
        for slab in xs.windows(2) {
            let [left, right] = [slab[0], slab[1]];
            let mid = (left + right) * 0.5;
            let mut edges: Vec<_> = self
                .segments
                .iter()
                .filter(|[a, b]| (a[x].min(b[x]) < mid) && (a[x].max(b[x]) > mid))
                .collect();
            let at = |edge: &[[f64; 3]; 2], xx: f64| -> [f64; 3] {
                let [a, b] = *edge;
                let t = (xx - a[x]) / (b[x] - a[x]);
                std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
            };
            edges.sort_by(|a, b| at(a, mid)[y].total_cmp(&at(b, mid)[y]));
            if edges.len() % 2 != 0 {
                continue;
            }
            for pair in edges.chunks_exact(2) {
                let a = at(pair[0], left);
                let b = at(pair[0], right);
                let c = at(pair[1], right);
                let d = at(pair[1], left);
                triangles.push([a, b, c]);
                triangles.push([a, c, d]);
            }
        }
        triangles
    }
    /// From a cut edge into the metal, intersect the next boundary of the
    /// same section. This is a local chord, not a global minimum-wall proof.
    pub fn wall_at(&self, index: usize, t: f64, axis: usize) -> Option<[[f64; 3]; 2]> {
        let [a, b] = *self.segments.get(index)?;
        let dir = *self.inward.get(index)?;
        let p: [f64; 3] = std::array::from_fn(|j| a[j] + (b[j] - a[j]) * t.clamp(0.0, 1.0));
        let axes: Vec<_> = (0..3).filter(|&i| i != axis.min(2)).collect();
        let (x, y) = (axes[0], axes[1]);
        let mut best = f64::INFINITY;
        for (i, [c, d]) in self.segments.iter().enumerate() {
            if i == index {
                continue;
            }
            let e: [f64; 3] = std::array::from_fn(|j| d[j] - c[j]);
            let q: [f64; 3] = std::array::from_fn(|j| c[j] - p[j]);
            let det = dir[x] * e[y] - dir[y] * e[x];
            if det.abs() < 1e-12 {
                continue;
            }
            let along = (q[x] * e[y] - q[y] * e[x]) / det;
            let u = (q[x] * dir[y] - q[y] * dir[x]) / det;
            if along > 1e-4 && (-1e-6..=1.000001).contains(&u) {
                best = best.min(along);
            }
        }
        best.is_finite()
            .then(|| [p, std::array::from_fn(|j| p[j] + dir[j] * best)])
    }
}
pub fn cut(mesh: &Mesh, p: Plane) -> Cut {
    let mut out = Cut::default();
    if !p.offset.is_finite() {
        return out;
    }
    let mut seen = std::collections::HashSet::new();
    let axis = p.axis.min(2);
    for face in &mesh.faces {
        let Some(a) = mesh.vertices.get(face[0] as usize).copied() else {
            continue;
        };
        let Some(b) = mesh.vertices.get(face[1] as usize).copied() else {
            continue;
        };
        let Some(c) = mesh.vertices.get(face[2] as usize).copied() else {
            continue;
        };
        let pts = [coords(a), coords(b), coords(c)];
        let mut hits: Vec<[f64; 3]> = Vec::with_capacity(3);
        for i in 0..3 {
            let a = pts[i];
            let b = pts[(i + 1) % 3];
            let da = a[axis] - p.offset;
            let db = b[axis] - p.offset;
            // A tolerance here invents intersections on triangles entirely
            // beside the plane, particularly along a sampled flat crest.
            if (da > 0.0 && db > 0.0) || (da < 0.0 && db < 0.0) || da == db {
                continue;
            }
            let t = (da / (da - db)).clamp(0.0, 1.0);
            let q = std::array::from_fn(|j| a[j] + t * (b[j] - a[j]));
            if !hits.iter().any(|h| distance(*h, q) < 1e-7) {
                hits.push(q);
            }
        }
        if hits.len() == 2 {
            let quantize = |p: [f64; 3]| p.map(|v| (v * 100000.0).round() as i64);
            let a = quantize(hits[0]);
            let b = quantize(hits[1]);
            if a == b || !seen.insert(if a < b { (a, b) } else { (b, a) }) {
                continue;
            }
            // Use the same welded coordinates for topology and triangulation.
            // Merely deduplicating their keys leaves tiny gaps between adjacent
            // triangles; scan slabs can then see an odd number of crossings.
            let restore = |key: [i64; 3]| {
                let mut point = key.map(|v| v as f64 / 100000.0);
                point[axis] = p.offset;
                point
            };
            let segment = [restore(a), restore(b)];
            out.length_mm += distance(segment[0], segment[1]);
            out.segments.push(segment);
            let ab = crate::mesh::sub(pts[1], pts[0]);
            let ac = crate::mesh::sub(pts[2], pts[0]);
            let mut n = crate::mesh::cross(ab, ac);
            n[axis] = 0.0;
            let len = crate::mesh::norm(n).max(1e-12);
            out.inward.push(n.map(|v| -v / len));
        }
    }
    out
}
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cut_interpolates_real_triangle_positions() {
        let m = Mesh {
            vertices: vec![
                Vec3(-2.0, 0.0, 0.0),
                Vec3(2.0, 0.0, 0.0),
                Vec3(0.0, 4.0, 0.0),
            ],
            faces: vec![[0, 1, 2]],
            normals: vec![],
        };
        let c = cut(
            &m,
            Plane {
                axis: 1,
                offset: 2.0,
                flip: false,
            },
        );
        assert_eq!(c.segments.len(), 1);
        assert!((c.length_mm - 2.0).abs() < 1e-9);
        assert!(
            cut(
                &m,
                Plane {
                    axis: 1,
                    offset: 5.0,
                    flip: false
                }
            )
            .segments
            .is_empty()
        );
    }
    #[test]
    fn a_plane_through_a_shared_edge_counts_the_boundary_once() {
        let m = Mesh {
            vertices: vec![
                Vec3(-2.0, 0.0, 0.0),
                Vec3(2.0, 0.0, 0.0),
                Vec3(0.0, 4.0, 0.0),
                Vec3(0.0, -4.0, 0.0),
            ],
            faces: vec![[0, 1, 2], [1, 0, 3]],
            normals: vec![],
        };
        let c = cut(
            &m,
            Plane {
                axis: 1,
                offset: 0.0,
                flip: false,
            },
        );
        assert_eq!(c.segments.len(), 1);
        assert!((c.length_mm - 4.0).abs() < 1e-9);
    }
    #[test]
    fn triangles_near_but_beside_the_plane_do_not_invent_cut_edges() {
        let mesh = Mesh {
            vertices: vec![
                Vec3(-1.0, 0.0, 1e-9),
                Vec3(1.0, 0.0, 2e-9),
                Vec3(0.0, 1.0, 1.0),
            ],
            faces: vec![[0, 1, 2]],
            normals: vec![],
        };
        assert!(cut(&mesh, Plane::default()).segments.is_empty());
    }
    #[test]
    fn cap_keeps_the_finger_hole_open() {
        let mut c = Cut::default();
        for extent in [2.0, 1.0] {
            let corners = [
                [-extent, -extent, 0.0],
                [extent, -extent, 0.0],
                [extent, extent, 0.0],
                [-extent, extent, 0.0],
            ];
            for i in 0..4 {
                c.segments.push([corners[i], corners[(i + 1) % 4]]);
            }
        }
        let area: f64 = c
            .cap(2)
            .into_iter()
            .map(|[a, b, c]| {
                crate::mesh::norm(crate::mesh::cross(
                    crate::mesh::sub(b, a),
                    crate::mesh::sub(c, a),
                )) * 0.5
            })
            .sum();
        assert!((area - 12.0).abs() < 1e-9);
    }
    #[test]
    fn a_dense_round_band_has_a_complete_annular_cap() {
        let mut d = crate::RingDesign::default();
        d.profile.style = crate::profile::ProfileStyle::Flat;
        d.layers.layers.clear();
        let b = crate::mesh::try_build(
            &d,
            &crate::AlphaLibrary::builtin(),
            crate::mesh::BuildParams {
                theta_steps: 512,
                profile_steps: 192,
                ..Default::default()
            },
        )
        .unwrap();
        let c = cut(&b.mesh, Plane::default());
        let area: f64 = c
            .cap(2)
            .into_iter()
            .map(|[a, b, c]| {
                crate::mesh::norm(crate::mesh::cross(
                    crate::mesh::sub(b, a),
                    crate::mesh::sub(c, a),
                )) * 0.5
            })
            .sum();
        let expected = std::f64::consts::PI
            * ((d.inner_radius_mm() + d.profile.thickness_mm).powi(2)
                - d.inner_radius_mm().powi(2));
        assert!(
            (area - expected).abs() / expected < 0.003,
            "cap {area}, expected {expected}, {} edges",
            c.segments.len()
        );
    }
}
