//! Bundled Inter UI faces, the `medium` / `semibold` weight families, the configurable font stack and the user text scale.

mod sfnt;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily, FontId, Id, RichText};

/// `font_data` key of Inter Regular, the first face of `Proportional`.
pub const INTER: &str = "Inter";
/// `font_data` key of Inter Medium.
pub const INTER_MEDIUM: &str = "Inter-Medium";
/// `font_data` key of Inter SemiBold.
pub const INTER_SEMIBOLD: &str = "Inter-SemiBold";
/// Name of the medium-weight font family.
pub const MEDIUM: &str = "medium";
/// Name of the semibold-weight font family.
pub const SEMIBOLD: &str = "semibold";

const INTER_REGULAR_TTF: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM_TTF: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD_TTF: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

/// The medium-weight family.
pub(crate) static MEDIUM_FAMILY: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name(MEDIUM.into()));
/// The semibold-weight family.
pub(crate) static SEMIBOLD_FAMILY: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name(SEMIBOLD.into()));

static REGULAR_FACE: OnceLock<Cow<'static, [u8]>> = OnceLock::new();
static MEDIUM_FACE: OnceLock<Cow<'static, [u8]>> = OnceLock::new();
static SEMIBOLD_FACE: OnceLock<Cow<'static, [u8]>> = OnceLock::new();
static STRIPPED_FACES: Mutex<BTreeMap<(usize, usize), &'static [u8]>> = Mutex::new(BTreeMap::new());

/// `raw` with its BMP Private Use Area cmap entries removed, cached in `cell` for the process.
pub fn without_private_use(
    cell: &'static OnceLock<Cow<'static, [u8]>>,
    raw: &'static [u8],
) -> &'static [u8] {
    cell.get_or_init(|| match sfnt::without_private_use(raw) {
        Some(stripped) => Cow::Owned(stripped),
        None => {
            log::warn!("fonts: cmap rewrite failed; icon glyphs may resolve to another face");
            Cow::Borrowed(raw)
        }
    })
}

