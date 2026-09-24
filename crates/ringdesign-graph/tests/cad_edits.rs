//! Every `CadEdit` gives the same document through `Document::apply` and the graph applier.
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{
        self, Attach, Boolean, Component, ComponentRole, Document, FaceRef, Feature, MirrorPlane, Operation, PatternKind, Placement, PlaneBase, Stage, builders,
        edit::{Applied, CadEdit},
    },
    gem::{Gem, GemCut},
    sketch::Id,
};
use serde_json::json;
use ringdesign_graph::{
    eval::{Evaluator, OUTPUT_DESIGN_PIN, OUTPUT_KIND, Targets},
    graph::Graph,
    nodes::cad as graph_cad,
    registry::Registry,
    value::Value,
};
use std::time::{Duration, Instant};

fn feature(id: Id, name: &str, operation: Operation) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component::default() }
}
fn cylinder(name: &str) -> Feature {
    feature(0, name, Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 })
}
/// The band with a joined cylinder standing on it.
fn band_and_cylinder() -> RingDesign {
    let mut d = RingDesign::default();
    let mut doc = Document::default();
    let mut band = feature(1, "Shank", Operation::Band);
    band.component.role = ComponentRole::Shank;
    doc.append(band).unwrap();
    let mut c = feature(2, "Bezel", Operation::Cylinder { radius_mm: 2.0, height_mm: 2.5 });
    c.component.placement = Placement::ring(90.0, 0.0);
    c.component.attach = Attach::Join;
    doc.append(c).unwrap();
    d.cad = Some(doc);
    d
}
/// The box the patterns document pulls: seated at 300°, and the top face of it signed in that seat.
fn pulled_box() -> (Feature, FaceRef) {
    let mut block = feature(5, "Block", Operation::Box { size: [2.0, 2.0, 1.0] });
    block.component.placement = Placement::ring(300.0, 0.3);
    block.component.attach = Attach::Join;
    let mut d = RingDesign::default();
    let mut doc = Document::default();
    doc.append(block.clone()).unwrap();
    d.cad = Some(doc);
    let e = cad::evaluate(&d, &AlphaLibrary::builtin(), params()).unwrap();
    let c = &e.components[0];
    let top = (0..c.body.faces.len()).find(|i| cad::face_signature(&c.body, *i, &c.frame).is_some_and(|s| s.normal[2] > 0.99)).unwrap();
    (block, FaceRef::signed(&c.body, top, &c.frame))
}
/// A band, bezel, stone, prong and box carrying two arrays, a work plane, a mirror across it and a press-pull.
fn band_and_patterns() -> RingDesign {
    let mut d = band_and_cylinder();
    let doc = d.cad.as_mut().unwrap();
    let gem = Gem::calibrated(GemCut::Round, 5.0);
    doc.append(builders::stone_feature(3, gem, Placement::ring(200.0, builders::stand_off_mm("claw4", gem)))).unwrap();
    let mut prong = feature(4, "Prong", Operation::Cylinder { radius_mm: 0.4, height_mm: 3.0 });
    prong.component.placement = Placement::ring(213.0, 1.0);
    prong.component.attach = Attach::Join;
    doc.append(prong).unwrap();
    let (block, top) = pulled_box();
    doc.append(block).unwrap();
    let joined = |id, name: &str, operation| {
        let mut f = feature(id, name, operation);
        f.component.attach = Attach::Join;
        f
    };
    doc.append(feature(6, "Section at 90°", Operation::Plane { base: PlaneBase::Section { theta_deg: 90.0 }, offset_mm: 0.0 })).unwrap();
    doc.append(joined(7, "Ring array of Bezel", Operation::Pattern { source: 2, kind: PatternKind::Ring { count: 3, span_deg: 360.0 } })).unwrap();
    doc.append(joined(8, "Prongs", Operation::Pattern { source: 4, kind: PatternKind::About { part: 3, count: 6, span_deg: 360.0 } })).unwrap();
    doc.append(joined(9, "Mirror of Block", Operation::Pattern { source: 5, kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 6 } } })).unwrap();
    doc.append(joined(10, "Pull", Operation::PressPull { source: 5, face: top, distance_mm: 0.4 })).unwrap();
    d
}
fn documents() -> Vec<(String, RingDesign)> {
    let mut all: Vec<(String, RingDesign)> = cad::examples::NAMES
        .iter()
        .map(|n| (n.to_string(), cad::examples::design(n).unwrap()))
        .collect();
    all.push(("band+cylinder".into(), band_and_cylinder()));
    all.push(("band+patterns".into(), band_and_patterns()));
    all
}
/// One edit of every kind that applies to the document, each keeping it evaluable.
fn edits_for(name: &str, doc: &Document) -> Vec<CadEdit> {
    let first = doc.features[0].id;
    // The last feature nothing reads.
    let leaf = doc.features.iter().rev().find(|f| doc.dependents(f.id).is_empty()).unwrap().id;
    let mut edits = vec![
        CadEdit::Add { feature: cylinder("Added"), after: None },
        CadEdit::Add { feature: cylinder("Added first"), after: Some(first) },
    ];
    // A leaf that reads a source, as a setting reads its stone, cannot move ahead of it.
    if doc.sources_of(leaf).is_empty() {
        edits.push(CadEdit::Move { id: leaf, after: None });
    }
    edits.extend([
        CadEdit::Rename { id: first, name: "Renamed".into() },
        CadEdit::Operation { id: leaf, operation: Operation::Sphere { radius_mm: 1.5 } },
        CadEdit::Placement { id: leaf, placement: Placement::ring(45.0, 0.5) },
        CadEdit::Attach { id: leaf, attach: Attach::Join },
        CadEdit::Stage { id: leaf, stage: Stage::Bench },
        CadEdit::Blend { id: leaf, blend_mm: 0.3 },
        CadEdit::Component {
            id: leaf,
            component: Component { role: ComponentRole::Head, material: "Gold 18k".into(), ..Default::default() },
        },
        CadEdit::Outputs { outputs: doc.outputs.iter().rev().copied().collect() },
        CadEdit::Through { through: Some(first) },
        CadEdit::Through { through: None },
    ]);
    if doc.features.len() > 1 {
        edits.push(CadEdit::Move { id: first, after: Some(leaf) });
        edits.push(CadEdit::Remove { id: leaf });
        edits.push(CadEdit::Enable { id: leaf, enabled: false });
    } else {
        edits.push(CadEdit::Remove { id: leaf });
    }
    match name {
        // Points the stone envelope at the bezel's cylinders; removes the bezel, freeing them and its joint.
        "solitaire" => {
            edits.push(CadEdit::Operation { id: 5, operation: Operation::Boolean { a: 2, b: 3, kind: Boolean::Subtract } });
            edits.push(CadEdit::Remove { id: 4 });
        }
        // Removes the raise, freeing the upper ring, and moves it last.
        "gallery" => {
            edits.push(CadEdit::Remove { id: 3 });
            edits.push(CadEdit::Move { id: 3, after: Some(7) });
        }
        // Removes the shank and its joint to the head.
        "two-part-signet" => edits.push(CadEdit::Remove { id: 1 }),
        // Six claws for four, the stone moved round the ring and suppressed, the head removed, a halo added round the stone.
        "claw-solitaire" => {
            let height = builders::stand_off_mm("claw4", Gem::calibrated(GemCut::Round, 6.5));
            edits.extend([
                CadEdit::Operation { id: 3, operation: Operation::Builder { key: builders::CLAW.into(), on: Some(2), params: json!({ "prongs": 6 }) } },
                CadEdit::Placement { id: 2, placement: Placement::ring(60.0, height) },
                CadEdit::Enable { id: 2, enabled: false },
                CadEdit::Remove { id: 3 },
                CadEdit::Add { feature: builders::feature_on(0, "Halo", builders::HALO, 2, json!({ "melee_mm": 1.2 })), after: Some(3) },
                CadEdit::Operation { id: 4, operation: Operation::Builder { key: builders::BUR.into(), on: Some(2), params: json!({ "through": false }) } },
            ]);
        }
        "band+cylinder" => {
            edits.push(CadEdit::Attach { id: 2, attach: Attach::Cut });
            edits.push(CadEdit::Enable { id: 1, enabled: false });
        }
        // Each pattern, plane and pull edited once, then the prongs suppressed.
        "band+patterns" => {
            let (_, top) = pulled_box();
            edits.extend([
                CadEdit::Operation { id: 7, operation: Operation::Pattern { source: 2, kind: PatternKind::Ring { count: 4, span_deg: 180.0 } } },
                CadEdit::Operation { id: 9, operation: Operation::Pattern { source: 5, kind: PatternKind::Mirror { plane: MirrorPlane::Band } } },
                CadEdit::Operation { id: 6, operation: Operation::Plane { base: PlaneBase::Parting, offset_mm: 0.5 } },
                CadEdit::Add { feature: feature(0, "Over the top", Operation::Plane { base: PlaneBase::Face { feature: 5, face: top.clone() }, offset_mm: 0.2 }), after: Some(5) },
                CadEdit::Operation { id: 10, operation: Operation::PressPull { source: 5, face: top, distance_mm: -0.3 } },
                CadEdit::Enable { id: 8, enabled: false },
            ]);
        }
        _ => {}
    }
    edits
}
fn params() -> BuildParams {
    BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() }
}
/// The design the graph's sink receives, without the field verdict.
fn evaluate(g: &Graph) -> Result<RingDesign, String> {
    let reg = Registry::builtin();
    let lib = AlphaLibrary::builtin();
    let sink = g.nodes.iter().find(|n| n.kind == OUTPUT_KIND).ok_or("no sink")?;
    let w = g.wire_into(sink.id, OUTPUT_DESIGN_PIN).ok_or("sink unwired")?;
    let report = Evaluator::new().evaluate(g, &reg, &lib, 0, Targets::Node(w.from));
    if let Some(e) = report.errors.first() {
        return Err(e.to_string());
    }
    match report.value(w.from, &w.out) {
        Some(Value::Design(d)) => Ok((**d).clone()),
        other => Err(format!("{other:?}: {:?}", report.notes(g))),
    }
}
/// The design's document as the bytes serde writes.
fn cad_bytes(d: &RingDesign) -> String {
    serde_json::to_string(&d.cad).unwrap()
}
/// Renames the ids an `Add` allocated on the document side to the graph side's.
fn renumber(d: &mut RingDesign, pairs: &[(Id, Id)]) {
    let Some(doc) = d.cad.as_mut() else { return };
    let map = |id: &mut Id| {
        if let Some((_, to)) = pairs.iter().find(|(from, _)| from == id) {
            *id = *to;
        }
    };
    for f in &mut doc.features {
        map(&mut f.id);
    }
    doc.outputs.iter_mut().for_each(map);
    if let Some(t) = doc.through.as_mut() {
        map(t);
    }
}
/// The id pair an `Add` produced on each side, when they differ.
fn allocated(plain: &Applied, graph: &Applied) -> Option<(Id, Id)> {
    match (plain.id, graph.id) {
        (Some(p), Some(g)) if p != g => Some((p, g)),
        _ => None,
    }
}

