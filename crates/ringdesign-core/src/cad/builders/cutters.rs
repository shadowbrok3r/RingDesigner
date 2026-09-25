//! Piercings, azure windows and cathedral shoulders, each made in its own frame and reading the band through [`Bore`].
use super::{AZURE, BASKET, BEZEL, BEZEL_SINK_MM, BUR, Bore, CATHEDRAL, CLAW, HEAD, Made, PIERCE, SPLIT, WINDOW, Seat, Values, build, component, gem_label, head_param, label, under_wall, walled_collet};
use crate::RingDesign;
use crate::cad::{Component, Feature, Operation, Placement, Stage};
use crate::csg::{P3, Solid};
use crate::gem::{Gem, GemCut};
use crate::mesh::MIN_WALL_MM;
use crate::profile::MIN_EDGE_MM;
use crate::setting::{self, Floor, Named, Plan, Rails, Wall};
use crate::sketch::Id;
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value as Json, json};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// The plans a piercing takes, by the names its parameters carry.
pub const PIERCE_SHAPES: &[&str] = &["Round", "Oval", "Marquise", "Heart", "Drop"];
/// The plans an azure window takes.
pub const AZURE_SHAPES: &[&str] = &["Round", "Teardrop"];
/// How far round the ring either side of the stone cathedral shoulders land, degrees, when the part does not say.
pub const SPREAD_DEG: f64 = 32.0;
/// How soon an arch lifts off the band when the part does not say: its height grows as its share of the way to the power `1.6 / (1 + rise)`.
pub const RISE: f64 = 0.6;
/// A cut whose axis lies within this of the pull stands its walls along it, degrees.
pub const ALONG_PULL_DEG: f64 = 10.0;
/// Spacing of an outline's points along its edge, mm.
const SAMPLE_MM: f64 = 0.05;
/// How far a cutter's top stands clear of the surface it opens, mm.
const LIFT_MM: f64 = 0.3;
/// How far a through cutter runs on past the metal, mm.
const PAST_MM: f64 = 0.5;
/// How far above its frame's origin a probe down the axis starts, mm.
const PROBE_ABOVE_MM: f64 = 8.0;
/// How far a probe down the axis looks, mm.
const PROBE_REACH_MM: f64 = 30.0;
/// Lobe centres off a heart's axis, as a share of the lobe's radius: how deep its cleft cuts.
const HEART_CLEFT: f64 = 0.55;
/// A teardrop window's length over its width.
const TEARDROP: f64 = 1.6;
/// The smallest window the fit settles for, mm.
const WINDOW_MIN_MM: f64 = 0.3;
/// Steps the fit takes through window sizes and radii, mm.
const FIT_STEP_MM: f64 = 0.04;
/// How far an arch's foot sinks under the band's surface, in its wire's radii.
const FOOT_SINK: f64 = 0.9;
/// Spacing of an arch's rings along its path, mm.
const ARCH_STEP_MM: f64 = 0.15;
/// How far along its path, in hundredths, an arch has left the band for good.
const EMERGE: usize = 50;
/// What a window reaching off the band meets.
const EDGE: &str = "the band's edge";
/// How far up a collet's outer wall an arch meets it, as a share of the wall.
const COLLET_MEET: f64 = 0.7;
/// Sides round an arch's wire, a multiple of four so a ring stands a point on its plane either side.
const WIRE_SIDES: usize = 16;
/// Bearings round a stone's axis along which the band's edge is found.
const EDGE_BEARINGS: usize = 180;

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
fn unit(a: P3) -> P3 {
    let l = norm(a);
    if l > 1e-300 { scale(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}
/// `p` turned `t` radians anticlockwise about the origin.
fn turned(p: [f64; 2], t: f64) -> [f64; 2] {
    let (s, c) = t.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}
/// Twice the signed area of `a b c`: positive anticlockwise.
fn cross2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
}
/// A point of a frame in world millimetres.
fn world(f: &cadkernel::brep::Placement, p: P3) -> P3 {
    f.point(p)
}
/// A world point in a frame's own axes.
fn local(f: &cadkernel::brep::Placement, w: P3) -> P3 {
    let d = sub(w, f.origin);
    [dot(d, f.x_axis), dot(d, f.y_axis), dot(d, f.z_axis)]
}

/// A piercing's or a window's plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Round,
    Oval,
    Marquise,
    Heart,
    Drop,
}

impl Shape {
    pub const ALL: [Shape; 5] = [Shape::Round, Shape::Oval, Shape::Marquise, Shape::Heart, Shape::Drop];

    /// The shape a name in the parameters stands for; a window's `Teardrop` is the drop.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "Round" => Self::Round,
            "Oval" => Self::Oval,
            "Marquise" => Self::Marquise,
            "Heart" => Self::Heart,
            "Drop" | "Teardrop" => Self::Drop,
            _ => return None,
        })
    }

    /// Its name in a piercing's parameters.
    pub fn name(self) -> &'static str {
        match self {
            Self::Round => "Round",
            Self::Oval => "Oval",
            Self::Marquise => "Marquise",
            Self::Heart => "Heart",
            Self::Drop => "Drop",
        }
    }

    /// The length along x and width across it the outline takes: a round as long as wide, a drop longer, a heart near square.
    pub fn sized(self, l: f64, w: f64) -> (f64, f64) {
        match self {
            Self::Round => (w, w),
            Self::Heart => (l.max(0.72 * w), w),
            Self::Drop => (l.max(1.05 * w), w),
            Self::Oval | Self::Marquise => (l, w),
        }
    }
}

/// The outline of `shape`, `l` along x by `w`, grown `g` outward: anticlockwise, one point count at every `g`, star-shaped about [`centre`], a point at −x.
pub fn outline(shape: Shape, l: f64, w: f64, g: f64) -> Vec<[f64; 2]> {
    let (l, w) = shape.sized(l, w);
    match shape {
        Shape::Round | Shape::Oval | Shape::Marquise => {
            let pow = if shape == Shape::Marquise { GemCut::Marquise.plan_pow() } else { GemCut::Oval.plan_pow() };
            let plan = Plan { a: 0.5 * l, b: 0.5 * w, pow };
            let n = ((plan.perimeter() / SAMPLE_MM).round() as usize).clamp(48, 192);
            (0..n)
                .map(|i| {
                    let phi = TAU * i as f64 / n as f64;
                    let (p, q) = (plan.point(phi), plan.normal(phi));
                    [p[0] + q[0] * g, p[1] + q[1] * g]
                })
                .collect()
        }
        Shape::Drop => drop_outline(l, w, g),
        Shape::Heart => heart_outline(l, w, g),
    }
}

/// The point every outline of `shape`, `l` by `w`, is star-shaped about.
pub fn centre(shape: Shape, l: f64, w: f64) -> [f64; 2] {
    let (l, w) = shape.sized(l, w);
    match shape {
        Shape::Drop => [0.5 * l - 0.5 * w, 0.0],
        Shape::Heart => [0.5 * l - 0.5 * w / (1.0 + HEART_CLEFT), 0.0],
        _ => [0.0, 0.0],
    }
}

/// A polygon's area, positive anticlockwise.
pub fn area(poly: &[[f64; 2]]) -> f64 {
    0.5 * (0..poly.len()).map(|i| cross2([0.0, 0.0], poly[i], poly[(i + 1) % poly.len()])).sum::<f64>()
}

/// A circle `w` across and its two tangents meeting `l` from its far side, grown by the radius and the tangents moved out parallel.
fn drop_outline(l: f64, w: f64, g: f64) -> Vec<[f64; 2]> {
    let q = 0.5 * w;
    let (cx, tip) = (0.5 * l - q, -0.5 * l);
    let beta = (q / (cx - tip)).clamp(-1.0, 1.0).asin();
    let a0 = FRAC_PI_2 + beta;
    let (r, t) = (q + g, tip - g / beta.sin().max(1e-6));
    let arc = ((2.0 * a0 * q / SAMPLE_MM).round() as usize).clamp(24, 160);
    let side = (((cx - tip).powi(2) - q * q).max(0.0).sqrt() / SAMPLE_MM).round().clamp(4.0, 80.0) as usize;
    let mut out = Vec::with_capacity(arc + 2 * side);
    for i in 0..=arc {
        let a = -a0 + 2.0 * a0 * i as f64 / arc as f64;
        out.push([cx + r * a.cos(), r * a.sin()]);
    }
    let upper = [cx + r * a0.cos(), r * a0.sin()];
    for i in 1..=side {
        let s = i as f64 / side as f64;
        out.push([upper[0] + (t - upper[0]) * s, upper[1] * (1.0 - s)]);
    }
    for i in 1..side {
        let s = i as f64 / side as f64;
        out.push([t + (upper[0] - t) * s, -upper[1] * s]);
    }
    out
}

