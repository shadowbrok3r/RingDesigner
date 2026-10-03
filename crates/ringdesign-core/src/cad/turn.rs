//! A revolution as a mesh, for a revolved sketch arc the kernel builds but cannot tessellate: its
//! tori come back with faces missing at one chord and whole at the next. The section is walked to
//! the chord, carried round the axis in steps that hold the chord at its widest radius, and capped
//! on a part turn; closed by construction, every vertex on the axis shared by every step.
use super::SurfaceKind;
use super::builders::{self, Made};
use super::twist;
use crate::csg::{P3, Solid};
use crate::setting::Named;
use crate::sketch::{Region, region};
use anyhow::{Context, Result, bail, ensure};
use cadkernel::geom2d::Curve;
use cadkernel::space::Plane;

/// The key a revolution's mesh carries as a mesh value.
pub const TURN: &str = "revolve.mesh";
/// Most steps round the axis.
const MAX_STEPS: usize = 8192;
/// Most triangles one revolution makes.
const MAX_TRIANGLES: usize = 4_000_000;

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: P3) -> Option<P3> {
    let l = dot(a, a).sqrt();
    (l.is_finite() && l > 1e-12).then(|| a.map(|v| v / l))
}

/// Every region on `plane` revolved `angle` radians about the axis through `pivot` along `axis`, holes included, within `chord` mm.
pub fn revolve(plane: Plane, regions: &[Region], pivot: [f64; 3], axis: [f64; 3], angle: f64, chord: f64) -> Result<Made> {
    ensure!(angle.is_finite() && angle != 0.0 && angle.abs() <= std::f64::consts::TAU + 1e-9, "Revolution must be between 0 and 360 degrees");
    let full = (angle.abs() - std::f64::consts::TAU).abs() <= 1e-9;
    let a = unit(axis).context("Revolution axis is zero")?;
    let n = plane.normal().and_then(unit).context("Revolution: the sketch plane has no normal")?;
    ensure!(dot(a, n).abs() < 1e-6, "Revolution: the axis must lie in the sketch's plane");
    // A quarter of the chord off the surface, as close as the kernel's own tessellation holds a revolve's volume.
    let chord = chord / 4.0;
    // Section points as (out from the axis, along it); `out` turns toward `side` with the angle.
    let mut out = cross(n, a);
    let mut loops: Vec<(Vec<[f64; 2]>, Vec<usize>)> = Vec::new();
    let mut pieces: Vec<(Curve, usize)> = Vec::new();
    for r in regions {
        for (k, l) in r.loops().iter().enumerate() {
            let senses = region::senses(l).context("Revolution: a loop of the section does not close")?;
            let (walked, _) = twist::section(l, &senses, chord, f64::INFINITY)?;
            let offset = pieces.len();
            pieces.extend(l.iter().cloned().map(|c| (c, k)));
            let mut ring: Vec<([f64; 2], usize)> = Vec::with_capacity(walked.len());
            for (p, _, piece) in walked {
                let q = plane.point_at(p);
                let d = [q[0] - pivot[0], q[1] - pivot[1], q[2] - pivot[2]];
                let at = [dot(d, out), dot(d, a)];
                if ring.last().is_none_or(|(b, _)| (b[0] - at[0]).hypot(b[1] - at[1]) > 1e-9) {
                    ring.push((at, offset + piece));
                }
            }
            while ring.len() > 1 && (ring[0].0[0] - ring[ring.len() - 1].0[0]).hypot(ring[0].0[1] - ring[ring.len() - 1].0[1]) <= 1e-9 {
                ring.pop();
            }
            ensure!(ring.len() >= 3, "Revolution: a loop of the section has {} points round", ring.len());
            let m = ring.len();
            let mut pts: Vec<[f64; 2]> = ring.iter().map(|r| r.0).collect();
            let mut piece: Vec<usize> = ring.iter().map(|r| r.1).collect();
            // The rim counter-clockwise and every hole clockwise, each point keeping the piece its leaving edge runs along.
            let area: f64 = (0..m).map(|j| pts[j][0] * pts[(j + 1) % m][1] - pts[(j + 1) % m][0] * pts[j][1]).sum();
            if (area < 0.0) == (k == 0) {
                pts.reverse();
                piece = (0..m).map(|j| ring[(2 * m - j - 2) % m].1).collect();
            }
            loops.push((pts, piece));
        }
    }
    // The whole section on one side of the axis, turned to the side `out` points to.
    let (lo, hi) = loops.iter().flat_map(|(p, _)| p.iter().map(|q| q[0])).fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), s| (l.min(s), h.max(s)));
    ensure!(lo > -1e-7 || hi < 1e-7, "Revolution: the section crosses the axis");
    let flip = hi < 1e-7;
    if flip {
        out = out.map(|v| -v);
        for (p, _) in &mut loops {
            p.iter_mut().for_each(|q| q[0] = -q[0]);
        }
    }
    let side = cross(a, out);
    let reach = loops.iter().flat_map(|(p, _)| p.iter().map(|q| q[0])).fold(0.0, f64::max);
    ensure!(reach > 1e-9, "Revolution: the section lies on the axis");
    let step = if reach > chord { (2.0 * (1.0 - chord / reach).acos()).min(0.15) } else { 0.15 };
    let steps = ((angle.abs() / step).ceil() as usize).max(if full { 3 } else { 1 });
    ensure!(steps <= MAX_STEPS, "Revolution would take more than {MAX_STEPS} steps round");
    let stations = if full { steps } else { steps + 1 };
    let mut v: Vec<P3> = Vec::new();
    let mut f: Vec<[u32; 3]> = Vec::new();
    let mut patch: Vec<u32> = Vec::new();
    let caps = pieces.len();
    // Each loop's vertex at each station: those on the axis made once.
    let mut index: Vec<Vec<Vec<u32>>> = Vec::with_capacity(loops.len());
    for (p, _) in &loops {
        let mut at = vec![Vec::with_capacity(p.len()); stations];
        for q in p {
            if q[0] <= 1e-9 {
                let id = v.len() as u32;
                v.push(std::array::from_fn(|k| pivot[k] + a[k] * q[1]));
                at.iter_mut().for_each(|s| s.push(id));
                continue;
            }
            for (i, s) in at.iter_mut().enumerate() {
                let (sin, cos) = (angle * i as f64 / steps as f64).sin_cos();
                s.push(v.len() as u32);
                v.push(std::array::from_fn(|k| pivot[k] + a[k] * q[1] + (out[k] * cos + side[k] * sin) * q[0]));
            }
        }
        index.push(at);
    }
    let triangles = loops.iter().map(|(p, _)| p.len()).sum::<usize>() * steps * 2;
    ensure!(triangles <= MAX_TRIANGLES, "Revolution would take {triangles} triangles; the most is {MAX_TRIANGLES}");
    for (l, (p, piece)) in loops.iter().enumerate() {
        let m = p.len();
        for i in 0..steps {
            let (s0, s1) = (&index[l][i], &index[l][(i + 1) % stations]);
            for j in 0..m {
                let k = (j + 1) % m;
                for t in [[s0[j], s0[k], s1[k]], [s0[j], s1[k], s1[j]]] {
                    if t[0] != t[1] && t[1] != t[2] && t[2] != t[0] {
                        f.push(t);
                        patch.push(piece[j] as u32);
                    }
                }
            }
        }
    }
    if !full {
        ensure!(loops.len() == 1, "Revolution: a part turn of a section with holes is the kernel's alone");
        let cap = twist::fill(&loops[0].0, "Revolution")?;
        let last = stations - 1;
        // Each cap runs against the sides it meets: the start one as the fill lays it, the end one turned over.
        for t in &cap {
            f.push(t.map(|j| index[0][0][j as usize]));
            patch.push(caps as u32);
            f.push([t[0], t[2], t[1]].map(|j| index[0][last][j as usize]));
            patch.push(caps as u32 + 1);
        }
    }
    let mut solid = Solid { v, f };
    // Outward whichever way the section was walked and the turn runs: a loop's walk sets every face's turn alike.
    if solid.volume() < 0.0 {
        solid.f.iter_mut().for_each(|t| t.swap(1, 2));
    }
    let (open, repeated) = solid.open_edges();
    if open != 0 || repeated != 0 {
        bail!("Revolution did not close ({open} open edges, {repeated} repeated)");
    }
    let crossings = crate::csg::self_crossings(&solid);
    ensure!(crossings == 0, "Revolution crosses itself ({crossings} pairs of faces meet)");
    let mut names: Vec<String> = (0..caps).map(|p| format!("Turn {}", p + 1)).collect();
    names.extend(["Start cap".to_string(), "End cap".to_string()]);
    let mut kinds: Vec<SurfaceKind> = pieces
        .iter()
        .map(|(c, _)| match c {
            Curve::Line(_) => SurfaceKind::Cone,
            Curve::Arc(_) | Curve::Circle(_) => SurfaceKind::Torus,
            _ => SurfaceKind::Freeform,
        })
        .collect();
    kinds.extend([SurfaceKind::Plane; 2]);
    let named = Named { solid, patch, names };
    let creases = builders::creases(&named, builders::CREASE_DEG);
    Ok(Made { key: TURN.to_string(), named, kinds, creases, gem: None, seat: None, stations: Vec::new() })
}
