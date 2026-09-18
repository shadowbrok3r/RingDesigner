use super::*;

#[test]
fn bottom_inspector_hugs_content_and_never_exceeds_half_the_workspace() {
    assert_eq!(content_height(720.0, Some(96.0)), 96.0);
    assert_eq!(content_height(720.0, Some(900.0)), 360.0);
    assert_eq!(content_height(720.0, Some(60.0)), 60.0);
    assert_eq!(content_height(240.0, Some(900.0)), 120.0);
    assert_eq!(content_height(720.0, None), 360.0);
    assert_eq!(content_height(720.0, Some(f32::NAN)), 360.0);
    assert_eq!(content_height(0.0, Some(96.0)), 0.0);
}

#[test]
fn keyboard_close_restores_layout_without_another_touch_then_stops_polling() {
    let full = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 680.0));
    let keyboard = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 420.0));
    let mut reflow = Reflow::default();
    assert!(reflow.next_frame(0.0, full, 0.0).is_some());
    assert!(reflow.next_frame(1.0, full, 0.0).is_none());
    assert!(reflow.next_frame(2.0, keyboard, 260.0).unwrap().as_millis() <= 16);
    assert!(reflow.next_frame(3.0, keyboard, 260.0).unwrap().as_millis() <= 250);
    // An inset callback may wake us before the GL surface finishes restoring.
    assert!(reflow.next_frame(4.0, keyboard, 0.0).is_some());
    assert!(reflow.next_frame(4.1, full, 0.0).is_some());
    assert!(reflow.next_frame(5.0, full, 0.0).is_none());
}

fn pointer(at: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn frame(ctx: &egui::Context, events: Vec<egui::Event>, content: impl FnMut(&mut egui::Ui)) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(320.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        content,
    );
    output.textures_delta.clear();
}

#[test]
fn numeric_swipes_scroll_without_edits_but_tapping_still_types() {
    for slider_value in [false, true] {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut value: f64 = 17.3;
        let mut hit = egui::Rect::NOTHING;
        let mut offset = 0.0;
        let mut draw = |events: Vec<egui::Event>| {
            frame(&ctx, events, |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    ui.spacing_mut().slider_width = 110.0;
                    let scroll = egui::ScrollArea::vertical()
                        .scroll_source(egui::containers::scroll_area::ScrollSource {
                            drag: egui::containers::scroll_area::DragScroll::Always,
                            ..Default::default()
                        })
                        .max_height(250.0)
                        .show(ui, |ui| {
                            ui.add_space(110.0);
                            hit = if slider_value {
                                let r = ui.add(egui::Slider::new(&mut value, 13.0..=25.0));
                                egui::Rect::from_min_max(
                                    egui::pos2(r.rect.right() - 30.0, r.rect.top()),
                                    r.rect.max,
                                )
                            } else {
                                ui.add(
                                    egui::DragValue::new(&mut value)
                                        .range(13.0..=25.0)
                                        .speed(0.1),
                                )
                                .rect
                            };
                            for _ in 0..30 {
                                ui.label("Scroll this inspector");
                            }
                        });
                    offset = scroll.state.offset.y;
                });
            });
            (hit, offset, value)
        };
        // ScrollArea learns its content bounds before registering a drag surface.
        draw(vec![]);
        let (rect, _, _) = draw(vec![]);
        let start = rect.center();
        draw(pointer(start, true));
        draw(vec![egui::Event::PointerMoved(
            start - egui::vec2(0.0, 65.0),
        )]);
        let (_, offset, after) = draw(pointer(start - egui::vec2(0.0, 65.0), false));
        assert_eq!(
            after, 17.3,
            "a swipe altered the numeric value (slider={slider_value})"
        );
        assert!(
            offset > 25.0,
            "the numeric field trapped the scroll gesture (slider={slider_value}, offset={offset})"
        );
        let (rect, _, _) = draw(vec![]);
        draw(pointer(rect.center(), true));
        draw(pointer(rect.center(), false));
        draw(vec![]);
        let (_, _, after) = draw(vec![egui::Event::Text("18.23".into())]);
        assert!(
            (after - 18.23).abs() < 1e-6,
            "exact entry failed (slider={slider_value}): {after}"
        );
    }
}

