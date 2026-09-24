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
fn a_part_added_to_a_driven_design_takes_an_id_its_graph_can_give() {
    use ringdesign_core::cad::{Attach, Component, Feature, Operation, Placement, edit::CadEdit};
    let mut h = harness();
    {
        let app = h.state_mut();
        app.design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        app.convert_to_graph();
        assert!(app.graph_driven(), "{}", app.status);
        app.history.commit(&app.design);
    }
    let taken = |h: &Harness<'static, RingDesignerApp>, id: u64| {
        let g: ringdesign_graph::graph::Graph = serde_json::from_value(h.state().design.graph.clone().unwrap()).unwrap();
        g.node(ringdesign_graph::graph::NodeId(id)).is_some()
    };
    assert!(taken(&h, 1) && taken(&h, 2), "the lifted band's nodes hold the ids a first part asks for");
    // The first part on a ring brings the shank at id 1 and takes id 2, as the viewport's add does.
    let shank = Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() };
    let post = Feature {
        id: 2,
        name: "Cylinder".into(),
        enabled: true,
        operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 3.0 },
        component: Component { attach: Attach::Join, placement: Placement::ring(95.0, 0.0), ..Default::default() },
    };
    let entries = h.state().history.present();
    let applied = crate::cad_edit::apply(h.state_mut(), &[CadEdit::Add { feature: shank, after: None }, CadEdit::Add { feature: post, after: None }]).unwrap();
    assert_eq!(h.state().history.present(), entries + 1, "one undo step");
    let ids: Vec<u64> = applied.iter().filter_map(|a| a.id).collect();
    assert_eq!(ids.len(), 2);
    let doc = h.state().design.cad.clone().expect("the graph's document");
    assert_eq!(doc.features.iter().map(|f| (f.id, f.name.as_str())).collect::<Vec<_>>(), [(ids[0], "Procedural shank"), (ids[1], "Cylinder")]);
    assert!(ids.iter().all(|id| *id > 2), "fresh ids above the graph's own: {ids:?}");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    assert_eq!(h.state().build.as_ref().unwrap().parts.joined, 1, "{}", h.state().status);
    h.state_mut().undo();
    assert!(h.state().design.cad.is_none(), "Undo takes both back");
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

/// A 5 × 2.2 mm low dome, a 3 mm round in a claw head on layer "Centre" at the top, a 1.2 mm disc struck at the palm.
pub(crate) fn claw_seat_and_stamp() -> ringdesign_core::RingDesign {
    use ringdesign_core::field::{Layer, LayerEntry, SeatPadLayer, SeatStyle};
    use ringdesign_core::gem::{Gem, GemCut};
    let mut d = ringdesign_core::RingDesign::default();
    d.profile.apply_style(ringdesign_core::ProfileStyle::LowDome);
    d.profile.width_mm = 5.0;
    d.profile.thickness_mm = 2.2;
    let v = d.field_context().crest_v_mm;
    let mut pad = SeatPadLayer { theta_deg: 90.0, v_mm: v, style: SeatStyle::Boss, blend_mm: 0.5, solid: ringdesign_core::setting::SolidKind::Prong, ..Default::default() };
    pad.fit_stone(Gem::calibrated(GemCut::Round, 3.0));
    pad.height_mm = 0.3;
    d.layers.layers.push(LayerEntry::new("Centre", Layer::SeatPad(pad)));
    let disc = (0..40).map(|i| {
        let t = std::f64::consts::TAU * f64::from(i) / 40.0;
        [1.2 * t.cos(), 1.2 * t.sin()]
    });
    d.stamps.push(ringdesign_core::setting::Stamp { name: "Disc".into(), theta_deg: 270.0, v_mm: v, rot_deg: 0.0, outline: disc.collect(), height_mm: 0.4, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false });
    d
}

