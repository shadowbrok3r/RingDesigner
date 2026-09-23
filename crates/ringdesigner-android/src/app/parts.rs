//! The ring view's CAD layer served: its requests through the funnel as one undo step each, its strip and its menu button.
use super::*;
use crate::cad::{Request, Then, strip};
use ringdesign_core::cad::edit::CadEdit;

impl RingApp {
    /// Ends a live command and lets go of the CAD choice and menu, as a new design or a history step does.
    pub(super) fn end_cad(&mut self) {
        let mut said = Vec::new();
        self.cad.live.cancel(&mut said);
        self.cad.clear();
    }

    /// Serves what the CAD layer asked for this frame.
    pub(super) fn serve_cad(&mut self, host: &Host) {
        for r in self.cad.take_requests() {
            match r {
                Request::Edit { edits, then } => {
                    if self.cad_edit(&edits, then) {
                        host.haptic(Haptic::Success);
                    }
                }
                Request::Status(text) => self.status = text,
                Request::FitView => {
                    if let Some(mesh) = &self.preview_mesh {
                        self.pane.camera.fit(mesh.bounds());
                    }
                    self.pane.camera.zoom = 1.0;
                    self.pane.camera.pan = [0.0; 2];
                    self.pane.actual_size = false;
                }
                Request::ToggleWire => {
                    self.pane.wireframe = !self.pane.wireframe;
                    self.save_prefs();
                }
                Request::OpenWorkshop => {
                    if self.editor.isolate {
                        self.editor.isolate = false;
                        self.request_view_update();
                    }
                    self.tab = Tab::Workshop;
                }
                Request::EndMeasure => {
                    self.visual.select(ringdesign_workbench::visual::Tool::Select);
                    self.status = "Measure put away".into();
                }
                Request::Prefs => self.save_prefs(),
            }
        }
    }

    /// The one road a CAD edit takes on the phone: the funnel as one undo step, then the choice it asks for.
    fn cad_edit(&mut self, edits: &[CadEdit], then: Then) -> bool {
        let evaluated = self.preview_mesh.as_ref().and_then(|b| b.evaluated());
        match crate::cad::commit(&mut self.design, &mut self.history, edits, evaluated) {
            Ok(Some(done)) => {
                self.graph.sync(&self.design);
                self.mark_dirty();
                let chosen = match then {
                    Then::Keep => None,
                    Then::LastAdded => done.applied.iter().rev().find_map(|a| a.id),
                    Then::Part(id) => Some(id),
                };
                if let Some(id) = chosen.filter(|id| self.design.cad.as_ref().is_some_and(|d| d.feature(*id).is_some())) {
                    self.cad.choose(id);
                }
                self.status = done.label;
                true
            }
            Ok(None) => false,
            Err(why) => {
                self.status = why;
                false
            }
        }
    }

    /// The feature strip under the ring, when the design has features.
    pub(super) fn feature_strip(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.design.cad.as_ref().filter(|d| !d.features.is_empty()).cloned() else { return };
        // Statuses come only from a build of the design as it stands.
        let current = self.dirty_at.is_none() && !self.preview_in_flight;
        let evaluated = self.preview_mesh.as_ref().and_then(|b| b.evaluated()).filter(|_| current);
        let chosen: Vec<u64> = self.cad.selection.items.iter().filter_map(|s| s.feature()).collect();
        let actions = strip::show(ui, &doc, evaluated, &chosen);
        for routed in strip::route(&doc, actions) {
            match routed {
                strip::Routed::Choose(id) => self.cad.choose(id),
                strip::Routed::Edit(id) => {
                    self.cad.choose(id);
                    self.status = format!("#{id}: edit its numbers on the Workshop's CAD tab");
                    self.tab = Tab::Workshop;
                }
                strip::Routed::Commit(edits) => {
                    self.cad_edit(&edits, Then::Keep);
                }
                strip::Routed::Say(text) => self.status = text,
            }
        }
    }
}
