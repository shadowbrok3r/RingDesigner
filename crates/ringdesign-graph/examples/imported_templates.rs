//! Bundle native stock compositions after running the stock_masterworks author.
//! cargo run -p ringdesign-graph --release --example imported_templates -- SOURCE_DIR
use anyhow::{Result, ensure};
use ringdesign_core::{AlphaLibrary, library, manufacturing, mesh::try_build};
use ringdesign_graph::{
    eval::{Evaluator, evaluate_design},
    file,
    graph::Mode,
    lift,
    registry::Registry,
};

fn main() -> Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("showcase/stock-masterworks"));
    let reg = Registry::builtin();
    for slug in ["nocturne", "solstice", "aurelia", "vesper", "saurian", "zenith", "caiman"] {
        let d = library::load_design(source.join(slug).join("design.ring.json"))?;
        let d = ringdesign_graph::templates::refine_sources(&d);
        ensure!(d.imported_base.is_some(), "{slug} must originate on stock");
        let lib = manufacturing::source_library(&d, &AlphaLibrary::default()).into_owned();
        let mut graph = lift::from_design(&d, &reg, &lib)?;
        if matches!(slug, "nocturne" | "vesper") {
            graph.mode = Mode::Free;
        }
        let graph = file::load_graph_str(&file::graph_to_string(&graph)?, Some(&reg))?;
        let out = evaluate_design(
            &mut Evaluator::new(),
            &graph,
            &reg,
            &AlphaLibrary::default(),
            0,
        )?;
        ensure!(out.notes.is_empty(), "{slug}: {:?}", out.notes);
        ensure!(
            serde_json::to_value(&*out.design)? == serde_json::to_value(&d)?,
            "{slug} graph lost source data"
        );
        let cold =
            manufacturing::source_library(&out.design, &AlphaLibrary::default()).into_owned();
        let before = try_build(&d, &lib, d.build)?;
        let after = try_build(&out.design, &cold, d.build)?;
        ensure!(
            before.mesh.vertices == after.mesh.vertices
                && before.mesh.faces == after.mesh.faces
                && before.mesh.normals == after.mesh.normals,
            "{slug} graph changed the rendered mesh"
        );
        file::save_graph(
            root.join(format!("graphs/templates/{slug}-imported.graph.json")),
            &graph,
        )?;
        let project = ringdesign_core::RingDesign {
            graph: Some(serde_json::to_value(&graph)?),
            ..d
        };
        library::save_design(source.join(slug).join("editable-graph.ring.json"), &project)?;
        println!(
            "{slug}: {} nodes, {} exposed stock dimensions, {} exact triangles",
            graph.nodes.len(),
            graph.exposed.len(),
            after.mesh.faces.len()
        );
    }
    Ok(())
}
