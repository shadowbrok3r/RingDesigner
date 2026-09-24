use crate::{
    app::RingDesignerApp,
    dock::{Desktop, ToolKind},
    pane::{Layout, PaneKind},
};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn harness(size: [f32; 2]) -> Harness<'static, RingDesignerApp> {
    let builder = Harness::builder().with_size(size);
    #[cfg(feature = "ui-shot")]
    let builder = builder.wgpu();
    builder.build_eframe(|cc| {
        crate::theme::install(&cc.egui_ctx);
        let mut app = RingDesignerApp::new(cc);
        app.updater.automatic = false;
        app.auto_rebuild = false;
        app
    })
}
#[test]
fn graph_opens_in_visible_pane_and_restores_model_workspace() {
    let mut h = harness([1600., 980.]);
    h.state_mut().layout = Layout::Single;
    h.state_mut().panes[3].kind = PaneKind::Graph;
    let width = h.state().dock.left_width;
    h.state_mut().show_graph_pane();
    assert_eq!(h.state().desktop, Desktop::Graph);
    assert_eq!(h.state().panes[h.state().active_pane].kind, PaneKind::Graph);
    assert!(h.state().active_pane < h.state().layout.count());
    assert!(h.state().dock.is_open(ToolKind::Node));
    assert!(!h.state().dock.is_open(ToolKind::Design));
    h.state_mut().dock.right_width = 480.;
    h.state_mut().switch_desktop(Desktop::Model);
    assert_eq!(h.state().dock.left_width, width);
    assert!(h.state().dock.is_open(ToolKind::Design));
    h.state_mut().switch_desktop(Desktop::Graph);
    assert_eq!(h.state().dock.right_width, 480.);
}
#[test]
fn compact_toolbar_and_viewport_actions_are_reachable_at_minimum_window() {
    let mut h = harness([1100., 700.]);
    h.run_steps(2);
    for text in [
        "File",
        "View",
        "Model workspace",
        "Tools",
        "Export",
        "Preview",
        "Updates",
    ] {
        let node = h.get_by_label(text);
        let bounds = node.rect();
        assert!(
            bounds.right() <= 1100. && bounds.top() < 48.,
            "{text}: {bounds:?}"
        );
    }
    h.get_by_label("View").hover();
    h.run_steps(2);
    h.get_by_label("View").click();
    // Popup areas need a sizing frame before their controls become interactive.
    h.run_steps(3);
    h.get_by_label_contains("Viewport layout").click();
    h.run_steps(3);
    for text in ["Single", "Split Vertical", "Split Horizontal", "Four"] {
        assert!(
            h.get_all_by_label(text)
                .any(|node| node.rect().top() < 250.)
        );
    }
    // Layout controls also exist in the viewport footer; choose the open menu.
    h.get_all_by_label("Four")
        .into_iter()
        .find(|node| node.rect().top() < 250.)
        .unwrap()
        .click();
    h.run_steps(3);
    assert_eq!(h.state().layout, Layout::Quad);
}
#[test]
fn escape_closes_the_menu_before_clearing_selection() {
    let mut h = harness([1100., 700.]);
    h.state_mut().probe = Some(([0.; 3], "Selected feature".into()));
    h.run_steps(3);
    h.get_by_label("View").click();
    h.run_steps(3);
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    assert!(h.state().probe.is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().probe.is_none());
}

#[test]
fn clearing_selection_does_not_change_the_design() {
    let mut h = harness([1600., 980.]);
    let before = serde_json::to_value(&h.state().design).unwrap();
    h.state_mut().selected_layer = Some(0);
    h.state_mut().probe = Some(([0.; 3], "selected".into()));
    h.state_mut()
        .visual
        .select(ringdesign_workbench::visual::Tool::Paint);
    h.state_mut().clear_selection();
    assert!(h.state().selected_layer.is_none());
    assert!(h.state().probe.is_none());
    assert_eq!(
        h.state().visual.tool,
        ringdesign_workbench::visual::Tool::Select
    );
    assert_eq!(before, serde_json::to_value(&h.state().design).unwrap());
}
#[test]
fn workspace_round_trip_keeps_each_desktop_and_update_preference() {
    let mut h = harness([1600., 980.]);
    h.state_mut().switch_desktop(Desktop::Graph);
    h.state_mut().switch_desktop(Desktop::Surface);
    let json = serde_json::to_string(&h.state().workspace()).unwrap();
    let restored: crate::app::Workspace = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.desktop, Desktop::Surface);
    assert_eq!(restored.desktops[&Desktop::Graph].dock.right_width, 420.);
    assert!(!restored.automatic_updates);
}

