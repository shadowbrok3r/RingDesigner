// Reading a real ring off a mesh: split, align, and measure the same things
// `BandProfile` and `ShankStyle` are made of.
#![allow(dead_code)]

use std::collections::HashMap;

use ringdesign_core::mesh::{Mesh, Vec3};

pub fn read_stl(path: &str) -> Mesh {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert!(b.len() > 84, "{path}: too short for a binary STL");
    let n = u32::from_le_bytes(b[80..84].try_into().unwrap()) as usize;
    assert_eq!(b.len(), 84 + n * 50, "{path}: not a binary STL");
    let f32_at = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());

    // Weld by quantized position: an STL has no vertex sharing at all, and
    // without it there are no connected components to find.
    let mut index: HashMap<[i64; 3], u32> = HashMap::new();
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for t in 0..n {
        let base = 84 + t * 50 + 12;
        let mut tri = [0u32; 3];
        for (k, slot) in tri.iter_mut().enumerate() {
            let p = [f32_at(base + k * 12), f32_at(base + k * 12 + 4), f32_at(base + k * 12 + 8)];
            let key = [
                (p[0] as f64 * 1e4).round() as i64,
                (p[1] as f64 * 1e4).round() as i64,
                (p[2] as f64 * 1e4).round() as i64,
            ];
            *slot = *index.entry(key).or_insert_with(|| {
                vertices.push(Vec3(p[0], p[1], p[2]));
                (vertices.len() - 1) as u32
            });
        }
        if tri[0] != tri[1] && tri[1] != tri[2] && tri[2] != tri[0] {
            faces.push(tri);
        }
    }
    Mesh { vertices, normals: Vec::new(), faces }
}

/// Split into connected pieces. These files hold one ring per size.
pub fn components(m: &Mesh) -> Vec<Mesh> {
    let mut parent: Vec<u32> = (0..m.vertices.len() as u32).collect();
    fn find(p: &mut [u32], mut i: u32) -> u32 {
        while p[i as usize] != i {
            p[i as usize] = p[p[i as usize] as usize];
            i = p[i as usize];
        }
        i
    }
    for f in &m.faces {
        for k in 0..3 {
            let (a, b) = (find(&mut parent, f[k]), find(&mut parent, f[(k + 1) % 3]));
            if a != b {
                parent[a as usize] = b;
            }
        }
    }
    let mut groups: HashMap<u32, Vec<[u32; 3]>> = HashMap::new();
    for f in &m.faces {
        groups.entry(find(&mut parent, f[0])).or_default().push(*f);
    }
    let mut out: Vec<Mesh> = groups
        .into_values()
        .map(|faces| {
            let mut remap: HashMap<u32, u32> = HashMap::new();
            let mut vertices = Vec::new();
            let faces = faces
                .into_iter()
                .map(|f| {
                    f.map(|i| {
                        *remap.entry(i).or_insert_with(|| {
                            vertices.push(m.vertices[i as usize]);
                            (vertices.len() - 1) as u32
                        })
                    })
                })
                .collect();
            Mesh { vertices, normals: Vec::new(), faces }
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.faces.len()));
    out
}

/// A ring read off a mesh, put in the app's own frame: bore on the origin, its
/// axis along Z, the head at 90 degrees.
pub struct Scan {
    pub mesh: Mesh,
    pub bore_r: f64,
    /// Width, crest radius and bore radius per degree off the head.
    pub steps: Vec<Step>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Step {
    pub deg: f64,
    pub width: f64,
    pub crest: f64,
    pub bore: f64,
    /// Where the band's mid-plane sits along the finger, mm.
    pub centre: f64,
}

const BUCKETS: usize = 720;

impl Scan {
    pub fn of(m: &Mesh) -> Self {
        let pts: Vec<[f64; 3]> =
            m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();

        // The finger axis is whichever the ring is annular about: looking down
        // it, *every* direction has to be empty out to the bore. Looking down
        // any other, the ring is seen edge-on and the middle of the picture is
        // solid metal, so some direction reaches the centre.
        let axis = (0..3).max_by(|&a, &b| hollow(&pts, a).total_cmp(&hollow(&pts, b))).unwrap();
        let (u, v) = ([1, 0, 0][axis], [2, 2, 1][axis]);
        let (cx, cy) = fit_bore(&pts, u, v);

        // Rotate the chosen axis into Z, keeping the frame right-handed.
        let to_ring = |p: &[f64; 3]| [p[u] - cx, p[v] - cy, p[axis]];
        let mut r = Self {
            mesh: Mesh {
                vertices: m
                    .vertices
                    .iter()
                    .map(|q| {
                        let p = to_ring(&[q.0 as f64, q.1 as f64, q.2 as f64]);
                        Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
                    })
                    .collect(),
                normals: Vec::new(),
                faces: m.faces.clone(),
            },
            bore_r: 0.0,
            steps: Vec::new(),
        };
        r.measure();
        // Put the head at the top and the shank's mid-plane on z = 0, then
        // measure again in that frame. The band's own centre, not the whole
        // ring's: an upright head reaches further one way across the finger
        // than the other, and centring on the extremes would tilt everything.
        let coarse = r
            .steps
            .iter()
            .max_by(|a, b| a.width.total_cmp(&b.width))
            .map(|s| s.deg)
            .unwrap_or(0.0);
        // Refine on the head's own symmetry rather than on the widest bucket.
        // A broad head has a flat maximum, so the argmax can sit degrees off,
        // and everything read as "so far off the head" is wrong by that much.
        let width = |d: f64| r.steps[bucket(d.to_radians())].width;
        let mut head = coarse;
        let mut best = f64::MAX;
        for i in -100..=100 {
            let c = coarse + i as f64 * 0.05;
            let err: f64 = (1..=120)
                .map(|k| (width(c + k as f64 * 0.5) - width(c - k as f64 * 0.5)).powi(2))
                .sum();
            if err < best {
                (best, head) = (err, c);
            }
        }
        let back = r.steps[bucket((head + 180.0).to_radians())].centre;
        let turn = (90.0f64 - head).to_radians();
        let (s, c) = turn.sin_cos();
        for q in r.mesh.vertices.iter_mut() {
            let (x, y) = (q.0 as f64, q.1 as f64);
            *q = Vec3((x * c - y * s) as f32, (x * s + y * c) as f32, q.2 - back as f32);
        }
        r.measure();
        r
    }

