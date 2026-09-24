//! Stamp outlines: closed counter-clockwise polylines in mm, long axis along `x`, no edge longer than [`STEP`].
use std::f64::consts::{PI, TAU};

/// The longest edge an outline carries, mm.
pub const STEP: f64 = 0.1;

/// Points with corners marked, resampled by [`Pen::finish`].
struct Pen {
    pts: Vec<[f64; 2]>,
    corner: Vec<bool>,
}

impl Pen {
    fn new() -> Self {
        Self { pts: Vec::new(), corner: Vec::new() }
    }

    fn push(&mut self, p: [f64; 2], corner: bool) {
        if let Some(last) = self.pts.last() {
            if (last[0] - p[0]).hypot(last[1] - p[1]) < 1e-9 {
                let k = self.corner.len() - 1;
                self.corner[k] |= corner;
                return;
            }
        }
        self.pts.push(p);
        self.corner.push(corner);
    }

    fn corner(&mut self, p: [f64; 2]) {
        self.push(p, true);
    }

    /// Samples `f` finely over `(t0, t1]`.
    fn curve(&mut self, f: impl Fn(f64) -> [f64; 2], t0: f64, t1: f64, end_corner: bool) {
        let probe = 64;
        let mut len = 0.0;
        let mut prev = f(t0);
        for k in 1..=probe {
            let p = f(t0 + (t1 - t0) * k as f64 / probe as f64);
            len += (p[0] - prev[0]).hypot(p[1] - prev[1]);
            prev = p;
        }
        let n = ((len / 0.004).ceil() as usize).clamp(probe, 40_000);
        for k in 1..=n {
            let p = f(t0 + (t1 - t0) * k as f64 / n as f64);
            self.push(p, k == n && end_corner);
        }
    }

