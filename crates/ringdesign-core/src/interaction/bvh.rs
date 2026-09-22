//! An axis-aligned bounding-box tree over a mesh's faces, so a ray or a point asks a few dozen
//! triangles instead of every one. Bounds are the mesh's own f32 coordinates, so the tree is
//! exact for the faces it holds, and the ray test is the same Möller–Trumbore as
//! `picking::raycast`, so both name the same face at the same distance.
use crate::mesh::Mesh;

/// Faces a leaf holds at most.
const LEAF_FACES: usize = 4;
/// Traversal stack depth: a median split over `u32::MAX` faces is 32 levels.
const STACK: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Node {
    min: [f32; 3],
    max: [f32; 3],
    /// First child (its sibling follows it) for an inner node, first slot in `order` for a leaf.
    first: u32,
    /// Faces in a leaf; zero for an inner node.
    count: u32,
}

/// Face bounds tree; `order` holds the face indices the leaves point into.
#[derive(Clone, Debug, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

/// Per-face bounds and centroids the build partitions on.
struct Slab {
    lo: Vec<[f32; 3]>,
    hi: Vec<[f32; 3]>,
    mid: Vec<[f32; 3]>,
}

impl Bvh {
    /// The tree over every face of `mesh` whose three indices are in range.
    pub fn build(mesh: &Mesh) -> Self {
        let n = mesh.faces.len();
        let mut slab = Slab { lo: Vec::with_capacity(n), hi: Vec::with_capacity(n), mid: Vec::with_capacity(n) };
        let mut order = Vec::with_capacity(n);
        for (i, f) in mesh.faces.iter().enumerate() {
            let (Some(a), Some(b), Some(c)) = (
                mesh.vertices.get(f[0] as usize),
                mesh.vertices.get(f[1] as usize),
                mesh.vertices.get(f[2] as usize),
            ) else {
                slab.lo.push([0.0; 3]);
                slab.hi.push([0.0; 3]);
                slab.mid.push([0.0; 3]);
                continue;
            };
            let (a, b, c) = ([a.0, a.1, a.2], [b.0, b.1, b.2], [c.0, c.1, c.2]);
            if !a.iter().chain(&b).chain(&c).all(|v| v.is_finite()) {
                slab.lo.push([0.0; 3]);
                slab.hi.push([0.0; 3]);
                slab.mid.push([0.0; 3]);
                continue;
            }
            let lo = std::array::from_fn(|k| a[k].min(b[k]).min(c[k]));
            let hi = std::array::from_fn(|k| a[k].max(b[k]).max(c[k]));
            slab.lo.push(lo);
            slab.hi.push(hi);
            slab.mid.push(std::array::from_fn(|k| (lo[k] + hi[k]) * 0.5));
            order.push(i as u32);
        }
        let mut tree = Self { nodes: Vec::with_capacity(2 * (order.len() / LEAF_FACES).max(1)), order };
        if !tree.order.is_empty() {
            tree.nodes.push(Node { min: [0.0; 3], max: [0.0; 3], first: 0, count: 0 });
            let end = tree.order.len();
            tree.fill(0, 0, end, &slab);
        }
        tree
    }

    /// Faces the tree holds.
    pub fn faces(&self) -> usize {
        self.order.len()
    }

