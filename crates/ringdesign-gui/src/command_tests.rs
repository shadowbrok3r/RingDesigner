//! The command session in the Ring viewport: hotkeys, the dimension bar, the tool rail and box select.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage};
use ringdesign_workbench::viewport::Sel;

/// The cylinder's id in the Band + joined Cylinder design.
const POST: u64 = 2;

/// A single Ring viewport, active, showing the design.
fn ring_view(h: &mut Harness<'static, RingDesignerApp>) -> usize {
    let app = h.state_mut();
    app.switch_desktop(crate::dock::Desktop::Model);
    app.set_layout(crate::pane::Layout::Single);
    let pane = app.visible_panes()[0];
    app.panes[pane].kind = crate::pane::PaneKind::Solid;
    app.active_pane = pane;
    pane
}

/// The procedural shank and a cylinder joined at the top of the ring, built and settled in the history.
fn band_and_cylinder(h: &mut Harness<'static, RingDesignerApp>) -> usize {
    let pane = ring_view(h);
    {
        let app = h.state_mut();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: POST,
            name: "Cylinder".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
            component: Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::ring(90.0, 0.25), ..Default::default() },
        })
        .unwrap();
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(h);
    look_down_at_the_top(h, pane);
    pane
}

/// Looks straight down −y at the top of the ring and returns the viewport's rect.
fn look_down_at_the_top(h: &mut Harness<'static, RingDesignerApp>, pane: usize) -> egui::Rect {
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
    viewport_rect(h)
}

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> egui::Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// A world point on screen through the pane's camera.
fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f32; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world)
}

/// The centre of the cylinder's top face on screen.
fn post_top(h: &Harness<'static, RingDesignerApp>, pane: usize) -> Pos2 {
    let app = h.state();
    let c = app.build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap();
    let (lo, hi) = c.mesh.bounds().unwrap();
    screen(h, pane, [(lo.0 + hi.0) * 0.5, hi.1, (lo.2 + hi.2) * 0.5])
}

/// The band's crest at `theta` on screen.
fn crest(h: &Harness<'static, RingDesignerApp>, pane: usize, theta: f32) -> Pos2 {
    let d = &h.state().design;
    let r = (d.inner_radius_mm() + d.profile.thickness_mm) as f32;
    let (s, c) = theta.to_radians().sin_cos();
    screen(h, pane, [r * c, r * s, 0.0])
}

fn post(h: &Harness<'static, RingDesignerApp>) -> Feature {
    h.state().design.cad.as_ref().and_then(|d| d.feature(POST)).cloned().expect("the cylinder")
}

fn live(h: &Harness<'static, RingDesignerApp>) -> Option<&'static str> {
    h.state().command.session.command().map(|c| c.key())
}

fn document(h: &Harness<'static, RingDesignerApp>) -> serde_json::Value {
    serde_json::to_value(&h.state().design.cad).unwrap()
}

/// Clicks the cylinder's top, which chooses one of its faces.
fn select_post(h: &mut Harness<'static, RingDesignerApp>, pane: usize) -> Pos2 {
    let top = post_top(h, pane);
    click_at(h, top, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(h.state().selection.items.iter().filter_map(Sel::feature).last(), Some(POST), "{:?}", h.state().selection.items);
    top
}

fn press(h: &mut Harness<'static, RingDesignerApp>, key: Key) {
    h.key_press(key);
    h.run_steps(2);
}

fn text(h: &mut Harness<'static, RingDesignerApp>, t: &str) {
    h.event(Event::Text(t.into()));
    h.run_steps(2);
}

/// A primary drag from `from` to `to` in steps, the button held throughout.
fn drag(h: &mut Harness<'static, RingDesignerApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=8 {
        h.event(Event::PointerMoved(from + (to - from) * (k as f32 / 8.0)));
        h.run_steps(1);
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

#[test]
fn g_moves_the_chosen_part_twelve_typed_degrees_round_the_ring_as_one_undo_step() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let top = select_post(&mut h, pane);
    let entries = h.state().history.present();
    let before = post(&h).component.placement.theta_deg().unwrap();
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::G);
    assert_eq!(live(&h), Some("move"));
    assert!(viewport_label(&h).contains("Move live"), "{}", viewport_label(&h));
    h.hover_at(top + egui::vec2(25.0, 0.0));
    h.run_steps(3);
    text(&mut h, "12");
    assert!(h.state().command.bar.has_focus(&h.ctx), "a digit starts the first field");
    assert_eq!(h.get_by_label("Δθ (°)").value().as_deref(), Some("12"));
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    let theta = post(&h).component.placement.theta_deg().unwrap();
    assert_eq!(theta - before, 12.0, "{before} -> {theta}");
    assert_eq!(h.state().history.present(), entries + 1, "one command is one undo step");
    assert_eq!(h.state().status, "Place Cylinder");
    // The rebuild a moved part asks for keeps the band it was read on: the band did not change.
    let band = h.state().command.band_surface().expect("the move read the band");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::G);
    h.hover_at(top + egui::vec2(5.0, 0.0));
    h.run_steps(2);
    assert_eq!(h.state().command.band_surface(), Some(band), "no second sweep for a part edit");
    press(&mut h, Key::Escape);
    h.state_mut().undo();
    assert_eq!(post(&h).component.placement.theta_deg(), Some(before));
}