#[test]
fn slider_rails_remain_draggable() {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let mut value = 0.2;
    let mut rect = egui::Rect::NOTHING;
    let mut draw = |events| {
        frame(&ctx, events, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                ui.spacing_mut().slider_width = 200.0;
                rect = ui
                    .add(egui::Slider::new(&mut value, 0.0..=1.0).show_value(false))
                    .rect;
            });
        });
        (rect, value)
    };
    let (r, _) = draw(vec![]);
    draw(pointer(r.left_center() + egui::vec2(45.0, 0.0), true));
    draw(vec![egui::Event::PointerMoved(
        r.right_center() - egui::vec2(25.0, 0.0),
    )]);
    let (_, value) = draw(pointer(r.right_center() - egui::vec2(25.0, 0.0), false));
    assert!(value > 0.7);
}

#[test]
fn floating_header_moves_and_clamps_without_moving_on_button_drags() {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let bounds = egui::Rect::from_min_max(egui::pos2(8.0, 30.0), egui::pos2(312.0, 540.0));
    let mut saved = None;
    let mut target = egui::Rect::NOTHING;
    let mut draw = |events| {
        let mut outer = egui::Rect::NOTHING;
        frame(&ctx, events, |root| {
            let f = floating(
                root.ctx(),
                "test-rail",
                "Tools",
                bounds,
                egui::vec2(92.0, 140.0),
                egui::Vec2::ZERO,
                &mut saved,
                None,
                |ui| {
                    target = ui
                        .add_sized([ui.available_width(), 32.0], egui::Button::new("Select"))
                        .rect;
                },
            );
            outer = f.rect;
        });
        (outer, target, saved)
    };
    draw(vec![]);
    let (initial, button, _) = draw(vec![]);
    draw(pointer(button.center(), true));
    draw(vec![egui::Event::PointerMoved(
        button.center() + egui::vec2(100.0, 100.0),
    )]);
    let (after, _, saved) = draw(pointer(button.center() + egui::vec2(100.0, 100.0), false));
    assert_eq!(saved, None, "button drag moved toolbar");
    assert_eq!(initial.min, after.min);
    let grip = initial.min + egui::vec2(35.0, 16.0);
    draw(pointer(grip, true));
    draw(vec![egui::Event::PointerMoved(egui::pos2(900.0, 900.0))]);
    draw(pointer(egui::pos2(900.0, 900.0), false));
    let (after, _, saved) = draw(vec![]);
    assert!(saved.is_some(), "grip did not move toolbar");
    assert!(after.left() > initial.left() + 100.0);
    assert!(
        bounds.expand(1.0).contains_rect(after),
        "toolbar escaped viewport: {after:?}"
    );
}

#[test]
fn panel_resize_uses_press_origin_and_keeps_viewport_space() {
    for landscape in [false, true] {
        let ctx = egui::Context::default();
        let available = if landscape {
            egui::vec2(800.0, 300.0)
        } else {
            egui::vec2(320.0, 560.0)
        };
        let mut fraction = 0.32;
        let mut draw = |events| {
            frame(&ctx, events, |root| {
                splitter(root, &mut fraction, available, landscape);
            });
            fraction
        };
        draw(vec![]);
        let start = egui::pos2(150.0, 8.0);
        draw(pointer(start, true));
        let end = start
            - if landscape {
                egui::vec2(80.0, 0.0)
            } else {
                egui::vec2(0.0, 80.0)
            };
        draw(vec![egui::Event::PointerMoved(end)]);
        let changed = draw(pointer(end, false));
        assert!(changed > 0.4, "resize ignored movement: {changed}");
        let extent = inspector_extent(available, landscape, changed);
        assert!(
            extent
                < if landscape {
                    available.x - 219.0
                } else {
                    available.y - 189.0
                }
        );
    }
    let saved = 0.65;
    assert_eq!(
        inspector_extent(egui::vec2(320.0, 240.0), false, saved),
        50.0
    );
    assert_eq!(
        saved, 0.65,
        "keyboard constraints should not rewrite the preference"
    );
}

#[test]
fn workspace_preferences_round_trip_and_sanitize_invalid_positions() {
    let mut p = crate::prefs::Prefs::default();
    p.workspace.inspector_fraction = [0.22, 0.51];
    p.workspace.rail_position = Some([0.0, 0.75]);
    p.workspace.palette_position = Some([0.8, 0.2]);
    let back: crate::prefs::Prefs =
        serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(p, back);
    p.workspace.rail_position = Some([f32::NAN, 0.0]);
    p.workspace.inspector_fraction = [f32::INFINITY, -4.0];
    p.workspace.sanitize();
    assert_eq!(p.workspace.rail_position, None);
    assert!(
        p.workspace
            .inspector_fraction
            .iter()
            .all(|v| v.is_finite() && *v > 0.0)
    );
}

