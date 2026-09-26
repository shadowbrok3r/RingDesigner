//! The meshed loft: closed sections matched piece for piece from their seams, blended smoothly through every section, capped, and closed by construction.
use super::SurfaceKind;
use super::builders::Made;
use super::twist::{self, CORNER_DEG, MAX_SECTION, MAX_TRIANGLES};
use crate::csg::{P3, Solid};
use crate::setting::Named;
use crate::sketch::region;
use anyhow::{Context, Result, bail, ensure};
use cadkernel::geom2d::Curve;
use cadkernel::space::Plane;

/// The key a meshed loft's mesh carries as a mesh value.
pub const LOFT: &str = "loft.meshed";
/// Most stations between two neighbouring sections.
pub const MAX_SPAN_STATIONS: usize = 256;
/// Most a point's track turns between two stations more than a chord apart, degrees.
pub const TURN_STEP_DEG: f64 = 5.0;
/// Parameter samples behind a curved piece's length table.
const TABLE: usize = 512;
/// Most times a curved piece's point count is doubled to hold the chord.
const MAX_SPLITS: usize = 1024;
/// Least cosine between a section's own turn and the way the loft runs through it.
const FACING: f64 = 0.05;

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: P3, k: f64) -> P3 {
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
fn gap2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// One piece of a section's walk: its curve, whether it runs its own way, its length, and lengths run to evenly spaced parameters when length is not proportional to parameter.
#[derive(Clone)]
struct Piece {
    curve: Curve,
    forward: bool,
    length: f64,
    table: Vec<f64>,
}

impl Piece {
    fn new(curve: &Curve, forward: bool) -> Result<Self> {
        let proportional = matches!(curve, Curve::Line(_) | Curve::Arc(_) | Curve::Circle(_));
        let table = if proportional {
            Vec::new()
        } else {
            let mut run = Vec::with_capacity(TABLE + 1);
            run.push(0.0);
            let mut last = curve.point_at(0.0);
            for k in 1..=TABLE {
                let p = curve.point_at(k as f64 / TABLE as f64);
                run.push(run[k - 1] + gap2(p, last));
                last = p;
            }
            run
        };
        let length = if proportional { curve.length() } else { table[TABLE] };
        ensure!(length.is_finite() && length > 1e-9, "Loft: a section has a piece of no length");
        Ok(Self { curve: curve.clone(), forward, length, table })
    }
    /// The curve's parameter at `share` of its own length from its start.
    fn parameter(&self, share: f64) -> f64 {
        let share = share.clamp(0.0, 1.0);
        if self.table.is_empty() {
            return share;
        }
        let want = share * self.length;
        let k = self.table.partition_point(|l| *l < want).clamp(1, TABLE);
        let (a, b) = (self.table[k - 1], self.table[k]);
        let f = if b > a { ((want - a) / (b - a)).clamp(0.0, 1.0) } else { 0.0 };
        (k as f64 - 1.0 + f) / TABLE as f64
    }
    /// The point at `share` of the walk along this piece.
    fn at(&self, share: f64) -> [f64; 2] {
        self.curve.point_at(self.parameter(if self.forward { share } else { 1.0 - share }))
    }
    /// The unit direction of travel at `share` of the walk along this piece.
    fn heading(&self, share: f64) -> [f64; 2] {
        let d = self.curve.tangent_at(self.parameter(if self.forward { share } else { 1.0 - share }));
        let l = d[0].hypot(d[1]).max(1e-300);
        if self.forward { [d[0] / l, d[1] / l] } else { [-d[0] / l, -d[1] / l] }
    }
    /// Whether `n` points from the piece's start, evenly spaced by length, sag past `chord`, or turn past the kernel's cap between chords longer than `chord`.
    fn rough(&self, n: usize, chord: f64) -> bool {
        let p: Vec<[f64; 2]> = (0..=n).map(|k| self.at(k as f64 / n as f64)).collect();
        let sag = (0..n)
            .map(|k| gap2(self.at((k as f64 + 0.5) / n as f64), [0.5 * (p[k][0] + p[k + 1][0]), 0.5 * (p[k][1] + p[k + 1][1])]))
            .fold(0.0, f64::max);
        let cap = twist::angle_cap(chord);
        let turn = p.windows(3).any(|w| {
            let (u, v) = ([w[1][0] - w[0][0], w[1][1] - w[0][1]], [w[2][0] - w[1][0], w[2][1] - w[1][1]]);
            u[0].hypot(u[1]).max(v[0].hypot(v[1])) > chord && (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]).abs() > cap
        });
        sag > chord || turn
    }
    /// Points the piece takes from its start to hold `chord`.
    fn need(&self, chord: f64) -> usize {
        let cap = twist::angle_cap(chord).to_degrees();
        match &self.curve {
            Curve::Line(_) => 1,
            Curve::Arc(a) => ((a.sweep().abs() / twist::step_for(a.radius, chord, cap)).ceil() as usize).max(2),
            Curve::Circle(c) => ((std::f64::consts::TAU / twist::step_for(c.radius, chord, cap)).ceil() as usize).max(8),
            _ => fewest(4, MAX_SPLITS, |n| self.rough(n, chord)),
        }
    }
}

