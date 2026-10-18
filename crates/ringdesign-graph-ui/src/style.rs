//! Graph geometry and shared Mastertech chrome.

use egui::{Color32, Stroke};
use egui_snarl::ui::{BackgroundPattern, NodeLayout, PinInfo, PinPlacement, SnarlStyle, WireStyle};
use ringdesign_graph::graph::Access;
use ringdesign_graph::value::ValueKind;

use egui_mobile_theme::palette;

pub const PINK: Color32 = palette::ACCENT;
pub const PINK_BRIGHT: Color32 = palette::LINK;
pub const AQUA: Color32 = palette::INFO;
pub const AQUA_BRIGHT: Color32 = palette::INFO;
pub const VIOLET: Color32 = palette::HOVER;
pub const RIM: Color32 = palette::BORDER;
pub const RIM_BRIGHT: Color32 = palette::BORDER;
pub const INK: Color32 = palette::TEXT;
pub const INK_DIM: Color32 = palette::TEXT_DIM;
pub const ERROR: Color32 = palette::ERROR;
pub const CANVAS: Color32 = palette::PANEL;
pub const NODE_FILL: Color32 = palette::WINDOW;
pub const NODE_CORNER: f32 = 5.0;
/// Dot grid spacing and radius in graph units, so the grid scales with the nodes.
pub const DOT_SPACING: f32 = 28.0;
pub const DOT_RADIUS: f32 = 1.7;
/// Quiet neutral ink for the dot grid.
pub const DOT_COLOR: Color32 = Color32::from_rgb(35, 35, 43);
pub const MIN_SCALE: f32 = 0.05;
pub const MAX_SCALE: f32 = 2.5;
/// Graph-space width of a node's field row. A width taken from
/// `available_width` feeds back into the node size it is derived from and
/// ratchets the node wider every frame; a constant does not.
pub const NODE_FIELD_W: f32 = 260.0;

/// The snarl settings: orthogonal wires with rounded corners, pins outside
/// the body, inputs above outputs, no built-in pattern (the canvas is drawn
/// by [`paint_canvas`]).
pub fn snarl_style() -> SnarlStyle {
    let mut s = SnarlStyle::new();
    s.bg_frame = Some(egui::Frame::new().fill(CANVAS));
    s.bg_pattern = Some(BackgroundPattern::NoPattern);
    s.min_scale = Some(MIN_SCALE);
    s.max_scale = Some(MAX_SCALE);
    s.centering = Some(true);
    s.wire_style = Some(WireStyle::AxisAligned { corner_radius: 8.0 });
    s.wire_width = Some(2.6);
    s.pin_placement = Some(PinPlacement::Outside { margin: 3.0 });
    s.pin_size = Some(15.0);
    s.node_layout = Some(NodeLayout::sandwich());
    s
}

/// A node's frame: glass over the canvas, a hairline rim at rest, pink when
/// chosen, the error colour when it carries diagnostics.
pub fn node_frame(default: egui::Frame, selected: bool, trouble: bool) -> egui::Frame {
    let frame = default.fill(NODE_FILL).corner_radius(NODE_CORNER);
    if trouble {
        frame.stroke(Stroke::new(2.0, ERROR))
    } else if selected {
        frame.stroke(Stroke::new(2.0, PINK))
    } else {
        frame.stroke(Stroke::new(1.0, RIM_BRIGHT))
    }
}

/// A pin's colour: the kind's own hue at the palette's muted saturation and
/// value; the kinds without a hue stay grey.
pub fn pin_color(kind: ValueKind) -> Color32 {
    let mut h = egui::ecolor::Hsva::from(crate::widgets::kind_color(kind));
    if h.s < 0.15 {
        return Color32::from_gray(110);
    }
    h.s = 0.5;
    h.v = 0.5;
    h.a = 1.0;
    h.into()
}

