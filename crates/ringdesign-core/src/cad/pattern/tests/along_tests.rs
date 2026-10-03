//! Arrays along a path and line arrays, built on the bands the pattern tests use.
use super::*;

#[test]
fn an_array_along_the_crest_spaces_its_copies_by_arc_and_stands_each_where_a_ring_placement_would() {
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let surface = bare(&court, &lib);
    let h = 0.3;
    let seat = Placement::ring(30.0, h);
    let post = feature(2, "Post", Operation::Cylinder { radius_mm: 0.4, height_mm: 1.2 }, joined(seat.clone()));
    let kind = PatternKind::Along(crest(30.0, 150.0, 5));
    let d = with(court.clone(), vec![band(), post, feature(3, "Posts", Operation::Pattern { sources: 2.into(), kind: kind.clone() }, joined(Placement::Free))]);
    let e = on(&d, &lib, &surface);
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let used = seat.frame_on(&d, Some(&surface)).unwrap();
    let moved = copy_motions(&d, Some(&surface), &e, 2, &kind).unwrap();
    assert_eq!(moved.len(), 4, "five instances, the post's own among them");
    let made = component(&e, 3).made.clone().unwrap();
    let source = centroid(&component(&e, 2).trace.positions);
    for (k, m) in moved.iter().enumerate() {
        let theta = 30.0 + 30.0 * (k + 1) as f64;
        let o = m.point(used.origin);
        let at = o[1].atan2(o[0]).to_degrees();
        let (hit, n) = cad::surface_hit(&surface, theta, 0.0).unwrap();
        let stand = dot(sub(o, hit), n);
        eprintln!("copy {} at {at:.4}°, {stand:.4} mm off the band, {:.5} along the finger", k + 1, o[2]);
        assert!((at - theta).abs() < 0.02, "copy {} at {at}° against {theta}°", k + 1);
        assert!((stand - h).abs() < 0.01 && o[2].abs() < 1e-3, "copy {}: {stand:.4} off the band at z {:.4}", k + 1, o[2]);
        assert!(dist(copy_centroid(&made, &format!("Copy {}, ", k + 1)), m.point(source)) < 1e-9, "the copy is the post carried by its motion");
        assert_eq!(scale_of(m), 1.0, "a flat scale moves rigidly");
    }
    let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
    let v = &built.report.validation;
    assert!(v.watertight && v.boundary_edges == 0 && built.parts.notes.is_empty(), "{v:?} {:?}", built.parts.notes);
    assert_eq!((built.parts.joined, pieces(&built.mesh)), (2, 1), "every post reaches the band");
    // A whole turn steps the count round and never stands a copy on the first.
    let (seated, frame_of) = env_on(&d, &surface, &e);
    let env = along::Env { design: &d, seated: &seated, frame_of: &frame_of };
    let turn = along::stations(&crest(10.0, 370.0, 8), &env).unwrap();
    assert_eq!(turn.len(), 8);
    for (k, s) in turn.iter().enumerate() {
        let a = (s.frame.origin[1].atan2(s.frame.origin[0]).to_degrees() - 10.0).rem_euclid(360.0);
        let a = if a > 359.9 { a - 360.0 } else { a };
        assert!((a - 45.0 * k as f64).abs() < 1e-6, "station {k} at {a}°");
        let o = s.frame.origin;
        assert!(unit_frame(&s.frame) && s.frame.y_axis[1] * o[0] - s.frame.y_axis[0] * o[1] > 0.0, "y runs round the ring");
    }
    // Phase 0.5 centres the copies in equal cells of an open path.
    let cells = along::stations(&Along { phase: 0.5, ..crest(30.0, 150.0, 4) }, &env).unwrap();
    for (k, s) in cells.iter().enumerate() {
        let a = s.frame.origin[1].atan2(s.frame.origin[0]).to_degrees();
        assert!((a - (45.0 + 30.0 * k as f64)).abs() < 1e-6, "cell {k} at {a}°");
    }
}

