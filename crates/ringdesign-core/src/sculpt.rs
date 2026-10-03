//! Sculpted parts: a signed distance field meshed, decimated and polished into a closed solid a design carries as a
//! stored mesh.
//!
//! The chain a sculpt runs is [`tetra_mesh`] over the part's box, [`relax`], [`clean_decimate`] to a face budget,
//! then [`settle`] (sliver collapse, edge-flip polish and fold-corner smoothing, each kept only while the mesh does
//! not cross itself), and [`packed`] for the design file. A field is built from the primitives and blends here, and
//! meets the stock through a [`Stock`] distance field; a hollow under a part is a [`Heights`] map of the first air
//! over the bore, eroded by a ball of the wall's thickness, meshed the same way and kept by [`open_shells`] where it
//! opens into the finger hole. Every tool is deterministic: the same field and settings give the same mesh bit for
//! bit, run to run.
//!
//! Grown in the Bestiarium Fenrir example and moved here unchanged. A grid a caller sizes by its step is held to
//! [`MAX_GRID_POINTS`] by coarsening the step, the way `MAX_CELLS` caps a tiling.
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::cad::{SurfaceKind, stored::Packed};
use crate::csg::{self, P3, Solid};
use crate::mesh::{Mesh, Vec3};
use crate::skin::Sample;
use anyhow::{Result, ensure};
use std::collections::{HashMap, HashSet};

/// Most points one sampled grid holds: a [`tetra_mesh`] lattice, a [`Stock`] or a [`Heights`] map. A step that
/// would take more is coarsened until the grid fits; 2^25 `f32` values are 128 MB.
pub const MAX_GRID_POINTS: usize = 1 << 25;

/// A distance field over world millimetres: negative inside.
pub type Field<'a> = &'a (dyn Fn(P3) -> f64 + Sync);

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn lerp3(a: P3, b: P3, t: f64) -> P3 {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

/// `f` over `0..n` in order, across the pool under `parallel`.
fn par_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync + Send) -> Vec<T> {
    #[cfg(feature = "parallel")]
    return (0..n).into_par_iter().map(f).collect();
    #[cfg(not(feature = "parallel"))]
    (0..n).map(f).collect()
}

/// Points along each axis of a grid from `lo` to `hi` at `step`, with the step coarsened until the grid holds at most
/// `cap` points; at least two along each axis.
fn grid<const D: usize>(lo: [f64; D], hi: [f64; D], step: f64, cap: usize) -> (f64, [usize; D]) {
    let extent: [f64; D] = std::array::from_fn(|k| (hi[k] - lo[k]).max(0.0));
    let widest = extent.iter().copied().fold(0.0, f64::max);
    let mut step = if step.is_finite() && step > 0.0 { step } else { widest.max(1e-3) };
    loop {
        let n: [f64; D] = std::array::from_fn(|k| ((extent[k] / step).ceil() + 1.0).max(2.0));
        if n.iter().product::<f64>() <= cap as f64 {
            return (step, n.map(|x| x as usize));
        }
        step *= 1.25;
    }
}

// --- Primitives and blends -------------------------------------------------------------------------------------------

/// Polynomial smooth minimum of radius `k`: a union that rounds the crease where two fields meet.
pub fn smin(a: f64, b: f64, k: f64) -> f64 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Polynomial smooth maximum of radius `k`: an intersection, or with one field negated a cut, with its crease rounded.
pub fn smax(a: f64, b: f64, k: f64) -> f64 {
    -smin(-a, -b, k)
}

/// A Gaussian bump of 1 at `c`, `w` wide.
pub fn bell(x: f64, c: f64, w: f64) -> f64 {
    (-((x - c) / w).powi(2)).exp()
}

/// Approximate distance to an axis-aligned ellipsoid of semi-axes `r` at the origin.
pub fn ellipsoid(p: P3, r: P3) -> f64 {
    let k0 = len([p[0] / r[0], p[1] / r[1], p[2] / r[2]]);
    let k1 = len([p[0] / (r[0] * r[0]), p[1] / (r[1] * r[1]), p[2] / (r[2] * r[2])]);
    if k1 < 1e-12 { -r[0].min(r[1]).min(r[2]) } else { k0 * (k0 - 1.0) / k1 }
}

/// Exact distance to the rounded cone joining sphere `a` of radius `ra` to sphere `b` of radius `rb`.
pub fn round_cone(p: P3, a: P3, b: P3, ra: f64, rb: f64) -> f64 {
    let ba = sub(b, a);
    let l2 = dot(ba, ba).max(1e-12);
    let rr = ra - rb;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = sub(p, a);
    let y = dot(pa, ba);
    let z = y - l2;
    let x = sub(mul(pa, l2), mul(ba, y));
    let x2 = dot(x, x);
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - rb;
    }
    if y.signum() * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - ra;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - ra
}

/// Distance in a section plane to an isosceles trapezoid `r1` half-wide at `y = -he`, `r2` at `y = he`.
pub fn trapezoid(p: [f64; 2], r1: f64, r2: f64, he: f64) -> f64 {
    let (x, y) = (p[0].abs(), p[1]);
    let k1 = [r2, he];
    let k2 = [r2 - r1, 2.0 * he];
    let ca = [x - x.min(if y < 0.0 { r1 } else { r2 }), y.abs() - he];
    let t = (((k1[0] - x) * k2[0] + (k1[1] - y) * k2[1]) / (k2[0] * k2[0] + k2[1] * k2[1])).clamp(0.0, 1.0);
    let cb = [x - k1[0] + k2[0] * t, y - k1[1] + k2[1] * t];
    let s = if cb[0] < 0.0 && ca[1] < 0.0 { -1.0 } else { 1.0 };
    s * (ca[0] * ca[0] + ca[1] * ca[1]).min(cb[0] * cb[0] + cb[1] * cb[1]).sqrt()
}

/// `p` turned by `deg` in the plane of axes `i` and `j`.
pub fn turn(p: P3, i: usize, j: usize, deg: f64) -> P3 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut q = p;
    q[i] = c * p[i] - s * p[j];
    q[j] = s * p[i] + c * p[j];
    q
}

/// The field's gradient at `p` by central differences.
pub fn gradient(field: Field, p: P3) -> P3 {
    let e = 1e-3;
    std::array::from_fn(|k| {
        let (mut a, mut b) = (p, p);
        a[k] += e;
        b[k] -= e;
        (field(a) - field(b)) / (2.0 * e)
    })
}

// --- The stock -------------------------------------------------------------------------------------------------------

/// Signed distance to the bare stock's outer surface on a coarse grid, read trilinear: what a sculpt blends into so it
/// meets the ring tangentially.
pub struct Stock {
    lo: P3,
    step: f64,
    n: [usize; 3],
    g: Vec<f32>,
}

impl Stock {
    /// The distance from each point of a grid over `lo..hi` at `step` to the nearest of the `samples` `keep` holds
    /// (an atlas's, say), signed by that sample's normal, out to `reach_mm`; further off, `-reach_mm` where `inside`
    /// holds and `reach_mm` elsewhere. Samples without a normal are skipped.
    pub fn of(samples: &[Sample], keep: impl Fn(&Sample) -> bool, lo: P3, hi: P3, step: f64, reach_mm: f64, inside: impl Fn(P3) -> bool + Sync) -> Self {
        let cell = 0.4;
        let key = |p: P3| -> [i32; 3] { std::array::from_fn(|k| (p[k] / cell).floor() as i32) };
        let mut map: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        for (i, s) in samples.iter().enumerate() {
            if !keep(s) || dot(s.n, s.n) < 0.5 {
                continue;
            }
            map.entry(key(s.p)).or_default().push(i as u32);
        }
        let (step, n) = grid(lo, hi, step, MAX_GRID_POINTS);
        let reach = (reach_mm / cell).ceil() as i32;
        let g: Vec<f32> = par_map(n[0] * n[1] * n[2], |m| {
            let (i, j, k) = (m % n[0], (m / n[0]) % n[1], m / (n[0] * n[1]));
            let p = [lo[0] + i as f64 * step, lo[1] + j as f64 * step, lo[2] + k as f64 * step];
            let c = key(p);
            let mut best = (f64::MAX, 0u32);
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    for dz in -reach..=reach {
                        if let Some(list) = map.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                            for &s in list {
                                let d2 = dot(sub(p, samples[s as usize].p), sub(p, samples[s as usize].p));
                                if d2 < best.0 {
                                    best = (d2, s);
                                }
                            }
                        }
                    }
                }
            }
            if best.0.sqrt() <= reach_mm {
                let s = &samples[best.1 as usize];
                (best.0.sqrt() * dot(sub(p, s.p), s.n).signum()) as f32
            } else if inside(p) {
                -reach_mm as f32
            } else {
                reach_mm as f32
            }
        });
        Self { lo, step, n, g }
    }

    /// The signed distance at `p`, clamped to the grid.
    pub fn at(&self, p: P3) -> f64 {
        let f: [f64; 3] = std::array::from_fn(|k| ((p[k] - self.lo[k]) / self.step).clamp(0.0, (self.n[k] - 2) as f64));
        let i: [usize; 3] = std::array::from_fn(|k| f[k].floor() as usize);
        let t: [f64; 3] = std::array::from_fn(|k| f[k] - i[k] as f64);
        let g = |a: usize, b: usize, c: usize| self.g[((i[2] + c) * self.n[1] + i[1] + b) * self.n[0] + i[0] + a] as f64;
        let x00 = lerp(g(0, 0, 0), g(1, 0, 0), t[0]);
        let x10 = lerp(g(0, 1, 0), g(1, 1, 0), t[0]);
        let x01 = lerp(g(0, 0, 1), g(1, 0, 1), t[0]);
        let x11 = lerp(g(0, 1, 1), g(1, 1, 1), t[0]);
        lerp(lerp(x00, x10, t[1]), lerp(x01, x11, t[1]), t[2])
    }
}

// --- Meshing ---------------------------------------------------------------------------------------------------------

/// A closed 2-manifold from a sampled distance field by marching tetrahedra on the Kuhn split of each cube, the field
/// sampled only in blocks a coarse pass finds within reach of the surface. The grid's border reads as outside, so the
/// result closes even where the field runs off the box; a step that would take more than [`MAX_GRID_POINTS`] is
/// coarsened.
pub fn tetra_mesh(lo: P3, hi: P3, step: f64, field: Field) -> Solid {
    tetra_mesh_capped(lo, hi, step, field, MAX_GRID_POINTS)
}