    fn measure(&mut self) {
        let mut lo = vec![f64::MAX; BUCKETS];
        let mut hi = vec![f64::MIN; BUCKETS];
        let mut r_max = vec![0.0f64; BUCKETS];
        let mut r_min = vec![f64::MAX; BUCKETS];
        // Along the edges, not at the vertices. A ring's shank is tessellated
        // far more coarsely than half a degree, so vertices alone leave buckets
        // empty and the band reads as zero wide there. Every extreme of a linear
        // function over a triangle is on its boundary, so edges are enough.
        for f in &self.mesh.faces {
            let Some((a, b, c)) = self.mesh.triangle(f) else { continue };
            for (p, q) in [(a, b), (b, c), (c, a)] {
                let len = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2))
                    .sqrt();
                let n = ((len / 0.08).ceil() as usize).clamp(1, 64);
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let (x, y, z) = (
                        p[0] + (q[0] - p[0]) * t,
                        p[1] + (q[1] - p[1]) * t,
                        p[2] + (q[2] - p[2]) * t,
                    );
                    let rad = x.hypot(y);
                    let i = bucket(y.atan2(x));
                    lo[i] = lo[i].min(z);
                    hi[i] = hi[i].max(z);
                    r_max[i] = r_max[i].max(rad);
                    r_min[i] = r_min[i].min(rad);
                }
            }
        }
        // The median rather than the least: a comfort-fit bore is a dome, so its
        // narrowest point is one station and not the size of the ring.
        self.bore_r = median(&mut r_min.clone());
        self.steps = (0..BUCKETS)
            .map(|i| Step {
                deg: i as f64 * 360.0 / BUCKETS as f64,
                width: (hi[i] - lo[i]).max(0.0),
                crest: r_max[i],
                bore: r_min[i],
                centre: 0.5 * (hi[i] + lo[i]),
            })
            .collect();
    }

    /// Read at a signed angle off the head, averaging the two sides.
    pub fn at(&self, off_deg: f64) -> Step {
        let one = |d: f64| self.steps[bucket((90.0 + d).to_radians())];
        let (a, b) = (one(off_deg), one(-off_deg));
        Step {
            deg: off_deg,
            width: 0.5 * (a.width + b.width),
            crest: 0.5 * (a.crest + b.crest),
            bore: 0.5 * (a.bore + b.bore),
            centre: 0.5 * (a.centre + b.centre),
        }
    }

    /// The cross-section at an angle off the head, as outer radius per station
    /// along the finger. `None` where the section has no metal.
    pub fn section(&self, off_deg: f64, bins: usize, half: f64) -> Vec<Option<(f64, f64)>> {
        let a = (90.0 + off_deg).to_radians();
        let (sa, ca) = a.sin_cos();
        let mut out = vec![None; bins];
        let mut put = |z: f64, rad: f64| {
            let i = (((z + half) / (2.0 * half) * bins as f64) as isize)
                .clamp(0, bins as isize - 1) as usize;
            let slot = out[i].get_or_insert((f64::MAX, 0.0f64));
            slot.0 = slot.0.min(rad);
            slot.1 = slot.1.max(rad);
        };
        // Every triangle crossing the half-plane through the axis at this angle
        // leaves a segment of the section. Walking the segment and not just its
        // ends is what fills the picture: a facet can span several stations.
        for f in &self.mesh.faces {
            let Some((p, q, r)) = self.mesh.triangle(f) else { continue };
            let side = |v: [f64; 3]| v[1] * ca - v[0] * sa;
            let tri = [p, q, r];
            let mut ends: Vec<(f64, f64)> = Vec::new();
            for k in 0..3 {
                let (m, n) = (tri[k], tri[(k + 1) % 3]);
                let (sm, sn) = (side(m), side(n));
                if (sm > 0.0) == (sn > 0.0) {
                    continue;
                }
                let t = sm / (sm - sn);
                let hit =
                    [m[0] + (n[0] - m[0]) * t, m[1] + (n[1] - m[1]) * t, m[2] + (n[2] - m[2]) * t];
                let rad = hit[0] * ca + hit[1] * sa;
                if rad > 0.0 {
                    ends.push((hit[2], rad));
                }
            }
            for w in ends.windows(2) {
                let (a, b) = (w[0], w[1]);
                let n = (((a.0 - b.0).abs() / (2.0 * half) * bins as f64).ceil() as usize).max(1);
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    put(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                }
            }
        }
        out
    }
}