fn press_undo(h: &mut Harness<'static, RingDesignerApp>) {
    h.event(Event::ModifiersChanged(Modifiers::COMMAND));
    h.event(Event::Key { key: Key::Z, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    h.run_steps(1);
    h.event(Event::Key { key: Key::Z, pressed: false, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

#[test]
fn undo_during_a_move_takes_back_the_move_and_the_next_undo_the_last_edit() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let top = select_post(&mut h, pane);
    let before = post(&h).component.placement.theta_deg().unwrap();
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::G);
    h.hover_at(top + egui::vec2(25.0, 0.0));
    h.run_steps(3);
    text(&mut h, "12");
    press(&mut h, Key::Enter);
    let (entries, doc) = (h.state().history.present(), document(&h));
    assert_eq!(post(&h).component.placement.theta_deg(), Some(before + 12.0));
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::G);
    h.hover_at(top + egui::vec2(40.0, 0.0));
    h.run_steps(3);
    assert_eq!(live(&h), Some("move"));
    press_undo(&mut h);
    assert_eq!(live(&h), None, "Undo ends the live command");
    assert_eq!((document(&h), h.state().history.present()), (doc, entries), "and takes back nothing committed");
    assert_eq!(h.state().status, "Cancelled; nothing changed");
    press_undo(&mut h);
    assert_eq!(post(&h).component.placement.theta_deg(), Some(before), "the next Undo takes back the typed move");
}

#[test]
fn escape_backs_out_of_a_typed_move_and_only_then_reaches_the_selection() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let top = select_post(&mut h, pane);
    let (entries, doc) = (h.state().history.present(), document(&h));
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::G);
    h.hover_at(top + egui::vec2(25.0, 0.0));
    h.run_steps(3);
    text(&mut h, "12");
    press(&mut h, Key::Escape);
    assert_eq!(live(&h), Some("move"), "the first Escape empties the field");
    assert_eq!(h.get_by_label("Δθ (°)").value().as_deref(), Some(""));
    press(&mut h, Key::Escape);
    assert_eq!(live(&h), None, "the second leaves the command");
    assert_eq!((document(&h), h.state().history.present()), (doc.clone(), entries), "nothing committed");
    assert!(!h.state().selection.items.is_empty(), "the selection outlives the command");
    press(&mut h, Key::Escape);
    assert!(h.state().selection.items.is_empty(), "with no command live Escape clears the selection");
    // A command with nothing typed backs straight out, and the right button cancels as Blender's does.
    select_post(&mut h, pane);
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::R);
    assert_eq!(live(&h), Some("rotate"));
    press(&mut h, Key::Escape);
    assert_eq!(live(&h), None);
    press(&mut h, Key::P);
    assert_eq!(live(&h), Some("place"));
    click_at(&mut h, top, PointerButton::Secondary, Modifiers::NONE);
    assert_eq!(live(&h), None);
    assert!(!egui::Popup::is_any_open(&h.ctx), "and opens no menu");
    assert_eq!(document(&h), doc);
}

#[test]
fn s_with_a_typed_radius_of_two_resizes_the_cylinder() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let top = select_post(&mut h, pane);
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::S);
    assert_eq!(live(&h), Some("scale"));
    // Tab from the viewport reaches the bar's first field, the factor; the next reaches the radius.
    press(&mut h, Key::Tab);
    assert!(h.get_by_label("Factor (×)").is_focused());
    press(&mut h, Key::Tab);
    assert!(h.get_by_label("Radius (mm)").is_focused());
    text(&mut h, "2");
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    assert!(matches!(post(&h).operation, Operation::Cylinder { radius_mm, height_mm } if radius_mm == 2.0 && height_mm == 2.5), "{:?}", post(&h).operation);
}

#[test]
fn shift_a_adds_a_typed_cylinder_where_the_click_lands_and_tab_stays_in_the_bar() {
    let mut h = harness();
    let pane = ring_view(&mut h);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    look_down_at_the_top(&mut h, pane);
    assert!(h.state().design.cad.is_none(), "a plain ring");
    let entries = h.state().history.present();
    let at = crest(&h, pane, 70.0);
    h.hover_at(at);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::A);
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(3);
    assert_eq!(live(&h), Some("add-cylinder"));
    click_at(&mut h, at, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(h.state().command.session.command().map(|c| c.step()), Some(1), "the click set the centre");
    text(&mut h, "1.5");
    press(&mut h, Key::Tab);
    assert!(h.get_by_label("Height (mm)").is_focused(), "Tab moves to the next field");
    assert!(h.state().command.bar.has_focus(&h.ctx), "and never to a button outside the bar");
    press(&mut h, Key::Tab);
    assert!(h.get_by_label("Radius (mm)").is_focused(), "past the last field it wraps");
    press(&mut h, Key::Tab);
    text(&mut h, "3");
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    let doc = h.state().design.cad.clone().expect("the part and its shank");
    let names: Vec<_> = doc.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Procedural shank", "Cylinder"], "the first part brings the shank with it");
    let cyl = doc.features.iter().find(|f| f.name == "Cylinder").unwrap();
    assert!(matches!(cyl.operation, Operation::Cylinder { radius_mm, height_mm } if radius_mm == 1.5 && height_mm == 3.0), "{:?}", cyl.operation);
    assert!(matches!(cyl.component.placement, Placement::Ring { theta_deg, across_mm, height_mm, .. } if theta_deg == 70.0 && across_mm == 0.0 && height_mm == 0.0), "{:?}", cyl.component.placement);
    assert_eq!(cyl.component.attach, Attach::Join);
    assert_eq!(h.state().selection.items, [Sel::Part(cyl.id)], "the new part is the selection");
    assert_eq!(h.state().history.present(), entries + 1);
}

#[test]
fn b_then_a_window_round_the_cylinder_selects_its_part_without_orbiting() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let (c, top) = (h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap().mesh.bounds().unwrap(), post_top(&h, pane));
    let (lo, hi) = (screen(&h, pane, [c.0.0, c.1.1, c.0.2]), screen(&h, pane, [c.1.0, c.1.1, c.1.2]));
    let r = egui::Rect::from_two_pos(lo, hi).expand(12.0);
    let yaw = h.state().panes[pane].camera.yaw;
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::B);
    assert!(h.state().command.box_armed && viewport_label(&h).contains("box select armed"));
    drag(&mut h, r.left_top(), r.right_bottom());
    assert!(!h.state().command.box_armed, "one box, then B is spent");
    assert_eq!(h.state().selection.items, [Sel::Part(POST)], "a window round the part takes the part");
    assert_eq!(h.state().panes[pane].camera.yaw, yaw, "the box drag does not orbit");
    // A crossing right to left over its rim takes it away with Ctrl.
    press(&mut h, Key::B);
    h.event(Event::ModifiersChanged(Modifiers::COMMAND));
    let rim = egui::Rect::from_center_size(egui::pos2(r.max.x - 14.0, top.y), egui::vec2(24.0, 10.0));
    drag(&mut h, rim.right_top(), rim.left_bottom());
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(1);
    assert!(h.state().selection.items.is_empty(), "{:?}", h.state().selection.items);
    // Escape puts an armed box away and the next drag orbits again.
    press(&mut h, Key::B);
    press(&mut h, Key::Escape);
    assert!(!h.state().command.box_armed);
    drag(&mut h, r.left_top(), r.right_bottom());
    assert_ne!(h.state().panes[pane].camera.yaw, yaw);
}

#[test]
fn j_turns_the_joined_cylinder_into_a_cut_and_the_build_says_so() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    assert_eq!(h.state().build.as_ref().map(|b| (b.parts.joined, b.parts.cut)), Some((1, 0)));
    let top = select_post(&mut h, pane);
    let entries = h.state().history.present();
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::J);
    assert_eq!(live(&h), None, "one press, one commit");
    assert_eq!(post(&h).component.attach, Attach::Cut);
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!(h.state().status, "Cut Cylinder");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    assert_eq!(h.state().build.as_ref().map(|b| (b.parts.joined, b.parts.cut)), Some((0, 1)), "the report counts a cut");
    press(&mut h, Key::J);
    assert_eq!(post(&h).component.attach, Attach::Separate, "Join, Cut, Separate, round again");
}

