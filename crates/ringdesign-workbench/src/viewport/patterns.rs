//! What a right-click on a part offers for patterns and press-pull.
use super::menu::MenuItem;
use ringdesign_core::sketch::Id;

/// Array, mirror and press-pull items for a part, and for the face under the click when there is one.
pub fn part_items(_feature: Id, _face: Option<u32>, _reference: bool) -> Vec<MenuItem> {
    Vec::new()
}
