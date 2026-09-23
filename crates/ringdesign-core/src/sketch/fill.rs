//! What a sketch is drawn with and over: a region's loops sampled and cut into triangles to fill,
//! the planar face a sketch lies on read back as its boundary, and a mesh cut by the sketch's plane.
use super::region::senses;
use super::{Region, distance};
use crate::Mesh;
use cadkernel::{
    brep::{self, Body},
    geom2d::Curve,
    space::Plane,
};

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A closed chain of curves as the points it walks through within `chord_mm`; empty when it does not close.
pub fn walk(curves: &[Curve], chord_mm: f64) -> Vec<[f64; 2]> {
    let pieces: Vec<Curve> = curves.iter().flat_map(|c| if matches!(c, Curve::Polyline(_)) { c.segments() } else { vec![c.clone()] }).collect();
    let Some(forward) = senses(&pieces) else { return Vec::new() };
    let mut out: Vec<[f64; 2]> = Vec::new();
    for (c, f) in pieces.iter().zip(forward) {
        let mut points = c.tessellate_within(chord_mm.max(1e-4));
        if !f {
            points.reverse();
        }
        for p in points {
            if out.last().is_none_or(|q| distance(*q, p) > 1e-9) {
                out.push(p);
            }
        }
    }
    if out.len() > 1 && distance(out[0], out[out.len() - 1]) <= 1e-9 {
        out.pop();
    }
    out
}

/// Polygons nested even-odd cut into triangles, every polygon edge held; empty when they will not triangulate.
pub fn triangulate(polygons: &[Vec<[f64; 2]>]) -> Vec<[[f64; 2]; 3]> {
    use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    for polygon in polygons {
        let mut handles = Vec::with_capacity(polygon.len());
        for p in polygon {
            let Ok(h) = cdt.insert(Point2::new(p[0], p[1])) else { return Vec::new() };
            if handles.last() != Some(&h) {
                handles.push(h);
            }
        }
        if handles.len() > 1 && handles.first() == handles.last() {
            handles.pop();
        }
        if handles.len() < 3 {
            continue;
        }
        for i in 0..handles.len() {
            let (a, b) = (handles[i], handles[(i + 1) % handles.len()]);
            if a == b {
                continue;
            }
            if !cdt.can_add_constraint(a, b) {
                return Vec::new();
            }
            cdt.add_constraint(a, b);
        }
    }
    let mut inside = std::collections::HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for e in cdt.convex_hull() {
        if let Some(f) = e.face().as_inner().or_else(|| e.rev().face().as_inner()) {
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(f.fix()) {
                v.insert(cdt.is_constraint_edge(e.as_undirected().fix()));
                queue.push_back(f.fix());
            }
        }
    }
    while let Some(f) = queue.pop_front() {
        let here = inside[&f];
        for e in cdt.face(f).adjacent_edges() {
            let Some(next) = e.rev().face().as_inner() else { continue };
            let flip = cdt.is_constraint_edge(e.as_undirected().fix());
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(next.fix()) {
                v.insert(here != flip);
                queue.push_back(next.fix());
            }
        }
    }
    cdt.inner_faces()
        .filter(|f| inside.get(&f.fix()).copied().unwrap_or(false))
        .map(|f| f.vertices().map(|v| [v.position().x, v.position().y]))
        .collect()
}

impl Region {
    /// Each loop as the points it walks through within `chord_mm`, the outer loop first.
    pub fn polygons(&self, chord_mm: f64) -> Vec<Vec<[f64; 2]>> {
        self.loops().iter().map(|l| walk(l, chord_mm)).filter(|p| p.len() >= 3).collect()
    }
    /// The region cut into triangles within `chord_mm` of its curves, its holes left out.
    pub fn triangles(&self, chord_mm: f64) -> Vec<[[f64; 2]; 3]> {
        triangulate(&self.polygons(chord_mm))
    }
    /// A point well inside the region: the centroid of its largest fill triangle.
    pub fn inside(&self) -> Option<[f64; 2]> {
        let start = self.outer.first()?.point_at(0.0);
        let reach = self.outer.iter().flat_map(|c| [c.point_at(0.0), c.point_at(0.5)]).fold(0.0_f64, |m, p| m.max(distance(p, start)));
        let area = |t: &[[f64; 2]; 3]| ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs();
        let biggest = self.triangles((reach * 1e-3).max(1e-4)).into_iter().max_by(|a, b| area(a).total_cmp(&area(b)))?;
        let c = [(biggest[0][0] + biggest[1][0] + biggest[2][0]) / 3.0, (biggest[0][1] + biggest[1][1] + biggest[2][1]) / 3.0];
        self.contains(c).then_some(c)
    }
}

