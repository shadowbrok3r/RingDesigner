//! The closed loops a sketch draws and the regions they bound, nested even-odd; where its curves
//! branch, the faces they divide the plane into.
use super::graph::{Graph, Key};
use super::{Id, Sketch};
use anyhow::{Result, bail, ensure};
use cadkernel::geom2d::{self, Arc, Curve, Tolerance};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};

/// Linear tolerance loops are tested against each other at: the kernel's own for region loops.
pub const CROSSING_MM: f64 = 1e-9;
/// How far a crossing may sit from a shared corner and still be that corner, and how near another
/// curve an end must lie to join it: the chain key's grid.
pub(super) const JOINT_MM: f64 = 1e-6;

/// One closed loop in joining order.
#[derive(Clone, Debug)]
pub struct Loop {
    pub curves: Vec<Curve>,
    /// Whether each curve is walked from its start to its end.
    pub forward: Vec<bool>,
    /// The entity each curve came from.
    pub entities: Vec<Id>,
}
impl Loop {
    /// Signed area, positive counter-clockwise.
    pub fn area(&self) -> f64 {
        walked_moments(&self.curves, &self.forward, [0.0; 2])[0]
    }
    /// The curve walked from its start, the start of curve `i`.
    fn start(&self, i: usize) -> [f64; 2] {
        self.curves[i].point_at(if self.forward[i] { 0.0 } else { 1.0 })
    }
    /// Where curve `i` ends as walked.
    fn end(&self, i: usize) -> [f64; 2] {
        self.curves[i].point_at(if self.forward[i] { 1.0 } else { 0.0 })
    }
}

/// A bounded area of a sketch: an outer loop less the loops directly inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub outer: Vec<Curve>,
    pub holes: Vec<Vec<Curve>>,
    /// The entities bounding it, outer loop first, each once.
    pub entities: Vec<Id>,
    /// How many of `entities` run round the outer loop.
    pub rim: usize,
}

/// One region of a sketch, named so it survives edits: the region whose outer loop runs through
/// `entity`, else the one holding `at`. An entity outlives a dimension that moves or resizes its
/// loop, and the point outlives an entity trimmed away and redrawn; a loop that has become a
/// hole matches neither, and is refused rather than taken for the region round it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegionRef {
    /// An entity on the region's outer loop.
    pub entity: Id,
    /// A point inside the region, in the sketch's own coordinates.
    pub at: [f64; 2],
}

impl RegionRef {
    /// The name of `region` with `at` inside it: its outer loop's first entity and that point.
    pub fn at(region: &Region, at: [f64; 2]) -> Option<Self> {
        (region.contains(at) && region.rim > 0).then(|| Self { entity: region.entities[0], at })
    }
    /// The name of `region` with a point well inside it.
    pub fn of(region: &Region) -> Option<Self> {
        Self::at(region, region.inside()?)
    }
    /// The name of region `i` of `regions` with `at` inside it: the first entity of its outer loop no
    /// other region's outer loop runs through, so regions sharing a curve keep names of their own.
    pub fn among(regions: &[Region], i: usize, at: [f64; 2]) -> Option<Self> {
        let r = regions.get(i)?;
        let mut name = Self::at(r, at)?;
        let own = |e: &Id| regions.iter().enumerate().all(|(j, o)| j == i || !o.entities[..o.rim].contains(e));
        if let Some(e) = r.entities[..r.rim].iter().find(|e| own(e)) {
            name.entity = *e;
        }
        Some(name)
    }
    /// Which of `regions` this names, and whether only its point found it; of several regions whose
    /// outer loops run through the entity, the one holding the point, else the nearest to it.
    pub fn position(&self, regions: &[Region]) -> Option<(usize, bool)> {
        let rims: Vec<usize> = (0..regions.len()).filter(|&i| regions[i].entities[..regions[i].rim].contains(&self.entity)).collect();
        if let [i] = rims.as_slice() {
            return Some((*i, false));
        }
        if let Some(&i) = rims.iter().find(|&&i| regions[i].contains(self.at)) {
            return Some((i, false));
        }
        let off = |i: usize| regions[i].outer.iter().map(|c| geom2d::distance_to(c, self.at)).fold(f64::INFINITY, f64::min);
        if let Some(&i) = rims.iter().min_by(|a, b| off(**a).total_cmp(&off(**b))) {
            return Some((i, false));
        }
        regions.iter().position(|r| r.contains(self.at)).map(|i| (i, true))
    }
    /// The region of `regions` this names, and a note when only its point found it.
    pub fn find(&self, mut regions: Vec<Region>) -> Result<(Region, Option<String>)> {
        let [x, y] = self.at;
        match self.position(&regions) {
            Some((i, false)) => Ok((regions.swap_remove(i), None)),
            Some((i, true)) => {
                let note = format!("Region found again by its point ({x:.3}, {y:.3}): #{} no longer runs round one", self.entity);
                Ok((regions.swap_remove(i), Some(note)))
            }
            None => bail!("No region of the sketch runs round #{} or holds ({x:.3}, {y:.3}); pick the region again", self.entity),
        }
    }
}

