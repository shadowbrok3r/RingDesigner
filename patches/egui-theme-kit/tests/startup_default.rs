//! The app-wide default theme is process state, so it is exercised alone in this test binary.

use eframe::egui::Context;
use egui_theme_kit::{
    metrics::Density,
    persist::{self, ThemeSource},
    presets::{self, PresetStyles},
    store::{MemoryStore, ThemeStore},
};

#[test]
fn an_app_default_theme_replaces_graphite_until_it_is_reset() {
    let mut noir = PresetStyles::Noir.def().expect("built in").theme();
    noir.density = Density::Comfortable;

    let ctx = Context::default();
    presets::bootstrap_startup_theme(&ctx);
    assert_eq!(presets::installed_preset(&ctx), PresetStyles::Graphite);

    persist::set_default_theme(noir.clone());
    let fresh = Context::default();
    presets::bootstrap_startup_theme(&fresh);
    assert_eq!(presets::installed_preset(&fresh), PresetStyles::Noir);
    assert!(persist::is_embedded_default(&noir));
    assert_eq!(persist::resolve(None, None).1, ThemeSource::Embedded);

    let store = MemoryStore::default();
    let (theme, source) = persist::resolve_with_store(&store, None);
    assert_eq!(source, ThemeSource::Embedded);
    assert!(presets::themes_equal(&theme, &noir));

    store.save_active(&PresetStyles::Quartz.def().expect("built in").theme());
    assert_eq!(
        persist::resolve_with_store(&store, None).1,
        ThemeSource::Stored
    );

    persist::reset_default_theme();
    assert!(!persist::is_embedded_default(&noir));
    let again = Context::default();
    presets::bootstrap_startup_theme(&again);
    assert_eq!(presets::installed_preset(&again), PresetStyles::Graphite);
}
