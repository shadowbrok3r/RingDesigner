//! Theme and styling system for egui apps, installed with one call to [`init`].
//!
//! The kit publishes a [`Tokens`] palette and a [`Metrics`] sizing table on the egui [`Context`],
//! restyles stock widgets from them, ships Inter fonts and dock, table and toast chrome, and adds
//! presets, saved themes, frosted glass and a debug panel on top.
//!
//! # Quick start
//!
//! ```toml
//! egui-theme-kit = { git = "https://github.com/shadowbrok3r/egui-theme-kit" }
//! ```
//!
//! ```no_run
//! use egui_theme_kit::KitConfig;
//!
//! struct App {
//!     ctx: egui::Context,
//! }
//!
//! impl eframe::App for App {
//!     fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
//!         egui::CentralPanel::default().show(ui, |ui| {
//!             ui.heading("Hello");
//!             if ui.add(egui_theme_kit::primary_button("Save")).clicked() {}
//!         });
//!         egui_theme_kit::debug_panel::show(&self.ctx);
//!     }
//!
//!     fn save(&mut self, storage: &mut dyn eframe::Storage) {
//!         egui_theme_kit::save(&self.ctx, storage);
//!     }
//! }
//!
//! fn main() -> eframe::Result {
//!     eframe::run_native(
//!         "demo",
//!         eframe::NativeOptions::default(),
//!         Box::new(|cc| {
//!             egui_theme_kit::init(cc, KitConfig::default());
//!             Ok(Box::new(App { ctx: cc.egui_ctx.clone() }))
//!         }),
//!     )
//! }
//! ```
//!
//! # Concepts
//!
//! - [`Tokens`]: the semantic colors and corner radii, derived from the installed `egui::Style`.
//! - [`Metrics`]: control heights, paddings and icon sizes for the active [`Density`].
//! - [`Density`] and [`TextScale`]: the two user-facing size knobs; density changes geometry, text
//!   scale changes glyph size.
//! - [`presets`]: named looks that install a style, semantic colors and a glass material together.
//! - [`SavedTheme`]: a style plus glass, density and text scale, serialized as JSON.
//! - [`widget_theme`]: restyles stock buttons, checkboxes, text edits and separators; classes such
//!   as [`widget_theme::PRIMARY`] pick a button kind and [`widget_theme::STOCK`] opts a widget out.
//!
//! # Startup theme
//!
//! [`init`] applies the first available of: the `--theme` file (or the `EGUI_THEME_KIT_THEME`
//! variable), the store's active theme, the theme saved in eframe storage,
//! [`KitConfig::default_theme`], [`KitConfig::default_preset`], then the shipped default. [`save`]
//! writes the live theme back to eframe storage and skips the write while the theme equals the
//! configured default or came from a `--theme` file.
//!
//! # Cargo features
//!
//! | Feature | Default | Adds |
//! |---|---|---|
//! | `glass` | yes | GPU backdrop blur through a grab-pass backend; needs eframe on glow |
//! | `files` | yes | native import/export dialogs and the user theme directory |
//! | `dock` | yes | `dock_style` for `egui_dock` |
//! | `toast` | yes | themed `egui-toast` notifications and the panel's toast tester |
//! | `data-table` | yes | `table_style` selection colors for `egui-data-table` |
//! | `picker` | yes | the theme picker window |
//! | `controls` | no | themed control widgets |
//! | `title-bar` | no | app-drawn window title bar, caption buttons, resize edges, `chrome` preferences and the Windows client-area fix |
//! | `logger` | no | in-app log viewer |
//!
//! With `default-features = false` the core remains: tokens, metrics, fonts, the widget theme,
//! presets, saved themes and the debug panel.

#[cfg(feature = "title-bar")]
pub mod chrome;
pub mod cli;
pub mod colors;
#[cfg(feature = "controls")]
pub mod controls;
pub mod debug_panel;
#[cfg(feature = "dock")]
pub mod dock_style;
pub mod fonts;
pub mod glass;
pub mod glass_material;
pub mod kit;
#[cfg(feature = "logger")]
pub mod logger;
pub mod metrics;
pub mod persist;
pub mod presets;
pub mod saved;
pub mod startup;
pub mod store;
pub mod studio;
#[cfg(feature = "data-table")]
pub mod table_style;
#[cfg(feature = "toast")]
pub mod toast;
#[cfg(feature = "title-bar")]
pub mod title_bar;
pub mod tokens;
pub mod widget_theme;

