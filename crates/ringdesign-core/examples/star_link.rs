//! Star link for a bracelet: a four-pointed star with a round stone in its centre, domed to an even outline thick enough
//! for a cross hole side to side near each east and west tip, for flat jump rings. Lost wax.
//! Shapes: `sparkle` (the client's traced sketch: side lobes round the stone and two long needle points, the default) and
//! `star` (four even points).
//! Settings: `prong` (four claws over a carved seat, the default), `bezel` (a collet on a flat centre), `gypsy` (a flush seat
//! cut into a domed centre, open back). Writes STL, 3MF, the design, renders and, for the prong link, a dimensioned drawing
//! and the bracelet's layout.
//! The east and west points are stretched until `--links` links on jump rings with a `--clasp` mm lobster clasp come to
//! `--bracelet` mm end to end; `--stretch` sets the stretch outright.
//! cargo run --release -p ringdesign-core --example star_link -- [OUT_DIR] [--shape sparkle|star] [--stone 4]
//!     [--setting prong|bezel|gypsy]... [--links 10] [--bracelet 170] [--clasp 12] [--stretch MM]
use anyhow::{Context, Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, BuildResult, Mesh, RingDesign, Vec3,
    cad::{self, Attach, Component, ComponentRole, Document, FaceSeat, Feature, Operation, SurfaceKind, builders, stored},
    castability::CastProcess,
    gem::{Gem, GemCut},
    library, mesh, render,
    setting::{self, SolidKind},
    stl, threemf,
};
use serde_json::json;
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation, handles::FixedVertexHandle};
use std::f64::consts::{FRAC_PI_2, PI, SQRT_2, TAU};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const STAR: u64 = 1;
const STONE: u64 = 6;
const SILVER: [f32; 3] = [0.95, 0.93, 0.88];
const BLUE_TOPAZ: [f32; 3] = [0.16, 0.46, 0.80];
const SILVER_G_PER_MM3: f64 = 0.01036;
const GOLD14_G_PER_MM3: f64 = 0.01307;
/// Where the link is built, clear of the finger hole the builders keep out of; outputs are moved back to the origin.
const AWAY: f64 = 40.0;
/// Most points a sketch polyline takes, less a margin.
const MAX_OUTLINE_POINTS: usize = 500;
/// The seat disc the stone stands on, after the cylinder it is moved from.
const DISC: u64 = 31;
/// How far the seat disc's face sits under the dome's flat, so the disc lies wholly inside the body, and how deep it reaches.
const DISC_SUNK: f64 = 0.02;
const DISC_DEPTH: f64 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Setting {
    Prong,
    Bezel,
    Gypsy,
}

impl Setting {
    const ALL: [Setting; 3] = [Self::Prong, Self::Bezel, Self::Gypsy];
    fn key(self) -> &'static str {
        match self {
            Self::Prong => "prong",
            Self::Bezel => "bezel",
            Self::Gypsy => "gypsy",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        Self::ALL.into_iter().find(|v| v.key() == s).with_context(|| format!("no setting {s:?}; choose prong, bezel or gypsy"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Star,
    Sparkle,
}

impl Shape {
    const ALL: [Shape; 2] = [Self::Sparkle, Self::Star];
    fn key(self) -> &'static str {
        match self {
            Self::Star => "star",
            Self::Sparkle => "sparkle",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        Self::ALL.into_iter().find(|v| v.key() == s).with_context(|| format!("no shape {s:?}; choose sparkle or star"))
    }
    fn name(self) -> &'static str {
        match self {
            Self::Star => "Star link",
            Self::Sparkle => "Sparkle link",
        }
    }
    /// Prefix of every file written.
    fn stem(self) -> &'static str {
        match self {
            Self::Star => "star-link",
            Self::Sparkle => "sparkle-link",
        }
    }
    /// The points carrying the cross holes.
    fn ring_points(self) -> &'static str {
        match self {
            Self::Star => "east and west points",
            Self::Sparkle => "long points",
        }
    }
}

/// Everything the link is made from, mm unless named.
#[derive(Clone, Debug)]
struct Params {
    shape: Shape,
    stone: f64,
    /// North and south tips' reach before rounding, and the sparkle's long points' before their stretch; every point's half
    /// angle between its sides, and its tip rounding.
    tip: f64,
    tip_half_deg: f64,
    tip_round: f64,
    /// How much further the east and west tips reach than the north and south.
    stretch: f64,
    /// Centre to the outline on the 45° line.
    valley: f64,
    /// Cross hole through the east and west points: radius, its centre's distance back from the tip, and the metal kept
    /// under and over it.
    hole_r: f64,
    hole_inset: f64,
    hole_wall: f64,
    /// Thickness at the outline, every point alike; it carries the cross holes with their metal over them.
    wall: f64,
    /// The bezel's plateau: rise over the wall and radius.
    bezel_rise: f64,
    plateau: f64,
    /// The carved settings' domed centre: the flat's radius, and the prong setting's rise over the wall.
    seat_flat: f64,
    prong_rise: f64,
    /// Girdle over the flat for the prong setting.
    prong_lift: f64,
    /// Radius of the gypsy stone's open back where it meets the underside.
    back_hole: f64,
    /// How far short of the culet skin bulging a full hemisphere into the open back must stop.
    skin_margin: f64,
    /// Jump ring's centre-line radius and wire radius.
    jump_major: f64,
    jump_minor: f64,
    chord: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            shape: Shape::Star,
            stone: 4.0,
            tip: 7.0,
            tip_half_deg: 15.0,
            tip_round: 0.12,
            stretch: 0.0,
            valley: 3.1,
            hole_r: 0.5,
            hole_inset: 1.5,
            hole_wall: 0.5,
            wall: 2.0,
            bezel_rise: 0.6,
            plateau: 2.95,
            seat_flat: 2.4,
            prong_rise: 0.3,
            prong_lift: 0.7,
            back_hole: 0.6,
            skin_margin: 0.2,
            jump_major: 2.15,
            jump_minor: 0.4,
            chord: 0.1,
        }
    }
}

