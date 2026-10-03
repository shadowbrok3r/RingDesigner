//! The twisted sweep as a mesh: a closed section carried along a planar or spatial path, turned and scaled per station, capped or joined round, closed by construction.
use super::SurfaceKind;
use super::builders::Made;
use crate::csg::{P3, Solid};
use crate::setting::Named;
use crate::sketch::region;
use anyhow::{Context, Result, bail, ensure};
use cadkernel::geom2d::Curve;
use cadkernel::space::Plane;
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

/// The key a twisted sweep's mesh carries as a mesh value.
pub const TWIST: &str = "sweep.twist";
/// Most the section turns between two stations, degrees.
pub const TWIST_STEP_DEG: f64 = 3.0;
/// Most the path turns between two stations, degrees.
pub const BEND_STEP_DEG: f64 = 5.0;
/// Turn at a joint of the section from which the sides part there, degrees.
pub const CORNER_DEG: f64 = 1.0;
/// Most stations along the path.
pub const MAX_STATIONS: usize = 8192;
/// Most points round the section.
pub const MAX_SECTION: usize = 4096;
/// Most triangles one sweep makes.
pub const MAX_TRIANGLES: usize = 2_000_000;
/// Most points a path in space runs through.
pub const MAX_PATH_POINTS: usize = 256;
/// Most knots a scale law takes.
pub const MAX_LAW_KNOTS: usize = 64;
/// Least and most scale a law may give the section.
pub const LAW_SCALES: [f64; 2] = [1e-3, 100.0];
/// A leaf's scale law: a narrow stalk, widest a third of the way along, and a point.
pub const LEAF_LAW: [[f64; 2]; 3] = [[0.0, 0.1], [0.35, 1.0], [1.0, 0.05]];
/// A thorn's scale law: a root flared to 1.4, the section's own size a quarter of the way along, and a tip at 0.28.
pub const THORN_LAW: [[f64; 2]; 3] = [[0.0, 1.4], [0.25, 1.0], [1.0, 0.28]];
/// Furthest a section point may stand off the plane square to the path at its start, mm.
const SQUARE_MM: f64 = 1e-6;
/// Most times a curved piece of the section is halved to hold the chord.
const MAX_SPLITS: usize = 1024;
/// Samples a span of a smooth path is measured at.
const SPAN_SAMPLES: usize = 32;

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
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
    let l = dot(a, a).sqrt();
    (l.is_finite() && l > 1e-300).then(|| a.map(|v| v / l))
}
fn gap2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
/// `v` with its part along the unit `t` taken out.
fn square_to(v: P3, t: P3) -> P3 {
    sub(v, mul(t, dot(v, t)))
}
/// `v` turned `angle` radians about the unit `axis`.
fn rotate(v: P3, axis: P3, angle: f64) -> P3 {
    let (sin, cos) = angle.sin_cos();
    let (c, along) = (cross(axis, v), dot(axis, v) * (1.0 - cos));
    std::array::from_fn(|k| v[k] * cos + c[k] * sin + axis[k] * along)
}
/// `v` turned by the least rotation that takes the unit `from` onto the unit `to`; `None` where they stand opposed.
fn transport(v: P3, from: P3, to: P3) -> Option<P3> {
    let c = cross(from, to);
    let (sin, cos) = (norm(c), dot(from, to).clamp(-1.0, 1.0));
    if sin <= 1e-12 {
        return (cos > 0.0).then_some(v);
    }
    Some(rotate(v, c.map(|x| x / sin), sin.atan2(cos)))
}

/// The line a twisted sweep follows.
#[derive(Clone, Copy, Debug)]
pub enum Path<'a> {
    /// Curves on one plane, whose normal is the section's up; the section stands square to the path at its start.
    Planar { plane: Plane, curves: &'a [Curve] },
    /// Points in space, mitred or a centripetal Catmull-Rom when `smooth`; the section's plane origin rides from the first point.
    Points { points: &'a [P3], smooth: bool },
}

/// How the section's size runs along the path.
#[derive(Clone, Copy, Debug)]
pub enum Scale<'a> {
    /// One at the start, straight to this at the end.
    Linear(f64),
    /// Knots of (share of the length, scale), shares rising, joined by the monotone cubic of [`law_at`].
    Law(&'a [[f64; 2]]),
}
impl Scale<'_> {
    /// The scale at `share` of the length.
    pub fn at(&self, share: f64) -> f64 {
        match self {
            Self::Linear(end) => 1.0 + (end - 1.0) * share,
            Self::Law(knots) => law_at(knots, share),
        }
    }
    /// The largest scale it reaches; a straight run's never below its start.
    fn most(&self) -> f64 {
        match self {
            Self::Linear(end) => end.max(1.0),
            Self::Law(knots) => knots.iter().map(|k| k[1]).fold(0.0, f64::max),
        }
    }
}

/// What a twisted sweep does along its path.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// The section's turn over the whole path, radians; whole turns round a closed one.
    pub twist: f64,
    pub scale: Scale<'a>,
    /// The path closes on itself: the sweep joins round it and has no caps.
    pub closed: bool,
    /// Most a facet stands off the surface it stands for, mm.
    pub chord: f64,
}

/// A scale law's knots checked: 1 to [`MAX_LAW_KNOTS`], shares rising within 0 to 1, scales within [`LAW_SCALES`].
pub fn check_law(knots: &[[f64; 2]]) -> Result<()> {
    ensure!(
        !knots.is_empty() && knots.len() <= MAX_LAW_KNOTS,
        "Twisted sweep: a scale law takes 1 to {MAX_LAW_KNOTS} knots; it has {}",
        knots.len()
    );
    for (i, [share, scale]) in knots.iter().enumerate() {
        ensure!(share.is_finite() && (0.0..=1.0).contains(share), "Twisted sweep: the scale law's knot {} stands at {share}; shares run from 0 to 1", i + 1);
        ensure!(
            scale.is_finite() && *scale >= LAW_SCALES[0] && *scale <= LAW_SCALES[1],
            "Twisted sweep: the scale law's knot {} scales by {scale}; a law scales between {} and {}",
            i + 1,
            LAW_SCALES[0],
            LAW_SCALES[1]
        );
    }
    for w in knots.windows(2) {
        ensure!(w[1][0] > w[0][0], "Twisted sweep: the scale law's shares must rise; {} follows {}", w[1][0], w[0][0]);
    }
    Ok(())
}

