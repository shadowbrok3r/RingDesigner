//! The alpha tile library: browse, preview, import, and regenerate patterns.

use egui_phosphor::regular as icon;
use ringdesign_core::alpha::Procedural;
use ringdesign_core::field::Layer;
use ringdesign_core::tiling::TilingLayer;

use crate::app::RingDesignerApp;
use crate::theme;

const THUMB: f32 = 62.0;
const LABEL_H: f32 = 14.0;
const SIZES: [usize; 3] = [128, 256, 512];

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    editor_window(app, ui);
    text_window(app, ui);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} Alphas", icon::IMAGES)).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{} tiles", app.lib.len())).color(theme::TEXT_DIM));
        });
    });
    ui.add_space(2.0);

    search_row(app, ui);
    source_row(app, ui);

    ui.add_space(2.0);
    let target = selected_tiling(app);
    ui.label(
        egui::RichText::new(match &target {
            Some((_, layer_name)) => format!("Click a tile to swap it into \"{layer_name}\""),
            None => "Click a tile to start a new tiling layer".to_string(),
        })
        .small()
        .color(theme::TEXT_DIM),
    );
    ui.add_space(3.0);

    let names = filtered_names(app);
    if names.is_empty() {
        ui.label(
            egui::RichText::new(if app.lib.is_empty() {
                "Library is empty. Import images or regenerate the built-ins."
            } else {
                "No tile matches the filter."
            })
            .color(theme::TEXT_DIM),
        );
        return;
    }

    let current = current_alpha(app);
    let action = grid(app, ui, &names, &current);

    if let Some(name) = action.clicked {
        apply_click(app, target.map(|(i, _)| i), name);
    }
    if let Some(name) = action.removed {
        app.library_mut().remove(&name);
        app.forget_thumbnail(&name);
        app.mark_dirty();
        app.set_status(format!("Removed tile {name}"));
    }
    if let Some(name) = action.edited {
        app.alpha_editor.open(&name);
    }
}

/// Inscriptions: text entries carried by the design, rasterized to tiles.
fn text_window(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    use ringdesign_core::text::{TextAlpha, TextFont};
    if !app.text_editor_open {
        return;
    }
    let ctx = ui.ctx().clone();
    let mut open = app.text_editor_open;
    let mut rebake: Option<usize> = None;
    let mut remove: Option<usize> = None;
    egui::Window::new(format!("{} Inscriptions", icon::TEXT_AA))
        .open(&mut open)
        .default_width(320.0)
        .show(&ctx, |ui| {
            for (i, t) in app.design.texts.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        if ui
                            .add(egui::TextEdit::singleline(&mut t.name).desired_width(110.0))
                            .changed()
                        {
                            rebake = Some(i);
                        }
                        if ui.small_button(icon::TRASH).on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Text");
                        if ui
                            .add(egui::TextEdit::singleline(&mut t.text).desired_width(170.0))
                            .changed()
                        {
                            rebake = Some(i);
                        }
                    });
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(("text_font", i))
                            .selected_text(t.font.label())
                            .width(170.0)
                            .show_ui(ui, |ui| {
                                for &f in TextFont::ALL {
                                    if ui.selectable_value(&mut t.font, f, f.label()).clicked() {
                                        rebake = Some(i);
                                    }
                                }
                            });
                        if ui
                            .add(
                                egui::Slider::new(&mut t.tracking, -0.1..=0.6)
                                    .fixed_decimals(2)
                                    .text("Track"),
                            )
                            .changed()
                        {
                            rebake = Some(i);
                        }
                    });
                    ui.separator();
                });
            }
            if ui.button(format!("{} Add inscription", icon::PLUS)).clicked() {
                let n = app.design.texts.len() + 1;
                app.design.texts.push(TextAlpha {
                    name: format!("Text {n}"),
                    ..Default::default()
                });
                rebake = Some(app.design.texts.len() - 1);
            }
            ui.label(
                egui::RichText::new(
                    "The rendered tile lands in the library under its name — use it in a \
                     Tiling, Decal or mask like any other. Fonts are SIL OFL.",
                )
                .small()
                .color(theme::TEXT_DIM),
            );
        });
    app.text_editor_open = open;

    if let Some(i) = remove {
        let name = app.design.texts[i].name.clone();
        app.design.texts.remove(i);
        app.library_mut().remove(&name);
        app.forget_thumbnail(&name);
        app.mark_dirty();
    } else if let Some(i) = rebake
        && let Some(t) = app.design.texts.get(i).cloned()
        && !t.is_empty()
    {
        let name = t.name.clone();
        app.library_mut().insert(t.rasterize());
        app.forget_thumbnail(&name);
        app.mark_dirty();
    }
}

