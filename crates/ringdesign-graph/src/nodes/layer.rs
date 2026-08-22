//! Layers, their fitters, and the entry that places one in a stack.

use std::sync::Arc;

use ringdesign_core::curve::{CurveLayer, WireProfile};
use ringdesign_core::field::{
    Blend, BorderLayer, BorderProfile, Decal, DecalLayer, FieldContext, FluteProfile, FlutesLayer, GroupLayer, MilgrainLayer,
    OpenworkLayer, Remap, SeatPadLayer, SeatRunLayer, SeatStyle, SideFacePick, SignetLayer, SignetOutline, VGate,
};
use ringdesign_core::profile::DropCurve;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG;
use ringdesign_core::{Layer, LayerEntry, LayerStack, RingDesign, Window};

use super::structs::{StructNode, enum_names};
use crate::graph::Node;
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind, layer_label};

macro_rules! layer_variant {
    ($wrap:ident, $unwrap:ident, $ty:ty, $variant:ident) => {
        fn $wrap(t: $ty) -> Value {
            Value::Layer(Arc::new(Layer::$variant(t)))
        }
        fn $unwrap(v: &Value) -> Option<$ty> {
            match v {
                Value::Layer(l) => match &**l {
                    Layer::$variant(t) => Some(t.clone()),
                    _ => None,
                },
                _ => None,
            }
        }
    };
}

layer_variant!(wrap_tiling, unwrap_tiling, TilingLayer, Tiling);
layer_variant!(wrap_border, unwrap_border, BorderLayer, Border);
layer_variant!(wrap_milgrain, unwrap_milgrain, MilgrainLayer, Milgrain);
layer_variant!(wrap_seat, unwrap_seat, SeatPadLayer, SeatPad);
layer_variant!(wrap_seatrun, unwrap_seatrun, SeatRunLayer, SeatRun);
layer_variant!(wrap_signet, unwrap_signet, SignetLayer, Signet);
layer_variant!(wrap_curve, unwrap_curve, CurveLayer, Curve);
layer_variant!(wrap_flutes, unwrap_flutes, FlutesLayer, Flutes);
layer_variant!(wrap_decals, unwrap_decals, DecalLayer, Decals);
layer_variant!(wrap_openwork, unwrap_openwork, OpenworkLayer, Openwork);

fn ctx_of(i: &Inputs, pin: &str) -> Result<FieldContext, NodeError> {
    match i.get(pin) {
        Value::Design(d) => Ok(d.field_context()),
        other => Err(NodeError::input(pin, format!("expected a design, got {}", other.summary()))),
    }
}

fn tiling_default() -> TilingLayer {
    TilingLayer::default_for("Scales", &RingDesign::default().field_context())
}

fn openwork_default() -> OpenworkLayer {
    OpenworkLayer { tiling: tiling_default(), depth_mm: 1.2, keep_mm: 0.8 }
}

fn entry_default() -> LayerEntry {
    LayerEntry::new("", Layer::Milgrain(MilgrainLayer::default()))
}

fn tiling_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.tiling", "Tiling", Category::Layer)
            .doc("An alpha tiled round the ring: an integer count around so the joint closes, rows across, and the relief's height. layer.tiling.fit sizes one to a design's side faces."),
        "layer",
        tiling_default,
        wrap_tiling,
        unwrap_tiling,
    )
    .base("layer", ValueKind::Layer, "Start from this tiling; Scales on the default band otherwise.")
    .field(PinSpec::item("alpha", ValueKind::AlphaRef).doc("The alpha's library name."))
    .field(PinSpec::item("repeats_around", ValueKind::Int).doc("Tiles round the ring; an integer, so the pattern closes."))
    .field(PinSpec::item("rows", ValueKind::Int).doc("Tiles across the band."))
    .field(PinSpec::item("v_center_mm", ValueKind::Number).doc("Where across the band the tiling centres, mm of section arc."))
    .field(PinSpec::item("v_span_mm", ValueKind::Number).doc("How much of the band it covers, mm of section arc."))
    .field(PinSpec::item("rotation_deg", ValueKind::Number).doc("Tile rotation, degrees."))
    .field(PinSpec::item("offset_u", ValueKind::Number).doc("Phase round the ring, in tiles."))
    .field(PinSpec::item("offset_v", ValueKind::Number).doc("Phase across the band, in tiles."))
    .field(PinSpec::item("height_mm", ValueKind::Number).widget(Widget::Mm { min: 0.0, max: 2.0 }).doc("Relief height, mm."))
    .field(PinSpec::item("gap_mm", ValueKind::Number).doc("Gap between tiles, mm."))
    .field(PinSpec::item("stagger", ValueKind::Number).doc("Row stagger, in tiles."))
    .field(PinSpec::item("mirror_alternate_u", ValueKind::Bool).doc("Mirror every other tile round the ring."))
    .field(PinSpec::item("mirror_alternate_v", ValueKind::Bool).doc("Mirror every other row."))
    .field(PinSpec::item("contrast", ValueKind::Number).doc("Alpha contrast."))
    .field(PinSpec::item("bias", ValueKind::Number).doc("Alpha bias."))
    .field(PinSpec::item("invert", ValueKind::Bool).doc("Invert the alpha."))
    .field(PinSpec::item("feather_mm", ValueKind::Number).doc("Soft edge, mm."))
    .field(PinSpec::item("continuous", ValueKind::Bool).doc("Sample continuously instead of per cell."))
    .field(PinSpec::item("mirror_v", ValueKind::Bool).doc("Mirror onto the other side face too."))
    .field(PinSpec::item("edge_mm", ValueKind::Number).doc("Bevel width from the alpha's distance field, mm."))
    .field(PinSpec::item("shear", ValueKind::Number).doc("Helix shear round the ring."))
    .field(PinSpec::item("kfold", ValueKind::Int).doc("k-fold kaleidoscope in u; 0 is off."))
    .hidden(&["warp"])
    .build()
}

