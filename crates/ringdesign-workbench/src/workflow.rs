//! Plain-language entry points. Navigation never replaces a user's design.
use crate::{
    icons::{self, Icon},
    visual::Tool,
};
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Fit,
    Shape,
    Tool(Tool),
    Stones,
    Checks,
    Export,
}
pub fn show(ui: &mut egui::Ui) -> Option<Action> {
    ui.label("Build a ring in five steps. Return to any step as your design develops.");
    ui.small("Tap to use a tool. Hold an icon for help; hover the pen for 0.7 seconds. Undo reverses design edits.");
    let mut action = None;
    for (title, why, choices) in [
        (
            "1 · Fit the finger",
            "Start with the inside diameter, width and metal thickness.",
            vec![(Icon::Shape, "Set fit", Action::Fit)],
        ),
        (
            "2 · Shape the ring",
            "Build the silhouette and a smooth shoulder before adding decoration.",
            vec![(Icon::Shape, "Shape & face", Action::Shape)],
        ),
        (
            "3 · Add your artwork",
            "Stamp places an image as relief. Path draws a raised wire or engraved line. Move edits a stamp already placed.",
            vec![
                (Icon::Stamp, "Stamp", Action::Tool(Tool::Stamp)),
                (Icon::Path, "Draw path", Action::Tool(Tool::Path)),
                (Icon::Transform, "Move", Action::Tool(Tool::Transform)),
            ],
        ),
        (
            "4 · Place stones",
            "Choose a setting, size the stone and inspect the space around it.",
            vec![
                (Icon::Stones, "Stone settings", Action::Stones),
                (
                    Icon::Spacing,
                    "Check spacing",
                    Action::Tool(Tool::Clearance),
                ),
            ],
        ),
        (
            "5 · Inspect & export",
            "Measure a wall with Section. Review casting findings and save the design before exporting a pattern.",
            vec![
                (Icon::Section, "Section", Action::Tool(Tool::Section)),
                (Icon::Casting, "Review", Action::Checks),
                (Icon::Export, "Save / export", Action::Export),
            ],
        ),
    ] {
        ui.separator();
        ui.strong(title);
        ui.label(why);
        ui.horizontal_wrapped(|ui| {
            for (icon, label, next) in choices {
                if icons::button(ui, icon, label, false, egui::vec2(0.0, 28.0)).clicked() {
                    action = Some(next);
                }
            }
        });
    }
    ui.separator();
    ui.small(if cfg!(target_os="android") {"One finger uses the selected tool. Two fingers move the view. Select restores one-finger orbit. Drag a toolbox by its title anywhere in the app. The top Panel icon restores a hidden or small inspector."}else{"Select drags the view; Shift-drag pans and the wheel zooms. Drag tool windows by their title to keep the ring clear."});
    action
}
