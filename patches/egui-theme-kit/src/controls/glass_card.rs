//! Themed card surfaces that frost their own backdrop on glass presets.

use eframe::egui::{
    Align, AsIdSalt, Color32, CornerRadius, Frame, InnerResponse, Layout, Margin, Rangef, Rect,
    Response, RichText, Sense, Shape, Stroke, Ui, UiBuilder, UiKind, UiStackInfo,
    epaint::MarginF32, layers::ShapeIdx, pos2, vec2,
};

use super::{Icon, hairline, subheading};
use crate::{colors, glass, tokens};

/// Lower bound of the card padding.
const CARD_PAD_MIN: i8 = 6;
/// Upper bound of the card padding.
const CARD_PAD_MAX: i8 = 10;
/// Vertical gap between stacked cards.
const CARD_GAP: i8 = 3;
/// Smallest card corner radius.
const MIN_RADIUS: u8 = 3;
/// Darkening share of a recessed panel on dark themes.
const RECESS_DARK: f32 = 0.72;
/// Lightening share of a recessed panel on light themes.
const RECESS_LIGHT: f32 = 0.28;

/// Card padding from the theme's window margin.
fn card_padding(ui: &Ui) -> i8 {
    ui.spacing()
        .window_margin
        .left
        .clamp(CARD_PAD_MIN, CARD_PAD_MAX)
}

/// Translucent window fill on glass themes, otherwise the faint tint over the panel.
pub fn card_fill(ui: &Ui) -> Color32 {
    let v = ui.visuals();
    if v.window_fill.a() < 255 {
        return v.window_fill;
    }
    let lifted = tokens::composite(v.faint_bg_color, v.panel_fill);
    if lifted == v.panel_fill {
        tokens::composite(v.widgets.noninteractive.weak_bg_fill, v.panel_fill)
    } else {
        lifted
    }
}

/// Panel fill pushed away from the card fill so cards read as raised.
pub fn recessed_fill(ui: &Ui) -> Color32 {
    let v = ui.visuals();
    let [r, g, b, a] = v.panel_fill.to_srgba_unmultiplied();
    let shift = |c: u8| {
        if v.dark_mode {
            (c as f32 * RECESS_DARK).round() as u8
        } else {
            (c as f32 + (255.0 - c as f32) * RECESS_LIGHT).round() as u8
        }
    };
    Color32::from_rgba_unmultiplied(shift(r), shift(g), shift(b), a)
}

/// The window stroke, or the noninteractive widget edge when that is invisible.
pub fn card_stroke(ui: &Ui) -> Stroke {
    let v = ui.visuals();
    if v.window_stroke.width > 0.0 && v.window_stroke.color.a() > 0 {
        v.window_stroke
    } else {
        v.widgets.noninteractive.bg_stroke
    }
}

/// A [`Frame`] shaped like a card; a bare frame does not frost.
pub fn card_frame(ui: &Ui) -> Frame {
    let radius = ui.visuals().window_corner_radius.nw.max(MIN_RADIUS);
    Frame::new()
        .fill(card_fill(ui))
        .stroke(card_stroke(ui))
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(Margin::same(card_padding(ui)))
        .outer_margin(Margin::symmetric(0, CARD_GAP))
}

/// A card that frosts its backdrop; `id_salt` must be unique among the cards of one `Ui`.
pub fn card<R>(
    ui: &mut Ui,
    id_salt: impl AsIdSalt,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let frame = card_frame(ui);
    glass::glass_frame(ui, id_salt, frame, contents)
}

/// A card without frost, for rows that repeat.
pub fn group<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    card_frame(ui).show(ui, contents)
}

/// A [`group`] exactly `ui.available_width()` wide whatever it holds; plain while unbounded.
pub fn row_card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let width = ui.available_width();
    if ui.is_sizing_pass() || !width.is_finite() {
        return group(ui, contents);
    }
    let frame = card_frame(ui);
    let mut card = FixedWidthFrame::begin(ui, frame, width);
    let inner = contents(&mut card.content_ui);
    InnerResponse::new(inner, card.end(ui))
}

/// A [`Frame`] of fixed outer width; wider content is clipped instead of widening its parent.
pub struct FixedWidthFrame {
    /// Fill, stroke and shadow are read when painting; margins are fixed by [`Self::begin`].
    pub frame: Frame,
    /// The `Ui` the frame's contents go in.
    pub content_ui: Ui,
    background: ShapeIdx,
    outer_left: f32,
    outer_top: f32,
    outer_width: f32,
    margin: MarginF32,
    outer_margin: Margin,
}

impl FixedWidthFrame {
    /// Starts `frame` at the cursor, `width` wide including its outer margin.
    pub fn begin(ui: &mut Ui, frame: Frame, width: f32) -> Self {
        debug_assert!(
            width.is_finite(),
            "a fixed-width frame needs a finite width"
        );
        let background = ui.painter().add(Shape::Noop);
        let available = ui.available_rect_before_wrap();
        let outer = Rect::from_min_size(available.min, vec2(width.max(0.0), available.height()));
        let margin = frame.total_margin();
        let mut content = outer - margin;
        content.max.x = content.max.x.max(content.min.x);
        content.max.y = content.max.y.max(content.min.y);

        let mut content_ui = ui.new_child(
            UiBuilder::new()
                .ui_stack_info(UiStackInfo::new(UiKind::Frame).with_frame(frame))
                .max_rect(content),
        );
        content_ui.set_width(content.width());
        let visible = outer - frame.outer_margin;
        content_ui.shrink_clip_rect(Rect::from_x_y_ranges(visible.x_range(), Rangef::EVERYTHING));

        Self {
            frame,
            content_ui,
            background,
            outer_left: outer.left(),
            outer_top: outer.top(),
            outer_width: outer.width(),
            margin,
            outer_margin: frame.outer_margin,
        }
    }

