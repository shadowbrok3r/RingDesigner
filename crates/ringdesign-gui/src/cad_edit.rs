//! The one road a CAD edit takes in the app: through the design's graph when it is driven, onto its document when it is plain.
use crate::app::RingDesignerApp;
use ringdesign_core::cad::edit::{Applied, CadEdit};

/// Applies every edit to a copy of the design and, only if all of them land, makes it the design as one undo step.
pub fn apply(app: &mut RingDesignerApp, edits: &[CadEdit]) -> Result<Vec<Applied>, String> {
    if edits.is_empty() {
        return Ok(Vec::new());
    }
    let mut next = app.design.clone();
    let mut applied = Vec::with_capacity(edits.len());
    for edit in edits {
        match ringdesign_graph::nodes::cad::edit_design(&mut next, edit) {
            Ok(a) => applied.push(a),
            Err(e) => {
                let message = format!("{e:#}");
                app.set_status(message.clone());
                return Err(message);
            }
        }
    }
    app.history.commit(&app.design);
    app.design = next;
    if app.design.graph.is_some() {
        app.sync_graph();
    }
    app.mark_dirty();
    app.history.commit(&app.design);
    app.set_status(applied.iter().map(|a| a.label.as_str()).collect::<Vec<_>>().join(" · "));
    Ok(applied)
}
