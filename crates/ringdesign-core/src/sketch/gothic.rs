//! Gothic tracery drawn exactly as closed loops of lines and arcs: lights radiating from a centre, the pointed-arch section, and the lancet arcade.
use super::{Geometry, Id, Sketch, Workplane};
use anyhow::{Result, bail, ensure};
use std::f64::consts::{PI, TAU};

type V = [f64; 2];
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn scale(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: V) -> f64 {
    a[0].hypot(a[1])
}
fn unit(a: V) -> V {
    scale(a, 1.0 / len(a))
}
fn turn(a: V, angle: f64) -> V {
    let (s, c) = angle.sin_cos();
    [a[0] * c - a[1] * s, a[0] * s + a[1] * c]
}
fn bearing(c: V, p: V) -> f64 {
    (p[1] - c[1]).atan2(p[0] - c[0])
}
fn polar(c: V, r: f64, angle: f64) -> V {
    [c[0] + r * angle.cos(), c[1] + r * angle.sin()]
}
fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Most lights one sketch radiates.
pub const MAX_LIGHTS: u32 = 120;
/// Most bays one arcade carries.
pub const MAX_BAYS: u32 = 64;
/// Lower foils' centres as a share of their radius off the light's axis; under 1, so neighbouring foils cross in a cusp.
const FOIL_SPREAD: f64 = 0.6;

/// How a light's head closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    /// Two arcs struck from the far side, tangent to the jambs: equilateral where the light is tall enough, a drop arch where it is not.
    Pointed,
    /// One arc tangent to both jambs.
    Round,
    /// Three foils, two tangent to the jambs and one under the apex, meeting in cusps.
    Trefoil,
    /// The whole light a curved dagger: a round head against the apex, a pointed tail at the sill.
    Mouchette,
}

impl Head {
    pub const ALL: [Head; 4] = [Head::Pointed, Head::Round, Head::Trefoil, Head::Mouchette];
    pub fn label(self) -> &'static str {
        match self {
            Head::Pointed => "Pointed",
            Head::Round => "Round",
            Head::Trefoil => "Trefoil",
            Head::Mouchette => "Mouchette",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.label().eq_ignore_ascii_case(name.trim()))
    }
}

/// Lights radiating from a centre between two radii, one to a cell of the net their mullions divide.
#[derive(Clone, Debug, PartialEq)]
pub struct Lights {
    pub count: u32,
    pub centre: V,
    /// The first light's axis, degrees counter-clockwise from the sketch's x.
    pub phase_deg: f64,
    /// Radius of each light's sill.
    pub sill_r_mm: f64,
    /// Radius of each light's apex.
    pub apex_r_mm: f64,
    /// Metal between neighbouring lights.
    pub bar_mm: f64,
    /// Zero fills each cell with jambs parallel to its mullions; above zero, every light is parallel-sided at this width.
    pub width_mm: f64,
    pub head: Head,
    /// Mouchettes lean clockwise instead of counter-clockwise.
    pub clockwise: bool,
}

impl Default for Lights {
    fn default() -> Self {
        Self { count: 24, centre: [0.0, 0.0], phase_deg: 90.0, sill_r_mm: 10.3, apex_r_mm: 12.05, bar_mm: 0.9, width_mm: 0.0, head: Head::Pointed, clockwise: false }
    }
}

/// A ring's cross-section as a blunt lancet standing on the bore, on [`Workplane::section`]: x out from the finger's axis, y along it.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchSection {
    /// The bore's radius at its tightest, on the parting plane.
    pub bore_r_mm: f64,
    pub width_mm: f64,
    /// From the bore's tightest point to the apex.
    pub thickness_mm: f64,
    /// 1 springs the flanks from the bore corners; 0 stands the feet straight until the head is a round arch.
    pub keel: f64,
    /// Radius rounding each bore corner; 0 leaves it sharp.
    pub fillet_mm: f64,
    /// How much wider the bore is at the band's edges than at its middle.
    pub comfort_mm: f64,
}

impl Default for ArchSection {
    fn default() -> Self {
        Self { bore_r_mm: 9.3, width_mm: 5.6, thickness_mm: 3.6, keel: 1.0, fillet_mm: 0.3, comfort_mm: 0.12 }
    }
}

/// A row of lancet bays standing on a sill, centred on the origin with x across and y up.
#[derive(Clone, Debug, PartialEq)]
pub struct Arcade {
    pub bays: u32,
    /// Across every bay and the posts between them.
    pub width_mm: f64,
    /// From the sill's foot to the apexes.
    pub height_mm: f64,
    /// Width of each post between two bays.
    pub bar_mm: f64,
    /// Height of the sill the bays stand on.
    pub sill_mm: f64,
    /// Pointed, Round or Trefoil.
    pub head: Head,
}

impl Default for Arcade {
    fn default() -> Self {
        Self { bays: 3, width_mm: 6.0, height_mm: 2.4, bar_mm: 0.6, sill_mm: 0.4, head: Head::Pointed }
    }
}

impl Arcade {
    /// Width of one bay between its jambs.
    pub fn bay_mm(&self) -> f64 {
        (self.width_mm - f64::from(self.bays.saturating_sub(1)) * self.bar_mm) / f64::from(self.bays.max(1))
    }
}

/// A line, or an arc counter-clockwise about `c` from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Piece {
    Line(V, V),
    Arc { c: V, from: V, to: V },
}

