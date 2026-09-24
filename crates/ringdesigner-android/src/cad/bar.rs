//! A mode's bar over the ring, Measure's or box select's: what the mode says and its buttons a thumb high, at the view's foot unless something covers it there.
use egui::Rect;
use egui_mobile::egui;
use ringdesign_workbench::{icons::Icon, touch};

/// Clearance kept under the bar on a tall view, for the view's own status line, points.
const FOOT_PT: f32 = 48.0;
/// A view shorter than this keeps the bar close to its edge, points.
const SHORT_VIEW_PT: f32 = 360.0;
/// The bar's height until it has been drawn once, points.
const BAR_PT: f32 = 96.0;

/// The area the bar is drawn in.
pub fn area() -> egui::Id {
    egui::Id::new("phone-mode-bar")
}

/// One of the bar's buttons: its mark, its words, whether it is ticked and whether it is offered.
#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub icon: Icon,
    pub label: &'static str,
    pub checked: bool,
    pub enabled: bool,
}

/// Whether the bar stands at the top of `view`: only when something drawn over the ring covers the foot it would stand at.
pub fn at_top(view: Rect, covered: &[Rect], height: f32) -> bool {
    let foot = if view.height() > SHORT_VIEW_PT { FOOT_PT } else { 6.0 };
    let bottom = Rect::from_min_max(egui::pos2(view.left(), view.bottom() - foot - height), egui::pos2(view.right(), view.bottom() - foot));
    covered.iter().any(|r| r.intersects(bottom))
}

/// Draws the bar across `view`, clear of `covered`; the index of the button tapped.
pub fn show(ctx: &egui::Context, view: Rect, covered: &[Rect], lines: &[(String, egui::Color32)], buttons: &[Button]) -> Option<usize> {
    show_groups(ctx, view, covered, lines, &[buttons]).map(|(_, i)| i)
}

/// Draws the bar with its buttons in `groups`, flowed as one run with a rule between groups; the group and index of the button tapped.
pub fn show_groups(ctx: &egui::Context, view: Rect, covered: &[Rect], lines: &[(String, egui::Color32)], groups: &[&[Button]]) -> Option<(usize, usize)> {
    let height = ctx.memory(|m| m.area_rect(area())).map_or(BAR_PT, |r| r.height());
    let (pivot, at) = if at_top(view, covered, height) {
        (egui::Align2::LEFT_TOP, view.left_top() + egui::vec2(8.0, 8.0))
    } else {
        let foot = if view.height() > SHORT_VIEW_PT { FOOT_PT } else { 6.0 };
        (egui::Align2::LEFT_BOTTOM, view.left_bottom() + egui::vec2(8.0, -foot))
    };
    let mut tapped = None;
    egui::Area::new(area()).order(egui::Order::Foreground).pivot(pivot).fixed_pos(at).constrain_to(view).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgba_unmultiplied(12, 12, 18, 236)).show(ui, |ui| {
            ui.set_max_width((view.width() - 32.0).max(120.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            for (text, color) in lines.iter().filter(|(t, _)| !t.is_empty()) {
                ui.label(egui::RichText::new(text).color(*color).size(12.0));
            }
            ui.horizontal_wrapped(|ui| {
                for (g, buttons) in groups.iter().enumerate() {
                    if g > 0 {
                        ui.separator();
                    }
                    for (i, b) in buttons.iter().enumerate() {
                        let button = egui::Button::selectable(b.checked, (b.icon.image(ui, 20.0), b.label)).min_size(egui::vec2(72.0, touch::TARGET_PT)).frame_when_inactive(true);
                        if ui.add_enabled(b.enabled, button).clicked() {
                            tapped = Some((g, i));
                        }
                    }
                }
            });
        });
    });
    tapped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_stands_at_the_foot_unless_something_covers_it_there() {
        let view = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 600.0));
        assert!(!at_top(view, &[], 96.0));
        // The navigator in the top corner leaves the foot clear.
        assert!(!at_top(view, &[Rect::from_min_size(egui::pos2(320.0, 8.0), egui::vec2(92.0, 92.0))], 96.0));
        // A palette standing over the foot sends the bar to the top.
        assert!(at_top(view, &[Rect::from_min_size(egui::pos2(100.0, 420.0), egui::vec2(220.0, 150.0))], 96.0));
        // Under the status line's clearance nothing is in the bar's way.
        assert!(!at_top(view, &[Rect::from_min_size(egui::pos2(0.0, 560.0), egui::vec2(420.0, 40.0))], 96.0));
    }
}
