// Client commissions: the sketched rings this application was built to make.
//
//   cargo run --release --example commissions [out_dir]
//
// Three half-wrap patterned bands (snake scale, rope, ornamental), a wide
// cross band carrying a gem column, a diamond-face signet and a four-stone
// hexagon. Every one is held to the field verdict, rendered, and saved with
// its alphas embedded under <designs>/commissions/, so the files open
// anywhere the app does.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::field::{
    Layer, LayerEntry, SeatPadLayer, SeatStyle, SignetOutline, VGate, Window,
    SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::TOP_DEG;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{library, render, stones, ProfileStyle, RingDesign};

const YELLOW: [f32; 3] = [0.86, 0.70, 0.42];
const ROSE: [f32; 3] = [0.84, 0.60, 0.49];
const SILVER: [f32; 3] = [0.79, 0.80, 0.81];
const WHITE: [f32; 3] = [0.83, 0.83, 0.80];
const PLATINUM: [f32; 3] = [0.75, 0.76, 0.78];

struct Shots {
    face: bool,
    gif: bool,
    /// View the -Z side face, where `wider()` may put single-face ornament.
    flip: bool,
}

const HERO: Shots = Shots { face: false, gif: false, flip: false };
const GIF: Shots = Shots { face: false, gif: true, flip: false };

fn wider_is_low(d: &RingDesign) -> bool {
    let ctx = d.field_context();
    ctx.side_faces_std()
        .and_then(|f| f.wider())
        .map(|(lo, hi)| 0.5 * (lo + hi) < ctx.crest_v_mm)
        .unwrap_or(false)
}

fn squared(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d
}

fn signet(outline: SignetOutline, width: f64, thickness: f64) -> RingDesign {
    let mut d = squared(width, thickness);
    d.shank.apply_signet(width);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d
}

fn side_tiling(d: &RingDesign, alpha: &str, height: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    if !t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG) {
        panic!("no side faces on a band meant to carry side relief");
    }
    t
}

/// The client's half-wrap: full strength over the top, fading out on the way
/// down — a hard end would raise a wall the mould has to clear.
fn half_wrap() -> Window {
    Window {
        enabled: true,
        theta_deg: TOP_DEG,
        span_deg: 170.0,
        fade_deg: 22.0,
        invert: false,
        v_gate: VGate::Off,
    }
}

/// A shallow gem spot on a signet table: a mound straddling the parting
/// plane, which is the one placement that releases. No bur dimple — a pit
/// locks even at the crest (its walls are the inverse of a dome's), so the
/// mound itself is the setter's mark.
fn table_spot(theta_deg: f64, v_mm: f64, gem_mm: f64) -> SeatPadLayer {
    let mut seat = SeatPadLayer {
        theta_deg,
        v_mm,
        height_mm: 0.28,
        crown: 1.0,
        blend_mm: 0.5,
        style: SeatStyle::GypsyMound,
        ..Default::default()
    };
    seat.fit_stone(Gem::calibrated(GemCut::Round, gem_mm));
    // A spot, not a full seat: the gypsy skirt would merge neighbouring
    // spots into one ridge at column pitch.
    seat.diameter_mm = gem_mm + 0.9;
    seat
}