impl Piece {
    fn map(self, f: impl Fn(V) -> V) -> Self {
        match self {
            Piece::Line(a, b) => Piece::Line(f(a), f(b)),
            Piece::Arc { c, from, to } => Piece::Arc { c: f(c), from: f(from), to: f(to) },
        }
    }
    /// The piece reflected in the y axis, still counter-clockwise.
    fn mirrored(self) -> Self {
        let m = |p: V| [-p[0], p[1]];
        match self {
            Piece::Line(a, b) => Piece::Line(m(b), m(a)),
            Piece::Arc { c, from, to } => Piece::Arc { c: m(c), from: m(to), to: m(from) },
        }
    }
    fn start(self) -> V {
        match self {
            Piece::Line(a, _) => a,
            Piece::Arc { from, .. } => from,
        }
    }
    fn end(self) -> V {
        match self {
            Piece::Line(_, b) => b,
            Piece::Arc { to, .. } => to,
        }
    }
    fn sweep(c: V, from: V, to: V) -> f64 {
        (bearing(c, to) - bearing(c, from)).rem_euclid(TAU)
    }
    fn length(self) -> f64 {
        match self {
            Piece::Line(a, b) => len(sub(b, a)),
            Piece::Arc { c, from, to } => len(sub(from, c)) * Self::sweep(c, from, to),
        }
    }
    /// Points from the start toward the end, no two further apart than `step`, the end left out.
    fn points(self, step: f64) -> Vec<V> {
        let n = (self.length() / step).ceil().max(1.0) as usize;
        match self {
            Piece::Line(a, b) => (0..n).map(|k| add(a, scale(sub(b, a), k as f64 / n as f64))).collect(),
            Piece::Arc { c, from, to } => {
                let (r, a0, sweep) = (len(sub(from, c)), bearing(c, from), Self::sweep(c, from, to));
                std::iter::once(from).chain((1..n).map(|k| polar(c, r, a0 + sweep * k as f64 / n as f64))).collect()
            }
        }
    }
    /// The nearest distance from `p` to the piece.
    fn distance(self, p: V) -> f64 {
        match self {
            Piece::Line(a, b) => {
                let d = sub(b, a);
                let t = (dot(sub(p, a), d) / dot(d, d).max(1e-300)).clamp(0.0, 1.0);
                len(sub(p, add(a, scale(d, t))))
            }
            Piece::Arc { c, from, to } => {
                let r = len(sub(from, c));
                let within = Self::sweep(c, from, p) <= Self::sweep(c, from, to);
                if within && len(sub(p, c)) > 0.0 { (len(sub(p, c)) - r).abs() } else { len(sub(p, from)).min(len(sub(p, to))) }
            }
        }
    }
}

/// A jamb line `p(s) = s·m + c·n`, `m` up along it and `n` its unit normal into the light.
#[derive(Clone, Copy, Debug)]
struct Jamb {
    m: V,
    n: V,
    c: f64,
}

impl Jamb {
    fn at(self, s: f64) -> V {
        add(scale(self.m, s), scale(self.n, self.c))
    }
    /// Half the light's width at `s`: the jamb's distance from the light's axis.
    fn half_width(self, s: f64) -> f64 {
        s * self.m[0] + self.c * self.n[0]
    }
    /// The parameter where the jamb meets the circle of radius `r` about the origin.
    fn on_circle(self, r: f64) -> Option<f64> {
        let q = r * r - self.c * self.c;
        (q > 0.0).then(|| q.sqrt())
    }
    /// The parameter at height `y`.
    fn at_height(self, y: f64) -> f64 {
        (y - self.c * self.n[1]) / self.m[1]
    }
    /// The mirror jamb across the light's axis.
    fn left(self) -> Self {
        Self { m: [-self.m[0], self.m[1]], n: [-self.n[0], self.n[1]], c: self.c }
    }
    /// Signed distance of `p` inside the jamb.
    fn inside(self, p: V) -> f64 {
        dot(p, self.n) - self.c
    }
}

/// Acuity of a round head between these jambs: its arcs share one centre on the axis.
fn round_acuity(j: Jamb) -> f64 {
    (-1.0 / j.n[0] - 1.0).max(0.0)
}

/// Where the arc tangent to the jamb at `s`, of radius `(1 + k)` half-widths, meets the light's axis.
fn head_height(j: Jamb, s: f64, k: f64) -> f64 {
    let q = ((1.0 + k).powi(2) - (1.0 + (1.0 + k) * j.n[0]).powi(2)).max(0.0).sqrt();
    let hw = j.half_width(s);
    j.at(s)[1] + hw * ((1.0 + k) * j.n[1] + q)
}

/// The right half of a pointed or round head from the sill corner `s_sill` up to the apex at height `apex`.
fn arched_half(j: Jamb, s_sill: f64, apex: f64, head: Head) -> Result<Vec<Piece>> {
    let k_round = round_acuity(j);
    let want = if head == Head::Round { k_round } else { k_round.max(1.0) };
    // Springing for the wanted acuity, from `head_height` being linear in `s`.
    let q = ((1.0 + want).powi(2) - (1.0 + (1.0 + want) * j.n[0]).powi(2)).max(0.0).sqrt();
    let kk = (1.0 + want) * j.n[1] + q;
    let mut s = (apex - j.c * (j.n[1] + j.n[0] * kk)) / (j.m[1] + j.m[0] * kk);
    let mut k = want;
    if s < s_sill {
        let short = head_height(j, s_sill, k_round);
        if head == Head::Round || short > apex + 1e-12 {
            bail!(
                "A light {:.3} mm wide at its sill needs {:.3} mm to its apex for a {} head; it has {:.3}",
                2.0 * j.half_width(s_sill),
                short - j.at(s_sill)[1],
                head.label().to_lowercase(),
                apex - j.at(s_sill)[1]
            );
        }
        // A drop arch: the acuity whose arc from the sill corner reaches the apex.
        let (mut lo, mut hi) = (k_round, want);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if head_height(j, s_sill, mid) > apex { hi = mid } else { lo = mid }
        }
        (s, k) = (s_sill, 0.5 * (lo + hi));
    }
    let p = j.at(s);
    let rho = (1.0 + k) * j.half_width(s);
    let centre = add(p, scale(j.n, rho));
    let a = [0.0, apex];
    ensure!((len(sub(a, centre)) - rho).abs() < 1e-7 * rho.max(1.0), "A head's arc misses its apex");
    let mut half = Vec::new();
    if s > s_sill {
        half.push(Piece::Line(j.at(s_sill), p));
    }
    half.push(Piece::Arc { c: centre, from: p, to: a });
    Ok(half)
}

/// The right half of a trefoil head from the sill corner `s_sill` up to the apex at height `apex`.
fn trefoil_half(j: Jamb, s_sill: f64, apex: f64) -> Result<Vec<Piece>> {
    let (d, e) = (FOIL_SPREAD - j.n[0], j.n[1] + 3f64.sqrt() * FOIL_SPREAD + 1.0);
    let (a1, a0) = (j.m[0], j.c * j.n[0]);
    let s = (apex - j.c * j.n[1] - a0 * e / d) / (j.m[1] + a1 * e / d);
    let rho = j.half_width(s) / d;
    ensure!(
        s > s_sill && rho > 0.0,
        "A light {:.3} mm wide at its sill needs a longer reach to its apex for a trefoil head; try a Pointed head",
        2.0 * j.half_width(s_sill)
    );
    let q = j.at(s);
    let foil = add(q, scale(j.n, rho));
    let top = [0.0, foil[1] + 3f64.sqrt() * foil[0]];
    // The cusp is where the two foils cross away from the centroid of the three centres.
    let mid = scale(add(foil, top), 0.5);
    let along = unit(sub(top, foil));
    let across = [-along[1], along[0]];
    let h = (rho * rho - (0.5 * len(sub(top, foil))).powi(2)).max(0.0).sqrt();
    let centroid = [0.0, (2.0 * foil[1] + top[1]) / 3.0];
    let (k1, k2) = (add(mid, scale(across, h)), sub(mid, scale(across, h)));
    let cusp = if len(sub(k1, centroid)) > len(sub(k2, centroid)) { k1 } else { k2 };
    Ok(vec![
        Piece::Line(j.at(s_sill), q),
        Piece::Arc { c: foil, from: q, to: cusp },
        Piece::Arc { c: top, from: cusp, to: [0.0, apex] },
    ])
}