/// Draws the clip-and-tile window and files whatever it saves.
fn editor_window(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let Some(alpha) = app.alpha_editor.ui(&ctx, &app.lib) else {
        return;
    };
    let name = alpha.name.clone();
    app.library_mut().insert(alpha);
    app.forget_thumbnail(&name);
    app.mark_dirty();
    app.set_status(format!("Saved tile {name}"));
}

fn search_row(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).color(theme::TEXT_DIM));
        let clearable = !app.library_filter.is_empty();
        let reserve = if clearable { 28.0 } else { 0.0 };
        let w = (ui.available_width() - reserve).max(60.0);
        ui.add_sized(
            [w, 20.0],
            egui::TextEdit::singleline(&mut app.library_filter).hint_text("Filter tiles"),
        );
        if clearable && ui.small_button(icon::X).clicked() {
            app.library_filter.clear();
        }
    });
}

fn source_row(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui
            .button(format!("{} Import images…", icon::FILE_IMAGE))
            .on_hover_text("PNG/JPG/BMP. Brightness reads as height: black is flat, white is the layer's full relief.")
            .clicked()
        {
            crate::export::import_alphas(app);
            app.mark_dirty();
        }

        let selected = current_alpha(app);
        let known = app.lib.get(&selected).is_some();
        if ui
            .add_enabled(
                known,
                egui::Button::new(format!("{} Clip & tile…", icon::SCISSORS))
                    .selected(app.alpha_editor.is_open()),
            )
            .on_hover_text("Crop a fragment out of the selected tile and mirror it into something that repeats")
            .on_disabled_hover_text("Click a tile first, or right-click one")
            .clicked()
        {
            app.alpha_editor.open(&selected);
        }

        if ui
            .button(format!("{} Text…", icon::TEXT_AA))
            .on_hover_text(
                "Render a name, date or monogram to a tile. The text travels in the design \
                 and re-renders on load.",
            )
            .clicked()
        {
            app.text_editor_open = !app.text_editor_open;
        }
    });

    ui.horizontal(|ui| {
        let size_id = egui::Id::new("library_builtin_size");
        let mut size: usize = ui.memory(|m| m.data.get_temp(size_id)).unwrap_or(256);

        if ui
            .button(format!("{} Regenerate built-ins", icon::ARROWS_CLOCKWISE))
            .on_hover_text("Re-render every procedural pattern at the chosen resolution")
            .clicked()
        {
            for p in Procedural::ALL {
                app.forget_thumbnail(p.label());
                app.library_mut().insert(p.generate(size));
            }
            app.mark_dirty();
            app.set_status(format!(
                "Regenerated {} built-in tiles at {size} px",
                Procedural::ALL.len()
            ));
        }

        egui::ComboBox::from_id_salt(size_id)
            .selected_text(format!("{size} px"))
            .width(74.0)
            .show_ui(ui, |ui| {
                for s in SIZES {
                    ui.selectable_value(&mut size, s, format!("{s} px"));
                }
            });
        ui.memory_mut(|m| m.data.insert_temp(size_id, size));
    });
}

/// Index and name of the selected layer when it is a tiling layer.
fn selected_tiling(app: &RingDesignerApp) -> Option<(usize, String)> {
    let i = app.selected_layer?;
    let e = app.design.layers.layers.get(i)?;
    match e.layer {
        Layer::Tiling(_) => Some((i, e.name.clone())),
        _ => None,
    }
}

