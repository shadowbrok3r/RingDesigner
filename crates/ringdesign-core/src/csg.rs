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
//!
//! Either input that is not closed is refused before a predicate runs;
//! [`combine_with`] reads a cancel flag between attempts and every 4096 face
//! pairs inside one; [`combine_traced`] keeps the input face behind every
//! output face and every crossing vertex, from which [`seam_loops`] chains
//! the junction; [`cluster`] groups parts by box so a ring of collars stays a
//! ring of small tools rather than one that covers the band.
use robust::{Coord3D, orient3d};
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};

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
    /// The caller's flag was raised.
    Cancelled,
    /// An input is not a closed manifold: directed edges without a twin, and edges used more than once.
    Unclosed { open: usize, repeated: usize },
}

impl std::fmt::Display for Snag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Degenerate(why) => write!(f, "the solids meet degenerately ({why})"),
            Self::Empty => write!(f, "nothing is left of the solid"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Unclosed { open, repeated } => write!(f, "the solid is not closed ({open} open edges, {repeated} repeated)"),
        }
    }
}

/// What a boolean needs of an input, measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Check {
    pub open_edges: usize,
    pub repeated_edges: usize,
    /// Faces the predicates call flat: a repeated corner, or three exactly collinear ones.
    pub zero_area_faces: usize,
    pub volume: f64,
    /// Only when asked for.
    pub self_crossings: Option<usize>,
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
        edge_census(self.v.len(), &self.f)
    }

    /// Closure, flat faces and volume; the crossing count only when asked, grid-culled.
    pub fn check(&self, crossings: bool) -> Check {
        let (open_edges, repeated_edges) = self.open_edges();
        Check {
            open_edges,
            repeated_edges,
            zero_area_faces: self.f.iter().filter(|f| flat(&self.v, **f, 0.0)).count(),
            volume: self.volume(),
            self_crossings: crossings.then(|| self_crossings(self)),
        }
    }

    /// Drops faces with a repeated corner and re-forms those thinner than `eps` (exactly flat at zero) the
    /// way [`clean`] does. Returns how many flat faces went; the rest stay where re-forming would open the solid.
    pub fn strip_zero_area(&mut self, eps: f64) -> usize {
        let before = self.f.iter().filter(|f| flat(&self.v, **f, eps)).count();
        if before == 0 {
            return 0;
        }
        self.f.retain(|f| f[0] != f[1] && f[1] != f[2] && f[2] != f[0]);
        clean(self, eps.max(1e-150));
        before - self.f.iter().filter(|f| flat(&self.v, **f, eps)).count()
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

/// Distinct directed edges without a twin, and distinct directed edges used more than once.
fn edge_census(nv: usize, f: &[[u32; 3]]) -> (usize, usize) {
    let nv = f.iter().flatten().fold(nv, |n, &i| n.max(i as usize + 1));
    let mut start = vec![0u32; nv + 1];
    for t in f {
        for &i in t {
            start[i as usize + 1] += 1;
        }
    }
    for v in 0..nv {
        start[v + 1] += start[v];
    }
    let mut fill = start.clone();
    let mut out = vec![0u32; f.len() * 3];
    for t in f {
        for k in 0..3 {
            let a = t[k] as usize;
            out[fill[a] as usize] = t[(k + 1) % 3];
            fill[a] += 1;
        }
    }
    let list = |v: u32| &out[start[v as usize] as usize..start[v as usize + 1] as usize];
    let (mut open, mut repeated) = (0, 0);
    for a in 0..nv as u32 {
        let mine = list(a);
        for (i, &b) in mine.iter().enumerate() {
            if mine[..i].contains(&b) {
                continue;
            }
            if mine[i + 1..].contains(&b) {
                repeated += 1;
            }
            if !list(b).contains(&a) {
                open += 1;
            }
        }
    }
    (open, repeated)
}

/// Height under `eps` times the longest edge; at zero, exactly collinear as the predicate sees it.
fn flat(v: &[P3], f: [u32; 3], eps: f64) -> bool {
    let [a, b, c] = f.map(|k| v[k as usize]);
    let (ab, ac, bc) = (sub(b, a), sub(c, a), sub(c, b));
    let n = cross(ab, ac);
    let h = dot(n, n).sqrt();
    let longest = dot(ab, ab).max(dot(ac, ac)).max(dot(bc, bc)).sqrt();
    if h <= eps * longest {
        return true;
    }
    // Above rounding the cross product decides; under it, the predicate against each axis does.
    if h > 1e-9 * longest * longest {
        return false;
    }
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].into_iter().all(|d| side(a, b, c, add(a, d)) == 0.0)
}

