//! The feature timeline under the Ring viewport and in the CAD pane.
use crate::app::RingDesignerApp;

/// Whether the design carries CAD features for a strip to show.
pub fn shown(_app: &RingDesignerApp) -> bool {
    false
}

/// The strip under the Ring viewport `pane`.
pub fn bar(_app: &mut RingDesignerApp, _ui: &mut egui::Ui, _pane: usize) {}
