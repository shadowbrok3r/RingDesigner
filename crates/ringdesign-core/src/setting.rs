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
use crate::csg::{self, Op, Parent, Snag, Solid, P3};
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
/// Half the girdle's own thickness, mm.
pub fn girdle_half_mm(gem: Gem) -> f64 { girdle_half(gem) }
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

/// [`sweep`] with the section interval behind each face: face `k` spans stations `seg[k]` and `seg[k] + 1`,
/// the last of a ring wrapping to the first.
pub fn sweep_traced(plan: &Plan, section: &[Station], around: usize) -> (Solid, Vec<u32>) {
    let solid = sweep(plan, section, around);
    let n = around.max(8);
    let is_pole = |s: &Station| s.s == 0.0 && s.o == 0.0;
    let ball = section.first().is_some_and(is_pole) && section.last().is_some_and(is_pole);
    let rows: Vec<u32> = section.iter().enumerate().filter(|(_, s)| !is_pole(s)).map(|(i, _)| i as u32).collect();
    let bands = if ball { rows.len().saturating_sub(1) } else { rows.len() };
    let mut seg = Vec::with_capacity(solid.f.len());
    for r in &rows[..bands] {
        seg.extend(std::iter::repeat_n(*r, 2 * n));
    }
    if let (true, Some(last)) = (ball, rows.last()) {
        for _ in 0..n {
            seg.extend([0, *last]);
        }
    }
    (solid, seg)
}

/// A closed solid whose every face names the patch it lies on: a claw, a rail, a bearing.
#[derive(Clone, Debug, Default)]
pub struct Named {
    pub solid: Solid,
    /// The patch behind each face.
    pub patch: Vec<u32>,
    /// Patch names in first-use order.
    pub names: Vec<String>,
}

impl Named {
    /// A solid that is one patch.
    pub fn whole(solid: Solid, name: impl Into<String>) -> Self {
        Self { patch: vec![0; solid.f.len()], solid, names: vec![name.into()] }
    }

    /// A swept solid whose section interval `k` is called `names[k]`; equal names share a patch.
    pub fn swept(plan: &Plan, section: &[Station], names: &[&str], around: usize) -> Self {
        let (solid, seg) = sweep_traced(plan, section, around);
        let mut out = Self { solid, patch: Vec::with_capacity(seg.len()), names: Vec::new() };
        for s in seg {
            let name = names.get(s as usize).or(names.last()).copied().unwrap_or("Face");
            let p = out.name_index(name);
            out.patch.push(p);
        }
        out
    }

    /// The index of the patch called `name`, added when it is new.
    fn name_index(&mut self, name: &str) -> u32 {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i as u32,
            None => {
                self.names.push(name.to_string());
                (self.names.len() - 1) as u32
            }
        }
    }

    /// The name of the patch behind face `face`.
    pub fn face_name(&self, face: usize) -> Option<&str> {
        self.names.get(*self.patch.get(face)? as usize).map(String::as_str)
    }

    /// How many faces each patch holds, by patch index.
    pub fn census(&self) -> Vec<usize> {
        let mut out = vec![0; self.names.len()];
        for p in &self.patch {
            if let Some(n) = out.get_mut(*p as usize) {
                *n += 1;
            }
        }
        out
    }

    pub fn placed(&self, frame: &csg::Frame) -> Self {
        Self { solid: self.solid.placed(frame), patch: self.patch.clone(), names: self.names.clone() }
    }

    /// `other` joined on, each face keeping its own patch; the solid is [`csg::combine`]'s own bytes.
    pub fn union(self, other: &Named) -> Result<Self, Snag> {
        self.combine(other, Op::Union)
    }

    /// `other` joined on, cut away or kept in common, each face keeping the patch it lies on; the solid is
    /// [`csg::combine`]'s own bytes, and patches of the same name are one.
    pub fn combine(self, other: &Named, op: Op) -> Result<Self, Snag> {
        let t = csg::combine_traced(&self.solid, &other.solid, op, None)?;
        let mut out = Self { solid: Solid::default(), patch: Vec::new(), names: self.names };
        let theirs: Vec<u32> = other.names.iter().map(|n| out.name_index(n)).collect();
        out.patch = t
            .parent
            .iter()
            .map(|q| match q {
                Parent::A(f) => self.patch.get(*f as usize).copied().unwrap_or(0),
                Parent::B(f) => other.patch.get(*f as usize).and_then(|p| theirs.get(*p as usize)).copied().unwrap_or(0),
            })
            .collect();
        out.solid = t.solid;
        Ok(out)
    }

    /// Every part joined into one solid, as [`csg::union_all`] joins them.
    pub fn union_all(parts: Vec<Named>) -> Result<Self, Snag> {
        let mut it = parts.into_iter().filter(|p| !p.solid.is_empty());
        let Some(mut out) = it.next() else { return Err(Snag::Empty) };
        for p in it {
            out = out.union(&p)?;
        }
        Ok(out)
    }

    /// `tool` taken away; a face the cut leaves takes the patch of the face beside it, so a claw's notch is the claw's.
    pub fn notched(self, tool: &Solid) -> Result<Self, Snag> {
        let t = csg::combine_traced(&self.solid, tool, Op::Subtract, None)?;
        let mut patch: Vec<u32> = t.parent.iter().map(|q| match q {
            Parent::A(f) => self.patch.get(*f as usize).copied().unwrap_or(0),
            Parent::B(_) => u32::MAX,
        }).collect();
        adopt_neighbours(&t.solid, &mut patch);
        Ok(Self { solid: t.solid, patch, names: self.names })
    }

    /// Drop vertices no face uses; faces and their patches keep their order.
    pub fn compact(&mut self) {
        self.solid.compact();
    }
}