fn finish(
    dir: &str,
    slug: &str,
    blurb: &str,
    tint: [f32; 3],
    shots: Shots,
    mut d: RingDesign,
    lib: &mut AlphaLibrary,
) {
    d.name = slug
        .split_once('-')
        .map(|(_, rest)| rest.replace('-', " "))
        .unwrap_or_else(|| slug.into());
    d.bake_all(lib);

    let field = castability::analyze_field(&d, lib, &d.draft, 256, 128);
    println!("{slug:<22} {blurb}");
    println!(
        "{:<22} {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm,
    );
    for f in ringdesign_core::dfm::findings(&d) {
        println!("{:<22}   dfm: {}: {}", "", f.label, f.message);
    }
    if let Some(s) = stones::report(&d, field.parting_z_mm) {
        println!("{:<22}   stones: {} stones, {:.2} ct total", "", s.stone_count, s.total_carats);
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

    let out = mesh::build(
        &d,
        lib,
        BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() },
    );
    assert!(out.report.validation.watertight, "{slug} not watertight");

    let pi = std::f64::consts::PI;
    let hero_pitch = if shots.flip { pi - 1.12 } else { 1.12 };
    render::write_png(format!("{dir}/{slug}-hero.png"), &out.mesh, 0.55, hero_pitch, 900, tint)
        .unwrap();
    if shots.face {
        let (face_yaw, face_pitch) = if shots.flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png(format!("{dir}/{slug}-face.png"), &out.mesh, face_yaw, face_pitch, 900, tint)
            .unwrap();
    }
    if shots.gif {
        render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 36, 480, tint).unwrap();
    }

    let designs = library::default_design_dir().join("commissions");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design_embedded(designs.join(format!("{slug}.ring.json")), &d, lib).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/ring-commissions".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();
    for d in library::alpha_dirs() {
        let _ = lib.load_dir(d);
    }

    // --- A. Scale crescent: snake scales halfway round, dot terminals. -------
    let mut d = squared(5.5, 3.0);
    let ctx = d.field_context();
    let mut t = side_tiling(&d, "scale-22", 0.32);
    t.contrast = 1.2;
    let mut e = LayerEntry::new("Snake scales", Layer::Tiling(t));
    e.window = half_wrap();
    d.layers.layers.push(e);
    // The sketch ends the arc in a round dot each side: a small flush mound
    // on the side face, which is castable ground for a proud stud.
    let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
    let low_face = wider_is_low(&d);
    for (label, side) in [("End dot A", -1.0), ("End dot B", 1.0)] {
        let seat = SeatPadLayer {
            theta_deg: TOP_DEG + side * 100.0,
            v_mm: 0.5 * (lo + hi),
            diameter_mm: 1.2,
            height_mm: 0.45,
            crown: 1.0,
            blend_mm: 0.35,
            style: SeatStyle::GypsyMound,
            ..Default::default()
        };
        d.layers.layers.push(LayerEntry::new(label, Layer::SeatPad(seat)));
    }
    finish(&dir, "a-scale-crescent", "snake scales halfway round, dot terminals", SILVER, Shots { face: true, gif: false, flip: low_face }, d, &mut lib);

    // --- B. Rope crescent: braided cord halfway round. -----------------------
    let mut d = squared(5.0, 2.5);
    let mut t = side_tiling(&d, "Braid", 0.28);
    // Three cords per tile: the auto square-cell count lands them at 0.6 mm
    // corduroy; twelve tiles keeps each cord near 2 mm, a rope that reads.
    t.repeats_around = 12;
    t.contrast = 1.1;
    let mut e = LayerEntry::new("Rope", Layer::Tiling(t));
    e.window = half_wrap();
    d.layers.layers.push(e);
    let low_face = wider_is_low(&d);
    finish(&dir, "b-rope-crescent", "braided cord halfway round", YELLOW, Shots { face: true, gif: false, flip: low_face }, d, &mut lib);

    // --- C. Ornament crescent: floral scrollwork halfway round. --------------
    let mut d = squared(6.0, 2.7);
    let mut t = side_tiling(&d, "ornament-a-07", 0.30);
    let mut e = LayerEntry::new("Ornament", Layer::Tiling(t));
    e.window = half_wrap();
    d.layers.layers.push(e);
    let low_face = wider_is_low(&d);
    finish(&dir, "c-ornament-crescent", "floral scrollwork halfway round", ROSE, Shots { face: true, gif: false, flip: low_face }, d, &mut lib);

    // --- D. Cross band: a plus-faced head on a wide band, gem column. --------
    // The cross is a head, not a relief: raised panels on a flat crown lean
    // by their own edge slope, while a head's flanks are drafted by the same
    // machinery as every signet. The gem column runs along the parting
    // plane, not across the band: a mound straddling the plane splits
    // between cope and drag, while the same mound 2.3 mm off it locked at
    // -9 deg over 1.3% — the sketch's column, turned to the axis the sand
    // can hold.
    let mut d = squared(8.0, 2.2);
    d.shank.apply_signet(8.0);
    d.shank.head.outline = SignetOutline::Cross;
    d.shank.head.fit_length_to(8.0);
    d.shank.head.rise_mm = 0.45;
    d.shank.head.dome = 1.0;
    let ctx = d.field_context();
    let du_deg = (2.6 / ctx.crest_radius_mm).to_degrees();
    for (label, su) in [("Spot A", -1.0), ("Spot centre", 0.0), ("Spot B", 1.0)] {
        d.layers.layers.push(LayerEntry::new(
            label,
            Layer::SeatPad(table_spot(TOP_DEG + su * du_deg, ctx.crest_v_mm, 1.5)),
        ));
    }
    finish(&dir, "d-cross-band", "plus-faced head on a wide band, three-gem column", PLATINUM, GIF, d, &mut lib);

    // --- E. Luxer: the diamond-face signet, cut from a dome. -----------------
    let mut d = signet(SignetOutline::Diamond, 13.0, 2.0);
    d.shank.head.dome = 1.0;
    finish(&dir, "e-luxer-diamond", "diamond face, blank for the engraver", YELLOW, GIF, d, &mut lib);

    // --- F. MSB: hexagon face, four gems spread evenly. ----------------------
    // Four spots in an even row along the crest line — off the parting
    // plane a table mound locks by its own slope, so the spread runs the
    // one axis the sand holds.
    let mut d = signet(SignetOutline::Hexagon, 14.0, 2.4);
    d.shank.head.dome = 1.0;
    let ctx = d.field_context();
    let pitch_deg = (2.4 / ctx.crest_radius_mm).to_degrees();
    for i in 0..4u32 {
        let su = i as f64 - 1.5;
        d.layers.layers.push(LayerEntry::new(
            format!("Spot {}", i + 1),
            Layer::SeatPad(table_spot(TOP_DEG + su * pitch_deg, ctx.crest_v_mm, 1.5)),
        ));
    }
    finish(&dir, "f-msb-hexagon", "hexagon face, four gems spread evenly", WHITE, HERO, d, &mut lib);

    // --- G. Halo: a centre stone in a bead-set melee ring. -------------------
    // The signature CrossGems cluster, cast the way a halo is actually made:
    // a clean domed plate carrying the centre seat, the melee ring riding it
    // as bench-set markers. A ring of proud accent mounds is the two-flange
    // valley (measured 1.4% at -33 deg); the plate is the stock the setter
    // beads the melee into.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 11.0;
    d.profile.thickness_mm = 2.4;
    let spec = ringdesign_core::pave::HaloSpec {
        center: Gem::calibrated(GemCut::Round, 4.5),
        accent: Gem::calibrated(GemCut::Round, 1.0),
        gap_mm: 0.3,
        bridge_mm: 0.25,
        ..Default::default()
    };
    let (entry, n) = ringdesign_core::pave::halo(&d, &spec).expect("halo fits an 11 mm band");
    println!("{:<22}   halo: {} accents round the centre", "", n);
    d.layers.layers.push(entry);
    finish(&dir, "g-halo-cluster", "centre stone in a bead-set melee halo", WHITE, GIF, d, &mut lib);

    println!(
        "renders in {dir}, designs in {}",
        library::default_design_dir().join("commissions").display()
    );
}
