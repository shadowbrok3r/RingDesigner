//! Milliseconds per phase of opening every template the menu offers, the way the menu opens it, and of the builds after it lands.
//! cargo run --release -p ringdesign-graph --example template_open_probe [-- [--sources] slug ...]
//! `--sources` also times each artwork source and distance field alone.
//! A graph's starter namesake is timed as a starter too, and each bundled design as a design; a dash is a phase that open has not got.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use ringdesign_core::{AlphaLibrary, BuildParams, RingDesign, castability, mesh};
use ringdesign_graph::{eval, file, registry::Registry, templates};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Milliseconds each of `design`'s artwork sources takes to rasterize alone, slowest first.
fn sources(design: &RingDesign, lib: &AlphaLibrary) {
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

/// One table row: milliseconds per phase, `None` for a phase the open has not got.
#[derive(Default)]
struct Row {
    mb: f64,
    unpack: Option<f64>,
    read: f64,
    evaluate: Option<f64>,
    bake: f64,
    bake_again: f64,
    detail: f64,
    detail_again: f64,
    build: f64,
    verdict: f64,
    rebuild: Option<f64>,
    mtexels: f64,
}

fn cell(v: Option<f64>, places: usize) -> String {
    v.map_or_else(|| "—".into(), |v| format!("{v:.places$}"))
}

impl Row {
    fn print(&self, name: &str) {
        println!(
            "| {name} | {:.1} | {} | {:.0} | {} | {:.0} | {:.1} | {:.0} | {:.1} | {:.0} | {:.0} | {} | {:.2} |",
            self.mb,
            cell(self.unpack, 0),
            self.read,
            cell(self.evaluate, 0),
            self.bake,
            self.bake_again,
            self.detail,
            self.detail_again,
            self.build,
            self.verdict,
            cell(self.rebuild, 1),
            self.mtexels,
        );
    }
}

/// Bakes, measures, builds and judges a design read without a graph, as the build worker does for one.
fn opened_design(row: &mut Row, design: &RingDesign, lib: &Arc<AlphaLibrary>, params: BuildParams, never: &Arc<AtomicBool>) -> anyhow::Result<()> {
    let t = Instant::now();
    let mut baked = (**lib).clone();
    design.unpack_and_bake_observed(&mut baked, &|_, _| {}, never).expect("not cancelled");
    row.bake = ms(t);
    row.mtexels = baked.changed_since(lib).map(|a| a.data.len()).sum::<usize>() as f64 / 1e6;
    let t = Instant::now();
    let mut again = (**lib).clone();
    design.unpack_and_bake_observed(&mut again, &|_, _| {}, never).expect("not cancelled");
    row.bake_again = ms(t);
    let t = Instant::now();
    ringdesign_core::dfm::findings_in(design, &baked);
    row.detail = ms(t);
    let t = Instant::now();
    ringdesign_core::dfm::findings_in(design, &baked);
    row.detail_again = ms(t);
    let t = Instant::now();
    mesh::try_build(design, &baked, params)?;
    row.build = ms(t);
    let t = Instant::now();
    castability::attributed_field_report(design, &baked, &design.draft, 192, 128);
    row.verdict = ms(t);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let per_source = std::env::args().any(|a| a == "--sources");
    let only: Vec<String> = std::env::args().skip(1).filter(|a| a != "--sources").collect();
    let wanted = |slug: &str| only.is_empty() || only.iter().any(|s| s == slug);
    let reg = Registry::builtin();
    let t = Instant::now();
    let lib = Arc::new(AlphaLibrary::installed());
    println!("installed library: {} alphas in {:.0} ms", lib.len(), ms(t));
    let params = BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() };
    println!("| template | MB | unpack | read | evaluate | bake | bake again | detail | detail again | first build | first verdict | rebuild | artwork Mtexels |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let never = Arc::new(AtomicBool::new(false));
    for template in templates::catalog().filter(|t| wanted(t.slug)) {
        let mut row = Row::default();
        let t = Instant::now();
        let json = template.json();
        row.unpack = Some(ms(t));
        row.mb = json.len() as f64 / 1e6;
        let t = Instant::now();
        let graph = file::load_graph_str(&json, None)?;
        row.read = ms(t);
        let t = Instant::now();
        let mut ev = eval::Evaluator::new();
        let (design, report) = eval::design_of(&mut ev, &graph, &reg, &lib, lib.revision())?;
        row.evaluate = Some(ms(t));
        let t = Instant::now();
        let mut baked = (*lib).clone();
        design.unpack_and_bake_observed(&mut baked, &|_, _| {}, &never).expect("not cancelled");
        row.bake = ms(t);
        row.mtexels = baked.changed_since(&lib).map(|a| a.data.len()).sum::<usize>() as f64 / 1e6;
        let t = Instant::now();
        let mut again = (*lib).clone();
        design.unpack_and_bake_observed(&mut again, &|_, _| {}, &never).expect("not cancelled");
        row.bake_again = ms(t);
        let baked = Arc::new(baked);
        let t = Instant::now();
        ringdesign_core::dfm::findings_in(&design, &baked);
        row.detail = ms(t);
        let t = Instant::now();
        ringdesign_core::dfm::findings_in(&design, &baked);
        row.detail_again = ms(t);
        let t = Instant::now();
        mesh::try_build(&design, &baked, params)?;
        row.build = ms(t);
        // The build worker takes the open's evaluation for its first build, then evaluates against the same base.
        let t = Instant::now();
        let first = eval::judge(&mut ev, design.clone(), report, &graph, &baked);
        row.verdict = ms(t);
        let host = first.baked_library.clone().unwrap_or_else(|| baked.clone());
        let t = Instant::now();
        let again = eval::evaluate_design_onto(&mut ev, &graph, &reg, &lib, &host)?;
        row.rebuild = Some(ms(t));
        assert!(again.baked_library.is_none() && Arc::ptr_eq(&again.design, &design), "{}: a rebuild of an unchanged graph reuses everything", template.slug);
        row.print(template.slug);
        if per_source {
            sources(&design, &lib);
        }
        // The menu opens a graph with a starter of its name as the starter.
        if let Some(starter) = ringdesign_core::templates::all().iter().find(|s| s.name == template.name) {
            let mut row = Row::default();
            let t = Instant::now();
            let design = starter.design();
            row.read = ms(t);
            opened_design(&mut row, &design, &lib, params, &never)?;
            row.print(&format!("{} (menu: starter)", template.slug));
        }
    }
    for asset in ringdesign_assets::DESIGNS.iter().filter(|a| wanted(a.name)) {
        let mut row = Row::default();
        let t = Instant::now();
        let text = asset.text();
        row.unpack = Some(ms(t));
        row.mb = text.len() as f64 / 1e6;
        let t = Instant::now();
        let design = templates::refine_sources(&serde_json::from_str(&text)?);
        row.read = ms(t);
        opened_design(&mut row, &design, &lib, params, &never)?;
        row.print(&format!("{} (menu: design)", asset.name));
        if per_source {
            sources(&design, &lib);
        }
    }
    println!("shared bakes hold {:.1} Mtexels", ringdesign_core::alpha::bake_cache_texels() as f64 / 1e6);
    Ok(())
}
