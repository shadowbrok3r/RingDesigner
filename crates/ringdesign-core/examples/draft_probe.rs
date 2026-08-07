// Is draft_angle right, and does ANY relief survive?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, FaceClass};
use ringdesign_core::field::{Layer, LayerEntry};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::{ProfileStyle, RingDesign};

fn main() {
    // --- Unit check of the sign convention. ---
    let p = 0.0;
    println!("draft_angle sign convention (parting z = 0):");
    for (label, n, z) in [
        ("above, normal +Z  (should be +90)", [0.0, 0.0, 1.0], 2.0),
        ("above, normal -Z  (should be -90)", [0.0, 0.0, -1.0], 2.0),
        ("above, normal +X  (should be   0)", [1.0, 0.0, 0.0], 2.0),
        ("below, normal -Z  (should be +90)", [0.0, 0.0, -1.0], -2.0),
        ("below, normal +Z  (should be -90)", [0.0, 0.0, 1.0], -2.0),
        ("above, 45 deg out+up (should +45)", [0.7071, 0.0, 0.7071], 2.0),
    ] {
        println!("  {label:<36} -> {:+7.2}", castability::draft_angle(n, z, p));
    }

    let lib = AlphaLibrary::builtin();
    println!("\nrelief sweep (Hammered = smooth dimples, High Dome, on the crest):");
    for h in [0.05, 0.10, 0.20, 0.40] {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::HighDome);
        d.profile.thickness_mm = 2.6;
        let c = d.field_context();
        let mut t = ringdesign_core::tiling::TilingLayer::default_for("Hammered", &c);
        t.repeats_around = 12;
        t.height_mm = h;
        t.v_span_mm = c.band_v_len_mm * 0.5;
        d.layers.layers.push(LayerEntry::new("hammered", Layer::Tiling(t)));
        let built = mesh::build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 160, ..Default::default() });
        let r = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "  relief {h:.2} mm -> {:<20} undercut {:.2}% area, worst {:+.1} deg",
            r.verdict.label(), r.undercut_fraction() * 100.0, r.worst_draft_deg
        );
    }

    // --- Where do the undercut faces actually sit? ---
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::HalfRound);
    let c = d.field_context();
    let mut t = ringdesign_core::tiling::TilingLayer::default_for("Hammered", &c);
    t.repeats_around = 12;
    t.height_mm = 0.2;
    t.v_span_mm = c.band_v_len_mm * 0.5;
    d.layers.layers.push(LayerEntry::new("hammered", Layer::Tiling(t)));
    let built = mesh::build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 160, ..Default::default() });
    let r = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());

    let mut zs: Vec<f64> = Vec::new();
    for (i, f) in built.mesh.faces.iter().enumerate() {
        if r.classes[i] == FaceClass::Undercut {
            if let Some((a, b, cc)) = built.mesh.triangle(f) {
                zs.push((a[2] + b[2] + cc[2]) / 3.0);
            }
        }
    }
    zs.sort_by(f64::total_cmp);
    if zs.is_empty() {
        println!("\nno undercut faces at 0.2 mm hammered relief");
    } else {
        println!(
            "\n{} undercut faces at z from {:+.2} to {:+.2} (parting {:+.3}), median {:+.2}",
            zs.len(), zs[0], zs[zs.len() - 1], r.parting_z_mm, zs[zs.len() / 2]
        );
    }
}
