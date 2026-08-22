//! A design lifted into a graph that evaluates back to it exactly.
//!
//! The lift wires the nodes a person would — section, shank, heads, one
//! node per layer with its gating, the alpha sources, the stack, the
//! assembly, the output — then evaluates what it built and compares the
//! result with the design field by field. Whatever the nodes cannot
//! express (a flange, a tiling warp, a custom outline registry, the draft
//! and build settings) rides as `design.set` patches at the end, so the
//! round trip is exact by construction rather than by coverage.

use ringdesign_core::field::{Layer, LayerEntry, Remap, VGate};
use ringdesign_core::{AlphaLibrary, RingDesign};

use crate::eval::{Evaluator, OUTPUT_DESIGN_PIN, OUTPUT_KIND, Targets};
use crate::graph::{Graph, GraphError, Mode, NodeId};
use crate::registry::Registry;
use crate::value::{Literal, Value};

/// Most `design.set` patches the lift adds before it patches whole
/// top-level fields instead.
const MAX_PATCHES: usize = 64;

/// Set a node's field pins from the struct's JSON, for every pin whose name
/// is a field of the object (and whose value is a literal).
fn set_fields(g: &mut Graph, id: NodeId, reg: &Registry, json: &serde_json::Value, skip: &[&str]) {
    let Some(obj) = json.as_object() else { return };
    let Some((ins, _)) = reg.node_pins(g.node(id).expect("added")) else { return };
    for pin in ins {
        if skip.contains(&pin.name.as_str()) {
            continue;
        }
        let Some(v) = obj.get(&pin.name) else { continue };
        if v.is_null() || v.is_object() {
            continue;
        }
        if let Ok(lit) = serde_json::from_value::<Literal>(v.clone()) {
            let _ = g.set_input(id, pin.name.clone(), lit);
        }
    }
}

fn json_of<T: serde::Serialize>(t: &T) -> serde_json::Value {
    serde_json::to_value(t).unwrap_or(serde_json::Value::Null)
}

fn gem_node(g: &mut Graph, reg: &Registry, gem: &ringdesign_core::gem::Gem) -> Result<NodeId, GraphError> {
    let id = g.add("gem")?;
    set_fields(g, id, reg, &json_of(gem), &[]);
    Ok(id)
}

fn window_node(g: &mut Graph, w: &ringdesign_core::Window) -> Result<NodeId, GraphError> {
    let id = g.add("window")?;
    g.set_input(id, "theta_deg", Literal::Number(w.theta_deg))?;
    g.set_input(id, "span_deg", Literal::Number(w.span_deg))?;
    g.set_input(id, "fade_deg", Literal::Number(w.fade_deg))?;
    g.set_input(id, "invert", Literal::Bool(w.invert))?;
    g.set_input(id, "enabled", Literal::Bool(w.enabled))?;
    match &w.v_gate {
        VGate::Off => {}
        VGate::Band { center_mm, span_mm, fade_mm } => {
            g.set_input(id, "v_gate", Literal::Text("band".into()))?;
            g.set_input(id, "band_center_mm", Literal::Number(*center_mm))?;
            g.set_input(id, "band_span_mm", Literal::Number(*span_mm))?;
            g.set_input(id, "band_fade_mm", Literal::Number(*fade_mm))?;
        }
        VGate::SideFaces(pick) => {
            g.set_input(id, "v_gate", Literal::Text("side_faces".into()))?;
            if let Some(s) = json_of(pick).as_str() {
                g.set_input(id, "side_pick", Literal::Text(s.into()))?;
            }
        }
    }
    Ok(id)
}

