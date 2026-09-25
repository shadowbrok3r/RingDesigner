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

/// [`sweep`] with each face's section interval: face `k` spans stations `seg[k]` and `seg[k] + 1`, a ring wrapping.
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

    /// `other` joined, cut or intersected, each face keeping its patch; the solid is [`csg::combine`]'s own bytes.
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

/// An oval wire with its first radius in the bending plane; invalid frames return no solid, and equal radii retain [`tube`]'s exact round path within the 4096-station and 128-vertex limits.
pub fn tube_oval(path: &[P3], radii: &[(f64, f64)], around: usize, dome: (bool, bool)) -> Solid {
    if path.len() < 2 || path.len() > 4096 || radii.len() != path.len()
        || path.iter().flatten().any(|v| !v.is_finite())
        || radii.iter().any(|(a, b)| !a.is_finite() || !b.is_finite() || *a <= 0.0 || *b <= 0.0) {
        return Solid::default();
    }
    let n = around.clamp(6, 128);
    let sub = |a: P3, b: P3| std::array::from_fn(|i| a[i] - b[i]);
    let add = |a: P3, b: P3, k: f64| std::array::from_fn(|i| a[i] + b[i] * k);
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let unit = |v: P3| { let l = dot(v, v).sqrt().max(1e-12); v.map(|x| x / l) };
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let m = path.len();
    if path.windows(2).any(|p| dot(sub(p[1], p[0]), sub(p[1], p[0])) < 1e-18) {
        return Solid::default();
    }
    if path.windows(3).any(|p| dot(sub(p[2], p[0]), sub(p[2], p[0])) < 1e-18) {
        return Solid::default();
    }
    let tangent = |i: usize| unit(sub(path[(i + 1).min(m - 1)], path[i.saturating_sub(1)]));
    let t0 = tangent(0);
    let in_plane = path.iter().skip(2).map(|p| {
        let v = sub(*p, path[0]);
        add(v, t0, -dot(v, t0))
    }).find(|v| dot(*v, *v) > 1e-12);
    let round = radii.iter().all(|(a, b)| a == b);
    let round_seed = if t0[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let seed = if round { round_seed } else { in_plane.unwrap_or(round_seed) };
    let mut u = unit(add(seed, t0, -dot(seed, t0)));
    for i in 0..m {
        let t = tangent(i);
        let projected = add(u, t, -dot(u, t));
        if dot(projected, projected) < 1e-18 { return Solid::default(); }
        u = unit(projected);
    }
    if round { return tube(path, &radii.iter().map(|r| r.0).collect::<Vec<_>>(), n, dome); }
    u = unit(add(seed, t0, -dot(seed, t0)));
    let mut rings = Vec::with_capacity(m + 8);
    let mut push = |c: P3, radii: (f64, f64), t: P3| {
        u = unit(add(u, t, -dot(u, t)));
        rings.push((c, radii, u, cross(u, t)));
    };
    if dome.0 {
        for j in (1..5).rev() {
            let a = PI * 0.5 * j as f64 / 5.0;
            push(add(path[0], t0, -radii[0].0.min(radii[0].1) * a.sin()), (radii[0].0 * a.cos(), radii[0].1 * a.cos()), t0);
        }
    }
    for i in 0..m { push(path[i], radii[i], tangent(i)); }
    let t1 = tangent(m - 1);
    if dome.1 {
        for j in 1..5 {
            let a = PI * 0.5 * j as f64 / 5.0;
            push(add(path[m - 1], t1, radii[m - 1].0.min(radii[m - 1].1) * a.sin()), (radii[m - 1].0 * a.cos(), radii[m - 1].1 * a.cos()), t1);
        }
    }
    let mut out = Solid::default();
    for &(c, (a, b), u, v) in &rings {
        for i in 0..n {
            let (s, co) = (TAU * (i as f64 + PHASE) / n as f64).sin_cos();
            out.v.push(std::array::from_fn(|k| c[k] + a * u[k] * co + b * v[k] * s));
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
    out.v.push(add(path[0], t0, if dome.0 { -radii[0].0.min(radii[0].1) } else { 0.0 }));
    out.v.push(add(path[m - 1], t1, if dome.1 { radii[m - 1].0.min(radii[m - 1].1) } else { 0.0 }));
    for i in 0..n {
        out.f.push([start, at(0, i), at(0, i + 1)]);
        out.f.push([at(rings.len() - 1, i), end, at(rings.len() - 1, i + 1)]);
    }
    out
}

/// A closed solid through anticlockwise rings of equal count from the top down, each end fanned from its own point, with each face's interval: band `k`, then `rings.len() - 1` for the top fan and `rings.len()` for the bottom.
pub fn loft_traced(rings: &[Vec<P3>], top: P3, bottom: P3) -> (Solid, Vec<u32>) {
    let n = rings.first().map_or(0, Vec::len);
    let mut out = Solid::default();
    let mut seg = Vec::new();
    if n < 3 || rings.len() < 2 || rings.iter().any(|r| r.len() != n) {
        return (out, seg);
    }
    for r in rings {
        out.v.extend_from_slice(r);
    }
    let at = |r: usize, i: usize| (r * n + i % n) as u32;
    for r in 0..rings.len() - 1 {
        for i in 0..n {
            out.f.push([at(r, i), at(r + 1, i), at(r + 1, i + 1)]);
            out.f.push([at(r, i), at(r + 1, i + 1), at(r, i + 1)]);
            seg.extend([r as u32; 2]);
        }
    }
    let last = rings.len() - 1;
    let (t, b) = (out.v.len() as u32, out.v.len() as u32 + 1);
    out.v.extend([top, bottom]);
    for i in 0..n {
        out.f.push([t, at(0, i), at(0, i + 1)]);
        seg.push(last as u32);
    }
    for i in 0..n {
        out.f.push([at(last, i), b, at(last, i + 1)]);
        seg.push(rings.len() as u32);
    }
    (out, seg)
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

/// [`collet`] with its wall, lip share and base height chosen, each interval of its section named.
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
/// Radial metal between a point of the stone's frame and the finger hole, mm; negative inside the hole.
pub type Wall<'a> = &'a dyn Fn(P3) -> f64;

/// How far a claw's foot reaches past the metal under it, mm.
pub const FOOT_SINK_MM: f64 = 0.5;
/// How far below the head's own base a claw may lengthen to find metal, mm.
pub const CLAW_REACH_MM: f64 = 6.0;
/// Margin a claw's sampled foot keeps over the wall, for the rim between its samples, mm.
const FOOT_WALL_SLACK_MM: f64 = 0.01;

/// How a head's claws met the band: those that found metal under their feet, and those the wall over the finger stopped first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    pub reached: u32,
    pub walled: u32,
}

/// The profile a claw follows from its existing foot to the crown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClawStyle {
    #[default]
    Wire,
    Talon,
    Fang,
    Tentacle,
    Thorn,
    Sepal,
}

/// How the claws are arranged round the girdle, in the stone's frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClawGrouping {
    #[default]
    Even,
    Feet,
    Jaws,
}

/// A rounded tip, or a tapered point along the last straight run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClawTip {
    #[default]
    Dome,
    Point,
}

/// The default preserves the original claw mesh, including its sampling and caps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClawOptions {
    pub style: ClawStyle,
    pub grouping: ClawGrouping,
    pub tip: ClawTip,
}

/// Why a made part cannot stand where its stone is.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadSnag {
    /// Its parts would not join or take the stone's notch.
    Csg(Snag),
    /// No claw finds the band before the wall over the finger.
    NoReach,
    /// A part would stand inside the wall over the finger, leaving `wall_mm` of metal; negative is inside the hole.
    Breaks { part: String, wall_mm: f64 },
    /// The foot leaves too little room for this claw's safe profile.
    ShortClaw { claw: u32, style: ClawStyle },
}

impl std::fmt::Display for HeadSnag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let wall = crate::mesh::MIN_WALL_MM;
        match self {
            Self::Csg(s) => write!(f, "would not resolve: {s}"),
            Self::NoReach => write!(f, "no claw reaches the band without breaking the {wall} mm wall over the finger; seat the stone further onto the band or choose a smaller one"),
            Self::Breaks { part, wall_mm } if *wall_mm < 0.0 => {
                write!(f, "{part} would reach {:.2} mm into the finger hole; seat the stone further onto the band or choose a smaller one", -wall_mm)
            }
            Self::Breaks { part, wall_mm } => {
                write!(f, "{part} would thin the wall over the finger to {wall_mm:.2} mm, under its {wall} mm; seat the stone further onto the band or choose a smaller one")
            }
            Self::ShortClaw { claw, style } => {
                let remedy = if *style == ClawStyle::Fang { "choose a thicker wire or another claw style" } else { "raise the stone or choose a thinner wire" };
                write!(f, "Claw {claw} has too little room for a safe {style:?} profile; {remedy}")
            }
        }
    }
}

impl From<Snag> for HeadSnag {
    fn from(s: Snag) -> Self {
        Self::Csg(s)
    }
}

/// The thinnest wall any vertex of `solid` leaves over the finger, mm.
pub fn thinnest_wall(solid: &Solid, wall: Wall) -> f64 {
    solid.v.iter().map(|p| wall(*p)).fold(f64::INFINITY, f64::min)
}

/// The first of `parts` that stands inside the wall over the finger, by name.
pub fn breaks_wall(parts: &[Named], wall: Wall) -> Option<HeadSnag> {
    parts.iter().find_map(|p| {
        let w = thinnest_wall(&p.solid, wall);
        (w < crate::mesh::MIN_WALL_MM).then(|| HeadSnag::Breaks { part: p.names.first().cloned().unwrap_or_else(|| "A part".into()), wall_mm: w })
    })
}

/// [`claw_head`] with its wire and rails chosen, every claw and rail named, each claw reaching the metal `floor` finds.
pub fn claw_head_named(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>) -> Result<Named, Snag> {
    let head = Named::union_all(claw_parts_named(gem, prongs, wire_mm, rails, floor))?;
    let mut head = head.notched(&envelope(gem, 0.02))?;
    head.compact();
    Ok(head)
}

/// [`claw_head_named`] floored by the wall over the finger: claws stop or shorten at it, and a head that cannot keep it is refused.
pub fn claw_head_within(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>, wall: Wall) -> Result<Named, HeadSnag> {
    let (parts, reach) = claw_parts_reach(gem, prongs, wire_mm, rails, floor, Some(wall));
    if floor.is_some() && reach.reached == 0 && reach.walled > 0 {
        return Err(HeadSnag::NoReach);
    }
    if let Some(snag) = breaks_wall(&parts, wall) {
        return Err(snag);
    }
    let head = Named::union_all(parts)?;
    let mut head = head.notched(&envelope(gem, 0.02))?;
    head.compact();
    Ok(head)
}

/// A chosen claw profile, with the same named rails, foot search and stone notch as [`claw_head_named`].
pub fn claw_head_named_styled(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>, options: ClawOptions) -> Result<Named, HeadSnag> {
    claw_head_styled(gem, prongs, wire_mm, rails, floor, None, options)
}

/// [`claw_head_named_styled`] keeping the wall over the finger, or refusing the head by name.
pub fn claw_head_within_styled(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>, wall: Wall, options: ClawOptions) -> Result<Named, HeadSnag> {
    claw_head_styled(gem, prongs, wire_mm, rails, floor, Some(wall), options)
}

