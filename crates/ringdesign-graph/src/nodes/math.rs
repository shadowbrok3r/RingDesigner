//! Arithmetic, trigonometry in degrees, comparison and logic.

use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::ValueKind;

fn finite(name: &str, x: f64) -> Result<f64, NodeError> {
    if x.is_finite() { Ok(x) } else { Err(NodeError::input(name, "the result is not a finite number")) }
}

macro_rules! binary {
    ($($f:ident, $key:literal, $label:literal, $doc:literal, |$a:ident, $b:ident| $body:expr;)*) => {
        $(
            fn $f(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
                let $a = i.number("a")?;
                let $b = i.number("b")?;
                let out: Result<f64, NodeError> = $body;
                Ok(Outputs::one("out", finite("out", out?)?))
            }
        )*
        fn binary_specs() -> Vec<NodeSpec> {
            vec![$(
                NodeSpec::new($key, $label, Category::Util)
                    .doc($doc)
                    .input(PinSpec::item("a", ValueKind::Number).default(0.0).doc("First operand."))
                    .input(PinSpec::item("b", ValueKind::Number).default(0.0).doc("Second operand."))
                    .output(PinSpec::item("out", ValueKind::Number).doc("The result."))
                    .eval($f),
            )*]
        }
    };
}

binary! {
    add, "math.add", "Add", "a + b.", |a, b| Ok(a + b);
    sub, "math.sub", "Subtract", "a − b.", |a, b| Ok(a - b);
    mul, "math.mul", "Multiply", "a × b.", |a, b| Ok(a * b);
    div, "math.div", "Divide", "a ÷ b; b must not be zero.", |a, b| if b == 0.0 { Err(NodeError::input("b", "division by zero")) } else { Ok(a / b) };
    pow, "math.pow", "Power", "a raised to b.", |a, b| Ok(a.powf(b));
    modulo, "math.mod", "Modulo", "a modulo b, with the sign of b (so −30 mod 360 is 330).", |a, b| if b == 0.0 { Err(NodeError::input("b", "modulo by zero")) } else { Ok(((a % b) + b) % b) };
    min, "math.min", "Minimum", "The smaller of a and b.", |a, b| Ok(a.min(b));
    max, "math.max", "Maximum", "The larger of a and b.", |a, b| Ok(a.max(b));
    atan2, "math.atan2", "Atan2", "The bearing of (a, b) = (y, x) in degrees.", |a, b| Ok(a.atan2(b).to_degrees());
}

macro_rules! unary {
    ($($f:ident, $key:literal, $label:literal, $doc:literal, |$x:ident| $body:expr;)*) => {
        $(
            fn $f(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
                let $x = i.number("x")?;
                let out: Result<f64, NodeError> = $body;
                Ok(Outputs::one("out", finite("out", out?)?))
            }
        )*
        fn unary_specs() -> Vec<NodeSpec> {
            vec![$(
                NodeSpec::new($key, $label, Category::Util)
                    .doc($doc)
                    .input(PinSpec::item("x", ValueKind::Number).default(0.0).doc("The operand."))
                    .output(PinSpec::item("out", ValueKind::Number).doc("The result."))
                    .eval($f),
            )*]
        }
    };
}

unary! {
    neg, "math.neg", "Negate", "−x.", |x| Ok(-x);
    abs, "math.abs", "Absolute", "|x|.", |x| Ok(x.abs());
    floor, "math.floor", "Floor", "x rounded down.", |x| Ok(x.floor());
    ceil, "math.ceil", "Ceiling", "x rounded up.", |x| Ok(x.ceil());
    round, "math.round", "Round", "x rounded to the nearest whole number.", |x| Ok(x.round());
    sqrt, "math.sqrt", "Square root", "√x; x must not be negative.", |x| if x < 0.0 { Err(NodeError::input("x", "negative")) } else { Ok(x.sqrt()) };
    sin, "math.sin", "Sine", "sin x, x in degrees.", |x| Ok(x.to_radians().sin());
    cos, "math.cos", "Cosine", "cos x, x in degrees.", |x| Ok(x.to_radians().cos());
    tan, "math.tan", "Tangent", "tan x, x in degrees.", |x| Ok(x.to_radians().tan());
    deg, "math.deg", "To degrees", "Radians to degrees.", |x| Ok(x.to_degrees());
    rad, "math.rad", "To radians", "Degrees to radians.", |x| Ok(x.to_radians());
}

fn lerp(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (a, b, t) = (i.number("a")?, i.number("b")?, i.number("t")?);
    Ok(Outputs::one("out", finite("out", a + (b - a) * t)?))
}

fn clamp(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (x, lo, hi) = (i.number("x")?, i.number("min")?, i.number("max")?);
    if lo > hi {
        return Err(NodeError::input("min", format!("{lo} is above max {hi}")));
    }
    Ok(Outputs::one("out", x.clamp(lo, hi)))
}

