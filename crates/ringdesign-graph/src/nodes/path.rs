//! Paths in the world and the parts made along them.
//!
//! A path travels as one JSON value, `[[x, y, z], …]` in world millimetres, so it reaches a sweep or an array whole; a node that
//! makes several (`path.wreath`) makes a list of them, and what it feeds runs once per path. `path.sweep` turns a path into a
//! sweep's operation and `path.along` into an array along it, both for a `cad.feature`; `path.crest` also hands back the crest
//! as placements, one prickle per point through `cad.features`.
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};
use ringdesign_core::RingDesign;
use ringdesign_core::cad::pattern::along::{self, Along, AlongPath, MAX_PATH_POINTS};
use ringdesign_core::cad::{Operation, PatternKind, Placement, Profile};
use ringdesign_core::sketch::Sketch;
use serde_json::json;

/// Most stations a sweep takes, as `Operation::Sweep` does.
pub const MAX_SWEEP_STATIONS: usize = 128;

fn design(i: &Inputs) -> Result<&RingDesign, NodeError> {
    match i.get("design") {
        Value::Design(d) => Ok(d),
        _ => Err(NodeError::input("design", "Connect a design")),
    }
}

/// A count of path points, two or more and at most [`MAX_PATH_POINTS`].
fn points_count(i: &Inputs, pin: &str) -> Result<usize, NodeError> {
    let n = i.int(pin)?;
    if !(2..=MAX_PATH_POINTS as i64).contains(&n) {
        return Err(NodeError::input(pin, format!("a path takes 2 to {MAX_PATH_POINTS} points, not {n}")));
    }
    Ok(n as usize)
}

fn finite(i: &Inputs, pin: &str) -> Result<f64, NodeError> {
    let v = i.number(pin)?;
    if !v.is_finite() {
        return Err(NodeError::input(pin, "expected a finite number"));
    }
    Ok(v)
}

/// A path as the graph carries it.
fn path_value(points: &[[f64; 3]]) -> Value {
    Value::Json(std::sync::Arc::new(json!(points)))
}

/// Points round the finger's axis, closed on a whole turn, at height `z` and radius `r(θ)`.
fn round(from_deg: f64, to_deg: f64, n: usize, r: impl Fn(f64) -> f64, z: impl Fn(f64) -> f64) -> Vec<[f64; 3]> {
    let whole = (to_deg - from_deg).abs() >= 360.0 - 1e-9;
    (0..n)
        .map(|k| {
            // A whole turn ends on its start, which closes it for an array.
            let t = if whole && k + 1 == n { 0.0 } else { k as f64 / (n - 1) as f64 };
            let theta = from_deg + (to_deg - from_deg) * t;
            let (s, c) = theta.to_radians().sin_cos();
            [r(theta) * c, r(theta) * s, z(theta)]
        })
        .collect()
}

fn arc(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (r, from, to, z) = (finite(i, "radius_mm")?, finite(i, "from_deg")?, finite(i, "to_deg")?, finite(i, "z_mm")?);
    if r <= 0.0 {
        return Err(NodeError::input("radius_mm", "a radius above zero"));
    }
    if (to - from).abs() > 360.0 + 1e-9 || (to - from).abs() < 1e-9 {
        return Err(NodeError::input("to_deg", "an arc runs more than 0° and at most 360°"));
    }
    let n = points_count(i, "count")?;
    Ok(Outputs::one("path", path_value(&round(from, to, n, |_| r, |_| z))))
}

fn helix(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (r, pitch, turns, from, z) = (finite(i, "radius_mm")?, finite(i, "pitch_mm")?, finite(i, "turns")?, finite(i, "from_deg")?, finite(i, "z_mm")?);
    if r <= 0.0 {
        return Err(NodeError::input("radius_mm", "a radius above zero"));
    }
    if turns == 0.0 || turns.abs() > 100.0 {
        return Err(NodeError::input("turns", "a helix turns more than 0 and at most 100 times"));
    }
    let n = points_count(i, "count")?;
    let points: Vec<[f64; 3]> = (0..n)
        .map(|k| {
            let t = k as f64 / (n - 1) as f64;
            let theta = from + 360.0 * turns * t;
            let (s, c) = theta.to_radians().sin_cos();
            [r * c, r * s, z + pitch * turns.abs() * t]
        })
        .collect();
    Ok(Outputs::one("path", path_value(&points)))
}

