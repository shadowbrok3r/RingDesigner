//! Preset registry and the install path: applying a preset or a saved theme to a [`Context`].

pub mod neon;
pub mod palette;
pub mod soft;

mod builtin;
mod galactic;
mod legacy;
mod rerun;
mod tokyo;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use eframe::egui::{Color32, Context, Id, Style, Visuals};

use super::{
    colors::{DEFAULT_ACCENT_SECONDARY, DEFAULT_SUCCESS},
    fonts::{self, TextScale},
    glass::{self, GlassParams},
    metrics::{self, Density, Metrics, apply_metrics},
    persist,
    saved::{SavedTheme, SemanticColors},
    tokens::Tokens,
};

/// A preset: how to build its theme, and the parts of it that have no `Style` slot.
#[derive(Clone, Copy)]
pub struct PresetDef {
    /// Stable identifier stored in records; lowercase words joined by `-`.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Title of the picker group listing the preset.
    pub group: &'static str,
    /// One sentence for tooltips and cards.
    pub description: &'static str,
    /// Style before the metrics overlay, glass material, and default density and text scale.
    pub build: fn() -> SavedTheme,
    /// Hand-tuned tokens published instead of those derived from the style.
    pub tokens: Option<fn() -> Tokens>,
    /// Success and secondary-accent colours.
    pub semantic_colors: fn() -> (Color32, Color32),
}

impl std::fmt::Debug for PresetDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresetDef")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("group", &self.group)
            .finish()
    }
}

impl PartialEq for PresetDef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PresetDef {
    /// The preset's theme with its default density and text scale.
    pub fn theme(&self) -> SavedTheme {
        (self.build)()
    }

    /// The preset's style under the Regular metrics overlay.
    pub fn style(&self) -> Arc<Style> {
        cached_style(self)
    }

    /// The preset's glass material; off for opaque presets.
    pub fn glass(&self) -> GlassParams {
        self.theme().glass.unwrap_or(GlassParams::OFF)
    }

    /// The preset's success and secondary-accent colours.
    pub fn semantic(&self) -> SemanticColors {
        let (success, secondary) = (self.semantic_colors)();
        SemanticColors { success, secondary }
    }
}

/// The presets sharing one group title.
#[derive(Clone, Debug)]
pub struct PresetGroupDefs {
    pub title: &'static str,
    pub presets: Vec<PresetDef>,
}

static REGISTRY: LazyLock<RwLock<Vec<PresetDef>>> = LazyLock::new(|| RwLock::new(builtin::defs()));

static STYLES: LazyLock<Mutex<HashMap<&'static str, Arc<Style>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Every registered preset: the built-ins in picker order, then those added by [`register_preset`].
pub fn presets() -> Vec<PresetDef> {
    REGISTRY.read().map(|r| r.clone()).unwrap_or_default()
}

/// The presets that ship with the kit, whatever the registry holds now.
pub fn builtin_presets() -> Vec<PresetDef> {
    builtin::defs()
}

/// The preset with exactly this id.
pub fn preset(id: &str) -> Option<PresetDef> {
    REGISTRY.read().ok()?.iter().find(|d| d.id == id).copied()
}

/// The preset a label names: an id, a display name or a legacy name, compared case-insensitively.
pub fn find_preset(label: &str) -> Option<PresetDef> {
    let label = label.trim();
    let registry = REGISTRY.read().ok()?;
    registry
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(label))
        .or_else(|| registry.iter().find(|d| d.name.eq_ignore_ascii_case(label)))
        .copied()
        .or_else(|| {
            let id = legacy::alias(label)?;
            registry.iter().find(|d| d.id == id).copied()
        })
}

/// Adds `def` to the registry, replacing the preset with the same id; call at startup.
pub fn register_preset(def: PresetDef) {
    if let Ok(mut registry) = REGISTRY.write() {
        match registry.iter_mut().find(|d| d.id == def.id) {
            Some(slot) => *slot = def,
            None => registry.push(def),
        }
    }
    forget_style(def.id);
}

/// Removes a preset added by [`register_preset`], or restores the built-in it replaced.
pub fn unregister_preset(id: &str) -> bool {
    let builtin = builtin::defs().into_iter().find(|d| d.id == id);
    let Ok(mut registry) = REGISTRY.write() else {
        return false;
    };
    let Some(index) = registry.iter().position(|d| d.id == id) else {
        return false;
    };
    match builtin {
        Some(original) => registry[index] = original,
        None => {
            registry.remove(index);
        }
    }
    drop(registry);
    forget_style(id);
    true
}

fn forget_style(id: &str) {
    if let Ok(mut cache) = STYLES.lock() {
        cache.retain(|k, _| *k != id);
    }
}

/// The registered presets grouped by title, in first-seen order.
pub fn grouped_presets() -> Vec<PresetGroupDefs> {
    let mut groups: Vec<PresetGroupDefs> = Vec::new();
    for def in presets() {
        match groups.iter_mut().find(|g| g.title == def.group) {
            Some(group) => group.presets.push(def),
            None => groups.push(PresetGroupDefs {
                title: def.group,
                presets: vec![def],
            }),
        }
    }
    groups
}

/// The theme of the preset a label names, with its default density and text scale.
pub fn preset_theme(label: &str) -> Option<SavedTheme> {
    find_preset(label).map(|def| def.theme())
}

fn cached_style(def: &PresetDef) -> Arc<Style> {
    if let Ok(cache) = STYLES.lock()
        && let Some(style) = cache.get(def.id)
    {
        return Arc::clone(style);
    }
    let style = Arc::new(normalized(&def.theme().style));
    if let Ok(mut cache) = STYLES.lock() {
        cache.insert(def.id, Arc::clone(&style));
    }
    style
}