/// `raw` with its BMP Private Use Area cmap entries removed, computed once per distinct slice for the process.
pub fn private_use_stripped(raw: &'static [u8]) -> &'static [u8] {
    let key = (raw.as_ptr() as usize, raw.len());
    let mut cache = STRIPPED_FACES
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    cache
        .entry(key)
        .or_insert_with(|| match sfnt::without_private_use(raw) {
            Some(stripped) => Box::leak(stripped.into_boxed_slice()),
            None => {
                log::warn!("fonts: cmap rewrite failed; icon glyphs may resolve to another face");
                raw
            }
        })
}

/// Puts Inter first in `Proportional` and adds the weight families; call after any icon face is registered.
pub fn install(fonts: &mut FontDefinitions) {
    let faces = [
        (INTER, without_private_use(&REGULAR_FACE, INTER_REGULAR_TTF)),
        (
            INTER_MEDIUM,
            without_private_use(&MEDIUM_FACE, INTER_MEDIUM_TTF),
        ),
        (
            INTER_SEMIBOLD,
            without_private_use(&SEMIBOLD_FACE, INTER_SEMIBOLD_TTF),
        ),
    ];
    for (key, bytes) in faces {
        fonts
            .font_data
            .insert(key.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.retain(|k| k != INTER);
    proportional.insert(0, INTER.to_owned());
    let fallback = proportional.clone();

    for (family, face) in [(MEDIUM, INTER_MEDIUM), (SEMIBOLD, INTER_SEMIBOLD)] {
        let stack = std::iter::once(face.to_owned())
            .chain(fallback.iter().filter(|k| *k != face).cloned())
            .collect();
        fonts
            .families
            .insert(FontFamily::Name(family.into()), stack);
    }
}

/// A font face registered under `name` in `FontDefinitions::font_data`.
#[derive(Clone, Copy, Debug)]
pub struct FontFace {
    pub name: &'static str,
    pub bytes: &'static [u8],
    /// Drops the face's Private Use Area mappings so icon faces win those codepoints.
    pub strip_private_use: bool,
}

/// The font stack beyond Inter: extra faces, monospace order, named families and installer hooks.
#[derive(Clone, Debug, Default)]
pub struct FontConfig {
    /// Faces added to `font_data` before anything else runs.
    pub faces: Vec<FontFace>,
    /// Face keys placed at the front of `Monospace`, first entry first.
    pub monospace: Vec<String>,
    /// Named families and the face keys of each, set before the installers run.
    pub families: Vec<(String, Vec<String>)>,
    /// Hooks run in order before Inter installs, such as an icon pack's font installer.
    pub installers: Vec<fn(&mut FontDefinitions)>,
}

impl FontConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a face under `name`.
    pub fn face(mut self, name: &'static str, bytes: &'static [u8]) -> Self {
        self.faces.push(FontFace {
            name,
            bytes,
            strip_private_use: false,
        });
        self
    }

    /// Adds a face under `name` with its Private Use Area mappings removed.
    pub fn face_stripped(mut self, name: &'static str, bytes: &'static [u8]) -> Self {
        self.faces.push(FontFace {
            name,
            bytes,
            strip_private_use: true,
        });
        self
    }

    /// Appends `name` to the face keys placed at the front of `Monospace`.
    pub fn monospace_first(mut self, name: impl Into<String>) -> Self {
        self.monospace.push(name.into());
        self
    }

    /// Defines the named family `name` over the face keys `faces`.
    pub fn family<S: Into<String>>(
        mut self,
        name: impl Into<String>,
        faces: impl IntoIterator<Item = S>,
    ) -> Self {
        let faces = faces.into_iter().map(Into::into).collect();
        self.families.push((name.into(), faces));
        self
    }

    /// Appends a hook that edits the definitions before Inter installs.
    pub fn installer(mut self, installer: fn(&mut FontDefinitions)) -> Self {
        self.installers.push(installer);
        self
    }
}

/// The default stack at `scale`: egui's fallback faces under Inter and its weight families.
pub fn definitions(scale: TextScale) -> FontDefinitions {
    definitions_with(scale, &FontConfig::default())
}

/// The font stack at `scale`: faces, monospace order and families from `config`, its installers, then Inter.
pub fn definitions_with(scale: TextScale, config: &FontConfig) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for face in &config.faces {
        let bytes = if face.strip_private_use {
            private_use_stripped(face.bytes)
        } else {
            face.bytes
        };
        fonts
            .font_data
            .insert(face.name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
    monospace.retain(|k| !config.monospace.contains(k));
    for (at, key) in config.monospace.iter().enumerate() {
        monospace.insert(at.min(monospace.len()), key.clone());
    }

    for (name, faces) in &config.families {
        fonts
            .families
            .insert(FontFamily::Name(name.as_str().into()), faces.clone());
    }

    for installer in &config.installers {
        installer(&mut fonts);
    }
    install(&mut fonts);
    drop_unknown_faces(&mut fonts);
    apply_text_scale(&mut fonts, scale);
    fonts
}

/// Removes family entries that name no registered face.
fn drop_unknown_faces(fonts: &mut FontDefinitions) {
    let known: BTreeSet<&String> = fonts.font_data.keys().collect();
    let missing: BTreeSet<String> = fonts
        .families
        .values()
        .flatten()
        .filter(|key| !known.contains(key))
        .cloned()
        .collect();
    if missing.is_empty() {
        return;
    }
    log::warn!("fonts: families name unregistered faces {missing:?}; skipping them");
    for faces in fonts.families.values_mut() {
        faces.retain(|key| !missing.contains(key));
    }
}

/// Plugin that keeps the context's [`FontConfig`] out of egui memory so a memory reset leaves it.
struct ConfigSlot(FontConfig);

impl eframe::egui::Plugin for ConfigSlot {
    fn debug_name(&self) -> &'static str {
        "egui_theme_kit::fonts::ConfigSlot"
    }
}

/// Registers `config` as the stack [`install_for_context`] and [`apply_scale`] build on `ctx`.
pub fn set_config(ctx: &Context, config: FontConfig) {
    let replaced = ctx.with_plugin::<ConfigSlot, _>(|slot| slot.0 = config.clone());
    if replaced.is_none() {
        ctx.add_plugin(ConfigSlot(config));
    }
}

/// The font config registered on `ctx`, the default when none was.
pub fn config(ctx: &Context) -> FontConfig {
    ctx.with_plugin::<ConfigSlot, _>(|slot| slot.0.clone())
        .unwrap_or_default()
}

/// Installs the registered font stack at the scale recorded on `ctx`.
pub fn install_for_context(ctx: &Context) {
    ctx.set_fonts(definitions_with(text_scale(ctx), &config(ctx)));
}

/// Inter Medium at `size`.
pub fn medium(size: f32) -> FontId {
    FontId::new(size, MEDIUM_FAMILY.clone())
}

/// Inter SemiBold at `size`.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, SEMIBOLD_FAMILY.clone())
}

/// The monospace stack at `size`.
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Weight shorthands for [`RichText`]; the size still comes from the text style.
pub trait RichTextExt {
    fn medium(self) -> RichText;
    fn semibold(self) -> RichText;
}

impl RichTextExt for RichText {
    fn medium(self) -> RichText {
        self.family(MEDIUM_FAMILY.clone())
    }

    fn semibold(self) -> RichText {
        self.family(SEMIBOLD_FAMILY.clone())
    }
}

/// User text size, applied as a glyph scale on the UI faces rather than as a zoom factor.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum TextScale {
    Small = 90,
    #[default]
    Normal = 100,
    Large = 115,
    XLarge = 130,
}

