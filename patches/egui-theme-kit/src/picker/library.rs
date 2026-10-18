//! The saved-themes list: apply, save over, rename, duplicate, delete and pin a startup theme.

use eframe::egui::{Frame, Key, RichText, Sense, TextEdit, Ui, vec2};

use super::{
    age, label,
    preview::paint_strip,
    state::{Origin, PickerState},
};
use crate::{
    colors,
    fonts::RichTextExt as _,
    kit::{self, ButtonKind},
    store::{StartupTheme, ThemeStore},
    tokens::Tokens,
};

/// Size of the swatch strip beside a theme name.
const STRIP: (f32, f32) = (72.0, 10.0);
/// Width of the inline rename field.
const RENAME_FIELD_W: f32 = 180.0;

pub(super) fn show(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    if state.users.is_empty() {
        ui.label(
            RichText::new(
                "No saved themes yet. Customize a theme, then name it above with Save as.",
            )
            .weak(),
        );
        return;
    }
    let startup = store.startup();
    let rows: Vec<(String, u64)> = state
        .users
        .iter()
        .map(|named| (named.name.clone(), named.updated_at))
        .collect();
    for (name, updated_at) in rows {
        row(ui, state, store, &Row { name, updated_at }, &startup);
    }
}

/// What a row shows of a saved theme.
struct Row {
    name: String,
    updated_at: u64,
}

fn row(
    ui: &mut Ui,
    state: &mut PickerState,
    store: &dyn ThemeStore,
    named: &Row,
    startup: &StartupTheme,
) {
    let ctx = ui.ctx().clone();
    let live = Tokens::get(&ctx);
    let applied = state
        .live
        .as_ref()
        .is_some_and(|l| l.applied_user.as_deref() == Some(named.name.as_str()));
    let origin = matches!(state.origin(), Some(Origin::User(n)) if n == &named.name);
    let pinned = matches!(startup, StartupTheme::User(n) if n.eq_ignore_ascii_case(&named.name));
    let renaming = state
        .rename
        .as_ref()
        .is_some_and(|(from, _)| from == &named.name);
    let deleting = state.confirm_delete.as_deref() == Some(named.name.as_str());

    Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            if let Some(tokens) = state.user_tokens.get(&named.name) {
                let (rect, _) = ui.allocate_exact_size(vec2(STRIP.0, STRIP.1), Sense::hover());
                paint_strip(ui, rect, &tokens.swatches(), live.card_border);
            }
            if renaming {
                rename_field(ui, state, store, &named.name);
            } else {
                ui.label(RichText::new(&named.name).semibold());
                if applied {
                    ui.label(RichText::new("• applied").color(colors::primary(ui)));
                }
                if pinned {
                    ui.label(RichText::new("• startup").color(colors::secondary(ui)));
                }
                ui.label(RichText::new(age(named.updated_at)).weak().small());
            }
        });
        if renaming {
            return;
        }
        if deleting {
            confirm_row(ui, state, store, &named.name);
            return;
        }
        buttons(ui, state, store, named, origin, pinned);
    });
}

fn buttons(
    ui: &mut Ui,
    state: &mut PickerState,
    store: &dyn ThemeStore,
    named: &Row,
    origin: bool,
    pinned: bool,
) {
    let ctx = ui.ctx().clone();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(kit::button(
                ButtonKind::Primary,
                label(state, "apply", "Apply"),
            ))
            .clicked()
        {
            state.apply_user_theme(&ctx, store, &named.name);
        }
        if origin
            && state.is_modified()
            && ui
                .add(kit::button(
                    ButtonKind::Neutral,
                    label(state, "save", "Save over"),
                ))
                .on_hover_text("Replace this theme with the live theme.")
                .clicked()
        {
            state.save_origin(&ctx, store);
        }
        if ui
            .add(kit::button(
                ButtonKind::Ghost,
                label(state, "rename", "Rename"),
            ))
            .clicked()
        {
            state.rename = Some((named.name.clone(), named.name.clone()));
            state.rename_focus = true;
            state.confirm_delete = None;
        }
        if ui
            .add(kit::button(
                ButtonKind::Ghost,
                label(state, "duplicate", "Duplicate"),
            ))
            .clicked()
        {
            state.duplicate(store, &named.name);
        }
        let startup = if pinned {
            kit::button(
                ButtonKind::Ghost,
                label(state, "startup", "Use last theme at start"),
            )
        } else {
            kit::button(
                ButtonKind::Ghost,
                label(state, "startup", "Start with this"),
            )
        };
        if ui
            .add(startup)
            .on_hover_text("Pin this theme as the one the app starts with.")
            .clicked()
        {
            let choice = if pinned {
                StartupTheme::LastUsed
            } else {
                StartupTheme::User(named.name.clone())
            };
            state.set_startup(store, choice);
        }
        if ui
            .add(kit::button(
                ButtonKind::Danger,
                label(state, "delete", "Delete"),
            ))
            .clicked()
        {
            state.confirm_delete = Some(named.name.clone());
            state.rename = None;
        }
    });
}

fn rename_field(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore, from: &str) {
    let Some((_, to)) = state.rename.as_mut() else {
        return;
    };
    let field = ui.add(TextEdit::singleline(to).desired_width(RENAME_FIELD_W));
    let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if std::mem::take(&mut state.rename_focus) {
        field.request_focus();
    }
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let to = to.clone();
    let accept = ui
        .add(kit::button(
            ButtonKind::Primary,
            label(state, "rename", "Rename"),
        ))
        .clicked()
        || enter;
    let cancel = ui.add(kit::button(ButtonKind::Ghost, "Cancel")).clicked() || escape;
    if accept {
        if state.rename(store, from, &to) {
            state.rename = None;
        }
    } else if cancel {
        state.rename = None;
    }
}

fn confirm_row(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore, name: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("Delete {name}? This cannot be undone."))
                .color(colors::error(ui)),
        );
        if ui
            .add(kit::button(
                ButtonKind::Danger,
                label(state, "delete", "Delete"),
            ))
            .clicked()
        {
            state.delete(store, name);
        }
        if ui.add(kit::button(ButtonKind::Ghost, "Cancel")).clicked() {
            state.confirm_delete = None;
        }
    });
}
