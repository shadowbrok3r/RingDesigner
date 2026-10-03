//! The wall census behind [`super::census`].
use super::{CensusOptions, MAX_ZONES, ThinKind, ThinZone, Thickness};
use crate::interaction::bvh::Bvh;
use crate::mesh::Mesh;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

type P3 = [f64; 3];

/// Samples a census aims at: the pitch widens until a plane at any turn crosses no more cells, and edges add a few percent.
const MAX_SAMPLES: f64 = 1_000_000.0;
/// Pitch cells a plane crosses per pitch² of its area at most, square to a cube diagonal: √3.
const OBLIQUE: f64 = 1.732_050_807_568_877_2;
/// Least height off its longest edge a face needs for its normal to be trusted, mm.
const MIN_FACE_HEIGHT_MM: f64 = 1e-4;
/// Directions a thin sample is marched in, as opposite pairs.
const DIRECTIONS: usize = 8;
/// Directions round a full turn the nearest straight way out of a parallel-faced section is probed in.
const PROBES: usize = 16;
/// Least length of the two faces' summed normals, square to the section, that orients the march toward where they converge.
const CONVERGING: f64 = 0.05;
/// Halvings that place where a march's section reaches the floor.
const BISECTIONS: usize = 3;
/// Crossings closer than this along a ray are one surface hit, mm.
const SAME_HIT_MM: f64 = 1e-7;
/// Bisected pieces of surface binned between two reads of the cancel flag.
const PIECES_PER_CHECK: usize = 1 << 16;
/// Samples read, or thin samples gathered into zones, between two reads of the cancel flag.
const SAMPLES_PER_CHECK: usize = 2048;

const NOTE: &str = "Area census: one sample per pitch-sized cell of surface, each read along its inward normal to where it leaves the metal. A reading under the floor is an edge where its section closes at a free edge, reaches the floor within the edge reach of it, and is everywhere on the way at least floor / reach of its distance from that edge; it is a wall otherwise. Gate on walls and unresolved samples; at most 64 zones of each kind are listed, largest first, and the counts and areas cover every sample.";
const NOT_ASSESSED: &str = "Thickness not assessed: the mesh is empty or not watertight, or the floor is not a positive length";

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: P3) -> Option<P3> {
    let l = norm(a);
    (l > 1e-12 && l.is_finite()).then(|| scale(a, 1.0 / l))
}
/// Any unit vector square to `a`.
fn square_to(a: P3) -> P3 {
    let k = (0..3).min_by(|&i, &j| a[i].abs().total_cmp(&a[j].abs())).unwrap_or(0);
    let mut e = [0.0; 3];
    e[k] = 1.0;
    unit(cross(a, e)).unwrap_or([1.0, 0.0, 0.0])
}

/// One sample: a point on a face, the face's outward unit normal, and the surface area it stands for.
#[derive(Clone, Copy, Debug)]
pub(super) struct Sample {
    pub p: P3,
    pub n: P3,
    pub area: f64,
}

/// One pitch-sized cell of surface as the samples are binned.
struct Cell {
    p: P3,
    n: P3,
    d2: f64,
    area: f64,
}

/// Whether `cancel` is raised.
fn stopped(cancel: Option<&AtomicBool>) -> bool {
    #[cfg(test)]
    tests::checked(cancel);
    cancel.is_some_and(|c| c.load(Ordering::Relaxed))
}

