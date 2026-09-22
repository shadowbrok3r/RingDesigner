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
/// end), holes included, as one body with a lump per region.
pub fn extrude(plane: Plane, regions: &[Region], direction: [f64; 3], draft_rad: f64) -> Result<Body> {
    swept(regions, "Extrusion", |r| {
        if r.holes.is_empty() {
            brep::extrude_tapered(plane, &r.outer, direction, draft_rad)
        } else {
            brep::extrude_region_tapered(plane, &r.loops(), direction, draft_rad)
        }
    })
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

/// Each region's body, gathered into one; a failure names its region when there is more than one.
fn swept(regions: &[Region], label: &str, build: impl Fn(&Region) -> Option<Body>) -> Result<Body> {
    ensure!(!regions.is_empty(), "Sketch has no profile geometry");
    let mut out: Option<Body> = None;
    for (i, r) in regions.iter().enumerate() {
        let body = build(r).with_context(|| {
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
