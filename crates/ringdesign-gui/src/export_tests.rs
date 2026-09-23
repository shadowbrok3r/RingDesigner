//! Parts into the app and the ring out of it: File > Import part and Export STEP, driven on the app.
use crate::app::RingDesignerApp;
use crate::interaction_tests::{harness, wait_for_build};
use egui_kittest::Harness;
use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement};
use ringdesign_core::{BuildParams, Mesh, RingDesign, Vec3};
use ringdesign_workbench::viewport::Sel;

fn court() -> RingDesign {
    ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
}

/// A closed `n`-sided cylinder of radius `r` along z from `z0` to `z0 + h`, wound outward.
fn cylinder_mesh(r: f64, h: f64, z0: f64, n: u32) -> Mesh {
    let at = |k: u32, z: f64| {
        let a = f64::from(k) * std::f64::consts::TAU / f64::from(n);
        Vec3((r * a.cos()) as f32, (r * a.sin()) as f32, z as f32)
    };
    let mut m = Mesh { vertices: vec![Vec3(0.0, 0.0, z0 as f32), Vec3(0.0, 0.0, (z0 + h) as f32)], ..Default::default() };
    for k in 0..n {
        m.vertices.push(at(k, z0));
        m.vertices.push(at(k, z0 + h));
    }
    for k in 0..n {
        let (b0, t0, b1, t1) = (2 + 2 * k, 3 + 2 * k, 2 + 2 * ((k + 1) % n), 3 + 2 * ((k + 1) % n));
        m.faces.extend([[0, b1, b0], [1, t0, t1], [b0, b1, t1], [b0, t1, t0]]);
    }
    m
}

/// A scratch directory of the test's own.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ringdesign-gui-{name}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The app showing `design`, built at a quick sweep and settled in its history.
fn showing(h: &mut Harness<'static, RingDesignerApp>, design: RingDesign) {
    {
        let app = h.state_mut();
        app.preview_params = BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        app.export_params = app.preview_params;
        app.design = design;
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(h);
}

fn doc(h: &Harness<'static, RingDesignerApp>) -> Document {
    h.state().design.cad.clone().expect("a document")
}

#[test]
fn the_file_and_export_menus_and_the_palette_offer_the_import_and_step() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    h.run_steps(3);
    h.get_by_label("File").click();
    h.run_steps(3);
    assert!(h.query_by_label_contains("Import part…").is_some() && h.query_by_label_contains("Export STEP…").is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label_contains("Import part…").is_none(), "the File menu closed");
    h.get_by_label("Export").click();
    h.run_steps(3);
    assert!(h.query_by_label_contains("Export STEP…").is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.state_mut().palette_open = true;
    h.state_mut().palette_query = "step".into();
    h.run_steps(4);
    assert!(h.query_by_label("Export STEP…").is_some() && h.query_by_label("Import part (STL, OBJ, STEP)…").is_some());
}

#[test]
fn an_stl_post_imports_joined_at_the_top_as_one_history_entry_and_chosen() {
    let dir = scratch("import");
    let mut h = harness();
    showing(&mut h, court());
    let bare = h.state().build.as_ref().unwrap().report.volume_mm3;
    let start = h.state().history.present();
    let post = cylinder_mesh(0.8, 2.0, -0.02, 64);
    let stl = dir.join("post.stl");
    ringdesign_core::stl::write_stl(&stl, &post, "post").unwrap();
    crate::export::import_part_path(h.state_mut(), &stl);
    // The plain band brings its shank with the part, one undo step, and the post is the chosen part.
    assert_eq!(h.state().history.present(), start + 1, "{}", h.state().status);
    let d = doc(&h);
    assert_eq!(d.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Procedural shank", "post"]);
    let part = d.features[1].clone();
    let Operation::Stored { recipe, sources, .. } = &part.operation else { panic!("{:?}", part.operation) };
    assert_eq!((recipe.op.as_str(), recipe.params["file"].as_str(), sources.len()), ("import", Some("post.stl"), 0));
    assert_eq!((part.component.attach, &part.component.placement), (Attach::Join, &Placement::ring(90.0, 0.0)));
    assert_eq!(h.state().selection.items, vec![Sel::Part(part.id)], "chosen, so G, R and the gizmo take it");
    assert!(h.state().status.starts_with("Imported post at the top of the ring, joined"), "{}", h.state().status);
    // Built, it is joined to the band and the ring grows by the post to within a percent.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let built = h.state().build.clone().unwrap();
    assert!(built.report.validation.watertight && built.parts.joined == 1, "{:?} {:?}", built.report.validation, built.parts.notes);
    let grown = built.report.volume_mm3 - bare;
    assert!((grown - post.volume_mm3()).abs() < 0.01 * post.volume_mm3(), "{grown} against {}", post.volume_mm3());
    // An open STL is refused by name and takes no undo step.
    let mut open = post.clone();
    open.faces.pop();
    let bad = dir.join("open.stl");
    ringdesign_core::stl::write_stl(&bad, &open, "open").unwrap();
    let present = h.state().history.present();
    crate::export::import_part_path(h.state_mut(), &bad);
    assert_eq!(h.state().history.present(), present);
    assert!(h.state().status.contains("open.stl is not a closed solid: 3 open and 0 non-manifold edges"), "{}", h.state().status);
    // One Undo takes the whole import back, shank and all.
    h.state_mut().undo();
    assert!(h.state().design.cad.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_step_writes_the_ring_off_the_ui_thread_and_it_imports_back() {
    let dir = scratch("step");
    let mut h = harness();
    let mut d = court();
    d.name = "Court".into();
    let mut parts = Document::default();
    parts.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    let post = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.9), ..Component::default() };
    parts.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.0 }, component: post }).unwrap();
    d.cad = Some(parts);
    showing(&mut h, d.clone());
    let step = dir.join("court.step");
    crate::export::export_step_to(h.state_mut(), step.clone());
    let started = std::time::Instant::now();
    while h.state().exporting.is_some() {
        h.run_steps(2);
        assert!(started.elapsed() < std::time::Duration::from_secs(60), "the STEP export never finished");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let status = h.state().status.clone();
    assert!(status.starts_with(&format!("Wrote {} • 1 exact and 1 faceted solids", step.display())), "{status}");
    let solids = ringdesign_core::cad::step::read_solids(&std::fs::read_to_string(&step).unwrap()).unwrap();
    assert_eq!(solids.iter().map(|s| (s.name.as_str(), s.faceted)).collect::<Vec<_>>(), [("Post", false), ("Court", true)]);
    // Imported back, the faceted band is one stored part and the exact post is named as left for OpenCascade.
    let start = h.state().history.present();
    crate::export::import_part_path(h.state_mut(), &step);
    assert_eq!(h.state().history.present(), start + 1, "{}", h.state().status);
    assert!(h.state().status.ends_with("court.step: the exact solid Post reads only where OpenCascade is, and was left out"), "{}", h.state().status);
    let d = doc(&h);
    let back = d.features.last().unwrap();
    let Operation::Stored { mesh, recipe, .. } = &back.operation else { panic!() };
    assert_eq!((back.name.as_str(), recipe.params["format"].as_str()), ("court", Some("step")));
    let (packed, faceted) = (mesh.made().unwrap().solid().volume(), solids[1].mesh.as_ref().unwrap().volume_mm3());
    assert!((packed - faceted).abs() < 5e-3 * faceted, "{packed} against {faceted}");
    let _ = std::fs::remove_dir_all(&dir);
}
