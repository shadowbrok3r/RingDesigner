//! The platform-free half of the CAD edit funnel: fresh ids on a driven design, references signed, every edit on a copy.
use ringdesign_core::{
    RingDesign,
    cad::{
        Evaluated,
        edit::{Applied, CadEdit},
    },
};
use ringdesign_graph::{graph::Graph, nodes::cad};
use std::collections::HashSet;

/// A copy of the design every edit landed on.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub design: RingDesign,
    pub applied: Vec<Applied>,
    /// The undo step's name: the edits' own labels.
    pub label: String,
}

/// `edits` applied to a copy of `design`, bare references signed against `evaluated`: all of them or none, `None` for no edits.
pub fn prepare(design: &RingDesign, edits: &[CadEdit], evaluated: Option<&Evaluated>) -> Result<Option<Prepared>, String> {
    if edits.is_empty() {
        return Ok(None);
    }
    let mut next = design.clone();
    let mut edits = fresh_where_taken(&next, edits);
    if let Some(e) = evaluated {
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
        applied.push(cad::edit_design(&mut next, edit).map_err(|e| format!("{e:#}"))?);
    }
    // A driven design carries the document its edited graph evaluates to.
    if let Some(json) = &next.graph {
        let doc = serde_json::from_value::<Graph>(json.clone()).map_err(|e| e.to_string()).and_then(|g| cad::document(&g).map_err(|e| e.message))?;
        next.cad = (!doc.features.is_empty()).then_some(doc);
    }
    let label = applied.iter().map(|a| a.label.as_str()).collect::<Vec<_>>().join(" · ");
    Ok(Some(Prepared { design: next, applied, label }))
}

/// Adds whose asked-for id another node of a driven design's graph carries, asking for a fresh id instead.
pub fn fresh_where_taken(design: &RingDesign, edits: &[CadEdit]) -> Vec<CadEdit> {
    let mut edits = edits.to_vec();
    if !edits.iter().any(|e| matches!(e, CadEdit::Add { .. })) {
        return edits;
    }
    let Some(g) = design.graph.as_ref().and_then(|j| serde_json::from_value::<Graph>(j.clone()).ok()) else {
        return edits;
    };
    let features: HashSet<u64> = cad::document(&g).map(|d| d.features.iter().map(|f| f.id).collect()).unwrap_or_default();
    let mut taken: HashSet<u64> = g.nodes.iter().map(|n| n.id.0).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{self, Attach, Component, ComponentRole, EdgeRef, Feature, Operation, Placement},
        templates,
    };

    fn court() -> RingDesign {
        templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }
    fn post(id: u64) -> Feature {
        Feature { id, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, component: Component { placement: Placement::ring(90.0, 0.25), attach: Attach::Join, ..Component::default() } }
    }
    fn shank(id: u64) -> Feature {
        Feature { id, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }
    }

    #[test]
    fn every_edit_lands_on_a_copy_under_one_label_or_none_is_kept() {
        let d = court();
        assert!(prepare(&d, &[], None).unwrap().is_none());
        let edits = [CadEdit::Add { feature: shank(1), after: None }, CadEdit::Add { feature: post(2), after: None }];
        let p = prepare(&d, &edits, None).unwrap().unwrap();
        assert!(d.cad.is_none(), "the design handed in is untouched");
        let doc = p.design.cad.as_ref().unwrap();
        assert_eq!(doc.features.iter().map(|f| (f.id, f.name.as_str())).collect::<Vec<_>>(), [(1, "Procedural shank"), (2, "Post")]);
        assert_eq!(p.applied.iter().map(|a| a.id).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!(p.label, "Add Procedural shank · Add Post");
        // One refusal and nothing lands: the post is added, then a missing feature is suppressed.
        let refused = prepare(&p.design, &[CadEdit::Add { feature: post(3), after: None }, CadEdit::Enable { id: 99, enabled: false }], None).unwrap_err();
        assert!(refused.contains("99"), "{refused}");
        assert_eq!(p.design.cad.as_ref().unwrap().features.len(), 2);
    }

    #[test]
    fn a_bare_edge_is_signed_in_the_frame_its_part_was_seated_by() {
        let mut d = court();
        d.cad = prepare(&d, &[CadEdit::Add { feature: shank(1), after: None }, CadEdit::Add { feature: post(2), after: None }], None).unwrap().unwrap().design.cad;
        let e = cad::evaluate(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 96, profile_steps: 48, refine: None, ..BuildParams::default() }).unwrap();
        let fillet = Feature { id: 0, name: "Fillet".into(), enabled: true, operation: Operation::Fillet { source: 2, edges: vec![EdgeRef::bare(0)], radius_mm: 0.2 }, component: Component::default() };
        let edits = [CadEdit::Add { feature: fillet, after: None }];
        let edge = |p: &Prepared| match &p.design.cad.as_ref().unwrap().features.last().unwrap().operation {
            Operation::Fillet { edges, .. } => edges[0].clone(),
            other => panic!("{other:?}"),
        };
        let unsigned = prepare(&d, &edits, None).unwrap().unwrap();
        assert!(edge(&unsigned).signature.is_none(), "without an evaluation the reference stays bare");
        let signed = prepare(&d, &edits, Some(&e)).unwrap().unwrap();
        assert_eq!((edge(&signed).ordinal, edge(&signed).signature.is_some()), (0, true));
        assert_eq!(signed.applied[0].id, Some(3), "an id of 0 asks the document for its next");
    }

    #[test]
    fn a_driven_design_takes_ids_its_graph_has_not_handed_out_and_carries_the_document_its_graph_reads_as() {
        let mut d = court();
        d.cad = prepare(&d, &[CadEdit::Add { feature: shank(1), after: None }, CadEdit::Add { feature: post(2), after: None }], None).unwrap().unwrap().design.cad;
        let g = ringdesign_graph::nodes::cad::from_document(&d).unwrap();
        let features: Vec<u64> = d.cad.as_ref().unwrap().features.iter().map(|f| f.id).collect();
        // A node the graph holds that is not a feature: the source, or one of the document's property nodes.
        let taken = g.nodes.iter().map(|n| n.id.0).find(|id| !features.contains(id) && *id != 0).unwrap();
        d.graph = Some(serde_json::to_value(&g).unwrap());
        let asked = Feature { id: taken, ..post(0) };
        let fixed = fresh_where_taken(&d, &[CadEdit::Add { feature: asked.clone(), after: None }]);
        let CadEdit::Add { feature, .. } = &fixed[0] else { unreachable!() };
        assert_eq!(feature.id, 0, "#{taken} names another node, so a fresh id is asked for");
        let p = prepare(&d, &[CadEdit::Add { feature: asked, after: None }], None).unwrap().unwrap();
        let added = p.applied[0].id.unwrap();
        assert!(added != taken && !features.contains(&added), "#{added}");
        let read = ringdesign_graph::nodes::cad::document(&serde_json::from_value(p.design.graph.clone().unwrap()).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(p.design.cad.as_ref().unwrap()).unwrap(), serde_json::to_value(&read).unwrap(), "the carried document is the graph's own");
        assert_eq!(read.features.len(), 3);
    }
}
