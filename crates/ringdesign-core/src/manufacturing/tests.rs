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