fn pane_rect(h: &Harness<'_, RingDesignerApp>, index: usize) -> egui::Rect {
    let tree = &h.state().viewport_layout.tree;
    let id = tree
        .tiles
        .iter()
        .find_map(|(id, tile)| {
            matches!(tile, egui_tiles::Tile::Pane(i) if *i == index).then_some(*id)
        })
        .unwrap();
    tree.tiles.rect(id).unwrap()
}

fn drag(h: &mut Harness<'_, RingDesignerApp>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.run_steps(2);
    h.drag_at(from);
    h.run_steps(2);
    h.hover_at(to);
    h.run_steps(2);
    h.drop_at(to);
    h.run_steps(3);
}

#[test]
fn graph_preview_splits_resize_and_survive_workspace_round_trip() {
    let mut h = harness([1600., 980.]);
    h.state_mut().show_graph_pane();
    h.run_steps(3);
    assert_eq!(h.state().layout, Layout::GraphReview);
    let top = pane_rect(&h, 0);
    let bottom = pane_rect(&h, 1);
    let graph = pane_rect(&h, 2);
    let graph_share = graph.width() / (top.width() + graph.width());
    assert!((graph_share - 0.65).abs() < 0.02, "{graph_share}");
    assert!(h.state().panes[1].follow_node);
    assert!(h.state().panes[1].navigation.locked);
    let vertical = egui::pos2((top.right() + graph.left()) * 0.5, top.center().y);
    drag(&mut h, vertical, vertical + egui::vec2(90., 0.));
    assert!(pane_rect(&h, 0).width() > top.width() + 60.);
    let horizontal = egui::pos2(top.center().x, (top.bottom() + bottom.top()) * 0.5);
    drag(&mut h, horizontal, horizontal + egui::vec2(0., 65.));
    assert!(pane_rect(&h, 0).height() > top.height() + 40.);
    let resized = pane_rect(&h, 0);
    h.state_mut().switch_desktop(Desktop::Model);
    h.run_steps(3);
    h.state_mut().switch_desktop(Desktop::Graph);
    h.run_steps(3);
    assert!((pane_rect(&h, 0).size() - resized.size()).abs().max_elem() < 2.);
    let encoded = serde_json::to_string(&h.state().workspace()).unwrap();
    let restored: crate::app::Workspace = serde_json::from_str(&encoded).unwrap();
    assert!(
        restored
            .viewport_layout
            .unwrap()
            .valid_for(Layout::GraphReview)
    );
    h.state_mut().set_layout(Layout::SplitV);
    assert!(!h.state().panes[1].follow_node);
    assert!(!h.state().panes[1].navigation.locked);
}

