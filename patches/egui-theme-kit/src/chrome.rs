//! Window chrome choice: the app-drawn title bar or the system one, resolved at startup and switchable live.
//!
//! [`resolve`] reads the saved [`ChromePrefs`], the `--system-titlebar` flag and the
//! `EGUI_THEME_KIT_SYSTEM_TITLEBAR` variable into a [`Chrome`]; [`viewport`] applies it to the
//! viewport builder; [`set_frameless`] and [`toggle_row`] switch it while the app runs and save the
//! choice. Preferences live in `<config_dir>/<app_id>/window_settings.json` with the `files`
//! feature and in process memory without it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use eframe::egui::{
    Context, Id, Response, Sense, Ui, Vec2, ViewportBuilder, ViewportCommand, vec2,
};
use serde::{Deserialize, Serialize};

use crate::controls::{menu_inset_row, toggle};

/// Command-line flag that keeps the system title bar for one launch.
pub const SYSTEM_TITLEBAR_FLAG: &str = "--system-titlebar";
/// Environment variable that keeps the system title bar for one launch.
pub const SYSTEM_TITLEBAR_ENV: &str = "EGUI_THEME_KIT_SYSTEM_TITLEBAR";
/// File name of the saved preferences inside the app's config directory.
pub const PREFS_FILE: &str = "window_settings.json";
/// Smallest inner size [`viewport`] gives a window that sets none.
pub const MIN_INNER_SIZE: Vec2 = vec2(360.0, 200.0);

/// How the main window is framed in this run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chrome {
    /// The app draws the title bar, caption buttons and resize edges.
    pub frameless: bool,
    /// The window can drop its system decorations; false for software-renderer fallbacks.
    pub available: bool,
}

impl Chrome {
    /// System decorations only; for a window the app cannot make frameless, such as a software-renderer fallback.
    pub const UNAVAILABLE: Self = Self {
        frameless: false,
        available: false,
    };
}

/// The saved window-chrome preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromePrefs {
    /// Draw the app's own title bar; on when nothing is saved.
    #[serde(default = "custom_by_default")]
    pub custom_title_bar: bool,
}

fn custom_by_default() -> bool {
    true
}

impl Default for ChromePrefs {
    fn default() -> Self {
        Self {
            custom_title_bar: custom_by_default(),
        }
    }
}

/// Preferences kept in memory for ids with no file location.
static MEMORY: Mutex<Option<HashMap<String, ChromePrefs>>> = Mutex::new(None);
/// The app id [`resolve`] last saw; the key [`set_frameless`] saves under.
static APP_ID: Mutex<String> = Mutex::new(String::new());

impl ChromePrefs {
    /// `<config_dir>/<app_id>/window_settings.json`; `None` without the `files` feature, a config directory or a plain `app_id`.
    pub fn path(app_id: &str) -> Option<PathBuf> {
        #[cfg(feature = "files")]
        {
            let plain = !app_id.is_empty()
                && app_id != "."
                && app_id != ".."
                && !app_id.contains(['/', '\\', '\0']);
            if !plain {
                return None;
            }
            directories::BaseDirs::new().map(|dirs| dirs.config_dir().join(app_id).join(PREFS_FILE))
        }
        #[cfg(not(feature = "files"))]
        {
            let _ = app_id;
            None
        }
    }

    /// The preferences saved for `app_id`; the defaults when missing or unreadable.
    pub fn load(app_id: &str) -> Self {
        match Self::path(app_id) {
            Some(path) => Self::load_from(&path),
            None => lock(&MEMORY)
                .as_ref()
                .and_then(|map| map.get(app_id).copied())
                .unwrap_or_default(),
        }
    }

    /// Saves the preferences for `app_id`.
    pub fn save(&self, app_id: &str) -> anyhow::Result<()> {
        match Self::path(app_id) {
            Some(path) => self.save_to(&path),
            None => {
                lock(&MEMORY)
                    .get_or_insert_with(HashMap::new)
                    .insert(app_id.to_owned(), *self);
                Ok(())
            }
        }
    }

