//! A rolling-ball bead along the seam where a part meets the band, joined through [`csg`].
//!
//! No pure-Rust B-rep kernel fillets a fused junction, so the round is built as a solid of its own.
//! At every station along a seam loop a ball of the bead's radius is settled tangent to both
//! surfaces, its section is the arc between the two touch points closed by a vertex pushed into the
//! material, and the sections are swept round the loop into a torus that a union lays into a
//! concave seam or a subtraction takes off a convex rim. The arc's radius is the ball's plus a few
//! microns so it crosses each surface at a small angle instead of touching it, which an exact
//! boolean cannot resolve; the round lands that far toward the corner, shy of the true fillet on a
//! union and proud of it on a cut. Where the seam turns tighter than the section reaches, or the
//! two surfaces close toward a crevice, the radius is clamped and the station counted; and the
//! swept solid is checked for faces crossing each other, with the stations of any crossing
//! shrunk and the sweep rebuilt until none cross, since a seam corner turns the ball against a
//! third surface and no curvature estimate sees that coming.
//!
//! Three-edge corners are not blended: a seam that turns a corner pinches the bead toward
//! [`RADIUS_MIN_MM`] there.
use crate::csg::{self, Op, Parent, Solid, Traced};
use std::collections::HashMap;

pub type P3 = [f64; 3];

/// Station spacing along the seam, capped at a third of the radius.
pub const STATION_MM: f64 = 0.05;
/// A station's ball may stand at most this many radii from the seam; a sharper crevice shrinks the radius.
pub const REACH_MAX: f64 = 4.0;
/// A section may reach at most this share of the seam's local radius toward its centre of curvature.
pub const CURVE_FRACTION: f64 = 0.8;
/// Largest change of radius per millimetre of seam.
pub const SLOPE: f64 = 0.5;
/// The radius no guard shrinks below, so every section stays a polygon.
pub const RADIUS_MIN_MM: f64 = 0.02;
/// Angular step of the arc; the chord sag at 3° is under the arc's proudness.
const ARC_STEP_DEG: f64 = 6.0;
/// How far past the seam the closing vertex sits, in radii.
const DEPTH_FRACTION: f64 = 0.5;
/// Sliver tolerance handed to `csg::clean` after each join.
const CLEAN_MM: f64 = 2e-5;
/// Rounds of shrinking the stations whose faces cross and re-sweeping before giving up.
const FOLD_ROUNDS: usize = 8;

/// A swept seam bead.
#[derive(Clone, Debug)]
pub struct Bead {
    pub solid: Solid,
    /// Stations whose radius a guard reduced.
    pub clamped: usize,
    pub stations: usize,
    /// The smallest radius laid.
    pub min_radius_mm: f64,
    /// Stations whose ball could not be settled against the faces and kept the planar estimate.
    pub unrefined: usize,
    /// Stations shrunk because the sweep crossed itself there.
    pub folded: usize,
}

/// One seam loop with the normals of each side at every vertex, as [`bead`] wants them.
#[derive(Clone, Debug)]
pub struct Seam {
    pub points: Vec<P3>,
    pub normals_a: Vec<P3>,
    pub normals_b: Vec<P3>,
}

/// A junction filleted, with what the beads did.
#[derive(Clone, Debug)]
pub struct Junction {
    pub solid: Solid,
    pub loops: usize,
    /// Loops too short to bead.
    pub skipped: usize,
    pub stations: usize,
    pub clamped: usize,
    pub unrefined: usize,
    pub folded: usize,
    pub min_radius_mm: f64,
}