#[test]
fn a_key_with_nothing_chosen_says_so_and_starts_nothing() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let at = crest(&h, pane, 60.0);
    h.hover_at(at);
    h.run_steps(2);
    for key in [Key::G, Key::R, Key::S, Key::P, Key::J, Key::A, Key::Q] {
        press(&mut h, key);
        assert_eq!(live(&h), None);
        assert_eq!(h.state().status, "Select a part first: click one on the ring");
    }
    assert_eq!(post(&h).component.attach, Attach::Join);
}

#[test]
fn q_pulls_the_chosen_flat_face_and_a_arrays_the_chosen_part_as_the_menu_does() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let top = select_post(&mut h, pane);
    assert!(matches!(h.state().selection.items.last(), Some(Sel::Face { feature: POST, .. })), "{:?}", h.state().selection.items);
    let entries = h.state().history.present();
    h.hover_at(top);
    h.run_steps(2);
    // Q on the chosen top: the press-pull the face's menu starts, sizing the cylinder.
    press(&mut h, Key::Q);
    assert_eq!(live(&h), Some("press-pull"));
    text(&mut h, "0.5");
    assert_eq!(h.state().command.session.preview().unwrap().caption, "Press-pull out 0.50 mm · sizes Cylinder");
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().history.present(), entries + 1, "one press-pull, one undo step");
    let sized = post(&h);
    assert!(matches!(sized.operation, Operation::Cylinder { height_mm, .. } if (height_mm - 3.0).abs() < 1e-12), "{:?}", sized.operation);
    assert!(matches!(sized.component.placement, Placement::Ring { height_mm, .. } if (height_mm - 0.5).abs() < 1e-12), "{:?}", sized.component.placement);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    // A on the same part: the array round the ring, six by default, its ghost five copies of the part.
    let top = select_post(&mut h, pane);
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::A);
    assert_eq!(live(&h), Some("array"));
    assert_eq!(h.state().command.session.preview().unwrap().caption, "Array round the ring: 6 in all, a copy every 60.0°");
    let per = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap().mesh.vertices.len();
    assert_eq!(crate::patterns::STAGED.with(|s| s.borrow().vertices.len()), 5 * per);
    while live(&h).is_some() {
        press(&mut h, Key::Escape);
    }
    assert_eq!(h.state().history.present(), entries + 1, "a cancelled array adds nothing");
    // The cylinder's curved side chosen: Q says why and starts nothing.
    let side = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap().trace.face_kind.iter().position(|k| *k != ringdesign_core::cad::SurfaceKind::Plane).unwrap() as u32;
    h.state_mut().selection.click(Some(Sel::Face { feature: POST, face: side }), ringdesign_workbench::viewport::Mods::default());
    h.hover_at(top);
    h.run_steps(2);
    press(&mut h, Key::Q);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().status, format!("Press-pull moves flat faces; face {side} of #2 Cylinder is curved"));
    // With the part chosen rather than a face, Q asks for a face.
    h.state_mut().selection.click(Some(Sel::Part(POST)), ringdesign_workbench::viewport::Mods::default());
    press(&mut h, Key::Q);
    assert_eq!((live(&h), h.state().status.as_str()), (None, "Choose a flat face of the part: click one on the ring"));
}

#[test]
fn every_command_has_an_icon_a_rail_slot_a_palette_entry_and_a_key() {
    use crate::panels::Command;
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let catalog = ringdesign_workbench::command::commands::rail();
    let slot = |c: &ringdesign_workbench::command::CommandInfo| crate::command::rail_label(&c.title, c.key);
    let rail = |h: &Harness<'static, RingDesignerApp>, label: &str| {
        h.query_all_by_label(label).find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button).map(|n| !n.accesskit_node().is_disabled())
    };
    for c in &catalog {
        assert!(!c.icon.svg().is_empty(), "{} has a mark", c.key);
        let key = crate::command::keys(c.key).unwrap_or_else(|| panic!("{} has no key", c.key));
        if let Some(hot) = c.hotkey {
            assert_eq!(key, hot.to_string(), "{}", c.key);
        } else {
            assert!(key.starts_with("Shift+A, "), "{}: {key}", c.key);
        }
        let entry = Command::ALL.iter().find(|p| p.tool() == Some(c.key)).unwrap_or_else(|| panic!("{} has no palette entry", c.key));
        assert!(entry.label().ends_with(&format!("({})", key.split(',').next().unwrap())), "{}", entry.label());
        let enabled = rail(&h, &slot(c)).unwrap_or_else(|| panic!("{} has no rail slot", c.title));
        assert_eq!(enabled, c.key.starts_with("add-"), "{}: with nothing chosen only the adds are enabled", c.key);
    }
    assert_eq!(Command::ALL.iter().filter(|p| p.tool().is_some()).count(), catalog.len());
    // Every entry runs the same start: from the palette, from the rail, and from its key.
    let top = select_post(&mut h, pane);
    for c in catalog.iter().filter(|c| c.key != "attach") {
        Command::ALL.iter().find(|p| p.tool() == Some(c.key)).unwrap().run(h.state_mut());
        assert_eq!(live(&h), Some(c.key), "palette {}", c.key);
        h.state_mut().command.session.feed(ringdesign_workbench::command::StepInput::Cancel);
        h.run_steps(2);
        assert_eq!(rail(&h, &slot(c)), Some(true), "{} is enabled on a chosen part", c.title);
        h.query_all_by_label(&slot(c)).find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button).unwrap().click();
        h.run_steps(2);
        assert_eq!(live(&h), Some(c.key), "rail {}", c.key);
        h.hover_at(top);
        h.run_steps(1);
        while live(&h).is_some() {
            press(&mut h, Key::Escape);
        }
        select_post(&mut h, pane);
    }
    Command::ToolAttach.run(h.state_mut());
    assert_eq!(post(&h).component.attach, Attach::Cut, "the palette's attach steps it on");
}

