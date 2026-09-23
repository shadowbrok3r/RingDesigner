//! Piercings, azures and cathedral shoulders added from the Ring viewport's right-click.
use crate::app::RingDesignerApp;
use crate::interaction_tests::{click_at, harness, wait_for_build};
use egui::{Modifiers, PointerButton, Pos2, Rect};
use egui_kittest::{Harness, kittest::Queryable};
use ringdesign_core::cad::{Attach, Document, Feature, Operation, Placement, Stage, builders};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::RingDesign;
use ringdesign_workbench::viewport::Sel;

/// One Ring viewport showing `design`, built and settled in the history, looked at from `yaw`, `pitch`.
fn ring_of(h: &mut Harness<'static, RingDesignerApp>, design: RingDesign, yaw: f32, pitch: f32) -> usize {
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
        (cam.yaw, cam.pitch, cam.roll) = (yaw, pitch, 0.0);
        cam.fit(bounds);
    }
    h.run_steps(3);
    pane
}

fn court() -> RingDesign {
    ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
}

/// The Court band with a 6.5 mm round #2 on its top, in four claws (#3 the head, #4 its seat) when `set`.
fn solitaire(set: bool) -> RingDesign {
    let gem = Gem::calibrated(GemCut::Round, 6.5);
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Default::default() }).unwrap();
    doc.append(builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm("claw4", gem)))).unwrap();
    if set {
        let mut next = 2;
        for f in builders::setting_features("claw4", 2, gem, true, &mut || {
            next += 1;
            next
        })
        .unwrap()
        {
            doc.append(f).unwrap();
        }
    }
    RingDesign { cad: Some(doc), ..court() }
}

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f32; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world)
}

fn doc(h: &Harness<'static, RingDesignerApp>) -> Document {
    h.state().design.cad.clone().expect("a document")
}

fn names(h: &Harness<'static, RingDesignerApp>) -> Vec<String> {
    doc(h).features.iter().map(|f| f.name.clone()).collect()
}

/// Right-clicks at `at` and chooses `item`, from `submenu` when there is one.
fn menu(h: &mut Harness<'static, RingDesignerApp>, at: Pos2, submenu: Option<&str>, item: &str) {
    click_at(h, at, PointerButton::Secondary, Modifiers::NONE);
    if let Some(sub) = submenu {
        // A submenu's button carries egui's own arrow.
        h.get_by_label(&format!("{sub} ⏵")).click();
        h.run_steps(3);
    }
    h.get_by_label(item).click();
    h.run_steps(3);
}

fn rebuilt(h: &mut Harness<'static, RingDesignerApp>) -> std::sync::Arc<ringdesign_core::mesh::BuildResult> {
    h.state_mut().rebuild_now();
    wait_for_build(h);
    let b = h.state().build.clone().unwrap();
    assert!(b.report.validation.watertight, "{:?}", b.report.validation);
    assert!(b.parts.notes.is_empty(), "{:?}", b.parts.notes);
    b
}

#[test]
fn a_right_click_on_the_crown_cuts_a_heart_through_it_in_one_undo_step_left_to_the_bench() {
    let mut h = harness();
    let pane = ring_of(&mut h, court(), std::f32::consts::FRAC_PI_2, 0.0);
    let start = h.state().history.present();
    let top = screen(&h, pane, [0.0, (court().inner_radius_mm() + court().profile.thickness_mm) as f32, 0.0]);
    menu(&mut h, top, Some("Cut here"), "Heart");
    assert_eq!(names(&h), ["Procedural shank", "Heart piercing"], "{}", h.state().status);
    let cut = doc(&h).features[1].clone();
    assert!(matches!(&cut.operation, Operation::Builder { key, on: None, params } if key == builders::PIERCE && params["shape"] == "Heart"));
    assert_eq!((cut.component.attach, cut.component.stage), (Attach::Cut, Stage::Bench), "a hole across the pull is drilled after a sand pour");
    assert!(matches!(cut.component.placement, Placement::Ring { theta_deg, cant_deg, .. } if (theta_deg - 90.0).abs() < 0.5 && cant_deg == 0.0), "{:?}", cut.component.placement);
    assert_eq!(h.state().history.present(), start + 1, "one gesture, one undo step");
    assert_eq!(h.state().selection.items.last(), Some(&Sel::Part(cut.id)), "the new cut is chosen");
    let b = rebuilt(&mut h);
    assert_eq!((b.parts.cut, b.parts.joined), (1, 0));
    let through = b.report.volume_mm3;
    // The cut chosen, Edit feature: the inspector's Through box unticked, previewed with Enter and applied with Ctrl+Enter.
    let rect = viewport_rect(&h);
    menu(&mut h, rect.center() - egui::vec2(0.0, rect.height() * 0.3), None, "Edit feature");
    h.run_steps(5);
    {
        use egui_kittest::kittest::NodeT;
        h.query_all_by_label("Through").find(|n| n.accesskit_node().role() == egui::accesskit::Role::CheckBox).expect("the Through box").click();
    }
    h.run_steps(3);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    crate::interaction_tests::wait_until_previewed(&mut h);
    crate::interaction_tests::press_ctrl_enter(&mut h);
    let params = match &doc(&h).feature(cut.id).unwrap().operation {
        Operation::Builder { params, .. } => params.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(params["through"], false, "{params}");
    assert_eq!(h.state().history.present(), start + 2);
    h.state_mut().switch_desktop(crate::dock::Desktop::Model);
    let b = rebuilt(&mut h);
    assert!(b.report.volume_mm3 > through, "a blind pocket takes less metal: {} against {through}", b.report.volume_mm3);
    h.state_mut().undo();
    h.state_mut().undo();
    assert!(h.state().design.cad.is_none(), "the undo before takes the cut and the shank it brought");
}

#[test]
fn a_right_click_on_a_side_face_cuts_along_the_finger_and_casts() {
    let mut h = harness();
    let mut flat = RingDesign::default();
    flat.profile.apply_style(ringdesign_core::profile::ProfileStyle::Flat);
    (flat.profile.width_mm, flat.profile.thickness_mm) = (5.0, 2.5);
    // Looking down the finger onto the high side face.
    let pane = ring_of(&mut h, flat.clone(), std::f32::consts::FRAC_PI_2, 1.5);
    let r = flat.inner_radius_mm() + 0.5 * flat.profile.thickness_mm;
    let face = screen(&h, pane, [0.0, r as f32, 2.5]);
    menu(&mut h, face, Some("Cut here"), "Round");
    assert_eq!(names(&h), ["Procedural shank", "Round piercing"], "{}", h.state().status);
    let cut = doc(&h).features[1].clone();
    assert_eq!(cut.component.stage, Stage::Cast, "walls along the pull cast");
    assert!(matches!(cut.component.placement, Placement::Ring { cant_deg, .. } if cant_deg == -90.0), "{:?}", cut.component.placement);
    let b = rebuilt(&mut h);
    let part = b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == cut.id).unwrap().clone();
    assert!(part.frame.z_axis[2] > 1.0 - 1e-9, "the cut runs along the finger: {:?}", part.frame.z_axis);
}