impl Params {
    /// The sparkle's holes sit further back from its needle tips, on bigger rings that clear two tips inside them.
    fn for_shape(shape: Shape) -> Self {
        match shape {
            Shape::Star => Self::default(),
            Shape::Sparkle => Self { shape, hole_inset: 2.3, jump_major: 2.65, ..Self::default() },
        }
    }
    fn gem(&self) -> Gem {
        Gem { preview_tint: Some(BLUE_TOPAZ), ..Gem::calibrated(GemCut::Round, self.stone) }
    }
    /// Lowest culet over the underside that skin bulging through the open back cannot reach.
    fn guarded_culet(&self) -> f64 {
        self.back_hole + self.skin_margin
    }
    /// Height of the face the stone is seated on; the gypsy's stands its culet at [`Self::guarded_culet`].
    fn seat_z(&self, s: Setting) -> f64 {
        match s {
            Setting::Bezel => self.wall + self.bezel_rise,
            Setting::Gypsy => self.guarded_culet() + self.gem().pavilion_mm() - self.girdle_over(s),
            Setting::Prong => self.wall + self.prong_rise,
        }
    }
    /// Girdle over the seat face; negative sinks it into the metal.
    fn girdle_over(&self, s: Setting) -> f64 {
        let gem = self.gem();
        match s {
            Setting::Bezel => builders::stand_off_mm("bezel", gem),
            Setting::Gypsy => -SolidKind::Flush.girdle_drop_mm(gem).unwrap_or(0.0),
            Setting::Prong => self.prong_lift,
        }
    }
    /// Height of the cross holes' axis.
    fn hole_z(&self) -> f64 {
        self.hole_wall + self.hole_r
    }
    /// Metal over the cross holes where they leave the points' sides.
    fn hole_cover(&self) -> f64 {
        self.wall - self.hole_z() - self.hole_r
    }
    /// Radius and height of the flat the seat disc stands on.
    fn flat(&self, s: Setting) -> (f64, f64) {
        match s {
            Setting::Bezel => (self.plateau, self.seat_z(s)),
            _ => (self.seat_flat, self.seat_z(s)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Cubic {
    p: [[f64; 2]; 4],
}

impl Cubic {
    fn at(&self, t: f64) -> [f64; 2] {
        let u = 1.0 - t;
        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        std::array::from_fn(|k| (0..4).map(|i| w[i] * self.p[i][k]).sum())
    }
    fn tangent(&self, t: f64) -> [f64; 2] {
        let u = 1.0 - t;
        let w = [-3.0 * u * u, 3.0 * u * u - 6.0 * u * t, 6.0 * u * t - 3.0 * t * t, 3.0 * t * t];
        std::array::from_fn(|k| (0..4).map(|i| w[i] * self.p[i][k]).sum())
    }
    /// Centre on the x axis and radius of the circle tangent to the curve at `t`.
    fn tip_circle(&self, t: f64) -> (f64, f64) {
        let (b, d) = (self.at(t), self.tangent(t));
        let x = b[0] + b[1] * d[1] / d[0];
        (x, (x - b[0]).hypot(b[1]))
    }
    /// The parameter near a tip on +x where a tangent circle centred on the axis has radius `r`.
    fn cut_for(&self, r: f64) -> Result<f64> {
        let (mut lo, mut hi) = (1e-9, 0.45);
        ensure!(self.tip_circle(hi).1 > r, "a {r} mm tip round does not fit the side");
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if self.tip_circle(mid).1 < r { lo = mid } else { hi = mid }
        }
        Ok(0.5 * (lo + hi))
    }
}

/// The side between two points of the north tip's reach: leaves each tip at `tip_half_deg`, crosses the 45° line at `valley`.
fn side(p: &Params) -> Result<Cubic> {
    let a = p.tip_half_deg.to_radians();
    let reach = (4.0 * p.tip - 4.0 * SQRT_2 * p.valley) / (3.0 * (a.cos() - a.sin()));
    ensure!(reach > 0.0 && reach < p.tip, "tip {} and valley {} leave no concave side at {}°", p.tip, p.valley, p.tip_half_deg);
    Ok(Cubic { p: [[p.tip, 0.0], [p.tip - reach * a.cos(), reach * a.sin()], [reach * a.sin(), p.tip - reach * a.cos()], [0.0, p.tip]] })
}

/// Half a side, from a tip on +x at `tip` to the valley: the even side's east half, its handles scaled with its chord.
fn half_side(p: &Params, tip: f64) -> Result<Cubic> {
    let [p0, p1, p2, p3] = side(p)?.p;
    let mid = |a: [f64; 2], b: [f64; 2]| [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
    let (h0, h1) = (mid(p0, p1), mid(mid(p0, p1), mid(p1, p2)));
    let m = mid(h1, mid(mid(p1, p2), mid(p2, p3)));
    let q0 = [tip, 0.0];
    let k = (q0[0] - m[0]).hypot(q0[1] - m[1]) / (p0[0] - m[0]).hypot(p0[1] - m[1]);
    let handle = |from: [f64; 2], to: [f64; 2], at: [f64; 2]| [at[0] + k * (to[0] - from[0]), at[1] + k * (to[1] - from[1])];
    Ok(Cubic { p: [q0, handle(p0, h0, q0), handle(m, h1, m), m] })
}

/// The star's outline, counter-clockwise from the east tip, and how far the east and north tips reach from the centre.
struct Star {
    pts: Vec<[f64; 2]>,
    tip_end: f64,
    north_end: f64,
}

impl Star {
    /// Half the width of the east point where it crosses `x`.
    fn half_width_at(&self, x: f64) -> f64 {
        let n = self.pts.len();
        (0..n)
            .filter_map(|i| {
                let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
                let inside = (a[0] - x) * (b[0] - x) <= 0.0 && a[0] > 0.0 && a[1] > 0.0 && b[1] > 0.0 && (b[0] - a[0]).abs() > 1e-12;
                inside.then(|| a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0]))
            })
            .fold(f64::NAN, f64::min)
    }
    /// Where the east and west cross holes are centred.
    fn hole_x(&self, p: &Params) -> f64 {
        self.tip_end - p.hole_inset
    }
}

/// The star's outline, its chord coarsened until it fits one polyline.
fn star(p: &Params) -> Result<Star> {
    let mut q = p.clone();
    for _ in 0..8 {
        let s = match p.shape {
            Shape::Star => star_at(&q)?,
            Shape::Sparkle => sparkle_at(&q)?,
        };
        if s.pts.len() <= MAX_OUTLINE_POINTS {
            return Ok(s);
        }
        q.chord *= 1.02 * s.pts.len() as f64 / MAX_OUTLINE_POINTS as f64;
    }
    bail!("the outline does not fit {MAX_OUTLINE_POINTS} points")
}

/// The star's outline at `p.chord`: one quarter from the east tip to the north tip, mirrored round.
fn star_at(p: &Params) -> Result<Star> {
    // The east tip's half of the side, and the north tip's as if turned onto +x.
    let [east, north] = [half_side(p, p.tip + p.stretch)?, half_side(p, p.tip)?];
    // Where each tip's round leaves its half: the parameter, the round's centre on the axis and radius, and its angle there.
    let round = |h: &Cubic| -> Result<(f64, f64, f64, f64)> {
        let t = h.cut_for(p.tip_round)?;
        let ((xc, r), q) = (h.tip_circle(t), h.at(t));
        Ok((t, xc, r, q[1].atan2(q[0] - xc)))
    };
    let ((te, xe, re, be), (tn, xn, rn, bn)) = (round(&east)?, round(&north)?);
    // The east tip's upper half, from the axis to where the side leaves it.
    let mut a = cap(0.0, be, [xe, 0.0], re, p.chord);
    // The side by arc length through the valley, its ends left to the tips.
    let dense: Vec<[f64; 2]> = (0..=2000)
        .map(|i| east.at(te + (1.0 - te) * i as f64 / 2000.0))
        .chain((1..=2000).map(|i| {
            let q = north.at(1.0 - (1.0 - tn) * i as f64 / 2000.0);
            [q[1], q[0]]
        }))
        .collect();
    let mut run = vec![0.0];
    for w in dense.windows(2) {
        run.push(run.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let total = *run.last().unwrap();
    let steps = (total / p.chord).ceil() as usize;
    let mut j = 0;
    for s in 1..steps {
        let at = total * s as f64 / steps as f64;
        while run[j + 1] < at {
            j += 1;
        }
        let f = (at - run[j]) / (run[j + 1] - run[j]);
        a.push([dense[j][0] + (dense[j + 1][0] - dense[j][0]) * f, dense[j][1] + (dense[j + 1][1] - dense[j][1]) * f]);
    }
    // The north tip's right half, from the side's tangent point to its tip.
    a.extend(cap(FRAC_PI_2 - bn, FRAC_PI_2, [0.0, xn], rn, p.chord));
    Ok(Star { pts: round_from_quarter(&a)?, tip_end: xe + re, north_end: xn + rn })
}

/// A tip's round arc on `centre`, from angle `from` to `to`, with chords no longer than half `chord`.
fn cap(from: f64, to: f64, centre: [f64; 2], r: f64, chord: f64) -> Vec<[f64; 2]> {
    let n = ((r * (to - from).abs() / (0.5 * chord)).ceil() as usize).max(4);
    (0..=n).map(|i| from + (to - from) * i as f64 / n as f64).map(|phi| [centre[0] + r * phi.cos(), centre[1] + r * phi.sin()]).collect()
}

/// The whole outline from its quarter running from the +x axis to the +y axis, mirrored round, each quarter sharing its end
/// points with its neighbours; refused unless star-shaped about the centre.
fn round_from_quarter(a: &[[f64; 2]]) -> Result<Vec<[f64; 2]>> {
    let (mx, my) = (|q: [f64; 2]| [-q[0], q[1]], |q: [f64; 2]| [q[0], -q[1]]);
    let mut pts = a.to_vec();
    pts.extend(a.iter().rev().skip(1).map(|&q| mx(q)));
    pts.extend(a.iter().skip(1).map(|&q| mx(my(q))));
    pts.extend(a.iter().rev().skip(1).take(a.len() - 2).map(|&q| my(q)));
    // Polar angle climbs once round.
    let mut climb = 0.0;
    for i in 0..pts.len() {
        let (u, w) = (pts[i], pts[(i + 1) % pts.len()]);
        ensure!((w[0] - u[0]).hypot(w[1] - u[1]) > 1e-6, "outline point {i} repeats");
        let d = (w[1].atan2(w[0]) - u[1].atan2(u[0]) + PI).rem_euclid(TAU) - PI;
        ensure!(d > 0.0, "outline turns back at point {i}");
        climb += d;
    }
    ensure!((climb - TAU).abs() < 1e-6, "outline winds {climb} rad");
    Ok(pts)
}

/// The client's sparkle traced from their sketch, a 4 mm stone in it: half its width every `SPARKLE_TIP / 54` mm out
/// along a long point, from the centre to the tip, the four quarters averaged.
const SPARKLE_TRACE: [f64; 55] = [
    3.7689, 3.7406, 3.6487, 3.4958, 3.2953, 3.0586, 2.8078, 2.5691, 2.3672, 2.2051, 2.0699, 1.9540, 1.8534, 1.7635, 1.6810,
    1.6035, 1.5299, 1.4596, 1.3923, 1.3280, 1.2663, 1.2063, 1.1477, 1.0911, 1.0359, 0.9817, 0.9287, 0.8768, 0.8263, 0.7776,
    0.7310, 0.6866, 0.6445, 0.6044, 0.5667, 0.5315, 0.4986, 0.4669, 0.4362, 0.4065, 0.3777, 0.3494, 0.3214, 0.2939, 0.2669,
    0.2406, 0.2146, 0.1891, 0.1639, 0.1389, 0.1146, 0.0860, 0.0573, 0.0287, 0.0,
];
const SPARKLE_TIP: f64 = 10.8138;
const SPARKLE_STONE: f64 = 4.0;
/// Out to `SPARKLE_KEEP` the side lobes are the sketch's; past `SPARKLE_FREE` the long points are the sketch's sampled
/// faster or slower to reach their tips; mm at the sketch's stone.
const SPARKLE_KEEP: f64 = 1.5;
const SPARKLE_FREE: f64 = 3.5;

/// A cubic spline through evenly spaced values: level at the first, leaving the last along its final chord.
struct Spline {
    h: f64,
    y: Vec<f64>,
    /// Second derivative at each knot.
    m: Vec<f64>,
}

impl Spline {
    fn clamped_level(y: &[f64], h: f64) -> Self {
        let n = y.len();
        let end = (y[n - 1] - y[n - 2]) / h;
        // Tridiagonal system for the second derivatives, solved by elimination.
        let (mut a, mut b, mut c, mut d) = (vec![1.0; n], vec![4.0; n], vec![1.0; n], vec![0.0; n]);
        (a[0], b[0], d[0]) = (0.0, 2.0, 6.0 / h * ((y[1] - y[0]) / h));
        (b[n - 1], c[n - 1], d[n - 1]) = (2.0, 0.0, 6.0 / h * (end - (y[n - 1] - y[n - 2]) / h));
        for i in 1..n - 1 {
            d[i] = 6.0 / (h * h) * (y[i + 1] - 2.0 * y[i] + y[i - 1]);
        }
        for i in 1..n {
            let k = a[i] / b[i - 1];
            b[i] -= k * c[i - 1];
            d[i] -= k * d[i - 1];
        }
        let mut m = vec![0.0; n];
        m[n - 1] = d[n - 1] / b[n - 1];
        for i in (0..n - 1).rev() {
            m[i] = (d[i] - c[i] * m[i + 1]) / b[i];
        }
        Self { h, y: y.to_vec(), m }
    }
    /// Value and slope at `x`, within the knots.
    fn at(&self, x: f64) -> (f64, f64) {
        let h = self.h;
        let i = ((x / h).floor().max(0.0) as usize).min(self.y.len() - 2);
        let (l, r) = (h * (i + 1) as f64 - x, x - h * i as f64);
        let (m0, m1) = (self.m[i], self.m[i + 1]);
        let (c0, c1) = (self.y[i] / h - m0 * h / 6.0, self.y[i + 1] / h - m1 * h / 6.0);
        let v = m0 * l * l * l / (6.0 * h) + m1 * r * r * r / (6.0 * h) + c0 * l + c1 * r;
        let s = -m0 * l * l / (2.0 * h) + m1 * r * r / (2.0 * h) - c0 + c1;
        (v, s)
    }
}

/// The sparkle's outline: the sketch scaled with the stone, its lobes kept and its long points carried out to `tip +
/// stretch`, their tips rounded to `tip_round`.
fn sparkle_at(p: &Params) -> Result<Star> {
    let k = p.stone / SPARKLE_STONE;
    let trace = Spline::clamped_level(&SPARKLE_TRACE, SPARKLE_TIP / (SPARKLE_TRACE.len() - 1) as f64);
    let (keep, free, reach) = (k * SPARKLE_KEEP, k * SPARKLE_FREE, p.tip + p.stretch);
    ensure!(reach > free + 1.0, "long points reaching {reach:.2} leave the lobes no room");
    // x maps onto the sketch at x + gain·g(x), g rising from 0 to a slope of 1 by a smootherstep from `keep` to `free`.
    let gain = (k * SPARKLE_TIP - reach) / (reach - 0.5 * (keep + free));
    ensure!(1.0 + gain > 0.05, "long points reaching {reach:.2} stretch the sketch's past folding");
    let half = |x: f64| -> (f64, f64) {
        let u = ((x - keep) / (free - keep)).clamp(0.0, 1.0);
        let g = if x <= free { (free - keep) * u.powi(4) * (u * (u - 3.0) + 2.5) } else { x - 0.5 * (keep + free) };
        let step = u * u * u * (u * (6.0 * u - 15.0) + 10.0);
        let (w, slope) = trace.at(((x + gain * g) / k).min(SPARKLE_TIP));
        (k * w.max(0.0), slope * (1.0 + gain * step))
    };
    // The tip's round: where the circle on the axis tangent to the side has radius `tip_round`.
    let radius = |x: f64| {
        let (w, s) = half(x);
        w * (1.0 + s * s).sqrt()
    };
    ensure!(radius(free) > p.tip_round, "a {} mm tip round does not fit the long point", p.tip_round);
    let (mut lo, mut hi) = (free, reach);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if radius(mid) > p.tip_round { lo = mid } else { hi = mid }
    }
    let x0 = 0.5 * (lo + hi);
    let (w0, s0) = half(x0);
    let (xc, r) = (x0 + w0 * s0, radius(x0));
    let mut a = cap(0.0, w0.atan2(x0 - xc), [xc, 0.0], r, p.chord);
    // The side from the round back to the lobe's crown on the y axis, by arc length.
    let dense: Vec<[f64; 2]> = (0..=4000).map(|i| x0 * (1.0 - i as f64 / 4000.0)).map(|x| [x, half(x).0]).collect();
    let mut run = vec![0.0];
    for w in dense.windows(2) {
        run.push(run.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let total = *run.last().unwrap();
    let steps = (total / p.chord).ceil() as usize;
    let mut j = 0;
    for s in 1..steps {
        let at = total * s as f64 / steps as f64;
        while run[j + 1] < at {
            j += 1;
        }
        let f = (at - run[j]) / (run[j + 1] - run[j]);
        a.push([dense[j][0] + (dense[j + 1][0] - dense[j][0]) * f, dense[j][1] + (dense[j + 1][1] - dense[j][1]) * f]);
    }
    let crown = half(0.0).0;
    a.push([0.0, crown]);
    Ok(Star { pts: round_from_quarter(&a)?, tip_end: xc + r, north_end: crown })
}

/// A bracelet of `links` stars on flat jump rings, a lobster clasp hung on one end ring and hooked into the other.
#[derive(Clone, Copy, Debug)]
struct Bracelet {
    links: usize,
    /// End to end with the clasp, laid straight.
    length: f64,
    /// The lobster clasp's overall length with its loop.
    clasp: f64,
}

impl Default for Bracelet {
    fn default() -> Self {
        Self { links: 10, length: 170.0, clasp: 12.0 }
    }
}

/// A bracelet's lengths with every ring pulled taut, mm.
struct Lengths {
    /// A cross hole's centre to its ring's centre.
    reach: f64,
    /// Link centre to link centre.
    pitch: f64,
    /// The first link's west tip to the last link's east tip.
    tips: f64,
    /// One end ring's outer edge to the other's.
    rings: f64,
    /// End to end with the clasp.
    length: f64,
}

/// The lengths of `b` on links of outline `st`, each ring's wire against its hole's tip side, the clasp's loop in its ring.
fn lengths(p: &Params, st: &Star, b: &Bracelet) -> Lengths {
    let hx = st.hole_x(p);
    let half = st.half_width_at(hx + p.hole_r);
    let reach = p.hole_r - p.jump_minor + (p.jump_major * p.jump_major - half * half).max(0.0).sqrt();
    let pitch = 2.0 * (hx + reach);
    let gaps = b.links.saturating_sub(1) as f64;
    let rings = gaps * pitch + 2.0 * (hx + reach + p.jump_major + p.jump_minor);
    Lengths { reach, pitch, tips: gaps * pitch + 2.0 * st.tip_end, rings, length: rings + b.clasp - 4.0 * p.jump_minor }
}

/// The bracelet's length end to end with the east and west points stretched by `stretch`.
fn length_at(p: &Params, b: &Bracelet, stretch: f64) -> Result<f64> {
    let q = Params { stretch, ..p.clone() };
    Ok(lengths(&q, &star(&q)?, b).length)
}

/// The stretch of the east and west points that brings `b` to its length.
fn solve_stretch(p: &Params, b: &Bracelet) -> Result<f64> {
    let (mut lo, mut hi) = (-1.0, 3.0);
    let (short, long) = (length_at(p, b, lo)?, length_at(p, b, hi)?);
    ensure!((short..=long).contains(&b.length), "{} links make {short:.1} to {long:.1} mm with a {} mm clasp, not {}", b.links, b.clasp, b.length);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if length_at(p, b, mid)? < b.length { lo = mid } else { hi = mid }
    }
    Ok(0.5 * (lo + hi))
}

/// Spacing of the interior points the body's top is triangulated through, mm.
const GRID_MM: f64 = 0.07;

/// The outline's radius along the ray at `theta`, the outline being star-shaped about the centre.
fn radius_at(pts: &[[f64; 2]], unwrapped: &[f64], theta: f64) -> f64 {
    let a0 = unwrapped[0];
    let t = a0 + (theta - a0).rem_euclid(TAU);
    let i = unwrapped.partition_point(|a| *a <= t).saturating_sub(1).min(pts.len() - 1);
    let (p0, p1) = (pts[i], pts[(i + 1) % pts.len()]);
    let (e, d) = ([p1[0] - p0[0], p1[1] - p0[1]], [theta.cos(), theta.sin()]);
    (p0[0] * e[1] - p0[1] * e[0]) / (d[0] * e[1] - d[1] * e[0])
}

/// Whether `q` lies inside the closed outline `pts`, and its distance to the outline.
fn inside_and_distance(pts: &[[f64; 2]], q: [f64; 2]) -> (bool, f64) {
    let (mut inside, mut best) = (false, f64::MAX);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) {
            inside = !inside;
        }
        let e = [b[0] - a[0], b[1] - a[1]];
        let t = (((q[0] - a[0]) * e[0] + (q[1] - a[1]) * e[1]) / (e[0] * e[0] + e[1] * e[1])).clamp(0.0, 1.0);
        best = best.min((q[0] - a[0] - t * e[0]).hypot(q[1] - a[1] - t * e[1]));
    }
    (inside, best)
}

/// The star's body as a closed mesh, in world millimetres centred `AWAY` along y: a constrained Delaunay triangulation of
/// the outline and an even grid inside it, lifted to a dome falling from the seat flat to the outline, walls straight
/// down at the outline, and a flat underside.
fn body_mesh(p: &Params, s: Setting, st: &Star) -> Result<stored::Packed> {
    let (flat, top) = p.flat(s);
    let pts = &st.pts;
    let mut unwrapped = vec![pts[0][1].atan2(pts[0][0])];
    for w in pts.windows(2) {
        let d = (w[1][1].atan2(w[1][0]) - w[0][1].atan2(w[0][0]) + PI).rem_euclid(TAU) - PI;
        unwrapped.push(unwrapped.last().unwrap() + d);
    }
    let height = |q: [f64; 2]| -> f64 {
        let r = q[0].hypot(q[1]);
        let outer = if r < 1e-9 { flat + 1.0 } else { radius_at(pts, &unwrapped, q[1].atan2(q[0])) };
        let f = ((outer - r) / (outer - flat)).clamp(0.0, 1.0);
        let rise = if s == Setting::Bezel { f } else { (FRAC_PI_2 * f).sin() };
        p.wall + (top - p.wall) * rise
    };
    /// Inserts `q`, recording its position under the handle the triangulation gave it.
    fn put(q: [f64; 2], cdt: &mut ConstrainedDelaunayTriangulation<Point2<f64>>, at: &mut Vec<[f64; 2]>) -> Result<FixedVertexHandle> {
        let h = cdt.insert(Point2::new(q[0], q[1])).map_err(|e| anyhow::anyhow!("a point will not triangulate: {e:?}"))?;
        if h.index() == at.len() {
            at.push(q);
        }
        Ok(h)
    }
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    let mut flat_pts: Vec<[f64; 2]> = Vec::new();
    let rim: Vec<FixedVertexHandle> = pts.iter().map(|q| put(*q, &mut cdt, &mut flat_pts)).collect::<Result<_>>()?;
    ensure!(flat_pts.len() == pts.len(), "two outline points coincide");
    for i in 0..rim.len() {
        cdt.add_constraint(rim[i], rim[(i + 1) % rim.len()]);
    }
    let reach = pts.iter().map(|q| q[0].abs().max(q[1].abs())).fold(0.0, f64::max);
    let n = (reach / GRID_MM).ceil() as i64;
    for gx in -n..=n {
        for gy in -n..=n {
            let q = [gx as f64 * GRID_MM, gy as f64 * GRID_MM];
            let (inside, d) = inside_and_distance(pts, q);
            if inside && d > 0.6 * GRID_MM {
                put(q, &mut cdt, &mut flat_pts)?;
            }
        }
    }
    ensure!(flat_pts.len() == cdt.num_vertices(), "the triangulation merged {} points", flat_pts.len() as i64 - cdt.num_vertices() as i64);
    let tops: Vec<[u32; 3]> = cdt
        .inner_faces()
        .map(|f| f.vertices().map(|v| v.fix().index() as u32))
        .filter(|t| {
            let c = [(0..3).map(|k| flat_pts[t[k] as usize][0]).sum::<f64>() / 3.0, (0..3).map(|k| flat_pts[t[k] as usize][1]).sum::<f64>() / 3.0];
            inside_and_distance(pts, c).0
        })
        .collect();
    let nv = flat_pts.len() as u32;
    let mut pos: Vec<[f64; 3]> = flat_pts.iter().map(|q| [q[0], q[1] + AWAY, height(*q)]).collect();
    pos.extend(flat_pts.iter().map(|q| [q[0], q[1] + AWAY, 0.0]));
    let (mut tri, mut face) = (Vec::new(), Vec::new());
    for t in &tops {
        tri.push(*t);
        tri.push([t[0] + nv, t[2] + nv, t[1] + nv]);
        face.extend([0, 2]);
    }
    for i in 0..rim.len() {
        let (a, b) = (rim[i].index() as u32, rim[(i + 1) % rim.len()].index() as u32);
        tri.push([a, a + nv, b + nv]);
        tri.push([a, b + nv, b]);
        face.extend([1, 1]);
    }
    stored::Packed::encode(&pos, &tri, &face, &[SurfaceKind::Freeform, SurfaceKind::Freeform, SurfaceKind::Plane])
}

fn feature(id: u64, name: &str, operation: Operation, role: ComponentRole) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { role, ..Component::default() } }
}