/// One sample per pitch cell and facing: the centroid nearest the cell centre of faces bisected to half a pitch, with their area; `None` once `cancel` is raised.
pub(super) fn sample(mesh: &Mesh, pitch: f64, cancel: Option<&AtomicBool>) -> Option<Vec<Sample>> {
    let half2 = 0.25 * pitch * pitch;
    let mut cells: HashMap<[i64; 4], Cell> = HashMap::new();
    let mut stack: Vec<[P3; 3]> = Vec::new();
    let mut pieces = 0usize;
    for f in &mesh.faces {
        let Some((a, b, c)) = mesh.triangle(f) else { continue };
        let normal = cross(sub(b, a), sub(c, a));
        let twice = norm(normal);
        if !(twice > 0.0 && twice.is_finite()) {
            continue;
        }
        let n = scale(normal, 1.0 / twice);
        let longest = [sub(b, a), sub(c, b), sub(a, c)].map(|e| dot(e, e)).into_iter().fold(0.0, f64::max).sqrt();
        let reliable = twice / longest > MIN_FACE_HEIGHT_MM;
        let bucket = {
            let k = (0..3).max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs())).unwrap_or(0);
            2 * k as i64 + i64::from(n[k] < 0.0)
        };
        stack.push([a, b, c]);
        while let Some([a, b, c]) = stack.pop() {
            let e = [dot(sub(b, a), sub(b, a)), dot(sub(c, b), sub(c, b)), dot(sub(a, c), sub(a, c))];
            let k = (0..3).max_by(|&i, &j| e[i].total_cmp(&e[j])).unwrap_or(0);
            if e[k] > half2 {
                let (p, q, r) = [(a, b, c), (b, c, a), (c, a, b)][k];
                let m = scale(add(p, q), 0.5);
                stack.push([p, m, r]);
                stack.push([m, q, r]);
                continue;
            }
            pieces += 1;
            if pieces % PIECES_PER_CHECK == 0 && stopped(cancel) {
                return None;
            }
            let p: P3 = std::array::from_fn(|i| (a[i] + b[i] + c[i]) / 3.0);
            let w = 0.5 * norm(cross(sub(b, a), sub(c, a)));
            let key = [(p[0] / pitch).floor() as i64, (p[1] / pitch).floor() as i64, (p[2] / pitch).floor() as i64, bucket];
            let cell = cells.entry(key).or_insert(Cell { p, n, d2: f64::INFINITY, area: 0.0 });
            cell.area += w;
            if reliable {
                let centre: P3 = std::array::from_fn(|i| (key[i] as f64 + 0.5) * pitch);
                let d2 = dot(sub(p, centre), sub(p, centre));
                if d2 < cell.d2 {
                    (cell.p, cell.n, cell.d2) = (p, n, d2);
                }
            }
        }
    }
    let mut keyed: Vec<([i64; 4], Cell)> = cells.into_iter().filter(|(_, c)| c.d2.is_finite()).collect();
    keyed.sort_unstable_by_key(|e| e.0);
    Some(keyed.into_iter().map(|(_, c)| Sample { p: c.p, n: c.n, area: c.area }).collect())
}

/// Each face's shell, by shared vertices, and how many shells there are.
fn shells(mesh: &Mesh) -> (Vec<u32>, usize) {
    fn find(parent: &mut [u32], mut i: u32) -> u32 {
        while parent[i as usize] != i {
            parent[i as usize] = parent[parent[i as usize] as usize];
            i = parent[i as usize];
        }
        i
    }
    let mut parent: Vec<u32> = (0..mesh.vertices.len() as u32).collect();
    for f in &mesh.faces {
        if f.iter().any(|&i| i as usize >= parent.len()) {
            continue;
        }
        for (x, y) in [(f[0], f[1]), (f[1], f[2])] {
            let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
            if rx != ry {
                parent[rx.max(ry) as usize] = rx.min(ry);
            }
        }
    }
    let n = parent.len();
    let of: Vec<u32> = mesh.faces.iter().map(|f| if (f[0] as usize) < n { find(&mut parent, f[0]) } else { u32::MAX }).collect();
    let mut roots: Vec<u32> = of.iter().copied().filter(|&r| r != u32::MAX).collect();
    roots.sort_unstable();
    roots.dedup();
    let count = roots.len();
    (of, count)
}

/// How a sample's ray went.
#[derive(Clone, Copy, Debug)]
pub(super) enum Reading {
    /// The sample lies inside another shell: no surface of the metal.
    Internal,
    /// The ray found no consistent way out.
    Unresolved,
    /// The section, mm, and the face the ray left by.
    Section { t: f64, far: usize },
}

/// How a march along the mid-surface ended.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Walk {
    /// Out through a free edge, this far along.
    Closed(f64),
    /// The section reached the floor this far along.
    Body(f64),
    /// Neither within the limit.
    Thin,
}

/// One thin sample's class.
#[derive(Clone, Copy, Debug)]
struct Thin {
    kind: ThinKind,
    mid: P3,
    depth: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Internal,
    Unresolved,
    Read { t: f64, thin: Option<Thin> },
}

/// The mesh, its tree, and the census's lengths.
pub(super) struct Probe<'a> {
    mesh: &'a Mesh,
    bvh: Bvh,
    /// Each face's shell.
    shell: Vec<u32>,
    multi: bool,
    floor: f64,
    reach: f64,
    step: f64,
}