    /// Resamples each run between corners at equal steps no longer than [`STEP`].
    fn finish(mut self) -> Vec<[f64; 2]> {
        while self.pts.len() > 1 {
            let (a, b) = (self.pts[0], self.pts[self.pts.len() - 1]);
            if (a[0] - b[0]).hypot(a[1] - b[1]) >= 1e-9 {
                break;
            }
            let c = self.corner.pop().unwrap_or(false);
            self.pts.pop();
            self.corner[0] |= c;
        }
        let n = self.pts.len();
        if n < 3 {
            return self.pts;
        }
        if !self.corner.iter().any(|c| *c) {
            self.corner[0] = true;
        }
        let corners: Vec<usize> = (0..n).filter(|i| self.corner[*i]).collect();
        let mut out = Vec::with_capacity(n.min(4096));
        for (k, &a) in corners.iter().enumerate() {
            let b = corners[(k + 1) % corners.len()];
            let mut run = vec![self.pts[a]];
            let mut i = a;
            loop {
                i = (i + 1) % n;
                run.push(self.pts[i]);
                if i == b {
                    break;
                }
            }
            out.push(run[0]);
            if run.len() == 2 {
                let (c, e) = (run[0], run[1]);
                let steps = ((c[0] - e[0]).hypot(c[1] - e[1]) / STEP).ceil().max(1.0) as usize;
                out.extend((1..steps).map(|q| {
                    let f = q as f64 / steps as f64;
                    [c[0] + (e[0] - c[0]) * f, c[1] + (e[1] - c[1]) * f]
                }));
                continue;
            }
            let mut cum = vec![0.0];
            for w in run.windows(2) {
                cum.push(cum[cum.len() - 1] + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
            }
            let total = cum[cum.len() - 1];
            let steps = (total / STEP * (1.0 + 1e-9)).ceil().max(1.0) as usize;
            let mut j = 0;
            for q in 1..steps {
                let s = total * q as f64 / steps as f64;
                while j + 2 < cum.len() && cum[j + 1] < s {
                    j += 1;
                }
                let f = ((s - cum[j]) / (cum[j + 1] - cum[j]).max(1e-15)).clamp(0.0, 1.0);
                out.push([run[j][0] + (run[j + 1][0] - run[j][0]) * f, run[j][1] + (run[j + 1][1] - run[j][1]) * f]);
            }
        }
        out
    }
}

/// Signed area, positive for a counter-clockwise outline.
pub fn area(outline: &[[f64; 2]]) -> f64 {
    let n = outline.len();
    0.5 * (0..n).map(|i| { let (a, b) = (outline[i], outline[(i + 1) % n]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>()
}

/// Refuses too few or too many points, an edge over [`STEP`], a clockwise turn or a self-crossing.
pub fn check(outline: &[[f64; 2]]) -> Result<(), String> {
    let n = outline.len();
    if !(3..=crate::setting::MAX_STAMP_POINTS).contains(&n) {
        return Err(format!("{n} points, where a stamp takes 3 to {}", crate::setting::MAX_STAMP_POINTS));
    }
    let long = (0..n).map(|i| { let (a, b) = (outline[i], outline[(i + 1) % n]); (a[0] - b[0]).hypot(a[1] - b[1]) }).fold(0.0, f64::max);
    if long > STEP + 1e-9 {
        return Err(format!("an edge runs {long:.3} mm"));
    }
    if area(outline) <= 0.0 {
        return Err("it runs clockwise".into());
    }
    if let Some((i, j)) = self_crossing(outline) {
        return Err(format!("edges {i} and {j} cross"));
    }
    Ok(())
}

/// The first two edges that cross, by the index of their first point.
pub fn self_crossing(outline: &[[f64; 2]]) -> Option<(usize, usize)> {
    let n = outline.len();
    let orient = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let boxes: Vec<[f64; 4]> = (0..n).map(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
    }).collect();
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (p, q) = (boxes[i], boxes[j]);
            if p[2] < q[0] || q[2] < p[0] || p[3] < q[1] || q[3] < p[1] {
                continue;
            }
            let (a, b, c, d) = (outline[i], outline[(i + 1) % n], outline[j], outline[(j + 1) % n]);
            let (d1, d2, d3, d4) = (orient(c, d, a), orient(c, d, b), orient(a, b, c), orient(a, b, d));
            if d1 * d2 <= 0.0 && d3 * d4 <= 0.0 {
                return Some((i, j));
            }
        }
    }
    None
}

/// A circle `diameter` across.
pub fn circle(diameter: f64) -> Vec<[f64; 2]> {
    let r = diameter.max(0.05) * 0.5;
    let mut pen = Pen::new();
    pen.corner([r, 0.0]);
    pen.curve(|t| [r * t.cos(), r * t.sin()], 0.0, TAU, false);
    pen.finish()
}

/// An elongated hexagon pointed along `x`, its ends blunted `tip` across, flanks turning at 0.4 of the half-length.
pub fn keel(length: f64, width: f64, tip: f64) -> Vec<[f64; 2]> {
    let (a, b) = (length.max(0.2) * 0.5, width.max(0.1) * 0.5);
    let t = (tip * 0.5).clamp(0.0, 0.9 * b);
    let corners = [[-a, -t], [-0.4 * a, -b], [0.4 * a, -b], [a, -t], [a, t], [0.4 * a, b], [-0.4 * a, b], [-a, t]];
    let mut pen = Pen::new();
    for c in corners {
        pen.corner(c);
    }
    pen.finish()
}

/// A polygon with every corner filleted by an arc of `radius`, limited by the edges either side.
pub fn rounded_polygon(corners: &[[f64; 2]], radius: f64) -> Vec<[f64; 2]> {
    let n = corners.len();
    let mut pen = Pen::new();
    for i in 0..n {
        let (p, v, q) = (corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n]);
        let (l1, l2) = ((p[0] - v[0]).hypot(p[1] - v[1]), (q[0] - v[0]).hypot(q[1] - v[1]));
        let u1 = [(p[0] - v[0]) / l1.max(1e-12), (p[1] - v[1]) / l1.max(1e-12)];
        let u2 = [(q[0] - v[0]) / l2.max(1e-12), (q[1] - v[1]) / l2.max(1e-12)];
        let half = 0.5 * (u1[0] * u2[0] + u1[1] * u2[1]).clamp(-1.0, 1.0).acos();
        if radius <= 0.0 || half < 1e-6 || half > 0.5 * PI - 1e-6 {
            pen.corner(v);
            continue;
        }
        let d = (radius / half.tan()).min(0.49 * l1.min(l2));
        let r = d * half.tan();
        let bis = [u1[0] + u2[0], u1[1] + u2[1]];
        let bl = bis[0].hypot(bis[1]).max(1e-12);
        let c = [v[0] + bis[0] / bl * r / half.sin(), v[1] + bis[1] / bl * r / half.sin()];
        let (t1, t2) = ([v[0] + u1[0] * d, v[1] + u1[1] * d], [v[0] + u2[0] * d, v[1] + u2[1] * d]);
        let (a1, mut a2) = ((t1[1] - c[1]).atan2(t1[0] - c[0]), (t2[1] - c[1]).atan2(t2[0] - c[0]));
        let convex = (v[0] - p[0]) * (q[1] - v[1]) - (v[1] - p[1]) * (q[0] - v[0]) > 0.0;
        if convex {
            while a2 < a1 { a2 += TAU; }
        } else {
            while a2 > a1 { a2 -= TAU; }
        }
        pen.corner(t1);
        pen.curve(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()], a1, a2, true);
    }
    pen.finish()
}

