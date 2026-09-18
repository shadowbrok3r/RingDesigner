//! Regenerate the bundled showcase graphs, optionally rendering a review.
//! cargo run -p ringdesign-graph --example showcase_templates -- --write [--render DIR]
use anyhow::{Result, ensure};
use ringdesign_core::{AlphaLibrary, BuildParams, build, library, render};
use ringdesign_graph::{eval::{Evaluator, evaluate_design}, file, lift, registry::Registry, templates};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(args.first().map(String::as_str) == Some("--write"), "use --write [--render DIR]");
    let render_dir = match &args[1..] {
        [] => None,
        [flag, path] if flag == "--render" => Some(PathBuf::from(path)),
        _ => anyhow::bail!("use --write [--render DIR]"),
    };
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reg = Registry::builtin();
    for t in templates::SHOWCASE {
        let source = match t.slug {
            "nocturne" | "solstice" => format!("showcase/masterwork-signets/{}/design.ring.json", t.slug),
            slug => format!("showcase/{slug}/design.ring.json"),
        };
        let d = library::load_design(repo.join(source))?;
        let mut lib = AlphaLibrary::builtin();
        d.unpack_embedded(&mut lib);
        d.bake_all(&mut lib);
        let g = lift::from_design(&d, &reg, &lib)?;
        let g = file::load_graph_str(&file::graph_to_string(&g)?, Some(&reg))?;
        let cold = AlphaLibrary::builtin();
        let result = evaluate_design(&mut Evaluator::new(), &g, &reg, &cold, 0)?;
        ensure!(serde_json::to_value(&*result.design)? == serde_json::to_value(&d)?, "{} does not reproduce its source", t.name);
        ensure!(result.notes.is_empty(), "{}: {:?}", t.name, result.notes);
        file::save_graph(repo.join(format!("graphs/templates/{}.graph.json", t.slug)), &g)?;
        println!("{}: {} nodes, {} controls, exact source match", t.name, g.nodes.len(), g.exposed.len());
        if let Some(dir) = &render_dir {
            std::fs::create_dir_all(dir)?;
            let mesh = build(&result.design, &lib, BuildParams { theta_steps: 768, profile_steps: 256, refine: None, ..Default::default() }).mesh;
            render::write_png(dir.join(format!("{}.png", t.slug)), &mesh, 0.48, 1.0, 1000, render::GOLD)?;
            library::save_design(dir.join(format!("{}.ring.json", t.slug)), &ringdesign_core::RingDesign { graph: Some(serde_json::to_value(&g)?), ..(*result.design).clone() })?;
        }
    }
    Ok(())
}
