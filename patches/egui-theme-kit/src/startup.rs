//! One-call setup: command-line options, fonts, glass and the startup theme, configured by [`KitConfig`].

use std::fmt;
use std::sync::Arc;

use eframe::egui::{Context, Plugin, Theme};

use super::{
    cli, debug_panel,
    fonts::{self, FontConfig},
    glass,
    persist::{self, ThemeSource},
    presets::{self, MATCHABLE_PRESETS, apply_preset, apply_saved_theme},
    saved::SavedTheme,
};

pub use store_stub::ThemeStore;

/// Settings handed to [`init`] and [`init_context`].
#[derive(Clone)]
pub struct KitConfig {
    /// Extra faces, monospace order and installer hooks for the font stack.
    pub fonts: FontConfig,
    /// Preset applied when nothing else supplies a theme, matched by name ignoring case.
    pub default_preset: Option<String>,
    /// Theme applied when nothing else supplies one; wins over `default_preset`.
    pub default_theme: Option<SavedTheme>,
    /// Persistent theme store read at startup.
    pub store: Option<Arc<dyn ThemeStore>>,
    /// Reserved; ignored. Temp-data ids use the `egui_theme_kit.` prefix.
    pub id_prefix: Option<String>,
    /// Reads `--theme`, `--theme-dump`, `--theme-panel` and `--no-frost` from the process arguments when [`cli::install`] was not called.
    pub parse_cli: bool,
}

impl Default for KitConfig {
    fn default() -> Self {
        Self {
            fonts: FontConfig::default(),
            default_preset: None,
            default_theme: None,
            store: None,
            id_prefix: None,
            parse_cli: true,
        }
    }
}

impl fmt::Debug for KitConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KitConfig")
            .field("fonts", &self.fonts)
            .field("default_preset", &self.default_preset)
            .field("default_theme", &self.default_theme.is_some())
            .field("store", &self.store.is_some())
            .field("parse_cli", &self.parse_cli)
            .finish_non_exhaustive()
    }
}

impl KitConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fonts(mut self, fonts: FontConfig) -> Self {
        self.fonts = fonts;
        self
    }

    pub fn default_preset(mut self, name: impl Into<String>) -> Self {
        self.default_preset = Some(name.into());
        self
    }

    pub fn default_theme(mut self, theme: SavedTheme) -> Self {
        self.default_theme = Some(theme);
        self
    }

    pub fn store(mut self, store: Arc<dyn ThemeStore>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn parse_cli(mut self, parse: bool) -> Self {
        self.parse_cli = parse;
        self
    }
}

/// Installs the glass backend, fonts and the startup theme; the source order is in the crate docs.
pub fn init(cc: &eframe::CreationContext<'_>, cfg: KitConfig) {
    if cfg.parse_cli {
        cli::ensure_installed();
    }
    glass::install(cc);
    init_context(&cc.egui_ctx, cc.storage, cfg);
}

/// [`init`] without the GL backend: command-line options, fonts and the startup theme on `ctx`.
pub fn init_context(ctx: &Context, storage: Option<&dyn eframe::Storage>, cfg: KitConfig) {
    if cfg.parse_cli {
        cli::ensure_installed();
    }
    ctx.set_theme(Theme::Dark);
    let default = configured_default(&cfg);
    set_state(
        ctx,
        KitState {
            default_theme: default.clone(),
            store: cfg.store.clone(),
        },
    );
    fonts::set_config(ctx, cfg.fonts);
    fonts::install_for_context(ctx);

    let options = cli::options();
    let (theme, source) = startup_theme(
        storage,
        options.theme_path.as_deref(),
        cfg.store.as_deref(),
        default.as_ref(),
    );
    log::info!("theme: applying {}", source.label());
    persist::set_source(ctx, source);
    apply_saved_theme(ctx, theme);
    if options.theme_panel {
        debug_panel::set_open(ctx, true);
    }
}

/// The store registered by [`init`] or [`init_context`].
pub fn store(ctx: &Context) -> Option<Arc<dyn ThemeStore>> {
    ctx.with_plugin::<KitState, _>(|state| state.store.clone())
        .flatten()
}

/// The default theme resolved from the config at init.
pub fn default_theme(ctx: &Context) -> Option<SavedTheme> {
    ctx.with_plugin::<KitState, _>(|state| state.default_theme.clone())
        .flatten()
}

/// Whether `theme` equals the configured default in every part; false when none was configured.
pub fn is_configured_default(ctx: &Context, theme: &SavedTheme) -> bool {
    default_theme(ctx).is_some_and(|default| same_theme(&default, theme))
}

/// Plugin holding the resolved config so a memory reset leaves it.
struct KitState {
    default_theme: Option<SavedTheme>,
    store: Option<Arc<dyn ThemeStore>>,
}

impl Plugin for KitState {
    fn debug_name(&self) -> &'static str {
        "egui_theme_kit::startup::KitState"
    }
}

