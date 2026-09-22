//! Native interaction regressions observed through eframe inspection.
use crate::app::RingDesignerApp;
use egui_kittest::{Harness, kittest::Queryable};
fn harness() -> Harness<'static, RingDesignerApp> {
    Harness::builder()
        .with_size([1100., 700.])
        .build_eframe(|cc| {
            crate::theme::install(&cc.egui_ctx);
            let mut app = RingDesignerApp::new(cc);
            app.updater.automatic = false;
            app.auto_rebuild = false;
            app
        })
}
#[test]
fn command_search_keyboard_chooses_the_highlighted_full_width_result() {
    let mut h = harness();
    h.state_mut().palette_open = true;
    h.state_mut().palette_query = "toggle".into();
    h.run_steps(4);
    let a = h.get_by_label("Toggle comparison ghost").rect();
    let b = h.get_by_label("Toggle stone previews").rect();
    assert!(a.width() > 330. && (a.width() - b.width()).abs() < 1.);
    let gems = h.state().show_gems;
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    assert_eq!(h.state().palette_selection, 1);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().show_gems, !gems);
    assert!(!h.state().palette_open);
}
#[test]
fn command_search_up_wraps_and_reopening_restores_all_rows() {
    let mut h = harness();
    h.state_mut().palette_open = true;
    h.state_mut().palette_query = "toggle".into();
    h.run_steps(4);
    h.key_press(egui::Key::ArrowUp);
    h.run_steps(2);
    assert_eq!(h.state().palette_selection, 2);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    h.state_mut().palette_query.clear();
    h.state_mut().palette_open = true;
    h.run_steps(4);
    let top = h.get_by_label("Turntable GIF…").rect();
    assert!(top.width() > 330.);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    assert!(h.state().palette_open);
}

#[test]
fn escape_never_discards_a_cad_candidate_and_cancel_keeps_it_restorable() {
    let mut h = harness();
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    h.run_steps(4);
    h.get_by_label("Create").click();
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(4);
    assert!(h.get_all_by_label("Cylinder").next().is_some());
    h.get_by_label("Inspect").click();
    h.run_steps(3);
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    for _ in 0..2 {
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        assert!(h.get_all_by_label("Cylinder").next().is_some(), "Escape keeps the uncommitted solid");
    }
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().design.cad.is_none(), "A bare Enter previews and never applies");
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    assert!(h.query_by_label("Cylinder").is_none(), "Cancel sets the candidate aside");
    h.get_by_label("Restore discarded candidate").click();
    h.run_steps(3);
    assert!(h.get_all_by_label("Cylinder").next().is_some(), "and it comes back whole");
}

#[test]
fn a_workspace_always_shows_its_own_view() {
    use crate::{dock::Desktop, pane::PaneKind};
    let mut h = harness();
    for desktop in Desktop::ALL {
        let app = h.state_mut();
        app.switch_desktop(desktop);
        // A stored layout that lost the workspace's view, as a pane-kind change leaves it.
        let other = if desktop.pane() == PaneKind::Graph { PaneKind::Solid } else { PaneKind::Graph };
        for i in app.visible_panes() {
            app.panes[i].kind = other;
        }
        app.switch_desktop(if desktop == Desktop::Model { Desktop::Casting } else { Desktop::Model });
        app.switch_desktop(desktop);
        let shown = app.visible_panes();
        assert!(shown.iter().any(|i| app.panes[*i].kind == desktop.pane()), "{}", desktop.label());
    }
    h.run_steps(2);
}

