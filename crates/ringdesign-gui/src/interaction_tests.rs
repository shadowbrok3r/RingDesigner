//! Native interaction regressions observed through eframe inspection.
use crate::app::RingDesignerApp;
use egui_kittest::{Harness, kittest::Queryable};
pub(crate) fn harness() -> Harness<'static, RingDesignerApp> {
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
pub(crate) fn wait_until_previewed(h: &mut Harness<'static, RingDesignerApp>) {
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
pub(crate) fn press_ctrl_enter(h: &mut Harness<'static, RingDesignerApp>) {
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
pub(crate) fn create_and_apply_a_cylinder(h: &mut Harness<'static, RingDesignerApp>) -> u64 {
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

/// Step until the first build lands; the worker builds off-thread.
pub(crate) fn wait_for_build(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    while h.state().build.is_none() || h.state().is_building() {
        h.run_steps(3);
        assert!(start.elapsed() < std::time::Duration::from_secs(30), "the ring never built");
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
/// A press and release at `pos` with the modifiers held across both.
pub(crate) fn click_at(h: &mut Harness<'static, RingDesignerApp>, pos: egui::Pos2, button: egui::PointerButton, modifiers: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.event(egui::Event::PointerMoved(pos));
    h.event(egui::Event::PointerButton { pos, button, pressed: true, modifiers });
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos, button, pressed: false, modifiers });
    h.run_steps(2);
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run_steps(1);
}
pub(crate) fn viewport_label(h: &Harness<'static, RingDesignerApp>) -> String {
    use egui_kittest::kittest::NodeT;
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").accesskit_node().label().unwrap_or_default()
}

#[test]
fn the_ring_viewport_names_what_it_hovers_and_the_modifiers_build_the_selection() {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage};
    let mut h = harness();
    let pane = {
        let app = h.state_mut();
        app.switch_desktop(crate::dock::Desktop::Model);
        app.set_layout(crate::pane::Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = crate::pane::PaneKind::Solid;
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: 1,
            name: "Cylinder".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
            component: Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::ring(90.0, 0.25), ..Default::default() },
        })
        .unwrap();
        app.design.cad = Some(doc);
        app.rebuild_now();
        pane
    };
    wait_for_build(&mut h);
    assert!(h.state().pick_scene.as_ref().is_some_and(|s| s.parts() == 1), "the scene names the cylinder");
    // Look straight down the cylinder's axis: the eye at +y, the top face square on.
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.yaw = std::f32::consts::FRAC_PI_2;
        cam.pitch = 0.0;
        cam.roll = 0.0;
        cam.fit(bounds);
    }
    h.run_steps(3);
    let rect = h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect();
    let (centre, band) = {
        let app = h.state();
        let proj = app.panes[pane].camera.projector(rect);
        let c = app.build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 1).unwrap();
        let (lo, hi) = c.mesh.bounds().unwrap();
        let centre = proj.at([(lo.0 + hi.0) * 0.5, hi.1, (lo.2 + hi.2) * 0.5]);
        let r = (app.design.inner_radius_mm() + app.design.profile.thickness_mm) as f32;
        let (s, co) = 70f32.to_radians().sin_cos();
        (centre, proj.at([r * co, r * s, 0.0]))
    };
    h.hover_at(centre);
    h.run_steps(3);
    let label = viewport_label(&h);
    assert!(label.contains("hovering Cylinder face"), "{label}");
    assert!(h.state().selection.items.is_empty());
    // A right-click there offers the part's items.
    click_at(&mut h, centre, egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(h.query_by_label("Edit feature").is_some(), "the part's feature is offered");
    assert!(h.query_all_by_label_contains("Attach").next().is_some(), "and its attachment");
    assert!(h.query_by_label("Fit view").is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    // A click chooses the face; Shift-click on the band adds a point; Escape clears both.
    click_at(&mut h, centre, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items.len(), 1, "{:?}", h.state().selection.items);
    assert!(matches!(h.state().selection.items[0], ringdesign_workbench::viewport::Sel::Face { feature: 1, .. }));
    click_at(&mut h, band, egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    assert_eq!(h.state().selection.items.len(), 2, "{:?}", h.state().selection.items);
    assert!(matches!(h.state().selection.items[1], ringdesign_workbench::viewport::Sel::BandPoint { .. }));
    h.hover_at(band);
    h.run_steps(3);
    assert!(viewport_label(&h).contains("2 selected"), "{}", viewport_label(&h));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().selection.items.is_empty(), "Escape clears the selection");
    assert!(!viewport_label(&h).contains("selected"));
}

#[test]
fn a_cad_edit_through_the_funnel_is_one_undo_step_and_lands_in_a_driven_designs_graph() {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage, edit::CadEdit};
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut doc = Document::default();
        doc.append(Feature { id: 5, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: 1,
            name: "Cylinder".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
            component: Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::ring(90.0, 0.25), ..Default::default() },
        })
        .unwrap();
        doc.append(Feature { id: 2, name: "Lift".into(), enabled: true, operation: Operation::Transform { source: 1, translation: [0.0, 0.0, 0.5], rotation_deg: [0.0; 3] }, component: Component::default() }).unwrap();
        app.design.cad = Some(doc);
        app.history.commit(&app.design);
    }
    let feature = |h: &Harness<'static, RingDesignerApp>, id: u64| h.state().design.cad.as_ref().and_then(|d| d.feature(id)).cloned().unwrap();
    // A refusal names the dependent and leaves the design and the history alone.
    let entries = h.state().history.present();
    let refused = crate::cad_edit::apply(h.state_mut(), &[CadEdit::Remove { id: 1 }]).unwrap_err();
    assert!(refused.contains("#2 Lift"), "{refused}");
    assert_eq!(h.state().history.present(), entries);
    assert_eq!(h.state().design.cad.as_ref().unwrap().features.len(), 3);
    // Two edits in one call are one undo step.
    let applied = crate::cad_edit::apply(h.state_mut(), &[CadEdit::Rename { id: 2, name: "Post".into() }, CadEdit::Attach { id: 2, attach: Attach::Cut }]).unwrap();
    assert_eq!(applied.len(), 2);
    assert_eq!(h.state().history.present(), entries + 1);
    assert_eq!((feature(&h, 2).name.as_str(), feature(&h, 2).component.attach), ("Post", Attach::Cut));
    h.state_mut().undo();
    assert_eq!((feature(&h, 2).name.as_str(), feature(&h, 2).component.attach), ("Lift", Attach::Separate));
    // A driven design takes the edit on its graph; the document is the worker's to evaluate.
    {
        let app = h.state_mut();
        let g = ringdesign_graph::nodes::cad::from_document(&app.design).unwrap();
        app.set_graph(g);
        app.history.commit(&app.design);
    }
    let entries = h.state().history.present();
    crate::cad_edit::apply(h.state_mut(), &[CadEdit::Enable { id: 2, enabled: false }]).unwrap();
    assert_eq!(h.state().history.present(), entries + 1);
    let graph = |h: &Harness<'static, RingDesignerApp>| -> Document {
        let g: ringdesign_graph::graph::Graph = serde_json::from_value(h.state().design.graph.clone().expect("still driven")).unwrap();
        ringdesign_graph::nodes::cad::document(&g).unwrap()
    };
    assert!(!graph(&h).feature(2).unwrap().enabled);
    assert_eq!(h.state().status, "Suppress Lift");
    h.state_mut().undo();
    assert!(graph(&h).feature(2).unwrap().enabled);
}