/// A preset named by a [`PresetStyles`] variant, an id or label, or a [`PresetDef`].
pub trait PresetRef {
    /// The registered preset this names.
    fn resolve(&self) -> Option<PresetDef>;

    /// Whether this names the blank custom preset.
    fn is_custom(&self) -> bool {
        false
    }
}

impl PresetRef for PresetDef {
    fn resolve(&self) -> Option<PresetDef> {
        preset(self.id).or(Some(*self))
    }
}

impl PresetRef for &PresetDef {
    fn resolve(&self) -> Option<PresetDef> {
        (**self).resolve()
    }
}

impl PresetRef for &str {
    fn resolve(&self) -> Option<PresetDef> {
        find_preset(self)
    }
}

impl PresetRef for String {
    fn resolve(&self) -> Option<PresetDef> {
        find_preset(self)
    }
}

impl PresetRef for &String {
    fn resolve(&self) -> Option<PresetDef> {
        find_preset(self)
    }
}

impl PresetRef for PresetStyles {
    fn resolve(&self) -> Option<PresetDef> {
        self.id().and_then(preset)
    }

    fn is_custom(&self) -> bool {
        *self == PresetStyles::Custom
    }
}

/// The built-in presets as an enum; presets added with [`register_preset`] read as `Custom` here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PresetStyles {
    #[default]
    Graphite,
    Studio,
    Noir,
    NoirGlass,
    Frost,
    Nebula,
    Aurora,
    Supernova,
    EventHorizon,
    Galactic,
    Obsidian,
    Velvet,
    Twilight,
    Quartz,
    TokyoNight,
    TokyoNightStorm,
    Rerun,
    RerunOled,
    DefaultEgui,
    Custom,
}

/// Each built-in variant with its registry id and display name.
const BUILTINS: [(PresetStyles, &str, &str); 19] = [
    (PresetStyles::Graphite, "graphite", "Graphite"),
    (PresetStyles::Studio, "studio", "Studio"),
    (PresetStyles::Noir, "noir", "Noir"),
    (PresetStyles::NoirGlass, "noir-glass", "Noir Glass"),
    (PresetStyles::Frost, "frost", "Frost"),
    (PresetStyles::Nebula, "nebula", "Nebula"),
    (PresetStyles::Aurora, "aurora", "Aurora"),
    (PresetStyles::Supernova, "supernova", "Supernova"),
    (PresetStyles::EventHorizon, "event-horizon", "Event Horizon"),
    (PresetStyles::Galactic, "galactic", "Galactic"),
    (PresetStyles::Obsidian, "obsidian", "Obsidian"),
    (PresetStyles::Velvet, "velvet", "Velvet"),
    (PresetStyles::Twilight, "twilight", "Twilight"),
    (PresetStyles::Quartz, "quartz", "Quartz"),
    (PresetStyles::TokyoNight, "tokyo-night", "Tokyo Night"),
    (
        PresetStyles::TokyoNightStorm,
        "tokyo-night-storm",
        "Tokyo Night Storm",
    ),
    (PresetStyles::Rerun, "rerun", "Rerun"),
    (PresetStyles::RerunOled, "rerun-oled", "Rerun OLED"),
    (PresetStyles::DefaultEgui, "default-egui", "Default egui"),
];

impl PresetStyles {
    /// The registry id; `None` for `Custom`.
    pub fn id(self) -> Option<&'static str> {
        BUILTINS.iter().find(|b| b.0 == self).map(|b| b.1)
    }

    /// The built-in with this registry id.
    pub fn from_id(id: &str) -> Option<Self> {
        BUILTINS.iter().find(|b| b.1 == id).map(|b| b.0)
    }

    /// The display name.
    pub fn as_str(&self) -> &'static str {
        BUILTINS
            .iter()
            .find(|b| b.0 == *self)
            .map_or("Custom", |b| b.2)
    }

    /// The built-in a label names, by id, display name or legacy name; unknown labels are `Custom`.
    pub fn from_label(label: &str) -> Self {
        find_preset(label)
            .and_then(|def| Self::from_id(def.id))
            .unwrap_or(Self::Custom)
    }

    /// The registered definition; `None` for `Custom`.
    pub fn def(self) -> Option<PresetDef> {
        self.resolve()
    }
}

/// One group of the built-in preset picker.
pub struct PresetGroup {
    pub title: &'static str,
    pub presets: &'static [PresetStyles],
}

/// The preset the picker lists above its groups.
pub const TOP_PRESET: PresetStyles = PresetStyles::Graphite;

/// Built-in picker groups in menu order, below [`TOP_PRESET`].
pub static PRESET_GROUPS: [PresetGroup; 5] = [
    PresetGroup {
        title: "Core",
        presets: &[PresetStyles::Studio, PresetStyles::Noir],
    },
    PresetGroup {
        title: "Glass",
        presets: &[PresetStyles::NoirGlass, PresetStyles::Frost],
    },
    PresetGroup {
        title: "Neon glass",
        presets: &[
            PresetStyles::Nebula,
            PresetStyles::Aurora,
            PresetStyles::Supernova,
            PresetStyles::EventHorizon,
            PresetStyles::Galactic,
        ],
    },
    PresetGroup {
        title: "Soft glass",
        presets: &[
            PresetStyles::Obsidian,
            PresetStyles::Velvet,
            PresetStyles::Twilight,
            PresetStyles::Quartz,
        ],
    },
    PresetGroup {
        title: "Classics",
        presets: &[
            PresetStyles::TokyoNight,
            PresetStyles::TokyoNightStorm,
            PresetStyles::Rerun,
            PresetStyles::RerunOled,
            PresetStyles::DefaultEgui,
        ],
    },
];

