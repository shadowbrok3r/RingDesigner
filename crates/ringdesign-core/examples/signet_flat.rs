// Is the signet table flat on the built mesh?
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::field::{Layer, LayerEntry, SignetLayer, SignetOutline};
use ringdesign_core::mesh::{self, BuildParams};
use ringdesign_core::RingDesign;

fn measure(outline: SignetOutline, width_mm: f64, length_mm: f64) {
    let lib = AlphaLibrary::builtin();
    let mut d = RingDesign::default();
    let ctx = d.field_context();
    let s = SignetLayer { v_mm: ctx.crest_v_mm, outline, width_mm, length_mm, ..Default::default() };
    let (flat, probe) = (s.top_flat, s);
    d.layers.layers.push(LayerEntry::new("signet", Layer::Signet(s)));

    let built = mesh::build(
        &d,
        &lib,
        BuildParams { theta_steps: 1024, profile_steps: 384, ..Default::default() },
    );

    // The table faces +Y and stands proud of the crest, so anything at or below
    // the crest radius is band or bore, not table.
    let floor_r = ctx.crest_radius_mm + 0.05;
    let (mut lo, mut hi, mut n) = (f64::MAX, f64::MIN, 0usize);
    for v in &built.mesh.vertices {
        let (x, y, z) = (v.0 as f64, v.1 as f64, v.2 as f64);
        if y <= 0.0 || (x * x + y * y).sqrt() < floor_r {
            continue;
        }
        // Arc offset around the ring, and offset across the band.
        let du = x.atan2(y) * ctx.crest_radius_mm;
        let dv = z;
        if probe.outline_distance(du, dv) > flat * 0.9 {
            continue;
        }
        lo = lo.min(y);
        hi = hi.max(y);
        n += 1;
    }
    let err = hi - lo;
    println!(
        "{:<10} {width_mm:>4.1}x{length_mm:<5.1} {n:>6} samples   flatness {err:.4} mm   {}",
        format!("{:?}", outline),
        if err < 0.05 { "FLAT" } else { "domed" }
    );
}

fn main() {
    println!("Signet table flatness (a graver needs well under 0.05 mm)\n");
    println!("band is 6.0 mm wide; a table wider than that hangs off surface that does not exist\n");
    for (w, l) in [(9.0, 12.0), (7.0, 10.0), (5.0, 8.0), (4.0, 6.0), (3.0, 5.0)] {
        measure(SignetOutline::Oval, w, l);
    }
    println!();
    for o in SignetOutline::ALL {
        measure(*o, 4.0, 6.0);
    }
}
