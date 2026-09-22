//! Bundle the Reptilia collection, verifying an empty-library round trip.
//! cargo run -p ringdesign-graph --release --example reptilia_templates -- SOURCE_DIR
use anyhow::{Result, ensure};
use ringdesign_core::{AlphaLibrary, RingDesign, library, manufacturing, mesh};
use ringdesign_graph::{eval::{Evaluator, evaluate_design}, file, lift, registry::Registry};

fn main() -> Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::env::args().nth(1).map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("showcase/reptilia"));
    let reg = Registry::builtin();
    for slug in ["ecdysis", "tessera", "lorica", "ophidian", "varanus"] {
        let d = library::load_design(source.join(slug).join("design.ring.json"))?;
        let d = ringdesign_graph::templates::refine_sources(&d);
        let lib = manufacturing::source_library(&d, &AlphaLibrary::default()).into_owned();
        let graph = lift::from_design(&d, &reg, &lib)?;
        let graph = file::load_graph_str(&file::graph_to_string(&graph)?, Some(&reg))?;
        let result = evaluate_design(&mut Evaluator::new(), &graph, &reg, &AlphaLibrary::default(), 0)?;
        ensure!(result.notes.is_empty(), "{slug}: {:?}", result.notes);
        ensure!(serde_json::to_value(&*result.design)? == serde_json::to_value(&d)?, "{slug}: graph changed source");
        let cold = manufacturing::source_library(&result.design, &AlphaLibrary::default()).into_owned();
        let before = mesh::try_build(&d, &lib, d.build)?;
        let after = mesh::try_build(&result.design, &cold, d.build)?;
        ensure!(before.mesh.vertices == after.mesh.vertices && before.mesh.faces == after.mesh.faces && before.mesh.normals == after.mesh.normals, "{slug}: graph changed geometry");
        file::save_graph(root.join(format!("graphs/templates/{slug}-reptilia.graph.json")), &graph)?;
        library::save_design(source.join(slug).join("editable-graph.ring.json"), &RingDesign {
            graph: Some(serde_json::to_value(&graph)?), ..d
        })?;
        std::fs::write(source.join(slug).join("verification.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "cold_design_reload": true, "cold_graph_reload": true,
            "source_identical": true, "vertices_faces_normals_identical": true,
            "triangles": after.mesh.faces.len(), "nodes": graph.nodes.len(),
            "exposed_controls": graph.exposed.iter().map(|p| &p.name).collect::<Vec<_>>()
        }))?)?;
        println!("{slug}: {} nodes; {} controls; {} identical triangles", graph.nodes.len(), graph.exposed.len(), after.mesh.faces.len());
    }
    Ok(())
}
