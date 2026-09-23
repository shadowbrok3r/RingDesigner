//! Sketch mode in the Ring viewport and the CAD pane's canvas tools.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::{Harness, kittest::Queryable};
use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, SurfaceKind};
use ringdesign_core::sketch::{Geometry, Sketch, distance};
use ringdesign_core::{ProfileStyle, RingDesign};
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

/// The Court band with `part` joined at the top of the ring, built and settled in the history.
fn court_with(h: &mut Harness<'static, RingDesignerApp>, part: Operation) -> usize {
    let pane = ring_view(h);
    {
        let app = h.state_mut();
        let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: BOX,
            name: part.label().into(),
            enabled: true,
            operation: part,
            component: Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() },
        })
        .unwrap();
        d.cad = Some(doc);
        d.bake_all(app.library_mut());
        app.design = d;
        app.history.reset(&app.design.clone());
        app.rebuild_now();
    }
    wait_for_build(h);
    // Above the top of the ring and off to one side: the box's top is seen at a slant.
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

fn court_with_a_box(h: &mut Harness<'static, RingDesignerApp>) -> usize {
    court_with(h, Operation::Box { size: [8.0, 6.0, 2.0] })
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

/// The centre of the part's face that looks most along `dir`, on screen.
fn face_centre(h: &Harness<'static, RingDesignerApp>, pane: usize, dir: [f64; 3]) -> Pos2 {
    let c = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == BOX).unwrap();
    let points: Vec<[f64; 3]> = c.mesh.vertices.iter().map(|v| [f64::from(v.0), f64::from(v.1), f64::from(v.2)]).collect();
    let reach = points.iter().map(|p| p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2]).fold(f64::NEG_INFINITY, f64::max);
    let top: Vec<&[f64; 3]> = points.iter().filter(|p| (p[0] * dir[0] + p[1] * dir[1] + p[2] * dir[2] - reach).abs() < 0.5).collect();
    let n = top.len() as f64;
    screen(h, pane, std::array::from_fn(|k| top.iter().map(|p| p[k]).sum::<f64>() / n))
}

fn press(h: &mut Harness<'static, RingDesignerApp>, key: Key) {
    h.key_press(key);
    h.run_steps(2);
}

fn text(h: &mut Harness<'static, RingDesignerApp>, t: &str) {
    h.event(Event::Text(t.into()));
    h.run_steps(2);
}

