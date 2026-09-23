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
    cad.landed(&built, scene, band, &d);
    let camera = camera(&built);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(420.0, 600.0));
    let lib = AlphaLibrary::builtin();
    let v = View { rect, camera: &camera, design: &d, lib: &lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false };
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
        cad.landed(&built, scene, band, &d);
        let camera = camera(&built);
        Self { d, built, cad, camera, lib: AlphaLibrary::builtin(), ctx: egui::Context::default(), renderer: Default::default(), time: 1.0, rect: RECT, measuring: false }
    }
    fn view(&self) -> View<'_> {
        View { rect: RECT, camera: &self.camera, design: &self.d, lib: &self.lib, build: Some(&self.built), field: None, covered: &[], active: true, measuring: self.measuring }
    }
    /// One frame with `events`, a tenth of a second after the last.
    fn step(&mut self, events: Vec<Event>) -> Took {
        self.time += 0.1;
        let v = View { rect: self.rect, camera: &self.camera, design: &self.d, lib: &self.lib, build: Some(&self.built), field: None, covered: &[], active: true, measuring: self.measuring };
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
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false };
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
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false };
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
        let [_, caption, fields, _] = super::areas().map(|id| b.ctx.memory(|m| m.area_rect(id)));
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
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false };
    b.cad.act(&v, MenuAction::Attach(2, Attach::Cut));
    b.cad.act(&v, MenuAction::Pattern { feature: 2, key: keys::MIRROR_BAND });
    b.cad.act(&v, MenuAction::SketchOnPlane { theta_deg: 0.0, across_mm: 0.0 });
    let asked = b.cad.take_requests();
    let edits: Vec<&Vec<CadEdit>> = asked.iter().filter_map(|r| if let Request::Edit { edits, .. } = r { Some(edits) } else { None }).collect();
    assert!(matches!(edits[0].as_slice(), [CadEdit::Attach { id: 2, attach: Attach::Cut }]));
    assert_eq!(edits.len(), 1, "the post stands on the band's mid-plane, so its mirror across it is refused: {asked:?}");
    assert!(asked.iter().any(|r| matches!(r, Request::Status(s) if s.contains("stands on that plane"))));
    assert!(asked.iter().any(|r| matches!(r, Request::Status(s) if s.starts_with("Not on the phone yet"))));
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
    let items = menu::phone_items(ringdesign_workbench::viewport::context_items(&b.cad.selection, None, &b.d));
    let four = items.iter().find(|i| i.label == "Four claws").expect("a chosen stone offers its settings").action.clone();
    let d = b.d.clone();
    let v = View { rect: RECT, camera: &b.camera, design: &d, lib: &b.lib, build: Some(&b.built), field: None, covered: &[], active: true, measuring: false };
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
    // Held, it opens its own menu: sketching greyed until the phone sketches, the mirror offered, hiding.
    assert!(b.hold(edge));
    let m = b.cad.planes.menu.clone().expect("the plane's menu is open");
    assert!(b.cad.menu.is_none(), "not the ring's menu");
    assert_eq!(m.heading, "Work plane: Section at 0°");
    let rows: Vec<(&str, bool)> = m.rows.iter().map(|r| (r.label, r.enabled)).collect();
    assert_eq!(rows, [("Sketch on this plane", false), ("Mirror the chosen part across it", true), ("Hide work planes", true)]);
    assert_eq!(m.rows[0].hint, menu::NO_SKETCH);
    assert_eq!(m.rows[1].hint, "Cylinder reflected across Section at 0°, one new Mirror feature");
    let drawn = b.ctx.memory(|mem| mem.area_rect(menu::area())).expect("the menu is drawn");
    assert!(RECT.contains_rect(drawn), "{drawn:?}");
    // A press on the ring beside it closes it and does nothing else.
    b.tap(b.post());
    assert!(b.cad.planes.menu.is_none() && b.cad.selection.items == [Sel::Part(2)]);
    // Its mirror row: one Mirror feature of the post across the plane.
    let (d, camera, built) = (b.d.clone(), b.camera, b.built.clone());
    let v = View { rect: RECT, camera: &camera, design: &d, lib: &b.lib, build: Some(&built), field: None, covered: &[], active: true, measuring: false };
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
