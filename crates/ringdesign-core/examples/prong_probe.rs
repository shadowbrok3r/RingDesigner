// Measures the shared-prong lean in sand at two resolutions.
use ringdesign_core::field::{Layer, LayerEntry, SeatRunLayer};
use ringdesign_core::gem::{Gem, GemCut};
use ringdesign_core::{castability, RingDesign};

fn main() {
    let mut d = RingDesign::default();
    d.profile.apply_style(ringdesign_core::ProfileStyle::LowDome);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.5;
    let ctx = d.field_context();
    let mut run = SeatRunLayer::default();
    run.gem = Gem::calibrated(GemCut::Round, 2.2);
    run.seat.v_mm = ctx.crest_v_mm;
    run.solve_spacing(&ctx);
    run.shared_prong_mm = 0.9;
    println!("count {} post dia {:.2} off {:.2}", run.count, run.prong_r_mm()*2.0, run.prong_off_mm());
    d.layers.layers.push(LayerEntry::new("Shared", Layer::SeatRun(run)));
    let lib = ringdesign_core::alpha::AlphaLibrary::builtin();
    for (t, p) in [(256usize, 144usize), (512, 288), (1024, 512)] {
        let f = castability::analyze_field(&d, &lib, &d.draft, t, p);
        println!("{t}x{p}: {:?} undercut {:.3}% worst {:.1} deg", f.verdict, f.undercut_fraction()*100.0, f.worst_draft_deg);
    }
}