fn sub(a: P3, b: P3) -> P3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: P3, b: P3) -> P3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: P3, s: f64) -> P3 { [a[0] * s, a[1] * s, a[2] * s] }
fn dot(a: P3, b: P3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: P3, b: P3) -> P3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: P3) -> f64 { dot(a, a).sqrt() }
fn unit(a: P3) -> P3 {
    let l = norm(a);
    if l > 1e-300 { scale(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}
/// `a` with its component along the unit vector `t` removed.
fn perp(a: P3, t: P3) -> P3 { sub(a, scale(t, dot(a, t))) }
fn face_normal(v: &[P3], f: [u32; 3]) -> P3 {
    let [a, b, c] = f.map(|k| v[k as usize]);
    unit(cross(sub(b, a), sub(c, a)))
}

/// Closest point on a triangle to `p`.
fn closest_on_tri(p: P3, [a, b, c]: [P3; 3]) -> P3 {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return add(a, scale(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return add(a, scale(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return add(b, scale(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let denom = 1.0 / (va + vb + vc);
    add(a, add(scale(ab, vb * denom), scale(ac, vc * denom)))
}

/// One side's triangles in a hash grid for closest-point queries.
struct Patch {
    tris: Vec<[P3; 3]>,
    cell: f64,
    cells: HashMap<[i32; 3], Vec<u32>>,
}

impl Patch {
    fn new(tris: Vec<[P3; 3]>, cell: f64) -> Self {
        let cell = cell.max(1e-3);
        let mut cells: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let (lo, hi) = tri_bounds(*t);
            let (lo, hi) = (Self::at(lo, cell), Self::at(hi, cell));
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        cells.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        Self { tris, cell, cells }
    }

    fn at(p: P3, cell: f64) -> [i32; 3] { p.map(|x| (x / cell).floor() as i32) }

    /// The closest point of the patch within `within` of `p`, with its distance.
    fn closest(&self, p: P3, within: f64) -> Option<(P3, f64)> {
        let lo = Self::at(sub(p, [within; 3]), self.cell);
        let hi = Self::at(add(p, [within; 3]), self.cell);
        let mut best: Option<(P3, f64)> = None;
        let mut seen = std::collections::HashSet::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let Some(list) = self.cells.get(&[x, y, z]) else { continue };
                    for &i in list {
                        if !seen.insert(i) {
                            continue;
                        }
                        let q = closest_on_tri(p, self.tris[i as usize]);
                        let d = norm(sub(q, p));
                        if d <= within && best.is_none_or(|(_, b)| d < b) {
                            best = Some((q, d));
                        }
                    }
                }
            }
        }
        best
    }
}

fn tri_bounds(t: [P3; 3]) -> (P3, P3) {
    (std::array::from_fn(|k| t[0][k].min(t[1][k]).min(t[2][k])), std::array::from_fn(|k| t[0][k].max(t[1][k]).max(t[2][k])))
}

/// The two surfaces a bead settles against: the joined solid's faces of each side near a seam.
pub struct Surfaces {
    a: Patch,
    b: Patch,
}

impl Surfaces {
    /// The faces of each side whose box lies within `pad` of the seam's box.
    pub fn near(t: &Traced, seam: &[P3], pad: f64) -> Self {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in seam {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k] - pad);
                hi[k] = hi[k].max(p[k] + pad);
            }
        }
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for (f, parent) in t.solid.f.iter().zip(&t.parent) {
            let tri = f.map(|k| t.solid.v[k as usize]);
            let (tlo, thi) = tri_bounds(tri);
            if (0..3).any(|k| thi[k] < lo[k] || tlo[k] > hi[k]) {
                continue;
            }
            match parent {
                Parent::A(_) => a.push(tri),
                Parent::B(_) => b.push(tri),
            }
        }
        Self { a: Patch::new(a, pad), b: Patch::new(b, pad) }
    }
}

/// A station along the resampled seam: its frame, the seam's curvature, and the ball settled there.
struct Station {
    p: P3,
    t: P3,
    e1: P3,
    e2: P3,
    /// Toward the seam's centre of curvature, in the section plane.
    n_in: P3,
    r_seam: f64,
    na: P3,
    nb: P3,
    r: f64,
    c: P3,
    /// Centre to the touch point on each side.
    ua: P3,
    ub: P3,
    refined: bool,
}

/// Resample a closed loop at even arc length, normals interpolated along it.
fn resample(seam: &[P3], na: &[P3], nb: &[P3], h: f64) -> (Vec<P3>, Vec<P3>, Vec<P3>, f64) {
    let n_in = seam.len();
    let seg: Vec<f64> = (0..n_in).map(|i| norm(sub(seam[(i + 1) % n_in], seam[i]))).collect();
    let total: f64 = seg.iter().sum();
    let n = ((total / h).ceil() as usize).max(8);
    let step = total / n as f64;
    let (mut pts, mut nas, mut nbs) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut i, mut start) = (0usize, 0.0);
    for k in 0..n {
        let s = k as f64 * step;
        while i + 1 < n_in && start + seg[i] < s {
            start += seg[i];
            i += 1;
        }
        let f = if seg[i] > 1e-300 { ((s - start) / seg[i]).clamp(0.0, 1.0) } else { 0.0 };
        let j = (i + 1) % n_in;
        pts.push(add(seam[i], scale(sub(seam[j], seam[i]), f)));
        nas.push(unit(add(scale(na[i], 1.0 - f), scale(na[j], f))));
        nbs.push(unit(add(scale(nb[i], 1.0 - f), scale(nb[j], f))));
    }
    (pts, nas, nbs, step)
}

/// Stations with tangents and curvature smoothed over a window of `w` either side.
fn stations(pts: &[P3], nas: &[P3], nbs: &[P3], step: f64, w: f64, r: f64) -> Vec<Station> {
    let n = pts.len();
    let k = ((w / step).round() as usize).clamp(1, (n / 4).max(1));
    let at = |i: isize| pts[i.rem_euclid(n as isize) as usize];
    let tangents: Vec<P3> = (0..n as isize).map(|i| unit(sub(at(i + k as isize), at(i - k as isize)))).collect();
    (0..n)
        .map(|i| {
            let t = tangents[i];
            let dt = sub(tangents[(i + k) % n], tangents[(i + n - k) % n]);
            let kappa = norm(perp(dt, t)) / (2.0 * k as f64 * step);
            let n_in = if kappa > 1e-9 { unit(perp(dt, t)) } else { [0.0; 3] };
            let r_seam = if kappa > 1e-9 { 1.0 / kappa } else { f64::INFINITY };
            let (na, nb) = (nas[i], nbs[i]);
            let mut e1 = perp(na, t);
            if norm(e1) < 1e-6 {
                e1 = perp(if t[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }, t);
            }
            let e1 = unit(e1);
            let e2 = cross(t, e1);
            Station { p: pts[i], t, e1, e2, n_in, r_seam, na, nb, r, c: pts[i], ua: scale(na, -1.0), ub: scale(nb, -1.0), refined: false }
        })
        .collect()
}

impl Station {
    /// The ball centre from the two normals as planes, and the touch directions with it.
    fn planar(&mut self) {
        let s = add(self.na, self.nb);
        let d = (1.0 + dot(self.na, self.nb)).max(1e-6);
        let c = add(self.p, scale(s, self.r / d));
        self.c = sub(c, scale(self.t, dot(self.t, sub(c, self.p))));
        self.ua = unit(perp(scale(self.na, -1.0), self.t));
        self.ub = unit(perp(scale(self.nb, -1.0), self.t));
        self.refined = false;
    }

    /// Settle the ball at distance `r` from both patches inside the section plane; false leaves the planar estimate.
    fn refine(&mut self, s: &Surfaces) -> bool {
        let r = self.r;
        let within = REACH_MAX * r + 0.05;
        let mut c = self.c;
        let mut touched = None;
        for _ in 0..12 {
            let (Some((qa, da)), Some((qb, db))) = (s.a.closest(c, within), s.b.closest(c, within)) else { return false };
            if da < 1e-9 || db < 1e-9 {
                return false;
            }
            let (ua, ub) = (scale(sub(c, qa), 1.0 / da), scale(sub(c, qb), 1.0 / db));
            touched = Some((qa, qb));
            if (r - da).abs() < 1e-8 && (r - db).abs() < 1e-8 {
                break;
            }
            let m = [[dot(ua, self.e1), dot(ua, self.e2)], [dot(ub, self.e1), dot(ub, self.e2)]];
            let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
            if det.abs() < 1e-4 {
                return false;
            }
            let rhs = [r - da, r - db];
            let x = (rhs[0] * m[1][1] - rhs[1] * m[0][1]) / det;
            let y = (m[0][0] * rhs[1] - m[1][0] * rhs[0]) / det;
            let mut step = add(scale(self.e1, x), scale(self.e2, y));
            let l = norm(step);
            if l > r {
                step = scale(step, r / l);
            }
            c = add(c, step);
        }
        let Some((qa, qb)) = touched else { return false };
        let (da, db) = (norm(sub(c, qa)), norm(sub(c, qb)));
        if (da - r).abs() > 1e-4 * r.max(0.1) || (db - r).abs() > 1e-4 * r.max(0.1) {
            return false;
        }
        let (ua, ub) = (unit(perp(sub(qa, c), self.t)), unit(perp(sub(qb, c), self.t)));
        if dot(ua, ub) < -0.9999 {
            return false;
        }
        self.c = c;
        self.ua = ua;
        self.ub = ub;
        self.refined = true;
        true
    }

    /// The section's polygon: the arc from the A touch to the B touch, then the closing vertex.
    fn section(&self, m_arc: usize, delta: f64) -> Vec<P3> {
        let (psi, w) = self.arc();
        let mut out = Vec::with_capacity(m_arc + 2);
        for j in 0..=m_arc {
            let th = psi * j as f64 / m_arc as f64;
            out.push(add(self.c, scale(add(scale(self.ua, th.cos()), scale(w, th.sin())), self.r + delta)));
        }
        let bis = add(scale(self.ua, (psi / 2.0).cos()), scale(w, (psi / 2.0).sin()));
        out.push(add(self.p, scale(bis, DEPTH_FRACTION * self.r)));
        out
    }

    /// The arc's angle and its in-plane direction perpendicular to `ua`, spread to at least 2° so no section is flat.
    fn arc(&self) -> (f64, P3) {
        let cos = dot(self.ua, self.ub).clamp(-1.0, 1.0);
        let mut w = perp(self.ub, self.ua);
        if norm(w) < 1e-6 {
            w = cross(self.t, self.ua);
        }
        (cos.acos().max(2.0_f64.to_radians()), unit(w))
    }

    /// How far the section reaches from the seam toward the seam's centre of curvature.
    fn reach_in(&self, m_arc: usize, delta: f64) -> f64 {
        if self.r_seam.is_infinite() {
            return 0.0;
        }
        self.section(m_arc, delta).iter().map(|v| dot(sub(*v, self.p), self.n_in)).fold(0.0, f64::max)
    }
}

/// The arc stands this much proud of the ball so it crosses each surface at 5–11°.
fn proud(r: f64) -> f64 { (0.01 * r).clamp(2e-3, 4e-3) }

fn arc_steps(st: &[Station]) -> usize {
    let psi = st.iter().map(|s| s.arc().0).fold(0.0, f64::max);
    ((psi.to_degrees() / ARC_STEP_DEG).ceil() as usize).max(3)
}

/// The bead round a closed seam loop from the normals alone: exact wherever the surfaces are flat
/// across the seam, an estimate where they curve. `normals_a`/`normals_b` point into the side the
/// ball sits on: the outward normals of the joined solid for a concave seam that takes the bead as
/// a union, their negation for a convex rim that loses it as a cut.
pub fn bead(seam: &[P3], normals_a: &[P3], normals_b: &[P3], radius_mm: f64) -> Result<Bead, String> {
    bead_against(seam, normals_a, normals_b, radius_mm, None)
}

/// [`bead`] with the ball settled against the faces of each side at every station.
pub fn bead_against(seam: &[P3], normals_a: &[P3], normals_b: &[P3], radius_mm: f64, surfaces: Option<&Surfaces>) -> Result<Bead, String> {
    if seam.len() < 3 {
        return Err(format!("a seam of {} points is not a loop", seam.len()));
    }
    if normals_a.len() != seam.len() || normals_b.len() != seam.len() {
        return Err("one normal per side per seam point".into());
    }
    if !(radius_mm > 0.0) || !radius_mm.is_finite() {
        return Err(format!("bead radius {radius_mm} mm"));
    }
    let r0 = radius_mm;
    let h = STATION_MM.min(r0 / 3.0);
    let (pts, nas, nbs, step) = resample(seam, normals_a, normals_b, h);
    let n = pts.len();
    let mut st = stations(&pts, &nas, &nbs, step, (2.0 * step).max(r0), r0);
    let mut radii = vec![r0; n];
    // Wedge guard on the normals as planes.
    for (s, r) in st.iter().zip(radii.iter_mut()) {
        let d = 1.0 + dot(s.na, s.nb);
        let reach = if d > 1e-9 { norm(add(s.na, s.nb)) / d } else { f64::INFINITY };
        if reach > REACH_MAX {
            *r = (r0 * REACH_MAX / reach).max(RADIUS_MIN_MM);
        }
    }
    rate_limit(&mut radii, step);
    let mut m_arc = settle(&mut st, &mut radii, step, surfaces);
    let mut folded = vec![false; n];
    let mut solid = sweep(&st, m_arc);
    for round in 0..=FOLD_ROUNDS {
        let crossing = folded_stations(&solid, m_arc + 2);
        if crossing.is_empty() {
            break;
        }
        if round == FOLD_ROUNDS || crossing.iter().all(|&i| radii[i] <= RADIUS_MIN_MM + 1e-12) {
            return Err(format!("the bead folds at {} of {n} stations at its smallest radius", crossing.len()));
        }
        for &i in &crossing {
            for d in 0..5 {
                let j = (i + n + d - 2) % n;
                radii[j] = (radii[j] * 0.5).max(RADIUS_MIN_MM);
                folded[j] = true;
            }
        }
        rate_limit(&mut radii, step);
        m_arc = settle(&mut st, &mut radii, step, surfaces);
        solid = sweep(&st, m_arc);
    }
    let clamped = radii.iter().filter(|r| **r < r0 - 1e-9).count();
    let min_radius_mm = radii.iter().copied().fold(f64::INFINITY, f64::min);
    let unrefined = if surfaces.is_some() { st.iter().filter(|s| !s.refined).count() } else { 0 };
    Ok(Bead { solid, clamped, stations: n, min_radius_mm, unrefined, folded: folded.iter().filter(|f| **f).count() })
}

/// Settle every station's ball at its radius, then shrink any whose reach or inward extent breaks a
/// guard, until the radii hold still; returns the arc step count the sections share.
fn settle(st: &mut [Station], radii: &mut [f64], step: f64, surfaces: Option<&Surfaces>) -> usize {
    let mut m_arc = 3;
    for _round in 0..4 {
        for (s, r) in st.iter_mut().zip(radii.iter()) {
            s.r = *r;
            s.planar();
            if let Some(surf) = surfaces {
                s.refine(surf);
            }
        }
        m_arc = arc_steps(st);
        let mut next = radii.to_vec();
        for (s, r) in st.iter().zip(next.iter_mut()) {
            let reach = norm(sub(s.c, s.p)) / s.r;
            if reach > REACH_MAX {
                *r = (*r * REACH_MAX / reach).max(RADIUS_MIN_MM);
            }
            let inward = s.reach_in(m_arc, proud(s.r));
            let allowed = CURVE_FRACTION * s.r_seam;
            if inward > allowed {
                *r = (*r * allowed / inward).max(RADIUS_MIN_MM).min(*r);
            }
        }
        rate_limit(&mut next, step);
        let moved = radii.iter().zip(&next).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        radii.copy_from_slice(&next);
        if moved < 1e-6 {
            break;
        }
    }
    m_arc
}

/// Where the segment `p q` passes through the open interior of the triangle, by Möller–Trumbore.
fn pierces(p: P3, q: P3, [a, b, c]: [P3; 3]) -> bool {
    let d = sub(q, p);
    let (e1, e2) = (sub(b, a), sub(c, a));
    let h = cross(d, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-18 {
        return false;
    }
    let inv = 1.0 / det;
    let s = sub(p, a);
    let u = dot(s, h) * inv;
    let qv = cross(s, e1);
    let v = dot(d, qv) * inv;
    let t = dot(e2, qv) * inv;
    let eps = 1e-9;
    u > eps && v > eps && u + v < 1.0 - eps && t > eps && t < 1.0 - eps
}

/// Stations of a sweep with `m` vertices per section whose faces cross a face sharing no vertex with them.
fn folded_stations(s: &Solid, m: usize) -> Vec<usize> {
    let tris: Vec<[P3; 3]> = s.f.iter().map(|f| f.map(|k| s.v[k as usize])).collect();
    let mean = tris.iter().map(|t| norm(sub(t[1], t[0])) + norm(sub(t[2], t[1]))).sum::<f64>() / (2.0 * tris.len().max(1) as f64);
    let cell = (mean * 2.0).max(1e-4);
    let mut cells: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
    let boxes: Vec<(P3, P3)> = tris.iter().map(|t| tri_bounds(*t)).collect();
    for (i, (lo, hi)) in boxes.iter().enumerate() {
        let (lo, hi) = (Patch::at(*lo, cell), Patch::at(*hi, cell));
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    cells.entry([x, y, z]).or_default().push(i as u32);
                }
            }
        }
    }
    let mut hit = vec![false; s.f.len()];
    for list in cells.values() {
        for (a, &i) in list.iter().enumerate() {
            for &j in &list[a + 1..] {
                let (i, j) = (i as usize, j as usize);
                if hit[i] && hit[j] || s.f[i].iter().any(|v| s.f[j].contains(v)) {
                    continue;
                }
                let (bi, bj) = (&boxes[i], &boxes[j]);
                if (0..3).any(|k| bi.1[k] < bj.0[k] || bj.1[k] < bi.0[k]) {
                    continue;
                }
                let (ti, tj) = (tris[i], tris[j]);
                if (0..3).any(|k| pierces(ti[k], ti[(k + 1) % 3], tj) || pierces(tj[k], tj[(k + 1) % 3], ti)) {
                    hit[i] = true;
                    hit[j] = true;
                }
            }
        }
    }
    let mut stations: Vec<usize> = hit.iter().enumerate().filter(|(_, h)| **h).map(|(f, _)| f / 2 / m).collect();
    stations.sort_unstable();
    stations.dedup();
    stations
}

