use super::*;
use egui::{Event, TouchDeviceId, TouchId, TouchPhase, vec2};
use ringdesign_core::{BuildParams, cad::{Operation, Placement}, mesh, templates};
use ringdesign_workbench::{command::Axis, gizmo::Handle};

fn court() -> RingDesign {
    templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
}

/// A plain band with a cylinder standing on its top, joined.
fn posted() -> RingDesign {
    let mut d = court();
    let mut history = History::new(&d);
    let (edits, _) = touch::parts::part_here(&d, "Cylinder", 90.0, 0.0).unwrap();
    commit(&mut d, &mut history, &edits, None).unwrap();
    // A small post rather than the starter's 4 mm drum, so the band shows round it.
    let edit = CadEdit::Operation { id: 2, operation: Operation::Cylinder { radius_mm: 1.2, height_mm: 2.0 } };
    commit(&mut d, &mut history, &[edit], None).unwrap();
    d
}

fn build(d: &RingDesign) -> (Built, Option<Arc<PickScene>>, Option<Arc<BandSurface>>) {
    let mut out = mesh::try_build(d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() }).unwrap();
    let band = out.band.take().map(|m| Arc::new(BandSurface::shared(m)));
    let scene = Some(Arc::new(PickScene::build(&out, d)));
    (Built(Arc::new(out)), scene, band)
}

#[test]
fn a_cad_commit_is_one_undo_step_named_by_its_edits_and_a_refusal_changes_nothing() {
    let mut d = court();
    let mut history = History::new(&d);
    // An edit still settling is its own step, committed before the CAD edit lands.
    d.name = "Court with a post".into();
    history.touch();
    let (edits, id) = touch::parts::part_here(&d, "Cylinder", 90.0, 0.0).unwrap();
    let c = commit(&mut d, &mut history, &edits, None).unwrap().unwrap();
    assert_eq!(c.label, "Add Procedural shank · Add Cylinder");
    assert_eq!(c.applied.iter().map(|a| a.id).collect::<Vec<_>>(), [Some(1), Some(id)]);
    assert_eq!(history.timeline().iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["Opened", "Name Court band -> Court with a post", "Add Procedural shank · Add Cylinder"]);
    assert!(!history.is_pending());
    let before = d.clone();
    let refused = commit(&mut d, &mut history, &[CadEdit::Rename { id: 1, name: "Shank".into() }, CadEdit::Remove { id: 99 }], None).unwrap_err();
    assert!(refused.contains("#99"), "{refused}");
    assert_eq!(serde_json::to_value(&d).unwrap(), serde_json::to_value(&before).unwrap());
    assert_eq!(history.timeline().len(), 3, "a refusal adds no step");
    assert!(commit(&mut d, &mut history, &[], None).unwrap().is_none());
    // Undo takes the whole commit back in one step.
    let undone = history.undo().unwrap();
    assert!(undone.cad.is_none() && undone.name == "Court with a post");
}

/// Runs one frame of the CAD layer over `v`'s ring with `events`, at `time`, and then its drawing.
fn frame(ctx: &egui::Context, cad: &mut Cad, v: &View, events: Vec<Event>, time: f64, renderer: &std::sync::Mutex<crate::viewport::GpuMeshRenderer>) -> Took {
    let mut took = Took::default();
    let input = egui::RawInput { events, time: Some(time), screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(420.0, 800.0))), ..Default::default() };
    let mut out = ctx.run_ui(input, |ui| {
        let (_, _) = ui.allocate_exact_size(v.rect.size(), egui::Sense::click_and_drag());
        took = cad.frame(ui, v);
        cad.draw(ui, v, renderer);
    });
    out.textures_delta.clear();
    took
}

fn touch(id: u64, phase: TouchPhase, p: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos: p, force: None }
}

/// A camera looking down on the ring's top, the post's end face to the viewer.
fn camera(built: &Built) -> OrbitCamera {
    let mut camera = OrbitCamera::default();
    camera.fit(built.bounds());
    camera.yaw = std::f32::consts::FRAC_PI_2;
    camera.pitch = 0.25;
    camera
}

