//! Editing a sketch's drawn geometry: split at crossings, trim, offset, corner fillet and
//! chamfer, mirror and pattern. Each works on a copy and keeps it only once it validates, so a
//! refusal leaves the sketch as it was; constraints naming a point that goes go with it.
use super::region::{self, Bounds};
use super::{Constraint, Entity, Geometry, Id, Sketch, distance};
use anyhow::{Context, Result, bail, ensure};
use cadkernel::geom2d::{self, Arc, Curve, Line, Tolerance};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::TAU;

/// Positions within this of each other are one point.
const SAME_MM: f64 = 1e-7;
/// Most copies one pattern lays down; the sketch's own 512-entity cap still applies after.
const MAX_COPIES: usize = 511;

type V = [f64; 2];
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
fn scale(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: V, b: V) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn unit(a: V) -> Option<V> {
    let l = a[0].hypot(a[1]);
    (l.is_finite() && l > 1e-12).then(|| scale(a, 1.0 / l))
}
fn angle(c: V, p: V) -> f64 {
    (p[1] - c[1]).atan2(p[0] - c[0])
}
fn polar(c: V, r: f64, a: f64) -> V {
    [c[0] + r * a.cos(), c[1] + r * a.sin()]
}

/// How copies of sketch geometry are laid out.
#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    /// A grid: `count[0]` copies `step[0]` apart, in `count[1]` rows `step[1]` apart; the original is the first.
    Rect { step: [[f64; 2]; 2], count: [usize; 2] },
    /// `count` copies about `centre`, the original first: evenly round a full turn when
    /// `|sweep_deg| >= 360`, otherwise spread so the last lands `sweep_deg` round.
    Polar { centre: [f64; 2], count: usize, sweep_deg: f64 },
}

/// One straight curve leaving a corner.
#[derive(Clone, Copy, Debug)]
struct Arm {
    /// Index of its entity.
    entity: usize,
    /// For a line, the end at the corner (0 is `a`); for a polyline, the corner's vertex index.
    at: usize,
    /// For a polyline, 1 when the arm runs to the next vertex, -1 to the previous; 0 for a line.
    step: isize,
    /// The point at the corner end of this arm.
    here: Id,
    /// The point at its far end.
    far: Id,
}

/// One segment of a loop as walked.
#[derive(Clone, Copy, Debug)]
enum Seg {
    Line { a: V, b: V },
    /// About `c`, radius `r`, leaving at angle `a0` and turning `s` (counter-clockwise positive).
    Arc { c: V, r: f64, a0: f64, s: f64, centre: Id },
}
impl Seg {
    fn start(&self) -> V {
        match *self {
            Self::Line { a, .. } => a,
            Self::Arc { c, r, a0, .. } => polar(c, r, a0),
        }
    }
    fn end(&self) -> V {
        match *self {
            Self::Line { b, .. } => b,
            Self::Arc { c, r, a0, s, .. } => polar(c, r, a0 + s),
        }
    }
}
/// The curve a segment is offset onto: a whole line or a whole circle.
#[derive(Clone, Copy, Debug)]
enum Carrier {
    Line { p: V, t: V },
    Circle { c: V, r: f64 },
}
impl Carrier {
    /// Where `p` on the original segment lands on the offset.
    fn moved(&self, seg: &Seg, p: V) -> V {
        match (*self, *seg) {
            (Self::Line { p: q, t }, Seg::Line { .. }) => add(q, scale(t, dot(sub(p, q), t))),
            (Self::Circle { c, r }, Seg::Arc { r: r0, .. }) => add(c, scale(sub(p, c), r / r0)),
            _ => p,
        }
    }
    /// Every point the two whole carriers share.
    fn meet(&self, other: &Self) -> Vec<V> {
        match (*self, *other) {
            (Self::Line { p, t }, Self::Line { p: q, t: u }) => {
                let d = cross(t, u);
                if d.abs() < 1e-12 {
                    return Vec::new();
                }
                vec![add(p, scale(t, cross(sub(q, p), u) / d))]
            }
            (Self::Line { p, t }, Self::Circle { c, r }) | (Self::Circle { c, r }, Self::Line { p, t }) => {
                let f = sub(p, c);
                let b = dot(f, t);
                let disc = b * b - (dot(f, f) - r * r);
                if disc < -1e-12 {
                    return Vec::new();
                }
                let root = disc.max(0.0).sqrt();
                vec![add(p, scale(t, -b - root)), add(p, scale(t, -b + root))]
            }
            (Self::Circle { c, r }, Self::Circle { c: k, r: q }) => {
                let d = sub(k, c);
                let l = d[0].hypot(d[1]);
                if l < 1e-12 || l > r + q + 1e-12 || l < (r - q).abs() - 1e-12 {
                    return Vec::new();
                }
                let along = (r * r - q * q + l * l) / (2.0 * l);
                let h = (r * r - along * along).max(0.0).sqrt();
                let e = scale(d, 1.0 / l);
                let m = add(c, scale(e, along));
                let n = [-e[1], e[0]];
                vec![add(m, scale(n, h)), add(m, scale(n, -h))]
            }
        }
    }
}

impl Sketch {
    /// Runs `edit` on a copy and keeps it only if the result validates.
    fn transact<T>(&mut self, edit: impl FnOnce(&mut Sketch) -> Result<T>) -> Result<T> {
        let mut s = self.clone();
        let out = edit(&mut s)?;
        s.validate()?;
        *self = s;
        Ok(out)
    }
    fn index_of(&self, id: Id) -> Result<usize> {
        self.entities.iter().position(|e| e.id == id).with_context(|| format!("Sketch entity #{id} is missing"))
    }
    /// The point at `xy`: one already there, or a new one.
    fn point_near(&mut self, xy: V) -> Id {
        match self.points.iter().find(|p| distance(p.xy, xy) <= SAME_MM) {
            Some(p) => p.id,
            None => self.point(xy),
        }
    }
    /// A new entity beside the others, drawn as construction when `construction` says so.
    fn add_entity(&mut self, geometry: Geometry, construction: bool) -> Id {
        let id = self.entity(geometry);
        if let Some(e) = self.entities.last_mut() {
            e.construction = construction;
        }
        id
    }
    /// Drops the points of `candidates` no entity names any more, and every constraint naming one.
    fn drop_orphans(&mut self, candidates: impl IntoIterator<Item = Id>) {
        let named: BTreeSet<Id> = self.entities.iter().flat_map(|e| e.geometry.points()).collect();
        let gone: BTreeSet<Id> = candidates.into_iter().filter(|id| !named.contains(id)).collect();
        self.points.retain(|p| !gone.contains(&p.id));
        self.constraints.retain(|c| c.points().iter().all(|id| !gone.contains(id)));
    }
    /// Horizontal and vertical constraints on the line `from` moved onto the line `to`, which carries on its direction.
    fn carry_direction(&mut self, from: (Id, Id), to: (Id, Id)) {
        for c in &mut self.constraints {
            match c {
                Constraint::Horizontal(a, b) | Constraint::Vertical(a, b)
                    if (*a, *b) == from || (*b, *a) == from =>
                {
                    (*a, *b) = to;
                }
                _ => {}
            }
        }
    }