/// A pin: a circle for an item, a square for a list, filled with its kind's colour.
pub fn pin_info(kind: ValueKind, access: Access) -> PinInfo {
    let info = match access {
        Access::Item => PinInfo::circle(),
        Access::List => PinInfo::square(),
    };
    info.with_fill(pin_color(kind))
}

/// The dot grid behind the nodes. `viewport` is the
/// visible rect in graph space and `scale` the view's zoom.
pub fn paint_canvas(painter: &egui::Painter, viewport: egui::Rect, scale: f32) {
    if !viewport.is_finite() {
        return;
    }
    let scale = scale.max(0.001);
    let mut spacing = DOT_SPACING;
    while spacing * scale < 26.0 {
        spacing *= 2.0;
    }
    let min_x = (viewport.min.x / spacing).floor() as i64;
    let max_x = (viewport.max.x / spacing).ceil() as i64;
    let min_y = (viewport.min.y / spacing).floor() as i64;
    let max_y = (viewport.max.y / spacing).ceil() as i64;
    if (max_x - min_x).saturating_mul(max_y - min_y) > 6500 {
        return;
    }
    for xi in min_x..=max_x {
        for yi in min_y..=max_y {
            painter.circle_filled(
                egui::pos2(xi as f32 * spacing, yi as f32 * spacing),
                DOT_RADIUS,
                DOT_COLOR,
            );
        }
    }
}

/// Use the host's density and the same Mastertech widget palette inside nodes.
pub fn apply_visuals(style: &mut egui::Style) {
    // Keep the live host palette, including edits from its Theme tab.
    // Nodes must not shrink through feedback from their previous frame's width.
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_style_is_the_comfyui_one() {
        let s = snarl_style();
        assert!(
            matches!(s.wire_style, Some(WireStyle::AxisAligned { corner_radius }) if corner_radius == 8.0)
        );
        assert!(matches!(s.pin_placement, Some(PinPlacement::Outside { margin }) if margin == 3.0));
        assert_eq!(
            (s.pin_size, s.wire_width, s.min_scale, s.max_scale),
            (Some(15.0), Some(2.6), Some(MIN_SCALE), Some(MAX_SCALE))
        );
        assert!(matches!(s.bg_pattern, Some(BackgroundPattern::NoPattern)));
        assert!(s.node_layout.is_some());
        assert_eq!(s.bg_frame.unwrap().fill, CANVAS);
    }

    #[test]
    fn pins_keep_their_kind_but_wear_the_palette() {
        assert_ne!(pin_color(ValueKind::Number), pin_color(ValueKind::Layer));
        let h = egui::ecolor::Hsva::from(pin_color(ValueKind::Design));
        assert!(
            (h.s - 0.5).abs() < 0.05 && (h.v - 0.5).abs() < 0.05,
            "{h:?}"
        );
        assert_eq!(pin_color(ValueKind::Any), Color32::from_gray(110));
    }

    #[test]
    fn frames_read_their_state() {
        let d = egui::Frame::new();
        assert_eq!(node_frame(d, true, false).stroke.color, PINK);
        assert_eq!(
            node_frame(d, true, true).stroke.color,
            ERROR,
            "trouble outranks selection"
        );
        let rest = node_frame(d, false, false);
        assert_eq!((rest.stroke.color, rest.fill), (RIM_BRIGHT, NODE_FILL));
    }

    #[test]
    fn the_visuals_share_the_mastertech_palette() {
        let mut style = egui_mobile_theme::desktop_style();
        apply_visuals(&mut style);
        let v = &style.visuals;
        assert_eq!(v.text_color(), INK);
        assert_eq!(
            v.selection.bg_fill,
            egui_mobile_theme::desktop_style().visuals.selection.bg_fill
        );
        assert_eq!(v.hyperlink_color, palette::LINK);
        assert_eq!(
            v.widgets.hovered.bg_stroke.color,
            Color32::from_rgba_premultiplied(193, 164, 222, 242)
        );
        assert_eq!(
            style.wrap_mode,
            Some(egui::TextWrapMode::Extend),
            "a host's wrapping must not collapse the nodes"
        );
    }
}
