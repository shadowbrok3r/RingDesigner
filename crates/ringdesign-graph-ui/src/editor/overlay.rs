//! What draws over the nodes: the minimap, the selection box, and the note that follows a dragged wire.

use super::*;

impl Editor {
    /// The layer above the snarl's own, in screen space, clipped to the canvas.
    fn overlay_painter(&self, ui: &Ui, viewport: egui::Rect) -> Option<egui::Painter> {
        let id = self.snarl_id?;
        let layer = egui::LayerId::new(ui.layer_id().order, id.with("overlay"));
        ui.ctx().set_sublayer(ui.layer_id(), layer);
        Some(ui.ctx().layer_painter(layer).with_clip_rect(viewport))
    }

    pub(super) fn paint_overlays(&mut self, ui: &Ui, viewport: egui::Rect) {
        let Some(painter) = self.overlay_painter(ui, viewport) else { return };
        if let Some(b) = self.gesture.screen_box(ui.ctx()) {
            let accent = egui_mobile_theme::color(ui.ctx(), egui_mobile_theme::Role::Accent);
            painter.rect(b, 2.0, accent.gamma_multiply(0.10), egui::Stroke::new(1.2, accent), egui::StrokeKind::Inside);
        }
        self.minimap = None;
        if self.show_minimap {
            self.paint_minimap(ui, &painter, viewport);
        }
        if let Some(h) = self.wire_hint(ui.ctx()) {
            self.paint_wire_note(ui, &painter, h);
        }
    }

    /// A small map of every node and the current view, in the host's corner; pressing it moves the view.
    fn paint_minimap(&mut self, ui: &Ui, painter: &egui::Painter, viewport: egui::Rect) {
        let Some(bounds) = node_bounds(&self.snarl, &self.sizes, &self.ids) else { return };
        if self.snarl.node_ids().count() < 2 || !viewport.is_finite() || viewport.width() < 160.0 || viewport.height() < 160.0 {
            return;
        }
        let pad = bounds.expand(60.0);
        let w = (viewport.width() * 0.30).clamp(96.0, 200.0);
        let h = (w * (pad.height() / pad.width().max(1.0)).clamp(0.35, 1.4)).clamp(60.0, 200.0);
        let map = self.minimap_corner.align_size_within_rect(egui::vec2(w, h), viewport.shrink(10.0));
        painter.rect_filled(map, 4.0, Color32::from_black_alpha(200));
        let scale = (map.size() / pad.size()).min_elem();
        let tf = egui::emath::TSTransform::new(map.center().to_vec2() - pad.center().to_vec2() * scale, scale);
        let selected = self.selected.and_then(|g| self.view_node_of(g));
        for (sid, pos, card) in self.snarl.nodes_pos_ids() {
            let mut m = tf * egui::Rect::from_min_size(pos, self.size_of(sid));
            if m.width() < 2.0 || m.height() < 2.0 {
                m = egui::Rect::from_center_size(m.center(), m.size().max(egui::vec2(2.0, 2.0)));
            }
            let vid = self.ids.to_graph.get(&sid).copied();
            let color = if !card.diag.is_empty() {
                crate::style::ERROR
            } else if vid.is_some() && vid == selected {
                Color32::from_rgb(110, 170, 255)
            } else if card.kind == view::GROUP_KIND {
                crate::style::AQUA.gamma_multiply(0.8)
            } else {
                Color32::from_gray(150)
            };
            painter.rect_filled(m.intersect(map), 1.0, color);
        }
        if let Some(t) = self.transform {
            let inv = t.inverse();
            let view = egui::Rect::from_min_max(inv * viewport.min, inv * viewport.max);
            painter.rect_stroke((tf * view).intersect(map), 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        painter.rect_stroke(map, 4.0, egui::Stroke::new(1.0, Color32::from_gray(90)), egui::StrokeKind::Inside);
        if ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| map.contains(p)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        self.minimap = Some((map, tf));
    }

    /// Beside the pointer while a wire is dragged: what it carries and where it can go.
    fn paint_wire_note(&self, ui: &Ui, painter: &egui::Painter, h: WireHint) {
        let Some(p) = ui.input(|i| i.pointer.latest_pos()) else { return };
        let many = if h.spec_access == Access::List { "a list of " } else { "" };
        let (verb, partner) = if h.from.input { ("takes", "an output that gives it") } else { ("gives", "an input that takes it") };
        let text = format!("{verb} {many}{}  —  drop on {partner} (ringed), or on empty canvas to add a node", h.kind.label());
        let font = egui::FontId::proportional(13.0);
        let galley = painter.layout_no_wrap(text, font, crate::style::INK);
        let at = p + egui::vec2(18.0, 18.0);
        let rect = egui::Rect::from_min_size(at, galley.size()).expand(6.0);
        painter.rect_filled(rect, 4.0, Color32::from_black_alpha(220));
        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, crate::widgets::kind_color(h.kind)), egui::StrokeKind::Inside);
        painter.galley(at, galley, crate::style::INK);
    }
}