/// A head's right half up to the apex, then the left half down to the sill: the walk round one light less its sill.
fn closed_head(j: Jamb, s_sill: f64, apex: f64, head: Head) -> Result<Vec<Piece>> {
    let half = match head {
        Head::Pointed | Head::Round => arched_half(j, s_sill, apex, head)?,
        Head::Trefoil => trefoil_half(j, s_sill, apex)?,
        Head::Mouchette => bail!("A mouchette is a whole light, not a head on jambs"),
    };
    let mut walk = half.clone();
    let left: Vec<Piece> = half.iter().rev().map(|p| p.mirrored()).collect();
    // A half turn about one centre on the axis is drawn as one arc.
    if let (Some(&Piece::Arc { c, from, to: apex_point }), Some(&Piece::Arc { c: c2, to, .. })) = (walk.last(), left.first()) {
        if c == [0.0, c[1]] && len(sub(c, c2)) == 0.0 && Piece::sweep(c, from, apex_point) * 2.0 <= PI {
            walk.pop();
            walk.push(Piece::Arc { c, from, to });
            walk.extend(left.into_iter().skip(1));
            return Ok(walk);
        }
    }
    walk.extend(left);
    Ok(walk)
}

/// Whether `p` lies in the cell between the jambs and the two radii, to `tol`.
fn in_cell(j: Jamb, p: V, sill: f64, apex: f64, tol: f64) -> bool {
    let r = len(p);
    j.inside(p) >= -tol && j.left().inside(p) >= -tol && r >= sill - tol && r <= apex + tol
}

/// A curved dagger filling the cell, its head against the apex on the left jamb and its tail on the sill by the right.
fn mouchette(j: Jamb, sill: f64, apex: f64) -> Result<Vec<Piece>> {
    let l = j.left();
    let mid = j.half_width(j.at_height(0.5 * (sill + apex)));
    let (Some(s_low), Some(s_high)) = (j.on_circle(sill), l.on_circle(apex)) else {
        bail!("A mouchette's cell is narrower than its bar");
    };
    let diagonal = len(sub(l.at(s_high), j.at(s_low)));
    let head0 = (0.36 * (2.0 * mid).min(apex - sill)).min(diagonal / 6.0);
    for f in [1.0, 0.85, 0.7, 0.55, 0.4] {
        let rh = f * head0;
        // The head touches the apex circle and the left jamb; the tail sits on the sill clear of the right jamb.
        let tail_c = j.c + 0.6 * rh;
        let (hq, tq) = ((apex - rh).powi(2) - (l.c + rh).powi(2), sill * sill - tail_c * tail_c);
        if hq <= 0.0 || tq <= 0.0 {
            continue;
        }
        let head = add(scale(l.m, hq.sqrt()), scale(l.n, l.c + rh));
        let tail = add(scale(j.m, tq.sqrt()), scale(j.n, tail_c));
        let span = len(sub(head, tail));
        let u = unit(sub(head, tail));
        let v = sub(tail, head);
        for deg in [34.0f64, 28.0, 22.0, 16.0, 11.0] {
            let th = deg.to_radians();
            if th.sin() < 1.15 * rh / span {
                continue;
            }
            let d1 = turn(u, -th);
            let n1 = [-d1[1], d1[0]];
            let rho1 = (rh * rh - span * span) / (2.0 * (dot(v, n1) + rh));
            let o1 = add(tail, scale(n1, rho1));
            let j1 = add(head, scale(unit(sub(head, o1)), rh));
            let d2 = turn(u, -0.35 * th);
            let n2 = [-d2[1], d2[0]];
            let rho2 = (span * span - rh * rh) / (2.0 * (rh - dot(v, n2)));
            let o2 = add(tail, scale(n2, rho2));
            let j2 = add(o2, scale(unit(sub(head, o2)), rho2));
            if !(finite(&[rho1, rho2]) && rho1 > rh && rho2 > 0.0) {
                continue;
            }
            let pieces = vec![
                Piece::Arc { c: o1, from: tail, to: j1 },
                Piece::Arc { c: head, from: j1, to: j2 },
                Piece::Arc { c: o2, from: tail, to: j2 },
            ];
            if Piece::sweep(head, j1, j2) <= PI || Piece::sweep(o1, tail, j1) >= PI || Piece::sweep(o2, tail, j2) >= PI {
                continue;
            }
            let step = (apex - sill).min(2.0 * mid).max(1e-3) / 400.0;
            if pieces.iter().all(|p| p.points(step).into_iter().chain([p.end()]).all(|q| in_cell(j, q, sill, apex, 1e-9))) {
                return Ok(pieces);
            }
        }
    }
    bail!("No mouchette fits a cell {:.3} mm wide and {:.3} mm deep; widen the net or draw fewer lights", 2.0 * mid, apex - sill)
}

/// Draws closed loops into a sketch, sharing every end and every centre by position.
struct Drawing {
    sketch: Sketch,
    ends: Vec<(V, Id)>,
    centres: Vec<(V, Id)>,
}

