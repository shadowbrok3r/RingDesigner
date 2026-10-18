use super::*;
use crate::fonts::{self, TextScale};
use crate::metrics::{self, Density};
use crate::presets::{MATCHABLE_PRESETS, apply_preset, bootstrap_startup_theme, set_density};
use eframe::egui::{
    CentralPanel, Context, Event, FontDefinitions, Modifiers, PointerButton, Pos2, RawInput, Shape,
    Style, Vec2, pos2, vec2,
};

fn context(density: Density) -> Context {
    let ctx = Context::default();
    let mut definitions = FontDefinitions::default();
    fonts::install(&mut definitions);
    ctx.set_fonts(definitions);
    metrics::set_density(&ctx, density);
    metrics::install_overlaid_style(&ctx, Style::default());
    ctx
}

fn themed_context() -> Context {
    let ctx = Context::default();
    ctx.set_fonts(fonts::definitions(TextScale::Normal));
    bootstrap_startup_theme(&ctx);
    ctx
}

fn input(events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0))),
        events,
        ..Default::default()
    }
}

/// Runs `f` for `passes` frames and returns what its last pass produced.
fn run<R>(ctx: &Context, passes: usize, mut f: impl FnMut(&mut Ui) -> R) -> R {
    let mut out = None;
    for _ in 0..passes {
        let mut full = ctx.run_ui(input(Vec::new()), |ui| {
            CentralPanel::default().show(ui, |ui| out = Some(f(ui)));
        });
        full.textures_delta.clear();
    }
    out.expect("the ui closure ran")
}

fn run_with(ctx: &Context, events: Vec<Event>, mut f: impl FnMut(&mut Ui)) {
    let mut full = ctx.run_ui(input(events), |ui| {
        CentralPanel::default().show(ui, |ui| f(ui));
    });
    full.textures_delta.clear();
}

/// The first row's rect that leaves `height` or the shared centre line.
fn misaligned(rects: &[Rect], height: f32) -> Option<Rect> {
    let centre = rects.first()?.center().y;
    rects
        .iter()
        .copied()
        .find(|r| (r.height() - height).abs() > 0.01 || (r.center().y - centre).abs() > 0.01)
}

fn row(ui: &mut Ui) -> Vec<Rect> {
    let mut text = "Text".to_owned();
    let mut size = 20.0_f32;
    let mut choice = 0_usize;
    let mut on = true;
    ui.horizontal(|ui| {
        vec![
            button(ui, "Apply").rect,
            icon_button(ui, icons::GEAR, IconBox::Md, false, "Settings").rect,
            text_field(ui, &mut text, "", 120.0).rect,
            value_field(ui, &mut size, 1.0..=100.0, "px", 70.0).rect,
            dropdown(ui, "row", &mut choice, &[(0, "One"), (1, "Two")], 100.0).rect,
            toggle(ui, &mut on, "Live").rect,
        ]
    })
    .inner
}

#[test]
fn row_controls_share_control_height_and_centre_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        let rects = run(&ctx, 2, row);
        let height = Metrics::for_density(density).control_h;
        assert_eq!(rects.len(), 6);
        assert_eq!(misaligned(&rects, height), None, "{density:?}: {rects:?}");
    }
}

#[test]
fn the_alignment_check_catches_a_control_off_the_row() {
    let ctx = context(Density::Regular);
    let mut rects = run(&ctx, 2, row);
    let height = Metrics::for_density(Density::Regular).control_h;
    rects.push(rects[0].translate(vec2(0.0, 1.0)));
    assert!(misaligned(&rects, height).is_some());
    rects.pop();
    rects.push(Rect::from_min_size(rects[0].min, vec2(30.0, height + 2.0)));
    assert!(misaligned(&rects, height).is_some());
}

#[test]
fn prominent_buttons_take_the_button_height() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let heights = run(&ctx, 1, |ui| {
            [
                primary_button(ui, "Save").rect.height(),
                secondary_button(ui, "Cancel").rect.height(),
                danger_button(ui, "Delete").rect.height(),
                ghost_button(ui, "More").rect.height(),
            ]
        });
        assert_eq!(heights, [m.button_h, m.button_h, m.button_h, m.control_h]);
    }
}

#[test]
fn every_button_kind_takes_its_height_and_a_text_button_wraps_its_label() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let rects = run(&ctx, 2, |ui| {
            ButtonKind::ALL.map(|kind| {
                let plain = TextButton::new("Go").kind(kind).show(ui).rect;
                let tall = TextButton::new("Go")
                    .kind(kind)
                    .prominent(true)
                    .show(ui)
                    .rect;
                (plain, tall)
            })
        });
        for (kind, (plain, tall)) in ButtonKind::ALL.into_iter().zip(rects) {
            assert_eq!(plain.height(), m.control_h, "{kind:?} {density:?}");
            assert_eq!(tall.height(), m.button_h, "{kind:?} {density:?}");
            assert!(plain.width() >= 2.0 * m.pad.x, "{kind:?} {density:?}");
        }
    }
}

