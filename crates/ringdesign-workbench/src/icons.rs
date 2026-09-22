//! Atelier marks: a curated 24-unit SVG family, shared by mouse, pen and touch.
use egui::{Response, Ui, Vec2};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Undo,
    Redo,
    Shape,
    Surface,
    Stones,
    Casting,
    Select,
    Paint,
    Stamp,
    Path,
    Transform,
    Section,
    Measure,
    Spacing,
    Mould,
    Layers,
    More,
    Guides,
    Panel,
    Close,
    Expand,
    Collapse,
    View,
    Fit,
    ZoomIn,
    ZoomOut,
    Guide,
    History,
    Files,
    Export,
    Pattern,
    Graph,
    Workshop,
    Save,
    Add,
    Duplicate,
    Mirror,
    Rotate,
    Scale,
    Move,
    Raise,
    Engrave,
    Reset,
    Check,
    Help,
    Search,
    Before,
    Settings,
    Locked,
    Unlocked,
    Magnifier,
    TurnLeft,
    TurnRight,
    Opposite,
    Grid,
    Wire,
    Cutters,
    Ghost,
    Single,
    SplitVertical,
    SplitHorizontal,
    Four,
    Rebuild,
    Delete,
    CadBand,
    CadBox,
    CadCylinder,
    CadSphere,
    CadTorus,
    CadTwistedRing,
    CadExtrude,
    CadRevolve,
    CadSweep,
    CadTwist,
    CadLoft,
    CadUnion,
    CadSubtract,
    CadIntersect,
    CadFillet,
    CadChamfer,
    CadShell,
    CadPlace,
    CadSketch,

    // Graph node marks: one per kind of node the add-node menu offers.
    NodeAbs,
    NodeAdd,
    NodeAngle,
    NodeAssembly,
    NodeBool,
    NodeBorder,
    NodeBranch,
    NodeBuild,
    NodeClamp,
    NodeCluster,
    NodeCompare,
    NodeConcat,
    NodeCull,
    NodeCurve,
    NodeDecal,
    NodeDiv,
    NodeEntry,
    NodeFit,
    NodeFlatten,
    NodeFlutes,
    NodeGate,
    NodeGenerator,
    NodeGraft,
    NodeGroup,
    NodeHead,
    NodeInfo,
    NodeItem,
    NodeJson,
    NodeLength,
    NodeLerp,
    NodeList,
    NodeLogic,
    NodeMerge,
    NodeMilgrain,
    NodeMinmax,
    NodeMod,
    NodeMul,
    NodeNeg,
    NodeNumber,
    NodeOpenwork,
    NodeOutline,
    NodePartition,
    NodePow,
    NodeProfile,
    NodeRange,
    NodeRefine,
    NodeRemap,
    NodeRender,
    NodeReport,
    NodeRound,
    NodeSeatrun,
    NodeSeries,
    NodeShank,
    NodeShift,
    NodeSignetpad,
    NodeSink,
    NodeSize,
    NodeSlice,
    NodeSolve,
    NodeSort,
    NodeSource,
    NodeSplit,
    NodeSqrt,
    NodeSub,
    NodeSum,
    NodeTerrace,
    NodeText,
    NodeTiling,
    NodeTrig,
    NodeVerdict,
    NodeWeave,
    NodeWindow,
    NodePave,
    NodeHalo,
    NodeChannel,
    NodeImage,
    NodeScript,
}
impl Icon {
    pub const ALL: &'static [Self] = &[
        Self::Undo,
        Self::Redo,
        Self::Shape,
        Self::Surface,
        Self::Stones,
        Self::Casting,
        Self::Select,
        Self::Paint,
        Self::Stamp,
        Self::Path,
        Self::Transform,
        Self::Section,
        Self::Measure,
        Self::Spacing,
        Self::Mould,
        Self::Layers,
        Self::More,
        Self::Guides,
        Self::Panel,
        Self::Close,
        Self::Expand,
        Self::Collapse,
        Self::View,
        Self::Fit,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::Guide,
        Self::History,
        Self::Files,
        Self::Export,
        Self::Pattern,
        Self::Graph,
        Self::Workshop,
        Self::Save,
        Self::Add,
        Self::Duplicate,
        Self::Mirror,
        Self::Rotate,
        Self::Scale,
        Self::Move,
        Self::Raise,
        Self::Engrave,
        Self::Reset,
        Self::Check,
        Self::Help,
        Self::Search,
        Self::Before,
        Self::Settings,
        Self::Locked,
        Self::Unlocked,
        Self::Magnifier,
        Self::TurnLeft,
        Self::TurnRight,
        Self::Opposite,
        Self::Grid,
        Self::Wire,
        Self::Cutters,
        Self::Ghost,
        Self::Single,
        Self::SplitVertical,
        Self::SplitHorizontal,
        Self::Four,
        Self::Rebuild,
        Self::Delete,
        Self::CadBand,
        Self::CadBox,
        Self::CadCylinder,
        Self::CadSphere,
        Self::CadTorus,
        Self::CadTwistedRing,
        Self::CadExtrude,
        Self::CadRevolve,
        Self::CadSweep,
        Self::CadTwist,
        Self::CadLoft,
        Self::CadUnion,
        Self::CadSubtract,
        Self::CadIntersect,
        Self::CadFillet,
        Self::CadChamfer,
        Self::CadShell,
        Self::CadPlace,
        Self::CadSketch,
            Self::NodeAbs,
        Self::NodeAdd,
        Self::NodeAngle,
        Self::NodeAssembly,
        Self::NodeBool,
        Self::NodeBorder,
        Self::NodeBranch,
        Self::NodeBuild,
        Self::NodeClamp,
        Self::NodeCluster,
        Self::NodeCompare,
        Self::NodeConcat,
        Self::NodeCull,
        Self::NodeCurve,
        Self::NodeDecal,
        Self::NodeDiv,
        Self::NodeEntry,
        Self::NodeFit,
        Self::NodeFlatten,
        Self::NodeFlutes,
        Self::NodeGate,
        Self::NodeGenerator,
        Self::NodeGraft,
        Self::NodeGroup,
        Self::NodeHead,
        Self::NodeInfo,
        Self::NodeItem,
        Self::NodeJson,
        Self::NodeLength,
        Self::NodeLerp,
        Self::NodeList,
        Self::NodeLogic,
        Self::NodeMerge,
        Self::NodeMilgrain,
        Self::NodeMinmax,
        Self::NodeMod,
        Self::NodeMul,
        Self::NodeNeg,
        Self::NodeNumber,
        Self::NodeOpenwork,
        Self::NodeOutline,
        Self::NodePartition,
        Self::NodePow,
        Self::NodeProfile,
        Self::NodeRange,
        Self::NodeRefine,
        Self::NodeRemap,
        Self::NodeRender,
        Self::NodeReport,
        Self::NodeRound,
        Self::NodeSeatrun,
        Self::NodeSeries,
        Self::NodeShank,
        Self::NodeShift,
        Self::NodeSignetpad,
        Self::NodeSink,
        Self::NodeSize,
        Self::NodeSlice,
        Self::NodeSolve,
        Self::NodeSort,
        Self::NodeSource,
        Self::NodeSplit,
        Self::NodeSqrt,
        Self::NodeSub,
        Self::NodeSum,
        Self::NodeTerrace,
        Self::NodeText,
        Self::NodeTiling,
        Self::NodeTrig,
        Self::NodeVerdict,
        Self::NodeWeave,
        Self::NodeWindow,
        Self::NodePave,
        Self::NodeHalo,
        Self::NodeChannel,
        Self::NodeImage,
        Self::NodeScript,
];
    pub fn svg(self) -> &'static str {
        match self {
            Self::Undo => include_str!("../assets/icons/undo.svg"),
            Self::Redo => include_str!("../assets/icons/redo.svg"),
            Self::Shape => include_str!("../assets/icons/shape.svg"),
            Self::Surface => include_str!("../assets/icons/surface.svg"),
            Self::Stones => include_str!("../assets/icons/stones.svg"),
            Self::Casting => include_str!("../assets/icons/casting.svg"),
            Self::Select => include_str!("../assets/icons/select.svg"),
            Self::Paint => include_str!("../assets/icons/paint.svg"),
            Self::Stamp => include_str!("../assets/icons/stamp.svg"),
            Self::Path => include_str!("../assets/icons/path.svg"),
            Self::Transform => include_str!("../assets/icons/transform.svg"),
            Self::Section => include_str!("../assets/icons/section.svg"),
            Self::Measure => include_str!("../assets/icons/measure.svg"),
            Self::Spacing => include_str!("../assets/icons/spacing.svg"),
            Self::Mould => include_str!("../assets/icons/mould.svg"),
            Self::Layers => include_str!("../assets/icons/layers.svg"),
            Self::More => include_str!("../assets/icons/more.svg"),
            Self::Guides => include_str!("../assets/icons/guides.svg"),
            Self::Panel => include_str!("../assets/icons/panel.svg"),
            Self::Close => include_str!("../assets/icons/close.svg"),
            Self::Expand => include_str!("../assets/icons/expand.svg"),
            Self::Collapse => include_str!("../assets/icons/collapse.svg"),
            Self::View => include_str!("../assets/icons/view.svg"),
            Self::Fit => include_str!("../assets/icons/fit.svg"),
            Self::ZoomIn => include_str!("../assets/icons/zoomin.svg"),
            Self::ZoomOut => include_str!("../assets/icons/zoomout.svg"),
            Self::Guide => include_str!("../assets/icons/guide.svg"),
            Self::History => include_str!("../assets/icons/history.svg"),
            Self::Files => include_str!("../assets/icons/files.svg"),
            Self::Export => include_str!("../assets/icons/export.svg"),
            Self::Pattern => include_str!("../assets/icons/pattern.svg"),
            Self::Graph => include_str!("../assets/icons/graph.svg"),
            Self::Workshop => include_str!("../assets/icons/workshop.svg"),
            Self::Save => include_str!("../assets/icons/save.svg"),
            Self::Add => include_str!("../assets/icons/add.svg"),
            Self::Duplicate => include_str!("../assets/icons/duplicate.svg"),
            Self::Mirror => include_str!("../assets/icons/mirror.svg"),
            Self::Rotate => include_str!("../assets/icons/rotate.svg"),
            Self::Scale => include_str!("../assets/icons/scale.svg"),
            Self::Move => include_str!("../assets/icons/move.svg"),
            Self::Raise => include_str!("../assets/icons/raise.svg"),
            Self::Engrave => include_str!("../assets/icons/engrave.svg"),
            Self::Reset => include_str!("../assets/icons/reset.svg"),
            Self::Check => include_str!("../assets/icons/check.svg"),
            Self::Help => include_str!("../assets/icons/help.svg"),
            Self::Search => include_str!("../assets/icons/search.svg"),
            Self::Before => include_str!("../assets/icons/before.svg"),
            Self::Settings => include_str!("../assets/icons/settings.svg"),
            Self::Locked => include_str!("../assets/icons/locked.svg"),
            Self::Unlocked => include_str!("../assets/icons/unlocked.svg"),
            Self::Magnifier => include_str!("../assets/icons/magnifier.svg"),
            Self::TurnLeft => include_str!("../assets/icons/turn-left.svg"),
            Self::TurnRight => include_str!("../assets/icons/turn-right.svg"),
            Self::Opposite => include_str!("../assets/icons/opposite.svg"),
            Self::Grid => include_str!("../assets/icons/grid.svg"),
            Self::Wire => include_str!("../assets/icons/wire.svg"),
            Self::Cutters => include_str!("../assets/icons/cutters.svg"),
            Self::Ghost => include_str!("../assets/icons/ghost.svg"),
            Self::Single => include_str!("../assets/icons/single.svg"),
            Self::SplitVertical => include_str!("../assets/icons/split-vertical.svg"),
            Self::SplitHorizontal => include_str!("../assets/icons/split-horizontal.svg"),
            Self::Four => include_str!("../assets/icons/four.svg"),
            Self::Rebuild => include_str!("../assets/icons/rebuild.svg"),
            Self::Delete => include_str!("../assets/icons/delete.svg"),
            Self::CadBand => include_str!("../assets/icons/cad-band.svg"),
            Self::CadBox => include_str!("../assets/icons/cad-box.svg"),
            Self::CadCylinder => include_str!("../assets/icons/cad-cylinder.svg"),
            Self::CadSphere => include_str!("../assets/icons/cad-sphere.svg"),
            Self::CadTorus => include_str!("../assets/icons/cad-torus.svg"),
            Self::CadTwistedRing => include_str!("../assets/icons/cad-twisted-ring.svg"),
            Self::CadExtrude => include_str!("../assets/icons/cad-extrude.svg"),
            Self::CadRevolve => include_str!("../assets/icons/cad-revolve.svg"),
            Self::CadSweep => include_str!("../assets/icons/cad-sweep.svg"),
            Self::CadTwist => include_str!("../assets/icons/cad-twist.svg"),
            Self::CadLoft => include_str!("../assets/icons/cad-loft.svg"),
            Self::CadUnion => include_str!("../assets/icons/cad-union.svg"),
            Self::CadSubtract => include_str!("../assets/icons/cad-subtract.svg"),
            Self::CadIntersect => include_str!("../assets/icons/cad-intersect.svg"),
            Self::CadFillet => include_str!("../assets/icons/cad-fillet.svg"),
            Self::CadChamfer => include_str!("../assets/icons/cad-chamfer.svg"),
            Self::CadShell => include_str!("../assets/icons/cad-shell.svg"),
            Self::CadPlace => include_str!("../assets/icons/cad-place.svg"),
            Self::CadSketch => include_str!("../assets/icons/cad-sketch.svg"),

            Self::NodeAbs => include_str!("../assets/icons/node-abs.svg"),
            Self::NodeAdd => include_str!("../assets/icons/node-add.svg"),
            Self::NodeAngle => include_str!("../assets/icons/node-angle.svg"),
            Self::NodeAssembly => include_str!("../assets/icons/node-assembly.svg"),
            Self::NodeBool => include_str!("../assets/icons/node-bool.svg"),
            Self::NodeBorder => include_str!("../assets/icons/node-border.svg"),
            Self::NodeBranch => include_str!("../assets/icons/node-branch.svg"),
            Self::NodeBuild => include_str!("../assets/icons/node-build.svg"),
            Self::NodeClamp => include_str!("../assets/icons/node-clamp.svg"),
            Self::NodeCluster => include_str!("../assets/icons/node-cluster.svg"),
            Self::NodeCompare => include_str!("../assets/icons/node-compare.svg"),
            Self::NodeConcat => include_str!("../assets/icons/node-concat.svg"),
            Self::NodeCull => include_str!("../assets/icons/node-cull.svg"),
            Self::NodeCurve => include_str!("../assets/icons/node-curve.svg"),
            Self::NodeDecal => include_str!("../assets/icons/node-decal.svg"),
            Self::NodeDiv => include_str!("../assets/icons/node-div.svg"),
            Self::NodeEntry => include_str!("../assets/icons/node-entry.svg"),
            Self::NodeFit => include_str!("../assets/icons/node-fit.svg"),
            Self::NodeFlatten => include_str!("../assets/icons/node-flatten.svg"),
            Self::NodeFlutes => include_str!("../assets/icons/node-flutes.svg"),
            Self::NodeGate => include_str!("../assets/icons/node-gate.svg"),
            Self::NodeGenerator => include_str!("../assets/icons/node-generator.svg"),
            Self::NodeGraft => include_str!("../assets/icons/node-graft.svg"),
            Self::NodeGroup => include_str!("../assets/icons/node-group.svg"),
            Self::NodeHead => include_str!("../assets/icons/node-head.svg"),
            Self::NodeInfo => include_str!("../assets/icons/node-info.svg"),
            Self::NodeItem => include_str!("../assets/icons/node-item.svg"),
            Self::NodeJson => include_str!("../assets/icons/node-json.svg"),
            Self::NodeLength => include_str!("../assets/icons/node-length.svg"),
            Self::NodeLerp => include_str!("../assets/icons/node-lerp.svg"),
            Self::NodeList => include_str!("../assets/icons/node-list.svg"),
            Self::NodeLogic => include_str!("../assets/icons/node-logic.svg"),
            Self::NodeMerge => include_str!("../assets/icons/node-merge.svg"),
            Self::NodeMilgrain => include_str!("../assets/icons/node-milgrain.svg"),
            Self::NodeMinmax => include_str!("../assets/icons/node-minmax.svg"),
            Self::NodeMod => include_str!("../assets/icons/node-mod.svg"),
            Self::NodeMul => include_str!("../assets/icons/node-mul.svg"),
            Self::NodeNeg => include_str!("../assets/icons/node-neg.svg"),
            Self::NodeNumber => include_str!("../assets/icons/node-number.svg"),
            Self::NodeOpenwork => include_str!("../assets/icons/node-openwork.svg"),
            Self::NodeOutline => include_str!("../assets/icons/node-outline.svg"),
            Self::NodePartition => include_str!("../assets/icons/node-partition.svg"),
            Self::NodePow => include_str!("../assets/icons/node-pow.svg"),
            Self::NodeProfile => include_str!("../assets/icons/node-profile.svg"),
            Self::NodeRange => include_str!("../assets/icons/node-range.svg"),
            Self::NodeRefine => include_str!("../assets/icons/node-refine.svg"),
            Self::NodeRemap => include_str!("../assets/icons/node-remap.svg"),
            Self::NodeRender => include_str!("../assets/icons/node-render.svg"),
            Self::NodeReport => include_str!("../assets/icons/node-report.svg"),
            Self::NodeRound => include_str!("../assets/icons/node-round.svg"),
            Self::NodeSeatrun => include_str!("../assets/icons/node-seatrun.svg"),
            Self::NodeSeries => include_str!("../assets/icons/node-series.svg"),
            Self::NodeShank => include_str!("../assets/icons/node-shank.svg"),
            Self::NodeShift => include_str!("../assets/icons/node-shift.svg"),
            Self::NodeSignetpad => include_str!("../assets/icons/node-signetpad.svg"),
            Self::NodeSink => include_str!("../assets/icons/node-sink.svg"),
            Self::NodeSize => include_str!("../assets/icons/node-size.svg"),
            Self::NodeSlice => include_str!("../assets/icons/node-slice.svg"),
            Self::NodeSolve => include_str!("../assets/icons/node-solve.svg"),
            Self::NodeSort => include_str!("../assets/icons/node-sort.svg"),
            Self::NodeSource => include_str!("../assets/icons/node-source.svg"),
            Self::NodeSplit => include_str!("../assets/icons/node-split.svg"),
            Self::NodeSqrt => include_str!("../assets/icons/node-sqrt.svg"),
            Self::NodeSub => include_str!("../assets/icons/node-sub.svg"),
            Self::NodeSum => include_str!("../assets/icons/node-sum.svg"),
            Self::NodeTerrace => include_str!("../assets/icons/node-terrace.svg"),
            Self::NodeText => include_str!("../assets/icons/node-text.svg"),
            Self::NodeTiling => include_str!("../assets/icons/node-tiling.svg"),
            Self::NodeTrig => include_str!("../assets/icons/node-trig.svg"),
            Self::NodeVerdict => include_str!("../assets/icons/node-verdict.svg"),
            Self::NodeWeave => include_str!("../assets/icons/node-weave.svg"),
            Self::NodeWindow => include_str!("../assets/icons/node-window.svg"),
            Self::NodePave => include_str!("../assets/icons/node-pave.svg"),
            Self::NodeHalo => include_str!("../assets/icons/node-halo.svg"),
            Self::NodeChannel => include_str!("../assets/icons/node-channel.svg"),
            Self::NodeImage => include_str!("../assets/icons/node-image.svg"),
            Self::NodeScript => include_str!("../assets/icons/node-script.svg"),
        }
    }
    pub fn image(self, ui: &Ui, side: f32) -> egui::Image<'static> {
        let id = egui::Id::new(("atelier-svg", self));
        let texture = ui
            .data_mut(|d| d.get_temp::<egui::TextureHandle>(id))
            .unwrap_or_else(|| {
                let tree = resvg::usvg::Tree::from_str(self.svg(), &Default::default())
                    .expect("bundled SVG");
                let mut pixmap = resvg::tiny_skia::Pixmap::new(96, 96).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::from_scale(4.0, 4.0),
                    &mut pixmap.as_mut(),
                );
                let tex = ui.ctx().load_texture(
                    format!("atelier-{self:?}"),
                    egui::ColorImage::from_rgba_premultiplied([96, 96], pixmap.data()),
                    egui::TextureOptions::LINEAR,
                );
                ui.data_mut(|d| d.insert_temp(id, tex.clone()));
                tex
            });
        egui::Image::new((texture.id(), Vec2::splat(side))).tint(self.color())
    }

    /// Semantic colour survives text tinting and is shared by both shells.
    pub fn color(self) -> egui::Color32 {
        use Icon::*;
        let rgb = match self {
            CadBand | CadBox | CadCylinder | CadSphere | CadTorus | CadTwistedRing | CadExtrude | CadRevolve | CadSweep | CadTwist | CadLoft | Shape | Surface | Stamp | Raise | Engrave | Mould => [232, 191, 112],
            CadUnion | CadSubtract | CadIntersect | CadFillet | CadChamfer | CadShell | Stones | Graph | Pattern | Layers | Duplicate => [190, 161, 255],
            Files | Save | Export | History | Guide | Help => [123, 188, 245],
            Check | Casting | Workshop => [111, 211, 156],
            Delete | Close => [241, 135, 158],
            _ => [103, 217, 213],
        };
        egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
    }
}

