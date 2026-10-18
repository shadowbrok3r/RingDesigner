//! Galactic glass: AMOLED-black page, violet glass surfaces, pink press and aqua hover accents.

use eframe::egui::style::{
    HandleShape, ImeComposition, Selection, TextCursorStyle, WidgetVisuals, Widgets,
};
use eframe::egui::{Color32, CornerRadius, Shadow, Stroke, Style, Visuals};

use crate::glass::GlassParams;
use crate::glass_material::{glass_interaction, glass_spacing, glass_text_styles, pane, pane_over};

/// Selection, press and error ink.
const PINK: Color32 = Color32::from_rgb(255, 61, 139);
const PINK_BRIGHT: Color32 = Color32::from_rgb(255, 110, 168);
/// Hover, links and open widgets.
const AQUA: Color32 = Color32::from_rgb(43, 226, 214);
const AQUA_BRIGHT: Color32 = Color32::from_rgb(120, 240, 232);
/// IME underlines.
const VIOLET: Color32 = Color32::from_rgb(163, 140, 255);
const INK: Color32 = Color32::from_rgb(233, 233, 239);
const INK_BRIGHT: Color32 = Color32::from_rgb(248, 250, 252);
/// Hue of floating surfaces.
const SURFACE: Color32 = Color32::from_rgb(19, 17, 30);
const PANE_REST: Color32 = Color32::from_rgb(31, 28, 47);
const PANE_REST_WEAK: Color32 = Color32::from_rgb(25, 23, 38);
const PANE_QUIET: Color32 = Color32::from_rgb(18, 16, 28);
const PANE_QUIET_WEAK: Color32 = Color32::from_rgb(14, 12, 22);
/// Dim white hairlines on resting panes.
const RIM: Color32 = Color32::from_rgba_premultiplied(46, 46, 52, 46);
const RIM_BRIGHT: Color32 = Color32::from_rgba_premultiplied(72, 72, 80, 72);

const WIDGET_RADIUS: u8 = 5;
const WINDOW_RADIUS: u8 = 8;

/// Rest is violet glass under a white rim, hover aqua, press pink, open an aqua rim.
fn galactic_widgets() -> Widgets {
    let radius = CornerRadius::same(WIDGET_RADIUS);
    Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: pane_over(PANE_QUIET, 0.52, Color32::BLACK),
            weak_bg_fill: pane(PANE_QUIET_WEAK, 0.47),
            bg_stroke: Stroke::new(1.0, RIM),
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0, INK),
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            bg_fill: pane_over(PANE_REST, 0.65, Color32::BLACK),
            weak_bg_fill: pane(PANE_REST_WEAK, 0.59),
            bg_stroke: Stroke::new(1.0, RIM_BRIGHT),
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0, INK),
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            bg_fill: pane_over(AQUA, 0.165, Color32::BLACK),
            weak_bg_fill: pane(AQUA, 0.165),
            bg_stroke: Stroke::new(1.5, pane(AQUA, 0.94)),
            corner_radius: radius,
            fg_stroke: Stroke::new(1.5, INK_BRIGHT),
            expansion: 1.0,
        },
        active: WidgetVisuals {
            bg_fill: pane_over(PINK, 0.21, Color32::BLACK),
            weak_bg_fill: pane(PINK, 0.21),
            bg_stroke: Stroke::new(1.7, pane(PINK, 0.96)),
            corner_radius: radius,
            fg_stroke: Stroke::new(2.0, Color32::WHITE),
            expansion: 1.0,
        },
        open: WidgetVisuals {
            bg_fill: pane_over(PANE_REST, 0.65, Color32::BLACK),
            weak_bg_fill: pane(PANE_REST_WEAK, 0.59),
            bg_stroke: Stroke::new(1.3, pane(AQUA, 0.80)),
            corner_radius: radius,
            fg_stroke: Stroke::new(1.0, INK),
            expansion: 0.0,
        },
    }
}

