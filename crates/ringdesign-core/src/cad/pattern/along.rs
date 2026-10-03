//! Arrays along a path: where each copy of an array along the band's crest, a path drawn in the chart, a sweep's or a twisted
//! sweep's centreline, curves of a sketch or a polyline in the world stands.
//!
//! The source stands at the path's first station and is never moved. Each copy is the source carried from that station's
//! frame to its own: turned about the station's normal, scaled about its origin, and laid square to the parting plane when
//! asked. A station's frame has `y` along the path, `z` out of the band (or the sketch's plane) and `x = y × z`, which on the
//! crest running round the ring is the frame a ring placement seats a part by.
use super::{MAX_PATTERN_COUNT, Motion, cross, dot, inverse, on_plane, then, unit};
use crate::RingDesign;
use crate::cad::{Document, Operation, Placement, SweepPath, TwistPath};
use crate::setting::BareSurface;
use crate::sketch::{Id, Sketch, Workplane};
use anyhow::{Context, Result, anyhow, bail, ensure};
use cadkernel::geom2d::Curve;
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;

type P3 = [f64; 3];

/// Most points a polyline or chart path holds.
pub const MAX_PATH_POINTS: usize = 4096;
/// Most curves a sketch path chains.
pub const MAX_PATH_CURVES: usize = 512;
/// Shortest step between copies, mm.
pub const MIN_PITCH_MM: f64 = 0.01;
/// Longest step between copies, mm.
pub const MAX_PITCH_MM: f64 = 1000.0;
/// Smallest and largest scale a copy takes.
pub const SCALE_RANGE: [f64; 2] = [0.05, 20.0];
/// Furthest a crest path is walked between samples, degrees.
const CREST_STEP_DEG: f64 = 1.0;
/// Furthest a chart path is walked between samples, mm.
const CHART_STEP_MM: f64 = 0.1;
/// Furthest a sketch curve is walked between samples, mm.
const CURVE_STEP_MM: f64 = 0.02;
/// Steps a smooth twisted sweep's path is walked between two of its points.
const SPLINE_STEPS: usize = 32;
/// Most samples a walked sketch path takes.
const MAX_WALK: usize = 50_000;

/// What an array along a path follows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlongPath {
    /// The band's crest line from `from_deg` to `to_deg` round the ring: where its outer surface crosses the parting plane,
    /// solved at every station as a stamp row's parting line is. On a procedural band the crest sits there by construction; on
    /// a stock it is the stock's parting line. A whole turn closes on itself.
    Crest { from_deg: f64, to_deg: f64 },
    /// Chart points `[θ°, v mm]`, joined straight in the chart and laid on the bare band.
    Chart(Vec<[f64; 2]>),
    /// The path a sweep runs along, or the centreline a twisted sweep follows, as that feature is seated.
    Feature(Id),
    /// Curves of a sketch feature chained end to end in the order named, each walked the way that continues the one before;
    /// none named chains every drawn curve. The sketch lies on its own plane or on a work plane.
    Sketch {
        feature: Id,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<Id>,
    },
    /// A polyline in world millimetres: what the graph's path nodes make. Ends that meet close it.
    Points(Vec<[f64; 3]>),
}

impl Default for AlongPath {
    fn default() -> Self {
        Self::Crest { from_deg: 0.0, to_deg: 360.0 }
    }
}

