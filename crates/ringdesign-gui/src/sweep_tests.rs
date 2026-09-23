//! The sketch solver at scale, one-region profiles, builder creases and the CAD pane's grips.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
use ringdesign_core::cad::{self, Attach, Component, Document, Feature, Operation, Placement, Profile};
use ringdesign_core::sketch::{Constraint, Geometry, Measure, Sketch};
use std::time::{Duration, Instant};

/// The box's id in the Court band + Box design.
const BOX: u64 = 2;

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

/// The Court band carrying `doc`, built and settled in the history.
fn court_carrying(h: &mut Harness<'static, RingDesignerApp>, doc: Document) -> usize {
    let pane = ring_view(h);
    {
        let app = h.state_mut();
        let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        d.cad = Some(doc);
        d.bake_all(app.library_mut());
        app.design = d;
        app.history.reset(&app.design.clone());
        app.rebuild_now();
    }
    wait_for_build(h);
    pane
}

/// The Court band with an 8 x 6 x 2 box joined at the top of the ring, seen from above and to one side.
fn court_with_a_box(h: &mut Harness<'static, RingDesignerApp>) -> usize {
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    let placement = Placement::ring(90.0, 0.0);
    doc.append(Feature { id: BOX, name: "Box".into(), enabled: true, operation: Operation::Box { size: [8.0, 6.0, 2.0] }, component: Component { attach: Attach::Join, placement, ..Component::default() } }).unwrap();
    let pane = court_carrying(h, doc);
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.yaw = std::f32::consts::FRAC_PI_2 - 0.5;
        cam.pitch = 0.4;
        cam.roll = 0.0;
        cam.fit(bounds);
    }
    h.run_steps(3);
    pane
}

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> egui::Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// A world point on screen through the pane's camera.
fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f64; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world.map(|v| v as f32))
}

/// A point of the live sketch's plane on screen.
fn on_plane(h: &Harness<'static, RingDesignerApp>, pane: usize, uv: [f64; 2]) -> Pos2 {
    screen(h, pane, h.state().sketch.world(uv).expect("a live sketch with its plane read"))
}

fn press(h: &mut Harness<'static, RingDesignerApp>, key: Key) {
    h.key_press(key);
    h.run_steps(2);
}

fn text(h: &mut Harness<'static, RingDesignerApp>, t: &str) {
    h.event(Event::Text(t.into()));
    h.run_steps(2);
}

/// Hovers and clicks at a point, as a hand would.
fn tap(h: &mut Harness<'static, RingDesignerApp>, at: Pos2) {
    h.hover_at(at);
    h.run_steps(2);
    click_at(h, at, PointerButton::Primary, Modifiers::NONE);
}

/// Steps until the pane's camera has arrived where it was turning to.
fn wait_for_turn(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    let start = Instant::now();
    while h.state().panes[pane].turn.is_some() {
        h.run_steps(1);
        std::thread::sleep(Duration::from_millis(25));
        assert!(start.elapsed() < Duration::from_secs(5), "the camera never arrived");
    }
    h.run_steps(2);
}

/// Right-clicks the box's top and sketches on it; returns the new sketch feature.
fn sketch_on_the_top(h: &mut Harness<'static, RingDesignerApp>, pane: usize) -> u64 {
    let c = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == BOX).unwrap().clone();
    let points: Vec<[f64; 3]> = c.mesh.vertices.iter().map(|v| [f64::from(v.0), f64::from(v.1), f64::from(v.2)]).collect();
    let reach = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let top: Vec<&[f64; 3]> = points.iter().filter(|p| (p[1] - reach).abs() < 0.5).collect();
    let n = top.len() as f64;
    let at = screen(h, pane, std::array::from_fn(|k| top.iter().map(|p| p[k]).sum::<f64>() / n));
    h.hover_at(at);
    h.run_steps(2);
    click_at(h, at, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Sketch on this face").click();
    h.run_steps(2);
    assert!(h.state().sketch.is_live(), "{}", h.state().status);
    wait_for_turn(h, pane);
    h.state().sketch.feature().unwrap()
}