fn tetra_mesh_capped(lo: P3, hi: P3, step: f64, field: Field, cap: usize) -> Solid {
    let (step, n) = grid(lo, hi, step, cap);
    let at = |i: usize, j: usize, k: usize| -> P3 { [lo[0] + i as f64 * step, lo[1] + j as f64 * step, lo[2] + k as f64 * step] };
    const B: usize = 8;
    let nb: [usize; 3] = std::array::from_fn(|k| (n[k] - 1).div_ceil(B));
    let reach = 2.5 * step * B as f64 * 3f64.sqrt();
    let live: Vec<bool> = par_map(nb[0] * nb[1] * nb[2], |m| {
        let (bi, bj, bk) = (m % nb[0], (m / nb[0]) % nb[1], m / (nb[0] * nb[1]));
        let c = at(bi * B + B / 2, bj * B + B / 2, bk * B + B / 2);
        field(c).abs() < reach
    });
    let block = |i: usize, j: usize, k: usize| live[((k / B).min(nb[2] - 1) * nb[1] + (j / B).min(nb[1] - 1)) * nb[0] + (i / B).min(nb[0] - 1)];
    let slab = |k: usize, out: &mut [f32]| {
        for j in 0..n[1] {
            for i in 0..n[0] {
                let border = i == 0 || j == 0 || k == 0 || i == n[0] - 1 || j == n[1] - 1 || k == n[2] - 1;
                let near = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 1, 0), (1, 0, 1), (0, 1, 1), (1, 1, 1)]
                    .iter()
                    .any(|&(a, b, c)| block(i.saturating_sub(a), j.saturating_sub(b), k.saturating_sub(c)));
                let v = if border { 1.0 } else if near { field(at(i, j, k)) as f32 } else { 1.0 };
                out[j * n[0] + i] = if v.abs() < 1e-5 { 1e-5 } else { v };
            }
        }
    };
    // The lattice is allocated here on the calling thread and filled a slab per pool task.
    let mut values = vec![1.0f32; n[0] * n[1] * n[2]];
    #[cfg(feature = "parallel")]
    values.par_chunks_mut(n[0] * n[1]).enumerate().for_each(|(k, out)| slab(k, out));
    #[cfg(not(feature = "parallel"))]
    values.chunks_mut(n[0] * n[1]).enumerate().for_each(|(k, out)| slab(k, out));
    let idx = |i: usize, j: usize, k: usize| (k * n[1] + j) * n[0] + i;
    const TETS: [[usize; 4]; 6] = [[0, 1, 3, 7], [0, 1, 5, 7], [0, 2, 3, 7], [0, 2, 6, 7], [0, 4, 5, 7], [0, 4, 6, 7]];
    let mut index: HashMap<(usize, usize), u32> = HashMap::new();
    let mut v: Vec<P3> = Vec::new();
    let mut f: Vec<[u32; 3]> = Vec::new();
    for k in 0..n[2] - 1 {
        for j in 0..n[1] - 1 {
            for i in 0..n[0] - 1 {
                let g: [usize; 8] = std::array::from_fn(|c| idx(i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1)));
                let c: [f32; 8] = std::array::from_fn(|m| values[g[m]]);
                let inside = c.iter().filter(|x| **x < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let pos = |m: usize| at(i + (m & 1), j + ((m >> 1) & 1), k + ((m >> 2) & 1));
                for tet in TETS {
                    let ins: Vec<usize> = tet.iter().copied().filter(|m| c[*m] < 0.0).collect();
                    let outs: Vec<usize> = tet.iter().copied().filter(|m| c[*m] >= 0.0).collect();
                    if ins.is_empty() || outs.is_empty() {
                        continue;
                    }
                    let mut vert = |a: usize, b: usize| -> u32 {
                        let key = (g[a].min(g[b]), g[a].max(g[b]));
                        *index.entry(key).or_insert_with(|| {
                            let t = ((c[a] / (c[a] - c[b])) as f64).clamp(0.02, 0.98);
                            v.push(lerp3(pos(a), pos(b), t));
                            (v.len() - 1) as u32
                        })
                    };
                    let mut tris: Vec<[u32; 3]> = Vec::new();
                    match (ins.len(), outs.len()) {
                        (1, 3) => tris.push([vert(ins[0], outs[0]), vert(ins[0], outs[1]), vert(ins[0], outs[2])]),
                        (3, 1) => tris.push([vert(ins[0], outs[0]), vert(ins[1], outs[0]), vert(ins[2], outs[0])]),
                        _ => {
                            let (a, b, c2, d) = (vert(ins[0], outs[0]), vert(ins[0], outs[1]), vert(ins[1], outs[1]), vert(ins[1], outs[0]));
                            tris.push([a, b, c2]);
                            tris.push([a, c2, d]);
                        }
                    }
                    let inner = ins.iter().fold([0.0; 3], |s, m| add(s, pos(*m)));
                    let outer = outs.iter().fold([0.0; 3], |s, m| add(s, pos(*m)));
                    let dir = sub(mul(outer, 1.0 / outs.len() as f64), mul(inner, 1.0 / ins.len() as f64));
                    for t in tris {
                        let [p, q, r] = t.map(|x| v[x as usize]);
                        let nrm = cross(sub(q, p), sub(r, p));
                        f.push(if dot(nrm, dir) >= 0.0 { t } else { [t[0], t[2], t[1]] });
                    }
                }
            }
        }
    }
    Solid { v, f }
}

/// Relax each vertex toward its neighbours' centroid within its tangent plane, then step it back onto the surface.
/// Vertices where the surface turns sharply stay put, no vertex moves more than a third of its mean edge, and a
/// triangle that turns over puts its corners back.
pub fn relax(mesh: &mut Solid, field: Field, rounds: usize) {
    let mut ring: Vec<Vec<u32>> = vec![Vec::new(); mesh.v.len()];
    for t in &mesh.f {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            ring[a as usize].push(b);
            ring[b as usize].push(a);
        }
    }
    for r in &mut ring {
        r.sort_unstable();
        r.dedup();
    }
    let unit = |g: P3| mul(g, 1.0 / len(g).max(1e-12));
    for _ in 0..rounds {
        let normals: Vec<P3> = par_map(mesh.v.len(), |i| unit(gradient(field, mesh.v[i])));
        let before = mesh.v.clone();
        let next: Vec<P3> = par_map(mesh.v.len(), |i| {
            let p = before[i];
            let n = normals[i];
            if ring[i].iter().any(|j| dot(normals[*j as usize], n) < 0.87) {
                return p;
            }
            let shortest = ring[i].iter().map(|j| len(sub(before[*j as usize], p))).sum::<f64>() / ring[i].len().max(1) as f64;
            let c = ring[i].iter().fold([0.0; 3], |s, j| add(s, before[*j as usize]));
            let c = mul(c, 1.0 / ring[i].len().max(1) as f64);
            let dv = sub(c, p);
            let dv = mul(sub(dv, mul(n, dot(dv, n))), 0.5);
            let cap = shortest / 3.0;
            let dv = if len(dv) > cap { mul(dv, cap / len(dv)) } else { dv };
            let mut q = add(p, dv);
            for _ in 0..2 {
                let f = field(q);
                let g = gradient(field, q);
                let step = mul(g, f / dot(g, g).max(1e-9));
                if len(step) > cap {
                    return p;
                }
                q = sub(q, step);
            }
            q
        });
        mesh.v = next;
        let mut undo = vec![false; mesh.v.len()];
        for t in &mesh.f {
            let [a, b, c] = t.map(|x| mesh.v[x as usize]);
            let nf = cross(sub(b, a), sub(c, a));
            let nv = t.iter().fold([0.0; 3], |s, x| add(s, normals[*x as usize]));
            if dot(nf, nv) <= 0.3 * len(nf) * len(nv) {
                for &x in t {
                    undo[x as usize] = true;
                }
            }
        }
        for (i, u) in undo.iter().enumerate() {
            if *u {
                mesh.v[i] = before[i];
            }
        }
    }
}

/// [`relax`], then the unrelaxed positions put back round each crossing it made, within 0.3, 0.6 and 1.2 mm of it and
/// then everywhere, until none is left: a relax that folds a thin crease through itself is undone only there. Returns
/// how many vertices were put back.
pub fn relax_clean(mesh: &mut Solid, field: Field, rounds: usize) -> usize {
    let before = mesh.v.clone();
    relax(mesh, field, rounds);
    let mut restored = 0;
    for reach in [0.3, 0.6, 1.2, f64::INFINITY] {
        let sites = crossing_sites(mesh);
        if sites.is_empty() {
            break;
        }
        for (v, orig) in mesh.v.iter_mut().zip(&before) {
            if *v != *orig && sites.iter().any(|s| len(sub(*v, *s)) < reach) {
                *v = *orig;
                restored += 1;
            }
        }
    }
    restored
}

// --- Decimation ------------------------------------------------------------------------------------------------------