/// Every built-in a saved style can be recognised as.
pub const MATCHABLE_PRESETS: [PresetStyles; 19] = [
    PresetStyles::Graphite,
    PresetStyles::Studio,
    PresetStyles::Noir,
    PresetStyles::NoirGlass,
    PresetStyles::Frost,
    PresetStyles::Nebula,
    PresetStyles::Aurora,
    PresetStyles::Supernova,
    PresetStyles::EventHorizon,
    PresetStyles::Galactic,
    PresetStyles::Obsidian,
    PresetStyles::Velvet,
    PresetStyles::Twilight,
    PresetStyles::Quartz,
    PresetStyles::TokyoNight,
    PresetStyles::TokyoNightStorm,
    PresetStyles::Rerun,
    PresetStyles::RerunOled,
    PresetStyles::DefaultEgui,
];

/// The picker group listing `preset`; `None` for the top-level row.
pub fn group_of(preset: PresetStyles) -> Option<&'static PresetGroup> {
    PRESET_GROUPS.iter().find(|g| g.presets.contains(&preset))
}

/// Applies the app's default theme: its style, glass material, density and text scale.
pub fn bootstrap_startup_theme(ctx: &Context) {
    apply_saved_theme(ctx, persist::default_theme());
}

/// Installs `style` under the current density's metrics overlay and publishes its tokens.
pub fn install_style(ctx: &Context, style: Style) {
    metrics::install_overlaid_style(ctx, style);
    if let Some(tokens) = exact_tokens(&ctx.global_style(), SemanticColors::current(ctx)) {
        Tokens::publish(ctx, tokens);
    }
}

/// The tokens [`apply_preset`] publishes for `preset` at the current density; installs nothing.
pub fn preset_tokens(ctx: &Context, preset: impl PresetRef) -> Tokens {
    let def = resolve_or_default(&preset);
    let density = metrics::density(ctx);
    let id = preset_tokens_id(def.id, density);
    if let Some(tokens) = ctx.data(|d| d.get_temp::<Tokens>(id)) {
        return tokens;
    }
    let tokens = def.tokens.map(|f| f()).unwrap_or_else(|| {
        let (success, secondary) = (def.semantic_colors)();
        let m = Metrics::for_density(density);
        metrics::overlay((*def.style()).clone(), &m, success, secondary).1
    });
    ctx.data_mut(|d| d.insert_temp(id, tokens));
    tokens
}

fn preset_tokens_id(preset: &'static str, density: Density) -> Id {
    Id::unique(("egui_theme_kit.preset_tokens", preset, density))
}

/// The hand-tuned tokens of the preset `style` matches, if it defines any and `semantic` is still its own.
fn exact_tokens(style: &Style, semantic: SemanticColors) -> Option<Tokens> {
    let style = normalized(style);
    presets()
        .into_iter()
        .filter_map(|def| def.tokens.map(|t| (def, t)))
        .find(|(def, _)| styles_visually_equal(&def.style(), &style))
        .filter(|(def, _)| def.semantic() == semantic)
        .map(|(_, tokens)| tokens())
}

/// Switches the density and re-installs the current style under it.
pub fn set_density(ctx: &Context, density: Density) {
    if metrics::density(ctx) == density {
        return;
    }
    metrics::set_density(ctx, density);
    install_style(ctx, (*ctx.global_style()).clone());
}

/// Switches the UI text size, rebuilding the font atlas when it changes.
pub fn set_text_scale(ctx: &Context, scale: TextScale) {
    fonts::apply_scale(ctx, scale);
}

/// Applies a saved theme: its style, its preset's semantic colours when recognised, and its glass material.
pub fn apply_saved_theme(ctx: &Context, theme: SavedTheme) {
    let normal = normalized(&theme.style);
    let (def, style, glass) = match (exact_preset(&normal), superseded_preset(&normal)) {
        (Some(def), _) => (Some(def), theme.style, theme.glass),
        (None, Some(def)) => (
            Some(def),
            (*def.style()).clone(),
            theme.glass.filter(|g| *g != GlassParams::OFF),
        ),
        (None, None) => (None, theme.style, theme.glass),
    };
    metrics::set_density(ctx, theme.density);
    apply_preset_semantics(ctx, def);
    if let Some(semantic) = theme.semantic {
        semantic.apply(ctx);
    }
    install_style(ctx, style);
    set_text_scale(ctx, theme.text_scale);
    if let Some(glass) = glass {
        glass::set_params(ctx, glass);
    }
}

/// Installs `style` as a custom theme with `semantic` colours, keeping the glass material and density.
pub fn apply_custom_style(ctx: &Context, style: Style, semantic: SemanticColors) {
    set_installed_id(ctx, None);
    semantic.apply(ctx);
    install_style(ctx, style);
}

/// Clears egui memory, then re-installs the current theme, density and text size into it.
pub fn reset_memory(ctx: &Context) {
    let saved = SavedTheme::current(ctx);
    ctx.memory_mut(|mem| *mem = Default::default());
    apply_saved_theme(ctx, saved);
}

/// Records the preset, publishes its semantic colours and sets its glass material.
fn apply_preset_semantics(ctx: &Context, def: Option<PresetDef>) {
    set_installed_id(ctx, def.map(|d| d.id));
    semantics_of(def).apply(ctx);
    glass::set_params(ctx, def.map_or(GlassParams::OFF, |d| d.glass()));
}

