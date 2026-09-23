//! Ring-aware snaps, Measure, pins and the ghost's castability tint in the Ring viewport.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage, builders};
use ringdesign_workbench::viewport::{Mods, Sel};
use ringdesign_workbench::visual::Tool;

/// The post every test carries.
const POST: u64 = 2;

/// The procedural shank, a 3 × 2.5 mm post joined at the top standing 0.25 mm off, and `more` features, built on one Ring viewport.
fn ring_with(h: &mut Harness<'static, RingDesignerApp>, more: Vec<Feature>) -> usize {
    let pane = {
        let app = h.state_mut();
        app.switch_desktop(crate::dock::Desktop::Model);
        app.set_layout(crate::pane::Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = crate::pane::PaneKind::Solid;
        app.active_pane = pane;
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(post(POST, "Post", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Placement::ring(90.0, 0.25))).unwrap();
        for f in more {
            doc.append(f).unwrap();
        }
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
        app.rebuild_now();
        pane
    };
    wait_for_build(h);
    pane
}

/// A part joined to the band at `placement`.
fn post(id: u64, name: &str, operation: Operation, placement: Placement) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { attach: Attach::Join, stage: Stage::Cast, placement, ..Default::default() } }
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
fn down_the_finger(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    look(h, pane, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
}
fn down_at_the_top(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    look(h, pane, std::f32::consts::FRAC_PI_2, 0.0);
}
/// Pans the view down the pane, clear of the tool inspector that floats over its middle.
fn clear_of_the_inspector(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    let rect = viewport_rect(h);
    h.state_mut().panes[pane].camera.pan_by(egui::vec2(-60.0, 200.0), rect);
    h.run_steps(2);
}

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> egui::Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// A world point on screen through the pane's camera.
fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f64; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world.map(|v| v as f32))
}

/// A point of the band's near side face at `theta`, half a millimetre in from the crest, as looking down the finger shows it.
fn on_the_side(h: &Harness<'static, RingDesignerApp>, pane: usize, theta: f64) -> Pos2 {
    let d = &h.state().design;
    let r = d.inner_radius_mm() + d.profile.thickness_mm - 0.5;
    let (s, c) = theta.to_radians().sin_cos();
    screen(h, pane, [r * c, r * s, d.profile.width_mm * 0.5])
}

fn part(h: &Harness<'static, RingDesignerApp>, id: u64) -> Feature {
    h.state().design.cad.as_ref().and_then(|d| d.feature(id)).cloned().expect("the part")
}

fn press(h: &mut Harness<'static, RingDesignerApp>, key: Key) {
    h.key_press(key);
    h.run_steps(2);
}

/// Hovers `at` and lets the viewport sample it.
fn hover(h: &mut Harness<'static, RingDesignerApp>, at: Pos2) {
    h.hover_at(at);
    h.run_steps(2);
}

/// Hovers the band's side face at `theta`.
fn hover_side(h: &mut Harness<'static, RingDesignerApp>, pane: usize, theta: f64) {
    let at = on_the_side(h, pane, theta);
    hover(h, at);
}

/// Clicks a world point with `modifiers` held.
fn click_world(h: &mut Harness<'static, RingDesignerApp>, pane: usize, world: [f64; 3], offset: egui::Vec2, modifiers: Modifiers) {
    let at = screen(h, pane, world) + offset;
    click_at(h, at, PointerButton::Primary, modifiers);
}

fn caption(h: &Harness<'static, RingDesignerApp>) -> String {
    h.state().command.session.preview().map(|p| p.caption).unwrap_or_default()
}