#[test]
fn a_stone_takes_azures_and_cathedral_shoulders_from_its_right_click() {
    let mut h = harness();
    let d = solitaire(true);
    let pane = ring_of(&mut h, d.clone(), std::f32::consts::FRAC_PI_2, 0.0);
    let stone = d.cad.as_ref().unwrap().feature(2).unwrap().component.placement.frame_on(&d, h.state().build.as_ref().and_then(|b| b.band.as_deref())).unwrap();
    let table: [f32; 3] = std::array::from_fn(|k| (stone.origin[k] + stone.z_axis[k] * 1.2) as f32);
    let start = h.state().history.present();
    let at = screen(&h, pane, table);
    menu(&mut h, at, Some("Azures"), "6 windows");
    assert_eq!(names(&h), ["Procedural shank", "Round 6.5 mm", "Four-claw head", "Seat bur", "6 azures"], "{}", h.state().status);
    let azures = doc(&h).features[4].clone();
    assert!(matches!(&azures.operation, Operation::Builder { key, on: Some(2), params } if key == builders::AZURE && params[builders::HEAD] == 3));
    assert_eq!((azures.component.attach, azures.component.stage), (Attach::Cut, Stage::Bench));
    assert_eq!(h.state().history.present(), start + 1);
    let b = rebuilt(&mut h);
    assert_eq!((b.parts.joined, b.parts.cut), (1, 2), "the head joined, the seat and the windows cut");
    // Then the shoulders, from the same stone.
    let table = screen(&h, pane, table);
    menu(&mut h, table, None, "Cathedral shoulders");
    assert_eq!(names(&h).last().map(String::as_str), Some("Cathedral shoulders"), "{}", h.state().status);
    let arches = doc(&h).features[5].clone();
    assert_eq!((arches.component.attach, arches.component.stage), (Attach::Join, Stage::Bench), "under sand the shoulders go to the bench with their head");
    assert_eq!(h.state().history.present(), start + 2);
    let b = rebuilt(&mut h);
    assert_eq!((b.parts.joined, b.parts.cut), (2, 2));
}

#[test]
fn shoulders_asked_of_a_stone_with_no_head_say_so_and_change_nothing() {
    let mut h = harness();
    let d = solitaire(false);
    let pane = ring_of(&mut h, d.clone(), std::f32::consts::FRAC_PI_2, 0.0);
    let stone = d.cad.as_ref().unwrap().feature(2).unwrap().component.placement.frame_on(&d, h.state().build.as_ref().and_then(|b| b.band.as_deref())).unwrap();
    let table: [f32; 3] = std::array::from_fn(|k| (stone.origin[k] + stone.z_axis[k] * 1.2) as f32);
    let start = h.state().history.present();
    let at = screen(&h, pane, table);
    menu(&mut h, at, None, "Cathedral shoulders");
    assert_eq!(h.state().status, "#2 Round 6.5 mm carries no head for cathedral shoulders to meet: set it in claws, a basket or a bezel first");
    assert_eq!(h.state().history.present(), start, "nothing committed");
    assert_eq!(names(&h), ["Procedural shank", "Round 6.5 mm"]);
}