/// Radii never rise faster than `SLOPE` per millimetre round the loop, either way.
fn rate_limit(r: &mut [f64], step: f64) {
    let n = r.len();
    let rise = SLOPE * step;
    for _ in 0..2 {
        for i in 0..n {
            let j = (i + 1) % n;
            r[j] = r[j].min(r[i] + rise);
        }
        for i in (0..n).rev() {
            let j = (i + n - 1) % n;
            r[j] = r[j].min(r[i] + rise);
        }
    }
}

/// The sections swept round the loop: torus topology, wound outward by the sign of its volume.
fn sweep(st: &[Station], m_arc: usize) -> Solid {
    let n = st.len();
    let m = m_arc + 2;
    let mut v = Vec::with_capacity(n * m);
    for s in st {
        v.extend(s.section(m_arc, proud(s.r)));
    }
    let mut f = Vec::with_capacity(2 * n * m);
    for i in 0..n {
        let i1 = (i + 1) % n;
        for j in 0..m {
            let j1 = (j + 1) % m;
            let (a, b, c, d) = ((i * m + j) as u32, (i1 * m + j) as u32, (i1 * m + j1) as u32, (i * m + j1) as u32);
            f.push([a, b, c]);
            f.push([a, c, d]);
        }
    }
    let mut solid = Solid { v, f };
    if solid.volume() < 0.0 {
        for face in &mut solid.f {
            face.swap(1, 2);
        }
    }
    solid
}

