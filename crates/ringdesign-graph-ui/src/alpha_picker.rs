//! Visual alpha inputs. Portable PNG bytes stay in the graph, outside text layout.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use egui::{Context, Id, TextureHandle, Ui};
use ringdesign_core::{Alpha, AlphaLibrary};
use ringdesign_graph::{registry::{PinSpec, Widget}, value::Literal};

#[derive(Default)]
struct Picker {
    library: Arc<AlphaLibrary>,
    thumbnails: HashMap<String, TextureHandle>,
    previews: HashMap<Id, Preview>,
    request: Option<(Id, bool)>,
    chosen: Option<(Id, Literal)>,
    filter: String,
    error: Option<String>,
}

struct Preview {
    payload: String,
    texture: Option<TextureHandle>,
    description: String,
}

fn state(ctx: &Context) -> Arc<Mutex<Picker>> {
    ctx.data_mut(|data| data.get_temp_mut_or_default::<Arc<Mutex<Picker>>>(Id::new("graph-alpha-picker")).clone())
}

pub fn is_open(ctx: &Context) -> bool { state(ctx).lock().unwrap().request.is_some() }

/// The host's current library includes built-ins, imports and this design's artwork.
pub fn set_library(ctx: &Context, library: Arc<AlphaLibrary>) {
    let shared = state(ctx);
    let mut picker = shared.lock().unwrap();
    if !Arc::ptr_eq(&picker.library, &library) {
        picker.thumbnails.clear();
        picker.library = library;
    }
}

fn texture(ctx: &Context, alpha: &Alpha) -> TextureHandle {
    let (w, h, rgba) = alpha.thumbnail_rgba8(96);
    ctx.load_texture(format!("alpha-picker/{}", alpha.name), egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba), egui::TextureOptions::LINEAR)
}

impl Picker {
    fn thumbnail(&mut self, ctx: &Context, name: &str) -> Option<TextureHandle> {
        if let Some(t) = self.thumbnails.get(name) { return Some(t.clone()); }
        let t = texture(ctx, self.library.get(name)?);
        self.thumbnails.insert(name.into(), t.clone());
        Some(t)
    }

    fn preview(&mut self, ctx: &Context, id: Id, payload: &str) -> &Preview {
        if !self.previews.get(&id).is_some_and(|p| p.payload == payload) {
            // Bound old node previews as projects and graph layouts change.
            if self.previews.len() >= 128 { self.previews.clear(); }
            let alpha = STANDARD.decode(payload).ok().and_then(|png| Alpha::from_png16("Selected image", &png).ok());
            let description = alpha.as_ref().map(|a| format!("{} × {} height image", a.width, a.height))
                .unwrap_or_else(|| if payload.is_empty() { "No image selected".into() } else { "Image could not be decoded. Choose another alpha.".into() });
            self.previews.insert(id, Preview { payload: payload.into(), texture: alpha.as_ref().map(|a| texture(ctx, a)), description });
        }
        &self.previews[&id]
    }
}

pub fn widget(ui: &mut Ui, pin: &PinSpec, literal: &mut Option<Literal>) -> bool {
    let id = ui.id().with(&pin.name);
    let shared = state(ui.ctx());
    let mut picker = shared.lock().unwrap();
    let mut changed = false;
    if picker.chosen.as_ref().is_some_and(|(target, _)| *target == id) {
        let (_, value) = picker.chosen.take().unwrap();
        changed = literal.as_ref() != Some(&value);
        *literal = Some(value);
    }
    let text = match literal.as_ref().or(pin.default.as_ref()) {
        Some(Literal::Text(text)) => text.as_str(),
        _ => "",
    };
    let embedded = pin.widget == Widget::Image;
    let (thumb, description) = if embedded {
        let preview = picker.preview(ui.ctx(), id, text);
        (preview.texture.clone(), preview.description.clone())
    } else {
        (picker.thumbnail(ui.ctx(), text), text.to_string())
    };
    let caption = if embedded || text.is_empty() { "Choose alpha…" } else { text };
    let edge = (ui.spacing().interact_size.y - ui.spacing().button_padding.y * 2.).min(22.);
    let button = match thumb {
        Some(t) => egui::Button::new((egui::Image::new((t.id(), t.size_vec2())).fit_to_exact_size(egui::vec2(edge, edge)), caption)),
        None => egui::Button::new(caption),
    };
    if ui.add(button.truncate()).on_hover_text(description).clicked() {
        picker.request = Some((id, embedded));
        picker.filter.clear();
        picker.error = None;
    }
    changed
}

/// Paint once after the host's graph, inspectors and viewport overlays.
pub fn show(ctx: &Context) {
    show_in(ctx, ctx.content_rect());
}

