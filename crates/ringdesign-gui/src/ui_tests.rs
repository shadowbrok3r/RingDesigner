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
    assert!(h.query_all_by_label_contains("Opening Ecdysis — ventral scales — ").any(|n| n.accesskit_node().role() == egui::accesskit::Role::ProgressIndicator), "the plate says how far it has got");
    assert!(h.state().status.starts_with("Opening Ecdysis — ventral scales — "), "and so does the status line: {}", h.state().status);
    crate::interaction_tests::wait_for_template(&mut h);
    assert!(h.state().design.name.starts_with("Ecdysis"));
    // Landed, it builds at once and says so until the build shows it.
    assert!(h.state().is_building() && h.state().opened_building.is_some());
    assert!(h.query_by_label("Opening Ecdysis — ventral scales — building the ring").is_some());
    crate::interaction_tests::wait_for_build(&mut h);
    h.run_steps(2);
    assert!(h.state().opened_building.is_none() && h.query_by_label_contains("Opening Ecdysis").is_none());
    assert!(h.state().design.graph.is_some());
    assert!(h.state().document_path.is_none());
    assert!(!h.state().graph_ed.as_ref().unwrap().graph().nodes.iter().any(|n| n.kind == "head"));
}

/// The catalogue entry `slug`.
fn template(slug: &str) -> &'static ringdesign_workbench::templates::Template {
    ringdesign_workbench::templates::collections().iter().flat_map(|c| &c.templates).find(|t| t.slug == slug).expect(slug)
}

/// How far the open on screen has got, before and after it lands.
fn open_fraction(app: &RingDesignerApp) -> Option<f32> {
    match (&app.opening, &app.opened_building) {
        (Some((opening, _)), _) => Some(opening.progress().fraction()),
        (None, Some((_, _, progress))) => Some(progress.fraction()),
        (None, None) => None,
    }
}

#[test]
fn a_template_opens_while_the_old_design_stays_live_and_its_first_build_takes_the_open_evaluation() {
    use std::sync::atomic::Ordering;
    let mut h = harness([1600., 980.]);
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    crate::export::load_catalog_template(h.state_mut(), template("nocturne"));
    let mut read = vec![open_fraction(h.state()).expect("a plate from the first frame")];
    h.run_steps(1);
    read.extend(open_fraction(h.state()));
    // The design on screen stays live: an edit made while the template opens lands and builds.
    assert!(h.state().opening.is_some());
    h.state_mut().design.name = "Edited while it opened".into();
    h.state_mut().mark_dirty();
    h.state_mut().rebuild_now();
    assert_eq!(h.state().design.name, "Edited while it opened");
    let probe = h.state().detail_probe();
    let evaluations = probe.evaluations.load(Ordering::Relaxed);
    let start = std::time::Instant::now();
    while h.state().opening.is_some() {
        h.run_steps(1);
        read.extend(open_fraction(h.state()));
        assert!(start.elapsed().as_secs() < 60, "the template never opened");
    }
    assert!(h.state().design.name.starts_with("Nocturne"));
    assert!(h.state().seed.is_none(), "the first build took the open's evaluation");
    while h.state().opened_building.is_some() {
        h.run_steps(1);
        read.extend(open_fraction(h.state()));
        assert!(start.elapsed().as_secs() < 60, "the template never built");
    }
    assert!(read.windows(2).all(|w| w[0] <= w[1]), "the bar never falls back: {read:?}");
    assert!(h.state().is_current(), "{}", h.state().status);
    assert_eq!(probe.evaluations.load(Ordering::Relaxed), evaluations, "the first build evaluated nothing again");
    // A rebuild evaluates once, against the library the open baked the template's artwork into.
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    assert!(h.state().is_current(), "{}", h.state().status);
    assert_eq!(probe.evaluations.load(Ordering::Relaxed), evaluations + 1);
}

