//! Stone settings as solids: heads made beforehand and the burs that cut
//! their seats, built once per stone in the stone's own frame — girdle plane
//! at `z = 0`, `x` along the stone's length, `z` up through the table — then
//! placed on the ring and resolved by [`crate::csg`].
//!
//! A seat in the height field is a bump: it cannot overhang, cannot open a
//! hole, and stretches with the chart it is drawn in. These are the real
//! parts — a claw head whose prongs are notched by the stone itself, a collet
//! with a bearing ledge, a bur that leaves a bright-cut bevel, a girdle wall,
//! a bearing cone and a pilot through to the finger — and because they are
//! sized in the stone's millimetres and placed rigidly, the seat fits its
//! stone wherever on the ring it lands.
use crate::csg::{self, Op, Snag, Solid, P3};
use crate::gem::{Gem, GemForm};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// Which pre-made solid a seat carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SolidKind {
    /// Height-field stock only: the cast blank, cut at the bench.
    #[default]
    None,
    /// The seat bur alone — a flush or gypsy setting.
    Flush,
    /// The bur plus four raised beads: pavé and melee.
    Bead,
    /// A claw head with gallery rails, notched to its stone.
    Prong,
    /// A collet with a bearing ledge and a lip up the crown.
    Bezel,
}

impl SolidKind {
    pub const ALL: &'static [SolidKind] = &[Self::None, Self::Flush, Self::Bead, Self::Prong, Self::Bezel];

    pub fn is_none(&self) -> bool { *self == Self::None }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Cast stock only",
            Self::Flush => "Flush cut",
            Self::Bead => "Bead set",
            Self::Prong => "Claw head",
            Self::Bezel => "Collet",
        }
    }

    /// How far the girdle sits below the pad's top when the seat does not say: negative stands it above.
    pub fn girdle_drop_mm(self, gem: Gem) -> Option<f64> {
        match self {
            Self::None => None,
            Self::Flush | Self::Bead => Some(match gem.form {
                GemForm::Faceted => 0.85 * gem.crown_mm(),
                GemForm::Cabochon => 0.25 * gem.depth_mm(),
            }),
            Self::Prong => Some(-(gem.pavilion_mm() + 0.2)),
            Self::Bezel => Some(-(collet_depth(gem) - COLLET_SINK)),
        }
    }
}

/// How the seat meets the metal it stands in, in the stone's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// Height of the metal surface over the girdle plane at the seat's centre, mm.
    pub surface_z: f64,
    /// Distance from the girdle down to open air past the bore, for a pilot drilled through; `None` leaves it blind.
    pub through_mm: Option<f64>,
    /// Claw count; 0 takes the cut's own.
    pub prongs: u32,
}

/// What a seat adds to the ring and what it cuts from it, in the stone's frame.
#[derive(Clone, Debug, Default)]
pub struct Parts {
    pub add: Vec<Solid>,
    pub cut: Vec<Solid>,
    /// Beads as centres and radii rather than solids: neighbours share them, so they are merged once every seat is placed.
    pub beads: Vec<(P3, f64)>,
}

const CLEAR: f64 = 0.03;
const COLLET_SINK: f64 = 0.25;
const PRONG_LEAN: f64 = 0.21;
const PHASE: f64 = 0.1234;

fn girdle_half(gem: Gem) -> f64 { (0.015 * gem.w_mm).clamp(0.02, 0.06) }
fn collet_depth(gem: Gem) -> f64 {
    match gem.form {
        GemForm::Faceted => 0.6 * gem.pavilion_mm() + 0.35,
        GemForm::Cabochon => 0.6,
    }
}
/// Claw wire diameter for a stone, mm.
pub fn prong_wire_mm(gem: Gem) -> f64 { (0.11 * gem.w_mm + 0.48).clamp(0.6, 1.5) }
/// Bead radius for a stone, mm.
pub fn bead_radius_mm(gem: Gem) -> f64 { (0.14 * gem.w_mm).clamp(0.16, 0.34) }

/// The girdle outline: the superellipse every seat, stone and report already reads.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub a: f64,
    pub b: f64,
    pub pow: f64,
}

impl Plan {
    pub fn of(gem: Gem) -> Self {
        Self { a: gem.l_mm.max(0.2) * 0.5, b: gem.w_mm.max(0.2) * 0.5, pow: gem.cut.plan_pow().max(1.0) }
    }
    pub fn point(&self, phi: f64) -> [f64; 2] {
        let (s, c) = phi.sin_cos();
        let m = (c.abs().powf(self.pow) + s.abs().powf(self.pow)).powf(-1.0 / self.pow);
        [c * m * self.a, s * m * self.b]
    }
    /// Outward unit normal, from a centred difference so a marquise's point reads the mean of its two sides.
    pub fn normal(&self, phi: f64) -> [f64; 2] {
        let h = 1e-3;
        let (p, q) = (self.point(phi - h), self.point(phi + h));
        let (tx, ty) = (q[0] - p[0], q[1] - p[1]);
        let l = tx.hypot(ty).max(1e-12);
        [ty / l, -tx / l]
    }
    pub fn perimeter(&self) -> f64 {
        let n = 256;
        (0..n).map(|i| {
            let (p, q) = (self.point(TAU * i as f64 / n as f64), self.point(TAU * (i + 1) as f64 / n as f64));
            (q[0] - p[0]).hypot(q[1] - p[1])
        }).sum()
    }
    fn segments(&self) -> usize {
        ((self.perimeter() / 0.13).round() as usize).clamp(28, 96)
    }
    /// Where the claws stand: the corners of a squared plan, the points of a marquise, else evenly round.
    pub fn claw_angles(&self, n: u32) -> Vec<f64> {
        let n = n.clamp(3, 8) as usize;
        if self.pow >= 3.0 && n == 4 {
            let c = self.b.atan2(self.a);
            return vec![c, PI - c, PI + c, TAU - c];
        }
        let start = if self.pow < 1.8 || n % 2 == 1 { 0.0 } else { PI / n as f64 };
        (0..n).map(|k| start + TAU * k as f64 / n as f64).collect()
    }
}

