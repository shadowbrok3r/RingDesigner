//! Solids swept from sketch regions, and the frame a sketch takes on a planar face.
use super::{Workplane, region::{self, Region}};
use anyhow::{Context, Result, ensure};
use cadkernel::{
    brep::{self, Body, Coedge, Edge, Face, Loop, Lump, Shell},
    geom2d::Curve,
    space::Plane,
};
use std::collections::HashMap;

/// The frame a sketch on a planar face is read in: origin at the face's area centroid, `x` along
/// its longest straight boundary edge (turned to agree with the face plane's own x), `normal`
/// out of the solid and `y = normal × x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceFrame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub normal: [f64; 3],
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l.is_finite() && l > 1e-12).then(|| v.map(|c| c / l))
}
/// `v` with its component along the unit `n` taken out.
fn flatten(v: [f64; 3], n: [f64; 3]) -> [f64; 3] {
    let d = dot(v, n);
    std::array::from_fn(|k| v[k] - n[k] * d)
}

impl FaceFrame {
    /// The frame of a planar face, read off the boundary the kernel gives it.
    pub fn of(face: &brep::PlanarFaceProfile) -> Result<Self> {
        let plane = face.plane;
        let normal = unit(face.outward).context("Sketch face has no normal")?;
        let loops: Vec<Vec<Curve>> = face
            .loops
            .iter()
            .map(|l| l.iter().flat_map(|c| if matches!(c, Curve::Polyline(_)) { c.segments() } else { vec![c.clone()] }).collect())
            .collect();
        let outer = loops.first().filter(|l| !l.is_empty()).context("Sketch face has no boundary")?;
        let about = outer[0].point_at(0.0);
        // Centroids are affine, so the centroid in the face's parameters is the world one.
        let mut total = [0.0; 3];
        for (i, l) in loops.iter().enumerate() {
            let m = region::loop_moments(l, about).context("Sketch face boundary does not close")?;
            let s = if (m[0] > 0.0) == (i == 0) { 1.0 } else { -1.0 };
            for k in 0..3 {
                total[k] += s * m[k];
            }
        }
        ensure!(total[0].abs() > 1e-12, "Sketch face encloses no area");
        let origin = plane.point_at([about[0] + total[1] / total[0], about[1] + total[2] / total[0]]);
        let px = unit(flatten(plane.x_axis, normal)).context("Sketch face plane is degenerate")?;
        let py = cross(normal, px);
        // Lengths and directions are measured in the world, which the parameters need not be.
        let mut best: Option<(f64, f64, [f64; 3])> = None;
        for c in outer {
            let Curve::Line(l) = c else { continue };
            let d: [f64; 3] = {
                let (a, b) = (plane.point_at(l.start), plane.point_at(l.end));
                std::array::from_fn(|k| b[k] - a[k])
            };
            let length = dot(d, d).sqrt();
            let Some(mut u) = unit(d) else { continue };
            let along = dot(u, px);
            if along < -1e-9 || (along.abs() <= 1e-9 && dot(u, py) < 0.0) {
                u = u.map(|v| -v);
            }
            let align = dot(u, px).abs();
            let tie = |b: f64| (length - b).abs() <= 1e-9 * (1.0 + b);
            let better = match best {
                None => true,
                Some((b, a, _)) => (length > b && !tie(b)) || (tie(b) && align > a + 1e-12),
            };
            if better {
                best = Some((length, align, u));
            }
        }
        let x = unit(flatten(best.map_or(px, |b| b.2), normal)).context("Sketch face edge is degenerate")?;
        Ok(Self { origin, x, y: cross(normal, x), normal })
    }
    /// A direction given in this frame, in the world.
    pub fn vector(&self, v: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|k| self.x[k] * v[0] + self.y[k] * v[1] + self.normal[k] * v[2])
    }
    /// A point given in this frame, in the world.
    pub fn point(&self, p: [f64; 3]) -> [f64; 3] {
        let v = self.vector(p);
        std::array::from_fn(|k| self.origin[k] + v[k])
    }
}

