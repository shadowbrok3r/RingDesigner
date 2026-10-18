//! The preset gallery: grouped cards with a live miniature of each preset.

use eframe::egui::{
    Align2, Context, CornerRadius, FontId, Rect, RichText, Role, Sense, Stroke, StrokeKind,
    TextEdit, Ui, WidgetInfo, pos2, vec2,
};

use super::{
    label,
    preview::{self, PREVIEW_HEIGHT, Preview},
    state::{Origin, PickerState},
};
use crate::{
    fonts,
    presets::{self, PresetDef},
    store::{StartupTheme, ThemeStore},
    tokens::Tokens,
};

/// Narrowest a card gets before the grid drops a column.
const MIN_CARD_W: f32 = 176.0;
/// Height of the name row under a miniature.
const LABEL_H: f32 = 22.0;
const NAME_PT: f32 = 12.0;
const TAG_PT: f32 = 10.0;

pub(super) fn show(ui: &mut Ui, state: &mut PickerState, store: &dyn ThemeStore) {
    let ctx = ui.ctx().clone();
    let hint = label(state, "search", "Search presets");
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut state.search)
                .hint_text(hint)
                .desired_width(ui.available_width().min(280.0)),
        );
        if !state.search.is_empty() && ui.small_button("Clear").clicked() {
            state.search.clear();
        }
    });
    ui.add_space(ui.spacing().item_spacing.y);

    let query = state.search.trim().to_lowercase();
    let startup = store.startup();
    let live = Tokens::get(&ctx);
    let mut any = false;
    let mut picked: Option<(PresetDef, Pick)> = None;
    for group in presets::grouped_presets() {
        let defs: Vec<PresetDef> = group
            .presets
            .into_iter()
            .filter(|def| matches(def, &query))
            .collect();
        if defs.is_empty() {
            continue;
        }
        any = true;
        ui.label(RichText::new(group.title).weak().small());
        let columns = columns(ui);
        let width = card_width(ui, columns);
        for row in defs.chunks(columns) {
            ui.horizontal(|ui| {
                for def in row {
                    let cue = cue_for(state, &ctx, def, &startup);
                    if let Some(pick) = card(ui, state, def, width, cue, &live) {
                        picked = Some((*def, pick));
                    }
                }
            });
        }
        ui.add_space(ui.spacing().item_spacing.y);
    }
    if !any {
        ui.label(RichText::new(format!("No presets match \"{}\".", state.search.trim())).weak());
    }
    match picked {
        Some((def, Pick::Apply)) => {
            state.apply_preset(&ctx, def);
        }
        Some((def, Pick::Startup)) => {
            state.set_startup(store, StartupTheme::Preset(def.id.to_owned()));
        }
        None => {}
    }
}

pub(super) fn matches(def: &PresetDef, query: &str) -> bool {
    query.is_empty()
        || [def.name, def.id, def.group, def.description]
            .iter()
            .any(|text| text.to_lowercase().contains(query))
}

/// Height of the name row, grown with the text size.
fn label_height(ctx: &Context) -> f32 {
    LABEL_H.max(NAME_PT * fonts::text_scale(ctx).factor() + 10.0)
}

fn columns(ui: &Ui) -> usize {
    let gap = ui.spacing().item_spacing.x;
    (((ui.available_width() + gap) / (MIN_CARD_W + gap)).floor() as usize).max(1)
}

fn card_width(ui: &Ui, columns: usize) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    ((ui.available_width() - gap * (columns - 1) as f32) / columns as f32)
        .floor()
        .max(MIN_CARD_W * 0.75)
}

/// What a card says about its preset's relation to the live theme.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cue {
    None,
    /// The live theme is exactly this preset.
    Active,
    /// The live theme started from this preset and was edited.
    Edited,
}

fn cue_for(state: &PickerState, ctx: &Context, def: &PresetDef, _startup: &StartupTheme) -> Cue {
    if presets::installed_preset_id(ctx) == Some(def.id) {
        return Cue::Active;
    }
    match state.origin() {
        Some(Origin::Preset(id)) if *id == def.id => Cue::Edited,
        _ => Cue::None,
    }
}

enum Pick {
    Apply,
    Startup,
}

/// One preset card; returns what the user chose on it.
fn card(
    ui: &mut Ui,
    state: &mut PickerState,
    def: &PresetDef,
    width: f32,
    cue: Cue,
    live: &Tokens,
) -> Option<Pick> {
    let ctx = ui.ctx().clone();
    let label_h = label_height(&ctx);
    let size = vec2(width, PREVIEW_HEIGHT + label_h);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, def.name));

    let tokens = presets::preset_tokens(&ctx, def);
    let (glass, opacity) = *state.card_data.entry(def.id).or_insert_with(|| {
        let theme = def.theme();
        (def.glass(), preview::window_opacity(&theme))
    });
    let mini = Rect::from_min_size(rect.min, vec2(width, PREVIEW_HEIGHT));
    if ui.is_rect_visible(rect) {
        preview::paint(ui, mini, &Preview::new(&tokens, glass, opacity, def.name));
        paint_frame(ui, mini, &response, cue, &tokens, live);
        paint_label(ui, rect, label_h, def, cue, live);
    }

    let response = response.on_hover_text(def.description);
    let mut pick = response.clicked().then_some(Pick::Apply);
    response.context_menu(|ui| {
        if ui.button(label(state, "apply", "Apply")).clicked() {
            pick = Some(Pick::Apply);
            ui.close();
        }
        if ui
            .button(label(state, "startup", "Start the app with this"))
            .clicked()
        {
            pick = Some(Pick::Startup);
            ui.close();
        }
    });
    pick
}

fn paint_frame(
    ui: &Ui,
    mini: Rect,
    response: &eframe::egui::Response,
    cue: Cue,
    tokens: &Tokens,
    live: &Tokens,
) {
    let radius = CornerRadius::from(tokens.radius_md.min(10));
    let stroke = match (cue, response.hovered()) {
        (Cue::Active, _) => Stroke::new(2.0, live.primary),
        (Cue::Edited, _) => Stroke::new(1.5, live.primary.gamma_multiply(0.6)),
        (Cue::None, true) => Stroke::new(1.5, live.text_dim),
        (Cue::None, false) => Stroke::new(1.0, live.card_border),
    };
    ui.painter()
        .rect_stroke(mini, radius, stroke, StrokeKind::Inside);
}

fn paint_label(ui: &Ui, rect: Rect, label_h: f32, def: &PresetDef, cue: Cue, live: &Tokens) {
    let y = rect.bottom() - label_h / 2.0 + 2.0;
    let name_color = live.text;
    let painter = ui.painter();
    painter.text(
        pos2(rect.left() + 3.0, y),
        Align2::LEFT_CENTER,
        def.name,
        fonts::semibold(NAME_PT),
        name_color,
    );
    let tag = match cue {
        Cue::Active => Some(("Active", live.primary)),
        Cue::Edited => Some(("Edited", live.text_dim)),
        Cue::None => None,
    };
    if let Some((text, color)) = tag {
        painter.text(
            pos2(rect.right() - 3.0, y),
            Align2::RIGHT_CENTER,
            text,
            FontId::proportional(TAG_PT),
            color,
        );
    }
}