impl AlongPath {
    /// The features the path is read from.
    pub fn reads(&self) -> Vec<Id> {
        match self {
            Self::Feature(id) | Self::Sketch { feature: id, .. } => vec![*id],
            _ => Vec::new(),
        }
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn is_zero_count(v: &u32) -> bool {
    *v == 0
}
fn unit_scale() -> [f64; 2] {
    [1.0, 1.0]
}
fn is_unit_scale(s: &[f64; 2]) -> bool {
    *s == [1.0, 1.0]
}

/// An array along a path: `count` instances, the source's own first, spread over the path or stepped `pitch_mm` apart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Along {
    pub path: AlongPath,
    /// Instances in all, the source's own among them; 0 takes as many as `pitch_mm` fits on the path.
    #[serde(default, skip_serializing_if = "is_zero_count")]
    pub count: u32,
    /// Path length between neighbouring copies, mm; `None` spreads `count` over the path's whole length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_mm: Option<f64>,
    /// Every station moved this share of one step along the path, 0 to under 1. With no pitch an open path is inset by it at
    /// both ends, so 0.5 centres the copies in equal cells.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub phase: f64,
    /// Turn about the station's normal added to every other copy, degrees: 180 stands alternate copies on the path's other side.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub alternate_deg: f64,
    /// Turn about the station's normal added per copy, degrees: 137.5 is phyllotaxis.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub roll_deg: f64,
    /// Scale at the path's start and at its end, read at each station's share of the length; a copy is the source scaled by its
    /// station's scale over the first station's.
    #[serde(default = "unit_scale", skip_serializing_if = "is_unit_scale")]
    pub scale: [f64; 2],
    /// Stand every station square to the parting plane, its normal and its direction laid flat: a part lying in a plane parallel
    /// to the parting plane stays in one, as a hook in the parting plane must.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub level: bool,
}

impl Default for Along {
    fn default() -> Self {
        Self { path: AlongPath::default(), count: 0, pitch_mm: None, phase: 0.0, alternate_deg: 0.0, roll_deg: 0.0, scale: unit_scale(), level: false }
    }
}

impl Along {
    /// Refused by name: a count, a pitch, a phase, turns or scales past what an array along a path takes.
    pub fn check(&self) -> Result<()> {
        ensure!(
            self.count == 0 || (2..=MAX_PATTERN_COUNT).contains(&self.count),
            "A pattern holds 2 to {MAX_PATTERN_COUNT} instances, its source among them, not {}",
            self.count
        );
        match self.pitch_mm {
            Some(p) => ensure!(
                p.is_finite() && (MIN_PITCH_MM..=MAX_PITCH_MM).contains(&p),
                "An array along a path steps {MIN_PITCH_MM} to {MAX_PITCH_MM} mm between copies, not {p}"
            ),
            None => ensure!(self.count > 0, "An array along a path takes a count, a pitch, or both"),
        }
        ensure!(self.phase.is_finite() && (0.0..1.0).contains(&self.phase), "A phase is a share of one step, 0 or more and under 1, not {}", self.phase);
        ensure!(
            self.alternate_deg.is_finite() && self.roll_deg.is_finite() && self.alternate_deg.abs() <= 3600.0 && self.roll_deg.abs() <= 3600.0,
            "An array along a path turns its copies by finite angles of at most ten turns"
        );
        let [lo, hi] = SCALE_RANGE;
        ensure!(self.scale.iter().all(|s| s.is_finite() && (lo..=hi).contains(s)), "An array along a path scales its copies by {lo} to {hi}, not {:?}", self.scale);
        Ok(())
    }

    /// Whether the copies change size along the path.
    pub fn scales(&self) -> bool {
        self.scale[0] != self.scale[1]
    }
}

/// One station of an array along a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// `y` along the path, `z` out of the band or the sketch's plane, `x = y × z`; unturned and unscaled.
    pub frame: Motion,
    /// Its distance along the path from the path's start, mm.
    pub along_mm: f64,
    /// That distance's share of the path's length.
    pub share: f64,
    /// The scale the path gives here.
    pub scale: f64,
}

/// What a path is read against: the design and its document, where a placement seats a part, and the frame a work plane or a
/// part stands in.
pub struct Env<'a> {
    pub design: &'a RingDesign,
    pub seated: &'a dyn Fn(&Placement) -> Result<Motion>,
    pub frame_of: &'a dyn Fn(Id) -> Option<Motion>,
}

/// A path walked into samples: points, the direction out of the band or the plane at each, and for a path on the band its
/// chart point, read again exactly at every station. A closed walk repeats its first sample last.
struct Walk {
    points: Vec<P3>,
    ups: Vec<P3>,
    /// Each sample's direction where the path knows it exactly: a sketch curve's own; otherwise read off the samples either side.
    tangents: Option<Vec<P3>>,
    chart: Option<Vec<[f64; 2]>>,
    /// The chart's `v` is solved onto the parting plane at every station.
    crest: bool,
    closed: bool,
}

/// The bare band, made once on first ask.
struct Band<'a> {
    design: &'a RingDesign,
    ctx: &'a OnceCell<crate::FieldContext>,
    bare: OnceCell<BareSurface<'a>>,
}