#[test]
fn icon_boxes_are_square_and_md_matches_the_control_height_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let sizes = run(&ctx, 1, |ui| {
            [IconBox::Sm, IconBox::Md, IconBox::Lg, IconBox::Tool]
                .map(|b| icon_button(ui, icons::GEAR, b, false, "").rect.size())
        });
        assert_eq!(sizes[1], Vec2::splat(m.control_h), "{density:?}");
        assert_eq!(sizes[2], Vec2::splat(m.button_h), "{density:?}");
        for size in sizes {
            assert_eq!(size.x, size.y, "{density:?}");
            assert_eq!(size.x, size.x.round(), "{density:?}");
        }
    }
}

#[test]
fn bars_hold_their_height_whatever_they_contain() {
    let ctx = context(Density::Regular);
    let m = Metrics::for_density(Density::Regular);
    for kind in [
        BarKind::Tool,
        BarKind::Header,
        BarKind::Status,
        BarKind::Footer,
    ] {
        let (bar_rect, inner) = run(&ctx, 1, |ui| {
            let top = ui.cursor().top();
            let inner = bar(ui, kind, |ui| {
                icon_button(ui, icons::GEAR, IconBox::Md, false, "").rect
            });
            (
                Rect::from_min_max(
                    pos2(0.0, top),
                    pos2(1.0, ui.cursor().top() - ui.spacing().item_spacing.y),
                ),
                inner,
            )
        });
        assert_eq!(bar_rect.height(), kind.height(&m), "{kind:?}");
        assert!(
            (inner.center().y - bar_rect.center().y).abs() < 0.01,
            "{kind:?} row is off centre"
        );
    }
}

#[test]
fn dialog_buttons_put_the_default_answer_first_and_cancel_last() {
    let buttons = [
        (Role::Cancel, "Cancel"),
        (Role::Secondary, "Don't save"),
        (Role::Primary, "Save"),
    ];
    assert_eq!(layout::dialog_order(&buttons), [2, 1, 0]);
    let danger = [(Role::Cancel, "Keep"), (Role::Danger, "Delete")];
    assert_eq!(layout::dialog_order(&danger), [1, 0]);
}

#[test]
fn a_toggle_flips_when_clicked() {
    let ctx = context(Density::Regular);
    let mut on = false;
    let mut at = Rect::NOTHING;
    run_with(&ctx, Vec::new(), |ui| at = toggle(ui, &mut on, "Live").rect);
    let p = at.center();
    let press = |pressed| Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run_with(&ctx, vec![Event::PointerMoved(p), press(true)], |ui| {
        let _ = toggle(ui, &mut on, "Live");
    });
    run_with(&ctx, vec![press(false)], |ui| {
        let _ = toggle(ui, &mut on, "Live");
    });
    assert!(on);
}

#[test]
fn a_preview_state_is_scoped_to_its_closure() {
    let ctx = context(Density::Regular);
    run(&ctx, 1, |ui| {
        let response = button(ui, "x");
        assert_eq!(state_of(ui, &response), (State::Rest, false));
        with_preview(ui, Preview::Pressed, |ui| {
            assert_eq!(state_of(ui, &response), (State::Pressed, false));
            with_preview(ui, Preview::Focused, |ui| assert!(preview_focused(ui)));
            assert!(!preview_focused(ui));
        });
        assert_eq!(state_of(ui, &response), (State::Rest, false));
    });
}

#[test]
fn the_gallery_draws_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        run(&ctx, 2, gallery::show);
    }
}

#[test]
fn the_gallery_draws_under_every_preset_and_density() {
    let ctx = themed_context();
    for preset in MATCHABLE_PRESETS {
        apply_preset(&ctx, preset);
        for density in Density::ALL {
            set_density(&ctx, density);
            run(&ctx, 2, |ui| {
                gallery::show_with(ui, &[("glyph", Icon::glyph("A"))])
            });
        }
    }
}

#[test]
fn the_kit_draws_without_the_weight_families() {
    let ctx = Context::default();
    metrics::install_overlaid_style(&ctx, Style::default());
    run(&ctx, 1, gallery::show);
}

fn click_at(ctx: &Context, p: Pos2, mut f: impl FnMut(&mut Ui)) {
    let press = |pressed| Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run_with(ctx, vec![Event::PointerMoved(p), press(true)], &mut f);
    run_with(ctx, vec![press(false)], &mut f);
}

#[test]
fn a_closed_section_stays_closed_until_clicked_and_a_forced_state_wins() {
    let ctx = context(Density::Regular);
    let mut header = Rect::NOTHING;
    let mut opened = false;
    run_with(&ctx, Vec::new(), |ui| {
        let top = ui.cursor().top();
        opened = Section::new("s", "Registers")
            .default_open(false)
            .header(ui);
        header = Rect::from_min_max(pos2(0.0, top), pos2(100.0, ui.cursor().top()));
    });
    assert!(!opened);
    click_at(&ctx, header.center(), |ui| {
        opened = Section::new("s", "Registers")
            .default_open(false)
            .header(ui);
    });
    run_with(&ctx, Vec::new(), |ui| {
        opened = Section::new("s", "Registers")
            .default_open(false)
            .header(ui);
    });
    assert!(opened, "a click opens it");
    run_with(&ctx, Vec::new(), |ui| {
        opened = Section::new("s", "Registers").open(Some(false)).header(ui);
    });
    assert!(!opened, "a forced state wins");
}