/// Quadric edge-collapse decimation of a closed manifold to `target` faces, never past `max_cost` or a collapse that
/// breaks the link condition, turns a face more than `max_turn_deg`, leaves one sharper than `min_deg`, or folds an
/// edge round it past `max_fold_deg` that did not already.
pub fn decimate(mesh: &Solid, target: usize, max_cost: f64, min_deg: f64, max_turn_deg: f64, max_fold_deg: f64) -> Solid {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;
    #[derive(PartialEq)]
    struct Item(f64, u32, u32, u32, u32);
    impl Eq for Item {}
    impl PartialOrd for Item {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Item {
        fn cmp(&self, o: &Self) -> Ordering {
            o.0.total_cmp(&self.0)
        }
    }
    let mut pos = mesh.v.clone();
    let mut faces = mesh.f.clone();
    let mut alive = vec![true; faces.len()];
    let mut vfaces: Vec<Vec<u32>> = vec![Vec::new(); pos.len()];
    for (i, t) in faces.iter().enumerate() {
        for &x in t {
            vfaces[x as usize].push(i as u32);
        }
    }
    let plane = |t: &[u32; 3], pos: &[P3]| -> Option<[f64; 4]> {
        let [a, b, c] = t.map(|x| pos[x as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let l = len(n);
        (l > 1e-14).then(|| {
            let n = mul(n, 1.0 / l);
            [n[0], n[1], n[2], -dot(n, a)]
        })
    };
    let mut quad = vec![[0.0f64; 10]; pos.len()];
    let add_q = |q: &mut [f64; 10], p: [f64; 4]| {
        let [a, b, c, d] = p;
        let terms = [a * a, a * b, a * c, a * d, b * b, b * c, b * d, c * c, c * d, d * d];
        for k in 0..10 {
            q[k] += terms[k];
        }
    };
    for t in &faces {
        if let Some(p) = plane(t, &pos) {
            for &x in t {
                add_q(&mut quad[x as usize], p);
            }
        }
    }
    let err = |q: &[f64; 10], p: P3| {
        let [x, y, z] = p;
        q[0] * x * x + 2.0 * q[1] * x * y + 2.0 * q[2] * x * z + 2.0 * q[3] * x + q[4] * y * y + 2.0 * q[5] * y * z + 2.0 * q[6] * y + q[7] * z * z + 2.0 * q[8] * z + q[9]
    };
    let best = |q: &[f64; 10], a: P3, b: P3| -> (P3, f64) {
        let m = [[q[0], q[1], q[2]], [q[1], q[4], q[5]], [q[2], q[5], q[7]]];
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        let mut cands = vec![a, b, mul(add(a, b), 0.5)];
        if det.abs() > 1e-9 {
            let r = [-q[3], -q[6], -q[8]];
            let solve = |col: usize| {
                let mut mm = m;
                for row in 0..3 {
                    mm[row][col] = r[row];
                }
                (mm[0][0] * (mm[1][1] * mm[2][2] - mm[1][2] * mm[2][1]) - mm[0][1] * (mm[1][0] * mm[2][2] - mm[1][2] * mm[2][0]) + mm[0][2] * (mm[1][0] * mm[2][1] - mm[1][1] * mm[2][0])) / det
            };
            let p = [solve(0), solve(1), solve(2)];
            let mid = mul(add(a, b), 0.5);
            if len(sub(p, mid)) < len(sub(a, b)) {
                cands.push(p);
            }
        }
        cands.into_iter().map(|p| (p, err(q, p))).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap()
    };
    let mut version = vec![0u32; pos.len()];
    let mut heap = BinaryHeap::new();
    let push_edges = |v0: u32, heap: &mut BinaryHeap<Item>, faces: &[[u32; 3]], alive: &[bool], vfaces: &[Vec<u32>], pos: &[P3], quad: &[[f64; 10]], version: &[u32]| {
        let mut seen = HashSet::new();
        for &fi in &vfaces[v0 as usize] {
            if !alive[fi as usize] {
                continue;
            }
            for &w in &faces[fi as usize] {
                if w != v0 && seen.insert(w) {
                    let mut q = quad[v0 as usize];
                    for k in 0..10 {
                        q[k] += quad[w as usize][k];
                    }
                    let (_, cost) = best(&q, pos[v0 as usize], pos[w as usize]);
                    heap.push(Item(cost, v0, w, version[v0 as usize], version[w as usize]));
                }
            }
        }
    };
    for v0 in 0..pos.len() as u32 {
        push_edges(v0, &mut heap, &faces, &alive, &vfaces, &pos, &quad, &version);
    }
    let mut count = faces.len();
    let min_cos = min_deg.to_radians().cos();
    let turn_cos = max_turn_deg.to_radians().cos();
    while count > target {
        let Some(Item(cost, a, b, va, vb)) = heap.pop() else { break };
        if cost > max_cost {
            break;
        }
        if version[a as usize] != va || version[b as usize] != vb {
            continue;
        }
        let fa: Vec<u32> = vfaces[a as usize].iter().copied().filter(|f| alive[*f as usize]).collect();
        let fb: Vec<u32> = vfaces[b as usize].iter().copied().filter(|f| alive[*f as usize]).collect();
        let shared: Vec<u32> = fa.iter().copied().filter(|f| faces[*f as usize].contains(&b)).collect();
        if shared.len() != 2 {
            continue;
        }
        let ring = |fs: &[u32], me: u32| -> HashSet<u32> { fs.iter().flat_map(|f| faces[*f as usize]).filter(|x| *x != me).collect() };
        let (na, nb) = (ring(&fa, a), ring(&fb, b));
        let common: Vec<u32> = na.intersection(&nb).copied().collect();
        let opposite: HashSet<u32> = shared.iter().flat_map(|f| faces[*f as usize]).filter(|x| *x != a && *x != b).collect();
        if common.len() != 2 || !common.iter().all(|x| opposite.contains(x)) {
            continue;
        }
        let mut q = quad[a as usize];
        for k in 0..10 {
            q[k] += quad[b as usize][k];
        }
        let (p, _) = best(&q, pos[a as usize], pos[b as usize]);
        let mut ok = true;
        for &fi in fa.iter().chain(&fb) {
            if shared.contains(&fi) {
                continue;
            }
            let t = faces[fi as usize];
            let old = t.map(|x| pos[x as usize]);
            let new = t.map(|x| if x == a || x == b { p } else { pos[x as usize] });
            let (n0, n1) = (cross(sub(old[1], old[0]), sub(old[2], old[0])), cross(sub(new[1], new[0]), sub(new[2], new[0])));
            if len(n1) < 1e-10 || dot(n0, n1) <= turn_cos * len(n0) * len(n1) {
                ok = false;
                break;
            }
            for e in 0..3 {
                let (u, w) = (sub(new[(e + 1) % 3], new[e]), sub(new[(e + 2) % 3], new[e]));
                if dot(u, w) > min_cos * len(u) * len(w) {
                    ok = false;
                }
            }
            if !ok {
                break;
            }
        }
        if !ok {
            continue;
        }
        // No edge round the collapse may fold past `max_fold_deg` unless it already did.
        let fold = {
            let near: HashSet<u32> = na.union(&nb).copied().chain([a, b]).flat_map(|x| vfaces[x as usize].iter().copied()).filter(|f| alive[*f as usize]).collect();
            let worst = |after: bool| -> f64 {
                let mut edges: HashMap<(u32, u32), Vec<P3>> = HashMap::new();
                for &fi in &near {
                    if after && shared.contains(&fi) {
                        continue;
                    }
                    let t = faces[fi as usize].map(|x| if after && x == b { a } else { x });
                    let ps = t.map(|x| if after && x == a { p } else { pos[x as usize] });
                    let n = cross(sub(ps[1], ps[0]), sub(ps[2], ps[0]));
                    let n = mul(n, 1.0 / len(n).max(1e-30));
                    let touches = |x: u32| x == a || (!after && x == b);
                    for e in 0..3 {
                        let (u, w) = (t[e], t[(e + 1) % 3]);
                        if touches(u) || touches(w) || touches(t[(e + 2) % 3]) {
                            edges.entry((u.min(w), u.max(w))).or_default().push(n);
                        }
                    }
                }
                edges.values().filter(|ns| ns.len() == 2).map(|ns| dot(ns[0], ns[1]).clamp(-1.0, 1.0).acos().to_degrees()).fold(0.0, f64::max)
            };
            let after = worst(true);
            after > max_fold_deg && after > worst(false) + 1.0
        };
        if fold {
            continue;
        }
        pos[a as usize] = p;
        quad[a as usize] = q;
        for &fi in &shared {
            alive[fi as usize] = false;
            count -= 1;
        }
        for &fi in &fb {
            if alive[fi as usize] {
                for x in faces[fi as usize].iter_mut() {
                    if *x == b {
                        *x = a;
                    }
                }
                vfaces[a as usize].push(fi);
            }
        }
        vfaces[b as usize].clear();
        vfaces[a as usize].retain(|f| alive[*f as usize]);
        vfaces[a as usize].sort_unstable();
        vfaces[a as usize].dedup();
        version[a as usize] += 1;
        version[b as usize] += 1;
        push_edges(a, &mut heap, &faces, &alive, &vfaces, &pos, &quad, &version);
    }
    let mut remap = vec![u32::MAX; pos.len()];
    let mut v = Vec::new();
    let mut f = Vec::new();
    for (i, t) in faces.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        f.push(t.map(|x| {
            if remap[x as usize] == u32::MAX {
                remap[x as usize] = v.len() as u32;
                v.push(pos[x as usize]);
            }
            remap[x as usize]
        }));
    }
    Solid { v, f }
}

/// [`decimate`] to `target` faces, backing off toward the raw mesh until the result does not cross itself: four ever
/// gentler tries, then the raw mesh compacted.
pub fn clean_decimate(raw: &Solid, target: usize) -> Solid {
    for (k, cap) in [2e-3, 1e-3, 5e-4, 2e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 20_000 * k, cap, 2.0 + k as f64, 18.0, 35.0);
        let crossings = csg::self_crossings(&nets);
        if crossings == 0 {
            return nets;
        }
        log::debug!("decimation {k}: {crossings} crossings, backing off");
    }
    decimate(raw, raw.f.len(), 0.0, 0.0, 0.0, 180.0)
}

/// [`clean_decimate`], or where its first try crossed itself when no try is clean, so a caller can mend the field there
/// rather than fall back to the raw mesh.
pub fn clean_decimate_or_sites(raw: &Solid, target: usize) -> std::result::Result<Solid, Vec<P3>> {
    let mut first = None;
    for (k, cap) in [2e-3, 1e-3, 5e-4, 2e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 20_000 * k, cap, 2.0 + k as f64, 18.0, 35.0);
        if csg::self_crossings(&nets) == 0 {
            return Ok(nets);
        }
        if k == 0 {
            first = Some(crossing_sites(&nets));
        }
    }
    Err(first.unwrap_or_default())
}

// --- Polish ----------------------------------------------------------------------------------------------------------

/// Two more rounds of [`relax`] on a decimated mesh, [`collapse_short`] slivers, [`polish`] the meshing's folds and
/// [`smooth_folds`] their corners, then fair the field's own creases wherever `fair_where` holds: each step kept only if
/// the mesh stays closed and does not cross itself.
pub fn settle(nets: Solid, field: Field, fair_where: &dyn Fn(P3) -> bool) -> Solid {
    let clean = |n: &Solid| csg::self_crossings(n) == 0;
    let mut relaxed = nets.clone();
    relax(&mut relaxed, field, 2);
    let nets = if clean(&relaxed) { relaxed } else { nets };
    let mut avoid: Vec<P3> = Vec::new();
    let mut nets = nets;
    for round in 0..3 {
        let mut shorter = nets.clone();
        let collapsed = collapse_short(&mut shorter, field, 0.02, &avoid);
        let (bad, _) = closure(&shorter);
        let crossings = csg::self_crossings(&shorter);
        log::debug!("sliver collapse {round}: {collapsed} edges, {bad} open edges, {crossings} crossings");
        if bad == 0 && crossings == 0 {
            nets = shorter;
            break;
        }
        // Read at `f32`, as a built mesh carries it.
        let rounded = Solid { v: shorter.v.iter().map(|p| p.map(|c| c as f32 as f64)).collect(), f: shorter.f.clone() };
        avoid.extend(crossing_sites(&rounded));
    }
    let mut flipped = nets.clone();
    let n = polish(&mut flipped, field, 6);
    let (bad, _) = closure(&flipped);
    let nets = if bad == 0 && clean(&flipped) { flipped } else { nets };
    let mut smoothed = nets.clone();
    let m = smooth_folds(&mut smoothed, field, 16);
    let nets = if clean(&smoothed) { smoothed } else { nets };
    let mut faired = nets.clone();
    let k = smooth_folds_by(&mut faired, field, 24, 55.0, Some((0.05, fair_where))) + polish_where(&mut faired, field, 4, fair_where);
    if clean(&faired) {
        log::debug!("flipped {n} edges, moved {m} fold corners, faired {k}");
        faired
    } else {
        log::debug!("flipped {n} edges, moved {m} fold corners; fairing crossed itself and was dropped");
        nets
    }
}