#[test]
fn graph_view_defaults_to_inspector_and_edit_restores_inline_fields() {
    let mut h = harness([1600., 980.]);
    h.state_mut()
        .open_graph(ringdesign_graph::templates::simple());
    h.run_steps(3);
    assert!(!h.state().graph_inline_edit);
    assert!(!h.state().graph_ed.as_ref().unwrap().inline_inputs);
    let inputs = h
        .state()
        .graph_ed
        .as_ref()
        .unwrap()
        .graph()
        .nodes
        .iter()
        .map(|n| (n.id, n.inputs.clone()))
        .collect::<Vec<_>>();
    h.get_by_label("Edit").click();
    h.run_steps(3);
    assert!(h.state().graph_ed.as_ref().unwrap().inline_inputs);
    h.get_all_by_label("View")
        .find(|n| n.rect().top() > 48.)
        .unwrap()
        .click();
    h.run_steps(3);
    assert!(!h.state().graph_ed.as_ref().unwrap().inline_inputs);
    let after = h
        .state()
        .graph_ed
        .as_ref()
        .unwrap()
        .graph()
        .nodes
        .iter()
        .map(|n| (n.id, n.inputs.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        inputs, after,
        "Changing graph presentation must not edit parameter values"
    );
}

#[test]
fn toolbar_and_viewport_buttons_share_one_height() {
    let mut h = harness([1600., 980.]);
    h.run_steps(3);
    let expected = h.ctx.global_style().spacing.interact_size.y;
    for label in [
        "File",
        "View",
        "Model workspace",
        "Tools",
        "History",
        "Undo",
        "Redo",
        "Select",
        "Display",
    ] {
        let rect = h.get_by_label(label).rect();
        assert!(
            (rect.height() - expected).abs() < 0.5,
            "{label}: {rect:?}; expected {expected}"
        );
    }
}

#[test]
fn file_menu_has_preview_collections_and_opens_the_selected_template() {
    let mut h = harness([1600., 980.]);
    h.state_mut().document_path = Some("old-project.ring.json".into());
    h.run_steps(3);
    h.get_by_label("File").click();
    h.run_steps(3);
    h.get_by_label_contains("New from template").click();
    h.run_steps(3);
    for collection in ringdesign_workbench::templates::collections() {
        assert!(h.query_by_label_contains(collection.name).is_some(), "{} missing", collection.name);
    }
    h.get_by_label_contains("Reptilia collection").click();
    h.run_steps(3);
    let before = h.state().design.name.clone();
    h.get_by_label("Ecdysis — ventral scales").click();
    h.run_steps(1);
    // The frame that chose it returns at once: the design on screen stays while a plate says how far the template has got.
    assert!(h.state().opening.is_some());
    assert_eq!(h.state().design.name, before);
    h.run_steps(1);
    assert!(h.state().opening.is_some());
    assert!(h.query_by_label_contains("Opening Ecdysis — ventral scales: ").is_some());
    crate::interaction_tests::wait_for_template(&mut h);
    assert!(h.state().design.name.starts_with("Ecdysis"));
    // Landed, it builds at once and says so until the build shows it.
    assert!(h.state().is_building() && h.state().opened_building.is_some());
    assert!(h.query_by_label("Opening Ecdysis — ventral scales: building the ring").is_some());
    crate::interaction_tests::wait_for_build(&mut h);
    h.run_steps(2);
    assert!(h.state().opened_building.is_none() && h.query_by_label_contains("Opening Ecdysis").is_none());
    assert!(h.state().design.graph.is_some());
    assert!(h.state().document_path.is_none());
    assert!(!h.state().graph_ed.as_ref().unwrap().graph().nodes.iter().any(|n| n.kind == "head"));
}

#[test]
fn active_document_title_is_centered_and_file_identity_persists() {
    for width in [1100., 1600.] {
        let mut h = harness([width, 980.]);
        h.state_mut().design.name = "Ophidian — amethyst serpent".into();
        h.run_steps(3);
        let rect = h.get_by_label("Ophidian — amethyst serpent").rect();
        assert!((rect.center().x - width * 0.5).abs() < 3., "{width}: {rect:?}");
        assert!(rect.left() > h.get_by_label("History").rect().right());
        assert!(rect.right() < h.get_by_label("Preview").rect().left());
        h.state_mut().document_path = Some("/tmp/custom-serpent.ring.json".into());
        h.run_steps(3);
        assert!(h.query_by_label("custom-serpent.ring.json").is_some());
        let saved: crate::app::Workspace = serde_json::from_str(&serde_json::to_string(&h.state().workspace()).unwrap()).unwrap();
        assert_eq!(saved.document_path, h.state().document_path);
    }
}

#[test]
fn command_search_covers_viewport_navigation_in_split_layout() {
    let mut h = harness([1100., 700.]);
    h.state_mut().set_layout(Layout::Quad);
    for pane in &mut h.state_mut().panes {
        pane.kind = PaneKind::Solid;
    }
    h.state_mut().palette_open = true;
    h.run_steps(4);
    let heading = h.get_by_label("Command search").rect();
    let layer = h.ctx.layer_id_at(heading.center()).unwrap();
    assert_eq!(layer.id, egui::Id::new("command-palette-modal"));
    assert_eq!(layer.order, egui::Order::Foreground);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!h.state().palette_open);
}

#[test]
fn surface_editing_explains_graph_ownership_and_offers_an_edit_route() {
    let mut h = harness([1600., 980.]);
    h.state_mut()
        .open_graph(ringdesign_graph::templates::simple());
    h.state_mut().set_layout(Layout::SplitV);
    h.state_mut().panes[0].kind = PaneKind::Unrolled;
    h.state_mut().band_paint = true;
    h.run_steps(3);
    assert!(!h.state().band_paint);
    assert!(
        h.state()
            .surface_edit_reason()
            .unwrap()
            .contains("generated by nodes")
    );
    h.get_by_label("Edit graph");
    h.get_by_label("Make editable");
    h.state_mut().switch_desktop(Desktop::Surface);
    assert!(
        h.state().surface_edit_reason().is_some(),
        "Changing workspace must not silently detach a graph"
    );
}

#[test]
fn feedback_starts_with_submission_disabled_and_escape_preserves_selection() {
    let mut h = harness([1100., 700.]);
    h.state_mut().probe = Some(([0.; 3], "Selected feature".into()));
    ringdesign_workbench::feedback::open(&h.ctx);
    h.run_steps(3);
    assert!(
        h.get_by_label("Review on GitHub")
            .accesskit_node()
            .is_disabled()
    );
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!ringdesign_workbench::feedback::is_open(&h.ctx));
    assert!(h.state().probe.is_some());
}
#[cfg(feature = "ui-shot")]
#[test]
fn workspace_review_images() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/workspace-review");
    std::fs::create_dir_all(&root).unwrap();
    let mut h = harness([1600., 980.]);
    h.run_steps(3);
    h.render()
        .unwrap()
        .save(root.join("desktop-model.png"))
        .unwrap();
    h.state_mut()
        .open_graph(ringdesign_graph::templates::simple());
    h.run_steps(3);
    h.get_by_label(">").click();
    h.run_steps(3);
    h.render()
        .unwrap()
        .save(root.join("desktop-graph.png"))
        .unwrap();
    h.set_size(egui::vec2(1100., 700.));
    h.state_mut().switch_desktop(Desktop::Model);
    h.run_steps(3);
    h.render()
        .unwrap()
        .save(root.join("desktop-compact.png"))
        .unwrap();
    h.state_mut().layout = Layout::Quad;
    for pane in &mut h.state_mut().panes {
        pane.kind = PaneKind::Solid;
    }
    h.run_steps(3);
    h.render()
        .unwrap()
        .save(root.join("desktop-four-compact.png"))
        .unwrap();
}

