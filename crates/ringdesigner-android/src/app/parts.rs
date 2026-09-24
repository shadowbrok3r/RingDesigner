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
        self.cad.isolated.clear();
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
                    let added = self.cad_edit(&edits, then);
                    self.cad.edit_landed(added.is_some());
                    if let Some(added) = added {
                        self.isolate_made(&added);
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
                Request::Isolate(Some(id)) => self.isolate(id),
                Request::Isolate(None) => {
                    self.cad.isolated.clear();
                    self.request_view_update();
                    self.status = "The whole ring again".into();
                }
                Request::TakeOut(id) => {
                    self.cad.isolated.retain(|x| *x != id);
                    self.request_view_update();
                    self.status = match self.cad.isolated.len() {
                        0 => "The whole ring again".into(),
                        n => format!("{} taken out of the view; {n} part{} still shown alone", self.feature_name(id), if n == 1 { "" } else { "s" }),
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
                Request::Stamp { index, edit } => {
                    self.history.commit(&self.design);
                    match ringdesign_workbench::viewport::made::edit(&mut self.design, index, &edit) {
                        Ok(label) => {
                            if edit == ringdesign_workbench::viewport::StampEdit::Delete {
                                self.cad.clear();
                                self.stamp_window = None;
                            }
                            self.history.commit_as(&self.design, &label);
                            self.mark_dirty();
                            self.status = label;
                        }
                        Err(why) => self.status = why,
                    }
                }
                Request::SeatLayer(path) => {
                    self.selected_layer = path.first().copied();
                    self.editor.sheet = Some(crate::editor::Sheet::Layers);
                    self.status = ringdesign_workbench::viewport::selection::entry_at(&self.design, &path)
                        .map_or_else(|| "Its layer".into(), |e| format!("Layer \"{}\" chosen", e.name));
                }
                Request::LiveCuts => {
                    self.cuts.live = !self.cuts.live;
                    self.save_prefs();
                    self.mark_dirty();
                }
                Request::Cutters => {
                    self.cuts.ghost = !self.cuts.ghost;
                    self.save_prefs();
                    self.mark_dirty();
                }
                Request::EditStamp(index) => self.stamp_window = Some(index),
                Request::Pins(pins) => {
                    self.history.commit(&self.design);
                    self.design.pins = pins;
                    self.history.commit_as(&self.design, "Pins");
                    self.autosave();
                }
            }
        }
    }

    /// Part `id` added to the parts shown alone on the ring.
    fn isolate(&mut self, id: u64) {
        if self.editor.isolate {
            self.editor.isolate = false;
        }
        if !self.cad.isolated.contains(&id) {
            self.cad.isolated.push(id);
        }
        self.request_view_update();
        self.status = match self.cad.isolated.len() {
            1 => format!("{} shown alone: the verdict still reads the whole ring; Show the whole ring or Clear brings it back", self.feature_name(id)),
            n => format!("{} shown alone beside {} other part{}", self.feature_name(id), n - 1, if n == 2 { "" } else { "s" }),
        };
    }

    /// Parts an edit `added` join the parts shown alone, in place of any they replace.
    pub(super) fn isolate_made(&mut self, added: &[u64]) {
        let Some(doc) = self.design.cad.as_ref() else { return };
        let next = ringdesign_workbench::touch::isolate::after_edit(&self.cad.isolated, doc, added);
        if next != self.cad.isolated {
            self.cad.isolated = next;
            self.request_view_update();
        }
    }

    /// The one road a CAD edit takes on the phone: the funnel as one undo step, then the choice it asks for; the features it added, `None` when nothing landed.
    pub(super) fn cad_edit(&mut self, edits: &[CadEdit], then: Then) -> Option<Vec<u64>> {
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
                Some(done.applied.iter().filter_map(|a| a.id).collect())
            }
            Ok(None) => None,
            Err(why) => {
                self.status = why;
                None
            }
        }
    }

    /// The stamp chosen for editing, in a window over the ring: a settled change is one History entry.
    pub(super) fn stamp_window(&mut self, ctx: &egui::Context) {
        let Some(i) = self.stamp_window else { return };
        let Some(mut stamp) = self.design.stamps.get(i).cloned() else {
            self.stamp_window = None;
            return;
        };
        let mut open = true;
        let mut read = ringdesign_workbench::viewport::made::Inspected::default();
        egui::Window::new("Stamp").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            read = ringdesign_workbench::viewport::made::inspector(ui, &mut stamp);
        });
        if read.changed {
            self.design.stamps[i] = stamp;
            self.mark_dirty();
        }
        if read.settled {
            self.history.commit_as(&self.design, &format!("Edit stamp \"{}\"", self.design.stamps[i].name));
        }
        if !open {
            self.stamp_window = None;
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
                strip::Routed::Isolate(id) => self.isolate(id),
                strip::Routed::Commit(edits) => {
                    if let Some(added) = self.cad_edit(&edits, Then::Keep) {
                        self.isolate_made(&added);
                    }
                }
                strip::Routed::Say(text) => self.status = text,
            }
        }
    }
}