/// The fewest `n` from `from` up to `most` for which `rough(n)` is false: doubled until smooth, then halved back by bisection.
fn fewest(from: usize, most: usize, rough: impl Fn(usize) -> bool) -> usize {
    let mut hi = from;
    while rough(hi) && hi < most {
        hi = (2 * hi).min(most);
    }
    let mut lo = (hi / 2).max(from.saturating_sub(1));
    if hi == from {
        return hi;
    }
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if rough(mid) { lo = mid } else { hi = mid }
    }
    hi
}

/// A section's closed chain as pieces in walking order from its seam, the start of its first piece.
fn walk(curves: &[Curve]) -> Result<Vec<Piece>> {
    let senses = region::senses(curves).context("Loft: a section does not close into one loop")?;
    curves.iter().zip(senses).map(|(c, forward)| Piece::new(c, forward)).collect()
}

/// The same chain walked the other way round from the same seam.
fn reversed(walk: &[Piece]) -> Vec<Piece> {
    walk.iter()
        .rev()
        .map(|p| Piece { forward: !p.forward, ..p.clone() })
        .collect()
}

/// How far the walk turns where piece `j` starts, radians.
fn turn_before(walk: &[Piece], j: usize) -> f64 {
    let before = walk[(j + walk.len() - 1) % walk.len()].heading(1.0);
    let after = walk[j].heading(0.0);
    (before[0] * after[0] + before[1] * after[1]).clamp(-1.0, 1.0).acos()
}

/// The walk as `per[j]` points on piece `j`, evenly spaced by length from each piece's start.
fn sample(walk: &[Piece], per: &[usize]) -> Vec<[f64; 2]> {
    walk.iter().zip(per).flat_map(|(p, n)| (0..*n).map(move |k| p.at(k as f64 / *n as f64))).collect()
}

/// A ring's mean point and its vector area.
fn moments(ring: &[P3]) -> (P3, P3) {
    let n = ring.len() as f64;
    let c = ring.iter().fold([0.0; 3], |s, p| add(s, *p)).map(|v| v / n);
    let area = (0..ring.len()).fold([0.0; 3], |s, j| add(s, cross(sub(ring[j], c), sub(ring[(j + 1) % ring.len()], c))));
    (c, scale(area, 0.5))
}

/// Values and derivatives of the four cubic Hermite bases at `u`.
fn hermite(u: f64) -> ([f64; 4], [f64; 4]) {
    let (u2, u3) = (u * u, u * u * u);
    (
        [2.0 * u3 - 3.0 * u2 + 1.0, u3 - 2.0 * u2 + u, -2.0 * u3 + 3.0 * u2, u3 - u2],
        [6.0 * u2 - 6.0 * u, 3.0 * u2 - 4.0 * u + 1.0, -6.0 * u2 + 6.0 * u, 3.0 * u2 - 2.0 * u],
    )
}

/// Each ring point's rate per unit of the loft's parameter at every section: the three-point slope inside, the parabola's at the ends, the chord between two sections.
fn slopes(rings: &[Vec<P3>], knots: &[f64]) -> Vec<Vec<P3>> {
    let n = rings.len();
    let m = rings[0].len();
    let h: Vec<f64> = knots.windows(2).map(|w| w[1] - w[0]).collect();
    let chord = |i: usize, j: usize| scale(sub(rings[i + 1][j], rings[i][j]), 1.0 / h[i]);
    let mut out = vec![vec![[0.0; 3]; m]; n];
    for j in 0..m {
        if n == 2 {
            out[0][j] = chord(0, j);
            out[1][j] = chord(0, j);
            continue;
        }
        for i in 1..n - 1 {
            out[i][j] = scale(add(scale(chord(i - 1, j), h[i]), scale(chord(i, j), h[i - 1])), 1.0 / (h[i - 1] + h[i]));
        }
        out[0][j] = sub(scale(chord(0, j), 2.0), out[1][j]);
        out[n - 1][j] = sub(scale(chord(n - 2, j), 2.0), out[n - 2][j]);
    }
    out
}