#[test]
fn session_save_keeps_unsaved_design_and_workspace_before_restart() {
    #[derive(Default)]
    struct Storage(std::collections::HashMap<String, String>);
    impl eframe::Storage for Storage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }
    let mut h = harness([1600., 980.]);
    h.state_mut().design.name = "Unsaved ring before update".into();
    h.state_mut().switch_desktop(Desktop::Graph);
    let mut storage = Storage::default();
    h.state().persist_session(&mut storage).unwrap();
    let design: ringdesign_core::RingDesign =
        serde_json::from_str(&storage.0[crate::app::DESIGN_STORAGE_KEY]).unwrap();
    let workspace: crate::app::Workspace =
        serde_json::from_str(&storage.0[crate::app::WORKSPACE_STORAGE_KEY]).unwrap();
    assert_eq!(design.name, "Unsaved ring before update");
    assert_eq!(workspace.desktop, Desktop::Graph);
    assert!(workspace.desktops.contains_key(&Desktop::Model));
}

/// The claw solitaire on one Ring viewport, built at `params`.
fn claw_solitaire(params: ringdesign_core::BuildParams) -> Harness<'static, RingDesignerApp> {
    let mut h = harness([1600., 980.]);
    {
        let app = h.state_mut();
        app.switch_desktop(Desktop::Model);
        app.set_layout(Layout::Single);
        let pane = app.visible_panes()[0];
        app.panes[pane].kind = PaneKind::Solid;
        app.design = ringdesign_core::cad::examples::design("claw-solitaire").expect("the claw solitaire");
        app.preview_params = params;
        app.history.commit(&app.design);
    }
    h
}