fn remap_node(g: &mut Graph, r: &Remap) -> Result<Option<NodeId>, GraphError> {
    Ok(match r {
        Remap::Off => None,
        Remap::Curve { curve, span_mm } => {
            let id = g.add("remap.curve")?;
            g.set_input(id, "points", Literal::Json(json_of(&curve.points().to_vec())))?;
            g.set_input(id, "span_mm", Literal::Number(*span_mm))?;
            Some(id)
        }
        Remap::Terrace { steps, span_mm, riser } => {
            let id = g.add("remap.terrace")?;
            g.set_input(id, "steps", Literal::Int(i64::from(*steps)))?;
            g.set_input(id, "span_mm", Literal::Number(*span_mm))?;
            g.set_input(id, "riser", Literal::Number(*riser))?;
            Some(id)
        }
    })
}

/// A layer as its node; nested payloads become nodes wired in.
fn layer_node(g: &mut Graph, reg: &Registry, layer: &Layer) -> Result<NodeId, GraphError> {
    let id = match layer {
        Layer::Tiling(t) => {
            let id = g.add("layer.tiling")?;
            set_fields(g, id, reg, &json_of(t), &[]);
            id
        }
        Layer::Border(b) => {
            let id = g.add("layer.border")?;
            set_fields(g, id, reg, &json_of(b), &[]);
            id
        }
        Layer::Milgrain(m) => {
            let id = g.add("layer.milgrain")?;
            set_fields(g, id, reg, &json_of(m), &[]);
            id
        }
        Layer::SeatPad(s) => {
            let id = g.add("layer.seat")?;
            set_fields(g, id, reg, &json_of(s), &["gem"]);
            if let Some(gem) = &s.gem {
                let gid = gem_node(g, reg, gem)?;
                g.connect(gid, "gem", id, "gem")?;
            }
            id
        }
        Layer::SeatRun(r) => {
            let id = g.add("layer.seatrun")?;
            set_fields(g, id, reg, &json_of(r), &["gem", "seat"]);
            let gid = gem_node(g, reg, &r.gem)?;
            g.connect(gid, "gem", id, "gem")?;
            let seat = layer_node(g, reg, &Layer::SeatPad(r.seat))?;
            g.connect(seat, "layer", id, "seat")?;
            id
        }
        Layer::Signet(s) => {
            let id = g.add("layer.signet")?;
            set_fields(g, id, reg, &json_of(s), &[]);
            id
        }
        Layer::Curve(c) => {
            let id = g.add("layer.curve")?;
            set_fields(g, id, reg, &json_of(c), &["points"]);
            g.set_input(id, "points", Literal::Json(json_of(&c.points)))?;
            id
        }
        Layer::Flutes(f) => {
            let id = g.add("layer.flutes")?;
            set_fields(g, id, reg, &json_of(f), &[]);
            id
        }
        Layer::Decals(d) => {
            let id = g.add("layer.decals")?;
            set_fields(g, id, reg, &json_of(d), &["decals"]);
            let stamps: Vec<Literal> = d.decals.iter().map(|s| Literal::Json(json_of(s))).collect();
            g.set_input(id, "decals", Literal::List(stamps))?;
            id
        }
        Layer::Openwork(o) => {
            let id = g.add("layer.openwork")?;
            set_fields(g, id, reg, &json_of(o), &["tiling"]);
            let tl = layer_node(g, reg, &Layer::Tiling(o.tiling.clone()))?;
            g.connect(tl, "layer", id, "tiling")?;
            id
        }
        Layer::Group(grp) => {
            let id = g.add("layer.group")?;
            if let Some(st) = stack_nodes(g, reg, &grp.stack.layers)? {
                g.connect(st, "stack", id, "stack")?;
            }
            id
        }
    };
    Ok(id)
}

fn entry_node(g: &mut Graph, reg: &Registry, e: &LayerEntry) -> Result<NodeId, GraphError> {
    let layer = layer_node(g, reg, &e.layer)?;
    let id = g.add("entry")?;
    g.connect(layer, "layer", id, "layer")?;
    g.set_input(id, "name", Literal::Text(e.name.clone()))?;
    g.set_input(id, "enabled", Literal::Bool(e.enabled))?;
    if let Some(b) = json_of(&e.blend).as_str() {
        g.set_input(id, "blend", Literal::Text(b.into()))?;
    }
    g.set_input(id, "opacity", Literal::Number(e.opacity))?;
    g.set_input(id, "soft_mm", Literal::Number(e.soft_mm))?;
    if let Some(m) = &e.mask {
        g.set_input(id, "mask", Literal::Text(m.clone()))?;
    }
    let w = window_node(g, &e.window)?;
    g.connect(w, "window", id, "window")?;
    if let Some(r) = remap_node(g, &e.remap)? {
        g.connect(r, "remap", id, "remap")?;
    }
    Ok(id)
}

