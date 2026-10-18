//! The "Theme" debug window: [`show`] draws it when open, [`toggle`] and [`is_open`] drive it, and Ctrl+Alt+T toggles it.

#[cfg(feature = "files")]
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(any(feature = "files", feature = "toast", feature = "picker"))]
use std::sync::Mutex;
use std::time::Duration;

use eframe::egui::class::HasClasses as _;
use eframe::egui::{
    self, CollapsingHeader, Color32, ComboBox, Context, DragValue, Grid, Id, Key, KeyboardShortcut,
    Modifiers, ProgressBar, RichText, ScrollArea, Sense, Slider, Spinner, Stroke, StrokeKind,
    TextEdit, TextStyle, Ui, Vec2, Window, vec2,
};
#[cfg(feature = "toast")]
use egui_toast::ToastKind;

use super::{
    colors,
    fonts::{self, RichTextExt as _, TextScale},
    glass::{self, GlassParams},
    kit::{self, ButtonKind},
    metrics::{self, Density, Metrics},
    persist,
    presets::{self, apply_preset, apply_saved_theme, grouped_presets, installed_def},
    saved::{SavedTheme, theme_from_json, theme_to_json},
    store::ThemeStore,
    tokens::{Tokens, contrast},
    widget_theme,
};

const SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::ALT), Key::T);
/// Smoothing of the frame-time readout per sample.
const EMA: f32 = 0.05;
/// Tallest the JSON box grows before it scrolls.
const JSON_BOX_HEIGHT: f32 = 110.0;
/// Width of one swatch chip.
const CHIP: Vec2 = vec2(22.0, 14.0);

#[cfg(feature = "files")]
static DIALOG_RESULTS: Mutex<Vec<DialogResult>> = Mutex::new(Vec::new());
#[cfg(feature = "toast")]
static TOAST_REQUESTS: Mutex<Vec<(ToastKind, String)>> = Mutex::new(Vec::new());

/// Outcome of a file dialog run off the UI thread.
#[cfg(feature = "files")]
enum DialogResult {
    Imported(Result<Box<SavedTheme>, String>, PathBuf),
    Exported(Result<PathBuf, String>),
}

fn open_id() -> Id {
    Id::unique("egui_theme_kit.panel.open")
}

fn state_id() -> Id {
    Id::unique("egui_theme_kit.panel.state")
}

fn stats_id() -> Id {
    Id::unique("egui_theme_kit.panel.frame_stats")
}

/// Whether the Theme window is open.
pub fn is_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false)
}

/// Opens or closes the Theme window.
pub fn set_open(ctx: &Context, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(), open));
    ctx.request_repaint();
}

/// Flips the Theme window.
pub fn toggle(ctx: &Context) {
    set_open(ctx, !is_open(ctx));
}

/// Toast requests raised by the panel's tester buttons, drained by the app each frame.
#[cfg(feature = "toast")]
pub fn take_toast_requests() -> Vec<(ToastKind, String)> {
    TOAST_REQUESTS
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

#[derive(Clone, Copy, Default)]
struct FrameStats {
    ui_ms: f32,
    frame_ms: f32,
}

/// Moves `current` toward `sample`, starting from the first sample.
fn smooth(current: f32, sample: f32) -> f32 {
    if current == 0.0 {
        sample
    } else {
        current + (sample - current) * EMA
    }
}

/// Records how long building the UI took this frame, for the frame-time readout.
pub fn record_ui_time(ctx: &Context, elapsed: Duration) {
    let sample = elapsed.as_secs_f32() * 1000.0;
    ctx.data_mut(|d| {
        let stats = d.get_temp_mut_or_default::<FrameStats>(stats_id());
        stats.ui_ms = smooth(stats.ui_ms, sample);
    });
}

/// Records the previous frame's CPU seconds from eframe, UI and rendering included.
pub fn record_frame_cpu(ctx: &Context, seconds: Option<f32>) {
    let Some(seconds) = seconds else {
        return;
    };
    ctx.data_mut(|d| {
        let stats = d.get_temp_mut_or_default::<FrameStats>(stats_id());
        stats.frame_ms = smooth(stats.frame_ms, seconds * 1000.0);
    });
}

#[derive(Clone)]
struct Gallery {
    text: String,
    multiline: String,
    check: bool,
    check_off: bool,
    radio: u8,
    slider: f32,
    drag: f32,
    combo: usize,
    progress: f32,
    selected: usize,
    collapse_open: bool,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            text: "Sample text".to_owned(),
            multiline: "Multi-line\ntext edit".to_owned(),
            check: true,
            check_off: false,
            radio: 0,
            slider: 40.0,
            drag: 12.5,
            combo: 1,
            progress: 0.62,
            selected: 1,
            collapse_open: true,
        }
    }
}

