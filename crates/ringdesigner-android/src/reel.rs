//! A showcase reel played on the real UI: a design's own construction replayed step by step — bare
//! stock, each layer switching on in stack order under its name, each family of stamps struck, the
//! stones, their cutters ghosted, the live cut, a closing spin and a flip — so recording one is a screen
//! recorder round a single tap, with no finger to script. The plan is plain data and built here, where
//! it can be tested off the device.
use ringdesign_core::RingDesign;

/// What one step of a reel shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Beat {
    /// The design with only its first `layers` entries switched on.
    Build { layers: usize },
    /// Every layer on and the first `families` families of stamps struck, the seats not yet cut.
    Stamps { families: usize },
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
    for (j, family) in stamp_families(design).into_iter().enumerate() {
        let count = design.stamps.iter().filter(|s| stamp_family(&s.name) == family).count();
        let plural = if count == 1 { "" } else { "s" };
        steps.push(Step { beat: Beat::Stamps { families: j + 1 }, caption: format!("{family} — {count} stamp{plural}"), hold_s: 2.4, view: Some(views[(enabled.len() + j + 1) % views.len()]) });
    }
    let set = ringdesign_core::setstone::set_stones(design);
    let made = set.iter().any(|s| !s.seat.solid.is_none());
    let stones = !set.is_empty();
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

/// A stamp's family: its name up to the first comma or colon, less a trailing number.
pub fn stamp_family(name: &str) -> &str {
    let head = name.split([',', ':']).next().unwrap_or(name);
    let family = head.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace());
    if family.is_empty() { "Stamps" } else { family }
}

/// The design's stamp families in order of first appearance.
pub fn stamp_families(design: &RingDesign) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for s in &design.stamps {
        let f = stamp_family(&s.name);
        if !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// The design a stamp step shows: every layer, and only the stamps of its first `families` families.
pub fn struck_to(full: &RingDesign, families: usize) -> RingDesign {
    let mut d = built_to(full, usize::MAX);
    let order: Vec<String> = stamp_families(full).into_iter().map(str::to_owned).collect();
    d.stamps.retain(|s| order.iter().position(|f| f == stamp_family(&s.name)).is_some_and(|k| k < families));
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

    #[test]
    fn stamps_are_struck_family_by_family_and_burs_need_a_seat() {
        let stamp = |name: &str| ringdesign_core::setting::Stamp {
            name: name.into(),
            theta_deg: 90.0,
            v_mm: 1.0,
            rot_deg: 0.0,
            outline: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            height_mm: 0.3,
            sink_mm: 0.2,
            draft_deg: 0.0,
            cut: false,
            bench: false,
            along_pull: false,
            tier: 0,
            top: Default::default(),
        };
        let mut d = RingDesign::default();
        d.name = "Stamped".into();
        d.stamps = ["Crest, left 1", "Trail 1", "Crest, right 1", "Trail 12", "Waning: full moon"].map(stamp).to_vec();
        let steps = plan(&d);
        let captions: Vec<&str> = steps.iter().map(|s| s.caption.as_str()).collect();
        assert_eq!(captions, ["The band, bare", "Crest — 2 stamps", "Trail — 2 stamps", "Waning — 1 stamp", "Stamped"]);
        assert_eq!(steps[1].beat, Beat::Stamps { families: 1 });
        assert!(!steps.iter().any(|s| matches!(s.beat, Beat::Finish { .. })), "stamps alone have no burs and no stones");
        let names = |d: &RingDesign| d.stamps.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&struck_to(&d, 1)), ["Crest, left 1", "Crest, right 1"]);
        assert_eq!(struck_to(&d, 3).stamps.len(), 5);
        assert_eq!(stamp_family("12"), "Stamps");
    }
}