fn claw_head_styled(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>, wall: Option<Wall>, options: ClawOptions) -> Result<Named, HeadSnag> {
    let (parts, reach) = claw_parts_reach_styled(gem, prongs, wire_mm, rails, floor, wall, options)?;
    if floor.is_some() && reach.reached == 0 && reach.walled > 0 { return Err(HeadSnag::NoReach); }
    if let Some(snag) = wall.and_then(|w| breaks_wall(&parts, w)) { return Err(snag); }
    let mut head = Named::union_all(parts)?.notched(&envelope(gem, 0.02))?;
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

/// [`claw_parts`] with the wire, rails and floor chosen, each part named: claws first, then rails from the base up.
pub fn claw_parts_named(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>) -> Vec<Named> {
    claw_parts_reach(gem, prongs, wire_mm, rails, floor, None).0
}

/// [`claw_parts_named`] with each claw's reach floored by `wall` when there is one, and how the claws met the band.
pub fn claw_parts_reach(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>, wall: Option<Wall>) -> (Vec<Named>, Reach) {
    claw_parts_reach_styled(gem, prongs, wire_mm, rails, floor, wall, ClawOptions::default()).expect("the legacy Wire path has no extra bend to fit")
}

fn grouped_claw_angles(plan: &Plan, count: u32, wire: f64, grouping: ClawGrouping) -> Vec<f64> {
    if grouping == ClawGrouping::Even { return plan.claw_angles(count); }
    let n = count.clamp(3, 8) as usize;
    let groups = if grouping == ClawGrouping::Jaws { 2 } else { (n / 2).max(2) };
    // Crowded toes spread towards equal spacing without wrapping into neighbouring groups.
    let gap = (1.45 * wire / plan.a.min(plan.b).max(0.1)).min(0.85 * TAU / n as f64);
    let phase = if grouping == ClawGrouping::Feet { PI * 0.5 } else { 0.0 };
    let mut angles = Vec::with_capacity(n);
    for group in 0..groups {
        let toes = n / groups + usize::from(group < n % groups);
        for toe in 0..toes {
            angles.push(phase + TAU * group as f64 / groups as f64 + (toe as f64 - (toes - 1) as f64 * 0.5) * gap);
        }
    }
    angles
}

/// Tangent arcs or a straight Fang cone, with normalized path distance; an unsafe unchanged foot yields no path.
fn shaped_claw_line(style: ClawStyle, wire: f64, foot: [f64; 2], target: [f64; 2], turn: f64, girdle: f64) -> Option<(Vec<[f64; 2]>, Vec<f64>)> {
    if style == ClawStyle::Fang {
        let target = [(-0.05 * wire).min(foot[0] - 0.05 * wire), girdle + 0.44 * wire];
        let tip_radius = (-target[0] + 0.20 * wire).max(0.10 * wire);
        let length = (target[0] - foot[0]).hypot(target[1] - foot[1]);
        if !length.is_finite() || wire <= 0.0 || tip_radius >= 0.60 * wire || target[1] <= foot[1] { return None; }
        let steps = (length / 0.12).ceil().clamp(2.0, 128.0) as usize;
        let along: Vec<_> = (0..=steps).map(|i| i as f64 / steps as f64).collect();
        let line = along.iter().map(|&t| std::array::from_fn(|i| foot[i] + t * (target[i] - foot[i]))).collect();
        return Some((line, along));
    }
    let lean = PRONG_LEAN.atan();
    let (arcs, target) = match style {
        ClawStyle::Talon => (vec![(0.64 * wire, -turn - 0.18)], target),
        ClawStyle::Fang => unreachable!(),
        ClawStyle::Tentacle => {
            // The lower S opens into a cabochon's available height while retaining both bend radii.
            let open = (((target[1] - foot[1]) / wire - 1.30) * 0.20).clamp(0.06, 0.40);
            (vec![(0.80 * wire, lean + open), (0.64 * wire, -turn)], target)
        }
        ClawStyle::Thorn => (vec![(0.64 * wire, -turn)], target),
        ClawStyle::Sepal => (vec![(1.05 * wire, -turn - 0.12)], target),
        ClawStyle::Wire => return None,
    };
    let step_arc = |from: f64, to: f64, radius: f64| {
        let r = radius * (to - from).signum();
        [r * (from.cos() - to.cos()), r * (to.sin() - from.sin())]
    };
    let (mut offset, mut heading, mut arc_length) = ([0.0, 0.0], lean, 0.0);
    for &(r, to) in &arcs {
        let v = step_arc(heading, to, r);
        offset[0] += v[0]; offset[1] += v[1];
        arc_length += r * (to - heading).abs();
        heading = to;
    }
    let d0 = [lean.sin(), lean.cos()];
    let d1 = [heading.sin(), heading.cos()];
    let v = [target[0] - foot[0] - offset[0], target[1] - foot[1] - offset[1]];
    let det = d0[0] * d1[1] - d0[1] * d1[0];
    if det.abs() < 1e-9 { return None; }
    let stem = (v[0] * d1[1] - v[1] * d1[0]) / det;
    let run = (d0[0] * v[1] - d0[1] * v[0]) / det;
    if !stem.is_finite() || !run.is_finite() || stem < 0.03 * wire || run < 0.03 * wire { return None; }
    let (mut line, mut along) = (vec![foot], vec![0.0]);
    let steps = (stem / 0.20).ceil().clamp(2.0, 128.0) as usize;
    for k in 1..=steps {
        let s = stem * k as f64 / steps as f64;
        line.push([foot[0] + d0[0] * s, foot[1] + d0[1] * s]); along.push(s);
    }
    let (mut heading, mut distance) = (lean, stem);
    for (r, to) in arcs {
        let from = *line.last().unwrap();
        let steps = ((to - heading).abs() / 0.08).ceil().clamp(2.0, 64.0) as usize;
        for k in 1..=steps {
            let angle = heading + (to - heading) * k as f64 / steps as f64;
            let v = step_arc(heading, angle, r);
            line.push([from[0] + v[0], from[1] + v[1]]); along.push(distance + r * (angle - heading).abs());
        }
        distance += r * (to - heading).abs(); heading = to;
    }
    let from = *line.last().unwrap();
    let steps = (run / 0.12).ceil().clamp(2.0, 64.0) as usize;
    for k in 1..=steps {
        let s = run * k as f64 / steps as f64;
        line.push([from[0] + d1[0] * s, from[1] + d1[1] * s]); along.push(distance + s);
    }
    let length = stem + arc_length + run;
    for s in &mut along { *s /= length; }
    Some((line, along))
}

/// Styled claws retain the legacy foot search and rails, check actual wall clearance, and refuse a foot without room for the chosen profile.
pub fn claw_parts_reach_styled(gem: Gem, prongs: u32, wire_mm: f64, rails: Rails, floor: Option<Floor>, wall: Option<Wall>, options: ClawOptions) -> Result<(Vec<Named>, Reach), HeadSnag> {
    let mut met = Reach::default();
    let plan = Plan::of(gem);
    let (c, p, g) = (gem.crown_mm(), gem.pavilion_mm(), girdle_half(gem));
    let d = wire_mm;
    let count = claw_count(gem, prongs);
    let depth = p + 0.2;
    let mut parts = Vec::new();
    for (claw, phi) in grouped_claw_angles(&plan, count, wire_mm, options.grouping).into_iter().enumerate() {
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
        let own = (-depth - 0.8).min(start[1] - 0.4);
        // The least wall the foot's rim and centre leave over the finger with the claw's base at `z`.
        let foot_wall = |wall: Wall, z: f64| {
            let foot = start[0] - (start[1] - z) * PRONG_LEAN;
            let c = [o[0] + n[0] * foot, o[1] + n[1] * foot, z];
            let l = PRONG_LEAN.hypot(1.0);
            let (u, v) = ([n[0] / l, n[1] / l, -PRONG_LEAN / l], [-n[1], n[0], 0.0]);
            let r = 0.60 * d;
            (0..24)
                .map(|k| {
                    let (s, co) = (TAU * k as f64 / 24.0).sin_cos();
                    wall(std::array::from_fn(|i| c[i] + r * (u[i] * co + v[i] * s)))
                })
                .fold(wall(c), f64::min)
        };
        let keeps = |z: f64| wall.is_none_or(|w| foot_wall(w, z) >= crate::mesh::MIN_WALL_MM + FOOT_WALL_SLACK_MM);
        // Lengthens the claw down its own line until its foot's inner edge is FOOT_SINK_MM into metal, else keeps it; the wall stops it first.
        let mut base_z = own;
        if let Some(floor) = floor {
            let inner = |z: f64| {
                let foot = start[0] - (start[1] - z) * PRONG_LEAN - 0.60 * d;
                [o[0] + n[0] * foot, o[1] + n[1] * foot]
            };
            let mut z = own;
            while z > own - CLAW_REACH_MM {
                if !keeps(z) {
                    met.walled += 1;
                    break;
                }
                if let Some(metal) = floor(inner(z)).filter(|metal| z <= metal - FOOT_SINK_MM) {
                    // Metal met only far below its top means the foot came down beside it.
                    if z >= metal - FOOT_SINK_MM - 0.5 {
                        base_z = z;
                        met.reached += 1;
                    }
                    break;
                }
                z -= 0.05;
            }
        }
        // A foot already inside the wall is shortened up its own line to it.
        let top = start[1] - 0.4;
        while base_z < top && !keeps(base_z) {
            base_z = (base_z + 0.05).min(top);
        }
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
        let mut oval = Vec::new();
        if options.style != ClawStyle::Wire {
            let (shaped, along) = shaped_claw_line(options.style, d, [foot, base_z], *line.last().unwrap(), turn, g)
                .ok_or(HeadSnag::ShortClaw { claw: claw as u32 + 1, style: options.style })?;
            line = shaped;
            let fang_tip_radius = (-line.last().unwrap()[0] + 0.20 * d).max(0.10 * d);
            rs = along.iter().map(|&t| d * match options.style {
                ClawStyle::Talon => if t < 0.80 { 0.60 - 0.10 * t / 0.80 } else { 0.50 - 0.41 * (t - 0.80) / 0.20 },
                ClawStyle::Fang => 0.60 + (fang_tip_radius / d - 0.60) * t,
                ClawStyle::Tentacle => if t < 0.85 { 0.60 - 0.38 * t } else { 0.277 - 0.137 * (t - 0.85) / 0.15 },
                ClawStyle::Thorn => if t < 0.85 { 0.60 - 0.38 * t } else { 0.277 - 0.207 * (t - 0.85) / 0.15 },
                ClawStyle::Sepal => if t < 0.35 { 0.60 - 0.24 * t / 0.35 } else if t < 0.80 { 0.36 } else { 0.36 - 0.26 * (t - 0.80) / 0.20 },
                ClawStyle::Wire => unreachable!(),
            }).collect();
            if options.style == ClawStyle::Sepal {
                oval = along.iter().zip(&rs).map(|(&t, &r)| (r, d * (0.60 + 0.18 * (PI * t).sin() - 0.16 * t))).collect();
            }
        }
        let path: Vec<P3> = line.iter().map(|q| [o[0] + n[0] * q[0], o[1] + n[1] * q[0], q[1]]).collect();
        let dome = (false, options.tip == ClawTip::Dome);
        let mut solid = if oval.is_empty() { tube(&path, &rs, 14, dome) } else { tube_oval(&path, &oval, 14, dome) };
        if options.tip == ClawTip::Point {
            // The last fan becomes a cone, without a collapsed ring of zero-area faces.
            let last = path.len() - 1;
            let v: P3 = std::array::from_fn(|i| path[last][i] - path[last - 1][i]);
            let length = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
            let r = oval.last().map_or(rs[last], |(a, b)| a.max(*b));
            *solid.v.last_mut().unwrap() = std::array::from_fn(|i| path[last][i] + 1.2 * r * v[i] / length);
        }
        parts.push(Named::whole(solid, format!("Claw {}", claw + 1)));
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
    Ok((parts, met))
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
    /// Made against the band as swept at 0, against the ring with every lower tier applied above it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tier: u8,
    /// The shape of its top over `height_mm`; on a cut, of its floor under `sink_mm`.
    #[serde(default, skip_serializing_if = "StampTop::is_flat")]
    pub top: StampTop,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

/// The shape of a stamp's top over its eaves at `height_mm`, in the outline's plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum StampTop {
    /// One height everywhere: the surface followed at `height_mm`.
    #[default]
    Flat,
    /// Two planes meeting `rise_mm` over the eaves in a ridge through the origin along `axis_deg`.
    Gable { rise_mm: f64, axis_deg: f64 },
    /// A ridge from `from` (`rise_mm` high) to `to` (`end_mm`), falling to the eaves at the outline's furthest reach.
    Ridge { rise_mm: f64, from: [f64; 2], to: [f64; 2], end_mm: f64 },
    /// A cone from the outline to an apex `apex_mm` over `at`, blunted to a flat `tip_mm` across.
    Cone { apex_mm: f64, at: [f64; 2], tip_mm: f64 },
    /// A dome from the outline to `crown_mm` over the origin.
    Dome { crown_mm: f64 },
    /// A top falling along `axis_deg` from `height_mm` at the outline's back to `tip_mm` at its front.
    Taper { axis_deg: f64, tip_mm: f64 },
}

impl StampTop {
    pub fn is_flat(&self) -> bool {
        *self == StampTop::Flat
    }

    /// The same top with every length scaled by `k`.
    pub fn scaled(&self, k: f64) -> StampTop {
        let s = |p: [f64; 2]| [p[0] * k, p[1] * k];
        match *self {
            StampTop::Flat => StampTop::Flat,
            StampTop::Gable { rise_mm, axis_deg } => StampTop::Gable { rise_mm: rise_mm * k, axis_deg },
            StampTop::Ridge { rise_mm, from, to, end_mm } => StampTop::Ridge { rise_mm: rise_mm * k, from: s(from), to: s(to), end_mm: end_mm * k },
            StampTop::Cone { apex_mm, at, tip_mm } => StampTop::Cone { apex_mm: apex_mm * k, at: s(at), tip_mm: tip_mm * k },
            StampTop::Dome { crown_mm } => StampTop::Dome { crown_mm: crown_mm * k },
            StampTop::Taper { axis_deg, tip_mm } => StampTop::Taper { axis_deg, tip_mm: tip_mm * k },
        }
    }

    /// The same top reflected across the outline's `y` axis.
    pub fn mirrored(&self) -> StampTop {
        let m = |p: [f64; 2]| [-p[0], p[1]];
        match *self {
            StampTop::Flat => StampTop::Flat,
            StampTop::Gable { rise_mm, axis_deg } => StampTop::Gable { rise_mm, axis_deg: -axis_deg },
            StampTop::Ridge { rise_mm, from, to, end_mm } => StampTop::Ridge { rise_mm, from: m(from), to: m(to), end_mm },
            StampTop::Cone { apex_mm, at, tip_mm } => StampTop::Cone { apex_mm, at: m(at), tip_mm },
            StampTop::Dome { crown_mm } => StampTop::Dome { crown_mm },
            StampTop::Taper { axis_deg, tip_mm } => StampTop::Taper { axis_deg: 180.0 - axis_deg, tip_mm },
        }
    }
}

/// A top read against one outline.
struct Shape<'a> {
    top: StampTop,
    outline: &'a [[f64; 2]],
    height: f64,
    /// The point the ridge line, apex or crown passes through.
    shift: [f64; 2],
    /// The outline's reach either side of a gable, from a ridge, or back and front along a taper.
    reach: [f64; 2],
}

impl<'a> Shape<'a> {
    fn new(top: StampTop, outline: &'a [[f64; 2]], height: f64, shift: [f64; 2]) -> Self {
        let mut reach: [f64; 2] = [0.0, 0.0];
        match top {
            StampTop::Gable { axis_deg, .. } => {
                let n = [-axis_deg.to_radians().sin(), axis_deg.to_radians().cos()];
                for p in outline {
                    let d = (p[0] - shift[0]) * n[0] + (p[1] - shift[1]) * n[1];
                    if d > 0.0 { reach[0] = reach[0].max(d) } else { reach[1] = reach[1].max(-d) }
                }
            }
            StampTop::Ridge { from, to, .. } => {
                reach[0] = outline.iter().map(|p| segment_distance(*p, from, to).0).fold(0.0, f64::max);
            }
            StampTop::Taper { axis_deg, .. } => {
                let a = [axis_deg.to_radians().cos(), axis_deg.to_radians().sin()];
                let proj = outline.iter().map(|p| p[0] * a[0] + p[1] * a[1]);
                reach = [proj.clone().fold(f64::MAX, f64::min), proj.fold(f64::MIN, f64::max)];
            }
            _ => {}
        }
        Self { top, outline, height, shift, reach }
    }

    /// Height of the top over the eaves at plan point `p`, mm.
    fn lift(&self, p: [f64; 2]) -> f64 {
        match self.top {
            StampTop::Flat => 0.0,
            StampTop::Gable { rise_mm, axis_deg } => {
                let n = [-axis_deg.to_radians().sin(), axis_deg.to_radians().cos()];
                let d = (p[0] - self.shift[0]) * n[0] + (p[1] - self.shift[1]) * n[1];
                let side = if d > 0.0 { self.reach[0] } else { self.reach[1] };
                rise_mm * (1.0 - d.abs() / side.max(1e-9)).max(0.0)
            }
            StampTop::Ridge { rise_mm, from, to, end_mm } => {
                let (dist, t) = segment_distance(p, from, to);
                (rise_mm + (end_mm - rise_mm) * t) * (1.0 - dist / self.reach[0].max(1e-9)).max(0.0)
            }
            StampTop::Cone { apex_mm, tip_mm, .. } => {
                let c = self.shift;
                let r = (p[0] - c[0]).hypot(p[1] - c[1]);
                if r < 1e-12 {
                    return apex_mm;
                }
                let Some(reach) = ray_reach(self.outline, c, [(p[0] - c[0]) / r, (p[1] - c[1]) / r]) else { return 0.0 };
                let flat = (0.5 * tip_mm).clamp(0.0, 0.9 * reach);
                apex_mm * (1.0 - ((r - flat) / (reach - flat).max(1e-9)).clamp(0.0, 1.0))
            }
            StampTop::Dome { crown_mm } => {
                let c = self.shift;
                let r = (p[0] - c[0]).hypot(p[1] - c[1]);
                if r < 1e-12 {
                    return crown_mm;
                }
                let Some(reach) = ray_reach(self.outline, c, [(p[0] - c[0]) / r, (p[1] - c[1]) / r]) else { return 0.0 };
                let rho = (r / reach.max(1e-9)).min(1.0);
                crown_mm * (1.0 - rho * rho)
            }
            StampTop::Taper { axis_deg, tip_mm } => {
                let a = [axis_deg.to_radians().cos(), axis_deg.to_radians().sin()];
                let u = ((p[0] * a[0] + p[1] * a[1] - self.reach[0]) / (self.reach[1] - self.reach[0]).max(1e-9)).clamp(0.0, 1.0);
                (tip_mm.max(0.0) - self.height) * u
            }
        }
    }

    /// The points and lines the cap must hold for this top.
    fn creases(&self) -> Creases {
        let mut out = Creases::default();
        let inside = |p: [f64; 2]| inside_polygon(self.outline, p) && edge_distance(self.outline, p) > CREASE_CLEAR;
        match self.top {
            StampTop::Flat | StampTop::Taper { .. } => {}
            StampTop::Gable { axis_deg, .. } => {
                let a = [axis_deg.to_radians().cos(), axis_deg.to_radians().sin()];
                for (t0, t1) in inside_runs(self.outline, self.shift, a, f64::MIN, f64::MAX) {
                    out.line(self.outline, [self.shift[0] + a[0] * t0, self.shift[1] + a[1] * t0], [self.shift[0] + a[0] * t1, self.shift[1] + a[1] * t1]);
                }
            }
            StampTop::Ridge { from, to, .. } => {
                let d = [to[0] - from[0], to[1] - from[1]];
                let l = d[0].hypot(d[1]);
                if l > 1e-9 {
                    let a = [d[0] / l, d[1] / l];
                    for (t0, t1) in inside_runs(self.outline, from, a, 0.0, l) {
                        out.line(self.outline, [from[0] + a[0] * t0, from[1] + a[1] * t0], [from[0] + a[0] * t1, from[1] + a[1] * t1]);
                    }
                } else if inside(from) {
                    out.points.push(from);
                }
            }
            StampTop::Dome { .. } => {
                if inside(self.shift) {
                    out.points.push(self.shift);
                }
            }
            StampTop::Cone { tip_mm, .. } => {
                let c = self.shift;
                if !inside(c) {
                    return out;
                }
                out.points.push(c);
                let n = self.outline.len();
                let corners: Vec<[f64; 2]> = (0..n).filter_map(|i| {
                    let (p, v, q) = (self.outline[(i + n - 1) % n], self.outline[i], self.outline[(i + 1) % n]);
                    let (e0, e1) = (unit2([v[0] - p[0], v[1] - p[1]]), unit2([q[0] - v[0], q[1] - v[1]]));
                    ((e0[0] * e1[0] + e0[1] * e1[1]).clamp(-1.0, 1.0).acos() > 20f64.to_radians()).then_some(v)
                }).collect();
                let mut dirs: Vec<f64> = (0..16).map(|k| TAU * k as f64 / 16.0).collect();
                for v in &corners {
                    let d = (v[1] - c[1]).atan2(v[0] - c[0]).rem_euclid(TAU);
                    dirs.retain(|a| { let g = (a - d).rem_euclid(TAU); g.min(TAU - g) > 5f64.to_radians() });
                    dirs.push(d);
                }
                let flat = 0.5 * tip_mm;
                let mut ring = Vec::new();
                if flat > 1e-3 {
                    for a in &dirs {
                        let u = [a.cos(), a.sin()];
                        if let Some(reach) = ray_reach(self.outline, c, u) {
                            let r = flat.min(0.9 * reach);
                            ring.push(([c[0] + u[0] * r, c[1] + u[1] * r], *a));
                        }
                    }
                }
                out.points.extend(ring.iter().map(|(p, _)| *p));
                for v in corners {
                    out.line(self.outline, c, v);
                }
            }
        }
        out.points.retain(|p| inside(*p));
        out.lines.retain(|(a, b)| inside(*a) && inside(*b) && !crosses_outline(self.outline, *a, *b));
        out
    }
}

/// Vertices and edges a cap must carry inside its outline.
#[derive(Default)]
struct Creases {
    points: Vec<[f64; 2]>,
    lines: Vec<([f64; 2], [f64; 2])>,
}

/// How far inside its outline a crease point or a crease line's end must stand.
const CREASE_CLEAR: f64 = 2e-3;

impl Creases {
    /// Holds `a`–`b` as a crease, each end drawn in along it until it stands clear of the outline.
    fn line(&mut self, outline: &[[f64; 2]], a: [f64; 2], b: [f64; 2]) {
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        if l <= 4.0 * CREASE_CLEAR {
            return;
        }
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        let (Some(ta), Some(tb)) = (clear_along(outline, a, u, l), clear_along(outline, b, [-u[0], -u[1]], l)) else { return };
        if ta + tb < l - 2.0 * CREASE_CLEAR {
            self.lines.push(([a[0] + u[0] * ta, a[1] + u[1] * ta], [b[0] - u[0] * tb, b[1] - u[1] * tb]));
        }
    }

    fn is_empty(&self) -> bool {
        self.points.is_empty() && self.lines.is_empty()
    }
}

/// Distance from `p` to segment `a`–`b`, and the nearest point's parameter along it.
fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / (ex * ex + ey * ey).max(1e-18)).clamp(0.0, 1.0);
    ((p[0] - a[0] - ex * t).hypot(p[1] - a[1] - ey * t), t)
}

/// Distance along `u` from `p`, within `limit`, to the first point standing 1.5 [`CREASE_CLEAR`] inside the outline.
fn clear_along(outline: &[[f64; 2]], p: [f64; 2], u: [f64; 2], limit: f64) -> Option<f64> {
    let want = 1.5 * CREASE_CLEAR;
    let mut t = 0.0;
    for _ in 0..400 {
        let q = [p[0] + u[0] * t, p[1] + u[1] * t];
        let d = edge_distance(outline, q);
        let signed = if inside_polygon(outline, q) { d } else { -d };
        if signed >= want {
            return Some(t);
        }
        // Steps by the clearance still wanted, with a quarter to spare.
        t += 1.25 * want - signed;
        if t > limit {
            return None;
        }
    }
    None
}