/// The tabs of the Theme window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Themes,
    Inspect,
    Widgets,
    Tools,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Themes, Self::Inspect, Self::Widgets, Self::Tools];

    fn label(self) -> &'static str {
        match self {
            Self::Themes => "Themes",
            Self::Inspect => "Inspect",
            Self::Widgets => "Widgets",
            Self::Tools => "Tools",
        }
    }
}

#[derive(Clone, Default)]
struct PanelState {
    tab: Tab,
    json: String,
    json_message: Option<(bool, String)>,
    gallery: Gallery,
    #[cfg(feature = "picker")]
    picker: Arc<Mutex<super::picker::PickerState>>,
}

fn store_id() -> Id {
    Id::unique("egui_theme_kit.panel.store")
}

/// Sets the store the Themes tab saves to; without one the tab keeps themes in memory only.
pub fn set_store(ctx: &Context, store: Arc<dyn ThemeStore>) {
    ctx.data_mut(|d| d.insert_temp(store_id(), store));
}

/// The store set by [`set_store`], or an in-memory one created on first use.
#[cfg(any(feature = "picker", test))]
fn store(ctx: &Context) -> Arc<dyn ThemeStore> {
    if let Some(store) = ctx.data(|d| d.get_temp::<Arc<dyn ThemeStore>>(store_id())) {
        return store;
    }
    let store: Arc<dyn ThemeStore> = Arc::new(super::store::MemoryStore::default());
    set_store(ctx, Arc::clone(&store));
    store
}

/// Draws the Theme window when it is open and handles its shortcut; call once per frame.
pub fn show(ctx: &Context) {
    if ctx.input_mut(|i| i.consume_shortcut(&SHORTCUT)) {
        toggle(ctx);
    }
    if !is_open(ctx) {
        return;
    }
    let mut open = true;
    let mut state = ctx
        .data_mut(|d| d.remove_temp::<PanelState>(state_id()))
        .unwrap_or_default();
    #[cfg(feature = "files")]
    drain_dialogs(ctx, &mut state);
    Window::new("Theme")
        .id(Id::unique("egui_theme_kit.panel.window"))
        .open(&mut open)
        .default_size(vec2(560.0, 700.0))
        .show(ctx, |ui| contents(ui, &mut state));
    ctx.data_mut(|d| d.insert_temp(state_id(), state));
    if !open {
        set_open(ctx, false);
    }
}

#[cfg(feature = "files")]
fn drain_dialogs(ctx: &Context, state: &mut PanelState) {
    let results = DIALOG_RESULTS
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default();
    for result in results {
        match result {
            DialogResult::Imported(Ok(theme), path) => {
                apply_saved_theme(ctx, *theme);
                state.json_message = Some((true, format!("Imported {}", path.display())));
            }
            DialogResult::Imported(Err(e), path) => {
                state.json_message = Some((false, format!("{}: {e}", path.display())));
            }
            DialogResult::Exported(Ok(path)) => {
                state.json_message = Some((true, format!("Exported to {}", path.display())));
            }
            DialogResult::Exported(Err(e)) => state.json_message = Some((false, e)),
        }
    }
}

fn contents(ui: &mut Ui, state: &mut PanelState) {
    let ctx = ui.ctx().clone();
    status_line(ui, &ctx);
    ui.horizontal(|ui| {
        for tab in Tab::ALL {
            ui.selectable_value(&mut state.tab, tab, tab.label());
        }
    });
    ui.separator();
    match state.tab {
        Tab::Themes => themes_tab(ui, state),
        Tab::Inspect => scrolled(ui, "inspect", |ui| {
            section(ui, "Quick settings", true, |ui| presets_section(ui, &ctx));
            section(ui, "Token swatches", true, |ui| token_section(ui, &ctx));
            section(ui, "Semantic colors", false, semantic_section);
            section(ui, "Metrics", false, |ui| metrics_section(ui, &ctx));
        }),
        Tab::Widgets => scrolled(ui, "widgets", |ui| gallery(ui, &mut state.gallery)),
        Tab::Tools => scrolled(ui, "tools", |ui| {
            section(ui, "Glass", true, |ui| glass_section(ui, &ctx));
            section(ui, "JSON", false, |ui| json_section(ui, &ctx, state));
            #[cfg(feature = "toast")]
            section(ui, "Toast tester", false, toast_section);
        }),
    }
}

