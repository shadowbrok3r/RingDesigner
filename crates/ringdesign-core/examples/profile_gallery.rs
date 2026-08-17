// Renders every saved profile as a bare band on one contact sheet.
//
//   cargo run --release --example profile_gallery -- [out.png]
//
// Each cell is the section applied to the same 5 mm band via `apply_shape`
// — the shape, never the size — so the sheet is the Saved picker at a
// glance and a roundtrip check on imported sections.
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::{library, render, RingDesign};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "profile-gallery.png".into());
    let profiles = library::list_profiles();
    if profiles.is_empty() {
        println!("no saved profiles in {}", library::profile_dir().display());
        return;
    }
    let lib = ringdesign_core::alpha::AlphaLibrary::builtin();
    let cell = 260usize;
    let cols = 6usize.min(profiles.len().max(1));
    let rows = profiles.len().div_ceil(cols);
    let mut sheet = vec![28u8; cols * cell * rows * cell * 3];

    for (i, (name, shape)) in profiles.iter().enumerate() {
        let mut d = RingDesign::default();
        d.profile.width_mm = 5.0;
        d.profile.thickness_mm = 2.2;
        d.profile.apply_shape(shape);
        let built = mesh::build(
            &d,
            &lib,
            BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() },
        );
        let img = render::render(&built.mesh, 0.55, 1.05, cell, cell);
        let (cx, cy) = (i % cols, i / cols);
        for y in 0..cell {
            for x in 0..cell {
                let src = (y * cell + x) * 3;
                let dst = ((cy * cell + y) * cols * cell + cx * cell + x) * 3;
                sheet[dst..dst + 3].copy_from_slice(&img[src..src + 3]);
            }
        }
        println!("{i:3} {name}");
    }
    image::save_buffer(
        &out,
        &sheet,
        (cols * cell) as u32,
        (rows * cell) as u32,
        image::ColorType::Rgb8,
    )
    .unwrap();
    println!("{} profiles -> {out}", profiles.len());
}
