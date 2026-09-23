//! Editable millimeter sketches. Constraint solutions are candidates: a failed
//! solve never changes the source sketch. Angles are counterclockwise degrees.
use anyhow::{Result, bail, ensure};
use cadkernel::{
    geom2d::{Arc, Circle, Curve, Line, NurbsCurve},
    space::Plane,
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub mod anchor;
pub mod dimension;
pub mod draw;
pub mod edit;
pub mod exchange;
pub mod fill;
pub mod query;
pub mod region;
pub mod solid;
pub use dimension::{Held, Measure};
pub use edit::Pattern;
pub use query::Near;
pub use region::Region;
pub use solid::FaceFrame;

pub type Id = u64;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workplane {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    /// Laid on a planar face of an earlier feature: `origin`, `x` and `y` are then read in the
    /// face's own frame ([`FaceFrame`]: origin at its centroid, x along its longest straight
    /// edge, z out of the solid), so the default workplane lies on the face at its centroid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_face: Option<FaceAnchor>,
}
/// A planar face of an earlier feature that a sketch lies on.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FaceAnchor {
    pub feature: Id,
    pub face: crate::cad::FaceRef,
}
impl Default for Workplane {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            on_face: None,
        }
    }
}
impl Workplane {
    pub fn plane(&self) -> Result<Plane> {
        ensure!(
            self.origin
                .iter()
                .chain(&self.x)
                .chain(&self.y)
                .all(|v| v.is_finite() && v.abs() < 1e6),
            "Plane contains invalid coordinates"
        );
        let p = Plane::from_axes(self.origin, self.x, self.y);
        ensure!(
            p.is_orthonormal(),
            "Workplane axes must be unit length and perpendicular"
        );
        Ok(p)
    }
    pub fn section() -> Self {
        Self {
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, 1.0],
            ..Default::default()
        }
    }
    /// This workplane laid onto the plane through `point` with `normal`: the origin projected
    /// along the normal, `x` flattened into the plane, `y` following the normal's hand.
    pub fn on(&self, point: [f64; 3], normal: [f64; 3]) -> Result<Plane> {
        let len = normal.iter().map(|v| v * v).sum::<f64>().sqrt();
        ensure!(len > 1e-9, "Sketch face has no normal");
        let n = normal.map(|v| v / len);
        let dot = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        let off = dot(std::array::from_fn(|k| self.origin[k] - point[k]), n);
        let origin: [f64; 3] = std::array::from_fn(|k| self.origin[k] - n[k] * off);
        let lift = dot(self.x, n);
        let mut x: [f64; 3] = std::array::from_fn(|k| self.x[k] - n[k] * lift);
        if dot(x, x) < 1e-12 {
            x = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
            let lift = dot(x, n);
            x = std::array::from_fn(|k| x[k] - n[k] * lift);
        }
        let xl = dot(x, x).sqrt();
        let x = x.map(|v| v / xl);
        let y = [n[1] * x[2] - n[2] * x[1], n[2] * x[0] - n[0] * x[2], n[0] * x[1] - n[1] * x[0]];
        let p = Plane::from_axes(origin, x, y);
        ensure!(p.is_orthonormal(), "Sketch face frame is degenerate");
        Ok(p)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub id: Id,
    pub xy: [f64; 2],
    #[serde(default)]
    pub fixed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Geometry {
    Line { a: Id, b: Id },
    Polyline { points: Vec<Id>, closed: bool },
    Circle { center: Id, rim: Id },
    Arc { center: Id, start: Id, end: Id },
    Bezier { points: [Id; 4] },
}
impl Geometry {
    /// Every point the geometry names.
    pub fn points(&self) -> Vec<Id> {
        match self {
            Self::Line { a, b } => vec![*a, *b],
            Self::Polyline { points, .. } => points.clone(),
            Self::Circle { center, rim } => vec![*center, *rim],
            Self::Arc { center, start, end } => vec![*center, *start, *end],
            Self::Bezier { points } => points.to_vec(),
        }
    }
    /// The two ends a neighbour joins; `None` for geometry that closes on itself.
    fn ends(&self) -> Option<(Id, Id)> {
        match self {
            Self::Line { a, b } => Some((*a, *b)),
            Self::Polyline { closed: true, .. } | Self::Circle { .. } => None,
            Self::Polyline { points, .. } => Some((*points.first()?, *points.last()?)),
            Self::Arc { start, end, .. } => Some((*start, *end)),
            Self::Bezier { points } => Some((points[0], points[3])),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub id: Id,
    #[serde(default)]
    pub construction: bool,
    pub geometry: Geometry,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Constraint {
    Horizontal(Id, Id),
    Vertical(Id, Id),
    Coincident(Id, Id),
    Distance {
        a: Id,
        b: Id,
        mm: f64,
    },
    Symmetry {
        a: Id,
        b: Id,
        center: Id,
    },
    /// The line a–b is perpendicular to the radius center–at.
    Tangent {
        a: Id,
        b: Id,
        center: Id,
        at: Id,
    },
}
impl Constraint {
    /// Every point the constraint names.
    pub fn points(&self) -> Vec<Id> {
        match *self {
            Self::Horizontal(a, b) | Self::Vertical(a, b) | Self::Coincident(a, b) => vec![a, b],
            Self::Distance { a, b, .. } => vec![a, b],
            Self::Symmetry { a, b, center } => vec![a, b, center],
            Self::Tangent { a, b, center, at } => vec![a, b, center, at],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sketch {
    pub name: String,
    pub plane: Workplane,
    pub points: Vec<Point>,
    pub entities: Vec<Entity>,
    pub constraints: Vec<Constraint>,
    pub next_id: Id,
    pub grid_mm: f64,
}
impl Default for Sketch {
    fn default() -> Self {
        Self {
            name: "Sketch".into(),
            plane: Workplane::default(),
            points: vec![],
            entities: vec![],
            constraints: vec![],
            next_id: 1,
            grid_mm: 0.5,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Solution {
    pub sketch: Sketch,
    pub residual_mm: f64,
    pub remaining_dof: usize,
    pub iterations: usize,
}

impl Sketch {
    fn id(&mut self) -> Id {
        self.next_id = self.next_id.max(
            self.points
                .iter()
                .map(|p| p.id)
                .chain(self.entities.iter().map(|e| e.id))
                .max()
                .unwrap_or(0)
                + 1,
        );
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn point(&mut self, xy: [f64; 2]) -> Id {
        let id = self.id();
        self.points.push(Point {
            id,
            xy,
            fixed: false,
        });
        id
    }
    pub fn entity(&mut self, geometry: Geometry) -> Id {
        let id = self.id();
        self.entities.push(Entity {
            id,
            construction: false,
            geometry,
        });
        id
    }
    pub fn at(&self, id: Id) -> Result<[f64; 2]> {
        self.points
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.xy)
            .ok_or_else(|| anyhow::anyhow!("Sketch point #{id} is missing"))
    }
    pub fn rectangle(width: f64, height: f64) -> Self {
        let mut s = Self::default();
        s.name = "Rectangle".into();
        let ids = [
            [-width / 2.0, -height / 2.0],
            [width / 2.0, -height / 2.0],
            [width / 2.0, height / 2.0],
            [-width / 2.0, height / 2.0],
        ]
        .map(|p| s.point(p));
        s.entity(Geometry::Polyline {
            points: ids.to_vec(),
            closed: true,
        });
        s.constraints = vec![
            Constraint::Horizontal(ids[0], ids[1]),
            Constraint::Vertical(ids[1], ids[2]),
            Constraint::Horizontal(ids[2], ids[3]),
            Constraint::Vertical(ids[3], ids[0]),
            Constraint::Distance {
                a: ids[0],
                b: ids[1],
                mm: width,
            },
            Constraint::Distance {
                a: ids[1],
                b: ids[2],
                mm: height,
            },
        ];
        s.points[0].fixed = true;
        s
    }
    pub fn circle(radius: f64) -> Self {
        let mut s = Self::default();
        s.name = "Circle".into();
        let center = s.point([0.0, 0.0]);
        let rim = s.point([radius, 0.0]);
        s.entity(Geometry::Circle { center, rim });
        s.points[0].fixed = true;
        s.constraints.push(Constraint::Distance {
            a: center,
            b: rim,
            mm: radius,
        });
        s
    }
    pub fn validate(&self) -> Result<()> {
        self.plane.plane()?;
        ensure!(
            self.points.len() <= 512 && self.entities.len() <= 512 && self.constraints.len() <= 512,
            "Sketch exceeds 512 points, entities, or constraints"
        );
        ensure!(
            self.grid_mm.is_finite() && self.grid_mm > 0.0,
            "Grid spacing must be positive"
        );
        let mut ids = BTreeSet::new();
        for p in &self.points {
            ensure!(ids.insert(p.id), "Duplicate sketch identity");
            ensure!(
                p.xy.iter().all(|v| v.is_finite() && v.abs() < 10000.0),
                "Invalid sketch coordinate"
            );
        }
        for e in &self.entities {
            ensure!(ids.insert(e.id), "Duplicate sketch identity");
            self.curves_of(e)?;
        }
        self.residuals()?;
        Ok(())
    }
    pub fn curves_of(&self, e: &Entity) -> Result<Vec<Curve>> {
        let line = |a, b| -> Result<Curve> {
            let start = self.at(a)?;
            let end = self.at(b)?;
            ensure!(distance(start, end) > 1e-8, "Zero-length sketch line");
            Ok(Curve::Line(Line { start, end }))
        };
        Ok(match &e.geometry {
            Geometry::Line { a, b } => vec![line(*a, *b)?],
            Geometry::Polyline { points, closed } => {
                ensure!(
                    points.len() >= 2 && points.len() <= 512,
                    "A polyline needs 2–512 points"
                );
                let mut c = Vec::new();
                for p in points.windows(2) {
                    c.push(line(p[0], p[1])?);
                }
                if *closed {
                    c.push(line(*points.last().unwrap(), points[0])?);
                }
                c
            }
            Geometry::Circle { center, rim } => {
                let centre = self.at(*center)?;
                let radius = distance(centre, self.at(*rim)?);
                ensure!(radius > 1e-8, "Circle radius must be positive");
                vec![Curve::Circle(Circle { centre, radius })]
            }
            Geometry::Arc { center, start, end } => {
                let centre = self.at(*center)?;
                let a = self.at(*start)?;
                let b = self.at(*end)?;
                let radius = distance(centre, a);
                ensure!(
                    radius > 1e-8 && (radius - distance(centre, b)).abs() < 1e-4,
                    "Arc endpoints must lie on the same circle"
                );
                vec![Curve::Arc(Arc {
                    centre,
                    radius,
                    start_angle: (a[1] - centre[1]).atan2(a[0] - centre[0]),
                    end_angle: (b[1] - centre[1]).atan2(b[0] - centre[0]),
                })]
            }
            Geometry::Bezier { points } => {
                let points = points
                    .iter()
                    .map(|id| self.at(*id))
                    .collect::<Result<Vec<_>>>()?;
                vec![Curve::Nurbs(
                    NurbsCurve::new(3, points, vec![0., 0., 0., 0., 1., 1., 1., 1.], None)
                        .ok_or_else(|| anyhow::anyhow!("Invalid cubic curve"))?,
                )]
            }
        })
    }
    pub fn curves(&self) -> Result<Vec<Curve>> {
        self.validate()?;
        let mut out = Vec::new();
        for e in self.entities.iter().filter(|e| !e.construction) {
            out.extend(self.curves_of(e)?);
        }
        ensure!(!out.is_empty(), "Sketch has no profile geometry");
        Ok(out)
    }
    /// Solve dimensions before constructing a solid. Source positions remain
    /// editable; the solver must converge before geometry is accepted.
    pub fn solved_curves(&self) -> Result<Vec<Curve>> {
        self.solve()?.sketch.curves()
    }
    /// Remove one entity, then every point nothing names any more.
    pub fn remove_entity(&mut self, id: Id) {
        self.entities.retain(|e| e.id != id);
        self.prune_points();
    }
    /// Remove one point with every entity and constraint that names it.
    pub fn remove_point(&mut self, id: Id) {
        self.entities.retain(|e| !e.geometry.points().contains(&id));
        self.constraints.retain(|c| !c.points().contains(&id));
        self.points.retain(|p| p.id != id);
        self.prune_points();
    }
    fn prune_points(&mut self) {
        let named: BTreeSet<Id> = self
            .entities
            .iter()
            .flat_map(|e| e.geometry.points())
            .collect();
        self.constraints
            .retain(|c| c.points().iter().all(|id| named.contains(id)));
        self.points.retain(|p| named.contains(&p.id));
    }
    /// The solved profile as one closed loop in joining order, or the reason it is not one.
    pub fn profile_curves(&self) -> Result<Vec<Curve>> {
        let mut loops = region::loops(&self.solve()?.sketch)?;
        ensure!(
            loops.len() == 1,
            "Sketch has {} separate loops; a profile is one closed loop. Delete the others or mark them Construction",
            loops.len()
        );
        Ok(loops.remove(0).curves)
    }
    fn residuals(&self) -> Result<Vec<f64>> {
        let mut r = Vec::new();
        for c in &self.constraints {
            match *c {
                Constraint::Horizontal(a, b) => r.push(self.at(a)?[1] - self.at(b)?[1]),
                Constraint::Vertical(a, b) => r.push(self.at(a)?[0] - self.at(b)?[0]),
                Constraint::Coincident(a, b) => {
                    let a = self.at(a)?;
                    let b = self.at(b)?;
                    r.extend([a[0] - b[0], a[1] - b[1]]);
                }
                Constraint::Distance { a, b, mm } => {
                    ensure!(
                        mm.is_finite() && mm >= 0.0 && mm < 10000.0,
                        "Invalid dimensional constraint"
                    );
                    r.push(distance(self.at(a)?, self.at(b)?) - mm);
                }
                Constraint::Symmetry { a, b, center } => {
                    let a = self.at(a)?;
                    let b = self.at(b)?;
                    let c = self.at(center)?;
                    r.extend([(a[0] + b[0]) * 0.5 - c[0], (a[1] + b[1]) * 0.5 - c[1]]);
                }
                Constraint::Tangent { a, b, center, at } => {
                    let a = self.at(a)?;
                    let b = self.at(b)?;
                    let c = self.at(center)?;
                    let p = self.at(at)?;
                    let len = distance(a, b).max(1e-8);
                    r.push(((b[0] - a[0]) * (p[0] - c[0]) + (b[1] - a[1]) * (p[1] - c[1])) / len);
                    r.push(((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / len);
                }
            }
        }
        Ok(r)
    }
    pub fn solve(&self) -> Result<Solution> {
        self.validate()?;
        let mut s = self.clone();
        // A point no constraint names has no residual to move it, so it is no variable.
        let named: BTreeSet<Id> = s.constraints.iter().flat_map(Constraint::points).collect();
        let loose = s.points.iter().filter(|p| !p.fixed && !named.contains(&p.id)).count();
        let vars: Vec<_> = s
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.fixed && named.contains(&p.id))
            .flat_map(|(i, _)| [(i, 0), (i, 1)])
            .collect();
        ensure!(
            vars.len() <= 256,
            "Constraint solver supports 128 constrained movable points per sketch"
        );
        for iteration in 0..80 {
            let r = DVector::from_vec(s.residuals()?);
            let residual = r.amax();
            let mut jac = DMatrix::zeros(r.len(), vars.len());
            for (j, &(point, axis)) in vars.iter().enumerate() {
                s.points[point].xy[axis] += 1e-5;
                let perturbed = s.residuals()?;
                s.points[point].xy[axis] -= 1e-5;
                for k in 0..r.len() {
                    jac[(k, j)] = (perturbed[k] - r[k]) / 1e-5;
                }
            }
            if residual < 1e-6 {
                let rank = if jac.nrows() == 0 || jac.ncols() == 0 {
                    0
                } else {
                    jac.svd(false, false).rank(1e-5)
                };
                return Ok(Solution {
                    sketch: s,
                    residual_mm: residual,
                    remaining_dof: vars.len().saturating_sub(rank) + 2 * loose,
                    iterations: iteration,
                });
            }
            if vars.is_empty() {
                break;
            }
            let jt = jac.transpose();
            let normal = &jt * &jac + DMatrix::identity(vars.len(), vars.len()) * 1e-7;
            let Some(delta) = normal.lu().solve(&(-jt * r)) else {
                bail!("Constraint system is singular");
            };
            let old = s.clone();
            let score = residual;
            let mut accepted = false;
            for factor in [1.0, 0.5, 0.25, 0.125, 0.0625] {
                s = old.clone();
                for (j, &(point, axis)) in vars.iter().enumerate() {
                    s.points[point].xy[axis] += delta[j] * factor;
                }
                if DVector::from_vec(s.residuals()?).amax() < score {
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                break;
            }
        }
        bail!(
            "Constraints conflict or did not converge (residual {:.6} mm); original sketch is unchanged",
            DVector::from_vec(s.residuals()?).amax()
        )
    }
    /// Candidate snaps are geometric aids; using one does not silently add a constraint.
    pub fn snap(&self, cursor: [f64; 2], anchor: Option<[f64; 2]>, radius: f64) -> Option<Snap> {
        let mut candidates: Vec<Snap> = self
            .points
            .iter()
            .map(|p| Snap {
                xy: p.xy,
                kind: "Endpoint",
            })
            .collect();
        if self.grid_mm.is_finite() && self.grid_mm > 0.0 {
            candidates.push(Snap {
                xy: cursor.map(|v| (v / self.grid_mm).round() * self.grid_mm),
                kind: "Grid",
            });
        }
        for e in &self.entities {
            for c in self.curves_of(e).unwrap_or_default() {
                match c {
                    Curve::Line(l) => {
                        candidates.push(Snap {
                            xy: [(l.start[0] + l.end[0]) * 0.5, (l.start[1] + l.end[1]) * 0.5],
                            kind: "Midpoint",
                        });
                        if let Some(a) = anchor {
                            let d = [l.end[0] - l.start[0], l.end[1] - l.start[1]];
                            let t = ((a[0] - l.start[0]) * d[0] + (a[1] - l.start[1]) * d[1])
                                / (d[0] * d[0] + d[1] * d[1]);
                            candidates.push(Snap {
                                xy: [l.start[0] + t * d[0], l.start[1] + t * d[1]],
                                kind: "Perpendicular",
                            });
                        }
                    }
                    Curve::Circle(c) => {
                        candidates.push(Snap {
                            xy: c.centre,
                            kind: "Center",
                        });
                        if let Some(a) = anchor {
                            let d = distance(a, c.centre);
                            if d > c.radius {
                                let base = (a[1] - c.centre[1]).atan2(a[0] - c.centre[0]);
                                let offset = (c.radius / d).acos();
                                for angle in [base - offset, base + offset] {
                                    candidates.push(Snap {
                                        xy: [
                                            c.centre[0] + c.radius * angle.cos(),
                                            c.centre[1] + c.radius * angle.sin(),
                                        ],
                                        kind: "Tangent",
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        candidates
            .into_iter()
            .filter(|s| distance(s.xy, cursor) <= radius)
            .min_by(|a, b| distance(a.xy, cursor).total_cmp(&distance(b.xy, cursor)))
    }
}
#[derive(Clone, Debug)]
pub struct Snap {
    pub xy: [f64; 2],
    pub kind: &'static str,
}
pub fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimensional_rectangle_and_conflicting_fixed_points() {
        let mut s = Sketch::rectangle(8.0, 6.0);
        s.points[1].xy = [5.0, -2.0];
        let solved = s.solve().unwrap();
        assert!(solved.residual_mm < 1e-6);
        assert_eq!(solved.remaining_dof, 0);
        assert_ne!(s, solved.sketch);
        s.points.iter_mut().for_each(|p| p.fixed = true);
        let before = s.clone();
        assert!(s.solve().is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn tangent_snap_is_on_circle_and_perpendicular_to_sightline() {
        let s = Sketch::circle(2.0);
        let a = [4.0, 0.0];
        let snap = s.snap([1.0, 1.73], Some(a), 0.02).unwrap();
        assert_eq!(snap.kind, "Tangent");
        assert!((distance(snap.xy, [0.0, 0.0]) - 2.0).abs() < 1e-9);
        assert!(((a[0] - snap.xy[0]) * snap.xy[0] + (a[1] - snap.xy[1]) * snap.xy[1]).abs() < 1e-9);
    }
    #[test]
    fn a_profile_names_its_loops_and_joins_lines_drawn_in_any_order() {
        let mut s = Sketch::rectangle(8.0, 6.0);
        let p = [[10.0, 0.0], [14.0, 0.0], [12.0, 3.0]].map(|p| s.point(p));
        s.entity(Geometry::Line { a: p[0], b: p[1] });
        s.entity(Geometry::Line { a: p[0], b: p[2] });
        let open = s.profile_curves().err().unwrap().to_string();
        assert!(open.contains("open at point"), "{open}");
        s.entity(Geometry::Line { a: p[2], b: p[1] });
        let two = s.profile_curves().err().unwrap().to_string();
        assert!(two.contains("2 separate loops"), "{two}");
        let rectangle = s.entities[0].id;
        s.remove_entity(rectangle);
        assert_eq!(s.points.len(), 3);
        assert!(s.constraints.is_empty());
        assert_eq!(s.profile_curves().unwrap().len(), 3);
        assert_eq!(Sketch::circle(2.0).profile_curves().unwrap().len(), 2, "a circle sweeps as two arcs");
        s.remove_point(p[2]);
        assert_eq!((s.points.len(), s.entities.len()), (2, 1));
    }
    #[test]
    fn loose_points_are_no_solver_variables_but_still_count_as_freedom() {
        // A rectangle plus a free point: the point adds two degrees of freedom and no variables.
        let mut s = Sketch::rectangle(8.0, 6.0);
        s.point([20.0, 20.0]);
        let solved = s.solve().unwrap();
        assert_eq!((solved.remaining_dof, solved.iterations), (2, 0));
        // Three hundred unconstrained points solve at once, where the cap once counted every one.
        let mut many = Sketch::default();
        let ids: Vec<Id> = (0..300).map(|i| many.point([(i as f64 * 0.1).cos() * 5.0, (i as f64 * 0.1).sin() * 5.0])).collect();
        many.entity(Geometry::Polyline { points: ids, closed: false });
        assert_eq!(many.solve().unwrap().remaining_dof, 600);
        // One region with no holes is the single loop the old profile gave, curve for curve.
        for s in [Sketch::rectangle(3.0, 2.0), Sketch::circle(1.5)] {
            assert_eq!(s.profile_region().unwrap().outer, s.profile_curves().unwrap());
        }
    }
    #[test]
    fn workplane_roundtrip_and_sketch_persistence() {
        let s = Sketch::rectangle(3.0, 4.0);
        let p = Workplane::section().plane().unwrap();
        assert_eq!(p.project(p.point_at([2.0, 3.0])), Some([2.0, 3.0]));
        assert_eq!(
            s,
            serde_json::from_str::<Sketch>(&serde_json::to_string(&s).unwrap()).unwrap()
        );
    }
}
