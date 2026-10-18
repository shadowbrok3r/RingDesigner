//! Edits that map a colour picked for a token or role onto the style fields the token is derived from.

use eframe::egui::{Color32, Context, Style};

use crate::{
    presets,
    saved::SemanticColors,
    tokens::{Tokens, contrast},
};

/// Alpha given to a selection wash edited from a fully transparent one.
const MIN_WASH_ALPHA: u8 = 90;
/// Least alpha at which a selection wash still counts as the primary accent.
const ACCENT_ALPHA_FLOOR: u8 = 64;
/// How far below a new primary a link's chroma is held so the primary stays the most vivid accent.
const LINK_CHROMA_MARGIN: u8 = 4;

/// A surface or text token the editor changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Base,
    Sunken,
    Raised,
    Window,
    Field,
    FieldBorder,
    Hover,
    Pressed,
    Separator,
    CardBorder,
    Text,
    TextDim,
}

impl Surface {
    pub const ALL: [Self; 12] = [
        Self::Base,
        Self::Raised,
        Self::Window,
        Self::Sunken,
        Self::Field,
        Self::FieldBorder,
        Self::Hover,
        Self::Pressed,
        Self::Separator,
        Self::CardBorder,
        Self::Text,
        Self::TextDim,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Base => "Base",
            Self::Sunken => "Sunken",
            Self::Raised => "Raised",
            Self::Window => "Window",
            Self::Field => "Field",
            Self::FieldBorder => "Field border",
            Self::Hover => "Hover",
            Self::Pressed => "Pressed",
            Self::Separator => "Separator",
            Self::CardBorder => "Card border",
            Self::Text => "Text",
            Self::TextDim => "Dim text",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Base => "Panel background.",
            Self::Sunken => "Troughs, plots and scroll tracks.",
            Self::Raised => "Cards and grouped content.",
            Self::Window => "Windows, menus and popups; keeps its transparency.",
            Self::Field => "Text fields and resting controls.",
            Self::FieldBorder => "Outline of fields and resting controls.",
            Self::Hover => "Control fill under the pointer.",
            Self::Pressed => "Control fill while pressed.",
            Self::Separator => "Rules and group outlines.",
            Self::CardBorder => "Outline of windows and cards.",
            Self::Text => "Body text.",
            Self::TextDim => "Secondary text and hints.",
        }
    }

    /// The token this surface edits.
    pub fn read(self, t: &Tokens) -> Color32 {
        match self {
            Self::Base => t.base,
            Self::Sunken => t.sunken,
            Self::Raised => t.raised,
            Self::Window => t.window,
            Self::Field => t.field,
            Self::FieldBorder => t.field_border,
            Self::Hover => t.hover,
            Self::Pressed => t.pressed,
            Self::Separator => t.separator,
            Self::CardBorder => t.card_border,
            Self::Text => t.text,
            Self::TextDim => t.text_dim,
        }
    }

    /// Sets the style fields the token is derived from so it reads as `color`.
    pub fn apply(self, style: &mut Style, color: Color32) {
        let v = &mut style.visuals;
        match self {
            Self::Base => v.panel_fill = color,
            Self::Sunken => v.extreme_bg_color = color,
            Self::Raised => v.widgets.noninteractive.bg_fill = color,
            Self::Window => v.window_fill = with_alpha_of(v.window_fill, color),
            Self::Field => {
                v.text_edit_bg_color = Some(color);
                v.widgets.inactive.bg_fill = color;
            }
            Self::FieldBorder => set_outline(&mut v.widgets.inactive.bg_stroke, color),
            Self::Hover => {
                v.widgets.hovered.bg_fill = color;
                v.widgets.hovered.weak_bg_fill = color;
            }
            Self::Pressed => {
                v.widgets.active.bg_fill = color;
                v.widgets.active.weak_bg_fill = color;
            }
            Self::Separator => set_outline(&mut v.widgets.noninteractive.bg_stroke, color),
            Self::CardBorder => set_outline(&mut v.window_stroke, color),
            Self::Text => {
                v.widgets.noninteractive.fg_stroke.color = color;
                v.widgets.inactive.fg_stroke.color = color;
                if v.override_text_color.is_some() {
                    v.override_text_color = Some(color);
                }
            }
            Self::TextDim => v.weak_text_color = Some(color),
        }
    }
}