fn scrolled(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    ScrollArea::vertical()
        .id_salt(("egui_theme_kit.panel.scroll", id))
        .auto_shrink([false, false])
        .show(ui, add);
}

#[cfg(feature = "picker")]
fn themes_tab(ui: &mut Ui, state: &mut PanelState) {
    let store = store(ui.ctx());
    if let Ok(mut picker) = state.picker.lock() {
        super::picker::show(ui, &mut picker, &*store);
    }
}

#[cfg(not(feature = "picker"))]
fn themes_tab(ui: &mut Ui, _state: &mut PanelState) {
    ui.label(RichText::new("Enable the `picker` feature for the theme picker.").weak());
}

fn section(ui: &mut Ui, title: &str, open: bool, add: impl FnOnce(&mut Ui)) {
    CollapsingHeader::new(RichText::new(title).semibold())
        .default_open(open)
        .show(ui, add);
}

fn status_line(ui: &mut Ui, ctx: &Context) {
    let name = installed_def(ctx).map_or("Custom", |def| def.name);
    let stats = ctx
        .data(|d| d.get_temp::<FrameStats>(stats_id()))
        .unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(name).semibold());
        ui.label(
            RichText::new(format!(
                "{} / {} / source: {}",
                metrics::density(ctx).label(),
                fonts::text_scale(ctx).label(),
                persist::source(ctx).label()
            ))
            .weak(),
        );
        ui.label(
            RichText::new(format!(
                "UI {:.1} ms, frame CPU {:.1} ms",
                stats.ui_ms, stats.frame_ms
            ))
            .weak()
            .small(),
        );
    });
}

fn presets_section(ui: &mut Ui, ctx: &Context) {
    let current = installed_def(ctx);
    ui.horizontal(|ui| {
        ui.label("Preset");
        let mut picked = None;
        ComboBox::from_id_salt("egui_theme_kit.panel.preset")
            .selected_text(current.map_or("Custom", |def| def.name))
            .show_ui(ui, |ui| {
                for group in grouped_presets() {
                    ui.label(RichText::new(group.title).weak().small());
                    for def in group.presets {
                        if ui
                            .selectable_label(current == Some(def), def.name)
                            .on_hover_text(def.description)
                            .clicked()
                        {
                            picked = Some(def);
                        }
                    }
                    ui.separator();
                }
                ui.add_enabled(
                    false,
                    egui::Button::new("Custom").selected(current.is_none()),
                );
            });
        if let Some(def) = picked.filter(|d| Some(*d) != current) {
            apply_preset(ctx, def);
        }
        if ui
            .add(kit::button(ButtonKind::Ghost, "Reset to default"))
            .on_hover_text("Apply the default theme with its density, text size and glass.")
            .clicked()
        {
            apply_saved_theme(ctx, persist::default_theme());
        }
    });

    ui.horizontal(|ui| {
        let mut density = metrics::density(ctx);
        ui.label("Density");
        ComboBox::from_id_salt("egui_theme_kit.panel.density")
            .selected_text(density.label())
            .show_ui(ui, |ui| {
                for d in Density::ALL {
                    ui.selectable_value(&mut density, d, d.label());
                }
            });
        presets::set_density(ctx, density);

        let mut scale = fonts::text_scale(ctx);
        ui.label("Text size");
        ComboBox::from_id_salt("egui_theme_kit.panel.scale")
            .selected_text(scale.label())
            .show_ui(ui, |ui| {
                for s in TextScale::ALL {
                    ui.selectable_value(&mut scale, s, s.label());
                }
            });
        presets::set_text_scale(ctx, scale);
    });

    let mut stock = !widget_theme::enabled(ctx);
    if ui
        .checkbox(
            &mut stock,
            "Stock egui widget look (theme widget styling off)",
        )
        .changed()
    {
        widget_theme::set_enabled(ctx, !stock);
    }
}

