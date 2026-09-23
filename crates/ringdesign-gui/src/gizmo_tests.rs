//! The ring-frame gizmo, the ring dial, click-drag primitives and the shared grips in the Ring viewport.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage};
use ringdesign_workbench::viewport::{Mods, Sel};

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

/// The procedural shank and a 3 × 2.5 mm cylinder joined at the top of the Court band, built and settled.
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
    look(h, pane, std::f32::consts::FRAC_PI_2, 0.0);
    pane
}

/// Turns the pane's camera to `yaw`, `pitch` and frames the ring: (π/2, 0) looks down −y at the top, (−π/2, π/2) down the finger.
fn look(h: &mut Harness<'static, RingDesignerApp>, pane: usize, yaw: f32, pitch: f32) {
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.yaw = yaw;
        cam.pitch = pitch;
        cam.roll = 0.0;
        cam.fit(bounds);
    }
    h.run_steps(3);
}

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> egui::Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// A world point on screen through the pane's camera.
fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f64; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world.map(|v| v as f32))
}

/// Screen pixels per millimetre in the pane.
fn px_per_mm(h: &Harness<'static, RingDesignerApp>, pane: usize) -> f64 {
    f64::from(screen(h, pane, [0.0, 0.0, 0.0]).distance(screen(h, pane, [0.0, 0.0, 1.0])).max(screen(h, pane, [0.0, 0.0, 0.0]).distance(screen(h, pane, [1.0, 0.0, 0.0]))))
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

/// The pose every camera of the app holds, to hold it to not having moved.
fn pose(h: &Harness<'static, RingDesignerApp>, pane: usize) -> [f32; 7] {
    let c = h.state().panes[pane].camera;
    [c.yaw, c.pitch, c.roll, c.zoom, c.pan[0], c.pan[1], c.target[0]]
}

/// Clicks the cylinder's top, which chooses one of its faces.
fn click_the_post(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    let (lo, hi) = {
        let app = h.state();
        app.build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap().mesh.bounds().unwrap()
    };
    let top = screen(h, pane, [f64::from(lo.0 + hi.0) * 0.5, f64::from(hi.1), f64::from(lo.2 + hi.2) * 0.5]);
    click_at(h, top, PointerButton::Primary, Modifiers::NONE);
    assert!(matches!(h.state().selection.items.as_slice(), [Sel::Face { feature: POST, .. }]), "{:?}", h.state().selection.items);
    h.run_steps(2);
}

/// Where the handle labelled `label` is taken, as its AccessKit node says.
fn handle(h: &Harness<'static, RingDesignerApp>, label: &str) -> Pos2 {
    h.get_by_label(label).rect().center()
}

/// A primary press at the first point, moves through the rest with the button held, then the release.
fn drag_through(h: &mut Harness<'static, RingDesignerApp>, points: &[Pos2]) {
    let (first, last) = (points[0], *points.last().unwrap());
    h.event(Event::PointerMoved(first));
    h.event(Event::PointerButton { pos: first, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for p in &points[1..] {
        h.event(Event::PointerMoved(*p));
        h.run_steps(1);
    }
    h.event(Event::PointerButton { pos: last, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

/// `from` to `to` in eight steps.
fn line(from: Pos2, to: Pos2) -> Vec<Pos2> {
    (0..=8).map(|k| from + (to - from) * (k as f32 / 8.0)).collect()
}

/// The world origin of the post's seat: the crest at the top of the ring, a quarter millimetre out.
fn seat_origin(h: &Harness<'static, RingDesignerApp>) -> [f64; 3] {
    let d = &h.state().design;
    let r = d.inner_radius_mm() + d.profile.thickness_mm;
    [0.0, r + 0.25, 0.0]
}

#[test]
fn dragging_the_round_the_ring_arrow_moves_the_post_by_the_axis_projection_as_one_undo_step() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    click_the_post(&mut h, pane);
    let (entries, before, cam) = (h.state().history.present(), h.state().selection.items.clone(), pose(&h, pane));
    assert!(viewport_label(&h).contains("1 selected"));
    let press = handle(&h, "Gizmo: move round the ring");
    let to = press + egui::vec2(40.0, 0.0);
    drag_through(&mut h, &line(press, to));
    assert_eq!(live(&h), None, "the release commits");
    // Looking down −y the arrow runs screen right along −x: the axis point's angle round the finger is the reading.
    let (o, px) = (seat_origin(&h), px_per_mm(&h, pane));
    let origin = screen(&h, pane, o);
    let r0 = o[1];
    let t = |p: Pos2| f64::from(p.x - origin.x) / px;
    let expected = (t(to) / r0).atan().to_degrees() - (t(press) / r0).atan().to_degrees();
    let moved = post(&h).component.placement.theta_deg().unwrap() - 90.0;
    assert!((moved - expected).abs() < 0.5, "moved {moved:.3}° against {expected:.3}°");
    assert!(moved > 3.0, "{moved}");
    assert_eq!(h.state().history.present(), entries + 1, "one drag is one undo step");
    assert_eq!(h.state().selection.items, before, "the press never touched the selection");
    assert_eq!(pose(&h, pane), cam, "nor the camera");
    assert_eq!(h.state().status, "Place Cylinder");
    h.state_mut().undo();
    assert_eq!(post(&h).component.placement.theta_deg(), Some(90.0));
}

#[test]
fn a_number_typed_mid_drag_locks_the_handles_own_field() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    click_the_post(&mut h, pane);
    let entries = h.state().history.present();
    let press = handle(&h, "Gizmo: move round the ring");
    h.event(Event::PointerMoved(press));
    h.event(Event::PointerButton { pos: press, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=3 {
        h.event(Event::PointerMoved(press + egui::vec2(8.0 * k as f32, 0.0)));
        h.run_steps(1);
    }
    assert_eq!(live(&h), Some("move"));
    assert_eq!(h.state().command.dragging(), Some(ringdesign_workbench::gizmo::Handle::Move(ringdesign_workbench::command::Axis::Theta)));
    assert!(viewport_label(&h).contains("Move live"), "{}", viewport_label(&h));
    h.event(Event::Text("12".into()));
    h.run_steps(2);
    assert_eq!(h.get_by_label("Δθ (°)").value().as_deref(), Some("12"), "the round-the-ring field takes it");
    h.event(Event::PointerMoved(press + egui::vec2(60.0, 0.0)));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: press + egui::vec2(60.0, 0.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert_eq!(post(&h).component.placement.theta_deg(), Some(102.0), "the typed twelve holds against the pointer");
    assert_eq!(h.state().history.present(), entries + 1);
}

#[test]
fn the_height_arrow_stands_the_post_off_the_surface() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    look(&mut h, pane, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    let entries = h.state().history.present();
    assert!(h.query_by_label("Gizmo: move across the band").is_none(), "looking down the finger the across arrow points at the eye");
    let press = handle(&h, "Gizmo: move off the surface");
    let to = press + egui::vec2(0.0, -30.0);
    drag_through(&mut h, &line(press, to));
    let height = match post(&h).component.placement {
        Placement::Ring { height_mm, theta_deg, across_mm, .. } => {
            assert_eq!((theta_deg, across_mm), (90.0, 0.0), "only the stand-off moved");
            height_mm
        }
        other => panic!("{other:?}"),
    };
    let expected = 0.25 + 30.0 / px_per_mm(&h, pane);
    assert!((height - expected).abs() < 0.02, "{height} against {expected}");
    assert_eq!(h.state().history.present(), entries + 1);
}

#[test]
fn the_spin_ring_turned_a_quarter_spins_the_post_ninety_degrees() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    click_the_post(&mut h, pane);
    let press = handle(&h, "Gizmo: spin");
    let centre = screen(&h, pane, seat_origin(&h));
    let arm = press - centre;
    // A quarter turn counter-clockwise on screen: about the normal, which points at the eye.
    let arc: Vec<Pos2> = (0..=12)
        .map(|k| {
            let a = (k as f32 / 12.0) * std::f32::consts::FRAC_PI_2;
            centre + egui::vec2(arm.x * a.cos() + arm.y * a.sin(), -arm.x * a.sin() + arm.y * a.cos())
        })
        .collect();
    drag_through(&mut h, &arc);
    let spin = match post(&h).component.placement {
        Placement::Ring { spin_deg, theta_deg, tilt_deg, cant_deg, .. } => {
            assert_eq!((theta_deg, tilt_deg, cant_deg), (90.0, 0.0, 0.0));
            spin_deg
        }
        other => panic!("{other:?}"),
    };
    assert!((spin - 90.0).abs() < 1.0, "{spin}");
}

#[test]
fn escape_or_the_right_button_mid_drag_changes_nothing_and_the_drag_never_orbits() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    click_the_post(&mut h, pane);
    let (entries, doc, sel, cam) = (h.state().history.present(), document(&h), h.state().selection.items.clone(), pose(&h, pane));
    for right_button in [false, true] {
        let press = handle(&h, "Gizmo: move round the ring");
        h.event(Event::PointerMoved(press));
        h.event(Event::PointerButton { pos: press, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        h.run_steps(1);
        for k in 1..=4 {
            h.event(Event::PointerMoved(press + egui::vec2(6.0 * k as f32, 0.0)));
            h.run_steps(1);
        }
        assert_eq!(live(&h), Some("move"));
        assert!(h.state().command.session.preview().is_some_and(|p| p.placement.as_ref().and_then(Placement::theta_deg) != Some(90.0)), "the ghost follows the drag");
        let cancel = if right_button {
            let pos = press + egui::vec2(24.0, 0.0);
            [Event::PointerButton { pos, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }, Event::PointerButton { pos, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }]
        } else {
            [true, false].map(|pressed| Event::Key { key: Key::Escape, pressed, modifiers: Modifiers::NONE, repeat: false, physical_key: None })
        };
        let [down, up] = cancel;
        h.event(down);
        h.run_steps(1);
        h.event(up);
        h.run_steps(1);
        let how = if right_button { "the right button" } else { "Escape" };
        assert_eq!(live(&h), None, "{how} cancels");
        // The rest of the press is the gizmo's: it neither moves the post nor orbits.
        for k in 5..=9 {
            h.event(Event::PointerMoved(press + egui::vec2(6.0 * k as f32, 3.0 * k as f32)));
            h.run_steps(1);
        }
        h.event(Event::PointerButton { pos: press + egui::vec2(54.0, 27.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        h.run_steps(2);
        assert_eq!((document(&h), h.state().history.present()), (doc.clone(), entries), "{how}: nothing changed");
        assert_eq!(h.state().selection.items, sel, "{how}");
        assert_eq!(pose(&h, pane), cam, "{how}");
        assert!(!egui::Popup::is_any_open(&h.ctx), "{how} opens no menu");
    }
}

#[test]
fn a_press_on_any_handle_leaves_the_camera_the_selection_and_the_design_alone() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    click_the_post(&mut h, pane);
    let (entries, doc, sel, cam) = (h.state().history.present(), document(&h), h.state().selection.items.clone(), pose(&h, pane));
    let handles: Vec<(String, Pos2)> = h
        .query_all_by_label_contains(": ")
        .filter_map(|n| n.accesskit_node().label().filter(|l| l.starts_with("Gizmo: ") || l.starts_with("Grip: ")).map(|l| (l.to_string(), n.rect().center())))
        .collect();
    let labels: Vec<&str> = handles.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Gizmo: move round the ring", "Gizmo: move across the band", "Gizmo: spin", "Grip: Radius"], "looking down −y at the top");
    for (label, at) in &handles {
        click_at(&mut h, *at, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(live(&h), None, "{label}");
        assert_eq!((document(&h), h.state().history.present()), (doc.clone(), entries), "{label}: a press in place changes nothing");
        assert_eq!(h.state().selection.items, sel, "{label}");
        assert_eq!(pose(&h, pane), cam, "{label}");
    }
    // A handle under the pointer lights instead of the part behind it.
    h.hover_at(handles[0].1);
    h.run_steps(2);
    assert!(h.state().command.gizmo_hot().is_some());
    assert!(h.state().selection.hover.is_none(), "{:?}", h.state().selection.hover);
    // Shift builds the selection, handle or not: a Shift-click on the spin ring adds what lies under it.
    let ring = handles.iter().find(|(l, _)| l == "Gizmo: spin").unwrap().1;
    click_at(&mut h, ring, PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().selection.items.len(), 2, "{:?}", h.state().selection.items);
    assert_eq!((document(&h), pose(&h, pane)), (doc.clone(), cam));
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    // A command live puts the gizmo away.
    h.hover_at(screen(&h, pane, seat_origin(&h)));
    h.run_steps(1);
    h.key_press(Key::R);
    h.run_steps(2);
    assert_eq!(live(&h), Some("rotate"));
    assert!(h.query_all_by_label_contains("Gizmo: ").next().is_none());
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.query_by_label("Gizmo: spin").is_some(), "and it comes back when the command ends");
}

/// The bare band as the build seats parts on it, for the stand-off a part keeps.
fn band_mesh(h: &Harness<'static, RingDesignerApp>) -> ringdesign_core::Mesh {
    let app = h.state();
    let mut bare = app.design.clone();
    bare.cad = None;
    ringdesign_core::mesh::build(&bare, &app.lib, app.preview_params).mesh
}

/// The post's centre and how far it stands off the bare band along the normal at its seat.
fn stand_off(h: &Harness<'static, RingDesignerApp>, band: &ringdesign_core::Mesh) -> f64 {
    let app = h.state();
    let c = app.build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap();
    let (lo, hi) = c.mesh.bounds().unwrap();
    let centre = [f64::from(lo.0 + hi.0) * 0.5, f64::from(lo.1 + hi.1) * 0.5, f64::from(lo.2 + hi.2) * 0.5];
    let theta = post(h).component.placement.theta_deg().unwrap();
    let (hit, n) = ringdesign_core::cad::surface_hit(band, theta, 0.0).expect("the band under the post");
    (0..3).map(|k| (centre[k] - hit[k]) * n[k]).sum()
}

#[test]
fn the_dial_slides_the_post_round_the_shank_to_sixty_degrees_and_it_stays_seated() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    let band = band_mesh(&h);
    let before = stand_off(&h, &band);
    assert!((before - 0.25).abs() < 0.02, "{before}");
    look(&mut h, pane, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    let entries = h.state().history.present();
    let marker = handle(&h, "Gizmo: slide round the shank");
    let (hit, _) = ringdesign_core::cad::surface_hit(&band, 90.0, 0.0).unwrap();
    let r = hit[0].hypot(hit[1]);
    assert!(marker.distance(screen(&h, pane, [0.0, r, 0.0])) < 1.5, "the marker stands at the post's angle on the crest");
    let at = |deg: f64| screen(&h, pane, [r * deg.to_radians().cos(), r * deg.to_radians().sin(), 0.0]);
    let path: Vec<Pos2> = (0..=10).map(|k| at(90.0 - 3.0 * f64::from(k))).collect();
    drag_through(&mut h, &path);
    assert_eq!(post(&h).component.placement, Placement::ring(60.0, 0.25), "theta alone moves, onto the 5° grid");
    assert_eq!(h.state().history.present(), entries + 1);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let after = stand_off(&h, &band);
    assert!((after - before).abs() < 0.02, "the surface drop keeps the stand-off: {before} -> {after}");
}

/// The value a dimension field shows as its hint, before anything is typed into it.
fn shown(h: &Harness<'static, RingDesignerApp>, label: &str) -> f64 {
    let hint = h.get_by_label(label).accesskit_node().placeholder().map(str::to_owned).unwrap_or_default();
    ringdesign_workbench::command::parse_value(&hint).unwrap_or_else(|| panic!("{label} shows {hint:?}"))
}

#[test]
fn a_cylinder_pressed_dragged_and_released_on_the_ring_then_lifted_takes_the_sizes_the_bar_showed() {
    let mut h = harness();
    let pane = ring_view(&mut h);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    look(&mut h, pane, std::f32::consts::FRAC_PI_2, 0.0);
    let d = h.state().design.clone();
    let r = d.inner_radius_mm() + d.profile.thickness_mm;
    let at = screen(&h, pane, [r * 70f64.to_radians().cos(), r * 70f64.to_radians().sin(), 0.0]);
    let (entries, cam) = (h.state().history.present(), pose(&h, pane));
    h.hover_at(at);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::A);
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(3);
    assert_eq!(live(&h), Some("add-cylinder"));
    // The press seats the base, the drag sizes it and the release fixes it.
    h.event(Event::PointerMoved(at));
    h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    assert_eq!(h.state().command.session.command().map(|c| c.step()), Some(1), "the press set the centre");
    let end = at + egui::vec2(40.0, 0.0);
    for p in &line(at, end)[1..] {
        h.event(Event::PointerMoved(*p));
        h.run_steps(1);
    }
    let radius = shown(&h, "Radius (mm)");
    assert!((radius - 40.0 / px_per_mm(&h, pane)).abs() < 0.02, "{radius} against {} at {} px/mm", 40.0 / px_per_mm(&h, pane), px_per_mm(&h, pane));
    h.event(Event::PointerButton { pos: end, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert_eq!(h.state().command.session.command().map(|c| c.step()), Some(2), "the release fixed the radius");
    // Then the pointer lifts the height, and a click confirms.
    let lifted = end + egui::vec2(0.0, -30.0);
    for p in &line(end, lifted)[1..] {
        h.hover_at(*p);
        h.run_steps(1);
    }
    let height = shown(&h, "Height (mm)");
    click_at(&mut h, lifted, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(live(&h), None);
    let doc = h.state().design.cad.clone().expect("the part and its shank");
    let cyl = doc.features.iter().find(|f| f.name == "Cylinder").expect("the cylinder");
    let Operation::Cylinder { radius_mm, height_mm } = cyl.operation else { panic!("{:?}", cyl.operation) };
    assert!((radius_mm - radius).abs() < 0.006 && (height_mm - height).abs() < 0.006, "{radius_mm} × {height_mm} against the bar's {radius} × {height}");
    assert!(matches!(cyl.component.placement, Placement::Ring { theta_deg, across_mm, .. } if theta_deg == 70.0 && across_mm == 0.0), "{:?}", cyl.component.placement);
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!(pose(&h, pane), cam, "the drag never orbited");
}

#[test]
fn a_plain_click_still_seats_a_typed_sphere() {
    let mut h = harness();
    let pane = ring_view(&mut h);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    look(&mut h, pane, std::f32::consts::FRAC_PI_2, 0.0);
    let d = h.state().design.clone();
    let r = d.inner_radius_mm() + d.profile.thickness_mm;
    let at = screen(&h, pane, [r * 100f64.to_radians().cos(), r * 100f64.to_radians().sin(), 0.0]);
    h.hover_at(at);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::SHIFT, Key::A);
    h.run_steps(3);
    h.get_by_label("Sphere").click();
    h.run_steps(3);
    click_at(&mut h, at, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(h.state().command.session.command().map(|c| c.step()), Some(1), "a click without a drag leaves the size to come");
    h.event(Event::Text("0.8".into()));
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let doc = h.state().design.cad.clone().expect("the sphere and its shank");
    let s = doc.features.iter().find(|f| f.name == "Sphere").expect("the sphere");
    assert!(matches!(s.operation, Operation::Sphere { radius_mm } if radius_mm == 0.8), "{:?}", s.operation);
    assert_eq!(s.component.placement.theta_deg(), Some(100.0));
}

#[test]
fn dragging_the_radius_grip_resizes_the_post_as_one_undo_step() {
    let mut h = harness();
    let pane = band_and_cylinder(&mut h);
    click_the_post(&mut h, pane);
    let (entries, cam) = (h.state().history.present(), pose(&h, pane));
    let press = handle(&h, "Grip: Radius");
    // The radius grip stands on the post's x, down the finger: down the screen.
    assert!(press.y > screen(&h, pane, seat_origin(&h)).y);
    h.event(Event::PointerMoved(press));
    h.event(Event::PointerButton { pos: press, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=4 {
        h.event(Event::PointerMoved(press + egui::vec2(0.0, 5.0 * k as f32)));
        h.run_steps(1);
    }
    assert_eq!(live(&h), Some("grip"));
    assert!(h.state().command.session.preview().is_some_and(|p| matches!(p.operation, Some(Operation::Cylinder { radius_mm, .. }) if radius_mm > 1.5)), "the ghost carries the new size");
    h.event(Event::PointerButton { pos: press + egui::vec2(0.0, 20.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    let Operation::Cylinder { radius_mm, height_mm } = post(&h).operation else { panic!() };
    let expected = 1.5 + 20.0 / px_per_mm(&h, pane);
    assert!((radius_mm - expected).abs() < 0.02 && height_mm == 2.5, "{radius_mm} against {expected}");
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!(pose(&h, pane), cam);
    assert_eq!(h.state().status, "Edit Cylinder");
}

#[test]
fn a_free_part_gets_the_worlds_axes_and_a_drag_along_x_wraps_it_in_a_transform() {
    let mut h = harness();
    let pane = ring_view(&mut h);
    {
        let app = h.state_mut();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Stud".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 4.0, height_mm: 3.0 }, component: Component::default() }).unwrap();
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(&mut h);
    look(&mut h, pane, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    h.state_mut().panes[pane].camera.zoom = 0.6;
    h.state_mut().selection.click(Some(Sel::Part(1)), Mods::default());
    h.run_steps(3);
    let entries = h.state().history.present();
    // Looking down the finger: X across the screen, Y up it, Z at the eye.
    for (label, shown) in [("Gizmo: move along X", true), ("Gizmo: move along Y", true), ("Gizmo: move along Z", false), ("Gizmo: turn about Z", true), ("Gizmo: turn about X", false), ("Grip: Radius", true)] {
        assert_eq!(h.query_by_label(label).is_some(), shown, "{label}");
    }
    assert!(h.query_by_label("Gizmo: slide round the shank").is_none(), "a free part has no dial");
    let press = handle(&h, "Gizmo: move along X");
    drag_through(&mut h, &line(press, press + egui::vec2(30.0, -9.0)));
    let doc = h.state().design.cad.clone().expect("the document");
    let moved = doc.features.iter().find(|f| matches!(f.operation, Operation::Transform { source: 1, .. })).expect("a transform wrapping the stud");
    let Operation::Transform { translation, rotation_deg, .. } = moved.operation else { unreachable!() };
    let expected = 30.0 / px_per_mm(&h, pane);
    assert!((translation[0] - expected).abs() < 0.02 && translation[1] == 0.0 && translation[2] == 0.0 && rotation_deg == [0.0; 3], "{translation:?} against {expected}");
    assert_eq!(h.state().selection.items, [Sel::Part(moved.id)], "the moved part is the one chosen");
    assert_eq!(h.state().history.present(), entries + 1);
}

#[test]
fn the_cad_panes_radius_grip_follows_the_pointer_along_its_own_axis() {
    let mut h = harness();
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    h.run_steps(4);
    h.get_by_label("Create").click();
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(4);
    h.key_press(Key::Enter);
    h.run_steps(3);
    wait_until_previewed(&mut h);
    h.run_steps(2);
    let grip = |h: &Harness<'static, RingDesignerApp>| h.get_by_label("Dimension grip: Radius").rect().center();
    let radius = |h: &Harness<'static, RingDesignerApp>| {
        h.query_all_by_label("Radius mm").find(|n| n.accesskit_node().role() == egui::accesskit::Role::SpinButton).and_then(|n| n.value()).and_then(|v| v.parse::<f64>().ok()).expect("the radius field")
    };
    let start = radius(&h);
    let (from, before) = (grip(&h), start);
    let drag = egui::vec2(40.0, 12.0);
    drag_through(&mut h, &line(from, from + drag));
    h.run_steps(2);
    let (to, after) = (grip(&h), radius(&h));
    // The grip lands where the pointer's move projects onto its own screen axis.
    let moved = to - from;
    assert!(moved.length() > 5.0 && after > before, "{moved:?} {before} -> {after}");
    let along = drag.dot(moved.normalized());
    assert!((along - moved.length()).abs() < 1.0, "the pointer moved {along:.2} px along the axis, the grip {:.2} px", moved.length());
}

/// The gizmo's per-frame work and a drag's on an export build; run `--ignored --nocapture`.
#[test]
#[ignore]
fn the_gizmo_frame_and_a_drag_frame_on_an_export_build() {
    use ringdesign_core::{AlphaLibrary, BuildParams, mesh};
    use ringdesign_workbench::command::{BandSurface, MoveCmd, Session, StepInput, placed_ghost};
    use ringdesign_workbench::gizmo::{self, Gizmo, Handle, View};
    use std::time::Instant;
    let lib = AlphaLibrary::builtin();
    let mut design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    let target = Feature { id: POST, name: "Cylinder".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, component: Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.25), ..Default::default() } };
    doc.append(target.clone()).unwrap();
    design.cad = Some(doc);
    for (name, params) in [("preview 384x144", BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }), ("export 1024x320", BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })] {
        let built = mesh::build(&design, &lib, params);
        let mut bare = design.clone();
        bare.cad = None;
        let band = BandSurface::new(mesh::try_build(&bare, &lib, params).unwrap().mesh);
        let part = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == POST).unwrap().mesh.clone();
        // An orthographic three-quarter view at 25 px/mm.
        let (fwd, right, up) = ([-0.5f64, -0.6, -0.62], [0.768, -0.64, 0.0], [-0.397, -0.476, 0.785]);
        let n = |v: [f64; 3]| {
            let l = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            v.map(|x| x / l)
        };
        let (fwd, right, up) = (n(fwd), n(right), n(up));
        let project = |p: [f64; 3]| egui::pos2((400.0 + 25.0 * (right[0] * p[0] + right[1] * p[1] + right[2] * p[2])) as f32, (300.0 - 25.0 * (up[0] * p[0] + up[1] * p[1] + up[2] * p[2])) as f32);
        let ray = |s: Pos2| {
            let (x, y) = (f64::from(s.x - 400.0) / 25.0, -f64::from(s.y - 300.0) / 25.0);
            ringdesign_core::interaction::pick::Ray { origin: std::array::from_fn(|k| right[k] * x + up[k] * y - fwd[k] * 60.0), direction: fwd }
        };
        let view = View { project: &project, forward: fwd, px_per_mm: 25.0 };
        let ctx = egui::Context::default();
        let frames = 1000;
        let (mut build_us, mut layout_us, mut hit_us, mut paint_us) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut layout = None;
        for i in 0..frames {
            let t = Instant::now();
            let g = Gizmo::on_ring(&design, Some(&band), &target.component.placement, 0.0).unwrap();
            let g = Gizmo { reach_mm: gizmo::reach(&part, g.origin), ..g }.with_grips(&target.operation);
            build_us += t.elapsed().as_secs_f64() * 1e6;
            let t = Instant::now();
            let l = g.layout(&view);
            layout_us += t.elapsed().as_secs_f64() * 1e6;
            let t = Instant::now();
            std::hint::black_box(l.hit(egui::pos2(300.0 + (i % 200) as f32, 200.0 + (i % 150) as f32)));
            hit_us += t.elapsed().as_secs_f64() * 1e6;
            layout = Some(l);
        }
        let layout = layout.unwrap();
        let t = Instant::now();
        for _ in 0..frames {
            std::hint::black_box(layout.handles().filter_map(|h| layout.anchor(h)).count());
        }
        let anchors_us = t.elapsed().as_secs_f64() * 1e6 / frames as f64;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            for _ in 0..frames {
                let t = Instant::now();
                gizmo::paint(&painter, &layout, Some(Handle::Turn(ringdesign_workbench::command::Axis::Spin)), None);
                paint_us += t.elapsed().as_secs_f64() * 1e6;
            }
        });
        out.textures_delta.clear();
        let shapes = out.shapes.len() / frames;
        let t = Instant::now();
        let meshes = ctx.tessellate(out.shapes, 1.0);
        let tessellate_us = t.elapsed().as_secs_f64() * 1e6 / frames as f64;
        std::hint::black_box(meshes);
        // A drag's frame: the ray read against the handle, the command fed, the ghost's matrix.
        let g = Gizmo::on_ring(&design, Some(&band), &target.component.placement, 1.95).unwrap();
        let mut session = Session::default();
        session.start(Box::new(MoveCmd::of(&target, 9)));
        session.feed(StepInput::Lock(ringdesign_workbench::command::Axis::Theta));
        let press = layout.anchor(Handle::Move(ringdesign_workbench::command::Axis::Theta)).unwrap();
        let (mut drag_us, mut worst) = (0.0f64, 0.0f64);
        for i in 0..frames {
            let t = Instant::now();
            let token = g.token(Handle::Move(ringdesign_workbench::command::Axis::Theta), ray(press + egui::vec2((i % 80) as f32, 0.0)), None).unwrap();
            session.feed(token);
            let preview = session.preview().unwrap();
            std::hint::black_box(placed_ghost(&design, Some(&band), &target, &preview).unwrap());
            let us = t.elapsed().as_secs_f64() * 1e6;
            drag_us += us;
            worst = worst.max(us);
        }
        let f = frames as f64;
        eprintln!(
            "{name} ({} faces): gizmo seat + reach + grips {:.1} µs, layout {:.1} µs, hit {:.2} µs, paint {:.1} µs + tessellate {tessellate_us:.1} µs ({shapes} shapes) a frame, anchors {anchors_us:.1} µs with AccessKit on; drag frame (token + command + ghost matrix) mean {:.1} µs, worst {worst:.1} µs",
            built.mesh.faces.len(),
            build_us / f,
            layout_us / f,
            hit_us / f,
            paint_us / f,
            drag_us / f
        );
    }
}