#[test]
fn every_edit_gives_the_same_document_through_the_graph_and_the_document() {
    let lib = AlphaLibrary::builtin();
    let mut count = 0;
    for (name, base) in documents() {
        let doc = base.cad.as_ref().unwrap();
        for edit in edits_for(&name, doc) {
            let mut plain = base.clone();
            let p = plain.apply_cad_edit(&edit).unwrap_or_else(|e| panic!("{name}: {edit:?}: {e}"));
            let mut g = graph_cad::from_document(&base).unwrap();
            let a = graph_cad::apply_edit(&mut g, &edit).unwrap_or_else(|e| panic!("{name}: {edit:?}: {e}"));
            assert_eq!(a.label, p.label, "{name}: {edit:?}");
            let out = evaluate(&g).unwrap_or_else(|e| panic!("{name}: {edit:?}: {e}"));
            let mut expect = plain.clone();
            renumber(&mut expect, &allocated(&p, &a).into_iter().collect::<Vec<_>>());
            assert_eq!(cad_bytes(&expect), cad_bytes(&out), "{name}: {edit:?}");
            // The graph reads back the document it will evaluate to.
            assert_eq!(
                serde_json::to_string(&graph_cad::document(&g).unwrap()).unwrap(),
                serde_json::to_string(&out.cad.clone().unwrap_or_default()).unwrap(),
                "{name}: {edit:?}"
            );
            // Document::apply alone is the plain path, until the document empties and is dropped.
            let mut alone = doc.clone();
            alone.apply(&edit).unwrap();
            match plain.cad.as_ref() {
                Some(pc) => assert_eq!(serde_json::to_string(&alone).unwrap(), serde_json::to_string(pc).unwrap()),
                None => assert!(alone.features.is_empty(), "{name}: {edit:?}"),
            }
            let plain_ok = cad::evaluate(&plain, &lib, params());
            let graph_ok = cad::evaluate(&out, &lib, params());
            assert_eq!(plain_ok.is_ok(), graph_ok.is_ok(), "{name}: {edit:?}: {plain_ok:?} / {graph_ok:?}");
            if plain.cad.is_some() {
                plain_ok.unwrap_or_else(|e| panic!("{name}: after {edit:?}: {e:#}"));
            }
            count += 1;
        }
    }
    assert_eq!(count, 143, "edits swept");
}

