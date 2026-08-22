//! The bypass shank, measured and rendered: bypass_probe [out dir]
//!
//! Prints the modulation round the crossing, the field verdict, and writes
//! hero, top and side renders of the read for an eyeball pass.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::profile::{ProfileStyle, ShankKind, TOP_DEG};
use ringdesign_core::{castability, mesh, render, RingDesign};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let lib = AlphaLibrary::builtin();
    for (name, style, width) in [("lowdome", ProfileStyle::LowDome, 5.0), ("flat", ProfileStyle::Flat, 6.0)] {
        let mut d = RingDesign::default();
        d.profile.width_mm = width;
        d.profile.apply_style(style);
        d.shank.kind = ShankKind::Bypass;
        d.shank.amount = 1.0;
        let ir = d.inner_radius_mm();
        let base = ir + d.profile.thickness_mm;
        println!("== {name}: theta  width  slide  groove");
        for off in [-90.0, -60.0, -37.5, -15.0, 0.0, 15.0, 37.5, 60.0, 90.0] {
            let m = d.modulation_at(TOP_DEG + off, ir, base);
            println!("{:+6.1}  {:.2}  {:+.2}  {:.2}", off, m.width_scale, m.z_center_frac, m.side_groove_mm);
        }
        let f = castability::analyze_field(&d, &lib, &d.draft, 384, 192);
        println!("field: {:?} {:.4}% worst {:+.1} deg", f.verdict, f.undercut_fraction() * 100.0, f.worst_draft_deg);
        let built = mesh::build(&d, &lib, BuildParams { theta_steps: 768, profile_steps: 256, ..Default::default() });
        let m = &built.mesh;
        let dir = std::path::Path::new(&out);
        for (tag, yaw, pitch) in [("hero", 0.55, 1.12), ("top", 0.0, 1.55), ("side", 1.5708, 0.05)] {
            let path = dir.join(format!("bypass_{name}_{tag}.png"));
            render::write_png(&path, m, yaw, pitch, 900, render::GOLD).expect("png");
            println!("wrote {}", path.display());
        }
    }
}