/// A chain of `stack` nodes, one per entry, in order.
fn stack_nodes(g: &mut Graph, reg: &Registry, entries: &[LayerEntry]) -> Result<Option<NodeId>, GraphError> {
    let mut last: Option<NodeId> = None;
    for e in entries {
        let en = entry_node(g, reg, e)?;
        let st = g.add("stack")?;
        if let Some(prev) = last {
            g.connect(prev, "stack", st, "stack")?;
        }
        g.connect(en, "entry", st, "entries")?;
        last = Some(st);
    }
    Ok(last)
}

fn alpha_nodes(g: &mut Graph, d: &RingDesign) -> Result<Vec<NodeId>, GraphError> {
    let mut ids = Vec::new();
    for r in &d.recipes {
        let id = g.add("alpha.proc")?;
        g.set_input(id, "name", Literal::Text(r.name.clone()))?;
        if let Some(k) = json_of(&r.kind).as_str() {
            g.set_input(id, "kind", Literal::Text(k.into()))?;
        }
        g.set_input(id, "repeats", Literal::Int(i64::from(r.repeats)))?;
        g.set_input(id, "quarter_turns", Literal::Int(i64::from(r.quarter_turns)))?;
        g.set_input(id, "gamma", Literal::Number(r.gamma))?;
        g.set_input(id, "invert", Literal::Bool(r.invert))?;
        ids.push(id);
    }
    for t in &d.texts {
        let id = g.add("alpha.text")?;
        g.set_input(id, "name", Literal::Text(t.name.clone()))?;
        g.set_input(id, "text", Literal::Text(t.text.clone()))?;
        if let Some(f) = json_of(&t.font).as_str() {
            g.set_input(id, "font", Literal::Text(f.into()))?;
        }
        g.set_input(id, "tracking", Literal::Number(t.tracking))?;
        ids.push(id);
    }
    for s in &d.svgs {
        let id = g.add("alpha.svg")?;
        g.set_input(id, "name", Literal::Text(s.name.clone()))?;
        g.set_input(id, "svg", Literal::Text(s.svg.clone()))?;
        g.set_input(id, "invert", Literal::Bool(s.invert))?;
        ids.push(id);
    }
    for a in &d.drawn {
        let id = g.add("alpha.drawn")?;
        g.set_input(id, "name", Literal::Text(a.name.clone()))?;
        g.set_input(id, "width", Literal::Int(i64::from(a.width)))?;
        g.set_input(id, "height", Literal::Int(i64::from(a.height)))?;
        g.set_input(id, "wrap_x", Literal::Bool(a.wrap_x))?;
        g.set_input(id, "wrap_y", Literal::Bool(a.wrap_y))?;
        g.set_input(id, "strokes", Literal::List(a.strokes.iter().map(|s| Literal::Json(json_of(s))).collect()))?;
        ids.push(id);
    }
    for e in &d.embedded {
        let id = g.add("alpha.png")?;
        g.set_input(id, "name", Literal::Text(e.name.clone()))?;
        g.set_input(id, "png_base64", Literal::Text(e.png.clone()))?;
        ids.push(id);
    }
    Ok(ids)
}

