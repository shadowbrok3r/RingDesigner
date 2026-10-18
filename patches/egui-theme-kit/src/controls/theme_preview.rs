//! A mock window painted from a theme's tokens.

use eframe::egui::{
    CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, TextStyle, Ui, Vec2, pos2, vec2,
};

use super::fields::{FIELD_PAD_X, ICON_GAP};
use super::labels::{badge_layout, paint_badge};
use super::{
    ButtonKind, Metrics, State, Tokens, button_colors, elided, icons, kit, medium_font, paint_box,
    selected_colors, semibold_font_sized, text_galley, whole,
};
use crate::metrics::type_scale;

/// Width of a [`theme_preview`].
pub const THEME_PREVIEW_W: f32 = 240.0;

/// Text of the mock field.
const FIELD_TEXT: &str = "Field text";
/// Labels of the mock list rows, selected first.
const ROWS: [&str; 2] = ["Selected row", "Row"];
/// Labels of the neutral and primary mock buttons.
const BUTTONS: [&str; 2] = ["Cancel", "Save"];
/// Labels of the success, warning and error badges.
const BADGES: [&str; 3] = ["Passed", "Warning", "Failed"];

/// A mock window in `t`'s colours titled `name`, with a field, a selected row, buttons and badges.
pub fn theme_preview(ui: &mut Ui, t: &Tokens, name: &str) -> Response {
    let (_, m) = kit(ui);
    let badge_h = badge_layout(ui, BADGES[0]).1.y;
    let size = vec2(THEME_PREVIEW_W, preview_h(&m, badge_h));
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_preview(ui, rect, t, &m, name, badge_h);
    }
    response
}

/// Height of a [`theme_preview`] whose badges are `badge_h` tall.
fn preview_h(m: &Metrics, badge_h: f32) -> f32 {
    let body = 2.0 * m.gap.x + 2.0 * m.control_h + 2.0 * m.row_h + badge_h + 3.0 * m.gap.y;
    m.header_h + body
}

fn paint_preview(ui: &Ui, rect: Rect, t: &Tokens, m: &Metrics, name: &str, badge_h: f32) {
    let painter = ui.painter();
    painter.rect_filled(rect, t.radius_lg, t.base);
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), m.header_h));
    let top = CornerRadius {
        nw: t.radius_lg,
        ne: t.radius_lg,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(bar, top, t.raised);
    let rule = painter.round_to_pixel_center(bar.bottom() - 0.5);
    painter.hline(bar.x_range(), rule, Stroke::new(1.0, t.separator));
    paint_title(ui, bar, t, m, name);

    let inset = m.gap.x;
    let width = rect.width() - 2.0 * inset;
    let mut y = bar.bottom() + inset;
    let mut block = |h: f32| {
        let r = Rect::from_min_size(pos2(rect.left() + inset, y), vec2(width, h));
        y = r.bottom() + m.gap.y;
        r
    };
    paint_field(ui, block(m.control_h), t, m);
    paint_rows(ui, block(2.0 * m.row_h), t, m);
    paint_buttons(ui, block(m.control_h), t, m);
    paint_badges(ui, block(badge_h), t, m);

    painter.rect_stroke(
        rect,
        t.radius_lg,
        Stroke::new(1.0, t.card_border),
        StrokeKind::Inside,
    );
}

/// The grid icon, `name` and the window controls along `bar`.
fn paint_title(ui: &Ui, bar: Rect, t: &Tokens, m: &Metrics, name: &str) {
    let left = bar.left() + m.gap.x;
    let icon = Rect::from_min_size(
        pos2(left, bar.center().y - m.icon_md / 2.0),
        Vec2::splat(m.icon_md),
    );
    icons::GRID.paint(ui, icon, m.icon_md, t.primary);
    let controls_w = 2.0 * m.icon_sm + m.gap.x;
    let controls_left = bar.right() - m.gap.x - controls_w;
    for (i, glyph) in [icons::MINUS, icons::X].into_iter().enumerate() {
        let x = controls_left + i as f32 * (m.icon_sm + m.gap.x);
        let slot = Rect::from_min_size(
            pos2(x, bar.center().y - m.icon_sm / 2.0),
            Vec2::splat(m.icon_sm),
        );
        glyph.paint(ui, slot, m.icon_sm, t.text_dim);
    }
    let text_left = icon.right() + ICON_GAP;
    let max_w = controls_left - m.gap.x - text_left;
    let font = semibold_font_sized(ui, type_scale::BODY);
    let galley = elided(ui, name, font, max_w);
    let y = (bar.center().y - galley.size().y / 2.0).round();
    ui.painter().galley(pos2(text_left, y), galley, t.text);
}