#[test]
fn a_section_stores_its_state_where_a_collapsing_header_would() {
    let ctx = context(Density::Regular);
    let id = run(&ctx, 1, |ui| {
        let _ = Section::new("salt", "T").default_open(false).header(ui);
        ui.make_persistent_id(eframe::egui::IdSalt::new("salt"))
    });
    let stored = eframe::egui::collapsing_header::CollapsingState::load(&ctx, id);
    assert!(stored.is_some_and(|s| !s.is_open()));
}

#[test]
fn a_segment_click_picks_its_value() {
    let ctx = context(Density::Regular);
    let mut value = 0_u8;
    let segments = [Segment::new(0_u8, "One"), Segment::new(1, "Two")];
    let mut rect = Rect::NOTHING;
    run_with(&ctx, Vec::new(), |ui| {
        rect = segmented(ui, &mut value, &segments).rect;
    });
    assert_eq!(
        rect.height(),
        Metrics::for_density(Density::Regular).control_h
    );
    let right = pos2(rect.right() - 4.0, rect.center().y);
    let mut changed = false;
    click_at(&ctx, right, |ui| {
        changed |= segmented(ui, &mut value, &segments).changed();
    });
    assert_eq!(value, 1);
    assert!(changed);
}

#[test]
fn a_slider_click_sets_the_value_under_the_pointer() {
    let ctx = context(Density::Regular);
    let mut value = 0.0_f32;
    let mut rect = Rect::NOTHING;
    run_with(&ctx, Vec::new(), |ui| {
        rect = Slider::new(&mut value, 0.0..=10.0)
            .width(200.0)
            .step(1.0)
            .show(ui)
            .rect;
    });
    click_at(&ctx, rect.right_center() - vec2(7.0, 0.0), |ui| {
        let _ = Slider::new(&mut value, 0.0..=10.0)
            .width(200.0)
            .step(1.0)
            .show(ui);
    });
    assert_eq!(value, 10.0);
}

#[test]
fn a_disabled_button_ignores_clicks() {
    let ctx = context(Density::Regular);
    let mut rect = Rect::NOTHING;
    let mut clicked = false;
    run_with(&ctx, Vec::new(), |ui| {
        rect = TextButton::new("Go").enabled(false).show(ui).rect;
    });
    click_at(&ctx, rect.center(), |ui| {
        clicked |= TextButton::new("Go").enabled(false).show(ui).clicked();
    });
    assert!(!clicked);
    run_with(&ctx, Vec::new(), |ui| {
        let _ = TextButton::new("Go").show(ui);
    });
    click_at(&ctx, rect.center(), |ui| {
        clicked |= TextButton::new("Go").show(ui).clicked();
    });
    assert!(clicked, "the enabled button at the same spot clicks");
}

#[test]
fn row_kit_additions_share_the_control_height() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let heights = run(&ctx, 2, |ui| {
            let mut v = 1.0_f32;
            let mut seg = 0_u8;
            let mut pick = 9_u8;
            ui.horizontal(|ui| {
                [
                    ProgressBar::new(0.5).width(80.0).show(ui).rect.height(),
                    Slider::new(&mut v, 0.0..=2.0).show(ui).rect.height(),
                    segmented(ui, &mut seg, &[Segment::new(0, "A")])
                        .rect
                        .height(),
                    Pill::new("P")
                        .tone(Color32::RED)
                        .selected(true)
                        .show(ui)
                        .rect
                        .height(),
                    Dropdown::new("d", &mut pick, &[(0, "x")])
                        .placeholder("Pick")
                        .show(ui)
                        .rect
                        .height(),
                ]
            })
            .inner
        });
        assert!(
            heights.iter().all(|h| *h == m.control_h),
            "{density:?}: {heights:?}"
        );
    }
}

#[test]
fn list_rows_menu_rows_and_headers_take_their_metrics_height() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let heights = run(&ctx, 2, |ui| {
            [
                ListItem::new("Row")
                    .icon(icons::FILE)
                    .show(ui)
                    .rect
                    .height(),
                menu_body(ui, |ui| MenuItem::new("Item").show(ui).rect.height()),
                fold_header(ui, true, icons::FOLDER, "Header").rect.height(),
                checkbox(ui, &mut true, "Check").rect.height(),
                toggle(ui, &mut true, "Switch").rect.height(),
            ]
        });
        assert_eq!(
            heights,
            [m.row_h, m.row_h, m.header_h, m.control_h, m.control_h],
            "{density:?}"
        );
    }
}