fn remap(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (x, a, b, c, d) = (i.number("x")?, i.number("from_min")?, i.number("from_max")?, i.number("to_min")?, i.number("to_max")?);
    if a == b {
        return Err(NodeError::input("from_max", "the source range is empty"));
    }
    Ok(Outputs::one("out", finite("out", c + (d - c) * (x - a) / (b - a))?))
}

fn compare(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let (a, b) = (i.number("a")?, i.number("b")?);
    let op = i.text("op")?;
    let out = match op {
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        ">=" => a >= b,
        "=" | "==" => a == b,
        "!=" => a != b,
        other => return Err(NodeError::input("op", format!("{other:?} is not one of < <= > >= = !="))),
    };
    Ok(Outputs::one("out", out))
}

fn not(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", !i.bool("x")?))
}

fn and(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", i.bool("a")? && i.bool("b")?))
}

fn or(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    Ok(Outputs::one("out", i.bool("a")? || i.bool("b")?))
}

pub fn register(reg: &mut Registry) {
    for s in binary_specs().into_iter().chain(unary_specs()) {
        reg.register(s).expect("unique");
    }
    let specs = [
        NodeSpec::new("math.lerp", "Lerp", Category::Util)
            .doc("a + (b − a)·t: the blend of a and b at t.")
            .input(PinSpec::item("a", ValueKind::Number).default(0.0).doc("The value at t = 0."))
            .input(PinSpec::item("b", ValueKind::Number).default(1.0).doc("The value at t = 1."))
            .input(PinSpec::item("t", ValueKind::Number).default(0.5).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("The blend."))
            .output(PinSpec::item("out", ValueKind::Number).doc("The blend."))
            .eval(lerp),
        NodeSpec::new("math.clamp", "Clamp", Category::Util)
            .doc("x held between min and max.")
            .input(PinSpec::item("x", ValueKind::Number).default(0.0).doc("The value."))
            .input(PinSpec::item("min", ValueKind::Number).default(0.0).doc("The floor."))
            .input(PinSpec::item("max", ValueKind::Number).default(1.0).doc("The ceiling."))
            .output(PinSpec::item("out", ValueKind::Number).doc("The held value."))
            .eval(clamp),
        NodeSpec::new("math.remap", "Remap", Category::Util)
            .doc("x carried from one range to another, linearly and without clamping.")
            .input(PinSpec::item("x", ValueKind::Number).default(0.0).doc("The value."))
            .input(PinSpec::item("from_min", ValueKind::Number).default(0.0).doc("The source range's start."))
            .input(PinSpec::item("from_max", ValueKind::Number).default(1.0).doc("The source range's end."))
            .input(PinSpec::item("to_min", ValueKind::Number).default(0.0).doc("The target range's start."))
            .input(PinSpec::item("to_max", ValueKind::Number).default(1.0).doc("The target range's end."))
            .output(PinSpec::item("out", ValueKind::Number).doc("The carried value."))
            .eval(remap),
        NodeSpec::new("math.compare", "Compare", Category::Util)
            .doc("a compared with b by op: < <= > >= = !=.")
            .input(PinSpec::item("a", ValueKind::Number).default(0.0).doc("Left side."))
            .input(PinSpec::item("b", ValueKind::Number).default(0.0).doc("Right side."))
            .input(
                PinSpec::item("op", ValueKind::Text)
                    .default("<")
                    .widget(Widget::Select(["<", "<=", ">", ">=", "=", "!="].iter().map(|s| s.to_string()).collect()))
                    .doc("The comparison."),
            )
            .output(PinSpec::item("out", ValueKind::Bool).doc("Whether it holds."))
            .eval(compare),
        NodeSpec::new("logic.not", "Not", Category::Util)
            .doc("The opposite of x.")
            .input(PinSpec::item("x", ValueKind::Bool).default(false).doc("The value."))
            .output(PinSpec::item("out", ValueKind::Bool).doc("Not x."))
            .eval(not),
        NodeSpec::new("logic.and", "And", Category::Util)
            .doc("Both a and b.")
            .input(PinSpec::item("a", ValueKind::Bool).default(false).doc("First."))
            .input(PinSpec::item("b", ValueKind::Bool).default(false).doc("Second."))
            .output(PinSpec::item("out", ValueKind::Bool).doc("a and b."))
            .eval(and),
        NodeSpec::new("logic.or", "Or", Category::Util)
            .doc("Either a or b.")
            .input(PinSpec::item("a", ValueKind::Bool).default(false).doc("First."))
            .input(PinSpec::item("b", ValueKind::Bool).default(false).doc("Second."))
            .output(PinSpec::item("out", ValueKind::Bool).doc("a or b."))
            .eval(or),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}