/// Two overlapping lobes and the lines from their outer sides to the point, grown by the lobes' radii and the lines moved out parallel.
fn heart_outline(l: f64, w: f64, g: f64) -> Vec<[f64; 2]> {
    let rho = 0.5 * w / (1.0 + HEART_CLEFT);
    let yl = HEART_CLEFT * rho;
    let (xl, tip) = (0.5 * l - rho, -0.5 * l);
    let (dx, dy) = (tip - xl, -yl);
    // The upper line's outward normal, along the upper lobe's radius at its tangent point.
    let psi = (dy.atan2(dx) - (rho / dx.hypot(dy)).clamp(-1.0, 1.0).acos()).rem_euclid(TAU);
    let n = [psi.cos(), psi.sin()];
    let r = rho + g;
    let t = xl + (r + n[1] * yl) / n[0].min(-1e-6);
    let cleft = xl + (r * r - yl * yl).max(0.0).sqrt();
    let ak = (-yl).atan2(cleft - xl);
    let base_arc = psi - (-yl).atan2((rho * rho - yl * yl).max(0.0).sqrt());
    let arc = ((base_arc * rho / SAMPLE_MM).round() as usize).clamp(24, 160);
    let pu = [xl + r * n[0], yl + r * n[1]];
    let side = (((xl + rho * n[0]) - tip).hypot(yl + rho * n[1]) / SAMPLE_MM).round().clamp(4.0, 80.0) as usize;
    let mut out = Vec::with_capacity(2 * arc + 2 * side);
    for i in 0..=arc {
        let a = ak + (psi - ak) * i as f64 / arc as f64;
        out.push([xl + r * a.cos(), yl + r * a.sin()]);
    }
    for i in 1..=side {
        let s = i as f64 / side as f64;
        out.push([pu[0] + (t - pu[0]) * s, pu[1] * (1.0 - s)]);
    }
    for i in 1..side {
        let s = i as f64 / side as f64;
        out.push([t + (pu[0] - t) * s, -pu[1] * s]);
    }
    for i in 0..arc {
        let a = -psi + (psi - ak) * i as f64 / arc as f64;
        out.push([xl + r * a.cos(), -yl + r * a.sin()]);
    }
    out
}

/// Rings of an outline lofted from the top down into a solid whose band `k` is called `bands[k]`, its top fan `ends[0]` and its bottom `ends[1]`.
fn prism(rings: Vec<Vec<P3>>, centre: [f64; 2], bands: &[&str], ends: [&str; 2]) -> Named {
    let mean = |r: &[P3]| r.iter().map(|p| p[2]).sum::<f64>() / r.len().max(1) as f64;
    let top = [centre[0], centre[1], mean(&rings[0])];
    let bottom = [centre[0], centre[1], mean(&rings[rings.len() - 1])];
    let (solid, seg) = setting::loft_traced(&rings, top, bottom);
    let wanted: Vec<&str> = bands.iter().copied().chain(ends).collect();
    let mut names: Vec<String> = Vec::new();
    let patch = seg
        .iter()
        .map(|s| {
            let name = wanted.get(*s as usize).copied().unwrap_or("Wall");
            match names.iter().position(|n| n == name) {
                Some(i) => i as u32,
                None => {
                    names.push(name.into());
                    (names.len() - 1) as u32
                }
            }
        })
        .collect();
    Named { solid, patch, names }
}

/// The ring of `plan` at the heights `z` gives each point.
fn ring(plan: &[[f64; 2]], z: impl Fn(usize) -> f64) -> Vec<P3> {
    plan.iter().enumerate().map(|(i, p)| [p[0], p[1], z(i)]).collect()
}

/// The first run of metal down the frame's axis under plan point `p`: the surface's height and where the metal ends, in the frame.
fn down(bore: &Bore, p: [f64; 2]) -> Option<(f64, f64)> {
    bore.first_metal([p[0], p[1], PROBE_ABOVE_MM], [0.0, 0.0, -1.0], 0.0, PROBE_REACH_MM).map(|(i, o)| (PROBE_ABOVE_MM - i, PROBE_ABOVE_MM - o))
}

/// The band behind a builder's bore, else its refusal said with `who`.
fn band<'b, 'a>(bore: Option<&'b Bore<'a>>, who: &str) -> Result<&'b Bore<'a>> {
    bore.filter(|b| b.design().is_some()).ok_or_else(|| anyhow!("{who}: there is no procedural band to work, this ring is its parts alone"))
}

/// Refused for breaking through the band's edge.
fn off_the_edge(who: &str) -> anyhow::Error {
    anyhow!("{who} would break through the band's edge: keep it {MIN_EDGE_MM} mm inside the band, or make it smaller")
}

/// A cutter in its builder frame with outward faces and one named wall patch.
fn named_cutter(mut solid: Solid, bore: &Bore) -> Named {
    if solid.volume() < 0.0 {
        for f in &mut solid.f { f.swap(1, 2); }
    }
    solid.v.iter_mut().for_each(|p| *p = local(bore.frame(), *p));
    Named { patch: vec![0; solid.f.len()], solid, names: vec!["Wall".into()] }
}

/// A radial section loft with shared vertices where either end closes to a line.
fn split_loft(rings: &[Vec<P3>]) -> Solid {
    let mut solid = Solid::default();
    let mut index = std::collections::BTreeMap::new();
    let mut ids = Vec::new();
    for ring in rings {
        ids.push(ring.iter().map(|&p| {
            let key = p.map(|x| if x == 0.0 { 0 } else { x.to_bits() });
            *index.entry(key).or_insert_with(|| {
                let id = solid.v.len() as u32;
                solid.v.push(p);
                id
            })
        }).collect::<Vec<_>>());
    }
    let mut face = |f: [u32; 3]| {
        if f[0] != f[1] && f[1] != f[2] && f[2] != f[0] { solid.f.push(f); }
    };
    for pair in ids.windows(2) {
        for i in 0..pair[0].len() {
            let j = (i + 1) % pair[0].len();
            face([pair[0][i], pair[1][i], pair[1][j]]);
            face([pair[0][i], pair[1][j], pair[0][j]]);
        }
    }
    solid
}

/// The slot follows each actual section and leaves a named minimum rail on both sides.
pub(super) fn split(v: &Values, bore: Option<&Bore>) -> Result<Named> {
    let bore = band(bore, label(SPLIT))?;
    let (centre, spread, gap) = (v.f("theta_deg"), v.f("spread_deg"), v.f("gap_mm"));
    let steps = (spread * 2.0).ceil() as usize;
    let mut rings = Vec::with_capacity(steps + 1);
    for k in 0..=steps {
        let x = 2.0 * k as f64 / steps as f64 - 1.0;
        let theta = centre + spread * x;
        let t = 1.0 - x.abs();
        let share = if v.s("tip") == "Round" { (1.0 - x * x).max(0.0).sqrt() } else { t * t * t * (t * (6.0 * t - 15.0) + 10.0) };
        let half = gap * 0.5 * share;
        let (lo, hi) = bore.z_extent(theta).ok_or_else(|| off_the_edge(label(SPLIT)))?;
        let rail = (hi - half).min(-half - lo);
        let rounding = v.f("rail_round_mm") * share;
        ensure!(rail - rounding >= MIN_EDGE_MM, "Split at {theta:.1}° leaves a {:.2} mm rail after rounding, below the {MIN_EDGE_MM} mm edge; narrow the gap or widen this station", rail - rounding);
        let section = bore.section(theta).ok_or_else(|| off_the_edge(label(SPLIT)))?;
        let r0 = section.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) - 0.8;
        let r1 = section.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max) + 0.8;
        let (sin, cos) = theta.to_radians().sin_cos();
        let side = |sign: f64| -> Result<Vec<P3>> {
            let crossing = bore.crossings(theta, sign * half);
            ensure!(crossing.len() == 2, "Split at {theta:.1}° needs one solid band section");
            let (ri, ro) = (crossing[0], crossing[1]);
            let radius = rounding.min((ro - ri) * 0.45);
            let mut profile = vec![(r0, half + radius)];
            for k in 0..=6 {
                let a = FRAC_PI_2 * k as f64 / 6.0;
                profile.push((ri + radius * (1.0 - a.cos()), half + radius * (1.0 - a.sin())));
            }
            for k in 0..=6 {
                let a = FRAC_PI_2 * k as f64 / 6.0;
                profile.push((ro - radius * (1.0 - a.sin()), half + radius * (1.0 - a.cos())));
            }
            profile.push((r1, half + radius));
            Ok(profile.into_iter().map(|(r, z)| [r * cos, r * sin, sign * z]).collect())
        };
        let mut ring = side(-1.0)?;
        ring.extend(side(1.0)?.into_iter().rev());
        rings.push(ring);
    }
    Ok(named_cutter(split_loft(&rings), bore))
}

