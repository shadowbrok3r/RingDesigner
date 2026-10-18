//! Mastertech widget chrome shared with EguiMobile and the Android shell.
use egui::{Color32, Context};
use egui_mobile_theme::palette;

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

/// Load the icon font and apply the theme.
pub fn install(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    egui_mobile_theme::apply(ctx, egui_mobile_theme::Density::Compact);
}

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
