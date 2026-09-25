//! Arrays round the ring and round a stone, mirrors and press-pull from the Ring viewport.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use ringdesign_core::{
    Mesh, RingDesign,
    cad::{Attach, Component, ComponentRole, Document, Feature, MirrorPlane, Operation, PatternKind, Placement, builders, face_signature},
    gem::{Gem, GemCut},
};
use ringdesign_workbench::viewport::{Sel, patterns as keys};

fn court() -> RingDesign {
    ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
}

fn part(id: u64, name: &str, operation: Operation, placement: Placement) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { attach: Attach::Join, placement, ..Component::default() } }
}

/// A 2 mm cylinder standing 1.3 mm proud of the band at `theta`.
fn post(id: u64, theta: f64) -> Feature {
    part(id, "Post", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, Placement::ring(theta, 0.3))
}

/// A built Ring viewport of the Court band carrying `features`, seen straight down onto the ring's top.
fn ring_with(h: &mut Harness<'static, RingDesignerApp>, features: Vec<Feature>) -> usize {
    ring_on(h, court(), features)
}

/// A built Ring viewport of `base` carrying `features`, seen straight down onto the ring's top.
fn ring_on(h: &mut Harness<'static, RingDesignerApp>, base: RingDesign, features: Vec<Feature>) -> usize {
    let pane = {
        let app = h.state_mut();
        app.switch_desktop(crate::dock::Desktop::Model);
        app.set_layout(crate::pane::Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = crate::pane::PaneKind::Solid;
        app.active_pane = pane;
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
        for f in features {
            doc.append(f).unwrap();
        }
        app.design = RingDesign { cad: Some(doc), ..base };
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

fn viewport_rect(h: &Harness<'static, RingDesignerApp>) -> Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

fn screen(h: &Harness<'static, RingDesignerApp>, pane: usize, world: [f32; 3]) -> Pos2 {
    h.state().panes[pane].camera.projector(viewport_rect(h)).at(world)
}

/// The top of part `id` on screen, looking down onto it.
fn top_of(h: &Harness<'static, RingDesignerApp>, pane: usize, id: u64) -> Pos2 {
    let (lo, hi) = component_mesh(h, id).bounds().unwrap();
    screen(h, pane, [(lo.0 + hi.0) * 0.5, hi.1, (lo.2 + hi.2) * 0.5])
}

/// A point of the viewport with nothing under it, over the ring's top.
fn beside(h: &Harness<'static, RingDesignerApp>) -> Pos2 {
    let rect = viewport_rect(h);
    rect.center() - vec2(rect.width() * 0.3, rect.height() * 0.3)
}

fn doc(h: &Harness<'static, RingDesignerApp>) -> Document {
    h.state().design.cad.clone().expect("a document")
}

fn live(h: &Harness<'static, RingDesignerApp>) -> Option<&'static str> {
    h.state().command.session.command().map(|c| c.key())
}

fn component_mesh(h: &Harness<'static, RingDesignerApp>, id: u64) -> Mesh {
    let b = h.state().build.clone().expect("a build");
    b.parts.evaluated.as_ref().and_then(|e| e.components.iter().find(|c| c.id == id)).map(|c| c.mesh.clone()).unwrap_or_else(|| panic!("part #{id} was not built"))
}

/// The ordinal of part `id`'s planar face that looks along `dir` in the part's own frame.
fn face_along(h: &Harness<'static, RingDesignerApp>, id: u64, dir: [f64; 3]) -> u32 {
    let b = h.state().build.clone().unwrap();
    let c = b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap().clone();
    (0..c.body.faces.len()).find(|i| face_signature(&c.body, *i, &c.frame).is_some_and(|s| (0..3).map(|k| s.normal[k] * dir[k]).sum::<f64>() > 0.99)).unwrap() as u32
}

fn staged() -> Mesh {
    crate::patterns::STAGED.with(|s| s.borrow().clone())
}

fn centroid(points: &[ringdesign_core::Vec3]) -> [f64; 3] {
    let n = points.len().max(1) as f64;
    [points.iter().map(|p| p.0 as f64).sum::<f64>() / n, points.iter().map(|p| p.1 as f64).sum::<f64>() / n, points.iter().map(|p| p.2 as f64).sum::<f64>() / n]
}

/// Where each copy of a ghost of `per` vertices a copy stands round the ring, in degrees.
fn thetas(ghost: &Mesh, per: usize) -> Vec<f64> {
    ghost.vertices.chunks(per).map(|c| {
        let p = centroid(c);
        p[1].atan2(p[0]).to_degrees().rem_euclid(360.0)
    }).collect()
}

fn text(h: &mut Harness<'static, RingDesignerApp>, t: &str) {
    h.event(Event::Text(t.into()));
    h.run_steps(2);
}

fn press(h: &mut Harness<'static, RingDesignerApp>, key: Key) {
    h.key_press(key);
    h.run_steps(2);
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
fn an_array_round_the_ring_takes_its_count_from_the_bar_shows_every_copy_and_is_one_undo_step() {
    let mut h = harness();
    let pane = ring_with(&mut h, vec![post(2, 90.0)]);
    let start = h.state().history.present();
    let per = component_mesh(&h, 2).vertices.len();
    // Right-click the post's top: Pattern, Array round the ring.
    let top = top_of(&h, pane, 2);
    menu(&mut h, top, "Pattern", "Array round the ring…");
    assert_eq!(live(&h), Some("array"));
    assert!(viewport_label(&h).contains("Array round the ring live"), "{}", viewport_label(&h));
    // Six by default: five ghosts, a sixth of the ring apart.
    let ghost = staged();
    assert_eq!(ghost.vertices.len(), 5 * per);
    let expect = |n: usize| (1..n).map(|k| (90.0 + 360.0 * k as f64 / n as f64) % 360.0).collect::<Vec<_>>();
    let near = |a: Vec<f64>, b: Vec<f64>| a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 0.5);
    assert!(near(thetas(&ghost, per), expect(6)), "{:?}", thetas(&ghost, per));
    // A digit starts the count: three in all is two ghosts, at 210° and 330°.
    h.hover_at(beside(&h));
    h.run_steps(2);
    text(&mut h, "3");
    assert!(h.state().command.bar.has_focus(&h.ctx), "a digit starts the first field");
    let ghost = staged();
    assert!(ghost.vertices.len() == 2 * per && near(thetas(&ghost, per), expect(3)), "{:?}", thetas(&ghost, per));
    assert_eq!(h.state().command.session.preview().unwrap().caption, "Array round the ring: 3 in all, a copy every 120.0°");
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().history.present(), start + 1, "one gesture, one undo step");
    let array = doc(&h).features.last().cloned().unwrap();
    assert_eq!((array.name.as_str(), array.component.attach, array.component.placement.clone()), ("Ring array of Post", Attach::Join, Placement::Free));
    assert!(matches!(&array.operation, Operation::Pattern { sources, kind: PatternKind::Ring { count: 3, span_deg } } if sources[..] == [2] && *span_deg == 360.0), "{:?}", array.operation);
    assert_eq!(h.state().selection.items.last(), Some(&Sel::Part(array.id)), "the copies are chosen");
    // Built: the post and its two copies stand joined in one watertight ring.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let b = h.state().build.clone().unwrap();
    assert!(b.report.validation.watertight, "{:?}", b.report.validation);
    assert_eq!((b.parts.joined, b.parts.features.clone()), (2, vec![2, array.id]), "{:?}", b.parts.notes);
    let built = thetas(&component_mesh(&h, array.id), per);
    assert!(near(built, expect(3)), "the copies stand where their ghosts did");
    // Escape on a fresh array puts its ghosts away; Undo takes the array back.
    crate::patterns::start(h.state_mut(), pane, 2, keys::RING_ARRAY);
    h.run_steps(2);
    assert_eq!((live(&h), staged().vertices.len()), (Some("array"), 5 * per));
    h.hover_at(beside(&h));
    press(&mut h, Key::Escape);
    assert_eq!((live(&h), staged().vertices.len()), (None, 0), "cancelled, with nothing left staged");
    h.state_mut().undo();
    assert_eq!(doc(&h).features.len(), 2);
}

#[test]
fn an_array_round_its_stone_turns_a_post_about_the_stone_it_stands_by_and_refuses_a_part_by_none() {
    let mut h = harness();
    let gem = Gem::calibrated(GemCut::Round, 5.0);
    let stone = builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm("claw4", gem)));
    let prong = part(3, "Prong", Operation::Cylinder { radius_mm: 0.4, height_mm: 3.0 }, Placement::ring(106.0, 0.9));
    let pane = ring_with(&mut h, vec![stone, prong, post(4, 270.0)]);
    let start = h.state().history.present();
    crate::patterns::start(h.state_mut(), pane, 3, keys::STONE_ARRAY);
    h.run_steps(2);
    assert_eq!(live(&h), Some("array"));
    assert_eq!(h.state().command.session.command().unwrap().title(), "Array round the stone");
    let per = component_mesh(&h, 3).vertices.len();
    assert_eq!(staged().vertices.len(), 5 * per, "five ghosts round the stone");
    h.hover_at(beside(&h));
    h.run_steps(2);
    text(&mut h, "4");
    assert_eq!(staged().vertices.len(), 3 * per);
    press(&mut h, Key::Enter);
    let array = doc(&h).features.last().cloned().unwrap();
    assert_eq!(array.name, "Array of Prong");
    assert!(matches!(&array.operation, Operation::Pattern { sources, kind: PatternKind::About { part: 2, count: 4, .. } } if sources[..] == [3]), "{:?}", array.operation);
    assert_eq!(h.state().history.present(), start + 1);
    // A post on the palm stands by no stone: refused by name, nothing started.
    crate::patterns::start(h.state_mut(), pane, 4, keys::STONE_ARRAY);
    assert_eq!(live(&h), None);
    assert!(h.state().status.contains("stands on no stone and by none"), "{}", h.state().status);
    assert_eq!(h.state().history.present(), start + 1);
}

#[test]
fn mirrors_across_the_band_and_through_the_head_land_at_once_and_a_part_on_the_plane_is_refused() {
    let mut h = harness();
    let block = |id: u64, theta: f64, across: f64| {
        let seat = Placement::Ring { theta_deg: theta, across_mm: across, height_mm: 0.3, spin_deg: 25.0, tilt_deg: 0.0, cant_deg: 0.0 };
        part(id, "Block", Operation::Box { size: [1.2, 0.8, 1.0] }, seat)
    };
    let pane = ring_with(&mut h, vec![block(2, 90.0, 1.0), block(3, 60.0, 0.0), block(4, 90.0, 0.0)]);
    let start = h.state().history.present();
    crate::patterns::start(h.state_mut(), pane, 2, keys::MIRROR_BAND);
    assert_eq!(live(&h), None, "a mirror has nothing to type");
    let mirror = doc(&h).features.last().cloned().unwrap();
    assert_eq!((mirror.name.as_str(), mirror.component.attach), ("Mirror of Block", Attach::Join));
    assert!(matches!(&mirror.operation, Operation::Pattern { sources, kind: PatternKind::Mirror { plane: MirrorPlane::Band } } if sources[..] == [2]));
    assert_eq!(h.state().history.present(), start + 1);
    assert_eq!(h.state().selection.items.last(), Some(&Sel::Part(mirror.id)));
    crate::patterns::start(h.state_mut(), pane, 3, keys::MIRROR_HEAD);
    let through = doc(&h).features.last().cloned().unwrap();
    assert!(matches!(&through.operation, Operation::Pattern { sources, kind: PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg } } } if sources[..] == [3] && *theta_deg == 90.0), "{:?}", through.operation);
    assert_eq!(h.state().history.present(), start + 2);
    // A block on the band's mid-plane and on the head's plane is its own mirror.
    for key in [keys::MIRROR_BAND, keys::MIRROR_HEAD] {
        crate::patterns::start(h.state_mut(), pane, 4, key);
        assert!(h.state().status.contains("its mirror would be itself"), "{key}: {}", h.state().status);
    }
    assert_eq!(h.state().history.present(), start + 2);
    // Built: both mirrors stand joined, the first at the far edge of the band.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let b = h.state().build.clone().unwrap();
    assert!(b.report.validation.watertight, "{:?}", b.report.validation);
    assert_eq!(b.parts.joined, 5, "{:?}", b.parts.notes);
    let (a, m) = (centroid(&component_mesh(&h, 2).vertices), centroid(&component_mesh(&h, mirror.id).vertices));
    assert!((a[2] + m[2]).abs() < 0.02 && a[2] > 0.5, "{a:?} {m:?}");
    let (c, t) = (centroid(&component_mesh(&h, 3).vertices), centroid(&component_mesh(&h, through.id).vertices));
    assert!((c[0] + t[0]).abs() < 0.02 && (c[1] - t[1]).abs() < 0.02, "{c:?} {t:?}");
}