#[test]
fn a_text_area_grows_with_its_rows() {
    let ctx = context(Density::Regular);
    let (two, six) = run(&ctx, 1, |ui| {
        let mut a = String::new();
        let mut b = String::new();
        (
            TextArea::new(&mut a).rows(2).width(300.0).show(ui).rect,
            TextArea::new(&mut b).rows(6).width(300.0).show(ui).rect,
        )
    });
    assert_eq!(two.width(), 300.0);
    assert!(six.height() > two.height() + 3.0 * 12.0, "{two:?} {six:?}");
}

#[test]
fn a_value_field_formatter_shows_its_text() {
    let ctx = context(Density::Regular);
    let mut threads = 0_u32;
    let mut full = ctx.run_ui(input(Vec::new()), |ui| {
        let _ = ValueField::new(&mut threads, 0..=64)
            .formatter(|v| {
                if v == 0.0 {
                    "auto".into()
                } else {
                    format!("{v}")
                }
            })
            .show(ui);
    });
    full.textures_delta.clear();
    let texts: Vec<String> = full
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            eframe::egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t == "auto"), "{texts:?}");
}

#[test]
fn an_icon_window_keeps_the_plain_title_id() {
    let ctx = context(Density::Regular);
    let mut full = ctx.run_ui(input(Vec::new()), |ui| {
        icon_window(ui.ctx(), icons::GEAR, "Settings").show(ui.ctx(), |_| {});
        icon_window(ui.ctx(), Icon::glyph("G"), "Glyph").show(ui.ctx(), |_| {});
        eframe::egui::Window::new("Plain").show(ui.ctx(), |_| {});
    });
    full.textures_delta.clear();
    let known = |id| ctx.memory(|m| m.area_rect(id).is_some());
    assert!(known(window_id("Plain")), "window_id matches Window::new");
    for title in ["Settings", "Glyph"] {
        assert!(known(window_id(title)), "{title} keeps the plain id");
    }
}

#[test]
fn a_tab_title_leads_with_a_glyph_only() {
    let glyph = tab_title(Icon::glyph("G"), "Tab");
    let painted = tab_title(icons::GEAR, "Tab");
    let none = tab_title(None, "Tab");
    assert_eq!(glyph.text(), "GTab");
    assert_eq!(painted.text(), "Tab");
    assert_eq!(none.text(), "Tab");
}

#[test]
fn a_tree_node_toggles_from_its_caret() {
    let ctx = context(Density::Regular);
    let mut row = Rect::NOTHING;
    let mut shown = false;
    let draw = |ui: &mut Ui, row: &mut Rect, shown: &mut bool| {
        let (r, inner) = ListItem::new("C:").show_node(ui, "c", false, |_| ());
        *row = r.rect;
        *shown = inner.is_some();
    };
    run_with(&ctx, Vec::new(), |ui| draw(ui, &mut row, &mut shown));
    assert!(!shown);
    let caret = row.left_center() + vec2(12.0, 0.0);
    click_at(&ctx, caret, |ui| draw(ui, &mut row, &mut shown));
    run_with(&ctx, Vec::new(), |ui| draw(ui, &mut row, &mut shown));
    assert!(shown, "a caret click opens the node");
}

