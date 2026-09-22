//! Inspector rows share a left label and right value column.
pub fn row<R>(ui: &mut egui::Ui, label: &str, value: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.spacing_mut().interact_size.x = 84.;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), value)
            .inner
    })
    .inner
}

/// A row whose value widget answers to `name` in the accessibility tree.
pub fn named(
    ui: &mut egui::Ui,
    label: &str,
    name: &str,
    value: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    let response = row(ui, label, value);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, name));
    response
}

pub fn slider(ui: &mut egui::Ui, label: &str, slider: egui::Slider<'_>) -> egui::Response {
    slider_row(ui, label, slider, true)
}

pub fn slider_track(ui: &mut egui::Ui, label: &str, slider: egui::Slider<'_>) -> egui::Response {
    slider_row(ui, label, slider, false)
}

fn slider_row(
    ui: &mut egui::Ui,
    label: &str,
    slider: egui::Slider<'_>,
    value: bool,
) -> egui::Response {
    row(ui, label, |ui| {
        let width = ui.available_width().min(if value { 268. } else { 180. });
        ui.allocate_ui_with_layout(
            egui::vec2(width, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().slider_width = (width
                    - if value {
                        84. + ui.spacing().item_spacing.x
                    } else {
                        0.
                    })
                .max(32.);
                ui.add(slider.show_value(value))
            },
        )
        .inner
    })
}