impl TextScale {
    pub const ALL: [Self; 4] = [Self::Small, Self::Normal, Self::Large, Self::XLarge];

    pub fn percent(self) -> u16 {
        self as u16
    }

    pub fn factor(self) -> f32 {
        f32::from(self.percent()) / 100.0
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small (90%)",
            Self::Normal => "Normal (100%)",
            Self::Large => "Large (115%)",
            Self::XLarge => "Extra large (130%)",
        }
    }
}

/// Multiplies the glyph scale of every UI text face by `scale`; expects freshly built definitions.
pub fn apply_text_scale(fonts: &mut FontDefinitions, scale: TextScale) {
    if scale == TextScale::Normal {
        return;
    }
    let families = [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name(MEDIUM.into()),
        FontFamily::Name(SEMIBOLD.into()),
    ];
    let faces: BTreeSet<String> = families
        .iter()
        .filter_map(|family| fonts.families.get(family))
        .flatten()
        .cloned()
        .collect();
    for key in faces {
        if let Some(data) = fonts.font_data.get_mut(&key) {
            Arc::make_mut(data).tweak.scale *= scale.factor();
        }
    }
}

fn text_scale_id() -> Id {
    Id::unique("egui_theme_kit.fonts.text_scale")
}

/// The text scale last installed on `ctx`.
pub fn text_scale(ctx: &Context) -> TextScale {
    ctx.data(|d| d.get_temp::<TextScale>(text_scale_id()))
        .unwrap_or_default()
}

/// Records `scale` on `ctx` and installs `build(scale)` when it differs from the recorded scale.
pub fn set_text_scale(
    ctx: &Context,
    scale: TextScale,
    build: impl FnOnce(TextScale) -> FontDefinitions,
) {
    if text_scale(ctx) == scale {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(text_scale_id(), scale));
    ctx.set_fonts(build(scale));
    ctx.request_repaint();
}

