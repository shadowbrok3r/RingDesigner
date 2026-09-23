//! The constraint solver. Constraints that share no movable point are independent systems, each
//! solved on its own by damped Gauss–Newton over analytic derivatives; every step is one sparse
//! LDLᵀ factorisation of the normal equations in minimum-degree order, so the cost follows the
//! size of each system and the fill its constraints make, never the size of the sketch.
use super::{Constraint, Id, Sketch, Solution, distance};
use anyhow::{Result, bail, ensure};
use std::collections::HashMap;

/// Most movable points one connected system may join. Measured in the test profile
/// (`the_solver_at_scale`): a 512-point staircase solves from knocked positions in 1.6 ms and a
/// 16 x 32 lattice of held edges (512 points, 1952 constraints) in 5.0 ms, inside a frame.
pub const MAX_SYSTEM_POINTS: usize = 512;
/// Gauss–Newton steps before a system is called unsolvable.
const ITERATIONS: usize = 80;
/// A system counts as solved once no residual exceeds this, mm.
const SOLVED_MM: f64 = 1e-6;
/// Added to the normal equations' diagonal so a system with freedom left still has one step.
const DAMPING: f64 = 1e-7;
/// A pivot of `JᵀJ` under this holds nothing: the square of the 1e-5 singular value the dense rank cut at.
const RANK_FLOOR: f64 = 1e-10;
/// Rounding in a pivot, as a share of the largest diagonal; a pivot under it holds nothing either.
const RANK_ROUNDING: f64 = 1e-12;

/// A constraint with its points resolved to indices into `Sketch::points`.
#[derive(Clone, Copy, Debug)]
enum Term {
    Horizontal(usize, usize),
    Vertical(usize, usize),
    Coincident(usize, usize),
    Distance(usize, usize, f64),
    Symmetry(usize, usize, usize),
    Tangent(usize, usize, usize, usize),
}

