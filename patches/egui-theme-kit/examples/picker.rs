//! The theme picker over a directory store: `cargo run --example picker [DIR]`.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, CentralPanel, Panel, RichText};
use egui_theme_kit::{
    debug_panel, fonts, glass, persist,
    picker::{self, PickerState},
    store::JsonDirStore,
};

fn main() -> eframe::Result {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("egui_theme_kit_picker_example"));
    let store = Arc::new(JsonDirStore::open(&dir).expect("open the theme directory"));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1240.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Theme picker",
        options,
        Box::new(move |cc| Ok(Box::new(Demo::new(cc, store)))),
    )
}

struct Demo {
    ctx: egui::Context,
    store: Arc<JsonDirStore>,
    picker: PickerState,
    sample: Sample,
}

#[derive(Default)]
struct Sample {
    text: String,
    check: bool,
    choice: usize,
    value: f32,
}

impl Demo {
    fn new(cc: &eframe::CreationContext<'_>, store: Arc<JsonDirStore>) -> Self {
        glass::install(cc);
        let ctx = cc.egui_ctx.clone();
        ctx.set_theme(egui::Theme::Dark);
        fonts::install_for_context(&ctx);
        persist::apply_startup_from_store(&ctx, &*store, None);
        debug_panel::set_store(&ctx, store.clone());
        let mut picker = PickerState::default();
        picker.options.debounce = std::time::Duration::from_millis(300);
        Self {
            ctx,
            store,
            picker,
            sample: Sample {
                text: "Sample text".to_owned(),
                check: true,
                choice: 1,
                value: 0.4,
            },
        }
    }
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        Panel::left("picker")
            .default_size(560.0)
            .min_size(380.0)
            .show(ui, |ui| picker::show(ui, &mut self.picker, &*self.store));
        CentralPanel::default().show(ui, |ui| sample(ui, &mut self.sample));
        debug_panel::show(ui.ctx());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.picker.flush(&self.ctx, &*self.store);
        self.store.flush();
    }
}

fn sample(ui: &mut egui::Ui, s: &mut Sample) {
    ui.heading("Live sample");
    ui.label(
        RichText::new("Everything here follows the picker. Ctrl+Alt+T opens the debug panel.")
            .weak(),
    );
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        for kind in egui_theme_kit::kit::ButtonKind::ALL {
            let _ = ui.add(egui_theme_kit::kit::button(kind, kind.label()));
        }
        ui.add_enabled(false, egui::Button::new("Disabled"));
    });
    ui.add(egui::TextEdit::singleline(&mut s.text).hint_text("Text field"));
    ui.checkbox(&mut s.check, "A checkbox");
    ui.horizontal(|ui| {
        for (i, name) in ["One", "Two", "Three"].into_iter().enumerate() {
            ui.radio_value(&mut s.choice, i, name);
        }
    });
    ui.add(egui::Slider::new(&mut s.value, 0.0..=1.0).text("Slider"));
    ui.add(egui::ProgressBar::new(s.value).show_percentage());
    egui::ComboBox::from_label("Combo")
        .selected_text(["One", "Two", "Three"][s.choice])
        .show_ui(ui, |ui| {
            for (i, name) in ["One", "Two", "Three"].into_iter().enumerate() {
                ui.selectable_value(&mut s.choice, i, name);
            }
        });
    ui.collapsing("Collapsing header", |ui| {
        ui.label("Inside the header.");
        ui.hyperlink_to("A link", "https://example.com");
        ui.code("monospace");
    });
    ui.horizontal(|ui| {
        ui.colored_label(egui_theme_kit::colors::success(ui), "Success");
        ui.colored_label(egui_theme_kit::colors::warn(ui), "Warning");
        ui.colored_label(egui_theme_kit::colors::error(ui), "Error");
    });
}