#[test]
fn a_pick_at_the_top_of_the_ring_reads_ninety_degrees_on_the_crest_with_no_stand_off() {
    use ringdesign_core::interaction::pick::{Filter, PickScene, Ray, ViewScale};
    let design = ringdesign_core::RingDesign::default();
    let lib = ringdesign_core::AlphaLibrary::builtin();
    let built = ringdesign_core::mesh::build(&design, &lib, ringdesign_core::BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() });
    let scene = PickScene::build(&built, &design);
    let band = ringdesign_workbench::command::BandSurface::new(built.mesh.clone());
    let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 20.0 };
    let pick = scene.pick(Ray { origin: [0.0, 40.0, 0.0], direction: [0.0, -1.0, 0.0] }, &view, 0.0, Filter::default()).into_iter().next().expect("the band");
    let nominal = design.inner_radius_mm() + design.profile.thickness_mm;
    let p = ringdesign_workbench::command::ring_point(pick.world, Some(&band), nominal);
    assert!((p.theta_deg - 90.0).abs() < 1e-3 && p.across_mm.abs() < 1e-3 && p.height_mm.abs() < 1e-3, "{p:?}");
    // Round the ring and along the finger the same pick reads its own angle and offset, still on the surface.
    for (theta, across) in [(0.0f64, 0.0f64), (135.0, 0.6), (270.0, -0.8)] {
        let (s, c) = theta.to_radians().sin_cos();
        let pick = scene.pick(Ray { origin: [40.0 * c, 40.0 * s, across], direction: [-c, -s, 0.0] }, &view, 0.0, Filter::default()).into_iter().next().unwrap();
        let p = ringdesign_workbench::command::ring_point(pick.world, Some(&band), nominal);
        let dtheta = (p.theta_deg - theta + 540.0).rem_euclid(360.0) - 180.0;
        assert!(dtheta.abs() < 1e-3 && (p.across_mm - across).abs() < 1e-3 && p.height_mm.abs() < 1e-3, "{theta} {across}: {p:?}");
    }
    // A ring of parts only reads its stand-off from the reference crest.
    let p = ringdesign_workbench::command::ring_point([0.0, nominal + 1.25, 0.5], None, nominal);
    assert!((p.theta_deg - 90.0).abs() < 1e-9 && (p.across_mm - 0.5).abs() < 1e-12 && (p.height_mm - 1.25).abs() < 1e-9, "{p:?}");
}

/// The stone's id in the plate design.
const STONE: u64 = 3;

/// A 4 x 6 x 1.5 mm plate joined on the top of the ring, a 3 mm stone on the plate's top and the stone's four-claw head, built and settled.
fn stone_on_plate(h: &mut Harness<'static, RingDesignerApp>) -> usize {
    use ringdesign_core::cad::{FaceSeat, builders, face_signature, stone_on_face};
    let pane = ring_view(h);
    {
        let app = h.state_mut();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let plate = Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::ring(90.0, 0.65), ..Default::default() };
        doc.append(Feature { id: 2, name: "Plate".into(), enabled: true, operation: Operation::Box { size: [4.0, 6.0, 1.5] }, component: plate }).unwrap();
        let mut d = app.design.clone();
        d.cad = Some(doc.clone());
        let e = ringdesign_core::cad::evaluate(&d, &ringdesign_core::AlphaLibrary::builtin(), ringdesign_core::BuildParams::default()).unwrap();
        let host = e.components.iter().find(|c| c.id == 2).unwrap();
        let top = (0..host.body.faces.len()).find(|i| face_signature(&host.body, *i, &host.frame).is_some_and(|s| s.normal[2] > 0.99)).unwrap() as u32;
        let gem = ringdesign_core::gem::Gem::calibrated(ringdesign_core::gem::GemCut::Round, 3.0);
        let seat = FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
        doc.append(stone_on_face(STONE, gem, 2, &seat)).unwrap();
        doc.append(builders::feature_on(4, "Four-claw head", builders::CLAW, STONE, serde_json::json!({ "prongs": 4 }))).unwrap();
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(h);
    look_down_at_the_top(h, pane);
    pane
}

/// Part `id`'s frame as the last build seated it: its origin and its axes.
fn frame_of(h: &Harness<'static, RingDesignerApp>, id: u64) -> [[f64; 3]; 4] {
    let c = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("#{id} built")).frame;
    [c.origin, c.x_axis, c.y_axis, c.z_axis]
}

fn seat(h: &Harness<'static, RingDesignerApp>) -> ringdesign_core::cad::FaceSeat {
    let Some(Operation::Builder { params, .. }) = h.state().design.cad.as_ref().and_then(|d| d.feature(STONE)).map(|f| f.operation.clone()) else { panic!("the stone") };
    ringdesign_core::cad::FaceSeat::of(&params).unwrap().expect("a seat on the plate")
}

#[test]
fn g_r_and_the_gizmo_on_a_stone_on_a_plate_edit_its_seat_and_the_head_on_it_follows() {
    let mut h = harness();
    let pane = stone_on_plate(&mut h);
    let (stone, head) = (frame_of(&h, STONE), frame_of(&h, 4));
    assert_eq!(stone, head, "the head stands in its stone's frame");
    let (before, features) = (seat(&h), h.state().design.cad.as_ref().unwrap().features.len());
    h.state_mut().selection.click(Some(Sel::Part(STONE)), ringdesign_workbench::viewport::Mods::default());
    h.run_steps(2);
    // The stone's gizmo is the face's own: along and across it, spun on it; looking down onto the plate its normal points at the eye.
    for (label, shown) in [("Gizmo: slide along the face", true), ("Gizmo: slide across the face", true), ("Gizmo: lift off the face", false), ("Gizmo: spin on the face", true), ("Gizmo: move along X", false)] {
        assert_eq!(h.query_by_label(label).is_some(), shown, "{label}");
    }
    let entries = h.state().history.present();
    // G with 0.4 typed slides the seat 0.4 mm along the face: an edit of the stone, never a Transform wrapped round it.
    let over = viewport_rect(&h).center();
    h.hover_at(over);
    h.run_steps(2);
    press(&mut h, Key::G);
    assert_eq!(live(&h), Some("move"));
    assert!(h.state().command.session.preview().unwrap().caption.starts_with("Move Δalong"), "{}", h.state().command.session.preview().unwrap().caption);
    text(&mut h, "0.4");
    assert_eq!(h.get_by_label("Δalong (mm)").value().as_deref(), Some("0.4"));
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    let moved = seat(&h);
    assert_eq!((moved.u_mm - before.u_mm, moved.v_mm, moved.height_mm, moved.spin_deg), (0.4, before.v_mm, before.height_mm, before.spin_deg));
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), features, "no Transform wraps the stone");
    assert_eq!(h.state().history.present(), entries + 1, "one command is one undo step");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    // Built again the stone stands 0.4 mm along the face, its bearing kept, and its head with it.
    let (stone2, head2) = (frame_of(&h, STONE), frame_of(&h, 4));
    let along: [f64; 3] = std::array::from_fn(|k| stone[1][k]);
    let expected: [f64; 3] = std::array::from_fn(|k| stone[0][k] + 0.4 * along[k]);
    assert!((0..3).all(|k| (stone2[0][k] - expected[k]).abs() < 1e-6) && stone2[1..] == stone[1..], "{stone2:?} against {expected:?}");
    assert_eq!(head2, stone2, "the head follows its stone");
    // R with 30 typed spins it on the face.
    h.state_mut().selection.click(Some(Sel::Part(STONE)), ringdesign_workbench::viewport::Mods::default());
    h.hover_at(over);
    h.run_steps(2);
    press(&mut h, Key::R);
    assert_eq!(live(&h), Some("rotate"));
    text(&mut h, "30");
    press(&mut h, Key::Enter);
    assert_eq!((seat(&h).spin_deg, seat(&h).u_mm), (30.0, moved.u_mm));
    // Placing it on the ring is refused by name: it stands where the plate's face is.
    press(&mut h, Key::P);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().status, "#3 Round 3 mm stands on a part's face: G slides it on the face, R spins it");
    // The along arrow dragged 30 px slides it along the face alone, as one undo step.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    h.state_mut().selection.click(Some(Sel::Part(STONE)), ringdesign_workbench::viewport::Mods::default());
    h.run_steps(2);
    let (spun, entries) = (seat(&h), h.state().history.present());
    let arrow = h.get_by_label("Gizmo: slide along the face").rect().center();
    let o = stone2[0].map(|v| v as f32);
    let way = (screen(&h, pane, [o[0] + along[0] as f32, o[1] + along[1] as f32, o[2] + along[2] as f32]) - screen(&h, pane, o)).normalized();
    drag(&mut h, arrow, arrow + way * 30.0);
    let dragged = seat(&h);
    assert!((dragged.u_mm - spun.u_mm).abs() > 0.2 && dragged.v_mm == spun.v_mm && dragged.spin_deg == spun.spin_deg, "{spun:?} -> {dragged:?}");
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), features);
    h.state_mut().undo();
    assert_eq!(seat(&h), spun);
}

