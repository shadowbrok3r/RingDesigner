use super::*;

#[test]
fn phone_camera_actions_share_one_row_and_hidden_cube_releases_its_hit_area() {
    for width in [280.0, 320.0, 411.0] {
        for cube in [false, true] {
            let ctx = egui::Context::default();
            ctx.all_styles_mut(|s| s.spacing.interact_size.y = 32.0);
            let bounds = Rect::from_min_max(egui::pos2(4.0, 60.0), egui::pos2(width - 4.0, 500.0));
            let mut settings = Settings::default();
            let mut response = None;
            for _ in 0..3 {
                let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 600.0))), ..Default::default() }, |ui| {
                    response = Some(show_mobile(ui, bounds, egui::Id::unique("mobile-test"), &mut settings, [0.6, 0.3, 0.0], 90.0, cube));
                });
                out.textures_delta.clear();
            }
            let response = response.unwrap();
            let controls: Vec<_> = response.controls.iter().filter(|(label, _)| ["Home view", "Mirror", "Views", "Lock view", "Magnifier", "Upside down"].contains(label)).collect();
            assert_eq!(controls.len(), 6);
            for (_, rect) in &controls {
                assert!((rect.top() - controls[0].1.top()).abs() < 0.5, "camera controls used another row");
                assert!(bounds.contains_rect(*rect), "control escaped a {width}pt phone: {rect:?}");
            }
            assert_eq!(response.controls.iter().any(|(label, _)| *label == "View cube"), cube);
            assert_eq!(response.obstacles.len(), if cube { 2 } else { 1 });
            if !cube { assert!(response.rect.height() <= 43.0); }
        }
    }
}