#[test]
fn a_tap_chooses_the_post_the_same_spot_walks_down_to_its_face_and_the_band_lets_it_go() {
    let d = posted();
    let (built, scene, band) = build(&d);
    let mut cad = Cad::default();
    cad.landed(&built, scene, band, None, &d);
    let camera = camera(&built);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(420.0, 600.0));
    let lib = AlphaLibrary::builtin();
    let v = View { rect, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    let post = built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap();
    let (lo, hi) = post.mesh.bounds().unwrap();
    let top = camera.projector(rect).at([(lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5, (lo.2 + hi.2) * 0.5]);
    let ctx = egui::Context::default();
    let renderer = std::sync::Mutex::new(crate::viewport::GpuMeshRenderer::default());
    let tap = |cad: &mut Cad, p: Pos2, t: f64| {
        frame(&ctx, cad, &v, vec![touch(1, TouchPhase::Start, p)], t, &renderer);
        frame(&ctx, cad, &v, vec![touch(1, TouchPhase::End, p)], t + 0.1, &renderer)
    };
    let took = tap(&mut cad, top, 1.0);
    assert!(took.tap, "the tap was the CAD layer's");
    assert_eq!(cad.selection.items, [Sel::Part(2)]);
    assert!(renderer.lock().unwrap().has_select(), "the chosen post is tinted");
    let took = tap(&mut cad, top + vec2(3.0, 2.0), 2.0);
    assert!(took.tap);
    assert!(matches!(cad.selection.items[..], [Sel::Face { feature: 2, .. }]), "{:?}", cad.selection.items);
    // A tap on the band far from the post lets the choice go and leaves the tap to the view.
    let band_spot = camera.projector(rect).at([9.8, 0.0, 0.0]);
    let took = tap(&mut cad, band_spot, 3.0);
    assert!(!took.tap);
    assert!(cad.selection.items.is_empty());
    let asked: Vec<String> = cad.take_requests().into_iter().filter_map(|r| if let Request::Status(s) = r { Some(s) } else { None }).collect();
    assert!(asked[0].starts_with("Cylinder") && asked[1].contains("of"), "{asked:?}");
}

/// The ring with the post on it, its build, the CAD layer landed on it, and a camera over the post.
struct Bench {
    d: RingDesign,
    built: Built,
    cad: Cad,
    camera: OrbitCamera,
    lib: AlphaLibrary,
    ctx: egui::Context,
    renderer: std::sync::Mutex<crate::viewport::GpuMeshRenderer>,
    time: f64,
    /// The view the frames run in; the keyboard rising shortens it.
    rect: egui::Rect,
    /// The Measure tool is out.
    measuring: bool,
}

const RECT: egui::Rect = egui::Rect { min: egui::Pos2::ZERO, max: egui::pos2(420.0, 600.0) };

impl Bench {
    fn new(d: RingDesign) -> Self {
        let (built, scene, band) = build(&d);
        let mut cad = Cad::default();
        cad.landed(&built, scene, band, None, &d);
        let camera = camera(&built);
        Self { d, built, cad, camera, lib: AlphaLibrary::builtin(), ctx: egui::Context::default(), renderer: Default::default(), time: 1.0, rect: RECT, measuring: false }
    }
    fn view(&self) -> View<'_> {
        View { rect: RECT, camera: &self.camera, design: &self.d, lib: &self.lib, build: Some(&self.built), field: None, covered: &[], active: true, measuring: self.measuring, switches: Default::default() }
    }
    /// One frame with `events`, a tenth of a second after the last.
    fn step(&mut self, events: Vec<Event>) -> Took {
        self.time += 0.1;
        let v = View { rect: self.rect, camera: &self.camera, design: &self.d, lib: &self.lib, build: Some(&self.built), field: None, covered: &[], active: true, measuring: self.measuring, switches: Default::default() };
        frame(&self.ctx, &mut self.cad, &v, events, self.time, &self.renderer)
    }
    fn tap(&mut self, p: Pos2) -> Took {
        self.step(vec![touch(1, TouchPhase::Start, p)]);
        self.step(vec![touch(1, TouchPhase::End, p)])
    }
    /// A finger held still at `p` past the long press, then lifted: whether the press opened a menu.
    fn hold(&mut self, p: Pos2) -> bool {
        self.step(vec![touch(1, TouchPhase::Start, p)]);
        let mut long = false;
        for _ in 0..5 {
            long |= self.step(Vec::new()).long_press;
        }
        self.step(vec![touch(1, TouchPhase::End, p)]);
        long
    }
    /// One finger dragged from `from` to `to` in `n` steps: whether the CAD layer held the ring at every step.
    fn drag(&mut self, from: Pos2, to: Pos2, n: usize) -> bool {
        let mut held = self.step(vec![touch(1, TouchPhase::Start, from)]).hold;
        for k in 1..=n {
            held &= self.step(vec![touch(1, TouchPhase::Move, from.lerp(to, k as f32 / n as f32))]).hold;
        }
        self.step(vec![touch(1, TouchPhase::End, to)]);
        held
    }
    /// A finger's tap on a button drawn over the ring, as a touch screen reports it: the touch and the pointer it drives.
    fn press_button(&mut self, p: Pos2) {
        let button = |pressed: bool| Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.step(vec![Event::PointerMoved(p), button(true), touch(1, TouchPhase::Start, p)]);
        self.step(vec![button(false), touch(1, TouchPhase::End, p), Event::PointerGone]);
    }
    /// Statuses the CAD layer said since the last call.
    fn said(&mut self) -> Vec<String> {
        self.cad.take_requests().into_iter().filter_map(|r| if let Request::Status(s) = r { Some(s) } else { None }).collect()
    }
    /// Where the post's middle lands on screen.
    fn post(&self) -> Pos2 {
        let post = self.built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap();
        let (lo, hi) = post.mesh.bounds().unwrap();
        self.camera.projector(RECT).at([(lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5, (lo.2 + hi.2) * 0.5])
    }
    /// A point well along one of the chosen part's arrows on screen, and the way out along it.
    fn arrow(&self, handle: Handle) -> (Pos2, egui::Vec2) {
        let c = command::Ctx { rect: RECT, camera: &self.camera, design: &self.d, build: Some(&self.built), band: self.cad.band(), field: None, pins: &[], selection: &self.cad.selection, gizmo: true };
        let (_, g) = command::gizmo_of(&c, &self.cad.live).expect("one part chosen, nothing live");
        let layout = command::layout(&c, &g);
        let (_, mark) = layout.marks.iter().find(|(h, _)| *h == handle).expect("the handle is drawn");
        match mark {
            ringdesign_workbench::gizmo::Mark::Arrow { base, tip } => (base.lerp(*tip, 0.6), (*tip - *base).normalized()),
            other => panic!("{other:?}"),
        }
    }
    fn edits(&mut self) -> Vec<(Vec<CadEdit>, Then)> {
        self.cad.take_requests().into_iter().filter_map(|r| if let Request::Edit { edits, then } = r { Some((edits, then)) } else { None }).collect()
    }
}

#[test]
fn a_stone_on_a_parts_face_takes_the_pressed_point_and_builds_on_the_part() {
    use ringdesign_core::cad::{FaceSeat, FeatureStatus, builders};
    let mut d = court();
    let mut history = History::new(&d);
    let (edits, plate) = touch::parts::part_here(&d, "Box", 90.0, 0.0).unwrap();
    commit(&mut d, &mut history, &edits, None).unwrap();
    let mut b = Bench::new(d);
    let c = b.built.evaluated().unwrap().components.iter().find(|c| c.id == plate).cloned().unwrap();
    let face = (0..c.body.faces.len() as u32).find(|f| FaceSeat::on(&c, *f, None, 0.0).is_ok()).expect("a box has planar faces");
    // Pressed at the part's own origin, off the face's middle.
    let at = c.frame.origin;
    let d = b.d.clone();
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.pressed = Some((at, [0.0, 0.0, 1.0]));
    b.cad.act(&v, MenuAction::AddStoneOnFace { feature: plate, face, key: "round-5" });
    assert!(b.cad.pressed.is_none(), "the press is spent");
    let (edits, then) = b.edits().remove(0);
    let [CadEdit::Add { feature, .. }] = edits.as_slice() else { panic!("{edits:?}") };
    let Operation::Builder { key, on, params } = &feature.operation else { panic!("{:?}", feature.operation) };
    assert_eq!((key.as_str(), *on, then), (builders::STONE, Some(plate), Then::Part(feature.id)));
    let gem = builders::stone_preset("round-5").unwrap().gem();
    let pressed = FaceSeat::on(&c, face, Some(at), builders::stand_off_mm("claw4", gem)).unwrap();
    let middle = FaceSeat::on(&c, face, None, builders::stand_off_mm("claw4", gem)).unwrap();
    assert_eq!(FaceSeat::of(params).unwrap(), Some(pressed.clone()), "seated where the finger pressed");
    assert_ne!(pressed, middle);
    // Committed and built, the stone stands on the plate.
    let mut d = b.d.clone();
    let mut history = History::new(&d);
    let done = commit(&mut d, &mut history, &edits, None).unwrap().unwrap();
    assert_eq!(done.label, "Add Round 5 mm");
    let b = Bench::new(d);
    let e = b.built.evaluated().unwrap();
    assert_eq!(e.status_of(feature.id), Some(&FeatureStatus::Ok), "{:?}", e.features);
}

/// The plain band with a 6.5 mm round standing at 68° in four claws.
fn set_stone() -> (RingDesign, Id) {
    let mut d = RingDesign::default();
    let mut history = History::new(&d);
    let (edits, stone) = touch::parts::stone_here(&d, 68.0, "round-6.5").unwrap();
    commit(&mut d, &mut history, &edits, None).unwrap();
    let (edits, _) = touch::parts::setting(&d, None, Some(stone), None, "claw4").unwrap();
    commit(&mut d, &mut history, &edits, None).unwrap();
    (d, stone)
}

#[test]
fn a_set_stone_follows_the_finger_round_the_ring_as_a_ghost_of_its_own_facets_left_unread() {
    let (d, stone) = set_stone();
    let mut b = Bench::new(d);
    b.cad.choose(stone);
    b.step(Vec::new());
    let (on, out) = b.arrow(Handle::Move(Axis::Theta));
    assert!(b.step(vec![touch(1, TouchPhase::Start, on)]).hold);
    let faces = b.built.evaluated().unwrap().components.iter().find(|c| c.id == stone).unwrap().mesh.faces.len();
    {
        let r = b.renderer.lock().unwrap();
        let (verts, model, draft) = r.preview_state();
        assert_eq!(verts.map(<[f32]>::len), Some(faces * 3 * 12), "the stone's own facets are staged once, twelve floats a corner");
        assert!(model.is_some() && !draft, "a reference stone is carried but never read for draft");
    }
    let theta = |b: &Bench| b.cad.live.session.dimensions().iter().find(|x| x.key == "theta").map(|x| x.value).unwrap();
    // Its head is seated on the stone's own frame, so no angle holds it: 10 pt past the press it already turns.
    b.step(vec![touch(1, TouchPhase::Move, on + out * 10.0)]);
    let first = theta(&b);
    assert!(first.abs() > 1.0 && first.abs() < 6.0, "{first}");
    b.step(vec![touch(1, TouchPhase::Move, on + out * 70.0)]);
    let moved = theta(&b);
    assert!(moved.signum() == first.signum() && moved.abs() > 4.0 * first.abs(), "it follows the finger: {first} then {moved}");
    b.step(vec![touch(1, TouchPhase::End, on + out * 70.0)]);
    let edits = b.edits();
    let [CadEdit::Placement { id, placement: Placement::Ring { theta_deg, .. } }] = edits[0].0.as_slice() else { panic!("{edits:?}") };
    assert_eq!(*id, stone);
    assert!((theta_deg - (68.0 + moved)).abs() < 1e-9, "{theta_deg} after {moved}");
}

#[test]
fn a_ring_array_from_the_menu_waits_for_its_count_and_commits_one_pattern() {
    let mut b = Bench::new(posted());
    b.cad.choose(2);
    let d = b.d.clone();
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.act(&v, MenuAction::Pattern { feature: 2, key: keys::RING_ARRAY });
    assert!(b.cad.live.is_live(), "the array waits for how many");
    assert!(b.edits().is_empty());
    // The count field has the keyboard once the bar is drawn.
    b.step(Vec::new());
    {
        let r = b.renderer.lock().unwrap();
        let (verts, model, _) = r.preview_state();
        assert!(verts.is_some_and(|v| !v.is_empty()) && model.is_some(), "the copies are drawn as a ghost");
    }
    b.step(vec![Event::Text("6".into())]);
    b.step(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    let edits = b.edits();
    let [CadEdit::Add { feature, .. }] = edits[0].0.as_slice() else { panic!("{edits:?}") };
    assert!(matches!(feature.operation, Operation::Pattern { source: 2, kind: ringdesign_core::cad::PatternKind::Ring { count: 6, .. } }), "{:?}", feature.operation);
    assert_eq!(edits[0].1, Then::LastAdded);
    // Through the funnel it is one undo step.
    let mut d = b.d.clone();
    let mut history = History::new(&d);
    let done = commit(&mut d, &mut history, &edits[0].0, b.built.evaluated()).unwrap().unwrap();
    assert_eq!(history.timeline().len(), 2, "{:?}", done.label);
}

#[test]
fn a_finger_drags_the_round_the_ring_arrow_the_ghost_follows_and_the_lift_commits_one_placement() {
    let mut b = Bench::new(posted());
    b.tap(b.post());
    assert_eq!(b.cad.selection.items, [Sel::Part(2)]);
    let (on, out) = b.arrow(Handle::Move(Axis::Theta));
    let took = b.step(vec![touch(1, TouchPhase::Start, on)]);
    assert!(took.hold, "a press on the arrow holds the view");
    let (_, model, _) = b.renderer.lock().unwrap().preview_state();
    let (m, _) = model.expect("the ghost stands where the post does at the press");
    let identity = [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    assert!(m.iter().zip(identity).all(|(a, b)| (a - b).abs() < 1e-3), "{m:?}");
    let mut at = on;
    for _ in 0..6 {
        at += out * 8.0;
        let took = b.step(vec![touch(1, TouchPhase::Move, at)]);
        assert!(took.hold && !took.tap);
    }
    let (_, model, _) = b.renderer.lock().unwrap().preview_state();
    assert!(model.is_some(), "the carried post is drawn as a ghost under a model matrix");
    assert!(b.edits().is_empty(), "nothing lands while the finger is down");
    let took = b.step(vec![touch(1, TouchPhase::End, at)]);
    assert!(!took.hold);
    let edits = b.edits();
    assert_eq!(edits.len(), 1, "one lift, one undo step");
    let [CadEdit::Placement { id: 2, placement: Placement::Ring { theta_deg, .. } }] = edits[0].0.as_slice() else { panic!("{:?}", edits[0]) };
    // 48 pt along the arrow at some 16 pt a millimetre on a 10.5 mm crest turns the post by about 16°.
    assert!((theta_deg - 90.0).abs() > 5.0 && (theta_deg - 90.0).abs() < 30.0, "{theta_deg}");
}

#[test]
fn a_stone_on_a_plates_face_slides_by_its_seat_under_the_finger() {
    use ringdesign_core::cad::{FaceSeat, builders};
    let mut d = court();
    let mut history = History::new(&d);
    let (edits, plate) = touch::parts::part_here(&d, "Box", 90.0, 0.0).unwrap();
    commit(&mut d, &mut history, &edits, None).unwrap();
    let b = Bench::new(d.clone());
    let c = b.built.evaluated().unwrap().components.iter().find(|c| c.id == plate).cloned().unwrap();
    // The plate's top: the planar face turned most nearly along the part's own z.
    let outward = |f: u32| FaceSeat::on(&c, f, None, 0.0).ok().and_then(|s| s.face_of(&c).ok()).map(|fr| (0..3).map(|k| fr.normal[k] * c.frame.z_axis[k]).sum::<f64>());
    let top = (0..c.body.faces.len() as u32).filter_map(|f| outward(f).map(|w| (f, w))).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
    let (edits, stone) = touch::parts::stone_on_face(&d, b.built.evaluated(), plate, top, None, "round-5").unwrap();
    commit(&mut d, &mut history, &edits, None).unwrap();
    let mut b = Bench::new(d);
    b.cad.choose(stone);
    let features = b.d.cad.as_ref().unwrap().features.len();
    let seat_of = |op: &Operation| match op {
        Operation::Builder { params, .. } => FaceSeat::of(params).ok().flatten(),
        _ => None,
    };
    let before = seat_of(&b.d.cad.as_ref().unwrap().feature(stone).unwrap().operation).expect("seated on the plate");
    let (on, out) = b.arrow(Handle::Move(Axis::X));
    b.step(vec![touch(1, TouchPhase::Start, on)]);
    let mut at = on;
    for _ in 0..6 {
        at += out * 8.0;
        b.step(vec![touch(1, TouchPhase::Move, at)]);
    }
    b.step(vec![touch(1, TouchPhase::End, at)]);
    let edits = b.edits();
    assert_eq!(edits.len(), 1, "one lift, one undo step");
    // The seat slides along the face: its operation is edited, nothing is wrapped in a Transform.
    let [CadEdit::Operation { id, operation }] = edits[0].0.as_slice() else { panic!("{:?}", edits[0]) };
    assert!(*id == stone && matches!(operation, Operation::Builder { key, .. } if key == builders::STONE));
    let seat = seat_of(operation).expect("still on the face");
    assert_eq!(seat.face.ordinal, before.face.ordinal);
    assert!((seat.u_mm - before.u_mm).abs() > 0.2 && (seat.v_mm - before.v_mm).abs() < 1e-6 && seat.height_mm == before.height_mm, "{before:?} then {seat:?}");
    assert_eq!(b.d.cad.as_ref().unwrap().features.len(), features);
}

#[test]
fn the_gizmo_keeps_its_parts_own_reach_while_a_moved_placement_waits_for_its_rebuild() {
    let mut b = Bench::new(posted());
    b.cad.choose(2);
    let reach = |b: &Bench| {
        let c = command::Ctx { rect: RECT, camera: &b.camera, design: &b.d, build: Some(&b.built), band: b.cad.band(), field: None, pins: &[], selection: &b.cad.selection, gizmo: true };
        command::gizmo_of(&c, &b.cad.live).expect("the post is chosen").1.reach_mm
    };
    let before = reach(&b);
    // A 1.2 x 2 mm post reaches about 2.3 mm from its seat.
    assert!(before > 1.5 && before < 3.0, "{before}");
    // The move lands in the design while the build on screen still stands the post at 90°.
    let mut history = History::new(&b.d);
    commit(&mut b.d, &mut history, &[CadEdit::Placement { id: 2, placement: Placement::ring(150.0, 0.0) }], None).unwrap();
    let after = reach(&b);
    assert!((after - before).abs() < 1e-9, "read about the new seat it would span the 60° between them: {before} then {after}");
}

#[test]
fn a_second_finger_lets_the_handle_go_and_nothing_moves() {
    let mut b = Bench::new(posted());
    b.tap(b.post());
    let (on, out) = b.arrow(Handle::Move(Axis::Theta));
    b.step(vec![touch(1, TouchPhase::Start, on)]);
    b.step(vec![touch(1, TouchPhase::Move, on + out * 30.0)]);
    let took = b.step(vec![touch(2, TouchPhase::Start, on + vec2(0.0, 120.0))]);
    assert!(!took.hold, "the pinch is the camera's");
    assert!(!b.cad.live.is_live());
    b.step(vec![touch(1, TouchPhase::Move, on + out * 60.0), touch(2, TouchPhase::Move, on + vec2(0.0, 160.0))]);
    let took = b.step(vec![touch(1, TouchPhase::End, on + out * 60.0), touch(2, TouchPhase::End, on + vec2(0.0, 160.0))]);
    assert!(took.tap, "the pinch's lift is no tap");
    let asked = b.cad.take_requests();
    assert!(!asked.iter().any(|r| matches!(r, Request::Edit { .. })), "{asked:?}");
    assert!(asked.iter().any(|r| matches!(r, Request::Status(s) if s.starts_with("Two fingers"))));
    let (_, model, _) = b.renderer.lock().unwrap().preview_state();
    assert!(model.is_none(), "the ghost is gone");
}

#[test]
fn a_lift_in_a_pass_that_drew_another_tab_never_turns_the_next_tap_into_a_long_press() {
    let mut b = Bench::new(posted());
    let at = b.post();
    b.step(vec![touch(1, TouchPhase::Start, at)]);
    // The finger lifts in a pass that draws another tab, so the CAD layer never sees it.
    b.time += 0.1;
    let mut out = b.ctx.run_ui(egui::RawInput { events: vec![touch(1, TouchPhase::End, at)], time: Some(b.time), ..Default::default() }, |_| {});
    out.textures_delta.clear();
    // Back on the ring half a minute later, the same finger id taps the post.
    b.time += 30.0;
    let took = b.tap(at);
    assert!(took.tap && !took.long_press && b.cad.menu.is_none(), "a tap, not the stale press's long press");
    assert_eq!(b.cad.selection.items, [Sel::Part(2)]);
}

#[test]
fn the_commands_bars_stand_away_from_the_finger_and_never_overlap_even_when_the_keyboard_shortens_the_view() {
    let mut b = Bench::new(posted());
    b.tap(b.post());
    let (on, _) = b.arrow(Handle::Move(Axis::Theta));
    b.tap(on);
    // The bar's first frame measures it; the next stands it on that height.
    b.step(Vec::new());
    b.step(Vec::new());
    let areas = |b: &Bench| {
        let [_, caption, fields, _, _, _] = super::areas().map(|id| b.ctx.memory(|m| m.area_rect(id)));
        (caption.expect("the caption is drawn"), fields.expect("the fields are drawn"))
    };
    let (caption, fields) = areas(&b);
    assert!(!caption.intersects(fields), "{caption:?} against {fields:?}");
    assert!(RECT.contains_rect(caption) && RECT.contains_rect(fields));
    let away = if on.y > RECT.center().y { caption.max.y < RECT.center().y } else { caption.min.y > RECT.center().y };
    assert!(away, "the caption at {caption:?} stands away from the finger at {on:?}");
    // The keyboard rises: the view keeps its top and loses 400 points beneath.
    b.rect = egui::Rect::from_min_size(RECT.min, vec2(420.0, 200.0));
    b.step(Vec::new());
    b.step(Vec::new());
    let (caption, fields) = areas(&b);
    assert!(!caption.intersects(fields), "{caption:?} against {fields:?}");
    assert!(b.rect.contains_rect(caption) && b.rect.contains_rect(fields), "{caption:?} and {fields:?} in {:?}", b.rect);
    assert!(b.cad.live.is_live(), "the command still waits for its number");
}

#[test]
fn a_tapped_arrow_waits_for_a_typed_value_and_enter_commits_it() {
    let mut b = Bench::new(posted());
    b.tap(b.post());
    let (on, _) = b.arrow(Handle::Move(Axis::Theta));
    b.tap(on);
    assert!(b.cad.live.is_live(), "a tap on the arrow leaves its command waiting for a number");
    assert!(b.edits().is_empty());
    // The field has the keyboard now: the digits go into it and Enter confirms.
    b.step(Vec::new());
    b.step(vec![Event::Text("12".into())]);
    b.step(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    let edits = b.edits();
    let [CadEdit::Placement { id: 2, placement: Placement::Ring { theta_deg, .. } }] = edits[0].0.as_slice() else { panic!("{edits:?}") };
    assert!((theta_deg - 102.0).abs() < 1e-9, "typed 12° round the ring from 90: {theta_deg}");
    assert!(!b.cad.live.is_live());
}

#[test]
fn a_long_press_opens_the_posts_menu_and_its_rows_serve_attach_and_refuse_what_they_must() {
    let mut b = Bench::new(posted());
    let at = b.post();
    b.step(vec![touch(1, TouchPhase::Start, at)]);
    let mut long_press = false;
    for _ in 0..5 {
        long_press |= b.step(Vec::new()).long_press;
    }
    assert!(long_press);
    let menu = b.cad.menu.clone().expect("the menu is open");
    assert!(menu.heading.as_deref().is_some_and(|h| h.starts_with("Face") && h.ends_with("of Cylinder")), "{:?}", menu.heading);
    assert!(menu.items.iter().any(|i| matches!(i.action, MenuAction::PressPull { feature: 2, .. }) && i.enabled));
    let took = b.step(vec![touch(1, TouchPhase::End, at)]);
    assert!(took.tap, "the lift after a long press is no tap");
    let d = b.d.clone();
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.act(&v, MenuAction::Attach(2, Attach::Cut));
    b.cad.act(&v, MenuAction::Pattern { feature: 2, key: keys::MIRROR_BAND });
    b.cad.act(&v, MenuAction::SketchOnPlane { theta_deg: 0.0, across_mm: 0.0 });
    let asked = b.cad.take_requests();
    let edits: Vec<&Vec<CadEdit>> = asked.iter().filter_map(|r| if let Request::Edit { edits, .. } = r { Some(edits) } else { None }).collect();
    assert!(matches!(edits[0].as_slice(), [CadEdit::Attach { id: 2, attach: Attach::Cut }]));
    assert_eq!(edits.len(), 1, "the post stands on the band's mid-plane, so its mirror across it is refused: {asked:?}");
    assert!(asked.iter().any(|r| matches!(r, Request::Status(s) if s.contains("stands on that plane"))));
    // A sketch on the band where it was pressed opens square to it.
    assert!(b.cad.sketching());
    assert!(asked.iter().any(|r| matches!(r, Request::Look(_))));
    assert!(asked.iter().any(|r| matches!(r, Request::Status(s) if s.starts_with("Sketching · Select"))), "{asked:?}");
}

#[test]
fn a_stone_added_on_the_band_is_drawn_with_the_stones_and_its_setting_is_built_round_it() {
    let b = Bench::new(court());
    let mut cad = Cad::default();
    cad.act(&b.view(), MenuAction::AddStone { theta_deg: 90.0, height_mm: 0.0, key: "round-6.5" });
    let Some(Request::Edit { edits, then }) = cad.take_requests().into_iter().next() else { panic!() };
    let mut d = b.d.clone();
    let mut history = History::new(&d);
    let done = commit(&mut d, &mut history, &edits, None).unwrap().unwrap();
    assert_eq!(done.label, "Add Procedural shank · Add Round 6.5 mm");
    assert_eq!(then, Then::Part(2));
    // The stone built, chosen, and set in four claws.
    let mut b = Bench::new(d);
    b.cad.choose(2);
    let items = menu::phone_items(ringdesign_workbench::viewport::context_items(&b.cad.selection, None, &b.d), &[]);
    let four = items.iter().find(|i| i.label == "Four claws").expect("a chosen stone offers its settings").action.clone();
    let d = b.d.clone();
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.act(&v, four);
    let (edits, then) = b.edits().remove(0);
    let names: Vec<String> = edits.iter().filter_map(|e| if let CadEdit::Add { feature, .. } = e { Some(feature.name.clone()) } else { None }).collect();
    assert_eq!(names, ["Four-claw head", "Seat bur"]);
    assert_eq!(then, Then::Part(3), "the head is chosen once it lands");
    let stone = b.built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap();
    assert!(stone.settings.reference, "the stone is a reference part");
    let mut gems = Vec::new();
    crate::ring::stones_as_parts(&b.built.0, &mut gems);
    assert_eq!(gems.len(), stone.mesh.faces.len() * 3 * 12, "it is drawn with the preview stones");
}

#[test]
fn a_measure_tap_reads_wherever_the_finger_meets_metal_and_says_so_where_it_meets_none() {
    let mut b = Bench::new(two_posts());
    b.measuring = true;
    b.camera = OrbitCamera::default();
    b.camera.fit(b.built.bounds());
    let (mut on, mut off) = (0, 0);
    // A grid over the 3/4 view above the Measure bar: through the opening and past the rim a finger meets nothing.
    for iy in 0..16 {
        for ix in 0..16 {
            let p = egui::pos2(60.0 + ix as f32 * 20.0, 150.0 + iy as f32 * 18.0);
            b.cad.measuring.measure.clear();
            b.tap(p);
            let said = b.said();
            let (o, d) = b.camera.ray(RECT, p);
            let read = b.cad.measuring.measure.picks.len();
            if ringdesign_core::interaction::picking::raycast(&b.built.0.mesh, o, d).is_some() {
                on += 1;
                assert_eq!(read, 1, "a tap on the metal at {p:?} read nothing: {said:?}");
            } else if read == 0 {
                off += 1;
                assert_eq!(said, ["Nothing under the finger to measure"], "{p:?}");
                assert!(b.cad.measuring.missed, "the bar says so at {p:?}");
            }
        }
    }
    assert!(on >= 40 && off >= 40, "{on} taps on the metal, {off} off it");
}

/// The band's surface point at `theta_deg` and `across_mm`, and where it lands on screen.
fn crest(b: &Bench, theta_deg: f64, across_mm: f64) -> ([f64; 3], Pos2) {
    let (hit, _) = ringdesign_core::cad::surface_hit(&b.built.0.mesh, theta_deg, across_mm).expect("the band is there");
    (hit, b.camera.projector(RECT).at(hit.map(|x| x as f32)))
}

fn apart(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}

#[test]
fn measure_taps_read_two_band_points_a_third_chains_to_a_corner_and_the_bars_clear_empties_it() {
    let mut b = Bench::new(posted());
    b.measuring = true;
    b.cad.measuring.free = true;
    // Twice as close, the post's end is wider than a finger's aperture and reads as its face rather than its rim.
    b.camera.zoom *= 2.0;
    let (a, pa) = crest(&b, 70.0, 0.0);
    let (c, pc) = crest(&b, 110.0, 0.0);
    let took = b.tap(pa);
    assert!(took.tap, "the tap is Measure's");
    assert_eq!(b.said(), ["From the band: tap the second"]);
    b.tap(pc);
    let r = b.cad.measuring.measure.readings();
    // 40° of a 10.5 mm crest apart, read where the fingers landed: the chord between the two hits.
    let chord = apart(a, c);
    assert!(r.len() == 1 && r[0].what == "Distance" && (r[0].distance_mm - chord).abs() < 0.01, "{r:?} against {chord}");
    assert!(chord > 7.0 && chord < 7.4, "{chord}");
    assert_eq!(b.said(), [r[0].line()]);
    // A third tap, on the post's end, chains on from the second: the plane's drop and the corner at the second point.
    b.tap(b.post());
    let r = b.cad.measuring.measure.readings();
    assert_eq!(r.iter().map(|r| r.what).collect::<Vec<_>>(), ["Distance", "To the plane", "Corner"]);
    assert!(matches!(b.cad.measuring.measure.picks[2], ringdesign_workbench::visual::measure::Picked::Face { flat: true, .. }), "the post's end is read by its plane");
    // A fourth starts again.
    b.tap(pa);
    assert_eq!(b.cad.measuring.measure.picks.len(), 1);
    // A drag turns the ring and measures nothing.
    assert!(!b.drag(pc, pc + vec2(90.0, 10.0), 4), "a drag is the ring's while measuring");
    assert_eq!(b.cad.measuring.measure.picks.len(), 1);
    // The bar's Clear empties it, and the tap on the bar measures nothing.
    let bar = b.ctx.memory(|m| m.area_rect(bar::area())).expect("the Measure bar is drawn");
    assert!(RECT.contains_rect(bar), "{bar:?}");
    b.press_button(egui::pos2(bar.left() + 40.0, bar.bottom() - 29.0));
    assert!(b.cad.measuring.measure.picks.is_empty(), "{:?}", b.cad.measuring.measure.picks);
    assert!(b.said().iter().any(|s| s == "Measurement cleared"));
    // Snapped, a band point off the crest lands on the ring's line there and is named by it.
    b.cad.measuring.free = false;
    let (off, p) = crest(&b, 70.0, 0.3);
    b.tap(p);
    let snapped = b.cad.measuring.measure.picks[0].clone();
    // The band's surface is read a tenth of a micron off the line, where its ray cannot slip between faces.
    assert!(off[2] > 0.25 && snapped.at()[2].abs() < 1e-3, "{off:?} snapped to {:?}", snapped.at());
    assert_ne!(snapped.label(), "the band");
    assert!(!b.cad.live.is_live() && b.cad.selection.items.is_empty(), "Measure chooses nothing");
}

/// The Court band with two small posts joined at 80° and 100°.
fn two_posts() -> RingDesign {
    let mut d = court();
    let mut history = History::new(&d);
    for theta in [80.0, 100.0] {
        let (edits, _) = touch::parts::part_here(&d, "Cylinder", theta, 0.0).unwrap();
        commit(&mut d, &mut history, &edits, None).unwrap();
    }
    for id in [2, 3] {
        commit(&mut d, &mut history, &[CadEdit::Operation { id, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 } }], None).unwrap();
    }
    d
}

/// Where part `id`'s middle lands on screen.
fn middle(b: &Bench, id: Id) -> Pos2 {
    let c = b.built.evaluated().unwrap().components.iter().find(|c| c.id == id).unwrap();
    let (lo, hi) = c.mesh.bounds().unwrap();
    b.camera.projector(RECT).at([(lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5, (lo.2 + hi.2) * 0.5])
}

#[test]
fn box_select_draws_a_window_or_a_crossing_by_one_finger_and_its_op_replaces_adds_or_takes_away() {
    use ringdesign_workbench::touch::boxes::BoxOp;
    let mut b = Bench::new(two_posts());
    // Close enough that a finger's box fits round one post and clear of the other.
    b.camera.zoom *= 2.5;
    let (p2, p3) = (middle(&b, 2), middle(&b, 3));
    assert!((p2 - p3).length() > 90.0, "the posts stand apart on screen: {p2:?} {p3:?}");
    let round = |p: Pos2| (p - vec2(40.0, 40.0), p + vec2(40.0, 40.0));
    // Off, a drag is the ring's.
    let (from, to) = round(p2);
    assert!(!b.drag(from, to, 4));
    assert!(b.cad.selection.items.is_empty());
    b.cad.boxing.toggle();
    // Left to right round the post at 80°: a window that holds it whole, the ring held while the finger draws.
    assert!(b.drag(from, to, 5), "the finger holds the ring while it draws the box");
    assert_eq!(b.cad.selection.items, [Sel::Part(2)]);
    assert_eq!(b.said(), ["Window box: 1 selected"]);
    // Add: a window round the other joins it.
    b.cad.boxing.op = BoxOp::Add;
    let (from, to) = round(p3);
    b.drag(from, to, 5);
    assert_eq!(b.cad.selection.items, [Sel::Part(2), Sel::Part(3)]);
    // Remove, right to left over the first post's middle: a crossing touches it and takes it out.
    b.cad.boxing.op = BoxOp::Remove;
    b.drag(p2 + vec2(10.0, 6.0), p2 - vec2(10.0, 6.0), 3);
    assert_eq!(b.cad.selection.items, [Sel::Part(3)]);
    // Replace, right to left across both middles: a crossing takes both.
    b.cad.boxing.op = BoxOp::Replace;
    let (left, right) = if p2.x < p3.x { (p2, p3) } else { (p3, p2) };
    b.drag(right + vec2(0.0, -6.0), left + vec2(0.0, 6.0), 5);
    let mut chosen = b.cad.selection.items.clone();
    chosen.sort_by_key(|s| s.feature());
    assert_eq!(chosen, [Sel::Part(2), Sel::Part(3)]);
    assert_eq!(b.said().last().map(String::as_str), Some("Crossing box: 2 selected"));
    // The same box left to right is a window that holds neither whole: Replace clears.
    b.drag(left + vec2(0.0, -6.0), right + vec2(0.0, 6.0), 5);
    assert!(b.cad.selection.items.is_empty(), "{:?}", b.cad.selection.items);
    // A second finger drops the box unfinished and the pinch is the camera's.
    b.cad.selection.click(Some(Sel::Part(2)), Mods::default());
    let (from, to) = round(p3);
    b.step(vec![touch(1, TouchPhase::Start, from)]);
    b.step(vec![touch(1, TouchPhase::Move, from.lerp(to, 0.5))]);
    assert!(!b.step(vec![touch(2, TouchPhase::Start, to + vec2(0.0, 150.0))]).hold);
    b.step(vec![touch(1, TouchPhase::End, to), touch(2, TouchPhase::End, to + vec2(0.0, 150.0))]);
    assert_eq!(b.cad.selection.items, [Sel::Part(2)], "nothing boxed");
    // With Add, a tap adds the part under it rather than walking down it.
    b.cad.boxing.op = BoxOp::Add;
    b.tap(p3);
    assert_eq!(b.cad.selection.items, [Sel::Part(2), Sel::Part(3)]);
    // While it is on, the chosen parts carry no gizmo to take the drag.
    let c = command::Ctx { rect: RECT, camera: &b.camera, design: &b.d, build: Some(&b.built), band: b.cad.band(), field: None, pins: &[], selection: &b.cad.selection, gizmo: !b.cad.boxing.on };
    assert!(command::gizmo_of(&c, &b.cad.live).is_none());
    b.cad.boxing.toggle();
    assert!(!b.drag(from, to, 4), "put away, a drag turns the ring again");
}

/// The post and a work plane through 0°, which faces a camera over the ring's top.
fn posted_with_plane() -> RingDesign {
    use ringdesign_core::cad::{Component, Feature, PlaneBase};
    let mut d = posted();
    let mut history = History::new(&d);
    let plane = Feature { id: 3, name: "Section at 0°".into(), enabled: true, operation: Operation::Plane { base: PlaneBase::Section { theta_deg: 0.0 }, offset_mm: 0.0 }, component: Component::default() };
    commit(&mut d, &mut history, &[CadEdit::Add { feature: plane, after: None }], None).unwrap();
    d
}

#[test]
fn a_work_plane_is_drawn_chosen_by_a_tap_and_held_for_its_menu_which_mirrors_the_chosen_post_across_it() {
    use ringdesign_core::cad::{MirrorPlane, PatternKind};
    let mut b = Bench::new(posted_with_plane());
    b.step(Vec::new());
    let shapes = touch::planes::shapes(&b.d, &b.built.0);
    assert_eq!(shapes.iter().map(|s| (s.id, s.name.as_str())).collect::<Vec<_>>(), [(3, "Section at 0°")]);
    let proj = b.camera.projector(RECT);
    let corners = shapes[0].corners.map(|c| proj.at(c.map(|x| x as f32)));
    // Its lower edge runs under the band on screen, clear of the post over the top.
    let edge = corners[0] + (corners[1] - corners[0]) * 0.8;
    assert!(RECT.contains(edge) && (edge - b.post()).length() > 60.0, "{edge:?}");
    // Tilted over the top, the post stands 9 points from the plane's lower edge on screen, and the part under the finger outranks the outline.
    let gap = (b.post().y - corners[0].y).abs();
    assert!(gap < touch::planes::REACH_PT, "{gap}");
    b.tap(b.post());
    assert_eq!(b.cad.selection.items, [Sel::Part(2)]);
    assert_eq!(b.cad.planes.chosen, None);
    b.said();
    assert!(b.tap(edge).tap);
    assert_eq!(b.cad.planes.chosen, Some(3));
    assert_eq!(b.cad.selection.items, [Sel::Part(2)], "choosing a plane leaves the parts as they were");
    assert!(b.said().last().is_some_and(|s| s.starts_with("Work plane Section at 0°")));
    // Held, it opens its own menu: a sketch on it, the mirror offered, hiding.
    assert!(b.hold(edge));
    let m = b.cad.planes.menu.clone().expect("the plane's menu is open");
    assert!(b.cad.menu.is_none(), "not the ring's menu");
    assert_eq!(m.heading, "Work plane: Section at 0°");
    let rows: Vec<(&str, bool)> = m.rows.iter().map(|r| (r.label, r.enabled)).collect();
    assert_eq!(rows, [("Sketch on this plane", true), ("Mirror the chosen part across it", true), ("Hide work planes", true)]);
    assert_eq!(m.rows[1].hint, "Cylinder reflected across Section at 0°, one new Mirror feature");
    let drawn = b.ctx.memory(|mem| mem.area_rect(menu::area())).expect("the menu is drawn");
    assert!(RECT.contains_rect(drawn), "{drawn:?}");
    // A press on the ring beside it closes it and does nothing else.
    b.tap(b.post());
    assert!(b.cad.planes.menu.is_none() && b.cad.selection.items == [Sel::Part(2)]);
    // Its mirror row: one Mirror feature of the post across the plane.
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &b.lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.plane_act(&v, 3, planes::Act::Mirror);
    let requests = b.cad.take_requests();
    let Some(Request::Edit { edits, then }) = requests.into_iter().next() else { panic!("the mirror is an edit") };
    let [CadEdit::Add { feature, .. }] = edits.as_slice() else { panic!("{edits:?}") };
    assert!(matches!(&feature.operation, Operation::Pattern { source: 2, kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 3 } } }), "{:?}", feature.operation);
    assert_eq!(then, Then::LastAdded);
    // Through the funnel it is one undo step, and the copy stands across y = 0 from the post, under the ring.
    let mut d = b.d.clone();
    let mut history = History::new(&d);
    let done = commit(&mut d, &mut history, &edits, b.built.evaluated()).unwrap().unwrap();
    assert_eq!(history.timeline().len(), 2, "{}", done.label);
    let mirror = done.applied.last().and_then(|a| a.id).unwrap();
    let after = Bench::new(d);
    let centre = |id: Id| {
        let c = after.built.evaluated().unwrap().components.iter().find(|c| c.id == id).unwrap();
        let n = c.mesh.vertices.len() as f64;
        c.mesh.vertices.iter().fold([0.0; 3], |s, v| [s[0] + f64::from(v.0) / n, s[1] + f64::from(v.1) / n, s[2] + f64::from(v.2) / n])
    };
    let (post, copy) = (centre(2), centre(mirror));
    assert!((post[0] - copy[0]).abs() < 1e-3 && (post[1] + copy[1]).abs() < 1e-3 && (post[2] - copy[2]).abs() < 1e-3 && post[1] > 9.0, "{post:?} {copy:?}");
    // The copy follows its source: chosen, it carries no gizmo of its own, where the post does.
    let mut after = after;
    for (id, has) in [(mirror, false), (2, true)] {
        after.cad.choose(id);
        let c = command::Ctx { rect: RECT, camera: &after.camera, design: &after.d, build: Some(&after.built), band: after.cad.band(), field: None, pins: &[], selection: &after.cad.selection, gizmo: true };
        assert_eq!(command::gizmo_of(&c, &after.cad.live).is_some(), has, "#{id}");
    }
    // Hidden, it is neither drawn nor taken, and the app is asked to keep the switch.
    b.cad.plane_act(&v, 3, planes::Act::Hide);
    assert!(b.cad.planes.hidden && b.cad.planes.chosen.is_none());
    assert!(b.cad.take_requests().iter().any(|r| matches!(r, Request::Prefs)));
    b.tap(edge);
    assert_eq!(b.cad.planes.chosen, None);
    b.hold(edge);
    assert!(b.cad.planes.menu.is_none(), "a hold there never opens the plane's menu");
    // With no part chosen the mirror waits for one, and the plane is never its own mirror.
    b.cad.clear();
    let rows = planes::rows(&b.d, &b.cad.selection, 3);
    assert!(!rows[1].enabled && rows[1].hint == "Choose a part on the ring first, then hold the plane");
    b.cad.selection.click(Some(Sel::Part(3)), Mods::default());
    assert!(planes::mirrorable(&b.d, &b.cad.selection, 3).is_err());
}

/// The planar face of part `id` facing out along +y, the ring's top.
fn top_face(built: &Built, id: Id) -> u32 {
    let c = built.evaluated().unwrap().components.iter().find(|c| c.id == id).unwrap();
    (0..c.trace.face_kind.len() as u32).find(|f| ringdesign_core::cad::pattern::planar_face(c, *f).is_ok_and(|(_, _, n)| n[1] > 0.99)).expect("the part has a face on top")
}

#[test]
fn a_sketch_on_the_posts_end_squares_the_view_draws_by_taps_and_extrudes_as_one_undo_step() {
    use ringdesign_workbench::{sketch_tools::Tool, touch::sketch::Make};
    let mut b = Bench::new(posted());
    b.step(Vec::new());
    let face = top_face(&b.built, 2);
    // Held, the post's end offers a sketch on it and a work plane on it.
    assert!(b.hold(b.post()));
    let m = b.cad.menu.clone().expect("the post's menu is open");
    assert!(m.items.iter().any(|i| i.enabled && i.action == MenuAction::SketchOnFace { feature: 2, face }));
    assert!(m.extras.iter().any(|x| x.enabled && x.act == menu::Extra::PlaneOnFace { feature: 2, face }), "{:?}", m.extras);
    // Its row chosen, the popup is gone by the time a finger comes back to the ring.
    b.cad.menu = None;
    b.step(Vec::new());
    b.cad.take_requests();
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let lib = AlphaLibrary::builtin();
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.act(&v, MenuAction::SketchOnFace { feature: 2, face });
    assert!(b.cad.sketching());
    // The view turns square to the face, its normal toward the eye.
    let look = b.cad.take_requests().into_iter().find_map(|r| if let Request::Look(p) = r { Some(p) } else { None }).expect("the camera turns");
    b.camera.set_pose(look);
    let frame = *b.cad.sketch().unwrap().pad.frame().expect("the face's plane is read");
    let (_, forward) = b.camera.ray(RECT, RECT.center());
    assert!((0..3).map(|k| f64::from(forward[k]) * frame.n[k]).sum::<f64>() < -0.999, "looking straight at the face: {forward:?}");
    let proj = b.camera.projector(RECT);
    let centre = proj.at(frame.origin.map(|x| x as f32));
    assert!((centre - RECT.center()).length() < 1.0, "the face mid-view: {centre:?}");
    // Two taps through the camera draw a 1 × 1 mm square on the face; each tap is the sketch's.
    let screen = |uv: [f64; 2]| proj.at(frame.point(uv).map(|x| x as f32));
    b.cad.sketch_mut().unwrap().pad.set_tool(Tool::Rectangle);
    assert!(b.tap(screen([-0.5, -0.5])).tap);
    b.tap(screen([0.5, 0.5]));
    assert_eq!(b.cad.sketch().unwrap().pad.working.entities.len(), 4);
    let said = b.said();
    assert!(said.iter().any(|s| s.starts_with("Rectangle 1.000 × 1.000")), "{said:?}");
    // A finger held on the ring keeps the view from turning; a drag from nothing in Select pans it.
    b.cad.sketch_mut().unwrap().pad.set_tool(Tool::Select);
    assert!(b.drag(screen([-1.2, -1.0]), screen([-0.9, -0.8]), 3), "every step of the drag is the sketch's");
    let pans = b.cad.take_requests().into_iter().filter(|r| matches!(r, Request::Pan { .. })).count();
    assert!(pans >= 3, "{pans} pans");
    // Finish finds the region; the view tilts to watch an extrusion rise.
    let pad = &mut b.cad.sketch_mut().unwrap().pad;
    assert!(matches!(pad.finish(), ringdesign_workbench::sketch_tools::Outcome::Edited(_)));
    pad.make(Make::Extrude);
    b.step(Vec::new());
    let tilt = b.cad.take_requests().into_iter().find_map(|r| if let Request::Look(p) = r { Some(p) } else { None }).expect("the camera tilts");
    b.camera.set_pose(tilt);
    // Its arrow rises under the finger by all the finger moved along it, the slop before the drag included.
    let (grab, lift) = {
        let v = View { rect: RECT, camera: &b.camera, design: &b.d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
        let (foot, tip) = Cad::arrow(b.cad.sketch_mut().unwrap(), &v, sketch::px_per_mm(&v)).expect("the arrow stands on the extrusion");
        let grab = foot.lerp(tip, 0.5);
        (grab, grab + (tip - foot).normalized() * 40.0)
    };
    let reach = |b: &mut Bench, p: Pos2| {
        let (o, d) = b.camera.ray(RECT, p);
        let pad = &mut b.cad.sketch_mut().unwrap().pad;
        let (n, foot) = (pad.frame().unwrap().n, pad.anchor().unwrap());
        ringdesign_workbench::command::along_line(ringdesign_core::interaction::pick::Ray { origin: o.map(f64::from), direction: d.map(f64::from) }, foot, n).unwrap()
    };
    let want = reach(&mut b, lift) - reach(&mut b, grab);
    let before = b.cad.sketch().unwrap().pad.height_mm();
    assert!(b.drag(grab, lift, 4), "the arrow holds the view");
    let rose = b.cad.sketch().unwrap().pad.height_mm() - before;
    assert!(want > 0.2 && (rose - want).abs() <= 0.026, "rose {rose:.3} mm under a finger that moved {want:.3} mm along the arrow");
    // Typed 0.6 high, the extrusion leaves as one funnel commit.
    b.cad.sketch_mut().unwrap().pad.typed("height", 0.6);
    b.cad.sketch_commit(&v);
    let Some(Request::Edit { edits, then }) = b.cad.take_requests().into_iter().find(|r| matches!(r, Request::Edit { .. })) else { panic!("finishing is an edit") };
    assert_eq!(then, Then::LastAdded);
    assert!(b.cad.sketching(), "the sketch stays until its edit lands");
    let mut d = b.d.clone();
    let mut history = History::new(&d);
    let done = commit(&mut d, &mut history, &edits, b.built.evaluated()).unwrap().unwrap();
    assert_eq!(done.label, "Add Sketch · Add Extrude");
    assert_eq!(history.timeline().len(), 2, "one undo step");
    // Landed, the sketch closes and the camera turns back to where it opened; a post shown alone keeps the view, and the new solid joins it.
    b.cad.isolated = vec![2];
    b.cad.edit_landed(true);
    assert!(!b.cad.sketching());
    assert_eq!(b.cad.isolated, [2]);
    assert!(b.cad.take_requests().iter().any(|r| matches!(r, Request::Look(p) if *p == camera.pose())), "back to the view the sketch opened from");
    let extrude = done.applied.last().and_then(|a| a.id).unwrap();
    let added: Vec<Id> = done.applied.iter().filter_map(|a| a.id).collect();
    assert_eq!(touch::isolate::after_edit(&b.cad.isolated, d.cad.as_ref().unwrap(), &added), [2, extrude], "the sketch stays out of the view, the extrusion joins it");
    // Built, the square stands 0.6 mm proud of the post's end: 0.6 mm³ of its own, joined into the ring.
    let after = Bench::new(d);
    let c = after.built.evaluated().unwrap().components.iter().find(|c| c.id == extrude).expect("the extrusion built");
    assert!((c.mesh.volume_mm3() - 0.6).abs() < 1e-3, "{}", c.mesh.volume_mm3());
    let grew = after.built.volume_mm3() - b.built.volume_mm3();
    assert!((grew - 0.6).abs() < 0.01, "{grew}");
}

#[test]
fn a_work_plane_by_touch_waits_on_the_face_for_its_offset_and_a_part_is_asked_to_stand_alone() {
    let mut b = Bench::new(posted());
    b.step(Vec::new());
    let face = top_face(&b.built, 2);
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let lib = AlphaLibrary::builtin();
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    // The Actions button's menu for a chosen face offers the plane too, and a curved face refuses one by name.
    let chosen = Sel::Face { feature: 2, face };
    assert!(menu::extras(None, Some(&chosen), b.built.evaluated(), &[]).iter().any(|x| x.enabled && x.act == menu::Extra::PlaneOnFace { feature: 2, face }));
    let post = b.built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap();
    let side = (0..post.trace.face_kind.len() as u32).find(|f| post.trace.face_kind[*f as usize] == ringdesign_core::cad::SurfaceKind::Cylinder).unwrap();
    let curved = menu::extras(None, Some(&Sel::Face { feature: 2, face: side }), b.built.evaluated(), &[]);
    assert!(!curved[0].enabled && curved[0].hint.ends_with("is a cylinder; a work plane lies on a flat face"), "{:?}", curved[0].hint);
    b.cad.extra(&v, menu::Extra::PlaneOnFace { feature: 2, face });
    assert!(b.cad.live.is_live());
    assert_eq!(b.cad.live.session.command().map(|c| c.key()), Some("work-plane"));
    assert_eq!(b.cad.live.session.dimensions()[0].key, "offset");
    let prompt = b.cad.live.session.prompt();
    assert!(prompt.starts_with("Work plane: drag the arrow") && b.said().contains(&prompt), "the status line says what the bar does: {prompt}");
    // Typed and done, it is one new Plane feature on the post's end, 0.4 mm off it.
    b.cad.live.session.feed(ringdesign_workbench::command::StepInput::Typed { key: "offset", value: 0.4 });
    let ringdesign_workbench::command::Outcome::Commit(effects) = b.cad.live.session.enter() else { panic!("Done makes the plane") };
    let (edits, _) = touch::parts::effect_edits(&b.d, effects);
    let [CadEdit::Add { feature, .. }] = edits.as_slice() else { panic!("{edits:?}") };
    assert!(matches!(&feature.operation, Operation::Plane { base: ringdesign_core::cad::PlaneBase::Face { feature: 2, .. }, offset_mm } if (*offset_mm - 0.4).abs() < 1e-12));
    assert_eq!(feature.name, "On Cylinder +0.40 mm");
    // Landed, the plane is chosen on the planes layer and the part choice is left as it was; a body is chosen as a part.
    let mut landed = b.d.clone();
    let done = commit(&mut landed, &mut History::new(&b.d), &edits, None).unwrap().unwrap();
    let plane = done.applied.last().and_then(|a| a.id).unwrap();
    b.cad.selection.click(Some(chosen.clone()), Mods::default());
    b.cad.choose_made(&landed, plane);
    assert_eq!((b.cad.planes.chosen, b.cad.selection.items.clone()), (Some(plane), vec![chosen]));
    b.cad.choose_made(&landed, 2);
    assert_eq!(b.cad.selection.items, [Sel::Part(2)]);
    // At an angle through the axis it waits for the keyboard.
    b.cad.extra(&v, menu::Extra::PlaneAtAngle { theta_deg: 35.0 });
    assert_eq!((b.cad.live.session.dimensions()[0].key, b.cad.live.session.dimensions()[0].value), ("angle", 35.0));
    b.cad.live.cancel(&mut Vec::new());
    // Isolate in CAD asks the app to show the post alone; Show all brings the ring back.
    b.cad.act(&v, MenuAction::IsolateInCad(2));
    assert!(b.cad.take_requests().iter().any(|r| matches!(r, Request::Isolate(Some(2)))));
    b.cad.isolated = vec![2];
    assert!(menu::extras(None, None, b.built.evaluated(), &b.cad.isolated).iter().any(|x| x.act == menu::Extra::ShowAll));
    b.cad.extra(&v, menu::Extra::ShowAll);
    assert!(b.cad.take_requests().iter().any(|r| matches!(r, Request::Isolate(None))));
}

/// Opens a sketch on the post's end as its menu row does, the camera squared to it; the plane's frame.
fn sketch_on_end(b: &mut Bench) -> touch::sketch::Frame {
    b.step(Vec::new());
    let face = top_face(&b.built, 2);
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let lib = AlphaLibrary::builtin();
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.act(&v, MenuAction::SketchOnFace { feature: 2, face });
    let look = b.cad.take_requests().into_iter().find_map(|r| if let Request::Look(p) = r { Some(p) } else { None }).expect("the camera turns");
    b.camera.set_pose(look);
    *b.cad.sketch().unwrap().pad.frame().expect("the face's plane is read")
}

/// Where plane point `uv` of `frame` lands on the bench's screen.
fn on_screen(b: &Bench, frame: &touch::sketch::Frame, uv: [f64; 2]) -> Pos2 {
    b.camera.projector(b.rect).at(frame.point(uv).map(|x| x as f32))
}

/// Where the sketch's `k`th tool button stands: eight a row in the bench's 420-point view, 47 points apart inside the popup's margin.
fn tool_button(b: &Bench, k: usize) -> Pos2 {
    let r = sketch::drawn_rect(&b.ctx, sketch::tools_area()).expect("the tools are drawn");
    egui::pos2(r.left() + 5.0 + (k % 8) as f32 * 47.0 + 22.0, r.top() + 5.0 + (k / 8) as f32 * 47.0 + 22.0)
}

#[test]
fn the_sketch_bar_offers_every_tool_and_a_finger_picks_one_then_chamfers_a_corner_by_tap_and_keyboard() {
    use ringdesign_workbench::sketch_tools::Tool;
    let mut b = Bench::new(posted());
    let frame = sketch_on_end(&mut b);
    let listed: Vec<String> = sketch::tool_buttons(&b.cad.sketch().unwrap().pad, true).into_iter().map(|t| t.name).collect();
    let tools = ["Select tool", "Line tool", "Rectangle tool", "Circle tool", "Arc tool", "Trim tool", "Offset tool", "Fillet corner tool", "Chamfer corner tool", "Mirror tool", "Dimension tool", "Erase tool"];
    assert_eq!(listed[..12], tools);
    assert_eq!(listed[12..], ["Undo sketch edit", "Redo sketch edit", "Construction", "Look at the sketch"], "a face's sketch has no plane to switch");
    // A square on the post's end by two taps.
    b.cad.sketch_mut().unwrap().pad.set_tool(Tool::Rectangle);
    b.tap(on_screen(&b, &frame, [-0.5, -0.5]));
    b.tap(on_screen(&b, &frame, [0.5, 0.5]));
    assert_eq!(b.cad.sketch().unwrap().pad.working.entities.len(), 4);
    // Each new tool is a thumb's tap on the bar, and the tap is the bar's, not the plane's.
    for (k, tool) in [(5, Tool::Trim), (6, Tool::Offset), (9, Tool::Mirror), (8, Tool::Chamfer)] {
        b.press_button(tool_button(&b, k));
        assert_eq!(b.cad.sketch().unwrap().pad.tools.tool, tool);
    }
    assert_eq!(b.cad.sketch().unwrap().pad.working.entities.len(), 4, "no tap reached the plane");
    // A tap on the corner takes it and gives the distance field the keyboard; typed and entered, the corner is cut.
    b.tap(on_screen(&b, &frame, [0.49, 0.51]));
    assert!(b.cad.sketch().unwrap().pad.tools.busy());
    b.step(Vec::new());
    b.step(vec![Event::Text("0.2".into())]);
    b.step(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    let pad = &mut b.cad.sketch_mut().unwrap().pad;
    assert_eq!(pad.working.entities.len(), 5, "four sides and the chamfer");
    let area = pad.regions().unwrap()[0].area();
    assert!((area - (1.0 - 0.02)).abs() < 1e-9, "a 0.2 mm chamfer takes 0.02 mm² off the square: {area}");
    assert!(b.said().iter().any(|s| s == "Chamfer 0.200 mm"));
}

#[test]
fn a_sketch_finishes_joined_cut_or_apart_and_a_cut_carves_the_post_it_stands_on_as_one_undo_step() {
    use ringdesign_core::cad::Attach;
    use ringdesign_workbench::{sketch_tools::Tool, touch::sketch::Make};
    let mut b = Bench::new(posted());
    let frame = sketch_on_end(&mut b);
    b.cad.sketch_mut().unwrap().pad.set_tool(Tool::Rectangle);
    b.tap(on_screen(&b, &frame, [-0.5, -0.5]));
    b.tap(on_screen(&b, &frame, [0.5, 0.5]));
    let (d, built) = (b.d.clone(), b.built.clone());
    let lib = AlphaLibrary::builtin();
    let camera = b.camera;
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.serve_sketch(&v, vec![sketch::Ask::Finish, sketch::Ask::Make(Make::Extrude)]);
    // The view tilts to watch the solid rise, which is how its arrow is seen.
    b.step(Vec::new());
    let tilt = b.cad.take_requests().into_iter().find_map(|r| if let Request::Look(p) = r { Some(p) } else { None }).expect("the camera tilts");
    b.camera.set_pose(tilt);
    let camera = b.camera;
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    let row = |b: &mut Bench, design: &RingDesign| sketch::stage_buttons(b.cad.sketch_mut().unwrap(), design).into_iter().map(|(x, _)| (x.label, x.checked, x.enabled)).collect::<Vec<_>>();
    assert_eq!(row(&mut b, &d), [("Join", true, true), ("Cut", false, true), ("Separate", false, true), ("Extrude", false, true), ("Back", false, true)]);
    let (_, joined_tip) = Cad::arrow(b.cad.sketch_mut().unwrap(), &v, sketch::px_per_mm(&v)).unwrap();
    // Cut: ticked, and the arrow turns to run into the post.
    b.cad.serve_sketch(&v, vec![sketch::Ask::Attach(Attach::Cut)]);
    assert_eq!(row(&mut b, &d)[..3], [("Join", false, true), ("Cut", true, true), ("Separate", false, true)]);
    assert!(b.said().iter().any(|s| s.starts_with("Cut: the solid carves")));
    let (foot, cut_tip) = Cad::arrow(b.cad.sketch_mut().unwrap(), &v, sketch::px_per_mm(&v)).unwrap();
    assert!((cut_tip - foot).dot(joined_tip - foot) < 0.0, "the arrow points the other way: {foot:?} {cut_tip:?} {joined_tip:?}");
    // A ring of parts alone offers no cut.
    let mut parts_only = d.clone();
    parts_only.cad.as_mut().unwrap().features.retain(|f| !matches!(f.operation, Operation::Band));
    assert_eq!(row(&mut b, &parts_only)[1], ("Cut", true, false));
    // Typed 0.4 deep, the cut leaves as one funnel commit and carves 1 × 1 × 0.4 mm out of the post's end.
    b.cad.serve_sketch(&v, vec![sketch::Ask::Typed("height", 0.4)]);
    b.cad.sketch_commit(&v);
    let Some(Request::Edit { edits, then }) = b.cad.take_requests().into_iter().find(|r| matches!(r, Request::Edit { .. })) else { panic!("finishing is an edit") };
    assert_eq!(then, Then::LastAdded);
    let mut d2 = b.d.clone();
    let mut history = History::new(&d2);
    let done = commit(&mut d2, &mut history, &edits, b.built.evaluated()).unwrap().unwrap();
    assert_eq!(done.label, "Add Sketch · Add Extrude cut");
    assert_eq!(history.timeline().len(), 2, "one undo step");
    let after = Bench::new(d2);
    assert_eq!(after.built.0.parts.cut, 1, "{:?}", after.built.0.parts.notes);
    let taken = b.built.volume_mm3() - after.built.volume_mm3();
    assert!((taken - 0.4).abs() < 0.005, "1 × 1 × 0.4 = 0.4 mm³ out of the post: {taken}");
}

#[test]
fn the_sketch_tools_bar_and_fields_never_overlap_as_the_keyboard_shortens_the_view() {
    use ringdesign_workbench::sketch_tools::Tool;
    let mut b = Bench::new(posted());
    let frame = sketch_on_end(&mut b);
    // A rectangle's first corner: the bar offers Done and the fields take its width and height.
    b.cad.sketch_mut().unwrap().pad.set_tool(Tool::Rectangle);
    b.tap(on_screen(&b, &frame, [-0.5, -0.5]));
    assert!(b.cad.sketch().unwrap().pad.tools.busy());
    for (tall, tools) in [(600.0, true), (400.0, true), (300.0, true), (240.0, true), (200.0, false)] {
        b.rect = egui::Rect::from_min_size(RECT.min, vec2(420.0, tall));
        b.step(Vec::new());
        b.step(Vec::new());
        let bar = sketch::drawn_rect(&b.ctx, bar::area()).expect("the bar is drawn");
        let fields = sketch::drawn_rect(&b.ctx, sketch::fields_area()).expect("the fields are drawn");
        let drawn = sketch::drawn_rect(&b.ctx, sketch::tools_area());
        assert_eq!(drawn.is_some(), tools, "tools in a {tall}-point view");
        let all: Vec<egui::Rect> = [bar, fields].into_iter().chain(drawn).collect();
        for (i, a) in all.iter().enumerate() {
            assert!(b.rect.contains_rect(*a), "{a:?} in a {tall}-point view");
            for c in &all[i + 1..] {
                assert!(!a.intersects(*c), "{a:?} against {c:?} in a {tall}-point view");
            }
        }
        // Wrapped rows on a tall view, one scrolling row once the keyboard is up.
        if let Some(t) = drawn {
            assert!(if tall < sketch::ROW_VIEW_PT { t.height() < 70.0 } else { t.height() > 90.0 }, "{t:?} in a {tall}-point view");
        }
    }
}

/// The Court band with a 4 × 14 mm plate joined at its top and a 1.5 mm stone on the plate in four claws, the head #4.
fn plate_with_head() -> RingDesign {
    use ringdesign_core::{
        cad::{Attach, Component, ComponentRole, Document, FaceSeat, Feature, builders},
        gem::{Gem, GemCut},
    };
    let mut d = court();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
    let plate = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.65), ..Component::default() };
    doc.append(Feature { id: 2, name: "Plate".into(), enabled: true, operation: Operation::Box { size: [4.0, 14.0, 1.5] }, component: plate }).unwrap();
    d.cad = Some(doc.clone());
    let (built, _, _) = build(&d);
    let host = built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap();
    let outward = |f: u32| FaceSeat::on(host, f, None, 0.0).ok().and_then(|s| s.face_of(host).ok()).map(|fr| (0..3).map(|k| fr.normal[k] * host.frame.z_axis[k]).sum::<f64>());
    let top = (0..host.body.faces.len() as u32).filter_map(|f| outward(f).map(|w| (f, w))).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
    let gem = Gem::calibrated(GemCut::Round, 1.5);
    let seat = FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
    doc.append(ringdesign_core::cad::stone_on_face(3, gem, 2, &seat)).unwrap();
    doc.append(builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }))).unwrap();
    d.cad = Some(doc);
    d
}

