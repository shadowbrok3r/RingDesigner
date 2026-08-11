// Contact sheet of the design-complexity wave: demo_wave2 <out dir>
use ringdesign_core::curve::CurveLayer;
use ringdesign_core::field::{FlutesLayer, Layer, LayerEntry, SideFacePick, VGate};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::profile::{ProfileMorph, ShankKind, ShankStyle};
use ringdesign_core::text::{TextAlpha, TextFont};
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{AlphaLibrary, ProfileStyle, RingDesign, castability};

#[path = "common/raster.rs"]
mod raster;

const W: usize = 820;
const H: usize = 820;

fn shot(name: &str, d: &RingDesign, lib: &AlphaLibrary, out: &str) {
    let params = BuildParams { theta_steps: 768, profile_steps: 224, ..Default::default() };
    let built = mesh::build(d, lib, params);
    let rep = castability::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
    println!(
        "{name}: {} {:.3}% undercut, watertight {}, relief {:.2} mm",
        rep.verdict.label(),
        rep.undercut_fraction() * 100.0,
        built.report.validation.watertight,
        built.report.max_relief_mm
    );
    let img = raster::render(&built.mesh, 0.55, 0.42, W, H);
    image::save_buffer(
        format!("{out}/{name}.png"),
        &img,
        W as u32,
        H as u32,
        image::ColorType::Rgb8,
    )
    .unwrap();
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    let mut lib = AlphaLibrary::builtin();

    // 1. Twist band carrying guilloche on squared sides.
    let mut twist = RingDesign::default();
    twist.name = "twist".into();
    twist.profile.width_mm = 7.0;
    twist.profile.apply_style(ProfileStyle::LowDome);
    twist.shank = ShankStyle { kind: ShankKind::Twist, amount: 1.0, waves: 3, ..Default::default() };
    // Bare: a twisted crown's zero-draft line oscillates, so texture there
    // locks — the spiral light-line is the ornament.
    shot("1_twist", &twist, &lib, &out);

    // 2. Wave band, morphed crown, milgrain edges.
    let mut wave = RingDesign::default();
    wave.name = "wave_morph".into();
    wave.profile.apply_style(ProfileStyle::DShape);
    wave.profile.morph = Some(ProfileMorph::from_style(ProfileStyle::Flat, &wave.profile));
    wave.shank = ShankStyle { kind: ShankKind::Wave, amount: 1.0, waves: 1, ..Default::default() };
    shot("2_wave_morph", &wave, &lib, &out);

    // 3. Reeded bombe band with a starburst crown.
    let mut reeded = RingDesign::default();
    reeded.name = "bombe_reeded".into();
    reeded.profile.apply_style(ProfileStyle::LowDome);
    reeded.shank = ShankStyle { kind: ShankKind::Bombe, amount: 0.7, ..Default::default() };
    reeded
        .layers
        .layers
        .push(LayerEntry::new("reeding", Layer::Flutes(FlutesLayer::default())));
    shot("3_bombe_reeded", &reeded, &lib, &out);

    // 4. Squared band: script inscription and a vine, both on the side faces.
    let mut side = RingDesign::default();
    side.name = "side_text_vine".into();
    side.profile.width_mm = 8.0;
    side.profile.thickness_mm = 3.0;
    side.profile.apply_style(ProfileStyle::Flat);
    side.profile.flatten_sides();
    side.texts.push(TextAlpha {
        name: "inscription".into(),
        text: "Semper fidelis".into(),
        font: TextFont::Script,
        tracking: 0.05,
    });
    side.bake_all(&mut lib);
    let ctx = side.field_context();
    let sf = ctx.side_faces_std().expect("squared sides").wider().unwrap();
    let mut vine = CurveLayer::preset_vine(&ctx);
    vine.height_mm = 0.45;
    vine.mirror_v = true;
    vine.retarget_v(0.5 * (sf.0 + sf.1), (sf.1 - sf.0) * 0.3);
    let mut entry = LayerEntry::new("vine", Layer::Curve(vine));
    entry.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    entry.window.enabled = true;
    entry.window.theta_deg = 270.0;
    entry.window.span_deg = 150.0;
    side.layers.layers.push(entry);
    // The tile's own band must sit on the run; the gate only attenuates.
    let mut text_tile = TilingLayer::default_for("inscription", &ctx);
    text_tile.repeats_around = 3;
    text_tile.height_mm = 0.4;
    text_tile.rows = 1;
    text_tile.v_center_mm = 0.5 * (sf.0 + sf.1);
    text_tile.v_span_mm = (sf.1 - sf.0) * 0.9;
    text_tile.mirror_v = true;
    let mut text_entry = LayerEntry::new("text", Layer::Tiling(text_tile));
    text_entry.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    text_entry.window.enabled = true;
    text_entry.window.theta_deg = 90.0;
    text_entry.window.span_deg = 150.0;
    side.layers.layers.push(text_entry);
    shot("4_side_text_vine", &side, &lib, &out);
}