/// Distance from `p` to the nearest edge of `outline`.
fn edge_distance(outline: &[[f64; 2]], p: [f64; 2]) -> f64 {
    let n = outline.len();
    (0..n).map(|i| segment_distance(p, outline[i], outline[(i + 1) % n]).0).fold(f64::MAX, f64::min)
}

/// Distance along a ray from `c` in unit direction `u` to the outline.
fn ray_reach(outline: &[[f64; 2]], c: [f64; 2], u: [f64; 2]) -> Option<f64> {
    let n = outline.len();
    let mut best: Option<f64> = None;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let den = u[0] * e[1] - u[1] * e[0];
        if den.abs() < 1e-15 {
            continue;
        }
        let w = [a[0] - c[0], a[1] - c[1]];
        let s = (w[0] * e[1] - w[1] * e[0]) / den;
        let t = (w[0] * u[1] - w[1] * u[0]) / den;
        if s > 1e-12 && (-1e-12..=1.0 + 1e-12).contains(&t) && best.is_none_or(|x| s < x) {
            best = Some(s);
        }
    }
    best
}

/// The stretches `[t0, t1]` of the line `a + t·u` that lie inside the outline, within `lo..hi`.
fn inside_runs(outline: &[[f64; 2]], a: [f64; 2], u: [f64; 2], lo: f64, hi: f64) -> Vec<(f64, f64)> {
    let n = outline.len();
    let side = |p: [f64; 2]| (p[0] - a[0]) * u[1] - (p[1] - a[1]) * u[0];
    let mut ts: Vec<f64> = (0..n).filter_map(|i| {
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        let (sp, sq) = (side(p), side(q));
        ((sp > 0.0) != (sq > 0.0)).then(|| {
            let f = sp / (sp - sq);
            let x = [p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f];
            (x[0] - a[0]) * u[0] + (x[1] - a[1]) * u[1]
        })
    }).collect();
    ts.sort_by(f64::total_cmp);
    ts.chunks_exact(2).filter_map(|w| {
        let (t0, t1) = (w[0].max(lo), w[1].min(hi));
        (t1 > t0).then_some((t0, t1))
    }).collect()
}

/// Whether the segment `a`–`b` properly crosses any edge of the outline.
fn crosses_outline(outline: &[[f64; 2]], a: [f64; 2], b: [f64; 2]) -> bool {
    let n = outline.len();
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    (0..n).any(|i| {
        let (c, d) = (outline[i], outline[(i + 1) % n]);
        let (d1, d2, d3, d4) = (orient(c, d, a), orient(c, d, b), orient(a, b, c), orient(a, b, d));
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    })
}

/// The most outline points a stamp may carry.
pub const MAX_STAMP_POINTS: usize = 1024;

/// The most outline points a format-5 build strikes.
pub const PLAIN_MAX_STAMP_POINTS: usize = 512;

impl Stamp {
    /// Where the stamp stands: its origin on the bare surface, `z` out of the metal, `x` its outline's own.
    pub fn frame(&self, design: &crate::RingDesign, ctx: &crate::FieldContext) -> csg::Frame {
        self.frame_on(&BareSurface::new(design, ctx))
    }

    /// [`Stamp::frame`] read off a surface already sampled; in a design whose poured stamps are all plain, at the nearest sample as a format-5 build reads it.
    fn frame_on(&self, surface: &BareSurface) -> csg::Frame {
        let (point, mut normal, mut along, mut across) =
            if surface.shaped() { surface.at(self.theta_deg, self.v_mm) } else { surface.nearest_at(self.theta_deg, self.v_mm) };
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
        let shape = Shape::new(self.top, &self.outline, self.height_mm, self.top_anchor());
        Some(self.assemble(&cap.0, &cap.1, &flat, &shape))
    }

    /// The point the top's ridge line, apex or crown passes through.
    fn top_anchor(&self) -> [f64; 2] {
        match self.top {
            StampTop::Cone { at, .. } => at,
            StampTop::Ridge { from, .. } => from,
            _ => [0.0, 0.0],
        }
    }

    /// Tier 0, a flat top and at most [`PLAIN_MAX_STAMP_POINTS`] outline points: what a format-5 build strikes.
    pub fn is_plain(&self) -> bool {
        self.tier == 0 && self.top.is_flat() && self.outline.len() <= PLAIN_MAX_STAMP_POINTS
    }

    /// Closed solid from cap points, their triangles and the surface height under each.
    fn assemble(&self, points: &[[f64; 2]], tris: &[[u32; 3]], surface: &[f64], shape: &Shape) -> Solid {
        let n = self.outline.len();
        let count = points.len() as u32;
        let lean = if self.cut { 0.0 } else { (self.height_mm + self.sink_mm).max(0.0) * self.draft_deg.clamp(0.0, 30.0).to_radians().tan() };
        let flat = self.top.is_flat();
        let mut out = Solid::default();
        for (p, z) in points.iter().zip(surface) {
            let top = z + self.height_mm;
            out.v.push([p[0], p[1], if flat || self.cut { top } else { top + shape.lift(*p) }]);
        }
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
            let floor = z - self.sink_mm;
            out.v.push([q[0], q[1], if flat || !self.cut { floor } else { floor - shape.lift(*p) }]);
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

    /// The stamp with every cap point dropped onto the solid beneath it.
    fn solid(&self, frame: &csg::Frame, band: &Solid) -> Result<Solid, String> {
        if let StampTop::Cone { at, .. } = self.top {
            if !inside_polygon(&self.outline, at) {
                return Err("its cone's apex stands outside its outline".into());
            }
        }
        // A bench stamp never meets the sand. Split, its edges would lie on its blank's own split, and the
        // two solids meeting edge on edge leave zero-length slivers in the join.
        if self.bench {
            let shape = Shape::new(self.top, &self.outline, self.height_mm, self.top_anchor());
            let creases = shape.creases();
            return self.stand(frame, band, &[], &creases, &shape);
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
        // Snaps a ridge, apex or crown within 3 * OVER of the chord onto it.
        let mut anchor = self.top_anchor();
        let mut top = self.top;
        let mut on_chord = false;
        let g2 = frame.x[2] * frame.x[2] + frame.y[2] * frame.y[2];
        if !chords.is_empty() && g2 > 0.25 && !top.is_flat() {
            let off = height(anchor);
            let parallel = match top {
                StampTop::Gable { axis_deg, .. } => (axis_deg.to_radians().cos() * frame.x[2] + axis_deg.to_radians().sin() * frame.y[2]).abs() < 1e-6,
                StampTop::Ridge { from, to, .. } => ((to[0] - from[0]) * frame.x[2] + (to[1] - from[1]) * frame.y[2]).abs() < 1e-6 * (to[0] - from[0]).hypot(to[1] - from[1]).max(1e-3),
                StampTop::Cone { .. } | StampTop::Dome { .. } => true,
                _ => false,
            };
            if parallel && off.abs() < 3.0 * OVER {
                let delta = [-frame.x[2] * off / g2, -frame.y[2] * off / g2];
                anchor = [anchor[0] + delta[0], anchor[1] + delta[1]];
                if let StampTop::Ridge { from, to, .. } = &mut top {
                    *from = [from[0] + delta[0], from[1] + delta[1]];
                    *to = [to[0] + delta[0], to[1] + delta[1]];
                }
                on_chord = true;
            }
        }
        let this = Stamp { outline: split, top, ..self.clone() };
        let shape = Shape::new(this.top, &this.outline, this.height_mm, anchor);
        let creases = if on_chord && matches!(this.top, StampTop::Gable { .. } | StampTop::Ridge { .. }) { Creases::default() } else { shape.creases() };
        this.stand(frame, band, &chords, &creases, &shape)
    }

    fn stand(&self, frame: &csg::Frame, band: &Solid, chords: &[(usize, usize)], creases: &Creases, shape: &Shape) -> Result<Solid, String> {
        let n = self.outline.len();
        if !(3..=MAX_STAMP_POINTS).contains(&n) { return Err(format!("needs 3 to {MAX_STAMP_POINTS} outline points")); }
        let area: f64 = (0..n).map(|i| { let (a, b) = (self.outline[i], self.outline[(i + 1) % n]); a[0] * b[1] - b[0] * a[1] }).sum();
        if area <= 1e-6 { return Err("its outline must run counter-clockwise and enclose something".into()); }
        let reach = self.outline.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max) + 1.0;
        let near: Vec<[P3; 3]> = band.f.iter().filter_map(|f| {
            let t = f.map(|k| band.v[k as usize]);
            t.iter().any(|q| (0..3).map(|k| (q[k] - frame.origin[k]).powi(2)).sum::<f64>() < reach * reach).then_some(t)
        }).collect();
        let pitch = (reach / 14.0).clamp(0.12, 0.35);
        let cap = if creases.is_empty() { cap_faces(&self.outline, pitch, chords) } else { cap_faces_with(&self.outline, pitch, chords, creases) };
        let (points, tris) = cap.ok_or("its outline will not triangulate")?;
        const ABOVE: f64 = 8.0;
        let down = [-frame.z[0], -frame.z[1], -frame.z[2]];
        let mut surface = Vec::with_capacity(points.len());
        for p in &points {
            let hit = first_hit(&near, frame.point([p[0], p[1], ABOVE]), down).ok_or("it runs off the edge of the surface it stands on")?;
            surface.push(ABOVE - hit);
        }
        Ok(self.assemble(&points, &tris, &surface, shape).placed(frame))
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

/// An outline's convex hull as a blank, and per bay a cutter closed `margin` past the hull, stopping 0.01 mm short of its corners.
pub fn hull_and_bays(outline: &[[f64; 2]], margin: f64) -> (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>) {
    const SHY: f64 = 0.01;
    let n = outline.len();
    if n < 3 {
        return (outline.to_vec(), Vec::new());
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| outline[*a][0].total_cmp(&outline[*b][0]).then(outline[*a][1].total_cmp(&outline[*b][1])));
    let mut hull: Vec<usize> = Vec::new();
    for pass in [false, true] {
        let start = hull.len();
        let seq: Vec<usize> = if pass { order.iter().rev().copied().collect() } else { order.clone() };
        for i in seq {
            while hull.len() >= start + 2 && cross(outline[hull[hull.len() - 2]], outline[hull[hull.len() - 1]], outline[i]) <= 0.0 {
                hull.pop();
            }
            hull.push(i);
        }
        hull.pop();
    }
    hull.sort_unstable();
    hull.dedup();
    let h = hull.len();
    let mut blank = Vec::new();
    let mut bays = Vec::new();
    let m = margin.max(0.02);
    for k in 0..h {
        let (i, j) = (hull[k], hull[(k + 1) % h]);
        let (a, b) = (outline[i], outline[j]);
        let steps = ((a[0] - b[0]).hypot(a[1] - b[1]) / crate::outline::STEP).ceil().max(1.0) as usize;
        blank.extend((0..steps).map(|q| {
            let f = q as f64 / steps as f64;
            [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
        }));
        let chain: Vec<[f64; 2]> = (0..).map(|q| outline[(i + q) % n]).take((j + n - i) % n + 1).collect();
        let c = chain.len();
        if c < 3 || chain.iter().map(|p| segment_distance(*p, a, b).0).fold(0.0, f64::max) < 1e-3 {
            continue;
        }
        let shy = |p: [f64; 2], q: [f64; 2]| {
            let l = (q[0] - p[0]).hypot(q[1] - p[1]);
            let t = SHY.min(0.3 * l) / l.max(1e-12);
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        };
        let e = unit2([b[0] - a[0], b[1] - a[1]]);
        let out = [e[1] * m, -e[0] * m];
        let mut bay = vec![shy(b, chain[c - 2])];
        bay.extend(chain[1..c - 1].iter().rev());
        bay.extend([shy(a, chain[1]), [a[0] + out[0], a[1] + out[1]], [b[0] + out[0], b[1] + out[1]]]);
        bays.push(bay);
    }
    (blank, bays)
}

/// Which way a row of stamps runs round the ring.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RowPath {
    /// Where the section crosses the parting plane, solved per station.
    PartingLine,
    /// At one chart `v`.
    ChartV { v_mm: f64 },
    /// `frac` of the way across a side face from its low-`v` end; the high-`v` face if `high`.
    SideFace { high: bool, frac: f64 },
}

/// A row of one stamp struck along a path round the ring.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StampRow {
    /// The stamp each station strikes, its name the row's family; its angle and `v` are ignored.
    pub stamp: Stamp,
    pub path: RowPath,
    /// Ring angles of the first and last centres, degrees.
    pub from_deg: f64,
    pub to_deg: f64,
    pub count: u32,
    /// Scale lost by the last stamp, 0 to 0.85, as a raised cosine along the path at a constant gap.
    #[serde(default)]
    pub taper: f64,
    /// Drops stations whose stamp comes within this of a fold in the path, mm; 0 keeps all.
    #[serde(default)]
    pub fold_clear_mm: f64,
    /// Also strikes the row reflected about the head.
    #[serde(default)]
    pub mirror_shoulders: bool,
}

/// The most stations a row strikes.
pub const MAX_ROW_STAMPS: u32 = 400;

/// The row's stamps, named "Thorn, 3"; mirrored, each side numbered outward from the head, less any reflection in the row's own span or meeting a struck stamp more than the row's own stations meet.
pub fn stamp_row(design: &crate::RingDesign, row: &StampRow) -> Vec<Stamp> {
    let ctx = design.field_context();
    let surface = BareSurface::new(design, &ctx);
    let span = row.to_deg - row.from_deg;
    let side_v = match row.path {
        RowPath::SideFace { high, frac } => {
            let Some(faces) = ctx.side_faces_std() else { return Vec::new() };
            let Some((a, b)) = (if high { faces.high } else { faces.low }) else { return Vec::new() };
            Some(a + (b - a) * frac.clamp(0.0, 1.0))
        }
        _ => None,
    };
    let v_at = |theta: f64, guess: f64| -> f64 {
        match row.path {
            RowPath::PartingLine => surface.parting_v(theta, guess).unwrap_or(guess),
            RowPath::ChartV { v_mm } => v_mm,
            RowPath::SideFace { .. } => side_v.unwrap_or(guess),
        }
    };
    // Path samples every half degree, with arc length.
    let samples = ((span.abs() / 0.5).ceil() as usize).clamp(4, 2880);
    let mut guess = ctx.crest_v_mm;
    let mut path: Vec<(f64, f64, [f64; 3])> = Vec::with_capacity(samples + 1);
    for k in 0..=samples {
        let theta = row.from_deg + span * k as f64 / samples as f64;
        let v = v_at(theta, guess);
        guess = v;
        path.push((theta, v, surface.at(theta, v).0));
    }
    let mut arc = vec![0.0];
    for w in path.windows(2) {
        let (p, q) = (w[0].2, w[1].2);
        arc.push(arc[arc.len() - 1] + ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt());
    }
    let total = arc[arc.len() - 1];
    // Full-size reach behind and ahead along the path.
    let (rs, rc) = row.stamp.rot_deg.to_radians().sin_cos();
    let along = row.stamp.outline.iter().map(|p| p[0] * rc - p[1] * rs);
    let (back, front) = (along.clone().fold(f64::MAX, f64::min).min(0.0), along.fold(f64::MIN, f64::max).max(0.0));
    let taper = row.taper.clamp(0.0, 0.85);
    let scale = |s: f64| 1.0 - taper * 0.5 * (1.0 - (PI * (s / total.max(1e-9)).clamp(0.0, 1.0)).cos());
    let count = row.count.clamp(1, MAX_ROW_STAMPS) as usize;
    let stations: Vec<f64> = if count == 1 {
        vec![0.0]
    } else if taper <= 0.0 {
        (0..count).map(|k| total * k as f64 / (count - 1) as f64).collect()
    } else {
        // Bisects the edge-to-edge gap that lands the last centre on the path's end.
        let lay = |gap: f64| -> Vec<f64> {
            let mut s = vec![0.0];
            for _ in 1..count {
                let prev = s[s.len() - 1];
                let mut next = prev + front * scale(prev) - back * scale(prev) + gap;
                for _ in 0..30 {
                    next = prev + front * scale(prev) - back * scale(next) + gap;
                }
                s.push(next);
            }
            s
        };
        let (mut lo, mut hi) = (-(front - back), total.max(1e-6));
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if lay(mid)[count - 1] > total { hi = mid } else { lo = mid }
        }
        lay(0.5 * (lo + hi))
    };
    let folds = if row.fold_clear_mm > 0.0 { path_folds(&path.iter().map(|p| p.2).collect::<Vec<_>>(), &arc) } else { Vec::new() };
    let theta_at = |s: f64| -> f64 {
        let j = arc.partition_point(|a| *a <= s).clamp(1, arc.len() - 1);
        let f = ((s - arc[j - 1]) / (arc[j] - arc[j - 1]).max(1e-15)).clamp(0.0, 1.0);
        path[j - 1].0 + (path[j].0 - path[j - 1].0) * f
    };
    let guess_at = |s: f64| -> f64 {
        let j = arc.partition_point(|a| *a <= s).clamp(1, arc.len() - 1);
        path[j].1
    };
    let mut kept = Vec::new();
    for s in stations {
        let k = scale(s);
        if folds.iter().any(|f| *f >= s + back * k - row.fold_clear_mm && *f <= s + front * k + row.fold_clear_mm) {
            continue;
        }
        let theta = theta_at(s);
        let mut stamp = row.stamp.clone();
        stamp.theta_deg = theta;
        stamp.v_mm = v_at(theta, guess_at(s));
        if (k - 1.0).abs() > 0.0 {
            stamp.outline = stamp.outline.iter().map(|p| [p[0] * k, p[1] * k]).collect();
            stamp.height_mm *= k;
            stamp.top = stamp.top.scaled(k);
        }
        if kept.iter().any(|o: &Stamp| crate::field::wrap_delta(o.theta_deg - stamp.theta_deg, 360.0).abs() < 1e-9 && (o.v_mm - stamp.v_mm).abs() < 1e-9) {
            continue;
        }
        kept.push(stamp);
    }
    let family = row.stamp.name.clone();
    if !row.mirror_shoulders {
        for (i, s) in kept.iter_mut().enumerate() {
            s.name = format!("{family}, {}", i + 1);
        }
        return kept;
    }
    let covered = |theta: f64| -> bool {
        if span.abs() >= 360.0 {
            return true;
        }
        let d = if span >= 0.0 { theta - row.from_deg } else { row.from_deg - theta }.rem_euclid(360.0);
        d <= span.abs() + 1e-9 || d >= 360.0 - 1e-9
    };
    let mut out = kept.clone();
    let mut frames: Vec<csg::Frame> = out.iter().map(|s| s.frame_on(&surface)).collect();
    let apart = |a: &csg::Frame, b: &csg::Frame| (0..3).map(|k| (a.origin[k] - b.origin[k]).powi(2)).sum::<f64>().sqrt();
    let pitch = frames.windows(2).map(|w| apart(&w[0], &w[1])).fold(f64::MAX, f64::min);
    let crowded = (1..kept.len()).any(|i| plans_meet(&kept[i - 1], &frames[i - 1], &kept[i], &frames[i]));
    for s in &kept {
        let theta = (2.0 * crate::profile::TOP_DEG - s.theta_deg).rem_euclid(360.0);
        if covered(theta) {
            continue;
        }
        let mut m = s.clone();
        m.theta_deg = theta;
        m.v_mm = v_at(theta, s.v_mm);
        m.rot_deg = -s.rot_deg;
        m.outline = s.outline.iter().rev().map(|p| [-p[0], p[1]]).collect();
        m.top = s.top.mirrored();
        let frame = m.frame_on(&surface);
        if out.iter().zip(&frames).any(|(o, f)| plans_meet(o, f, &m, &frame) && (!crowded || apart(f, &frame) < 0.5 * pitch)) {
            continue;
        }
        out.push(m);
        frames.push(frame);
    }
    // Each side numbered outward from the head.
    let offset = |s: &Stamp| crate::field::wrap_delta(s.theta_deg - crate::profile::TOP_DEG, 360.0);
    let side = |d: f64| if d.abs() < 1e-9 { "head" } else if d > 0.0 { "right" } else { "left" };
    let heads = out.iter().filter(|s| side(offset(s)) == "head").count();
    let mut order: Vec<usize> = (0..out.len()).collect();
    order.sort_by(|a, b| offset(&out[*a]).abs().total_cmp(&offset(&out[*b]).abs()));
    let mut counts = std::collections::HashMap::new();
    for i in order {
        let label = side(offset(&out[i]));
        let k = counts.entry(label).or_insert(0usize);
        *k += 1;
        out[i].name = if label == "head" && heads == 1 { format!("{family}, head") } else { format!("{family}, {label} {k}") };
    }
    out
}

/// Whether two placed stamps' plans overlap, drawn into the first one's plane.
fn plans_meet(a: &Stamp, fa: &csg::Frame, b: &Stamp, fb: &csg::Frame) -> bool {
    let d = (0..3).map(|k| (fa.origin[k] - fb.origin[k]).powi(2)).sum::<f64>().sqrt();
    if a.outline.len() < 3 || b.outline.len() < 3 || d > plan_reach(a) + plan_reach(b) {
        return false;
    }
    let other = drawn_into(fa, b, fb);
    // A point just inside an outline's first edge.
    let within = |o: &[[f64; 2]]| {
        let (p, q) = (o[0], o[1]);
        let e = unit2([q[0] - p[0], q[1] - p[1]]);
        [0.5 * (p[0] + q[0]) - 1e-4 * e[1], 0.5 * (p[1] + q[1]) + 1e-4 * e[0]]
    };
    inside_polygon(&other, within(&a.outline))
        || inside_polygon(&a.outline, within(&other))
        || other.iter().any(|q| inside_polygon(&a.outline, *q))
        || a.outline.iter().any(|q| inside_polygon(&other, *q))
        || (0..other.len()).any(|i| crosses_outline(&a.outline, other[i], other[(i + 1) % other.len()]))
}

/// Arc positions where the path turns more than 12 degrees per mm, read over 0.4 mm either side.
fn path_folds(points: &[[f64; 3]], arc: &[f64]) -> Vec<f64> {
    let at = |s: f64| -> [f64; 3] {
        let j = arc.partition_point(|a| *a <= s).clamp(1, arc.len() - 1);
        let f = ((s - arc[j - 1]) / (arc[j] - arc[j - 1]).max(1e-15)).clamp(0.0, 1.0);
        std::array::from_fn(|k| points[j - 1][k] + (points[j][k] - points[j - 1][k]) * f)
    };
    let total = arc[arc.len() - 1];
    let mut out = Vec::new();
    let mut s = 0.4;
    while s <= total - 0.4 {
        let (p, q, r) = (at(s - 0.4), at(s), at(s + 0.4));
        let (u, w): ([f64; 3], [f64; 3]) = (std::array::from_fn(|k| q[k] - p[k]), std::array::from_fn(|k| r[k] - q[k]));
        let dot = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
        let len = (u.iter().map(|x| x * x).sum::<f64>() * w.iter().map(|x| x * x).sum::<f64>()).sqrt().max(1e-12);
        if (dot / len).clamp(-1.0, 1.0).acos().to_degrees() > 12.0 * 0.8 {
            out.push(s);
        }
        s += 0.1;
    }
    out
}

/// Sections the bare surface keeps for reuse.
const SECTIONS_KEPT: usize = 64;

/// Bare-surface points kept across calls.
const POINTS_KEPT: usize = 4096;

/// Section steps a plain stamp's frame reads, as [`crate::stones::surface_frame`] does.
const PLAIN_SECTION_STEPS: usize = 192;

/// A bare-surface point: position, outward normal, the ring's tangent and the section's.
type SurfacePoint = ([f64; 3], [f64; 3], [f64; 3], [f64; 3]);

/// Surface samples `(v, r, z, nr, nz)` with arc per chart mm, or with surface length for a plain read.
type Samples = std::rc::Rc<(Vec<[f64; 5]>, f64)>;

#[cfg(test)]
thread_local! {
    /// Bare-surface points and sections this thread has made rather than read from a store.
    pub(crate) static MADE: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
}

/// Counts one made point (`0`) or section (`1`) on this thread under test.
fn count_made(_what: usize) {
    #[cfg(test)]
    MADE.with(|m| {
        let mut n = m.get();
        n[_what] += 1;
        m.set(n);
    });
}

/// Bare-surface points by band key, chart point and whether read off the nearest plain sample.
fn kept_point(key: (u64, u64, u64, bool), make: impl FnOnce() -> SurfacePoint) -> SurfacePoint {
    type Kept = (std::collections::HashMap<(u64, u64, u64, bool), SurfacePoint>, std::collections::VecDeque<(u64, u64, u64, bool)>);
    static KEPT: std::sync::Mutex<Option<Kept>> = std::sync::Mutex::new(None);
    if let Some(hit) = KEPT.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|(m, _)| m.get(&key).copied()) {
        return hit;
    }
    let made = make();
    let mut guard = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let (map, order) = guard.get_or_insert_with(Default::default);
    if map.insert(key, made).is_none() {
        order.push_back(key);
        while order.len() > POINTS_KEPT {
            if let Some(old) = order.pop_front() {
                map.remove(&old);
            }
        }
    }
    made
}