    /// Splits every line, polyline, arc and circle where it crosses another, one point shared at
    /// each crossing; a circle takes two crossings to become arcs, and splines only cut. Returns
    /// the entities added; the first piece of a split entity keeps its id.
    pub fn split_at_intersections(&mut self) -> Result<Vec<Id>> {
        self.transact(|s| {
            let tol = Tolerance::new(SAME_MM);
            let mut pieces = Vec::new();
            for (i, e) in s.entities.iter().enumerate().filter(|(_, e)| !e.construction) {
                for (k, c) in s.curves_of(e)?.into_iter().enumerate() {
                    let b = Bounds::of(&c);
                    pieces.push((i, k, c, b));
                }
            }
            let mut hits: BTreeMap<(usize, usize), Vec<V>> = BTreeMap::new();
            for x in 0..pieces.len() {
                for y in x + 1..pieces.len() {
                    let (a, b) = (&pieces[x], &pieces[y]);
                    if a.0 == b.0 || !a.3.meets(&b.3) {
                        continue;
                    }
                    for c in geom2d::intersect(&a.2, &b.2, tol) {
                        hits.entry((a.0, a.1)).or_default().push(c.point);
                        hits.entry((b.0, b.1)).or_default().push(c.point);
                    }
                }
            }
            let targets: BTreeSet<usize> = hits.keys().map(|k| k.0).collect();
            let mut added = Vec::new();
            let mut dropped = Vec::new();
            for i in targets {
                let e = s.entities[i].clone();
                let curves = s.curves_of(&e)?;
                let on: Vec<Vec<V>> = (0..curves.len()).map(|k| hits.get(&(i, k)).cloned().unwrap_or_default()).collect();
                let parts = s.split_geometry(&e.geometry, &curves, &on);
                if parts.len() < 2 {
                    continue;
                }
                dropped.extend(e.geometry.points());
                let mut parts = parts.into_iter();
                s.entities[i].geometry = parts.next().unwrap_or(e.geometry.clone());
                for g in parts {
                    added.push(s.add_entity(g, e.construction));
                }
            }
            s.drop_orphans(dropped);
            Ok(added)
        })
    }
    /// The pieces `g` breaks into at the points `on` each of its curves; one piece when nothing splits it.
    fn split_geometry(&mut self, g: &Geometry, curves: &[Curve], on: &[Vec<V>]) -> Vec<Geometry> {
        // Parameters strictly inside a curve, in order, one per place.
        let inside = |c: &Curve, points: &[V]| -> Vec<(f64, V)> {
            let length = c.length();
            let mut ts: Vec<(f64, V)> = points
                .iter()
                .map(|p| (c.parameter_at(*p), *p))
                .filter(|(t, _)| *t * length > SAME_MM && (1.0 - *t) * length > SAME_MM)
                .collect();
            ts.sort_by(|a, b| a.0.total_cmp(&b.0));
            ts.dedup_by(|a, b| (a.0 - b.0).abs() * length <= SAME_MM);
            ts
        };
        match g {
            Geometry::Line { a, b } => {
                let cuts = inside(&curves[0], &on[0]);
                let mut ids = vec![*a];
                ids.extend(cuts.iter().map(|(_, p)| self.point_near(*p)));
                ids.push(*b);
                ids.windows(2).map(|w| Geometry::Line { a: w[0], b: w[1] }).collect()
            }
            Geometry::Arc { center, start, end } => {
                let cuts = inside(&curves[0], &on[0]);
                let mut ids = vec![*start];
                ids.extend(cuts.iter().map(|(_, p)| self.point_near(*p)));
                ids.push(*end);
                ids.windows(2).map(|w| Geometry::Arc { center: *center, start: w[0], end: w[1] }).collect()
            }
            Geometry::Circle { center, .. } => {
                let Curve::Circle(circle) = &curves[0] else { return vec![g.clone()] };
                let mut at: Vec<(f64, V)> = on[0].iter().map(|p| (angle(circle.centre, *p).rem_euclid(TAU), *p)).collect();
                at.sort_by(|a, b| a.0.total_cmp(&b.0));
                at.dedup_by(|a, b| (a.0 - b.0).abs() * circle.radius <= SAME_MM);
                if at.len() > 1 && (at[0].0 + TAU - at[at.len() - 1].0) * circle.radius <= SAME_MM {
                    at.pop();
                }
                if at.len() < 2 {
                    return vec![g.clone()];
                }
                let ids: Vec<Id> = at.iter().map(|(_, p)| self.point_near(*p)).collect();
                (0..ids.len()).map(|j| Geometry::Arc { center: *center, start: ids[j], end: ids[(j + 1) % ids.len()] }).collect()
            }
            Geometry::Polyline { points, closed } => {
                // The vertices in order with every split point inserted, split points flagged.
                let mut seq: Vec<(Id, bool)> = Vec::new();
                for (k, c) in curves.iter().enumerate() {
                    seq.push((points[k], false));
                    for (_, p) in inside(c, &on[k]) {
                        seq.push((self.point_near(p), true));
                    }
                }
                if !closed {
                    seq.push((points[points.len() - 1], false));
                }
                let Some(first) = seq.iter().position(|(_, split)| *split) else { return vec![g.clone()] };
                if *closed {
                    seq.rotate_left(first);
                    seq.push(seq[0]);
                }
                let mut parts: Vec<Vec<Id>> = vec![Vec::new()];
                for (i, (id, split)) in seq.iter().enumerate() {
                    parts.last_mut().unwrap().push(*id);
                    if *split && i + 1 < seq.len() && i > 0 {
                        parts.push(vec![*id]);
                    }
                }
                parts.into_iter().filter(|p| p.len() > 1).map(|p| run(p)).collect()
            }
            Geometry::Bezier { .. } => vec![g.clone()],
        }
    }