/// Step the harness until the Apply button enables; the evaluation runs on a worker thread.
fn wait_until_previewed(h: &mut Harness<'static, RingDesignerApp>) {
    use egui_kittest::kittest::NodeT;
    let start = std::time::Instant::now();
    loop {
        h.run_steps(5);
        if h.get_all_by_label("Apply").any(|n| !n.accesskit_node().is_disabled()) {
            return;
        }
        let error = h.state().cad.last_error().map(str::to_owned);
        assert!(error.is_none(), "the candidate failed to evaluate: {error:?}");
        assert!(start.elapsed() < std::time::Duration::from_secs(20), "the candidate never previewed");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
/// Ctrl+Enter held across a frame: egui keeps only the last modifier change of a frame.
fn press_ctrl_enter(h: &mut Harness<'static, RingDesignerApp>) {
    let key = |pressed| egui::Event::Key {
        key: egui::Key::Enter,
        pressed,
        modifiers: egui::Modifiers::COMMAND,
        repeat: false,
        physical_key: None,
    };
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::COMMAND));
    h.event(key(true));
    h.run_steps(2);
    h.event(key(false));
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run_steps(2);
}
/// Create → Cylinder on the CAD desktop, previewed with Enter and applied with Ctrl+Enter.
fn create_and_apply_a_cylinder(h: &mut Harness<'static, RingDesignerApp>) -> u64 {
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    h.run_steps(4);
    h.get_by_label("Create").click();
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(4);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    wait_until_previewed(h);
    let selected = h.state().cad.selected_feature().expect("the new feature is selected");
    press_ctrl_enter(h);
    selected
}

#[test]
fn a_plain_design_stays_plain_after_a_cad_apply() {
    use egui_kittest::kittest::NodeT;
    let mut h = harness();
    let selected = create_and_apply_a_cylinder(&mut h);
    {
        let app = h.state();
        assert_eq!(app.cad.last_error(), None);
        assert!(app.design.graph.is_none() && !app.graph_driven(), "a plain design stays plain");
        let doc = app.design.cad.as_ref().expect("the applied document");
        let names: Vec<_> = doc.features.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Procedural shank", "Cylinder"], "the first part brings the band with it");
        assert!(doc.outputs.contains(&selected));
        assert_eq!(app.cad.selected_feature(), Some(selected), "the selection survives the re-lift");
    }
    let chosen = h.get_all_by_label("Cylinder")
        .any(|n| n.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True));
    assert!(chosen, "the feature history still shows the Cylinder chosen");
    assert!(h.get_all_by_label("Apply").all(|n| n.accesskit_node().is_disabled()), "nothing is left to apply");
    assert!(h.query_by_label_contains("Parameters changed").is_none(), "the preview stays valid");
    h.state_mut().switch_desktop(crate::dock::Desktop::Model);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("Driven by the graph").next().is_none(), "the Design panel is not driven");
    h.state_mut().undo();
    h.run_steps(2);
    assert!(h.state().design.cad.is_none(), "undo takes the parts back");
}

#[test]
fn a_graph_driven_design_keeps_its_graph_after_a_cad_apply() {
    let mut h = harness();
    {
        let app = h.state_mut();
        let g = ringdesign_graph::nodes::cad::from_document(&app.design).unwrap();
        app.set_graph(g);
    }
    h.run_steps(2);
    let before = h.state().design.graph.clone().expect("driven before the edit");
    let selected = create_and_apply_a_cylinder(&mut h);
    {
        let app = h.state();
        assert_eq!(app.cad.last_error(), None);
        let after = app.design.graph.clone().expect("the graph stays");
        assert!(app.graph_driven() && after != before, "the graph carries the edit");
        let g: ringdesign_graph::graph::Graph = serde_json::from_value(after).unwrap();
        let feature = g.nodes.iter().find(|n| n.kind == "cad.feature" && n.id.0 == selected).expect("the feature node");
        assert_eq!(feature.params["name"], "Cylinder");
        assert_eq!(app.design.cad.as_ref().map(|d| d.features.len()), Some(2), "the shank came with the first part");
    }
    h.state_mut().switch_desktop(crate::dock::Desktop::Model);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("Driven by the graph").next().is_some(), "the Design panel says so");
}

