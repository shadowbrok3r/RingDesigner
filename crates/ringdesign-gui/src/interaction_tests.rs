//! Native interaction regressions observed through eframe inspection.
use crate::app::RingDesignerApp;
use egui_kittest::{Harness, kittest::Queryable};
pub(crate) fn harness() -> Harness<'static, RingDesignerApp> {
    sized([1100., 700.])
}
/// The app in a window `size` points big, rebuilding only when asked.
fn sized(size: [f32; 2]) -> Harness<'static, RingDesignerApp> {
    Harness::builder()
        .with_size(size)
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
/// Steps until the template being opened has landed.
pub(crate) fn wait_for_template(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    while h.state().opening.is_some() {
        h.run_steps(1);
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "the template never opened");
        std::thread::sleep(std::time::Duration::from_millis(5));
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
    d.stamps.push(ringdesign_core::setting::Stamp { name: "Disc".into(), theta_deg: 270.0, v_mm: v, rot_deg: 0.0, outline: disc.collect(), height_mm: 0.4, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: Default::default() });
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
    assert_eq!(h.state().selection.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Seat { path: vec![0], station: 0 }));
    assert_eq!(lit(&h, 2.0), head_vertices, "the hover lights every vertex the head's solid made");
    // A click chooses it and names it.
    click_at(&mut h, claw, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items, [Sel::Seat { path: vec![0], station: 0 }]);
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
    // The Delete key takes a chosen stamp away as the menu row does.
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    click_at(&mut h, disc, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items, [Sel::Stamp(0)]);
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert!(h.state().design.stamps.is_empty(), "the Delete key deletes the chosen stamp");
    assert_eq!(h.state().history.undo_label(), Some("Delete stamp \"Disc\""));
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
        move |_: &std::sync::atomic::AtomicBool| {
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

/// A 5 × 2.2 mm low dome with three discs struck round the palm: Moon at 240°, Disc at 270°, Star at 300°.
fn three_stamps() -> ringdesign_core::RingDesign {
    let mut d = claw_seat_and_stamp();
    d.layers.layers.clear();
    let disc = d.stamps[0].clone();
    d.stamps = [("Moon", 240.0), ("Disc", 270.0), ("Star", 300.0)].into_iter().map(|(name, theta_deg)| ringdesign_core::setting::Stamp { name: name.into(), theta_deg, ..disc.clone() }).collect();
    d
}

#[test]
fn a_chosen_stamp_and_its_inspector_follow_undo_and_redo_by_what_the_stamp_is() {
    use ringdesign_core::interaction::pick::Entity;
    use ringdesign_workbench::viewport::{Mods, Sel, StampEdit};
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, three_stamps());
    let names = |h: &Harness<'static, RingDesignerApp>| h.state().design.stamps.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
    let chosen = |h: &Harness<'static, RingDesignerApp>| (h.state().selection.items.clone(), h.state().stamp_inspector);
    // Star chosen and in its inspector, Moon deleted: Star moves down one, inspector and all.
    {
        let app = h.state_mut();
        app.selection.click(Some(Sel::Stamp(2)), Mods::default());
        app.stamp_inspector = Some(2);
        crate::viewport::stamp_edit(app, 0, &StampEdit::Delete);
    }
    assert_eq!((names(&h), chosen(&h)), (vec!["Disc".to_string(), "Star".into()], (vec![Sel::Stamp(1)], Some(1))));
    // Before the ring is built again the scene on screen still numbers the three: over Star it names Star where Star now is, over Moon nothing.
    let rect = face(&mut h, pane, -std::f32::consts::FRAC_PI_2);
    let over = |h: &Harness<'static, RingDesignerApp>, theta_deg: f64| {
        let app = h.state();
        let r = app.design.inner_radius_mm() + app.design.profile.thickness_mm + 0.4;
        let (s, c) = theta_deg.to_radians().sin_cos();
        app.panes[pane].camera.projector(rect).at([(r * c) as f32, (r * s) as f32, 0.0])
    };
    assert_eq!(h.state().pick_stamps.len(), 3, "the scene is the one built before the deletion");
    h.hover_at(over(&h, 300.0));
    h.run_steps(3);
    assert_eq!(h.state().selection.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Stamp { index: 1 }));
    assert!(viewport_label(&h).contains("hovering stamp \"Star\""), "{}", viewport_label(&h));
    h.hover_at(over(&h, 240.0));
    h.run_steps(3);
    assert!(!matches!(h.state().selection.hover.as_ref().map(|p| &p.entity), Some(Entity::Stamp { .. })), "Moon is gone: {:?}", h.state().selection.hover);
    // Undone, Moon is back before it and Star is still the one chosen and inspected.
    h.state_mut().undo();
    h.run_steps(2);
    assert_eq!((names(&h), chosen(&h)), (vec!["Moon".to_string(), "Disc".into(), "Star".into()], (vec![Sel::Stamp(2)], Some(2))), "Star, not Disc");
    assert_eq!(h.get_by_label("Stamp name").value().as_deref(), Some("Star"), "the inspector shows Star");
    // Redone, both follow it back down; Moon chosen where the undo brings it back is let go when the redo takes it.
    h.state_mut().redo();
    assert_eq!(chosen(&h), (vec![Sel::Stamp(1)], Some(1)));
    h.state_mut().undo();
    {
        let app = h.state_mut();
        app.selection.click(Some(Sel::Stamp(0)), Mods::default());
        app.stamp_inspector = Some(0);
    }
    h.state_mut().redo();
    h.run_steps(2);
    assert_eq!(chosen(&h), (vec![], None), "a stamp gone is let go");
    // An edit of the stamp itself undone keeps it chosen in its place.
    h.state_mut().undo();
    {
        let app = h.state_mut();
        app.selection.click(Some(Sel::Stamp(2)), Mods::default());
        app.stamp_inspector = Some(2);
        app.design.stamps[2].theta_deg = 310.0;
        app.design.stamps[2].name = "Evening star".into();
        let design = app.design.clone();
        app.history.commit_as(&design, "Edit stamp \"Evening star\"");
    }
    h.state_mut().undo();
    h.run_steps(2);
    assert_eq!((h.state().design.stamps[2].theta_deg, chosen(&h)), (300.0, (vec![Sel::Stamp(2)], Some(2))));
    // The Delete key after the undo takes the stamp chosen, not the one standing at its old place.
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert_eq!(names(&h), ["Moon", "Disc"]);
    assert_eq!(h.state().history.undo_label(), Some("Delete stamp \"Star\""));
}

