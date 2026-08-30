//! Batch export from the command line — the size-run tool.
//!
//! Sand casters pour size runs in one flask, so the unit of work here is a
//! design times a list of sizes: each size is resized, field-checked, built
//! at export resolution and written, with a manifest CSV naming what every
//! file is and what the verdict was. Doubles as the export-regression
//! harness: the manifest diffs.
//!
//! ```text
//! ringdesign export ring.json --sizes 5:9:0.5 --formats stl,3mf --shrink sterling
//! ringdesign check ring.json
//! ringdesign graph eval court.graph.json --set Width=6 --out court.ring.json
//! ```
//!
//! The graph commands evaluate a `.graph.json` the way the app does — the
//! same registry, the script engine attached — so a graph is a design
//! file that can be parameterized from the shell.

use std::path::{Path, PathBuf};

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::analyze_field;
use ringdesign_core::mesh::build;
use ringdesign_core::sizing::RingSize;
use ringdesign_core::{RingDesign, library, metal, stl, stones, threemf};
use ringdesign_graph::eval::{Evaluator, Targets, evaluate_design};
use ringdesign_graph::file;
use ringdesign_graph::graph::Graph;
use ringdesign_graph::value::Literal;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("ringdesign: {e}");
        eprintln!();
        eprintln!("{USAGE}");
        std::process::exit(1);
    }
}

const USAGE: &str = "usage:
  ringdesign export <design.json> [options]
  ringdesign check  <design.json>
  ringdesign graph eval     <graph.json> [--set Name=value]* [--preset name] [--out design.ring.json] [--run-sinks]
  ringdesign graph check    <graph.json> [--set Name=value]*
  ringdesign graph describe <graph.json>

