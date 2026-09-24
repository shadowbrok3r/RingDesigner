//! Regenerate the actual CAD example previews used by both applications: the finished metal in studio gold,
//! stones set.
use ringdesign_core::{AlphaLibrary, BuildParams, cad, render};
fn main() -> anyhow::Result<()> {
    let dest = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dest)?;
    let lib = AlphaLibrary::builtin();
    for name in cad::examples::NAMES {
        let design = cad::examples::design(name)?;
        let finished = render::finished(&design, &lib, BuildParams::default())?;
        render::write_png_parts(format!("{dest}/{name}.png"), &finished.parts(render::GOLD), 0.55, 1.12, 160)?;
        println!("{name}: {} triangles, {} of stones", finished.metal.faces.len(), finished.stone_faces());
    }
    Ok(())
}