/// A scale law at `share`: the monotone cubic (PCHIP) through its knots, held level past either end.
pub fn law_at(knots: &[[f64; 2]], share: f64) -> f64 {
    let n = knots.len();
    if n == 0 {
        return 1.0;
    }
    if n == 1 || share <= knots[0][0] {
        return knots[0][1];
    }
    if share >= knots[n - 1][0] {
        return knots[n - 1][1];
    }
    let k = knots.partition_point(|p| p[0] <= share).clamp(1, n - 1) - 1;
    let ([x0, y0], [x1, y1]) = (knots[k], knots[k + 1]);
    let h = x1 - x0;
    let t = (share - x0) / h;
    let (t2, t3) = (t * t, t * t * t);
    let (d0, d1) = (slope_at(knots, k), slope_at(knots, k + 1));
    (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * h * d0 + (3.0 * t2 - 2.0 * t3) * y1 + (t3 - t2) * h * d1
}

/// The slope the monotone cubic takes at knot `i`.
fn slope_at(knots: &[[f64; 2]], i: usize) -> f64 {
    let n = knots.len();
    let h = |k: usize| knots[k + 1][0] - knots[k][0];
    let delta = |k: usize| (knots[k + 1][1] - knots[k][1]) / h(k);
    if n == 2 {
        return delta(0);
    }
    // An end takes the three-point slope, kept to its secant's sign and within three times it.
    let end = |h0: f64, h1: f64, d0: f64, d1: f64| -> f64 {
        let d = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
        if d0 == 0.0 || d.signum() != d0.signum() {
            0.0
        } else if d0.signum() != d1.signum() && d.abs() > 3.0 * d0.abs() {
            3.0 * d0
        } else {
            d
        }
    };
    if i == 0 {
        return end(h(0), h(1), delta(0), delta(1));
    }
    if i == n - 1 {
        return end(h(n - 2), h(n - 3), delta(n - 2), delta(n - 3));
    }
    let (d0, d1) = (delta(i - 1), delta(i));
    if d0 * d1 <= 0.0 {
        return 0.0;
    }
    let (w1, w2) = (2.0 * h(i) + h(i - 1), h(i) + 2.0 * h(i - 1));
    (w1 + w2) / (w1 / d0 + w2 / d1)
}

/// Stations the share interval `from`..`to` needs for a law's curve to stay within `chord` at `reach`; none for a linear scale.
fn law_need(scale: &Scale, reach: f64, chord: f64, from: f64, to: f64) -> usize {
    let Scale::Law(knots) = scale else { return 0 };
    if knots.len() < 2 || reach.is_nan() || reach <= 0.0 {
        return 0;
    }
    let mut step = f64::INFINITY;
    for k in 0..knots.len() - 1 {
        let ([x0, y0], [x1, y1]) = (knots[k], knots[k + 1]);
        if x1 < from || x0 > to {
            continue;
        }
        let h = x1 - x0;
        let (d0, d1) = (slope_at(knots, k), slope_at(knots, k + 1));
        // The cubic's second derivative is linear across a knot span, so largest at an end.
        let bend = |t: f64| ((12.0 * t - 6.0) * y0 + (6.0 * t - 4.0) * h * d0 + (6.0 - 12.0 * t) * y1 + (6.0 * t - 2.0) * h * d1).abs() / (h * h);
        let most = bend(0.0).max(bend(1.0));
        if most > 1e-12 {
            step = step.min((8.0 * chord / (reach * most)).sqrt());
        }
    }
    if step.is_finite() { ((to - from) / step).ceil() as usize } else { 0 }
}

/// A station: point, direction (a joint's bisector), share of the length, mitre stretch, and in space the section's across, up and mitre bend.
#[derive(Clone, Copy, Debug)]
struct Station {
    at: P3,
    tangent: P3,
    share: f64,
    miter: f64,
    side: P3,
    up: P3,
    bend: P3,
}
impl Station {
    fn plain(at: P3, tangent: P3, share: f64) -> Self {
        Self { at, tangent, share, miter: 1.0, side: [0.0; 3], up: [0.0; 3], bend: [0.0; 3] }
    }
}

/// A section point in the start frame's (across, up), whether the sides part there, and the piece its leaving edge runs along.
#[derive(Clone, Copy, Debug)]
struct Rim {
    at: [f64; 2],
    corner: bool,
    piece: usize,
}

/// The most a curved piece of the section turns between two points, radians: the kernel's own cap at this chord.
fn angle_cap(chord: f64) -> f64 {
    (0.15 * (chord / 0.04).sqrt()).clamp(0.01, 0.15)
}

/// The largest turn, radians, that holds a chord's sag within `chord` at `radius`, and never more than `cap_deg`.
fn step_for(radius: f64, chord: f64, cap_deg: f64) -> f64 {
    let cap = cap_deg.to_radians();
    if radius.is_finite() && radius > chord {
        cap.min(2.0 * (1.0 - chord / radius).acos())
    } else {
        cap
    }
}

/// Where a piece walked from its start (`forward`) or its end is at share `u` of the walk, and its unit direction of travel.
fn walked(plane: &Plane, c: &Curve, forward: bool, u: f64) -> Option<(P3, P3)> {
    let t = if forward { u } else { 1.0 - u };
    let d = plane.vector_at(c.tangent_at(t));
    let d = if forward { d } else { d.map(|v| -v) };
    Some((plane.point_at(c.point_at(t)), unit(d)?))
}

/// How far a path piece turns in all, radians, and the tightest radius it turns at.
fn bend_of(plane: &Plane, c: &Curve) -> (f64, f64) {
    match c {
        Curve::Line(_) => (0.0, f64::INFINITY),
        Curve::Arc(a) => (a.sweep(), a.radius),
        _ => {
            let samples: Vec<(P3, P3)> = (0..=64).filter_map(|k| walked(plane, c, true, k as f64 / 64.0)).collect();
            let (mut total, mut tightest) = (0.0, f64::INFINITY);
            for w in samples.windows(2) {
                let turn = dot(w[0].1, w[1].1).clamp(-1.0, 1.0).acos();
                let run = dot(sub(w[1].0, w[0].0), sub(w[1].0, w[0].0)).sqrt();
                total += turn;
                if turn > 1e-12 {
                    tightest = tightest.min(run / turn);
                }
            }
            (total, tightest)
        }
    }
}

/// The path's curves as one open chain in walking order, each with whether it runs its own way.
fn chain(path: &[Curve]) -> Result<Vec<(Curve, bool)>> {
    ensure!(!path.is_empty(), "Twisted sweep: the path sketch has no curves");
    ensure!(path.len() <= 256, "Twisted sweep: the path has {} curves; the most is 256", path.len());
    for c in path {
        ensure!(
            !c.is_closed(),
            "Twisted sweep: the path closes on itself; an open twisted sweep runs along an open path, so close the sweep to run round it"
        );
        ensure!(
            matches!(c, Curve::Line(_) | Curve::Arc(_) | Curve::Nurbs(_) | Curve::Ellipse(_)),
            "Twisted sweep: the path is drawn with lines, arcs and curves"
        );
    }
    let ends: Vec<[[f64; 2]; 2]> = path.iter().map(|c| [c.point_at(0.0), c.point_at(1.0)]).collect();
    let scale = ends.iter().flatten().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
    let near = |a: [f64; 2], b: [f64; 2]| gap2(a, b) <= 1e-7 * scale;
    let meeting = |p: [f64; 2]| ends.iter().flatten().filter(|q| near(p, **q)).count();
    for e in ends.iter().flatten() {
        ensure!(meeting(*e) <= 2, "Twisted sweep: the path branches at ({:.3}, {:.3}); it follows one unbroken path", e[0], e[1]);
    }
    let loose: Vec<(usize, usize)> = (0..path.len()).flat_map(|i| [(i, 0), (i, 1)]).filter(|&(i, k)| meeting(ends[i][k]) == 1).collect();
    ensure!(
        !loose.is_empty(),
        "Twisted sweep: the path closes on itself; an open twisted sweep runs along an open path, so close the sweep to run round it"
    );
    ensure!(loose.len() == 2, "Twisted sweep: the path is in {} pieces; a twisted sweep follows one unbroken path", loose.len() / 2);
    let (first, side) = loose[0];
    let mut used = vec![false; path.len()];
    used[first] = true;
    let mut out = vec![(path[first].clone(), side == 0)];
    let mut head = ends[first][1 - side];
    while out.len() < path.len() {
        let next = (0..path.len()).filter(|i| !used[*i]).find_map(|i| {
            if near(ends[i][0], head) {
                Some((i, true))
            } else if near(ends[i][1], head) {
                Some((i, false))
            } else {
                None
            }
        });
        let Some((i, forward)) = next else {
            bail!("Twisted sweep: the path breaks off at ({:.3}, {:.3}); join it into one", head[0], head[1]);
        };
        used[i] = true;
        head = ends[i][usize::from(forward)];
        out.push((path[i].clone(), forward));
    }
    Ok(out)
}

/// The path's curves as one closed loop walked from the start of its first curve, each with whether it runs its own way.
fn ring(path: &[Curve]) -> Result<Vec<(Curve, bool)>> {
    ensure!(!path.is_empty(), "Twisted sweep: the path sketch has no curves");
    ensure!(path.len() <= 256, "Twisted sweep: the path has {} curves; the most is 256", path.len());
    // A whole circle is walked as an arc from its angle zero.
    let path: Vec<Curve> = path
        .iter()
        .map(|c| match c {
            Curve::Circle(k) => Curve::Arc(cadkernel::geom2d::Arc { centre: k.centre, radius: k.radius, start_angle: 0.0, end_angle: TAU }),
            c => c.clone(),
        })
        .collect();
    for c in &path {
        ensure!(
            matches!(c, Curve::Line(_) | Curve::Arc(_) | Curve::Nurbs(_) | Curve::Ellipse(_)),
            "Twisted sweep: the path is drawn with lines, arcs and curves"
        );
    }
    let ends: Vec<[[f64; 2]; 2]> = path.iter().map(|c| [c.point_at(0.0), c.point_at(1.0)]).collect();
    let scale = ends.iter().flatten().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
    let near = |a: [f64; 2], b: [f64; 2]| gap2(a, b) <= 1e-7 * scale;
    if path.len() == 1 {
        let [a, b] = ends[0];
        ensure!(
            path[0].is_closed() || near(a, b),
            "Twisted sweep: the path does not close; it ends at ({:.3}, {:.3}), {:.3} mm from its start. Draw it closed or leave the sweep open",
            b[0],
            b[1],
            gap2(a, b)
        );
        return Ok(vec![(path[0].clone(), true)]);
    }
    ensure!(!path.iter().any(Curve::is_closed), "Twisted sweep: the path holds a closed curve among others; a closed sweep follows one loop");
    let meeting = |p: [f64; 2]| ends.iter().flatten().filter(|q| near(p, **q)).count();
    for e in ends.iter().flatten() {
        let met = meeting(*e);
        ensure!(met <= 2, "Twisted sweep: the path branches at ({:.3}, {:.3}); it follows one unbroken path", e[0], e[1]);
        ensure!(met == 2, "Twisted sweep: the path does not close; it ends at ({:.3}, {:.3}). Join its ends or leave the sweep open", e[0], e[1]);
    }
    let mut used = vec![false; path.len()];
    used[0] = true;
    let mut out = vec![(path[0].clone(), true)];
    let mut head = ends[0][1];
    while out.len() < path.len() {
        let next = (0..path.len()).filter(|i| !used[*i]).find_map(|i| {
            if near(ends[i][0], head) {
                Some((i, true))
            } else if near(ends[i][1], head) {
                Some((i, false))
            } else {
                None
            }
        });
        let Some((i, forward)) = next else {
            bail!("Twisted sweep: the path is in more than one loop; a closed sweep follows one");
        };
        used[i] = true;
        head = ends[i][usize::from(forward)];
        out.push((path[i].clone(), forward));
    }
    Ok(out)
}

/// Stations along the walked path within one twist step and `chord` at `reach`, mitred on each joint and kept `reach · tan(θ/2)` off it.
fn stations(plane: &Plane, walk: &[(Curve, bool)], twist: f64, reach: f64, chord: f64, need: &dyn Fn(f64, f64) -> usize) -> Result<Vec<Station>> {
    let lengths: Vec<f64> = walk.iter().map(|(c, _)| c.length()).collect();
    let total: f64 = lengths.iter().sum();
    ensure!(total.is_finite() && total > 1e-6, "Twisted sweep: the path has no length");
    let turn_step = step_for(reach, chord, TWIST_STEP_DEG);
    // Each joint's bisector, the cosine of half its turn, and how far stations keep off it either side.
    let mut joints = Vec::with_capacity(walk.len());
    for k in 0..walk.len() - 1 {
        let (at, before) = walked(plane, &walk[k].0, walk[k].1, 1.0).context("Twisted sweep: the path stops dead")?;
        let (_, after) = walked(plane, &walk[k + 1].0, walk[k + 1].1, 0.0).context("Twisted sweep: the path stops dead")?;
        let bisector = unit(add(before, after)).context("Twisted sweep: the path doubles back on itself")?;
        let cos = dot(bisector, before);
        ensure!(cos > 1e-6, "Twisted sweep: the path doubles back on itself at ({:.3}, {:.3}, {:.3})", at[0], at[1], at[2]);
        let keep = if cos < 1.0 - 1e-12 { 1.1 * reach * (1.0 - cos * cos).sqrt() / cos } else { 0.0 };
        joints.push((at, bisector, cos, keep));
    }
    let gap = |k: usize| if k == 0 || k == walk.len() { 0.0 } else { joints[k - 1].3 };
    let mut out: Vec<Station> = Vec::new();
    let mut run = 0.0;
    for (k, ((c, forward), len)) in walk.iter().zip(&lengths).enumerate() {
        let (g_in, g_out) = (gap(k), gap(k + 1));
        let span = len - g_in - g_out;
        ensure!(
            span > 1e-9,
            "Twisted sweep: the section reaches {:.2} mm round a corner of the path, past the {len:.2} mm run beside it; ease the corner or shrink the section",
            g_in.max(g_out)
        );
        let (bend, radius) = bend_of(plane, c);
        let bend_step = step_for(radius + reach, chord, BEND_STEP_DEG);
        let turns = twist.abs() * span / total;
        let mut n = ((turns / turn_step).ceil() as usize).max((bend * span / len / bend_step).ceil() as usize).max(1).max(need(run / total, (run + len) / total));
        // A curve piece takes twice the stations and at least sixteen.
        if !matches!(c, Curve::Line(_) | Curve::Arc(_)) {
            n = (2 * n).max(16);
        }
        ensure!(
            out.len() + n + 2 < MAX_STATIONS,
            "Twisted sweep: the path would take more than {MAX_STATIONS} stations; twist it less or draw it shorter"
        );
        // Distances along the piece: the path's start, each keep's edge, the run between, the joint at its end.
        let mut marks: Vec<(f64, bool)> = Vec::with_capacity(n + 3);
        if k == 0 {
            marks.push((0.0, false));
        } else if g_in > 0.0 {
            marks.push((g_in, false));
        }
        marks.extend((1..=n).map(|s| (g_in + span * s as f64 / n as f64, false)));
        if k + 1 < walk.len() {
            match marks.last_mut() {
                Some(last) if g_out == 0.0 => *last = (*len, true),
                _ => marks.push((*len, true)),
            }
        }
        for (d, joint) in marks {
            let u = (d / len).clamp(0.0, 1.0);
            let (at, tangent) = walked(plane, c, *forward, u).context("Twisted sweep: the path stops dead")?;
            let along = match c {
                Curve::Line(_) | Curve::Arc(_) => d,
                _ => {
                    let l = c.length_to(if *forward { u } else { 1.0 - u });
                    if *forward { l } else { len - l }
                }
            };
            let share = ((run + along) / total).clamp(0.0, 1.0);
            out.push(match joint {
                true => Station { tangent: joints[k].1, miter: 1.0 / joints[k].2, ..Station::plain(at, tangent, share) },
                false => Station::plain(at, tangent, share),
            });
        }
        run += len;
    }
    Ok(out)
}

/// [`stations`] round a closed loop, the first station being the loop's own joint where its first curve starts.
fn stations_round(plane: &Plane, walk: &[(Curve, bool)], twist: f64, reach: f64, chord: f64, need: &dyn Fn(f64, f64) -> usize) -> Result<Vec<Station>> {
    let count = walk.len();
    let lengths: Vec<f64> = walk.iter().map(|(c, _)| c.length()).collect();
    let total: f64 = lengths.iter().sum();
    ensure!(total.is_finite() && total > 1e-6, "Twisted sweep: the path has no length");
    let turn_step = step_for(reach, chord, TWIST_STEP_DEG);
    // Joint `k` stands where piece `k` ends and the next begins; the last joins the loop back to its first piece.
    let mut joints = Vec::with_capacity(count);
    for k in 0..count {
        let next = (k + 1) % count;
        let (at, before) = walked(plane, &walk[k].0, walk[k].1, 1.0).context("Twisted sweep: the path stops dead")?;
        let (_, after) = walked(plane, &walk[next].0, walk[next].1, 0.0).context("Twisted sweep: the path stops dead")?;
        let bisector = unit(add(before, after)).context("Twisted sweep: the path doubles back on itself")?;
        let cos = dot(bisector, before);
        ensure!(cos > 1e-6, "Twisted sweep: the path doubles back on itself at ({:.3}, {:.3}, {:.3})", at[0], at[1], at[2]);
        let keep = if cos < 1.0 - 1e-12 { 1.1 * reach * (1.0 - cos * cos).sqrt() / cos } else { 0.0 };
        joints.push((at, bisector, cos, keep));
    }
    let mut out: Vec<Station> = Vec::new();
    let mut run = 0.0;
    for (k, ((c, forward), len)) in walk.iter().zip(&lengths).enumerate() {
        let (g_in, g_out) = (joints[(k + count - 1) % count].3, joints[k].3);
        let span = len - g_in - g_out;
        ensure!(
            span > 1e-9,
            "Twisted sweep: the section reaches {:.2} mm round a corner of the path, past the {len:.2} mm run beside it; ease the corner or shrink the section",
            g_in.max(g_out)
        );
        let (bend, radius) = bend_of(plane, c);
        let bend_step = step_for(radius + reach, chord, BEND_STEP_DEG);
        let turns = twist.abs() * span / total;
        let mut n = ((turns / turn_step).ceil() as usize).max((bend * span / len / bend_step).ceil() as usize).max(1).max(need(run / total, (run + len) / total));
        if !matches!(c, Curve::Line(_) | Curve::Arc(_)) {
            n = (2 * n).max(16);
        }
        ensure!(
            out.len() + n + 2 < MAX_STATIONS,
            "Twisted sweep: the path would take more than {MAX_STATIONS} stations; twist it less or draw it shorter"
        );
        let mut marks: Vec<(f64, Option<usize>)> = Vec::with_capacity(n + 3);
        if k == 0 {
            marks.push((0.0, Some(count - 1)));
        }
        if g_in > 0.0 {
            marks.push((g_in, None));
        }
        marks.extend((1..=n).map(|s| (g_in + span * s as f64 / n as f64, None)));
        if k + 1 < count {
            match marks.last_mut() {
                Some(last) if g_out == 0.0 => *last = (*len, Some(k)),
                _ => marks.push((*len, Some(k))),
            }
        } else if g_out == 0.0 {
            // The last piece's end is the loop's start.
            marks.pop();
        }
        for (d, joint) in marks {
            let u = (d / len).clamp(0.0, 1.0);
            let (at, tangent) = walked(plane, c, *forward, u).context("Twisted sweep: the path stops dead")?;
            let along = match c {
                Curve::Line(_) | Curve::Arc(_) => d,
                _ => {
                    let l = c.length_to(if *forward { u } else { 1.0 - u });
                    if *forward { l } else { len - l }
                }
            };
            let share = ((run + along) / total).clamp(0.0, 1.0);
            out.push(match joint {
                Some(j) => Station { tangent: joints[j].1, miter: 1.0 / joints[j].2, ..Station::plain(at, tangent, share) },
                None => Station::plain(at, tangent, share),
            });
        }
        run += len;
    }
    Ok(out)
}

/// A path in space checked: 2 to [`MAX_PATH_POINTS`] coordinates, every step moving; a closed one's repeated end dropped.
fn prepared(points: &[P3], closed: bool) -> Result<Vec<P3>> {
    ensure!(
        (2..=MAX_PATH_POINTS).contains(&points.len()),
        "Twisted sweep: a path in space runs through 2 to {MAX_PATH_POINTS} points; it has {}",
        points.len()
    );
    ensure!(
        points.iter().flatten().all(|v| v.is_finite() && v.abs() < 10000.0),
        "Twisted sweep: the path has a point that is not a finite coordinate below 10000 mm"
    );
    let mut p = points.to_vec();
    if closed && p.len() > 2 && norm(sub(p[0], p[p.len() - 1])) <= 1e-9 {
        p.pop();
    }
    ensure!(!closed || p.len() >= 3, "Twisted sweep: a closed path runs through three points at least");
    let steps = if closed { p.len() } else { p.len() - 1 };
    for k in 0..steps {
        let a = p[k];
        ensure!(
            norm(sub(p[(k + 1) % p.len()], a)) > 1e-6,
            "Twisted sweep: the path stays put at ({:.3}, {:.3}, {:.3}); every step along it must move",
            a[0],
            a[1],
            a[2]
        );
    }
    Ok(p)
}

/// Stations along a polyline in space: runs within one twist step and a mitred station on each corner, kept `reach · tan(θ/2)` off it.
fn polyline_stations(p: &[P3], closed: bool, twist: f64, reach: f64, chord: f64, need: &dyn Fn(f64, f64) -> usize) -> Result<Vec<Station>> {
    let count = if closed { p.len() } else { p.len() - 1 };
    let ends = |k: usize| (p[k], p[(k + 1) % p.len()]);
    let lengths: Vec<f64> = (0..count).map(|k| norm(sub(ends(k).1, ends(k).0))).collect();
    let total: f64 = lengths.iter().sum();
    ensure!(total.is_finite() && total > 1e-6, "Twisted sweep: the path has no length");
    let dirs: Vec<P3> = (0..count).map(|k| unit(sub(ends(k).1, ends(k).0))).collect::<Option<_>>().context("Twisted sweep: the path stops dead")?;
    let turn_step = step_for(reach, chord, TWIST_STEP_DEG);
    // A closed loop's frame correction, at most half a turn, counts toward the twist.
    let twist = twist.abs() + if closed { PI } else { 0.0 };
    // Joint `k` stands where run `k` ends and the next begins; a closed path's last joins it back to its first.
    let joined = if closed { count } else { count - 1 };
    let mut joints = Vec::with_capacity(joined);
    for k in 0..joined {
        let (before, after, at) = (dirs[k], dirs[(k + 1) % count], ends(k).1);
        let bisector = unit(add(before, after)).with_context(|| format!("Twisted sweep: the path doubles back on itself at ({:.3}, {:.3}, {:.3})", at[0], at[1], at[2]))?;
        let cos = dot(bisector, before);
        ensure!(cos > 1e-6, "Twisted sweep: the path doubles back on itself at ({:.3}, {:.3}, {:.3})", at[0], at[1], at[2]);
        let keep = if cos < 1.0 - 1e-12 { 1.1 * reach * (1.0 - cos * cos).sqrt() / cos } else { 0.0 };
        joints.push((at, bisector, cos, keep, unit(sub(after, before)).unwrap_or([0.0; 3])));
    }
    let keep_in = |k: usize| if k > 0 { joints[k - 1].3 } else if closed { joints[count - 1].3 } else { 0.0 };
    let keep_out = |k: usize| if k < joined { joints[k].3 } else { 0.0 };
    let mut out: Vec<Station> = Vec::new();
    let mut run = 0.0;
    for k in 0..count {
        let (len, start, t) = (lengths[k], ends(k).0, dirs[k]);
        let (g_in, g_out) = (keep_in(k), keep_out(k));
        let span = len - g_in - g_out;
        ensure!(
            span > 1e-9,
            "Twisted sweep: the section reaches {:.2} mm round a corner of the path, past the {len:.2} mm run beside it; ease the corner or shrink the section",
            g_in.max(g_out)
        );
        let n = ((twist * span / total / turn_step).ceil() as usize).max(1).max(need(run / total, (run + len) / total));
        ensure!(
            out.len() + n + 2 < MAX_STATIONS,
            "Twisted sweep: the path would take more than {MAX_STATIONS} stations; twist it less or draw it shorter"
        );
        let mut marks: Vec<(f64, Option<usize>)> = Vec::with_capacity(n + 3);
        if k == 0 {
            marks.push((0.0, closed.then_some(count - 1)));
        }
        if g_in > 0.0 {
            marks.push((g_in, None));
        }
        marks.extend((1..=n).map(|s| (g_in + span * s as f64 / n as f64, None)));
        if k + 1 < count {
            match marks.last_mut() {
                Some(last) if g_out == 0.0 => *last = (len, Some(k)),
                _ => marks.push((len, Some(k))),
            }
        } else if closed && g_out == 0.0 {
            marks.pop();
        }
        for (d, joint) in marks {
            let share = ((run + d) / total).clamp(0.0, 1.0);
            out.push(match joint {
                Some(j) => {
                    let (at, bisector, cos, _, bend) = joints[j];
                    Station { miter: 1.0 / cos, bend, ..Station::plain(at, bisector, share) }
                }
                None => Station::plain(add(start, mul(t, d)), t, share),
            });
        }
        run += len;
    }
    Ok(out)
}

/// One span of a centripetal Catmull-Rom curve, between two of its points, as a cubic Hermite over 0 to 1.
#[derive(Clone, Copy, Debug)]
struct Span {
    p1: P3,
    p2: P3,
    m1: P3,
    m2: P3,
}
impl Span {
    fn point(&self, s: f64) -> P3 {
        let (s2, s3) = (s * s, s * s * s);
        let h = [2.0 * s3 - 3.0 * s2 + 1.0, s3 - 2.0 * s2 + s, 3.0 * s2 - 2.0 * s3, s3 - s2];
        std::array::from_fn(|k| h[0] * self.p1[k] + h[1] * self.m1[k] + h[2] * self.p2[k] + h[3] * self.m2[k])
    }
    fn velocity(&self, s: f64) -> P3 {
        let s2 = s * s;
        let h = [6.0 * s2 - 6.0 * s, 3.0 * s2 - 4.0 * s + 1.0, 6.0 * s - 6.0 * s2, 3.0 * s2 - 2.0 * s];
        std::array::from_fn(|k| h[0] * self.p1[k] + h[1] * self.m1[k] + h[2] * self.p2[k] + h[3] * self.m2[k])
    }
}

/// The spans of the centripetal Catmull-Rom curve through `p`; an open curve's end points mirror their neighbours.
fn spans(p: &[P3], closed: bool) -> Vec<Span> {
    let n = p.len();
    let at = |i: isize| -> P3 {
        if closed {
            p[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            sub(mul(p[0], 2.0), p[1])
        } else if i as usize >= n {
            sub(mul(p[n - 1], 2.0), p[n - 2])
        } else {
            p[i as usize]
        }
    };
    let count = if closed { n } else { n - 1 };
    (0..count as isize)
        .map(|k| {
            let [q0, q1, q2, q3] = [at(k - 1), at(k), at(k + 1), at(k + 2)];
            let gap = |a: P3, b: P3| norm(sub(b, a)).sqrt();
            let (d0, d1, d2) = (gap(q0, q1), gap(q1, q2), gap(q2, q3));
            let m1 = std::array::from_fn(|i| ((q1[i] - q0[i]) / d0 - (q2[i] - q0[i]) / (d0 + d1) + (q2[i] - q1[i]) / d1) * d1);
            let m2 = std::array::from_fn(|i| ((q2[i] - q1[i]) / d1 - (q3[i] - q1[i]) / (d1 + d2) + (q3[i] - q2[i]) / d2) * d1);
            Span { p1: q1, p2: q2, m1, m2 }
        })
        .collect()
}

/// Points and directions along a path in space as a twisted sweep runs it, `per_span` steps to each run between two points: the
/// polyline itself, or the centripetal Catmull-Rom curve through it when `smooth`. A closed path ends on its first point.
pub fn path_points(points: &[P3], smooth: bool, closed: bool, per_span: usize) -> Result<Vec<(P3, P3)>> {
    let p = prepared(points, closed)?;
    let per = per_span.max(1);
    let mut out = Vec::new();
    if smooth {
        for (k, span) in spans(&p, closed).iter().enumerate() {
            for i in usize::from(k > 0)..=per {
                let s = i as f64 / per as f64;
                out.push((span.point(s), unit(span.velocity(s)).unwrap_or([0.0; 3])));
            }
        }
        return Ok(out);
    }
    let n = p.len();
    let runs = if closed { n } else { n - 1 };
    for k in 0..runs {
        let (a, b) = (p[k], p[(k + 1) % n]);
        let d = unit(sub(b, a)).unwrap_or([0.0; 3]);
        for i in usize::from(k > 0)..=per {
            out.push((add(a, mul(sub(b, a), i as f64 / per as f64)), d));
        }
    }
    Ok(out)
}

/// The parameter at arc length `d` along a span, read off its table of lengths at even parameters.
fn param_at(lengths: &[f64], d: f64) -> f64 {
    let n = lengths.len() - 1;
    let i = lengths.partition_point(|l| *l <= d).clamp(1, n) - 1;
    let run = lengths[i + 1] - lengths[i];
    let f = if run > 0.0 { ((d - lengths[i]) / run).clamp(0.0, 1.0) } else { 0.0 };
    (i as f64 + f) / n as f64
}

/// Stations along the Catmull-Rom curve through `p`, even by length in each span, within one twist step and the bend `chord` allows at `reach`.
fn spline_stations(p: &[P3], closed: bool, twist: f64, reach: f64, chord: f64, need: &dyn Fn(f64, f64) -> usize) -> Result<Vec<Station>> {
    let spans = spans(p, closed);
    let mut tables = Vec::with_capacity(spans.len());
    for span in &spans {
        let at: Vec<P3> = (0..=SPAN_SAMPLES).map(|i| span.point(i as f64 / SPAN_SAMPLES as f64)).collect();
        let dir: Vec<P3> = (0..=SPAN_SAMPLES)
            .map(|i| unit(span.velocity(i as f64 / SPAN_SAMPLES as f64)))
            .collect::<Option<_>>()
            .with_context(|| format!("Twisted sweep: the path stops dead near ({:.3}, {:.3}, {:.3})", span.p1[0], span.p1[1], span.p1[2]))?;
        let mut lengths = vec![0.0; SPAN_SAMPLES + 1];
        // The most the curve turns per millimetre anywhere along the span.
        let mut rate = 0.0_f64;
        for i in 0..SPAN_SAMPLES {
            let run = norm(sub(at[i + 1], at[i]));
            lengths[i + 1] = lengths[i] + run;
            let turn = dot(dir[i], dir[i + 1]).clamp(-1.0, 1.0).acos();
            if run > 0.0 {
                rate = rate.max(turn / run);
            }
        }
        tables.push((lengths, rate));
    }
    let total: f64 = tables.iter().map(|(l, _)| l[SPAN_SAMPLES]).sum();
    ensure!(total.is_finite() && total > 1e-6, "Twisted sweep: the path has no length");
    let turn_step = step_for(reach, chord, TWIST_STEP_DEG);
    // A closed loop's frame correction, at most half a turn, counts toward the twist.
    let twist = twist.abs() + if closed { PI } else { 0.0 };
    let mut out: Vec<Station> = Vec::new();
    let mut run = 0.0;
    for (k, (span, (lengths, rate))) in spans.iter().zip(&tables).enumerate() {
        let len = lengths[SPAN_SAMPLES];
        let radius = if *rate > 1e-12 { 1.0 / rate } else { f64::INFINITY };
        let bend_step = step_for(radius + reach, chord, BEND_STEP_DEG);
        let n = ((twist * len / total / turn_step).ceil() as usize).max((len * rate / bend_step).ceil() as usize).max(1).max(need(run / total, (run + len) / total));
        ensure!(
            out.len() + n + 2 < MAX_STATIONS,
            "Twisted sweep: the path would take more than {MAX_STATIONS} stations; twist it less or draw it shorter"
        );
        // A span starts where the last one ended, and a closed curve's last ends where it began.
        let first = usize::from(k > 0);
        let last = if closed && k + 1 == spans.len() { n - 1 } else { n };
        for i in first..=last {
            let d = len * i as f64 / n as f64;
            let s = param_at(lengths, d);
            let tangent = unit(span.velocity(s)).context("Twisted sweep: the path stops dead")?;
            out.push(Station::plain(span.point(s), tangent, ((run + d) / total).clamp(0.0, 1.0)));
        }
        run += len;
    }
    Ok(out)
}

/// Lays a rotation-minimising frame on every station from the section plane's x axis; returns the turn a closed loop must give back.
fn carry(st: &mut [Station], plane: &Plane, closed: bool) -> Result<f64> {
    let t0 = st[0].tangent;
    let x = square_to(plane.x_axis, t0);
    let first = if norm(x) > 1e-6 { unit(x) } else { unit(square_to(plane.y_axis, t0)) }.context("Twisted sweep: the section's plane has no axes")?;
    let mut side = first;
    for i in 0..st.len() {
        if i > 0 {
            side = transport(side, st[i - 1].tangent, st[i].tangent).context("Twisted sweep: the path doubles back on itself")?;
        }
        let t = st[i].tangent;
        // Held square to the tangent against rounding.
        side = unit(square_to(side, t)).context("Twisted sweep: the path doubles back on itself")?;
        st[i].side = side;
        st[i].up = cross(t, side);
    }
    if !closed {
        return Ok(0.0);
    }
    let round = transport(side, st[st.len() - 1].tangent, t0).context("Twisted sweep: the path doubles back on itself")?;
    let round = unit(square_to(round, t0)).context("Twisted sweep: the path doubles back on itself")?;
    Ok(-dot(cross(first, round), t0).atan2(dot(first, round)))
}

/// What a section piece is, for the surface its side sweeps.
#[derive(Clone, Copy, Debug)]
enum Piece {
    Line,
    Round { centre: [f64; 2], radius: f64 },
    Other,
}

/// The walked section as points on its own plane, each piece cut to `chord` and `longest`, a corner where the walk turns.
fn section(profile: &[Curve], senses: &[bool], chord: f64, longest: f64) -> Result<(Vec<([f64; 2], bool, usize)>, Vec<Piece>)> {
    let tangent = |c: &Curve, forward: bool, at_end: bool| -> [f64; 2] {
        let t = if forward == at_end { 1.0 } else { 0.0 };
        let d = c.tangent_at(t);
        let l = d[0].hypot(d[1]).max(1e-300);
        if forward { [d[0] / l, d[1] / l] } else { [-d[0] / l, -d[1] / l] }
    };
    let count = profile.len();
    let mut out = Vec::new();
    let mut pieces = Vec::with_capacity(count);
    for (i, (c, forward)) in profile.iter().zip(senses).enumerate() {
        let (p, pf) = (&profile[(i + count - 1) % count], senses[(i + count - 1) % count]);
        let (before, after) = (tangent(p, pf, true), tangent(c, *forward, false));
        let turn = (before[0] * after[0] + before[1] * after[1]).clamp(-1.0, 1.0).acos();
        let corner = turn >= CORNER_DEG.to_radians();
        let len = c.length();
        ensure!(len.is_finite() && len > 1e-9, "Twisted sweep: the section has a piece of no length");
        let fewest = if longest.is_finite() { (len / longest).ceil() as usize } else { 1 };
        let n = match c {
            Curve::Line(_) => fewest.max(1),
            Curve::Arc(a) => fewest.max((a.sweep() / step_for(a.radius, chord, angle_cap(chord).to_degrees())).ceil() as usize).max(2),
            _ => {
                let mut n = fewest.max(4);
                // The worst sag of a chord off the curve, and the worst turn from one chord to the next.
                let rough = |n: usize| {
                    let p: Vec<[f64; 2]> = (0..=n).map(|k| c.point_at(k as f64 / n as f64)).collect();
                    let sag = (0..n)
                        .map(|k| gap2(c.point_at((k as f64 + 0.5) / n as f64), [0.5 * (p[k][0] + p[k + 1][0]), 0.5 * (p[k][1] + p[k + 1][1])]))
                        .fold(0.0, f64::max);
                    let turn = p
                        .windows(3)
                        .map(|w| {
                            let (u, v) = ([w[1][0] - w[0][0], w[1][1] - w[0][1]], [w[2][0] - w[1][0], w[2][1] - w[1][1]]);
                            (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]).abs()
                        })
                        .fold(0.0, f64::max);
                    sag > chord || turn > angle_cap(chord)
                };
                while rough(n) && n < MAX_SPLITS {
                    n *= 2;
                }
                n
            }
        };
        ensure!(out.len() + n <= MAX_SECTION, "Twisted sweep: the section would take more than {MAX_SECTION} points round");
        for k in 0..n {
            let u = k as f64 / n as f64;
            out.push((c.point_at(if *forward { u } else { 1.0 - u }), k == 0 && corner, i));
        }
        pieces.push(match c {
            Curve::Line(_) => Piece::Line,
            Curve::Arc(a) => Piece::Round { centre: a.centre, radius: a.radius },
            _ => Piece::Other,
        });
    }
    Ok((out, pieces))
}

/// Triangles filling a counter-clockwise simple polygon from its own points only, each counter-clockwise.
fn fill(ring: &[[f64; 2]]) -> Result<Vec<[u32; 3]>> {
    use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
    let n = ring.len();
    ensure!(n >= 3, "Twisted sweep: the section has {n} points round; a cap needs three");
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    let mut index = HashMap::new();
    let mut handles = Vec::with_capacity(n);
    for (i, p) in ring.iter().enumerate() {
        let h = cdt.insert(Point2::new(p[0], p[1])).map_err(|e| anyhow::anyhow!("Twisted sweep: the section's point {i} will not place ({e:?})"))?;
        ensure!(index.insert(h, i as u32).is_none(), "Twisted sweep: the section meets itself at ({:.4}, {:.4})", p[0], p[1]);
        handles.push(h);
    }
    for i in 0..n {
        let (a, b) = (handles[i], handles[(i + 1) % n]);
        ensure!(cdt.can_add_constraint(a, b), "Twisted sweep: the section crosses itself");
        cdt.add_constraint(a, b);
    }
    // Faces flood from the hull: outside against a hull edge the outline does not hold, flipping across each outline edge.
    let mut inside = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for e in cdt.convex_hull() {
        if let Some(f) = e.face().as_inner().or_else(|| e.rev().face().as_inner()) {
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(f.fix()) {
                v.insert(cdt.is_constraint_edge(e.as_undirected().fix()));
                queue.push_back(f.fix());
            }
        }
    }
    while let Some(f) = queue.pop_front() {
        let here = inside[&f];
        for e in cdt.face(f).adjacent_edges() {
            let Some(next) = e.rev().face().as_inner() else { continue };
            let edge = cdt.is_constraint_edge(e.as_undirected().fix());
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(next.fix()) {
                v.insert(if edge { !here } else { here });
                queue.push_back(next.fix());
            }
        }
    }
    let mut tris = Vec::with_capacity(n - 2);
    for face in cdt.inner_faces() {
        if !inside.get(&face.fix()).copied().unwrap_or(false) {
            continue;
        }
        let [a, b, c] = face.vertices().map(|v| index[&v.fix()]);
        let [pa, pb, pc] = [a, b, c].map(|i| ring[i as usize]);
        let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pc[0] - pa[0]) * (pb[1] - pa[1]);
        tris.push(if area >= 0.0 { [a, b, c] } else { [a, c, b] });
    }
    ensure!(tris.len() == n - 2, "Twisted sweep: the section's cap did not fill ({} triangles for {n} points round)", tris.len());
    Ok(tris)
}

