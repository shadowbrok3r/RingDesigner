//! Regenerate the actual CAD example previews used by both applications.
use ringdesign_core::{AlphaLibrary, BuildParams, Mesh, cad, render};
fn main() -> anyhow::Result<()> {
    let dest = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dest)?;
    let lib = AlphaLibrary::builtin();
    for name in cad::examples::NAMES {
        let design = cad::examples::design(name)?;
        let evaluated = cad::evaluate(&design, &lib, BuildParams::default())?;
        let mut mesh = Mesh::default();
        for c in evaluated.components.iter().filter(|c| c.settings.visible) {
            let offset = mesh.vertices.len() as u32;
            mesh.vertices.extend_from_slice(&c.mesh.vertices);
            mesh.normals.extend_from_slice(&c.mesh.normals);
            mesh.faces
                .extend(c.mesh.faces.iter().map(|f| f.map(|v| v + offset)));
        }
        render::write_png(
            format!("{dest}/{name}.png"),
            &mesh,
            0.55,
            1.12,
            160,
            [0.76, 0.80, 0.87],
        )?;
        println!("{name}: {} triangles", mesh.faces.len());
    }
    Ok(())
}
