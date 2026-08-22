//! The metal between two stones, at the girdle and again at the culet.
//!
//! A row's bridge is measured where the girdles are. The ring's own
//! curvature closes the arc under them — pitch `p` at crest radius `r` is
//! only `p (r - t) / r` at depth `t` — and a stone with straight pavilion
//! walls keeps its full width the whole way down. So a row that clears at
//! the girdle can be short of metal at the keel, and the shallower the cut
//! the less it matters.
//!
//! Run: `cargo run --example crowd_probe`

use ringdesign_core::field::{Layer, LayerEntry, SeatRunLayer};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::RingDesign;

fn main() {
    println!("size-7 low dome, stones on the crest\n");
    println!(
        "{:<16} {:>5} {:>6} {:>7} {:>8} {:>8} {:>7}",
        "cut", "mm", "count", "pitch", "girdle", "culet", "loss"
    );
    for (cut, w) in [
        (GemCut::Round, 2.5),
        (GemCut::Princess, 2.5),
        (GemCut::Emerald, 2.5),
        (GemCut::Baguette, 2.0),
        (GemCut::Trillion, 2.5),
    ] {
        let gem = Gem::calibrated(cut, w);
        let mut d = RingDesign::default();
        d.profile.apply_style(ringdesign_core::ProfileStyle::LowDome);
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = gem;
        run.seat.v_mm = ctx.crest_v_mm;
        run.seat.fit_stone(gem);
        run.bridge_mm = 0.4;
        run.solve_spacing(&ctx);
        let count = run.count;
        let pitch = ctx.circumference_mm / count as f64;
        d.layers.layers.push(LayerEntry::new("row", Layer::SeatRun(run)));

        let r = ringdesign_core::stones::report(&d, 0.0).expect("a run is stones");
        let close = r.closest.expect("a run has neighbours");
        let (girdle, culet) = (close.gap_mm, close.gap_deep_mm);
        println!(
            "{:<16} {w:>5.1} {count:>6} {pitch:>7.3} {girdle:>8.3} {culet:>8.3} {:>7.3}",
            cut.label(),
            girdle - culet
        );
    }
    println!(
        "\nloss = pitch * pavilion / crest_radius, to within a hundredth. The \
         culet column holds each girdle's full width all the way down, which \
         is the truth for a step cut and pessimistic for a brilliant — and \
         step cuts are the population that gets set tight."
    );
}