/// Ticks until a fresh build lands: the landing tick's time, the median of the idle ticks that follow, and the time from dispatch to landed.
fn land(h: &mut Harness<'static, RingDesignerApp>) -> (std::time::Duration, std::time::Duration, std::time::Duration) {
    let ctx = h.ctx.clone();
    let at = |h: &Harness<'static, RingDesignerApp>| h.state().build.as_ref().map(|b| std::sync::Arc::as_ptr(b) as usize);
    let before = at(h);
    h.state_mut().rebuild_now();
    let start = std::time::Instant::now();
    let landing = loop {
        let t = std::time::Instant::now();
        h.state_mut().tick(&ctx);
        let dt = t.elapsed();
        if !h.state().is_building() && at(h) != before {
            break dt;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(90), "the ring never built: {}", h.state().status);
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let latency = start.elapsed();
    assert!(h.state().is_current(), "{}", h.state().status);
    let mut idle: Vec<std::time::Duration> = (0..15)
        .map(|_| {
            let t = std::time::Instant::now();
            h.state_mut().tick(&ctx);
            t.elapsed()
        })
        .collect();
    idle.sort();
    (landing, idle[idle.len() / 2], latency)
}

/// What one build costs the UI thread on the claw solitaire, printed with `--nocapture`: the landing tick over an idle
/// one and the first frame's edge sync. The worker stages the metal and the edges and builds the pick scene, so a build
/// lands with its edges current and the first frame restages none.
#[test]
fn a_build_lands_staged_and_the_ui_thread_only_uploads() {
    use ringdesign_workbench::render;
    let ws = crate::app::Workspace::default();
    for (name, params) in [("preview", ws.preview_params), ("export", ws.export_params)] {
        let mut h = claw_solitaire(params);
        land(&mut h);
        let mut rows = Vec::new();
        let mut latencies = Vec::new();
        for _ in 0..3 {
            let (landing, idle, latency) = land(&mut h);
            latencies.push(latency);
            let build = h.state().build.clone().expect("a build");
            let key = std::sync::Arc::as_ptr(&build) as usize;
            let e = build.parts.evaluated.as_ref().expect("the parts are evaluated");
            {
                let r = h.state().renderer.lock().unwrap();
                let (staged_for, runs) = r.staged_edges();
                assert_eq!(staged_for, Some(key), "the edges arrive with the build, before a frame is drawn");
                assert_eq!(runs, render::stage_edges(e).runs.as_slice(), "the worker's edges are the ones the pass stages");
            }
            let t = std::time::Instant::now();
            h.state().renderer.lock().unwrap().sync_edges(&build, &[], None);
            let sync = t.elapsed();
            assert_eq!(h.state().renderer.lock().unwrap().staged_edges().0, Some(key), "nothing restaged");
            let scene = h.state().pick_scene.clone().expect("the pick scene");
            assert_eq!(scene.faces(), build.mesh.faces.len(), "the pick scene is over the build on screen");
            rows.push((landing.saturating_sub(idle), idle, sync));
        }
        rows.sort_by_key(|r| r.0);
        let (landing, idle, sync) = rows[1];
        latencies.sort();
        let latency = latencies[1];
        assert_eq!(h.state().panes[3].kind, PaneKind::Section);
        assert!(h.state().panes[3].section.is_none(), "a section out of sight is not resliced by a build");
        h.state_mut().set_layout(Layout::Quad);
        assert!(h.state().panes[3].section.is_some(), "the layout that shows it reslices it");
        let app = h.state();
        let build = app.build.clone().unwrap();
        let e = build.parts.evaluated.as_ref().expect("the parts are evaluated");
        let t = std::time::Instant::now();
        let edges = render::stage_edges(e);
        let stage_edges = t.elapsed();
        let t = std::time::Instant::now();
        let metal = render::stage_mesh(&build.mesh, app.cast.as_ref(), (app.design.inner_radius_mm(), app.design.draft.min_section_mm));
        let stage_mesh = t.elapsed();
        let t = std::time::Instant::now();
        let scene = ringdesign_core::interaction::pick::PickScene::build(&build, &app.design);
        let pick = t.elapsed();
        // With a part chosen, the first frame after a build still restages its tint on the UI thread.
        let head = e.components.iter().find(|c| c.name == "Four-claw head").expect("the head").id;
        let mut chosen = ringdesign_workbench::viewport::Selection::default();
        chosen.click(Some(ringdesign_workbench::viewport::Sel::Part(head)), ringdesign_workbench::viewport::Mods::default());
        let t = std::time::Instant::now();
        let weights = ringdesign_workbench::viewport::tint(&chosen, &build);
        let tinted = crate::viewport::GpuMeshRenderer::stage_select(&build.mesh, &weights);
        let tint = t.elapsed();
        assert!(!tinted.is_empty());
        println!(
            "claw solitaire at {name} ({} tris): dispatch to landed {:.1} ms, landing {:.2} ms over an idle tick of {:.3} ms, first frame's edge sync {:.1} us, \
             a chosen part's tint {:.2} ms; on the worker: stage_mesh {:.2} ms ({} floats), pick scene {:.2} ms ({} faces), stage_edges {:.1} us ({} segments, {} bytes)",
            build.mesh.faces.len(),
            latency.as_secs_f64() * 1e3,
            landing.as_secs_f64() * 1e3,
            idle.as_secs_f64() * 1e3,
            sync.as_secs_f64() * 1e6,
            tint.as_secs_f64() * 1e3,
            stage_mesh.as_secs_f64() * 1e3,
            metal.len(),
            pick.as_secs_f64() * 1e3,
            scene.faces(),
            stage_edges.as_secs_f64() * 1e6,
            edges.segments(),
            edges.bytes(),
        );
    }
}

/// Steps until the CAD pane has evaluated and staged its view.
fn wait_for_cad_view(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    while h.state().cad.edge_runs().1.is_none() {
        h.run_steps(3);
        assert!(h.state().cad.last_error().is_none(), "{:?}", h.state().cad.last_error());
        assert!(start.elapsed() < std::time::Duration::from_secs(30), "the CAD pane never evaluated");
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    h.run_steps(2);
}

#[test]
fn the_cad_pane_draws_its_parts_edges_unless_they_are_isolated_or_exploded() {
    use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation, Placement};
    let mut h = harness([1600., 980.]);
    {
        let app = h.state_mut();
        let mut d = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let post = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.25), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, component: post }).unwrap();
        let hidden = Component { placement: Placement::ring(270.0, 0.5), visible: false, ..Component::default() };
        doc.append(Feature { id: 3, name: "Block".into(), enabled: true, operation: Operation::Box { size: [2.0, 2.0, 1.5] }, component: hidden }).unwrap();
        d.cad = Some(doc);
        app.design = d;
        app.history.commit(&app.design);
        app.switch_desktop(Desktop::Cad);
    }
    wait_for_cad_view(&mut h);
    let (held, every) = h.state().cad.edge_runs();
    let every = every.expect("the view's evaluation");
    let of = |id: u64| every.iter().filter(|r| r.key.0 == id).count();
    assert!(of(2) > 0 && of(3) > 0, "both parts have edges: {every:?}");
    assert_eq!(held, every, "the whole ring shows every part's edges, a hidden part's metal and all");
    // Drawn alone, the hidden block takes its edges with it and the rest are renumbered into one buffer.
    h.get_by_label("Parts only").click();
    h.run_steps(3);
    let post: Vec<_> = every.iter().filter(|r| r.key.0 == 2).map(|r| r.key).collect();
    let alone = h.state().cad.edge_runs().0;
    assert_eq!(alone.iter().map(|r| r.key).collect::<Vec<_>>(), post);
    assert_eq!(alone[0].first, 0);
    assert!(alone.windows(2).all(|w| w[1].first == w[0].first + w[0].count), "contiguous: {alone:?}");
    h.get_by_label("Parts only").click();
    h.run_steps(3);
    assert_eq!(h.state().cad.edge_runs().0, every);
    // Isolated from the Ring viewport's menu: none, then all of them back from the pane's Display menu.
    crate::panels::cad::ask(h.state_mut(), crate::panels::cad::CadRequest::Isolate { feature: 2 });
    h.run_steps(3);
    assert!(h.state().cad.edge_runs().0.is_empty(), "an isolated part is drawn without the pass");
    h.get_by_label("Display").click();
    h.run_steps(3);
    h.get_by_label("Show all components").click();
    h.run_steps(3);
    assert_eq!(h.state().cad.edge_runs().0, every);
    // Exploded parts leave their edges behind, so the pass draws none until they close up.
    h.state_mut().cad.explode_by(1.5);
    assert!(h.state().cad.edge_runs().0.is_empty());
    h.state_mut().cad.explode_by(0.0);
    assert_eq!(h.state().cad.edge_runs().0, every);
    // The Part edges switch is the Ring viewport's and the pane's alike: one field on the app.
    h.state_mut().show_part_edges = false;
    h.run_steps(2);
    assert!(h.state().cad.edge_runs().0.is_empty());
    h.state_mut().show_part_edges = true;
    h.run_steps(2);
    assert_eq!(h.state().cad.edge_runs().0, every);
    assert_eq!(h.state().cad.parts().iter().map(|(id, _)| *id).collect::<Vec<_>>(), [2, 3]);
}

