//! Palette presets: a flat colour spec painted onto an existing chrome without touching geometry or fonts.

use eframe::egui::style::WidgetVisuals;
use eframe::egui::{Color32, Stroke, Style};

use crate::tokens::mix;

/// Share of the text colour mixed into the border for the outline of resting controls.
const REST_EDGE_MIX: f32 = 0.22;
/// Share of the primary mixed into the border for the outline of a hovered control.
const HOVER_EDGE_MIX: f32 = 0.6;
/// Share of the primary mixed into the widget fill for a pressed control.
const PRESS_MIX: f32 = 0.28;

/// The colours a palette preset paints onto a chrome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub name: &'static str,
    pub primary: Color32,
    pub secondary: Color32,
    /// Panel and window background.
    pub base: Color32,
    /// Striped rows and open widgets.
    pub faint: Color32,
    /// Text fields, scroll tracks and widget outlines.
    pub sunken: Color32,
    /// Widget fills and code blocks.
    pub widget: Color32,
    /// Window outline and the base of control outlines.
    pub border: Color32,
    pub text: Color32,
    pub success: Color32,
    pub warn: Color32,
    pub error: Color32,
    /// Links; the secondary accent when unset.
    pub info: Option<Color32>,
}

impl Palette {
    /// Link colour.
    pub fn info_color(&self) -> Color32 {
        self.info.unwrap_or(self.secondary)
    }

    /// Success and secondary-accent colours, which have no `egui::Visuals` slot.
    pub fn semantic_colors(&self) -> (Color32, Color32) {
        (self.success, self.secondary)
    }
}

/// Paints `palette` onto `style`, leaving spacing, fonts and widget chrome flags as they are.
pub fn paint_palette(style: &mut Style, palette: &Palette) {
    let v = &mut style.visuals;
    v.override_text_color = Some(palette.text);
    v.panel_fill = palette.base;
    v.window_fill = palette.base;
    v.faint_bg_color = palette.faint;
    v.extreme_bg_color = palette.sunken;
    v.code_bg_color = palette.widget;
    v.warn_fg_color = palette.warn;
    v.error_fg_color = palette.error;
    v.hyperlink_color = palette.info_color();
    v.window_stroke.color = palette.border;
    v.selection.bg_fill = palette.primary;
    v.selection.stroke.color = palette.base;

    let rest_edge = mix(palette.border, palette.text, REST_EDGE_MIX);
    let hover_edge = mix(palette.border, palette.primary, HOVER_EDGE_MIX);
    let pressed = mix(palette.widget, palette.primary, PRESS_MIX);
    let paint = |wv: &mut WidgetVisuals, bg: Color32, weak: Color32, stroke: Color32| {
        wv.bg_fill = bg;
        wv.weak_bg_fill = weak;
        wv.bg_stroke = Stroke::new(1.0, stroke);
        wv.fg_stroke.color = palette.text;
    };
    let w = &mut v.widgets;
    paint(
        &mut w.noninteractive,
        palette.widget,
        palette.widget,
        rest_edge,
    );
    paint(&mut w.inactive, palette.widget, palette.widget, rest_edge);
    paint(&mut w.hovered, palette.widget, palette.widget, hover_edge);
    paint(&mut w.active, pressed, pressed, palette.primary);
    paint(&mut w.open, palette.faint, palette.faint, rest_edge);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: Palette = Palette {
        name: "Sample",
        primary: Color32::from_rgb(200, 80, 80),
        secondary: Color32::from_rgb(80, 80, 200),
        base: Color32::from_rgb(10, 10, 12),
        faint: Color32::from_rgb(16, 16, 20),
        sunken: Color32::from_rgb(30, 30, 36),
        widget: Color32::from_rgb(22, 22, 26),
        border: Color32::from_rgb(60, 60, 70),
        text: Color32::from_rgb(220, 220, 224),
        success: Color32::GREEN,
        warn: Color32::YELLOW,
        error: Color32::RED,
        info: None,
    };

    #[test]
    fn painting_replaces_colours_and_keeps_geometry() {
        let before = Style::default();
        let mut style = before.clone();
        paint_palette(&mut style, &SAMPLE);
        assert_eq!(style.visuals.panel_fill, SAMPLE.base);
        assert_eq!(style.visuals.selection.bg_fill, SAMPLE.primary);
        assert_eq!(style.visuals.hyperlink_color, SAMPLE.secondary);
        assert_ne!(style.visuals.widgets.active.weak_bg_fill, SAMPLE.widget);
        assert_eq!(style.visuals.widgets.active.bg_stroke.color, SAMPLE.primary);
        assert_eq!(style.spacing, before.spacing);
        assert_eq!(style.text_styles, before.text_styles);
    }

    #[test]
    fn every_state_gets_a_one_pixel_outline_apart_from_its_fill() {
        let mut style = Style::default();
        paint_palette(&mut style, &SAMPLE);
        let w = &style.visuals.widgets;
        for (name, state) in [
            ("noninteractive", &w.noninteractive),
            ("inactive", &w.inactive),
            ("hovered", &w.hovered),
            ("active", &w.active),
            ("open", &w.open),
        ] {
            assert_eq!(state.bg_stroke.width, 1.0, "{name}");
            assert_ne!(state.bg_stroke.color, state.bg_fill, "{name}");
        }
        assert_ne!(w.hovered.bg_stroke.color, w.inactive.bg_stroke.color);
    }

    #[test]
    fn an_explicit_info_colour_wins_over_the_secondary() {
        let palette = Palette {
            info: Some(Color32::from_rgb(1, 2, 3)),
            ..SAMPLE
        };
        let mut style = Style::default();
        paint_palette(&mut style, &palette);
        assert_eq!(style.visuals.hyperlink_color, Color32::from_rgb(1, 2, 3));
        assert_eq!(
            palette.semantic_colors(),
            (Color32::GREEN, palette.secondary)
        );
    }
}
