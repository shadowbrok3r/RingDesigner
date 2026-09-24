//! The drawn curves as one graph: every entity cut where another's end lies on it, so a trimmed
//! line ending on a side closes exactly as if that side were split there; the overhangs such a cut
//! leaves pruned; and the faces a sketch whose curves branch divides the plane into.
use super::region::{Bounds, CROSSING_MM, JOINT_MM, Loop, Region, distinct, halves, walked_moments};
use super::{Geometry, Id, Sketch, distance};
use anyhow::{Context, Result, bail};
use cadkernel::geom2d::{self, Arc, Curve, Line, Tolerance};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::TAU;

/// A place as the chain reads it: millimetres on the `JOINT_MM` grid.
pub(super) type Key = [i64; 2];

/// The chain key of a place.
pub(super) fn key(xy: [f64; 2]) -> Key {
    xy.map(|v| (v * 1e6).round() as i64)
}

/// One drawn entity as the graph reads it.
struct Drawn {
    id: Id,
    curves: Vec<Curve>,
    /// Where each curve starts, and for an open entity where the last ends; a circle has none.
    stations: Vec<([f64; 2], Id)>,
    closed: bool,
}

impl Drawn {
    fn of(s: &Sketch, id: Id, geometry: &Geometry, curves: Vec<Curve>) -> Result<Self> {
        let at = |p: Id| -> Result<([f64; 2], Id)> { Ok((s.at(p)?, p)) };
        let (stations, closed) = match geometry {
            Geometry::Line { a, b } => (vec![at(*a)?, at(*b)?], false),
            Geometry::Polyline { points, closed } => (points.iter().map(|p| at(*p)).collect::<Result<Vec<_>>>()?, *closed),
            Geometry::Circle { .. } => (Vec::new(), true),
            Geometry::Arc { start, end, .. } => (vec![at(*start)?, at(*end)?], false),
            Geometry::Bezier { points } => (vec![at(points[0])?, at(points[3])?], false),
        };
        let mut d = Self { id, curves, stations, closed };
        // An open entity whose ends share a place closes on itself there.
        if !d.closed && d.stations.len() >= 2 && key(d.stations[0].0) == key(d.stations[d.stations.len() - 1].0) {
            d.closed = true;
            d.stations.pop();
        }
        Ok(d)
    }
    /// The stations another entity's end can land on: every one of a closed entity, the inner ones of an open one.
    fn inner_stations(&self) -> std::ops::Range<usize> {
        if self.closed { 0..self.stations.len() } else { 1..self.stations.len().saturating_sub(1) }
    }
}

/// Where another entity's end lands on this one: `u` is the curve index plus the parameter along it.
#[derive(Clone, Copy, Debug)]
struct Joint {
    u: f64,
    xy: [f64; 2],
    key: Key,
    name: Id,
}

/// Every curve filed under the cells of a grid its box covers, so a place asks only the curves near it.
struct Grid {
    lo: [f64; 2],
    cell: f64,
    side: usize,
    cells: Vec<Vec<(usize, usize)>>,
}

impl Grid {
    /// The curves of `boxes`, indexed by entity and curve, on a grid about as many cells a side as the square root of their count.
    fn new(boxes: &[Vec<Bounds>]) -> Self {
        let all: Vec<Bounds> = boxes.iter().flatten().copied().collect();
        let whole = Bounds::union(&all);
        let side = ((all.len() as f64).sqrt().ceil() as usize).clamp(1, 64);
        let reach = (0..2).map(|k| whole.hi[k] - whole.lo[k]).fold(0.0_f64, f64::max);
        let cell = if reach.is_finite() && reach > 0.0 { reach / side as f64 } else { 1.0 };
        let mut grid = Self { lo: whole.lo, cell, side, cells: vec![Vec::new(); side * side] };
        for (i, curves) in boxes.iter().enumerate() {
            for (j, b) in curves.iter().enumerate() {
                let (c0, r0) = grid.at(b.lo);
                let (c1, r1) = grid.at(b.hi);
                for r in r0..=r1 {
                    for c in c0..=c1 {
                        grid.cells[r * side + c].push((i, j));
                    }
                }
            }
        }
        grid
    }
    /// The cell holding `p`, clamped onto the grid.
    fn at(&self, p: [f64; 2]) -> (usize, usize) {
        let index = |k: usize| {
            let v = ((p[k] - self.lo[k]) / self.cell).floor();
            if v.is_finite() { (v.max(0.0) as usize).min(self.side - 1) } else { 0 }
        };
        (index(0), index(1))
    }
    /// The curves filed under the cell holding `p`.
    fn near(&self, p: [f64; 2]) -> &[(usize, usize)] {
        let (c, r) = self.at(p);
        &self.cells[r * self.side + c]
    }
}