#[test]
fn an_array_ghost_draws_the_copies_it_would_leave_out_in_the_refusal_colour_and_says_which() {
    use ringdesign_workbench::command::StepInput;
    let mut b = Bench::new(plate_with_head());
    let d = b.d.clone();
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.act(&v, MenuAction::Pattern { feature: 4, key: keys::RING_ARRAY });
    assert!(b.cad.live.is_live(), "the array waits for how many");
    // Four heads 24° apart over 72°: the copies at 48° and 72° stand off the plate's 14 mm.
    b.cad.live.session.feed(StepInput::Typed { key: "count", value: 4.0 });
    b.cad.live.session.feed(StepInput::Typed { key: "span", value: 72.0 });
    b.step(Vec::new());
    let red = ringdesign_core::FaceClass::Undercut.rgb();
    let shares = |b: &Bench| {
        let r = b.renderer.lock().unwrap();
        let (verts, model, draft) = r.preview_state();
        let verts = verts.expect("the copies are staged");
        // Twelve floats a corner, three corners a face, the colour sixth to eighth.
        let faces = verts.len() / 36;
        let refused = verts.chunks(36).filter(|f| f[6..9] == red).count();
        (faces, refused, model.is_some(), draft)
    };
    let (faces, refused, placed, draft) = shares(&b);
    assert!(placed && draft, "a ghost with copies left out shades by its classes");
    assert!(faces > 0 && (refused as f64 / faces as f64 - 2.0 / 3.0).abs() < 0.01, "two copies of three refused: {refused} of {faces} faces");
    assert_eq!(b.cad.live.ghost_note(), Some("2 copies stand off the face of #2 Plate, left out: 48°, 72°"));
    // Two over 24° both stand on the plate: the ghost is plain and says nothing.
    b.cad.live.session.feed(StepInput::Typed { key: "count", value: 2.0 });
    b.cad.live.session.feed(StepInput::Typed { key: "span", value: 24.0 });
    b.step(Vec::new());
    let (faces, refused, _, draft) = shares(&b);
    assert!(faces > 0 && refused == 0 && !draft);
    assert_eq!(b.cad.live.ghost_note(), None);
}

