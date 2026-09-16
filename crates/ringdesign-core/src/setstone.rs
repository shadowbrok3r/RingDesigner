//! One record per stone the design sets — a pad, a run's station, a pavé
//! seat, a halo marker — read by the report's census, the gem preview and
//! the section view, so no two of them can disagree about where a stone is.

use crate::field::{wrap_delta, FieldContext, Layer, LayerEntry, LayerStack, SeatPadLayer, Uv};
use crate::gem::Gem;
use crate::RingDesign;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoneSource {
    /// A seat pad of its own.
    Pad,
    /// Station `station` of a seat run.
    Run { station: u32 },
}

/// A stone where the design sets it.
#[derive(Clone, Debug)]
pub struct SetStone {
    /// The entry's path, `"Halo / Centre"` for a nested layer.
    pub label: String,
    pub source: StoneSource,
    pub theta_deg: f64,
    pub v_mm: f64,
    pub gem: Gem,
    /// The seat as this stone meets it: a run's seat fitted to its gem and
    /// scaled with the grade.
    pub seat: SeatPadLayer,
}

impl SetStone {
    /// Height of the girdle over the bare band, mm.
    pub fn stand_off_mm(&self) -> f64 {
        self.seat.stand_off_mm(self.gem)
    }

    /// Bearing of the stone's long axis in the chart, degrees.
    pub fn rot_deg(&self) -> f64 {
        self.seat.rot_deg
    }

    /// How far the seat reaches round the ring at the crest radius, degrees.
    pub fn reach_deg(&self, ctx: &FieldContext) -> f64 {
        self.seat.chart_reach_mm(ctx).0 / ctx.crest_radius_mm.max(1e-9) * 180.0
            / std::f64::consts::PI
    }

    pub fn carats(&self) -> f64 {
        self.gem.carats()
    }
}

/// Whether a station survives its entry's window.
pub fn kept(entry: &LayerEntry, ctx: &FieldContext, theta_deg: f64, v_mm: f64) -> bool {
    let uv = Uv { u: ctx.u_of_theta(theta_deg.rem_euclid(360.0)), v: v_mm };
    entry.window.mask(uv, ctx) > 0.5
}

/// Every stone the design sets, in stack order, windows honoured. A seat
/// without a stone is stock, not a stone, and is not listed.
pub fn set_stones(design: &RingDesign) -> Vec<SetStone> {
    let ctx = design.field_context();
    let mut out = Vec::new();
    walk(&ctx, &design.layers, "", &mut out);
    out
}

/// The stones whose seat reaches a slice at `theta_deg`, never narrower
/// than `min_deg` either side.
pub fn stones_near<'a>(
    stones: &'a [SetStone],
    ctx: &FieldContext,
    theta_deg: f64,
    min_deg: f64,
) -> Vec<&'a SetStone> {
    stones
        .iter()
        .filter(|st| wrap_delta(theta_deg - st.theta_deg, 360.0).abs() <= st.reach_deg(ctx).max(min_deg))
        .collect()
}