/// A design's bare surface at chart points, read against one reference section, its latest sections kept by angle and modulation.
pub(crate) struct BareSurface<'a> {
    design: &'a crate::RingDesign,
    ctx: &'a crate::FieldContext,
    reference: std::cell::OnceCell<crate::profile::ProfileLoop>,
    band: std::cell::OnceCell<Option<u64>>,
    shaped: std::cell::OnceCell<bool>,
    sections: std::cell::RefCell<Vec<(u64, bool, String, Samples)>>,
}

impl<'a> BareSurface<'a> {
    pub(crate) fn new(design: &'a crate::RingDesign, ctx: &'a crate::FieldContext) -> Self {
        Self { design, ctx, reference: Default::default(), band: Default::default(), shaped: Default::default(), sections: Default::default() }
    }

    /// Whether the design pours a stamp a format-5 build cannot strike.
    fn shaped(&self) -> bool {
        *self.shaped.get_or_init(|| self.design.stamps.iter().any(|s| !s.bench && !s.is_plain()))
    }

    /// Hash of everything the bare surface is made from; `None` when the band cannot be serialized.
    fn band_key(&self) -> Option<u64> {
        *self.band.get_or_init(|| {
            use std::hash::{Hash, Hasher};
            let d = self.design;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_vec(&(&d.profile, &d.shank)).ok()?.hash(&mut h);
            d.inner_radius_mm().to_bits().hash(&mut h);
            if let Some(b) = &d.imported_base {
                b.source.fingerprint().hash(&mut h);
                serde_json::to_vec(&b.chart).ok()?.hash(&mut h);
                (b.bare, b.sand_envelope).hash(&mut h);
            }
            Some(h.finish())
        })
    }

    /// Reference-snapped surface samples `(v, r, z, nr, nz)` at a ring angle sorted by `v`, and section arc per chart mm.
    fn samples(&self, theta_deg: f64) -> Samples {
        self.section(theta_deg, false)
    }

    /// A swept section's surface samples kept by angle and modulation print: reference-snapped by `v`, or `plain` as [`crate::stones::surface_frame`] reads them.
    fn section(&self, theta_deg: f64, plain: bool) -> Samples {
        let design = self.design;
        let reference = self.reference.get_or_init(|| design.reference_loop());
        if design.imported_base.is_some() {
            return std::rc::Rc::new(self.read(&design.section_at(theta_deg, crate::profile::REFERENCE_PROFILE_STEPS, None, Some(reference))));
        }
        let angle = theta_deg.to_bits();
        if let Some(kept) = self.sections.borrow().iter().find(|(a, p, ..)| *a == angle && *p == plain).map(|(.., s)| s.clone()) {
            return kept;
        }
        // What [`crate::RingDesign::section_at`] makes of a swept band.
        let inner = design.inner_radius_mm();
        let m = design.modulation_at(theta_deg, inner, reference.crest_radius_mm);
        let key = format!("{m:?}");
        let found = self.sections.borrow().iter().find(|(_, p, k, _)| *p == plain && *k == key).map(|(.., s)| s.clone());
        let made = found.unwrap_or_else(|| {
            count_made(1);
            std::rc::Rc::new(if plain {
                let l = design.profile.sample_spaced(inner, PLAIN_SECTION_STEPS, &m, None, None);
                (l.pts.iter().filter(|p| p.surface).map(|p| [p.v_mm, p.r, p.z, p.nr, p.nz]).collect(), l.surface_len_mm)
            } else {
                self.read(&design.profile.sample_spaced(inner, crate::profile::REFERENCE_PROFILE_STEPS, &m, None, Some(reference)))
            })
        });
        let mut kept = self.sections.borrow_mut();
        if kept.len() >= SECTIONS_KEPT {
            kept.remove(0);
        }
        kept.push((angle, plain, key, made.clone()));
        made
    }

    /// A section's surface samples sorted by `v`, and its arc per chart mm.
    fn read(&self, l: &crate::profile::ProfileLoop) -> (Vec<[f64; 5]>, f64) {
        let mut s: Vec<[f64; 5]> = l.pts.iter().filter(|p| p.surface).map(|p| [p.v_mm, p.r, p.z, p.nr, p.nz]).collect();
        s.sort_by(|a, b| a[0].total_cmp(&b[0]));
        (s, l.surface_len_mm / self.ctx.band_v_len_mm.max(1e-9))
    }

    /// The swept section's `(r, z)` and outward normal at chart `v`, between its samples.
    fn section_point(&self, theta_deg: f64, v_mm: f64) -> ([f64; 2], [f64; 2]) {
        let samples = self.samples(theta_deg);
        let (s, k) = (&samples.0, samples.1);
        match s.len() {
            0 => return ([self.ctx.crest_radius_mm, 0.0], [1.0, 0.0]),
            1 => return ([s[0][1], s[0][2]], [s[0][3], s[0][4]]),
            _ => {}
        }
        let target = v_mm * k;
        let j = s.partition_point(|p| p[0] <= target).clamp(1, s.len() - 1);
        let (a, b) = (s[j - 1], s[j]);
        let f = ((target - a[0]) / (b[0] - a[0]).max(1e-15)).clamp(0.0, 1.0);
        let at = |i: usize| a[i] + (b[i] - a[i]) * f;
        ([at(1), at(2)], [at(3), at(4)])
    }

    /// Point, normal and tangents of the bare surface at a chart point, interpolated on a swept band.
    fn at(&self, theta_deg: f64, v_mm: f64) -> SurfacePoint {
        self.kept(theta_deg, v_mm, false, || {
            if self.design.imported_base.is_some() {
                return crate::stones::surface_frame(self.design, self.ctx, theta_deg, v_mm);
            }
            let ([r, z], [nr, nz]) = self.section_point(theta_deg, v_mm);
            let l = nr.hypot(nz).max(1e-12);
            let (nr, nz) = (nr / l, nz / l);
            let (sin, cos) = theta_deg.to_radians().sin_cos();
            ([r * cos, r * sin, z], [nr * cos, nr * sin, nz], [-sin, cos, 0.0], [-nz * cos, -nz * sin, nr])
        })
    }

    /// [`crate::stones::surface_frame`] bit for bit: on a swept band the nearest sample of a [`PLAIN_SECTION_STEPS`] section.
    fn nearest_at(&self, theta_deg: f64, v_mm: f64) -> SurfacePoint {
        self.kept(theta_deg, v_mm, true, || {
            if self.design.imported_base.is_some() {
                return crate::stones::surface_frame(self.design, self.ctx, theta_deg, v_mm);
            }
            let samples = self.section(theta_deg, true);
            let target = (v_mm / self.ctx.band_v_len_mm.max(1e-9)).clamp(0.0, 1.0) * samples.1;
            let mut best: Option<&[f64; 5]> = None;
            let mut best_d = f64::MAX;
            for p in &samples.0 {
                let d = (p[0] - target).abs();
                if d < best_d {
                    best_d = d;
                    best = Some(p);
                }
            }
            let [_, r, z, nr, nz] = best.copied().unwrap_or([0.0, self.design.inner_radius_mm(), 0.0, 1.0, 0.0]);
            let (sin, cos) = theta_deg.to_radians().sin_cos();
            ([r * cos, r * sin, z], [nr * cos, nr * sin, nz], [-sin, cos, 0.0], [-nz * cos, -nz * sin, nr])
        })
    }

    /// A surface point from the cross-call store, made and kept on a miss.
    fn kept(&self, theta_deg: f64, v_mm: f64, plain: bool, make: impl FnOnce() -> SurfacePoint) -> SurfacePoint {
        let make = || {
            count_made(0);
            make()
        };
        match self.band_key() {
            Some(band) => kept_point((band, theta_deg.to_bits(), v_mm.to_bits(), plain), make),
            None => make(),
        }
    }

    /// The chart `v` nearest `guess` at which the section at `theta_deg` crosses the parting plane.
    fn parting_v(&self, theta_deg: f64, guess: f64) -> Option<f64> {
        let (design, ctx) = (self.design, self.ctx);
        if design.imported_base.is_some() {
            let z = |v: f64| crate::stones::surface_frame(design, ctx, theta_deg, v).0[2];
            let (lo_v, hi_v) = (0.0, ctx.band_v_len_mm);
            let z0 = z(guess);
            if z0 == 0.0 {
                return Some(guess);
            }
            let mut bracket = None;
            for k in 1..400 {
                for dir in [1.0, -1.0] {
                    let v = guess + dir * 0.02 * k as f64;
                    if bracket.is_none() && (lo_v..=hi_v).contains(&v) && z(v).signum() != z0.signum() {
                        bracket = Some((v - dir * 0.02, v));
                    }
                }
                if bracket.is_some() {
                    break;
                }
            }
            let (mut a, mut b) = bracket?;
            let za = z(a);
            for _ in 0..60 {
                let mid = 0.5 * (a + b);
                if z(mid).signum() == za.signum() { a = mid } else { b = mid }
            }
            return Some(0.5 * (a + b));
        }
        let samples = self.samples(theta_deg);
        let (s, k) = (&samples.0, samples.1);
        let target = guess * k;
        let mut best: Option<(f64, f64)> = None;
        for w in s.windows(2) {
            let (a, b) = (w[0], w[1]);
            let v = if a[2] == 0.0 {
                a[0]
            } else if a[2] * b[2] < 0.0 {
                a[0] + (b[0] - a[0]) * a[2] / (a[2] - b[2])
            } else {
                continue;
            };
            if best.is_none_or(|(_, d)| (v - target).abs() < d) {
                best = Some((v, (v - target).abs()));
            }
        }
        best.map(|(v, _)| v / k.max(1e-9))
    }
}

/// Chart `v` where the section at `theta_deg` crosses the parting plane, nearest the reference crest.
pub fn crest_v(design: &crate::RingDesign, theta_deg: f64) -> Option<f64> {
    let ctx = design.field_context();
    BareSurface::new(design, &ctx).parting_v(theta_deg, ctx.crest_v_mm)
}

/// Each of a design's stamps' frames, made once on first ask.
pub(crate) struct StampFrames<'a> {
    surface: BareSurface<'a>,
    made: Vec<Option<csg::Frame>>,
}