/// Moves the corners of the meshing's remaining folds toward their neighbours on the surface: a move is kept when no
/// face round the corner turns against the field and the worst fold round it shrinks.
pub fn smooth_folds(nets: &mut Solid, field: Field, passes: usize) -> usize {
    smooth_folds_by(nets, field, passes, 35.0, None)
}

/// [`smooth_folds`] over edges turning `min_deg` or more; with `off_field` a corner where its test holds may leave the
/// field by up to its distance instead of being stepped back onto it, which fairs the field's own sub-tenth creases.
pub fn smooth_folds_by(nets: &mut Solid, field: Field, passes: usize, min_deg: f64, off_field: Option<(f64, &dyn Fn(P3) -> bool)>) -> usize {
    let unit = |g: P3| mul(g, 1.0 / len(g).max(1e-12));
    let mut vfaces: Vec<Vec<u32>> = vec![Vec::new(); nets.v.len()];
    for (i, t) in nets.f.iter().enumerate() {
        for &x in t {
            vfaces[x as usize].push(i as u32);
        }
    }
    let face_n = |t: &[u32; 3], v: &[P3]| unit(cross(sub(v[t[1] as usize], v[t[0] as usize]), sub(v[t[2] as usize], v[t[0] as usize])));
    // The worst dihedral cosine over edges touching `x`, faces taken from its two rings.
    let worst_round = |x: u32, v: &[P3], faces: &[[u32; 3]], vfaces: &[Vec<u32>]| -> f64 {
        let ring: HashSet<u32> = vfaces[x as usize].iter().flat_map(|f| faces[*f as usize]).collect();
        let near: HashSet<u32> = ring.iter().flat_map(|y| vfaces[*y as usize].iter().copied()).collect();
        let mut m: HashMap<(u32, u32), Vec<P3>> = HashMap::new();
        for f in &near {
            let t = faces[*f as usize];
            let n = face_n(&t, v);
            for e in 0..3 {
                let (p, q) = (t[e], t[(e + 1) % 3]);
                if ring.contains(&p) && ring.contains(&q) {
                    m.entry((p.min(q), p.max(q))).or_default().push(n);
                }
            }
        }
        m.values().filter(|ns| ns.len() == 2).map(|ns| dot(ns[0], ns[1])).fold(1.0, f64::min)
    };
    let mut moved = 0;
    for _ in 0..passes {
        let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, t) in nets.f.iter().enumerate() {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let mut corners: Vec<u32> = edges
            .iter()
            .filter(|(_, fs)| fs.len() == 2 && dot(face_n(&nets.f[fs[0]], &nets.v), face_n(&nets.f[fs[1]], &nets.v)) < min_deg.to_radians().cos())
            .flat_map(|(_, fs)| nets.f[fs[0]].into_iter().chain(nets.f[fs[1]]))
            .collect();
        corners.sort_unstable();
        corners.dedup();
        let mut changed = 0;
        for x in corners {
            let p = nets.v[x as usize];
            if off_field.is_some_and(|(_, inside)| !inside(p)) {
                continue;
            }
            let n = unit(gradient(field, p));
            // Sorted, so the centroid sums in the same order every run.
            let mut ring: Vec<u32> = vfaces[x as usize].iter().flat_map(|f| nets.f[*f as usize]).filter(|y| *y != x).collect();
            ring.sort_unstable();
            ring.dedup();
            if ring.is_empty() {
                continue;
            }
            let c = mul(ring.iter().fold([0.0; 3], |s, y| add(s, nets.v[*y as usize])), 1.0 / ring.len() as f64);
            let dv = sub(c, p);
            let mut q = add(p, sub(dv, mul(n, dot(dv, n))));
            match off_field {
                Some((limit, _)) => {
                    let full = c;
                    q = if len(sub(full, p)) > limit { add(p, mul(sub(full, p), limit / len(sub(full, p)))) } else { full };
                }
                None => {
                    for _ in 0..2 {
                        let g = gradient(field, q);
                        q = sub(q, mul(g, field(q) / dot(g, g).max(1e-9)));
                    }
                }
            }
            let before = worst_round(x, &nets.v, &nets.f, &vfaces);
            let old = nets.v[x as usize];
            nets.v[x as usize] = q;
            let flipped = vfaces[x as usize].iter().any(|f| dot(face_n(&nets.f[*f as usize], &nets.v), n) < 0.2);
            if flipped || worst_round(x, &nets.v, &nets.f, &vfaces) <= before + 1e-6 {
                nets.v[x as usize] = old;
            } else {
                changed += 1;
            }
        }
        moved += changed;
        if changed == 0 {
            break;
        }
    }
    moved
}

/// Collapses edges shorter than `min_len`, none within 0.4 mm of a point in `avoid`, onto their midpoints stepped back
/// onto the field, where the collapse keeps the mesh a manifold and turns no face round it over. The count collapsed.
pub fn collapse_short(nets: &mut Solid, field: Field, min_len: f64, avoid: &[P3]) -> usize {
    let mut total = 0;
    for _ in 0..6 {
        let mut vfaces: Vec<Vec<u32>> = vec![Vec::new(); nets.v.len()];
        for (i, t) in nets.f.iter().enumerate() {
            for &x in t {
                vfaces[x as usize].push(i as u32);
            }
        }
        let mut short: Vec<(f64, u32, u32)> = Vec::new();
        for t in &nets.f {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                let l = len(sub(nets.v[a as usize], nets.v[b as usize]));
                let mid = mul(add(nets.v[a as usize], nets.v[b as usize]), 0.5);
                if a < b && l < min_len && avoid.iter().all(|c| len(sub(*c, mid)) > 0.4) {
                    short.push((l, a, b));
                }
            }
        }
        short.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut dead = vec![false; nets.f.len()];
        let mut busy = vec![false; nets.v.len()];
        let mut done = 0;
        for (_, a, b) in short {
            if busy[a as usize] || busy[b as usize] {
                continue;
            }
            let fa: Vec<u32> = vfaces[a as usize].iter().copied().filter(|f| !dead[*f as usize]).collect();
            let fb: Vec<u32> = vfaces[b as usize].iter().copied().filter(|f| !dead[*f as usize]).collect();
            let shared: Vec<u32> = fa.iter().copied().filter(|f| nets.f[*f as usize].contains(&b)).collect();
            if shared.len() != 2 {
                continue;
            }
            let ring = |fs: &[u32], me: u32| -> HashSet<u32> { fs.iter().flat_map(|f| nets.f[*f as usize]).filter(|x| *x != me).collect() };
            let (na, nb) = (ring(&fa, a), ring(&fb, b));
            if na.intersection(&nb).count() != 2 {
                continue;
            }
            let mut p = mul(add(nets.v[a as usize], nets.v[b as usize]), 0.5);
            for _ in 0..2 {
                let g = gradient(field, p);
                p = sub(p, mul(g, field(p) / dot(g, g).max(1e-9)));
            }
            let ok = fa.iter().chain(&fb).filter(|f| !shared.contains(f)).all(|f| {
                let t = nets.f[*f as usize];
                let old = t.map(|x| nets.v[x as usize]);
                let new = t.map(|x| if x == a || x == b { p } else { nets.v[x as usize] });
                let (n0, n1) = (cross(sub(old[1], old[0]), sub(old[2], old[0])), cross(sub(new[1], new[0]), sub(new[2], new[0])));
                len(n1) > 1e-14 && dot(n0, n1) > 0.5 * len(n0) * len(n1)
            });
            if !ok {
                continue;
            }
            nets.v[a as usize] = p;
            for f in &shared {
                dead[*f as usize] = true;
            }
            for f in &fb {
                if !dead[*f as usize] {
                    for x in nets.f[*f as usize].iter_mut() {
                        if *x == b {
                            *x = a;
                        }
                    }
                }
            }
            for x in na.iter().chain(nb.iter()).chain([a, b].iter()) {
                busy[*x as usize] = true;
            }
            done += 1;
        }
        let mut keep = 0;
        for i in 0..nets.f.len() {
            if !dead[i] {
                nets.f[keep] = nets.f[i];
                keep += 1;
            }
        }
        nets.f.truncate(keep);
        total += done;
        if done == 0 {
            break;
        }
    }
    // Drop vertices no face uses.
    let mut remap = vec![u32::MAX; nets.v.len()];
    let mut v = Vec::new();
    for t in nets.f.iter_mut() {
        for x in t.iter_mut() {
            if remap[*x as usize] == u32::MAX {
                remap[*x as usize] = v.len() as u32;
                v.push(nets.v[*x as usize]);
            }
            *x = remap[*x as usize];
        }
    }
    nets.v = v;
    total
}

/// Flips and smooths away folds the meshing made on a smooth stretch of the field: an edge turning 40 degrees or more
/// whose two faces the field itself sees within 20 degrees of each other. Each change is kept only if no face round it
/// turns over and the worst fold round it shrinks. The count flipped.
pub fn polish(nets: &mut Solid, field: Field, passes: usize) -> usize {
    polish_where(nets, field, passes, &|_| false)
}