fn press_undo(h: &mut Harness<'static, RingDesignerApp>) {
    h.event(Event::ModifiersChanged(Modifiers::COMMAND));
    h.event(Event::Key { key: Key::Z, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    h.run_steps(1);
    h.event(Event::Key { key: Key::Z, pressed: false, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

/// Hovers and clicks at a point, as a hand would.
fn tap(h: &mut Harness<'static, RingDesignerApp>, at: Pos2) {
    h.hover_at(at);
    h.run_steps(2);
    click_at(h, at, PointerButton::Primary, Modifiers::NONE);
}

/// A primary drag from `from` to `to` in 24 steps, the button held throughout.
fn drag(h: &mut Harness<'static, RingDesignerApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=24 {
        h.event(Event::PointerMoved(from + (to - from) * (k as f32 / 24.0)));
        h.run_steps(1);
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

/// Taps a point of the live sketch's plane.
fn tap_plane(h: &mut Harness<'static, RingDesignerApp>, pane: usize, uv: [f64; 2]) {
    let at = on_plane(h, pane, uv);
    tap(h, at);
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

/// The angle in degrees between the camera's line of sight back to the eye and `n`.
fn off_axis_deg(h: &Harness<'static, RingDesignerApp>, pane: usize, n: [f64; 3]) -> f64 {
    let cam = h.state().panes[pane].camera;
    let (p, y) = (f64::from(cam.pitch), f64::from(cam.yaw));
    let eye = [p.cos() * y.cos(), p.cos() * y.sin(), p.sin()];
    (eye[0] * n[0] + eye[1] * n[1] + eye[2] * n[2]).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Right-clicks the box's top face and sketches on it; returns the new sketch feature.
fn sketch_on_the_top(h: &mut Harness<'static, RingDesignerApp>, pane: usize) -> u64 {
    let top = face_centre(h, pane, [0.0, 1.0, 0.0]);
    h.hover_at(top);
    h.run_steps(2);
    click_at(h, top, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Sketch on this face").click();
    h.run_steps(2);
    assert!(h.state().sketch.is_live(), "{}", h.state().status);
    wait_for_turn(h, pane);
    h.state().sketch.feature().unwrap()
}

fn working(h: &Harness<'static, RingDesignerApp>) -> Sketch {
    h.state().sketch.working().expect("a live sketch").clone()
}

fn committed(h: &Harness<'static, RingDesignerApp>, id: u64) -> Sketch {
    let f = h.state().design.cad.as_ref().and_then(|d| d.feature(id)).expect("the sketch feature").clone();
    match f.operation {
        Operation::Sketch { sketch } => sketch,
        other => panic!("{other:?}"),
    }
}

fn length(s: &Sketch, id: u64) -> f64 {
    s.curves_of(s.entities.iter().find(|e| e.id == id).unwrap()).unwrap().iter().map(|c| c.length()).sum()
}

fn tool(h: &mut Harness<'static, RingDesignerApp>, name: &str) {
    h.get_by_label(&format!("{name} tool")).click();
    h.run_steps(2);
}

#[test]
fn a_sketch_on_the_boxs_top_is_drawn_dimensioned_filleted_finished_and_extruded_to_stand_on_it() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    let entries = h.state().history.present();
    let slanted = off_axis_deg(&h, pane, [0.0, 1.0, 0.0]);
    let id = sketch_on_the_top(&mut h, pane);
    assert_eq!(h.state().history.present(), entries + 1, "the sketch is added through the funnel as one edit");
    let n = h.state().sketch.normal().unwrap();
    assert!(n[1] > 0.999, "the top face looks out along +y, as the box is seated on the crest: {n:?}");
    assert!(slanted > 30.0 && off_axis_deg(&h, pane, n) < 2.0, "the camera turned {slanted:.1}° to look along the face: {:.3}°", off_axis_deg(&h, pane, n));
    let origin = h.state().sketch.world([0.0; 2]).unwrap();
    // Two clicks draw a 4 × 3 rectangle on the face.
    tool(&mut h, "Rectangle");
    let (a, b) = (on_plane(&h, pane, [-2.0, -1.5]), on_plane(&h, pane, [2.0, 1.5]));
    tap(&mut h, a);
    tap(&mut h, b);
    let s = working(&h);
    assert_eq!(s.entities.len(), 4, "{:?}", h.state().status);
    assert!(s.entities.iter().all(|e| matches!(e.geometry, Geometry::Line { .. })));
    let mut sides: Vec<f64> = s.entities.iter().map(|e| length(&s, e.id)).collect();
    sides.sort_by(f64::total_cmp);
    for (got, want) in sides.iter().zip([3.0, 3.0, 4.0, 4.0]) {
        assert!((got - want).abs() < 0.05, "{sides:?}");
    }
    // Dimension the 4 mm side to 3.
    tool(&mut h, "Dimension");
    tap_plane(&mut h, pane, [0.0, -1.5]);
    text(&mut h, "3");
    press(&mut h, Key::Enter);
    let s = working(&h);
    let bottom = s.entities[0].id;
    assert!((length(&s, bottom) - 3.0).abs() < 1e-3, "{} · {}", length(&s, bottom), h.state().status);
    let solve_ms = h.state().sketch.last_solve_ms().unwrap();
    // Fillet the corner opposite the first at 0.5: a quarter arc, pi/4 long.
    tool(&mut h, "Fillet corner");
    let corner = s.points.iter().map(|p| p.xy).max_by(|p, q| (p[0] + p[1]).total_cmp(&(q[0] + q[1]))).unwrap();
    tap_plane(&mut h, pane, corner);
    text(&mut h, "0.5");
    press(&mut h, Key::Enter);
    let s = working(&h);
    let arc = s.entities.iter().find(|e| matches!(e.geometry, Geometry::Arc { .. })).expect(&h.state().status).id;
    assert!((length(&s, arc) - std::f64::consts::FRAC_PI_4).abs() < 1e-3, "{}", length(&s, arc));
    assert!(committed(&h, id).entities.is_empty(), "nothing reaches the document before Finish");
    // Finish commits the whole sketch as exactly one edit.
    let entries = h.state().history.present();
    h.get_by_label("Finish sketch").click();
    h.run_steps(2);
    assert!(!h.state().sketch.is_live());
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!(committed(&h, id), s);
    // Back in, the region's right-click extrudes it 1 mm.
    assert!(crate::sketch_mode::start_on_feature(h.state_mut(), pane, id));
    wait_for_turn(&mut h, pane);
    let parts = h.state().build.as_ref().unwrap().parts.features.len();
    let inside = on_plane(&h, pane, [0.0, 0.0]);
    h.hover_at(inside);
    h.run_steps(2);
    click_at(&mut h, inside, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Extrude").click();
    h.run_steps(2);
    text(&mut h, "1");
    press(&mut h, Key::Enter);
    assert!(!h.state().sketch.is_live(), "{}", h.state().status);
    let extrude = h.state().design.cad.as_ref().unwrap().features.iter().find(|f| matches!(f.operation, Operation::Extrude { .. })).cloned().expect("an Extrude feature");
    assert!(matches!(extrude.operation, Operation::Extrude { height_mm, .. } if height_mm == 1.0));
    assert_eq!(extrude.component.attach, Attach::Join, "a part beside the procedural shank starts joined");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let built = h.state().build.clone().unwrap();
    assert_eq!(built.parts.features.len(), parts + 1, "{:?}", built.parts.notes);
    let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == extrude.id).expect("the extruded part");
    let heights: Vec<f64> = c.trace.positions.iter().map(|p| (0..3).map(|k| (p[k] - origin[k]) * n[k]).sum()).collect();
    let (low, high) = heights.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    assert!(low.abs() < 1e-3 && (high - 1.0).abs() < 1e-3, "the part's base stands on the box's top: {low} .. {high}");
    // Undo twice: the extrusion, then the finished sketch; the box is the only part again.
    press_undo(&mut h);
    press_undo(&mut h);
    let doc = h.state().design.cad.clone().unwrap();
    let bodies: Vec<u64> = doc.features.iter().filter(|f| !matches!(f.operation, Operation::Sketch { .. })).map(|f| f.id).collect();
    assert_eq!(bodies, [1, BOX], "{:?}", doc.features.iter().map(|f| &f.name).collect::<Vec<_>>());
    assert!(committed(&h, id).entities.is_empty(), "the sketch is as it was added, empty");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    assert_eq!(h.state().build.as_ref().unwrap().parts.features, [BOX], "the box alone");
    eprintln!("dimension solve {solve_ms:.3} ms");
}

#[test]
fn escape_never_loses_committed_work_and_asks_before_dropping_strokes() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    let id = sketch_on_the_top(&mut h, pane);
    tool(&mut h, "Line");
    tap_plane(&mut h, pane, [-2.0, 0.0]);
    tap_plane(&mut h, pane, [2.0, 0.0]);
    assert_eq!(working(&h).entities.len(), 1);
    // Escape ends the line, then puts the tool away, then asks rather than dropping the stroke.
    press(&mut h, Key::Escape);
    assert_eq!(h.state().sketch.tool(), Some(ringdesign_workbench::sketch_tools::Tool::Line));
    press(&mut h, Key::Escape);
    assert_eq!(h.state().sketch.tool(), Some(ringdesign_workbench::sketch_tools::Tool::Select));
    press(&mut h, Key::Escape);
    assert!(h.state().sketch.is_live() && h.state().sketch.asking(), "{}", h.state().status);
    assert!(h.query_by_label("Discard changes").is_some());
    press(&mut h, Key::Escape);
    assert!(h.state().sketch.is_live() && !h.state().sketch.asking(), "Escape at the question keeps drawing");
    assert_eq!(working(&h).entities.len(), 1, "and keeps the stroke");
    h.get_by_label("Finish sketch").click();
    h.run_steps(2);
    assert_eq!(committed(&h, id).entities.len(), 1);
    // A finished sketch left with Escape is left as committed.
    assert!(crate::sketch_mode::start_on_feature(h.state_mut(), pane, id));
    h.run_steps(2);
    press(&mut h, Key::Escape);
    assert!(!h.state().sketch.is_live());
    assert_eq!(committed(&h, id).entities.len(), 1);
    // Discarding drops only what was never finished.
    assert!(crate::sketch_mode::start_on_feature(h.state_mut(), pane, id));
    wait_for_turn(&mut h, pane);
    tool(&mut h, "Line");
    tap_plane(&mut h, pane, [-2.0, 1.0]);
    tap_plane(&mut h, pane, [2.0, 1.0]);
    assert_eq!(working(&h).entities.len(), 2);
    for _ in 0..3 {
        press(&mut h, Key::Escape);
    }
    assert!(h.state().sketch.asking());
    h.get_by_label("Discard changes").click();
    h.run_steps(2);
    assert!(!h.state().sketch.is_live());
    assert_eq!(committed(&h, id).entities.len(), 1, "the committed line stays");
}

#[test]
fn a_chosen_line_deletes_a_dragged_corner_lands_on_the_face_edge_and_both_undo_inside_the_sketch() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    let id = sketch_on_the_top(&mut h, pane);
    let mut rect = Sketch { plane: working(&h).plane, ..Sketch::default() };
    let lines = rect.add_rectangle([-2.0, -1.5], [2.0, 1.5], false).unwrap();
    h.state_mut().sketch.set_working(rect);
    h.run_steps(2);
    tap_plane(&mut h, pane, [0.0, 1.5]);
    press(&mut h, Key::Delete);
    let s = working(&h);
    assert_eq!(s.entities.len(), 3, "{}", h.state().status);
    assert!(!s.entities.iter().any(|e| e.id == lines[2]), "the top line went");
    let entries = h.state().history.present();
    press_undo(&mut h);
    assert_eq!(working(&h).entities.len(), 4, "Ctrl+Z inside a sketch takes back the sketch's own edit");
    assert_eq!(h.state().history.present(), entries, "and not the document's");
    assert!(committed(&h, id).entities.is_empty());
    // A corner dragged to 0.03 mm short of the face's 4 mm edge lands on it, and the rectangle follows square.
    let (from, to) = (on_plane(&h, pane, [2.0, 1.5]), on_plane(&h, pane, [3.97, 0.8]));
    drag(&mut h, from, to);
    let corners: Vec<[f64; 2]> = working(&h).points.iter().map(|p| p.xy).collect();
    for (got, want) in corners.iter().zip([[-2.0, -1.5], [4.0, -1.5], [4.0, 0.8], [-2.0, 0.8]]) {
        assert!(distance(*got, want) < 1e-4, "{corners:?} · {}", h.state().status);
    }
    press_undo(&mut h);
    assert_eq!(working(&h).points.iter().map(|p| p.xy).collect::<Vec<_>>(), [[-2.0, -1.5], [2.0, -1.5], [2.0, 1.5], [-2.0, 1.5]], "one Ctrl+Z takes the whole drag back");
    assert_eq!(h.state().history.present(), entries);
}

#[test]
fn a_curved_face_refuses_in_the_cores_words_and_a_plane_on_the_band_is_square_to_it() {
    let mut h = harness();
    let pane = court_with(&mut h, Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 });
    let side = {
        let e = h.state().build.as_ref().unwrap().parts.evaluated.clone().unwrap();
        e.components.iter().find(|c| c.id == BOX).unwrap().trace.face_kind.iter().position(|k| *k == SurfaceKind::Cylinder).unwrap()
    };
    let entries = h.state().history.present();
    crate::sketch_mode::start_on_face(h.state_mut(), pane, BOX, side as u32);
    assert!(!h.state().sketch.is_live());
    assert!(h.state().status.contains("sketch planes need a planar face: cylinder"), "{}", h.state().status);
    assert_eq!(h.state().history.present(), entries, "nothing is added");
    // A plane through a point of the band lies square to it, x round the ring and y along the finger.
    crate::sketch_mode::start_on_plane(h.state_mut(), pane, 45.0, 0.0);
    assert!(h.state().sketch.is_live(), "{}", h.state().status);
    wait_for_turn(&mut h, pane);
    let n = h.state().sketch.normal().unwrap();
    let radial = [45f64.to_radians().cos(), 45f64.to_radians().sin(), 0.0];
    assert!((0..3).map(|k| n[k] * radial[k]).sum::<f64>() > 0.99, "{n:?}");
    assert!(off_axis_deg(&h, pane, n) < 2.0);
    let (o, x) = (h.state().sketch.world([0.0; 2]).unwrap(), h.state().sketch.world([1.0, 0.0]).unwrap());
    assert!((x[2] - o[2]).abs() < 1e-9 && ((x[0] - o[0]) * radial[0] + (x[1] - o[1]) * radial[1]).abs() < 1e-9, "x runs round the ring");
    // The section through the finger's axis there: x out from the axis, y along it.
    h.get_by_label("Section plane").click();
    h.run_steps(2);
    assert!(matches!(h.state().sketch.kind(), Some(crate::sketch_mode::PlaneKind::Section { .. })));
    let s = working(&h);
    assert!(distance([s.plane.x[0], s.plane.x[1]], [radial[0], radial[1]]) < 1e-12 && s.plane.y == [0.0, 0.0, 1.0] && s.plane.origin == [0.0; 3]);
    wait_for_turn(&mut h, pane);
    assert!(off_axis_deg(&h, pane, h.state().sketch.normal().unwrap()) < 2.0);
    h.get_by_label("Finish sketch").click();
    h.run_steps(2);
    assert!(!h.state().sketch.is_live(), "{}", h.state().status);
    let doc = h.state().design.cad.clone().unwrap();
    let sketch = doc.features.iter().find_map(|f| match &f.operation {
        Operation::Sketch { sketch } => Some(sketch.clone()),
        _ => None,
    });
    assert_eq!(sketch.map(|s| s.plane), Some(s.plane), "the section plane is what the document keeps");
}

/// The face of `part` whose plane stands furthest from the ring's centre along its own normal, and that normal.
fn outermost_face(h: &Harness<'static, RingDesignerApp>, part: u64) -> (u32, [f64; 3]) {
    let built = h.state().build.clone().unwrap();
    let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == part).expect("the part").clone();
    let mut faces: std::collections::BTreeMap<u32, ([f64; 3], [f64; 3], f64)> = Default::default();
    for (t, f) in c.mesh.faces.iter().enumerate() {
        let Some(ordinal) = c.trace.face_of(t) else { continue };
        let [a, b, q] = f.map(|i| c.trace.positions[i as usize]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [q[0] - a[0], q[1] - a[1], q[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let area = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let e = faces.entry(ordinal).or_insert(([0.0; 3], [0.0; 3], 0.0));
        for k in 0..3 {
            e.0[k] += n[k];
            e.1[k] += (a[k] + b[k] + q[k]) / 3.0 * area;
        }
        e.2 += area;
    }
    let reach = |(n, c, area): &([f64; 3], [f64; 3], f64)| {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
        (0..3).map(|k| n[k] / l * c[k] / area.max(1e-12)).sum::<f64>()
    };
    let (ordinal, at) = faces.iter().max_by(|x, y| reach(x.1).total_cmp(&reach(y.1))).unwrap();
    let l = (at.0[0] * at.0[0] + at.0[1] * at.0[1] + at.0[2] * at.0[2]).sqrt();
    (*ordinal, at.0.map(|v| v / l))
}

#[test]
fn a_face_of_a_block_on_the_bands_flank_anchors_a_sketch_the_build_lays_on_it() {
    let mut h = harness();
    let pane = ring_view(&mut h);
    {
        let app = h.state_mut();
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::HalfRound);
        d.profile.width_mm = 6.0;
        d.profile.thickness_mm = 3.0;
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        // 2.2 mm off the crest line the half-round's surface leans 34 degrees off the radial.
        let seat = Placement::Ring { theta_deg: 90.0, across_mm: 2.2, height_mm: 0.0, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let block = Component { attach: Attach::Join, placement: seat, ..Component::default() };
        doc.append(Feature { id: BOX, name: "Block".into(), enabled: true, operation: Operation::Box { size: [2.0, 2.0, 1.0] }, component: block }).unwrap();
        d.cad = Some(doc);
        app.design = d;
        app.history.reset(&app.design.clone());
        app.rebuild_now();
    }
    wait_for_build(&mut h);
    let (top, n) = outermost_face(&h, BOX);
    crate::sketch_mode::start_on_face(h.state_mut(), pane, BOX, top);
    assert!(h.state().sketch.is_live(), "{}", h.state().status);
    assert!(h.state().sketch.normal().is_some_and(|m| (0..3).map(|k| m[k] * n[k]).sum::<f64>() > 1.0 - 1e-6), "the plane is the top's");
    let id = h.state().sketch.feature().unwrap();
    let mut square = Sketch { plane: working(&h).plane, ..Sketch::default() };
    square.add_rectangle([-0.5, -0.5], [0.5, 0.5], false).unwrap();
    h.state_mut().sketch.set_working(square);
    h.run_steps(2);
    h.get_by_label("Finish sketch").click();
    h.run_steps(2);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    // The build reads the face by the signature sketch mode wrote: no failure, and not found again under another number.
    let built = h.state().build.clone().unwrap();
    let e = built.parts.evaluated.as_ref().unwrap();
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    assert!(e.features.iter().find(|r| r.id == id).is_some_and(|r| r.notes.is_empty()), "{:?}", e.features);
}

#[test]
fn the_cad_canvas_trims_and_offsets_through_the_shared_tools() {
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut s = Sketch::default();
        s.add_rectangle([0.0, 0.0], [4.0, 2.0], false).unwrap();
        let p = [[2.0, -1.0], [2.0, 3.0]].map(|xy| s.point(xy));
        s.entity(Geometry::Line { a: p[0], b: p[1] });
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch: s }, component: Component::default() }).unwrap();
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
        app.switch_desktop(crate::dock::Desktop::Cad);
        app.cad.open_sketch(1);
    }
    h.run_steps(4);
    // Finds the canvas again for every tap.
    let at = |h: &Harness<'static, RingDesignerApp>, xy: [f64; 2]| h.state().cad.canvas_point(h.get_by_label("Sketch canvas").rect(), xy);
    let choose = |h: &mut Harness<'static, RingDesignerApp>, name: &str| {
        h.query_all_by_label_contains("Tool: ").next().expect("the canvas's tool menu").click();
        h.run_steps(2);
        h.get_by_label(name).click();
        h.run_steps(2);
    };
    choose(&mut h, "Trim");
    let above = at(&h, [2.0, 2.5]);
    tap(&mut h, above);
    let s = h.state().cad.candidate_sketch(1).expect("the canvas edited the candidate");
    let upright = s.entities.iter().find(|e| matches!(e.geometry, Geometry::Line { a, b } if s.at(a).unwrap()[0] == 2.0 && s.at(b).unwrap()[0] == 2.0)).expect("the upright line");
    assert!((length(&s, upright.id) - 3.0).abs() < 1e-9, "trimmed back to the rectangle's top: {}", length(&s, upright.id));
    choose(&mut h, "Offset");
    let bottom = at(&h, [1.0, 0.0]);
    tap(&mut h, bottom);
    assert_eq!(h.state().cad.tool_words().0, "Offset: move out or in, or type the distance; click or Enter makes it");
    text(&mut h, "0.5");
    press(&mut h, Key::Enter);
    let s = h.state().cad.candidate_sketch(1).unwrap();
    assert_eq!(s.entities.len(), 9, "{:?}", h.state().cad.tool_words());
    assert!(s.points.iter().any(|p| distance(p.xy, [-0.5, -0.5]) < 1e-9), "the loop grew half a millimetre all round");
    // The pane's sketch goes to the Ring viewport on its own plane.
    assert!(crate::sketch_mode::start_in_ring(h.state_mut(), 1));
    h.run_steps(3);
    let app = h.state();
    assert!(app.sketch.is_live() && app.sketch.kind() == Some(crate::sketch_mode::PlaneKind::Own));
    assert_eq!(app.panes[app.active_pane].kind, crate::pane::PaneKind::Solid);
    assert!(app.sketch.normal().is_some_and(|n| n[2] > 0.999), "the head-plan plane faces up the finger");
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

/// Heights of a built part's points over the plane through `origin` facing `n`.
fn heights(h: &Harness<'static, RingDesignerApp>, part: u64, origin: [f64; 3], n: [f64; 3]) -> (f64, f64) {
    let built = h.state().build.clone().unwrap();
    let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == part).expect("the part").clone();
    c.trace.positions.iter().map(|p| (0..3).map(|k| (p[k] - origin[k]) * n[k]).sum::<f64>()).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

#[test]
fn unfinished_strokes_extrude_as_one_edit_and_the_part_rises_with_the_face_it_stands_on() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    let id = sketch_on_the_top(&mut h, pane);
    let (origin, n) = (h.state().sketch.world([0.0; 2]).unwrap(), h.state().sketch.normal().unwrap());
    let mut rect = Sketch { plane: working(&h).plane, ..Sketch::default() };
    rect.add_rectangle([-2.0, -1.5], [2.0, 1.5], false).unwrap();
    h.state_mut().sketch.set_working(rect.clone());
    h.run_steps(2);
    let entries = h.state().history.present();
    region_menu(&mut h, pane, [0.5, 0.5], "Extrude");
    text(&mut h, "1.5");
    press(&mut h, Key::Enter);
    assert!(!h.state().sketch.is_live(), "{}", h.state().status);
    assert_eq!(h.state().history.present(), entries + 1, "the strokes and the extrusion land as one edit");
    assert_eq!(committed(&h, id), rect);
    let extrude = h.state().design.cad.as_ref().unwrap().features.last().cloned().unwrap();
    assert!(matches!(extrude.operation, Operation::Extrude { height_mm, draft_deg, .. } if height_mm == 1.5 && draft_deg == 0.0), "{:?}", extrude.operation);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let (low, high) = heights(&h, extrude.id, origin, n);
    assert!(low.abs() < 1e-3 && (high - 1.5).abs() < 1e-3, "{low} .. {high}");
    // The box grows a millimetre about its centre: its top, and the part on it, rise half of one.
    crate::cad_edit::apply(h.state_mut(), &[ringdesign_core::cad::edit::CadEdit::Operation { id: BOX, operation: Operation::Box { size: [8.0, 6.0, 3.0] } }]).unwrap();
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let (low, high) = heights(&h, extrude.id, origin, n);
    assert!((low - 0.5).abs() < 1e-3 && (high - 2.0).abs() < 1e-3, "{low} .. {high}");
}

#[test]
fn a_region_revolves_about_the_line_clicked_for_its_axis() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    crate::sketch_mode::start_on_plane(h.state_mut(), pane, 45.0, 0.0);
    wait_for_turn(&mut h, pane);
    h.get_by_label("Section plane").click();
    h.run_steps(2);
    wait_for_turn(&mut h, pane);
    let id = h.state().sketch.feature().unwrap();
    let mut rect = Sketch { plane: working(&h).plane, ..Sketch::default() };
    let r = working(&h).points.first().map_or(10.5, |p| p.xy[0]).max(10.0);
    rect.add_rectangle([r, -0.5], [r + 1.0, 0.5], false).unwrap();
    h.state_mut().sketch.set_working(rect);
    h.run_steps(2);
    region_menu(&mut h, pane, [r + 0.5, 0.0], "Revolve…");
    tap_plane(&mut h, pane, [r, 0.1]);
    text(&mut h, "180");
    press(&mut h, Key::Enter);
    assert!(!h.state().sketch.is_live(), "{}", h.state().status);
    let revolve = h.state().design.cad.as_ref().unwrap().features.last().cloned().unwrap();
    let Operation::Revolve { sketch, pivot, axis, degrees } = &revolve.operation else { panic!("{:?}", revolve.operation) };
    assert_eq!((sketch.feature(), *degrees), (Some(id), 180.0));
    assert!(axis[0].abs() < 1e-9 && axis[1].abs() < 1e-9 && (axis[2].abs() - 1.0).abs() < 1e-9, "the rectangle's inner side runs along the finger: {axis:?}");
    let radial = [45f64.to_radians().cos(), 45f64.to_radians().sin()];
    assert!((pivot[0] * radial[0] + pivot[1] * radial[1] - r).abs() < 1e-9, "{pivot:?}");
    assert_eq!(revolve.component.attach, Attach::Join);
}

/// `n` rectangles half a millimetre apart, four lines each.
fn rectangles(n: usize) -> Sketch {
    let mut s = Sketch::default();
    for i in 0..n {
        let (x, y) = ((i % 10) as f64 * 0.7 - 3.5, (i / 10) as f64 * 0.9 - 2.5);
        s.add_rectangle([x, y], [x + 0.5, y + 0.6], false).unwrap();
    }
    s
}

/// Frame cost with 200 entities in sketch mode, its parts, and the solve behind one dimension; run `--ignored --nocapture`.
#[test]
#[ignore]
fn sketch_mode_frame_cost_with_two_hundred_entities() {
    use ringdesign_workbench::sketch_tools::{self, Input, SnapCache, Tool, Tools, Underlay};
    let best = |runs: usize, f: &mut dyn FnMut()| {
        (0..runs)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .fold(f64::INFINITY, f64::min)
    };
    // Fifty rectangles of four lines with their constraints cleared.
    let mut s = rectangles(50);
    s.constraints.clear();
    assert_eq!(s.entities.len(), 200);
    assert!(rectangles(50).profile_regions().is_err(), "the solver's 128-point cap refuses fifty held rectangles");
    let under = Underlay::default();
    let cache_ms = best(5, &mut || drop(SnapCache::of(&s, &under)));
    let regions_ms = best(5, &mut || drop(s.profile_regions().unwrap()));
    let regions = s.profile_regions().unwrap();
    let fill_ms = best(5, &mut || drop(regions.iter().map(|r| r.triangles(0.01)).collect::<Vec<_>>()));
    let cache = SnapCache::of(&s, &under);
    let snap_us = best(5, &mut || {
        for k in 0..100 {
            sketch_tools::snap(&s, &under, &cache, [k as f64 * 0.07 - 3.5, 0.3], 0.2, Some(0.5));
        }
    }) * 10.0;
    let draw_ms = best(5, &mut || drop(s.entities.iter().map(|e| s.polylines(e.id, 0.01)).collect::<Vec<_>>()));
    // A dimension's solve, within the solver's 128 movable points and past them.
    let solve = |n: usize| {
        let (mut t, mut s) = (Tools::default(), rectangles(n));
        t.set_tool(Tool::Dimension);
        t.feed(&mut s, Input::Pointer { raw: [-3.25, -2.5], snapped: None, reach: 0.05 });
        t.feed(&mut s, Input::Click { add: false });
        t.feed(&mut s, Input::Typed { key: "length", value: 0.4 });
        let out = t.feed(&mut s, Input::Confirm);
        (t.last_solve_ms, out)
    };
    let (small, small_out) = solve(30);
    let (large, large_out) = solve(50);
    // Whole frames in sketch mode, the pointer moving over the plane.
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    sketch_on_the_top(&mut h, pane);
    let rect = viewport_rect(&h);
    let frames = |h: &mut Harness<'static, RingDesignerApp>| {
        let t = Instant::now();
        for k in 0..60 {
            h.hover_at(rect.center() + egui::vec2((k % 7) as f32 * 3.0, (k % 5) as f32 * 3.0));
            h.run_steps(1);
        }
        t.elapsed().as_secs_f64() * 1e3 / 60.0
    };
    let empty = frames(&mut h);
    h.state_mut().sketch.set_working(s.clone());
    h.run_steps(3);
    let full = frames(&mut h);
    eprintln!(
        "200 entities: snap cache {cache_ms:.2} ms, regions {regions_ms:.2} ms and their fill {fill_ms:.2} ms once per edit; a snap {snap_us:.1} µs, the curves to draw {draw_ms:.3} ms a frame; \
         whole frames in sketch mode {empty:.2} ms empty, {full:.2} ms with 200 entities; a dimension's solve over 30 rectangles (120 points) {small:?} ms {small_out:?}, over 50 (200 points) {large:?} ms {large_out:?}"
    );
}