fn set_state(ctx: &Context, state: KitState) {
    let KitState {
        default_theme,
        store,
    } = state;
    let replaced = ctx.with_plugin::<KitState, _>(|slot| {
        slot.default_theme = default_theme.clone();
        slot.store = store.clone();
    });
    if replaced.is_none() {
        ctx.add_plugin(KitState {
            default_theme,
            store,
        });
    }
}

/// `default_theme`, else the theme of `default_preset`, else none.
fn configured_default(cfg: &KitConfig) -> Option<SavedTheme> {
    if let Some(theme) = &cfg.default_theme {
        return Some(theme.clone());
    }
    let name = cfg.default_preset.as_deref()?;
    let Some(preset) = MATCHABLE_PRESETS
        .into_iter()
        .find(|p| p.as_str().eq_ignore_ascii_case(name.trim()))
    else {
        log::warn!("default preset {name:?} is unknown; using the shipped default");
        return None;
    };
    let scratch = Context::default();
    apply_preset(&scratch, preset);
    Some(SavedTheme::current(&scratch))
}

/// The first available startup theme: file, store, eframe storage, configured default, shipped default.
fn startup_theme(
    storage: Option<&dyn eframe::Storage>,
    file: Option<&std::path::Path>,
    store: Option<&dyn ThemeStore>,
    default: Option<&SavedTheme>,
) -> (SavedTheme, ThemeSource) {
    if let Some(path) = file {
        match persist::load_file(path) {
            Ok(theme) => return (theme, ThemeSource::File(path.to_path_buf())),
            Err(e) => log::warn!(
                "--theme {}: {e}; using another theme source",
                path.display()
            ),
        }
    }
    if let Some(theme) = store.and_then(ThemeStore::load_active) {
        return (theme, ThemeSource::Stored);
    }
    if let Some(theme) = persist::load_stored(storage) {
        return (theme, ThemeSource::Stored);
    }
    match default {
        Some(theme) => (theme.clone(), ThemeSource::Embedded),
        None => persist::resolve(None, None),
    }
}

/// Whether two themes agree in style, glass, density and text scale.
fn same_theme(a: &SavedTheme, b: &SavedTheme) -> bool {
    a.density == b.density
        && a.text_scale == b.text_scale
        && a.glass == b.glass
        && presets::styles_visually_equal(
            &presets::normalized(&a.style),
            &presets::normalized(&b.style),
        )
}

/// Stand-in for the store trait until the persistence module replaces it.
pub mod store_stub {
    use super::SavedTheme;

