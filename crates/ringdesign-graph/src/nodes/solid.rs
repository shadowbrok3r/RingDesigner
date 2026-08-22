//! Free-mode solids: the Manifold kernel as nodes. Every node here runs in
//! Free mode only; a SandRing graph refuses them at validation, because a
//! solid has no chart and the field cannot judge it.

use std::sync::Arc;

use ringdesign_core::mesh::BuildParams;
use ringdesign_solid::kernel::{self, Frame, Solid, V3, v3};
use serde::{Deserialize, Serialize};

use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{SolidHandle, Value, ValueKind};

/// A Manifold behind the graph's solid handle.
pub struct ManifoldSolid(pub Solid);

impl SolidHandle for ManifoldSolid {
    fn describe(&self) -> String {
        format!("solid {} tris, {:.2} mm³", self.0.num_tri(), self.0.volume())
    }

    fn to_mesh(&self) -> Option<ringdesign_core::Mesh> {
        Some(kernel::to_mesh(&self.0))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn wrap(s: Solid) -> Value {
    Value::Solid(Arc::new(ManifoldSolid(s)))
}

fn solid_of(i: &Inputs, pin: &str) -> Result<Solid, NodeError> {
    match i.get(pin) {
        Value::Solid(h) => h.as_any().downcast_ref::<ManifoldSolid>().map(|m| m.0.clone()).ok_or_else(|| NodeError::input(pin, "a solid from another kernel")),
        Value::Mesh(m) => kernel::from_mesh(m).map_err(|e| NodeError::input(pin, e.to_string())),
        other => Err(NodeError::input(pin, format!("expected a solid, got {}", other.summary()))),
    }
}

fn segments(i: &Inputs) -> Result<i32, NodeError> {
    let s = i.int("segments")?;
    if !(3..=512).contains(&s) {
        return Err(NodeError::input("segments", format!("{s} is not between 3 and 512")));
    }
    Ok(s as i32)
}

fn positive(i: &Inputs, pin: &str) -> Result<f64, NodeError> {
    let v = i.number(pin)?;
    if !(v > 0.0) || !v.is_finite() {
        return Err(NodeError::input(pin, format!("{v} is not positive")));
    }
    Ok(v)
}

fn points2(i: &Inputs, pin: &str) -> Result<Vec<[f64; 2]>, NodeError> {
    match i.get(pin) {
        Value::Path(p) => Ok((**p).clone()),
        other => Err(NodeError::input(pin, format!("expected a path, got {}", other.summary()))),
    }
}

fn points3(i: &Inputs, pin: &str) -> Result<Vec<V3>, NodeError> {
    let json = i.get(pin).to_json_any().ok_or_else(|| NodeError::input(pin, "expected a list of [x, y, z]"))?;
    let pts: Vec<[f64; 3]> = serde_json::from_value(json).map_err(|e| NodeError::input(pin, format!("expected a list of [x, y, z]: {e}")))?;
    Ok(pts.into_iter().map(|p| v3(p[0], p[1], p[2])).collect())
}

/// A frame as the graph carries it: JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FrameJson {
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    z: [f64; 3],
}

impl FrameJson {
    fn of(f: &Frame) -> Self {
        let a = |v: V3| [v.x, v.y, v.z];
        Self { origin: a(f.origin), x: a(f.x), y: a(f.y), z: a(f.z) }
    }
    fn frame(&self) -> Frame {
        let v = |a: [f64; 3]| v3(a[0], a[1], a[2]);
        Frame { origin: v(self.origin), x: v(self.x), y: v(self.y), z: v(self.z) }
    }
}

fn frame_of(i: &Inputs, pin: &str) -> Result<Frame, NodeError> {
    let json = i.get(pin).to_json_any().ok_or_else(|| NodeError::input(pin, "expected a frame"))?;
    let f: FrameJson = serde_json::from_value(json).map_err(|e| NodeError::input(pin, format!("not a frame: {e}")))?;
    Ok(f.frame())
}

fn cylinder(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let h = positive(i, "height")?;
    let r = positive(i, "radius")?;
    let rt = i.get("radius_top").as_number().unwrap_or(r).max(0.0);
    let s = ringdesign_solid::kernel::manifold3d::Manifold::cylinder(h, r, rt, segments(i)?, i.bool("center")?);
    Ok(Outputs::one("solid", wrap(s)))
}

fn sphere(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(kernel::sphere(positive(i, "radius")?, segments(i)?))))
}