/// A closed `profile` carried along `path`, turned, scaled and capped or joined round as `o` says, within `o.chord` mm.
pub fn sweep(profile_plane: Plane, profile: &[Curve], path: Path<'_>, o: Options<'_>) -> Result<Made> {
    let end_scale = match o.scale {
        Scale::Linear(k) => k,
        Scale::Law(_) => 1.0,
    };
    ensure!(
        o.twist.is_finite() && end_scale.is_finite() && end_scale > 0.0 && o.chord.is_finite() && o.chord > 0.0,
        "Twisted sweep: the twist and end scale must be finite and the scale positive"
    );
    if let Scale::Law(knots) = o.scale {
        check_law(knots)?;
    }
    if o.closed {
        let turns = o.twist / TAU;
        ensure!(
            (turns - turns.round()).abs() < 1e-9,
            "Twisted sweep: a closed sweep turns its section whole turns round the loop, so its ends meet; {:.4}° is not",
            o.twist.to_degrees()
        );
        let (first, last) = (o.scale.at(0.0), o.scale.at(1.0));
        ensure!(
            (first - last).abs() <= 1e-9 * first.max(last),
            "Twisted sweep: a closed sweep's scale ends where it starts, so its ends meet; it runs from {first} to {last}"
        );
    }
    let senses = region::senses(profile).context("Twisted sweep: the section does not close into one loop")?;
    match path {
        Path::Planar { plane, curves } => planar(profile_plane, profile, &senses, plane, curves, &o),
        Path::Points { points, smooth } => spatial(profile_plane, profile, &senses, points, smooth, &o),
    }
}