impl<'a> Probe<'a> {
    pub(super) fn new(mesh: &'a Mesh, floor: f64, reach: f64) -> Self {
        let (shell, count) = shells(mesh);
        Self { mesh, bvh: Bvh::build(mesh), shell, multi: count > 1, floor, reach, step: (floor / 8.0).max(1e-3) }
    }

    /// Face `f`'s outward unit normal.
    fn normal(&self, f: usize) -> P3 {
        self.mesh
            .faces
            .get(f)
            .and_then(|face| self.mesh.triangle(face))
            .and_then(|(a, b, c)| unit(cross(sub(b, a), sub(c, a))))
            .unwrap_or([0.0; 3])
    }

    /// Faces the ray crosses, sorted, +1 leaving and -1 entering, one shell's hit on a shared edge counted once.
    fn crossings(&self, o: P3, d: P3) -> Vec<(f64, usize, i32)> {
        let mut raw = Vec::new();
        self.bvh.ray_all(self.mesh, o, d, &mut raw);
        let mut c: Vec<(f64, usize, i32)> = raw
            .into_iter()
            .filter_map(|(f, t)| {
                let s = dot(d, self.normal(f));
                (s != 0.0).then_some((t, f, if s > 0.0 { 1 } else { -1 }))
            })
            .collect();
        c.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let shell = |f: usize| self.shell.get(f).copied().unwrap_or(u32::MAX);
        let mut kept: Vec<(f64, usize, i32)> = Vec::with_capacity(c.len());
        let (mut cluster, mut prev) = (0, f64::NEG_INFINITY);
        for x in c {
            if x.0 - prev >= SAME_HIT_MM {
                cluster = kept.len();
            }
            prev = x.0;
            if !kept[cluster..].iter().any(|k| k.2 == x.2 && shell(k.1) == shell(x.1)) {
                kept.push(x);
            }
        }
        kept
    }

    /// How many shells hold `o`, read along `d`.
    fn winding(&self, o: P3, d: P3) -> i32 {
        self.crossings(o, d).iter().map(|c| c.2).sum()
    }

    /// Distance along the unit `d` to where the ray leaves the metal, and that face; `None` outside the metal.
    fn leave(&self, o: P3, d: P3) -> Option<(f64, usize)> {
        if !self.multi {
            let (f, t) = self.bvh.ray(self.mesh, o, d)?;
            return (dot(d, self.normal(f)) > 0.0).then_some((t, f));
        }
        let crossings = self.crossings(o, d);
        let mut w: i32 = crossings.iter().map(|c| c.2).sum();
        if w < 1 {
            return None;
        }
        for &(t, f, s) in &crossings {
            w -= s;
            if w == 0 {
                return Some((t, f));
            }
        }
        None
    }

    /// One sample's section along its inward normal, its rays starting on its own face, which they never cross.
    pub(super) fn read(&self, s: &Sample) -> Reading {
        if self.multi && self.winding(s.p, s.n) != 0 {
            return Reading::Internal;
        }
        self.leave(s.p, scale(s.n, -1.0)).map_or(Reading::Unresolved, |(t, far)| Reading::Section { t, far })
    }

    /// The section through `m` along the unit `axis`: its length, midpoint, and the axis its two faces give.
    fn section(&self, m: P3, axis: P3) -> Option<(f64, P3, P3)> {
        let (t1, f1) = self.leave(m, axis)?;
        let (t2, f2) = self.leave(m, scale(axis, -1.0))?;
        let mid = add(m, scale(axis, 0.5 * (t1 - t2)));
        let turned = unit(sub(self.normal(f1), self.normal(f2))).filter(|a| dot(*a, axis) > 0.0).unwrap_or(axis);
        Some((t1 + t2, mid, turned))
    }

    /// Whether a section `s` standing `x` from its free edge is at least floor / reach of that distance, within half a step.
    fn fed(&self, x: f64, s: f64) -> bool {
        x <= 0.5 * self.step + s * self.reach / self.floor
    }

