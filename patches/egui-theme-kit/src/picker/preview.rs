//! A miniature window painted from a theme's tokens.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::{
    glass::GlassParams,
    kit::ButtonKind,
    kit::{State, button_colors, selected_colors},
    presets,
    saved::SavedTheme,
    tokens::Tokens,
};

/// Height of a miniature, swatch strip included.
pub const PREVIEW_HEIGHT: f32 = 104.0;

const MARGIN: f32 = 8.0;
const TITLE_BAR_H: f32 = 13.0;
const ROW_H: f32 = 12.0;
const SWATCH_H: f32 = 8.0;
const TEXT_PT: f32 = 8.5;
const MAX_RADIUS: u8 = 8;

/// What a miniature shows.
pub struct Preview<'a> {
    pub tokens: &'a Tokens,
    /// The glass material the theme asks for; a visible one tints the backdrop and thins the window.
    pub glass: GlassParams,
    /// Opacity of the window fill, 1.0 for opaque themes.
    pub window_opacity: f32,
    pub title: &'a str,
}

impl<'a> Preview<'a> {
    pub fn new(
        tokens: &'a Tokens,
        glass: GlassParams,
        window_opacity: f32,
        title: &'a str,
    ) -> Self {
        Self {
            tokens,
            glass,
            window_opacity,
            title,
        }
    }
}

/// Window opacity a theme's style asks for, clamped to a range that stays legible in a miniature.
pub fn window_opacity(theme: &SavedTheme) -> f32 {
    (f32::from(theme.style.visuals.window_fill.a()) / 255.0).clamp(0.55, 1.0)
}

/// Tokens a saved theme publishes: its preset's hand-tuned ones, else derived from its style.
pub fn theme_tokens(theme: &SavedTheme) -> Tokens {
    let normal = presets::normalized(&theme.style);
    let def = presets::exact_preset(&normal);
    let semantic = theme.semantic.unwrap_or_else(|| match def {
        Some(def) => def.semantic(),
        None => crate::saved::SemanticColors {
            success: crate::colors::DEFAULT_SUCCESS,
            secondary: crate::colors::DEFAULT_ACCENT_SECONDARY,
        },
    });
    match def.and_then(|d| d.tokens) {
        Some(tokens) if theme.semantic.is_none() => tokens(),
        _ => Tokens::derive(&theme.style, semantic.success, semantic.secondary),
    }
}

/// Paints the miniature into `rect`.
pub fn paint(ui: &Ui, rect: Rect, preview: &Preview<'_>) {
    let t = preview.tokens;
    let painter = ui.painter().with_clip_rect(rect);
    let radius = t.radius_md.min(MAX_RADIUS + 2);
    painter.rect_filled(rect, radius, t.base);

    let glassy = preview.glass.is_visible();
    if glassy {
        paint_backdrop(&painter, rect, t);
    }

    let window = Rect::from_min_max(
        rect.min + vec2(MARGIN, MARGIN),
        pos2(
            rect.right() - MARGIN,
            rect.bottom() - MARGIN - SWATCH_H - 6.0,
        ),
    );
    let window_radius = t.radius_lg.min(MAX_RADIUS);
    let fill = if glassy {
        t.window.gamma_multiply(preview.window_opacity)
    } else {
        t.window
    };
    painter.rect_filled(window, window_radius, fill);

    let bar = Rect::from_min_size(window.min, vec2(window.width(), TITLE_BAR_H));
    let top = CornerRadius {
        nw: window_radius,
        ne: window_radius,
        sw: 0,
        se: 0,
    };
    let bar_fill = if glassy {
        t.raised.gamma_multiply(preview.window_opacity)
    } else {
        t.raised
    };
    painter.rect_filled(bar, top, bar_fill);
    let font = FontId::proportional(TEXT_PT / crate::fonts::text_scale(ui.ctx()).factor());
    painter.text(
        bar.left_center() + vec2(6.0, 0.0),
        Align2::LEFT_CENTER,
        preview.title,
        font.clone(),
        t.text,
    );
    for i in 0..3 {
        let x = bar.right() - 8.0 - i as f32 * 7.0;
        painter.circle_filled(pos2(x, bar.center().y), 1.8, t.text_dim);
    }
    painter.rect_stroke(
        window,
        window_radius,
        Stroke::new(1.0, t.card_border),
        StrokeKind::Inside,
    );

    paint_body(&painter, window, bar.bottom(), t, &font);
    paint_swatches(&painter, rect, t);
}

