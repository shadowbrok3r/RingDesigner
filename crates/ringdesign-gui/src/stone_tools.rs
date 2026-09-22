//! Stones and the settings built round them, from the Ring viewport's right-click.
use crate::app::RingDesignerApp;

/// Seats a reference stone named by `key` where the band was clicked.
pub fn add_stone(app: &mut RingDesignerApp, _theta_deg: f64, _height_mm: f64, _key: &'static str) {
    app.set_status("Stones arrive with the gem builders");
}

/// Builds the setting named by `key` round a reference stone part or a height-field stone.
pub fn setting(app: &mut RingDesignerApp, _part: Option<u64>, _stone: Option<Vec<usize>>, _key: &'static str) {
    app.set_status("Settings arrive with the gem builders");
}