    /// March the mid-surface from `start` along `dir` until it leaves the metal, meets the floor, or runs `limit` mm, keeping each thin section in `stations`; with a free edge `edge` behind it, stop at the first section not fed.
    fn walk(&self, start: P3, axis: P3, dir: P3, first: f64, limit: f64, edge: Option<f64>, stations: &mut Vec<(f64, f64)>) -> Walk {
        stations.clear();
        let (mut m, mut a, mut u, mut went, mut last) = (start, axis, dir, 0.0, first);
        while went < limit - 1e-9 {
            let step = self.step.min(limit - went);
            match self.leave(m, u) {
                Some((t, _)) if t <= step => return Walk::Closed(went + t),
                Some(_) => {}
                None => return Walk::Thin,
            }
            let Some((s, mid, turned)) = self.section(add(m, scale(u, step)), a) else {
                return Walk::Thin;
            };
            if s >= self.floor {
                let (mut lo, mut hi, mut s_lo, mut s_hi) = (0.0, step, last, s);
                for _ in 0..BISECTIONS {
                    let h = 0.5 * (lo + hi);
                    match self.section(add(m, scale(u, h)), a) {
                        Some((sh, _, _)) if sh >= self.floor => (hi, s_hi) = (h, sh),
                        Some((sh, _, _)) => {
                            (lo, s_lo) = (h, sh);
                            stations.push((went + h, sh));
                            if edge.is_some_and(|c| !self.fed(c + went + h, sh)) {
                                return Walk::Thin;
                            }
                        }
                        None => break,
                    }
                }
                let back = if s_hi > s_lo { (hi - lo) * (s_hi - self.floor) / (s_hi - s_lo) } else { 0.0 };
                return Walk::Body(went + hi - back.clamp(0.0, hi - lo));
            }
            went += step;
            stations.push((went, s));
            if edge.is_some_and(|c| !self.fed(c + went, s)) {
                return Walk::Thin;
            }
            last = s;
            m = mid;
            if dot(turned, a) > 0.5 {
                a = turned;
            }
            match unit(sub(u, scale(a, dot(u, a)))) {
                Some(v) => u = v,
                None => return Walk::Thin,
            }
        }
        Walk::Thin
    }

    /// The direction square to `axis` of the nearest straight way out from `m`, refined between probes.
    fn nearest_exit(&self, m: P3, axis: P3) -> P3 {
        let e1 = square_to(axis);
        let e2 = cross(axis, e1);
        let gap = std::f64::consts::TAU / PROBES as f64;
        let at = |k: f64| add(scale(e1, (k * gap).cos()), scale(e2, (k * gap).sin()));
        let d: Vec<f64> = (0..PROBES).map(|k| self.leave(m, at(k as f64)).map_or(f64::INFINITY, |(t, _)| t)).collect();
        let best = (0..PROBES).min_by(|&i, &j| d[i].total_cmp(&d[j])).unwrap_or(0);
        let (l, c, r) = (d[(best + PROBES - 1) % PROBES], d[best], d[(best + 1) % PROBES]);
        let curve = l - 2.0 * c + r;
        let shift = if curve.is_finite() && curve > 0.0 { (0.5 * (l - r) / curve).clamp(-0.5, 0.5) } else { 0.0 };
        at(best as f64 + shift)
    }

    /// Whether every station of a march pair is fed, the closed side's distances counted back from its edge at `c`.
    fn tapered(&self, c: f64, first: f64, closed: &[(f64, f64)], body: &[(f64, f64)]) -> bool {
        self.fed(c, first) && closed.iter().all(|&(w, s)| self.fed(c - w, s)) && body.iter().all(|&(w, s)| self.fed(c + w, s))
    }

