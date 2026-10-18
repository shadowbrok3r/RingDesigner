//! Gallery of every painted control with preset, density, text-scale and window-chrome switches.
//!
//! Flags: `--preset <name>`, `--density compact|regular|comfortable`, `--tab gallery|logger`,
//! `--text-scale small|normal|large|xlarge`, `--frameless`, `--backdrop`, `--scroll <points>`.

use eframe::egui::{self, Align, Layout, ScrollArea};
use egui_theme_kit::controls::StaticImage;
use egui_theme_kit::{
    controls::{self, Icon, IconBox, TextButton, icons},
    fonts::{self, TextScale},
    logger,
    metrics::Density,
    presets::{MATCHABLE_PRESETS, PresetStyles, apply_preset},
    title_bar::{self, Side, TitleBar},
};

/// Phosphor glyphs shown beside the built-in icons.
const GLYPHS: [(&str, &str); 6] = [
    ("house", egui_phosphor::regular::HOUSE),
    ("gear", egui_phosphor::regular::GEAR),
    ("folder", egui_phosphor::regular::FOLDER),
    ("magnifying-glass", egui_phosphor::regular::MAGNIFYING_GLASS),
    ("bell", egui_phosphor::regular::BELL),
    ("trash", egui_phosphor::regular::TRASH),
];

/// White ring SVG, tinted when drawn.
const RING_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="none" stroke="#fff" stroke-width="1.5"/></svg>"##;
/// White disc SVG used as the back layer of a duotone icon.
const DISC_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="#fff"/></svg>"##;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Gallery,
    Logger,
}

#[derive(Default)]
struct Args {
    preset: Option<String>,
    density: Option<Density>,
    scale: Option<TextScale>,
    tab: Option<Tab>,
    frameless: bool,
    backdrop: bool,
    scroll: Option<f32>,
}

fn parse_args() -> Args {
    let mut out = Args::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--preset" => out.preset = args.next(),
            "--density" => {
                out.density = args.next().and_then(|v| match v.as_str() {
                    "compact" => Some(Density::Compact),
                    "regular" => Some(Density::Regular),
                    "comfortable" => Some(Density::Comfortable),
                    _ => None,
                });
            }
            "--text-scale" => {
                out.scale = args.next().and_then(|v| match v.as_str() {
                    "small" => Some(TextScale::Small),
                    "normal" => Some(TextScale::Normal),
                    "large" => Some(TextScale::Large),
                    "xlarge" => Some(TextScale::XLarge),
                    _ => None,
                });
            }
            "--tab" => {
                out.tab = args.next().and_then(|v| match v.as_str() {
                    "gallery" => Some(Tab::Gallery),
                    "logger" => Some(Tab::Logger),
                    _ => None,
                });
            }
            "--frameless" => out.frameless = true,
            "--backdrop" => out.backdrop = true,
            "--scroll" => out.scroll = args.next().and_then(|v| v.parse().ok()),
            _ => {}
        }
    }
    out
}

struct App {
    preset: PresetStyles,
    density: Density,
    scale: TextScale,
    tab: Tab,
    frameless: bool,
    backdrop: bool,
    scroll: Option<f32>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let preset = args
            .preset
            .as_deref()
            .and_then(|name| {
                let name = name.to_lowercase();
                MATCHABLE_PRESETS
                    .into_iter()
                    .find(|p| p.as_str().to_lowercase().contains(&name))
            })
            .unwrap_or(MATCHABLE_PRESETS[0]);
        let density = args.density.unwrap_or_default();
        let scale = args.scale.unwrap_or_default();
        let app = Self {
            preset,
            density,
            scale,
            tab: args.tab.unwrap_or(Tab::Gallery),
            frameless: args.frameless,
            backdrop: args.backdrop,
            scroll: args.scroll,
        };
        app.apply_all(&cc.egui_ctx);
        app
    }

    fn apply_all(&self, ctx: &egui::Context) {
        apply_fonts(ctx, self.scale);
        egui_theme_kit::set_density(ctx, self.density);
        apply_preset(ctx, self.preset);
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        controls::section_label(ui, "Preset");
        let mut preset = self.preset;
        let labels: Vec<String> = MATCHABLE_PRESETS
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        let options: Vec<(PresetStyles, &str)> = MATCHABLE_PRESETS
            .iter()
            .copied()
            .zip(labels.iter().map(String::as_str))
            .collect();
        if controls::dropdown(ui, "preset", &mut preset, &options, 190.0).changed() {
            self.preset = preset;
            apply_preset(&ctx, preset);
        }
        ui.add_space(8.0);
        controls::section_label(ui, "Density");
        let mut density = self.density;
        let segments = Density::ALL.map(|d| controls::Segment::new(d, d.label()));
        if controls::segmented(ui, &mut density, &segments).changed() {
            self.density = density;
            egui_theme_kit::set_density(&ctx, density);
        }
        ui.add_space(8.0);
        controls::section_label(ui, "Text size");
        let mut scale = self.scale;
        let segments = [TextScale::Small, TextScale::Normal, TextScale::Large]
            .map(|s| controls::Segment::new(s, s.label()));
        if controls::segmented(ui, &mut scale, &segments).changed() {
            self.scale = scale;
            apply_fonts(&ctx, scale);
        }
        ui.add_space(8.0);
        controls::hairline(ui);
        ui.add_space(8.0);
        controls::section_label(ui, "Window");
        let mut frameless = self.frameless;
        if controls::toggle(ui, &mut frameless, "Custom title bar").changed() {
            self.frameless = frameless;
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!frameless));
        }
        controls::toggle(ui, &mut self.backdrop, "Colour backdrop");
        ui.add_space(8.0);
        controls::hairline(ui);
        ui.add_space(8.0);
        controls::section_label(ui, "Log lines");
        ui.horizontal_wrapped(|ui| {
            let t = controls::Tokens::get(&ctx);
            if controls::icon_text_button(ui, icons::INFO, "Info").clicked() {
                log::info!(target: "example", "info line");
            }
            if TextButton::new("Warn")
                .icon(icons::WARNING)
                .ink(t.warn)
                .show(ui)
                .clicked()
            {
                log::warn!(target: "example", "warning line");
            }
            if TextButton::new("Error")
                .icon(icons::ERROR)
                .ink(t.error)
                .show(ui)
                .clicked()
            {
                log::error!(target: "example", "error line");
            }
        });
    }
}