/// A resting text field holding [`FIELD_TEXT`] and a caret.
fn paint_field(ui: &Ui, rect: Rect, t: &Tokens, m: &Metrics) {
    paint_box(ui, rect, t.radius_sm, t.field, t.field_border);
    let icon_x = rect.left() + FIELD_PAD_X;
    let icon = Rect::from_min_size(
        pos2(icon_x, rect.center().y - m.icon_sm / 2.0),
        Vec2::splat(m.icon_sm),
    );
    icons::SEARCH.paint(ui, icon, m.icon_sm, t.text_faint);
    let galley = text_galley(ui, FIELD_TEXT, TextStyle::Body.resolve(ui.style()));
    let text = Rect::from_min_size(
        pos2(
            icon.right() + ICON_GAP,
            (rect.center().y - galley.size().y / 2.0).round(),
        ),
        galley.size(),
    );
    ui.painter().galley(text.min, galley, t.text);
    let caret_x = ui.painter().round_to_pixel_center(text.right() + 1.0);
    ui.painter()
        .vline(caret_x, text.y_range(), Stroke::new(1.0, t.primary));
}

/// A selected list row over a resting one.
fn paint_rows(ui: &Ui, rect: Rect, t: &Tokens, m: &Metrics) {
    let (selected_fill, _, selected_ink) = selected_colors(t);
    for (i, label) in ROWS.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(rect.left(), rect.top() + i as f32 * m.row_h),
            vec2(rect.width(), m.row_h),
        );
        let selected = i == 0;
        if selected {
            ui.painter().rect_filled(row, t.radius_sm, selected_fill);
        }
        let icon = Rect::from_min_size(
            pos2(row.left() + m.gap.x, row.center().y - m.icon_md / 2.0),
            Vec2::splat(m.icon_md),
        );
        let icon_ink = if selected { t.primary } else { t.icon };
        icons::FOLDER.paint(ui, icon, m.icon_md, icon_ink);
        let galley = text_galley(ui, label, TextStyle::Body.resolve(ui.style()));
        let y = (row.center().y - galley.size().y / 2.0).round();
        let ink = if selected { selected_ink } else { t.text };
        ui.painter()
            .galley(pos2(icon.right() + ICON_GAP, y), galley, ink);
        if selected {
            let check = Rect::from_min_size(
                pos2(
                    row.right() - m.gap.x - m.icon_sm,
                    row.center().y - m.icon_sm / 2.0,
                ),
                Vec2::splat(m.icon_sm),
            );
            icons::CHECK.paint(ui, check, m.icon_sm, t.primary);
        }
    }
}

/// Neutral and primary buttons at the right of `rect`.
fn paint_buttons(ui: &Ui, rect: Rect, t: &Tokens, m: &Metrics) {
    let font = medium_font(ui, TextStyle::Button);
    let mut right = rect.right();
    for (label, kind) in BUTTONS
        .into_iter()
        .zip([ButtonKind::Neutral, ButtonKind::Primary])
        .rev()
    {
        let galley = text_galley(ui, label, font.clone());
        let w = whole(galley.size().x + 2.0 * m.pad.x);
        let button = Rect::from_min_size(pos2(right - w, rect.top()), vec2(w, rect.height()));
        let (fill, stroke, ink) = button_colors(t, kind, State::Rest);
        paint_box(ui, button, t.radius_sm, fill, stroke);
        let pos = (button.center() - galley.size() / 2.0).round();
        ui.painter().galley(pos, galley, ink);
        right = button.left() - m.gap.x;
    }
}

/// Success, warning and error badges from the left of `rect`.
fn paint_badges(ui: &Ui, rect: Rect, t: &Tokens, m: &Metrics) {
    let mut x = rect.left();
    for (label, tone) in BADGES.into_iter().zip([t.success, t.warn, t.error]) {
        let (galley, size) = badge_layout(ui, label);
        let badge = Rect::from_min_size(pos2(x, rect.top()), size);
        paint_badge(ui, badge, galley, tone);
        x = badge.right() + m.gap.x;
    }
}
