// Contact sheet of the scale alphas, each shown 2x2 tiled with its seam error.
use ringdesign_core::alpha::AlphaLibrary;

const CELL: usize = 340;
const COLS: usize = 4;

fn main() {
    let out = std::env::args().nth(1).expect("output path");
    let mut lib = AlphaLibrary::builtin();
    for dir in ringdesign_core::library::alpha_dirs() {
        let _ = lib.load_dir(dir);
    }
    let pick: Vec<String> = std::env::args().skip(2).collect();
    let mut names: Vec<String> = if pick.is_empty() {
        lib.names().into_iter().filter(|n| n.starts_with("scale-")).collect()
    } else {
        pick
    };
    if std::env::args().nth(2).is_none() {
        names.sort();
    }

    let rows = names.len().div_ceil(COLS);
    let (w, h) = (COLS * CELL, rows * CELL);
    let mut sheet = vec![14u8; w * h];

    for (i, name) in names.iter().enumerate() {
        let Some(a) = lib.get(name) else { continue };
        let mirrored = a.mirror_tile(ringdesign_core::alpha::Axis::Both);
        let (su, sv) = a.seam_error();
        let (mu, mv) = mirrored.seam_error();
        println!(
            "{name:<10} {}x{}  seam u {su:.4} v {sv:.4} -> mirrored u {mu:.4} v {mv:.4}",
            a.width, a.height
        );
        let (cx, cy) = ((i % COLS) * CELL, (i / COLS) * CELL);
        // Two tiles across the cell so any seam shows.
        for y in 0..CELL {
            for x in 0..CELL {
                let u = (x as f64 / CELL as f64 * 2.0).fract();
                let v = (y as f64 / CELL as f64 * 2.0).fract();
                let s = a.sample(u, v).clamp(0.0, 1.0);
                sheet[(cy + y) * w + cx + x] = (s * 255.0) as u8;
            }
        }
    }
    image::save_buffer(&out, &sheet, w as u32, h as u32, image::ColorType::L8).unwrap();
    println!("\n{} scale alphas -> {out}", names.len());
}