#[test]
fn a_sequence_of_edits_keeps_the_two_appliers_in_step() {
    let lib = AlphaLibrary::builtin();
    for (name, base) in documents() {
        let mut plain = base.clone();
        let mut g = graph_cad::from_document(&base).unwrap();
        let mut pairs = Vec::new();
        let (mut landed, mut refused) = (0, 0);
        for edit in edits_for(&name, base.cad.as_ref().unwrap()) {
            let before = g.clone();
            match (plain.apply_cad_edit(&edit), graph_cad::apply_edit(&mut g, &edit)) {
                (Ok(p), Ok(a)) => {
                    pairs.extend(allocated(&p, &a));
                    landed += 1;
                }
                (Err(p), Err(a)) => {
                    // Both refuse with the document's own words, and the graph is left whole.
                    assert_eq!(p.to_string(), a.to_string(), "{name}: {edit:?}");
                    assert_eq!(g, before, "{name}: {edit:?}");
                    refused += 1;
                    continue;
                }
                (p, a) => panic!("{name}: {edit:?}: the appliers disagree: {p:?} / {a:?}"),
            }
            let out = evaluate(&g).unwrap_or_else(|e| panic!("{name}: {edit:?}: {e}"));
            let mut expect = plain.clone();
            renumber(&mut expect, &pairs);
            assert_eq!(cad_bytes(&expect), cad_bytes(&out), "{name}: after {edit:?}");
            let (p, q) = (cad::evaluate(&plain, &lib, params()), cad::evaluate(&out, &lib, params()));
            assert_eq!(p.is_ok(), q.is_ok(), "{name}: after {edit:?}: {p:?} / {q:?}");
        }
        assert!(landed >= 12 && landed + refused == edits_for(&name, base.cad.as_ref().unwrap()).len(), "{name}: {landed} landed, {refused} refused");
    }
}