/// Every face marked `u32::MAX` takes the patch of a neighbour across an edge, spreading until none is left.
fn adopt_neighbours(solid: &Solid, patch: &mut [u32]) {
    let mut by_edge: std::collections::HashMap<(u32, u32), Vec<usize>> = std::collections::HashMap::new();
    for (i, f) in solid.f.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            by_edge.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }
    let mut queue: std::collections::VecDeque<usize> = (0..patch.len()).filter(|i| patch[*i] != u32::MAX).collect();
    while let Some(i) = queue.pop_front() {
        let f = solid.f[i];
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            for &j in by_edge.get(&(a.min(b), a.max(b))).map(Vec::as_slice).unwrap_or(&[]) {
                if patch[j] == u32::MAX {
                    patch[j] = patch[i];
                    queue.push_back(j);
                }
            }
        }
    }
    for p in patch.iter_mut().filter(|p| **p == u32::MAX) {
        *p = 0;
    }
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
    envelope_named(gem, clear).solid
}

/// [`envelope`] with its table, crown, girdle and pavilion (a cabochon's dome, girdle and back) named.
pub fn envelope_named(gem: Gem, clear: f64) -> Named {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let (section, names): (Vec<Station>, Vec<&str>) = match gem.form {
        GemForm::Faceted => (
            vec![
                pole(c + clear), st(0.52, clear, c + clear), st(0.78, clear, 0.55 * c + clear * 0.8),
                st(1.0, clear, g + clear * 0.5), st(1.0, clear, -g - clear * 0.5), pole(-p - clear),
            ],
            vec!["Table", "Crown", "Crown", "Girdle", "Pavilion"],
        ),
        GemForm::Cabochon => {
            let mut s = vec![pole(c + clear)];
            for k in (0..8).rev() {
                let t = k as f64 / 8.0;
                s.push(st((1.0 - t * t).sqrt(), clear, c * t + clear * t));
            }
            s.push(st(1.0, clear, -clear));
            s.push(pole(-clear));
            let mut names = vec!["Dome"; 8];
            names.extend(["Girdle", "Back"]);
            (s, names)
        }
    };
    Named::swept(&plan, &section, &names, plan.segments())
}

/// The setting bur: a bright-cut bevel at the surface, a lip left over the girdle, the girdle wall,
/// a bearing cone at the pavilion's own angle, and a pilot below it.
pub fn bur(gem: Gem, fit: &Fit) -> Solid {
    bur_named(gem, fit).solid
}

/// [`bur`] with its clearance, bevel, lip, girdle wall, bearing and pilot named.
pub fn bur_named(gem: Gem, fit: &Fit) -> Named {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let bevel = (0.07 * gem.w_mm).clamp(0.08, 0.22);
    let lip = (0.03 * gem.w_mm).clamp(0.03, 0.08);
    let top = fit.surface_z.max(c) + 1.2;
    let mut s = vec![(pole(top), "Clearance"), (st(1.0, CLEAR + bevel, top), "Clearance")];
    let under = fit.surface_z - bevel - lip - CLEAR;
    if under > g + 0.04 {
        s.push((st(1.0, CLEAR + bevel, fit.surface_z + 0.01), "Bevel"));
        s.push((st(1.0, -lip, under), "Lip"));
    }
    s.push((st(1.0, CLEAR, g), "Girdle wall"));
    match gem.form {
        GemForm::Faceted => {
            let blind = p + 0.15;
            s.push((st(1.0, CLEAR, -g), "Bearing"));
            s.push((st(0.52, CLEAR, -g - 0.48 * p), "Pilot"));
            s.push((st(0.52, CLEAR, -fit.through_mm.unwrap_or(blind).max(g + 0.48 * p + 0.1)), "Pilot"));
            s.push((pole(-fit.through_mm.unwrap_or(blind).max(g + 0.48 * p + 0.1)), "Pilot"));
        }
        GemForm::Cabochon => {
            s.push((st(1.0, CLEAR, -g), "Bed"));
            s.push((pole(-g - 0.04), "Bed"));
        }
    }
    let (section, names): (Vec<Station>, Vec<&str>) = s.into_iter().unzip();
    Named::swept(&plan, &section, &names, plan.segments())
}

/// Room for a pavilion that hangs below its head into the band, and the pilot under it.
pub fn relief(gem: Gem, from_z: f64, inset: f64, fit: &Fit) -> Option<Solid> {
    relief_named(gem, from_z, inset, fit).map(|n| n.solid)
}

/// [`relief`] with its pocket and pilot named.
pub fn relief_named(gem: Gem, from_z: f64, inset: f64, fit: &Fit) -> Option<Named> {
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
    let mut names = vec!["Relief"; s.len() - 3];
    names.extend(["Pilot", "Pilot"]);
    Some(Named::swept(&plan, &s, &names, plan.segments()))
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

/// A collet's own wall thickness for a stone, mm.
pub fn collet_wall_mm(gem: Gem) -> f64 { (0.10 * gem.w_mm + 0.22).clamp(0.35, 0.9) }
/// How far a collet's lip rises over the girdle, as a share of the crown.
pub fn collet_lip(gem: Gem) -> f64 {
    match gem.form { GemForm::Faceted => 0.40, GemForm::Cabochon => 0.30 }
}
/// How far a collet reaches below the girdle, mm.
pub fn collet_depth_mm(gem: Gem) -> f64 { collet_depth(gem) }

/// A collet: tapered wall, bearing ledge at the pavilion's angle, a lip leaning up the crown.
pub fn collet(gem: Gem) -> Solid {
    collet_named(gem, collet_wall_mm(gem), collet_lip(gem), -collet_depth(gem)).solid
}

/// [`collet`] with its wall, lip share and base height chosen, and its rim, wall, base, inner wall,
/// bearing, girdle seat and lip named. A base below the collet's own depth lengthens the wall to it.
pub fn collet_named(gem: Gem, wall: f64, lip: f64, base_z: f64) -> Named {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let ledge = (0.09 * gem.w_mm).clamp(0.18, 0.45);
    let depth = -base_z.min(-collet_depth(gem));
    let top = g + lip * c;
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
    let names = ["Rim", "Wall", "Base", "Inner wall", "Bearing", "Girdle seat", "Lip", "Lip", "Rim"];
    Named::swept(&plan, &section, &names, plan.segments())
}

/// A claw head: tapered prongs leaning out from a base rail, bent over the crown, with a gallery rail between,
/// joined into one solid and then notched by the stone itself so every claw bears on it.
pub fn claw_head(gem: Gem, prongs: u32) -> Result<Solid, Snag> {
    claw_head_named(gem, prongs, prong_wire_mm(gem), Rails::Seat, None).map(|n| n.solid)
}

/// The height of the metal under a point of the stone's girdle plane, in the stone's frame; `None` where there is none.
pub type Floor<'a> = &'a dyn Fn([f64; 2]) -> Option<f64>;

/// How far a claw's foot reaches past the metal under it, mm.
pub const FOOT_SINK_MM: f64 = 0.5;
/// How far below the head's own base a claw may lengthen to find metal, mm.
pub const CLAW_REACH_MM: f64 = 6.0;

/// [`claw_head`] with its wire and rails chosen, every claw and rail named, and each claw reaching past the
/// metal `floor` finds under its own foot; the stone's notch in a claw is the claw's.
pub fn claw_head_named(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>) -> Result<Named, Snag> {
    let head = Named::union_all(claw_parts_named(gem, prongs, wire_mm, rails, floor))?;
    let mut head = head.notched(&envelope(gem, 0.02))?;
    head.compact();
    Ok(head)
}

/// The claws and rails of a head before they are joined, each closed on its own.
pub fn claw_parts(gem: Gem, prongs: u32) -> Vec<Solid> {
    claw_parts_named(gem, prongs, prong_wire_mm(gem), Rails::Seat, None).into_iter().map(|n| n.solid).collect()
}

/// The rails that tie a head's claws together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rails {
    /// A base rail on stones from 1.6 mm, and a gallery rail at 0.45 of the depth from 2.4 mm: the seats' own head.
    Seat,
    /// A basket: this many rails, evenly from the base rail up to 0.35 of the pavilion under the girdle.
    Basket(u32),
}

