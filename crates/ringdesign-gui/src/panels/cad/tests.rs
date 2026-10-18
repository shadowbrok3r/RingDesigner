use super::*;
use egui_kittest::Harness;

pub(crate) fn stored_body() -> RingDesign {
    let mesh = cad::stored::Packed::encode(
        &[[0., 0., 0.], [4., 0., 0.], [0., 4., 0.], [0., 0., 4.]],
        &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        &[0; 4], &[cad::SurfaceKind::Freeform],
    ).unwrap();
    let mut feature = cad::stored::imported_packed("body.stl", "stl", Default::default(), mesh).unwrap();
    feature.id = 1;
    let mut doc = cad::Document::default();
    doc.append(feature).unwrap();
    RingDesign { cad: Some(doc), ..Default::default() }
}

fn harness(design: RingDesign) -> Harness<'static, RingDesignerApp> {
    Harness::builder().with_size([1600., 980.]).build_eframe(move |cc| {
        let mut app = RingDesignerApp::new(cc);
        app.updater.automatic = false;
        app.auto_rebuild = false;
        app.design = design;
        app.sync_graph();
        app.history.reset(&app.design);
        app.mark_dirty();
        app.focus(PaneKind::Cad);
        app
    })
}

fn settled(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    loop {
        h.run_steps(2);
        let s = &h.state().cad;
        assert!(s.error.is_none(), "{:?}", s.error);
        if s.view.is_some() && s.view_key == s.observed && s.job.is_none() { break; }
        assert!(start.elapsed() < std::time::Duration::from_secs(30), "CAD preview did not settle");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn stored_candidate_edits_and_undo_preserve_the_mesh_in_plain_and_driven_designs() {
    for driven in [false, true] {
        let mut design = stored_body();
        if driven {
            design.graph = Some(serde_json::to_value(graph_cad::from_document(&design).unwrap()).unwrap());
        }
        let original = serde_json::to_value(&design.cad.as_ref().unwrap().features[0].operation).unwrap();
        let mut h = harness(design);
        settled(&mut h);
        let key = h.state().cad.view_key;
        h.run_steps(5);
        assert_eq!(h.state().cad.view_key, key, "idle frames reuse the candidate");
        h.state_mut().cad.request(CadRequest::Add {
            operation: Operation::Sphere { radius_mm: 1.0 }, anchor: None,
        });
        settled(&mut h);
        assert!(h.state().cad.draft.is_some());
        h.state_mut().cad.request(CadRequest::Apply);
        h.run_steps(3);
        let app = h.state();
        assert_eq!(app.design.cad.as_ref().unwrap().features.len(), 2);
        assert_eq!(serde_json::to_value(&app.design.cad.as_ref().unwrap().features[0].operation).unwrap(), original);
        let text = app.session_design().unwrap();
        assert!(!text.contains("@cad-mesh:"), "local payload tokens never reach a saved design");
        let reopened = ringdesign_core::library::load_design_str(&text).unwrap();
        assert_eq!(serde_json::to_value(&reopened.cad.as_ref().unwrap().features[0].operation).unwrap(), original);
        h.state_mut().undo();
        h.run_steps(3);
        assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), 1);
        assert_eq!(serde_json::to_value(&h.state().design.cad.as_ref().unwrap().features[0].operation).unwrap(), original);
    }
}

#[test]
fn compact_meshes_survive_graph_copies_and_reject_missing_payloads() {
    let mut graph = graph_cad::from_document(&stored_body()).unwrap();
    // Payload size must not determine the cost of copying or hashing the UI graph.
    graph.nodes[1].params["operation"]["Stored"]["mesh"]["data"] = "A".repeat(1_000_000).into();
    // Stored geometry can also occur inside a nested graph/patch value.
    graph.nodes[0].params["nested"] = serde_json::to_value(&graph.nodes[1].params).unwrap();
    let mut meshes = payload::Meshes::default();
    let compact = meshes.compact(graph.clone()).unwrap();
    assert!(serde_json::to_vec(&compact).unwrap().len() < 10_000);
    assert_eq!(meshes.expand(&compact.clone()).unwrap(), graph);
    assert_eq!(meshes.compact(compact.clone()).unwrap(), compact, "compaction is idempotent");
    assert!(payload::Meshes::default().expand(&compact).is_err());
}

/// Run against the real cuff with RINGDESIGNER_CAD_PERF=/absolute/design.ring.json.
/// Measures UI work only; hold the evaluator so build time cannot skew frame time.
#[test]
#[ignore]
fn large_stored_body_frame_time() {
    let path = std::env::var("RINGDESIGNER_CAD_PERF").expect("set RINGDESIGNER_CAD_PERF");
    let mut h = harness(ringdesign_core::library::load_design(path).unwrap());
    h.state_mut().cad.workers.store(MAX_WORKERS, Ordering::Relaxed);
    h.run_steps(3);
    let start = std::time::Instant::now();
    h.run_steps(10);
    eprintln!("CAD frame mean: {:.3} ms", start.elapsed().as_secs_f64() * 100.0);
}
