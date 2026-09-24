//! The soft keyboard a field asks for: the number keypad while a number field or a dimension bar's field holds focus.
use egui_mobile::egui;
use egui::output::OutputEvent;

/// Which focused widget takes numbers, read off egui's focus and each pass's output events.
#[derive(Clone, Copy, Debug, Default)]
pub struct Focus {
    /// The widget focused when the last pass ended.
    last: Option<egui::Id>,
    /// The last widget a tap or a tab on a number field focused.
    numeric: Option<egui::Id>,
}

impl Focus {
    /// Notes this pass's focus and asks for the number keypad while a number field holds it; whether it did.
    pub fn pass(&mut self, ctx: &egui::Context) -> bool {
        let focused = ctx.memory(|m| m.focused());
        if focused != self.last {
            match (focused, focusing(ctx)) {
                (Some(id), Some(true)) => self.numeric = Some(id),
                (Some(id), Some(false)) if self.numeric == Some(id) => self.numeric = None,
                _ => {}
            }
            self.last = focused;
        }
        let on = focused.is_some() && focused == self.numeric;
        if on {
            egui_mobile::keyboard::request_number(ctx);
        }
        on
    }
}

/// Whether this pass's taps and focus moves reached a number field: a DragValue's own event, a slider's value box included; `None` without any.
fn focusing(ctx: &egui::Context) -> Option<bool> {
    ctx.output(|o| {
        let mut seen = None;
        for e in &o.events {
            if let OutputEvent::Clicked(i) | OutputEvent::DoubleClicked(i) | OutputEvent::TripleClicked(i) | OutputEvent::FocusGained(i) = e {
                let number = i.typ == egui::WidgetType::DragValue && i.value.is_some();
                seen = Some(seen.unwrap_or(false) || number);
            }
        }
        seen
    })
}

