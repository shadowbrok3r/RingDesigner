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
use crate::value::{Literal, Value, ValueKind};

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
        if v.is_object() && pin.kind == ValueKind::Json {
            let _ = g.set_input(id, pin.name.clone(), Literal::Json(v.clone()));
            continue;
        }
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
            g.set_input(id, "points", serde_json::from_value(json_of(&curve.points().to_vec())).expect("point literal"))?;
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
            g.set_input(id, "points", serde_json::from_value(json_of(&c.points)).expect("point literal"))?;
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
    g.node_mut(layer).expect("added").label = Some(e.name.clone());
    let id = g.add("entry")?;
    g.node_mut(id).expect("added").label = Some(format!("{} / blend", e.name));
    g.connect(layer, "layer", id, "layer")?;
    g.set_input(id, "name", Literal::Text(e.name.clone()))?;
    g.set_input(id, "enabled", Literal::Bool(e.enabled))?;
    g.set_input(id, "bench_only", Literal::Bool(e.bench_only))?;
    if let Some(b) = json_of(&e.blend).as_str() {
        g.set_input(id, "blend", Literal::Text(b.into()))?;
    }
    g.set_input(id, "opacity", Literal::Number(e.opacity))?;
    g.set_input(id, "soft_mm", Literal::Number(e.soft_mm))?;
    if let Some(m) = &e.mask {
        g.set_input(id, "mask", Literal::Text(m.clone()))?;
    }
    let w = window_node(g, &e.window)?;
    g.node_mut(w).expect("added").label = Some(format!("{} / placement", e.name));
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
    // Pins are references on the ring, not something the graph builds.
    let unpinned;
    let d = if d.pins.is_empty() {
        d
    } else {
        unpinned = RingDesign { pins: Vec::new(), ..d.clone() };
        &unpinned
    };
    let mut g = Graph::new(&d.name, Mode::SandRing);
    let cad = d.cad.as_ref().filter(|doc| !doc.features.is_empty());
    // A CAD feature's id is its node's id, so every other node is numbered above them.
    if let Some(doc) = cad {
        g.next_id = g.next_id.max(doc.features.iter().map(|f| f.id + 1).max().unwrap_or(1));
    }
    let profile = g.add("band.profile")?;
    set_fields(&mut g, profile, reg, &json_of(&d.profile), &[]);
    // A uniform band's default head does not participate in its geometry.
    // Do not manufacture editable signet controls for that dormant state.
    let uses_head = d.shank.kind == ringdesign_core::ShankKind::Signet || d.imported_base.is_some();
    let mut shank_out = None;
    let mut head = None;
    let mut shank = None;
    if uses_head || json_of(&d.shank) != json_of(&ringdesign_core::ShankStyle::default()) {
        let id = g.add("shank")?;
        set_fields(&mut g, id, reg, &json_of(&d.shank), &["head", "head_theta_deg", "head_length_mm", "keys"]);
        let keys: Vec<_> = d.shank.keys.iter().map(|key| {
            let node = g.add("shank.key")?;
            set_fields(&mut g, node, reg, &json_of(key), &[]);
            Ok(node)
        }).collect::<Result<_, GraphError>>()?;
        if let Some(keys) = merge_chain(&mut g, &keys, "key")? { g.connect(keys, "out", id, "keys")?; }
        g.node_mut(id).expect("added").label = Some(if d.imported_base.is_some() { "Stock body" } else { "Shoulders and taper" }.into());
        shank = Some(id);
        shank_out = Some(id);
        if uses_head {
            let hn = g.add("head")?;
            set_fields(&mut g, hn, reg, &json_of(&d.shank.head), &[]);
            g.connect(hn, "head", id, "head")?;
            g.node_mut(hn).expect("added").label = Some(if d.imported_base.is_some() { "Stock face dimensions" } else { "Signet face" }.into());
            head = Some(hn);
        }
        for h in &d.shank.extra_heads {
            let hn = g.add("head")?;
            set_fields(&mut g, hn, reg, &json_of(h), &[]);
            let add = g.add("shank.add_head")?;
            g.connect(shank_out.expect("shank"), "shank", add, "shank")?;
            g.connect(hn, "head", add, "head")?;
            shank_out = Some(add);
        }
    }
    let design = g.add("design.new")?;
    g.set_input(design, "name", Literal::Text(d.name.clone()))?;
    g.set_input(design, "size", Literal::Number(d.size.0))?;
    g.connect(profile, "profile", design, "profile")?;
    if let Some(shank_out) = shank_out { g.connect(shank_out, "shank", design, "shank")?; }
    g.node_mut(profile).expect("added").label = Some("Band section".into());
    g.expose(design, "size", "US size")?;
    g.expose(profile, "width_mm", if d.imported_base.is_some() { "Face width" } else { "Band width" })?;
    g.expose(profile, "thickness_mm", if d.imported_base.is_some() { "Palm thickness" } else { "Band thickness" })?;
    if let Some(head) = head {
        for (pin, name) in [
            ("length_mm", "Face length"), ("rise_mm", "Face rise"),
            ("table_dome_mm", "Table dome"), ("shoulder_deg", "Shoulder arc"),
            ("swell_deg", "Body swell"), ("rim_round_mm", "Rim rounding"),
            ("dome", "Cut dome"), ("loft", "Loft"),
        ] {
            if d.imported_base.is_none() || matches!(pin, "length_mm" | "rise_mm") {
                g.expose(head, pin, name)?;
            }
        }
        if d.imported_base.is_none() { g.expose(shank.expect("head has a shank"), "amount", "Shank taper")?; }
    }

    let stack = stack_nodes(&mut g, reg, &d.layers.layers)?;
    let alphas = alpha_nodes(&mut g, d)?;
    for id in &alphas {
        let node = g.node_mut(*id).expect("added");
        if let Some(Literal::Text(name)) = node.inputs.get("name") {
            node.label = Some(name.clone());
        }
    }
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

    if let Some(base) = &d.imported_base
        && let Some(source) = ringdesign_core::imported_base::PresetSource::of(&base.source)
    {
        let id = g.add("base.preset")?;
        g.set_input(id, "id", Literal::Text(source.preset))?;
        g.set_input(id, "sand", Literal::Bool(source.sand_master))?;
        g.set_input(id, "preserve_parameters", Literal::Bool(true))?;
        g.set_input(id, "bare", Literal::Bool(base.bare))?;
        g.set_input(id, "sand_envelope", Literal::Bool(base.sand_envelope))?;
        g.set_input(id, "chart_enabled", Literal::Bool(base.chart.is_some()))?;
        if let Some(chart) = &base.chart { g.set_input(id, "chart", Literal::Json(json_of(chart)))?; }
        g.connect(last, "design", id, "design")?;
        last = id;
    }

    if let Some(doc) = cad {
        last = crate::nodes::cad::chain_document(&mut g, last, doc)?;
        crate::nodes::cad::lift_builders(&mut g, doc, reg)?;
    }

    if !d.stamps.is_empty() {
        let mut stamps = Vec::new();
        for stamp in &d.stamps {
            let id = g.add("stamp")?;
            set_fields(&mut g, id, reg, &json_of(stamp), &["outline"]);
            g.set_input(id, "outline", serde_json::from_value(json_of(&stamp.outline)).expect("points"))?;
            g.node_mut(id).expect("added").label = Some(stamp.name.clone());
            stamps.push(id);
        }
        let merge = merge_chain(&mut g, &stamps, "stamp")?.expect("nonempty stamps");
        let apply = g.add("design.stamps")?;
        g.connect(last, "design", apply, "design")?;
        g.connect(merge, "out", apply, "stamps")?;
        g.set_input(apply, "replace", Literal::Bool(true))?;
        last = apply;
    }

    let defaults = RingDesign::default();
    let build = json_of(&d.build);
    let draft = json_of(&d.draft);
    if build != json_of(&defaults.build) || draft != json_of(&defaults.draft) {
        let apply = g.add("design.settings")?;
        g.connect(last, "design", apply, "design")?;
        for (pin, kind, value, default) in [("build", "build.settings", build, json_of(&defaults.build)), ("draft", "draft.settings", draft, json_of(&defaults.draft))] {
            if value != default {
                let id = g.add(kind)?;
                set_fields(&mut g, id, reg, &value, &[]);
                g.connect(id, pin, apply, pin)?;
            }
        }
        last = apply;
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
        g.node_mut(set).expect("added").label = Some(format!("Preserve {pointer}"));
        g.set_input(set, "pointer", Literal::Text(pointer))?;
        // An array on a pin is an implicit list, and a node fed one runs once per item: a design's 32
        // stamps came out as 32 designs. The node holds arrays and nulls in its params whole.
        if value.is_array() || value.is_null() {
            g.node_mut(set).expect("added").params = serde_json::json!({ "json_value": value });
        } else {
            g.set_input(set, "value", Literal::Json(value))?;
        }
        last = set;
    }
    let out = g.add(OUTPUT_KIND)?;
    g.connect(last, "design", out, OUTPUT_DESIGN_PIN)?;
    // Nodes in id order, as the editor writes them back.
    g.nodes.sort_by_key(|n| n.id);
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
    fn uniform_band_does_not_acquire_dormant_signet_controls() {
        let reg = Registry::builtin();
        let (graph, got, want) = round_trip(&RingDesign::default(), &reg, &AlphaLibrary::default()).unwrap();
        assert_eq!(got, want);
        assert!(!graph.nodes.iter().any(|node| matches!(node.kind.as_str(), "head" | "shank")));
    }

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
    fn a_design_with_cad_parts_lifts_to_feature_nodes_the_edit_funnel_can_edit() {
        use ringdesign_core::cad::{self, Attach, Component, Document, Feature, Operation, Placement, edit::CadEdit};
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let mut band = ringdesign_core::templates::all()[0].design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature { id: 2, name: "Bezel".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 }, component: Component { attach: Attach::Join, placement: Placement::ring(90.0, 1.25), ..Default::default() } }).unwrap();
        band.cad = Some(doc);
        let designs: Vec<(String, RingDesign)> = cad::examples::NAMES.iter().map(|n| (n.to_string(), cad::examples::design(n).unwrap())).chain([("band and bezel".to_string(), band)]).collect();
        for (name, d) in designs {
            let (mut g, got, want) = round_trip(&d, &reg, &lib).unwrap();
            assert_eq!(got, want, "{name}");
            let whole = g.nodes.iter().filter(|n| n.kind == "design.set").any(|n| matches!(n.inputs.get("pointer"), Some(Literal::Text(p)) if p == "/cad"));
            assert!(!whole, "{name}: the document travels as feature nodes, not one patch");
            let features = d.cad.as_ref().unwrap().features.len();
            assert_eq!(g.nodes.iter().filter(|n| n.kind == "cad.feature").count(), features, "{name}");
            assert!(g.nodes.windows(2).all(|w| w[0].id < w[1].id), "{name}: nodes in id order, as the editor writes them back");
            assert_eq!(serde_json::to_value(crate::nodes::cad::document(&g).unwrap()).unwrap(), serde_json::to_value(d.cad.as_ref().unwrap()).unwrap(), "{name}");
            let id = d.cad.as_ref().unwrap().features.last().unwrap().id;
            crate::nodes::cad::apply_edit(&mut g, &CadEdit::Rename { id, name: "Renamed".into() }).unwrap();
            let out = crate::eval::evaluate_design(&mut Evaluator::new(), &g, &reg, &lib, 0).unwrap();
            assert_eq!(out.design.cad.as_ref().unwrap().feature(id).unwrap().name, "Renamed", "{name}");
        }
    }

    #[test]
    fn a_lift_exposes_casting_criteria_and_mesh_settings_without_property_patches() {
        use ringdesign_core::castability::{CastProcess, SandProcess};
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        SandProcess::DelftClay.apply(&mut d.draft);
        d.draft.process = CastProcess::LostWax;
        d.draft.min_section_mm = 0.8;
        d.draft.parting_z_mm = 0.3;
        d.draft.auto_parting = false;
        d.build.theta_steps = 1536;
        d.build.profile_steps = 448;
        d.build.refine = Some(Default::default());
        let (mut g, got, want) = round_trip(&d, &reg, &lib).unwrap();
        assert_eq!(got, want);
        assert!(g.nodes.iter().all(|n| n.kind != "design.set"));
        let node = g.nodes.iter().find(|n| n.kind == "draft.settings").unwrap().id;
        g.set_input(node, "min_section_mm", Literal::Number(1.1)).unwrap();
        let (edited, _) = crate::eval::design_of(&mut Evaluator::new(), &g, &reg, &lib, 0).unwrap();
        d.draft.min_section_mm = 1.1;
        assert_eq!(json_of(&*edited), json_of(&d));
    }

    #[test]
    fn a_lift_patches_only_the_cad_properties_its_feature_chain_cannot_replay() {
        use ringdesign_core::cad::{Component, Document, Feature, Joint, Operation};
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        let mut doc = Document::default();
        for (id, operation) in [(1, Operation::Band), (2, Operation::Box { size: [2.0; 3] }), (3, Operation::Transform { source: 2, translation: [0.0, 0.0, 1.0], rotation_deg: [0.0; 3] })] {
            doc.append(Feature { id, name: format!("Part {id}"), enabled: true, operation, component: Component::default() }).unwrap();
        }
        for custom in [false, true] {
            if custom {
                doc.outputs.reverse();
                doc.joints.push(Joint { a: 1, b: 3, clearance_mm: 0.2, method: "Solder".into(), notes: "Foot to band".into() });
                doc.through = Some(2);
            }
            d.cad = Some(doc.clone());
            let (g, got, want) = round_trip(&d, &reg, &lib).unwrap();
            assert_eq!(got, want);
            let pointers: Vec<&str> = g.nodes.iter().filter(|n| n.kind == "design.set").filter_map(|n| match n.inputs.get("pointer") {
                Some(Literal::Text(p)) if p.starts_with("/cad/") => Some(p.as_str()),
                _ => None,
            }).collect();
            assert_eq!(pointers, if custom { vec!["/cad/outputs", "/cad/joints", "/cad/through"] } else { Vec::new() });
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
        assert!(!pointers.iter().any(|p| p.contains("warp")), "warp must travel with its layer, not a stack index: {pointers:?}");
        assert!(!pointers.iter().any(|p| p.starts_with("/draft") || p.starts_with("/build")), "{pointers:?}");
        assert!(g.nodes.iter().any(|n| n.kind == "draft.settings" && n.inputs.get("min_draft_deg") == Some(&Literal::Number(4.5))));
        assert!(g.nodes.iter().any(|n| n.kind == "build.settings" && n.inputs.get("theta_steps") == Some(&Literal::Int(321))));
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "alpha.text").count(), 1);
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "remap.terrace").count(), 1);
    }
}
