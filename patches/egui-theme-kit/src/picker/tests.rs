//! Headless frame runs and state transitions of the picker, asserted through a memory store.

use std::time::Duration;

use eframe::egui::{CentralPanel, Color32, Context, Pos2, RawInput, Rect, vec2};

use super::{
    MessageKind, Origin, PickerOptions, PickerState,
    edit::{Role, Semantic, Surface},
    show, window,
};
use crate::{
    colors,
    fonts::{self, TextScale},
    glass::{self, GlassParams},
    metrics::{self, Density},
    presets::{self, PresetStyles, apply_preset},
    saved::SavedTheme,
    store::{MemoryStore, StartupTheme, ThemeStore},
    tokens::Tokens,
};

fn themed_context() -> Context {
    let ctx = Context::default();
    ctx.set_fonts(fonts::definitions(TextScale::Normal));
    presets::bootstrap_startup_theme(&ctx);
    ctx
}

fn frame(ctx: &Context, state: &mut PickerState, store: &dyn ThemeStore) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 1400.0))),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        CentralPanel::default().show(ui, |ui| show(ui, state, store));
    })
    .drop_without_applying_deltas();
}

fn run(ctx: &Context, state: &mut PickerState, store: &dyn ThemeStore) {
    frame(ctx, state, store);
    frame(ctx, state, store);
}

const PICK: Color32 = Color32::from_rgb(201, 43, 87);

fn names(store: &dyn ThemeStore) -> Vec<String> {
    store.user_themes().into_iter().map(|t| t.name).collect()
}

fn recognised(theme: &SavedTheme) -> Option<&'static str> {
    presets::exact_preset(&presets::normalized(&theme.style)).map(|d| d.id)
}

#[test]
fn the_picker_draws_under_every_preset_and_density() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    store.save_user_theme("Mine", &SavedTheme::current(&ctx));
    for def in presets::builtin_presets() {
        apply_preset(&ctx, def);
        for density in Density::ALL {
            presets::set_density(&ctx, density);
            run(&ctx, &mut state, &store);
        }
    }
}

#[test]
fn the_picker_draws_at_every_text_size_while_renaming_and_confirming_a_delete() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    store.save_user_theme("Mine", &SavedTheme::current(&ctx));
    for scale in TextScale::ALL {
        presets::set_text_scale(&ctx, scale);
        state.rename = Some(("Mine".to_owned(), "Mine".to_owned()));
        run(&ctx, &mut state, &store);
        state.rename = None;
        state.confirm_delete = Some("Mine".to_owned());
        run(&ctx, &mut state, &store);
        state.confirm_delete = None;
    }
}

#[test]
fn the_picker_draws_without_its_optional_sections_and_with_icons() {
    fn glyphs(name: &str) -> Option<&'static str> {
        (name == "apply").then_some("+")
    }
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::new(PickerOptions {
        icon: Some(glyphs),
        customize: false,
        import_export: false,
        ..Default::default()
    });
    run(&ctx, &mut state, &store);
    assert_eq!(super::label(&state, "apply", "Apply"), "+ Apply");
    assert_eq!(super::label(&state, "save", "Save"), "Save");
}

#[test]
fn the_window_opens_draws_and_closes_flushing_a_pending_save() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::new(PickerOptions {
        debounce: Duration::from_secs(3600),
        ..Default::default()
    });
    let draw = |ctx: &Context, state: &mut PickerState| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 900.0))),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| window(ui.ctx(), state, &store))
            .drop_without_applying_deltas();
    };
    draw(&ctx, &mut state);
    assert!(!state.open);
    assert!(state.origin().is_none(), "a closed window does no work");

    state.open = true;
    draw(&ctx, &mut state);
    draw(&ctx, &mut state);
    assert!(state.origin().is_some());

    state.apply_preset(&ctx, "nebula");
    draw(&ctx, &mut state);
    assert!(
        store.load_active().is_none(),
        "the debounce has not elapsed"
    );
    state.flush(&ctx, &store);
    assert_eq!(recognised(&store.load_active().unwrap()), Some("nebula"));
}