/// A cut side to side through the point at `(x, 0)`: a cylinder, built centred on z, turned onto y.
fn cross_hole(doc: &mut Document, raw: u64, name: &str, x: f64, p: &Params) -> Result<()> {
    doc.append(feature(raw, &format!("{name} tool"), Operation::Cylinder { radius_mm: p.hole_r, height_mm: 4.0 }, ComponentRole::Other))?;
    let turned = Operation::Transform { source: raw, translation: [x, AWAY, p.hole_z()], rotation_deg: [90.0, 0.0, 0.0] };
    let mut f = feature(raw + 1, name, turned, ComponentRole::Other);
    f.component.attach = Attach::Cut;
    doc.append(f)
}

/// The star body with its seat disc and two cross holes.
fn body(p: &Params, s: Setting) -> Result<Document> {
    let st = star(p)?;
    ensure!(p.hole_cover() >= p.hole_wall - 1e-9, "a {:.2} mm outline leaves {:.2} over the cross holes, under {:.2}", p.wall, p.hole_cover(), p.hole_wall);
    let mut doc = Document::default();
    let recipe = stored::Recipe { kernel: "star_link".into(), op: "import".into(), params: json!({ "shape": p.shape.key(), "setting": s.key() }), digest: String::new() };
    doc.append(feature(STAR, p.shape.name(), Operation::Stored { recipe, sources: Vec::new(), mesh: body_mesh(p, s, &st)? }, ComponentRole::Head))?;
    let (flat, top) = p.flat(s);
    doc.append(feature(DISC - 1, "Seat disc tool", Operation::Cylinder { radius_mm: flat, height_mm: DISC_DEPTH }, ComponentRole::Setting))?;
    let disc = Operation::Transform { source: DISC - 1, translation: [0.0, AWAY, top - DISC_SUNK - 0.5 * DISC_DEPTH], rotation_deg: [0.0; 3] };
    doc.append(feature(DISC, "Seat disc", disc, ComponentRole::Setting))?;
    let x = st.hole_x(p);
    cross_hole(&mut doc, 2, "Jump-ring hole, east", x, p)?;
    cross_hole(&mut doc, 4, "Jump-ring hole, west", -x, p)?;
    Ok(doc)
}