#[test]
fn the_stamp_inspector_and_the_tool_inspector_open_apart() {
    let mut h = harness();
    on_one_ring_view(&mut h, three_stamps());
    h.state_mut().visual.select(ringdesign_workbench::visual::Tool::Measure);
    h.state_mut().stamp_inspector = Some(1);
    h.run_steps(3);
    let view = h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect();
    let area = |id: &str| h.ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap_or_else(|| panic!("{id} is open"));
    let (tool, stamp) = (area("direct-viewport-inspector"), area("stamp-inspector"));
    assert!(!tool.intersects(stamp), "{tool:?} and {stamp:?}");
    for r in [tool, stamp] {
        assert!(view.contains_rect(r), "{r:?} in {view:?}");
    }
    assert!(stamp.right() >= view.right() - 30.0 && stamp.bottom() >= view.bottom() - 60.0, "the stamp's at the lower right: {stamp:?} in {view:?}");
}

/// Six flush-cut stones in a run round a 5 × 2.2 mm low dome, on layer "Row".
fn flush_row() -> ringdesign_core::RingDesign {
    use ringdesign_core::field::{Layer, LayerEntry, SeatRunLayer};
    let mut d = claw_seat_and_stamp();
    d.layers.layers.clear();
    d.stamps.clear();
    let mut run = SeatRunLayer { count: 6, ..SeatRunLayer::default() };
    run.seat.v_mm = d.field_context().crest_v_mm;
    run.seat.solid = ringdesign_core::setting::SolidKind::Flush;
    d.layers.layers.push(LayerEntry::new("Row", Layer::SeatRun(run)));
    d
}

#[test]
fn each_station_of_a_seat_run_lights_and_chooses_alone_and_its_menu_names_the_runs_layer() {
    use ringdesign_core::interaction::pick::Entity;
    use ringdesign_workbench::viewport::{Sel, tint};
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, flush_row());
    h.state_mut().selection.filter.stones = false;
    let build = h.state().build.clone().unwrap();
    assert_eq!(build.solids.paths, vec![vec![0usize]; 6], "{:?}", build.solids.notes);
    let own = |i: usize| build.mesh.origin.iter().filter(|o| build.solids.stone_of(**o) == Some(i)).count();
    let lit = |h: &Harness<'static, RingDesignerApp>, weight: f32| tint(&h.state().selection, h.state().build.as_ref().unwrap()).iter().filter(|w| **w == weight).count();
    // Down onto the top: the station nearest the eye, then its neighbour round the ring.
    let rect = face(&mut h, pane, std::f32::consts::FRAC_PI_2);
    let frames = ringdesign_core::stones::stone_frames(&h.state().design);
    let mut by_height: Vec<usize> = (0..frames.len()).collect();
    by_height.sort_by(|a, b| frames[*b].1.girdle[1].total_cmp(&frames[*a].1.girdle[1]));
    let at = |h: &Harness<'static, RingDesignerApp>, k: usize| h.state().panes[pane].camera.projector(rect).at(frames[k].1.girdle.map(|v| v as f32));
    let top = by_height[0];
    h.hover_at(at(&h, top));
    h.run_steps(3);
    assert_eq!(h.state().selection.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Seat { path: vec![0], station: top as u32 }));
    assert_eq!(lit(&h, 2.0), own(top), "the hover lights its own station's bur, not the run's");
    assert!(own(top) * 5 < (0..6).map(own).sum::<usize>());
    assert!(viewport_label(&h).contains(&format!("hovering flush cut {} on Row", top + 1)), "{}", viewport_label(&h));
    let spot = at(&h, top);
    click_at(&mut h, spot, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items, [Sel::Seat { path: vec![0], station: top as u32 }]);
    assert_eq!(h.state().status, format!("flush cut {} on Row", top + 1));
    // Its menu still chooses the run's layer.
    click_at(&mut h, spot, egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(h.query_by_label(&format!("Flush cut {} on Row", top + 1)).is_some(), "the menu names the station");
    h.get_by_label("Select layer \"Row\"").click();
    h.run_steps(3);
    assert_eq!(h.state().selected_layer, Some(0));
    assert_eq!(h.state().status, "Layer \"Row\" chosen in the Layers tool");
    // The next station round the ring answers as itself.
    let next = by_height[1];
    h.hover_at(at(&h, next));
    h.run_steps(3);
    assert_eq!(h.state().selection.hover.as_ref().map(|p| p.entity.clone()), Some(Entity::Seat { path: vec![0], station: next as u32 }));
    assert_eq!((lit(&h, 2.0), lit(&h, 1.0)), (own(next), own(top)), "the hovered station and the chosen one, each alone");
}

/// The Court band with a 1 mm post 2 mm tall joined at its top.
fn posted() -> ringdesign_core::RingDesign {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement};
    let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
    let post = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
    doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: post }).unwrap();
    d.cad = Some(doc);
    d
}

