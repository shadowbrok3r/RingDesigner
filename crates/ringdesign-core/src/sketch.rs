//! Editable millimeter sketches. Constraint solutions are candidates: a failed
//! solve never changes the source sketch. Angles are counterclockwise degrees.
use anyhow::{Result, ensure};
use cadkernel::{
    geom2d::{Arc, Circle, Curve, Line, NurbsCurve},
    space::Plane,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub mod anchor;
pub mod dimension;
pub mod draw;
pub mod edit;
pub mod exchange;
pub mod fill;
mod graph;
pub mod query;
pub mod region;
pub mod solid;
pub mod solver;
pub use dimension::{Held, Measure};
pub use edit::Pattern;
pub use query::Near;
pub use region::{Region, RegionRef};
pub use solid::FaceFrame;

pub type Id = u64;
/// Most points, and most entities, one sketch holds.
pub const MAX_ITEMS: usize = 1024;
/// Most constraints one sketch holds: a held rectangle carries six for its four points.
pub const MAX_CONSTRAINTS: usize = 2048;
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
        // Points are pushed in id order, so bisection finds one; a file written out of order is searched through.
        let found = match self.points.binary_search_by_key(&id, |p| p.id) {
            Ok(i) => Some(&self.points[i]),
            Err(_) => self.points.iter().find(|p| p.id == id),
        };
        found.map(|p| p.xy).ok_or_else(|| anyhow::anyhow!("Sketch point #{id} is missing"))
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
            self.points.len() <= MAX_ITEMS && self.entities.len() <= MAX_ITEMS && self.constraints.len() <= MAX_CONSTRAINTS,
            "Sketch exceeds {MAX_ITEMS} points or entities, or {MAX_CONSTRAINTS} constraints"
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
        solver::check(self, &solver::index_of(self))
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
    /// Solves every independent system of constraints apart ([`solver`]); refused, the sketch is unchanged.
    pub fn solve(&self) -> Result<Solution> {
        self.validate()?;
        solver::solve(self)
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
    /// `n` rectangles 2 x 1.5 mm, twenty to a row, each held square with its width and height dimensioned.
    pub(super) fn held_rectangles(n: usize) -> Sketch {
        let mut s = Sketch::default();
        for i in 0..n {
            let (x, y) = ((i % 20) as f64 * 3.0, (i / 20) as f64 * 3.0);
            let p = [[x, y], [x + 2.0, y], [x + 2.0, y + 1.5], [x, y + 1.5]].map(|xy| s.point(xy));
            for k in 0..4 {
                s.entity(Geometry::Line { a: p[k], b: p[(k + 1) % 4] });
            }
            s.constraints.extend([
                Constraint::Horizontal(p[0], p[1]),
                Constraint::Vertical(p[1], p[2]),
                Constraint::Horizontal(p[2], p[3]),
                Constraint::Vertical(p[3], p[0]),
                Constraint::Distance { a: p[0], b: p[1], mm: 2.0 },
                Constraint::Distance { a: p[1], b: p[2], mm: 1.5 },
            ]);
        }
        s
    }
    /// `n` points, 1 mm treads and risers alternating, the first fixed and the rest knocked by up to 0.2 mm.
    pub(super) fn staircase(n: usize) -> Sketch {
        let mut seed = 7u64;
        let mut noise = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.4
        };
        let mut s = Sketch::default();
        let mut exact = [0.0, 0.0];
        let mut prev = s.point(exact);
        s.points[0].fixed = true;
        for i in 0..n - 1 {
            exact[i % 2] += 1.0;
            let next = s.point([exact[0] + noise(), exact[1] + noise()]);
            s.constraints.push(if i % 2 == 0 { Constraint::Horizontal(prev, next) } else { Constraint::Vertical(prev, next) });
            s.constraints.push(Constraint::Distance { a: prev, b: next, mm: 1.0 });
            prev = next;
        }
        s
    }
    /// The solver's cost on held rectangles and a staircase; run `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_solver_at_scale() {
        let best = |f: &mut dyn FnMut() -> Result<String>| {
            let mut ms = f64::INFINITY;
            let mut out = String::new();
            for _ in 0..5 {
                let t = std::time::Instant::now();
                let r = f();
                ms = ms.min(t.elapsed().as_secs_f64() * 1e3);
                out = match r {
                    Ok(s) => s,
                    Err(e) => format!("refused: {e}"),
                };
            }
            (ms, out)
        };
        for n in [1, 30, 50, 200] {
            let s = held_rectangles(n);
            let (solve_ms, solved) = best(&mut || s.solve().map(|r| format!("{} DOF, {} iterations", r.remaining_dof, r.iterations)));
            let bottom = Measure::Length { a: s.points[0].id, b: s.points[1].id };
            let (edit_ms, edited) = best(&mut || {
                let mut t = s.clone();
                t.dimension(&bottom, 2.5).map(|h| format!("{} DOF, {} iterations", h.remaining_dof, h.iterations))
            });
            eprintln!("{n} held rectangles ({} points): solve {solve_ms:.3} ms ({solved}); a dimension {edit_ms:.3} ms ({edited})", s.points.len());
        }
        let s = held_rectangles(200);
        let (ms, regions) = best(&mut || s.profile_regions().map(|r| format!("{} regions", r.len())));
        eprintln!("200 held rectangles: profile_regions {ms:.3} ms ({regions})");
        // A corner of the last rectangle dragged, as the sketch tools hold it while they solve.
        let mut s = held_rectangles(200);
        let dragged = s.points.len() - 2;
        s.points[dragged].xy[0] += 0.3;
        s.points[dragged].fixed = true;
        let (ms, solved) = best(&mut || s.solve().map(|r| format!("{} DOF, {} iterations", r.remaining_dof, r.iterations)));
        eprintln!("200 held rectangles, one corner dragged 0.3 mm: solve {ms:.3} ms ({solved})");
        for n in [64, 512] {
            let s = staircase(n);
            let (ms, solved) = best(&mut || s.solve().map(|r| format!("residual {:.1e} mm, {} DOF, {} iterations", r.residual_mm, r.remaining_dof, r.iterations)));
            eprintln!("{n}-point staircase: solve {ms:.3} ms ({solved})");
        }
        let s = lattice(16, 32);
        let (ms, solved) = best(&mut || s.solve().map(|r| format!("residual {:.1e} mm, {} DOF, {} iterations", r.residual_mm, r.remaining_dof, r.iterations)));
        eprintln!("16 x 32 lattice ({} points, {} constraints): solve {ms:.3} ms ({solved})", s.points.len(), s.constraints.len());
    }
    /// A `w` x `h` grid of points 1 mm apart, every edge held level or plumb and 1 mm long, the first fixed and the rest knocked.
    pub(super) fn lattice(w: usize, h: usize) -> Sketch {
        let mut seed = 11u64;
        let mut noise = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.2
        };
        let mut s = Sketch::default();
        let mut ids = Vec::new();
        for j in 0..h {
            for i in 0..w {
                ids.push(s.point([i as f64 + noise(), j as f64 + noise()]));
            }
        }
        s.points[0].fixed = true;
        for j in 0..h {
            for i in 0..w {
                let here = ids[j * w + i];
                if i + 1 < w {
                    let next = ids[j * w + i + 1];
                    s.constraints.extend([Constraint::Horizontal(here, next), Constraint::Distance { a: here, b: next, mm: 1.0 }]);
                }
                if j + 1 < h {
                    let up = ids[(j + 1) * w + i];
                    s.constraints.extend([Constraint::Vertical(here, up), Constraint::Distance { a: here, b: up, mm: 1.0 }]);
                }
            }
        }
        s
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