/// An isosceles triangle pointing along `+x`, centroid on the origin, corners rounded by `radius`.
pub fn rounded_triangle(width: f64, height: f64, radius: f64) -> Vec<[f64; 2]> {
    let (w, h) = (width.max(0.2) * 0.5, height.max(0.2));
    rounded_polygon(&[[-h / 3.0, -w], [2.0 * h / 3.0, 0.0], [-h / 3.0, w]], radius)
}

/// A keel hexagon `length` by `width` with every corner rounded by `round`.
pub fn comb_lobe(length: f64, width: f64, round: f64) -> Vec<[f64; 2]> {
    let (a, b) = (length.max(0.2) * 0.5, width.max(0.1) * 0.5);
    rounded_polygon(&[[-a, 0.0], [-0.4 * a, -b], [0.4 * a, -b], [a, 0.0], [0.4 * a, b], [-0.4 * a, b]], round)
}

/// Unit half-width of a blade at parameter `s`, 1 at `s = 0.5`.
fn blade_width(s: f64) -> f64 {
    (4.0 * s * (1.0 - s)).max(0.0)
}

/// A blade widest near `widest` of its length, rounded at `-x` and pointed at `+x`, blunted `tip` across.
fn blade(length: f64, width: f64, widest: f64, tip: f64) -> Vec<[f64; 2]> {
    let (l, w) = (length.max(0.3), width.max(0.1) * 0.5);
    let k = 0.5f64.ln() / widest.clamp(0.1, 0.9).ln();
    let half = |s: f64| w * blade_width(s);
    let end = if tip > 0.0 {
        let (mut lo, mut hi) = (0.5, 1.0);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if half(mid) > 0.5 * tip { lo = mid } else { hi = mid }
        }
        lo
    } else {
        1.0
    };
    let reach = end.powf(1.0 / k);
    let x = |s: f64| -0.5 * l + l * s.clamp(0.0, 1.0).powf(1.0 / k) / reach;
    let mut pen = Pen::new();
    pen.push([x(0.0), 0.0], false);
    pen.curve(|s| [x(s), -half(s)], 0.0, end, true);
    if tip > 0.0 {
        pen.corner([x(end), half(end)]);
    }
    pen.curve(|s| [x(end - s), half(end - s)], 0.0, end, false);
    pen.finish()
}

/// A lance widest a third of the way from its rounded base, pointed at `+x`, blunted `tip` across.
pub fn lanceolate(length: f64, width: f64, tip: f64) -> Vec<[f64; 2]> {
    blade(length, width, 0.35, tip)
}

/// A quill widest a fifth of the way from its rounded base, pointed at `+x`, blunted `tip` across.
pub fn quill(length: f64, width: f64, tip: f64) -> Vec<[f64; 2]> {
    blade(length, width, 0.2, tip)
}

/// The edge of a leaf's blade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Margin {
    Entire,
    /// Saw teeth, `teeth` a side, cut `depth_mm` into the blade and leaning `lean_deg` toward the tip.
    Serrate { teeth: u32, depth_mm: f64, lean_deg: f64 },
    /// Rounded lobes, `lobes` a side, the sinuses between them `depth` of the half-width deep.
    Lobed { lobes: u32, depth: f64 },
    /// Spines, `spines` a side plus the tip, between rounded bays `depth_mm` deep, leaning `lean_deg` tipward.
    Holly { spines: u32, depth_mm: f64, lean_deg: f64 },
}