/// [`polish`], also flipping folds of 55 degrees or more the field itself makes wherever `anywhere` holds.
pub fn polish_where(nets: &mut Solid, field: Field, passes: usize, anywhere: &dyn Fn(P3) -> bool) -> usize {
    let unit = |g: P3| mul(g, 1.0 / len(g).max(1e-12));
    let normal = |t: &[u32; 3], v: &[P3]| unit(cross(sub(v[t[1] as usize], v[t[0] as usize]), sub(v[t[2] as usize], v[t[0] as usize])));
    let centre = |t: &[u32; 3], v: &[P3]| mul(add(add(v[t[0] as usize], v[t[1] as usize]), v[t[2] as usize]), 1.0 / 3.0);
    let mut fixed = 0;
    for _ in 0..passes {
        let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, t) in nets.f.iter().enumerate() {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        let mut touched = vec![false; nets.f.len()];
        let mut bad: Vec<((u32, u32), usize, usize)> = edges
            .iter()
            .filter(|(_, fs)| fs.len() == 2)
            .filter_map(|(k, fs)| {
                let (n0, n1) = (normal(&nets.f[fs[0]], &nets.v), normal(&nets.f[fs[1]], &nets.v));
                if dot(n0, n1) > 40f64.to_radians().cos() {
                    return None;
                }
                let (g0, g1) = (unit(gradient(field, centre(&nets.f[fs[0]], &nets.v))), unit(gradient(field, centre(&nets.f[fs[1]], &nets.v))));
                let mid = mul(add(nets.v[k.0 as usize], nets.v[k.1 as usize]), 0.5);
                let own = anywhere(mid) && dot(n0, n1) < 55f64.to_radians().cos();
                (own || dot(g0, g1) > 20f64.to_radians().cos()).then_some((*k, fs[0], fs[1]))
            })
            .collect();
        bad.sort_unstable();
        let mut changed = 0;
        for ((a, b), f0, f1) in bad {
            if touched[f0] || touched[f1] {
                continue;
            }
            let (t0, t1) = (nets.f[f0], nets.f[f1]);
            // Orient so t0 runs a -> b.
            let (a, b) = if (0..3).any(|e| t0[e] == a && t0[(e + 1) % 3] == b) { (a, b) } else { (b, a) };
            let c = *t0.iter().find(|x| **x != a && **x != b).unwrap();
            let d = *t1.iter().find(|x| **x != a && **x != b).unwrap();
            if c == d || edges.contains_key(&(c.min(d), c.max(d))) {
                continue;
            }
            let worst = |faces: &[[u32; 3]]| -> f64 {
                let mut m: HashMap<(u32, u32), Vec<P3>> = HashMap::new();
                for t in faces {
                    let n = normal(t, &nets.v);
                    for e in 0..3 {
                        let (p, q) = (t[e], t[(e + 1) % 3]);
                        m.entry((p.min(q), p.max(q))).or_default().push(n);
                    }
                }
                m.values().filter(|ns| ns.len() == 2).map(|ns| dot(ns[0], ns[1])).fold(1.0, f64::min)
            };
            // The ring of faces across the quad's four outer edges, which the flip's new faces must also sit well with.
            let mut ring: Vec<[u32; 3]> = Vec::new();
            for (p, q) in [(a, d), (d, b), (b, c), (c, a)] {
                if let Some(fs) = edges.get(&(p.min(q), p.max(q))) {
                    ring.extend(fs.iter().filter(|f| **f != f0 && **f != f1).map(|f| nets.f[*f]));
                }
            }
            let (n0, n1) = ([a, d, c], [d, b, c]);
            let g = unit(gradient(field, mul(add(nets.v[c as usize], nets.v[d as usize]), 0.5)));
            if dot(normal(&n0, &nets.v), g) < 0.3 || dot(normal(&n1, &nets.v), g) < 0.3 {
                continue;
            }
            let before: Vec<[u32; 3]> = [t0, t1].into_iter().chain(ring.iter().copied()).collect();
            let after: Vec<[u32; 3]> = [n0, n1].into_iter().chain(ring.iter().copied()).collect();
            if worst(&after) > worst(&before) + 1e-6 {
                nets.f[f0] = n0;
                nets.f[f1] = n1;
                touched[f0] = true;
                touched[f1] = true;
                changed += 1;
            }
        }
        fixed += changed;
        if changed == 0 {
            break;
        }
    }
    fixed
}

/// Points where one face of `s` pierces another, found on a 0.3 mm grid with exact orientation tests.
pub fn crossing_sites(s: &Solid) -> Vec<P3> {
    let side = |a: P3, b: P3, c: P3, d: P3| {
        let xyz = |p: P3| robust::Coord3D { x: p[0], y: p[1], z: p[2] };
        robust::orient3d(xyz(a), xyz(b), xyz(c), xyz(d))
    };
    let pierce = |p: P3, q: P3, tri: [P3; 3]| {
        let [a, b, c] = tri;
        let (sp, sq) = (side(a, b, c, p), side(a, b, c, q));
        if sp == 0.0 || sq == 0.0 || (sp > 0.0) == (sq > 0.0) {
            return None;
        }
        let signs = [side(p, q, a, b), side(p, q, b, c), side(p, q, c, a)];
        if signs.iter().all(|v| *v > 0.0) || signs.iter().all(|v| *v < 0.0) {
            let t = sp / (sp - sq);
            Some(std::array::from_fn(|k| p[k] + t * (q[k] - p[k])))
        } else {
            None
        }
    };
    let boxes: Vec<([i32; 3], [i32; 3])> = s
        .f
        .iter()
        .map(|f| {
            let lo: P3 = std::array::from_fn(|k| f.iter().map(|i| s.v[*i as usize][k]).fold(f64::MAX, f64::min));
            let hi: P3 = std::array::from_fn(|k| f.iter().map(|i| s.v[*i as usize][k]).fold(f64::MIN, f64::max));
            (lo.map(|v| (v / 0.3).floor() as i32), hi.map(|v| (v / 0.3).floor() as i32))
        })
        .collect();
    let mut cells: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    cells.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let mut stamp = vec![usize::MAX; s.f.len()];
    let mut points = Vec::new();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        let a = s.f[i].map(|k| s.v[k as usize]);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for &j in &cells[&[x, y, z]] {
                        if j <= i || stamp[j] == i || s.f[i].iter().any(|v| s.f[j].contains(v)) {
                            continue;
                        }
                        stamp[j] = i;
                        let b = s.f[j].map(|k| s.v[k as usize]);
                        if let Some(p) = (0..3).find_map(|k| pierce(a[k], a[(k + 1) % 3], b)).or_else(|| (0..3).find_map(|k| pierce(b[k], b[(k + 1) % 3], a))) {
                            points.push(p);
                        }
                    }
                }
            }
        }
    }
    points
}

// --- Growing a part out of the stock ---------------------------------------------------------------------------------

/// Signed distance to a closed mesh, negative inside: the nearest face out to `reach_mm` (further reads `±reach_mm`),
/// the side by which way the first face meets three skew rays, the majority deciding.
pub struct MeshField {
    mesh: Mesh,
    bvh: crate::interaction::bvh::Bvh,
    reach: f64,
}

impl MeshField {
    pub fn of(s: &Solid, reach_mm: f64) -> Self {
        Self::of_mesh(Mesh { vertices: s.v.iter().map(|p| Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), faces: s.f.clone(), ..Default::default() }, reach_mm)
    }

    pub fn of_mesh(mesh: Mesh, reach_mm: f64) -> Self {
        let bvh = crate::interaction::bvh::Bvh::build(&mesh);
        Self { mesh, bvh, reach: reach_mm }
    }

    /// Whether `p` is inside the mesh.
    pub fn inside(&self, p: P3) -> bool {
        const RAYS: [P3; 3] = [[0.5773, 0.5774, 0.5774], [-0.6412, 0.2113, 0.7377], [0.1531, -0.8122, -0.5631]];
        let votes = RAYS
            .iter()
            .filter(|d| {
                self.bvh.ray(&self.mesh, p, **d).is_some_and(|(f, _)| {
                    self.mesh.triangle(&self.mesh.faces[f]).is_some_and(|(a, b, c)| dot(cross(sub(b, a), sub(c, a)), **d) > 0.0)
                })
            })
            .count();
        votes >= 2
    }

    /// The signed distance at `p`.
    pub fn at(&self, p: P3) -> f64 {
        let d = self.bvh.nearest(&self.mesh, p, self.reach).map_or(self.reach, |(_, q)| len(sub(p, q)));
        if self.inside(p) { -d } else { d }
    }
}

/// How far below the stock's surface a fillet's foot is joined, and how far inside the part its top, as a share of its
/// radius: the fillet then crosses both across a shallow angle the union can trace, never along them.
pub const FILLET_SINK: f64 = 0.02;

/// How far past the fillet's foot the stock under a grown part curves down out of reach, mm: enough that the clipped
/// collar's rim lies deeper than a decimation moves a crease.
pub const FILLET_DIVE_MM: f64 = 0.2;

/// How fast the collar's copy of the part sinks into the part above the fillet, per mm of height: slow enough that the
/// fillet's top meets the part within a few degrees, fast enough that the copy is well inside it where the collar ends.
const FILLET_TUCK: f64 = 0.15;

/// `part` grown out of the stock with a fillet of `blend_mm`, the part's own mesh kept as it is. A collar is meshed at
/// `step_mm` round the part's foot: `smin(part, stock, blend_mm)` with the stock sunk a little and the part tucked a
/// little inside itself, so the fillet crosses both rather than lying on either; past the fillet's foot the stock
/// curves down into the metal ([`FILLET_DIVE_MM`]) before the collar is clipped to the part's footprint, and above
/// the fillet it ends inside the part. The collar is relaxed ([`relax_clean`]), decimated ([`clean_decimate_or_sites`])
/// and united with the part. `stock` is negative inside the metal. `None` when the part does not come within the
/// fillet's reach of the stock; an error when the collar will not come clean or unite.
pub fn fillet_into(part: &Solid, stock: Field, blend_mm: f64, step_mm: f64) -> Result<Option<Solid>> {
    let k = blend_mm;
    let sink = FILLET_SINK * k;
    let (foot, dive) = (k + sink, FILLET_DIVE_MM);
    let clip = foot + dive + 2.0 * step_mm;
    // Above this height over the stock the part stands as stored.
    let top = foot + sink + 2.0 * step_mm;
    let pad = clip + 2.0 * step_mm;
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &part.v {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k] - pad);
            hi[k] = hi[k].max(p[k] + pad);
        }
    }
    let own = MeshField::of(part, pad + step_mm);
    // A quadratic easing into a straight line `rate` steep, from `start` on, over `w`.
    let ease = |x: f64, start: f64, w: f64, rate: f64| {
        let u = (x - start).max(0.0);
        rate * if u < w { u * u / (2.0 * w) } else { u - 0.5 * w }
    };
    let field = |p: P3| {
        let b = stock(p);
        if b >= top {
            return b - top;
        }
        let a = own.at(p);
        if a >= clip {
            return a - clip;
        }
        let tuck = sink + ease(b, 0.5 * k, 0.5 * k, FILLET_TUCK);
        let down = ease(a, foot, dive, 1.0);
        smin(a + tuck, b + sink + down, k).max(a - clip).max(b - top)
    };
    let mut collar = tetra_mesh(lo, hi, step_mm, &field);
    if collar.f.is_empty() {
        return Ok(None);
    }
    relax_clean(&mut collar, &field, 2);
    let collar = clean_decimate_or_sites(&collar, collar.f.len() / 4).map_err(|sites| {
        let at = sites.first().map_or_else(String::new, |p| format!(", first at ({:.2}, {:.2}, {:.2})", p[0], p[1], p[2]));
        anyhow::anyhow!("its fillet crosses itself at {} sites{at}", sites.len())
    })?;
    let grown = csg::combine(part, &collar, csg::Op::Union).map_err(|e| anyhow::anyhow!("its fillet would not unite with it ({e})"))?;
    let (bad, volume) = closure(&grown);
    ensure!(bad == 0 && volume > 0.0, "grown into the band it does not close: {bad} open edges");
    let crossings = csg::self_crossings(&grown);
    ensure!(crossings == 0, "grown into the band it crosses itself {crossings} times");
    Ok(Some(grown))
}

