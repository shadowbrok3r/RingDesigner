//! Patterns of a part and press-pull, from the Ring viewport's right-click.
use crate::app::RingDesignerApp;

/// Starts the pattern named by `key` on a part: an array round the ring or round its stone, or a mirror.
pub fn start(app: &mut RingDesignerApp, _pane: usize, _feature: u64, _key: &'static str) {
    app.set_status("Patterns arrive with M9");
}

/// Starts pushing or pulling a planar face of a part along its normal.
pub fn press_pull(app: &mut RingDesignerApp, _pane: usize, _feature: u64, _face: u32) {
    app.set_status("Press-pull arrives with M9");
}