/// [`set_text_scale`] with the font stack registered on `ctx`.
pub fn apply_scale(ctx: &Context, scale: TextScale) {
    set_text_scale(ctx, scale, |scale| definitions_with(scale, &config(ctx)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::epaint::text::{Fonts, TextOptions};

    const PUA: std::ops::RangeInclusive<u32> = 0xE000..=0xF8FF;

    fn with_phosphor() -> FontConfig {
        FontConfig::new().installer(|fonts| {
            egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular);
        })
    }

    fn with_single_face(mut fonts: FontDefinitions, family: &str, face: &str) -> FontDefinitions {
        fonts
            .families
            .insert(FontFamily::Name(family.into()), vec![face.to_owned()]);
        fonts
    }

    #[test]
    fn every_inter_face_drops_its_private_use_mappings_and_keeps_the_rest() {
        for raw in [INTER_REGULAR_TTF, INTER_MEDIUM_TTF, INTER_SEMIBOLD_TTF] {
            let before = sfnt::mapped_codepoints(raw).expect("Inter has a format 12 cmap");
            let stripped = sfnt::without_private_use(raw).expect("Inter's cmap rewrites in place");
            let after = sfnt::mapped_codepoints(&stripped).expect("rewritten cmap parses");

            assert!(before.iter().any(|c| PUA.contains(c)));
            assert!(after.iter().all(|c| !PUA.contains(c)));
            let expected: Vec<u32> = before.into_iter().filter(|c| !PUA.contains(c)).collect();
            assert_eq!(after, expected);
            assert_eq!(stripped.len(), raw.len());
        }
    }

    #[test]
    fn a_stripped_extra_face_drops_its_private_use_mappings_and_a_plain_one_keeps_them() {
        let config = FontConfig::new()
            .face_stripped("extra-stripped", INTER_REGULAR_TTF)
            .face("extra-plain", INTER_REGULAR_TTF);
        let fonts = definitions_with(TextScale::Normal, &config);
        let codepoints = |key: &str| {
            let bytes: &[u8] = fonts.font_data[key].font.as_ref().as_ref();
            sfnt::mapped_codepoints(bytes).unwrap()
        };
        let (stripped, plain) = (codepoints("extra-stripped"), codepoints("extra-plain"));
        assert!(stripped.iter().all(|c| !PUA.contains(c)));
        assert!(plain.iter().any(|c| PUA.contains(c)));
    }

    #[test]
    fn stripping_the_same_slice_twice_reuses_the_result() {
        let first = private_use_stripped(INTER_MEDIUM_TTF);
        let second = private_use_stripped(INTER_MEDIUM_TTF);
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn private_use_codepoints_fall_through_to_the_icon_face_in_every_ui_family() {
        let defs = with_single_face(
            definitions_with(TextScale::Normal, &with_phosphor()),
            "icon-only",
            "phosphor",
        );
        let ui_families = [
            FontFamily::Proportional,
            FontFamily::Name(MEDIUM.into()),
            FontFamily::Name(SEMIBOLD.into()),
        ];
        let glyph = egui_phosphor::regular::GEAR.chars().next().expect("glyph");
        let mut fonts = Fonts::new(TextOptions::default(), defs);
        let mut view = fonts.with_pixels_per_point(1.0);
        let icon = view.glyph_width(
            &FontId::new(14.0, FontFamily::Name("icon-only".into())),
            glyph,
        );
        assert!(icon > 0.0, "the icon face lacks the glyph");
        for family in ui_families {
            let width = view.glyph_width(&FontId::new(14.0, family.clone()), glyph);
            assert_eq!(
                width, icon,
                "{glyph:?} in {family:?} is not the icon face's"
            );
        }
    }

    #[test]
    fn latin_text_resolves_to_the_inter_weights() {
        let defs = with_single_face(definitions(TextScale::Normal), "inter-only", INTER);
        let defs = with_single_face(defs, "semibold-only", INTER_SEMIBOLD);
        let mut fonts = Fonts::new(TextOptions::default(), defs);
        let mut view = fonts.with_pixels_per_point(1.0);
        for ch in ['a', 'W', '0'] {
            let regular = view.glyph_width(
                &FontId::new(14.0, FontFamily::Name("inter-only".into())),
                ch,
            );
            let bold = view.glyph_width(
                &FontId::new(14.0, FontFamily::Name("semibold-only".into())),
                ch,
            );
            assert_eq!(view.glyph_width(&FontId::proportional(14.0), ch), regular);
            assert_eq!(view.glyph_width(&semibold(14.0), ch), bold);
        }
    }

    #[test]
    fn installing_twice_keeps_one_inter_entry() {
        let mut fonts = definitions(TextScale::Normal);
        install(&mut fonts);
        let proportional = &fonts.families[&FontFamily::Proportional];
        assert_eq!(proportional.iter().filter(|k| *k == INTER).count(), 1);
        assert_eq!(proportional.first().map(String::as_str), Some(INTER));
        let medium = &fonts.families[&FontFamily::Name(MEDIUM.into())];
        assert_eq!(medium.iter().filter(|k| *k == INTER_MEDIUM).count(), 1);
    }

    #[test]
    fn the_default_stack_keeps_the_fallback_faces_under_inter() {
        let fonts = definitions(TextScale::Normal);
        let defaults = FontDefinitions::default();
        let proportional = &fonts.families[&FontFamily::Proportional];
        assert_eq!(proportional[0], INTER);
        assert_eq!(
            &proportional[1..],
            defaults.families[&FontFamily::Proportional].as_slice()
        );
        assert_eq!(
            fonts.families[&FontFamily::Monospace],
            defaults.families[&FontFamily::Monospace]
        );
    }

    #[test]
    fn monospace_faces_land_first_in_order_and_named_families_are_defined() {
        let config = FontConfig::new()
            .face("mono-a", INTER_REGULAR_TTF)
            .face("mono-b", INTER_MEDIUM_TTF)
            .monospace_first("mono-a")
            .monospace_first("mono-b")
            .family("Display", ["mono-b"]);
        let fonts = definitions_with(TextScale::Normal, &config);
        let mono = &fonts.families[&FontFamily::Monospace];
        assert_eq!(&mono[..2], ["mono-a", "mono-b"]);
        let default_mono = &FontDefinitions::default().families[&FontFamily::Monospace];
        assert_eq!(&mono[2..], default_mono.as_slice());
        assert_eq!(
            fonts.families[&FontFamily::Name("Display".into())],
            vec!["mono-b".to_owned()]
        );
    }

    #[test]
    fn installers_run_in_order_and_before_inter() {
        fn first(fonts: &mut FontDefinitions) {
            let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
            proportional.insert(0, "first".into());
        }
        fn second(fonts: &mut FontDefinitions) {
            let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
            assert_eq!(proportional.first().map(String::as_str), Some("first"));
            proportional.insert(1, "second".into());
        }
        let config = FontConfig::new().installer(first).installer(second);
        let mut fonts = FontDefinitions::default();
        for installer in &config.installers {
            installer(&mut fonts);
        }
        install(&mut fonts);
        let order: Vec<&str> = fonts.families[&FontFamily::Proportional]
            .iter()
            .map(String::as_str)
            .take(3)
            .collect();
        assert_eq!(order, [INTER, "first", "second"]);
    }

    #[test]
    fn families_naming_unregistered_faces_are_pruned_not_fatal() {
        let config = FontConfig::new().monospace_first("not-registered");
        let fonts = definitions_with(TextScale::Normal, &config);
        assert!(
            !fonts.families[&FontFamily::Monospace]
                .iter()
                .any(|k| k == "not-registered")
        );
        let mut atlas = Fonts::new(TextOptions::default(), fonts);
        let mut view = atlas.with_pixels_per_point(1.0);
        assert!(view.glyph_width(&FontId::monospace(12.0), 'a') > 0.0);
    }

    #[test]
    fn text_scale_reaches_ui_faces_and_skips_dedicated_families() {
        let config = FontConfig::new()
            .face("mono-ui", INTER_REGULAR_TTF)
            .face("dedicated", INTER_REGULAR_TTF)
            .monospace_first("mono-ui")
            .family("Dedicated", ["dedicated"]);
        let fonts = definitions_with(TextScale::Large, &config);
        let large = TextScale::Large.factor();
        for key in [INTER, INTER_MEDIUM, INTER_SEMIBOLD, "mono-ui"] {
            assert_eq!(fonts.font_data[key].tweak.scale, large, "{key}");
        }
        assert_eq!(fonts.font_data["dedicated"].tweak.scale, 1.0);
    }

    #[test]
    fn text_scale_is_recorded_and_rebuilds_only_on_change() {
        let ctx = Context::default();
        let mut builds = 0;
        set_text_scale(&ctx, TextScale::Normal, |_| {
            builds += 1;
            FontDefinitions::default()
        });
        assert_eq!(builds, 0);
        set_text_scale(&ctx, TextScale::XLarge, |_| {
            builds += 1;
            FontDefinitions::default()
        });
        assert_eq!(builds, 1);
        assert_eq!(text_scale(&ctx), TextScale::XLarge);
    }

    #[test]
    fn the_registered_config_drives_the_context_stack_and_survives_a_memory_reset() {
        let ctx = Context::default();
        assert!(config(&ctx).installers.is_empty());
        set_config(&ctx, with_phosphor());
        set_config(&ctx, with_phosphor().face("extra", INTER_REGULAR_TTF));
        assert_eq!(config(&ctx).installers.len(), 1);
        assert_eq!(config(&ctx).faces.len(), 1);

        ctx.memory_mut(|mem| *mem = Default::default());
        assert_eq!(config(&ctx).faces.len(), 1);

        install_for_context(&ctx);
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
        let installed = ctx.fonts(|f| f.definitions().font_data.contains_key("phosphor"));
        assert!(installed, "the installer hook ran for the context stack");
    }

    #[test]
    fn rescaling_keeps_the_registered_installers() {
        let ctx = Context::default();
        set_config(&ctx, with_phosphor());
        apply_scale(&ctx, TextScale::Large);
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
        let (has_icons, scale) = ctx.fonts(|f| {
            let defs = f.definitions();
            (
                defs.font_data.contains_key("phosphor"),
                defs.font_data[INTER].tweak.scale,
            )
        });
        assert!(has_icons);
        assert_eq!(scale, TextScale::Large.factor());
    }
}