/// The middle of a box and the radius of the sphere round it, mm.
fn ball(b: (ringdesign_core::Vec3, ringdesign_core::Vec3)) -> ([f32; 3], f32) {
    let ext = [b.1.0 - b.0.0, b.1.1 - b.0.1, b.1.2 - b.0.2];
    ([(b.0.0 + b.1.0) * 0.5, (b.0.1 + b.1.1) * 0.5, (b.0.2 + b.1.2) * 0.5], 0.5 * (ext[0] * ext[0] + ext[1] * ext[1] + ext[2] * ext[2]).sqrt())
}

/// The box round every vertex of the built ring whose origin `owns` says it made.
fn made_box(b: &ringdesign_core::BuildResult, owns: impl Fn(u32) -> bool) -> (ringdesign_core::Vec3, ringdesign_core::Vec3) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for (v, o) in b.mesh.vertices.iter().zip(&b.mesh.origin) {
        if owns(*o) {
            for (k, x) in [v.0, v.1, v.2].into_iter().enumerate() {
                lo[k] = lo[k].min(x);
                hi[k] = hi[k].max(x);
            }
        }
    }
    (ringdesign_core::Vec3(lo[0], lo[1], lo[2]), ringdesign_core::Vec3(hi[0], hi[1], hi[2]))
}