impl Region {
    /// Every loop, outer first, as the kernel's region builders take them.
    pub fn loops(&self) -> Vec<Vec<Curve>> {
        std::iter::once(self.outer.clone()).chain(self.holes.iter().cloned()).collect()
    }
    /// Area inside the outer loop less its holes, in mm².
    pub fn area(&self) -> f64 {
        let area = |c: &[Curve]| loop_moments(c, [0.0; 2]).map_or(0.0, |m| m[0].abs());
        area(&self.outer) - self.holes.iter().map(|h| area(h)).sum::<f64>()
    }
    /// Whether `p` lies in the region: inside the outer loop and in no hole, boundaries included.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        let tol = Tolerance::new(CROSSING_MM);
        let on = |c: &[Curve]| c.iter().any(|c| geom2d::distance_to(c, p) <= CROSSING_MM);
        geom2d::contains(&self.outer, p, tol) && self.holes.iter().all(|h| on(h) || !geom2d::contains(h, p, tol))
    }
}

impl Sketch {
    /// Every region the solved profile bounds, which a pick chooses among: closed loops nested
    /// even-odd, so a loop inside a loop is a hole and a loop inside that hole is a region of its
    /// own; where the curves branch, each face they close off, so a line across a rectangle makes
    /// two. An end lying on another curve joins it there. Curves that cross or touch elsewhere are
    /// refused with the place they meet.
    pub fn profile_regions(&self) -> Result<Vec<Region>> {
        let graph = Graph::build(&self.solve()?.sketch)?;
        if graph.branch().is_some() {
            return graph.cells();
        }
        regions(graph.loops()?)
    }
    /// Every region the whole profile sweeps at once: closed loops nested even-odd, refused where
    /// the curves branch, since regions sharing a curve do not sweep as one solid.
    pub fn sweep_regions(&self) -> Result<Vec<Region>> {
        regions(loops(&self.solve()?.sketch)?)
    }
    /// The region `pick` names among the solved profile's, and a note when only its point found it.
    pub fn region_of(&self, pick: &RegionRef) -> Result<(Region, Option<String>)> {
        pick.find(self.profile_regions()?)
    }
    /// The one region an inline profile sweeps, holes allowed.
    pub fn profile_region(&self) -> Result<Region> {
        let mut regions = self.sweep_regions()?;
        ensure!(
            regions.len() == 1,
            "Sketch has {} separate loops; a profile is one closed loop with any holes inside it. Delete the others, mark them Construction, or draw it as a Sketch feature to sweep every region",
            regions.len()
        );
        Ok(regions.remove(0))
    }
    /// The entities of the closed loop `entity` runs through, as drawn, in joining order.
    pub fn loop_through(&self, entity: Id) -> Result<Vec<Id>> {
        ensure!(self.entities.iter().any(|e| e.id == entity), "Sketch entity #{entity} is missing");
        let mut drawn = self.clone();
        for e in &mut drawn.entities {
            e.construction = false;
        }
        // The entities joined to this one on loops, at their ends or where an end lands on one, and nothing else.
        let graph = Graph::build(&drawn)?;
        let mut picked = BTreeSet::from([entity]);
        let mut ends: BTreeSet<Key> = BTreeSet::new();
        loop {
            let before = picked.len();
            for p in graph.pieces.iter().zip(&graph.alive).filter(|(_, alive)| **alive).map(|(p, _)| p) {
                if picked.contains(&p.entity) || p.ends.iter().any(|k| ends.contains(k)) {
                    picked.insert(p.entity);
                    ends.extend(p.ends);
                }
            }
            if picked.len() == before {
                break;
            }
        }
        drawn.entities.retain(|e| picked.contains(&e.id));
        let found = loops(&drawn)?.into_iter().find(|l| l.entities.contains(&entity));
        let Some(found) = found else {
            bail!("Sketch entity #{entity} is not part of a closed loop");
        };
        Ok(distinct(found.entities))
    }
}

