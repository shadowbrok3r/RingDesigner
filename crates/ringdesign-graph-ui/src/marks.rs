//! The Atelier mark every node and category in the add-node menu carries.
//!
//! One mark per *kind* of node, not one drawing per key: every `math.*` gets
//! its own operator glyph because those differ in what they do, while the
//! three culls share a cull mark because they do not. A node whose mark would
//! say nothing new borrows the one from the thing it makes — a wire layer is
//! the wire, a gem is the stone, a saved profile is the file it came from.
//!
//! `every_node_has_its_own_mark` holds the registry to it: a key [`node`]
//! does not name fails the test rather than shipping a bare button. It
//! returns an `Option` rather than a sentinel icon so a mark chosen on
//! purpose and a mark missed are never the same value.

use ringdesign_graph::registry::{Category, NodeSpec};
use ringdesign_workbench::icons::Icon;

/// What a key no one has named yet draws, until someone names it.
pub const UNNAMED: Icon = Icon::More;

/// The mark for a whole family, on the menu that opens it.
pub fn category(cat: Category) -> Icon {
    match cat {
        Category::Source => Icon::NodeSource,
        Category::Band => Icon::Shape,
        Category::Shank => Icon::NodeShank,
        Category::Layer => Icon::Layers,
        Category::Generator => Icon::NodeGenerator,
        Category::Alpha => Icon::Pattern,
        Category::Assembly => Icon::NodeAssembly,
        Category::Sink => Icon::NodeSink,
        Category::Util => Icon::Workshop,
        Category::Solid => Icon::CadBox,
    }
}

