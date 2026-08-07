// Dump every procedural alpha as a 2x2-tiled PNG so seams are visible.
use ringdesign_core::alpha::Procedural;

fn main() {
    let out = std::env::args().nth(1).expect("output dir");
    std::fs::create_dir_all(&out).unwrap();
    for p in Procedural::ALL {
        let a = p.generate(256);
        let (w, h) = (a.width, a.height);
        // Tile 2x2 to expose any seam at the wrap.
        let mut buf = vec![0u8; w * 2 * h * 2];
        for y in 0..h * 2 {
            for x in 0..w * 2 {
                let v = a.data[(y % h) * w + (x % w)];
                buf[y * w * 2 + x] = (v.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
        let path = format!("{out}/{}.png", p.label().replace(' ', "_"));
        image::save_buffer(&path, &buf, (w * 2) as u32, (h * 2) as u32, image::ColorType::L8).unwrap();
    }
    println!("wrote {} patterns", Procedural::ALL.len());
}
