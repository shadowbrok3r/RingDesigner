//! What a right-click offers for cutters: a cutter placed on the band, and the builders made under a stone.
use super::menu::MenuItem;
use ringdesign_core::sketch::Id;

/// Cutters to place at a point of the band.
pub fn band_items(_theta_deg: f64, _across_mm: f64) -> Vec<MenuItem> {
    Vec::new()
}

/// Builders to make under reference stone `stone`.
pub fn stone_items(_stone: Id) -> Vec<MenuItem> {
    Vec::new()
}
