// What we have: a collection built on the pieces that landed in one day —
// the bypass shank, tilted runs, the bezel that stands on its stone, the
// hollowed lofted signet, the measured DFM and the stone map. Every ring
// is held to the field verdict, rendered with its stones, saved so it opens
// in the app, and given its casting sheet and setter's map.
//
//   cargo run --release --example collection3 [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastProcess, Verdict};
use ringdesign_core::field::{
    Layer, LayerEntry, MilgrainLayer, SeatPadLayer, SeatRunLayer, SeatStyle, SideFacePick,
    SignetOutline, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ShankKind, TOP_DEG};
use ringdesign_core::render::{self, Part};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{gems, library, stonemap, stones, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

/// The wider side face's `v` run, and whether it is the low (-Z) face.
fn wider_face(d: &RingDesign) -> ((f64, f64), bool) {
    let ctx = d.field_context();
    let run = ctx.side_faces_std().and_then(|f| f.wider()).expect("a squared band has side faces");
    (run, 0.5 * (run.0 + run.1) < ctx.crest_v_mm)
}

/// Field-check, render with stones, sheet, map, save. Refuses to ship an
/// uncastable ring.
fn finish(dir: &str, slug: &str, blurb: &str, tint: [f32; 3], pitch: f64, gif: bool, mut d: RingDesign, lib: &mut AlphaLibrary) {
    d.name = slug.split_once('-').map(|(_, r)| r.replace('-', " ")).unwrap_or_else(|| slug.into());
    d.bake_all(lib);
    let field = castability::attributed_field_report(&d, lib, &d.draft, 256, 128);
    println!("{slug:<26} {blurb}");
    println!(
        "{:<26} {} under {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        d.draft.process.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in &field.notes {
        println!("{:<26}   note: {n}", "");
    }
    let dfm = ringdesign_core::dfm::findings_in(&d, lib);
    for f in &dfm {
        println!("{:<26}   dfm: {}: {}", "", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("{:<26}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        if let Some(p) = &s.closest {
            println!("{:<26}   closest pair: {} / {} — {:.2} mm at the girdle, {:.2} mm at the culet", "", p.a, p.b, p.gap_mm, p.gap_deep_mm);
        }
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<26}   stone warning: {w}", "");
                }
            }
        }
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(&d, lib, BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() });
    assert!(out.report.validation.watertight, "{slug} not watertight");
    for m in out.report.metals.iter().take(2) {
        println!("{:<26}   {}: {:.2} g", "", m.metal, m.grams);
    }
    let stones_mesh = gems::preview_mesh(&d, lib);
    let mut parts = vec![Part::metal(&out.mesh, tint)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    render::write_png_parts(format!("{dir}/{slug}-hero.png"), &parts, 0.55, pitch, 900).unwrap();
    if gif {
        render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 36, 480, tint).unwrap();
    }
    let sheet = ringdesign_core::spec::html(&d, &out.report, &field, report.as_ref(), &dfm, "collection3");
    std::fs::write(format!("{dir}/{slug}-sheet.html"), sheet).unwrap();
    if report.is_some() {
        stonemap::write_stone_map_svg(format!("{dir}/{slug}-stones.svg"), &d, report.as_ref()).unwrap();
    }
    let designs = library::default_design_dir().join("collection3");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- 01. Bypass solitaire: the two arms pass each other under a stone. --
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    // 5.0 wide: at 4.5 the crossing left the mound's skirt a wedge against
    // the band edge that the wall sweep read as 0.67 mm.
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.3;
    d.shank.kind = ShankKind::Bypass;
    d.shank.amount = 1.0;
    let ctx = d.field_context();
    // The crossing is 1.45 band widths wide; a 3.5 mm round's mound with its
    // skirt just fits inside it, a 4 mm one spilled 0.06 mm past the edge.
    // A 3.5 mm stone's foot still grazed the edge and the verdict read the
    // feather it left as a 0.68 mm wall; 3.2 keeps a third of a millimetre.
    // The skirt is the seat's finest feature for the DFM, read at the chart's
    // 0.85 metal scale: 0.45 measures 0.38 against the 0.35 floor.
    let mut seat = SeatPadLayer { theta_deg: TOP_DEG, v_mm: ctx.crest_v_mm, style: SeatStyle::GypsyMound, height_mm: 0.7, crown: 1.0, blend_mm: 0.45, ..Default::default() };
    seat.fit_stone(Gem::calibrated(GemCut::Round, 3.2));
    d.layers.layers.push(LayerEntry::new("Centre", Layer::SeatPad(seat)));
    finish(&dir, "01-bypass-solitaire", "a 3.2 mm round in a gypsy mound where the two arms cross", YELLOW, 1.3, true, d, &mut lib);

    // --- 02. Diagonal princess band: a tilted run on the side face, milgrain on the crest.
    // A gypsy seat carries 1.8 mm of stock round its stone, and a turned
    // square reaches 1.19x further: 2.5 mm princesses overhung a 5 mm band's
    // face by a millimetre. Bosses carry 1.2 mm of stock; 1.8 mm stones on
    // a 5.5 mm band leave a quarter millimetre of face either side.
    let mut d = squared(7.0, 5.5);
    let ((lo, hi), low_face) = wider_face(&d);
    let ctx = d.field_context();
    let mut run = SeatRunLayer { gem: Gem::calibrated(GemCut::Princess, 1.8), bridge_mm: 0.45, ..Default::default() };
    run.seat.style = SeatStyle::Boss;
    run.seat.crown = 0.3;
    run.seat.blend_mm = 0.3;
    run.seat.height_mm = 0.5;
    run.seat.v_mm = 0.5 * (lo + hi);
    run.tilt_deg = 45.0;
    run.solve_spacing(&ctx);
    let mut e = LayerEntry::new("Princess row", Layer::SeatRun(run));
    e.window = Window::around(TOP_DEG, 200.0);
    d.layers.layers.push(e);
    d.layers.layers.push(LayerEntry::new("Milgrain", Layer::Milgrain(MilgrainLayer { v_mm: ctx.crest_v_mm, bead_diameter_mm: 0.5, beads_around: 96, height_mm: 0.2, ..Default::default() })));
    // Pitch 1.12 tilts the +Z face to the viewer; ornament on the -Z face needs the mirrored attitude.
    let pitch = if low_face { std::f64::consts::PI - 1.12 } else { 1.12 };
    finish(&dir, "02-diagonal-princess", "1.8 mm princess cuts set on the diagonal along one face, a bead line on the crest", PLATINUM, pitch, false, d, &mut lib);

    // --- 03. Hollow clover signet: a lofted head on a drawn plan, scooped from the finger.
    let mut d = squared(12.0, 1.8);
    d.shank.apply_signet(12.0);
    let outline = library::list_outlines().into_iter().find(|o| o.name == "CG Clover").map(|o| d.shank.adopt_outline(o)).unwrap_or(SignetOutline::Hexagon);
    d.shank.head.dome = d.shank.suggest_dome(outline);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(12.0);
    d.shank.head.table_dome_mm = 1.0;
    let solid = mesh::build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }).report.volume_mm3;
    d.shank.head.hollow_mm = 0.6;
    let hollow = mesh::build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }).report.volume_mm3;
    println!("{:<26} hollow saves {:.1} mm³ of {:.1} ({:.0}%)", "03-hollow-clover-signet", solid - hollow, solid, (solid - hollow) / solid * 100.0);
    finish(&dir, "03-hollow-clover-signet", "a lofted clover head with a smooth table, 0.6 mm hollowed under the face", YELLOW, 1.12, true, d, &mut lib);

    // --- 04. Cabochon bezel band: the collar stands on its stone; a trellis on the far side.
    // A bezel is a side-face feature and its collar is the stone plus two
    // walls, so the face has to be wider than the stone: a 5 mm cab on a
    // 4.5 mm band overhung its face by 1.5 mm and leaned to -68 deg. A cigar
    // band 7.5 mm thick gives a 4 mm oval cab a 6.4 mm face.
    let mut d = squared(6.0, 7.5);
    let ((lo, hi), low_face) = wider_face(&d);
    let mut bezel = SeatPadLayer { theta_deg: TOP_DEG, v_mm: 0.5 * (lo + hi), style: SeatStyle::Bezel, recess_mm: 0.5, bezel_wall_mm: 0.6, blend_mm: 0.8, ..Default::default() };
    bezel.fit_stone(Gem::cabochon(GemCut::Oval, 4.0));
    d.layers.layers.push(LayerEntry::new("Cabochon", Layer::SeatPad(bezel)));
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for("Trellis", &ctx);
    t.height_mm = 0.3;
    // Trellis carries four wires per tile: fourteen repeats measured 0.27 mm
    // strokes against the 0.35 mm floor, eight read 0.47.
    t.repeats_around = 8;
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG));
    let mut e = LayerEntry::new("Trellis", Layer::Tiling(t));
    e.window = Window::except(TOP_DEG, 70.0);
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    let pitch = if low_face { std::f64::consts::PI - 1.12 } else { 1.12 };
    finish(&dir, "04-cabochon-bezel", "a 4 mm oval cab in a bezel whose collar height comes from the stone's dome; trellis round the rest of the cigar band", ROSE, pitch, false, d, &mut lib);

    // --- 05. Graded prong eternity, for lost wax: the stock sand refuses. ---
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.5;
    d.draft.process = CastProcess::LostWax;
    d.draft.min_section_mm = 0.5;
    d.draft.min_detail_mm = 0.15;
    let ctx = d.field_context();
    let mut run = SeatRunLayer { gem: Gem::calibrated(GemCut::Round, 2.2), bridge_mm: 0.4, taper: 0.5, taper_theta_deg: TOP_DEG, shared_prong_mm: 0.25, ..Default::default() };
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    d.layers.layers.push(LayerEntry::new("Graded row", Layer::SeatRun(run)));
    finish(&dir, "05-graded-prong-eternity", "a graduated row with shared prongs, judged for lost wax", WHITE, 1.12, false, d, &mut lib);
}