/// A parallel polygon offset, with its vertices retained for matched lofts.
fn offset_window(plan: &[[f64; 2]], grow: f64) -> Result<Vec<[f64; 2]>> {
    let n = plan.len();
    (0..n).map(|i| {
        let p = plan[i];
        let normal = |q: [f64; 2], r: [f64; 2]| {
            let d = [r[0] - q[0], r[1] - q[1]];
            let len = d[0].hypot(d[1]);
            [d[1] / len, -d[0] / len]
        };
        let (a, b) = (normal(plan[(i + n - 1) % n], p), normal(p, plan[(i + 1) % n]));
        let den = 1.0 + a[0] * b[0] + a[1] * b[1];
        ensure!(den.is_finite() && den > 0.01, "Gallery window turns too sharply at a tip; shorten its arc or reduce the rails");
        Ok([p[0] + grow * (a[0] + b[0]) / den, p[1] + grow * (a[1] + b[1]) / den])
    }).collect()
}

/// Two overlapping drafted halves share their exterior, with no coincident internal caps.
pub(super) fn window(v: &Values, bore: Option<&Bore>) -> Result<Named> {
    let bore = band(bore, label(WINDOW))?;
    let start = v.f("from_deg");
    let span = (v.f("to_deg") - start).rem_euclid(360.0);
    ensure!(span >= 2.0 && span <= 300.0, "Gallery window needs an arc between 2° and 300°");
    let steps = (span * 2.0).ceil() as usize;
    let mut inner = Vec::with_capacity(steps + 1);
    let mut outer = Vec::with_capacity(steps + 1);
    let mut half_z: f64 = 0.0;
    for k in 0..=steps {
        let theta = start + span * k as f64 / steps as f64;
        let crossings = bore.crossings(theta, 0.0);
        ensure!(crossings.len() == 2, "Gallery window at {theta:.1}° needs one solid band section");
        let r0 = crossings[0] + v.f("rail_in_mm");
        let r1 = crossings[1] - v.f("rail_out_mm");
        ensure!(r1 - r0 >= 0.05, "Gallery window at {theta:.1}° has no room between its rails; thicken this station or reduce the rails");
        let radius = v.f("tip_round_mm").min((r1 - r0) * 0.45);
        let along = (theta - start).min(start + span - theta).to_radians() * (r0 + r1) * 0.5;
        let trim = if along < radius { radius - (2.0 * radius * along - along * along).max(0.0).sqrt() } else { 0.0 };
        let (sin, cos) = theta.to_radians().sin_cos();
        inner.push([(r0 + trim) * cos, (r0 + trim) * sin]);
        outer.push([(r1 - trim) * cos, (r1 - trim) * sin]);
        let (lo, hi) = bore.z_extent(theta).ok_or_else(|| off_the_edge(label(WINDOW)))?;
        half_z = half_z.max(lo.abs()).max(hi.abs());
    }
    let m = outer.len();
    let plan: Vec<[f64; 2]> = outer.into_iter().chain(inner.into_iter().rev()).collect();
    let n = plan.len();
    let height = half_z + PAST_MM;
    let mut solid = Solid::default();
    for z in [height, 0.0, -height] {
        let outline = offset_window(&plan, (z.abs() + 0.05) * v.f("draft_deg").to_radians().tan())?;
        solid.v.extend(outline.iter().map(|p| [p[0], p[1], z]));
    }
    let at = |layer: usize, i: usize| (layer * n + i % n) as u32;
    for layer in 0..2 {
        for i in 0..n {
            solid.f.extend([[at(layer, i), at(layer + 1, i), at(layer + 1, i + 1)], [at(layer, i), at(layer + 1, i + 1), at(layer, i + 1)]]);
        }
    }
    for (layer, reverse) in [(0, false), (2, true)] {
        for i in 0..m - 1 {
            for mut f in [[at(layer, i), at(layer, i + 1), at(layer, n - 2 - i)], [at(layer, i), at(layer, n - 2 - i), at(layer, n - 1 - i)]] {
                if reverse { f.swap(1, 2); }
                solid.f.push(f);
            }
        }
    }
    Ok(named_cutter(solid, bore))
}

/// A piercing down its frame's axis from over the surface to past the metal or to its depth, a 45° bright cut at the rim.
pub(super) fn pierce(v: &Values, bore: Option<&Bore>) -> Result<Named> {
    let who = label(PIERCE);
    let bore = band(bore, who)?;
    let shape = Shape::named(v.s("shape")).unwrap_or(Shape::Round);
    let (l, w) = shape.sized(v.f("length_mm"), v.f("width_mm"));
    let turn = v.f("turn_deg").to_radians();
    let chamfer = v.f("chamfer_mm");
    let through = v.b("through");
    let plan = |g: f64| -> Vec<[f64; 2]> { outline(shape, l, w, g).into_iter().map(|p| turned(p, turn)).collect() };
    let base = plan(0.0);
    let spans: Vec<(f64, f64)> = base.iter().map(|p| down(bore, *p)).collect::<Option<_>>().ok_or_else(|| off_the_edge(who))?;
    // The outline grown by the least edge the sand fills stands over metal.
    if plan(MIN_EDGE_MM).iter().any(|p| down(bore, *p).is_none()) {
        return Err(off_the_edge(who));
    }
    let c = turned(centre(shape, l, w), turn);
    let (entry, exit) = down(bore, c).ok_or_else(|| off_the_edge(who))?;
    let lowest_rim = spans.iter().map(|s| s.0 - chamfer).fold(f64::INFINITY, f64::min);
    let floor = if through {
        spans.iter().map(|s| s.1).fold(f64::INFINITY, f64::min) - PAST_MM
    } else {
        let depth = v.f("depth_mm");
        let z = (entry - depth).min(lowest_rim - 0.05);
        let left = z - exit;
        ensure!(left > 0.0, "{who} {depth} mm deep runs through the metal here: make it a through cut, or shallower");
        ensure!(left >= MIN_WALL_MM, "{who} would leave {left:.2} mm of metal under its floor, under the {MIN_WALL_MM} mm a wall keeps: make it a through cut, or shallower");
        z
    };
    let mut rings = Vec::new();
    let mut bands = Vec::new();
    if chamfer > 0.0 {
        rings.push(ring(&plan(chamfer + LIFT_MM), |i| spans[i].0 + LIFT_MM));
        rings.push(ring(&base, |i| spans[i].0 - chamfer));
        bands.push("Bright cut");
    } else {
        rings.push(ring(&base, |i| spans[i].0 + LIFT_MM));
    }
    rings.push(ring(&base, |_| floor));
    bands.push("Wall");
    Ok(prism(rings, c, &bands, ["Clearance", if through { "Clearance" } else { "Floor" }]))
}

/// A stone's head rebuilt in the stone's frame from its own parameters: its claws and rails, or its collet.
struct Head {
    name: String,
    key: String,
    parts: Vec<Named>,
    /// Where its claws stand round the stone's axis, radians; none on a collet.
    claws: Vec<f64>,
}