#[test]
fn a_crest_path_rides_a_bypass_arms_one_crest_on_the_parting_plane_at_the_sections_furthest_reach() {
    let lib = AlphaLibrary::builtin();
    let mut d = template("Court band");
    d.shank.kind = crate::profile::ShankKind::Bypass;
    let surface = bare(&d, &lib);
    let d = with(d, vec![band()]);
    let e = on(&d, &lib, &surface);
    let (seated, frame_of) = env_on(&d, &surface, &e);
    let env = along::Env { design: &d, seated: &seated, frame_of: &frame_of };
    let arm = Along { alternate_deg: 50.0, scale: [1.0, 0.6], ..crest(20.0, 160.0, 8) };
    let st = along::stations(&arm, &env).unwrap();
    let reference = d.reference_loop();
    for (k, s) in st.iter().enumerate() {
        let o = s.frame.origin;
        let theta = o[1].atan2(o[0]).to_degrees();
        let section = d.section_at(theta, 512, None, Some(&reference));
        let reach = section.pts.iter().filter(|p| p.surface).map(|p| p.r).fold(0.0, f64::max);
        assert!(o[2].abs() < 1e-9, "station {k} stands {:.6} off the parting plane", o[2]);
        assert!((o[0].hypot(o[1]) - reach).abs() < 0.01, "station {k} at {theta:.2}°: {:.4} against the section's reach {reach:.4}", o[0].hypot(o[1]));
        assert!(unit_frame(&s.frame) && (s.scale - (1.0 - 0.4 * k as f64 / 7.0)).abs() < 1e-9);
    }
}

#[test]
fn stones_an_array_along_a_path_carries_stay_stones_graded_with_their_copies_and_the_cap_holds() {
    use crate::setstone::{self, StoneSource};
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let gem = Gem::calibrated(GemCut::Round, 2.0);
    let stand = builders::stand_off_mm("claw4", gem);
    let stone = builders::stone_feature(2, gem, Placement::ring(40.0, stand));
    let claws = builders::feature_on(3, "Four-claw head", builders::CLAW, 2, serde_json::json!({ "prongs": 4 }));
    let graded = Along { scale: [1.0, 0.5], ..crest(40.0, 140.0, 6) };
    let heads = feature(4, "Heads", Operation::Pattern { sources: 3.into(), kind: PatternKind::Along(graded) }, joined(Placement::Free));
    let d = with(court.clone(), vec![band(), stone, claws, heads]);
    let stones = setstone::set_stones(&d);
    assert_eq!(stones.len(), 6, "the stone and the five its head's copies hold");
    let copies: Vec<&setstone::SetStone> = stones.iter().filter(|s| s.cad_feature() == Some(4)).collect();
    assert_eq!(copies.iter().map(|s| s.source).collect::<Vec<_>>(), (0..5).map(|copy| StoneSource::Cad { feature: 4, copy }).collect::<Vec<_>>());
    for (k, s) in copies.iter().enumerate() {
        let want = 2.0 * (1.0 - 0.5 * (k + 1) as f64 / 5.0);
        assert!((s.gem.w_mm - want).abs() < 1e-9 && (s.gem.l_mm - want).abs() < 1e-9, "copy {}: {} mm against {want}", k + 1, s.gem.w_mm);
        assert!(unit_frame(s.frame.as_ref().unwrap()), "copy {}'s girdle frame stays square: {:?}", k + 1, s.frame);
    }
    // The build's own record agrees, stone for stone.
    let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
    let v = &built.report.validation;
    assert!(v.watertight && built.parts.notes.is_empty(), "{v:?} {:?}", built.parts.notes);
    let on_build = setstone::set_stones_built(&d, &built);
    assert_eq!(on_build.len(), stones.len());
    for (a, b) in stones.iter().zip(&on_build) {
        assert_eq!((a.source, a.gem.cut), (b.source, b.gem.cut));
        assert!((a.gem.w_mm - b.gem.w_mm).abs() < 1e-9);
        let (fa, fb) = (a.frame.unwrap(), b.frame.unwrap());
        assert!(dist(fa.origin, fb.origin) < 0.02, "{:?} {:?}", fa.origin, fb.origin);
    }
    let made = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 4).unwrap().made.clone().unwrap();
    assert!(made.named.names.iter().any(|n| n.starts_with("Copy 5, ")));
    // An array of an array carrying more than the cap is counted to it and named.
    let pad = builders::stone_feature(2, Gem::calibrated(GemCut::Round, 1.0), Placement::ring(30.0, 0.5));
    let reference = Component { reference: true, ..Component::default() };
    let inner = feature(3, "Ten", Operation::Pattern { sources: 2.into(), kind: PatternKind::Along(crest(30.0, 39.0, 10)) }, reference.clone());
    let outer = feature(4, "Sixty", Operation::Pattern { sources: 3.into(), kind: PatternKind::Along(crest(30.0, 330.0, 60)) }, reference);
    let crowded = with(court, vec![band(), pad, inner, outer]);
    let record = setstone::record(&crowded, None);
    let by = |id: Id| record.stones.iter().filter(|s| s.cad_feature() == Some(id)).count();
    assert_eq!((by(2), by(3)), (1, 9), "the pad's stone and its nine copies");
    assert_eq!(record.stones.len(), setstone::MAX_CAD_STONES, "fifty-nine copies of those nine stop at the cap: {}", by(4));
    assert!(record.past_cap.iter().any(|(id, _)| *id == 4), "{:?}", record.past_cap);
}

