use ringdesign_core::{AlphaLibrary, library};
use ringdesign_graph::{
    eval::{Evaluator, evaluate_design},
    file, lift,
    registry::Registry,
    templates,
};

#[test]
fn bestiarium_graphs_evaluate_cold_to_their_showcase_designs() {
    let reg = Registry::builtin();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("collection.json")).unwrap()).unwrap();
    let shipped: Vec<&str> = manifest["rings"].as_array().unwrap().iter().map(|r| r["slug"].as_str().unwrap()).collect();
    let menu: Vec<&str> = templates::BESTIARIUM.iter().map(|t| t.slug.strip_suffix("-bestiarium").unwrap()).collect();
    assert_eq!(menu, shipped, "the menu and the collection manifest list the same rings");
    for t in templates::BESTIARIUM {
        let slug = t.slug.strip_suffix("-bestiarium").unwrap();
        let source = templates::refine_sources(&library::load_design(root.join(slug).join("design.ring.json")).unwrap());
        assert_eq!(t.name, source.name, "{slug}: the menu names the design");
        let text = t.json();
        assert!(text.len() <= 3_000_000, "{slug}: {} bytes exceed the painted template budget", text.len());
        let graph = file::load_graph_str(&text, Some(&reg)).unwrap();
        assert!(graph.validate(Some(&reg)).is_empty(), "{slug}: {:?}", graph.validate(Some(&reg)));
        assert!(graph.nodes.iter().filter(|n| n.kind == "design.set").count() <= 4, "{slug} carries too many opaque patches");
        let out = evaluate_design(&mut Evaluator::new(), &graph, &reg, &AlphaLibrary::default(), 0).unwrap();
        assert!(out.notes.is_empty(), "{slug}: {:?}", out.notes);
        let mut differences = Vec::new();
        lift::diff(&serde_json::to_value(&*out.design).unwrap(), &serde_json::to_value(&source).unwrap(), "", &mut differences);
        assert!(differences.is_empty(), "{slug} differs from its showcase design at {:?}", differences.iter().map(|(p, _)| p).collect::<Vec<_>>());
    }
}