/// One stretch of an entity between two vertices of the graph.
#[derive(Clone, Debug)]
pub(super) struct Piece {
    pub entity: Id,
    /// In the entity's own direction.
    pub curves: Vec<Curve>,
    pub ends: [Key; 2],
    /// The points naming each end, for a message.
    pub names: [Id; 2],
}

/// The sketch's profile curves as vertices and the pieces between them, with what pruning left.
pub(super) struct Graph {
    /// Closed entities nothing ends on, each a loop of its own.
    pub rings: Vec<(Vec<Curve>, Id)>,
    pub pieces: Vec<Piece>,
    /// Whether each piece lies on a cycle or between cycles, rather than hanging free.
    pub alive: Vec<bool>,
    /// Pieces left hanging whose entity keeps no other piece on a cycle: a real free end.
    pub strays: Vec<usize>,
}

impl Graph {
    /// Every non-construction entity of `solved` cut at the joints other entities' ends make on it.
    pub fn build(solved: &Sketch) -> Result<Self> {
        let mut drawn = Vec::new();
        for e in solved.entities.iter().filter(|e| !e.construction) {
            drawn.push(Drawn::of(solved, e.id, &e.geometry, solved.curves_of(e)?)?);
        }
        let boxes: Vec<Vec<Bounds>> = drawn.iter().map(|d| d.curves.iter().map(Bounds::of).collect()).collect();
        let grid = Grid::new(&boxes);
        let mut stations: HashMap<Key, Vec<(usize, usize)>> = HashMap::new();
        for (i, d) in drawn.iter().enumerate() {
            for s in d.inner_stations() {
                stations.entry(key(d.stations[s].0)).or_default().push((i, s));
            }
        }
        let mut joints: Vec<Vec<Joint>> = vec![Vec::new(); drawn.len()];
        for d in drawn.iter().filter(|d| !d.closed) {
            for &(xy, name) in [d.stations[0], d.stations[d.stations.len() - 1]].iter() {
                let k = key(xy);
                // A station within the joint's reach sits on the same key or a neighbouring one.
                for dk in [[-1, -1], [-1, 0], [-1, 1], [0, -1], [0, 0], [0, 1], [1, -1], [1, 0], [1, 1]] {
                    for &(i, s) in stations.get(&[k[0] + dk[0], k[1] + dk[1]]).into_iter().flatten() {
                        let (p, _) = drawn[i].stations[s];
                        if key(p) == k || distance(p, xy) <= JOINT_MM {
                            joints[i].push(Joint { u: s as f64, xy, key: k, name });
                        }
                    }
                }
                for &(i, j) in grid.near(xy) {
                    let c = &drawn[i].curves[j];
                    if !boxes[i][j].holds(xy) {
                        continue;
                    }
                    // A curve's own ends are stations, weighed above; a circle has none.
                    let whole = matches!(c, Curve::Circle(_));
                    if !whole && (distance(c.point_at(0.0), xy) <= JOINT_MM || distance(c.point_at(1.0), xy) <= JOINT_MM) {
                        continue;
                    }
                    let hit = geom2d::closest_point(c, xy);
                    if hit.distance <= JOINT_MM {
                        joints[i].push(Joint { u: j as f64 + hit.t.clamp(0.0, 1.0), xy, key: k, name });
                    }
                }
            }
        }
        let mut rings = Vec::new();
        let mut pieces = Vec::new();
        for (d, mut js) in drawn.iter().zip(joints) {
            js.sort_by(|a, b| a.u.total_cmp(&b.u));
            js.dedup_by(|a, b| a.key == b.key || distance(a.xy, b.xy) <= JOINT_MM);
            cut(d, &js, &mut rings, &mut pieces)?;
        }
        let mut g = Self { rings, alive: vec![true; pieces.len()], pieces, strays: Vec::new() };
        g.prune();
        Ok(g)
    }

    /// How many pieces end at each vertex, and the point naming it, before pruning.
    fn degrees(&self, only: impl Fn(usize) -> bool) -> BTreeMap<Key, (usize, Id)> {
        let mut degree: BTreeMap<Key, (usize, Id)> = BTreeMap::new();
        for (_, p) in self.pieces.iter().enumerate().filter(|(i, _)| only(*i)) {
            for side in 0..2 {
                degree.entry(p.ends[side]).or_insert((0, p.names[side])).0 += 1;
            }
        }
        degree
    }