pub struct Action {
    pub response: Response,
    held: bool,
}
impl std::ops::Deref for Action {
    type Target = Response;
    fn deref(&self) -> &Response {
        &self.response
    }
}
impl Action {
    pub fn clicked(&self) -> bool {
        self.response.clicked() && !self.held
    }
}

/// A hold explains a control and must never also activate it when released.
pub fn help(r: &Response, title: &str, what: &str, how: &str, when: &str) -> bool {
    let ctx = &r.ctx;
    let (now, down, pressed, released, pos, origin) = ctx.input(|i| {
        (
            i.time,
            i.pointer.any_down(),
            i.pointer.any_pressed(),
            i.pointer.any_released(),
            i.pointer.interact_pos(),
            i.pointer.press_origin(),
        )
    });
    let id = r.id.with("atelier-hold-help");
    let mut state = ctx.data(|d| d.get_temp::<(f64, bool)>(id));
    if pressed && origin.is_some_and(|p| r.rect.contains(p)) && r.contains_pointer() {
        state = Some((now, false));
    }
    let over = pos.is_some_and(|p| r.rect.contains(p));
    if let Some((start, ref mut held)) = state {
        let steady = pos.zip(origin).is_some_and(|(a, b)| a.distance(b) < 8.0);
        if down && over && steady {
            *held |= now - start >= 0.65;
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        } else if !released {
            state = None;
        }
    }
    let held = state.is_some_and(|s| s.1);
    let show = |ui: &mut Ui| {
        ui.set_max_width(230.0);
        ui.strong(title);
        ui.label(what);
        if !how.is_empty() {
            ui.label(how);
        }
        if !when.is_empty() {
            ui.small(when);
        }
    };
    if held && down && over {
        egui::Tooltip::for_widget(r).width(230.0).show(show);
    } else {
        r.clone().on_hover_ui(show);
    }
    ctx.data_mut(|d| {
        if !down {
            d.remove::<(f64, bool)>(id);
        } else if let Some(s) = state {
            d.insert_temp(id, s);
        } else {
            d.remove::<(f64, bool)>(id);
        }
    });
    held
}

