//! A stone's true girdle in plan: the exact silhouette of its bundled facet mesh, as a star-shaped polygon.

use crate::gem::GemCut;
use std::f64::consts::TAU;
use std::sync::OnceLock;

/// Outward turn at which a girdle point takes a claw of its own, radians.
pub const CORNER_TURN: f64 = std::f64::consts::FRAC_PI_4;
/// Largest turn inside a run between corners that still reads as a straight side, radians.
const STRAIGHT_TURN: f64 = 0.0175;
/// Outward turn past which a corner gets [`Girdle::corner_rays`], radians.
const SHARP_TURN: f64 = 0.35;

/// A girdle in plan: a polygon star-shaped about the plan's centre, at unit half-extents, `x` along the length.
#[derive(Clone, Debug)]
pub struct Girdle {
    /// Corners anticlockwise from the +x axis.
    points: Vec<[f64; 2]>,
    /// Each corner's polar angle, ascending in `[0, TAU]`.
    angles: Vec<f64>,
    /// How far the outline turns at each corner, radians: positive where convex, negative where it turns back in.
    turns: Vec<f64>,
    /// The outline seat stock follows, when it is not this one: see [`Girdle::stock`].
    stock: Option<Box<Girdle>>,
}

/// The girdle of `cut`'s bundled mesh, for every cut that has one; [`GemCut::girdle`] is the subset seats read.
pub fn of(cut: GemCut) -> Option<&'static Girdle> {
    static CACHE: [OnceLock<Option<Girdle>>; GemCut::ALL.len()] = [const { OnceLock::new() }; GemCut::ALL.len()];
    let i = GemCut::ALL.iter().position(|c| *c == cut)?;
    CACHE[i].get_or_init(|| crate::gems::bundled_facets(cut).and_then(Girdle::from_facets)).as_ref()
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn norm(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}

/// Polar angle in `[0, TAU)`, with a negative zero read as zero.
fn angle(p: [f64; 2]) -> f64 {
    let a = p[1].atan2(p[0]);
    if a < 0.0 { a + TAU } else { a + 0.0 }
}

/// The farthest any projected facet reaches along the ray at `psi`: the silhouette's own radius there.
pub(crate) fn silhouette_radius(facets: &[[[f64; 2]; 3]], psi: f64) -> f64 {
    let d = [psi.cos(), psi.sin()];
    let mut best = 0.0f64;
    for f in facets {
        for k in 0..3 {
            let (p, q) = (f[k], f[(k + 1) % 3]);
            let e = sub(q, p);
            let den = cross(d, e);
            if den.abs() < 1e-15 {
                continue;
            }
            let t = cross(p, e) / den;
            let u = cross(p, d) / den;
            if (-1e-12..=1.0 + 1e-12).contains(&u) && t > best {
                best = t;
            }
        }
    }
    best
}

/// Points sorted by angle, less any within `1e-7` of the one before.
fn sorted_unique(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_by(|p, q| angle(*p).total_cmp(&angle(*q)));
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_some_and(|q| norm(sub(p, *q)) < 1e-7) {
            continue;
        }
        out.push(p);
    }
    while out.len() > 1 && norm(sub(out[0], out[out.len() - 1])) < 1e-7 {
        out.pop();
    }
    out
}

/// Every point that lies on a straight run between its neighbours removed.
fn without_collinear(points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let n = points.len();
    if n < 4 {
        return points;
    }
    (0..n)
        .filter(|i| {
            let (a, b, c) = (points[(i + n - 1) % n], points[*i], points[(i + 1) % n]);
            cross(sub(b, a), sub(c, b)).abs() > 1e-12
        })
        .map(|i| points[i])
        .collect()
}