#[test]
fn the_toolbar_undo_while_sketching_takes_back_the_stroke_and_leaves_the_document_alone() {
    let mut h = harness();
    let pane = court_with_a_box(&mut h);
    sketch_on_the_top(&mut h, pane);
    let entries = h.state().history.present();
    tool(&mut h, "Rectangle");
    let (a, b) = (on_plane(&h, pane, [-2.0, -1.5]), on_plane(&h, pane, [2.0, 1.5]));
    tap(&mut h, a);
    tap(&mut h, b);
    assert_eq!(working(&h).entities.len(), 4, "{}", h.state().status);
    // What the Undo button and the Edit menu call.
    h.state_mut().undo();
    h.run_steps(2);
    assert!(crate::sketch_mode::active(h.state()), "still sketching");
    assert_eq!(working(&h).entities.len(), 0, "the rectangle is taken back inside the sketch");
    assert_eq!(h.state().history.present(), entries, "the document's history is untouched");
    h.state_mut().redo();
    h.run_steps(2);
    assert_eq!(working(&h).entities.len(), 4, "and redo puts it back");
    h.state_mut().jump_history(0);
    assert_eq!(h.state().history.present(), entries, "a history jump waits until the sketch is finished or left");
    assert!(h.state().status.contains("Finish or leave the sketch"), "{}", h.state().status);
}
