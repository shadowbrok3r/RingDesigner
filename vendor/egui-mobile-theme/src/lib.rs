//! Mastertech's exported scheme, with one explicit density for each input modality.
//! The export remains intact so glass settings and future style fields are retained.
use egui::{Color32, Context, FontId, Style, TextStyle};
use std::sync::OnceLock;

mod config;
pub use config::{
    Colors, Glass, Role, ThemeConfig, color, editor, editor_with_glass, glass, role_button, styled,
    with_role,
};

pub const CONTROL_HEIGHT: f32 = 36.0;
pub const DESKTOP_CONTROL_HEIGHT: f32 = 22.0;
pub const GLASS_BLUR_RADIUS: f32 = 32.0;
pub const GLASS_TINT: [u8; 4] = [5, 5, 7, 81];
pub const GLASS_CORNER_RADIUS: f32 = 4.0;

/// egui exports premultiplied Color32; the blur renderer consumes straight sRGBA.
pub fn glass_tint_srgba() -> [u8; 4] {
    let [r, g, b, a] = GLASS_TINT;
    Color32::from_rgba_premultiplied(r, g, b, a).to_srgba_unmultiplied()
}

pub mod palette {
    use super::Color32;
    pub const PANEL: Color32 = Color32::from_rgb(8, 8, 10);
    pub const WELL: Color32 = Color32::from_rgb(4, 4, 6);
    pub const WINDOW: Color32 = Color32::from_rgb(17, 17, 21);
    pub const TEXT: Color32 = Color32::from_rgb(226, 226, 234);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(136, 136, 140);
    pub const ACCENT: Color32 = Color32::from_rgb(209, 166, 231);
    pub const HOVER: Color32 = Color32::from_rgb(193, 164, 222);
    pub const LINK: Color32 = Color32::from_rgb(229, 152, 194);
    pub const INFO: Color32 = Color32::from_rgb(126, 200, 230);
    pub const ERROR: Color32 = Color32::from_rgb(236, 98, 132);
    pub const BORDER: Color32 = Color32::from_rgba_premultiplied(124, 126, 140, 204);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Compact,
    Touch,
}

/// Exact desktop spacing and colors from the supplied Mastertech export.
pub fn desktop_style() -> Style {
    static STYLE: OnceLock<Style> = OnceLock::new();
    STYLE
        .get_or_init(|| {
            let export: serde_json::Value =
                serde_json::from_str(include_str!("mastertech_color_scheme.json"))
                    .expect("embedded Mastertech export must be valid JSON");
            let mut base = serde_json::to_value(Style::default()).expect("serialize egui defaults");
            merge(&mut base, export["style"].clone());
            serde_json::from_value(base).expect("Mastertech style must match egui 0.37")
        })
        .clone()
}

/// Same palette with a single 36-point control height for touch input.
pub fn touch_style() -> Style {
    let mut style = desktop_style();
    for (role, size) in [
        (TextStyle::Body, 14.0),
        (TextStyle::Button, 14.0),
        (TextStyle::Small, 12.0),
        (TextStyle::Heading, 18.0),
        (TextStyle::Monospace, 13.0),
    ] {
        style.text_styles.insert(role, FontId::proportional(size));
    }
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(13.0));
    style.spacing.interact_size = egui::vec2(CONTROL_HEIGHT, CONTROL_HEIGHT);
    style.spacing.button_padding = egui::vec2(7.0, 5.0);
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.icon_width = 18.0;
    style.spacing.icon_width_inner = 10.0;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.handle_min_length = 28.0;
    style.compact_menu_style = false;
    style
}

pub fn apply(ctx: &Context, density: Density) {
    ThemeConfig::default().apply(ctx, density);
}

/// Restore the same widget spacing in popup menus (egui normally compacts it).
pub fn menu_style(ui: &mut egui::Ui) {
    let global = ui.ctx().global_style();
    let style = ui.style_mut();
    style.spacing.button_padding = global.spacing.button_padding;
    style.spacing.interact_size = global.spacing.interact_size;
    style.spacing.item_spacing = global.spacing.item_spacing;
    style.visuals.widgets = global.visuals.widgets.clone();
}

fn merge(base: &mut serde_json::Value, over: serde_json::Value) {
    match (base, over) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                merge(b.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (b, o) => *b = o,
    }
}

/// Legacy `Button::small` calls change typography, never the shared control height.
#[derive(Debug)]
struct UniformButtons;
impl egui::theme::StyleProvider<egui::widget_style::ButtonStyle> for UniformButtons {
    fn style(
        &mut self,
        args: &egui::widget_style::StyleArgs<'_>,
    ) -> egui::widget_style::ButtonStyle {
        use egui::theme::{DefaultStyle, StyleProvider};
        let mut style =
            StyleProvider::<egui::widget_style::ButtonStyle>::style(&mut DefaultStyle, args);
        style.atom_layout.min_size.y = args.style.spacing.interact_size.y;
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_palette_and_glass_are_preserved() {
        let style = desktop_style();
        assert_eq!(style.visuals.panel_fill, palette::PANEL);
        assert_eq!(style.visuals.window_fill, palette::WINDOW);
        assert_eq!(style.visuals.text_color(), palette::TEXT);
        assert_eq!(
            style.visuals.widgets.active.bg_stroke.color,
            palette::ACCENT
        );
        assert_eq!(style.spacing.interact_size.y, DESKTOP_CONTROL_HEIGHT);
        let export: serde_json::Value =
            serde_json::from_str(include_str!("mastertech_color_scheme.json")).unwrap();
        assert_eq!(
            export["glass"]["blur_radius"].as_f64().unwrap(),
            GLASS_BLUR_RADIUS as f64
        );
        assert_eq!(export["glass"]["tint"], serde_json::json!(GLASS_TINT));
    }

    #[test]
    fn ordinary_controls_share_the_density_height() {
        for (density, expected) in [
            (Density::Compact, DESKTOP_CONTROL_HEIGHT),
            (Density::Touch, CONTROL_HEIGHT),
        ] {
            let ctx = Context::default();
            apply(&ctx, density);
            let mut heights = Vec::new();
            let mut text = String::from("Text");
            let mut number = 1.0;
            let mut checked = true;
            for _ in 0..3 {
                heights.clear();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(600.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        heights.push(("button", ui.button("Button").rect.height()));
                        heights.push(("small button", ui.small_button("Small").rect.height()));
                        heights.push((
                            "selected",
                            ui.selectable_label(true, "Selected").rect.height(),
                        ));
                        heights.push((
                            "text field",
                            ui.text_edit_singleline(&mut text).rect.height(),
                        ));
                        heights.push((
                            "number",
                            ui.add(egui::DragValue::new(&mut number)).rect.height(),
                        ));
                        heights
                            .push(("checkbox", ui.checkbox(&mut checked, "Check").rect.height()));
                        heights.push((
                            "combo",
                            egui::ComboBox::from_id_salt("combo")
                                .selected_text("Choice")
                                .show_ui(ui, |_| {})
                                .response
                                .rect
                                .height(),
                        ));
                    },
                );
                output.textures_delta.clear();
            }
            for (name, height) in heights {
                assert!(
                    (height - expected).abs() < 0.5,
                    "{density:?} {name}: {height}, expected {expected}"
                );
            }
        }
    }
}