/// `design` on one active Ring viewport, built and settled in its history.
fn on_one_ring_view(h: &mut Harness<'static, RingDesignerApp>, design: ringdesign_core::RingDesign) -> usize {
    let pane = {
        let app = h.state_mut();
        app.switch_desktop(crate::dock::Desktop::Model);
        app.set_layout(crate::pane::Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = crate::pane::PaneKind::Solid;
        app.active_pane = pane;
        app.design = design;
        app.history.commit(&app.design);
        app.rebuild_now();
        pane
    };
    wait_for_build(h);
    pane
}

/// Turns the pane's camera to `yaw` and frames the ring: π/2 looks down −y onto the top, −π/2 up +y onto the palm.
fn face(h: &mut Harness<'static, RingDesignerApp>, pane: usize, yaw: f32) -> egui::Rect {
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.yaw = yaw;
        cam.pitch = 0.0;
        cam.roll = 0.0;
        cam.fit(bounds);
    }
    h.run_steps(3);
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// Opens the menu at `at` and chooses `item`, through `submenu` when there is one.
fn menu_row(h: &mut Harness<'static, RingDesignerApp>, at: egui::Pos2, submenu: Option<&str>, item: &str) {
    click_at(h, at, egui::PointerButton::Secondary, egui::Modifiers::NONE);
    if let Some(sub) = submenu {
        // A submenu's button carries egui's own arrow.
        h.get_by_label(&format!("{sub} ⏵")).click();
        h.run_steps(3);
    }
    h.get_by_label(item).click();
    h.run_steps(3);
}

#[test]
fn a_claw_head_on_a_seat_and_a_struck_stamp_answer_the_pointer_as_themselves() {
    use ringdesign_core::interaction::pick::Entity;
    use ringdesign_workbench::viewport::{Sel, tint};
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, claw_seat_and_stamp());
    let made = |h: &Harness<'static, RingDesignerApp>, of: &dyn Fn(&ringdesign_core::BuildResult, u32) -> bool| {
        let b = h.state().build.clone().unwrap();
        b.mesh.origin.iter().filter(|o| of(&b, **o)).count()
    };
    let head_vertices = made(&h, &|b, o| b.solids.stone_of(o) == Some(0));
    let disc_vertices = made(&h, &|b, o| b.solids.stamp_of(o) == Some(0));
    assert!(head_vertices > 1000 && disc_vertices > 100, "{head_vertices} {disc_vertices}");
    let lit = |h: &Harness<'static, RingDesignerApp>, weight: f32| tint(&h.state().selection, h.state().build.as_ref().unwrap()).iter().filter(|w| **w == weight).count();
    // Down onto the top, over one of the claws, wherever the viewport stands on screen.
    face(&mut h, pane, std::f32::consts::FRAC_PI_2);
    let claw_at = |h: &Harness<'static, RingDesignerApp>| {
        let app = h.state();
        let (_, frame) = ringdesign_core::stones::stone_frames(&app.design).remove(0);
        let gem = ringdesign_core::gem::Gem::calibrated(ringdesign_core::gem::GemCut::Round, 3.0);
        let plan = ringdesign_core::setting::Plan::of(gem);
        let p = plan.point(plan.claw_angles(ringdesign_core::setting::claw_count(gem, 0))[0]);
        let at: [f32; 3] = std::array::from_fn(|k| (frame.girdle[k] + frame.long[k] * p[0] + frame.short[k] * p[1]) as f32);
        let rect = h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect();
        app.panes[pane].camera.projector(rect).at(at)
    };
    let claw = claw_at(&h);
    h.hover_at(claw);
    h.run_steps(3);
    assert!(viewport_label(&h).contains("hovering claw head on Centre"), "{}", viewport_label(&h));
    assert_eq!(h.state().selection.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Seat { path: vec![0] }));
    assert_eq!(lit(&h, 2.0), head_vertices, "the hover lights every vertex the head's solid made");
    // A click chooses it and names it.
    click_at(&mut h, claw, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items, [Sel::Seat(vec![0])]);
    assert_eq!(h.state().status, "claw head on Centre");
    // Its menu: its layer and the view's switches, ticked as they stand.
    click_at(&mut h, claw, egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(h.query_by_label("Claw head on Centre").is_some(), "the menu names what it is about");
    for row in ["Select layer \"Centre\"", "Live cuts", "Show cutters"] {
        assert!(h.query_by_label(row).is_some(), "{row}");
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    menu_row(&mut h, claw, None, "Show cutters");
    assert!(h.state().show_cutters, "the cutters are drawn");
    menu_row(&mut h, claw, None, "Select layer \"Centre\"");
    assert_eq!(h.state().selected_layer, Some(0));
    assert!(h.state().dock.is_open(crate::dock::ToolKind::Layers), "the Layers tool shows it");
    assert_eq!(h.state().status, "Layer \"Centre\" chosen in the Layers tool");
    // Live cuts off builds the stock alone, with nothing of the head; the chosen seat's menu off the ring turns them back on.
    let before = serde_json::to_value(&h.state().design).unwrap();
    h.run_steps(2);
    let claw = claw_at(&h);
    menu_row(&mut h, claw, None, "Live cuts");
    assert!(!h.state().live_cuts);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    let scene = h.state().pick_scene.clone().unwrap();
    assert!((0..scene.faces()).all(|f| scene.made_of_face(f).is_none()), "no seat's solid and no stamp in the stock");
    let off_ring = {
        let rect = h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect();
        rect.center() - egui::vec2(0.0, rect.height() * 0.35)
    };
    menu_row(&mut h, off_ring, None, "Live cuts");
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    assert!(h.state().live_cuts && serde_json::to_value(&h.state().design).unwrap() == before, "the switches never touch the design");

    // Up onto the palm, over the disc.
    let rect = face(&mut h, pane, -std::f32::consts::FRAC_PI_2);
    let disc = {
        let app = h.state();
        let r = (app.design.inner_radius_mm() + app.design.profile.thickness_mm) as f32;
        app.panes[pane].camera.projector(rect).at([0.0, -(r + 0.4), 0.0])
    };
    h.hover_at(disc);
    h.run_steps(3);
    assert!(viewport_label(&h).contains("hovering stamp \"Disc\""), "{}", viewport_label(&h));
    assert_eq!(lit(&h, 2.0), disc_vertices, "the hover lights every vertex the disc made");
    click_at(&mut h, disc, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items, [Sel::Stamp(0)]);
    click_at(&mut h, disc, egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(h.query_by_label("Stamp \"Disc\"").is_some());
    for row in ["Edit stamp…", "Attach ⏵", "Stage ⏵", "Delete stamp"] {
        assert!(h.query_by_label(row).is_some(), "{row}");
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // Staged for the bench: one undo step.
    let start = h.state().history.present();
    menu_row(&mut h, disc, Some("Stage"), "Bench");
    assert!(h.state().design.stamps[0].bench);
    assert_eq!(h.state().history.present(), start + 1);
    assert_eq!(h.state().history.undo_label(), Some("Bench stamp \"Disc\""));
    // Its inspector: a typed angle is one undo step of its own.
    menu_row(&mut h, disc, None, "Edit stamp…");
    assert!(h.query_by_label("Stamp name").is_some() && h.query_by_label("Stamp across").is_some());
    h.get_by_label("Stamp angle").click_accesskit();
    h.run_steps(2);
    h.event(egui::Event::Text("265".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!((h.state().design.stamps[0].theta_deg - 265.0).abs() < 1e-9, "{}", h.state().design.stamps[0].theta_deg);
    assert_eq!(h.state().history.present(), start + 2);
    assert_eq!(h.state().history.undo_label(), Some("Edit stamp \"Disc\""));
    // Deleted, one undo step; Undo brings it back as it was.
    menu_row(&mut h, disc, None, "Delete stamp");
    assert!(h.state().design.stamps.is_empty());
    assert_eq!(h.state().history.present(), start + 3);
    assert_eq!(h.state().status, "Delete stamp \"Disc\"");
    assert!(h.state().selection.items.is_empty() && h.state().stamp_inspector.is_none(), "nothing is left naming it");
    h.state_mut().undo();
    let back = &h.state().design.stamps;
    assert_eq!((back.len(), back[0].name.as_str(), back[0].bench, back[0].theta_deg), (1, "Disc", true, 265.0));
}

/// A closed 16-sided post of radius 0.8 and height 2, its foot a hair under z 0, wound outward.
fn post_mesh() -> ringdesign_core::Mesh {
    use ringdesign_core::Vec3;
    let n = 16u32;
    let at = |k: u32, z: f32| {
        let a = f64::from(k) * std::f64::consts::TAU / f64::from(n);
        Vec3((0.8 * a.cos()) as f32, (0.8 * a.sin()) as f32, z)
    };
    let mut m = ringdesign_core::Mesh { vertices: vec![Vec3(0.0, 0.0, -0.02), Vec3(0.0, 0.0, 1.98)], ..Default::default() };
    for k in 0..n {
        m.vertices.push(at(k, -0.02));
        m.vertices.push(at(k, 1.98));
    }
    for k in 0..n {
        let (b0, t0, b1, t1) = (2 + 2 * k, 3 + 2 * k, 2 + 2 * ((k + 1) % n), 3 + 2 * ((k + 1) % n));
        m.faces.extend([[0, b1, b0], [1, t0, t1], [b0, b1, t1], [b0, t1, t0]]);
    }
    m
}

#[test]
fn a_slow_part_import_keeps_the_window_live_says_so_can_be_cancelled_and_lands_as_one_undo_step() {
    use ringdesign_workbench::viewport::Sel;
    let dir = tempfile::tempdir().unwrap();
    let stl = dir.path().join("post.stl");
    ringdesign_core::stl::write_stl(&stl, &post_mesh(), "post").unwrap();
    // A reader that takes its time, as OpenCascade does over a whole ring.
    let slow = |path: std::path::PathBuf, wait: u64| {
        move || {
            std::thread::sleep(std::time::Duration::from_millis(wait));
            ringdesign_mcp::import::part_file(&path).map_err(|e| format!("{e:#}"))
        }
    };
    let mut h = harness();
    on_one_ring_view(&mut h, ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design());
    let start = h.state().history.present();
    let asked = std::time::Instant::now();
    h.state_mut().start_import("post.stl".into(), "a slow reader", slow(stl.clone(), 1500));
    assert!(asked.elapsed() < std::time::Duration::from_millis(200), "asking returns at once: {:?}", asked.elapsed());
    // The window stays live while it reads, and the status line and the plate say what is read.
    for _ in 0..3 {
        h.run_steps(1);
    }
    assert!(h.state().importing.is_some(), "three frames ran while the reader slept");
    assert!(h.state().status.starts_with("Reading post.stl in a slow reader… "), "{}", h.state().status);
    assert!(h.query_by_label("Cancel import").is_some());
    // One import at a time.
    h.state_mut().start_import("other.stl".into(), "a slow reader", slow(stl.clone(), 0));
    assert_eq!(h.state().status, "post.stl is still being read; cancel it before importing another part");
    assert_eq!(h.state().importing.as_ref().map(|p| p.file.as_str()), Some("post.stl"));
    // It lands joined at the top and chosen, one undo step.
    let waited = std::time::Instant::now();
    while h.state().importing.is_some() {
        h.run_steps(1);
        assert!(waited.elapsed() < std::time::Duration::from_secs(20), "the part never landed");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.run_steps(2);
    let doc = h.state().design.cad.clone().expect("the part's document");
    assert_eq!(doc.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Procedural shank", "post"]);
    assert_eq!(h.state().history.present(), start + 1);
    assert_eq!(h.state().selection.items, [Sel::Part(doc.features[1].id)]);
    assert!(h.state().status.starts_with("Imported post at the top of the ring, joined"), "{}", h.state().status);
    assert!(h.query_by_label("Cancel import").is_none(), "the plate is gone");
    // Cancelled, what the reader finds is dropped: no undo step, nothing changes.
    h.state_mut().start_import("again.stl".into(), "a slow reader", slow(stl.clone(), 400));
    h.run_steps(2);
    h.get_by_label("Cancel import").click();
    h.run_steps(2);
    assert!(h.state().importing.is_none());
    assert_eq!(h.state().status, "Stopped reading again.stl: nothing was imported");
    std::thread::sleep(std::time::Duration::from_millis(600));
    h.run_steps(3);
    assert_eq!(h.state().history.present(), start + 1);
    assert_eq!(h.state().design.cad.as_ref().map(|d| d.features.len()), Some(2));
    // A reader that fails says why, and nothing changes.
    h.state_mut().start_import("missing.stl".into(), "a slow reader", slow(dir.path().join("missing.stl"), 0));
    let waited = std::time::Instant::now();
    while h.state().importing.is_some() {
        h.run_steps(1);
        assert!(waited.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(h.state().status.contains("missing.stl"), "{}", h.state().status);
    assert_eq!(h.state().history.present(), start + 1);
}