    /// Edge if a line through the mid-surface point leaves the metal one way and meets the floor the other within the reach, fed all the way.
    fn classify(&self, s: &Sample, t: f64, far: usize) -> Thin {
        let inward = scale(s.n, -1.0);
        let n_far = self.normal(far);
        let (mid, axis, first) = {
            let m = add(s.p, scale(inward, 0.5 * t));
            let a = unit(add(inward, n_far)).unwrap_or(inward);
            self.section(m, a).map_or((m, a, t), |(len, mid, turned)| (mid, turned, len))
        };
        // Section square to the mid-surface already at the floor.
        if first >= self.floor {
            return Thin { kind: ThinKind::Edge, mid, depth: Some(0.0) };
        }
        // Sum of the two faces' outward normals, toward the edge they close at; parallel faces probe for it instead.
        let toward = add(s.n, n_far);
        let converging = sub(toward, scale(axis, dot(toward, axis)));
        let e1 = if norm(converging) >= CONVERGING { scale(converging, 1.0 / norm(converging)) } else { self.nearest_exit(mid, axis) };
        let e2 = cross(axis, e1);
        let limit = self.reach + 0.5 * self.step;
        let (mut out, mut home) = (Vec::new(), Vec::new());
        for k in 0..DIRECTIONS / 2 {
            let angle = k as f64 * std::f64::consts::TAU / DIRECTIONS as f64;
            let u = add(scale(e1, angle.cos()), scale(e2, angle.sin()));
            let back = scale(u, -1.0);
            let depth = match self.walk(mid, axis, u, first, limit, None, &mut out) {
                Walk::Closed(c) => match self.walk(mid, axis, back, first, limit - c, Some(c), &mut home) {
                    Walk::Body(b) => self.tapered(c, first, &out, &home).then_some(c + b),
                    _ => None,
                },
                Walk::Body(b) => match self.walk(mid, axis, back, first, limit - b, None, &mut home) {
                    Walk::Closed(c) => self.tapered(c, first, &home, &out).then_some(c + b),
                    _ => None,
                },
                Walk::Thin => None,
            };
            if let Some(d) = depth.filter(|d| *d <= limit) {
                return Thin { kind: ThinKind::Edge, mid, depth: Some(d) };
            }
        }
        Thin { kind: ThinKind::Wall, mid, depth: None }
    }

    fn measure(&self, s: &Sample) -> Outcome {
        match self.read(s) {
            Reading::Internal => Outcome::Internal,
            Reading::Unresolved => Outcome::Unresolved,
            Reading::Section { t, far } => Outcome::Read { t, thin: (t < self.floor).then(|| self.classify(s, t, far)) },
        }
    }
}

/// A thin sample as the zones gather it.
struct ThinRead {
    kind: ThinKind,
    mid: P3,
    point: P3,
    t: f64,
    area: f64,
    depth: Option<f64>,
}

/// Samples of one kind whose mid-surface points stand within `link` of each other, as the [`MAX_ZONES`] largest zones and the area of all of them; `None` once `cancel` is raised.
fn zones(reads: &[ThinRead], kind: ThinKind, link: f64, cancel: Option<&AtomicBool>) -> Option<(Vec<ThinZone>, f64)> {
    let ids: Vec<usize> = (0..reads.len()).filter(|&i| reads[i].kind == kind).collect();
    let key = |p: P3| -> [i64; 3] { std::array::from_fn(|k| (p[k] / link).floor() as i64) };
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (slot, &i) in ids.iter().enumerate() {
        grid.entry(key(reads[i].mid)).or_default().push(slot);
    }
    let mut parent: Vec<usize> = (0..ids.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (slot, &i) in ids.iter().enumerate() {
        if slot % SAMPLES_PER_CHECK == 0 && stopped(cancel) {
            return None;
        }
        let c = key(reads[i].mid);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(near) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) else { continue };
                    for &other in near {
                        let d = sub(reads[ids[other]].mid, reads[i].mid);
                        if other > slot && dot(d, d) <= link * link {
                            let (a, b) = (find(&mut parent, slot), find(&mut parent, other));
                            if a != b {
                                parent[a.max(b)] = a.min(b);
                            }
                        }
                    }
                }
            }
        }
    }
    let mut by_root: HashMap<usize, (ThinZone, P3, P3)> = HashMap::new();
    let mut total = 0.0;
    for (slot, &i) in ids.iter().enumerate() {
        if slot % SAMPLES_PER_CHECK == 0 && stopped(cancel) {
            return None;
        }
        let r = &reads[i];
        total += r.area;
        let root = find(&mut parent, slot);
        let z = by_root.entry(root).or_insert_with(|| {
            (ThinZone { kind, area_mm2: 0.0, thinnest_mm: f64::INFINITY, point: r.point, samples: 0, depth_mm: None, span_mm: 0.0 }, r.mid, r.mid)
        });
        z.0.area_mm2 += r.area;
        z.0.samples += 1;
        if r.t < z.0.thinnest_mm {
            (z.0.thinnest_mm, z.0.point) = (r.t, r.point);
        }
        if let Some(d) = r.depth {
            z.0.depth_mm = Some(z.0.depth_mm.map_or(d, |old: f64| old.max(d)));
        }
        for k in 0..3 {
            z.1[k] = z.1[k].min(r.mid[k]);
            z.2[k] = z.2[k].max(r.mid[k]);
        }
    }
    let mut out: Vec<ThinZone> = by_root
        .into_values()
        .map(|(mut z, lo, hi)| {
            z.span_mm = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
            z
        })
        .collect();
    out.sort_by(|a, b| {
        b.area_mm2.total_cmp(&a.area_mm2).then(a.thinnest_mm.total_cmp(&b.thinnest_mm)).then(a.point.partial_cmp(&b.point).unwrap_or(std::cmp::Ordering::Equal))
    });
    out.truncate(MAX_ZONES);
    Some((out, total))
}