/// The ids in order, each once.
pub(super) fn distinct(ids: Vec<Id>) -> Vec<Id> {
    let mut out: Vec<Id> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// A whole circle as two half arcs; the kernel sweeps arcs, never a whole circle.
pub(super) fn halves(c: Curve) -> Vec<Curve> {
    match c {
        Curve::Circle(circle) => {
            let half = |from: f64| {
                Curve::Arc(Arc { centre: circle.centre, radius: circle.radius, start_angle: from, end_angle: from + PI })
            };
            vec![half(0.0), half(PI)]
        }
        other => vec![other],
    }
}

/// Every closed loop of the sketch's profile geometry in joining order, or where one fails to close:
/// an end lying on another curve joins it there, a cut's overhang is left out, and a point three
/// curves still on loops meet at is refused.
pub(super) fn loops(solved: &Sketch) -> Result<Vec<Loop>> {
    Graph::build(solved)?.loops()
}

/// A box around a curve, padded, never smaller than the curve.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub(super) lo: [f64; 2],
    pub(super) hi: [f64; 2],
}
impl Bounds {
    pub(super) fn of(c: &Curve) -> Self {
        let mut b = Self { lo: [f64::INFINITY; 2], hi: [f64::NEG_INFINITY; 2] };
        match c {
            Curve::Line(l) => {
                b.add(l.start);
                b.add(l.end);
            }
            Curve::Arc(Arc { centre, radius, .. }) | Curve::Circle(geom2d::Circle { centre, radius }) => {
                b.add([centre[0] - radius, centre[1] - radius]);
                b.add([centre[0] + radius, centre[1] + radius]);
            }
            Curve::Nurbs(n) => n.control_points().iter().for_each(|p| b.add(*p)),
            other => other.tessellate(16.0).into_iter().for_each(|p| b.add(p)),
        }
        let pad = 1e-6 * (1.0 + b.lo.iter().chain(&b.hi).fold(0.0_f64, |m, v| m.max(v.abs())));
        b.lo = b.lo.map(|v| v - pad);
        b.hi = b.hi.map(|v| v + pad);
        b
    }
    fn add(&mut self, p: [f64; 2]) {
        for k in 0..2 {
            self.lo[k] = self.lo[k].min(p[k]);
            self.hi[k] = self.hi[k].max(p[k]);
        }
    }
    pub(super) fn union(all: &[Self]) -> Self {
        let mut b = Self { lo: [f64::INFINITY; 2], hi: [f64::NEG_INFINITY; 2] };
        for o in all {
            b.add(o.lo);
            b.add(o.hi);
        }
        b
    }
    pub(super) fn meets(&self, o: &Self) -> bool {
        (0..2).all(|k| self.lo[k] <= o.hi[k] && o.lo[k] <= self.hi[k])
    }
    pub(super) fn holds(&self, p: [f64; 2]) -> bool {
        (0..2).all(|k| self.lo[k] <= p[k] && p[k] <= self.hi[k])
    }
}

/// Where a loop crosses or touches itself away from the corners it joins at, if it does.
pub(super) fn self_crossing(l: &Loop, boxes: &[Bounds]) -> Option<[f64; 2]> {
    let tol = Tolerance::new(CROSSING_MM);
    let n = l.curves.len();
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) <= JOINT_MM;
    for p in 0..n {
        for q in p + 1..n {
            if !boxes[p].meets(&boxes[q]) {
                continue;
            }
            // The corners the two share as neighbours in the walk.
            let mut joints = Vec::new();
            if q == p + 1 {
                joints.push(l.end(p));
            }
            if p == 0 && q == n - 1 {
                joints.push(l.start(0));
            }
            for x in geom2d::intersect(&l.curves[p], &l.curves[q], tol) {
                if !joints.iter().any(|j| near(*j, x.point)) {
                    return Some(x.point);
                }
            }
        }
    }
    None
}