/// Turns the pane's camera to `yaw`, `pitch` with the whole ring framed; the viewport's rect.
fn looking(h: &mut Harness<'static, RingDesignerApp>, pane: usize, yaw: f32, pitch: f32) -> egui::Rect {
    {
        let app = h.state_mut();
        let bounds = app.build.as_ref().and_then(|b| b.mesh.bounds());
        app.panes[pane].turn = None;
        let cam = &mut app.panes[pane].camera;
        cam.reset();
        cam.yaw = yaw;
        cam.pitch = pitch;
        cam.fit(bounds);
    }
    h.run_steps(3);
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// The point nearest the middle of `rect` where `misses` holds, clear of every one of `controls`.
fn background_in(rect: egui::Rect, controls: &[(&'static str, egui::Rect)], misses: impl Fn(egui::Pos2) -> bool) -> egui::Pos2 {
    let inner = rect.shrink(16.0);
    let mut spots: Vec<egui::Pos2> = (-80..=80)
        .flat_map(|i| (-80..=80).map(move |j| inner.center() + egui::vec2(i as f32, j as f32) * 6.0))
        .filter(|p| inner.contains(*p) && !controls.iter().any(|(_, r)| r.expand(8.0).contains(*p)))
        .collect();
    spots.sort_by(|a, b| (*a - inner.center()).length().total_cmp(&(*b - inner.center()).length()));
    spots.into_iter().find(|p| misses(*p)).expect("empty background in the view")
}

/// Where nothing picks in the Ring pane `pane`, clear of its navigator.
fn background(h: &Harness<'static, RingDesignerApp>, pane: usize) -> egui::Pos2 {
    let rect = h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect();
    let app = h.state();
    let scene = app.pick_scene.clone().expect("the ring's pick scene");
    let camera = app.panes[pane].camera;
    let controls = crate::viewport::navigator_controls(&h.ctx, Some(pane));
    assert!(controls.iter().any(|(n, _)| *n == "View cube"), "the navigator drew: {controls:?}");
    let ray = |p: egui::Pos2| camera.ray(rect, p);
    let every = ringdesign_core::interaction::pick::Filter::default();
    background_in(rect, &controls, |p| ringdesign_workbench::hover::pick_at(&scene, p, &ray, crate::viewport::APERTURE_PX, every).is_empty())
}

/// Steps until the pane's camera turn has arrived.
fn settle(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    let start = std::time::Instant::now();
    while h.state().panes[pane].turn.is_some() {
        h.run_steps(1);
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "the turn never arrived");
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    h.run_steps(2);
}

/// Fit view from the menu opened on empty background, the camera's turn let arrive.
fn fit_from_menu(h: &mut Harness<'static, RingDesignerApp>, pane: usize) {
    let at = background(h, pane);
    menu_row(h, at, None, "Fit view");
    settle(h, pane);
}

/// Whether every corner of `b` stands between the view's near and far planes.
fn within_depth(cam: &crate::camera::OrbitCamera, rect: egui::Rect, b: (ringdesign_core::Vec3, ringdesign_core::Vec3)) -> bool {
    let (m, _) = cam.matrices(rect);
    [b.0.0, b.1.0].into_iter().all(|x| [b.0.1, b.1.1].into_iter().all(|y| [b.0.2, b.1.2].into_iter().all(|z| (m[2] * x + m[6] * y + m[10] * z + m[14]).abs() < 1.0)))
}

#[test]
fn fit_view_frames_the_chosen_post_orbits_about_it_and_with_nothing_chosen_frames_the_ring() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, posted());
    let rect = looking(&mut h, pane, 0.3, 0.35);
    let build = h.state().build.clone().unwrap();
    let ring = build.mesh.bounds().unwrap();
    let post = build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.bounds().unwrap();
    let (post_mid, post_r) = ball(post);
    let (ring_mid, ring_r) = ball(ring);
    assert!(post_r < 2.0 && post_mid[1] > 9.0 && ring_r > 11.0, "a post {post_r} mm round at {post_mid:?} on a ring {ring_r} mm round");
    // A click on the post chooses it; Fit view from the menu frames it.
    let on_post = h.state().panes[pane].camera.projector(rect).at(post_mid);
    click_at(&mut h, on_post, egui::PointerButton::Primary, egui::Modifiers::NONE);
    let chosen = h.state().selection.items.clone();
    assert!(matches!(chosen.as_slice(), [Sel::Face { feature: 2, .. } | Sel::Part(2)]), "{chosen:?}");
    fit_from_menu(&mut h, pane);
    assert_eq!(h.state().status, "Fit view: Post, as chosen");
    let cam = h.state().panes[pane].camera;
    assert!((0..3).all(|k| (cam.target[k] - post_mid[k]).abs() < 1e-4), "the pivot is the post's middle: {:?}", cam.target);
    assert!(cam.pan[0].abs() < 1e-4 && cam.pan[1].abs() < 1e-4, "{:?}", cam.pan);
    assert!((cam.half_extent() - 1.15 * post_r).abs() < 1e-3, "the post's sphere fills the shorter side: {} for {post_r}", cam.half_extent());
    assert!((cam.projector(rect).at(post_mid) - rect.center()).length() < 0.5);
    assert!(within_depth(&cam, rect, ring), "the ring's far side is not clipped");
    // Chosen no more, a drag on the view orbits about the post: it holds the middle while the palm swings.
    h.state_mut().selection.items.clear();
    h.run_steps(2);
    let palm = [ring_mid[0], ring.0.1, ring_mid[2]];
    let palm_at = cam.projector(rect).at(palm);
    let from = rect.center() + egui::vec2(-rect.width() * 0.25, rect.height() * 0.3);
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run_steps(1);
    for k in 1..=8 {
        h.event(egui::Event::PointerMoved(from + egui::vec2(10.0 * k as f32, 3.0 * k as f32)));
        h.run_steps(1);
    }
    h.event(egui::Event::PointerButton { pos: from + egui::vec2(80.0, 24.0), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run_steps(2);
    let turned = h.state().panes[pane].camera;
    assert!((turned.yaw - cam.yaw).abs() > 0.3, "the drag orbited: yaw {} to {}", cam.yaw, turned.yaw);
    assert!((turned.projector(rect).at(post_mid) - rect.center()).length() < 0.5, "the post holds the middle of the view");
    let swung = (turned.projector(rect).at(palm) - palm_at).length();
    assert!(swung > 100.0, "the palm swings round the post: {swung} pt");
    // The cube's home view eases in about the ring's own middle again.
    let home = crate::viewport::navigator_controls(&h.ctx, Some(pane)).into_iter().find(|(n, _)| *n == "Home view").expect("the navigator's home view").1.center();
    click_at(&mut h, home, egui::PointerButton::Primary, egui::Modifiers::NONE);
    settle(&mut h, pane);
    let homed = h.state().panes[pane].camera;
    assert!((0..3).all(|k| (homed.target[k] - ring_mid[k]).abs() < 1e-4) && homed.pan == [0.0; 2], "{:?} {:?}", homed.target, homed.pan);
    assert!((homed.projector(rect).at(ring_mid) - rect.center()).length() < 0.5);
    // Nothing chosen: the whole ring, about its own middle.
    fit_from_menu(&mut h, pane);
    assert_eq!(h.state().status, "Fit view: the whole ring");
    let cam = h.state().panes[pane].camera;
    assert!((0..3).all(|k| (cam.target[k] - ring_mid[k]).abs() < 1e-4), "{:?} against {ring_mid:?}", cam.target);
    assert!((cam.half_extent() - 1.15 * ring_r).abs() < 1e-3 && (cam.zoom - 1.0).abs() < 1e-4, "{} {}", cam.half_extent(), cam.zoom);
    assert!((cam.projector(rect).at(ring_mid) - rect.center()).length() < 0.5);
}

#[test]
fn fit_view_frames_a_chosen_station_of_a_seat_run() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, flush_row());
    h.state_mut().selection.filter.stones = false;
    let rect = face(&mut h, pane, std::f32::consts::FRAC_PI_2);
    let frames = ringdesign_core::stones::stone_frames(&h.state().design);
    let top = (0..frames.len()).max_by(|a, b| frames[*a].1.girdle[1].total_cmp(&frames[*b].1.girdle[1])).unwrap();
    let spot = h.state().panes[pane].camera.projector(rect).at(frames[top].1.girdle.map(|v| v as f32));
    click_at(&mut h, spot, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().selection.items, [Sel::Seat { path: vec![0], station: top as u32 }]);
    fit_from_menu(&mut h, pane);
    assert_eq!(h.state().status, "Fit view: the seat, as chosen");
    // The station's own bur, not the run: a millimetre or two round, where the ring is 25 across.
    let build = h.state().build.clone().unwrap();
    let solids = &build.solids;
    let stations = ringdesign_core::interaction::pick::seat_stations(&solids.paths);
    let seat = made_box(&build, |o| solids.stone_of(o).is_some_and(|s| solids.paths[s] == [0] && stations[s] == top as u32));
    let (mid, r) = ball(seat);
    assert!(r < 2.5 && r > 0.5, "one station's bur is {r} mm round");
    let cam = h.state().panes[pane].camera;
    assert!((0..3).all(|k| (cam.target[k] - mid[k]).abs() < 1e-4), "{:?} against {mid:?}", cam.target);
    assert!((cam.half_extent() - 1.15 * r).abs() < 1e-3, "{} for {r}", cam.half_extent());
    assert!((cam.projector(rect).at(mid) - rect.center()).length() < 0.5);
    assert!(within_depth(&cam, rect, build.mesh.bounds().unwrap()), "the ring's far side is not clipped");
}


/// Whether `a` and `b` agree to within `tol` on every axis.
fn near(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < tol)
}