impl Drawing {
    fn new(name: &str, plane: Workplane) -> Self {
        let mut sketch = Sketch::default();
        sketch.name = name.into();
        sketch.plane = plane;
        Self { sketch, ends: Vec::new(), centres: Vec::new() }
    }
    fn shared(sketch: &mut Sketch, pool: &mut Vec<(V, Id)>, p: V) -> Id {
        if let Some((_, id)) = pool.iter().find(|(q, _)| len(sub(*q, p)) <= 1e-9) {
            return *id;
        }
        let id = sketch.point(p);
        pool.push((p, id));
        id
    }
    fn draw(&mut self, piece: Piece) {
        let geometry = match piece {
            Piece::Line(a, b) => {
                let a = Self::shared(&mut self.sketch, &mut self.ends, a);
                let b = Self::shared(&mut self.sketch, &mut self.ends, b);
                Geometry::Line { a, b }
            }
            Piece::Arc { c, from, to } => {
                let start = Self::shared(&mut self.sketch, &mut self.ends, from);
                let end = Self::shared(&mut self.sketch, &mut self.ends, to);
                let center = Self::shared(&mut self.sketch, &mut self.centres, c);
                Geometry::Arc { center, start, end }
            }
        };
        self.sketch.entity(geometry);
    }
    fn finish(self, loops: usize) -> Result<Sketch> {
        self.sketch.validate()?;
        let regions = self.sketch.sweep_regions()?;
        ensure!(regions.len() == loops, "The tracery drew {} regions where it meant {loops}", regions.len());
        Ok(self.sketch)
    }
}

/// The nearest metal between two loops, sampled finely along the first and measured exactly to the second.
fn gap(a: &[Piece], b: &[Piece]) -> f64 {
    let size = a.iter().chain(b).map(|p| p.length()).fold(0.0, f64::max).max(1e-3);
    let step = size / 2000.0;
    let one = |a: &[Piece], b: &[Piece]| {
        a.iter()
            .flat_map(|p| p.points(step))
            .map(|q| b.iter().map(|p| p.distance(q)).fold(f64::INFINITY, f64::min))
            .fold(f64::INFINITY, f64::min)
    };
    one(a, b).min(one(b, a))
}

impl Lights {
    fn check(&self) -> Result<()> {
        ensure!(
            finite(&[self.centre[0], self.centre[1], self.phase_deg, self.sill_r_mm, self.apex_r_mm, self.bar_mm, self.width_mm]),
            "Tracery needs finite sizes"
        );
        ensure!((1..=MAX_LIGHTS).contains(&self.count), "Tracery radiates 1 to {MAX_LIGHTS} lights, not {}", self.count);
        ensure!(self.width_mm > 0.0 || self.count >= 3, "Lights filling their cells need at least three cells; give them a width for fewer");
        ensure!(self.sill_r_mm > 0.0 && self.apex_r_mm > self.sill_r_mm, "A light's apex stands further out than its sill");
        ensure!(self.bar_mm > 0.0, "Lights need a bar of metal between them");
        ensure!(self.width_mm >= 0.0, "A light's width is zero (fill the cell) or above");
        Ok(())
    }
    /// The jamb on the right of a light whose axis is the local y.
    fn jamb(&self) -> Jamb {
        if self.width_mm > 0.0 {
            Jamb { m: [0.0, 1.0], n: [-1.0, 0.0], c: -0.5 * self.width_mm }
        } else {
            let beta = PI / f64::from(self.count);
            Jamb { m: [beta.sin(), beta.cos()], n: [-beta.cos(), beta.sin()], c: 0.5 * self.bar_mm }
        }
    }
    /// One light about the local y axis, as the loop it walks.
    fn light(&self) -> Result<Vec<Piece>> {
        let j = self.jamb();
        let (sill, apex) = (self.sill_r_mm, self.apex_r_mm);
        let Some(s_sill) = j.on_circle(sill).filter(|s| j.half_width(*s) > 1e-6) else {
            bail!("At {sill:.3} mm a {}-light net leaves no room for a light beside its bar", self.count);
        };
        if self.head == Head::Mouchette {
            let pieces = mouchette(j, sill, apex)?;
            return Ok(if self.clockwise { pieces.into_iter().map(Piece::mirrored).collect() } else { pieces });
        }
        let mut walk = closed_head(j, s_sill, apex, self.head)?;
        let (right, left) = (j.at(s_sill), j.left().at(s_sill));
        walk.push(Piece::Arc { c: [0.0, 0.0], from: right, to: left });
        Ok(walk)
    }
    /// Every light's loop, turned to its axis and moved to the centre.
    fn loops(&self) -> Result<Vec<Vec<Piece>>> {
        self.check()?;
        let one = self.light()?;
        Ok((0..self.count)
            .map(|i| {
                let angle = (self.phase_deg - 90.0).to_radians() + TAU * f64::from(i) / f64::from(self.count);
                one.iter().map(|p| p.map(|q| add(turn(q, angle), self.centre))).collect()
            })
            .collect())
    }
    /// The narrowest metal between two neighbouring lights; infinite for one light.
    pub fn land_mm(&self) -> Result<f64> {
        let loops = self.loops()?;
        Ok(if loops.len() < 2 { f64::INFINITY } else { gap(&loops[0], &loops[1]) })
    }
}

/// The lights as one sketch on the default workplane, a closed loop each.
pub fn lights(l: &Lights) -> Result<Sketch> {
    let loops = l.loops()?;
    if loops.len() >= 2 {
        let land = gap(&loops[0], &loops[1]);
        ensure!(
            land >= l.bar_mm - 1e-6,
            "These lights leave {land:.3} mm between neighbours, under the {:.3} mm bar; draw fewer, narrower or further out",
            l.bar_mm
        );
    }
    let mut d = Drawing::new("Radiating lights", Workplane::default());
    for walk in &loops {
        for p in walk {
            d.draw(*p);
        }
    }
    d.finish(loops.len())
}