fn glass_section(ui: &mut Ui, ctx: &Context) {
    let mut params = glass::params(ctx);
    let before = params;
    let status = glass::status();

    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(status.backend_installed && !status.disabled, |ui| {
            ui.checkbox(&mut params.enabled, "Backdrop blur");
        });
        ui.label(RichText::new(format!("frost: {}", status.summary())).weak());
    });
    ui.add_enabled_ui(params.enabled, |ui| {
        Grid::new("theme_panel_glass_grid")
            .num_columns(2)
            .spacing(vec2(12.0, 4.0))
            .show(ui, |ui| {
                ui.label("Blur radius");
                ui.add(
                    DragValue::new(&mut params.blur_radius)
                        .range(0.0..=64.0)
                        .speed(0.5)
                        .suffix(" px"),
                );
                ui.end_row();
                ui.label("Tint");
                ui.color_edit_button_srgba(&mut params.tint);
                ui.end_row();
                ui.label("Corner radius");
                ui.add(
                    DragValue::new(&mut params.corner_radius)
                        .range(0.0..=32.0)
                        .speed(0.2)
                        .suffix(" px"),
                );
                ui.end_row();
                ui.label("Presence");
                ui.add(Slider::new(&mut params.presence, 0.0..=1.0).step_by(0.01));
                ui.end_row();
            });
    });
    if params != before {
        glass::set_params(ctx, params);
    }

    ui.horizontal_wrapped(|ui| {
        if ui
            .add(kit::button(ButtonKind::Ghost, "Material off"))
            .clicked()
        {
            glass::set_params(ctx, GlassParams::OFF);
        }
        if ui
            .add(kit::button(ButtonKind::Ghost, "Disable frost"))
            .on_hover_text("Trips the process-wide kill switch, like --no-frost.")
            .clicked()
        {
            glass::disable("disabled from the Theme panel");
        }
        if ui
            .add(kit::button(ButtonKind::Ghost, "Re-enable frost"))
            .on_hover_text(
                "Clears the kill switch and the failure run; the environment switch stays.",
            )
            .clicked()
        {
            glass::reenable();
        }
    });

    Grid::new("theme_panel_frost_grid")
        .num_columns(2)
        .spacing(vec2(12.0, 2.0))
        .show(ui, |ui| {
            let row = |ui: &mut Ui, name: &str, value: String| {
                ui.label(RichText::new(name).weak());
                ui.label(value);
                ui.end_row();
            };
            row(ui, "GL backend", yes_no(status.backend_installed));
            row(ui, "Kill switch", yes_no(status.disabled));
            row(ui, "Environment switch", yes_no(status.env_disabled));
            row(
                ui,
                "Last frame",
                status
                    .last
                    .map_or("none yet".to_owned(), |r| r.label().to_owned()),
            );
            row(
                ui,
                "Composited frames",
                status.composited_frames.to_string(),
            );
            row(ui, "Failed run", status.consecutive_failures.to_string());
            row(
                ui,
                "Reason",
                status.reason.clone().unwrap_or_else(|| "none".to_owned()),
            );
        });

    glass::preview(ui, glass::params(ctx));
}

fn yes_no(v: bool) -> String {
    if v { "yes" } else { "no" }.to_owned()
}

