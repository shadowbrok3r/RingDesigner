//! What a right-click on a part offers for patterns and press-pull.
use super::menu::{MenuAction, MenuItem};
use crate::icons::Icon;
use ringdesign_core::sketch::Id;

/// Copies of the part round the finger, the count typed.
pub const RING_ARRAY: &str = "ring-array";
/// Copies of the part round the stone it stands on or by, the count typed.
pub const STONE_ARRAY: &str = "stone-array";
/// The part reflected across the band's mid-plane.
pub const MIRROR_BAND: &str = "mirror-band";
/// The part reflected through the plane of the finger's axis and the head.
pub const MIRROR_HEAD: &str = "mirror-head";
/// Every pattern the menu offers, in its order.
pub const KEYS: [&str; 4] = [RING_ARRAY, STONE_ARRAY, MIRROR_BAND, MIRROR_HEAD];
/// The submenu the patterns fold into.
pub const SUBMENU: &str = "Pattern";

/// Press-pull for the face under the click, then the patterns for the part; nothing for a reference stone, which its setting carries.
pub fn part_items(feature: Id, face: Option<u32>, reference: bool) -> Vec<MenuItem> {
    if reference {
        return Vec::new();
    }
    let mut items = Vec::new();
    if let Some(face) = face {
        items.push(MenuItem::new("Press-pull", Icon::Raise, MenuAction::PressPull { feature, face }, "Push or pull this face along its normal; drag or type the distance"));
    }
    for (key, label, icon, hint) in [
        (RING_ARRAY, "Array round the ring…", Icon::Pattern, "Copies of the part round the finger, each dropped onto the band at its own angle; type how many"),
        (STONE_ARRAY, "Array round its stone…", Icon::Duplicate, "Copies of the part round the stone it stands on or by, about the stone's axis; type how many"),
        (MIRROR_BAND, "Mirror across the band", Icon::Mirror, "The part reflected across the band's mid-plane, onto its other edge"),
        (MIRROR_HEAD, "Mirror through the head", Icon::Mirror, "The part reflected through the plane of the finger's axis and the head, onto the head's other side"),
    ] {
        items.push(MenuItem::new(label, icon, MenuAction::Pattern { feature, key }, hint).under(SUBMENU));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_offers_its_patterns_in_one_submenu_a_face_press_pull_first_and_a_stone_neither() {
        let items = part_items(3, None, false);
        let rows: Vec<(Option<&str>, &str, MenuAction)> = items.iter().map(|i| (i.submenu, i.label.as_str(), i.action.clone())).collect();
        assert_eq!(
            rows,
            [
                (Some(SUBMENU), "Array round the ring…", MenuAction::Pattern { feature: 3, key: RING_ARRAY }),
                (Some(SUBMENU), "Array round its stone…", MenuAction::Pattern { feature: 3, key: STONE_ARRAY }),
                (Some(SUBMENU), "Mirror across the band", MenuAction::Pattern { feature: 3, key: MIRROR_BAND }),
                (Some(SUBMENU), "Mirror through the head", MenuAction::Pattern { feature: 3, key: MIRROR_HEAD }),
            ]
        );
        assert!(items.iter().all(|i| i.enabled && !i.hint.is_empty()));
        let on_face = part_items(3, Some(4), false);
        assert_eq!((on_face.len(), &on_face[0].action, on_face[0].submenu, on_face[0].icon), (5, &MenuAction::PressPull { feature: 3, face: 4 }, None, Icon::Raise));
        assert!(part_items(3, Some(4), true).is_empty(), "a reference stone is patterned with its setting, never alone");
        let keys: Vec<&str> = items.iter().filter_map(|i| match i.action { MenuAction::Pattern { key, .. } => Some(key), _ => None }).collect();
        assert_eq!(keys, KEYS);
    }
}
