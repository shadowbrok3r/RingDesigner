//! The feature timeline under the Ring viewport and in the CAD pane.
use crate::{
    app::RingDesignerApp,
    panels::cad::{self, CadRequest},
};
use ringdesign_core::cad::Document;
use ringdesign_workbench::{
    icons::Icon,
    timeline::{self, Action},
    viewport::{Mods, Sel},
};

/// Whether the design carries CAD features for a strip to show.
pub fn shown(app: &RingDesignerApp) -> bool {
    app.design.cad.as_ref().is_some_and(|d| !d.features.is_empty())
}

/// The features the Ring viewport has chosen: a part, or a face, edge or vertex of one.
pub fn selected(app: &RingDesignerApp) -> Vec<u64> {
    app.selection.items.iter().filter_map(Sel::feature).collect()
}

/// The strip under the Ring viewport `pane`.
pub fn bar(app: &mut RingDesignerApp, ui: &mut egui::Ui, pane: usize) {
    let Some(doc) = app.design.cad.as_ref() else { return };
    // Statuses come only from a build of the design as it stands.
    let evaluated = if app.is_current() { app.build.as_ref().and_then(|b| b.parts.evaluated.as_ref()) } else { None };
    let chips = timeline::chips(doc, evaluated, &selected(app));
    let rollback = doc.through;
    let actions = ui
        .horizontal(|ui| {
            ui.add(Icon::History.image(ui, 16.0)).on_hover_text(
                "Feature timeline: click a feature to select it, double-click to edit it, drag to reorder it, right-click for the rest. Delete removes the chosen feature while the pointer is over the strip.",
            );
            egui::ScrollArea::horizontal()
                .id_salt(("viewport-timeline-scroll", pane))
                .auto_shrink([false, true])
                .show(ui, |ui| timeline::show(ui, &chips, rollback, false))
                .inner
        })
        .inner;
    if !actions.is_empty() {
        let doc = doc.clone();
        route(app, &doc, actions);
    }
}

/// Serves the strip's actions: selection and the CAD pane at once, every edit through the funnel as one undo step.
pub fn route(app: &mut RingDesignerApp, doc: &Document, actions: Vec<Action>) {
    let mut edits = Vec::new();
    for action in actions {
        match action {
            Action::Select(id) => app.selection.click(Some(Sel::Part(id)), Mods::default()),
            Action::Edit(id) => {
                // A sketch opens where it lies, in the Ring viewport; anything else in the CAD pane.
                let sketch = doc.feature(id).is_some_and(|f| matches!(f.operation, ringdesign_core::cad::Operation::Sketch { .. }));
                if !(sketch && crate::sketch_mode::start_in_ring(app, id)) {
                    app.selection.click(Some(Sel::Part(id)), Mods::default());
                    cad::ask(app, CadRequest::Select { feature: id });
                }
            }
            Action::Isolate(id) => cad::ask(app, CadRequest::Isolate { feature: id }),
            edit => match timeline::edits(doc, &edit) {
                Ok(e) => edits.extend(e),
                Err(reason) => {
                    app.set_status(reason);
                    return;
                }
            },
        }
    }
    if !edits.is_empty() {
        let _ = crate::cad_edit::apply(app, &edits);
    }
}