fn tiling_fit(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let fc = ctx_of(i, "design")?;
    let alpha = i.text("alpha")?.to_string();
    if ctx.lib.get(&alpha).is_none() {
        ctx.warn(format!("no alpha named {alpha:?} in the library now; a source assembled into the design bakes one on load"));
    }
    let mut t = TilingLayer::default_for(alpha, &fc);
    if i.bool("side_faces")? {
        let draft = i.number("min_draft_deg")?;
        if !t.fit_to_side_faces(&fc, draft) {
            ctx.warn(format!("the band has no side face at {draft:.0}°; the tiling stays where default_for put it"));
        }
    }
    if i.bool("square_cells")? {
        t.repeats_around = t.repeats_for_square_cells(&fc).max(1);
    }
    Ok(Outputs::one("layer", wrap_tiling(t)))
}

fn border_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.border", "Border", Category::Layer).doc("A rail round the ring at one v: round, flat, knife, step or rope."),
        "layer",
        BorderLayer::default,
        wrap_border,
        unwrap_border,
    )
    .base("layer", ValueKind::Layer, "Start from this border.")
    .field(PinSpec::item("v_mm", ValueKind::Number).doc("Where across the band, mm of section arc."))
    .field(PinSpec::item("width_mm", ValueKind::Number).doc("Rail width, mm."))
    .field(PinSpec::item("height_mm", ValueKind::Number).doc("Rail height, mm."))
    .field(PinSpec::select("profile", enum_names(BorderProfile::ALL)).doc("The rail's section."))
    .field(PinSpec::item("mirror", ValueKind::Bool).doc("A second rail mirrored across the band."))
    .field(PinSpec::item("rope_twists", ValueKind::Int).doc("Twists round the ring for a rope rail; an integer."))
    .build()
}

fn milgrain_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.milgrain", "Milgrain", Category::Layer).doc("A row of beads round the ring at one v."),
        "layer",
        MilgrainLayer::default,
        wrap_milgrain,
        unwrap_milgrain,
    )
    .base("layer", ValueKind::Layer, "Start from this milgrain.")
    .field(PinSpec::item("v_mm", ValueKind::Number).doc("Where across the band, mm of section arc."))
    .field(PinSpec::item("bead_diameter_mm", ValueKind::Number).doc("Bead diameter, mm."))
    .field(PinSpec::item("beads_around", ValueKind::Int).doc("Beads round the ring; an integer."))
    .field(PinSpec::item("height_mm", ValueKind::Number).doc("Bead height, mm."))
    .field(PinSpec::item("mirror", ValueKind::Bool).doc("A second row mirrored across the band."))
    .build()
}

fn seat_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.seat", "Seat", Category::Layer).doc("Stock for one stone: a boss, a bezel collar or a gypsy mound, with the stone's own plan. layer.seat.fit sizes one to a gem."),
        "layer",
        SeatPadLayer::default,
        wrap_seat,
        unwrap_seat,
    )
    .base("layer", ValueKind::Layer, "Start from this seat.")
    .field(PinSpec::item("theta_deg", ValueKind::Number).widget(Widget::Angle).doc("Where round the ring; 90° is the top."))
    .field(PinSpec::item("v_mm", ValueKind::Number).doc("Where across the band, mm of section arc."))
    .field(PinSpec::item("diameter_mm", ValueKind::Number).doc("The seat's short axis, mm."))
    .field(PinSpec::item("height_mm", ValueKind::Number).doc("Stand-off over the band, mm."))
    .field(PinSpec::item("crown", ValueKind::Number).doc("How domed the mound is, 0..1."))
    .field(PinSpec::item("blend_mm", ValueKind::Number).doc("Skirt width, mm."))
    .field(PinSpec::select("style", enum_names(SeatStyle::ALL)).doc("Boss, bezel or gypsy mound."))
    .field(PinSpec::item("bezel_wall_mm", ValueKind::Number).doc("Bezel wall, mm."))
    .field(PinSpec::item("recess_mm", ValueKind::Number).doc("Bezel recess, mm."))
    .field(PinSpec::item("bezel_lip", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 0.8 }).doc("How far up the crown the collar stands, fraction; fit derives the height."))
    .field(PinSpec::item("bezel_bearing_mm", ValueKind::Number).doc("The girdle's ledge inside the wall, mm."))
    .field(PinSpec::item("prongs", ValueKind::Int).doc("Prong bumps; 0 for none."))
    .field(PinSpec::item("prong_mm", ValueKind::Number).doc("Prong diameter, mm."))
    .field(PinSpec::item("gem", ValueKind::Gem).doc("The stone this seat carries."))
    .field(PinSpec::item("dimple_mm", ValueKind::Number).doc("Bur dimple depth, mm."))
    .field(PinSpec::item("elong", ValueKind::Number).doc("Long over short axis."))
    .field(PinSpec::item("rot_deg", ValueKind::Number).doc("Bearing in the chart, degrees."))
    .field(PinSpec::item("plan_pow", ValueKind::Number).doc("Superellipse exponent of the plan; 2 is an ellipse."))
    .field(PinSpec::item("set_depth_mm", ValueKind::Number).doc("Girdle depth below the pad's top, mm; the style's own if unset."))
    .build()
}

