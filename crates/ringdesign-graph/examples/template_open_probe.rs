//! Milliseconds per phase of opening every catalogued template graph, and of the builds after it lands.
//! cargo run --release -p ringdesign-graph --example template_open_probe [-- [--sources] slug ...]
//! `--sources` also times each artwork source and distance field alone.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use ringdesign_core::{AlphaLibrary, BuildParams, mesh};
use ringdesign_graph::{eval, file, registry::Registry, templates};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Milliseconds each of `design`'s artwork sources takes to rasterize alone, slowest first.
fn sources(design: &ringdesign_core::RingDesign, lib: &AlphaLibrary) {
    let mut rows: Vec<(f64, String)> = Vec::new();
    let mut time = |what: String, f: &dyn Fn() -> usize| {
        let t = Instant::now();
        let texels = f();
        rows.push((ms(t), format!("{what}: {texels} texels")));
    };
    for d in &design.drawn {
        time(format!("drawn {}", d.name), &|| d.rasterize().data.len());
    }
    for s in &design.texts {
        time(format!("text {}", s.name), &|| s.rasterize().data.len());
    }
    for s in &design.svgs {
        time(format!("svg {}", s.name), &|| s.rasterize().data.len());
    }
    for r in &design.recipes {
        time(format!("recipe {}", r.name), &|| r.rasterize(256).data.len());
    }
    let mut baked = lib.clone();
    design.unpack_embedded(&mut baked);
    design.bake_all(&mut baked);
    for a in baked.changed_since(lib).filter(|a| a.name.ends_with(ringdesign_core::alpha::SDF_SUFFIX)) {
        let source = baked.get(a.name.trim_end_matches(ringdesign_core::alpha::SDF_SUFFIX)).expect("its source").clone();
        time(format!("field {}", a.name), &|| source.signed_distance_px().data.len());
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (t, what) in rows {
        println!("  {t:7.1} ms  {what}");
    }
}

fn main() -> anyhow::Result<()> {
    let detail = std::env::args().any(|a| a == "--sources");
    let only: Vec<String> = std::env::args().skip(1).filter(|a| a != "--sources").collect();
    let reg = Registry::builtin();
    let t = Instant::now();
    let lib = Arc::new(AlphaLibrary::installed());
    println!("installed library: {} alphas in {:.0} ms", lib.len(), ms(t));
    let params = BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() };
    println!("| template | MB | unpack | read | evaluate | bake | bake again | first build | first verdict | rebuild | artwork Mtexels |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let never = Arc::new(AtomicBool::new(false));
    for template in templates::catalog().filter(|t| only.is_empty() || only.iter().any(|s| s == t.slug)) {
        let t = Instant::now();
        let json = template.json();
        let unpack = ms(t);
        let t = Instant::now();
        let graph = file::load_graph_str(&json, None)?;
        let read = ms(t);
        let t = Instant::now();
        let mut ev = eval::Evaluator::new();
        let (design, report) = eval::design_of(&mut ev, &graph, &reg, &lib, lib.revision())?;
        let evaluate = ms(t);
        let t = Instant::now();
        let mut baked = (*lib).clone();
        design.unpack_and_bake_observed(&mut baked, &|_, _| {}, &never).expect("not cancelled");
        let bake = ms(t);
        let texels: usize = baked.changed_since(&lib).map(|a| a.data.len()).sum();
        let t = Instant::now();
        let mut again = (*lib).clone();
        design.unpack_and_bake_observed(&mut again, &|_, _| {}, &never).expect("not cancelled");
        let bake_again = ms(t);
        let baked = Arc::new(baked);
        let t = Instant::now();
        mesh::try_build(&design, &baked, params)?;
        let build = ms(t);
        // The build worker takes the open's evaluation for its first build, then evaluates against the same base.
        let t = Instant::now();
        let first = eval::judge(&mut ev, design.clone(), report, &graph, &baked);
        let verdict = ms(t);
        let host = first.baked_library.clone().unwrap_or_else(|| baked.clone());
        let t = Instant::now();
        let again = eval::evaluate_design_onto(&mut ev, &graph, &reg, &lib, &host)?;
        let rebuild = ms(t);
        assert!(again.baked_library.is_none() && Arc::ptr_eq(&again.design, &design), "{}: a rebuild of an unchanged graph reuses everything", template.slug);
        println!(
            "| {} | {:.1} | {unpack:.0} | {read:.0} | {evaluate:.0} | {bake:.0} | {bake_again:.1} | {build:.0} | {verdict:.0} | {rebuild:.1} | {:.2} |",
            template.slug,
            json.len() as f64 / 1e6,
            texels as f64 / 1e6,
        );
        if detail {
            sources(&design, &lib);
        }
    }
    println!("shared bakes hold {:.1} Mtexels", ringdesign_core::alpha::bake_cache_texels() as f64 / 1e6);
    Ok(())
}