#[test]
fn one_tab_press_focuses_the_text_inside_a_field() {
    let ctx = context(Density::Regular);
    let mut text = String::new();
    let mut draw = |ui: &mut Ui| {
        let _ = TextField::new(&mut text).id_salt("tab.field").show(ui);
    };
    run_with(&ctx, Vec::new(), &mut draw);
    let tab = Event::Key {
        key: eframe::egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    run_with(&ctx, vec![tab], &mut draw);
    run_with(&ctx, Vec::new(), &mut draw);
    let focused = ctx.memory(|m| m.has_focus(eframe::egui::Id::unique("tab.field")));
    assert!(focused, "the first Tab lands on the text, not the box");
}

#[test]
fn disabled_menu_rows_and_dialog_buttons_ignore_clicks() {
    let ctx = context(Density::Regular);
    let mut rects = (Rect::NOTHING, Rect::NOTHING);
    let mut hits = (false, None);
    let draw = |ui: &mut Ui, rects: &mut (Rect, Rect), hits: &mut (bool, Option<usize>)| {
        let row = MenuItem::new("Archive").enabled(false).show(ui);
        rects.0 = row.rect;
        hits.0 |= row.clicked();
        let buttons = [(Role::Primary, "Apply")];
        let top = ui.cursor().top();
        hits.1 = hits.1.or(dialog_buttons_enabled(ui, &buttons, &[false]));
        rects.1 = Rect::from_min_max(
            pos2(ui.max_rect().right() - 40.0, top),
            pos2(ui.max_rect().right() - 20.0, top + 20.0),
        );
    };
    run_with(&ctx, Vec::new(), |ui| draw(ui, &mut rects, &mut hits));
    let (row, button) = rects;
    click_at(&ctx, row.center(), |ui| draw(ui, &mut rects, &mut hits));
    click_at(&ctx, button.center(), |ui| draw(ui, &mut rects, &mut hits));
    assert_eq!(hits, (false, None));
}

#[test]
fn a_section_reports_its_header_and_takes_an_exact_id() {
    let ctx = context(Density::Regular);
    let id = eframe::egui::Id::unique("exact.section");
    let mut header = Rect::NOTHING;
    let mut clicked = false;
    let draw = |ui: &mut Ui, header: &mut Rect, clicked: &mut bool| {
        let (response, _) = Section::new("unused", "Inventory")
            .id(id)
            .suffix("(3)", None)
            .trailing(|ui| {
                let _ = badge(ui, "1 running", Color32::YELLOW);
            })
            .default_open(false)
            .show_with_header(ui, |_| ());
        *header = response.rect;
        *clicked |= response.clicked();
    };
    run_with(&ctx, Vec::new(), |ui| draw(ui, &mut header, &mut clicked));
    assert_eq!(
        header.height(),
        Metrics::for_density(Density::Regular).header_h
    );
    click_at(&ctx, header.left_center() + vec2(30.0, 0.0), |ui| {
        draw(ui, &mut header, &mut clicked)
    });
    assert!(clicked);
    let stored = eframe::egui::collapsing_header::CollapsingState::load(&ctx, id);
    assert!(
        stored.is_some_and(|s| s.is_open()),
        "the click opened the exact id"
    );
}

#[test]
fn a_loading_row_sits_on_one_line() {
    let ctx = context(Density::Regular);
    let (row, line) = run(&ctx, 1, |ui| {
        (
            loading_row(ui, "Loading drivers").rect,
            ui.text_style_height(&eframe::egui::TextStyle::Body),
        )
    });
    assert!(
        (row.height() - line.max(SpinnerSize::Sm.side())).abs() < 0.5,
        "{row:?} {line}"
    );
    assert_eq!(row.width(), row.width().round());
}

#[test]
fn a_menu_without_a_set_width_fits_its_rows() {
    let ctx = context(Density::Regular);
    let trigger = std::cell::Cell::new(Rect::NOTHING);
    let mut rows = Vec::new();
    let mut draw = |ui: &mut Ui| {
        let (response, _) = MenuButton::new("View").show(ui, |ui| {
            rows.clear();
            rows.push(menu_item(ui, icons::GEAR, "Settings").rect.width());
            menu_separator(ui);
            rows.push(menu_check_item(ui, None, "Wrap lines", true).rect.width());
        });
        trigger.set(response.rect);
    };
    run_with(&ctx, Vec::new(), &mut draw);
    click_at(&ctx, trigger.get().center(), &mut draw);
    for _ in 0..3 {
        run_with(&ctx, Vec::new(), &mut draw);
    }
    assert_eq!(rows.len(), 2, "the menu opened");
    for width in rows {
        assert!(
            (width - layout::MENU_MIN_W).abs() < 1.0,
            "menu row is {width} wide"
        );
    }
}

#[test]
fn a_checkbox_row_flips_from_its_label_and_holds_the_control_height() {
    let ctx = context(Density::Comfortable);
    let mut on = false;
    let mut row = Rect::NOTHING;
    run_with(&ctx, Vec::new(), |ui| {
        row = checkbox(ui, &mut on, "Run on startup").rect
    });
    assert_eq!(
        row.height(),
        Metrics::for_density(Density::Comfortable).control_h
    );
    click_at(&ctx, row.right_center() - vec2(4.0, 0.0), |ui| {
        let _ = checkbox(ui, &mut on, "Run on startup");
    });
    assert!(on);
}

#[test]
fn an_icon_link_reports_the_width_it_allocates() {
    let ctx = context(Density::Regular);
    for (text, small) in [("Link", true), ("customer:abc123", false)] {
        let (reported, drawn) = run(&ctx, 2, |ui| {
            let link = IconLink::new(text).icon(icons::PLUS).small(small);
            (link.width(ui), link.show(ui).rect.width())
        });
        assert_eq!(reported, drawn, "{text}");
    }
}

/// Every shape the last of `passes` frames of `f` painted on the root Ui, flattened.
fn painted(ctx: &Context, passes: usize, mut f: impl FnMut(&mut Ui)) -> Vec<Shape> {
    fn flatten(shape: Shape, out: &mut Vec<Shape>) {
        match shape {
            Shape::Vec(shapes) => shapes.into_iter().for_each(|s| flatten(s, out)),
            other => out.push(other),
        }
    }
    let mut shapes = Vec::new();
    for _ in 0..passes {
        let mut full = ctx.run_ui(input(Vec::new()), |ui| f(ui));
        full.textures_delta.clear();
        shapes.clear();
        for clipped in full.shapes {
            flatten(clipped.shape, &mut shapes);
        }
    }
    shapes
}

/// Swatch colours no theme paints.
const PROBES: [Color32; 7] = [
    Color32::from_rgb(1, 2, 3),
    Color32::from_rgb(4, 5, 6),
    Color32::from_rgb(7, 8, 9),
    Color32::from_rgb(10, 11, 12),
    Color32::from_rgb(13, 14, 15),
    Color32::from_rgb(16, 17, 18),
    Color32::from_rgb(19, 20, 21),
];

#[test]
fn swatches_keep_the_menu_row_height_and_sit_before_the_check_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let mut rows = [Rect::NOTHING; 2];
        let shapes = painted(&ctx, 2, |ui| {
            menu_body(ui, |ui| {
                for (i, checked) in [true, false].into_iter().enumerate() {
                    rows[i] = MenuItem::new("Palette")
                        .swatches(&PROBES)
                        .checked(checked)
                        .show(ui)
                        .rect;
                }
            });
        });
        let chips: Vec<Rect> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) if PROBES.contains(&r.fill) => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(chips.len(), 2 * PROBES.len(), "{density:?}");
        for (row, strip) in rows.iter().zip(chips.chunks(PROBES.len())) {
            assert_eq!(row.height(), m.row_h, "{density:?}");
            let check_left = row.right() - m.pad.x - m.icon_sm;
            for chip in strip {
                assert!(
                    row.contains_rect(*chip),
                    "{density:?}: {chip:?} leaves {row:?}"
                );
                assert!(
                    (chip.center().y - row.center().y).abs() <= 0.5,
                    "{density:?}"
                );
                assert!(chip.right() + m.gap.x <= check_left + 0.01, "{density:?}");
            }
            for pair in strip.windows(2) {
                assert!(
                    pair[0].right() < pair[1].left(),
                    "{density:?}: chips overlap"
                );
            }
        }
        assert_eq!(
            chips[0].left(),
            chips[PROBES.len()].left(),
            "{density:?}: checked and unchecked strips misalign"
        );
    }
}