/// The claw solitaire on the Court band, built and settled: the stone #2, its four-claw head #3 and its seat bur #4.
fn claw_solitaire(h: &mut Harness<'static, RingDesignerApp>) -> usize {
    let pane = ring_view(h);
    {
        let app = h.state_mut();
        let court = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        app.design = ringdesign_core::RingDesign { cad: ringdesign_core::cad::examples::design("claw-solitaire").unwrap().cad, ..court };
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(h);
    look_down_at_the_top(h, pane);
    pane
}

/// Feature `id` of the document.
fn feature_of(h: &Harness<'static, RingDesignerApp>, id: u64) -> Feature {
    h.state().design.cad.as_ref().and_then(|d| d.feature(id)).cloned().unwrap_or_else(|| panic!("#{id}"))
}

/// Chooses part `id` alone.
fn choose(h: &mut Harness<'static, RingDesignerApp>, id: u64) {
    h.state_mut().selection.click(Some(Sel::Part(id)), ringdesign_workbench::viewport::Mods::default());
    h.run_steps(2);
}

#[test]
fn g_r_p_and_the_gizmo_on_a_claw_head_move_its_stone_and_the_head_follows_as_one_undo_step_each() {
    let mut h = harness();
    let pane = claw_solitaire(&mut h);
    let (stone, head) = (frame_of(&h, 2), frame_of(&h, 3));
    assert_eq!(stone, head, "the head is built in its stone's frame");
    let features = h.state().design.cad.as_ref().unwrap().features.len();
    let head_at_first = serde_json::to_value(feature_of(&h, 3)).unwrap();
    choose(&mut h, 3);
    // The head wears its stone's gizmo: round the ring, across the band and its spin; never the world's axes.
    for (label, shown) in [("Gizmo: move round the ring", true), ("Gizmo: move across the band", true), ("Gizmo: spin", true), ("Gizmo: move along X", false)] {
        assert_eq!(h.query_by_label(label).is_some(), shown, "{label}");
    }
    // G with 12 typed turns the stone 12° round the ring: an edit of the stone's placement, never a Transform round the head.
    let over = viewport_rect(&h).center();
    h.hover_at(over);
    h.run_steps(2);
    let entries = h.state().history.present();
    press(&mut h, Key::G);
    assert_eq!(live(&h), Some("move"));
    assert!(h.state().command.session.preview().unwrap().caption.starts_with("Move Δθ"), "{}", h.state().command.session.preview().unwrap().caption);
    // The ghost carried is the head's own mesh.
    let head_mesh = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap().mesh.clone();
    let staged = h.state().renderer.lock().unwrap().staged_preview().0.map(<[f32]>::len);
    assert_eq!(staged, Some(crate::viewport::GpuMeshRenderer::stage_part(&head_mesh).len()), "the ghost is the head");
    text(&mut h, "12");
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    assert_eq!(feature_of(&h, 2).component.placement.theta_deg(), Some(102.0));
    assert_eq!(serde_json::to_value(feature_of(&h, 3)).unwrap(), head_at_first, "the head itself is not edited");
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), features, "no Transform wraps it");
    assert_eq!(h.state().history.present(), entries + 1, "one command is one undo step");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let (stone2, head2) = (frame_of(&h, 2), frame_of(&h, 3));
    let round = |o: [f64; 3]| o[1].atan2(o[0]).to_degrees();
    eprintln!("G 12 on the head: its stone and the head stand at {:.6}° round the ring, from {:.6}°", round(head2[0]), round(head[0]));
    assert_eq!(head2, stone2, "the head follows its stone");
    assert!((round(head2[0]) - 102.0).abs() < 1e-3 && (round(head[0]) - 90.0).abs() < 1e-3);
    // R with 30 typed spins the stone on its seat, and the head with it.
    choose(&mut h, 3);
    h.hover_at(over);
    h.run_steps(2);
    press(&mut h, Key::R);
    assert_eq!(live(&h), Some("rotate"));
    text(&mut h, "30");
    press(&mut h, Key::Enter);
    let spun = feature_of(&h, 2).component.placement;
    assert!(matches!(spun, Placement::Ring { theta_deg: 102.0, spin_deg: 30.0, .. }), "{spun:?}");
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), features);
    // S says the head is sized by its stone.
    press(&mut h, Key::S);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().status, "#3 Four-claw head is sized by its stone: size #2 Round 6.5 mm and it follows");
    // P seats the stone under the pointer; Escape leaves it where it was.
    press(&mut h, Key::P);
    assert_eq!(live(&h), Some("place"));
    press(&mut h, Key::Escape);
    assert_eq!(feature_of(&h, 2).component.placement, spun);
    // The round-the-ring arrow on the head, dragged 30 px, moves the stone round the ring as one undo step.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    choose(&mut h, 3);
    let entries = h.state().history.present();
    let arrow = h.get_by_label("Gizmo: move round the ring").rect().center();
    let o = frame_of(&h, 2)[0].map(|v| v as f32);
    let way = (screen(&h, pane, [o[0] - 0.2 * o[1], o[1] + 0.2 * o[0], o[2]]) - screen(&h, pane, o)).normalized();
    drag(&mut h, arrow, arrow + way * 30.0);
    let dragged = feature_of(&h, 2).component.placement;
    let (Placement::Ring { theta_deg: t0, .. }, Placement::Ring { theta_deg: t1, spin_deg, .. }) = (&spun, &dragged) else { panic!("{dragged:?}") };
    assert!((t1 - t0).abs() > 1.0 && *spin_deg == 30.0, "{spun:?} -> {dragged:?}");
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), features);
    assert!(h.state().command.dragging().is_none());
    h.state_mut().undo();
    assert_eq!(feature_of(&h, 2).component.placement, spun);
}

