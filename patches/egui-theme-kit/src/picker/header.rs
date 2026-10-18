//! The picker header: what is applied, whether it was modified, and the revert and save controls.

use eframe::egui::{Frame, Key, RichText, TextEdit, Ui};

use super::{
    label,
    state::{MessageKind, Origin, PickerState},
};
use crate::{
    colors,
    fonts::RichTextExt as _,
    kit::{self, ButtonKind},
    store::{StartupTheme, ThemeStore},
};

/// Width of the name field beside "Save as".
const NAME_FIELD_W: f32 = 200.0;

pub(super) fn show(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        title_row(ui, state, store);
        action_row(ui, state, store);
        save_row(ui, state, store);
        message_rows(ui, state, store);
    });
}

fn title_row(ui: &mut Ui, state: &PickerState, store: &dyn ThemeStore) {
    let ctx = ui.ctx().clone();
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(state.current_name(&ctx))
                .semibold()
                .size(ui.text_style_height(&eframe::egui::TextStyle::Heading)),
        );
        if state.is_modified() {
            ui.label(RichText::new("• modified").color(colors::warn(ui)));
        }
        if let Some(from) = state
            .origin_name()
            .filter(|from| *from != state.current_name(&ctx))
        {
            ui.label(RichText::new(format!("based on {from}")).weak());
        }
    });
    let startup = match store.startup() {
        StartupTheme::LastUsed => "the last theme used".to_owned(),
        StartupTheme::User(name) => name,
        StartupTheme::Preset(id) => crate::presets::preset(&id).map_or(id, |d| d.name.to_owned()),
    };
    ui.label(
        RichText::new(format!("The app starts with {startup}."))
            .weak()
            .small(),
    );
}

fn action_row(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    let ctx = ui.ctx().clone();
    ui.horizontal_wrapped(|ui| {
        let modified = state.is_modified();
        let revert = ui
            .add_enabled(
                state.can_revert(),
                kit::button(ButtonKind::Ghost, label(state, "revert", "Revert")),
            )
            .on_hover_text("Return to the theme this window opened with.");
        if revert.clicked() {
            state.revert(&ctx);
        }
        let reset = ui
            .add_enabled(
                modified,
                kit::button(
                    ButtonKind::Ghost,
                    label(state, "reset", state.reset_label()),
                ),
            )
            .on_hover_text("Drop colour and glass edits made since the theme was applied.");
        if reset.clicked() {
            state.reset_to_origin(&ctx);
        }
        if let Some(Origin::User(name)) = state.origin().cloned() {
            let save = ui
                .add_enabled(
                    modified,
                    kit::button(ButtonKind::Primary, label(state, "save", "Save")),
                )
                .on_hover_text(format!(
                    "Replace the saved theme {name} with the live theme."
                ));
            if save.clicked() {
                state.save_origin(&ctx, store);
            }
        }
        if matches!(store.startup(), StartupTheme::LastUsed) {
            return;
        }
        let use_last = ui
            .add(kit::button(
                ButtonKind::Ghost,
                label(state, "startup", "Start with the last theme"),
            ))
            .on_hover_text("Stop pinning a startup theme.");
        if use_last.clicked() {
            state.set_startup(store, StartupTheme::LastUsed);
        }
    });
}

fn save_row(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    let ctx = ui.ctx().clone();
    ui.horizontal(|ui| {
        let field = ui.add(
            TextEdit::singleline(&mut state.save_name)
                .hint_text("Name for a new saved theme")
                .desired_width(NAME_FIELD_W * crate::fonts::text_scale(&ctx).factor()),
        );
        let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let named = !state.save_name.trim().is_empty();
        let clicked = ui
            .add_enabled(
                named,
                kit::button(ButtonKind::Neutral, label(state, "save_as", "Save as…")),
            )
            .on_hover_text("Store the live theme under this name.")
            .clicked();
        if (clicked || entered) && named {
            let name = state.save_name.clone();
            if state.save_as(&ctx, store, &name, false) {
                state.save_name.clear();
            }
        }
    });
}

fn message_rows(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    if let Some(message) = state.message() {
        let color = match message.kind {
            MessageKind::Info => colors::weak_text(ui),
            MessageKind::Success => colors::success(ui),
            MessageKind::Error => colors::error(ui),
        };
        ui.label(RichText::new(message.text.clone()).color(color));
    }
    if let Some(error) = store.last_error() {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!("{}: {error}", label(state, "warning", "Storage")))
                    .color(colors::error(ui)),
            );
            if ui.add(kit::button(ButtonKind::Ghost, "Dismiss")).clicked() {
                store.clear_error();
            }
        });
    }
}