// --- Hollows ---------------------------------------------------------------------------------------------------------

/// A height map on a regular (`x`, `u`) grid, read bilinear: the first air over a bore, and the roof a hollow keeps
/// under it.
#[derive(Clone, Debug)]
pub struct Heights {
    x0: f64,
    u0: f64,
    step: f64,
    nx: usize,
    nu: usize,
    h: Vec<f64>,
}

impl Heights {
    /// A map of `nx` by `nu` heights from (`x0`, `u0`) at `step`, row by row along `x`; `None` unless it holds two
    /// or more each way and no more than [`MAX_GRID_POINTS`].
    pub fn new(x0: f64, u0: f64, step: f64, nx: usize, nu: usize, h: Vec<f64>) -> Option<Self> {
        (nx >= 2 && nu >= 2 && nx.checked_mul(nu).is_some_and(|n| n == h.len() && n <= MAX_GRID_POINTS) && step > 0.0).then_some(Self { x0, u0, step, nx, nu, h })
    }

    /// Over each point of the grid from `lo` to `hi` (`x`, `u`) at `step`, the first height a probe rising from
    /// `floor(x, u)` finds outside `solid` (a distance field in `x`, `u`, `h`): the floor itself where the solid does
    /// not stand 0.05 mm over it, else in 0.04 mm steps up to `top`. The step is coarsened to hold [`MAX_GRID_POINTS`].
    pub fn first_air(lo: [f64; 2], hi: [f64; 2], step: f64, top: f64, floor: impl Fn(f64, f64) -> f64 + Sync, solid: impl Fn(P3) -> f64 + Sync) -> Self {
        let (step, _) = grid(lo, hi, step, MAX_GRID_POINTS);
        let (x0, u0) = (lo[0], lo[1]);
        let (nx, nu) = ((((hi[0] - x0) / step) as usize + 1).max(2), (((hi[1] - u0) / step) as usize + 1).max(2));
        let h = par_map(nx * nu, |c| {
            let (x, u) = (x0 + (c % nx) as f64 * step, u0 + (c / nx) as f64 * step);
            let mut h = floor(x, u) + 0.05;
            if solid([x, u, h]) > 0.0 {
                return floor(x, u);
            }
            while h < top && solid([x, u, h]) < 0.0 {
                h += 0.04;
            }
            h
        });
        Self { x0, u0, step, nx, nu, h }
    }

    /// The height at (`x`, `u`), clamped to the grid.
    pub fn at(&self, x: f64, u: f64) -> f64 {
        let fx = ((x - self.x0) / self.step).clamp(0.0, (self.nx - 2) as f64);
        let fu = ((u - self.u0) / self.step).clamp(0.0, (self.nu - 2) as f64);
        let (i, j) = (fx.floor() as usize, fu.floor() as usize);
        let (tx, tu) = (fx - i as f64, fu - j as f64);
        let g = |a: usize, b: usize| self.h[(j + b) * self.nx + i + a];
        lerp(lerp(g(0, 0), g(1, 0), tx), lerp(g(0, 1), g(1, 1), tx), tu)
    }

    /// The highest height at each cell that stays `r` mm clear of the surface this map draws: its erosion by a ball,
    /// so a roof at it leaves a wall `r` thick in every direction. Off the map reads as far below.
    pub fn ball_eroded(&self, r: f64) -> Self {
        let k = (r / self.step).ceil() as i64;
        let h: Vec<f64> = par_map(self.h.len(), |c| {
            let (i, j) = ((c % self.nx) as i64, (c / self.nx) as i64);
            let mut m = f64::MAX;
            for dj in -k..=k {
                for di in -k..=k {
                    let d = ((di * di + dj * dj) as f64).sqrt() * self.step;
                    if d > r {
                        continue;
                    }
                    let (a, b) = (i + di, j + dj);
                    let v = if a < 0 || b < 0 || a >= self.nx as i64 || b >= self.nu as i64 { -20.0 } else { self.h[b as usize * self.nx + a as usize] };
                    m = m.min(v - (r * r - d * d).sqrt());
                }
            }
            m
        });
        Self { h, ..*self }
    }

    /// A Gaussian blur of `sigma` mm.
    pub fn blurred(&self, sigma: f64) -> Self {
        let k = (3.0 * sigma / self.step).ceil() as i64;
        let w: Vec<f64> = (-k..=k).map(|d| (-0.5 * (d as f64 * self.step / sigma).powi(2)).exp()).collect();
        let sum: f64 = w.iter().sum();
        let pass = |src: &[f64], along_x: bool| -> Vec<f64> {
            par_map(src.len(), |c| {
                let (i, j) = ((c % self.nx) as i64, (c / self.nx) as i64);
                (-k..=k)
                    .zip(&w)
                    .map(|(d, wt)| {
                        let (a, b) = if along_x { ((i + d).clamp(0, self.nx as i64 - 1), j) } else { (i, (j + d).clamp(0, self.nu as i64 - 1)) };
                        wt * src[b as usize * self.nx + a as usize]
                    })
                    .sum::<f64>()
                    / sum
            })
        };
        let h = pass(&pass(&self.h, true), false);
        Self { h, ..*self }
    }
}

/// The connected shells of `m` with a vertex where `open` holds, and how many others were left out: a hollow keeps the
/// pockets that open into the finger hole, since a sealed one would cast solid.
pub fn open_shells(m: &Solid, open: impl Fn(P3) -> bool) -> (Solid, usize) {
    let mut parent: Vec<u32> = (0..m.v.len() as u32).collect();
    fn find(p: &mut [u32], x: u32) -> u32 {
        let mut r = x;
        while p[r as usize] != r {
            r = p[r as usize];
        }
        let mut y = x;
        while p[y as usize] != r {
            let next = p[y as usize];
            p[y as usize] = r;
            y = next;
        }
        r
    }
    for t in &m.f {
        for e in 1..3 {
            let (a, b) = (find(&mut parent, t[0]), find(&mut parent, t[e]));
            if a != b {
                parent[a as usize] = b;
            }
        }
    }
    let mut kept = HashSet::new();
    let mut all = HashSet::new();
    for (i, p) in m.v.iter().enumerate() {
        let root = find(&mut parent, i as u32);
        all.insert(root);
        if open(*p) {
            kept.insert(root);
        }
    }
    let f: Vec<[u32; 3]> = m.f.iter().copied().filter(|t| kept.contains(&find(&mut parent, t[0]))).collect();
    let mut remap = vec![u32::MAX; m.v.len()];
    let mut v = Vec::new();
    let f = f
        .into_iter()
        .map(|t| {
            t.map(|x| {
                if remap[x as usize] == u32::MAX {
                    remap[x as usize] = v.len() as u32;
                    v.push(m.v[x as usize]);
                }
                remap[x as usize]
            })
        })
        .collect();
    (Solid { v, f }, all.len() - kept.len())
}

// --- Out -------------------------------------------------------------------------------------------------------------

/// Undirected edges of `s` used other than once each way, and the volume it encloses, mm³: a closed outward mesh
/// reads `(0, positive)`.
pub fn closure(s: &Solid) -> (usize, f64) {
    let mut uses: HashMap<(u32, u32), i32> = HashMap::new();
    for t in &s.f {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { 1000 };
        }
    }
    let bad = uses.values().filter(|u| **u != 1001).count();
    let vol = s
        .f
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| s.v[i as usize]);
            dot(a, cross(b, c)) / 6.0
        })
        .sum();
    (bad, vol)
}

/// `s` as a render mesh, its normals the field's own.
pub fn to_mesh(s: &Solid, field: Field) -> Mesh {
    let normals: Vec<Vec3> = par_map(s.v.len(), |i| {
        let g = gradient(field, s.v[i]);
        let l = len(g).max(1e-12);
        Vec3((g[0] / l) as f32, (g[1] / l) as f32, (g[2] / l) as f32)
    });
    Mesh { vertices: s.v.iter().map(|p| Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), normals, faces: s.f.clone(), ..Default::default() }
}