#[test]
fn press_pull_sizes_a_seated_box_from_its_top_pushes_a_side_through_the_kernel_and_refuses_a_mesh_by_name() {
    let mut h = harness();
    let block = part(2, "Block", Operation::Box { size: [2.0, 1.5, 1.0] }, Placement::ring(90.0, 0.4));
    let gem = Gem::calibrated(GemCut::Round, 5.0);
    let stone = builders::stone_feature(3, gem, Placement::ring(270.0, builders::stand_off_mm("claw4", gem)));
    let claws = builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }));
    let pane = ring_with(&mut h, vec![block, stone, claws]);
    let start = h.state().history.present();
    let top = face_along(&h, 2, [0.0, 0.0, 1.0]);
    let (below, before) = component_mesh(&h, 2).bounds().unwrap();
    crate::patterns::press_pull(h.state_mut(), pane, 2, top);
    h.run_steps(2);
    assert_eq!(live(&h), Some("press-pull"));
    h.hover_at(beside(&h));
    h.run_steps(2);
    text(&mut h, "0.5");
    // The ghost is the top carried half a millimetre out, walled back to where it stands.
    let (lo, hi) = staged().bounds().expect("a ghost");
    assert!((hi.1 - before.1 - 0.5).abs() < 1e-3 && (lo.1 - before.1).abs() < 1e-3, "{lo:?} {hi:?} over {before:?}");
    assert_eq!(h.state().command.session.preview().unwrap().caption, "Press-pull out 0.50 mm · sizes Block");
    press(&mut h, Key::Enter);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().history.present(), start + 1, "the size and the seat are one undo step");
    let sized = doc(&h).feature(2).cloned().unwrap();
    assert!(matches!(sized.operation, Operation::Box { size } if size == [2.0, 1.5, 1.5]), "{:?}", sized.operation);
    assert!(matches!(sized.component.placement, Placement::Ring { height_mm, .. } if (height_mm - 0.65).abs() < 1e-12), "{:?}", sized.component.placement);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let (lo2, hi2) = component_mesh(&h, 2).bounds().unwrap();
    assert!((hi2.1 - before.1 - 0.5).abs() < 0.02 && (lo2.1 - below.1).abs() < 0.02, "the top rose half a millimetre and the bottom stayed: {lo2:?} {hi2:?}");
    let volume = component_mesh(&h, 2).volume_mm3();
    assert!((volume - 4.5).abs() < 1e-3, "{volume}");
    // A side face is no size of the box's: the kernel pushes it, as a feature of its own.
    let side = face_along(&h, 2, [0.0, 1.0, 0.0]);
    crate::patterns::press_pull(h.state_mut(), pane, 2, side);
    h.run_steps(2);
    h.hover_at(beside(&h));
    h.run_steps(2);
    text(&mut h, "0.3");
    press(&mut h, Key::Enter);
    let pull = doc(&h).features.last().cloned().unwrap();
    assert_eq!((pull.name.as_str(), pull.component.attach, h.state().history.present()), ("Press-pull of Block", Attach::Join, start + 2));
    assert!(matches!(&pull.operation, Operation::PressPull { source: 2, face, distance_mm } if face.ordinal == side as usize && *distance_mm == 0.3 && face.signature.is_some()), "{:?}", pull.operation);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let grown = component_mesh(&h, pull.id).volume_mm3() - volume;
    assert!((grown - 2.0 * 1.5 * 0.3).abs() < 1e-3, "a 2 × 1.5 face pushed 0.3 mm out adds {grown:.4} mm³");
    // A claw head is a mesh a builder made: refused by name, nothing started.
    crate::patterns::press_pull(h.state_mut(), pane, 4, 0);
    assert_eq!(live(&h), None);
    assert_eq!(h.state().status, "Press-pull moves a kernel part's faces; #4 Four-claw head is a mesh a builder made");
    assert_eq!(h.state().history.present(), start + 2);
    // The right-click on a face offers it beside the patterns.
    let at = top_of(&h, pane, pull.id);
    click_at(&mut h, at, PointerButton::Secondary, Modifiers::NONE);
    assert!(h.query_by_label("Press-pull").is_some() && h.query_by_label("Pattern ⏵").is_some(), "the face's menu offers press-pull and the patterns");
    press(&mut h, Key::Escape);
}