impl<'a> Band<'a> {
    fn get(&self) -> &BareSurface<'a> {
        self.bare.get_or_init(|| BareSurface::new(self.design, self.ctx.get_or_init(|| self.design.field_context())))
    }
    fn ctx(&self) -> &crate::FieldContext {
        self.ctx.get_or_init(|| self.design.field_context())
    }
    /// Point and outward normal of the bare band at a chart point.
    fn at(&self, theta_deg: f64, v_mm: f64) -> (P3, P3) {
        let (p, n, _, _) = self.get().point(theta_deg, v_mm);
        (p, n)
    }
    /// The direction out of the band nearest a point in the world: the bare section's normal, or out from the finger's axis
    /// where the parts are the whole ring.
    fn up_near(&self, p: P3) -> P3 {
        let radial = unit([p[0], p[1], 0.0]).unwrap_or([0.0, 0.0, 1.0]);
        if !self.design.band_is_procedural() {
            return radial;
        }
        let theta = p[1].atan2(p[0]).to_degrees();
        self.get().normal_near(theta, p[0].hypot(p[1]), p[2]).unwrap_or(radial)
    }
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn lerp(a: P3, b: P3, f: f64) -> P3 {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f)
}
fn gap(a: P3, b: P3) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// The frame a station stands in: `y` along `tangent`, `z` the part of `up` square to it, `x = y × z`; laid flat when `level`.
fn station_frame(origin: P3, tangent: P3, up: P3, level: bool) -> Result<Motion> {
    let flat = |v: P3| [v[0], v[1], 0.0];
    let (t, u) = if level { (flat(tangent), flat(up)) } else { (tangent, up) };
    let y = unit(t).ok_or_else(|| match level {
        true => anyhow!("a levelled array needs a path that runs round the ring, and this one runs along the finger here"),
        false => anyhow!("the path stops dead here"),
    })?;
    let square = |u: P3| unit(sub(u, y.map(|c| c * dot(u, y))));
    // A path running straight out of the band takes the finger's axis for its normal.
    let z = square(u).or_else(|| square([0.0, 0.0, 1.0])).or_else(|| square([1.0, 0.0, 0.0])).context("the path has no direction out of it here")?;
    Ok(Motion { x_axis: cross(y, z), y_axis: y, z_axis: z, origin })
}

/// `f` turned `deg` about its own `z` and scaled `by` about its origin.
fn turned_scaled(f: &Motion, deg: f64, by: f64) -> Motion {
    let (s, c) = deg.to_radians().sin_cos();
    let mix = |a: P3, b: P3, ka: f64, kb: f64| -> P3 { std::array::from_fn(|k| by * (ka * a[k] + kb * b[k])) };
    Motion { x_axis: mix(f.x_axis, f.y_axis, c, s), y_axis: mix(f.x_axis, f.y_axis, -s, c), z_axis: f.z_axis.map(|v| by * v), origin: f.origin }
}

/// Curves walked end to end, each with whether it runs its own way, and whether the chain closes. Named curves (`ordered`)
/// are walked in the order given; otherwise the chain is found from where the curves meet, as a twisted sweep's path is.
fn chain(curves: Vec<Curve>, ordered: bool) -> Result<(Vec<(Curve, bool)>, bool)> {
    ensure!(!curves.is_empty(), "the path has no curves");
    ensure!(curves.len() <= MAX_PATH_CURVES, "the path chains {} curves; the most is {MAX_PATH_CURVES}", curves.len());
    let ends: Vec<[[f64; 2]; 2]> = curves.iter().map(|c| [c.point_at(0.0), c.point_at(1.0)]).collect();
    let scale = ends.iter().flatten().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) <= 1e-6 * scale;
    if curves.len() == 1 {
        let closed = curves[0].is_closed() || near(ends[0][0], ends[0][1]);
        return Ok((vec![(curves[0].clone(), true)], closed));
    }
    if let Some(c) = curves.iter().find(|c| c.is_closed()) {
        bail!("the path chains a closed curve with others; a closed curve is a path of its own ({:?})", c.point_at(0.0));
    }
    let head = |(i, forward): (usize, bool)| ends[i][usize::from(forward)];
    let tail = |(i, forward): (usize, bool)| ends[i][usize::from(!forward)];
    let mut walk: Vec<(usize, bool)> = Vec::with_capacity(curves.len());
    if ordered {
        // The first runs whichever way meets the second.
        let first = if near(ends[0][1], ends[1][0]) || near(ends[0][1], ends[1][1]) {
            true
        } else if near(ends[0][0], ends[1][0]) || near(ends[0][0], ends[1][1]) {
            false
        } else {
            bail!("the path's first two curves do not meet; name them in the order they run, end to end");
        };
        walk.push((0, first));
        for i in 1..curves.len() {
            let at = head(*walk.last().expect("walked"));
            let forward = if near(ends[i][0], at) {
                true
            } else if near(ends[i][1], at) {
                false
            } else {
                bail!("curve {} of the path does not meet the one before it at ({:.3}, {:.3}); name them in the order they run", i + 1, at[0], at[1]);
            };
            walk.push((i, forward));
        }
    } else {
        let meeting = |p: [f64; 2]| ends.iter().flatten().filter(|q| near(p, **q)).count();
        for e in ends.iter().flatten() {
            ensure!(meeting(*e) <= 2, "the path branches at ({:.3}, {:.3}); it follows one unbroken path", e[0], e[1]);
        }
        let loose: Vec<(usize, usize)> = (0..curves.len()).flat_map(|i| [(i, 0), (i, 1)]).filter(|&(i, k)| meeting(ends[i][k]) == 1).collect();
        ensure!(loose.is_empty() || loose.len() == 2, "the path is in {} pieces; it follows one unbroken path", loose.len() / 2);
        let (first, side) = loose.first().copied().unwrap_or((0, 0));
        let mut used = vec![false; curves.len()];
        used[first] = true;
        walk.push((first, side == 0));
        while walk.len() < curves.len() {
            let at = head(*walk.last().expect("walked"));
            let next = (0..curves.len()).filter(|i| !used[*i]).find_map(|i| {
                if near(ends[i][0], at) {
                    Some((i, true))
                } else if near(ends[i][1], at) {
                    Some((i, false))
                } else {
                    None
                }
            });
            let Some((i, forward)) = next else { bail!("the path breaks off at ({:.3}, {:.3}); join it into one", at[0], at[1]) };
            used[i] = true;
            walk.push((i, forward));
        }
    }
    let closed = near(head(*walk.last().expect("walked")), tail(walk[0]));
    Ok((walk.into_iter().map(|(i, f)| (curves[i].clone(), f)).collect(), closed))
}

