// Renders every template to PNG for an eyeball pass.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::mesh::{build, BuildParams};

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let lib = AlphaLibrary::builtin();
    for t in ringdesign_core::templates::all() {
        let d = t.design();
        let out = build(&d, &lib, BuildParams { theta_steps: 512, profile_steps: 192, ..Default::default() });
        let slug: String = t.name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        ringdesign_core::render::write_png(
            format!("{dir}/{slug}.png"), &out.mesh, 0.55, 1.12, 700, ringdesign_core::render::GOLD,
        ).unwrap();
        println!("{}: {} tris", t.name, out.mesh.faces.len());
    }
}