impl Workplane {
    /// This workplane read in `frame`: its origin an offset from the frame's, its axes turned with it.
    pub fn on_frame(&self, frame: &FaceFrame) -> Result<Plane> {
        ensure!(
            self.origin.iter().chain(&self.x).chain(&self.y).all(|v| v.is_finite() && v.abs() < 1e6),
            "Plane contains invalid coordinates"
        );
        let p = Plane::from_axes(frame.point(self.origin), frame.vector(self.x), frame.vector(self.y));
        ensure!(p.is_orthonormal(), "Workplane axes must be unit length and perpendicular");
        Ok(p)
    }
}

/// Every region extruded along `direction` with a draft of `draft_rad` (positive narrows the far
/// end), holes included, as one body with a lump per region; `direction` may run against the
/// plane's normal as well as along it.
pub fn extrude(plane: Plane, regions: &[Region], direction: [f64; 3], draft_rad: f64) -> Result<Body> {
    let n = plane.normal().context("Extrusion plane has no normal")?;
    // Every loop wound counter-clockwise about the way the extrusion runs, as the kernel tapers it.
    let along = dot(direction, n) >= 0.0;
    let wind = !along || draft_rad != 0.0;
    let wound = |l: &Vec<Curve>| if wind { winding(l, if along { 1.0 } else { -1.0 }) } else { l.clone() };
    swept(regions, "Extrusion", |r| {
        if r.holes.is_empty() {
            brep::extrude_tapered(plane, &wound(&r.outer), direction, draft_rad)
        } else {
            brep::extrude_region_tapered(plane, &r.loops().iter().map(wound).collect::<Vec<_>>(), direction, draft_rad)
        }
    })
}

/// Loop `l` walked so its signed area has the sign of `sign`: as it is, or its curves in the reverse order.
fn winding(l: &[Curve], sign: f64) -> Vec<Curve> {
    match region::loop_moments(l, [0.0; 2]) {
        Some(m) if m[0] * sign < 0.0 => l.iter().rev().cloned().collect(),
        _ => l.to_vec(),
    }
}

/// Every region revolved `angle_rad` about the axis through `pivot`, holes included, as one body
/// with a lump per region.
pub fn revolve(plane: Plane, regions: &[Region], pivot: [f64; 3], axis: [f64; 3], angle_rad: f64) -> Result<Body> {
    swept(regions, "Revolution", |r| {
        if r.holes.is_empty() {
            brep::revolve(plane, &r.outer, pivot, axis, angle_rad)
        } else {
            brep::revolve_region(plane, &r.loops(), pivot, axis, angle_rad)
        }
    })
}

/// `r` with every loop's pieces meeting exactly: each joint the mean of the two ends the region builder
/// joined within its own reach, a line's ends moved onto its joints and an arc's centre onto its two
/// joints' bisector, so both lie on its circle. The kernel meets ends within a nanometre, and an arc
/// read from an SVG drawn to nine places misses its next piece by about that much once scaled up.
fn healed(r: &Region) -> Region {
    use cadkernel::geom2d::{Arc, Line};
    let heal = |curves: &Vec<Curve>| -> Vec<Curve> {
        let Some(forward) = region::senses(curves) else { return curves.clone() };
        let n = curves.len();
        if n < 2 || !curves.iter().all(|c| matches!(c, Curve::Line(_) | Curve::Arc(_))) {
            return curves.clone();
        }
        let end = |i: usize| curves[i].point_at(if forward[i] { 1.0 } else { 0.0 });
        let start = |i: usize| curves[i].point_at(if forward[i] { 0.0 } else { 1.0 });
        let joint: Vec<[f64; 2]> = (0..n).map(|i| {
            let (a, b) = (end(i), start((i + 1) % n));
            [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]
        }).collect();
        (0..n)
            .map(|i| {
                // The piece's own two ends, in its own direction.
                let (from, to) = (joint[(i + n - 1) % n], joint[i]);
                let (p0, p1) = if forward[i] { (from, to) } else { (to, from) };
                match &curves[i] {
                    Curve::Line(_) => Curve::Line(Line { start: p0, end: p1 }),
                    Curve::Arc(a) => {
                        let m = [0.5 * (p0[0] + p1[0]), 0.5 * (p0[1] + p1[1])];
                        let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
                        let l = dx.hypot(dy);
                        if !(l > 1e-12) {
                            return curves[i].clone();
                        }
                        let nrm = [-dy / l, dx / l];
                        let k = (a.centre[0] - m[0]) * nrm[0] + (a.centre[1] - m[1]) * nrm[1];
                        let centre = [m[0] + nrm[0] * k, m[1] + nrm[1] * k];
                        Curve::Arc(Arc {
                            centre,
                            radius: (p0[0] - centre[0]).hypot(p0[1] - centre[1]),
                            start_angle: (p0[1] - centre[1]).atan2(p0[0] - centre[0]),
                            end_angle: (p1[1] - centre[1]).atan2(p1[0] - centre[0]),
                        })
                    }
                    other => other.clone(),
                }
            })
            .collect()
    };
    Region { outer: heal(&r.outer), holes: r.holes.iter().map(heal).collect(), entities: r.entities.clone(), rim: r.rim }
}

