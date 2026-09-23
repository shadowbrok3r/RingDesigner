//! Questions a pointer asks of a sketch: the nearest point and curve, where curves cross, the
//! points worth snapping to, and each entity as polylines to draw.
use super::{Geometry, Id, Sketch, distance};
use cadkernel::geom2d::{self, Curve, Tolerance};

/// The part of an entity nearest a place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Near {
    pub entity: Id,
    /// Which of the entity's curves, in `curves_of` order.
    pub curve: usize,
    /// The nearest place on it.
    pub at: [f64; 2],
    pub distance: f64,
}

/// The point on a polyline nearest `xy`, and how far it is.
pub fn nearest_on(polyline: &[[f64; 2]], xy: [f64; 2]) -> Option<([f64; 2], f64)> {
    let mut best: Option<([f64; 2], f64)> = None;
    for w in polyline.windows(2) {
        let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
        let t = (((xy[0] - w[0][0]) * d[0] + (xy[1] - w[0][1]) * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-18)).clamp(0.0, 1.0);
        let p = [w[0][0] + d[0] * t, w[0][1] + d[1] * t];
        let e = distance(p, xy);
        if best.is_none_or(|(_, b)| e < b) {
            best = Some((p, e));
        }
    }
    if polyline.len() == 1 {
        best = Some((polyline[0], distance(polyline[0], xy)));
    }
    best
}

impl Sketch {
    /// The point nearest `xy` no further than `reach`.
    pub fn nearest_point(&self, xy: [f64; 2], reach: f64) -> Option<Id> {
        self.points
            .iter()
            .map(|p| (p.id, distance(p.xy, xy)))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
    /// The entity passing nearest `xy` no further than `reach`, and where on it.
    pub fn nearest_entity(&self, xy: [f64; 2], reach: f64) -> Option<Near> {
        let mut best: Option<Near> = None;
        for e in &self.entities {
            for (k, c) in self.curves_of(e).unwrap_or_default().iter().enumerate() {
                let hit = geom2d::closest_point(c, xy);
                if hit.distance <= reach && best.is_none_or(|b| hit.distance < b.distance) {
                    best = Some(Near { entity: e.id, curve: k, at: hit.point, distance: hit.distance });
                }
            }
        }
        best
    }
    /// Every place curves of two different entities cross away from the sketch's own points, each once.
    pub fn crossings(&self) -> Vec<[f64; 2]> {
        let tol = Tolerance::new(1e-9);
        let pieces: Vec<(Id, Curve)> = self.entities.iter().flat_map(|e| self.curves_of(e).unwrap_or_default().into_iter().map(move |c| (e.id, c))).collect();
        let boxes: Vec<[f64; 4]> = pieces.iter().map(|(_, c)| bounds(c)).collect();
        let mut out: Vec<[f64; 2]> = Vec::new();
        for i in 0..pieces.len() {
            for j in i + 1..pieces.len() {
                let (a, b) = (&boxes[i], &boxes[j]);
                if pieces[i].0 == pieces[j].0 || a[0] > b[2] || b[0] > a[2] || a[1] > b[3] || b[1] > a[3] {
                    continue;
                }
                for x in geom2d::intersect(&pieces[i].1, &pieces[j].1, tol) {
                    let known = self.points.iter().any(|p| distance(p.xy, x.point) <= 1e-7);
                    if !known && !out.iter().any(|p| distance(*p, x.point) <= 1e-7) {
                        out.push(x.point);
                    }
                }
            }
        }
        out
    }
    /// The midpoint of every straight segment, drawn or construction.
    pub fn midpoints(&self) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for e in &self.entities {
            for c in self.curves_of(e).unwrap_or_default() {
                if let Curve::Line(l) = c {
                    out.push([(l.start[0] + l.end[0]) * 0.5, (l.start[1] + l.end[1]) * 0.5]);
                }
            }
        }
        out
    }
    /// The centre of every circle and arc.
    pub fn centres(&self) -> Vec<[f64; 2]> {
        self.entities
            .iter()
            .filter_map(|e| match e.geometry {
                Geometry::Circle { center, .. } | Geometry::Arc { center, .. } => self.at(center).ok(),
                _ => None,
            })
            .collect()
    }
    /// Entity `id` as one polyline per curve, sampled within `chord_mm`; empty when it will not read.
    pub fn polylines(&self, id: Id, chord_mm: f64) -> Vec<Vec<[f64; 2]>> {
        let Some(e) = self.entities.iter().find(|e| e.id == id) else { return Vec::new() };
        self.curves_of(e).unwrap_or_default().iter().map(|c| c.tessellate_within(chord_mm.max(1e-4))).collect()
    }
}

