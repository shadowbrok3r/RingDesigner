//! Exact STEP solids read back into cadkernel's B-rep, the writer's records inverted one for one; anything but analytic surfaces is refused by name.
use super::{Arg, Record};
use anyhow::{Context, Result, bail, ensure};
use cadkernel::brep::{
    Body, Circle3, Coedge, Cone, Curve3, CurveKey, Cylinder, Edge, EdgeKey, Ellipse3, Face, FaceKey, Line3, Loop, LoopKey, Lump, LumpKey,
    Provenance, Shell, ShellKey, Sphere, Surface, SurfaceKey, Torus, Vertex, VertexKey,
};
use cadkernel::space::{NurbsCurve3, Plane};
use std::collections::HashMap;
use std::f64::consts::TAU;

/// Solid record `id` as a cadkernel body, lengths at `mm` millimetres per unit and angles at `rad` radians per unit.
pub(super) fn body(records: &HashMap<usize, Record>, id: usize, mm: f64, rad: f64) -> Result<Body> {
    let mut lift = Lift { records, body: Body::new(), vertices: HashMap::new(), edges: HashMap::new(), curves: HashMap::new(), surfaces: HashMap::new(), mm, rad };
    let (kind, args) = lift.expect(Some(id), &["MANIFOLD_SOLID_BREP", "BREP_WITH_VOIDS"])?;
    let lump = lift.body.lumps.insert(Lump { shells: Vec::new(), provenance: Provenance::Synthesized });
    lift.body.roots.push(lump);
    let mut shells = vec![lift.shell(args.get(1), lump)?];
    if kind == "BREP_WITH_VOIDS" {
        for void in args.get(2).map(Arg::list).unwrap_or_default() {
            let (_, v) = lift.expect(void.reference(), &["ORIENTED_CLOSED_SHELL"])?;
            shells.push(lift.shell(v.get(2), lump)?);
        }
    }
    lift.body.lumps.get_mut(lump).context("the solid lost its lump")?.shells = shells;
    let flaws = lift.body.validate();
    ensure!(flaws.is_empty(), "its topology does not hold together: {:?}", flaws.first());
    Ok(lift.body)
}

/// What a record is, in words: its type, or the typed parts of a complex one.
fn describe(r: &Record) -> String {
    let kind = if r.kind.is_empty() { r.parts.iter().map(|(k, _)| k.as_str()).find(|k| k.starts_with("B_SPLINE")).unwrap_or("COMPLEX_ENTITY") } else { &r.kind };
    match kind {
        k if k.starts_with("B_SPLINE_SURFACE") => "B-spline surface".into(),
        k if k.starts_with("B_SPLINE_CURVE") => "B-spline curve".into(),
        k => k.to_ascii_lowercase().replace('_', " "),
    }
}

/// The parameters an edge between `a` and `b` spans on `curve`, rising; once round from `a` when it closes on itself.
fn span(curve: &Curve3, a: [f64; 3], b: [f64; 3], closed: bool) -> (f64, f64) {
    match curve {
        Curve3::Circle(_) | Curve3::Ellipse(_) => {
            let t0 = curve.parameter_at(a);
            if closed {
                return (t0, t0 + TAU);
            }
            let mut t1 = curve.parameter_at(b);
            while t1 <= t0 {
                t1 += TAU;
            }
            while t1 > t0 + TAU {
                t1 -= TAU;
            }
            (t0, t1)
        }
        Curve3::Nurbs(n) if closed => {
            let (d0, d1) = n.domain();
            if n.periodicity() {
                let t0 = curve.parameter_at(a);
                (t0, t0 + (d1 - d0))
            } else {
                (d0, d1)
            }
        }
        _ => (curve.parameter_at(a), curve.parameter_at(b)),
    }
}

/// A body being lifted, and every record already turned into a node of it.
struct Lift<'a> {
    records: &'a HashMap<usize, Record>,
    body: Body,
    vertices: HashMap<usize, VertexKey>,
    /// Each EDGE_CURVE's kernel edge, and whether that edge runs the record's own way, first vertex to second.
    edges: HashMap<usize, (EdgeKey, bool)>,
    curves: HashMap<usize, CurveKey>,
    surfaces: HashMap<usize, SurfaceKey>,
    mm: f64,
    rad: f64,
}

