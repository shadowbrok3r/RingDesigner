//! The editor's look — the comfyui-android graph view's, number for number:
//! an AMOLED canvas lit by three pools of colour under a graph-space dot
//! grid, glass nodes with a white hairline rim (pink when chosen),
//! axis-aligned wires, fat pins outside the body, inputs stacked above
//! outputs, and that app's visuals on every widget inside a node.

use egui::{Color32, CornerRadius, Stroke};
use egui_snarl::ui::{BackgroundPattern, NodeLayout, PinInfo, PinPlacement, SnarlStyle, WireStyle};
use ringdesign_graph::graph::Access;
use ringdesign_graph::value::ValueKind;

/// Primary accent — hot pink, reserved for what is chosen.
pub const PINK: Color32 = Color32::from_rgb(255, 61, 139);
pub const PINK_BRIGHT: Color32 = Color32::from_rgb(255, 110, 168);
/// Secondary accent — aqua: hover, links, live.
pub const AQUA: Color32 = Color32::from_rgb(43, 226, 214);
pub const AQUA_BRIGHT: Color32 = Color32::from_rgb(120, 240, 232);
/// Ambient light, never a signal.
pub const VIOLET: Color32 = Color32::from_rgb(163, 140, 255);
/// A pane's edge: a dim white hairline, because glass has no colour at its edge.
pub const RIM: Color32 = Color32::from_rgba_premultiplied(46, 46, 52, 46);
pub const RIM_BRIGHT: Color32 = Color32::from_rgba_premultiplied(72, 72, 80, 72);
/// Body ink — cool near-white.
pub const INK: Color32 = Color32::from_rgb(233, 233, 239);
pub const INK_DIM: Color32 = Color32::from_rgb(150, 148, 162);
/// A node in trouble.
pub const ERROR: Color32 = Color32::from_rgb(237, 69, 92);

/// The AMOLED canvas.
pub const CANVAS: Color32 = Color32::from_rgb(3, 3, 5);
/// A node body: dark glass a step above the black canvas, `(22, 21, 34)` at alpha 190.
pub const NODE_FILL: Color32 = Color32::from_rgba_premultiplied(16, 16, 25, 190);
pub const NODE_CORNER: f32 = 8.0;
/// Dot grid spacing and radius in graph units, so the grid scales with the nodes.
pub const DOT_SPACING: f32 = 28.0;
pub const DOT_RADIUS: f32 = 1.7;
/// Dim teal ink for the dot grid.
pub const DOT_COLOR: Color32 = Color32::from_rgb(30, 70, 74);
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

/// The canvas behind the nodes: light, then the dot grid. `viewport` is the
/// visible rect in graph space and `scale` the view's zoom.
pub fn paint_canvas(painter: &egui::Painter, viewport: egui::Rect, scale: f32) {
    if !viewport.is_finite() {
        return;
    }
    ambience(painter, viewport, 3);
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
            painter.circle_filled(egui::pos2(xi as f32 * spacing, yi as f32 * spacing), DOT_RADIUS, DOT_COLOR);
        }
    }
}

/// Three pools of light — violet, aqua, pink — anchored to the visible rect
/// so the canvas stays evenly lit as it pans.
pub fn ambience(painter: &egui::Painter, rect: egui::Rect, ring_alpha: u8) {
    let d = rect.width().min(rect.height()).max(1.0);
    for (fx, fy, fr, color) in [(0.12, 0.14, 0.46, VIOLET), (0.94, 0.38, 0.38, AQUA), (0.46, 0.97, 0.42, PINK)] {
        light_pool(painter, rect.lerp_inside(egui::vec2(fx, fy)), d * fr, color, ring_alpha);
    }
}

/// One pool of light: nested discs of a constant low alpha, largest first.
fn light_pool(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32, ring_alpha: u8) {
    const RINGS: usize = 16;
    let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), ring_alpha);
    for i in 0..RINGS {
        let t = 1.0 - i as f32 / RINGS as f32;
        painter.circle_filled(center, radius * t, fill);
    }
}

/// That app's visuals on the widgets inside the editor: a black page, glass
/// panes with a cool rim, aqua hover, pink press and selection.
pub fn apply_visuals(style: &mut egui::Style) {
    let rgba = Color32::from_rgba_unmultiplied;
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(INK);
    v.panel_fill = rgba(0, 0, 0, 232);
    v.window_fill = rgba(19, 17, 30, 120);
    v.window_stroke = Stroke::new(1.2, RIM);
    v.faint_bg_color = Color32::from_rgb(11, 10, 16);
    v.extreme_bg_color = Color32::from_rgb(8, 7, 13);
    v.code_bg_color = Color32::from_rgb(6, 5, 10);
    v.hyperlink_color = AQUA;
    v.warn_fg_color = AQUA_BRIGHT;
    v.error_fg_color = PINK;
    v.selection.bg_fill = rgba(255, 61, 139, 140);
    v.selection.stroke = Stroke::new(1.4, rgba(255, 110, 168, 255));
    v.window_shadow = egui::epaint::Shadow { offset: [0, 2], blur: 12, spread: 2, color: rgba(0, 0, 0, 200) };
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 1, color: rgba(0, 0, 0, 170) };
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    widget_palette(&mut v.widgets);
    v.striped = true;
    v.collapsing_header_frame = true;
    v.indent_has_left_vline = true;
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 6.0);
}

