// Renders every template finished, stones set, in studio gold to PNG for an eyeball pass.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::render;

fn main() {
    let dir = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dir).unwrap();
    let lib = AlphaLibrary::builtin();
    for t in ringdesign_core::templates::all() {
        let d = t.design();
        let finished = match render::finished(&d, &lib, BuildParams { theta_steps: 512, profile_steps: 192, ..Default::default() }) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}: {e:#}", t.name);
                continue;
            }
        };
        let slug: String = t.name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        render::write_png_parts(format!("{dir}/{slug}.png"), &finished.parts(render::GOLD), 0.55, 1.12, 700).unwrap();
        println!("{}: {} tris, {} stone tris", t.name, finished.metal.faces.len(), finished.stone_faces());
    }
}
