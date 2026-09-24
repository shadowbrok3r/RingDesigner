//! The twisted sweep as a mesh: a closed section carried along an open planar path, turned and scaled station by station, capped and closed by construction.
use super::SurfaceKind;
use super::builders::Made;
use crate::csg::{P3, Solid};
use crate::setting::Named;
use crate::sketch::region;
use anyhow::{Context, Result, bail, ensure};
use cadkernel::geom2d::Curve;
use cadkernel::space::Plane;
use std::collections::HashMap;

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
/// Furthest a section point may stand off the plane square to the path at its start, mm.
const SQUARE_MM: f64 = 1e-6;
/// Most times a curved piece of the section is halved to hold the chord.
const MAX_SPLITS: usize = 1024;

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: P3) -> Option<P3> {
    let l = dot(a, a).sqrt();
    (l.is_finite() && l > 1e-300).then(|| a.map(|v| v / l))
}
fn gap2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// A station: its point, its direction (a joint's bisector), the share of the length run to it, and a mitre's stretch across.
#[derive(Clone, Copy, Debug)]
struct Station {
    at: P3,
    tangent: P3,
    share: f64,
    miter: f64,
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
            "Twisted sweep: the path closes on itself; a twisted sweep runs along an open path, and the twisted ring closes round the finger"
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
        "Twisted sweep: the path closes on itself; a twisted sweep runs along an open path, and the twisted ring closes round the finger"
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

/// Stations along the walked path within one twist step and `chord` at `reach`, mitred on each joint and kept `reach · tan(θ/2)` off it.
fn stations(plane: &Plane, walk: &[(Curve, bool)], twist: f64, reach: f64, chord: f64) -> Result<Vec<Station>> {
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
        let mut n = ((turns / turn_step).ceil() as usize).max((bend * span / len / bend_step).ceil() as usize).max(1);
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
                true => Station { at, tangent: joints[k].1, share, miter: 1.0 / joints[k].2 },
                false => Station { at, tangent, share, miter: 1.0 },
            });
        }
        run += len;
    }
    Ok(out)
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