fn semantics_of(def: Option<PresetDef>) -> SemanticColors {
    def.map(|d| d.semantic()).unwrap_or(SemanticColors {
        success: DEFAULT_SUCCESS,
        secondary: DEFAULT_ACCENT_SECONDARY,
    })
}

/// The semantic colours the installed preset publishes; the defaults for a custom theme.
pub fn installed_semantics(ctx: &Context) -> SemanticColors {
    semantics_of(installed_def(ctx))
}

fn installed_id_key() -> Id {
    Id::unique("egui_theme_kit.preset.installed")
}

fn set_installed_id(ctx: &Context, id: Option<&'static str>) {
    ctx.data_mut(|d| d.insert_temp(installed_id_key(), id));
}

/// The id of the preset the installed style was applied as; `None` for a custom theme.
pub fn installed_preset_id(ctx: &Context) -> Option<&'static str> {
    ctx.data(|d| d.get_temp::<Option<&'static str>>(installed_id_key()))
        .flatten()
}

/// The preset the installed style was applied as.
pub fn installed_def(ctx: &Context) -> Option<PresetDef> {
    installed_preset_id(ctx).and_then(preset)
}

/// The built-in the installed style was applied as; `Custom` for a custom theme or a registered preset.
pub fn installed_preset(ctx: &Context) -> PresetStyles {
    installed_preset_id(ctx)
        .and_then(PresetStyles::from_id)
        .unwrap_or(PresetStyles::Custom)
}

/// Records `preset` as the one the installed style was applied as.
pub fn set_installed_preset(ctx: &Context, preset: impl PresetRef) {
    set_installed_id(ctx, preset.resolve().map(|d| d.id));
}

/// Compares styles ignoring `number_formatter`, whose PartialEq is Arc pointer identity.
pub fn styles_visually_equal(a: &Style, b: &Style) -> bool {
    let mut a = a.clone();
    a.number_formatter = b.number_formatter.clone();
    a == *b
}

/// Whether two saved themes are equal in every part, ignoring `number_formatter` identity.
pub fn themes_equal(a: &SavedTheme, b: &SavedTheme) -> bool {
    a.density == b.density
        && a.text_scale == b.text_scale
        && a.glass == b.glass
        && a.semantic == b.semantic
        && styles_visually_equal(&normalized(&a.style), &normalized(&b.style))
}

/// `style` under the Regular metrics overlay, the form presets are compared in.
pub fn normalized(style: &Style) -> Style {
    let mut style = style.clone();
    apply_metrics(&mut style, &Metrics::for_density(Density::Regular));
    style
}

/// The preset a normalized style equals.
pub fn exact_preset(normal: &Style) -> Option<PresetDef> {
    presets()
        .into_iter()
        .find(|def| styles_visually_equal(&def.style(), normal))
}

/// The preset a normalized style was saved as before that preset's colours changed.
fn superseded_preset(normal: &Style) -> Option<PresetDef> {
    legacy::superseded_styles()
        .into_iter()
        .find(|(style, _)| styles_visually_equal(&normalized(style), normal))
        .and_then(|(_, id)| preset(id))
}

/// Whether `style` is a blank egui default, dark or light.
pub fn is_blank_default_style(style: &Style) -> bool {
    let style = normalized(style);
    let dark_default = Style {
        visuals: Visuals::dark(),
        ..Style::default()
    };
    [Style::default(), dark_default]
        .iter()
        .any(|blank| styles_visually_equal(&style, &normalized(blank)))
}

/// The preset `target` names, or Graphite for the custom and unknown ones.
fn resolve_or_default(target: &impl PresetRef) -> PresetDef {
    target
        .resolve()
        .or_else(|| preset("graphite"))
        .expect("the graphite preset is built in")
}

/// A preset's style under the Regular metrics overlay; Graphite for custom and unknown presets.
pub fn style_for_preset(preset: impl PresetRef) -> Style {
    (*resolve_or_default(&preset).style()).clone()
}

/// A preset's success and secondary-accent colours; the defaults for custom and unknown presets.
pub fn semantic_colors_for_preset(preset: impl PresetRef) -> (Color32, Color32) {
    let s = semantics_of(preset.resolve());
    (s.success, s.secondary)
}

/// The backdrop-blur material a preset draws its glass against; off for opaque and custom presets.
pub fn glass_params_for_preset(preset: impl PresetRef) -> GlassParams {
    preset.resolve().map_or(GlassParams::OFF, |d| d.glass())
}

/// Installs a preset's style, semantic colours and glass, keeping density and text scale; `false` for an unknown preset.
pub fn apply_preset(ctx: &Context, preset: impl PresetRef) -> bool {
    let def = preset.resolve();
    if def.is_none() && !preset.is_custom() {
        log::warn!("apply_preset: unknown preset");
        return false;
    }
    apply_preset_semantics(ctx, def);
    install_style(ctx, style_for_preset(preset));
    true
}