    /// Takes away, until none is left, every piece with an end no other piece reaches; one whose entity keeps a piece on a cycle is an overhang, the rest are strays.
    fn prune(&mut self) {
        let mut degree: HashMap<Key, usize> = HashMap::new();
        let mut touching: HashMap<Key, Vec<usize>> = HashMap::new();
        for (i, p) in self.pieces.iter().enumerate() {
            for side in 0..2 {
                *degree.entry(p.ends[side]).or_default() += 1;
                touching.entry(p.ends[side]).or_default().push(i);
            }
        }
        let mut queue: Vec<Key> = degree.iter().filter(|(_, n)| **n == 1).map(|(k, _)| *k).collect();
        queue.sort_unstable();
        while let Some(k) = queue.pop() {
            if degree.get(&k) != Some(&1) {
                continue;
            }
            let Some(&i) = touching[&k].iter().find(|i| self.alive[**i]) else { continue };
            self.alive[i] = false;
            for side in 0..2 {
                let end = self.pieces[i].ends[side];
                let n = degree.entry(end).or_default();
                *n = n.saturating_sub(1);
                if *n == 1 {
                    queue.push(end);
                }
            }
        }
        let kept: BTreeSet<Id> = self.pieces.iter().zip(&self.alive).filter(|(_, a)| **a).map(|(p, _)| p.entity).collect();
        self.strays = (0..self.pieces.len()).filter(|i| !self.alive[*i] && !kept.contains(&self.pieces[*i].entity)).collect();
    }

    /// Refused with a free end when a stray is left: walking on from the first stray as a chain does, the first end no piece continues from.
    pub fn closes(&self) -> Result<()> {
        let Some(&first) = self.strays.first() else { return Ok(()) };
        let degree = self.degrees(|_| true);
        let free = |k: &Key| degree.get(k).is_some_and(|(n, _)| *n == 1);
        for side in [1, 0] {
            let mut used = BTreeSet::from([first]);
            let (mut at, mut name) = (self.pieces[first].ends[side], self.pieces[first].names[side]);
            loop {
                if free(&at) {
                    bail!("Sketch profile is open at point #{name}; join it to close the loop");
                }
                let next = self.strays.iter().copied().find(|i| !used.contains(i) && self.pieces[*i].ends.contains(&at));
                let Some(i) = next else { break };
                used.insert(i);
                let far = usize::from(self.pieces[i].ends[0] == at);
                (at, name) = (self.pieces[i].ends[far], self.pieces[i].names[far]);
            }
        }
        let name = self.strays.iter().flat_map(|i| (0..2).map(move |side| (*i, side))).find(|(i, side)| free(&self.pieces[*i].ends[*side])).map_or(self.pieces[first].names[1], |(i, side)| self.pieces[i].names[side]);
        bail!("Sketch profile is open at point #{name}; join it to close the loop")
    }

    /// The first vertex, in place order, where three or more pieces left on cycles meet, and how many.
    pub fn branch(&self) -> Option<(Id, usize)> {
        self.degrees(|i| self.alive[i]).values().find(|(n, _)| *n > 2).map(|(n, id)| (*id, *n))
    }

    /// Every closed loop in joining order: rings first, then chains of pieces; refused where the curves branch.
    pub fn loops(&self) -> Result<Vec<Loop>> {
        self.closes()?;
        if let Some((id, n)) = self.branch() {
            bail!("Sketch point #{id} joins {n} curves; a profile loop passes through a point once. Trim the extra curve or mark it Construction");
        }
        let mut out: Vec<Loop> = self
            .rings
            .iter()
            .map(|(curves, id)| {
                let curves: Vec<Curve> = curves.iter().cloned().flat_map(halves).collect();
                Loop { forward: vec![true; curves.len()], entities: vec![*id; curves.len()], curves }
            })
            .collect();
        let mut open: Vec<&Piece> = self.pieces.iter().zip(&self.alive).filter(|(_, a)| **a).map(|(p, _)| p).collect();
        ensure_any(!out.is_empty() || !open.is_empty())?;
        while !open.is_empty() {
            let first = open.remove(0);
            let (start, mut end, mut name) = (first.ends[0], first.ends[1], first.names[1]);
            let mut chain = Loop { forward: vec![true; first.curves.len()], entities: vec![first.entity; first.curves.len()], curves: first.curves.clone() };
            while end != start {
                let Some(i) = open.iter().position(|p| p.ends.contains(&end)) else {
                    bail!("Sketch profile is open at point #{name}; join it to close the loop");
                };
                let p = open.remove(i);
                let forward = p.ends[0] == end;
                let mut curves = p.curves.clone();
                if forward {
                    (end, name) = (p.ends[1], p.names[1]);
                } else {
                    curves.reverse();
                    (end, name) = (p.ends[0], p.names[0]);
                }
                chain.forward.extend(std::iter::repeat_n(forward, curves.len()));
                chain.entities.extend(std::iter::repeat_n(p.entity, curves.len()));
                chain.curves.extend(curves);
            }
            out.push(chain);
        }
        Ok(out)
    }

