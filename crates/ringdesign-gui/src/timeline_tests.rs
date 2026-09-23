//! The feature timeline on the Ring viewport and in the CAD pane.
#[allow(unused_imports)]
use crate::interaction_tests::*;
use crate::app::RingDesignerApp;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use ringdesign_core::{
    RingDesign,
    cad::{Attach, Component, Document, Feature, Operation, Placement},
};

/// The procedural shank, a cylinder joined on the top and a box standing beside the palm.
fn document() -> Document {
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    doc.append(Feature {
        id: 2,
        name: "Cylinder".into(),
        enabled: true,
        operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
        component: Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.25), ..Default::default() },
    })
    .unwrap();
    doc.append(Feature {
        id: 3,
        name: "Box".into(),
        enabled: true,
        operation: Operation::Box { size: [2.0, 2.0, 1.5] },
        component: Component { placement: Placement::ring(270.0, 0.5), ..Default::default() },
    })
    .unwrap();
    doc
}

/// The Court band with the document's parts on it.
fn design() -> RingDesign {
    let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").expect("the Court band template").design();
    d.cad = Some(document());
    d
}

/// Rebuilds and steps until the ring on screen is the design as it stands.
fn settle(h: &mut Harness<'static, RingDesignerApp>) {
    h.state_mut().rebuild_now();
    wait_for_build(h);
    h.run_steps(2);
    assert!(h.state().is_current(), "{}", h.state().status);
}

/// One Ring viewport on the Model desktop over the design, built.
fn ring() -> Harness<'static, RingDesignerApp> {
    let mut h = harness();
    {
        let app = h.state_mut();
        app.switch_desktop(crate::dock::Desktop::Model);
        app.set_layout(crate::pane::Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = crate::pane::PaneKind::Solid;
        app.design = design();
        app.history.commit(&app.design);
    }
    settle(&mut h);
    h
}

/// The parts the field verdict lists: feature, attachment, and whether it was judged against the parting plane.
fn verdict_parts(h: &Harness<'static, RingDesignerApp>) -> Vec<(u64, Attach, bool)> {
    let field = h.state().field.as_ref().expect("the field verdict");
    field.parts.iter().map(|p| (p.feature, p.attach, p.judged)).collect()
}

fn joined(h: &Harness<'static, RingDesignerApp>) -> (usize, usize) {
    let parts = &h.state().build.as_ref().expect("a build").parts;
    (parts.joined, parts.separate)
}

/// Right-click the chip whose label starts `chip` and choose `item`.
fn menu(h: &mut Harness<'static, RingDesignerApp>, chip: &str, item: &str) {
    h.query_all_by_label_contains(chip).find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(chip))).expect("the chip").click_secondary();
    h.run_steps(2);
    h.get_by_label(item).click();
    h.run_steps(2);
}

#[test]
fn the_ring_viewport_strip_suppresses_through_the_funnel_undoes_and_does_the_same_through_a_graph() {
    let mut h = ring();
    let labels = ["Procedural shank · ok", "Cylinder · ok", "Box · ok"];
    let rects: Vec<egui::Rect> = labels.iter().map(|l| h.get_by_label(l).rect()).collect();
    let viewport = h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect();
    assert!(rects.iter().all(|r| (r.top() - rects[0].top()).abs() < 0.5), "one row: {rects:?}");
    assert!(rects[0].right() <= rects[1].left() && rects[1].right() <= rects[2].left(), "in document order: {rects:?}");
    assert!(rects.iter().all(|r| r.top() >= viewport.bottom() - 1.0 && viewport.x_range().contains(r.center().x)), "under the Ring viewport: {viewport:?} {rects:?}");
    assert_eq!(joined(&h), (1, 1));
    assert_eq!(verdict_parts(&h), [(2, Attach::Join, true), (3, Attach::Separate, false)], "the joined cylinder judged, the box beside it not");
    let entries = h.state().history.present();

    menu(&mut h, "Cylinder · ok", "Suppress");
    assert_eq!(h.state().status, "Suppress Cylinder");
    assert_eq!(h.state().history.present(), entries + 1, "one undo step");
    assert!(h.query_by_label("Cylinder · suppressed").is_some(), "the chip says so before the rebuild lands");
    settle(&mut h);
    assert_eq!(joined(&h), (0, 1), "one fewer part in the build");
    assert_eq!(verdict_parts(&h), [(3, Attach::Separate, false)], "a suppressed part is not in the verdict");
    h.get_by_label("Cylinder · suppressed");
    let plain = serde_json::to_value(h.state().design.cad.as_ref().unwrap()).unwrap();

    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(joined(&h), (1, 1));
    h.get_by_label("Cylinder · ok");

    // The same design converted to a graph takes the same gesture on its graph.
    {
        let app = h.state_mut();
        app.convert_to_graph();
        assert!(app.graph_driven(), "{}", app.status);
        // Back to the one Ring viewport; the Graph desktop shows two previews, each with its strip.
        app.switch_desktop(crate::dock::Desktop::Model);
        app.history.commit(&app.design);
    }
    settle(&mut h);
    h.get_by_label("Cylinder · ok");
    let before = h.state().design.graph.clone();
    menu(&mut h, "Cylinder · ok", "Suppress");
    assert_ne!(h.state().design.graph, before, "the graph took the edit");
    assert!(h.query_by_label("Cylinder · suppressed").is_some(), "the funnel carries the graph's document: no build of lag");
    settle(&mut h);
    assert!(h.state().graph_driven(), "still driven");
    assert_eq!(serde_json::to_value(h.state().design.cad.as_ref().unwrap()).unwrap(), plain, "the graph evaluates to the plain edit's document");
    assert_eq!(joined(&h), (0, 1));
    h.get_by_label("Cylinder · suppressed");
}