#[test]
fn applying_a_preset_changes_the_live_theme_and_records_its_origin() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    assert_eq!(state.origin(), Some(&Origin::Preset("graphite")));
    assert!(!state.is_modified());

    assert!(state.apply_preset(&ctx, "nebula"));
    run(&ctx, &mut state, &store);
    assert_eq!(presets::installed_preset_id(&ctx), Some("nebula"));
    assert_eq!(state.origin(), Some(&Origin::Preset("nebula")));
    assert!(!state.is_modified());
    assert_eq!(state.current_name(&ctx), "Nebula");
    assert_eq!(state.message().unwrap().kind, MessageKind::Info);

    assert!(!state.apply_preset(&ctx, "no-such-preset"));
    assert_eq!(state.message().unwrap().kind, MessageKind::Error);
    assert_eq!(presets::installed_preset_id(&ctx), Some("nebula"));
}

#[test]
fn applying_a_preset_keeps_the_density_and_text_size() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.set_density(&ctx, Density::Comfortable);
    state.set_text_scale(&ctx, TextScale::Large);
    state.apply_preset(&ctx, PresetStyles::Noir);
    run(&ctx, &mut state, &store);
    assert_eq!(metrics::density(&ctx), Density::Comfortable);
    assert_eq!(fonts::text_scale(&ctx), TextScale::Large);
}

#[test]
fn editing_marks_the_theme_modified_and_resetting_restores_the_preset() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "aurora");
    run(&ctx, &mut state, &store);
    let before = Tokens::get(&ctx);

    state.set_surface(&ctx, Surface::Base, PICK);
    run(&ctx, &mut state, &store);
    assert!(state.is_modified());
    assert_eq!(Tokens::get(&ctx).base, PICK);
    assert_eq!(presets::installed_preset_id(&ctx), None);
    assert_eq!(state.current_name(&ctx), "Custom");
    assert_eq!(state.origin_name().as_deref(), Some("Aurora"));

    state.reset_to_origin(&ctx);
    run(&ctx, &mut state, &store);
    assert!(!state.is_modified());
    assert_eq!(Tokens::get(&ctx), before);
    assert_eq!(presets::installed_preset_id(&ctx), Some("aurora"));
}

#[test]
fn every_kind_of_customization_marks_the_theme_modified() {
    type Edit = Box<dyn Fn(&mut PickerState, &Context)>;
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    let edits: Vec<(&str, Edit)> = vec![
        (
            "surface",
            Box::new(|s, c| s.set_surface(c, Surface::Field, PICK)),
        ),
        ("role", Box::new(|s, c| s.set_role(c, Role::Error, PICK))),
        (
            "semantic",
            Box::new(|s, c| s.set_semantic(c, Semantic::Success, PICK)),
        ),
        (
            "glass",
            Box::new(|s, c| {
                let mut params = glass::params(c);
                params.blur_radius += 7.0;
                s.set_glass(c, params);
            }),
        ),
        (
            "density",
            Box::new(|s, c| s.set_density(c, Density::Comfortable)),
        ),
        (
            "text size",
            Box::new(|s, c| s.set_text_scale(c, TextScale::XLarge)),
        ),
        (
            "glass off",
            Box::new(|s, c| s.set_glass(c, GlassParams::OFF)),
        ),
    ];
    for (name, edit) in edits {
        state.apply_preset(&ctx, "frost");
        run(&ctx, &mut state, &store);
        assert!(!state.is_modified(), "{name}: modified before editing");
        edit(&mut state, &ctx);
        run(&ctx, &mut state, &store);
        assert!(state.is_modified(), "{name}: not marked modified");
        state.reset_to_origin(&ctx);
        run(&ctx, &mut state, &store);
        assert!(!state.is_modified(), "{name}: still modified after reset");
    }
}

#[test]
fn glass_edits_apply_to_the_live_material() {
    let ctx = themed_context();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "frost");
    let mut params = glass::params(&ctx);
    params.blur_radius = 51.0;
    params.presence = 0.5;
    state.set_glass(&ctx, params);
    assert_eq!(glass::params(&ctx), params);
    assert_eq!(SavedTheme::current(&ctx).glass, Some(params));
}

#[test]
fn saving_as_stores_the_live_theme_and_makes_it_the_origin() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "velvet");
    state.set_surface(&ctx, Surface::Base, PICK);
    run(&ctx, &mut state, &store);
    assert!(state.is_modified());

    assert!(state.save_as(&ctx, &store, "  My   Velvet ", false));
    assert_eq!(names(&store), ["My Velvet"]);
    assert_eq!(state.origin(), Some(&Origin::User("My Velvet".to_owned())));
    assert_eq!(state.message().unwrap().kind, MessageKind::Success);
    run(&ctx, &mut state, &store);
    assert!(!state.is_modified());
    assert_eq!(state.current_name(&ctx), "My Velvet");
    let stored = &store.user_themes()[0].theme;
    assert_eq!(super::preview::theme_tokens(stored).base, PICK);

    assert!(!state.save_as(&ctx, &store, "my velvet", false));
    assert_eq!(state.message().unwrap().kind, MessageKind::Error);
    assert!(state.save_as(&ctx, &store, "my velvet", true));
    assert_eq!(names(&store), ["my velvet"]);
    assert!(!state.save_as(&ctx, &store, "   ", false));
    assert_eq!(names(&store).len(), 1);
}