impl<'a> StampFrames<'a> {
    pub(crate) fn new(design: &'a crate::RingDesign, ctx: &'a crate::FieldContext) -> Self {
        Self { surface: BareSurface::new(design, ctx), made: vec![None; design.stamps.len()] }
    }

    pub(crate) fn get(&mut self, k: usize) -> csg::Frame {
        if let Some(f) = self.made[k] {
            return f;
        }
        let f = self.surface.design.stamps[k].frame_on(&self.surface);
        self.made[k] = Some(f);
        f
    }
}

/// Furthest the stamp reaches from its origin in plan, its drafted walls included.
pub(crate) fn plan_reach(s: &Stamp) -> f64 {
    let lean = if s.cut { 0.0 } else { (s.height_mm + s.sink_mm).max(0.0) * s.draft_deg.clamp(0.0, 30.0).to_radians().tan() };
    s.outline.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max) + 2.5 * lean
}

/// False when two stamps' ring angles part them by more than their reaches, on a surface no nearer the axis than 0.9 of the bore.
pub(crate) fn may_touch(design: &crate::RingDesign, a: &Stamp, b: &Stamp) -> bool {
    let turn = crate::field::wrap_delta(a.theta_deg - b.theta_deg, 360.0).abs().min(90.0).to_radians();
    0.9 * design.inner_radius_mm() * turn.sin() <= plan_reach(a) + plan_reach(b) + 0.5
}

/// Whether two placed stamps face the same way on one stretch of surface, near enough to meet.
pub(crate) fn same_ground(a: &Stamp, af: &csg::Frame, b: &Stamp, bf: &csg::Frame) -> bool {
    let d: [f64; 3] = std::array::from_fn(|k| bf.origin[k] - af.origin[k]);
    let dz = d[0] * af.z[0] + d[1] * af.z[1] + d[2] * af.z[2];
    let lateral = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - dz * dz).max(0.0).sqrt();
    let depth = a.height_mm.max(0.0) + a.sink_mm.max(0.0) + b.height_mm.max(0.0) + b.sink_mm.max(0.0);
    af.z[0] * bf.z[0] + af.z[1] * bf.z[1] + af.z[2] * bf.z[2] > 0.5 && lateral <= plan_reach(a) + plan_reach(b) + 0.5 && dz.abs() <= 0.5 * lateral + depth
}

/// `other`'s outline, placed by `of`, drawn into `frame`'s plane.
pub(crate) fn drawn_into(frame: &csg::Frame, other: &Stamp, of: &csg::Frame) -> Vec<[f64; 2]> {
    other.outline.iter().map(|p| {
        let w = of.point([p[0], p[1], 0.0]);
        let d = [w[0] - frame.origin[0], w[1] - frame.origin[1], w[2] - frame.origin[2]];
        [d[0] * frame.x[0] + d[1] * frame.x[1] + d[2] * frame.x[2], d[0] * frame.y[0] + d[1] * frame.y[1] + d[2] * frame.y[2]]
    }).collect()
}

/// Points along an outline no more than `step` apart, each with the index of the edge it lies on.
fn densified(outline: &[[f64; 2]], step: f64) -> Vec<([f64; 2], usize)> {
    let n = outline.len();
    (0..n).flat_map(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let k = ((a[0] - b[0]).hypot(a[1] - b[1]) / step).ceil().clamp(1.0, 4096.0) as usize;
        (0..k).map(move |q| {
            let f = q as f64 / k as f64;
            ([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f], i)
        })
    }).collect()
}

/// Outline points of `s` whose cap leaves the lower-tier stamps it overlaps, past a joined one's edge or over a cut; empty when it rests wholly on them or on none.
fn overhang(design: &crate::RingDesign, frames: &mut StampFrames, s: &Stamp, frame: &csg::Frame) -> Vec<usize> {
    const STEP: f64 = 0.05;
    if s.cut || s.outline.len() < 3 {
        return Vec::new();
    }
    let (mut joined, mut cuts) = (Vec::new(), Vec::new());
    for (j, l) in design.stamps.iter().enumerate() {
        if l.tier >= s.tier || l.outline.len() < 3 || !may_touch(design, s, l) {
            continue;
        }
        let f = frames.get(j);
        if same_ground(s, frame, l, &f) {
            if l.cut { cuts.push(drawn_into(frame, l, &f)) } else { joined.push(drawn_into(frame, l, &f)) }
        }
    }
    let own = densified(&s.outline, STEP);
    let meets = |plan: &Vec<[f64; 2]>| own.iter().any(|(q, _)| inside_polygon(plan, *q)) || plan.iter().any(|q| inside_polygon(&s.outline, *q));
    joined.retain(meets);
    cuts.retain(meets);
    if joined.is_empty() && cuts.is_empty() {
        return Vec::new();
    }
    let clear = |plan: &Vec<[f64; 2]>, q: [f64; 2]| inside_polygon(plan, q) && edge_distance(plan, q) > 1e-3;
    let nearest = |q: [f64; 2]| (0..s.outline.len()).min_by(|a, b| {
        let d = |i: &usize| (s.outline[*i][0] - q[0]).hypot(s.outline[*i][1] - q[1]);
        d(a).total_cmp(&d(b))
    }).unwrap_or(0);
    let mut bad = std::collections::BTreeSet::new();
    for (q, i) in &own {
        if !joined.iter().any(|p| clear(p, *q)) || cuts.iter().any(|c| inside_polygon(c, *q)) {
            bad.insert(*i);
        }
    }
    // An edge of what lies beneath running under the cap, with nothing else there to carry it.
    for (a, plan) in joined.iter().enumerate() {
        for (q, _) in densified(plan, STEP) {
            if inside_polygon(&s.outline, q) && !joined.iter().enumerate().any(|(b, o)| b != a && clear(o, q)) {
                bad.insert(nearest(q));
            }
        }
    }
    for plan in &cuts {
        for (q, _) in densified(plan, STEP) {
            if inside_polygon(&s.outline, q) {
                bad.insert(nearest(q));
            }
        }
    }
    bad.into_iter().collect()
}

impl Stamp {
    /// Checks each plan line along the pull meets it in one stretch across the parting line with a top never rising away from it, resting wholly on any lower tier it overlaps; `Err` lists offending outline points.
    pub fn parting_monotone(&self, design: &crate::RingDesign) -> Result<(), Vec<usize>> {
        let o = &self.outline;
        let n = o.len();
        if self.bench {
            return Ok(());
        }
        if n < 3 {
            return Err((0..n).collect());
        }
        let ctx = design.field_context();
        let mut frames = StampFrames::new(design, &ctx);
        let frame = self.frame_on(&frames.surface);
        let g = [frame.x[2], frame.y[2]];
        let slope = g[0].hypot(g[1]);
        if slope < 0.1 {
            return Ok(());
        }
        let mut bad: std::collections::BTreeSet<usize> = overhang(design, &mut frames, self, &frame).into_iter().collect();
        let (d, e) = ([g[0] / slope, g[1] / slope], [-g[1] / slope, g[0] / slope]);
        let t0 = -frame.origin[2] / slope;
        let s_of = |p: [f64; 2]| p[0] * e[0] + p[1] * e[1];
        let t_of = |p: [f64; 2]| p[0] * d[0] + p[1] * d[1];
        let shape = Shape::new(self.top, o, self.height_mm, self.top_anchor());
        let lift = |s: f64, t: f64| shape.lift([e[0] * s + d[0] * t, e[1] * s + d[1] * t]);
        let falls = |s: f64, from: f64, to: f64| {
            let steps = (((to - from).abs() / 0.02).ceil() as usize).max(8);
            let mut last = lift(s, from);
            (1..=steps).all(|k| {
                let h = lift(s, from + (to - from) * k as f64 / steps as f64);
                let ok = h <= last + 1e-7;
                last = h;
                ok
            })
        };
        for i in 0..n {
            let (sa, sb) = (s_of(o[i]), s_of(o[(i + 1) % n]));
            if (sa - sb).abs() < 1e-9 {
                continue;
            }
            let s = 0.5 * (sa + sb);
            let mut hits: Vec<(f64, usize)> = (0..n).filter_map(|j| {
                let (p, q) = (o[j], o[(j + 1) % n]);
                let (sp, sq) = (s_of(p), s_of(q));
                ((sp > s) != (sq > s)).then(|| (t_of(p) + (t_of(q) - t_of(p)) * (s - sp) / (sq - sp), j))
            }).collect();
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            let ok = !self.cut
                && hits.len() == 2
                && hits[0].0 <= t0 + 1e-9
                && hits[1].0 >= t0 - 1e-9
                && (self.top.is_flat() || (falls(s, t0, hits[1].0) && falls(s, t0, hits[0].0)));
            if !ok {
                bad.extend(hits.iter().map(|h| h.1));
                bad.insert(i);
            }
        }
        if bad.is_empty() { Ok(()) } else { Err(bad.into_iter().collect()) }
    }

    /// The hull blank followed by one bench cut per bay, on the same tier.
    pub fn cast_as_hull(&self, margin: f64) -> Vec<Stamp> {
        let (hull, bays) = hull_and_bays(&self.outline, margin);
        let rise = {
            let shape = Shape::new(self.top, &self.outline, self.height_mm, self.top_anchor());
            hull.iter().map(|p| shape.lift(*p)).fold(0.0, f64::max).max(match self.top {
                StampTop::Gable { rise_mm, .. } | StampTop::Ridge { rise_mm, .. } => rise_mm,
                StampTop::Cone { apex_mm, .. } => apex_mm,
                StampTop::Dome { crown_mm } => crown_mm,
                _ => 0.0,
            })
        };
        let mut out = vec![Stamp { outline: hull, ..self.clone() }];
        for (k, bay) in bays.into_iter().enumerate() {
            out.push(Stamp {
                name: format!("{}: bay {}, cut at the bench", self.name, k + 1),
                outline: bay,
                height_mm: self.height_mm + rise + 0.3,
                sink_mm: -0.02,
                draft_deg: 0.0,
                cut: true,
                bench: true,
                top: StampTop::Flat,
                ..self.clone()
            });
        }
        out
    }
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
    cap_faces_with(outline, pitch, chords, &Creases::default())
}