#[test]
fn floating_submenus_can_grow_after_an_initial_short_layout() {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let bounds = egui::Rect::from_min_size(egui::pos2(8.0, 30.0), egui::vec2(300.0, 500.0));
    let mut saved = None;
    let mut draw = |rows| {
        let mut size = egui::Vec2::ZERO;
        frame(&ctx, vec![], |root| {
            size = floating(
                root.ctx(),
                "growing-palette",
                "Tools",
                bounds,
                egui::vec2(220.0, 300.0),
                egui::Vec2::ZERO,
                &mut saved,
                Some("×"),
                |ui| {
                    for _ in 0..rows {
                        ui.add_sized([ui.available_width(), 32.0], egui::Button::new("Property"));
                    }
                },
            )
            .rect
            .size();
        });
        size
    };
    draw(1);
    let small = draw(1);
    draw(7);
    let expanded = draw(7);
    assert!(
        expanded.y > small.y + 100.0,
        "expanded controls stayed trapped in the initial area: {small:?} -> {expanded:?}"
    );
    assert!(expanded.y <= 301.0);
}

#[test]
fn a_focused_palette_shows_one_parameter_without_clamping_other_values() {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let mut editor = crate::editor::Editor::default();
    editor.active_field = "Finger opening".into();
    let mut design = ringdesign_core::RingDesign::default();
    design.profile.width_mm = 25.0;
    let before = serde_json::to_value(&design).unwrap();
    let key = egui::Id::new("focused-test");
    for _ in 0..2 {
        frame(&ctx, vec![], |root| {
            crate::editor::layout::begin(root.ctx());
            crate::editor::controls::begin_compact(root, key, &mut editor.active_field);
            crate::editor::controls::shape(root, &mut editor, &mut design);
            crate::editor::controls::end_compact(root, key, &mut editor.active_field);
        });
    }
    let report = crate::editor::layout::report(&ctx, ctx.content_rect());
    let values: Vec<_> = report["controls"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["label"].as_str().unwrap().starts_with("value/"))
        .collect();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["label"], "value/Finger opening");
    assert_eq!(before, serde_json::to_value(&design).unwrap());
}

#[test]
fn stone_options_fit_a_narrow_floating_palette_with_submenu_open_or_closed() {
    for width in [200.0, 250.0] {
        for expanded in [false, true] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            ctx.memory_mut(|m| m.set_everything_is_visible(expanded));
            let mut editor = crate::editor::Editor::default();
            editor.active_field = "Stone width".into();
            let mut design = ringdesign_core::RingDesign::default();
            let mut seat = ringdesign_core::field::SeatPadLayer::default();
            seat.fit_stone(ringdesign_core::gem::Gem::calibrated(
                ringdesign_core::gem::GemCut::Round,
                4.0,
            ));
            design
                .layers
                .layers
                .push(ringdesign_core::field::LayerEntry::new(
                    "Central emerald setting",
                    ringdesign_core::field::Layer::SeatPad(seat),
                ));
            let before = serde_json::to_value(&design).unwrap();
            let mut selected = Some(0);
            let mut at = Some([1.0, 1.0]);
            let bounds = egui::Rect::from_min_max(egui::pos2(8.0, 30.0), egui::pos2(312.0, 570.0));
            let mut rect = egui::Rect::NOTHING;
            for _ in 0..4 {
                frame(&ctx, vec![], |root| {
                    rect = floating(
                        root.ctx(),
                        "narrow-stone-palette",
                        "Stones",
                        bounds,
                        egui::vec2(width, 300.0),
                        egui::Vec2::ZERO,
                        &mut at,
                        Some("×"),
                        |ui| {
                            let key = egui::Id::new("narrow-stone-fields");
                            crate::editor::controls::begin_compact(
                                ui,
                                key,
                                &mut editor.active_field,
                            );
                            crate::editor::controls::stones(
                                ui,
                                &mut editor,
                                &mut design,
                                &mut selected,
                            );
                            crate::editor::controls::end_compact(ui, key, &mut editor.active_field);
                        },
                    )
                    .rect;
                });
            }
            assert!(
                rect.width() <= width + 1.0,
                "stone controls grew beyond {width}pt: {rect:?}, expanded={expanded}"
            );
            assert!(
                bounds.expand(1.0).contains_rect(rect),
                "palette escaped viewport: {rect:?}"
            );
            assert_eq!(before, serde_json::to_value(&design).unwrap());
        }
    }
}
