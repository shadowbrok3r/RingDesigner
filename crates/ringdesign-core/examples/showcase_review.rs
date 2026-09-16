//! Inspect and render a saved showcase candidate using the actual casting pattern.
use anyhow::{Result, ensure};
use ringdesign_core::{AlphaLibrary, BuildParams, library, manufacturing as mf, render};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(args.len() >= 2, "showcase_review SOURCE NEW_DIRECTORY [--final]");
    let final_run = args.iter().any(|s| s == "--final");
    let out = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out)?;
    let mut d = library::load_design(&args[0])?;
    let empty = AlphaLibrary::default();
    let lib = mf::source_library(&d, &empty).into_owned();
    let params = BuildParams {
        theta_steps: if final_run { 1920 } else { 768 },
        profile_steps: if final_run { 640 } else { 320 },
        refine: None,
        ..Default::default()
    };
    d.build = params;
    let setup = d.manufacturing.clone().unwrap_or_else(|| mf::Setup::from_design(&d));
    let inspected = mf::inspect(&d, &lib, &setup, params)?;
    let report = mf::package::report(&d, &setup, &inspected, false);
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("{}: {:?}, {} obstructions / {} unresolved, field {:?}", d.name,
        inspected.release.status, inspected.release.obstructions.len(),
        inspected.release.unresolved_rays, inspected.field.as_ref().map(|f| (f.verdict, f.thinnest_wall_mm)));
    let mesh = ringdesign_core::mesh::try_build(&d, &lib, params)?.mesh;
    for (name, yaw, pitch) in [("hero", 0.48, 1.0), ("seal", 0., std::f64::consts::FRAC_PI_2), ("cheek", 0., 0.24)] {
        render::write_png(out.join(format!("{name}.png")), &mesh, yaw, pitch, if final_run { 1600 } else { 1000 }, render::GOLD)?;
    }
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    if final_run {
        ensure!(inspected.release.obstructions.is_empty() && inspected.release.unresolved_rays == 0, "Pattern does not release");
        let mut fine = setup.clone();
        fine.sample_pitch_mm = 0.075;
        let r = mf::release::analyze(&inspected.prepared.mesh, &fine)?;
        std::fs::write(out.join("release-fine.json"), serde_json::to_vec_pretty(&r)?)?;
        ensure!(r.obstructions.is_empty() && r.unresolved_rays == 0, "Fine sampling found obstruction");
        mf::package::export(&out.join("pattern-package"), &d, &lib, &setup, params, false)?;
        ringdesign_core::stl::write_stl(out.join("nominal.stl"), &mesh, &d.name)?;
    }
    Ok(())
}
