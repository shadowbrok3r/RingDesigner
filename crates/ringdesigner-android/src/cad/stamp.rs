//! The stamp window over the ring: pinned low in a band of the view clear of what floats over it, drawn over the Tools rail, its rows scrolling when the band is short.
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

/// What stands over the view: the floating tools, the rail's layer, and what the window never stands under.
#[derive(Clone, Copy, Debug, Default)]
pub struct Over<'a> {
    /// The floating tools, which the window stands over when no band clears them.
    pub floating: &'a [Rect],
    /// The Tools rail's layer, which the window is drawn directly above.
    pub rail: Option<egui::LayerId>,
    /// What the window's band always keeps clear of.
    pub fixed: &'a [Rect],
}

/// The stretches of `view`'s height clear of `covered`, top to bottom, each [`GAP_PT`] from what covers it.
fn bands(view: Rect, covered: &[Rect]) -> Vec<(f32, f32)> {
    let mut spans: Vec<(f32, f32)> = covered.iter().map(|r| (r.top() - GAP_PT, r.bottom() + GAP_PT)).collect();
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
    bands
}

/// The band across `view` a window `size` points big stands in: of those clear of the `floating` and `fixed` rects its width meets, the lowest that holds it, else the tallest that holds `least`, else from under `fixed` to the view's foot and at least `least` tall.
pub fn room(view: Rect, floating: &[Rect], fixed: &[Rect], size: Vec2, least: f32) -> Rect {
    let reach = view.left() + GAP_PT + size.x;
    let meets = |r: &&Rect| r.intersects(view) && r.left() < reach;
    let fixed: Vec<Rect> = fixed.iter().filter(meets).copied().collect();
    let all: Vec<Rect> = floating.iter().filter(meets).chain(&fixed).copied().collect();
    let bands = bands(view, &all);
    let tallest = bands.iter().copied().filter(|(lo, hi)| hi - lo >= least).max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)));
    let (lo, hi) = bands.iter().rev().copied().find(|(lo, hi)| hi - lo >= size.y + GAP_PT).or(tallest).unwrap_or_else(|| {
        let top = fixed.iter().map(|r| r.bottom() + GAP_PT).fold(view.top(), f32::max);
        (top, view.bottom().max(top + least))
    });
    Rect::from_x_y_ranges(view.x_range(), lo..=hi)
}