#[test]
fn parts_shown_alone_are_offered_out_one_at_a_time_and_the_worker_builds_them_together() {
    let mut b = Bench::new(two_posts());
    b.camera.zoom *= 2.5;
    b.cad.isolated = vec![2, 3];
    // Held, post #2's menu offers taking it out of the view, and no Isolate for it.
    assert!(b.hold(middle(&b, 2)));
    let m = b.cad.menu.clone().expect("the post's menu is open");
    assert!(!m.items.iter().any(|i| matches!(i.action, MenuAction::IsolateInCad(_))), "{:?}", m.items.iter().map(|i| &i.label).collect::<Vec<_>>());
    let acts: Vec<menu::Extra> = m.extras.iter().map(|x| x.act.clone()).filter(|a| matches!(a, menu::Extra::TakeOut(_) | menu::Extra::ShowAll)).collect();
    assert_eq!(acts, [menu::Extra::TakeOut(2), menu::Extra::ShowAll]);
    b.cad.menu = None;
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &b.lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.extra(&v, menu::Extra::TakeOut(2));
    assert!(b.cad.take_requests().iter().any(|r| matches!(r, Request::TakeOut(2))));
    // The worker builds both posts alone, each its own closed solid, and leaves a part the ring does not hold out, saying so.
    let worker = crate::ring::Worker::spawn(egui::Context::default());
    let params = BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..BuildParams::default() };
    assert!(worker.dispatch(1, &d, &Arc::new(AlphaLibrary::builtin()), params, false, false, None, &[2, 3, 9], Default::default()));
    let done = (0..1000).find_map(|_| worker.poll().or_else(|| {
        std::thread::sleep(Duration::from_millis(10));
        None
    })).expect("the build lands");
    assert_eq!((done.alone.as_slice(), done.left_out.as_slice()), (&[2, 3][..], &[9][..]));
    assert!(done.alone_note.as_deref().is_some_and(|n| n.contains("#9")), "{:?}", done.alone_note);
    let own = |id: Id| built.evaluated().unwrap().components.iter().find(|c| c.id == id).unwrap().mesh.faces.len();
    assert_eq!(done.build.mesh.faces.len(), own(2) + own(3));
    let check = done.build.mesh.validate();
    assert!(check.watertight && check.boundary_edges == 0, "{check:?}");
}