fn wreath(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let strands = i.int("strands")?;
    let crossings = i.int("crossings")?;
    if !(1..=12).contains(&strands) {
        return Err(NodeError::input("strands", "a wreath winds 1 to 12 strands"));
    }
    if !(0..=24).contains(&crossings) {
        return Err(NodeError::input("crossings", "a strand weaves 0 to 24 times round the ring"));
    }
    let (wander, cane, overlap, r) = (finite(i, "wander_mm")?, finite(i, "cane_mm")?, finite(i, "overlap_mm")?, finite(i, "radius_mm")?);
    let (from, to) = (finite(i, "from_deg")?, finite(i, "to_deg")?);
    if cane <= 0.0 || overlap < 0.0 || overlap >= 2.0 * cane {
        return Err(NodeError::input("overlap_mm", "canes overlap by 0 or more and less than a whole cane"));
    }
    if r <= cane {
        return Err(NodeError::input("radius_mm", "the wreath's radius reaches past its canes"));
    }
    if (to - from).abs() < 1e-9 || (to - from).abs() > 720.0 {
        return Err(NodeError::input("to_deg", "a strand runs more than 0° and at most two turns"));
    }
    let n = points_count(i, "count")?;
    // Opposite strands pass each other at centres two canes less the overlap apart, a crossing that is never a tangency.
    let swing = cane - 0.5 * overlap;
    let c = crossings as f64;
    let paths: Vec<Value> = (0..strands)
        .map(|k| {
            let phase = 360.0 * k as f64 / strands as f64;
            let wave = move |theta: f64| (c * theta + phase).to_radians();
            path_value(&round(from, to, n, |t| r + swing * wave(t).cos(), |t| wander * wave(t).sin()))
        })
        .collect();
    Ok(Outputs::one("paths", paths).with("swing_mm", swing))
}

fn climb(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design(i)?;
    let (from, sweep, turns, amplitude, phase, lift) =
        (finite(i, "from_deg")?, finite(i, "sweep_deg")?, finite(i, "turns")?, finite(i, "amplitude_mm")?, finite(i, "phase_deg")?, finite(i, "lift_mm")?);
    if sweep.abs() < 1e-9 || sweep.abs() > 720.0 {
        return Err(NodeError::input("sweep_deg", "a climb runs more than 0° and at most two turns"));
    }
    let n = points_count(i, "count")?;
    let ctx = d.field_context();
    let (mid, span) = (ctx.crest_v_mm, ctx.band_v_len_mm);
    let chart: Vec<[f64; 2]> = (0..n)
        .map(|k| {
            let t = k as f64 / (n - 1) as f64;
            let run = sweep * t;
            let v = mid + amplitude * (turns * run + phase).to_radians().sin();
            [from + run, v.clamp(0.0, span)]
        })
        .collect();
    let points = along::chart_points(d, &chart, lift).map_err(NodeError::from)?;
    Ok(Outputs::one("path", path_value(&points)).with("chart", Value::from(chart)))
}

/// The turn about a frame's normal `z` that carries a ring placement's frame onto it, its `y` onto the frame's `y`.
fn spin_onto(y: [f64; 3], z: [f64; 3]) -> f64 {
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // A ring placement's x runs along the finger, squared to the normal; its y is z × x.
    let d = -z[2];
    let x0 = [-z[0] * d, -z[1] * d, -1.0 - z[2] * d];
    let l = dot(x0, x0).sqrt();
    if l < 1e-9 {
        return 0.0;
    }
    let x0 = x0.map(|v| v / l);
    let y0 = [z[1] * x0[2] - z[2] * x0[1], z[2] * x0[0] - z[0] * x0[2], z[0] * x0[1] - z[1] * x0[0]];
    let spin = (-dot(y, x0)).atan2(dot(y, y0)).to_degrees();
    if spin.abs() < 1e-9 { 0.0 } else { spin }
}

fn crest(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design(i)?;
    let (from, to) = (finite(i, "from_deg")?, finite(i, "to_deg")?);
    let count = i.int("count")?;
    let most = ringdesign_core::cad::pattern::MAX_PATTERN_COUNT;
    if !(2..=i64::from(most)).contains(&count) {
        return Err(NodeError::input("count", format!("a crest path takes 2 to {most} points")));
    }
    let frames = along::crest_frames(d, from, to, count as u32).map_err(NodeError::from)?;
    let points: Vec<[f64; 3]> = frames.iter().map(|f| f.origin).collect();
    let placements: Vec<Value> = frames
        .iter()
        .map(|f| {
            let o = f.origin;
            let p = Placement::Ring { theta_deg: o[1].atan2(o[0]).to_degrees().rem_euclid(360.0), across_mm: o[2], height_mm: 0.0, spin_deg: spin_onto(f.y_axis, f.z_axis), tilt_deg: 0.0, cant_deg: 0.0, level: false };
            Value::from(serde_json::to_value(p).unwrap_or_default())
        })
        .collect();
    let along_json = serde_json::to_value(AlongPath::Crest { from_deg: from, to_deg: to }).map_err(|e| NodeError::new(e.to_string()))?;
    Ok(Outputs::one("path", path_value(&points)).with("placements", placements).with("along", Value::from(along_json)))
}

