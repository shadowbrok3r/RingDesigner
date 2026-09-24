//! Edge collapses that hold a closed mesh within a distance of every vertex it had: how a band that
//! cannot be refined is sized for STEP. A removed vertex rides a face near it and is measured again
//! whenever that face moves, so the bound is checked at every collapse rather than estimated.
use crate::Mesh;
use crate::interaction::bvh::{closest_on_triangle, cross, dist2, dot, sub};
use std::collections::VecDeque;

/// A collapse is refused when a face it moves turns further than this, as a cosine: 60°.
const TURN_COS: f64 = 0.5;
/// A collapse is refused when a face it moves would span less than this, mm², twice over.
const DEGENERATE: f64 = 1e-10;
/// Cost buckets a quarter octave wide, from 2⁻⁶⁴ up.
const BUCKETS: usize = 512;

/// A collapse offered: `(from, to, from's version, to's version)`.
type Offer = (u32, u32, u32, u32);

/// Offers by cost, cheapest bucket first and in arrival order within one: a queue whose every push and pop costs the same.
struct Buckets {
    slots: Vec<VecDeque<Offer>>,
    lowest: usize,
}

impl Buckets {
    fn new() -> Self {
        Self { slots: vec![VecDeque::new(); BUCKETS], lowest: BUCKETS }
    }

    fn push(&mut self, cost: f64, offer: Offer) {
        let b = if cost > 0.0 { (cost.log2() * 4.0 + 256.0).clamp(1.0, (BUCKETS - 1) as f64) as usize } else { 0 };
        self.slots[b].push_back(offer);
        self.lowest = self.lowest.min(b);
    }

    fn pop(&mut self) -> Option<Offer> {
        while self.lowest < BUCKETS {
            if let Some(offer) = self.slots[self.lowest].pop_front() {
                return Some(offer);
            }
            self.lowest += 1;
        }
        None
    }
}

/// `mesh` with edges collapsed while every vertex it had stays within `tolerance_mm` of a face left, and
/// closed as it came; `None` when it came open or nothing would collapse.
pub(super) fn decimated(mesh: &Mesh, tolerance_mm: f64) -> Option<Mesh> {
    if mesh.faces.len() < 8 || !(tolerance_mm > 0.0) || !mesh.validate().watertight {
        return None;
    }
    let mut c = Collapser::new(mesh, tolerance_mm);
    c.run();
    let out = c.into_mesh();
    (out.faces.len() < mesh.faces.len() && out.validate().watertight).then_some(out)
}

/// A quadric's ten coefficients: the squared distance to a sum of planes.
type Quadric = [f64; 10];

fn plane_quadric(n: [f64; 3], d: f64, w: f64) -> Quadric {
    let [a, b, c] = n;
    [a * a * w, a * b * w, a * c * w, a * d * w, b * b * w, b * c * w, b * d * w, c * c * w, c * d * w, d * d * w]
}

fn quadric_at(q: &Quadric, x: [f64; 3]) -> f64 {
    let [a2, ab, ac, ad, b2, bc, bd, c2, cd, d2] = *q;
    let [x, y, z] = x;
    a2 * x * x + 2.0 * ab * x * y + 2.0 * ac * x * z + 2.0 * ad * x + b2 * y * y + 2.0 * bc * y * z + 2.0 * bd * y + c2 * z * z + 2.0 * cd * z + d2
}

struct Collapser {
    p: Vec<[f64; 3]>,
    faces: Vec<[u32; 3]>,
    alive: Vec<bool>,
    /// The live faces round each vertex.
    around: Vec<Vec<u32>>,
    gone: Vec<bool>,
    version: Vec<u32>,
    quadric: Vec<Quadric>,
    /// The removed vertices each face answers for.
    orphans: Vec<Vec<u32>>,
    tol2: f64,
    queue: Buckets,
}