/// Every seam loop of a traced boolean with the normals of the input faces that made each vertex,
/// pointed into the side the ball sits on for `concave`. The tool's faces are flipped in a cut, so a
/// convex rim negates the band's normal and keeps the tool's.
pub fn seams(traced: &Traced, a: &Solid, b: &Solid, concave: bool) -> Vec<Seam> {
    let by_vertex: HashMap<u32, (u32, u32)> = traced.seam.iter().map(|s| (s.vertex, (s.a_face, s.b_face))).collect();
    let (sa, sb) = if concave { (1.0, 1.0) } else { (-1.0, 1.0) };
    csg::seam_loops(traced)
        .into_iter()
        .filter_map(|lp| {
            let mut seam = Seam { points: Vec::new(), normals_a: Vec::new(), normals_b: Vec::new() };
            for v in lp {
                let &(fa, fb) = by_vertex.get(&v)?;
                seam.points.push(traced.solid.v[v as usize]);
                seam.normals_a.push(scale(face_normal(&a.v, *a.f.get(fa as usize)?), sa));
                seam.normals_b.push(scale(face_normal(&b.v, *b.f.get(fb as usize)?), sb));
            }
            Some(seam)
        })
        .collect()
}

/// Every seam loop of `traced` beaded and joined: a union into a concave seam, a subtraction off a
/// convex rim. `a` and `b` are the boolean's inputs, for the normals at the seam.
pub fn fillet_junction(traced: &Traced, a: &Solid, b: &Solid, radius_mm: f64, concave: bool) -> Result<Solid, String> {
    fillet_junction_report(traced, a, b, radius_mm, concave).map(|j| j.solid)
}