/// Blurry-looking colour blobs behind a glass window.
fn paint_backdrop(painter: &eframe::egui::Painter, rect: Rect, t: &Tokens) {
    let h = rect.height();
    for (i, color) in [t.primary, t.secondary, t.tertiary].into_iter().enumerate() {
        let x = rect.left() + rect.width() * (0.2 + 0.3 * i as f32);
        let y = rect.top() + h * (0.35 + 0.18 * (i % 2) as f32);
        painter.circle_filled(pos2(x, y), h * 0.30, color.gamma_multiply(0.55));
    }
}

fn paint_body(painter: &eframe::egui::Painter, window: Rect, top: f32, t: &Tokens, font: &FontId) {
    let inset = 6.0;
    let x0 = window.left() + inset;
    let x1 = window.right() - inset;
    let mut y = top + 5.0;
    let small = t.radius_sm.min(4);

    let field = Rect::from_min_max(pos2(x0, y), pos2(x0 + (x1 - x0) * 0.56, y + ROW_H));
    painter.rect_filled(field, small, t.field);
    painter.rect_stroke(
        field,
        small,
        Stroke::new(1.0, t.field_border),
        StrokeKind::Inside,
    );
    let label = painter.text(
        field.left_center() + vec2(4.0, 0.0),
        Align2::LEFT_CENTER,
        "Field",
        font.clone(),
        t.text,
    );
    let caret_x = (label.right() + 2.0).round() + 0.5;
    painter.vline(
        caret_x,
        field.y_range().shrink(3.0),
        Stroke::new(1.0, t.primary),
    );

    let neutral = button_colors(t, ButtonKind::Neutral, State::Rest);
    let primary = button_colors(t, ButtonKind::Primary, State::Rest);
    let mut right = x1;
    for (text, (fill, edge, ink)) in [("Save", primary), ("Cancel", neutral)] {
        let width = text_width(painter, text, font) + 10.0;
        let button = Rect::from_min_max(pos2(right - width, y), pos2(right, y + ROW_H));
        painter.rect_filled(button, small, fill);
        if edge != Color32::TRANSPARENT {
            painter.rect_stroke(button, small, Stroke::new(1.0, edge), StrokeKind::Inside);
        }
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            text,
            font.clone(),
            ink,
        );
        right = button.left() - 4.0;
    }
    y += ROW_H + 4.0;

    let (select_fill, select_edge, select_ink) = selected_colors(t);
    let row = Rect::from_min_max(
        pos2(x0, y),
        pos2(x1, (y + ROW_H).min(window.bottom() - 3.0)),
    );
    if row.height() > 4.0 {
        painter.rect_filled(row, small, select_fill);
        painter.rect_stroke(
            row,
            small,
            Stroke::new(1.0, select_edge),
            StrokeKind::Inside,
        );
        painter.text(
            row.left_center() + vec2(5.0, 0.0),
            Align2::LEFT_CENTER,
            "Selected row",
            font.clone(),
            select_ink,
        );
        painter.circle_filled(pos2(row.right() - 7.0, row.center().y), 2.2, t.primary);
    }
    y += ROW_H + 4.0;
    paint_status_row(
        painter,
        Rect::from_min_max(pos2(x0, y), pos2(x1, y + ROW_H)),
        t,
        font,
    );
}

/// Body and dim text beside a progress bar.
fn paint_status_row(painter: &eframe::egui::Painter, row: Rect, t: &Tokens, font: &FontId) {
    let text = painter.text(
        row.left_center() + vec2(3.0, 0.0),
        Align2::LEFT_CENTER,
        "Text",
        font.clone(),
        t.text,
    );
    painter.text(
        pos2(text.right() + 6.0, row.center().y),
        Align2::LEFT_CENTER,
        "Dim",
        font.clone(),
        t.text_dim,
    );
    let track = Rect::from_min_max(
        pos2(row.left() + row.width() * 0.52, row.center().y - 2.5),
        pos2(row.right() - 2.0, row.center().y + 2.5),
    );
    painter.rect_filled(track, 3, t.sunken);
    let fill = Rect::from_min_size(track.min, vec2(track.width() * 0.62, track.height()));
    painter.rect_filled(fill, 3, t.primary);
}