#[test]
fn saving_over_the_origin_replaces_the_stored_theme_and_needs_a_name_first() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "twilight");
    run(&ctx, &mut state, &store);
    assert!(
        !state.save_origin(&ctx, &store),
        "a preset has nothing to save over"
    );
    assert_eq!(state.message().unwrap().kind, MessageKind::Error);

    state.save_as(&ctx, &store, "Dusk", false);
    state.set_role(&ctx, Role::Primary, PICK);
    run(&ctx, &mut state, &store);
    assert!(state.is_modified());
    assert!(state.save_origin(&ctx, &store));
    run(&ctx, &mut state, &store);
    assert!(!state.is_modified());
    assert_eq!(names(&store), ["Dusk"]);
    let stored = &store.user_themes()[0].theme;
    assert!(presets::themes_equal(stored, &SavedTheme::current(&ctx)));
}

#[test]
fn custom_semantic_colors_survive_saving_and_applying_a_user_theme() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "nebula");
    state.set_semantic(&ctx, Semantic::Success, PICK);
    state.save_as(&ctx, &store, "Picked", false);

    state.apply_preset(&ctx, "noir");
    assert_ne!(colors::success_ctx(&ctx), PICK);
    assert!(state.apply_user_theme(&ctx, &store, "picked"));
    assert_eq!(colors::success_ctx(&ctx), PICK);
    assert_eq!(state.origin(), Some(&Origin::User("Picked".to_owned())));
    run(&ctx, &mut state, &store);
    assert!(!state.is_modified());
}

#[test]
fn applying_a_user_theme_that_is_gone_reports_it() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    assert!(!state.apply_user_theme(&ctx, &store, "ghost"));
    assert_eq!(state.message().unwrap().kind, MessageKind::Error);
}

#[test]
fn renaming_duplicating_and_deleting_update_the_store_and_the_origin() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "quartz");
    state.save_as(&ctx, &store, "First", false);
    state.apply_preset(&ctx, "obsidian");
    state.save_as(&ctx, &store, "Second", false);
    assert_eq!(state.origin(), Some(&Origin::User("Second".to_owned())));

    assert!(state.rename(&store, "Second", "Renamed"));
    assert_eq!(names(&store), ["First", "Renamed"]);
    assert_eq!(state.origin(), Some(&Origin::User("Renamed".to_owned())));

    assert!(!state.rename(&store, "Renamed", "first"));
    assert_eq!(state.message().unwrap().kind, MessageKind::Error);
    assert!(state.message().unwrap().text.contains("already exists"));
    assert!(
        store.last_error().is_none(),
        "the failure is reported once, then cleared"
    );
    assert!(!state.rename(&store, "Renamed", "  "));
    assert_eq!(names(&store), ["First", "Renamed"]);

    assert_eq!(
        state.duplicate(&store, "First").as_deref(),
        Some("First copy")
    );
    assert_eq!(
        state.duplicate(&store, "First").as_deref(),
        Some("First copy 2")
    );
    assert_eq!(state.duplicate(&store, "missing"), None);
    assert_eq!(store.user_themes().len(), 4);

    state.delete(&store, "renamed");
    assert_eq!(names(&store), ["First", "First copy", "First copy 2"]);
    assert_eq!(state.origin(), Some(&Origin::Custom));
    assert!(state.confirm_delete.is_none());
}

#[test]
fn a_delete_waits_for_its_confirmation() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    store.save_user_theme("Doomed", &SavedTheme::current(&ctx));
    state.confirm_delete = Some("Doomed".to_owned());
    run(&ctx, &mut state, &store);
    assert_eq!(names(&store), ["Doomed"], "asking does not delete");
    state.confirm_delete = None;
    run(&ctx, &mut state, &store);
    assert_eq!(names(&store), ["Doomed"]);
    state.delete(&store, "Doomed");
    assert!(names(&store).is_empty());
}

