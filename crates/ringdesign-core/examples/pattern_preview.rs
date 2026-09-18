//! The casting pattern of a saved design beside its finished ring: what the founder is handed.
//! cargo run --release -p ringdesign-core --example pattern_preview -- DESIGN.ring.json OUT_DIR
use anyhow::{Context, Result};
use ringdesign_core::{AlphaLibrary, BuildParams, gems, library, manufacturing, mesh, render};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().context("pattern_preview DESIGN.ring.json OUT_DIR")?;
    let out = std::path::PathBuf::from(args.next().context("pattern_preview DESIGN.ring.json OUT_DIR")?);
    std::fs::create_dir_all(&out)?;
    let d = library::load_design(&path)?;
    let lib = manufacturing::source_library(&d, &AlphaLibrary::builtin()).into_owned();
    let params = BuildParams { theta_steps: 1280, profile_steps: 384, refine: None, ..d.build };
    let finished = mesh::try_build(&d, &lib, params)?;
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    println!(
        "{}: finished {} triangles with {} made settings; pattern {} triangles, {:.1} mm3 against {:.1}",
        d.name, finished.mesh.faces.len(), finished.solids.resolved, pattern.mesh.faces.len(), pattern.report.volume_mm3, finished.report.volume_mm3
    );
    let stones = gems::preview_mesh(&d, &lib);
    for (view, yaw, pitch) in [("face", 0.0, std::f64::consts::FRAC_PI_2), ("hero", 0.48, 1.0)] {
        let mut parts = vec![render::Part::metal(&finished.mesh, render::GOLD)];
        if let Some(s) = &stones {
            parts.push(render::Part::stone(s));
        }
        render::write_png_parts(out.join(format!("finished-{view}.png")), &parts, yaw, pitch, 1400)?;
        render::write_png(out.join(format!("pattern-{view}.png")), &pattern.mesh, yaw, pitch, 1400, render::GOLD)?;
    }
    Ok(())
}
