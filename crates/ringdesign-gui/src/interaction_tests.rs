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