/// A session store held in memory.
#[derive(Default)]
struct MemoryStorage(std::collections::HashMap<String, String>);
impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

/// The app launched on a saved session.
fn relaunch(storage: MemoryStorage) -> Harness<'static, RingDesignerApp> {
    let storage: &'static MemoryStorage = Box::leak(Box::new(storage));
    Harness::builder().with_size([1600., 980.]).build_eframe(move |cc| {
        cc.storage = Some(storage);
        crate::theme::install(&cc.egui_ctx);
        let mut app = RingDesignerApp::new(cc);
        app.updater.automatic = false;
        app.auto_rebuild = false;
        app
    })
}

#[test]
fn the_part_edges_and_work_plane_switches_come_back_with_the_workspace() {
    let mut h = harness([1600., 980.]);
    h.run_steps(3);
    assert!(h.state().show_part_edges && !h.state().command.planes.hidden, "both on to begin with");
    h.get_by_label("Display").click();
    h.run_steps(3);
    h.get_by_label("Part edges").click();
    h.run_steps(3);
    assert!(!h.state().show_part_edges, "the Display menu turned the edges off");
    // One store: nothing of the switch lives in egui's own data any more.
    assert_eq!(h.ctx.data_mut(|d| d.get_persisted::<bool>(egui::Id::new("ring-viewport-part-edges"))), None);
    h.state_mut().command.planes.hidden = true;
    h.state_mut().show_wireframe = true;
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    let key = crate::app::WORKSPACE_STORAGE_KEY;
    let saved: crate::app::Workspace = serde_json::from_str(&storage.0[key]).unwrap();
    assert_eq!((saved.show_part_edges, saved.show_work_planes, saved.show_wireframe), (false, false, true));

    // The same session as a build from before the two switches were kept would have saved it.
    let mut json: serde_json::Value = serde_json::from_str(&storage.0[key]).unwrap();
    for gone in ["show_part_edges", "show_work_planes"] {
        json.as_object_mut().unwrap().remove(gone);
    }
    let mut older = MemoryStorage::default();
    older.0.insert(key.into(), json.to_string());

    let restored = relaunch(storage);
    assert!(!restored.state().show_part_edges, "the edges stay off after a restart");
    assert!(restored.state().command.planes.hidden, "and the work planes hidden");
    assert!(restored.state().show_wireframe, "as the wireframe does");
    let upgraded = relaunch(older);
    assert!(upgraded.state().show_part_edges && !upgraded.state().command.planes.hidden, "an older workspace opens with both on");
    assert!(upgraded.state().show_wireframe);
}

