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
pub fn face_items(feature: Id, face: u32) -> Vec<MenuItem> {
    STONES
        .iter()
        .map(|s| {
            MenuItem::new(s.label, Icon::Stones, MenuAction::AddStoneOnFace { feature, face, key: s.key }, "A reference stone seated on this face where the click landed, its culet clear of it; it rides the part when the part moves or grows")
                .under("Add stone here")
        })
        .collect()
}

#[cfg(test)]
mod face_tests {
    use super::*;

    #[test]
    fn a_face_offers_every_stone_preset_under_add_stone_here() {
        let items = face_items(4, 2);
        assert_eq!(items.len(), STONES.len());
        assert!(items.iter().all(|i| i.submenu == Some("Add stone here") && i.enabled && i.icon == Icon::Stones));
        assert_eq!(items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), band_items(0.0, 0.0).iter().map(|i| i.label.as_str()).collect::<Vec<_>>());
        assert_eq!(items[3].action, MenuAction::AddStoneOnFace { feature: 4, face: 2, key: "princess-5" });
    }

    #[test]
    fn a_parts_face_menu_carries_the_stones_between_the_sketch_and_press_pull() {
        use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation};
        use ringdesign_core::interaction::pick::{Entity, Pick};
        let mut d = ringdesign_core::RingDesign::default();
        let mut doc = Document::default();
        let mut add = |id, operation, component| doc.append(Feature { id, name: format!("Part {id}"), enabled: true, operation, component }).unwrap();
        add(0, Operation::Band, Component::default());
        add(3, Operation::Cylinder { radius_mm: 2.0, height_mm: 2.0 }, Component { attach: Attach::Join, ..Default::default() });
        add(4, Operation::Sphere { radius_mm: 1.0 }, Component { reference: true, ..Default::default() });
        d.cad = Some(doc);
        let face = |feature| Pick { entity: Entity::Face { feature, face: 1 }, world: [0.0, 10.0, 0.0], normal: [0.0, 1.0, 0.0], depth: 1.0, px: 0.0 };
        let sel = super::super::selection::Selection::default();
        let items = super::super::menu::context_items(&sel, Some(&face(3)), &d);
        let n = STONES.len();
        assert_eq!(items[0].action, MenuAction::SketchOnFace { feature: 3, face: 1 });
        assert!(items[1..=n].iter().all(|i| i.submenu == Some("Add stone here") && matches!(i.action, MenuAction::AddStoneOnFace { feature: 3, face: 1, .. })));
        assert_eq!(items[1 + n].action, MenuAction::PressPull { feature: 3, face: 1 });
        assert_eq!(items.len(), 17 + n);
        let on_stone = super::super::menu::context_items(&sel, Some(&face(4)), &d);
        assert!(!on_stone.iter().any(|i| matches!(i.action, MenuAction::AddStoneOnFace { .. })), "a reference stone's face seats no stone");
    }
}
