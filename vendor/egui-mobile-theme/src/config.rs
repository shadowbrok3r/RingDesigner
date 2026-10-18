//! Persistable theme settings and the same responsive editor in every host app.
use crate::{Density, palette};
use egui::{Color32, Context, Stroke, Ui};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Colors {
    pub panel: Color32,
    pub window: Color32,
    pub well: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub widget: Color32,
    pub hovered: Color32,
    pub active: Color32,
    pub accent: Color32,
    pub primary: Color32,
    pub info: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
}
impl Default for Colors {
    fn default() -> Self {
        let v = crate::desktop_style().visuals;
        Self {
            panel: palette::PANEL,
            window: palette::WINDOW,
            well: palette::WELL,
            text: palette::TEXT,
            muted: palette::TEXT_DIM,
            border: palette::BORDER,
            widget: v.widgets.inactive.bg_fill,
            hovered: v.widgets.hovered.bg_fill,
            active: v.widgets.active.bg_fill,
            accent: palette::ACCENT,
            primary: palette::LINK,
            info: palette::INFO,
            success: Color32::from_rgb(143, 211, 174),
            warning: Color32::from_rgb(231, 193, 128),
            danger: palette::ERROR,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Glass {
    pub blur_radius: f32,
    pub tint: Color32,
    pub corner_radius: f32,
}
impl Default for Glass {
    fn default() -> Self {
        let [r, g, b, a] = crate::GLASS_TINT;
        Self {
            blur_radius: crate::GLASS_BLUR_RADIUS,
            tint: Color32::from_rgba_premultiplied(r, g, b, a),
            corner_radius: crate::GLASS_CORNER_RADIUS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub colors: Colors,
    /// None follows the host's default: 22 pt desktop, 36 pt touch.
    pub control_height: Option<f32>,
    pub text_scale: f32,
    pub spacing_scale: f32,
    pub rounding: u8,
    pub glass: Glass,
}
impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            colors: Colors::default(),
            control_height: None,
            text_scale: 1.,
            spacing_scale: 1.,
            rounding: 4,
            glass: Glass::default(),
        }
    }
}
fn finite(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
impl ThemeConfig {
    pub fn control_height(&self, density: Density) -> f32 {
        let default = match density {
            Density::Compact => crate::DESKTOP_CONTROL_HEIGHT,
            Density::Touch => crate::CONTROL_HEIGHT,
        };
        finite(
            self.control_height.unwrap_or(default),
            default,
            default.min(28.),
            52.,
        )
    }
    pub fn apply(&self, ctx: &Context, density: Density) {
        let mut style = match density {
            Density::Compact => crate::desktop_style(),
            Density::Touch => crate::touch_style(),
        };
        let scale = finite(self.text_scale, 1., 0.8, 1.3);
        let spacing = finite(self.spacing_scale, 1., 0.75, 1.5);
        for font in style.text_styles.values_mut() {
            font.size *= scale;
        }
        style.spacing.item_spacing *= spacing;
        style.spacing.button_padding *= spacing;
        let font_height = style
            .text_styles
            .get(&egui::TextStyle::Button)
            .map_or(14., |f| f.size * 1.25);
        let height = self
            .control_height(density)
            .max((font_height + 2. * style.spacing.button_padding.y).ceil());
        style.spacing.interact_size = egui::vec2(height, height);
        let c = &self.colors;
        let v = &mut style.visuals;
        v.panel_fill = c.panel;
        v.window_fill = c.window;
        v.extreme_bg_color = c.well;
        v.faint_bg_color = c.widget;
        v.code_bg_color = c.well;
        v.override_text_color = Some(c.text);
        v.weak_text_color = Some(c.muted);
        v.hyperlink_color = c.primary;
        v.warn_fg_color = c.warning;
        v.error_fg_color = c.danger;
        v.selection.bg_fill = tint(c.window, c.accent, 0.28);
        v.selection.stroke.color = c.accent;
        v.window_stroke.color = c.border;
        v.window_corner_radius = egui::CornerRadius::same(self.rounding.min(16));
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = egui::CornerRadius::same(self.rounding.min(16));
            w.fg_stroke.color = c.text;
            w.bg_stroke.color = c.border;
        }
        v.widgets.inactive.bg_fill = c.widget;
        v.widgets.inactive.weak_bg_fill = c.widget;
        v.widgets.hovered.bg_fill = c.hovered;
        v.widgets.hovered.weak_bg_fill = c.hovered;
        v.widgets.hovered.bg_stroke.color = c.info;
        v.widgets.active.bg_fill = c.active;
        v.widgets.active.weak_bg_fill = c.active;
        v.widgets.active.bg_stroke.color = c.accent;
        v.widgets.open.bg_fill = c.active;
        v.widgets.open.weak_bg_fill = c.active;
        v.widgets.open.bg_stroke.color = c.accent;
        ctx.all_styles_mut(|s| *s = style.clone());
        ctx.set_theme(egui::ThemePreference::Dark);
        ctx.add_widget_theme::<egui::widget_style::ButtonStyle>(crate::UniformButtons);
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::unique("mastertech-theme-config"), self.clone())
        });
        ctx.request_repaint();
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Role {
    Accent,
    Primary,
    Info,
    Success,
    Warning,
    Danger,
    Muted,
}
pub fn color(ctx: &Context, role: Role) -> Color32 {
    ctx.data(|data| {
        let config = data
            .get_temp::<ThemeConfig>(egui::Id::unique("mastertech-theme-config"))
            .unwrap_or_default();
        let c = config.colors;
        match role {
            Role::Accent => c.accent,
            Role::Primary => c.primary,
            Role::Info => c.info,
            Role::Success => c.success,
            Role::Warning => c.warning,
            Role::Danger => c.danger,
            Role::Muted => c.muted,
        }
    })
}
pub fn glass(ctx: &Context) -> Glass {
    let mut glass = ctx.data(|data| {
        data.get_temp::<ThemeConfig>(egui::Id::unique("mastertech-theme-config"))
            .unwrap_or_default()
            .glass
    });
    glass.blur_radius = finite(glass.blur_radius, crate::GLASS_BLUR_RADIUS, 0., 64.);
    glass.corner_radius = finite(glass.corner_radius, crate::GLASS_CORNER_RADIUS, 0., 24.);
    glass
}
fn tint(base: Color32, color: Color32, amount: f32) -> Color32 {
    let blend = |a: u8, b: u8| (a as f32 * (1. - amount) + b as f32 * amount).round() as u8;
    Color32::from_rgb(
        blend(base.r(), color.r()),
        blend(base.g(), color.g()),
        blend(base.b(), color.b()),
    )
}
/// Give a related action its role while retaining shared dimensions and disabled behavior.
pub fn with_role<R>(
    ui: &mut Ui,
    role: Role,
    content: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.scope(|ui| styled(ui, role, content))
}
/// Tint widgets without introducing a child layout, so wrapped rows can measure each button.
pub fn styled<R>(ui: &mut Ui, role: Role, content: impl FnOnce(&mut Ui) -> R) -> R {
    let color = color(ui.ctx(), role);
    let saved = ui.style().clone();
    let v = &mut ui.style_mut().visuals;
    v.override_text_color = Some(color);
    for (widget, amount) in [
        (&mut v.widgets.inactive, 0.10),
        (&mut v.widgets.hovered, 0.20),
        (&mut v.widgets.active, 0.28),
        (&mut v.widgets.open, 0.20),
    ] {
        widget.bg_fill = tint(widget.bg_fill, color, amount);
        widget.weak_bg_fill = widget.bg_fill;
        widget.fg_stroke.color = color;
        widget.bg_stroke = Stroke::new(1., color.gamma_multiply(0.65));
    }
    let result = content(ui);
    ui.set_style(saved);
    result
}
pub fn role_button(ui: &mut Ui, text: impl Into<egui::WidgetText>, role: Role) -> egui::Response {
    styled(ui, role, |ui| {
        ui.add(egui::Button::new(text.into()).wrap_mode(egui::TextWrapMode::Extend))
    })
}

/// Responsive, scroll-container-independent editor. The host saves and applies on true.
pub fn editor(ui: &mut Ui, config: &mut ThemeConfig, density: Density) -> bool {
    editor_with_glass(ui, config, density, true)
}

/// Hosts without a backdrop renderer can omit glass-only controls.
pub fn editor_with_glass(
    ui: &mut Ui,
    config: &mut ThemeConfig,
    density: Density,
    supports_glass: bool,
) -> bool {
    let before = config.clone();
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.heading("Theme");
    ui.label("Changes preview immediately and are saved on this device.");
    ui.add_space(6.);
    ui.strong("Size and spacing");
    ui.spacing_mut().slider_width = (ui.available_width() - 120.).clamp(60., 240.);
    let label = ui.label("Control height");
    let mut height = config.control_height(density);
    let min = if density == Density::Compact {
        22.
    } else {
        28.
    };
    if ui
        .add(
            egui::Slider::new(&mut height, min..=52.)
                .step_by(1.)
                .fixed_decimals(0)
                .suffix(" pt"),
        )
        .labelled_by(label.id)
        .changed()
    {
        config.control_height = Some(height);
    }
    let label = ui.label("Text size");
    ui.add(
        egui::Slider::new(&mut config.text_scale, 0.8..=1.3)
            .step_by(0.05)
            .fixed_decimals(2)
            .suffix("×"),
    )
    .labelled_by(label.id);
    let label = ui.label("Spacing");
    ui.add(
        egui::Slider::new(&mut config.spacing_scale, 0.75..=1.5)
            .step_by(0.05)
            .fixed_decimals(2)
            .suffix("×"),
    )
    .labelled_by(label.id);
    let label = ui.label("Corner radius");
    ui.add(egui::Slider::new(&mut config.rounding, 0..=16).suffix(" pt"))
        .labelled_by(label.id);
    ui.add_space(8.);
    ui.strong("Color roles");
    for (name, color) in [
        ("Selection", &mut config.colors.accent),
        ("Main actions", &mut config.colors.primary),
        ("Navigation / info", &mut config.colors.info),
        ("Success", &mut config.colors.success),
        ("Warnings", &mut config.colors.warning),
        ("Destructive / errors", &mut config.colors.danger),
    ] {
        color_row(ui, name, color);
    }
    ui.collapsing("Surfaces and text", |ui| {
        for (name, color) in [
            ("Panels", &mut config.colors.panel),
            ("Windows / cards", &mut config.colors.window),
            ("Input / canvas wells", &mut config.colors.well),
            ("Text", &mut config.colors.text),
            ("Muted text", &mut config.colors.muted),
            ("Borders", &mut config.colors.border),
            ("Idle widgets", &mut config.colors.widget),
            ("Hovered widgets", &mut config.colors.hovered),
            ("Pressed widgets", &mut config.colors.active),
        ] {
            color_row(ui, name, color);
        }
    });
    if supports_glass {
        ui.collapsing("Glass surfaces", |ui| {
            ui.small("Lower window opacity to reveal the glass behind cards.");
            let label = ui.label("Blur radius");
            ui.add(
                egui::Slider::new(&mut config.glass.blur_radius, 0.0..=64.)
                    .step_by(1.)
                    .suffix(" px"),
            )
            .labelled_by(label.id);
            color_row(ui, "Tint / opacity", &mut config.glass.tint);
            let label = ui.label("Glass corners");
            ui.add(
                egui::Slider::new(&mut config.glass.corner_radius, 0.0..=24.)
                    .step_by(1.)
                    .suffix(" pt"),
            )
            .labelled_by(label.id);
        });
    }
    ui.add_space(8.);
    ui.strong("Preview");
    ui.horizontal_wrapped(|ui| {
        for (text, role) in [
            ("Action", Role::Primary),
            ("Navigate", Role::Info),
            ("Success", Role::Success),
            ("Warning", Role::Warning),
            ("Delete", Role::Danger),
        ] {
            let _ = role_button(ui, text, role);
        }
        let _ = ui.selectable_label(true, "Selected");
    });
    ui.add_space(8.);
    if ui.button("Reset to Mastertech").clicked() {
        *config = ThemeConfig::default();
    }
    *config != before
}
fn color_row(ui: &mut Ui, name: &str, color: &mut Color32) {
    ui.horizontal(|ui| {
        let response = ui.color_edit_button_srgba(color);
        let label = ui.add(egui::Label::new(name).wrap());
        response.labelled_by(label.id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_saved_config_migrates_and_custom_values_round_trip() {
        let config: ThemeConfig = serde_json::from_str(r#"{"text_scale":1.1}"#).unwrap();
        assert_eq!(config.colors, Colors::default());
        assert_eq!(config.text_scale, 1.1);
        let mut custom = config;
        custom.colors.primary = Color32::from_rgb(17, 155, 221);
        custom.control_height = Some(34.);
        custom.glass.tint = Color32::from_rgba_unmultiplied(12, 23, 34, 90);
        let loaded: ThemeConfig =
            serde_json::from_str(&serde_json::to_string(&custom).unwrap()).unwrap();
        assert_eq!(loaded, custom);
        let ctx = Context::default();
        loaded.apply(&ctx, Density::Touch);
        assert_eq!(color(&ctx, Role::Primary), custom.colors.primary);
        assert_eq!(glass(&ctx).tint, custom.glass.tint);
        assert_eq!(ctx.global_style().spacing.interact_size.y, 34.);
    }
    #[test]
    fn editor_fits_narrow_columns_with_large_text() {
        for (density, width, scale) in [
            (Density::Compact, 240., 1.3),
            (Density::Touch, 320., 1.3),
            (Density::Touch, 360., 1.),
        ] {
            let ctx = Context::default();
            let mut config = ThemeConfig {
                text_scale: scale,
                ..Default::default()
            };
            config.apply(&ctx, density);
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1800.),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let right = ui.max_rect().right();
                        editor(ui, &mut config, density);
                        assert!(
                            ui.min_rect().right() <= right + 1.,
                            "{density:?} at {width} pt: {:?} exceeds {right}",
                            ui.min_rect()
                        );
                    },
                );
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn malformed_metrics_are_bounded_and_large_text_keeps_space() {
        let ctx = Context::default();
        let mut config = ThemeConfig {
            text_scale: f32::NAN,
            spacing_scale: f32::INFINITY,
            control_height: Some(-100.),
            ..Default::default()
        };
        config.apply(&ctx, Density::Touch);
        assert!(ctx.global_style().spacing.interact_size.y.is_finite());
        config.text_scale = 1.3;
        config.apply(&ctx, Density::Touch);
        let style = ctx.global_style();
        assert!(
            style.spacing.interact_size.y
                >= style.text_styles[&egui::TextStyle::Button].size * 1.25
                    + 2. * style.spacing.button_padding.y
        );
    }
}