/// The world boundary loops of the face of `body` lying in `plane` and facing its way, preferring one holding its origin.
pub fn face_outline(body: &Body, plane: &Plane, chord_mm: f64) -> Option<Vec<Vec<[f64; 3]>>> {
    let n = plane.normal()?;
    let mut first = None;
    for (key, _) in body.faces.iter() {
        let Some(face) = brep::planar_face_profile(body, key) else { continue };
        let reach = face.plane.origin.iter().chain(&plane.origin).fold(1.0_f64, |m, v| m.max(v.abs()));
        if dot(face.outward, n) < 1.0 - 1e-6 || dot(sub(face.plane.origin, plane.origin), n).abs() > 1e-6 * reach {
            continue;
        }
        let loops: Vec<Vec<[f64; 2]>> = face.loops.iter().map(|l| walk(l, chord_mm)).filter(|l| l.len() >= 3).collect();
        let Some(outer) = loops.first() else { continue };
        let world: Vec<Vec<[f64; 3]>> = loops.iter().map(|l| l.iter().map(|uv| face.plane.point_at(*uv)).collect()).collect();
        let origin = face.plane.project(plane.origin);
        if origin.is_some_and(|o| inside(outer, o)) {
            return Some(world);
        }
        first.get_or_insert(world);
    }
    first
}

/// Whether `p` lies inside the closed polygon.
fn inside(polygon: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut odd = false;
    let n = polygon.len();
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            odd = !odd;
        }
    }
    odd
}

/// Where `plane` cuts `mesh`, as segments in the plane's own coordinates.
pub fn slice(mesh: &Mesh, plane: &Plane) -> Vec<[[f64; 2]; 2]> {
    let Some(n) = plane.normal() else { return Vec::new() };
    let at = |i: u32| {
        let v = mesh.vertices[i as usize];
        [f64::from(v.0), f64::from(v.1), f64::from(v.2)]
    };
    let local = |p: [f64; 3]| {
        let d = sub(p, plane.origin);
        [dot(d, plane.x_axis), dot(d, plane.y_axis)]
    };
    let mut out = Vec::new();
    for f in &mesh.faces {
        let v = [at(f[0]), at(f[1]), at(f[2])];
        let d = v.map(|p| dot(sub(p, plane.origin), n));
        if d.iter().all(|x| *x > 0.0) || d.iter().all(|x| *x < 0.0) || d.iter().all(|x| *x == 0.0) {
            continue;
        }
        let mut cut: Vec<[f64; 3]> = Vec::with_capacity(2);
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            if d[i] == 0.0 {
                cut.push(v[i]);
            } else if (d[i] > 0.0) != (d[j] > 0.0) && d[j] != 0.0 {
                let t = d[i] / (d[i] - d[j]);
                cut.push(std::array::from_fn(|k| v[i][k] + (v[j][k] - v[i][k]) * t));
            }
        }
        if let [a, b] = cut.as_slice() {
            let (a, b) = (local(*a), local(*b));
            if distance(a, b) > 1e-9 {
                out.push([a, b]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::{Geometry, Sketch};

    fn area(triangles: &[[[f64; 2]; 3]]) -> f64 {
        triangles.iter().map(|t| ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs() * 0.5).sum()
    }

    #[test]
    fn regions_fill_with_their_holes_left_out_and_a_face_reads_back_as_its_outline() {
        // A washer: the triangles cover the ring between the circles, chord error aside.
        let mut s = Sketch::default();
        for r in [3.0, 2.0] {
            let c = s.point([0.0, 0.0]);
            let rim = s.point([r, 0.0]);
            s.entity(Geometry::Circle { center: c, rim });
        }
        let r = s.profile_regions().unwrap();
        let t = r[0].triangles(0.001);
        let want = std::f64::consts::PI * 5.0;
        assert!((area(&t) - want).abs() < 0.01, "{} against {want}", area(&t));
        assert!(t.iter().all(|t| {
            let c = [(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0];
            r[0].contains(c)
        }));
        // A filleted rectangle: its polygon starts once and closes on itself.
        let mut s = Sketch::default();
        s.add_rectangle([0.0, 0.0], [4.0, 3.0], false).unwrap();
        let corner = s.points[2].id;
        s.fillet_corner(corner, 0.5).unwrap();
        let region = &s.profile_regions().unwrap()[0];
        let polygon = &region.polygons(0.001)[0];
        assert!(distance(polygon[0], polygon[polygon.len() - 1]) > 1e-6);
        assert!((area(&region.triangles(0.001)) - region.area()).abs() < 1e-3, "{}", region.area());
        // The top face of a box, read back through the plane it lies in.
        let body = brep::make::cuboid([-4.0, -3.0, -1.0], [8.0, 6.0, 2.0]).unwrap();
        let top = Plane::from_axes([0.5, 0.25, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let outline = face_outline(&body, &top, 0.01).unwrap();
        assert_eq!(outline.len(), 1);
        assert!(outline[0].iter().all(|p| (p[2] - 1.0).abs() < 1e-9));
        let flat: Vec<[f64; 2]> = outline[0].iter().map(|p| [p[0], p[1]]).collect();
        assert!(flat.iter().all(|p| (p[0].abs() - 4.0).abs() < 1e-9 || (p[1].abs() - 3.0).abs() < 1e-9));
        let bottom_up = Plane::from_axes([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(face_outline(&body, &bottom_up, 0.01).is_none(), "the bottom face looks the other way");
        // A plane through the middle of a cube's tessellation cuts a closed square.
        let mesh = crate::cad::tessellate(&body, 0.05).unwrap();
        let mid = Plane::from_axes([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let cut = slice(&mesh, &mid);
        let length: f64 = cut.iter().map(|s| distance(s[0], s[1])).sum();
        assert!((length - 28.0).abs() < 1e-6, "{length}");
    }
}
