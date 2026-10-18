//! Soft glass family: low-chroma glass over a near-black base with outlined controls, generated from one [`SoftPalette`].

use eframe::egui::style::{
    HandleShape, ImeComposition, Interaction, Selection, Spacing, TextCursorStyle, WidgetVisuals,
    Widgets,
};
use eframe::egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Style, Vec2, Visuals};

use crate::glass::GlassParams;
use crate::glass_material::{
    edge, glass_interaction, glass_spacing, glass_text_styles, lift, pane, pane_over,
};

/// The colours one soft glass theme is generated from.
#[derive(Clone, Copy, Debug)]
pub struct SoftPalette {
    pub name: &'static str,
    /// Panel background: near-black, cast toward the palette hue.
    pub base: Color32,
    /// Hue of floating surfaces and sunken wells.
    pub surface: Color32,
    /// Opacity of floating surfaces; low values make nested popups hard to read.
    pub surface_alpha: f32,
    /// Resting outline of every control; near-neutral so frames read without shouting.
    pub frame: Color32,
    /// Hover accent, links and selection.
    pub primary: Color32,
    /// Press accent.
    pub secondary: Color32,
    /// Warnings and highlights.
    pub tertiary: Color32,
    pub success: Color32,
    pub error: Color32,
    pub text: Color32,
    /// Shadow colour under floating surfaces.
    pub shade: Color32,
}

const WIDGET_RADIUS: u8 = 3;
const WINDOW_RADIUS: u8 = 5;
const MENU_RADIUS: u8 = 4;

/// Opacity of the resting outline; below about 0.5 controls stop reading as controls.
const FRAME_OPACITY: f32 = 0.72;

/// Graphite neutral under muted violet.
pub const OBSIDIAN: SoftPalette = SoftPalette {
    name: "Obsidian",
    base: Color32::from_rgb(8, 8, 10),
    surface: Color32::from_rgb(17, 17, 21),
    surface_alpha: 0.82,
    frame: Color32::from_rgb(108, 112, 138),
    primary: Color32::from_rgb(150, 134, 224),
    secondary: Color32::from_rgb(188, 124, 220),
    tertiary: Color32::from_rgb(126, 200, 230),
    success: Color32::from_rgb(128, 206, 168),
    error: Color32::from_rgb(236, 98, 132),
    text: Color32::from_rgb(226, 226, 234),
    shade: Color32::from_rgb(2, 2, 4),
};

/// Warm plum base under mauve, pressing to rose.
pub const VELVET: SoftPalette = SoftPalette {
    name: "Velvet",
    base: Color32::from_rgb(12, 9, 12),
    surface: Color32::from_rgb(25, 18, 26),
    surface_alpha: 0.82,
    frame: Color32::from_rgb(124, 108, 126),
    primary: Color32::from_rgb(182, 142, 206),
    secondary: Color32::from_rgb(226, 132, 158),
    tertiary: Color32::from_rgb(150, 196, 220),
    success: Color32::from_rgb(142, 206, 164),
    error: Color32::from_rgb(238, 88, 96),
    text: Color32::from_rgb(234, 228, 236),
    shade: Color32::from_rgb(5, 2, 5),
};

/// Deep indigo under periwinkle, pressing to soft magenta.
pub const TWILIGHT: SoftPalette = SoftPalette {
    name: "Twilight",
    base: Color32::from_rgb(8, 9, 14),
    surface: Color32::from_rgb(17, 19, 31),
    surface_alpha: 0.82,
    frame: Color32::from_rgb(100, 108, 142),
    primary: Color32::from_rgb(138, 152, 232),
    secondary: Color32::from_rgb(196, 130, 224),
    tertiary: Color32::from_rgb(122, 198, 222),
    success: Color32::from_rgb(124, 208, 176),
    error: Color32::from_rgb(240, 110, 138),
    text: Color32::from_rgb(222, 226, 240),
    shade: Color32::from_rgb(2, 3, 7),
};

/// Cool smoke with the lightest surfaces of the family, under lilac.
pub const QUARTZ: SoftPalette = SoftPalette {
    name: "Quartz",
    base: Color32::from_rgb(14, 14, 17),
    surface: Color32::from_rgb(31, 31, 37),
    surface_alpha: 0.80,
    frame: Color32::from_rgb(128, 130, 152),
    primary: Color32::from_rgb(170, 158, 226),
    secondary: Color32::from_rgb(216, 140, 180),
    tertiary: Color32::from_rgb(140, 196, 214),
    success: Color32::from_rgb(140, 204, 172),
    error: Color32::from_rgb(238, 100, 120),
    text: Color32::from_rgb(232, 232, 238),
    shade: Color32::from_rgb(3, 3, 5),
};

/// Every palette in the family, in picker order.
pub const SOFT_THEMES: [SoftPalette; 4] = [OBSIDIAN, VELVET, TWILIGHT, QUARTZ];

struct Pane {
    fill: f32,
    weak_fill: f32,
    edge_opacity: f32,
    edge_width: f32,
    fg_width: f32,
    expansion: f32,
}