/// The head `params` names under [`HEAD`], or `None` when they name none; refused by name when the feature is gone, off, not a head, or round another stone.
fn head_in(bore: &Bore, params: &Json, gem: Gem, seat: Seat, floor: Option<Floor>, wall: Option<Wall>, who: &str) -> Result<Option<Head>> {
    let Some(id) = head_param(params) else { return Ok(None) };
    let doc = bore.design().and_then(|d| d.cad.as_ref()).ok_or_else(|| anyhow!("{who}: head #{id} is not in the document"))?;
    let f = doc.feature(id).ok_or_else(|| anyhow!("{who}: head #{id} is not in the document"))?;
    let named = format!("#{id} {}", f.name);
    ensure!(f.enabled, "{who}: {named} is suppressed");
    let Operation::Builder { key, on, params: own } = &f.operation else { bail!("{who}: {named} is not a head") };
    ensure!(matches!(key.as_str(), CLAW | BASKET | BEZEL), "{who}: {named} is a {}, not a head", label(key).to_lowercase());
    // Refused when the head's own stone is another size or cut.
    if let Some(Operation::Builder { params: stone, .. }) = on.and_then(|s| doc.feature(s)).map(|s| &s.operation) {
        let theirs = super::gem_of(stone)?;
        ensure!((theirs.w_mm - gem.w_mm).abs() < 1e-9 && (theirs.l_mm - gem.l_mm).abs() < 1e-9 && theirs.cut == gem.cut, "{who}: {named} stands on another stone");
    }
    let v = Values::of(key, gem, own)?;
    let (parts, claws) = match key.as_str() {
        CLAW | BASKET => {
            let rails = if key == CLAW { Rails::Seat } else { Rails::Basket(v.n("rails")) };
            let count = setting::claw_count(gem, v.n("prongs"));
            (setting::claw_parts_reach(gem, v.n("prongs"), v.f("wire_mm"), rails, floor, wall).0, Plan::of(gem).claw_angles(count))
        }
        _ => {
            let metal = floor.and_then(|f| under_wall(gem, v.f("wall_mm"), f)).unwrap_or(seat.surface_z).min(seat.surface_z);
            let collet = match wall {
                Some(w) => walled_collet(gem, v.f("wall_mm"), v.f("lip"), metal - BEZEL_SINK_MM, w).map_err(|e| anyhow!("{who}: {named} {e}"))?,
                None => setting::collet_named(gem, v.f("wall_mm"), v.f("lip"), metal - BEZEL_SINK_MM),
            };
            (vec![collet], Vec::new())
        }
    };
    Ok(Some(Head { name: f.name.clone(), key: key.clone(), parts, claws }))
}

/// Cells of the stone's plan, each free or held by a named obstacle, with each cell's distance to the nearest held one, mm.
struct Room {
    lo: f64,
    cell: f64,
    n: usize,
    held: Vec<u16>,
    dist: Vec<f64>,
    names: Vec<String>,
}

impl Room {
    /// A square of cells `radius` either side of the axis, at most 400 across.
    fn new(radius: f64) -> Self {
        let cell = (2.0 * radius / 400.0).max(0.02);
        let n = (2.0 * radius / cell).ceil() as usize + 1;
        Self { lo: -radius, cell, n, held: vec![0; n * n], dist: Vec::new(), names: Vec::new() }
    }

    fn at(&self, i: usize, j: usize) -> [f64; 2] {
        [self.lo + (i as f64 + 0.5) * self.cell, self.lo + (j as f64 + 0.5) * self.cell]
    }

    fn index(&self, p: [f64; 2]) -> Option<usize> {
        let (i, j) = (((p[0] - self.lo) / self.cell).floor(), ((p[1] - self.lo) / self.cell).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.n && (j as usize) < self.n).then(|| j as usize * self.n + i as usize)
    }

    fn label(&mut self, name: &str) -> u16 {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i as u16 + 1,
            None => {
                self.names.push(name.to_string());
                self.names.len() as u16
            }
        }
    }

    /// Hold every free cell whose line along the axis is inside `solid` between `z0` and `z1`, entering at faces turned down.
    fn solid(&mut self, solid: &Solid, z0: f64, z1: f64, name: &str) {
        let id = self.label(name);
        let mut hits: Vec<(u32, f64, bool)> = Vec::new();
        let jitter = [1.3e-7, 2.9e-7];
        for f in &solid.f {
            let [a, b, c] = f.map(|i| solid.v[i as usize]);
            let (a2, b2, c2) = ([a[0], a[1]], [b[0], b[1]], [c[0], c[1]]);
            let whole = cross2(a2, b2, c2);
            if whole.abs() < 1e-14 {
                continue;
            }
            let span = |k: usize| {
                let (lo, hi) = (a[k].min(b[k]).min(c[k]), a[k].max(b[k]).max(c[k]));
                let from = ((lo - self.lo) / self.cell - 0.5).ceil().max(0.0) as usize;
                let to = ((hi - self.lo) / self.cell - 0.5).floor();
                (from, if to < 0.0 { None } else { Some((to as usize).min(self.n - 1)) })
            };
            let ((i0, Some(i1)), (j0, Some(j1))) = (span(0), span(1)) else { continue };
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let q = self.at(i, j);
                    let q = [q[0] + jitter[0], q[1] + jitter[1]];
                    let (wa, wb, wc) = (cross2(q, b2, c2) / whole, cross2(a2, q, c2) / whole, cross2(a2, b2, q) / whole);
                    if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                        continue;
                    }
                    hits.push(((j * self.n + i) as u32, wa * a[2] + wb * b[2] + wc * c[2], whole > 0.0));
                }
            }
        }
        hits.sort_by(|p, q| p.0.cmp(&q.0).then(p.1.total_cmp(&q.1)));
        for run in hits.chunk_by(|p, q| p.0 == q.0) {
            let (mut depth, mut from, mut held) = (0i32, f64::NEG_INFINITY, false);
            for &(_, z, up) in run {
                if !up {
                    if depth == 0 {
                        from = z;
                    }
                    depth += 1;
                } else {
                    depth -= 1;
                    if depth <= 0 {
                        held |= from <= z1 && z >= z0;
                        depth = 0;
                        from = f64::NEG_INFINITY;
                    }
                }
            }
            held |= depth > 0 && from <= z1;
            let cell = run[0].0 as usize;
            if held && self.held[cell] == 0 {
                self.held[cell] = id;
            }
        }
    }

    /// Hold every free cell whose centre `test` says is in the way.
    fn mark(&mut self, name: &str, test: impl Fn([f64; 2]) -> bool) {
        let id = self.label(name);
        for j in 0..self.n {
            for i in 0..self.n {
                let k = j * self.n + i;
                if self.held[k] == 0 && test(self.at(i, j)) {
                    self.held[k] = id;
                }
            }
        }
    }

    /// Every cell's distance to the nearest held one, by the exact transform along rows then columns.
    fn settle(&mut self) {
        let n = self.n;
        let mut d: Vec<f64> = self.held.iter().map(|h| if *h > 0 { 0.0 } else { f64::INFINITY }).collect();
        let mut line = vec![0.0; n];
        for j in 0..n {
            line.copy_from_slice(&d[j * n..(j + 1) * n]);
            d[j * n..(j + 1) * n].copy_from_slice(&squared_distances(&line));
        }
        for i in 0..n {
            for j in 0..n {
                line[j] = d[j * n + i];
            }
            let out = squared_distances(&line);
            for j in 0..n {
                d[j * n + i] = out[j];
            }
        }
        self.dist = d.into_iter().map(|x| x.sqrt() * self.cell).collect();
    }

    /// How far point `p` stands from the nearest obstacle, less the cell's own half-diagonal; nothing off the grid.
    fn clearance(&self, p: [f64; 2]) -> f64 {
        self.index(p).map_or(0.0, |k| self.dist[k] - 0.71 * self.cell)
    }

    /// The name of the held cell nearest `p` within `reach`.
    fn nearest(&self, p: [f64; 2], reach: f64) -> Option<&str> {
        let span = (reach / self.cell).ceil() as i64 + 1;
        let (ci, cj) = (((p[0] - self.lo) / self.cell).floor() as i64, ((p[1] - self.lo) / self.cell).floor() as i64);
        let mut best: Option<(f64, u16)> = None;
        for j in (cj - span).max(0)..=(cj + span).min(self.n as i64 - 1) {
            for i in (ci - span).max(0)..=(ci + span).min(self.n as i64 - 1) {
                let h = self.held[j as usize * self.n + i as usize];
                if h == 0 {
                    continue;
                }
                let q = self.at(i as usize, j as usize);
                let d = (q[0] - p[0]).hypot(q[1] - p[1]);
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, h));
                }
            }
        }
        best.and_then(|(_, h)| self.names.get(h as usize - 1).map(String::as_str))
    }
}

/// Squared distances along a line of cells to the nearest finite one, by the lower envelope of parabolas.
fn squared_distances(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let (mut v, mut z): (Vec<usize>, Vec<f64>) = (Vec::new(), Vec::new());
    for q in (0..n).filter(|q| f[*q].is_finite()) {
        let fq = f[q] + (q * q) as f64;
        let mut s = f64::NEG_INFINITY;
        while let Some(&p) = v.last() {
            s = (fq - (f[p] + (p * p) as f64)) / (2.0 * (q - p) as f64);
            if z.last().is_some_and(|zl| s <= *zl) {
                v.pop();
                z.pop();
                s = f64::NEG_INFINITY;
            } else {
                break;
            }
        }
        v.push(q);
        z.push(if v.len() == 1 { f64::NEG_INFINITY } else { s });
    }
    let mut out = vec![f64::INFINITY; n];
    if v.is_empty() {
        return out;
    }
    let mut k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while k + 1 < v.len() && z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *o = (q as f64 - p as f64).powi(2) + f[p];
    }
    out
}