/// The mark for one node, by its registry key; `None` where none is named.
pub fn node(key: &str) -> Option<Icon> {
    // A family first, so a node added to one of these is covered the day it
    // lands; the table below only names what differs inside a family.
    match key {
        k if k.starts_with("solid.") || k.starts_with("frame.") => return solid(k),
        k if k.starts_with("cad.op.") => return Some(Icon::CadSketch),
        k if k.starts_with("stamp.outline.") => return Some(Icon::Stamp),
        _ => {}
    }
    Some(match key {
        // --- values ------------------------------------------------------
        "bool" => Icon::NodeBool,
        "int" | "number" => Icon::NodeNumber,
        "text" => Icon::NodeText,
        "range" => Icon::NodeRange,
        "series" => Icon::NodeSeries,

        // --- the band ----------------------------------------------------
        "band.profile" => Icon::NodeProfile,
        "band.profile.library" | "outline.library" | "alpha.library" => Icon::Files,
        "band.size" => Icon::NodeSize,
        "band.size.fit" => Icon::NodeFit,
        "design.new" => Icon::Add,
        "design.settings" => Icon::Settings,
        "base.preset" => Icon::NodeSignetpad,
        "shank.key" => Icon::NodeShank,
        "stamp" | "stamp.top" | "stamp.row" | "design.stamps" => Icon::Stamp,
        "build.settings" => Icon::NodeBuild,
        "draft.settings" => Icon::Casting,

        // --- the shank and its head --------------------------------------
        "shank" => Icon::NodeShank,
        "head" | "shank.add_head" => Icon::NodeHead,
        "shank.signet" => Icon::NodeSignetpad,
        "outline.custom" | "shank.outline" => Icon::NodeOutline,

        // --- layers ------------------------------------------------------
        "entry" => Icon::NodeEntry,
        "window" => Icon::NodeWindow,
        "layer.group" => Icon::NodeGroup,
        "layer.border" => Icon::NodeBorder,
        "layer.milgrain" => Icon::NodeMilgrain,
        "layer.flutes" => Icon::NodeFlutes,
        "layer.openwork" => Icon::NodeOpenwork,
        "layer.tiling" => Icon::NodeTiling,
        "layer.curve" | "layer.curve.preset" => Icon::Wire,
        "layer.decals" => Icon::NodeDecal,
        "decal.stamp" => Icon::Stamp,
        "layer.signet" => Icon::NodeSignetpad,
        "layer.seat" => Icon::Raise,
        "layer.seatrun" => Icon::NodeSeatrun,
        "layer.seatrun.solve" => Icon::NodeSolve,
        "layer.seat.fit" | "layer.tiling.fit" => Icon::NodeFit,
        "gem" | "gem.calibrated" => Icon::Stones,
        "remap.curve" => Icon::NodeCurve,
        "remap.terrace" => Icon::NodeTerrace,

        // --- generators ---------------------------------------------------
        "gen.pave" => Icon::NodePave,
        "gen.halo" => Icon::NodeHalo,
        "gen.channel" => Icon::NodeChannel,

        // --- alphas -------------------------------------------------------
        "alpha.drawn" => Icon::Paint,
        "alpha.png" => Icon::NodeImage,
        "alpha.proc" => Icon::Pattern,
        "alpha.svg" => Icon::Path,
        "alpha.text" => Icon::Engrave,

        // --- assembly -----------------------------------------------------
        "stack" => Icon::Layers,
        "cluster" => Icon::NodeCluster,
        "design.assemble" => Icon::NodeAssembly,
        "design.get" => Icon::NodeJson,
        "design.set" => Icon::Settings,
        "design.info" => Icon::NodeInfo,
        "design.resize" => Icon::Scale,
        "cad.source" => Icon::CadBand,
        "cad.feature" => Icon::CadSketch,

        // --- sinks --------------------------------------------------------
        "gate.castable" => Icon::NodeGate,
        "sink.build" => Icon::NodeBuild,
        "sink.refine" => Icon::NodeRefine,
        "sink.render" => Icon::NodeRender,
        "sink.output" => Icon::NodeSink,
        "sink.export" => Icon::Export,
        "sink.save_design" => Icon::Save,
        "sink.write_text" => Icon::Files,
        "sink.sheet" => Icon::Casting,
        "sink.stones" => Icon::Stones,
        "sink.dfm" => Icon::NodeReport,
        "sink.field_verdict" => Icon::NodeVerdict,
        "sink.mesh_verdict" => Icon::Mould,

        // --- flow ---------------------------------------------------------
        "flow.if" | "list.dispatch" => Icon::NodeBranch,
        "stream.gate" => Icon::NodeGate,
        "logic.and" | "logic.or" | "logic.not" => Icon::NodeLogic,

        // --- lists --------------------------------------------------------
        "list.item" => Icon::NodeItem,
        "list.length" => Icon::NodeLength,
        "list.merge" => Icon::NodeMerge,
        "list.split" => Icon::NodeSplit,
        "list.slice" => Icon::NodeSlice,
        "list.partition" => Icon::NodePartition,
        "list.sort" | "list.sort_keys" => Icon::NodeSort,
        "list.cull_index" | "list.cull_nth" | "list.cull_pattern" => Icon::NodeCull,
        "list.weave" | "list.entwine" => Icon::NodeWeave,
        "list.graft" => Icon::NodeGraft,
        "list.flatten" => Icon::NodeFlatten,
        "list.shift" => Icon::NodeShift,
        "list.sum" => Icon::NodeSum,
        "list.repeat" => Icon::Duplicate,
        "list.reverse" => Icon::Mirror,
        "polar.array" => Icon::Rotate,

        // --- arithmetic ---------------------------------------------------
        "math.add" => Icon::NodeAdd,
        "math.sub" => Icon::NodeSub,
        "math.mul" => Icon::NodeMul,
        "math.div" => Icon::NodeDiv,
        "math.pow" => Icon::NodePow,
        "math.sqrt" => Icon::NodeSqrt,
        "math.mod" => Icon::NodeMod,
        "math.abs" => Icon::NodeAbs,
        "math.neg" => Icon::NodeNeg,
        "math.round" | "math.floor" | "math.ceil" => Icon::NodeRound,
        "math.sin" | "math.cos" | "math.tan" | "math.atan2" => Icon::NodeTrig,
        "math.deg" | "math.rad" => Icon::NodeAngle,
        "math.compare" => Icon::NodeCompare,
        "math.max" | "math.min" => Icon::NodeMinmax,
        "math.clamp" => Icon::NodeClamp,
        "math.lerp" => Icon::NodeLerp,
        "math.remap" => Icon::NodeRemap,

        // --- text, json and the rest --------------------------------------
        "text.concat" | "text.format" | "text.join" => Icon::NodeConcat,
        "json.get" | "json.set" => Icon::NodeJson,
        "script" => Icon::NodeScript,
        "cad.inspect" => Icon::Search,
        "manufacturing.inspect" => Icon::Mould,

        _ => return None,
    })
}