/// One station of a section swept round the plan: the outline scaled by `s`, pushed out along its normal by `o`, at height `z`.
/// An inward `o` is read as a scale instead, exact across the stone's width and wider toward its ends: a normal
/// offset folds over itself wherever it outruns the outline's own radius, which at a marquise's point is zero.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub s: f64,
    pub o: f64,
    pub z: f64,
}

fn st(s: f64, o: f64, z: f64) -> Station { Station { s, o, z } }
const POLE: f64 = 0.0;
fn pole(z: f64) -> Station { st(POLE, 0.0, z) }

/// Sweep a section round the plan. Stations run down the outside; a section that starts and ends on the axis
/// closes as a ball, any other closes on itself as a ring and must run clockwise with the axis to its left.
pub fn sweep(plan: &Plan, section: &[Station], around: usize) -> Solid {
    let n = around.max(8);
    let is_pole = |s: &Station| s.s == 0.0 && s.o == 0.0;
    let ball = section.first().is_some_and(is_pole) && section.last().is_some_and(is_pole);
    let rows: Vec<&Station> = section.iter().filter(|s| !is_pole(s)).collect();
    let mut out = Solid::default();
    for row in &rows {
        let (s, o) = if row.o < 0.0 { ((row.s * (1.0 + row.o / plan.a.min(plan.b).max(1e-6))).max(0.02), 0.0) } else { (row.s, row.o) };
        for i in 0..n {
            let phi = TAU * (i as f64 + PHASE) / n as f64;
            let (p, nrm) = (plan.point(phi), plan.normal(phi));
            out.v.push([p[0] * s + nrm[0] * o, p[1] * s + nrm[1] * o, row.z]);
        }
    }
    let at = |r: usize, i: usize| (r * n + i % n) as u32;
    let bands = if ball { rows.len().saturating_sub(1) } else { rows.len() };
    for r in 0..bands {
        let below = (r + 1) % rows.len();
        for i in 0..n {
            out.f.push([at(r, i), at(below, i), at(below, i + 1)]);
            out.f.push([at(r, i), at(below, i + 1), at(r, i + 1)]);
        }
    }
    if ball {
        let top = out.v.len() as u32;
        out.v.push([0.0, 0.0, section[0].z]);
        let bottom = out.v.len() as u32;
        out.v.push([0.0, 0.0, section[section.len() - 1].z]);
        let last = rows.len() - 1;
        for i in 0..n {
            out.f.push([top, at(0, i), at(0, i + 1)]);
            out.f.push([at(last, i), bottom, at(last, i + 1)]);
        }
    }
    out
}

