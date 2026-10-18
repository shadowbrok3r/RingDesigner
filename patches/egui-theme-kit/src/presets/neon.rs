//! Neon glass family: AMOLED-black base, translucent accent panes and glowing edges generated from one [`NeonPalette`].

use eframe::egui::style::{
    HandleShape, ImeComposition, Selection, TextCursorStyle, WidgetVisuals, Widgets,
};
use eframe::egui::{Color32, CornerRadius, Shadow, Stroke, Style, Visuals};

use crate::glass::GlassParams;
use crate::glass_material::{
    edge, glass_interaction, glass_spacing, glass_text_styles, lift, pane, pane_over,
};

/// The colours one neon glass theme is generated from.
#[derive(Clone, Copy, Debug)]
pub struct NeonPalette {
    pub name: &'static str,
    /// Panel background; true black for AMOLED.
    pub base: Color32,
    /// Hue of floating surfaces and sunken wells.
    pub surface: Color32,
    /// Opacity of floating surfaces; low values make nested popups hard to read.
    pub surface_alpha: f32,
    /// Idle and hover accent.
    pub primary: Color32,
    /// Press accent.
    pub secondary: Color32,
    /// Warnings and highlights.
    pub tertiary: Color32,
    pub success: Color32,
    pub error: Color32,
    pub text: Color32,
    /// Shadow colour behind floating surfaces.
    pub glow: Color32,
}

const WIDGET_RADIUS: u8 = 7;
const WINDOW_RADIUS: u8 = 10;
const MENU_RADIUS: u8 = 8;

/// Violet and magenta over deep space.
pub const NEBULA: NeonPalette = NeonPalette {
    name: "Nebula",
    base: Color32::BLACK,
    surface: Color32::from_rgb(11, 8, 20),
    surface_alpha: 0.82,
    primary: Color32::from_rgb(139, 110, 255),
    secondary: Color32::from_rgb(216, 102, 255),
    tertiary: Color32::from_rgb(94, 226, 255),
    success: Color32::from_rgb(94, 232, 178),
    error: Color32::from_rgb(255, 73, 137),
    text: Color32::from_rgb(233, 230, 245),
    glow: Color32::from_rgb(108, 68, 255),
};

/// Aqua and mint with violet edges.
pub const AURORA: NeonPalette = NeonPalette {
    name: "Aurora",
    base: Color32::BLACK,
    surface: Color32::from_rgb(5, 15, 16),
    surface_alpha: 0.82,
    primary: Color32::from_rgb(72, 232, 196),
    secondary: Color32::from_rgb(94, 226, 255),
    tertiary: Color32::from_rgb(167, 139, 255),
    success: Color32::from_rgb(110, 240, 170),
    error: Color32::from_rgb(255, 92, 140),
    text: Color32::from_rgb(226, 241, 238),
    glow: Color32::from_rgb(28, 190, 172),
};

/// Hot pink over a magenta-black base.
pub const SUPERNOVA: NeonPalette = NeonPalette {
    name: "Supernova",
    base: Color32::BLACK,
    surface: Color32::from_rgb(18, 6, 15),
    surface_alpha: 0.82,
    primary: Color32::from_rgb(255, 92, 170),
    secondary: Color32::from_rgb(177, 108, 255),
    tertiary: Color32::from_rgb(94, 226, 255),
    success: Color32::from_rgb(94, 232, 178),
    error: Color32::from_rgb(255, 104, 72),
    text: Color32::from_rgb(247, 230, 241),
    glow: Color32::from_rgb(255, 58, 150),
};

/// Electric blue and cyan over navy-black.
pub const EVENT_HORIZON: NeonPalette = NeonPalette {
    name: "Event Horizon",
    base: Color32::BLACK,
    surface: Color32::from_rgb(5, 10, 22),
    surface_alpha: 0.82,
    primary: Color32::from_rgb(92, 142, 255),
    secondary: Color32::from_rgb(94, 226, 255),
    tertiary: Color32::from_rgb(167, 139, 255),
    success: Color32::from_rgb(94, 232, 178),
    error: Color32::from_rgb(255, 86, 132),
    text: Color32::from_rgb(223, 232, 250),
    glow: Color32::from_rgb(48, 96, 255),
};

/// Every palette in the family, in picker order.
pub const NEON_THEMES: [NeonPalette; 4] = [NEBULA, AURORA, SUPERNOVA, EVENT_HORIZON];

struct Pane {
    fill: f32,
    weak_fill: f32,
    edge_opacity: f32,
    edge_width: f32,
    fg_width: f32,
    expansion: f32,
}

/// One widget state: a translucent pane of `tint` over the base inside a brighter cut of it.
fn neon_widget(p: &NeonPalette, tint: Color32, fg: Color32, s: Pane) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: pane_over(tint, s.fill, p.base),
        weak_bg_fill: pane(tint, s.weak_fill),
        bg_stroke: Stroke::new(s.edge_width, edge(tint, s.edge_opacity)),
        corner_radius: CornerRadius::same(WIDGET_RADIUS),
        fg_stroke: Stroke::new(s.fg_width, fg),
        expansion: s.expansion,
    }
}

fn neon_widgets(p: &NeonPalette) -> Widgets {
    let state = |fill, weak_fill, edge_opacity, edge_width, fg_width, expansion| Pane {
        fill,
        weak_fill,
        edge_opacity,
        edge_width,
        fg_width,
        expansion,
    };
    Widgets {
        noninteractive: neon_widget(p, p.primary, p.text, state(0.05, 0.03, 0.22, 1.0, 1.0, 0.0)),
        inactive: neon_widget(
            p,
            p.primary,
            p.text,
            state(0.10, 0.065, 0.40, 1.0, 1.0, 0.0),
        ),
        hovered: neon_widget(
            p,
            p.primary,
            lift(p.text, 0.45),
            state(0.19, 0.14, 0.85, 1.4, 1.4, 1.0),
        ),
        active: neon_widget(
            p,
            p.secondary,
            Color32::WHITE,
            state(0.27, 0.19, 1.0, 1.6, 1.8, 1.0),
        ),
        open: neon_widget(p, p.primary, p.text, state(0.15, 0.11, 0.56, 1.0, 1.0, 0.0)),
    }
}