    /// Removes the span of `entity` between the crossings nearest either side of `near`: the whole
    /// entity when nothing crosses it, and a whole circle unless two things do. A polyline loses the
    /// span of its segment nearest `near`, bounded by its own corners too. Returns what is left of it.
    pub fn trim(&mut self, entity: Id, near: [f64; 2]) -> Result<Vec<Id>> {
        ensure!(near.iter().all(|v| v.is_finite()), "Trim needs a finite pick");
        self.transact(|s| {
            let tol = Tolerance::new(SAME_MM);
            let i = s.index_of(entity)?;
            let e = s.entities[i].clone();
            let curves = s.curves_of(&e)?;
            let k = (0..curves.len())
                .min_by(|a, b| geom2d::distance_to(&curves[*a], near).total_cmp(&geom2d::distance_to(&curves[*b], near)))
                .context("Nothing to trim")?;
            let piece = curves[k].clone();
            ensure!(
                matches!(piece, Curve::Line(_) | Curve::Arc(_) | Curve::Circle(_)),
                "Trim takes lines, polylines, arcs and circles; #{entity} is a spline"
            );
            let bounds = Bounds::of(&piece);
            let mut cuts = Vec::new();
            for o in &s.entities {
                for (j, c) in s.curves_of(o)?.iter().enumerate() {
                    if (o.id == entity && j == k) || !bounds.meets(&Bounds::of(c)) {
                        continue;
                    }
                    cuts.extend(geom2d::intersect(&piece, c, tol).into_iter().map(|x| x.t_a));
                }
            }
            let picked = piece.parameter_at(near);
            let dropped = e.geometry.points();
            let left = match (&e.geometry, &piece) {
                (Geometry::Circle { center, .. }, Curve::Circle(circle)) => {
                    let mut at: Vec<f64> = cuts.iter().map(|t| t.rem_euclid(1.0)).collect();
                    at.sort_by(f64::total_cmp);
                    at.dedup_by(|a, b| (*a - *b).abs() * TAU * circle.radius <= SAME_MM);
                    if at.len() > 1 && (at[0] + 1.0 - at[at.len() - 1]) * TAU * circle.radius <= SAME_MM {
                        at.pop();
                    }
                    if at.len() < 2 {
                        s.entities.remove(i);
                        Vec::new()
                    } else {
                        // The removed span runs from the last cut at or before the pick to the next one.
                        let p = picked.rem_euclid(1.0);
                        let from = at.iter().rposition(|t| *t <= p).unwrap_or(at.len() - 1);
                        let to = (from + 1) % at.len();
                        let start = s.point_near(piece.point_at(at[to]));
                        let end = s.point_near(piece.point_at(at[from]));
                        s.entities[i].geometry = Geometry::Arc { center: *center, start, end };
                        vec![entity]
                    }
                }
                (Geometry::Polyline { points, closed }, Curve::Line(_)) => {
                    let (r0, r1) = removed(&geom2d::trim_spans(&piece, &cuts, picked, tol));
                    let n = points.len();
                    let from = if r0 > 0.0 { Some(s.point_near(piece.point_at(r0))) } else { None };
                    let to = if r1 < 1.0 { Some(s.point_near(piece.point_at(r1))) } else { None };
                    let mut runs: Vec<Vec<Id>> = Vec::new();
                    if *closed {
                        let mut r: Vec<Id> = to.into_iter().collect();
                        r.extend((1..=n).map(|j| points[(k + j) % n]));
                        r.extend(from);
                        runs.push(r);
                    } else {
                        let mut head: Vec<Id> = points[..=k].to_vec();
                        head.extend(from);
                        let mut tail: Vec<Id> = to.into_iter().collect();
                        tail.extend(&points[k + 1..]);
                        runs.extend([head, tail]);
                    }
                    let runs: Vec<Geometry> = runs.into_iter().filter(|r| r.len() > 1).map(run).collect();
                    s.replace(i, runs)
                }
                (Geometry::Line { a, b }, Curve::Line(_)) => {
                    let kept = geom2d::trim_spans(&piece, &cuts, picked, tol);
                    let (a, b) = (*a, *b);
                    let mut parts = Vec::new();
                    for span in &kept {
                        let p = if span[0] <= 0.0 { a } else { s.point_near(piece.point_at(span[0])) };
                        let q = if span[1] >= 1.0 { b } else { s.point_near(piece.point_at(span[1])) };
                        parts.push(Geometry::Line { a: p, b: q });
                    }
                    if let [Geometry::Line { a: p, b: q }] = parts.as_slice() {
                        s.carry_direction((a, b), (*p, *q));
                    }
                    s.replace(i, parts)
                }
                (Geometry::Arc { center, start, end }, Curve::Arc(_)) => {
                    let kept = geom2d::trim_spans(&piece, &cuts, picked, tol);
                    let mut parts = Vec::new();
                    for span in &kept {
                        let p = if span[0] <= 0.0 { *start } else { s.point_near(piece.point_at(span[0])) };
                        let q = if span[1] >= 1.0 { *end } else { s.point_near(piece.point_at(span[1])) };
                        parts.push(Geometry::Arc { center: *center, start: p, end: q });
                    }
                    s.replace(i, parts)
                }
                _ => bail!("Trim cannot read #{entity}"),
            };
            s.drop_orphans(dropped);
            Ok(left)
        })
    }
    /// Entity `i` becomes the first of `parts` and the rest are added after it; none removes it. The ids that stand.
    fn replace(&mut self, i: usize, parts: Vec<Geometry>) -> Vec<Id> {
        let e = self.entities[i].clone();
        let mut parts = parts.into_iter();
        let Some(first) = parts.next() else {
            self.entities.remove(i);
            return Vec::new();
        };
        self.entities[i].geometry = first;
        let mut ids = vec![e.id];
        for g in parts {
            ids.push(self.add_entity(g, e.construction));
        }
        ids
    }

