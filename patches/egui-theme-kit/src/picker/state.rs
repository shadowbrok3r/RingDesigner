//! Picker state and the actions the picker's buttons run, usable without a UI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::Context;

use super::edit::{self, Role, Semantic, Surface};
use super::io::{self, DialogResult};
use crate::{
    fonts::TextScale,
    glass::{self, GlassParams},
    metrics::Density,
    presets::{self, PresetRef, themes_equal},
    saved::{SavedTheme, SemanticColors, theme_from_json},
    store::{AutoSave, Fingerprint, NamedTheme, StartupTheme, ThemeStore, clean_name},
    tokens::Tokens,
};

/// What the live theme was derived from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A preset, by id.
    Preset(&'static str),
    /// A saved user theme, by name.
    User(String),
    /// A theme imported from JSON.
    Imported,
    /// A theme with no known source.
    Custom,
}

/// Outcome tone of the last action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Success,
    Error,
}

/// The last action's outcome, shown under the header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    pub text: String,
}

/// How the picker is dressed and which sections it shows.
#[derive(Clone, Copy)]
pub struct PickerOptions {
    /// Maps an action name to a glyph drawn before its label; names are listed in [`super::ICON_NAMES`].
    pub icon: Option<fn(&str) -> Option<&'static str>>,
    /// Whether the Customize section is shown.
    pub customize: bool,
    /// Whether the import and export section is shown.
    pub import_export: bool,
    /// How long the live theme must be still before it is saved to the store.
    pub debounce: Duration,
}

impl Default for PickerOptions {
    fn default() -> Self {
        Self {
            icon: None,
            customize: true,
            import_export: true,
            debounce: Duration::from_millis(600),
        }
    }
}

/// The theme as last read from the context, with what is derived from it.
pub(super) struct Live {
    fingerprint: Fingerprint,
    store_revision: u64,
    pub(super) dirty: bool,
    pub(super) can_revert: bool,
    pub(super) applied_user: Option<String>,
}

/// A theme and where it came from, kept to return to.
#[derive(Clone)]
struct Anchor {
    theme: SavedTheme,
    origin: Origin,
}

/// State of the theme picker; keep one per picker and pass it to [`super::show`] or [`super::window`].
pub struct PickerState {
    /// Whether [`super::window`] draws the window.
    pub open: bool,
    pub options: PickerOptions,
    pub(super) search: String,
    pub(super) origin: Option<Origin>,
    pub(super) snapshot: Option<SavedTheme>,
    baseline: Option<Anchor>,
    pub(super) save_name: String,
    pub(super) rename: Option<(String, String)>,
    pub(super) rename_focus: bool,
    pub(super) confirm_delete: Option<String>,
    pub(super) message: Option<Message>,
    pub(super) json: String,
    pub(super) autosave: AutoSave,
    pub(super) inbox: Arc<Mutex<Vec<DialogResult>>>,
    pub(super) live: Option<Live>,
    pub(super) users: Vec<NamedTheme>,
    users_revision: Option<u64>,
    pub(super) user_tokens: HashMap<String, Tokens>,
    pub(super) card_data: HashMap<&'static str, (GlassParams, f32)>,
}

impl Default for PickerState {
    fn default() -> Self {
        let options = PickerOptions::default();
        Self {
            open: false,
            options,
            search: String::new(),
            origin: None,
            snapshot: None,
            baseline: None,
            save_name: String::new(),
            rename: None,
            rename_focus: false,
            confirm_delete: None,
            message: None,
            json: String::new(),
            autosave: AutoSave::new(options.debounce),
            inbox: Arc::default(),
            live: None,
            users: Vec::new(),
            users_revision: None,
            user_tokens: HashMap::new(),
            card_data: HashMap::new(),
        }
    }
}

impl PickerState {
    pub fn new(options: PickerOptions) -> Self {
        Self {
            options,
            autosave: AutoSave::new(options.debounce),
            ..Self::default()
        }
    }

    /// What the live theme was derived from; `None` before the first frame.
    pub fn origin(&self) -> Option<&Origin> {
        self.origin.as_ref()
    }

    /// Whether the live theme differs from the one it was derived from.
    pub fn is_modified(&self) -> bool {
        self.live.as_ref().is_some_and(|live| live.dirty)
    }

    /// The last action's outcome.
    pub fn message(&self) -> Option<&Message> {
        self.message.as_ref()
    }