/// The sweep along curves on one plane, which keeps that plane's normal for the section's up.
fn planar(profile_plane: Plane, profile: &[Curve], senses: &[bool], path_plane: Plane, path: &[Curve], o: &Options) -> Result<Made> {
    let (twist, chord) = (o.twist, o.chord);
    let normal = path_plane.normal().and_then(unit).context("Twisted sweep: the path's plane has no normal")?;
    let mut walk = if o.closed { ring(path)? } else { chain(path)? };
    let rough: Vec<P3> = profile.iter().flat_map(|c| (0..8).map(move |k| profile_plane.point_at(c.point_at(k as f64 / 8.0)))).collect();
    let off = |walk: &[(Curve, bool)]| -> Option<f64> {
        let (at, t) = walked(&path_plane, &walk[0].0, walk[0].1, 0.0)?;
        Some(rough.iter().map(|p| dot(sub(*p, at), t).abs()).fold(0.0, f64::max))
    };
    let here = off(&walk).context("Twisted sweep: the path stops dead where it starts")?;
    if here > SQUARE_MM {
        ensure!(
            !o.closed,
            "Twisted sweep: the section must stand square to the path where it starts, the start of the loop's first curve; it stands {here:.3} mm off that plane"
        );
        // A section at the far end walks the path back.
        let back: Vec<(Curve, bool)> = walk.iter().rev().map(|(c, f)| (c.clone(), !f)).collect();
        match off(&back) {
            Some(there) if there <= SQUARE_MM => walk = back,
            _ => bail!("Twisted sweep: the section must stand square to the path where it starts; it stands {here:.3} mm off that plane"),
        }
    }
    let (origin, start) = walked(&path_plane, &walk[0].0, walk[0].1, 0.0).context("Twisted sweep: the path stops dead where it starts")?;
    let across = cross(normal, start);
    let local = |p: P3| -> [f64; 2] {
        let d = sub(p, origin);
        [dot(d, across), dot(d, normal)]
    };
    let reach = rough.iter().map(|p| local(*p)).map(|q| q[0].hypot(q[1])).fold(0.0, f64::max) * o.scale.most();
    let need = |from: f64, to: f64| law_need(&o.scale, reach, chord, from, to);
    let st = if o.closed { stations_round(&path_plane, &walk, twist, reach, chord, &need)? } else { stations(&path_plane, &walk, twist, reach, chord, &need)? };
    // Section edges no longer than the chord over half the largest turn between two stations.
    let turn = turn_between(&st, twist.abs(), o.closed);
    let longest = if turn > 1e-12 { chord / (0.5 * turn).sin() } else { f64::INFINITY };
    let (points, pieces) = section(profile, senses, chord, longest)?;
    let rims = counter_clockwise(points.iter().map(|(p, corner, piece)| Rim { at: local(profile_plane.point_at(*p)), corner: *corner, piece: *piece }).collect())?;
    let m = rims.len();
    let triangles = budget(st.len(), m, o.closed)?;
    let mut v: Vec<P3> = Vec::with_capacity(st.len() * m);
    for s in &st {
        let (sin, cos) = (twist * s.share).sin_cos();
        let scale = o.scale.at(s.share);
        let side = cross(normal, s.tangent);
        for r in &rims {
            let [a, b] = r.at;
            let out = (a * cos - b * sin) * scale * s.miter;
            let up = (a * sin + b * cos) * scale;
            v.push(std::array::from_fn(|k| s.at[k] + side[k] * out + normal[k] * up));
        }
    }
    let straight = !o.closed && twist.abs() < 1e-12 && walk.len() == 1 && matches!(walk[0].0, Curve::Line(_));
    assemble(v, &st, &rims, &pieces, straight, o, triangles)
}