/// The pitch asked for, else a floor's eighth held to 0.02-0.1 mm, widened until a plane of `area` at any turn takes at most `most` cells.
fn pitch_for(asked: Option<f64>, floor: f64, area: f64, most: f64) -> f64 {
    asked.filter(|p| p.is_finite() && *p > 0.0).unwrap_or((floor / 8.0).clamp(0.02, 0.1)).max((OBLIQUE * area / most).sqrt())
}

/// The census of `mesh`; `None` once `cancel` is raised, read between stages, every [`PIECES_PER_CHECK`] pieces binned and every [`SAMPLES_PER_CHECK`] samples read.
pub(super) fn run(mesh: &Mesh, options: &CensusOptions, cancel: Option<&AtomicBool>) -> Option<Thickness> {
    let floor = options.floor_mm;
    let reach = options.edge_reach_mm.filter(|r| r.is_finite() && *r >= 0.0).unwrap_or(floor);
    let mut r = Thickness {
        sampled_min_mm: None,
        point: None,
        rays: 0,
        unresolved: 0,
        below_limit: 0,
        limit_mm: floor,
        note: NOT_ASSESSED,
        edge_below_limit: 0,
        internal: 0,
        assessed: false,
        area_mm2: 0.0,
        pitch_mm: 0.0,
        edge_reach_mm: reach,
        wall_area_mm2: 0.0,
        edge_area_mm2: 0.0,
        walls: Vec::new(),
        edges: Vec::new(),
    };
    if !(floor > 0.0 && floor.is_finite()) || mesh.faces.is_empty() {
        return Some(r);
    }
    if stopped(cancel) {
        return None;
    }
    if !mesh.validate().watertight {
        return Some(r);
    }
    if stopped(cancel) {
        return None;
    }
    r.area_mm2 = mesh
        .faces
        .iter()
        .filter_map(|f| mesh.triangle(f))
        .map(|(a, b, c)| 0.5 * norm(cross(sub(b, a), sub(c, a))))
        .filter(|a| a.is_finite())
        .sum();
    let pitch = pitch_for(options.pitch_mm, floor, r.area_mm2, MAX_SAMPLES);
    r.pitch_mm = pitch;
    let samples = sample(mesh, pitch, cancel)?;
    if stopped(cancel) {
        return None;
    }
    let probe = Probe::new(mesh, floor, reach);
    let mut outcomes: Vec<Outcome> = Vec::with_capacity(samples.len());
    for chunk in samples.chunks(SAMPLES_PER_CHECK) {
        if stopped(cancel) {
            return None;
        }
        #[cfg(feature = "parallel")]
        outcomes.par_extend(chunk.par_iter().map(|s| probe.measure(s)));
        #[cfg(not(feature = "parallel"))]
        outcomes.extend(chunk.iter().map(|s| probe.measure(s)));
    }
    let mut thin = Vec::new();
    for (s, o) in samples.iter().zip(&outcomes) {
        match *o {
            Outcome::Internal => r.internal += 1,
            Outcome::Unresolved => r.unresolved += 1,
            Outcome::Read { t, thin: class } => {
                r.rays += 1;
                if r.sampled_min_mm.is_none_or(|old| t < old) {
                    (r.sampled_min_mm, r.point) = (Some(t), Some(s.p));
                }
                if let Some(c) = class {
                    match c.kind {
                        ThinKind::Edge => r.edge_below_limit += 1,
                        ThinKind::Wall => r.below_limit += 1,
                    }
                    thin.push(ThinRead { kind: c.kind, mid: c.mid, point: s.p, t, area: s.area, depth: c.depth });
                }
            }
        }
    }
    let link = 3.0 * pitch;
    (r.walls, r.wall_area_mm2) = zones(&thin, ThinKind::Wall, link, cancel)?;
    (r.edges, r.edge_area_mm2) = zones(&thin, ThinKind::Edge, link, cancel)?;
    r.assessed = true;
    r.note = NOTE;
    Some(r)
}

#[cfg(test)]
mod tests;
