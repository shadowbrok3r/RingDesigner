//! Booleans between closed triangle meshes, pure Rust, for every target.
//!
//! Built for one job: a small pre-made solid — a setting head, a seat bur —
//! against the ring. Every decision about *whether* an edge crosses a face is
//! an exact predicate, so the two meshes agree on the topology of their
//! intersection; every intersection point is one vertex shared by both sides,
//! keyed by the edge and face that make it, so the result is closed by
//! construction rather than by welding. Coordinates are only ever used to
//! place a vertex, never to decide one. A configuration the predicates call
//! degenerate is retried with the tool nudged by a tenth of a micron.
use robust::{Coord3D, orient3d};
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
use std::collections::{HashMap, HashSet, VecDeque};

pub type P3 = [f64; 3];

/// A closed, outward-wound triangle mesh in f64.
#[derive(Clone, Debug, Default)]
pub struct Solid {
    pub v: Vec<P3>,
    pub f: Vec<[u32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Union,
    Subtract,
    Intersect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Snag {
    /// The predicates found a coincidence; a nudge resolves it.
    Degenerate(&'static str),
    /// Nothing is left.
    Empty,
}

impl std::fmt::Display for Snag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Degenerate(why) => write!(f, "the solids meet degenerately ({why})"),
            Self::Empty => write!(f, "nothing is left of the solid"),
        }
    }
}

fn sub(a: P3, b: P3) -> P3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: P3, b: P3) -> P3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: P3, s: f64) -> P3 { [a[0] * s, a[1] * s, a[2] * s] }
fn dot(a: P3, b: P3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: P3, b: P3) -> P3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(a: P3) -> P3 {
    let l = dot(a, a).sqrt();
    if l > 1e-300 { scale(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}
fn c3(p: P3) -> Coord3D<f64> { Coord3D { x: p[0], y: p[1], z: p[2] } }
/// Positive with `d` on the inner side of the outward-wound `a b c`.
fn side(a: P3, b: P3, c: P3, d: P3) -> f64 { orient3d(c3(a), c3(b), c3(c), c3(d)) }

/// A rigid placement: columns are where the local axes land.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: P3,
    pub x: P3,
    pub y: P3,
    pub z: P3,
}

impl Frame {
    pub const IDENTITY: Frame = Frame { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], z: [0.0, 0.0, 1.0] };
    /// `z` along `normal`, `x` as near `along` as a right angle allows.
    pub fn from_normal(origin: P3, normal: P3, along: P3) -> Self {
        let z = unit(normal);
        let mut x = sub(along, scale(z, dot(along, z)));
        if dot(x, x) < 1e-12 {
            x = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
            x = sub(x, scale(z, dot(x, z)));
        }
        let x = unit(x);
        Self { origin, x, y: cross(z, x), z }
    }
    pub fn point(&self, p: P3) -> P3 {
        add(self.origin, add(scale(self.x, p[0]), add(scale(self.y, p[1]), scale(self.z, p[2]))))
    }
}

impl Solid {
    pub fn is_empty(&self) -> bool { self.f.is_empty() }

    pub fn placed(&self, frame: &Frame) -> Solid {
        Solid { v: self.v.iter().map(|p| frame.point(*p)).collect(), f: self.f.clone() }
    }

    pub fn translated(&self, by: P3) -> Solid {
        Solid { v: self.v.iter().map(|p| add(*p, by)).collect(), f: self.f.clone() }
    }

    /// Append another shell without resolving anything.
    pub fn push(&mut self, other: &Solid) {
        let base = self.v.len() as u32;
        self.v.extend_from_slice(&other.v);
        self.f.extend(other.f.iter().map(|f| f.map(|i| i + base)));
    }

    pub fn volume(&self) -> f64 {
        self.f.iter().map(|f| {
            let [a, b, c] = f.map(|i| self.v[i as usize]);
            dot(a, cross(b, c)) / 6.0
        }).sum()
    }

    pub fn bounds(&self) -> Option<(P3, P3)> {
        let mut it = self.v.iter();
        let first = *it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| {
            (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k])))
        }))
    }

    /// Directed edges with no twin, and edges used more than once: both zero on a closed manifold.
    pub fn open_edges(&self) -> (usize, usize) {
        let mut seen: HashMap<(u32, u32), u32> = HashMap::with_capacity(self.f.len() * 3);
        for f in &self.f {
            for k in 0..3 {
                *seen.entry((f[k], f[(k + 1) % 3])).or_default() += 1;
            }
        }
        let open = seen.keys().filter(|(a, b)| !seen.contains_key(&(*b, *a))).count();
        let repeated = seen.values().filter(|n| **n > 1).count();
        (open, repeated)
    }

    /// Drop vertices no face uses.
    pub fn compact(&mut self) -> Vec<u32> {
        let mut map = vec![u32::MAX; self.v.len()];
        for f in &self.f {
            for i in f { map[*i as usize] = 0; }
        }
        // In order, so whatever came first stays first.
        let mut kept = Vec::new();
        for (i, m) in map.iter_mut().enumerate() {
            if *m == 0 {
                *m = kept.len() as u32;
                kept.push(self.v[i]);
            }
        }
        for f in &mut self.f {
            for i in f.iter_mut() { *i = map[*i as usize]; }
        }
        self.v = kept;
        map
    }

    fn jittered(&self, attempt: u32) -> Solid {
        let Some((lo, hi)) = self.bounds() else { return self.clone() };
        let c = scale(add(lo, hi), 0.5);
        let k = attempt as f64;
        let axis = unit([0.31, 0.53, 0.79]);
        let angle = 3.1e-7 * k;
        let shift = [1.3e-7 * k, -0.7e-7 * k, 0.9e-7 * k];
        let (sin, cos) = angle.sin_cos();
        Solid {
            v: self.v.iter().map(|p| {
                let r = sub(*p, c);
                let turned = add(add(scale(r, cos), scale(cross(axis, r), sin)), scale(axis, dot(axis, r) * (1.0 - cos)));
                add(add(c, turned), shift)
            }).collect(),
            f: self.f.clone(),
        }
    }
}