/// A window of one shape and size, its round end's centre at the origin and a drop's point toward −x.
struct Pane {
    outline: Vec<[f64; 2]>,
}

impl Pane {
    fn new(shape: Shape, s: f64) -> Self {
        let (l, w) = if shape == Shape::Drop { (TEARDROP * s, s) } else { (s, s) };
        let c = centre(shape, l, w);
        Self { outline: outline(shape, l, w, 0.0).into_iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect() }
    }

    /// Its outline and the point it is star-shaped about, standing `rho` from the axis at bearing `phi`, a drop's point toward the axis.
    fn at(&self, rho: f64, phi: f64) -> (Vec<[f64; 2]>, [f64; 2]) {
        (self.outline.iter().map(|p| turned([p[0] + rho, p[1]], phi)).collect(), turned([rho, 0.0], phi))
    }
}

/// The convex hull of points in the plane, anticlockwise.
fn hull(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let half = |it: &mut dyn Iterator<Item = [f64; 2]>| {
        let mut out: Vec<[f64; 2]> = Vec::new();
        for p in it {
            while out.len() >= 2 && cross2(out[out.len() - 2], out[out.len() - 1], p) <= 0.0 {
                out.pop();
            }
            out.push(p);
        }
        out.pop();
        out
    };
    let mut lower = half(&mut pts.iter().copied());
    lower.extend(half(&mut pts.iter().rev().copied()));
    lower
}

/// Whether `q` lies inside polygon `poly`.
fn inside(poly: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            odd = !odd;
        }
    }
    odd
}

/// The least distance between two polygons' edges; zero where one reaches into the other.
fn gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    if a.iter().any(|p| inside(b, *p)) || b.iter().any(|p| inside(a, *p)) {
        return 0.0;
    }
    let to_edges = |p: [f64; 2], poly: &[[f64; 2]]| {
        (0..poly.len())
            .map(|i| {
                let (u, v) = (poly[i], poly[(i + 1) % poly.len()]);
                let (dx, dy) = (v[0] - u[0], v[1] - u[1]);
                let t = (((p[0] - u[0]) * dx + (p[1] - u[1]) * dy) / (dx * dx + dy * dy).max(1e-300)).clamp(0.0, 1.0);
                (p[0] - u[0] - t * dx).hypot(p[1] - u[1] - t * dy)
            })
            .fold(f64::INFINITY, f64::min)
    };
    a.iter().map(|p| to_edges(*p, b)).chain(b.iter().map(|p| to_edges(*p, a))).fold(f64::INFINITY, f64::min)
}

/// Why a set of windows does not fit.
enum Miss {
    /// Window `window` comes within the least edge of an obstacle at `at`; `into` when it reaches it.
    Held { window: usize, at: [f64; 2], into: bool },
    /// Neighbouring windows stand `gap` apart.
    Crowded { gap: f64 },
}

/// Whether every window round the axis at `rho` keeps the least edge from every obstacle, then from its neighbour.
fn fit(room: &Room, pane: &Pane, count: u32, phase: f64, rho: f64) -> std::result::Result<(), Miss> {
    let step = TAU / count as f64;
    for k in 0..count as usize {
        let (pts, c) = pane.at(rho, phase + step * k as f64);
        for p in &pts {
            let clear = room.clearance(*p);
            if clear < MIN_EDGE_MM {
                return Err(Miss::Held { window: k, at: *p, into: clear <= 0.0 });
            }
        }
        for p in pts.iter().step_by(3) {
            for t in [0.35, 0.7] {
                let q = [c[0] + (p[0] - c[0]) * t, c[1] + (p[1] - c[1]) * t];
                if room.clearance(q) <= 0.0 {
                    return Err(Miss::Held { window: k, at: q, into: true });
                }
            }
        }
        if room.clearance(c) <= 0.0 {
            return Err(Miss::Held { window: k, at: c, into: true });
        }
    }
    let ((first, _), (second, _)) = (pane.at(rho, phase), pane.at(rho, phase + step));
    let g = gap(&first, &second);
    if g < MIN_EDGE_MM {
        return Err(Miss::Crowded { gap: g });
    }
    Ok(())
}

/// A miss in words: what the window reaches, and how.
fn say(miss: &Miss, room: &Room, head: Option<&Head>) -> String {
    match miss {
        Miss::Crowded { gap } => format!("neighbouring windows would leave {gap:.2} mm of metal between them, under the {MIN_EDGE_MM} mm the sand fills"),
        Miss::Held { window, at, into } => {
            let name = room.nearest(*at, MIN_EDGE_MM + 2.0 * room.cell).unwrap_or("the metal round it");
            let what = match head {
                Some(h) if name.starts_with("Claw") || name.contains("rail") || name == "Collet" => format!("{name} of {}", h.name),
                _ => name.to_string(),
            };
            match (*into, name == EDGE) {
                (true, true) => format!("window {} would break through {EDGE}", window + 1),
                (true, false) => format!("window {} would cut into {what}", window + 1),
                (false, _) => format!("window {} would leave under {MIN_EDGE_MM} mm of metal to {what}", window + 1),
            }
        }
    }
}

/// The turn that stands `count` windows furthest from every claw: midway between them where the counts allow.
fn phase_between(claws: &[f64], count: u32) -> f64 {
    if claws.is_empty() {
        return 0.0;
    }
    let step = TAU / count as f64;
    let apart = |a: f64, b: f64| (a - b + PI).rem_euclid(TAU) - PI;
    (0..720)
        .map(|k| step * k as f64 / 720.0)
        .map(|phase| {
            let worst = (0..count).flat_map(|w| claws.iter().map(move |c| apart(phase + step * w as f64, *c).abs())).fold(f64::INFINITY, f64::min);
            (worst, phase)
        })
        .fold((f64::NEG_INFINITY, 0.0), |best, x| if x.0 > best.0 + 1e-9 { x } else { best })
        .1
}

