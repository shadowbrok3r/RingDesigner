//! What a press on the canvas does: move a node, pan, draw a selection box, steer from the minimap, drag a wire.

use egui::emath::TSTransform;
use egui::{PointerButton, Pos2, Rect, Vec2};

use super::*;

/// Where a canvas press landed — decides whether a drag moves a node, pans, selects or wires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DragKind {
    #[default]
    None,
    /// On a node's title bar: the node moves.
    Header,
    /// On a node below its title: a mouse moves the node, a finger pans.
    Body,
    /// On a pin: snarl draws a wire.
    Pin,
    /// Empty canvas: a mouse draws a selection box, a finger pans.
    Canvas,
    /// On the minimap: the view follows the pointer.
    Minimap,
}

/// The least a title bar may measure on screen and still be grabbed by a finger, in points.
const MIN_GRAB_PT: f32 = 36.0;
/// A press that travels less than this is a click, in points.
const CLICK_SLOP: f32 = 4.0;

/// The graph-space height that moves a node at this zoom: the title bar,
/// grown so it never falls under a fingertip on screen. Zoomed far out that
/// covers the whole node, which is right — nothing inside it can be read or
/// touched at that size, so all of it is the handle.
pub fn grab_height(scale: f32) -> f32 {
    NODE_HEADER_H.max(MIN_GRAB_PT / scale.max(0.01))
}

/// Which region a graph-space point falls on, over the drawn node frames.
/// Any header wins over any body, so a title-bar grab can always move a
/// node where nodes overlap.
pub fn classify_point(gp: egui::Pos2, frames: impl IntoIterator<Item = egui::Rect>, grab_h: f32) -> DragKind {
    let mut on_body = false;
    for frame in frames {
        if !frame.contains(gp) {
            continue;
        }
        let header = egui::Rect::from_min_size(frame.min, egui::vec2(frame.width(), grab_h.min(frame.height())));
        if header.contains(gp) {
            return DragKind::Header;
        }
        on_body = true;
    }
    if on_body { DragKind::Body } else { DragKind::Canvas }
}

/// What the press in hand does to the view this frame.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Frame {
    /// Screen delta to pan by.
    pub pan: Vec2,
    /// Undo every node move this frame.
    pub veto: bool,
    /// Discard snarl's own pan and zoom this frame: the gesture owns the view.
    pub freeze: Option<TSTransform>,
    /// Zoom by this factor about a screen point.
    pub wheel: Option<(f32, Pos2)>,
    /// Centre the view on this graph point.
    pub centre: Option<Pos2>,
    /// The selection box being drawn, in graph space.
    pub boxed: Option<Rect>,
}

/// The press in hand, from the frame it landed to the frame it lifts.
#[derive(Clone, Debug, Default)]
pub(crate) struct Gesture {
    pub kind: DragKind,
    button: Option<PointerButton>,
    touch: bool,
    /// Where the press landed, on the screen.
    origin: Option<Pos2>,
    /// The pin under the press.
    pin: Option<PinRef>,
    /// A box drawn with Ctrl or Cmd held takes nodes out of the selection.
    subtract: bool,
    /// Clear the snarl's selection at the next frame.
    pub clear_selection: bool,
    /// A click on empty canvas this frame: nothing is chosen any more.
    pub cleared: Option<bool>,
    /// A box selection ended this frame: the node chosen if it took exactly one.
    pub boxed: Option<Option<NodeId>>,
}

impl Gesture {
    /// The selection box on the screen, while one is being drawn.
    pub fn screen_box(&self, ctx: &egui::Context) -> Option<Rect> {
        if self.kind != DragKind::Canvas || self.touch || self.button != Some(PointerButton::Primary) {
            return None;
        }
        let (origin, now) = (self.origin?, ctx.input(|i| i.pointer.latest_pos())?);
        ((now - origin).length() > CLICK_SLOP).then(|| Rect::from_two_pos(origin, now))
    }
}

fn on_canvas(ctx: &egui::Context, viewport: Rect, p: Pos2) -> bool {
    viewport.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)
}