impl ArchSection {
    fn check(&self) -> Result<()> {
        ensure!(finite(&[self.bore_r_mm, self.width_mm, self.thickness_mm, self.keel, self.fillet_mm, self.comfort_mm]), "An arch section needs finite sizes");
        ensure!(self.bore_r_mm > 0.0 && self.width_mm > 0.0, "An arch section needs a bore and a width");
        ensure!((0.0..=1.0).contains(&self.keel), "Keel runs from 0 (round) to 1 (springing at the bore)");
        ensure!(self.fillet_mm >= 0.0 && self.comfort_mm >= 0.0, "Fillet and comfort are zero or above");
        ensure!(
            self.thickness_mm - self.comfort_mm >= 0.5 * self.width_mm - 1e-9,
            "A pointed section {:.3} mm wide needs at least {:.3} mm over its comfort; it has {:.3}",
            self.width_mm,
            0.5 * self.width_mm,
            self.thickness_mm - self.comfort_mm
        );
        Ok(())
    }
    /// The lower half's walk from the bore's mid-plane to the apex, and the bore's own piece below the mid-plane.
    fn lower(&self) -> Result<Vec<Piece>> {
        let h = 0.5 * self.width_mm;
        let (rb, c, rf) = (self.bore_r_mm, self.comfort_mm, self.fillet_mm);
        let xs = rb + c;
        let rise = self.thickness_mm - c;
        let feet = (1.0 - self.keel) * (rise - h);
        let xspr = xs + feet;
        let arch = rise - feet;
        let cy = (arch * arch - h * h) / (2.0 * h);
        let flank = [xspr, cy];
        let rho = h + cy;
        let apex = [rb + self.thickness_mm, 0.0];
        let spring = [xspr, -h];
        // The bore as a circle of radius `bore` about `bc`, or the line x = rb.
        let bore = (c > 0.0).then(|| (h * h + c * c) / (2.0 * c));
        let bc = [rb + bore.unwrap_or(0.0), 0.0];
        let on_bore = |f: V| match bore {
            Some(r) => add(bc, scale(unit(sub(f, bc)), r)),
            None => [rb, f[1]],
        };
        let corner = [xs, -h];
        let mut walk = Vec::new();
        let bore_piece = |to: V| match bore {
            Some(_) => Piece::Arc { c: bc, from: [rb, 0.0], to },
            None => Piece::Line([rb, 0.0], to),
        };
        if rf <= 0.0 {
            walk.push(bore_piece(corner));
            if feet > 0.0 {
                walk.push(Piece::Line(corner, spring));
            }
            walk.push(Piece::Arc { c: flank, from: spring, to: apex });
            return Ok(walk);
        }
        ensure!(rf < h, "A {rf:.3} mm fillet is wider than the half section");
        // Against the straight foot when it is long enough, else against the flank.
        let on_foot = match bore {
            Some(r) => {
                let q = (r - rf).powi(2) - (h - rf).powi(2);
                (q > 0.0).then(|| [bc[0] - q.sqrt(), -h + rf])
            }
            None => Some([rb + rf, -h + rf]),
        };
        if let Some(f) = on_foot.filter(|f| f[0] <= xspr) {
            let (tb, tf) = (on_bore(f), [f[0], -h]);
            walk.push(bore_piece(tb));
            walk.push(Piece::Arc { c: f, from: tb, to: tf });
            if xspr - tf[0] > 1e-9 {
                walk.push(Piece::Line(tf, spring));
            }
            walk.push(Piece::Arc { c: flank, from: spring, to: apex });
            return Ok(walk);
        }
        // The fillet's centre: inside the flank by its radius, and inside the bore by its radius.
        let ri = rho - rf;
        let f = match bore {
            Some(r) => {
                let ro = r - rf;
                let d = len(sub(bc, flank));
                ensure!(d > 1e-12 && d <= ri + ro && d >= (ri - ro).abs(), "The bore fillet finds no room between the bore and the flank");
                let a = (ri * ri - ro * ro + d * d) / (2.0 * d);
                let hh = (ri * ri - a * a).max(0.0).sqrt();
                let along = unit(sub(bc, flank));
                let base = add(flank, scale(along, a));
                let across = [-along[1], along[0]];
                let (p, q) = (add(base, scale(across, hh)), sub(base, scale(across, hh)));
                if p[1] < q[1] { p } else { q }
            }
            None => {
                let x = rb + rf;
                let dy = (ri * ri - (x - flank[0]).powi(2)).max(0.0).sqrt();
                [x, flank[1] - dy]
            }
        };
        let (tb, tf) = (on_bore(f), add(flank, scale(unit(sub(f, flank)), rho)));
        ensure!(bearing(flank, tf) > bearing(flank, spring) - 1e-12, "The bore fillet would round past the flank's springing");
        walk.push(bore_piece(tb));
        walk.push(Piece::Arc { c: f, from: tb, to: tf });
        walk.push(Piece::Arc { c: flank, from: tf, to: apex });
        Ok(walk)
    }
}

/// Most points the section's one polyline carries, under the sketch's 512.
const SECTION_POINTS: f64 = 480.0;

/// The section as one closed polyline of short chords on [`Workplane::section`]: comfort bore, filleted corners, straight feet, keeled flanks.
pub fn arch_section(a: &ArchSection) -> Result<Sketch> {
    a.check()?;
    let lower = a.lower()?;
    let length: f64 = lower.iter().map(|p| p.length()).sum();
    let step = (2.0 * length / SECTION_POINTS).max(0.03);
    // The lower half from the bore's mid-plane to the apex, then its mirror back.
    let mut points: Vec<V> = lower.iter().filter(|p| p.length() > 1e-12).flat_map(|p| p.points(step)).collect();
    let apex = lower[lower.len() - 1].end();
    let back: Vec<V> = points[1..].iter().rev().map(|p| [p[0], -p[1]]).collect();
    points.push(apex);
    points.extend(back);
    let mut sketch = Sketch::default();
    sketch.name = "Pointed-arch section".into();
    sketch.plane = Workplane::section();
    let ids = points.into_iter().map(|p| sketch.point(p)).collect();
    sketch.entity(Geometry::Polyline { points: ids, closed: true });
    let d = Drawing { sketch, ends: Vec::new(), centres: Vec::new() };
    d.finish(1)
}

impl Arcade {
    fn check(&self) -> Result<()> {
        ensure!(finite(&[self.width_mm, self.height_mm, self.bar_mm, self.sill_mm]), "An arcade needs finite sizes");
        ensure!((1..=MAX_BAYS).contains(&self.bays), "An arcade has 1 to {MAX_BAYS} bays, not {}", self.bays);
        ensure!(self.head != Head::Mouchette, "An arcade's bays take a Pointed, Round or Trefoil head");
        ensure!(self.bar_mm > 0.0 || self.bays == 1, "Bays need a post between them");
        ensure!(self.sill_mm >= 0.0 && self.height_mm > self.sill_mm, "An arcade stands taller than its sill");
        ensure!(self.bay_mm() > 0.0, "{} bays and their posts are wider than {:.3} mm", self.bays, self.width_mm);
        Ok(())
    }
    /// Each bay's walk up its right jamb, over its head and down its left, in the arcade's own frame.
    fn bays(&self) -> Result<Vec<Vec<Piece>>> {
        self.check()?;
        let b = self.bay_mm();
        let j = Jamb { m: [0.0, 1.0], n: [-1.0, 0.0], c: -0.5 * b };
        let sill = -0.5 * self.height_mm + self.sill_mm;
        let walk = closed_head(j, sill, 0.5 * self.height_mm, self.head)?;
        Ok((0..self.bays)
            .map(|i| {
                let x = -0.5 * self.width_mm + 0.5 * b + f64::from(i) * (b + self.bar_mm);
                walk.iter().map(|p| p.map(|q| [q[0] + x, q[1]])).collect()
            })
            .collect())
    }
}