fn design(p: &Params, s: Setting, lib: &AlphaLibrary, params: BuildParams) -> Result<RingDesign> {
    let mut d = RingDesign::default();
    d.name = format!("{}, {}", p.shape.name(), s.key());
    d.draft.process = CastProcess::LostWax;
    d.cad = Some(body(p, s)?);
    let bare = mesh::try_build(&d, lib, params).context("star and holes")?;
    for n in &bare.parts.notes {
        println!("  bare note: {n}");
    }
    let star = bare.parts.evaluated.as_ref().context("no evaluation")?.components.iter().find(|c| c.id == DISC).context("no seat disc")?;
    ensure!(star.made.is_none(), "the seat disc came back as a mesh, and a stone sits on a kernel face");
    // The seat face: the planar face facing up, highest.
    let mut top: Option<(u32, f64)> = None;
    for f in 0..star.body.faces.len() as u32 {
        let Ok(seat) = FaceSeat::on(star, f, None, 0.0) else { continue };
        let Ok(frame) = seat.face_of(star) else { continue };
        if frame.normal[2] > 0.999 && top.is_none_or(|(_, z)| frame.origin[2] > z) {
            top = Some((f, frame.origin[2]));
        }
    }
    let (face, z) = top.context("the star has no upward planar face")?;
    ensure!((z - p.seat_z(s) + DISC_SUNK).abs() < 1e-6, "the upward face stands at {z}, not the seat's {}", p.seat_z(s) - DISC_SUNK);
    let gem = p.gem();
    let seat = FaceSeat::on(star, face, None, p.girdle_over(s) + DISC_SUNK)?;
    let doc = d.cad.as_mut().unwrap();
    doc.append(cad::stone_on_face(STONE, gem, DISC, &seat))?;
    let mut next = STONE + 1;
    let mut id = || {
        next += 1;
        next - 1
    };
    let made = match s {
        Setting::Bezel => builders::setting_features("bezel", STONE, gem, false, &mut id)?,
        Setting::Gypsy => vec![builders::feature_on(id(), "Seat bur", builders::BUR, STONE, json!({}))],
        Setting::Prong => {
            let mut v = builders::setting_features("claw4", STONE, gem, false, &mut id)?;
            for f in &mut v {
                if let Operation::Builder { key, params, .. } = &mut f.operation
                    && key == builders::CLAW
                {
                    params["rails"] = json!("None");
                }
            }
            v
        }
    };
    for f in made {
        doc.append(f)?;
    }
    // An open back from under the gypsy stone's culet through the star's underside.
    if s == Setting::Gypsy {
        let culet = p.seat_z(s) + p.girdle_over(s) - gem.pavilion_mm();
        ensure!(culet - p.back_hole >= p.skin_margin - 1e-9, "skin bulging through a {:.2} mm opening reaches the culet {culet:.2} mm up", 2.0 * p.back_hole);
        let (raw, cut) = (id(), id());
        doc.append(feature(raw, "Back hole", Operation::Cylinder { radius_mm: p.back_hole, height_mm: culet + 0.5 }, ComponentRole::Setting))?;
        // A cylinder is built centred on its height: this spans 0.5 under the underside up to the culet.
        let mut f = feature(cut, "Open back", Operation::Transform { source: raw, translation: [0.0, AWAY, 0.5 * (culet - 0.5)], rotation_deg: [0.0; 3] }, ComponentRole::Setting);
        f.component.attach = Attach::Cut;
        doc.append(f)?;
    }
    Ok(d)
}

fn moved(m: &Mesh, d: [f64; 3]) -> Mesh {
    let d = d.map(|v| v as f32);
    Mesh { vertices: m.vertices.iter().map(|v| Vec3(v.0 + d[0], v.1 + d[1], v.2 + d[2])).collect(), normals: m.normals.clone(), faces: m.faces.clone(), corner_normals: m.corner_normals.clone(), ..Default::default() }
}

fn merged(parts: &[Mesh]) -> Mesh {
    let mut out = Mesh::default();
    for m in parts {
        let (v0, f0) = (out.vertices.len() as u32, out.faces.len() as u32);
        out.vertices.extend(&m.vertices);
        out.normals.extend(&m.normals);
        out.faces.extend(m.faces.iter().map(|f| f.map(|i| i + v0)));
        out.corner_normals.extend(m.corner_normals.iter().map(|(f, n)| (f + f0, *n)));
    }
    out
}