#[test]
fn an_array_along_a_sweep_steps_its_pitch_along_the_stem_and_alternates_sides() {
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let surface = bare(&court, &lib);
    let r = court.inner_radius_mm() + court.profile.thickness_mm + 0.4;
    let path: Vec<[f64; 3]> = (0..=40)
        .map(|k| {
            let t = (60.0 + 1.5 * k as f64).to_radians();
            [r * t.cos(), r * t.sin(), 0.8 * (k as f64 / 40.0 * PI).sin()]
        })
        .collect();
    let stem = feature(2, "Stem", Operation::sweep(Sketch::circle(0.3), path.clone()), joined(Placement::Free));
    let rootlet = feature(3, "Rootlet", Operation::Cylinder { radius_mm: 0.1, height_mm: 0.4 }, joined(Placement::ring(60.0, 0.2)));
    let a = Along { path: AlongPath::Feature(2), pitch_mm: Some(0.6), alternate_deg: 180.0, ..Along::default() };
    let rootlets = feature(4, "Rootlets", Operation::Pattern { sources: 3.into(), kind: PatternKind::Along(a.clone()) }, joined(Placement::Free));
    let d = with(court.clone(), vec![band(), stem, rootlet, rootlets]);
    assert_eq!(d.cad.as_ref().unwrap().feature(4).unwrap().operation.sources(), vec![3, 2], "the stem is read");
    let e = on(&d, &lib, &surface);
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let (seated, frame_of) = env_on(&d, &surface, &e);
    let env = along::Env { design: &d, seated: &seated, frame_of: &frame_of };
    let st = along::stations(&a, &env).unwrap();
    let length: f64 = path.windows(2).map(|w| dist(w[0], w[1])).sum();
    assert_eq!(st.len(), (length / 0.6).floor() as usize + 1, "as many as the pitch fits on {length:.3} mm");
    for (k, s) in st.iter().enumerate() {
        assert!((s.along_mm - 0.6 * k as f64).abs() < 1e-9);
        let off = path
            .windows(2)
            .map(|w| {
                let d = sub(w[1], w[0]);
                let t = (dot(sub(s.frame.origin, w[0]), d) / dot(d, d)).clamp(0.0, 1.0);
                dist(s.frame.origin, std::array::from_fn(|i| w[0][i] + d[i] * t))
            })
            .fold(f64::MAX, f64::min);
        assert!(off < 1e-9, "station {k} stands {off} off the stem");
        let radial = [s.frame.origin[0] / r, s.frame.origin[1] / r, 0.0];
        assert!(unit_frame(&s.frame) && dot(s.frame.z_axis, radial) > 0.9, "station {k} stands out of the band: {:?}", s.frame.z_axis);
    }
    let moved = along::motions(&a, &env).unwrap();
    let probe = st[0].frame.point([1.0, 0.0, 0.0]);
    for (k, m) in moved.iter().enumerate() {
        let side = if (k + 1) % 2 == 1 { -1.0 } else { 1.0 };
        assert!(dist(m.point(probe), st[k + 1].frame.point([side, 0.0, 0.0])) < 1e-9, "copy {} stands on its side of the stem", k + 1);
    }
    let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
    let v = &built.report.validation;
    eprintln!("{} rootlets along {length:.2} mm of stem: {} faces, watertight {}", st.len(), built.mesh.faces.len(), v.watertight);
    assert!(v.watertight && built.parts.notes.is_empty(), "{v:?} {:?}", built.parts.notes);
}

