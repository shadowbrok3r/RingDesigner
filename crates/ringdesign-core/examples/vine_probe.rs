//! Undercut of a crest rail vs amplitude, and a side-face vine's contribution.
use ringdesign_core::castability;
use ringdesign_core::curve::CurveLayer;
use ringdesign_core::field::{SideFacePick, VGate};
use ringdesign_core::{AlphaLibrary, BuildParams, Layer, LayerEntry, ProfileStyle, RingDesign, mesh};

fn main() {
    let lib = AlphaLibrary::builtin();
    let params = BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() };

    for amp in [0.2, 0.4, 0.8] {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::LowDome);
        let fc = d.field_context();
        let m = fc.crest_v_mm;
        let rail = CurveLayer {
            points: vec![[0.0, m], [0.25, m + amp], [0.5, m], [0.75, m - amp]],
            repeats_around: 6,
            closed: true,
            height_mm: 0.15,
            taper: 0.0,
            ..CurveLayer::default()
        };
        d.layers.layers.push(LayerEntry::new("rail", Layer::Curve(rail)));
        let out = mesh::build(&d, &lib, params);
        let cast = castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        println!("rail amp {amp}: {:.3}% worst {:.1}", cast.undercut_fraction() * 100.0, cast.worst_draft_deg);
    }

    // Side-face vine: points inside the run, gated to it.
    let mut d = RingDesign::default();
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.0;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.flatten_sides();
    let fc = d.field_context();
    let sf = fc.side_faces_std().expect("faces");
    let (lo, hi) = sf.wider().unwrap();
    let mid = 0.5 * (lo + hi);
    let a = (hi - lo) * 0.3;
    println!("side run {lo:.2}..{hi:.2}");
    let vine = CurveLayer {
        points: vec![[0.0, mid], [0.25, mid + a], [0.5, mid], [0.75, mid - a], [1.0, mid]],
        repeats_around: 12,
        height_mm: 0.5,
        taper: 0.0,
        ..CurveLayer::default()
    };
    let mut entry = LayerEntry::new("side vine", Layer::Curve(vine));
    entry.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
    d.layers.layers.push(entry);
    let out = mesh::build(&d, &lib, params);
    let cast = castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
    println!(
        "side vine: {:.4}% worst {:.1}, relief {:.3} mm",
        cast.undercut_fraction() * 100.0,
        cast.worst_draft_deg,
        out.report.max_relief_mm
    );
}
