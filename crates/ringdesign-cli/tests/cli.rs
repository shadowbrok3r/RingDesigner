//! The binary end to end: a template graph evaluated to a design file,
//! then checked as a design.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ringdesign"))
}

#[test]
fn graph_eval_writes_a_design_that_check_reads_with_the_same_verdict() {
    let dir = std::env::temp_dir().join(format!("ringdesign-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let graph = dir.join("court.graph.json");
    std::fs::write(&graph, ringdesign_graph::templates::BUNDLED.iter().find(|t| t.name == "Court band").unwrap().json().as_bytes()).unwrap();
    let out = dir.join("court.ring.json");

    let o = bin().args(["graph", "describe", graph.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("band.profile") && text.contains("exposed Width"), "{text}");

    let o = bin().args(["graph", "eval", graph.to_str().unwrap(), "--set", "Width=6", "--out", out.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.starts_with("Court band: Castable"), "{text}");
    let design = ringdesign_core::library::load_design(&out).unwrap();
    assert_eq!(design.profile.width_mm, 6.0, "--set reached the exposed pin");
    assert!(design.graph.is_some(), "the design carries its graph");

    let o = bin().args(["check", out.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("Castable"), "{text}");

    let o = bin().args(["graph", "eval", graph.to_str().unwrap(), "--set", "Nope=1"]).output().unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("not an exposed parameter"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn casting_commands_export_the_checked_pattern_and_preserve_the_source() {
    let root=std::env::temp_dir().join(format!("ring-casting-cli-{}",std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path=root.join("source.ring.json");
    ringdesign_core::library::save_design(&path,&ringdesign_core::RingDesign::default()).unwrap();
    let before=std::fs::read(&path).unwrap();
    let o=bin().args(["casting","check",path.to_str().unwrap(),"--json"]).output().unwrap();
    assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));
    let report:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(report["release"]["obstructions"].as_array().unwrap().len(),0);
    let package=root.join("package");
    let o=bin().args(["casting","export",path.to_str().unwrap(),"--out",package.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));
    let written:serde_json::Value=serde_json::from_slice(&std::fs::read(package.join("report.json")).unwrap()).unwrap();
    assert_eq!(written["pattern_fingerprint"],report["pattern_fingerprint"]);
    assert_eq!(before,std::fs::read(&path).unwrap());
    assert!(package.join("molding-sheet.html").is_file());
    std::fs::remove_dir_all(root).unwrap();
}

/// A 6 mm court band with a 2 mm post joined 1.5 mm off its mid-plane, staged as asked.
fn post_band(stage: ringdesign_core::cad::Stage) -> ringdesign_core::RingDesign {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement};
    let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    d.profile.width_mm = 6.0;
    let mut doc = Document::default();
    doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    let placement = Placement::Ring { theta_deg: 90.0, across_mm: 1.5, height_mm: 0.6, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
    doc.append(Feature { id: 3, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Component { attach: Attach::Join, stage, placement, ..Default::default() } }).unwrap();
    d.cad = Some(doc);
    d
}

#[test]
fn check_and_the_size_run_judge_the_parts_a_build_carries() {
    use ringdesign_core::cad::Stage;
    let dir = std::env::temp_dir().join(format!("ring-parts-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (stage, verdict, label) in [(Stage::Cast, "Marginal", "Castable with care"), (Stage::Bench, "Castable", "Castable")] {
        let path = dir.join(format!("{stage:?}.ring.json"));
        ringdesign_core::library::save_design(&path, &post_band(stage)).unwrap();
        let o = bin().args(["check", path.to_str().unwrap()]).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        assert!(text.lines().next().unwrap().ends_with(&format!("—  {label}")), "{text}");
        let out = dir.join(format!("{stage:?}"));
        let o = bin().args(["export", path.to_str().unwrap(), "--sizes", "7", "--formats", "stl", "--out", out.to_str().unwrap()]).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let manifest = std::fs::read_to_string(out.join("untitled_manifest.csv")).unwrap_or_else(|_| {
            let name = std::fs::read_dir(&out).unwrap().filter_map(Result::ok).map(|e| e.path()).find(|p| p.to_string_lossy().ends_with("_manifest.csv")).unwrap();
            std::fs::read_to_string(name).unwrap()
        });
        let row = manifest.lines().nth(1).unwrap();
        assert_eq!(row.split(',').nth(6), Some(verdict), "{manifest}");
        match stage {
            Stage::Cast => {
                assert!(text.contains("part Cylinder \"Post\" (#3) — Join, Cast: 5.2"), "{text}");
                assert!(text.contains("on its drag-facing flank above the parting plane") && text.contains("stage it Bench"), "{text}");
            }
            Stage::Bench => {
                assert!(text.contains("part Cylinder \"Post\" (#3) — Join, Bench: not judged"), "{text}");
                assert!(text.contains("locating mark on the parting line at 90°; the part's foot is 1.5 mm toward the high edge"), "{text}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The claw solitaire, renamed, with a 1.5 mm spacer kept beside the band at the palm.
fn solitaire_and_spacer() -> ringdesign_core::RingDesign {
    use ringdesign_core::cad::{Component, Feature, Operation, Placement};
    let mut d = ringdesign_core::cad::examples::design("claw-solitaire").unwrap();
    d.name = "Solitaire".into();
    let spacer = Component { placement: Placement::ring(270.0, 2.0), ..Component::default() };
    d.cad.as_mut().unwrap().append(Feature { id: 5, name: "Spacer".into(), enabled: true, operation: Operation::Box { size: [1.5, 1.5, 1.5] }, component: spacer }).unwrap();
    d
}

/// The names of the objects a store-only 3MF package's model holds.
fn objects_in(package: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(package);
    let model = &text[text.find("<model").unwrap()..text.find("</model>").unwrap()];
    model.split("<object ").skip(1).map(|o| { let at = o.find("name=\"").unwrap() + 6; o[at..at + o[at..].find('"').unwrap()].to_string() }).collect()
}

#[test]
fn a_size_run_with_parts_reseats_them_per_size_and_packs_each_separate_part_as_its_own_object() {
    let dir = std::env::temp_dir().join(format!("ring-size-run-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("solitaire.ring.json");
    ringdesign_core::library::save_design(&path, &solitaire_and_spacer()).unwrap();
    let out = dir.join("run");
    let o = bin().args(["export", path.to_str().unwrap(), "--sizes", "5:9:1", "--formats", "stl,3mf", "--steps", "192x96", "--out", out.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let manifest = std::fs::read_to_string(out.join("solitaire_manifest.csv")).unwrap();
    let mut rows = manifest.lines();
    assert!(rows.next().unwrap().ends_with(",volume_mm3,dfm_findings,stone_warnings,parts,part_notes"), "{manifest}");
    let rows: Vec<Vec<&str>> = rows.map(|r| r.split(',').collect()).collect();
    assert_eq!(rows.len(), 10, "{manifest}");
    // Every size watertight with its head joined and its spacer beside it, nothing left out, one verdict.
    for r in &rows {
        assert_eq!((r[5], r[12], r[13]), ("true", "2", "0"), "{manifest}");
        assert_eq!(r[6], rows[0][6], "{manifest}");
    }
    // The band grows with the size and the parts come along.
    let volume = |size: &str| rows.iter().find(|r| r[0] == format!("US {size}") && r[2] == "3mf").unwrap()[9].parse::<f64>().unwrap();
    assert!(volume("5") < volume("7") && volume("7") < volume("9"), "{manifest}");
    for size in ["5", "7", "9"] {
        let package = std::fs::read(out.join(format!("solitaire_size{size}.3mf"))).unwrap();
        assert_eq!(objects_in(&package), ["Solitaire", "Spacer"], "size {size}");
    }
    // Check runs each size and says what its parts did.
    let o = bin().args(["check", path.to_str().unwrap(), "--sizes", "5,9"]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(text.contains("Solitaire  size US 5  —") && text.contains("Solitaire  size US 9  —"), "{text}");
    assert_eq!(text.matches("3 parts resolved: 1 joined, 1 cut, 1 separate; 1 stones set as parts").count(), 2, "{text}");
    // STEP with the band: the spacer analytic, the band with its head and seat faceted, the stone left out.
    let step = dir.join("solitaire.step");
    let o = bin().args(["cad", "step", path.to_str().unwrap(), "--out", step.to_str().unwrap(), "--band"]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(text.starts_with("STEP solids: 1 analytic, 1 faceted"), "{text}");
    let solids = ringdesign_core::cad::step::read_solids(&std::fs::read_to_string(&step).unwrap()).unwrap();
    assert_eq!(solids.iter().map(|s| (s.name.as_str(), s.faceted)).collect::<Vec<_>>(), [("Spacer", false), ("Solitaire", true)]);
    assert!(solids[1].mesh.as_ref().unwrap().validate().watertight);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn casting_check_casts_the_ring_of_a_band_with_parts_and_a_separate_part_alone() {
    use ringdesign_core::cad::{Component, Feature, Operation, Placement, Stage};
    let dir = std::env::temp_dir().join(format!("ring-casting-parts-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut d = post_band(Stage::Cast);
    // Centred on the parting plane, the post pulls both ways.
    d.cad.as_mut().unwrap().features[1].component.placement = Placement::ring(90.0, 0.6);
    let spacer = Component { placement: Placement::ring(270.0, 2.0), ..Component::default() };
    d.cad.as_mut().unwrap().append(Feature { id: 4, name: "Spacer".into(), enabled: true, operation: Operation::Box { size: [1.5; 3] }, component: spacer }).unwrap();
    let path = dir.join("parts.ring.json");
    ringdesign_core::library::save_design(&path, &d).unwrap();
    let check = |extra: &[&str]| -> serde_json::Value {
        let o = bin().args(["casting", "check", path.to_str().unwrap(), "--json"]).args(extra).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        serde_json::from_slice(&o.stdout).unwrap()
    };
    // The ring by default and for the post; the spacer named as its own casting.
    for extra in [&[][..], &["--component", "3"][..]] {
        let r = check(extra);
        assert_eq!(r["casting"], "ring", "{extra:?}");
        assert_eq!(r["not_in_pattern"], serde_json::json!(["#4 Spacer is its own casting and not in this pattern: choose it as the casting component to prepare it"]), "{extra:?}");
    }
    // The spacer chosen is poured alone: a 1.5 mm cube scaled for the shrink.
    let r = check(&["--component", "4"]);
    assert_eq!(r["casting"], serde_json::json!({ "part": 4 }));
    let volume = r["source_build"]["volume_mm3"].as_f64().unwrap();
    assert!((volume - 3.375).abs() < 1e-3, "{volume}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_size_run_writes_each_part_as_an_obj_object_and_the_ring_as_step() {
    let dir = std::env::temp_dir().join(format!("ring-obj-step-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("solitaire.ring.json");
    ringdesign_core::library::save_design(&path, &solitaire_and_spacer()).unwrap();
    let out = dir.join("run");
    let o = bin().args(["export", path.to_str().unwrap(), "--sizes", "6,8", "--formats", "obj,step", "--steps", "192x96", "--out", out.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let manifest = std::fs::read_to_string(out.join("solitaire_manifest.csv")).unwrap();
    assert_eq!(manifest.lines().skip(1).map(|r| r.split(',').nth(2).unwrap()).collect::<Vec<_>>(), ["obj", "step", "obj", "step"], "{manifest}");
    for size in ["6", "8"] {
        // The band with its head joined and its seat cut, then the spacer; the stone is never metal.
        let obj = std::fs::read_to_string(out.join(format!("solitaire_size{size}.obj"))).unwrap();
        assert_eq!(obj.lines().filter(|l| l.starts_with("o ")).collect::<Vec<_>>(), ["o Solitaire", "o Spacer"], "size {size}");
        // The spacer exact, the band faceted and closed.
        let step = std::fs::read_to_string(out.join(format!("solitaire_size{size}.step"))).unwrap();
        let solids = ringdesign_core::cad::step::read_solids(&step).unwrap();
        assert_eq!(solids.iter().map(|s| (s.name.as_str(), s.faceted)).collect::<Vec<_>>(), [("Spacer", false), ("Solitaire", true)], "size {size}");
        assert!(solids[1].mesh.as_ref().unwrap().validate().watertight, "size {size}");
    }
    // The bigger size's band carries more metal.
    let band = |size: &str| ringdesign_core::cad::step::read_solids(&std::fs::read_to_string(out.join(format!("solitaire_size{size}.step"))).unwrap()).unwrap()[1].mesh.as_ref().unwrap().volume_mm3();
    assert!(band("8") > band("6"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn editable_cad_roundtrip_exports_components_and_resized_manufacturing_reports() {
    let dir=std::env::temp_dir().join(format!("ring-cad-cli-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&dir).unwrap();let design=dir.join("signet.ring.json");
    let run=|args:&[&str]|{let o=bin().args(args).output().unwrap();assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));o};
    run(&["cad","example","two-part-signet","--out",design.to_str().unwrap()]);
    let o=run(&["cad","check",design.to_str().unwrap()]);let report:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();assert_eq!(report["components"].as_array().unwrap().len(),2);
    let package=dir.join("assembly");run(&["cad","export",design.to_str().unwrap(),"--out",package.to_str().unwrap()]);
    let step=std::fs::read_to_string(package.join("assembly-nominal.step")).unwrap();assert!(step.contains("TOROIDAL_SURFACE"));assert!(package.join("component-2-nominal.stl").is_file());
    let variants=dir.join("sizes");run(&["cad","resize",design.to_str().unwrap(),"--bores","18.123,19.321","--out",variants.to_str().unwrap()]);
    let rows:serde_json::Value=serde_json::from_slice(&std::fs::read(variants.join("variants.json")).unwrap()).unwrap();assert_eq!(rows.as_array().unwrap().len(),2);assert_eq!(rows[0]["manufacturing"].as_array().unwrap().len(),2);assert!(rows[0]["manufacturing"][0]["pattern_fingerprint"].is_string());
    let resized=ringdesign_core::library::load_design(variants.join("variant-01-bore-18.123.ring.json")).unwrap();assert!(resized.graph.is_some());assert!((resized.size.inner_diameter_mm()-18.123).abs()<1e-9);
    assert!(!bin().args(["cad","export",design.to_str().unwrap(),"--out",package.to_str().unwrap()]).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}
