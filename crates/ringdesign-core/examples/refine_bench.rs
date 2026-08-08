//! Triangles and time needed to hit a given surface accuracy, refined vs swept.

use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::field::{Layer, LayerEntry, MilgrainLayer};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::refine::RefineParams;
use ringdesign_core::tiling::TilingLayer;
use ringdesign_core::RingDesign;

fn main() {
    let lib = AlphaLibrary::builtin();
    let mut d = RingDesign::default();
    d.profile.apply_style(ringdesign_core::ProfileStyle::DShape);
    let ctx = d.field_context();
    let name = lib.names()[0].clone();
    d.layers
        .layers
        .push(LayerEntry::new("tile", Layer::Tiling(TilingLayer::default_for(name, &ctx))));
    d.layers.layers.push(LayerEntry::new(
        "milgrain",
        Layer::Milgrain(MilgrainLayer { v_mm: 0.55, ..MilgrainLayer::default() }),
    ));

    println!("{:<28} {:>10} {:>10} {:>12}", "build", "tris", "ms", "worst err mm");
    for &(name, t, p) in BuildParams::PRESETS {
        let out = mesh::build(&d, &lib, BuildParams { theta_steps: t, profile_steps: p, ..Default::default() });
        let err = ringdesign_core::refine::grid_error_mm(&d, &lib, t as u32, p as u32, 0.5);
        println!(
            "{:<28} {:>10} {:>10} {:>12.4}",
            format!("swept {name} {t}x{p}"),
            out.report.validation.triangle_count,
            out.report.build_ms,
            err
        );
    }
    for &(name, tol, tilt) in RefineParams::PRESETS {
        let rp = RefineParams { tolerance_mm: tol, normal_tolerance_deg: tilt, base_cell_mm: 1.6, max_level: 6 };
        let out = mesh::build(&d, &lib, BuildParams { refine: Some(rp), ..Default::default() });
        let s = out.report.refine.unwrap();
        println!(
            "{:<28} {:>10} {:>10} {:>12.4}",
            format!("refined {name} tol {tol}"),
            out.report.validation.triangle_count,
            out.report.build_ms,
            s.worst_error_mm
        );
    }
}