#[test]
fn a_library_that_moved_while_a_template_opened_gets_the_template_artwork_baked_onto_it() {
    use ringdesign_core::alpha::Alpha;
    let mut h = harness([1600., 980.]);
    let probe = h.state().detail_probe();
    let evaluations = probe.evaluations.load(std::sync::atomic::Ordering::Relaxed);
    crate::export::load_catalog_template(h.state_mut(), template("nocturne"));
    // The old design redraws one of the template's own sources and adds one of its own while it opens.
    h.state_mut().library_mut().insert(Alpha::new("Palmette", 2, 2, vec![0.5; 4]));
    h.state_mut().library_mut().insert(Alpha::new("Mine", 2, 2, vec![0.25; 4]));
    crate::interaction_tests::wait_for_template(&mut h);
    let lib = h.state().lib.clone();
    assert_eq!(lib.get("Mine").map(|a| a.data.clone()), Some(vec![0.25; 4]));
    let palmette = lib.get("Palmette").expect("the template's own art");
    assert_eq!((palmette.width, palmette.height), (1024, 1024), "baked again onto the library as it stands");
    crate::interaction_tests::wait_for_build(&mut h);
    assert!(h.state().is_current(), "{}", h.state().status);
    assert!(probe.evaluations.load(std::sync::atomic::Ordering::Relaxed) > evaluations, "an evaluation against a library that moved is made again, not carried over");
    let mut fresh = ringdesign_core::AlphaLibrary::builtin();
    h.state().design.unpack_embedded(&mut fresh);
    h.state().design.bake_all(&mut fresh);
    assert_eq!(fresh.get("Palmette").map(|a| &a.data), Some(&palmette.data));
    assert!(lib.sdf_of("Palmette").is_some_and(|f| f.data == fresh.sdf_of("Palmette").unwrap().data), "its field follows the template's art");
}

