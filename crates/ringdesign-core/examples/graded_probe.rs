//! What a graduated row's bridges actually do.
//!
//! Stations evenly spaced in theta leave the metal between them growing with
//! every step, because the seats shrink and the pitch does not. Holding the
//! *bridge* constant instead makes `R dΔ = span·scale(Δ) + bridge`, and
//! `scale_at` is a raised cosine, so it integrates in closed form.
//!
//! Run: `cargo run --example graded_probe`

use ringdesign_core::field::SeatRunLayer;
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::{ProfileStyle, RingDesign};

fn main() {
    let mut d = RingDesign::default();
    d.profile.apply_style(ProfileStyle::LowDome);
    let ctx = d.field_context();

    println!("{:>6} {:>6} {:>9} {:>9} {:>9} {:>8}", "taper", "count", "min", "max", "spread", "report");
    for taper in [0.0, 0.2, 0.4, 0.6, 0.85] {
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 1.5);
        run.seat.v_mm = ctx.crest_v_mm;
        run.bridge_mm = 0.4;
        run.taper = taper;
        run.solve_spacing(&ctx);

        let (warped, uniform) = bridges(&run, &ctx);
        let f = |v: &Vec<f64>| {
            let lo = v.iter().cloned().fold(f64::MAX, f64::min);
            let hi = v.iter().cloned().fold(0.0f64, f64::max);
            (lo, hi, hi / lo)
        };
        let (lo, hi, r) = f(&warped);
        let (_, _, ru) = f(&uniform);
        println!(
            "{taper:>6.2} {:>6} {lo:>9.3} {hi:>9.3} {r:>8.2}x {:>7.3} | uniform would be {ru:.2}x",
            run.count,
            run.bridge_at(&ctx)
        );
    }
}

/// Girdle-to-girdle metal at every boundary of the row, warped and uniform.
fn bridges(
    run: &SeatRunLayer,
    ctx: &ringdesign_core::field::FieldContext,
) -> (Vec<f64>, Vec<f64>) {
    let n = run.count as usize;
    let r = ctx.crest_radius_mm * ctx.arc_scale(run.seat.v_mm);
    // The seat scales whole, footprint and skirt together, exactly as
    // `SeatRunLayer::height` scales it — a graded row is a row of
    // self-similar mounds.
    let gap = |a: f64, b: f64| {
        let arc = ringdesign_core::field::wrap_delta(b - a, 360.0).abs().to_radians() * r;
        let half = |t: f64| run.seat_span_mm() * 0.5 * run.scale_at(t);
        arc - half(a) - half(b)
    };
    let mut warped = Vec::new();
    for k in 0..n {
        let (a, b) = (k as f64, k as f64 + 1.0);
        warped.push(gap(run.theta_of_station(a, ctx), run.theta_of_station(b, ctx)));
    }
    // What the row did before: a count solved off the full-size seat, and a
    // uniform angular lattice.
    let k_arc = ctx.arc_scale(run.seat.v_mm);
    let n_old = ((ctx.circumference_mm * k_arc / (run.seat_span_mm() * k_arc + run.bridge_mm))
        .floor() as usize)
        .clamp(3, 200);
    let step = 360.0 / n_old as f64;
    let uniform = (0..n_old).map(|k| gap(k as f64 * step, (k + 1) as f64 * step)).collect();
    (warped, uniform)
}