/// Up to four sources per `list.merge`, chained.
fn merge_chain(g: &mut Graph, ids: &[NodeId], out: &str) -> Result<Option<NodeId>, GraphError> {
    if ids.is_empty() {
        return Ok(None);
    }
    let mut prev: Option<NodeId> = None;
    for chunk in ids.chunks(3) {
        let m = g.add("list.merge")?;
        let mut pins = ["a", "b", "c", "d"].iter();
        if let Some(p) = prev {
            g.connect(p, "out", m, *pins.next().expect("four pins"))?;
        }
        for id in chunk {
            g.connect(*id, out, m, *pins.next().expect("four pins"))?;
        }
        prev = Some(m);
    }
    Ok(prev)
}

/// The minimal set of pointers at which `got` differs from `want`.
pub fn diff(got: &serde_json::Value, want: &serde_json::Value, path: &str, out: &mut Vec<(String, serde_json::Value)>) {
    if got == want {
        return;
    }
    match (got, want) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            if a.keys().any(|k| !b.contains_key(k)) {
                out.push((path.to_string(), want.clone()));
                return;
            }
            for (k, bv) in b {
                let sub = format!("{path}/{}", k.replace('~', "~0").replace('/', "~1"));
                match a.get(k) {
                    Some(av) => diff(av, bv, &sub, out),
                    None => out.push((sub, bv.clone())),
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) if a.len() == b.len() => {
            for (k, (av, bv)) in a.iter().zip(b).enumerate() {
                diff(av, bv, &format!("{path}/{k}"), out);
            }
        }
        _ => out.push((path.to_string(), want.clone())),
    }
}

/// Lift `d` into a graph whose evaluation reproduces it exactly.
pub fn from_design(d: &RingDesign, reg: &Registry, lib: &AlphaLibrary) -> Result<Graph, GraphError> {
    let mut g = Graph::new(&d.name, Mode::SandRing);
    let profile = g.add("band.profile")?;
    set_fields(&mut g, profile, reg, &json_of(&d.profile), &[]);
    let shank = g.add("shank")?;
    set_fields(&mut g, shank, reg, &json_of(&d.shank), &["head", "head_theta_deg", "head_length_mm"]);
    let head = g.add("head")?;
    set_fields(&mut g, head, reg, &json_of(&d.shank.head), &[]);
    g.connect(head, "head", shank, "head")?;
    let mut shank_out = shank;
    for h in &d.shank.extra_heads {
        let hn = g.add("head")?;
        set_fields(&mut g, hn, reg, &json_of(h), &[]);
        let add = g.add("shank.add_head")?;
        g.connect(shank_out, "shank", add, "shank")?;
        g.connect(hn, "head", add, "head")?;
        shank_out = add;
    }
    let design = g.add("design.new")?;
    g.set_input(design, "name", Literal::Text(d.name.clone()))?;
    g.set_input(design, "size", Literal::Number(d.size.0))?;
    g.connect(profile, "profile", design, "profile")?;
    g.connect(shank_out, "shank", design, "shank")?;

    let stack = stack_nodes(&mut g, reg, &d.layers.layers)?;
    let alphas = alpha_nodes(&mut g, d)?;
    let merged = merge_chain(&mut g, &alphas, "source")?;
    let mut last = design;
    if stack.is_some() || merged.is_some() {
        let asm = g.add("design.assemble")?;
        g.connect(design, "design", asm, "design")?;
        if let Some(st) = stack {
            g.connect(st, "stack", asm, "stack")?;
        }
        if let Some(m) = merged {
            g.connect(m, "out", asm, "alphas")?;
        }
        last = asm;
    }

    // Evaluate what the nodes express and patch the rest.
    let report = Evaluator::new().evaluate(&g, reg, lib, 0, Targets::Node(last));
    if let Some(e) = report.errors.first() {
        return Err(e.clone());
    }
    let got = match report.value(last, "design") {
        Some(Value::Design(x)) => json_of(&**x),
        _ => {
            let notes = report.notes(&g).join("; ");
            return Err(GraphError::at(last, format!("the lifted nodes did not evaluate: {notes}")));
        }
    };
    let mut want = json_of(d);
    if let Some(obj) = want.as_object_mut() {
        obj.remove("graph");
    }
    let mut patches = Vec::new();
    diff(&got, &want, "", &mut patches);
    if patches.len() > MAX_PATCHES || patches.iter().any(|(p, _)| p.is_empty()) {
        patches.clear();
        if let (Some(a), Some(b)) = (got.as_object(), want.as_object()) {
            for (k, bv) in b {
                if a.get(k) != Some(bv) {
                    patches.push((format!("/{k}"), bv.clone()));
                }
            }
        }
    }
    for (pointer, value) in patches {
        let set = g.add("design.set")?;
        g.connect(last, "design", set, "design")?;
        g.set_input(set, "pointer", Literal::Text(pointer))?;
        g.set_input(set, "value", Literal::Json(value))?;
        last = set;
    }
    let out = g.add(OUTPUT_KIND)?;
    g.connect(last, "design", out, OUTPUT_DESIGN_PIN)?;
    crate::templates::arrange(&mut g);
    Ok(g)
}