#[test]
fn g_and_the_face_gizmo_on_a_head_round_a_stone_on_a_plate_slide_the_stone_on_the_plate() {
    let mut h = harness();
    let _pane = stone_on_plate(&mut h);
    let stone = frame_of(&h, STONE);
    let (before, features) = (seat(&h), h.state().design.cad.as_ref().unwrap().features.len());
    choose(&mut h, 4);
    // The head wears the face's gizmo of the stone it is built round.
    for (label, shown) in [("Gizmo: slide along the face", true), ("Gizmo: slide across the face", true), ("Gizmo: spin on the face", true), ("Gizmo: move along X", false)] {
        assert_eq!(h.query_by_label(label).is_some(), shown, "{label}");
    }
    let over = viewport_rect(&h).center();
    h.hover_at(over);
    h.run_steps(2);
    let entries = h.state().history.present();
    press(&mut h, Key::G);
    assert_eq!(live(&h), Some("move"));
    assert!(h.state().command.session.preview().unwrap().caption.starts_with("Move Δalong"), "{}", h.state().command.session.preview().unwrap().caption);
    text(&mut h, "0.4");
    press(&mut h, Key::Enter);
    let moved = seat(&h);
    assert_eq!((moved.u_mm - before.u_mm, moved.v_mm, moved.height_mm, moved.spin_deg), (0.4, before.v_mm, before.height_mm, before.spin_deg));
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), features, "no Transform wraps the head");
    assert_eq!(h.state().history.present(), entries + 1);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let (stone2, head2) = (frame_of(&h, STONE), frame_of(&h, 4));
    let expected: [f64; 3] = std::array::from_fn(|k| stone[0][k] + 0.4 * stone[1][k]);
    eprintln!("G 0.4 on the head round a stone on a plate: the stone stands {:.6} mm from where it was", (0..3).map(|k| (stone2[0][k] - stone[0][k]).powi(2)).sum::<f64>().sqrt());
    assert!((0..3).all(|k| (stone2[0][k] - expected[k]).abs() < 1e-6) && head2 == stone2, "{stone2:?} {head2:?} against {expected:?}");
    // P and S on the head say why not, naming the stone.
    choose(&mut h, 4);
    press(&mut h, Key::P);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().status, "#4 Four-claw head follows #3 Round 3 mm, which stands on a part's face: G slides it on the face, R spins it");
    press(&mut h, Key::S);
    assert_eq!(h.state().status, "#4 Four-claw head is sized by its stone: size #3 Round 3 mm and it follows");
}

#[test]
fn a_claw_heads_ghost_carried_off_the_parting_line_is_the_head_read_where_its_stone_takes_it() {
    use ringdesign_core::FaceClass;
    use ringdesign_core::cad::builders;
    const HEAD: u64 = 4;
    let mut h = harness();
    let pane = ring_view(&mut h);
    {
        let app = h.state_mut();
        let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        d.profile.width_mm = 8.0;
        d.profile.thickness_mm = 2.5;
        let gem = builders::stone_preset("round-5").unwrap().gem();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(builders::stone_feature(3, gem, Placement::ring(60.0, builders::stand_off_mm("claw4", gem)))).unwrap();
        doc.append(builders::feature_on(HEAD, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }))).unwrap();
        d.cad = Some(doc);
        app.design = d;
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(&mut h);
    look_down_at_the_top(&mut h, pane);
    choose(&mut h, HEAD);
    // Looking down −y at the top, the finger's axis runs up the screen: the head's across arrow carries its stone along it.
    let press_at = h.get_by_label("Gizmo: move across the band").rect().center();
    let px = screen(&h, pane, [0.0, 0.0, 0.0]).distance(screen(&h, pane, [0.0, 0.0, 1.0]));
    let to = press_at + egui::vec2(0.0, -2.0 * px);
    h.event(Event::PointerMoved(press_at));
    h.event(Event::PointerButton { pos: press_at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    assert_eq!(live(&h), Some("move"));
    let rest = h.state().command.ghost_read().cloned().expect("the taken head is read where it stands");
    for k in 1..=6 {
        h.event(Event::PointerMoved(press_at + (to - press_at) * (k as f32 / 6.0)));
        h.run_steps(1);
    }
    let read = h.state().command.ghost_read().cloned().expect("the carried head is read");
    let preview = h.state().command.session.preview().unwrap();
    let Some(Placement::Ring { theta_deg, across_mm, .. }) = preview.placement else { panic!("the head moves by its stone's seat: {preview:?}") };
    assert!(preview.operation.is_none() && theta_deg == 60.0 && (across_mm - 2.0).abs() < 0.05, "{theta_deg} {across_mm}");
    let build = h.state().build.clone().unwrap();
    let head = build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == HEAD).unwrap().mesh.clone();
    let under = |r: &ringdesign_core::castability::ghost::GhostRead| r.classes.iter().filter(|c| **c == FaceClass::Undercut).count();
    eprintln!("the head carried 2 mm off the parting line by its stone: {} of {} faces lock, {:.2} mm² at {:.0}°, from {} faces and {:.2} mm² where it stood", under(&read), head.faces.len(), read.undercut_mm2, read.worst_deg, under(&rest), rest.undercut_mm2);
    // What is read is the head's own mesh carried by its stone's move, judged at the verdict's parting plane.
    let band = ringdesign_workbench::command::BandSurface::shared(build.band.clone().unwrap());
    let model = ringdesign_workbench::command::placed_ghost(&h.state().design, Some(&band), &feature_of(&h, 3), &preview).expect("the stone's move as a map");
    let parting = h.state().field.as_ref().map_or(0.0, |f| f.parting_z_mm);
    let direct = ringdesign_core::castability::ghost::GhostJudge::shared(&h.state().design, build.band.clone(), parting).read(&head, &model.0, false);
    assert_eq!(read, direct, "the ghost is the head, carried where its stone goes");
    assert!(read.locks() && under(&read) != under(&rest) && (read.undercut_mm2 - rest.undercut_mm2).abs() > 1.0, "{read:?}");
    let said = h.state().command.ghost_caption().unwrap();
    assert!(said.starts_with(&format!("Ghost would lock {:.1} mm² at {:.0}°", read.undercut_mm2, read.worst_deg)), "{said}");
    assert!(crate::interaction_tests::viewport_label(&h).contains(&said));
    // The ghost is staged in the draft colours, its locking faces red.
    {
        let renderer = h.state().renderer.clone();
        let r = renderer.lock().unwrap();
        let (Some(verts), true) = r.staged_preview() else { panic!("the ghost shades by class") };
        let red = FaceClass::Undercut.rgb();
        assert_eq!(verts.chunks(12).filter(|v| v[6..9] == red).count(), 3 * under(&read));
    }
    // Carried back to where it stood it reads as it did, and Escape leaves the stone where it was.
    for k in (0..=6).rev() {
        h.event(Event::PointerMoved(press_at + (to - press_at) * (k as f32 / 6.0)));
        h.run_steps(1);
    }
    let back = h.state().command.ghost_read().cloned().unwrap();
    assert!((back.undercut_mm2 - rest.undercut_mm2).abs() < 1e-6, "{} against {}", back.undercut_mm2, rest.undercut_mm2);
    press(&mut h, Key::Escape);
    assert!(h.state().command.ghost_caption().is_none(), "no command, no caption");
}