#[cfg(feature = "picker")]
pub mod picker;

#[cfg(test)]
mod tests;
mod theme_chrome;

use eframe::egui::Context;

pub use colors::{
    Status, StatusStyle, accent, accent_secondary, accent_secondary_ctx, bg_extreme, bg_faint,
    bg_surface, border, error, faint_text, info, other_series, primary, primary_ctx, secondary,
    secondary_ctx, series_color, set_accent_secondary, set_success_color, status_style,
    strong_text, success, success_ctx, tag_color, tertiary, tertiary_ctx, text, usage_level,
    usage_status, warn, weak_text,
};
pub use fonts::{FontConfig, FontFace, TextScale};
pub use startup::{KitConfig, init, init_context};
pub use kit::{ButtonKind, danger_button, ghost_button, primary_button};
pub use metrics::{Density, Metrics};
pub use presets::{PresetStyles, apply_preset, apply_saved_theme, bootstrap_startup_theme};
pub use saved::SavedTheme;
pub use tokens::Tokens;

/// The config the deprecated bootstrap functions apply: defaults, with the process arguments left unread.
fn legacy_config() -> KitConfig {
    KitConfig {
        parse_cli: false,
        ..KitConfig::default()
    }
}

/// Installs glass, fonts and the startup theme (`--theme` file, saved theme or embedded default) from the creation context.
#[deprecated(note = "use `init` with a `KitConfig`")]
pub fn bootstrap(cc: &eframe::CreationContext<'_>) {
    glass::install(cc);
    init_context(&cc.egui_ctx, cc.storage, legacy_config());
}

/// [`bootstrap`] without the GL backend: fonts and the startup theme on `ctx`.
#[deprecated(note = "use `init_context` with a `KitConfig`")]
pub fn bootstrap_context(ctx: &Context, storage: Option<&dyn eframe::Storage>) {
    init_context(ctx, storage, legacy_config());
}

/// Replaces the context options with `options`, keeping the installed styles and theme preference.
pub fn apply_preferences(ctx: &Context, options: eframe::egui::Options) {
    ctx.options_mut(|o| {
        let (dark, light) = (o.dark_style.clone(), o.light_style.clone());
        let (preference, fallback) = (o.theme_preference, o.fallback_theme);
        *o = options;
        o.dark_style = dark;
        o.light_style = light;
        o.theme_preference = preference;
        o.fallback_theme = fallback;
    });
}

/// The tokens published on `ctx`.
pub fn tokens(ctx: &Context) -> Tokens {
    Tokens::get(ctx)
}

/// The metrics published on `ctx`.
pub fn metrics(ctx: &Context) -> Metrics {
    Metrics::get(ctx)
}

/// Switches the UI text size, rebuilding the font atlas when it changes.
pub fn set_text_scale(ctx: &Context, scale: fonts::TextScale) {
    presets::set_text_scale(ctx, scale);
}

/// Switches the density and re-installs the current style under it.
pub fn set_density(ctx: &Context, density: Density) {
    presets::set_density(ctx, density);
}

/// The dock chrome built from the published tokens and metrics.
#[cfg(feature = "dock")]
pub fn dock_style(ctx: &Context) -> egui_dock::Style {
    dock_style::style(ctx)
}

/// Writes the live theme to eframe storage; call from `eframe::App::save`.
pub fn save(ctx: &Context, storage: &mut dyn eframe::Storage) {
    let from_file = matches!(persist::source(ctx), persist::ThemeSource::File(_));
    if !from_file && startup::is_configured_default(ctx, &SavedTheme::current(ctx)) {
        storage.remove_string(persist::STORAGE_KEY);
        return;
    }
    persist::save(ctx, storage);
}
