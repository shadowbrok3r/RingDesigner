// Does the plane solve fold the surface, or is the banding just shading?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{Layer, LayerEntry, SignetLayer};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::{DraftSettings, RingDesign};

fn main() {
    let lib = AlphaLibrary::builtin();
    let mut d = RingDesign::default();
    d.shank.kind = ringdesign_core::ShankKind::Cathedral;
    d.shank.amount = 0.9;
    let ctx = d.field_context();
    d.layers.layers.push(LayerEntry::new("signet", Layer::Signet(SignetLayer::fitted_to(&ctx))));

    let built = mesh::build(&d, &lib, BuildParams { theta_steps: 512, profile_steps: 256, ..Default::default() });
    let rep = castability::analyze(&built.mesh, &DraftSettings::default(), d.inner_radius_mm());

    println!("watertight   {}", built.report.validation.watertight);
    println!("verdict      {}", rep.verdict.label());
    println!("undercut     {} faces ({:.3}%)", rep.undercut, rep.undercut_fraction() * 100.0);
    println!("worst draft  {:+.2} deg", rep.worst_draft_deg);

    // A fold shows up as a face whose outward normal points back at the axis.
    let mut inward = 0usize;
    let mut table = 0usize;
    for f in &built.mesh.faces {
        let (Some(n), Some((a, b, c))) = (built.mesh.face_normal(f), built.mesh.triangle(f)) else {
            continue;
        };
        let cx = (a[0] + b[0] + c[0]) / 3.0;
        let cy = (a[1] + b[1] + c[1]) / 3.0;
        let r = (cx * cx + cy * cy).sqrt();
        if r < ctx.crest_radius_mm + 0.05 {
            continue;
        }
        table += 1;
        if (n[0] * cx + n[1] * cy) / r.max(1e-9) < -0.05 {
            inward += 1;
        }
    }
    println!("\ntable faces: {table}, pointing back at the axis (a fold): {inward}");
    println!("{}", if inward == 0 { "NO FOLD - the banding is flat-shading on a near-plane" } else { "FOLDED" });
}
