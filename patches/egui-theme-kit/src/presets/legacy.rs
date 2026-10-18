//! Names and styles earlier releases saved presets under, kept so old records still decode.

use eframe::egui::Style;

use crate::glass_material;

/// Old display names and the id of the preset each became.
pub(super) const ALIASES: &[(&str, &str)] = &[
    ("Mastertech", "graphite"),
    ("Shipped Classic", "graphite"),
    ("MTech Studio", "studio"),
    ("MTech Noir", "noir"),
    ("MTech Noir Glass", "noir-glass"),
    ("MTech Glass", "frost"),
    ("Nebula Glass", "nebula"),
    ("Aurora Glass", "aurora"),
    ("Supernova Glass", "supernova"),
    ("Event Horizon Glass", "event-horizon"),
    ("Galactic Glass", "galactic"),
    ("Obsidian Glass", "obsidian"),
    ("Velvet Glass", "velvet"),
    ("Twilight Glass", "twilight"),
    ("Quartz Glass", "quartz"),
    ("Rerun MTech", "rerun"),
    ("Rerun MTech OLED", "rerun-oled"),
    ("TokyoNight", "tokyo-night"),
    ("TokyoNight Storm", "tokyo-night-storm"),
];

/// Styles a preset was saved with before its colours changed, and the id they now decode as.
pub(super) fn superseded_styles() -> [(Style, &'static str); 1] {
    [(glass_material::legacy_style(), "frost")]
}

/// The id a legacy display name decodes as, compared case-insensitively.
pub(super) fn alias(label: &str) -> Option<&'static str> {
    let label = label.trim();
    ALIASES
        .iter()
        .find(|(old, _)| old.eq_ignore_ascii_case(label))
        .map(|(_, id)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{PresetStyles, builtin_presets, find_preset};

    #[test]
    fn legacy_names_resolve_to_the_renamed_presets() {
        for (old, id) in ALIASES {
            let def = find_preset(old).unwrap_or_else(|| panic!("{old} did not resolve"));
            assert_eq!(def.id, *id, "{old}");
            assert_eq!(alias(&old.to_uppercase()), Some(*id), "{old} in capitals");
            assert_eq!(
                find_preset(&format!("  {} ", old.to_lowercase())).map(|d| d.id),
                Some(*id)
            );
        }
        assert_eq!(find_preset("  GRAPHITE ").map(|d| d.id), Some("graphite"));
        assert!(find_preset("").is_none());
        assert!(alias("no such name").is_none());
    }

    #[test]
    fn the_enum_decodes_legacy_labels() {
        for (old, id) in ALIASES {
            assert_eq!(PresetStyles::from_label(old).id(), Some(*id), "{old}");
        }
    }

    #[test]
    fn brand_names_appear_only_in_this_table() {
        let brands = ["mtech", "mastertech"];
        for def in builtin_presets() {
            for text in [def.id, def.name, def.group, def.description] {
                let lower = text.to_lowercase();
                assert!(
                    !brands.iter().any(|b| lower.contains(b)),
                    "{}: {text}",
                    def.id
                );
            }
        }
        for (old, id) in ALIASES {
            assert!(!brands.iter().any(|b| id.contains(b)), "id {id} for {old}");
        }
    }

    #[test]
    fn a_superseded_style_decodes_as_its_current_preset() {
        for (style, id) in superseded_styles() {
            let def = crate::presets::preset(id).expect("registered");
            assert_ne!(
                crate::presets::normalized(&style),
                *def.style(),
                "{id}: the superseded style must differ from the current one"
            );
        }
    }
}
