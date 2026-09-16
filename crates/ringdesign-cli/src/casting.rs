use ringdesign_core::{
    AlphaLibrary, BuildParams, library,
    manufacturing::{
        self as mf, Recipe, Setup,
        release::Status,
        repair::{self, Repair},
    },
};
use std::path::PathBuf;

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let (cmd, path) = match args {
        [cmd, path, ..] => (cmd.as_str(), path),
        _ => anyhow::bail!("casting expects check, export, or repair and a design file"),
    };
    let d = super::load(path)?;
    let mut setup = d
        .manufacturing
        .clone()
        .unwrap_or_else(|| Setup::from_design(&d));
    let mut out = None;
    let mut json = false;
    let mut diagnostic = false;
    let mut layer = None;
    let mut repair_kind = None;
    let mut params = BuildParams {
        theta_steps: 256,
        profile_steps: 128,
        refine: None,
        ..d.build
    };
    let mut flags = args[2..].iter();
    while let Some(flag) = flags.next() {
        let mut value = || {
            flags
                .next()
                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))
        };
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(value()?)),
            "--recipe" => setup.recipe = Recipe::load(value()?)?,
            "--component" => setup.component = Some(value()?.parse()?),
            "--pull" => {
                let v = value()?
                    .split(',')
                    .map(str::parse::<f64>)
                    .collect::<Result<Vec<_>, _>>()?;
                anyhow::ensure!(v.len() == 3, "--pull needs x,y,z");
                setup.pull = [v[0], v[1], v[2]];
            }
            "--parting" => {
                setup.parting_mm = value()?.parse()?;
                setup.auto_parting = false;
            }
            "--pitch" => setup.sample_pitch_mm = value()?.parse()?,
            "--json" => json = true,
            "--diagnostic" => diagnostic = true,
            "--layer" => layer = Some(value()?.parse::<usize>()?),
            "--repair" => {
                repair_kind = Some(match value()?.as_str() {
                    "half" => Repair::ReduceRelief,
                    "side" => Repair::MoveToSide,
                    "bench" => Repair::DeferToBench,
                    "square" => Repair::SquareSides,
                    "parting" => Repair::SuggestedParting,
                    other => anyhow::bail!("Unknown repair {other}"),
                })
            }
            "--steps" => {
                let v = value()?;
                let (a, b) = v
                    .split_once('x')
                    .ok_or_else(|| anyhow::anyhow!("--steps needs THETAxPROFILE"))?;
                params.theta_steps = a.parse()?;
                params.profile_steps = b.parse()?;
                anyhow::ensure!(
                    (24..=2048).contains(&params.theta_steps)
                        && (16..=1024).contains(&params.profile_steps),
                    "Steps exceed supported bounds"
                );
            }
            other => anyhow::bail!("Unknown casting option {other}"),
        }
    }
    let mut lib = AlphaLibrary::builtin();
    d.unpack_embedded(&mut lib);
    d.bake_all(&mut lib);
    match cmd {
        "check" => {
            let i = mf::inspect(&d, &lib, &setup, params)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&mf::package::report(&d, &setup, &i, false))?
                );
            } else {
                println!("{}: {}", d.name, i.release.status.label());
                println!(
                    "Recipe: {}; pattern ×{:.6}; parting {:+.3} mm",
                    setup.recipe.name, i.prepared.scale, i.release.parting_mm
                );
                for o in &i.release.obstructions {
                    println!(
                        "{}: {:.3} mm obstruction at [{:.2}, {:.2}, {:.2}]",
                        o.half.label(),
                        o.depth_mm,
                        o.point[0],
                        o.point[1],
                        o.point[2]
                    );
                }
                for n in i.release.notes.iter().chain(i.details.iter()) {
                    println!("  {n}");
                }
            }
            anyhow::ensure!(
                !matches!(i.release.status, Status::Invalid | Status::Blocked),
                "Manufacturing check failed; inspect the reported findings"
            );
        }
        "export" => {
            let path =
                out.ok_or_else(|| anyhow::anyhow!("Export requires --out <new-directory>"))?;
            let report = mf::package::export(&path, &d, &lib, &setup, params, diagnostic)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Pattern package: {}", path.display());
            }
        }
        "repair" => {
            let path =
                out.ok_or_else(|| anyhow::anyhow!("Repair requires --out <new-design.json>"))?;
            anyhow::ensure!(!path.exists(), "Output design already exists");
            let kind = repair_kind.ok_or_else(|| {
                anyhow::anyhow!("Specify --repair half|side|bench|square|parting")
            })?;
            let before = mf::inspect(&d, &lib, &setup, params)?;
            let candidate =
                repair::candidate(&d, &setup, layer, kind, before.release.suggested_parting_mm)?;
            let after = mf::inspect(
                &candidate,
                &lib,
                candidate.manufacturing.as_ref().unwrap(),
                params,
            )?;
            library::save_design_embedded(&path, &candidate, &lib)?;
            println!(
                "{}: {} → {} obstructions; candidate saved to {}",
                kind.label(),
                before.release.obstructions.len(),
                after.release.obstructions.len(),
                path.display()
            );
        }
        other => anyhow::bail!("Unknown casting command {other}"),
    }
    Ok(())
}