/// A path read off a pin: `[[x, y, z], …]`.
fn points3(i: &Inputs, pin: &str) -> Result<Vec<[f64; 3]>, NodeError> {
    let json = i.get(pin).to_json_any().ok_or_else(|| NodeError::input(pin, "expected a path, a list of [x, y, z]"))?;
    serde_json::from_value(json).map_err(|e| NodeError::input(pin, format!("expected a path, a list of [x, y, z]: {e}")))
}

fn sweep(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let path = points3(i, "path")?;
    if !(2..=MAX_SWEEP_STATIONS).contains(&path.len()) {
        return Err(NodeError::input("path", format!("a sweep takes 2 to {MAX_SWEEP_STATIONS} stations, and this path has {}; ask the path for fewer points", path.len())));
    }
    let section: Profile = match i.get("section") {
        Value::Null => {
            let r = finite(i, "radius_mm")?;
            if r <= 0.0 {
                return Err(NodeError::input("radius_mm", "a radius above zero"));
            }
            Sketch::circle(r).into()
        }
        v => {
            let json = v.to_json_any().ok_or_else(|| NodeError::input("section", "expected a profile or a Sketch operation"))?;
            // A Sketch feature's operation sweeps its sketch.
            let json = json.get("Sketch").and_then(|s| s.get("sketch")).cloned().map(|s| json!(s)).unwrap_or(json);
            serde_json::from_value(json).map_err(|e| NodeError::input("section", format!("expected a profile: {e}")))?
        }
    };
    let op = Operation::sweep(section, path);
    Ok(Outputs::one("operation", serde_json::to_value(op).map_err(|e| NodeError::new(e.to_string()))?))
}

/// What an array follows, read off a pin: a JSON path object (`{"crest": …}`, `{"feature": 7}`, `{"sketch": …}`), a list of
/// `[x, y, z]` world points, or `[θ, v]` chart points.
fn along_path(i: &Inputs) -> Result<AlongPath, NodeError> {
    let json = i.get("path").to_json_any().ok_or_else(|| NodeError::input("path", "expected a path"))?;
    if let Some(items) = json.as_array() {
        if items.first().and_then(serde_json::Value::as_array).is_some_and(|p| p.len() == 2) {
            let chart: Vec<[f64; 2]> = serde_json::from_value(json).map_err(|e| NodeError::input("path", format!("expected [θ, v] chart points: {e}")))?;
            return Ok(AlongPath::Chart(chart));
        }
        let points: Vec<[f64; 3]> = serde_json::from_value(json).map_err(|e| NodeError::input("path", format!("expected [x, y, z] points: {e}")))?;
        return Ok(AlongPath::Points(points));
    }
    serde_json::from_value(json).map_err(|e| NodeError::input("path", format!("expected a crest, a feature, a sketch, or points: {e}")))
}

fn along_op(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut sources = Vec::new();
    for (k, v) in i.list("sources").iter().enumerate() {
        sources.push(v.as_int().and_then(|n| u64::try_from(n).ok()).ok_or_else(|| NodeError::input("sources", format!("item {k} is not a feature id")))?);
    }
    if sources.is_empty() {
        return Err(NodeError::input("sources", "an array copies at least one part"));
    }
    let count = i.int("count")?;
    let a = Along {
        path: along_path(i)?,
        count: u32::try_from(count).map_err(|_| NodeError::input("count", "a count of 0 or more"))?,
        pitch_mm: i.get("pitch_mm").as_number(),
        phase: i.number("phase")?,
        alternate_deg: i.number("alternate_deg")?,
        roll_deg: i.number("roll_deg")?,
        scale: [i.number("scale_start")?, i.number("scale_end")?],
        level: i.bool("level")?,
    };
    a.check().map_err(NodeError::from)?;
    let op = Operation::Pattern { sources: ringdesign_core::cad::pattern::Sources(sources), kind: PatternKind::Along(a) };
    Ok(Outputs::one("operation", serde_json::to_value(op).map_err(|e| NodeError::new(e.to_string()))?))
}

