//! The presets that ship with the kit.

use eframe::egui::{Color32, Style};

use super::{
    PresetDef, galactic,
    neon::{self, NeonPalette},
    palette::{Palette, paint_palette},
    rerun,
    soft::{self, SoftPalette},
    tokyo,
};
use crate::{
    colors::{DEFAULT_ACCENT_SECONDARY, DEFAULT_SUCCESS},
    fonts::TextScale,
    glass::GlassParams,
    glass_material,
    metrics::Density,
    saved::SavedTheme,
    studio,
    theme_chrome::{
        classic_chrome, default_egui_chrome, graphite_theme, noir_chrome, noir_glass_chrome,
        noir_glass_params,
    },
};

pub(super) const CORE: &str = "Core";
pub(super) const GLASS: &str = "Glass";
pub(super) const NEON: &str = "Neon glass";
pub(super) const SOFT: &str = "Soft glass";
pub(super) const CLASSICS: &str = "Classics";

/// A theme at the default density and text scale.
fn theme(style: Style, glass: GlassParams) -> SavedTheme {
    SavedTheme {
        style,
        glass: Some(glass),
        density: Density::default(),
        text_scale: TextScale::default(),
        semantic: None,
    }
}

fn neon_theme(p: &NeonPalette) -> SavedTheme {
    theme(neon::neon_style(p), neon::neon_glass_params(p))
}

fn soft_theme(p: &SoftPalette) -> SavedTheme {
    theme(soft::soft_style(p), soft::soft_glass_params(p))
}

fn palette_theme(p: &Palette) -> SavedTheme {
    let mut style = classic_chrome();
    paint_palette(&mut style, p);
    theme(style, GlassParams::OFF)
}

const NOIR_SEMANTIC: (Color32, Color32) = (
    Color32::from_rgb(90, 220, 160),
    Color32::from_rgb(233, 130, 255),
);