#[test]
fn press_pull_on_the_floor_of_a_cut_sketched_on_a_face_deepens_the_cut() {
    let mut h = harness();
    let pane = ring_with(&mut h, vec![part(2, "Block", Operation::Box { size: [4.0, 3.0, 2.0] }, Placement::ring(90.0, 0.0))]);
    // A 2 × 1.5 pocket sketched on the block's top face and cut 0.5 mm into it, the way both apps make one.
    let top = face_along(&h, 2, [0.0, 0.0, 1.0]);
    let built = h.state().build.clone().unwrap();
    let c = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap();
    let mut sketch = ringdesign_core::sketch::Sketch::rectangle(2.0, 1.5);
    sketch.plane.on_face = Some(ringdesign_core::sketch::anchor::on_face(&c.frame, 2, &c.body, top as usize).unwrap());
    let pocket = Operation::Extrude { sketch: ringdesign_core::cad::Profile::Feature { feature: 3 }, height_mm: -0.5, draft_deg: 0.0 };
    {
        let app = h.state_mut();
        let doc = app.design.cad.as_mut().unwrap();
        doc.append(Feature { id: 3, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch }, component: Component::default() }).unwrap();
        doc.append(Feature { id: 4, name: "Extrude cut".into(), enabled: true, operation: pocket, component: Component { attach: Attach::Cut, ..Component::default() } }).unwrap();
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(&mut h);
    let before = h.state().build.as_ref().unwrap().mesh.volume_mm3();
    // Chosen and seen from the side, its gizmo carries a grip on its depth, read from the face its sketch lies on.
    h.state_mut().panes[pane].camera.yaw = 0.0;
    h.state_mut().selection.click(Some(Sel::Part(4)), ringdesign_workbench::viewport::Mods::default());
    h.run_steps(3);
    assert!(h.query_by_label("Grip: Extrusion").is_some(), "the cut's depth has a grip");
    h.state_mut().selection.click(None, ringdesign_workbench::viewport::Mods::default());
    h.state_mut().panes[pane].camera.yaw = std::f32::consts::FRAC_PI_2;
    h.run_steps(2);
    let start = h.state().history.present();
    // Its floor faces down into the block; pulled 0.3 mm out it deepens the cut in one step, no kernel press-pull added.
    let floor = face_along(&h, 4, [0.0, -1.0, 0.0]);
    crate::patterns::press_pull(h.state_mut(), pane, 4, floor);
    h.run_steps(2);
    assert_eq!(live(&h), Some("press-pull"));
    h.hover_at(beside(&h));
    h.run_steps(2);
    text(&mut h, "0.3");
    press(&mut h, Key::Enter);
    assert_eq!((live(&h), h.state().history.present()), (None, start + 1));
    let d = doc(&h);
    assert!(matches!(d.feature(4).unwrap().operation, Operation::Extrude { height_mm, .. } if (height_mm + 0.8).abs() < 1e-12), "{:?}", d.feature(4).unwrap().operation);
    assert!(!d.features.iter().any(|f| matches!(f.operation, Operation::PressPull { .. })));
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let taken = before - h.state().build.as_ref().unwrap().mesh.volume_mm3();
    assert!((taken - 2.0 * 1.5 * 0.3).abs() < 1e-3, "0.3 mm more of a 2 × 1.5 pocket: {taken:.4} mm³");
}