fn cancelled(flag: Option<&AtomicBool>) -> bool {
    flag.is_some_and(|f| f.load(Ordering::Relaxed))
}

/// Reads the flag once every 4096 ticks.
struct Poll<'a> {
    n: u32,
    cancel: Option<&'a AtomicBool>,
}

impl Poll<'_> {
    fn tick(&mut self) -> Result<(), Snag> {
        self.n = self.n.wrapping_add(1);
        if self.n & 4095 == 0 && cancelled(self.cancel) { Err(Snag::Cancelled) } else { Ok(()) }
    }
}

/// A boolean's result with the input face behind every output face and every crossing vertex named.
#[derive(Clone, Debug)]
pub struct Traced {
    pub solid: Solid,
    /// The input face each output face lies on, one per face.
    pub parent: Vec<Parent>,
    /// Every crossing vertex, with a face of each side whose meeting made it.
    pub seam: Vec<SeamVertex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parent {
    A(u32),
    B(u32),
}

/// A vertex where an edge of one side pierced a face of the other, numbered as `Traced::solid` is before any compaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeamVertex {
    pub vertex: u32,
    pub a_face: u32,
    pub b_face: u32,
}

/// A solid as it is, every face its own parent on `side`.
fn own(s: &Solid, side: fn(u32) -> Parent) -> Traced {
    Traced { solid: s.clone(), parent: (0..s.f.len() as u32).map(side).collect(), seam: Vec::new() }
}

/// Two shells side by side, unresolved.
fn both(a: &Solid, b: &Solid) -> Traced {
    let mut solid = a.clone();
    solid.push(b);
    let parent = (0..a.f.len() as u32).map(Parent::A).chain((0..b.f.len() as u32).map(Parent::B)).collect();
    Traced { solid, parent, seam: Vec::new() }
}

/// `a` with `b` joined, cut away, or kept in common. `b` is the small one.
pub fn combine(a: &Solid, b: &Solid, op: Op) -> Result<Solid, Snag> {
    combine_with(a, b, op, None)
}

/// [`combine`] that returns `Snag::Cancelled` soon after `cancel` is raised: read before each attempt and
/// every 4096 face pairs inside one.
pub fn combine_with(a: &Solid, b: &Solid, op: Op, cancel: Option<&AtomicBool>) -> Result<Solid, Snag> {
    combine_traced(a, b, op, cancel).map(|t| t.solid)
}