    /// A new closed loop `distance` outside the loop `entities` make, inside for a negative
    /// distance: lines stay lines, arcs stay arcs about the same centres, and corners join where
    /// the offset curves meet, extended outside and trimmed inside. A distance that would turn a
    /// segment round or fold the loop through itself is refused. Returns the new entities.
    pub fn offset(&mut self, entities: &[Id], distance: f64) -> Result<Vec<Id>> {
        ensure!(
            distance.is_finite() && distance.abs() > SAME_MM && distance.abs() < 1000.0,
            "An offset needs a distance between 1e-7 and 1000 mm"
        );
        self.transact(|s| {
            let wanted: BTreeSet<Id> = entities.iter().copied().collect();
            ensure!(!wanted.is_empty(), "Select the curves of one closed loop to offset");
            let mut construction = true;
            for id in &wanted {
                construction &= s.entities[s.index_of(*id)?].construction;
            }
            if let [id] = entities {
                if let Geometry::Circle { center, rim } = s.entities[s.index_of(*id)?].geometry {
                    let (c, p) = (s.at(center)?, s.at(rim)?);
                    let r = super::distance(c, p) + distance;
                    ensure!(r > SAME_MM, "Offset by {distance} mm turns circle #{id} inside out");
                    let dir = unit(sub(p, c)).context("Circle has no radius")?;
                    let rim = s.point(add(c, scale(dir, r)));
                    return Ok(vec![s.add_entity(Geometry::Circle { center, rim }, construction)]);
                }
            }
            let mut drawn = s.clone();
            drawn.entities.retain(|e| wanted.contains(&e.id));
            for e in &mut drawn.entities {
                e.construction = false;
            }
            let mut loops = region::loops(&drawn)?;
            ensure!(loops.len() == 1, "Offset takes one closed loop; the selection makes {}", loops.len());
            let walk = loops.remove(0);
            let n = walk.curves.len();
            let mut segs = Vec::with_capacity(n);
            for ((c, f), id) in walk.curves.iter().zip(&walk.forward).zip(&walk.entities) {
                segs.push(match c {
                    Curve::Line(l) if *f => Seg::Line { a: l.start, b: l.end },
                    Curve::Line(l) => Seg::Line { a: l.end, b: l.start },
                    Curve::Arc(arc) => {
                        let centre = match s.entities[s.index_of(*id)?].geometry {
                            Geometry::Arc { center, .. } | Geometry::Circle { center, .. } => center,
                            _ => bail!("Offset cannot find the centre of #{id}"),
                        };
                        let sweep = arc.sweep();
                        if *f {
                            Seg::Arc { c: arc.centre, r: arc.radius, a0: arc.start_angle, s: sweep, centre }
                        } else {
                            Seg::Arc { c: arc.centre, r: arc.radius, a0: arc.start_angle + sweep, s: -sweep, centre }
                        }
                    }
                    _ => bail!("Offset takes lines and arcs; #{id} is a spline"),
                });
            }
            let sigma = walk.area().signum();
            // Each segment's carrier, moved `distance` out of the loop.
            let mut carriers = Vec::with_capacity(n);
            for (g, id) in segs.iter().zip(&walk.entities) {
                carriers.push(match *g {
                    Seg::Line { a, b } => {
                        let t = unit(sub(b, a)).context("A loop segment has no length")?;
                        let out = if sigma > 0.0 { [t[1], -t[0]] } else { [-t[1], t[0]] };
                        Carrier::Line { p: add(a, scale(out, distance)), t }
                    }
                    Seg::Arc { c, r, s: turn, .. } => {
                        let radius = r + distance * turn.signum() * sigma;
                        ensure!(
                            radius > SAME_MM,
                            "Offset by {distance} mm turns arc #{id} inside out: its radius would be {radius:.4} mm"
                        );
                        Carrier::Circle { c, r: radius }
                    }
                });
            }
            // joins[k] is the new corner between segment k and the next.
            let mut joins = Vec::with_capacity(n);
            for k in 0..n {
                let m = (k + 1) % n;
                let corner = segs[k].end();
                let e = carriers[k].moved(&segs[k], corner);
                let f = carriers[m].moved(&segs[m], segs[m].start());
                let mid = scale(add(e, f), 0.5);
                if super::distance(e, f) <= 1e-9 * (1.0 + distance.abs()) {
                    joins.push(mid);
                    continue;
                }
                let best = carriers[k]
                    .meet(&carriers[m])
                    .into_iter()
                    .min_by(|p, q| super::distance(*p, mid).total_cmp(&super::distance(*q, mid)));
                let Some(best) = best else {
                    bail!("Offset by {distance} mm cannot rejoin the corner at ({:.4}, {:.4})", corner[0], corner[1]);
                };
                joins.push(best);
            }
            // The new loop, each segment checked against the one it came from.
            let mut curves = Vec::with_capacity(n);
            let mut forward = Vec::with_capacity(n);
            for k in 0..n {
                let (p, q) = (joins[(k + n - 1) % n], joins[k]);
                let id = walk.entities[k];
                match (segs[k], carriers[k]) {
                    (Seg::Line { a, b }, _) => {
                        let d = sub(q, p);
                        ensure!(
                            d[0].hypot(d[1]) > SAME_MM && dot(d, sub(b, a)) > 0.0,
                            "Offset by {distance} mm turns line #{id} round; use a smaller distance"
                        );
                        curves.push(Curve::Line(Line { start: p, end: q }));
                        forward.push(true);
                    }
                    (Seg::Arc { a0, s: turn, .. }, Carrier::Circle { c, r }) => {
                        let (from, to) = (angle(c, p), angle(c, q));
                        let now = if turn > 0.0 { (to - from).rem_euclid(TAU) } else { -(from - to).rem_euclid(TAU) };
                        let drift = (from + now * 0.5) - (a0 + turn * 0.5);
                        ensure!(
                            now.abs() > 1e-9 && drift.cos() > 0.0,
                            "Offset by {distance} mm turns arc #{id} round; use a smaller distance"
                        );
                        let (start_angle, end_angle) = if turn > 0.0 { (from, to) } else { (to, from) };
                        curves.push(Curve::Arc(Arc { centre: c, radius: r, start_angle, end_angle }));
                        forward.push(turn > 0.0);
                    }
                    _ => bail!("Offset lost the circle of arc #{id}"),
                }
            }
            let made = region::Loop { curves, forward, entities: walk.entities.clone() };
            let boxes: Vec<Bounds> = made.curves.iter().map(Bounds::of).collect();
            if let Some(x) = region::self_crossing(&made, &boxes) {
                bail!("Offset by {distance} mm folds the loop through itself at ({:.4}, {:.4}); use a smaller distance", x[0], x[1]);
            }
            ensure!(made.area().signum() == sigma, "Offset by {distance} mm turns the loop inside out");
            // One closed polyline offsets to one closed polyline; anything else to lines and arcs.
            let one = walk.entities.iter().all(|id| *id == walk.entities[0]);
            if let (true, Geometry::Polyline { closed: true, .. }) = (one, &s.entities[s.index_of(walk.entities[0])?].geometry) {
                let points: Vec<Id> = (0..n).map(|k| s.point(joins[(k + n - 1) % n])).collect();
                return Ok(vec![s.add_entity(Geometry::Polyline { points, closed: true }, construction)]);
            }
            let ids: Vec<Id> = joins.iter().map(|p| s.point(*p)).collect();
            let mut out = Vec::with_capacity(n);
            for k in 0..n {
                let (p, q) = (ids[(k + n - 1) % n], ids[k]);
                let g = match segs[k] {
                    Seg::Line { .. } => Geometry::Line { a: p, b: q },
                    Seg::Arc { s: turn, centre, .. } if turn > 0.0 => Geometry::Arc { center: centre, start: p, end: q },
                    Seg::Arc { centre, .. } => Geometry::Arc { center: centre, start: q, end: p },
                };
                out.push(s.add_entity(g, construction));
            }
            Ok(out)
        })
    }

