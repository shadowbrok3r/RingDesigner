//! The ring view's CAD layer served: its requests through the funnel as one undo step each, its strip and its menu button.
use super::*;
use crate::cad::{Request, Then, strip};
use ringdesign_core::cad::edit::CadEdit;

impl RingApp {
    /// Ends a live command and a sketch, lets go of the CAD choice and menu and shows the whole ring, as a new design or a history step does.
    pub(super) fn end_cad(&mut self) {
        let mut said = Vec::new();
        self.cad.live.cancel(&mut said);
        self.cad.clear();
        self.cad.drop_sketch();
        self.cad.isolated = None;
    }

    /// A feature's name as the document holds it.
    fn feature_name(&self, id: u64) -> String {
        self.design.cad.as_ref().and_then(|d| d.feature(id)).map_or_else(|| format!("#{id}"), |f| f.name.clone())
    }

    /// Serves what the CAD layer asked for this frame.
    pub(super) fn serve_cad(&mut self, host: &Host) {
        for r in self.cad.take_requests() {
            match r {
                Request::Edit { edits, then } => {
                    let ok = self.cad_edit(&edits, then);
                    let alone = self.cad.isolated;
                    self.cad.edit_landed(ok);
                    // The ring comes back round a sketch's new solid under the part's own framing.
                    if alone.is_some() && self.cad.isolated.is_none() {
                        self.shown_alone = None;
                    }
                    if ok {
                        host.haptic(Haptic::Success);
                    }
                }
                Request::Look(pose) => {
                    self.camera_turn = Some(crate::focus::Turn::new(self.pane.camera.pose(), pose));
                    self.pane.actual_size = false;
                }
                Request::Pan { by, rect } => {
                    self.camera_turn = None;
                    self.pane.camera.pan_by(by, rect);
                }
                Request::Isolate(part) => {
                    if self.editor.isolate {
                        self.editor.isolate = false;
                    }
                    self.cad.isolated = part;
                    self.request_view_update();
                    self.status = match part {
                        Some(id) => format!("{} shown alone: the verdict still reads the whole ring; Show the whole ring or Clear brings it back", self.feature_name(id)),
                        None => "The whole ring again".into(),
                    };
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
                Request::Pins(pins) => {
                    self.history.commit(&self.design);
                    self.design.pins = pins;
                    self.history.commit_as(&self.design, "Pins");
                    self.autosave();
                }
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
                    self.cad.choose_made(&self.design, id);
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
                strip::Routed::Choose(id) => self.cad.choose_made(&self.design, id),
                // A sketch opens on the ring to draw on again; anything else on the Workshop's CAD tab.
                strip::Routed::Edit(id) if doc.feature(id).is_some_and(|f| matches!(f.operation, ringdesign_core::cad::Operation::Sketch { .. })) => {
                    self.cad.open_sketch_later(id);
                    self.status = format!("Opening {} on the ring", self.feature_name(id));
                }
                strip::Routed::Edit(id) => {
                    self.cad.choose(id);
                    self.status = format!("#{id}: edit its numbers on the Workshop's CAD tab");
                    self.tab = Tab::Workshop;
                }
                strip::Routed::Isolate(id) => {
                    self.cad.isolated = Some(id);
                    self.request_view_update();
                    self.status = format!("{} shown alone; Clear brings the whole ring back", self.feature_name(id));
                }
                strip::Routed::Commit(edits) => {
                    self.cad_edit(&edits, Then::Keep);
                }
                strip::Routed::Say(text) => self.status = text,
            }
        }
    }
}