/// The furthest any vertex of `a` stands from its nearest vertex of `b`, mm.
fn farthest(a: &[ringdesign_core::Vec3], b: &[ringdesign_core::Vec3]) -> f64 {
    let d = |p: &ringdesign_core::Vec3, q: &ringdesign_core::Vec3| ((p.0 - q.0) as f64).hypot((p.1 - q.1) as f64).hypot((p.2 - q.2) as f64);
    a.iter().map(|p| b.iter().map(|q| d(p, q)).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max)
}

#[test]
fn an_arrays_ghost_on_a_signets_shoulders_stands_where_its_copies_are_built() {
    let mut h = harness();
    let heart = ringdesign_core::templates::all().iter().find(|t| t.name == "Heart signet").unwrap().design();
    // A post on the shoulder at 45°: six round the ring stand on the shoulders, the head's edge and the shank.
    let pane = ring_on(&mut h, heart, vec![post(2, 45.0)]);
    let source = component_mesh(&h, 2);
    let per = source.vertices.len();
    crate::patterns::start(h.state_mut(), pane, 2, keys::RING_ARRAY);
    h.run_steps(2);
    assert_eq!(live(&h), Some("array"));
    let ghost = staged();
    assert_eq!(ghost.vertices.len(), 5 * per);
    h.hover_at(beside(&h));
    h.run_steps(2);
    press(&mut h, Key::Enter);
    let array = doc(&h).features.last().cloned().unwrap();
    assert!(matches!(&array.operation, Operation::Pattern { sources, kind: PatternKind::Ring { count: 6, .. } } if sources[..] == [2]), "{:?}", array.operation);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let built = component_mesh(&h, array.id);
    // Each copy the ghost showed against the copies built, vertex to nearest vertex both ways.
    let (shown, kept) = (farthest(&ghost.vertices, &built.vertices), farthest(&built.vertices, &ghost.vertices));
    // The same copies turned rigidly round the finger, as the ghost used to carry them.
    let turned: Vec<ringdesign_core::Vec3> = (1..6)
        .flat_map(|k| {
            let (s, c) = (60.0f64 * k as f64).to_radians().sin_cos();
            source.vertices.iter().map(move |v| ringdesign_core::Vec3((c * v.0 as f64 - s * v.1 as f64) as f32, (s * v.0 as f64 + c * v.1 as f64) as f32, v.2))
        })
        .collect();
    let rigid = farthest(&turned, &built.vertices);
    eprintln!("ring array of 6 on a heart signet's shoulder: the ghost stands within {shown:.5} mm of the built copies ({kept:.5} back); turned rigidly it stood {rigid:.3} mm off");
    assert!(shown < 0.02 && kept < 0.02, "{shown} {kept}");
    assert!(rigid > 0.2, "the signet's shoulders must be where rigid copies miss: {rigid}");
}