/// Chained curves on `plane` walked into world points no further apart than [`CURVE_STEP_MM`] and each curve's own direction of
/// travel there, a closed chain's first sample repeated last.
fn walk_curves(plane: &cadkernel::space::Plane, chained: &[(Curve, bool)], closed: bool) -> Result<(Vec<P3>, Vec<P3>)> {
    let lengths: Vec<f64> = chained.iter().map(|(c, _)| plane_length(plane, c)).collect();
    let total: f64 = lengths.iter().sum();
    ensure!(total.is_finite() && total > 1e-9, "the path has no length");
    let step = CURVE_STEP_MM.max(total / MAX_WALK as f64);
    let (mut out, mut along): (Vec<P3>, Vec<P3>) = (Vec::new(), Vec::new());
    for ((c, forward), len) in chained.iter().zip(&lengths) {
        let n = ((len / step).ceil() as usize).clamp(8, MAX_WALK);
        let start = usize::from(!out.is_empty());
        for j in start..=n {
            let t = j as f64 / n as f64;
            let t = if *forward { t } else { 1.0 - t };
            let d = plane.vector_at(c.tangent_at(t));
            out.push(plane.point_at(c.point_at(t)));
            along.push(unit(if *forward { d } else { d.map(|v| -v) }).unwrap_or([0.0; 3]));
        }
    }
    if closed {
        let (first, ahead) = (out[0], along[0]);
        match out.last_mut() {
            Some(last) if gap(*last, first) < 1e-6 => *last = first,
            _ => {
                out.push(first);
                along.push(ahead);
            }
        }
    }
    Ok((out, along))
}

/// A curve's length in the world, walked on `plane`.
fn plane_length(plane: &cadkernel::space::Plane, c: &Curve) -> f64 {
    let n = 256;
    (0..n).map(|k| gap(plane.point_at(c.point_at(k as f64 / n as f64)), plane.point_at(c.point_at((k + 1) as f64 / n as f64)))).sum()
}

/// The world plane `sketch` lies on: its own, or the work plane it is laid on.
fn sketch_plane(sketch: &Sketch, doc: &Document, env: &Env, who: &str) -> Result<cadkernel::space::Plane> {
    let Some(anchor) = &sketch.plane.on_face else { return sketch.plane.plane() };
    let on_plane_feature = doc.feature(anchor.feature).is_some_and(|f| matches!(f.operation, Operation::Plane { .. }));
    ensure!(on_plane_feature, "{who} lies on a face of #{}; an array follows a sketch on its own plane or on a work plane", anchor.feature);
    let frame = (env.frame_of)(anchor.feature).ok_or_else(|| anyhow!("Work plane #{} {who} lies on is unavailable or suppressed", anchor.feature))?;
    on_plane(sketch, &frame)
}

/// `points` closed when their ends meet: the last moved onto the first.
fn closing(mut points: Vec<P3>) -> (Vec<P3>, bool) {
    let n = points.len();
    let closed = n > 2 && gap(points[0], points[n - 1]) < 1e-6;
    if closed {
        points[n - 1] = points[0];
    }
    (points, closed)
}