fn neon_selection(p: &NeonPalette) -> Selection {
    Selection {
        bg_fill: pane(p.primary, 0.34),
        stroke: Stroke::new(1.0, edge(p.primary, 0.9)),
    }
}

fn neon_visuals(p: &NeonPalette) -> Visuals {
    Visuals {
        dark_mode: true,
        override_text_color: None,
        widgets: neon_widgets(p),
        selection: neon_selection(p),
        hyperlink_color: lift(p.tertiary, 0.1),
        faint_bg_color: pane(p.primary, 0.045),
        extreme_bg_color: pane_over(p.primary, 0.09, p.base),
        code_bg_color: pane_over(p.surface, 0.9, p.base),
        warn_fg_color: p.tertiary,
        error_fg_color: p.error,
        window_corner_radius: CornerRadius::same(WINDOW_RADIUS),
        window_shadow: Shadow {
            offset: [0, 0],
            blur: 26,
            spread: 3,
            color: pane(p.glow, 0.42),
        },
        window_fill: pane(p.surface, p.surface_alpha),
        window_stroke: Stroke::new(1.0, edge(p.primary, 0.55)),
        menu_corner_radius: CornerRadius::same(MENU_RADIUS),
        panel_fill: p.base,
        popup_shadow: Shadow {
            offset: [0, 0],
            blur: 18,
            spread: 2,
            color: pane(p.glow, 0.34),
        },
        resize_corner_size: 12.0,
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, lift(p.tertiary, 0.4)),
            ..Default::default()
        },
        ime_composition: ImeComposition {
            active_underline_stroke: Stroke::new(2.0, lift(p.primary, 0.5)),
            inactive_underline_stroke: Stroke::new(2.0, pane(lift(p.primary, 0.5), 0.5)),
            ..Default::default()
        },
        button_frame: true,
        collapsing_header_frame: true,
        indent_has_left_vline: true,
        striped: true,
        slider_trailing_fill: true,
        handle_shape: HandleShape::Circle,
        image_loading_spinners: true,
        ..Default::default()
    }
}

/// The full `Style` for a palette, sharing geometry and type with the other glass families.
pub fn neon_style(p: &NeonPalette) -> Style {
    Style {
        text_styles: glass_text_styles(),
        spacing: glass_spacing(),
        interaction: glass_interaction(),
        visuals: neon_visuals(p),
        animation_time: 0.1,
        explanation_tooltips: false,
        url_in_tooltip: true,
        compact_menu_style: true,
        ..Default::default()
    }
}

/// The backdrop-blur material for a palette, under a light film of its surface hue.
pub fn neon_glass_params(p: &NeonPalette) -> GlassParams {
    let [r, g, b, _] = p.surface.to_srgba_unmultiplied();
    GlassParams {
        enabled: true,
        blur_radius: 26.0,
        tint: Color32::from_rgba_unmultiplied(r, g, b, 26),
        corner_radius: MENU_RADIUS as f32,
        presence: 1.0,
    }
}

/// Success and secondary-accent colours, which have no `egui::Visuals` slot.
pub fn neon_semantic_colors(p: &NeonPalette) -> (Color32, Color32) {
    (p.success, p.secondary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distance(a: Color32, b: Color32) -> i32 {
        let (x, y) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
        (0..3).map(|i| (x[i] as i32 - y[i] as i32).abs()).sum()
    }

    #[test]
    fn every_palette_is_amoled_translucent_and_glassy() {
        for p in NEON_THEMES {
            let v = neon_style(&p).visuals;
            assert_eq!(v.panel_fill, Color32::BLACK, "{}: panel", p.name);
            assert!(v.window_fill.a() < 255, "{}: window opaque", p.name);
            assert!(
                v.widgets.inactive.weak_bg_fill.a() < 255,
                "{}: pane",
                p.name
            );
            assert!(neon_glass_params(&p).is_visible(), "{}: glass", p.name);
        }
    }

    #[test]
    fn surface_alpha_keeps_nested_popups_readable() {
        for p in NEON_THEMES {
            assert!(
                (0.70..=0.92).contains(&p.surface_alpha),
                "{}: surface_alpha {}",
                p.name,
                p.surface_alpha
            );
            let fill = neon_style(&p).visuals.window_fill;
            assert!(fill.a() >= 178, "{}: window alpha {}", p.name, fill.a());
        }
    }

    #[test]
    fn sunken_wells_stay_visible_over_true_black() {
        for p in NEON_THEMES {
            let [r, g, b, a] = neon_style(&p)
                .visuals
                .extreme_bg_color
                .to_srgba_unmultiplied();
            assert_eq!(a, 255, "{}: extreme_bg translucent", p.name);
            assert!(r as u32 + g as u32 + b as u32 > 12, "{}: well", p.name);
        }
    }

    #[test]
    fn error_is_distinguishable_from_the_accents() {
        for p in NEON_THEMES {
            for (label, accent) in [("primary", p.primary), ("secondary", p.secondary)] {
                assert!(
                    distance(p.error, accent) > 90,
                    "{}: error vs {label}",
                    p.name
                );
            }
        }
    }

    #[test]
    fn palette_names_are_unique() {
        let mut names: Vec<&str> = NEON_THEMES.iter().map(|p| p.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