    /// The locked width by the content's height plus the margins.
    pub fn outer_rect(&self) -> Rect {
        let content_top = self.outer_top + self.margin.top;
        let bottom = self.content_ui.min_rect().bottom().max(content_top) + self.margin.bottom;
        Rect::from_min_max(
            pos2(self.outer_left, self.outer_top),
            pos2(self.outer_left + self.outer_width, bottom),
        )
    }

    /// Allocates [`Self::outer_rect`] in `ui`.
    pub fn allocate_space(&self, ui: &mut Ui) -> Response {
        ui.allocate_rect(self.outer_rect(), Sense::hover())
    }

    /// Paints the frame behind the contents.
    pub fn paint(&self, ui: &Ui) {
        let visible = self.outer_rect() - self.outer_margin;
        if ui.is_rect_visible(visible) {
            let content = (visible - self.frame.inner_margin).shrink(self.frame.stroke.width);
            ui.painter().set(self.background, self.frame.paint(content));
        }
    }

    /// Paints and allocates the frame.
    pub fn end(self, ui: &mut Ui) -> Response {
        self.paint(ui);
        self.allocate_space(ui)
    }
}

/// A frosting card opening with an icon, a title and an optional right-aligned subtitle.
pub fn titled_card<R>(
    ui: &mut Ui,
    icon: impl Into<Option<Icon>>,
    title: &str,
    subtitle: Option<&str>,
    contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let icon = icon.into();
    let salt = ui.next_auto_id();
    ui.skip_ahead_auto_ids(1);
    card(ui, salt, |ui| {
        ui.set_min_width(ui.available_width());
        card_header(ui, icon, title, subtitle);
        contents(ui)
    })
    .inner
}

/// A non-frosting [`group`] card opening with an icon, a title and an optional subtitle.
pub fn titled_group<R>(
    ui: &mut Ui,
    icon: impl Into<Option<Icon>>,
    title: &str,
    subtitle: Option<&str>,
    contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let icon = icon.into();
    group(ui, |ui| {
        ui.set_min_width(ui.available_width());
        card_header(ui, icon, title, subtitle);
        contents(ui)
    })
    .inner
}

/// The icon and title row a titled card opens with, then a hairline.
pub fn card_header(
    ui: &mut Ui,
    icon: impl Into<Option<Icon>>,
    title: &str,
    subtitle: Option<&str>,
) {
    let icon = icon.into();
    ui.horizontal(|ui| {
        subheading(ui, icon, title);
        if let Some(subtitle) = subtitle {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(subtitle).small().color(colors::weak_text(ui)));
            });
        }
    });
    hairline(ui);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{MATCHABLE_PRESETS, PresetStyles, style_for_preset};
    use eframe::egui::Context;

    fn with_preset<R>(preset: PresetStyles, f: impl FnOnce(&mut Ui) -> R) -> R {
        let ctx = Context::default();
        ctx.set_global_style(std::sync::Arc::new(style_for_preset(preset)));
        let mut once = Some(f);
        let mut out = None;
        let mut full = ctx.run_ui(Default::default(), |ui| {
            if let Some(f) = once.take() {
                out = Some(f(ui));
            }
        });
        full.textures_delta.clear();
        out.expect("the ui closure ran")
    }

    #[test]
    fn a_card_never_disappears_into_its_panel() {
        for preset in MATCHABLE_PRESETS {
            let (fill, panel, stroke) = with_preset(preset, |ui| {
                (card_fill(ui), ui.visuals().panel_fill, card_stroke(ui))
            });
            let separated = fill != panel
                || (stroke.width > 0.0 && stroke.color.a() > 0 && stroke.color != panel);
            assert!(
                separated,
                "{}: a card is indistinguishable from its panel (fill {fill:?}, panel {panel:?})",
                preset.as_str(),
            );
        }
    }

    #[test]
    fn a_fixed_width_frame_keeps_its_width_whatever_it_holds() {
        let (parent, rects) = with_preset(MATCHABLE_PRESETS[0], |ui| {
            let mut column = ui.new_child(
                UiBuilder::new()
                    .max_rect(Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, 600.0))),
            );
            let rects: Vec<Rect> = [40.0, 900.0, 120.0]
                .into_iter()
                .map(|content_w| {
                    let frame =
                        card_frame(&column).stroke(Stroke::new(1.0, card_stroke(&column).color));
                    let width = column.available_width();
                    let mut card = FixedWidthFrame::begin(&mut column, frame, width);
                    card.content_ui
                        .allocate_exact_size(vec2(content_w, 20.0), Sense::hover());
                    card.frame.stroke = Stroke::new(3.0, card.frame.stroke.color);
                    card.end(&mut column).rect
                })
                .collect();
            (column.min_rect(), rects)
        });
        for rect in &rects {
            assert_eq!((rect.left(), rect.right()), (10.0, 310.0), "card {rect:?}");
        }
        assert_eq!(parent.right(), 310.0, "a wide card widened its column");
    }

    #[test]
    fn a_glass_preset_keeps_its_cards_translucent() {
        let glass: Vec<_> = MATCHABLE_PRESETS
            .into_iter()
            .filter(|p| p.as_str().contains("Glass"))
            .collect();
        assert!(!glass.is_empty(), "no glass preset to check");
        for preset in glass {
            let fill = with_preset(preset, |ui| card_fill(ui));
            assert!(fill.a() < 255, "{}: card fill is opaque", preset.as_str());
        }
    }
}
