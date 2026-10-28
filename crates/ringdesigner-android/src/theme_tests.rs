use super::*;
use egui_theme_kit::{store::{ThemeStore, StartupTheme}, presets};

#[test]
fn legacy_colors_migrate_and_mobile_preferences_survive_a_restart() {
    let legacy = egui_mobile_theme::ThemeConfig::default();
    let appearance = Appearance::restore(None, &legacy);
    let ctx = egui::Context::default();
    appearance.apply(&ctx);
    assert_eq!(egui_theme_kit::colors::success_ctx(&ctx), legacy.colors.success);
    assert!(ctx.global_style().spacing.interact_size.y >= MENU_ROW_H);
    let before = ctx.global_style().visuals.panel_fill;
    let json = appearance.save().expect("legacy appearance saves");
    let restored = Appearance::restore(Some(&json), &legacy);
    let other = egui::Context::default();
    restored.apply(&other);
    assert_eq!(other.global_style().visuals.panel_fill, before);
    assert!(!other.global_style().interaction.drag_value_dragging);
}

#[test]
fn shared_presets_custom_colors_and_startup_choices_round_trip() {
    for preset in presets::builtin_presets() {
        let ctx = egui::Context::default();
        let appearance = Appearance::default();
        presets::apply_preset_theme(&ctx, preset.id);
        presets::set_text_scale(&ctx, egui_theme_kit::TextScale::Large);
        touch_metrics(&ctx);
        let saved = egui_theme_kit::SavedTheme::current(&ctx);
        appearance.store.save_active(&saved);
        appearance.store.save_user_theme("My workshop", &saved);
        appearance.store.set_startup(StartupTheme::User("My workshop".into()));
        let json = appearance.save().expect("appearance serializes");
        let restored = Appearance::restore(Some(&json), &Default::default());
        let after = egui::Context::default();
        restored.apply(&after);
        assert_eq!(restored.store.user_themes().len(), 1);
        assert_eq!(restored.store.startup(), StartupTheme::User("My workshop".into()));
        assert_eq!(ctx.global_style().visuals.panel_fill, after.global_style().visuals.panel_fill, "{}", preset.id);
        assert_eq!(egui_theme_kit::fonts::text_scale(&after), egui_theme_kit::TextScale::Large);
        assert_eq!(presets::installed_preset_id(&after), Some(preset.id), "preset identity lost on restart");
        assert_eq!(egui_theme_kit::saved::SemanticColors::current(&ctx), egui_theme_kit::saved::SemanticColors::current(&after), "preset colors changed on restart");
        assert!(after.global_style().spacing.interact_size.y >= MENU_ROW_H);
        assert!(!after.global_style().spacing.scroll.floating);
    }
}

#[test]
fn appearance_picker_fits_narrow_phones_with_large_text() {
    fn label<'a>(shapes: &'a [egui::epaint::ClippedShape], label: &str) -> &'a egui::epaint::TextShape {
        fn find<'a>(shape: &'a egui::epaint::Shape, label: &str) -> Option<&'a egui::epaint::TextShape> {
            match shape {
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, label)),
                egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t),
                _ => None,
            }
        }
        shapes.iter().find_map(|s| find(&s.shape, label)).unwrap_or_else(|| panic!("{label} is not visible"))
    }
    for width in [280.0, 320.0, 411.0] {
        let ctx = egui::Context::default();
        let mut appearance = Appearance::default();
        appearance.apply(&ctx);
        presets::set_text_scale(&ctx, egui_theme_kit::TextScale::XLarge);
        touch_metrics(&ctx);
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        let mut time = 0.0;
        let mut draw = |events, height| {
            time += 0.05;
            let mut extent = 0.0;
            let mut out = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height))),
                events, time: Some(time), ..Default::default()
            }, |ui| {
                appearance.show(ui);
                extent = ui.min_rect().width();
            });
            out.textures_delta.clear();
            (out.shapes, extent)
        };
        let mut shown = draw(vec![], 650.0);
        for _ in 0..3 { shown = draw(vec![], 650.0); }
        assert!(shown.1 <= width + 1.0, "gallery grew beyond {width}pt: {}", shown.1);
        // Verified device path: collapse the long gallery, then expand Customize.
        for title in ["Presets", "Customize"] {
            let text = label(&shown.0, title);
            let pos = text.pos + text.galley.size() * 0.5;
            draw(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }], 650.0);
            shown = draw(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }], 650.0);
            for _ in 0..3 { shown = draw(vec![], 650.0); }
        }
        assert!(shown.1 <= width + 1.0, "customization grew beyond {width}pt: {}", shown.1);
        // Expose the scroll area's full content without changing its phone width.
        // The device previously fit horizontally by wrapping names one letter per line.
        for _ in 0..4 { shown = draw(vec![], 3000.0); }
        assert!(shown.1 <= width + 1.0, "color tables grew beyond {width}pt: {}", shown.1);
        for name in ["Corner radius", "Presence", "Primary", "Secondary", "Success", "Field border", "Card border"] {
            assert_eq!(label(&shown.0, name).galley.rows.len(), 1, "{name} wrapped at {width}pt");
        }
    }
}

#[test]
fn casting_workshop_keeps_all_three_steps_inside_phone_widths() {
    for width in [280.0, 320.0, 411.0] {
        for scale in [egui_theme_kit::TextScale::Normal, egui_theme_kit::TextScale::XLarge] {
            let ctx = egui::Context::default();
            Appearance::default().apply(&ctx);
            presets::set_text_scale(&ctx, scale);
            touch_metrics(&ctx);
            let mut workshop = ringdesign_workbench::Workshop::default();
            workshop.open_casting();
            let mut design = ringdesign_core::RingDesign::default();
            let library = ringdesign_core::AlphaLibrary::default();
            let mut extent = 0.0;
            for _ in 0..4 {
                let mut out = ctx.run_ui(egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 800.0))),
                    ..Default::default()
                }, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    workshop.show(ui, &mut design, &library);
                    extent = ui.min_rect().width();
                });
                out.textures_delta.clear();
            }
            assert!(extent <= width + 1.0, "casting grew beyond {width}pt at {scale:?}: {extent}");
        }
    }
}