/// A curve's box as `[min x, min y, max x, max y]`, padded.
fn bounds(c: &Curve) -> [f64; 4] {
    let points = match c {
        Curve::Arc(a) => vec![[a.centre[0] - a.radius, a.centre[1] - a.radius], [a.centre[0] + a.radius, a.centre[1] + a.radius]],
        Curve::Circle(k) => vec![[k.centre[0] - k.radius, k.centre[1] - k.radius], [k.centre[0] + k.radius, k.centre[1] + k.radius]],
        Curve::Nurbs(n) => n.control_points().to_vec(),
        other => other.tessellate(16.0),
    };
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for p in points {
        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
    }
    [b[0] - 1e-6, b[1] - 1e-6, b[2] + 1e-6, b[3] + 1e-6]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_finds_the_nearest_point_and_curve_and_where_curves_cross() {
        let mut s = Sketch::default();
        let lines = s.add_rectangle([0.0, 0.0], [4.0, 2.0], false).unwrap();
        let p = [[2.0, -1.0], [2.0, 3.0]].map(|xy| s.point(xy));
        let cross = s.draw(Geometry::Line { a: p[0], b: p[1] }, true);
        assert_eq!(s.nearest_point([3.9, 0.05], 0.2), Some(s.points[1].id));
        assert_eq!(s.nearest_point([3.0, 1.0], 0.2), None);
        let near = s.nearest_entity([1.0, 0.1], 0.2).unwrap();
        assert_eq!((near.entity, near.at), (lines[0], [1.0, 0.0]));
        assert!((near.distance - 0.1).abs() < 1e-12);
        assert_eq!(s.nearest_entity([2.1, 1.0], 0.2).unwrap().entity, cross);
        let mut x = s.crossings();
        x.sort_by(|a, b| a[1].total_cmp(&b[1]));
        assert_eq!(x, vec![[2.0, 0.0], [2.0, 2.0]], "the construction line crosses the top and bottom; corners are not crossings");
        assert_eq!(s.midpoints().len(), 5);
        assert!(s.centres().is_empty());
        assert_eq!(s.polylines(lines[1], 0.01), vec![vec![[4.0, 0.0], [4.0, 2.0]]]);
        assert_eq!(nearest_on(&[[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]], [3.0, 1.0]), Some(([2.0, 1.0], 1.0)));
        // A cubic hump cresting at y = 3, crossed a thousandth under its crest: both crossings, 0.11 mm apart.
        let mut s = Sketch::default();
        let hump = [[0.0, 0.0], [0.0, 4.0], [4.0, 4.0], [4.0, 0.0]].map(|xy| s.point(xy));
        s.draw(Geometry::Bezier { points: hump }, false);
        let rail = [[-1.0, 2.999], [5.0, 2.999]].map(|xy| s.point(xy));
        s.draw(Geometry::Line { a: rail[0], b: rail[1] }, false);
        let mut x = s.crossings();
        x.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert_eq!(x.len(), 2, "{x:?}");
        assert!(x.iter().all(|p| (p[1] - 2.999).abs() < 1e-9), "{x:?}");
        assert!((x[0][0] + x[1][0] - 4.0).abs() < 1e-6 && (x[1][0] - x[0][0] - 0.10953).abs() < 1e-4, "{x:?}");
    }
}