#[test]
fn swatch_outlines_stand_out_from_the_menu_and_hover_fills() {
    let ctx = context(Density::Regular);
    let dark = Tokens::get(&ctx);
    let light_style = Style {
        visuals: eframe::egui::Visuals::light(),
        ..Style::default()
    };
    let light = Tokens::derive(&light_style, dark.success, dark.secondary);
    let flush = Tokens {
        separator: dark.window,
        hover: dark.sunken,
        ..dark
    };
    let raised_hover = Tokens {
        hover: mix(dark.window, dark.text, 0.3),
        ..dark
    };
    let cases = [
        ("dark", dark),
        ("light", light),
        ("separator on the menu fill", flush),
        ("bright hover", raised_hover),
    ];
    for (name, t) in cases {
        Tokens::publish(&ctx, t);
        let shapes = painted(&ctx, 2, |ui| {
            menu_body(ui, |ui| {
                let _ = MenuItem::new("Palette").swatches(&PROBES).show(ui);
            });
        });
        let rings: Vec<Color32> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) if PROBES.contains(&r.fill) => Some(r.stroke.color),
                _ => None,
            })
            .collect();
        assert_eq!(rings.len(), PROBES.len(), "{name}");
        for ring in rings {
            for (fill_name, fill) in [("menu", t.window), ("hover", t.hover)] {
                let ratio = crate::tokens::contrast(ring, fill);
                assert!(ratio >= 1.4, "{name}: {ratio:.2} on the {fill_name} fill");
            }
        }
    }
}

#[test]
fn a_menu_of_swatched_rows_stays_narrow() {
    let ctx = context(Density::Regular);
    let trigger = std::cell::Cell::new(Rect::NOTHING);
    let mut widths = Vec::new();
    let mut draw = |ui: &mut Ui| {
        let (response, _) = MenuButton::new("Theme").show(ui, |ui| {
            widths.clear();
            widths.push(MenuItem::new("Custom").checked(false).show(ui).rect.width());
            let row = MenuItem::new("Midnight glass preset")
                .swatches(&PROBES)
                .checked(true)
                .show(ui);
            widths.push(row.rect.width());
        });
        trigger.set(response.rect);
    };
    run_with(&ctx, Vec::new(), &mut draw);
    click_at(&ctx, trigger.get().center(), &mut draw);
    for _ in 0..3 {
        run_with(&ctx, Vec::new(), &mut draw);
    }
    assert_eq!(widths.len(), 2, "the menu opened");
    assert_eq!(widths[0], widths[1], "rows share the menu width");
    assert!(
        widths[1] > layout::MENU_MIN_W,
        "the strip widened nothing: {}",
        widths[1]
    );
    assert!(widths[1] <= 300.0, "the menu is {} wide", widths[1]);
}

