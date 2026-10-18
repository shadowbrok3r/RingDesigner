//! Startup theme resolution and persistence in eframe storage.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use eframe::egui::{Context, Id};

use super::{
    presets::{self, themes_equal},
    saved::{SavedTheme, theme_from_json, theme_to_json},
    store::{StartupTheme, ThemeStore},
    theme_chrome::graphite_theme,
};

/// Storage key holding the user's saved theme JSON.
pub const STORAGE_KEY: &str = "egui_theme_kit.theme.v1";

static DEFAULT_THEME: RwLock<Option<SavedTheme>> = RwLock::new(None);

/// Replaces the theme applied when nothing is saved; call before bootstrapping.
pub fn set_default_theme(theme: SavedTheme) {
    if let Ok(mut slot) = DEFAULT_THEME.write() {
        *slot = Some(theme);
    }
}

/// Restores the Graphite theme as the default.
pub fn reset_default_theme() {
    if let Ok(mut slot) = DEFAULT_THEME.write() {
        *slot = None;
    }
}

/// The theme applied when nothing is saved: the app's default, else Graphite.
pub fn default_theme() -> SavedTheme {
    DEFAULT_THEME
        .read()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_else(graphite_theme)
}

/// Where the theme applied at startup came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    /// The default theme: the app's, else Graphite.
    Embedded,
    /// The theme saved by an earlier run.
    Stored,
    /// A file named by `--theme`; never written back to storage.
    File(PathBuf),
}

impl ThemeSource {
    pub fn label(&self) -> String {
        match self {
            Self::Embedded => "embedded default".to_owned(),
            Self::Stored => "saved by an earlier run".to_owned(),
            Self::File(path) => format!("file {}", path.display()),
        }
    }
}

fn source_id() -> Id {
    Id::unique("egui_theme_kit.theme.source")
}

/// Records where the startup theme came from.
pub fn set_source(ctx: &Context, source: ThemeSource) {
    ctx.data_mut(|d| d.insert_temp(source_id(), source));
}

/// Where the startup theme came from; embedded when none was recorded.
pub fn source(ctx: &Context) -> ThemeSource {
    ctx.data(|d| d.get_temp::<ThemeSource>(source_id()))
        .unwrap_or(ThemeSource::Embedded)
}

/// Reads a theme file in either the wrapper or bare `egui::Style` shape.
pub fn load_file(path: &Path) -> anyhow::Result<SavedTheme> {
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("cannot read theme file {}: {e}", path.display()))?;
    theme_from_json(&bytes)
}

/// The theme saved in `storage`, if any; a stored value that cannot be decoded is logged and ignored.
pub fn load_stored(storage: Option<&dyn eframe::Storage>) -> Option<SavedTheme> {
    load_stored_from(storage, &[STORAGE_KEY])
}

/// The theme under the first of `keys` that holds a decodable value; use to read keys an earlier release wrote.
pub fn load_stored_from(
    storage: Option<&dyn eframe::Storage>,
    keys: &[&str],
) -> Option<SavedTheme> {
    let storage = storage?;
    keys.iter().find_map(|key| {
        let json = storage.get_string(key).filter(|s| !s.trim().is_empty())?;
        match theme_from_json(json.as_bytes()) {
            Ok(theme) => Some(theme),
            Err(e) => {
                log::warn!("stored theme under {key} could not be read ({e}); ignoring it");
                None
            }
        }
    })
}

/// The theme to apply at startup: the `--theme` file, else the stored theme, else the default.
pub fn resolve(
    storage: Option<&dyn eframe::Storage>,
    file: Option<&Path>,
) -> (SavedTheme, ThemeSource) {
    resolve_from(file, || load_stored(storage))
}

/// The theme to apply at startup from a [`ThemeStore`]: the `--theme` file, the pinned startup theme, the active theme, else the default.
pub fn resolve_with_store(
    store: &dyn ThemeStore,
    file: Option<&Path>,
) -> (SavedTheme, ThemeSource) {
    resolve_from(file, || {
        startup_theme(store).or_else(|| store.load_active())
    })
}