/// The session's stored design, parsed.
fn stored_design(storage: &MemoryStorage) -> serde_json::Value {
    serde_json::from_str(&storage.0[crate::app::DESIGN_STORAGE_KEY]).unwrap()
}

#[test]
fn the_session_design_travels_the_files_ladder_and_a_newer_one_is_refused_by_name_and_kept() {
    let mut h = harness([1600., 980.]);
    h.state_mut().design.name = "Session ring".into();
    h.state_mut().design.pins.push(ringdesign_workbench::viewport::pins::Pin::at([0.0, 10.65, 0.5], 1));
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    // Written at the version the files take, a design without a stored mesh at the one 0.6.0 reads.
    let written = stored_design(&storage);
    assert_eq!(written["format_version"], u64::from(ringdesign_core::library::PLAIN_FORMAT_VERSION));
    assert_eq!(written["pins"][0]["name"], "Pin 1");
    let back = relaunch(storage);
    assert_eq!((back.state().design.name.as_str(), back.state().design.pins.len()), ("Session ring", 1));
    // A session an older build wrote carries no version; it climbs the ladder from 0.
    let mut older = written.clone();
    older.as_object_mut().unwrap().remove("format_version");
    let mut store = MemoryStorage::default();
    store.0.insert(crate::app::DESIGN_STORAGE_KEY.into(), older.to_string());
    assert_eq!(relaunch(store).state().design.name, "Session ring");
    // One from a newer build is refused by name on the status line, and a new design opens in its place.
    let mut newer = written.clone();
    newer["format_version"] = serde_json::json!(ringdesign_core::library::FORMAT_VERSION + 1);
    let text = newer.to_string();
    let mut store = MemoryStorage::default();
    store.0.insert(crate::app::DESIGN_STORAGE_KEY.into(), text.clone());
    let mut refused = relaunch(store);
    let status = refused.state().status.clone();
    assert!(status.starts_with("The last session's design \"Session ring\" did not reopen") && status.contains(&format!("format version {}", ringdesign_core::library::FORMAT_VERSION + 1)) && status.contains("newer RingDesigner"), "{status}");
    assert_eq!(refused.state().design.name, ringdesign_core::RingDesign::default().name);
    // The status line keeps saying so past the first build, and the session keeps the refused design until the new one is edited.
    refused.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut refused);
    assert_eq!(refused.state().status, status);
    let mut again = MemoryStorage::default();
    refused.state().persist_session(&mut again).unwrap();
    assert_eq!(again.0[crate::app::DESIGN_STORAGE_KEY], text, "kept byte for byte");
    refused.state_mut().design.name = "Fresh".into();
    refused.state_mut().mark_dirty();
    refused.state().persist_session(&mut again).unwrap();
    assert_eq!(stored_design(&again)["name"], "Fresh");
}

#[test]
fn pins_a_workspace_kept_by_file_move_into_the_design_when_it_opens_and_travel_with_it() {
    use ringdesign_workbench::viewport::pins::Pin;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pinned.ring.json");
    let d = ringdesign_core::RingDesign { name: "Pinned".into(), ..Default::default() };
    ringdesign_core::library::save_design(&path, &d).unwrap();
    let (kept, unsaved) = (vec![Pin::at([0.0, 10.65, 0.0], 1), Pin::at([10.65, 0.0, 0.5], 2)], vec![Pin::at([0.0, -10.65, 0.0], 1)]);
    // A workspace as the last build wrote it: pins by file, the unsaved design's under "".
    let h = harness([1600., 980.]);
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    let mut ws: serde_json::Value = serde_json::from_str(&storage.0[crate::app::WORKSPACE_STORAGE_KEY]).unwrap();
    assert!(ws.get("pins").is_none(), "a workspace with nothing left to carry writes no pins");
    let by_file = |entries: &[(&str, &Vec<Pin>)]| serde_json::Value::Object(entries.iter().map(|(k, v)| (k.to_string(), serde_json::to_value(v).unwrap())).collect());
    let file = path.to_string_lossy().into_owned();
    ws["pins"] = by_file(&[(file.as_str(), &kept), ("", &unsaved)]);
    storage.0.insert(crate::app::WORKSPACE_STORAGE_KEY.into(), ws.to_string());
    let mut h = relaunch(storage);
    // The unsaved design in the session takes its own at once, as the design it always was: nothing to undo.
    assert_eq!(h.state().design.pins, unsaved);
    assert!(!h.state().history.can_undo());
    // Opening the file carries its pins into it, once; the design is what the Measure tool and the snaps read, and saves with them.
    crate::export::open_design_path(h.state_mut(), &path);
    h.run_steps(2);
    assert_eq!((h.state().design.name.as_str(), h.state().pins()), ("Pinned", kept.as_slice()));
    assert!(h.state().workspace().pins.is_empty(), "nothing left in the workspace to carry");
    ringdesign_core::library::save_design(&path, &h.state().design).unwrap();
    assert_eq!(ringdesign_core::library::load_design(&path).unwrap().pins, kept);
    // A pin dropped now is an edit of the design, settled into the history.
    let pin = Pin::at([-10.65, 0.0, 0.0], ringdesign_workbench::viewport::pins::next_number(h.state().pins()));
    h.state_mut().pins_mut().push(pin);
    let pinned = h.state().design.clone();
    h.state_mut().history.commit(&pinned);
    assert_eq!(h.state().pins().last().map(|p| p.name.as_str()), Some("Pin 3"));
    h.state_mut().undo();
    assert_eq!(h.state().pins(), kept.as_slice(), "Undo takes the pin back");
    // A file that carries pins of its own keeps them over any the workspace held for it.
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    let mut ws: serde_json::Value = serde_json::from_str(&storage.0[crate::app::WORKSPACE_STORAGE_KEY]).unwrap();
    ws["pins"] = by_file(&[(file.as_str(), &unsaved)]);
    storage.0.insert(crate::app::WORKSPACE_STORAGE_KEY.into(), ws.to_string());
    let mut h = relaunch(storage);
    crate::export::open_design_path(h.state_mut(), &path);
    h.run_steps(2);
    assert_eq!(h.state().pins(), kept.as_slice());
    assert!(h.state().workspace().pins.is_empty());
}