#[test]
fn a_marked_row_paints_a_primary_dot_in_its_icon_slot() {
    let ctx = context(Density::Regular);
    let t = Tokens::get(&ctx);
    let m = Metrics::for_density(Density::Regular);
    let mut item = Rect::NOTHING;
    let shapes = painted(&ctx, 2, |ui| {
        menu_body(ui, |ui| {
            let _ = MenuItem::new("Group").marked(true).submenu(ui, |_| ());
            item = MenuItem::new("Item")
                .icon(icons::GEAR)
                .marked(true)
                .show(ui)
                .rect;
            let _ = MenuItem::new("Plain").show(ui);
        });
    });
    let dots: Vec<Pos2> = shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Circle(c) if c.fill == t.primary => Some(c.center),
            _ => None,
        })
        .collect();
    assert_eq!(dots.len(), 2, "{dots:?}");
    let slot = pos2(item.left() + m.pad.x + m.icon_md / 2.0, item.center().y);
    assert!(
        (dots[1] - slot).length() < 0.01,
        "{:?} vs {slot:?}",
        dots[1]
    );
}

#[test]
fn a_theme_preview_paints_inside_its_rect_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let t = Tokens::get(&ctx);
        let mut rect = Rect::NOTHING;
        let name = "A theme whose name runs well past the title bar of the preview";
        let shapes = painted(&ctx, 2, |ui| rect = theme_preview(ui, &t, name).rect);
        assert_eq!(rect.width(), THEME_PREVIEW_W, "{density:?}");
        assert!(
            THEME_PREVIEW_W <= m.tooltip_w,
            "{density:?}: wider than a tooltip"
        );
        assert!(
            rect.height() > m.header_h + 4.0 * m.control_h,
            "{density:?}"
        );
        assert!(rect.height() < THEME_PREVIEW_W, "{density:?}");
        assert!(shapes.len() > 10, "{density:?}: {} shapes", shapes.len());
        for shape in &shapes {
            let bounds = shape.visual_bounding_rect();
            assert!(
                rect.expand(0.5).contains_rect(bounds),
                "{density:?}: {bounds:?} leaves {rect:?}"
            );
        }
    }
}

#[test]
fn a_theme_preview_paints_the_given_tokens_not_the_installed_ones() {
    let ctx = context(Density::Regular);
    let installed = Tokens::get(&ctx);
    let preset = Tokens {
        base: Color32::from_rgb(3, 60, 9),
        primary: Color32::from_rgb(250, 10, 200),
        field: Color32::from_rgb(70, 1, 70),
        ..installed
    };
    let shapes = painted(&ctx, 2, |ui| {
        let _ = theme_preview(ui, &preset, "Preset");
    });
    let fills: Vec<Color32> = shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) => Some(r.fill),
            _ => None,
        })
        .collect();
    for c in [preset.base, preset.primary, preset.field] {
        assert!(fills.contains(&c), "{c:?} missing from {fills:?}");
    }
    assert!(
        !fills.contains(&installed.base),
        "painted the installed base"
    );
}

#[test]
fn button_colours_change_with_state_and_follow_the_kind() {
    let ctx = context(Density::Regular);
    let t = Tokens::get(&ctx);
    for kind in ButtonKind::ALL {
        let [rest, hover, pressed] =
            [State::Rest, State::Hovered, State::Pressed].map(|s| button_colors(&t, kind, s));
        assert_ne!(rest.0, pressed.0, "{kind:?}: pressed looks like rest");
        match kind {
            ButtonKind::Neutral => {
                assert_eq!(rest.0, t.field);
                assert_eq!(hover.0, t.hover);
                assert_eq!(pressed.0, t.pressed);
                assert_eq!(rest.2, t.text);
            }
            ButtonKind::Primary => {
                assert_eq!(rest.0, t.primary);
                assert_ne!(hover.0, rest.0);
                assert_eq!(rest.2, t.on_primary);
            }
            ButtonKind::Danger => {
                assert_eq!(rest.2, t.error);
                assert_ne!(hover.0, rest.0);
                assert_ne!(rest.1, Color32::TRANSPARENT);
            }
            ButtonKind::Ghost => {
                assert_eq!(rest.0, Color32::TRANSPARENT);
                assert_eq!(hover.0, t.hover);
                assert_eq!(rest.2, t.text_dim);
            }
        }
    }
}

#[test]
fn a_painted_button_uses_the_colours_of_its_state() {
    let ctx = context(Density::Regular);
    let t = Tokens::get(&ctx);
    for kind in ButtonKind::ALL {
        let shapes = painted(&ctx, 2, |ui| {
            CentralPanel::default().show(ui, |ui| {
                let _ = TextButton::new("Go").kind(kind).show(ui);
            });
        });
        let (fill, _, _) = button_colors(&t, kind, State::Rest);
        let want = if fill == Color32::TRANSPARENT {
            None
        } else {
            Some(fill)
        };
        let found = shapes
            .iter()
            .any(|s| matches!(s, Shape::Rect(r) if Some(r.fill) == want));
        assert!(
            want.is_none() || found,
            "{kind:?}: rest fill {fill:?} not painted"
        );
    }
}

#[test]
fn a_glyph_icon_paints_text_and_a_painted_icon_paints_shapes() {
    let ctx = context(Density::Regular);
    let shapes = painted(&ctx, 1, |ui| {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), Vec2::splat(16.0));
        Icon::glyph("G").paint(ui, rect, 16.0, Color32::WHITE);
        icons::CHECK.paint(ui, rect, 16.0, Color32::WHITE);
    });
    let has_text = shapes.iter().any(|s| matches!(s, Shape::Text(_)));
    let has_line = shapes
        .iter()
        .any(|s| matches!(s, Shape::LineSegment { .. } | Shape::Path(_)));
    assert!(has_text, "glyph paints no text");
    assert!(has_line, "painted icon paints no line");
}