#[test]
fn a_g_move_near_the_palm_lands_the_part_on_270_and_says_so() {
    let mut h = harness();
    let pane = ring_with(&mut h, Vec::new());
    down_the_finger(&mut h, pane);
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    let entries = h.state().history.present();
    hover_side(&mut h, pane, 90.0);
    press(&mut h, Key::G);
    assert_eq!(h.state().command.session.command().map(|c| c.key()), Some("move"));
    hover_side(&mut h, pane, 90.0);
    // Carried half a degree past the palm, the post lands on it.
    hover_side(&mut h, pane, 270.5);
    assert!(caption(&h).contains(" · palm 270.0° · parting line"), "{}", caption(&h));
    let snap = h.state().command.snapped().cloned().expect("the landing snapped");
    assert_eq!((snap.kind, snap.ring.theta_deg), (ringdesign_workbench::command::SnapKind::Angle, 270.0));
    assert!(snap.label.starts_with("palm 270.0°"), "the marker says where it landed: {}", snap.label);
    press(&mut h, Key::Enter);
    let Placement::Ring { theta_deg, across_mm, .. } = part(&h, POST).component.placement else { panic!() };
    assert_eq!((theta_deg, across_mm), (270.0, 0.0), "exactly the palm, on the parting line");
    assert_eq!(h.state().history.present(), entries + 1);
    // Ctrl frees it: the same carry lands half a degree past.
    h.state_mut().undo();
    hover_side(&mut h, pane, 90.0);
    press(&mut h, Key::G);
    hover_side(&mut h, pane, 90.0);
    h.event(Event::ModifiersChanged(Modifiers::COMMAND));
    hover_side(&mut h, pane, 270.5);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    let free = h.state().command.session.preview().and_then(|p| p.placement).and_then(|p| p.theta_deg()).unwrap();
    assert!((free - 270.5).abs() < 0.2 && free != 270.0, "{free}");
    press(&mut h, Key::Escape);
}

#[test]
fn a_move_reads_the_pointer_under_the_dimension_bar_and_the_bar_holds_still_there() {
    let mut h = harness();
    let pane = ring_with(&mut h, Vec::new());
    down_the_finger(&mut h, pane);
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    hover_side(&mut h, pane, 90.0);
    press(&mut h, Key::G);
    hover_side(&mut h, pane, 90.0);
    let bar = |h: &Harness<'static, RingDesignerApp>| h.ctx.memory(|m| m.area_rect(egui::Id::new("ring-viewport-dimensions").with("area"))).expect("the bar is shown");
    let (view, r) = (viewport_rect(&h), bar(&h));
    assert!(view.contains_rect(r.shrink(0.5)), "the bar stands inside the view: {r:?} in {view:?}");
    // Carried to a point of the band well inside the bar, clear of egui's 5 px search radius at its edge, snaps freed:
    // the part lands there and the bar stays put.
    let theta = (91..180).map(f64::from).find(|t| r.shrink(8.0).contains(on_the_side(&h, pane, *t))).expect("the band passes under the bar");
    h.event(Event::ModifiersChanged(Modifiers::COMMAND));
    hover_side(&mut h, pane, theta);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    assert_eq!(bar(&h), r, "the bar holds still under the pointer");
    let at = h.state().command.session.preview().and_then(|p| p.placement).and_then(|p| p.theta_deg()).unwrap();
    assert!((at - theta).abs() < 1.0, "landed at {at}° for the band at {theta}°");
    press(&mut h, Key::Escape);
}

#[test]
fn a_part_dragged_near_a_stone_station_lands_on_it() {
    let mut h = harness();
    let gem = builders::stone_preset("round-5").unwrap().gem();
    let pane = ring_with(&mut h, vec![builders::stone_feature(3, gem, Placement::ring(135.0, 0.0))]);
    down_the_finger(&mut h, pane);
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    hover_side(&mut h, pane, 90.0);
    press(&mut h, Key::G);
    hover_side(&mut h, pane, 90.0);
    hover_side(&mut h, pane, 134.6);
    let snap = h.state().command.snapped().cloned().expect("the landing snapped");
    assert_eq!((snap.kind, snap.label.as_str()), (ringdesign_workbench::command::SnapKind::Station, "stone Round 5 mm"));
    assert!(caption(&h).ends_with(" · stone Round 5 mm"), "{}", caption(&h));
    press(&mut h, Key::Enter);
    let Placement::Ring { theta_deg, across_mm, height_mm, .. } = part(&h, POST).component.placement else { panic!() };
    assert_eq!((theta_deg, across_mm), (135.0, 0.0), "on the stone's seat");
    assert!((height_mm - 0.25).abs() < 1e-3, "keeping its own stand-off: {height_mm}");
}