/// A wake that holds an opening thread at its first stage until the returned release is called.
fn held() -> (impl Fn() + Send + Sync + 'static, impl Fn()) {
    let gate = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let wait = gate.clone();
    let hold = move || {
        let (open, turned) = &*wait;
        let mut open = open.lock().unwrap_or_else(|e| e.into_inner());
        let started = std::time::Instant::now();
        while !*open && started.elapsed().as_secs() < 60 {
            open = turned.wait_timeout(open, std::time::Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
        }
    };
    let release = move || {
        *gate.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
        gate.1.notify_all();
    };
    (hold, release)
}

/// Caiman opening on the app, held at its first stage: its progress, the flag its thread sets on returning, and the release.
fn caiman_held(h: &mut Harness<'static, RingDesignerApp>) -> (std::sync::Arc<ringdesign_workbench::templates::Progress>, std::sync::Arc<std::sync::atomic::AtomicBool>, impl Fn() + use<>) {
    let (hold, release) = held();
    let app = h.state_mut();
    let opening = template("caiman-imported").open(app.graph_reg.clone(), app.lib.clone(), hold);
    let (progress, finished) = (opening.progress(), opening.finished());
    app.start_opening(opening, crate::app::Lands::Template { graph_pane: false });
    (progress, finished, release)
}

fn wait_until_returned(h: &mut Harness<'static, RingDesignerApp>, finished: &std::sync::atomic::AtomicBool) {
    let start = std::time::Instant::now();
    while !finished.load(std::sync::atomic::Ordering::Relaxed) {
        h.run_steps(1);
        assert!(start.elapsed().as_secs() < 60, "the first open never stopped");
    }
}

#[test]
fn choosing_another_template_stops_the_first_and_only_the_second_lands() {
    let mut h = harness([1600., 980.]);
    let (progress, finished, release) = caiman_held(&mut h);
    h.run_steps(1);
    crate::export::load_catalog_template(h.state_mut(), template("court-band"));
    release();
    wait_until_returned(&mut h, &finished);
    assert_eq!(progress.stage(), ringdesign_workbench::templates::Stage::Reading, "the replaced open stopped before its first node");
    crate::interaction_tests::wait_for_template(&mut h);
    crate::interaction_tests::wait_for_build(&mut h);
    h.run_steps(3);
    assert_eq!(h.state().design.name, "Court band");
    assert!(h.state().opening.is_none() && h.state().opened_building.is_none());
}

#[test]
fn a_file_opened_from_recent_stops_a_template_still_opening() {
    let dir = std::env::temp_dir().join(format!("recent-stops-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mine.ring.json");
    let mut mine = ringdesign_core::RingDesign::default();
    mine.name = "Opened from Recent".into();
    ringdesign_core::library::save_design(&path, &mine).unwrap();
    let mut h = harness([1600., 980.]);
    let (progress, finished, release) = caiman_held(&mut h);
    crate::export::open_design_path(h.state_mut(), &path);
    assert!(h.state().opening.is_none(), "the file stopped the template");
    release();
    wait_until_returned(&mut h, &finished);
    h.run_steps(3);
    assert_eq!(progress.stage(), ringdesign_workbench::templates::Stage::Reading, "no node ran after the file opened");
    assert_eq!(h.state().design.name, "Opened from Recent");
    assert_eq!(h.state().document_path.as_deref(), Some(path.as_path()));
    assert!(h.state().opened_building.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Holds a worker before its detail measures for as long as it lives.
struct DetailHeld(std::sync::Arc<crate::app::DetailProbe>);

impl DetailHeld {
    fn new(probe: std::sync::Arc<crate::app::DetailProbe>) -> Self {
        *probe.held.lock().unwrap_or_else(|e| e.into_inner()) = true;
        DetailHeld(probe)
    }
}

impl Drop for DetailHeld {
    fn drop(&mut self) {
        *self.0.held.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.0.released.notify_all();
    }
}

/// Steps until the detail findings of the build on screen have landed.
fn wait_for_detail(h: &mut Harness<'static, RingDesignerApp>) {
    let start = std::time::Instant::now();
    while h.state().dfm_generation < h.state().build_generation() {
        h.run_steps(1);
        assert!(start.elapsed().as_secs() < 60, "the detail never followed");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn detail_messages(h: &Harness<'static, RingDesignerApp>) -> (Vec<String>, Vec<String>) {
    let want = ringdesign_core::dfm::findings_in(&h.state().design, &h.state().lib).into_iter().map(|f| f.message).collect();
    let got = h.state().detail_findings().iter().map(|f| f.message.clone()).collect();
    (got, want)
}

#[test]
fn a_build_lands_before_its_detail_findings_and_they_follow_it() {
    let mut h = harness([1600., 980.]);
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    h.state().detail_findings();
    wait_for_detail(&mut h);
    let braided = ringdesign_core::templates::all().iter().find(|t| t.name == "Braided band").unwrap().design();
    let held = DetailHeld::new(h.state().detail_probe());
    h.state_mut().design = braided;
    h.state_mut().mark_dirty();
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    let generation = h.state().build_generation();
    assert!(h.state().is_current(), "{}", h.state().status);
    assert!(h.state().dfm_generation < generation, "the build landed while its detail was held");
    drop(held);
    wait_for_detail(&mut h);
    let (got, want) = detail_messages(&h);
    assert!(!want.is_empty(), "the braid measures under the floor");
    assert_eq!(got, want);
}

#[test]
fn the_worker_measures_no_detail_until_a_panel_reads_it_and_then_the_build_on_screen() {
    let mut h = harness([1600., 980.]);
    let probe = h.state().detail_probe();
    h.state_mut().dock.close(ToolKind::Report);
    h.state_mut().dock.close(ToolKind::Layers);
    h.state_mut().design = ringdesign_core::templates::all().iter().find(|t| t.name == "Braided band").unwrap().design();
    h.state_mut().mark_dirty();
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    std::thread::sleep(std::time::Duration::from_millis(200));
    h.run_steps(3);
    assert_eq!(probe.started.load(std::sync::atomic::Ordering::SeqCst), 0, "measured with no panel reading it");
    assert_eq!(h.state().dfm_generation, 0);
    let generation = h.state().build_generation();
    // The Report reads the worker's findings as it draws.
    h.state_mut().dock.open_on(ToolKind::Report, crate::dock::Side::Right);
    h.run_steps(1);
    wait_for_detail(&mut h);
    assert_eq!(h.state().build_generation(), generation, "the build on screen measured without another");
    assert_eq!(probe.started.load(std::sync::atomic::Ordering::SeqCst), 1);
    let (got, want) = detail_messages(&h);
    assert!(!want.is_empty(), "the braid measures under the floor");
    assert_eq!(got, want);
}

#[test]
fn a_dropped_app_gives_up_the_detail_measure_it_was_making() {
    let mut h = harness([1600., 980.]);
    let probe = h.state().detail_probe();
    let n = 512;
    let ink = 0.7 + (std::process::id() % 1000) as f32 * 1e-4;
    let mask = ringdesign_core::alpha::Alpha::new("dropped-mask", n, n, (0..n * n).map(|i| if (i % n) % 9 < 2 || (i / n) % 17 < 3 { ink } else { 0.0 }).collect());
    std::sync::Arc::make_mut(&mut h.state_mut().lib).insert(mask);
    let mut design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    let tiling = ringdesign_core::tiling::TilingLayer::default_for("dropped-mask", &design.field_context());
    design.layers.layers.push(ringdesign_core::field::LayerEntry::new("Dropped", ringdesign_core::field::Layer::Tiling(tiling)));
    let lib = h.state().lib.clone();
    // Dispatched with no frame run, so no panel measures the mask before the worker does.
    h.state().detail_findings();
    h.state_mut().design = design.clone();
    h.state_mut().rebuild_now();
    let start = std::time::Instant::now();
    while probe.started.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(start.elapsed().as_secs() < 60, "the measure never started");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
    let dropped = std::time::Instant::now();
    drop(h);
    let ended = loop {
        if let Some(at) = *probe.ended.lock().unwrap() {
            break at;
        }
        assert!(dropped.elapsed().as_secs() < 120, "the measure never ended");
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let given_up = ended.saturating_duration_since(dropped);
    // The same measure made whole, which it could not skip had the dropped one kept anything.
    let t = std::time::Instant::now();
    ringdesign_core::dfm::findings_in(&design, &lib);
    let whole = t.elapsed();
    assert!(given_up * 3 < whole, "given up {given_up:?} after the drop against {whole:?} for the whole measure");
}

#[test]
fn an_arrange_writes_only_the_positions_into_the_graph_json() {
    let mut h = harness([1600., 980.]);
    h.state_mut().design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    h.state_mut().convert_to_graph();
    let mut piled = h.state().graph_ed.as_ref().expect("driven").graph().clone();
    for node in &mut piled.nodes {
        node.pos = [0.0, 0.0];
    }
    h.state_mut().design.graph = serde_json::to_value(&piled).ok();
    h.state_mut().sync_graph();
    let before = h.state().design.graph.clone();
    h.state_mut().arrange_graph();
    let app = h.state();
    assert_ne!(app.design.graph, before, "the arrange moved the piled nodes");
    assert_eq!(app.design.graph, serde_json::to_value(app.graph_ed.as_ref().unwrap().graph()).ok(), "written in place, the same JSON as the graph written whole");
    assert_eq!(app.graph_json, app.design.graph, "the editor stays in step");
    assert!(ringdesign_core::history::graph_layout_only(before.as_ref(), app.design.graph.as_ref()), "layout, not an edit");
}

#[test]
fn a_driven_design_file_lands_with_its_graph_read_on_the_opening_thread() {
    let dir = std::env::temp_dir().join(format!("driven-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("driven.ring.json");
    let mut h = harness([1600., 980.]);
    h.state_mut().design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    h.state_mut().convert_to_graph();
    ringdesign_core::library::save_design(&path, &h.state().design).unwrap();
    h.state_mut().design = ringdesign_core::RingDesign::default();
    h.state_mut().sync_graph();
    assert!(h.state().graph_ed.is_none());
    let reads = RingDesignerApp::graph_reads();
    h.state_mut().open_file(path.clone());
    crate::interaction_tests::wait_for_template(&mut h);
    crate::interaction_tests::wait_for_build(&mut h);
    let app = h.state();
    assert!(app.graph_ed.is_some() && app.design.graph.is_some() && app.graph_json == app.design.graph, "it lands driven, its editor in step");
    assert_eq!(RingDesignerApp::graph_reads(), reads, "the graph was read on the opening thread, not the UI's");
    assert!(app.is_current(), "{}", app.status);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_driven_graph_reads_its_own_lettering_by_name_once_the_host_holds_it() {
    use ringdesign_graph::value::Literal;
    let mut g = ringdesign_graph::graph::Graph::default();
    let d = g.add("design.new").unwrap();
    let motto = g.add("alpha.text").unwrap();
    g.set_input(motto, "name", Literal::Text("Motto".into())).unwrap();
    g.set_input(motto, "text", Literal::Text("ever".into())).unwrap();
    let named = g.add("alpha.library").unwrap();
    g.set_input(named, "name", Literal::Text("Motto".into())).unwrap();
    let tiling = g.add("layer.tiling.fit").unwrap();
    g.connect(d, "design", tiling, "design").unwrap();
    g.connect(named, "alpha", tiling, "alpha").unwrap();
    let entry = g.add("entry").unwrap();
    g.connect(tiling, "layer", entry, "layer").unwrap();
    let stack = g.add("stack").unwrap();
    g.connect(entry, "entry", stack, "entries").unwrap();
    let asm = g.add("design.assemble").unwrap();
    g.connect(d, "design", asm, "design").unwrap();
    g.connect(stack, "stack", asm, "stack").unwrap();
    g.connect(motto, "source", asm, "alphas").unwrap();
    let out = g.add(ringdesign_graph::eval::OUTPUT_KIND).unwrap();
    g.connect(asm, "design", out, ringdesign_graph::eval::OUTPUT_DESIGN_PIN).unwrap();
    let mut h = harness([1600., 980.]);
    h.state_mut().design.graph = serde_json::to_value(&g).ok();
    h.state_mut().sync_graph();
    let alpha = |app: &RingDesignerApp| {
        app.design.layers.layers.iter().find_map(|e| match &e.layer {
            ringdesign_core::Layer::Tiling(t) => Some(t.alpha.clone()),
            _ => None,
        })
    };
    let mut read = Vec::new();
    for _ in 0..3 {
        h.state_mut().rebuild_now();
        crate::interaction_tests::wait_for_build(&mut h);
        read.push(alpha(h.state()));
    }
    assert!(h.state().lib.get("Motto").is_some(), "the lettering is baked into the host's library");
    let motto = Some("Motto".to_string());
    assert_eq!(read[1..], [motto.clone(), motto], "from the second build the tiling reads the lettering by name: {read:?}");
}

#[test]
fn one_undo_takes_back_a_graph_edit_and_what_it_evaluated_to() {
    use ringdesign_graph::value::Literal;
    let mut h = harness([1600., 980.]);
    h.state_mut().design = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
    h.state_mut().convert_to_graph();
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    let settled = h.state().design.clone();
    h.state_mut().history.commit(&settled);
    let (graph, width) = (h.state().design.graph.clone(), h.state().design.profile.width_mm);
    let mut g = h.state().graph_ed.as_ref().expect("driven").graph().clone();
    let profile = g.nodes.iter().find(|n| n.kind == "band.profile").expect("a profile node").id;
    g.set_input(profile, "width_mm", Literal::Number(width + 2.0)).unwrap();
    h.state_mut().design.graph = serde_json::to_value(&g).ok();
    h.state_mut().mark_dirty();
    let edited = h.state().design.clone();
    assert_eq!(h.state_mut().history.commit(&edited).as_deref(), Some("Graph edited"));
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    assert_eq!(h.state().design.profile.width_mm, width + 2.0, "the edit evaluated");
    h.state_mut().undo();
    assert_eq!((h.state().design.graph.clone(), h.state().design.profile.width_mm), (graph, width), "one undo took the edit back");
    h.state_mut().rebuild_now();
    crate::interaction_tests::wait_for_build(&mut h);
    h.state_mut().redo();
    assert_eq!(h.state().design.profile.width_mm, width + 2.0, "and one redo brings it back");
}

#[test]
fn an_open_wakes_its_host_only_from_its_own_threads_never_the_bakes_pool() {
    use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
    let reg = Arc::new(ringdesign_script::registry());
    let lib = Arc::new(ringdesign_core::AlphaLibrary::builtin());
    let (strays, wakes) = (Arc::new(Mutex::new(Vec::<String>::new())), Arc::new(AtomicUsize::new(0)));
    let (seen, counted) = (strays.clone(), wakes.clone());
    let opening = template("caiman-imported").open(reg, lib, move || {
        counted.fetch_add(1, Ordering::Relaxed);
        let name = std::thread::current().name().unwrap_or("").to_owned();
        if name != "template-wake" && name != "template-open" {
            seen.lock().unwrap().push(name);
        }
    });
    let started = std::time::Instant::now();
    loop {
        if let Some(opened) = opening.poll() {
            opened.expect("opens");
            break;
        }
        assert!(started.elapsed().as_secs() < 60);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(wakes.load(Ordering::Relaxed) > 0);
    let strays = strays.lock().unwrap();
    assert!(strays.is_empty(), "woken {} times from threads that are neither its own: {:?}", strays.len(), strays.iter().take(3).collect::<Vec<_>>());
}

#[test]
fn a_template_with_an_expression_pin_opens_from_the_menu_path() {
    let mut g = ringdesign_graph::templates::graph("Court band").unwrap();
    g.set_input(ringdesign_graph::graph::NodeId(1), "width_mm", ringdesign_graph::value::Literal::expr("2.5 + 2.0")).unwrap();
    let t: &'static _ = Box::leak(Box::new(ringdesign_workbench::templates::Template::document("Court band by expression", "court-band-expression", Box::leak(Box::new(g)))));
    let mut h = harness([1600., 980.]);
    h.state_mut().open_template(t, true);
    crate::interaction_tests::wait_for_template(&mut h);
    assert_eq!(h.state().design.profile.width_mm, 4.5, "{}", h.state().status);
    crate::interaction_tests::wait_for_build(&mut h);
    assert!(h.state().is_current(), "{}", h.state().status);
    assert!(h.state().graph_errors.is_empty(), "{:?}", h.state().graph_errors);
    assert_eq!(h.state().design.profile.width_mm, 4.5);
}

#[test]
fn a_design_file_saves_and_opens_off_the_ui_thread() {
    let dir = std::env::temp_dir().join(format!("open-save-{}", std::process::id()));
    let path = dir.join("saved.ring.json");
    let mut h = harness([1600., 980.]);
    h.state_mut().design.name = "Written off the UI thread".into();
    crate::export::save_design_to(h.state_mut(), path.clone());
    assert!(h.state().saving.is_some() && h.state().document_path.is_none(), "the document takes the path once the write lands");
    let start = std::time::Instant::now();
    while h.state().saving.is_some() {
        h.run_steps(1);
        assert!(start.elapsed().as_secs() < 60, "the save never landed");
    }
    assert_eq!(h.state().document_path.as_deref(), Some(path.as_path()), "{}", h.state().status);
    assert_eq!(ringdesign_core::library::load_design(&path).unwrap().name, "Written off the UI thread");
    h.state_mut().design.name = "Edited since".into();
    h.state_mut().document_path = None;
    h.state_mut().open_file(path.clone());
    h.run_steps(1);
    assert_eq!(h.state().design.name, "Edited since", "the design on screen stays until the file lands");
    crate::interaction_tests::wait_for_template(&mut h);
    assert_eq!(h.state().design.name, "Written off the UI thread");
    assert_eq!(h.state().document_path.as_deref(), Some(path.as_path()));
    assert!(!h.state().history.can_undo(), "a file opened is a new timeline");
    // One that does not read says so and leaves the design alone.
    std::fs::write(dir.join("broken.ring.json"), "{ not a design").unwrap();
    h.state_mut().open_file(dir.join("broken.ring.json"));
    crate::interaction_tests::wait_for_template(&mut h);
    assert!(h.state().status.starts_with("Open failed"), "{}", h.state().status);
    assert_eq!(h.state().design.name, "Written off the UI thread");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_template_plate_stands_clear_of_every_views_controls() {
    use egui::containers::panel::PanelState;
    for layout in [Layout::Single, Layout::Quad] {
        let mut h = claw_solitaire(crate::app::Workspace::default().preview_params);
        h.state_mut().set_layout(layout);
        for pane in &mut h.state_mut().panes {
            pane.kind = PaneKind::Solid;
        }
        h.run_steps(3);
        assert!(crate::panels::timeline::shown(h.state()), "the claw solitaire shows its CAD timeline");
        crate::export::load_catalog_template(h.state_mut(), template("caiman-imported"));
        h.run_steps(2);
        let bar = h.query_all_by_label_contains("Opening Caiman — armoured hide").find(|n| n.accesskit_node().role() == egui::accesskit::Role::ProgressIndicator).expect("the plate").rect();
        let cancel = h.get_all_by_label("Cancel").map(|n| n.rect()).find(|r| r.center().y > bar.top() && r.center().y < bar.bottom()).expect("the plate's Cancel");
        let plate = bar.union(cancel);
        let canvases = crate::export::pane_canvases(h.state(), &h.ctx);
        assert_eq!(canvases.len(), layout.count(), "{layout:?}");
        for (i, canvas) in &canvases {
            for strip in ["pane_head", "viewport-footer", "viewport-timeline"] {
                let rect = PanelState::load(&h.ctx, egui::Id::new((strip, *i))).expect(strip).outer_rect;
                assert!(!rect.intersects(plate), "{layout:?}: the plate {plate:?} covers pane {i}'s {strip} {rect:?}");
            }
            if canvas.intersects(plate) {
                assert!(canvas.contains_rect(plate), "{layout:?}: the plate {plate:?} straddles pane {i}'s canvas {canvas:?}");
                assert!(plate.right() <= canvas.right() - 110.0, "{layout:?}: clear of the navigator");
                assert!(plate.left() >= canvas.left() + crate::command::RAIL_W, "{layout:?}: clear of the tool rail");
            }
        }
        drop(h);
    }
}

/// Milliseconds `f` holds the calling thread.
fn ms<T>(f: impl FnOnce() -> T) -> (f64, T) {
    let t = std::time::Instant::now();
    let out = f();
    (t.elapsed().as_secs_f64() * 1e3, out)
}

/// The UI thread's cost of the heavy operations on the heaviest designs: `cargo test --release -p ringdesign-gui ui_thread_costs -- --ignored --nocapture --test-threads=1`.
#[test]
#[ignore = "a timing table for release builds"]
fn ui_thread_costs_on_heavy_designs() {
    let dir = std::env::temp_dir().join(format!("ui-costs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut rows: Vec<(String, f64)> = Vec::new();
    // The template menu, opened cold: every collection's thumbnail, then every entry's in one collection.
    let mut h = harness([1600., 980.]);
    h.run_steps(3);
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label_contains("New from template").click();
    let (open_menu, _) = ms(|| h.run_steps(1));
    rows.push(("template menu, first open".into(), open_menu));
    h.get_by_label_contains("Stock masterworks").click();
    let (open_group, _) = ms(|| h.run_steps(1));
    rows.push(("template menu, first open of a collection".into(), open_group));
    let (again, _) = ms(|| h.run_steps(1));
    rows.push(("template menu, a frame after".into(), again));
    drop(h);
    for slug in ["caiman-imported", "nocturne"] {
        let mut h = harness([1600., 980.]);
        crate::export::load_catalog_template(h.state_mut(), template(slug));
        let mut slowest: f64 = 0.0;
        while h.state().opening.is_some() || h.state().opened_building.is_some() {
            let (t, _) = ms(|| h.run_steps(1));
            slowest = slowest.max(t);
        }
        rows.push((format!("{slug}: slowest frame while it opened, landed and built"), slowest));
        crate::interaction_tests::wait_for_build(&mut h);
        let idle: Vec<f64> = (0..10).map(|_| ms(|| h.run_steps(1)).0).collect();
        rows.push((format!("{slug}: idle frame (median)"), { let mut i = idle.clone(); i.sort_by(f64::total_cmp); i[5] }));
        let path = dir.join(format!("{slug}.ring.json"));
        let (save, saved) = ms(|| ringdesign_core::library::save_design_embedded(&path, &h.state().design, &h.state().lib));
        saved.unwrap();
        rows.push((format!("{slug}: the writing a save does ({:.1} MB)", std::fs::metadata(&path).unwrap().len() as f64 / 1e6), save));
        let (save, _) = ms(|| crate::export::save_design_to(h.state_mut(), path.clone()));
        rows.push((format!("{slug}: save and save-as, on the UI thread"), save));
        let start = std::time::Instant::now();
        while h.state().saving.is_some() {
            h.run_steps(1);
            assert!(start.elapsed().as_secs() < 60, "the save never landed");
        }
        assert!(h.state().status.starts_with("Saved "), "{}", h.state().status);
        let (snapshot, copy) = ms(|| (h.state().design.clone(), h.state().lib.clone()));
        drop(copy);
        rows.push((format!("{slug}: export snapshot"), snapshot));
        let (session, _) = ms(|| h.state().session_design().unwrap());
        rows.push((format!("{slug}: session save"), session));
        let (open, _) = ms(|| crate::export::open_design_path(h.state_mut(), &path));
        rows.push((format!("{slug}: open design file, Recent and --open"), open));
        let (tick, _) = ms(|| h.run_steps(1));
        rows.push((format!("{slug}: first frame after opening the file"), tick));
        let (open, _) = ms(|| h.state_mut().open_file(path.clone()));
        rows.push((format!("{slug}: open design file, the dialog, on the UI thread"), open));
        let mut slowest: f64 = 0.0;
        while h.state().opening.is_some() || h.state().opened_building.is_some() {
            slowest = slowest.max(ms(|| h.run_steps(1)).0);
        }
        rows.push((format!("{slug}: slowest frame while the file opened, landed and built"), slowest));
        h.state_mut().rebuild_now();
        crate::interaction_tests::wait_for_build(&mut h);
        // An edit, committed as the history commits a settled one.
        h.state_mut().design.name.push_str(" edited");
        h.state_mut().mark_dirty();
        let (commit, _) = ms(|| {
            let app = h.state_mut();
            app.history.commit(&app.design)
        });
        rows.push((format!("{slug}: history snapshot of a settled edit"), commit));
        let (undo, _) = ms(|| h.state_mut().undo());
        rows.push((format!("{slug}: undo"), undo));
        let (redo, _) = ms(|| h.state_mut().redo());
        rows.push((format!("{slug}: redo"), redo));
        if h.state().design.graph.is_some() {
            h.state_mut().graph_json = None;
            let (sync, _) = ms(|| h.state_mut().sync_graph());
            rows.push((format!("{slug}: graph sync"), sync));
            let (arrange, _) = ms(|| h.state_mut().arrange_graph());
            rows.push((format!("{slug}: arrange"), arrange));
            let (same, _) = ms(|| h.state_mut().sync_graph());
            rows.push((format!("{slug}: graph sync, nothing moved"), same));
            // What a sync and a landing spend it on.
            let app = h.state();
            let (t, json) = ms(|| app.design.graph.clone());
            rows.push((format!("{slug}:   graph JSON cloned ({:.1} MB)", serde_json::to_string(&json).map_or(0, |s| s.len()) as f64 / 1e6), t));
            let (t, _) = ms(|| app.design.graph == json);
            rows.push((format!("{slug}:   graph JSON compared, equal"), t));
            let (t, parsed) = ms(|| <ringdesign_graph::graph::Graph as serde::Deserialize>::deserialize(json.as_ref().unwrap()).unwrap());
            rows.push((format!("{slug}:   graph read from its JSON"), t));
            let (t, copy) = ms(|| parsed.clone());
            rows.push((format!("{slug}:   graph cloned"), t));
            let (t, mut ed) = ms(|| ringdesign_graph_ui::Editor::new(copy, &app.graph_reg));
            rows.push((format!("{slug}:   editor built"), t));
            let (t, _) = ms(|| ed.arrange(&app.graph_reg));
            rows.push((format!("{slug}:   editor arranged"), t));
            let (t, _) = ms(|| serde_json::to_value(ed.graph()));
            rows.push((format!("{slug}:   graph written whole to JSON"), t));
            let mut history = ringdesign_core::history::History::new(&ringdesign_core::RingDesign::default());
            let (t, _) = ms(|| history.reset(&app.design));
            rows.push((format!("{slug}:   history reset"), t));
        }
        let (dispatch, _) = ms(|| h.state_mut().rebuild_now());
        rows.push((format!("{slug}: dispatch a build"), dispatch));
        crate::interaction_tests::wait_for_build(&mut h);
    }
    let _ = std::fs::remove_dir_all(&dir);
    println!("| UI thread | ms |\n| --- | --- |");
    for (what, t) in rows {
        println!("| {what} | {t:.1} |");
    }
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
