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
    std::fs::write(&graph, ringdesign_graph::templates::BUNDLED.iter().find(|t| t.name == "Court band").unwrap().json).unwrap();
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