/// Alpha name used by the selected tiling layer.
fn current_alpha(app: &RingDesignerApp) -> String {
    app.selected_layer
        .and_then(|i| app.design.layers.layers.get(i))
        .and_then(|e| match &e.layer {
            Layer::Tiling(t) => Some(t.alpha.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn filtered_names(app: &RingDesignerApp) -> Vec<String> {
    let needle = app.library_filter.trim().to_lowercase();
    app.lib
        .names()
        .into_iter()
        .filter(|n| needle.is_empty() || n.to_lowercase().contains(&needle))
        .collect()
}

/// What the thumbnail grid asked for this frame.
#[derive(Default)]
struct GridAction {
    clicked: Option<String>,
    removed: Option<String>,
    edited: Option<String>,
}

fn grid(
    app: &mut RingDesignerApp,
    ui: &mut egui::Ui,
    names: &[String],
    current: &str,
) -> GridAction {
    let spacing = ui.spacing().item_spacing.x;
    let cols = (((ui.available_width() + spacing) / (THUMB + spacing)).floor() as usize).clamp(1, 8);

    let mut action = GridAction::default();
    for row in names.chunks(cols) {
        ui.horizontal(|ui| {
            for name in row {
                let hit = tile(app, ui, name, name == current);
                if hit.clicked {
                    action.clicked = Some(name.clone());
                }
                if hit.remove {
                    action.removed = Some(name.clone());
                }
                if hit.edit {
                    action.edited = Some(name.clone());
                }
            }
        });
    }
    action
}

/// What one thumbnail asked for this frame.
#[derive(Default)]
struct TileHit {
    clicked: bool,
    remove: bool,
    edit: bool,
}

fn tile(app: &mut RingDesignerApp, ui: &mut egui::Ui, name: &str, selected: bool) -> TileHit {
    let tex = app.thumbnail(ui.ctx(), name);
    let dims = app.lib.get(name).map(|a| (a.width, a.height)).unwrap_or((0, 0));
    let mut hit = TileHit::default();

    ui.allocate_ui_with_layout(
        egui::vec2(THUMB, THUMB + LABEL_H),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            // Letterboxed into the square cell: many imported alphas are tall
            // or wide strips, and stretching them to square makes them
            // unrecognisable in the grid.
            let fitted = match dims {
                (w, h) if w > 0 && h > 0 => {
                    let s = (THUMB / w as f32).min(THUMB / h as f32);
                    egui::vec2((w as f32 * s).max(1.0), (h as f32 * s).max(1.0))
                }
                _ => egui::vec2(THUMB, THUMB),
            };
            let resp = match tex {
                Some(id) => {
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(THUMB, THUMB), egui::Sense::click());
                    let inner = egui::Align2::CENTER_CENTER.anchor_size(rect.center(), fitted);
                    egui::Image::new(egui::load::SizedTexture::new(id, fitted))
                        .corner_radius(3.0)
                        .paint_at(ui, inner);
                    resp
                }
                None => ui.allocate_response(egui::vec2(THUMB, THUMB), egui::Sense::click()),
            };
            hit.clicked = resp.clicked();

            let stroke = if selected {
                Some(egui::Stroke::new(1.6, theme::ACCENT))
            } else if resp.hovered() {
                Some(egui::Stroke::new(1.0, theme::ACCENT_DIM))
            } else {
                None
            };
            if let Some(s) = stroke {
                ui.painter().rect_stroke(
                    resp.rect.expand(1.0),
                    4.0,
                    s,
                    egui::StrokeKind::Outside,
                );
            }

            let resp = resp.on_hover_ui(|ui| {
                ui.strong(name);
                ui.label(
                    egui::RichText::new(format!("{} x {} px", dims.0, dims.1))
                        .color(theme::TEXT_DIM),
                );
            });
            resp.context_menu(|ui| {
                if ui.button(format!("{} Edit / clip…", icon::SCISSORS)).clicked() {
                    hit.edit = true;
                    ui.close();
                }
                if ui.button(format!("{} Remove", icon::TRASH)).clicked() {
                    hit.remove = true;
                    ui.close();
                }
            });

            let color = if selected { theme::ACCENT } else { theme::TEXT_DIM };
            ui.add(egui::Label::new(egui::RichText::new(name).small().color(color)).truncate());
        },
    );

    hit
}

/// Retargets the selected tiling layer, or adds a new one using the alpha.
fn apply_click(app: &mut RingDesignerApp, target: Option<usize>, name: String) {
    match target {
        Some(i) => {
            if let Some(Layer::Tiling(t)) = app.design.layers.layers.get_mut(i).map(|e| &mut e.layer)
            {
                t.alpha = name.clone();
                app.mark_dirty();
                app.set_status(format!("Tile set to {name}"));
            }
        }
        None => {
            let fctx = app.design.field_context();
            app.add_layer(
                name.clone(),
                Layer::Tiling(TilingLayer::default_for(name, &fctx)),
            );
        }
    }
}