/// The Court band with three posts joined on its top, one of them moved to `last_theta`.
fn three_posts(last_theta: f64) -> ringdesign_core::RingDesign {
    let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    for (i, theta) in [60.0, 90.0, last_theta].into_iter().enumerate() {
        doc.append(Feature {
            id: 2 + i as u64,
            name: format!("Post {i}"),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.2, height_mm: 2.0 },
            component: Component { attach: Attach::Join, placement: Placement::ring(theta, 0.0), ..Default::default() },
        })
        .unwrap();
    }
    d.cad = Some(doc);
    d
}

/// Warm against cold rebuilds through the worker's cache after moving one part of three; run `--ignored --nocapture`.
#[test]
#[ignore]
fn warm_and_cold_rebuilds_after_moving_one_part_of_three() {
    use ringdesign_core::{AlphaLibrary, BuildParams, cad, mesh};
    use std::sync::Mutex;
    use std::time::Instant;
    let lib = AlphaLibrary::builtin();
    let never = std::sync::atomic::AtomicBool::new(false);
    for (name, params) in [("preview 384x144", BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }), ("export 1024x320", BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })] {
        let (before, after) = (three_posts(120.0), three_posts(130.0));
        let time = |d: &ringdesign_core::RingDesign, cache: &Mutex<cad::Cache>| {
            let t = Instant::now();
            let b = mesh::try_build_memo(d, &lib, params, &never, cad::Memo::new(cache)).unwrap();
            (t.elapsed().as_secs_f64() * 1e3, b.parts.joined)
        };
        let mut rows = Vec::new();
        for _ in 0..3 {
            let warm_cache = Mutex::new(cad::Cache::default());
            let (first, _) = time(&before, &warm_cache);
            let (warm, joined) = time(&after, &warm_cache);
            let (cold, _) = time(&after, &Mutex::new(cad::Cache::default()));
            let c = warm_cache.lock().unwrap();
            rows.push((first, warm, cold, joined, c.hits(), c.misses()));
        }
        let best = |k: fn(&(f64, f64, f64, usize, u64, u64)) -> f64| rows.iter().map(k).fold(f64::MAX, f64::min);
        let (_, _, _, joined, hits, misses) = rows[0];
        eprintln!("{name}: first {:.1} ms, warm after moving one of three {:.1} ms, cold {:.1} ms ({joined} joined; cache {hits} hits {misses} misses)", best(|r| r.0), best(|r| r.1), best(|r| r.2));
    }
}

/// The pointer path's and the ghost's cost on preview and export builds; run `--ignored --nocapture`.
#[test]
#[ignore]
fn the_pointer_path_and_the_ghost_on_an_export_build() {
    use ringdesign_core::interaction::pick::{Filter, PickScene, Ray, ViewScale};
    use ringdesign_core::{AlphaLibrary, BuildParams, mesh};
    use ringdesign_workbench::command::{BandSurface, MoveCmd, Probe, Reading, Session, SnapGeometry, Snapper, placed_ghost};
    use std::time::Instant;
    let lib = AlphaLibrary::builtin();
    let design = three_posts(120.0);
    for (name, params) in [("preview 384x144", BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }), ("export 1024x320", BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })] {
        let built = mesh::build(&design, &lib, params);
        let t = Instant::now();
        let mut bare = ringdesign_core::setting::without_solids(&design);
        bare.cad = None;
        let band_mesh = mesh::try_build(&bare, &lib, params).unwrap().mesh;
        let sweep_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let band = BandSurface::new(band_mesh);
        let tree_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_vec(&bare).unwrap_or_default().hash(&mut h);
            h.finish()
        };
        let key_us = t.elapsed().as_secs_f64() * 1e6;
        assert_ne!(key, 0);
        let scene = PickScene::build(&built, &design);
        let e = built.parts.evaluated.as_ref().unwrap();
        let carried = e.components.iter().find(|c| c.id == 3).unwrap();
        let (vertices, edges): (Vec<_>, Vec<_>) = (
            e.components.iter().filter(|c| c.id != 3).flat_map(|c| c.trace.vertices.clone()).collect(),
            e.components.iter().filter(|c| c.id != 3).flat_map(|c| c.edges.clone()).collect(),
        );
        let target = design.cad.as_ref().unwrap().feature(3).unwrap().clone();
        let mut session = Session::default();
        session.start(Box::new(MoveCmd::of(&target, 9)));
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 20.0 };
        let snapper = Snapper { grid: Some(crate::command::GRID), crest: true, ..Snapper::default() };
        let (mut sample_total, mut sample_worst, mut ghost_total) = (0.0f64, 0.0f64, 0.0f64);
        let n = 1000;
        for i in 0..n {
            // Over the top of the ring, part and band alike.
            let (x, z) = (((i * 37) % 200) as f64 * 0.05 - 5.0, ((i * 91) % 80) as f64 * 0.05 - 2.0);
            let ray = Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] };
            let t = Instant::now();
            let picks = scene.pick(ray, &view, 0.0, Filter { vertices: false, edges: false, ..Filter::default() });
            let probe = Probe { design: &design, surface: Some(&band), snapper: Some(snapper), geometry: SnapGeometry { vertices: &vertices, edges: &edges }, view, aperture_px: 8.0, carried: Some(3) };
            let token = probe.token(Reading::Surface, &picks, ray);
            let us = t.elapsed().as_secs_f64() * 1e6;
            sample_total += us;
            sample_worst = sample_worst.max(us);
            if let Some(token) = token {
                session.feed(token);
            }
            let preview = session.preview().unwrap();
            let t = Instant::now();
            let m = placed_ghost(&design, Some(&band), &target, &preview);
            ghost_total += t.elapsed().as_secs_f64() * 1e6;
            assert!(m.is_some());
        }
        let t = Instant::now();
        let staged = crate::viewport::GpuMeshRenderer::stage_part(&carried.mesh);
        let stage_ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "{name} ({} faces): band sweep {sweep_ms:.1} ms + tree {tree_ms:.1} ms once per band, its key {key_us:.0} µs once per build; pointer sample (pick + ring frame + snap) mean {:.1} µs, worst {sample_worst:.1} µs; ghost matrix mean {:.1} µs; ghost staged once {stage_ms:.2} ms, {} KiB for {} triangles",
            built.mesh.faces.len(),
            sample_total / n as f64,
            ghost_total / n as f64,
            staged.len() * 4 / 1024,
            carried.mesh.faces.len()
        );
    }
}