#[test]
fn an_array_follows_a_closed_sweep_round_a_sweep_along_a_sketch_and_a_smooth_twist_through_points() {
    use crate::cad::{SweepPath, TwistPath};
    use crate::sketch::Geometry;
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let surface = bare(&court, &lib);
    let r = court.inner_radius_mm() + court.profile.thickness_mm + 0.5;
    // A closed sweep: a hoop round the crest, its first station not repeated last.
    let hoop: Vec<[f64; 3]> = (0..48).map(|k| {
        let t = (7.5 * k as f64).to_radians();
        [r * t.cos(), r * t.sin(), 0.0]
    }).collect();
    let closed = Operation::Sweep { sketch: Sketch::circle(0.3).into(), path: SweepPath::Points(hoop.clone()), closed: true, twist_deg: 0.0, end_scale: 1.0 };
    // A sweep along a sketch's arc on the parting plane, and a smooth twist through points.
    let mut arc = Sketch::default();
    let (c, a, b) = (arc.point([0.0, 0.0]), arc.point([r, 0.0]), arc.point([0.0, r]));
    let entity = arc.entity(Geometry::Arc { center: c, start: a, end: b });
    let along_sketch = Operation::Sweep { sketch: Sketch::circle(0.3).into(), path: SweepPath::Sketch { feature: 3, entity, lift_mm: 0.0 }, closed: false, twist_deg: 0.0, end_scale: 1.0 };
    let bent: Vec<[f64; 3]> = [0.0, 30.0, 60.0, 90.0].iter().map(|deg: &f64| {
        let t = deg.to_radians();
        [r * t.cos(), r * t.sin(), 0.4 * t.sin()]
    }).collect();
    let smooth = Operation::Twist { sketch: Sketch::circle(0.3).into(), path: TwistPath::Points { points: bent.clone(), smooth: true }, degrees: 0.0, end_scale: 1.0, scale: Vec::new(), closed: false };
    let d = with(court.clone(), vec![
        band(),
        feature(2, "Hoop", closed, joined(Placement::Free)),
        feature(3, "Arc", Operation::Sketch { sketch: arc }, Component::default()),
        feature(4, "Along the arc", along_sketch, joined(Placement::Free)),
        feature(5, "Smooth", smooth, joined(Placement::Free)),
    ]);
    let e = on(&d, &lib, &surface);
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let (seated, frame_of) = env_on(&d, &surface, &e);
    let env = along::Env { design: &d, seated: &seated, frame_of: &frame_of };
    // Round the closed hoop, eight copies stand 45° apart and the last stops short of the first.
    let round = along::stations(&Along { path: AlongPath::Feature(2), count: 8, ..Along::default() }, &env).unwrap();
    let angles: Vec<f64> = round.iter().map(|s| s.frame.origin[1].atan2(s.frame.origin[0]).to_degrees().rem_euclid(360.0)).collect();
    for (k, a) in angles.iter().enumerate() {
        assert!((a - 45.0 * k as f64).abs() < 1e-6, "{angles:?}");
    }
    // Along the sketch's arc, the stations stand on the arc where the sweep samples it.
    let arc_stations = along::stations(&Along { path: AlongPath::Feature(4), count: 4, ..Along::default() }, &env).unwrap();
    for s in &arc_stations {
        let o = s.frame.origin;
        assert!((o[0].hypot(o[1]) - r).abs() < 0.01 && o[2].abs() < 1e-9, "{o:?}");
    }
    let last = arc_stations[3].frame.origin;
    assert!(last[0].abs() < 1e-6 && (last[1] - r).abs() < 1e-6, "{last:?}");
    // Through the smooth twist's points, the path is its curve: it passes each point it was drawn through.
    let st = along::stations(&Along { path: AlongPath::Feature(5), count: 4, ..Along::default() }, &env).unwrap();
    assert!(dist(st[0].frame.origin, bent[0]) < 1e-9 && dist(st[3].frame.origin, bent[3]) < 1e-9);
    let run = crate::cad::twist::path_points(&bent, true, false, 32).unwrap();
    for p in &bent {
        assert!(run.iter().any(|(q, _)| dist(*p, *q) < 1e-9), "the curve passes {p:?}");
    }
}

