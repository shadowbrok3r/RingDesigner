//! Minimal eframe app on the kit: init, stock widgets, a dock demo behind the `dock` feature, and Ctrl+Alt+T for the Theme panel.

use eframe::egui::{self, CentralPanel, ComboBox, Context, Frame, ProgressBar, Slider, Ui, Window};
use egui_theme_kit::{ButtonKind, FontConfig, KitConfig, Tokens, cli, debug_panel, glass, kit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Widgets,
    Palette,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Widgets => "Widgets",
            Self::Palette => "Palette",
        }
    }
}

struct Panes {
    name: String,
    notes: String,
    checked: bool,
    mode: usize,
    volume: f32,
    progress: f32,
    count: f64,
    #[cfg(feature = "toast")]
    toasts: egui_toast::Toasts,
}

struct App {
    ctx: Context,
    panes: Panes,
    show_window: bool,
    #[cfg(not(feature = "dock"))]
    tab: Tab,
    #[cfg(feature = "dock")]
    dock: egui_dock::DockState<Tab>,
}

fn font_config() -> FontConfig {
    FontConfig::new().installer(|fonts| {
        egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular);
    })
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_theme_kit::init(cc, KitConfig::default().fonts(font_config()));
        #[cfg(feature = "dock")]
        let dock = {
            let mut dock = egui_dock::DockState::new(vec![Tab::Widgets]);
            dock.main_surface_mut().split_right(
                egui_dock::NodeIndex::root(),
                0.55,
                vec![Tab::Palette],
            );
            dock
        };
        Self {
            ctx: cc.egui_ctx.clone(),
            panes: Panes {
                name: "Sample text".to_owned(),
                notes: "Multi-line\ntext edit".to_owned(),
                checked: true,
                mode: 1,
                volume: 40.0,
                progress: 0.62,
                count: 12.5,
                #[cfg(feature = "toast")]
                toasts: egui_theme_kit::toast::style(
                    egui_toast::Toasts::new()
                        .anchor(egui::Align2::RIGHT_BOTTOM, (-12.0, -12.0))
                        .direction(egui::Direction::BottomUp),
                ),
            },
            show_window: true,
            #[cfg(not(feature = "dock"))]
            tab: Tab::Widgets,
            #[cfg(feature = "dock")]
            dock,
        }
    }
}

impl Panes {
    fn show(&mut self, ui: &mut Ui, tab: Tab) {
        match tab {
            Tab::Widgets => self.widgets(ui),
            Tab::Palette => palette(ui),
        }
    }

    fn widgets(&mut self, ui: &mut Ui) {
        ui.heading("Stock widgets");
        ui.horizontal_wrapped(|ui| {
            for kind in ButtonKind::ALL {
                ui.add(kit::button(kind, kind.label()));
            }
            ui.add(kit::primary_button(format!(
                "{} Save",
                egui_phosphor::regular::FLOPPY_DISK
            )));
            ui.add_enabled(false, kit::button(ButtonKind::Neutral, "Disabled"));
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.name);
            ComboBox::from_id_salt("mode")
                .selected_text(["Low", "Medium", "High"][self.mode])
                .show_ui(ui, |ui| {
                    for (i, label) in ["Low", "Medium", "High"].into_iter().enumerate() {
                        ui.selectable_value(&mut self.mode, i, label);
                    }
                });
            ui.add(egui::DragValue::new(&mut self.count).speed(0.1));
        });
        ui.checkbox(&mut self.checked, "Checkbox");
        ui.add(Slider::new(&mut self.volume, 0.0..=100.0).text("Volume"));
        ui.add(ProgressBar::new(self.progress).show_percentage());
        ui.text_edit_multiline(&mut self.notes);
        #[cfg(feature = "toast")]
        ui.horizontal(|ui| {
            use egui_toast::{Toast, ToastKind};
            for (kind, label) in [
                (ToastKind::Info, "Info"),
                (ToastKind::Success, "Success"),
                (ToastKind::Warning, "Warning"),
                (ToastKind::Error, "Error"),
            ] {
                if ui.button(format!("{label} toast")).clicked() {
                    self.toasts.add(Toast {
                        kind,
                        text: format!("{label} message").into(),
                        ..Default::default()
                    });
                }
            }
        });
    }
}

fn palette(ui: &mut Ui) {
    ui.heading("Token palette");
    let tokens = Tokens::get(ui.ctx());
    egui::Grid::new("palette").num_columns(2).show(ui, |ui| {
        for (name, color) in tokens.colors() {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, tokens.radius_sm, color);
            ui.label(name);
            ui.end_row();
        }
    });
}

#[cfg(feature = "dock")]
impl egui_dock::TabViewer for Panes {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::unique(("tab", tab.title()))
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Tab) {
        self.show(ui, *tab);
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        glass::poll_outcome(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        let ctx = ui.ctx().clone();
        glass::frost_open_windows(&ctx);

        CentralPanel::default()
            .frame(Frame::central_panel(&ctx.global_style()).inner_margin(0.0))
            .show(ui, |ui| {
                #[cfg(feature = "dock")]
                egui_dock::DockArea::new(&mut self.dock)
                    .style(egui_theme_kit::dock_style(&ctx))
                    .show_inside(ui, &mut self.panes);
                #[cfg(not(feature = "dock"))]
                {
                    ui.horizontal(|ui| {
                        for tab in [Tab::Widgets, Tab::Palette] {
                            ui.selectable_value(&mut self.tab, tab, tab.title());
                        }
                    });
                    ui.separator();
                    self.panes.show(ui, self.tab);
                }
            });

        Window::new("Floating window")
            .open(&mut self.show_window)
            .default_pos([40.0, 380.0])
            .show(&ctx, |ui| {
                ui.label("Windows are frosted when the preset has a glass material.");
                ui.label("Press Ctrl+Alt+T for the Theme panel.");
            });

        #[cfg(feature = "toast")]
        {
            for (kind, text) in debug_panel::take_toast_requests() {
                self.panes.toasts.add(egui_toast::Toast {
                    kind,
                    text: text.into(),
                    ..Default::default()
                });
            }
            self.panes.toasts.show(ui);
        }

        debug_panel::show(&ctx);
        debug_panel::record_ui_time(&ctx, started.elapsed());
        debug_panel::record_frame_cpu(&ctx, frame.info().cpu_usage);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        egui_theme_kit::save(&self.ctx, storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        #[cfg(feature = "glass")]
        if let Some(gl) = gl {
            glass::shutdown(gl);
        }
        #[cfg(not(feature = "glass"))]
        let _ = gl;
    }
}

fn main() -> eframe::Result {
    let options = cli::Options::from_env().unwrap_or_default();
    match cli::handle_dump(&options, &font_config()) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => eprintln!("theme dump failed: {e:#}"),
    }
    cli::install(options);

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui-theme-kit basic",
        native,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