    /// The preferences in the file at `path`; the defaults when it is missing or not valid JSON.
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Writes the preferences to `path` through a temporary file, creating its directory.
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let temp = path.with_extension("json.tmp");
        let written =
            write_synced(&temp, json.as_bytes()).and_then(|()| std::fs::rename(&temp, path));
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        Ok(written?)
    }
}

/// Creates `path` and writes `bytes` to it; the handle is opened for writing so `sync_all` is allowed on Windows.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether `value` turns an environment switch on: anything but empty, `0`, `false`, `off` or `no`.
fn truthy(value: &str) -> bool {
    !matches!(value.trim(), "" | "0" | "false" | "off" | "no")
}

/// The chrome for this run from the saved preference, `args` and `env`.
///
/// [`SYSTEM_TITLEBAR_FLAG`] in `args` or a truthy [`SYSTEM_TITLEBAR_ENV`] keeps the system title
/// bar for this launch without touching the saved preference. Records `app_id` as the key
/// [`set_frameless`] saves under. The result is always [`Chrome::available`]; replace it with
/// [`Chrome::UNAVAILABLE`] when the window falls back to a software renderer.
pub fn resolve(
    app_id: &str,
    args: impl IntoIterator<Item = String>,
    env: impl Fn(&str) -> Option<String>,
) -> Chrome {
    *lock(&APP_ID) = app_id.to_owned();
    resolve_with(ChromePrefs::load(app_id), args, env)
}

/// [`resolve`] over an explicit `prefs`, touching neither the disk nor the recorded app id.
pub fn resolve_with(
    prefs: ChromePrefs,
    args: impl IntoIterator<Item = String>,
    env: impl Fn(&str) -> Option<String>,
) -> Chrome {
    let flagged = args.into_iter().any(|arg| arg == SYSTEM_TITLEBAR_FLAG);
    let forced = flagged || env(SYSTEM_TITLEBAR_ENV).is_some_and(|v| truthy(&v));
    Chrome {
        frameless: prefs.custom_title_bar && !forced,
        available: true,
    }
}

/// [`resolve`] over the process arguments and environment.
pub fn resolve_from_process(app_id: &str) -> Chrome {
    resolve(app_id, std::env::args().skip(1), |key| {
        std::env::var(key).ok()
    })
}

/// `builder` with the system decorations dropped when `chrome` is frameless, and [`MIN_INNER_SIZE`] when it has no minimum size.
///
/// The minimum applies in both modes so a window switched to the app's title bar later keeps a usable size.
pub fn viewport(builder: ViewportBuilder, chrome: &Chrome) -> ViewportBuilder {
    let frameless = chrome.frameless && chrome.available;
    let builder = builder.with_decorations(!frameless);
    if builder.min_inner_size.is_none() && chrome.available {
        builder.with_min_inner_size(MIN_INNER_SIZE)
    } else {
        builder
    }
}

/// Switches the window between the app's title bar and the system one and saves the choice.
///
/// Does nothing while `chrome` is not [`Chrome::available`]. The window switches even when the
/// returned error says the choice could not be saved.
pub fn set_frameless(ctx: &Context, chrome: &mut Chrome, frameless: bool) -> anyhow::Result<()> {
    if !chrome.available {
        return Ok(());
    }
    chrome.frameless = frameless;
    ctx.send_viewport_cmd(ViewportCommand::Decorations(!frameless));
    let app_id = lock(&APP_ID).clone();
    ChromePrefs {
        custom_title_bar: frameless,
    }
    .save(&app_id)
}

fn save_error_id() -> Id {
    Id::unique("egui_theme_kit.chrome.save_error")
}

