use anyhow::{Context, Result, ensure};
use ringdesign_core::{AlphaLibrary, BuildParams, cad, manufacturing as mf, sketch};
use std::path::PathBuf;

pub fn run(args: &[String]) -> Result<()> {
    let command=args.first().map(String::as_str).context("CAD expects example, check, export, step, import, resize, profile-import, profile-export, or calibrate")?;
    let input = args
        .get(1)
        .context("CAD command needs a source file or example name")?;
    let mut output = None;
    let mut bores = None;
    let mut band = false;
    let mut part = None;
    let mut flags = args[2..].iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--out" => output = Some(PathBuf::from(flags.next().context("--out needs a path")?)),
            "--part" => part = Some(PathBuf::from(flags.next().context("--part needs a path")?)),
            "--band" => band = true,
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
        "import" => {
            let path = output.context("Import needs --out design.ring.json")?;
            ensure!(!path.exists(), "Output already exists");
            let part = part.context("Import needs --part part.step, part.stl or part.obj")?;
            let (feature, notes) = part_file(&part)?;
            let mut d = ringdesign_core::library::load_design(input).map_err(|e| anyhow::anyhow!("{input}: {e}"))?;
            let name = feature.name.clone();
            let cad::Operation::Stored { mesh, .. } = &feature.operation else { anyhow::bail!("{name} did not come in as a stored part") };
            let (triangles, volume) = (mesh.triangles, mesh.made()?.solid().volume());
            for edit in cad::stored::import_edits(&d, feature) {
                ringdesign_graph::nodes::cad::edit_design(&mut d, &edit)?;
            }
            // A graph-driven design is evaluated with the part in its graph.
            let d = super::evaluated(d)?;
            ringdesign_core::library::save_design(&path, &d)?;
            println!("Imported {name}: {triangles} triangles, {volume:.4} mm³, joined at the top of the ring: {}", path.display());
            for n in notes {
                println!("  {n}");
            }
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
                    let (text, said) = if band {
                        let sized = cad::step::ring_sized(&d, &lib, params, cad::step::BAND_TOLERANCE_MM, &d.name)?;
                        let said = sized.summary();
                        (sized.text, Some(said))
                    } else {
                        (cad::step::export(&cad::evaluate(&d, &lib, params)?, &d.name)?, None)
                    };
                    // Every solid read back as an import reads it.
                    let solids = cad::step::read_meshes(&text)?;
                    ringdesign_core::library::write_atomic(&path, text.as_bytes())?;
                    let faceted = solids.iter().filter(|s| s.faceted).count();
                    println!("STEP solids: {} analytic, {faceted} faceted: {}", solids.len() - faceted, path.display());
                    if let Some(said) = said {
                        println!("  {said}");
                    }
                    for s in &solids {
                        let kind = if s.faceted { "faceted" } else { "exact" };
                        match &s.mesh {
                            Ok(mesh) => println!("  {} — {kind}, reads back closed at {:.4} mm³", s.name, mesh.volume_mm3()),
                            Err(why) => println!("  {} — {kind}, reads only where OpenCascade is: {why}", s.name),
                        }
                    }
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

/// A part file as a stored feature and what reading it said: STEP by the core's reader, its exact solids through cadkernel; STL and OBJ by the solid crate's readers.
fn part_file(path: &std::path::Path) -> Result<(cad::Feature, Vec<String>)> {
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("The file").to_string();
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let (format, solids, notes) = match ext.as_str() {
        "step" | "stp" => {
            let text = std::fs::read_to_string(path).with_context(|| format!("{file} could not be read"))?;
            let (meshes, notes) = cad::step::solid_meshes(&text, &file)?;
            ("step", meshes, notes)
        }
        "stl" => ("stl", vec![ringdesign_solid::io::read_stl(path).with_context(|| format!("{file} does not read as STL"))?], Vec::new()),
        "obj" => ("obj", vec![ringdesign_solid::io::read_obj(path).with_context(|| format!("{file} does not read as OBJ"))?], Vec::new()),
        _ => anyhow::bail!("{file}: a part comes in as STEP, STL or OBJ"),
    };
    Ok((cad::stored::imported(&file, format, &solids)?, notes))
}
