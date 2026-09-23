//! The one road a CAD edit takes in the app: through the design's graph when it is driven, onto its document when it is plain.
use crate::app::RingDesignerApp;
use ringdesign_core::cad::edit::{Applied, CadEdit};
use ringdesign_workbench::touch::funnel;

/// Applies every edit to a copy of the design and, only if all of them land, makes it the design as one undo step.
pub fn apply(app: &mut RingDesignerApp, edits: &[CadEdit]) -> Result<Vec<Applied>, String> {
    // Fresh ids on a driven design, bare references signed in the frame each part was seated by, every edit on a copy.
    let evaluated = app.build.as_ref().and_then(|b| b.parts.evaluated.as_ref());
    let prepared = match funnel::prepare(&app.design, edits, evaluated) {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(Vec::new()),
        Err(message) => {
            app.set_status(message.clone());
            return Err(message);
        }
    };
    app.history.commit(&app.design);
    app.design = prepared.design;
    if app.design.graph.is_some() {
        app.sync_graph();
    }
    app.mark_dirty();
    app.history.commit_as(&app.design, &prepared.label);
    app.set_status(prepared.label);
    Ok(prepared.applied)
}