/// Loops grouped into regions by even-odd nesting; loops that cross, touch or enclose nothing are refused.
pub(super) fn regions(loops: Vec<Loop>) -> Result<Vec<Region>> {
    let tol = Tolerance::new(CROSSING_MM);
    let boxes: Vec<Vec<Bounds>> = loops.iter().map(|l| l.curves.iter().map(Bounds::of).collect()).collect();
    let whole: Vec<Bounds> = boxes.iter().map(|b| Bounds::union(b)).collect();
    for (i, l) in loops.iter().enumerate() {
        if let Some(x) = self_crossing(l, &boxes[i]) {
            bail!(
                "Sketch loop through #{} crosses itself at ({:.4}, {:.4}); split it there and trim",
                l.entities[0],
                x[0],
                x[1]
            );
        }
        ensure!(l.area().abs() > 1e-12, "Sketch loop through #{} encloses no area", l.entities[0]);
        for (j, m) in loops.iter().enumerate().skip(i + 1) {
            if !whole[i].meets(&whole[j]) {
                continue;
            }
            for (p, a) in l.curves.iter().enumerate() {
                for (q, b) in m.curves.iter().enumerate() {
                    if !boxes[i][p].meets(&boxes[j][q]) {
                        continue;
                    }
                    if let Some(x) = geom2d::intersect(a, b, tol).first() {
                        bail!(
                            "Sketch loops through #{} and #{} meet at ({:.4}, {:.4}); loops may nest but not touch. Split and trim them, or mark one Construction",
                            l.entities[p],
                            m.entities[q],
                            x.point[0],
                            x.point[1]
                        );
                    }
                }
            }
        }
    }
    // A point of each loop, tested against every other: loops that never meet nest whole.
    let probe: Vec<[f64; 2]> = loops.iter().map(|l| l.curves[0].point_at(0.5)).collect();
    let n = loops.len();
    let parents: Vec<Vec<usize>> = (0..n)
        .map(|i| (0..n).filter(|&j| j != i && whole[j].holds(probe[i]) && geom2d::contains(&loops[j].curves, probe[i], tol)).collect())
        .collect();
    let depth: Vec<usize> = parents.iter().map(Vec::len).collect();
    let mut out: Vec<Region> = Vec::new();
    let mut index = vec![usize::MAX; n];
    for i in (0..n).filter(|i| depth[*i] % 2 == 0) {
        index[i] = out.len();
        let entities = distinct(loops[i].entities.clone());
        out.push(Region { outer: loops[i].curves.clone(), holes: Vec::new(), rim: entities.len(), entities });
    }
    for i in (0..n).filter(|i| depth[*i] % 2 == 1) {
        let parent = parents[i].iter().copied().find(|j| depth[*j] + 1 == depth[i]);
        let Some(parent) = parent else {
            bail!("Sketch loop through #{} has no loop around it to be a hole of", loops[i].entities[0]);
        };
        let r = &mut out[index[parent]];
        r.holes.push(loops[i].curves.clone());
        let more = distinct(loops[i].entities.clone());
        r.entities.extend(more);
    }
    Ok(out)
}

/// Whether each piece of a closed chain runs its own way, found by which ends meet; `None` if it does not close.
pub fn senses(curves: &[Curve]) -> Option<Vec<bool>> {
    let first = curves.first()?;
    if curves.len() == 1 {
        return first.is_closed().then(|| vec![true]);
    }
    let scale = curves.iter().flat_map(|c| c.point_at(0.0)).fold(1.0_f64, |m, v| m.max(v.abs()));
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) <= 1e-7 * scale;
    'first: for lead in [true, false] {
        let mut out = vec![lead];
        let begin = first.point_at(if lead { 0.0 } else { 1.0 });
        let mut head = first.point_at(if lead { 1.0 } else { 0.0 });
        for c in &curves[1..] {
            let (s, e) = (c.point_at(0.0), c.point_at(1.0));
            if near(s, head) {
                out.push(true);
                head = e;
            } else if near(e, head) {
                out.push(false);
                head = s;
            } else {
                continue 'first;
            }
        }
        if near(head, begin) {
            return Some(out);
        }
    }
    None
}

/// Area and first moments `[A, ∫∫x dA, ∫∫y dA]` of the region a closed chain walks round, about
/// `about`, signed by its winding; `None` if the chain does not close.
pub fn loop_moments(curves: &[Curve], about: [f64; 2]) -> Option<[f64; 3]> {
    let forward = senses(curves)?;
    Some(walked_moments(curves, &forward, about))
}

