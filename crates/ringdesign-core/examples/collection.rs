// The CrossGems-powered collection: designs that lean on the assets and
// features harvested from the CrossGems reverse-engineering — shared-prong
// eternities, graduated runs, the factory profile library, the Pattern
// relief-motif alphas (as tilings and decals), cut-dome signets, the halo.
//
//   cargo run --release --example collection [out_dir]
//
// Unlike the showcase (builtin-only, portable), this reads the user asset
// libraries: `~/.local/share/ringdesigner/{alphas,profiles}`. Any ring whose
// asset is missing is skipped with a note, so it still runs on a bare machine
// — it just makes fewer pieces there. Every ring is held to the field verdict
// before it renders, and saved under <designs>/collection/.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, CastProcess, Verdict};
use ringdesign_core::field::{
    Decal, DecalLayer, Layer, LayerEntry, SeatRunLayer, SideFacePick, SignetOutline, VGate,
    SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::pave::{self, HaloSpec};
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
    flip: bool,
    hero_pitch: f64,
}
const HERO: Shots = Shots { face: false, gif: false, flip: false, hero_pitch: 1.12 };

fn face_of(low: bool) -> Shots {
    Shots { face: true, gif: false, flip: low, hero_pitch: 1.12 }
}

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

/// A tiling fit to both side faces of a squared band, at a chosen relief.
/// `repeat_scale` multiplies the square-cell count: below 1 makes bolder
/// cells (a coarse voronoi), above 1 finer.
fn side_pattern(d: &RingDesign, alpha: &str, height: f64, edge: f64, repeat_scale: f64) -> TilingLayer {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.height_mm = height;
    t.rows = 1;
    if !t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG) {
        panic!("{alpha}: no side face to carry it");
    }
    t.repeats_around = ((t.repeats_for_square_cells(&ctx) as f64 * repeat_scale).round() as u32).max(1);
    t.edge_mm = edge;
    t.mirror_v = true;
    t
}

