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
    Delete,
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
        Self::Delete,
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
            Self::Delete => include_str!("../assets/icons/delete.svg"),
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
        egui::Image::new((texture.id(), Vec2::splat(side)))
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
        .image_tint_follows_text_color(true);
    let r = if size.x > 0.0 {
        ui.add_sized(size, button)
    } else {
        ui.add(button.min_size(size))
    };
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
            Locked | Unlocked => (
                "Lock view angle",
                "Keep the ring facing the same way while you edit.",
                "Tap to lock or unlock. Drag empty space to pan when locked; two fingers still pan and zoom.",
                "The ring navigator can still change the view deliberately.",
            ),
            Magnifier => (
                "Placement magnifier",
                "Show a live 2.5× close-up above your contact.",
                "Touch or drag on the ring with Stamp, Path, Paint or Move. Tap this icon to hide or show the lens.",
                "The crosshair marks the actual contact; the image includes the surface overlay.",
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
            Settings => (
                "Edit controls",
                "Adjust the selected part of the design.",
                "Tap a value to type an exact number; swipe to scroll.",
                "Use the named fields to make precise changes.",
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
            "Files & exports" => Files,
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
