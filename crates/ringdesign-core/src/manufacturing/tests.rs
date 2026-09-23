use super::release::Status;
use super::*;

fn params() -> BuildParams {
    BuildParams {
        theta_steps: 128,
        profile_steps: 96,
        refine: None,
        ..Default::default()
    }
}

#[test]
fn old_documents_and_new_setups_round_trip() {
    let d = RingDesign::default();
    let text = crate::library::design_json(&d).unwrap();
    assert!(!text.contains("manufacturing"));
    assert!(
        crate::library::load_design_str(&text)
            .unwrap()
            .manufacturing
            .is_none()
    );
    let mut d = d;
    d.manufacturing = Some(Setup::default());
    let new = crate::library::load_design_str(&crate::library::design_json(&d).unwrap()).unwrap();
    assert_eq!(
        new.manufacturing.unwrap().recipe.name,
        Setup::default().recipe.name
    );
}

#[test]
fn a_pattern_rebuilds_embedded_art_even_with_an_empty_session_library() {
    let mut d = RingDesign::default();
    let mut lib = AlphaLibrary::default();
    lib.insert(crate::Alpha::new("Embedded proof", 2, 2, vec![0.8; 4]));
    let tile = crate::tiling::TilingLayer::default_for("Embedded proof", &d.field_context());
    d.layers.layers.push(crate::field::LayerEntry::new(
        "Embedded ornament",
        Layer::Tiling(tile),
    ));
    d.embed_alphas(&lib);
    assert!(!d.embedded.is_empty());
    let setup = Setup::default();
    let expected = prepare(&d, &lib, &setup, params()).unwrap();
    let portable = prepare(&d, &AlphaLibrary::default(), &setup, params()).unwrap();
    assert_eq!(expected.mesh.vertices, portable.mesh.vertices);
    assert!(
        portable.mesh.volume_mm3()
            > prepare(
                &RingDesign::default(),
                &AlphaLibrary::default(),
                &setup,
                params()
            )
            .unwrap()
            .mesh
            .volume_mm3()
    );
    let path = std::env::temp_dir().join(format!(
        "ring-portable-pattern-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    package::export(&path, &d, &AlphaLibrary::default(), &setup, params(), true).unwrap();
    let reloaded = crate::library::load_design(path.join("design.ring.json")).unwrap();
    assert_eq!(reloaded.embedded.len(), d.embedded.len());
    let rebuilt = prepare(&reloaded, &AlphaLibrary::default(), &setup, params()).unwrap();
    assert_eq!(portable.mesh.vertices, rebuilt.mesh.vertices);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn repair_comparison_uses_measured_candidates_without_mutating_the_design() {
    let mut d = RingDesign::default();
    let ctx = d.field_context();
    let rail = crate::curve::CurveLayer {
        height_mm: 0.9,
        width_mm: 0.35,
        points: vec![
            [0.0, ctx.crest_v_mm - 0.4],
            [0.5, ctx.crest_v_mm + 0.4],
            [1.0, ctx.crest_v_mm - 0.4],
        ],
        ..Default::default()
    };
    d.layers.layers.push(crate::field::LayerEntry::new(
        "Test rail",
        Layer::Curve(rail),
    ));
    let before = serde_json::to_string(&d).unwrap();
    let rows = repair::rank(
        &d,
        &AlphaLibrary::builtin(),
        &Setup::default(),
        Some(0),
        params(),
    )
    .unwrap();
    let bench = rows
        .iter()
        .find(|r| r.repair == repair::Repair::DeferToBench)
        .unwrap();
    assert_eq!(bench.obstructions, 0);
    assert!(rows.len() >= 3);
    assert_eq!(before, serde_json::to_string(&d).unwrap());
}

#[test]
fn nominal_geometry_is_not_scaled_or_modified_when_preparing_patterns() {
    let d = RingDesign::default();
    let lib = AlphaLibrary::builtin();
    let mut s = Setup::default();
    s.recipe.shrink_pct = 2.0;
    let before = serde_json::to_string(&d).unwrap();
    let p = prepare(&d, &lib, &s, params()).unwrap();
    let nominal = crate::mesh::build(&d, &lib, params());
    assert!((p.mesh.volume_mm3() / nominal.mesh.volume_mm3() - s.scale().powi(3)).abs() < 1e-5);
    assert_eq!(serde_json::to_string(&d).unwrap(), before);
    let again = prepare(&d, &lib, &s, params()).unwrap();
    assert_eq!(p.mesh.vertices, again.mesh.vertices);
}

#[test]
fn finishing_stock_preserves_the_nominal_document_and_leaves_bore_metal() {
    let d = RingDesign::default();
    let lib = AlphaLibrary::builtin();
    let s = Setup {
        bore_stock_mm: 0.2,
        radial_stock_mm: 0.1,
        ..Default::default()
    };
    let p = prepare(&d, &lib, &s, params()).unwrap();
    assert!((p.design.inner_radius_mm() - (d.inner_radius_mm() - 0.2)).abs() < 1e-9);
    assert!((p.design.profile.thickness_mm - d.profile.thickness_mm - 0.3).abs() < 1e-9);
}

#[test]
fn bench_features_remain_in_design_but_not_in_pattern_and_undo_restores_them() {
    let mut d = RingDesign::default();
    let lib = AlphaLibrary::builtin();
    d.layers.layers.push(crate::field::LayerEntry::new(
        "Cut at bench",
        Layer::Curve(Default::default()),
    ));
    let mut history = crate::history::History::new(&d);
    let candidate = repair::candidate(
        &d,
        &Setup::default(),
        Some(0),
        repair::Repair::DeferToBench,
        0.0,
    )
    .unwrap();
    history.commit(&candidate);
    let p = prepare(&candidate, &lib, &Setup::default(), params()).unwrap();
    assert!(candidate.layers.layers[0].enabled);
    assert!(!p.design.layers.layers[0].enabled);
    assert_eq!(p.bench_layers, vec!["Cut at bench"]);
    assert!(!history.undo().unwrap().layers.layers[0].bench_only);
}

#[test]
fn ordinary_ring_releases_in_both_halves_without_exempting_the_bore() {
    let d = RingDesign::default();
    let lib = AlphaLibrary::builtin();
    let s = Setup::default();
    let i = inspect(&d, &lib, &s, params()).unwrap();
    assert!(
        i.release.obstructions.is_empty(),
        "{:?}",
        i.release.obstructions
    );
    assert_ne!(i.release.status, Status::Invalid);
    assert!(
        i.release.columns.iter().any(|c| c.intervals.is_empty()),
        "Finger-hole sand is represented"
    );
}

#[test]
fn package_report_matches_the_written_pattern_and_never_overwrites() {
    let root = std::env::temp_dir().join(format!(
        "ring-manufacture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let d = RingDesign::default();
    let lib = AlphaLibrary::builtin();
    let s = Setup::default();
    let report = package::export(&root, &d, &lib, &s, params(), false).unwrap();
    for file in [
        "pattern.stl",
        "pattern.3mf",
        "report.json",
        "recipe.json",
        "mold.svg",
        "molding-sheet.html",
        "design.ring.json",
    ] {
        assert!(root.join(file).is_file(), "missing {file}");
    }
    let disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("report.json")).unwrap()).unwrap();
    assert_eq!(disk["release"]["status"], report["release"]["status"]);
    assert!(
        (disk["geometry"]["volume_mm3"].as_f64().unwrap()
            - report["geometry"]["volume_mm3"].as_f64().unwrap())
        .abs()
            < 1e-8
    );
    let bytes = std::fs::read(root.join("pattern.stl")).unwrap();
    let portable = package::files(&d, &lib, &s, params(), false).unwrap();
    for entry in &portable.entries {
        let disk = std::fs::read(root.join(&entry.name)).unwrap();
        if entry.name != "report.json" { assert_eq!(disk, entry.data, "portable {} differs", entry.name); }
    }
    assert_eq!(portable.report["pattern_fingerprint"], report["pattern_fingerprint"]);
    assert_eq!(&portable.zip()[..4], b"PK\x03\x04");
    let triangles = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(
        triangles as u64,
        report["geometry"]["validation"]["triangle_count"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        disk["pattern_fingerprint"],
        format!("{:016x}", package::fingerprint(&bytes[80..]))
    );
    assert!(package::export(&root, &d, &lib, &s, params(), false).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recipe_validation_and_html_escaping_are_enforced() {
    let mut s = Setup::default();
    s.recipe.shrink_pct = f64::NAN;
    assert!(s.validate().is_err());
    s = Setup::default();
    s.channels.push(Channel {
        kind: ChannelKind::Gate,
        start: [0.0; 3],
        end: [0.0; 3],
        diameter_mm: 2.0,
    });
    assert!(s.validate().is_err());
    assert_eq!(package::escape("<script>&\""), "&lt;script&gt;&amp;&quot;");
}

#[test]
fn investment_sheet_does_not_instruct_sand_withdrawal() {
    let d = RingDesign::default();
    let mut setup = Setup::default();
    setup.recipe.process = crate::castability::CastProcess::LostWax;
    let i = inspect(&d, &AlphaLibrary::builtin(), &setup, params()).unwrap();
    let sheet = package::sheet(&d, &setup, &i, false);
    assert!(sheet.contains("Investment pattern preparation"));
    assert!(!sheet.contains("Lift the upper mold straight"));
}

/// The Court band with a procedural shank and `more` parts on it.
fn court_with(more: Vec<crate::cad::Feature>) -> RingDesign {
    use crate::cad::{Component, Document, Feature, Operation};
    let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    for f in more {
        doc.append(f).unwrap();
    }
    d.cad = Some(doc);
    d
}

/// A 1.6 mm post joined to the top of the band, its foot 0.1 mm into the crown.
fn post() -> crate::cad::Feature {
    use crate::cad::{Attach, Component, Feature, Operation, Placement};
    let component = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.9), ..Component::default() };
    Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.0 }, component }
}

/// A 1.5 mm cube kept beside the band at the palm.
fn spacer() -> crate::cad::Feature {
    use crate::cad::{Component, Feature, Operation, Placement};
    let component = Component { placement: Placement::ring(270.0, 2.0), ..Component::default() };
    Feature { id: 3, name: "Spacer".into(), enabled: true, operation: Operation::Box { size: [1.5; 3] }, component }
}

#[test]
fn a_band_with_parts_casts_the_ring_and_names_a_separate_part_as_its_own_casting() {
    let lib = AlphaLibrary::builtin();
    let setup = Setup::default();
    let claw = crate::cad::examples::design("claw-solitaire").unwrap();
    let bare = prepare(&court_with(Vec::new()), &lib, &setup, params()).unwrap();
    let with_post = prepare(&court_with(vec![post()]), &lib, &setup, params()).unwrap();
    let both = court_with(vec![post(), spacer()]);
    let with_spacer = prepare(&both, &lib, &setup, params()).unwrap();
    // The ring is the band with every joined and cut part; the anchor, the stone and the separate spacer are not outputs of it.
    assert_eq!(with_post.design.cad.as_ref().unwrap().outputs, vec![2]);
    assert_eq!(with_spacer.design.cad.as_ref().unwrap().outputs, vec![2]);
    assert!(with_post.notes.is_empty() && with_post.casting == Casting::Ring, "{:?}", with_post.notes);
    assert_eq!(with_spacer.notes, vec!["#3 Spacer is its own casting and not in this pattern: choose it as the casting component to prepare it".to_string()]);
    // The spacer changes nothing poured: the same band and post, vertex for vertex.
    assert!(with_spacer.mesh.vertices == with_post.mesh.vertices && with_spacer.mesh.faces == with_post.mesh.faces);
    let post_mm3 = std::f64::consts::PI * 0.64 * 2.0;
    let added = with_post.build.volume_mm3 - bare.build.volume_mm3;
    eprintln!("ring casting: bare {:.4} mm³, with the post {:.4} (+{added:.4} of {post_mm3:.4})", bare.build.volume_mm3, with_post.build.volume_mm3);
    assert!(added > 0.9 * post_mm3 && added < post_mm3, "added {added:.4} of {post_mm3:.4}");
    assert!(with_post.mesh.validate().watertight);
    // The claw solitaire pours its band with the head joined; the bur waits for the bench under sand and the stone is never metal.
    let p = prepare(&claw, &lib, &setup, params()).unwrap();
    eprintln!("claw solitaire ring casting: {:.4} mm³ poured, {} faces, bench {:?}", p.build.volume_mm3, p.mesh.faces.len(), p.bench_layers);
    assert_eq!(p.design.cad.as_ref().unwrap().outputs, vec![3]);
    assert_eq!(p.bench_layers, vec!["Seat bur (part)".to_string()]);
    assert!(p.notes.is_empty() && p.mesh.validate().watertight, "{:?}", p.notes);
    // The mould study and the casting inspection read the same pattern, parts judged on the build it was cut from.
    for (name, d, part) in [("claw solitaire", &claw, 3), ("post", &court_with(vec![post()]), 2), ("post and spacer", &both, 2)] {
        let study = crate::interaction::mould::build(d, &lib).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        let seen = prepare(d, &lib, &setup, BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() }).unwrap();
        assert!((study.pattern.volume_mm3() - seen.mesh.volume_mm3()).abs() < 1e-9, "{name}");
        assert_eq!(study.notes, seen.notes, "{name}");
        let i = inspect(d, &lib, &setup, params()).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        let field = i.field.as_ref().unwrap_or_else(|| panic!("{name}: the ring is judged by the field"));
        assert!(field.parts.iter().any(|p| p.feature == part && p.judged), "{name}: {:?}", field.parts);
        assert!(i.local_wall.is_none(), "{name}");
    }
    let i = inspect(&both, &lib, &setup, params()).unwrap();
    assert!(i.details.iter().any(|d| d.starts_with("#3 Spacer is its own casting")), "{:?}", i.details);
    let package = package::files(&both, &lib, &setup, params(), true).unwrap();
    assert_eq!(package.report["casting"], "ring");
    assert_eq!(package.report["not_in_pattern"], serde_json::json!(["#3 Spacer is its own casting and not in this pattern: choose it as the casting component to prepare it"]));
    let sheet = package.entries.iter().find(|e| e.name == "molding-sheet.html").unwrap();
    assert!(String::from_utf8_lossy(&sheet.data).contains("#3 Spacer is its own casting"));
}

#[test]
fn a_chosen_separate_part_is_cast_alone_and_a_chosen_ring_member_casts_the_ring() {
    let lib = AlphaLibrary::builtin();
    let both = court_with(vec![post(), spacer()]);
    let ring = prepare(&both, &lib, &Setup::default(), params()).unwrap();
    // The band's anchor and a joined part each stand for the ring.
    for id in [1, 2] {
        let p = prepare(&both, &lib, &Setup { component: Some(id), ..Setup::default() }, params()).unwrap();
        assert_eq!((p.casting, &p.design.cad.as_ref().unwrap().outputs), (Casting::Ring, &vec![2]), "#{id}");
        assert!(p.mesh.vertices == ring.mesh.vertices, "#{id}");
    }
    // The spacer chosen is poured alone, standing where the finished ring stands it: a 1.5 mm cube and nothing of the band.
    let setup = Setup { component: Some(3), ..Setup::default() };
    let p = prepare(&both, &lib, &setup, params()).unwrap();
    assert_eq!((p.casting, &p.design.cad.as_ref().unwrap().outputs), (Casting::Part(3), &vec![3]));
    assert!(p.notes.is_empty() && p.bench_layers.is_empty());
    let cube = 3.375 * setup.scale().powi(3);
    assert!((p.mesh.volume_mm3() - cube).abs() < 1e-3, "{} against {cube}", p.mesh.volume_mm3());
    assert!((p.build.volume_mm3 - 3.375).abs() < 1e-3 && p.mesh.validate().watertight && p.mesh.faces.len() == 12, "{:?}", p.build.validation);
    let finished = crate::mesh::try_build(&both, &lib, params()).unwrap();
    let own = crate::threemf::objects(&finished, "Court band").into_iter().find(|o| o.feature == Some(3)).unwrap();
    let low = |m: &crate::Mesh| m.bounds().unwrap().0;
    let (a, b) = (low(&own.mesh), low(&p.mesh.scaled(1.0 / p.scale)));
    assert!((a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5 && (a.2 - b.2).abs() < 1e-5, "{a:?} against {b:?}");
    // Inspected as a CAD component: a local wall, no band field, no hot spot.
    let i = inspect(&both, &lib, &setup, params()).unwrap();
    assert!(i.field.is_none() && i.local_wall.is_some() && i.hot_spot.is_none());
    assert!((i.ring_grams - p.build.volume_mm3 * crate::metal::find("Silver 925").unwrap().density / 1000.0).abs() < 1e-9);
    let study = crate::interaction::mould::build(&RingDesign { manufacturing: Some(setup.clone()), ..both.clone() }, &lib).unwrap();
    assert!((study.pattern.volume_mm3() - cube).abs() < 1e-3);
    // Its stages are the cube too: nominal and as-cast one size, the pattern the shrink over it.
    let s = stages::evaluate(&both, &lib, &setup, params()).unwrap();
    assert!((s.nominal.volume_mm3() - 3.375).abs() < 1e-3 && (s.as_cast.volume_mm3() - 3.375).abs() < 1e-3, "{}", s.nominal.volume_mm3());
    // Profile stock goes on the band, which this pattern does not pour; a stone and a stranger are refused by name.
    let stock = prepare(&both, &lib, &Setup { radial_stock_mm: 0.1, ..setup.clone() }, params()).err().unwrap().to_string();
    assert_eq!(stock, "Profile stock goes on the band, and this pattern casts #3 Spacer on its own; model stock into it explicitly");
    let claw = crate::cad::examples::design("claw-solitaire").unwrap();
    for id in [2, 99] {
        let refused = prepare(&claw, &lib, &Setup { component: Some(id), ..Setup::default() }, params()).err().unwrap().to_string();
        assert_eq!(refused, "Selected casting component is missing or a reference stone", "#{id}");
    }
    // A ring of parts only still casts the one component it is asked for.
    let gallery = crate::cad::examples::design("gallery").unwrap();
    let refused = prepare(&gallery, &lib, &Setup::default(), params()).err().unwrap().to_string();
    assert_eq!(refused, "Select one CAD component in the casting recipe");
    let one = prepare(&gallery, &lib, &Setup { component: Some(1), ..Setup::default() }, params()).unwrap();
    assert_eq!((one.casting, &one.design.cad.as_ref().unwrap().outputs), (Casting::Part(1), &vec![1]));
}

#[test]
fn recipe_persistence_and_failed_production_export_leave_no_package() {
    let root = std::env::temp_dir().join(format!(
        "ring-recipe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut recipe = Recipe::default();
    recipe.name = "Measured shop batch".into();
    recipe.shrink_pct = 2.15;
    recipe.save(root.join("recipe.json")).unwrap();
    let loaded = Recipe::load(root.join("recipe.json")).unwrap();
    assert_eq!(loaded.name, recipe.name);
    assert_eq!(loaded.shrink_pct, recipe.shrink_pct);
    let mut s = Setup::default();
    s.flask.width_mm = 5.0;
    assert!(package::files(&RingDesign::default(), &AlphaLibrary::builtin(), &s, params(), false).is_err());
    assert!(package::files(&RingDesign::default(), &AlphaLibrary::builtin(), &s, params(), true).is_ok());
    assert!(
        package::export(
            &root.join("production"),
            &RingDesign::default(),
            &AlphaLibrary::builtin(),
            &s,
            params(),
            false
        )
        .is_err()
    );
    assert!(!root.join("production").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