/// Mobile hosts supply their safe area, including the visible keyboard inset.
pub fn show_in(ctx: &Context, bounds: egui::Rect) {
    let shared = state(ctx);
    let mut picker = shared.lock().unwrap();
    let Some((target, embedded)) = picker.request else { return };
    let mut choice = None;
    let mut close = false;
    let width = (bounds.width() - 40.).clamp(240., 520.);
    let id = Id::new("choose-graph-alpha");
    let response = egui::Modal::new(id).area(egui::Modal::default_area(id).constrain_to(bounds)).show(ctx, |ui| {
        ui.set_width(width);
        let header_top = ui.cursor().top();
        ui.horizontal(|ui| {
            ui.strong("Choose alpha");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { close = ui.button("Cancel").clicked(); });
        });
        let label = ui.label("Search alphas");
        ui.add(egui::TextEdit::singleline(&mut picker.filter).hint_text("Name or pattern").desired_width(f32::INFINITY)).labelled_by(label.id);
        let query = picker.filter.to_lowercase();
        let mut names: Vec<_> = picker.library.iter()
            .filter(|a| !a.name.ends_with(ringdesign_core::alpha::SDF_SUFFIX) && a.name.to_lowercase().contains(&query))
            .map(|a| a.name.clone()).collect();
        names.sort_by_cached_key(|name| name.to_lowercase());
        ui.weak(format!("{} alphas · imports are available from the Alphas library", names.len()));
        // Virtual rows avoid decoding/rasterizing offscreen choices.
        let columns = (width / 116.).floor().max(2.) as usize;
        let cell = (width - 8. * (columns - 1) as f32) / columns as f32;
        let row_height = 90. + ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        let grid_height = (bounds.height() - (ui.cursor().top() - header_top) - 40.).clamp(40., 470.);
        // Areas retain their last size. A short search must not permanently
        // shrink the next gallery to the scroll area's default 64-point minimum.
        egui::ScrollArea::vertical().id_salt("alpha-choices").min_scrolled_height(grid_height).max_height(grid_height)
            .show_rows(ui, row_height, names.len().div_ceil(columns), |ui, rows| {
                for row in rows {
                    ui.horizontal(|ui| {
                        for name in names.iter().skip(row * columns).take(columns) {
                            ui.allocate_ui_with_layout(egui::vec2(cell, row_height), egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.set_width(cell);
                                if let Some(t) = picker.thumbnail(ctx, name) {
                                    let image = egui::Image::new((t.id(), t.size_vec2())).fit_to_exact_size(egui::vec2(cell.min(92.), 86.));
                                    if ui.add(egui::Button::image(image).min_size(egui::vec2(cell, 90.))).on_hover_text(name).clicked() { choice = Some(name.clone()); }
                                }
                                if ui.add_sized([cell, ui.spacing().interact_size.y], egui::Button::new(name).truncate()).on_hover_text(name).clicked() { choice = Some(name.clone()); }
                            });
                        }
                    });
                }
            });
        if names.is_empty() { ui.weak("No matching alphas. Try a different search."); }
        if let Some(error) = &picker.error { ui.colored_label(ui.visuals().error_fg_color, error); }
    });
    if let Some(name) = choice {
        let value = if embedded {
            picker.library.get(&name).unwrap().to_png16().map(|png| Literal::Text(STANDARD.encode(png))).map_err(|e| e.to_string())
        } else { Ok(Literal::Text(name)) };
        match value {
            Ok(value) => { picker.chosen = Some((target, value)); close = true; ctx.request_repaint(); }
            Err(error) => picker.error = Some(error),
        }
    }
    if close || response.should_close() { picker.request = None; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};
    use ringdesign_graph::value::ValueKind;

    #[test]
    fn png_input_uses_cached_preview_and_visual_choice_without_text_layout() {
        let pixels = (0..512 * 512).map(|i| ((i * 73 % 65535) as f32) / 65535.).collect();
        let original = Alpha::new("Original", 512, 512, pixels);
        let payload = STANDARD.encode(original.to_png16().unwrap());
        let replacement = Alpha::new("Chosen scales", 16, 16, vec![0.75; 256]);
        let expected = STANDARD.encode(replacement.to_png16().unwrap());
        let mut library = AlphaLibrary::default();
        library.insert(replacement);
        let library = Arc::new(library);
        let pin = PinSpec::item("png_base64", ValueKind::Text).widget(Widget::Image);
        let mut h = Harness::builder().with_size([600., 560.]).build_ui_state(|ui, value| {
            set_library(ui.ctx(), library.clone());
            crate::widgets::pin_widget(ui, &pin, value);
            show(ui.ctx());
        }, Some(Literal::Text(payload.clone())));
        h.run_steps(3);
        let cache = state(&h.ctx);
        let first = cache.lock().unwrap().previews.values().next().unwrap().texture.as_ref().unwrap().id();
        h.run_steps(15);
        assert_eq!(cache.lock().unwrap().previews.values().next().unwrap().texture.as_ref().unwrap().id(), first);
        assert!(h.query_by_label_contains("Source (").is_none());
        h.get_by_label("Choose alpha…").click();
        h.run_steps(3);
        h.get_by_label("Cancel").click();
        h.run_steps(3);
        assert_eq!(h.state(), &Some(Literal::Text(payload)));
        h.get_by_label("Choose alpha…").click();
        h.run_steps(3);
        h.get_by_label("Chosen scales").click();
        h.run_steps(3);
        assert_eq!(h.state(), &Some(Literal::Text(expected)));
        assert!(cache.lock().unwrap().request.is_none());
    }

    #[test]
    fn image_cache_refreshes_when_library_replaces_a_same_named_alpha() {
        let ctx = Context::default();
        let mut library = AlphaLibrary::default();
        library.insert(Alpha::new("Scales", 2, 2, vec![0.; 4]));
        set_library(&ctx, Arc::new(library.clone()));
        let shared = state(&ctx);
        let first = shared.lock().unwrap().thumbnail(&ctx, "Scales").unwrap().id();
        library.insert(Alpha::new("Scales", 2, 2, vec![1.; 4]));
        set_library(&ctx, Arc::new(library));
        let second = shared.lock().unwrap().thumbnail(&ctx, "Scales").unwrap().id();
        assert_ne!(first, second);
    }

    #[test]
    fn image_cards_stay_inside_phone_and_desktop_dialogs() {
        for width in [360., 600., 900.] {
            let mut library = AlphaLibrary::default();
            for i in 0..4 {
                library.insert(Alpha::new(format!("Tile {i}"), 2, 2, vec![0.5; 4]));
            }
            let library = Arc::new(library);
            let pin = PinSpec::item("alpha", ValueKind::AlphaRef);
            let mut h = Harness::builder().with_size([width, 720.]).build_ui_state(|ui, value| {
                set_library(ui.ctx(), library.clone());
                widget(ui, &pin, value);
                show(ui.ctx());
            }, None);
            h.get_by_label("Choose alpha…").click();
            h.run_steps(4);
            for i in 0..4 {
                let rect = h.get_by_label(&format!("Tile {i}")).rect();
                assert!(rect.left() >= 0. && rect.right() <= width, "{width}: {rect:?}");
            }
        }
    }

    #[test]
    fn reopening_after_empty_results_restores_space_for_thumbnail_captions() {
        let mut library = AlphaLibrary::default();
        for i in 0..36 {
            library.insert(Alpha::new(format!("Alpha {i:02}"), 2, 2, vec![0.5; 4]));
        }
        let library = Arc::new(library);
        let pin = PinSpec::item("alpha", ValueKind::AlphaRef);
        let full_bounds = egui::Rect::from_min_max(egui::pos2(0., 24.), egui::pos2(412., 870.));
        let mut h = Harness::builder().with_size([412., 890.]).build_ui_state(|ui, (value, bounds)| {
            set_library(ui.ctx(), library.clone());
            widget(ui, &pin, value);
            show_in(ui.ctx(), *bounds);
        }, (None, full_bounds));
        h.get_by_label("Choose alpha…").click();
        h.run_steps(4);
        state(&h.ctx).lock().unwrap().filter = "no-match".into();
        h.run_steps(4);
        h.get_by_label("Cancel").click();
        h.run_steps(4);
        h.get_by_label("Choose alpha…").click();
        h.run_steps(4);
        let dialog = h.ctx.memory(|m| m.area_rect(Id::new("choose-graph-alpha"))).unwrap();
        let caption = h.get_by_label("Alpha 00").rect();
        assert!(caption.bottom() < dialog.bottom(), "caption clipped after reopening: {caption:?} in {dialog:?}");
        assert!(dialog.height() > 350., "gallery must regain its browsing height: {dialog:?}");
        let keyboard_bounds = egui::Rect::from_min_max(egui::pos2(0., 24.), egui::pos2(412., 390.));
        h.state_mut().1 = keyboard_bounds;
        h.run_steps(4);
        let dialog = h.ctx.memory(|m| m.area_rect(Id::new("choose-graph-alpha"))).unwrap();
        assert!(keyboard_bounds.contains_rect(dialog), "dialog must respect the keyboard: {dialog:?}");
        assert!(h.get_by_label("Alpha 00").rect().bottom() < dialog.bottom());
        h.state_mut().1 = full_bounds;
        h.run_steps(4);
        let dialog = h.ctx.memory(|m| m.area_rect(Id::new("choose-graph-alpha"))).unwrap();
        assert!(dialog.height() > 350., "gallery must grow again after keyboard dismissal");
    }
}
