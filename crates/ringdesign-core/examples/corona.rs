// Corona — the combination ring. One signet carrying every lever this project
// has proved out, held to the same two-part sand verdict as a plain band.
//
// What it puts together, and why each earns its place:
//   * a lofted signet head, the factory construction, hollowed underneath so a
//     14 mm head does not weigh like one;
//   * a terraced plinth of step rails running the whole ring on the side
//     faces — the surfaces square to the pull, where relief cannot undercut;
//   * reeding windowed to the two shoulders, lean zero, because lean costs
//     draft in proportion to itself;
//   * a bead spine down the palm arc, on the crest line, the one place off a
//     side face where a bead row splits cleanly between cope and drag;
//   * two gypsy-set stones flanking the table, cast as stock and set at the
//     bench like every stone in this app;
//   * the table left blank, because a signet's table is a zero-draft wall and
//     what it is for is the engraver.
//
//   cargo run --release --example corona [out_dir]
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, Verdict};
use ringdesign_core::field::{
    BorderLayer, Remap, BorderProfile, FluteProfile, FlutesLayer, Layer, LayerEntry, MilgrainLayer,
    SeatPadLayer, SeatStyle, SignetOutline, Window, SIDE_FACE_MIN_DRAFT_DEG,
};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::TOP_DEG;
use ringdesign_core::render::{self, Part};
use ringdesign_core::{gems, library, stonemap, stones, ProfileStyle, RingDesign};

const GOLD: [f32; 3] = [0.86, 0.70, 0.42];

