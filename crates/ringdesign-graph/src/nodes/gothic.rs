//! Gothic tracery as Sketch feature operations: radiating lights, the pointed-arch section and the lancet arcade.

use ringdesign_core::cad::{FaceRef, Operation};
use ringdesign_core::sketch::gothic::{self, Head};
use ringdesign_core::sketch::{FaceAnchor, Sketch, Workplane};

use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

pub const LIGHTS: &str = "sketch.gothic.lights";
pub const ARCH: &str = "sketch.gothic.arch";
pub const ARCADE: &str = "sketch.gothic.arcade";

fn failed(e: anyhow::Error) -> NodeError {
    NodeError::new(format!("{e:#}"))
}

fn head(i: &Inputs, allowed: &[Head]) -> Result<Head, NodeError> {
    let name = i.text("head")?;
    Head::parse(name).filter(|h| allowed.contains(h)).ok_or_else(|| {
        let names: Vec<&str> = allowed.iter().map(|h| h.label()).collect();
        NodeError::input("head", format!("no head {name:?}; there are {}", names.join(", ")))
    })
}

fn count(i: &Inputs, pin: &str) -> Result<u32, NodeError> {
    u32::try_from(i.int(pin)?).map_err(|_| NodeError::input(pin, "a count of zero or more"))
}

/// The sketch named and laid on the `plane` feature's `face` when one is wired, as a Sketch feature's operation.
fn operation(mut sketch: Sketch, i: &Inputs) -> Result<Value, NodeError> {
    let name = i.text("name")?.trim();
    if !name.is_empty() {
        sketch.name = name.into();
    }
    if let Some(id) = i.get("plane").as_int() {
        let feature = u64::try_from(id).map_err(|_| NodeError::input("plane", "a feature id is zero or more"))?;
        let face = usize::try_from(i.int("face")?).map_err(|_| NodeError::input("face", "a face ordinal is zero or more"))?;
        sketch.plane = Workplane { on_face: Some(FaceAnchor { feature, face: FaceRef::bare(face) }), ..Workplane::default() };
    }
    serde_json::to_value(Operation::Sketch { sketch }).map(Value::from).map_err(|e| NodeError::new(e.to_string()))
}

fn lights(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let l = gothic::Lights {
        count: count(i, "count")?,
        centre: [i.number("centre_x_mm")?, i.number("centre_y_mm")?],
        phase_deg: i.number("phase_deg")?,
        sill_r_mm: i.number("sill_r_mm")?,
        apex_r_mm: i.number("apex_r_mm")?,
        bar_mm: i.number("bar_mm")?,
        width_mm: i.number("width_mm")?,
        head: head(i, &Head::ALL)?,
        clockwise: i.bool("clockwise")?,
    };
    let sketch = gothic::lights(&l).map_err(failed)?;
    let land = l.land_mm().map_err(failed)?;
    Ok(Outputs::one("operation", operation(sketch, i)?)
        .with("lights", Value::Int(i64::from(l.count)))
        .with("land_mm", if land.is_finite() { Value::Number(land) } else { Value::Null }))
}

fn arch(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let a = gothic::ArchSection {
        bore_r_mm: i.number("bore_r_mm")?,
        width_mm: i.number("width_mm")?,
        thickness_mm: i.number("thickness_mm")?,
        keel: i.number("keel")?,
        fillet_mm: i.number("fillet_mm")?,
        comfort_mm: i.number("comfort_mm")?,
    };
    let sketch = gothic::arch_section(&a).map_err(failed)?;
    Ok(Outputs::one("operation", operation(sketch, i)?))
}

fn arcade(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let a = gothic::Arcade {
        bays: count(i, "bays")?,
        width_mm: i.number("width_mm")?,
        height_mm: i.number("height_mm")?,
        bar_mm: i.number("bar_mm")?,
        sill_mm: i.number("sill_mm")?,
        head: head(i, &[Head::Pointed, Head::Round, Head::Trefoil])?,
    };
    let sketch = gothic::arcade(&a, [i.number("x_mm")?, i.number("y_mm")?], i.number("turn_deg")?).map_err(failed)?;
    let outline = match gothic::arcade_outline(&a, ringdesign_core::outline::STEP) {
        Ok(points) => Value::from(points),
        Err(e) => {
            ctx.warn(format!("outline: {e:#}"));
            Value::Null
        }
    };
    Ok(Outputs::one("operation", operation(sketch, i)?).with("outline", outline).with("bay_mm", Value::Number(a.bay_mm())))
}

