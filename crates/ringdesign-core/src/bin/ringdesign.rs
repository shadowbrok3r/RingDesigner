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
//! ```

use std::path::{Path, PathBuf};

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::analyze_field;
use ringdesign_core::mesh::build;
use ringdesign_core::sizing::RingSize;
use ringdesign_core::{library, metal, stl, stones, threemf, RingDesign};

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

options:
  --sizes 5:9:0.5 | 6,7,8   sizes to run (default: the design's own)
  --formats stl,obj,3mf,glb,ply   files per size (default: stl)
  --shrink <metal>          cut patterns oversize for this metal's shrink
                            (sterling, bronze, 14k, ... — see the app's table)
  --out <dir>               output directory (default: beside the design)
  --steps 1024x320          sweep resolution (default: the design's build)";

fn run(args: &[String]) -> anyhow::Result<()> {
    let (cmd, design_path) = match args {
        [c, p, ..] => (c.as_str(), p.as_str()),
        _ => anyhow::bail!("expected a command and a design file"),
    };
    let design = load(design_path)?;
    let mut lib = AlphaLibrary::builtin();
    design.unpack_embedded(&mut lib);
    design.bake_drawn(&mut lib);
    design.bake_texts(&mut lib);
    design.bake_svgs(&mut lib);

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
                    if !matches!(f.as_str(), "stl" | "obj" | "3mf" | "glb" | "ply") {
                        anyhow::bail!("unknown format {f:?} (stl, obj, 3mf)");
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
        "size,file,format,bytes,triangles,watertight,verdict,undercut_pct,thinnest_wall_mm,volume_mm3\n",
    );

    for &size in &sizes {
        let mut d = base.clone();
        d.size = RingSize(size);
        let f = analyze_field(&d, lib, &d.draft, 192, 128);
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
            "size {:>4}  {}  {} tris{}",
            d.size.display(),
            f.verdict.label(),
            v.triangle_count,
            if v.watertight { "" } else { "  NOT WATERTIGHT" }
        );

        for fmt in &formats {
            let tag = match scale {
                Some((m, _)) => {
                    format!("_pattern-{}", slug_of(m.name))
                }
                None => String::new(),
            };
            let file = out_dir.join(format!("{slug}_size{}{}.{fmt}", fmt_size(size), tag));
            let bytes = match fmt.as_str() {
                "stl" => stl::write_stl(&file, &mesh, &name)?,
                "obj" => stl::write_obj(&file, &mesh, &name)?,
                "glb" => ringdesign_core::gltf::write_glb(&file, &mesh, &name, ringdesign_core::render::GOLD)?,
                "ply" => stl::write_ply(&file, &mesh, &name)?,
                _ => threemf::write_3mf(&file, &mesh, &name, &d.size.display())?,
            };
            manifest.push_str(&format!(
                "{},{},{},{},{},{},{:?},{:.4},{:.2},{:.2}\n",
                d.size.display(),
                file.file_name().unwrap_or_default().to_string_lossy(),
                fmt,
                bytes,
                v.triangle_count,
                v.watertight,
                f.verdict,
                f.undercut_fraction() * 100.0,
                f.thinnest_wall_mm,
                built.report.volume_mm3
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