/// The sweep through points in space, its section laid square to the start and carried on a rotation-minimising frame.
fn spatial(profile_plane: Plane, profile: &[Curve], senses: &[bool], points: &[P3], smooth: bool, o: &Options) -> Result<Made> {
    let (twist, chord) = (o.twist, o.chord);
    let p = prepared(points, o.closed)?;
    // The section about its own plane's origin, which rides the path.
    let rough: Vec<[f64; 2]> = profile.iter().flat_map(|c| (0..8).map(move |k| c.point_at(k as f64 / 8.0))).collect();
    let reach = rough.iter().map(|q| q[0].hypot(q[1])).fold(0.0, f64::max) * o.scale.most();
    let need = |from: f64, to: f64| law_need(&o.scale, reach, chord, from, to);
    let mut st = if smooth { spline_stations(&p, o.closed, twist, reach, chord, &need)? } else { polyline_stations(&p, o.closed, twist, reach, chord, &need)? };
    // The twist plus the closed loop's frame correction, spread over the length.
    let turned = twist + carry(&mut st, &profile_plane, o.closed)?;
    let turn = turn_between(&st, turned.abs(), o.closed);
    let longest = if turn > 1e-12 { chord / (0.5 * turn).sin() } else { f64::INFINITY };
    let (points, pieces) = section(profile, senses, chord, longest)?;
    let rims = counter_clockwise(points.iter().map(|(q, corner, piece)| Rim { at: *q, corner: *corner, piece: *piece }).collect())?;
    let m = rims.len();
    let triangles = budget(st.len(), m, o.closed)?;
    let mut v: Vec<P3> = Vec::with_capacity(st.len() * m);
    for s in &st {
        let (sin, cos) = (turned * s.share).sin_cos();
        let scale = o.scale.at(s.share);
        for r in &rims {
            let [a, b] = r.at;
            let (out, up) = ((a * cos - b * sin) * scale, (a * sin + b * cos) * scale);
            let mut d: P3 = std::array::from_fn(|k| s.side[k] * out + s.up[k] * up);
            // A mitre stretches the section along the way the path bends.
            if s.miter != 1.0 {
                let stretch = dot(d, s.bend) * (s.miter - 1.0);
                d = std::array::from_fn(|k| d[k] + s.bend[k] * stretch);
            }
            v.push(add(s.at, d));
        }
    }
    let straight = !o.closed && twist.abs() < 1e-12 && p.len() == 2;
    assemble(v, &st, &rims, &pieces, straight, o, triangles)
}

/// The most the section turns, at `rate` over the length, between neighbouring stations, the closing band included.
fn turn_between(st: &[Station], rate: f64, closed: bool) -> f64 {
    let open = st.windows(2).map(|w| rate * (w[1].share - w[0].share)).fold(0.0, f64::max);
    if closed { open.max(rate * (1.0 - st[st.len() - 1].share)) } else { open }
}