#[test]
fn box_select_takes_parts_faces_edges_or_vertices_as_its_bar_says_and_a_tap_takes_the_same() {
    use ringdesign_workbench::touch::boxes::{BoxOp, Takes};
    let mut b = Bench::new(two_posts());
    b.camera.zoom *= 2.5;
    let post = b.built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap().clone();
    let (faces, edges, vertices) = (post.trace.face_kind.len(), post.edges.len(), post.trace.vertices.len());
    let (from, to) = (middle(&b, 2) - vec2(40.0, 40.0), middle(&b, 2) + vec2(40.0, 40.0));
    b.cad.boxing.toggle();
    // Faces: every face of the post the window holds whole, and the post itself not.
    b.cad.boxing.takes = Takes::Faces;
    assert!(b.drag(from, to, 5));
    assert_eq!(b.cad.selection.items, (0..faces as u32).map(|face| Sel::Face { feature: 2, face }).collect::<Vec<_>>());
    assert_eq!(b.said(), [format!("Window box of faces: {faces} selected")]);
    // Edges, then vertices.
    b.cad.boxing.takes = Takes::Edges;
    b.drag(from, to, 5);
    assert_eq!(b.cad.selection.items, (0..edges as u32).map(|edge| Sel::Edge { feature: 2, edge }).collect::<Vec<_>>());
    b.cad.boxing.takes = Takes::Vertices;
    b.drag(from, to, 5);
    assert_eq!(b.cad.selection.items, (0..vertices as u32).map(|vertex| Sel::Vertex { feature: 2, vertex }).collect::<Vec<_>>());
    // Parts: the post whole, as the box always took it.
    b.cad.boxing.takes = Takes::Parts;
    b.said();
    b.drag(from, to, 5);
    assert_eq!((b.cad.selection.items.clone(), b.said()), (vec![Sel::Part(2)], vec!["Window box: 1 selected".to_string()]));
    // A tap while box select takes faces adds the face under it, not its part.
    b.cad.boxing.takes = Takes::Faces;
    b.cad.boxing.op = BoxOp::Add;
    b.tap(middle(&b, 3));
    assert!(matches!(b.cad.selection.items[..], [Sel::Part(2), Sel::Face { feature: 3, .. }]), "{:?}", b.cad.selection.items);
    // The bar stands over the ring with what the box takes among its buttons.
    b.step(Vec::new());
    assert!(b.ctx.memory(|m| m.area_rect(bar::area())).is_some_and(|r| RECT.contains_rect(r)));
    assert_eq!(boxes::caught_words("Crossing", Takes::Edges, 4), "Crossing box of edges: 4 selected");
}