#[test]
fn the_cad_workspace_docks_the_report_beside_its_pane() {
    let mut h = harness([1600., 980.]);
    h.state_mut().switch_desktop(Desktop::Cad);
    h.run_steps(3);
    let right = |h: &Harness<'static, RingDesignerApp>| h.state().dock.tree(crate::dock::Side::Right).tiles.iter().filter_map(|(_, t)| if let egui_tiles::Tile::Pane(p) = t { Some(*p) } else { None }).collect::<Vec<_>>();
    assert_eq!(right(&h), [crate::dock::ToolKind::Report]);
    // Restoring the workspace's default puts it back where it was closed from.
    h.state_mut().dock.close(crate::dock::ToolKind::Report);
    h.state_mut().restore_default_layout();
    h.run_steps(2);
    assert_eq!(right(&h), [crate::dock::ToolKind::Report]);
}

/// A CAD desktop's dock as a build before it docked the Report stored it: nothing on either side and no defaults mark.
fn old_empty_cad_dock() -> serde_json::Value {
    let empty = crate::dock::Dock { left: egui_tiles::Tree::empty("dock_left"), right: egui_tiles::Tree::empty("dock_right"), ..crate::dock::Dock::for_desktop(Desktop::Cad) };
    let mut json = serde_json::to_value(&empty).unwrap();
    json.as_object_mut().unwrap().remove("defaults");
    json
}

#[test]
fn a_stored_cad_layout_with_the_old_empty_docks_gains_the_report_once() {
    let report = |h: &Harness<'static, RingDesignerApp>| h.state().dock.is_open(ToolKind::Report);
    // A session left on the Model desktop, its CAD layout stored with the old empty docks.
    let mut h = harness([1600., 980.]);
    h.state_mut().switch_desktop(Desktop::Cad);
    h.state_mut().switch_desktop(Desktop::Model);
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    let key = crate::app::WORKSPACE_STORAGE_KEY;
    let mut ws: serde_json::Value = serde_json::from_str(&storage.0[key]).unwrap();
    ws["desktops"]["Cad"]["dock"] = old_empty_cad_dock();
    storage.0.insert(key.into(), ws.to_string());
    let mut h = relaunch(storage);
    h.state_mut().switch_desktop(Desktop::Cad);
    h.run_steps(2);
    assert!(report(&h), "the CAD desktop gains its Report");
    // Closed now, it stays closed: across desktops and across a restart.
    h.state_mut().dock.close(ToolKind::Report);
    h.state_mut().switch_desktop(Desktop::Model);
    h.state_mut().switch_desktop(Desktop::Cad);
    assert!(!report(&h), "once");
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    let mut h = relaunch(storage);
    assert_eq!(h.state().desktop, Desktop::Cad);
    assert!(!report(&h), "once, after a restart too");
    h.state_mut().switch_desktop(Desktop::Model);
    h.state_mut().switch_desktop(Desktop::Cad);
    assert!(!report(&h));
    // A session left on the CAD desktop itself, with the old empty docks, gains it at startup.
    let mut storage = MemoryStorage::default();
    h.state().persist_session(&mut storage).unwrap();
    storage.0.insert(crate::app::DOCK_STORAGE_KEY.into(), old_empty_cad_dock().to_string());
    let h = relaunch(storage);
    assert_eq!(h.state().desktop, Desktop::Cad);
    assert!(report(&h), "the stored dock of the desktop in hand catches up too");
}
