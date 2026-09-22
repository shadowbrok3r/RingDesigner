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
    // A driven design's document is what its graph evaluates to; carrying it now keeps the build's splice from reading as an edit of its own.
    if let Some(json) = &next.graph {
        let document = serde_json::from_value::<ringdesign_graph::graph::Graph>(json.clone())
            .map_err(|e| e.to_string())
            .and_then(|g| ringdesign_graph::nodes::cad::document(&g).map_err(|e| e.message));
        match document {
            Ok(doc) => next.cad = (!doc.features.is_empty()).then_some(doc),
            Err(message) => {
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