#[test]
fn undo_takes_back_a_funnel_edit_on_a_driven_design_while_the_graph_pane_is_open() {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement, Stage, edit::CadEdit};
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut doc = Document::default();
        doc.append(Feature { id: 5, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: 6,
            name: "Cylinder".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
            component: Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::ring(90.0, 0.25), ..Default::default() },
        })
        .unwrap();
        app.design.cad = Some(doc);
        app.switch_desktop(crate::dock::Desktop::Graph);
        app.convert_to_graph();
    }
    for _ in 0..40 {
        h.run_steps(1);
    }
    {
        let app = h.state_mut();
        let design = app.design.clone();
        app.history.commit(&design);
    }
    let attach = |h: &Harness<'static, RingDesignerApp>| -> Attach {
        let g: ringdesign_graph::graph::Graph = serde_json::from_value(h.state().design.graph.clone().expect("driven")).unwrap();
        ringdesign_graph::nodes::cad::document(&g).unwrap().feature(6).unwrap().component.attach
    };
    assert_eq!(attach(&h), Attach::Join);
    crate::cad_edit::apply(h.state_mut(), &[CadEdit::Attach { id: 6, attach: Attach::Cut }]).unwrap();
    // The build lands and splices the evaluated design in before the undo, as it does in the app.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    for _ in 0..20 {
        h.run_steps(1);
    }
    assert_eq!(attach(&h), Attach::Cut);
    assert_eq!(h.state().design.cad.as_ref().and_then(|d| d.feature(6)).map(|f| f.component.attach), Some(Attach::Cut));
    let timeline: Vec<String> = h.state().history.timeline().into_iter().map(|(l, _)| l).collect();
    h.state_mut().undo();
    for _ in 0..20 {
        h.run_steps(1);
    }
    assert_eq!(attach(&h), Attach::Join, "undo takes the cut back; timeline before it: {timeline:?}");
}