/// [`loop_moments`] with the senses known.
pub(super) fn walked_moments(curves: &[Curve], forward: &[bool], about: [f64; 2]) -> [f64; 3] {
    let mut m = [0.0; 3];
    for (c, f) in curves.iter().zip(forward) {
        let piece = moments(c, about);
        let s = if *f { 1.0 } else { -1.0 };
        for k in 0..3 {
            m[k] += s * piece[k];
        }
    }
    m
}

/// Gauss–Legendre nodes and weights on `[-1, 1]`.
const NODES: [(f64, f64); 5] = [
    (-0.906_179_845_938_664, 0.236_926_885_056_189),
    (-0.538_469_310_105_683, 0.478_628_670_499_366),
    (0.0, 0.568_888_888_888_889),
    (0.538_469_310_105_683, 0.478_628_670_499_366),
    (0.906_179_845_938_664, 0.236_926_885_056_189),
];

/// One curve's share of Green's integrals `[½∮(x dy − y dx), ½∮x² dy, −½∮y² dx]`, walked start to end.
pub fn moments(c: &Curve, about: [f64; 2]) -> [f64; 3] {
    let o = |p: [f64; 2]| [p[0] - about[0], p[1] - about[1]];
    match c {
        Curve::Line(l) => {
            let (p, q) = (o(l.start), o(l.end));
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            [
                0.5 * (p[0] * q[1] - p[1] * q[0]),
                0.5 * dy * (p[0] * p[0] + p[0] * dx + dx * dx / 3.0),
                -0.5 * dx * (p[1] * p[1] + p[1] * dy + dy * dy / 3.0),
            ]
        }
        Curve::Arc(a) => arc_moments(o(a.centre), a.radius, a.start_angle, a.sweep()),
        Curve::Circle(c) => arc_moments(o(c.centre), c.radius, 0.0, TAU),
        Curve::Polyline(_) => c.segments().iter().map(|s| moments(s, about)).fold([0.0; 3], |a, b| [a[0] + b[0], a[1] + b[1], a[2] + b[2]]),
        other => {
            const PANELS: usize = 32;
            let mut m = [0.0; 3];
            for k in 0..PANELS {
                for (x, w) in NODES {
                    let t = (k as f64 + (x + 1.0) * 0.5) / PANELS as f64;
                    let p = o(other.point_at(t));
                    let d = other.tangent_at(t);
                    let w = w * 0.5 / PANELS as f64;
                    m[0] += w * 0.5 * (p[0] * d[1] - p[1] * d[0]);
                    m[1] += w * 0.5 * p[0] * p[0] * d[1];
                    m[2] -= w * 0.5 * p[1] * p[1] * d[0];
                }
            }
            m
        }
    }
}