/// Right-clicks the plane point `uv` and chooses `item` from the menu that opens.
fn region_menu(h: &mut Harness<'static, RingDesignerApp>, pane: usize, uv: [f64; 2], item: &str) {
    let at = on_plane(h, pane, uv);
    h.hover_at(at);
    h.run_steps(2);
    click_at(h, at, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label(item).click();
    h.run_steps(2);
}

/// Two held rectangles side by side on `plane`: 1.5 x 2 on the left, 2 x 2 on the right; the right one's lines.
fn two_rectangles(plane: ringdesign_core::sketch::Workplane, left: [f64; 2], right: [f64; 2]) -> (Sketch, [u64; 4]) {
    let mut s = Sketch { plane, ..Sketch::default() };
    s.add_rectangle(left, [left[0] + 1.5, left[1] + 2.0], false).unwrap();
    let lines = s.add_rectangle(right, [right[0] + 2.0, right[1] + 2.0], false).unwrap();
    (s, lines)
}

/// The built part `id`'s own volume.
fn part_volume(h: &Harness<'static, RingDesignerApp>, id: u64) -> f64 {
    let built = h.state().build.clone().unwrap();
    built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).expect("the part").mesh.volume_mm3()
}

/// Types a height, confirms, and returns the feature the sketch made once the ring has rebuilt.
fn make_it(h: &mut Harness<'static, RingDesignerApp>, value: &str) -> Feature {
    text(h, value);
    press(h, Key::Enter);
    assert!(!h.state().sketch.is_live(), "{}", h.state().status);
    let made = h.state().design.cad.as_ref().unwrap().features.last().cloned().unwrap();
    h.state_mut().rebuild_now();
    wait_for_build(h);
    made
}

#[test]
fn one_region_of_a_sketch_extrudes_alone_beside_the_whole_sketch() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    let id = sketch_on_the_top(&mut h, pane);
    let (two, right) = two_rectangles(h.state().sketch.working().unwrap().plane.clone(), [-3.0, -1.0], [0.5, -1.0]);
    h.state_mut().sketch.set_working(two);
    h.run_steps(2);
    // The whole-sketch item sweeps both regions: 3 + 4 mm² at 1.5.
    let entries = h.state().history.present();
    region_menu(&mut h, pane, [-2.0, 0.0], "Extrude");
    let every = make_it(&mut h, "1.5");
    assert_eq!(h.state().history.present(), entries + 1, "the strokes and the extrusion land as one edit");
    assert!(matches!(every.operation, Operation::Extrude { sketch: Profile::Feature { feature }, .. } if feature == id), "{:?}", every.operation);
    let v = part_volume(&h, every.id);
    assert!((v - 10.5).abs() < 1e-3, "{v}");
    // The one beside it sweeps only the region clicked, named by its rim and the point.
    assert!(crate::sketch_mode::start_on_feature(h.state_mut(), pane, id));
    wait_for_turn(&mut h, pane);
    region_menu(&mut h, pane, [1.5, 0.0], "Extrude this region");
    assert!(h.state().status.contains("Extrude this region"), "{}", h.state().status);
    let one = make_it(&mut h, "1.5");
    let Operation::Extrude { sketch: Profile::Region { feature, region }, height_mm, .. } = &one.operation else { panic!("{:?}", one.operation) };
    assert_eq!((*feature, *height_mm), (id, 1.5));
    assert!(right.contains(&region.entity) && (region.at[0] - 1.5).abs() < 0.05 && region.at[1].abs() < 0.05, "{region:?}");
    // The right rectangle alone: 2 x 2 x 1.5.
    let v = part_volume(&h, one.id);
    assert!((v - 6.0).abs() < 1e-3, "{v}");
}

#[test]
fn one_region_revolves_about_the_line_clicked_for_its_axis() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    crate::sketch_mode::start_on_plane(h.state_mut(), pane, 45.0, 0.0);
    wait_for_turn(&mut h, pane);
    h.get_by_label("Section plane").click();
    h.run_steps(2);
    wait_for_turn(&mut h, pane);
    let id = h.state().sketch.feature().unwrap();
    let r = h.state().sketch.working().unwrap().points.first().map_or(10.5, |p| p.xy[0]).max(10.0);
    // Two squares on the section, one over the other: the upper alone turns half round its own inner side.
    let (two, upper) = two_rectangles(h.state().sketch.working().unwrap().plane.clone(), [r, -3.5], [r, 0.5]);
    h.state_mut().sketch.set_working(two);
    h.run_steps(2);
    region_menu(&mut h, pane, [r + 1.0, 1.5], "Revolve this region…");
    let axis = on_plane(&h, pane, [r, 1.5]);
    tap(&mut h, axis);
    let turned = make_it(&mut h, "180");
    let Operation::Revolve { sketch: Profile::Region { feature, region }, degrees, .. } = &turned.operation else { panic!("{:?}", turned.operation) };
    assert_eq!((*feature, *degrees), (id, 180.0));
    assert!(upper.contains(&region.entity), "{region:?}");
    // Half a cylinder 2 mm round and 2 mm long: the lower square is not in it.
    let v = part_volume(&h, turned.id);
    let expected = std::f64::consts::PI * 4.0;
    assert!((v / expected - 1.0).abs() < 0.01, "{v} against {expected}");
}