/// Closed pieces in `m`, by faces sharing vertices.
fn shells(m: &Mesh) -> usize {
    let mut up: Vec<usize> = (0..m.vertices.len()).collect();
    fn root(up: &mut [usize], mut i: usize) -> usize {
        while up[i] != i {
            up[i] = up[up[i]];
            i = up[i];
        }
        i
    }
    for f in &m.faces {
        for k in 1..3 {
            let (a, b) = (root(&mut up, f[0] as usize), root(&mut up, f[k] as usize));
            up[a] = b;
        }
    }
    let mut roots: Vec<usize> = m.faces.iter().map(|f| root(&mut up, f[0] as usize)).collect();
    roots.sort_unstable();
    roots.dedup();
    roots.len()
}

/// A torus lying flat: a jump ring through two links' cross holes.
fn jump_ring(centre: [f64; 3], major: f64, minor: f64) -> Mesh {
    let (nu, nv) = (96usize, 24usize);
    let mut m = Mesh::default();
    for i in 0..nu {
        let u = TAU * i as f64 / nu as f64;
        for j in 0..nv {
            let v = TAU * j as f64 / nv as f64;
            let n = [v.cos() * u.cos(), v.cos() * u.sin(), v.sin()];
            let p: [f64; 3] = std::array::from_fn(|k| centre[k] + [major * u.cos(), major * u.sin(), 0.0][k] + minor * n[k]);
            m.vertices.push(Vec3(p[0] as f32, p[1] as f32, p[2] as f32));
            m.normals.push(Vec3(n[0] as f32, n[1] as f32, n[2] as f32));
        }
    }
    let at = |i: usize, j: usize| ((i % nu) * nv + j % nv) as u32;
    for i in 0..nu {
        for j in 0..nv {
            m.faces.push([at(i, j), at(i, j + 1), at(i + 1, j)]);
            m.faces.push([at(i + 1, j), at(i, j + 1), at(i + 1, j + 1)]);
        }
    }
    // Winds every face outward.
    let wrong = {
        let f = m.faces[0];
        let [a, b, c] = f.map(|i| m.vertices[i as usize]);
        let e = [b.0 - a.0, b.1 - a.1, b.2 - a.2];
        let g = [c.0 - a.0, c.1 - a.1, c.2 - a.2];
        let x = [e[1] * g[2] - e[2] * g[1], e[2] * g[0] - e[0] * g[2], e[0] * g[1] - e[1] * g[0]];
        let n = m.normals[f[0] as usize];
        x[0] * n.0 + x[1] * n.1 + x[2] * n.2 < 0.0
    };
    if wrong {
        for f in &mut m.faces {
            f.swap(1, 2);
        }
    }
    m
}

/// A finished link at the origin: its metal and its stones.
struct Link {
    setting: Setting,
    metal: Mesh,
    stones: Vec<(Mesh, [f32; 3])>,
}

fn render_set(path: &Path, metal: &Mesh, stones: &[(Mesh, [f32; 3])], yaw: f64, pitch: f64, edge: usize) -> Result<()> {
    let parts: Vec<render::Part<'_>> = std::iter::once(render::Part::metal(metal, SILVER)).chain(stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t))).collect();
    render::write_png_parts(path, &parts, yaw, pitch, edge)
}

/// What the dimensioned drawing reads off the built link.
struct Measured {
    bounds: [[f64; 2]; 3],
    claw_top: f64,
    table_z: f64,
    girdle_z: f64,
    culet_z: f64,
    volume: f64,
    /// Centre of each claw where it leaves the seat face, and its diameter there.
    claws: Vec<([f64; 2], f64)>,
}

fn measure(p: &Params, s: Setting, built: &BuildResult) -> Measured {
    let mut bounds = [[f64::MAX, f64::MIN]; 3];
    for v in &built.mesh.vertices {
        for (k, c) in [v.0 as f64, v.1 as f64 - AWAY, v.2 as f64].into_iter().enumerate() {
            bounds[k][0] = bounds[k][0].min(c);
            bounds[k][1] = bounds[k][1].max(c);
        }
    }
    let gem = p.gem();
    let girdle_z = p.seat_z(s) + p.girdle_over(s);
    let mut claws = Vec::new();
    if let Some(head) = built.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.name.contains("claw"))) {
        let (lo, hi) = (p.seat_z(s) + 0.05, p.seat_z(s) + 0.3);
        for q in 0..4 {
            let a = FRAC_PI_2 * q as f64 + 0.25 * PI;
            let near: Vec<[f64; 2]> = head
                .mesh
                .vertices
                .iter()
                .filter(|v| (lo..hi).contains(&(v.2 as f64)))
                .map(|v| [v.0 as f64, v.1 as f64 - AWAY])
                .filter(|v| ((v[1].atan2(v[0]) - a + PI).rem_euclid(TAU) - PI).abs() < 0.6)
                .collect();
            if near.is_empty() {
                continue;
            }
            let c: [f64; 2] = std::array::from_fn(|k| near.iter().map(|v| v[k]).sum::<f64>() / near.len() as f64);
            let r = near.iter().map(|v| (v[0] - c[0]).hypot(v[1] - c[1])).fold(0.0, f64::max);
            claws.push((c, 2.0 * r));
        }
    }
    Measured {
        claw_top: bounds[2][1],
        bounds,
        table_z: girdle_z + gem.crown_mm(),
        girdle_z,
        culet_z: girdle_z - gem.pavilion_mm(),
        volume: built.report.volume_mm3,
        claws,
    }
}