/// The Ring viewport's rect as last drawn.
fn ring_rect(h: &Harness<'static, RingDesignerApp>) -> egui::Rect {
    h.query_all_by_label_contains("Ring viewport").next().expect("the Ring viewport").rect()
}

/// The construction guide's band carried through its signet head, with the post at its top.
fn guided_post() -> ringdesign_core::RingDesign {
    use ringdesign_core::construction as recipe;
    let mut d = recipe::blank();
    recipe::apply(&mut d, 0, 1.0).unwrap();
    recipe::apply(&mut d, 1, 1.0).unwrap();
    d.cad = posted().cad;
    d
}

#[test]
fn a_construction_view_and_a_signet_view_after_a_fit_centre_the_ring_again() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, guided_post());
    assert_eq!(h.state().design.shank.kind, ringdesign_core::ShankKind::Signet);
    looking(&mut h, pane, 0.3, 0.35);
    let build = h.state().build.clone().unwrap();
    let (ring_mid, _) = ball(build.mesh.bounds().unwrap());
    let (post_mid, _) = ball(build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.bounds().unwrap());
    assert!(!near(post_mid, ring_mid, 5.0), "the post stands off the ring's middle: {post_mid:?} {ring_mid:?}");
    h.state_mut().selection.items = vec![Sel::Part(2)];
    fit_from_menu(&mut h, pane);
    assert_eq!(h.state().status, "Fit view: Post, as chosen");
    assert!(near(h.state().panes[pane].camera.target, post_mid, 1e-4));
    // The guide's Seal view looks at the head about the ring's middle, not about the post.
    h.state_mut().construction.open = true;
    h.run_steps(3);
    h.get_by_label("Continue this design").click();
    h.run_steps(3);
    h.query_all_by_label("Seal").find(|n| n.rect().right() < 400.0).expect("the guide's Seal view").click();
    h.run_steps(3);
    let rect = ring_rect(&h);
    let cam = h.state().panes[pane].camera;
    assert!(h.state().panes[pane].turn.is_none());
    assert!(near(cam.target, ring_mid, 1e-4) && cam.pan == [0.0; 2] && cam.zoom == 1.23, "{:?} {:?} {}", cam.target, cam.pan, cam.zoom);
    assert!((cam.projector(rect).at(ring_mid) - rect.center()).length() < 0.5);
    // Framed on the post again, the pane's own Signet 3/4 centres the ring the same way.
    fit_from_menu(&mut h, pane);
    assert!(near(h.state().panes[pane].camera.target, post_mid, 1e-4));
    h.query_by_label("Camera").or_else(|| h.query_by_label("Cam")).expect("the pane's Camera menu").click();
    h.run_steps(3);
    h.get_by_label("Signet 3/4").click();
    h.run_steps(3);
    let rect = ring_rect(&h);
    let cam = h.state().panes[pane].camera;
    assert!(h.state().panes[pane].turn.is_none());
    assert!(near(cam.target, ring_mid, 1e-4) && cam.pan == [0.0; 2], "{:?} {:?}", cam.target, cam.pan);
    assert!((cam.projector(rect).at(ring_mid) - rect.center()).length() < 0.5);
}

#[test]
fn a_design_opened_after_a_fit_opens_whole_at_zoom_one_about_its_middle() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, posted());
    looking(&mut h, pane, 0.3, 0.35);
    h.state_mut().selection.items = vec![Sel::Part(2)];
    fit_from_menu(&mut h, pane);
    assert_eq!(h.state().status, "Fit view: Post, as chosen");
    assert!(h.state().panes[pane].camera.zoom > 8.0, "framed close on the post: {}", h.state().panes[pane].camera.zoom);
    // A pan taken about the post, then a new ring from a template.
    h.state_mut().panes[pane].camera.pan = [0.8, -0.5];
    let tide = ringdesign_workbench::templates::collections().iter().flat_map(|c| c.templates.iter()).find(|t| t.slug == "tide-workshop").expect("the Tide template");
    crate::export::load_catalog_template(h.state_mut(), tide);
    wait_for_template(&mut h);
    assert!(h.state().fit_pending);
    h.state_mut().rebuild_now();
    wait_for_build(&mut h);
    h.run_steps(2);
    assert!(h.state().design.name.starts_with("Tide"), "{}", h.state().design.name);
    let ring = h.state().build.as_ref().unwrap().mesh.bounds().unwrap();
    let (mid, r) = ball(ring);
    let rect = ring_rect(&h);
    let cam = h.state().panes[pane].camera;
    assert!(h.state().panes[pane].turn.is_none());
    assert_eq!((cam.zoom, cam.pan), (1.0, [0.0; 2]));
    assert!(near(cam.target, mid, 1e-4), "{:?} against {mid:?}", cam.target);
    assert!((cam.half_extent() - 1.15 * r).abs() < 1e-3, "{} for {r}", cam.half_extent());
    for x in [ring.0.0, ring.1.0] {
        for y in [ring.0.1, ring.1.1] {
            for z in [ring.0.2, ring.1.2] {
                assert!(rect.contains(cam.projector(rect).at([x, y, z])), "({x}, {y}, {z}) in view");
            }
        }
    }
    assert!(within_depth(&cam, rect, ring));
}