fn chip(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(CHIP, Sense::hover());
    ui.painter().rect_filled(rect, 3.0, color);
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, colors::border(ui)),
        StrokeKind::Inside,
    );
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn token_section(ui: &mut Ui, ctx: &Context) {
    let t = Tokens::get(ctx);
    Grid::new("theme_panel_tokens")
        .num_columns(6)
        .spacing(vec2(8.0, 3.0))
        .show(ui, |ui| {
            for (i, (name, color)) in t.colors().into_iter().enumerate() {
                chip(ui, color);
                ui.label(name);
                ui.label(RichText::new(hex(color)).weak().monospace());
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(4.0);
    ui.label(format!(
        "radii: sm {}  md {}  lg {}   contrast text/base {:.1}, text_dim/base {:.1}",
        t.radius_sm,
        t.radius_md,
        t.radius_lg,
        contrast(t.text, t.base),
        contrast(t.text_dim, t.base)
    ));
}

fn semantic_section(ui: &mut Ui) {
    let rows: [(&str, Color32); 17] = [
        ("error", colors::error(ui)),
        ("warn", colors::warn(ui)),
        ("info", colors::info(ui)),
        ("success", colors::success(ui)),
        ("accent", colors::accent(ui)),
        ("accent_secondary", colors::accent_secondary(ui)),
        ("primary", colors::primary(ui)),
        ("secondary", colors::secondary(ui)),
        ("tertiary", colors::tertiary(ui)),
        ("strong_text", colors::strong_text(ui)),
        ("text", colors::text(ui)),
        ("weak_text", colors::weak_text(ui)),
        ("faint_text", colors::faint_text(ui)),
        ("border", colors::border(ui)),
        ("bg_surface", colors::bg_surface(ui)),
        ("bg_faint", colors::bg_faint(ui)),
        ("bg_extreme", colors::bg_extreme(ui)),
    ];
    Grid::new("theme_panel_semantic")
        .num_columns(4)
        .spacing(vec2(8.0, 3.0))
        .show(ui, |ui| {
            for (i, (name, color)) in rows.into_iter().enumerate() {
                chip(ui, color);
                ui.label(name);
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("tag colors");
        for i in 0..colors::TAG_LEN {
            chip(ui, colors::tag_color(ui, i));
        }
        ui.label("series");
        for i in 0..colors::SERIES_LEN {
            if let Some(c) = colors::series_color(i) {
                chip(ui, c);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("usage");
        for pct in [10.0, 75.0, 95.0] {
            ui.colored_label(colors::usage_level(ui, pct), format!("{pct:.0}%"));
        }
    });
}

fn metrics_section(ui: &mut Ui, ctx: &Context) {
    let m = Metrics::get(ctx);
    let rows = [
        ("control_h", m.control_h),
        ("button_h", m.button_h),
        ("pad.x", m.pad.x),
        ("pad.y", m.pad.y),
        ("gap.x", m.gap.x),
        ("gap.y", m.gap.y),
        ("window_margin", m.window_margin as f32),
        ("menu_margin", m.menu_margin as f32),
        ("combo_w", m.combo_w),
        ("slider_w", m.slider_w),
        ("tooltip_w", m.tooltip_w),
        ("icon_sm", m.icon_sm),
        ("icon_md", m.icon_md),
        ("icon_lg", m.icon_lg),
        ("row_h", m.row_h),
        ("header_h", m.header_h),
        ("bar_h", m.bar_h),
        ("footer_h", m.footer_h),
    ];
    Grid::new("theme_panel_metrics")
        .num_columns(6)
        .spacing(vec2(8.0, 2.0))
        .show(ui, |ui| {
            for (i, (name, value)) in rows.into_iter().enumerate() {
                ui.label(RichText::new(name).weak());
                ui.label(RichText::new(format!("{value}")).monospace());
                if i % 3 == 2 {
                    ui.end_row();
                }
            }
        });
    let style = ctx.global_style();
    ui.add_space(4.0);
    ui.label(format!(
        "type scale: small {}  body {}  button {}  monospace {}  heading {}",
        style.text_styles[&TextStyle::Small].size,
        style.text_styles[&TextStyle::Body].size,
        style.text_styles[&TextStyle::Button].size,
        style.text_styles[&TextStyle::Monospace].size,
        style.text_styles[&TextStyle::Heading].size,
    ));
}

fn gallery(ui: &mut Ui, g: &mut Gallery) {
    ui.label(RichText::new("Buttons").weak().small());
    ui.horizontal_wrapped(|ui| {
        for kind in ButtonKind::ALL {
            ui.add(kit::button(kind, kind.label()));
        }
        ui.add(kit::button(ButtonKind::Primary, "Prominent").with_class(widget_theme::PROMINENT));
        ui.add(egui::Button::new("Small").small());
        ui.add_enabled(false, egui::Button::new("Disabled"));
        ui.add(egui::Button::new("Selected").selected(true));
        ui.add(egui::Button::new("Frameless").frame(false));
    });

    ui.label(RichText::new("Toggles").weak().small());
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut g.check, "Checked");
        ui.checkbox(&mut g.check_off, "Unchecked");
        ui.radio_value(&mut g.radio, 0, "Radio A");
        ui.radio_value(&mut g.radio, 1, "Radio B");
    });

    ui.label(RichText::new("Fields").weak().small());
    ui.horizontal_wrapped(|ui| {
        ui.add(TextEdit::singleline(&mut g.text).desired_width(140.0));
        ui.add(
            TextEdit::singleline(&mut String::new())
                .hint_text("Hint text")
                .desired_width(120.0),
        );
        ComboBox::from_id_salt("theme_panel_gallery_combo")
            .selected_text(["Alpha", "Beta", "Gamma"][g.combo])
            .show_ui(ui, |ui| {
                for (i, name) in ["Alpha", "Beta", "Gamma"].into_iter().enumerate() {
                    ui.selectable_value(&mut g.combo, i, name);
                }
            });
        ui.add(DragValue::new(&mut g.drag).speed(0.1).suffix(" px"));
    });
    ui.add(
        TextEdit::multiline(&mut g.multiline)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    ui.add(Slider::new(&mut g.slider, 0.0..=100.0).text("Slider"));
    ui.add(ProgressBar::new(g.progress).show_percentage());
    ui.horizontal(|ui| {
        ui.add(Spinner::new());
        ui.label("Spinner");
        ui.hyperlink_to("Hyperlink", "https://example.com");
        ui.code("code");
    });

    ui.label(RichText::new("Text").weak().small());
    ui.horizontal_wrapped(|ui| {
        ui.heading("Heading");
        ui.label("Body");
        ui.label(RichText::new("Medium").medium());
        ui.label(RichText::new("Semibold").semibold());
        ui.label(RichText::new("Small").small());
        ui.label(RichText::new("Monospace 0123").monospace());
        ui.label(RichText::new("Weak").weak());
        ui.label(RichText::new("Strong").strong());
        ui.colored_label(colors::error(ui), "Error");
        ui.colored_label(colors::warn(ui), "Warn");
        ui.colored_label(colors::success(ui), "Success");
        ui.label("Emoji \u{1F5FA} \u{1F4DC} \u{1F5D1} \u{26A1} \u{2714} \u{2716}");
    });

    ui.label(RichText::new("Selection").weak().small());
    ui.horizontal_wrapped(|ui| {
        for (i, name) in ["Row one", "Row two", "Row three"].into_iter().enumerate() {
            if ui.selectable_label(g.selected == i, name).clicked() {
                g.selected = i;
            }
        }
    });

    CollapsingHeader::new("Collapsing header")
        .default_open(g.collapse_open)
        .show(ui, |ui| {
            ui.label("Inside the header.");
            ui.separator();
            ui.label("After a separator.");
        });

    ui.label(RichText::new("Table").weak().small());
    table(ui, g);

    ui.label(RichText::new("Overlays").weak().small());
    ui.horizontal_wrapped(|ui| {
        ui.label("Hover for a tooltip")
            .on_hover_text("Plain tooltip text.");
        ui.label("Rich tooltip").on_hover_ui(|ui| {
            ui.heading("Tooltip");
            ui.label("With several widgets.");
            ui.add(ProgressBar::new(0.4));
        });
        let popup_btn = ui.button("Popup");
        egui::Popup::from_toggle_button_response(&popup_btn).show(|ui| {
            ui.label("Popup contents");
            let _ = ui.button("Popup action");
        });
        ui.menu_button("Menu", |ui| {
            let _ = ui.button("Menu item");
            ui.checkbox(&mut g.check, "Menu check");
            ui.separator();
            ui.menu_button("Submenu", |ui| {
                let _ = ui.button("Nested item");
            });
        });
    });
}

fn table(ui: &mut Ui, g: &mut Gallery) {
    use egui_extras::{Column, TableBuilder};
    TableBuilder::new(ui)
        .striped(true)
        .id_salt("theme_panel_gallery_table")
        .column(Column::exact(120.0))
        .column(Column::exact(80.0))
        .column(Column::remainder())
        .sense(Sense::click())
        .max_scroll_height(130.0)
        .header(20.0, |mut header| {
            for title in ["Name", "Size", "Modified"] {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|mut body| {
            for (i, (name, size)) in [
                ("sunset.png", "2.4 MB"),
                ("clip.mp4", "48 MB"),
                ("notes.txt", "1 KB"),
                ("scan.tiff", "9.1 MB"),
            ]
            .into_iter()
            .enumerate()
            {
                body.row(22.0, |mut row| {
                    row.set_selected(g.selected == i);
                    row.col(|ui| {
                        ui.label(name);
                    });
                    row.col(|ui| {
                        ui.label(size);
                    });
                    row.col(|ui| {
                        ui.label(RichText::new("2026-10-18").weak());
                    });
                    if row.response().clicked() {
                        g.selected = i;
                    }
                });
            }
        });
}

fn json_section(ui: &mut Ui, ctx: &Context, state: &mut PanelState) {
    ui.horizontal_wrapped(|ui| {
        #[cfg(feature = "files")]
        {
            if ui.button("Export to file").clicked() {
                export_file(ctx);
            }
            if ui.button("Import from file").clicked() {
                import_file(ctx);
            }
        }
        if ui.button("Load current into box").clicked() {
            match theme_to_json(&SavedTheme::current(ctx)) {
                Ok(bytes) => {
                    state.json = String::from_utf8_lossy(&bytes).into_owned();
                    state.json_message = None;
                }
                Err(e) => state.json_message = Some((false, e.to_string())),
            }
        }
        if ui.button("Copy to clipboard").clicked()
            && let Ok(bytes) = theme_to_json(&SavedTheme::current(ctx))
        {
            ctx.copy_text(String::from_utf8_lossy(&bytes).into_owned());
        }
    });
    ScrollArea::vertical()
        .id_salt("egui_theme_kit.panel.json_scroll")
        .max_height(JSON_BOX_HEIGHT)
        .show(ui, |ui| {
            ui.add(
                TextEdit::multiline(&mut state.json)
                    .code_editor()
                    .hint_text("Paste theme JSON (wrapper or bare egui Style)")
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
        });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !state.json.trim().is_empty(),
                kit::button(ButtonKind::Primary, "Apply JSON"),
            )
            .clicked()
        {
            state.json_message = Some(match theme_from_json(state.json.as_bytes()) {
                Ok(theme) => {
                    apply_saved_theme(ctx, theme);
                    (true, "Applied".to_owned())
                }
                Err(e) => (false, format!("{e:#}")),
            });
        }
        if let Some((ok, message)) = &state.json_message {
            let color = if *ok {
                colors::success(ui)
            } else {
                colors::error(ui)
            };
            ui.colored_label(color, message);
        }
    });
}

#[cfg(feature = "files")]
fn export_file(ctx: &Context) {
    let theme = SavedTheme::current(ctx);
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export theme")
            .set_file_name("theme.json")
            .add_filter("Json", &["json"])
            .save_file()
        else {
            return;
        };
        let result = theme_to_json(&theme)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()))
            .map(|_| path);
        push_dialog(&ctx, DialogResult::Exported(result));
    });
}

