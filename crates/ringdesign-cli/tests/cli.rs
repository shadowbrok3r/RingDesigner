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