/// The path walked into samples.
fn walk(path: &AlongPath, env: &Env, band: &Band) -> Result<Walk> {
    let design = env.design;
    let doc = || design.cad.as_ref().context("No CAD features");
    match path {
        AlongPath::Crest { from_deg, to_deg } => {
            ensure!(from_deg.is_finite() && to_deg.is_finite(), "A crest path runs between two finite angles");
            let span = to_deg - from_deg;
            ensure!(span.abs() > 1e-6 && span.abs() <= 360.0 + 1e-9, "A crest path runs more than 0° and at most 360° either way");
            ensure!(design.band_is_procedural(), "A crest path runs along the band, and this ring's parts are the whole ring");
            let closed = span.abs() >= 360.0 - 1e-9;
            let n = ((span.abs() / CREST_STEP_DEG).ceil() as usize).max(4);
            let mut guess = band.ctx().crest_v_mm;
            let (mut points, mut ups, mut chart) = (Vec::with_capacity(n + 1), Vec::with_capacity(n + 1), Vec::with_capacity(n + 1));
            for i in 0..=n {
                let theta = from_deg + span * i as f64 / n as f64;
                let v = band.get().parting_v(theta, guess).ok_or_else(|| anyhow!("The band has no crest line on the parting plane at {theta:.1}°"))?;
                guess = v;
                let (p, up) = band.at(theta, v);
                points.push(p);
                ups.push(up);
                chart.push([theta, v]);
            }
            if closed {
                // The last sample is the first a turn on; it stands where the first does.
                points[n] = points[0];
                ups[n] = ups[0];
            }
            Ok(Walk { points, ups, tangents: None, chart: Some(chart), crest: true, closed })
        }
        AlongPath::Chart(points) => {
            ensure!((2..=MAX_PATH_POINTS).contains(&points.len()), "A chart path holds 2 to {MAX_PATH_POINTS} points, not {}", points.len());
            ensure!(points.iter().flatten().all(|v| v.is_finite()), "A chart path's points are finite");
            ensure!(design.band_is_procedural(), "A chart path lies on the band, and this ring's parts are the whole ring");
            let (span, r) = (band.ctx().band_v_len_mm, band.ctx().crest_radius_mm);
            for (k, p) in points.iter().enumerate() {
                ensure!((-1e-9..=span + 1e-9).contains(&p[1]), "Chart point {} stands off the band: v {:.3} mm, which runs 0 to {span:.3}", k + 1, p[1]);
            }
            let mut chart: Vec<[f64; 2]> = vec![points[0]];
            for w in points.windows(2) {
                let run = ((w[1][0] - w[0][0]).to_radians().abs() * r).max((w[1][1] - w[0][1]).abs());
                // A point repeated is no step at all.
                if run < 1e-12 {
                    continue;
                }
                let n = ((run / CHART_STEP_MM).ceil() as usize).clamp(1, MAX_WALK);
                ensure!(chart.len() + n <= MAX_WALK, "A chart path walks past {MAX_WALK} samples; draw it shorter");
                chart.extend((1..=n).map(|j| {
                    let f = j as f64 / n as f64;
                    [w[0][0] + (w[1][0] - w[0][0]) * f, w[0][1] + (w[1][1] - w[0][1]) * f]
                }));
            }
            let (points, ups): (Vec<P3>, Vec<P3>) = chart.iter().map(|c| band.at(c[0], c[1])).unzip();
            let (points, closed) = closing(points);
            let mut ups = ups;
            if closed {
                let n = ups.len();
                ups[n - 1] = ups[0];
            }
            Ok(Walk { points, ups, tangents: None, chart: Some(chart), crest: false, closed })
        }
        AlongPath::Points(points) => {
            ensure!((2..=MAX_PATH_POINTS).contains(&points.len()), "A polyline path holds 2 to {MAX_PATH_POINTS} points, not {}", points.len());
            ensure!(points.iter().flatten().all(|v| v.is_finite() && v.abs() <= 10_000.0), "A polyline path's points are finite millimetres");
            let mut kept: Vec<P3> = Vec::with_capacity(points.len());
            for p in points {
                if kept.last().is_none_or(|q| gap(*q, *p) > 1e-9) {
                    kept.push(*p);
                }
            }
            let (points, closed) = closing(kept);
            let ups = points.iter().map(|p| band.up_near(*p)).collect();
            Ok(Walk { points, ups, tangents: None, chart: None, crest: false, closed })
        }
        AlongPath::Feature(id) => {
            let doc = doc()?;
            let f = doc.feature(*id).ok_or_else(|| anyhow!("Feature #{id} the array follows is not in the document"))?;
            let who = format!("#{} {}", f.id, f.name);
            let seat = (env.seated)(&f.component.placement).with_context(|| format!("Where {who} stands"))?;
            // A polyline in the part's own frame, seated, closed when the feature closes it or its ends meet.
            let polyline = |stations: &[P3], closes: bool| -> Result<Walk> {
                ensure!(stations.len() >= 2, "{who} runs along fewer than two stations");
                let mut kept: Vec<P3> = Vec::with_capacity(stations.len() + 1);
                for p in stations.iter().map(|p| seat.point(*p)) {
                    if kept.last().is_none_or(|q| gap(*q, p) > 1e-9) {
                        kept.push(p);
                    }
                }
                if closes && kept.len() > 2 && gap(kept[0], kept[kept.len() - 1]) > 1e-9 {
                    kept.push(kept[0]);
                }
                let (points, closed) = closing(kept);
                let ups = points.iter().map(|p| band.up_near(*p)).collect();
                Ok(Walk { points, ups, tangents: None, chart: None, crest: false, closed })
            };
            match &f.operation {
                Operation::Sweep { path, closed, .. } => match path {
                    SweepPath::Points(points) => polyline(points, *closed),
                    SweepPath::Sketch { feature, entity, lift_mm } => {
                        let s = doc.feature(*feature).ok_or_else(|| anyhow!("Sketch #{feature} {who} sweeps along is not in the document"))?;
                        let Operation::Sketch { sketch } = &s.operation else { bail!("#{feature} {who} sweeps along is not a sketch") };
                        // The sketch laid where it lies, as the evaluation lays it before sampling the entity.
                        let plane = sketch_plane(sketch, doc, env, &who)?;
                        let mut laid = sketch.clone();
                        laid.plane = Workplane { origin: plane.origin, x: plane.x_axis, y: plane.y_axis, on_face: None };
                        let (points, round) = crate::cad::sweep_path_points(&laid, *entity, *lift_mm).with_context(|| format!("{who}'s path, entity #{entity} of sketch #{feature}"))?;
                        polyline(&points, *closed || round)
                    }
                },
                Operation::Twist { path: TwistPath::Points { points, smooth }, closed, .. } => match smooth {
                    false => polyline(points, *closed),
                    true => {
                        let run = crate::cad::twist::path_points(points, true, *closed, SPLINE_STEPS).with_context(|| format!("{who}'s path"))?;
                        let points: Vec<P3> = run.iter().map(|(p, _)| seat.point(*p)).collect();
                        let tangents = run.iter().map(|(_, t)| seat.vector(*t)).collect();
                        let ups = points.iter().map(|p| band.up_near(*p)).collect();
                        let (points, closed) = closing(points);
                        Ok(Walk { points, ups, tangents: Some(tangents), chart: None, crest: false, closed })
                    }
                },
                Operation::Twist { path: TwistPath::Sketch(path), closed: closes, .. } => {
                    let plane = sketch_plane(path, doc, env, &who)?;
                    let solved = path.solve().with_context(|| format!("{who}'s path"))?.sketch;
                    let (chained, closed) = chain(solved.curves().with_context(|| format!("{who}'s path"))?, false).with_context(|| format!("{who}: the path"))?;
                    ensure!(closed || !closes, "{who} closes round a path whose ends do not meet");
                    let normal = plane.normal().context("The path's plane has no normal")?;
                    let (points, along) = walk_curves(&plane, &chained, closed)?;
                    let points: Vec<P3> = points.into_iter().map(|p| seat.point(p)).collect();
                    let tangents = along.into_iter().map(|t| seat.vector(t)).collect();
                    let up = seat.vector(normal);
                    Ok(Walk { ups: vec![up; points.len()], points, tangents: Some(tangents), chart: None, crest: false, closed })
                }
                other => bail!("{who} is a {}; an array follows a sweep's or a twisted sweep's path", other.label().to_lowercase()),
            }
        }
        AlongPath::Sketch { feature, entities } => {
            let doc = doc()?;
            let f = doc.feature(*feature).ok_or_else(|| anyhow!("Sketch #{feature} the array follows is not in the document"))?;
            let who = format!("#{} {}", f.id, f.name);
            let Operation::Sketch { sketch } = &f.operation else { bail!("{who} is a {}; an array follows the curves of a sketch", f.operation.label().to_lowercase()) };
            let plane = sketch_plane(sketch, doc, env, &who)?;
            let solved = sketch.solve().with_context(|| who.clone())?.sketch;
            let curves: Vec<Curve> = if entities.is_empty() {
                solved.curves().with_context(|| who.clone())?
            } else {
                let mut out = Vec::new();
                for id in entities {
                    let e = solved.entities.iter().find(|e| e.id == *id).ok_or_else(|| anyhow!("{who} has no curve #{id}"))?;
                    out.extend(solved.curves_of(e).with_context(|| format!("{who}, curve #{id}"))?);
                }
                out
            };
            let (chained, closed) = chain(curves, !entities.is_empty()).with_context(|| format!("{who}: the path"))?;
            let normal = plane.normal().context("The sketch's plane has no normal")?;
            let (points, tangents) = walk_curves(&plane, &chained, closed)?;
            Ok(Walk { ups: vec![normal; points.len()], points, tangents: Some(tangents), chart: None, crest: false, closed })
        }
    }
}

