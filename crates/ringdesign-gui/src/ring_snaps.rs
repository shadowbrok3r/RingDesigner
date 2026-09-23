//! Pins on the ring: reference points the snaps and Measure read from.
use crate::app::RingDesignerApp;

/// Drops a pin where the band was clicked.
pub fn pin_here(app: &mut RingDesignerApp, _pane: usize, _world: [f64; 3]) {
    app.set_status("Pins arrive with the ring-aware snaps");
}

/// Takes every pin off the ring.
pub fn clear_pins(app: &mut RingDesignerApp) {
    app.set_status("Pins arrive with the ring-aware snaps");
}
