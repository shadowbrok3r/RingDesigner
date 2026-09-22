//! Stones and made settings added from the Ring viewport's right-click.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use ringdesign_core::cad::{Attach, Document, Operation, Placement, Stage, builders};
use ringdesign_workbench::viewport::Sel;

/// One Ring viewport showing `design`, built, settled in the history, and seen straight down onto the ring's top.
fn ring_of(h: &mut Harness<'static, RingDesignerApp>, design: ringdesign_core::RingDesign) -> usize {
    let pane = {
        let app = h.state_mut();
        app.switch_desktop(crate::dock::Desktop::Model);
        app.set_layout(crate::pane::Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = crate::pane::PaneKind::Solid;
        app.active_pane = pane;
        app.design = design;
        app.history.commit(&app.design);
        app.rebuild_now();
        pane
    };
    wait_for_build(h);
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.yaw = std::f32::consts::FRAC_PI_2;
        cam.pitch = 0.0;
        cam.roll = 0.0;
        cam.fit(bounds);
    }
    h.run_steps(3);
    pane
}

fn court() -> ringdesign_core::RingDesign {
    ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
}

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f32; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world)
}

/// A point of the viewport with nothing under it, over the ring's top.
fn beside(h: &Harness<'static, RingDesignerApp>) -> Pos2 {
    let rect = viewport_rect(h);
    rect.center() - vec2(0.0, rect.height() * 0.3)
}

fn doc(h: &Harness<'static, RingDesignerApp>) -> Document {
    h.state().design.cad.clone().expect("a document")
}

fn names(h: &Harness<'static, RingDesignerApp>) -> Vec<String> {
    doc(h).features.iter().map(|f| f.name.clone()).collect()
}

/// Right-clicks at `at` and chooses `item` from `submenu`.
fn menu(h: &mut Harness<'static, RingDesignerApp>, at: Pos2, submenu: &str, item: &str) {
    click_at(h, at, PointerButton::Secondary, Modifiers::NONE);
    // A submenu's button carries egui's own arrow.
    h.get_by_label(&format!("{submenu} ⏵")).click();
    h.run_steps(3);
    h.get_by_label(item).click();
    h.run_steps(3);
}

#[test]
fn a_solitaire_in_three_gestures_on_the_court_band_is_one_undo_step_each() {
    let mut h = harness();
    let pane = ring_of(&mut h, court());
    let start = h.state().history.present();
    // 1. Right-click the crest: Add stone here, Round 6.5 mm. The plain band brings its shank with the first part.
    let top = screen(&h, pane, [0.0, (court().inner_radius_mm() + court().profile.thickness_mm) as f32, 0.0]);
    menu(&mut h, top, "Add stone here", "Round 6.5 mm");
    assert_eq!(names(&h), ["Procedural shank", "Round 6.5 mm"]);
    let stone = doc(&h).features[1].clone();
    assert!(matches!(&stone.operation, Operation::Builder { key, on: None, .. } if key == builders::STONE));
    assert!(stone.component.reference);
    assert!(matches!(stone.component.placement, Placement::Ring { theta_deg, .. } if (theta_deg - 90.0).abs() < 0.5), "{:?}", stone.component.placement);
    assert_eq!(h.state().history.present(), start + 1, "one gesture, one undo step");
    assert_eq!(h.state().selection.items.last(), Some(&Sel::Part(stone.id)), "the new stone is chosen");
    // 2. With the stone chosen, right-click beside the ring: Setting, Four claws. The head and its seat are one commit.
    let at = beside(&h);
    menu(&mut h, at, "Setting", "Four claws");
    assert_eq!(names(&h), ["Procedural shank", "Round 6.5 mm", "Four-claw head", "Seat bur"]);
    let (head, bur) = { let d = doc(&h); (d.features[2].clone(), d.features[3].clone()) };
    assert!(matches!(&head.operation, Operation::Builder { key, on: Some(s), .. } if key == builders::CLAW && *s == stone.id));
    assert!(matches!(&bur.operation, Operation::Builder { key, on: Some(s), .. } if key == builders::BUR && *s == stone.id));
    assert_eq!((head.component.attach, bur.component.attach, bur.component.stage), (Attach::Join, Attach::Cut, Stage::Bench), "a seat is cut after a sand pour");
    assert_eq!(h.state().history.present(), start + 2);
    // 3. The head chosen, Edit feature: six claws in the CAD pane's inspector, previewed with Enter, applied with Ctrl+Enter.
    let at = beside(&h);
    menu_item(&mut h, at, "Edit feature");
    h.run_steps(5);
    h.get_by_label("Claws 6").click();
    h.run_steps(3);
    h.key_press(Key::Enter);
    h.run_steps(3);
    wait_until_previewed(&mut h);
    press_ctrl_enter(&mut h);
    let prongs = |h: &Harness<'static, RingDesignerApp>| match &doc(h).feature(head.id).unwrap().operation {
        Operation::Builder { params, .. } => params["prongs"].as_u64(),
        _ => None,
    };
    assert_eq!(prongs(&h), Some(6));
    assert_eq!(h.state().history.present(), start + 3);
    // The ring as built: the head joined, the seat cut, the stone never metal.
    h.state_mut().switch_desktop(crate::dock::Desktop::Model);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    {
        let b = h.state().build.clone().unwrap();
        assert_eq!((b.parts.joined, b.parts.cut, b.parts.references), (1, 1, 1), "{:?}", b.parts.notes);
        assert!(b.report.validation.watertight, "{:?}", b.report.validation);
    }
    // Undo takes each gesture back in turn, to the plain band.
    h.state_mut().undo();
    assert_eq!(prongs(&h), Some(4));
    h.state_mut().undo();
    assert_eq!(names(&h), ["Procedural shank", "Round 6.5 mm"]);
    h.state_mut().undo();
    assert!(h.state().design.cad.is_none(), "the first undo step took the shank with the stone");
}