/// Distances along a path `total` mm long at which `a` stands its stations, the source's first.
fn spacing(a: &Along, total: f64, closed: bool) -> Result<Vec<f64>> {
    let eps = 1e-9 * total.max(1.0);
    let at: Vec<f64> = match a.pitch_mm {
        None => {
            let n = f64::from(a.count);
            let step = if closed { total / n } else { total / (n - 1.0 + 2.0 * a.phase) };
            (0..a.count).map(|k| (a.phase + f64::from(k)) * step).collect()
        }
        Some(p) => {
            let fits = |k: u32| if closed { f64::from(k) * p < total - eps } else { (a.phase + f64::from(k)) * p <= total + eps };
            if a.count > 0 {
                let need = if closed { f64::from(a.count - 1) * p } else { (a.phase + f64::from(a.count - 1)) * p };
                ensure!(fits(a.count - 1), "{} copies {p} mm apart need {need:.2} mm of path; it runs {total:.2} mm", a.count);
                (0..a.count).map(|k| (a.phase + f64::from(k)) * p).collect()
            } else {
                let mut out = Vec::new();
                while fits(out.len() as u32) {
                    ensure!(
                        out.len() < MAX_PATTERN_COUNT as usize,
                        "{total:.2} mm of path holds more than {MAX_PATTERN_COUNT} copies {p} mm apart; take a longer pitch or a count"
                    );
                    out.push((a.phase + out.len() as f64) * p);
                }
                ensure!(out.len() >= 2, "{total:.2} mm of path holds fewer than two copies {p} mm apart; take a shorter pitch");
                out
            }
        }
    };
    Ok(at.into_iter().map(|s| if closed { s.rem_euclid(total) } else { s.clamp(0.0, total) }).collect())
}

