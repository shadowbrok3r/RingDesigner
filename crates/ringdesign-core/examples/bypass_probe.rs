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
        println!("field: {:?} {:.4}% worst {:+.1} deg, thinnest wall {:.2} mm at {:.0} deg", f.verdict, f.undercut_fraction() * 100.0, f.worst_draft_deg, f.thinnest_wall_mm, f.thinnest_wall_theta_deg);
        for off in [0.0, 30.0, 45.0, 60.0] {
            let l = d.profile.sample_mod(ir, 192, &d.modulation_at(TOP_DEG + off, ir, base));
            let (zmin, zmax) = l.pts.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
            let edge_r = l.pts.iter().filter(|p| p.surface && (p.z - zmin).abs() < 0.05).map(|p| p.r).fold(f64::MIN, f64::max);
            println!("  off {off:+5.1}: z {zmin:+.2}..{zmax:+.2}, outer r at the low edge {edge_r:.2} vs bore {ir:.2} -> edge wall {:.2}", edge_r - ir);
        }
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