/// The rim turned counter-clockwise in (across, up), so the sides face out; refused when it encloses nothing.
fn counter_clockwise(mut rims: Vec<Rim>) -> Result<Vec<Rim>> {
    let m = rims.len();
    let area: f64 = (0..m)
        .map(|j| {
            let (a, b) = (rims[j].at, rims[(j + 1) % m].at);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        * 0.5;
    ensure!(area.abs() > 1e-12, "Twisted sweep: the section encloses no area");
    if area < 0.0 {
        let old = rims.clone();
        rims = (0..m).map(|j| Rim { at: old[(m - j) % m].at, corner: old[(m - j) % m].corner, piece: old[(2 * m - j - 1) % m].piece }).collect();
    }
    Ok(rims)
}

/// The triangles `stations` stations of an `m`-point section make, capped or joined round; refused past [`MAX_TRIANGLES`].
fn budget(stations: usize, m: usize, closed: bool) -> Result<usize> {
    let triangles = if closed { stations * m * 2 } else { (stations - 1) * m * 2 + 2 * (m - 2) };
    ensure!(triangles <= MAX_TRIANGLES, "Twisted sweep would take {triangles} triangles; the most is {MAX_TRIANGLES}: twist it less, or shrink the section");
    Ok(triangles)
}

/// The placed rings `v` as a part: sides station to station and round when closed, caps when open, faces named, corners drawn, checked closed and uncrossed.
fn assemble(v: Vec<P3>, st: &[Station], rims: &[Rim], pieces: &[Piece], straight: bool, o: &Options, triangles: usize) -> Result<Made> {
    let (m, n, closed) = (rims.len(), st.len(), o.closed);
    let bands = if closed { n } else { n - 1 };
    for i in 0..bands {
        let (here, next) = (&st[i], &st[(i + 1) % n]);
        for j in 0..m {
            let d = sub(v[((i + 1) % n) * m + j], v[i * m + j]);
            ensure!(
                dot(d, here.tangent) > 0.0 && dot(d, next.tangent) > 0.0,
                "Twisted sweep folds through itself {:.0}% along the path: the section reaches past the middle of a bend, or past a corner's reach. Ease the path or shrink the section",
                100.0 * here.share
            );
        }
    }
    // The sides part at every corner of the section.
    let corners: Vec<usize> = (0..m).filter(|j| rims[*j].corner).collect();
    let run_of: Vec<usize> = match corners.first() {
        None => vec![0; m],
        Some(&first) => {
            let mut run = vec![0; m];
            let mut r = corners.len() - 1;
            for step in 0..m {
                let j = (first + step) % m;
                if rims[j].corner {
                    r = (r + 1) % corners.len();
                }
                run[j] = r;
            }
            run
        }
    };
    let runs = corners.len().max(1);
    let (straight, end_scale) = match o.scale {
        Scale::Linear(k) => (straight, k),
        Scale::Law(_) => (false, 1.0),
    };
    let mut kinds: Vec<SurfaceKind> = (0..runs)
        .map(|r| {
            let own: Vec<Piece> = (0..m).filter(|j| run_of[*j] == r).map(|j| pieces[rims[j].piece]).collect();
            let one_line = own.iter().all(|p| matches!(p, Piece::Line)) && (0..m).filter(|j| run_of[*j] == r).map(|j| rims[j].piece).collect::<std::collections::BTreeSet<_>>().len() == 1;
            let round = match own.first() {
                Some(Piece::Round { centre, radius }) => own.iter().all(|p| matches!(p, Piece::Round { centre: c, radius: q } if gap2(*c, *centre) < 1e-9 && (q - radius).abs() < 1e-9)),
                _ => false,
            };
            match (straight, one_line, round) {
                (true, true, _) => SurfaceKind::Plane,
                (true, _, true) if (end_scale - 1.0).abs() < 1e-12 => SurfaceKind::Cylinder,
                (true, _, true) => SurfaceKind::Cone,
                _ => SurfaceKind::Freeform,
            }
        })
        .collect();
    let mut names: Vec<String> = (0..runs).map(|r| if runs == 1 { "Side".to_string() } else { format!("Side {}", r + 1) }).collect();
    if !closed {
        kinds.extend([SurfaceKind::Plane; 2]);
        names.extend(["Start cap".to_string(), "End cap".to_string()]);
    }
    let mut f: Vec<[u32; 3]> = Vec::with_capacity(triangles);
    let mut patch: Vec<u32> = Vec::with_capacity(triangles);
    let at = |i: usize, j: usize| ((i % n) * m + j % m) as u32;
    for i in 0..bands {
        for j in 0..m {
            let (a, b, c, d) = (at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j));
            // Diagonals alternate like a chequerboard.
            if (i + j) % 2 == 0 {
                f.extend([[a, b, c], [a, c, d]]);
            } else {
                f.extend([[a, b, d], [b, c, d]]);
            }
            patch.extend([run_of[j] as u32; 2]);
        }
    }
    let last = n - 1;
    if !closed {
        let cap = fill(&rims.iter().map(|r| r.at).collect::<Vec<_>>())?;
        for t in &cap {
            f.push([at(0, t[0] as usize), at(0, t[2] as usize), at(0, t[1] as usize)]);
            patch.push(runs as u32);
        }
        for t in &cap {
            f.push(t.map(|k| at(last, k as usize)));
            patch.push(runs as u32 + 1);
        }
    }
    let solid = Solid { v, f };
    let (open, repeated) = solid.open_edges();
    ensure!(open == 0 && repeated == 0, "Twisted sweep did not close ({open} open edges, {repeated} repeated)");
    let crossings = crate::csg::self_crossings(&solid);
    ensure!(crossings == 0, "Twisted sweep crosses itself ({crossings} pairs of faces meet); ease the path, twist it less or shrink the section");
    // Creases: a rail along every corner, round to its start on a closed path, and an open one's cap rims cut at the corners.
    let rail = if closed { n + 1 } else { n };
    let mut creases: Vec<Vec<P3>> = corners.iter().map(|j| (0..rail).map(|i| solid.v[at(i, *j) as usize]).collect()).collect();
    if !closed {
        for i in [0, last] {
            match corners.as_slice() {
                [] => creases.push((0..=m).map(|j| solid.v[at(i, j) as usize]).collect()),
                all => {
                    for (k, from) in all.iter().enumerate() {
                        let to = all[(k + 1) % all.len()];
                        let span = (to + m - from) % m;
                        let span = if span == 0 { m } else { span };
                        creases.push((0..=span).map(|s| solid.v[at(i, from + s) as usize]).collect());
                    }
                }
            }
        }
    }
    creases.truncate(2048);
    Ok(Made { key: TWIST.to_string(), named: Named { solid, patch, names }, kinds, creases, gem: None, seat: None, stations: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Attach, Component, Document, Feature, Operation, Placement};
    use crate::sketch::{Geometry, Sketch, Workplane};
    use crate::{AlphaLibrary, BuildParams, RingDesign};

    const PREVIEW: BuildParams = BuildParams { theta_steps: 256, profile_steps: 128, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };
    const EXPORT: BuildParams = BuildParams { theta_steps: 1024, profile_steps: 320, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };

    /// A path up the finger's axis from the origin, `len` long, drawn on the section plane.
    fn up(len: f64) -> Sketch {
        let mut path = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let (a, b) = (path.point([0.0, 0.0]), path.point([0.0, len]));
        path.entity(Geometry::Line { a, b });
        path
    }
    fn twist(section: Sketch, path: Sketch, degrees: f64, end_scale: f64) -> Operation {
        Operation::twist(section, path, degrees, end_scale)
    }
    fn design(op: Operation) -> RingDesign {
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Twisted sweep".into(), enabled: true, operation: op, component: Component::default() }).unwrap();
        RingDesign { cad: Some(doc), ..RingDesign::default() }
    }
    fn lib() -> &'static AlphaLibrary {
        static LIB: std::sync::OnceLock<AlphaLibrary> = std::sync::OnceLock::new();
        LIB.get_or_init(AlphaLibrary::builtin)
    }
    /// The one part `op` evaluates to, or why it failed.
    fn made(op: Operation, params: BuildParams) -> std::result::Result<(Made, crate::Mesh), String> {
        let e = crate::cad::evaluate(&design(op), lib(), params).map_err(|e| format!("{e:#}"))?;
        if let Some(why) = e.first_error() {
            return Err(why);
        }
        let c = e.components.into_iter().next().ok_or("no part")?;
        Ok(((*c.made.ok_or("not a mesh")?).clone(), c.mesh))
    }
    fn closed(m: &Made, mesh: &crate::Mesh, what: &str) {
        assert_eq!(m.solid().open_edges(), (0, 0), "{what}");
        let v = mesh.validate();
        assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{what}: {v:?}");
        assert_eq!(crate::csg::self_crossings(m.solid()), 0, "{what}");
    }

    #[test]
    fn a_twisted_sweep_closes_and_keeps_its_section_at_every_twist() {
        // The starter's 2 x 1.5 mm rectangle up an 8 mm path: every section is the rectangle turned, so the volume is area times length.
        for params in [PREVIEW, EXPORT] {
            for degrees in [0.0, 30.0, 90.0, 180.0, 270.0, 360.0, 450.0, 540.0, 630.0, 720.0, -720.0] {
                let started = std::time::Instant::now();
                let (m, mesh) = made(twist(Sketch::rectangle(2.0, 1.5), up(8.0), degrees, 1.0), params).unwrap();
                let ms = started.elapsed().as_secs_f64() * 1e3;
                let what = format!("{degrees}° at {} steps", params.theta_steps);
                closed(&m, &mesh, &what);
                let v = m.solid().volume();
                assert!((v / 24.0 - 1.0).abs() < 0.005, "{what}: {v} against 24");
                if degrees == 720.0 {
                    eprintln!("rectangle 720° at {} steps: {} triangles, {v:.4} mm³ against 24 ({:+.4}%), {ms:.1} ms", params.theta_steps, m.solid().f.len(), (v / 24.0 - 1.0) * 100.0);
                }
            }
        }
        // A section off the path's axis sweeps a helix round it, and a round one keeps its polygon's area.
        let mut off = Sketch::default();
        off.add_rectangle([5.0, -1.5], [8.0, 1.5], false).unwrap();
        for degrees in [0.0, 90.0, 720.0] {
            let (m, mesh) = made(twist(off.clone(), up(5.0), degrees, 1.0), PREVIEW).unwrap();
            closed(&m, &mesh, "off the axis");
            assert!((m.solid().volume() / 45.0 - 1.0).abs() < 0.005, "{degrees}°: {} against 45", m.solid().volume());
        }
        for params in [PREVIEW, EXPORT] {
            let (m, mesh) = made(twist(Sketch::circle(0.8), up(6.0), 720.0, 1.0), params).unwrap();
            closed(&m, &mesh, "round");
            let round = std::f64::consts::PI * 0.64 * 6.0;
            assert!((m.solid().volume() / round - 1.0).abs() < 0.005, "{} against {round}", m.solid().volume());
        }
        // Ten turns, the most a twist takes, at the export chord.
        let started = std::time::Instant::now();
        let (m, mesh) = made(twist(Sketch::circle(1.5), up(12.0), 3600.0, 1.0), EXPORT).unwrap();
        let swept = started.elapsed().as_secs_f64() * 1e3;
        let started = std::time::Instant::now();
        closed(&m, &mesh, "ten turns");
        let checked = started.elapsed().as_secs_f64() * 1e3;
        let round = std::f64::consts::PI * 2.25 * 12.0;
        assert!((m.solid().volume() / round - 1.0).abs() < 0.005, "{} against {round}", m.solid().volume());
        eprintln!("ten turns of a 3 mm round at the export chord: {} triangles, {:.4} mm³ against {round:.4}, swept in {swept:.1} ms, closure and crossings checked in {checked:.1} ms", m.solid().f.len(), m.solid().volume());
        // Scaled toward its end, the section's area runs as the square of the scale: the mean of 1, k and k².
        let (m, mesh) = made(twist(Sketch::rectangle(2.0, 1.5), up(8.0), 360.0, 0.5), PREVIEW).unwrap();
        closed(&m, &mesh, "tapered");
        let want = 24.0 * (1.0 + 0.5 + 0.25) / 3.0;
        assert!((m.solid().volume() / want - 1.0).abs() < 0.005, "{} against {want}", m.solid().volume());
    }

    #[test]
    fn a_twisted_sweep_names_its_faces_and_draws_its_corners() {
        let (square, _) = made(twist(Sketch::rectangle(2.0, 1.5), up(8.0), 180.0, 1.0), PREVIEW).unwrap();
        assert_eq!(square.key, TWIST);
        assert_eq!(square.named.names, ["Side 1", "Side 2", "Side 3", "Side 4", "Start cap", "End cap"]);
        assert_eq!(square.kinds[..4], [SurfaceKind::Freeform; 4], "a twisted side is no plane");
        assert!(square.named.census().iter().all(|n| *n > 0), "every patch holds faces");
        // Four corner rails, and each cap's rim cut at the four corners.
        assert_eq!(square.creases.len(), 12);
        assert_eq!(square.corners().len(), 8, "the corners snap at both ends");
        let (flat, _) = made(twist(Sketch::rectangle(2.0, 1.5), up(8.0), 0.0, 1.0), PREVIEW).unwrap();
        assert_eq!(flat.kinds, [SurfaceKind::Plane; 6], "untwisted, a prism's sides are planes");
        let (round, _) = made(twist(Sketch::circle(1.0), up(4.0), 90.0, 1.0), PREVIEW).unwrap();
        assert_eq!(round.named.names, ["Side", "Start cap", "End cap"]);
        assert_eq!(round.creases.len(), 2, "a round section draws only its rims");
        let (rod, _) = made(twist(Sketch::circle(1.0), up(4.0), 0.0, 1.0), PREVIEW).unwrap();
        assert_eq!(rod.kinds, [SurfaceKind::Cylinder, SurfaceKind::Plane, SurfaceKind::Plane]);
    }

    #[test]
    fn a_twisted_sweep_follows_bends_and_corners_and_says_where_it_cannot() {
        // An L of two lines meets in a mitre; an arc bends round its centre; a cubic curve takes its own tangents.
        let mut l = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let p = [[0.0, 0.0], [0.0, 6.0], [5.0, 6.0]].map(|p| l.point(p));
        l.entity(Geometry::Polyline { points: p.to_vec(), closed: false });
        let mut bend = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let (c, a, b) = (bend.point([6.0, 0.0]), bend.point([0.0, 0.0]), bend.point([6.0, 6.0]));
        bend.entity(Geometry::Arc { center: c, start: b, end: a });
        let mut curve = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let q = [[0.0, 0.0], [0.0, 3.0], [2.0, 5.0], [4.0, 8.0]].map(|p| curve.point(p));
        curve.entity(Geometry::Bezier { points: q });
        for (name, path) in [("corner", l), ("bend", bend), ("curve", curve)] {
            for degrees in [0.0, 180.0, 720.0] {
                let (m, mesh) = made(twist(Sketch::rectangle(1.2, 0.8), path.clone(), degrees, 1.0), PREVIEW).unwrap_or_else(|e| panic!("{name} {degrees}°: {e}"));
                closed(&m, &mesh, &format!("{name} {degrees}°"));
            }
        }
        // A section drawn at the path's far end sweeps back from there.
        let mut far = Sketch::rectangle(2.0, 1.5);
        far.plane.origin = [0.0, 0.0, 8.0];
        let (m, mesh) = made(twist(far, up(8.0), 90.0, 1.0), PREVIEW).unwrap();
        closed(&m, &mesh, "from the far end");
        assert!((m.solid().volume() / 24.0 - 1.0).abs() < 0.005);
        // Refused by name: a section lying along the path, a closed path, a section wider than the bend it rounds.
        let mut along = Sketch { plane: Workplane::section(), ..Sketch::default() };
        along.add_rectangle([1.0, 1.0], [2.0, 3.0], false).unwrap();
        let why = made(twist(along, up(8.0), 90.0, 1.0), PREVIEW).unwrap_err();
        assert!(why.contains("the section must stand square to the path where it starts"), "{why}");
        let mut ring = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let (c, r) = (ring.point([0.0, 5.0]), ring.point([0.0, 0.0]));
        ring.entity(Geometry::Circle { center: c, rim: r });
        let why = made(twist(Sketch::rectangle(1.0, 1.0), ring, 90.0, 1.0), PREVIEW).unwrap_err();
        assert!(why.contains("the path closes on itself"), "{why}");
        let mut tight = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let (c, a, b) = (tight.point([1.0, 0.0]), tight.point([0.0, 0.0]), tight.point([1.0, 1.0]));
        tight.entity(Geometry::Arc { center: c, start: b, end: a });
        let why = made(twist(Sketch::rectangle(4.0, 1.0), tight, 0.0, 1.0), PREVIEW).unwrap_err();
        assert!(why.contains("folds through itself"), "{why}");
        // A curve that loops back across its own start carries the section through itself.
        let mut looped = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let q = [[0.0, 0.0], [10.0, 10.0], [-10.0, 10.0], [1.0, -1.0]].map(|p| looped.point(p));
        looped.entity(Geometry::Bezier { points: q });
        let mut square = Sketch::rectangle(1.0, 1.0);
        let h = std::f64::consts::FRAC_1_SQRT_2;
        square.plane = Workplane { origin: [0.0; 3], x: [0.0, 1.0, 0.0], y: [-h, 0.0, h], on_face: None };
        let why = made(twist(square, looped, 0.0, 1.0), PREVIEW).unwrap_err();
        assert!(why.contains("Twisted sweep crosses itself"), "{why}");
    }

    #[test]
    fn a_twisted_post_joins_the_court_band_and_the_ring_stays_watertight() {
        let lib = lib();
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        // Sunk a millimetre into the band and standing three out of it, turned half round.
        let component = Component { attach: Attach::Join, placement: Placement::ring(90.0, -1.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Twisted post".into(), enabled: true, operation: twist(Sketch::rectangle(2.0, 1.5), up(4.0), 180.0, 1.0), component }).unwrap();
        for params in [PREVIEW, EXPORT] {
            let bare = crate::mesh::try_build(&d, &lib, params).unwrap().mesh.volume_mm3();
            let with = RingDesign { cad: Some(doc.clone()), ..d.clone() };
            let started = std::time::Instant::now();
            let b = crate::mesh::try_build(&with, &lib, params).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1e3;
            assert!(b.parts.notes.is_empty(), "{:?}", b.parts.notes);
            assert_eq!(b.parts.joined, 1);
            let v = b.report.validation;
            assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
            let grew = b.mesh.volume_mm3() - bare;
            eprintln!("twisted post on the Court band at {} steps: +{grew:.4} mm³ of the post's 12, {} faces, {ms:.1} ms", params.theta_steps, b.mesh.faces.len());
            // What stands out of the band: the post's three millimetres over the crest, and a little more where the crown falls away under its edges.
            assert!(grew > 9.0 && grew < 9.5, "{grew}");
        }
        d.cad = Some(doc);
        assert!(d.band_is_procedural());
    }

    use crate::cad::TwistPath;

    /// A twisted sweep along `points`, with every option spelled out.
    fn through(section: Sketch, points: &[P3], smooth: bool, degrees: f64, scale: &[[f64; 2]], closed: bool) -> Operation {
        Operation::Twist { sketch: section.into(), path: TwistPath::Points { points: points.to_vec(), smooth }, degrees, end_scale: 1.0, scale: scale.to_vec(), closed }
    }
    /// A lens `length` long and `width` wide across its middle, of two arcs, centred on the origin.
    fn lens(length: f64, width: f64) -> Sketch {
        let (a, b) = (length / 2.0, width / 2.0);
        // Each arc's centre stands `c` beyond the far side, its radius reaching both tips and the middle.
        let c = (a * a - b * b) / (2.0 * b);
        let mut s = Sketch::default();
        let (right, left) = (s.point([a, 0.0]), s.point([-a, 0.0]));
        let (under, over) = (s.point([0.0, -c]), s.point([0.0, c]));
        s.entity(Geometry::Arc { center: under, start: right, end: left });
        s.entity(Geometry::Arc { center: over, start: left, end: right });
        s
    }
    /// A five-pointed star `outer` to its points and `inner` to its notches, a point straight up.
    fn star(outer: f64, inner: f64) -> Sketch {
        let mut s = Sketch::default();
        let ids: Vec<_> = (0..10)
            .map(|k| {
                let (r, a) = (if k % 2 == 0 { outer } else { inner }, std::f64::consts::FRAC_PI_2 + PI * k as f64 / 5.0);
                s.point([r * a.cos(), r * a.sin()])
            })
            .collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
        s
    }
    /// One cane of a wreath of four round the finger, through `points` stations, crossing three times each way.
    fn cane(points: usize) -> Vec<P3> {
        (0..points)
            .map(|k| {
                let t = TAU * k as f64 / points as f64;
                let r = 10.85 + 0.55 * (3.0 * t).cos();
                [r * t.cos(), r * t.sin(), 2.1 * (3.0 * t).sin()]
            })
            .collect()
    }

    #[test]
    fn a_scale_law_is_a_monotone_cubic_that_never_overshoots_its_knots() {
        // The leaf: narrow at the stalk, its full width a third along and level there, a point at the tip.
        let leaf: Vec<f64> = (0..=1000).map(|k| law_at(&LEAF_LAW, k as f64 / 1000.0)).collect();
        assert_eq!((leaf[0], leaf[350], leaf[1000]), (0.1, 1.0, 0.05));
        assert!(leaf.iter().all(|s| (0.05..=1.0).contains(s)), "the cubic overshoots a knot");
        assert!(leaf[..=350].windows(2).all(|w| w[1] >= w[0]) && leaf[350..].windows(2).all(|w| w[1] <= w[0]), "it waves between knots");
        // The thorn: a flared root falling all the way to the tip.
        let thorn: Vec<f64> = (0..=1000).map(|k| law_at(&THORN_LAW, k as f64 / 1000.0)).collect();
        assert_eq!((thorn[0], thorn[250], thorn[1000]), (1.4, 1.0, 0.28));
        assert!(thorn.windows(2).all(|w| w[1] <= w[0]));
        // Two knots are a straight run, one a constant, and either end holds level past its knot.
        assert!((law_at(&[[0.0, 1.0], [1.0, 0.5]], 0.3) - 0.85).abs() < 1e-15);
        assert_eq!(law_at(&[[0.5, 2.0]], 0.1), 2.0);
        assert_eq!((law_at(&[[0.2, 1.0], [0.8, 3.0]], 0.0), law_at(&[[0.2, 1.0], [0.8, 3.0]], 1.0)), (1.0, 3.0));
        for law in [LEAF_LAW.as_slice(), THORN_LAW.as_slice()] {
            check_law(law).unwrap();
        }
        // Refused by name: no knots, shares that fall or run past one, a scale of nothing.
        for (law, why) in [
            (vec![], "takes 1 to 64 knots"),
            (vec![[0.5, 1.0], [0.4, 1.0]], "must rise"),
            (vec![[0.0, 1.0], [1.5, 1.0]], "shares run from 0 to 1"),
            (vec![[0.0, 0.0]], "a law scales between"),
        ] {
            let e = check_law(&law).unwrap_err().to_string();
            assert!(e.contains(why), "{e}");
        }
    }

    #[test]
    fn a_scale_law_shapes_the_sweep_and_its_volume_is_the_mean_of_its_square() {
        // Along a straight path every section is the rectangle scaled, so the volume is its area times the law's mean square.
        let mean_square = |law: &[[f64; 2]]| (0..20_000).map(|k| law_at(law, (k as f64 + 0.5) / 20_000.0).powi(2)).sum::<f64>() / 20_000.0;
        for (name, law) in [("leaf", LEAF_LAW), ("thorn", THORN_LAW)] {
            for (params, degrees) in [(PREVIEW, 0.0), (EXPORT, 0.0), (EXPORT, 90.0)] {
                let op = Operation::Twist { sketch: Sketch::rectangle(2.0, 1.5).into(), path: up(8.0).into(), degrees, end_scale: 1.0, scale: law.to_vec(), closed: false };
                let (m, mesh) = made(op, params).unwrap();
                closed(&m, &mesh, name);
                // Facets within the chord of a curve run a little inside it, so the volume falls short by about the chord's share.
                let (v, want) = (m.solid().volume(), 24.0 * mean_square(&law));
                let short = if params.theta_steps >= 512 { 0.01 } else { 0.03 };
                assert!((v / want - 1.0).abs() < short, "{name} {degrees}°: {v} against {want}");
                assert_eq!(m.kinds[..4], [SurfaceKind::Freeform; 4], "{name}: a side under a law is no plane");
                if degrees != 0.0 {
                    continue;
                }
                // Untwisted, the stations come from the law alone, and every corner rail stands within the chord of the
                // corner the law draws, so the leaf keeps its belly: (±1, ±0.75) scaled, 8 mm up.
                let chord = crate::cad::part_chord(params);
                let mut worst = 0.0_f64;
                for rail in &m.creases[..4] {
                    for w in rail.windows(2) {
                        for k in 1..8 {
                            let f = k as f64 / 8.0;
                            let at: P3 = std::array::from_fn(|i| w[0][i] + (w[1][i] - w[0][i]) * f);
                            let s = law_at(&law, at[2] / 8.0);
                            worst = worst.max((at[0] - at[0].signum() * s).hypot(at[1] - at[1].signum() * 0.75 * s));
                        }
                    }
                }
                assert!(worst <= chord * 1.001, "{name} at {chord} mm: a rail stands {worst:.4} mm off the law");
            }
        }
        // A two-knot law is the straight run to its end.
        let law = Operation::Twist { sketch: Sketch::rectangle(2.0, 1.5).into(), path: up(8.0).into(), degrees: 360.0, end_scale: 1.0, scale: vec![[0.0, 1.0], [1.0, 0.5]], closed: false };
        let (a, b) = (made(law, PREVIEW).unwrap().0, made(twist(Sketch::rectangle(2.0, 1.5), up(8.0), 360.0, 0.5), PREVIEW).unwrap().0);
        assert!((a.solid().volume() / b.solid().volume() - 1.0).abs() < 1e-9, "{} against {}", a.solid().volume(), b.solid().volume());
        // A law's own end scale is not read.
        let ignored = Operation::Twist { sketch: Sketch::rectangle(2.0, 1.5).into(), path: up(8.0).into(), degrees: 360.0, end_scale: 3.0, scale: vec![[0.0, 1.0], [1.0, 0.5]], closed: false };
        assert_eq!(made(ignored, PREVIEW).unwrap().0.solid().volume(), a.solid().volume());
    }

    #[test]
    fn a_path_through_points_carries_the_section_and_a_planar_one_matches_the_sketch() {
        // The section drawn at the origin rides a path wherever it starts, its x axis kept: 2 wide in x, 1.5 in y.
        let (m, mesh) = made(through(Sketch::rectangle(2.0, 1.5), &[[5.0, 2.0, 1.0], [5.0, 2.0, 9.0]], false, 0.0, &[], false), PREVIEW).unwrap();
        closed(&m, &mesh, "carried");
        let (lo, hi) = m.solid().v.iter().fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), p| (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k]))));
        for (got, want) in lo.iter().chain(&hi).zip([4.0, 1.25, 1.0, 6.0, 2.75, 9.0]) {
            assert!((got - want).abs() < 1e-12, "{lo:?} {hi:?}");
        }
        assert_eq!(m.kinds, [SurfaceKind::Plane; 6], "straight and untwisted through two points, a prism's sides are planes");
        // Turned 720° up the same line it keeps its 24 mm³.
        let (m, _) = made(through(Sketch::rectangle(2.0, 1.5), &[[0.0; 3], [0.0, 0.0, 8.0]], true, 720.0, &[], false), PREVIEW).unwrap();
        assert!((m.solid().volume() / 24.0 - 1.0).abs() < 0.005, "{}", m.solid().volume());
        // The sketch's L up the finger and along it, as points: the frame that turns least keeps the plane's normal, so the two agree.
        let mut l = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let p = [[0.0, 0.0], [0.0, 6.0], [5.0, 6.0]].map(|p| l.point(p));
        l.entity(Geometry::Polyline { points: p.to_vec(), closed: false });
        for degrees in [0.0, 180.0] {
            let (drawn, _) = made(twist(Sketch::rectangle(1.2, 0.8), l.clone(), degrees, 1.0), PREVIEW).unwrap();
            let (pointed, mesh) = made(through(Sketch::rectangle(1.2, 0.8), &[[0.0; 3], [0.0, 0.0, 6.0], [5.0, 0.0, 6.0]], false, degrees, &[], false), PREVIEW).unwrap();
            closed(&pointed, &mesh, "the L as points");
            assert_eq!(drawn.solid().f, pointed.solid().f, "{degrees}°");
            let worst = drawn.solid().v.iter().zip(&pointed.solid().v).map(|(a, b)| norm(sub(*a, *b))).fold(0.0, f64::max);
            assert!(worst < 1e-9, "{degrees}°: {worst} mm apart");
        }
        // Out of the plane: a corner up, along and across, mitred; an untwisted mitred run is its area times its length.
        let bent = [[0.0; 3], [0.0, 0.0, 6.0], [3.0, 4.0, 6.0], [3.0, 4.0, 10.0]];
        let (m, mesh) = made(through(Sketch::rectangle(1.2, 0.8), &bent, false, 0.0, &[], false), PREVIEW).unwrap();
        closed(&m, &mesh, "corners in space");
        assert!((m.solid().volume() / (0.96 * 15.0) - 1.0).abs() < 1e-9, "{}", m.solid().volume());
        let (m, mesh) = made(through(Sketch::rectangle(1.2, 0.8), &bent, false, 270.0, &[], false), EXPORT).unwrap();
        closed(&m, &mesh, "corners in space, twisted");
        // A smooth helix, two turns of 3 mm radius rising 2 mm a turn: a round tube is its area times its length.
        let helix: Vec<P3> = (0..=128)
            .map(|k| {
                let t = TAU * k as f64 / 64.0;
                [3.0 * t.cos(), 3.0 * t.sin(), 2.0 * t / TAU]
            })
            .collect();
        let length = 2.0 * (TAU * 3.0).hypot(2.0);
        for (params, degrees) in [(PREVIEW, 0.0), (EXPORT, 720.0)] {
            let (m, mesh) = made(through(Sketch::circle(0.5), &helix, true, degrees, &[], false), params).unwrap();
            closed(&m, &mesh, "helix");
            let want = PI * 0.25 * length;
            assert!((m.solid().volume() / want - 1.0).abs() < 0.01, "{degrees}°: {} against {want}", m.solid().volume());
        }
        // Refused by name: one point, a step that stays put, a path past the most points, a corner it cannot turn.
        for (points, why) in [
            (vec![[0.0; 3]], "runs through 2 to 256 points"),
            (vec![[0.0; 3], [0.0; 3], [0.0, 0.0, 4.0]], "stays put"),
            ((0..300).map(|k| [0.0, 0.0, k as f64]).collect(), "runs through 2 to 256 points"),
            (vec![[0.0; 3], [0.0, 0.0, 4.0], [0.0, 0.0, 1.0]], "doubles back"),
        ] {
            let why_not = made(through(Sketch::rectangle(1.0, 1.0), &points, false, 0.0, &[], false), PREVIEW).unwrap_err();
            assert!(why_not.contains(why), "{why_not}");
        }
    }

    #[test]
    fn a_closed_twisted_sweep_joins_round_with_no_caps_and_is_watertight_and_uncrossed() {
        // A sketch circle of 10 mm round the finger, the rectangle standing square to it at its start and turned once round.
        let mut hoop = Sketch::default();
        let (c, r) = (hoop.point([0.0, 0.0]), hoop.point([10.0, 0.0]));
        hoop.entity(Geometry::Circle { center: c, rim: r });
        let mut section = Sketch::rectangle(1.2, 0.8);
        section.plane = Workplane { origin: [10.0, 0.0, 0.0], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
        let ring = |section: Sketch, degrees: f64, scale: Vec<[f64; 2]>| Operation::Twist { sketch: section.into(), path: hoop.clone().into(), degrees, end_scale: 1.0, scale, closed: true };
        for (params, degrees) in [(PREVIEW, 0.0), (PREVIEW, 360.0), (EXPORT, -720.0)] {
            let (m, mesh) = made(ring(section.clone(), degrees, vec![]), params).unwrap();
            closed(&m, &mesh, &format!("hoop {degrees}°"));
            assert_eq!(m.named.names, ["Side 1", "Side 2", "Side 3", "Side 4"], "no caps");
            // Pappus: the rectangle's area times the round its centre runs.
            let want = 0.96 * TAU * 10.0;
            assert!((m.solid().volume() / want - 1.0).abs() < 0.005, "{degrees}°: {} against {want}", m.solid().volume());
            assert!(m.creases.iter().all(|c| c.first() == c.last()), "every rail runs round to its start");
        }
        // A closed polyline sketch, its first corner where the section stands: mitred all round.
        let mut square = Sketch::default();
        let ids = [[10.0, -10.0], [10.0, 10.0], [-10.0, 10.0], [-10.0, -10.0]].map(|p| square.point(p));
        square.entity(Geometry::Polyline { points: ids.to_vec(), closed: true });
        let mut start = Sketch::rectangle(1.0, 1.0);
        start.plane = Workplane { origin: [10.0, -10.0, 0.0], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
        let op = Operation::Twist { sketch: start.into(), path: square.into(), degrees: 0.0, end_scale: 1.0, scale: vec![], closed: true };
        let (m, mesh) = made(op, PREVIEW).unwrap();
        closed(&m, &mesh, "square loop");
        assert!((m.solid().volume() / 80.0 - 1.0).abs() < 1e-9, "a mitred loop is its area times its length: {}", m.solid().volume());

        // Sentis's cane: one closed sweep of a star through 128 points, tapered to 0.85 between the crossings and turned once round.
        let mut law: Vec<[f64; 2]> = (0..=12).map(|k| [k as f64 / 12.0, if k % 2 == 0 { 1.0 } else { 0.85 }]).collect();
        for (params, degrees) in [(PREVIEW, 0.0), (EXPORT, 360.0)] {
            let started = std::time::Instant::now();
            let (m, mesh) = made(through(star(0.72, 0.45), &cane(128), true, degrees, &law, true), params).unwrap();
            closed(&m, &mesh, "cane");
            assert_eq!(m.named.names.len(), 10, "ten sides and no caps");
            eprintln!("star cane at {} steps, {degrees}°: {} triangles, {:.3} mm³, {:.1} ms", params.theta_steps, m.solid().f.len(), m.solid().volume(), started.elapsed().as_secs_f64() * 1e3);
        }
        // The same cane straight-jointed through its points.
        let (m, mesh) = made(through(Sketch::circle(0.72), &cane(128), false, 0.0, &[], true), PREVIEW).unwrap();
        closed(&m, &mesh, "jointed cane");

        // Refused by name: an open path, part of a turn, a scale that does not come back, a section off the start, two points.
        let mut line = Sketch { plane: Workplane::section(), ..Sketch::default() };
        let (a, b) = (line.point([0.0, 0.0]), line.point([0.0, 5.0]));
        line.entity(Geometry::Line { a, b });
        let open = Operation::Twist { sketch: Sketch::rectangle(1.0, 1.0).into(), path: line.into(), degrees: 0.0, end_scale: 1.0, scale: vec![], closed: true };
        // Square to the loop at its top, not where its first curve starts.
        let mut far = section.clone();
        far.plane = Workplane { origin: [0.0, 10.0, 0.0], x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
        law.pop();
        for (op, why) in [
            (open, "the path does not close"),
            (ring(section.clone(), 90.0, vec![]), "whole turns"),
            (ring(section.clone(), 0.0, vec![[0.0, 1.0], [1.0, 0.5]]), "ends where it starts"),
            (through(star(0.72, 0.45), &cane(128), true, 0.0, &law, true), "ends where it starts"),
            (ring(far, 0.0, vec![]), "the start of the loop's first curve"),
            (through(Sketch::circle(0.5), &[[0.0; 3], [0.0, 0.0, 5.0]], true, 0.0, &[], true), "three points at least"),
        ] {
            let why_not = made(op, PREVIEW).unwrap_err();
            assert!(why_not.contains(why), "{why_not}");
        }
    }

    #[test]
    fn a_closed_loop_in_space_gives_back_the_turn_its_frame_gathers() {
        // A loop that leaves its plane unevenly, so a frame carried round it comes back turned.
        let p: Vec<P3> = (0..96)
            .map(|k| {
                let t = TAU * k as f64 / 96.0;
                [6.0 * t.cos(), 6.0 * t.sin(), 2.5 * (2.0 * t).sin() + 1.5 * t.cos()]
            })
            .collect();
        let need = |_: f64, _: f64| 0;
        let mut st = spline_stations(&p, true, 0.0, 0.5, crate::cad::PREVIEW_CHORD_MM, &need).unwrap();
        let plane = Workplane::default().plane().unwrap();
        let gathered = carry(&mut st, &plane, true).unwrap();
        assert!(gathered.abs() > 2f64.to_radians(), "the loop gathers {:.3}°; pick one that turns the frame", gathered.to_degrees());
        // With the turn given back, the band that closes the loop is like every other: no section spun round in one step.
        let (m, mesh) = made(through(Sketch::rectangle(1.0, 0.6), &p, true, 0.0, &[], true), PREVIEW).unwrap();
        closed(&m, &mesh, "uneven loop");
        let n = m.creases[0].len() - 1;
        let v = &m.solid().v;
        let ring = v.len() / n;
        let widest = |i: usize| (0..ring).map(|j| norm(sub(v[((i + 1) % n) * ring + j], v[i * ring + j]))).fold(0.0, f64::max);
        let others = (0..n - 1).map(widest).fold(0.0, f64::max);
        assert!(widest(n - 1) <= 1.3 * others, "the closing band runs {:.3} mm against {others:.3}", widest(n - 1));
    }

    #[test]
    fn the_vepres_laws_make_a_leaf_and_a_thorn_that_close() {
        // Hedera's edge leaf: a lens curling over a band edge, narrow at the stalk, widest a third along, a point at its tip.
        let curl: Vec<P3> = (0..=24)
            .map(|k| {
                let t = k as f64 / 24.0;
                let a = 1.4 * t;
                [4.0 * t, 1.6 * (1.0 - a.cos()), 0.96 * a.sin()]
            })
            .collect();
        let (m, mesh) = made(through(lens(1.6, 0.5), &curl, true, 40.0, &LEAF_LAW, false), EXPORT).unwrap();
        closed(&m, &mesh, "leaf");
        // Rubus's prickle: a round rising, then hooking, flared at its root and blunt at its tip.
        let hook: Vec<P3> = std::iter::once([0.0; 3])
            .chain((0..=12).map(|k| {
                let a = 70f64.to_radians() * k as f64 / 12.0;
                [1.6 * (1.0 - a.cos()), 0.0, 0.5 + 1.6 * a.sin()]
            }))
            .collect();
        let (m, mesh) = made(through(Sketch::circle(0.55), &hook, true, 0.0, &THORN_LAW, false), EXPORT).unwrap();
        closed(&m, &mesh, "thorn");
    }

    #[test]
    fn old_twists_read_and_write_as_they_always_did_and_new_ones_round_trip() {
        let old = twist(Sketch::rectangle(2.0, 1.5), up(8.0), 180.0, 1.0);
        let json = serde_json::json!({ "Twist": { "sketch": Sketch::rectangle(2.0, 1.5), "path": up(8.0), "degrees": 180.0, "end_scale": 1.0 } });
        assert_eq!(serde_json::to_value(&old).unwrap(), json, "written as before");
        let Operation::Twist { path: TwistPath::Sketch(path), scale, closed, .. } = serde_json::from_value::<Operation>(json).unwrap() else { panic!("not a sketch path") };
        assert!(path == up(8.0) && scale.is_empty() && !closed, "read as before");
        // A sketch with no points at all is still a sketch.
        let empty = serde_json::json!({ "Twist": { "sketch": Sketch::circle(1.0), "path": Sketch::default(), "degrees": 0.0, "end_scale": 1.0 } });
        assert!(matches!(serde_json::from_value::<Operation>(empty).unwrap(), Operation::Twist { path: TwistPath::Sketch(_), .. }));
        let new = through(star(0.72, 0.45), &cane(16), true, 360.0, &[[0.0, 1.0], [0.5, 0.85], [1.0, 1.0]], true);
        let text = serde_json::to_string(&new).unwrap();
        assert!(text.contains(r#""smooth":true"#) && text.contains(r#""closed":true"#) && text.contains(r#""scale":[[0.0,1.0],[0.5,0.85],[1.0,1.0]]"#), "{text}");
        let back: Operation = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), text, "round trip");
    }
}