#[test]
fn the_cad_panes_profile_picker_takes_one_region_of_several() {
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut two = Sketch::default();
        two.add_rectangle([0.0, 0.0], [2.0, 2.0], false).unwrap();
        two.add_rectangle([4.0, 0.0], [7.0, 2.0], false).unwrap();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Plan".into(), enabled: true, operation: Operation::Sketch { sketch: two }, component: Component::default() }).unwrap();
        let post = Operation::Extrude { sketch: Profile::Feature { feature: 1 }, height_mm: 1.5, draft_deg: 0.0 };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: post, component: Component::default() }).unwrap();
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
        app.switch_desktop(crate::dock::Desktop::Cad);
    }
    h.run_steps(4);
    crate::panels::cad::ask(h.state_mut(), crate::panels::cad::CadRequest::Select { feature: 2 });
    h.run_steps(4);
    h.get_by_value("Sketch #1").click();
    h.run_steps(2);
    // The whole sketch, then each of its two regions with its area.
    assert!(h.query_by_label("#1 Plan").is_some());
    assert!(h.query_by_label("#1 Plan · region 1 of 2 (4.00 mm²)").is_some());
    h.get_by_label("#1 Plan · region 2 of 2 (6.00 mm²)").click();
    h.run_steps(3);
    let chosen = h.state().cad.candidate_operation(2);
    let Some(Operation::Extrude { sketch: Profile::Region { feature: 1, region }, .. }) = &chosen else { panic!("{chosen:?}") };
    assert!((4.0..=7.0).contains(&region.at[0]) && (0.0..=2.0).contains(&region.at[1]), "{region:?}");
    assert!(h.query_by_value("Sketch #1 · region 2 of 2").is_some(), "the picker names the region it took");
    wait_until_previewed(&mut h);
    press_ctrl_enter(&mut h);
    let d = h.state().design.clone();
    let e = cad::evaluate(&d, &h.state().lib, ringdesign_core::BuildParams::default()).unwrap();
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let post = e.components.iter().find(|c| c.id == 2).unwrap();
    assert_eq!(post.body.roots.len(), 1);
    assert!((post.mesh.volume_mm3() - 9.0).abs() < 1e-6, "{}", post.mesh.volume_mm3());
}

#[test]
fn a_region_profile_lands_the_same_through_the_document_and_the_graph() {
    use ringdesign_core::cad::edit::CadEdit;
    use ringdesign_core::sketch::RegionRef;
    use ringdesign_graph::nodes::cad as graph_cad;
    let (two, right) = two_rectangles(ringdesign_core::sketch::Workplane::default(), [0.0, 0.0], [4.0, 0.0]);
    let region = RegionRef::of(two.profile_regions().unwrap().iter().find(|r| r.entities.contains(&right[0])).unwrap()).unwrap();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Plan".into(), enabled: true, operation: Operation::Sketch { sketch: two }, component: Component::default() }).unwrap();
    let post = Operation::Extrude { sketch: Profile::Feature { feature: 1 }, height_mm: 1.5, draft_deg: 0.0 };
    doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: post, component: Component::default() }).unwrap();
    let d = ringdesign_core::RingDesign { cad: Some(doc), ..ringdesign_core::RingDesign::default() };
    let edit = CadEdit::Operation { id: 2, operation: Operation::Extrude { sketch: Profile::Region { feature: 1, region }, height_mm: 1.5, draft_deg: 0.0 } };
    let mut plain = d.clone();
    plain.apply_cad_edit(&edit).unwrap();
    let mut g = graph_cad::from_document(&d).unwrap();
    graph_cad::apply_edit(&mut g, &edit).unwrap();
    let bytes = |doc: &Document| serde_json::to_string(doc).unwrap();
    assert_eq!(bytes(&graph_cad::document(&g).unwrap()), bytes(plain.cad.as_ref().unwrap()));
    // Evaluated, the graph carries the pick through and builds the one region.
    let (reg, lib) = (ringdesign_graph::registry::Registry::builtin(), ringdesign_core::AlphaLibrary::builtin());
    let out = ringdesign_graph::eval::evaluate_design(&mut ringdesign_graph::eval::Evaluator::new(), &g, &reg, &lib, 0).unwrap();
    assert_eq!(bytes(out.design.cad.as_ref().unwrap()), bytes(plain.cad.as_ref().unwrap()));
    let e = cad::evaluate(&out.design, &lib, ringdesign_core::BuildParams::default()).unwrap();
    let post = e.components.iter().find(|c| c.id == 2).unwrap();
    assert!((post.mesh.volume_mm3() - 6.0).abs() < 1e-6, "{}", post.mesh.volume_mm3());
}