#[test]
fn a_ring_array_of_a_head_on_a_stone_on_a_plate_builds_each_copy_on_the_plate_where_copy_motions_stands_it() {
    use ringdesign_core::cad::{FaceSeat, pattern, stone_on_face};
    let mut h = harness();
    // A plate 14 mm round the ring on the top, a 1.5 mm stone on its top, and the stone's four-claw head.
    let plate = part(2, "Plate", Operation::Box { size: [4.0, 14.0, 1.5] }, Placement::ring(90.0, 0.65));
    let mut d = court();
    let mut first = Document::default();
    first.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    first.append(plate.clone()).unwrap();
    d.cad = Some(first);
    let e = ringdesign_core::cad::evaluate(&d, &ringdesign_core::AlphaLibrary::builtin(), ringdesign_core::BuildParams::default()).unwrap();
    let host = e.components.iter().find(|c| c.id == 2).unwrap();
    let top = (0..host.body.faces.len()).find(|i| face_signature(&host.body, *i, &host.frame).is_some_and(|s| s.normal[2] > 0.99)).unwrap() as u32;
    let gem = Gem::calibrated(GemCut::Round, 1.5);
    let seat = FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
    let head = builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }));
    let pane = ring_with(&mut h, vec![plate, stone_on_face(3, gem, 2, &seat), head]);
    // Three heads 12° apart, typed in the bar.
    crate::patterns::start(h.state_mut(), pane, 4, keys::RING_ARRAY);
    h.run_steps(2);
    assert_eq!(live(&h), Some("array"));
    let ghost_ready = staged().vertices.len();
    h.hover_at(beside(&h));
    h.run_steps(2);
    text(&mut h, "3");
    press(&mut h, Key::Tab);
    text(&mut h, "24");
    assert_eq!(h.state().command.session.preview().unwrap().caption, "Array round the ring: 3 in all over 24°, a copy every 12.0°");
    let ghost = staged();
    press(&mut h, Key::Enter);
    let array = doc(&h).features.last().cloned().unwrap();
    assert!(matches!(&array.operation, Operation::Pattern { sources, kind: PatternKind::Ring { count: 3, span_deg } } if sources[..] == [4] && *span_deg == 24.0), "{:?}", array.operation);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let b = h.state().build.clone().unwrap();
    assert!(b.report.validation.watertight && b.parts.notes.is_empty(), "{:?} {:?}", b.report.validation, b.parts.notes);
    let evaluated = b.parts.evaluated.as_ref().unwrap();
    // Each copy is the head carried by the motion `copy_motions` reads off the build: dropped back onto the plate, square to its top.
    let kind = PatternKind::Ring { count: 3, span_deg: 24.0 };
    let motions = pattern::copy_motions(&h.state().design, b.band.as_deref(), evaluated, 4, &kind).unwrap();
    let source = component_mesh(&h, 4);
    let carried: Vec<ringdesign_core::Vec3> = motions.iter().flat_map(|m| source.vertices.iter().map(|v| {
        let p = m.point([v.0 as f64, v.1 as f64, v.2 as f64]);
        ringdesign_core::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
    })).collect();
    let built = component_mesh(&h, array.id);
    let (there, back) = (farthest(&carried, &built.vertices), farthest(&built.vertices, &carried));
    let stone = evaluated.components.iter().find(|c| c.id == 3).unwrap().frame;
    let tilts: Vec<f64> = motions
        .iter()
        .map(|m| {
            let z = m.vector(stone.z_axis);
            (0..3).map(|k| z[k] * stone.z_axis[k]).sum::<f64>().clamp(-1.0, 1.0).acos().to_degrees()
        })
        .collect();
    eprintln!("ring array of a head on a plate: built within {there:.6} mm of copy_motions ({back:.6} back); its copies lean {tilts:?}° off the plate's normal; the ghost as staged stood {:.4} mm off the built copies over {ghost_ready} vertices", farthest(&ghost.vertices, &built.vertices));
    assert!(there < 1e-4 && back < 1e-4, "{there} {back}");
    assert!(tilts.len() == 2 && tilts.iter().all(|t| *t < 1e-6), "every copy's table faces along the plate's normal: {tilts:?}");
}