/// The pins every Gothic sketch carries: its name, and the feature and face it lies on.
fn placed(spec: NodeSpec, name: &str) -> NodeSpec {
    spec.input(PinSpec::item("plane", ValueKind::Int).optional().doc("The work plane or part the sketch lies on, by feature id; unset lies on the world plane."))
        .input(PinSpec::item("face", ValueKind::Int).default(0i64).doc("Which planar face of that feature; a work plane has one, 0."))
        .input(PinSpec::item("name", ValueKind::Text).default(name).widget(Widget::TextLine).doc("The sketch's name."))
}

fn mm(pin: &str, default: f64, max: f64, doc: &str) -> PinSpec {
    PinSpec::item(pin, ValueKind::Number).default(default).widget(Widget::Mm { min: 0.0, max }).doc(doc)
}

fn offset(pin: &str, default: f64, doc: &str) -> PinSpec {
    PinSpec::item(pin, ValueKind::Number).default(default).widget(Widget::Mm { min: -30.0, max: 30.0 }).doc(doc)
}

pub fn register(reg: &mut Registry) {
    let d = gothic::Lights::default();
    let heads = |list: &[Head]| list.iter().map(|h| h.label().to_string()).collect::<Vec<_>>();
    let spec = NodeSpec::new(LIGHTS, "Radiating lights", Category::Assembly)
        .doc("Gothic lights radiating from a centre, a closed loop each: a wheel window round the finger, or a rose on a table. Each light stands in its cell of the net its mullions divide, from its sill radius to its apex radius, a bar of metal from its neighbours; the head is Pointed (a drop arch where the light is short), Round, Trefoil, or the whole light a Mouchette leaning round the rose. Wire the operation into a CAD feature.")
        .input(PinSpec::item("count", ValueKind::Int).default(i64::from(d.count)).doc("How many lights round the centre."))
        .input(mm("sill_r_mm", d.sill_r_mm, 30.0, "Radius of each light's sill, mm."))
        .input(mm("apex_r_mm", d.apex_r_mm, 30.0, "Radius of each light's apex, mm."))
        .input(mm("bar_mm", d.bar_mm, 3.0, "Metal between neighbouring lights, mm."))
        .input(mm("width_mm", d.width_mm, 10.0, "0 fills each cell, the jambs parallel to its mullions; above 0, every light is parallel-sided at this width, mm."))
        .input(PinSpec::select("head", heads(&Head::ALL)).default(d.head.label()).doc("How each light's head closes."))
        .input(PinSpec::item("phase_deg", ValueKind::Number).default(d.phase_deg).widget(Widget::Angle).doc("The first light's axis, degrees counter-clockwise from the sketch's x."))
        .input(PinSpec::item("clockwise", ValueKind::Bool).default(d.clockwise).doc("Mouchettes lean clockwise instead of counter-clockwise."))
        .input(offset("centre_x_mm", d.centre[0], "Where the lights radiate from, along the sketch's x, mm."))
        .input(offset("centre_y_mm", d.centre[1], "Where the lights radiate from, along the sketch's y, mm."));
    let spec = placed(spec, "Radiating lights")
        .output(PinSpec::item("operation", ValueKind::Json).doc("The Sketch feature's operation."))
        .output(PinSpec::item("lights", ValueKind::Int).doc("How many lights were drawn."))
        .output(PinSpec::item("land_mm", ValueKind::Number).doc("The narrowest metal between two neighbouring lights, mm; none for one light."))
        .eval(lights);
    reg.register(spec).expect("unique");

    let a = gothic::ArchSection::default();
    let spec = NodeSpec::new(ARCH, "Pointed-arch section", Category::Assembly)
        .doc("A ring's cross-section as a blunt lancet standing on the bore, in the plane through the finger's axis: a comfort bore, filleted corners, straight feet that are side faces, and two flanks keeled on the parting plane. Every section is a single crest falling to vertical feet, so the ring it revolves into pulls from two-part sand. Wire the operation into a CAD feature and revolve it about the finger.")
        .input(mm("bore_r_mm", a.bore_r_mm, 15.0, "The bore's radius at its tightest, on the parting plane, mm."))
        .input(mm("width_mm", a.width_mm, 15.0, "Across the band, mm."))
        .input(mm("thickness_mm", a.thickness_mm, 10.0, "From the bore's tightest point to the keel, mm."))
        .input(PinSpec::item("keel", ValueKind::Number).default(a.keel).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("1 springs the flanks from the bore corners into a sharp keel; 0 stands the feet straight up until the head is a round arch."))
        .input(mm("fillet_mm", a.fillet_mm, 2.0, "Radius rounding each bore corner, mm; 0 leaves it sharp."))
        .input(mm("comfort_mm", a.comfort_mm, 1.0, "How much wider the bore is at the band's edges than at its middle, mm."));
    let spec = placed(spec, "Pointed-arch section").output(PinSpec::item("operation", ValueKind::Json).doc("The Sketch feature's operation, on the section plane unless a plane is wired.")).eval(arch);
    reg.register(spec).expect("unique");

    let c = gothic::Arcade::default();
    let spec = NodeSpec::new(ARCADE, "Lancet arcade", Category::Assembly)
        .doc("A row of lancet bays standing on a sill, centred on the sketch's origin: one closed loop per bay to cut as niches, and the whole arcade on its sill as one stamp outline. Wire the operation into a CAD feature, or the outline into a stamp.")
        .input(PinSpec::item("bays", ValueKind::Int).default(i64::from(c.bays)).doc("How many bays."))
        .input(mm("width_mm", c.width_mm, 30.0, "Across every bay and the posts between them, mm."))
        .input(mm("height_mm", c.height_mm, 20.0, "From the sill's foot to the apexes, mm."))
        .input(mm("bar_mm", c.bar_mm, 3.0, "Width of each post between two bays, mm."))
        .input(mm("sill_mm", c.sill_mm, 5.0, "Height of the sill the bays stand on, mm."))
        .input(PinSpec::select("head", heads(&[Head::Pointed, Head::Round, Head::Trefoil])).default(c.head.label()).doc("How each bay's head closes; Trefoil cusps it."))
        .input(offset("x_mm", 0.0, "Where the arcade's centre sits along the sketch's x, mm."))
        .input(offset("y_mm", 0.0, "Where the arcade's centre sits along the sketch's y, mm."))
        .input(PinSpec::item("turn_deg", ValueKind::Number).default(0.0).widget(Widget::Angle).doc("Turn about the arcade's centre, degrees counter-clockwise."));
    let spec = placed(spec, "Lancet arcade")
        .output(PinSpec::item("operation", ValueKind::Json).doc("The Sketch feature's operation: one loop per bay."))
        .output(PinSpec::item("outline", ValueKind::Path).doc("The bays on their sill as one closed stamp outline about the origin, mm; none without a sill under several bays."))
        .output(PinSpec::item("bay_mm", ValueKind::Number).doc("Width of one bay between its jambs, mm."))
        .eval(arcade);
    reg.register(spec).expect("unique");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets};
    use crate::graph::Graph;
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    fn sketch_of(v: Option<&Value>) -> Sketch {
        let json = v.and_then(Value::to_json_any).expect("an operation");
        serde_json::from_value(json["Sketch"]["sketch"].clone()).expect("a Sketch operation")
    }

    #[test]
    fn each_node_draws_what_its_core_construction_draws_and_lies_on_the_plane_it_names() {
        let reg = Registry::builtin();
        let mut g = Graph::default();
        let l = g.add(LIGHTS).unwrap();
        let a = g.add(ARCH).unwrap();
        let c = g.add(ARCADE).unwrap();
        g.set_input(c, "plane", Literal::Int(7)).unwrap();
        g.set_input(c, "face", Literal::Int(2)).unwrap();
        g.set_input(c, "name", Literal::Text("Bays".into())).unwrap();
        assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
        let r = Evaluator::new().evaluate(&g, &reg, &AlphaLibrary::default(), 0, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        let mut want = gothic::lights(&gothic::Lights::default()).unwrap();
        assert_eq!(sketch_of(r.value(l, "operation")), want);
        assert_eq!(r.value(l, "lights"), Some(&Value::Int(24)));
        assert!((r.value(l, "land_mm").and_then(Value::as_number).unwrap() - 0.9).abs() < 1e-6);
        want = gothic::arch_section(&gothic::ArchSection::default()).unwrap();
        assert_eq!(sketch_of(r.value(a, "operation")), want);
        let arcade = gothic::Arcade::default();
        want = gothic::arcade(&arcade, [0.0, 0.0], 0.0).unwrap();
        want.name = "Bays".into();
        want.plane = Workplane { on_face: Some(FaceAnchor { feature: 7, face: FaceRef::bare(2) }), ..Workplane::default() };
        assert_eq!(sketch_of(r.value(c, "operation")), want);
        assert_eq!(r.value(c, "outline"), Some(&Value::from(gothic::arcade_outline(&arcade, ringdesign_core::outline::STEP).unwrap())));
        // A head the arcade cannot take, and lights the net cannot hold, are refused by name.
        g.set_input(c, "head", Literal::Text("Mouchette".into())).unwrap();
        g.set_input(l, "count", Literal::Int(200)).unwrap();
        let r = Evaluator::new().evaluate(&g, &reg, &AlphaLibrary::default(), 0, Targets::AllPure);
        assert!(r.status[&c].errors[0].1.contains("Pointed, Round, Trefoil"), "{:?}", r.status[&c].errors);
        assert!(r.status[&l].errors[0].1.contains("1 to 120"), "{:?}", r.status[&l].errors);
    }
}
