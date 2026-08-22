//! What `u` costs on a side face.
//!
//! `u` is arc distance **at the crest radius**, so it is the true metal only
//! on the crest. A squared band's side faces sit well inside that radius, and
//! everything measured in `u` there — a seat bridge, a tile cell, a halo's
//! ring — is longer in the chart than it is in metal.
//!
//! Run: `cargo run --example metric_probe`

use ringdesign_core::{ProfileStyle, RingDesign};

fn main() {
    println!("{:<14} {:>7} {:>9} {:>9} {:>9}", "profile", "w x t", "crest", "face", "k");
    for (name, style, w, t) in [
        ("HalfRound", ProfileStyle::HalfRound, 6.0, 3.0),
        ("LowDome", ProfileStyle::LowDome, 6.0, 3.0),
        ("Flat", ProfileStyle::Flat, 6.0, 3.0),
        ("Flat", ProfileStyle::Flat, 7.0, 5.0),
        ("Beveled", ProfileStyle::Beveled, 6.0, 2.0),
        ("DShape", ProfileStyle::DShape, 5.0, 2.0),
    ] {
        let mut d = RingDesign::default();
        d.profile.apply_style(style);
        d.profile.width_mm = w;
        d.profile.thickness_mm = t;
        let ctx = d.field_context();
        let face = ctx
            .side_faces_std()
            .and_then(|sf| sf.wider())
            .map(|(lo, hi)| 0.5 * (lo + hi));
        match face {
            Some(v) => println!(
                "{name:<14} {:>7} {:>9.3} {:>9.3} {:>9.4}",
                format!("{w}x{t}"),
                ctx.crest_radius_mm,
                ctx.crest_radius_mm * ctx.arc_scale(v),
                ctx.arc_scale(v)
            ),
            None => println!("{name:<14} {:>7} {:>9.3} {:>9} {:>9}", format!("{w}x{t}"), ctx.crest_radius_mm, "—", "—"),
        }
    }
    println!(
        "\nk = r(v)/r_crest. A bridge the chart calls 0.55 mm is 0.55k of metal, \
         and k runs 0.80-0.83 on the faces the doctrine sends all ornament to."
    );
}