fn galactic_visuals() -> Visuals {
    Visuals {
        dark_mode: true,
        override_text_color: None,
        widgets: galactic_widgets(),
        selection: Selection {
            bg_fill: pane(PINK, 0.55),
            stroke: Stroke::new(1.4, PINK_BRIGHT),
        },
        hyperlink_color: AQUA,
        faint_bg_color: Color32::from_rgb(11, 10, 16),
        extreme_bg_color: Color32::from_rgb(8, 7, 13),
        code_bg_color: Color32::from_rgb(6, 5, 10),
        warn_fg_color: AQUA_BRIGHT,
        error_fg_color: PINK,
        window_corner_radius: CornerRadius::same(WINDOW_RADIUS),
        window_shadow: Shadow {
            offset: [0, 2],
            blur: 12,
            spread: 2,
            color: Color32::from_black_alpha(200),
        },
        window_fill: pane(SURFACE, 0.82),
        window_stroke: Stroke::new(1.2, RIM),
        menu_corner_radius: CornerRadius::same(WINDOW_RADIUS),
        panel_fill: Color32::BLACK,
        popup_shadow: Shadow {
            offset: [0, 2],
            blur: 10,
            spread: 1,
            color: Color32::from_black_alpha(170),
        },
        resize_corner_size: 12.0,
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, PINK_BRIGHT),
            ..Default::default()
        },
        ime_composition: ImeComposition {
            active_underline_stroke: Stroke::new(2.0, VIOLET),
            inactive_underline_stroke: Stroke::new(2.0, pane(VIOLET, 0.5)),
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

/// The full `Style`, sharing geometry and type with the other glass families.
pub fn galactic_style() -> Style {
    Style {
        text_styles: glass_text_styles(),
        spacing: glass_spacing(),
        interaction: glass_interaction(),
        visuals: galactic_visuals(),
        animation_time: 0.1,
        explanation_tooltips: false,
        url_in_tooltip: true,
        compact_menu_style: true,
        ..Default::default()
    }
}

/// The backdrop-blur material under a faint violet film.
pub fn galactic_glass_params() -> GlassParams {
    GlassParams {
        enabled: true,
        blur_radius: 26.0,
        tint: Color32::from_rgba_unmultiplied(19, 17, 30, 26),
        corner_radius: WINDOW_RADIUS as f32,
        presence: 1.0,
    }
}

/// Success and secondary-accent colours, which have no `egui::Visuals` slot.
pub fn galactic_semantic_colors() -> (Color32, Color32) {
    (Color32::from_rgb(94, 232, 178), AQUA)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amoled_translucent_and_readable() {
        let v = galactic_style().visuals;
        assert_eq!(v.panel_fill, Color32::BLACK);
        assert!(v.window_fill.a() < 255, "floating surfaces opaque");
        assert!(v.window_fill.a() >= 178, "window fill too sheer");
        assert!(v.widgets.inactive.weak_bg_fill.a() < 255, "panes opaque");
        assert!(galactic_glass_params().is_visible());
    }

    #[test]
    fn surfaces_carry_the_violet_cast() {
        let v = galactic_style().visuals;
        for (what, c) in [
            ("window_fill", v.window_fill),
            ("widget rest", v.widgets.inactive.weak_bg_fill),
            ("frame", v.widgets.noninteractive.weak_bg_fill),
            ("text well", v.extreme_bg_color),
            ("striped row", v.faint_bg_color),
        ] {
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            assert!(b > r && r >= g, "{what} ({c:?}) is not violet-cast");
        }
        assert_eq!(v.error_fg_color, PINK);
        assert_eq!(v.hyperlink_color, AQUA);
    }

    #[test]
    fn hover_is_aqua_and_press_is_pink() {
        let w = galactic_widgets();
        let [hr, hg, hb, _] = w.hovered.bg_stroke.color.to_srgba_unmultiplied();
        assert!(hg > hr && hb > hr, "hover rim is not aqua");
        let [ar, ag, ab, _] = w.active.bg_stroke.color.to_srgba_unmultiplied();
        assert!(ar > ag && ar > ab, "press rim is not pink");
    }

    #[test]
    fn sunken_wells_stay_visible_over_true_black() {
        let [r, g, b, a] = galactic_style()
            .visuals
            .extreme_bg_color
            .to_srgba_unmultiplied();
        assert_eq!(a, 255);
        assert!(r as u32 + g as u32 + b as u32 > 12);
    }
}