/// A closed `profile` square to the open `path` at one end, swept from there, turned `twist` radians and scaled to `end_scale` with the length run, within `chord` mm.
pub fn sweep(profile_plane: Plane, profile: &[Curve], path_plane: Plane, path: &[Curve], twist: f64, end_scale: f64, chord: f64) -> Result<Made> {
    ensure!(
        twist.is_finite() && end_scale.is_finite() && end_scale > 0.0 && chord.is_finite() && chord > 0.0,
        "Twisted sweep: the twist and end scale must be finite and the scale positive"
    );
    let senses = region::senses(profile).context("Twisted sweep: the section does not close into one loop")?;
    let normal = path_plane.normal().and_then(unit).context("Twisted sweep: the path's plane has no normal")?;
    let mut walk = chain(path)?;
    let rough: Vec<P3> = profile.iter().flat_map(|c| (0..8).map(move |k| profile_plane.point_at(c.point_at(k as f64 / 8.0)))).collect();
    let off = |walk: &[(Curve, bool)]| -> Option<f64> {
        let (at, t) = walked(&path_plane, &walk[0].0, walk[0].1, 0.0)?;
        Some(rough.iter().map(|p| dot(sub(*p, at), t).abs()).fold(0.0, f64::max))
    };
    let here = off(&walk).context("Twisted sweep: the path stops dead where it starts")?;
    if here > SQUARE_MM {
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
    let reach = rough.iter().map(|p| local(*p)).map(|q| q[0].hypot(q[1])).fold(0.0, f64::max) * end_scale.max(1.0);
    let st = stations(&path_plane, &walk, twist, reach, chord)?;
    // Section edges no longer than the chord over half the largest turn between two stations.
    let turn = st.windows(2).map(|w| twist.abs() * (w[1].share - w[0].share)).fold(0.0, f64::max);
    let longest = if turn > 1e-12 { chord / (0.5 * turn).sin() } else { f64::INFINITY };
    let (points, pieces) = section(profile, &senses, chord, longest)?;
    let mut rims: Vec<Rim> = points.iter().map(|(p, corner, piece)| Rim { at: local(profile_plane.point_at(*p)), corner: *corner, piece: *piece }).collect();
    let m = rims.len();
    let area: f64 = (0..m).map(|j| {
        let (a, b) = (rims[j].at, rims[(j + 1) % m].at);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() * 0.5;
    ensure!(area.abs() > 1e-12, "Twisted sweep: the section encloses no area");
    // Counter-clockwise in (across, up), so the sides face out.
    if area < 0.0 {
        let old = rims.clone();
        rims = (0..m).map(|j| Rim { at: old[(m - j) % m].at, corner: old[(m - j) % m].corner, piece: old[(2 * m - j - 1) % m].piece }).collect();
    }
    let triangles = (st.len() - 1) * m * 2 + 2 * (m - 2);
    ensure!(triangles <= MAX_TRIANGLES, "Twisted sweep would take {triangles} triangles; the most is {MAX_TRIANGLES}: twist it less, or shrink the section");
    let mut v: Vec<P3> = Vec::with_capacity(st.len() * m);
    for s in &st {
        let (sin, cos) = (twist * s.share).sin_cos();
        let scale = 1.0 + (end_scale - 1.0) * s.share;
        let side = cross(normal, s.tangent);
        for r in &rims {
            let [a, b] = r.at;
            let out = (a * cos - b * sin) * scale * s.miter;
            let up = (a * sin + b * cos) * scale;
            v.push(std::array::from_fn(|k| s.at[k] + side[k] * out + normal[k] * up));
        }
    }
    for (i, w) in st.windows(2).enumerate() {
        for j in 0..m {
            let d = sub(v[(i + 1) * m + j], v[i * m + j]);
            ensure!(
                dot(d, w[0].tangent) > 0.0 && dot(d, w[1].tangent) > 0.0,
                "Twisted sweep folds through itself {:.0}% along the path: the section reaches past the middle of a bend, or past a corner's reach. Ease the path or shrink the section",
                100.0 * w[0].share
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
    let straight = twist.abs() < 1e-12 && walk.len() == 1 && matches!(walk[0].0, Curve::Line(_));
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
    kinds.extend([SurfaceKind::Plane; 2]);
    let mut names: Vec<String> = (0..runs).map(|r| if runs == 1 { "Side".to_string() } else { format!("Side {}", r + 1) }).collect();
    names.extend(["Start cap".to_string(), "End cap".to_string()]);
    let mut f: Vec<[u32; 3]> = Vec::with_capacity(triangles);
    let mut patch: Vec<u32> = Vec::with_capacity(triangles);
    let at = |i: usize, j: usize| (i * m + j % m) as u32;
    for i in 0..st.len() - 1 {
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
    let cap = fill(&rims.iter().map(|r| r.at).collect::<Vec<_>>())?;
    let last = st.len() - 1;
    for t in &cap {
        f.push([at(0, t[0] as usize), at(0, t[2] as usize), at(0, t[1] as usize)]);
        patch.push(runs as u32);
    }
    for t in &cap {
        f.push(t.map(|k| at(last, k as usize)));
        patch.push(runs as u32 + 1);
    }
    let solid = Solid { v, f };
    let (open, repeated) = solid.open_edges();
    ensure!(open == 0 && repeated == 0, "Twisted sweep did not close ({open} open edges, {repeated} repeated)");
    let crossings = crate::csg::self_crossings(&solid);
    ensure!(crossings == 0, "Twisted sweep crosses itself ({crossings} pairs of faces meet); ease the path, twist it less or shrink the section");
    // Creases: a rail along every corner, and each cap's rim cut at the corners.
    let mut creases: Vec<Vec<P3>> = corners.iter().map(|j| (0..st.len()).map(|i| solid.v[at(i, *j) as usize]).collect()).collect();
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
        Operation::Twist { sketch: section.into(), path, degrees, end_scale }
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
}