fn knob(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = AlphaLibrary::builtin();

    let width = knob("W", 13.0);
    let thickness = knob("T", 2.9);

    // --- the band and the head -------------------------------------------
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d.shank.apply_signet(width);
    d.shank.head.outline = SignetOutline::Cushion;
    d.shank.head.fit_length_to(width);
    d.shank.head.hollow_mm = knob("HOLLOW", 0.9);
    d.shank.head.rim_round_mm = knob("RIM", 0.7);
    // The loft's crest hands over from the table's plane solve to the shoulder
    // fall at the head's angular end, and that corner is not rounded the way
    // the prism's is — measured as a curvature step of 0.0353 per deg^2 at 33
    // deg off the top against 0.0004 either side, which is the ridge you see
    // down the flank. Growing the loft's body rows spreads the hand-over: 2 mm
    // gives 0.0353, 6 mm gives 0.0217.
    // The lofted crest is constant under the table and falls past it, so its
    // slope flips sign at the table's end and draws a ridge down the flank.
    // A millimetre of rim roll takes that corner from 0.0353 per deg^2 to
    // 0.0108 — under the prism's own 0.0125.
    d.shank.head.crest_round_mm = knob("CREST_ROUND", 1.0);
    d.shank.head.loft_frontal_mm = knob("LOFT_F", 2.0);
    d.shank.head.loft_lateral_mm = knob("LOFT_L", 2.0);

    let ctx = d.field_context();
    let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("a squared signet has side faces");

    // --- the plinth: stepped rails terracing both side faces --------------
    for (i, f) in [faces.low, faces.high].iter().flatten().enumerate() {
        for (k, (at, w, h)) in [(0.16, 0.95, 0.40), (0.44, 0.66, 0.26)].iter().enumerate() {
            let mut b = BorderLayer::default();
            b.v_mm = f.0 + (f.1 - f.0) * at;
            b.width_mm = *w;
            b.height_mm = *h;
            b.profile = BorderProfile::Step;
            b.mirror = false;
            // A rail sits at a fixed v, but the head widens the section, so a
            // rail that rides the side face on the shank wanders onto the
            // head's flank and leans there. Stop it where the head starts.
            let mut e = LayerEntry::new(format!("Plinth {}-{}", i + 1, k + 1), Layer::Border(b));
            e.window = Window::except(TOP_DEG, knob("HEADCLEAR", 96.0));
            e.window.fade_deg = 20.0;
            d.layers.layers.push(e);
        }
    }

    // --- the shoulders: reeding, gated off the table and off the palm -----
    let mut f = FlutesLayer::default();
    f.count = knob("RIBS", 44.0) as u32;
    f.profile = FluteProfile::Round;
    f.width_mm = 0.9;
    f.height_mm = 0.30;
    f.along = false;
    f.lean = 0.0;
    let mut e = LayerEntry::new("Shoulder reeding", Layer::Flutes(f));
    e.window = Window::except(TOP_DEG, knob("HEADCLEAR", 96.0));
    e.window.fade_deg = 18.0;
    // Stepped ribs rather than smooth swells: each tread is a plateau, so the
    // terrace buys texture without spending any draft.
    e.remap = Remap::Terrace { steps: 2, span_mm: 0.30, riser: 0.34 };
    d.layers.layers.push(e);

    // --- the palm: a bead spine on the parting line -----------------------
    let mut m = MilgrainLayer::default();
    m.beads_around = 96;
    m.bead_diameter_mm = 0.5;
    m.height_mm = 0.22;
    m.v_mm = ctx.crest_v_mm;
    m.mirror = false;
    let mut e = LayerEntry::new("Palm spine", Layer::Milgrain(m));
    e.window = Window::around(TOP_DEG + 180.0, 150.0);
    e.window.fade_deg = 24.0;
    d.layers.layers.push(e);

    // --- two stone markers flanking the table -----------------------------
    // Not a mound — a circle that says "carve the seat here" and nothing more,
    // a fifth of a millimetre proud of the shoulder.
    //
    // It is not quite a plane, and that is the whole lesson. A truly
    // flat-topped pad here fields 0.16% at -39 deg, and the app says why:
    // "flat-topped pad on +2 deg of base draft — its rim can lock". A pad's
    // rim leans by atan(height/skirt) against whatever draft the base has, and
    // on the crest line the base has almost none, so no skirt wide enough
    // exists. Carrying the crown instead makes every section a monotone drop
    // and the disc releases — and at 0.20 mm over 3.6 mm the rise is 1:18,
    // which the eye reads as flat.
    for (i, side) in [-1.0f64, 1.0].iter().enumerate() {
        let mut p = SeatPadLayer {
            theta_deg: TOP_DEG + side * knob("STONE_AT", 64.0),
            v_mm: ctx.crest_v_mm + knob("MARK_V", 0.0),
            style: SeatStyle::Boss,
            ..Default::default()
        };
        let gem = Gem::calibrated(GemCut::Round, knob("STONE", 2.8));
        p.fit_stone(gem);
        p.diameter_mm = gem.w_mm + knob("MARK_MARGIN", 0.8);
        p.height_mm = knob("MARK_H", 0.20);
        p.crown = knob("MARK_CROWN", 1.0);
        p.blend_mm = knob("MARK_SKIRT", 0.5);
        d.layers.layers.push(LayerEntry::new(format!("Stone marker {}", i + 1), Layer::SeatPad(p)));
    }

    if std::env::var("BARE").is_ok() {
        d.layers.layers.clear();
    }

    // --- judge, report, render -------------------------------------------
    d.name = "Corona".into();
    d.bake_all(&mut lib);
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 320, 160);
    println!(
        "Corona — {} — {:.4}% undercut, worst {:+.1} deg, thinnest wall {:.2} mm",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm
    );
    for n in field.notes.iter().filter(|n| !n.starts_with("Field-sampled: the surface")) {
        println!("  note: {n}");
    }
    for f in &ringdesign_core::dfm::findings_in(&d, &lib) {
        println!("  DFM: {}: {}", f.label, f.message);
    }
    let report = stones::report(&d, field.parting_z_mm);
    if let Some(s) = &report {
        println!("  stones: {} at {:.2} ct total", s.stone_count, s.total_carats);
        for seat in &s.seats {
            for w in &seat.warnings {
                println!("  stone warning: {w}");
            }
        }
    }

    let out = mesh::build(&d, &lib, BuildParams { theta_steps: 2200, profile_steps: 420, ..Default::default() });
    println!(
        "  {:.1} mm3, {:.2} g silver, watertight {}",
        out.report.volume_mm3,
        out.report.metals.first().map(|m| m.grams).unwrap_or(0.0),
        out.report.validation.watertight
    );
    assert_ne!(field.verdict, Verdict::NotCastable, "Corona must not ship uncastable");

    let stones_mesh = gems::preview_mesh(&d, &lib);
    let mut parts = vec![Part::metal(&out.mesh, GOLD)];
    if let Some(g) = &stones_mesh {
        parts.push(Part::stone(g));
    }
    render::write_png_parts(format!("{dir}/corona-hero.png"), &parts, 0.55, 1.05, 1500).unwrap();
    render::write_png_parts(format!("{dir}/corona-face.png"), &parts, 0.0, 1.52, 1200).unwrap();
    render::write_png_parts(format!("{dir}/corona-shoulder.png"), &parts, 1.15, 0.55, 1200).unwrap();
    // The casting, without the stones that get set into it afterwards: this is
    // what comes out of the sand, marker circles and all.
    render::write_png_parts(format!("{dir}/corona-ascast.png"), &parts[..1], 0.75, 0.80, 1300).unwrap();
    let sheet = ringdesign_core::spec::html(&d, &out.report, &field, report.as_ref(), &ringdesign_core::dfm::findings_in(&d, &lib), "corona");
    std::fs::write(format!("{dir}/corona-sheet.html"), sheet).unwrap();
    if report.is_some() {
        stonemap::write_stone_map_svg(format!("{dir}/corona-stones.svg"), &d, report.as_ref()).unwrap();
    }
    let designs = library::default_design_dir().join("corona");
    std::fs::create_dir_all(&designs).unwrap();
    library::save_design(designs.join("corona.ring.json"), &d).unwrap();
}