/// One widget state: a faint pane of `fill_tint` bounded by an outline cut from `edge_tint`.
fn soft_widget(
    p: &SoftPalette,
    (fill_tint, edge_tint): (Color32, Color32),
    fg: Color32,
    s: Pane,
) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: pane_over(fill_tint, s.fill, p.base),
        weak_bg_fill: pane(fill_tint, s.weak_fill),
        bg_stroke: Stroke::new(s.edge_width, edge(edge_tint, s.edge_opacity)),
        corner_radius: CornerRadius::same(WIDGET_RADIUS),
        fg_stroke: Stroke::new(s.fg_width, fg),
        expansion: s.expansion,
    }
}

fn soft_widgets(p: &SoftPalette) -> Widgets {
    let state = |fill, weak_fill, edge_opacity, edge_width, fg_width, expansion| Pane {
        fill,
        weak_fill,
        edge_opacity,
        edge_width,
        fg_width,
        expansion,
    };
    let framed = (p.primary, p.frame);
    Widgets {
        noninteractive: soft_widget(p, framed, p.text, state(0.035, 0.025, 0.30, 1.0, 1.0, 0.0)),
        inactive: soft_widget(
            p,
            framed,
            p.text,
            state(0.085, 0.055, FRAME_OPACITY, 1.0, 1.0, 0.0),
        ),
        hovered: soft_widget(
            p,
            (p.primary, p.primary),
            lift(p.text, 0.4),
            state(0.16, 0.12, 0.95, 1.4, 1.4, 1.0),
        ),
        active: soft_widget(
            p,
            (p.secondary, p.secondary),
            Color32::WHITE,
            state(0.24, 0.18, 1.0, 1.6, 1.8, 1.0),
        ),
        open: soft_widget(
            p,
            (p.primary, p.primary),
            p.text,
            state(0.13, 0.10, 0.70, 1.2, 1.0, 0.0),
        ),
    }
}

fn soft_selection(p: &SoftPalette) -> Selection {
    Selection {
        bg_fill: pane(p.primary, 0.30),
        stroke: Stroke::new(1.0, edge(p.primary, 0.85)),
    }
}

fn soft_visuals(p: &SoftPalette) -> Visuals {
    Visuals {
        dark_mode: true,
        override_text_color: None,
        widgets: soft_widgets(p),
        selection: soft_selection(p),
        hyperlink_color: lift(p.primary, 0.15),
        faint_bg_color: pane(p.primary, 0.04),
        extreme_bg_color: pane_over(lift(p.surface, 0.06), 0.95, p.base),
        code_bg_color: pane_over(p.surface, 0.90, p.base),
        warn_fg_color: p.tertiary,
        error_fg_color: p.error,
        window_corner_radius: CornerRadius::same(WINDOW_RADIUS),
        window_shadow: Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: pane(p.shade, 0.60),
        },
        window_fill: pane(p.surface, p.surface_alpha),
        window_stroke: Stroke::new(1.0, edge(p.frame, 0.80)),
        menu_corner_radius: CornerRadius::same(MENU_RADIUS),
        panel_fill: p.base,
        popup_shadow: Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: pane(p.shade, 0.50),
        },
        resize_corner_size: 12.0,
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, lift(p.primary, 0.45)),
            ..Default::default()
        },
        ime_composition: ImeComposition {
            active_underline_stroke: Stroke::new(2.0, lift(p.primary, 0.45)),
            inactive_underline_stroke: Stroke::new(2.0, pane(lift(p.primary, 0.45), 0.5)),
            ..Default::default()
        },
        button_frame: true,
        collapsing_header_frame: true,
        indent_has_left_vline: true,
        striped: true,
        slider_trailing_fill: true,
        handle_shape: HandleShape::Rect { aspect_ratio: 0.6 },
        image_loading_spinners: true,
        ..Default::default()
    }
}

/// Spacing tighter than the shared glass baseline, keeping its horizontal button padding.
fn soft_spacing() -> Spacing {
    Spacing {
        item_spacing: Vec2 { x: 3.0, y: 2.0 },
        window_margin: Margin::same(4),
        button_padding: Vec2 { x: 5.0, y: 1.0 },
        menu_margin: Margin::same(5),
        indent: 14.0,
        ..glass_spacing()
    }
}

fn soft_interaction() -> Interaction {
    Interaction {
        tooltip_delay: 0.35,
        ..glass_interaction()
    }
}

/// The full `Style` for a palette.
pub fn soft_style(p: &SoftPalette) -> Style {
    Style {
        text_styles: glass_text_styles(),
        spacing: soft_spacing(),
        interaction: soft_interaction(),
        visuals: soft_visuals(p),
        animation_time: 0.12,
        explanation_tooltips: false,
        url_in_tooltip: true,
        compact_menu_style: true,
        ..Default::default()
    }
}

/// The backdrop-blur material for a palette: a wide smear under a slightly heavier film.
pub fn soft_glass_params(p: &SoftPalette) -> GlassParams {
    let [r, g, b, _] = p.surface.to_srgba_unmultiplied();
    GlassParams {
        enabled: true,
        blur_radius: 32.0,
        tint: Color32::from_rgba_unmultiplied(r, g, b, 34),
        corner_radius: MENU_RADIUS as f32,
        presence: 1.0,
    }
}