fn box_(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(kernel::cube(positive(i, "x")?, positive(i, "y")?, positive(i, "z")?, i.bool("center")?))))
}

fn extrude(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let pts = points2(i, "points")?;
    if pts.len() < 3 {
        return Err(NodeError::input("points", "a section needs at least three points"));
    }
    let cs = ringdesign_solid::kernel::manifold3d::CrossSection::from_simple_polygon(&pts);
    Ok(Outputs::one("solid", wrap(cs.extrude(positive(i, "height")?))))
}

fn revolve(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let pts = points2(i, "points")?;
    if pts.len() < 3 {
        return Err(NodeError::input("points", "a section needs at least three points"));
    }
    if pts.iter().any(|p| p[0] < 0.0) {
        return Err(NodeError::input("points", "a revolved section lies at x ≥ 0 (x is the radius)"));
    }
    let cs = ringdesign_solid::kernel::manifold3d::CrossSection::from_simple_polygon(&pts);
    let deg = i.number("degrees")?.clamp(1.0, 360.0);
    Ok(Outputs::one("solid", wrap(ringdesign_solid::kernel::manifold3d::Manifold::revolve(&cs, segments(i)?, deg))))
}

fn from_design(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = match i.get("design") {
        Value::Design(d) => d.clone(),
        other => return Err(NodeError::input("design", format!("expected a design, got {}", other.summary()))),
    };
    let preset = i.text("preset")?;
    let (_, t, p) = BuildParams::PRESETS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(preset)).ok_or_else(|| NodeError::input("preset", format!("{preset:?} is not a build preset")))?;
    let mut params = d.build;
    params.theta_steps = *t;
    params.profile_steps = *p;
    params.refine = None;
    let mut lib = (*ctx.lib).clone();
    d.unpack_embedded(&mut lib);
    d.bake_all(&mut lib);
    let out = ringdesign_core::mesh::build(&d, &lib, params);
    let s = kernel::from_mesh(&out.mesh).map_err(|e| NodeError::new(format!("the sweep did not take as a solid: {e}")))?;
    Ok(Outputs::one("solid", wrap(s)))
}

fn union(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(solid_of(i, "a")?.union(&solid_of(i, "b")?))))
}

fn difference(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(solid_of(i, "a")?.difference(&solid_of(i, "b")?))))
}

fn intersect(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(solid_of(i, "a")?.intersection(&solid_of(i, "b")?))))
}

fn union_all(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut solids = Vec::new();
    for (k, v) in i.list("solids").iter().enumerate() {
        match v {
            Value::Solid(h) => solids.push(h.as_any().downcast_ref::<ManifoldSolid>().map(|m| m.0.clone()).ok_or_else(|| NodeError::input("solids", format!("item {k} is from another kernel")))?),
            other => return Err(NodeError::input("solids", format!("item {k} is {}, not a solid", other.summary()))),
        }
    }
    Ok(Outputs::one("solid", wrap(kernel::union_all(&solids))))
}

fn translate(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(solid_of(i, "solid")?.translate(i.number("x")?, i.number("y")?, i.number("z")?))))
}

fn rotate(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(solid_of(i, "solid")?.rotate(i.number("x_deg")?, i.number("y_deg")?, i.number("z_deg")?))))
}

fn scale(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(solid_of(i, "solid")?.scale(positive(i, "x")?, positive(i, "y")?, positive(i, "z")?))))
}

fn frame_on_ring(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let radius = match i.get("design") {
        Value::Design(d) => d.inner_radius_mm() + d.profile.thickness_mm,
        Value::Null => i.get("radius_mm").as_number().ok_or_else(|| NodeError::input("radius_mm", "a design or a radius is needed"))?,
        other => return Err(NodeError::input("design", format!("expected a design, got {}", other.summary()))),
    };
    let f = Frame::on_ring(radius, i.number("theta_deg")?, i.number("axial_mm")?, i.number("extra_mm")?).tilted(i.number("tilt_deg")?).rolled(i.number("roll_deg")?);
    let json = serde_json::to_value(FrameJson::of(&f)).map_err(|e| NodeError::new(e.to_string()))?;
    Ok(Outputs::one("frame", json).with("origin", serde_json::json!([f.origin.x, f.origin.y, f.origin.z])))
}