/// The Court band's surface point at `theta_deg` and `across_mm`, and its normal.
fn on_band(b: &Bench, theta_deg: f64, across_mm: f64) -> ([f64; 3], [f64; 3]) {
    ringdesign_core::cad::surface_hit(&b.built.0.mesh, theta_deg, across_mm).expect("the band is there")
}

#[test]
fn a_box_is_dragged_out_where_the_band_was_held_its_size_then_its_height_and_lands_as_one_undo_step() {
    let mut b = Bench::new(court());
    b.step(Vec::new());
    let (at, n) = on_band(&b, 90.0, 0.0);
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &b.lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    // The Add row's Box, chosen from the menu a hold on the band's top opened.
    b.cad.pressed = Some((at, n));
    b.cad.act(&v, MenuAction::AddPartHere { theta_deg: 90.0, height_mm: 0.0, label: "Box" });
    assert!(b.cad.pressed.is_none(), "the press is spent");
    assert_eq!(b.cad.live.session.command().map(|c| (c.key(), c.step())), Some(("add-box", 1)), "seated where the band was held, it waits for its size");
    let said = b.said();
    assert!(said.last().is_some_and(|s| s.starts_with("Add box on top 90.0° · parting line: drag its size out from where you pressed")), "{said:?}");
    let seat = b.camera.projector(RECT).at(at.map(|x| x as f32));
    // Looking down on the ring's top, a millimetre round the ring runs across the screen.
    let pt_per_mm = (b.camera.projector(RECT).at([at[0] as f32 + 1.0, at[1] as f32, at[2] as f32]) - seat).length();
    assert!(pt_per_mm > 10.0, "{pt_per_mm}");
    // A finger dragged 60 pt out from the seat: the half-size follows it on the view plane, in twentieths.
    assert!(b.drag(seat + vec2(4.0, 0.0), seat + vec2(60.0, 0.0), 6), "the finger holds the ring while it sizes");
    let want = |pt: f32| ((f64::from(pt / pt_per_mm) / 0.05).round() * 0.05).max(0.05);
    let dims = b.cad.live.session.dimensions();
    assert_eq!(dims[0].key, "height", "the lift went on to the height");
    let size = b.cad.live.session.preview().unwrap().operation.unwrap();
    let Operation::Box { size: [x, y, _] } = size else { panic!("{size:?}") };
    assert!((x - 2.0 * want(60.0)).abs() < 0.11 && (y - x).abs() < 1e-12, "{x} against {}", 2.0 * want(60.0));
    {
        let r = b.renderer.lock().unwrap();
        let (verts, model, draft) = r.preview_state();
        assert!(verts.is_some_and(|v| v.len() == 12 * 3 * 12) && model.is_some() && !draft, "a unit cube staged once, carried by the model matrix");
    }
    assert!(b.edits().is_empty(), "nothing lands before the height");
    // Then 30 pt up the screen stands it that tall, and the lift adds it: the plain ring's shank and the box, one undo step.
    b.drag(seat, seat - vec2(0.0, 30.0), 4);
    let edits = b.edits();
    assert_eq!(edits.len(), 1, "one lift, one edit");
    let (edits, then) = edits.into_iter().next().unwrap();
    assert_eq!(then, Then::LastAdded);
    let [CadEdit::Add { feature: shank, .. }, CadEdit::Add { feature: part, .. }] = edits.as_slice() else { panic!("{edits:?}") };
    assert!(matches!(shank.operation, Operation::Band));
    let Operation::Box { size } = part.operation else { panic!("{:?}", part.operation) };
    assert!((size[2] - want(30.0)).abs() < 0.051, "{size:?} against {}", want(30.0));
    let mut d = b.d.clone();
    let mut history = History::new(&d);
    let done = commit(&mut d, &mut history, &edits, None).unwrap().unwrap();
    assert_eq!((done.label.as_str(), history.timeline().len()), ("Add Procedural shank · Add Box", 2));
    let after = Bench::new(d);
    let c = after.built.evaluated().unwrap().components.iter().find(|c| c.id == part.id).expect("the box builds");
    let (lo, hi) = c.mesh.bounds().unwrap();
    assert!((f64::from(hi.0 - lo.0) - size[0]).abs() < 1e-3 && hi.1 > at[1] as f32, "{lo:?} {hi:?}");
    assert!(!b.cad.live.is_live());
}

