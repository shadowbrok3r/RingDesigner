// Render a saved .ring.json from several angles: render_design <design> <out dir>
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::mesh::{self, BuildParams};

const W: usize = 820;
const H: usize = 820;

#[path = "common/raster.rs"]
mod raster;

fn main() {
    let mut args = std::env::args().skip(1);
    let design_path = args.next().expect("design path");
    let out = args.next().unwrap_or_else(|| "/tmp".into());

    let d = ringdesign_core::library::load_design(&design_path).expect("load design");
    let mut lib = AlphaLibrary::builtin();
    for dir in ringdesign_core::library::alpha_dirs() {
        let _ = lib.load_dir(dir);
    }

    let params = BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() };
    let built = mesh::build(&d, &lib, params);
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    println!(
        "{}  {}  {:.3}% undercut  worst {:+.2} deg  {} tris  watertight {}",
        d.name,
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        rep.worst_draft_deg,
        built.mesh.faces.len(),
        built.report.validation.watertight
    );

    let stem = std::path::Path::new(&design_path)
        .file_stem()
        .map(|s| s.to_string_lossy().replace(".ring", ""))
        .unwrap_or_else(|| "design".into());

    // A head faces +Y, so a pitch near a right angle looks straight at it.
    for (view, yaw, pitch, classed) in [
        ("hero", 0.5, 1.15, false),
        ("face", 0.0, 1.571, false),
        ("down", 0.55, 0.55, false),
        ("draft", 0.5, 1.15, true),
    ] {
        let img = raster::render_classed(
            &built.mesh,
            yaw,
            pitch,
            W,
            H,
            classed.then_some(rep.classes.as_slice()),
        );
        let path = format!("{out}/{stem}_{view}.png");
        image::save_buffer(&path, &img, W as u32, H as u32, image::ColorType::Rgb8).unwrap();
        println!("  {path}");
    }
}
