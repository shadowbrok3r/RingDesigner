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
    h.get_by_label("Ecdysis — ventral scales").click();
    h.run_steps(3);
    assert!(h.state().design.name.starts_with("Ecdysis"));
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
