//! Cross-module theme tests: stock control geometry per preset and density, init behavior, and source hygiene checks.

use std::fs;
use std::path::PathBuf;

use eframe::egui::{
    Button, CentralPanel, ComboBox, Context, DragValue, Pos2, RawInput, Rect, TextEdit, Ui, vec2,
};
use regex::Regex;

use super::{
    fonts::{self, TextScale},
    metrics::{Density, Metrics},
    presets::{
        MATCHABLE_PRESETS, PresetStyles, apply_preset, bootstrap_startup_theme, installed_preset,
        set_density,
    },
    tokens::Tokens,
};

fn context() -> Context {
    let ctx = Context::default();
    ctx.set_fonts(fonts::definitions(TextScale::Normal));
    bootstrap_startup_theme(&ctx);
    ctx
}

/// Runs `passes` frames of `f` in a central panel; returns the last result.
fn run<R>(ctx: &Context, passes: usize, mut f: impl FnMut(&mut Ui) -> R) -> R {
    let mut out = None;
    for _ in 0..passes {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 400.0))),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            CentralPanel::default().show(ui, |ui| out = Some(f(ui)));
        })
        .drop_without_applying_deltas();
    }
    out.expect("the ui closure ran")
}

/// Rects of a stock Button, ComboBox, DragValue and TextEdit in one row.
fn stock_row(ui: &mut Ui) -> Vec<Rect> {
    let mut text = "Text".to_owned();
    let mut size = 20.0_f32;
    ui.horizontal(|ui| {
        let combo = ComboBox::from_id_salt("stock_row")
            .width(100.0)
            .selected_text("One")
            .show_ui(ui, |ui| {
                let _ = ui.selectable_label(true, "One");
            });
        vec![
            ui.button("Apply").rect,
            combo.response.rect,
            ui.add(DragValue::new(&mut size)).rect,
            ui.add(TextEdit::singleline(&mut text).desired_width(120.0))
                .rect,
        ]
    })
    .inner
}

/// A matchable preset other than the one the shipped default installs.
fn other_preset(ctx: &Context) -> PresetStyles {
    let current = installed_preset(ctx);
    MATCHABLE_PRESETS
        .into_iter()
        .find(|p| *p != current)
        .expect("more than one preset")
}

#[test]
fn stock_controls_share_the_control_height_and_centre_line_for_every_preset_and_density() {
    let ctx = context();
    for preset in MATCHABLE_PRESETS {
        apply_preset(&ctx, preset);
        for density in Density::ALL {
            set_density(&ctx, density);
            let rects = run(&ctx, 2, stock_row);
            let height = Metrics::for_density(density).control_h;
            let first = rects[0];
            for (i, r) in rects.iter().enumerate() {
                assert!(
                    (r.height() - height).abs() < 0.01,
                    "{} at {density:?}: control {i} is {r:?}, want height {height}",
                    preset.as_str()
                );
                assert!(
                    (r.center().y - first.center().y).abs() < 0.01,
                    "{} at {density:?}: control {i} is off the centre line",
                    preset.as_str()
                );
            }
        }
    }
}

#[test]
fn init_pins_the_dark_slot_and_applies_the_shipped_default() {
    let reference = context();
    let ctx = Context::default();
    ctx.set_theme(eframe::egui::Theme::Light);
    super::init_context(&ctx, None, super::KitConfig::new().parse_cli(false));
    assert_eq!(ctx.theme(), eframe::egui::Theme::Dark);
    assert_eq!(installed_preset(&ctx), installed_preset(&reference));
    assert_eq!(Tokens::get(&ctx), Tokens::get(&reference));
    assert_eq!(
        super::persist::source(&ctx),
        super::persist::ThemeSource::Embedded
    );
}

#[test]
#[allow(deprecated)]
fn the_deprecated_bootstrap_matches_init_with_defaults() {
    let reference = Context::default();
    super::init_context(&reference, None, super::KitConfig::new().parse_cli(false));
    let ctx = Context::default();
    super::bootstrap_context(&ctx, None);
    assert_eq!(installed_preset(&ctx), installed_preset(&reference));
    assert_eq!(Tokens::get(&ctx), Tokens::get(&reference));
    assert_eq!(fonts::text_scale(&ctx), fonts::text_scale(&reference));
}

#[test]
fn preferences_replace_the_options_but_keep_the_installed_style() {
    let ctx = context();
    let style = ctx.global_style();
    let mut options = eframe::egui::Options::default();
    options.zoom_factor = 1.5;
    super::apply_preferences(&ctx, options);
    assert_eq!(ctx.options(|o| o.zoom_factor), 1.5);
    assert!(std::sync::Arc::ptr_eq(&style, &ctx.global_style()));
}

#[test]
fn a_prominent_primary_button_is_taller_than_a_stock_one() {
    use eframe::egui::class::HasClasses as _;
    let ctx = context();
    let (stock, prominent) = run(&ctx, 2, |ui| {
        ui.horizontal(|ui| {
            (
                ui.button("Apply").rect,
                ui.add(Button::new("Save").with_class(super::widget_theme::PROMINENT))
                    .rect,
            )
        })
        .inner
    });
    assert!(prominent.height() > stock.height());
}

