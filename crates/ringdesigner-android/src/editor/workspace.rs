//! Persistent workspace geometry and touch grips, independent of Android's host.
use egui_mobile::egui;

/// Android's keyboard can finish its inset animation without another input event.
/// Keep layout awake through transitions, then stop polling once it is closed.
#[derive(Default)]
pub struct Reflow {
    last: Option<(egui::Rect, f32)>,
    settle_until: f64,
}

impl Reflow {
    pub fn next_frame(
        &mut self,
        now: f64,
        bounds: egui::Rect,
        keyboard_height: f32,
    ) -> Option<std::time::Duration> {
        let current = (bounds, keyboard_height);
        if self.last != Some(current) {
            self.last = Some(current);
            self.settle_until = now + 0.75;
        }
        if now < self.settle_until {
            Some(std::time::Duration::from_millis(16))
        } else if keyboard_height > 0.0 {
            Some(std::time::Duration::from_millis(250))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    Properties,
    Layers,
    More,
    Details(super::Sheet),
}

pub fn inspector_extent(available: egui::Vec2, landscape: bool, fraction: f32) -> f32 {
    let axis = if landscape { available.x } else { available.y };
    let reserved = if landscape { 220.0 } else { 190.0 };
    let maximum = (axis - reserved).max(0.0).min(axis * 0.75);
    let minimum = (if landscape { 180.0_f32 } else { 72.0 }).min(maximum);
    let fraction = if fraction.is_finite() { fraction } else { 0.32 };
    (axis * fraction.clamp(0.08, 0.75)).clamp(minimum, maximum)
}

/// The bottom inspector hugs its measured contents; long editors scroll at half height.
pub fn content_height(available: f32, measured: Option<f32>) -> f32 {
    let cap = (available * 0.5).max(0.0);
    measured.filter(|v| v.is_finite()).unwrap_or(cap).clamp(42.0_f32.min(cap), cap)
}

#[derive(Clone, Copy)]
struct Grab {
    pointer: egui::Pos2,
    initial: egui::Pos2,
}

/// Dedicated grip: scrolling controls below it never resizes the inspector.
/// Returns true when the new preference should be persisted.
pub fn splitter(
    ui: &mut egui::Ui,
    fraction: &mut f32,
    available: egui::Vec2,
    landscape: bool,
) -> bool {
    let id = egui::Id::new(("inspector-resize", landscape));
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 24.0),
        egui::Sense::click_and_drag(),
    );
    super::layout::record(ui, "inspector/resize", rect);
    let color = if response.dragged() || response.hovered() {
        crate::theme::AQUA
    } else {
        crate::theme::INK_DIM
    };
    let middle = rect.center();
    for dy in [-2.0, 2.0] {
        let endpoints = if landscape {
            [middle + egui::vec2(dy, -5.0), middle + egui::vec2(dy, 5.0)]
        } else {
            [
                middle + egui::vec2(-20.0, dy),
                middle + egui::vec2(20.0, dy),
            ]
        };
        ui.painter()
            .line_segment(endpoints, egui::Stroke::new(1.5, color));
    }
    let response = response
        .on_hover_cursor(if landscape {
            egui::CursorIcon::ResizeHorizontal
        } else {
            egui::CursorIcon::ResizeVertical
        })
        .on_hover_text("Drag to give the ring or inspector more room. Double-tap to reset.");
    if response.drag_started() {
        if let Some(pointer) = ui.input(|i| i.pointer.press_origin()) {
            let extent = inspector_extent(available, landscape, *fraction);
            ui.data_mut(|d| {
                d.insert_temp(
                    id,
                    Grab {
                        pointer,
                        initial: egui::pos2(extent, 0.0),
                    },
                )
            });
        }
    }
    if response.dragged() {
        if let (Some(grab), Some(pointer)) = (
            ui.data(|d| d.get_temp::<Grab>(id)),
            response.interact_pointer_pos(),
        ) {
            let delta = grab.pointer - pointer;
            let axis = if landscape { available.x } else { available.y };
            let extent = grab.initial.x + if landscape { delta.x } else { delta.y };
            *fraction = (inspector_extent(available, landscape, extent / axis.max(1.0))
                / axis.max(1.0))
            .clamp(0.08, 0.75);
            ui.ctx().request_repaint();
        }
    }
    if response.double_clicked() {
        *fraction = if landscape { 0.36 } else { 0.32 };
        return true;
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<Grab>(id));
        return true;
    }
    false
}

pub fn position(
    bounds: egui::Rect,
    size: egui::Vec2,
    normalized: Option<[f32; 2]>,
    default_offset: egui::Vec2,
) -> egui::Pos2 {
    let room = (bounds.size() - size).max(egui::Vec2::ZERO);
    let offset = normalized.map_or(default_offset, |p| egui::vec2(p[0] * room.x, p[1] * room.y));
    bounds.min + offset.clamp(egui::Vec2::ZERO, room)
}