/// Claws a head carries: `prongs` when it names three or more, else the cut's own.
pub fn claw_count(gem: Gem, prongs: u32) -> u32 {
    let plan = Plan::of(gem);
    if prongs >= 3 { prongs } else if plan.pow < 1.8 || (plan.a / plan.b > 1.25 && plan.pow < 3.0) { 6 } else { 4 }
}

/// [`claw_parts`] with the wire and the rails chosen, each part named: claws first, then rails from the base up.
/// Where `floor` finds metal under a claw's foot deeper than the head's own base, the claw lengthens to reach
/// [`FOOT_SINK_MM`] past it.
pub fn claw_parts_named(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>) -> Vec<Named> {
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let d = wire_mm;
    let count = claw_count(gem, prongs);
    let depth = p + 0.2;
    let mut parts = Vec::new();
    for (claw, phi) in plan.claw_angles(count).into_iter().enumerate() {
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
        // Down the claw's own line, whose foot moves in as it lengthens, until the inner edge of its foot enters
        // metal from above; a claw with none within reach keeps its own length and is held by its rails.
        let base_z = floor.map_or(base_z, |floor| {
            let inner = |z: f64| {
                let foot = start[0] - (start[1] - z) * PRONG_LEAN - 0.60 * d;
                [o[0] + n[0] * foot, o[1] + n[1] * foot]
            };
            let mut z = base_z;
            while z > base_z - CLAW_REACH_MM {
                if let Some(metal) = floor(inner(z)).filter(|metal| z <= metal - FOOT_SINK_MM) {
                    // Found only far below the top, the foot came down beside a wall rather than onto metal.
                    return if z >= metal - FOOT_SINK_MM - 0.5 { z } else { base_z };
                }
                z -= 0.05;
            }
            base_z
        });
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
        parts.push(Named::whole(tube(&path, &rs, 14, (false, true)), format!("Claw {}", claw + 1)));
    }
    // Rails thread the prongs' own axes at their heights.
    let rail = |z: f64, r: f64, name: String| {
        let offset = 0.2 * d + z * PRONG_LEAN;
        let section: Vec<Station> = (0..10).map(|k| {
            let a = TAU * (k as f64 + 0.31) / 10.0;
            st(1.0, offset + r * a.sin(), z + r * a.cos())
        }).collect();
        Named::whole(sweep(&plan, &section, plan.segments()), name)
    };
    match rails {
        Rails::Seat => {
            if gem.w_mm >= 1.6 { parts.push(rail(-depth + 0.18, 0.45 * d, "Base rail".into())); }
            if gem.w_mm >= 2.4 { parts.push(rail(-0.45 * depth, 0.36 * d, "Gallery rail".into())); }
        }
        Rails::Basket(n) => {
            let (low, high) = (-depth + 0.18, -g - 0.35 * p);
            let n = n.clamp(1, 6);
            for k in 0..n {
                let t = if n > 1 { k as f64 / (n - 1) as f64 } else { 0.0 };
                let name = if k == 0 { "Base rail".to_string() } else { format!("Gallery rail {k}") };
                parts.push(rail(low + (high - low) * t, if k == 0 { 0.45 * d } else { 0.36 * d }, name));
            }
        }
    }
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

/// An outline extruded off the built surface and joined to it, or cut from it: a struck stamp. Its top
/// follows the surface it stands on at one height, its walls stand along the surface's own normal at
/// its centre, and its silhouette is as crisp as its polygon — which no height field holds, because a
/// wall there is one cell wide and steps with the grid wherever it runs across it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    pub name: String,
    /// Where it stands, in the chart every layer uses: degrees round the ring, mm across the section.
    pub theta_deg: f64,
    pub v_mm: f64,
    /// Turn of the outline's x axis from the ring's tangent, degrees.
    #[serde(default)]
    pub rot_deg: f64,
    /// Closed outline in mm in the stamp's own plane, counter-clockwise seen from outside the metal.
    pub outline: Vec<[f64; 2]>,
    /// How far its top stands over the surface, mm. For a cut, how far over it the cutter starts.
    pub height_mm: f64,
    /// How far under the surface it reaches, mm. For a cut, its depth.
    pub sink_mm: f64,
    /// Walls lean out toward the surface by this much, degrees. Ignored by a cut.
    #[serde(default)]
    pub draft_deg: f64,
    /// Taken away rather than joined on.
    #[serde(default)]
    pub cut: bool,
    /// Made at the bench after the pour: in the finished ring, never in the pattern.
    #[serde(default)]
    pub bench: bool,
    /// Stand its walls along the mould's pull — the finger's axis — rather than along the surface's own
    /// normal. On a head's wall the two differ by the wall's lean, and a stamp struck square to a leaning
    /// wall tucks one edge under it: measured 0.35 mm on a factory signet's cheek.
    #[serde(default)]
    pub along_pull: bool,
}

