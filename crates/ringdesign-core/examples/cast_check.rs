// Does the castability analyser actually distinguish castable from not?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{Layer, LayerEntry, SeatPadLayer};
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::{ProfileStyle, RingDesign, mesh};

fn run(name: &str, d: &RingDesign, lib: &AlphaLibrary) {
    let params = BuildParams { theta_steps: 384, profile_steps: 160, ..Default::default() };
    let built = mesh::build(d, lib, params);
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    println!("\n=== {name} ===");
    println!(
        "  verdict {:<20} parting z {:+.3} mm   worst draft {:+.2} deg",
        rep.verdict.label(),
        rep.parting_z_mm,
        rep.worst_draft_deg
    );
    println!(
        "  good {:<7} marginal {:<6} vertical {:<7} undercut {:<6} ({:.3}% of area)",
        rep.good, rep.marginal, rep.vertical, rep.undercut,
        rep.undercut_fraction() * 100.0
    );
    for n in &rep.notes {
        println!("  - {n}");
    }
    let sec = castability::section_at(d, lib, 90.0, 200);
    println!(
        "  section @90deg: {} pts, r {:.2}..{:.2}, z {:.2}..{:.2}, min wall {:.2} mm, {} undercut segs",
        sec.points.len(), sec.min_r, sec.max_r, sec.min_z, sec.max_z, sec.min_wall_mm, sec.undercut_count
    );
}

fn main() {
    let lib = AlphaLibrary::builtin();

    let mut plain = RingDesign::default();
    plain.profile.apply_style(ProfileStyle::HalfRound);
    run("Plain half-round band (should be castable)", &plain, &lib);

    let mut flat = RingDesign::default();
    flat.profile.apply_style(ProfileStyle::Flat);
    run("Flat band (near-vertical outer wall)", &flat, &lib);

    // A straight-walled boss: crown 0 with no skirt is a cylinder standing on
    // the band, which is the classic sand-casting failure.
    let mut boss = RingDesign::default();
    boss.profile.apply_style(ProfileStyle::HalfRound);
    let ctx = boss.field_context();
    boss.layers.layers.push(LayerEntry::new(
        "straight boss",
        Layer::SeatPad(SeatPadLayer {
            theta_deg: 90.0,
            v_mm: ctx.crest_v_mm,
            diameter_mm: 6.0,
            height_mm: 2.5,
            crown: 0.0,
            blend_mm: 0.0,
        }),
    ));
    run("Straight-walled boss (should NOT release)", &boss, &lib);

    let mut domed = RingDesign::default();
    domed.profile.apply_style(ProfileStyle::HalfRound);
    let ctx2 = domed.field_context();
    domed.layers.layers.push(LayerEntry::new(
        "domed pad",
        Layer::SeatPad(SeatPadLayer {
            theta_deg: 90.0,
            v_mm: ctx2.crest_v_mm,
            diameter_mm: 6.0,
            height_mm: 1.4,
            crown: 1.0,
            blend_mm: 0.8,
        }),
    ));
    run("Domed gem seat pad (should be castable)", &domed, &lib);

    // A hard-edged alpha at tall relief: its walls are near-vertical by construction.
    for (name, alpha, h) in [
        ("Greek Key relief 0.30 mm", "Greek Key", 0.30),
        ("Greek Key relief 0.90 mm", "Greek Key", 0.90),
        ("Rope relief 0.90 mm (soft shoulders)", "Rope", 0.90),
    ] {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::HalfRound);
        let c = d.field_context();
        let mut t = ringdesign_core::tiling::TilingLayer::default_for(alpha, &c);
        t.repeats_around = 20;
        t.height_mm = h;
        t.v_span_mm = c.band_v_len_mm * 0.5;
        d.layers.layers.push(LayerEntry::new(alpha, Layer::Tiling(t)));
        run(name, &d, &lib);
    }

    // Where the relief sits is what decides it: side faces pull straight out.
    for (name, style, v_frac, span_frac, h) in [
        ("Rope 0.4mm on the CREST (half round)", ProfileStyle::HalfRound, 0.50, 0.45, 0.4),
        ("Rope 0.4mm on the SIDE FACE (half round)", ProfileStyle::HalfRound, 0.13, 0.20, 0.4),
        ("Rope 0.4mm on the crest (HIGH DOME)", ProfileStyle::HighDome, 0.50, 0.45, 0.4),
        ("Rope 0.15mm on the crest (half round)", ProfileStyle::HalfRound, 0.50, 0.45, 0.15),
    ] {
        let mut d = RingDesign::default();
        d.profile.apply_style(style);
        let c = d.field_context();
        let mut t = ringdesign_core::tiling::TilingLayer::default_for("Rope", &c);
        t.repeats_around = 24;
        t.height_mm = h;
        t.v_center_mm = c.band_v_len_mm * v_frac;
        t.v_span_mm = c.band_v_len_mm * span_frac;
        d.layers.layers.push(LayerEntry::new("rope", Layer::Tiling(t)));
        run(name, &d, &lib);
    }
}
