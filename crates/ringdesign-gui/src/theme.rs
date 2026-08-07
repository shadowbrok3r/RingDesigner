//! Visual style and the shared palette.
//!
//! The widget chrome is a serialized [`egui::Style`] in `assets/theme.json`,
//! applied verbatim. The constants below are for the panels that paint
//! themselves — the viewport, the unrolled tile editor, the section view — and
//! are picked from the same palette so hand-drawn shapes match the widgets.

use egui::{Color32, Context, Style, Theme};

/// Serialized `egui::Style`. Deserialized rather than transcribed so a theme
/// edit is a data change and an egui upgrade fails loudly in the test below.
const THEME_JSON: &str = include_str!("../assets/theme.json");

// --- Chrome, taken from the theme -----------------------------------------

/// `visuals.extreme_bg_color`.
pub const BG: Color32 = Color32::from_rgb(13, 13, 18);
/// `visuals.panel_fill`.
pub const PANEL: Color32 = Color32::from_rgb(0, 0, 0);
/// A hair above panel black so the metal reads against it.
pub const VIEWPORT_BG: Color32 = Color32::from_rgb(10, 10, 14);
pub const GRID: Color32 = Color32::from_rgb(34, 33, 46);
/// `widgets.active.bg_stroke` — the theme's pink highlight.
pub const ACCENT: Color32 = Color32::from_rgb(230, 108, 153);
/// `widgets.hovered.bg_stroke` — the periwinkle secondary.
pub const ACCENT_DIM: Color32 = Color32::from_rgb(125, 122, 166);
/// `visuals.override_text_color`.
pub const TEXT: Color32 = Color32::from_rgb(232, 232, 232);
/// `override_text_color` at `weak_text_alpha`.
pub const TEXT_DIM: Color32 = Color32::from_rgb(139, 139, 139);
/// `visuals.selection.stroke.color`.
pub const SELECT: Color32 = Color32::from_rgb(204, 145, 217);
/// `visuals.window_stroke.color`.
pub const HAIRLINE: Color32 = Color32::from_rgb(75, 70, 120);

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

/// Metal colour for the shaded viewport, linear RGB. This is the material, not
/// chrome, so it stays gold.
pub const METAL_RGB: [f32; 3] = [0.86, 0.70, 0.42];

/// Load the icon font and apply the theme.
pub fn install(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::ThemePreference::Dark);
    let style = load_style();
    ctx.set_style_of(Theme::Dark, style.clone());
    ctx.set_style_of(Theme::Light, style);
}

/// The theme, falling back to egui's dark default if it will not parse.
fn load_style() -> Style {
    match serde_json::from_str::<Style>(THEME_JSON) {
        Ok(s) => s,
        Err(e) => {
            log::error!("theme.json did not parse, using the default dark style: {e}");
            let mut s = Style::default();
            s.visuals = egui::Visuals::dark();
            s
        }
    }
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
        let style: Style = serde_json::from_str(THEME_JSON)
            .expect("assets/theme.json no longer matches egui::Style");
        assert!(style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, PANEL);
        assert_eq!(style.visuals.extreme_bg_color, BG);
        assert_eq!(style.visuals.override_text_color, Some(TEXT));
    }

    #[test]
    fn palette_constants_match_the_theme_they_were_taken_from() {
        // The theme's strokes carry alpha; the painted constants are opaque and
        // composite over a black panel to the same colour, so compare RGB only.
        let rgb = |c: Color32| [c.r(), c.g(), c.b()];
        let style: Style = serde_json::from_str(THEME_JSON).unwrap();
        assert_eq!(rgb(style.visuals.widgets.active.bg_stroke.color), rgb(ACCENT));
        assert_eq!(rgb(style.visuals.widgets.hovered.bg_stroke.color), rgb(ACCENT_DIM));
        assert_eq!(rgb(style.visuals.selection.stroke.color), rgb(SELECT));
        assert_eq!(rgb(style.visuals.window_stroke.color), rgb(HAIRLINE));
    }
}