/// The most outline points a stamp may carry.
pub const MAX_STAMP_POINTS: usize = 512;

impl Stamp {
    /// Where the stamp stands: its origin on the bare surface, `z` out of the metal, `x` its outline's own.
    pub fn frame(&self, design: &crate::RingDesign, ctx: &crate::FieldContext) -> csg::Frame {
        let (point, mut normal, mut along, mut across) = crate::stones::surface_frame(design, ctx, self.theta_deg, self.v_mm);
        if self.along_pull && normal[2].abs() > 0.25 {
            normal = [0.0, 0.0, normal[2].signum()];
            let flat = [along[0], along[1], 0.0];
            let l = flat[0].hypot(flat[1]).max(1e-9);
            along = [flat[0] / l, flat[1] / l, 0.0];
            across = [-normal[2] * along[1], normal[2] * along[0], 0.0];
        }
        let (sin, cos) = self.rot_deg.to_radians().sin_cos();
        let mut x: [f64; 3] = std::array::from_fn(|k| along[k] * cos + across[k] * sin);
        let mut y: [f64; 3] = std::array::from_fn(|k| across[k] * cos - along[k] * sin);
        // Across the parting line the walls must stand square to the pull. A crest's normal on real stock
        // tips a few degrees off horizontal, and a wall along it crosses the parting plane by its length
        // times the tip: 0.05 mm of undercut beside a moon on the factory signet 017.
        let height = |p: &[f64; 2]| point[2] + p[0] * x[2] + p[1] * y[2];
        let straddles = self.outline.iter().any(|p| height(p) < 0.0) && self.outline.iter().any(|p| height(p) > 0.0);
        if !self.along_pull && straddles && normal[0].hypot(normal[1]) > 0.5 {
            let l = normal[0].hypot(normal[1]);
            normal = [normal[0] / l, normal[1] / l, 0.0];
            let d = x[0] * normal[0] + x[1] * normal[1];
            let flat = [x[0] - normal[0] * d, x[1] - normal[1] * d, x[2]];
            let m = (flat[0] * flat[0] + flat[1] * flat[1] + flat[2] * flat[2]).sqrt().max(1e-12);
            x = flat.map(|v| v / m);
            y = [normal[1] * x[2] - normal[2] * x[1], normal[2] * x[0] - normal[0] * x[2], normal[0] * x[1] - normal[1] * x[0]];
        }
        csg::Frame { origin: point, x, y, z: normal }
    }

    /// The outline as a plain prism, unprojected: enough for a ghost.
    fn prism(&self) -> Option<Solid> {
        let n = self.outline.len();
        if !(3..=MAX_STAMP_POINTS).contains(&n) { return None; }
        let cap = cap_faces(&self.outline, 10.0, &[])?;
        let flat: Vec<f64> = vec![0.0; cap.0.len()];
        Some(self.assemble(&cap.0, &cap.1, &flat))
    }

    /// Closed solid from cap points, their triangles and the surface height under each.
    fn assemble(&self, points: &[[f64; 2]], tris: &[[u32; 3]], surface: &[f64]) -> Solid {
        let n = self.outline.len();
        let count = points.len() as u32;
        let lean = if self.cut { 0.0 } else { (self.height_mm + self.sink_mm).max(0.0) * self.draft_deg.clamp(0.0, 30.0).to_radians().tan() };
        let mut out = Solid::default();
        for (p, z) in points.iter().zip(surface) { out.v.push([p[0], p[1], z + self.height_mm]); }
        for (i, (p, z)) in points.iter().zip(surface).enumerate() {
            let mut q = *p;
            if i < n && lean > 0.0 {
                // Out along the corner's own bisector, mitred, so a drafted wall stays a plane.
                let (a, b) = (self.outline[(i + n - 1) % n], self.outline[(i + 1) % n]);
                let e0 = unit2([p[0] - a[0], p[1] - a[1]]);
                let e1 = unit2([b[0] - p[0], b[1] - p[1]]);
                let m = [e0[1] + e1[1], -(e0[0] + e1[0])];
                let l = (m[0] * m[0] + m[1] * m[1]).sqrt();
                if l > 1e-9 {
                    let k = (2.0 / l).min(2.5) * lean / l;
                    q = [p[0] + m[0] * k, p[1] + m[1] * k];
                }
            }
            out.v.push([q[0], q[1], z - self.sink_mm]);
        }
        for t in tris {
            out.f.push(*t);
            out.f.push([t[0] + count, t[2] + count, t[1] + count]);
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            out.f.push([i, i + count, j + count]);
            out.f.push([i, j + count, j]);
        }
        out
    }