/// An accent or status role stored in the style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Primary,
    Tertiary,
    Warn,
    Error,
}

impl Role {
    pub const ALL: [Self; 4] = [Self::Primary, Self::Tertiary, Self::Warn, Self::Error];

    pub fn label(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Tertiary => "Tertiary",
            Self::Warn => "Warning",
            Self::Error => "Error",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Primary => {
                "Selection, focus and primary buttons; links are softened if they would outshine it."
            }
            Self::Tertiary => {
                "Links and informational accents; a link more vivid than the primary becomes the primary."
            }
            Self::Warn => "Warning text and badges.",
            Self::Error => "Error text and destructive actions.",
        }
    }

    /// The token this role edits.
    pub fn read(self, t: &Tokens) -> Color32 {
        match self {
            Self::Primary => t.primary,
            Self::Tertiary => t.tertiary,
            Self::Warn => t.warn,
            Self::Error => t.error,
        }
    }

    /// Sets the style fields the role is derived from so it reads as `color`.
    pub fn apply(self, style: &mut Style, color: Color32) {
        let v = &mut style.visuals;
        match self {
            Self::Primary => {
                let alpha = match v.selection.bg_fill.a() {
                    a if a >= ACCENT_ALPHA_FLOOR => a,
                    _ => MIN_WASH_ALPHA,
                };
                v.selection.bg_fill =
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
                v.selection.stroke.color = color;
                v.text_cursor.stroke.color = color;
                let limit = chroma(color).saturating_sub(LINK_CHROMA_MARGIN);
                v.hyperlink_color = limit_chroma(v.hyperlink_color, limit);
            }
            Self::Tertiary => v.hyperlink_color = color,
            Self::Warn => v.warn_fg_color = color,
            Self::Error => v.error_fg_color = color,
        }
    }
}

/// A colour that lives outside the style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Semantic {
    Secondary,
    Success,
}

impl Semantic {
    pub const ALL: [Self; 2] = [Self::Secondary, Self::Success];

    pub fn label(self) -> &'static str {
        match self {
            Self::Secondary => "Secondary",
            Self::Success => "Success",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Secondary => "Second accent for badges and charts.",
            Self::Success => "Success text and badges.",
        }
    }

    pub fn read(self, colors: &SemanticColors) -> Color32 {
        match self {
            Self::Secondary => colors.secondary,
            Self::Success => colors.success,
        }
    }

    pub fn apply(self, colors: &mut SemanticColors, color: Color32) {
        match self {
            Self::Secondary => colors.secondary = color,
            Self::Success => colors.success = color,
        }
    }
}

/// Max minus min channel of the opaque colour.
fn chroma(c: Color32) -> u8 {
    c.r().max(c.g()).max(c.b()) - c.r().min(c.g()).min(c.b())
}