#[test]
fn the_dial_lands_on_another_parts_angle_rather_than_its_grid() {
    let mut h = harness();
    let other = post(3, "Post B", Operation::Cylinder { radius_mm: 0.8, height_mm: 1.5 }, Placement::ring(47.0, 0.0));
    let pane = ring_with(&mut h, vec![other]);
    down_the_finger(&mut h, pane);
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    let entries = h.state().history.present();
    let marker = h.get_by_label("Gizmo: slide round the shank").rect().center();
    let r = {
        let b = h.state().build.clone().unwrap();
        let (hit, _) = ringdesign_core::cad::surface_hit(b.band.as_ref().unwrap(), 90.0, 0.0).unwrap();
        hit[0].hypot(hit[1])
    };
    let at = |h: &Harness<'static, RingDesignerApp>, deg: f64| screen(h, pane, [r * deg.to_radians().cos(), r * deg.to_radians().sin(), 0.0]);
    let mut path = vec![marker];
    path.extend((1..=10).map(|k| at(&h, 90.0 - 4.27 * f64::from(k))));
    h.event(Event::PointerMoved(path[0]));
    h.event(Event::PointerButton { pos: path[0], button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for p in &path[1..] {
        h.event(Event::PointerMoved(*p));
        h.run_steps(1);
    }
    assert!(caption(&h).contains("Post B 47.0°"), "{}", caption(&h));
    let last = *path.last().unwrap();
    h.event(Event::PointerButton { pos: last, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert_eq!(part(&h, POST).component.placement, Placement::ring(47.0, 0.25), "the other part's angle, not the grid's 45");
    assert_eq!(h.state().history.present(), entries + 1);
}

#[test]
fn the_tool_inspector_opens_against_the_viewports_right_edge_clear_of_its_middle() {
    let mut h = harness();
    ring_with(&mut h, Vec::new());
    h.state_mut().visual.select(Tool::Measure);
    h.run_steps(3);
    let view = viewport_rect(&h);
    let r = h.ctx.memory(|m| m.area_rect(egui::Id::new("direct-viewport-inspector"))).expect("the inspector is open");
    assert!(r.right() <= view.right() && r.right() >= view.right() - 30.0 && r.left() > view.center().x, "{r:?} in {view:?}");
}

#[test]
fn measure_between_two_posts_top_vertices_reads_their_true_distance() {
    let mut h = harness();
    let boxed = |id, theta| post(id, "Box", Operation::Box { size: [1.5, 1.5, 2.0] }, Placement::ring(theta, 0.0));
    let pane = ring_with(&mut h, vec![boxed(3, 60.0), boxed(4, 120.0)]);
    down_at_the_top(&mut h, pane);
    clear_of_the_inspector(&mut h, pane);
    h.state_mut().visual.select(Tool::Measure);
    h.run_steps(2);
    // Each box's top corner nearest the viewer's up, farthest along the finger.
    let top = |h: &Harness<'static, RingDesignerApp>, id: u64| -> [f64; 3] {
        let b = h.state().build.clone().unwrap();
        let c = b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap();
        let mut v = c.trace.vertices.clone();
        v.sort_by(|a, b| (b[0].hypot(b[1])).total_cmp(&a[0].hypot(a[1])));
        v.truncate(4);
        *v.iter().max_by(|a, b| a[2].total_cmp(&b[2]).then(a[0].total_cmp(&b[0]))).unwrap()
    };
    let (a, b) = (top(&h, 3), top(&h, 4));
    click_world(&mut h, pane, a, egui::Vec2::ZERO, Modifiers::NONE);
    click_world(&mut h, pane, b, egui::Vec2::ZERO, Modifiers::NONE);
    let picks = &h.state().visual.measurement.picks;
    assert_eq!(picks.len(), 2);
    assert_eq!((picks[0].at(), picks[1].at()), (a, b), "each pick is the vertex itself");
    assert!(picks[0].label().contains("vertex"), "{}", picks[0].label());
    let label = h.query_all_by_label_contains("Measure: ").next().expect("the reading is named for a reader").accesskit_node().label().unwrap_or_default();
    let read: f64 = label.trim_start_matches("Measure: Distance ").trim_end_matches(" mm").parse().unwrap_or_else(|_| panic!("{label}"));
    let truth = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    assert!((read - truth).abs() < 1e-3, "{read} against {truth}");
    // Shift chains a third pick onto the band, snapped to the parting line, and reads the corner at the second.
    let d = h.state().design.clone();
    let crest = d.inner_radius_mm() + d.profile.thickness_mm;
    let (s, c) = 75f64.to_radians().sin_cos();
    click_world(&mut h, pane, [crest * c, crest * s, 0.0], egui::Vec2::ZERO, Modifiers::SHIFT);
    let third = h.state().visual.measurement.picks.get(2).map(|p| p.label().to_owned());
    assert_eq!(third.as_deref(), Some("parting line"));
    let readings = h.state().visual.measurement.readings();
    assert_eq!(readings.iter().map(|r| r.what).collect::<Vec<_>>(), ["Distance", "Distance", "Corner"]);
    assert_eq!(h.query_all_by_label_contains("Measure: ").count(), 3);
    // Escape clears the measurement first, and only then leaves the tool.
    press(&mut h, Key::Escape);
    assert!(h.state().visual.measurement.picks.is_empty());
    assert_eq!(h.state().visual.tool, Tool::Measure);
    press(&mut h, Key::Escape);
    assert_eq!(h.state().visual.tool, Tool::Select);
}

#[test]
fn choosing_a_part_and_carrying_it_sweeps_no_band_on_the_ui_thread() {
    let mut h = harness();
    let pane = ring_with(&mut h, Vec::new());
    down_at_the_top(&mut h, pane);
    let before = ringdesign_core::mesh::sweeps();
    // The gizmo reads the ring frame from the first frame the part is chosen.
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(3);
    let at = h.get_by_label("Gizmo: move round the ring").rect().center();
    hover(&mut h, at + egui::vec2(0.0, 30.0));
    press(&mut h, Key::G);
    hover(&mut h, at + egui::vec2(20.0, 30.0));
    press(&mut h, Key::Escape);
    assert_eq!(ringdesign_core::mesh::sweeps(), before, "the band came with the build");
    let surface = h.state().command.band_surface().expect("the ring frame was read");
    // A build of the same band keeps the same frame; the worker swept the one band there is.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    h.run_steps(2);
    assert_eq!(h.state().command.band_surface(), Some(surface));
    assert_eq!(ringdesign_core::mesh::sweeps(), before);
}

#[test]
fn a_pin_dropped_on_the_band_lands_a_part_and_measure_reads_it() {
    let mut h = harness();
    let pane = ring_with(&mut h, Vec::new());
    down_at_the_top(&mut h, pane);
    clear_of_the_inspector(&mut h, pane);
    let d = h.state().design.clone();
    let crest = d.inner_radius_mm() + d.profile.thickness_mm;
    let (s, c) = 70f64.to_radians().sin_cos();
    let spot = screen(&h, pane, [crest * c, crest * s, 0.6]);
    click_at(&mut h, spot, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Pin here").click();
    h.run_steps(3);
    let pin = h.state().pins().first().cloned().expect("a pin");
    assert_eq!(pin.name, "Pin 1");
    assert!((pin.theta_deg - 70.0).abs() < 0.3 && (pin.across_mm - 0.6).abs() < 0.05, "{pin:?}");
    assert!(h.state().status.starts_with("Pin 1 at "), "{}", h.state().status);
    // P lands the post on the pin, whole: round the ring and along the finger.
    h.state_mut().selection.click(Some(Sel::Part(POST)), Mods::default());
    h.run_steps(2);
    let near = screen(&h, pane, pin.world) + egui::vec2(2.0, 1.0);
    hover(&mut h, near);
    press(&mut h, Key::P);
    hover(&mut h, near + egui::vec2(-1.0, 0.0));
    assert!(caption(&h).contains(" · Pin 1"), "{}", caption(&h));
    press(&mut h, Key::Enter);
    let Placement::Ring { theta_deg, across_mm, .. } = part(&h, POST).component.placement else { panic!() };
    assert_eq!((theta_deg, across_mm), (pin.theta_deg, pin.across_mm));
    // Measure reads from the pin itself.
    h.state_mut().visual.select(Tool::Measure);
    h.run_steps(2);
    click_world(&mut h, pane, pin.world, egui::vec2(3.0, 0.0), Modifiers::NONE);
    assert_eq!(h.state().visual.measurement.picks.first().map(|p| (p.at(), p.label().to_owned())), Some((pin.world, "Pin 1".to_owned())));
    // Clear pins takes it away.
    h.state_mut().visual.select(Tool::Select);
    let band = screen(&h, pane, [crest * 125f64.to_radians().cos(), crest * 125f64.to_radians().sin(), 0.0]);
    click_at(&mut h, band, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Clear pins").click();
    h.run_steps(2);
    assert!(h.state().pins().is_empty());
}

/// The ring frame's UI-thread cost, the kept band, a claw head's ghost read and a landing snap; run `--ignored --nocapture`.
#[test]
#[ignore]
fn the_ring_frame_the_kept_band_and_a_claw_heads_ghost_measured() {
    use ringdesign_core::castability::ghost::GhostJudge;
    use ringdesign_core::{AlphaLibrary, BuildParams, cad, mesh};
    use ringdesign_workbench::command::BandSurface;
    use std::time::Instant;
    let lib = AlphaLibrary::builtin();
    let gem = builders::stone_preset("round-6.5").unwrap().gem();
    let mut design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    doc.append(builders::stone_feature(2, gem, Placement::ring(90.0, 0.0))).unwrap();
    doc.append(builders::feature_on(3, "Four-claw head", builders::CLAW, 2, serde_json::json!({ "prongs": 4 }))).unwrap();
    design.cad = Some(doc);
    let best = |n: usize, f: &mut dyn FnMut()| {
        (0..n)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .fold(f64::MAX, f64::min)
    };
    for (name, params) in [("preview 384x144", BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }), ("export 1024x320", BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })] {
        let built = mesh::build(&design, &lib, params);
        let band = built.band.clone().unwrap();
        let bytes = band.vertices.len() * 12 + band.normals.len() * 12 + band.faces.len() * 12;
        // What the UI thread paid on first choosing a part: the bare band swept again and its tree.
        let mut bare = ringdesign_core::setting::without_solids(&design);
        bare.cad = None;
        bare.graph = None;
        let sweep_ms = best(3, &mut || drop(mesh::try_build(&bare, &lib, params).unwrap()));
        let tree_ms = best(3, &mut || drop(BandSurface::shared(band.clone())));
        let epoch_ms = best(3, &mut || {
            std::hint::black_box(cad::surface_epoch(&band));
        });
        let head = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap().mesh.clone();
        let judge = GhostJudge::new(&design, Some(&band), 0.0);
        let judge_ms = best(3, &mut || drop(GhostJudge::new(&design, Some(&band), 0.0)));
        let frames = 200;
        let t = Instant::now();
        for i in 0..frames {
            let dz = f64::from(i) * 0.01;
            std::hint::black_box(judge.read(&head, &[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, dz]], false));
        }
        let read_ms = t.elapsed().as_secs_f64() * 1e3 / f64::from(frames);
        let classes = judge.read(&head, &[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]], false).classes;
        let stage_ms = best(5, &mut || drop(crate::viewport::GpuMeshRenderer::stage_part_classes(&head, &classes)));
        eprintln!(
            "{name}: band {} faces kept as {:.2} MiB; the UI thread used to sweep it again in {sweep_ms:.1} ms + tree {tree_ms:.1} ms on first choosing a part, now 0 (the worker builds the tree, {tree_ms:.1} ms, epoch {epoch_ms:.2} ms, per band change); claw head {} faces: judge {judge_ms:.2} ms once a build, read {read_ms:.3} ms a moved frame, restage {stage_ms:.3} ms when its classes change",
            band.faces.len(),
            bytes as f64 / (1024.0 * 1024.0),
            head.faces.len()
        );
    }
    // A landing sample: the ring's features gathered once a build, then one snap of the landing a sample.
    use ringdesign_workbench::command::{Dofs, RingFeatures, RingPoint, Scene, SnapGeometry, Snapper};
    let mut squared = design.clone();
    squared.profile.width_mm = 7.0;
    squared.profile.thickness_mm = 3.4;
    squared.profile.apply_style(ringdesign_core::ProfileStyle::Flat);
    squared.profile.flatten_sides();
    for (name, d) in [("Court band", &design), ("squared band with side faces", &squared)] {
        let params = BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() };
        let built = mesh::build(d, &lib, params);
        let gather = || RingFeatures::of(d, 0.0).with_stones(d).with_parts(built.parts.evaluated.as_ref(), Some(3));
        let features_ms = best(3, &mut || drop(gather()));
        let features = gather();
        let surface = BandSurface::shared(built.band.clone().unwrap());
        let world_of = |p: RingPoint| surface.world(p);
        let view = ringdesign_core::interaction::pick::ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], px_per_mm: 25.0 };
        let scene = Scene { view, aperture_px: 8.0, geometry: SnapGeometry::default(), features: &features, design: Some(d), world_of: &world_of };
        let snapper = Snapper { grid: Some(crate::command::GRID), crest: true, ..Snapper::default() };
        let n = 1000;
        let (mut total, mut worst) = (0.0f64, 0.0f64);
        for i in 0..n {
            let landing = RingPoint { theta_deg: 250.0 + f64::from(i % 400) * 0.1, across_mm: 0.1, height_mm: 0.25 };
            let t = Instant::now();
            let at = surface.world(landing).unwrap();
            std::hint::black_box(snapper.snap_ring(at, landing, Dofs::ALL, &scene));
            let us = t.elapsed().as_secs_f64() * 1e6;
            total += us;
            worst = worst.max(us);
        }
        let each = |f: &mut dyn FnMut(f64)| {
            let t = Instant::now();
            for i in 0..n {
                f(250.0 + f64::from(i % 400) * 0.1);
            }
            t.elapsed().as_secs_f64() * 1e6 / f64::from(n)
        };
        let seat_us = each(&mut |theta| {
            std::hint::black_box(surface.world(RingPoint { theta_deg: theta, across_mm: 0.1, height_mm: 0.25 }));
        });
        let side_us = each(&mut |theta| {
            std::hint::black_box(features.side_points(d, theta));
        });
        eprintln!(
            "{name}: {} side lines; features {features_ms:.2} ms once a build; a landing snap {:.1} µs mean, {worst:.1} µs worst, of which a seat on the band {seat_us:.1} µs (two a sample) and the side lines' section {side_us:.1} µs",
            features.side_lines.len(),
            total / f64::from(n)
        );
    }
}