#[test]
fn a_dragged_out_part_keeps_its_size_through_a_pinch_and_done_adds_it_as_it_stands() {
    let mut b = Bench::new(posted());
    b.step(Vec::new());
    let (at, n) = on_band(&b, 45.0, 0.5);
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &b.lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.pressed = Some((at, n));
    b.cad.act(&v, MenuAction::AddPartHere { theta_deg: 45.0, height_mm: 0.0, label: "Cylinder" });
    let seat = b.camera.projector(RECT).at(at.map(|x| x as f32));
    b.drag(seat, seat + vec2(0.0, 40.0), 4);
    let radius = |b: &Bench| {
        b.cad.live.session.preview().and_then(|p| p.operation).map(|op| match op {
            Operation::Cylinder { radius_mm, .. } => radius_mm,
            other => panic!("{other:?}"),
        })
    };
    let sized = radius(&b).unwrap();
    // A second finger lets the size be; the pinch is the camera's.
    b.step(vec![touch(1, TouchPhase::Start, seat + vec2(60.0, 60.0))]);
    let took = b.step(vec![touch(2, TouchPhase::Start, seat + vec2(120.0, 60.0))]);
    assert!(!took.hold);
    b.step(vec![touch(1, TouchPhase::Move, seat + vec2(40.0, 60.0)), touch(2, TouchPhase::Move, seat + vec2(150.0, 60.0))]);
    b.step(vec![touch(1, TouchPhase::End, seat + vec2(40.0, 60.0)), touch(2, TouchPhase::End, seat + vec2(150.0, 60.0))]);
    assert_eq!(radius(&b), Some(sized));
    assert!(b.cad.live.is_live() && b.edits().is_empty());
    // A typed height, then Done: the cylinder as it stands, joined beside the post.
    b.cad.live.session.feed(ringdesign_workbench::command::StepInput::Typed { key: "height", value: 1.25 });
    let o = ringdesign_workbench::touch::primitive::finish(&mut b.cad.live.session);
    let ringdesign_workbench::command::Outcome::Commit(effects) = o else { panic!("{o:?}") };
    let (edits, added) = touch::parts::effect_edits(&b.d, effects);
    let [CadEdit::Add { feature, .. }] = edits.as_slice() else { panic!("the ring has its shank already: {edits:?}") };
    assert!(added && feature.id == 3 && feature.component.attach == ringdesign_core::cad::Attach::Join);
    assert!(matches!(feature.operation, Operation::Cylinder { radius_mm, height_mm } if radius_mm == sized && height_mm == 1.25), "{:?}", feature.operation);
}