/// The error of the last [`toggle_row`] save that failed, once.
pub fn take_save_error(ctx: &Context) -> Option<String> {
    ctx.data_mut(|d| {
        let error = d.get_temp::<String>(save_error_id());
        d.remove::<String>(save_error_id());
        error
    })
}

/// A menu row with a switch for the app's own title bar; draws nothing while `chrome` is not available.
///
/// A click calls [`set_frameless`]; a failed save is kept for [`take_save_error`].
pub fn toggle_row(ui: &mut Ui, chrome: &mut Chrome, label: &str) -> Response {
    if !chrome.available {
        return ui.allocate_response(Vec2::ZERO, Sense::hover());
    }
    let mut custom = chrome.frameless;
    let response = menu_inset_row(ui, |ui| toggle(ui, &mut custom, label)).on_hover_text(
        "Off: the system title bar. --system-titlebar also keeps it for one launch.",
    );
    if response.changed()
        && let Err(error) = set_frameless(ui.ctx(), chrome, custom)
    {
        log::error!("saving the title bar setting failed: {error}");
        ui.data_mut(|d| d.insert_temp(save_error_id(), error.to_string()));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fonts,
        metrics::{self, Density},
    };
    use eframe::egui::{
        Event, FontDefinitions, Modifiers, PointerButton, Pos2, RawInput, Rect, Style, ViewportId,
    };
    use std::sync::Mutex;

    /// Serializes the tests that read or write the process-wide app id.
    static GLOBAL: Mutex<()> = Mutex::new(());

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "egui-theme-kit-chrome-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn prefs(custom: bool) -> ChromePrefs {
        ChromePrefs {
            custom_title_bar: custom,
        }
    }

    fn headless() -> Context {
        let ctx = Context::default();
        let mut definitions = FontDefinitions::default();
        fonts::install(&mut definitions);
        ctx.set_fonts(definitions);
        metrics::set_density(&ctx, Density::Regular);
        metrics::install_overlaid_style(&ctx, Style::default());
        ctx
    }

    fn frame(
        ctx: &Context,
        time: f64,
        events: Vec<Event>,
        add: impl FnMut(&mut Ui),
    ) -> Vec<ViewportCommand> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, add);
        out.textures_delta.clear();
        out.viewport_output
            .remove(&ViewportId::ROOT)
            .map(|v| v.commands)
            .unwrap_or_default()
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn nothing_saved_means_a_custom_title_bar() {
        assert!(ChromePrefs::default().custom_title_bar);
        let dir = temp_dir("missing");
        assert!(ChromePrefs::load_from(&dir.join(PREFS_FILE)).custom_title_bar);
    }

    #[test]
    fn the_saved_choice_round_trips_and_bad_files_mean_custom() {
        let dir = temp_dir("round-trip");
        let path = dir.join("nested").join(PREFS_FILE);
        prefs(false)
            .save_to(&path)
            .expect("save creates the directory");
        assert!(!ChromePrefs::load_from(&path).custom_title_bar);
        assert!(
            !path.with_extension("json.tmp").exists(),
            "the temporary file was left behind"
        );
        prefs(true).save_to(&path).expect("overwrite");
        assert!(ChromePrefs::load_from(&path).custom_title_bar);
        for contents in ["{}", "", "not json", "[]", "{\"custom_title_bar\": 3}"] {
            std::fs::write(&path, contents).expect("write");
            assert!(
                ChromePrefs::load_from(&path).custom_title_bar,
                "{contents:?} must mean custom"
            );
        }
        std::fs::write(&path, "{\"custom_title_bar\": false, \"other\": 1}").expect("write");
        assert!(!ChromePrefs::load_from(&path).custom_title_bar);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_where_the_directory_cannot_exist_is_an_error() {
        let dir = temp_dir("blocked");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("file");
        std::fs::write(&file, "x").expect("write");
        assert!(prefs(false).save_to(&file.join(PREFS_FILE)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "files")]
    #[test]
    fn the_prefs_file_lives_in_the_app_config_directory() {
        let path = ChromePrefs::path("my-app").expect("a config directory");
        assert!(path.ends_with(std::path::Path::new("my-app").join(PREFS_FILE)));
        for id in ["", ".", "..", "a/b", "a\\b", "../x"] {
            assert_eq!(ChromePrefs::path(id), None, "{id:?}");
        }
    }

    #[test]
    fn the_saved_choice_decides_when_no_flag_is_given() {
        let none: &[(&str, &str)] = &[];
        assert!(resolve_with(prefs(true), args(&[]), env(none)).frameless);
        assert!(!resolve_with(prefs(false), args(&[]), env(none)).frameless);
        assert!(resolve_with(prefs(true), args(&[]), env(none)).available);
    }

    #[test]
    fn the_flag_and_the_variable_keep_the_system_bar_for_one_launch() {
        let none: &[(&str, &str)] = &[];
        let on = [(SYSTEM_TITLEBAR_ENV, "1")];
        assert!(!resolve_with(prefs(true), args(&["--system-titlebar"]), env(none)).frameless);
        assert!(
            !resolve_with(
                prefs(true),
                args(&["x", SYSTEM_TITLEBAR_FLAG, "y"]),
                env(none)
            )
            .frameless
        );
        assert!(!resolve_with(prefs(true), args(&[]), env(&on)).frameless);
        for value in ["1", "true", "yes", "on", "anything"] {
            let pairs = [(SYSTEM_TITLEBAR_ENV, value)];
            assert!(
                !resolve_with(prefs(true), args(&[]), env(&pairs)).frameless,
                "{value}"
            );
        }
        for value in ["", "0", "false", "off", "no", " 0 "] {
            let pairs = [(SYSTEM_TITLEBAR_ENV, value)];
            assert!(
                resolve_with(prefs(true), args(&[]), env(&pairs)).frameless,
                "{value:?}"
            );
        }
    }

    #[test]
    fn the_flag_beats_a_false_variable_and_a_saved_system_bar_stays_system() {
        let off = [(SYSTEM_TITLEBAR_ENV, "0")];
        assert!(!resolve_with(prefs(true), args(&["--system-titlebar"]), env(&off)).frameless);
        let on = [(SYSTEM_TITLEBAR_ENV, "1")];
        assert!(!resolve_with(prefs(false), args(&["--system-titlebar"]), env(&on)).frameless);
        assert!(!resolve_with(prefs(false), args(&[]), env(&off)).frameless);
        assert!(resolve_with(prefs(true), args(&["--system-titlebar=1"]), env(&off)).frameless);
    }

    #[test]
    fn a_one_launch_override_never_overwrites_the_saved_choice() {
        let _guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        let id = "no/file/location";
        prefs(true).save(id).expect("memory save");
        let none: &[(&str, &str)] = &[];
        let chrome = resolve(id, args(&["--system-titlebar"]), env(none));
        assert!(!chrome.frameless);
        assert!(
            ChromePrefs::load(id).custom_title_bar,
            "the flag changed the saved choice"
        );
        assert!(resolve(id, args(&[]), env(none)).frameless);
    }

    #[test]
    fn the_viewport_drops_decorations_and_gets_a_minimum_size() {
        let framed = Chrome {
            frameless: false,
            available: true,
        };
        let frameless = Chrome {
            frameless: true,
            available: true,
        };
        let builder = viewport(ViewportBuilder::default(), &frameless);
        assert_eq!(builder.decorations, Some(false));
        assert_eq!(builder.min_inner_size, Some(MIN_INNER_SIZE));
        let builder = viewport(ViewportBuilder::default(), &framed);
        assert_eq!(builder.decorations, Some(true));
        assert_eq!(builder.min_inner_size, Some(MIN_INNER_SIZE));
        let own = vec2(500.0, 400.0);
        let builder = viewport(
            ViewportBuilder::default().with_min_inner_size(own),
            &frameless,
        );
        assert_eq!(builder.min_inner_size, Some(own));
    }

    #[test]
    fn an_unavailable_chrome_keeps_the_system_frame_and_ignores_requests() {
        let _guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut chrome = Chrome::UNAVAILABLE;
        let builder = viewport(
            ViewportBuilder::default(),
            &Chrome {
                frameless: true,
                available: false,
            },
        );
        assert_eq!(builder.decorations, Some(true));
        assert_eq!(builder.min_inner_size, None);
        let ctx = headless();
        let mut commands = Vec::new();
        let mut response_size = None;
        commands.extend(frame(&ctx, 0.1, vec![], |ui| {
            let response = ui
                .vertical(|ui| toggle_row(ui, &mut chrome, "Custom title bar"))
                .inner;
            response_size = Some(response.rect.size());
        }));
        assert_eq!(response_size, Some(Vec2::ZERO));
        assert!(set_frameless(&ctx, &mut chrome, true).is_ok());
        commands.extend(frame(&ctx, 0.2, vec![], |_| {}));
        let decorations = commands
            .iter()
            .filter(|c| matches!(c, ViewportCommand::Decorations(_)))
            .count();
        assert_eq!(decorations, 0, "{commands:?}");
        assert!(!chrome.frameless);
    }

    #[test]
    fn set_frameless_sends_the_decorations_command_and_remembers_the_choice() {
        let _guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        let id = "no/file/set";
        let none: &[(&str, &str)] = &[];
        let mut chrome = resolve(id, args(&[]), env(none));
        assert!(chrome.frameless);
        let ctx = headless();
        set_frameless(&ctx, &mut chrome, false).expect("saved");
        assert!(!chrome.frameless);
        let sent = frame(&ctx, 0.1, vec![], |_| {});
        assert!(
            sent.contains(&ViewportCommand::Decorations(true)),
            "{sent:?}"
        );
        assert!(!ChromePrefs::load(id).custom_title_bar);
        assert!(!resolve(id, args(&[]), env(none)).frameless);
        set_frameless(&ctx, &mut chrome, true).expect("saved");
        assert!(frame(&ctx, 0.2, vec![], |_| {}).contains(&ViewportCommand::Decorations(false)));
        assert!(ChromePrefs::load(id).custom_title_bar);
    }

    #[test]
    fn the_toggle_row_flips_the_window_and_the_saved_choice() {
        let _guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        let id = "no/file/toggle";
        let none: &[(&str, &str)] = &[];
        let mut chrome = resolve(id, args(&[]), env(none));
        let ctx = headless();
        let run = |chrome: &mut Chrome, time: f64, events: Vec<Event>| {
            let mut rect = Rect::NOTHING;
            let sent = frame(&ctx, time, events, |ui| {
                rect = toggle_row(ui, chrome, "Custom title bar").rect;
            });
            (sent, rect)
        };
        run(&mut chrome, 0.1, vec![]);
        let (_, rect) = run(&mut chrome, 0.2, vec![]);
        let at = rect.center();
        run(&mut chrome, 0.3, vec![Event::PointerMoved(at)]);
        run(&mut chrome, 0.4, vec![press(at, true)]);
        let (sent, _) = run(&mut chrome, 0.5, vec![press(at, false)]);
        assert!(
            sent.contains(&ViewportCommand::Decorations(true)),
            "{sent:?}"
        );
        assert!(!chrome.frameless);
        assert!(!ChromePrefs::load(id).custom_title_bar);
        assert_eq!(take_save_error(&ctx), None);
    }

    #[test]
    fn a_stored_save_error_is_handed_out_once() {
        let ctx = headless();
        assert_eq!(take_save_error(&ctx), None);
        ctx.data_mut(|d| d.insert_temp(save_error_id(), "disk full".to_owned()));
        assert_eq!(take_save_error(&ctx).as_deref(), Some("disk full"));
        assert_eq!(take_save_error(&ctx), None);
    }
}