/// The arcade's bays as one sketch, a closed loop each, turned `turn_deg` about the origin and moved to `at`.
pub fn arcade(a: &Arcade, at: V, turn_deg: f64) -> Result<Sketch> {
    ensure!(finite(&[at[0], at[1], turn_deg]), "An arcade needs a finite place");
    let bays = a.bays()?;
    let angle = turn_deg.to_radians();
    let mut d = Drawing::new("Lancet arcade", Workplane::default());
    for walk in &bays {
        for p in walk {
            d.draw(p.map(|q| add(turn(q, angle), at)));
        }
        let (right, left) = (walk[0].start(), walk[walk.len() - 1].end());
        d.draw(Piece::Line(add(turn(left, angle), at), add(turn(right, angle), at)));
    }
    d.finish(bays.len())
}

/// The bays standing on their sill as one counter-clockwise stamp outline, centred on the origin, no edge over `step`.
pub fn arcade_outline(a: &Arcade, step: f64) -> Result<Vec<V>> {
    ensure!(step.is_finite() && step > 0.0, "An outline needs a step above zero");
    ensure!(a.sill_mm > 0.0 || a.bays == 1, "An arcade's outline stands its bays on a sill; give the sill a height");
    let bays = a.bays()?;
    let (w, h) = (0.5 * a.width_mm, 0.5 * a.height_mm);
    let mut walk = vec![Piece::Line([-w, -h], [w, -h])];
    let mut from = [w, -h];
    for bay in bays.iter().rev() {
        let right = bay[0].start();
        if len(sub(from, right)) > 1e-12 {
            walk.push(Piece::Line(from, right));
        }
        walk.extend(bay.iter().copied());
        from = bay[bay.len() - 1].end();
    }
    walk.push(Piece::Line(from, [-w, -h]));
    let mut points: Vec<V> = walk.iter().filter(|p| p.length() > 1e-12).flat_map(|p| p.points(step)).collect();
    points.dedup_by(|a, b| len(sub(*a, *b)) <= 1e-12);
    if points.len() > 1 && len(sub(points[0], points[points.len() - 1])) <= 1e-12 {
        points.pop();
    }
    crate::outline::check(&points).map_err(|e| anyhow::anyhow!("The arcade's outline is no stamp outline: {e}"))?;
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn radii(s: &Sketch, centre: V) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = 0.0f64;
        for e in &s.entities {
            for c in s.curves_of(e).unwrap() {
                for p in c.tessellate_within(1e-4) {
                    let r = len(sub(p, centre));
                    lo = lo.min(r);
                    hi = hi.max(r);
                }
            }
        }
        (lo, hi)
    }

    /// `RD_GOTHIC_SHEET=/dir cargo test -p ringdesign-core gothic_sheet` writes every construction as SVG for an eyeball pass.
    #[test]
    fn gothic_sheet() {
        let Some(dir) = std::env::var_os("RD_GOTHIC_SHEET").map(std::path::PathBuf::from) else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let mut sheets: Vec<(String, Sketch)> = Vec::new();
        for head in Head::ALL {
            for (name, l) in [
                ("wheel", Lights { head, ..Lights::default() }),
                ("rose", Lights { count: 8, sill_r_mm: 4.0, apex_r_mm: 6.7, bar_mm: 0.8, head, ..Lights::default() }),
                ("lantern", Lights { count: 8, sill_r_mm: 4.4, apex_r_mm: 6.8, bar_mm: 0.8, width_mm: 1.1, head, ..Lights::default() }),
                ("flamboyant", Lights { count: 12, sill_r_mm: 2.5, apex_r_mm: 7.0, bar_mm: 0.6, head, ..Lights::default() }),
            ] {
                if let Ok(s) = lights(&l) {
                    sheets.push((format!("{name}-{}", head.label().to_lowercase()), s));
                }
            }
        }
        for keel in [0.0, 0.5, 1.0] {
            sheets.push((format!("arch-keel-{keel}"), arch_section(&ArchSection { keel, ..ArchSection::default() }).unwrap()));
        }
        for head in [Head::Pointed, Head::Round, Head::Trefoil] {
            let a = Arcade { head, ..Arcade::default() };
            sheets.push((format!("arcade-{}", head.label().to_lowercase()), arcade(&a, [0.0, 0.0], 0.0).unwrap()));
            let mut outline = Sketch::default();
            let ids: Vec<Id> = arcade_outline(&a, 0.1).unwrap().into_iter().map(|p| outline.point(p)).collect();
            outline.entity(Geometry::Polyline { points: ids, closed: true });
            sheets.push((format!("arcade-outline-{}", head.label().to_lowercase()), outline));
        }
        for (name, s) in sheets {
            std::fs::write(dir.join(format!("{name}.svg")), super::super::exchange::svg(&s).unwrap()).unwrap();
        }
    }

    /// Pointed and round lights cut with a draft close when tessellated, at preview and export chords; cusped trefoils and mouchettes cut straight.
    #[test]
    fn drafted_tracery_tessellates_closed() {
        let rose = |head| lights(&Lights { count: 8, sill_r_mm: 4.0, apex_r_mm: 6.7, bar_mm: 0.8, head, ..Lights::default() }).unwrap();
        let lantern = Lights { count: 8, sill_r_mm: 4.4, apex_r_mm: 6.8, bar_mm: 0.8, width_mm: 1.1, ..Lights::default() };
        let cases = [
            ("wheel", lights(&Lights::default()).unwrap()),
            ("tall wheel", lights(&Lights { apex_r_mm: 13.0, ..Lights::default() }).unwrap()),
            ("round wheel", lights(&Lights { head: Head::Round, ..Lights::default() }).unwrap()),
            ("pointed rose", rose(Head::Pointed)),
            ("round rose", rose(Head::Round)),
            ("lantern", lights(&lantern).unwrap()),
            ("arcade", arcade(&Arcade::default(), [0.0, 0.0], 0.0).unwrap()),
            ("round arcade", arcade(&Arcade { head: Head::Round, ..Arcade::default() }, [0.0, 0.0], 0.0).unwrap()),
        ];
        for (name, s) in cases {
            let regions = s.sweep_regions().unwrap();
            let plane = s.plane.plane().unwrap();
            for (draft, depth) in [(3.0f64, 0.35), (3.0, 3.55), (5.0, 1.5), (5.0, 2.0)] {
                let body = super::super::solid::extrude(plane, &regions, [0.0, 0.0, -depth], draft.to_radians()).unwrap_or_else(|e| panic!("{name} at {draft} deg: {e}"));
                for chord in [crate::cad::PREVIEW_CHORD_MM, crate::cad::EXPORT_CHORD_MM] {
                    let mesh = crate::cad::tessellate(&body, chord).unwrap_or_else(|e| panic!("{name} at {draft} deg, chord {chord}: {e}"));
                    assert!(mesh.validate().watertight, "{name} at {draft} deg, chord {chord}");
                    assert_eq!(mesh.quality().degenerate_faces, 0, "{name} at {draft} deg, chord {chord}");
                    let solid = crate::csg::Solid { v: mesh.vertices.iter().map(|p| [f64::from(p.0), f64::from(p.1), f64::from(p.2)]).collect(), f: mesh.faces.clone() };
                    assert_eq!(crate::csg::self_crossings(&solid), 0, "{name} at {draft} deg, chord {chord}");
                    // Each cap's triangles all face one way: no fold doubles back over the cap.
                    for z in [0.0, -depth] {
                        let (mut signed, mut total) = (0.0, 0.0);
                        for f in &solid.f {
                            let p = f.map(|i| solid.v[i as usize]);
                            if p.iter().all(|q| (q[2] - z).abs() < 1e-6) {
                                let nz = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
                                signed += nz;
                                total += nz.abs();
                            }
                        }
                        assert!(total > 0.0 && (signed.abs() - total).abs() < 1e-9 * total, "{name} at {draft} deg, chord {chord}: a cap folds ({signed} of {total})");
                    }
                }
            }
        }
        for head in [Head::Trefoil, Head::Mouchette] {
            let s = rose(head);
            let regions = s.sweep_regions().unwrap();
            assert!(super::super::solid::extrude(s.plane.plane().unwrap(), &regions, [0.0, 0.0, -2.0], 3f64.to_radians()).is_err(), "{head:?} cusps refuse a draft");
            let straight = super::super::solid::extrude(s.plane.plane().unwrap(), &regions, [0.0, 0.0, -2.0], 0.0).unwrap();
            assert!(crate::cad::tessellate(&straight, crate::cad::EXPORT_CHORD_MM).unwrap().validate().watertight, "{head:?} cuts straight");
        }
    }

    /// The section revolved about the finger closes when tessellated, comfort bore and fillets included.
    #[test]
    fn a_revolved_arch_section_tessellates_closed() {
        for keel in [0.0, 1.0] {
            for (fillet, comfort) in [(0.0, 0.0), (0.3, 0.12)] {
                let s = arch_section(&ArchSection { keel, fillet_mm: fillet, comfort_mm: comfort, ..ArchSection::default() }).unwrap();
                let regions = s.sweep_regions().unwrap();
                let body = super::super::solid::revolve(s.plane.plane().unwrap(), &regions, [0.0; 3], [0.0, 0.0, 1.0], TAU).unwrap();
                for chord in [crate::cad::PREVIEW_CHORD_MM, crate::cad::EXPORT_CHORD_MM] {
                    let mesh = crate::cad::tessellate(&body, chord).unwrap_or_else(|e| panic!("keel {keel}, fillet {fillet}, comfort {comfort}: {e}"));
                    assert!(mesh.validate().watertight);
                }
            }
        }
    }

    #[test]
    fn a_wheel_of_lancets_keeps_its_bar_between_lights_and_its_radii() {
        let w = Lights::default();
        let s = lights(&w).unwrap();
        assert_eq!(s.sweep_regions().unwrap().len(), 24);
        assert!((w.land_mm().unwrap() - 0.9).abs() < 1e-6, "{}", w.land_mm().unwrap());
        let (lo, hi) = radii(&s, [0.0, 0.0]);
        assert!((lo - 10.3).abs() < 1e-6 && (hi - 12.05).abs() < 1e-6, "{lo} {hi}");
        // Too short for an equilateral head, so each light is a drop arch springing from its sill.
        let jambs = s.entities.iter().filter(|e| matches!(e.geometry, Geometry::Line { .. })).count();
        assert_eq!(jambs, 0, "no straight jamb under a drop arch");
        let mut tall = w.clone();
        tall.apex_r_mm = 13.0;
        let s = lights(&tall).unwrap();
        assert_eq!(s.entities.iter().filter(|e| matches!(e.geometry, Geometry::Line { .. })).count(), 48, "equilateral heads on straight jambs");
    }

    #[test]
    fn every_head_closes_inside_its_cell_with_the_bar_between_neighbours() {
        // A cell four times wider than it is deep takes only a mouchette.
        let all = &Head::ALL[..];
        let cases: [(u32, f64, f64, f64, f64, &[Head]); 5] = [
            (24, 10.3, 12.05, 0.9, 0.0, all),
            (8, 3.0, 7.0, 0.8, 0.0, all),
            (8, 4.4, 6.8, 0.8, 1.1, all),
            (12, 2.5, 7.0, 0.6, 0.0, all),
            (8, 6.1, 7.4, 0.6, 0.0, &[Head::Mouchette]),
        ];
        for (count, sill, apex, bar, width, fits) in cases {
            for head in Head::ALL {
                for clockwise in [false, true] {
                    let l = Lights { count, sill_r_mm: sill, apex_r_mm: apex, bar_mm: bar, width_mm: width, head, clockwise, phase_deg: 30.0, centre: [1.0, -2.0] };
                    let s = match lights(&l) {
                        Ok(s) => s,
                        Err(e) if !fits.contains(&head) && e.to_string().contains("needs") => continue,
                        Err(e) => panic!("{count} {head:?} {width}: {e}"),
                    };
                    assert!(fits.contains(&head), "{count} {head:?} fits a cell it should not");
                    let regions = s.sweep_regions().unwrap();
                    assert_eq!(regions.len(), count as usize, "{head:?}");
                    let area: f64 = regions.iter().map(|r| r.area()).sum::<f64>() / count as f64;
                    assert!(regions.iter().all(|r| (r.area() - area).abs() < 1e-6 * area), "every light the same");
                    assert!(l.land_mm().unwrap() >= bar - 1e-6, "{count} {head:?}: {}", l.land_mm().unwrap());
                    let (lo, hi) = radii(&s, l.centre);
                    assert!(lo >= sill - 1e-6 && hi <= apex + 1e-6, "{count} {head:?}: {lo} {hi}");
                }
            }
        }
    }

    #[test]
    fn a_net_that_cannot_hold_its_lights_is_refused_by_name() {
        let crowded = Lights { count: 60, sill_r_mm: 3.0, apex_r_mm: 4.0, ..Lights::default() };
        assert!(lights(&crowded).unwrap_err().to_string().contains("no room"));
        let flat = Lights { apex_r_mm: 10.5, head: Head::Round, ..Lights::default() };
        let e = lights(&flat).unwrap_err().to_string();
        assert!(e.contains("round head") && e.contains("needs"), "{e}");
        let wide = Lights { count: 8, width_mm: 3.0, sill_r_mm: 3.0, apex_r_mm: 6.0, bar_mm: 0.8, ..Lights::default() };
        assert!(lights(&wide).unwrap_err().to_string().contains("between neighbours"));
        assert!(lights(&Lights { bar_mm: 0.0, ..Lights::default() }).is_err());
        assert!(lights(&Lights { count: 2, ..Lights::default() }).is_err());
    }

    #[test]
    fn the_arch_section_stands_on_its_bore_and_keels_on_the_parting_plane() {
        let a = ArchSection::default();
        let s = arch_section(&a).unwrap();
        assert_eq!(s.plane, Workplane::section());
        assert_eq!((s.entities.len(), s.sweep_regions().unwrap().len()), (1, 1), "one closed polyline");
        let walk = |s: &Sketch| -> Vec<V> { s.points.iter().map(|p| p.xy).collect() };
        let p = walk(&s);
        let chord = (0..p.len()).map(|i| len(sub(p[(i + 1) % p.len()], p[i]))).fold(0.0, f64::max);
        assert!(p.len() <= 512 && chord <= 0.05, "{} points, chords to {chord}", p.len());
        let lo = p.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min);
        let hi = p.iter().map(|q| q[0]).fold(0.0, f64::max);
        assert!(lo == a.bore_r_mm && hi == a.bore_r_mm + a.thickness_mm, "the bore is tightest at the size's radius and the keel at the thickness: {lo} {hi}");
        // Springing at the bore corner, the fillet takes the corner's last hundredths of width; on straight feet it keeps them.
        let span = |p: &[V]| p.iter().map(|q| q[1].abs()).fold(0.0, f64::max);
        assert!(span(&p) < 2.8 && span(&p) > 2.8 - 0.3, "{}", span(&p));
        assert!((span(&walk(&arch_section(&ArchSection { keel: 0.5, ..a.clone() }).unwrap())) - 2.8).abs() < 1e-12);
        // The angle between the flanks at the apex: 155 degrees for a full keel, straight on for a round top.
        let keel_angle = |p: &[V]| {
            let i = (0..p.len()).max_by(|x, y| p[*x][0].total_cmp(&p[*y][0])).unwrap();
            let (u, v) = (sub(p[(i + p.len() - 1) % p.len()], p[i]), sub(p[(i + 1) % p.len()], p[i]));
            (dot(u, v) / (len(u) * len(v))).acos().to_degrees()
        };
        assert!((keel_angle(&p) - 155.3).abs() < 1.0, "{}", keel_angle(&p));
        let round = walk(&arch_section(&ArchSection { keel: 0.0, ..a.clone() }).unwrap());
        assert!(keel_angle(&round) > 178.0, "{}", keel_angle(&round));
        for keel in [0.0, 0.3, 0.7, 1.0] {
            for (fillet, comfort) in [(0.0, 0.0), (0.3, 0.0), (0.0, 0.12), (0.3, 0.12), (0.6, 0.3)] {
                let s = arch_section(&ArchSection { keel, fillet_mm: fillet, comfort_mm: comfort, ..a.clone() }).unwrap_or_else(|e| panic!("{keel} {fillet} {comfort}: {e}"));
                assert!(s.profile_region().unwrap().area() > 10.0);
            }
        }
        let squat = ArchSection { thickness_mm: 2.0, ..a };
        assert!(arch_section(&squat).unwrap_err().to_string().contains("needs at least"));
    }

    #[test]
    fn an_arcade_draws_a_loop_per_bay_and_one_outline_standing_on_its_sill() {
        for head in [Head::Pointed, Head::Round, Head::Trefoil] {
            let a = Arcade { head, ..Arcade::default() };
            let s = arcade(&a, [0.0, 0.0], 0.0).unwrap();
            let regions = s.sweep_regions().unwrap();
            assert_eq!(regions.len(), 3, "{head:?}");
            assert!((a.bay_mm() - 1.6).abs() < 1e-12);
            let outline = arcade_outline(&a, crate::outline::STEP).unwrap();
            let bays: f64 = regions.iter().map(|r| r.area()).sum();
            let sill = 6.0 * 0.4;
            let area = crate::outline::area(&outline);
            assert!((area - bays - sill).abs() < 0.005 * area, "{head:?}: {area} against {}", bays + sill);
            assert!(outline.len() <= crate::setting::PLAIN_MAX_STAMP_POINTS, "{}", outline.len());
            let (lo, hi) = outline.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
            assert!((lo[0] + 3.0).abs() < 1e-9 && (hi[0] - 3.0).abs() < 1e-9 && (lo[1] + 1.2).abs() < 1e-9 && hi[1] <= 1.2 + 1e-12 && hi[1] > 1.2 - 0.01, "{lo:?} {hi:?}");
        }
        let moved = arcade(&Arcade::default(), [1.0, 11.0], 90.0).unwrap();
        let first = arcade(&Arcade::default(), [0.0, 0.0], 0.0).unwrap();
        let a = moved.sweep_regions().unwrap();
        assert!((a.iter().map(|r| r.area()).sum::<f64>() - first.sweep_regions().unwrap().iter().map(|r| r.area()).sum::<f64>()).abs() < 1e-9);
        let squat = Arcade { height_mm: 1.0, ..Arcade::default() };
        assert!(arcade(&squat, [0.0, 0.0], 0.0).unwrap_err().to_string().contains("needs"));
        assert!(arcade_outline(&Arcade { sill_mm: 0.0, ..Arcade::default() }, 0.1).is_err());
        assert!(arcade(&Arcade { bays: 3, width_mm: 1.0, ..Arcade::default() }, [0.0, 0.0], 0.0).is_err());
    }
}