#[cfg(feature = "files")]
fn import_file(ctx: &Context) {
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import theme")
            .add_filter("Json", &["json"])
            .pick_file()
        else {
            return;
        };
        let result = persist::load_file(&path)
            .map(Box::new)
            .map_err(|e| format!("{e:#}"));
        push_dialog(&ctx, DialogResult::Imported(result, path));
    });
}

#[cfg(feature = "files")]
fn push_dialog(ctx: &Context, result: DialogResult) {
    if let Ok(mut q) = DIALOG_RESULTS.lock() {
        q.push(result);
    }
    ctx.request_repaint();
}

/// A file-name-safe form of `name`; empty when nothing usable remains.
pub fn sanitized_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned()
}

/// Directory the deprecated [`save_user_theme`] writes to.
#[cfg(feature = "files")]
#[deprecated = "use store::JsonDirStore, which keeps user themes for any app"]
pub fn user_theme_dir() -> Option<PathBuf> {
    super::store::JsonDirStore::default_dir("egui-theme-kit")
}

/// Writes the live theme as `<name>.json` in [`user_theme_dir`]; returns the path.
#[cfg(feature = "files")]
#[deprecated = "use store::JsonDirStore, which keeps user themes for any app"]
pub fn save_user_theme(ctx: &Context, name: &str) -> anyhow::Result<PathBuf> {
    let dir = super::store::JsonDirStore::default_dir("egui-theme-kit")
        .ok_or_else(|| anyhow::anyhow!("no config directory on this host"))?;
    write_user_theme(&dir, ctx, name)
}

