//! What a right-click offers for stones: a stone added where the band was clicked, and a made setting round one.
use super::menu::{MenuAction, MenuItem};
use crate::icons::Icon;
use ringdesign_core::cad::builders::{SETTINGS, STONES};
use ringdesign_core::sketch::Id;

/// Stones to seat at a point of the band.
pub fn band_items(theta_deg: f64, height_mm: f64) -> Vec<MenuItem> {
    STONES
        .iter()
        .map(|s| {
            MenuItem::new(s.label, Icon::Stones, MenuAction::AddStone { theta_deg, height_mm, key: s.key }, "A reference stone seated here, its culet clear of the metal: never metal itself, the settings are built round it")
                .under("Add stone here")
        })
        .collect()
}

/// Settings to build round a reference stone part or a height-field stone.
pub fn setting_items(part: Option<Id>, stone: Option<Vec<usize>>) -> Vec<MenuItem> {
    if part.is_none() && stone.is_none() {
        return Vec::new();
    }
    SETTINGS
        .iter()
        .map(|s| MenuItem::new(s.label, Icon::NodeHead, MenuAction::Setting { part, stone: stone.clone(), key: s.key }, s.hint).under("Setting"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_offers_every_stone_preset_and_a_stone_every_setting_each_under_its_submenu() {
        let items = band_items(90.0, 0.25);
        assert_eq!(items.len(), STONES.len());
        assert!(items.iter().all(|i| i.submenu == Some("Add stone here") && i.enabled && i.icon == Icon::Stones));
        assert_eq!(items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Round 5 mm", "Round 6.5 mm", "Oval 7 × 5", "Princess 5 mm", "Cushion 6 mm", "Emerald 7 × 5", "Pear 7 × 5", "Marquise 8 × 4"]);
        assert_eq!(items[1].action, MenuAction::AddStone { theta_deg: 90.0, height_mm: 0.25, key: "round-6.5" });
        let part = setting_items(Some(4), None);
        assert_eq!(part.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Four claws", "Six claws", "Bezel", "Basket", "Halo"]);
        assert!(part.iter().all(|i| i.submenu == Some("Setting") && i.enabled));
        assert_eq!(part[0].action, MenuAction::Setting { part: Some(4), stone: None, key: "claw4" });
        let seat = setting_items(None, Some(vec![2, 0]));
        assert_eq!(seat[2].action, MenuAction::Setting { part: None, stone: Some(vec![2, 0]), key: "bezel" });
        assert!(setting_items(None, None).is_empty(), "nothing to set");
    }
}

/// Stones to seat on a planar face of a part.
pub fn face_items(_feature: Id, _face: u32) -> Vec<MenuItem> {
    Vec::new()
}