/// Each region's body, gathered into one; a failure names its region when there is more than one. A
/// region the kernel refuses is tried once more with its pieces meeting exactly ([`healed`]), so a
/// region it takes as drawn is built as drawn.
fn swept(regions: &[Region], label: &str, build: impl Fn(&Region) -> Option<Body>) -> Result<Body> {
    ensure!(!regions.is_empty(), "Sketch has no profile geometry");
    let mut out: Option<Body> = None;
    for (i, r) in regions.iter().enumerate() {
        let body = build(r).or_else(|| build(&healed(r))).with_context(|| {
            if regions.len() == 1 {
                format!("{label}: unsupported or degenerate geometry")
            } else {
                format!("{label} of region {} of {}: unsupported or degenerate geometry", i + 1, regions.len())
            }
        })?;
        match &mut out {
            None => out = Some(body),
            Some(all) => append(all, &body).with_context(|| format!("{label}: region {} could not join the body", i + 1))?,
        }
    }
    let body = out.context("Sketch has no profile geometry")?;
    let faults = body.validate();
    ensure!(faults.is_empty(), "{label}: generated invalid topology: {faults:?}");
    Ok(body)
}

/// The lumps of `source` added to `target` as lumps of their own, every node re-keyed.
pub fn append(target: &mut Body, source: &Body) -> Option<()> {
    let vertices: HashMap<_, _> = source.vertices.iter().map(|(k, v)| (k, target.vertices.insert(v.clone()))).collect();
    let curves: HashMap<_, _> = source.curves.iter().map(|(k, v)| (k, target.curves.insert(v.clone()))).collect();
    let surfaces: HashMap<_, _> = source.surfaces.iter().map(|(k, v)| (k, target.surfaces.insert(v.clone()))).collect();
    let lumps: HashMap<_, _> = source
        .lumps
        .iter()
        .map(|(k, v)| (k, target.lumps.insert(Lump { shells: Vec::new(), provenance: v.provenance })))
        .collect();
    let shells: HashMap<_, _> = source
        .shells
        .iter()
        .map(|(k, v)| Some((k, target.shells.insert(Shell { faces: Vec::new(), owner: *lumps.get(&v.owner)?, provenance: v.provenance }))))
        .collect::<Option<_>>()?;
    let faces: HashMap<_, _> = source
        .faces
        .iter()
        .map(|(k, v)| {
            let face = Face {
                surface: *surfaces.get(&v.surface)?,
                forward: v.forward,
                loops: Vec::new(),
                owner: *shells.get(&v.owner)?,
                provenance: v.provenance,
            };
            Some((k, target.faces.insert(face)))
        })
        .collect::<Option<_>>()?;
    let loops: HashMap<_, _> = source
        .loops
        .iter()
        .map(|(k, v)| Some((k, target.loops.insert(Loop { coedges: Vec::new(), owner: *faces.get(&v.owner)?, provenance: v.provenance }))))
        .collect::<Option<_>>()?;
    let edges: HashMap<_, _> = source
        .edges
        .iter()
        .map(|(k, v)| {
            let edge = Edge {
                curve: *curves.get(&v.curve)?,
                start_parameter: v.start_parameter,
                end_parameter: v.end_parameter,
                start: *vertices.get(&v.start)?,
                end: *vertices.get(&v.end)?,
                coedges: Vec::new(),
                provenance: v.provenance,
            };
            Some((k, target.edges.insert(edge)))
        })
        .collect::<Option<_>>()?;
    let coedges: HashMap<_, _> = source
        .coedges
        .iter()
        .map(|(k, v)| {
            let coedge = Coedge {
                edge: *edges.get(&v.edge)?,
                forward: v.forward,
                pcurve: v.pcurve.clone(),
                owner: *loops.get(&v.owner)?,
                provenance: v.provenance,
            };
            Some((k, target.coedges.insert(coedge)))
        })
        .collect::<Option<_>>()?;
    for (k, v) in source.edges.iter() {
        target.edges.get_mut(*edges.get(&k)?)?.coedges = v.coedges.iter().map(|c| coedges.get(c).copied()).collect::<Option<_>>()?;
    }
    for (k, v) in source.loops.iter() {
        target.loops.get_mut(*loops.get(&k)?)?.coedges = v.coedges.iter().map(|c| coedges.get(c).copied()).collect::<Option<_>>()?;
    }
    for (k, v) in source.faces.iter() {
        target.faces.get_mut(*faces.get(&k)?)?.loops = v.loops.iter().map(|l| loops.get(l).copied()).collect::<Option<_>>()?;
    }
    for (k, v) in source.shells.iter() {
        target.shells.get_mut(*shells.get(&k)?)?.faces = v.faces.iter().map(|f| faces.get(f).copied()).collect::<Option<_>>()?;
    }
    for (k, v) in source.lumps.iter() {
        target.lumps.get_mut(*lumps.get(&k)?)?.shells = v.shells.iter().map(|s| shells.get(s).copied()).collect::<Option<_>>()?;
    }
    for root in &source.roots {
        target.roots.push(*lumps.get(root)?);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quatrefoil read from its nine-place SVG and drawn twice the size misses where its arcs meet by about a
    /// nanometre, which the kernel refuses; healed, the same region extrudes, its area times its height.
    #[test]
    fn a_scaled_arc_outline_extrudes_once_its_pieces_meet_exactly() {
        let mut q = crate::library::list_sketches().into_iter().find(|(n, _)| n == "gothic/gallery-quatrefoil").unwrap().1;
        q.points.iter_mut().for_each(|p| p.xy = p.xy.map(|v| v * 2.0));
        let regions = q.sweep_regions().unwrap();
        let plane = q.plane.plane().unwrap();
        assert!(brep::extrude(plane, &regions[0].outer, [0.0, 0.0, 0.3]).is_none(), "the kernel takes it as drawn now");
        let fixed = healed(&regions[0]);
        let senses = region::senses(&fixed.outer).unwrap();
        let n = fixed.outer.len();
        for i in 0..n {
            let a = fixed.outer[i].point_at(if senses[i] { 1.0 } else { 0.0 });
            let b = fixed.outer[(i + 1) % n].point_at(if senses[(i + 1) % n] { 0.0 } else { 1.0 });
            assert!((a[0] - b[0]).hypot(a[1] - b[1]) < 1e-12, "joint {i}: {a:?} {b:?}");
        }
        assert!((fixed.area() - regions[0].area()).abs() < 1e-9 * regions[0].area());
        let body = extrude(plane, &regions, [0.0, 0.0, 0.3], 0.0).unwrap();
        let mesh = crate::cad::tessellate(&body, 0.005).unwrap();
        assert!((mesh.volume_mm3() / (regions[0].area() * 0.3) - 1.0).abs() < 0.01, "{} against {}", mesh.volume_mm3(), regions[0].area() * 0.3);
    }
}
