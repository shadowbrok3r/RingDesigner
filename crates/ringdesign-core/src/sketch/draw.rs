//! Drawing into a sketch: points placed or reused, and the lines, rectangles, circles and three-point
//! arcs the drawing tools lay down, each kept only once it validates.
use super::{Constraint, Geometry, Id, Sketch, distance};
use anyhow::{Context, Result, ensure};
use std::f64::consts::TAU;

/// Positions within this of each other are one point.
pub const SAME_MM: f64 = 1e-7;

/// The centre of the circle through three points; `None` when they lie on a line.
pub fn circumcentre(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<[f64; 2]> {
    let (bx, by) = (b[0] - a[0], b[1] - a[1]);
    let (cx, cy) = (c[0] - a[0], c[1] - a[1]);
    let d = 2.0 * (bx * cy - by * cx);
    let scale = (bx * bx + by * by).max(cx * cx + cy * cy);
    if !(d.abs() > 1e-12 * scale) || !d.is_finite() {
        return None;
    }
    let (b2, c2) = (bx * bx + by * by, cx * cx + cy * cy);
    Some([a[0] + (cy * b2 - by * c2) / d, a[1] + (bx * c2 - cx * b2) / d])
}

impl Sketch {
    /// Runs `edit` on a copy and keeps it only if the result validates.
    fn drawn<T>(&mut self, edit: impl FnOnce(&mut Sketch) -> Result<T>) -> Result<T> {
        let mut s = self.clone();
        let out = edit(&mut s)?;
        s.validate()?;
        *self = s;
        Ok(out)
    }
    /// The point at `xy`: one already there, or a new one.
    pub fn place(&mut self, xy: [f64; 2]) -> Id {
        match self.points.iter().find(|p| distance(p.xy, xy) <= SAME_MM) {
            Some(p) => p.id,
            None => self.point(xy),
        }
    }
    /// A new entity, drawn as construction when `construction` says so.
    pub fn draw(&mut self, geometry: Geometry, construction: bool) -> Id {
        let id = self.entity(geometry);
        if let Some(e) = self.entities.iter_mut().find(|e| e.id == id) {
            e.construction = construction;
        }
        id
    }
    /// A line between two points; refused when they coincide.
    pub fn add_line(&mut self, a: Id, b: Id, construction: bool) -> Result<Id> {
        self.drawn(|s| {
            ensure!(a != b && distance(s.at(a)?, s.at(b)?) > SAME_MM, "A line needs two different points");
            Ok(s.draw(Geometry::Line { a, b }, construction))
        })
    }
    /// A rectangle with opposite corners `a` and `b`: four lines round from `a`, held horizontal and vertical by turns.
    pub fn add_rectangle(&mut self, a: [f64; 2], b: [f64; 2], construction: bool) -> Result<[Id; 4]> {
        ensure!(a.iter().chain(&b).all(|v| v.is_finite()), "A rectangle needs finite corners");
        ensure!(
            (a[0] - b[0]).abs() > SAME_MM && (a[1] - b[1]).abs() > SAME_MM,
            "A rectangle needs its corners apart in both directions"
        );
        self.drawn(|s| {
            let p = [a, [b[0], a[1]], b, [a[0], b[1]]].map(|xy| s.place(xy));
            let lines = [0, 1, 2, 3].map(|k| s.draw(Geometry::Line { a: p[k], b: p[(k + 1) % 4] }, construction));
            s.constraints.extend([
                Constraint::Horizontal(p[0], p[1]),
                Constraint::Vertical(p[1], p[2]),
                Constraint::Horizontal(p[2], p[3]),
                Constraint::Vertical(p[3], p[0]),
            ]);
            Ok(lines)
        })
    }
    /// A circle about `centre` through `rim`.
    pub fn add_circle(&mut self, centre: [f64; 2], rim: [f64; 2], construction: bool) -> Result<Id> {
        ensure!(distance(centre, rim) > SAME_MM, "A circle needs a radius");
        self.drawn(|s| {
            let center = s.place(centre);
            let rim = s.point(rim);
            Ok(s.draw(Geometry::Circle { center, rim }, construction))
        })
    }
    /// The arc from `start` to `end` that passes through `through`.
    pub fn add_arc_through(&mut self, start: [f64; 2], end: [f64; 2], through: [f64; 2], construction: bool) -> Result<Id> {
        ensure!(distance(start, end) > SAME_MM, "An arc needs its ends apart");
        let c = circumcentre(start, end, through).context("The three points of an arc lie on a line")?;
        let angle = |p: [f64; 2]| (p[1] - c[1]).atan2(p[0] - c[0]);
        let to_end = (angle(end) - angle(start)).rem_euclid(TAU);
        let to_through = (angle(through) - angle(start)).rem_euclid(TAU);
        // An arc runs counter-clockwise from its start; one that would miss the third point runs the other way.
        let (from, to) = if to_through < to_end { (start, end) } else { (end, start) };
        self.drawn(|s| {
            let center = s.point(c);
            let (start, end) = (s.place(from), s.place(to));
            Ok(s.draw(Geometry::Arc { center, start, end }, construction))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn a_rectangle_is_four_lines_held_square_and_an_arc_passes_its_third_point() {
        let mut s = Sketch::default();
        let lines = s.add_rectangle([-2.0, -1.5], [2.0, 1.5], false).unwrap();
        assert_eq!((s.entities.len(), s.points.len(), s.constraints.len()), (4, 4, 4));
        let lengths: Vec<f64> = lines.iter().map(|id| s.curves_of(s.entities.iter().find(|e| e.id == *id).unwrap()).unwrap()[0].length()).collect();
        assert_eq!(lengths, [4.0, 3.0, 4.0, 3.0]);
        assert_eq!(s.profile_regions().unwrap().len(), 1);
        assert!((s.profile_regions().unwrap()[0].area() - 12.0).abs() < 1e-12);
        // A second rectangle sharing a corner reuses its point.
        s.add_rectangle([2.0, 1.5], [3.0, 2.5], true).unwrap();
        assert_eq!(s.points.len(), 7);
        let before = s.clone();
        assert!(s.add_rectangle([0.0, 0.0], [0.0, 3.0], false).is_err());
        assert_eq!(s, before);
        // Three points on a quarter circle: the arc through them is that quarter, either way round.
        for (a, b) in [([1.0, 0.0], [0.0, 1.0]), ([0.0, 1.0], [1.0, 0.0])] {
            let mut s = Sketch::default();
            let id = s.add_arc_through(a, b, [0.5f64.sqrt(), 0.5f64.sqrt()], false).unwrap();
            let arc = s.curves_of(s.entities.iter().find(|e| e.id == id).unwrap()).unwrap();
            assert!((arc[0].length() - PI / 2.0).abs() < 1e-12, "{}", arc[0].length());
        }
        let mut s = Sketch::default();
        assert!(s.add_arc_through([0.0, 0.0], [2.0, 0.0], [1.0, 0.0], false).is_err());
        assert!(s.entities.is_empty() && s.points.is_empty());
        assert_eq!(circumcentre([0.0, 0.0], [2.0, 0.0], [0.0, 2.0]), Some([1.0, 1.0]));
        let c = s.add_circle([1.0, 1.0], [1.0, 3.0], false).unwrap();
        assert!((s.curves_of(s.entities.iter().find(|e| e.id == c).unwrap()).unwrap()[0].length() - 4.0 * PI).abs() < 1e-12);
        let p = s.place([5.0, 5.0]);
        assert_eq!(s.place([5.0, 5.0 + 1e-9]), p);
        assert!(s.add_line(p, p, false).is_err());
    }
}
