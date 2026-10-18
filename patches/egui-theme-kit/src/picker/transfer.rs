//! The import and export section: theme files, the clipboard and a paste box.

use eframe::egui::{RichText, ScrollArea, TextEdit, Ui};

use super::{label, state::PickerState};
use crate::kit::{self, ButtonKind};

/// Tallest the JSON box grows before it scrolls.
const JSON_BOX_HEIGHT: f32 = 140.0;

pub(super) fn show(ui: &mut Ui, state: &mut PickerState) {
    let ctx = ui.ctx().clone();
    ui.horizontal_wrapped(|ui| {
        #[cfg(feature = "files")]
        {
            if ui
                .add(kit::button(
                    ButtonKind::Neutral,
                    label(state, "export", "Export to file…"),
                ))
                .clicked()
            {
                super::io::spawn_export(
                    &ctx,
                    &state.inbox,
                    crate::saved::SavedTheme::current(&ctx),
                );
            }
            if ui
                .add(kit::button(
                    ButtonKind::Neutral,
                    label(state, "import", "Import from file…"),
                ))
                .clicked()
            {
                super::io::spawn_import(&ctx, &state.inbox);
            }
        }
        if ui
            .add(kit::button(
                ButtonKind::Neutral,
                label(state, "copy", "Copy JSON"),
            ))
            .clicked()
        {
            ctx.copy_text(state.export_json(&ctx));
        }
        if ui
            .add(kit::button(ButtonKind::Ghost, "Show current JSON"))
            .clicked()
        {
            state.json = state.export_json(&ctx);
        }
    });
    ScrollArea::vertical()
        .id_salt("egui_theme_kit.picker.json")
        .max_height(JSON_BOX_HEIGHT)
        .show(ui, |ui| {
            ui.add(
                TextEdit::multiline(&mut state.json)
                    .code_editor()
                    .hint_text("Paste theme JSON: a saved theme or a bare egui Style")
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
        });
    ui.horizontal(|ui| {
        let ready = !state.json.trim().is_empty();
        if ui
            .add_enabled(
                ready,
                kit::button(ButtonKind::Primary, label(state, "apply", "Apply JSON")),
            )
            .clicked()
        {
            let text = state.json.clone();
            state.import_json(&ctx, &text);
        }
        if ui
            .add_enabled(ready, kit::button(ButtonKind::Ghost, "Clear"))
            .clicked()
        {
            state.json.clear();
        }
    });
    ui.label(
        RichText::new("Imported themes apply at once; use Save as to keep them.")
            .weak()
            .small(),
    );
}