/// Create → Procedural shank, then Create → Cylinder, previewed with Enter and applied with Ctrl+Enter.
fn create_and_apply_a_shank_with_a_cylinder(h: &mut Harness<'static, RingDesignerApp>) {
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    h.run_steps(4);
    h.get_by_label("Create").click();
    h.run_steps(3);
    h.get_by_label("Procedural shank").click();
    h.run_steps(4);
    h.get_by_label("Create").click();
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(4);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    wait_until_previewed(h);
    press_ctrl_enter(h);
}
/// Open the Ring viewport's tool menu on the Model desktop and say whether Paint 3D is offered.
fn paint_offered(h: &mut Harness<'static, RingDesignerApp>) -> bool {
    use egui_kittest::kittest::NodeT;
    let app = h.state_mut();
    app.switch_desktop(crate::dock::Desktop::Model);
    app.set_layout(crate::pane::Layout::Single);
    app.visual.select(ringdesign_workbench::visual::Tool::Select);
    h.run_steps(3);
    // The menu button carries the current tool's name and sits on the viewport's footer.
    h.query_all_by_label(ringdesign_workbench::visual::Tool::Select.label())
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .max_by(|a, b| a.rect().bottom().total_cmp(&b.rect().bottom()))
        .expect("the tool menu button")
        .click();
    h.run_steps(3);
    let offered = !h.get_by_label("Paint 3D").accesskit_node().is_disabled();
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    offered
}
/// Show the unrolled editor in the first pane and say whether it unrolls the band.
fn unrolled_available(h: &mut Harness<'static, RingDesignerApp>) -> bool {
    let app = h.state_mut();
    app.set_layout(crate::pane::Layout::SplitV);
    app.panes[0].kind = crate::pane::PaneKind::Unrolled;
    h.run_steps(3);
    h.query_all_by_label(ringdesign_workbench::cad_tools::PARTS_ONLY).next().is_none()
}

#[test]
fn parts_beside_a_procedural_shank_keep_the_surface_tools_and_start_joined() {
    use ringdesign_core::cad::{Attach, Operation};
    let mut h = harness();
    create_and_apply_a_shank_with_a_cylinder(&mut h);
    {
        let app = h.state();
        assert_eq!(app.cad.last_error(), None);
        assert!(app.design.graph.is_none(), "a plain design stays plain");
        let doc = app.design.cad.as_ref().expect("the applied document");
        let names: Vec<_> = doc.features.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Procedural shank", "Cylinder"]);
        assert!(matches!(doc.features[0].operation, Operation::Band));
        assert_eq!(doc.features[0].component.attach, Attach::Separate, "the shank is the band, not a part on it");
        assert_eq!(doc.features[1].component.attach, Attach::Join, "a part beside a procedural shank starts joined");
        assert!(!ringdesign_workbench::cad_tools::replaces_band(&app.design));
    }
    assert!(paint_offered(&mut h), "Paint 3D stays on the tool menu beside CAD parts");
    assert_eq!(h.state().surface_edit_reason(), None);
    assert!(unrolled_available(&mut h), "the unrolled editor still unrolls the band");
}

#[test]
fn a_ring_of_parts_only_refuses_the_surface_tools_and_says_why() {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation};
    let mut h = harness();
    // A ring of parts only cannot be made from the menus any more; it is what a CAD-only document is.
    {
        let app = h.state_mut();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Cylinder".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 4.0, height_mm: 3.0 }, component: Component::default() }).unwrap();
        app.design.cad = Some(doc);
        app.mark_dirty();
    }
    h.run_steps(4);
    {
        let app = h.state();
        let doc = app.design.cad.as_ref().expect("the document");
        assert_eq!(doc.features[0].component.attach, Attach::Separate, "with no shank there is nothing to join");
        assert!(ringdesign_workbench::cad_tools::replaces_band(&app.design));
    }
    assert!(!paint_offered(&mut h), "a ring of parts only has no band to paint");
    assert_eq!(h.state().surface_edit_reason(), Some(ringdesign_workbench::cad_tools::PARTS_ONLY));
    assert!(!unrolled_available(&mut h), "and nothing to unroll");
}