/// Every station of `a`, the source's first.
pub fn stations(a: &Along, env: &Env) -> Result<Vec<Station>> {
    a.check()?;
    let ctx = OnceCell::new();
    let band = Band { design: env.design, ctx: &ctx, bare: OnceCell::new() };
    let w = walk(&a.path, env, &band)?;
    let n = w.points.len();
    ensure!(n >= 2, "The path has no length");
    let mut lengths = Vec::with_capacity(n);
    lengths.push(0.0);
    for k in 1..n {
        lengths.push(lengths[k - 1] + gap(w.points[k], w.points[k - 1]));
    }
    let total = lengths[n - 1];
    ensure!(total.is_finite() && total > 1e-6, "The path has no length");
    // Each sample's direction: the path's own where it knows it, else from its neighbours, a closed walk's ends across its seam.
    let tangents: Vec<P3> = match &w.tangents {
        Some(t) => t.clone(),
        None => (0..n)
            .map(|k| {
                let (before, after) = if w.closed && (k == 0 || k == n - 1) {
                    (w.points[n - 2], w.points[1])
                } else if k == 0 {
                    (w.points[0], w.points[1])
                } else if k == n - 1 {
                    (w.points[n - 2], w.points[n - 1])
                } else {
                    (w.points[k - 1], w.points[k + 1])
                };
                unit(sub(after, before)).unwrap_or([0.0; 3])
            })
            .collect(),
    };
    let at = spacing(a, total, w.closed)?;
    // A point on the band's crest, or on a chart path, at chart (θ, v).
    let on_band = |theta: f64, v: f64| -> Result<(P3, P3)> {
        let v = match w.crest {
            true => band.get().parting_v(theta, v).ok_or_else(|| anyhow!("The band has no crest line on the parting plane at {theta:.1}°"))?,
            false => v,
        };
        Ok(band.at(theta, v))
    };
    at.iter()
        .enumerate()
        .map(|(k, &s)| {
            let i = lengths.partition_point(|l| *l <= s).clamp(1, n - 1);
            let run = lengths[i] - lengths[i - 1];
            let f = if run > 0.0 { ((s - lengths[i - 1]) / run).clamp(0.0, 1.0) } else { 0.0 };
            let (origin, up, tangent) = match &w.chart {
                // A path on the band is read again exactly where the station stands, its direction a hair either side.
                Some(chart) => {
                    let (dt, dv) = (chart[i][0] - chart[i - 1][0], chart[i][1] - chart[i - 1][1]);
                    let (theta, v) = (chart[i - 1][0] + dt * f, chart[i - 1][1] + dv * f);
                    let (origin, up) = on_band(theta, v)?;
                    let h = 1e-3;
                    let ahead = on_band(theta + dt * h, v + dv * h)?.0;
                    let behind = on_band(theta - dt * h, v - dv * h)?.0;
                    (origin, up, sub(ahead, behind))
                }
                None => (lerp(w.points[i - 1], w.points[i], f), lerp(w.ups[i - 1], w.ups[i], f), lerp(tangents[i - 1], tangents[i], f)),
            };
            let frame = station_frame(origin, tangent, up, a.level).with_context(|| format!("Station {} of the array, {s:.2} mm along the path", k + 1))?;
            let share = s / total;
            Ok(Station { frame, along_mm: s, share, scale: a.scale[0] + (a.scale[1] - a.scale[0]) * share })
        })
        .collect()
}