/// Right-clicks at `at` and chooses `item` from the menu itself.
fn menu_item(h: &mut Harness<'static, RingDesignerApp>, at: Pos2, item: &str) {
    click_at(h, at, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label(item).click();
    h.run_steps(3);
}

#[test]
fn a_height_field_stone_takes_a_setting_in_one_commit_as_a_part_at_its_own_girdle() {
    use ringdesign_core::field::{Layer, LayerEntry, SeatPadLayer, SeatStyle};
    use ringdesign_core::gem::{Gem, GemCut};
    let mut h = harness();
    let mut d = court();
    let v = d.field_context().crest_v_mm;
    let mut pad = SeatPadLayer { theta_deg: 90.0, v_mm: v, style: SeatStyle::GypsyMound, ..Default::default() };
    pad.fit_stone(Gem::calibrated(GemCut::Round, 2.5));
    d.layers.layers.push(LayerEntry::new("Seat", Layer::SeatPad(pad)));
    let pane = ring_of(&mut h, d);
    let start = h.state().history.present();
    // Right-click the drawn stone's table: the menu is about the stone, and offers every setting.
    let (seat, frame) = ringdesign_core::stones::stone_frames(&h.state().design).remove(0);
    let table: [f32; 3] = std::array::from_fn(|k| (frame.girdle[k] + frame.normal[k] * 0.5) as f32);
    let at = screen(&h, pane, table);
    menu(&mut h, at, "Setting", "Bezel");
    assert_eq!(names(&h), ["Procedural shank", "Round 2.5 mm", "Bezel", "Seat bur"]);
    assert_eq!(h.state().history.present(), start + 1, "the stone and its setting are one undo step");
    let stone = doc(&h).features[1].clone();
    assert!(stone.component.reference && matches!(&stone.operation, Operation::Builder { key, .. } if key == builders::STONE));
    let Placement::Ring { theta_deg, spin_deg, .. } = stone.component.placement else { panic!("{:?}", stone.component.placement) };
    assert_eq!((theta_deg, spin_deg), (seat.theta_deg, 0.0));
    // The part stone stands at the drawn stone's girdle on the ring as built, and the setting resolves round it.
    let girdle = stone.component.placement.frame_on(&h.state().design, h.state().build.as_ref().map(|b| &b.mesh)).unwrap().origin;
    let off = (0..3).map(|k| (girdle[k] - frame.girdle[k]).powi(2)).sum::<f64>().sqrt();
    assert!(off < 0.05, "{off:.4} mm from the drawn stone's girdle");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let b = h.state().build.clone().unwrap();
    assert_eq!((b.parts.joined, b.parts.cut, b.parts.references), (1, 1, 1), "{:?}", b.parts.notes);
    assert!(b.report.validation.watertight, "{:?}", b.report.validation);
    h.state_mut().undo();
    assert!(h.state().design.cad.is_none(), "one undo takes the stone, the shank and the setting back");
}