fn place(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let f = frame_of(i, "frame")?;
    Ok(Outputs::one("solid", wrap(f.place(&solid_of(i, "solid")?))))
}

fn tube(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let pts = points3(i, "points")?;
    if pts.len() < 2 {
        return Err(NodeError::input("points", "a tube needs at least two points"));
    }
    Ok(Outputs::one("solid", wrap(kernel::tube(&pts, positive(i, "radius")?, segments(i)?))))
}

fn setting(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let seat_h = positive(i, "seat_h")?;
    let parts = match i.text("kind")? {
        "marquise" => kernel::marquise_setting(positive(i, "length")?, positive(i, "width")?, seat_h, i.bool("cross_brace")?),
        "round" => kernel::round_setting(positive(i, "width")?, seat_h),
        other => return Err(NodeError::input("kind", format!("{other:?} is not marquise or round"))),
    };
    Ok(Outputs::one("solid", wrap(parts.resolve())))
}

fn leaf(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("solid", wrap(kernel::leaf(positive(i, "length")?, positive(i, "width")?, positive(i, "thickness")?))))
}

fn import(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let path = i.text("path")?.trim().to_string();
    let lower = path.to_lowercase();
    let mesh = if lower.ends_with(".obj") {
        ringdesign_solid::io::read_obj(&path)
    } else if lower.ends_with(".stl") {
        ringdesign_solid::io::read_stl(&path)
    } else {
        return Err(NodeError::input("path", "an .obj or .stl file"));
    }
    .map_err(|e| NodeError::input("path", format!("{path}: {e:#}")))?;
    let s = kernel::from_mesh(&mesh).map_err(|e| NodeError::input("path", format!("{path}: {e}")))?;
    Ok(Outputs::one("solid", wrap(s)))
}

fn mesh(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let s = solid_of(i, "solid")?;
    let m = kernel::to_mesh(&s);
    let v = m.validate();
    Ok(Outputs::one("triangles", v.triangle_count as i64)
        .with("watertight", v.watertight)
        .with("volume_mm3", s.volume())
        .with("surface_area_mm2", s.surface_area())
        .with("mesh", Value::Mesh(Arc::new(m))))
}