/// Writes the live theme as `<name>.json` in `dir`, creating it; returns the path.
#[cfg(feature = "files")]
fn write_user_theme(dir: &std::path::Path, ctx: &Context, name: &str) -> anyhow::Result<PathBuf> {
    let name = sanitized_name(name);
    anyhow::ensure!(
        !name.is_empty(),
        "enter a name made of letters, digits, - or _"
    );
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, theme_to_json(&SavedTheme::current(ctx))?)?;
    Ok(path)
}

#[cfg(feature = "toast")]
fn toast_section(ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        for (kind, label) in [
            (ToastKind::Info, "Info"),
            (ToastKind::Warning, "Warning"),
            (ToastKind::Error, "Error"),
            (ToastKind::Success, "Success"),
        ] {
            if ui.button(label).clicked()
                && let Ok(mut q) = TOAST_REQUESTS.lock()
            {
                q.push((kind, format!("{label} toast from the Theme panel")));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{CentralPanel, Pos2, RawInput, Rect};

    #[cfg(feature = "files")]
    use super::super::presets::PresetStyles;
    use super::super::{fonts::TextScale, presets::bootstrap_startup_theme, store::MemoryStore};

    fn frame(ctx: &Context) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            CentralPanel::default().show(ui, |_| {});
            show(ui.ctx());
        })
        .drop_without_applying_deltas();
    }

    fn themed_context() -> Context {
        let ctx = Context::default();
        ctx.set_fonts(fonts::definitions(TextScale::Normal));
        bootstrap_startup_theme(&ctx);
        ctx
    }

    #[test]
    fn the_panel_toggles_and_draws_every_section() {
        let ctx = themed_context();
        assert!(!is_open(&ctx));
        frame(&ctx);
        toggle(&ctx);
        assert!(is_open(&ctx));
        for _ in 0..3 {
            frame(&ctx);
        }
        toggle(&ctx);
        frame(&ctx);
        assert!(!is_open(&ctx));
    }

    #[test]
    fn the_panel_draws_under_every_preset_and_density() {
        let ctx = themed_context();
        set_open(&ctx, true);
        for def in presets::builtin_presets() {
            apply_preset(&ctx, def);
            for density in Density::ALL {
                presets::set_density(&ctx, density);
                frame(&ctx);
                frame(&ctx);
            }
        }
    }

    fn select_tab(ctx: &Context, tab: Tab) {
        let mut state = ctx
            .data_mut(|d| d.remove_temp::<PanelState>(state_id()))
            .unwrap_or_default();
        state.tab = tab;
        ctx.data_mut(|d| d.insert_temp(state_id(), state));
    }

    #[test]
    fn every_tab_draws_under_every_density() {
        let ctx = themed_context();
        set_open(&ctx, true);
        for tab in Tab::ALL {
            select_tab(&ctx, tab);
            for density in Density::ALL {
                presets::set_density(&ctx, density);
                frame(&ctx);
                frame(&ctx);
            }
        }
    }

    #[test]
    fn the_store_is_created_once_and_replaced_by_set_store() {
        let ctx = Context::default();
        let first = store(&ctx);
        assert!(Arc::ptr_eq(&first, &store(&ctx)));
        let custom: Arc<dyn ThemeStore> = Arc::new(MemoryStore::default());
        set_store(&ctx, Arc::clone(&custom));
        assert!(Arc::ptr_eq(&custom, &store(&ctx)));
        assert!(!Arc::ptr_eq(&first, &store(&ctx)));
    }

    #[cfg(feature = "picker")]
    #[test]
    fn the_themes_tab_hosts_the_picker_over_the_configured_store() {
        let ctx = themed_context();
        let backing = Arc::new(MemoryStore::default());
        backing.save_user_theme("Kept", &SavedTheme::current(&ctx));
        set_store(&ctx, backing);
        set_open(&ctx, true);
        select_tab(&ctx, Tab::Themes);
        frame(&ctx);
        frame(&ctx);
        let state = ctx
            .data(|d| d.get_temp::<PanelState>(state_id()))
            .expect("the panel keeps its state");
        let picker = state.picker.lock().unwrap();
        assert!(picker.origin().is_some());
        assert_eq!(picker.user_themes().len(), 1);
    }

    #[test]
    fn names_are_made_file_safe() {
        assert_eq!(sanitized_name("  My Theme! "), "My_Theme");
        assert_eq!(sanitized_name("a/b\\c"), "a_b_c");
        assert_eq!(sanitized_name("???"), "");
    }

    #[cfg(feature = "files")]
    #[test]
    fn a_user_theme_file_is_written_and_loads_back_as_the_same_theme() {
        let ctx = themed_context();
        apply_preset(&ctx, PresetStyles::NoirGlass);
        let dir = crate::store::test_dir("panel_user_theme");
        let path = write_user_theme(dir.path(), &ctx, " My Noir! ").unwrap();
        assert_eq!(path.file_name().unwrap(), "My_Noir.json");
        assert!(write_user_theme(dir.path(), &ctx, "???").is_err());

        let fresh = themed_context();
        apply_saved_theme(&fresh, persist::load_file(&path).unwrap());
        assert_eq!(presets::installed_preset(&fresh), PresetStyles::NoirGlass);
    }

    #[test]
    fn frame_time_smooths_toward_new_samples() {
        let ctx = Context::default();
        record_ui_time(&ctx, Duration::from_millis(10));
        let first = ctx
            .data(|d| d.get_temp::<FrameStats>(stats_id()))
            .unwrap()
            .ui_ms;
        assert!((first - 10.0).abs() < 0.01);
        record_ui_time(&ctx, Duration::from_millis(20));
        let second = ctx
            .data(|d| d.get_temp::<FrameStats>(stats_id()))
            .unwrap()
            .ui_ms;
        assert!(second > first && second < 20.0);
    }

    #[test]
    fn frame_cpu_time_is_recorded_and_a_missing_sample_is_ignored() {
        let ctx = Context::default();
        record_frame_cpu(&ctx, None);
        assert!(ctx.data(|d| d.get_temp::<FrameStats>(stats_id())).is_none());
        record_frame_cpu(&ctx, Some(0.004));
        let stats = ctx.data(|d| d.get_temp::<FrameStats>(stats_id())).unwrap();
        assert!((stats.frame_ms - 4.0).abs() < 0.01);
    }

    #[cfg(feature = "toast")]
    #[test]
    fn toast_requests_are_drained_once() {
        TOAST_REQUESTS
            .lock()
            .unwrap()
            .push((ToastKind::Info, "x".to_owned()));
        assert!(!take_toast_requests().is_empty());
        assert!(take_toast_requests().is_empty());
    }
}
