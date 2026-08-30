// Regalia — the second combination ring, built on everything added since
// Corona. Where Corona used a builtin cushion, this one's plan is *drawn*:
// a heraldic shield rasterized into an alpha, traced to a closed boundary by
// `contour::trace`, and adopted as the head's outline. That path did not exist
// a day ago — core could smooth an imported plan but had no way to get a
// polyline out of a raster.
//
// Everything it puts together, and where each came from:
//   * a drawn plan through `contour::trace` -> `CustomOutline::from_points`;
//   * a lofted head with `crest_round_mm`, which rounds the crest's corner at
//     the table's theta-end — the ridge that read down Corona's flank;
//   * the head hollowed, so a 14 mm shield does not weigh like one;
//   * reeding on the shoulders at zero lean, stepped by the terrace remap,
//     because lean costs draft in proportion to itself (`SAND_MAX_LEAN`);
//   * a lattice carved into the palm's side faces, its repeat count solved by
//     `dfm::fit_to_floor` against the sand's own detail floor rather than
//     guessed — on a band deliberately thick enough to hold it;
//   * a bead spine on the parting line where the openwork ends;
//   * flat stone markers, crown carried so the rim cannot lock.
//
//   cargo run --release --example regalia [out_dir]
use ringdesign_core::alpha::{Alpha, AlphaLibrary};
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::dfm::{self, FloorFit};
use ringdesign_core::field::{
    CustomOutline, FluteProfile, FlutesLayer, Layer, LayerEntry, MilgrainLayer,
    OpenworkLayer, Remap, SeatPadLayer, SeatStyle, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::TOP_DEG;
use ringdesign_core::render::{self, Part};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{gems, library, stonemap, stones, ProfileStyle, RingDesign};

const GOLD: [f32; 3] = [0.86, 0.70, 0.42];

fn knob(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// A heraldic shield, rasterized. Flat across the top with rounded upper
/// corners, sides falling away, and a soft point at the base — drawn as ink so
/// the contour tracer has something to walk, exactly as a scanned sketch would
/// arrive.
fn shield_raster(n: usize) -> Alpha {
    let mut data = vec![0.0f32; n * n];
    let s = 3; // supersample, so the traced edge is not a staircase
    for j in 0..n {
        for i in 0..n {
            let mut hits = 0;
            for sj in 0..s {
                for si in 0..s {
                    let x = ((i * s + si) as f64 + 0.5) / (n * s) as f64 * 2.0 - 1.0;
                    let y = ((j * s + sj) as f64 + 0.5) / (n * s) as f64 * 2.0 - 1.0;
                    // t runs 0 at the top edge to 1 at the base point.
                    let t = (1.0 - y) * 0.5;
                    if !(0.0..=1.0).contains(&t) {
                        continue;
                    }
                    let w = if t < 0.18 {
                        // The two upper corners, rounded off a full-width top.
                        let u = (0.18 - t) / 0.18;
                        (1.0 - 0.55 * u * u).max(0.0).sqrt()
                    } else {
                        // The sides falling away to a soft point at the base.
                        let u = (t - 0.18) / 0.82;
                        (1.0 - u * u * u).max(0.0).powf(0.62)
                    };
                    if x.abs() <= w * 0.92 {
                        hits += 1;
                    }
                }
            }
            data[j * n + i] = hits as f32 / (s * s) as f32;
        }
    }
    Alpha::new("Shield sketch", n, n, data)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    // --- the drawn plan ---------------------------------------------------
    let sketch = shield_raster(512);
    let pts = ringdesign_core::contour::trace(&sketch, 0.5).expect("the sketch must trace");
    let plan = CustomOutline::from_points("Shield", &pts).expect("a real boundary");
    println!("traced plan: {} boundary points, aspect {:.3}, fair_r {:.2}", pts.len(), plan.aspect, plan.fair_r);

    // --- the band and the head --------------------------------------------
    let width = knob("W", 14.0);
    let thickness = knob("T", 4.0);
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d.shank.apply_signet(width);
    let outline = d.shank.adopt_outline(plan);
    d.shank.head.outline = outline;
    d.shank.head.fit_length_to(width);
    d.shank.head.hollow_mm = knob("HOLLOW", 0.9);
    d.shank.head.rim_round_mm = knob("RIM", 0.7);
    d.shank.head.crest_round_mm = knob("CREST_ROUND", 1.0);

    let ctx = d.field_context();
    let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("a squared signet has side faces");
    let head_clear = knob("HEADCLEAR", 100.0);

    // --- the shoulders: terraced reeding, gated off the table and the palm --
    let mut f = FlutesLayer::default();
    f.count = knob("RIBS", 48.0) as u32;
    f.profile = FluteProfile::Round;
    f.width_mm = 1.0;
    f.height_mm = 0.34;
    f.along = false;
    f.lean = 0.0; // SAND_MAX_LEAN: a leaned flute leans its walls with it
    // Two arcs, one per shoulder, so the reeding never reaches the table and
    // never reaches the carved palm. Three zones, each doing one thing.
    for (i, side) in [-1.0f64, 1.0].iter().enumerate() {
        let mut e = LayerEntry::new(format!("Shoulder reeding {}", i + 1), Layer::Flutes(f));
        e.window = Window::around(TOP_DEG + side * knob("SHOULDER_AT", 80.0), knob("SHOULDER_ARC", 54.0));
        e.window.fade_deg = 16.0;
        e.remap = Remap::Terrace { steps: 2, span_mm: 0.34, riser: 0.32 };
        d.layers.layers.push(e);
    }
    let _ = head_clear;

    // --- the palm: a lattice carved into the side faces, count solved -------
    let mut t = TilingLayer::default_for("Lattice", &ctx);
    t.height_mm = 0.32;
    t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG);
    match dfm::fit_to_floor(&mut t, &lib, &ctx, d.draft.min_detail_mm) {
        FloorFit::Repeats(n) => {
            println!("solver: Lattice takes {n} repeats on this band");
            let ow = OpenworkLayer { tiling: t, depth_mm: knob("CARVE", 0.8), keep_mm: 1.1 };
            let mut e = LayerEntry::new("Carved lattice", Layer::Openwork(ow));
            e.window = Window::around(TOP_DEG + 180.0, knob("PALM", 96.0));
            e.window.fade_deg = 26.0;
            d.layers.layers.push(e);
        }
        other => println!("solver refused the lattice: {other:?} — band too thin"),
    }

    // --- the parting line: beads where the carving stops --------------------
    let mut m = MilgrainLayer::default();
    m.beads_around = 108;
    m.bead_diameter_mm = 0.5;
    m.height_mm = 0.22;
    m.v_mm = ctx.crest_v_mm;
    m.mirror = false;
    let mut e = LayerEntry::new("Palm spine", Layer::Milgrain(m));
    e.window = Window::around(TOP_DEG + 180.0, knob("PALM", 96.0));
    e.window.fade_deg = 26.0;
    d.layers.layers.push(e);

    // --- flat stone markers flanking the table ------------------------------
    // At 90 deg, not tucked against the head: a pad inside the head's swell
    // arc is sized against the reference section and lands on a modulated one,
    // and its rim leans there — 0.148% at -23.6 deg at 66 deg off the top
    // against 0.027% out here, which the field calls sampling noise. The same
    // trap as a rail crossing the head.
    for (i, side) in [-1.0f64, 1.0].iter().enumerate() {
        let mut p = SeatPadLayer {
            theta_deg: TOP_DEG + side * knob("STONE_AT", 90.0),
            v_mm: ctx.crest_v_mm + knob("MARK_V", 0.0),
            style: SeatStyle::Boss,
            ..Default::default()
        };
        let gem = Gem::calibrated(GemCut::Round, knob("STONE", 3.0));
        p.fit_stone(gem);
        p.diameter_mm = gem.w_mm + 0.8;
        p.height_mm = knob("MARK_H", 0.20);
        p.crown = 1.0; // a flat top's rim locks on the crest's 2 deg of draft
        p.blend_mm = knob("MARK_SKIRT", 0.5);
        d.layers.layers.push(LayerEntry::new(format!("Stone marker {}", i + 1), Layer::SeatPad(p)));
    }
    let _ = faces;

    // --- judge, report, render ---------------------------------------------
    d.name = "Regalia".into();
    d.bake_all(&mut lib);
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 320, 160);
    println!(
        "Regalia — {} — {:.4}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in field.notes.iter().filter(|n| !n.starts_with("Field-sampled: the surface")) {
        println!("  note: {n}");
    }
    for f in &dfm::findings_in(&d, &lib) {
        println!("  DFM: {}: {}", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("  stones: {} at {:.2} ct", s.stone_count, s.total_carats);
    }
    assert_ne!(field.verdict, Verdict::NotCastable, "Regalia must not ship uncastable");

    let out = mesh::build(&d, &lib, BuildParams { theta_steps: 2400, profile_steps: 440, ..Default::default() });
    println!(
        "  {:.1} mm3, {:.2} g silver, watertight {}",
        out.report.volume_mm3,
        out.report.metals.first().map(|m| m.grams).unwrap_or(0.0),
        out.report.validation.watertight
    );

    let stones_mesh = gems::preview_mesh(&d, &lib);
    let mut parts = vec![Part::metal(&out.mesh, GOLD)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    render::write_png_parts(format!("{dir}/regalia-hero.png"), &parts, 0.55, 1.05, 1500).unwrap();
    render::write_png_parts(format!("{dir}/regalia-face.png"), &parts, 0.0, 1.52, 1200).unwrap();
    render::write_png_parts(format!("{dir}/regalia-palm.png"), &parts[..1], 0.55, -1.05, 1300).unwrap();
    let sheet = ringdesign_core::spec::html(&d, &out.report, &field, report.as_ref(), &dfm::findings_in(&d, &lib), "regalia");
    std::fs::write(format!("{dir}/regalia-sheet.html"), sheet).unwrap();
    if report.is_some() {
        stonemap::write_stone_map_svg(format!("{dir}/regalia-stones.svg"), &d, report.as_ref()).unwrap();
    }
    let designs = library::default_design_dir().join("regalia");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join("regalia.ring.json"), &d).unwrap();
}