pub fn button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, size: Vec2) -> Action {
    let image = icon.image(ui, 18.0);
    let button = if label.is_empty() {
        egui::Button::image(image)
    } else {
        egui::Button::image_and_text(image, label)
    };
    let button = button
        .selected(selected)
        .truncate()
        .image_tint_follows_text_color(false);
    let r = if size.x > 0.0 {
        ui.add_sized(egui::vec2(size.x, ui.spacing().interact_size.y), button)
    } else {
        ui.add(button.min_size(egui::vec2(size.x, ui.spacing().interact_size.y)))
    }.on_hover_cursor(egui::CursorIcon::PointingHand);
    r.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            if label.is_empty() {
                icon.hint().0
            } else {
                label
            },
        )
    });
    let (title, what, how, when) = icon.hint();
    let held = help(
        &r,
        if label.is_empty() { title } else { label },
        what,
        how,
        when,
    );
    Action { response: r, held }
}

pub fn compact(ui: &mut Ui, icon: Icon, selected: bool) -> Action {
    button(ui, icon, "", selected, egui::vec2(30.0, 28.0))
}

impl Icon {
    pub fn hint(self) -> (&'static str, &'static str, &'static str, &'static str) {
        use Icon::*;
        match self {
            Rebuild => ("Rebuild", "Regenerate the preview from the current design.", "Click to build now.", "Available when the previous build has finished."),
            Four => ("Four", "Show four independent viewports.", "Choose each pane's view and camera from its header.", "Compare several angles or editors together."),
            SplitHorizontal => ("Split Horizontal", "Stack two viewports above and below.", "Choose each pane's view from its header.", "Use when each view needs more width."),
            SplitVertical => ("Split Vertical", "Place two viewports side by side.", "Choose each pane's view from its header.", "Compare the ring with a second view or editor."),
            Single => ("Single", "Show the first viewport at full size.", "Click to return to one view.", "Use when a task needs the whole canvas."),
            Ghost => ("Comparison ghost", "Pin the current mesh as a transparent reference.", "Click again to remove the pinned reference.", "Compare your next edits with the last built shape."),
            Cutters => ("Setting cutters", "Show the volumes used to cut the stone seats.", "Click to show or hide the cutter overlay.", "Inspect the seat independently of the ring's selection tint."),
            Wire => ("Wireframe", "Draw the preview's triangle edges.", "Click to show or hide the wire overlay.", "Inspect mesh detail and topology."),
            Grid => ("Grid", "Show the viewport's reference grid.", "Click to show or hide the grid.", "Use the grid to judge the ring's orientation."),
            Locked | Unlocked => (
                "Lock view angle",
                "Keep the ring facing the same way while you edit.",
                "Tap to lock or unlock. Drag empty space to pan when locked; two fingers still pan and zoom.",
                "The ring navigator can still change the view deliberately.",
            ),
            Magnifier => (
                "Placement magnifier",
                "Show a live close-up above your contact, up to 2.5×.",
                "Touch or drag on the ring with Stamp, Path, Paint or Move. Tap this icon to hide or show the lens.",
                "The lens holds the whole stamp, easing off 2.5× for a large one; its own factor is printed on the rim.",
            ),
            TurnLeft | TurnRight => (
                "Quarter turn",
                "Turn the camera around the ring by 90°.",
                "Tap an arrow. Use Views for 90° tilts.",
                "Works while the view is locked, without changing the design.",
            ),
            Opposite => (
                "Opposite side",
                "Look from the opposite side of the ring.",
                "Tap to flip the view by 180°. Tap again to return.",
                "Use to compare shoulders or decorate the reverse.",
            ),
            Undo => (
                "Undo",
                "Take back the latest design change.",
                "Tap once per step. Redo brings it back.",
                "Use after an unwanted edit; camera movements are not design changes.",
            ),
            Redo => (
                "Redo",
                "Restore an edit you undid.",
                "Tap to move forward through edit history.",
                "Making a new edit replaces the redo branch.",
            ),
            Shape => (
                "Shape the ring",
                "Set the finger opening, band and signet face.",
                "Start with fit, then change width, thickness and the head.",
                "Do this before placing detailed ornament or stones.",
            ),
            Surface => (
                "Decorate the surface",
                "Add raised or engraved artwork to the ring.",
                "Choose a stamp, paint a stroke, or draw an editable path.",
                "Use for motifs, borders, lettering and relief.",
            ),
            Stones => (
                "Set stones",
                "Add and edit stones and their settings.",
                "Pick a stone to adjust its size, cut and placement.",
                "Check gaps with Spacing before exporting.",
            ),
            Casting => (
                "Check the design",
                "Inspect walls, draft and mould withdrawal.",
                "Choose the casting process and review the findings.",
                "Use after shape changes and before export.",
            ),
            Select => (
                "Select / orbit",
                "Pick a detail or turn the ring to inspect it.",
                "Tap to select. Drag empty space to orbit; two fingers pan and zoom.",
                "Empty-space drags also navigate while a drawing tool is selected.",
            ),
            Paint => (
                "Paint relief",
                "Draw raised or engraved strokes on the ring.",
                "Drag the pen or finger; lift to build the relief.",
                "Use for freehand marks. Path is better for points you want to edit later.",
            ),
            Stamp => (
                "Place a pattern",
                "Place an image as raised or engraved relief.",
                "Choose a thumbnail. Hover or touch the ring to preview; release to place.",
                "Use for a motif, monogram or repeated decoration.",
            ),
            Path => (
                "Draw a surface path",
                "Make a raised border or engraved line.",
                "Tap points, drag handles, then Apply. Edit width, taper and copies.",
                "Use for controlled curves following the ring surface.",
            ),
            Transform => (
                "Move ornament",
                "Move, rotate or resize an existing stamp.",
                "Pick a stamp, drag its handles, then Apply. Duplicate or mirror from the inspector.",
                "The original layer and masks stay editable.",
            ),
            Section => (
                "Inspect a section",
                "Cut the preview to see inside the ring.",
                "Drag the section handle or enter the cut position.",
                "Use to inspect the bore and local wall thickness; the design is unchanged.",
            ),
            Measure => (
                "Measure two points",
                "Read a straight distance and its X/Y/Z spans.",
                "Tap two points on the visible mesh. Clear to start again.",
                "This is a chord, not a distance following the curved surface.",
            ),
            Spacing => (
                "Check stone spacing",
                "Show the space around stones and their pavilions.",
                "Set the desired gap and inspect overlapping envelopes.",
                "Use before committing closely packed settings.",
            ),
            Mould => (
                "Open the mould",
                "Preview how the prepared pattern withdraws.",
                "Build the study, then move or play the opening.",
                "Use to understand a casting obstruction.",
            ),
            Layers => (
                "Design layers",
                "Find, hide or edit the details that build the ring.",
                "Pick a layer to edit; its checkbox includes it in the design.",
                "Use when several details overlap on the model.",
            ),
            More => (
                "Toolbox",
                "Find additional shape, surface, inspection and file tools.",
                "Open a group. Drag the header anywhere in the app to move this toolbox.",
                "Use Guide for a suggested order of work.",
            ),
            Guides => (
                "Dimension handles",
                "Show handles for direct size and placement edits.",
                "Drag a visible handle or use exact values in the inspector.",
                "Turn these off when you want a clear view of the ring.",
            ),
            Panel => (
                "Inspector",
                "Show or hide the controls for the current task.",
                "Drag its grip to resize. Tap Expand on a small panel to restore it.",
                "This control remains available even when the inspector is hidden.",
            ),
            Guide => (
                "Jewelry workflow",
                "A practical route from ring fit to finished design.",
                "Choose Fit, Shape, Decorate, Set stones, or Check & export.",
                "You can enter any stage without replacing your current project.",
            ),
            History => (
                "Edit history",
                "Review the saved steps in this design session.",
                "Choose a step to return to that state.",
                "Use for changes beyond a single Undo.",
            ),
            Files | Save | Export => (
                "Files and export",
                "Save an editable project or export a manufacturing file.",
                "Keep the ring project as well as the exported mesh.",
                "Review size, walls and process findings before manufacturing.",
            ),
            Pattern => (
                "Pattern library",
                "Browse the actual images used as surface relief.",
                "Light areas add more relief; dark areas add less. Choose a thumbnail.",
                "Use Stamp for a single motif, or tiling for a repeating texture.",
            ),
            Graph => (
                "Recipe graph",
                "Build a design from connected modelling operations.",
                "Connect outputs to compatible inputs and preview the result.",
                "Use for procedural designs you want to reuse or vary.",
            ),
            Workshop => (
                "CAD workshop",
                "Construct solids from sketches, sections and components.",
                "Build a candidate, inspect it, then Apply.",
                "Use for geometry that cannot be expressed as surface relief.",
            ),
            View | Fit => (
                "Camera and display",
                "Frame the ring and choose how it is shown.",
                "Choose a standard view or Fit to bring the complete ring back.",
                "These controls change the view, not the ring size.",
            ),
            ZoomIn | ZoomOut => (
                "Zoom",
                "Bring the view closer or farther away.",
                "Tap to change the camera magnification.",
                "The ring's physical dimensions stay the same.",
            ),
            Duplicate => (
                "Duplicate",
                "Copy the selected ornament as another editable stamp.",
                "Tap to add a copy beside the original, then move it with the handles.",
                "Use for repeated motifs with different placements.",
            ),
            Mirror => (
                "Mirror",
                "Reflect the ornament onto the opposite side of the band.",
                "Tap to add a reflected copy; adjust it with the handles. Undo removes the copy.",
                "Use for matching left and right shoulders.",
            ),
            Rotate => (
                "Rotate",
                "Turn the ornament within the surface.",
                "Drag the rotation handle or enter an angle.",
                "Use to align lettering or motifs to the ring.",
            ),
            Scale => (
                "Resize",
                "Change the ornament's width in millimetres.",
                "Drag the size handle or enter a width.",
                "The original image proportions are preserved.",
            ),
            Move => (
                "Move",
                "Slide the ornament along the ring surface.",
                "Drag its centre or enter angle and across-band position.",
                "Use to align a motif without rebuilding it.",
            ),
            Raise | Engrave => (
                "Relief direction",
                "Choose whether artwork adds or removes material.",
                "Raise stands above the surface; Engrave cuts into it.",
                "Check the remaining wall after deep engraving.",
            ),
            Close | Collapse => (
                "Hide controls",
                "Close or fold this toolbox.",
                "Use its toolbar button to bring it back.",
                "Your design and selected tool are preserved.",
            ),
            Expand => (
                "Expand inspector",
                "Restore a useful inspector size.",
                "Tap once, or drag the grip to set your own size.",
                "Use when the panel is too small to show its controls.",
            ),
            Reset => (
                "Reset controls",
                "Return these controls to their starting values.",
                "In Move ornament, reload the saved placement. In workspace settings, restore panel sizes and positions.",
                "The ring is unchanged until you apply a design edit.",
            ),
            Check => (
                "Apply",
                "Commit the preview as an editable design change.",
                "Review the preview, then tap. Undo takes it back.",
                "Use when the placement or shape is ready.",
            ),
            Add => (
                "Add",
                "Create another editable item.",
                "Choose the item and adjust it in the inspector.",
                "Use Layers to find it again later.",
            ),
            Delete => (
                "Remove",
                "Remove the selected item.",
                "Tap only when this is the item you intend to remove.",
                "Undo can restore a design edit.",
            ),
            Before => (
                "Compare before",
                "Temporarily show the ring before the latest edit.",
                "Keep this button pressed; release to return.",
                "Use to judge how much an edit changes the design.",
            ),
            Search => (
                "Search",
                "Find a tool or pattern by name.",
                "Type a few letters to narrow the choices.",
                "Clear the text to show everything again.",
            ),
            Help => (
                "Help",
                "Learn what a control does and when to use it.",
                "Hold a button with your finger or hover the pen for about 0.7 seconds.",
                "Guide shows the overall jewelry workflow.",
            ),
            CadBand | CadBox | CadCylinder | CadSphere | CadTorus | CadTwistedRing |
            CadExtrude | CadRevolve | CadSweep | CadTwist | CadLoft | CadUnion |
            CadSubtract | CadIntersect | CadFillet | CadChamfer | CadShell | CadPlace | CadSketch => (
                "CAD feature", "Create or edit a solid modeling operation.",
                "Choose a tool, edit its parameters, Preview, then Apply.",
                "Each operation keeps its source recipe in feature history.",
            ),
            Settings => (
                "Edit controls",
                "Adjust the selected part of the design.",
                "Tap a value to type an exact number; swipe to scroll.",
                "Use the named fields to make precise changes.",
            ),
            Self::NodeSource |
            Self::NodeShank |
            Self::NodeGenerator |
            Self::NodeAssembly |
            Self::NodeSink |
            Self::NodeCluster => (
                "Node family",
                "A group of graph nodes that do the same kind of work.",
                "Open the group to choose one; the node lands where you opened the menu.",
                "The add-node menu sorts every node into one of these.",
            ),
            Self::NodeBool |
            Self::NodeNumber |
            Self::NodeText |
            Self::NodeRange |
            Self::NodeSeries => (
                "Value node",
                "A literal the rest of the graph reads: a number, some text, a list.",
                "Type the value on the node, or wire it from somewhere else.",
                "Use one wherever the same figure feeds several nodes.",
            ),
            Self::NodeAbs |
            Self::NodeAdd |
            Self::NodeSub |
            Self::NodeMul |
            Self::NodeDiv |
            Self::NodePow |
            Self::NodeSqrt |
            Self::NodeMod |
            Self::NodeNeg |
            Self::NodeRound |
            Self::NodeTrig |
            Self::NodeAngle |
            Self::NodeCompare |
            Self::NodeMinmax |
            Self::NodeClamp |
            Self::NodeLerp |
            Self::NodeRemap => (
                "Arithmetic node",
                "One arithmetic step on the numbers wired into it.",
                "Wire numbers in; the result comes out of the single output.",
                "Compute a dimension rather than typing it in two places.",
            ),
            Self::NodeLogic |
            Self::NodeBranch |
            Self::NodeGate => (
                "Logic node",
                "Choose between values, or let them past only on a condition.",
                "Wire the test into the condition pin and the choices into the rest.",
                "Use it to switch a design between two arrangements.",
            ),
            Self::NodeList |
            Self::NodeItem |
            Self::NodeLength |
            Self::NodeMerge |
            Self::NodeSplit |
            Self::NodeSort |
            Self::NodeCull |
            Self::NodeWeave |
            Self::NodeGraft |
            Self::NodeFlatten |
            Self::NodeShift |
            Self::NodeSum |
            Self::NodeSlice |
            Self::NodePartition |
            Self::NodeConcat |
            Self::NodeJson => (
                "List node",
                "Reshape the list a pin carries: reorder, join, split or count it.",
                "Wire a list in; a pin fed several items runs its node once per item.",
                "Use it to lay out repeated ornament without repeating the nodes.",
            ),
            Self::NodeBorder |
            Self::NodeMilgrain |
            Self::NodeFlutes |
            Self::NodeOpenwork |
            Self::NodeTiling |
            Self::NodeEntry |
            Self::NodeWindow |
            Self::NodeCurve |
            Self::NodeTerrace |
            Self::NodeSeatrun |
            Self::NodeSignetpad |
            Self::NodeGroup |
            Self::NodeDecal |
            Self::NodeFit |
            Self::NodeSolve => (
                "Layer node",
                "One layer of the height field the ring is built from.",
                "Wire it into a stack; the entry above it carries its blend and window.",
                "Every ornament on the band is a layer like this one.",
            ),
            Self::NodeProfile |
            Self::NodeSize |
            Self::NodeHead |
            Self::NodeOutline => (
                "Band node",
                "The band itself: its section, its size, or the head it carries.",
                "Set the millimetres on the node, or wire them from a value node.",
                "Start a graph here; everything else is laid on what this makes.",
            ),
            Self::NodePave |
            Self::NodeHalo |
            Self::NodeChannel |
            Self::NodeImage |
            Self::NodeScript |
            Self::NodeVerdict |
            Self::NodeReport |
            Self::NodeRender |
            Self::NodeBuild |
            Self::NodeRefine |
            Self::NodeInfo => (
                "Sink node",
                "The end of a chain: a verdict, a report, a mesh or a file.",
                "Wire the finished design in; a file sink only runs when the graph is run.",
                "A sand graph refuses to write a ring the field verdict fails.",
            ),
        }
    }
    pub fn for_label(label: &str) -> Self {
        use Icon::*;
        match label {
            "Undo" => Undo,
            "Redo" => Redo,
            "Shape" | "Signet face" | "Band & fit" | "All shape controls" => Shape,
            "Surface" => Surface,
            "Stones" | "Add stone setting" => Stones,
            "Casting" | "Casting findings" => Casting,
            "Select" => Select,
            "Edit" | "Full layer controls" => Settings,
            "Paint" | "Paint on ring" | "Paint the band" => Paint,
            "Stamp" | "Place an alpha" => Stamp,
            "Path" | "Draw a surface path" => Path,
            "Move" | "Move ornament" => Transform,
            "Section" => Section,
            "Measure" | "Measurements" => Measure,
            "Spacing" => Spacing,
            "Mould" => Mould,
            "Layers" | "Design layers" => Layers,
            "More" | "Tools" => More,
            "Guides" => Guides,
            "Panel" => Panel,
            "Before" => Before,
            "Jewelry workflow" | "Construction guide" | "Guide" => Guide,
            "Edit history" => History,
            "File" | "Files & exports" => Files,
            "New" => Shape,
            "Pattern library" | "Patterns & alphas" => Pattern,
            "Recipe graph" => Graph,
            "CAD & mould workshop" => Workshop,
            "View" | "Signet 3/4" => View,
            "Fit ring" => Fit,
            "Zoom in" => ZoomIn,
            "Zoom out" => ZoomOut,
            "Reset workspace layout" => Reset,
            "Raise" => Raise,
            "Engrave" => Engrave,
            "Duplicate" => Duplicate,
            "Mirror" => Mirror,
            "Apply" => Check,
            "Cancel" | "Hide" | "×" => Close,
            _ => Settings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_icon_renders_visible_svg_and_has_help() {
        for &icon in Icon::ALL {
            let tree = resvg::usvg::Tree::from_str(icon.svg(), &Default::default()).unwrap();
            let mut pix = resvg::tiny_skia::Pixmap::new(24, 24).unwrap();
            resvg::render(&tree, Default::default(), &mut pix.as_mut());
            assert!(
                pix.data().chunks_exact(4).filter(|p| p[3] > 32).count() > 12,
                "{icon:?}"
            );
            let (title, what, how, when) = icon.hint();
            assert!([title, what, how, when].iter().all(|s| !s.is_empty()));
        }
    }
    #[test]
    fn hold_explains_without_firing_on_release_then_normal_tap_works() {
        let ctx = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;
        let mut frame = |time, events| {
            let mut state = (false, false);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320.0, 480.0),
                    )),
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        let r = compact(ui, Icon::Stamp, false);
                        rect = r.rect;
                        state = (r.clicked(), r.held);
                    });
                },
            );
            output.textures_delta.clear();
            state
        };
        frame(0.0, vec![]);
        frame(0.02, vec![]);
        let at = egui::pos2(20.0, 20.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(0.1, vec![egui::Event::PointerMoved(at), press(true)]);
        assert_eq!(frame(0.8, vec![]), (false, true));
        assert_eq!(frame(0.9, vec![press(false)]), (false, true));
        frame(1.0, vec![press(true)]);
        assert_eq!(frame(1.1, vec![press(false)]), (true, false));
    }
}