fn normalized(bounds: egui::Rect, size: egui::Vec2, at: egui::Pos2) -> [f32; 2] {
    let room = (bounds.size() - size).max(egui::vec2(1.0, 1.0));
    let offset = (at - bounds.min) / room;
    [offset.x.clamp(0.0, 1.0), offset.y.clamp(0.0, 1.0)]
}

pub struct Floating<R> {
    pub inner: R,
    pub rect: egui::Rect,
    pub moved: bool,
    pub close: bool,
    pub dragging: bool,
}

/// Only the named header captures drag; buttons, sliders and scrolls cannot move it.
pub fn floating<R>(
    ctx: &egui::Context,
    key: &'static str,
    title: &str,
    bounds: egui::Rect,
    size: egui::Vec2,
    default_offset: egui::Vec2,
    saved_position: &mut Option<[f32; 2]>,
    close_label: Option<&str>,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> Floating<R> {
    let id = egui::Id::new(key);
    let size = size.min(bounds.size()).max(egui::vec2(1.0, 1.0));
    let measured = ctx
        .data(|d| d.get_temp::<egui::Vec2>(id.with("measured")))
        .unwrap_or(size)
        .min(size);
    let at = position(bounds, measured, *saved_position, default_offset);
    let mut moved = false;
    let mut close = false;
    let mut dragging = false;
    let close_button = close_label.is_some();
    let response = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .default_size(size)
        .constrain_to(bounds)
        .movable(false)
        .sense(egui::Sense::click())
        .show(ctx, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(bounds));
            ui.set_width((size.x - 12.0).max(1.0));
            // Area remembers its last content height. Let a newly expanded
            // submenu/parameter grow again, while the scroll area enforces the cap.
            ui.set_max_height(size.y);
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
            ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
            ui.spacing_mut().interact_size.y = 26.0;
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let frame = egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 25, 248))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(67)))
                .corner_radius(6)
                .inner_margin(5)
                .show(ui, |ui| {
                    ui.set_width((size.x - 12.0).max(1.0));
                    ui.horizontal(|ui| {
                        let w =
                            (ui.available_width() - if close_button { 34.0 } else { 0.0 }).max(1.0);
                        let (grip, drag) =
                            ui.allocate_exact_size(egui::vec2(w, 25.0), egui::Sense::drag());
                        super::layout::record(ui, format!("{key}/grip"), grip);
                        let ink = if drag.dragged() {
                            crate::theme::AQUA
                        } else {
                            crate::theme::INK_DIM
                        };
                        for x in [3.0, 6.0] {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(grip.left() + x, grip.center().y - 4.0),
                                    egui::pos2(grip.left() + x, grip.center().y + 4.0),
                                ],
                                egui::Stroke::new(1.0, ink),
                            );
                        }
                        let text_rect = grip.shrink2(egui::vec2(11.0, 0.0));
                        ui.painter().with_clip_rect(text_rect).text(
                            text_rect.left_center(),
                            egui::Align2::LEFT_CENTER,
                            title,
                            egui::FontId::proportional(11.0),
                            ink,
                        );
                        let drag = drag
                            .on_hover_cursor(egui::CursorIcon::Grab)
                            .on_hover_text("Drag this header to move the toolbar");
                        if drag.drag_started() {
                            if let Some(pointer) = ui.input(|i| i.pointer.press_origin()) {
                                ui.data_mut(|d| {
                                    d.insert_temp(
                                        id,
                                        Grab {
                                            pointer,
                                            initial: at,
                                        },
                                    )
                                });
                            }
                        }
                        if drag.dragged() {
                            dragging = true;
                            if let (Some(grab), Some(pointer)) = (
                                ui.data(|d| d.get_temp::<Grab>(id)),
                                drag.interact_pointer_pos(),
                            ) {
                                *saved_position = Some(normalized(
                                    bounds,
                                    measured,
                                    grab.initial + (pointer - grab.pointer),
                                ));
                                ui.ctx().request_repaint();
                            }
                        }
                        if drag.drag_stopped() {
                            moved = true;
                            ui.data_mut(|d| d.remove::<Grab>(id));
                        }
                        if close_button {
                            let icon = match close_label {
                                Some("+") => ringdesign_workbench::icons::Icon::Expand,
                                Some("−") => ringdesign_workbench::icons::Icon::Collapse,
                                _ => ringdesign_workbench::icons::Icon::Close,
                            };
                            let r = ringdesign_workbench::icons::compact(ui, icon, false);
                            super::layout::record(ui, format!("{key}/close"), r.rect);
                            close = r.clicked();
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_salt((key, "scroll"))
                        .auto_shrink([false, true])
                        .max_height((size.y - 42.0).max(1.0))
                        .min_scrolled_height(1.0)
                        .show(ui, content)
                        .inner
                });
            super::layout::record(ui, key, frame.response.rect);
            frame.inner
        });
    ctx.data_mut(|d| d.insert_temp(id.with("measured"), response.response.rect.size()));
    Floating {
        inner: response.inner,
        rect: response.response.rect,
        moved,
        close,
        dragging,
    }
}

#[cfg(test)]
mod tests;