    /// Where the active theme persists between runs.
    pub trait ThemeStore: Send + Sync {
        /// The active theme, or none when nothing was saved.
        fn load_active(&self) -> Option<SavedTheme>;
        /// Records `theme` as the active theme.
        fn save_active(&self, theme: &SavedTheme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage as _;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::super::{metrics::Density, presets::PresetStyles, presets::installed_preset};

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

    struct FakeStore(Mutex<Option<SavedTheme>>);

    impl ThemeStore for FakeStore {
        fn load_active(&self) -> Option<SavedTheme> {
            self.0.lock().unwrap().clone()
        }

        fn save_active(&self, theme: &SavedTheme) {
            *self.0.lock().unwrap() = Some(theme.clone());
        }
    }

    fn preset_theme(preset: PresetStyles, density: Density) -> SavedTheme {
        let ctx = Context::default();
        apply_preset(&ctx, preset);
        presets::set_density(&ctx, density);
        SavedTheme::current(&ctx)
    }

    fn others() -> [PresetStyles; 4] {
        let mut presets = MATCHABLE_PRESETS
            .into_iter()
            .filter(|p| *p != presets::TOP_PRESET);
        std::array::from_fn(|_| presets.next().expect("at least four non-top presets"))
    }

    fn temp_theme_file(tag: &str, theme: &SavedTheme) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("egui_theme_kit_init_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("theme.json");
        std::fs::write(&path, super::super::saved::theme_to_json(theme).unwrap()).unwrap();
        path
    }

    fn preset_of(theme: SavedTheme) -> PresetStyles {
        let ctx = Context::default();
        apply_saved_theme(&ctx, theme);
        installed_preset(&ctx)
    }

    #[test]
    fn the_startup_order_is_file_store_storage_default_then_shipped() {
        let [file_preset, store_preset, stored_preset, default_preset] = others();
        let file_theme = preset_theme(file_preset, Density::Regular);
        let store_theme = preset_theme(store_preset, Density::Regular);
        let stored_theme = preset_theme(stored_preset, Density::Regular);
        let default_theme = preset_theme(default_preset, Density::Regular);

        let path = temp_theme_file("order", &file_theme);
        let mut storage = MemStorage::default();
        let ctx = Context::default();
        apply_saved_theme(&ctx, stored_theme);
        persist::save(&ctx, &mut storage);
        let store = FakeStore(Mutex::new(Some(store_theme)));

        let pick = |file: Option<&std::path::Path>,
                    store: Option<&dyn ThemeStore>,
                    storage: Option<&dyn eframe::Storage>,
                    default: Option<&SavedTheme>| {
            let (theme, source) = startup_theme(storage, file, store, default);
            (preset_of(theme), source)
        };

        let (preset, source) = pick(
            Some(&path),
            Some(&store),
            Some(&storage),
            Some(&default_theme),
        );
        assert_eq!(
            (preset, source),
            (file_preset, ThemeSource::File(path.clone()))
        );

        let (preset, source) = pick(None, Some(&store), Some(&storage), Some(&default_theme));
        assert_eq!((preset, source), (store_preset, ThemeSource::Stored));

        let (preset, source) = pick(None, None, Some(&storage), Some(&default_theme));
        assert_eq!((preset, source), (stored_preset, ThemeSource::Stored));

        let (preset, source) = pick(None, None, None, Some(&default_theme));
        assert_eq!((preset, source), (default_preset, ThemeSource::Embedded));

        let (_, source) = pick(None, None, None, None);
        assert_eq!(source, ThemeSource::Embedded);

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn a_missing_file_and_an_empty_store_fall_through() {
        let [_, store_preset, ..] = others();
        let missing = PathBuf::from("/nonexistent/egui_theme_kit/theme.json");
        let default = preset_theme(store_preset, Density::Regular);
        let empty = FakeStore(Mutex::new(None));
        let (theme, source) = startup_theme(None, Some(&missing), Some(&empty), Some(&default));
        assert_eq!(source, ThemeSource::Embedded);
        assert_eq!(preset_of(theme), store_preset);
    }

    #[test]
    fn init_applies_the_configured_default_and_registers_the_config() {
        let [preset, ..] = others();
        let ctx = Context::default();
        let cfg = KitConfig::new()
            .parse_cli(false)
            .default_preset(preset.as_str().to_uppercase())
            .fonts(FontConfig::new().face("probe", &[0, 1, 2, 3]));
        init_context(&ctx, None, cfg);

        assert_eq!(ctx.theme(), Theme::Dark);
        assert_eq!(installed_preset(&ctx), preset);
        assert_eq!(persist::source(&ctx), ThemeSource::Embedded);
        assert_eq!(fonts::config(&ctx).faces.len(), 1);
        assert!(default_theme(&ctx).is_some());
        assert!(is_configured_default(&ctx, &SavedTheme::current(&ctx)));

        ctx.memory_mut(|mem| *mem = Default::default());
        assert!(
            default_theme(&ctx).is_some(),
            "the config survives a memory reset"
        );
    }

    #[test]
    fn an_explicit_default_theme_beats_the_default_preset() {
        let [first, second, ..] = others();
        let ctx = Context::default();
        let cfg = KitConfig::new()
            .parse_cli(false)
            .default_preset(first.as_str())
            .default_theme(preset_theme(second, Density::Comfortable));
        init_context(&ctx, None, cfg);
        assert_eq!(installed_preset(&ctx), second);
        assert_eq!(metrics_density(&ctx), Density::Comfortable);
    }

    fn metrics_density(ctx: &Context) -> Density {
        super::super::metrics::density(ctx)
    }

    #[test]
    fn an_unknown_default_preset_falls_back_to_the_shipped_default() {
        let ctx = Context::default();
        init_context(
            &ctx,
            None,
            KitConfig::new()
                .parse_cli(false)
                .default_preset("no such preset"),
        );
        assert!(default_theme(&ctx).is_none());
        assert_eq!(persist::source(&ctx), ThemeSource::Embedded);
    }

    #[test]
    fn the_store_theme_is_applied_and_reachable_from_the_context() {
        let [preset, ..] = others();
        let store = Arc::new(FakeStore(Mutex::new(Some(preset_theme(
            preset,
            Density::Regular,
        )))));
        let ctx = Context::default();
        init_context(
            &ctx,
            None,
            KitConfig::new().parse_cli(false).store(store.clone()),
        );
        assert_eq!(installed_preset(&ctx), preset);
        assert_eq!(persist::source(&ctx), ThemeSource::Stored);
        let registered = super::store(&ctx).expect("the store is registered");
        assert!(registered.load_active().is_some());
    }

    #[test]
    fn same_theme_tells_density_and_style_apart() {
        let [a, b, ..] = others();
        let base = preset_theme(a, Density::Regular);
        assert!(same_theme(&base, &preset_theme(a, Density::Regular)));
        assert!(!same_theme(&base, &preset_theme(a, Density::Compact)));
        assert!(!same_theme(&base, &preset_theme(b, Density::Regular)));
    }

    #[test]
    fn saved_storage_is_cleared_when_the_theme_is_the_configured_default() {
        let [preset, ..] = others();
        let ctx = Context::default();
        init_context(
            &ctx,
            None,
            KitConfig::new()
                .parse_cli(false)
                .default_preset(preset.as_str()),
        );
        let mut storage = MemStorage::default();
        storage.set_string(persist::STORAGE_KEY, "stale".to_owned());
        super::super::save(&ctx, &mut storage);
        assert_eq!(storage.get_string(persist::STORAGE_KEY), None);

        apply_preset(&ctx, others()[3]);
        super::super::save(&ctx, &mut storage);
        assert!(storage.get_string(persist::STORAGE_KEY).is_some());
    }
}