    /// The stamp standing on the band: every cap point dropped onto the mesh beneath it.
    fn solid(&self, frame: &csg::Frame, band: &Solid) -> Result<Solid, String> {
        // A bench stamp never meets the sand. Split, its edges would lie on its blank's own split, and the
        // two solids meeting edge on edge leave zero-length slivers in the join.
        if self.bench {
            return self.stand(frame, band, &[]);
        }
        // No wall may straddle the parting plane: a facet that does faces the wrong mould half over part
        // of its length, by half its own turn — 0.035 mm of undercut on an eighty-sided moon. So the
        // outline gets a corner wherever it crosses.
        // The split and the chord sit a tenth of a micron over it: exactly on it, the stamp's walls and the
        // band's own parting loop are coplanar, and the boolean cannot decide which crosses which.
        const OVER: f64 = 1e-4;
        let height = |p: [f64; 2]| frame.origin[2] + p[0] * frame.x[2] + p[1] * frame.y[2] - OVER;
        let mut split = Vec::with_capacity(self.outline.len() + 4);
        let mut on_line = Vec::new();
        for i in 0..self.outline.len() {
            let (a, b) = (self.outline[i], self.outline[(i + 1) % self.outline.len()]);
            split.push(a);
            let (za, zb) = (height(a), height(b));
            if za.abs() <= 1e-6 {
                on_line.push(split.len() - 1);
            } else if za * zb < 0.0 && zb.abs() > 1e-6 {
                let t = za / (za - zb);
                split.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                on_line.push(split.len() - 1);
            }
        }
        // And no facet of the top may straddle it either, or its lower part faces the wrong mould half:
        // a chord along the parting line, between the points where the outline crosses it, held as an edge.
        let along = [-frame.y[2], frame.x[2]];
        on_line.sort_by(|a, b| (split[*a][0] * along[0] + split[*a][1] * along[1]).total_cmp(&(split[*b][0] * along[0] + split[*b][1] * along[1])));
        let chords: Vec<(usize, usize)> = on_line.windows(2).map(|w| (w[0], w[1])).filter(|(a, b)| {
            let (p, q) = (split[*a], split[*b]);
            (p[0] - q[0]).hypot(p[1] - q[1]) > 1e-6 && inside_polygon(&split, [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])])
        }).collect();
        let this = Stamp { outline: split, ..self.clone() };
        this.stand(frame, band, &chords)
    }

    fn stand(&self, frame: &csg::Frame, band: &Solid, chords: &[(usize, usize)]) -> Result<Solid, String> {
        let n = self.outline.len();
        if !(3..=MAX_STAMP_POINTS).contains(&n) { return Err(format!("needs 3 to {MAX_STAMP_POINTS} outline points")); }
        let area: f64 = (0..n).map(|i| { let (a, b) = (self.outline[i], self.outline[(i + 1) % n]); a[0] * b[1] - b[0] * a[1] }).sum();
        if area <= 1e-6 { return Err("its outline must run counter-clockwise and enclose something".into()); }
        let reach = self.outline.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max) + 1.0;
        let near: Vec<[P3; 3]> = band.f.iter().filter_map(|f| {
            let t = f.map(|k| band.v[k as usize]);
            t.iter().any(|q| (0..3).map(|k| (q[k] - frame.origin[k]).powi(2)).sum::<f64>() < reach * reach).then_some(t)
        }).collect();
        let (points, tris) = cap_faces(&self.outline, (reach / 14.0).clamp(0.12, 0.35), chords).ok_or("its outline will not triangulate")?;
        const ABOVE: f64 = 8.0;
        let down = [-frame.z[0], -frame.z[1], -frame.z[2]];
        let mut surface = Vec::with_capacity(points.len());
        for p in &points {
            let hit = first_hit(&near, frame.point([p[0], p[1], ABOVE]), down).ok_or("it runs off the edge of the surface it stands on")?;
            surface.push(ABOVE - hit);
        }
        Ok(self.assemble(&points, &tris, &surface).placed(frame))
    }
}

/// The lit face of a moon as an outline: the limb a half circle on the `-x` side, the terminator the
/// half ellipse it really is — bulging to `+x` for a gibbous moon, straight at the half, curving back
/// inside the limb for a crescent. `lit` runs 0 (new) to 1 (full). A crescent's horns come to nothing,
/// which no metal holds, so they are squared off where the lune is still `horn` of the radius tall.
pub fn moon_outline(radius: f64, lit: f64, horn: f64) -> Vec<[f64; 2]> {
    let k = 2.0 * lit.clamp(0.02, 1.0) - 1.0;
    let top = if k < 0.0 { horn.clamp(0.3, 1.0) } else { 1.0 };
    let reach = top.asin();
    let steps = 40;
    let mut out: Vec<[f64; 2]> = (0..=steps).map(|i| {
        let a = PI - reach + 2.0 * reach * i as f64 / steps as f64;
        [radius * a.cos(), radius * a.sin()]
    }).collect();
    for i in 0..=steps {
        let y = radius * top * (2.0 * i as f64 / steps as f64 - 1.0);
        let p = [k * (radius * radius - y * y).max(0.0).sqrt(), y];
        if out.iter().all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > 1e-6) { out.push(p); }
    }
    out
}

/// What the bench takes from a half moon cast as a blank to leave the crescent of [`moon_outline`]:
/// everything inside the terminator and beyond the squared horns, with a margin past the blank's own
/// walls so no two faces coincide.
pub fn crescent_cutter(radius: f64, lit: f64, horn: f64, margin: f64) -> Vec<[f64; 2]> {
    let k = 2.0 * lit.clamp(0.02, 0.49) - 1.0;
    let top = horn.clamp(0.3, 1.0) * radius;
    let far = radius + margin;
    let mut out = vec![[margin, -far], [margin, far], [-far, far], [-far, top]];
    let steps = 40;
    for i in 0..=steps {
        let y = top * (1.0 - 2.0 * i as f64 / steps as f64);
        out.push([k * (radius * radius - y * y).max(0.0).sqrt(), y]);
    }
    out.extend([[-far, -top], [-far, -far]]);
    out
}

fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = v[0].hypot(v[1]).max(1e-12);
    [v[0] / l, v[1] / l]
}