#[test]
fn an_arrays_ghost_shows_a_copy_off_the_plate_refused_and_the_built_pattern_leaves_it_out_by_name() {
    use ringdesign_core::cad::{FaceSeat, FeatureStatus, stone_on_face};
    let mut h = harness();
    // A plate 14 mm round the ring on the top, a 1.5 mm stone on its top, and the stone's four-claw head.
    let plate = part(2, "Plate", Operation::Box { size: [4.0, 14.0, 1.5] }, Placement::ring(90.0, 0.65));
    let mut d = court();
    let mut first = Document::default();
    first.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    first.append(plate.clone()).unwrap();
    d.cad = Some(first);
    let e = ringdesign_core::cad::evaluate(&d, &ringdesign_core::AlphaLibrary::builtin(), ringdesign_core::BuildParams::default()).unwrap();
    let host = e.components.iter().find(|c| c.id == 2).unwrap();
    let top = (0..host.body.faces.len()).find(|i| face_signature(&host.body, *i, &host.frame).is_some_and(|s| s.normal[2] > 0.99)).unwrap() as u32;
    let gem = Gem::calibrated(GemCut::Round, 1.5);
    let seat = FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
    let head = builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }));
    let pane = ring_with(&mut h, vec![plate, stone_on_face(3, gem, 2, &seat), head]);
    let source = component_mesh(&h, 4);
    let per = source.vertices.len();
    let draft = |h: &Harness<'static, RingDesignerApp>| h.state().renderer.lock().unwrap().staged_preview().1;
    let array = |h: &mut Harness<'static, RingDesignerApp>, count: &str, span: &str| {
        crate::patterns::start(h.state_mut(), pane, 4, keys::RING_ARRAY);
        h.run_steps(2);
        h.hover_at(beside(h));
        h.run_steps(2);
        text(h, count);
        press(h, Key::Tab);
        text(h, span);
        h.state().command.session.preview().unwrap().caption
    };
    // Three over 24° all stand on the plate: the ghost is plain metal and says nothing of the face.
    let caption = array(&mut h, "3", "24");
    assert!(!caption.contains("left out") && !draft(&h), "{caption}");
    assert_eq!(staged().vertices.len(), 2 * per);
    crate::command::cancel(h.state_mut());
    h.run_steps(2);
    assert!(!draft(&h) && staged().vertices.is_empty(), "a cancelled ghost goes, colours and all");
    // Four over 72°: the copies 48° and 72° round would stand past the plate's end; the ghost draws them red and says which.
    let caption = array(&mut h, "4", "72");
    assert_eq!(caption, "Array round the ring: 4 in all over 72°, a copy every 24.0° · 2 copies stand off the face of #2 Plate, left out: 48°, 72°");
    assert_eq!(staged().vertices.len(), 3 * per, "every copy is drawn, the refused ones among them");
    assert!(draft(&h), "in the draft colours: kept green, refused red");
    press(&mut h, Key::Enter);
    let added = doc(&h).features.last().cloned().unwrap();
    assert!(matches!(&added.operation, Operation::Pattern { sources, kind: PatternKind::Ring { count: 4, span_deg } } if sources[..] == [4] && *span_deg == 72.0), "{:?}", added.operation);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    // Built, the pattern stands the one copy the plate carries, and its status names the two it left out.
    let b = h.state().build.clone().unwrap();
    let evaluated = b.parts.evaluated.as_ref().unwrap();
    let report = evaluated.features.iter().find(|r| r.id == added.id).unwrap().clone();
    assert_eq!(report.status, FeatureStatus::Ok);
    let off: Vec<&String> = report.notes.iter().filter(|n| n.contains("stands off the face of #2 Plate")).collect();
    assert_eq!(off.len(), 2, "{:?}", report.notes);
    assert!(off[0].starts_with("The copy 48.0°") && off[1].starts_with("The copy 72.0°"), "{off:?}");
    let kind = PatternKind::Ring { count: 4, span_deg: 72.0 };
    let kept = ringdesign_core::cad::pattern::copy_motions(&h.state().design, b.band.as_deref(), evaluated, 4, &kind).unwrap();
    assert_eq!(kept.len(), 1);
    let carried: Vec<ringdesign_core::Vec3> = source.vertices.iter().map(|v| {
        let p = kept[0].point([v.0 as f64, v.1 as f64, v.2 as f64]);
        ringdesign_core::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
    }).collect();
    let built = component_mesh(&h, added.id);
    assert!(farthest(&carried, &built.vertices) < 1e-4 && farthest(&built.vertices, &carried) < 1e-4, "the one copy built is the one the ghost kept");
}

