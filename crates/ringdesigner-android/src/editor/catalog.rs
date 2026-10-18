//! Phone template library: one scrollable page instead of cascading menus.
use egui_mobile::egui;
use ringdesign_workbench::templates::{self, Template};

pub enum Choice {
    Blank,
    Template(&'static Template),
}

pub fn show(ui: &mut egui::Ui) -> Option<Choice> {
    let id = egui::Id::unique("phone-template-search");
    let mut query = ui.data(|d| d.get_temp::<String>(id).unwrap_or_default());
    let mut chosen = None;
    ui.horizontal(|ui| {
        let clear_width = ui
            .painter()
            .layout_no_wrap(
                "Clear".into(),
                egui::TextStyle::Button.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x
            + 2.0 * ui.spacing().button_padding.x
            + ui.spacing().item_spacing.x;
        let search = ui.add_sized(
            [
                (ui.available_width() - clear_width).max(1.0),
                ui.spacing().interact_size.y,
            ],
            egui::TextEdit::singleline(&mut query).hint_text("Search designs or collections"),
        );
        super::layout::record(ui, "templates/search", search.rect);
        let clear = ui.add_enabled(!query.is_empty(), egui::Button::new("Clear"));
        super::layout::record(ui, "templates/Clear", clear.rect);
        if clear.clicked() {
            query.clear();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, query.clone()));
    let query = query.trim().to_lowercase();
    crate::theme::scroll_vertical()
        .id_salt("phone-templates")
        .show(ui, |ui| {
            let blank = ui.add_sized(
                [ui.available_width(), crate::theme::MENU_ROW_H],
                egui::Button::new("Blank band"),
            );
            super::layout::record(ui, "templates/Blank band", blank.rect);
            if blank.clicked() {
                chosen = Some(Choice::Blank);
            }
            let mut found = false;
            for (index, collection) in templates::collections().iter().enumerate() {
                let matches: Vec<_> = collection
                    .templates
                    .iter()
                    .filter(|t| {
                        query.is_empty()
                            || [
                                collection.name,
                                t.name,
                                t.description,
                                t.family.unwrap_or(""),
                            ]
                            .iter()
                            .any(|text| text.to_lowercase().contains(&query))
                    })
                    .collect();
                if matches.is_empty() {
                    continue;
                }
                found = true;
                let header =
                    egui::CollapsingHeader::new(format!("{} ({})", collection.name, matches.len()))
                        .id_salt(collection.name)
                        .default_open(index == 0)
                        .open((!query.is_empty()).then_some(true))
                        .show_unindented(ui, |ui| {
                            for template in matches {
                                let thumb = templates::thumbnail(ui, template);
                                let button = egui::Button::new((
                                    egui::Image::new((thumb.id(), egui::vec2(40.0, 40.0))),
                                    template.name,
                                ))
                                .image_tint_follows_text_color(false)
                                .wrap_mode(egui::TextWrapMode::Wrap);
                                let response = ui.add_sized([ui.available_width(), 48.0], button);
                                super::layout::record(
                                    ui,
                                    format!("templates/{}", template.name),
                                    response.rect,
                                );
                                if response.clicked() {
                                    chosen = Some(Choice::Template(template));
                                }
                                response.on_hover_text(template.description);
                                if let Some(badge) = template.badge {
                                    ui.small(badge);
                                }
                            }
                        });
                super::layout::record(
                    ui,
                    format!("templates/{}", collection.name),
                    header.header_response.rect,
                );
            }
            if !found {
                ui.label("No matching designs. Try a collection or shape name.");
            }
        });
    chosen
}

#[cfg(test)]
mod tests;