/// Asks for the number keypad this pass when the field holding focus is drawn on one of `areas`, a command's dimension fields; whether it did.
pub fn fields(ctx: &egui::Context, areas: &[egui::Id]) -> bool {
    let on = ctx.memory(|m| m.focused()).and_then(|f| ctx.read_response(f)).is_some_and(|r| areas.contains(&r.layer_id.id));
    if on {
        egui_mobile::keyboard::request_number(ctx);
    }
    on
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_mobile::keyboard::{KeyboardKind, requested};
    use std::cell::Cell;

    /// One pass of `f` with `events`, `focus` read at its end, and the keyboard asked for.
    fn pass(ctx: &egui::Context, focus: &mut Focus, events: Vec<egui::Event>, mut f: impl FnMut(&mut egui::Ui)) -> KeyboardKind {
        let mut kind = KeyboardKind::Text;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 640.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            f(ui);
            focus.pass(ui.ctx());
            kind = requested(ui.ctx());
        });
        out.textures_delta.clear();
        kind
    }

    fn touch(at: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]
    }

    /// Taps the middle of `rect`: press, release and one quiet pass; the keyboard each asked for.
    fn tap(ctx: &egui::Context, focus: &mut Focus, rect: egui::Rect, draw: &mut impl FnMut(&mut egui::Ui)) -> [KeyboardKind; 3] {
        let at = rect.center();
        [pass(ctx, focus, touch(at, true), &mut *draw), pass(ctx, focus, touch(at, false), &mut *draw), pass(ctx, focus, vec![], &mut *draw)]
    }

    /// The innermost widgets under a pointer run down `x` from `top` to `bottom`, top first.
    fn column(ctx: &egui::Context, focus: &mut Focus, x: f32, (top, bottom): (f32, f32), draw: &mut impl FnMut(&mut egui::Ui)) -> Vec<egui::Rect> {
        let mut found: Vec<egui::Rect> = Vec::new();
        let mut y = top;
        while y < bottom {
            pass(ctx, focus, vec![egui::Event::PointerMoved(egui::pos2(x, y))], &mut *draw);
            let hovered: Vec<egui::Id> = ctx.interaction_snapshot(|s| s.hovered.iter().copied().collect());
            for r in hovered.into_iter().filter_map(|id| ctx.read_response(id)) {
                if !found.contains(&r.rect) {
                    found.push(r.rect);
                }
            }
            y += 2.0;
        }
        // Only the innermost: a row or a panel holds the fields it lays out.
        let leaves: Vec<egui::Rect> = found.iter().copied().filter(|r| !found.iter().any(|o| o != r && r.contains_rect(*o))).collect();
        leaves
    }

    #[test]
    fn a_tapped_value_slider_box_or_stamp_window_field_asks_for_numbers_and_a_name_for_letters() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut focus = Focus::default();
        let (mut value, mut size, mut name) = (0.35_f64, 7.0_f64, String::from("Band"));
        let mut stamp = ringdesign_core::setting::Stamp { name: "Moon".into(), theta_deg: 270.0, v_mm: 0.0, rot_deg: 0.0, outline: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]], height_mm: 0.4, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: Default::default() };
        let rects = Cell::new([egui::Rect::NOTHING; 4]);
        let mut draw = |root: &mut egui::Ui| {
            egui::CentralPanel::default().show(root, |ui| {
                let v = ui.add(egui::DragValue::new(&mut value).suffix(" mm")).rect;
                let s = ui.add(egui::Slider::new(&mut size, 3.0..=13.0)).rect;
                // The slider's value box is its right end.
                let b = egui::Rect::from_min_max(egui::pos2(s.right() - 30.0, s.top()), s.max);
                let n = ui.add(egui::TextEdit::singleline(&mut name)).rect;
                let inspector = ui.scope(|ui| ringdesign_workbench::viewport::made::inspector(ui, &mut stamp)).response.rect;
                rects.set([v, b, n, inspector]);
            });
        };
        pass(&ctx, &mut focus, vec![], &mut draw);
        let [value_box, slider_box, name_box, inspector] = rects.get();
        assert_eq!(tap(&ctx, &mut focus, value_box, &mut draw), [KeyboardKind::Text, KeyboardKind::Number, KeyboardKind::Number], "a millimetre value, from the release on");
        assert_eq!(tap(&ctx, &mut focus, name_box, &mut draw)[1..], [KeyboardKind::Text; 2], "a name");
        assert_eq!(tap(&ctx, &mut focus, slider_box, &mut draw)[1..], [KeyboardKind::Number; 2], "a slider's value box");
        assert_eq!(tap(&ctx, &mut focus, name_box, &mut draw)[1..], [KeyboardKind::Text; 2], "back to the name");
        // The stamp window's rows, their fields right-aligned: the stamp's name, then its angle round the ring.
        let fields = column(&ctx, &mut focus, inspector.right() - 12.0, (inspector.top(), inspector.bottom()), &mut draw);
        assert_eq!(fields.len(), 6, "{fields:?}");
        assert_eq!(tap(&ctx, &mut focus, fields[1], &mut draw)[1..], [KeyboardKind::Number; 2], "the stamp's angle");
        assert!(ctx.memory(|m| m.focused()).is_some());
        assert_eq!(tap(&ctx, &mut focus, fields[0], &mut draw)[1..], [KeyboardKind::Text; 2], "the stamp's name, though its row labels it a DragValue");
        assert!(ctx.memory(|m| m.focused()).is_some());
        assert_eq!(tap(&ctx, &mut focus, fields[4], &mut draw)[1..], [KeyboardKind::Number; 2], "how far it stands");
    }

    #[test]
    fn a_number_field_refocused_without_a_tap_keeps_the_keypad_and_nothing_focused_asks_for_nothing() {
        let ctx = egui::Context::default();
        let mut focus = Focus::default();
        let (mut value, mut name) = (1.5_f64, String::from("Rail"));
        let ids = Cell::new([egui::Id::NULL; 2]);
        let rect = Cell::new(egui::Rect::NOTHING);
        let mut draw = |root: &mut egui::Ui| {
            egui::CentralPanel::default().show(root, |ui| {
                let v = ui.add(egui::DragValue::new(&mut value));
                let n = ui.add(egui::TextEdit::singleline(&mut name));
                rect.set(v.rect);
                ids.set([v.id, n.id]);
            });
        };
        pass(&ctx, &mut focus, vec![], &mut draw);
        assert_eq!(tap(&ctx, &mut focus, rect.get(), &mut draw)[2], KeyboardKind::Number);
        // The runtime drops focus for a pass and pins it back, with no event for either.
        ctx.memory_mut(|m| m.surrender_focus(ids.get()[0]));
        assert_eq!(pass(&ctx, &mut focus, vec![], &mut draw), KeyboardKind::Text);
        ctx.memory_mut(|m| m.request_focus(ids.get()[0]));
        assert_eq!(pass(&ctx, &mut focus, vec![], &mut draw), KeyboardKind::Number, "pinned back");
        // Focus handed to the name without a tap takes no keypad with it.
        ctx.memory_mut(|m| m.request_focus(ids.get()[1]));
        assert_eq!(pass(&ctx, &mut focus, vec![], &mut draw), KeyboardKind::Text);
    }

    #[test]
    fn a_field_on_a_dimension_bar_asks_for_numbers_and_one_elsewhere_does_not() {
        let ctx = egui::Context::default();
        let mut focus = Focus::default();
        let bar = egui::Id::new("dims").with("area");
        let (mut radius, mut note) = (String::from("1.5"), String::new());
        let (on_bar, said) = (Cell::new(true), Cell::new(false));
        let mut draw = |root: &mut egui::Ui| {
            let r = egui::Area::new(bar).order(egui::Order::Foreground).show(root.ctx(), |ui| ui.add(egui::TextEdit::singleline(&mut radius))).inner;
            let n = egui::CentralPanel::default().show(root, |ui| ui.add(egui::TextEdit::singleline(&mut note))).inner;
            if on_bar.get() { r.request_focus() } else { n.request_focus() }
            said.set(fields(root.ctx(), &[bar]));
        };
        assert_eq!(pass(&ctx, &mut focus, vec![], &mut draw), KeyboardKind::Number);
        assert!(said.get());
        on_bar.set(false);
        assert_eq!(pass(&ctx, &mut focus, vec![], &mut draw), KeyboardKind::Text);
        assert!(!said.get());
    }
}
