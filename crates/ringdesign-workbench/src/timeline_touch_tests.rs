use super::*;
use egui_kittest::{Harness, kittest::Queryable};

#[derive(Default)]
struct Phone {
    actions: Vec<Action>,
    offset: f32,
}

fn phone() -> Harness<'static, Phone> {
    let chips: Vec<_> = (1..=14).map(|id| Chip {
        id, name: format!("Feature {id}"), icon: Icon::CadBox,
        status: Status::Ok, enabled: true, selected: false,
        after_rollback: false, dependents: vec![], sources: vec![],
        is_band: false, hint: "A feature".into(),
    }).collect();
    let mut h = Harness::builder().with_size([320.0, 140.0]).with_step_dt(0.05)
        .build_ui_state(move |ui, phone: &mut Phone| {
            ui.spacing_mut().interact_size.y = 32.0;
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            let scroll = egui::ScrollArea::horizontal()
                .scroll_source(egui::scroll_area::ScrollSource::ALL)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .auto_shrink([false, true])
                .show(ui, |ui| show_touch(ui, &chips, None));
            phone.offset = scroll.state.clamped_offset().x;
            phone.actions.extend(scroll.inner);
        }, Phone::default());
    h.run_steps(3); // Hit testing is seeded by an idle frame with no touches.
    h
}

fn pointer(h: &mut Harness<'_, Phone>, pos: Pos2, pressed: bool) {
    h.event(egui::Event::PointerMoved(pos));
    h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    h.run_steps(1);
}

#[test]
fn first_swipe_starts_on_a_chip_and_scrolls_without_selecting_or_reordering() {
    let mut h = phone();
    let start = h.get_by_label("Feature 2 · ok").rect().center();
    pointer(&mut h, start, true);
    for step in 1..=5 {
        h.event(egui::Event::PointerMoved(start - Vec2::new(step as f32 * 20.0, 0.0)));
        h.run_steps(1);
    }
    pointer(&mut h, start - Vec2::new(100.0, 0.0), false);
    assert!(h.state().offset > 60.0, "chip captured the first swipe: {}", h.state().offset);
    assert!(h.state().actions.is_empty(), "swiping edited the history: {:?}", h.state().actions);
    assert!(!egui::DragAndDrop::has_any_payload(&h.ctx));
}

#[test]
fn touch_strip_keeps_tap_and_feature_menu_actions() {
    let mut h = phone();
    h.get_by_label("Feature 1 · ok").click();
    h.run_steps(2);
    assert_eq!(std::mem::take(&mut h.state_mut().actions), [Action::Select(1)]);
    h.get_by_label("Feature 1 · ok").click_secondary();
    h.run_steps(2);
    h.get_by_label("Suppress").click();
    h.run_steps(2);
    assert_eq!(h.state().actions, [Action::Enable(1, false)]);
}