#[test]
fn reverting_returns_to_the_theme_the_picker_started_with() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    let start = Tokens::get(&ctx);

    state.apply_preset(&ctx, "supernova");
    state.set_density(&ctx, Density::Comfortable);
    run(&ctx, &mut state, &store);
    assert_ne!(Tokens::get(&ctx), start);

    state.revert(&ctx);
    run(&ctx, &mut state, &store);
    assert_eq!(Tokens::get(&ctx), start);
    assert_eq!(metrics::density(&ctx), Density::Compact);
    assert_eq!(state.origin(), Some(&Origin::Preset("graphite")));
    assert!(!state.is_modified());
}

#[test]
fn saving_moves_the_revert_point() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    state.apply_preset(&ctx, "galactic");
    state.save_as(&ctx, &store, "Kept", false);
    state.apply_preset(&ctx, "rerun");
    state.revert(&ctx);
    assert_eq!(recognised(&SavedTheme::current(&ctx)), Some("galactic"));
    assert_eq!(state.origin(), Some(&Origin::User("Kept".to_owned())));
}

#[test]
fn the_startup_theme_is_pinned_cleared_and_validated() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.save_as(&ctx, &store, "Pinned", false);
    state.set_startup(&store, StartupTheme::User("pinned".to_owned()));
    assert_eq!(store.startup(), StartupTheme::User("Pinned".to_owned()));
    assert_eq!(state.message().unwrap().kind, MessageKind::Success);
    run(&ctx, &mut state, &store);

    state.set_startup(&store, StartupTheme::Preset("nebula".to_owned()));
    assert_eq!(store.startup(), StartupTheme::Preset("nebula".to_owned()));
    state.set_startup(&store, StartupTheme::LastUsed);
    assert_eq!(store.startup(), StartupTheme::LastUsed);

    state.set_startup(&store, StartupTheme::User("nobody".to_owned()));
    assert_eq!(state.message().unwrap().kind, MessageKind::Error);
    assert_eq!(store.startup(), StartupTheme::LastUsed);
    assert!(store.last_error().is_none());
}

#[test]
fn exporting_and_importing_json_round_trips_and_rejects_garbage() {
    let ctx = themed_context();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "event-horizon");
    state.set_semantic(&ctx, Semantic::Secondary, PICK);
    let json = state.export_json(&ctx);
    assert!(json.contains("\n  "), "the export is indented");

    let fresh = themed_context();
    let mut other = PickerState::default();
    assert!(other.import_json(&fresh, &json));
    assert!(presets::themes_equal(
        &SavedTheme::current(&fresh),
        &SavedTheme::current(&ctx)
    ));
    assert_eq!(other.message().unwrap().kind, MessageKind::Success);

    let before = Tokens::get(&fresh);
    assert!(!other.import_json(&fresh, "{ not json"));
    assert_eq!(other.message().unwrap().kind, MessageKind::Error);
    assert_eq!(Tokens::get(&fresh), before);
}

#[test]
fn a_bare_style_import_is_recognised_as_its_preset() {
    let ctx = themed_context();
    let mut state = PickerState::default();
    let style = presets::style_for_preset(PresetStyles::Quartz);
    assert!(state.import_json(&ctx, &serde_json::to_string(&style).unwrap()));
    assert_eq!(presets::installed_preset_id(&ctx), Some("quartz"));
    assert_eq!(state.origin(), Some(&Origin::Preset("quartz")));

    let mut edited = style;
    edited.visuals.hyperlink_color = PICK;
    assert!(state.import_json(&ctx, &serde_json::to_string(&edited).unwrap()));
    assert_eq!(state.origin(), Some(&Origin::Imported));
}

#[test]
fn changes_made_through_the_picker_reach_the_store_after_the_debounce() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::new(PickerOptions {
        debounce: Duration::ZERO,
        ..Default::default()
    });
    run(&ctx, &mut state, &store);
    assert!(
        store.load_active().is_none(),
        "the startup theme is not re-saved"
    );

    state.apply_preset(&ctx, "velvet");
    run(&ctx, &mut state, &store);
    assert_eq!(recognised(&store.load_active().unwrap()), Some("velvet"));

    state.set_role(&ctx, Role::Warn, PICK);
    run(&ctx, &mut state, &store);
    let saved = store.load_active().unwrap();
    assert_eq!(saved.style.visuals.warn_fg_color, PICK);

    state.apply_preset(&ctx, "graphite");
    state.set_density(&ctx, Density::Compact);
    state.set_text_scale(&ctx, TextScale::Small);
    run(&ctx, &mut state, &store);
    assert!(
        store.load_active().is_none(),
        "returning to the default clears the entry"
    );
}