/// A closed loft through `sections`, each a plane and a closed chain of curves on it, matched piece for piece from each chain's seam and blended smoothly through every section, within `chord` mm.
pub fn mesh(sections: &[(Plane, Vec<Curve>)], chord: f64) -> Result<Made> {
    ensure!(chord.is_finite() && chord > 0.0, "Loft: the chord must be positive");
    ensure!(sections.len() >= 2, "Loft needs 2–32 compatible sections");
    let mut walks = sections
        .iter()
        .enumerate()
        .map(|(i, (_, curves))| walk(curves).with_context(|| format!("Loft: section {}", i + 1)))
        .collect::<Result<Vec<_>>>()?;
    let count = walks[0].len();
    if let Some(k) = walks.iter().position(|w| w.len() != count) {
        bail!("Loft sections need the same number of pieces round: section 1 has {count}, section {} has {}", k + 1, walks[k].len());
    }
    let place = |walks: &[Vec<Piece>]| -> (Vec<usize>, Vec<Vec<[f64; 2]>>, Vec<Vec<P3>>) {
        let per: Vec<usize> = (0..count).map(|j| walks.iter().map(|w| w[j].need(chord)).max().unwrap_or(1)).collect();
        let flat: Vec<Vec<[f64; 2]>> = walks.iter().map(|w| sample(w, &per)).collect();
        let rings = flat.iter().zip(sections).map(|(f, (plane, _))| f.iter().map(|p| plane.point_at(*p)).collect()).collect();
        (per, flat, rings)
    };
    let (mut per, mut flat, mut rings) = place(&walks);
    // Each section winds round the way the loft runs through it, or all of them are walked the other way from the same seams.
    let n = sections.len();
    let mut centres: Vec<P3> = rings.iter().map(|r| moments(r).0).collect();
    for i in 1..n {
        ensure!(norm(sub(centres[i], centres[i - 1])) > 1e-6, "Loft: sections {i} and {} stand at the same place", i + 1);
    }
    let mut winding = Vec::with_capacity(n);
    for i in 0..n {
        let run = sub(centres[(i + 1).min(n - 1)], centres[i.saturating_sub(1)]);
        let area = moments(&rings[i]).1;
        let (lr, la) = (norm(run), norm(area));
        ensure!(lr > 1e-9 && la > 1e-12, "Loft: section {} encloses no area, or doubles back on its neighbours", i + 1);
        let facing = dot(run, area) / (lr * la);
        ensure!(facing.abs() >= FACING, "Loft: section {} lies along the loft; each section stands across the way the loft runs", i + 1);
        winding.push(facing > 0.0);
    }
    if let Some(k) = winding.iter().position(|w| *w != winding[0]) {
        bail!("Loft sections need matching winding: section {} winds the other way round the loft from section 1", k + 1);
    }
    if !winding[0] {
        walks = walks.iter().map(|w| reversed(w)).collect();
        (per, flat, rings) = place(&walks);
        centres = rings.iter().map(|r| moments(r).0).collect();
    }
    let m = rings[0].len();
    ensure!((3..=MAX_SECTION).contains(&m), "Loft: the sections would take {m} points round; a section takes 3 to {MAX_SECTION}");
    let corner: Vec<bool> = (0..count).map(|j| count > 1 && walks.iter().any(|w| turn_before(w, j) >= CORNER_DEG.to_radians())).collect();
    let mut starts = Vec::with_capacity(count);
    let mut at = 0;
    for p in &per {
        starts.push(at);
        at += p;
    }
    // Knots by the distance between section centres.
    let mut knots = vec![0.0];
    for i in 1..n {
        knots.push(knots[i - 1] + norm(sub(centres[i], centres[i - 1])));
    }
    let slope = slopes(&rings, &knots);
    let track = |i: usize, j: usize, u: f64| -> P3 {
        let (b, _) = hermite(u);
        let h = knots[i + 1] - knots[i];
        let p = add(scale(rings[i][j], b[0]), scale(slope[i][j], b[1] * h));
        add(p, add(scale(rings[i + 1][j], b[2]), scale(slope[i + 1][j], b[3] * h)))
    };
    let heading = |i: usize, u: f64| -> P3 {
        let (_, d) = hermite(u);
        let h = knots[i + 1] - knots[i];
        (0..m).fold([0.0; 3], |s, j| {
            let p = add(scale(rings[i][j], d[0]), scale(slope[i][j], d[1] * h));
            add(s, add(p, add(scale(rings[i + 1][j], d[2]), scale(slope[i + 1][j], d[3] * h))))
        })
    };
    // Stations in each span: the fewest that hold every track to the chord and turn it at most the step between them.
    let turn_cap = TURN_STEP_DEG.to_radians();
    let mut steps = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let rough = |k: usize| {
            (0..m).any(|j| {
                let p: Vec<P3> = (0..=k).map(|s| track(i, j, s as f64 / k as f64)).collect();
                let sag = (0..k).any(|s| norm(sub(track(i, j, (s as f64 + 0.5) / k as f64), scale(add(p[s], p[s + 1]), 0.5))) > chord);
                let turn = p.windows(3).any(|w| {
                    let (a, b) = (sub(w[1], w[0]), sub(w[2], w[1]));
                    let (la, lb) = (norm(a), norm(b));
                    la.max(lb) > chord && la.min(lb) > 1e-12 && (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos() > turn_cap
                });
                sag || turn
            })
        };
        steps.push(fewest(1, MAX_SPAN_STATIONS, rough));
    }
    let stations: usize = steps.iter().sum::<usize>() + 1;
    let triangles = (stations - 1) * m * 2 + 2 * (m - 2);
    ensure!(triangles <= MAX_TRIANGLES, "Loft would take {triangles} triangles; the most is {MAX_TRIANGLES}: ease its bends or simplify its sections");
    let mut v: Vec<P3> = Vec::with_capacity(stations * m);
    let mut along: Vec<(P3, f64)> = Vec::with_capacity(stations);
    for (i, k) in steps.iter().enumerate() {
        for s in 0..*k {
            let u = s as f64 / *k as f64;
            if s == 0 {
                v.extend_from_slice(&rings[i]);
            } else {
                v.extend((0..m).map(|j| track(i, j, u)));
            }
            along.push((heading(i, u), knots[i] + u * (knots[i + 1] - knots[i])));
        }
    }
    v.extend_from_slice(&rings[n - 1]);
    along.push((heading(n - 2, 1.0), knots[n - 1]));
    for s in 0..stations - 1 {
        let (ta, tb) = (along[s].0, along[s + 1].0);
        for j in 0..m {
            let d = sub(v[(s + 1) * m + j], v[s * m + j]);
            ensure!(
                dot(d, ta) > 0.0 && dot(d, tb) > 0.0,
                "Loft folds through itself {:.0}% along: the sections tilt more than they stand apart; space them further or ease the bend",
                100.0 * along[s].1 / knots[n - 1]
            );
        }
    }
    // The sides part at every corner of the sections.
    let corners: Vec<usize> = (0..count).filter(|j| corner[*j]).map(|j| starts[j]).collect();
    let run_of: Vec<usize> = match corners.first() {
        None => vec![0; m],
        Some(&first) => {
            let mut run = vec![0; m];
            let mut r = corners.len() - 1;
            for step in 0..m {
                let j = (first + step) % m;
                if corners.contains(&j) {
                    r = (r + 1) % corners.len();
                }
                run[j] = r;
            }
            run
        }
    };
    let runs = corners.len().max(1);
    let mut names: Vec<String> = (0..runs).map(|r| if runs == 1 { "Side".to_string() } else { format!("Side {}", r + 1) }).collect();
    names.extend(["Start cap".to_string(), "End cap".to_string()]);
    let mut kinds = vec![SurfaceKind::Freeform; runs];
    kinds.extend([SurfaceKind::Plane; 2]);
    let at = |s: usize, j: usize| (s * m + j % m) as u32;
    let mut f: Vec<[u32; 3]> = Vec::with_capacity(triangles);
    let mut patch: Vec<u32> = Vec::with_capacity(triangles);
    for s in 0..stations - 1 {
        for j in 0..m {
            let (a, b, c, d) = (at(s, j), at(s, j + 1), at(s + 1, j + 1), at(s + 1, j));
            // Diagonals alternate like a chequerboard.
            if (s + j) % 2 == 0 {
                f.extend([[a, b, c], [a, c, d]]);
            } else {
                f.extend([[a, b, d], [b, c, d]]);
            }
            patch.extend([run_of[j] as u32; 2]);
        }
    }
    // Caps fill each end section in its own plane, facing out of the loft.
    let last = stations - 1;
    for (end, s, out) in [(0, 0, scale(along[0].0, -1.0)), (n - 1, last, along[last].0)] {
        let cap = twist::fill(&flat[end], "Loft")?;
        let normal = |t: &[u32; 3]| {
            let [p, q, r] = t.map(|k| v[at(s, k as usize) as usize]);
            cross(sub(q, p), sub(r, p))
        };
        let flip = cap.first().is_some_and(|t| dot(normal(t), out) < 0.0);
        for t in &cap {
            let t = if flip { [t[0], t[2], t[1]] } else { *t };
            f.push(t.map(|k| at(s, k as usize)));
            patch.push(runs as u32 + u32::from(end != 0));
        }
    }
    let solid = Solid { v, f };
    let check = solid.check(true);
    ensure!(
        check.open_edges == 0 && check.repeated_edges == 0,
        "Loft did not close ({} open edges, {} repeated)",
        check.open_edges,
        check.repeated_edges
    );
    ensure!(check.zero_area_faces == 0, "Loft makes {} faces of no area; space the sections further apart", check.zero_area_faces);
    ensure!(check.volume > 0.0, "Loft turned inside out");
    let crossings = check.self_crossings.unwrap_or(0);
    ensure!(crossings == 0, "Loft crosses itself ({crossings} pairs of faces meet); space the sections further, ease the bend or shrink them");
    // Creases: a rail along every corner, and each cap's rim cut at the corners.
    let mut creases: Vec<Vec<P3>> = corners.iter().map(|j| (0..stations).map(|s| solid.v[at(s, *j) as usize]).collect()).collect();
    for s in [0, last] {
        match corners.as_slice() {
            [] => creases.push((0..=m).map(|j| solid.v[at(s, j) as usize]).collect()),
            all => {
                for (k, from) in all.iter().enumerate() {
                    let to = all[(k + 1) % all.len()];
                    let span = (to + m - from) % m;
                    let span = if span == 0 { m } else { span };
                    creases.push((0..=span).map(|q| solid.v[at(s, from + q) as usize]).collect());
                }
            }
        }
    }
    creases.truncate(2048);
    Ok(Made { key: LOFT.to_string(), named: Named { solid, patch, names }, kinds, creases, gem: None, seat: None, stations: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Attach, Component, Document, Feature, Operation, Placement};
    use crate::sketch::{Geometry, Sketch, Workplane};
    use crate::{AlphaLibrary, BuildParams, RingDesign};
    use std::f64::consts::PI;

    const PREVIEW: BuildParams = BuildParams { theta_steps: 256, profile_steps: 128, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };
    const EXPORT: BuildParams = BuildParams { theta_steps: 1024, profile_steps: 320, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: false, refine: None, soften_mm: 0.0 };

    fn meshed(sections: Vec<Sketch>) -> Operation {
        Operation::Loft { sections: sections.into_iter().map(Into::into).collect(), meshed: true }
    }
    fn design(op: Operation) -> RingDesign {
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Loft".into(), enabled: true, operation: op, component: Component::default() }).unwrap();
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
        let c = m.solid().check(true);
        assert_eq!((c.open_edges, c.repeated_edges, c.zero_area_faces, c.self_crossings), (0, 0, 0, Some(0)), "{what}");
        assert!(c.volume > 0.0, "{what}");
        let v = mesh.validate();
        assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{what}: {v:?}");
    }
    /// A `w` by `h` rectangle `z` over the XY plane.
    fn slab(w: f64, h: f64, z: f64) -> Sketch {
        let mut s = Sketch::rectangle(w, h);
        s.plane.origin = [0.0, 0.0, z];
        s
    }
    /// A disc of radius `r` whose plane stands at `origin` with axes `x` and `y`.
    fn disc(r: f64, origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Sketch {
        let mut s = Sketch::circle(r);
        s.plane = Workplane { origin, x, y, on_face: None };
        s
    }

    /// A feather's section: a flat underside, round edges `t` thick, a top cambered `camber` over the edges and a rachis `ridge` over its crown, in six pieces from its leading edge.
    fn vane(w: f64, t: f64, camber: f64, ridge: f64) -> Sketch {
        let e = 0.5 * t;
        let xn = (w - e).max(0.05);
        let camber = camber * (xn / 0.5).min(1.0);
        let q = (0.18 + 0.1 * w).min(0.6 * xn);
        let top = |x: f64| e + camber * (1.0 - (x / xn).powi(2));
        let slope = |x: f64| -2.0 * camber * x / (xn * xn);
        let l = 4.0 / 3.0 * e;
        let mut s = Sketch::default();
        let cubic = |s: &mut Sketch, p: [[f64; 2]; 4]| {
            let ids = p.map(|q| s.point(q));
            s.entity(Geometry::Bezier { points: ids });
        };
        // A parabola's stretch as a cubic through its tangents' meeting point.
        let arc = |x0: f64, x1: f64| -> [[f64; 2]; 4] {
            let q = [0.5 * (x0 + x1), top(x0) + slope(x0) * 0.5 * (x1 - x0)];
            let (p0, p3) = ([x0, top(x0)], [x1, top(x1)]);
            [p0, [p0[0] + 2.0 / 3.0 * (q[0] - p0[0]), p0[1] + 2.0 / 3.0 * (q[1] - p0[1])], [p3[0] + 2.0 / 3.0 * (q[0] - p3[0]), p3[1] + 2.0 / 3.0 * (q[1] - p3[1])], p3]
        };
        let lean = (1.0 + (2.0 * camber / xn).powi(2)).sqrt();
        let (a, b) = (s.point([-xn, -e]), s.point([xn, -e]));
        s.entity(Geometry::Line { a, b });
        cubic(&mut s, [[xn, -e], [xn + l, -e], [xn + l / lean, e - l * (2.0 * camber / xn) / lean], [xn, e]]);
        cubic(&mut s, arc(xn, q));
        let (yq, crown) = (top(q), top(0.0) + ridge);
        let h1 = 4.0 / 3.0 * (crown - yq);
        cubic(&mut s, [[q, yq], [0.5 * q, yq + h1], [-0.5 * q, yq + h1], [-q, yq]]);
        cubic(&mut s, arc(-q, -xn));
        cubic(&mut s, [[-xn, e], [-xn - l / lean, e - l * (2.0 * camber / xn) / lean], [-xn - l, -e], [-xn, -e]]);
        s
    }

    /// A feather `length` long and `width` wide along a planar rachis bent at `radius`, lifted `lift` over its root, through `count` sections from quill to tip.
    fn feather(length: f64, width: f64, radius: f64, lift: f64, count: usize) -> Vec<Sketch> {
        let keys = [(0.0, 0.42), (0.1, 0.5), (0.3, 1.0), (0.65, 0.9), (0.88, 0.62), (1.0, 0.44)];
        let half = |s: f64| {
            let k = keys.windows(2).find(|w| s <= w[1].0).unwrap_or(&keys[keys.len() - 2..]);
            let f = ((s - k[0].0) / (k[1].0 - k[0].0)).clamp(0.0, 1.0);
            0.5 * width * (k[0].1 + (k[1].1 - k[0].1) * f)
        };
        (0..count)
            .map(|i| {
                let s = i as f64 / (count - 1) as f64;
                let run = s * length;
                let (at, heading) = if radius.is_finite() {
                    let a = run / radius;
                    ([radius * a.sin(), radius * (1.0 - a.cos())], a)
                } else {
                    ([run, 0.0], 0.0)
                };
                let z = lift * (s / 0.4).min(1.0).powi(2);
                let mut v = vane(half(s).max(0.44), 0.85, 0.12 * width.min(2.0), 0.12);
                v.plane = Workplane { origin: [at[0], at[1], z], x: [-heading.sin(), heading.cos(), 0.0], y: [0.0, 0.0, 1.0], on_face: None };
                v
            })
            .collect()
    }

    #[test]
    fn a_meshed_loft_through_a_frustum_is_that_frustum() {
        // Rectangles from 2 x 1.5 to 1 x 0.75 over 8 mm are a frustum of h/3 (A + a + sqrt(A a)) = 14 mm³, with or without sections between.
        for params in [PREVIEW, EXPORT] {
            for between in [0usize, 1, 3] {
                let k = between + 1;
                let sections = (0..=k).map(|i| i as f64 / k as f64).map(|t| slab(2.0 - t, 1.5 - 0.75 * t, 8.0 * t)).collect();
                let (m, mesh) = made(meshed(sections), params).unwrap();
                closed(&m, &mesh, "frustum");
                assert!((m.solid().volume() - 14.0).abs() < 1e-9, "{between} between: {}", m.solid().volume());
                assert_eq!(m.named.names, ["Side 1", "Side 2", "Side 3", "Side 4", "Start cap", "End cap"]);
            }
        }
        // Discs from r 1 to r 0.5 over 6 mm: the cone's frustum, 3.5 pi, to the chord's polygon.
        let up = |r: f64, z: f64| disc(r, [0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        for (params, within) in [(PREVIEW, 0.005), (EXPORT, 0.002)] {
            let (m, mesh) = made(meshed(vec![up(1.0, 0.0), up(0.75, 3.0), up(0.5, 6.0)]), params).unwrap();
            closed(&m, &mesh, "cone");
            let want = 3.5 * PI;
            assert!((m.solid().volume() / want - 1.0).abs() < within, "{} against {want}", m.solid().volume());
        }
        // Discs round a quarter turn of radius 5, each wound against the way the loft runs: Pappus's pi r² times the 5 pi / 2 arc.
        for (params, within) in [(PREVIEW, 0.006), (EXPORT, 0.003)] {
            let sections = (0..7)
                .map(|i| 0.5 * PI * i as f64 / 6.0)
                .map(|a: f64| disc(1.0, [5.0 * a.cos(), 5.0 * a.sin(), 0.0], [a.cos(), a.sin(), 0.0], [0.0, 0.0, 1.0]))
                .collect();
            let (m, mesh) = made(meshed(sections), params).unwrap();
            closed(&m, &mesh, "quarter torus");
            let want = PI * 5.0 * 0.5 * PI;
            assert!((m.solid().volume() / want - 1.0).abs() < within, "{} against {want}", m.solid().volume());
        }
    }

    #[test]
    fn feathers_close_across_their_shapes() {
        let mut worst = (0usize, 0.0f64);
        for params in [PREVIEW, EXPORT] {
            for length in [3.0, 6.5, 11.0] {
                for width in [1.2, 1.9, 2.6] {
                    for radius in [f64::INFINITY, 16.0, 3.0 * width] {
                        for lift in [0.0, 0.9] {
                            for count in [5, 7, 9] {
                                if params.theta_steps >= 512 && (count != 7 || lift == 0.0) {
                                    continue;
                                }
                                let what = format!("{length} x {width} bent {radius} lifted {lift} through {count} at {}", params.theta_steps);
                                let started = std::time::Instant::now();
                                let (m, mesh) = made(meshed(feather(length, width, radius, lift, count)), params).unwrap_or_else(|e| panic!("{what}: {e}"));
                                let ms = started.elapsed().as_secs_f64() * 1e3;
                                closed(&m, &mesh, &what);
                                worst = (worst.0.max(m.solid().f.len()), worst.1.max(ms));
                                // Six pieces with the rachis' two corners: two sides and two caps.
                                assert_eq!(m.named.names, ["Side 1", "Side 2", "Start cap", "End cap"], "{what}");
                            }
                        }
                    }
                }
            }
        }
        eprintln!("feather sweep: largest {} triangles, slowest {:.1} ms", worst.0, worst.1);
    }

    #[test]
    fn a_meshed_loft_says_where_it_cannot_close() {
        let slabs = |z: f64| slab(2.0, 1.0, z);
        // A disc against a rectangle: one piece round against four.
        let round = disc(1.0, [0.0, 0.0, 4.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let why = made(Operation::Loft { sections: vec![slabs(0.0).into(), round.into()], meshed: true }, PREVIEW).unwrap_err();
        assert!(why.contains("the same number of pieces round: section 1 has 4, section 2 has"), "{why}");
        // A section drawn the other way round.
        let mut turned = slabs(4.0);
        turned.plane.y = [0.0, -1.0, 0.0];
        let why = made(meshed(vec![slabs(0.0), turned, slabs(8.0)]), PREVIEW).unwrap_err();
        assert!(why.contains("matching winding: section 2 winds the other way"), "{why}");
        // A section standing along the loft.
        let mut along = slabs(4.0);
        along.plane.y = [0.0, 0.0, 1.0];
        let why = made(meshed(vec![slabs(0.0), along, slabs(8.0)]), PREVIEW).unwrap_err();
        assert!(why.contains("section 2 lies along the loft"), "{why}");
        // Two sections in one place.
        let why = made(meshed(vec![slabs(0.0), slabs(0.0)]), PREVIEW).unwrap_err();
        assert!(why.contains("stand at the same place"), "{why}");
        // Wide sections round a bend tighter than their reach fold through themselves.
        let tight = (0..5)
            .map(|i| 0.6 * PI * i as f64 / 4.0)
            .map(|a: f64| {
                let mut s = Sketch::rectangle(6.0, 1.0);
                s.plane = Workplane { origin: [a.cos(), a.sin(), 0.0], x: [a.cos(), a.sin(), 0.0], y: [0.0, 0.0, 1.0], on_face: None };
                s
            })
            .collect();
        let why = made(meshed(tight), PREVIEW).unwrap_err();
        assert!(why.contains("folds through itself") || why.contains("crosses itself"), "{why}");
    }

    #[test]
    fn a_kernel_loft_writes_as_it_always_did_and_a_meshed_one_is_fenced_at_six() {
        let op = |meshed: bool| Operation::Loft { sections: vec![slab(2.0, 1.0, 0.0).into(), slab(1.0, 0.5, 4.0).into()], meshed };
        let text = serde_json::to_string(&op(false)).unwrap();
        assert!(!text.contains("meshed"), "{text}");
        let back: Operation = serde_json::from_str(&text).unwrap();
        assert!(matches!(back, Operation::Loft { meshed: false, .. }));
        let text = serde_json::to_string(&op(true)).unwrap();
        assert!(text.contains("\"meshed\":true"), "{text}");
        assert!(matches!(serde_json::from_str::<Operation>(&text).unwrap(), Operation::Loft { meshed: true, .. }));
        let plain = design(op(false));
        assert!(!crate::cad::lofts_meshed(&plain));
        assert_eq!(crate::library::format_version_for(&plain), crate::library::PLAIN_FORMAT_VERSION);
        let fenced = design(op(true));
        assert!(crate::cad::lofts_meshed(&fenced));
        assert_eq!(crate::library::format_version_for(&fenced), crate::library::FORMAT_VERSION);
        // In a graph's JSON alone, anywhere, it fences the design as well.
        let graph = serde_json::json!({"nodes": [{"kind": "cad.feature", "params": {"operation": serde_json::to_value(op(true)).unwrap()}}]});
        assert!(crate::cad::lofts_meshed_json(&graph));
        assert!(!crate::cad::lofts_meshed_json(&serde_json::json!({"nodes": [{"params": {"operation": serde_json::to_value(op(false)).unwrap()}}]})));
        let driven = RingDesign { graph: Some(graph), ..RingDesign::default() };
        assert_eq!(crate::library::format_version_for(&driven), crate::library::FORMAT_VERSION);
    }

    #[test]
    fn a_feather_joins_the_band_and_the_ring_stays_watertight() {
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        // Laid along the top of the ring on its side face, its root sunk into the metal.
        let half = 0.5 * d.profile.width_mm;
        let mut sections = feather(6.0, 1.8, 14.0, 0.5, 7);
        for s in &mut sections {
            let [x, y, z] = s.plane.origin;
            s.plane.origin = [x - 3.0, y + d.inner_radius_mm() + 0.6 * d.profile.thickness_mm, half - 0.3 + z];
        }
        let component = Component { attach: Attach::Join, placement: Placement::Free, ..Component::default() };
        doc.append(Feature { id: 2, name: "Feather".into(), enabled: true, operation: meshed(sections), component }).unwrap();
        for params in [PREVIEW, EXPORT] {
            let bare = crate::mesh::try_build(&d, lib(), params).unwrap().mesh.volume_mm3();
            let with = RingDesign { cad: Some(doc.clone()), ..d.clone() };
            let b = crate::mesh::try_build(&with, lib(), params).unwrap();
            assert!(b.parts.notes.is_empty(), "{:?}", b.parts.notes);
            assert_eq!(b.parts.joined, 1);
            let v = b.report.validation;
            assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
            assert!(b.mesh.volume_mm3() > bare + 1.0, "{} against {bare}", b.mesh.volume_mm3());
        }
        d.cad = Some(doc);
        assert!(d.band_is_procedural());
    }
}