/// `a` with `b` joined, cut away, or kept in common. `b` is the small one.
pub fn combine(a: &Solid, b: &Solid, op: Op) -> Result<Solid, Snag> {
    let mut last = Snag::Degenerate("untried");
    for attempt in 0..8 {
        let tool;
        let b = if attempt == 0 { b } else { tool = b.jittered(attempt); &tool };
        match boolean(a, b, op) {
            Ok(s) => return Ok(s),
            Err(Snag::Empty) => return Err(Snag::Empty),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Every part joined into one solid; parts that touch nothing ride along as their own shells.
pub fn union_all(parts: &[Solid]) -> Result<Solid, Snag> {
    let mut it = parts.iter().filter(|p| !p.is_empty());
    let Some(first) = it.next() else { return Err(Snag::Empty) };
    let mut out = first.clone();
    for p in it {
        out = combine(&out, p, Op::Union)?;
    }
    Ok(out)
}

/// Collapses every edge shorter than `eps` and flips away every face thinner than it. Exact topology keeps
/// a crossing that lands a hair from a vertex as a vertex of its own, and in f32 the face between the two
/// is a line or a point. Returns how many faces went; the solid is left as it was if the result would
/// not close.
pub fn clean(s: &mut Solid, eps: f64) -> usize {
    let before = s.open_edges();
    let mut faces = s.f.clone();
    let mut alive = vec![true; faces.len()];
    let mut around: Vec<Vec<u32>> = vec![Vec::new(); s.v.len()];
    for (i, f) in faces.iter().enumerate() {
        for &v in f {
            around[v as usize].push(i as u32);
        }
    }
    let mut removed = 0;
    for _ in 0..6 {
        let mut changed = false;
        for fi in 0..faces.len() {
            if !alive[fi] {
                continue;
            }
            let f = faces[fi];
            let p = f.map(|k| s.v[k as usize]);
            let len2: [f64; 3] = std::array::from_fn(|k| { let d = sub(p[(k + 1) % 3], p[k]); dot(d, d) });
            let short = (0..3).min_by(|a, b| len2[*a].total_cmp(&len2[*b])).unwrap_or(0);
            if len2[short] < eps * eps {
                if collapse(&mut faces, &mut alive, &mut around, &s.v, f[short], f[(short + 1) % 3], eps) {
                    removed += 2;
                    changed = true;
                }
                continue;
            }
            let long = (0..3).max_by(|a, b| len2[*a].total_cmp(&len2[*b])).unwrap_or(0);
            let twice_area = dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), cross(sub(p[1], p[0]), sub(p[2], p[0]))).sqrt();
            if twice_area >= eps * len2[long].sqrt() {
                continue;
            }
            if flip(&mut faces, &mut alive, &mut around, &s.v, fi, long, eps) {
                changed = true;
                continue;
            }
            // Too near a corner for the neighbour's halves to have any width: merge the corner in instead.
            let near = if len2[(long + 1) % 3] < len2[(long + 2) % 3] { (long + 1) % 3 } else { (long + 2) % 3 };
            if len2[near] < 100.0 * eps * eps && collapse(&mut faces, &mut alive, &mut around, &s.v, f[near], f[(near + 1) % 3], eps) {
                removed += 2;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if removed == 0 && faces == s.f {
        return 0;
    }
    let kept: Vec<[u32; 3]> = faces.into_iter().zip(alive).filter_map(|(f, a)| a.then_some(f)).collect();
    let old = std::mem::replace(&mut s.f, kept);
    let after = s.open_edges();
    if after.0 > before.0 || after.1 > before.1 {
        s.f = old;
        return 0;
    }
    removed
}

fn twice_area_of(v: &[P3], f: [u32; 3]) -> P3 {
    let [a, b, c] = f.map(|k| v[k as usize]);
    cross(sub(b, a), sub(c, a))
}

/// Merge the far end of edge `a b` into the lower-numbered one, dropping the two faces on the edge.
/// Refused where the ends share a neighbour off the edge, which would pinch the surface, or where a
/// face round the moved end would turn over.
fn collapse(faces: &mut [[u32; 3]], alive: &mut [bool], around: &mut [Vec<u32>], v: &[P3], a: u32, b: u32, eps: f64) -> bool {
    let (keep, gone) = (a.min(b), a.max(b));
    let live = |x: u32| -> Vec<u32> { around[x as usize].iter().copied().filter(|g| alive[*g as usize]).collect() };
    let (at_keep, at_gone) = (live(keep), live(gone));
    let shared: Vec<u32> = at_gone.iter().copied().filter(|g| faces[*g as usize].contains(&keep)).collect();
    if shared.len() != 2 {
        return false;
    }
    let ring = |fs: &[u32]| -> HashSet<u32> { fs.iter().flat_map(|g| faces[*g as usize]).filter(|x| *x != keep && *x != gone).collect() };
    let apexes: HashSet<u32> = ring(&shared);
    if apexes.len() != 2 || ring(&at_keep).intersection(&ring(&at_gone)).any(|x| !apexes.contains(x)) {
        return false;
    }
    for g in at_gone.iter().filter(|g| !shared.contains(g)) {
        let f = faces[*g as usize];
        let old = twice_area_of(v, f);
        let [p, q, r] = f.map(|k| v[k as usize]);
        let longest = dot(sub(q, p), sub(q, p)).max(dot(sub(r, q), sub(r, q))).max(dot(sub(p, r), sub(p, r))).sqrt();
        // A sliver has no side to keep.
        if dot(old, old).sqrt() < eps * longest {
            continue;
        }
        if dot(old, twice_area_of(v, f.map(|k| if k == gone { keep } else { k }))) <= 0.0 {
            return false;
        }
    }
    for g in &shared {
        alive[*g as usize] = false;
    }
    for g in at_gone.iter().filter(|g| !shared.contains(g)) {
        for k in faces[*g as usize].iter_mut() {
            if *k == gone {
                *k = keep;
            }
        }
        around[keep as usize].push(*g);
    }
    around[gone as usize].clear();
    true
}

/// Turn the long edge of a face whose third corner lies on it: the corner then splits the neighbour
/// across that edge instead of standing on it as a face of no width.
fn flip(faces: &mut [[u32; 3]], alive: &mut [bool], around: &mut [Vec<u32>], v: &[P3], fi: usize, long: usize, eps: f64) -> bool {
    let f = faces[fi];
    let (b, c, a) = (f[long], f[(long + 1) % 3], f[(long + 2) % 3]);
    let Some(&g) = around[c as usize].iter().find(|g| {
        let h = faces[**g as usize];
        alive[**g as usize] && **g as usize != fi && (0..3).any(|k| h[k] == c && h[(k + 1) % 3] == b)
    }) else {
        return false;
    };
    let Some(&d) = faces[g as usize].iter().find(|x| **x != b && **x != c) else { return false };
    if d == a || around[a as usize].iter().any(|h| alive[*h as usize] && faces[*h as usize].contains(&d)) {
        return false;
    }
    let before = twice_area_of(v, faces[g as usize]);
    let (one, two) = ([a, b, d], [a, d, c]);
    let (n1, n2) = (twice_area_of(v, one), twice_area_of(v, two));
    let edge = |x: u32, y: u32| { let e = sub(v[y as usize], v[x as usize]); dot(e, e).sqrt() };
    // Both halves must face as the neighbour did and have a width of their own.
    if dot(n1, before) <= 0.0 || dot(n2, before) <= 0.0 || dot(n1, n1).sqrt() < eps * edge(b, d) || dot(n2, n2).sqrt() < eps * edge(d, c) {
        return false;
    }
    faces[fi] = one;
    faces[g as usize] = two;
    around[c as usize].retain(|h| *h as usize != fi);
    around[d as usize].push(fi as u32);
    around[b as usize].retain(|h| *h != g);
    around[a as usize].push(g);
    true
}

type Edge = (u32, u32);
fn key(a: u32, b: u32) -> Edge { if a < b { (a, b) } else { (b, a) } }

/// Where an edge of one mesh passes through a face of the other.
#[derive(Clone, Copy)]
struct Crossing {
    t: f64,
    vertex: u32,
    face: u32,
}

/// Edge `p q` against the outward-wound face `a b c`: the parameter along the edge, exactly decided.
fn pierce(p: P3, q: P3, a: P3, b: P3, c: P3) -> Result<Option<f64>, Snag> {
    let sp = side(a, b, c, p);
    let sq = side(a, b, c, q);
    if sp == 0.0 || sq == 0.0 {
        return Err(Snag::Degenerate("a vertex lies in a face's plane"));
    }
    if (sp > 0.0) == (sq > 0.0) {
        return Ok(None);
    }
    let s = [side(p, q, a, b), side(p, q, b, c), side(p, q, c, a)];
    let pos = s.iter().filter(|v| **v > 0.0).count();
    let neg = s.iter().filter(|v| **v < 0.0).count();
    if pos > 0 && neg > 0 {
        return Ok(None);
    }
    if pos + neg < 3 {
        return Err(Snag::Degenerate("an edge passes through a face's border"));
    }
    Ok(Some(sp / (sp - sq)))
}

struct Grid {
    lo: P3,
    width: P3,
    dims: [usize; 3],
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn over(s: &Solid, faces: &[u32]) -> Self {
        let (lo, hi) = s.bounds().unwrap_or(([0.0; 3], [1.0; 3]));
        let mut mean = 0.0;
        for &f in faces {
            let [a, b, c] = s.f[f as usize].map(|i| s.v[i as usize]);
            mean += dot(sub(b, a), sub(b, a)).sqrt() + dot(sub(c, b), sub(c, b)).sqrt();
        }
        let cell = (mean / (2.0 * faces.len().max(1) as f64) * 2.0).max(1e-4);
        let dims: [usize; 3] = std::array::from_fn(|k| (((hi[k] - lo[k]) / cell).ceil() as usize).clamp(1, 96));
        let width = std::array::from_fn(|k| ((hi[k] - lo[k]) / dims[k] as f64).max(1e-9));
        let mut grid = Self { lo, width, dims, cells: vec![Vec::new(); dims[0] * dims[1] * dims[2]] };
        for &f in faces {
            let (flo, fhi) = tri_bounds(s.f[f as usize].map(|i| s.v[i as usize]));
            grid.each_cell(flo, fhi, |cells, i| cells[i].push(f));
        }
        grid
    }
    fn each_cell(&mut self, lo: P3, hi: P3, mut visit: impl FnMut(&mut Vec<Vec<u32>>, usize)) {
        let at = |k: usize, x: f64| (((x - self.lo[k]) / self.width[k]).floor().max(0.0) as usize).min(self.dims[k] - 1);
        let r: [(usize, usize); 3] = std::array::from_fn(|k| (at(k, lo[k]), at(k, hi[k])));
        for x in r[0].0..=r[0].1 {
            for y in r[1].0..=r[1].1 {
                for z in r[2].0..=r[2].1 {
                    let i = (x * self.dims[1] + y) * self.dims[2] + z;
                    visit(&mut self.cells, i);
                }
            }
        }
    }
}

fn tri_bounds(t: [P3; 3]) -> (P3, P3) {
    (std::array::from_fn(|k| t[0][k].min(t[1][k]).min(t[2][k])), std::array::from_fn(|k| t[0][k].max(t[1][k]).max(t[2][k])))
}
fn overlap(a: &(P3, P3), b: &(P3, P3)) -> bool {
    (0..3).all(|k| a.0[k] <= b.1[k] && b.0[k] <= a.1[k])
}

/// One side's faces after the cut, labelled by whether they sit inside the other solid.
struct Side {
    faces: Vec<[u32; 3]>,
    inside: Vec<bool>,
}

fn boolean(a: &Solid, b: &Solid, op: Op) -> Result<Solid, Snag> {
    let na = a.v.len() as u32;
    let nb = b.v.len() as u32;
    let Some(bb) = b.bounds() else { return if op == Op::Intersect { Err(Snag::Empty) } else { Ok(a.clone()) } };
    let pad = 1e-6;
    let bb = (bb.0.map(|v| v - pad), bb.1.map(|v| v + pad));
    let local: Vec<u32> = (0..a.f.len() as u32).filter(|&i| overlap(&tri_bounds(a.f[i as usize].map(|v| a.v[v as usize])), &bb)).collect();
    let all_b: Vec<u32> = (0..b.f.len() as u32).collect();
    let mut grid = Grid::over(b, &all_b);

    // Output numbering: A's vertices, then B's, then the crossings.
    let mut points: Vec<P3> = Vec::new();
    let pos = |i: u32, points: &Vec<P3>| -> P3 {
        if i < na { a.v[i as usize] } else if i < na + nb { b.v[(i - na) as usize] } else { points[(i - na - nb) as usize] }
    };
    let mut tested_a: HashMap<(Edge, u32), Option<u32>> = HashMap::new();
    let mut tested_b: HashMap<(Edge, u32), Option<u32>> = HashMap::new();
    let mut chain_a: HashMap<Edge, Vec<Crossing>> = HashMap::new();
    let mut chain_b: HashMap<Edge, Vec<Crossing>> = HashMap::new();
    let mut cuts_a: HashMap<u32, Vec<Edge>> = HashMap::new();
    let mut cuts_b: HashMap<u32, Vec<Edge>> = HashMap::new();
    let mut stamp = vec![u32::MAX; b.f.len()];

    for &fa in &local {
        let ta = a.f[fa as usize];
        let pa = ta.map(|i| a.v[i as usize]);
        let box_a = tri_bounds(pa);
        let mut near = Vec::new();
        grid.each_cell(box_a.0, box_a.1, |cells, i| {
            for &fb in &cells[i] {
                if stamp[fb as usize] != fa {
                    stamp[fb as usize] = fa;
                    near.push(fb);
                }
            }
        });
        for fb in near {
            let tb = b.f[fb as usize];
            let pb = tb.map(|i| b.v[i as usize]);
            if !overlap(&box_a, &tri_bounds(pb)) {
                continue;
            }
            let mut ends: Vec<u32> = Vec::new();
            for k in 0..3 {
                let (i, j) = (ta[k], ta[(k + 1) % 3]);
                let e = key(i, j);
                let hit = match tested_a.get(&(e, fb)) {
                    Some(h) => *h,
                    None => {
                        let (p, q) = (a.v[e.0 as usize], a.v[e.1 as usize]);
                        let h = match pierce(p, q, pb[0], pb[1], pb[2])? {
                            Some(t) => {
                                let id = na + nb + points.len() as u32;
                                points.push(add(p, scale(sub(q, p), t)));
                                chain_a.entry(e).or_default().push(Crossing { t, vertex: id, face: fb });
                                Some(id)
                            }
                            None => None,
                        };
                        tested_a.insert((e, fb), h);
                        h
                    }
                };
                ends.extend(hit);
            }
            for k in 0..3 {
                let (i, j) = (tb[k], tb[(k + 1) % 3]);
                let e = key(i, j);
                let hit = match tested_b.get(&(e, fa)) {
                    Some(h) => *h,
                    None => {
                        let (p, q) = (b.v[e.0 as usize], b.v[e.1 as usize]);
                        let h = match pierce(p, q, pa[0], pa[1], pa[2])? {
                            Some(t) => {
                                let id = na + nb + points.len() as u32;
                                points.push(add(p, scale(sub(q, p), t)));
                                chain_b.entry(e).or_default().push(Crossing { t, vertex: id, face: fa });
                                Some(id)
                            }
                            None => None,
                        };
                        tested_b.insert((e, fa), h);
                        h
                    }
                };
                ends.extend(hit);
            }
            match ends.len() {
                0 => {}
                2 => {
                    let seg = key(ends[0], ends[1]);
                    cuts_a.entry(fa).or_default().push(seg);
                    cuts_b.entry(fb).or_default().push(seg);
                }
                _ => return Err(Snag::Degenerate("two faces meet in other than a segment")),
            }
        }
    }

    if cuts_a.is_empty() {
        return apart(a, b, op);
    }
    for chain in chain_a.values_mut().chain(chain_b.values_mut()) {
        chain.sort_by(|x, y| x.t.total_cmp(&y.t));
        if chain.windows(2).any(|w| w[0].t == w[1].t) {
            return Err(Snag::Degenerate("two crossings coincide on an edge"));
        }
    }
    let constraint: HashSet<Edge> = cuts_a.values().flatten().copied().collect();

    // --- A's side: the local faces, cut ones retriangulated. ---
    let mut side_a = Side { faces: Vec::new(), inside: Vec::new() };
    for &fa in &local {
        let t = a.f[fa as usize];
        match cuts_a.get(&fa) {
            None => side_a.faces.push(t),
            Some(segs) => side_a.faces.extend(split(t, &chain_a, segs, |i| pos(i, &points))?),
        }
    }
    let mut known: HashMap<u32, bool> = HashMap::new();
    for &fa in &local {
        for i in a.f[fa as usize] {
            let p = a.v[i as usize];
            if (0..3).any(|k| p[k] < bb.0[k] || p[k] > bb.1[k]) {
                known.insert(i, false);
            }
        }
    }
    label_ends(&chain_a, |f| b.f[f as usize].map(|i| b.v[i as usize]), |i| a.v[i as usize], 0, &mut known)?;
    let centre = |f: [u32; 3]| scale(add(add(pos(f[0], &points), pos(f[1], &points)), pos(f[2], &points)), 1.0 / 3.0);
    flood(&mut side_a, &known, &constraint, |f| inside(b, centre(f)))?;

    // --- B's side: every face. ---
    let mut side_b = Side { faces: Vec::new(), inside: Vec::new() };
    for fb in 0..b.f.len() as u32 {
        let t = b.f[fb as usize].map(|i| i + na);
        match cuts_b.get(&fb) {
            None => side_b.faces.push(t),
            Some(segs) => {
                let shifted: HashMap<Edge, Vec<Crossing>> = b.f[fb as usize].iter().enumerate().filter_map(|(k, &i)| {
                    let e = key(i, b.f[fb as usize][(k + 1) % 3]);
                    chain_b.get(&e).map(|c| (key(e.0 + na, e.1 + na), c.clone()))
                }).collect();
                side_b.faces.extend(split(t, &shifted, segs, |i| pos(i, &points))?);
            }
        }
    }
    let mut known_b: HashMap<u32, bool> = HashMap::new();
    label_ends(&chain_b, |f| a.f[f as usize].map(|i| a.v[i as usize]), |i| b.v[i as usize], na, &mut known_b)?;
    flood(&mut side_b, &known_b, &constraint, |f| inside(a, centre(f)))?;

    // --- Assemble. ---
    let (keep_a_inside, keep_b_inside, flip_b) = match op {
        Op::Union => (false, false, false),
        Op::Subtract => (false, true, true),
        Op::Intersect => (true, true, false),
    };
    let mut kept: Vec<[u32; 3]> = Vec::new();
    for (f, inside) in side_a.faces.iter().zip(&side_a.inside) {
        if *inside == keep_a_inside { kept.push(*f); }
    }
    for (f, inside) in side_b.faces.iter().zip(&side_b.inside) {
        if *inside == keep_b_inside { kept.push(if flip_b { [f[0], f[2], f[1]] } else { *f }); }
    }

    // The cut region must close on itself and onto the faces left alone.
    let mut is_local = vec![false; a.f.len()];
    for &f in &local { is_local[f as usize] = true; }
    let mut directed: HashSet<(u32, u32)> = HashSet::with_capacity(kept.len() * 3);
    for f in &kept {
        for k in 0..3 {
            if !directed.insert((f[k], f[(k + 1) % 3])) {
                return Err(Snag::Degenerate("an edge is used twice the same way"));
            }
        }
    }
    let mut border: HashSet<(u32, u32)> = HashSet::new();
    if op != Op::Intersect {
        let mut local_directed: HashSet<(u32, u32)> = HashSet::new();
        for &fa in &local {
            let t = a.f[fa as usize];
            for k in 0..3 { local_directed.insert((t[k], t[(k + 1) % 3])); }
        }
        for &(u, v) in &local_directed {
            if !local_directed.contains(&(v, u)) { border.insert((u, v)); }
        }
    }
    for &(u, v) in &directed {
        if !directed.contains(&(v, u)) && !border.contains(&(u, v)) {
            return Err(Snag::Degenerate("the cut does not close"));
        }
    }
    if border.iter().any(|e| !directed.contains(e)) {
        return Err(Snag::Degenerate("the cut reaches past its neighbourhood"));
    }

    let mut out = Solid { v: Vec::with_capacity((na + nb) as usize + points.len()), f: Vec::with_capacity(a.f.len() + kept.len()) };
    out.v.extend_from_slice(&a.v);
    out.v.extend_from_slice(&b.v);
    out.v.extend_from_slice(&points);
    if op != Op::Intersect {
        out.f.extend(a.f.iter().zip(&is_local).filter(|(_, local)| !**local).map(|(f, _)| *f));
    }
    out.f.extend(kept);
    if out.f.is_empty() {
        return Err(Snag::Empty);
    }
    Ok(out)
}

/// The end vertices of every crossed edge, labelled exactly by the first and last face they cross.
fn label_ends(
    chains: &HashMap<Edge, Vec<Crossing>>,
    face: impl Fn(u32) -> [P3; 3],
    vertex: impl Fn(u32) -> P3,
    offset: u32,
    known: &mut HashMap<u32, bool>,
) -> Result<(), Snag> {
    for (e, chain) in chains {
        let (Some(first), Some(last)) = (chain.first(), chain.last()) else { continue };
        for (end, c) in [(e.0, first), (e.1, last)] {
            let [p, q, r] = face(c.face);
            let inside = side(p, q, r, vertex(end)) > 0.0;
            if known.insert(end + offset, inside).is_some_and(|was| was != inside) {
                return Err(Snag::Degenerate("a vertex is both inside and outside"));
            }
        }
    }
    Ok(())
}

/// Spread the known labels over a side; crossing a cut flips the label.
fn flood(side: &mut Side, known: &HashMap<u32, bool>, constraint: &HashSet<Edge>, probe: impl Fn([u32; 3]) -> Option<bool>) -> Result<(), Snag> {
    let n = side.faces.len();
    let mut by_edge: HashMap<Edge, Vec<u32>> = HashMap::with_capacity(n * 2);
    for (i, f) in side.faces.iter().enumerate() {
        for k in 0..3 { by_edge.entry(key(f[k], f[(k + 1) % 3])).or_default().push(i as u32); }
    }
    let mut label: Vec<Option<bool>> = vec![None; n];
    let mut queue = VecDeque::new();
    for (i, f) in side.faces.iter().enumerate() {
        let mut seen = None;
        for v in f {
            if let Some(&l) = known.get(v) {
                if seen.is_some_and(|s| s != l) {
                    return Err(Snag::Degenerate("a face is both inside and outside"));
                }
                seen = Some(l);
            }
        }
        if let Some(l) = seen {
            label[i] = Some(l);
            queue.push_back(i as u32);
        }
    }
    let mut next_unlabelled = 0;
    loop {
        while let Some(i) = queue.pop_front() {
            let f = side.faces[i as usize];
            let mine = label[i as usize].expect("queued faces are labelled");
            for k in 0..3 {
                let e = key(f[k], f[(k + 1) % 3]);
                let theirs = if constraint.contains(&e) { !mine } else { mine };
                for &j in &by_edge[&e] {
                    if j == i { continue; }
                    match label[j as usize] {
                        None => {
                            label[j as usize] = Some(theirs);
                            queue.push_back(j);
                        }
                        Some(l) if l != theirs => return Err(Snag::Degenerate("labels disagree across an edge")),
                        _ => {}
                    }
                }
            }
        }
        // A shell the cut never reached: ask where it is.
        while next_unlabelled < n && label[next_unlabelled].is_some() { next_unlabelled += 1; }
        if next_unlabelled == n { break; }
        let l = probe(side.faces[next_unlabelled]).ok_or(Snag::Degenerate("a shell could not be placed"))?;
        label[next_unlabelled] = Some(l);
        queue.push_back(next_unlabelled as u32);
    }
    side.inside = label.into_iter().map(|l| l.unwrap_or(false)).collect();
    Ok(())
}

/// Retriangulate one face round the segments that cross it.
fn split(t: [u32; 3], chains: &HashMap<Edge, Vec<Crossing>>, segs: &[Edge], pos: impl Fn(u32) -> P3) -> Result<Vec<[u32; 3]>, Snag> {
    let p = t.map(&pos);
    let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
    let drop = (0..3).max_by(|a, b| n[*a].abs().total_cmp(&n[*b].abs())).unwrap();
    let (ax, ay) = if n[drop] >= 0.0 { ((drop + 1) % 3, (drop + 2) % 3) } else { ((drop + 2) % 3, (drop + 1) % 3) };
    let flat = |q: P3| Point2::new(q[ax], q[ay]);

    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    let mut handle = HashMap::new();
    let mut id_of = HashMap::new();
    let mut put = |id: u32, cdt: &mut ConstrainedDelaunayTriangulation<Point2<f64>>| -> Result<_, Snag> {
        if let Some(h) = handle.get(&id) { return Ok(*h); }
        let h = cdt.insert(flat(pos(id))).map_err(|_| Snag::Degenerate("a point will not triangulate"))?;
        if id_of.insert(h, id).is_some() {
            return Err(Snag::Degenerate("two points coincide in a face"));
        }
        handle.insert(id, h);
        Ok(h)
    };
    // Which border chain a vertex belongs to, for dropping the slivers a point a hair inside its edge leaves.
    let mut on_edge: HashMap<u32, u8> = HashMap::new();
    let mut constraints: Vec<(u32, u32)> = Vec::new();
    for k in 0..3 {
        let (i, j) = (t[k], t[(k + 1) % 3]);
        let mut run = vec![i];
        if let Some(chain) = chains.get(&key(i, j)) {
            let mut ids: Vec<u32> = chain.iter().map(|c| c.vertex).collect();
            if i > j { ids.reverse(); }
            run.extend(ids);
        }
        run.push(j);
        for id in &run { *on_edge.entry(*id).or_default() |= 1 << k; }
        constraints.extend(run.windows(2).map(|w| (w[0], w[1])));
    }
    constraints.extend(segs.iter().copied());
    for (u, v) in constraints {
        let (hu, hv) = (put(u, &mut cdt)?, put(v, &mut cdt)?);
        if hu == hv { return Err(Snag::Degenerate("a cut has no length")); }
        if !cdt.can_add_constraint(hu, hv) {
            return Err(Snag::Degenerate("two cuts cross inside a face"));
        }
        cdt.add_constraint(hu, hv);
    }
    let mut out = Vec::new();
    for face in cdt.inner_faces() {
        let ids = face.vertices().map(|v| id_of[&v.fix()]);
        let shared = ids.iter().fold(0b111u8, |m, id| m & on_edge.get(id).copied().unwrap_or(0));
        if shared != 0 { continue; }
        out.push(ids);
    }
    Ok(out)
}

/// Whether a point is inside a closed solid, by exact ray parity. `None` when every ray grazes something.
pub fn inside(s: &Solid, p: P3) -> Option<bool> {
    let (lo, hi) = s.bounds()?;
    if (0..3).any(|k| p[k] < lo[k] || p[k] > hi[k]) { return Some(false); }
    let reach = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max) * 4.0 + 1.0;
    'rays: for d in [[0.577, 0.211, 0.789], [-0.313, 0.871, 0.379], [0.127, -0.433, 0.892], [-0.703, -0.521, -0.484], [0.904, -0.057, -0.424]] {
        let q = add(p, scale(unit(d), reach));
        let mut hits = 0;
        for f in &s.f {
            let [a, b, c] = f.map(|i| s.v[i as usize]);
            match pierce(p, q, a, b, c) {
                Ok(Some(_)) => hits += 1,
                Ok(None) => {}
                Err(_) => continue 'rays,
            }
        }
        return Some(hits % 2 == 1);
    }
    None
}

/// Pairs of faces sharing no vertex whose interiors cross — zero on a sound solid. Quadratic; for tests and probes.
pub fn self_crossings(s: &Solid) -> usize {
    let boxes: Vec<(P3, P3)> = s.f.iter().map(|f| tri_bounds(f.map(|i| s.v[i as usize]))).collect();
    let mut found = 0;
    for i in 0..s.f.len() {
        for j in i + 1..s.f.len() {
            if !overlap(&boxes[i], &boxes[j]) || s.f[i].iter().any(|v| s.f[j].contains(v)) { continue; }
            let (a, b) = (s.f[i].map(|k| s.v[k as usize]), s.f[j].map(|k| s.v[k as usize]));
            let hits = (0..3).filter(|&k| matches!(pierce(a[k], a[(k + 1) % 3], b[0], b[1], b[2]), Ok(Some(_)))).count()
                + (0..3).filter(|&k| matches!(pierce(b[k], b[(k + 1) % 3], a[0], a[1], a[2]), Ok(Some(_)))).count();
            if hits > 0 { found += 1; }
        }
    }
    found
}

/// Two solids whose surfaces never meet: one holds the other, or neither does.
fn apart(a: &Solid, b: &Solid, op: Op) -> Result<Solid, Snag> {
    let b_in_a = b.v.first().and_then(|p| inside(a, *p)).ok_or(Snag::Degenerate("a point could not be placed"))?;
    let a_in_b = !b_in_a && a.v.first().and_then(|p| inside(b, *p)).ok_or(Snag::Degenerate("a point could not be placed"))?;
    let flipped = |s: &Solid| Solid { v: s.v.clone(), f: s.f.iter().map(|f| [f[0], f[2], f[1]]).collect() };
    match op {
        Op::Union if b_in_a => Ok(a.clone()),
        Op::Union if a_in_b => Ok(b.clone()),
        Op::Union => { let mut s = a.clone(); s.push(b); Ok(s) }
        Op::Subtract if b_in_a => { let mut s = a.clone(); s.push(&flipped(b)); Ok(s) }
        Op::Subtract if a_in_b => Err(Snag::Empty),
        Op::Subtract => Ok(a.clone()),
        Op::Intersect if b_in_a => Ok(b.clone()),
        Op::Intersect if a_in_b => Ok(a.clone()),
        Op::Intersect => Err(Snag::Empty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    pub fn sphere(c: P3, r: f64, rings: usize, around: usize) -> Solid {
        let mut s = Solid::default();
        s.v.push(add(c, [0.0, 0.0, r]));
        for i in 1..rings {
            let phi = PI * i as f64 / rings as f64;
            for j in 0..around {
                let th = 2.0 * PI * (j as f64 + 0.37) / around as f64;
                s.v.push(add(c, [r * phi.sin() * th.cos(), r * phi.sin() * th.sin(), r * phi.cos()]));
            }
        }
        s.v.push(add(c, [0.0, 0.0, -r]));
        let ring = |i: usize, j: usize| (1 + (i - 1) * around + j % around) as u32;
        let last = (s.v.len() - 1) as u32;
        for j in 0..around {
            s.f.push([0, ring(1, j), ring(1, j + 1)]);
            for i in 1..rings - 1 {
                s.f.push([ring(i, j), ring(i + 1, j), ring(i + 1, j + 1)]);
                s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i, j + 1)]);
            }
            s.f.push([ring(rings - 1, j), last, ring(rings - 1, j + 1)]);
        }
        s
    }

    fn closed(s: &Solid) {
        assert_eq!(s.open_edges(), (0, 0), "closed and manifold");
    }

    #[test]
    fn the_predicate_calls_the_inner_side_positive() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(side(a, b, c, [0.2, 0.2, -1.0]) > 0.0, "below an upward-wound face is inside");
        let s = sphere([0.0; 3], 1.0, 12, 16);
        closed(&s);
        assert!(s.volume() > 3.9 && s.volume() < 4.2);
        assert_eq!(inside(&s, [0.1, 0.2, 0.3]), Some(true));
        assert_eq!(inside(&s, [0.9, 0.9, 0.3]), Some(false));
    }

    #[test]
    fn two_spheres_join_cut_and_meet_with_the_volumes_the_lens_formula_gives() {
        let (r, d) = (1.0f64, 1.1f64);
        let a = sphere([0.0; 3], r, 40, 56);
        let b = sphere([d, 0.013, 0.007], r, 38, 52);
        let lens = PI * (4.0 * r + d) * (2.0 * r - d).powi(2) / 12.0;
        let ball = 4.0 / 3.0 * PI;
        for (op, want) in [(Op::Union, 2.0 * ball - lens), (Op::Subtract, ball - lens), (Op::Intersect, lens)] {
            let mut s = combine(&a, &b, op).unwrap_or_else(|e| panic!("{op:?}: {e}"));
            s.compact();
            closed(&s);
            let got = s.volume();
            assert!((got - want).abs() < 0.02 * ball, "{op:?}: {got} against {want}");
        }
    }

    #[test]
    fn a_tool_through_a_wall_opens_a_hole_and_a_buried_one_leaves_a_void() {
        let shell = combine(&sphere([0.0; 3], 2.0, 40, 60), &sphere([0.0; 3], 1.5, 30, 44), Op::Subtract).unwrap();
        closed(&shell);
        let hollow = 4.0 / 3.0 * PI * (8.0 - 3.375);
        assert!((shell.volume() - hollow).abs() < 0.03 * hollow, "{}", shell.volume());
        // A small ball through the wall: both skins are cut and joined by the tube between them.
        let mut drilled = combine(&shell, &sphere([1.75, 0.01, 0.02], 0.45, 24, 32), Op::Subtract).unwrap();
        drilled.compact();
        closed(&drilled);
        assert!(drilled.volume() < shell.volume() - 0.15);
        // Apart: nothing changes, and a union keeps both shells.
        let far = sphere([9.0, 0.0, 0.0], 0.5, 8, 10);
        assert_eq!(combine(&shell, &far, Op::Subtract).unwrap().f.len(), shell.f.len());
        assert_eq!(combine(&shell, &far, Op::Union).unwrap().f.len(), shell.f.len() + far.f.len());
    }

    #[test]
    fn slivers_collapse_and_caps_flip_without_opening_the_solid() {
        let ball = sphere([0.0; 3], 1.0, 12, 16);
        let area = |s: &Solid| s.f.iter().map(|f| dot(twice_area_of(&s.v, *f), twice_area_of(&s.v, *f)).sqrt() * 0.5).fold(f64::MAX, f64::min);
        // Split a face's edge `k` at `t` of its length; the far side is split too, or carries a face of no width.
        fn split(s: &mut Solid, fi: usize, k: usize, t: f64, cap: bool) {
            let f = s.f[fi];
            let (x, y, z) = (f[k], f[(k + 1) % 3], f[(k + 2) % 3]);
            let m = s.v.len() as u32;
            s.v.push(add(s.v[x as usize], scale(sub(s.v[y as usize], s.v[x as usize]), t)));
            s.f[fi] = [x, m, z];
            s.f.push([m, y, z]);
            if cap {
                s.f.push([x, y, m]);
                return;
            }
            let g = s.f.iter().position(|h| (0..3).any(|j| h[j] == y && h[(j + 1) % 3] == x)).unwrap();
            let j = (0..3).find(|j| s.f[g][*j] == y).unwrap();
            let w = s.f[g][(j + 2) % 3];
            s.f[g] = [y, m, w];
            s.f.push([m, x, w]);
        }
        let mut hair = ball.clone();
        // A vertex a hair from a corner: two faces with an edge of nothing.
        split(&mut hair, 20, 0, 1e-8, false);
        // A vertex halfway along an edge that the far side still runs straight past: a cap.
        split(&mut hair, 60, 1, 0.5, true);
        closed(&hair);
        assert!(area(&hair) < 1e-9);
        let volume = hair.volume();
        clean(&mut hair, 2e-5);
        hair.compact();
        closed(&hair);
        assert!(area(&hair) > 1e-6, "{}", area(&hair));
        assert!((hair.volume() - volume).abs() < 1e-9);
        // Nothing to clean leaves the solid as it was.
        let mut same = ball.clone();
        assert_eq!(clean(&mut same, 2e-5), 0);
        assert_eq!(same.f, ball.f);
    }

    #[test]
    fn many_parts_union_into_one_closed_solid() {
        let parts: Vec<Solid> = (0..6).map(|k| {
            let a = k as f64 * PI / 3.0;
            sphere([a.cos() * 0.8, a.sin() * 0.8, 0.05 * k as f64], 0.55, 18, 24)
        }).collect();
        let mut s = union_all(&parts).unwrap();
        s.compact();
        closed(&s);
        assert!(s.volume() > 0.0);
    }
}