/// Field-check, render, save. Refuses to ship an uncastable ring.
fn finish(dir: &str, slug: &str, blurb: &str, tint: [f32; 3], shots: Shots, mut d: RingDesign, lib: &mut AlphaLibrary) {
    d.name = slug.split_once('-').map(|(_, r)| r.replace('-', " ")).unwrap_or_else(|| slug.into());
    d.bake_all(lib);

    let field = castability::analyze_field(&d, lib, &d.draft, 256, 128);
    println!("{slug:<26} {blurb}");
    println!(
        "{:<26} {}: {:.3}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        "", field.verdict.label(), field.undercut_fraction() * 100.0, field.worst_draft_deg, field.thinnest_wall_mm,
    );
    for n in &field.notes {
        println!("{:<26}   note: {n}", "");
    }
    if let Some(s) = stones::report(&d, field.parting_z_mm) {
        println!("{:<26}   stones: {} stones, {:.2} ct", "", s.stone_count, s.total_carats);
        let mut seen = std::collections::BTreeSet::new();
        for seat in &s.seats {
            for w in &seat.warnings {
                if seen.insert(w.clone()) {
                    println!("{:<26}   stone: {w}", "");
                }
            }
        }
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "{slug} must not ship uncastable");

    let out = mesh::build(&d, lib, BuildParams { theta_steps: 1600, profile_steps: 360, ..Default::default() });
    assert!(out.report.validation.watertight, "{slug} not watertight");

    let pi = std::f64::consts::PI;
    let hero_pitch = if shots.flip { pi - shots.hero_pitch } else { shots.hero_pitch };
    render::write_png(format!("{dir}/{slug}-hero.png"), &out.mesh, 0.55, hero_pitch, 900, tint).unwrap();
    if shots.face {
        let (yaw, pitch) = if shots.flip { (pi, pi - 0.35) } else { (0.0, 0.35) };
        render::write_png(format!("{dir}/{slug}-face.png"), &out.mesh, yaw, pitch, 900, tint).unwrap();
    }
    if shots.gif {
        render::write_turntable_gif(format!("{dir}/{slug}.gif"), &out.mesh, 36, 480, tint).unwrap();
    }

    let designs = library::default_design_dir().join("collection");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join(format!("{slug}.ring.json")), &d).unwrap();
    println!();
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/ring-collection".into());
    std::fs::create_dir_all(&dir).unwrap();

    let mut lib = AlphaLibrary::builtin();
    for d in library::alpha_dirs() {
        if let Ok(n) = lib.load_dir(&d) {
            if n > 0 {
                println!("loaded {n} alphas from {}", d.display());
            }
        }
    }
    let has = |lib: &AlphaLibrary, name: &str| lib.get(name).is_some();
    let profiles = library::list_profiles();
    let profile = |name: &str| profiles.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone());
    println!("{} saved profiles available\n", profiles.len());

    // --- 01. Shared-prong graduated eternity (lost wax). ---------------------
    // The marquee. One post pair at each boundary between stones, cut for both
    // at once — the CrossGems Prongs_Row rule. Proud posts flank the column off
    // the parting plane, so this is a lost-wax piece: the verdict carries the
    // pull stats but does not gate, and the sheet prints the claw stock.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.5;
    d.draft.process = CastProcess::LostWax;
    let ctx = d.field_context();
    let mut run = SeatRunLayer { gem: Gem::calibrated(GemCut::Round, 2.5), bridge_mm: 0.5, ..Default::default() };
    run.seat.v_mm = ctx.crest_v_mm;
    run.taper = 0.3;
    run.solve_spacing(&ctx);
    run.shared_prong_mm = 0.35;
    d.layers.layers.push(LayerEntry::new("Shared-prong run", Layer::SeatRun(run)));
    finish(&dir, "01-shared-prong-eternity", "graduated round eternity, one claw per boundary (lost wax)",
        YELLOW, Shots { face: true, gif: true, flip: false, hero_pitch: 1.12 }, d, &mut lib);

    // --- 02. Graduated gypsy eternity (sand). --------------------------------
    // The castable cousin: gypsy mounds grade with their stones, and the whole
    // row releases in two-part sand — no proud posts, the bench beads the seats.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.5;
    let ctx = d.field_context();
    let mut run = SeatRunLayer { gem: Gem::calibrated(GemCut::Round, 2.1), bridge_mm: 0.5, ..Default::default() };
    run.seat.v_mm = ctx.crest_v_mm;
    run.taper = 0.45;
    run.solve_spacing(&ctx);
    d.layers.layers.push(LayerEntry::new("Graded run", Layer::SeatRun(run)));
    finish(&dir, "02-graduated-gypsy", "gypsy mounds grading toward the shoulders, sand-castable", PLATINUM, HERO, d, &mut lib);

    // --- 03. Factory profile band. -------------------------------------------
    // A cross-section decoded from the CrossGems master preset library, applied
    // as the band's own shape — the profile is the design.
    for name in ["CG Second Floor", "CG Half Round", "CG Pear", "CG Tapered"] {
        if let Some(p) = profile(name) {
            let mut d = RingDesign::default();
            d.profile.width_mm = 5.0;
            d.profile.thickness_mm = 3.0;
            d.profile.apply_shape(&p);
            let slug = format!("03-profile-{}", name.trim_start_matches("CG ").to_lowercase().replace(' ', "-"));
            finish(&dir, &slug, &format!("the {name} factory section, bare"), SILVER, HERO, d, &mut lib);
            break;
        }
    }

    // --- 04. Filigree band: a Pattern scroll tiled on both faces. ------------
    // A single relief motif from the Pattern_Element library, one per square
    // cell, on each side face — where relief along the pull cannot undercut.
    if has(&lib, "pattern-24") {
        let d = squared(6.0, 2.6);
        let low = wider_is_low(&d);
        let mut d = d;
        d.layers.layers.push(LayerEntry::new("Filigree", Layer::Tiling(side_pattern(&d, "pattern-24", 0.35, 0.3, 1.0))));
        finish(&dir, "04-filigree-band", "a Pattern scroll motif tiled on the side faces", YELLOW, face_of(low), d, &mut lib);
    } else {
        println!("04-filigree-band          skipped: pattern-24 not in the alpha library\n");
    }

    // --- 05. Woven band: the Pattern basket-weave. ---------------------------
    if has(&lib, "pattern-16") {
        let d = squared(6.5, 2.6);
        let low = wider_is_low(&d);
        let mut d = d;
        d.layers.layers.push(LayerEntry::new("Weave", Layer::Tiling(side_pattern(&d, "pattern-16", 0.32, 0.3, 1.0))));
        finish(&dir, "05-woven-band", "a Pattern weave tiled on the side faces", ROSE, face_of(low), d, &mut lib);
    } else {
        println!("05-woven-band             skipped: pattern-16 not in the alpha library\n");
    }

    // --- 06. Cut-dome diamond signet. ----------------------------------------
    // The face cut from a full crown, so the corners are smooth fillets, not
    // pinched creases — a diamond facet on a domed head.
    let mut d = squared(13.0, 2.2);
    d.shank.apply_signet(13.0);
    d.shank.head.outline = SignetOutline::Diamond;
    d.shank.head.fit_length_to(13.0);
    d.shank.head.dome = 1.0;
    finish(&dir, "06-diamond-signet", "a diamond facet cut from a domed head", WHITE, HERO, d, &mut lib);

    // --- 07. Pattern decal: one motif stamped on a wide face. ----------------
    // The same library used as a free-placed stamp rather than a tiling — a
    // single crest medallion on the wider side face.
    if has(&lib, "pattern-39") {
        let mut d = squared(7.5, 2.8);
        let ctx = d.field_context();
        let (lo, hi) = ctx.side_faces_std().and_then(|f| f.wider()).unwrap();
        let low = wider_is_low(&d);
        let mut decals = DecalLayer::default();
        decals.alpha = "pattern-39".into();
        decals.feather_mm = 0.25;
        decals.decals = vec![Decal {
            theta_deg: TOP_DEG,
            v_mm: 0.5 * (lo + hi),
            size_mm: (hi - lo) * 0.9,
            rotation_deg: 0.0,
            height_mm: 0.4,
            flip: !low,
        }];
        let mut e = LayerEntry::new("Medallion", Layer::Decals(decals));
        e.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
        d.layers.layers.push(e);
        finish(&dir, "07-pattern-medallion", "a single Pattern motif stamped on the wide face", YELLOW, face_of(low), d, &mut lib);
    } else {
        println!("07-pattern-medallion      skipped: pattern-39 not in the alpha library\n");
    }

    // --- 08. Halo (lost wax): the proud accent ring. -------------------------
    // The process-aware generator: in sand a halo casts as a clean gypsy plate
    // with the melee as bench-set markers (a proud accent ring is a two-flange
    // valley with the centre, ~1.4% undercut). Under lost wax the same spec
    // frees the classic proud ring of accent mounds — which is what reads as a
    // halo before the stones go in.
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 11.0;
    d.profile.thickness_mm = 2.4;
    d.draft.process = CastProcess::LostWax;
    let spec = HaloSpec { center: Gem::calibrated(GemCut::Round, 5.0), accent: Gem::calibrated(GemCut::Round, 1.2), ..Default::default() };
    if let Some((entry, count)) = pave::halo(&d, &spec) {
        d.layers.layers.push(entry);
        finish(&dir, "08-halo", &format!("centre stone ringed by {count} proud accents (lost wax)"), PLATINUM, HERO, d, &mut lib);
    } else {
        println!("08-halo                   skipped: halo did not fit this band\n");
    }

    // --- 09. Voronoi band: the new cellular generator. -----------------------
    // Auto_Voronoi read into the height field: a builtin procedural alpha,
    // cells raised as domes with recessed boundaries, on the side faces where
    // relief along the pull releases. Coarse cells (repeat_scale 0.6) so each
    // sits well above the sand's detail floor.
    let d = squared(6.0, 2.6);
    let low = wider_is_low(&d);
    let mut d = d;
    d.layers.layers.push(LayerEntry::new("Voronoi", Layer::Tiling(side_pattern(&d, "Voronoi", 0.3, 0.2, 0.6))));
    finish(&dir, "09-voronoi-band", "the new Voronoi generator, coarse cells on the side faces", ROSE, face_of(low), d, &mut lib);

    // --- 10. Trellis band: the new wire-lattice generator. -------------------
    // Wire_Pattern read into the height field: an open diagonal grille of
    // round wires, raised over recessed gaps, on the side faces.
    let d = squared(6.0, 2.6);
    let low = wider_is_low(&d);
    let mut d = d;
    d.layers.layers.push(LayerEntry::new("Trellis", Layer::Tiling(side_pattern(&d, "Trellis", 0.3, 0.25, 0.85))));
    finish(&dir, "10-trellis-band", "the new Trellis wire-lattice on the side faces", WHITE, face_of(low), d, &mut lib);

    // --- 11. Clover signet: an imported factory plan as the head. ------------
    // One of the 19 table plans decoded straight out of the CrossGems signet
    // presets (tools/outline_export.py), adopted into the design as a polar
    // table — the same rolling-ball fairing and containment guarantee as
    // every builtin outline. Skipped politely on a machine without the
    // outline library.
    match library::list_outlines().into_iter().find(|c| c.name == "CG Clover") {
        Some(clover) => {
            let mut d = squared(9.0, 2.6);
            d.shank.apply_signet(9.0);
            let o = d.shank.adopt_outline(clover);
            d.shank.head.outline = o;
            d.shank.head.length_mm =
                (9.0 * d.shank.outline_aspect(o)).clamp(2.0, 40.0);
            // Four deep lobes corrugate a prism's flank; on the cut dome the
            // body is one smooth lens and the clover reads in the arris.
            d.shank.head.dome = d.shank.suggest_dome(o);
            finish(
                &dir,
                "11-clover-signet",
                "a clover face imported from the factory presets",
                YELLOW,
                HERO,
                d,
                &mut lib,
            );
        }
        None => println!("11-clover-signet     skipped: no CG Clover in the outline library
"),
    }

    println!("renders in {dir}, designs in {}", library::default_design_dir().join("collection").display());
}
