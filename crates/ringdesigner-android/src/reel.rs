//! A showcase reel played on the real UI: a design's own construction replayed step by step — bare
//! stock, each layer switching on in stack order under its name, the stones, their cutters ghosted, the
//! live cut, a closing spin and a flip — so recording one is a screen recorder round a single tap, with
//! no finger to script. The plan is plain data and built here, where it can be tested off the device.
use ringdesign_core::RingDesign;

/// What one step of a reel shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Beat {
    /// The design with only its first `layers` entries switched on.
    Build { layers: usize },
    /// Every layer on, stones shown or hidden, cutters ghosted or not, cuts resolved or not.
    Finish { stones: bool, ghost: bool, live: bool },
    /// A whole turn about the finger, then the ring stood on its head and back.
    Spin,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub beat: Beat,
    pub caption: String,
    /// Seconds on screen once the build behind it has landed.
    pub hold_s: f32,
    /// `[yaw, pitch]` to ease to, from the head's own angle; `None` keeps the view.
    pub view: Option<[f32; 2]>,
}

/// The reel for a design: its stack replayed in order, then its settings, then a turn.
pub fn plan(design: &RingDesign) -> Vec<Step> {
    // The cameras' own angles: yaw about the finger from the head, pitch toward the finger's axis, so the
    // face is looked at from `[head, 0]` and a three-quarter view stands off to one side and a little over.
    let head = (design.shank.head.theta_deg as f32).to_radians();
    let views = [[head - 0.48, -0.55], [head, 0.0], [head + 0.95, -0.35], [head - 1.05, -0.42], [head + 0.35, -0.85]];
    let enabled: Vec<usize> = design.layers.layers.iter().enumerate().filter(|(_, e)| e.enabled).map(|(i, _)| i).collect();
    let stock = if design.imported_base.is_some() { "The stock, bare" } else { "The band, bare" };
    let mut steps = vec![Step { beat: Beat::Build { layers: 0 }, caption: stock.into(), hold_s: 2.2, view: Some(views[0]) }];
    for (k, &i) in enabled.iter().enumerate() {
        let entry = &design.layers.layers[i];
        let stage = if entry.bench_only { " — cut at the bench" } else { "" };
        steps.push(Step { beat: Beat::Build { layers: i + 1 }, caption: format!("{}{stage}", entry.name), hold_s: 2.4, view: Some(views[(k + 1) % views.len()]) });
    }
    let made = ringdesign_core::setting::any(design);
    let stones = !ringdesign_core::setstone::set_stones(design).is_empty();
    if made {
        steps.push(Step { beat: Beat::Finish { stones: false, ghost: true, live: false }, caption: "The setting burs, ghosted where they will cut".into(), hold_s: 3.0, view: Some(views[1]) });
        steps.push(Step { beat: Beat::Finish { stones: false, ghost: false, live: true }, caption: "Seats cut: bevel, girdle wall, bearing, pilot".into(), hold_s: 3.0, view: None });
    }
    if stones {
        steps.push(Step { beat: Beat::Finish { stones: true, ghost: false, live: true }, caption: "Stones set".into(), hold_s: 2.4, view: Some(views[0]) });
    }
    steps.push(Step { beat: Beat::Spin, caption: design.name.clone(), hold_s: 7.0, view: Some(views[0]) });
    steps
}

/// The design a build step shows: a copy with the stack cut off after `layers` entries and no graph to
/// put them back.
pub fn built_to(full: &RingDesign, layers: usize) -> RingDesign {
    let mut d = full.clone();
    d.graph = None;
    for (i, e) in d.layers.layers.iter_mut().enumerate() {
        e.enabled &= i < layers;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::field::{Layer, LayerEntry, MilgrainLayer, SeatPadLayer};

    #[test]
    fn a_reel_replays_the_stack_in_order_then_the_settings_then_a_turn() {
        let mut d = RingDesign::default();
        d.name = "Test ring".into();
        d.layers.layers.push(LayerEntry::new("Beads", Layer::Milgrain(MilgrainLayer::default())));
        let mut off = LayerEntry::new("Hidden", Layer::Milgrain(MilgrainLayer::default()));
        off.enabled = false;
        d.layers.layers.push(off);
        let mut seat = SeatPadLayer { solid: ringdesign_core::setting::SolidKind::Flush, ..Default::default() };
        seat.fit_stone(ringdesign_core::gem::Gem::default());
        d.layers.layers.push(LayerEntry::new("Stone", Layer::SeatPad(seat)));
        let steps = plan(&d);
        let captions: Vec<&str> = steps.iter().map(|s| s.caption.as_str()).collect();
        assert_eq!(captions[..3], ["The band, bare", "Beads", "Stone"], "disabled layers are not part of the story");
        assert!(matches!(steps[1].beat, Beat::Build { layers: 1 }) && matches!(steps[2].beat, Beat::Build { layers: 3 }));
        assert!(steps.iter().any(|s| s.beat == Beat::Finish { stones: false, ghost: true, live: false }), "the burs are shown before they cut");
        assert_eq!(steps.last().unwrap().beat, Beat::Spin);
        assert_eq!(steps.last().unwrap().caption, "Test ring");
        // A build step never shows more than it says, and leaves the source alone.
        let bare = built_to(&d, 0);
        assert!(bare.layers.layers.iter().all(|e| !e.enabled) && d.layers.layers[0].enabled);
        assert!(built_to(&d, 3).layers.layers[2].enabled && !built_to(&d, 3).layers.layers[1].enabled);
    }
}