/// `color` with its saturation reduced until its chroma is at most `limit`, keeping its hue and alpha.
fn limit_chroma(color: Color32, limit: u8) -> Color32 {
    let current = chroma(color);
    if current <= limit {
        return color;
    }
    let k = f32::from(limit) / f32::from(current);
    let mean = (f32::from(color.r()) + f32::from(color.g()) + f32::from(color.b())) / 3.0;
    let pull = |c: u8| (mean + (f32::from(c) - mean) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(pull(color.r()), pull(color.g()), pull(color.b()), color.a())
}

fn with_alpha_of(old: Color32, color: Color32) -> Color32 {
    let alpha = old.a();
    if alpha == 255 {
        return color;
    }
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn set_outline(stroke: &mut eframe::egui::Stroke, color: Color32) {
    stroke.color = color;
    stroke.width = stroke.width.max(1.0);
}

/// Reinstalls the live style after `edit`, as a custom theme keeping the semantic colours.
pub fn edit_style(ctx: &Context, edit: impl FnOnce(&mut Style)) {
    let mut style = (*ctx.global_style()).clone();
    edit(&mut style);
    presets::apply_custom_style(ctx, style, SemanticColors::current(ctx));
}

/// Re-styles the live theme as tinted glass from its own colours, with the matching blur material.
pub fn glassify_live(ctx: &Context) {
    let glassified = crate::glass_material::glassify(&ctx.global_style());
    crate::glass::set_params(
        ctx,
        crate::glass_material::glass_params_for_style(&glassified),
    );
    presets::apply_custom_style(ctx, glassified, SemanticColors::current(ctx));
}

/// Reinstalls the live style as a custom theme with `edit` applied to its semantic colours.
pub fn edit_semantic(ctx: &Context, edit: impl FnOnce(&mut SemanticColors)) {
    let mut semantic = SemanticColors::current(ctx);
    edit(&mut semantic);
    presets::apply_custom_style(ctx, (*ctx.global_style()).clone(), semantic);
}

/// A pair of colours that fall short of their contrast floor.
#[derive(Clone, Debug, PartialEq)]
pub struct ContrastWarning {
    pub message: String,
    pub ratio: f32,
    pub floor: f32,
}

/// Floors for body text, secondary text and the role colours.
const TEXT_FLOOR: f32 = 4.5;
const DIM_FLOOR: f32 = 3.0;
const ROLE_FLOOR: f32 = 3.0;

/// Text and role pairs of `t` that read poorly.
pub fn contrast_warnings(t: &Tokens) -> Vec<ContrastWarning> {
    let mut out = Vec::new();
    let mut check = |message: String, fg: Color32, bg: Color32, floor: f32| {
        let ratio = contrast(fg, bg);
        if ratio < floor {
            out.push(ContrastWarning {
                message,
                ratio,
                floor,
            });
        }
    };
    for (name, bg) in [
        ("base", t.base),
        ("raised", t.raised),
        ("window", t.window),
        ("field", t.field),
    ] {
        check(format!("Text on {name}"), t.text, bg, TEXT_FLOOR);
        check(format!("Dim text on {name}"), t.text_dim, bg, DIM_FLOOR);
    }
    check("Text on hover".to_owned(), t.text, t.hover, TEXT_FLOOR);
    check("Text on pressed".to_owned(), t.text, t.pressed, TEXT_FLOOR);
    check(
        "Button text on primary".to_owned(),
        t.on_primary,
        t.primary,
        TEXT_FLOOR,
    );
    for (name, color) in [
        ("Primary", t.primary),
        ("Success", t.success),
        ("Warning", t.warn),
        ("Error", t.error),
    ] {
        check(format!("{name} on base"), color, t.base, ROLE_FLOOR);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{PresetStyles, apply_preset};

    const PICK: Color32 = Color32::from_rgb(201, 43, 87);

    fn tokens_after(preset: PresetStyles, edit: impl FnOnce(&mut Style)) -> Tokens {
        let ctx = Context::default();
        apply_preset(&ctx, preset);
        let mut style = (*ctx.global_style()).clone();
        edit(&mut style);
        Tokens::derive(&style, Color32::GREEN, Color32::BLUE)
    }

    fn near(a: Color32, b: Color32, tolerance: u8) -> bool {
        [(a.r(), b.r()), (a.g(), b.g()), (a.b(), b.b())]
            .iter()
            .all(|(x, y)| x.abs_diff(*y) <= tolerance)
    }

    #[test]
    fn every_surface_edit_reads_back_on_opaque_presets() {
        for preset in [
            PresetStyles::Studio,
            PresetStyles::Noir,
            PresetStyles::DefaultEgui,
            PresetStyles::TokyoNight,
            PresetStyles::Rerun,
        ] {
            let ctx = Context::default();
            apply_preset(&ctx, preset);
            let window_opaque = ctx.global_style().visuals.window_fill.a() == 255;
            for surface in Surface::ALL {
                if surface == Surface::Window && !window_opaque {
                    continue;
                }
                let t = tokens_after(preset, |s| surface.apply(s, PICK));
                assert_eq!(
                    surface.read(&t),
                    PICK,
                    "{preset:?}: {} did not read back",
                    surface.label()
                );
            }
        }
    }

    #[test]
    fn surface_edits_change_the_token_on_glass_presets() {
        for preset in [
            PresetStyles::Frost,
            PresetStyles::Nebula,
            PresetStyles::Quartz,
        ] {
            let ctx = Context::default();
            apply_preset(&ctx, preset);
            let before = Tokens::get(&ctx);
            for surface in Surface::ALL {
                let t = tokens_after(preset, |s| surface.apply(s, PICK));
                assert_ne!(
                    surface.read(&t),
                    surface.read(&before),
                    "{preset:?}: {} did not change",
                    surface.label()
                );
            }
        }
    }

    #[test]
    fn a_window_edit_keeps_its_transparency() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Nebula);
        let mut style = (*ctx.global_style()).clone();
        let alpha = style.visuals.window_fill.a();
        assert!(alpha < 255);
        Surface::Window.apply(&mut style, PICK);
        assert_eq!(style.visuals.window_fill.a(), alpha);
    }

    #[test]
    fn role_edits_read_back() {
        for preset in [
            PresetStyles::Studio,
            PresetStyles::Noir,
            PresetStyles::TokyoNight,
        ] {
            for role in Role::ALL {
                let t = tokens_after(preset, |s| role.apply(s, PICK));
                assert!(
                    near(role.read(&t), PICK, 3),
                    "{preset:?}: {} read back as {:?}",
                    role.label(),
                    role.read(&t)
                );
            }
        }
    }

    #[test]
    fn a_new_primary_softens_a_link_that_would_outshine_it() {
        let muted = Color32::from_rgb(120, 110, 100);
        let t = tokens_after(PresetStyles::Studio, |s| Role::Primary.apply(s, muted));
        assert!(near(t.primary, muted, 3), "{:?}", t.primary);
        assert_eq!(chroma(t.tertiary).min(chroma(muted)), chroma(t.tertiary));

        let pale = Color32::from_rgb(200, 200, 200);
        assert_eq!(limit_chroma(pale, 10), Color32::from_rgb(200, 200, 200));
        let cyan = Color32::from_rgb(34, 211, 238);
        let softer = limit_chroma(cyan, 60);
        assert!(
            chroma(softer) <= 60 && softer.b() > softer.r(),
            "{softer:?}"
        );
        assert_eq!(limit_chroma(cyan, 255), cyan);
    }

    #[test]
    fn semantic_edits_change_only_their_colour() {
        let mut colors = SemanticColors {
            success: Color32::GREEN,
            secondary: Color32::BLUE,
        };
        Semantic::Success.apply(&mut colors, PICK);
        assert_eq!(Semantic::Success.read(&colors), PICK);
        assert_eq!(Semantic::Secondary.read(&colors), Color32::BLUE);
    }

    #[test]
    fn editing_the_live_theme_makes_it_custom_and_keeps_the_semantic_colours() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Aurora);
        let semantic = SemanticColors::current(&ctx);
        edit_style(&ctx, |s| Surface::Base.apply(s, PICK));
        assert_eq!(presets::installed_preset_id(&ctx), None);
        assert_eq!(SemanticColors::current(&ctx), semantic);
        assert_eq!(Tokens::get(&ctx).base, PICK);

        edit_semantic(&ctx, |c| Semantic::Success.apply(c, PICK));
        assert_eq!(Tokens::get(&ctx).success, PICK);
        assert_eq!(Tokens::get(&ctx).base, PICK);
    }

    #[test]
    fn glassifying_keeps_the_palette_and_turns_on_a_visible_material() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Rerun);
        let before = (*ctx.global_style()).clone();
        assert!(!crate::glass::params(&ctx).is_visible());

        glassify_live(&ctx);
        let after = ctx.global_style();
        assert_eq!(presets::installed_preset_id(&ctx), None);
        assert!(crate::glass::params(&ctx).is_visible());
        assert_eq!(after.visuals.panel_fill, before.visuals.panel_fill);
        assert!(after.visuals.widgets.hovered.weak_bg_fill.a() < 255);
        assert!(after.visuals.window_fill.a() < 255);
    }

    #[test]
    fn built_in_presets_raise_no_contrast_warnings_apart_from_stock_egui() {
        let ctx = Context::default();
        for def in crate::presets::builtin_presets() {
            let warnings = contrast_warnings(&crate::presets::preset_tokens(&ctx, def));
            if def.id == "default-egui" {
                assert!(!warnings.is_empty(), "stock egui is known to fall short");
            } else {
                assert!(warnings.is_empty(), "{}: {warnings:?}", def.name);
            }
        }
    }

    #[test]
    fn contrast_warnings_flag_unreadable_pairs_only() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Studio);
        assert!(contrast_warnings(&Tokens::get(&ctx)).is_empty());

        let mut bad = Tokens::get(&ctx);
        bad.text = bad.field;
        let warnings = contrast_warnings(&bad);
        assert!(warnings.iter().any(|w| w.message == "Text on field"));
        assert!(warnings.iter().all(|w| w.ratio < w.floor));
    }
}
