// Base draft across each profile's cross-section, and what relief survives there.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::Flange;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{ProfileStyle, RingDesign};

fn params() -> BuildParams {
    BuildParams { theta_steps: 512, profile_steps: 192, ..Default::default() }
}

fn main() {
    let mut lib = AlphaLibrary::builtin();
    for dir in ringdesign_core::library::alpha_dirs() {
        let _ = lib.load_dir(dir);
    }
    let orn = if lib.get("ornament-a-01").is_some() { "ornament-a-01" } else { "Rope" };

    println!("profile presets (a, b, crown_frac, round_frac):");
    for s in ProfileStyle::ALL {
        let (a, b, c, r) = s.preset();
        println!("  {:<14} a {a:.2}  b {b:.2}  crown {c:.2}  round {r:.2}", s.label());
    }

    let mut cases: Vec<(String, RingDesign)> = Vec::new();
    for style in [ProfileStyle::Flat, ProfileStyle::LowDome, ProfileStyle::HalfRound] {
        let mut d = RingDesign::default();
        d.profile.apply_style(style);
        cases.push((style.label().to_string(), d));
    }
    // Symmetric flat sides: no side draft, small fillet.
    for (style, round) in [
        (ProfileStyle::Flat, 0.05),
        (ProfileStyle::LowDome, 0.05),
        (ProfileStyle::HalfRound, 0.05),
        (ProfileStyle::CushionDome, 0.05),
    ] {
        let mut d = RingDesign::default();
        d.profile.apply_style(style);
        d.profile.side_draft_deg = 0.0;
        d.profile.edge_round_mm = round;
        cases.push((format!("{} + planar sides", style.label()), d));
    }
    for extent in [0.6, 1.2] {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::HalfRound);
        d.profile.flange =
            Flange { enabled: true, v_pos: 0.0, extent_mm: extent, thickness_mm: 0.9, edge_round_mm: 0.15 };
        cases.push((format!("half round + {extent} mm edge flange"), d));
    }

    println!("\nornament on the side faces, NO signet:");
    for (label, d) in &cases {
        let c = d.field_context();
        let mut t = TilingLayer::default_for(orn, &c);
        let fitted = t.fit_to_side_faces(&c, SIDE_FACE_MIN_DRAFT_DEG);
        let span = c.side_faces(SIDE_FACE_MIN_DRAFT_DEG);
        println!(
            "\n  {label}  (band v span {:.2} mm, crest v {:.2} mm)",
            c.band_v_len_mm, c.crest_v_mm
        );
        // Where the draft actually is, at the resolution the fit uses.
        let mut marks = String::new();
        for k in 0..=40 {
            let v = k as f64 / 40.0 * c.band_v_len_mm;
            let deg = c.surface.draft_deg(v, c.band_v_len_mm).unwrap_or(0.0);
            marks.push(match deg {
                d if d >= 80.0 => '#',
                d if d >= 55.0 => '+',
                d if d >= 25.0 => '-',
                _ => '.',
            });
        }
        println!("    draft  [{marks}]   # 80+  + 55+  - 25+  . wall");
        match span {
            Some(f) => println!(
                "    side faces {} and {}  (even: {})",
                f.low.map_or("-".into(), |(a, b)| format!("{a:.2}..{b:.2}")),
                f.high.map_or("-".into(), |(a, b)| format!("{a:.2}..{b:.2}")),
                f.is_even()
            ),
            None => println!("    no side face at {SIDE_FACE_MIN_DRAFT_DEG:.0} deg"),
        }
        if !fitted {
            continue;
        }
        for h in [0.15, 0.30, 0.50, 0.80, 1.20, 1.60] {
            let mut r = d.clone();
            let mut tt = t.clone();
            tt.height_mm = h;
            r.layers.layers.push(LayerEntry::new("sides", Layer::Tiling(tt)));
            let built = mesh::build(&r, &lib, params());
            let rep = castability::analyze(&built.mesh, &r.draft, r.inner_radius_mm());
            println!(
                "    relief {h:.2} mm  {:<18} {:>6.3}% undercut   worst {:>+7.2} deg   actual relief {:.3} mm",
                rep.verdict.label(),
                rep.undercut_fraction() * 100.0,
                rep.worst_draft_deg,
                built.report.max_relief_mm,
            );
        }
    }
}