    /// Whether the live theme differs from the one [`PickerState::revert`] returns to.
    pub fn can_revert(&self) -> bool {
        self.live.as_ref().is_some_and(|live| live.can_revert)
    }

    /// What [`PickerState::reset_to_origin`] returns to: "Reset to preset", "Reset to saved" or "Reset".
    pub fn reset_label(&self) -> &'static str {
        match self.origin {
            Some(Origin::Preset(_)) => "Reset to preset",
            Some(Origin::User(_)) => "Reset to saved",
            _ => "Reset",
        }
    }

    /// The saved user themes as of the last frame.
    pub fn user_themes(&self) -> &[NamedTheme] {
        &self.users
    }

    /// Saves a pending change to the store now.
    pub fn flush(&mut self, ctx: &Context, store: &dyn ThemeStore) {
        self.autosave.flush(ctx, store);
    }

    fn say(&mut self, kind: MessageKind, text: impl Into<String>) {
        self.message = Some(Message {
            kind,
            text: text.into(),
        });
    }

    /// Refreshes the cached lists and the modified marker; call once per frame before drawing.
    pub fn sync(&mut self, ctx: &Context, store: &dyn ThemeStore) {
        self.drain_inbox(ctx);
        let revision = store.revision();
        if self.users_revision != Some(revision) {
            self.users = store.user_themes();
            self.users_revision = Some(revision);
            self.refresh_user_tokens();
        }
        let fingerprint = Fingerprint::of(ctx);
        let fresh = self.live.as_ref().is_some_and(|live| {
            live.fingerprint.same_as(&fingerprint) && live.store_revision == revision
        });
        if !fresh {
            self.refresh_live(ctx, fingerprint, revision);
        }
        self.autosave.update(ctx, store);
    }

    fn refresh_user_tokens(&mut self) {
        self.user_tokens = self
            .users
            .iter()
            .map(|named| {
                (
                    named.name.clone(),
                    super::preview::theme_tokens(&named.theme),
                )
            })
            .collect();
    }

    fn refresh_live(&mut self, ctx: &Context, fingerprint: Fingerprint, store_revision: u64) {
        let theme = SavedTheme::current(ctx);
        if self.origin.is_none() {
            self.adopt_startup_origin(ctx, &theme);
        }
        let dirty = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !themes_equal(&theme, snapshot));
        let can_revert = self
            .baseline
            .as_ref()
            .is_some_and(|anchor| !themes_equal(&theme, &anchor.theme));
        let applied_user = self
            .users
            .iter()
            .find(|named| themes_equal(&named.theme, &theme))
            .map(|named| named.name.clone());
        self.live = Some(Live {
            fingerprint,
            store_revision,
            dirty,
            can_revert,
            applied_user,
        });
    }

    /// Sets the origin from the theme found on first sight: a matching user theme, its preset, or custom.
    fn adopt_startup_origin(&mut self, ctx: &Context, theme: &SavedTheme) {
        let saved = self
            .users
            .iter()
            .find(|named| themes_equal(&named.theme, theme));
        let origin = match (saved, presets::installed_preset_id(ctx)) {
            (Some(named), _) => Origin::User(named.name.clone()),
            (None, Some(id)) => Origin::Preset(id),
            (None, None) => Origin::Custom,
        };
        self.origin = Some(origin.clone());
        self.snapshot = Some(theme.clone());
        self.baseline = Some(Anchor {
            theme: theme.clone(),
            origin,
        });
        self.autosave.mark_clean(ctx);
    }

    /// The display name of the live theme: the saved theme it matches, its preset, or "Custom".
    pub fn current_name(&self, ctx: &Context) -> String {
        match (self.origin.as_ref(), self.is_modified()) {
            (Some(Origin::User(name)), false) => name.clone(),
            _ => presets::installed_def(ctx)
                .map_or_else(|| "Custom".to_owned(), |d| d.name.to_owned()),
        }
    }

    /// The name of what the live theme was derived from, for "based on" lines.
    pub fn origin_name(&self) -> Option<String> {
        match self.origin.as_ref()? {
            Origin::Preset(id) => presets::preset(id).map(|d| d.name.to_owned()),
            Origin::User(name) => Some(name.clone()),
            Origin::Imported | Origin::Custom => None,
        }
    }

    fn set_origin(&mut self, origin: Origin, ctx: &Context) {
        self.origin = Some(origin);
        self.snapshot = Some(SavedTheme::current(ctx));
        self.live = None;
    }

    /// Applies a preset by id, name or enum, keeping the density and text size.
    pub fn apply_preset(&mut self, ctx: &Context, preset: impl PresetRef) -> bool {
        let Some(def) = preset.resolve() else {
            self.say(MessageKind::Error, "That preset is not registered.");
            return false;
        };
        presets::apply_preset(ctx, def);
        self.set_origin(Origin::Preset(def.id), ctx);
        self.say(MessageKind::Info, format!("Applied {}.", def.name));
        true
    }

    /// Applies the saved user theme `name`.
    pub fn apply_user_theme(&mut self, ctx: &Context, store: &dyn ThemeStore, name: &str) -> bool {
        let found = store
            .user_themes()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name));
        let Some(named) = found else {
            self.say(
                MessageKind::Error,
                format!("There is no theme named \"{name}\"."),
            );
            return false;
        };
        presets::apply_saved_theme(ctx, named.theme);
        self.set_origin(Origin::User(named.name.clone()), ctx);
        self.say(MessageKind::Info, format!("Applied {}.", named.name));
        true
    }

    /// Returns to the theme the picker started with, or the last one saved from it.
    pub fn revert(&mut self, ctx: &Context) {
        let Some(anchor) = self.baseline.clone() else {
            return;
        };
        presets::apply_saved_theme(ctx, anchor.theme);
        self.set_origin(anchor.origin, ctx);
        self.say(MessageKind::Info, "Reverted.");
    }

    /// Drops edits made since the origin was applied; density and text size stay as they were then.
    pub fn reset_to_origin(&mut self, ctx: &Context) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let origin = self.origin.clone().unwrap_or(Origin::Custom);
        presets::apply_saved_theme(ctx, snapshot);
        self.set_origin(origin, ctx);
        self.say(MessageKind::Info, "Reset to the original theme.");
    }

    fn commit_anchor(&mut self, origin: Origin, ctx: &Context) {
        let theme = SavedTheme::current(ctx);
        self.origin = Some(origin.clone());
        self.snapshot = Some(theme.clone());
        self.baseline = Some(Anchor { theme, origin });
        self.live = None;
    }

    /// Saves the live theme under `name`; refuses to replace another theme unless `replace` is set.
    pub fn save_as(
        &mut self,
        ctx: &Context,
        store: &dyn ThemeStore,
        name: &str,
        replace: bool,
    ) -> bool {
        let name = match clean_name(name) {
            Ok(name) => name,
            Err(e) => {
                self.say(MessageKind::Error, e.to_string());
                return false;
            }
        };
        let existing = store
            .user_themes()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(&name));
        if existing.is_some() && !replace {
            self.say(
                MessageKind::Error,
                format!("A theme named \"{name}\" already exists."),
            );
            return false;
        }
        self.autosave.flush(ctx, store);
        store.save_user_theme(&name, &SavedTheme::current(ctx));
        let saved_as = existing.map_or(name, |t| t.name);
        self.commit_anchor(Origin::User(saved_as.clone()), ctx);
        self.say(MessageKind::Success, format!("Saved {saved_as}."));
        true
    }

    /// Saves the live theme over the user theme it came from.
    pub fn save_origin(&mut self, ctx: &Context, store: &dyn ThemeStore) -> bool {
        match self.origin.clone() {
            Some(Origin::User(name)) => self.save_as(ctx, store, &name, true),
            _ => {
                self.say(MessageKind::Error, "Use Save as to name this theme first.");
                false
            }
        }
    }

    pub fn rename(&mut self, store: &dyn ThemeStore, from: &str, to: &str) -> bool {
        let to = match clean_name(to) {
            Ok(to) => to,
            Err(e) => {
                self.say(MessageKind::Error, e.to_string());
                return false;
            }
        };
        store.rename_user_theme(from, &to);
        if let Some(error) = store.last_error() {
            store.clear_error();
            self.say(MessageKind::Error, error);
            return false;
        }
        if matches!(&self.origin, Some(Origin::User(n)) if n.eq_ignore_ascii_case(from)) {
            self.origin = Some(Origin::User(to.clone()));
        }
        self.say(MessageKind::Success, format!("Renamed {from} to {to}."));
        true
    }

    /// Saves a copy of the stored theme `name` as "name copy".
    pub fn duplicate(&mut self, store: &dyn ThemeStore, name: &str) -> Option<String> {
        let themes = store.user_themes();
        let source = themes.iter().find(|t| t.name.eq_ignore_ascii_case(name))?;
        let copy = (1..)
            .map(|n| match n {
                1 => format!("{} copy", source.name),
                n => format!("{} copy {n}", source.name),
            })
            .find(|candidate| {
                !themes
                    .iter()
                    .any(|t| t.name.eq_ignore_ascii_case(candidate))
            })?;
        store.save_user_theme(&copy, &source.theme);
        self.say(MessageKind::Success, format!("Duplicated as {copy}."));
        Some(copy)
    }

    pub fn delete(&mut self, store: &dyn ThemeStore, name: &str) {
        store.delete_user_theme(name);
        if matches!(&self.origin, Some(Origin::User(n)) if n.eq_ignore_ascii_case(name)) {
            self.origin = Some(Origin::Custom);
        }
        self.confirm_delete = None;
        self.say(MessageKind::Info, format!("Deleted {name}."));
    }

    /// Pins the startup theme, reporting a store failure.
    pub fn set_startup(&mut self, store: &dyn ThemeStore, startup: StartupTheme) {
        let label = match &startup {
            StartupTheme::LastUsed => "the last theme used".to_owned(),
            StartupTheme::User(name) => name.clone(),
            StartupTheme::Preset(id) => {
                presets::preset(id).map_or_else(|| id.clone(), |d| d.name.to_owned())
            }
        };
        store.set_startup(startup);
        match store.last_error() {
            Some(error) => {
                store.clear_error();
                self.say(MessageKind::Error, error);
            }
            None => self.say(
                MessageKind::Success,
                format!("The app now starts with {label}."),
            ),
        }
    }

    /// Applies theme JSON in either the wrapper or the bare `egui::Style` shape.
    pub fn import_json(&mut self, ctx: &Context, text: &str) -> bool {
        match theme_from_json(text.as_bytes()) {
            Ok(theme) => {
                presets::apply_saved_theme(ctx, theme);
                let origin = match presets::installed_preset_id(ctx) {
                    Some(id) => Origin::Preset(id),
                    None => Origin::Imported,
                };
                self.set_origin(origin, ctx);
                self.say(MessageKind::Success, "Imported the theme.");
                true
            }
            Err(e) => {
                self.say(MessageKind::Error, format!("{e:#}"));
                false
            }
        }
    }

    /// The live theme as indented JSON.
    pub fn export_json(&self, ctx: &Context) -> String {
        io::pretty_json(&SavedTheme::current(ctx))
    }

    pub fn set_density(&mut self, ctx: &Context, density: Density) {
        presets::set_density(ctx, density);
    }

    pub fn set_text_scale(&mut self, ctx: &Context, scale: TextScale) {
        presets::set_text_scale(ctx, scale);
    }

    pub fn set_glass(&mut self, ctx: &Context, params: GlassParams) {
        glass::set_params(ctx, params);
    }

    /// Re-styles the live theme as tinted glass from its own colours.
    pub fn glassify(&mut self, ctx: &Context) {
        edit::glassify_live(ctx);
    }

    pub fn set_surface(&mut self, ctx: &Context, surface: Surface, color: eframe::egui::Color32) {
        edit::edit_style(ctx, |style| surface.apply(style, color));
    }

    pub fn set_role(&mut self, ctx: &Context, role: Role, color: eframe::egui::Color32) {
        edit::edit_style(ctx, |style| role.apply(style, color));
    }

    pub fn set_semantic(&mut self, ctx: &Context, which: Semantic, color: eframe::egui::Color32) {
        edit::edit_semantic(ctx, |colors: &mut SemanticColors| {
            which.apply(colors, color)
        });
    }

    fn drain_inbox(&mut self, ctx: &Context) {
        let results = self
            .inbox
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for result in results {
            match result {
                DialogResult::Imported(Ok(theme), path) => {
                    presets::apply_saved_theme(ctx, *theme);
                    self.set_origin(Origin::Imported, ctx);
                    self.say(
                        MessageKind::Success,
                        format!("Imported {}.", path.display()),
                    );
                }
                DialogResult::Imported(Err(e), path) => {
                    self.say(MessageKind::Error, format!("{}: {e}", path.display()));
                }
                DialogResult::Exported(Ok(path)) => {
                    self.say(
                        MessageKind::Success,
                        format!("Exported to {}.", path.display()),
                    );
                }
                DialogResult::Exported(Err(e)) => self.say(MessageKind::Error, e),
            }
        }
    }
}
