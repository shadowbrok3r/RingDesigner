//! The stamp window over the ring: pinned low in a band of the view clear of what floats over it, its rows scrolling when the band is short.
use egui::{Rect, Vec2};
use egui_mobile::egui;
use ringdesign_core::setting::Stamp;
use ringdesign_workbench::{
    touch,
    viewport::made::{self, Inspected},
};

/// Space kept between the window and the view's foot or anything covering the view, points.
pub const GAP_PT: f32 = 8.0;
/// The window's title bar and frame until it has been drawn once, points.
const CHROME_PT: f32 = 56.0;

/// The window's id.
pub fn id() -> egui::Id {
    egui::Id::new("phone-stamp-window")
}

/// The band across `view` a window `size` points big stands in: the lowest clear of the `covered` rects its width meets that holds its height, else the tallest.
pub fn room(view: Rect, covered: &[Rect], size: Vec2) -> Rect {
    let reach = view.left() + GAP_PT + size.x;
    let mut spans: Vec<(f32, f32)> = covered.iter().filter(|r| r.intersects(view) && r.left() < reach).map(|r| (r.top() - GAP_PT, r.bottom() + GAP_PT)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut bands = Vec::new();
    let mut top = view.top();
    for (lo, hi) in spans {
        if lo > top {
            bands.push((top, lo));
        }
        top = top.max(hi);
    }
    if view.bottom() > top {
        bands.push((top, view.bottom()));
    }
    let tallest = bands.iter().copied().max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)));
    bands.iter().rev().copied().find(|(lo, hi)| hi - lo >= size.y + GAP_PT).or(tallest).map_or(view, |(lo, hi)| Rect::from_x_y_ranges(view.x_range(), lo..=hi))
}

/// Draws `stamp`'s window pinned to the bottom-left of its [`room`], the focused field scrolled into sight; what the inspector read and whether the window stays open.
pub fn show(ctx: &egui::Context, view: Rect, covered: &[Rect], stamp: &mut Stamp) -> (Inspected, bool) {
    let last = ctx.data(|d| d.get_temp::<Measured>(id()));
    let Measured { chrome, size, .. } = last.unwrap_or(Measured { chrome: CHROME_PT, size: view.size(), rect: Rect::NOTHING });
    let room = room(view, covered, size);
    let rows = (room.height() - GAP_PT - chrome).max(touch::TARGET_PT);
    let mut open = true;
    let window = egui::Window::new("Stamp").id(id()).open(&mut open).collapsible(false).resizable(false);
    let shown = window.pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(room.left_bottom() + egui::vec2(GAP_PT, -GAP_PT)).constrain_to(room).show(ctx, |ui| {
        let out = egui::ScrollArea::vertical().max_height(rows).min_scrolled_height(rows).show(ui, |ui| {
            let read = made::inspector(ui, stamp);
            let focused = ui.memory(|m| m.focused()).and_then(|f| ui.ctx().read_response(f));
            if let Some(r) = focused.filter(|r| r.layer_id == ui.layer_id() && !ui.clip_rect().contains_rect(r.rect)) {
                ui.scroll_to_rect(r.rect, None);
            }
            read
        });
        (out.inner, ui.min_rect().height(), out.content_size.y)
    });
    let Some(shown) = shown else { return (Inspected::default(), open) };
    let window = shown.response.rect;
    let Some((read, seen, content)) = shown.inner else { return (Inspected::default(), open) };
    let chrome = window.height() - seen;
    let now = Measured { chrome, size: egui::vec2(window.width(), chrome + content), rect: window };
    if last.is_none_or(|l| !l.settled(&now)) {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id(), now));
    (read, open)
}

/// The window as last drawn: its title bar and frame, its size unscrolled, and where it stood.
#[derive(Clone, Copy, Debug)]
struct Measured {
    chrome: f32,
    size: Vec2,
    rect: Rect,
}

