// Import factory cross-sections from a manifest of crown envelopes.
//
//   cargo run --release --example import_profiles -- <profiles.json>
//
// Each entry names a single-crest crown: a 16-point drop from crest to
// edge, a crest bias, and crown-over-thickness. They become saved profiles
// (library::profile_dir) with Custom style and a drawn DropCurve, applied
// like any other saved section — the shape, never the size.
use ringdesign_core::profile::DropCurve;
use ringdesign_core::{library, BandProfile, ProfileStyle};

#[derive(serde::Deserialize)]
struct Entry {
    name: String,
    thickness_over_width: f64,
    crown_over_thickness: f64,
    crest_bias: f64,
    drop: Vec<[f64; 2]>,
}

fn main() {
    let path = std::env::args().nth(1).expect("manifest path");
    let entries: Vec<Entry> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut saved = 0;
    for e in &entries {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::Custom);
        p.width_mm = 4.0;
        p.thickness_mm = (4.0 * e.thickness_over_width).clamp(0.8, 6.0);
        p.crown_mm = (p.thickness_mm * e.crown_over_thickness.clamp(0.05, 1.0)).max(0.1);
        p.crest_bias = e.crest_bias.clamp(-1.0, 1.0);
        let n = e.drop.len().clamp(2, 16);
        let mut curve = DropCurve::from_superellipse(2.0, 2.0, n);
        for (i, q) in e.drop.iter().take(n).enumerate() {
            curve.set(i, q[0], q[1]);
        }
        p.drop_curve = curve;
        match library::save_profile(&e.name, &p) {
            Ok(_) => saved += 1,
            Err(err) => eprintln!("{}: {err}", e.name),
        }
    }
    println!("saved {saved} profiles into {}", library::profile_dir().display());
}
