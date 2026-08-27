// Three sketched rings — a heart signet, a half-eternity, a princess
// solitaire — built here with the same numbers handed to CrossGems through
// its Grasshopper components, so the two engines can be laid side by side
// on the same brief. Each is field-checked, rendered with its stones,
// exported as OBJ for the mesh comparison, and saved so it opens in the app.
//
//   cargo run --release --example sketches [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastProcess, Verdict};
use ringdesign_core::field::{Layer, LayerEntry, SeatPadLayer, SeatRunLayer, SeatStyle, SignetOutline, Window};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKey, ShankKind, SIGNET_MIN_SHANK_FRAC, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::{gems, library, stonemap, stones, ProfileStyle, RingDesign, RingSize};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];

/// Field-check, render with stones, sheet, map, OBJ, save.
fn finish(dir: &str, slug: &str, blurb: &str, tint: [f32; 3], mut d: RingDesign, lib: &mut AlphaLibrary) {
    d.name = slug.split_once('-').map(|(_, r)| r.replace('-', " ")).unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!("{slug:<22} {blurb}");
    println!(
        "{:<22} {} under {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        d.draft.process.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("{:<22}   note: {n}", "");
    }
    let dfm = ringdesign_core::dfm::findings_in(&d, lib);
    for f in &dfm {
        println!("{:<22}   dfm: {}: {}", "", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("{:<22}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        if let Some(p) = &s.closest {
            println!("{:<22}   closest pair: {} / {} — {:.2} mm at the girdle, {:.2} mm at the culet", "", p.a, p.b, p.gap_mm, p.gap_deep_mm);
        }
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<22}   stone warning: {w}", "");
                }
            }
        }
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(&d, lib, BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() });
    assert!(out.report.validation.watertight, "{slug} not watertight");
    println!("{:<22}   bore {:.2} mm, volume {:.1} mm3", "", out.report.inner_diameter_mm, out.report.volume_mm3);
    for m in out.report.metals.iter().take(2) {
        println!("{:<22}   {}: {:.2} g", "", m.metal, m.grams);
    }
    let stones_mesh = gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, 1.05, 900).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-top.png"), &parts, 0.0, 1.55, 700).unwrap();
    render::write_png_parts(format!("{dir}/{slug}-side.png"), &parts, 1.5708, 0.0, 700).unwrap();
    // The comparison mesh: dense enough to measure against, light enough to
    // measure with.
    let probe = mesh::build(&d, lib, BuildParams { theta_steps: 640, profile_steps: 160, ..Default::default() });
    ringdesign_core::stl::write_obj(format!("{dir}/{slug}.obj"), &probe.mesh, &d.name).unwrap();
    let sheet = ringdesign_core::spec::html(&d, &out.report, &field, report.as_ref(), &dfm, "sketches");
    std::fs::write(format!("{dir}/{slug}-sheet.html"), sheet).unwrap();
    if report.is_some() {
        stonemap::write_stone_map_svg(format!("{dir}/{slug}-stones.svg"), &d, report.as_ref()).unwrap();
    }
    let designs = library::default_design_dir().join("sketches");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- A. Heart signet: the same numbers as CrossGems' Signet Ring ------
    // (Table Length 12 across the finger, Width 11.4 along the ring, Table
    // Height 3.0 over the bore, sides 4.5 x 1.5, Frontal/Lateral 2), through
    // the same mapping cg_signet.rs uses for a decoded preset.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.width_mm = 12.0;
    d.profile.thickness_mm = 1.75;
    d.profile.crown_mm = 1.49;
    d.profile.edge_round_mm = 0.05;
    d.shank.apply_signet(12.0);
    let shank_frac = (4.5_f64 / 12.0).clamp(SIGNET_MIN_SHANK_FRAC, 1.0);
    d.shank.amount = ((1.0 - shank_frac) / (1.0 - SIGNET_MIN_SHANK_FRAC)).clamp(0.0, 1.0);
    d.shank.head.outline = SignetOutline::Heart;
    d.shank.head.length_mm = 11.4;
    d.shank.head.rise_mm = 3.0 - 1.75;
    d.shank.head.rim_round_mm = 0.3;
    d.shank.head.table_dome_mm = 0.0;
    d.shank.head.loft_frontal_mm = 2.0;
    d.shank.head.loft_lateral_mm = 2.0;
    finish(&dir, "A-heart-signet", "lofted heart signet, 12 x 11.4 plate, flat table", YELLOW, d, &mut lib);

    // --- B. Half-eternity: 2 mm rounds over 200 degrees --------------------
    // CrossGems packs them 0.2 mm apart for shared prongs or a channel; in
    // sand the seats are bosses drilled at the bench, the bridge is the
    // 0.3 mm fill floor, and the count follows from that.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 3.8;
    d.profile.thickness_mm = 2.2;
    let ctx = d.field_context();
    let seat = SeatPadLayer { v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.45, crown: 0.9, blend_mm: 0.4, ..Default::default() };
    let mut run = SeatRunLayer { seat, gem: Gem::calibrated(GemCut::Round, 2.0), bridge_mm: 0.3, ..Default::default() };
    run.solve_spacing(&ctx);
    let mut entry = LayerEntry::new("Half eternity", Layer::SeatRun(run));
    entry.window = Window::around(TOP_DEG, 200.0);
    d.layers.layers.push(entry);
    finish(&dir, "B-half-eternity", "2 mm rounds over 200 degrees on a 3.8 x 2.2 band", WHITE, d, &mut lib);

    // --- C. Princess solitaire: a 4 mm stone on a 3 x 2 band ---------------
    // A prong head stands above the band and is lost-wax stock: here the
    // band widens to carry a boss with four prong bumps, and the sheet is
    // judged for lost wax, as CrossGems' head is.
    let mut d = RingDesign::default();
    d.size = RingSize::new(7.0);
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 3.0;
    d.profile.thickness_mm = 2.0;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = vec![
        ShankKey { theta_deg: TOP_DEG, width_scale: 2.35, thickness_scale: 1.3, crown_scale: 0.6 },
        ShankKey { theta_deg: TOP_DEG + 55.0, width_scale: 1.35, thickness_scale: 1.1, crown_scale: 1.0 },
        ShankKey { theta_deg: TOP_DEG - 55.0, width_scale: 1.35, thickness_scale: 1.1, crown_scale: 1.0 },
        ShankKey { theta_deg: TOP_DEG + 180.0, width_scale: 1.0, thickness_scale: 1.0, crown_scale: 1.0 },
    ];
    d.draft.process = CastProcess::LostWax;
    let ctx = d.field_context();
    let mut seat = SeatPadLayer { theta_deg: TOP_DEG, v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.9, crown: 1.0, blend_mm: 0.45, prongs: 4, prong_mm: 0.55, ..Default::default() };
    seat.fit_stone(Gem::calibrated(GemCut::Princess, 4.0));
    d.layers.layers.push(LayerEntry::new("Princess", Layer::SeatPad(seat)));
    finish(&dir, "C-princess-solitaire", "4 mm princess in a four-prong boss on a widened 3 x 2 band", WHITE, d, &mut lib);
}