options:
  --sizes 5:9:0.5 | 6,7,8   sizes to run (default: the design's own)
  --formats stl,obj,3mf,glb,ply,stonemap   files per size (default: stl); stonemap is the setter's SVG
  --shrink <metal>          cut patterns oversize for this metal's shrink
                            (sterling, bronze, 14k, ... — see the app's table)
  --out <dir>               output directory (default: beside the design)
  --steps 1024x320          sweep resolution; overrides a saved refine tolerance";

fn run(args: &[String]) -> anyhow::Result<()> {
    if args.first().map(String::as_str) == Some("graph") {
        return graph::run(&args[1..]);
    }
    let (cmd, design_path) = match args {
        [c, p, ..] => (c.as_str(), p.as_str()),
        _ => anyhow::bail!("expected a command and a design file"),
    };
    let design = load(design_path)?;
    let mut lib = AlphaLibrary::builtin();
    design.unpack_embedded(&mut lib);
    design.bake_all(&mut lib);

    match cmd {
        "check" => check(&design, &lib),
        "export" => export(design_path, design, &lib, &args[2..]),
        other => anyhow::bail!("unknown command {other:?}"),
    }
}

fn load(path: &str) -> anyhow::Result<RingDesign> {
    library::load_design(path).map_err(|e| anyhow::anyhow!("{path}: {e}"))
}

/// The field verdict and the stones checks, printed plainly.
fn check(design: &RingDesign, lib: &AlphaLibrary) -> anyhow::Result<()> {
    let f = analyze_field(design, lib, &design.draft, 192, 128);
    println!(
        "{}  size {}  —  {}",
        design.name,
        design.size.display(),
        f.verdict.label()
    );
    println!(
        "  undercut {:.3}% of surface, worst {:+.1} deg, thinnest wall {:.2} mm at {:.0} deg",
        f.undercut_fraction() * 100.0,
        f.worst_draft_deg,
        f.thinnest_wall_mm,
        f.thinnest_wall_theta_deg
    );
    for n in &f.notes {
        println!("  • {n}");
    }
    if let Some(s) = stones::report(design, f.parting_z_mm) {
        println!("  {} stones, {:.2} ct total", s.stone_count, s.total_carats);
        for seat in &s.seats {
            for w in &seat.warnings {
                println!("  ! {}: {w}", seat.label);
            }
        }
    }
    Ok(())
}

fn export(
    design_path: &str,
    base: RingDesign,
    lib: &AlphaLibrary,
    opts: &[String],
) -> anyhow::Result<()> {
    let mut sizes: Vec<f64> = vec![base.size.0];
    let mut formats: Vec<String> = vec!["stl".into()];
    let mut shrink: Option<&'static metal::Metal> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut params = base.build;

    let mut it = opts.iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--sizes" => sizes = parse_sizes(value()?)?,
            "--formats" => {
                formats = value()?.split(',').map(|s| s.trim().to_lowercase()).collect();
                for f in &formats {
                    if !matches!(f.as_str(), "stl" | "obj" | "3mf" | "glb" | "ply" | "stonemap") {
                        anyhow::bail!("unknown format {f:?} (stl, obj, 3mf, glb, ply, stonemap)");
                    }
                }
            }
            "--shrink" => {
                let name = value()?;
                shrink = Some(
                    metal::find(name)
                        .ok_or_else(|| anyhow::anyhow!("no metal matches {name:?}"))?,
                );
            }
            "--out" => out_dir = Some(PathBuf::from(value()?)),
            "--steps" => {
                let v = value()?;
                let (t, p) = v
                    .split_once(['x', 'X'])
                    .ok_or_else(|| anyhow::anyhow!("--steps wants THETAxPROFILE, e.g. 1024x320"))?;
                params.theta_steps = t.trim().parse()?;
                params.profile_steps = p.trim().parse()?;
                // A refine tolerance short-circuits the sweep in `mesh::build`,
                // so a design carrying one swallowed this flag whole and wrote
                // a file at a resolution nobody asked for. Naming a sweep
                // resolution is asking for the sweep.
                if params.refine.take().is_some() {
                    eprintln!(
                        "note: --steps asks for a swept build, so the design's refine tolerance is ignored for this run"
                    );
                }
            }
            other => anyhow::bail!("unknown option {other:?}"),
        }
    }

    let out_dir = out_dir.unwrap_or_else(|| {
        Path::new(design_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    });
    std::fs::create_dir_all(&out_dir)?;

    let scale = shrink.map(|m| (m, metal::pattern_scale(m.shrink_pct)));
    let slug = slug(&base.name);
    let mut manifest = String::from(
        "size,file,format,bytes,triangles,watertight,verdict,undercut_pct,thinnest_wall_mm,volume_mm3,dfm_findings,stone_warnings\n",
    );

    for &size in &sizes {
        let mut d = base.clone();
        d.size = RingSize(size);
        let f = analyze_field(&d, lib, &d.draft, 192, 128);
        // The run used to gate on the field verdict alone. Both of the other
        // checks move with the size: circumference grows 20% from a 5 to a 9,
        // so a tiling's cell pitch changes and a run's stone bridges close.
        let dfm = ringdesign_core::dfm::findings_in(&d, lib);
        let stones_at = stones::report(&d, f.parting_z_mm);
        let stone_warnings: usize = stones_at
            .as_ref()
            .map(|s| s.seats.iter().map(|c| c.warnings.len()).sum())
            .unwrap_or(0);
        let built = build(&d, lib, params);
        let v = built.report.validation;
        let (mesh, name) = match scale {
            Some((m, k)) => (
                built.mesh.scaled(k),
                format!("{} [pattern +{:.1}% for {}]", d.name, m.shrink_pct, m.name),
            ),
            None => (built.mesh.clone(), d.name.clone()),
        };
        println!(
            "size {:>4}  {}  {} tris{}{}{}",
            d.size.display(),
            f.verdict.label(),
            v.triangle_count,
            if v.watertight { "" } else { "  NOT WATERTIGHT" },
            if dfm.is_empty() { String::new() } else { format!("  {} DFM", dfm.len()) },
            if stone_warnings == 0 {
                String::new()
            } else {
                format!("  {stone_warnings} stone")
            }
        );
        for finding in &dfm {
            println!("        {}: {}", finding.label, finding.message);
        }
        if let Some(s) = &stones_at {
            for seat in &s.seats {
                for w in &seat.warnings {
                    println!("        {}: {w}", seat.label);
                }
            }
        }

        for fmt in &formats {
            let tag = match scale {
                Some((m, _)) => {
                    format!("_pattern-{}", slug_of(m.name))
                }
                None => String::new(),
            };
            let file = if fmt == "stonemap" {
                out_dir.join(format!("{slug}_size{}_stones.svg", fmt_size(size)))
            } else {
                out_dir.join(format!("{slug}_size{}{}.{fmt}", fmt_size(size), tag))
            };
            let bytes = match fmt.as_str() {
                "stonemap" => {
                    ringdesign_core::stonemap::write_stone_map_svg(&file, &d, stones_at.as_ref())?
                }
                "stl" => stl::write_stl(&file, &mesh, &name)?,
                "obj" => stl::write_obj(&file, &mesh, &name)?,
                "glb" => ringdesign_core::gltf::write_glb(&file, &mesh, &name, ringdesign_core::render::GOLD)?,
                "ply" => stl::write_ply(&file, &mesh, &name)?,
                _ => {
                    // The mesh is scaled for shrink, bore and all — an
                    // oversize file stamped with the nominal size is a ring
                    // that comes out a size small.
                    let label = match scale {
                        Some((m, _)) => format!(
                            "{} pattern, cut +{:.1}% for {}",
                            d.size.display(),
                            m.shrink_pct,
                            m.name
                        ),
                        None => d.size.display(),
                    };
                    threemf::write_3mf(&file, &mesh, &name, &label)?
                }
            };
            manifest.push_str(&format!(
                "{},{},{},{},{},{},{:?},{:.4},{:.2},{:.2},{},{}\n",
                d.size.display(),
                file.file_name().unwrap_or_default().to_string_lossy(),
                fmt,
                bytes,
                v.triangle_count,
                v.watertight,
                f.verdict,
                f.undercut_fraction() * 100.0,
                f.thinnest_wall_mm,
                built.report.volume_mm3,
                dfm.len(),
                stone_warnings
            ));
        }
    }

    let manifest_path = out_dir.join(format!("{slug}_manifest.csv"));
    std::fs::write(&manifest_path, manifest)?;
    println!("manifest {}", manifest_path.display());
    Ok(())
}

/// A size as a filename chunk: `6`, `6.5`, `11.25`.
fn fmt_size(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `5:9:0.5` inclusive, or a comma list.
fn parse_sizes(spec: &str) -> anyhow::Result<Vec<f64>> {
    if spec.contains(':') {
        let parts: Vec<&str> = spec.split(':').collect();
        let [a, b, step] = parts.as_slice() else {
            anyhow::bail!("--sizes wants FROM:TO:STEP or a comma list");
        };
        let (a, b, step): (f64, f64, f64) = (a.parse()?, b.parse()?, step.parse()?);
        if !(step > 1e-6) || b < a {
            anyhow::bail!("--sizes range must climb");
        }
        let n = ((b - a) / step).round() as usize;
        anyhow::ensure!(n <= 200, "--sizes: {n} sizes is past any flask");
        Ok((0..=n).map(|i| a + i as f64 * step).collect())
    } else {
        spec.split(',')
            .map(|s| s.trim().parse::<f64>().map_err(Into::into))
            .collect()
    }
}

fn slug(name: &str) -> String {
    let s = slug_of(name);
    if s.is_empty() { "ring".into() } else { s }
}

fn slug_of(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_specs_parse_both_ways() {
        assert_eq!(parse_sizes("6,7,8").unwrap(), vec![6.0, 7.0, 8.0]);
        let run = parse_sizes("5:9:0.5").unwrap();
        assert_eq!(run.len(), 9);
        assert_eq!(run[0], 5.0);
        assert_eq!(*run.last().unwrap(), 9.0);
        assert!(parse_sizes("9:5:0.5").is_err());
        assert!(parse_sizes("1:1000:0.01").is_err());
    }

    #[test]
    fn slugs_are_filename_safe() {
        assert_eq!(slug("My Heart / Signet!"), "my-heart-signet");
        assert_eq!(slug("***"), "ring");
    }
}

/// `ringdesign graph …`: a graph file evaluated like the app does it.
mod graph {
    use super::*;

    pub fn run(args: &[String]) -> anyhow::Result<()> {
        let (sub, path) = match args {
            [s, p, ..] => (s.as_str(), p.as_str()),
            _ => anyhow::bail!("expected `graph eval|check|describe <graph.json>`"),
        };
        let reg = ringdesign_script::registry();
        let mut g = file::load_graph(path, Some(&reg)).map_err(|e| anyhow::anyhow!("{path}: {e:#}"))?;
        let mut out: Option<PathBuf> = None;
        let mut run_sinks = false;
        let mut rest = args[2..].iter();
        while let Some(flag) = rest.next() {
            match flag.as_str() {
                "--set" => {
                    let kv = rest.next().ok_or_else(|| anyhow::anyhow!("--set needs Name=value"))?;
                    let (name, value) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("--set {kv:?}: expected Name=value"))?;
                    set_exposed(&mut g, name.trim(), value.trim())?;
                }
                "--preset" => {
                    let name = rest.next().ok_or_else(|| anyhow::anyhow!("--preset needs a name"))?;
                    let preset = file::list_presets().into_iter().find(|p| &p.name == name).ok_or_else(|| anyhow::anyhow!("no preset {name:?} in {}", file::preset_dir().display()))?;
                    for (k, v) in &preset.values {
                        set_exposed_literal(&mut g, k, v.clone())?;
                    }
                }
                "--out" => out = Some(PathBuf::from(rest.next().ok_or_else(|| anyhow::anyhow!("--out needs a path"))?)),
                "--run-sinks" => run_sinks = true,
                other => anyhow::bail!("unknown option {other:?}"),
            }
        }
        let errors = g.validate(Some(&reg));
        match sub {
            "describe" => {
                println!("{} ({:?}, {} nodes, {} wires)", g.name, g.mode, g.nodes.len(), g.wires.len());
                for n in &g.nodes {
                    let (ins, outs) = reg.node_pins(n).unwrap_or_default();
                    let lits: Vec<String> = n.inputs.iter().map(|(k, v)| format!("{k}={}", serde_json::to_string(v).unwrap_or_default())).collect();
                    println!(
                        "  {:>4}  {:<24} {:<18} in: {}  out: {}{}",
                        n.id,
                        n.kind,
                        n.label.clone().unwrap_or_default(),
                        ins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(","),
                        outs.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(","),
                        if lits.is_empty() { String::new() } else { format!("  [{}]", lits.join(" ")) }
                    );
                }
                for w in &g.wires {
                    println!("  {}.{} -> {}.{}", w.from, w.out, w.to, w.input);
                }
                for e in &g.exposed {
                    println!("  exposed {} = {}.{}", e.name, e.node, e.input);
                }
                for e in &errors {
                    println!("  error: {e}");
                }
                Ok(())
            }
            "eval" | "check" => {
                if !errors.is_empty() {
                    for e in &errors {
                        eprintln!("error: {e}");
                    }
                    anyhow::bail!("the graph does not validate");
                }
                let mut lib = AlphaLibrary::builtin();
                if let Err(e) = lib.load_dir(library::user_alpha_dir()) {
                    eprintln!("note: user alphas not loaded: {e}");
                }
                let mut ev = Evaluator::with_exprs(ringdesign_script::engine());
                let result = evaluate_design(&mut ev, &g, &reg, &lib, 0).map_err(|e| anyhow::anyhow!("{e}"))?;
                for n in &result.notes {
                    eprintln!("note: {n}");
                }
                let f = &result.field;
                println!(
                    "{}: {}: {:.4}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
                    result.design.name,
                    f.verdict.label(),
                    f.undercut_fraction() * 100.0,
                    f.worst_draft_deg,
                    f.thinnest_wall_mm
                );
                for n in &f.notes {
                    println!("  {n}");
                }
                if sub == "check" {
                    return Ok(());
                }
                if run_sinks {
                    let report = ev.evaluate(&g, &reg, &lib, 0, Targets::Everything);
                    for line in report.notes(&g) {
                        eprintln!("sink: {line}");
                    }
                    println!("ran {} nodes with side effects", report.ran().len());
                }
                if let Some(path) = out {
                    let mut d = (*result.design).clone();
                    d.graph = Some(serde_json::to_value(&g)?);
                    let mut baked = lib.clone();
                    d.bake_all(&mut baked);
                    library::save_design_embedded(&path, &d, &baked)?;
                    println!("wrote {}", path.display());
                }
                Ok(())
            }
            other => anyhow::bail!("unknown graph command {other:?} (eval, check, describe)"),
        }
    }

    /// `--set Name=value`: the value parses as a literal (JSON), else as text.
    fn set_exposed(g: &mut Graph, name: &str, value: &str) -> anyhow::Result<()> {
        let lit: Literal = serde_json::from_str(value).unwrap_or_else(|_| Literal::Text(value.to_string()));
        set_exposed_literal(g, name, lit)
    }

    fn set_exposed_literal(g: &mut Graph, name: &str, lit: Literal) -> anyhow::Result<()> {
        let e = g
            .exposed
            .iter()
            .find(|e| e.name == name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("{name:?} is not an exposed parameter; the graph exposes {:?}", g.exposed.iter().map(|e| e.name.as_str()).collect::<Vec<_>>()))?;
        g.set_input(e.node, e.input, lit).map_err(|er| anyhow::anyhow!("{er}"))?;
        Ok(())
    }
}