impl<'a> Lift<'a> {
    fn record(&self, id: Option<usize>) -> Result<(usize, &'a Record)> {
        let id = id.context("a reference is missing where one belongs")?;
        Ok((id, self.records.get(&id).with_context(|| format!("it names a missing record #{id}"))?))
    }
    /// Simple record `id` of one of `kinds`: its type and parameters.
    fn expect(&self, id: Option<usize>, kinds: &[&str]) -> Result<(&'a str, &'a [Arg])> {
        let (id, r) = self.record(id)?;
        ensure!(kinds.contains(&r.kind.as_str()), "record #{id} is a {} where a {} belongs", describe(r), kinds[0].to_ascii_lowercase().replace('_', " "));
        Ok((r.kind.as_str(), r.args.as_slice()))
    }
    fn numbers(&self, a: Option<&Arg>, kind: &str) -> Result<[f64; 3]> {
        let (_, args) = self.expect(a.and_then(Arg::reference), &[kind])?;
        let xyz: Vec<f64> = args.get(1).map(Arg::list).unwrap_or_default().iter().filter_map(Arg::number).collect();
        ensure!(xyz.len() == 3 && xyz.iter().all(|v| v.is_finite()), "a {} is not three numbers", kind.to_ascii_lowercase().replace('_', " "));
        Ok([xyz[0], xyz[1], xyz[2]])
    }
    fn point(&self, a: Option<&Arg>) -> Result<[f64; 3]> {
        Ok(self.numbers(a, "CARTESIAN_POINT")?.map(|v| v * self.mm))
    }
    fn direction(&self, a: Option<&Arg>) -> Result<[f64; 3]> {
        let d = self.numbers(a, "DIRECTION")?;
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        ensure!(l > 1e-12, "a direction has no length");
        Ok(d.map(|v| v / l))
    }
    fn length(&self, a: Option<&Arg>) -> Result<f64> {
        let v = a.and_then(Arg::number).context("a length is not a number")?;
        ensure!(v.is_finite(), "a length is not finite");
        Ok(v * self.mm)
    }
    /// An AXIS2_PLACEMENT_3D as a unit frame, its axis turned about when `flip`.
    fn frame(&self, a: Option<&Arg>, flip: bool) -> Result<Plane> {
        let (_, args) = self.expect(a.and_then(Arg::reference), &["AXIS2_PLACEMENT_3D"])?;
        let origin = self.point(args.get(1))?;
        let axis = match args.get(2) {
            Some(Arg::Ref(_)) => self.direction(args.get(2))?,
            _ => [0.0, 0.0, 1.0],
        };
        let axis = if flip { axis.map(|v| -v) } else { axis };
        let x = match args.get(3) {
            Some(Arg::Ref(_)) => self.direction(args.get(3))?,
            _ if axis[0].abs() < 0.9 => [1.0, 0.0, 0.0],
            _ => [0.0, 1.0, 0.0],
        };
        Plane::orthonormal(origin, x, axis).context("a placement's axes are parallel")
    }
    /// A B-spline curve from its degree, control points, knot multiplicities and knots, and its weights when rational.
    fn spline(&self, degree: Option<&Arg>, points: Option<&Arg>, multiplicities: Option<&Arg>, knots: Option<&Arg>, weights: Option<&Arg>) -> Result<Curve3> {
        let degree = degree.and_then(Arg::number).context("a B-spline curve has no degree")? as usize;
        let points = points.map(Arg::list).unwrap_or_default().iter().map(|p| self.point(Some(p))).collect::<Result<Vec<_>>>()?;
        let multiplicities: Vec<f64> = multiplicities.map(Arg::list).unwrap_or_default().iter().filter_map(Arg::number).collect();
        let values: Vec<f64> = knots.map(Arg::list).unwrap_or_default().iter().filter_map(Arg::number).collect();
        ensure!(multiplicities.len() == values.len(), "a B-spline curve's knots and multiplicities disagree");
        let knots: Vec<f64> = values.iter().zip(&multiplicities).flat_map(|(k, m)| std::iter::repeat_n(*k, m.max(0.0) as usize)).collect();
        let weights = match weights {
            Some(w) => w.list().iter().filter_map(Arg::number).collect(),
            None => vec![1.0; points.len()],
        };
        let curve = NurbsCurve3::new_strict(degree, points, knots, weights).context("a B-spline curve does not hold together")?;
        Ok(Curve3::Nurbs(curve))
    }
    fn curve(&mut self, a: Option<&Arg>) -> Result<CurveKey> {
        let (id, r) = self.record(a.and_then(Arg::reference))?;
        if let Some(k) = self.curves.get(&id) {
            return Ok(*k);
        }
        let args = r.args.as_slice();
        let curve = match r.kind.as_str() {
            "LINE" => {
                let origin = self.point(args.get(1))?;
                let (_, v) = self.expect(args.get(2).and_then(Arg::reference), &["VECTOR"])?;
                let (d, l) = (self.direction(v.get(1))?, self.length(v.get(2))?);
                Curve3::Line(Line3 { origin, direction: d.map(|x| x * l) })
            }
            "CIRCLE" => Curve3::Circle(Circle3 { plane: self.frame(args.get(1), false)?, radius: self.length(args.get(2))? }),
            "ELLIPSE" => Curve3::Ellipse(Ellipse3 { plane: self.frame(args.get(1), false)?, major_radius: self.length(args.get(2))?, minor_radius: self.length(args.get(3))? }),
            // The space curve an edge runs along.
            "SURFACE_CURVE" | "SEAM_CURVE" | "INTERSECTION_CURVE" => {
                let k = self.curve(args.get(1))?;
                self.curves.insert(id, k);
                return Ok(k);
            }
            "B_SPLINE_CURVE_WITH_KNOTS" => self.spline(args.get(1), args.get(2), args.get(6), args.get(7), None)?,
            "" if r.parts.iter().any(|(k, _)| k == "B_SPLINE_CURVE") => {
                let part = |kind: &str| r.parts.iter().find(|(k, _)| k == kind).map(|(_, a)| a.as_slice()).unwrap_or_default();
                let (curve, knots) = (part("B_SPLINE_CURVE"), part("B_SPLINE_CURVE_WITH_KNOTS"));
                self.spline(curve.first(), curve.get(1), knots.first(), knots.get(1), part("RATIONAL_B_SPLINE_CURVE").first())?
            }
            _ => bail!("an edge runs along a {}", describe(r)),
        };
        let k = self.body.curves.insert(curve);
        self.curves.insert(id, k);
        Ok(k)
    }
    fn surface(&mut self, a: Option<&Arg>) -> Result<SurfaceKey> {
        let (id, r) = self.record(a.and_then(Arg::reference))?;
        if let Some(k) = self.surfaces.get(&id) {
            return Ok(*k);
        }
        let args = r.args.as_slice();
        let surface = match r.kind.as_str() {
            "PLANE" => Surface::Plane(self.frame(args.get(1), false)?),
            "CYLINDRICAL_SURFACE" => Surface::Cylinder(Cylinder { base: self.frame(args.get(1), false)?, radius: self.length(args.get(2))? }),
            // A STEP cone widens along its axis where ours narrows: the axis flips.
            "CONICAL_SURFACE" => Surface::Cone(Cone {
                base: self.frame(args.get(1), true)?,
                radius: self.length(args.get(2))?,
                half_angle: args.get(3).and_then(Arg::number).context("a cone's angle is not a number")? * self.rad,
            }),
            "SPHERICAL_SURFACE" => Surface::Sphere(Sphere { frame: self.frame(args.get(1), false)?, radius: self.length(args.get(2))? }),
            "TOROIDAL_SURFACE" => Surface::Torus(Torus { frame: self.frame(args.get(1), false)?, major_radius: self.length(args.get(2))?, minor_radius: self.length(args.get(3))? }),
            _ => bail!("it carries a {}", describe(r)),
        };
        let k = self.body.surfaces.insert(surface);
        self.surfaces.insert(id, k);
        Ok(k)
    }
    fn vertex(&mut self, a: Option<&Arg>) -> Result<VertexKey> {
        let (id, _) = self.record(a.and_then(Arg::reference))?;
        if let Some(k) = self.vertices.get(&id) {
            return Ok(*k);
        }
        let (_, args) = self.expect(Some(id), &["VERTEX_POINT"])?;
        let point = self.point(args.get(1))?;
        let k = self.body.vertices.insert(Vertex { point, provenance: Provenance::Synthesized });
        self.vertices.insert(id, k);
        Ok(k)
    }
    /// The kernel edge an EDGE_CURVE is, rising along its curve, and whether it runs the record's own way.
    fn edge(&mut self, a: Option<&Arg>) -> Result<(EdgeKey, bool)> {
        let (id, _) = self.record(a.and_then(Arg::reference))?;
        if let Some(k) = self.edges.get(&id) {
            return Ok(*k);
        }
        let (_, args) = self.expect(Some(id), &["EDGE_CURVE"])?;
        let (a, b) = (self.vertex(args.get(1))?, self.vertex(args.get(2))?);
        let curve = self.curve(args.get(3))?;
        let same = args.get(4).and_then(Arg::logical).context("an edge's sense is not a logical")?;
        let shape = self.body.curves.get(curve).context("an edge lost its curve")?.clone();
        let at = |v: VertexKey| self.body.vertices.get(v).map(|v| v.point).context("an edge lost a vertex");
        let (mut start, mut end, mut along) = if same { (a, b, true) } else { (b, a, false) };
        let (mut t0, mut t1) = span(&shape, at(start)?, at(end)?, a == b);
        if t1 < t0 {
            (start, end, along, t0, t1) = (end, start, !along, t1, t0);
        }
        ensure!(t0.is_finite() && t1.is_finite() && t1 > t0, "an edge spans nothing of its curve");
        let k = self.body.edges.insert(Edge { curve, start_parameter: t0, end_parameter: t1, start, end, coedges: Vec::new(), provenance: Provenance::Synthesized });
        self.edges.insert(id, (k, along));
        Ok((k, along))
    }
    fn face(&mut self, a: &Arg, shell: ShellKey) -> Result<FaceKey> {
        let (_, args) = self.expect(a.reference(), &["ADVANCED_FACE", "FACE_SURFACE"])?;
        let surface = self.surface(args.get(2))?;
        let forward = args.get(3).and_then(Arg::logical).context("a face's sense is not a logical")?;
        let face = self.body.faces.insert(Face { surface, forward, loops: Vec::new(), owner: shell, provenance: Provenance::Synthesized });
        let mut loops: Vec<LoopKey> = Vec::new();
        for bound in args.get(1).map(Arg::list).unwrap_or_default() {
            let (kind, b) = self.expect(bound.reference(), &["FACE_OUTER_BOUND", "FACE_BOUND"])?;
            let keeps = b.get(2).and_then(Arg::logical).context("a bound's orientation is not a logical")?;
            let ring = self.body.loops.insert(Loop { coedges: Vec::new(), owner: face, provenance: Provenance::Synthesized });
            let (way, l) = self.expect(b.get(1).and_then(Arg::reference), &["EDGE_LOOP", "VERTEX_LOOP"])?;
            // A vertex loop is a loop with no coedges.
            if way == "EDGE_LOOP" {
                let mut walk = Vec::new();
                for oriented in l.get(1).map(Arg::list).unwrap_or_default() {
                    let (_, o) = self.expect(oriented.reference(), &["ORIENTED_EDGE"])?;
                    let (edge, along) = self.edge(o.get(3))?;
                    let with = o.get(4).and_then(Arg::logical).context("an oriented edge's sense is not a logical")?;
                    walk.push((edge, with == along));
                }
                // A kept bound walks the loop the other way round from ours: reversed, each coedge against its walk.
                if keeps {
                    walk.reverse();
                    for w in &mut walk {
                        w.1 = !w.1;
                    }
                }
                let mut coedges = Vec::with_capacity(walk.len());
                for (edge, forward) in walk {
                    let c = self.body.coedges.insert(Coedge { edge, forward, pcurve: None, owner: ring, provenance: Provenance::Synthesized });
                    self.body.edges.get_mut(edge).context("a coedge lost its edge")?.coedges.push(c);
                    coedges.push(c);
                }
                self.body.loops.get_mut(ring).context("a face lost its loop")?.coedges = coedges;
            }
            if kind == "FACE_OUTER_BOUND" {
                loops.insert(0, ring);
            } else {
                loops.push(ring);
            }
        }
        self.body.faces.get_mut(face).context("a shell lost its face")?.loops = loops;
        Ok(face)
    }
    fn shell(&mut self, a: Option<&Arg>, lump: LumpKey) -> Result<ShellKey> {
        let (kind, args) = self.expect(a.and_then(Arg::reference), &["CLOSED_SHELL", "OPEN_SHELL"])?;
        ensure!(kind == "CLOSED_SHELL", "its shell is open");
        let shell = self.body.shells.insert(Shell { faces: Vec::new(), owner: lump, provenance: Provenance::Synthesized });
        let mut faces = Vec::new();
        for f in args.get(1).map(Arg::list).unwrap_or_default() {
            faces.push(self.face(f, shell)?);
        }
        self.body.shells.get_mut(shell).context("the solid lost its shell")?.faces = faces;
        Ok(shell)
    }
}
