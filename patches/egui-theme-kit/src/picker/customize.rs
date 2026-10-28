//! The Customize section: layout, glass material, accent roles and surface tokens, edited live.

use eframe::egui::{
    Color32, ComboBox, CornerRadius, DragValue, Grid, RichText, Slider, Stroke, StrokeKind, Ui,
    vec2,
};

use super::{
    edit::{self, Role, Semantic, Surface},
    label,
    state::PickerState,
};
use crate::{
    colors,
    fonts::{self, RichTextExt as _, TextScale},
    glass::{self, GlassParams},
    kit::{self, ButtonKind},
    metrics::{self, Density},
    saved::SemanticColors,
    tokens::{Tokens, contrast},
};

/// Largest blur radius offered, in points.
const MAX_BLUR: f32 = 64.0;
/// Largest glass corner radius offered, in points.
const MAX_CORNER: f32 = 32.0;
/// Corner radius of the outline drawn around a colour swatch.
const SWATCH_RADIUS: u8 = 4;
/// Least width of the name column in the colour grids.
const NAME_COL_W: f32 = 96.0;

pub(super) fn show(ui: &mut Ui, state: &mut PickerState) {
    layout_block(ui, state);
    glass_block(ui, state);
    colors_block(ui, state);
    surfaces_block(ui, state);
    warnings_block(ui);
    let ctx = ui.ctx().clone();
    ui.add_space(ui.spacing().item_spacing.y);
    if ui
        .add_enabled(
            state.is_modified(),
            kit::button(
                ButtonKind::Ghost,
                label(state, "reset", state.reset_label()),
            ),
        )
        .clicked()
    {
        state.reset_to_origin(&ctx);
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(ui.spacing().item_spacing.y);
    ui.label(RichText::new(text).semibold());
}

fn layout_block(ui: &mut Ui, state: &mut PickerState) {
    let ctx = ui.ctx().clone();
    heading(ui, "Layout");
    if state.options.compact {
        ui.horizontal_wrapped(|ui| {
            ui.label("Density");
            let mut density = metrics::density(&ctx);
            for d in Density::ALL {
                ui.selectable_value(&mut density, d, d.label());
            }
            state.set_density(&ctx, density);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Text size");
            let mut scale = fonts::text_scale(&ctx);
            for (s, label) in TextScale::ALL.into_iter().zip(["90%", "100%", "115%", "130%"]) {
                ui.selectable_value(&mut scale, s, label).on_hover_text(s.label());
            }
            state.set_text_scale(&ctx, scale);
        });
        return;
    }
    ui.horizontal(|ui| {
        let mut density = metrics::density(&ctx);
        ui.label("Density");
        ComboBox::from_id_salt("egui_theme_kit.picker.density")
            .selected_text(density.label())
            .show_ui(ui, |ui| {
                for d in Density::ALL {
                    ui.selectable_value(&mut density, d, d.label());
                }
            });
        state.set_density(&ctx, density);
    });
    ui.horizontal(|ui| {
        let mut scale = fonts::text_scale(&ctx);
        ui.label("Text size");
        ComboBox::from_id_salt("egui_theme_kit.picker.text_scale")
            .selected_text(scale.label())
            .show_ui(ui, |ui| {
                for s in TextScale::ALL {
                    ui.selectable_value(&mut scale, s, s.label());
                }
            });
        state.set_text_scale(&ctx, scale);
    });
}

fn glass_block(ui: &mut Ui, state: &mut PickerState) {
    let ctx = ui.ctx().clone();
    heading(ui, "Glass");
    let mut params = glass::params(&ctx);
    let before = params;
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut params.enabled, "Backdrop blur");
        if !state.options.glass_available.map_or_else(glass::is_available, |available| available()) {
            ui.label(
                RichText::new("GPU blur is unavailable on this host; the tint still applies.")
                    .weak()
                    .small(),
            );
        }
    });
    ui.add_enabled_ui(params.enabled, |ui| {
        let font = eframe::egui::TextStyle::Body.resolve(ui.style());
        let label_width = ui.fonts_mut(|f| f.layout_no_wrap("Corner radius".into(), font, Color32::WHITE).size().x);
        ui.spacing_mut().slider_width = ui.spacing().slider_width.min((ui.available_width() - label_width - 108.0).max(48.0));
        Grid::new("egui_theme_kit.picker.glass")
            .num_columns(2)
            .min_col_width(0.0)
            .spacing(vec2(12.0, 4.0))
            .show(ui, |ui| {
                ui.add(eframe::egui::Label::new("Blur radius").extend());
                ui.add(
                    DragValue::new(&mut params.blur_radius)
                        .range(0.0..=MAX_BLUR)
                        .speed(0.5)
                        .suffix(" px"),
                );
                ui.end_row();
                ui.add(eframe::egui::Label::new("Tint").extend());
                ui.color_edit_button_srgba(&mut params.tint);
                ui.end_row();
                ui.add(eframe::egui::Label::new("Corner radius").extend());
                ui.add(
                    DragValue::new(&mut params.corner_radius)
                        .range(0.0..=MAX_CORNER)
                        .speed(0.2)
                        .suffix(" px"),
                );
                ui.end_row();
                ui.add(eframe::egui::Label::new("Presence").extend());
                ui.add(Slider::new(&mut params.presence, 0.0..=1.0).step_by(0.01));
                ui.end_row();
            });
    });
    if params != before {
        state.set_glass(&ctx, params);
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(kit::button(ButtonKind::Ghost, "Material off"))
            .on_hover_text("Turn the glass material off for this theme.")
            .clicked()
        {
            state.set_glass(&ctx, GlassParams::OFF);
        }
        if ui
            .add(kit::button(
                ButtonKind::Ghost,
                label(state, "glass", "Glassify theme"),
            ))
            .on_hover_text(
                "Re-style the theme as tinted glass from its own colours: translucent fills with brighter outlines.",
            )
            .clicked()
        {
            state.glassify(&ctx);
        }
    });
    if state.options.glass_available.is_none() {
        glass::preview(ui, glass::params(&ctx));
    }
}