#[test]
fn an_applied_operation_after_a_fit_keeps_the_post_on_screen_about_the_rings_middle() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, guided_post());
    looking(&mut h, pane, 0.3, 0.35);
    h.state_mut().selection.items = vec![Sel::Part(2)];
    fit_from_menu(&mut h, pane);
    let build = h.state().build.clone().unwrap();
    let (ring_mid, _) = ball(build.mesh.bounds().unwrap());
    let (post_mid, _) = ball(build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.bounds().unwrap());
    let before = h.state().panes[pane].camera;
    assert!(before.zoom > 8.0 && near(before.target, post_mid, 1e-4) && !near(before.target, ring_mid, 5.0), "framed about the post: {} {:?}", before.zoom, before.target);
    // With no view chosen, the guide's next operation lands and keeps the reader's framing.
    h.state_mut().selection.items.clear();
    h.state_mut().construction.open = true;
    h.run_steps(3);
    h.get_by_label("Continue this design").click();
    h.run_steps(3);
    h.get_by_label("Apply operation").click();
    h.run_steps(3);
    assert!(h.state().fit_pending && h.state().fit_keeps_view);
    land(&mut h);
    let build = h.state().build.clone().unwrap();
    let (applied, _) = ball(build.mesh.bounds().unwrap());
    let (post, _) = ball(build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.bounds().unwrap());
    let rect = ring_rect(&h);
    let cam = h.state().panes[pane].camera;
    assert_eq!((cam.zoom, cam.yaw, cam.pitch), (before.zoom, before.yaw, before.pitch));
    assert!(near(cam.target, applied, 1e-4), "it orbits the ring's middle again: {:?} against {applied:?}", cam.target);
    // The post stays where the reader framed it, and the empty bore is nowhere near the middle of the view.
    let at = cam.projector(rect).at(post);
    assert!((at - rect.center()).length() < 0.2 * rect.height(), "the post at {at:?} in {rect:?}");
    assert!((cam.projector(rect).at(applied) - rect.center()).length() > rect.height(), "the ring's middle stands off screen");
}

#[test]
fn a_new_design_opened_while_a_fit_is_still_turning_opens_whole_and_stays() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, posted());
    looking(&mut h, pane, 0.3, 0.35);
    h.state_mut().selection.items = vec![Sel::Part(2)];
    crate::viewport::fit_view(h.state_mut(), pane);
    let turn = h.state().panes[pane].turn.expect("Fit view turns toward the post");
    assert!(turn.to.zoom > 8.0, "{}", turn.to.zoom);
    crate::panels::Command::New.run(h.state_mut());
    h.state_mut().rebuild_now();
    // Every frame until the build lands starts with the Fit still turning.
    let start = std::time::Instant::now();
    while h.state().is_building() {
        h.state_mut().panes[pane].turn = Some(ringdesign_workbench::focus::Turn::new(turn.from, turn.to));
        h.run_steps(1);
        assert!(start.elapsed() < std::time::Duration::from_secs(30), "the ring never built");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let whole = |h: &Harness<'static, RingDesignerApp>| {
        let (mid, r) = ball(h.state().build.as_ref().unwrap().mesh.bounds().unwrap());
        let cam = h.state().panes[pane].camera;
        assert!(h.state().panes[pane].turn.is_none(), "the landing stops the turn");
        assert_eq!((cam.zoom, cam.pan), (1.0, [0.0; 2]));
        assert!(near(cam.target, mid, 1e-4) && (cam.half_extent() - 1.15 * r).abs() < 1e-3, "{:?} against {mid:?}, {} for {r}", cam.target, cam.half_extent());
    };
    whole(&h);
    // Past the turn's own length nothing carries the view back toward the post.
    std::thread::sleep(std::time::Duration::from_secs_f32(ringdesign_workbench::focus::Turn::SECONDS + 0.1));
    h.run_steps(4);
    whole(&h);
}

#[test]
fn a_cylinder_clicked_out_on_the_spot_takes_the_tools_default_size() {
    use ringdesign_core::cad::Operation;
    let mut h = harness();
    let court = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let r = court.inner_radius_mm() + court.profile.thickness_mm;
    let pane = on_one_ring_view(&mut h, court);
    let rect = looking(&mut h, pane, std::f32::consts::FRAC_PI_2, 0.0);
    let a = 70f64.to_radians();
    let at = h.state().panes[pane].camera.projector(rect).at([(r * a.cos()) as f32, (r * a.sin()) as f32, 0.0]);
    h.hover_at(at);
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::A);
    h.run_steps(3);
    h.get_by_label("Cylinder").click();
    h.run_steps(3);
    assert_eq!(h.state().command.session.command().map(|c| c.key()), Some("add-cylinder"));
    // The base clicked, then the size and the height clicked a point off it: both stay the tool's own.
    click_at(&mut h, at, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().command.session.command().map(|c| c.step()), Some(1), "the click seated the base");
    let nudged = at + egui::vec2(1.0, 0.0);
    h.hover_at(nudged);
    h.run_steps(2);
    click_at(&mut h, nudged, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().command.session.command().map(|c| c.step()), Some(2), "the second click fixed the radius");
    click_at(&mut h, nudged, egui::PointerButton::Primary, egui::Modifiers::NONE);
    assert_eq!(h.state().command.session.command().map(|c| c.key()), None);
    let cyl = h.state().design.cad.as_ref().and_then(|d| d.features.iter().find(|f| f.name == "Cylinder")).cloned().expect("the cylinder");
    assert!(matches!(cyl.operation, Operation::Cylinder { radius_mm, height_mm } if radius_mm == 1.0 && height_mm == 1.0), "{:?}", cyl.operation);
}

/// Rebuilds now and steps until the build has landed.
fn land(h: &mut Harness<'static, RingDesignerApp>) {
    h.state_mut().rebuild_now();
    wait_for_build(h);
    h.run_steps(2);
}