#[test]
fn a_solitaire_built_round_its_stone_round_trips_byte_for_byte_through_both_appliers() {
    let lib = AlphaLibrary::builtin();
    let base = cad::examples::design("claw-solitaire").unwrap();
    let g = graph_cad::from_document(&base).unwrap();
    // The lift reads back the document it was made from, and evaluates to it, through a JSON round trip too.
    assert_eq!(serde_json::to_string(&graph_cad::document(&g).unwrap()).unwrap(), serde_json::to_string(base.cad.as_ref().unwrap()).unwrap());
    assert_eq!(cad_bytes(&evaluate(&g).unwrap()), cad_bytes(&base));
    let reread: Graph = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(cad_bytes(&evaluate(&reread).unwrap()), cad_bytes(&base));
    // A halo setting's features, added as one sequence on each side, land as the same bytes.
    let gem = Gem::calibrated(GemCut::Round, 6.5);
    let mut n = 10;
    let halo = builders::setting_features("halo", 2, gem, true, &mut || { n += 1; n }).unwrap();
    let mut plain = base.clone();
    let mut g = g;
    for f in halo {
        let edit = CadEdit::Add { feature: f, after: None };
        let (p, a) = (plain.apply_cad_edit(&edit).unwrap(), graph_cad::apply_edit(&mut g, &edit).unwrap());
        assert_eq!((p.id, p.label), (a.id, a.label));
    }
    let out = evaluate(&g).unwrap();
    assert_eq!(cad_bytes(&out), cad_bytes(&plain));
    // Both evaluate to the same parts: the stone, the two heads, the halo and the two burs.
    let parts = |d: &RingDesign| {
        let e = cad::evaluate(d, &lib, params()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        e.components.iter().map(|c| (c.id, c.made.as_ref().map(|m| m.key.clone()), c.mesh.faces.len())).collect::<Vec<_>>()
    };
    let (a, b) = (parts(&plain), parts(&out));
    assert_eq!(a, b);
    let keys: Vec<Option<String>> = a.iter().map(|(_, k, _)| k.clone()).collect();
    let key = |k: &str| Some(k.to_string());
    assert_eq!(keys, [key("stone"), key("head.claw"), key("seat.bur"), key("head.claw"), key("halo"), key("seat.bur")]);
}

#[test]
fn patterns_work_planes_and_a_press_pull_round_trip_byte_for_byte_through_both_appliers() {
    let lib = AlphaLibrary::builtin();
    let base = band_and_patterns();
    let g = graph_cad::from_document(&base).unwrap();
    // The lift reads back the document it was made from, and evaluates to it, through a JSON round trip too.
    assert_eq!(serde_json::to_string(&graph_cad::document(&g).unwrap()).unwrap(), serde_json::to_string(base.cad.as_ref().unwrap()).unwrap());
    assert_eq!(cad_bytes(&evaluate(&g).unwrap()), cad_bytes(&base));
    let reread: Graph = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(cad_bytes(&evaluate(&reread).unwrap()), cad_bytes(&base));
    // Every kind added, edited and removed as one sequence on each side lands as the same bytes.
    let mut plain = base.clone();
    let mut g = g;
    let edits = [
        CadEdit::Add { feature: feature(20, "Half ring", Operation::Pattern { source: 2, kind: PatternKind::Ring { count: 3, span_deg: 180.0 } }), after: None },
        CadEdit::Add { feature: feature(21, "Section at 30°", Operation::Plane { base: PlaneBase::Section { theta_deg: 30.0 }, offset_mm: 0.25 }), after: None },
        CadEdit::Add { feature: feature(22, "Mirror of Prong", Operation::Pattern { source: 4, kind: PatternKind::Mirror { plane: MirrorPlane::Plane { feature: 21 } } }), after: None },
        CadEdit::Add { feature: feature(23, "Mirror through the head", Operation::Pattern { source: 2, kind: PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } }), after: None },
        CadEdit::Operation { id: 20, operation: Operation::Pattern { source: 2, kind: PatternKind::Ring { count: 5, span_deg: 240.0 } } },
        CadEdit::Operation { id: 8, operation: Operation::Pattern { source: 4, kind: PatternKind::About { part: 3, count: 4, span_deg: 360.0 } } },
        CadEdit::Remove { id: 23 },
    ];
    for edit in &edits {
        let (p, a) = (plain.apply_cad_edit(edit).unwrap(), graph_cad::apply_edit(&mut g, edit).unwrap());
        assert_eq!((p.id, &p.label), (a.id, &a.label), "{edit:?}");
    }
    let out = evaluate(&g).unwrap();
    assert_eq!(cad_bytes(&out), cad_bytes(&plain));
    assert_eq!(serde_json::to_string(&graph_cad::document(&g).unwrap()).unwrap(), serde_json::to_string(plain.cad.as_ref().unwrap()).unwrap());
    // Both evaluate to the same parts and planes, every feature built.
    let parts = |d: &RingDesign| {
        let e = cad::evaluate(d, &lib, params()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        (e.components.iter().map(|c| (c.id, c.mesh.faces.len())).collect::<Vec<_>>(), e.planes.iter().map(|p| p.id).collect::<Vec<_>>())
    };
    let (a, b) = (parts(&plain), parts(&out));
    assert_eq!(a, b);
    assert_eq!(a.0.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![2, 3, 4, 7, 8, 9, 10, 20, 22]);
    assert_eq!(a.1, vec![6, 21]);
    // A plane cannot be an output on either side.
    let refused = CadEdit::Outputs { outputs: vec![2, 6] };
    let (p, q) = (plain.apply_cad_edit(&refused).unwrap_err().to_string(), graph_cad::apply_edit(&mut g, &refused).unwrap_err().to_string());
    assert_eq!((p.as_str(), q.as_str()), ("#6 Section at 90° is a work plane and has no body to output", "#6 Section at 90° is a work plane and has no body to output"));
}

#[test]
fn a_cut_below_its_sketch_round_trips_byte_for_byte_through_both_appliers() {
    use ringdesign_core::{cad::Profile, sketch::Sketch};
    let lib = AlphaLibrary::builtin();
    let base = band_and_cylinder();
    // A 2 × 1.5 rectangle on the bezel's plane at its top, cut 1 mm down into it, then 0.6, joined, then cut again and turned half round a line beside it.
    let mut rect = Sketch::rectangle(2.0, 1.5);
    let e = cad::evaluate(&base, &lib, params()).unwrap();
    let frame = e.components.iter().find(|c| c.id == 2).unwrap().frame;
    rect.plane.origin = std::array::from_fn(|k| frame.origin[k] + frame.z_axis[k] * 1.25);
    (rect.plane.x, rect.plane.y) = (frame.x_axis, frame.y_axis);
    let mut cut = feature(21, "Extrude cut", Operation::Extrude { sketch: Profile::Feature { feature: 20 }, height_mm: -1.0, draft_deg: 3.0 });
    cut.component.attach = Attach::Cut;
    let pivot = std::array::from_fn(|k| rect.plane.origin[k] - frame.x_axis[k] * 1.5);
    let mut turn = feature(22, "Revolve cut", Operation::Revolve { sketch: Profile::Feature { feature: 20 }, pivot, axis: frame.y_axis, degrees: 180.0 });
    turn.component.attach = Attach::Cut;
    let edits = [
        CadEdit::Add { feature: feature(20, "Sketch", Operation::Sketch { sketch: rect }), after: None },
        CadEdit::Add { feature: cut, after: None },
        CadEdit::Operation { id: 21, operation: Operation::Extrude { sketch: Profile::Feature { feature: 20 }, height_mm: -0.6, draft_deg: 3.0 } },
        CadEdit::Attach { id: 21, attach: Attach::Join },
        CadEdit::Attach { id: 21, attach: Attach::Cut },
        CadEdit::Add { feature: turn, after: None },
    ];
    let mut plain = base.clone();
    let mut g = graph_cad::from_document(&base).unwrap();
    for edit in &edits {
        let (p, a) = (plain.apply_cad_edit(edit).unwrap(), graph_cad::apply_edit(&mut g, edit).unwrap());
        assert_eq!((p.id, &p.label), (a.id, &a.label), "{edit:?}");
        let out = evaluate(&g).unwrap();
        assert_eq!(cad_bytes(&out), cad_bytes(&plain), "{edit:?}");
    }
    assert!(cad_bytes(&plain).contains(r#""height_mm":-0.6"#), "the height keeps its sign");
    let reread: Graph = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(cad_bytes(&evaluate(&reread).unwrap()), cad_bytes(&plain));
    // Both build the same parts, the cuts carving the bezel.
    let parts = |d: &RingDesign| {
        let e = cad::evaluate(d, &lib, params()).unwrap();
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        e.components.iter().map(|c| (c.id, c.attach, c.mesh.faces.len())).collect::<Vec<_>>()
    };
    let (a, b) = (parts(&plain), parts(&evaluate(&g).unwrap()));
    assert_eq!(a, b);
    assert_eq!(a.iter().map(|(id, attach, _)| (*id, *attach)).collect::<Vec<_>>(), [(2, Attach::Join), (21, Attach::Cut), (22, Attach::Cut)]);
}

#[test]
fn a_part_appended_after_the_lifts_property_nodes_still_edits_to_the_same_document() {
    let base = cad::examples::design("solitaire").unwrap();
    let mut g = graph_cad::from_document(&base).unwrap();
    // Appends a part after the lift's property nodes, as the CAD pane's add path does.
    let post = graph_cad::append(&mut g, Operation::Cylinder { radius_mm: 0.5, height_mm: 3.0 }).unwrap().0;
    let doc = graph_cad::document(&g).unwrap();
    assert_eq!(doc.outputs, vec![1, 4, 5, post]);
    assert_eq!(cad_bytes(&evaluate(&g).unwrap()), serde_json::to_string(&Some(&doc)).unwrap());
    for edit in [
        CadEdit::Rename { id: 1, name: "Hoop".into() },
        CadEdit::Remove { id: post },
        CadEdit::Move { id: post, after: Some(1) },
        CadEdit::Add { feature: cylinder("Pin"), after: None },
        CadEdit::Outputs { outputs: vec![post, 1] },
        CadEdit::Remove { id: 4 },
    ] {
        let mut expect = doc.clone();
        let p = expect.apply(&edit).unwrap();
        let mut edited = g.clone();
        let a = graph_cad::apply_edit(&mut edited, &edit).unwrap();
        let mut expect = RingDesign { cad: Some(expect), ..Default::default() };
        renumber(&mut expect, &allocated(&p, &a).into_iter().collect::<Vec<_>>());
        assert_eq!(cad_bytes(&expect), cad_bytes(&evaluate(&edited).unwrap()), "{edit:?}");
    }
}

#[test]
fn both_appliers_refuse_the_same_edits_and_name_the_features() {
    let solitaire = cad::examples::design("solitaire").unwrap();
    let band = band_and_cylinder();
    let mut second_band = feature(0, "Second shank", Operation::Band);
    second_band.component.role = ComponentRole::Shank;
    let cases: Vec<(&RingDesign, CadEdit, &str)> = vec![
        (&solitaire, CadEdit::Remove { id: 2 }, "Remove #2 Setting stock: 1 depends on it: #4 Open bezel stock"),
        (&solitaire, CadEdit::Move { id: 4, after: None }, "before its source #2 Setting stock"),
        (&solitaire, CadEdit::Move { id: 2, after: Some(4) }, "after #4 Open bezel stock, which depends on it"),
        (&solitaire, CadEdit::Operation { id: 2, operation: Operation::Boolean { a: 4, b: 3, kind: Boolean::Union } }, "would form a cycle"),
        (&solitaire, CadEdit::Operation { id: 4, operation: Operation::Transform { source: 5, translation: [0.0; 3], rotation_deg: [0.0; 3] } }, "comes after it"),
        (&solitaire, CadEdit::Add { feature: cylinder("x"), after: Some(99) }, "no such feature"),
        (&solitaire, CadEdit::Enable { id: 99, enabled: true }, "No feature #99"),
        (&solitaire, CadEdit::Rename { id: 99, name: "x".into() }, "No feature #99"),
        (&solitaire, CadEdit::Outputs { outputs: vec![1, 99] }, "No feature #99"),
        (&solitaire, CadEdit::Through { through: Some(99) }, "No feature #99"),
        (&solitaire, CadEdit::Blend { id: 5, blend_mm: -1.0 }, "not negative"),
        (&band, CadEdit::Add { feature: second_band, after: None }, "one procedural shank"),
    ];
    for (design, edit, expected) in cases {
        let e = design.cad.clone().unwrap().apply(&edit).unwrap_err().to_string();
        assert!(e.contains(expected), "document: {edit:?}: {e}");
        let before = graph_cad::from_document(design).unwrap();
        let mut g = before.clone();
        let e = graph_cad::apply_edit(&mut g, &edit).unwrap_err().to_string();
        assert!(e.contains(expected), "graph: {edit:?}: {e}");
        assert_eq!(g, before, "a refused edit leaves the graph as it was: {edit:?}");
    }
}

#[test]
fn the_funnel_edits_a_driven_design_on_its_graph_and_a_plain_one_on_its_document() {
    let base = cad::examples::design("solitaire").unwrap();
    let rename = CadEdit::Rename { id: 1, name: "Hoop".into() };
    let mut driven = base.clone();
    driven.graph = Some(serde_json::to_value(graph_cad::from_document(&base).unwrap()).unwrap());
    let e = driven.apply_cad_edit(&rename).unwrap_err();
    assert!(e.to_string().contains("edit the graph"), "{e}");
    let a = graph_cad::edit_design(&mut driven, &rename).unwrap();
    assert_eq!(a.label, "Rename Shank");
    assert_eq!(driven.cad.as_ref().unwrap().features[0].name, "Shank", "the document waits for the worker");
    let g: Graph = serde_json::from_value(driven.graph.clone().unwrap()).unwrap();
    assert_eq!(evaluate(&g).unwrap().cad.unwrap().features[0].name, "Hoop");
    // An add on a driven design is named by the node made for it.
    let a = graph_cad::edit_design(&mut driven, &CadEdit::Add { feature: cylinder("Pin"), after: Some(1) }).unwrap();
    let g: Graph = serde_json::from_value(driven.graph.clone().unwrap()).unwrap();
    let pin = a.id.unwrap();
    assert!(g.node(ringdesign_graph::graph::NodeId(pin)).is_some_and(|n| n.kind == "cad.feature"));
    let doc = evaluate(&g).unwrap().cad.unwrap();
    assert_eq!(doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![1, pin, 2, 3, 4, 5]);
    // A refused edit leaves the stored graph untouched.
    let stored = driven.graph.clone();
    assert!(graph_cad::edit_design(&mut driven, &CadEdit::Remove { id: 2 }).is_err());
    assert_eq!(driven.graph, stored);
    let mut plain = base.clone();
    let a = graph_cad::edit_design(&mut plain, &rename).unwrap();
    assert_eq!(a.label, "Rename Shank");
    assert_eq!(plain.cad.as_ref().unwrap().features[0].name, "Hoop");
    assert!(plain.graph.is_none());
    // Removing with dependents is the same leaf-first sequence on both sides.
    let mut doc = base.cad.clone().unwrap();
    let mut g = graph_cad::from_document(&base).unwrap();
    assert_eq!(doc.remove_with_dependents(2).unwrap(), vec![4, 2]);
    assert_eq!(graph_cad::remove_with_dependents(&mut g, 2).unwrap(), vec![4, 2]);
    assert_eq!(serde_json::to_string(&Some(&doc)).unwrap(), cad_bytes(&evaluate(&g).unwrap()));
    assert_eq!(doc.outputs, vec![1, 5, 3]);
}

/// Mean time of `run` over `n` calls, each on a fresh copy from `setup` made outside the clock.
fn per_call<T>(n: usize, setup: impl Fn() -> T, mut run: impl FnMut(&mut T)) -> f64 {
    let mut total = Duration::ZERO;
    for _ in 0..n {
        let mut v = setup();
        let t = Instant::now();
        run(&mut v);
        total += t.elapsed();
        std::hint::black_box(v);
    }
    total.as_secs_f64() * 1e6 / n as f64
}

/// Apply cost per edit on the largest example, and the lift's cost; run with `--nocapture`.
#[test]
fn measure_apply_cost() {
    let base = cad::examples::design("gallery").unwrap();
    let doc = base.cad.clone().unwrap();
    let g = graph_cad::from_document(&base).unwrap();
    let mut driven = base.clone();
    driven.graph = Some(serde_json::to_value(&g).unwrap());
    let lift_us = per_call(200, || (), |_| {
        std::hint::black_box(graph_cad::from_document(&base).unwrap());
    });
    let snapshot_us = per_call(200, || (), |_| {
        std::hint::black_box(g.clone());
    });
    eprintln!("gallery, {} features: from_document {lift_us:.1} us, graph clone {snapshot_us:.1} us", doc.features.len());
    eprintln!("{:<22} {:>16} {:>12} {:>14}", "edit", "Document::apply", "apply_edit", "edit_design");
    for edit in edits_for("gallery", &doc) {
        let doc_us = per_call(2000, || doc.clone(), |d| {
            d.apply(&edit).unwrap();
        });
        let graph_us = per_call(200, || g.clone(), |g| {
            graph_cad::apply_edit(g, &edit).unwrap();
        });
        let design_us = per_call(200, || driven.clone(), |d| {
            graph_cad::edit_design(d, &edit).unwrap();
        });
        eprintln!("{:<22} {doc_us:>13.2} us {graph_us:>9.1} us {design_us:>11.1} us", edit.label());
        assert!(doc_us < 1_000.0 && graph_us < 20_000.0 && design_us < 50_000.0);
    }
}