#[test]
fn icons_stay_inside_their_slot_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        for (name, icon) in icons::ALL {
            let slot = Rect::from_min_size(pos2(20.0, 20.0), Vec2::splat(m.icon_md));
            let shapes = painted(&ctx, 1, |ui| {
                icon.paint(ui, slot, m.icon_md, Color32::WHITE)
            });
            for shape in &shapes {
                let bounds = shape.visual_bounding_rect();
                assert!(
                    slot.expand(1.5).contains_rect(bounds),
                    "{name} at {density:?}: {bounds:?} leaves {slot:?}"
                );
            }
        }
    }
}

#[test]
fn glyph_strings_convert_into_icons_through_the_builders() {
    let ctx = context(Density::Regular);
    run(&ctx, 2, |ui| {
        let _ = icon_button(ui, "G", IconBox::Md, false, "Glyph");
        let _ = icon_text_button(ui, "G", "Glyph");
        let _ = TextButton::new("Glyph").icon("G").show(ui);
        let _ = IconButton::new("G", IconBox::Lg).selected(true).show(ui);
        let _ = IconLabel::new("G", "Glyph").show(ui);
        let _ = status_label(ui, "G", "Glyph", Color32::GREEN);
        let _ = TextField::new(&mut String::new()).icon("G").show(ui);
        let _ = MenuButton::new("Glyph").icon("G").show(ui, |_| ());
        let _ = ListItem::new("Glyph")
            .icon(Icon::glyph("G"))
            .action("G", "tip")
            .show(ui);
        let _ = Segment::new(0, "Glyph").icon("G");
        let _ = empty_state(ui, "G", "Empty", "", &[]);
    });
}

#[test]
fn sides_put_the_right_content_at_the_right_edge() {
    let ctx = context(Density::Regular);
    let (left, right, panel) = run(&ctx, 2, |ui| {
        let width = ui.available_width();
        let origin = ui.cursor().left();
        let (l, r) = sides(
            ui,
            |ui| button(ui, "Left").rect,
            |ui| button(ui, "Right").rect,
        );
        (l, r, origin..=origin + width)
    });
    assert!((left.left() - panel.start()).abs() < 1.0, "{left:?}");
    assert!(
        (right.right() - panel.end()).abs() < 1.0,
        "{right:?} in {panel:?}"
    );
    assert!(left.right() < right.left());
}

#[test]
fn form_rows_align_labels_and_controls_on_the_control_height_at_every_density() {
    for density in Density::ALL {
        let ctx = context(density);
        let m = Metrics::for_density(density);
        let rows = run(&ctx, 3, |ui| {
            let mut text = String::new();
            let mut size = 1.0_f32;
            form(ui, "form", |ui| {
                let a = form_row(ui, "Name", |ui| text_field(ui, &mut text, "", 140.0).rect);
                let b = form_row(ui, "A longer label", |ui| {
                    value_field(ui, &mut size, 0.0..=9.0, "px", 80.0).rect
                });
                [a, b]
            })
        });
        for rect in rows {
            assert_eq!(rect.height(), m.control_h, "{density:?}");
        }
        assert_eq!(
            rows[0].left(),
            rows[1].left(),
            "{density:?}: columns misalign"
        );
        assert!(rows[1].top() > rows[0].top(), "{density:?}");
    }
}

#[test]
fn striped_rows_paint_a_band_only_at_rest() {
    let ctx = context(Density::Regular);
    let t = Tokens::get(&ctx);
    let band = mix(t.base, t.text, 0.045);
    let fills = |striped: bool| {
        painted(&ctx, 2, |ui| {
            let _ = ListItem::new("Row").striped(striped).show(ui);
        })
        .iter()
        .filter(|s| matches!(s, Shape::Rect(r) if r.fill == band))
        .count()
    };
    assert_eq!(fills(false), 0);
    assert_eq!(fills(true), 1);
}

#[test]
fn usage_meters_take_the_colour_of_their_level() {
    let ctx = context(Density::Regular);
    let colours = |fraction: f32| {
        let shapes = painted(&ctx, 2, |ui| {
            let _ = usage_meter(ui, fraction, 100.0);
        });
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) => Some(r.fill),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let (good, warn, critical) = (colours(0.3), colours(0.75), colours(0.95));
    let level = |pct: f32| {
        let mut out = Color32::TRANSPARENT;
        run(&ctx, 1, |ui| out = crate::colors::usage_level(ui, pct));
        out
    };
    assert!(good.contains(&level(30.0)));
    assert!(warn.contains(&level(75.0)));
    assert!(critical.contains(&level(95.0)));
    assert_ne!(level(30.0), level(95.0));
}