impl Measured {
    /// Whether `now` draws where and as big as this, to half a point.
    fn settled(&self, now: &Self) -> bool {
        (self.chrome - now.chrome).abs() <= 0.5 && (self.size - now.size).length() <= 0.5 && (self.rect.min - now.rect.min).length() <= 0.5 && (self.rect.max - now.rect.max).length() <= 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn the_window_stands_in_the_lowest_band_that_holds_it_else_the_tallest() {
        let view = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 700.0));
        let rail = Rect::from_min_size(pos2(5.0, 110.0), vec2(135.0, 50.0));
        let navigator = Rect::from_min_size(pos2(270.0, 110.0), vec2(140.0, 190.0));
        let band = |lo: f32, hi: f32| Rect::from_x_y_ranges(view.x_range(), lo..=hi);
        assert_eq!(room(view, &[rail, navigator], vec2(380.0, 300.0)), band(308.0, 800.0), "under the navigator");
        // A panel floating low leaves the band over it, when that holds the window.
        let panel = Rect::from_min_size(pos2(20.0, 640.0), vec2(200.0, 80.0));
        assert_eq!(room(view, &[rail, navigator, panel], vec2(380.0, 300.0)), band(308.0, 632.0));
        // The keypad up: no band holds the window, so the tallest.
        let short = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 380.0));
        assert_eq!(room(short, &[rail, navigator], vec2(380.0, 300.0)), band(308.0, 480.0));
        // A window that ends left of the navigator stands beside it.
        assert_eq!(room(view, &[rail, navigator], vec2(200.0, 300.0)), band(168.0, 800.0));
        // Nothing clear at all: the whole view.
        assert_eq!(room(view, &[view], vec2(380.0, 300.0)), view);
    }

    #[test]
    fn a_scrolling_window_squeezed_further_asks_for_the_frame_that_fits_it_and_then_for_none() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut stamp = Stamp { name: "Moon".into(), theta_deg: 44.6, v_mm: 8.01, rot_deg: 180.0, outline: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]], height_mm: 0.34, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false };
        let navigator = Rect::from_min_size(pos2(270.0, 110.0), vec2(140.0, 190.0));
        let mut pass = |view: Rect| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 900.0))), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                show(ui.ctx(), view, &[navigator], &mut stamp);
            });
            out.textures_delta.clear();
            (ctx.memory(|m| m.area_rect(id())).unwrap(), out.viewport_output[&egui::ViewportId::ROOT].repaint_delay)
        };
        let (short, shorter) = (Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 380.0)), Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 330.0)));
        for _ in 0..40 {
            pass(short);
        }
        let (scrolling, settled) = pass(short);
        assert!(scrolling.height() < 180.0 && settled == std::time::Duration::MAX, "{scrolling:?} {settled:?}");
        // The first pass squeezed further asks for the next at once.
        assert_eq!(pass(shorter).1, std::time::Duration::ZERO);
        let passes: Vec<(Rect, std::time::Duration)> = (0..4).map(|_| pass(shorter)).collect();
        let (fitted, quiet) = passes[passes.len() - 1];
        assert_eq!(quiet, std::time::Duration::MAX, "{passes:?}");
        assert!(!fitted.intersects(navigator) && shorter.contains_rect(fitted), "{fitted:?}");
        assert!(fitted.height() < scrolling.height() - 40.0, "{fitted:?}");
    }

    #[test]
    fn with_the_keypad_up_the_window_stays_clear_of_the_navigator_and_scrolls_to_the_focused_field() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut stamp = Stamp { name: "Moon".into(), theta_deg: 44.6, v_mm: 8.01, rot_deg: 180.0, outline: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]], height_mm: 0.34, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false };
        let navigator = Rect::from_min_size(pos2(270.0, 110.0), vec2(140.0, 190.0));
        let mut pass = |view: Rect, events: Vec<egui::Event>| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 900.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                show(ui.ctx(), view, &[navigator], &mut stamp);
            });
            out.textures_delta.clear();
            ctx.memory(|m| m.area_rect(id())).unwrap()
        };
        let tall = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 700.0));
        for _ in 0..3 {
            pass(tall, vec![]);
        }
        let window = pass(tall, vec![]);
        assert!(window.left_bottom().distance(tall.left_bottom() + vec2(GAP_PT, -GAP_PT)) < 0.5, "{window:?} pinned to the view's foot");
        assert!(!window.intersects(navigator));
        // Tap the last field, Sinks: its value box is the bottom row's right end.
        let sinks = pos2(window.right() - 30.0, window.bottom() - 24.0);
        let touch = |pressed| vec![egui::Event::PointerMoved(sinks), egui::Event::PointerButton { pos: sinks, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }];
        pass(tall, touch(true));
        pass(tall, touch(false));
        let focused = ctx.memory(|m| m.focused()).expect("Sinks takes focus");
        // The keypad takes the view's lower half.
        let short = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 380.0));
        for _ in 0..30 {
            pass(short, vec![]);
        }
        let window = pass(short, vec![]);
        assert!(short.contains_rect(window), "{window:?}");
        assert!(!window.intersects(navigator.expand(GAP_PT - 0.5)), "{window:?} under the navigator");
        assert!(window.height() < 180.0, "{window:?} scrolls in the band under the navigator");
        assert_eq!(ctx.memory(|m| m.focused()), Some(focused));
        let field = ctx.read_response(focused).unwrap().rect;
        assert!(window.contains_rect(field), "{field:?} in sight in {window:?}");
    }
}