pub fn register(reg: &mut Registry) {
    let mm = |name: &str, default: f64, doc: &str| PinSpec::item(name, ValueKind::Number).default(default).widget(Widget::Mm { min: -50.0, max: 50.0 }).doc(doc);
    let angle = |name: &str, default: f64, doc: &str| PinSpec::item(name, ValueKind::Number).default(default).widget(Widget::Angle).doc(doc);
    let count = |default: i64, doc: &str| PinSpec::item("count", ValueKind::Int).default(default).doc(doc);
    let path_out = || PinSpec::item("path", ValueKind::Json).doc("The path: [x, y, z] points in world millimetres.");
    let design_in = || PinSpec::item("design", ValueKind::Design).doc("The ring whose band the path lies on.");

    reg.register(
        NodeSpec::new("path.arc", "Arc path", Category::Assembly)
            .doc("Points on a circle round the finger's axis, in a plane parallel to the parting plane; a whole turn ends on its start, which closes it for an array.")
            .input(mm("radius_mm", 10.0, "Distance from the finger's axis, mm."))
            .input(angle("from_deg", 0.0, "Where the arc starts round the ring, degrees."))
            .input(angle("to_deg", 360.0, "Where it ends, degrees."))
            .input(mm("z_mm", 0.0, "Height along the finger, mm."))
            .input(count(64, "Points along the arc."))
            .output(path_out())
            .eval(arc),
    )
    .expect("unique");
    reg.register(
        NodeSpec::new("path.helix", "Helix path", Category::Assembly)
            .doc("Points on a helix round the finger's axis, climbing `pitch_mm` along the finger per turn.")
            .input(mm("radius_mm", 10.0, "Distance from the finger's axis, mm."))
            .input(mm("pitch_mm", 2.0, "Climb along the finger per turn, mm."))
            .input(PinSpec::item("turns", ValueKind::Number).default(1.0).doc("Turns round the ring; negative winds the other way."))
            .input(angle("from_deg", 0.0, "Where it starts round the ring, degrees."))
            .input(mm("z_mm", 0.0, "Where it starts along the finger, mm."))
            .input(count(128, "Points along the helix."))
            .output(path_out())
            .eval(helix),
    )
    .expect("unique");
    reg.register(
        NodeSpec::new("path.wreath", "Wreath paths", Category::Assembly)
            .doc("One path per strand of a woven wreath round the finger: each strand waves `crossings` times round the ring along the finger and in and out, opposite strands passing each other a cane apart less the overlap, so every crossing is a real overlap and never a tangency.")
            .input(PinSpec::item("strands", ValueKind::Int).default(4i64).doc("Strands woven together."))
            .input(PinSpec::item("crossings", ValueKind::Int).default(3i64).doc("Waves a strand makes round the ring."))
            .input(mm("wander_mm", 2.1, "How far a strand swings along the finger, mm."))
            .input(mm("cane_mm", 0.72, "A strand's radius, mm: the cane the paths are swept with."))
            .input(mm("overlap_mm", 0.34, "How far crossing canes run into each other, mm."))
            .input(mm("radius_mm", 10.85, "The strands' mean distance from the finger's axis, mm."))
            .input(angle("from_deg", 0.0, "Where each strand starts round the ring, degrees."))
            .input(angle("to_deg", 360.0, "Where it ends; split a whole turn in two to sweep it in 128-station halves."))
            .input(count(128, "Points along each strand."))
            .output(PinSpec::list("paths", ValueKind::Json).doc("One path per strand, [x, y, z] points in world millimetres."))
            .output(PinSpec::item("swing_mm", ValueKind::Number).doc("How far a strand swings in and out from the mean radius, mm."))
            .eval(wreath),
    )
    .expect("unique");
    reg.register(
        NodeSpec::new("path.climb", "Climbing path", Category::Assembly)
            .doc("A stem climbing the band edge to edge: from the crest's chart line it swings `amplitude_mm` across the band `turns` times per turn of the ring, laid on the bare band and lifted off it along its normal.")
            .input(design_in())
            .input(angle("from_deg", 250.0, "Where the climb starts round the ring, degrees."))
            .input(angle("sweep_deg", 330.0, "How far round the ring it runs, degrees; negative runs the other way."))
            .input(PinSpec::item("turns", ValueKind::Number).default(1.5).doc("Swings across the band per turn of the ring."))
            .input(mm("amplitude_mm", 2.0, "How far across the band it swings either side of the crest, chart mm."))
            .input(angle("phase_deg", 0.0, "Where in its swing it starts, degrees."))
            .input(mm("lift_mm", 0.12, "How far off the band the path runs, along its normal, mm."))
            .input(count(128, "Points along the climb."))
            .output(path_out())
            .output(PinSpec::item("chart", ValueKind::Path).doc("The same path as [θ, v] chart points, which an array follows on the band as it is resized."))
            .eval(climb),
    )
    .expect("unique");
    reg.register(
        NodeSpec::new("path.crest", "Crest path", Category::Assembly)
            .doc("Points along the band's crest, where its outer surface crosses the parting plane, spaced evenly along it from one angle to another, ends included; as placements, one part per point.")
            .input(design_in())
            .input(angle("from_deg", 0.0, "Where the crest path starts round the ring, degrees."))
            .input(angle("to_deg", 360.0, "Where it ends; a whole turn closes on itself."))
            .input(count(12, "Points along the crest."))
            .output(path_out())
            .output(PinSpec::list("placements", ValueKind::Json).doc("A ring placement at each point, turned to run along the crest."))
            .output(PinSpec::item("along", ValueKind::Json).doc("The crest as an array's path, which follows the band as it is resized."))
            .eval(crest),
    )
    .expect("unique");
    reg.register(
        NodeSpec::new("path.sweep", "Sweep along path", Category::Assembly)
            .doc("A sweep's operation: a round section, or any profile, carried along a path of 2 to 128 stations. Wire it into a CAD feature.")
            .input(PinSpec::item("path", ValueKind::Json).doc("The path, [x, y, z] points in world millimetres."))
            .input(mm("radius_mm", 0.5, "The round section's radius, mm."))
            .input(PinSpec::item("section", ValueKind::Json).optional().doc("A profile or a Sketch operation to sweep instead of the round section, square to the path at its start."))
            .output(PinSpec::item("operation", ValueKind::Json).doc("The Sweep operation."))
            .eval(sweep),
    )
    .expect("unique");
    reg.register(
        NodeSpec::new("path.along", "Array along path", Category::Assembly)
            .doc("A Pattern operation copying parts along a path: the band's crest, a sweep's or a twisted sweep's path, a sketch's curves, chart points or world points. The first part stands at the path's first station; each copy is carried from there to its own, turned, alternated and graded. Wire it into a CAD feature.")
            .input(PinSpec::list("sources", ValueKind::Int).doc("The parts copied, by feature id; the first's frame is carried."))
            .input(PinSpec::item("path", ValueKind::Json).doc("A path object ({\"crest\": {\"from_deg\", \"to_deg\"}}, {\"feature\": id}, {\"sketch\": {\"feature\", \"entities\"}}), [x, y, z] world points, or [θ, v] chart points."))
            .input(PinSpec::item("count", ValueKind::Int).default(0i64).doc("Instances in all, the first part among them; 0 fits as many as the pitch allows."))
            .input(PinSpec::item("pitch_mm", ValueKind::Number).optional().doc("Path length between neighbouring copies, mm; unset spreads the count over the whole path."))
            .input(PinSpec::item("phase", ValueKind::Number).default(0.0).doc("Every station moved this share of one step along the path, 0 to under 1."))
            .input(angle("alternate_deg", 0.0, "Turn added to every other copy about its normal, degrees: 180 alternates sides."))
            .input(angle("roll_deg", 0.0, "Turn added per copy about its normal, degrees: 137.5 is phyllotaxis."))
            .input(PinSpec::item("scale_start", ValueKind::Number).default(1.0).doc("Scale at the path's start."))
            .input(PinSpec::item("scale_end", ValueKind::Number).default(1.0).doc("Scale at the path's end; copies grade between."))
            .input(PinSpec::item("level", ValueKind::Bool).default(false).doc("Stand every station square to the parting plane."))
            .output(PinSpec::item("operation", ValueKind::Json).doc("The Pattern operation."))
            .eval(along_op),
    )
    .expect("unique");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets};
    use crate::graph::{Graph, Mode};
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::default(), 0, Targets::AllPure)
    }

    fn json_of(v: Option<&Value>) -> serde_json::Value {
        v.and_then(Value::to_json_any).unwrap_or_default()
    }

    #[test]
    fn a_wreath_weaves_strands_that_cross_a_cane_apart_less_the_overlap() {
        let mut g = Graph::new("wreath", Mode::Free);
        let w = g.add("path.wreath").unwrap();
        g.set_input(w, "count", Literal::Int(97)).unwrap();
        let report = run(&g);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let Some(Value::List(paths)) = report.value(w, "paths") else { panic!("{:?}", report.value(w, "paths").map(Value::summary)) };
        assert_eq!(paths.len(), 4);
        let strands: Vec<Vec<[f64; 3]>> = paths.iter().map(|p| serde_json::from_value(p.to_json_any().unwrap()).unwrap()).collect();
        let swing = report.value(w, "swing_mm").and_then(Value::as_number).unwrap();
        assert!((swing - 0.55).abs() < 1e-12, "{swing}");
        for s in &strands {
            assert_eq!(s.len(), 97);
            assert!((s[0][0] - s[96][0]).abs() < 1e-9 && (s[0][1] - s[96][1]).abs() < 1e-9 && (s[0][2] - s[96][2]).abs() < 1e-9, "a whole turn closes");
        }
        // Strands 0 and 2 are half a wave apart: where they cross the parting plane they stand two swings apart radially.
        let r = |p: [f64; 3]| p[0].hypot(p[1]);
        let (a, b) = (strands[0][0], strands[2][0]);
        assert!(a[2].abs() < 1e-9 && b[2].abs() < 1e-9);
        assert!(((r(a) - r(b)).abs() - 2.0 * swing).abs() < 1e-9, "{} {}", r(a), r(b));
        // And they swing along the finger by the wander either way.
        let reach = strands[0].iter().map(|p| p[2].abs()).fold(0.0, f64::max);
        assert!((reach - 2.1).abs() < 0.01, "{reach}");
        // Each strand sweeps whole.
        let mut g2 = g.clone();
        let s = g2.add("path.sweep").unwrap();
        g2.connect(w, "paths", s, "path").unwrap();
        g2.set_input(s, "radius_mm", Literal::Number(0.72)).unwrap();
        let report = run(&g2);
        let Some(Value::List(ops)) = report.value(s, "operation") else { panic!() };
        assert_eq!(ops.len(), 4, "one sweep per strand");
        let op: Operation = serde_json::from_value(ops[1].to_json_any().unwrap()).unwrap();
        assert!(matches!(op, Operation::Sweep { path: ringdesign_core::cad::SweepPath::Points(ref p), .. } if p.len() == 97));
    }

    #[test]
    fn arcs_and_helices_land_where_they_say_and_a_long_path_refuses_a_sweep_by_name() {
        let mut g = Graph::new("paths", Mode::Free);
        let a = g.add("path.arc").unwrap();
        g.set_input(a, "radius_mm", Literal::Number(9.0)).unwrap();
        g.set_input(a, "to_deg", Literal::Number(90.0)).unwrap();
        g.set_input(a, "count", Literal::Int(4)).unwrap();
        let h = g.add("path.helix").unwrap();
        g.set_input(h, "turns", Literal::Number(2.0)).unwrap();
        g.set_input(h, "count", Literal::Int(200)).unwrap();
        let s = g.add("path.sweep").unwrap();
        g.connect(h, "path", s, "path").unwrap();
        let report = run(&g);
        let arc: Vec<[f64; 3]> = serde_json::from_value(json_of(report.value(a, "path"))).unwrap();
        assert_eq!(arc.len(), 4);
        assert!((arc[0][0] - 9.0).abs() < 1e-12 && arc[3][0].abs() < 1e-9 && (arc[3][1] - 9.0).abs() < 1e-12, "{arc:?}");
        let helix: Vec<[f64; 3]> = serde_json::from_value(json_of(report.value(h, "path"))).unwrap();
        assert!((helix[199][2] - 4.0).abs() < 1e-12 && (helix[199][0] - 10.0).abs() < 1e-9, "two turns of 2 mm: {:?}", helix[199]);
        let why = report.status.get(&s).and_then(|st| st.errors.first()).map(|(_, m)| m.clone()).unwrap_or_default();
        assert!(why.contains("a sweep takes 2 to 128 stations, and this path has 200"), "{why}");
    }

    #[test]
    fn a_crest_path_stands_its_points_on_the_crest_and_its_placements_seat_there() {
        let mut g = Graph::new("crest", Mode::Free);
        let d = g.add("design.new").unwrap();
        let c = g.add("path.crest").unwrap();
        g.connect(d, "design", c, "design").unwrap();
        g.set_input(c, "from_deg", Literal::Number(30.0)).unwrap();
        g.set_input(c, "to_deg", Literal::Number(150.0)).unwrap();
        g.set_input(c, "count", Literal::Int(5)).unwrap();
        let report = run(&g);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let points: Vec<[f64; 3]> = serde_json::from_value(json_of(report.value(c, "path"))).unwrap();
        assert_eq!(points.len(), 5);
        let Some(Value::Design(design)) = report.value(d, "design") else { panic!() };
        let crest_r = design.inner_radius_mm() + design.profile.thickness_mm;
        for (k, p) in points.iter().enumerate() {
            let theta = p[1].atan2(p[0]).to_degrees();
            assert!((theta - (30.0 + 30.0 * k as f64)).abs() < 1e-6, "point {k} at {theta}°");
            assert!(p[2].abs() < 1e-6 && (p[0].hypot(p[1]) - crest_r).abs() < 0.01, "on the crest: {p:?}");
        }
        let Some(Value::List(placements)) = report.value(c, "placements") else { panic!() };
        let p: Placement = serde_json::from_value(placements[2].to_json_any().unwrap()).unwrap();
        let Placement::Ring { theta_deg, across_mm, spin_deg, .. } = p else { panic!("{p:?}") };
        assert!((theta_deg - 90.0).abs() < 1e-6 && across_mm.abs() < 1e-6 && spin_deg.abs() < 1e-6, "{p:?}");
        assert_eq!(json_of(report.value(c, "along")), json!({"crest": {"from_deg": 30.0, "to_deg": 150.0}}));
    }

    #[test]
    fn cad_features_appends_one_part_per_crest_placement_to_one_design_and_the_file_is_fenced() {
        use ringdesign_core::cad::{Component, Feature};
        let mut g = Graph::new("prickles", Mode::Free);
        let src = g.add("cad.source").unwrap();
        g.node_mut(src).unwrap().params = serde_json::to_value(RingDesign::default()).unwrap();
        let band = g.add("cad.feature").unwrap();
        let shank = Feature { id: band.0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() };
        g.node_mut(band).unwrap().params = serde_json::to_value(&shank).unwrap();
        g.connect(src, "design", band, "design").unwrap();
        let crest = g.add("path.crest").unwrap();
        g.connect(src, "design", crest, "design").unwrap();
        g.set_input(crest, "from_deg", Literal::Number(40.0)).unwrap();
        g.set_input(crest, "to_deg", Literal::Number(140.0)).unwrap();
        g.set_input(crest, "count", Literal::Int(5)).unwrap();
        let many = g.add("cad.features").unwrap();
        g.node_mut(many).unwrap().params = json!({ "name": "Prickle", "operation": { "Cylinder": { "radius_mm": 0.3, "height_mm": 0.8 } } });
        g.set_input(many, "attach", Literal::Text("join".into())).unwrap();
        g.connect(band, "design", many, "design").unwrap();
        g.connect(crest, "placements", many, "placement").unwrap();
        let report = run(&g);
        assert!(report.errors.is_empty() && !report.any_failed(), "{:?} {:?}", report.errors, report.status.get(&many));
        let Some(Value::Design(d)) = report.value(many, "design") else { panic!("{:?}", report.value(many, "design").map(Value::summary)) };
        let doc = d.cad.as_ref().unwrap();
        assert_eq!(doc.features.len(), 6, "the band and five prickles in one design");
        let Some(Value::List(ids)) = report.value(many, "ids") else { panic!() };
        for (k, id) in ids.iter().enumerate() {
            let want = super::super::cad::features_id(many, k).unwrap();
            assert_eq!(id.as_int(), Some(want as i64));
            let f = doc.feature(want).unwrap();
            assert_eq!(f.name, format!("Prickle, {}", k + 1));
            assert_eq!(f.component.attach, ringdesign_core::cad::Attach::Join);
            assert!(matches!(f.component.placement, Placement::Ring { theta_deg, .. } if (theta_deg - (40.0 + 25.0 * k as f64)).abs() < 1e-6), "{:?}", f.component.placement);
        }
        let e = ringdesign_core::cad::evaluate(d, &AlphaLibrary::default(), ringdesign_core::BuildParams::default()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        // A graph carrying the path nodes is written at the version that fences them.
        assert_eq!(crate::file::graph_version_for(&g), crate::file::GRAPH_FORMAT_VERSION);
        let mut plain = Graph::new("plain", Mode::Free);
        plain.add("cad.source").unwrap();
        assert_eq!(crate::file::graph_version_for(&plain), crate::file::PLAIN_GRAPH_FORMAT_VERSION);
    }

    #[test]
    fn a_climbing_stem_sweeps_over_the_band_and_its_rootlets_follow_it_at_their_pitch() {
        use ringdesign_core::cad::{Attach, Component, Feature};
        let mut g = Graph::new("ivy", Mode::Free);
        let src = g.add("cad.source").unwrap();
        g.node_mut(src).unwrap().params = serde_json::to_value(RingDesign::default()).unwrap();
        let joined = |id: crate::graph::NodeId, name: &str, operation: Operation, placement: Placement| {
            serde_json::to_value(Feature { id: id.0, name: name.into(), enabled: true, operation, component: Component { attach: Attach::Join, placement, ..Component::default() } }).unwrap()
        };
        let band = g.add("cad.feature").unwrap();
        g.node_mut(band).unwrap().params = joined(band, "Procedural shank", Operation::Band, Placement::Free);
        g.connect(src, "design", band, "design").unwrap();
        let climb = g.add("path.climb").unwrap();
        g.connect(src, "design", climb, "design").unwrap();
        g.set_input(climb, "sweep_deg", Literal::Number(120.0)).unwrap();
        let sweep = g.add("path.sweep").unwrap();
        g.connect(climb, "path", sweep, "path").unwrap();
        g.set_input(sweep, "radius_mm", Literal::Number(0.4)).unwrap();
        let stem = g.add("cad.feature").unwrap();
        g.node_mut(stem).unwrap().params = joined(stem, "Stem", Operation::Sphere { radius_mm: 1.0 }, Placement::Free);
        g.connect(band, "design", stem, "design").unwrap();
        g.connect(sweep, "operation", stem, "operation").unwrap();
        let rootlet = g.add("cad.feature").unwrap();
        g.node_mut(rootlet).unwrap().params = joined(rootlet, "Rootlet", Operation::Cylinder { radius_mm: 0.1, height_mm: 0.4 }, Placement::ring(250.0, 0.1));
        g.connect(stem, "design", rootlet, "design").unwrap();
        let along = g.add("path.along").unwrap();
        g.set_input(along, "sources", Literal::List(vec![Literal::Int(rootlet.0 as i64)])).unwrap();
        g.set_input(along, "path", Literal::Json(json!({ "feature": stem.0 }))).unwrap();
        g.set_input(along, "pitch_mm", Literal::Number(0.6)).unwrap();
        g.set_input(along, "alternate_deg", Literal::Number(180.0)).unwrap();
        let rootlets = g.add("cad.feature").unwrap();
        g.node_mut(rootlets).unwrap().params = joined(rootlets, "Rootlets", Operation::Band, Placement::Free);
        g.connect(rootlet, "design", rootlets, "design").unwrap();
        g.connect(along, "operation", rootlets, "operation").unwrap();
        let report = run(&g);
        assert!(report.errors.is_empty() && !report.any_failed(), "{:?}", report.status.iter().filter(|(_, s)| s.failed()).collect::<Vec<_>>());
        let Some(Value::Design(d)) = report.value(rootlets, "design") else { panic!() };
        let chart: Vec<[f64; 2]> = match report.value(climb, "chart") {
            Some(Value::Path(p)) => (**p).clone(),
            other => panic!("{:?}", other.map(Value::summary)),
        };
        assert_eq!((chart.len(), chart[0][0], chart[127][0]), (128, 250.0, 370.0));
        let e = ringdesign_core::cad::evaluate(d, &AlphaLibrary::default(), ringdesign_core::BuildParams::default()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let copies = e.components.iter().find(|c| c.id == rootlets.0).and_then(|c| c.made.clone()).unwrap();
        let n = copies.named.names.iter().filter_map(|n| n.split(", ").next()).collect::<std::collections::BTreeSet<_>>().len();
        eprintln!("{n} rootlet copies along the stem");
        assert!((30..120).contains(&n), "{n}");
    }

    #[test]
    fn the_array_node_reads_crest_points_and_chart_paths_and_refuses_a_count_of_one() {
        let mut g = Graph::new("along", Mode::Free);
        let n = g.add("path.along").unwrap();
        g.set_input(n, "sources", Literal::List(vec![Literal::Int(3)])).unwrap();
        g.set_input(n, "path", Literal::Json(json!({"crest": {"from_deg": 10.0, "to_deg": 170.0}}))).unwrap();
        g.set_input(n, "count", Literal::Int(7)).unwrap();
        g.set_input(n, "alternate_deg", Literal::Number(50.0)).unwrap();
        g.set_input(n, "scale_end", Literal::Number(0.6)).unwrap();
        let report = run(&g);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let op: Operation = serde_json::from_value(json_of(report.value(n, "operation"))).unwrap();
        let Operation::Pattern { sources, kind: PatternKind::Along(a) } = op else { panic!() };
        assert_eq!((&sources[..], a.count, a.alternate_deg, a.scale), (&[3][..], 7, 50.0, [1.0, 0.6]));
        assert_eq!(a.path, AlongPath::Crest { from_deg: 10.0, to_deg: 170.0 });
        for (path, want) in [
            (json!([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]), AlongPath::Points(vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])),
            (json!([[80.0, 2.0], [100.0, 2.5]]), AlongPath::Chart(vec![[80.0, 2.0], [100.0, 2.5]])),
            (json!({"feature": 9}), AlongPath::Feature(9)),
            (json!({"sketch": {"feature": 4, "entities": [7, 8]}}), AlongPath::Sketch { feature: 4, entities: vec![7, 8] }),
        ] {
            g.set_input(n, "path", Literal::Json(path)).unwrap();
            let op: Operation = serde_json::from_value(json_of(run(&g).value(n, "operation"))).unwrap();
            assert!(matches!(op, Operation::Pattern { kind: PatternKind::Along(ref a), .. } if a.path == want), "{op:?}");
        }
        g.set_input(n, "count", Literal::Int(1)).unwrap();
        let report = run(&g);
        let why = report.status.get(&n).and_then(|st| st.errors.first()).map(|(_, m)| m.clone()).unwrap_or_default();
        assert!(why.contains("A pattern holds 2 to 120 instances, its source among them, not 1"), "{why}");
    }
}