/// Every built-in preset in picker order.
pub(super) fn defs() -> Vec<PresetDef> {
    vec![
        PresetDef {
            id: "graphite",
            name: "Graphite",
            group: CORE,
            description: "Near-black graphite surfaces with a crimson accent and frosted windows, compact by default.",
            build: graphite_theme,
            tokens: None,
            semantic_colors: || (DEFAULT_SUCCESS, DEFAULT_ACCENT_SECONDARY),
        },
        PresetDef {
            id: "studio",
            name: "Studio",
            group: CORE,
            description: "Opaque near-black surfaces, one fill per widget state and three accent roles.",
            build: || theme(studio::studio_style(), studio::studio_glass_params()),
            tokens: Some(|| studio::TOKENS),
            semantic_colors: studio::studio_semantic_colors,
        },
        PresetDef {
            id: "noir",
            name: "Noir",
            group: CORE,
            description: "OLED black with violet strokes, a crimson press, cyan warnings and rose errors.",
            build: || theme(noir_chrome(), GlassParams::OFF),
            tokens: None,
            semantic_colors: || NOIR_SEMANTIC,
        },
        PresetDef {
            id: "noir-glass",
            name: "Noir Glass",
            group: GLASS,
            description: "Noir as tinted glass: translucent panes over a wide backdrop blur.",
            build: || theme(noir_glass_chrome(), noir_glass_params()),
            tokens: None,
            semantic_colors: || NOIR_SEMANTIC,
        },
        PresetDef {
            id: "frost",
            name: "Frost",
            group: GLASS,
            description: "Violet tinted panes with magenta presses over a near-black base.",
            build: || {
                let style = glass_material::glass_style();
                let glass = glass_material::glass_params_for_style(&style);
                theme(style, glass)
            },
            tokens: None,
            semantic_colors: glass_material::semantic_colors,
        },
        PresetDef {
            id: "nebula",
            name: "Nebula",
            group: NEON,
            description: "Violet and magenta glass over deep space.",
            build: || neon_theme(&neon::NEBULA),
            tokens: None,
            semantic_colors: || neon::neon_semantic_colors(&neon::NEBULA),
        },
        PresetDef {
            id: "aurora",
            name: "Aurora",
            group: NEON,
            description: "Aqua and mint glass with violet edges.",
            build: || neon_theme(&neon::AURORA),
            tokens: None,
            semantic_colors: || neon::neon_semantic_colors(&neon::AURORA),
        },
        PresetDef {
            id: "supernova",
            name: "Supernova",
            group: NEON,
            description: "Hot pink glass over a magenta-black base.",
            build: || neon_theme(&neon::SUPERNOVA),
            tokens: None,
            semantic_colors: || neon::neon_semantic_colors(&neon::SUPERNOVA),
        },
        PresetDef {
            id: "event-horizon",
            name: "Event Horizon",
            group: NEON,
            description: "Electric blue and cyan glass over navy-black.",
            build: || neon_theme(&neon::EVENT_HORIZON),
            tokens: None,
            semantic_colors: || neon::neon_semantic_colors(&neon::EVENT_HORIZON),
        },
        PresetDef {
            id: "galactic",
            name: "Galactic",
            group: NEON,
            description: "Violet glass with aqua hover and hot pink press on an AMOLED page.",
            build: || {
                theme(
                    galactic::galactic_style(),
                    galactic::galactic_glass_params(),
                )
            },
            tokens: None,
            semantic_colors: galactic::galactic_semantic_colors,
        },
        PresetDef {
            id: "obsidian",
            name: "Obsidian",
            group: SOFT,
            description: "Graphite glass with muted violet and outlined controls; the quietest of the soft family.",
            build: || soft_theme(&soft::OBSIDIAN),
            tokens: None,
            semantic_colors: || soft::soft_semantic_colors(&soft::OBSIDIAN),
        },
        PresetDef {
            id: "velvet",
            name: "Velvet",
            group: SOFT,
            description: "Warm plum glass under mauve, pressing to rose.",
            build: || soft_theme(&soft::VELVET),
            tokens: None,
            semantic_colors: || soft::soft_semantic_colors(&soft::VELVET),
        },
        PresetDef {
            id: "twilight",
            name: "Twilight",
            group: SOFT,
            description: "Deep indigo glass under periwinkle, pressing to soft magenta.",
            build: || soft_theme(&soft::TWILIGHT),
            tokens: None,
            semantic_colors: || soft::soft_semantic_colors(&soft::TWILIGHT),
        },
        PresetDef {
            id: "quartz",
            name: "Quartz",
            group: SOFT,
            description: "Cool smoke glass with the lightest surfaces of the soft family, under lilac.",
            build: || soft_theme(&soft::QUARTZ),
            tokens: None,
            semantic_colors: || soft::soft_semantic_colors(&soft::QUARTZ),
        },
        PresetDef {
            id: "tokyo-night",
            name: "Tokyo Night",
            group: CLASSICS,
            description: "The Tokyo Night editor palette: slate surfaces with sky-blue accents.",
            build: || palette_theme(&tokyo::TOKYO_NIGHT),
            tokens: None,
            semantic_colors: || tokyo::TOKYO_NIGHT.semantic_colors(),
        },
        PresetDef {
            id: "tokyo-night-storm",
            name: "Tokyo Night Storm",
            group: CLASSICS,
            description: "The deeper indigo Storm variant of Tokyo Night.",
            build: || palette_theme(&tokyo::TOKYO_NIGHT_STORM),
            tokens: None,
            semantic_colors: || tokyo::TOKYO_NIGHT_STORM.semantic_colors(),
        },
        PresetDef {
            id: "rerun",
            name: "Rerun",
            group: CLASSICS,
            description: "Neutral grey surfaces under a pink accent, inspired by the Rerun viewer.",
            build: || palette_theme(&rerun::RERUN),
            tokens: None,
            semantic_colors: || rerun::RERUN.semantic_colors(),
        },
        PresetDef {
            id: "rerun-oled",
            name: "Rerun OLED",
            group: CLASSICS,
            description: "Rerun on true black for OLED panels.",
            build: || palette_theme(&rerun::RERUN_OLED),
            tokens: None,
            semantic_colors: || rerun::RERUN_OLED.semantic_colors(),
        },
        PresetDef {
            id: "default-egui",
            name: "Default egui",
            group: CLASSICS,
            description: "Stock egui dark visuals.",
            build: || theme(default_egui_chrome(), GlassParams::OFF),
            tokens: None,
            semantic_colors: || {
                (
                    Color32::from_rgb(42, 172, 170),
                    Color32::from_rgb(56, 114, 238),
                )
            },
        },
    ]
}