#[test]
fn an_array_along_a_sketch_curve_on_a_work_plane_follows_it_square_to_the_plane() {
    use crate::sketch::Geometry;
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let surface = bare(&court, &lib);
    let mut hood = Sketch::default();
    hood.plane.on_face = Some(FaceAnchor { feature: 2, face: FaceRef::bare(0) });
    let p: Vec<Id> = [[-3.0, -1.0], [-1.0, -1.0], [1.0, 1.0], [3.0, 1.0]].into_iter().map(|xy| hood.point(xy)).collect();
    let ogee = hood.entity(Geometry::Bezier { points: [p[0], p[1], p[2], p[3]] });
    let tip = hood.point([3.0, 2.5]);
    let stem = hood.entity(Geometry::Line { a: p[3], b: tip });
    let table = feature(2, "Table", Operation::Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: 0.0 }, Component::default());
    let hood = feature(3, "Hood", Operation::Sketch { sketch: hood }, Component::default());
    let a = Along { path: AlongPath::Sketch { feature: 3, entities: vec![ogee] }, count: 6, phase: 0.5, alternate_deg: 180.0, ..Along::default() };
    // The path read before the crocket is made, to model it at the origin and stand it at the first station.
    let d = with(court.clone(), vec![band(), table.clone(), hood.clone()]);
    let e = on(&d, &lib, &surface);
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let plane = *e.plane(2).unwrap();
    let (seated, frame_of) = env_on(&d, &surface, &e);
    let env = along::Env { design: &d, seated: &seated, frame_of: &frame_of };
    let st = along::stations(&a, &env).unwrap();
    let (translation, rotation_deg) = along::transform_onto(&st[0].frame);
    let placed = crate::cad::rotate_place(translation, rotation_deg).unwrap();
    for (got, want) in [(placed.x_axis, st[0].frame.x_axis), (placed.y_axis, st[0].frame.y_axis), (placed.z_axis, st[0].frame.z_axis), (placed.origin, st[0].frame.origin)] {
        assert!(dist(got, want) < 1e-12, "{got:?} against {want:?}");
    }
    let crocket = feature(4, "Crocket", Operation::Box { size: [0.6, 0.4, 0.3] }, Component::default());
    let springer = feature(5, "Crocket at the springer", Operation::Transform { source: 4, translation, rotation_deg }, joined(Placement::Free));
    let crockets = feature(6, "Crockets", Operation::Pattern { sources: 5.into(), kind: PatternKind::Along(a.clone()) }, joined(Placement::Free));
    let full = with(court.clone(), vec![band(), table, hood, crocket, springer, crockets]);
    let built = on(&full, &lib, &surface);
    assert!(built.failures().is_empty(), "{:?}", built.failures());
    assert!(dist(centroid(&component(&built, 5).trace.positions), st[0].frame.origin) < 1e-9, "the crocket stands at the first station");
    let made = component(&built, 6).made.clone().unwrap();
    for (k, s) in st.iter().enumerate().skip(1) {
        assert!(dist(copy_centroid(&made, &format!("Copy {k}, ")), s.frame.origin) < 1e-9, "copy {k} stands at station {k}");
    }
    assert_eq!(st.len(), 6);
    let step = st[1].along_mm - st[0].along_mm;
    assert!((st[0].along_mm - 0.5 * step).abs() < 1e-9, "the first stands half a step in");
    for (k, s) in st.iter().enumerate() {
        assert!(dist(s.frame.z_axis, plane.normal) < 1e-9, "station {k} stands square to the table");
        assert!(dot(sub(s.frame.origin, plane.origin), plane.normal).abs() < 1e-9, "station {k} lies in the table");
        if k > 0 {
            assert!((s.along_mm - st[k - 1].along_mm - step).abs() < 1e-9);
        }
    }
    // The curve and its stem chained, in the order named or found from where they meet: longer, and still in the plane.
    for entities in [vec![ogee, stem], vec![]] {
        let both = along::stations(&Along { path: AlongPath::Sketch { feature: 3, entities }, ..a.clone() }, &env).unwrap();
        assert!(both[5].along_mm > st[5].along_mm + 1.0, "{} against {}", both[5].along_mm, st[5].along_mm);
    }
    // Named out of order, the chain says which curve does not meet the one before it.
    let mut skew = d.clone();
    let Some(Operation::Sketch { sketch }) = skew.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 3).map(|f| &mut f.operation) else { panic!() };
    let lone = sketch.point([9.0, 9.0]);
    let far = sketch.point([9.0, 12.0]);
    let apart = sketch.entity(Geometry::Line { a: lone, b: far });
    let why = along::stations(&Along { path: AlongPath::Sketch { feature: 3, entities: vec![ogee, apart] }, ..a.clone() }, &along::Env { design: &skew, seated: &seated, frame_of: &frame_of }).unwrap_err();
    assert!(format!("{why:#}").contains("do not meet"), "{why:#}");
    let built = crate::mesh::try_build(&full, &lib, params()).unwrap();
    assert!(built.report.validation.watertight && built.parts.notes.is_empty(), "{:?} {:?}", built.report.validation, built.parts.notes);
}

