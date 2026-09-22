//! Refine authored templates and verify that their effective geometry inputs match.
//! cargo run -p ringdesign-graph --example refine_templates -- --write [--meshes]
use anyhow::{ensure, Result};
use ringdesign_core::{AlphaLibrary, RingDesign, manufacturing, mesh};
use ringdesign_graph::{eval::{evaluate_design, Evaluator}, file, lift, registry::Registry, templates};

fn geometry_data(design: &RingDesign) -> serde_json::Value {
    let mut value = serde_json::to_value(design).unwrap();
    for key in ["embedded", "recipes", "texts", "drawn", "svgs", "graph"] { value.as_object_mut().unwrap().remove(key); }
    value
}

fn main() -> Result<()> {
    let write = std::env::args().any(|a| a == "--write");
    let meshes = std::env::args().any(|a| a == "--meshes");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lib = AlphaLibrary::builtin();
    let reg = Registry::builtin();
    let mut audit = Vec::new();
    for template in templates::catalog() {
        let old = template.load();
        let before = evaluate_design(&mut Evaluator::new(), &old, &reg, &lib, 0)?;
        ensure!(before.notes.is_empty(), "{}: {:?}", template.slug, before.notes);
        let refined = templates::refine_sources(&before.design);
        // Small procedural starters are already authored by hand, with no lifted dead heads.
        let starter = templates::BUNDLED.iter().any(|t| t.slug == template.slug);
        let mut graph = if starter { old.clone() } else { lift::from_design(&refined, &reg, &lib)? };
        graph.mode = old.mode;
        let after = evaluate_design(&mut Evaluator::new(), &graph, &reg, &lib, 0)?;
        ensure!(after.notes.is_empty(), "{}: {:?}", template.slug, after.notes);
        ensure!(geometry_data(&before.design) == geometry_data(&after.design), "{}: geometry settings changed", template.slug);
        let old_lib = manufacturing::source_library(&before.design, &lib);
        let new_lib = manufacturing::source_library(&after.design, &lib);
        for name in before.design.layers.referenced_alphas() {
            let a = old_lib.get(name).ok_or_else(|| anyhow::anyhow!("missing source {name}"))?;
            let b = new_lib.get(name).ok_or_else(|| anyhow::anyhow!("missing refined source {name}"))?;
            ensure!(a.width == b.width && a.height == b.height && a.data == b.data, "{}: changed artwork {name}", template.slug);
        }
        let sinks: Vec<_> = graph.nodes.iter().filter(|n| n.kind == "sink.output").map(|n| n.id).collect();
        let reachable: std::collections::HashSet<_> = sinks.iter().flat_map(|id| graph.upstream(*id).into_iter().chain([*id])).collect();
        ensure!(graph.nodes.iter().all(|n| reachable.contains(&n.id)), "{}: disconnected nodes", template.slug);
        let mut mesh_verified = false;
        if meshes && template.slug.ends_with("-reptilia") {
            let a = mesh::try_build(&before.design, &old_lib, before.design.build)?;
            let b = mesh::try_build(&after.design, &new_lib, after.design.build)?;
            ensure!(a.mesh.vertices == b.mesh.vertices && a.mesh.faces == b.mesh.faces && a.mesh.normals == b.mesh.normals, "{}: changed mesh", template.slug);
            mesh_verified = true;
        }
        let source_count = |d: &RingDesign| d.embedded.len()+d.recipes.len()+d.texts.len()+d.svgs.len()+d.drawn.len();
        println!("{}: {} → {} nodes, {} → {} artwork sources; identical effective inputs{}", template.slug, old.nodes.len(), graph.nodes.len(), source_count(&before.design), source_count(&after.design), if mesh_verified { " and mesh" } else { "" });
        audit.push(serde_json::json!({"slug":template.slug,"nodes_before":old.nodes.len(),"nodes_after":graph.nodes.len(),"sources_before":source_count(&before.design),"sources_after":source_count(&after.design),"geometry_settings_identical":true,"referenced_pixels_identical":true,"all_nodes_reach_output":true,"mesh_identical":mesh_verified.then_some(true)}));
        if write && !starter {
            file::save_graph(root.join(format!("graphs/templates/{}.graph.json", template.slug)), &graph)?;
            let collection = if template.slug.ends_with("-reptilia") { Some(("reptilia", "-reptilia")) } else if template.slug.ends_with("-imported") { Some(("stock-masterworks", "-imported")) } else { None };
            if let Some((directory, suffix)) = collection {
                let mut design = (*after.design).clone();
                design.graph = Some(serde_json::to_value(&graph)?);
                ringdesign_core::library::save_design(root.join(format!("showcase/{directory}/{}/editable-graph.ring.json", template.slug.trim_end_matches(suffix))), &design)?;
            }
        }
    }
    let out = root.join("target/template-library-review");
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("template-audit.json"), serde_json::to_vec_pretty(&audit)?)?;
    Ok(())
}