#[cfg(feature = "dock")]
#[test]
fn dock_chrome_builds_from_the_tokens_for_every_preset_and_density() {
    let ctx = context();
    for preset in MATCHABLE_PRESETS {
        apply_preset(&ctx, preset);
        for density in Density::ALL {
            set_density(&ctx, density);
            let dock = super::dock_style::style(&ctx);
            let t = Tokens::get(&ctx);
            let m = Metrics::for_density(density);
            assert_eq!(dock.tab.tab_body.bg_fill, t.raised, "{}", preset.as_str());
            let margin = dock.tab_bar.inner_margin;
            let pill = dock.tab_bar.height - f32::from(margin.top + margin.bottom);
            assert_eq!(pill, m.control_h, "{} at {density:?}", preset.as_str());
        }
    }
}

#[test]
fn leaving_and_returning_to_the_default_preset_restores_the_published_look() {
    let ctx = context();
    let default = installed_preset(&ctx);
    let before = Tokens::get(&ctx);
    apply_preset(&ctx, other_preset(&ctx));
    assert_ne!(Tokens::get(&ctx), before);
    apply_preset(&ctx, default);
    assert_eq!(Tokens::get(&ctx), before);
}

/// Most `Color32` constructors and named colours in the kit's widget-painting modules.
const COLOR_LITERALS: usize = 4;
/// Most `add_space` calls whose argument starts with a number literal in the kit's widget-painting modules.
const ADD_SPACE_LITERALS: usize = 0;

/// Modules that paint widgets from the tokens.
const THEMED: &[&str] = &[
    "dock_style.rs",
    "kit.rs",
    "table_style.rs",
    "toast.rs",
    "widget_theme.rs",
];

/// Modules owned by the core, checked for app-specific names.
const NEUTRAL: &[&str] = &[
    "cli.rs",
    "colors.rs",
    "dock_style.rs",
    "fonts.rs",
    "glass.rs",
    "startup.rs",
    "kit.rs",
    "metrics.rs",
    "saved.rs",
    "table_style.rs",
    "toast.rs",
    "tokens.rs",
    "widget_theme.rs",
];

fn source_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The file's text before its `#[cfg(test)]` module.
fn production_part(text: &str) -> &str {
    text.split("\n#[cfg(test)]").next().unwrap_or(text)
}

fn themed_sources() -> Vec<(String, String)> {
    THEMED
        .iter()
        .map(|name| {
            let text = fs::read_to_string(source_root().join(name))
                .unwrap_or_else(|e| panic!("cannot read {name}: {e}"));
            ((*name).to_owned(), production_part(&text).to_owned())
        })
        .collect()
}

fn ratchet(name: &str, ceiling: usize, pattern: &Regex, skip: impl Fn(&str) -> bool) {
    let mut per_file: Vec<(usize, String)> = themed_sources()
        .into_iter()
        .map(|(path, text)| {
            let n = pattern
                .find_iter(&text)
                .filter(|m| !skip(m.as_str()))
                .count();
            (n, path)
        })
        .filter(|(n, _)| *n > 0)
        .collect();
    let total: usize = per_file.iter().map(|(n, _)| n).sum();
    per_file.sort_unstable_by(|a, b| b.cmp(a));
    let top: Vec<String> = per_file
        .iter()
        .take(6)
        .map(|(n, p)| format!("{n} {p}"))
        .collect();
    assert!(
        total <= ceiling,
        "{name}: {total} found, ceiling {ceiling}; use theme tokens and semantic colors instead. Top files: {top:#?}"
    );
}

fn color_pattern() -> Regex {
    Regex::new(r"Color32::(?:from_\w+\(|[A-Z][A-Z0-9_]*\b)").unwrap()
}

fn add_space_pattern() -> Regex {
    Regex::new(r"add_space\(\s*[-+]?\d[\d_.]*(?:_?f32)?\s*\)").unwrap()
}

#[test]
fn color_literals_in_widget_painting_modules_only_go_down() {
    ratchet("Color32 literals", COLOR_LITERALS, &color_pattern(), |m| {
        matches!(m, "Color32::TRANSPARENT" | "Color32::PLACEHOLDER")
    });
}

#[test]
fn add_space_literals_in_widget_painting_modules_only_go_down() {
    ratchet(
        "add_space(<literal>)",
        ADD_SPACE_LITERALS,
        &add_space_pattern(),
        |_| false,
    );
}

#[test]
fn the_ratchet_patterns_see_each_pattern_and_skip_lookalikes() {
    let text = "let a = Color32::from_rgb(1, 2, 3); let b = egui::Color32::RED; let c = Color32::TRANSPARENT;";
    let counted = color_pattern()
        .find_iter(text)
        .filter(|m| m.as_str() != "Color32::TRANSPARENT")
        .count();
    assert_eq!(counted, 2);
    assert_eq!(
        add_space_pattern()
            .find_iter("ui.add_space(4.0); ui.add_space(m.gap.y); ui.add_space(2.0 * m.gap.y);")
            .count(),
        1
    );
    assert_eq!(
        production_part("fn a() {}\n#[cfg(test)]\nmod tests { Color32::RED }"),
        "fn a() {}"
    );
}

#[test]
fn core_sources_name_no_application() {
    let forbidden = Regex::new(r"(?i)smart[_ ]?media|mastertech|mtech|aimedia|surreal").unwrap();
    for name in NEUTRAL {
        let text = fs::read_to_string(source_root().join(name))
            .unwrap_or_else(|e| panic!("cannot read {name}: {e}"));
        if let Some(hit) = forbidden.find(&text) {
            panic!("{name} mentions {:?}", hit.as_str());
        }
    }
}