#[test]
fn a_line_array_steps_its_copies_in_the_frame_its_source_is_seated_by_and_scaled_copies_scale_their_volume() {
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let surface = bare(&court, &lib);
    let seat = Placement::ring(90.0, 1.0);
    let kind = PatternKind::Line { along: [0.0, 2.0, 0.0], count: 3, pitch_mm: 1.5 };
    let d = with(
        court.clone(),
        vec![band(), feature(2, "Bay", Operation::Box { size: [1.0, 0.8, 0.6] }, joined(seat.clone())), feature(3, "Bays", Operation::Pattern { sources: 2.into(), kind: kind.clone() }, Component::default())],
    );
    let e = on(&d, &lib, &surface);
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let used = seat.frame_on(&d, Some(&surface)).unwrap();
    let made = component(&e, 3).made.clone().unwrap();
    let source = centroid(&component(&e, 2).trace.positions);
    for k in 1..3 {
        let want: [f64; 3] = std::array::from_fn(|i| source[i] + used.y_axis[i] * 1.5 * k as f64);
        assert!(dist(copy_centroid(&made, &format!("Copy {k}, ")), want) < 1e-9, "copy {k} steps round the ring in the bay's own frame");
    }
    // A free part steps in the world.
    assert_eq!(world_motions(&kind, &|_| None).unwrap()[1].origin, [0.0, 3.0, 0.0]);
    // Copies of a ball graded to twice its size hold eight times its metal.
    let ball = feature(2, "Ball", Operation::Sphere { radius_mm: 0.5 }, Component::default());
    let grow = Along { path: AlongPath::Points(vec![[0.0, -12.0, 3.0], [0.0, -12.0, 9.0]]), count: 2, scale: [1.0, 2.0], ..Along::default() };
    let d = with(RingDesign::default(), vec![ball, feature(3, "Grown", Operation::Pattern { sources: 2.into(), kind: PatternKind::Along(grow) }, Component::default())]);
    let e = evaluate(&d, &lib, params()).unwrap();
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let (one, two) = (component(&e, 2).mesh.volume_mm3(), component(&e, 3).mesh.volume_mm3());
    assert!((two / one - 8.0).abs() < 1e-6, "{two} against {one}");
    // The inverse undoes a scale, and is the transpose bit for bit on a rigid motion.
    let m = turn_about([1.0, 2.0, 3.0], [0.3, 0.4, 0.5], 37.0);
    let scaled = Motion { x_axis: m.x_axis.map(|v| v * 1.7), y_axis: m.y_axis.map(|v| v * 1.7), z_axis: m.z_axis.map(|v| v * 1.7), origin: m.origin };
    let round = then(&inverse(&scaled), &scaled);
    assert!(dist(round.point([0.7, -0.2, 4.0]), [0.7, -0.2, 4.0]) < 1e-12 && (scale_of(&scaled) - 1.7).abs() < 1e-12);
    let row = |k: usize| [m.x_axis[k], m.y_axis[k], m.z_axis[k]];
    let transpose = Motion { x_axis: row(0), y_axis: row(1), z_axis: row(2), origin: [-dot(m.x_axis, m.origin), -dot(m.y_axis, m.origin), -dot(m.z_axis, m.origin)] };
    assert_eq!(inverse(&m), transpose);
}