/// Draws `stamp`'s window pinned to the bottom-left of its [`room`] and directly above `over`'s rail, the focused field scrolled into sight; what the inspector read and whether the window stays open.
pub fn show(ctx: &egui::Context, view: Rect, over: Over<'_>, stamp: &mut Stamp) -> (Inspected, bool) {
    let last = ctx.data(|d| d.get_temp::<Measured>(id()));
    let Measured { chrome, size, .. } = last.unwrap_or(Measured { chrome: CHROME_PT, size: view.size(), rect: Rect::NOTHING });
    let room = room(view, over.floating, over.fixed, size, chrome + touch::TARGET_PT + GAP_PT);
    let rows = (room.height() - GAP_PT - chrome).max(touch::TARGET_PT);
    let mut open = true;
    let window = egui::Window::new("Stamp").id(id()).order(egui::Order::Foreground).open(&mut open).collapsible(false).resizable(false);
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
    let layer = shown.response.layer_id;
    if let Some(rail) = over.rail.filter(|r| r.order == layer.order) {
        ctx.memory_mut(|m| m.areas_mut().set_sublayer(rail, layer));
    }
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
    fn the_window_stands_in_the_lowest_band_that_holds_it_else_the_tallest_else_over_the_tools_under_the_navigator() {
        let view = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 700.0));
        let rail = Rect::from_min_size(pos2(5.0, 110.0), vec2(135.0, 50.0));
        let navigator = Rect::from_min_size(pos2(270.0, 110.0), vec2(140.0, 190.0));
        let band = |lo: f32, hi: f32| Rect::from_x_y_ranges(view.x_range(), lo..=hi);
        let (size, least) = (vec2(380.0, 300.0), 108.0);
        assert_eq!(room(view, &[rail], &[navigator], size, least), band(308.0, 800.0), "under the navigator");
        // A panel floating low leaves the band over it, when that holds the window.
        let panel = Rect::from_min_size(pos2(20.0, 640.0), vec2(200.0, 80.0));
        assert_eq!(room(view, &[rail, panel], &[navigator], size, least), band(308.0, 632.0));
        // The keypad up: no band holds the window, so the tallest.
        let short = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 380.0));
        assert_eq!(room(short, &[rail], &[navigator], size, least), band(308.0, 480.0));
        // A window that ends left of the navigator stands beside it.
        assert_eq!(room(view, &[rail], &[navigator], vec2(200.0, 300.0), least), band(168.0, 800.0));
        // No band clears the floating tools: the band under the navigator, over them.
        assert_eq!(room(view, &[view], &[navigator], size, least), band(308.0, 800.0));
        // The rail expanded in Surface mode with the keypad up on rdsmoke, in points.
        let vt = 80.0;
        let phone = Rect::from_min_size(pos2(0.0, vt), vec2(411.0, 337.0));
        let (tools, cube) = (Rect::from_min_size(pos2(4.0, vt + 8.0), vec2(92.0, 340.0)), Rect::from_min_size(pos2(298.0, vt + 10.0), vec2(106.0, 173.0)));
        assert_eq!(room(phone, &[tools], &[cube], vec2(340.0, 255.0), least), Rect::from_x_y_ranges(phone.x_range(), vt + 191.0..=vt + 337.0));
        // Too short under the navigator for the least window: under it all the same, past the view's foot.
        let stub = Rect::from_min_size(pos2(0.0, 100.0), vec2(420.0, 250.0));
        assert_eq!(room(stub, &[rail], &[navigator], size, least), Rect::from_x_y_ranges(stub.x_range(), 308.0..=416.0));
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
                show(ui.ctx(), view, Over { fixed: &[navigator], ..Default::default() }, &mut stamp);
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
                show(ui.ctx(), view, Over { fixed: &[navigator], ..Default::default() }, &mut stamp);
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

    #[test]
    fn with_the_rail_expanded_and_the_keypad_up_the_window_stands_under_the_navigator_over_the_rail_under_a_later_palette_and_its_close_button_takes_the_tap() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut stamp = Stamp { name: "Moon".into(), theta_deg: 44.6, v_mm: 8.01, rot_deg: 180.0, outline: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]], height_mm: 0.34, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false };
        let vt = 80.0;
        let navigator = Rect::from_min_size(pos2(298.0, vt + 10.0), vec2(106.0, 173.0));
        let mut rail = (Rect::NOTHING, egui::LayerId::background());
        let mut taps_on_rail = 0;
        let mut open = true;
        let palette = std::cell::Cell::new(None::<egui::Pos2>);
        let taps_on_palette = std::cell::Cell::new(0);
        let mut pass = |view: Rect, events: Vec<egui::Event>| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(411.0, 900.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                let tools = egui::Area::new(egui::Id::new("test-rail")).order(egui::Order::Foreground).movable(false).fixed_pos(pos2(4.0, vt + 8.0)).show(ui.ctx(), |ui| ui.add_sized([92.0, 340.0], egui::Button::new("Tools")));
                taps_on_rail += usize::from(tools.inner.clicked());
                rail = (tools.response.rect, tools.response.layer_id);
                let mut floating = vec![rail.0];
                if let Some(at) = palette.get() {
                    let shown = egui::Area::new(egui::Id::new("test-palette")).order(egui::Order::Foreground).movable(false).fixed_pos(at).show(ui.ctx(), |ui| ui.add_sized([250.0, 80.0], egui::Button::new("Palette")));
                    taps_on_palette.set(taps_on_palette.get() + usize::from(shown.inner.clicked()));
                    floating.push(shown.response.rect);
                }
                open = show(ui.ctx(), view, Over { floating: &floating, rail: Some(rail.1), fixed: &[navigator] }, &mut stamp).1;
            });
            out.textures_delta.clear();
            (ctx.memory(|m| m.area_rect(id())).unwrap(), rail, taps_on_rail, open)
        };
        let tap = |p: egui::Pos2, pressed| vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }];
        let window_on_top = |rail: egui::LayerId| {
            let order: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
            order.iter().position(|l| l.id == id()) > order.iter().position(|l| *l == rail)
        };
        let tall = Rect::from_min_size(pos2(0.0, vt), vec2(411.0, 565.0));
        for _ in 0..3 {
            pass(tall, vec![]);
        }
        let short = Rect::from_min_size(pos2(0.0, vt), vec2(411.0, 337.0));
        for _ in 0..30 {
            pass(short, vec![]);
        }
        let (window, (tools, layer), _, _) = pass(short, vec![]);
        assert!(tools.height() > short.height(), "{tools:?} spans the view");
        assert!(short.contains_rect(window) && window.top() >= navigator.bottom() + GAP_PT - 0.5, "{window:?} under the navigator");
        assert!(window.intersects(tools) && window_on_top(layer), "{window:?} over the rail");
        // A tap on the rail above the window raises the rail, and the window stands over it again in the same pass.
        let above = pos2(50.0, window.top() - 30.0);
        pass(short, tap(above, true));
        assert!(window_on_top(layer));
        let (_, _, taps, _) = pass(short, tap(above, false));
        assert_eq!(taps, 1);
        assert!(window_on_top(layer));
        // A tap where the window stands over the rail is the window's.
        let covered = pos2(window.left() + 40.0, window.center().y);
        assert_eq!(ctx.layer_id_at(covered).map(|l| l.id), Some(id()));
        pass(short, tap(covered, true));
        let (_, _, taps, still) = pass(short, tap(covered, false));
        assert_eq!((taps, still), (1, true));
        // A palette opened later over the window stands above it and takes its own taps; the window stays over the rail.
        let at = pos2(20.0, window.top() + 20.0);
        palette.set(Some(at));
        pass(short, vec![]);
        let (under, ..) = pass(short, vec![]);
        let order: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
        let (rail_at, window_at, palette_at) = (order.iter().position(|l| *l == layer), order.iter().position(|l| l.id == id()), order.iter().position(|l| l.id == egui::Id::new("test-palette")));
        assert!(rail_at < window_at && window_at < palette_at, "{order:?}");
        let on_palette = at + vec2(125.0, 40.0);
        assert!(under.contains(on_palette), "{under:?}");
        assert_eq!(ctx.layer_id_at(on_palette).map(|l| l.id), Some(egui::Id::new("test-palette")));
        pass(short, tap(on_palette, true));
        let (_, _, taps, still) = pass(short, tap(on_palette, false));
        assert_eq!((taps_on_palette.get(), taps, still), (1, 1, true));
        palette.set(None);
        pass(short, vec![]);
        pass(short, vec![]);
        // The close button, at the title bar's right end.
        let style = ctx.global_style();
        let heading = ctx.fonts_mut(|f| f.row_height(&egui::TextStyle::Heading.resolve(&style)));
        let margin = style.spacing.window_margin;
        let close = pos2(window.right() - f32::from(margin.right) - heading / 2.0, window.top() + f32::from(margin.top) + heading / 2.0);
        assert!(!navigator.contains(close) && short.contains(close), "{close:?}");
        pass(short, tap(close, true));
        let (_, _, _, open) = pass(short, tap(close, false));
        assert!(!open, "{close:?} closes the window");
    }
}