#[test]
fn the_construction_guides_rebuilds_keep_the_view_it_set_and_the_readers_zoom() {
    use ringdesign_workbench::viewport::Sel;
    let mut h = harness();
    let pane = on_one_ring_view(&mut h, posted());
    looking(&mut h, pane, 0.3, 0.35);
    h.state_mut().selection.items = vec![Sel::Part(2)];
    fit_from_menu(&mut h, pane);
    assert!(h.state().panes[pane].camera.zoom > 8.0, "framed close on the post: {}", h.state().panes[pane].camera.zoom);
    h.state_mut().selection.items.clear();
    // Started from a blank band, the guide's own 3/4 view at 1.23 holds when the band lands, centred on it.
    h.state_mut().construction.open = true;
    h.run_steps(3);
    h.get_by_label("Start from a blank band").click();
    h.run_steps(3);
    assert!(h.state().fit_pending && h.state().fit_keeps_view);
    land(&mut h);
    let (mid, _) = ball(h.state().build.as_ref().unwrap().mesh.bounds().unwrap());
    let rect = ring_rect(&h);
    let cam = h.state().panes[pane].camera;
    assert!(!h.state().fit_pending && !h.state().fit_keeps_view);
    assert_eq!((cam.zoom, cam.pan), (1.23, [0.0; 2]));
    assert!(near(cam.target, mid, 1e-4), "{:?} against {mid:?}", cam.target);
    assert!((cam.projector(rect).at(mid) - rect.center()).length() < 0.5);
    // The Cheek view, then the reader's own zoom and pan: an applied operation keeps both, about the new band's middle.
    h.query_all_by_label("Cheek").find(|n| n.rect().right() < 400.0).expect("the guide's Cheek view").click();
    h.run_steps(3);
    assert_eq!((h.state().panes[pane].camera.zoom, h.state().panes[pane].camera.pitch), (1.23, -1.30));
    h.state_mut().panes[pane].camera.zoom = 2.5;
    h.state_mut().panes[pane].camera.pan = [0.8, -0.5];
    h.get_by_label("Apply operation").click();
    h.run_steps(3);
    assert!(h.state().fit_pending && h.state().fit_keeps_view);
    land(&mut h);
    let (applied, _) = ball(h.state().build.as_ref().unwrap().mesh.bounds().unwrap());
    let cam = h.state().panes[pane].camera;
    assert_eq!((cam.zoom, cam.pan, cam.pitch), (2.5, [0.8, -0.5], -1.30));
    assert!(near(cam.target, applied, 1e-4), "{:?} against {applied:?}", cam.target);
    // A new design started before the guide's rebuild lands opens whole all the same.
    h.get_by_label("Apply settings").click();
    h.run_steps(3);
    assert!(h.state().fit_keeps_view);
    crate::panels::Command::New.run(h.state_mut());
    assert!(h.state().fit_pending && !h.state().fit_keeps_view);
    land(&mut h);
    let (fresh, _) = ball(h.state().build.as_ref().unwrap().mesh.bounds().unwrap());
    let cam = h.state().panes[pane].camera;
    assert_eq!((cam.zoom, cam.pan), (1.0, [0.0; 2]));
    assert!(near(cam.target, fresh, 1e-4), "{:?} against {fresh:?}", cam.target);
}