/// The motions carrying the source from the first station to each other one, turned and scaled as `a` asks.
pub fn motions(a: &Along, env: &Env) -> Result<Vec<Motion>> {
    let st = stations(a, env)?;
    let first = st[0];
    let back = inverse(&first.frame);
    Ok(st
        .iter()
        .enumerate()
        .skip(1)
        .map(|(k, s)| {
            let turn = a.roll_deg * k as f64 + if k % 2 == 1 { a.alternate_deg } else { 0.0 };
            then(&turned_scaled(&s.frame, turn, s.scale / first.scale), &back)
        })
        .collect())
}

/// The `Operation::Transform` translation and rotation that stand a part modelled at the origin — `y` along the path, `z` out
/// of it — in `frame`: seat an array's source at its first station this way and every copy stands at its own.
pub fn transform_onto(frame: &Motion) -> ([f64; 3], [f64; 3]) {
    let m = nalgebra::Matrix3::from_columns(&[frame.x_axis.into(), frame.y_axis.into(), frame.z_axis.into()]);
    let (roll, pitch, yaw) = nalgebra::Rotation3::from_matrix_unchecked(m).euler_angles();
    (frame.origin, [roll.to_degrees(), pitch.to_degrees(), yaw.to_degrees()])
}

/// No part's frame: a path read with no build.
fn no_frame(_: Id) -> Option<Motion> {
    None
}

/// The bare band read with no build: what a placement seats a part on, and no part's frame.
fn bare_env<'a>(design: &'a RingDesign, seated: &'a dyn Fn(&Placement) -> Result<Motion>) -> Env<'a> {
    Env { design, seated, frame_of: &no_frame }
}

/// `count` frames along the band's crest from `from_deg` to `to_deg`, ends included, evenly spaced along it: `y` along the
/// crest, `z` out of the band.
pub fn crest_frames(design: &RingDesign, from_deg: f64, to_deg: f64, count: u32) -> Result<Vec<Motion>> {
    let seated = |p: &Placement| p.frame(design);
    let a = Along { path: AlongPath::Crest { from_deg, to_deg }, count, ..Along::default() };
    Ok(stations(&a, &bare_env(design, &seated))?.into_iter().map(|s| s.frame).collect())
}

/// Chart points `[θ°, v mm]` on the bare band lifted `lift_mm` along its normal: a path drawn over the band, in the world.
pub fn chart_points(design: &RingDesign, points: &[[f64; 2]], lift_mm: f64) -> Result<Vec<[f64; 3]>> {
    ensure!(lift_mm.is_finite() && lift_mm.abs() <= 100.0, "A lift off the band is finite millimetres");
    ensure!(points.len() <= MAX_PATH_POINTS, "A chart path holds at most {MAX_PATH_POINTS} points");
    ensure!(design.band_is_procedural(), "A chart path lies on the band, and this ring's parts are the whole ring");
    ensure!(points.iter().flatten().all(|v| v.is_finite()), "A chart path's points are finite");
    let ctx = OnceCell::new();
    let band = Band { design, ctx: &ctx, bare: OnceCell::new() };
    Ok(points
        .iter()
        .map(|c| {
            let (p, n) = band.at(c[0], c[1]);
            std::array::from_fn(|k| p[k] + n[k] * lift_mm)
        })
        .collect())
}

/// The chart `v` of the band's crest at `theta_deg`, nearest `guess` (the reference crest's when `None`).
pub fn crest_v_at(design: &RingDesign, theta_deg: f64, guess: Option<f64>) -> Option<f64> {
    let ctx = design.field_context();
    let bare = BareSurface::new(design, &ctx);
    bare.parting_v(theta_deg, guess.unwrap_or(ctx.crest_v_mm))
}