#[test]
fn a_levelled_array_stands_square_to_the_parting_plane_and_a_path_along_the_finger_refuses_it() {
    let lib = AlphaLibrary::builtin();
    let court = template("Court band");
    let surface = bare(&court, &lib);
    let d = with(court.clone(), vec![band()]);
    let e = on(&d, &lib, &surface);
    let (seated, frame_of) = env_on(&d, &surface, &e);
    let env = along::Env { design: &d, seated: &seated, frame_of: &frame_of };
    let r = court.inner_radius_mm() + court.profile.thickness_mm + 0.2;
    let climbing: Vec<[f64; 3]> = (0..=20)
        .map(|k| {
            let t = (80.0 + k as f64).to_radians();
            [r * t.cos(), r * t.sin(), -1.0 + 0.1 * k as f64]
        })
        .collect();
    let a = Along { path: AlongPath::Points(climbing), count: 5, ..Along::default() };
    let free = along::stations(&a, &env).unwrap();
    let level = along::stations(&Along { level: true, ..a.clone() }, &env).unwrap();
    for (f, l) in free.iter().zip(&level) {
        assert!(f.frame.y_axis[2].abs() > 0.05, "the path climbs along the finger");
        assert!(l.frame.y_axis[2].abs() < 1e-12 && l.frame.z_axis[2].abs() < 1e-12 && (l.frame.x_axis[2].abs() - 1.0).abs() < 1e-12, "{:?}", l.frame);
        assert_eq!(f.frame.origin, l.frame.origin);
    }
    let along_finger = Along { path: AlongPath::Points(vec![[r, 0.0, -1.0], [r, 0.0, 1.0]]), count: 3, level: true, ..Along::default() };
    let why = format!("{:#}", along::stations(&along_finger, &env).unwrap_err());
    assert!(why.contains("a levelled array needs a path that runs round the ring"), "{why}");
}

#[test]
fn an_array_along_a_path_refuses_what_it_cannot_follow_by_name() {
    let lib = AlphaLibrary::builtin();
    let post = || feature(2, "Post", Operation::Cylinder { radius_mm: 0.4, height_mm: 1.0 }, joined(Placement::ring(90.0, 0.3)));
    let fail = |kind: PatternKind, extra: Vec<Feature>| {
        let mut features = vec![band(), post()];
        features.extend(extra);
        features.push(feature(9, "Copies", Operation::Pattern { sources: 2.into(), kind }, joined(Placement::Free)));
        failed(&evaluate(&with(template("Court band"), features), &lib, params()).unwrap(), 9)
    };
    let along = PatternKind::Along;
    assert_eq!(fail(along(crest(0.0, 90.0, 1)), vec![]), "A pattern holds 2 to 120 instances, its source among them, not 1");
    assert_eq!(fail(along(Along { path: AlongPath::Crest { from_deg: 0.0, to_deg: 90.0 }, ..Along::default() }), vec![]), "An array along a path takes a count, a pitch, or both");
    assert_eq!(fail(along(Along { pitch_mm: Some(0.0), ..crest(0.0, 90.0, 0) }), vec![]), "An array along a path steps 0.01 to 1000 mm between copies, not 0");
    let short = fail(along(Along { pitch_mm: Some(5.0), ..crest(80.0, 100.0, 4) }), vec![]);
    assert!(short.starts_with("4 copies 5 mm apart need 15.00 mm of path; it runs"), "{short}");
    let crowded = fail(along(Along { pitch_mm: Some(0.1), ..crest(0.0, 360.0, 0) }), vec![]);
    assert!(crowded.contains("holds more than 120 copies 0.1 mm apart"), "{crowded}");
    let block = feature(3, "Block", Operation::Box { size: [1.0, 1.0, 1.0] }, Component::default());
    assert_eq!(fail(along(Along { path: AlongPath::Feature(3), count: 3, ..Along::default() }), vec![block]), "#3 Block is a box; an array follows a sweep's or a twisted sweep's path");
    assert_eq!(fail(along(Along { scale: [1.0, 0.0], ..crest(0.0, 90.0, 3) }), vec![]), "An array along a path scales its copies by 0.05 to 20, not [1.0, 0.0]");
    assert_eq!(fail(PatternKind::Line { along: [0.0; 3], count: 3, pitch_mm: 1.0 }, vec![]), "A line array runs along a direction, and [0, 0, 0] is none");
    // The parts are the whole ring: there is no crest to follow.
    let alone = with(RingDesign::default(), vec![post(), feature(9, "Copies", Operation::Pattern { sources: 2.into(), kind: along(crest(0.0, 90.0, 3)) }, joined(Placement::Free))]);
    assert_eq!(failed(&evaluate(&alone, &lib, params()).unwrap(), 9), "A crest path runs along the band, and this ring's parts are the whole ring");
}