/// A primary drag through `points`, the button held throughout.
fn drag_through(h: &mut Harness<'static, RingDesignerApp>, points: &[Pos2]) {
    let first = points[0];
    h.event(Event::PointerMoved(first));
    h.event(Event::PointerButton { pos: first, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for p in &points[1..] {
        h.event(Event::PointerMoved(*p));
        h.run_steps(1);
    }
    let last = *points.last().unwrap();
    h.event(Event::PointerButton { pos: last, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

#[test]
fn the_cad_panes_first_grip_drag_lands_on_the_pointer() {
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
    let (canvas, scale) = h.state().cad.canvas_scale();
    let (from, start) = (grip(&h), radius(&h));
    assert!(h.query_by_label_contains("Parameters changed").is_none(), "the candidate is previewed");
    // The very first drag turns the candidate into a draft; its note lies over the canvas and nothing moves.
    let drag = egui::vec2(40.0, 12.0);
    let steps: Vec<Pos2> = (0..=24).map(|k| from + drag * (k as f32 / 24.0)).collect();
    drag_through(&mut h, &steps);
    assert!(h.query_by_label_contains("Parameters changed").is_some(), "the draft says so");
    assert_eq!(h.state().cad.canvas_scale(), (canvas, scale), "the canvas keeps its place and its {scale:.2} px/mm");
    let (to, after) = (grip(&h), radius(&h));
    let moved = to - from;
    assert!(moved.length() > 5.0 && after > start, "{moved:?} {start} -> {after}");
    // The grip lands where the pointer's move projects onto its own screen axis.
    let along = drag.dot(moved.normalized());
    assert!((along - moved.length()).abs() < 1.0, "the pointer moved {along:.2} px along the axis, the grip {:.2} px", moved.length());
}

#[test]
fn a_hover_on_a_claws_side_names_the_claw_not_a_rails_facet() {
    let mut h = harness();
    let pane = court_carrying(&mut h, cad::examples::design("claw-solitaire").unwrap().cad.unwrap());
    let built = h.state().build.clone().unwrap();
    let head = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap().clone();
    let made = head.made.clone().unwrap();
    let named = &made.named;
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let patch = |name: &str| named.names.iter().position(|n| n == name).unwrap() as u32;
    let (claw, rail) = (patch("Claw 1"), patch("Gallery rail"));
    let faces = |p: u32| (0..named.solid.f.len()).filter(move |f| named.patch[*f] == p);
    let centroid = |f: usize| -> [f64; 3] { std::array::from_fn(|k| named.solid.f[f].iter().map(|i| named.solid.v[*i as usize][k]).sum::<f64>() / 3.0) };
    let (o, up) = (head.frame.origin, head.frame.z_axis);
    let height = |p: [f64; 3]| dot(std::array::from_fn(|k| p[k] - o[k]), up);
    // Claw 1 seen square on from outside, a quarter millimetre over the gallery rail's top.
    let n = faces(claw).count() as f64;
    let middle: [f64; 3] = std::array::from_fn(|k| faces(claw).map(|f| centroid(f)[k]).sum::<f64>() / n);
    let out: [f64; 3] = {
        let d: [f64; 3] = std::array::from_fn(|k| middle[k] - o[k]);
        let flat: [f64; 3] = std::array::from_fn(|k| d[k] - up[k] * dot(d, up));
        let l = dot(flat, flat).sqrt();
        flat.map(|x| x / l)
    };
    let top = faces(rail).flat_map(|f| named.solid.f[f]).map(|i| height(named.solid.v[i as usize])).fold(f64::MIN, f64::max);
    let facing = |f: &usize| {
        let [a, b, c] = named.solid.f[*f].map(|i| named.solid.v[i as usize]);
        let (e, g) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let m = [e[1] * g[2] - e[2] * g[1], e[2] * g[0] - e[0] * g[2], e[0] * g[1] - e[1] * g[0]];
        dot(m, out) / dot(m, m).sqrt().max(1e-12) > 0.6
    };
    let at = faces(claw).filter(facing).map(centroid).min_by(|a, b| (height(*a) - top - 0.25).abs().total_cmp(&(height(*b) - top - 0.25).abs())).unwrap();
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.yaw = out[1].atan2(out[0]) as f32;
        cam.pitch = out[2].asin() as f32;
        cam.roll = 0.0;
        cam.fit(bounds);
        // Twice the whole ring's framing: about 20 px/mm, the scale the pick scene's pin is measured at.
        cam.zoom = 2.0;
        cam.target = at.map(|v| v as f32);
        cam.pan = [0.0; 2];
    }
    h.run_steps(3);
    let rect = viewport_rect(&h);
    let camera = h.state().panes[pane].camera;
    let (view, _) = ringdesign_workbench::hover::view_scale(rect.center(), &|p| camera.ray(rect, p));
    assert!((15.0..30.0).contains(&view.px_per_mm), "{:.1} px/mm", view.px_per_mm);
    let spot = screen(&h, pane, at);
    for _ in 0..3 {
        h.hover_at(spot);
        h.run_steps(2);
        std::thread::sleep(Duration::from_millis(40));
    }
    let label = viewport_label(&h);
    assert!(label.contains("hovering Claw 1 of Four-claw head"), "{label} at {:.1} px/mm", view.px_per_mm);
}

#[test]
fn two_hundred_held_rectangles_hold_a_dimension_inside_a_frame() {
    use ringdesign_workbench::sketch_tools::{Input, Outcome, Tool, Tools};
    let mut s = Sketch::default();
    for i in 0..200 {
        let (x, y) = ((i % 20) as f64 * 3.0, (i / 20) as f64 * 3.0);
        let p = [[x, y], [x + 2.0, y], [x + 2.0, y + 1.5], [x, y + 1.5]].map(|xy| s.point(xy));
        for k in 0..4 {
            s.entity(Geometry::Line { a: p[k], b: p[(k + 1) % 4] });
        }
        s.constraints.extend([
            Constraint::Horizontal(p[0], p[1]),
            Constraint::Vertical(p[1], p[2]),
            Constraint::Horizontal(p[2], p[3]),
            Constraint::Vertical(p[3], p[0]),
            Constraint::Distance { a: p[0], b: p[1], mm: 2.0 },
            Constraint::Distance { a: p[1], b: p[2], mm: 1.5 },
        ]);
    }
    assert_eq!(s.solve().unwrap().remaining_dof, 400, "eight hundred points, each rectangle free to slide");
    // The Dimension tool on the last rectangle's bottom, as a click and a typed value.
    let before = s.clone();
    let mut t = Tools::default();
    t.set_tool(Tool::Dimension);
    t.feed(&mut s, Input::Pointer { raw: [58.0, 27.0], snapped: None, reach: 0.05 });
    t.feed(&mut s, Input::Click { add: false });
    t.feed(&mut s, Input::Typed { key: "length", value: 2.5 });
    let out = t.feed(&mut s, Input::Confirm);
    assert!(matches!(out, Outcome::Edited(_)), "{out:?}");
    let ms = t.last_solve_ms.unwrap();
    assert!(ms < 16.0, "a dimension over two hundred held rectangles took {ms:.2} ms");
    let bottom = Measure::Length { a: s.points[796].id, b: s.points[797].id };
    assert!((s.measured(&bottom).unwrap() - 2.5).abs() < 1e-6);
    let moved: Vec<usize> = (0..800).filter(|i| s.points[*i].xy != before.points[*i].xy).collect();
    assert!(!moved.is_empty() && moved.iter().all(|i| *i >= 796), "only the dimensioned rectangle moves: {moved:?}");
}