/// [`combine_with`] with the provenance kept: the solid is the same bytes. Either input that is not closed
/// is refused as `Snag::Unclosed` before anything is computed.
pub fn combine_traced(a: &Solid, b: &Solid, op: Op, cancel: Option<&AtomicBool>) -> Result<Traced, Snag> {
    if cancelled(cancel) {
        return Err(Snag::Cancelled);
    }
    for s in [a, b] {
        let (open, repeated) = s.open_edges();
        if open > 0 || repeated > 0 {
            return Err(Snag::Unclosed { open, repeated });
        }
    }
    let mut last = Snag::Degenerate("untried");
    for attempt in 0..8 {
        if cancelled(cancel) {
            return Err(Snag::Cancelled);
        }
        let tool;
        let b = if attempt == 0 { b } else { tool = b.jittered(attempt); &tool };
        match boolean(a, b, op, cancel) {
            Ok(t) => return Ok(t),
            Err(e @ (Snag::Empty | Snag::Cancelled)) => return Err(e),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// The crossing vertices chained into loops along the output edges an `A` face shares with a `B` face,
/// each loop's last vertex joining its first. A chain that cannot close is returned as it stands.
pub fn seam_loops(t: &Traced) -> Vec<Vec<u32>> {
    let seam: HashSet<u32> = t.seam.iter().map(|s| s.vertex).collect();
    let mut side_of: HashMap<(u32, u32), bool> = HashMap::new();
    for (f, p) in t.solid.f.iter().zip(&t.parent) {
        for k in 0..3 {
            let (u, v) = (f[k], f[(k + 1) % 3]);
            if seam.contains(&u) && seam.contains(&v) {
                side_of.insert((u, v), matches!(p, Parent::A(_)));
            }
        }
    }
    let mut next: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&(u, v), &s) in &side_of {
        if u < v && side_of.get(&(v, u)).is_some_and(|o| *o != s) {
            next.entry(u).or_default().push(v);
            next.entry(v).or_default().push(u);
        }
    }
    let mut starts: Vec<u32> = next.keys().copied().collect();
    starts.sort_unstable();
    for n in next.values_mut() {
        n.sort_unstable();
    }
    let mut used: HashSet<Edge> = HashSet::new();
    let mut loops = Vec::new();
    for start in starts {
        loop {
            let mut walk = vec![start];
            let mut at = start;
            while let Some(&to) = next[&at].iter().find(|&&to| !used.contains(&key(at, to))) {
                used.insert(key(at, to));
                if to == start {
                    break;
                }
                walk.push(to);
                at = to;
            }
            if walk.len() < 2 {
                break;
            }
            loops.push(walk);
        }
    }
    loops
}

/// Parts grouped by overlap of their boxes grown by `pad`, transitively, so a caller can union each group and
/// run one band operation per group: eight collars round a ring stay eight tools.
pub fn cluster(parts: &[&Solid], pad: f64) -> Vec<Vec<usize>> {
    let boxes: Vec<Option<(P3, P3)>> = parts.iter().map(|p| p.bounds().map(|(lo, hi)| (lo.map(|x| x - pad), hi.map(|x| x + pad)))).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    let mut root: Vec<usize> = (0..parts.len()).collect();
    for i in 0..parts.len() {
        for j in i + 1..parts.len() {
            if let (Some(p), Some(q)) = (&boxes[i], &boxes[j]) {
                if overlap(p, q) {
                    let (ri, rj) = (find(&mut root, i), find(&mut root, j));
                    root[ri.max(rj)] = ri.min(rj);
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut at: HashMap<usize, usize> = HashMap::new();
    for i in 0..parts.len() {
        let r = find(&mut root, i);
        match at.get(&r) {
            Some(&g) => groups[g].push(i),
            None => {
                at.insert(r, groups.len());
                groups.push(vec![i]);
            }
        }
    }
    groups
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

/// One side's faces after the cut, labelled by whether they sit inside the other solid, each with the input face it came from.
struct Side {
    faces: Vec<[u32; 3]>,
    inside: Vec<bool>,
    parent: Vec<u32>,
}

fn boolean(a: &Solid, b: &Solid, op: Op, cancel: Option<&AtomicBool>) -> Result<Traced, Snag> {
    let na = a.v.len() as u32;
    let nb = b.v.len() as u32;
    let Some(bb) = b.bounds() else { return if op == Op::Intersect { Err(Snag::Empty) } else { Ok(own(a, Parent::A)) } };
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
    let mut seam: Vec<SeamVertex> = Vec::new();
    let mut poll = Poll { n: 0, cancel };

    for &fa in &local {
        poll.tick()?;
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
            poll.tick()?;
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
                                seam.push(SeamVertex { vertex: id, a_face: fa, b_face: fb });
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
                                seam.push(SeamVertex { vertex: id, a_face: fa, b_face: fb });
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
    let mut side_a = Side { faces: Vec::new(), inside: Vec::new(), parent: Vec::new() };
    for &fa in &local {
        let t = a.f[fa as usize];
        match cuts_a.get(&fa) {
            None => side_a.faces.push(t),
            Some(segs) => side_a.faces.extend(split(t, &chain_a, segs, |i| pos(i, &points))?),
        }
        side_a.parent.resize(side_a.faces.len(), fa);
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
    let mut side_b = Side { faces: Vec::new(), inside: Vec::new(), parent: Vec::new() };
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
        side_b.parent.resize(side_b.faces.len(), fb);
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
    let mut kept_parent: Vec<Parent> = Vec::new();
    for ((f, inside), &p) in side_a.faces.iter().zip(&side_a.inside).zip(&side_a.parent) {
        if *inside == keep_a_inside {
            kept.push(*f);
            kept_parent.push(Parent::A(p));
        }
    }
    for ((f, inside), &p) in side_b.faces.iter().zip(&side_b.inside).zip(&side_b.parent) {
        if *inside == keep_b_inside {
            kept.push(if flip_b { [f[0], f[2], f[1]] } else { *f });
            kept_parent.push(Parent::B(p));
        }
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
    let mut parent: Vec<Parent> = Vec::with_capacity(a.f.len() + kept.len());
    out.v.extend_from_slice(&a.v);
    out.v.extend_from_slice(&b.v);
    out.v.extend_from_slice(&points);
    if op != Op::Intersect {
        for (i, (f, local)) in a.f.iter().zip(&is_local).enumerate() {
            if !*local {
                out.f.push(*f);
                parent.push(Parent::A(i as u32));
            }
        }
    }
    out.f.extend(kept);
    parent.extend(kept_parent);
    if out.f.is_empty() {
        return Err(Snag::Empty);
    }
    Ok(Traced { solid: out, parent, seam })
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

/// Pairs of faces sharing no vertex whose interiors cross — zero on a sound solid. Culled by the grid, so a
/// band costs seconds rather than hours.
pub fn self_crossings(s: &Solid) -> usize {
    let all: Vec<u32> = (0..s.f.len() as u32).collect();
    let mut grid = Grid::over(s, &all);
    let boxes: Vec<(P3, P3)> = s.f.iter().map(|f| tri_bounds(f.map(|i| s.v[i as usize]))).collect();
    let mut stamp = vec![u32::MAX; s.f.len()];
    let mut near = Vec::new();
    let mut found = 0;
    for i in 0..s.f.len() {
        near.clear();
        grid.each_cell(boxes[i].0, boxes[i].1, |cells, c| {
            for &j in &cells[c] {
                if j as usize > i && stamp[j as usize] != i as u32 {
                    stamp[j as usize] = i as u32;
                    near.push(j as usize);
                }
            }
        });
        let a = s.f[i].map(|k| s.v[k as usize]);
        for &j in &near {
            if !overlap(&boxes[i], &boxes[j]) || s.f[i].iter().any(|v| s.f[j].contains(v)) { continue; }
            let b = s.f[j].map(|k| s.v[k as usize]);
            let hits = (0..3).filter(|&k| matches!(pierce(a[k], a[(k + 1) % 3], b[0], b[1], b[2]), Ok(Some(_)))).count()
                + (0..3).filter(|&k| matches!(pierce(b[k], b[(k + 1) % 3], a[0], a[1], a[2]), Ok(Some(_)))).count();
            if hits > 0 { found += 1; }
        }
    }
    found
}

/// Two solids whose surfaces never meet: one holds the other, or neither does.
fn apart(a: &Solid, b: &Solid, op: Op) -> Result<Traced, Snag> {
    let b_in_a = b.v.first().and_then(|p| inside(a, *p)).ok_or(Snag::Degenerate("a point could not be placed"))?;
    let a_in_b = !b_in_a && a.v.first().and_then(|p| inside(b, *p)).ok_or(Snag::Degenerate("a point could not be placed"))?;
    let flipped = |s: &Solid| Solid { v: s.v.clone(), f: s.f.iter().map(|f| [f[0], f[2], f[1]]).collect() };
    match op {
        Op::Union if b_in_a => Ok(own(a, Parent::A)),
        Op::Union if a_in_b => Ok(own(b, Parent::B)),
        Op::Union => Ok(both(a, b)),
        Op::Subtract if b_in_a => Ok(both(a, &flipped(b))),
        Op::Subtract if a_in_b => Err(Snag::Empty),
        Op::Subtract => Ok(own(a, Parent::A)),
        Op::Intersect if b_in_a => Ok(own(b, Parent::B)),
        Op::Intersect if a_in_b => Ok(own(a, Parent::A)),
        Op::Intersect => Err(Snag::Empty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;
    use std::time::{Duration, Instant};

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

    /// An outward-wound box.
    pub fn cuboid(lo: P3, hi: P3) -> Solid {
        let v: Vec<P3> = (0..8).map(|i: usize| std::array::from_fn(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })).collect();
        let quads: [[u32; 4]; 6] = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
        let f = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
        Solid { v, f }
    }

    /// An outward-wound cylinder about z from `z0` to `z1`, caps fanned from their centres.
    pub fn cylinder(r: f64, z0: f64, z1: f64, n: usize) -> Solid {
        let mut s = Solid::default();
        for z in [z0, z1] {
            for j in 0..n {
                let th = 2.0 * PI * (j as f64 + 0.37) / n as f64;
                s.v.push([r * th.cos(), r * th.sin(), z]);
            }
        }
        s.v.push([0.0, 0.0, z0]);
        s.v.push([0.0, 0.0, z1]);
        let n = n as u32;
        let (cb, ct) = (2 * n, 2 * n + 1);
        for j in 0..n {
            let (b0, b1, t0, t1) = (j, (j + 1) % n, n + j, n + (j + 1) % n);
            s.f.push([b0, b1, t1]);
            s.f.push([b0, t1, t0]);
            s.f.push([cb, b1, b0]);
            s.f.push([ct, t0, t1]);
        }
        s
    }

    /// Split a face's edge `k` at `t` of its length; the far side is split too, or carries a face of no width.
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

    /// Every face pair tested, the way `self_crossings` was before the grid.
    fn brute_crossings(s: &Solid) -> usize {
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

    fn on_face(s: &Solid, f: u32, p: P3) -> bool {
        let [a, b, c] = s.f[f as usize].map(|k| s.v[k as usize]);
        dot(unit(cross(sub(b, a), sub(c, a))), sub(p, a)).abs() < 1e-9
    }

    /// The Court band built at the given sweep, with its mesh.
    fn court_band(theta_steps: usize, profile_steps: usize) -> (crate::Mesh, Solid) {
        let t = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap();
        let params = crate::BuildParams { theta_steps, profile_steps, ..crate::BuildParams::default() };
        let built = crate::mesh::try_build(&t.design(), &crate::AlphaLibrary::builtin(), params).unwrap();
        let solid = Solid { v: built.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: built.mesh.faces.clone() };
        (built.mesh, solid)
    }

    /// A part's frame `sink` mm under the crest at the top of the ring, as `join_probe` seats one.
    fn seat_on_crest(mesh: &crate::Mesh, sink: f64) -> Frame {
        let theta = 90.0_f64.to_radians();
        let from = [(40.0 * theta.cos()) as f32, (40.0 * theta.sin()) as f32, 0.0];
        let (face, hit) = crate::interaction::picking::raycast(mesh, from, [(-theta.cos()) as f32, (-theta.sin()) as f32, 0.0]).unwrap();
        let (a, b, c) = mesh.triangle(&mesh.faces[face]).unwrap();
        let normal = unit(cross(sub(b, a), sub(c, a)));
        Frame::from_normal(sub(hit.map(f64::from), scale(normal, sink)), normal, [0.0, 0.0, 1.0])
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

    #[test]
    fn a_raised_flag_returns_before_the_band_is_touched() {
        let (_, band) = court_band(1024, 384);
        assert!(band.f.len() > 700_000, "{}", band.f.len());
        let tool = cylinder(3.0, 0.0, 2.5, 48).translated([0.0, 9.0, 0.0]);
        let flag = AtomicBool::new(true);
        let started = Instant::now();
        let result = combine_with(&band, &tool, Op::Union, Some(&flag));
        let took = started.elapsed();
        assert!(matches!(result, Err(Snag::Cancelled)));
        assert!(took < Duration::from_millis(200), "{took:?}");
        assert_eq!(Snag::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn a_flag_raised_mid_way_is_seen_within_a_tenth_of_a_second() {
        let (_, band) = court_band(256, 128);
        // The band against itself shifted: every face has a dozen near pairs, so one attempt runs long.
        let tool = band.translated([0.011, 0.007, 0.013]);
        let flag = AtomicBool::new(false);
        let raise_at = Duration::from_millis(60);
        let started = Instant::now();
        let result = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(raise_at);
                flag.store(true, Ordering::Relaxed);
            });
            combine_with(&band, &tool, Op::Union, Some(&flag))
        });
        let took = started.elapsed();
        assert!(matches!(result, Err(Snag::Cancelled)), "{:?}", result.map(|s| s.f.len()));
        assert!(took >= raise_at && took < raise_at + Duration::from_millis(100), "{took:?}");
    }

    #[test]
    fn an_unclosed_input_is_refused_before_anything_is_computed() {
        let ball = sphere([0.0; 3], 1.0, 12, 16);
        let mut torn = cuboid([-2.0; 3], [2.0; 3]);
        torn.f.pop();
        assert_eq!(torn.open_edges(), (3, 0));
        assert!(matches!(combine(&torn, &ball, Op::Union), Err(Snag::Unclosed { open: 3, repeated: 0 })));
        assert!(matches!(combine(&ball, &torn, Op::Subtract), Err(Snag::Unclosed { open: 3, repeated: 0 })));
        let mut doubled = cuboid([-2.0; 3], [2.0; 3]);
        doubled.f.push(doubled.f[0]);
        assert!(matches!(combine(&doubled, &ball, Op::Union), Err(Snag::Unclosed { open: 0, repeated: 3 })));
        assert_eq!(Snag::Unclosed { open: 0, repeated: 3 }.to_string(), "the solid is not closed (0 open edges, 3 repeated)");
        // Said before any predicate runs: a torn tool that would only ever meet the block degenerately is refused for its edges.
        let block = cuboid([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
        let mut coplanar = cylinder(1.5, 10.0, 13.0, 32);
        coplanar.f.pop();
        assert!(matches!(combine(&block, &coplanar, Op::Union), Err(Snag::Unclosed { open: 3, repeated: 0 })));
        assert!(combine(&cuboid([-2.0; 3], [2.0; 3]), &ball, Op::Union).is_ok());
    }

    #[test]
    fn check_counts_what_a_boolean_needs_and_strip_repairs_the_flat_faces() {
        let ball = sphere([0.0; 3], 1.0, 12, 16);
        let c = ball.check(true);
        assert_eq!((c.open_edges, c.repeated_edges, c.zero_area_faces, c.self_crossings), (0, 0, 0, Some(0)));
        assert_eq!(c.volume, ball.volume());
        assert_eq!(ball.check(false).self_crossings, None);
        // Two shells pushed together unresolved cross each other; the grid count is the brute-force count.
        let mut pair = ball.clone();
        pair.push(&sphere([0.9, 0.05, 0.02], 1.0, 10, 14));
        let brute = brute_crossings(&pair);
        assert!(brute > 20, "{brute}");
        assert_eq!(pair.check(true).self_crossings, Some(brute));
        // Flat faces: a repeated corner, a needle (a vertex on top of a corner) and an exactly collinear cap.
        let mut flat = cuboid([0.0; 3], [2.0; 3]);
        flat.f.push([3, 3, 4]);
        split(&mut flat, 4, 1, 0.0, false);
        split(&mut flat, 0, 0, 0.5, true);
        closed(&flat);
        let c = flat.check(false);
        assert_eq!((c.open_edges, c.repeated_edges, c.zero_area_faces), (0, 0, 4));
        // A collinear face puts every point in its plane, which no nudge of the other solid can change.
        let [x, y, m] = flat.f.last().unwrap().map(|k| flat.v[k as usize]);
        assert!(matches!(pierce([5.0, 5.0, 5.0], [-5.0, -3.0, -4.0], x, y, m), Err(Snag::Degenerate(_))));
        assert_eq!(flat.strip_zero_area(0.0), 4);
        closed(&flat);
        let c = flat.check(true);
        assert_eq!((c.zero_area_faces, c.self_crossings), (0, Some(0)));
        assert!((c.volume - 8.0).abs() < 1e-12, "{}", c.volume);
        // Nothing flat leaves the solid untouched.
        let mut same = ball.clone();
        assert_eq!(same.strip_zero_area(0.0), 0);
        assert_eq!(same.f, ball.f);
    }

    #[test]
    fn a_traced_union_names_every_face_and_chains_one_seam() {
        let a = cuboid([0.0; 3], [1.0; 3]);
        let b = cuboid([0.37, 0.52, 0.61], [1.37, 1.52, 1.61]);
        let t = combine_traced(&a, &b, Op::Union, None).unwrap();
        for s in [combine(&a, &b, Op::Union).unwrap(), combine_with(&a, &b, Op::Union, None).unwrap()] {
            assert!(s.v == t.solid.v && s.f == t.solid.f, "the same bytes with or without provenance");
        }
        closed(&t.solid);
        assert!((t.solid.volume() - (2.0 - 0.63 * 0.48 * 0.39)).abs() < 1e-12);
        assert_eq!(t.parent.len(), t.solid.f.len());
        let (mut area_a, mut area_b) = (0.0, 0.0);
        for (f, p) in t.solid.f.iter().zip(&t.parent) {
            let [x, y, z] = f.map(|k| t.solid.v[k as usize]);
            let centre = scale(add(add(x, y), z), 1.0 / 3.0);
            let n = cross(sub(y, x), sub(z, x));
            let area = 0.5 * dot(n, n).sqrt();
            match *p {
                Parent::A(i) => { assert!(on_face(&a, i, centre)); area_a += area; }
                Parent::B(i) => { assert!(on_face(&b, i, centre)); area_b += area; }
            }
        }
        // Each side keeps exactly its surface outside the other: six less the three rectangles the other covers.
        let hidden = 0.48 * 0.39 + 0.63 * 0.39 + 0.63 * 0.48;
        assert!((area_a - (6.0 - hidden)).abs() < 1e-9 && (area_b - (6.0 - hidden)).abs() < 1e-9, "{area_a} {area_b}");
        let (na, nb) = (a.v.len() as u32, b.v.len() as u32);
        assert!(!t.seam.is_empty());
        for s in &t.seam {
            assert!(s.vertex >= na + nb && (s.a_face as usize) < a.f.len() && (s.b_face as usize) < b.f.len());
            let p = t.solid.v[s.vertex as usize];
            assert!(on_face(&a, s.a_face, p) && on_face(&b, s.b_face, p));
        }
        let loops = seam_loops(&t);
        assert_eq!(loops.len(), 1, "{loops:?}");
        let along: HashSet<u32> = loops[0].iter().copied().collect();
        assert_eq!(along.len(), loops[0].len(), "no vertex twice");
        assert_eq!(along, t.seam.iter().map(|s| s.vertex).collect::<HashSet<_>>());
        // Every step, the last back to the first, is an edge one A face shares with one B face.
        let n = loops[0].len();
        for i in 0..n {
            let (u, v) = (loops[0][i], loops[0][(i + 1) % n]);
            let sides: Vec<bool> = t.solid.f.iter().zip(&t.parent).filter(|(f, _)| f.contains(&u) && f.contains(&v)).map(|(_, p)| matches!(p, Parent::A(_))).collect();
            assert_eq!(sides.len(), 2, "{u} {v}");
            assert_ne!(sides[0], sides[1]);
        }
    }

    #[test]
    fn a_bezel_on_the_court_band_makes_one_seam_loop() {
        let (mesh, band) = court_band(256, 128);
        let tool = cylinder(3.0, 0.0, 2.5, 48).placed(&seat_on_crest(&mesh, 0.05));
        let t = combine_traced(&band, &tool, Op::Union, None).unwrap();
        let plain = combine(&band, &tool, Op::Union).unwrap();
        assert!(plain.v == t.solid.v && plain.f == t.solid.f);
        closed(&t.solid);
        // The whole cylinder less the sliver its foot sinks into the crest.
        let added = t.solid.volume() - band.volume();
        assert!(added > tool.volume() - 0.5 && added < tool.volume(), "{added} against {}", tool.volume());
        assert_eq!(t.parent.len(), t.solid.f.len());
        assert!(t.parent.iter().all(|p| match *p { Parent::A(i) => (i as usize) < band.f.len(), Parent::B(i) => (i as usize) < tool.f.len() }));
        let loops = seam_loops(&t);
        assert_eq!(loops.len(), 1, "{}", loops.len());
        assert_eq!(loops[0].iter().copied().collect::<HashSet<_>>(), t.seam.iter().map(|s| s.vertex).collect::<HashSet<_>>());
        assert!(loops[0].len() >= 8, "{}", loops[0].len());
    }

    #[test]
    fn parts_cluster_by_padded_box_overlap() {
        let collars: Vec<Solid> = (0..8).map(|k| {
            let a = k as f64 * PI / 4.0;
            let c = [9.0 * a.cos(), 9.0 * a.sin(), 0.0];
            cuboid(sub(c, [0.5; 3]), add(c, [0.5; 3]))
        }).collect();
        let parts: Vec<&Solid> = collars.iter().collect();
        assert_eq!(cluster(&parts, 0.05), (0..8).map(|i| vec![i]).collect::<Vec<_>>());
        // Neighbours stand 1.64 mm apart on one axis and 5.36 on the other; a pad past half of that chains the ring.
        assert_eq!(cluster(&parts, 3.0), vec![(0..8).collect::<Vec<_>>()]);
        let touching = [cuboid([0.0; 3], [1.0; 3]), cuboid([1.0, 0.0, 0.0], [2.0, 1.0, 1.0])];
        assert_eq!(cluster(&[&touching[0], &touching[1]], 0.0), vec![vec![0, 1]]);
        // Transitive: the ends of a chain never meet, and one group holds all three.
        let chain = [cuboid([0.0; 3], [1.0; 3]), cuboid([2.0, 0.0, 0.0], [3.0, 1.0, 1.0]), cuboid([0.9, 0.0, 0.0], [2.1, 1.0, 1.0])];
        assert_eq!(cluster(&[&chain[0], &chain[1], &chain[2]], 0.0), vec![vec![0, 1, 2]]);
        assert_eq!(cluster(&[&chain[0], &chain[1]], 0.0), vec![vec![0], vec![1]]);
        let none: [&Solid; 0] = [];
        assert!(cluster(&none, 1.0).is_empty());
    }

    /// A part's foot against the surface it stands on, the four ways it can land. Measured: a foot sunk
    /// 0.05 mm and a foot 0.05 mm clear resolve on the first attempt; a coplanar foot and a tangent sphere
    /// are degenerate as drawn (a vertex in a face's plane; an edge through a face's border) and resolve on
    /// the first nudge, the foot fused with a 0.1 µm wedge of slivers left for `clean`, the sphere lifted
    /// clear and riding along as its own shell. M2 sinks a part 0.05 mm by default because that is the
    /// case that needs neither a nudge nor a cleaning.
    #[test]
    fn the_contact_matrix_says_which_touches_resolve() {
        let block = cuboid([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
        let post = |foot: f64| cylinder(1.5, 10.0 + foot, 13.0 + foot, 32);
        let first_ok = |tool: &Solid| (0..8).find(|&k| boolean(&block, &if k == 0 { tool.clone() } else { tool.jittered(k) }, Op::Union, None).is_ok());
        let resolved = |tool: &Solid| -> (Solid, usize) {
            let mut s = combine(&block, tool, Op::Union).unwrap();
            closed(&s);
            let cleaned = clean(&mut s, 2e-5);
            closed(&s);
            (s, cleaned)
        };
        // (a) coplanar: every cap vertex lies in the top face's plane.
        let tool = post(0.0);
        assert!(matches!(boolean(&block, &tool, Op::Union, None), Err(Snag::Degenerate("a vertex lies in a face's plane"))));
        assert_eq!(first_ok(&tool), Some(1));
        let (s, cleaned) = resolved(&tool);
        assert!((s.volume() - (block.volume() + tool.volume())).abs() < 1e-6, "{}", s.volume());
        assert!(cleaned > 0, "the tilted foot leaves a wedge of slivers");
        // (b) sunk 0.05 mm: first attempt, the overlap exactly the sunk disc.
        let tool = post(-0.05);
        assert_eq!(first_ok(&tool), Some(0));
        let (s, cleaned) = resolved(&tool);
        assert!((s.volume() - (block.volume() + tool.volume() * (2.95 / 3.0))).abs() < 1e-9, "{}", s.volume());
        assert_eq!(cleaned, 0);
        // (c) raised 0.05 mm: apart, two shells.
        let tool = post(0.05);
        assert_eq!(first_ok(&tool), Some(0));
        let (s, cleaned) = resolved(&tool);
        assert_eq!((s.f.len(), cleaned), (block.f.len() + tool.f.len(), 0));
        assert!((s.volume() - (block.volume() + tool.volume())).abs() < 1e-9);
        // (d) tangent sphere: its pole sits on the top face's diagonal; the nudge lifts it clear, unfused.
        let tool = sphere([0.0, 0.0, 11.5], 1.5, 12, 16);
        assert!(matches!(boolean(&block, &tool, Op::Union, None), Err(Snag::Degenerate("an edge passes through a face's border"))));
        assert_eq!(first_ok(&tool), Some(1));
        let (s, cleaned) = resolved(&tool);
        assert_eq!((s.f.len(), cleaned), (block.f.len() + tool.f.len(), 0));
        assert!((s.volume() - (block.volume() + tool.volume())).abs() < 1e-9);
    }
}