#[test]
fn a_work_plane_is_drawn_named_and_right_clicked_to_sketch_on_or_mirror_the_chosen_part_across() {
    use egui_kittest::kittest::NodeT;
    use ringdesign_core::cad::PlaneBase;
    let mut h = harness();
    let plane = part(3, "Section at 0°", Operation::Plane { base: PlaneBase::Section { theta_deg: 0.0 }, offset_mm: 0.0 }, Placement::Free);
    let parting = part(4, "Parting", Operation::Plane { base: PlaneBase::Parting, offset_mm: 0.0 }, Placement::Free);
    let planes = [plane, parting].map(|p| Feature { component: Component::default(), ..p });
    let pane = ring_with(&mut h, [vec![post(2, 90.0)], planes.to_vec()].concat());
    let start = h.state().history.present();
    // Seen from over the top the section through 0° faces the camera: a rectangle wider than the ring, named at its corner.
    assert!(h.query_by_label("Work plane: Section at 0°").is_some(), "the plane is drawn and named");
    let shapes = crate::viewport::plane_shapes(h.state());
    assert_eq!(shapes.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Section at 0°", "Parting"]);
    assert!(h.query_by_label("Work plane: Parting").is_some());
    let corners = shapes[0].corners.map(|c| screen(&h, pane, c.map(|v| v as f32)));
    // Its lower edge under the band on screen, clear of the chosen part's gizmo over the top.
    let edge = corners[0] + (corners[1] - corners[0]) * 0.8;
    h.hover_at(edge);
    h.run_steps(2);
    assert!(viewport_label(&h).contains("work plane Section at 0° under the pointer"), "{}", viewport_label(&h));
    // With nothing chosen its menu sketches on it; the mirror waits for a part.
    click_at(&mut h, edge, PointerButton::Secondary, Modifiers::NONE);
    let mirror = h.get_by_label("Mirror the chosen part across it");
    assert!(mirror.accesskit_node().is_disabled());
    assert!(h.query_by_label("Sketch on this plane").is_some() && h.query_by_label("Hide work planes").is_some());
    press(&mut h, Key::Escape);
    // A click on its outline chooses the plane and leaves the parts as they were.
    click_at(&mut h, edge, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(h.state().command.planes.chosen, Some(3));
    assert!(h.state().status.starts_with("Work plane Section at 0°"), "{}", h.state().status);
    // Choose the post, then mirror it across the plane: one Mirror feature, one undo step.
    // The parting plane is seen edge on, a line through the post: the post still takes the pointer.
    let top = top_of(&h, pane, 2);
    click_at(&mut h, top, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(h.state().selection.items.iter().filter_map(Sel::feature).last(), Some(2));
    assert_eq!(h.state().command.planes.chosen, None, "choosing anything else lets the plane go");
    click_at(&mut h, edge, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Mirror the chosen part across it").click();
    h.run_steps(3);
    let mirror = doc(&h).features.last().cloned().unwrap();
    assert!(matches!(&mirror.operation, Operation::Pattern { sources, kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 3 } } } if sources[..] == [2]), "{:?}", mirror.operation);
    assert_eq!(h.state().history.present(), start + 1);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let (a, m) = (centroid(&component_mesh(&h, 2).vertices), centroid(&component_mesh(&h, mirror.id).vertices));
    assert!((a[0] - m[0]).abs() < 1e-3 && (a[1] + m[1]).abs() < 1e-3 && (a[2] - m[2]).abs() < 1e-3 && a[1] > 9.0, "reflected across y = 0: {a:?} {m:?}");
    // Its menu starts a sketch lying on it.
    click_at(&mut h, edge, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Sketch on this plane").click();
    h.run_steps(3);
    assert!(crate::sketch_mode::active(h.state()), "a sketch is live");
    let sketch = doc(&h).features.last().cloned().unwrap();
    let Operation::Sketch { sketch: drawn } = &sketch.operation else { panic!("{:?}", sketch.operation) };
    assert_eq!(drawn.plane.on_face.as_ref().map(|a| a.feature), Some(3), "it lies on the plane");
    assert_eq!(h.state().history.present(), start + 2);
    let n = h.state().sketch.normal().expect("the sketch's plane");
    assert!(n[0].abs() < 1e-9 && (n[1].abs() - 1.0).abs() < 1e-9 && n[2].abs() < 1e-9, "{n:?}");
    crate::sketch_mode::finish(h.state_mut());
    h.run_steps(2);
    // Hidden, it is neither drawn nor taken; the ring's own menu shows it again.
    h.state_mut().command.planes.hidden = true;
    h.run_steps(2);
    assert!(h.query_by_label("Work plane: Section at 0°").is_none() && h.query_by_label("Work plane: Parting").is_none());
    h.hover_at(edge);
    h.run_steps(2);
    assert_eq!(h.state().command.planes.hot, None);
    let away = beside(&h);
    click_at(&mut h, away, PointerButton::Secondary, Modifiers::NONE);
    h.get_by_label("Work planes").click();
    h.run_steps(3);
    assert!(!h.state().command.planes.hidden && h.query_by_label("Work plane: Section at 0°").is_some());
}
