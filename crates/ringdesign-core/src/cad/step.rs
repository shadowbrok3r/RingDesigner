//! STEP AP214 B-rep export. Analytic curves/surfaces stay analytic; faceted
//! source features remain planar faces. No triangle mesh is substituted for
//! an unsupported surface. The source graph remains the editable project.
use super::Evaluated;
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
}
pub fn export(e: &Evaluated, name: &str) -> Result<String> {
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
    for c in e.components.iter().filter(|c| !c.settings.reference && c.made.is_none()) {
        bodies.extend(
            w.body(&c.body, &c.name)
                .with_context(|| format!("STEP component #{} {}", c.id, c.name))?,
        );
    }
    ensure!(!bodies.is_empty(), "No metal solids to export");
    let rep = w.add(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION({}, {},#{geometry})",
        label(name),
        refs(&bodies)
    ));
    w.add(format!("SHAPE_DEFINITION_REPRESENTATION(#{shape},#{rep})"));
    // Leave the optional timestamp unspecified for reproducible exports.
    let mut out = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('RingDesigner analytic feature export'),'2;1');\nFILE_NAME({},'',(''),(''),'RingDesigner','RingDesigner','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n",
        label(name)
    );
    for (index, record) in w.records.into_iter().enumerate() {
        out.push_str(&format!("#{}={record};\n", index + 1));
    }
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(out)
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
    }
}
