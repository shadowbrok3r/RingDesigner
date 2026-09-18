use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams,
    imported_base::{ImportedBase, PRESETS, Source},
    library, render,
};
use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<()> {
    let command = args
        .first()
        .map(String::as_str)
        .context("base: use list, attach, render, or check")?;
    if command == "list" {
        for p in PRESETS {
            println!("{}", p.name);
        }
        return Ok(());
    }
    let input = args.get(1).context("base needs a design path")?;
    let mut output = None;
    let mut source = None;
    let mut preset = None;
    let mut bare = false;
    let mut calibration = None;
    let mut scale = 1.0;
    let mut axes = false;
    let mut flags = args[2..].iter();
    while let Some(f) = flags.next() {
        match f.as_str() {
            "--out" => output = Some(PathBuf::from(flags.next().context("--out needs a path")?)),
            "--base" => {
                source = Some(
                    flags
                        .next()
                        .context("--base needs a .ringbase.json file")?
                        .clone(),
                )
            }
            "--preset" => {
                preset = Some(
                    flags
                        .next()
                        .context("--preset needs a three-digit stock ID")?
                        .clone(),
                )
            }
            "--bare" => bare = true,
            "--calibration" => {
                calibration = Some(
                    flags
                        .next()
                        .context("--calibration needs a JSON file")?
                        .clone(),
                )
            }
            "--scale" => scale = flags.next().context("--scale needs a number")?.parse()?,
            "--crossgems-axes" => axes = true,
            _ => anyhow::bail!("Unknown base flag: {f}"),
        }
    }
    if command == "import" {
        let c = serde_json::from_str(&std::fs::read_to_string(
            calibration.context("import needs --calibration calibration.json")?,
        )?)?;
        let name = std::path::Path::new(input)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let source = Source::from_obj(&std::fs::read_to_string(input)?, name, c, scale, axes)?;
        let mut d = ringdesign_core::RingDesign::default();
        ImportedBase::attach(&mut d, source.clone())?;
        d.imported_base.as_ref().unwrap().validate_shape(&d)?;
        let out = output.context("import needs --out master.ringbase.json")?;
        ensure!(!out.exists(), "Output exists");
        library::write_atomic(&out, &serde_json::to_vec(&*source)?)?;
        println!("Validated portable master: {}", out.display());
        return Ok(());
    }
    let mut d = super::load(input)?;
    match command {
        "attach" => {
            let s = if let Some(path) = source {
                Source::from_json(&std::fs::read_to_string(path)?)?
            } else {
                let id = preset.context("attach needs --preset ID or --base file")?;
                PRESETS
                    .iter()
                    .find(|p| p.name.starts_with(&format!("{id} ·")))
                    .context("Unknown stock ID")?
                    .load()?
            };
            ImportedBase::attach(&mut d, s)?;
            d.imported_base.as_ref().unwrap().validate_shape(&d)?;
            let out = output.context("attach needs --out design.ring.json")?;
            ensure!(!out.exists(), "Output exists; choose a new design path");
            library::save_design(&out, &d)?;
            println!(
                "Attached {} → {}",
                d.imported_base.as_ref().unwrap().source.name,
                out.display()
            );
        }
        "render" | "check" => {
            if let Some(b) = d.imported_base.as_mut() {
                b.bare = bare || b.bare;
            }
            let mut lib = AlphaLibrary::builtin();
            d.unpack_embedded(&mut lib);
            d.bake_all(&mut lib);
            let built = ringdesign_core::mesh::try_build(
                &d,
                &lib,
                BuildParams {
                    theta_steps: 512,
                    profile_steps: 192,
                    refine: None,
                    ..d.build
                },
            )?;
            if command == "render" {
                let out = output.context("render needs --out image.png")?;
                let gem = if bare {
                    None
                } else {
                    ringdesign_core::gems::preview_mesh(&d, &lib)
                };
                let mut parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
                if let Some(g) = &gem {
                    parts.push(render::Part::stone(g));
                }
                render::write_png_parts(&out, &parts, 0.48, 1.0, 1000)?;
                println!("{}", out.display());
            } else {
                println!("{}", serde_json::to_string_pretty(&built.report)?);
            }
        }
        _ => anyhow::bail!("Unknown base command: {command}"),
    }
    Ok(())
}