/// [`cap_faces`] also holding a top's crease points as vertices and crease lines as non-outline edges.
fn cap_faces_with(outline: &[[f64; 2]], pitch: f64, chords: &[(usize, usize)], creases: &Creases) -> Option<(Vec<[f64; 2]>, Vec<[u32; 3]>)> {
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
    for p in &creases.points {
        let h = cdt.insert(Point2::new(p[0], p[1])).ok()?;
        if let std::collections::hash_map::Entry::Vacant(v) = index.entry(h) {
            v.insert(points.len() as u32);
            points.push(*p);
        }
    }
    // Chords are held as edges too, but are not the outline: crossing one is still inside. Each carries
    // points at the cap's pitch, or its top would run straight across the dome it spans and sag under it.
    let mut chord_edges = std::collections::HashSet::new();
    for (a, b) in chords {
        let (pa, pb) = (outline[*a], outline[*b]);
        let steps = (((pb[0] - pa[0]).hypot(pb[1] - pa[1]) / pitch).ceil() as usize).max(1);
        let mut run = vec![handles[*a]];
        // Crease points lying on the chord join its run in order.
        let (ex, ey) = (pb[0] - pa[0], pb[1] - pa[1]);
        let len2 = (ex * ex + ey * ey).max(1e-18);
        let mut on: Vec<(f64, [f64; 2])> = creases.points.iter().filter_map(|p| {
            let t = ((p[0] - pa[0]) * ex + (p[1] - pa[1]) * ey) / len2;
            (t > 1e-9 && t < 1.0 - 1e-9 && segment_distance(*p, pa, pb).0 < 1e-9).then_some((t, *p))
        }).collect();
        on.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut next = on.iter().peekable();
        let len = len2.sqrt();
        for k in 1..steps {
            let t = k as f64 / steps as f64;
            while let Some((_, pc)) = next.next_if(|(tc, _)| *tc <= t) {
                run.push(cdt.insert(Point2::new(pc[0], pc[1])).ok()?);
            }
            if on.iter().any(|(tc, _)| (tc - t).abs() * len < 1e-6) { continue; }
            let q = [pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t];
            let h = cdt.insert(Point2::new(q[0], q[1])).ok()?;
            if let std::collections::hash_map::Entry::Vacant(v) = index.entry(h) {
                v.insert(points.len() as u32);
                points.push(q);
            }
            run.push(h);
        }
        for (_, pc) in next {
            run.push(cdt.insert(Point2::new(pc[0], pc[1])).ok()?);
        }
        run.push(handles[*b]);
        for w in run.windows(2) {
            if w[0] == w[1] || !cdt.can_add_constraint(w[0], w[1]) { return None; }
            cdt.add_constraint(w[0], w[1]);
            chord_edges.insert((w[0].min(w[1]), w[0].max(w[1])));
        }
    }
    // Crease lines subdivided at the pitch, split where they cross a chord.
    for (pa, pb) in &creases.lines {
        let steps = (((pb[0] - pa[0]).hypot(pb[1] - pa[1]) / pitch).ceil() as usize).max(1);
        let mut run = Vec::with_capacity(steps + 1);
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let q = [pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t];
            run.push(cdt.insert(Point2::new(q[0], q[1])).ok()?);
        }
        run.dedup();
        for w in run.windows(2) {
            cdt.add_constraint_and_split(w[0], w[1], |p| p);
        }
    }
    for v in cdt.fixed_vertices() {
        if let std::collections::hash_map::Entry::Vacant(e) = index.entry(v) {
            let p = cdt.vertex(v).position();
            e.insert(points.len() as u32);
            points.push([p.x, p.y]);
        }
    }
    let (lo, hi) = outline.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let segments: Vec<([f64; 2], [f64; 2])> = (0..n).map(|i| (outline[i], outline[(i + 1) % n]))
        .chain(chords.iter().map(|(a, b)| (outline[*a], outline[*b])))
        .chain(creases.lines.iter().copied())
        .collect();
    let clear = |p: [f64; 2]| segments.iter().all(|&(a, b)| {
        let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
        let t = (((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / (ex * ex + ey * ey).max(1e-18)).clamp(0.0, 1.0);
        (p[0] - a[0] - ex * t).hypot(p[1] - a[1] - ey * t) > pitch * 0.45
    }) && creases.points.iter().all(|c| (p[0] - c[0]).hypot(p[1] - c[1]) > pitch * 0.45);
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
    // An outline edge joins two outline points and is not a chord.
    let on_outline = |v: spade::handles::FixedVertexHandle| index.get(&v).is_some_and(|i| (*i as usize) < n);
    let outline_edge = |a: spade::handles::FixedVertexHandle, b: spade::handles::FixedVertexHandle, constraint: bool| {
        constraint && on_outline(a) && on_outline(b) && !chord_edges.contains(&(a.min(b), a.max(b)))
    };
    let mut inside = std::collections::HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for e in cdt.convex_hull() {
        if let Some(f) = e.face().as_inner().or_else(|| e.rev().face().as_inner()) {
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(f.fix()) {
                let [a, b] = e.vertices().map(|v| v.fix());
                v.insert(outline_edge(a, b, cdt.is_constraint_edge(e.as_undirected().fix())));
                queue.push_back(f.fix());
            }
        }
    }
    while let Some(f) = queue.pop_front() {
        let here = inside[&f];
        for e in cdt.face(f).adjacent_edges() {
            let Some(next) = e.rev().face().as_inner() else { continue };
            let [a, b] = e.vertices().map(|v| v.fix());
            let flips = outline_edge(a, b, cdt.is_constraint_edge(e.as_undirected().fix()));
            if let std::collections::hash_map::Entry::Vacant(v) = inside.entry(next.fix()) {
                v.insert(if flips { !here } else { here });
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
    /// Stamps a [`crate::Mesh::origin`] numbers after the stones: the design's own, as struck.
    pub stamps: usize,
}

impl Applied {
    /// The stone whose solid made a vertex of origin `origin`, by its index in `paths`.
    pub fn stone_of(&self, origin: u32) -> Option<usize> {
        let i = origin.checked_sub(crate::mesh::SOLID_VERTEX)? as usize;
        (i < self.paths.len()).then_some(i)
    }

    /// The stamp that made a vertex of origin `origin`, by its index in the design's stamps.
    pub fn stamp_of(&self, origin: u32) -> Option<usize> {
        let k = (origin.checked_sub(crate::mesh::SOLID_VERTEX)? as usize).checked_sub(self.paths.len())?;
        (k < self.stamps).then_some(k)
    }
}

/// Whether any enabled seat in the design carries a solid.
pub fn any(design: &crate::RingDesign) -> bool {
    !design.stamps.is_empty() || crate::setstone::seat_stones(design).iter().any(|s| !s.seat.solid.is_none())
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
    out.stamps = design.stamps.len();
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
    let mut tiers: Vec<u8> = design.stamps.iter().map(|s| s.tier).collect();
    tiers.sort_unstable();
    tiers.dedup();
    let mut frames = StampFrames::new(design, &ctx);
    let make = |tier: u8, on: &Solid, notes: &mut Vec<String>, frames: &mut StampFrames| -> Vec<(usize, Solid)> {
        let mut made = Vec::new();
        for (k, s) in design.stamps.iter().enumerate().filter(|(_, s)| s.tier == tier) {
            let frame = frames.get(k);
            let spill = overhang(design, frames, s, &frame);
            if !spill.is_empty() {
                notes.push(format!("{}: its cap leaves the stamps beneath it at {} of its {} outline points and drapes over their edges", s.name, spill.len(), s.outline.len()));
            }
            match s.solid(&frame, on) {
                Ok(solid) => made.push((k, solid)),
                Err(why) => notes.push(format!("{}: {why}", s.name)),
            }
        }
        made
    };
    let strike = |made: &[(usize, Solid)], op: Op, solid: &mut Solid, vouched: &mut bool, spans: &mut Vec<(usize, u32)>, out: &mut Applied| {
        let cut = op == Op::Subtract;
        for (k, part) in made.iter().filter(|(k, _)| design.stamps[*k].cut == cut) {
            match chain(solid, part, op, *vouched) {
                Ok(next) => {
                    *solid = next;
                    *vouched = true;
                    spans.push((solid.v.len(), (stones.len() + k) as u32));
                    out.stamped += 1;
                }
                Err(e) => out.notes.push(format!("{}: could not be {} ({e})", design.stamps[*k].name, if cut { "cut" } else { "joined to the band" })),
            }
        }
    };
    // The lowest tier is made against the band as swept.
    let first = tiers.first().map_or_else(Vec::new, |t| make(*t, &solid, &mut out.notes, &mut frames));
    for (op, pick) in [(Op::Union, 0usize), (Op::Subtract, 1)] {
        strike(&first, op, &mut solid, &mut vouched, &mut spans, &mut out);
        if pick == 1 {
            // Each higher tier is made against the ring as struck so far.
            for &tier in tiers.iter().skip(1) {
                let made = make(tier, &solid, &mut out.notes, &mut frames);
                for op in [Op::Union, Op::Subtract] {
                    strike(&made, op, &mut solid, &mut vouched, &mut spans, &mut out);
                }
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
    fn wire_geometry_pins() {
        fn fingerprint(s: &Solid) -> u64 {
            s.v.iter().flatten().map(|v| v.to_bits()).chain(s.f.iter().flatten().map(|v| *v as u64))
                .fold(0xcbf29ce484222325u64, |h, v| (h ^ v).wrapping_mul(0x100000001b3))
        }
        for (cut, expected_parts, expected_head) in [
            (GemCut::Round, [504612185168181325, 6803036983644726103, 4475010908983553063, 8558655448122456469, 6113582356115520274, 6748612298262231730], 0x35c0e5c7e6788a73),
            (GemCut::Oval, [2268300194296525938, 18140777434241358261, 17260596408947708632, 3376019039033878010, 8876282416183073339, 3731318358088716635], 0xed02712445ded99e),
        ] {
            let gem = Gem::calibrated(cut, 6.5);
            let parts = claw_parts(gem, 4);
            let head = claw_head(gem, 4).unwrap();
            // Master 27bc954 supplies every pinned vertex bit and face index, including the original foot, bend, caps and rails.
            assert_eq!(parts.iter().map(fingerprint).collect::<Vec<_>>(), expected_parts, "{cut:?}");
            assert_eq!(fingerprint(&head), expected_head, "{cut:?}");
            let explicit = claw_head_named_styled(gem, 4, prong_wire_mm(gem), Rails::Seat, None, ClawOptions::default()).unwrap();
            assert_eq!(head.v, explicit.solid.v); assert_eq!(head.f, explicit.solid.f);
        }
    }

    const CLAW_STYLES: [ClawStyle; 6] = [ClawStyle::Wire, ClawStyle::Talon, ClawStyle::Fang, ClawStyle::Tentacle, ClawStyle::Thorn, ClawStyle::Sepal];

    fn claw_bends_clear_the_local_radius(s: &Solid, who: &str) {
        // The unnotched Dome claw has 14 vertices per ring, four cap rings, and two poles.
        let rings = (s.v.len() - 2) / 14 - 4;
        let centres: Vec<P3> = s.v[..rings * 14].chunks_exact(14).map(|ring| std::array::from_fn(|i| ring.iter().map(|p| p[i]).sum::<f64>() / 14.0)).collect();
        let sub = |a: P3, b: P3| std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
        let norm = |v: P3| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        for i in 1..rings - 1 {
            let a = sub(centres[i], centres[i - 1]);
            let b = sub(centres[i + 1], centres[i]);
            let cross = norm([a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]);
            if cross < 1e-12 { continue; }
            let bend = norm(a) * norm(b) * norm(sub(centres[i + 1], centres[i - 1])) / (2.0 * cross);
            let radius = s.v[i * 14..(i + 1) * 14].iter().map(|p| norm(sub(*p, centres[i]))).fold(0.0, f64::max);
            assert!(bend + 1e-8 >= radius, "{who}: bend {bend} smaller than local radius {radius} at station {i}");
        }
    }

    #[test]
    fn every_claw_style_is_closed_uncrossed_and_notched_for_round_and_oval_three_to_eight() {
        for cut in [GemCut::Round, GemCut::Oval] {
            let gem = Gem::calibrated(cut, 6.5);
            let stone = envelope(gem, 0.0);
            for style in CLAW_STYLES {
                for prongs in 3..=8 {
                    let options = ClawOptions { style, ..ClawOptions::default() };
                    let who = format!("{cut:?} {style:?} {prongs}");
                    let (parts, _) = claw_parts_reach_styled(gem, prongs, prong_wire_mm(gem), Rails::Seat, None, None, options).unwrap_or_else(|e| panic!("{who}: {e}"));
                    for part in &parts {
                        sound(&part.solid, &who);
                        assert_eq!(csg::self_crossings(&part.solid), 0, "{who} {:?}", part.names);
                        if part.names[0].starts_with("Claw ") { claw_bends_clear_the_local_radius(&part.solid, &who); }
                    }
                    let raw = Named::union_all(parts).unwrap_or_else(|e| panic!("{who} join: {e}"));
                    let raw_volume = raw.solid.volume();
                    let mut head = raw.notched(&envelope(gem, 0.02)).unwrap_or_else(|e| panic!("{who} notch: {e}"));
                    head.compact();
                    sound(&head.solid, &who);
                    assert_eq!(csg::self_crossings(&head.solid), 0, "{who} notched head");
                    assert!(head.solid.volume() < raw_volume - 0.01, "{who}: notch removed no metal");
                    assert!(head.solid.v.iter().all(|p| csg::inside(&stone, *p) != Some(true)), "{who}: metal in the stone");
                }
            }
        }
    }

    #[test]
    fn claw_groupings_and_tips_keep_the_basket_and_each_girdle_bite() {
        let mut missing_bites = Vec::new();
        for cut in [GemCut::Round, GemCut::Oval] {
            let gem = Gem::calibrated(cut, 6.5);
            let plan = Plan::of(gem);
            let stone = envelope(gem, 0.0);
            let wire = prong_wire_mm(gem);
            for grouping in [ClawGrouping::Even, ClawGrouping::Feet, ClawGrouping::Jaws] {
                for style in CLAW_STYLES {
                    for tip in [ClawTip::Dome, ClawTip::Point] {
                        for prongs in 3..=8 {
                            let options = ClawOptions { style, grouping, tip };
                            let who = format!("{cut:?} {prongs} {options:?}");
                            let (parts, _) = claw_parts_reach_styled(gem, prongs, wire, Rails::Basket(3), None, None, options).unwrap_or_else(|e| panic!("{who}: {e}"));
                            for (part, phi) in parts.iter().take(prongs as usize).zip(grouped_claw_angles(&plan, prongs, wire, grouping)) {
                                sound(&part.solid, &who);
                                assert_eq!(csg::self_crossings(&part.solid), 0, "{who} {:?}", part.names);
                                let (o, n) = (plan.point(phi), plan.normal(phi));
                                let bites = (1..=30).any(|k| {
                                    let p = [o[0] - n[0] * 0.005 * k as f64, o[1] - n[1] * 0.005 * k as f64, 0.0];
                                    csg::inside(&stone, p) == Some(true) && csg::inside(&part.solid, p) == Some(true)
                                });
                                if !bites { missing_bites.push(format!("{who} {:?}", part.names)); }
                            }
                            let head = claw_head_named_styled(gem, prongs, wire, Rails::Basket(3), None, options).unwrap_or_else(|e| panic!("{who}: {e}"));
                            sound(&head.solid, &who);
                            assert_eq!(csg::self_crossings(&head.solid), 0, "{who}");
                            assert!(head.names.contains(&"Gallery rail 2".into()));
                        }
                    }
                }
            }
        }
        assert!(missing_bites.is_empty(), "no girdle bite before notching: {missing_bites:#?}");
    }

    #[test]
    fn claw_styles_fit_the_bestiarium_cabochons() {
        for cut in [GemCut::Round, GemCut::Oval] {
            let gem = Gem::cabochon(cut, 10.0);
            let stone = envelope(gem, 0.0);
            for style in CLAW_STYLES {
                for grouping in [ClawGrouping::Even, ClawGrouping::Jaws] {
                    let options = ClawOptions { style, grouping, tip: ClawTip::Point };
                    let who = format!("10 mm {cut:?} cabochon {options:?}");
                    let (parts, _) = claw_parts_reach_styled(gem, 6, 1.4, Rails::Seat, None, None, options).unwrap_or_else(|e| panic!("{who}: {e}"));
                    for part in &parts { assert_eq!(csg::self_crossings(&part.solid), 0, "{who} {:?}", part.names); }
                    let head = claw_head_named_styled(gem, 6, 1.4, Rails::Seat, None, options).unwrap_or_else(|e| panic!("{who}: {e}"));
                    sound(&head.solid, &who);
                    assert_eq!(csg::self_crossings(&head.solid), 0, "{who}");
                    assert!(head.solid.v.iter().all(|p| csg::inside(&stone, *p) != Some(true)), "{who}: metal in stone");
                }
            }
        }
    }

    #[test]
    fn styled_claws_keep_the_existing_foot_reach_and_actual_bore_wall() {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let wire = prong_wire_mm(gem);
        let (shallow, deep, wall) = (|_: [f64; 2]| Some(-4.2), |_: [f64; 2]| Some(-4.6), |p: P3| p[2] + 5.5);
        for style in CLAW_STYLES {
            let options = ClawOptions { style, ..ClawOptions::default() };
            let (_, reach) = claw_parts_reach_styled(gem, 4, wire, Rails::Seat, Some(&shallow), Some(&wall), options).unwrap();
            assert_eq!(reach, Reach { reached: 4, walled: 0 }, "{style:?}");
            let head = claw_head_within_styled(gem, 4, wire, Rails::Seat, Some(&shallow), &wall, options).unwrap();
            assert!(thinnest_wall(&head.solid, &wall) >= crate::mesh::MIN_WALL_MM, "{style:?}");
            assert_eq!(claw_head_within_styled(gem, 4, wire, Rails::Seat, Some(&deep), &wall, options).unwrap_err(), HeadSnag::NoReach, "{style:?}");
        }
    }

    #[test]
    fn oval_tubes_keep_round_bytes_and_flatten_in_the_bending_plane() {
        let path: Vec<P3> = (0..=24).map(|k| { let a = k as f64 * PI / 48.0; [3.0 * a.cos(), 0.0, 3.0 * a.sin()] }).collect();
        let round = tube(&path, &vec![0.4; path.len()], 18, (true, true));
        let equal = tube_oval(&path, &vec![(0.4, 0.4); path.len()], 18, (true, true));
        assert_eq!(round.v, equal.v); assert_eq!(round.f, equal.f);
        let oval = tube_oval(&path, &vec![(0.16, 0.50); path.len()], 24, (true, true));
        sound(&oval, "oval wire");
        assert_eq!(csg::self_crossings(&oval), 0);
        let (lo, hi) = oval.bounds().unwrap();
        assert!(hi[1] - lo[1] > 0.98 && hi[0] < 3.17 && hi[2] < 3.17, "{lo:?}..{hi:?}");
        assert!(tube_oval(&path, &[(0.1, 0.2)], usize::MAX, (true, true)).f.is_empty());
        let capped = tube_oval(&path, &vec![(0.1, 0.2); path.len()], usize::MAX, (false, false));
        assert_eq!(capped.v.len(), path.len() * 128 + 2);
    }

    #[test]
    fn oval_tubes_refuse_a_zero_central_tangent() {
        let path = [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
        for radii in [[(0.1, 0.2); 3], [(0.1, 0.1); 3]] {
            let solid = tube_oval(&path, &radii, 14, (true, true));
            assert!(solid.v.is_empty() && solid.f.is_empty());
        }
    }

    #[test]
    fn oval_tubes_refuse_a_frame_that_projects_to_zero() {
        let path = [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]];
        for radii in [[(0.1, 0.2); 3], [(0.1, 0.1); 3]] {
            let solid = tube_oval(&path, &radii, 14, (false, false));
            assert!(solid.v.is_empty() && solid.f.is_empty());
        }
    }

    #[test]
    fn fangs_are_straight_inward_cones_with_the_original_feet_and_live_girdle_stock() {
        for cut in [GemCut::Round, GemCut::Oval] {
            for gem in [Gem::calibrated(cut, 6.5), Gem::cabochon(cut, 10.0)] {
                let wire = prong_wire_mm(gem);
                let plan = Plan::of(gem);
                let legacy = claw_parts(gem, 4);
                let options = ClawOptions { style: ClawStyle::Fang, ..Default::default() };
                let (parts, _) = claw_parts_reach_styled(gem, 4, wire, Rails::Seat, None, None, options).unwrap();
                for ((part, old), phi) in parts.iter().take(4).zip(&legacy).zip(plan.claw_angles(4)) {
                    let rings = (part.solid.v.len() - 2) / 14 - 4;
                    let centres: Vec<P3> = part.solid.v[..rings * 14].chunks_exact(14).map(|ring| std::array::from_fn(|i| ring.iter().map(|p| p[i]).sum::<f64>() / 14.0)).collect();
                    let first: P3 = std::array::from_fn(|i| old.v[..14].iter().map(|p| p[i]).sum::<f64>() / 14.0);
                    for i in 0..3 { assert!((first[i] - centres[0][i]).abs() < 1e-12); }
                    let (a, b) = (centres[0], *centres.last().unwrap());
                    let n = plan.normal(phi);
                    assert!((b[0] - a[0]) * n[0] + (b[1] - a[1]) * n[1] < 0.0);
                    for p in &centres {
                        let t = (p[2] - a[2]) / (b[2] - a[2]);
                        assert!((p[0] - a[0] - t * (b[0] - a[0])).abs() < 1e-12);
                        assert!((p[1] - a[1] - t * (b[1] - a[1])).abs() < 1e-12);
                    }
                    let t = -a[2] / (b[2] - a[2]);
                    let center: P3 = std::array::from_fn(|i| a[i] + t * (b[i] - a[i]));
                    let o = plan.point(phi);
                    let stock = [o[0] + n[0] * 0.15 * wire, o[1] + n[1] * 0.15 * wire, center[2]];
                    assert_eq!(csg::inside(&part.solid, stock), Some(true), "{cut:?} {:?}: the notch must leave connected metal outside the girdle", gem.form);
                }
            }
        }
    }

    /// Manual visual audit, like the CAD gallery: `RD_CLAW_GALLERY` chooses the output directory.
    #[test]
    #[ignore]
    fn claw_styles_gallery() {
        let Some(dest) = std::env::var_os("RD_CLAW_GALLERY") else { return };
        std::fs::create_dir_all(&dest).unwrap();
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let mesh = |s: Solid| { let n = s.v.len(); crate::parts::into_mesh(s, &[], vec![u32::MAX; n]) };
        let stone = mesh(envelope(gem, 0.0));
        for style in CLAW_STYLES {
            for tip in [ClawTip::Dome, ClawTip::Point] {
                let options = ClawOptions { style, tip, ..ClawOptions::default() };
                let head = mesh(claw_head_named_styled(gem, 4, prong_wire_mm(gem), Rails::Seat, None, options).unwrap().solid);
                let parts = [
                    crate::render::Part::metal(&head, [1.0, 0.766, 0.336]),
                    crate::render::Part::tinted_stone(&stone, [0.08, 0.20, 0.28]),
                ];
                let path = std::path::Path::new(&dest).join(format!("{style:?}-{tip:?}.png"));
                crate::render::write_png_parts(path, &parts, 0.55, -1.12, 480).unwrap();
                let (parts, _) = claw_parts_reach_styled(gem, 3, prong_wire_mm(gem), Rails::Seat, None, None, options).unwrap();
                let first = mesh(parts[0].solid.clone());
                let path = std::path::Path::new(&dest).join(format!("{style:?}-{tip:?}-profile.png"));
                crate::render::write_png(path, &first, 0.0, -PI * 0.5, 480, [1.0, 0.766, 0.336]).unwrap();
            }
        }
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
        let (p, _) = casting_pattern(&d, &lib);
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
        let stamp = Stamp { name: "Dent".into(), theta_deg: 90.0, v_mm: 0.0, rot_deg: 0.0, outline, height_mm: 0.4, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: StampTop::Flat };
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
        let blank = Stamp { name: "Half moon".into(), theta_deg: 60.0, v_mm: v, rot_deg: 0.0, outline: moon_outline(1.6, 0.5, 0.85), height_mm: 0.45, sink_mm: 0.35, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: StampTop::Flat };
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

    fn plain(name: &str, theta_deg: f64, v_mm: f64, outline: Vec<[f64; 2]>, height_mm: f64) -> Stamp {
        Stamp { name: name.into(), theta_deg, v_mm, rot_deg: 0.0, outline, height_mm, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: StampTop::Flat }
    }

    fn crest_band() -> crate::RingDesign {
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 2.4;
        d
    }

    fn strike() -> crate::BuildParams {
        crate::BuildParams { theta_steps: 384, profile_steps: 192, ..Default::default() }
    }

    #[test]
    fn a_plain_stamp_reads_and_writes_as_it_did_and_a_shaped_one_is_fenced() {
        use crate::library::{format_version_for, FORMAT_VERSION, PLAIN_FORMAT_VERSION};
        let mut d = crest_band();
        let v = d.field_context().crest_v_mm;
        let flat = plain("Disc", 90.0, v, crate::outline::circle(1.6), 0.4);
        let json = serde_json::to_value(&flat).unwrap();
        assert!(json.get("tier").is_none() && json.get("top").is_none(), "{json}");
        let old: Stamp = serde_json::from_str(r#"{"name":"Old","theta_deg":90.0,"v_mm":1.0,"outline":[[0,0],[1,0],[0,1]],"height_mm":0.3,"sink_mm":0.2}"#).unwrap();
        assert!(old.is_plain());
        d.stamps = vec![flat.clone()];
        assert_eq!(format_version_for(&d), PLAIN_FORMAT_VERSION);
        let cone = StampTop::Cone { apex_mm: 0.6, at: [0.1, 0.0], tip_mm: 0.3 };
        for shaped in [Stamp { tier: 1, ..flat.clone() }, Stamp { top: cone, ..flat.clone() }] {
            d.stamps = vec![flat.clone(), shaped.clone()];
            assert_eq!(format_version_for(&d), FORMAT_VERSION, "an older build would strike {shaped:?} flat, on the band");
            let back = crate::library::load_design_str(&crate::library::design_json(&d).unwrap()).unwrap();
            assert_eq!(back.stamps, d.stamps);
        }
        assert_eq!(serde_json::to_value(cone).unwrap(), serde_json::json!({"Cone": {"apex_mm": 0.6, "at": [0.1, 0.0], "tip_mm": 0.3}}));
        // A format-5 build refuses an outline past 512 points, so a flat tier-0 stamp carrying one is fenced too.
        let ring = |n: usize| (0..n).map(|i| { let t = 2.0 * PI * i as f64 / n as f64; [1.2 * t.cos(), 1.2 * t.sin()] }).collect::<Vec<_>>();
        let spiral = crate::outline::spiral(2.5, 0.5, 2.8, 0.4, 0.7);
        assert_eq!(spiral.len(), 540);
        for (outline, version) in [(ring(512), PLAIN_FORMAT_VERSION), (ring(513), FORMAT_VERSION), (spiral, FORMAT_VERSION)] {
            let n = outline.len();
            d.stamps = vec![Stamp { outline, ..flat.clone() }];
            assert_eq!(format_version_for(&d), version, "a flat stamp of {n} points");
        }
    }

    #[test]
    fn a_design_of_plain_stamps_stands_them_where_a_format_5_build_did() {
        let heart = crate::templates::all().iter().find(|t| t.name == "Heart signet").unwrap().design();
        for d in [crest_band(), heart] {
            let ctx = d.field_context();
            let surface = BareSurface::new(&d, &ctx);
            for i in 0..50 {
                let (theta, v) = (7.3 * i as f64, ctx.band_v_len_mm * (i % 10) as f64 / 9.0);
                assert_eq!(surface.nearest_at(theta, v), crate::stones::surface_frame(&d, &ctx, theta, v), "{} at {theta} deg, {v:.3} mm", d.name);
            }
        }
        let mut d = crest_band();
        let ctx = d.field_context();
        d.stamps = vec![plain("Plate", 90.0, ctx.crest_v_mm, crate::outline::circle(2.4), 0.3)];
        let z = d.stamps[0].frame(&d, &ctx).origin[2];
        assert!((z + 0.043380).abs() < 1e-6, "a plain design's plate stands {z:.6} mm off the parting line, where a format-5 build put it at -0.043380");
        // A design only a format-6 build opens reads every stamp between the samples, so a tier stands concentric on its plate.
        d.stamps.push(Stamp { tier: 1, ..plain("Boss", 90.0, ctx.crest_v_mm, crate::outline::circle(1.2), 0.2) });
        let (plate, boss) = (d.stamps[0].frame(&d, &ctx), d.stamps[1].frame(&d, &ctx));
        assert_eq!(plate.origin[2], 0.0);
        assert_eq!(plate.origin, boss.origin);
        // A tier cut at the bench leaves the poured plate where the pattern, which leaves the tier out, strikes it.
        d.stamps[1].bench = true;
        let (plate, boss) = (d.stamps[0].frame(&d, &ctx), d.stamps[1].frame(&d, &ctx));
        let pattern = crate::castability::pattern_parts(&d, &crate::AlphaLibrary::builtin()).design;
        assert_eq!(pattern.stamps.len(), 1);
        assert_eq!(plate.origin, pattern.stamps[0].frame(&pattern, &ctx).origin);
        assert!((plate.origin[2] + 0.043380).abs() < 1e-6 && plate.origin == boss.origin, "{:?} {:?}", plate.origin, boss.origin);
    }

    #[test]
    fn a_cone_on_the_crest_and_a_tier_on_a_stamp_pull_clean() {
        use crate::manufacturing::{inspect, release, Setup};
        let mut d = crest_band();
        let ctx = d.field_context();
        let (v, crest) = (crest_v(&d, 90.0).unwrap(), ctx.crest_radius_mm);
        let lib = crate::AlphaLibrary::builtin();
        let bare = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        let thorn = Stamp { top: StampTop::Cone { apex_mm: 0.7, at: [0.0, 0.0], tip_mm: 0.3 }, ..plain("Thorn", 60.0, v, crate::outline::circle(1.6), 0.25) };
        let plate = plain("Plate", 120.0, v, crate::outline::circle(3.2), 0.35);
        let keel = Stamp { tier: 1, top: StampTop::Gable { rise_mm: 0.25, axis_deg: 0.0 }, ..plain("Plate: keel", 120.0, v, crate::outline::keel(2.2, 1.2, 0.18), 0.3) };
        d.stamps = vec![thorn, plate, keel];
        for s in &d.stamps {
            assert_eq!(s.parting_monotone(&d), Ok(()), "{}", s.name);
        }
        let built = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        assert!(built.solids.notes.is_empty(), "{:?}", built.solids.notes);
        assert_eq!(built.solids.stamped, 3);
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert_eq!(built.report.quality.degenerate_faces, 0);
        // The thorn rises to its blunted apex, and the keel stands on the plate rather than on the band.
        let reach = |theta: f64| built.mesh.vertices.iter().filter(|p| {
            let a = (p.1 as f64).atan2(p.0 as f64).to_degrees().rem_euclid(360.0);
            (a - theta).abs() < 6.0
        }).map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(0.0, f64::max);
        let (tip, top) = (reach(60.0) - crest, reach(120.0) - crest);
        assert!((tip - 0.95).abs() < 0.02, "the thorn stands {tip:.3} over the crest");
        assert!((top - 0.90).abs() < 0.03, "the keel's ridge stands {top:.3} over the crest, on the plate");
        let volume = built.report.volume_mm3 - bare.report.volume_mm3;
        let flat = PI * 0.8 * 0.8 * 0.25 + PI * 1.6 * 1.6 * 0.35;
        // The cone over its disc and the gabled keel on the plate: 1.43 mm³ measured over the flat stamps.
        assert!(volume > flat + 1.1 && volume < flat + 1.7, "{volume:.3} over the flat stamps' {flat:.3}");
        // Every poured face above the parting line faces up, and every one below it down.
        let m = &built.mesh;
        for f in m.faces.iter().filter(|f| f.iter().any(|v| m.origin[*v as usize] >= crate::mesh::SOLID_VERTEX)) {
            let [a, b, c] = f.map(|v| m.vertices[v as usize]);
            let n = crate::mesh::cross([(b.0 - a.0) as f64, (b.1 - a.1) as f64, (b.2 - a.2) as f64], [(c.0 - a.0) as f64, (c.1 - a.1) as f64, (c.2 - a.2) as f64]);
            let mid = (a.2 + b.2 + c.2) as f64 / 3.0;
            let area = 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(area < 1e-5 || mid.abs() < 1e-3 || mid.signum() * n[2] / (2.0 * area) >= -1e-3, "a stamp face tips into its own mould half at z {mid}, area {area:.2e}");
        }
        // Judged as Caiman's horns are: the field, and rays pulled out of the sand at two pitches.
        let setup = Setup::from_design(&d);
        let inspection = inspect(&d, &lib, &setup, strike()).unwrap();
        assert_eq!(inspection.field.as_ref().unwrap().verdict, crate::castability::Verdict::Castable);
        assert!(inspection.release.obstructions.is_empty() && inspection.release.unresolved_rays == 0, "{:?}", inspection.release.obstructions);
        let fine = Setup { sample_pitch_mm: 0.075, ..setup.clone() };
        let rel = release::analyze(&inspection.prepared.mesh, &fine).unwrap();
        assert!(rel.obstructions.is_empty() && rel.unresolved_rays == 0, "{:?}", rel.obstructions);
        assert!(crate::dfm::findings_in(&d, &lib).is_empty());
        // A cone whose apex stands outside its outline is refused by name.
        d.stamps[0].top = StampTop::Cone { apex_mm: 0.7, at: [2.0, 0.0], tip_mm: 0.3 };
        let off = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        assert!(off.solids.notes.iter().any(|n| n.starts_with("Thorn: its cone's apex")), "{:?}", off.solids.notes);
    }

    #[test]
    fn a_crease_is_an_edge_of_the_cap_and_the_cap_stays_whole() {
        let corners: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let square: Vec<[f64; 2]> = (0..4).flat_map(|i| {
            let (c, n) = (corners[i], corners[(i + 1) % 4]);
            (0..20).map(move |k| { let f = k as f64 / 20.0; [c[0] + (n[0] - c[0]) * f, c[1] + (n[1] - c[1]) * f] })
        }).collect();
        // A chord across the middle, and a crease crossing it and two more from a point beside it.
        let chords = [(70, 30)];
        let creases = Creases { points: vec![[0.3, 0.2]], lines: vec![([0.0, -0.8], [0.0, 0.8]), ([0.3, 0.2], [0.9, 0.9]), ([0.3, 0.2], [-0.9, 0.9])] };
        let (points, tris) = cap_faces_with(&square, 0.2, &chords, &creases).unwrap();
        let covered: f64 = tris.iter().map(|t| crate::outline::area(&t.map(|i| points[i as usize]))).sum();
        assert!((covered - 4.0).abs() < 1e-9, "{covered}");
        let mut uses = std::collections::HashMap::new();
        for t in &tris { for k in 0..3 { *uses.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1; } }
        assert!(uses.values().all(|n| *n == 1));
        let n = square.len() as u32;
        let boundary: std::collections::HashSet<(u32, u32)> = uses.keys().copied().filter(|(a, b)| !uses.contains_key(&(*b, *a))).collect();
        assert_eq!(boundary, (0..n).map(|i| (i, (i + 1) % n)).collect());
        // Every crease is carried by edges: no triangle straddles one.
        for &(a, b) in &creases.lines {
            for t in &tris {
                let side: Vec<f64> = t.iter().map(|i| { let p = points[*i as usize]; (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) }).collect();
                let c = t.iter().map(|i| points[*i as usize]).fold([0.0, 0.0], |s, p| [s[0] + p[0] / 3.0, s[1] + p[1] / 3.0]);
                let (_, along) = segment_distance(c, a, b);
                if along > 0.05 && along < 0.95 {
                    assert!(!(side.iter().any(|s| *s > 1e-9) && side.iter().any(|s| *s < -1e-9)), "a triangle straddles the crease {a:?}-{b:?}");
                }
            }
        }
    }

    /// A gable off the parting line holds its ridge as an edge of the cap, every vertex on it at full rise.
    #[test]
    fn an_off_chord_gable_creases_its_ridge_at_full_rise() {
        let d = crest_band();
        let ctx = d.field_context();
        let lib = crate::AlphaLibrary::builtin();
        let mesh = crate::mesh::try_build(&d, &lib, strike()).unwrap().mesh;
        let band = Solid { v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: mesh.faces.clone() };
        let (height, rise) = (0.3, 0.3);
        let s = Stamp { top: StampTop::Gable { rise_mm: rise, axis_deg: 0.0 }, ..plain("Keel", 90.0, ctx.crest_v_mm + 1.8, crate::outline::keel(4.0, 1.6, 0.18), height) };
        let frame = s.frame(&d, &ctx);
        let made = s.solid(&frame, &band).unwrap();
        sound(&made, "the keel");
        let local = |w: &P3| {
            let q = [w[0] - frame.origin[0], w[1] - frame.origin[1], w[2] - frame.origin[2]];
            [frame.x, frame.y, frame.z].map(|a| q[0] * a[0] + q[1] * a[1] + q[2] * a[2])
        };
        let count = made.v.len() / 2;
        let (top, floor): (Vec<P3>, Vec<P3>) = (made.v[..count].iter().map(local).collect(), made.v[count..].iter().map(local).collect());
        let ridge: Vec<usize> = (0..count).filter(|i| top[*i][1].abs() < 1e-9).collect();
        assert!(ridge.len() >= 15, "{} cap vertices on the ridge", ridge.len());
        for i in ridge {
            let lift = top[i][2] - floor[i][2] - height - s.sink_mm;
            assert!((lift - rise).abs() < 1e-9, "the ridge stands {lift:.4} at x {:.3}", top[i][0]);
        }
        for f in made.f.iter().filter(|f| f.iter().all(|k| (*k as usize) < count)) {
            let ys = f.map(|k| top[k as usize][1]);
            assert!(!(ys.iter().any(|y| *y > 1e-9) && ys.iter().any(|y| *y < -1e-9)), "a cap facet straddles the ridge: {ys:?}");
        }
    }

    /// A higher tier spilling past the stamp it stands on, or over a pit in it, is refused by the draft check and named by the build.
    #[test]
    fn a_tier_that_leaves_its_base_is_refused_and_named() {
        let mut d = crest_band();
        let (v, r) = (crest_v(&d, 90.0).unwrap(), d.field_context().crest_radius_mm);
        let lib = crate::AlphaLibrary::builtin();
        let plate = plain("Plate", 90.0, v, crate::outline::circle(1.6), 0.4);
        let on = |name: &str, dia: f64, off_mm: f64| Stamp { tier: 1, ..plain(name, 90.0 + (off_mm / r).to_degrees(), v, crate::outline::circle(dia), 0.3) };
        d.stamps = vec![plate.clone(), on("Boss", 1.2, 0.0)];
        assert_eq!(d.stamps[1].parting_monotone(&d), Ok(()));
        let built = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        assert!(built.solids.notes.is_empty() && built.solids.stamped == 2, "{:?}", built.solids.notes);
        for upper in [on("Cap", 2.6, 0.0), on("Boss", 1.2, 0.9)] {
            d.stamps = vec![plate.clone(), upper.clone()];
            let bad = upper.parting_monotone(&d).expect_err(&upper.name);
            assert!(!bad.is_empty() && bad.iter().all(|i| *i < upper.outline.len()));
            let built = crate::mesh::try_build(&d, &lib, strike()).unwrap();
            assert!(built.solids.notes.iter().any(|n| n.starts_with(&format!("{}: its cap leaves", upper.name))), "{:?}", built.solids.notes);
        }
        // Over a pit cut in its plate it drops into the pit.
        let pit = Stamp { cut: true, bench: true, ..plain("Pit", 90.0, v, crate::outline::circle(0.5), 0.2) };
        d.stamps = vec![plate.clone(), pit, on("Boss", 1.2, 0.0)];
        assert!(d.stamps[2].parting_monotone(&d).is_err());
        // With nothing beneath it a higher tier stands on the band as a lower one does.
        d.stamps = vec![plate, on("Stud", 1.0, 6.0)];
        assert_eq!(d.stamps[1].parting_monotone(&d), Ok(()));
    }

    #[test]
    fn shaped_tops_straddling_the_parting_line_crease_and_pull() {
        use crate::manufacturing::{inspect, Setup};
        let mut d = crest_band();
        let ctx = d.field_context();
        let (v, crest) = (crest_v(&d, 90.0).unwrap(), ctx.crest_radius_mm);
        let lib = crate::AlphaLibrary::builtin();
        let keel = || crate::outline::keel(2.4, 1.3, 0.18);
        d.stamps = vec![
            Stamp { top: StampTop::Cone { apex_mm: 0.5, at: [0.3, 0.0], tip_mm: 0.0 }, ..plain("Horn", 40.0, v, keel(), 0.2) },
            Stamp { top: StampTop::Gable { rise_mm: 0.3, axis_deg: 90.0 }, ..plain("Saddle", 90.0, v, crate::outline::circle(1.8), 0.25) },
            Stamp { top: StampTop::Ridge { rise_mm: 0.35, from: [-0.6, 0.0], to: [0.6, 0.0], end_mm: 0.1 }, ..plain("Rib", 140.0, v, keel(), 0.2) },
            Stamp { top: StampTop::Cone { apex_mm: 0.3, at: [0.0, 0.2], tip_mm: 0.2 }, bench: true, ..plain("Bench boss", 200.0, v, keel(), 0.2) },
        ];
        assert_eq!(d.stamps[0].frame(&d, &ctx).origin[2], 0.0, "placed exactly on the parting line");
        for s in &d.stamps[..3] {
            assert_eq!(s.parting_monotone(&d), Ok(()), "{}", s.name);
        }
        let built = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        assert!(built.solids.notes.is_empty(), "{:?}", built.solids.notes);
        assert!(built.report.validation.watertight && built.solids.stamped == 4, "{:?}", built.report.validation);
        assert_eq!(built.report.quality.degenerate_faces, 0);
        let reach = |theta: f64| built.mesh.vertices.iter().filter(|p| {
            let a = (p.1 as f64).atan2(p.0 as f64).to_degrees().rem_euclid(360.0);
            (a - theta).abs() < 6.0
        }).map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(0.0, f64::max) - crest;
        for (theta, want) in [(40.0, 0.7), (90.0, 0.55), (140.0, 0.55), (200.0, 0.5)] {
            assert!((reach(theta) - want).abs() < 0.03, "{theta}: {} against {want}", reach(theta));
        }
        let inspection = inspect(&d, &lib, &Setup::from_design(&d), strike()).unwrap();
        assert!(inspection.release.obstructions.is_empty() && inspection.release.unresolved_rays == 0, "{:?}", inspection.release.obstructions);
    }

    #[test]
    fn parting_monotone_reads_the_margin_and_the_top() {
        let mut d = crest_band();
        let v = crest_v(&d, 90.0).unwrap();
        let holly = crate::outline::leaf(crate::outline::Margin::Holly { spines: 3, depth_mm: 0.5, lean_deg: 20.0 }, 5.2, 3.0);
        let along = plain("Holly", 90.0, v, holly.clone(), 0.4);
        // Struck where a format-5 build strikes it, 0.043 mm off the parting line, its tip hooks back over the line.
        assert_eq!(along.parting_monotone(&d), Err(vec![56, 57]));
        // A design carrying a tier is read between the samples, exactly on the line.
        d.stamps = vec![Stamp { tier: 1, ..plain("Palm stud", 270.0, v, crate::outline::circle(0.8), 0.2) }];
        assert_eq!(along.parting_monotone(&d), Ok(()));
        let across = Stamp { rot_deg: 90.0, ..along.clone() };
        assert!(across.parting_monotone(&d).is_err(), "spines laid across the parting line hook back over it");
        let moon = |lit: f64, rot: f64| Stamp { rot_deg: rot, ..plain("Moon", 90.0, v, moon_outline(1.2, lit, 0.85), 0.4) };
        assert_eq!(moon(0.5, 0.0).parting_monotone(&d), Ok(()));
        for rot in [0.0, 90.0, 180.0] {
            assert!(moon(0.25, rot).parting_monotone(&d).is_err(), "a crescent at {rot}");
        }
        let off = plain("Off the crest", 90.0, v + 1.2, crate::outline::circle(1.2), 0.3);
        assert!(off.parting_monotone(&d).is_err());
        let cone = |at: [f64; 2]| Stamp { top: StampTop::Cone { apex_mm: 0.6, at, tip_mm: 0.2 }, ..plain("Cone", 90.0, v, crate::outline::circle(1.6), 0.2) };
        assert_eq!(cone([0.3, 0.0]).parting_monotone(&d), Ok(()));
        assert!(cone([0.0, 0.4]).parting_monotone(&d).is_err(), "an apex off the parting line rises away from it");
        let shaped = |top: StampTop| Stamp { top, ..plain("Shaped", 90.0, v, crate::outline::keel(2.4, 1.2, 0.2), 0.3) };
        assert_eq!(shaped(StampTop::Gable { rise_mm: 0.3, axis_deg: 0.0 }).parting_monotone(&d), Ok(()));
        assert_eq!(shaped(StampTop::Gable { rise_mm: 0.3, axis_deg: 90.0 }).parting_monotone(&d), Ok(()));
        assert_eq!(shaped(StampTop::Taper { axis_deg: 0.0, tip_mm: 0.1 }).parting_monotone(&d), Ok(()));
        assert!(shaped(StampTop::Taper { axis_deg: 90.0, tip_mm: 0.1 }).parting_monotone(&d).is_err());
        assert_eq!(shaped(StampTop::Dome { crown_mm: 0.3 }).parting_monotone(&d), Ok(()));
        let pit = Stamp { cut: true, ..plain("Pit", 90.0, v, crate::outline::circle(1.0), 0.3) };
        assert!(pit.parting_monotone(&d).is_err());
        assert_eq!(Stamp { bench: true, ..pit }.parting_monotone(&d), Ok(()));
        // On a side face every wall stands along the pull, whatever the outline.
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.thickness_mm = 3.0;
        d.profile.flatten_sides();
        let ctx = d.field_context();
        let (a, b) = ctx.side_faces_std().unwrap().high.unwrap();
        let side = Stamp { along_pull: true, ..moon(0.25, 0.0) };
        assert_eq!(Stamp { v_mm: 0.5 * (a + b), ..side }.parting_monotone(&d), Ok(()));
    }

    #[test]
    fn a_row_grades_holds_its_gap_mirrors_and_follows_the_parting_line() {
        let mut d = crest_band();
        let ctx = d.field_context();
        let (v, r) = (ctx.crest_v_mm, ctx.crest_radius_mm);
        let proto = plain("Thorn", 0.0, 0.0, crate::outline::keel(1.2, 0.6, 0.12), 0.3);
        let row = StampRow { stamp: proto, path: RowPath::ChartV { v_mm: v }, from_deg: 100.0, to_deg: 160.0, count: 8, taper: 0.5, fold_clear_mm: 0.0, mirror_shoulders: true };
        let s = stamp_row(&d, &row);
        assert_eq!(s.len(), 16);
        let half = |st: &Stamp| st.outline.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!((s[0].theta_deg - 100.0).abs() < 1e-6 && (s[7].theta_deg - 160.0).abs() < 1e-6);
        assert!((half(&s[0]) - 0.6).abs() < 1e-9 && (half(&s[7]) - 0.3).abs() < 1e-9, "graded from full size to half");
        let gaps: Vec<f64> = s[..8].windows(2).map(|w| r * (w[1].theta_deg - w[0].theta_deg).to_radians() - half(&w[0]) - half(&w[1])).collect();
        let spread = gaps.iter().fold(f64::MIN, |m, g| m.max(*g)) - gaps.iter().fold(f64::MAX, |m, g| m.min(*g));
        assert!(spread < 2e-3 && gaps[0] > 0.1, "one gap down the whole row: {gaps:?}");
        assert!(s[..8].windows(2).all(|w| half(&w[1]) < half(&w[0])));
        for (k, (a, b)) in s[..8].iter().zip(&s[8..]).enumerate() {
            assert_eq!(a.name, format!("Thorn, right {}", k + 1));
            assert_eq!(b.name, format!("Thorn, left {}", k + 1));
            assert!((b.theta_deg - (180.0 - a.theta_deg)).abs() < 1e-9);
            let n = a.outline.len();
            assert!((0..n).all(|i| b.outline[i] == [-a.outline[n - 1 - i][0], a.outline[n - 1 - i][1]]));
            assert!(crate::outline::area(&b.outline) > 0.0);
        }
        // On a wave the parting line leaves the reference crest, and the row rides it exactly.
        d.shank.kind = crate::ShankKind::Wave;
        d.shank.amount = 0.7;
        let ctx = d.field_context();
        let wave = stamp_row(&d, &StampRow { path: RowPath::PartingLine, from_deg: 0.0, to_deg: 300.0, count: 11, taper: 0.0, mirror_shoulders: false, ..row.clone() });
        assert_eq!(wave.len(), 11);
        for st in &wave {
            assert!(BareSurface::new(&d, &ctx).section_point(st.theta_deg, st.v_mm).0[1].abs() < 1e-9, "{} stands off the parting line", st.name);
            assert!(st.parting_monotone(&d).is_ok(), "{}", st.name);
        }
        let wander = wave.iter().map(|st| (st.v_mm - ctx.crest_v_mm).abs()).fold(0.0, f64::max);
        assert!(wander > 0.3, "the wave carries the parting line {wander:.3} mm off the reference crest");
        let names: Vec<_> = wave.iter().map(|st| st.name.clone()).collect();
        assert_eq!(names[10], "Thorn, 11");
        // Across a side face the row stands on the face, square to the pull.
        let mut flat = crate::RingDesign::default();
        flat.profile.apply_style(crate::ProfileStyle::Flat);
        flat.profile.thickness_mm = 3.0;
        flat.profile.flatten_sides();
        let fctx = flat.field_context();
        let (a, b) = fctx.side_faces_std().unwrap().high.unwrap();
        let side = stamp_row(&flat, &StampRow { path: RowPath::SideFace { high: true, frac: 0.5 }, taper: 0.0, mirror_shoulders: false, ..row.clone() });
        assert_eq!(side.len(), 8);
        for st in &side {
            assert!((st.v_mm - 0.5 * (a + b)).abs() < 1e-9);
            assert!(st.frame(&flat, &fctx).z[2].abs() > 0.95);
        }
        // A fold is where the path turns faster than the ring's own round.
        let path = |bend: bool| -> (Vec<[f64; 3]>, Vec<f64>) {
            let pts: Vec<[f64; 3]> = (0..200).map(|i| {
                let s = i as f64 * 0.05;
                match (bend, s > 5.0) {
                    (true, true) => [5.0, s - 5.0, 0.0],
                    (true, false) => [s, 0.0, 0.0],
                    _ => { let t = s / 9.0; [9.0 * t.sin(), 9.0 * (1.0 - t.cos()), 0.0] }
                }
            }).collect();
            let mut arc = vec![0.0];
            for w in pts.windows(2) { arc.push(arc[arc.len() - 1] + ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()); }
            (pts, arc)
        };
        let (round, arc) = path(false);
        assert!(path_folds(&round, &arc).is_empty());
        let (bent, arc) = path(true);
        let folds = path_folds(&bent, &arc);
        assert!(!folds.is_empty() && folds.iter().all(|f| (f - 5.0).abs() < 0.6), "{folds:?}");
    }

    #[test]
    fn a_mirrored_row_across_the_head_strikes_each_station_once() {
        let d = crest_band();
        let row = |from_deg: f64, to_deg: f64, count: u32, mirror_shoulders: bool, outline: Vec<[f64; 2]>| {
            let stamp = plain("Thorn", 0.0, 0.0, outline, 0.3);
            stamp_row(&d, &StampRow { stamp, path: RowPath::PartingLine, from_deg, to_deg, count, taper: 0.0, fold_clear_mm: 0.0, mirror_shoulders })
        };
        let keel = || crate::outline::keel(1.2, 0.6, 0.12);
        let distinct = |s: &[Stamp]| {
            let names: std::collections::HashSet<&str> = s.iter().map(|st| st.name.as_str()).collect();
            let apart = |a: &Stamp, b: &Stamp| crate::field::wrap_delta(a.theta_deg - b.theta_deg, 360.0).abs() > 1e-6 || (a.v_mm - b.v_mm).abs() > 1e-6;
            names.len() == s.len() && s.iter().enumerate().all(|(i, a)| s[..i].iter().all(|b| apart(a, b)))
        };
        // Across the head every reflection lands on the row itself, and is left out.
        let across = row(60.0, 120.0, 5, true, keel());
        let names: Vec<&str> = across.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Thorn, left 2", "Thorn, left 1", "Thorn, head", "Thorn, right 1", "Thorn, right 2"]);
        let mut struck = d.clone();
        struck.stamps = across;
        let built = crate::mesh::try_build(&struck, &crate::AlphaLibrary::builtin(), strike()).unwrap();
        assert_eq!((built.solids.stamped, built.report.quality.degenerate_faces), (5, 0), "{:?}", built.solids.notes);
        // From the head outward: the head struck once, each side numbered on its own, right k mirrored as left k.
        let half = row(90.0, 118.0, 6, true, keel());
        assert!(half.len() == 11 && distinct(&half), "{:?}", half.iter().map(|s| (&s.name, s.theta_deg)).collect::<Vec<_>>());
        assert_eq!(half[0].name, "Thorn, head");
        for k in 1..=5 {
            let (r, l) = (&half[k], &half[5 + k]);
            assert_eq!([r.name.clone(), l.name.clone()], [format!("Thorn, right {k}"), format!("Thorn, left {k}")]);
            assert!((l.theta_deg - (180.0 - r.theta_deg)).abs() < 1e-9);
        }
        // A stamp reaching over the head would meet its own reflection, so it is struck once.
        let wide = row(92.0, 150.0, 5, true, crate::outline::circle(2.0));
        assert!(wide.len() == 9 && distinct(&wide) && wide.iter().all(|s| (s.theta_deg - 88.0).abs() > 1e-6));
        // A span crossing the head unevenly strikes no reflection onto another stamp of the row.
        let ctx = d.field_context();
        let surface = BareSurface::new(&d, &ctx);
        for (from, to, count) in [(60.0, 104.0, 4), (86.0, 130.0, 5)] {
            let uneven = row(from, to, count, true, keel());
            let frames: Vec<csg::Frame> = uneven.iter().map(|s| s.frame_on(&surface)).collect();
            let meeting: Vec<(&str, &str)> = (0..uneven.len()).flat_map(|i| (0..i).map(move |j| (i, j))).filter(|(i, j)| plans_meet(&uneven[*i], &frames[*i], &uneven[*j], &frames[*j])).map(|(i, j)| (uneven[i].name.as_str(), uneven[j].name.as_str())).collect();
            assert!(meeting.is_empty() && distinct(&uneven), "{from}..{to} x{count}: {meeting:?}");
        }
        // A full turn ends where it starts, and strikes that station once.
        let turn = row(0.0, 360.0, 13, false, keel());
        assert!(turn.len() == 12 && distinct(&turn));
        assert_eq!(turn[11].name, "Thorn, 12");
    }

    #[test]
    fn a_concave_outline_casts_as_its_hull_and_the_bench_cuts_its_bays() {
        let holly = crate::outline::leaf(crate::outline::Margin::Holly { spines: 3, depth_mm: 0.5, lean_deg: 20.0 }, 6.0, 3.6);
        let (hull, bays) = hull_and_bays(&holly, 0.2);
        assert_eq!(bays.len(), 6, "three bays a side between the spines and the tip");
        assert!(crate::outline::check(&hull).is_ok());
        let area = crate::outline::area;
        for bay in &bays {
            assert!(area(bay) > 0.0 && crate::outline::self_crossing(bay).is_none());
        }
        // Hull area less the bays' area inside it is the leaf's.
        let over: f64 = bays.iter().map(|b| {
            let (p, q) = (b[b.len() - 2], b[b.len() - 1]);
            area(b) - 0.2 * (p[0] - q[0]).hypot(p[1] - q[1])
        }).sum();
        assert!((area(&hull) - over - area(&holly)).abs() < 0.03, "{} - {over} against {}", area(&hull), area(&holly));
        let (_, crescent) = hull_and_bays(&moon_outline(1.6, 0.25, 0.85), 0.2);
        assert_eq!(crescent.len(), 1);
        let mut d = crest_band();
        d.profile.width_mm = 7.5;
        let v = crest_v(&d, 90.0).unwrap();
        let lib = crate::AlphaLibrary::builtin();
        let bare = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        d.stamps = plain("Holly", 90.0, v, holly.clone(), 0.4).cast_as_hull(0.2);
        assert_eq!(d.stamps.len(), 7);
        assert!(d.stamps[1..].iter().all(|s| s.cut && s.bench && s.name.starts_with("Holly: bay")));
        assert_eq!(d.stamps[0].parting_monotone(&d), Ok(()));
        let finished = crate::mesh::try_build(&d, &lib, strike()).unwrap();
        assert!(finished.solids.notes.is_empty(), "{:?}", finished.solids.notes);
        assert_eq!(finished.solids.stamped, 7);
        assert!(finished.report.validation.watertight, "{:?}", finished.report.validation);
        assert_eq!(finished.report.quality.degenerate_faces, 0);
        let pattern = crate::mesh::try_build_pattern(&d, &lib, strike()).unwrap();
        assert!(pattern.report.validation.watertight && pattern.solids.stamped == 1);
        let (cast, cut) = (pattern.report.volume_mm3 - bare.report.volume_mm3, finished.report.volume_mm3 - bare.report.volume_mm3);
        let (want_cast, want_cut) = (area(&hull) * 0.4, area(&holly) * 0.4);
        assert!((cast - want_cast).abs() < 0.12 * want_cast, "the blank: {cast:.3} against {want_cast:.3}");
        assert!((cut - want_cut).abs() < 0.15 * want_cut, "the leaf the bench leaves: {cut:.3} against {want_cut:.3}");
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

    #[test]
    fn the_wall_over_the_finger_is_the_floor_of_every_claws_reach() {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let wire = prong_wire_mm(gem);
        let keep = crate::mesh::MIN_WALL_MM;
        let low = |s: &Solid| s.v.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        // The finger hole 5.5 mm under the girdle, flat: a point keeps its height over that of metal.
        let hole = |p: P3| p[2] + 5.5;
        // Metal 4.2 mm down: the claws sink their feet half a millimetre into it and keep the wall.
        let shallow = |_: [f64; 2]| Some(-4.2);
        let (_, met) = claw_parts_reach(gem, 4, wire, Rails::Seat, Some(&shallow), Some(&hole));
        assert_eq!(met, Reach { reached: 4, walled: 0 });
        let head = claw_head_within(gem, 4, wire, Rails::Seat, Some(&shallow), &hole).unwrap();
        let bottom = low(&head.solid);
        assert!(bottom >= -5.5 + keep && bottom < -4.2 - FOOT_SINK_MM, "{bottom}");
        // Metal 4.6 mm down: a foot half a millimetre into it would leave under the wall, so the wall stops every claw first.
        let deep = |_: [f64; 2]| Some(-4.6);
        let (_, met) = claw_parts_reach(gem, 4, wire, Rails::Seat, Some(&deep), Some(&hole));
        assert_eq!(met, Reach { reached: 0, walled: 4 });
        assert_eq!(claw_head_within(gem, 4, wire, Rails::Seat, Some(&deep), &hole).unwrap_err(), HeadSnag::NoReach);
        // Floored by nothing, the same claws went down past the wall.
        let through = low(&claw_head_named(gem, 4, wire, Rails::Seat, Some(&deep)).unwrap().solid);
        assert!(through < -5.5 + keep - 0.2, "{through}");
        // A hole under the head's own base rail: the claws shorten, and the rail refuses the head by name.
        let near = |p: P3| p[2] + 3.6;
        let Err(HeadSnag::Breaks { part, wall_mm }) = claw_head_within(gem, 4, wire, Rails::Seat, None, &near) else { panic!("the base rail stands in the wall") };
        assert_eq!(part, "Base rail");
        assert!(wall_mm < keep && wall_mm > 0.0, "{wall_mm}");
        eprintln!("claws over metal 4.2 mm down end at {bottom:.3}; over 4.6 mm, unfloored, at {through:.3}; the base rail leaves {wall_mm:.3} mm");
        assert!(HeadSnag::Breaks { part, wall_mm: -0.25 }.to_string().starts_with("Base rail would reach 0.25 mm into the finger hole"));
    }
}