fn apply_fonts(ctx: &egui::Context, scale: TextScale) {
    fonts::set_text_scale(ctx, scale, |scale| {
        let mut definitions = fonts::definitions(scale);
        egui_phosphor::add_to_fonts(&mut definitions, egui_phosphor::Variant::Regular);
        definitions
    });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.frameless {
            let mut tab = self.tab;
            let _ = TitleBar::new("example.title_bar", "egui-theme-kit controls")
                .mark(icons::GRID)
                .frameless(true)
                .show(ui, |ui, side| match side {
                    Side::Left => {
                        for (candidate, label) in
                            [(Tab::Gallery, "Gallery"), (Tab::Logger, "Logger")]
                        {
                            if controls::pill(ui, None, label, tab == candidate).clicked() {
                                tab = candidate;
                            }
                        }
                    }
                    Side::Right => {
                        let _ =
                            controls::icon_button(ui, icons::GEAR, IconBox::Md, false, "Settings");
                    }
                });
            self.tab = tab;
            title_bar::window_edges(ui);
        }
        egui::Panel::left("example.side")
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(8.0);
                if !self.frameless {
                    let mut tab = self.tab;
                    ui.horizontal(|ui| {
                        for (candidate, label) in
                            [(Tab::Gallery, "Gallery"), (Tab::Logger, "Logger")]
                        {
                            if controls::pill(ui, None, label, tab == candidate).clicked() {
                                tab = candidate;
                            }
                        }
                    });
                    self.tab = tab;
                    ui.add_space(8.0);
                }
                self.side_panel(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if self.backdrop {
                paint_backdrop(ui);
            }
            match self.tab {
                Tab::Gallery => {
                    let mut area = ScrollArea::vertical().auto_shrink([false, false]);
                    if let Some(offset) = self.scroll.take() {
                        area = area.vertical_scroll_offset(offset);
                    }
                    area.show(ui, |ui| {
                        let mut extra: Vec<(&str, Icon)> = GLYPHS
                            .iter()
                            .map(|(name, glyph)| (*name, Icon::glyph(glyph)))
                            .collect();
                        let ring = StaticImage::new("bytes://example-ring.svg", RING_SVG);
                        let disc = StaticImage::new("bytes://example-disc.svg", DISC_SVG);
                        extra.push(("svg ring", Icon::Image(ring)));
                        extra.push(("svg duotone", Icon::duotone(ring, disc)));
                        controls::gallery::show_with(ui, &extra);
                    });
                }
                Tab::Logger => {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        logger::logger_ui().show(ui);
                    });
                }
            }
        });
    }
}

/// Large coloured discs behind the content, for glass presets to frost.
fn paint_backdrop(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let discs = [
        (0.15, 0.1, 140.0, egui::Color32::from_rgb(220, 70, 120)),
        (0.7, 0.2, 180.0, egui::Color32::from_rgb(60, 140, 230)),
        (0.35, 0.55, 150.0, egui::Color32::from_rgb(70, 200, 140)),
        (0.85, 0.7, 170.0, egui::Color32::from_rgb(240, 170, 60)),
        (0.2, 0.9, 160.0, egui::Color32::from_rgb(150, 90, 230)),
    ];
    for (x, y, radius, color) in discs {
        let center = rect.min + egui::vec2(x * rect.width(), y * rect.height());
        painter.circle_filled(center, radius, color);
    }
}

fn main() -> eframe::Result {
    let _ = logger::builder().init();
    let args = parse_args();
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 900.0])
        .with_decorations(!args.frameless);
    eframe::run_native(
        "egui-theme-kit controls",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            egui_theme_kit::init(cc, egui_theme_kit::KitConfig::default());
            egui_extras::install_image_loaders(&cc.egui_ctx);
            log::info!(target: "example", "controls example started");
            Ok(Box::new(App::new(cc, args)))
        }),
    )
}