fn rgb(color: Color32) -> [u8; 3] {
    [color.r(), color.g(), color.b()]
}

fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

/// One editable colour row: name, picker, hex and contrast against `against`.
fn color_row(
    ui: &mut Ui,
    name: &str,
    hint: &str,
    current: Color32,
    against: Option<Color32>,
    compact: bool,
) -> Option<Color32> {
    ui.set_min_width(NAME_COL_W);
    ui.add(eframe::egui::Label::new(name).extend()).on_hover_text(hint);
    let mut channels = rgb(current);
    let button = ui.color_edit_button_srgb(&mut channels);
    let outline = Tokens::get(ui.ctx()).text_faint;
    ui.painter().rect_stroke(
        button.rect,
        CornerRadius::same(SWATCH_RADIUS),
        Stroke::new(1.0, outline),
        StrokeKind::Inside,
    );
    let changed = button.changed();
    let hex_label = ui.add(eframe::egui::Label::new(RichText::new(hex(current)).monospace().weak()).extend());
    if compact {
        if let Some(bg) = against { hex_label.on_hover_text(format!("Contrast {:.1}:1", contrast(current, bg))); }
    } else { match against {
        Some(bg) => {
            let ratio = contrast(current, bg);
            ui.label(RichText::new(format!("{ratio:.1}:1")).weak());
        }
        None => {
            ui.label("");
        }
    } }
    ui.end_row();
    changed.then(|| Color32::from_rgb(channels[0], channels[1], channels[2]))
}

fn colors_block(ui: &mut Ui, state: &mut PickerState) {
    let ctx = ui.ctx().clone();
    heading(ui, "Accent and status colors");
    let tokens = Tokens::get(&ctx);
    let semantic = SemanticColors::current(&ctx);
    let mut role_edit = None;
    let mut semantic_edit = None;
    Grid::new("egui_theme_kit.picker.roles")
        .num_columns(if state.options.compact { 3 } else { 4 })
        .min_col_width(0.0)
        .spacing(vec2(12.0, 4.0))
        .show(ui, |ui| {
            for role in Role::ALL {
                if let Some(c) = color_row(
                    ui,
                    role.label(),
                    role.hint(),
                    role.read(&tokens),
                    Some(tokens.base),
                    state.options.compact,
                ) {
                    role_edit = Some((role, c));
                }
            }
            for which in Semantic::ALL {
                let current = composite_over(which.read(&semantic), tokens.base);
                if let Some(c) =
                    color_row(ui, which.label(), which.hint(), current, Some(tokens.base), state.options.compact)
                {
                    semantic_edit = Some((which, c));
                }
            }
        });
    if let Some((role, c)) = role_edit {
        state.set_role(&ctx, role, c);
    }
    if let Some((which, c)) = semantic_edit {
        state.set_semantic(&ctx, which, c);
    }
}

fn composite_over(color: Color32, under: Color32) -> Color32 {
    crate::tokens::composite(color, under)
}

fn surfaces_block(ui: &mut Ui, state: &mut PickerState) {
    let ctx = ui.ctx().clone();
    heading(ui, "Surfaces and text");
    let tokens = Tokens::get(&ctx);
    let mut edit = None;
    Grid::new("egui_theme_kit.picker.surfaces")
        .num_columns(if state.options.compact { 3 } else { 4 })
        .min_col_width(0.0)
        .spacing(vec2(12.0, 4.0))
        .show(ui, |ui| {
            for surface in Surface::ALL {
                let against =
                    matches!(surface, Surface::Text | Surface::TextDim).then_some(tokens.base);
                if let Some(c) = color_row(
                    ui,
                    surface.label(),
                    surface.hint(),
                    surface.read(&tokens),
                    against,
                    state.options.compact,
                ) {
                    edit = Some((surface, c));
                }
            }
        });
    if let Some((surface, c)) = edit {
        state.set_surface(&ctx, surface, c);
    }
}

fn warnings_block(ui: &mut Ui) {
    let tokens = Tokens::get(ui.ctx());
    let warnings = edit::contrast_warnings(&tokens);
    if warnings.is_empty() {
        return;
    }
    heading(ui, "Contrast warnings");
    for w in warnings {
        ui.label(
            RichText::new(format!(
                "{}: {:.1}:1, below {:.1}:1",
                w.message, w.ratio, w.floor
            ))
            .color(colors::warn(ui)),
        );
    }
}
