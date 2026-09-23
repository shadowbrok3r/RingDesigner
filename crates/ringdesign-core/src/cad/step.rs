//! STEP AP214 B-rep export. Analytic curves/surfaces stay analytic; faceted
//! source features remain planar faces. No triangle mesh is substituted for
//! an unsupported surface. The source graph remains the editable project.
use super::{Attach, Evaluated, EvaluatedComponent};
use crate::{AlphaLibrary, BuildParams, Mesh, RingDesign, sketch::Id};
use anyhow::{Context, Result, ensure};
use cadkernel::{
    brep::{Body, Curve3, Surface},
    space::Plane,
};
use std::collections::HashMap;

fn logical(v: bool) -> &'static str {
    if v { ".T." } else { ".F." }
}
fn scalar(v: f64) -> String {
    let s = format!("{v:.12}");
    let s = s.trim_end_matches('0');
    if s.ends_with('.') {
        s.into()
    } else {
        s.to_string()
    }
}
fn numbers(values: &[f64]) -> String {
    format!(
        "({})",
        values
            .iter()
            .map(|v| scalar(*v))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn refs(ids: &[usize]) -> String {
    format!(
        "({})",
        ids.iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn label(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c => {
                out.push_str("\\X2\\");
                for u in c.encode_utf16(&mut [0; 2]) {
                    out.push_str(&format!("{u:04X}"));
                }
                out.push_str("\\X0\\");
            }
        }
    }
    out.push('\'');
    out
}
fn knots(values: &[f64]) -> (String, String) {
    let mut unique = Vec::<f64>::new();
    let mut counts = Vec::<usize>::new();
    for v in values {
        if unique.last() == Some(v) {
            *counts.last_mut().unwrap() += 1;
        } else {
            unique.push(*v);
            counts.push(1);
        }
    }
    (
        format!(
            "({})",
            counts
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        numbers(&unique),
    )
}
#[derive(Default)]
struct Writer {
    records: Vec<String>,
}
impl Writer {
    fn add(&mut self, record: String) -> usize {
        self.records.push(record);
        self.records.len()
    }
    fn point(&mut self, p: [f64; 3]) -> Result<usize> {
        ensure!(p.iter().all(|v| v.is_finite()), "Nonfinite STEP point");
        Ok(self.add(format!("CARTESIAN_POINT('',{})", numbers(&p))))
    }
    fn direction(&mut self, d: [f64; 3]) -> Result<usize> {
        let n = crate::mesh::norm(d);
        ensure!(n.is_finite() && n > 1e-12, "Degenerate STEP direction");
        Ok(self.add(format!("DIRECTION('',{})", numbers(&d.map(|v| v / n)))))
    }
    fn axis(&mut self, p: Plane) -> Result<usize> {
        let origin = self.point(p.origin)?;
        let normal = self.direction(p.normal().context("STEP plane normal is zero")?)?;
        let x = self.direction(p.x_axis)?;
        Ok(self.add(format!("AXIS2_PLACEMENT_3D('',#{origin},#{normal},#{x})")))
    }
    fn spline(
        &mut self,
        degree: usize,
        points: &[[f64; 3]],
        weights: &[f64],
        values: &[f64],
    ) -> Result<usize> {
        ensure!(
            points.len() == weights.len() && values.len() == points.len() + degree + 1,
            "Invalid spline source"
        );
        let points = points
            .iter()
            .map(|p| self.point(*p))
            .collect::<Result<Vec<_>>>()?;
        let (mult, knots) = knots(values);
        Ok(self.add(format!("(BOUNDED_CURVE() B_SPLINE_CURVE({degree},{},.UNSPECIFIED.,.F.,.F.) B_SPLINE_CURVE_WITH_KNOTS({mult},{knots},.UNSPECIFIED.) CURVE() GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_CURVE({}) REPRESENTATION_ITEM(''))",refs(&points),numbers(weights))))
    }
    fn curve(&mut self, c: &Curve3) -> Result<usize> {
        Ok(match c {
            Curve3::Line(l) => {
                let p = self.point(l.origin)?;
                let direction = self.direction(l.direction)?;
                let length = crate::mesh::norm(l.direction);
                let v = self.add(format!("VECTOR('',#{direction},{})", scalar(length)));
                self.add(format!("LINE('',#{p},#{v})"))
            }
            Curve3::Circle(c) => {
                let axis = self.axis(c.plane)?;
                self.add(format!("CIRCLE('',#{axis},{})", scalar(c.radius)))
            }
            Curve3::Ellipse(e) => {
                ensure!(
                    e.major_radius >= e.minor_radius,
                    "STEP ellipse axes need major ≥ minor"
                );
                let axis = self.axis(e.plane)?;
                self.add(format!(
                    "ELLIPSE('',#{axis},{},{})",
                    scalar(e.major_radius),
                    scalar(e.minor_radius)
                ))
            }
            Curve3::Nurbs(c) => {
                self.spline(c.degree(), c.control_points(), c.weights(), c.knots())?
            }
            Curve3::PlanarSpline { plane, curve } => self.spline(
                curve.degree(),
                &curve
                    .control_points()
                    .iter()
                    .map(|p| plane.point_at(*p))
                    .collect::<Vec<_>>(),
                curve.weights(),
                curve.knots(),
            )?,
        })
    }
    fn surface(&mut self, s: &Surface) -> Result<usize> {
        Ok(match s {
            Surface::Plane(p) => {
                let axis = self.axis(*p)?;
                self.add(format!("PLANE('',#{axis})"))
            }
            Surface::Cylinder(c) => {
                let axis = self.axis(c.base)?;
                self.add(format!(
                    "CYLINDRICAL_SURFACE('',#{axis},{})",
                    scalar(c.radius)
                ))
            }
            Surface::Sphere(s) => {
                let axis = self.axis(s.frame)?;
                self.add(format!(
                    "SPHERICAL_SURFACE('',#{axis},{})",
                    scalar(s.radius)
                ))
            }
            Surface::Torus(t) => {
                let axis = self.axis(t.frame)?;
                self.add(format!(
                    "TOROIDAL_SURFACE('',#{axis},{},{})",
                    scalar(t.major_radius),
                    scalar(t.minor_radius)
                ))
            }
            Surface::Cone(c) => {
                // STEP cones grow along +axis. Reverse the source cone's axis;
                // the geometric surface is the same, independent of UV direction.
                let p = Plane::from_axes(c.base.origin, c.base.x_axis, c.base.y_axis.map(|v| -v));
                let axis = self.axis(p)?;
                self.add(format!(
                    "CONICAL_SURFACE('',#{axis},{},{})",
                    scalar(c.radius),
                    scalar(c.half_angle)
                ))
            }
            Surface::Nurbs(s) => {
                let (u, v) = s.degrees();
                let rows = s
                    .control_points()
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|p| self.point(*p))
                            .collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<Vec<_>>>()?;
                let points = format!(
                    "({})",
                    rows.iter().map(|r| refs(r)).collect::<Vec<_>>().join(",")
                );
                let (ku, kv) = s.knots();
                let (mu, ku) = knots(ku);
                let (mv, kv) = knots(kv);
                let weights = format!(
                    "({})",
                    s.weights()
                        .iter()
                        .map(|w| numbers(w))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                self.add(format!("(BOUNDED_SURFACE() B_SPLINE_SURFACE({u},{v},{points},.UNSPECIFIED.,.F.,.F.,.F.) B_SPLINE_SURFACE_WITH_KNOTS({mu},{mv},{ku},{kv},.UNSPECIFIED.) GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_SURFACE({weights}) REPRESENTATION_ITEM('') SURFACE())"))
            }
        })
    }
    fn body(&mut self, b: &Body, name: &str) -> Result<Vec<usize>> {
        ensure!(b.validate().is_empty(), "STEP source topology is invalid");
        let mut vertices = HashMap::new();
        let mut curves = HashMap::new();
        let mut surfaces = HashMap::new();
        let mut edges = HashMap::new();
        let mut faces = HashMap::new();
        let mut shells = HashMap::new();
        for (id, v) in b.vertices.iter() {
            let p = self.point(v.point)?;
            vertices.insert(id, self.add(format!("VERTEX_POINT('',#{p})")));
        }
        for (id, c) in b.curves.iter() {
            curves.insert(id, self.curve(c)?);
        }
        for (id, s) in b.surfaces.iter() {
            surfaces.insert(id, self.surface(s)?);
        }
        for (id, e) in b.edges.iter() {
            edges.insert(
                id,
                self.add(format!(
                    "EDGE_CURVE('',#{},#{},#{},{})",
                    vertices[&e.start],
                    vertices[&e.end],
                    curves[&e.curve],
                    logical(e.end_parameter >= e.start_parameter)
                )),
            );
        }
        for (id, f) in b.faces.iter() {
            let mut bounds = Vec::new();
            for (index, loop_id) in f.loops.iter().enumerate() {
                let boundary = b.loops.get(*loop_id).context("Missing face loop")?;
                let mut oriented = Vec::new();
                // The kernel keeps clockwise loops relative to outward face
                // normals; STEP positive bounds traverse the opposite sense.
                for coedge_id in boundary.coedges.iter().rev() {
                    let c = b.coedges.get(*coedge_id).context("Missing coedge")?;
                    oriented.push(self.add(format!(
                        "ORIENTED_EDGE('',*,*,#{},{})",
                        edges[&c.edge],
                        logical(!c.forward)
                    )));
                }
                let loop_id = self.add(format!("EDGE_LOOP('',{})", refs(&oriented)));
                bounds.push(self.add(format!(
                    "{}('',#{loop_id},.T.)",
                    if index == 0 {
                        "FACE_OUTER_BOUND"
                    } else {
                        "FACE_BOUND"
                    }
                )));
            }
            faces.insert(
                id,
                self.add(format!(
                    "ADVANCED_FACE('',{},#{},{})",
                    refs(&bounds),
                    surfaces[&f.surface],
                    logical(f.forward)
                )),
            );
        }
        for (id, s) in b.shells.iter() {
            shells.insert(
                id,
                self.add(format!(
                    "CLOSED_SHELL('',{})",
                    refs(&s.faces.iter().map(|f| faces[f]).collect::<Vec<_>>())
                )),
            );
        }
        let mut out = Vec::new();
        for root in &b.roots {
            let lump = b.lumps.get(*root).context("Missing solid lump")?;
            let first = lump.shells.first().context("Empty solid shell")?;
            let outer = shells[first];
            if lump.shells.len() == 1 {
                out.push(self.add(format!("MANIFOLD_SOLID_BREP({},#{outer})", label(name))));
            } else {
                let mut voids = Vec::new();
                for shell in lump.shells.iter().skip(1) {
                    voids.push(self.add(format!(
                        "ORIENTED_CLOSED_SHELL('',*,#{},.F.)",
                        shells[shell]
                    )));
                }
                out.push(self.add(format!(
                    "BREP_WITH_VOIDS({},#{outer},{})",
                    label(name),
                    refs(&voids)
                )));
            }
        }
        Ok(out)
    }
    /// One FACETED_BREP per edge-connected shell of a closed mesh, each triangle a planar face bounded by a poly loop.
    fn faceted(&mut self, mesh: &Mesh, name: &str) -> Result<Vec<usize>> {
        let mut out = Vec::new();
        for shell in shells(mesh) {
            let mut points: HashMap<u32, usize> = HashMap::new();
            let mut faces = Vec::with_capacity(shell.len());
            for fi in shell {
                let f = mesh.faces[fi];
                let mut ids = [0usize; 3];
                for (k, v) in f.iter().enumerate() {
                    ids[k] = match points.get(v) {
                        Some(p) => *p,
                        None => {
                            let p = mesh.vertices.get(*v as usize).context("A faceted face names a missing vertex")?;
                            let id = self.point([p.0 as f64, p.1 as f64, p.2 as f64])?;
                            points.insert(*v, id);
                            id
                        }
                    };
                }
                let normal = mesh.face_normal(&f).unwrap_or_else(|| {
                    let n = f.iter().filter_map(|v| mesh.normals.get(*v as usize)).fold([0.0; 3], |s, n| [s[0] + n.0 as f64, s[1] + n.1 as f64, s[2] + n.2 as f64]);
                    if crate::mesh::norm(n) > 1e-12 { n } else { [0.0, 0.0, 1.0] }
                });
                let direction = self.direction(normal)?;
                let axis = self.add(format!("AXIS2_PLACEMENT_3D('',#{},#{direction},$)", ids[0]));
                let plane = self.add(format!("PLANE('',#{axis})"));
                let polygon = self.add(format!("POLY_LOOP('',{})", refs(&ids)));
                let bound = self.add(format!("FACE_OUTER_BOUND('',#{polygon},.T.)"));
                faces.push(self.add(format!("FACE_SURFACE('',(#{bound}),#{plane},.T.)")));
            }
            let shell = self.add(format!("CLOSED_SHELL('',{})", refs(&faces)));
            out.push(self.add(format!("FACETED_BREP({},#{shell})", label(name))));
        }
        Ok(out)
    }
}
/// The faces of `mesh` grouped into shells that share edges, each in face order.
fn shells(mesh: &Mesh) -> Vec<Vec<usize>> {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..mesh.faces.len()).collect();
    let mut first: HashMap<(u32, u32), usize> = HashMap::new();
    for (fi, f) in mesh.faces.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            match first.entry((a.min(b), a.max(b))) {
                std::collections::hash_map::Entry::Occupied(e) => {
                    let (x, y) = (root(&mut parent, *e.get()), root(&mut parent, fi));
                    parent[x.max(y)] = x.min(y);
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(fi);
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot: HashMap<usize, usize> = HashMap::new();
    for fi in 0..mesh.faces.len() {
        let r = root(&mut parent, fi);
        let g = *slot.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(fi);
    }
    groups
}
/// A closed mesh a STEP file carries as faceted solids, one per shell.
pub struct Faceted<'a> {
    pub name: String,
    pub mesh: &'a Mesh,
}
pub fn export(e: &Evaluated, name: &str) -> Result<String> {
    export_with(e, name, &|c| !c.settings.reference, &[])
}
/// STEP of the kernel bodies `keep` admits as analytic solids beside `faceted` meshes as faceted ones.
pub fn export_with(e: &Evaluated, name: &str, keep: &dyn Fn(&EvaluatedComponent) -> bool, faceted: &[Faceted]) -> Result<String> {
    let mut w = Writer::default();
    let app = w.add("APPLICATION_CONTEXT('automotive_design')".into());
    w.add(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})"
    ));
    let product_context = w.add(format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let product = w.add(format!(
        "PRODUCT({},{},'',(#{product_context}))",
        label(name),
        label(name)
    ));
    let formation = w.add(format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
    let context = w.add(format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
    ));
    let definition = w.add(format!(
        "PRODUCT_DEFINITION('design','',#{formation},#{context})"
    ));
    let shape = w.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
    let mm = w.add("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))".into());
    let angle = w.add("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))".into());
    let solid_angle = w.add("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())".into());
    let uncertainty = w.add(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(0.000001),#{mm},'distance_accuracy_value','')"
    ));
    let geometry=w.add(format!("(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{mm},#{angle},#{solid_angle})) REPRESENTATION_CONTEXT('',''))"));
    let mut bodies = Vec::new();
    // A builder's part is a mesh with no B-rep to write.
    for c in e.components.iter().filter(|c| keep(c) && c.made.is_none()) {
        bodies.extend(
            w.body(&c.body, &c.name)
                .with_context(|| format!("STEP component #{} {}", c.id, c.name))?,
        );
    }
    let mut facets = Vec::new();
    for f in faceted {
        facets.extend(w.faceted(f.mesh, &f.name).with_context(|| format!("STEP faceted solid {}", f.name))?);
    }
    ensure!(!bodies.is_empty() || !facets.is_empty(), "No metal solids to export");
    if facets.is_empty() {
        let rep = w.add(format!(
            "ADVANCED_BREP_SHAPE_REPRESENTATION({}, {},#{geometry})",
            label(name),
            refs(&bodies)
        ));
        w.add(format!("SHAPE_DEFINITION_REPRESENTATION(#{shape},#{rep})"));
    } else {
        // One representation per kind of solid, each related to the product's own.
        let origin = w.axis(Plane::from_axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]))?;
        let main = w.add(format!("SHAPE_REPRESENTATION({},(#{origin}),#{geometry})", label(name)));
        w.add(format!("SHAPE_DEFINITION_REPRESENTATION(#{shape},#{main})"));
        if !bodies.is_empty() {
            let rep = w.add(format!("ADVANCED_BREP_SHAPE_REPRESENTATION({},{},#{geometry})", label(name), refs(&bodies)));
            w.add(format!("SHAPE_REPRESENTATION_RELATIONSHIP('','',#{rep},#{main})"));
        }
        let rep = w.add(format!("FACETED_BREP_SHAPE_REPRESENTATION({},{},#{geometry})", label(name), refs(&facets)));
        w.add(format!("SHAPE_REPRESENTATION_RELATIONSHIP('','',#{rep},#{main})"));
    }
    let description = if facets.is_empty() { "RingDesigner analytic feature export" } else { "RingDesigner ring export: analytic parts and faceted solids" };
    // Leave the optional timestamp unspecified for reproducible exports.
    let mut out = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('{description}'),'2;1');\nFILE_NAME({},'',(''),(''),'RingDesigner','RingDesigner','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n",
        label(name)
    );
    for (index, record) in w.records.into_iter().enumerate() {
        out.push_str(&format!("#{}={record};\n", index + 1));
    }
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(out)
}
/// The whole ring as STEP: kernel parts the band does not fuse as analytic solids, the band and every other part faceted.
pub fn ring(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams, name: &str) -> Result<String> {
    let metal = |c: &EvaluatedComponent| !c.settings.reference;
    let Some(doc) = &design.cad else {
        let built = crate::mesh::try_build(design, lib, params)?;
        let none = Evaluated { components: Vec::new(), features: Vec::new(), band: None, planes: Vec::new() };
        return export_with(&none, name, &metal, &[Faceted { name: name.to_string(), mesh: &built.mesh }]);
    };
    if doc.replaces_band() {
        let e = super::evaluate(design, lib, params)?;
        let made: Vec<Faceted> = e.components.iter().filter(|c| metal(c) && c.made.is_some()).map(|c| Faceted { name: c.name.clone(), mesh: &c.mesh }).collect();
        return export_with(&e, name, &metal, &made);
    }
    // Kernel parts joined to the band are built beside it, so each stays its own analytic solid.
    let before = super::evaluate(design, lib, params)?;
    let analytic: Vec<Id> = before.components.iter().filter(|c| metal(c) && c.made.is_none() && c.attach == Attach::Join).map(|c| c.id).collect();
    let mut apart = design.clone();
    for f in apart.cad.iter_mut().flat_map(|d| d.features.iter_mut()).filter(|f| analytic.contains(&f.id)) {
        f.component.attach = Attach::Separate;
    }
    let built = crate::mesh::try_build(&apart, lib, params)?;
    let e = built.parts.evaluated.as_ref().context("The ring's parts were not evaluated")?;
    let objects = crate::threemf::objects(&built, name);
    let made = |id: Id| e.components.iter().any(|c| c.id == id && c.made.is_some());
    let faceted: Vec<Faceted> = objects.iter().filter(|o| o.feature.is_none_or(made)).map(|o| Faceted { name: o.name.clone(), mesh: o.mesh.as_ref() }).collect();
    export_with(e, name, &|c| metal(c) && c.attach == Attach::Separate, &faceted)
}
/// A solid a STEP file holds, as [`read_solids`] finds it.
#[derive(Clone, Debug)]
pub struct Found {
    pub name: String,
    /// A FACETED_BREP, rather than an analytic MANIFOLD_SOLID_BREP or BREP_WITH_VOIDS.
    pub faceted: bool,
    pub faces: usize,
    /// A faceted solid's facets, points shared by record.
    pub mesh: Option<Mesh>,
}
/// One parameter of a STEP record, as far as reading solids needs.
#[derive(Clone, Debug)]
enum Arg {
    Ref(usize),
    Text(String),
    Number(f64),
    List(Vec<Arg>),
    Other,
}
impl Arg {
    fn reference(&self) -> Option<usize> {
        if let Arg::Ref(r) = self { Some(*r) } else { None }
    }
    fn list(&self) -> &[Arg] {
        if let Arg::List(v) = self { v } else { &[] }
    }
}
struct Cursor<'a> {
    s: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn skip(&mut self) {
        while self.s.get(self.at).is_some_and(|c| c.is_ascii_whitespace()) {
            self.at += 1;
        }
    }
    fn peek(&mut self) -> Result<u8> {
        self.skip();
        self.s.get(self.at).copied().context("STEP data ends inside a record")
    }
    fn take_while(&mut self, keep: impl Fn(u8) -> bool) -> &str {
        let start = self.at;
        while self.s.get(self.at).is_some_and(|c| keep(*c)) {
            self.at += 1;
        }
        std::str::from_utf8(&self.s[start..self.at]).unwrap_or("")
    }
    fn expect(&mut self, c: u8) -> Result<()> {
        ensure!(self.peek()? == c, "STEP expected '{}' at byte {}", c as char, self.at);
        self.at += 1;
        Ok(())
    }
    fn arg(&mut self) -> Result<Arg> {
        match self.peek()? {
            b'#' => {
                self.at += 1;
                Ok(Arg::Ref(self.take_while(|c| c.is_ascii_digit()).parse()?))
            }
            b'\'' => {
                self.at += 1;
                let mut text = Vec::new();
                loop {
                    let c = *self.s.get(self.at).context("STEP text runs to the end")?;
                    self.at += 1;
                    if c == b'\'' {
                        if self.s.get(self.at) != Some(&b'\'') {
                            break;
                        }
                        self.at += 1;
                    }
                    text.push(c);
                }
                Ok(Arg::Text(String::from_utf8_lossy(&text).into_owned()))
            }
            b'(' => {
                self.at += 1;
                let mut items = Vec::new();
                if self.peek()? == b')' {
                    self.at += 1;
                    return Ok(Arg::List(items));
                }
                loop {
                    items.push(self.arg()?);
                    match self.peek()? {
                        b',' => self.at += 1,
                        b')' => {
                            self.at += 1;
                            return Ok(Arg::List(items));
                        }
                        c => anyhow::bail!("STEP list holds '{}' at byte {}", c as char, self.at),
                    }
                }
            }
            b'.' => {
                self.at += 1;
                self.take_while(|c| c != b'.');
                self.expect(b'.')?;
                Ok(Arg::Other)
            }
            b'$' | b'*' => {
                self.at += 1;
                Ok(Arg::Other)
            }
            c if c == b'-' || c == b'+' || c.is_ascii_digit() => Ok(Arg::Number(self.take_while(|c| c.is_ascii_digit() || b"+-.eE".contains(&c)).parse()?)),
            c if c.is_ascii_alphabetic() => {
                self.take_while(|c| c.is_ascii_alphanumeric() || c == b'_');
                self.arg()?;
                Ok(Arg::Other)
            }
            c => anyhow::bail!("STEP holds '{}' at byte {}", c as char, self.at),
        }
    }
}
/// Every solid a STEP file's data section holds, in record order: analytic ones counted by face, faceted ones read back as meshes.
pub fn read_solids(text: &str) -> Result<Vec<Found>> {
    let data = text.find("DATA;").context("STEP file has no data section")? + "DATA;".len();
    let mut c = Cursor { s: text.as_bytes(), at: data };
    let mut records: HashMap<usize, (String, Vec<Arg>)> = HashMap::new();
    let mut order = Vec::new();
    while c.peek()? == b'#' {
        c.at += 1;
        let id: usize = c.take_while(|c| c.is_ascii_digit()).parse()?;
        c.expect(b'=')?;
        let record = if c.peek()? == b'(' {
            // A complex entity: typed parts side by side, which no solid is.
            c.at += 1;
            while c.peek()? != b')' {
                c.take_while(|c| c.is_ascii_alphanumeric() || c == b'_');
                c.arg()?;
            }
            c.at += 1;
            (String::new(), Vec::new())
        } else {
            let kind = c.take_while(|c| c.is_ascii_alphanumeric() || c == b'_').to_string();
            let Arg::List(args) = c.arg()? else { anyhow::bail!("STEP record #{id} has no parameters") };
            (kind, args)
        };
        c.expect(b';')?;
        records.insert(id, record);
        order.push(id);
    }
    let get = |id: Option<usize>, kind: &str| -> Result<&Vec<Arg>> {
        let id = id.context("STEP expected a reference")?;
        let (k, args) = records.get(&id).with_context(|| format!("STEP names a missing record #{id}"))?;
        ensure!(k == kind, "STEP record #{id} is {k}, not {kind}");
        Ok(args)
    };
    let mut found = Vec::new();
    for id in order {
        let (kind, args) = &records[&id];
        let faceted = match kind.as_str() {
            "FACETED_BREP" => true,
            "MANIFOLD_SOLID_BREP" | "BREP_WITH_VOIDS" => false,
            _ => continue,
        };
        let name = match args.first() {
            Some(Arg::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let faces = get(args.get(1).and_then(Arg::reference), "CLOSED_SHELL")?.get(1).map(Arg::list).unwrap_or_default();
        let mesh = if faceted {
            let mut mesh = Mesh::default();
            let mut index: HashMap<usize, u32> = HashMap::new();
            for face in faces {
                let surface = get(face.reference(), "FACE_SURFACE")?;
                let bound = get(surface.get(1).and_then(|b| b.list().first()).and_then(Arg::reference), "FACE_OUTER_BOUND")?;
                let polygon = get(bound.get(1).and_then(Arg::reference), "POLY_LOOP")?;
                let mut corners = Vec::new();
                for p in polygon.get(1).map(Arg::list).unwrap_or_default() {
                    let point = p.reference().context("A poly loop names a point by reference")?;
                    let v = match index.get(&point) {
                        Some(v) => *v,
                        None => {
                            let xyz: Vec<f64> = get(Some(point), "CARTESIAN_POINT")?.get(1).map(Arg::list).unwrap_or_default().iter().filter_map(|a| if let Arg::Number(n) = a { Some(*n) } else { None }).collect();
                            ensure!(xyz.len() == 3, "STEP point #{point} is not three numbers");
                            mesh.vertices.push(crate::mesh::Vec3(xyz[0] as f32, xyz[1] as f32, xyz[2] as f32));
                            index.insert(point, mesh.vertices.len() as u32 - 1);
                            mesh.vertices.len() as u32 - 1
                        }
                    };
                    corners.push(v);
                }
                ensure!(corners.len() >= 3, "A STEP facet has {} corners", corners.len());
                for k in 1..corners.len() - 1 {
                    mesh.faces.push([corners[0], corners[k], corners[k + 1]]);
                }
            }
            Some(mesh)
        } else {
            None
        };
        found.push(Found { name, faceted, faces: faces.len(), mesh });
    }
    Ok(found)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_and_surfaces_are_exported_as_geometry_not_triangles() {
        let d = super::super::examples::design("two-part-signet").unwrap();
        let e = super::super::evaluate(
            &d,
            &crate::AlphaLibrary::builtin(),
            crate::BuildParams::default(),
        )
        .unwrap();
        let text = export(&e, "Shop's signet").unwrap();
        assert!(text.contains("TOROIDAL_SURFACE"));
        assert!(text.contains("ADVANCED_FACE"));
        assert!(text.contains("Shop''s signet"));
        assert!(!text.contains("TRIANGULATED_FACE_SET"));
        // The reader finds both parts, analytic, and the file keeps its one representation.
        let solids = read_solids(&text).unwrap();
        assert_eq!(solids.iter().map(|s| (s.name.as_str(), s.faceted)).collect::<Vec<_>>(), [("Shank", false), ("Separate signet head", false)]);
        assert!(solids.iter().all(|s| s.faces > 0 && s.mesh.is_none()));
        assert!(!text.contains("FACETED_BREP") && !text.contains("SHAPE_REPRESENTATION_RELATIONSHIP"));
    }

    /// The claw solitaire with a post joined to the band and a spacer kept beside it.
    fn solitaire_with_parts() -> RingDesign {
        use crate::cad::{Component, Feature, Operation, Placement};
        let mut d = super::super::examples::design("claw-solitaire").unwrap();
        let doc = d.cad.as_mut().unwrap();
        let post = Component { attach: Attach::Join, placement: Placement::ring(200.0, 0.6), ..Component::default() };
        doc.append(Feature { id: 5, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.0 }, component: post }).unwrap();
        let spacer = Component { placement: Placement::ring(270.0, 2.0), ..Component::default() };
        doc.append(Feature { id: 6, name: "Spacer".into(), enabled: true, operation: Operation::Box { size: [1.5, 1.5, 1.5] }, component: spacer }).unwrap();
        d
    }

    #[test]
    fn the_whole_ring_carries_its_kernel_parts_analytic_and_its_band_faceted() {
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        let d = solitaire_with_parts();
        let text = ring(&d, &lib, params, "Claw solitaire").unwrap();
        assert_eq!(text, ring(&d, &lib, params, "Claw solitaire").unwrap(), "the same ring writes the same file");
        let count = |entity: &str| text.matches(&format!("={entity}(")).count();
        // The post and the spacer are analytic; the band, with its head joined and its seat cut, is one faceted shell; the stone is not metal.
        assert_eq!((count("MANIFOLD_SOLID_BREP"), count("FACETED_BREP"), count("CLOSED_SHELL")), (2, 1, 3));
        assert_eq!((count("SHAPE_REPRESENTATION"), count("ADVANCED_BREP_SHAPE_REPRESENTATION"), count("FACETED_BREP_SHAPE_REPRESENTATION")), (1, 1, 1));
        assert_eq!((count("SHAPE_DEFINITION_REPRESENTATION"), count("SHAPE_REPRESENTATION_RELATIONSHIP")), (1, 2));
        let solids = read_solids(&text).unwrap();
        assert_eq!(
            solids.iter().map(|s| (s.name.as_str(), s.faceted, s.faces)).collect::<Vec<_>>(),
            [("Post", false, 3), ("Spacer", false, 6), ("Claw solitaire", true, count("FACE_SURFACE"))]
        );
        // Read back, the faceted band is closed and holds exactly the metal the build put in it.
        let band = solids[2].mesh.as_ref().unwrap();
        let v = band.validate();
        assert!(v.watertight, "{v:?}");
        let mut apart = d.clone();
        apart.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 5).unwrap().component.attach = Attach::Separate;
        let built = crate::mesh::try_build(&apart, &lib, params).unwrap();
        let object = &crate::threemf::objects(&built, "Claw solitaire")[0];
        assert_eq!(band.faces.len(), object.mesh.faces.len());
        assert!((band.volume_mm3() - object.mesh.volume_mm3()).abs() < 1e-6 * object.mesh.volume_mm3(), "{} against {}", band.volume_mm3(), object.mesh.volume_mm3());
        assert_eq!(count("FACE_SURFACE"), count("POLY_LOOP"));
        // A ring of parts only keeps every kernel part analytic and nothing faceted but what a builder made.
        let gallery = super::super::examples::design("gallery").unwrap();
        let text = ring(&gallery, &lib, params, "Gallery").unwrap();
        assert_eq!(text, export(&super::super::evaluate(&gallery, &lib, params).unwrap(), "Gallery").unwrap());
    }
}