#[test]
fn a_landed_build_brings_its_ring_frame_its_ghosts_judge_and_the_selections_channel_from_the_worker() {
    let mut h = harness();
    ring_view(&mut h);
    // A plain band: the worker builds its ring frame too, so a first command or a Measure click has nothing left to build.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    assert!(h.state().command.band_surface().is_some(), "the worker built the ring frame of a design without parts");
    assert!(h.state().command.judge().is_some_and(|(_, prepared)| prepared), "and the ghost's judge over it, its lines laid out");
    // The claw solitaire with its head chosen: the viewport stages the channel once for the choice.
    claw_solitaire(&mut h);
    choose(&mut h, 3);
    let here = || crate::viewport::SELECT_STAGED_HERE.with(|c| c.get());
    let (judge, staged) = (h.state().command.judge(), here());
    // A rebuild with the choice held still lands with the worker's channel: the viewport stages nothing for it.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    h.run_steps(3);
    assert_eq!(here(), staged, "the landed build's channel came from the worker");
    let (again, prepared) = h.state().command.judge().expect("a judge with the build");
    assert!(prepared);
    assert_eq!(Some(again), judge.map(|(j, _)| j), "an unchanged band and plane keep the judge, and every line it has cast");
    // A choice made while the build is in flight is staged here, once, as before.
    h.state_mut().rebuild_now();
    choose(&mut h, 4);
    wait_for_build(&mut h);
    h.run_steps(3);
    assert!(here() > staged, "a moved selection restages on the UI thread");
}

/// The UI thread's share of a landed build and of a command's first frame on the claw solitaire, before this change and after; run `--ignored --nocapture`.
#[test]
#[ignore]
fn the_ui_threads_share_of_a_landed_build_on_the_claw_solitaire_measured() {
    use ringdesign_core::castability::ghost::GhostJudge;
    use ringdesign_core::interaction::pick::{Entity, Pick};
    use ringdesign_core::{AlphaLibrary, BuildParams, mesh};
    use ringdesign_workbench::command::BandSurface;
    use ringdesign_workbench::viewport::{Selection, tint};
    use std::time::Instant;
    let lib = AlphaLibrary::builtin();
    let court = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let design = ringdesign_core::RingDesign { cad: ringdesign_core::cad::examples::design("claw-solitaire").unwrap().cad, ..court };
    let best = |times: Vec<f64>| times.into_iter().fold(f64::MAX, f64::min);
    let timed = |f: &mut dyn FnMut()| {
        let t = Instant::now();
        f();
        t.elapsed().as_secs_f64() * 1e3
    };
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
    for (name, params) in [("preview 384x144", BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }), ("export 1024x320", BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })] {
        let built = mesh::try_build(&design, &lib, params).unwrap();
        let band = built.band.clone().unwrap();
        let bur = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 4).unwrap().mesh.clone();
        // The ring frame: master's UI thread built it, tree and all, for any design without parts on its first command or Measure click.
        let frame_ms = best((0..3).map(|_| timed(&mut || drop(BandSurface::shared(band.clone())))).collect());
        let surface = std::sync::Arc::new(BandSurface::shared(band.clone()));
        let frame_after_ms = best((0..20).map(|_| timed(&mut || drop(std::hint::black_box(surface.clone())))).collect());
        // The seat bur carried as a ghost: master's first cut read built the band's tree on the UI thread.
        let lazy_ms = best((0..3).map(|_| timed(&mut || drop(std::hint::black_box(GhostJudge::shared(&design, Some(band.clone()), 0.0).read(&bur, &identity, true))))).collect());
        let judge_ms = best((0..3).map(|_| timed(&mut || drop(GhostJudge::prepared_with(&design, Some(band.clone()), Some(surface.tree().clone()), 0.0)))).collect());
        let read_after_ms = best(
            (0..3)
                .map(|_| {
                    let j = GhostJudge::prepared_with(&design, Some(band.clone()), Some(surface.tree().clone()), 0.0);
                    timed(&mut || drop(std::hint::black_box(j.read(&bur, &identity, true))))
                })
                .collect(),
        );
        // The selection's channel with the head chosen and one of its faces hovered: master staged it on the UI thread for every landed build.
        let mut sel = Selection::default();
        sel.click(Some(Sel::Part(3)), ringdesign_workbench::viewport::Mods::default());
        let c3 = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap();
        let face = c3.trace.tri_face.first().copied().unwrap_or(0);
        sel.hovered(vec![Pick { entity: Entity::Face { feature: 3, face }, world: c3.frame.origin, normal: c3.frame.z_axis, depth: 1.0, px: 0.0 }]);
        let tint_ms = best((0..3).map(|_| timed(&mut || drop(std::hint::black_box(crate::viewport::GpuMeshRenderer::stage_select(&built.mesh, &tint(&sel, &built)))))).collect());
        eprintln!(
            "{name} ({} faces), the UI thread before → after: ring frame {frame_ms:.1} → {frame_after_ms:.4} ms (built on the worker beside the verdict); the bur's first cut read {lazy_ms:.1} → {read_after_ms:.2} ms (the judge {judge_ms:.2} ms on the worker over the frame's own tree); the selection's channel {tint_ms:.2} → 0 ms a landed build",
            built.mesh.faces.len()
        );
    }
}