fn seat_fit(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut seat = match i.get("layer") {
        Value::Null => SeatPadLayer::default(),
        v => unwrap_seat(v).ok_or_else(|| NodeError::input("layer", format!("expected a seat layer, got {}", v.summary())))?,
    };
    let gem = match i.get("gem") {
        Value::Gem(g) => *g,
        other => return Err(NodeError::input("gem", format!("expected a gem, got {}", other.summary()))),
    };
    seat.fit_stone(gem);
    Ok(Outputs::one("layer", wrap_seat(seat)))
}

fn seatrun_seat(t: &mut SeatRunLayer, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    match i.get("seat") {
        Value::Null => Ok(()),
        v => {
            t.seat = unwrap_seat(v).ok_or_else(|| NodeError::input("seat", format!("expected a seat layer, got {}", v.summary())))?;
            Ok(())
        }
    }
}

fn seatrun_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.seatrun", "Seat run", Category::Layer)
            .doc("A row of seats round the ring: a prototype seat, a stone, a count and the bridge between; graded by taper. layer.seatrun.solve fits the count to the band."),
        "layer",
        SeatRunLayer::default,
        wrap_seatrun,
        unwrap_seatrun,
    )
    .base("layer", ValueKind::Layer, "Start from this run.")
    .extra(PinSpec::item("seat", ValueKind::Layer).doc("The prototype seat, a seat layer."))
    .field(PinSpec::item("count", ValueKind::Int).doc("Stones round the ring; an integer."))
    .field(PinSpec::item("gem", ValueKind::Gem).doc("The stone at every station."))
    .field(PinSpec::item("bridge_mm", ValueKind::Number).doc("Metal between neighbours, mm."))
    .field(PinSpec::item("taper", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 0.85 }).doc("Graduation toward the far side, 0..0.85."))
    .field(PinSpec::item("taper_theta_deg", ValueKind::Number).widget(Widget::Angle).doc("Where the largest stone sits."))
    .field(PinSpec::item("shared_prong_mm", ValueKind::Number).doc("Shared prong post diameter, mm; 0 for none."))
    .field(PinSpec::item("tilt_deg", ValueKind::Number).widget(Widget::Angle).doc("Every stone turned in plan, degrees; 45 sets a square on the diagonal."))
    .hidden(&["seat"])
    .prepare(seatrun_seat)
    .build()
}

fn seatrun_solve(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let fc = ctx_of(i, "design")?;
    let mut run = match i.get("layer") {
        Value::Null => SeatRunLayer::default(),
        v => unwrap_seatrun(v).ok_or_else(|| NodeError::input("layer", format!("expected a seat run, got {}", v.summary())))?,
    };
    run.solve_spacing(&fc);
    Ok(Outputs::one("layer", wrap_seatrun(run.clone())).with("count", i64::from(run.count)).with("bridge_mm", run.bridge_at(&fc)))
}

fn signet_layer_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.signet", "Signet pad", Category::Layer)
            .doc("A flat facet standing on the band as a pad. For a signet, make the head the band instead (shank.signet)."),
        "layer",
        SignetLayer::default,
        wrap_signet,
        unwrap_signet,
    )
    .base("layer", ValueKind::Layer, "Start from this pad.")
    .field(PinSpec::item("theta_deg", ValueKind::Number).widget(Widget::Angle).doc("Where round the ring."))
    .field(PinSpec::item("v_mm", ValueKind::Number).doc("Where across the band, mm of section arc."))
    .field(PinSpec::select("outline", enum_names(SignetOutline::ALL)).doc("The pad's plan."))
    .field(PinSpec::item("length_mm", ValueKind::Number).doc("Along the ring, mm."))
    .field(PinSpec::item("width_mm", ValueKind::Number).doc("Across the band, mm."))
    .field(PinSpec::item("height_mm", ValueKind::Number).doc("Stand-off, mm."))
    .field(PinSpec::item("top_flat", ValueKind::Number).doc("How flat the top is, 0..1."))
    .field(PinSpec::item("shoulder_mm", ValueKind::Number).doc("Shoulder blend, mm."))
    .field(PinSpec::item("rotation_deg", ValueKind::Number).doc("Plan rotation, degrees."))
    .build()
}