fn bucket(a: f64) -> usize {
    ((a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * BUCKETS as f64) as usize)
        .min(BUCKETS - 1)
}

/// How hollow the ring looks down this axis: the smallest inner radius any
/// direction leaves, against the largest radius anywhere. Scale-free, so it
/// cannot be gamed by a centre that has wandered off — which the variance of
/// the inner radius could be, and was. From far enough away every direction
/// looks the same, so "most even" picked whichever fit had diverged worst.
fn hollow(pts: &[[f64; 3]], axis: usize) -> f64 {
    let (u, v) = ([1, 0, 0][axis], [2, 2, 1][axis]);
    let (cx, cy) = fit_bore(pts, u, v);
    let (mut r_min, mut r_max) = (vec![f64::MAX; 180], 0.0f64);
    for p in pts {
        let (dx, dy) = (p[u] - cx, p[v] - cy);
        let r = dx.hypot(dy);
        let i = ((dy.atan2(dx).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * 180.0)
            as usize)
            .min(179);
        r_min[i] = r_min[i].min(r);
        r_max = r_max.max(r);
    }
    // A low percentile rather than the minimum, so one stray vertex in the
    // bore's chamfer cannot decide it.
    r_min.sort_by(f64::total_cmp);
    r_min[9] / r_max.max(1e-9)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Kasa circle fit to the innermost point per direction — the bore.
///
/// One point per bucket so an uneven mesh cannot vote twice, and a circle fit
/// rather than a centroid because a chamfer at the bore's edge makes sure the
/// samples are not evenly spread.
fn fit_bore(pts: &[[f64; 3]], u: usize, v: usize) -> (f64, f64) {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in pts {
        for (k, a) in [u, v].into_iter().enumerate() {
            lo[k] = lo[k].min(p[a]);
            hi[k] = hi[k].max(p[a]);
        }
    }
    let (mut cx, mut cy) = (0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]));
    for _ in 0..40 {
        const B: usize = 360;
        let mut best = [None::<[f64; 2]>; B];
        for p in pts {
            let (dx, dy) = (p[u] - cx, p[v] - cy);
            let rad = dx.hypot(dy);
            let i = ((dy.atan2(dx).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU
                * B as f64) as usize)
                .min(B - 1);
            if best[i].map(|q| (q[0] - cx).hypot(q[1] - cy) > rad).unwrap_or(true) {
                best[i] = Some([p[u], p[v]]);
            }
        }
        let found: Vec<[f64; 2]> = best.into_iter().flatten().collect();
        let k = found.len() as f64;
        let (mx, my) = (
            found.iter().map(|q| q[0]).sum::<f64>() / k,
            found.iter().map(|q| q[1]).sum::<f64>() / k,
        );
        let (mut suu, mut svv, mut suv, mut suz, mut svz) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for q in &found {
            let (a, b) = (q[0] - mx, q[1] - my);
            let z = a * a + b * b;
            suu += a * a;
            svv += b * b;
            suv += a * b;
            suz += a * z;
            svz += b * z;
        }
        let det = suu * svv - suv * suv;
        if det.abs() < 1e-9 {
            break;
        }
        // Held inside the bounding box: down a wrong axis the innermost points
        // are not a circle at all and the fit runs away, which then reads as a
        // perfectly even bore seen from a great distance.
        cx = (mx + 0.5 * (suz * svv - svz * suv) / det).clamp(lo[0], hi[0]);
        cy = (my + 0.5 * (svz * suu - suz * suv) / det).clamp(lo[1], hi[1]);
    }
    (cx, cy)
}