/// The convex hull, anticlockwise, by the monotone chain.
fn convex_hull(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let mut lower: Vec<[f64; 2]> = Vec::new();
    for q in &p {
        while lower.len() >= 2 && cross(sub(lower[lower.len() - 1], lower[lower.len() - 2]), sub(*q, lower[lower.len() - 1])) <= 0.0 {
            lower.pop();
        }
        lower.push(*q);
    }
    let mut upper: Vec<[f64; 2]> = Vec::new();
    for q in p.iter().rev() {
        while upper.len() >= 2 && cross(sub(upper[upper.len() - 1], upper[upper.len() - 2]), sub(*q, upper[upper.len() - 1])) <= 0.0 {
            upper.pop();
        }
        upper.push(*q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

impl Girdle {
    /// A girdle through `points`, sorted by angle about the origin; `None` unless they make a polygon star-shaped about it.
    pub fn from_points(points: Vec<[f64; 2]>) -> Option<Self> {
        let mut girdle = Self::outline(points)?;
        let mirrored: Vec<[f64; 2]> = girdle.points.iter().flat_map(|p| [*p, [-p[0], p[1]], [p[0], -p[1]], [-p[0], -p[1]]]).collect();
        let stock = Self::outline(convex_hull(&mirrored))?;
        if stock.points != girdle.points {
            girdle.stock = Some(Box::new(stock));
        }
        Some(girdle)
    }

    /// [`from_points`](Self::from_points) without the stock outline.
    fn outline(points: Vec<[f64; 2]>) -> Option<Self> {
        let points = sorted_unique(points);
        let n = points.len();
        if n < 3 || (0..n).any(|i| cross(points[i], points[(i + 1) % n]) <= 0.0) {
            return None;
        }
        let angles: Vec<f64> = points.iter().map(|p| angle(*p)).collect();
        if angles.windows(2).any(|w| w[1] <= w[0]) {
            return None;
        }
        let turns: Vec<f64> = (0..n)
            .map(|i| {
                let (u, v) = (sub(points[i], points[(i + n - 1) % n]), sub(points[(i + 1) % n], points[i]));
                cross(u, v).atan2(u[0] * v[0] + u[1] * v[1])
            })
            .collect();
        Some(Self { points, angles, turns, stock: None })
    }

    /// The silhouette of a unit-extent facet mesh: projected corners and edge crossings on the outer boundary.
    pub fn from_facets(tris: &[([f64; 3], [f64; 3], [f64; 3])]) -> Option<Self> {
        let flat = |p: [f64; 3]| [2.0 * p[0], 2.0 * p[1]];
        let facets: Vec<[[f64; 2]; 3]> = tris.iter().map(|(a, b, c)| [flat(*a), flat(*b), flat(*c)]).collect();
        let on = |p: [f64; 2]| norm(p) >= silhouette_radius(&facets, angle(p)) - 1e-9;
        let mut corners: Vec<[f64; 2]> = facets.iter().flatten().copied().collect();
        corners.sort_by(|p, q| p[0].total_cmp(&q[0]).then(p[1].total_cmp(&q[1])));
        corners.dedup();
        let mut points: Vec<[f64; 2]> = corners.into_iter().filter(|p| on(*p)).collect();
        // Crossings of projected edges, such as the four crown-over-pavilion notches beside a heart's cleft.
        let mut edges: Vec<([f64; 2], [f64; 2])> = facets
            .iter()
            .flat_map(|f| (0..3).map(move |k| (f[k], f[(k + 1) % 3])))
            .map(|(p, q)| if (p[0], p[1]) <= (q[0], q[1]) { (p, q) } else { (q, p) })
            .filter(|(p, q)| norm(sub(*q, *p)) > 1e-12)
            .collect();
        edges.sort_by(|x, y| x.0[0].total_cmp(&y.0[0]).then(x.0[1].total_cmp(&y.0[1])).then(x.1[0].total_cmp(&y.1[0])).then(x.1[1].total_cmp(&y.1[1])));
        edges.dedup();
        for i in 0..edges.len() {
            let (p, r) = (edges[i].0, sub(edges[i].1, edges[i].0));
            for (s0, s1) in &edges[i + 1..] {
                let s = sub(*s1, *s0);
                let den = cross(r, s);
                if den.abs() < 1e-14 {
                    continue;
                }
                let w = sub(*s0, p);
                let (t, u) = (cross(w, s) / den, cross(w, r) / den);
                if !(1e-9..1.0 - 1e-9).contains(&t) || !(1e-9..1.0 - 1e-9).contains(&u) {
                    continue;
                }
                let x = [p[0] + t * r[0], p[1] + t * r[1]];
                if on(x) {
                    points.push(x);
                }
            }
        }
        Self::from_points(without_collinear(sorted_unique(points)))
    }

    /// The corners, anticlockwise from the +x axis, at unit half-extents.
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }

    /// Each corner's polar angle, ascending.
    pub fn angles(&self) -> &[f64] {
        &self.angles
    }

    /// How far the outline turns at each corner, radians; negative where it turns back in.
    pub fn turns(&self) -> &[f64] {
        &self.turns
    }

    /// Whether the outline never turns back in.
    pub fn is_convex(&self) -> bool {
        self.turns.iter().all(|t| *t >= -1e-12)
    }

    /// The outline seat stock follows: the convex hull of the girdle and its mirror images across both axes.
    pub fn stock(&self) -> &Girdle {
        self.stock.as_deref().unwrap_or(self)
    }

    /// The edge from corner `k` to the next whose angular span holds `psi`, in `[0, TAU)`.
    fn edge(&self, psi: f64) -> usize {
        match self.angles.partition_point(|a| *a <= psi) {
            0 => self.points.len() - 1,
            i => i - 1,
        }
    }

    /// Distance from the centre to the outline along the angle `psi`, at unit half-extents.
    pub fn radius(&self, psi: f64) -> f64 {
        norm(self.unit_point(psi))
    }

    /// The outline's point on the ray at angle `psi`, at unit half-extents; a corner itself when `psi` is its angle.
    pub fn unit_point(&self, psi: f64) -> [f64; 2] {
        let psi = psi.rem_euclid(TAU);
        let k = self.edge(psi);
        if self.angles[k] == psi {
            return self.points[k];
        }
        let (p, q) = (self.points[k], self.points[(k + 1) % self.points.len()]);
        let d = [psi.cos(), psi.sin()];
        let e = sub(q, p);
        let den = cross(d, e);
        let t = if den.abs() < 1e-15 { norm(p).max(norm(q)) } else { cross(p, e) / den };
        [d[0] * t, d[1] * t]
    }

    /// Outward unit normal at `psi` in mm at half-extents `a` and `b`; the mean of both edges at a corner.
    pub fn normal_mm(&self, psi: f64, a: f64, b: f64) -> [f64; 2] {
        let psi = psi.rem_euclid(TAU);
        let n = self.points.len();
        let k = self.edge(psi);
        let edge_normal = |i: usize| {
            let (p, q) = (self.points[i % n], self.points[(i + 1) % n]);
            let t = [(q[0] - p[0]) * a, (q[1] - p[1]) * b];
            let l = norm(t).max(1e-12);
            [t[1] / l, -t[0] / l]
        };
        if self.angles[k] != psi {
            return edge_normal(k);
        }
        let (m0, m1) = (edge_normal(k + n - 1), edge_normal(k));
        let m = [m0[0] + m1[0], m0[1] + m1[1]];
        let l = norm(m);
        if l > 1e-9 {
            return [m[0] / l, m[1] / l];
        }
        let p = [self.points[k][0] * a, self.points[k][1] * b];
        let l = norm(p).max(1e-12);
        [p[0] / l, p[1] / l]
    }

    /// Distance from the centre to the outline toward `(x, y)`, mm, at half-extents `a` and `b`; `b` at the centre.
    pub fn radius_mm(&self, x: f64, y: f64, a: f64, b: f64) -> f64 {
        let (a, b) = (a.max(1e-9), b.max(1e-9));
        let d = x.hypot(y);
        if d <= 1e-12 {
            return b;
        }
        let (u, v) = (x / a, y / b);
        d * self.radius(v.atan2(u)) / u.hypot(v).max(1e-12)
    }

    /// The farthest the outline reaches from its centre, mm.
    pub fn reach_mm(&self, a: f64, b: f64) -> f64 {
        self.points.iter().map(|p| (p[0] * a).hypot(p[1] * b)).fold(0.0, f64::max)
    }

    /// The outline's length, mm.
    pub fn perimeter_mm(&self, a: f64, b: f64) -> f64 {
        let n = self.points.len();
        (0..n).map(|i| self.edge_mm(i, a, b)).sum()
    }

    fn edge_mm(&self, i: usize, a: f64, b: f64) -> f64 {
        let n = self.points.len();
        let (p, q) = (self.points[i % n], self.points[(i + 1) % n]);
        ((q[0] - p[0]) * a).hypot((q[1] - p[1]) * b)
    }

    /// Half-extents along `u` and across `v`, mm, after turning by `rot_deg`; the farther side of each axis.
    pub fn half_extents_mm(&self, a: f64, b: f64, rot_deg: f64) -> (f64, f64) {
        if rot_deg == 0.0 {
            return (a, b);
        }
        let (s, c) = rot_deg.to_radians().sin_cos();
        self.points.iter().fold((0.0f64, 0.0f64), |(hu, hv), p| {
            let (x, y) = (p[0] * a, p[1] * b);
            (hu.max((x * c - y * s).abs()), hv.max((x * s + y * c).abs()))
        })
    }

    /// Distance along the unit direction `dir` to the boundary of the outline scaled by `s` plus a disc of radius `o`, mm.
    pub fn grown_mm(&self, dir: [f64; 2], a: f64, b: f64, s: f64, o: f64) -> f64 {
        let inside = s * self.radius_mm(dir[0], dir[1], a, b);
        if o <= 0.0 {
            return inside;
        }
        let n = self.points.len();
        let scaled = |i: usize| [self.points[i % n][0] * a * s, self.points[i % n][1] * b * s];
        let mut best = inside;
        for i in 0..n {
            let (p, q) = (scaled(i), scaled(i + 1));
            // The disc at `p`; the next edge takes `q`'s.
            let dp = dir[0] * p[0] + dir[1] * p[1];
            let disc = dp * dp - (p[0] * p[0] + p[1] * p[1]) + o * o;
            if disc >= 0.0 {
                best = best.max(dp + disc.sqrt());
            }
            let e = sub(q, p);
            let (l, den) = (norm(e), cross(dir, e));
            if l < 1e-15 || den.abs() < 1e-15 {
                continue;
            }
            let m = [e[1] / l * o, -e[0] / l * o];
            for side in [m, [-m[0], -m[1]]] {
                let p0 = [p[0] + side[0], p[1] + side[1]];
                let (t, u) = (cross(p0, e) / den, cross(p0, dir) / den);
                if (0.0..=1.0).contains(&u) {
                    best = best.max(t);
                }
            }
        }
        best
    }

    /// Angles for `n` claws: the sharpest corners first, the rest by arc length along the curved runs between them.
    pub fn claw_angles(&self, n: usize, a: f64, b: f64) -> Vec<f64> {
        let m = self.points.len();
        let mut corners: Vec<usize> = (0..m).filter(|i| self.turns[*i] >= CORNER_TURN).collect();
        corners.sort_by(|x, y| self.turns[*y].total_cmp(&self.turns[*x]).then(x.cmp(y)));
        corners.truncate(n);
        corners.sort_unstable();
        let mut at = vec![0.0; m + 1];
        for i in 0..m {
            at[i + 1] = at[i] + self.edge_mm(i, a, b);
        }
        let total = at[m];
        // (start, length, straight) of each run from one corner to the next.
        let runs: Vec<(f64, f64, bool)> = if corners.is_empty() {
            vec![(0.0, total, false)]
        } else {
            let k = corners.len();
            (0..k)
                .map(|j| {
                    let (s, e) = (corners[j], corners[(j + 1) % k]);
                    let length = if k == 1 { total } else { (at[e] - at[s]).rem_euclid(total) };
                    let straight = (1..).map(|d| (s + d) % m).take_while(|i| *i != e).all(|i| self.turns[i].abs() < STRAIGHT_TURN);
                    (at[s], length, straight)
                })
                .collect()
        };
        let curved = runs.iter().any(|r| !r.2);
        let mut extra = vec![0usize; runs.len()];
        for _ in corners.len()..n {
            let pick = (0..runs.len())
                .filter(|j| !curved || !runs[*j].2)
                .max_by(|x, y| (runs[*x].1 / (extra[*x] + 1) as f64).total_cmp(&(runs[*y].1 / (extra[*y] + 1) as f64)).then(y.cmp(x)));
            if let Some(j) = pick {
                extra[j] += 1;
            }
        }
        let mut out: Vec<f64> = corners.iter().map(|i| self.angles[*i]).collect();
        for ((start, length, _), count) in runs.iter().zip(&extra) {
            for c in 0..*count {
                // Evenly inside a run; mid-step round the outline when there is no corner.
                let f = if corners.is_empty() { (c as f64 + 0.5) / *count as f64 } else { (c + 1) as f64 / (count + 1) as f64 };
                out.push(self.angle_at_arc((start + length * f).rem_euclid(total.max(1e-12)), &at));
            }
        }
        out.sort_by(f64::total_cmp);
        out
    }

    /// Angles of the points 0.02 and 0.06 mm along both edges of every sharp corner, at most a quarter of either edge.
    pub fn corner_rays(&self, a: f64, b: f64) -> Vec<f64> {
        let n = self.points.len();
        let mut out = Vec::new();
        for (k, turn) in self.turns.iter().enumerate() {
            if *turn < SHARP_TURN {
                continue;
            }
            let c = self.points[k];
            for nb in [self.points[(k + n - 1) % n], self.points[(k + 1) % n]] {
                let len = ((nb[0] - c[0]) * a).hypot((nb[1] - c[1]) * b);
                if len <= 1e-9 {
                    continue;
                }
                for s in [0.02, 0.06] {
                    let t = (s / len).min(0.25);
                    out.push(angle([c[0] + t * (nb[0] - c[0]), c[1] + t * (nb[1] - c[1])]));
                }
            }
        }
        out
    }

    /// The angle of the point `s` mm along the outline from corner 0, given the outline's cumulative lengths `at`.
    fn angle_at_arc(&self, s: f64, at: &[f64]) -> f64 {
        let m = self.points.len();
        let i = at.partition_point(|x| *x <= s).saturating_sub(1).min(m - 1);
        let len = (at[i + 1] - at[i]).max(1e-12);
        let f = ((s - at[i]) / len).clamp(0.0, 1.0);
        let (p, q) = (self.points[i], self.points[(i + 1) % m]);
        angle([p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bundled mesh's girdle is its exact silhouette and fills the unit box.
    #[test]
    fn every_bundled_mesh_reads_as_its_exact_silhouette() {
        for &cut in GemCut::ALL {
            let g = of(cut).unwrap_or_else(|| panic!("{cut:?} has no girdle"));
            let tris = crate::gems::bundled_facets(cut).unwrap();
            let flat = |p: [f64; 3]| [2.0 * p[0], 2.0 * p[1]];
            let facets: Vec<[[f64; 2]; 3]> = tris.iter().map(|(a, b, c)| [flat(*a), flat(*b), flat(*c)]).collect();
            let worst = (0..1440)
                .map(|k| {
                    let psi = TAU * (k as f64 + 0.37) / 1440.0;
                    (g.radius(psi) - silhouette_radius(&facets, psi)).abs()
                })
                .fold(0.0, f64::max);
            assert!(worst < 1e-9, "{cut:?}: the girdle strays {worst:.2e} off the silhouette");
            for axis in 0..2 {
                let (lo, hi) = g.points().iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[axis]), hi.max(p[axis])));
                assert!((lo + 1.0).abs() < 1e-9 && (hi - 1.0).abs() < 1e-9, "{cut:?} axis {axis}: {lo} to {hi}");
            }
        }
    }

    /// The four true girdles carry their points, pinned off the bundled meshes.
    #[test]
    fn the_four_true_girdles_carry_their_points() {
        let true_cuts: Vec<GemCut> = GemCut::ALL.iter().copied().filter(|c| c.has_true_girdle()).collect();
        assert_eq!(true_cuts, [GemCut::Pear, GemCut::Trillion, GemCut::Heart, GemCut::HalfMoon]);
        assert!(GemCut::ALL.iter().all(|c| c.girdle().is_some() == c.has_true_girdle()));
        let corners = |g: &Girdle| -> Vec<f64> { g.angles().iter().zip(g.turns()).filter(|(_, t)| **t >= CORNER_TURN).map(|(a, _)| a.to_degrees()).collect() };
        let pear = GemCut::Pear.girdle().unwrap();
        assert_eq!((pear.points().len(), corners(pear).len()), (32, 1));
        assert!((corners(pear)[0] - 180.0).abs() < 1e-9, "the point leads along -x: {:?}", corners(pear));
        let trillion = GemCut::Trillion.girdle().unwrap();
        assert_eq!((trillion.points().len(), corners(trillion).len()), (18, 3));
        let heart = GemCut::Heart.girdle().unwrap();
        assert_eq!((heart.points().len(), corners(heart).len()), (50, 1));
        assert!(!heart.is_convex() && heart.stock().is_convex());
        let cleft = heart.turns().iter().copied().fold(f64::MAX, f64::min);
        assert!(cleft < -2.0, "the cleft turns back in by {:.1} degrees", cleft.to_degrees());
        let moon = GemCut::HalfMoon.girdle().unwrap();
        assert_eq!((moon.points().len(), corners(moon).len()), (23, 2));
        assert!([pear, trillion, moon].iter().all(|g| g.is_convex()));
        // The half moon's two corners span its full length.
        let c: Vec<[f64; 2]> = moon.angles().iter().zip(moon.turns()).zip(moon.points()).filter(|((_, t), _)| **t >= CORNER_TURN).map(|(_, p)| *p).collect();
        assert!((c[0][0] - c[1][0]).abs() > 1.99 && (c[0][1] - c[1][1]).abs() < 1e-5, "{c:?}");
    }

    #[test]
    fn a_square_reads_its_corners_and_grows_round_them() {
        let g = Girdle::from_points(vec![[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]]).unwrap();
        assert!(g.is_convex());
        assert_eq!(g.angles().len(), 4);
        assert!((g.radius(0.0) - 1.0).abs() < 1e-12);
        assert!((g.radius(std::f64::consts::FRAC_PI_4) - 2f64.sqrt()).abs() < 1e-12);
        // Side and corner normals at 3 x 2 mm.
        let n = g.normal_mm(0.3, 3.0, 2.0);
        assert!((n[0] - 1.0).abs() < 1e-12 && n[1].abs() < 1e-12);
        let c = g.normal_mm(std::f64::consts::FRAC_PI_4, 3.0, 2.0);
        assert!((c[0] - c[1]).abs() < 1e-12);
        assert!((g.perimeter_mm(3.0, 2.0) - 20.0).abs() < 1e-12);
        assert_eq!(g.half_extents_mm(3.0, 2.0, 0.0), (3.0, 2.0));
        let (u, v) = g.half_extents_mm(3.0, 2.0, 90.0);
        assert!((u - 2.0).abs() < 1e-12 && (v - 3.0).abs() < 1e-12);
        // Grown by 0.5 mm along a side and toward a corner.
        assert!((g.grown_mm([1.0, 0.0], 3.0, 2.0, 1.0, 0.5) - 3.5).abs() < 1e-12);
        let diag = [3.0 / 13f64.sqrt(), 2.0 / 13f64.sqrt()];
        assert!((g.grown_mm(diag, 3.0, 2.0, 1.0, 0.5) - (13f64.sqrt() + 0.5)).abs() < 1e-12);
        // Four claws on four corners.
        let claws = g.claw_angles(4, 3.0, 2.0);
        assert_eq!(claws, g.angles().to_vec());
    }

    #[test]
    fn a_notched_outline_carries_its_hull_and_grows_without_folding() {
        // A square with a cleft cut into its +x side.
        let g = Girdle::from_points(vec![[0.4, 0.0], [1.0, 0.6], [1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0], [1.0, -0.6]]).unwrap();
        assert!(!g.is_convex());
        assert!(g.turns()[0] < -1.0, "the cleft turns back in: {}", g.turns()[0]);
        assert!(g.stock().is_convex() && (g.stock().radius(0.0) - 1.0).abs() < 1e-12);
        // Grown past the cleft's width, the ray down the cleft stops on the sides' offsets.
        let r = g.grown_mm([1.0, 0.0], 1.0, 1.0, 1.0, 0.5);
        assert!(r > 0.4 + 0.5 && r < 1.5, "{r}");
        // Successive rays stay anticlockwise.
        let rays: Vec<[f64; 2]> = (0..720).map(|k| { let a = TAU * (k as f64 + 0.5) / 720.0; let r = g.grown_mm([a.cos(), a.sin()], 1.0, 1.0, 1.0, 0.5); [a.cos() * r, a.sin() * r] }).collect();
        assert!((0..720).all(|k| cross(rays[k], rays[(k + 1) % 720]) > 0.0));
    }

    #[test]
    fn claws_take_the_corners_and_spread_along_the_curve() {
        // A half disc: the straight side along +y, two corners.
        let mut pts: Vec<[f64; 2]> = (0..=16).map(|k| { let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * k as f64 / 16.0; [a.cos(), a.sin()] }).collect();
        for p in &mut pts {
            p[0] = 2.0 * p[0] - 1.0;
        }
        let g = Girdle::from_points(pts).unwrap();
        let claws = g.claw_angles(4, 1.0, 1.0);
        assert_eq!(claws.len(), 4);
        // Two claws on the corners, two on the arc.
        let corner = |c: f64| g.angles().iter().zip(g.turns()).any(|(a, t)| *a == c && *t >= CORNER_TURN);
        assert_eq!(claws.iter().filter(|c| corner(**c)).count(), 2);
        assert!(claws.iter().filter(|c| !corner(**c)).all(|c| g.unit_point(*c)[0] > -0.99), "{claws:?}");
    }
}