fn curve_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.curve", "Wire", Category::Layer).doc("A wire swept along a point list in the (u, v) chart, repeated round the ring."),
        "layer",
        CurveLayer::default,
        wrap_curve,
        unwrap_curve,
    )
    .base("layer", ValueKind::Layer, "Start from this wire.")
    .field(PinSpec::item("points", ValueKind::Path).doc("The guide, [u, v] pairs in mm of the chart."))
    .field(PinSpec::item("repeats_around", ValueKind::Int).doc("Copies round the ring; an integer."))
    .field(PinSpec::item("closed", ValueKind::Bool).doc("Close the guide on itself."))
    .field(PinSpec::item("width_mm", ValueKind::Number).doc("Wire width, mm."))
    .field(PinSpec::item("height_mm", ValueKind::Number).doc("Wire height, mm."))
    .field(PinSpec::select("profile", enum_names(WireProfile::ALL)).doc("The wire's section."))
    .field(PinSpec::item("taper", ValueKind::Number).doc("Thinning toward the ends, 0..1."))
    .field(PinSpec::item("mirror_v", ValueKind::Bool).doc("Mirror onto the other side face."))
    .build()
}

fn curve_preset(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let fc = ctx_of(i, "design")?;
    let l = match i.text("preset")? {
        "scroll" => CurveLayer::preset_scroll(&fc),
        "vine" => CurveLayer::preset_vine(&fc),
        "wave_rail" => CurveLayer::preset_wave_rail(&fc),
        other => return Err(NodeError::input("preset", format!("{other:?} is not scroll, vine or wave_rail"))),
    };
    Ok(Outputs::one("layer", wrap_curve(l)))
}

fn flutes_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.flutes", "Flutes", Category::Layer).doc("Parametric flutes or reeds: a count round the ring, a section, a lean."),
        "layer",
        FlutesLayer::default,
        wrap_flutes,
        unwrap_flutes,
    )
    .base("layer", ValueKind::Layer, "Start from these flutes.")
    .field(PinSpec::item("count", ValueKind::Int).doc("Flutes round the ring; an integer."))
    .field(PinSpec::select("profile", enum_names(FluteProfile::ALL)).doc("Round, vee or square."))
    .field(PinSpec::item("width_mm", ValueKind::Number).doc("Flute width, mm."))
    .field(PinSpec::item("height_mm", ValueKind::Number).doc("Flute depth, mm; negative carves."))
    .field(PinSpec::item("lean", ValueKind::Number).doc("Lean across the band, 0..1."))
    .field(PinSpec::item("along", ValueKind::Bool).doc("Run along the ring instead of across."))
    .build()
}

fn decals_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.decals", "Decals", Category::Layer).doc("Free-placed stamps of one alpha; each stamp from decal.stamp."),
        "layer",
        DecalLayer::default,
        wrap_decals,
        unwrap_decals,
    )
    .base("layer", ValueKind::Layer, "Start from this decal layer.")
    .field(PinSpec::item("alpha", ValueKind::AlphaRef).doc("The alpha's library name."))
    .field(PinSpec::list("decals", ValueKind::Json).doc("The stamps, from decal.stamp."))
    .field(PinSpec::item("feather_mm", ValueKind::Number).doc("Soft edge, mm."))
    .field(PinSpec::item("invert", ValueKind::Bool).doc("Invert the alpha."))
    .build()
}

fn decal_stamp(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = Decal {
        theta_deg: i.number("theta_deg")?,
        v_mm: i.number("v_mm")?,
        size_mm: i.number("size_mm")?,
        rotation_deg: i.number("rotation_deg")?,
        height_mm: i.number("height_mm")?,
        flip: i.bool("flip")?,
    };
    Ok(Outputs::one("stamp", serde_json::to_value(d).map_err(|e| NodeError::new(e.to_string()))?))
}

fn openwork_tiling(t: &mut OpenworkLayer, i: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    match i.get("tiling") {
        Value::Null => Ok(()),
        v => {
            t.tiling = unwrap_tiling(v).ok_or_else(|| NodeError::input("tiling", format!("expected a tiling layer, got {}", v.summary())))?;
            Ok(())
        }
    }
}

fn openwork_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("layer.openwork", "Openwork", Category::Layer).doc("A tiling's ink carved toward a floor over the bore: pierced work that keeps a wall."),
        "layer",
        openwork_default,
        wrap_openwork,
        unwrap_openwork,
    )
    .base("layer", ValueKind::Layer, "Start from this openwork.")
    .extra(PinSpec::item("tiling", ValueKind::Layer).doc("The mask, a tiling layer."))
    .field(PinSpec::item("depth_mm", ValueKind::Number).doc("Carve depth, mm."))
    .field(PinSpec::item("keep_mm", ValueKind::Number).doc("Wall kept over the bore, mm."))
    .hidden(&["tiling"])
    .prepare(openwork_tiling)
    .build()
}

