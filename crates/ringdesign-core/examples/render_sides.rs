// Contact sheet: a signet whose ornament sits on the side faces, not the table.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability;
use ringdesign_core::field::{
    Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SignetLayer, Window,
};
use ringdesign_core::mesh::{self, BuildParams, Mesh};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{ProfileStyle, RingDesign};

const W: usize = 760;
const H: usize = 760;

#[path = "common/raster.rs"]
mod raster;


fn params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 224, ..Default::default() }
}

/// A signet on a squared-sided band with ornament fitted to the side faces.
fn signet_with_side_ornament(alpha: &str, relief: f64, window: bool) -> RingDesign {
    let mut d = RingDesign::default();
    // The ornamentable side face is thickness minus crown, so a thick band on a
    // flat profile leaves the most of it.
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.flatten_sides();

    let ctx = d.field_context();
    let mut s = SignetLayer::fitted_to(&ctx);
    s.height_mm = 1.5;
    d.layers.layers.push(LayerEntry::new("signet", Layer::Signet(s)));

    let mut t = TilingLayer::default_for(alpha, &ctx);
    assert!(t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG), "squared sides should fit");
    t.height_mm = relief;
    let f = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).unwrap();
    println!(
        "  side face {:.2} mm, {} tiles of {:.2} x {:.2} mm",
        f.low_width(),
        t.repeats_around,
        t.cell_size(&ctx).0,
        t.cell_size(&ctx).1
    );
    let mut e = LayerEntry::new("side ornament", Layer::Tiling(t));
    if window {
        // Everywhere but the head, so nothing creeps onto the table.
        e = e.with_window(Window::except(90.0, 70.0));
    }
    d.layers.layers.push(e);
    d
}

fn save(path: &str, m: &Mesh, yaw: f64, pitch: f64) {
    let img = raster::render(m, yaw, pitch, W, H);
    image::save_buffer(path, &img, W as u32, H as u32, image::ColorType::Rgb8).unwrap();
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let mut lib = AlphaLibrary::builtin();
    for dir in ringdesign_core::library::alpha_dirs() {
        let _ = lib.load_dir(dir);
    }
    let alpha = ["ornament-a-01", "Rope"]
        .into_iter()
        .find(|n| lib.get(n).is_some())
        .unwrap_or("Rope");

    for (name, relief, window, yaw, pitch) in [
        ("side_signet_face", 0.5, true, 0.0, 0.35),
        ("side_signet_edge", 0.5, true, 0.0, 1.35),
        ("side_signet_three_quarter", 0.5, true, 0.6, 0.85),
        ("side_signet_unwindowed", 0.5, false, 0.6, 0.85),
    ] {
        let d = signet_with_side_ornament(alpha, relief, window);
        let built = mesh::build(&d, &lib, params());
        let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "{name:<28} {:<18} {:>6.3}% undercut  worst {:>+6.2} deg  relief {:.2} mm  watertight {}",
            rep.verdict.label(),
            rep.undercut_fraction() * 100.0,
            rep.worst_draft_deg,
            built.report.max_relief_mm,
            built.report.validation.watertight,
        );
        save(&format!("{out}/{name}.png"), &built.mesh, yaw, pitch);
    }

    // Control: the same ornament on the crest of the same band.
    let mut d = signet_with_side_ornament(alpha, 0.5, true);
    if let Some(Layer::Tiling(t)) = d.layers.layers.last_mut().map(|e| &mut e.layer) {
        let ctx = RingDesign::default().field_context();
        let _ = ctx;
        t.mirror_v = false;
        t.v_center_mm = 4.4;
        t.v_span_mm = 3.0;
    }
    let built = mesh::build(&d, &lib, params());
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    println!(
        "{:<28} {:<18} {:>6.3}% undercut  worst {:>+6.2} deg   <- control, same tiles on the crest",
        "crest_control",
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        rep.worst_draft_deg
    );
    save(&format!("{out}/crest_control.png"), &built.mesh, 0.6, 0.85);
}