pub fn register(reg: &mut Registry) {
    let seg = || PinSpec::item("segments", ValueKind::Int).default(48i64).doc("Facets round a circle, 3..512.");
    let specs = [
        NodeSpec::new("solid.cylinder", "Cylinder", Category::Solid).free_only()
            .doc("A cylinder (or a cone, with a different top radius) along Z.")
            .input(PinSpec::item("height", ValueKind::Number).default(2.0).widget(Widget::Mm { min: 0.01, max: 100.0 }).doc("Height, mm."))
            .input(PinSpec::item("radius", ValueKind::Number).default(1.0).widget(Widget::Mm { min: 0.01, max: 100.0 }).doc("Bottom radius, mm."))
            .input(PinSpec::item("radius_top", ValueKind::Number).optional().doc("Top radius, mm; the bottom's if unset."))
            .input(seg())
            .input(PinSpec::item("center", ValueKind::Bool).default(false).doc("Centre on the origin instead of standing on z = 0."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .eval(cylinder),
        NodeSpec::new("solid.sphere", "Sphere", Category::Solid).free_only()
            .doc("A sphere at the origin.")
            .input(PinSpec::item("radius", ValueKind::Number).default(1.0).widget(Widget::Mm { min: 0.01, max: 100.0 }).doc("Radius, mm."))
            .input(seg())
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .eval(sphere),
        NodeSpec::new("solid.box", "Box", Category::Solid).free_only()
            .doc("A box from the origin, or centred.")
            .input(PinSpec::item("x", ValueKind::Number).default(2.0).doc("Size along x, mm."))
            .input(PinSpec::item("y", ValueKind::Number).default(2.0).doc("Size along y, mm."))
            .input(PinSpec::item("z", ValueKind::Number).default(2.0).doc("Size along z, mm."))
            .input(PinSpec::item("center", ValueKind::Bool).default(false).doc("Centre on the origin."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .eval(box_),
        NodeSpec::new("solid.extrude", "Extrude", Category::Solid).free_only()
            .doc("A closed polygon in the XY plane extruded along Z.")
            .input(PinSpec::item("points", ValueKind::Path).doc("The polygon, [x, y] pairs."))
            .input(PinSpec::item("height", ValueKind::Number).default(1.0).doc("Height, mm."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .eval(extrude),
        NodeSpec::new("solid.revolve", "Revolve", Category::Solid).free_only()
            .doc("A closed polygon at x ≥ 0 revolved about the Y axis — a ring section turned into a ring.")
            .input(PinSpec::item("points", ValueKind::Path).doc("The section, [radius, height] pairs."))
            .input(seg())
            .input(PinSpec::item("degrees", ValueKind::Number).default(360.0).doc("How far round, degrees."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .eval(revolve),
        NodeSpec::new("solid.from_design", "Design as solid", Category::Solid).free_only()
            .doc("The design's watertight sweep taken into the kernel, so settings and vines can be added to a band the field already judged.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::select("preset", BuildParams::PRESETS.iter().map(|p| p.0.to_string()).collect()).default("Preview").doc("Sweep resolution."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The band as a solid."))
            .eval(from_design),
        NodeSpec::new("solid.union", "Union", Category::Solid).free_only()
            .doc("a ∪ b.")
            .input(PinSpec::item("a", ValueKind::Solid).doc("First.")).input(PinSpec::item("b", ValueKind::Solid).doc("Second."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The union.")).eval(union),
        NodeSpec::new("solid.difference", "Difference", Category::Solid).free_only()
            .doc("a minus b.")
            .input(PinSpec::item("a", ValueKind::Solid).doc("Kept.")).input(PinSpec::item("b", ValueKind::Solid).doc("Cut away."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The difference.")).eval(difference),
        NodeSpec::new("solid.intersect", "Intersect", Category::Solid).free_only()
            .doc("a ∩ b.")
            .input(PinSpec::item("a", ValueKind::Solid).doc("First.")).input(PinSpec::item("b", ValueKind::Solid).doc("Second."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The intersection.")).eval(intersect),
        NodeSpec::new("solid.union_all", "Union all", Category::Solid).free_only()
            .doc("Every solid in a list, unioned at once.")
            .input(PinSpec::list("solids", ValueKind::Solid).doc("The solids."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The union.")).eval(union_all),
        NodeSpec::new("solid.translate", "Translate", Category::Solid).free_only()
            .doc("Move a solid.")
            .input(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .input(PinSpec::item("x", ValueKind::Number).default(0.0).doc("mm")).input(PinSpec::item("y", ValueKind::Number).default(0.0).doc("mm")).input(PinSpec::item("z", ValueKind::Number).default(0.0).doc("mm"))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The moved solid.")).eval(translate),
        NodeSpec::new("solid.rotate", "Rotate", Category::Solid).free_only()
            .doc("Rotate a solid about the axes, degrees, X then Y then Z.")
            .input(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .input(PinSpec::item("x_deg", ValueKind::Number).default(0.0).doc("About X.")).input(PinSpec::item("y_deg", ValueKind::Number).default(0.0).doc("About Y.")).input(PinSpec::item("z_deg", ValueKind::Number).default(0.0).doc("About Z."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The rotated solid.")).eval(rotate),
        NodeSpec::new("solid.scale", "Scale", Category::Solid).free_only()
            .doc("Scale a solid per axis.")
            .input(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .input(PinSpec::item("x", ValueKind::Number).default(1.0).doc("Factor.")).input(PinSpec::item("y", ValueKind::Number).default(1.0).doc("Factor.")).input(PinSpec::item("z", ValueKind::Number).default(1.0).doc("Factor."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The scaled solid.")).eval(scale),
        NodeSpec::new("frame.on_ring", "Frame on ring", Category::Solid).free_only()
            .doc("A frame on the band at an angle (90° is the top): z points out of the metal, x along the ring; lifted along the finger, pushed outward, tilted about x and rolled about z.")
            .input(PinSpec::item("design", ValueKind::Design).optional().doc("The design whose crest radius the frame sits at."))
            .input(PinSpec::item("radius_mm", ValueKind::Number).optional().doc("A radius instead of a design."))
            .input(PinSpec::item("theta_deg", ValueKind::Number).default(90.0).widget(Widget::Angle).doc("Where round the ring."))
            .input(PinSpec::item("axial_mm", ValueKind::Number).default(0.0).doc("Along the finger axis, mm."))
            .input(PinSpec::item("extra_mm", ValueKind::Number).default(0.0).doc("Outward from the crest, mm."))
            .input(PinSpec::item("tilt_deg", ValueKind::Number).default(0.0).doc("Tilt about the frame's x."))
            .input(PinSpec::item("roll_deg", ValueKind::Number).default(0.0).doc("Roll about the frame's z."))
            .output(PinSpec::item("frame", ValueKind::Json).doc("The frame."))
            .output(PinSpec::item("origin", ValueKind::Json).doc("Its origin, [x, y, z]."))
            .eval(frame_on_ring),
        NodeSpec::new("solid.place", "Place", Category::Solid).free_only()
            .doc("A solid moved into a frame: its origin to the frame's origin, its axes to the frame's.")
            .input(PinSpec::item("solid", ValueKind::Solid).doc("The solid, built at the origin with z up."))
            .input(PinSpec::item("frame", ValueKind::Json).doc("The frame, from frame.on_ring."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The placed solid.")).eval(place),
        NodeSpec::new("solid.tube", "Tube", Category::Solid).free_only()
            .doc("A round wire along a polyline of [x, y, z] points: spheres at the knots, cylinders between.")
            .input(PinSpec::item("points", ValueKind::Json).doc("The path, a list of [x, y, z]."))
            .input(PinSpec::item("radius", ValueKind::Number).default(0.4).doc("Wire radius, mm."))
            .input(seg())
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The wire.")).eval(tube),
        NodeSpec::new("solid.setting", "Setting", Category::Solid).free_only()
            .doc("A four-prong setting, marquise or round, standing on z = 0 with the girdle rail at the seat height.")
            .input(PinSpec::select("kind", vec!["round".into(), "marquise".into()]).default("round").doc("Round or marquise."))
            .input(PinSpec::item("length", ValueKind::Number).default(6.0).doc("Stone length, mm (marquise)."))
            .input(PinSpec::item("width", ValueKind::Number).default(4.0).doc("Stone width (or round diameter), mm."))
            .input(PinSpec::item("seat_h", ValueKind::Number).default(2.0).doc("Seat height over the base, mm."))
            .input(PinSpec::item("cross_brace", ValueKind::Bool).default(false).doc("Cross bars under a marquise."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The setting.")).eval(setting),
        NodeSpec::new("solid.leaf", "Leaf", Category::Solid).free_only()
            .doc("A domed leaf with a midrib and veins cut in, lying on the XY plane.")
            .input(PinSpec::item("length", ValueKind::Number).default(6.0).doc("mm")).input(PinSpec::item("width", ValueKind::Number).default(3.0).doc("mm")).input(PinSpec::item("thickness", ValueKind::Number).default(0.8).doc("mm"))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The leaf.")).eval(leaf),
        NodeSpec::new("solid.import", "Import mesh", Category::Solid).free_only()
            .doc("An OBJ or STL file as a solid; it must be watertight.")
            .input(PinSpec::item("path", ValueKind::Text).default("").widget(Widget::TextLine).doc("The file."))
            .output(PinSpec::item("solid", ValueKind::Solid).doc("The solid.")).eval(import),
        NodeSpec::new("solid.mesh", "Solid as mesh", Category::Solid).free_only()
            .doc("A solid's surface as a mesh, for sink.mesh_verdict, sink.export and sink.render.")
            .input(PinSpec::item("solid", ValueKind::Solid).doc("The solid."))
            .output(PinSpec::item("mesh", ValueKind::Mesh).doc("The mesh."))
            .output(PinSpec::item("triangles", ValueKind::Int).doc("Triangle count."))
            .output(PinSpec::item("watertight", ValueKind::Bool).doc("Always, for a solid."))
            .output(PinSpec::item("volume_mm3", ValueKind::Number).doc("Volume, mm³."))
            .output(PinSpec::item("surface_area_mm2", ValueKind::Number).doc("Area, mm²."))
            .eval(mesh),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}

#[cfg(test)]
mod tests {
    use crate::eval::{Evaluator, Targets};
    use crate::graph::{Graph, Mode};
    use crate::registry::Registry;
    use crate::value::{Literal, Value};
    use ringdesign_core::AlphaLibrary;

    #[test]
    fn a_semi_mount_builds_in_free_mode_and_sandring_refuses_the_nodes() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let mut g = Graph::new("semi-mount", Mode::Free);
        let d = g.add("design.new").unwrap();
        let band = g.add("solid.from_design").unwrap();
        g.connect(d, "design", band, "design").unwrap();
        g.set_input(band, "preset", Literal::Text("Draft".into())).unwrap();
        let frame = g.add("frame.on_ring").unwrap();
        g.connect(d, "design", frame, "design").unwrap();
        g.set_input(frame, "extra_mm", Literal::Number(-0.4)).unwrap();
        let setting = g.add("solid.setting").unwrap();
        g.set_input(setting, "width", Literal::Number(4.0)).unwrap();
        let placed = g.add("solid.place").unwrap();
        g.connect(setting, "solid", placed, "solid").unwrap();
        g.connect(frame, "frame", placed, "frame").unwrap();
        let union = g.add("solid.union").unwrap();
        g.connect(band, "solid", union, "a").unwrap();
        g.connect(placed, "solid", union, "b").unwrap();
        let mesh = g.add("solid.mesh").unwrap();
        g.connect(union, "solid", mesh, "solid").unwrap();
        let verdict = g.add("sink.mesh_verdict").unwrap();
        g.connect(mesh, "mesh", verdict, "mesh").unwrap();
        g.connect(d, "design", verdict, "design").unwrap();
        let dir = std::env::temp_dir().join(format!("ringdesign-free-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stl = dir.join("semi-mount.stl");
        let export = g.add("sink.export").unwrap();
        g.connect(mesh, "mesh", export, "mesh").unwrap();
        g.set_input(export, "path", Literal::Text(stl.display().to_string())).unwrap();
        assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::Everything);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(r.value(mesh, "watertight"), Some(&Value::Bool(true)));
        let band_vol = match r.value(band, "solid") { Some(Value::Solid(s)) => s.describe(), other => panic!("{other:?}") };
        assert!(band_vol.contains("mm³"));
        let v = r.value(mesh, "volume_mm3").unwrap().as_number().unwrap();
        assert!(v > 50.0, "{v}");
        assert!(matches!(r.value(verdict, "verdict"), Some(Value::Text(_))), "{:?}", r.status[&verdict]);
        assert!(std::fs::metadata(&stl).unwrap().len() > 84, "the semi-mount exported");

        // The exported file comes back in as a solid of the same volume.
        let imp = g.add("solid.import").unwrap();
        g.set_input(imp, "path", Literal::Text(stl.display().to_string())).unwrap();
        let m2 = g.add("solid.mesh").unwrap();
        g.connect(imp, "solid", m2, "solid").unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(!r.status[&imp].failed(), "{:?}", r.status[&imp].errors);
        let v2 = r.value(m2, "volume_mm3").unwrap().as_number().unwrap();
        assert!((v2 - v).abs() < 1e-3 * v, "{v2} vs {v}");
        let _ = std::fs::remove_dir_all(&dir);

        // The same graph in SandRing mode is refused at validation.
        g.mode = Mode::SandRing;
        let errs = g.validate(Some(&reg));
        assert!(errs.iter().any(|e| e.message.contains("does not run in SandRing")), "{errs:?}");

        // Primitives, booleans and a revolve agree with the kernel.
        let mut g = Graph::new("prims", Mode::Free);
        let a = g.add("solid.cylinder").unwrap();
        g.set_input(a, "height", Literal::Number(2.0)).unwrap();
        let b = g.add("solid.sphere").unwrap();
        let diff = g.add("solid.difference").unwrap();
        g.connect(a, "solid", diff, "a").unwrap();
        g.connect(b, "solid", diff, "b").unwrap();
        let rv = g.add("solid.revolve").unwrap();
        g.set_input(rv, "points", Literal::Json(serde_json::json!([[8.0, -1.0], [10.0, -1.0], [10.0, 1.0], [8.0, 1.0]]))).unwrap();
        let rm = g.add("solid.mesh").unwrap();
        g.connect(rv, "solid", rm, "solid").unwrap();
        let bad = g.add("solid.revolve").unwrap();
        g.set_input(bad, "points", Literal::Json(serde_json::json!([[-1.0, 0.0], [1.0, 0.0], [0.0, 1.0]]))).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &lib, 0, Targets::AllPure);
        assert!(matches!(r.value(diff, "solid"), Some(Value::Solid(_))));
        let ring_vol = r.value(rm, "volume_mm3").unwrap().as_number().unwrap();
        let expect = std::f64::consts::PI * (100.0 - 64.0) * 2.0;
        assert!((ring_vol - expect).abs() < 0.02 * expect, "{ring_vol} vs {expect}");
        assert!(r.status[&bad].errors[0].1.contains("x ≥ 0"));
    }
}