/// Green's integrals along a circular arc about the origin: centre `c`, radius `r`, from `a0` turning `s`.
fn arc_moments(c: [f64; 2], r: f64, a0: f64, s: f64) -> [f64; 3] {
    let a1 = a0 + s;
    let (s0, c0, s1, c1) = (a0.sin(), a0.cos(), a1.sin(), a1.cos());
    let twice = 0.25 * ((2.0 * a1).sin() - (2.0 * a0).sin());
    let area = 0.5 * (r * c[0] * (s1 - s0) - r * c[1] * (c1 - c0) + r * r * s);
    let cos2 = 0.5 * s + twice;
    let cos3 = (s1 - s1.powi(3) / 3.0) - (s0 - s0.powi(3) / 3.0);
    let mx = 0.5 * r * (c[0] * c[0] * (s1 - s0) + 2.0 * c[0] * r * cos2 + r * r * cos3);
    let sin2 = 0.5 * s - twice;
    let sin3 = (c1.powi(3) / 3.0 - c1) - (c0.powi(3) / 3.0 - c0);
    let my = 0.5 * r * (c[1] * c[1] * (c0 - c1) + 2.0 * c[1] * r * sin2 + r * r * sin3);
    [area, mx, my]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::{Geometry, Sketch};

    /// Two held rectangles on the section plane: 2 x 2 at x 5..7, 3 x 3 at x 10..13; their bottoms.
    fn two_rectangles() -> (Sketch, Id, Id) {
        let mut s = Sketch { plane: crate::sketch::Workplane::section(), ..Sketch::default() };
        let a = s.add_rectangle([5.0, 0.0], [7.0, 2.0], false).unwrap();
        let b = s.add_rectangle([10.0, 0.0], [13.0, 3.0], false).unwrap();
        (s, a[0], b[0])
    }

    #[test]
    fn a_region_is_named_by_its_rim_and_found_again_by_its_point_never_by_the_loop_round_it() {
        let (mut s, a, b) = two_rectangles();
        let regions = s.profile_regions().unwrap();
        let of = |regions: &[Region], e: Id| regions.iter().find(|r| r.entities.contains(&e)).cloned().unwrap();
        let (left, right) = (of(&regions, a), of(&regions, b));
        let pick = RegionRef::of(&right).unwrap();
        assert!(right.entities[..right.rim].contains(&pick.entity) && right.contains(pick.at) && !left.contains(pick.at));
        assert!(RegionRef::at(&right, [6.0, 1.0]).is_none(), "a point outside is no name for it");
        // Moved clear of its point, the region is still the one its rim names.
        let corners: Vec<Id> = s.entities.iter().filter(|e| right.entities.contains(&e.id)).flat_map(|e| e.geometry.points()).collect();
        let shift = |s: &mut Sketch, dx: f64| s.points.iter_mut().filter(|p| corners.contains(&p.id)).for_each(|p| p.xy[0] += dx);
        shift(&mut s, 10.0);
        let (found, note) = s.region_of(&pick).unwrap();
        assert!(note.is_none() && (found.area() - 9.0).abs() < 1e-9 && found.contains([21.5, 1.5]));
        // Its bottom redrawn as a new line: the rim no longer names it and the point is not in it, so it is refused.
        let (p, q) = match s.entities.iter().find(|e| e.id == b).unwrap().geometry { Geometry::Line { a, b } => (a, b), _ => unreachable!() };
        s.remove_entity(b);
        let redrawn = s.add_line(p, q, false).unwrap();
        assert_ne!(redrawn, b);
        let e = s.region_of(&pick).unwrap_err().to_string();
        assert!(e.contains(&format!("runs round #{b}")) && e.contains("pick the region again"), "{e}");
        // Moved back over its point, it is found again, and says how.
        shift(&mut s, -10.0);
        let (found, note) = s.region_of(&pick).unwrap();
        assert!((found.area() - 9.0).abs() < 1e-9 && found.entities.contains(&redrawn));
        assert!(note.unwrap().contains("found again by its point"));
        // A loop drawn round the left rectangle makes it a hole: neither its rim nor its point names a region now.
        let left_pick = RegionRef::of(&left).unwrap();
        s.add_rectangle([4.0, -1.0], [8.0, 3.0], false).unwrap();
        let e = s.region_of(&left_pick).unwrap_err().to_string();
        assert!(e.contains("No region of the sketch"), "{e}");
        // The profile that carries a pick reads and writes as its own shape, and older shapes read as before.
        use crate::cad::Profile;
        let profile = Profile::Region { feature: 3, region: pick };
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(json, serde_json::json!({ "feature": 3, "region": { "entity": pick.entity, "at": pick.at } }));
        assert_eq!(serde_json::from_value::<Profile>(json).unwrap(), profile);
        assert_eq!(serde_json::from_value::<Profile>(serde_json::json!({ "feature": 3 })).unwrap(), Profile::Feature { feature: 3 });
        let inline = serde_json::to_value(Sketch::rectangle(2.0, 1.0)).unwrap();
        assert!(matches!(serde_json::from_value::<Profile>(inline).unwrap(), Profile::Inline(_)));
    }

    #[test]
    fn one_region_of_several_extrudes_or_revolves_alone() {
        use crate::cad::{Component, Document, Feature, FeatureStatus, Operation, Profile, evaluate};
        use crate::{AlphaLibrary, BuildParams, RingDesign};
        let (s, a, b) = two_rectangles();
        let regions = s.profile_regions().unwrap();
        let pick = |e: Id| RegionRef::of(regions.iter().find(|r| r.entities.contains(&e)).unwrap()).unwrap();
        let feature = |id, operation| Feature { id, name: format!("#{id}"), enabled: true, operation, component: Component::default() };
        let design = |sketch: &Sketch, operation: Operation| {
            let mut doc = Document::default();
            doc.append(feature(1, Operation::Sketch { sketch: sketch.clone() })).unwrap();
            doc.append(feature(2, operation)).unwrap();
            RingDesign { cad: Some(doc), ..RingDesign::default() }
        };
        let (lib, params) = (AlphaLibrary::builtin(), BuildParams::default());
        let extrude = |from: Profile| Operation::Extrude { sketch: from, height_mm: 1.5, draft_deg: 0.0 };
        // The whole sketch is two lumps; the right region alone is one, its own area times the height.
        for (from, lumps, volume) in [(Profile::Feature { feature: 1 }, 2, (4.0 + 9.0) * 1.5), (Profile::Region { feature: 1, region: pick(b) }, 1, 9.0 * 1.5)] {
            let e = evaluate(&design(&s, extrude(from)), &lib, params).unwrap();
            assert!(e.failures().is_empty(), "{:?}", e.failures());
            let c = &e.components[0];
            assert!(c.mesh.validate().watertight && c.body.roots.len() == lumps);
            assert!((c.mesh.volume_mm3() - volume).abs() < 1e-6, "{} against {volume}", c.mesh.volume_mm3());
        }
        // The left region turned a full turn about the finger's axis: a washer 5 to 7 mm out, 2 mm tall.
        let turn = Operation::Revolve { sketch: Profile::Region { feature: 1, region: pick(a) }, pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0 };
        let e = evaluate(&design(&s, turn), &lib, params).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let expected = std::f64::consts::PI * (49.0 - 25.0) * 2.0;
        let v = e.components[0].mesh.volume_mm3();
        assert!((v / expected - 1.0).abs() < 0.005, "{v} against {expected}");
        // Its rim redrawn, the region is found by its point and the feature says so; made a hole, the feature fails by name.
        let mut redrawn = s.clone();
        let (p, q) = match redrawn.entities.iter().find(|e| e.id == b).unwrap().geometry { Geometry::Line { a, b } => (a, b), _ => unreachable!() };
        redrawn.remove_entity(b);
        redrawn.add_line(p, q, false).unwrap();
        let e = evaluate(&design(&redrawn, extrude(Profile::Region { feature: 1, region: pick(b) })), &lib, params).unwrap();
        let report = e.features.iter().find(|f| f.id == 2).unwrap();
        assert!(report.status.is_ok() && report.notes.iter().any(|n| n.contains("Sketch #1: Region found again by its point")), "{:?}", report.notes);
        let mut holed = s.clone();
        holed.add_rectangle([9.0, -1.0], [14.0, 4.0], false).unwrap();
        let e = evaluate(&design(&holed, extrude(Profile::Region { feature: 1, region: pick(b) })), &lib, params).unwrap();
        let status = &e.features.iter().find(|f| f.id == 2).unwrap().status;
        assert!(matches!(status, FeatureStatus::Failed(m) if m.contains("Sketch #1") && m.contains("pick the region again")), "{status:?}");
    }

    fn square(s: &mut Sketch, lo: [f64; 2], side: f64) -> Id {
        let p = [lo, [lo[0] + side, lo[1]], [lo[0] + side, lo[1] + side], [lo[0], lo[1] + side]].map(|p| s.point(p));
        s.entity(Geometry::Polyline { points: p.to_vec(), closed: true })
    }
    fn circle(s: &mut Sketch, c: [f64; 2], r: f64) -> Id {
        let centre = s.point(c);
        let rim = s.point([c[0] + r, c[1]]);
        s.entity(Geometry::Circle { center: centre, rim })
    }

    #[test]
    fn loops_nest_even_odd_into_regions() {
        // A washer: one region with one hole.
        let mut s = Sketch::default();
        circle(&mut s, [0.0; 2], 3.0);
        circle(&mut s, [0.0; 2], 2.0);
        let r = s.profile_regions().unwrap();
        assert_eq!((r.len(), r[0].holes.len()), (1, 1));
        let washer = std::f64::consts::PI * (9.0 - 4.0);
        assert!((r[0].area() - washer).abs() < 1e-9, "{}", r[0].area());
        assert!(r[0].contains([2.5, 0.0]) && !r[0].contains([1.0, 0.0]) && !r[0].contains([4.0, 0.0]));
        // Two disjoint squares: two regions.
        let mut s = Sketch::default();
        square(&mut s, [0.0, 0.0], 2.0);
        square(&mut s, [5.0, 0.0], 2.0);
        let r = s.profile_regions().unwrap();
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|r| r.holes.is_empty() && (r.area() - 4.0).abs() < 1e-12));
        let one = s.profile_region().err().unwrap().to_string();
        assert!(one.contains("2 separate loops"), "{one}");
        // A square in the hole of a square: the island is a region of its own.
        let mut s = Sketch::default();
        let outer = square(&mut s, [0.0, 0.0], 10.0);
        let hole = square(&mut s, [2.0, 2.0], 6.0);
        let island = square(&mut s, [4.0, 4.0], 2.0);
        let r = s.profile_regions().unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].entities.clone(), r[0].holes.len(), r[1].entities.clone()), (vec![outer, hole], 1, vec![island]));
        assert!((r[0].area() - 64.0).abs() < 1e-9 && (r[1].area() - 4.0).abs() < 1e-12);
        // Crossing and touching loops are refused with where they meet.
        let mut s = Sketch::default();
        square(&mut s, [0.0, 0.0], 4.0);
        square(&mut s, [2.0, 2.0], 4.0);
        let error = s.profile_regions().err().unwrap().to_string();
        assert!(error.contains("meet at (4.0000, 2.0000)"), "{error}");
        let mut s = Sketch::default();
        square(&mut s, [0.0, 0.0], 2.0);
        square(&mut s, [2.0, 2.0], 2.0);
        assert!(s.profile_regions().err().unwrap().to_string().contains("meet at (2.0000, 2.0000)"));
        // A loop crossing itself is named where it crosses; a line hanging off a corner by its free end.
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 4.0]].map(|p| s.point(p));
        s.entity(Geometry::Polyline { points: p.to_vec(), closed: true });
        let error = s.profile_regions().err().unwrap().to_string();
        assert!(error.contains("crosses itself at (2.0000, 2.0000)"), "{error}");
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 0.0], [2.0, 3.0], [2.0, -3.0]].map(|p| s.point(p));
        s.entity(Geometry::Line { a: p[0], b: p[1] });
        s.entity(Geometry::Line { a: p[1], b: p[2] });
        s.entity(Geometry::Line { a: p[2], b: p[0] });
        s.entity(Geometry::Line { a: p[0], b: p[3] });
        let error = s.profile_regions().err().unwrap().to_string();
        assert!(error.contains(&format!("open at point #{}", p[3])), "{error}");
        // Closed back to the triangle's far corner, the corner three curves meet at is named when the whole sketch sweeps.
        s.entity(Geometry::Line { a: p[3], b: p[1] });
        let error = s.sweep_regions().err().unwrap().to_string();
        assert!(error.contains(&format!("point #{} joins 3 curves", p[0])), "{error}");
        assert_eq!(s.profile_regions().unwrap().len(), 2, "and a pick chooses between its two triangles");
    }

    #[test]
    fn green_moments_are_exact_for_lines_arcs_and_splines() {
        // A half disc of radius 2 at (1, 1): centroid 4r/3π above its diameter.
        let arc = Curve::Arc(Arc { centre: [1.0, 1.0], radius: 2.0, start_angle: 0.0, end_angle: PI });
        let diameter = Curve::Line(geom2d::Line { start: [-1.0, 1.0], end: [3.0, 1.0] });
        let m = loop_moments(&[arc, diameter], [0.3, -0.2]).unwrap();
        let area = 2.0 * PI;
        assert!((m[0] - area).abs() < 1e-12, "{m:?}");
        assert!((m[1] / m[0] + 0.3 - 1.0).abs() < 1e-12 && (m[2] / m[0] - 0.2 - (1.0 + 8.0 / (3.0 * PI))).abs() < 1e-12, "{m:?}");
        // A straight spline edge closes the same triangle three lines would.
        let bezier = Curve::Nurbs(geom2d::NurbsCurve::new(3, vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]], vec![0., 0., 0., 0., 1., 1., 1., 1.], None).unwrap());
        let up = Curve::Line(geom2d::Line { start: [3.0, 0.0], end: [3.0, 2.0] });
        let back = Curve::Line(geom2d::Line { start: [3.0, 2.0], end: [0.0, 0.0] });
        let m = loop_moments(&[bezier, up, back], [0.0; 2]).unwrap();
        assert!((m[0] - 3.0).abs() < 1e-9 && (m[1] / m[0] - 2.0).abs() < 1e-9 && (m[2] / m[0] - 2.0 / 3.0).abs() < 1e-9, "{m:?}");
    }
}