impl Collapser {
    fn new(mesh: &Mesh, tolerance_mm: f64) -> Self {
        let p: Vec<[f64; 3]> = mesh.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();
        let faces = mesh.faces.clone();
        let mut around = vec![Vec::new(); p.len()];
        let mut quadric = vec![[0.0; 10]; p.len()];
        for (f, t) in faces.iter().enumerate() {
            let n = cross(sub(p[t[1] as usize], p[t[0] as usize]), sub(p[t[2] as usize], p[t[0] as usize]));
            let len = dot(n, n).sqrt();
            for &v in t {
                around[v as usize].push(f as u32);
            }
            if len > 0.0 {
                let unit = n.map(|x| x / len);
                let q = plane_quadric(unit, -dot(unit, p[t[0] as usize]), len * 0.5);
                for &v in t {
                    for (k, x) in q.iter().enumerate() {
                        quadric[v as usize][k] += x;
                    }
                }
            }
        }
        let n = p.len();
        let mut c = Self { p, faces, alive: vec![true; mesh.faces.len()], around, gone: vec![false; n], version: vec![0; n], quadric, orphans: vec![Vec::new(); mesh.faces.len()], tol2: tolerance_mm * tolerance_mm, queue: Buckets::new() };
        for f in 0..c.faces.len() {
            let t = c.faces[f];
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                if a < b {
                    c.push(a, b);
                    c.push(b, a);
                }
            }
        }
        c
    }

    /// Offers the collapse of `from` into `to` at what `from`'s and `to`'s planes say it costs.
    fn push(&mut self, from: u32, to: u32) {
        let mut q = self.quadric[from as usize];
        for (k, x) in self.quadric[to as usize].iter().enumerate() {
            q[k] += x;
        }
        let cost = quadric_at(&q, self.p[to as usize]).max(0.0);
        if cost.is_finite() {
            self.queue.push(cost, (from, to, self.version[from as usize], self.version[to as usize]));
        }
    }

    fn run(&mut self) {
        while let Some((from, to, vf, vt)) = self.queue.pop() {
            let (u, v) = (from as usize, to as usize);
            if self.gone[u] || self.gone[v] || self.version[u] != vf || self.version[v] != vt {
                continue;
            }
            self.collapse(from, to);
        }
    }

    fn normal(&self, t: [u32; 3]) -> [f64; 3] {
        cross(sub(self.p[t[1] as usize], self.p[t[0] as usize]), sub(self.p[t[2] as usize], self.p[t[0] as usize]))
    }

    /// The vertices sharing a face with `x`, once each.
    fn neighbours(&self, x: u32) -> Vec<u32> {
        let mut n: Vec<u32> = self.around[x as usize].iter().flat_map(|f| self.faces[*f as usize]).filter(|w| *w != x).collect();
        n.sort_unstable();
        n.dedup();
        n
    }

    /// The squared distance from `x` to face `t`.
    fn reach2(&self, x: [f64; 3], t: [u32; 3]) -> f64 {
        let [a, b, c] = t.map(|i| self.p[i as usize]);
        dist2(x, closest_on_triangle(x, a, b, c))
    }

    /// Collapses `from` into `to` when the mesh stays a closed manifold, no moved face turns over or
    /// thins to nothing, and every vertex answered for stays within the tolerance of the faces round `to`.
    fn collapse(&mut self, from: u32, to: u32) -> bool {
        let (u, v) = (from as usize, to as usize);
        let round_u = self.around[u].clone();
        let shared: Vec<u32> = round_u.iter().copied().filter(|f| self.faces[*f as usize].contains(&to)).collect();
        if shared.len() != 2 {
            return false;
        }
        let opposite = |f: u32| self.faces[f as usize].into_iter().find(|w| *w != from && *w != to);
        let (Some(a), Some(b)) = (opposite(shared[0]), opposite(shared[1])) else { return false };
        if a == b {
            return false;
        }
        // Link condition: the two ends share exactly the two corners across the edge.
        let near_v = self.neighbours(to);
        let common = self.neighbours(from).into_iter().filter(|w| near_v.binary_search(w).is_ok()).count();
        if common != 2 {
            return false;
        }
        let moving: Vec<(u32, [u32; 3])> = round_u.iter().filter(|f| !shared.contains(f)).map(|f| (*f, self.faces[*f as usize].map(|w| if w == from { to } else { w }))).collect();
        for (f, t) in &moving {
            let (old, new) = (self.normal(self.faces[*f as usize]), self.normal(*t));
            let (lo, ln) = (dot(old, old).sqrt(), dot(new, new).sqrt());
            if ln <= DEGENERATE || dot(old, new) < TURN_COS * lo * ln {
                return false;
            }
        }
        // The faces round `to` once the collapse is made.
        let fan: Vec<(u32, [u32; 3])> = moving.iter().copied().chain(self.around[v].iter().filter(|f| !shared.contains(f)).map(|f| (*f, self.faces[*f as usize]))).collect();
        let mut placed: Vec<(u32, u32)> = Vec::new();
        let riders = std::iter::once((from, None)).chain(round_u.iter().flat_map(|f| self.orphans[*f as usize].iter().map(move |o| (*o, Some(*f)))));
        for (o, on) in riders {
            let x = self.p[o as usize];
            // The face it rode first, where that face still stands; else the nearest of the fan.
            let kept = on.and_then(|f| fan.iter().find(|(g, _)| *g == f)).filter(|(_, t)| self.reach2(x, *t) <= self.tol2);
            let face = match kept {
                Some((g, _)) => *g,
                None => {
                    let best = fan.iter().map(|(g, t)| (*g, self.reach2(x, *t))).min_by(|a, b| a.1.total_cmp(&b.1));
                    match best {
                        Some((g, d)) if d <= self.tol2 => g,
                        _ => return false,
                    }
                }
            };
            placed.push((o, face));
        }
        // Made: `from` leaves, the two faces across the edge close, the rest turn to `to`.
        self.gone[u] = true;
        for f in &shared {
            self.alive[*f as usize] = false;
            for w in [a, b, to] {
                self.around[w as usize].retain(|g| g != f);
            }
        }
        for f in &round_u {
            self.orphans[*f as usize].clear();
        }
        for (f, t) in &moving {
            self.faces[*f as usize] = *t;
            self.around[v].push(*f);
        }
        self.around[u].clear();
        for (o, f) in placed {
            self.orphans[f as usize].push(o);
        }
        let q = self.quadric[u];
        for (k, x) in q.iter().enumerate() {
            self.quadric[v][k] += x;
        }
        self.version[v] += 1;
        for w in self.neighbours(to) {
            self.push(to, w);
            self.push(w, to);
        }
        true
    }

    /// The live faces over the vertices left, renumbered.
    fn into_mesh(self) -> Mesh {
        let mut index = vec![u32::MAX; self.p.len()];
        let mut out = Mesh::default();
        for (i, x) in self.p.iter().enumerate() {
            if !self.gone[i] && !self.around[i].is_empty() {
                index[i] = out.vertices.len() as u32;
                out.vertices.push(crate::mesh::Vec3(x[0] as f32, x[1] as f32, x[2] as f32));
            }
        }
        out.faces = self.faces.iter().zip(&self.alive).filter(|(_, a)| **a).map(|(t, _)| t.map(|i| index[i as usize])).collect();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain band swept at `t` x `p`.
    fn band(t: usize, p: usize) -> Mesh {
        let d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        crate::mesh::build(&d, &crate::AlphaLibrary::builtin(), crate::BuildParams { theta_steps: t, profile_steps: p, refine: None, ..Default::default() }).mesh
    }

    /// The farthest any vertex of `from` stands from a face of `to`.
    fn farthest(from: &Mesh, to: &Mesh) -> f64 {
        let bvh = crate::interaction::bvh::Bvh::build(to);
        from.vertices.iter().map(|v| {
            let x = [v.0 as f64, v.1 as f64, v.2 as f64];
            bvh.nearest(to, x, 1.0).map_or(1.0, |(_, q)| dist2(x, q).sqrt())
        }).fold(0.0, f64::max)
    }

    #[test]
    fn a_band_collapses_to_a_closed_mesh_every_old_vertex_within_the_tolerance() {
        let dense = band(384, 128);
        for tolerance in [0.005, 0.02] {
            let started = std::time::Instant::now();
            let out = decimated(&dense, tolerance).expect("the band collapses");
            let ms = started.elapsed().as_secs_f64() * 1e3;
            let v = out.validate();
            let far = farthest(&dense, &out);
            let (before, after) = (dense.volume_mm3(), out.volume_mm3());
            eprintln!("{tolerance} mm: {} to {} faces in {ms:.0} ms, farthest old vertex {far:.4} mm, volume {before:.3} to {after:.3} mm3", dense.faces.len(), out.faces.len());
            assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
            assert!(far <= tolerance * 1.0001, "{far} against {tolerance}");
            assert!(out.faces.len() * 4 < dense.faces.len(), "{} of {}", out.faces.len(), dense.faces.len());
            assert!((after - before).abs() < tolerance * dense.surface_area_mm2(), "{after} against {before}");
        }
    }

    /// The collapse alone on an export sweep: `cargo test --release -p ringdesign-core -- --ignored collapse_time --nocapture`.
    #[test]
    #[ignore]
    fn collapse_time() {
        let dense = band(1024, 320);
        for tolerance in [0.02, 0.01] {
            let started = std::time::Instant::now();
            let out = decimated(&dense, tolerance).unwrap();
            eprintln!("{tolerance} mm: {} to {} faces in {:.0} ms", dense.faces.len(), out.faces.len(), started.elapsed().as_secs_f64() * 1e3);
        }
    }

    #[test]
    fn an_open_mesh_and_a_zero_tolerance_are_left_alone() {
        let mut open = band(48, 32);
        assert!(decimated(&open, 0.0).is_none());
        open.faces.pop();
        assert!(decimated(&open, 0.02).is_none());
    }
}