#[test]
fn delete_over_the_strip_removes_the_chosen_feature_and_leaves_the_chosen_layer() {
    let mut h = ring();
    {
        // A chosen layer for the app's own Delete to find.
        let app = h.state_mut();
        let beads = ringdesign_core::Layer::Milgrain(ringdesign_core::field::MilgrainLayer::default());
        app.add_layer("Beads", beads);
        app.history.commit(&app.design);
    }
    settle(&mut h);
    let layers = h.state().design.layers.layers.len();
    assert_eq!(h.state().selected_layer, Some(layers - 1));
    h.state_mut().selected_layer = Some(0);
    h.get_by_label("Box · ok").click();
    h.run_steps(2);
    assert_eq!(h.state().selection.items, [ringdesign_workbench::viewport::Sel::Part(3)], "a click on a chip chooses its part");
    h.get_by_label("Box · ok").hover();
    h.run_steps(2);
    h.key_press(egui::Key::Delete);
    h.run_steps(2);
    let doc = h.state().design.cad.clone().unwrap();
    assert_eq!(doc.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Procedural shank", "Cylinder"]);
    assert_eq!((h.state().design.layers.layers.len(), h.state().selected_layer), (layers, Some(0)), "the layer is not the strip's to delete");
    // Over the viewport the key is the app's own again.
    let viewport = h.query_all_by_label_contains("Ring viewport").next().unwrap().rect();
    h.hover_at(viewport.center());
    h.run_steps(2);
    h.key_press(egui::Key::Delete);
    h.run_steps(2);
    assert_eq!(h.state().design.layers.layers.len(), layers - 1);
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), 2);
}

#[test]
fn the_cad_pane_lists_the_same_chips_and_says_it_is_evaluating_before_it_has() {
    let mut h = ring();
    menu(&mut h, "Box · ok", "Suppress");
    settle(&mut h);
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    h.run_steps(1);
    assert!(h.query_by_label_contains("Evaluating the candidate").is_some(), "the first evaluation is on its way");
    assert!(h.query_by_label_contains("Parameters changed").is_none(), "nothing has changed yet");
    let start = std::time::Instant::now();
    while h.query_by_label("Procedural shank · ok").is_none() {
        h.run_steps(3);
        assert!(start.elapsed() < std::time::Duration::from_secs(20), "the pane never evaluated");
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    let rects: Vec<egui::Rect> = ["Procedural shank · ok", "Cylinder · ok", "Box · suppressed"].iter().map(|l| h.get_by_label(l).rect()).collect();
    assert!(rects[0].bottom() <= rects[1].top() && rects[1].bottom() <= rects[2].top(), "one per line: {rects:?}");
    assert!(h.query_by_label_contains("Evaluating the candidate").is_none());
    // No candidate is pending, so the list's edit is committed through the funnel.
    let entries = h.state().history.present();
    menu(&mut h, "Box · suppressed", "Unsuppress");
    assert!(h.state().design.cad.as_ref().unwrap().feature(3).unwrap().enabled);
    assert_eq!(h.state().history.present(), entries + 1);
    assert!(h.get_all_by_label("Apply").all(|n| n.accesskit_node().is_disabled()), "nothing is left to apply");
}

#[test]
fn a_structural_edit_in_the_cad_pane_keeps_a_pending_candidate() {
    let mut h = ring();
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    h.run_steps(4);
    h.get_by_label("Create").click();
    h.run_steps(3);
    h.get_by_label("Sphere").click();
    h.run_steps(4);
    assert!(h.query_by_label_contains("Sphere ·").is_some(), "the candidate's sphere is on the list");
    let entries = h.state().history.present();
    // Pending or evaluated by now, the chip is the same one.
    menu(&mut h, "Cylinder · ", "Suppress");
    assert!(h.state().design.cad.as_ref().unwrap().feature(2).unwrap().enabled, "the design is not touched");
    assert_eq!(h.state().history.present(), entries);
    assert!(h.query_by_label("Cylinder · suppressed").is_some() && h.query_by_label_contains("Sphere ·").is_some(), "the candidate kept both");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    wait_until_previewed(&mut h);
    press_ctrl_enter(&mut h);
    let doc = h.state().design.cad.clone().unwrap();
    assert_eq!(doc.features.iter().map(|f| (f.name.as_str(), f.enabled)).collect::<Vec<_>>(), [("Procedural shank", true), ("Cylinder", false), ("Box", true), ("Sphere", true)]);
}
