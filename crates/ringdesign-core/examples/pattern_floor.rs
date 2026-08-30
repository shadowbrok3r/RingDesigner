// Which builtin patterns the sand can actually hold, and on what band.
//
// A side face's usable width is `thickness - crown`, so the question "can this
// mask be cast in sand" is really "how thick must the band be". Prints the
// minimum cell height each builtin needs against the sand's detail floor, and
// the repeats it then wants on a band that clears it.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::dfm::{self, FloorFit};
use ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{ProfileStyle, RingDesign};

fn band(thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 6.0;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

fn main() {
    let lib = AlphaLibrary::builtin();
    let floor = RingDesign::default().draft.min_detail_mm;
    let mut names: Vec<String> = lib.names().into_iter().map(|s| s.to_string()).collect();
    names.sort();
    println!("sand detail floor {floor:.2} mm — minimum side face a builtin mask needs\n");
    println!("{:<16} {:>10}  {:>9}  {}", "pattern", "face mm", "band mm", "repeats on a band that fits");
    let mut rows: Vec<(f64, String)> = Vec::new();
    for n in &names {
        let d = band(2.6);
        let ctx = d.field_context();
        let mut t = TilingLayer::default_for(n, &ctx);
        t.height_mm = 0.3;
        t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
        let need = match dfm::fit_to_floor(&mut t, &lib, &ctx, floor) {
            FloorFit::Repeats(_) => 0.0,
            FloorFit::NeedsTallerCell { min_cell_h_mm } => min_cell_h_mm,
            FloorFit::Unmeasurable => f64::NAN,
        };
        if need.is_nan() {
            continue;
        }
        // Thickness that yields a face of `need`: crown eats a share of it.
        let thick = if need <= 0.0 { 2.6 } else { (need * 1.4 + 1.0).min(14.0) };
        let d2 = band(thick);
        let ctx2 = d2.field_context();
        let mut t2 = TilingLayer::default_for(n, &ctx2);
        t2.height_mm = 0.3;
        t2.fit_to_side_faces(&ctx2, SIDE_FACE_MIN_DRAFT_DEG);
        let got = dfm::fit_to_floor(&mut t2, &lib, &ctx2, floor);
        let reps = match got {
            FloorFit::Repeats(n) => format!("{n}"),
            FloorFit::NeedsTallerCell { min_cell_h_mm } => format!("still short ({min_cell_h_mm:.1} mm)"),
            FloorFit::Unmeasurable => "—".into(),
        };
        rows.push((need, format!("{:<16} {:>10.2}  {:>9.1}  {}", n, need, thick, reps)));
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for (_, line) in &rows {
        println!("{line}");
    }
    println!("\n(face mm 0.00 = already clears on a 2.6 mm band)");
}