#[test]
fn the_lists_follow_the_store_between_frames() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    assert!(state.user_themes().is_empty());

    store.save_user_theme("From elsewhere", &SavedTheme::current(&ctx));
    run(&ctx, &mut state, &store);
    assert_eq!(state.user_themes().len(), 1);
    assert!(state.user_tokens.contains_key("From elsewhere"));
    let applied = state.live.as_ref().and_then(|l| l.applied_user.clone());
    assert_eq!(applied.as_deref(), Some("From elsewhere"));

    store.delete_user_theme("From elsewhere");
    run(&ctx, &mut state, &store);
    assert!(state.user_themes().is_empty());
    assert!(state.user_tokens.is_empty());
}

#[test]
fn a_saved_theme_matching_the_live_one_becomes_the_origin_on_first_sight() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    apply_preset(&ctx, PresetStyles::Rerun);
    store.save_user_theme("Mine", &SavedTheme::current(&ctx));
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    assert_eq!(state.origin(), Some(&Origin::User("Mine".to_owned())));
    assert_eq!(state.current_name(&ctx), "Mine");
}

#[test]
fn a_custom_theme_on_first_sight_has_no_origin_name() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut style = (*ctx.global_style()).clone();
    style.visuals.hyperlink_color = PICK;
    presets::apply_custom_style(&ctx, style, crate::saved::SemanticColors::current(&ctx));
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    assert_eq!(state.origin(), Some(&Origin::Custom));
    assert_eq!(state.origin_name(), None);
    assert_eq!(state.current_name(&ctx), "Custom");
    assert!(!state.is_modified());
}

#[test]
fn contrast_warnings_render_for_an_unreadable_theme() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.set_surface(&ctx, Surface::Text, Color32::from_rgb(12, 12, 14));
    run(&ctx, &mut state, &store);
    assert!(!super::edit::contrast_warnings(&Tokens::get(&ctx)).is_empty());
}

#[test]
fn ages_read_naturally() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert_eq!(super::age(0), "date unknown");
    assert_eq!(super::age(now), "just now");
    assert_eq!(super::age(now - 300), "5 min ago");
    assert_eq!(super::age(now - 7_300), "2 h ago");
    assert_eq!(super::age(now - 3 * 86_400), "3 d ago");
    assert!(super::age(now - 90 * 86_400).ends_with("mo ago"));
}

#[test]
fn preset_search_matches_names_ids_groups_and_descriptions() {
    use super::gallery::matches;
    let nebula = presets::preset("nebula").unwrap();
    for query in ["", "neb", "neon", "violet"] {
        assert!(matches(&nebula, query), "{query}");
    }
    assert!(!matches(&nebula, "zzz"));
    let horizon = presets::preset("event-horizon").unwrap();
    assert!(matches(&horizon, "event"));
    assert!(!matches(&horizon, "nebula"));
}

#[test]
fn revert_and_reset_are_offered_only_when_they_would_change_something() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    run(&ctx, &mut state, &store);
    assert!(!state.can_revert());
    assert!(!state.is_modified());
    assert_eq!(state.reset_label(), "Reset to preset");

    state.apply_preset(&ctx, "noir");
    run(&ctx, &mut state, &store);
    assert!(
        state.can_revert(),
        "the theme moved away from the starting one"
    );
    assert!(!state.is_modified());

    state.revert(&ctx);
    run(&ctx, &mut state, &store);
    assert!(!state.can_revert());

    state.save_as(&ctx, &store, "Saved", false);
    assert_eq!(state.reset_label(), "Reset to saved");
    state.set_surface(&ctx, Surface::Base, PICK);
    run(&ctx, &mut state, &store);
    assert!(state.can_revert() && state.is_modified());

    state.import_json(&ctx, &state.export_json(&ctx));
    assert_eq!(state.reset_label(), "Reset");
}

#[test]
fn glassifying_through_the_picker_is_a_modification() {
    let ctx = themed_context();
    let store = MemoryStore::default();
    let mut state = PickerState::default();
    state.apply_preset(&ctx, "tokyo-night");
    run(&ctx, &mut state, &store);
    state.glassify(&ctx);
    run(&ctx, &mut state, &store);
    assert!(state.is_modified());
    assert!(glass::params(&ctx).is_visible());
    state.reset_to_origin(&ctx);
    assert_eq!(presets::installed_preset_id(&ctx), Some("tokyo-night"));
    assert!(!glass::params(&ctx).is_visible());
}
