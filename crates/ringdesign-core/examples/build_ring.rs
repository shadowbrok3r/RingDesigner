// End-to-end: build a tiled, bordered, gem-seated ring and export it.
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::field::{Blend, BorderLayer, Layer, LayerEntry, MilgrainLayer, SeatPadLayer};
use ringdesign_core::mesh::BuildParams;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::{RingDesign, mesh, stl};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/ring.stl".into());
    let lib = AlphaLibrary::builtin();

    let mut d = RingDesign::default();
    d.name = "Rope band with seat".into();
    d.size = ringdesign_core::RingSize(8.0);
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 2.4;
    d.profile.apply_style(ringdesign_core::ProfileStyle::HalfRound);

    let ctx = d.field_context();
    println!(
        "band: circumference {:.2} mm, v span {:.2} mm, crest at v {:.2}, crest r {:.2}",
        ctx.circumference_mm, ctx.band_v_len_mm, ctx.crest_v_mm, ctx.crest_radius_mm
    );

    let mut tiling = TilingLayer::default_for("Rope", &ctx);
    tiling.repeats_around = 32;
    tiling.height_mm = 0.4;
    tiling.v_span_mm = ctx.band_v_len_mm * 0.5;
    d.layers.layers.push(LayerEntry::new("rope", Layer::Tiling(tiling)));

    let mut border = LayerEntry::new("edge rails", Layer::Border(BorderLayer::default()));
    border.blend = Blend::Max;
    d.layers.layers.push(border);

    d.layers.layers.push(LayerEntry::new(
        "milgrain",
        Layer::Milgrain(MilgrainLayer::default()),
    ));

    let pad = SeatPadLayer { diameter_mm: 5.5, height_mm: 1.4, v_mm: ctx.crest_v_mm, ..Default::default() };
    println!("seat pad fits a {:.2} mm stone", pad.suggested_stone_mm());
    d.layers.layers.push(LayerEntry::new("gem seat", Layer::SeatPad(pad)));

    let params = BuildParams { theta_steps: 768, profile_steps: 256, ..Default::default() };
    let built = mesh::build(&d, &lib, params);
    let r = &built.report;
    println!(
        "\nmesh: {} tris, {} verts, watertight={} (boundary {}, non-manifold {})",
        r.validation.triangle_count,
        r.validation.vertex_count,
        r.validation.watertight,
        r.validation.boundary_edges,
        r.validation.non_manifold_edges
    );
    println!(
        "volume {:.1} mm3, relief +{:.3}/-{:.3} mm, size {:.1} x {:.1} x {:.1} mm, {} ms",
        r.volume_mm3, r.max_relief_mm, r.min_relief_mm,
        r.bounds_mm[0], r.bounds_mm[1], r.bounds_mm[2], r.build_ms
    );
    for m in r.metals.iter().filter(|m| m.metal.contains("14k") || m.metal.contains("925")) {
        println!("  {:<14} {:.2} g  ({:.2} dwt)", m.metal, m.grams, m.dwt);
    }
    let bytes = stl::write_stl(&out, &built.mesh, &d.name).unwrap();
    println!("\nwrote {out} ({:.1} KB)", bytes as f64 / 1024.0);
    assert!(r.validation.watertight, "MESH IS NOT WATERTIGHT");
}