/// `count` windows along a stone's axis round it through the band under it, fitted clear of its head, pilot and the band's edge.
#[allow(clippy::too_many_arguments)]
pub(super) fn azure(gem: Gem, v: &Values, params: &Json, seat: Seat, floor: Option<Floor>, bore: Option<&Bore>, wall: Option<Wall>) -> Result<Made> {
    let who = label(AZURE);
    let bore = band(bore, who)?;
    let head = head_in(bore, params, gem, seat, floor, wall, who)?;
    let shape = if v.s("shape") == "Round" { Shape::Round } else { Shape::Drop };
    let count = v.n("count").clamp(3, 12);
    let stone = gem_label(gem);
    let (entry, exit) = down(bore, [0.0, 0.0]).ok_or_else(|| anyhow!("{who}: there is no band under {stone} to cut"))?;
    let plan = Plan::of(gem);
    let radius = 1.3 * plan.a.max(plan.b) + 1.2;
    // The run of the axis from past the lowest metal to over the highest surface round the stone.
    let (mut top, mut bottom) = (entry, exit);
    for k in 0..48 {
        for r in [0.35, 0.7, 1.0] {
            let p = turned([r * radius, 0.0], TAU * k as f64 / 48.0);
            if let Some((i, o)) = down(bore, p) {
                top = top.max(i);
                bottom = bottom.min(o);
            }
        }
    }
    let (z0, z1) = (bottom - PAST_MM - 0.1, top + LIFT_MM + 0.3);
    let mut room = Room::new(radius);
    if let Some(h) = &head {
        for part in &h.parts {
            let name = if h.key == BEZEL { "Collet" } else { part.names.first().map_or("the head", String::as_str) };
            room.solid(&part.solid, z0, z1, name);
        }
    }
    // The pilot the stone's own bur drills.
    if let Some(bur) = build(BUR, gem, &json!({ "through": true }), seat, None).ok() {
        let named = &bur.named;
        if let Some(pilot) = named.names.iter().position(|n| n == "Pilot") {
            let pts: Vec<[f64; 2]> = named.solid.f.iter().zip(&named.patch).filter(|(_, p)| **p as usize == pilot).flat_map(|(f, _)| f.map(|i| named.solid.v[i as usize])).map(|v| [v[0], v[1]]).collect();
            let rim = hull(pts);
            let reach = rim.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max);
            if rim.len() >= 3 {
                room.mark("the seat's pilot", |q| q[0].hypot(q[1]) <= reach && inside(&rim, q));
            }
        }
    }
    // Along each bearing, how far out a line down the axis still meets metal halfway through the band.
    let f = *bore.frame();
    let (inner, mid) = (bore.design().map_or(0.0, |d| d.inner_radius_mm()), 0.5 * (entry + exit));
    let crest = world(&f, [0.0, 0.0, entry]);
    let middle = 0.5 * (inner + crest[0].hypot(crest[1]));
    let column = |p: [f64; 2]| {
        let (a, d) = (world(&f, [p[0], p[1], mid]), f.z_axis);
        let (qa, qb, qc) = (d[0] * d[0] + d[1] * d[1], 2.0 * (a[0] * d[0] + a[1] * d[1]), a[0] * a[0] + a[1] * a[1] - middle * middle);
        let disc = qb * qb - 4.0 * qa * qc;
        let across = if qa > 1e-9 && disc >= 0.0 {
            let (t1, t2) = ((-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa));
            Some(if t1.abs() < t2.abs() { t1 } else { t2 })
        } else {
            None
        };
        bore.metal_at(a) || across.is_some_and(|t| bore.metal_at(add(a, scale(d, t))))
    };
    let edge: Vec<f64> = (0..EDGE_BEARINGS)
        .map(|k| {
            let dir = turned([1.0, 0.0], TAU * k as f64 / EDGE_BEARINGS as f64);
            let at = |r: f64| column([dir[0] * r, dir[1] * r]);
            let mut r = 0.0;
            while r < radius + 0.5 {
                let next = r + 0.1;
                if !at(next) {
                    let (mut lo, mut hi) = (r, next);
                    for _ in 0..8 {
                        let m = 0.5 * (lo + hi);
                        if at(m) { lo = m } else { hi = m }
                    }
                    return lo;
                }
                r = next;
            }
            f64::INFINITY
        })
        .collect();
    room.mark(EDGE, |p| {
        let b = p[1].atan2(p[0]).rem_euclid(TAU) / TAU * EDGE_BEARINGS as f64;
        let k = b.floor() as usize % EDGE_BEARINGS;
        p[0].hypot(p[1]) > edge[k].min(edge[(k + 1) % EDGE_BEARINGS])
    });
    room.settle();
    let phase = phase_between(head.as_ref().map_or(&[][..], |h| &h.claws[..]), count) + v.f("turn_deg").to_radians();
    let (size, at) = (v.f("size_mm"), v.f("radius_mm"));
    let refuse = |miss: &Miss| anyhow!("{who}: {count} windows under {stone}: {}", say(miss, &room, head.as_ref()));
    let (pane, rho) = if size > 0.0 && at > 0.0 {
        let pane = Pane::new(shape, size);
        fit(&room, &pane, count, phase, at).map_err(|m| refuse(&m))?;
        (pane, at)
    } else {
        let sizes: Vec<f64> = if size > 0.0 {
            vec![size]
        } else {
            let most = (0.3 * gem.w_mm.min(gem.l_mm)).clamp(0.6, 2.5);
            (0..).map(|k| most - FIT_STEP_MM * k as f64).take_while(|s| *s >= WINDOW_MIN_MM - 1e-9).collect()
        };
        let radii: Vec<f64> = if at > 0.0 { vec![at] } else { (0..).map(|k| 0.3 + FIT_STEP_MM * k as f64).take_while(|r| *r < radius - 0.3).collect() };
        let mut found = None;
        let mut last_miss = None;
        // The largest window that fits anywhere, at the middle of the longest run of radii it fits at.
        'sizes: for s in &sizes {
            let pane = Pane::new(shape, *s);
            let (mut run, mut best): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
            for rho in &radii {
                match fit(&room, &pane, count, phase, *rho) {
                    Ok(()) => run.push(*rho),
                    Err(m) => {
                        last_miss = Some(m);
                        if run.len() > best.len() {
                            best = std::mem::take(&mut run);
                        }
                        run.clear();
                    }
                }
            }
            if run.len() > best.len() {
                best = run;
            }
            if !best.is_empty() {
                let rho = best[best.len() / 2];
                found = Some((pane, rho));
                break 'sizes;
            }
        }
        match found {
            Some(x) => x,
            None => {
                // The smallest window tried midway between the pilot and the girdle names what stands in the way.
                let rho = if at > 0.0 { at } else { 0.76 * plan.a.min(plan.b) };
                let smallest = Pane::new(shape, sizes.last().copied().unwrap_or(WINDOW_MIN_MM));
                let miss = fit(&room, &smallest, count, phase, rho).err().or(last_miss);
                bail!("{who}: no room for {count} windows of {WINDOW_MIN_MM} mm or more under {stone}{}", miss.map_or(String::new(), |m| format!(" — {}", say(&m, &room, head.as_ref()))));
            }
        }
    };
    // Each window from over the highest surface it opens down past the lowest metal it goes through.
    let step = TAU / count as f64;
    let mut out = Named::default();
    let mut stations = Vec::with_capacity(count as usize);
    for k in 0..count as usize {
        let (pts, c) = pane.at(rho, phase + step * k as f64);
        let (mut hi, mut lo) = (f64::NEG_INFINITY, f64::INFINITY);
        for p in pts.iter().step_by(3).chain(std::iter::once(&c)) {
            let (i, o) = down(bore, *p).ok_or_else(|| anyhow!("{who}: window {} would break through the band's edge", k + 1))?;
            hi = hi.max(i.max(floor.and_then(|f| f(*p)).unwrap_or(i)));
            lo = lo.min(o);
        }
        let (hi, lo) = (hi + 0.05, lo - 0.05);
        let name = format!("Window {}", k + 1);
        let prism = prism(vec![ring(&pts, |_| hi + LIFT_MM), ring(&pts, |_| lo - PAST_MM)], c, &[&name], [&name, &name]);
        let base = out.solid.v.len() as u32;
        out.solid.v.extend_from_slice(&prism.solid.v);
        out.solid.f.extend(prism.solid.f.iter().map(|f| f.map(|i| i + base)));
        out.patch.extend(std::iter::repeat_n(out.names.len() as u32, prism.solid.f.len()));
        out.names.push(name);
        stations.push([c[0], c[1], hi]);
    }
    let mut made = Made::of(AZURE, out, Some(gem))?;
    made.stations = stations;
    Ok(made)
}

/// An arch's wire for a stone: most of its claws' own, 0.5 to 1.4 mm.
pub fn arch_wire_mm(gem: Gem) -> f64 {
    (0.8 * setting::prong_wire_mm(gem)).clamp(0.5, 1.4)
}

/// Where an arch meets the head at plan angle `phi`: its highest rail's centre and radius there, or a point up a collet's wall and none.
fn meeting(head: &Head, phi: f64) -> Option<(P3, f64)> {
    let near = |part: &Named| -> Vec<P3> {
        part.solid.v.iter().filter(|v| ((v[1].atan2(v[0]) - phi + PI).rem_euclid(TAU) - PI).abs() < 0.06).copied().collect()
    };
    let mean_z = |part: &Named| part.solid.v.iter().map(|v| v[2]).sum::<f64>() / part.solid.v.len().max(1) as f64;
    let (dir, sec) = ([phi.cos(), phi.sin()], |v: &P3| [v[0] * phi.cos() + v[1] * phi.sin(), v[2]]);
    if head.key == BEZEL {
        // The collet's outer wall corners, and the point [`COLLET_MEET`] of the way from the lowest to the highest.
        let pts: Vec<[f64; 2]> = head.parts.first().map(near)?.iter().map(sec).collect();
        let most = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        let wall: Vec<[f64; 2]> = pts.into_iter().filter(|p| p[0] >= most - 0.25).collect();
        let top = wall.iter().copied().max_by(|a, b| a[1].total_cmp(&b[1]))?;
        let low = wall.iter().copied().min_by(|a, b| a[1].total_cmp(&b[1]))?;
        let at = |k: usize| low[k] + COLLET_MEET * (top[k] - low[k]);
        return (top[1] > low[1]).then(|| ([at(0) * dir[0], at(0) * dir[1], at(1)], 0.0));
    }
    let rail = head.parts.iter().filter(|p| p.names.first().is_some_and(|n| n.contains("rail"))).max_by(|a, b| mean_z(a).total_cmp(&mean_z(b)))?;
    let pts: Vec<[f64; 2]> = near(rail).iter().map(sec).collect();
    if pts.len() < 3 {
        return None;
    }
    let n = pts.len() as f64;
    let c = [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n];
    let r = pts.iter().map(|p| (p[0] - c[0]).hypot(p[1] - c[1])).sum::<f64>() / n;
    Some(([c[0] * dir[0], c[0] * dir[1], c[1]], r))
}