/// Free-mode solids read as the CAD operation they are.
fn solid(key: &str) -> Option<Icon> {
    Some(match key {
        "solid.cylinder" => Icon::CadCylinder,
        "solid.sphere" => Icon::CadSphere,
        "solid.box" => Icon::CadBox,
        "solid.extrude" => Icon::CadExtrude,
        "solid.revolve" => Icon::CadRevolve,
        "solid.tube" => Icon::CadSweep,
        "solid.union" | "solid.union_all" => Icon::CadUnion,
        "solid.difference" => Icon::CadSubtract,
        "solid.intersect" => Icon::CadIntersect,
        "solid.translate" => Icon::Move,
        "solid.rotate" => Icon::Rotate,
        "solid.scale" => Icon::Scale,
        "solid.place" | "frame.on_ring" => Icon::CadPlace,
        "solid.setting" => Icon::Stones,
        "solid.leaf" => Icon::Path,
        "solid.vine_semimount" => Icon::Wire,
        "solid.import" => Icon::Files,
        "solid.from_design" => Icon::Shape,
        "solid.mesh" => Icon::NodeBuild,
        _ => return None,
    })
}

/// A menu row: the node's mark, then its label.
pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    ui.add(
        egui::Button::image_and_text(icon.image(ui, 16.0), label)
            .image_tint_follows_text_color(false)
            .min_size(egui::vec2(ui.available_width().min(210.0), 0.0)),
    )
}

/// The same row for a whole category, which opens a submenu.
pub fn submenu<R>(ui: &mut egui::Ui, cat: Category, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::Response {
    let icon = category(cat);
    egui::containers::menu::MenuButton::from_button(
        egui::Button::image_and_text(icon.image(ui, 16.0), cat.label())
            .image_tint_follows_text_color(false)
            .min_size(egui::vec2(ui.available_width().min(210.0), 0.0)),
    )
    .ui(ui, add)
    .0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_graph::graph::Mode;
    use ringdesign_graph::registry::Registry;

    #[test]
    fn every_node_has_its_own_mark() {
        let reg = Registry::builtin();
        let bare: Vec<_> = reg
            .list(Mode::Free)
            .into_iter()
            .filter(|s| node(&s.key).is_none())
            .map(|s| s.key.clone())
            .collect();
        assert!(bare.is_empty(), "these would draw the unnamed mark: {bare:?}");
        // Every category the menu opens is named too, and none of them
        // borrows the unnamed mark.
        for cat in Category::ALL {
            assert_ne!(category(*cat), UNNAMED, "{cat:?}");
        }
    }

    #[test]
    fn a_mark_is_a_drawing_and_not_a_letter() {
        // The mark for every node renders ink, which is what the workbench's
        // own icon test guarantees for the family; this pins the join.
        let reg = Registry::builtin();
        for spec in reg.list(Mode::Free) {
            let svg = of(&spec).svg();
            assert!(svg.starts_with("<svg"), "{}", spec.key);
        }
    }
}

/// The mark a spec draws, for callers that already hold one.
pub fn of(spec: &NodeSpec) -> Icon {
    node(&spec.key).unwrap_or(UNNAMED)
}
