//! The theme settings picker: preset gallery, saved themes, customization and import/export in one embeddable panel.
//!
//! [`show`] draws the panel into any `Ui`; [`window`] wraps it in a window opened with
//! [`PickerState::open`]. Every change applies to the live theme at once and reaches the
//! [`ThemeStore`] a moment after the theme stops changing. The panel draws with whatever theme is
//! active, using stock egui widgets and the kit's own widget theme.
//!
//! ```ignore
//! let store = std::sync::Arc::new(JsonDirStore::open(dir)?);
//! persist::apply_startup_from_store(&ctx, &*store, None);
//! let mut picker = PickerState::default();
//!
//! // Every frame, in a panel or a window.
//! picker::show(ui, &mut picker, &*store);
//!
//! // On exit, so a change made in the last moments is not lost.
//! picker.flush(&ctx, &*store);
//! ```

pub mod edit;
pub mod preview;

mod customize;
mod gallery;
mod header;
mod io;
mod library;
mod state;
mod transfer;

use eframe::egui::{Context, Id, RichText, ScrollArea, Ui, Window, vec2};

pub use state::{Message, MessageKind, Origin, PickerOptions, PickerState};

use crate::{fonts::RichTextExt as _, store::ThemeStore};

/// Action names passed to [`PickerOptions::icon`].
pub const ICON_NAMES: [&str; 15] = [
    "apply",
    "revert",
    "reset",
    "save",
    "save_as",
    "rename",
    "duplicate",
    "delete",
    "startup",
    "import",
    "export",
    "copy",
    "search",
    "warning",
    "glass",
];

/// Draws the picker into `ui`; call every frame.
pub fn show(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    let ctx = ui.ctx().clone();
    state.sync(&ctx, store);
    ScrollArea::vertical()
        .id_salt("egui_theme_kit.picker.scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            header::show(ui, state, store);
            section(ui, "Presets", true, |ui| gallery::show(ui, state, store));
            section(ui, "Saved themes", true, |ui| {
                library::show(ui, state, store)
            });
            if state.options.customize {
                section(ui, "Customize", false, |ui| customize::show(ui, state));
            }
            if state.options.import_export {
                section(ui, "Import and export", false, |ui| {
                    transfer::show(ui, state)
                });
            }
        });
}

/// Draws the picker in a window while [`PickerState::open`] is set; call every frame.
pub fn window(ctx: &Context, state: &mut PickerState, store: &dyn ThemeStore) {
    if !state.open {
        return;
    }
    let mut open = true;
    Window::new("Theme settings")
        .id(Id::unique("egui_theme_kit.picker.window"))
        .open(&mut open)
        .default_size(vec2(660.0, 740.0))
        .show(ctx, |ui| show(ui, state, store));
    if !open {
        state.flush(ctx, store);
        state.open = false;
    }
}

fn section(ui: &mut Ui, title: &str, open: bool, add: impl FnOnce(&mut Ui)) {
    ui.add_space(ui.spacing().item_spacing.y);
    eframe::egui::CollapsingHeader::new(RichText::new(title).semibold())
        .id_salt(("egui_theme_kit.picker.section", title))
        .default_open(open)
        .show(ui, add);
}

/// `text` prefixed with the glyph the options map `icon` to.
fn label(state: &PickerState, icon: &str, text: &str) -> String {
    match state.options.icon.and_then(|hook| hook(icon)) {
        Some(glyph) => format!("{glyph} {text}"),
        None => text.to_owned(),
    }
}

/// A rough "3 min ago" for a Unix timestamp.
fn age(updated_at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let seconds = now.saturating_sub(updated_at);
    match seconds {
        _ if updated_at == 0 => "date unknown".to_owned(),
        0..=44 => "just now".to_owned(),
        45..=3_599 => format!("{} min ago", (seconds / 60).max(1)),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        86_400..=2_591_999 => format!("{} d ago", seconds / 86_400),
        _ => format!("{} mo ago", seconds / 2_592_000),
    }
}

#[cfg(test)]
mod tests;