/// A round wire domed at both ends along a path in the plane with normal `m`, each ring holding a point on the plane either side.
fn wire(path: &[P3], radii: &[f64], m: P3) -> Solid {
    let (len, n) = (path.len(), WIRE_SIDES);
    let mut out = Solid::default();
    if len < 2 {
        return out;
    }
    let tangent = |i: usize| unit(sub(path[(i + 1).min(len - 1)], path[i.saturating_sub(1)]));
    let axes = |t: P3| {
        let u = unit(sub(m, scale(t, dot(m, t))));
        (u, cross(u, t))
    };
    let mut rings: Vec<(P3, f64, P3)> = Vec::new();
    let (t0, t1) = (tangent(0), tangent(len - 1));
    for j in (1..5).rev() {
        let a = FRAC_PI_2 * j as f64 / 5.0;
        rings.push((add(path[0], scale(t0, -radii[0] * a.sin())), radii[0] * a.cos(), t0));
    }
    rings.extend((0..len).map(|i| (path[i], radii[i], tangent(i))));
    for j in 1..5 {
        let a = FRAC_PI_2 * j as f64 / 5.0;
        rings.push((add(path[len - 1], scale(t1, radii[len - 1] * a.sin())), radii[len - 1] * a.cos(), t1));
    }
    for (c, r, t) in &rings {
        let (u, v) = axes(*t);
        for k in 0..n {
            let (s, co) = (TAU * k as f64 / n as f64).sin_cos();
            out.v.push(add(*c, add(scale(u, r * co), scale(v, r * s))));
        }
    }
    let at = |r: usize, i: usize| (r * n + i % n) as u32;
    for r in 0..rings.len() - 1 {
        for i in 0..n {
            out.f.push([at(r, i), at(r + 1, i), at(r + 1, i + 1)]);
            out.f.push([at(r, i), at(r + 1, i + 1), at(r, i + 1)]);
        }
    }
    let (start, end) = (out.v.len() as u32, out.v.len() as u32 + 1);
    out.v.push(add(path[0], scale(t0, -radii[0])));
    out.v.push(add(path[len - 1], scale(t1, radii[len - 1])));
    let last = rings.len() - 1;
    for i in 0..n {
        out.f.push([start, at(0, i), at(0, i + 1)]);
        out.f.push([at(last, i), end, at(last, i + 1)]);
    }
    out
}

/// A curve through `fine` resampled every [`ARCH_STEP_MM`] of its length.
fn resample(fine: Vec<P3>) -> Vec<P3> {
    let mut arc = vec![0.0];
    for w in fine.windows(2) {
        arc.push(arc[arc.len() - 1] + norm(sub(w[1], w[0])));
    }
    let total = arc[arc.len() - 1];
    let n = ((total / ARCH_STEP_MM).ceil() as usize).max(8);
    (0..=n)
        .map(|k| {
            let s = total * k as f64 / n as f64;
            let i = arc.partition_point(|a| *a < s).clamp(1, fine.len() - 1);
            let t = if arc[i] > arc[i - 1] { (s - arc[i - 1]) / (arc[i] - arc[i - 1]) } else { 0.0 };
            add(fine[i - 1], scale(sub(fine[i], fine[i - 1]), t))
        })
        .collect()
}

/// Two wire arches in the stone's frame from the band `spread` either side of it to the underside of its head's gallery rail, sunk into both.
#[allow(clippy::too_many_arguments)]
pub(super) fn cathedral(gem: Gem, v: &Values, params: &Json, seat: Seat, floor: Option<Floor>, bore: Option<&Bore>, wall: Option<Wall>) -> Result<Named> {
    let who = label(CATHEDRAL);
    let bore = band(bore, who)?;
    let head = head_in(bore, params, gem, seat, floor, wall, who)?
        .ok_or_else(|| anyhow!("{who} rise to a head's gallery rail and name no head: set {} in claws, a basket or a bezel first", gem_label(gem)))?;
    let f = *bore.frame();
    let (o, up) = (f.origin, f.z_axis);
    let theta0 = o[1].atan2(o[0]);
    let tangent = [-theta0.sin(), theta0.cos(), 0.0];
    // The arches' plane: through the stone's origin, holding the ring's tangent and the stone's axis.
    let m = unit(cross(tangent, up));
    ensure!(m[2].abs() > 0.5, "{who} rise from the band's crown either side of a stone standing out of it; {} faces along the finger", gem_label(gem));
    let r_a = 0.5 * v.f("wire_mm");
    let (spread, rise) = (v.f("spread_deg").to_radians(), v.f("rise"));
    // The band's outer surface at ring angle `theta` in the arches' plane.
    let surface = |theta: f64| -> Option<P3> {
        let (s, c) = theta.sin_cos();
        let mut z = o[2];
        let mut r = 0.0;
        for _ in 0..3 {
            r = bore.crossings(theta.to_degrees(), z).last().copied()?;
            z = o[2] - (m[0] * (r * c - o[0]) + m[1] * (r * s - o[1])) / m[2];
        }
        Some([r * c, r * s, z])
    };
    let mut out = Named::default();
    let sink = FOOT_SINK * r_a;
    // The power an arch's height grows by along its way.
    let lift = 1.6 / (1.0 + rise);
    for (k, sign) in [(1u32, 1.0), (2, -1.0)] {
        let name = format!("Arch {k}");
        // The ring's tangent on this side, as a bearing in the stone's plan.
        let side = local(&f, add(o, scale(tangent, sign)));
        let phi = side[1].atan2(side[0]);
        let (centre, rail_r) = meeting(&head, phi).ok_or_else(|| anyhow!("{who}: #{} {} has no rail on its {} side for an arch to meet", head_param(params).unwrap_or(0), head.name, if sign > 0.0 { "one" } else { "other" }))?;
        let inward = [-phi.cos(), -phi.sin(), 0.0];
        let end = if head.key == BEZEL { add(centre, scale(inward, 0.5 * r_a)) } else { sub(centre, [0.0, 0.0, 0.5 * rail_r]) };
        let p3 = world(&f, end);
        let (theta_f, theta_r) = (theta0 + sign * spread, p3[1].atan2(p3[0]));
        let under_head = surface(theta_r).ok_or_else(|| anyhow!("{who}: the band has no surface under {}", head.name))?;
        let height = p3[0].hypot(p3[1]) - under_head[0].hypot(under_head[1]);
        ensure!(height > r_a, "{who}: {} stands too low over the band for an arch to meet it", head.name);
        // Round the ring from the foot to the head with an easing to a stop, off the band by a height growing from under it to the head's.
        let fine: Vec<P3> = (0..=400)
            .map(|i| {
                let u = i as f64 / 400.0;
                let theta = theta_f + (theta_r - theta_f) * (FRAC_PI_2 * u).sin();
                let s = surface(theta).ok_or_else(|| anyhow!("{who}: the band has no surface in the arches' plane {:.0}° round the ring", theta.to_degrees()))?;
                let radial = unit([s[0], s[1], 0.0]);
                Ok(add(s, scale(radial, (height + sink) * u.powf(lift) - sink)))
            })
            .collect::<Result<_>>()?;
        let path = resample(fine);
        // From halfway on, the wire's underside stays out of the band.
        let under = |p: P3| {
            let radial = unit([p[0], p[1], 0.0]);
            bore.metal_at(sub(p, scale(radial, 0.9 * r_a)))
        };
        let emerged = path.len() * EMERGE / 100;
        if let Some(back) = path[emerged..].iter().position(|p| under(*p)) {
            let at = local(&f, path[emerged + back]);
            bail!("{who}: {name} would run back into the band {:.1} mm from the stone's axis: give it more rise, or less spread", at[0].hypot(at[1]));
        }
        let n = path.len();
        let radii: Vec<f64> = (0..n).map(|i| r_a * (1.0 - 0.12 * i as f64 / (n - 1).max(1) as f64)).collect();
        let plane = [dot(m, f.x_axis), dot(m, f.y_axis), dot(m, f.z_axis)];
        let tube = wire(&path.iter().map(|p| local(&f, *p)).collect::<Vec<_>>(), &radii, plane);
        let base = out.solid.v.len() as u32;
        out.solid.v.extend_from_slice(&tube.v);
        out.solid.f.extend(tube.f.iter().map(|t| t.map(|i| i + base)));
        out.patch.extend(std::iter::repeat_n(out.names.len() as u32, tube.f.len()));
        out.names.push(name);
    }
    if let Some(snag) = wall.and_then(|w| setting::breaks_wall(&[Named { solid: out.solid.clone(), patch: Vec::new(), names: vec!["An arch's foot".into()] }], w)) {
        bail!("{who}: {snag}");
    }
    Ok(out)
}

