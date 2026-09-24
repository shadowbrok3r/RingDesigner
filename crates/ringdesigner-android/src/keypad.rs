//! The soft keyboard a field asks for: the number keypad for every value the phone takes as a number, whose Done arrives as Enter.
use egui_mobile::egui;

/// `response`'s field asks for the number keypad while it holds focus; the response back.
pub fn number(response: egui::Response) -> egui::Response {
    egui_mobile::keyboard::number(&response);
    response
}

/// A widget's response that can ask for the number keypad.
pub trait Numeric {
    /// The field asks for the number keypad while it holds focus.
    fn numeric(self) -> Self;
}

impl Numeric for egui::Response {
    fn numeric(self) -> Self {
        number(self)
    }
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

    /// One pass of `f` with `events`, and the keyboard its marks ask for once it has drawn.
    fn pass(ctx: &egui::Context, events: Vec<egui::Event>, mut f: impl FnMut(&mut egui::Ui)) -> KeyboardKind {
        let mut kind = KeyboardKind::Text;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 640.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            f(ui);
            kind = requested(ui.ctx());
        });
        out.textures_delta.clear();
        kind
    }

    fn touch(at: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]
    }

    #[test]
    fn a_tapped_value_or_slider_box_asks_for_numbers_and_a_tapped_name_for_letters() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut value, mut size, mut name) = (0.35_f64, 7.0_f64, String::from("Band"));
        let rects = Cell::new([egui::Rect::NOTHING; 3]);
        let mut draw = |root: &mut egui::Ui| {
            egui::CentralPanel::default().show(root, |ui| {
                let v = number(ui.add(egui::DragValue::new(&mut value).suffix(" mm"))).rect;
                let s = number(ui.add(egui::Slider::new(&mut size, 3.0..=13.0))).rect;
                // The slider's value box is its right end.
                let b = egui::Rect::from_min_max(egui::pos2(s.right() - 30.0, s.top()), s.max);
                let n = ui.add(egui::TextEdit::singleline(&mut name)).rect;
                rects.set([v, b, n]);
            });
        };
        pass(&ctx, vec![], &mut draw);
        let mut tap = |i: usize| {
            let at = rects.get()[i].center();
            pass(&ctx, touch(at, true), &mut draw);
            pass(&ctx, touch(at, false), &mut draw);
            pass(&ctx, vec![], &mut draw)
        };
        assert_eq!(tap(0), KeyboardKind::Number, "a millimetre value");
        assert_eq!(tap(2), KeyboardKind::Text, "a name");
        assert_eq!(tap(1), KeyboardKind::Number, "a slider's value box");
        assert_eq!(tap(2), KeyboardKind::Text, "back to the name");
    }

    #[test]
    fn a_field_on_a_dimension_bar_asks_for_numbers_and_one_elsewhere_does_not() {
        let ctx = egui::Context::default();
        let bar = egui::Id::new("dims").with("area");
        let (mut radius, mut note) = (String::from("1.5"), String::new());
        let (on_bar, said) = (Cell::new(true), Cell::new(false));
        let mut draw = |root: &mut egui::Ui| {
            let r = egui::Area::new(bar).order(egui::Order::Foreground).show(root.ctx(), |ui| ui.add(egui::TextEdit::singleline(&mut radius))).inner;
            let n = egui::CentralPanel::default().show(root, |ui| ui.add(egui::TextEdit::singleline(&mut note))).inner;
            if on_bar.get() { r.request_focus() } else { n.request_focus() }
            said.set(fields(root.ctx(), &[bar]));
        };
        assert_eq!(pass(&ctx, vec![], &mut draw), KeyboardKind::Number);
        assert!(said.get());
        on_bar.set(false);
        assert_eq!(pass(&ctx, vec![], &mut draw), KeyboardKind::Text);
        assert!(!said.get());
    }
}