/// An ovate leaf pointed at `+x` with the given margin, both margins single-valued along `x`.
pub fn leaf(margin: Margin, length: f64, width: f64) -> Vec<[f64; 2]> {
    const WIDEST: f64 = 0.4;
    let (l, w) = (length.max(0.3), width.max(0.1) * 0.5);
    let k = 0.5f64.ln() / WIDEST.ln();
    let x = |s: f64| -0.5 * l + l * s.clamp(0.0, 1.0).powf(1.0 / k);
    let s_of = |xv: f64| ((xv + 0.5 * l) / l).clamp(0.0, 1.0).powf(k);
    let env = |s: f64| w * blade_width(s);
    // The upper margin from base to tip.
    let mut upper = Pen::new();
    upper.push([x(0.0), 0.0], false);
    match margin {
        Margin::Entire => upper.curve(|s| [x(s), env(s)], 0.0, 1.0, true),
        Margin::Lobed { lobes, depth } => {
            let n = lobes.clamp(1, 24) as f64;
            let d = depth.clamp(0.0, 0.9);
            let y = |s: f64| {
                let t = (x(s) + 0.5 * l) / l;
                env(s) * (1.0 - d * (1.0 - (PI * n * t).cos().abs().powf(0.6)))
            };
            let mut from = 0.0;
            for j in 0..n as usize {
                let sinus = s_of(-0.5 * l + l * (j as f64 + 0.5) / n);
                upper.curve(|s| [x(s), y(s)], from, sinus, true);
                from = sinus;
            }
            upper.curve(|s| [x(s), y(s)], from, 1.0, true);
        }
        Margin::Serrate { teeth, depth_mm, lean_deg } | Margin::Holly { spines: teeth, depth_mm, lean_deg } => {
            let holly = matches!(margin, Margin::Holly { .. });
            let n = teeth.clamp(1, 40) as usize;
            let f = (0.5 * (1.0 - lean_deg.to_radians().sin())).clamp(0.1, 0.9);
            let (x0, x1) = (-0.5 * l + 0.14 * l, if holly { 0.5 * l } else { 0.5 * l - 0.08 * l });
            let count = if holly { n + 1 } else { n };
            let tips: Vec<[f64; 2]> = (0..count).map(|j| {
                let xv = x0 + (x1 - x0) * j as f64 / (count - 1).max(1) as f64;
                [xv, env(s_of(xv))]
            }).collect();
            let s0 = s_of(tips[0][0]);
            upper.curve(|s| [x(s), env(s)], 0.0, s0, true);
            for pair in tips.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let xb = a[0] + f * (b[0] - a[0]);
                let yb = (env(s_of(xb)) - depth_mm.max(0.0)).max(0.35 * env(s_of(xb)));
                if holly {
                    upper.curve(|u| [a[0] + u * (xb - a[0]), yb + (a[1] - yb) * (1.0 - u) * (1.0 - u)], 0.0, 1.0, false);
                    upper.curve(|u| [xb + u * (b[0] - xb), yb + (b[1] - yb) * u * u], 0.0, 1.0, true);
                } else {
                    upper.corner([xb, yb]);
                    upper.corner(b);
                }
            }
            if !holly {
                let s1 = s_of(tips[count - 1][0]);
                upper.curve(|s| [x(s), env(s)], s1, 1.0, true);
            }
        }
    }
    let half = upper;
    let m = half.pts.len();
    let mut pen = Pen::new();
    for i in 0..m {
        pen.push([half.pts[i][0], -half.pts[i][1]], half.corner[i]);
    }
    for i in (1..m - 1).rev() {
        pen.push(half.pts[i], half.corner[i]);
    }
    pen.finish()
}

/// A flower `diameter` across its petal tips, one petal along `+x`, notches at half the radius.
pub fn blossom(petals: u32, diameter: f64) -> Vec<[f64; 2]> {
    let n = petals.clamp(3, 16) as f64;
    let r = diameter.max(0.3) * 0.5;
    let rad = |a: f64| r * (0.5 + 0.5 * (0.5 * n * a).cos().abs().sqrt());
    let mut pen = Pen::new();
    pen.corner([rad(0.0), 0.0]);
    for j in 0..2 * n as usize {
        let (from, to) = (j as f64 * PI / n, (j + 1) as f64 * PI / n);
        pen.curve(|a| [rad(a) * a.cos(), rad(a) * a.sin()], from, to, true);
    }
    pen.finish()
}

fn meet(p: [f64; 2], d: [f64; 2], q: [f64; 2], e: [f64; 2]) -> [f64; 2] {
    let den = d[0] * e[1] - d[1] * e[0];
    let t = if den.abs() < 1e-12 { 0.0 } else { ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / den };
    [p[0] + d[0] * t, p[1] + d[1] * t]
}

/// A stem from `-length/2` to a fork at the origin and two arms `length/2` long, `spread_deg` apart, ends rounded.
pub fn fork(length: f64, spread_deg: f64, stem_w: f64, arm_w: f64) -> Vec<[f64; 2]> {
    let a = (0.5 * spread_deg.clamp(20.0, 150.0)).to_radians();
    let (ls, la) = (length.max(0.5) * 0.5, length.max(0.5) * 0.5);
    let (hs, ha) = (stem_w.max(0.05) * 0.5, arm_w.max(0.05) * 0.5);
    let (da, db) = ([a.cos(), a.sin()], [a.cos(), -a.sin()]);
    let left = |d: [f64; 2]| [-d[1], d[0]];
    let right = |d: [f64; 2]| [d[1], -d[0]];
    let at = |d: [f64; 2], t: f64, side: [f64; 2]| [d[0] * t + side[0] * ha, d[1] * t + side[1] * ha];
    let arc = |pen: &mut Pen, c: [f64; 2], r: f64, a0: f64| pen.curve(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()], a0, a0 + PI, true);
    let mut pen = Pen::new();
    pen.corner([-ls, -hs]);
    pen.corner(meet([0.0, -hs], [1.0, 0.0], at(db, 0.0, right(db)), db));
    let end_b = [db[0] * la, db[1] * la];
    pen.corner(at(db, la, right(db)));
    arc(&mut pen, end_b, ha, right(db)[1].atan2(right(db)[0]));
    pen.corner([ha / a.sin(), 0.0]);
    let end_a = [da[0] * la, da[1] * la];
    pen.corner(at(da, la, right(da)));
    arc(&mut pen, end_a, ha, right(da)[1].atan2(right(da)[0]));
    pen.corner(meet([0.0, hs], [1.0, 0.0], at(da, 0.0, left(da)), da));
    pen.corner([-ls, hs]);
    arc(&mut pen, [-ls, 0.0], hs, 0.5 * PI);
    pen.finish()
}