    /// The two straight curves ending at the point `corner`, or why it is not such a corner.
    fn arms(&self, corner: Id) -> Result<[Arm; 2]> {
        let p = self.at(corner)?;
        let here = |id: Id| self.at(id).is_ok_and(|q| distance(q, p) <= SAME_MM);
        let mut arms = Vec::new();
        let mut curved = Vec::new();
        for (i, e) in self.entities.iter().enumerate() {
            match &e.geometry {
                Geometry::Line { a, b } => {
                    if here(*a) {
                        arms.push(Arm { entity: i, at: 0, step: 0, here: *a, far: *b });
                    }
                    if here(*b) {
                        arms.push(Arm { entity: i, at: 1, step: 0, here: *b, far: *a });
                    }
                }
                Geometry::Polyline { points, closed } => {
                    let n = points.len();
                    for (k, id) in points.iter().enumerate().filter(|(_, id)| here(**id)) {
                        if k + 1 < n || *closed {
                            arms.push(Arm { entity: i, at: k, step: 1, here: *id, far: points[(k + 1) % n] });
                        }
                        if k > 0 || *closed {
                            arms.push(Arm { entity: i, at: k, step: -1, here: *id, far: points[(k + n - 1) % n] });
                        }
                    }
                }
                Geometry::Arc { start, end, .. } if here(*start) || here(*end) => curved.push(e.id),
                Geometry::Bezier { points } if here(points[0]) || here(points[3]) => curved.push(e.id),
                _ => {}
            }
        }
        if let Some(id) = curved.first() {
            bail!("A corner here is two lines; curve #{id} also ends at point #{corner}");
        }
        match arms.as_slice() {
            [a, b] => Ok([*a, *b]),
            _ => bail!("Point #{corner} ends {} lines; a corner is where exactly two meet", arms.len()),
        }
    }
    /// Cuts the corner the arms leave: each arm now ends at its point of `to`.
    fn cut_corner(&mut self, arms: [Arm; 2], to: [Id; 2]) -> Result<()> {
        if arms[0].entity == arms[1].entity && arms[0].at == arms[1].at && arms[0].step != 0 {
            let i = arms[0].entity;
            let Geometry::Polyline { points, closed } = self.entities[i].geometry.clone() else {
                bail!("A corner lost its polyline");
            };
            let (k, n) = (arms[0].at, points.len());
            let next = if arms[0].step > 0 { to[0] } else { to[1] };
            let previous = if arms[0].step > 0 { to[1] } else { to[0] };
            if closed {
                let mut r = vec![next];
                r.extend((1..n).map(|j| points[(k + j) % n]));
                r.push(previous);
                self.entities[i].geometry = Geometry::Polyline { points: r, closed: false };
            } else {
                let mut head = points[..k].to_vec();
                head.push(previous);
                let mut tail = vec![next];
                tail.extend(&points[k + 1..]);
                self.replace(i, vec![run(head), run(tail)]);
            }
            return Ok(());
        }
        for (arm, id) in arms.iter().zip(to) {
            match &mut self.entities[arm.entity].geometry {
                Geometry::Line { a, .. } if arm.at == 0 => *a = id,
                Geometry::Line { b, .. } => *b = id,
                Geometry::Polyline { points, .. } => points[arm.at] = id,
                _ => bail!("A corner lost its line"),
            }
        }
        Ok(())
    }
    /// The corner's far ends, the directions to them from the corner, and how long each arm is.
    fn corner_rays(&self, arms: &[Arm; 2]) -> Result<(V, [V; 2], [f64; 2])> {
        let p = self.at(arms[0].here)?;
        let mut dirs = [[0.0; 2]; 2];
        let mut lengths = [0.0; 2];
        for (k, arm) in arms.iter().enumerate() {
            let d = sub(self.at(arm.far)?, p);
            lengths[k] = d[0].hypot(d[1]);
            dirs[k] = unit(d).context("A corner line has no length")?;
        }
        ensure!(
            cross(dirs[0], dirs[1]).abs() > 1e-9,
            "The lines at point #{} run straight on; there is no corner to cut",
            arms[0].here
        );
        Ok((p, dirs, lengths))
    }

    /// Rounds the corner where two lines meet at `point` with a tangent arc of `radius`: each
    /// line ends where the arc meets it, the lines keep their direction constraints, and the arc
    /// is held tangent to both at its radius. Returns the arc.
    pub fn fillet_corner(&mut self, point: Id, radius: f64) -> Result<Id> {
        ensure!(radius.is_finite() && radius > SAME_MM && radius < 10000.0, "A fillet needs a positive radius");
        self.transact(|s| {
            let arms = s.arms(point)?;
            let (p, dirs, lengths) = s.corner_rays(&arms)?;
            let f = geom2d::fillet_between_rays(p, dirs[0], dirs[1], radius).context("These lines have no corner to round")?;
            let tangents = [f.tangent1, f.tangent2];
            for k in 0..2 {
                let need = super::distance(tangents[k], p);
                ensure!(
                    need < lengths[k] - SAME_MM,
                    "A {radius} mm fillet needs {need:.4} mm of each line; the line to point #{} is {:.4} mm",
                    arms[k].far,
                    lengths[k]
                );
            }
            let t = [s.point(tangents[0]), s.point(tangents[1])];
            let centre = s.point(f.centre);
            s.cut_corner(arms, t)?;
            let first = (angle(f.centre, tangents[0]) - f.start_angle).rem_euclid(TAU);
            let (start, end) = if first.min(TAU - first) < 1e-6 { (t[0], t[1]) } else { (t[1], t[0]) };
            let arc = s.add_entity(Geometry::Arc { center: centre, start, end }, false);
            for k in 0..2 {
                s.carry_direction((arms[k].far, arms[k].here), (arms[k].far, t[k]));
                s.constraints.push(Constraint::Tangent { a: arms[k].far, b: t[k], center: centre, at: t[k] });
                s.constraints.push(Constraint::Distance { a: centre, b: t[k], mm: radius });
            }
            s.drop_orphans([arms[0].here, arms[1].here]);
            Ok(arc)
        })
    }

    /// Cuts the corner where two lines meet at `point` with a straight line `distance` along each.
    /// Returns the new line.
    pub fn chamfer_corner(&mut self, point: Id, distance: f64) -> Result<Id> {
        ensure!(distance.is_finite() && distance > SAME_MM && distance < 10000.0, "A chamfer needs a positive distance");
        self.transact(|s| {
            let arms = s.arms(point)?;
            let (p, dirs, lengths) = s.corner_rays(&arms)?;
            for k in 0..2 {
                ensure!(
                    distance < lengths[k] - SAME_MM,
                    "A {distance} mm chamfer is longer than the line to point #{} ({:.4} mm)",
                    arms[k].far,
                    lengths[k]
                );
            }
            let t = [s.point(add(p, scale(dirs[0], distance))), s.point(add(p, scale(dirs[1], distance)))];
            s.cut_corner(arms, t)?;
            let line = s.add_entity(Geometry::Line { a: t[0], b: t[1] }, false);
            for k in 0..2 {
                s.carry_direction((arms[k].far, arms[k].here), (arms[k].far, t[k]));
            }
            s.drop_orphans([arms[0].here, arms[1].here]);
            Ok(line)
        })
    }