#[test]
fn a_work_plane_square_to_the_band_or_on_the_parting_plane_is_made_where_the_band_was_held() {
    use ringdesign_core::cad::PlaneBase;
    let mut b = Bench::new(court());
    b.step(Vec::new());
    let (at, n) = on_band(&b, 70.0, 0.4);
    let pick = Pick { entity: Entity::Band, world: at, normal: n, depth: 1.0, px: 0.0 };
    let rows: Vec<&str> = menu::extras(Some(&pick), None, None, &[]).iter().map(|x| x.label).collect();
    assert_eq!(rows, ["Work plane at angle…", "Work plane square to the band…", "Work plane on the parting plane…"]);
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let lib = AlphaLibrary::builtin();
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    let make = |b: &mut Bench, extra: menu::Extra, offset: f64| {
        b.cad.extra(&v, extra);
        assert_eq!(b.cad.live.session.command().map(|c| c.key()), Some("work-plane"));
        assert_eq!(b.cad.live.session.dimensions()[0].key, "offset");
        b.cad.live.session.feed(ringdesign_workbench::command::StepInput::Typed { key: "offset", value: offset });
        let ringdesign_workbench::command::Outcome::Commit(effects) = b.cad.live.session.enter() else { panic!("Done makes the plane") };
        let (edits, _) = touch::parts::effect_edits(&b.d, effects);
        let mut d = b.d.clone();
        let done = commit(&mut d, &mut History::new(&b.d), &edits, None).unwrap().unwrap();
        let id = done.applied.last().and_then(|a| a.id).unwrap();
        let op = d.cad.as_ref().unwrap().feature(id).unwrap().operation.clone();
        let after = Bench::new(d);
        let plane = *after.built.evaluated().unwrap().plane(id).expect("the plane builds");
        (done.label, plane, op)
    };
    // Square to the band where it was held, a quarter of a millimetre off it: the plain ring's first plane brings its shank.
    let (label, plane, op) = make(&mut b, menu::Extra::PlaneTangent { at, normal: n }, 0.25);
    assert_eq!(label, "Add Procedural shank · Add Tangent at 70° +0.25 mm");
    assert!(matches!(op, Operation::Plane { base: PlaneBase::Tangent { theta_deg, across_mm }, offset_mm } if (theta_deg - 70.0).abs() < 1e-3 && (across_mm - 0.4).abs() < 1e-3 && offset_mm == 0.25), "{op:?}");
    let want: [f64; 3] = std::array::from_fn(|k| at[k] + n[k] * 0.25);
    assert!((0..3).all(|k| (plane.origin[k] - want[k]).abs() < 2e-4) && (0..3).map(|k| plane.normal[k] * n[k]).sum::<f64>() > 0.9999, "{plane:?} against {want:?}");
    // On the parting plane, starting where the verdict parts the mould (0 here), raised 0.4.
    let (label, plane, _) = make(&mut b, menu::Extra::PlaneParting { at }, 0.4);
    assert_eq!(label, "Add Procedural shank · Add Parting +0.40 mm");
    assert_eq!((plane.origin, plane.normal), ([0.0, 0.0, 0.4], [0.0, 0.0, 1.0]));
}

#[test]
fn a_part_dragged_out_near_the_top_seats_on_the_top_and_the_parting_line_as_the_desktops_click_does() {
    let mut b = Bench::new(court());
    b.step(Vec::new());
    // Held 1.4° short of the top and 0.3 mm off the parting line.
    let (at, n) = on_band(&b, 88.6, 0.3);
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &b.lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.pressed = Some((at, n));
    b.cad.act(&v, MenuAction::AddPartHere { theta_deg: 88.6, height_mm: 0.0, label: "Cylinder" });
    let placed = b.cad.live.session.preview().and_then(|p| p.placement);
    let Some(Placement::Ring { theta_deg, across_mm, .. }) = placed else { panic!("{placed:?}") };
    assert!((theta_deg - 90.0).abs() < 1e-9 && across_mm.abs() < 1e-9, "{theta_deg} {across_mm}");
    let said = b.said();
    assert!(said.last().is_some_and(|s| s.starts_with("Add cylinder on top 90.0° · parting line: drag its size out")), "{said:?}");
    // The size is dragged from the snapped seat, on the band's top.
    let seat = ringdesign_workbench::touch::primitive::centre(b.cad.live.session.command().unwrap()).unwrap();
    let (top, _) = on_band(&b, 90.0, 0.0);
    assert!((0..3).all(|k| (seat[k] - top[k]).abs() < 1e-3), "{seat:?} against {top:?}");
}

#[test]
fn a_focused_dimension_field_asks_for_the_number_keypad_in_the_frame_it_holds_the_keyboard() {
    use egui_mobile::keyboard::{KeyboardKind, requested};
    let mut b = Bench::new(court());
    b.step(Vec::new());
    let (at, n) = on_band(&b, 90.0, 0.0);
    let (d, camera, built, lib) = (b.d.clone(), b.camera, b.built.clone(), AlphaLibrary::builtin());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false, switches: Default::default() };
    b.cad.pressed = Some((at, n));
    b.cad.act(&v, MenuAction::AddPartHere { theta_deg: 90.0, height_mm: 0.0, label: "Cylinder" });
    let kind = |b: &mut Bench, focus: bool| {
        b.time += 0.1;
        let input = egui::RawInput { time: Some(b.time), screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(420.0, 800.0))), ..Default::default() };
        let mut kind = KeyboardKind::Text;
        let mut out = b.ctx.run_ui(input, |ui| {
            let (_, _) = ui.allocate_exact_size(RECT.size(), egui::Sense::click_and_drag());
            b.cad.frame(ui, &v);
            b.cad.draw(ui, &v, &b.renderer);
            if focus {
                ui.ctx().memory_mut(|m| m.request_focus(egui::Id::new("phone-ring-dimensions").with("radius")));
                super::keypad(ui.ctx());
            }
            kind = requested(ui.ctx());
        });
        out.textures_delta.clear();
        kind
    };
    assert_eq!(kind(&mut b, false), KeyboardKind::Text, "nothing holds the keyboard");
    assert_eq!(kind(&mut b, true), KeyboardKind::Number, "the radius field, as a tap on it gives it the keyboard");
    assert_eq!(kind(&mut b, false), KeyboardKind::Number, "drawn again with the field still focused");
    b.ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("phone-ring-dimensions").with("radius")));
    assert_eq!(kind(&mut b, false), KeyboardKind::Text);
}

#[test]
fn the_stamp_window_follows_its_stamp_across_undo_and_redo() {
    let disc = |name: &str| ringdesign_core::setting::Stamp {
        name: name.into(),
        theta_deg: 270.0,
        v_mm: 0.0,
        rot_deg: 0.0,
        outline: (0..12).map(|i| std::f64::consts::TAU * f64::from(i) / 12.0).map(|t| [t.cos(), t.sin()]).collect(),
        height_mm: 0.4,
        sink_mm: 0.3,
        draft_deg: 0.0,
        cut: false,
        bench: false,
        along_pull: false,
    };
    let mut d = court();
    d.stamps = vec![disc("Moon"), disc("Star")];
    let mut history = History::new(&d);
    // The window is on the star, and the moon before it is deleted.
    let mut window = Some(1);
    let before = d.stamps.clone();
    d.stamps.remove(0);
    history.commit_as(&d, "Delete stamp \"Moon\"");
    window = stamp_after(window, &before, &d.stamps);
    assert_eq!(window, Some(0));
    // Undo brings the moon back in front of it, Redo takes it away again: the window stays on the star.
    let undone = history.undo().unwrap();
    window = stamp_after(window, &d.stamps, &undone.stamps);
    assert_eq!((window, undone.stamps[1].name.as_str()), (Some(1), "Star"));
    let redone = history.redo().unwrap();
    window = stamp_after(window, &undone.stamps, &redone.stamps);
    assert_eq!(window, Some(0));
    // A window on the moon closes when an Undo takes the moon away.
    assert_eq!(stamp_after(Some(0), &undone.stamps, &redone.stamps), None);
    assert_eq!(stamp_after(None, &undone.stamps, &redone.stamps), None);
}

#[test]
fn fit_view_frames_the_chosen_part_else_the_whole_ring() {
    let b = Bench::new(posted());
    let d = b.d.clone();
    let post = b.built.evaluated().unwrap().components.iter().find(|c| c.id == 2).unwrap();
    let (lo, hi) = post.mesh.bounds().unwrap();
    let middle = |lo: ringdesign_core::Vec3, hi: ringdesign_core::Vec3| [(lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5, (lo.2 + hi.2) * 0.5];
    // The post chosen: the pivot moves onto its middle and the zoom fills the view with it.
    let mut camera = b.camera;
    let (pose, framed) = camera.fit_view(&b.built.0, &d, &[Sel::Part(2)], &[]).unwrap();
    assert_eq!(framed.said(), "Fit view: Cylinder, as chosen");
    let want = middle(lo, hi);
    assert!((0..3).all(|k| (camera.target[k] - want[k]).abs() < 1e-4), "{:?} against {want:?}", camera.target);
    assert!(pose.zoom > 3.0 && pose.pan == [0.0; 2], "{pose:?}");
    // Nothing chosen: the whole ring at its own zoom.
    let mut camera = b.camera;
    let (pose, framed) = camera.fit_view(&b.built.0, &d, &[], &[]).unwrap();
    assert_eq!(framed.said(), "Fit view: the whole ring");
    let (lo, hi) = b.built.bounds().unwrap();
    let want = middle(lo, hi);
    assert!((0..3).all(|k| (camera.target[k] - want[k]).abs() < 1e-4) && (pose.zoom - 1.0).abs() < 1e-4, "{:?} {pose:?}", camera.target);
}