    /// Every region a branching sketch bounds: the faces its pieces divide the plane into, a face a
    /// region when as many outlines of other groups of curves hold it as make an even count, with
    /// the outlines of the groups one deeper inside it as holes. Curves that cross are refused.
    pub fn cells(&self) -> Result<Vec<Region>> {
        self.closes()?;
        let mut keep: Vec<usize> = (0..self.pieces.len()).filter(|i| self.alive[*i]).collect();
        self.check_crossings(&keep)?;
        // A piece with the same face on both sides runs between cycles and bounds nothing.
        let first = trace(&self.pieces, &keep);
        let mut face_of: HashMap<usize, usize> = HashMap::new();
        for (f, walk) in first.iter().enumerate() {
            for he in walk {
                face_of.insert(*he, f);
            }
        }
        keep.retain(|i| face_of.get(&(2 * i)) != face_of.get(&(2 * i + 1)));
        let walks = trace(&self.pieces, &keep);
        // Groups of pieces joined at their ends; a ring is a group of its own.
        let mut parent: HashMap<Key, Key> = HashMap::new();
        fn root(parent: &mut HashMap<Key, Key>, k: Key) -> Key {
            let mut r = k;
            while let Some(&p) = parent.get(&r).filter(|p| **p != r) {
                r = p;
            }
            parent.insert(k, r);
            r
        }
        for &i in &keep {
            let [a, b] = self.pieces[i].ends;
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            parent.insert(ra, rb);
        }
        struct Group {
            outline: Option<Walked>,
            cells: Vec<Walked>,
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut index: HashMap<Key, usize> = HashMap::new();
        for walk in &walks {
            let w = walked(&self.pieces, walk);
            let area = walked_moments(&w.curves, &w.forward, [0.0; 2])[0];
            if area.abs() <= 1e-12 {
                continue;
            }
            let tail = self.pieces[walk[0] / 2].ends[walk[0] % 2];
            let r = root(&mut parent, tail);
            let g = *index.entry(r).or_insert_with(|| {
                groups.push(Group { outline: None, cells: Vec::new() });
                groups.len() - 1
            });
            if area > 0.0 {
                groups[g].cells.push(w);
            } else {
                groups[g].outline = Some(w);
            }
        }
        for (curves, id) in &self.rings {
            let curves: Vec<Curve> = curves.iter().cloned().flat_map(halves).collect();
            let w = Walked { forward: vec![true; curves.len()], entities: vec![*id; curves.len()], curves };
            groups.push(Group { outline: Some(w.clone()), cells: vec![w] });
        }
        groups.retain(|g| g.outline.is_some() && !g.cells.is_empty());
        let tol = Tolerance::new(CROSSING_MM);
        let probe: Vec<[f64; 2]> = groups.iter().map(|g| g.outline.as_ref().map_or([0.0; 2], |o| o.curves[0].point_at(0.5))).collect();
        let inside = |w: &Walked, p: [f64; 2]| geom2d::contains(&w.curves, p, tol);
        let depth: Vec<usize> = (0..groups.len())
            .map(|g| (0..groups.len()).filter(|&h| h != g && groups[h].outline.as_ref().is_some_and(|o| inside(o, probe[g]))).count())
            .collect();
        let mut out = Vec::new();
        for (g, group) in groups.iter().enumerate().filter(|(g, _)| depth[*g] % 2 == 0) {
            for cell in &group.cells {
                let holes: Vec<&Walked> = (0..groups.len())
                    .filter(|&h| depth[h] == depth[g] + 1 && inside(cell, probe[h]))
                    .filter_map(|h| groups[h].outline.as_ref())
                    .collect();
                let rim = distinct(cell.entities.clone());
                let mut entities = rim.clone();
                for h in &holes {
                    for id in distinct(h.entities.clone()) {
                        if !entities.contains(&id) {
                            entities.push(id);
                        }
                    }
                }
                out.push(Region { outer: cell.curves.clone(), holes: holes.iter().map(|h| h.curves.clone()).collect(), rim: rim.len(), entities });
            }
        }
        Ok(out)
    }

    /// Refused where two kept curves meet anywhere but at a place both of them end.
    fn check_crossings(&self, keep: &[usize]) -> Result<()> {
        let tol = Tolerance::new(CROSSING_MM);
        let mut all: Vec<(Id, Curve)> = Vec::new();
        for &i in keep {
            all.extend(self.pieces[i].curves.iter().map(|c| (self.pieces[i].entity, c.clone())));
        }
        for (curves, id) in &self.rings {
            all.extend(curves.iter().cloned().flat_map(halves).map(|c| (*id, c)));
        }
        let boxes: Vec<Bounds> = all.iter().map(|(_, c)| Bounds::of(c)).collect();
        let ends = |c: &Curve, p: [f64; 2]| distance(c.point_at(0.0), p) <= JOINT_MM || distance(c.point_at(1.0), p) <= JOINT_MM;
        for a in 0..all.len() {
            for b in a + 1..all.len() {
                if !boxes[a].meets(&boxes[b]) {
                    continue;
                }
                for x in geom2d::intersect(&all[a].1, &all[b].1, tol) {
                    if ends(&all[a].1, x.point) && ends(&all[b].1, x.point) {
                        continue;
                    }
                    let [px, py] = x.point;
                    if all[a].0 == all[b].0 {
                        bail!("Sketch loop through #{} crosses itself at ({px:.4}, {py:.4}); split it there and trim", all[a].0);
                    }
                    bail!(
                        "Sketch loops through #{} and #{} meet at ({px:.4}, {py:.4}); loops may nest but not touch. Split and trim them, or mark one Construction",
                        all[a].0,
                        all[b].0
                    );
                }
            }
        }
        Ok(())
    }
}

/// `Ok` when the sketch draws anything at all.
fn ensure_any(any: bool) -> Result<()> {
    if any { Ok(()) } else { bail!("Sketch has no profile geometry") }
}

/// A closed walk: its curves in walking order, whether each runs its own way, and the entities behind them.
#[derive(Clone, Debug)]
struct Walked {
    curves: Vec<Curve>,
    forward: Vec<bool>,
    entities: Vec<Id>,
}

/// The half-edges of `walk`, `2i` piece `i` forward and `2i + 1` backward, as one closed walk.
fn walked(pieces: &[Piece], walk: &[usize]) -> Walked {
    let mut w = Walked { curves: Vec::new(), forward: Vec::new(), entities: Vec::new() };
    for &he in walk {
        let p = &pieces[he / 2];
        let forward = he % 2 == 0;
        let n = p.curves.len();
        if forward {
            w.curves.extend(p.curves.iter().cloned());
        } else {
            w.curves.extend(p.curves.iter().rev().cloned());
        }
        w.forward.extend(std::iter::repeat_n(forward, n));
        w.entities.extend(std::iter::repeat_n(p.entity, n));
    }
    w
}

/// Every face walk of the `keep` pieces: each half-edge's successor is the one leaving its head
/// next clockwise from its twin, so a bounded face comes back counter-clockwise.
fn trace(pieces: &[Piece], keep: &[usize]) -> Vec<Vec<usize>> {
    let tail = |he: usize| pieces[he / 2].ends[he % 2];
    // The curve a half-edge leaves its tail along, the place it leaves and its length there.
    let leaving = |he: usize| -> (&Curve, bool) {
        let p = &pieces[he / 2];
        if he % 2 == 0 { (&p.curves[0], true) } else { (&p.curves[p.curves.len() - 1], false) }
    };
    let mut out: HashMap<Key, Vec<usize>> = HashMap::new();
    for &i in keep {
        for he in [2 * i, 2 * i + 1] {
            out.entry(tail(he)).or_default().push(he);
        }
    }
    // Leaving directions read a hair along each curve, the same hair for every curve at a vertex.
    let mut rank: HashMap<usize, usize> = HashMap::new();
    for list in out.values_mut() {
        let reach = list.iter().map(|he| leaving(*he).0.length()).fold(f64::INFINITY, f64::min);
        let hair = (reach * 1e-4).max(1e-12);
        let angle = |he: usize| {
            let (c, forward) = leaving(he);
            let t = (hair / c.length().max(1e-300)).min(0.5);
            let (from, to) = if forward { (c.point_at(0.0), c.point_at(t)) } else { (c.point_at(1.0), c.point_at(1.0 - t)) };
            (to[1] - from[1]).atan2(to[0] - from[0])
        };
        let mut keyed: Vec<(f64, usize)> = list.iter().map(|he| (angle(*he), *he)).collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        *list = keyed.into_iter().map(|(_, he)| he).collect();
        for (k, he) in list.iter().enumerate() {
            rank.insert(*he, k);
        }
    }
    let next = |he: usize| {
        let twin = he ^ 1;
        let list = &out[&tail(twin)];
        list[(rank[&twin] + list.len() - 1) % list.len()]
    };
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut walks = Vec::new();
    for &i in keep {
        for start in [2 * i, 2 * i + 1] {
            if seen.contains(&start) {
                continue;
            }
            let mut walk = Vec::new();
            let mut he = start;
            while seen.insert(he) {
                walk.push(he);
                he = next(he);
            }
            walks.push(walk);
        }
    }
    walks
}

/// Entity `d` cut at `joints`, sorted along it: a closed entity nothing ends on is a ring, anything else pieces from end or joint to the next.
fn cut(d: &Drawn, joints: &[Joint], rings: &mut Vec<(Vec<Curve>, Id)>, pieces: &mut Vec<Piece>) -> Result<()> {
    let place = |s: usize| Joint { u: s as f64, xy: d.stations[s].0, key: key(d.stations[s].0), name: d.stations[s].1 };
    if joints.is_empty() {
        if d.closed {
            rings.push((d.curves.clone(), d.id));
        } else {
            let (a, b) = (place(0), place(d.stations.len() - 1));
            pieces.push(Piece { entity: d.id, curves: d.curves.clone(), ends: [a.key, b.key], names: [a.name, b.name] });
        }
        return Ok(());
    }
    let m = d.curves.len() as f64;
    let mut places: Vec<Joint> = Vec::with_capacity(joints.len() + 2);
    if !d.closed {
        places.push(place(0));
    }
    places.extend_from_slice(joints);
    if !d.closed {
        places.push(place(d.stations.len() - 1));
    }
    let spans = if d.closed { places.len() } else { places.len() - 1 };
    for k in 0..spans {
        let (from, to) = (places[k], places[(k + 1) % places.len()]);
        let u1 = if d.closed && to.u <= from.u { to.u + m } else { to.u };
        let curves = span(d, from.u, u1, from.xy, to.xy)?;
        if curves.is_empty() {
            continue;
        }
        pieces.push(Piece { entity: d.id, curves, ends: [from.key, to.key], names: [from.name, to.name] });
    }
    Ok(())
}

/// The curves of `d` between positions `u0` and `u1` along it, wrapping round a closed entity, with a
/// straight end moved exactly onto the place it joins.
fn span(d: &Drawn, u0: f64, u1: f64, a: [f64; 2], b: [f64; 2]) -> Result<Vec<Curve>> {
    let m = d.curves.len();
    let mut out = Vec::new();
    let mut c = u0.floor();
    while c < u1 - 1e-12 {
        let (t0, t1) = ((u0 - c).max(0.0), (u1 - c).min(1.0));
        if t1 - t0 > 1e-12 {
            out.extend(part(&d.curves[(c as usize) % m], t0, t1)?);
        }
        c += 1.0;
    }
    if let Some(Curve::Line(l)) = out.first_mut() {
        l.start = a;
    }
    if let Some(Curve::Line(l)) = out.last_mut() {
        l.end = b;
    }
    Ok(out)
}

/// The part of `c` between parameters `t0 < t1`: the curve itself when that is all of it, a whole circle as two half arcs.
fn part(c: &Curve, t0: f64, t1: f64) -> Result<Vec<Curve>> {
    if t0 <= 0.0 && t1 >= 1.0 {
        return Ok(halves(c.clone()));
    }
    Ok(vec![match c {
        Curve::Line(l) => Curve::Line(Line {
            start: if t0 <= 0.0 { l.start } else { c.point_at(t0) },
            end: if t1 >= 1.0 { l.end } else { c.point_at(t1) },
        }),
        Curve::Arc(arc) => {
            let from = arc.start_angle.rem_euclid(TAU);
            let sweep = arc.sweep();
            Curve::Arc(Arc { centre: arc.centre, radius: arc.radius, start_angle: from + t0 * sweep, end_angle: from + t1 * sweep })
        }
        Curve::Circle(k) => Curve::Arc(Arc { centre: k.centre, radius: k.radius, start_angle: t0 * TAU, end_angle: t1 * TAU }),
        Curve::Nurbs(n) => Curve::Nurbs(n.trimmed(t0, t1).context("A spline would not cut where another curve ends on it")?),
        other => bail!("A {other:?} cannot be cut where another curve ends on it"),
    }])
}

#[cfg(test)]
mod tests {
    use crate::cad::{Component, Document, Feature, FeatureStatus, Operation, Profile, evaluate};
    use crate::sketch::{Geometry, Id, RegionRef, Sketch, distance};
    use crate::{AlphaLibrary, BuildParams, RingDesign};
    use std::f64::consts::PI;