fn walk(ctx: &FieldContext, stack: &LayerStack, prefix: &str, out: &mut Vec<SetStone>) {
    for entry in &stack.layers {
        if !entry.enabled {
            continue;
        }
        match &entry.layer {
            Layer::SeatPad(seat) => {
                if let Some(gem) = seat.gem {
                    if kept(entry, ctx, seat.theta_deg, seat.v_mm) {
                        out.push(SetStone {
                            label: format!("{prefix}{}", entry.name),
                            source: StoneSource::Pad,
                            theta_deg: seat.theta_deg,
                            v_mm: seat.v_mm,
                            gem,
                            seat: *seat,
                        });
                    }
                }
            }
            Layer::SeatRun(run) => {
                let n = run.count.clamp(1, 200);
                let mut fitted = run.seat;
                fitted.fit_stone(run.gem);
                let fitted = run.turned(fitted);
                for k in 0..n {
                    let theta = run.theta_of_station(k as f64, ctx);
                    if !kept(entry, ctx, theta, fitted.v_mm) {
                        continue;
                    }
                    // The field scales the whole seat with its stone.
                    let mut seat = fitted;
                    seat.height_mm *= run.scale_at(theta);
                    out.push(SetStone {
                        label: format!("{prefix}{}", entry.name),
                        source: StoneSource::Run { station: k },
                        theta_deg: theta,
                        v_mm: fitted.v_mm,
                        gem: run.gem_at(theta),
                        seat,
                    });
                }
            }
            Layer::Group(g) => walk(ctx, &g.stack, &format!("{prefix}{} / ", entry.name), out),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{GroupLayer, SeatRunLayer, Window};
    use crate::gem::GemCut;

    fn pad(theta: f64, v: f64, gem: Option<Gem>) -> SeatPadLayer {
        let mut s = SeatPadLayer::default();
        s.theta_deg = theta;
        s.v_mm = v;
        s.gem = gem;
        if let Some(g) = gem {
            s.fit_stone(g);
        }
        s
    }

    fn design() -> (RingDesign, usize) {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let v = ctx.band_v_len_mm * 0.5;
        let round = |mm: f64| Gem::calibrated(GemCut::Round, mm);
        d.layers.layers.push(LayerEntry::new("Centre", Layer::SeatPad(pad(90.0, v, Some(round(4.0))))));
        d.layers.layers.push(LayerEntry::new("Blank stock", Layer::SeatPad(pad(270.0, v, None))));
        let mut off = LayerEntry::new("Off", Layer::SeatPad(pad(180.0, v, Some(round(2.0)))));
        off.enabled = false;
        d.layers.layers.push(off);
        let mut run = SeatRunLayer::default();
        run.gem = round(1.5);
        run.seat.v_mm = v;
        run.count = 20;
        run.solve_spacing(&ctx);
        let mut row = LayerEntry::new("Row", Layer::SeatRun(run));
        row.window = Window::around(270.0, 120.0);
        let kept_stations = (0..20)
            .filter(|&k| {
                let t = run.theta_of_station(k as f64, &ctx);
                kept(&row, &ctx, t, v)
            })
            .count();
        d.layers.layers.push(row);
        let mut g = GroupLayer::default();
        g.stack.layers.push(LayerEntry::new("Seat 1", Layer::SeatPad(pad(30.0, v, Some(round(1.2))))));
        g.stack.layers.push(LayerEntry::new("Seat 2", Layer::SeatPad(pad(40.0, v, Some(round(1.2))))));
        g.stack.layers.push(LayerEntry::new("Marker", Layer::SeatPad(pad(50.0, v, None))));
        d.layers.layers.push(LayerEntry::new("Pavé", Layer::Group(g)));
        (d, 1 + kept_stations + 2)
    }

    #[test]
    fn every_consumer_counts_the_same_stones() {
        let (d, expected) = design();
        let stones = set_stones(&d);
        assert_eq!(stones.len(), expected);
        assert!(expected > 4, "the window keeps some stations: {expected}");
        assert!(stones.iter().any(|s| s.label == "Pavé / Seat 2"));
        assert!(stones.iter().all(|s| s.label != "Off" && s.label != "Blank stock"));
        let lib = crate::AlphaLibrary::builtin();
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert_eq!(report.stone_count as usize, stones.len(), "the report counts the record's stones");
        assert!((report.total_carats - stones.iter().map(|s| s.carats()).sum::<f64>()).abs() < 1e-9);
        let mesh = crate::gems::preview_mesh(&d, &lib).unwrap();
        assert_eq!(mesh.faces.len() % stones.len(), 0, "every stone draws the same facet count");
    }

    #[test]
    fn a_graded_run_records_each_stone_at_its_own_size() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.0);
        run.seat.v_mm = ctx.band_v_len_mm * 0.5;
        run.taper = 0.6;
        run.taper_theta_deg = 90.0;
        run.solve_spacing(&ctx);
        d.layers.layers.push(LayerEntry::new("Graded", Layer::SeatRun(run)));
        let stones = set_stones(&d);
        let big = stones.iter().max_by(|a, b| a.gem.w_mm.total_cmp(&b.gem.w_mm)).unwrap();
        let small = stones.iter().min_by(|a, b| a.gem.w_mm.total_cmp(&b.gem.w_mm)).unwrap();
        assert!(wrap_delta(big.theta_deg - 90.0, 360.0).abs() < 20.0, "largest at the pole");
        assert!(small.gem.w_mm < big.gem.w_mm * 0.6);
        assert!(small.seat.height_mm < big.seat.height_mm, "the seat scales with its stone");
        assert!(matches!(small.source, StoneSource::Run { .. }));
    }

    #[test]
    fn stones_near_picks_the_slices_neighbours() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let v = ctx.band_v_len_mm * 0.5;
        let g = Gem::calibrated(GemCut::Round, 3.0);
        d.layers.layers.push(LayerEntry::new("Top", Layer::SeatPad(pad(90.0, v, Some(g)))));
        d.layers.layers.push(LayerEntry::new("Bottom", Layer::SeatPad(pad(270.0, v, Some(g)))));
        let stones = set_stones(&d);
        let near = stones_near(&stones, &ctx, 92.0, 2.0);
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].label, "Top");
        assert!(stones_near(&stones, &ctx, 180.0, 2.0).is_empty());
    }
}
