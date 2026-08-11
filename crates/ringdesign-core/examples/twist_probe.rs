//! Twist-band undercut vs strength and sweep resolution.
use ringdesign_core::castability;
use ringdesign_core::profile::{ShankKind, ShankStyle};
use ringdesign_core::{AlphaLibrary, BuildParams, ProfileStyle, RingDesign};

fn main() {
    let lib = AlphaLibrary::builtin();
    for amount in [0.5, 0.75, 1.0] {
        for (t, p) in [(384usize, 128usize), (768, 256)] {
            let mut d = RingDesign::default();
            d.profile.apply_style(ProfileStyle::LowDome);
            d.shank =
                ShankStyle { kind: ShankKind::Twist, amount, waves: 3, ..Default::default() };
            let out = ringdesign_core::mesh::build(
                &d,
                &lib,
                BuildParams { theta_steps: t, profile_steps: p, ..Default::default() },
            );
            let cast = castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
            println!(
                "amount {amount} {t}x{p}: {:.4}% worst {:.1}",
                cast.undercut_fraction() * 100.0,
                cast.worst_draft_deg
            );
        }
    }
}
