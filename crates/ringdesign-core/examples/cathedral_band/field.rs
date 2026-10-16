//! Sampled fields: the opening that rounds every pocket's corners, contours, and outline clean-up.
use crate::geom::{add, len, mul, seg_dist, sub, V};

pub struct Grid {
    pub x0: f64,
    pub y0: f64,
    pub step: f64,
    pub nx: usize,
    pub ny: usize,
}

impl Grid {
    pub fn at(&self, i: usize, j: usize) -> V {
        [self.x0 + i as f64 * self.step, self.y0 + j as f64 * self.step]
    }
    pub fn x1(&self) -> f64 {
        self.x0 + (self.nx - 1) as f64 * self.step
    }
}

/// Squared Euclidean distance, in cells, from every cell to the nearest cell where `feature` holds.
fn edt(g: &Grid, feature: &[bool]) -> Vec<f64> {
    const INF: f64 = 1e20;
    let (nx, ny) = (g.nx, g.ny);
    let mut d: Vec<f64> = feature.iter().map(|&f| if f { 0.0 } else { INF }).collect();
    let n_max = nx.max(ny);
    let (mut f, mut out, mut v, mut z) = (vec![0.0; n_max], vec![0.0; n_max], vec![0usize; n_max], vec![0.0; n_max + 1]);
    let mut pass = |d: &mut Vec<f64>, n: usize, idx: &dyn Fn(usize) -> usize| {
        for q in 0..n {
            f[q] = d[idx(q)];
        }
        let mut k = 0usize;
        v[0] = 0;
        z[0] = -INF;
        z[1] = INF;
        for q in 1..n {
            loop {
                let s = ((f[q] + (q * q) as f64) - (f[v[k]] + (v[k] * v[k]) as f64)) / (2.0 * q as f64 - 2.0 * v[k] as f64);
                if s <= z[k] && k > 0 {
                    k -= 1;
                    continue;
                }
                if s <= z[k] {
                    v[0] = q;
                    z[0] = -INF;
                    z[1] = INF;
                    break;
                }
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = INF;
                break;
            }
        }
        k = 0;
        for q in 0..n {
            while z[k + 1] < q as f64 {
                k += 1;
            }
            let dq = q as f64 - v[k] as f64;
            out[q] = dq * dq + f[v[k]];
        }
        for q in 0..n {
            d[idx(q)] = out[q];
        }
    };
    for j in 0..ny {
        pass(&mut d, nx, &|i| j * nx + i);
    }
    for i in 0..nx {
        pass(&mut d, ny, &|j| j * nx + i);
    }
    d
}

/// `field` with every positive region opened by `r`: thinner than `2r` gone, convex corners rounded to `r`.
pub fn opened(g: &Grid, field: &[f64], r: f64) -> Vec<f64> {
    let cells = r / g.step;
    let outside: Vec<bool> = field.iter().map(|v| *v <= 0.0).collect();
    let d1 = edt(g, &outside);
    let eroded: Vec<bool> = d1.iter().map(|d| d.sqrt() > cells).collect();
    let d2 = edt(g, &eroded);
    let bias = 1.5 * g.step;
    field.iter().zip(&d2).map(|(m, d)| m.min(r - d.sqrt() * g.step + bias)).collect()
}

/// Closed contours where `field` crosses zero, each running counter-clockwise round the positive side.
pub fn contours(g: &Grid, field: &[f64]) -> Vec<Vec<V>> {
    use std::collections::HashMap;
    let (nx, ny) = (g.nx, g.ny);
    let val = |i: usize, j: usize| field[j * nx + i];
    let key_h = |i: usize, j: usize| 2 * (j * nx + i);
    let key_v = |i: usize, j: usize| 2 * (j * nx + i) + 1;
    let cross = |a: (usize, usize), b: (usize, usize)| -> V {
        let (fa, fb) = (val(a.0, a.1), val(b.0, b.1));
        let t = fa / (fa - fb);
        let (pa, pb) = (g.at(a.0, a.1), g.at(b.0, b.1));
        [pa[0] + t * (pb[0] - pa[0]), pa[1] + t * (pb[1] - pa[1])]
    };
    let mut next: HashMap<usize, (usize, V)> = HashMap::new();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let c = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let pos: [bool; 4] = std::array::from_fn(|k| val(c[k].0, c[k].1) > 0.0);
            if pos.iter().all(|&b| b) || pos.iter().all(|&b| !b) {
                continue;
            }
            // Cell edges counter-clockwise: bottom, right, top, left.
            let edges = [key_h(i, j), key_v(i + 1, j), key_h(i, j + 1), key_v(i, j)];
            let (mut exits, mut enters) = (Vec::new(), Vec::new());
            for e in 0..4 {
                let (a, b) = (e, (e + 1) % 4);
                if pos[a] != pos[b] {
                    let point = cross(c[a], c[b]);
                    if pos[a] { exits.push((e, edges[e], point)) } else { enters.push((e, edges[e], point)) }
                }
            }
            if exits.len() == 1 {
                next.insert(exits[0].1, (enters[0].1, exits[0].2));
                continue;
            }
            let centre = c.iter().map(|q| val(q.0, q.1)).sum::<f64>() * 0.25;
            for &(e, key, point) in &exits {
                let want = if centre > 0.0 { (e + 1) % 4 } else { (e + 3) % 4 };
                let to = enters.iter().find(|x| x.0 == want).expect("saddle pairing");
                next.insert(key, (to.1, point));
            }
        }
    }
    let mut loops = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut keys: Vec<usize> = next.keys().copied().collect();
    keys.sort_unstable();
    for start in keys {
        if seen.contains(&start) {
            continue;
        }
        let mut pts = Vec::new();
        let mut k = start;
        while seen.insert(k) {
            let Some(&(to, point)) = next.get(&k) else { break };
            pts.push(point);
            k = to;
        }
        if pts.len() >= 3 {
            loops.push(pts);
        }
    }
    loops
}

pub fn area(pts: &[V]) -> f64 {
    0.5 * (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
}

/// Douglas-Peucker on a closed loop, keeping points further than `tol` from the chord.
pub fn simplify(pts: &[V], tol: f64) -> Vec<V> {
    fn rec(pts: &[V], tol: f64, out: &mut Vec<V>) {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (mut worst, mut at) = (0.0, 0);
        for (k, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
            let d = seg_dist(*p, a, b);
            if d > worst {
                worst = d;
                at = k;
            }
        }
        if worst > tol {
            rec(&pts[..=at], tol, out);
            rec(&pts[at..], tol, out);
        } else {
            out.push(a);
        }
    }
    let n = pts.len();
    let far = (0..n).max_by(|i, j| len(sub(pts[*i], pts[0])).total_cmp(&len(sub(pts[*j], pts[0])))).unwrap_or(0);
    let mut out = Vec::new();
    rec(&pts[..=far], tol, &mut out);
    let mut back: Vec<V> = pts[far..].to_vec();
    back.push(pts[0]);
    rec(&back, tol, &mut out);
    out
}

/// The closed loop with points inserted so no edge runs longer than `step`.
pub fn densify(pts: &[V], step: f64) -> Vec<V> {
    let mut out = Vec::with_capacity(pts.len() * 2);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let n = (len(sub(b, a)) / step).ceil().max(1.0) as usize;
        for k in 0..n {
            out.push(add(a, mul(sub(b, a), k as f64 / n as f64)));
        }
    }
    out
}

pub fn bounds(pts: &[V]) -> (V, V) {
    pts.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])]))
}