fn widget_palette(w: &mut egui::style::Widgets) {
    let rgba = Color32::from_rgba_unmultiplied;
    let text = INK;
    let text_bright = Color32::from_rgb(248, 250, 252);
    let radius = CornerRadius::same(5);
    w.noninteractive.bg_fill = rgba(18, 16, 28, 132);
    w.noninteractive.weak_bg_fill = rgba(14, 12, 22, 120);
    w.noninteractive.bg_stroke = Stroke::new(1.0, RIM);
    w.noninteractive.fg_stroke = Stroke::new(1.0, text);
    w.noninteractive.corner_radius = radius;
    w.inactive.bg_fill = rgba(31, 28, 47, 165);
    w.inactive.weak_bg_fill = rgba(25, 23, 38, 150);
    w.inactive.bg_stroke = Stroke::new(1.0, RIM_BRIGHT);
    w.inactive.fg_stroke = Stroke::new(1.0, text);
    w.inactive.corner_radius = radius;
    w.hovered.bg_fill = rgba(43, 226, 214, 42);
    w.hovered.weak_bg_fill = rgba(43, 226, 214, 42);
    w.hovered.bg_stroke = Stroke::new(1.5, rgba(43, 226, 214, 240));
    w.hovered.fg_stroke = Stroke::new(1.5, text_bright);
    w.hovered.corner_radius = radius;
    w.active.bg_fill = rgba(255, 61, 139, 54);
    w.active.weak_bg_fill = rgba(255, 61, 139, 54);
    w.active.bg_stroke = Stroke::new(1.7, rgba(255, 61, 139, 245));
    w.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
    w.active.corner_radius = radius;
    w.open.bg_fill = rgba(31, 28, 47, 165);
    w.open.weak_bg_fill = rgba(25, 23, 38, 150);
    w.open.bg_stroke = Stroke::new(1.3, rgba(43, 226, 214, 205));
    w.open.fg_stroke = Stroke::new(1.0, text);
    w.open.corner_radius = radius;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_style_is_the_comfyui_one() {
        let s = snarl_style();
        assert!(matches!(s.wire_style, Some(WireStyle::AxisAligned { corner_radius }) if corner_radius == 8.0));
        assert!(matches!(s.pin_placement, Some(PinPlacement::Outside { margin }) if margin == 3.0));
        assert_eq!((s.pin_size, s.wire_width, s.min_scale, s.max_scale), (Some(15.0), Some(2.6), Some(MIN_SCALE), Some(MAX_SCALE)));
        assert!(matches!(s.bg_pattern, Some(BackgroundPattern::NoPattern)));
        assert!(s.node_layout.is_some());
        assert_eq!(s.bg_frame.unwrap().fill, CANVAS);
    }

    #[test]
    fn pins_keep_their_kind_but_wear_the_palette() {
        assert_ne!(pin_color(ValueKind::Number), pin_color(ValueKind::Layer));
        let h = egui::ecolor::Hsva::from(pin_color(ValueKind::Design));
        assert!((h.s - 0.5).abs() < 0.05 && (h.v - 0.5).abs() < 0.05, "{h:?}");
        assert_eq!(pin_color(ValueKind::Any), Color32::from_gray(110));
    }

    #[test]
    fn frames_read_their_state() {
        let d = egui::Frame::new();
        assert_eq!(node_frame(d, true, false).stroke.color, PINK);
        assert_eq!(node_frame(d, true, true).stroke.color, ERROR, "trouble outranks selection");
        let rest = node_frame(d, false, false);
        assert_eq!((rest.stroke.color, rest.fill), (RIM_BRIGHT, NODE_FILL));
    }

    #[test]
    fn the_visuals_are_pink_and_aqua_on_black() {
        let mut style = egui::Style::default();
        apply_visuals(&mut style);
        let v = &style.visuals;
        assert_eq!(v.override_text_color, Some(INK));
        assert_eq!(v.selection.bg_fill, Color32::from_rgba_unmultiplied(255, 61, 139, 140));
        assert_eq!(v.hyperlink_color, AQUA);
        assert_eq!(v.widgets.hovered.bg_stroke.color, Color32::from_rgba_unmultiplied(43, 226, 214, 240));
    }
}
