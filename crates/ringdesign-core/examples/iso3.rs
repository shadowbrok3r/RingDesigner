// How much room a table gets on each profile, and whether it then casts.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{Layer, LayerEntry, SignetLayer};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::ShankKind;
use ringdesign_core::{ProfileStyle, RingDesign};

fn main() {
    let lib = AlphaLibrary::builtin();
    let p = BuildParams { theta_steps: 900, profile_steps: 256, ..Default::default() };
    println!("{:<28} {:>8} {:>8} {:>8}   verdict", "profile", "band", "room", "table");
    for (label, style, squared, taper) in [
        ("Flat squared + taper", ProfileStyle::Flat, true, true),
        ("Flat + taper", ProfileStyle::Flat, false, true),
        ("Flat squared", ProfileStyle::Flat, true, false),
        ("Low dome + taper", ProfileStyle::LowDome, false, true),
        ("D-shape + taper", ProfileStyle::DShape, false, true),
        ("Half round + taper", ProfileStyle::HalfRound, false, true),
        ("Cushion dome + taper", ProfileStyle::CushionDome, false, true),
    ] {
        let mut d = RingDesign::default();
        d.profile.apply_style(style);
        d.profile.width_mm = 12.0;
        d.profile.thickness_mm = 2.8;
        if squared {
            d.profile.flatten_sides();
        }
        if taper {
            d.shank.kind = ShankKind::Signet;
            d.shank.amount = 0.85;
        }
        let ctx = d.field_context();
        let room = SignetLayer::room_across(&ctx);
        let mut s = SignetLayer::fitted_to(&ctx);
        s.height_mm = 0.5;
        s.top_flat = 0.86;
        s.shoulder_mm = 0.8;
        s.fill_head(&ctx);
        let table = s.width_mm;
        d.layers.layers.push(LayerEntry::new("table", Layer::Signet(s)));
        let b = mesh::build(&d, &lib, p);
        let r = castability::analyze(&b.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "{label:<28} {:>8.2} {room:>8.2} {table:>8.2}   {:<18} {:>6.3}% undercut  worst {:>+7.2} deg",
            ctx.band_v_len_mm, r.verdict.label(), r.undercut_fraction() * 100.0, r.worst_draft_deg
        );
    }
}