    /// The root's bounds.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let root = self.nodes.first()?;
        Some((root.min.map(f64::from), root.max.map(f64::from)))
    }

    /// Bounds node `node` over `order[start..end]` and splits it at the centroid median of the widest axis.
    fn fill(&mut self, node: usize, start: usize, end: usize, slab: &Slab) {
        let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        let (mut cmin, mut cmax) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for &f in &self.order[start..end] {
            let f = f as usize;
            for k in 0..3 {
                min[k] = min[k].min(slab.lo[f][k]);
                max[k] = max[k].max(slab.hi[f][k]);
                cmin[k] = cmin[k].min(slab.mid[f][k]);
                cmax[k] = cmax[k].max(slab.mid[f][k]);
            }
        }
        self.nodes[node].min = min;
        self.nodes[node].max = max;
        let count = end - start;
        let axis = (0..3).max_by(|&a, &b| (cmax[a] - cmin[a]).total_cmp(&(cmax[b] - cmin[b]))).unwrap_or(0);
        if count <= LEAF_FACES || cmax[axis] - cmin[axis] <= 0.0 {
            self.nodes[node].first = start as u32;
            self.nodes[node].count = count as u32;
            return;
        }
        let mid = count / 2;
        self.order[start..end].select_nth_unstable_by(mid, |&a, &b| slab.mid[a as usize][axis].total_cmp(&slab.mid[b as usize][axis]));
        let first = self.nodes.len();
        self.nodes[node].first = first as u32;
        self.nodes[node].count = 0;
        self.nodes.push(Node { min: [0.0; 3], max: [0.0; 3], first: 0, count: 0 });
        self.nodes.push(Node { min: [0.0; 3], max: [0.0; 3], first: 0, count: 0 });
        self.fill(first, start, start + mid, slab);
        self.fill(first + 1, start + mid, end, slab);
    }

    /// The nearest face the ray from `origin` along `direction` crosses, and the distance to it in
    /// units of `direction`; the same test and thresholds as `picking::raycast`.
    pub fn ray(&self, mesh: &Mesh, origin: [f64; 3], direction: [f64; 3]) -> Option<(usize, f64)> {
        if self.nodes.is_empty() || !origin.iter().chain(&direction).all(|v| v.is_finite()) {
            return None;
        }
        let mut best = f64::INFINITY;
        let mut hit = None;
        let mut stack = [(0u32, 0.0f64); STACK];
        let mut depth = 0;
        let Some(t0) = enter(&self.nodes[0], origin, direction, best) else { return None };
        stack[0] = (0, t0);
        depth += 1;
        while depth > 0 {
            depth -= 1;
            let (index, t_enter) = stack[depth];
            if t_enter >= best {
                continue;
            }
            let node = &self.nodes[index as usize];
            if node.count > 0 {
                for &f in &self.order[node.first as usize..(node.first + node.count) as usize] {
                    if let Some(t) = triangle_ray(mesh, f as usize, origin, direction) {
                        if t < best {
                            best = t;
                            hit = Some(f as usize);
                        }
                    }
                }
                continue;
            }
            let (l, r) = (node.first, node.first + 1);
            let tl = enter(&self.nodes[l as usize], origin, direction, best);
            let tr = enter(&self.nodes[r as usize], origin, direction, best);
            match (tl, tr) {
                (Some(a), Some(b)) => {
                    // The nearer child is popped first.
                    let (near, far) = if a <= b { ((l, a), (r, b)) } else { ((r, b), (l, a)) };
                    if depth + 2 > STACK {
                        return hit.map(|f| (f, best));
                    }
                    stack[depth] = far;
                    stack[depth + 1] = near;
                    depth += 2;
                }
                (Some(a), None) => {
                    stack[depth] = (l, a);
                    depth += 1;
                }
                (None, Some(b)) => {
                    stack[depth] = (r, b);
                    depth += 1;
                }
                (None, None) => {}
            }
        }
        hit.map(|f| (f, best))
    }

    /// The face nearest `point` within `max_mm`, and the closest point on it.
    pub fn nearest(&self, mesh: &Mesh, point: [f64; 3], max_mm: f64) -> Option<(usize, [f64; 3])> {
        if self.nodes.is_empty() || !point.iter().all(|v| v.is_finite()) || !(max_mm > 0.0) {
            return None;
        }
        let mut best = max_mm * max_mm;
        let mut found = None;
        let mut stack = [(0u32, 0.0f64); STACK];
        let mut depth = 0;
        let d0 = box_distance2(&self.nodes[0], point);
        if d0 > best {
            return None;
        }
        stack[0] = (0, d0);
        depth += 1;
        while depth > 0 {
            depth -= 1;
            let (index, d_enter) = stack[depth];
            if d_enter > best {
                continue;
            }
            let node = &self.nodes[index as usize];
            if node.count > 0 {
                for &f in &self.order[node.first as usize..(node.first + node.count) as usize] {
                    let Some((a, b, c)) = mesh.triangle(&mesh.faces[f as usize]) else { continue };
                    let q = closest_on_triangle(point, a, b, c);
                    let d2 = dist2(q, point);
                    if d2 < best {
                        best = d2;
                        found = Some((f as usize, q));
                    }
                }
                continue;
            }
            let (l, r) = (node.first, node.first + 1);
            let dl = box_distance2(&self.nodes[l as usize], point);
            let dr = box_distance2(&self.nodes[r as usize], point);
            let (near, far) = if dl <= dr { ((l, dl), (r, dr)) } else { ((r, dr), (l, dl)) };
            if depth + 2 > STACK {
                return found;
            }
            if far.1 <= best {
                stack[depth] = far;
                depth += 1;
            }
            if near.1 <= best {
                stack[depth] = near;
                depth += 1;
            }
        }
        found
    }
}

/// Rounding slack on a slab exit (Pharr, Physically Based Rendering 3.9.2): a ray through a box's
/// corner or edge has its entry and exit equal, and a few ulps the wrong way would drop the box.
const SLAB_SLACK: f64 = 1.0 + 2.0 * 3.0 * f64::EPSILON;

