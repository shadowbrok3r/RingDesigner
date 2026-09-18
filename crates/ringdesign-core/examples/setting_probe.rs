//! Seats as solids: build a few rings with pre-made heads and burs, check them, time them, render them.
//! cargo run --release -p ringdesign-core --example setting_probe -- /tmp/out
use ringdesign_core::field::{Layer, LayerEntry, SeatPadLayer, SeatStyle};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::setting::SolidKind;
use ringdesign_core::{AlphaLibrary, BuildParams, ProfileStyle, RingDesign, render};

fn seat(d: &RingDesign, theta: f64, gem: Gem, kind: SolidKind, style: SeatStyle, height: f64) -> SeatPadLayer {
    let ctx = d.field_context();
    let mut pad = SeatPadLayer { theta_deg: theta, v_mm: ctx.crest_v_mm, style, height_mm: height, blend_mm: 0.6, solid: kind, through: true, metal_true: true, ..Default::default() };
    pad.fit_stone(gem);
    pad.height_mm = height;
    pad
}

fn main() -> anyhow::Result<()> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/setting_probe".into()));
    std::fs::create_dir_all(&out)?;
    let lib = AlphaLibrary::builtin();
    let mut designs: Vec<(String, RingDesign)> = Vec::new();

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 3.2;
    d.profile.thickness_mm = 1.9;
    let pad = seat(&d, 90.0, Gem::calibrated(GemCut::Round, 6.0), SolidKind::Prong, SeatStyle::Boss, 0.5);
    d.layers.layers.push(LayerEntry::new("Centre", Layer::SeatPad(pad)));
    designs.push(("claw-solitaire".into(), d));

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 4.0;
    d.profile.thickness_mm = 2.0;
    let pad = seat(&d, 90.0, Gem::calibrated(GemCut::Oval, 5.0), SolidKind::Bezel, SeatStyle::Boss, 0.4);
    d.layers.layers.push(LayerEntry::new("Centre", Layer::SeatPad(pad)));
    designs.push(("collet-oval".into(), d));

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.2;
    for (k, w) in [(0, 3.0), (-1, 2.4), (1, 2.4), (-2, 1.8), (2, 1.8)] {
        let pad = seat(&d, 90.0 + k as f64 * 19.0, Gem::calibrated(GemCut::Round, w), SolidKind::Flush, SeatStyle::GypsyMound, 0.0);
        d.layers.layers.push(LayerEntry::new(format!("Flush {w}"), Layer::SeatPad(pad)));
    }
    designs.push(("flush-five".into(), d));

    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 3.0;
    d.profile.thickness_mm = 1.8;
    for k in -6..=6 {
        let pad = seat(&d, 90.0 + k as f64 * 9.2, Gem::calibrated(GemCut::Round, 1.5), SolidKind::Bead, SeatStyle::Boss, 0.0);
        d.layers.layers.push(LayerEntry::new(format!("Melee {k}"), Layer::SeatPad(pad)));
    }
    designs.push(("bead-row".into(), d));

    for (name, d) in &designs {
        for (label, params) in [("preview", BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }), ("export", BuildParams { theta_steps: 1024, profile_steps: 320, ..Default::default() })] {
            let b = ringdesign_core::mesh::try_build(d, &lib, params)?;
            println!(
                "{name} {label}: {} faces, solids {} resolved / {} faces in {} ms, watertight {}, boundary {} non-manifold {}, volume {:.1} mm3, notes {:?}",
                b.mesh.faces.len(), b.solids.resolved, b.solids.faces, b.solids.ms, b.report.validation.watertight,
                b.report.validation.boundary_edges, b.report.validation.non_manifold_edges, b.report.volume_mm3, b.solids.notes
            );
            if label == "export" {
                let stones = ringdesign_core::gems::preview_mesh(d, &lib);
                for (view, yaw, pitch) in [("hero", 0.6f64, 0.75f64), ("top", 0.0, 1.5), ("side", 1.45, 0.12)] {
                    let mut parts = vec![render::Part::metal(&b.mesh, render::GOLD)];
                    if view != "top-bare" { if let Some(s) = &stones { parts.push(render::Part::stone(s)); } }
                    render::write_png_parts(out.join(format!("{name}-{view}.png")), &parts, yaw, pitch, 1400)?;
                }
                let parts = vec![render::Part::metal(&b.mesh, render::GOLD)];
                render::write_png_parts(out.join(format!("{name}-bare.png")), &parts, 0.6, 0.75, 1400)?;
            }
        }
    }
    Ok(())
}