impl Editor {
    /// Classify a press as it lands and say what the press in hand does to the view this frame.
    pub(super) fn gesture_begin(&mut self, ctx: &egui::Context, viewport: Rect) -> Frame {
        let (pressed, origin, delta, zooming, touching) = ctx.input(|i| {
            (i.pointer.any_pressed(), i.pointer.press_origin(), i.pointer.delta(), (i.zoom_delta() - 1.0).abs() > f32::EPSILON || i.multi_touch().is_some(), i.any_touches())
        });
        if pressed {
            self.pinched = false;
        }
        if ctx.input(|i| i.multi_touch().is_some()) {
            self.pinched = true;
        }
        if pressed {
            let button = ctx.input(|i| [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle].into_iter().find(|b| i.pointer.button_pressed(*b)));
            let mut g = Gesture { button, touch: touching, origin, subtract: ctx.input(|i| i.modifiers.command), ..Gesture::default() };
            if let (Some(p), Some(t)) = (origin, self.transform) {
                if on_canvas(ctx, viewport, p) {
                    let gp = t.inverse() * p;
                    g.pin = self.pin_rects.iter().find(|(_, r)| r.contains(gp)).map(|(pin, _)| *pin);
                    g.kind = if self.minimap.is_some_and(|(map, _)| map.contains(p)) {
                        DragKind::Minimap
                    } else if g.pin.is_some() {
                        DragKind::Pin
                    } else {
                        classify_point(gp, self.node_frames(), grab_height(t.scaling))
                    };
                }
            }
            g.clear_selection = self.gesture.clear_selection;
            self.gesture = g;
        }
        let down = ctx.input(|i| i.pointer.any_down());
        // During a pinch the primary pointer still reports a delta; adding a
        // pan on top of the zoom drifts the view.
        let d = if delta.is_finite() && !zooming { delta } else { Vec2::ZERO };
        let mut f = Frame { wheel: self.take_wheel(ctx, viewport), ..Frame::default() };
        if !down {
            return f;
        }
        let g = &self.gesture;
        let mouse = !g.touch;
        match (g.kind, g.button) {
            (DragKind::Header | DragKind::Body | DragKind::Pin, Some(PointerButton::Secondary | PointerButton::Middle)) => f.pan = d,
            (DragKind::Minimap, Some(PointerButton::Primary)) => {
                f.freeze = self.transform;
                if let (Some(p), Some((_, tf))) = (ctx.input(|i| i.pointer.latest_pos()), self.minimap) {
                    f.centre = Some(tf.inverse() * p);
                }
            }
            (DragKind::Canvas, Some(PointerButton::Primary)) if mouse => {
                f.freeze = self.transform;
                if let (Some(b), Some(t)) = (g.screen_box(ctx), self.transform) {
                    f.boxed = Some(Rect::from_two_pos(t.inverse() * b.min, t.inverse() * b.max));
                }
            }
            (DragKind::Header | DragKind::Body, Some(PointerButton::Primary)) if !self.editable => {
                f.pan = d;
                f.veto = true;
            }
            (_, Some(PointerButton::Primary)) if !self.editable => f.veto = true,
            (DragKind::Body, Some(PointerButton::Primary)) if !mouse => {
                f.pan = d;
                f.veto = true;
            }
            _ => {}
        }
        f
    }

    /// The scroll wheel over the canvas zooms about the pointer; held with Shift or Alt it pans.
    fn take_wheel(&self, ctx: &egui::Context, viewport: Rect) -> Option<(f32, Pos2)> {
        let p = ctx.input(|i| i.pointer.hover_pos())?;
        if !on_canvas(ctx, viewport, p) {
            return None;
        }
        let speed = ctx.options(|o| o.input_options.scroll_zoom_speed);
        ctx.input_mut(|i| {
            if i.modifiers.shift || i.modifiers.alt || i.modifiers.command {
                return None;
            }
            let d = i.smooth_scroll_delta;
            if d == Vec2::ZERO {
                return None;
            }
            i.smooth_scroll_delta = Vec2::ZERO;
            Some((((d.x + d.y) * speed).exp(), p))
        })
    }

    /// The wire being dragged out of a pin, once the drag is decided.
    pub(super) fn wire_hint(&self, ctx: &egui::Context) -> Option<WireHint> {
        let g = &self.gesture;
        if g.kind != DragKind::Pin || g.button != Some(PointerButton::Primary) || !ctx.input(|i| i.pointer.primary_down() && i.pointer.is_decidedly_dragging()) {
            return None;
        }
        let pin = g.pin?;
        let card = self.snarl.get_node(pin.node)?;
        let spec = if pin.input { card.pins_in.get(pin.index)? } else { card.pins_out.get(pin.index)? };
        Some(WireHint { from: pin, kind: spec.kind, spec_access: spec.access })
    }