fn inside_polygon(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// A polygon's cap: its own points first and in order, then a grid of inner points `pitch` apart so the
/// cap can follow a curved surface, triangulated with the outline held as edges.
fn cap_faces(outline: &[[f64; 2]], pitch: f64, chords: &[(usize, usize)]) -> Option<(Vec<[f64; 2]>, Vec<[u32; 3]>)> {
    use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
    let n = outline.len();
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut index = std::collections::HashMap::new();
    let mut handles = Vec::with_capacity(n);
    for p in outline {
        let h = cdt.insert(Point2::new(p[0], p[1])).ok()?;
        if index.insert(h, points.len() as u32).is_some() { return None; }
        points.push(*p);
        handles.push(h);
    }
    for i in 0..n {
        let (a, b) = (handles[i], handles[(i + 1) % n]);
        if !cdt.can_add_constraint(a, b) { return None; }
        cdt.add_constraint(a, b);
    }
    // Chords are held as edges too, but are not the outline: crossing one is still inside. Each carries
    // points at the cap's pitch, or its top would run straight across the dome it spans and sag under it.
    let mut chord_edges = std::collections::HashSet::new();
    for (a, b) in chords {
        let (pa, pb) = (outline[*a], outline[*b]);
        let steps = (((pb[0] - pa[0]).hypot(pb[1] - pa[1]) / pitch).ceil() as usize).max(1);
        let mut run = vec![handles[*a]];
        for k in 1..steps {
            let t = k as f64 / steps as f64;
            let q = [pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t];
            let h = cdt.insert(Point2::new(q[0], q[1])).ok()?;
            if let std::collections::hash_map::Entry::Vacant(v) = index.entry(h) {
                v.insert(points.len() as u32);
                points.push(q);
            }
            run.push(h);
        }
        run.push(handles[*b]);
        for w in run.windows(2) {
            if w[0] == w[1] || !cdt.can_add_constraint(w[0], w[1]) { return None; }
            cdt.add_constraint(w[0], w[1]);
            chord_edges.insert((w[0].min(w[1]), w[0].max(w[1])));
        }
    }
    let (lo, hi) = outline.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let segments: Vec<([f64; 2], [f64; 2])> = (0..n).map(|i| (outline[i], outline[(i + 1) % n])).chain(chords.iter().map(|(a, b)| (outline[*a], outline[*b]))).collect();
    let clear = |p: [f64; 2]| segments.iter().all(|&(a, b)| {
        let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / (ex * ex + ey * ey).max(1e-18)).clamp(0.0, 1.0);
        (p[0] - a[0] - ex * t).hypot(p[1] - a[1] - ey * t) > pitch * 0.45
    });
    let (mut y, mut row) = (lo[1] + pitch * 0.5, 0);
    while y < hi[1] && points.len() < 6000 {
        let mut x = lo[0] + pitch * if row % 2 == 0 { 0.5 } else { 1.0 };
        while x < hi[0] {
            if inside_polygon(outline, [x, y]) && clear([x, y]) {
                let h = cdt.insert(Point2::new(x, y)).ok()?;
                index.entry(h).or_insert_with(|| { points.push([x, y]); points.len() as u32 - 1 });
            }
            x += pitch;
        }
        y += pitch * 0.866;
        row += 1;
    }
    // Inside is read off the outline itself: a face against a hull edge the outline does not own is
    // outside, and crossing any outline edge flips it. A centroid test cannot be trusted here — a point
    // split onto a chord lands a hair inside it, and the sliver between them has its centroid on the line.
    let mut inside = std::collections::HashMap::new();
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
            let [a, b] = e.vertices().map(|v| v.fix());
            let outline_edge = cdt.is_constraint_edge(e.as_undirected().fix()) && !chord_edges.contains(&(a.min(b), a.max(b)));
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(next.fix()) {
                v.insert(if outline_edge { !here } else { here });
                queue.push_back(next.fix());
            }
        }
    }
    let mut tris = Vec::new();
    for face in cdt.inner_faces() {
        if inside.get(&face.fix()).copied().unwrap_or(false) {
            let v = face.vertices();
            tris.push([index[&v[0].fix()], index[&v[1].fix()], index[&v[2].fix()]]);
        }
    }
    (!tris.is_empty()).then_some((points, tris))
}