/// Steps until the CAD pane has evaluated and staged its view.
fn wait_for_cad(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    while h.state().cad.edge_runs().1.is_none() {
        h.run_steps(3);
        assert!(start.elapsed() < std::time::Duration::from_secs(30), "the CAD pane never evaluated");
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    h.run_steps(2);
}

/// Steps until the CAD canvas's camera turn has arrived.
fn settle_cad(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    while h.state().cad.view_camera().1 {
        h.run_steps(1);
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "the turn never arrived");
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    h.run_steps(2);
}

/// Fit view from the CAD canvas's menu opened where neither the ring nor anything within 8 points of it is.
fn cad_fit_from_menu(h: &mut Harness<'static, RingDesignerApp>, ring: &ringdesign_core::Mesh) {
    let (canvas, _) = h.state().cad.canvas_scale();
    let (cam, _) = h.state().cad.view_camera();
    let controls = crate::viewport::navigator_controls(&h.ctx, None);
    assert!(controls.iter().any(|(n, _)| *n == "View cube"), "the navigator drew: {controls:?}");
    let clear = |p: egui::Pos2| {
        [egui::vec2(0.0, 0.0), egui::vec2(8.0, 0.0), egui::vec2(-8.0, 0.0), egui::vec2(0.0, 8.0), egui::vec2(0.0, -8.0)].iter().all(|d| {
            let (o, dir) = cam.ray(canvas, p + *d);
            ringdesign_core::interaction::picking::raycast(ring, o, dir).is_none()
        })
    };
    let at = background_in(canvas, &controls, clear);
    menu_row(h, at, None, "Fit view");
    settle_cad(h);
}

/// The Fit button on the CAD pane's footer, settled.
fn cad_footer_fit(h: &mut Harness<'static, RingDesignerApp>) {
    let (canvas, _) = h.state().cad.canvas_scale();
    h.query_all_by_label("Camera and display").find(|n| n.rect().top() > canvas.bottom()).expect("the footer's Fit").click();
    h.run_steps(2);
    settle_cad(h);
}

#[test]
fn the_cad_panes_fit_view_frames_the_part_a_history_pick_became() {
    use ringdesign_core::cad::{Attach, Component, Feature, Operation, Placement};
    let mut h = sized([1600., 980.]);
    {
        let app = h.state_mut();
        let mut d = posted();
        let doc = d.cad.as_mut().unwrap();
        let stud = Component { attach: Attach::Separate, placement: Placement::ring(270.0, 3.0), ..Component::default() };
        doc.append(Feature { id: 3, name: "Stud".into(), enabled: true, operation: Operation::Box { size: [1.0; 3] }, component: stud.clone() }).unwrap();
        doc.append(Feature { id: 4, name: "Stud raised".into(), enabled: true, operation: Operation::Transform { source: 3, translation: [0.0, 0.0, 1.0], rotation_deg: [0.0; 3] }, component: stud }).unwrap();
        app.design = d;
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(&mut h);
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    wait_for_cad(&mut h);
    assert!(h.state().cad.drawn_bounds(3).is_none(), "the box is consumed by its move");
    let (mid, r) = ball(h.state().cad.drawn_bounds(4).expect("the raised stud as drawn"));
    let build = h.state().build.clone().unwrap();
    let (ring_mid, _) = ball(build.mesh.bounds().unwrap());
    // The box chosen in the history frames the stud it became, from the canvas's menu.
    h.state_mut().cad.choose_feature(3);
    cad_fit_from_menu(&mut h, &build.mesh);
    assert_eq!(h.state().status, "Fit view: Stud raised, as chosen");
    let (cam, _) = h.state().cad.view_camera();
    assert!(near(cam.target, mid, 1e-4) && cam.pan[0].abs() < 1e-4 && cam.pan[1].abs() < 1e-4, "{:?} {:?} against {mid:?}", cam.target, cam.pan);
    assert!((cam.half_extent() - 1.15 * r).abs() < 1e-3, "{} for {r}", cam.half_extent());
    // The footer's Fit brings all the metal back with the box still chosen.
    cad_footer_fit(&mut h);
    assert_eq!(h.state().status, "Fit view: the whole ring");
    let (cam, _) = h.state().cad.view_camera();
    assert!((cam.zoom - 1.0).abs() < 1e-4 && near(cam.target, ring_mid, 0.05), "{} {:?} against {ring_mid:?}", cam.zoom, cam.target);
}

#[test]
fn the_cad_panes_fit_view_frames_the_chosen_part_else_all_the_metal_shown() {
    let mut h = sized([1600., 980.]);
    {
        let app = h.state_mut();
        app.design = posted();
        app.history.commit(&app.design);
        app.rebuild_now();
    }
    wait_for_build(&mut h);
    let build = h.state().build.clone().unwrap();
    let ring = build.mesh.bounds().unwrap();
    let (ring_mid, _) = ball(ring);
    let (post_mid, _) = ball(build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().mesh.bounds().unwrap());
    h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
    wait_for_cad(&mut h);
    let drawn = h.state().cad.drawn_bounds(2).expect("the post as drawn");
    let (mid, r) = ball(drawn);
    assert!(near(mid, post_mid, 0.05) && r < 2.0, "the drawn post is the post: {mid:?} against {post_mid:?}, {r} mm round");
    // The chosen post, framed as the Ring viewport frames it.
    h.state_mut().cad.choose_feature(2);
    cad_fit_from_menu(&mut h, &build.mesh);
    assert_eq!(h.state().status, "Fit view: Post, as chosen");
    let (canvas, _) = h.state().cad.canvas_scale();
    let (cam, _) = h.state().cad.view_camera();
    assert!(near(cam.target, mid, 1e-4) && cam.pan[0].abs() < 1e-4 && cam.pan[1].abs() < 1e-4, "{:?} {:?}", cam.target, cam.pan);
    assert!((cam.half_extent() - 1.15 * r).abs() < 1e-3, "the post's sphere fills the shorter side: {} for {r}", cam.half_extent());
    assert!((cam.projector(canvas).at(mid) - canvas.center()).length() < 0.5);
    assert!(within_depth(&cam, canvas, ring), "the ring's far side is not clipped");
    let framed = cam.zoom;
    // A named view from the pane's View menu turns about the ring's middle again, the framing's zoom kept.
    h.query_all_by_label("View").find(|n| n.rect().top() > canvas.bottom()).expect("the pane's View menu").click();
    h.run_steps(3);
    h.get_by_label("Signet face").click();
    h.run_steps(2);
    settle_cad(&mut h);
    let (cam, _) = h.state().cad.view_camera();
    assert!(near(cam.target, ring_mid, 0.05) && cam.pan[0].abs() < 1e-4 && cam.pan[1].abs() < 1e-4, "{:?} {:?} against {ring_mid:?}", cam.target, cam.pan);
    assert!((cam.zoom - framed).abs() < 1e-4);
    // The footer's Fit brings all the metal back, the post still chosen: zoom 1 about its middle.
    cad_footer_fit(&mut h);
    assert_eq!(h.state().status, "Fit view: the whole ring");
    let (cam, _) = h.state().cad.view_camera();
    assert!((cam.zoom - 1.0).abs() < 1e-4 && cam.pan[0].abs() < 1e-4 && cam.pan[1].abs() < 1e-4 && near(cam.target, ring_mid, 0.05), "{} {:?} {:?}", cam.zoom, cam.pan, cam.target);
    // Zoomed in by the wheel, then with the band chosen the menu's Fit view frames all the metal again.
    h.hover_at(canvas.center());
    h.run_steps(1);
    h.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, 240.0), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE });
    let start = std::time::Instant::now();
    while h.state().cad.view_camera().0.zoom < 1.2 {
        h.run_steps(1);
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "the wheel never zoomed: {}", h.state().cad.view_camera().0.zoom);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.state_mut().cad.choose_feature(1);
    cad_fit_from_menu(&mut h, &build.mesh);
    assert_eq!(h.state().status, "Fit view: the whole ring");
    let (cam, _) = h.state().cad.view_camera();
    assert!((cam.zoom - 1.0).abs() < 1e-4 && cam.pan[0].abs() < 1e-4 && cam.pan[1].abs() < 1e-4 && near(cam.target, ring_mid, 0.05), "{} {:?} {:?}", cam.zoom, cam.pan, cam.target);
}
