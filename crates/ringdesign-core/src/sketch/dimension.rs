//! Dimensions: what a pick on a sketch measures, and holding it at a typed value with distance
//! constraints and a solve.
use super::{Constraint, Geometry, Id, Sketch, distance};
use anyhow::{Result, bail, ensure};

/// What a dimension holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Measure {
    /// A line's, or one polyline segment's, length.
    Length { a: Id, b: Id },
    /// The distance between two points.
    Distance { a: Id, b: Id },
    /// A circle's or an arc's radius, held at each of its rim points.
    Radius { center: Id, rim: Vec<Id> },
}

/// What holding a dimension took.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub residual_mm: f64,
    pub remaining_dof: usize,
    pub iterations: usize,
}

impl Measure {
    /// The word a dimension field is named by.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Length { .. } => "Length",
            Self::Distance { .. } => "Distance",
            Self::Radius { .. } => "Radius",
        }
    }
    /// The point pairs its distance constraints join.
    pub fn pairs(&self) -> Vec<(Id, Id)> {
        match self {
            Self::Length { a, b } | Self::Distance { a, b } => vec![(*a, *b)],
            Self::Radius { center, rim } => rim.iter().map(|r| (*center, *r)).collect(),
        }
    }
    /// Every point it names.
    pub fn points(&self) -> Vec<Id> {
        self.pairs().into_iter().flat_map(|(a, b)| [a, b]).collect()
    }
}

impl Sketch {
    /// What dimensioning `entity` near `at` holds: a line's or nearest polyline segment's length, or a radius.
    pub fn measure_of(&self, entity: Id, at: [f64; 2]) -> Result<Measure> {
        let Some(e) = self.entities.iter().find(|e| e.id == entity) else {
            bail!("Sketch entity #{entity} is missing");
        };
        Ok(match &e.geometry {
            Geometry::Line { a, b } => Measure::Length { a: *a, b: *b },
            Geometry::Polyline { points, closed } => {
                let n = points.len();
                let segments = if *closed { n } else { n.saturating_sub(1) };
                let near = |k: usize| -> f64 {
                    let (p, q) = (self.at(points[k]).unwrap_or(at), self.at(points[(k + 1) % n]).unwrap_or(at));
                    let d = [q[0] - p[0], q[1] - p[1]];
                    let t = (((at[0] - p[0]) * d[0] + (at[1] - p[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-18)).clamp(0.0, 1.0);
                    distance(at, [p[0] + d[0] * t, p[1] + d[1] * t])
                };
                let k = (0..segments).min_by(|x, y| near(*x).total_cmp(&near(*y)));
                let Some(k) = k else { bail!("Polyline #{entity} has no segment to dimension") };
                Measure::Length { a: points[k], b: points[(k + 1) % n] }
            }
            Geometry::Circle { center, rim } => Measure::Radius { center: *center, rim: vec![*rim] },
            Geometry::Arc { center, start, end } => Measure::Radius { center: *center, rim: vec![*start, *end] },
            Geometry::Bezier { .. } => bail!("A spline has no single length to dimension; dimension two of its points instead"),
        })
    }
    /// The value a measure reads now, in millimetres.
    pub fn measured(&self, m: &Measure) -> Result<f64> {
        let (a, b) = m.pairs().first().copied().ok_or_else(|| anyhow::anyhow!("A radius needs a rim point"))?;
        Ok(distance(self.at(a)?, self.at(b)?))
    }
    /// Holds `m` at `mm` with distance constraints in place of any between the same points, then solves; refused, nothing changes.
    pub fn dimension(&mut self, m: &Measure, mm: f64) -> Result<Held> {
        ensure!(mm.is_finite() && mm > 1e-6 && mm < 10000.0, "A dimension needs a length between 0.000001 and 10000 mm");
        let pairs = m.pairs();
        ensure!(!pairs.is_empty() && pairs.iter().all(|(a, b)| a != b), "A dimension needs two different points");
        let mut next = self.clone();
        let same = |a: Id, b: Id| pairs.iter().any(|(p, q)| (a, b) == (*p, *q) || (a, b) == (*q, *p));
        next.constraints.retain(|c| !matches!(c, Constraint::Distance { a, b, .. } if same(*a, *b)));
        next.constraints.extend(pairs.iter().map(|&(a, b)| Constraint::Distance { a, b, mm }));
        let solved = next.solve()?;
        solved.sketch.validate()?;
        let held = Held { residual_mm: solved.residual_mm, remaining_dof: solved.remaining_dof, iterations: solved.iterations };
        *self = solved.sketch;
        Ok(held)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dimension_holds_a_side_a_radius_or_a_span_and_a_conflict_changes_nothing() {
        let mut s = Sketch::default();
        let lines = s.add_rectangle([-2.0, -1.5], [2.0, 1.5], false).unwrap();
        let bottom = s.measure_of(lines[0], [0.0, -1.5]).unwrap();
        assert!(matches!(bottom, Measure::Length { .. }));
        assert_eq!(s.measured(&bottom).unwrap(), 4.0);
        let held = s.dimension(&bottom, 3.0).unwrap();
        assert!(held.residual_mm < 1e-6, "{held:?}");
        assert!((s.measured(&bottom).unwrap() - 3.0).abs() < 1e-5);
        // The top follows the bottom and the sides keep their 3 mm.
        let top = s.measure_of(lines[2], [0.0, 1.5]).unwrap();
        let side = s.measure_of(lines[1], [2.0, 0.0]).unwrap();
        assert!((s.measured(&top).unwrap() - 3.0).abs() < 1e-5 && (s.measured(&side).unwrap() - 3.0).abs() < 1e-5);
        // Dimensioning it again replaces the constraint rather than stacking a second.
        s.dimension(&bottom, 2.5).unwrap();
        let held: Vec<_> = s.constraints.iter().filter(|c| matches!(c, Constraint::Distance { .. })).collect();
        assert_eq!(held.len(), 1);
        assert!((s.measured(&bottom).unwrap() - 2.5).abs() < 1e-5);
        // A radius on an arc holds both of its ends.
        let arc = s.add_arc_through([5.0, 0.0], [7.0, 0.0], [6.0, 1.0], false).unwrap();
        let r = s.measure_of(arc, [6.0, 1.0]).unwrap();
        assert!(matches!(&r, Measure::Radius { rim, .. } if rim.len() == 2));
        s.dimension(&r, 1.5).unwrap();
        assert!((s.measured(&r).unwrap() - 1.5).abs() < 1e-5);
        assert!(s.validate().is_ok(), "both ends of the arc stay on its circle");
        // Two fixed points cannot move to meet a dimension: refused, nothing changes.
        let mut t = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 0.0]].map(|xy| t.point(xy));
        t.entity(Geometry::Line { a: p[0], b: p[1] });
        t.points.iter_mut().for_each(|p| p.fixed = true);
        let before = t.clone();
        assert!(t.dimension(&Measure::Distance { a: p[0], b: p[1] }, 2.0).is_err());
        assert_eq!(t, before);
        assert!(t.dimension(&Measure::Distance { a: p[0], b: p[0] }, 2.0).is_err());
        assert!(t.dimension(&Measure::Distance { a: p[0], b: p[1] }, -1.0).is_err());
    }
}
