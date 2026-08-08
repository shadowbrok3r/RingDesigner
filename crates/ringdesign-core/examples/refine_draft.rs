//! Does a refined mesh report the same castability as a dense swept one?
//!
//! Refinement is driven by *position* error, but `castability::analyze` reads
//! face *normals*. A coarse facet's normal is a chord, so a mesh can sit close
//! to the surface and still misreport draft.

use ringdesign_core::castability;
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::refine::RefineParams;
use ringdesign_core::{ProfileStyle, RingDesign};

fn main() {
    let lib = ringdesign_core::AlphaLibrary::builtin();
    for (name, d) in [("signet shank", signet()), ("plain half round", RingDesign::default())] {
        println!("\n== {name} ==");
        println!("{:<26} {:>9} {:>10} {:>12} {:>10}", "build", "tris", "undercut%", "worst draft", "verdict");
        let truth = report(&d, &lib, BuildParams { theta_steps: 1536, profile_steps: 448, ..Default::default() }, "swept 1536x448");
        for &(t, p) in &[(384usize, 144usize), (512, 192)] {
            report(&d, &lib, BuildParams { theta_steps: t, profile_steps: p, ..Default::default() }, &format!("swept {t}x{p}"));
        }
        for &(pn, tol, tilt) in RefineParams::PRESETS {
            let rp = RefineParams { tolerance_mm: tol, normal_tolerance_deg: tilt, base_cell_mm: 1.6, max_level: 6 };
            report(&d, &lib, BuildParams { refine: Some(rp), ..Default::default() }, &format!("refined {pn} {tol}mm/{tilt}deg"));
        }
        println!("(reference undercut {truth:.3}%)");
    }
}

fn signet() -> RingDesign {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.shank.kind = ringdesign_core::ShankKind::Signet;
    d.shank.amount = 0.72;
    d
}

fn report(d: &RingDesign, lib: &ringdesign_core::AlphaLibrary, p: BuildParams, name: &str) -> f64 {
    let out = mesh::build(d, lib, p);
    let rep = castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
    let pct = rep.undercut_fraction() * 100.0;
    println!(
        "{:<26} {:>9} {:>9.3}% {:>11.2}° {:>10}",
        name,
        out.report.validation.triangle_count,
        pct,
        rep.worst_draft_deg,
        rep.verdict.label()
    );
    pct
}