/// [`fillet_junction`] with what every bead did.
pub fn fillet_junction_report(traced: &Traced, a: &Solid, b: &Solid, radius_mm: f64, concave: bool) -> Result<Junction, String> {
    let seams = seams(traced, a, b, concave);
    if seams.is_empty() {
        return Err("the solids share no seam".into());
    }
    let mut out = Junction { solid: traced.solid.clone(), loops: seams.len(), skipped: 0, stations: 0, clamped: 0, unrefined: 0, folded: 0, min_radius_mm: radius_mm };
    let op = if concave { Op::Union } else { Op::Subtract };
    for (k, seam) in seams.iter().enumerate() {
        let length: f64 = (0..seam.points.len()).map(|i| norm(sub(seam.points[(i + 1) % seam.points.len()], seam.points[i]))).sum();
        if seam.points.len() < 3 || length < 3.0 * STATION_MM.min(radius_mm / 3.0) {
            out.skipped += 1;
            continue;
        }
        let surfaces = Surfaces::near(traced, &seam.points, (REACH_MAX + 1.5) * radius_mm + 0.05);
        let bead = bead_against(&seam.points, &seam.normals_a, &seam.normals_b, radius_mm, Some(&surfaces))?;
        out.stations += bead.stations;
        out.clamped += bead.clamped;
        out.unrefined += bead.unrefined;
        out.folded += bead.folded;
        out.min_radius_mm = out.min_radius_mm.min(bead.min_radius_mm);
        out.solid = csg::combine(&out.solid, &bead.solid, op).map_err(|e| format!("bead on seam {}: {e}", k + 1))?;
        csg::clean(&mut out.solid, CLEAN_MM);
    }
    if out.stations == 0 {
        return Err("every seam is too short to bead".into());
    }
    out.solid.compact();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;
    use std::time::Instant;

    /// An outward-wound box.
    fn cuboid(lo: P3, hi: P3) -> Solid {
        let v: Vec<P3> = (0..8).map(|i: usize| std::array::from_fn(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })).collect();
        let quads: [[u32; 4]; 6] = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
        let f = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
        Solid { v, f }
    }

    /// An outward-wound cylinder about z from `z0` to `z1`, caps fanned from their centres.
    fn cylinder(r: f64, z0: f64, z1: f64, n: usize) -> Solid {
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

    fn closed(s: &Solid) -> (usize, usize) {
        let e = s.open_edges();
        assert_eq!(e, (0, 0), "closed and manifold");
        e
    }

    /// Volume of the fillet round a post of radius `big` with a bead of radius `r`, by Pappus.
    fn torus_fillet_volume(big: f64, r: f64) -> f64 {
        let area = r * r * (1.0 - PI / 4.0);
        let centroid = (r * r * (big + r / 2.0) - PI * r * r / 4.0 * (big + r - 4.0 * r / (3.0 * PI))) / area;
        2.0 * PI * centroid * area
    }

    /// Volume of the bead itself round that post: the fillet plus the two triangles from the seam to
    /// each touch point and the closing vertex half a radius past the seam along the bisector.
    fn bead_section_volume(big: f64, r: f64) -> f64 {
        let deep = big - DEPTH_FRACTION * r / 2.0_f64.sqrt();
        let tri = r * r * 2.0_f64.sqrt() / 8.0;
        let moment = tri * (big + (big + r) + deep) / 3.0 + tri * (big + big + deep) / 3.0;
        torus_fillet_volume(big, r) + 2.0 * PI * moment
    }

    /// Signed distance of a result vertex in the fillet zone to the torus fillet round a post of radius
    /// `big` on the plane `z = 0`: the tube of radius `r` about the circle at radius `big + r`, height `r`.
    fn fillet_deviation(s: &Solid, big: f64, r: f64) -> (f64, f64, usize) {
        let (mut worst, mut sum, mut count) = (0.0_f64, 0.0, 0usize);
        for p in &s.v {
            let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
            let (dr, dz) = (rho - (big + r), p[2] - r);
            if dr > 1e-6 || dz > 1e-6 || rho < big - 0.02 || p[2] < -0.02 {
                continue;
            }
            let d = (dr * dr + dz * dz).sqrt() - r;
            worst = worst.max(d.abs());
            sum += d * d;
            count += 1;
        }
        (worst, if count > 0 { (sum / count as f64).sqrt() } else { 0.0 }, count)
    }

    /// The Court band built at the given sweep, with its mesh.
    fn court_band(theta_steps: usize, profile_steps: usize) -> (crate::Mesh, Solid) {
        let t = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap();
        let params = crate::BuildParams { theta_steps, profile_steps, ..crate::BuildParams::default() };
        let built = crate::mesh::try_build(&t.design(), &crate::AlphaLibrary::builtin(), params).unwrap();
        let solid = Solid { v: built.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: built.mesh.faces.clone() };
        (built.mesh, solid)
    }

    /// A frame on the built surface at `theta`, `across` mm along the finger, sunk `sink` under it and tilted `tilt` about the ring's tangent.
    fn on_band(mesh: &crate::Mesh, theta_deg: f64, across: f64, sink: f64, tilt_deg: f64) -> csg::Frame {
        let theta = theta_deg.to_radians();
        let from = [(40.0 * theta.cos()) as f32, (40.0 * theta.sin()) as f32, across as f32];
        let (face, hit) = crate::interaction::picking::raycast(mesh, from, [(-theta.cos()) as f32, (-theta.sin()) as f32, 0.0]).unwrap();
        let (a, b, c) = mesh.triangle(&mesh.faces[face]).unwrap();
        let normal = unit(cross(sub(b, a), sub(c, a)));
        let tangent = [-theta.sin(), theta.cos(), 0.0];
        let (sn, cs) = tilt_deg.to_radians().sin_cos();
        let tilted = add(scale(normal, cs), scale(cross(tangent, normal), sn));
        csg::Frame::from_normal(sub(hit.map(f64::from), scale(tilted, sink)), tilted, tangent)
    }

    #[test]
    fn the_bead_alone_is_a_torus_of_the_fillet_section() {
        // A circular seam of radius 3 between a plane facing up and a post facing out.
        let n = 240;
        let seam: Vec<P3> = (0..n).map(|i| { let th = 2.0 * PI * i as f64 / n as f64; [3.0 * th.cos(), 3.0 * th.sin(), 0.0] }).collect();
        let up = vec![[0.0, 0.0, 1.0]; n];
        let out: Vec<P3> = seam.iter().map(|p| unit([p[0], p[1], 0.0])).collect();
        let b = bead(&seam, &up, &out, 0.3).unwrap();
        closed(&b.solid);
        assert_eq!(csg::self_crossings(&b.solid), 0);
        assert_eq!((b.clamped, b.unrefined, b.folded), (0, 0, 0));
        assert_eq!(b.stations, (2.0 * PI * 3.0 / 0.05).ceil() as usize);
        // The section is the fillet's own plus the closing wedge from the arc's ends to the vertex half a radius past the seam.
        let (vol, expected) = (b.solid.volume(), bead_section_volume(3.0, 0.3));
        assert!((vol / expected - 1.0).abs() < 0.02, "{vol} against {expected}");
        // Every arc vertex stands 2–4 µm off the ball.
        let (worst, _, count) = fillet_deviation(&b.solid, 3.0, 0.3);
        assert!(count > 1000 && worst < 4.1e-3, "{worst} over {count}");
        assert!(bead(&seam[..2], &up[..2], &out[..2], 0.3).is_err());
        assert!(bead(&seam, &up, &out, 0.0).is_err());
    }

    #[test]
    fn a_post_on_a_plane_takes_the_torus_fillet_to_a_hundredth() {
        let slab = cuboid([-8.0, -8.0, -4.0], [8.0, 8.0, 0.0]);
        let post = cylinder(3.0, -0.05, 2.45, 96);
        let t = csg::combine_traced(&slab, &post, Op::Union, None).unwrap();
        let before = t.solid.volume();
        let started = Instant::now();
        let j = fillet_junction_report(&t, &slab, &post, 0.3, true).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        closed(&j.solid);
        assert_eq!(csg::self_crossings(&j.solid), 0);
        assert_eq!((j.loops, j.skipped, j.clamped, j.unrefined, j.folded), (1, 0, 0, 0, 0));
        let added = j.solid.volume() - before;
        let fillet = torus_fillet_volume(3.0, 0.3);
        // The arc stands 3 µm off the ball toward the corner: 0.0012 mm² shy of the fillet's section, 0.023 mm³ round the loop.
        assert!(added > fillet - 0.035 && added < fillet, "added {added} mm³ against {fillet}");
        let (worst, rms, count) = fillet_deviation(&j.solid, 3.0, 0.3);
        eprintln!("plane fillet: {ms:.1} ms, {} faces, worst {:.4} mm rms {:.4} over {count} vertices, added {added:.4} mm³", j.solid.f.len(), worst, rms);
        assert!(count > 500 && worst < 0.01, "worst {worst} over {count}");
        let plain = fillet_junction(&t, &slab, &post, 0.3, true).unwrap();
        assert_eq!(plain.f.len(), j.solid.f.len());
    }

    #[test]
    fn a_re_entrant_seam_corner_pinches_the_bead_instead_of_folding_it() {
        let slab = cuboid([-8.0, -8.0, -4.0], [8.0, 8.0, 0.0]);
        let plus = csg::combine(&cuboid([-2.0, -1.0, -0.05], [2.0, 1.0, 2.45]), &cuboid([-1.0, -2.0, -0.05], [1.0, 2.0, 2.45]), Op::Union).unwrap();
        let t = csg::combine_traced(&slab, &plus, Op::Union, None).unwrap();
        let seam = &seams(&t, &slab, &plus, true)[0];
        let surfaces = Surfaces::near(&t, &seam.points, 1.5);
        let started = Instant::now();
        let bead = bead_against(&seam.points, &seam.normals_a, &seam.normals_b, 0.3, Some(&surfaces)).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        closed(&bead.solid);
        assert_eq!(csg::self_crossings(&bead.solid), 0);
        eprintln!("plus post: {ms:.1} ms, {} stations, {} folded, {} clamped, min radius {:.3}", bead.stations, bead.folded, bead.clamped, bead.min_radius_mm);
        assert!(bead.folded > 0 && bead.clamped >= bead.folded, "{} folded, {} clamped", bead.folded, bead.clamped);
        // Twelve corners on a 24 mm loop, each pinch ramping back over 0.56 mm either side: the middles of the runs keep the full radius.
        assert!(bead.stations - bead.clamped >= bead.stations / 4, "{} of {}", bead.clamped, bead.stations);
        let j = fillet_junction_report(&t, &slab, &plus, 0.3, true).unwrap();
        closed(&j.solid);
        assert_eq!(csg::self_crossings(&j.solid), 0);
        assert!(j.solid.volume() > t.solid.volume());
    }

    #[test]
    fn a_rim_loses_the_bead_as_a_cut() {
        let block = cuboid([-8.0, -8.0, -5.0], [8.0, 8.0, 0.0]);
        let drill = cylinder(3.0, -6.0, 1.0, 96);
        let t = csg::combine_traced(&block, &drill, Op::Subtract, None).unwrap();
        let before = t.solid.volume();
        let j = fillet_junction_report(&t, &block, &drill, 0.3, false).unwrap();
        closed(&j.solid);
        assert_eq!(csg::self_crossings(&j.solid), 0);
        assert_eq!((j.loops, j.clamped, j.unrefined), (2, 0, 0), "a rim at each face of the block");
        let removed = before - j.solid.volume();
        let fillet = 2.0 * torus_fillet_volume(3.0, 0.3);
        assert!(removed > fillet - 0.07 && removed < fillet, "removed {removed} mm³ against {fillet}");
        // The top rim's round is the tube of radius 0.3 about the circle at radius 3.3, 0.3 mm down.
        let (mut worst, mut count) = (0.0_f64, 0);
        for p in &j.solid.v {
            let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
            let (dr, dz) = (rho - 3.3, p[2] + 0.3);
            if dr > 1e-6 || dz < -1e-6 || rho < 2.98 || p[2] > 0.02 {
                continue;
            }
            worst = worst.max(((dr * dr + dz * dz).sqrt() - 0.3).abs());
            count += 1;
        }
        assert!(count > 500 && worst < 0.01, "worst {worst} over {count}");
    }

    #[test]
    fn a_wire_lying_on_the_court_band_clamps_the_bead_where_the_wedge_closes() {
        let (mesh, band) = court_band(256, 128);
        // A 0.8 mm wire along the ring, sunk 0.05 mm at the top: it lifts off within a millimetre either way.
        let frame = on_band(&mesh, 90.0, 0.0, 0.0, 0.0);
        let axis = add(frame.origin, scale(frame.z, 0.4 - 0.05));
        let lying = csg::Frame::from_normal(axis, frame.x, frame.z);
        let wire = cylinder(0.4, -1.5, 1.5, 48).placed(&lying);
        let t = csg::combine_traced(&band, &wire, Op::Union, None).unwrap();
        let seams = seams(&t, &band, &wire, true);
        assert_eq!(seams.len(), 1, "one lens where the wire dips in");
        let started = Instant::now();
        let j = fillet_junction_report(&t, &band, &wire, 0.3, true).unwrap();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        closed(&j.solid);
        eprintln!("lying wire: {ms:.1} ms, {} stations, {} clamped, min radius {:.3}, {} unrefined", j.stations, j.clamped, j.min_radius_mm, j.unrefined);
        assert!(j.clamped > j.stations / 2, "the crevice closes toward both tips: {} of {}", j.clamped, j.stations);
        assert!(j.min_radius_mm <= RADIUS_MIN_MM + 1e-9);
        let bead = bead_against(&seams[0].points, &seams[0].normals_a, &seams[0].normals_b, 0.3, Some(&Surfaces::near(&t, &seams[0].points, 1.5))).unwrap();
        assert_eq!(csg::self_crossings(&bead.solid), 0);
    }

    #[test]
    fn twenty_bezels_round_the_court_band_bead_clean() {
        let (mesh, band) = court_band(256, 128);
        let mut seed = 0x9e37_79b9_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let (mut clamped, mut stations, mut worst_ms) = (0, 0, 0.0_f64);
        for k in 0..20 {
            let theta = 360.0 * next();
            let across = 0.8 * (next() - 0.5);
            let tilt = 6.0 * (next() - 0.5);
            let frame = on_band(&mesh, theta, across, 0.5, tilt);
            let bezel = cylinder(1.5, 0.0, 2.5, 48).placed(&frame);
            let t = csg::combine_traced(&band, &bezel, Op::Union, None).unwrap_or_else(|e| panic!("placement {k}: {e}"));
            let started = Instant::now();
            let j = fillet_junction_report(&t, &band, &bezel, 0.3, true).unwrap_or_else(|e| panic!("placement {k} at {theta:.1}°: {e}"));
            worst_ms = worst_ms.max(started.elapsed().as_secs_f64() * 1e3);
            assert_eq!(j.solid.open_edges(), (0, 0), "placement {k} at {theta:.1}°");
            assert_eq!(j.loops, 1, "placement {k}: the wall meets the crown all round");
            let seam = &seams(&t, &band, &bezel, true)[0];
            let bead = bead_against(&seam.points, &seam.normals_a, &seam.normals_b, 0.3, Some(&Surfaces::near(&t, &seam.points, 1.5))).unwrap();
            assert_eq!(csg::self_crossings(&bead.solid), 0, "placement {k}");
            assert!(j.solid.volume() > t.solid.volume(), "placement {k}: the bead adds metal");
            clamped += j.clamped;
            stations += j.stations;
        }
        eprintln!("twenty bezels: {clamped} of {stations} stations clamped, slowest {worst_ms:.0} ms");
    }
}
