// Taper alone, table alone, both: which one undercuts?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{Layer, LayerEntry, SignetLayer};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::ShankKind;
use ringdesign_core::{ProfileStyle, RingDesign};

fn main() {
    let lib = AlphaLibrary::builtin();
    let p = BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() };

    for style in [ProfileStyle::HalfRound, ProfileStyle::DShape, ProfileStyle::Flat] {
        println!("\n{}:", style.label());
        for (name, taper, table) in [
            ("bare band", false, false),
            ("taper only", true, false),
            ("table only", false, true),
            ("both", true, true),
        ] {
            let mut d = RingDesign::default();
            d.profile.apply_style(style);
            d.profile.width_mm = 12.0;
            d.profile.thickness_mm = 2.6;
            if style == ProfileStyle::Flat {
                d.profile.flatten_sides();
            }
            if taper {
                d.shank.kind = ShankKind::Signet;
                d.shank.amount = 0.85;
            }
            if table {
                let ctx = d.field_context();
                let mut s = SignetLayer::fitted_to(&ctx);
                s.height_mm = 0.35;
                println!("      table {:.1} x {:.1} mm, reach {:.2}, room {:.2}",
                    s.length_mm, s.width_mm, s.reach_mm(), SignetLayer::room_across(&ctx));
                d.layers.layers.push(LayerEntry::new("table", Layer::Signet(s)));
            }
            let b = mesh::build(&d, &lib, p);
            let r = castability::analyze(&b.mesh, &d.draft, d.inner_radius_mm());
            println!("  {name:<12} {:<18} {:>6.3}% undercut  worst {:>+7.2} deg",
                r.verdict.label(), r.undercut_fraction() * 100.0, r.worst_draft_deg);
        }
    }
}