fn group(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let stack = match i.get("stack") {
        Value::Stack(s) => (**s).clone(),
        Value::Null => LayerStack::default(),
        other => return Err(NodeError::input("stack", format!("expected a stack, got {}", other.summary()))),
    };
    Ok(Outputs::one("layer", Value::Layer(Arc::new(Layer::Group(GroupLayer { stack, recipe: None })))))
}

fn entry_name(e: &mut LayerEntry, _: &Inputs, _: &mut EvalCtx<'_>) -> Result<(), NodeError> {
    if e.name.trim().is_empty() {
        e.name = layer_label(&e.layer).to_string();
    }
    Ok(())
}

fn entry_node() -> NodeSpec {
    StructNode::new(
        NodeSpec::new("entry", "Entry", Category::Layer)
            .doc("A layer placed in a stack: its name, blend, opacity, softness, angular window, painted mask and remap."),
        "entry",
        entry_default,
        |e| Value::Entry(Arc::new(e)),
        |v| match v {
            Value::Entry(e) => Some((**e).clone()),
            _ => None,
        },
    )
    .base("entry", ValueKind::Entry, "Start from this entry.")
    .field(PinSpec::item("layer", ValueKind::Layer).doc("The layer."))
    .field(PinSpec::item("name", ValueKind::Text).widget(Widget::TextLine).doc("Its name; the layer's kind if left empty."))
    .field(PinSpec::item("enabled", ValueKind::Bool).doc("Whether it contributes."))
    .field(PinSpec::select("blend", enum_names(Blend::ALL)).doc("How it combines with what is under it."))
    .field(PinSpec::item("opacity", ValueKind::Number).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("Strength, 0..1."))
    .field(PinSpec::item("soft_mm", ValueKind::Number).doc("Blur radius, mm."))
    .field(PinSpec::item("window", ValueKind::Window).doc("The angular window, from window."))
    .field(PinSpec::item("mask", ValueKind::AlphaRef).doc("A painted mask's alpha name."))
    .field(PinSpec::item("remap", ValueKind::Remap).doc("A relief remap, from remap.curve or remap.terrace."))
    .finish(entry_name)
    .build()
}

fn window(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mut w = Window::around(i.number("theta_deg")?, i.number("span_deg")?);
    if let Some(f) = i.get("fade_deg").as_number() {
        w.fade_deg = f.max(0.0);
    }
    w.invert = i.bool("invert")?;
    w.enabled = i.bool("enabled")?;
    w.v_gate = match i.text("v_gate")? {
        "off" => VGate::Off,
        "band" => VGate::Band { center_mm: i.number("band_center_mm")?, span_mm: i.number("band_span_mm")?, fade_mm: i.number("band_fade_mm")? },
        "side_faces" => {
            let pick: SideFacePick = serde_json::from_value(serde_json::Value::String(i.text("side_pick")?.to_string()))
                .map_err(|_| NodeError::input("side_pick", "expected Low, High, Wider or Both"))?;
            VGate::SideFaces(pick)
        }
        other => return Err(NodeError::input("v_gate", format!("{other:?} is not off, band or side_faces"))),
    };
    Ok(Outputs::one("window", w))
}

fn remap_curve(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let pts: Vec<[f64; 2]> = match i.get("points") {
        Value::Path(p) => (**p).clone(),
        other => return Err(NodeError::input("points", format!("expected a path, got {}", other.summary()))),
    };
    if pts.len() < 2 {
        return Err(NodeError::input("points", "a remap curve needs at least two points"));
    }
    let mut curve = DropCurve::from_points(&pts);
    curve.sanitize();
    Ok(Outputs::one("remap", Remap::Curve { curve, span_mm: i.number("span_mm")? }))
}

fn remap_terrace(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let steps = i.int("steps")?;
    if !(1..=64).contains(&steps) {
        return Err(NodeError::input("steps", format!("{steps} is not between 1 and 64")));
    }
    Ok(Outputs::one("remap", Remap::Terrace { steps: steps as u32, span_mm: i.number("span_mm")?, riser: i.number("riser")? }))
}