/// A strip winding `turns` counter-clockwise from radius `r0` to `r1`, `w0` to `w1` wide, ends rounded.
pub fn spiral(turns: f64, r0: f64, r1: f64, w0: f64, w1: f64) -> Vec<[f64; 2]> {
    let span = TAU * turns.clamp(0.25, 8.0);
    let r = |p: f64| r0 + (r1 - r0) * p / span;
    let w = |p: f64| 0.5 * (w0 + (w1 - w0) * p / span);
    let at = |p: f64, o: f64| [(r(p) + o) * p.cos(), (r(p) + o) * p.sin()];
    let mut pen = Pen::new();
    pen.corner(at(0.0, w(0.0)));
    pen.curve(|p| at(p, w(p)), 0.0, span, true);
    let c1 = at(span, 0.0);
    pen.curve(|t| [c1[0] + w(span) * t.cos(), c1[1] + w(span) * t.sin()], span, span + PI, true);
    pen.curve(|p| at(span - p, -w(span - p)), 0.0, span, true);
    let c0 = at(0.0, 0.0);
    pen.curve(|t| [c0[0] + w(0.0) * t.cos(), c0[1] + w(0.0) * t.sin()], PI, TAU, true);
    pen.finish()
}

/// An arc `width` deep outside `radius`, sweeping `sweep_deg` about `+y`, with `teeth` teeth `tooth_mm` long pointing inward.
pub fn jaw(radius: f64, width: f64, sweep_deg: f64, teeth: u32, tooth_mm: f64) -> Vec<[f64; 2]> {
    let half = 0.5 * sweep_deg.clamp(10.0, 300.0).to_radians();
    let (a0, a1) = (0.5 * PI - half, 0.5 * PI + half);
    let (ri, ro) = (radius.max(0.3), radius.max(0.3) + width.max(0.1));
    let polar = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
    let mut pen = Pen::new();
    pen.corner(polar(ro, a0));
    pen.curve(|a| polar(ro, a), a0, a1, true);
    pen.corner(polar(ri, a1));
    let n = teeth.clamp(1, 64) as usize;
    let depth = tooth_mm.clamp(0.0, 0.9 * ri);
    for j in 0..n {
        let (b0, b1) = (a1 - (a1 - a0) * j as f64 / n as f64, a1 - (a1 - a0) * (j + 1) as f64 / n as f64);
        pen.corner(polar(ri - depth, 0.5 * (b0 + b1)));
        pen.corner(polar(ri, b1));
    }
    pen.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every family at the sizes the collections ask of it.
    pub(crate) fn families() -> Vec<(&'static str, Vec<[f64; 2]>)> {
        vec![
            ("circle", circle(1.4)),
            ("keel", keel(2.2, 1.1, 0.18)),
            ("lanceolate", lanceolate(5.2, 2.0, 0.3)),
            ("lanceolate, pointed", lanceolate(5.4, 2.8, 0.0)),
            ("leaf, entire", leaf(Margin::Entire, 3.6, 1.4)),
            ("leaf, serrate", leaf(Margin::Serrate { teeth: 5, depth_mm: 0.2, lean_deg: 30.0 }, 2.8, 1.4)),
            ("leaf, lobed", leaf(Margin::Lobed { lobes: 3, depth: 0.45 }, 4.5, 3.2)),
            ("leaf, holly", leaf(Margin::Holly { spines: 3, depth_mm: 0.55, lean_deg: 20.0 }, 8.6, 5.0)),
            ("leaf, holly, small", leaf(Margin::Holly { spines: 3, depth_mm: 0.5, lean_deg: 20.0 }, 5.2, 3.0)),
            ("blossom", blossom(5, 3.4)),
            ("fork", fork(3.6, 50.0, 0.55, 0.45)),
            ("spiral", spiral(2.5, 0.6, 3.6, 0.45, 0.9)),
            ("rounded triangle", rounded_triangle(2.6, 2.0, 0.3)),
            ("comb lobe", comb_lobe(2.2, 1.1, 0.25)),
            ("quill", quill(1.4, 0.45, 0.12)),
            ("jaw", jaw(5.2, 1.4, 150.0, 7, 0.55)),
        ]
    }

    #[test]
    fn every_family_is_closed_counter_clockwise_and_never_crosses_itself() {
        for (name, o) in families() {
            check(&o).unwrap_or_else(|e| panic!("{name}: {e}"));
            let n = o.len();
            let closing = (o[0][0] - o[n - 1][0]).hypot(o[0][1] - o[n - 1][1]);
            assert!(closing > 1e-9 && closing <= STEP + 1e-9, "{name}: closes over {closing}");
            let short = (0..n).map(|i| (o[i][0] - o[(i + 1) % n][0]).hypot(o[i][1] - o[(i + 1) % n][1])).fold(f64::MAX, f64::min);
            assert!(short > 1e-6, "{name}: a zero-length edge");
        }
        // A clockwise copy and a bow tie are refused by name.
        let mut back = circle(1.0);
        back.reverse();
        assert!(check(&back).unwrap_err().contains("clockwise"));
        let bow = [[0.0, 0.0], [0.1, 0.1], [0.1, 0.0], [0.0, 0.1]];
        assert!(self_crossing(&bow).is_some());
    }

    #[test]
    fn each_family_measures_what_it_was_asked() {
        let bounds = |o: &[[f64; 2]]| o.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let (lo, hi) = bounds(&circle(1.4));
        assert!((hi[0] - lo[0] - 1.4).abs() < 1e-6 && (area(&circle(1.4)) - PI * 0.49).abs() < 0.01);
        for (o, l, w) in [(lanceolate(5.2, 2.0, 0.3), 5.2, 2.0), (leaf(Margin::Entire, 3.6, 1.4), 3.6, 1.4), (quill(1.4, 0.45, 0.12), 1.4, 0.45)] {
            let (lo, hi) = bounds(&o);
            assert!((hi[0] - lo[0] - l).abs() < 0.02, "length {} against {l}", hi[0] - lo[0]);
            assert!((hi[1] - lo[1] - w).abs() < 0.01, "width {} against {w}", hi[1] - lo[1]);
            assert!((lo[1] + hi[1]).abs() < 1e-9, "symmetric about its midrib");
        }
        // A lance is widest about a third of the way along, a quill about a fifth.
        let widest = |o: &[[f64; 2]], l: f64| (o.iter().max_by(|a, b| a[1].total_cmp(&b[1])).map(|p| p[0]).unwrap() + 0.5 * l) / l;
        let (lance, quill_at) = (widest(&lanceolate(5.2, 2.0, 0.3), 5.2), widest(&quill(1.4, 0.45, 0.12), 1.4));
        assert!((0.34..0.42).contains(&lance) && (0.19..0.27).contains(&quill_at), "{lance} {quill_at}");
        // The blunt tip is the width asked.
        let tip = lanceolate(5.2, 2.0, 0.3);
        let front = tip.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        let end: Vec<_> = tip.iter().filter(|p| (p[0] - front).abs() < 1e-9).collect();
        assert!((end.iter().map(|p| p[1]).fold(f64::MIN, f64::max) - 0.15).abs() < 1e-6, "{end:?}");
        // A blossom's petals reach the diameter and its notches half of it.
        let b = blossom(5, 3.4);
        let radii: Vec<f64> = b.iter().map(|p| p[0].hypot(p[1])).collect();
        assert!((radii.iter().copied().fold(f64::MIN, f64::max) - 1.7).abs() < 1e-3);
        assert!((radii.iter().copied().fold(f64::MAX, f64::min) - 0.85).abs() < 1e-3);
        // A jaw's teeth reach in to the radius less their length.
        let j = jaw(5.2, 1.4, 150.0, 7, 0.55);
        let inner = j.iter().map(|p| p[0].hypot(p[1])).fold(f64::MAX, f64::min);
        assert!((inner - 4.65).abs() < 1e-9, "{inner}");
        // Every leaf's margins are single-valued along x: each line across it meets the outline twice.
        let margins = [
            Margin::Entire,
            Margin::Serrate { teeth: 5, depth_mm: 0.2, lean_deg: 30.0 },
            Margin::Lobed { lobes: 3, depth: 0.45 },
            Margin::Holly { spines: 3, depth_mm: 0.55, lean_deg: 20.0 },
        ];
        for margin in margins {
            let o = leaf(margin, 8.6, 5.0);
            let n = o.len();
            for c in (1..430).map(|i| -4.3 + 0.02 * i as f64 + 1e-7) {
                let hits = (0..n).filter(|i| (o[*i][0] - c) * (o[(i + 1) % n][0] - c) < 0.0).count();
                assert_eq!(hits, 2, "{margin:?} at x {c}");
            }
        }
    }

    /// `RD_STAMP_SHEET=/dir` renders every family and shaped stamp on a band in studio gold, per cell and as one sheet.
    #[test]
    fn contact_sheet() {
        use crate::setting::{stamp_row, crest_v, RowPath, Stamp, StampRow, StampTop};
        let Some(dir) = std::env::var_os("RD_STAMP_SHEET") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let band = || {
            let mut d = crate::RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::LowDome);
            d.profile.width_mm = 8.0;
            d.profile.thickness_mm = 2.6;
            d
        };
        let centred = |o: Vec<[f64; 2]>| {
            let (lo, hi) = o.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
            let c = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])];
            o.into_iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect::<Vec<_>>()
        };
        let d0 = band();
        let v = crest_v(&d0, 90.0).unwrap();
        let strike = |name: &str, outline: Vec<[f64; 2]>, height: f64, top: StampTop| Stamp {
            name: name.into(), theta_deg: 90.0, v_mm: v, rot_deg: 0.0, outline, height_mm: height, sink_mm: 0.3, draft_deg: 0.0,
            cut: false, bench: false, along_pull: false, tier: 0, top,
        };
        let mut cells: Vec<(String, crate::RingDesign, f64)> = Vec::new();
        let sheet_families: Vec<(&str, Vec<[f64; 2]>)> = families().into_iter().map(|(n, o)| match n {
            "spiral" => (n, spiral(2.5, 0.5, 2.8, 0.4, 0.7)),
            "jaw" => (n, jaw(2.0, 0.9, 150.0, 7, 0.35)),
            _ => (n, o),
        }).collect();
        for (name, o) in sheet_families {
            let mut d = band();
            d.stamps = vec![strike(name, centred(o), 0.4, StampTop::Flat)];
            cells.push((name.to_string(), d, 0.0));
        }
        let mut d = band();
        d.stamps = vec![
            strike("Plate", circle(4.2), 0.35, StampTop::Flat),
            Stamp { tier: 1, ..strike("Plate: keel", keel(3.0, 1.5, 0.18), 0.3, StampTop::Gable { rise_mm: 0.3, axis_deg: 0.0 }) },
        ];
        cells.push(("tier, gabled keel on a plate".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![strike("Horn", rounded_triangle(2.6, 2.0, 0.3), 0.25, StampTop::Cone { apex_mm: 0.9, at: [0.35, 0.0], tip_mm: 0.3 })];
        cells.push(("cone, horn".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![strike("Seal", circle(3.4), 0.25, StampTop::Dome { crown_mm: 0.5 })];
        cells.push(("dome".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![strike("Beak", lanceolate(5.2, 2.0, 0.3), 0.3, StampTop::Gable { rise_mm: 0.35, axis_deg: 0.0 })];
        cells.push(("gable, beak".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![Stamp { v_mm: v + 1.8, ..strike("Keel", keel(4.0, 1.6, 0.18), 0.3, StampTop::Gable { rise_mm: 0.3, axis_deg: 0.0 }) }];
        cells.push(("gable, off the parting line".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![strike("Rib", keel(4.4, 1.0, 0.2), 0.2, StampTop::Ridge { rise_mm: 0.4, from: [-1.6, 0.0], to: [1.8, 0.0], end_mm: 0.1 })];
        cells.push(("ridge, rib".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![strike("Quill", quill(3.0, 0.8, 0.2), 0.45, StampTop::Taper { axis_deg: 0.0, tip_mm: 0.12 })];
        cells.push(("taper, quill".into(), d, 0.0));
        let mut d = band();
        let proto = strike("Thorn", circle(1.1), 0.2, StampTop::Cone { apex_mm: 0.55, at: [0.0, 0.0], tip_mm: 0.25 });
        d.stamps = stamp_row(&d, &StampRow { stamp: proto, path: RowPath::PartingLine, from_deg: 90.0, to_deg: 118.0, count: 6, taper: 0.5, fold_clear_mm: 0.0, mirror_shoulders: true });
        cells.push(("row of cone thorns, graded and mirrored".into(), d, 0.0));
        let mut d = band();
        let holly = leaf(Margin::Holly { spines: 3, depth_mm: 0.55, lean_deg: 20.0 }, 6.4, 3.8);
        d.stamps = strike("Holly", holly, 0.4, StampTop::Flat).cast_as_hull(0.2);
        cells.push(("holly cast as its hull, bays cut at the bench".into(), d, 0.0));
        let mut d = band();
        d.stamps = vec![strike("Blossom", blossom(5, 3.8), 0.35, StampTop::Flat)];
        let mut pad = crate::field::SeatPadLayer { theta_deg: 90.0, v_mm: v, style: crate::field::SeatStyle::GypsyMound, blend_mm: 0.3, solid: crate::setting::SolidKind::Bead, ..Default::default() };
        pad.fit_stone(crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 1.3));
        pad.height_mm = 0.0;
        d.layers.layers.push(crate::field::LayerEntry::new("Heart", crate::field::Layer::SeatPad(pad)));
        cells.push(("blossom with a stone set in its heart".into(), d, 0.0));

        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 768, profile_steps: 256, ..Default::default() };
        const EDGE: usize = 420;
        let mut shots = Vec::new();
        let mut index = String::new();
        for (k, (name, d, _)) in cells.iter().enumerate() {
            let built = crate::mesh::try_build(d, &lib, params).unwrap();
            assert!(built.solids.notes.is_empty(), "{name}: {:?}", built.solids.notes);
            let ctx = d.field_context();
            let origins: Vec<[f64; 3]> = d.stamps.iter().map(|s| s.frame(d, &ctx).origin).collect();
            let reach = d.stamps.iter().map(|s| s.outline.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max)).fold(0.0, f64::max) + 1.2;
            let near = |p: &crate::mesh::Vec3| origins.iter().any(|o| ((p.0 as f64 - o[0]).powi(2) + (p.1 as f64 - o[1]).powi(2) + (p.2 as f64 - o[2]).powi(2)).sqrt() < reach);
            let m = &built.mesh;
            let mut keep = vec![u32::MAX; m.vertices.len()];
            let mut crop = crate::Mesh::default();
            let mut cursor = 0;
            for (fi, f) in m.faces.iter().enumerate() {
                if !f.iter().all(|i| near(&m.vertices[*i as usize])) {
                    continue;
                }
                let corners = m.face_normals(fi, &mut cursor);
                crop.corner_normals.push((crop.faces.len() as u32, corners));
                crop.faces.push(f.map(|i| {
                    if keep[i as usize] == u32::MAX {
                        keep[i as usize] = crop.vertices.len() as u32;
                        crop.vertices.push(m.vertices[i as usize]);
                        crop.normals.push(m.normals[i as usize]);
                    }
                    keep[i as usize]
                }));
            }
            let stones = crate::gems::preview_mesh(d, &lib);
            let mut parts = vec![crate::render::Part::metal(&crop, crate::render::GOLD)];
            if let Some(s) = &stones {
                parts.push(crate::render::Part::stone(s));
            }
            let img = crate::render::render_parts_ss(&parts, 0.9, 0.75, EDGE, EDGE, 3);
            let file = format!("{:02}-{}.png", k + 1, name.replace([' ', ','], "-").replace("--", "-"));
            image::save_buffer(dir.join(&file), &img, EDGE as u32, EDGE as u32, image::ColorType::Rgb8).unwrap();
            index.push_str(&format!("{file}\t{name}\t{} faces\n", built.mesh.faces.len()));
            shots.push(img);
        }
        let cols = 5;
        let rows = shots.len().div_ceil(cols);
        let (w, h) = (cols * EDGE, rows * EDGE);
        let mut sheet = vec![18u8; w * h * 3];
        for (k, img) in shots.iter().enumerate() {
            let (cx, cy) = ((k % cols) * EDGE, (k / cols) * EDGE);
            for y in 0..EDGE {
                let row = ((cy + y) * w + cx) * 3;
                sheet[row..row + EDGE * 3].copy_from_slice(&img[y * EDGE * 3..(y + 1) * EDGE * 3]);
            }
        }
        image::save_buffer(dir.join("stamp-sheet.png"), &sheet, w as u32, h as u32, image::ColorType::Rgb8).unwrap();
        std::fs::write(dir.join("index.txt"), index).unwrap();
    }

    #[test]
    fn keel_is_the_caiman_horn_outline() {
        let (a_half, b_half, tip): (f64, f64, f64) = (1.37, 0.43, 0.09);
        let corners = [
            [-a_half, -tip], [-0.4 * a_half, -b_half], [0.4 * a_half, -b_half], [a_half, -tip],
            [a_half, tip], [0.4 * a_half, b_half], [-0.4 * a_half, b_half], [-a_half, tip],
        ];
        let mut want = Vec::new();
        for (c, n) in corners.iter().zip(corners.iter().cycle().skip(1)) {
            let steps = ((c[0] - n[0]).hypot(c[1] - n[1]) / 0.1).ceil().max(1.0) as usize;
            want.extend((0..steps).map(|q| {
                let f = q as f64 / steps as f64;
                [c[0] + (n[0] - c[0]) * f, c[1] + (n[1] - c[1]) * f]
            }));
        }
        assert_eq!(keel(2.0 * a_half, 2.0 * b_half, 2.0 * tip), want);
    }
}