/// A plan view and an elevation with the link's dimensions, in millimetres.
fn drawing(p: &Params, s: Setting, m: &Measured, st: &Star, side: &[(f64, f64)], len: &Lengths, b: &Bracelet) -> String {
    let (w, h) = (44.0, 39.0);
    let hx = st.hole_x(p);
    let hw = st.half_width_at(hx);
    let mut o = String::new();
    let _ = writeln!(o, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {w} {h}" font-family="Helvetica, Arial, sans-serif">"#, w * 20.0, h * 20.0);
    let _ = writeln!(o, r##"<rect width="{w}" height="{h}" fill="#ffffff"/>
<defs><marker id="a" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0,1 L10,5 L0,9 z" fill="#1a1a1a"/></marker></defs>"##);
    let line = |o: &mut String, a: (f64, f64), b: (f64, f64)| {
        let _ = writeln!(o, r##"<line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" stroke="#1a1a1a" stroke-width="0.035" marker-start="url(#a)" marker-end="url(#a)"/>"##, a.0, a.1, b.0, b.1);
    };
    let thin = |o: &mut String, a: (f64, f64), b: (f64, f64)| {
        let _ = writeln!(o, r##"<line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" stroke="#8a8a90" stroke-width="0.025"/>"##, a.0, a.1, b.0, b.1);
    };
    let text = |o: &mut String, x: f64, y: f64, size: f64, anchor: &str, t: &str| {
        let _ = writeln!(o, r##"<text x="{x:.3}" y="{y:.3}" font-size="{size}" text-anchor="{anchor}" fill="#1a1a1a">{t}</text>"##);
    };
    // Plan view centred at (cx, cy); y up in the drawing means north.
    let (cx, cy) = (20.0, 11.0);
    let at = |x: f64, y: f64| (cx + x, cy - y);
    text(&mut o, 2.0, 1.6, 0.75, "start", "Plan");
    let poly: Vec<String> = st.pts.iter().map(|q| { let (x, y) = at(q[0], q[1]); format!("{x:.3},{y:.3}") }).collect();
    let _ = writeln!(o, r##"<polygon points="{}" fill="#e4e4e8" stroke="#1a1a1a" stroke-width="0.06"/>"##, poly.join(" "));
    // The cross holes as hidden lines, and the east jump ring lying flat through its hole.
    for x in [hx, -hx] {
        let (x0, y0) = at(x - p.hole_r, hw);
        let _ = writeln!(o, r##"<rect x="{x0:.3}" y="{y0:.3}" width="{:.3}" height="{:.3}" fill="none" stroke="#1a1a1a" stroke-width="0.04" stroke-dasharray="0.18 0.12"/>"##, 2.0 * p.hole_r, 2.0 * hw);
    }
    let (rx, ry) = at(hx + len.reach, 0.0);
    for r in [p.jump_major - p.jump_minor, p.jump_major + p.jump_minor] {
        let _ = writeln!(o, r##"<circle cx="{rx:.3}" cy="{ry:.3}" r="{r:.3}" fill="none" stroke="#9a7a3a" stroke-width="0.04" stroke-dasharray="0.3 0.15"/>"##);
    }
    let (sx, sy) = at(0.0, 0.0);
    let _ = writeln!(o, r##"<circle cx="{sx}" cy="{sy}" r="{}" fill="none" stroke="#7a7a80" stroke-width="0.04" stroke-dasharray="0.25 0.18"/>"##, p.seat_flat);
    let _ = writeln!(o, r##"<circle cx="{sx}" cy="{sy}" r="{}" fill="#b9d3ea" stroke="#2c5d8a" stroke-width="0.06"/>"##, 0.5 * p.stone);
    for (c, d) in &m.claws {
        let (x, y) = at(c[0], c[1]);
        let _ = writeln!(o, r##"<circle cx="{x:.3}" cy="{y:.3}" r="{:.3}" fill="#c9c9cf" stroke="#1a1a1a" stroke-width="0.05"/>"##, 0.5 * d);
    }
    let [bx, by, _] = m.bounds;
    // Tip to tip east to west, under the plan.
    let yl = by[0] - 1.4;
    thin(&mut o, at(bx[0], -0.3), at(bx[0], yl - 0.3));
    thin(&mut o, at(bx[1], -0.3), at(bx[1], yl - 0.3));
    line(&mut o, at(bx[0], yl), at(bx[1], yl));
    text(&mut o, cx, cy - yl + 0.75, 0.7, "middle", &format!("{:.2} tip to tip", bx[1] - bx[0]));
    // Hole centres, over the plan.
    let yh = by[1] + 0.9;
    thin(&mut o, at(hx, hw), at(hx, yh + 0.3));
    thin(&mut o, at(-hx, hw), at(-hx, yh + 0.3));
    line(&mut o, at(-hx, yh), at(hx, yh));
    text(&mut o, cx, cy - yh - 0.3, 0.7, "middle", &format!("{:.2} hole centres", 2.0 * hx));
    // North to south, to the left.
    let xw = bx[0] - 1.3;
    thin(&mut o, at(-0.3, by[1]), at(xw - 0.3, by[1]));
    thin(&mut o, at(-0.3, by[0]), at(xw - 0.3, by[0]));
    line(&mut o, at(xw, by[0]), at(xw, by[1]));
    let (tx, ty) = at(xw - 0.45, 0.0);
    let _ = writeln!(o, r##"<text x="{tx:.3}" y="{ty:.3}" font-size="0.7" text-anchor="middle" fill="#1a1a1a" transform="rotate(-90 {tx:.3} {ty:.3})">{:.2} tip to tip</text>"##, by[1] - by[0]);
    // Leaders: the cross hole, the jump ring, the stone and claws.
    let (hx0, hy0) = at(hx, -hw);
    // Beside the star's lower points; under the sparkle's length dimension, which its lobes leave too close.
    let leader_y = match p.shape {
        Shape::Star => -4.2,
        Shape::Sparkle => by[0] - 3.0,
    };
    let (lx, ly) = at(hx + 1.2, leader_y);
    thin(&mut o, (hx0, hy0), (lx, ly - 0.5));
    text(&mut o, lx, ly + 0.3, 0.6, "middle", &format!("Ø{:.2} cross hole, side to side", 2.0 * p.hole_r));
    text(&mut o, lx, ly + 1.05, 0.6, "middle", &format!("{:.2} from the tip, {:.2} up", p.hole_inset, p.hole_z()));
    let (jx, jy) = at(hx + len.reach, p.jump_major + p.jump_minor);
    text(&mut o, jx, jy - 0.4, 0.55, "middle", "jump ring, lying flat");
    let (gx, gy) = at(-0.5 * p.stone * 0.7, -0.5 * p.stone * 0.7);
    let (lx, ly) = at(-6.4, leader_y - 0.2);
    thin(&mut o, (gx, gy), (lx + 1.5, ly - 0.25));
    text(&mut o, lx, ly, 0.6, "middle", &format!("Ø{:.1} round", p.stone));
    if !m.claws.is_empty() {
        text(&mut o, lx, ly + 0.75, 0.6, "middle", &format!("{} claws, Ø{:.2} wire", m.claws.len(), setting::prong_wire_mm(p.gem())));
    }
    // Elevation from the south, under the plan, metal then stone; the cross holes show end on.
    let (ex, ez) = (20.0, 27.5);
    let ate = |x: f64, z: f64| (ex + x, ez - z);
    text(&mut o, 2.0, ez - 4.2, 0.75, "start", "Front elevation");
    let up: Vec<String> = side.iter().map(|(x, z)| { let (a, b) = ate(*x, *z); format!("{a:.3},{b:.3}") }).collect();
    let (l0, _) = ate(side.first().map_or(0.0, |s| s.0), 0.0);
    let (l1, _) = ate(side.last().map_or(0.0, |s| s.0), 0.0);
    let _ = writeln!(o, r##"<polygon points="{l0:.3},{ez:.3} {} {l1:.3},{ez:.3}" fill="#e4e4e8" stroke="#1a1a1a" stroke-width="0.06"/>"##, up.join(" "));
    for x in [hx, -hx] {
        let (a, b) = ate(x, p.hole_z());
        let _ = writeln!(o, r##"<circle cx="{a:.3}" cy="{b:.3}" r="{}" fill="#ffffff" stroke="#1a1a1a" stroke-width="0.05"/>"##, p.hole_r);
    }
    let r = 0.5 * p.stone;
    let stone: Vec<String> = [(-0.52 * r, m.table_z), (-r, m.girdle_z), (0.0, m.culet_z), (r, m.girdle_z), (0.52 * r, m.table_z)]
        .iter()
        .map(|(x, z)| { let (a, b) = ate(*x, *z); format!("{a:.3},{b:.3}") })
        .collect();
    let _ = writeln!(o, r##"<polygon points="{}" fill="#b9d3ea" stroke="#2c5d8a" stroke-width="0.05" fill-opacity="0.45"/>"##, stone.join(" "));
    // Heights, to the right of the elevation.
    let xr = bx[1] + 0.8;
    // Each label at least a line above the one under it, joined to its height by a short leader.
    let mut floor = f64::MAX;
    let mut heights = [
        (p.hole_z(), format!("{:.2} to the holes' centre", p.hole_z())),
        (p.wall, format!("{:.2} at the points and edges", p.wall)),
        (p.seat_z(s), format!("{:.2} under the stone", p.seat_z(s))),
        (m.table_z, format!("{:.2} to the table, claws {:.2}", m.table_z, m.claw_top)),
    ];
    heights.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (z, label) in heights {
        thin(&mut o, ate(0.0, z), ate(xr + 0.3, z));
        line(&mut o, ate(xr, 0.0), ate(xr, z));
        let (tx, ty) = ate(xr + 0.5, z);
        let ly = ty.min(floor - 0.75);
        floor = ly;
        if ly < ty - 0.05 {
            thin(&mut o, (xr + ex + 0.3, ty), (tx - 0.1, ly));
        }
        text(&mut o, tx, ly + 0.25, 0.6, "start", &label);
    }
    thin(&mut o, ate(bx[0] - 0.5, 0.0), ate(xr + 0.3, 0.0));
    // Notes.
    let waist = 2.0 * st.pts.iter().map(|q| q[0].hypot(q[1])).fold(f64::MAX, f64::min);
    let notes = [
        format!("{}, {} setting · all dimensions mm · lost wax", p.shape.name(), s.key()),
        format!("Stone: {:.1} mm round, girdle {:.2} over the seat; claws reach {:.2}", p.stone, p.girdle_over(s), m.claw_top),
        match p.shape {
            Shape::Star => format!("Four points R{:.2} at the tip; waist {waist:.2} across at its narrowest; the east and west points {:.2} wide at the holes", p.tip_round, 2.0 * hw),
            Shape::Sparkle => format!(
                "The client's sparkle: side lobes {:.2} across round the stone, long points R{:.2} at the tip and {:.2} wide at the holes",
                2.0 * st.north_end,
                p.tip_round,
                2.0 * hw
            ),
        },
        format!(
            "Even all round: {:.2} tall at the outline, domed to {:.2} under the stone; {:.2} of metal under each hole, {:.2} over it at the sides",
            p.wall,
            p.seat_z(s),
            p.hole_z() - p.hole_r,
            p.hole_cover()
        ),
        format!("Metal {:.1} mm³: {:.2} g sterling, {:.2} g 14k yellow", m.volume, m.volume * SILVER_G_PER_MM3, m.volume * GOLD14_G_PER_MM3),
        format!(
            "Jump rings (not cast), lying flat: Ø{:.1} wire, {:.1} ID through the cross holes; links {:.2} centre to centre, pulled straight",
            2.0 * p.jump_minor,
            2.0 * (p.jump_major - p.jump_minor),
            len.pitch
        ),
        format!(
            "Bracelet: {} links on {} rings with a {:.0} mm lobster clasp, {:.1} end to end; {}",
            b.links,
            b.links + 1,
            b.clasp,
            len.length,
            match p.shape {
                Shape::Star => format!("east and west points {:.2} longer than north and south", p.stretch),
                Shape::Sparkle => format!("long points {:.2} tip to tip", 2.0 * st.tip_end),
            }
        ),
    ];
    for (i, n) in notes.iter().enumerate() {
        text(&mut o, 2.0, 30.4 + 1.15 * i as f64, if i == 0 { 0.8 } else { 0.6 }, "start", n);
    }
    o.push_str("</svg>\n");
    o
}

/// The bracelet laid straight at full size: its links and rings, a stand-in for the clasp, and its lengths, in millimetres.
fn bracelet_drawing(p: &Params, st: &Star, b: &Bracelet, len: &Lengths, per_tenth: f64) -> String {
    let (x0, yc) = (10.0, 15.0);
    let (w, h) = (len.length + 2.0 * x0, 55.0);
    let (hx, m, major) = (st.hole_x(p), p.jump_minor, p.jump_major);
    // From the clasp's far end: its loop's outer edge, the end ring's wire inside it, the ring's centre, the hole.
    let first = x0 + b.clasp - 3.0 * m + major + len.reach + hx;
    let centres: Vec<f64> = (0..b.links).map(|i| first + i as f64 * len.pitch).collect();
    let last = first + (b.links - 1) as f64 * len.pitch;
    let mut o = String::new();
    let _ = writeln!(o, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{:.0}" height="{:.0}" viewBox="0 0 {w:.3} {h}" font-family="Helvetica, Arial, sans-serif">"#, w * 12.0, h * 12.0);
    let _ = writeln!(o, r##"<rect width="{w:.3}" height="{h}" fill="#ffffff"/>
<defs><marker id="a" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse"><path d="M0,1 L10,5 L0,9 z" fill="#1a1a1a"/></marker></defs>"##);
    let thin = |o: &mut String, x: f64, y0: f64, y1: f64| {
        let _ = writeln!(o, r##"<line x1="{x:.3}" y1="{y0:.3}" x2="{x:.3}" y2="{y1:.3}" stroke="#8a8a90" stroke-width="0.06"/>"##);
    };
    let dim = |o: &mut String, a: f64, b: f64, y: f64, label: &str| {
        let _ = writeln!(o, r##"<line x1="{a:.3}" y1="{y:.3}" x2="{b:.3}" y2="{y:.3}" stroke="#1a1a1a" stroke-width="0.08" marker-start="url(#a)" marker-end="url(#a)"/>"##);
        let _ = writeln!(o, r##"<text x="{:.3}" y="{:.3}" font-size="1.5" text-anchor="middle" fill="#1a1a1a">{label}</text>"##, 0.5 * (a + b), y - 0.5);
    };
    // Rings, under the links.
    let rings: Vec<f64> = std::iter::once(first - hx - len.reach).chain(centres.iter().map(|c| c + hx + len.reach)).collect();
    for x in &rings {
        let _ = writeln!(o, r##"<circle cx="{x:.3}" cy="{yc}" r="{major:.3}" fill="none" stroke="#b08d4a" stroke-width="{:.3}"/>"##, 2.0 * m);
    }
    // The clasp's stand-in: its loop, centre-line radius 1, and a body with its opening.
    let loop_x = x0 + b.clasp - m - 1.0;
    let body = (x0, loop_x - 1.0 - m + 0.4);
    let (bx, brx, bry) = (0.5 * (body.0 + body.1), 0.5 * (body.1 - body.0), 0.27 * b.clasp);
    let _ = writeln!(o, r##"<circle cx="{loop_x:.3}" cy="{yc}" r="1" fill="none" stroke="#7a7a80" stroke-width="{:.3}"/>"##, 2.0 * m);
    let _ = writeln!(o, r##"<ellipse cx="{bx:.3}" cy="{yc}" rx="{brx:.3}" ry="{bry:.3}" fill="#d6d6dc" stroke="#1a1a1a" stroke-width="0.08"/>"##);
    let _ = writeln!(o, r##"<ellipse cx="{:.3}" cy="{yc}" rx="{:.3}" ry="{:.3}" fill="#ffffff" stroke="#1a1a1a" stroke-width="0.06"/>"##, bx - 0.15 * brx, 0.62 * brx, 0.45 * bry);
    let _ = writeln!(o, r##"<text x="{bx:.3}" y="{:.3}" font-size="1.5" text-anchor="middle" fill="#1a1a1a">clasp</text>"##, yc - bry - 0.8);
    let poly: String = st.pts.iter().map(|q| format!("{:.3},{:.3}", q[0], -q[1])).collect::<Vec<_>>().join(" ");
    for c in &centres {
        let _ = writeln!(o, r##"<g transform="translate({c:.3} {yc})"><polygon points="{poly}" fill="#e4e4e8" stroke="#1a1a1a" stroke-width="0.08"/>"##);
        for x in [hx, -hx] {
            let hw = st.half_width_at(hx);
            let _ = writeln!(o, r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="none" stroke="#1a1a1a" stroke-width="0.05" stroke-dasharray="0.25 0.15"/>"##, x - p.hole_r, -hw, 2.0 * p.hole_r, 2.0 * hw);
        }
        let _ = writeln!(o, r##"<circle r="{:.3}" fill="#b9d3ea" stroke="#2c5d8a" stroke-width="0.08"/></g>"##, 0.5 * p.stone);
    }
    // Dimension lines.
    let top = yc - st.north_end;
    if b.links > 1 {
        thin(&mut o, centres[0], top - 3.6, yc - 3.0);
        thin(&mut o, centres[1], top - 3.6, yc - 3.0);
        dim(&mut o, centres[0], centres[1], top - 2.6, &format!("{:.2} link to link", len.pitch));
    }
    let low = yc + st.north_end;
    let (wt, et) = (centres[0] - st.tip_end, last + st.tip_end);
    let (wr, er) = (rings[0] - major - m, rings[rings.len() - 1] + major + m);
    for (x, y) in [(wt, 2.4), (centres[0] + st.tip_end, 2.4), (wt, 6.4), (et, 6.4), (wr, 10.4), (er, 10.4), (x0, 14.4), (x0 + len.length, 14.4)] {
        thin(&mut o, x, yc + 0.3, low + y + 0.4);
    }
    dim(&mut o, wt, centres[0] + st.tip_end, low + 2.4, &format!("{:.2}", 2.0 * st.tip_end));
    dim(&mut o, wt, et, low + 6.4, &format!("{:.1} the {} links tip to tip", len.tips, b.links));
    dim(&mut o, wr, er, low + 10.4, &format!("{:.1} with the end rings", len.rings));
    dim(&mut o, x0, x0 + len.length, low + 14.4, &format!("{:.1} end to end with the clasp", len.length));
    let notes = [
        format!("{} bracelet, {} links · all dimensions mm · full size", p.shape.name(), b.links),
        match p.shape {
            Shape::Star => format!(
                "Each link {:.2} east to west by {:.2} north to south: the east and west points reach {:.2} further than the north and south",
                2.0 * st.tip_end,
                2.0 * st.north_end,
                p.stretch
            ),
            Shape::Sparkle => format!(
                "Each link {:.2} along the bracelet by {:.2} across: the client's sparkle, joined long point to long point",
                2.0 * st.tip_end,
                2.0 * st.north_end
            ),
        },
        format!(
            "{} jump rings, Ø{:.1} wire, {:.1} ID, lying flat through the cross holes; {:.2} link to link with every ring pulled straight",
            b.links + 1,
            2.0 * m,
            2.0 * (major - m),
            len.pitch
        ),
        format!(
            "Lobster clasp {:.0} long overall with its loop, hung on one end ring and hooked into the other; its loop lies inside its ring, so it adds {:.1}",
            b.clasp,
            len.length - len.rings
        ),
        format!(
            "{:.1} + {:.1} = {:.1} end to end. Each 0.1 on the {} moves this {per_tenth:.1}; each mm of clasp, one mm. The clasp is a stand-in.",
            len.rings,
            len.length - len.rings,
            len.length,
            p.shape.ring_points()
        ),
    ];
    for (i, n) in notes.iter().enumerate() {
        let _ = writeln!(o, r##"<text x="{x0}" y="{:.3}" font-size="{}" fill="#1a1a1a">{n}</text>"##, low + 21.0 + 2.4 * i as f64, if i == 0 { 2.1 } else { 1.6 });
    }
    o.push_str("</svg>\n");
    o
}

/// The link's outline seen from the south: highest metal over each x, sampled every `step`.
fn elevation(mesh: &Mesh, step: f64) -> Vec<(f64, f64)> {
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for v in &mesh.vertices {
        lo = lo.min(v.0 as f64);
        hi = hi.max(v.0 as f64);
    }
    let n = ((hi - lo) / step).ceil() as usize + 1;
    let mut top = vec![f64::MIN; n];
    for f in &mesh.faces {
        let [a, b, c] = f.map(|i| mesh.vertices[i as usize]);
        // Each triangle's edges sampled densely enough to land in every bin they cross.
        for (u, w) in [(a, b), (b, c), (c, a)] {
            let k = (((u.0 - w.0).abs() as f64 / step).ceil() as usize).max(1);
            for i in 0..=k {
                let t = i as f32 / k as f32;
                let (x, z) = (u.0 + (w.0 - u.0) * t, u.2 + (w.2 - u.2) * t);
                let bin = (((x as f64) - lo) / step).round() as usize;
                top[bin.min(n - 1)] = top[bin.min(n - 1)].max(z as f64);
            }
        }
    }
    top.iter().enumerate().filter(|(_, z)| **z > f64::MIN).map(|(i, z)| (lo + step * i as f64, *z)).collect()
}

fn make(p: &Params, b: &Bracelet, s: Setting, lib: &AlphaLibrary, params: BuildParams, out: &Path) -> Result<Link> {
    let d = design(p, s, lib, params)?;
    let built: BuildResult = mesh::try_build(&d, lib, params)?;
    let (key, stem) = (s.key(), p.shape.stem());
    let v = &built.report.validation;
    println!("\n{}, {key}: {} triangles, watertight {}, boundary {}, non-manifold {}, shells {}", p.shape.name(), v.triangle_count, v.watertight, v.boundary_edges, v.non_manifold_edges, shells(&built.mesh));
    println!("parts: joined {}, cut {}, references {}, separate {}", built.parts.joined, built.parts.cut, built.parts.references, built.parts.separate);
    for n in &built.parts.notes {
        println!("  note: {n}");
    }
    let m = measure(p, s, &built);
    let [bx, by, bz] = m.bounds;
    println!(
        "bounds {:.3} x {:.3} x {:.3} mm (z {:.3}..{:.3}), volume {:.1} mm³, {:.2} g sterling, {:.2} g 14k",
        bx[1] - bx[0],
        by[1] - by[0],
        bz[1] - bz[0],
        bz[0],
        bz[1],
        m.volume,
        m.volume * SILVER_G_PER_MM3,
        m.volume * GOLD14_G_PER_MM3
    );
    let wall = cad::measure::thickness(&built.mesh, 0.5);
    println!(
        "wall census at 0.5 mm: thinnest {:.3} mm, {} wall samples under ({:.3} mm²), {} edge samples ({:.3} mm²), {} unresolved, {} internal",
        wall.sampled_min_mm.unwrap_or(f64::NAN),
        wall.below_limit,
        wall.wall_area_mm2,
        wall.edge_below_limit,
        wall.edge_area_mm2,
        wall.unresolved,
        wall.internal
    );
    for z in wall.walls.iter().chain(wall.edges.iter().take(4)) {
        println!("  thin {:?}: {:.3} mm² down to {:.3} mm at ({:.2}, {:.2}, {:.2})", z.kind, z.area_mm2, z.thinnest_mm, z.point[0], z.point[1] - AWAY, z.point[2]);
    }
    println!("stone {:.1} mm: seat face z {:.3}, girdle z {:.3}, table z {:.3}, culet z {:.3}", p.stone, p.seat_z(s), m.girdle_z, m.table_z, m.culet_z);
    for (c, dia) in &m.claws {
        println!("  claw at ({:+.3}, {:+.3}), r {:.3} from the centre, Ø{dia:.3} at its foot", c[0], c[1], c[0].hypot(c[1]));
    }
    let st = star(p)?;
    let hx = st.hole_x(p);
    let hw = st.half_width_at(hx);
    println!("{} {:.3} along the bracelet by {:.3} across", p.shape.key(), 2.0 * st.tip_end, 2.0 * st.north_end);
    println!(
        "cross holes Ø{:.2} at x ±{hx:.3} ({:.3} apart), {:.2} up; the point is {:.3} wide there; {:.2} of metal over and under each, {:.2} to the tip",
        2.0 * p.hole_r,
        2.0 * hx,
        p.hole_z(),
        2.0 * hw,
        p.hole_z() - p.hole_r,
        p.hole_inset - p.hole_r
    );
    match s {
        Setting::Gypsy => println!(
            "open back {:.2} mm across: skin bulging a full hemisphere into it reaches {:.2} mm, {:.2} mm short of the culet",
            2.0 * p.back_hole,
            p.back_hole,
            m.culet_z - p.back_hole
        ),
        _ => println!("closed back: the culet stands {:.2} mm over the underside", m.culet_z),
    }
    ensure!(v.watertight, "the {key} link is not closed");

    let pattern = moved(&mesh::try_build_pattern(&d, lib, params)?.mesh, [0.0, -AWAY, 0.0]);
    stl::write_stl(out.join(format!("{stem}-{key}.stl")), &pattern, &d.name)?;
    std::fs::write(out.join(format!("{stem}-{key}.3mf")), threemf::to_3mf(&pattern, &d.name, &format!("{} mm stone", p.stone)))?;
    library::save_design(out.join(format!("{stem}-{key}.ring.json")), &d)?;
    let len = lengths(p, &st, b);
    let per_tenth = length_at(p, b, p.stretch + 0.05)? - length_at(p, b, p.stretch - 0.05)?;
    println!(
        "bracelet of {} links: {:.3} link to link, {:.2} tip to tip, {:.2} with the end rings, {:.2} end to end with a {} mm clasp; {per_tenth:.2} mm per 0.1 mm of stretch",
        b.links,
        len.pitch,
        len.tips,
        len.rings,
        len.length,
        b.clasp
    );
    if s == Setting::Prong {
        std::fs::write(out.join(format!("{stem}-{key}-dimensions.svg")), drawing(p, s, &m, &st, &elevation(&pattern, 0.05), &len, b))?;
        std::fs::write(out.join(format!("{stem}-{key}-bracelet.svg")), bracelet_drawing(p, &st, b, &len, per_tenth))?;
    }

    let fin = render::finished_from(&d, lib, built);
    let metal = moved(&fin.metal, [0.0, -AWAY, 0.0]);
    let stones: Vec<(Mesh, [f32; 3])> = fin.stones.iter().map(|(m, t)| (moved(m, [0.0, -AWAY, 0.0]), *t)).collect();
    for (name, yaw, pitch) in [("hero", -0.55, -0.8), ("front", 0.0, -0.75), ("top", 0.0, 0.0), ("side", 0.0, -1.45), ("back", 0.0, PI)] {
        render_set(&out.join(format!("{stem}-{key}-{name}.png")), &metal, &stones, yaw, pitch, 900)?;
    }
    // The east point close up, from the stone to the tip.
    let parts: Vec<render::Part<'_>> = std::iter::once(render::Part::metal(&metal, SILVER)).chain(stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t))).collect();
    let framing = render::Framing::new([0.6 * st.tip_end, 0.0, 1.0], 0.42 * st.tip_end);
    render::write_png_framed(out.join(format!("{stem}-{key}-east-point.png")), &parts, -0.35, -1.0, framing, 900)?;
    // Three links on two flat jump rings through their cross holes, pulled straight.
    let pitch_x = len.pitch;
    let mut chain: Vec<Mesh> = (-1..=1).map(|i| moved(&metal, [i as f64 * pitch_x, 0.0, 0.0])).collect();
    for i in [-0.5, 0.5] {
        chain.push(jump_ring([i * pitch_x, 0.0, p.hole_z()], p.jump_major, p.jump_minor));
    }
    let chain_stones: Vec<(Mesh, [f32; 3])> = (-1..=1).flat_map(|i| stones.iter().map(move |(m, t)| (moved(m, [i as f64 * pitch_x, 0.0, 0.0]), *t))).collect();
    let chain_metal = merged(&chain);
    render_set(&out.join(format!("{stem}-{key}-chain.png")), &chain_metal, &chain_stones, -0.25, -0.75, 1200)?;
    render_set(&out.join(format!("{stem}-{key}-chain-top.png")), &chain_metal, &chain_stones, 0.0, 0.0, 1200)?;
    // The whole bracelet's links and rings, laid straight, from above.
    let mid = 0.5 * (b.links as f64 - 1.0);
    let mut row: Vec<Mesh> = (0..b.links).map(|i| moved(&metal, [(i as f64 - mid) * len.pitch, 0.0, 0.0])).collect();
    row.extend((0..=b.links).map(|i| jump_ring([(i as f64 - mid - 0.5) * len.pitch, 0.0, p.hole_z()], p.jump_major, p.jump_minor)));
    let row_stones: Vec<(Mesh, [f32; 3])> = (0..b.links)
        .flat_map(|i| {
            let x = (i as f64 - mid) * len.pitch;
            stones.iter().map(move |(m, t)| (moved(m, [x, 0.0, 0.0]), *t))
        })
        .collect();
    let row_metal = merged(&row);
    let parts: Vec<render::Part<'_>> = std::iter::once(render::Part::metal(&row_metal, SILVER)).chain(row_stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t))).collect();
    let (w, h) = (3200usize, 440usize);
    let img = render::render_parts_ss(&parts, 0.0, 0.0, w, h, 3);
    image::save_buffer(out.join(format!("{stem}-{key}-bracelet-top.png")), &img, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(Link { setting: s, metal, stones })
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut out = None;
    let (mut shape, mut stone) = (Shape::Sparkle, None);
    let mut bracelet = Bracelet::default();
    let mut stretch = None;
    let mut settings = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--shape" => shape = Shape::parse(&args.next().context("--shape needs a name")?)?,
            "--stone" => stone = Some(args.next().context("--stone needs mm")?.parse()?),
            "--setting" => settings.push(Setting::parse(&args.next().context("--setting needs a name")?)?),
            "--links" => bracelet.links = args.next().context("--links needs a count")?.parse()?,
            "--bracelet" => bracelet.length = args.next().context("--bracelet needs mm")?.parse()?,
            "--clasp" => bracelet.clasp = args.next().context("--clasp needs mm")?.parse()?,
            "--stretch" => stretch = Some(args.next().context("--stretch needs mm")?.parse()?),
            flag if flag.starts_with("--") => bail!("unknown option {flag}"),
            other => out = Some(PathBuf::from(other)),
        }
    }
    let out = out.unwrap_or_else(|| PathBuf::from(format!("target/{}", shape.stem())));
    let mut p = Params::for_shape(shape);
    p.stone = stone.unwrap_or(p.stone);
    if settings.is_empty() {
        settings.push(Setting::Prong);
    }
    ensure!(bracelet.links >= 1, "a bracelet needs a link");
    p.stretch = match stretch {
        Some(s) => s,
        None => solve_stretch(&p, &bracelet)?,
    };
    println!(
        "{} {:+.4} mm, reaching {:.4}: {} links with a {} mm clasp come to {:.3} mm (asked {})",
        p.shape.ring_points(),
        p.stretch,
        p.tip + p.stretch,
        bracelet.links,
        bracelet.clasp,
        length_at(&p, &bracelet, p.stretch)?,
        bracelet.length
    );
    std::fs::create_dir_all(&out)?;
    let lib = AlphaLibrary::builtin();
    let params = BuildParams::default();
    let links: Vec<Link> = settings.iter().map(|s| make(&p, &bracelet, *s, &lib, params, &out)).collect::<Result<_>>()?;
    if links.len() > 1 {
        // Every link side by side, in the order asked.
        let step = 2.0 * (p.tip + p.stretch) + 3.0;
        let mid = 0.5 * (links.len() - 1) as f64;
        let metal = merged(&links.iter().enumerate().map(|(i, l)| moved(&l.metal, [(i as f64 - mid) * step, 0.0, 0.0])).collect::<Vec<_>>());
        let stones: Vec<(Mesh, [f32; 3])> =
            links.iter().enumerate().flat_map(|(i, l)| l.stones.iter().map(move |(m, t)| (moved(m, [(i as f64 - mid) * step, 0.0, 0.0]), *t))).collect();
        let names: Vec<&str> = links.iter().map(|l| l.setting.key()).collect();
        render_set(&out.join(format!("{}-compare-top.png", p.shape.stem())), &metal, &stones, 0.0, 0.0, 1500)?;
        render_set(&out.join(format!("{}-compare-hero.png", p.shape.stem())), &metal, &stones, -0.2, -0.85, 1500)?;
        println!("\ncompare, left to right: {}", names.join(", "));
    }
    println!("wrote {}", out.display());
    Ok(())
}
