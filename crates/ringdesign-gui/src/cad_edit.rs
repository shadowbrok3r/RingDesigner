//! The one road a CAD edit takes in the app: through the design's graph when it is driven, onto its document when it is plain.
use crate::app::RingDesignerApp;
use ringdesign_core::cad::edit::{Applied, CadEdit};

/// Applies every edit to a copy of the design and, only if all of them land, makes it the design as one undo step.
pub fn apply(app: &mut RingDesignerApp, edits: &[CadEdit]) -> Result<Vec<Applied>, String> {
    if edits.is_empty() {
        return Ok(Vec::new());
    }
    let mut next = app.design.clone();
    let mut edits = fresh_where_taken(&next, edits);
    // Bare face and edge references are signed in the frame each part was seated by.
    if let Some(e) = app.build.as_ref().and_then(|b| b.parts.evaluated.as_ref()) {
        for edit in &mut edits {
            let op = match edit {
                CadEdit::Add { feature, .. } => &mut feature.operation,
                CadEdit::Operation { operation, .. } => operation,
                _ => continue,
            };
            ringdesign_core::cad::sign_refs(op, e);
        }
    }
    let mut applied = Vec::with_capacity(edits.len());
    for edit in &edits {
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
    let label = applied.iter().map(|a| a.label.as_str()).collect::<Vec<_>>().join(" · ");
    app.history.commit_as(&app.design, &label);
    app.set_status(label);
    Ok(applied)
}

/// Adds whose asked-for id another node of a driven design's graph carries, asking for a fresh id instead.
fn fresh_where_taken(design: &ringdesign_core::RingDesign, edits: &[CadEdit]) -> Vec<CadEdit> {
    let mut edits = edits.to_vec();
    if !edits.iter().any(|e| matches!(e, CadEdit::Add { .. })) {
        return edits;
    }
    let Some(g) = design.graph.as_ref().and_then(|j| serde_json::from_value::<ringdesign_graph::graph::Graph>(j.clone()).ok()) else {
        return edits;
    };
    let features: std::collections::HashSet<u64> =
        ringdesign_graph::nodes::cad::document(&g).map(|d| d.features.iter().map(|f| f.id).collect()).unwrap_or_default();
    let mut taken: std::collections::HashSet<u64> = g.nodes.iter().map(|n| n.id.0).collect();
    // The graph applier adds a node at its next id for every add, then renames it to an asked-for id.
    let mut next_id = g.next_id;
    for edit in &mut edits {
        let CadEdit::Add { feature, .. } = edit else { continue };
        if feature.id != 0 && taken.contains(&feature.id) && !features.contains(&feature.id) {
            feature.id = 0;
        }
        if feature.id == 0 {
            taken.insert(next_id);
            next_id += 1;
        } else {
            taken.insert(feature.id);
            next_id = (next_id + 1).max(feature.id + 1);
        }
    }
    edits
}