pub fn register(reg: &mut Registry) {
    let side_picks: Vec<String> = enum_names(&[SideFacePick::Low, SideFacePick::High, SideFacePick::Wider, SideFacePick::Both]);
    let specs = [
        tiling_node(),
        NodeSpec::new("layer.tiling.fit", "Fit tiling", Category::Layer)
            .doc("A tiling sized to a design the way the app's add-menu does it: onto the side faces, with square cells.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design whose band it fits."))
            .input(PinSpec::item("alpha", ValueKind::AlphaRef).default("Scales").doc("The alpha's library name."))
            .input(PinSpec::item("side_faces", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Land it on the side faces, mirrored when both exist."))
            .input(PinSpec::item("square_cells", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Choose the count round the ring that makes square cells."))
            .input(PinSpec::item("min_draft_deg", ValueKind::Number).default(SIDE_FACE_MIN_DRAFT_DEG).doc("What counts as a side face, degrees."))
            .output(PinSpec::item("layer", ValueKind::Layer).doc("The tiling."))
            .eval(tiling_fit),
        border_node(),
        milgrain_node(),
        seat_node(),
        NodeSpec::new("layer.seat.fit", "Fit seat to gem", Category::Layer)
            .doc("A seat sized to a stone: the stone's plan, stock and depth.")
            .input(PinSpec::item("layer", ValueKind::Layer).optional().doc("The seat to size; the default seat otherwise."))
            .input(PinSpec::item("gem", ValueKind::Gem).doc("The stone."))
            .output(PinSpec::item("layer", ValueKind::Layer).doc("The fitted seat."))
            .eval(seat_fit),
        seatrun_node(),
        NodeSpec::new("layer.seatrun.solve", "Solve run spacing", Category::Layer)
            .doc("The count round the ring a run's stone and bridge want on this design, measured in metal.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design the run sits on."))
            .input(PinSpec::item("layer", ValueKind::Layer).optional().doc("The run; the default run otherwise."))
            .output(PinSpec::item("layer", ValueKind::Layer).doc("The solved run."))
            .output(PinSpec::item("count", ValueKind::Int).doc("Stones round the ring."))
            .output(PinSpec::item("bridge_mm", ValueKind::Number).doc("Metal between neighbours as the report will read it, mm."))
            .eval(seatrun_solve),
        signet_layer_node(),
        curve_node(),
        NodeSpec::new("layer.curve.preset", "Wire preset", Category::Layer)
            .doc("One of the app's wire presets laid onto a design's side face.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::select("preset", vec!["scroll".into(), "vine".into(), "wave_rail".into()]).default("scroll").doc("Which preset."))
            .output(PinSpec::item("layer", ValueKind::Layer).doc("The wire."))
            .eval(curve_preset),
        flutes_node(),
        decals_node(),
        NodeSpec::new("decal.stamp", "Stamp", Category::Layer)
            .doc("One placement of a decal: where, how big, how high.")
            .input(PinSpec::item("theta_deg", ValueKind::Number).default(90.0).widget(Widget::Angle).doc("Where round the ring."))
            .input(PinSpec::item("v_mm", ValueKind::Number).default(0.0).doc("Where across the band, mm of section arc."))
            .input(PinSpec::item("size_mm", ValueKind::Number).default(3.0).doc("Stamp size, mm."))
            .input(PinSpec::item("rotation_deg", ValueKind::Number).default(0.0).doc("Rotation, degrees."))
            .input(PinSpec::item("height_mm", ValueKind::Number).default(0.3).doc("Relief height, mm."))
            .input(PinSpec::item("flip", ValueKind::Bool).default(false).doc("Mirror the stamp (the chart reads true from −Z)."))
            .output(PinSpec::item("stamp", ValueKind::Json).doc("The stamp, for layer.decals."))
            .eval(decal_stamp),
        openwork_node(),
        NodeSpec::new("layer.group", "Group", Category::Layer)
            .doc("A stack composited first and then placed as one layer, so a Replace inside cannot leak past it.")
            .input(PinSpec::item("stack", ValueKind::Stack).doc("The nested stack."))
            .output(PinSpec::item("layer", ValueKind::Layer).doc("The group."))
            .eval(group),
        entry_node(),
        NodeSpec::new("window", "Window", Category::Layer)
            .doc("An angular window round the ring, with an optional gate across the band.")
            .input(PinSpec::item("theta_deg", ValueKind::Number).default(90.0).widget(Widget::Angle).doc("Centre; 90° is the top."))
            .input(PinSpec::item("span_deg", ValueKind::Number).default(60.0).widget(Widget::Slider { min: 0.0, max: 360.0 }).doc("Full width, degrees."))
            .input(PinSpec::item("fade_deg", ValueKind::Number).optional().doc("Fade at each end, degrees; a fifth of the span if unset."))
            .input(PinSpec::item("invert", ValueKind::Bool).default(false).widget(Widget::Checkbox).doc("Everything but the arc."))
            .input(PinSpec::item("enabled", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Whether the window gates at all."))
            .input(PinSpec::select("v_gate", vec!["off".into(), "band".into(), "side_faces".into()]).default("off").doc("The gate across the band."))
            .input(PinSpec::item("band_center_mm", ValueKind::Number).default(0.0).doc("Band gate centre, mm of section arc."))
            .input(PinSpec::item("band_span_mm", ValueKind::Number).default(2.0).doc("Band gate width, mm."))
            .input(PinSpec::item("band_fade_mm", ValueKind::Number).default(0.3).doc("Band gate fade, mm."))
            .input(PinSpec::select("side_pick", side_picks).default("Wider").doc("Which side face, for the side_faces gate."))
            .output(PinSpec::item("window", ValueKind::Window).doc("The window."))
            .eval(window),
        NodeSpec::new("remap.curve", "Remap curve", Category::Layer)
            .doc("Reshape a layer's relief through a curve: [x, d] points from (0, 0) to (1, 1).")
            .input(PinSpec::item("points", ValueKind::Path).doc("The curve's points."))
            .input(PinSpec::item("span_mm", ValueKind::Number).default(1.0).doc("The relief span the curve maps, mm."))
            .output(PinSpec::item("remap", ValueKind::Remap).doc("The remap."))
            .eval(remap_curve),
        NodeSpec::new("remap.terrace", "Remap terrace", Category::Layer)
            .doc("Step a layer's relief into terraces.")
            .input(PinSpec::item("steps", ValueKind::Int).default(4i64).doc("Steps, 1..64."))
            .input(PinSpec::item("span_mm", ValueKind::Number).default(1.0).doc("The relief span stepped, mm."))
            .input(PinSpec::item("riser", ValueKind::Number).default(0.5).widget(Widget::Slider { min: 0.0, max: 1.0 }).doc("Riser sharpness, 0..1."))
            .output(PinSpec::item("remap", ValueKind::Remap).doc("The remap."))
            .eval(remap_terrace),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets};
    use crate::graph::{Graph, NodeId};
    use crate::value::Literal;
    use ringdesign_core::AlphaLibrary;

    fn run(g: &Graph) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::builtin(), 0, Targets::AllPure)
    }

    fn layer_of(r: &crate::eval::EvalReport, id: NodeId) -> Layer {
        match r.value(id, "layer") {
            Some(Value::Layer(l)) => (**l).clone(),
            other => panic!("{other:?} / {:?}", r.status.get(&id)),
        }
    }

    #[test]
    fn every_layer_node_makes_its_own_variant_unset() {
        let kinds = [
            ("layer.tiling", "Tiling"),
            ("layer.border", "Border"),
            ("layer.milgrain", "Milgrain"),
            ("layer.seat", "Seat"),
            ("layer.seatrun", "Seat run"),
            ("layer.signet", "Signet"),
            ("layer.curve", "Wire"),
            ("layer.flutes", "Flutes"),
            ("layer.decals", "Decals"),
            ("layer.openwork", "Openwork"),
        ];
        let mut g = Graph::default();
        let ids: Vec<(NodeId, &str)> = kinds.iter().map(|(k, label)| (g.add(*k).unwrap(), *label)).collect();
        let r = run(&g);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        for (id, label) in ids {
            assert_eq!(layer_label(&layer_of(&r, id)), label);
        }
    }

    #[test]
    fn a_fitted_tiling_and_a_run_come_from_the_design_and_field_clean() {
        let mut g = Graph::default();
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(6.0)).unwrap();
        g.set_input(p, "thickness_mm", Literal::Number(2.0)).unwrap();
        g.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        let t = g.add("layer.tiling.fit").unwrap();
        g.connect(d, "design", t, "design").unwrap();
        let w = g.add("window").unwrap();
        g.set_input(w, "span_deg", Literal::Number(120.0)).unwrap();
        let e = g.add("entry").unwrap();
        g.connect(t, "layer", e, "layer").unwrap();
        g.connect(w, "window", e, "window").unwrap();
        g.set_input(e, "blend", Literal::Text("Max".into())).unwrap();
        let gem = g.add("gem.calibrated").unwrap();
        g.set_input(gem, "w_mm", Literal::Number(2.0)).unwrap();
        let run_ = g.add("layer.seatrun").unwrap();
        g.connect(gem, "gem", run_, "gem").unwrap();
        let solved = g.add("layer.seatrun.solve").unwrap();
        g.connect(d, "design", solved, "design").unwrap();
        g.connect(run_, "layer", solved, "layer").unwrap();
        let r = run(&g);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        let Layer::Tiling(tl) = layer_of(&r, t) else { panic!() };
        assert!(tl.mirror_v, "a squared band has two side faces; the fit mirrors");
        assert!(tl.repeats_around >= 1 && tl.v_span_mm > 0.0);
        assert!(r.status[&t].warnings.is_empty(), "{:?}", r.status[&t].warnings);
        let Some(Value::Entry(entry)) = r.value(e, "entry") else { panic!() };
        assert_eq!(entry.name, "Tiling");
        assert_eq!(entry.blend, Blend::Max);
        assert_eq!(entry.window.span_deg, 120.0);
        assert!(entry.window.enabled);
        let count = r.value(solved, "count").unwrap().as_int().unwrap();
        assert!(count >= 8, "a 2 mm stone round a size 7: {count}");
        assert!(r.value(solved, "bridge_mm").unwrap().as_number().unwrap() > 0.0);

        // The entry drops into a design and the verdict reads it.
        let Some(Value::Design(design)) = r.value(d, "design") else { panic!() };
        let mut design = (**design).clone();
        design.layers.layers.push((**entry).clone());
        let lib = AlphaLibrary::builtin();
        let f = ringdesign_core::castability::analyze_field(&design, &lib, &design.draft, 160, 96);
        assert_ne!(f.verdict, ringdesign_core::castability::Verdict::NotCastable, "{:?}", f.notes);

        // A dome has no side face: the fit says so instead of failing.
        let p2 = g.add("band.profile").unwrap();
        g.set_input(p2, "style", Literal::Text("HalfRound".into())).unwrap();
        let d2 = g.add("design.new").unwrap();
        g.connect(p2, "profile", d2, "profile").unwrap();
        let t2 = g.add("layer.tiling.fit").unwrap();
        g.connect(d2, "design", t2, "design").unwrap();
        let r = run(&g);
        assert!(r.status[&t2].warnings.iter().any(|w| w.contains("no side face")), "{:?}", r.status[&t2]);
        g.set_input(t2, "alpha", Literal::Text("NoSuchAlpha".into())).unwrap();
        let r = run(&g);
        assert!(r.status[&t2].warnings.iter().any(|w| w.contains("no alpha named")), "{:?}", r.status[&t2]);
    }

    #[test]
    fn gating_nodes_build_windows_remaps_seats_and_stamps() {
        let mut g = Graph::default();
        let w = g.add("window").unwrap();
        g.set_input(w, "theta_deg", Literal::Number(270.0)).unwrap();
        g.set_input(w, "span_deg", Literal::Number(90.0)).unwrap();
        g.set_input(w, "invert", Literal::Bool(true)).unwrap();
        g.set_input(w, "v_gate", Literal::Text("side_faces".into())).unwrap();
        g.set_input(w, "side_pick", Literal::Text("Both".into())).unwrap();
        let wb = g.add("window").unwrap();
        g.set_input(wb, "v_gate", Literal::Text("band".into())).unwrap();
        g.set_input(wb, "band_span_mm", Literal::Number(1.5)).unwrap();
        g.set_input(wb, "fade_deg", Literal::Number(5.0)).unwrap();
        let rt = g.add("remap.terrace").unwrap();
        g.set_input(rt, "steps", Literal::Int(3)).unwrap();
        let rc = g.add("remap.curve").unwrap();
        g.set_input(rc, "points", Literal::Json(serde_json::json!([[0.0, 0.0], [0.5, 0.1], [1.0, 1.0]]))).unwrap();
        let gem = g.add("gem").unwrap();
        g.set_input(gem, "cut", Literal::Text("Emerald".into())).unwrap();
        g.set_input(gem, "w_mm", Literal::Number(3.0)).unwrap();
        g.set_input(gem, "l_mm", Literal::Number(4.5)).unwrap();
        let seat = g.add("layer.seat.fit").unwrap();
        g.connect(gem, "gem", seat, "gem").unwrap();
        let stamp = g.add("decal.stamp").unwrap();
        g.set_input(stamp, "size_mm", Literal::List(vec![Literal::Number(2.0), Literal::Number(3.0)])).unwrap();
        let decals = g.add("layer.decals").unwrap();
        g.connect(stamp, "stamp", decals, "decals").unwrap();
        g.set_input(decals, "alpha", Literal::Text("Scales".into())).unwrap();
        let e = g.add("entry").unwrap();
        g.connect(seat, "layer", e, "layer").unwrap();
        g.connect(w, "window", e, "window").unwrap();
        g.connect(rt, "remap", e, "remap").unwrap();
        g.set_input(e, "name", Literal::Text("Centre stone".into())).unwrap();
        g.set_input(e, "mask", Literal::Text("band".into())).unwrap();
        let r = run(&g);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        let Some(Value::Window(win)) = r.value(w, "window") else { panic!() };
        assert!(win.invert && win.theta_deg == 270.0 && win.span_deg == 90.0);
        assert_eq!(win.v_gate, VGate::SideFaces(SideFacePick::Both));
        let Some(Value::Window(win2)) = r.value(wb, "window") else { panic!() };
        assert_eq!(win2.fade_deg, 5.0);
        assert!(matches!(win2.v_gate, VGate::Band { span_mm, .. } if span_mm == 1.5));
        assert!(matches!(r.value(rt, "remap"), Some(Value::Remap(Remap::Terrace { steps: 3, .. }))));
        assert!(matches!(r.value(rc, "remap"), Some(Value::Remap(Remap::Curve { .. }))));
        let Layer::SeatPad(s) = layer_of(&r, seat) else { panic!() };
        assert_eq!(s.gem.map(|g| g.cut), Some(ringdesign_core::gem::GemCut::Emerald));
        assert!(s.elong > 1.0, "an emerald cut gets an elongated plan: {}", s.elong);
        let Layer::Decals(dl) = layer_of(&r, decals) else { panic!() };
        assert_eq!(dl.decals.len(), 2);
        assert_eq!(dl.decals[1].size_mm, 3.0);
        assert_eq!(dl.alpha, "Scales");
        let Some(Value::Entry(entry)) = r.value(e, "entry") else { panic!() };
        assert_eq!(entry.name, "Centre stone");
        assert_eq!(entry.mask.as_deref(), Some("band"));
        assert!(matches!(entry.remap, Remap::Terrace { .. }));
        assert!(entry.window.invert);
        // A group takes a stack.
        let st = g.add("layer.group").unwrap();
        g.connect(e, "entry", st, "stack").unwrap();
        let r = run(&g);
        let Layer::Group(grp) = layer_of(&r, st) else { panic!("{:?}", r.status[&st]) };
        assert_eq!(grp.stack.layers.len(), 1, "an entry coerces into a one-entry stack");
    }
}
