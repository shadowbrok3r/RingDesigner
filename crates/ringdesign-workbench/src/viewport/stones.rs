//! What a right-click offers for stones: a stone added where the band was clicked, and a made setting round one.
use super::menu::MenuItem;
use ringdesign_core::sketch::Id;

/// Stones to seat at a point of the band.
pub fn band_items(_theta_deg: f64, _height_mm: f64) -> Vec<MenuItem> {
    Vec::new()
}

/// Settings to build round a reference stone part or a height-field stone.
pub fn setting_items(_part: Option<Id>, _stone: Option<Vec<usize>>) -> Vec<MenuItem> {
    Vec::new()
}