    /// Four lines round a 10 × 6 rectangle whose bottom runs 2 mm past the right side and whose right side runs 1 mm below the bottom: the two, crossing at (10, 0).
    fn crossed_corner() -> (Sketch, Id, Id) {
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [12.0, 0.0], [10.0, -1.0], [10.0, 6.0], [0.0, 6.0]].map(|xy| s.point(xy));
        let bottom = s.add_line(p[0], p[1], false).unwrap();
        let right = s.add_line(p[2], p[3], false).unwrap();
        s.add_line(p[3], p[4], false).unwrap();
        s.add_line(p[4], p[0], false).unwrap();
        (s, bottom, right)
    }

    /// The volume an extrusion of `from` in `sketch`, held by feature #1, builds; or why it fails.
    fn extruded(sketch: &Sketch, from: Profile, height_mm: f64) -> Result<f64, String> {
        let feature = |id, operation| Feature { id, name: format!("#{id}"), enabled: true, operation, component: Component::default() };
        let mut doc = Document::default();
        doc.append(feature(1, Operation::Sketch { sketch: sketch.clone() })).unwrap();
        doc.append(feature(2, Operation::Extrude { sketch: from, height_mm, draft_deg: 0.0 })).unwrap();
        let e = evaluate(&RingDesign { cad: Some(doc), ..RingDesign::default() }, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        if let FeatureStatus::Failed(why) = &e.features[1].status {
            return Err(why.clone());
        }
        let m = &e.components[0].mesh;
        assert!(m.validate().watertight);
        Ok(m.volume_mm3())
    }

    /// The areas of the regions a pick chooses among, smallest first, to a nanometre squared.
    fn areas(s: &Sketch) -> Vec<f64> {
        let mut a: Vec<f64> = s.profile_regions().unwrap().iter().map(|r| (r.area() * 1e9).round() / 1e9).collect();
        a.sort_by(f64::total_cmp);
        a
    }

    #[test]
    fn a_trimmed_overhang_closes_where_its_end_lands_on_the_side_it_crossed() {
        let (mut s, bottom, right) = crossed_corner();
        let open = s.profile_curves().unwrap_err().to_string();
        assert!(open.contains("open at point"), "{open}");
        // Trimmed back to the right side, the bottom ends partway along that side's line.
        s.trim(bottom, [11.0, 0.0]).unwrap();
        assert_eq!(s.points.iter().filter(|p| distance(p.xy, [10.0, 0.0]) < 1e-9).count(), 1, "one new point where it stops");
        assert_eq!(s.profile_curves().unwrap().len(), 4, "the right side joins from there up; its overhang below is left out");
        assert_eq!(areas(&s), [60.0]);
        assert_eq!(s.loop_through(bottom).unwrap().len(), 4);
        assert!(s.loop_through(right).unwrap().contains(&bottom));
        // The whole sketch extrudes to its area times the height, off its plane either way.
        for h in [1.5, -1.5] {
            let v = extruded(&s, Profile::Feature { feature: 1 }, h).unwrap();
            assert!((v - 90.0).abs() < 1e-6, "{h}: {v}");
        }
        // The other overhang trimmed too: the same loop.
        s.trim(right, [10.0, -0.5]).unwrap();
        assert_eq!(areas(&s), [60.0]);
    }

    #[test]
    fn a_line_across_a_rectangle_makes_two_regions_and_sweeping_both_at_once_names_where_they_branch() {
        let mut s = Sketch::default();
        let sides = s.add_rectangle([0.0, 0.0], [10.0, 6.0], false).unwrap();
        let p = [[4.0, 0.0], [4.0, 6.0]].map(|xy| s.point(xy));
        let wall = s.add_line(p[0], p[1], false).unwrap();
        assert_eq!(areas(&s), [24.0, 36.0], "the two parts of the rectangle's 60 mm²");
        let regions = s.profile_regions().unwrap();
        assert!(regions.iter().all(|r| r.holes.is_empty() && r.entities[..r.rim].contains(&wall)));
        for why in [s.sweep_regions().unwrap_err().to_string(), s.profile_curves().unwrap_err().to_string()] {
            assert!(why.contains(&format!("point #{} joins 3 curves", p[0])), "{why}");
        }
        // Each region is named by a side only it runs along, and the right one extrudes alone.
        let right = regions.iter().position(|r| r.contains([7.0, 3.0])).unwrap();
        let pick = RegionRef::among(&regions, right, [7.0, 3.0]).unwrap();
        assert!(sides.contains(&pick.entity), "{pick:?}");
        assert_eq!(pick.position(&regions), Some((right, false)));
        let left = RegionRef::among(&regions, 1 - right, [2.0, 3.0]).unwrap();
        assert!(sides.contains(&left.entity) && left.entity != pick.entity);
        assert_eq!(left.position(&regions), Some((1 - right, false)));
        assert_eq!(RegionRef { entity: wall, at: [2.0, 3.0] }.position(&regions), Some((1 - right, false)), "a name on the shared wall goes by its point");
        let alone = extruded(&s, Profile::Region { feature: 1, region: pick }, 1.0).unwrap();
        assert!((alone - 36.0).abs() < 1e-6, "{alone}");
        let whole = extruded(&s, Profile::Feature { feature: 1 }, 1.0).unwrap_err();
        assert!(whole.contains("joins 3 curves"), "{whole}");
        // The same across one closed polyline.
        let mut s = Sketch::default();
        let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]].map(|xy| s.point(xy));
        s.draw(Geometry::Polyline { points: corners.to_vec(), closed: true }, false);
        let p = [[4.0, 0.0], [4.0, 6.0]].map(|xy| s.point(xy));
        s.add_line(p[0], p[1], false).unwrap();
        assert_eq!(areas(&s), [24.0, 36.0]);
        // Two triangles meeting at a corner branch there too, and each is a region.
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [2.0, 1.0], [2.0, -1.0], [-2.0, 1.0], [-2.0, -1.0]].map(|xy| s.point(xy));
        for (a, b) in [(0, 1), (1, 2), (2, 0), (0, 3), (3, 4), (4, 0)] {
            s.add_line(p[a], p[b], false).unwrap();
        }
        assert_eq!(areas(&s), [2.0, 2.0]);
        assert!(s.sweep_regions().unwrap_err().to_string().contains(&format!("point #{} joins 4 curves", p[0])));
    }

    #[test]
    fn a_real_gap_still_names_its_point_and_so_does_a_line_left_hanging_off_a_loop() {
        // A rectangle whose last side stops half a millimetre short of its first corner.
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0], [0.0, 0.5]].map(|xy| s.point(xy));
        for k in 0..4 {
            s.add_line(p[k], p[k + 1], false).unwrap();
        }
        let why = s.profile_regions().unwrap_err().to_string();
        assert!(why.contains(&format!("open at point #{}", p[4])), "{why}");
        assert_eq!(why, s.profile_curves().unwrap_err().to_string());
        // Closed, then a line drawn from nowhere down onto the middle of its top: the end in the air is named.
        s.add_line(p[4], p[0], false).unwrap();
        assert_eq!(areas(&s), [60.0]);
        let q = [[5.0, 9.0], [5.0, 6.0]].map(|xy| s.point(xy));
        s.add_line(q[0], q[1], false).unwrap();
        let why = s.profile_regions().unwrap_err().to_string();
        assert!(why.contains(&format!("open at point #{}", q[0])), "{why}");
        // A triangle with a line off one corner: the line's free end, not the corner it leaves.
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 0.0], [2.0, 3.0], [2.0, -3.0]].map(|xy| s.point(xy));
        for (a, b) in [(0, 1), (1, 2), (2, 0), (0, 3)] {
            s.add_line(p[a], p[b], false).unwrap();
        }
        let why = s.profile_regions().unwrap_err().to_string();
        assert!(why.contains(&format!("open at point #{}", p[3])), "{why}");
        // Two lines crossing a rectangle's corner, neither trimmed: both overhangs are real free ends.
        let (s, ..) = crossed_corner();
        assert!(s.profile_regions().unwrap_err().to_string().contains("open at point"));
    }

    #[test]
    fn a_diameter_ending_on_its_circle_halves_it_and_a_divided_rectangle_inside_a_circle_is_its_hole() {
        let mut s = Sketch::default();
        s.add_circle([0.0, 0.0], [3.0, 0.0], false).unwrap();
        let p = [[-3.0, 0.0], [3.0, 0.0]].map(|xy| s.point(xy));
        s.add_line(p[0], p[1], false).unwrap();
        let half = PI * 9.0 / 2.0;
        let a = areas(&s);
        assert!(a.len() == 2 && a.iter().all(|x| (x - half).abs() < 1e-9), "{a:?}");
        let v = extruded(&s, Profile::Region { feature: 1, region: RegionRef::of(&s.profile_regions().unwrap()[0]).unwrap() }, 2.0).unwrap();
        assert!((v / (half * 2.0) - 1.0).abs() < 0.005, "{v}");
        // A rectangle divided by a wall, inside a circle: one region, the circle less the rectangle.
        let mut s = Sketch::default();
        s.add_circle([5.0, 3.0], [25.0, 3.0], false).unwrap();
        s.add_rectangle([0.0, 0.0], [10.0, 6.0], false).unwrap();
        let p = [[4.0, 0.0], [4.0, 6.0]].map(|xy| s.point(xy));
        s.add_line(p[0], p[1], false).unwrap();
        let r = s.profile_regions().unwrap();
        assert_eq!((r.len(), r[0].holes.len()), (1, 1));
        assert!((r[0].area() - (PI * 400.0 - 60.0)).abs() < 1e-6, "{}", r[0].area());
        // And a circle inside one part of the divided rectangle is a hole of that part alone.
        let mut s = Sketch::default();
        s.add_rectangle([0.0, 0.0], [10.0, 6.0], false).unwrap();
        let p = [[4.0, 0.0], [4.0, 6.0]].map(|xy| s.point(xy));
        s.add_line(p[0], p[1], false).unwrap();
        s.add_circle([7.0, 3.0], [8.0, 3.0], false).unwrap();
        let a = areas(&s);
        assert!((a[0] - 24.0).abs() < 1e-9 && (a[1] - (36.0 - PI)).abs() < 1e-9, "{a:?}");
    }
}