/// Applies a preset as [`PresetDef::theme`] defines it, including density and text scale; `false` for an unknown preset.
pub fn apply_preset_theme(ctx: &Context, preset: impl PresetRef) -> bool {
    match preset.resolve() {
        Some(def) => {
            apply_saved_theme(ctx, def.theme());
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        colors,
        saved::{theme_from_json, theme_to_json},
        theme_chrome::{graphite_theme, noir_glass_params},
        tokens::{composite, contrast},
    };
    use super::*;

    fn saved(def: PresetDef) -> SavedTheme {
        SavedTheme {
            style: (*def.style()).clone(),
            glass: Some(def.glass()),
            density: Density::default(),
            text_scale: TextScale::default(),
            semantic: None,
        }
    }

    fn all_presets() -> Vec<PresetDef> {
        builtin_presets()
    }

    fn round_trip(theme: &SavedTheme) -> SavedTheme {
        theme_from_json(&theme_to_json(theme).unwrap()).unwrap()
    }

    #[test]
    fn registry_ids_are_unique_and_every_preset_builds() {
        let defs = presets();
        let mut ids: Vec<&str> = defs.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate preset ids");
        for def in defs {
            assert!(!def.id.is_empty() && !def.name.is_empty() && !def.group.is_empty());
            assert!(
                def.id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{}: id is not kebab-case",
                def.id
            );
            assert!(def.description.ends_with('.'), "{}: description", def.id);
            assert!(def.theme().glass.is_some(), "{}: no glass record", def.id);
            let (success, secondary) = (def.semantic_colors)();
            assert_eq!(success.a(), 255, "{}: success", def.id);
            assert_eq!(secondary.a(), 255, "{}: secondary", def.id);
        }
    }

    #[test]
    fn the_enum_and_the_registry_agree() {
        for p in MATCHABLE_PRESETS {
            let def = p.def().unwrap_or_else(|| panic!("{p:?} is not registered"));
            assert_eq!(def.name, p.as_str());
            assert_eq!(PresetStyles::from_id(def.id), Some(p));
            assert_eq!(PresetStyles::from_label(def.name), p);
            assert_eq!(PresetStyles::from_label(def.id), p);
        }
        assert_eq!(PresetStyles::Custom.id(), None);
        assert_eq!(PresetStyles::Custom.as_str(), "Custom");
        assert_eq!(
            PresetStyles::from_label("no such preset"),
            PresetStyles::Custom
        );
        for group in &PRESET_GROUPS {
            for p in group.presets {
                assert_eq!(p.def().unwrap().group, group.title, "{p:?}");
                assert_eq!(group_of(*p).map(|g| g.title), Some(group.title));
            }
        }
        let listed: usize = PRESET_GROUPS.iter().map(|g| g.presets.len()).sum();
        assert_eq!(listed + 1, MATCHABLE_PRESETS.len());
        assert!(group_of(TOP_PRESET).is_none());
    }

    #[test]
    fn every_preset_round_trips_as_itself_under_the_overlay() {
        for def in all_presets() {
            let decoded = round_trip(&saved(def));
            assert_eq!(
                exact_preset(&normalized(&decoded.style)).map(|d| d.id),
                Some(def.id),
                "{} did not round trip",
                def.name,
            );
        }
    }

    #[test]
    fn every_preset_shares_the_overlay_geometry() {
        let regular = Metrics::for_density(Density::Regular);
        for def in all_presets() {
            let style = def.style();
            let name = def.name;
            assert_eq!(
                style.spacing.interact_size.y, regular.control_h,
                "{name}: control height"
            );
            assert_eq!(style.spacing.button_padding, regular.pad, "{name}: padding");
            assert!(style.override_font_id.is_none(), "{name}: font override");

            let w = &style.visuals.widgets;
            let radius = w.inactive.corner_radius;
            for state in [
                &w.noninteractive,
                &w.inactive,
                &w.hovered,
                &w.active,
                &w.open,
            ] {
                for width in [state.bg_stroke.width, state.fg_stroke.width] {
                    assert_eq!(width, width.round(), "{name}: fractional stroke {width}");
                }
                assert_eq!(state.expansion, 0.0, "{name}: widget grows");
                assert_eq!(
                    state.corner_radius, radius,
                    "{name}: states disagree on radius"
                );
            }
        }
    }

    #[test]
    fn presets_stay_distinct() {
        let defs = all_presets();
        for (i, a) in defs.iter().enumerate() {
            for b in &defs[i + 1..] {
                assert!(
                    !styles_visually_equal(&a.style(), &b.style()),
                    "{} and {} produce the same style",
                    a.name,
                    b.name,
                );
            }
        }
    }

    #[test]
    fn presets_stay_distinct_in_their_tokens() {
        let ctx = Context::default();
        let defs = all_presets();
        for (i, a) in defs.iter().enumerate() {
            for b in &defs[i + 1..] {
                assert_ne!(
                    preset_tokens(&ctx, a),
                    preset_tokens(&ctx, b),
                    "{} and {} publish the same tokens",
                    a.name,
                    b.name,
                );
            }
        }
    }

    #[test]
    fn every_preset_has_legible_text_and_roles() {
        let ctx = Context::default();
        for def in all_presets() {
            let t = preset_tokens(&ctx, def);
            let name = def.name;
            for (slot, bg) in [
                ("base", t.base),
                ("raised", t.raised),
                ("field", t.field),
                ("window", t.window),
            ] {
                let ratio = contrast(t.text, bg);
                assert!(ratio >= 4.5, "{name}: text on {slot} is {ratio:.2}");
                let dim = contrast(t.text_dim, bg);
                let dim_floor = if def.id == "default-egui" { 2.5 } else { 3.9 };
                assert!(dim >= dim_floor, "{name}: text_dim on {slot} is {dim:.2}");
            }
            for (role, c) in [
                ("primary", t.primary),
                ("success", t.success),
                ("warn", t.warn),
                ("error", t.error),
            ] {
                let ratio = contrast(c, t.base);
                assert!(ratio >= 3.0, "{name}: {role} on base is {ratio:.2}");
            }
            let on_primary = contrast(t.on_primary, t.primary);
            assert!(on_primary >= 4.5, "{name}: on_primary is {on_primary:.2}");
            assert_ne!(t.hover, t.field, "{name}: hover matches the resting field");
        }
    }

    #[test]
    fn every_preset_draws_visible_edges() {
        let ctx = Context::default();
        for def in all_presets() {
            let t = preset_tokens(&ctx, def);
            let border = contrast(t.field_border, t.field);
            assert!(border >= 1.35, "{}: field border is {border:.2}", def.name);
            let rule = contrast(t.separator, t.base);
            assert!(rule >= 1.1, "{}: separator is {rule:.2}", def.name);
        }
    }

    #[test]
    fn bootstrap_applies_the_embedded_graphite_file() {
        let ctx = Context::default();
        bootstrap_startup_theme(&ctx);
        assert_eq!(installed_preset(&ctx), PresetStyles::Graphite);
        assert_eq!(metrics::density(&ctx), Density::Compact);
        assert_eq!(fonts::text_scale(&ctx), TextScale::Small);
        assert_eq!(Metrics::get(&ctx), Metrics::for_density(Density::Compact));
        let file = graphite_theme();
        assert_eq!(glass::params(&ctx), file.glass.unwrap());
        assert!(glass::params(&ctx).is_visible());
        let style = ctx.global_style();
        let compact = Metrics::for_density(Density::Compact);
        assert_eq!(style.spacing.interact_size.y, compact.control_h);
        assert_eq!(style.spacing.item_spacing, compact.gap);
        assert_eq!(style.spacing.button_padding, compact.pad);
        for (text_style, size) in [
            (eframe::egui::TextStyle::Small, 10.5),
            (eframe::egui::TextStyle::Body, 12.5),
            (eframe::egui::TextStyle::Button, 12.5),
            (eframe::egui::TextStyle::Monospace, 12.0),
            (eframe::egui::TextStyle::Heading, 15.0),
        ] {
            assert_eq!(style.text_styles[&text_style].size, size, "{text_style:?}");
        }
    }

    #[test]
    fn the_graphite_preset_reproduces_the_file_style_under_its_own_density() {
        let file = graphite_theme();
        let ctx = Context::default();
        apply_saved_theme(&ctx, file.clone());
        let mut live = (*ctx.global_style()).clone();
        live.number_formatter = file.style.number_formatter.clone();
        let mut expected = file.style.clone();
        apply_metrics(&mut expected, &Metrics::for_density(Density::Compact));
        assert!(
            live == expected,
            "the installed style drifted from the file"
        );
    }

    #[test]
    fn the_file_semantic_colors_are_the_app_wide_defaults() {
        let ctx = Context::default();
        bootstrap_startup_theme(&ctx);
        assert_eq!(colors::success_ctx(&ctx), DEFAULT_SUCCESS);
        assert_eq!(colors::accent_secondary_ctx(&ctx), DEFAULT_ACCENT_SECONDARY);
    }

    #[test]
    fn density_and_text_scale_round_trip_through_a_saved_theme() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Frost);
        set_density(&ctx, Density::Comfortable);
        set_text_scale(&ctx, TextScale::XLarge);
        let decoded = round_trip(&SavedTheme::current(&ctx));
        assert_eq!(decoded.density, Density::Comfortable);
        assert_eq!(decoded.text_scale, TextScale::XLarge);

        let fresh = Context::default();
        apply_saved_theme(&fresh, decoded);
        assert_eq!(metrics::density(&fresh), Density::Comfortable);
        assert_eq!(fonts::text_scale(&fresh), TextScale::XLarge);
        assert_eq!(installed_preset(&fresh), PresetStyles::Frost);
        assert_eq!(
            fresh.global_style().spacing.interact_size.y,
            Metrics::for_density(Density::Comfortable).control_h
        );
    }

    #[test]
    fn switching_density_reinstalls_the_current_style() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Frost);
        let fill = ctx.global_style().visuals.widgets.inactive.weak_bg_fill;

        set_density(&ctx, Density::Comfortable);
        let style = ctx.global_style();
        assert_eq!(
            style.spacing.interact_size.y,
            Metrics::for_density(Density::Comfortable).control_h
        );
        assert_eq!(style.visuals.widgets.inactive.weak_bg_fill, fill);
    }

    #[test]
    fn a_blank_default_style_is_recognised() {
        let legacy = Style {
            visuals: Visuals::dark(),
            ..Style::default()
        };
        assert!(is_blank_default_style(&legacy));
        assert!(is_blank_default_style(&normalized(&legacy)));
        assert!(!is_blank_default_style(&style_for_preset(
            PresetStyles::Graphite
        )));
    }

    #[test]
    fn a_hand_tuned_material_survives_a_save() {
        let tuned = GlassParams {
            enabled: true,
            blur_radius: 41.0,
            tint: Color32::from_rgba_unmultiplied(9, 30, 44, 96),
            corner_radius: 13.0,
            presence: 0.72,
        };
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Frost);
        glass::set_params(&ctx, tuned);
        let saved = round_trip(&SavedTheme::current(&ctx));

        let fresh = Context::default();
        bootstrap_startup_theme(&fresh);
        apply_saved_theme(&fresh, saved);
        assert_eq!(glass::params(&fresh), tuned);
    }

    #[test]
    fn a_bare_style_record_takes_its_presets_material() {
        let style = style_for_preset(PresetStyles::NoirGlass);
        let theme = theme_from_json(&serde_json::to_vec(&style).unwrap()).unwrap();
        assert!(theme.glass.is_none());
        let ctx = Context::default();
        apply_saved_theme(&ctx, theme);
        assert_eq!(installed_preset(&ctx), PresetStyles::NoirGlass);
        assert_eq!(glass::params(&ctx), noir_glass_params());
    }

    #[test]
    fn applying_a_preset_carries_its_glass_material() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::NoirGlass);
        let params = glass::params(&ctx);
        assert_eq!(params, noir_glass_params());
        assert!(params.is_visible());

        apply_preset(&ctx, PresetStyles::Noir);
        assert_eq!(glass::params(&ctx), GlassParams::OFF);
    }

    #[test]
    fn studio_publishes_its_exact_tokens_on_every_install_path() {
        use super::super::studio;
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Studio);
        assert_eq!(Tokens::get(&ctx), studio::TOKENS);
        assert_eq!(glass::params(&ctx), studio::studio_glass_params());

        set_density(&ctx, Density::Compact);
        assert_eq!(Tokens::get(&ctx), studio::TOKENS);

        let decoded = round_trip(&SavedTheme::current(&ctx));
        assert_eq!(decoded.density, Density::Compact);
        let fresh = Context::default();
        bootstrap_startup_theme(&fresh);
        assert_ne!(Tokens::get(&fresh), studio::TOKENS);
        apply_saved_theme(&fresh, decoded);
        assert_eq!(Tokens::get(&fresh), studio::TOKENS);
    }

    #[test]
    fn a_semantic_edit_on_a_hand_tuned_preset_reaches_the_published_tokens() {
        use super::super::studio;
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Studio);
        assert_eq!(Tokens::get(&ctx), studio::TOKENS);

        let picked = SemanticColors {
            success: Color32::from_rgb(200, 200, 20),
            secondary: studio::TOKENS.secondary,
        };
        apply_custom_style(&ctx, (*ctx.global_style()).clone(), picked);
        let tokens = Tokens::get(&ctx);
        assert_eq!(tokens.success, picked.success);
        assert_ne!(tokens, studio::TOKENS);

        apply_preset(&ctx, PresetStyles::Studio);
        assert_eq!(Tokens::get(&ctx), studio::TOKENS);
    }

    #[test]
    fn leaving_studio_publishes_derived_tokens() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Studio);
        apply_preset(&ctx, PresetStyles::Graphite);
        let (success, accent2) = semantic_colors_for_preset(PresetStyles::Graphite);
        assert_eq!(
            Tokens::get(&ctx),
            Tokens::derive(&ctx.global_style(), success, accent2)
        );
    }

    #[test]
    fn every_preset_publishes_opaque_tokens_with_its_semantic_colors() {
        let ctx = Context::default();
        for def in all_presets() {
            apply_preset(&ctx, def);
            let tokens = Tokens::get(&ctx);
            let name = def.name;
            for (slot, c) in tokens.colors() {
                assert_eq!(c.a(), 255, "{name}: {slot} is translucent");
            }
            let (success, accent2) = (def.semantic_colors)();
            assert_eq!(
                tokens.success,
                composite(success, tokens.base),
                "{name}: success"
            );
            assert_eq!(
                tokens.secondary,
                composite(accent2, tokens.base),
                "{name}: secondary"
            );
        }
    }

    #[test]
    fn preset_tokens_match_what_applying_the_preset_publishes() {
        let ctx = Context::default();
        for def in all_presets() {
            apply_preset(&ctx, def);
            assert_eq!(preset_tokens(&ctx, def), Tokens::get(&ctx), "{}", def.name);
        }
    }

    #[test]
    fn saved_themes_never_name_the_weight_families() {
        let ctx = Context::default();
        for def in all_presets() {
            apply_preset(&ctx, def);
            let current = SavedTheme::current(&ctx);
            for style in [&current.style, &round_trip(&current).style] {
                for (text_style, font) in &style.text_styles {
                    assert!(
                        !matches!(font.family, eframe::egui::FontFamily::Name(_)),
                        "{}: {text_style:?} saved as {:?}",
                        def.name,
                        font.family,
                    );
                }
            }
        }
    }

    #[test]
    fn resetting_memory_keeps_the_theme_density_and_text_size() {
        use super::super::studio;
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Studio);
        set_density(&ctx, Density::Compact);
        set_text_scale(&ctx, TextScale::Large);

        reset_memory(&ctx);

        assert_eq!(installed_preset(&ctx), PresetStyles::Studio);
        assert_eq!(Metrics::get(&ctx), Metrics::for_density(Density::Compact));
        assert_eq!(fonts::text_scale(&ctx), TextScale::Large);
        assert_eq!(Tokens::get(&ctx), studio::TOKENS);
        assert_eq!(glass::params(&ctx), studio::studio_glass_params());
    }

    #[test]
    fn an_edited_style_is_custom_with_the_default_semantics() {
        let ctx = Context::default();
        let mut theme = graphite_theme();
        theme.style.visuals.hyperlink_color = Color32::from_rgb(1, 2, 3);
        apply_saved_theme(&ctx, theme);
        assert_eq!(installed_preset(&ctx), PresetStyles::Custom);
        assert_eq!(installed_preset_id(&ctx), None);
        assert_eq!(colors::success_ctx(&ctx), DEFAULT_SUCCESS);
    }

    #[test]
    fn custom_semantic_colors_survive_a_save() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Nebula);
        let preset_colors = installed_semantics(&ctx);
        assert_eq!(SavedTheme::current(&ctx).semantic, None);

        let picked = SemanticColors {
            success: Color32::from_rgb(10, 200, 30),
            secondary: Color32::from_rgb(200, 10, 90),
        };
        picked.apply(&ctx);
        let saved = SavedTheme::current(&ctx);
        assert_eq!(saved.semantic, Some(picked));
        assert_ne!(picked, preset_colors);

        let fresh = Context::default();
        apply_saved_theme(&fresh, round_trip(&saved));
        assert_eq!(installed_preset(&fresh), PresetStyles::Nebula);
        assert_eq!(SemanticColors::current(&fresh), picked);
    }

    #[test]
    fn an_edited_preset_keeps_its_semantic_colors_through_a_save() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Aurora);
        let semantic = SemanticColors::current(&ctx);
        let mut style = (*ctx.global_style()).clone();
        style.visuals.hyperlink_color = Color32::from_rgb(4, 5, 6);
        apply_custom_style(&ctx, style, semantic);
        assert_eq!(installed_preset(&ctx), PresetStyles::Custom);

        let saved = round_trip(&SavedTheme::current(&ctx));
        assert_eq!(saved.semantic, Some(semantic));
        let fresh = Context::default();
        apply_saved_theme(&fresh, saved);
        assert_eq!(SemanticColors::current(&fresh), semantic);
    }

    #[test]
    fn an_old_glass_record_decodes_as_frost_with_the_current_style() {
        let theme = SavedTheme {
            style: super::super::glass_material::legacy_style(),
            glass: Some(GlassParams::OFF),
            density: Density::Regular,
            text_scale: TextScale::Normal,
            semantic: None,
        };
        let ctx = Context::default();
        apply_saved_theme(&ctx, theme);
        assert_eq!(installed_preset(&ctx), PresetStyles::Frost);
        assert!(styles_visually_equal(
            &normalized(&ctx.global_style()),
            &style_for_preset(PresetStyles::Frost)
        ));
        assert!(glass::params(&ctx).is_visible());
    }

    #[test]
    fn applying_by_id_or_enum_is_the_same_and_unknown_ids_change_nothing() {
        let ctx = Context::default();
        assert!(apply_preset(&ctx, "tokyo-night"));
        let by_id = Tokens::get(&ctx);
        assert!(apply_preset(&ctx, PresetStyles::Noir));
        assert!(apply_preset(&ctx, PresetStyles::TokyoNight));
        assert_eq!(Tokens::get(&ctx), by_id);
        assert_eq!(installed_preset_id(&ctx), Some("tokyo-night"));

        assert!(!apply_preset(&ctx, "no-such-preset"));
        assert_eq!(installed_preset_id(&ctx), Some("tokyo-night"));
        assert!(apply_preset(&ctx, "Nebula Glass"));
        assert_eq!(installed_preset(&ctx), PresetStyles::Nebula);
    }

    #[test]
    fn applying_a_preset_theme_takes_its_density_and_text_scale() {
        let ctx = Context::default();
        apply_preset(&ctx, PresetStyles::Noir);
        set_density(&ctx, Density::Comfortable);
        assert!(apply_preset_theme(&ctx, "graphite"));
        assert_eq!(metrics::density(&ctx), Density::Compact);
        assert_eq!(fonts::text_scale(&ctx), TextScale::Small);
        assert!(!apply_preset_theme(&ctx, "no-such-preset"));
    }

    #[test]
    fn registered_presets_are_listed_applied_recognised_and_removable() {
        fn tweaked() -> SavedTheme {
            let mut theme = PresetStyles::Noir.def().unwrap().theme();
            theme.style.visuals.hyperlink_color = Color32::from_rgb(12, 240, 77);
            theme
        }
        let def = PresetDef {
            id: "test-registered",
            name: "Test Registered",
            group: "Test Group",
            description: "A preset added at runtime.",
            build: tweaked,
            tokens: None,
            semantic_colors: || (Color32::from_rgb(1, 200, 1), Color32::from_rgb(200, 1, 200)),
        };
        register_preset(def);
        assert!(presets().iter().any(|d| d.id == "test-registered"));
        assert_eq!(
            find_preset("Test Registered").map(|d| d.id),
            Some("test-registered")
        );
        assert!(
            grouped_presets()
                .iter()
                .any(|g| g.title == "Test Group" && g.presets.len() == 1)
        );

        let ctx = Context::default();
        assert!(apply_preset(&ctx, "test-registered"));
        assert_eq!(installed_preset_id(&ctx), Some("test-registered"));
        assert_eq!(installed_preset(&ctx), PresetStyles::Custom);
        assert_eq!(colors::success_ctx(&ctx), Color32::from_rgb(1, 200, 1));
        let recognised = exact_preset(&normalized(&ctx.global_style()));
        assert_eq!(recognised.map(|d| d.id), Some("test-registered"));

        assert!(unregister_preset("test-registered"));
        assert!(preset("test-registered").is_none());
        assert!(!unregister_preset("test-registered"));
    }

    #[test]
    fn registering_over_a_built_in_replaces_it_until_unregistered() {
        let original = preset("quartz").unwrap();
        let replaced = PresetDef {
            description: "Replaced.",
            ..original
        };
        register_preset(replaced);
        assert_eq!(preset("quartz").unwrap().description, "Replaced.");
        assert!(unregister_preset("quartz"));
        assert_eq!(preset("quartz").unwrap().description, original.description);
    }

    #[test]
    fn built_in_groups_follow_the_registry_order() {
        let groups = grouped_presets();
        let titles: Vec<&str> = groups.iter().map(|g| g.title).collect();
        for expected in ["Core", "Glass", "Neon glass", "Soft glass", "Classics"] {
            assert!(titles.contains(&expected), "missing group {expected}");
        }
        assert_eq!(groups[0].presets[0].id, "graphite");
    }
}