    /// Finish the press that lifted this frame: apply a selection box, or read a click on empty canvas.
    /// Returns the group whose frame title was clicked.
    pub(super) fn gesture_end(&mut self, ctx: &egui::Context, frame_titles: &[(usize, Rect)]) -> Option<usize> {
        if !ctx.input(|i| i.pointer.any_released()) || self.gesture.kind == DragKind::None {
            return None;
        }
        let g = std::mem::take(&mut self.gesture);
        self.gesture.clear_selection = g.clear_selection;
        if g.kind != DragKind::Canvas || g.touch || g.button != Some(PointerButton::Primary) || self.pinched {
            return None;
        }
        let (Some(origin), Some(t), Some(snarl_id)) = (g.origin, self.transform, self.snarl_id) else { return None };
        let up = ctx.input(|i| i.pointer.latest_pos()).unwrap_or(origin);
        if (up - origin).length() <= CLICK_SLOP {
            let gp = t.inverse() * up;
            if let Some((index, _)) = frame_titles.iter().find(|(_, r)| r.contains(gp)) {
                return Some(*index);
            }
            if !ctx.input(|i| i.modifiers.shift || i.modifiers.command) {
                egui_snarl::ui::set_selected_nodes(snarl_id, ctx, std::iter::empty());
                self.gesture.cleared = Some(true);
            }
            return None;
        }
        let area = Rect::from_two_pos(t.inverse() * origin, t.inverse() * up);
        let inside: Vec<SnarlId> = self.snarl.nodes_pos_ids().filter(|(sid, pos, _)| area.intersects(Rect::from_min_size(*pos, self.size_of(*sid)))).map(|(sid, _, _)| sid).collect();
        let chosen: Vec<SnarlId> = if g.subtract {
            let gone: BTreeSet<SnarlId> = inside.iter().copied().collect();
            egui_snarl::ui::get_selected_nodes(snarl_id, ctx).into_iter().filter(|s| !gone.contains(s)).collect()
        } else {
            inside
        };
        let single = match chosen.as_slice() {
            [one] => self.ids.to_graph.get(one).copied().and_then(|v| self.graph_node_of(v)),
            _ => None,
        };
        egui_snarl::ui::set_selected_nodes(snarl_id, ctx, chosen);
        self.gesture.boxed = Some(single);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_is_classified_by_where_it_lands() {
        let a = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 120.0));
        let b = egui::Rect::from_min_size(egui::pos2(150.0, 100.0), egui::vec2(200.0, 120.0));
        let frames = || [a, b];
        let h = grab_height(1.5);
        assert_eq!(h, NODE_HEADER_H, "zoomed in, the title bar is the handle");
        assert_eq!(classify_point(egui::pos2(20.0, 10.0), frames(), h), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), frames(), h), DragKind::Body);
        assert_eq!(classify_point(egui::pos2(400.0, 400.0), frames(), h), DragKind::Canvas);
        // Where a's body overlaps b's header, the header wins.
        assert_eq!(classify_point(egui::pos2(160.0, 110.0), frames(), h), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(5.0, 5.0), std::iter::empty(), h), DragKind::Canvas);
    }

    #[test]
    fn a_title_bar_never_shrinks_under_a_fingertip() {
        // At a third of full size a 30-unit bar is 10 points on screen; the
        // handle grows to stay 36, and far enough out it is the whole node.
        let a = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 120.0));
        for scale in [0.05_f32, 0.2, 0.33, 0.8, 1.0, 2.5] {
            assert!(grab_height(scale) * scale >= MIN_GRAB_PT.min(NODE_HEADER_H * scale).max(MIN_GRAB_PT - 0.01) - 0.01 || scale > 1.2, "{scale}");
            assert!(grab_height(scale) >= NODE_HEADER_H);
        }
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), [a], grab_height(0.33)), DragKind::Header);
        assert_eq!(classify_point(egui::pos2(20.0, 119.0), [a], grab_height(0.2)), DragKind::Header, "the whole node moves it");
        assert_eq!(classify_point(egui::pos2(20.0, 80.0), [a], grab_height(1.0)), DragKind::Body);
    }
}