/// Where the ray enters the node's box, when it does before `limit`.
fn enter(node: &Node, o: [f64; 3], d: [f64; 3], limit: f64) -> Option<f64> {
    let (mut tmin, mut tmax) = (0.0f64, limit);
    for k in 0..3 {
        let (lo, hi) = (node.min[k] as f64, node.max[k] as f64);
        if d[k].abs() < 1e-300 {
            if o[k] < lo || o[k] > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[k];
        let (mut t0, mut t1) = ((lo - o[k]) * inv, (hi - o[k]) * inv);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        tmin = tmin.max(t0);
        tmax = tmax.min(t1 * SLAB_SLACK);
        if tmin > tmax {
            return None;
        }
    }
    Some(tmin)
}

/// Squared distance from `p` to the node's box; zero inside it.
fn box_distance2(node: &Node, p: [f64; 3]) -> f64 {
    let mut d2 = 0.0;
    for k in 0..3 {
        let (lo, hi) = (node.min[k] as f64, node.max[k] as f64);
        let d = if p[k] < lo { lo - p[k] } else if p[k] > hi { p[k] - hi } else { 0.0 };
        d2 += d * d;
    }
    d2
}

/// Möller–Trumbore with `picking::raycast`'s thresholds; the distance in units of `d`.
pub(crate) fn triangle_ray(mesh: &Mesh, face: usize, o: [f64; 3], d: [f64; 3]) -> Option<f64> {
    let (a, b, c) = mesh.triangle(mesh.faces.get(face)?)?;
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let t = sub(o, a);
    let u = dot(t, p) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(t, e1);
    let v = dot(d, q) / det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = dot(e2, q) / det;
    (distance > 1e-6).then_some(distance)
}

/// The point of triangle `abc` closest to `p` (Ericson, Real-Time Collision Detection 5.1.5).
pub(crate) fn closest_on_triangle(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = if (d1 - d3).abs() > 0.0 { d1 / (d1 - d3) } else { 0.0 };
        return lerp(a, ab, v);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = if (d2 - d6).abs() > 0.0 { d2 / (d2 - d6) } else { 0.0 };
        return lerp(a, ac, w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let denom = (d4 - d3) + (d5 - d6);
        let w = if denom.abs() > 0.0 { (d4 - d3) / denom } else { 0.0 };
        return lerp(b, sub(c, b), w);
    }
    let denom = va + vb + vc;
    if denom.abs() <= 0.0 {
        return a;
    }
    let v = vb / denom;
    let w = vc / denom;
    std::array::from_fn(|k| a[k] + ab[k] * v + ac[k] * w)
}

fn lerp(a: [f64; 3], d: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|k| a[k] + d[k] * t)
}
pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|k| a[k] - b[k])
}
pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub(crate) fn dist2(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{AlphaLibrary, BuildParams, RingDesign, mesh};

    /// A small deterministic generator for the comparisons.
    pub(crate) struct Lcg(pub u64);
    impl Lcg {
        pub(crate) fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
        pub(crate) fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next()
        }
    }

    /// Every face, the way `picking::raycast` walks them, with the distance kept.
    pub(crate) fn brute_ray(mesh: &Mesh, o: [f64; 3], d: [f64; 3]) -> Option<(usize, f64)> {
        let mut best = f64::INFINITY;
        let mut index = None;
        for f in 0..mesh.faces.len() {
            if let Some(t) = triangle_ray(mesh, f, o, d) {
                if t < best {
                    best = t;
                    index = Some(f);
                }
            }
        }
        index.map(|f| (f, best))
    }

    fn brute_nearest(mesh: &Mesh, p: [f64; 3], max_mm: f64) -> Option<(usize, [f64; 3])> {
        let mut best = max_mm * max_mm;
        let mut found = None;
        for (f, face) in mesh.faces.iter().enumerate() {
            let Some((a, b, c)) = mesh.triangle(face) else { continue };
            let q = closest_on_triangle(p, a, b, c);
            let d2 = dist2(q, p);
            if d2 < best {
                best = d2;
                found = Some((f, q));
            }
        }
        found
    }

    /// Rays from a sphere round the ring aimed at random points inside its box, plus a share
    /// aimed at the surface itself.
    pub(crate) fn random_rays(mesh: &Mesh, n: usize, seed: u64) -> Vec<([f64; 3], [f64; 3])> {
        let (lo, hi) = mesh.bounds().unwrap();
        let (lo, hi) = ([lo.0 as f64, lo.1 as f64, lo.2 as f64], [hi.0 as f64, hi.1 as f64, hi.2 as f64]);
        let mut rng = Lcg(seed);
        (0..n)
            .map(|i| {
                let (th, ph) = (rng.range(0.0, std::f64::consts::TAU), rng.range(-1.0, 1.0f64).acos());
                let o = [40.0 * ph.sin() * th.cos(), 40.0 * ph.sin() * th.sin(), 40.0 * ph.cos()];
                let target: [f64; 3] = if i % 3 == 0 {
                    // Toward a vertex, so the ray crosses a shared edge or corner now and then.
                    let v = mesh.vertices[(rng.next() * mesh.vertices.len() as f64) as usize % mesh.vertices.len()];
                    [v.0 as f64, v.1 as f64, v.2 as f64]
                } else {
                    std::array::from_fn(|k| rng.range(lo[k], hi[k]))
                };
                (o, sub(target, o))
            })
            .collect()
    }

    fn ring(theta: usize, profile: usize) -> Mesh {
        let d = RingDesign::default();
        let lib = AlphaLibrary::builtin();
        mesh::build(&d, &lib, BuildParams { theta_steps: theta, profile_steps: profile, ..BuildParams::default() }).mesh
    }

    #[test]
    fn the_tree_holds_every_face_and_agrees_with_the_brute_force_ray() {
        let m = ring(192, 96);
        let bvh = Bvh::build(&m);
        assert_eq!(bvh.faces(), m.faces.len());
        let rays = random_rays(&m, 1000, 7);
        let mut hits = 0;
        for (o, d) in rays {
            let a = bvh.ray(&m, o, d);
            let b = brute_ray(&m, o, d);
            match (a, b) {
                (Some((fa, ta)), Some((fb, tb))) => {
                    assert!(fa == fb || (ta - tb).abs() < 1e-9, "face {fa} at {ta} vs {fb} at {tb}");
                    assert!((ta - tb).abs() < 1e-9);
                    hits += 1;
                }
                (None, None) => {}
                (a, b) => panic!("{a:?} vs {b:?}"),
            }
        }
        assert!(hits > 500, "{hits} of 1000 rays hit the ring");
        // A ray down the finger axis misses; the same ray from the side hits the outer wall first.
        assert!(bvh.ray(&m, [0.0, 0.0, 40.0], [0.0, 0.0, -1.0]).is_none());
        let (face, t) = bvh.ray(&m, [0.0, -40.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
        let (a, b, c) = m.triangle(&m.faces[face]).unwrap();
        let y = (a[1] + b[1] + c[1]) / 3.0;
        assert!((t - 40.0 - y).abs() < 0.2, "t {t} face at y {y}");
    }

    #[test]
    fn nearest_agrees_with_the_brute_force_over_200_points() {
        let m = ring(192, 96);
        let bvh = Bvh::build(&m);
        let mut rng = Lcg(11);
        let (lo, hi) = m.bounds().unwrap();
        let mut found = 0;
        for i in 0..200 {
            let p: [f64; 3] = if i % 2 == 0 {
                [rng.range(lo.0 as f64 - 2.0, hi.0 as f64 + 2.0), rng.range(lo.1 as f64 - 2.0, hi.1 as f64 + 2.0), rng.range(lo.2 as f64 - 2.0, hi.2 as f64 + 2.0)]
            } else {
                let v = m.vertices[(rng.next() * m.vertices.len() as f64) as usize % m.vertices.len()];
                [v.0 as f64 + rng.range(-0.3, 0.3), v.1 as f64 + rng.range(-0.3, 0.3), v.2 as f64 + rng.range(-0.3, 0.3)]
            };
            let max = if i % 4 == 3 { 0.5 } else { 5.0 };
            let a = bvh.nearest(&m, p, max);
            let b = brute_nearest(&m, p, max);
            match (a, b) {
                (Some((fa, qa)), Some((fb, qb))) => {
                    assert!(fa == fb || dist2(qa, qb) < 1e-18, "face {fa} at {qa:?} vs {fb} at {qb:?}");
                    assert!((dist2(qa, p).sqrt() - dist2(qb, p).sqrt()).abs() < 1e-9);
                    found += 1;
                }
                (None, None) => {}
                (a, b) => panic!("{a:?} vs {b:?}"),
            }
        }
        assert!(found > 100, "{found} of 200 points within reach");
        assert!(bvh.nearest(&m, [0.0, 0.0, 0.0], 1.0).is_none(), "the finger hole is empty");
    }

    #[test]
    fn an_empty_mesh_and_a_bad_face_are_harmless() {
        let empty = Mesh::default();
        let bvh = Bvh::build(&empty);
        assert_eq!(bvh.faces(), 0);
        assert!(bvh.ray(&empty, [0.0; 3], [1.0, 0.0, 0.0]).is_none());
        assert!(bvh.nearest(&empty, [0.0; 3], 1.0).is_none());
        let mut m = ring(48, 24);
        m.faces.push([0, 1, u32::MAX]);
        let bvh = Bvh::build(&m);
        assert_eq!(bvh.faces(), m.faces.len() - 1);
        assert!(bvh.ray(&m, [0.0, -40.0, 0.0], [0.0, 1.0, 0.0]).is_some());
    }
}
