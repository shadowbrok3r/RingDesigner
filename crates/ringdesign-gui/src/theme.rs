//! Desktop chrome from egui-theme-kit, with migration of the earlier mobile theme.
use egui::{Color32, Context};
use egui_mobile_theme::palette;
use egui_theme_kit::store::{EframeStorageStore, ThemeStore};
use egui_theme_kit::{Density, FontConfig, KitConfig, SavedTheme, TextScale};
use std::sync::Arc;

const MIGRATED_KEY: &str = "ringdesigner.theme_kit.v1";

pub struct ThemeState {
    picker: egui_theme_kit::picker::PickerState,
    store: Arc<EframeStorageStore>,
}

// The initial kit release exposes a narrower startup-store trait than its
// picker store. Bridge the two, including the picker's pinned startup choice.
struct StartupStore(Arc<EframeStorageStore>);
impl egui_theme_kit::startup::ThemeStore for StartupStore {
    fn load_active(&self) -> Option<SavedTheme> {
        let (theme, source) = egui_theme_kit::persist::resolve_with_store(self.0.as_ref(), None);
        (!matches!(source, egui_theme_kit::persist::ThemeSource::Embedded)).then_some(theme)
    }
    fn save_active(&self, theme: &SavedTheme) {
        self.0.save_active(theme);
    }
}

impl ThemeState {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let store = Arc::new(EframeStorageStore::from_storage(cc.storage));
        // A marker is separate from the active theme: the kit can remove its key
        // when reset to the default. Never resurrect an old theme after that reset.
        if store.load_active().is_none()
            && cc
                .storage
                .and_then(|s| s.get_string(MIGRATED_KEY))
                .is_none()
        {
            if let Some(legacy) = cc
                .storage
                .and_then(|s| s.get_string(crate::app::THEME_STORAGE_KEY))
                .and_then(|json| serde_json::from_str(&json).ok())
            {
                store.save_active(&import_legacy(legacy));
            }
        }
        egui_theme_kit::init(
            cc,
            KitConfig::new()
                .fonts(FontConfig::new().installer(|fonts| {
                    egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular);
                }))
                .default_theme(import_legacy(Default::default()))
                .store(Arc::new(StartupStore(store.clone())))
                .parse_cli(!cfg!(test)),
        );
        Self {
            picker: Default::default(),
            store,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let before = ui.ctx().global_style();
        let revision = self.store.revision();
        egui_theme_kit::picker::show(ui, &mut self.picker, self.store.as_ref());
        !Arc::ptr_eq(&before, &ui.ctx().global_style()) || revision != self.store.revision()
    }

    pub fn save(&self, ctx: &Context, storage: &mut dyn eframe::Storage) {
        // Capture the live choice even if the picker debounce hasn't fired yet.
        let from_file = matches!(
            egui_theme_kit::persist::source(ctx),
            egui_theme_kit::persist::ThemeSource::File(_)
        );
        if !from_file {
            self.store.save_active(&SavedTheme::current(ctx));
        }
        self.store.save_to(storage);
        // The store elides Graphite, its own default. RingDesigner's configured
        // default is different, so explicitly retain every chosen active theme.
        if let Some(active) = self.store.load_active() {
            match egui_theme_kit::saved::theme_to_json(&active) {
                Ok(json) => storage.set_string(
                    egui_theme_kit::persist::STORAGE_KEY,
                    String::from_utf8_lossy(&json).into_owned(),
                ),
                Err(error) => log::error!("Cannot save theme: {error}"),
            }
        }
        storage.set_string(MIGRATED_KEY, "true".into());
    }
}

fn import_legacy(mut legacy: egui_mobile_theme::ThemeConfig) -> SavedTheme {
    let density = match legacy.control_height.unwrap_or(22.) {
        height if height >= 27. => Density::Comfortable,
        height if height >= 24. => Density::Regular,
        _ => Density::Compact,
    };
    let text_scale = match legacy.text_scale {
        scale if scale < 0.95 => TextScale::Small,
        scale if scale >= 1.225 => TextScale::XLarge,
        scale if scale >= 1.075 => TextScale::Large,
        _ => TextScale::Normal,
    };
    legacy.text_scale = 1.;
    // Convert on a temporary context, so only the kit installs widget styling
    // in the real app. The previous JSON stays in storage for older versions.
    let ctx = Context::default();
    legacy.apply(&ctx, egui_mobile_theme::Density::Compact);
    SavedTheme {
        style: (*ctx.global_style()).clone(),
        glass: None,
        density,
        text_scale,
        semantic: Some(egui_theme_kit::saved::SemanticColors {
            success: legacy.colors.success,
            secondary: legacy.colors.info,
        }),
    }
}

pub const BG: Color32 = palette::WELL;
pub const PANEL: Color32 = palette::PANEL;
pub const VIEWPORT_BG: Color32 = Color32::from_rgb(12, 12, 16);
pub const FLOAT: Color32 = palette::WINDOW;
pub const GRID: Color32 = Color32::from_rgb(34, 34, 42);
pub const ACCENT: Color32 = palette::ACCENT;
pub const ACCENT_DIM: Color32 = palette::HOVER;
pub const TEXT: Color32 = palette::TEXT;
pub const TEXT_DIM: Color32 = palette::TEXT_DIM;
pub const SELECT: Color32 = Color32::from_rgb(184, 147, 199);
pub const HAIRLINE: Color32 = palette::BORDER;

// --- Castability status ----------------------------------------------------
//
// These track `FaceClass::rgb` in ringdesign-core, not the theme's warn/error
// colours: the report's status text sits beside legend swatches painted from
// the core colours, and the 3D viewport bakes those same colours into the mesh.
// Recolouring them here would desync the words from the picture.

pub const GOOD: Color32 = Color32::from_rgb(82, 199, 115);
pub const WARN: Color32 = Color32::from_rgb(242, 194, 61);
pub const BAD: Color32 = Color32::from_rgb(237, 69, 92);
pub const INFO: Color32 = Color32::from_rgb(92, 153, 235);

/// Colour for a castability verdict.
pub fn verdict_color(v: ringdesign_core::castability::Verdict) -> Color32 {
    use ringdesign_core::castability::Verdict;
    match v {
        Verdict::Castable => GOOD,
        Verdict::Marginal => WARN,
        Verdict::NotCastable => BAD,
    }
}

/// Colour for a face class.
pub fn class_color(c: ringdesign_core::FaceClass) -> Color32 {
    let [r, g, b] = c.rgb();
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_json_parses_against_this_egui() {
        let style = egui_mobile_theme::desktop_style();
        assert!(style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, PANEL);
        assert_eq!(style.visuals.text_color(), TEXT);
    }
    #[test]
    fn the_chrome_sits_below_the_viewport() {
        let lum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        assert!(lum(BG) < lum(PANEL));
        assert!(lum(PANEL) < lum(VIEWPORT_BG));
        assert!(lum(VIEWPORT_BG) < lum(FLOAT));
        assert!(lum(VIEWPORT_BG) < lum(GRID));
    }
    #[test]
    fn palette_constants_match_the_theme_they_were_taken_from() {
        let style = egui_mobile_theme::desktop_style();
        assert_eq!(style.visuals.widgets.active.bg_stroke.color, ACCENT);
        assert_eq!(style.visuals.window_stroke.color, HAIRLINE);
    }
}