#[test]
fn arrays_along_a_path_read_and_write_their_json_and_fence_the_format() {
    let a = Along { scale: [1.0, 0.6], alternate_deg: 50.0, ..crest(10.0, 170.0, 7) };
    let op = Operation::Pattern { sources: 3.into(), kind: PatternKind::Along(a.clone()) };
    let text = serde_json::to_string(&op).unwrap();
    assert_eq!(text, r#"{"Pattern":{"source":3,"kind":{"along":{"path":{"crest":{"from_deg":10.0,"to_deg":170.0}},"count":7,"alternate_deg":50.0,"scale":[1.0,0.6]}}}}"#);
    let back: Operation = serde_json::from_str(&text).unwrap();
    assert!(matches!(back, Operation::Pattern { kind: PatternKind::Along(ref b), .. } if *b == a));
    let short: Operation = serde_json::from_str(r#"{"Pattern":{"source":3,"kind":{"along":{"path":{"feature":7},"pitch_mm":0.6}}}}"#).unwrap();
    let Operation::Pattern { kind: PatternKind::Along(b), .. } = &short else { panic!() };
    assert_eq!((b.count, b.scale, b.level, b.phase), (0, [1.0, 1.0], false, 0.0), "what is left out is the default");
    assert_eq!(short.sources(), vec![3, 7], "a feature path is read");
    let sketch: PatternKind = serde_json::from_str(r#"{"along":{"path":{"sketch":{"feature":4}},"count":3}}"#).unwrap();
    assert_eq!(sketch.reads(), vec![4]);
    let line = Operation::Pattern { sources: 2.into(), kind: PatternKind::Line { along: [0.0, 1.0, 0.0], count: 3, pitch_mm: 2.0 } };
    assert_eq!(serde_json::to_string(&line).unwrap(), r#"{"Pattern":{"source":2,"kind":{"line":{"along":[0.0,1.0,0.0],"count":3,"pitch_mm":2.0}}}}"#);
    assert_eq!((op.label(), line.label()), ("Array along a path", "Line array"));
    // Fenced at format 6, in the document and in a graph; a plain ring array is not.
    let with_kind = |kind: PatternKind| {
        let post = feature(2, "Post", Operation::Cylinder { radius_mm: 0.4, height_mm: 1.0 }, joined(Placement::ring(90.0, 0.3)));
        with(template("Court band"), vec![band(), post, feature(3, "Copies", Operation::Pattern { sources: 2.into(), kind }, joined(Placement::Free))])
    };
    for kind in [PatternKind::Along(a.clone()), PatternKind::Line { along: [0.0, 1.0, 0.0], count: 3, pitch_mm: 2.0 }] {
        let d = with_kind(kind);
        assert_eq!(crate::library::format_version_for(&d), crate::library::FORMAT_VERSION);
        let text = crate::library::design_json(&d).unwrap();
        assert!(crate::library::read_design(&text, crate::library::PLAIN_FORMAT_VERSION).is_err(), "a format-5 reader refuses it by its version");
        let reopened = crate::library::load_design_str(&text).unwrap();
        assert_eq!(serde_json::to_value(&reopened.cad).unwrap(), serde_json::to_value(&d.cad).unwrap());
    }
    assert_eq!(crate::library::format_version_for(&with_kind(PatternKind::Ring { count: 3, span_deg: 360.0 })), crate::library::PLAIN_FORMAT_VERSION);
    let graph = serde_json::json!({"nodes": [{"kind": "cad.feature", "params": {"operation": op}}]});
    assert!(crate::library::template_features_in_json(&graph) && follows_line_or_path_json(&graph));
    for kind in ["path.wreath", "path.along", "cad.features"] {
        assert!(crate::library::template_features_in_json(&serde_json::json!({"nodes": [{"kind": kind, "inputs": {}}]})), "{kind}");
    }
}
