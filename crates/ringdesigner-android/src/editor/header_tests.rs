use super::*;
use egui_mobile::egui;

#[test]
fn inline_header_resizes_without_stealing_the_close_button() {
    for landscape in [false, true] {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut fraction = 0.32;
        let mut draw = |events| {
            let mut closed = false;
            let mut output = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 600.0))),
                events, ..Default::default()
            }, |ui| {
                crate::editor::layout::begin(ui.ctx());
                ui.add_space(100.0);
                closed = inspector_header(ui, "Casting findings", &mut fraction, egui::vec2(800.0, 560.0), landscape).close;
            });
            output.textures_delta.clear();
            let report = crate::editor::layout::report(&ctx, ctx.content_rect());
            let rect = |label| {
                let values = report["controls"].as_array().unwrap().iter().find(|c| c["label"] == label).unwrap()["rect"].as_array().unwrap();
                egui::Rect::from_min_max(egui::pos2(values[0].as_f64().unwrap() as f32, values[1].as_f64().unwrap() as f32), egui::pos2(values[2].as_f64().unwrap() as f32, values[3].as_f64().unwrap() as f32))
            };
            (rect("inspector/resize"), rect("inspector/close"), fraction, closed)
        };
        draw(vec![]);
        let (grip, close, _, _) = draw(vec![]);
        assert!((grip.top() - close.top()).abs() < 0.5);
        assert!((grip.height() - 32.0).abs() < 0.5);
        assert!(!grip.intersects(close));
        let pointer = |pos, pressed| vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }];
        let start = grip.center();
        draw(pointer(start, true));
        let end = start - if landscape { egui::vec2(80.0, 0.0) } else { egui::vec2(0.0, 80.0) };
        draw(vec![egui::Event::PointerMoved(end)]);
        let (_, _, resized, closed) = draw(pointer(end, false));
        assert!(resized > 0.4 && !closed);
        draw(pointer(close.center(), true));
        let (_, _, after, closed) = draw(pointer(close.center(), false));
        assert!(closed, "close was captured by the resize grip");
        assert_eq!(resized, after);
    }
}