/// A round wire along a path, its radius given per point; either end flat or domed.
pub fn tube(path: &[P3], radii: &[f64], around: usize, dome: (bool, bool)) -> Solid {
    let n = around.max(6);
    let m = path.len();
    let mut out = Solid::default();
    if m < 2 { return out; }
    let sub = |a: P3, b: P3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let add = |a: P3, b: P3, k: f64| [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let unit = |a: P3| { let l = dot(a, a).sqrt().max(1e-12); [a[0] / l, a[1] / l, a[2] / l] };
    let tangent = |i: usize| unit(sub(path[(i + 1).min(m - 1)], path[i.saturating_sub(1)]));

    // Rings: centre, radius, and a frame carried along the path without twisting.
    let mut rings: Vec<(P3, f64, P3, P3)> = Vec::new();
    let t0 = tangent(0);
    let seed = if t0[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let mut u = unit(sub(seed, [t0[0] * dot(seed, t0), t0[1] * dot(seed, t0), t0[2] * dot(seed, t0)]));
    let cap = 5;
    let mut push = |c: P3, r: f64, t: P3, u: &mut P3| {
        let k = dot(*u, t);
        *u = unit([u[0] - t[0] * k, u[1] - t[1] * k, u[2] - t[2] * k]);
        rings.push((c, r, *u, cross(*u, t)));
    };
    if dome.0 {
        for j in (1..cap).rev() {
            let a = PI / 2.0 * j as f64 / cap as f64;
            push(add(path[0], t0, -radii[0] * a.sin()), radii[0] * a.cos(), t0, &mut u);
        }
    }
    for i in 0..m { push(path[i], radii[i], tangent(i), &mut u); }
    let t1 = tangent(m - 1);
    if dome.1 {
        for j in 1..cap {
            let a = PI / 2.0 * j as f64 / cap as f64;
            push(add(path[m - 1], t1, radii[m - 1] * a.sin()), radii[m - 1] * a.cos(), t1, &mut u);
        }
    }
    for (c, r, u, v) in &rings {
        for i in 0..n {
            let (s, co) = (TAU * (i as f64 + PHASE) / n as f64).sin_cos();
            out.v.push([c[0] + r * (u[0] * co + v[0] * s), c[1] + r * (u[1] * co + v[1] * s), c[2] + r * (u[2] * co + v[2] * s)]);
        }
    }
    let at = |r: usize, i: usize| (r * n + i % n) as u32;
    for r in 0..rings.len() - 1 {
        for i in 0..n {
            out.f.push([at(r, i), at(r + 1, i), at(r + 1, i + 1)]);
            out.f.push([at(r, i), at(r + 1, i + 1), at(r, i + 1)]);
        }
    }
    let start = out.v.len() as u32;
    out.v.push(add(path[0], t0, if dome.0 { -radii[0] } else { 0.0 }));
    let end = out.v.len() as u32;
    out.v.push(add(path[m - 1], t1, if dome.1 { radii[m - 1] } else { 0.0 }));
    let last = rings.len() - 1;
    for i in 0..n {
        out.f.push([start, at(0, i), at(0, i + 1)]);
        out.f.push([at(last, i), end, at(last, i + 1)]);
    }
    out
}

/// A ball, for a bead.
pub fn ball(centre: P3, r: f64, rings: usize) -> Solid {
    let section: Vec<Station> = (0..=rings).map(|k| {
        let a = PI * k as f64 / rings as f64;
        if k == 0 || k == rings { pole(r * a.cos()) } else { st(a.sin(), 0.0, r * a.cos()) }
    }).collect();
    sweep(&Plan { a: r, b: r, pow: 2.0 }, &section, rings * 2).translated(centre)
}

/// The crown's own scale at a height over the girdle: 1 at the girdle, 0.78 at the bezel facets' break, 0.52 at the table.
fn crown_scale(gem: Gem, z: f64) -> f64 {
    let c = gem.crown_mm().max(1e-6);
    match gem.form {
        GemForm::Cabochon => (1.0 - (z / c).clamp(0.0, 1.0).powi(2)).sqrt(),
        GemForm::Faceted => {
            let t = (z / c).clamp(0.0, 1.0);
            if t <= 0.55 { 1.0 - 0.22 * t / 0.55 } else { 0.78 - 0.26 * (t - 0.55) / 0.45 }
        }
    }
}

/// The stone as the preview draws it, grown by `clear`: what notches a claw and what a seat must swallow.
pub fn envelope(gem: Gem, clear: f64) -> Solid {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let section: Vec<Station> = match gem.form {
        GemForm::Faceted => vec![
            pole(c + clear), st(0.52, clear, c + clear), st(0.78, clear, 0.55 * c + clear * 0.8),
            st(1.0, clear, g + clear * 0.5), st(1.0, clear, -g - clear * 0.5), pole(-p - clear),
        ],
        GemForm::Cabochon => {
            let mut s = vec![pole(c + clear)];
            for k in (0..8).rev() {
                let t = k as f64 / 8.0;
                s.push(st((1.0 - t * t).sqrt(), clear, c * t + clear * t));
            }
            s.push(st(1.0, clear, -clear));
            s.push(pole(-clear));
            s
        }
    };
    sweep(&plan, &section, plan.segments())
}

/// The setting bur: a bright-cut bevel at the surface, a lip left over the girdle, the girdle wall,
/// a bearing cone at the pavilion's own angle, and a pilot below it.
pub fn bur(gem: Gem, fit: &Fit) -> Solid {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let bevel = (0.07 * gem.w_mm).clamp(0.08, 0.22);
    let lip = (0.03 * gem.w_mm).clamp(0.03, 0.08);
    let top = fit.surface_z.max(c) + 1.2;
    let mut s = vec![pole(top), st(1.0, CLEAR + bevel, top)];
    let under = fit.surface_z - bevel - lip - CLEAR;
    if under > g + 0.04 {
        s.push(st(1.0, CLEAR + bevel, fit.surface_z + 0.01));
        s.push(st(1.0, -lip, under));
    }
    s.push(st(1.0, CLEAR, g));
    s.push(st(1.0, CLEAR, -g));
    match gem.form {
        GemForm::Faceted => {
            let blind = p + 0.15;
            s.push(st(0.52, CLEAR, -g - 0.48 * p));
            s.push(st(0.52, CLEAR, -fit.through_mm.unwrap_or(blind).max(g + 0.48 * p + 0.1)));
            s.push(pole(-fit.through_mm.unwrap_or(blind).max(g + 0.48 * p + 0.1)));
        }
        GemForm::Cabochon => s.push(pole(-g - 0.04)),
    }
    sweep(&plan, &s, plan.segments())
}

/// Room for a pavilion that hangs below its head into the band, and the pilot under it.
pub fn relief(gem: Gem, from_z: f64, inset: f64, fit: &Fit) -> Option<Solid> {
    if gem.form == GemForm::Cabochon { return None; }
    let plan = Plan::of(gem);
    let p = gem.pavilion_mm();
    let blind = p + 0.25;
    let mut s = vec![pole(from_z), st(1.0, -inset, from_z), st(1.0, -inset, from_z - 0.05)];
    let waist = (1.0 - inset / plan.b.max(1e-6)).clamp(0.3, 1.0);
    let z_waist = -(1.0 - waist) * p - 0.2;
    if z_waist < from_z - 0.1 { s.push(st(waist, 0.0, z_waist)); }
    s.push(st(0.3, 0.0, -0.7 * p - 0.2));
    let floor = fit.through_mm.unwrap_or(blind).max(0.7 * p + 0.3);
    s.push(st(0.3, 0.0, -floor));
    s.push(pole(-floor));
    Some(sweep(&plan, &s, plan.segments()))
}

/// Four beads raised at the stone's corners, overlapping the girdle so the bur shapes them to it.
pub fn beads(gem: Gem, fit: &Fit) -> Vec<(P3, f64)> {
    let plan = Plan::of(gem);
    let r = bead_radius_mm(gem);
    plan.claw_angles(4).into_iter().map(|phi| {
        let (p, n) = (plan.point(phi), plan.normal(phi));
        ([p[0] + n[0] * r * 0.45, p[1] + n[1] * r * 0.45, fit.surface_z + 0.15 * r], r)
    }).collect()
}

/// A collet: tapered wall, bearing ledge at the pavilion's angle, a lip leaning up the crown.
pub fn collet(gem: Gem) -> Solid {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let wall = (0.10 * gem.w_mm + 0.22).clamp(0.35, 0.9);
    let ledge = (0.09 * gem.w_mm).clamp(0.18, 0.45);
    let depth = collet_depth(gem);
    let top = g + match gem.form { GemForm::Faceted => 0.40 * c, GemForm::Cabochon => 0.30 * c };
    let lean = 0.8 * (1.0 - crown_scale(gem, top - g)) * plan.b;
    let slope = match gem.form { GemForm::Faceted => 0.9 * p / plan.b.max(1e-6), GemForm::Cabochon => 0.0 };
    let section = [
        st(1.0, CLEAR + wall - lean * 0.5, top),
        st(1.0, CLEAR + wall, top - 0.16),
        st(1.0, CLEAR + wall * 0.8, -depth),
        st(1.0, -ledge, -depth),
        st(1.0, -ledge, -g - ledge * slope),
        st(1.0, CLEAR, -g),
        st(1.0, CLEAR, g),
        st(1.0, CLEAR - lean, top - 0.05),
        st(1.0, CLEAR - lean + 0.05, top),
    ];
    sweep(&plan, &section, plan.segments())
}

/// A claw head: tapered prongs leaning out from a base rail, bent over the crown, with a gallery rail between,
/// joined into one solid and then notched by the stone itself so every claw bears on it.
pub fn claw_head(gem: Gem, prongs: u32) -> Result<Solid, Snag> {
    let head = csg::union_all(&claw_parts(gem, prongs))?;
    let mut head = csg::combine(&head, &envelope(gem, 0.02), Op::Subtract)?;
    head.compact();
    Ok(head)
}

/// The claws and rails of a head before they are joined, each closed on its own.
pub fn claw_parts(gem: Gem, prongs: u32) -> Vec<Solid> {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let d = prong_wire_mm(gem);
    let count = if prongs >= 3 { prongs } else if plan.pow < 1.8 || (plan.a / plan.b > 1.25 && plan.pow < 3.0) { 6 } else { 4 };
    let depth = p + 0.2;
    let mut parts = Vec::new();
    for phi in plan.claw_angles(count) {
        let (o, n) = (plan.point(phi), plan.normal(phi));
        let reach = o[0].hypot(o[1]).max(1e-6);
        // The crown as this claw meets it: rising `rise` over `inward` from the girdle to the facets' break.
        let (inward, rise) = match gem.form {
            GemForm::Faceted => (0.22 * reach, 0.55 * c),
            GemForm::Cabochon => (0.30 * reach, 0.71 * c),
        };
        let slope = rise.atan2(inward);
        // A straight leaning wire, one bend no tighter than the wire is thick, then along the crown to the
        // tip. The run lies an eighth of the wire outside the facet, so the stone's own notch flattens the
        // claw onto it. A kink anywhere folds the tube through itself: rings must stand further apart than they tilt.
        let bend = 0.8 * d;
        let lean = PRONG_LEAN.atan();
        let turn = std::f64::consts::FRAC_PI_2 - slope;
        let hold = (0.95 * d).min(0.6 * inward / slope.cos().max(0.2));
        // Where the run starts: over the girdle's edge, lifted off the facet by an eighth of the wire.
        let lift = 0.12 * d;
        let run_from = [lift * slope.sin(), g + lift * slope.cos() + 0.04];
        let arc_end = [run_from[0] + 0.15 * d * turn.sin(), run_from[1] - 0.15 * d * turn.cos()];
        let start = [arc_end[0] - bend * (turn.cos() - lean.cos()), arc_end[1] - bend * (turn.sin() + lean.sin())];
        let base_z = (-depth - 0.8).min(start[1] - 0.4);
        let foot = start[0] - (start[1] - base_z) * PRONG_LEAN;
        let steps = (((start[1] - base_z) / 0.3).ceil() as usize).max(2);
        let (mut line, mut rs): (Vec<[f64; 2]>, Vec<f64>) = (0..steps).map(|k| {
            let t = k as f64 / steps as f64;
            ([foot + (start[0] - foot) * t, base_z + (start[1] - base_z) * t], (0.60 - 0.10 * t) * d)
        }).unzip();
        for k in 0..=10 {
            let t = k as f64 / 10.0;
            let psi = -lean + (turn + lean) * t;
            line.push([start[0] + bend * (psi.cos() - lean.cos()), start[1] + bend * (psi.sin() + lean.sin())]);
            rs.push((0.5 - 0.08 * t) * d);
        }
        let end = *line.last().unwrap();
        line.push([end[0] - (hold + 0.15 * d) * turn.sin(), end[1] + (hold + 0.15 * d) * turn.cos()]);
        rs.push(0.36 * d);
        let path: Vec<P3> = line.iter().map(|q| [o[0] + n[0] * q[0], o[1] + n[1] * q[0], q[1]]).collect();
        parts.push(tube(&path, &rs, 14, (false, true)));
    }
    // Rails thread the prongs' own axes at their heights.
    let rail = |z: f64, r: f64| {
        let offset = 0.2 * d + z * PRONG_LEAN;
        let section: Vec<Station> = (0..10).map(|k| {
            let a = TAU * (k as f64 + 0.31) / 10.0;
            st(1.0, offset + r * a.sin(), z + r * a.cos())
        }).collect();
        sweep(&plan, &section, plan.segments())
    };
    if gem.w_mm >= 1.6 { parts.push(rail(-depth + 0.18, 0.45 * d)); }
    if gem.w_mm >= 2.4 { parts.push(rail(-0.45 * depth, 0.36 * d)); }
    parts
}

/// Everything a seat of this kind adds and cuts, made once per stone and fit and kept.
pub fn parts(gem: Gem, kind: SolidKind, fit: Fit) -> Result<Arc<Parts>, Snag> {
    type Key = (SolidKind, [u64; 2], u8, u8, [i64; 3]);
    static CACHE: std::sync::Mutex<Vec<(Key, Arc<Parts>)>> = std::sync::Mutex::new(Vec::new());
    let q = |v: f64| (v * 1e4).round() as i64;
    let key: Key = (kind, [gem.w_mm.to_bits(), gem.l_mm.to_bits()], gem.cut as u8, gem.form as u8, [q(fit.surface_z), fit.through_mm.map_or(-1, q), fit.prongs as i64]);
    if let Some((_, hit)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return Ok(hit.clone());
    }
    let mut out = Parts::default();
    match kind {
        SolidKind::None => {}
        SolidKind::Flush => out.cut.push(bur(gem, &fit)),
        SolidKind::Bead => {
            out.beads = beads(gem, &fit);
            out.cut.push(bur(gem, &fit));
        }
        SolidKind::Prong => {
            out.add.push(claw_head(gem, fit.prongs)?);
            if fit.surface_z > -gem.pavilion_mm() - 0.05 {
                out.cut.extend(relief(gem, fit.surface_z.min(-0.1) + 0.3, (0.18 * gem.w_mm).max(0.3), &fit));
            }
        }
        SolidKind::Bezel => {
            out.add.push(collet(gem));
            let ledge = (0.09 * gem.w_mm).clamp(0.18, 0.45);
            out.cut.extend(relief(gem, -girdle_half(gem) - ledge - 0.05, ledge - 0.04, &fit));
        }
    }
    let out = Arc::new(out);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= 64 { cache.remove(0); }
    cache.push((key, out.clone()));
    Ok(out)
}

/// Every seat's cutters as they stand on the ring, for a ghost drawn over the live result: a soup of
/// triangles in the viewports' interleaved layout (position, normal, two colours), like the stones'.
pub fn ghost_vertices(design: &crate::RingDesign, lib: &crate::AlphaLibrary) -> Vec<f32> {
    let mut out = Vec::new();
    for (part, _) in placed_parts(design, lib, false) {
        for f in &part.f {
            let [a, b, c] = f.map(|i| part.v[i as usize]);
            let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
            for p in [a, b, c] {
                out.extend_from_slice(&[p[0] as f32, p[1] as f32, p[2] as f32, (n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
            }
        }
    }
    out
}

/// The design with no seat carrying a solid: what the viewport builds with live cuts switched off.
pub fn without_solids(design: &crate::RingDesign) -> crate::RingDesign {
    fn strip(stack: &mut crate::field::LayerStack) {
        for e in &mut stack.layers {
            match &mut e.layer {
                crate::field::Layer::SeatPad(s) => hold(s, s.gem),
                crate::field::Layer::SeatRun(r) => { let g = r.gem; hold(&mut r.seat, Some(g)) }
                crate::field::Layer::Group(g) => strip(&mut g.stack),
                _ => {}
            }
        }
    }
    // The stone stays where its solid stood it.
    fn hold(seat: &mut crate::field::SeatPadLayer, gem: Option<Gem>) {
        if seat.solid.is_none() { return; }
        if let Some(g) = gem { seat.set_depth_mm.get_or_insert(seat.girdle_drop_mm(g)); }
        seat.solid = SolidKind::None;
    }
    let mut d = design.clone();
    strip(&mut d.layers);
    d
}

/// Each seat's placed solids in ring space: the cutters, or with `heads` the parts joined on.
fn placed_parts(design: &crate::RingDesign, lib: &crate::AlphaLibrary, heads: bool) -> Vec<(Solid, usize)> {
    let ctx = design.field_context();
    let inner = design.inner_radius_mm();
    let mut out = Vec::new();
    for (i, (stone, frame)) in crate::stones::stone_frames(design).into_iter().filter(|(s, _)| !s.seat.solid.is_none()).enumerate() {
        let Ok(p) = parts(stone.gem, stone.seat.solid, fit_of(design, lib, &ctx, inner, &stone, &frame)) else { continue };
        let place = csg::Frame { origin: frame.girdle, x: frame.long, y: frame.short, z: frame.normal };
        for part in if heads { &p.add } else { &p.cut } {
            out.push((part.placed(&place), i));
        }
    }
    out
}

/// How a stone's seat meets the metal it stands in.
fn fit_of(design: &crate::RingDesign, lib: &crate::AlphaLibrary, ctx: &crate::FieldContext, inner: f64, stone: &crate::setstone::SetStone, frame: &crate::stones::StoneFrame) -> Fit {
    let stand_off = stone.stand_off_mm();
    let here = crate::field::Uv { u: ctx.u_of_theta(stone.theta_deg), v: stone.v_mm };
    let relief = if design.imported_base.is_some() { stone.seat.height_mm } else { design.layers.height(here, ctx, lib) };
    let bare = [frame.girdle[0] - frame.normal[0] * stand_off, frame.girdle[1] - frame.normal[1] * stand_off];
    let r = bare[0].hypot(bare[1]);
    let outward = if r > 1e-9 { (frame.normal[0] * bare[0] + frame.normal[1] * bare[1]) / r } else { 0.0 };
    let through = (stone.seat.through && outward > 0.75 && stone.gem.form == GemForm::Faceted)
        .then(|| stand_off + (r - inner).max(0.0) / outward + 0.6);
    Fit { surface_z: relief - stand_off, through_mm: through, prongs: stone.seat.prongs }
}

/// A seat as the sand pattern carries it: its made solid left to the bench, its stock where the finished
/// seat needs it, and a raised drill mark at its centre. Returns whether anything was left out.
pub fn pattern_seat(seat: &mut crate::field::SeatPadLayer) -> bool {
    if seat.solid.is_none() { return false; }
    if let Some(g) = seat.gem {
        seat.set_depth_mm.get_or_insert(seat.girdle_drop_mm(g));
        if seat.mark_mm <= 0.0 {
            seat.mark_mm = (0.4 * g.w_mm).clamp(0.6, 1.0);
        }
    }
    seat.solid = SolidKind::None;
    true
}

/// What resolving the seats' solids did to a build.
#[derive(Clone, Debug, Default)]
pub struct Applied {
    /// Stones whose solid was placed and resolved.
    pub resolved: usize,
    /// Faces the solids account for.
    pub faces: usize,
    pub ms: u128,
    /// What could not be resolved, by seat.
    pub notes: Vec<String>,
    /// The layer path behind each stone a [`crate::Mesh::origin`] names, in the order it counts them.
    pub paths: Vec<Vec<usize>>,
}

/// Whether any enabled seat in the design carries a solid.
pub fn any(design: &crate::RingDesign) -> bool {
    crate::setstone::set_stones(design).iter().any(|s| !s.seat.solid.is_none())
}

/// Place every seat's solid on the built band and resolve it: every head first, then every cut, so a
/// neighbour's bead never fills a seat already cut. A solid that will not resolve is left out and said.
pub fn apply(design: &crate::RingDesign, lib: &crate::AlphaLibrary, mesh: &mut crate::Mesh) -> Applied {
    let mut out = Applied::default();
    let stones: Vec<_> = crate::stones::stone_frames(design).into_iter().filter(|(s, _)| !s.seat.solid.is_none()).collect();
    if stones.is_empty() || mesh.faces.is_empty() {
        return out;
    }
    let clock = crate::mesh::BuildClock::start();
    out.paths = stones.iter().map(|(s, _)| s.path.clone()).collect();
    let ctx = design.field_context();
    let inner = design.inner_radius_mm();
    let band_vertices = mesh.vertices.len();
    let mut solid = Solid {
        v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(),
        f: mesh.faces.clone(),
    };
    // Which stone's solid each appended vertex belongs to, by the vertex count after its operation.
    let mut spans: Vec<(usize, u32)> = Vec::new();
    let mut placed: Vec<(usize, Arc<Parts>, csg::Frame, String)> = Vec::new();
    for (i, (stone, frame)) in stones.iter().enumerate() {
        let fit = fit_of(design, lib, &ctx, inner, stone, frame);
        match parts(stone.gem, stone.seat.solid, fit) {
            Ok(p) => placed.push((i, p, csg::Frame { origin: frame.girdle, x: frame.long, y: frame.short, z: frame.normal }, stone.label.clone())),
            Err(e) => out.notes.push(format!("{}: its {} could not be made ({e})", stone.label, stone.seat.solid.label().to_lowercase())),
        }
    }
    let mut done = vec![true; stones.len()];
    // Beads first, in the ring's own space: two stones raising a bead in the same gap share one between them.
    let mut beads: Vec<(P3, f64, usize, u32)> = Vec::new();
    for (i, p, frame, _) in &placed {
        if p.beads.is_empty() { continue; }
        // The band under this seat, for standing each bead on the metal actually beneath it: the seat
        // centre's height is not the surface's a bead's reach away, where a dome has fallen off.
        let reach = p.beads.iter().map(|(c, r)| c[0].hypot(c[1]) + 2.0 * r).fold(0.0, f64::max) + 0.5;
        let near: Vec<[P3; 3]> = solid.f.iter().filter_map(|f| {
            let t = f.map(|k| solid.v[k as usize]);
            t.iter().any(|q| (0..3).map(|k| (q[k] - frame.origin[k]).powi(2)).sum::<f64>() < reach * reach).then_some(t)
        }).collect();
        for (centre, r) in &p.beads {
            let above = frame.point([centre[0], centre[1], centre[2] + 6.0]);
            let Some(drop) = first_hit(&near, above, [-frame.z[0], -frame.z[1], -frame.z[2]]) else { continue };
            // No further down than a bead's own height: past that it is off the seat's shoulder, and left out.
            if drop > 6.0 + 2.0 * r { continue; }
            let c = frame.point([centre[0], centre[1], centre[2] + 6.0 - drop + 0.15 * r]);
            match beads.iter_mut().find(|b| (0..3).map(|k| (b.0[k] - c[k]).powi(2)).sum::<f64>().sqrt() < 1.4 * b.1.max(*r)) {
                Some(b) => {
                    let n = b.3 as f64;
                    b.0 = std::array::from_fn(|k| (b.0[k] * n + c[k]) / (n + 1.0));
                    b.1 = b.1.max(*r);
                    b.3 += 1;
                }
                None => beads.push((c, *r, *i, 1)),
            }
        }
    }
    for (c, r, i, _) in &beads {
        match csg::combine(&solid, &ball(*c, *r, 8), Op::Union) {
            Ok(next) => {
                solid = next;
                spans.push((solid.v.len(), *i as u32));
            }
            Err(e) => out.notes.push(format!("{}: a bead could not be raised ({e})", stones[*i].0.label)),
        }
    }
    for (op, pick) in [(Op::Union, 0usize), (Op::Subtract, 1)] {
        for (i, p, frame, label) in &placed {
            for part in if pick == 0 { &p.add } else { &p.cut } {
                match csg::combine(&solid, &part.placed(frame), op) {
                    Ok(next) => {
                        solid = next;
                        spans.push((solid.v.len(), *i as u32));
                    }
                    Err(e) => {
                        done[*i] = false;
                        out.notes.push(format!("{label}: {} ({e})", if pick == 0 { "its head could not be joined to the band" } else { "its seat could not be cut" }));
                    }
                }
            }
        }
    }
    out.resolved = placed.iter().filter(|(i, ..)| done[*i]).count();
    if spans.is_empty() {
        out.ms = clock.ms();
        return out;
    }
    let stone_of = |vertex: usize| spans.iter().find(|(end, _)| vertex < *end).map_or(0, |(_, s)| *s);
    let origin: Vec<u32> = (0..solid.v.len()).map(|v| if v < band_vertices { v as u32 } else { crate::mesh::SOLID_VERTEX + stone_of(v) }).collect();
    let band_faces = mesh.faces.len();
    *mesh = into_mesh(solid, &mesh.normals, origin);
    out.faces = mesh.faces.len().saturating_sub(band_faces);
    out.ms = clock.ms();
    out
}

/// Distance along a ray to the first face it meets.
fn first_hit(faces: &[[P3; 3]], origin: P3, dir: P3) -> Option<f64> {
    let sub = |a: P3, b: P3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut best: Option<f64> = None;
    for [a, b, c] in faces {
        let (e1, e2) = (sub(*b, *a), sub(*c, *a));
        let h = cross(dir, e2);
        let det = dot(e1, h);
        if det.abs() < 1e-14 { continue; }
        let s = sub(origin, *a);
        let u = dot(s, h) / det;
        let q = cross(s, e1);
        let v = dot(dir, q) / det;
        let t = dot(e2, q) / det;
        if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

/// The resolved solid as a mesh: the band keeps the normals it was swept with, and every face a solid
/// touched gets corner normals that hold a crease wherever its neighbours turn more than `CREASE_DEG`.
fn into_mesh(mut solid: Solid, band_normals: &[crate::Vec3], origin: Vec<u32>) -> crate::Mesh {
    const CREASE_DEG: f64 = 38.0;
    let map = solid.compact();
    let mut from = vec![u32::MAX; solid.v.len()];
    for (old, new) in map.iter().enumerate() {
        if *new != u32::MAX { from[*new as usize] = origin[old]; }
    }
    let n = solid.v.len();
    let sub = |a: P3, b: P3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let unit = |a: P3| { let l = dot(a, a).sqrt(); if l > 1e-300 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 0.0, 1.0] } };
    let face_n: Vec<P3> = solid.f.iter().map(|f| {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        unit(cross(sub(b, a), sub(c, a)))
    }).collect();
    let angle_at = |f: &[u32; 3], k: usize| {
        let (p, q, r) = (solid.v[f[k] as usize], solid.v[f[(k + 1) % 3] as usize], solid.v[f[(k + 2) % 3] as usize]);
        dot(unit(sub(q, p)), unit(sub(r, p))).clamp(-1.0, 1.0).acos()
    };
    let is_band = |v: u32| from[v as usize] < crate::mesh::SOLID_VERTEX;
    // Faces round each vertex a solid made.
    let mut start = vec![0u32; n + 1];
    for f in &solid.f { for &v in f { if !is_band(v) { start[v as usize + 1] += 1; } } }
    for i in 0..n { start[i + 1] += start[i]; }
    let mut fill = start.clone();
    let mut around = vec![0u32; start[n] as usize];
    for (fi, f) in solid.f.iter().enumerate() {
        for &v in f {
            if !is_band(v) {
                around[fill[v as usize] as usize] = fi as u32;
                fill[v as usize] += 1;
            }
        }
    }
    let cos_crease = CREASE_DEG.to_radians().cos();
    let to_v3 = |p: P3| crate::Vec3(p[0] as f32, p[1] as f32, p[2] as f32);
    let mut normals = vec![crate::Vec3(0.0, 0.0, 1.0); n];
    for v in 0..n {
        if is_band(v as u32) {
            normals[v] = band_normals.get(from[v] as usize).copied().unwrap_or(crate::Vec3(0.0, 0.0, 1.0));
        } else {
            let mut sum = [0.0; 3];
            for &fi in &around[start[v] as usize..start[v + 1] as usize] {
                let f = &solid.f[fi as usize];
                let k = f.iter().position(|x| *x as usize == v).unwrap_or(0);
                let w = angle_at(f, k);
                sum = [sum[0] + face_n[fi as usize][0] * w, sum[1] + face_n[fi as usize][1] * w, sum[2] + face_n[fi as usize][2] * w];
            }
            normals[v] = to_v3(unit(sum));
        }
    }
    let mut corner_normals = Vec::new();
    for (fi, f) in solid.f.iter().enumerate() {
        if f.iter().all(|v| is_band(*v)) { continue; }
        let mine = face_n[fi];
        let corners = std::array::from_fn(|k| {
            let v = f[k];
            if is_band(v) { return normals[v as usize]; }
            let mut sum = [0.0; 3];
            for &gi in &around[start[v as usize] as usize..start[v as usize + 1] as usize] {
                let theirs = face_n[gi as usize];
                if dot(mine, theirs) < cos_crease { continue; }
                let g = &solid.f[gi as usize];
                let w = angle_at(g, g.iter().position(|x| *x == v).unwrap_or(0));
                sum = [sum[0] + theirs[0] * w, sum[1] + theirs[1] * w, sum[2] + theirs[2] * w];
            }
            to_v3(unit(if dot(sum, sum) > 1e-20 { sum } else { mine }))
        });
        corner_normals.push((fi as u32, corners));
    }
    crate::Mesh { vertices: solid.v.iter().map(|p| to_v3(*p)).collect(), normals, faces: solid.f, corner_normals, origin: from }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gem::GemCut;

    fn sound(s: &Solid, what: &str) {
        assert_eq!(s.open_edges(), (0, 0), "{what} is closed");
        assert!(s.volume() > 0.0, "{what} is wound outward: {}", s.volume());
    }

    #[test]
    fn every_tool_is_a_closed_outward_solid_for_every_cut() {
        let fit = Fit { surface_z: 0.4, through_mm: Some(2.4), prongs: 0 };
        for &cut in GemCut::ALL {
            for w in [1.3, 3.0, 6.0] {
                let gem = Gem::calibrated(cut, w);
                sound(&envelope(gem, 0.02), "the stone");
                sound(&bur(gem, &fit), "the bur");
                sound(&collet(gem), "the collet");
                sound(&relief(gem, -0.4, 0.3, &fit).unwrap(), "the relief");
                for (c, r) in beads(gem, &fit) { sound(&ball(c, r, 8), "a bead"); }
            }
        }
        sound(&envelope(Gem::cabochon(GemCut::Oval, 6.0), 0.02), "a cabochon");
        sound(&collet(Gem::cabochon(GemCut::Oval, 6.0)), "a cabochon's collet");
    }

    #[test]
    fn a_claw_head_is_one_solid_and_its_claws_bear_on_the_stone() {
        for (cut, w, claws) in [(GemCut::Round, 6.0, 4), (GemCut::Round, 5.0, 6), (GemCut::Oval, 5.0, 6), (GemCut::Princess, 4.0, 4), (GemCut::Marquise, 3.0, 6), (GemCut::Round, 1.5, 4)] {
            let gem = Gem::calibrated(cut, w);
            for part in claw_parts(gem, 0) {
                assert_eq!(csg::self_crossings(&part), 0, "{cut:?} {w}: no claw or rail folds through itself");
            }
            let head = claw_head(gem, 0).unwrap_or_else(|e| panic!("{cut:?} {w}: {e}"));
            sound(&head, "the head");
            let plan = Plan::of(gem);
            assert_eq!(plan.claw_angles(0.max(claws)).len(), claws as usize);
            // No metal inside the stone; metal over its crown at every claw.
            let stone = envelope(gem, 0.0);
            let inside = head.v.iter().filter(|p| csg::inside(&stone, **p) == Some(true)).count();
            assert_eq!(inside, 0, "{cut:?}: the head stays out of the stone");
            let (_, hi) = head.bounds().unwrap();
            assert!(hi[2] > 0.3 * gem.crown_mm(), "{cut:?}: claws stand over the girdle: {}", hi[2]);
            let over = head.v.iter().filter(|p| p[2] > girdle_half(gem) + 0.05 && {
                let q = plan.point(p[1].atan2(p[0]));
                p[0].hypot(p[1]) < q[0].hypot(q[1]) - 0.05
            }).count();
            assert!(over > 20, "{cut:?}: claws reach in over the crown: {over}");
        }
    }

    fn ring_with(kind: SolidKind, gem: Gem, thetas: &[f64], through: bool) -> crate::RingDesign {
        use crate::field::{Layer, LayerEntry, SeatPadLayer, SeatStyle};
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 5.0;
        d.profile.thickness_mm = 2.2;
        let v = d.field_context().crest_v_mm;
        for (k, theta) in thetas.iter().enumerate() {
            let mut pad = SeatPadLayer { theta_deg: *theta, v_mm: v, style: SeatStyle::GypsyMound, blend_mm: 0.5, solid: kind, through, ..Default::default() };
            pad.fit_stone(gem);
            pad.height_mm = 0.0;
            d.layers.layers.push(LayerEntry::new(format!("Seat {k}"), Layer::SeatPad(pad)));
        }
        d
    }

    fn built(d: &crate::RingDesign) -> crate::BuildResult {
        let params = crate::BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() };
        crate::mesh::try_build(d, &crate::AlphaLibrary::builtin(), params).unwrap()
    }

    #[test]
    fn every_kind_resolves_into_one_watertight_ring_that_clears_its_stone() {
        let gem = Gem::calibrated(GemCut::Round, 2.5);
        let bare = built(&ring_with(SolidKind::None, gem, &[90.0], false));
        assert!(bare.mesh.origin.is_empty() && bare.mesh.corner_normals.is_empty() && bare.solids.resolved == 0);
        for kind in [SolidKind::Flush, SolidKind::Bead, SolidKind::Prong, SolidKind::Bezel] {
            let d = ring_with(kind, gem, &[90.0], false);
            let b = built(&d);
            assert!(b.solids.notes.is_empty(), "{kind:?}: {:?}", b.solids.notes);
            assert_eq!(b.solids.resolved, 1);
            assert!(b.report.validation.watertight, "{kind:?}: {:?}", b.report.validation);
            let dv = b.report.volume_mm3 - bare.report.volume_mm3;
            match kind {
                SolidKind::Flush => assert!(dv < -1.0, "a bur only takes metal away: {dv}"),
                SolidKind::Prong | SolidKind::Bezel => assert!(dv > 1.0, "a head adds metal: {dv}"),
                _ => {}
            }
            // The stone as the viewport draws it touches no metal it was not cut for.
            let (_, frame) = &crate::stones::stone_frames(&d)[0];
            let place = csg::Frame { origin: frame.girdle, x: frame.long, y: frame.short, z: frame.normal };
            let ring = Solid { v: b.mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: b.mesh.faces.clone() };
            let stone = envelope(gem, -0.01).placed(&place);
            let buried = stone.v.iter().filter(|p| csg::inside(&ring, **p) == Some(true)).count();
            assert_eq!(buried, 0, "{kind:?}: the stone sits in its seat, not in the metal");
            // Every vertex knows where it came from, and hard faces are listed in order.
            assert_eq!(b.mesh.origin.len(), b.mesh.vertices.len());
            assert!(b.mesh.origin.iter().any(|o| *o >= crate::mesh::SOLID_VERTEX) && b.mesh.origin.iter().any(|o| *o < crate::mesh::SOLID_VERTEX));
            assert!(!b.mesh.corner_normals.is_empty() && b.mesh.corner_normals.windows(2).all(|w| w[0].0 < w[1].0));
        }
    }

    #[test]
    fn a_pilot_drilled_through_opens_the_bore_and_neighbours_share_their_beads() {
        let gem = Gem::calibrated(GemCut::Round, 2.0);
        let blind = built(&ring_with(SolidKind::Flush, gem, &[90.0], false));
        let through = built(&ring_with(SolidKind::Flush, gem, &[90.0], true));
        assert!(through.report.validation.watertight && through.report.volume_mm3 < blind.report.volume_mm3 - 0.2);
        // A hole through the wall is a handle: the Euler characteristic drops by two.
        let euler = |m: &crate::Mesh| m.vertices.len() as i64 - (m.faces.len() as i64 * 3 / 2) + m.faces.len() as i64;
        assert_eq!(euler(&blind.mesh), 0, "a ring is a torus");
        assert_eq!(euler(&through.mesh), -2, "and a drilled one a double torus");
        // Three stones at pitch: four gaps' worth of beads would be twelve; shared, the two inner gaps give eight.
        let pitch = 2.0 + 2.0 * bead_radius_mm(gem) * 0.9;
        let step = pitch / (through.report.outer_diameter_mm * 0.5) * 180.0 / PI;
        let row = ring_with(SolidKind::Bead, gem, &[90.0 - step, 90.0, 90.0 + step], false);
        let b = built(&row);
        assert!(b.report.validation.watertight && b.solids.notes.is_empty(), "{:?}", b.solids.notes);
        let alone = built(&ring_with(SolidKind::Bead, gem, &[90.0], false));
        let bead = 4.0 / 3.0 * PI * bead_radius_mm(gem).powi(3);
        let gained = b.report.volume_mm3 - blind.report.volume_mm3;
        let one = alone.report.volume_mm3 - blind.report.volume_mm3;
        assert!(gained < 3.0 * one - 1.2 * bead, "shared beads are raised once: {gained} against three of {one}");
    }

    #[test]
    fn a_sand_pattern_leaves_the_cut_to_the_bench_and_marks_where_the_drill_starts() {
        use crate::castability::{CastProcess, casting_pattern};
        let gem = Gem::calibrated(GemCut::Round, 2.5);
        let mut d = ring_with(SolidKind::Flush, gem, &[90.0], true);
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() };
        let finished = crate::mesh::try_build(&d, &lib, params).unwrap();
        let pattern = crate::mesh::try_build_pattern(&d, &lib, params).unwrap();
        let bare = built(&ring_with(SolidKind::None, gem, &[90.0], false));
        assert_eq!(finished.solids.resolved, 1);
        assert!(pattern.solids.resolved == 0 && pattern.mesh.origin.is_empty(), "nothing is cut from a sand pattern");
        let dot = pattern.report.volume_mm3 - bare.report.volume_mm3;
        assert!(dot > 0.02 && dot < 0.3, "the mark is a raised dot, a tenth of a cubic millimetre: {dot}");
        assert!(finished.report.volume_mm3 < bare.report.volume_mm3 - 1.0, "and the bur takes the dot with the seat");
        // The source design is untouched, and the verdict names what it left out.
        let (p, _) = casting_pattern(&d);
        let crate::field::Layer::SeatPad(seat) = &p.layers.layers[0].layer else { panic!() };
        assert!(seat.solid.is_none() && seat.mark_mm >= 0.6);
        let field = crate::castability::attributed_field_report(&d, &lib, &d.draft, 96, 64);
        assert!(field.notes.iter().any(|n| n.contains("drill mark")), "{:?}", field.notes);
        // Lost wax casts its seats in place: the design is its own pattern.
        CastProcess::LostWax.apply(&mut d.draft);
        assert_eq!(crate::mesh::try_build_pattern(&d, &lib, params).unwrap().solids.resolved, 1);
    }

    #[test]
    fn a_head_is_made_once() {
        let gem = Gem::calibrated(GemCut::Round, 4.0);
        let fit = Fit { surface_z: -2.0, through_mm: None, prongs: 4 };
        let a = parts(gem, SolidKind::Prong, fit).unwrap();
        let b = parts(gem, SolidKind::Prong, fit).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(a.cut.is_empty() && a.add.len() == 1);
    }
}

