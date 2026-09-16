use anyhow::{Context, Result, ensure};
use ringdesign_core::{AlphaLibrary, BuildParams, cad, manufacturing as mf, sketch};
use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<()> {
    let command=args.first().map(String::as_str).context("CAD expects example, check, export, step, resize, profile-import, profile-export, or calibrate")?;
    let input = args
        .get(1)
        .context("CAD command needs a source file or example name")?;
    let mut output = None;
    let mut bores = None;
    let mut flags = args[2..].iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--out" => output = Some(PathBuf::from(flags.next().context("--out needs a path")?)),
            "--bores" => {
                bores = Some(
                    flags
                        .next()
                        .context("--bores needs comma-separated millimeters")?
                        .split(',')
                        .map(str::parse::<f64>)
                        .collect::<std::result::Result<Vec<_>, _>>()?,
                )
            }
            _ => anyhow::bail!("Unknown CAD option {flag}"),
        }
    }
    match command {
        "example" => {
            let path = output.context("Example needs --out design.ring.json")?;
            ensure!(!path.exists(), "Output already exists");
            let mut d = cad::examples::design(input)?;
            let g = ringdesign_graph::nodes::cad::from_document(&d)?;
            d.graph = Some(serde_json::to_value(g)?);
            ringdesign_core::library::save_design(&path, &d)?;
            println!("Editable CAD example: {}", path.display());
        }
        "profile-import" => {
            let path = output.context("Import needs --out sketch.json")?;
            ensure!(!path.exists(), "Output already exists");
            let text = std::fs::read_to_string(input)?;
            let sketch = if input.to_lowercase().ends_with(".svg") {
                sketch::exchange::import_svg(&text)?
            } else {
                sketch::exchange::import_dxf(&text)?
            };
            ringdesign_core::library::write_atomic(&path, &serde_json::to_vec_pretty(&sketch)?)?;
            println!(
                "{} points, {} vector entities",
                sketch.points.len(),
                sketch.entities.len()
            );
        }
        "profile-export" => {
            let path = output.context("Export needs --out profile.svg or profile.dxf")?;
            ensure!(!path.exists(), "Output already exists");
            let s: sketch::Sketch = serde_json::from_slice(&std::fs::read(input)?)?;
            let text = if path.extension().is_some_and(|e| e == "dxf") {
                sketch::exchange::dxf(&s)?
            } else {
                sketch::exchange::svg(&s)?
            };
            ringdesign_core::library::write_atomic(&path, text.as_bytes())?;
            println!("Vector profile in millimeters: {}", path.display());
        }
        _ => {
            let d = super::load(input)?;
            let mut lib = AlphaLibrary::builtin();
            d.unpack_embedded(&mut lib);
            d.bake_all(&mut lib);
            let params = BuildParams {
                theta_steps: 256,
                profile_steps: 128,
                refine: None,
                ..d.build
            };
            match command {
                "check" => {
                    let e = cad::evaluate(&d, &lib, params)?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&cad::assembly::manifest(&d, &e))?
                    );
                }
                "export" => {
                    let path = output.context("Export needs --out <new-directory>")?;
                    cad::assembly::export(&path, &d, &lib, params)?;
                    println!("Assembly package: {}", path.display());
                }
                "step" => {
                    let path = output.context("STEP export needs --out model.step")?;
                    ensure!(!path.exists(), "Output already exists");
                    let e = cad::evaluate(&d, &lib, params)?;
                    let text = cad::step::export(&e, &d.name)?;
                    ringdesign_core::library::write_atomic(&path, text.as_bytes())?;
                    println!("Analytic STEP solids: {}", path.display());
                }
                "calibrate" => {
                    let recipe = d
                        .manufacturing
                        .as_ref()
                        .map(|s| s.recipe.clone())
                        .unwrap_or_default();
                    let result = mf::trials::calibrate(&d.casting_trials, &recipe)?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                    if let Some(path) = output {
                        ensure!(!path.exists(), "Dataset output already exists");
                        let bytes = if path.extension().is_some_and(|e| e == "csv") {
                            mf::trials::csv(&d.casting_trials)?.into_bytes()
                        } else {
                            serde_json::to_vec_pretty(&d.casting_trials)?
                        };
                        ringdesign_core::library::write_atomic(&path, &bytes)?;
                    }
                }
                "resize" => {
                    let path = output.context("Resize needs --out <new-directory>")?;
                    ensure!(!path.exists(), "Output directory already exists");
                    let bores =
                        bores.context("Resize needs --bores 17.3,18.1,... in millimeters")?;
                    let mut ev =
                        ringdesign_graph::eval::Evaluator::with_exprs(ringdesign_script::engine());
                    let variants = ringdesign_graph::variants::evaluate(
                        &d,
                        &bores,
                        &Default::default(),
                        &lib,
                        params,
                        &ringdesign_script::registry(),
                        &mut ev,
                    )?;
                    ringdesign_graph::variants::export(&path, &variants, &lib)?;
                    println!(
                        "{} size variants with manufacturing reports: {}",
                        variants.len(),
                        path.display()
                    );
                }
                _ => anyhow::bail!("Unknown CAD command {command}"),
            }
        }
    }
    Ok(())
}