/// `s` packed for a stored part as one freeform face, refused unless it is closed, encloses positive volume and does not
/// cross itself.
pub fn packed(s: &Solid) -> Result<Packed> {
    let (bad, volume) = closure(s);
    ensure!(bad == 0 && volume > 0.0, "The sculpt does not close: {bad} open edges");
    let crossings = csg::self_crossings(s);
    ensure!(crossings == 0, "The sculpt crosses itself {crossings} times");
    Packed::encode(&s.v, &s.f, &vec![0; s.f.len()], &[SurfaceKind::Freeform])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(r: f64) -> impl Fn(P3) -> f64 + Sync {
        move |p: P3| ellipsoid(p, [r; 3])
    }

    /// A ball with a tapering horn blended on: curvature enough for folds, slivers and a seam.
    fn blob(p: P3) -> f64 {
        smin(ellipsoid(p, [1.6, 1.3, 1.2]), round_cone(p, [0.0; 3], [2.2, 0.6, 0.3], 0.9, 0.35), 0.5)
    }

    fn assert_solid(s: &Solid, volume: f64, tolerance: f64) {
        let (bad, v) = closure(s);
        assert_eq!(bad, 0, "open edges");
        assert_eq!(csg::self_crossings(s), 0, "self-crossings");
        assert!((v - volume).abs() <= tolerance * volume, "volume {v:.3} against {volume:.3}");
    }

    fn same(a: &Solid, b: &Solid) -> bool {
        a.f == b.f && a.v.len() == b.v.len() && a.v.iter().zip(&b.v).all(|(p, q)| p.map(f64::to_bits) == q.map(f64::to_bits))
    }

    const BOX: (P3, P3) = ([-2.2, -2.0, -2.0], [3.0, 2.0, 2.0]);

    fn raw_blob() -> Solid {
        tetra_mesh(BOX.0, BOX.1, 0.08, &blob)
    }

    /// The blob's volume from a fine voxel count.
    fn blob_volume() -> f64 {
        let (h, (lo, hi)) = (0.02, BOX);
        let n: [usize; 3] = std::array::from_fn(|k| ((hi[k] - lo[k]) / h) as usize);
        let inside: usize = par_map(n[2], |k| {
            let mut c = 0;
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let p = [lo[0] + (i as f64 + 0.5) * h, lo[1] + (j as f64 + 0.5) * h, lo[2] + (k as f64 + 0.5) * h];
                    c += (blob(p) < 0.0) as usize;
                }
            }
            c
        })
        .into_iter()
        .sum();
        inside as f64 * h * h * h
    }

    #[test]
    fn primitives_measure_what_they_name() {
        assert!((ellipsoid([3.0, 4.0, 0.0], [2.0; 3]) - 3.0).abs() < 1e-12);
        // A rounded cone of equal radii is a capsule.
        assert!((round_cone([1.0, 2.0, 0.0], [0.0; 3], [3.0, 0.0, 0.0], 0.5, 0.5) - 1.5).abs() < 1e-12);
        assert!((round_cone([5.0, 0.0, 0.0], [0.0; 3], [3.0, 0.0, 0.0], 1.0, 0.5) - 1.5).abs() < 1e-12);
        assert_eq!(smin(0.0, 2.0, 0.5), 0.0);
        assert!(smin(0.3, 0.3, 0.5) < 0.3 && smax(0.3, 0.3, 0.5) > 0.3);
        assert!((trapezoid([0.0, 0.0], 1.0, 1.0, 1.0) + 1.0).abs() < 1e-12);
        assert!((trapezoid([3.0, 0.0], 1.0, 1.0, 1.0) - 2.0).abs() < 1e-12);
        let q = turn([1.0, 0.0, 0.0], 0, 1, 90.0);
        assert!(q[0].abs() < 1e-12 && (q[1] - 1.0).abs() < 1e-12);
        assert_eq!(bell(2.0, 2.0, 0.3), 1.0);
        let g = gradient(&sphere(1.0), [2.0, 0.0, 0.0]);
        assert!((g[0] - 1.0).abs() < 1e-6 && g[1].abs() < 1e-9);
    }

    #[test]
    fn tetra_mesh_closes_a_ball_at_its_volume() {
        let field = sphere(2.0);
        let s = tetra_mesh([-2.5; 3], [2.5; 3], 0.1, &field);
        assert!(s.f.len() > 10_000);
        assert_solid(&s, 4.0 / 3.0 * std::f64::consts::PI * 8.0, 0.01);
        assert!(same(&s, &tetra_mesh([-2.5; 3], [2.5; 3], 0.1, &field)), "not deterministic");
        // A field running off the box still closes, a cell inside the box's border: between the half-ball over that
        // cell and the whole half.
        let cut = tetra_mesh([-2.5, -2.5, 0.0], [2.5; 3], 0.1, &field);
        let over = |z: f64| std::f64::consts::PI * ((4.0 * 2.0 - 8.0 / 3.0) - (4.0 * z - z * z * z / 3.0));
        assert_solid(&cut, 0.5 * (over(0.0) + over(0.1)), 0.5 * (over(0.0) - over(0.1)) / over(0.1));
    }

    #[test]
    fn a_grid_past_the_cap_coarsens_its_step() {
        let (step, n) = grid([0.0; 3], [10.0; 3], 1e-4, MAX_GRID_POINTS);
        assert!(n.iter().product::<usize>() <= MAX_GRID_POINTS && step > 1e-4);
        let (step, n) = grid([0.0; 3], [5.0; 3], 0.1, MAX_GRID_POINTS);
        assert_eq!((step, n), (0.1, [51; 3]), "a grid under the cap keeps its step");
        let small = tetra_mesh_capped([-2.5; 3], [2.5; 3], 1e-6, &sphere(2.0), 40 * 40 * 40);
        assert_solid(&small, 4.0 / 3.0 * std::f64::consts::PI * 8.0, 0.03);
        for (lo, hi, step) in [([1.0; 3], [-1.0; 3], 0.1), ([0.0; 3], [1.0; 3], 0.0), ([0.0; 3], [1.0; 3], f64::NAN), ([0.0; 3], [0.0; 3], 0.1)] {
            let s = tetra_mesh(lo, hi, step, &sphere(0.3));
            assert_eq!(closure(&s).0, 0);
        }
    }

    #[test]
    fn relax_keeps_the_mesh_closed_and_on_the_field() {
        let raw = raw_blob();
        let off = |s: &Solid| s.v.iter().map(|p| blob(*p).abs()).sum::<f64>() / s.v.len() as f64;
        let mut relaxed = raw.clone();
        relax(&mut relaxed, &blob, 3);
        assert_solid(&relaxed, blob_volume(), 0.01);
        assert!(off(&relaxed) < off(&raw), "relaxing moved vertices off the field");
        let mut again = raw.clone();
        relax(&mut again, &blob, 3);
        assert!(same(&relaxed, &again), "not deterministic");
    }

    #[test]
    fn decimation_meets_its_budget_without_folding_or_crossing() {
        let raw = raw_blob();
        let d = decimate(&raw, 4_000, 5e-2, 3.0, 30.0, 45.0);
        // The guards stop it a little over budget, at an eighth of the raw faces.
        assert!(d.f.len() < 5_000 && d.f.len() > 3_000, "{} faces", d.f.len());
        assert_solid(&d, blob_volume(), 0.02);
        assert!(same(&d, &decimate(&raw, 4_000, 5e-2, 3.0, 30.0, 45.0)), "not deterministic");
        let c = clean_decimate(&raw, 6_000);
        assert!(c.f.len() < raw.f.len());
        assert_solid(&c, blob_volume(), 0.02);
        assert!(same(&c, &clean_decimate(&raw, 6_000)), "not deterministic");
    }

    #[test]
    fn sliver_collapse_removes_short_edges() {
        let raw = raw_blob();
        let shortest = |s: &Solid| s.f.iter().flat_map(|t| (0..3).map(move |e| len(sub(s.v[t[e] as usize], s.v[t[(e + 1) % 3] as usize])))).fold(f64::MAX, f64::min);
        assert!(shortest(&raw) < 0.01);
        let mut s = raw.clone();
        let n = collapse_short(&mut s, &blob, 0.02, &[]);
        assert!(n > 0 && s.f.len() == raw.f.len() - 2 * n);
        assert_solid(&s, blob_volume(), 0.01);
        let mut again = raw.clone();
        assert_eq!(collapse_short(&mut again, &blob, 0.02, &[]), n);
        assert!(same(&s, &again), "not deterministic");
        // Nothing is collapsed near a point to avoid.
        let mut spared = raw.clone();
        let everywhere: Vec<P3> = raw.v.iter().step_by(7).copied().collect();
        assert!(collapse_short(&mut spared, &blob, 0.02, &everywhere) < n);
    }

    #[test]
    fn polish_and_fold_smoothing_keep_the_mesh_closed() {
        let rough = decimate(&raw_blob(), 3_000, 5e-2, 3.0, 30.0, 45.0);
        let folds = |s: &Solid| {
            let mut edges: HashMap<(u32, u32), Vec<P3>> = HashMap::new();
            for t in &s.f {
                let [a, b, c] = t.map(|x| s.v[x as usize]);
                let n = cross(sub(b, a), sub(c, a));
                for e in 0..3 {
                    edges.entry((t[e].min(t[(e + 1) % 3]), t[e].max(t[(e + 1) % 3]))).or_default().push(mul(n, 1.0 / len(n)));
                }
            }
            // Edges turning 40 degrees or more.
            edges.values().filter(|ns| dot(ns[0], ns[1]) < 40f64.to_radians().cos()).count()
        };
        let mut flipped = rough.clone();
        let n = polish(&mut flipped, &blob, 6);
        assert_solid(&flipped, blob_volume(), 0.02);
        assert!(n > 0 && folds(&flipped) < folds(&rough));
        let mut smoothed = flipped.clone();
        let m = smooth_folds(&mut smoothed, &blob, 16);
        assert_solid(&smoothed, blob_volume(), 0.02);
        assert!(m > 0 && folds(&smoothed) < folds(&flipped));
        let mut again = flipped.clone();
        smooth_folds(&mut again, &blob, 16);
        assert!(same(&smoothed, &again), "not deterministic");
        let mut faired = smoothed.clone();
        smooth_folds_by(&mut faired, &blob, 24, 55.0, Some((0.05, &|p: P3| p[0] > 0.5)));
        polish_where(&mut faired, &blob, 4, &|p: P3| p[0] > 0.5);
        assert_eq!(closure(&faired).0, 0);
    }

    #[test]
    fn a_settled_sculpt_packs_as_a_stored_part() {
        let run = || {
            let mut raw = raw_blob();
            relax(&mut raw, &blob, 3);
            settle(clean_decimate(&raw, 6_000), &blob, &|p| p[0] > 0.5)
        };
        let s = run();
        assert_solid(&s, blob_volume(), 0.02);
        assert!(same(&s, &run()), "not deterministic");
        let mesh = to_mesh(&s, &blob);
        assert_eq!((mesh.vertices.len(), mesh.normals.len(), mesh.faces.len()), (s.v.len(), s.v.len(), s.f.len()));
        let p = packed(&s).unwrap();
        assert_eq!(p.triangles as usize, s.f.len());
        let mut open = s.clone();
        open.f.pop();
        assert!(packed(&open).is_err());
    }

    #[test]
    fn crossing_sites_find_two_balls_through_each_other() {
        let a = tetra_mesh([-1.5; 3], [1.5; 3], 0.2, &sphere(1.0));
        let mut both = a.clone();
        let base = a.v.len() as u32;
        both.v.extend(a.v.iter().map(|p| add(*p, [1.0, 0.013, 0.007])));
        both.f.extend(a.f.iter().map(|t| t.map(|x| x + base)));
        assert_eq!(crossing_sites(&a).len(), 0);
        let sites = crossing_sites(&both);
        assert!(!sites.is_empty());
        assert_eq!(csg::self_crossings(&both) > 0, !sites.is_empty());
        // Where two unit balls a millimetre apart meet: the circle of radius sqrt(3)/2 in the plane x = 0.5.
        for p in &sites {
            assert!((p[0] - 0.5).abs() < 0.05 && ((p[1] - 0.0065).hypot(p[2] - 0.0035) - 0.866).abs() < 0.05, "{p:?}");
        }
    }

    #[test]
    fn stock_reads_the_signed_distance_to_its_samples() {
        // Samples over a ball of radius 3, normals outward.
        let n = 20_000;
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        let samples: Vec<Sample> = (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
                let r = (1.0 - y * y).sqrt();
                let (s, c) = (golden * i as f64).sin_cos();
                let d = [r * c, y, r * s];
                Sample { p: mul(d, 3.0), n: d, i, ..Default::default() }
            })
            .collect();
        let stock = Stock::of(&samples, |s| s.p[1] > -2.0, [-4.0; 3], [4.0; 3], 0.1, 1.0, |p| len(p) < 3.0);
        for (p, want) in [([3.5, 0.0, 0.0], 0.5), ([0.0, 2.7, 0.0], -0.3), ([0.0, 0.0, -3.2], 0.2)] {
            assert!((stock.at(p) - want).abs() < 0.05, "{:.3} at {p:?}", stock.at(p));
        }
        // Past reach, and where `keep` left the samples out, it reads the fallback.
        assert_eq!(stock.at([0.0; 3]), -1.0);
        assert_eq!(stock.at([0.0, -3.5, 0.0]), 1.0);
        assert_eq!(stock.at([9.0, 9.0, 9.0]), 1.0);
    }

    #[test]
    fn a_ball_eroded_hollow_keeps_its_wall() {
        // A block over the floor h = 0, 3 mm tall and 8 mm square; the bore is everything under the floor.
        let block = |q: P3| (q[0].abs() - 4.0).max(q[1].abs() - 4.0).max(q[2] - 3.0).max(-q[2] - 1.0);
        let air = Heights::first_air([-5.0, -5.0], [5.0, 5.0], 0.1, 8.0, |_, _| 0.0, block);
        assert!((air.at(0.0, 0.0) - 3.0).abs() < 0.05 && air.at(4.6, 0.0) == 0.0);
        let wall = 1.0;
        let roof = air.ball_eroded(wall);
        assert!((roof.at(0.0, 0.0) - 2.0).abs() < 0.05);
        // Every roof point stays the wall's thickness from every surface point the first air draws.
        for (x, u) in [(0.0, 0.0), (2.9, 0.0), (2.0, 2.0), (-3.0, 1.0)] {
            for (a, b) in [(x + 0.5, u), (x, u - 0.7), (x + 0.3, u + 0.3)] {
                let d = len([a - x, b - u, air.at(a, b) - roof.at(x, u)]);
                assert!(d >= wall - 0.06, "{d:.3} from ({x}, {u})");
            }
        }
        let blurred = roof.blurred(0.35);
        assert!((blurred.at(0.0, 0.0) - roof.at(0.0, 0.0)).abs() < 0.01);
        // The hollow: under the roof, over the bore, within the block's footprint less the wall; plus a sealed pocket.
        let hollow = |p: P3| smax(p[2] - smin(roof.at(p[0], p[1]), 2.5, 0.3), (p[0].abs() - 3.0).max(p[1].abs() - 3.0).max(-p[2] - 0.5), 0.2);
        let pocket = |p: P3| len(sub(p, [0.0, 0.0, 4.5])) - 0.6;
        let field = |p: P3| hollow(p).min(pocket(p));
        let raw = tetra_mesh([-4.0, -4.0, -1.0], [4.0, 4.0, 5.5], 0.15, &field);
        let (kept, dropped) = open_shells(&raw, |p| p[2] < 0.0);
        assert_eq!(dropped, 1);
        let d = decimate(&kept, 4_000, 5e-2, 3.0, 30.0, 45.0);
        let (bad, volume) = closure(&d);
        assert!(bad == 0 && volume > 6.0 * 6.0 * 2.0 && volume < 6.0 * 6.0 * 3.0, "volume {volume:.2}");
        assert!(d.v.iter().all(|p| p[2] < 2.0 + 0.1), "the hollow breaks through its wall");
        assert!(packed(&d).is_ok());
        let (again, _) = open_shells(&raw, |p| p[2] < 0.0);
        assert!(same(&kept, &again));
    }

    #[test]
    fn heights_refuse_a_grid_they_cannot_read() {
        assert!(Heights::new(0.0, 0.0, 0.1, 2, 2, vec![0.0; 4]).is_some());
        assert!(Heights::new(0.0, 0.0, 0.1, 1, 4, vec![0.0; 4]).is_none());
        assert!(Heights::new(0.0, 0.0, 0.1, 3, 3, vec![0.0; 4]).is_none());
        assert!(Heights::new(0.0, 0.0, 0.0, 2, 2, vec![0.0; 4]).is_none());
    }


    /// `b` appended to `a` as a second shell, wound the other way when `void`.
    fn with_shell(a: &Solid, b: &Solid, void: bool) -> Solid {
        let mut both = a.clone();
        let base = a.v.len() as u32;
        both.v.extend(&b.v);
        both.f.extend(b.f.iter().map(|t| if void { [t[0] + base, t[2] + base, t[1] + base] } else { t.map(|x| x + base) }));
        both
    }

    /// A relax that pulls a ball's skin through a bead sitting just under it folds the mesh through itself; the clean
    /// relax puts back only the vertices round the crossing, and changes nothing where nothing folds.
    #[test]
    fn a_clean_relax_undoes_only_its_own_folds() {
        let ball = tetra_mesh([-1.3; 3], [1.3; 3], 0.08, &sphere(1.0));
        let bead = tetra_mesh([-0.2, -0.2, 0.75], [0.2, 0.2, 1.1], 0.02, &|p: P3| ellipsoid(sub(p, [0.0, 0.0, 0.9]), [0.095; 3]));
        let raw = with_shell(&ball, &bead, false);
        assert!(crossing_sites(&raw).is_empty());
        let pull = sphere(0.985);
        let mut folded = raw.clone();
        relax(&mut folded, &pull, 2);
        assert!(!crossing_sites(&folded).is_empty(), "the plain relax should fold here");
        let mut clean = raw.clone();
        let restored = relax_clean(&mut clean, &pull, 2);
        assert!(crossing_sites(&clean).is_empty());
        let moved = folded.v.iter().zip(&raw.v).filter(|(a, b)| a != b).count();
        assert!(restored > 0 && restored < moved / 4, "{restored} put back of {moved} moved");
        // Far from the bead the clean relax is the plain one.
        assert!(clean.v.iter().zip(&folded.v).all(|(c, f)| c[2] > 0.4 || c == f));
        let (mut plain, mut again) = (raw_blob(), raw_blob());
        relax(&mut plain, &blob, 3);
        assert_eq!(relax_clean(&mut again, &blob, 3), 0);
        assert!(same(&plain, &again));
    }

    /// Two balls meshed through each other cannot be decimated clean: `clean_decimate` hands back the raw mesh, still
    /// crossing, where `clean_decimate_or_sites` says where it crosses; a clean try comes back as `clean_decimate`
    /// gives it.
    #[test]
    fn a_decimation_that_crosses_says_where() {
        let a = tetra_mesh([-1.5; 3], [1.5; 3], 0.08, &sphere(1.0));
        let moved = Solid { v: a.v.iter().map(|p| add(*p, [1.0, 0.013, 0.007])).collect(), f: a.f.clone() };
        let raw = with_shell(&a, &moved, false);
        assert!(csg::self_crossings(&clean_decimate(&raw, 2_000)) > 0, "the plain fallback still crosses");
        let sites = clean_decimate_or_sites(&raw, 2_000).expect_err("two balls through each other should not decimate clean");
        // On the circle where they meet: radius sqrt(3)/2 in the plane x = 0.5.
        assert!(!sites.is_empty());
        for p in &sites {
            assert!((p[0] - 0.5).abs() < 0.1 && ((p[1] - 0.0065).hypot(p[2] - 0.0035) - 0.866).abs() < 0.1, "{p:?}");
        }
        let blob = raw_blob();
        assert!(same(&clean_decimate_or_sites(&blob, 6_000).unwrap(), &clean_decimate(&blob, 6_000)));
    }

    /// A part grown out of a slab of stock: closed, uncrossed, the part's own mesh kept above the fillet, the collar's
    /// rim buried in the stock, and a fillet's worth of metal at the junction where the plain union has a crease.
    #[test]
    fn a_part_grows_out_of_the_stock_with_a_fillet() {
        let stock = |p: P3| p[2];
        let post = tetra_mesh([-1.0, -1.0, -0.5], [1.0, 1.0, 2.0], 0.05, &|p: P3| round_cone(p, [0.0, 0.0, -0.3], [0.0, 0.0, 1.4], 0.6, 0.4));
        let post = clean_decimate(&post, 3_000);
        let grown = fillet_into(&post, &stock, 0.4, 0.05).unwrap().unwrap();
        let (bad, _) = closure(&grown);
        assert_eq!((bad, csg::self_crossings(&grown)), (0, 0));
        // Above the collar the post is its own mesh, vertex for vertex.
        let high = |s: &Solid| { let mut v: Vec<[u64; 3]> = s.v.iter().filter(|p| p[2] > 0.7).map(|p| p.map(f64::to_bits)).collect(); v.sort(); v };
        assert_eq!(high(&grown), high(&post));
        // Every vertex is on the post, on the fillet within its reach, or under the stock's surface.
        let own = MeshField::of(&post, 2.0);
        for p in &grown.v {
            assert!(own.at(*p) < 0.4 + FILLET_DIVE_MM + 0.25 && (own.at(*p) < 0.03 || p[2] < 0.42), "{p:?}");
            assert!(p[2] < -1e-3 || p[0].hypot(p[1]) < 0.6 + 0.45, "{p:?} at {} from the post", own.at(*p));
        }
        // At the foot, 0.05 mm off the stock and 0.05 mm out from the post, the grown part has metal and the post has none.
        let r = 0.6 - 0.2 * 0.3 / 1.7 + 0.05;
        let field = MeshField::of(&grown, 2.0);
        assert!(own.at([r, 0.0, 0.05]) > 0.0 && field.at([r, 0.0, 0.05]) < 0.0);
        // Out of the stock's reach nothing grows.
        assert!(fillet_into(&post, &|p: P3| p[2] + 3.0, 0.4, 0.05).unwrap().is_none());
    }
}