/// Every design the lift must reproduce byte for byte, for the tests.
#[cfg(test)]
pub fn round_trip(d: &RingDesign, reg: &Registry, lib: &AlphaLibrary) -> Result<(Graph, String, String), GraphError> {
    let g = from_design(d, reg, lib)?;
    let out = crate::eval::evaluate_design(&mut Evaluator::new(), &g, reg, lib, 0)?;
    let mut want = d.clone();
    want.graph = None;
    Ok((g, serde_json::to_string(&*out.design).unwrap_or_default(), serde_json::to_string(&want).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_lifts_and_evaluates_back_byte_for_byte() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        for t in ringdesign_core::templates::all() {
            let d = t.design();
            let (g, got, want) = round_trip(&d, &reg, &lib).unwrap_or_else(|e| panic!("{}: {e}", t.name));
            assert_eq!(got, want, "{}: the lift does not round-trip", t.name);
            let patches = g.nodes.iter().filter(|n| n.kind == "design.set").count();
            assert!(patches <= 4, "{}: {patches} design.set patches — the nodes should carry a template: {:?}", t.name, g.nodes.iter().filter(|n| n.kind == "design.set").map(|n| n.inputs.get("pointer").cloned()).collect::<Vec<_>>());
        }
    }

    #[test]
    fn a_design_with_sources_and_odd_fields_still_lifts_exactly() {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let mut d = ringdesign_core::templates::all()[2].design();
        d.texts.push(ringdesign_core::text::TextAlpha { name: "Motto".into(), text: "ever".into(), font: ringdesign_core::text::TextFont::Script, tracking: 0.1 });
        d.recipes.push(ringdesign_core::alpha::ProcRecipe { name: "R".into(), repeats: 3, ..Default::default() });
        d.profile.flange.enabled = true;
        d.draft.min_draft_deg = 4.5;
        d.build.theta_steps = 321;
        if let Layer::Tiling(t) = &mut d.layers.layers[0].layer {
            t.warp = Some(ringdesign_core::tiling::WarpField { points: vec![[0.0, 0.0], [10.0, 1.0]], strength: 0.5, falloff_mm: 2.0 });
        }
        d.layers.layers[0].window = ringdesign_core::Window::except(90.0, 80.0);
        d.layers.layers[0].remap = Remap::Terrace { steps: 3, span_mm: 0.3, riser: 0.4 };
        let (g, got, want) = round_trip(&d, &reg, &lib).unwrap();
        assert_eq!(got, want);
        let pointers: Vec<String> = g.nodes.iter().filter(|n| n.kind == "design.set").filter_map(|n| n.inputs.get("pointer")).filter_map(|l| if let Literal::Text(s) = l { Some(s.clone()) } else { None }).collect();
        assert!(pointers.iter().any(|p| p.starts_with("/profile/flange")), "{pointers:?}");
        assert!(pointers.iter().any(|p| p.contains("warp")), "{pointers:?}");
        assert!(pointers.iter().any(|p| p.starts_with("/draft")), "{pointers:?}");
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "alpha.text").count(), 1);
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "remap.terrace").count(), 1);
    }
}