    /// Copies of `entities` reflected in the line `axis`; points on the axis are shared rather
    /// than copied, so a half drawn against the axis closes with its mirror. Returns the copies.
    pub fn mirror(&mut self, entities: &[Id], axis: Id) -> Result<Vec<Id>> {
        self.transact(|s| {
            let Geometry::Line { a, b } = s.entities[s.index_of(axis)?].geometry else {
                bail!("A mirror axis is a line; #{axis} is not one");
            };
            let a = s.at(a)?;
            let u = unit(sub(s.at(b)?, a)).context("The mirror axis has no length")?;
            s.copy_mapped(entities, |p| {
                let d = sub(p, a);
                sub(add(a, scale(u, 2.0 * dot(d, u))), d)
            }, true)
        })
    }

    /// Copies of `entities` laid out by `pattern`, the original counting as the first; points a
    /// copy maps onto themselves, such as a polar pattern's centre, are shared. Returns each
    /// copy's entities in order.
    pub fn pattern(&mut self, entities: &[Id], pattern: &Pattern) -> Result<Vec<Vec<Id>>> {
        let moves: Vec<[f64; 3]> = match *pattern {
            Pattern::Rect { step, count } => {
                ensure!(
                    step.iter().flatten().all(|v| v.is_finite() && v.abs() < 10000.0),
                    "A pattern step must be a finite length"
                );
                ensure!(
                    count[0] >= 1 && count[1] >= 1 && count[0].saturating_mul(count[1]) <= MAX_COPIES + 1,
                    "A pattern lays down 1 to {} copies",
                    MAX_COPIES + 1
                );
                for (k, n) in count.iter().enumerate() {
                    ensure!(*n == 1 || step[k][0].hypot(step[k][1]) > SAME_MM, "A pattern of {n} needs a step that moves");
                }
                (0..count[1])
                    .flat_map(|j| (0..count[0]).map(move |i| (i, j)))
                    .skip(1)
                    .map(|(i, j)| {
                        let (i, j) = (i as f64, j as f64);
                        [step[0][0] * i + step[1][0] * j, step[0][1] * i + step[1][1] * j, 0.0]
                    })
                    .collect()
            }
            Pattern::Polar { centre, count, sweep_deg } => {
                ensure!(centre.iter().all(|v| v.is_finite()), "A pattern centre must be finite");
                ensure!((1..=MAX_COPIES + 1).contains(&count), "A pattern lays down 1 to {} copies", MAX_COPIES + 1);
                ensure!(sweep_deg.is_finite() && sweep_deg.abs() > 1e-9, "A polar pattern needs an angle");
                let step = if sweep_deg.abs() >= 360.0 - 1e-9 {
                    sweep_deg.signum() * 360.0 / count as f64
                } else {
                    sweep_deg / (count.max(2) - 1) as f64
                };
                (1..count).map(|k| [centre[0], centre[1], (step * k as f64).to_radians()]).collect()
            }
        };
        let turning = matches!(pattern, Pattern::Polar { .. });
        self.transact(|s| {
            let mut out = Vec::with_capacity(moves.len());
            for m in &moves {
                let (sin, cos) = m[2].sin_cos();
                out.push(s.copy_mapped(entities, |p| {
                    if turning {
                        let d = [p[0] - m[0], p[1] - m[1]];
                        [m[0] + d[0] * cos - d[1] * sin, m[1] + d[0] * sin + d[1] * cos]
                    } else {
                        [p[0] + m[0], p[1] + m[1]]
                    }
                }, false)?);
            }
            Ok(out)
        })
    }

    /// Copies of `entities` with every point moved by `map`; a point `map` leaves where it is is
    /// shared. `reflects` swaps each arc's ends, since a reflection turns it clockwise.
    fn copy_mapped(&mut self, entities: &[Id], map: impl Fn(V) -> V, reflects: bool) -> Result<Vec<Id>> {
        ensure!(!entities.is_empty(), "Select the geometry to copy");
        let mut moved: HashMap<Id, Id> = HashMap::new();
        let mut out = Vec::new();
        for id in region::distinct(entities.to_vec()) {
            let e: Entity = self.entities[self.index_of(id)?].clone();
            let mut to = |s: &mut Sketch, p: Id| -> Result<Id> {
                if let Some(q) = moved.get(&p) {
                    return Ok(*q);
                }
                let xy = s.at(p)?;
                let there = map(xy);
                ensure!(there.iter().all(|v| v.is_finite()), "A copied point is not finite");
                let q = if super::distance(xy, there) <= SAME_MM { p } else { s.point(there) };
                moved.insert(p, q);
                Ok(q)
            };
            let g = match &e.geometry {
                Geometry::Line { a, b } => Geometry::Line { a: to(self, *a)?, b: to(self, *b)? },
                Geometry::Polyline { points, closed } => {
                    let points = points.iter().map(|p| to(self, *p)).collect::<Result<Vec<_>>>()?;
                    Geometry::Polyline { points, closed: *closed }
                }
                Geometry::Circle { center, rim } => Geometry::Circle { center: to(self, *center)?, rim: to(self, *rim)? },
                Geometry::Arc { center, start, end } => {
                    let (center, start, end) = (to(self, *center)?, to(self, *start)?, to(self, *end)?);
                    if reflects { Geometry::Arc { center, start: end, end: start } } else { Geometry::Arc { center, start, end } }
                }
                Geometry::Bezier { points } => {
                    let mut out = [0; 4];
                    for (k, p) in points.iter().enumerate() {
                        out[k] = to(self, *p)?;
                    }
                    Geometry::Bezier { points: out }
                }
            };
            out.push(self.add_entity(g, e.construction));
        }
        Ok(out)
    }
}

/// The parameter span a trim removed from `[0, 1]`, read off what it kept.
fn removed(kept: &[[f64; 2]]) -> (f64, f64) {
    let r0 = kept.iter().find(|s| s[0] <= 0.0).map_or(0.0, |s| s[1]);
    let r1 = kept.iter().find(|s| s[1] >= 1.0).map_or(1.0, |s| s[0]);
    (r0, r1)
}