impl Term {
    fn points(&self) -> ([usize; 4], usize) {
        match *self {
            Self::Horizontal(a, b) | Self::Vertical(a, b) | Self::Coincident(a, b) | Self::Distance(a, b, _) => ([a, b, 0, 0], 2),
            Self::Symmetry(a, b, c) => ([a, b, c, 0], 3),
            Self::Tangent(a, b, c, p) => ([a, b, c, p], 4),
        }
    }
    fn rows(&self) -> usize {
        match self {
            Self::Coincident(..) | Self::Symmetry(..) | Self::Tangent(..) => 2,
            _ => 1,
        }
    }
    /// The residual rows, in the order the dense solver read them.
    fn residual(&self, xy: &[[f64; 2]], out: &mut Vec<f64>) {
        match *self {
            Self::Horizontal(a, b) => out.push(xy[a][1] - xy[b][1]),
            Self::Vertical(a, b) => out.push(xy[a][0] - xy[b][0]),
            Self::Coincident(a, b) => out.extend([xy[a][0] - xy[b][0], xy[a][1] - xy[b][1]]),
            Self::Distance(a, b, mm) => out.push(distance(xy[a], xy[b]) - mm),
            Self::Symmetry(a, b, c) => out.extend([(xy[a][0] + xy[b][0]) * 0.5 - xy[c][0], (xy[a][1] + xy[b][1]) * 0.5 - xy[c][1]]),
            Self::Tangent(a, b, c, p) => {
                let (a, b, c, p) = (xy[a], xy[b], xy[c], xy[p]);
                let len = distance(a, b).max(1e-8);
                out.push(((b[0] - a[0]) * (p[0] - c[0]) + (b[1] - a[1]) * (p[1] - c[1])) / len);
                out.push(((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / len);
            }
        }
    }
    /// Each nonzero derivative as `(row within the term, point, axis, value)`.
    fn gradient(&self, xy: &[[f64; 2]], emit: &mut impl FnMut(usize, usize, usize, f64)) {
        match *self {
            Self::Horizontal(a, b) => {
                emit(0, a, 1, 1.0);
                emit(0, b, 1, -1.0);
            }
            Self::Vertical(a, b) => {
                emit(0, a, 0, 1.0);
                emit(0, b, 0, -1.0);
            }
            Self::Coincident(a, b) => {
                for k in 0..2 {
                    emit(k, a, k, 1.0);
                    emit(k, b, k, -1.0);
                }
            }
            Self::Distance(a, b, _) => {
                let d = [xy[a][0] - xy[b][0], xy[a][1] - xy[b][1]];
                let l = d[0].hypot(d[1]);
                // Coincident ends have no direction yet; pulling them apart along x starts one.
                let u = if l > 1e-12 { [d[0] / l, d[1] / l] } else { [1.0, 0.0] };
                for k in 0..2 {
                    emit(0, a, k, u[k]);
                    emit(0, b, k, -u[k]);
                }
            }
            Self::Symmetry(a, b, c) => {
                for k in 0..2 {
                    emit(k, a, k, 0.5);
                    emit(k, b, k, 0.5);
                    emit(k, c, k, -1.0);
                }
            }
            Self::Tangent(ia, ib, ic, ip) => {
                let (a, b, c, p) = (xy[ia], xy[ib], xy[ic], xy[ip]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let raw = d[0].hypot(d[1]);
                let len = raw.max(1e-8);
                let (q, w) = ([p[0] - c[0], p[1] - c[1]], [p[0] - a[0], p[1] - a[1]]);
                let r1 = (d[0] * q[0] + d[1] * q[1]) / len;
                let r2 = (d[0] * w[1] - d[1] * w[0]) / len;
                // The length only moves with the line while it is longer than its floor.
                let u = if raw > 1e-8 { [d[0] / len, d[1] / len] } else { [0.0; 2] };
                let d1 = [(q[0] - r1 * u[0]) / len, (q[1] - r1 * u[1]) / len];
                let d2 = [(w[1] - r2 * u[0]) / len, (-w[0] - r2 * u[1]) / len];
                let (along, across) = ([d[0] / len, d[1] / len], [-d[1] / len, d[0] / len]);
                for k in 0..2 {
                    emit(0, ib, k, d1[k]);
                    emit(0, ia, k, -d1[k]);
                    emit(0, ip, k, along[k]);
                    emit(0, ic, k, -along[k]);
                    emit(1, ib, k, d2[k]);
                    emit(1, ia, k, -d2[k] - across[k]);
                    emit(1, ip, k, across[k]);
                }
            }
        }
    }
}

/// Every constraint resolved to point indices, refused in the sketch's words when one names a missing point or holds an invalid length.
fn terms(s: &Sketch, index: &HashMap<Id, usize>) -> Result<Vec<Term>> {
    let at = |id: Id| index.get(&id).copied().ok_or_else(|| anyhow::anyhow!("Sketch point #{id} is missing"));
    s.constraints
        .iter()
        .map(|c| {
            Ok(match *c {
                Constraint::Horizontal(a, b) => Term::Horizontal(at(a)?, at(b)?),
                Constraint::Vertical(a, b) => Term::Vertical(at(a)?, at(b)?),
                Constraint::Coincident(a, b) => Term::Coincident(at(a)?, at(b)?),
                Constraint::Distance { a, b, mm } => {
                    let (a, b) = (at(a)?, at(b)?);
                    ensure!(mm.is_finite() && mm >= 0.0 && mm < 10000.0, "Invalid dimensional constraint");
                    Term::Distance(a, b, mm)
                }
                Constraint::Symmetry { a, b, center } => Term::Symmetry(at(a)?, at(b)?, at(center)?),
                Constraint::Tangent { a, b, center, at: p } => Term::Tangent(at(a)?, at(b)?, at(center)?, at(p)?),
            })
        })
        .collect()
}

/// Point id to its index in `Sketch::points`; duplicates are the validator's to refuse.
pub(super) fn index_of(s: &Sketch) -> HashMap<Id, usize> {
    s.points.iter().enumerate().map(|(i, p)| (p.id, i)).collect()
}

/// Checks every constraint names points the sketch has and holds a valid length.
pub(super) fn check(s: &Sketch, index: &HashMap<Id, usize>) -> Result<()> {
    terms(s, index).map(drop)
}

/// One connected set of constraints and the movable points they join.
struct System {
    /// Movable point indices, two variables each: x at `2k`, y at `2k + 1`.
    points: Vec<usize>,
    terms: Vec<Term>,
}

/// The constraints split into independent systems: joined by any movable point they share.
fn systems(s: &Sketch, terms: &[Term]) -> Vec<System> {
    let n = s.points.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let movable = |i: usize| !s.points[i].fixed;
    for t in terms {
        let (p, k) = t.points();
        let mut first = None;
        for &i in p[..k].iter().filter(|i| movable(**i)) {
            let r = root(&mut parent, i);
            match first {
                None => first = Some(r),
                Some(f) if f != r => parent[r] = f,
                _ => {}
            }
        }
    }
    // Systems in the order their first constraint comes; one of fixed points alone goes last.
    let mut by_root: HashMap<usize, usize> = HashMap::new();
    let mut out: Vec<System> = Vec::new();
    let mut pinned: Vec<Term> = Vec::new();
    for t in terms {
        let (p, k) = t.points();
        let Some(&i) = p[..k].iter().find(|i| movable(**i)) else {
            pinned.push(*t);
            continue;
        };
        let r = root(&mut parent, i);
        let at = *by_root.entry(r).or_insert_with(|| {
            out.push(System { points: Vec::new(), terms: Vec::new() });
            out.len() - 1
        });
        out[at].terms.push(*t);
    }
    for sys in &mut out {
        let mut points: Vec<usize> = sys.terms.iter().flat_map(|t| {
            let (p, k) = t.points();
            p.into_iter().take(k)
        }).filter(|i| movable(*i)).collect();
        points.sort_unstable();
        points.dedup();
        sys.points = points;
    }
    if !pinned.is_empty() {
        out.push(System { points: Vec::new(), terms: pinned });
    }
    out
}

/// A symmetric sparse LDLᵀ in a fixed elimination order: the order and the fill found once per system.
struct Factor {
    /// Variable at each elimination position, and each variable's position.
    order: Vec<usize>,
    at: Vec<usize>,
    /// Per position, the later positions its column of L reaches, ascending.
    cols: Vec<Vec<usize>>,
    /// Per position, the earlier positions whose columns reach it, ascending.
    rows: Vec<Vec<usize>>,
    /// Numeric state: L's columns, parallel to `cols`, and the pivots.
    values: Vec<Vec<f64>>,
    pivots: Vec<f64>,
}

impl Factor {
    /// The minimum-degree order and its fill for `n` variables coupled as `edges` says.
    fn plan(n: usize, edges: &[(usize, usize)]) -> Self {
        let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(a, b) in edges {
            if a != b {
                adjacent[a].push(b);
                adjacent[b].push(a);
            }
        }
        for a in &mut adjacent {
            a.sort_unstable();
            a.dedup();
        }
        let mut gone = vec![false; n];
        let mut order = Vec::with_capacity(n);
        let mut reach: Vec<Vec<usize>> = Vec::with_capacity(n);
        for _ in 0..n {
            // The remaining variable with the fewest neighbours, lowest index on a tie.
            let v = (0..n).filter(|v| !gone[*v]).min_by_key(|v| (adjacent[*v].len(), *v)).unwrap_or(0);
            gone[v] = true;
            let near = std::mem::take(&mut adjacent[v]);
            for &a in &near {
                let list = &mut adjacent[a];
                if let Ok(k) = list.binary_search(&v) {
                    list.remove(k);
                }
                for &b in &near {
                    if b != a && let Err(k) = list.binary_search(&b) {
                        list.insert(k, b);
                    }
                }
            }
            order.push(v);
            reach.push(near);
        }
        let mut at = vec![0; n];
        for (p, &v) in order.iter().enumerate() {
            at[v] = p;
        }
        let cols: Vec<Vec<usize>> = reach
            .into_iter()
            .map(|near| {
                let mut c: Vec<usize> = near.into_iter().map(|v| at[v]).collect();
                c.sort_unstable();
                c
            })
            .collect();
        let mut rows = vec![Vec::new(); n];
        for (p, c) in cols.iter().enumerate() {
            for &q in c {
                rows[q].push(p);
            }
        }
        let values = cols.iter().map(|c| vec![0.0; c.len()]).collect();
        Self { order, at, cols, rows, values, pivots: vec![0.0; n] }
    }
    /// Factors the matrix whose lower triangle `lower[q]` lists by `(position ≥ q, value)`; a pivot
    /// under `floor` is taken as zero and its direction left out. Returns how many pivots counted.
    fn factor(&mut self, lower: &[Vec<(usize, f64)>], floor: f64) -> usize {
        let n = self.order.len();
        let mut w = vec![0.0; n];
        let mut next = vec![0usize; n];
        let mut rank = 0;
        for q in 0..n {
            for &(i, v) in &lower[q] {
                w[i] += v;
            }
            for &p in &self.rows[q] {
                let k = next[p];
                next[p] += 1;
                let f = self.values[p][k] * self.pivots[p];
                if f == 0.0 {
                    continue;
                }
                for (&i, &l) in self.cols[p][k..].iter().zip(&self.values[p][k..]) {
                    w[i] -= l * f;
                }
            }
            let d = w[q];
            w[q] = 0.0;
            if d > floor {
                rank += 1;
                self.pivots[q] = d;
                for (j, &i) in self.cols[q].iter().enumerate() {
                    self.values[q][j] = w[i] / d;
                    w[i] = 0.0;
                }
            } else {
                self.pivots[q] = 0.0;
                for (j, &i) in self.cols[q].iter().enumerate() {
                    self.values[q][j] = 0.0;
                    w[i] = 0.0;
                }
            }
        }
        rank
    }
    /// Solves the factored system for `b`, given and returned by variable.
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.order.len();
        let mut x: Vec<f64> = self.order.iter().map(|v| b[*v]).collect();
        for p in 0..n {
            let xp = x[p];
            for (&i, &l) in self.cols[p].iter().zip(&self.values[p]) {
                x[i] -= l * xp;
            }
        }
        for p in 0..n {
            x[p] = if self.pivots[p] != 0.0 { x[p] / self.pivots[p] } else { 0.0 };
        }
        for p in (0..n).rev() {
            let mut xp = x[p];
            for (&i, &l) in self.cols[p].iter().zip(&self.values[p]) {
                xp -= l * x[i];
            }
            x[p] = xp;
        }
        (0..n).map(|v| x[self.at[v]]).collect()
    }
}

/// A system's variables and the Jacobian's rows as sparse `(variable, value)` lists.
struct Linear<'a> {
    sys: &'a System,
    /// Variable of each movable point's x, by point index.
    var: HashMap<usize, usize>,
}

impl<'a> Linear<'a> {
    fn new(sys: &'a System) -> Self {
        Self { sys, var: sys.points.iter().enumerate().map(|(k, p)| (*p, 2 * k)).collect() }
    }
    fn residuals(&self, xy: &[[f64; 2]], out: &mut Vec<f64>) {
        out.clear();
        for t in &self.sys.terms {
            t.residual(xy, out);
        }
    }
    /// The Jacobian's rows, each merged to one value per variable.
    fn jacobian(&self, xy: &[[f64; 2]]) -> Vec<Vec<(usize, f64)>> {
        let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
        for t in &self.sys.terms {
            let base = rows.len();
            rows.extend((0..t.rows()).map(|_| Vec::new()));
            t.gradient(xy, &mut |row, point, axis, value| {
                if let Some(v) = self.var.get(&point) {
                    let r = &mut rows[base + row];
                    let var = v + axis;
                    match r.iter_mut().find(|(k, _)| *k == var) {
                        Some(e) => e.1 += value,
                        None => r.push((var, value)),
                    }
                }
            });
        }
        rows
    }
    /// The normal matrix `JᵀJ + damping·I`'s lower triangle by elimination position.
    fn normal(&self, jac: &[Vec<(usize, f64)>], f: &Factor, damping: f64) -> Vec<Vec<(usize, f64)>> {
        let n = f.order.len();
        let mut lower: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for (q, l) in lower.iter_mut().enumerate() {
            l.push((q, damping));
        }
        for row in jac {
            for &(a, va) in row {
                for &(b, vb) in row {
                    let (pa, pb) = (f.at[a], f.at[b]);
                    if pa >= pb {
                        lower[pb].push((pa, va * vb));
                    }
                }
            }
        }
        lower
    }
}

/// The largest residual magnitude, 0 for none.
fn worst(r: &[f64]) -> f64 {
    r.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
}

/// What solving one system did: its residual, the steps it took and the freedom it has left.
struct Solved {
    residual: f64,
    iterations: usize,
    dof: usize,
}

/// Solves one system in place, or says it conflicts.
fn solve_system(sys: &System, xy: &mut [[f64; 2]]) -> Result<Solved> {
    let lin = Linear::new(sys);
    let n = 2 * sys.points.len();
    let mut r = Vec::new();
    lin.residuals(xy, &mut r);
    if n == 0 {
        let residual = worst(&r);
        ensure!(residual < SOLVED_MM, "Constraints conflict or did not converge (residual {residual:.6} mm); original sketch is unchanged");
        return Ok(Solved { residual, iterations: 0, dof: 0 });
    }
    // The coupling is fixed by which points each term names, so the order is planned once.
    let jac = lin.jacobian(xy);
    let edges: Vec<(usize, usize)> = jac.iter().flat_map(|row| row.iter().flat_map(move |a| row.iter().map(move |b| (a.0, b.0)))).collect();
    let mut f = Factor::plan(n, &edges);
    let mut jac = Some(jac);
    let mut probe = Vec::new();
    for iteration in 0..ITERATIONS {
        let residual = worst(&r);
        if residual < SOLVED_MM {
            let jac = jac.take().unwrap_or_else(|| lin.jacobian(xy));
            let lower = lin.normal(&jac, &f, 0.0);
            let biggest = lower.iter().enumerate().map(|(q, l)| l.iter().filter(|e| e.0 == q).map(|e| e.1).sum::<f64>()).fold(0.0_f64, f64::max);
            let rank = f.factor(&lower, RANK_FLOOR.max(RANK_ROUNDING * biggest));
            return Ok(Solved { residual, iterations: iteration, dof: n - rank });
        }
        let j = jac.take().unwrap_or_else(|| lin.jacobian(xy));
        let lower = lin.normal(&j, &f, DAMPING);
        f.factor(&lower, 0.0);
        let mut g = vec![0.0; n];
        for (row, rv) in j.iter().zip(&r) {
            for &(v, jv) in row {
                g[v] -= jv * rv;
            }
        }
        let delta = f.solve(&g);
        ensure!(delta.iter().all(|d| d.is_finite()), "Constraint system is singular");
        // The step is tried in place, halved while it does not lower the worst residual.
        let from: Vec<[f64; 2]> = sys.points.iter().map(|p| xy[*p]).collect();
        let mut accepted = false;
        for factor in [1.0, 0.5, 0.25, 0.125, 0.0625] {
            for (k, &p) in sys.points.iter().enumerate() {
                xy[p] = [from[k][0] + delta[2 * k] * factor, from[k][1] + delta[2 * k + 1] * factor];
            }
            lin.residuals(xy, &mut probe);
            if worst(&probe) < residual {
                accepted = true;
                break;
            }
        }
        if !accepted {
            for (k, &p) in sys.points.iter().enumerate() {
                xy[p] = from[k];
            }
            break;
        }
        std::mem::swap(&mut r, &mut probe);
    }
    bail!("Constraints conflict or did not converge (residual {:.6} mm); original sketch is unchanged", worst(&r))
}

/// Solves every system of `s` apart and gathers what they did into one solution.
pub(super) fn solve(s: &Sketch) -> Result<Solution> {
    let index = index_of(s);
    let terms = terms(s, &index)?;
    let systems = systems(s, &terms);
    if let Some(big) = systems.iter().find(|sys| sys.points.len() > MAX_SYSTEM_POINTS) {
        bail!(
            "Constraint solver takes up to {MAX_SYSTEM_POINTS} movable points joined in one system; {} are joined here. Split the sketch or delete constraints between its parts",
            big.points.len()
        );
    }
    let named: std::collections::BTreeSet<usize> = terms.iter().flat_map(|t| {
        let (p, k) = t.points();
        p.into_iter().take(k)
    }).collect();
    let loose = s.points.iter().enumerate().filter(|(i, p)| !p.fixed && !named.contains(i)).count();
    let mut xy: Vec<[f64; 2]> = s.points.iter().map(|p| p.xy).collect();
    let (mut residual, mut iterations, mut dof) = (0.0_f64, 0, 2 * loose);
    for sys in &systems {
        let done = solve_system(sys, &mut xy)?;
        residual = residual.max(done.residual);
        iterations = iterations.max(done.iterations);
        dof += done.dof;
    }
    let mut sketch = s.clone();
    for (p, xy) in sketch.points.iter_mut().zip(xy) {
        p.xy = xy;
    }
    Ok(Solution { sketch, residual_mm: residual, remaining_dof: dof, iterations })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::Geometry;

    /// The Jacobian by forward differences, as the dense solver built it.
    fn numeric(t: &Term, xy: &[[f64; 2]]) -> Vec<Vec<f64>> {
        let mut base = Vec::new();
        t.residual(xy, &mut base);
        let mut out = vec![vec![0.0; 2 * xy.len()]; base.len()];
        for p in 0..xy.len() {
            for k in 0..2 {
                let mut moved = xy.to_vec();
                moved[p][k] += 1e-7;
                let mut r = Vec::new();
                t.residual(&moved, &mut r);
                for (row, v) in r.iter().enumerate() {
                    out[row][2 * p + k] = (v - base[row]) / 1e-7;
                }
            }
        }
        out
    }

    #[test]
    fn analytic_derivatives_match_differences_on_every_constraint() {
        let xy = [[0.3, -0.2], [2.1, 0.7], [1.2, 1.9], [-0.4, 1.1]];
        for t in [
            Term::Horizontal(0, 1),
            Term::Vertical(2, 3),
            Term::Coincident(1, 3),
            Term::Distance(0, 2, 1.5),
            Term::Symmetry(0, 1, 3),
            Term::Tangent(0, 1, 2, 3),
            Term::Tangent(0, 1, 2, 0),
        ] {
            let mut analytic = vec![vec![0.0; 8]; t.rows()];
            t.gradient(&xy, &mut |row, p, k, v| analytic[row][2 * p + k] += v);
            let fd = numeric(&t, &xy);
            for (a, b) in analytic.iter().flatten().zip(fd.iter().flatten()) {
                assert!((a - b).abs() < 1e-5, "{t:?}: {analytic:?} against {fd:?}");
            }
        }
    }

    /// The freedom a dense SVD of the analytic Jacobian leaves, as the dense solver counted it.
    fn dense_dof(s: &Sketch) -> usize {
        let index = index_of(s);
        let terms = terms(s, &index).unwrap();
        let named: std::collections::BTreeSet<usize> = terms.iter().flat_map(|t| {
            let (p, k) = t.points();
            p.into_iter().take(k)
        }).collect();
        let loose = s.points.iter().enumerate().filter(|(i, p)| !p.fixed && !named.contains(i)).count();
        let xy: Vec<[f64; 2]> = s.points.iter().map(|p| p.xy).collect();
        let whole = System { points: (0..s.points.len()).filter(|i| !s.points[*i].fixed && named.contains(i)).collect(), terms };
        let lin = Linear::new(&whole);
        let rows = lin.jacobian(&xy);
        let n = 2 * whole.points.len();
        let mut j = nalgebra::DMatrix::zeros(rows.len(), n);
        for (r, row) in rows.iter().enumerate() {
            for &(v, x) in row {
                j[(r, v)] = x;
            }
        }
        let rank = if rows.is_empty() || n == 0 { 0 } else { j.svd(false, false).rank(1e-5) };
        n - rank + 2 * loose
    }

    #[test]
    fn independent_systems_solve_apart_and_count_their_own_freedom() {
        // Two rectangles and a square sharing a corner with the second: two systems.
        let mut s = Sketch::default();
        let a = s.add_rectangle([0.0, 0.0], [2.0, 1.0], false).unwrap();
        s.add_rectangle([5.0, 0.0], [7.0, 1.0], false).unwrap();
        s.add_rectangle([7.0, 1.0], [8.0, 2.0], false).unwrap();
        let index = index_of(&s);
        let terms = terms(&s, &index).unwrap();
        let systems = systems(&s, &terms);
        assert_eq!(systems.iter().map(|sys| sys.points.len()).collect::<Vec<_>>(), [4, 7]);
        // Four held horizontal or vertical sides leave each rectangle four freedoms, the pair sharing a corner six more.
        assert_eq!(s.solve().unwrap().remaining_dof, 4 + 4 + 4 - 2);
        assert_eq!(dense_dof(&s), 10);
        // Holding the first rectangle's width moves nothing of the others.
        let before = s.clone();
        let bottom = s.measure_of(a[0], [1.0, 0.0]).unwrap();
        s.dimension(&bottom, 3.0).unwrap();
        let first: Vec<Id> = before.points[..4].iter().map(|p| p.id).collect();
        for (p, q) in s.points.iter().zip(&before.points).filter(|(p, _)| !first.contains(&p.id)) {
            assert_eq!(p.xy, q.xy, "#{} stayed", p.id);
        }
        assert!((s.measured(&bottom).unwrap() - 3.0).abs() < 1e-6);
        // A constraint between fixed points alone is checked, and refused when it does not hold.
        let mut t = Sketch::default();
        let p = [[0.0, 0.0], [3.0, 0.0]].map(|xy| t.point(xy));
        t.entity(Geometry::Line { a: p[0], b: p[1] });
        t.points.iter_mut().for_each(|p| p.fixed = true);
        t.constraints.push(Constraint::Distance { a: p[0], b: p[1], mm: 3.0 });
        assert_eq!(t.solve().unwrap().remaining_dof, 0);
        t.constraints[0] = Constraint::Distance { a: p[0], b: p[1], mm: 2.0 };
        assert!(t.solve().unwrap_err().to_string().contains("conflict"));
    }

    #[test]
    fn two_hundred_held_rectangles_are_two_hundred_systems_and_a_system_past_the_cap_is_refused() {
        use crate::sketch::{Measure, tests::{held_rectangles, staircase}};
        let mut s = held_rectangles(200);
        assert_eq!((s.points.len(), s.entities.len(), s.constraints.len()), (800, 800, 1200));
        let index = index_of(&s);
        assert_eq!(systems(&s, &terms(&s, &index).unwrap()).len(), 200);
        let solved = s.solve().unwrap();
        assert_eq!((solved.remaining_dof, solved.iterations), (400, 0), "each rectangle keeps its two translations");
        // Holding the second rectangle's bottom at 2.5 mm moves its corners and nothing else.
        let before = s.clone();
        let bottom = Measure::Length { a: s.points[4].id, b: s.points[5].id };
        let held = s.dimension(&bottom, 2.5).unwrap();
        assert_eq!((held.remaining_dof, held.iterations), (400, 1));
        assert!((s.measured(&bottom).unwrap() - 2.5).abs() < 1e-6);
        let moved: Vec<usize> = (0..800).filter(|i| s.points[*i].xy != before.points[*i].xy).collect();
        assert!(!moved.is_empty() && moved.iter().all(|i| (4..8).contains(i)), "{moved:?}");
        // One system of 512 movable points solves; one of 513 is refused by name, the sketch unchanged.
        assert_eq!(staircase(MAX_SYSTEM_POINTS + 1).solve().unwrap().remaining_dof, 0);
        let big = staircase(MAX_SYSTEM_POINTS + 2);
        let e = big.solve().unwrap_err().to_string();
        assert!(e.contains("up to 512 movable points joined in one system; 513 are joined here"), "{e}");
        // And the sketch's own caps.
        let e = held_rectangles(257).solve().unwrap_err().to_string();
        assert!(e.contains("exceeds 1024 points or entities, or 2048 constraints"), "{e}");
    }

    #[test]
    fn freedom_is_counted_the_way_the_dense_rank_counted_it() {
        // A triangle held by its three sides: three rigid freedoms left, x, y and the turn.
        let mut s = Sketch::default();
        let p = [[0.0, 0.0], [4.0, 0.0], [1.0, 3.0]].map(|xy| s.point(xy));
        for k in 0..3 {
            s.entity(Geometry::Line { a: p[k], b: p[(k + 1) % 3] });
        }
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let mm = distance(s.points[a].xy, s.points[b].xy);
            s.constraints.push(Constraint::Distance { a: p[a], b: p[b], mm });
        }
        assert_eq!(s.solve().unwrap().remaining_dof, 3);
        // Holding it level takes the turn away; the same constraint again takes nothing more.
        s.constraints.push(Constraint::Horizontal(p[0], p[1]));
        assert_eq!(s.solve().unwrap().remaining_dof, 2);
        s.constraints.push(Constraint::Horizontal(p[1], p[0]));
        assert_eq!(s.solve().unwrap().remaining_dof, 2, "a repeated constraint is dependent");
        assert_eq!(dense_dof(&s), 2);
        // A line tangent to a circle with its middle on the rim: the rim's angle and the half length are left.
        let mut t = Sketch::circle(2.0);
        let (c, rim) = (t.points[0].id, t.points[1].id);
        t.points[1].xy = [0.35, 1.97];
        let q = [[4.0, 2.0], [-4.0, 2.0]].map(|xy| t.point(xy));
        t.entity(Geometry::Line { a: q[0], b: q[1] });
        t.constraints.push(Constraint::Tangent { a: q[0], b: q[1], center: c, at: rim });
        t.constraints.push(Constraint::Symmetry { a: q[0], b: q[1], center: rim });
        let solved = t.solve().unwrap();
        assert!(solved.residual_mm < 1e-6, "{}", solved.residual_mm);
        assert_eq!(solved.remaining_dof, 2, "the middle on the line makes the tangency's second row redundant");
        assert_eq!(dense_dof(&solved.sketch), 2);
        // Held rectangles and a knocked staircase agree with the dense count too.
        for s in [crate::sketch::tests::held_rectangles(12), crate::sketch::tests::staircase(40)] {
            let solved = s.solve().unwrap();
            assert_eq!(solved.remaining_dof, dense_dof(&solved.sketch));
        }
    }
}
