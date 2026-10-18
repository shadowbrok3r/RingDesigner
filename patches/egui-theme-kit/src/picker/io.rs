//! Theme JSON text and the file dialogs that move it in and out of the picker.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(feature = "files")]
use eframe::egui::Context;

use crate::saved::{SavedTheme, theme_to_json};

/// Outcome of a file dialog run off the UI thread.
#[cfg_attr(not(feature = "files"), allow(dead_code))]
pub(super) enum DialogResult {
    Imported(Result<Box<SavedTheme>, String>, PathBuf),
    Exported(Result<PathBuf, String>),
}

/// Results queued by dialog threads for the picker to drain.
#[cfg_attr(not(feature = "files"), allow(dead_code))]
pub(super) type Inbox = Arc<Mutex<Vec<DialogResult>>>;

/// `theme` as indented JSON.
pub(super) fn pretty_json(theme: &SavedTheme) -> String {
    let compact = theme_to_json(theme).unwrap_or_default();
    serde_json::from_slice::<serde_json::Value>(&compact)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| String::from_utf8_lossy(&compact).into_owned())
}

#[cfg(feature = "files")]
fn push(ctx: &Context, inbox: &Inbox, result: DialogResult) {
    if let Ok(mut queue) = inbox.lock() {
        queue.push(result);
    }
    ctx.request_repaint();
}

/// Asks for a file name on a worker thread and writes the live theme there.
#[cfg(feature = "files")]
pub(super) fn spawn_export(ctx: &Context, inbox: &Inbox, theme: SavedTheme) {
    let (ctx, inbox) = (ctx.clone(), Arc::clone(inbox));
    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export theme")
            .set_file_name("theme.json")
            .add_filter("Json", &["json"])
            .save_file()
        else {
            return;
        };
        let result = std::fs::write(&path, pretty_json(&theme))
            .map_err(|e| e.to_string())
            .map(|()| path);
        push(&ctx, &inbox, DialogResult::Exported(result));
    });
}

/// Asks for a theme file on a worker thread and reads it.
#[cfg(feature = "files")]
pub(super) fn spawn_import(ctx: &Context, inbox: &Inbox) {
    let (ctx, inbox) = (ctx.clone(), Arc::clone(inbox));
    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import theme")
            .add_filter("Json", &["json"])
            .pick_file()
        else {
            return;
        };
        let result = crate::persist::load_file(&path)
            .map(Box::new)
            .map_err(|e| format!("{e:#}"));
        push(&ctx, &inbox, DialogResult::Imported(result, path));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saved::theme_from_json;

    #[test]
    fn pretty_json_is_indented_and_reads_back() {
        let theme = crate::presets::PresetStyles::Noir.def().unwrap().theme();
        let text = pretty_json(&theme);
        assert!(text.contains("\n  "));
        let back = theme_from_json(text.as_bytes()).unwrap();
        assert!(crate::presets::themes_equal(&back, &theme));
    }
}