/// A run of points as a line when it is two, an open polyline when it is more.
fn run(points: Vec<Id>) -> Geometry {
    match points.as_slice() {
        [a, b] => Geometry::Line { a: *a, b: *b },
        _ => Geometry::Polyline { points, closed: false },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn length(s: &Sketch, id: Id) -> f64 {
        let e = s.entities.iter().find(|e| e.id == id).unwrap();
        s.curves_of(e).unwrap().iter().map(Curve::length).sum()
    }
    fn lines_of(s: &Sketch) -> Vec<f64> {
        let mut out: Vec<f64> = s
            .entities
            .iter()
            .flat_map(|e| s.curves_of(e).unwrap())
            .filter(|c| matches!(c, Curve::Line(_)))
            .map(|c| (c.length() * 1e9).round() / 1e9)
            .collect();
        out.sort_by(f64::total_cmp);
        out
    }

    #[test]
    fn a_rectangle_offsets_out_and_in_and_refuses_to_turn_inside_out() {
        let mut s = Sketch::rectangle(10.0, 6.0);
        let before = s.clone();
        let rect = s.entities[0].id;
        let out = s.offset(&[rect], 1.0).unwrap();
        assert_eq!(out.len(), 1);
        let grown = s.entities.iter().find(|e| e.id == out[0]).unwrap();
        let Geometry::Polyline { points, closed: true } = &grown.geometry else { panic!("{grown:?}") };
        let xy: Vec<V> = points.iter().map(|p| s.at(*p).unwrap()).collect();
        for (p, q) in xy.iter().zip([[-6.0, -4.0], [6.0, -4.0], [6.0, 4.0], [-6.0, 4.0]]) {
            assert!(distance(*p, q) < 1e-12, "{xy:?}");
        }
        let area = region::loop_moments(&s.curves_of(grown).unwrap(), [0.0; 2]).unwrap()[0];
        assert!((area - 96.0).abs() < 1e-12, "{area}");
        // Inward by 2.9 leaves a 4.2 × 0.2 slot; by 3 the short sides would vanish, and nothing changes.
        let mut t = before.clone();
        let inner = t.offset(&[rect], -2.9).unwrap();
        let area = region::loop_moments(&t.curves_of(t.entities.iter().find(|e| e.id == inner[0]).unwrap()).unwrap(), [0.0; 2]).unwrap()[0];
        assert!((area - 4.2 * 0.2).abs() < 1e-9, "{area}");
        let mut t = before.clone();
        let error = t.offset(&[rect], -3.0).err().unwrap().to_string();
        assert!(error.contains("turns line") && error.contains("round"), "{error}");
        assert_eq!(t, before);
        // A slot of two lines and two half circles stays lines and arcs about the same centres.
        let mut s = Sketch::default();
        let p = [[-2.0, -1.0], [2.0, -1.0], [2.0, 1.0], [-2.0, 1.0], [2.0, 0.0], [-2.0, 0.0]].map(|p| s.point(p));
        let bottom = s.entity(Geometry::Line { a: p[0], b: p[1] });
        let right = s.entity(Geometry::Arc { center: p[4], start: p[1], end: p[2] });
        let top = s.entity(Geometry::Line { a: p[2], b: p[3] });
        let left = s.entity(Geometry::Arc { center: p[5], start: p[3], end: p[0] });
        let made = s.offset(&[bottom, right, top, left], 0.5).unwrap();
        assert_eq!(made.len(), 4);
        let loop_curves: Vec<Curve> = made.iter().flat_map(|id| s.curves_of(s.entities.iter().find(|e| e.id == *id).unwrap()).unwrap()).collect();
        let area = region::loop_moments(&loop_curves, [0.0; 2]).unwrap()[0].abs();
        assert!((area - (4.0 * 3.0 + PI * 1.5 * 1.5)).abs() < 1e-9, "{area}");
        let centres: BTreeSet<Id> = made
            .iter()
            .filter_map(|id| match s.entities.iter().find(|e| e.id == *id).unwrap().geometry {
                Geometry::Arc { center, .. } => Some(center),
                _ => None,
            })
            .collect();
        assert_eq!(centres, BTreeSet::from([p[4], p[5]]));
        // Inward past the arcs' radius turns them inside out.
        let error = s.offset(&[bottom, right, top, left], -1.0).err().unwrap().to_string();
        assert!(error.contains("inside out"), "{error}");
        // A circle offsets to a circle.
        let mut c = Sketch::circle(2.0);
        let ring = c.offset(&[c.entities[0].id], 0.5).unwrap();
        assert!((length(&c, ring[0]) - 2.0 * PI * 2.5).abs() < 1e-12);
    }

    #[test]
    fn a_corner_fillet_shortens_both_lines_by_its_setback() {
        let mut s = Sketch::rectangle(10.0, 6.0);
        let corner = s.points[1].id;
        let arc = s.fillet_corner(corner, 1.0).unwrap();
        assert!((length(&s, arc) - PI / 2.0).abs() < 1e-12, "{}", length(&s, arc));
        assert_eq!(lines_of(&s), vec![5.0, 6.0, 9.0, 10.0]);
        assert!(s.points.iter().all(|p| p.id != corner), "the corner point is gone");
        // The shortened sides keep their direction constraints, and the arc its tangency: it still solves.
        let solved = s.solve().unwrap();
        assert!(solved.residual_mm < 1e-6);
        assert!(s.constraints.iter().any(|c| matches!(c, Constraint::Tangent { .. })));
        let profile = s.profile_curves().unwrap();
        let area = region::loop_moments(&profile, [0.0; 2]).unwrap()[0].abs();
        assert!((area - (60.0 - (1.0 - PI / 4.0))).abs() < 1e-9, "{area}");
        // A radius the lines cannot hold is refused and the sketch is unchanged.
        let before = s.clone();
        let error = s.fillet_corner(s.points[2].id, 6.5).err().unwrap().to_string();
        assert!(error.contains("needs"), "{error}");
        assert_eq!(s, before);
        // Two separate lines meeting at a point, then a chamfer.
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 0.0], [0.0, 3.0]].map(|p| s.point(p));
        s.entity(Geometry::Line { a: p[0], b: p[1] });
        s.entity(Geometry::Line { a: p[2], b: p[0] });
        let cut = s.chamfer_corner(p[0], 1.0).unwrap();
        assert!((length(&s, cut) - 2f64.sqrt()).abs() < 1e-12);
        assert_eq!(lines_of(&s), [2f64.sqrt(), 2.0, 3.0].map(|v| (v * 1e9).round() / 1e9).to_vec());
        let error = s.fillet_corner(p[1], 0.5).err().unwrap().to_string();
        assert!(error.contains("ends 1 lines"), "{error}");
    }

    #[test]
    fn trim_removes_the_span_between_the_nearest_crossings() {
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [10.0, 0.0], [3.0, -2.0], [3.0, 2.0], [7.0, -2.0], [7.0, 2.0]].map(|p| s.point(p));
        let long = s.entity(Geometry::Line { a: p[0], b: p[1] });
        s.entity(Geometry::Line { a: p[2], b: p[3] });
        s.entity(Geometry::Line { a: p[4], b: p[5] });
        s.constraints.push(Constraint::Horizontal(p[0], p[1]));
        let left = s.trim(long, [5.0, 0.1]).unwrap();
        assert_eq!(left.len(), 2, "trimming the middle of a line crossed twice leaves two lines");
        assert_eq!(lines_of(&s), vec![3.0, 3.0, 4.0, 4.0]);
        assert_eq!(s.constraints, vec![Constraint::Horizontal(p[0], p[1])], "both ends stand, so the constraint still applies");
        // A span crossed only at its own end goes whole.
        let end = s.trim(left[1], [9.0, 0.0]).unwrap();
        assert!(end.is_empty(), "nothing crosses the last span but its own end: it goes whole");
        let first = s.trim(left[0], [0.5, 0.0]).unwrap();
        assert!(first.is_empty());
        // A circle crossed twice keeps the arc away from the pick; crossed once it goes whole.
        let mut s = Sketch::circle(2.0);
        let circle = s.entities[0].id;
        let q = [[-3.0, 0.0], [3.0, 0.0]].map(|p| s.point(p));
        s.entity(Geometry::Line { a: q[0], b: q[1] });
        let arc = s.trim(circle, [0.0, 2.0]).unwrap();
        assert_eq!(arc, vec![circle]);
        assert!((length(&s, circle) - 2.0 * PI).abs() < 1e-12, "the lower half stays");
        let Geometry::Arc { start, .. } = s.entities[0].geometry else { panic!() };
        assert!(distance(s.at(start).unwrap(), [-2.0, 0.0]) < 1e-12, "counter-clockwise from the left crossing");
        // A closed polyline opens where its segment was trimmed.
        let mut s = Sketch::rectangle(10.0, 6.0);
        let rect = s.entities[0].id;
        let q = [[-1.0, -4.0], [-1.0, -2.0], [1.0, -4.0], [1.0, -2.0]].map(|p| s.point(p));
        s.entity(Geometry::Line { a: q[0], b: q[1] });
        s.entity(Geometry::Line { a: q[2], b: q[3] });
        let left = s.trim(rect, [0.0, -3.0]).unwrap();
        assert_eq!(left, vec![rect]);
        let Geometry::Polyline { points, closed: false } = &s.entities[0].geometry else { panic!() };
        assert_eq!(points.len(), 6);
        assert!((length(&s, rect) - 30.0).abs() < 1e-12, "{}", length(&s, rect));
    }

    #[test]
    fn crossings_split_into_shared_points_and_mirror_and_pattern_copy() {
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 0.0], [2.0, -2.0], [2.0, 2.0], [2.0, 0.5], [3.0, 0.5]].map(|p| s.point(p));
        let a = s.entity(Geometry::Line { a: p[0], b: p[1] });
        s.entity(Geometry::Line { a: p[2], b: p[3] });
        let c = s.entity(Geometry::Circle { center: p[4], rim: p[5] });
        let added = s.split_at_intersections().unwrap();
        // Each line is cut three times and the circle four: nine pieces added, one point per crossing.
        assert_eq!((added.len(), s.entities.len()), (9, 12));
        let shared = s.points.iter().filter(|q| distance(q.xy, [2.0, 0.0]) < 1e-12).count();
        assert_eq!(shared, 1);
        assert!(matches!(s.entities.iter().find(|e| e.id == c).unwrap().geometry, Geometry::Arc { .. }));
        assert!((length(&s, a) - (2.0 - 0.75f64.sqrt())).abs() < 1e-12, "{}", length(&s, a));
        assert!(s.points.iter().all(|q| q.id != p[5]), "the circle's rim point went with the circle");
        let arcs: f64 = s.entities.iter().filter(|e| matches!(e.geometry, Geometry::Arc { .. })).map(|e| length(&s, e.id)).sum();
        assert!((arcs - TAU).abs() < 1e-12, "{arcs}");
        // Mirror a quarter of a square across the y axis: the point on the axis is shared.
        let mut s = Sketch::default();
        let q = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 3.0], [0.0, -1.0]].map(|p| s.point(p));
        let half = s.entity(Geometry::Polyline { points: q[..4].to_vec(), closed: false });
        let axis = s.entity(Geometry::Line { a: q[4], b: q[3] });
        s.entities.iter_mut().find(|e| e.id == axis).unwrap().construction = true;
        let copy = s.mirror(&[half], axis).unwrap();
        let Geometry::Polyline { points, .. } = &s.entities.iter().find(|e| e.id == copy[0]).unwrap().geometry else { panic!() };
        assert_eq!((points[0], points[3]), (q[0], q[3]), "points on the axis are shared");
        assert_eq!(s.at(points[2]).unwrap(), [-2.0, 2.0]);
        let r = s.profile_regions().unwrap();
        assert_eq!(r.len(), 1);
        assert!((r[0].area() - 10.0).abs() < 1e-12, "{}", r[0].area());
        // A polar pattern of a spoke shares the hub; a grid of squares makes separate regions.
        let mut s = Sketch::default();
        let hub = s.point([0.0, 0.0]);
        let tip = s.point([3.0, 0.0]);
        let spoke = s.entity(Geometry::Line { a: hub, b: tip });
        let copies = s.pattern(&[spoke], &Pattern::Polar { centre: [0.0, 0.0], count: 6, sweep_deg: 360.0 }).unwrap();
        assert_eq!(copies.len(), 5);
        let last = s.entities.iter().find(|e| e.id == copies[4][0]).unwrap();
        let Geometry::Line { a, b } = last.geometry else { panic!() };
        assert_eq!(a, hub);
        assert!(distance(s.at(b).unwrap(), [3.0 * (300f64).to_radians().cos(), 3.0 * (300f64).to_radians().sin()]) < 1e-12);
        let mut s = Sketch::rectangle(1.0, 1.0);
        let cell = s.entities[0].id;
        let grid = s.pattern(&[cell], &Pattern::Rect { step: [[2.0, 0.0], [0.0, 2.0]], count: [3, 2] }).unwrap();
        assert_eq!(grid.len(), 5);
        assert_eq!(s.profile_regions().unwrap().len(), 6);
        let too_many = s.pattern(&[cell], &Pattern::Rect { step: [[2.0, 0.0], [0.0, 2.0]], count: [600, 1] }).err().unwrap().to_string();
        assert!(too_many.contains("1 to 512"), "{too_many}");
        assert_eq!(s.loop_through(grid[4][0]).unwrap(), grid[4]);
    }
}