fn text_width(painter: &eframe::egui::Painter, text: &str, font: &FontId) -> f32 {
    painter
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x
}

/// The seven swatches of [`Tokens::swatches`] along the foot of `rect`.
fn paint_swatches(painter: &eframe::egui::Painter, rect: Rect, t: &Tokens) {
    let swatches = t.swatches();
    let gap = 2.0;
    let width = rect.width() - 2.0 * MARGIN;
    let each = (width - gap * (swatches.len() - 1) as f32) / swatches.len() as f32;
    let y = rect.bottom() - MARGIN - SWATCH_H + 2.0;
    for (i, color) in swatches.into_iter().enumerate() {
        let x = rect.left() + MARGIN + i as f32 * (each + gap);
        let cell = Rect::from_min_size(pos2(x, y), vec2(each, SWATCH_H));
        painter.rect_filled(cell, 2, color);
        painter.rect_stroke(
            cell,
            2,
            Stroke::new(1.0, t.card_border.gamma_multiply(0.8)),
            StrokeKind::Inside,
        );
    }
}

/// A strip of `colors` in `rect`, for rows that have no room for a miniature.
pub fn paint_strip(ui: &Ui, rect: Rect, colors: &[Color32], outline: Color32) {
    let painter = ui.painter();
    let gap = 1.0;
    let each =
        (rect.width() - gap * (colors.len().saturating_sub(1)) as f32) / colors.len().max(1) as f32;
    for (i, color) in colors.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(rect.left() + i as f32 * (each + gap), rect.top()),
            vec2(each, rect.height()),
        );
        painter.rect_filled(cell, 1, *color);
    }
    painter.rect_stroke(
        rect.expand(0.5),
        2,
        Stroke::new(1.0, outline),
        StrokeKind::Outside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{PresetStyles, apply_preset};
    use eframe::egui::{Context, Pos2, RawInput};

    fn frame(ctx: &Context, f: impl FnMut(&mut Ui)) {
        let mut f = f;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 400.0))),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| f(ui)).drop_without_applying_deltas();
    }

    #[test]
    fn theme_tokens_match_what_applying_the_theme_publishes() {
        let ctx = Context::default();
        for def in presets::builtin_presets() {
            apply_preset(&ctx, def);
            let theme = SavedTheme::current(&ctx);
            assert_eq!(theme_tokens(&theme), Tokens::get(&ctx), "{}", def.name);
        }
    }

    #[test]
    fn a_custom_theme_derives_its_tokens_from_its_style_and_semantic_colours() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Aurora);
        let mut theme = SavedTheme::current(&ctx);
        theme.style.visuals.panel_fill = Color32::from_rgb(33, 22, 11);
        theme.semantic = Some(crate::saved::SemanticColors {
            success: Color32::from_rgb(1, 2, 3),
            secondary: Color32::from_rgb(4, 5, 6),
        });
        let t = theme_tokens(&theme);
        assert_eq!(t.base, Color32::from_rgb(33, 22, 11));
        assert_eq!(t.success, Color32::from_rgb(1, 2, 3));
        assert_eq!(t.secondary, Color32::from_rgb(4, 5, 6));
    }

    #[test]
    fn window_opacity_follows_the_fill_alpha_within_bounds() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Noir);
        assert_eq!(window_opacity(&SavedTheme::current(&ctx)), 1.0);
        apply_preset(&ctx, PresetStyles::NoirGlass);
        let glass = window_opacity(&SavedTheme::current(&ctx));
        assert!((0.55..1.0).contains(&glass));
    }

    #[test]
    fn a_miniature_paints_for_every_preset_without_panicking() {
        let ctx = Context::default();
        crate::presets::bootstrap_startup_theme(&ctx);
        for def in presets::builtin_presets() {
            let theme = def.theme();
            let tokens = theme_tokens(&theme);
            frame(&ctx, |ui| {
                let (rect, _) = ui
                    .allocate_exact_size(vec2(180.0, PREVIEW_HEIGHT), eframe::egui::Sense::hover());
                let preview = Preview::new(&tokens, def.glass(), window_opacity(&theme), def.name);
                paint(ui, rect, &preview);
                paint_strip(ui, rect, &tokens.swatches(), tokens.card_border);
            });
        }
    }
}
