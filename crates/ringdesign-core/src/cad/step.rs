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
mod decimate;
mod lift;

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
    ring_built(design, lib, params, name, None).map(|w| w.text)
}
/// What a ring's STEP file holds beside its text.
struct Written {
    text: String,
    /// Faceted triangles written.
    facets: usize,
    /// Metal the faceted solids enclose, mm³.
    volume_mm3: f64,
    /// Triangles of the band's faceted solid as written, with the seats, stamps and parts it carries.
    band_facets: usize,
    /// When the band's solid was collapsed: its triangles and vertices before, and the farthest of those vertices from it, mm.
    collapsed: Option<(usize, usize, f64)>,
}
/// The band's solid collapsed to within `tolerance_mm` of every vertex it had, and what it was; `None` when no collapse held.
fn collapsed(solid: &Mesh, tolerance_mm: f64) -> Option<(Mesh, (usize, usize, f64))> {
    let out = decimate::decimated(solid, tolerance_mm)?;
    let far = deviation_mm(solid, &out, DEVIATION_REACH_MM);
    Some((out, (solid.faces.len(), solid.vertices.len(), far)))
}
/// [`ring`] with what it wrote faceted, the band's solid collapsed to within `collapse` mm of every vertex it had when asked.
fn ring_built(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams, name: &str, collapse: Option<f64>) -> Result<Written> {
    let metal = |c: &EvaluatedComponent| !c.settings.reference;
    let written = |text: String, faceted: &[Faceted], band_facets: usize, collapsed: Option<(usize, usize, f64)>| Written {
        text,
        facets: faceted.iter().map(|f| f.mesh.faces.len()).sum(),
        volume_mm3: faceted.iter().map(|f| f.mesh.volume_mm3()).sum(),
        band_facets,
        collapsed,
    };
    let Some(doc) = &design.cad else {
        let built = crate::mesh::try_build(design, lib, params)?;
        let none = Evaluated { components: Vec::new(), features: Vec::new(), band: None, planes: Vec::new() };
        let small = collapse.and_then(|tol| collapsed(&built.mesh, tol));
        let solid = small.as_ref().map_or(&built.mesh, |(m, _)| m);
        let faceted = [Faceted { name: name.to_string(), mesh: solid }];
        let text = export_with(&none, name, &metal, &faceted)?;
        return Ok(written(text, &faceted, solid.faces.len(), small.as_ref().map(|(_, c)| *c)));
    };
    if doc.replaces_band() {
        let e = super::evaluate(design, lib, params)?;
        let made: Vec<Faceted> = e.components.iter().filter(|c| metal(c) && c.made.is_some()).map(|c| Faceted { name: c.name.clone(), mesh: &c.mesh }).collect();
        return Ok(written(export_with(&e, name, &metal, &made)?, &made, 0, None));
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
    // The band's own object, with its joined and cut parts, is the one no feature names.
    let band = objects.iter().find(|o| o.feature.is_none());
    let small = band.zip(collapse).and_then(|(o, tol)| collapsed(o.mesh.as_ref(), tol));
    let faceted: Vec<Faceted> = objects
        .iter()
        .filter(|o| o.feature.is_none_or(made))
        .map(|o| Faceted { name: o.name.clone(), mesh: match (&small, o.feature) { (Some((m, _)), None) => m, _ => o.mesh.as_ref() } })
        .collect();
    let band_facets = small.as_ref().map(|(m, _)| m.faces.len()).or(band.map(|o| o.mesh.faces.len())).unwrap_or(0);
    let text = export_with(e, name, &|c| metal(c) && c.attach == Attach::Separate, &faceted)?;
    Ok(written(text, &faceted, band_facets, small.as_ref().map(|(_, c)| *c)))
}
/// How near a STEP file's faceted band stands to every vertex of the export build, mm.
pub const BAND_TOLERANCE_MM: f64 = 0.01;
/// How far out a vertex's nearest facet is looked for, mm; one farther reads as this.
const DEVIATION_REACH_MM: f64 = 1.0;
/// How a sized STEP file's band stands to the build it came from.
#[derive(Clone, Copy, Debug)]
pub struct BandFacets {
    /// The build the band comes from: the export build, less a refinement an imported base cannot take.
    pub params: BuildParams,
    /// Triangles of the band's solid as that build made it, with the seats, stamps and parts it carries.
    pub built: usize,
    /// Triangles of it written: collapsed to the tolerance, or as built when no collapse held.
    pub written: usize,
    /// The farthest any vertex of the built solid stands from the written one, mm.
    pub deviation_mm: f64,
    /// Vertices it was measured at.
    pub samples: usize,
}
/// A ring written as STEP with its band's facets sized.
pub struct Sized {
    pub text: String,
    /// How the band was sized; `None` for a ring of parts alone.
    pub band: Option<BandFacets>,
    /// Faceted triangles the file carries: the band with the parts joined to it, and every faceted part.
    pub facets: usize,
    /// Metal the faceted solids enclose, mm³.
    pub faceted_volume_mm3: f64,
}
impl Sized {
    /// Exact solids and faceted solids the file holds.
    pub fn solids(&self) -> (usize, usize) {
        let exact = self.text.matches("=MANIFOLD_SOLID_BREP(").count() + self.text.matches("=BREP_WITH_VOIDS(").count();
        (exact, self.text.matches("=FACETED_BREP(").count())
    }
    /// The file's solids, its size and its band, as the desktop's status line says them.
    pub fn summary(&self) -> String {
        let (exact, faceted) = self.solids();
        format!("{exact} exact and {faceted} faceted solid{} • {} • {}", if exact + faceted == 1 { "" } else { "s" }, size_words(self.text.len()), band_words(self.band.as_ref()))
    }
}
/// A file's size in the unit that reads.
pub fn size_words(bytes: usize) -> String {
    if bytes >= 1 << 20 { format!("{:.1} MB", bytes as f64 / 1048576.0) } else { format!("{:.1} KB", bytes as f64 / 1024.0) }
}
/// `n` with its thousands grouped.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
/// What a STEP file's band holds: its facets, and how near every vertex of the export build stands to them.
pub fn band_words(band: Option<&BandFacets>) -> String {
    match band {
        Some(b) if b.written < b.built => format!("band {} facets from {}, every vertex of the export build within {:.3} mm", grouped(b.written), grouped(b.built), b.deviation_mm),
        Some(b) => format!("band {} facets as the export build made them", grouped(b.written)),
        None => "no band: every part as it was built".into(),
    }
}
/// The farthest any vertex of `truth` stands from the faces of `mesh`, mm, looked for out to `reach`.
fn deviation_mm(truth: &Mesh, mesh: &Mesh, reach: f64) -> f64 {
    let bvh = crate::interaction::bvh::Bvh::build(mesh);
    let far = |v: &crate::mesh::Vec3| {
        let p = [v.0 as f64, v.1 as f64, v.2 as f64];
        bvh.nearest(mesh, p, reach).map_or(reach, |(_, q)| crate::interaction::bvh::dist2(p, q).sqrt())
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        truth.vertices.par_iter().map(far).reduce(|| 0.0, f64::max)
    }
    #[cfg(not(feature = "parallel"))]
    truth.vertices.iter().map(far).fold(0.0, f64::max)
}
/// [`ring`] at `export` with the band's solid collapsed while every vertex it had stays within `tolerance_mm` of what is
/// written; an imported base, which cannot refine, at `export`'s sweep. A ring of parts alone is [`ring`] at `export`.
pub fn ring_sized(design: &RingDesign, lib: &AlphaLibrary, export: BuildParams, tolerance_mm: f64, name: &str) -> Result<Sized> {
    let params = if design.imported_base.is_some() { BuildParams { refine: None, ..export } } else { export };
    let procedural = design.band_is_procedural();
    let w = ring_built(design, lib, params, name, procedural.then_some(tolerance_mm))?;
    let band = procedural.then(|| match w.collapsed {
        Some((built, samples, far)) => BandFacets { params, built, written: w.band_facets, deviation_mm: far, samples },
        None => BandFacets { params, built: w.band_facets, written: w.band_facets, deviation_mm: 0.0, samples: 0 },
    });
    Ok(Sized { text: w.text, band, facets: w.facets, faceted_volume_mm3: w.volume_mm3 })
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
    /// An enumeration or a logical, named without its dots: `T`, `F`, `MILLI`.
    Enum(String),
    List(Vec<Arg>),
    /// A typed value such as `LENGTH_MEASURE(0.1)`: what it holds.
    Typed(Box<Arg>),
    /// `$` or `*`.
    Other,
}
impl Arg {
    fn reference(&self) -> Option<usize> {
        if let Arg::Ref(r) = self { Some(*r) } else { None }
    }
    fn list(&self) -> &[Arg] {
        if let Arg::List(v) = self { v } else { &[] }
    }
    /// A number, bare or typed.
    fn number(&self) -> Option<f64> {
        match self {
            Arg::Number(n) => Some(*n),
            Arg::Typed(inner) => inner.number(),
            Arg::List(items) if items.len() == 1 => items[0].number(),
            _ => None,
        }
    }
    fn logical(&self) -> Option<bool> {
        match self {
            Arg::Enum(e) if e == "T" => Some(true),
            Arg::Enum(e) if e == "F" => Some(false),
            _ => None,
        }
    }
}
/// A record of a STEP data section: a simple entity's type and parameters, or a complex entity's typed parts side by side.
#[derive(Clone, Debug)]
struct Record {
    kind: String,
    args: Vec<Arg>,
    parts: Vec<(String, Vec<Arg>)>,
}
struct Cursor<'a> {
    s: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn skip(&mut self) {
        loop {
            while self.s.get(self.at).is_some_and(|c| c.is_ascii_whitespace()) {
                self.at += 1;
            }
            // A comment, /* … */, reads as space.
            if self.s.get(self.at) != Some(&b'/') || self.s.get(self.at + 1) != Some(&b'*') {
                return;
            }
            self.at = self.s[self.at + 2..].windows(2).position(|w| w == b"*/").map_or(self.s.len(), |p| self.at + 2 + p + 2);
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
                let name = self.take_while(|c| c != b'.').to_string();
                self.expect(b'.')?;
                Ok(Arg::Enum(name))
            }
            b'$' | b'*' => {
                self.at += 1;
                Ok(Arg::Other)
            }
            c if c == b'-' || c == b'+' || c.is_ascii_digit() => Ok(Arg::Number(self.take_while(|c| c.is_ascii_digit() || b"+-.eE".contains(&c)).parse()?)),
            c if c.is_ascii_alphabetic() => {
                self.take_while(|c| c.is_ascii_alphanumeric() || c == b'_');
                Ok(Arg::Typed(Box::new(self.arg()?)))
            }
            c => anyhow::bail!("STEP holds '{}' at byte {}", c as char, self.at),
        }
    }
}
/// Every record of a STEP file's data section by id, and the ids in the order they are written.
fn parse(text: &str) -> Result<(HashMap<usize, Record>, Vec<usize>)> {
    let data = text.find("DATA;").context("STEP file has no data section")? + "DATA;".len();
    let mut c = Cursor { s: text.as_bytes(), at: data };
    let mut records: HashMap<usize, Record> = HashMap::new();
    let mut order = Vec::new();
    while c.peek()? == b'#' {
        c.at += 1;
        let id: usize = c.take_while(|c| c.is_ascii_digit()).parse()?;
        c.expect(b'=')?;
        let record = if c.peek()? == b'(' {
            c.at += 1;
            let mut parts = Vec::new();
            while c.peek()? != b')' {
                let kind = c.take_while(|c| c.is_ascii_alphanumeric() || c == b'_').to_string();
                ensure!(!kind.is_empty(), "STEP record #{id} holds a part with no type");
                let Arg::List(args) = c.arg()? else { anyhow::bail!("STEP record #{id} has a part with no parameters") };
                parts.push((kind, args));
            }
            c.at += 1;
            Record { kind: String::new(), args: Vec::new(), parts }
        } else {
            let kind = c.take_while(|c| c.is_ascii_alphanumeric() || c == b'_').to_string();
            let Arg::List(args) = c.arg()? else { anyhow::bail!("STEP record #{id} has no parameters") };
            Record { kind, args, parts: Vec::new() }
        };
        c.expect(b';')?;
        records.insert(id, record);
        order.push(id);
    }
    Ok((records, order))
}
/// The parameters of simple record `id`, refused unless it is a `kind`.
fn args_of<'a>(records: &'a HashMap<usize, Record>, id: Option<usize>, kind: &str) -> Result<&'a [Arg]> {
    let id = id.context("STEP expected a reference")?;
    let r = records.get(&id).with_context(|| format!("STEP names a missing record #{id}"))?;
    ensure!(r.kind == kind, "STEP record #{id} is {}, not {kind}", if r.kind.is_empty() { "a complex entity" } else { &r.kind });
    Ok(&r.args)
}
/// A faceted solid's facets as one mesh, points shared by record and lengths at `mm` millimetres per unit.
fn facets(records: &HashMap<usize, Record>, faces: &[Arg], mm: f64) -> Result<Mesh> {
    let mut mesh = Mesh::default();
    let mut index: HashMap<usize, u32> = HashMap::new();
    for face in faces {
        let surface = args_of(records, face.reference(), "FACE_SURFACE")?;
        let bound = args_of(records, surface.get(1).and_then(|b| b.list().first()).and_then(Arg::reference), "FACE_OUTER_BOUND")?;
        let polygon = args_of(records, bound.get(1).and_then(Arg::reference), "POLY_LOOP")?;
        let mut corners = Vec::new();
        for p in polygon.get(1).map(Arg::list).unwrap_or_default() {
            let point = p.reference().context("A poly loop names a point by reference")?;
            let v = match index.get(&point) {
                Some(v) => *v,
                None => {
                    let xyz: Vec<f64> = args_of(records, Some(point), "CARTESIAN_POINT")?.get(1).map(Arg::list).unwrap_or_default().iter().filter_map(Arg::number).collect();
                    ensure!(xyz.len() == 3, "STEP point #{point} is not three numbers");
                    mesh.vertices.push(crate::mesh::Vec3((xyz[0] * mm) as f32, (xyz[1] * mm) as f32, (xyz[2] * mm) as f32));
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
    Ok(mesh)
}
/// Every solid a STEP file's data section holds, in record order: analytic ones counted by face, faceted ones read back as meshes.
pub fn read_solids(text: &str) -> Result<Vec<Found>> {
    let (records, order) = parse(text)?;
    let mut found = Vec::new();
    for id in order {
        let r = &records[&id];
        let faceted = match r.kind.as_str() {
            "FACETED_BREP" => true,
            "MANIFOLD_SOLID_BREP" | "BREP_WITH_VOIDS" => false,
            _ => continue,
        };
        let name = match r.args.first() {
            Some(Arg::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let faces = args_of(&records, r.args.get(1).and_then(Arg::reference), "CLOSED_SHELL")?.get(1).map(Arg::list).unwrap_or_default();
        let mesh = if faceted { Some(facets(&records, faces, 1.0)?) } else { None };
        found.push(Found { name, faceted, faces: faces.len(), mesh });
    }
    Ok(found)
}
/// Millimetres per length unit and radians per plane-angle unit, as the file's unit records declare them.
fn units(records: &HashMap<usize, Record>) -> (f64, f64) {
    let (mut mm, mut rad) = (1.0, 1.0);
    for r in records.values() {
        let part = |kind: &str| r.parts.iter().find(|(k, _)| k == kind).map(|(_, a)| a.as_slice());
        let si = part("SI_UNIT").map(|a| match a.first() {
            Some(Arg::Enum(prefix)) => prefix.to_ascii_uppercase(),
            _ => String::new(),
        });
        let named = part("CONVERSION_BASED_UNIT").and_then(|a| match a.first() {
            Some(Arg::Text(t)) => Some(t.to_ascii_lowercase()),
            _ => None,
        });
        if part("LENGTH_UNIT").is_some() {
            mm = match (si.as_deref(), named.as_deref()) {
                (Some("MILLI"), _) => 1.0,
                (Some("CENTI"), _) => 10.0,
                (Some("DECI"), _) => 100.0,
                (Some("MICRO"), _) => 1e-3,
                (Some(""), _) => 1000.0,
                (_, Some("inch")) => 25.4,
                (_, Some("foot")) => 304.8,
                _ => mm,
            };
        }
        if part("PLANE_ANGLE_UNIT").is_some() && named.as_deref().is_some_and(|n| n.starts_with("degree")) {
            rad = std::f64::consts::PI / 180.0;
        }
    }
    (mm, rad)
}
/// A solid a STEP file holds, read back: a closed mesh in millimetres, or why only OpenCascade reads it.
#[derive(Clone, Debug)]
pub struct Meshed {
    pub name: String,
    /// Written as facets, rather than as an exact B-rep.
    pub faceted: bool,
    pub mesh: std::result::Result<Mesh, String>,
}
/// An exact body tessellated at the part chord, its faces wound out.
fn exact_mesh(body: &Body) -> Result<Mesh> {
    let (mut mesh, _) = super::tessellate_traced(body, super::EXPORT_CHORD_MM).map_err(|e| anyhow::anyhow!("cadkernel could not tessellate it: {e}"))?;
    let signed: f64 = mesh.faces.iter().filter_map(|f| mesh.triangle(f)).map(|(a, b, c)| a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])).sum();
    if signed < 0.0 {
        for f in &mut mesh.faces {
            f.swap(1, 2);
        }
        mesh.normals = super::normals(&mesh);
    }
    Ok(mesh)
}
/// Every solid a STEP file holds, in record order, as a closed mesh: faceted ones as written, exact ones through cadkernel at [`super::EXPORT_CHORD_MM`], or why not.
pub fn read_meshes(text: &str) -> Result<Vec<Meshed>> {
    let (records, order) = parse(text)?;
    let (mm, rad) = units(&records);
    let mut out = Vec::new();
    for id in order {
        let r = &records[&id];
        let faceted = match r.kind.as_str() {
            "FACETED_BREP" => true,
            "MANIFOLD_SOLID_BREP" | "BREP_WITH_VOIDS" => false,
            _ => continue,
        };
        let name = match r.args.first() {
            Some(Arg::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let mesh = if faceted {
            let faces = args_of(&records, r.args.get(1).and_then(Arg::reference), "CLOSED_SHELL")?.get(1).map(Arg::list).unwrap_or_default();
            Ok(welded(&facets(&records, faces, mm)?))
        } else {
            lift::body(&records, id, mm, rad).and_then(|b| exact_mesh(&b)).map_err(|why| format!("{why:#}"))
        };
        out.push(Meshed { name, faceted, mesh });
    }
    Ok(out)
}
/// The solids a STEP file holds as closed meshes and a line for each left for OpenCascade, refused by name when none reads: what a part import takes.
pub fn solid_meshes(text: &str, file: &str) -> Result<(Vec<Mesh>, Vec<String>)> {
    let solids = read_meshes(text).with_context(|| format!("{file} does not read as STEP"))?;
    let named = |n: &str| if n.is_empty() { "unnamed".to_string() } else { n.to_string() };
    let left: Vec<(String, &str)> = solids.iter().filter_map(|s| s.mesh.as_ref().err().map(|why| (named(&s.name), why.as_str()))).collect();
    let meshes: Vec<Mesh> = solids.iter().filter_map(|s| s.mesh.as_ref().ok().cloned()).collect();
    let mut why: Vec<&str> = left.iter().map(|(_, w)| *w).collect();
    why.sort_unstable();
    why.dedup();
    ensure!(
        !meshes.is_empty(),
        "{file} holds no solid that reads without OpenCascade{}",
        if left.is_empty() {
            String::new()
        } else {
            format!("; its {} exact solid(s) ({}) read only where OpenCascade is: {}", left.len(), left.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "), why.join("; "))
        }
    );
    let notes = left.iter().map(|(n, why)| format!("{file}: the exact solid {n} reads only where OpenCascade is ({why}), and was left out")).collect();
    Ok((meshes, notes))
}
/// The faceted solids a STEP file holds as welded meshes and a line for each exact solid left for OpenCascade; refused by name when none.
pub fn faceted_meshes(text: &str, file: &str) -> Result<(Vec<Mesh>, Vec<String>)> {
    let solids = read_solids(text).with_context(|| format!("{file} does not read as STEP"))?;
    let exact: Vec<String> = solids.iter().filter(|s| !s.faceted).map(|s| if s.name.is_empty() { "unnamed".to_string() } else { s.name.clone() }).collect();
    let meshes: Vec<Mesh> = solids.into_iter().filter_map(|s| s.mesh).map(|m| welded(&m)).collect();
    ensure!(
        !meshes.is_empty(),
        "{file} holds no faceted solid; its {} exact solid(s) ({}) read only where OpenCascade is",
        exact.len(),
        exact.join(", ")
    );
    let notes = exact.iter().map(|name| format!("{file}: the exact solid {name} reads only where OpenCascade is, and was left out")).collect();
    Ok((meshes, notes))
}

/// `mesh` with every corner at one position made one vertex.
fn welded(mesh: &Mesh) -> Mesh {
    let mut index: HashMap<[u32; 3], u32> = HashMap::new();
    let mut out = Mesh::default();
    let remap: Vec<u32> = mesh
        .vertices
        .iter()
        .map(|v| {
            *index.entry([v.0.to_bits(), v.1.to_bits(), v.2.to_bits()]).or_insert_with(|| {
                out.vertices.push(*v);
                out.vertices.len() as u32 - 1
            })
        })
        .collect();
    out.faces = mesh.faces.iter().map(|f| f.map(|i| remap[i as usize])).filter(|f| f[0] != f[1] && f[1] != f[2] && f[0] != f[2]).collect();
    out
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

    #[test]
    fn a_ring_written_as_step_imports_back_as_its_faceted_solids() {
        use crate::cad::{Component, Document, Feature, Operation, Placement};
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let post = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.9), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.0 }, component: post }).unwrap();
        d.cad = Some(doc);
        let text = ring(&d, &lib, params, "Court").unwrap();
        // The band comes back faceted and closed, holding what the build put in it; the exact post waits for OpenCascade.
        let (meshes, notes) = faceted_meshes(&text, "court.step").unwrap();
        assert_eq!(notes, ["court.step: the exact solid Post reads only where OpenCascade is, and was left out"]);
        assert_eq!(meshes.len(), 1);
        assert!(meshes[0].validate().watertight);
        let mut apart = d.clone();
        apart.cad.as_mut().unwrap().features[1].component.attach = Attach::Separate;
        let built = crate::mesh::try_build(&apart, &lib, params).unwrap();
        let band = crate::threemf::objects(&built, "Court").remove(0);
        let (read, want) = (meshes[0].volume_mm3(), band.mesh.volume_mm3());
        assert!((read - want).abs() < 5e-3 * want, "{read} against {want}");
        // Imported, it packs as one closed part of the same metal.
        let f = super::super::stored::imported("court.step", "step", &meshes).unwrap();
        let Operation::Stored { mesh, .. } = &f.operation else { panic!() };
        let packed = mesh.made().unwrap().solid().volume();
        eprintln!("court STEP back: {read:.4} mm³ faceted against {want:.4} built, {packed:.4} packed");
        assert!((packed - want).abs() < 5e-3 * want, "{packed} against {want}");
        // A file of exact solids only is refused by name.
        let gallery = super::super::examples::design("gallery").unwrap();
        let exact = export(&super::super::evaluate(&gallery, &lib, params).unwrap(), "Gallery").unwrap();
        let refused = faceted_meshes(&exact, "gallery.step").unwrap_err().to_string();
        assert!(refused.starts_with("gallery.step holds no faceted solid; its 6 exact solid(s) (Gallery lower ring, Raise upper ring, Gallery strut 1,"), "{refused}");
        assert!(refused.ends_with(") read only where OpenCascade is"), "{refused}");
    }

    #[test]
    fn every_exact_part_our_writer_makes_reads_back_through_cadkernel() {
        let lib = crate::AlphaLibrary::builtin();
        // 512 steps round the ring tessellates parts at the export chord, the one the reader reads at.
        let params = crate::BuildParams::default();
        let mut parts = 0;
        for name in super::super::examples::NAMES {
            let e = super::super::evaluate(&super::super::examples::design(name).unwrap(), &lib, params).unwrap();
            let Ok(text) = export(&e, name) else { continue };
            let started = std::time::Instant::now();
            let read = read_meshes(&text).unwrap();
            let ms = started.elapsed().as_secs_f64() * 1e3;
            for s in &read {
                let mesh = s.mesh.as_ref().unwrap_or_else(|why| panic!("{name}, {}: {why}", s.name));
                let c = e.components.iter().find(|c| c.name == s.name).unwrap();
                let (got, want) = (mesh.volume_mm3(), c.mesh.volume_mm3());
                eprintln!("{name}, {}: {got:.4} mm³ read back against {want:.4} built ({:+.4}%), {} B-rep faces", s.name, (got / want - 1.0) * 100.0, c.body.faces.len());
                assert!(!s.faceted && mesh.validate().watertight, "{name}, {}", s.name);
                assert!((got / want - 1.0).abs() < 0.005, "{name}, {}: {got} against {want}", s.name);
                parts += 1;
            }
            eprintln!("{name}: {} solids read back in {ms:.1} ms from {:.0} KB", read.len(), text.len() as f64 / 1024.0);
        }
        assert!(parts >= 8, "{parts} parts read back");
    }

    #[test]
    fn a_ring_exported_as_step_imports_whole_without_opencascade() {
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        let d = solitaire_with_parts();
        let text = ring(&d, &lib, params, "Claw solitaire").unwrap();
        let started = std::time::Instant::now();
        let read = read_meshes(&text).unwrap();
        eprintln!("claw solitaire STEP, {:.0} KB, read back in {:.1} ms", text.len() as f64 / 1024.0, started.elapsed().as_secs_f64() * 1e3);
        assert_eq!(read.iter().map(|s| (s.name.as_str(), s.faceted, s.mesh.is_ok())).collect::<Vec<_>>(), [("Post", false, true), ("Spacer", false, true), ("Claw solitaire", true, true)]);
        let read: Vec<(&str, &Mesh)> = read.iter().map(|s| (s.name.as_str(), s.mesh.as_ref().unwrap())).collect();
        // Every part within half a percent of its own metal: the post and the spacer as built, the band as the build put it in the file.
        let e = super::super::evaluate(&d, &lib, crate::BuildParams::default()).unwrap();
        let built = |name: &str| e.components.iter().find(|c| c.name == name).unwrap().mesh.volume_mm3();
        let mut apart = d.clone();
        apart.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 5).unwrap().component.attach = Attach::Separate;
        let band = crate::threemf::objects(&crate::mesh::try_build(&apart, &lib, params).unwrap(), "Claw solitaire")[0].mesh.volume_mm3();
        for ((name, mesh), want, analytic) in [(read[0], built("Post"), std::f64::consts::PI * 0.64 * 2.0), (read[1], built("Spacer"), 3.375), (read[2], band, band)] {
            let got = mesh.volume_mm3();
            eprintln!("{name}: {got:.4} mm³ read back, {want:.4} built, {analytic:.4} exact");
            assert!(mesh.validate().watertight, "{name}");
            assert!((got / want - 1.0).abs() < 0.005 && (got / analytic - 1.0).abs() < 0.005, "{name}: {got} against {want} and {analytic}");
        }
        // What the import takes: all three, nothing left out, packed as one closed part of the same metal.
        let (meshes, notes) = solid_meshes(&text, "solitaire.step").unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let f = super::super::stored::imported("solitaire.step", "step", &meshes).unwrap();
        let super::super::Operation::Stored { mesh, .. } = &f.operation else { panic!() };
        let whole: f64 = read.iter().map(|(_, m)| m.volume_mm3()).sum();
        let packed = mesh.made().unwrap().solid().volume();
        assert!((packed / whole - 1.0).abs() < 1e-4, "{packed} against {whole}");
        // The faceted-only reader still leaves the exact two out, as it always did.
        let (faceted, notes) = faceted_meshes(&text, "solitaire.step").unwrap();
        assert_eq!((faceted.len(), notes.len()), (1, 2));
    }

    #[test]
    fn a_vendor_b_spline_solid_is_named_not_dropped() {
        use crate::cad::{Component, Document, Feature, Operation};
        let lib = crate::AlphaLibrary::builtin();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Block".into(), enabled: true, operation: Operation::Box { size: [2.0, 3.0, 4.0] }, component: Component::default() }).unwrap();
        doc.append(Feature { id: 2, name: "Pin".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Component::default() }).unwrap();
        let d = RingDesign { cad: Some(doc), ..RingDesign::default() };
        let text = export(&super::super::evaluate(&d, &lib, crate::BuildParams::default()).unwrap(), "Parts").unwrap();
        // The block's first plane rewritten as a vendor writes a spline face: a bilinear patch over four fresh points.
        let at = text.find("=PLANE(").unwrap();
        let id: usize = text[..at].rsplit('#').next().unwrap().parse().unwrap();
        let end = text[at..].find(";\n").unwrap() + at;
        let last: usize = text.lines().filter_map(|l| l.strip_prefix('#')?.split('=').next()?.parse().ok()).max().unwrap();
        let points: String = (1..=4).map(|k| format!("#{}=CARTESIAN_POINT('',({}.,{}.,0.));\n", last + k, k % 2, k / 3)).collect();
        let spline = format!(
            "#{id}=B_SPLINE_SURFACE_WITH_KNOTS('',1,1,((#{},#{}),(#{},#{})),.UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.)",
            last + 1,
            last + 2,
            last + 3,
            last + 4
        );
        let vendor = format!("{}{spline}{}", &text[..text[..at].rfind('#').unwrap()], &text[end..]).replace("ENDSEC;\nEND-ISO", &format!("{points}ENDSEC;\nEND-ISO"));
        let read = read_meshes(&vendor).unwrap();
        assert_eq!(read.iter().map(|s| (s.name.as_str(), s.mesh.as_ref().err().map(String::as_str))).collect::<Vec<_>>(), [("Block", Some("it carries a B-spline surface")), ("Pin", None)]);
        let (meshes, notes) = solid_meshes(&vendor, "vendor.step").unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(notes, ["vendor.step: the exact solid Block reads only where OpenCascade is (it carries a B-spline surface), and was left out"]);
        // With nothing else in the file, the import is refused by the solid's name.
        let only = vendor.replacen("MANIFOLD_SOLID_BREP('Pin'", "UNREAD_SOLID('Pin'", 1);
        let refused = solid_meshes(&only, "vendor.step").unwrap_err().to_string();
        assert_eq!(refused, "vendor.step holds no solid that reads without OpenCascade; its 1 exact solid(s) (Block) read only where OpenCascade is: it carries a B-spline surface");
        // A comment between records reads as space.
        let commented = vendor.replacen("DATA;\n", "DATA;\n/* written by hand */\n", 1);
        assert_eq!(read_meshes(&commented).unwrap().iter().filter(|s| s.mesh.is_ok()).count(), 1);
    }

    /// Zenith as the showcase saved it, its art in the library.
    fn zenith() -> (RingDesign, crate::AlphaLibrary) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/stock-masterworks/zenith/design.ring.json");
        let mut lib = crate::AlphaLibrary::builtin();
        let d = crate::library::load_design(&path).unwrap();
        d.unpack_embedded(&mut lib);
        d.bake_all(&mut lib);
        (d, lib)
    }

    /// The export build's STEP against the sized one, timed: `cargo test --release -p ringdesign-core -- --ignored step_sizes --nocapture`.
    #[test]
    #[ignore]
    fn step_sizes() {
        let export = crate::BuildParams { theta_steps: 1024, profile_steps: 320, refine: None, ..Default::default() };
        let (zenith, zenith_lib) = zenith();
        let braided = crate::templates::all().iter().find(|t| t.name == "Braided band").unwrap().design();
        let rings = [
            ("Claw solitaire", super::super::examples::design("claw-solitaire").unwrap(), crate::AlphaLibrary::builtin()),
            ("Braided band", braided, crate::AlphaLibrary::builtin()),
            ("Zenith", zenith, zenith_lib),
        ];
        let file = std::env::temp_dir().join(format!("step-sizes-{}.step", std::process::id()));
        for (name, d, lib) in &rings {
            let started = std::time::Instant::now();
            let old = ring_built(d, lib, export, name, None).unwrap();
            crate::library::write_atomic(&file, old.text.as_bytes()).unwrap();
            let old_ms = started.elapsed().as_secs_f64() * 1e3;
            eprintln!("{name}, the export build as built: {:.1} MB, {} facets, {old_ms:.0} ms, faceted volume {:.4} mm3", old.text.len() as f64 / 1048576.0, old.facets, old.volume_mm3);
            drop(old);
            for tolerance in [0.02, BAND_TOLERANCE_MM] {
                let started = std::time::Instant::now();
                let sized = ring_sized(d, lib, export, tolerance, name).unwrap();
                crate::library::write_atomic(&file, sized.text.as_bytes()).unwrap();
                let ms = started.elapsed().as_secs_f64() * 1e3;
                let b = sized.band.unwrap();
                eprintln!(
                    "{name}, sized to {tolerance} mm: {:.1} MB, {} facets ({} of the band's {}), {ms:.0} ms, within {:.4} mm at {} points, faceted volume {:.4} mm3",
                    sized.text.len() as f64 / 1048576.0,
                    sized.facets,
                    b.written,
                    b.built,
                    b.deviation_mm,
                    b.samples,
                    sized.faceted_volume_mm3
                );
            }
        }
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn a_sized_step_holds_its_band_within_the_tolerance_of_every_vertex_the_export_build_had() {
        let lib = crate::AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..Default::default() };
        let d = solitaire_with_parts();
        let whole = ring(&d, &lib, params, "Claw solitaire").unwrap();
        let sized = ring_sized(&d, &lib, params, BAND_TOLERANCE_MM, "Claw solitaire").unwrap();
        let b = sized.band.unwrap();
        eprintln!("claw solitaire at 256 x 128: {:.0} KB as built, {:.0} KB sized; the band {} facets of {}, within {:.4} mm at {} points", whole.len() as f64 / 1024.0, sized.text.len() as f64 / 1024.0, b.written, b.built, b.deviation_mm, b.samples);
        // The same solids in the same order; the analytic parts untouched, the band's facets far fewer.
        let (was, now) = (read_solids(&whole).unwrap(), read_solids(&sized.text).unwrap());
        let names = |s: &[Found]| s.iter().map(|s| (s.name.clone(), s.faceted)).collect::<Vec<_>>();
        assert_eq!(names(&now), names(&was));
        assert_eq!((now[0].faces, now[1].faces), (was[0].faces, was[1].faces));
        let (band_was, band_now) = (was[2].mesh.as_ref().unwrap(), now[2].mesh.as_ref().unwrap());
        assert_eq!((b.built, b.written, b.params.theta_steps), (band_was.faces.len(), band_now.faces.len(), 256));
        assert!(b.written * 5 < b.built && sized.text.len() * 5 < whole.len(), "{} of {} facets", b.written, b.built);
        // Every vertex the build had stands within the tolerance of the file's band, measured again from the files.
        assert!(b.samples > 0 && b.deviation_mm <= BAND_TOLERANCE_MM, "{b:?}");
        let far = deviation_mm(band_was, band_now, DEVIATION_REACH_MM);
        assert!(far <= BAND_TOLERANCE_MM + 1e-5, "{far}");
        assert!(band_now.validate().watertight);
        let (v_was, v_now) = (band_was.volume_mm3(), band_now.volume_mm3());
        assert!((v_now / v_was - 1.0).abs() < 0.005, "{v_now} against {v_was}");
        assert!((sized.faceted_volume_mm3 - v_now).abs() < 1e-3 * v_now, "{} against {v_now}", sized.faceted_volume_mm3);
        // What every writer says of it: the post and the spacer exact, the band faceted, the size and the deviation.
        assert_eq!(sized.solids(), (2, 1));
        let said = sized.summary();
        let want = format!("2 exact and 1 faceted solids • {} • band {} facets from {}, every vertex of the export build within {:.3} mm", size_words(sized.text.len()), grouped(b.written), grouped(b.built), b.deviation_mm);
        assert_eq!(said, want);
        assert_eq!((size_words(1536), size_words(3 << 20), grouped(1_234_567), grouped(999)), ("1.5 KB".into(), "3.0 MB".into(), "1,234,567".into(), "999".into()));
        assert_eq!(band_words(None), "no band: every part as it was built");
    }

    #[test]
    fn an_imported_base_is_sized_at_its_sweep_where_a_refined_export_build_is_refused() {
        let mut d = RingDesign::default();
        crate::imported_base::ImportedBase::attach(&mut d, crate::imported_base::PRESETS[1].load().unwrap()).unwrap();
        let lib = crate::AlphaLibrary::builtin();
        let refined = crate::BuildParams { theta_steps: 192, profile_steps: 96, refine: Some(crate::refine::RefineParams::default()), ..Default::default() };
        assert!(ring(&d, &lib, refined, "Stock").is_err(), "an imported base takes no refinement");
        let sized = ring_sized(&d, &lib, refined, BAND_TOLERANCE_MM, "Stock").unwrap();
        let b = sized.band.unwrap();
        assert!(b.params.refine.is_none() && b.params.theta_steps == 192, "{:?}", b.params);
        assert!(b.written * 2 < b.built && b.deviation_mm <= BAND_TOLERANCE_MM, "{b:?}");
        let solids = read_solids(&sized.text).unwrap();
        assert_eq!(solids.iter().map(|s| (s.name.as_str(), s.faceted)).collect::<Vec<_>>(), [("Stock", true)]);
        let mesh = solids[0].mesh.as_ref().unwrap();
        assert!(mesh.validate().watertight && mesh.faces.len() == b.written);
        // A ring of parts alone has no band to size.
        let gallery = super::super::examples::design("gallery").unwrap();
        let parts = ring_sized(&gallery, &lib, crate::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() }, BAND_TOLERANCE_MM, "Gallery").unwrap();
        assert!(parts.band.is_none() && parts.facets == 0);
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
