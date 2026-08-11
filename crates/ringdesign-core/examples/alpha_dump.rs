// Dumps builtin alphas as PNGs for inspection.
use ringdesign_core::alpha::AlphaLibrary;

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let lib = AlphaLibrary::builtin();
    for name in lib.names() { let name = name.as_str();
        let a = lib.get(name).unwrap();
        let mut img = vec![0u8; a.width * a.height];
        for (i, &v) in a.data.iter().enumerate() {
            img[i] = (v.clamp(0.0, 1.0) * 255.0) as u8;
        }
        let slug: String = name.chars().map(|c| if c == ' ' { '_' } else { c.to_ascii_lowercase() }).collect();
        image::save_buffer(format!("{dir}/alpha_{slug}.png"), &img, a.width as u32, a.height as u32, image::ColorType::L8).unwrap();
        println!("{name}: {}x{}", a.width, a.height);
    }
}
