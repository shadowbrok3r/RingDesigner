//! Sketching in the Ring viewport: a sketch drawn on the face or plane it lies on, with the ring as its underlay.
use crate::app::RingDesignerApp;
use crate::camera::Projector;
use crate::command::Took;

/// What the Ring viewport's sketch mode holds between frames.
#[derive(Default)]
pub struct SketchMode {}

impl SketchMode {
    /// Whether a sketch is live.
    pub fn is_live(&self) -> bool {
        false
    }
}

/// Whether a sketch is being drawn in the Ring viewport.
pub fn active(app: &RingDesignerApp) -> bool {
    app.sketch.is_live()
}

/// Starts a sketch on a planar face of a part.
pub fn start_on_face(app: &mut RingDesignerApp, _pane: usize, _feature: u64, _face: u32) {
    app.set_status("Sketching on a face arrives with sketch mode");
}

/// Starts a sketch on a plane through a point of the band.
pub fn start_on_plane(app: &mut RingDesignerApp, _pane: usize, _theta_deg: f64, _across_mm: f64) {
    app.set_status("Sketching on a plane arrives with sketch mode");
}

/// Takes the pointer and keys while a sketch is live, before the command layer sees them.
pub fn input(_app: &mut RingDesignerApp, _ui: &mut egui::Ui, _pane: usize, _rect: egui::Rect, _response: &egui::Response) -> Took {
    Took::default()
}

/// Draws the live sketch over the metal.
pub fn draw(_app: &mut RingDesignerApp, _ui: &mut egui::Ui, _pane: usize, _response: &egui::Response, _painter: &egui::Painter, _proj: &Projector, _active: bool) {}
