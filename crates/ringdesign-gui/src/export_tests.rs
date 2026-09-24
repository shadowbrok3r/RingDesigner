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
    // The line says the file's size, the band's facets and how near every vertex of the export build stands to them.
    let band = solids[1].mesh.as_ref().unwrap();
    let size = crate::export::size_words(std::fs::metadata(&step).unwrap().len() as usize);
    assert!(status.contains(&format!(" • {size} • band {} facets from ", crate::export::grouped(band.faces.len()))), "{status}");
    let within: f64 = status.split("every vertex of the export build within ").nth(1).and_then(|s| s.strip_suffix(" mm")).and_then(|s| s.parse().ok()).unwrap_or_else(|| panic!("{status}"));
    assert!(within <= ringdesign_core::cad::step::BAND_TOLERANCE_MM, "{status}");
    let built = ringdesign_core::cad::step::read_solids(&ringdesign_core::cad::step::ring(&d, &h.state().lib, h.state().export_params, "Court").unwrap()).unwrap();
    let as_built = built[1].mesh.as_ref().unwrap();
    assert!(band.faces.len() * 2 < as_built.faces.len() && band.validate().watertight, "{} of {} facets", band.faces.len(), as_built.faces.len());
    assert!((band.volume_mm3() / as_built.volume_mm3() - 1.0).abs() < 0.01, "{} against {}", band.volume_mm3(), as_built.volume_mm3());
    // Imported back, the faceted band and the exact post come in together as one stored part, from the slot when the file is big.
    let start = h.state().history.present();
    crate::export::import_part_path(h.state_mut(), &step);
    let waited = std::time::Instant::now();
    while h.state().importing.is_some() {
        h.run_steps(1);
        assert!(waited.elapsed() < std::time::Duration::from_secs(60), "the STEP never landed");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(h.state().history.present(), start + 1, "{}", h.state().status);
    assert!(h.state().status.ends_with("Imported court at the top of the ring, joined: G moves it, R turns it"), "{}", h.state().status);
    let d = doc(&h);
    let back = d.features.last().unwrap();
    let Operation::Stored { mesh, recipe, .. } = &back.operation else { panic!() };
    assert_eq!((back.name.as_str(), recipe.params["format"].as_str()), ("court", Some("step")));
    let meshed = ringdesign_core::cad::step::read_meshes(&std::fs::read_to_string(&step).unwrap()).unwrap();
    let whole: f64 = meshed.iter().map(|m| m.mesh.as_ref().unwrap().volume_mm3()).sum();
    let packed = mesh.made().unwrap().solid().volume();
    assert!((packed - whole).abs() < 5e-3 * whole, "{packed} against {whole}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A closed post of radius `r` and height `h` from `z0`, `around` sides and `rows` rings of them up its wall, wound outward.
fn post_grid(r: f64, h: f64, z0: f64, around: u32, rows: u32) -> Mesh {
    let at = |k: u32, j: u32| {
        let a = f64::from(k) * std::f64::consts::TAU / f64::from(around);
        Vec3((r * a.cos()) as f32, (r * a.sin()) as f32, (z0 + h * f64::from(j) / f64::from(rows)) as f32)
    };
    let mut m = Mesh { vertices: vec![Vec3(0.0, 0.0, z0 as f32), Vec3(0.0, 0.0, (z0 + h) as f32)], ..Default::default() };
    for j in 0..=rows {
        for k in 0..around {
            m.vertices.push(at(k, j));
        }
    }
    let v = |k: u32, j: u32| 2 + j * around + k % around;
    for k in 0..around {
        m.faces.push([0, v(k + 1, 0), v(k, 0)]);
        m.faces.push([1, v(k, rows), v(k + 1, rows)]);
        for j in 0..rows {
            m.faces.push([v(k, j), v(k + 1, j), v(k + 1, j + 1)]);
            m.faces.push([v(k, j), v(k + 1, j + 1), v(k, j + 1)]);
        }
    }
    m
}

#[test]
fn a_part_file_over_a_megabyte_is_read_off_the_ui_thread_and_one_under_it_on_the_spot() {
    use egui_kittest::kittest::Queryable;
    let dir = scratch("big-import");
    let mut h = harness();
    showing(&mut h, court());
    let start = h.state().history.present();
    // 102,912 facets: a 5 MB STL, where a megabyte reads in 13 ms or less in release in any format.
    let post = post_grid(0.8, 2.0, -0.02, 256, 200);
    let big = dir.join("post.stl");
    ringdesign_core::stl::write_stl(&big, &post, "post").unwrap();
    let bytes = std::fs::metadata(&big).unwrap().len();
    assert!(bytes > 4 * crate::export::SYNC_IMPORT_BYTES, "{bytes}");
    let asked = std::time::Instant::now();
    crate::export::import_part_path(h.state_mut(), &big);
    // It takes the slot at once: nothing is imported yet, and the status line says what reads what.
    assert!(asked.elapsed() < std::time::Duration::from_millis(100), "asking returns at once: {:?}", asked.elapsed());
    assert_eq!(h.state().importing.as_ref().map(|p| (p.file.as_str(), p.reader)), Some(("post.stl", "the STL reader")));
    assert!(h.state().status.starts_with("Reading post.stl in the STL reader… "), "{}", h.state().status);
    assert_eq!(h.state().history.present(), start);
    let (mut frames, mut plate) = (0, false);
    while h.state().importing.is_some() {
        h.run_steps(1);
        frames += 1;
        plate |= h.state().importing.is_some() && h.query_by_label("Cancel import").is_some();
        assert!(asked.elapsed() < std::time::Duration::from_secs(60), "the part never landed");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(plate && frames > 1, "the window drew {frames} frames with the plate up while it read");
    // It lands joined at the top and chosen, one undo step.
    let d = doc(&h);
    assert_eq!(d.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Procedural shank", "post"]);
    assert_eq!(h.state().history.present(), start + 1);
    assert_eq!(h.state().selection.items, vec![Sel::Part(d.features[1].id)]);
    assert!(h.state().status.starts_with("Imported post at the top of the ring, joined"), "{}", h.state().status);
    // Cancelled, whatever the reader finds is dropped: no undo step, no part.
    let again = dir.join("again.stl");
    std::fs::copy(&big, &again).unwrap();
    crate::export::import_part_path(h.state_mut(), &again);
    h.run_steps(1);
    h.get_by_label("Cancel import").click();
    h.run_steps(2);
    assert!(h.state().importing.is_none());
    assert_eq!(h.state().status, "Stopped reading again.stl: nothing was imported");
    std::thread::sleep(std::time::Duration::from_millis(1500));
    h.run_steps(3);
    assert_eq!((h.state().history.present(), doc(&h).features.len()), (start + 1, 2));
    // A file under the megabyte lands on the spot, as it always did.
    let small = dir.join("small.stl");
    ringdesign_core::stl::write_stl(&small, &cylinder_mesh(0.5, 1.0, -0.02, 32), "small").unwrap();
    crate::export::import_part_path(h.state_mut(), &small);
    assert!(h.state().importing.is_none());
    assert_eq!((h.state().history.present(), doc(&h).features.len()), (start + 2, 3));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each reader's time on part files of rising size: `cargo test --release -p ringdesign-gui -- --ignored import_read_times --nocapture`.
#[test]
#[ignore]
fn import_read_times() {
    let dir = scratch("read-times");
    let time = |path: &std::path::Path| {
        let bytes = std::fs::metadata(path).unwrap().len() as f64 / 1048576.0;
        let started = std::time::Instant::now();
        let read = ringdesign_mcp::import::part_file(path);
        let ms = started.elapsed().as_secs_f64() * 1e3;
        assert!(read.is_ok(), "{}: {:?}", path.display(), read.err());
        eprintln!("{}: {bytes:.2} MB in {ms:.1} ms, {:.1} ms a MB", path.file_name().unwrap().to_string_lossy(), ms / bytes);
    };
    let lib = ringdesign_core::AlphaLibrary::builtin();
    for (t, p) in [(48, 32), (96, 48), (192, 96), (384, 144), (512, 192), (1024, 320)] {
        let params = BuildParams { theta_steps: t, profile_steps: p, refine: None, ..Default::default() };
        let ring = ringdesign_core::mesh::build(&court(), &lib, params).mesh;
        let stl = dir.join(format!("court-{t}x{p}.stl"));
        ringdesign_core::stl::write_stl(&stl, &ring, "court").unwrap();
        time(&stl);
        let obj = dir.join(format!("court-{t}x{p}.obj"));
        ringdesign_core::stl::write_obj(&obj, &ring, "court").unwrap();
        time(&obj);
        let text = ringdesign_core::cad::step::ring(&court(), &lib, params, "Court").unwrap();
        let step = dir.join(format!("court-{t}x{p}.step"));
        std::fs::write(&step, text).unwrap();
        time(&step);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