/// Success and secondary-accent colours, which have no `egui::Visuals` slot.
pub fn soft_semantic_colors(p: &SoftPalette) -> (Color32, Color32) {
    (p.success, p.secondary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chroma(c: Color32) -> i32 {
        let [r, g, b, _] = c.to_srgba_unmultiplied();
        (r.max(g).max(b) as i32) - (r.min(g).min(b) as i32)
    }

    fn distance(a: Color32, b: Color32) -> i32 {
        let (x, y) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
        (0..3).map(|i| (x[i] as i32 - y[i] as i32).abs()).sum()
    }

    #[test]
    fn a_resting_control_is_framed_and_uncolored() {
        for p in SOFT_THEMES {
            let w = soft_style(&p).visuals.widgets.inactive;
            assert!(w.bg_stroke.width >= 1.0, "{}: sub-pixel frame", p.name);
            assert!(w.bg_stroke.color.a() >= 150, "{}: frame too sheer", p.name);
            assert!(
                w.bg_stroke.color.a() > w.weak_bg_fill.a(),
                "{}: fill",
                p.name
            );
            assert!(
                chroma(w.bg_stroke.color) < chroma(p.primary),
                "{}: frame",
                p.name
            );
        }
    }

    #[test]
    fn hover_and_press_escalate_the_outline() {
        for p in SOFT_THEMES {
            let w = &soft_style(&p).visuals.widgets;
            let (rest, hover, press) = (&w.inactive, &w.hovered, &w.active);
            assert!(hover.bg_stroke.width > rest.bg_stroke.width, "{}", p.name);
            assert!(press.bg_stroke.width > hover.bg_stroke.width, "{}", p.name);
            assert!(chroma(hover.bg_stroke.color) > chroma(rest.bg_stroke.color));
        }
    }

    #[test]
    fn accents_stay_below_the_neon_families_saturation() {
        for p in SOFT_THEMES {
            for c in [p.primary, p.secondary, p.tertiary, p.success] {
                assert!(chroma(c) <= 120, "{}: chroma {}", p.name, chroma(c));
            }
        }
    }

    #[test]
    fn surface_alpha_keeps_nested_popups_readable() {
        for p in SOFT_THEMES {
            assert!((0.70..=0.92).contains(&p.surface_alpha), "{}", p.name);
            let fill = soft_style(&p).visuals.window_fill;
            assert!(fill.a() >= 178, "{}: window alpha {}", p.name, fill.a());
        }
    }

    #[test]
    fn sunken_wells_stay_visible_over_the_base() {
        for p in SOFT_THEMES {
            let v = soft_style(&p).visuals;
            let [r, g, b, a] = v.extreme_bg_color.to_srgba_unmultiplied();
            assert_eq!(a, 255, "{}: translucent well", p.name);
            let [br, bg, bb, _] = p.base.to_srgba_unmultiplied();
            let step = (r as i32 - br as i32) + (g as i32 - bg as i32) + (b as i32 - bb as i32);
            assert!(step > 40, "{}: well flush with the base", p.name);
            let surface_blue = p.surface.to_srgba_unmultiplied()[2];
            assert!(v.extreme_bg_color.to_srgba_unmultiplied()[2] > surface_blue);
        }
    }

    #[test]
    fn error_is_distinguishable_from_the_accents() {
        for p in SOFT_THEMES {
            for accent in [p.primary, p.secondary] {
                assert!(distance(p.error, accent) > 90, "{}", p.name);
            }
        }
    }

    #[test]
    fn every_palette_is_translucent_and_glassy() {
        for p in SOFT_THEMES {
            let v = soft_style(&p).visuals;
            assert!(v.window_fill.a() < 255, "{}", p.name);
            assert!(v.widgets.inactive.weak_bg_fill.a() < 255, "{}", p.name);
            assert!(soft_glass_params(&p).is_visible(), "{}", p.name);
        }
    }

    #[test]
    fn geometry_stays_at_or_under_the_shared_glass_baseline() {
        let (base, s) = (glass_spacing(), soft_spacing());
        assert!(s.item_spacing.y <= base.item_spacing.y);
        assert!(s.item_spacing.x <= base.item_spacing.x);
        assert!(s.button_padding.x <= base.button_padding.x);
        assert!(s.button_padding.y <= base.button_padding.y);
        assert!(s.window_margin.left <= base.window_margin.left);
        assert!(s.menu_margin.left <= base.menu_margin.left);
        assert!(s.indent <= base.indent);
        assert!(s.interact_size.y <= base.interact_size.y);
        for p in SOFT_THEMES {
            assert_eq!(soft_glass_params(&p).corner_radius, MENU_RADIUS as f32);
        }
    }

    #[test]
    fn palette_names_are_unique() {
        let mut names: Vec<&str> = SOFT_THEMES.iter().map(|p| p.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