fn resolve_from(
    file: Option<&Path>,
    stored: impl FnOnce() -> Option<SavedTheme>,
) -> (SavedTheme, ThemeSource) {
    if let Some(path) = file {
        match load_file(path) {
            Ok(theme) => return (theme, ThemeSource::File(path.to_path_buf())),
            Err(e) => log::warn!(
                "--theme {}: {e}; using the saved or default theme",
                path.display()
            ),
        }
    }
    match stored() {
        Some(theme) => (theme, ThemeSource::Stored),
        None => (default_theme(), ThemeSource::Embedded),
    }
}

/// The theme the store pins for startup, if it is set and still exists.
fn startup_theme(store: &dyn ThemeStore) -> Option<SavedTheme> {
    match store.startup() {
        StartupTheme::LastUsed => None,
        StartupTheme::User(name) => store
            .user_themes()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(&name))
            .map(|t| t.theme),
        StartupTheme::Preset(id) => {
            let mut theme = presets::find_preset(&id)?.theme();
            if let Some(active) = store.load_active() {
                theme.density = active.density;
                theme.text_scale = active.text_scale;
            }
            Some(theme)
        }
    }
}

/// Applies the startup theme from `store` (or the `--theme` file) to `ctx` and records where it came from.
pub fn apply_startup_from_store(
    ctx: &Context,
    store: &dyn ThemeStore,
    file: Option<&Path>,
) -> ThemeSource {
    let (theme, source) = resolve_with_store(store, file);
    set_source(ctx, source.clone());
    presets::apply_saved_theme(ctx, theme);
    source
}

/// Whether `theme` equals the default theme in every part.
pub fn is_embedded_default(theme: &SavedTheme) -> bool {
    themes_equal(theme, &default_theme())
}