/// Every seat's cutters as they stand on the ring, for a ghost drawn over the live result: a soup of
/// triangles in the viewports' interleaved layout (position, normal, two colours), like the stones'.
pub fn ghost_vertices(design: &crate::RingDesign, lib: &crate::AlphaLibrary) -> Vec<f32> {
    let mut out = Vec::new();
    let ctx = design.field_context();
    let cutters = design.stamps.iter().filter(|s| s.cut).filter_map(|s| Some((s.prism()?.placed(&s.frame(design, &ctx)), 0usize)));
    for (part, _) in placed_parts(design, lib, false).into_iter().chain(cutters) {
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
    d.stamps.clear();
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
    /// Stamps joined on or cut away.
    pub stamped: usize,
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
    !design.stamps.is_empty() || crate::setstone::set_stones(design).iter().any(|s| !s.seat.solid.is_none())
}

/// One boolean on the running solid: its census is paid while it is still the band as swept, and skipped
/// once it is a combine's own closed output.
fn chain(solid: &Solid, tool: &Solid, op: Op, vouched: bool) -> Result<Solid, Snag> {
    if vouched { csg::combine_unchecked(solid, tool, op, None).map(|t| t.solid) } else { csg::combine(solid, tool, op) }
}

/// Place every seat's solid on the built band and resolve it: every head first, then every cut, so a
/// neighbour's bead never fills a seat already cut. A solid that will not resolve is left out and said.
pub fn apply(design: &crate::RingDesign, lib: &crate::AlphaLibrary, mesh: &mut crate::Mesh) -> Applied {
    let mut out = Applied::default();
    let stones: Vec<_> = crate::stones::stone_frames(design).into_iter().filter(|(s, _)| !s.seat.solid.is_none()).collect();
    if (stones.is_empty() && design.stamps.is_empty()) || mesh.faces.is_empty() {
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
    let mut vouched = false;
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
        match chain(&solid, &ball(*c, *r, 8), Op::Union, vouched) {
            Ok(next) => {
                solid = next;
                vouched = true;
                spans.push((solid.v.len(), *i as u32));
            }
            Err(e) => out.notes.push(format!("{}: a bead could not be raised ({e})", stones[*i].0.label)),
        }
    }
    // Stamps stand on the band as it was swept, so each is made against it before anything is joined.
    let stamps: Vec<(usize, Solid)> = design.stamps.iter().enumerate().filter_map(|(k, s)| match s.solid(&s.frame(design, &ctx), &solid) {
        Ok(made) => Some((k, made)),
        Err(why) => { out.notes.push(format!("{}: {why}", s.name)); None }
    }).collect();
    for (op, pick) in [(Op::Union, 0usize), (Op::Subtract, 1)] {
        for (k, made) in stamps.iter().filter(|(k, _)| design.stamps[*k].cut == (pick == 1)) {
            match chain(&solid, made, op, vouched) {
                Ok(next) => {
                    solid = next;
                    vouched = true;
                    spans.push((solid.v.len(), (stones.len() + k) as u32));
                    out.stamped += 1;
                }
                Err(e) => out.notes.push(format!("{}: could not be {} ({e})", design.stamps[*k].name, if pick == 0 { "joined to the band" } else { "cut" })),
            }
        }
        for (i, p, frame, label) in &placed {
            for part in if pick == 0 { &p.add } else { &p.cut } {
                match chain(&solid, &part.placed(frame), op, vouched) {
                    Ok(next) => {
                        solid = next;
                        vouched = true;
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
    *mesh = crate::parts::into_mesh(solid, &mesh.normals, origin);
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
        // A hair of slack: a ray along the parting plane runs exactly through the band's own edge loop there,
        // and rounding can put it a hair outside both faces that share the edge.
        const SLACK: f64 = 1e-9;
        if u >= -SLACK && v >= -SLACK && u + v <= 1.0 + SLACK && t > 0.0 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
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
    fn a_cap_keeps_what_its_outline_holds_even_where_a_point_sits_a_hair_inside_a_chord() {
        // A circle with a point split onto a chord exactly as the parting-plane split computes it,
        // `a + (b - a) t`: rounding leaves it off the chord by 1e-17. The centroid test this replaced
        // kept the sliver between them — the waxing moons' non-manifold caps.
        let mut outline: Vec<[f64; 2]> = (0..40).map(|i| { let t = TAU * i as f64 / 40.0; [1.6 * t.cos(), 1.6 * t.sin()] }).collect();
        outline[24] = [-1.294427190999916, -0.9404564036679569];
        outline[25] = [-1.1313708498984762, -1.131370849898476];
        outline.insert(25, [-1.2658173660885563, -0.9739542047519781]);
        let (points, tris) = cap_faces(&outline, 0.3, &[]).unwrap();
        let area = |o: &[[f64; 2]]| 0.5 * (0..o.len()).map(|i| o[i][0] * o[(i + 1) % o.len()][1] - o[(i + 1) % o.len()][0] * o[i][1]).sum::<f64>();
        let covered: f64 = tris.iter().map(|t| area(&t.map(|i| points[i as usize]))).sum();
        assert!((covered - area(&outline)).abs() < 1e-9, "{covered} against {}", area(&outline));
        let mut uses = std::collections::HashMap::new();
        for t in &tris { for k in 0..3 { *uses.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1; } }
        assert!(uses.values().all(|n| *n == 1), "a directed edge is used twice");
        // The cap's boundary is the outline, edge for edge and the right way round.
        let n = outline.len() as u32;
        let boundary: std::collections::HashSet<(u32, u32)> = uses.keys().copied().filter(|(a, b)| !uses.contains_key(&(*b, *a))).collect();
        let want: std::collections::HashSet<(u32, u32)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
        assert_eq!(boundary, want);
        let stamp = Stamp { name: "Dent".into(), theta_deg: 90.0, v_mm: 0.0, rot_deg: 0.0, outline, height_mm: 0.4, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false };
        assert_eq!(stamp.prism().unwrap().open_edges(), (0, 0));
    }

    #[test]
    fn a_stamp_is_a_crisp_conforming_solid_and_the_bench_keeps_its_own_out_of_the_pattern() {
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 2.2;
        let v = d.field_context().crest_v_mm;
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 384, profile_steps: 128, ..Default::default() };
        let bare = crate::mesh::try_build(&d, &lib, params).unwrap();
        // Every phase is a closed, counter-clockwise outline, and the lit share is the area's.
        let full = moon_outline(1.6, 1.0, 0.85);
        let area = |o: &[[f64; 2]]| 0.5 * (0..o.len()).map(|i| o[i][0] * o[(i + 1) % o.len()][1] - o[(i + 1) % o.len()][0] * o[i][1]).sum::<f64>();
        assert!((area(&full) - PI * 1.6 * 1.6).abs() < 0.05, "{}", area(&full));
        assert!((area(&moon_outline(1.6, 0.5, 0.85)) - 0.5 * PI * 2.56).abs() < 0.03);
        assert!(area(&moon_outline(1.6, 0.25, 0.85)) > 0.0 && area(&crescent_cutter(1.6, 0.25, 0.85, 0.2)) > 0.0);
        // A half moon cast as a blank, and the bench's cut that leaves the crescent.
        let blank = Stamp { name: "Half moon".into(), theta_deg: 60.0, v_mm: v, rot_deg: 0.0, outline: moon_outline(1.6, 0.5, 0.85), height_mm: 0.45, sink_mm: 0.35, draft_deg: 0.0, cut: false, bench: false, along_pull: false };
        let cut = Stamp { name: "Crescent cut".into(), outline: crescent_cutter(1.6, 0.25, 0.85, 0.2), height_mm: 0.8, sink_mm: -0.02, cut: true, bench: true, ..blank.clone() };
        d.stamps = vec![blank, cut];
        let finished = crate::mesh::try_build(&d, &lib, params).unwrap();
        assert!(finished.solids.notes.is_empty(), "{:?}", finished.solids.notes);
        assert_eq!(finished.solids.stamped, 2);
        assert!(finished.report.validation.watertight, "{:?}", finished.report.validation);
        // The cut and its blank share a frame across the parting line, and the join leaves no sliver there.
        assert_eq!(finished.report.quality.degenerate_faces, 0);
        // Across the parting line only walls of what is poured may cross it: a sloped facet of the top
        // straddling z = 0 faces the wrong mould half on one side, which the chord along the line prevents
        // — and the chord's own points keep the top on the dome, or it runs straight under the arc and the
        // facets beside it tip.
        let pattern = crate::mesh::try_build_pattern(&d, &lib, params).unwrap();
        assert_eq!(pattern.report.quality.degenerate_faces, 0);
        let m = &pattern.mesh;
        for f in m.faces.iter().filter(|f| f.iter().any(|v| m.origin[*v as usize] >= crate::mesh::SOLID_VERTEX)) {
            let [a, b, c] = f.map(|v| m.vertices[v as usize]);
            let (lo, hi) = (a.2.min(b.2).min(c.2), a.2.max(b.2).max(c.2));
            if lo < -1e-3 && hi > 1e-3 {
                let n = crate::mesh::cross([(b.0 - a.0) as f64, (b.1 - a.1) as f64, (b.2 - a.2) as f64], [(c.0 - a.0) as f64, (c.1 - a.1) as f64, (c.2 - a.2) as f64]);
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!(n[2].abs() < 1e-3 * l, "a sloped facet crosses the parting line: z {lo}..{hi}, normal z {}", n[2] / l);
            }
        }
        // The same design builds the same mesh, vertex for vertex: a saved file has to reopen as it was.
        let again = crate::mesh::try_build(&d, &lib, params).unwrap();
        assert!(again.mesh.vertices == finished.mesh.vertices && again.mesh.faces == finished.mesh.faces, "a rebuild moved geometry");
        assert!(pattern.report.validation.watertight && pattern.solids.stamped == 1);
        let (half, crescent) = (pattern.report.volume_mm3 - bare.report.volume_mm3, finished.report.volume_mm3 - bare.report.volume_mm3);
        // The top follows the dome at one height, so the volume is the outline's area times it.
        let want = area(&moon_outline(1.6, 0.5, 0.85)) * 0.45;
        assert!((half - want).abs() < 0.12 * want, "a conforming half moon: {half} against {want}");
        let lune = area(&moon_outline(1.6, 0.25, 0.85)) * 0.45;
        assert!((crescent - lune).abs() < 0.2 * lune + 0.05, "the bench leaves the crescent: {crescent} against {lune}");
        // Switched off for a faster preview, the band is as it was swept.
        assert_eq!(crate::mesh::try_build(&without_solids(&d), &lib, params).unwrap().mesh.faces.len(), bare.mesh.faces.len());
        assert!(!ghost_vertices(&d, &lib).is_empty(), "and the cutter can be ghosted");
        // And what is poured pulls: no face of the cast stamp above the parting line faces down, nor one
        // below it up. (The bench's cut is a pocket, and is never poured.) Slivers along the seam are
        // lines with rounding for a normal; nothing that small holds sand.
        let cast = &pattern.mesh;
        for f in cast.faces.iter().filter(|f| f.iter().any(|v| cast.origin[*v as usize] >= crate::mesh::SOLID_VERTEX)) {
            let [a, b, c] = f.map(|v| cast.vertices[v as usize]);
            let n = crate::mesh::cross([(b.0 - a.0) as f64, (b.1 - a.1) as f64, (b.2 - a.2) as f64], [(c.0 - a.0) as f64, (c.1 - a.1) as f64, (c.2 - a.2) as f64]);
            let mid = (a.2 + b.2 + c.2) as f64 / 3.0;
            let area = 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            // A wall stands along the pull: its normal is level to within the mesh's f32 rounding, either way.
            assert!(area < 1e-5 || mid.abs() < 1e-3 || mid.signum() * n[2] / (2.0 * area) >= -1e-3, "a cast stamp face tips into its own mould half at z {mid}, area {area:.2e}");
        }
        // One that runs off the band is refused by name and nothing is half made.
        d.stamps[0].outline = moon_outline(9.0, 1.0, 0.85);
        let off = crate::mesh::try_build(&d, &lib, params).unwrap();
        assert!(off.solids.notes.iter().any(|n| n.contains("Half moon")) && off.report.validation.watertight, "{:?}", off.solids.notes);
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

    #[test]
    fn a_named_solid_is_its_plain_solid_with_every_face_named_and_claws_reach_the_floor() {
        let same = |a: &Solid, b: &Solid| a.v == b.v && a.f == b.f;
        for (cut, w) in [(GemCut::Round, 6.5), (GemCut::Oval, 5.0), (GemCut::Princess, 4.0), (GemCut::Marquise, 3.0)] {
            let gem = Gem::calibrated(cut, w);
            let fit = Fit { surface_z: 0.4, through_mm: Some(2.4), prongs: 0 };
            // The named variants are the seats' own solids, byte for byte, with a patch per face.
            let head = claw_head_named(gem, 0, prong_wire_mm(gem), Rails::Seat, None).unwrap();
            assert!(same(&head.solid, &claw_head(gem, 0).unwrap()), "{cut:?}");
            assert!(same(&bur_named(gem, &fit).solid, &bur(gem, &fit)), "{cut:?}");
            assert!(same(&relief_named(gem, -0.4, 0.3, &fit).unwrap().solid, &relief(gem, -0.4, 0.3, &fit).unwrap()), "{cut:?}");
            assert!(same(&envelope_named(gem, 0.02).solid, &envelope(gem, 0.02)), "{cut:?}");
            assert!(same(&collet_named(gem, collet_wall_mm(gem), collet_lip(gem), -collet_depth_mm(gem)).solid, &collet(gem)), "{cut:?}");
            assert_eq!(head.patch.len(), head.solid.f.len());
            let claws = head.names.iter().filter(|n| n.starts_with("Claw ")).count();
            assert_eq!(claws, claw_count(gem, 0) as usize, "{cut:?}");
            assert!(head.census().iter().all(|n| *n > 0), "{cut:?}: every patch owns faces, the notches their claws'");
            let sunk = Fit { surface_z: 1.2, ..fit };
            assert_eq!(bur_named(gem, &sunk).names, ["Clearance", "Bevel", "Lip", "Girdle wall", "Bearing", "Pilot"]);
        }
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        // A basket's rails run evenly from the base rail to just under the girdle.
        let basket = claw_head_named(gem, 4, prong_wire_mm(gem), Rails::Basket(3), None).unwrap();
        assert_eq!(basket.names, ["Claw 1", "Claw 2", "Claw 3", "Claw 4", "Base rail", "Gallery rail 1", "Gallery rail 2"]);
        // Over metal 2 mm below its own base a claw lengthens to reach it; with none in reach, it keeps its length.
        let own = claw_head_named(gem, 4, prong_wire_mm(gem), Rails::Seat, None).unwrap();
        let low = |s: &Solid| s.v.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        let floor = |_: [f64; 2]| Some(low(&own.solid) - 2.0);
        let reaching = claw_head_named(gem, 4, prong_wire_mm(gem), Rails::Seat, Some(&floor)).unwrap();
        assert!(low(&reaching.solid) < low(&own.solid) - 2.0 - FOOT_SINK_MM + 0.1, "{} against {}", low(&reaching.solid), low(&own.solid));
        let nothing = |_: [f64; 2]| None;
        assert!(same(&claw_head_named(gem, 4, prong_wire_mm(gem), Rails::Seat, Some(&nothing)).unwrap().solid, &own.solid));
    }
}