/// A piercing at a band point: its placement, its parameters sized from the band there, and its stage.
#[derive(Clone, Debug, PartialEq)]
pub struct PierceAt {
    pub placement: Placement,
    pub params: Json,
    pub stage: Stage,
    /// Cut through a side face along the finger, rather than down the surface's normal.
    pub side_face: bool,
}

/// The stage a cut along `axis` defaults to: Cast within [`ALONG_PULL_DEG`] of the pull or under lost wax, else Bench.
pub fn cut_stage(axis: P3, sand: bool) -> Stage {
    if !sand || unit(axis)[2].abs() >= ALONG_PULL_DEG.to_radians().cos() { Stage::Cast } else { Stage::Bench }
}

/// Farthest a stone's girdle centre may stand off the parting plane for cathedral shoulders under it to pour clean in sand, mm.
pub const SHOULDER_PARTING_MM: f64 = 0.005;

/// The stage shoulders under a stone whose girdle centre stands at `stone_z_mm` default to: Cast under lost wax, and in sand
/// Cast within [`SHOULDER_PARTING_MM`] of the parting plane at `parting_z_mm`, else Bench with the head they meet.
pub fn shoulder_stage_at(stone_z_mm: f64, parting_z_mm: f64, sand: bool) -> Stage {
    if !sand || (stone_z_mm - parting_z_mm).abs() <= SHOULDER_PARTING_MM { Stage::Cast } else { Stage::Bench }
}

/// [`shoulder_stage_at`] where the stone's seat is not known: Bench in sand, Cast under lost wax.
pub fn shoulder_stage(sand: bool) -> Stage {
    shoulder_stage_at(f64::INFINITY, 0.0, sand)
}

/// [`shoulder_stage_at`] for `design`, its stone's girdle centre at `stone` as built, against the parting plane its draft names, else
/// the band's mid-plane; a stone not yet built is the bench's.
pub fn shoulder_stage_for(design: &RingDesign, stone: Option<[f64; 3]>) -> Stage {
    let d = &design.draft;
    let parting = if !d.auto_parting && d.parting_z_mm.is_finite() { d.parting_z_mm } else { 0.0 };
    shoulder_stage_at(stone.map_or(f64::INFINITY, |p| p[2]), parting, super::sand(design))
}

/// Rounded to the hundredth, as the inspector shows it.
fn hundredth(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Piercing `shape` at `theta_deg`, `across_mm`, or at `hit` (a point and its normal): along the finger through a side face, else down the surface.
pub fn pierce_at(design: &RingDesign, theta_deg: f64, across_mm: f64, hit: Option<(P3, P3)>, shape: Shape) -> Result<PierceAt> {
    ensure!(design.band_is_procedural(), "A piercing cuts the procedural band, and this ring is its parts alone");
    let bore = Bore::of(design, &cadkernel::brep::Placement::IDENTITY);
    let sand = super::sand(design);
    let (lo, hi) = bore.z_extent(theta_deg).ok_or_else(|| anyhow!("The band has no section at {theta_deg:.0}°"))?;
    let side = hit.map(|(_, n)| n[2]).filter(|z| z.abs() >= 0.8).map(f64::signum);
    let keep = MIN_EDGE_MM + 0.2;
    let (placement, s, point) = match side {
        Some(side) => {
            let face = if side > 0.0 { hi } else { lo };
            let rs = bore.crossings(theta_deg, face - side * 0.05);
            ensure!(rs.len() >= 2, "The band has no side face at {theta_deg:.0}°");
            let r_hit = hit.map_or(0.5 * (rs[0] + rs[rs.len() - 1]), |(w, _)| w[0].hypot(w[1]));
            let (r_in, r_out) = rs.chunks(2).filter(|p| p.len() == 2).map(|p| (p[0], p[1])).find(|(a, b)| r_hit >= *a && r_hit <= *b).unwrap_or((rs[0], rs[rs.len() - 1]));
            let room = r_out - r_in;
            let s = (0.45 * room).min(room - 2.0 * keep);
            ensure!(s >= WINDOW_MIN_MM, "The side face here is {room:.2} mm across, too narrow to pierce");
            let r_c = r_hit.clamp(r_in + keep + 0.5 * s, r_out - keep - 0.5 * s);
            let crest = bore.crossings(theta_deg, 0.0).last().copied().ok_or_else(|| anyhow!("The band has no crest at {theta_deg:.0}°"))?;
            let placement = Placement::Ring { theta_deg, across_mm: 0.0, height_mm: hundredth(r_c - crest), spin_deg: 0.0, tilt_deg: 0.0, cant_deg: -90.0 * side };
            (placement, s, -90.0 * side)
        }
        None => {
            let edge = (across_mm - lo).min(hi - across_mm);
            let s = (0.35 * (hi - lo)).min(2.0 * (edge - keep - 0.05));
            ensure!(s >= WINDOW_MIN_MM, "Too near the band's edge to pierce here: {edge:.2} mm from it");
            (Placement::Ring { theta_deg, across_mm, height_mm: 0.0, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 }, s, 90.0)
        }
    };
    let s = s.min(3.0);
    let (length, width, turn) = match shape {
        Shape::Round => (s, s, 0.0),
        Shape::Oval => (1.5 * s, s, 0.0),
        Shape::Marquise => (2.0 * s, s, 0.0),
        Shape::Heart => (s, 0.95 * s, point),
        Shape::Drop => (s, 0.62 * s, point),
    };
    let (length, width) = shape.sized(length.max(WINDOW_MIN_MM), width.max(WINDOW_MIN_MM));
    let chamfer = (0.07 * width.min(length)).clamp(0.06, 0.2);
    let depth = (0.6 * design.profile.thickness_mm).clamp(0.1, 10.0);
    let params = json!({
        "shape": shape.name(),
        "width_mm": hundredth(width),
        "length_mm": hundredth(length),
        "turn_deg": turn,
        "through": true,
        "depth_mm": hundredth(depth),
        "chamfer_mm": hundredth(chamfer),
    });
    let axis = match side {
        Some(side) => [0.0, 0.0, side],
        None => hit.map_or([theta_deg.to_radians().cos(), theta_deg.to_radians().sin(), 0.0], |(_, n)| n),
    };
    Ok(PierceAt { placement, params, stage: cut_stage(axis, sand), side_face: side.is_some() })
}

/// The piercing feature `id` cut as `at` says.
pub fn pierce_feature(id: Id, shape: Shape, at: &PierceAt) -> Feature {
    Feature {
        id,
        name: format!("{} piercing", shape.name()),
        enabled: true,
        operation: Operation::Builder { key: PIERCE.into(), on: None, params: at.params.clone() },
        component: Component { placement: at.placement.clone(), stage: at.stage, ..component(PIERCE) },
    }
}

/// `count` azures feature `id` under stone `stone`, clear of head `head` when it has one.
pub fn azure_feature(id: Id, stone: Id, head: Option<Id>, count: u32, stage: Stage) -> Feature {
    let mut params = json!({ "count": count });
    if let (Some(h), Some(map)) = (head, params.as_object_mut()) {
        map.insert(HEAD.into(), json!(h));
    }
    Feature { id, name: format!("{count} azures"), enabled: true, operation: Operation::Builder { key: AZURE.into(), on: Some(stone), params }, component: Component { stage, ..component(AZURE) } }
}

/// Cathedral shoulders feature `id` from the band to head `head` of stone `stone`.
pub fn cathedral_feature(id: Id, stone: Id, head: Id, stage: Stage) -> Feature {
    let mut params = json!({});
    if let Some(map) = params.as_object_mut() {
        map.insert(HEAD.into(), json!(head));
    }
    Feature { id, name: "Cathedral shoulders".into(), enabled: true, operation: Operation::Builder { key: CATHEDRAL.into(), on: Some(stone), params }, component: Component { stage, ..component(CATHEDRAL) } }
}

#[cfg(test)]
mod tests;