/// Writes the live theme to `storage`, or removes the entry when it equals the default theme.
pub fn save(ctx: &Context, storage: &mut dyn eframe::Storage) {
    if matches!(source(ctx), ThemeSource::File(_)) {
        return;
    }
    let current = SavedTheme::current(ctx);
    if is_embedded_default(&current) {
        storage.remove_string(STORAGE_KEY);
        return;
    }
    match theme_to_json(&current) {
        Ok(json) => storage.set_string(STORAGE_KEY, String::from_utf8_lossy(&json).into_owned()),
        Err(e) => log::warn!("theme was not saved: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        metrics::Density,
        presets::{PresetStyles, apply_preset, apply_saved_theme, set_density},
        store::{MemoryStore, test_dir},
    };
    use super::*;
    use eframe::Storage as _;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemStorage(HashMap<String, String>);

    impl eframe::Storage for MemStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn nothing_stored_resolves_to_the_embedded_default() {
        let storage = MemStorage::default();
        let (theme, source) = resolve(Some(&storage), None);
        assert_eq!(source, ThemeSource::Embedded);
        assert!(is_embedded_default(&theme));
        let (_, source) = resolve(None, None);
        assert_eq!(source, ThemeSource::Embedded);
    }

    #[test]
    fn an_untouched_theme_clears_the_entry_instead_of_pinning_the_default() {
        let ctx = Context::default();
        apply_saved_theme(&ctx, graphite_theme());
        let mut storage = MemStorage::default();
        storage.set_string(STORAGE_KEY, "stale".to_owned());
        save(&ctx, &mut storage);
        assert_eq!(storage.get_string(STORAGE_KEY), None);
        assert_eq!(resolve(Some(&storage), None).1, ThemeSource::Embedded);
    }

    #[test]
    fn a_changed_theme_round_trips_through_storage() {
        let ctx = Context::default();
        apply_saved_theme(&ctx, graphite_theme());
        apply_preset(&ctx, PresetStyles::NoirGlass);
        set_density(&ctx, Density::Comfortable);
        let mut storage = MemStorage::default();
        save(&ctx, &mut storage);

        let (theme, source) = resolve(Some(&storage), None);
        assert_eq!(source, ThemeSource::Stored);
        assert_eq!(theme.density, Density::Comfortable);

        let fresh = Context::default();
        apply_saved_theme(&fresh, theme);
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::NoirGlass);
    }

    #[test]
    fn a_corrupt_entry_falls_back_to_the_embedded_default() {
        let mut storage = MemStorage::default();
        storage.set_string(STORAGE_KEY, "{ definitely not json".to_owned());
        let (theme, source) = resolve(Some(&storage), None);
        assert_eq!(source, ThemeSource::Embedded);
        assert!(is_embedded_default(&theme));
    }

    #[test]
    fn an_earlier_release_key_is_read_when_the_current_one_is_empty() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Quartz);
        let json = theme_to_json(&SavedTheme::current(&ctx)).unwrap();
        let mut storage = MemStorage::default();
        storage.set_string("earlier.key", String::from_utf8(json).unwrap());
        assert!(load_stored(Some(&storage)).is_none());
        let theme = load_stored_from(Some(&storage), &[STORAGE_KEY, "earlier.key"]).unwrap();
        assert!(presets::exact_preset(&presets::normalized(&theme.style)).is_some());
    }

    #[test]
    fn startup_from_a_store_applies_the_theme_and_records_its_source() {
        let store = MemoryStore::default();
        let ctx = Context::default();
        assert_eq!(
            apply_startup_from_store(&ctx, &store, None),
            ThemeSource::Embedded
        );
        assert_eq!(presets::installed_preset(&ctx), PresetStyles::Graphite);

        let other = Context::default();
        apply_preset(&other, PresetStyles::Supernova);
        store.save_active(&SavedTheme::current(&other));
        let fresh = Context::default();
        assert_eq!(
            apply_startup_from_store(&fresh, &store, None),
            ThemeSource::Stored
        );
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::Supernova);
        assert_eq!(source(&fresh), ThemeSource::Stored);
    }

    #[test]
    fn a_theme_file_wins_and_is_never_written_back() {
        let dir = test_dir("persist_file");
        let path = dir.path().join("noir.json");
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Noir);
        std::fs::write(&path, theme_to_json(&SavedTheme::current(&ctx)).unwrap()).unwrap();

        let (theme, source) = resolve(None, Some(&path));
        assert_eq!(source, ThemeSource::File(path.clone()));
        let fresh = Context::default();
        apply_saved_theme(&fresh, theme);
        set_source(&fresh, source);
        let mut storage = MemStorage::default();
        save(&fresh, &mut storage);
        assert!(storage.get_string(STORAGE_KEY).is_none());

        let missing = dir.path().join("missing.json");
        assert_eq!(resolve(None, Some(&missing)).1, ThemeSource::Embedded);
    }

    #[test]
    fn the_store_resolves_file_then_startup_then_active_then_default() {
        let store = MemoryStore::default();
        assert_eq!(resolve_with_store(&store, None).1, ThemeSource::Embedded);

        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Aurora);
        store.save_active(&SavedTheme::current(&ctx));
        let (theme, source) = resolve_with_store(&store, None);
        assert_eq!(source, ThemeSource::Stored);
        let fresh = Context::default();
        apply_saved_theme(&fresh, theme);
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::Aurora);

        apply_preset(&ctx, PresetStyles::Velvet);
        store.save_user_theme("Mine", &SavedTheme::current(&ctx));
        store.set_startup(StartupTheme::User("mine".to_owned()));
        let (theme, _) = resolve_with_store(&store, None);
        apply_saved_theme(&fresh, theme);
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::Velvet);

        store.set_startup(StartupTheme::Preset("galactic".to_owned()));
        let (theme, _) = resolve_with_store(&store, None);
        apply_saved_theme(&fresh, theme);
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::Galactic);

        store.set_startup(StartupTheme::User("Mine".to_owned()));
        store.delete_user_theme("Mine");
        let (theme, _) = resolve_with_store(&store, None);
        apply_saved_theme(&fresh, theme);
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::Aurora);
    }
}
